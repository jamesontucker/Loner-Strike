//! Fast downloads from Loner-Strike's own mirror (Cloudflare R2 behind dl.lonerstrike.com): a
//! server's custom files go up to `fastdl/<server id>/` there, and its sv_downloadurl points at
//! that folder, so players get missing maps, models and sounds over HTTP from the first join.
//!
//! Only the mirror's owner can upload (the keys live in ~/.config/lonerstrike/r2.env on their Mac;
//! the app only looks at whether that file is there, never inside it, and nothing goes to a server
//! machine). The app does the parts that need the game: which files a server has that players may
//! not (never the game's own, `doctor::is_stock`), copying them into a folder on this computer
//! with a manifest (path, size, SHA-256), and checking afterwards that the mirror serves exactly
//! those files. scripts/r2-fastdl.sh uploads the prepared folder (only what changed).
//!
//! Routes (this computer only): GET /api/fastdl/:id, POST /api/fastdl/:id/prepare,
//! POST /api/fastdl/:id/check.

use axum::{
    extract::Path as UrlPath,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::game::Game;
use crate::server::Shared;

/// Where the mirror's fast download folders are.
pub const MIRROR: &str = "https://dl.lonerstrike.com/fastdl";
/// The folders players download from (and top-level .wad files).
const DIRS: &[&str] = &["maps", "sound", "models", "sprites", "gfx", "overviews"];
/// A file bigger than this isn't offered (the game's own limit on a download is lower).
const MAX_FILE: u64 = 512 << 20;
/// At most this many files are checked on the mirror one by one.
const MAX_CHECK: usize = 2000;

/// The mirror's address: LS_FASTDL_MIRROR in a test (a local fake mirror), else the real one.
pub fn mirror_base() -> String {
    std::env::var("LS_FASTDL_MIRROR").ok().filter(|u| crate::extras::valid_url(u)).map(|u| u.trim_end_matches('/').to_string()).unwrap_or_else(|| MIRROR.to_string())
}

/// sv_downloadurl for a server: its own folder on the mirror (the game adds "/maps/x.bsp").
pub fn mirror_url(id: &str) -> String {
    format!("{}/{id}", mirror_base())
}

/// Only the mirror's owner uploads: their keys file is on this computer (only whether it's
/// there is looked at), or a test points at a fake mirror.
pub fn can_upload() -> bool {
    std::env::var_os("LS_FASTDL_MIRROR").is_some() || dirs::home_dir().is_some_and(|h| h.join(".config/lonerstrike/r2.env").is_file())
}

/// The folder a server's files are prepared in for scripts/r2-fastdl.sh.
pub fn stage_dir(id: &str) -> PathBuf {
    crate::sys::app_dir().join("fastdl").join(id)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct File {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    /// what the mirror answered on upload (the script fills it in): the copy a check expects
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub etag: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Manifest {
    pub server: String,
    pub game: String,
    #[serde(default)]
    pub made: u64,
    pub files: Vec<File>,
}

/// A path that is the same in a URL, an R2 key and the game: plain characters, no ".." or
/// leading slash.
pub fn safe_path(rel: &str) -> bool {
    !rel.is_empty()
        && rel.len() <= 200
        && !rel.starts_with('/')
        && rel.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
        && rel.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-./+!()".contains(&c))
}

/// A file players may need from this server: in a download folder, a kind the game sends, and
/// not Valve's: nothing on the stock lists of Half-Life, CS 1.6 or Condition Zero ever goes to the
/// public mirror, whichever game the server runs (a CS 1.6 player lacking a Condition Zero file
/// is the map's problem; we never hand out Valve's files). `_game` is kept for the callers.
pub fn wanted(rel: &str, _game: &str) -> bool {
    safe_path(rel) && crate::extras::downloadable(rel) && !valves(rel)
}

/// On any of Valve's lists (stock_valve, stock_cstrike, stock_czero; doctor::is_stock for both
/// games), or a texture file with Valve's Condition Zero naming (czcs_*.wad, czde_*.wad), listed
/// or not.
pub fn valves(rel: &str) -> bool {
    crate::tools::doctor::is_stock(rel, "cstrike") || crate::tools::doctor::is_stock(rel, "czero") || cz_named(rel)
}

/// czcs_*.wad / czde_*.wad (any folder, any case): Valve's Condition Zero texture naming.
fn cz_named(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    name.ends_with(".wad") && (name.starts_with("czcs_") || name.starts_with("czde_"))
}

/// The custom maps that name a file only Condition Zero has (its textures, sky or sounds): on a
/// CS 1.6 server their players see missing textures for those, since the mirror never has them.
pub fn needs_czero(maps: &BTreeMap<String, MapNeeds>, game: &str) -> Vec<String> {
    if game.eq_ignore_ascii_case("czero") {
        return Vec::new();
    }
    let cz_only = |rel: &str| cz_named(rel) || (crate::tools::doctor::is_stock(rel, "czero") && !crate::tools::doctor::is_stock(rel, "cstrike"));
    maps.iter()
        .filter(|(_, n)| {
            let (wads, sky) = worldspawn_of(&n.ent);
            wads.iter().any(|w| cz_only(w))
                || sky.is_some_and(|s| ["up", "dn", "lf", "rt", "ft", "bk"].iter().any(|side| cz_only(&format!("gfx/env/{s}{side}.tga"))))
                || crate::extras::parse_res(&n.res).iter().any(|r| cz_only(r.trim_start_matches('/')))
        })
        .map(|(m, _)| m.clone())
        .collect()
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Opens `rel` under `root` without following a link anywhere on the way: on Unix each folder
/// is opened from the one before with O_NOFOLLOW (openat), so nothing swapped in between can
/// send it outside the game folder; elsewhere every part is checked and the opened path must
/// still be inside. Only a plain file opens.
pub fn open_inside(root: &Path, rel: &str) -> Result<std::fs::File, String> {
    let refuse = || format!("{rel} isn't a plain file in the game folder (a link, or outside it), so it isn't offered.");
    if !safe_path(rel) {
        return Err(refuse());
    }
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::fd::FromRawFd;
        use std::os::unix::ffi::OsStrExt;
        let c = |s: &[u8]| CString::new(s).map_err(|_| refuse());
        let root_c = c(root.as_os_str().as_bytes())?;
        // SAFETY: plain libc calls on C strings we own; every fd opened here is closed or handed
        // to a File exactly once
        let mut dir = unsafe { libc::open(root_c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        if dir < 0 {
            return Err(format!("Couldn't open the game folder ({}).", std::io::Error::last_os_error()));
        }
        let parts: Vec<&str> = rel.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            let name = c(part.as_bytes())?;
            let last = i + 1 == parts.len();
            let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | if last { libc::O_NONBLOCK } else { libc::O_DIRECTORY };
            let next = unsafe { libc::openat(dir, name.as_ptr(), flags) };
            unsafe { libc::close(dir) };
            if next < 0 {
                return Err(refuse());
            }
            dir = next;
        }
        let f = unsafe { std::fs::File::from_raw_fd(dir) };
        if !f.metadata().is_ok_and(|m| m.is_file()) {
            return Err(refuse());
        }
        Ok(f)
    }
    #[cfg(not(unix))]
    {
        let mut p = root.to_path_buf();
        for part in rel.split('/') {
            p.push(part);
            if std::fs::symlink_metadata(&p).map_or(true, |m| m.file_type().is_symlink()) {
                return Err(refuse());
            }
        }
        let f = std::fs::File::open(&p).map_err(|_| refuse())?;
        let inside = match (std::fs::canonicalize(&p), std::fs::canonicalize(root)) {
            (Ok(a), Ok(b)) => a.starts_with(b),
            _ => false,
        };
        if !inside || !f.metadata().is_ok_and(|m| m.is_file()) {
            return Err(refuse());
        }
        Ok(f)
    }
}

/// A file's bytes and SHA-256, read through open_inside.
fn read_inside(root: &Path, rel: &str) -> Result<(Vec<u8>, String), String> {
    let mut f = open_inside(root, rel)?;
    let mut data = Vec::new();
    (&mut f).take(MAX_FILE + 1).read_to_end(&mut data).map_err(|e| format!("Couldn't read {rel} ({e})."))?;
    if data.len() as u64 > MAX_FILE {
        return Err(format!("{rel} is too big to offer."));
    }
    let sha = hex(&Sha256::digest(&data));
    Ok((data, sha))
}

fn sha_inside(root: &Path, rel: &str) -> Result<(u64, String), String> {
    let mut f = open_inside(root, rel)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut n_all = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("Couldn't read {rel} ({e})."))?;
        if n == 0 {
            break;
        }
        n_all += n as u64;
        h.update(&buf[..n]);
    }
    Ok((n_all, hex(&h.finalize())))
}

/// Run on the server's machine as `python3 -I -S -c HASH_PY <game folder>` (isolated: never
/// imports anything from the game folder or the current one, which can hold downloaded files,
/// and ignores PYTHON* settings; no cd into the folder) with the wanted paths on stdin, NUL
/// separated, relative to the folder: each is opened folder by folder with O_NOFOLLOW (no link anywhere on the way, so
/// nothing outside the folder), must be a plain file, and is hashed from that open file.
/// Prints "size<TAB>sha256<TAB>path", or "refused<TAB>path".
pub const HASH_PY: &str = r#"import os, sys, stat, hashlib
root = os.open(sys.argv[1], os.O_RDONLY | os.O_DIRECTORY)
for rel in sys.stdin.buffer.read().split(b'\0'):
    if not rel:
        continue
    fd = None
    try:
        name = rel.decode('ascii')
        parts = rel.split(b'/')
        d = os.dup(root)
        try:
            for p in parts[:-1]:
                if p in (b'', b'.', b'..'):
                    raise OSError('bad path')
                n = os.open(p, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=d)
                os.close(d)
                d = n
            if parts[-1] in (b'', b'.', b'..'):
                raise OSError('bad path')
            fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=d)
        finally:
            os.close(d)
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode):
            raise OSError('not a file')
        h = hashlib.sha256()
        f = os.fdopen(fd, 'rb')
        fd = None
        size = 0
        with f:
            for b in iter(lambda: f.read(65536), b''):
                h.update(b)
                size += len(b)
        sys.stdout.write('%d\t%s\t%s\n' % (size, h.hexdigest(), name))
    except (OSError, UnicodeDecodeError):
        if fd is not None:
            os.close(fd)
        sys.stdout.write('refused\t%s\n' % rel.decode('ascii', 'replace'))
"#;

/// The command that hashes on the server's machine: `q` is the game folder, shell-quoted.
fn hash_cmd(q: &str) -> String {
    format!("python3 -I -S -c {} {q}", crate::remote::shell_quote(HASH_PY))
}

/// HASH_PY's answer for the paths asked about (anything else it printed is ignored).
fn parse_hashes(out: &str, asked: &std::collections::BTreeSet<String>) -> (Vec<File>, usize) {
    let (mut files, mut refused) = (Vec::new(), 0);
    for l in out.lines() {
        let f: Vec<&str> = l.splitn(3, '\t').collect();
        match f.as_slice() {
            ["refused", rel] if asked.contains(*rel) => refused += 1,
            [size, sha, rel] if asked.contains(*rel) && sha.len() == 64 && sha.bytes().all(|c| c.is_ascii_hexdigit()) => {
                if let Ok(size) = size.parse::<u64>() {
                    files.push(File { path: rel.to_string(), size, sha256: sha.to_lowercase(), etag: String::new() });
                }
            }
            _ => {}
        }
    }
    (files, refused)
}

/// The files under a local game folder (relative, "/" separated), depth-limited.
fn walk_local(root: &Path) -> Vec<(String, u64)> {
    fn go(root: &Path, dir: &Path, depth: u32, out: &mut Vec<(String, u64)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() && depth < 8 {
                go(root, &p, depth + 1, out);
            } else if ft.is_file() {
                if let (Ok(rel), Ok(m)) = (p.strip_prefix(root), e.metadata()) {
                    out.push((rel.to_string_lossy().replace('\\', "/"), m.len()));
                }
            }
        }
    }
    let mut out = Vec::new();
    // (read_dir's entries say what they are without following links, so a linked file or
    // folder is never listed or entered; the download folders themselves are checked too)
    for d in DIRS {
        if std::fs::symlink_metadata(root.join(d)).is_ok_and(|m| m.is_dir()) {
            go(root, &root.join(d), 1, &mut out);
        }
    }
    for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.to_lowercase().ends_with(".wad") && e.file_type().is_ok_and(|t| t.is_file()) {
            out.push((name, e.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
    out
}

/// What a custom map makes a joining player fetch besides the .bsp: from its entity lump (the
/// first block, worldspawn: "wad" and "skyname") and its .res text.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct MapNeeds {
    pub ent: String,
    pub res: String,
}

/// worldspawn's texture files (file names only) and sky name, from the entity lump's text.
pub fn worldspawn_of(ent: &str) -> (Vec<String>, Option<String>) {
    let first = ent.split('}').next().unwrap_or("");
    let key = |k: &str| -> Option<String> {
        let at = first.find(&format!("\"{k}\""))? + k.len() + 2;
        let rest = &first[at..];
        let start = rest.find('"')? + 1;
        let end = rest[start..].find('"')? + start;
        Some(rest[start..end].to_string())
    };
    let wads = key("wad").unwrap_or_default().split(';').filter_map(|w| w.replace('\\', "/").rsplit('/').next().map(|s| s.trim().to_string())).filter(|w| w.to_lowercase().ends_with(".wad")).collect();
    let sky = key("skyname").map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)));
    (wads, sky)
}

/// The files to offer, out of the custom ones present: each custom map with its .res, .txt,
/// overview, the .res's entries, its texture files and sky; and the custom models, sounds and
/// sprites (game modes and plugins precache them). Loose texture files, skies and overviews of
/// maps the server doesn't have stay off the mirror. Names match without regard to case, as the
/// game does on Windows; the path offered is the file's own.
pub fn select(present: &[(String, u64)], maps: &BTreeMap<String, MapNeeds>) -> Vec<(String, u64)> {
    let by_lower: BTreeMap<String, &(String, u64)> = present.iter().map(|p| (p.0.to_lowercase(), p)).collect();
    let mut want: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (m, needs) in maps {
        for rel in [format!("maps/{m}.bsp"), format!("maps/{m}.res"), format!("maps/{m}.txt"), format!("overviews/{m}.bmp"), format!("overviews/{m}.tga"), format!("overviews/{m}.txt")] {
            want.insert(rel.to_lowercase());
        }
        for rel in crate::extras::parse_res(&needs.res) {
            want.insert(rel.trim_start_matches('/').to_lowercase());
        }
        let (wads, sky) = worldspawn_of(&needs.ent);
        for w in wads {
            want.insert(w.to_lowercase());
        }
        if let Some(sky) = sky {
            for side in ["up", "dn", "lf", "rt", "ft", "bk"] {
                want.insert(format!("gfx/env/{sky}{side}.tga").to_lowercase());
            }
        }
    }
    let mut out: Vec<(String, u64)> = present
        .iter()
        .filter(|(rel, _)| {
            let l = rel.to_lowercase();
            want.contains(&l) || l.starts_with("models/") || l.starts_with("sound/") || l.starts_with("sprites/")
        })
        .cloned()
        .collect();
    out.retain(|(rel, _)| by_lower.contains_key(&rel.to_lowercase()));
    out.sort();
    out.dedup();
    out
}

/// The custom maps among the present files (their names).
fn custom_maps(present: &[(String, u64)]) -> Vec<String> {
    present.iter().filter_map(|(rel, _)| rel.strip_prefix("maps/").and_then(|f| f.strip_suffix(".bsp")).filter(|m| !m.contains('/')).map(String::from)).collect()
}

/// Run on the server's machine like HASH_PY (`python3 -I -S -c NEEDS_PY <game folder>`), map
/// names on stdin: each map's entity lump start and .res text, as JSON, read without following
/// links.
pub const NEEDS_PY: &str = r#"import os, sys, json, stat
root = os.open(sys.argv[1], os.O_RDONLY | os.O_DIRECTORY)
def opennf(rel):
    parts = rel.split('/')
    d = os.dup(root)
    try:
        for p in parts[:-1]:
            n = os.open(p, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=d)
            os.close(d)
            d = n
        fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=d)
    finally:
        os.close(d)
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        os.close(fd)
        raise OSError('not a file')
    return os.fdopen(fd, 'rb')
out = {}
for m in sys.stdin.read().split():
    if '/' in m or m.startswith('.'):
        continue
    info = {'ent': '', 'res': ''}
    try:
        with opennf('maps/%s.bsp' % m) as f:
            h = f.read(12)
            off = int.from_bytes(h[4:8], 'little', signed=True)
            ln = int.from_bytes(h[8:12], 'little', signed=True)
            if off >= 0 and ln > 0:
                f.seek(off)
                info['ent'] = f.read(min(ln, 65536)).decode('latin-1').split('}')[0]
    except (OSError, ValueError):
        pass
    try:
        with opennf('maps/%s.res' % m) as f:
            info['res'] = f.read(65536).decode('latin-1')
    except OSError:
        pass
    out[m] = info
sys.stdout.write(json.dumps(out))
"#;

/// A map's needs from a local game folder (read through open_inside).
fn local_needs(root: &Path, map: &str) -> MapNeeds {
    use std::io::{Seek, SeekFrom};
    let mut n = MapNeeds::default();
    if let Ok(mut f) = open_inside(root, &format!("maps/{map}.bsp")) {
        let mut h = [0u8; 12];
        if f.read_exact(&mut h).is_ok() {
            let off = i32::from_le_bytes(h[4..8].try_into().unwrap());
            let len = i32::from_le_bytes(h[8..12].try_into().unwrap());
            if off >= 0 && len > 0 && f.seek(SeekFrom::Start(off as u64)).is_ok() {
                let mut buf = Vec::new();
                let _ = (&mut f).take(len.min(65536) as u64).read_to_end(&mut buf);
                let text: String = buf.iter().map(|&b| b as char).collect();
                n.ent = text.split('}').next().unwrap_or("").to_string();
            }
        }
    }
    if let Ok(f) = open_inside(root, &format!("maps/{map}.res")) {
        let mut buf = Vec::new();
        let _ = f.take(65536).read_to_end(&mut buf);
        n.res = buf.iter().map(|&b| b as char).collect();
    }
    n
}

/// The custom files of a server (local or on a Linux machine) that players may need, with their
/// SHA-256.
pub fn collect(g: &Game, game: &str) -> Result<Vec<File>, String> {
    collect_all(g, game).map(|(f, _)| f)
}

/// collect, and the custom maps that need Condition Zero's own files (needs_czero).
pub fn collect_all(g: &Game, game: &str) -> Result<(Vec<File>, Vec<String>), String> {
    let keep = |rel: &str, size: u64| size > 0 && size <= MAX_FILE && wanted(rel, game);
    let mut files = Vec::new();
    let cz;
    match &g.remote {
        None => {
            let present: Vec<(String, u64)> = walk_local(&g.dir).into_iter().filter(|(r, s)| keep(r, *s)).collect();
            let maps: BTreeMap<String, MapNeeds> = custom_maps(&present).into_iter().map(|m| (m.clone(), local_needs(&g.dir, &m))).collect();
            cz = needs_czero(&maps, game);
            for (rel, _) in select(&present, &maps) {
                // a link swapped in since the listing is refused here, never followed
                if let Ok((size, sha256)) = sha_inside(&g.dir, &rel) {
                    if size > 0 && size <= MAX_FILE {
                        files.push(File { path: rel, size, sha256, etag: String::new() });
                    }
                }
            }
        }
        Some((r, dir)) => {
            let q = crate::remote::shell_quote(dir);
            let list = format!(
                "cd {q} || exit 3; find -P {} -type f -printf '%p\\t%s\\n' 2>/dev/null; find -P . -maxdepth 1 -type f -iname '*.wad' -printf '%P\\t%s\\n'; true",
                DIRS.join(" ")
            );
            let (code, out) = crate::remote::exec(r, &list)?;
            if code == 3 {
                return Err("The server's game folder isn't on that machine.".into());
            }
            let mut present = Vec::new();
            for l in out.lines() {
                if let Some((rel, size)) = l.split_once('\t') {
                    let rel = rel.strip_prefix("./").unwrap_or(rel);
                    let size = size.trim().parse::<u64>().unwrap_or(0);
                    if keep(rel, size) {
                        present.push((rel.to_string(), size));
                    }
                }
            }
            let names = custom_maps(&present);
            let mut maps: BTreeMap<String, MapNeeds> = names.iter().map(|m| (m.clone(), MapNeeds::default())).collect();
            if !names.is_empty() {
                let mut text = String::new();
                let code = crate::remote::exec_stream(r, &needs_cmd(&q), names.join("\n").as_bytes(), |l| text.push_str(l))?;
                if code != 0 {
                    return Err("Couldn't read the server's maps on that machine (it needs python3, which Loner-Strike's setup installs).".into());
                }
                if let Ok(v) = serde_json::from_str::<BTreeMap<String, Value>>(&text) {
                    for (m, info) in v {
                        if let Some(n) = maps.get_mut(&m) {
                            n.ent = info["ent"].as_str().unwrap_or("").to_string();
                            n.res = info["res"].as_str().unwrap_or("").to_string();
                        }
                    }
                }
            }
            cz = needs_czero(&maps, game);
            let asked: std::collections::BTreeSet<String> = select(&present, &maps).into_iter().map(|(r, _)| r).collect();
            if asked.is_empty() {
                return Ok((files, cz));
            }
            let input: Vec<u8> = asked.iter().flat_map(|k| k.bytes().chain([0u8])).collect();
            let mut out = String::new();
            let code = crate::remote::exec_stream(r, &hash_cmd(&q), &input, |l| {
                out.push_str(l);
                out.push('\n');
            })?;
            if code != 0 {
                return Err("Couldn't read the server's files on that machine (it needs python3, which Loner-Strike's setup installs).".into());
            }
            let (got, _refused) = parse_hashes(&out, &asked);
            files.extend(got.into_iter().filter(|f| f.size > 0 && f.size <= MAX_FILE));
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((files, cz))
}

/// NEEDS_PY's command, like hash_cmd.
fn needs_cmd(q: &str) -> String {
    format!("python3 -I -S -c {} {q}", crate::remote::shell_quote(NEEDS_PY))
}

fn read_manifest(p: &Path) -> Option<Manifest> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

/// Copies a server's custom files into its folder on this computer (only what changed since the
/// last time) and writes manifest.json, ready for scripts/r2-fastdl.sh.
pub fn prepare(id: &str, g: &Game, game: &str) -> Result<Value, String> {
    let (files, needs_cz) = collect_all(g, game)?;
    let dir = stage_dir(id);
    let out = dir.join("files");
    std::fs::create_dir_all(&out).map_err(|e| format!("Couldn't make {} ({e}).", out.display()))?;
    let before: BTreeMap<String, String> = read_manifest(&dir.join("manifest.json")).map(|m| m.files.into_iter().map(|f| (f.path, f.sha256)).collect()).unwrap_or_default();
    let (mut copied, mut bytes) = (0usize, 0u64);
    for f in &files {
        let dst = out.join(&f.path);
        bytes += f.size;
        if before.get(&f.path) == Some(&f.sha256) && std::fs::metadata(&dst).is_ok_and(|m| m.len() == f.size) {
            continue;
        }
        if !wanted(&f.path, game) {
            return Err(format!("{} isn't a file players download, so it isn't offered.", f.path));
        }
        let data = match &g.remote {
            None => read_inside(&g.dir, &f.path)?.0,
            Some((r, rdir)) => {
                if !crate::remote::plain_file_under(r, rdir, &f.path)? {
                    return Err(format!("{} is a link or not a plain file on that machine, so it isn't offered.", f.path));
                }
                crate::remote::read_file(r, &format!("{rdir}/{}", f.path))?
            }
        };
        // only the very bytes hashed from the game folder (opened without following links) go
        // up: anything else read here (changed, or swapped for a link since) stops the prepare
        if hex(&Sha256::digest(&data)) != f.sha256 {
            return Err(format!("{} changed while it was being copied. Try again.", f.path));
        }
        if let Some(d) = dst.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make a folder for {} ({e}).", f.path))?;
        }
        std::fs::write(&dst, &data).map_err(|e| format!("Couldn't write {} ({e}).", f.path))?;
        copied += 1;
    }
    // files the server doesn't have any more leave the folder (the script deletes them from
    // the mirror too)
    let keep: std::collections::HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let mut removed = 0;
    for (rel, _) in walk_all(&out) {
        if !keep.contains(rel.as_str()) && std::fs::remove_file(out.join(&rel)).is_ok() {
            removed += 1;
        }
    }
    let made = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let m = Manifest { server: id.to_string(), game: game.to_string(), made, files };
    let text = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("manifest.json"), text).map_err(|e| format!("Couldn't write the manifest ({e})."))?;
    Ok(json!({
        "ok": true, "files": m.files.len(), "copied": copied, "removed": removed, "needs_czero": needs_cz,
        "mb": (bytes as f64 / 1_048_576.0 * 10.0).round() / 10.0,
        "dir": dir.to_string_lossy(), "command": format!("scripts/r2-fastdl.sh {id}"),
    }))
}

/// Every file under a folder (relative paths).
fn walk_all(root: &Path) -> Vec<(String, u64)> {
    fn go(root: &Path, dir: &Path, depth: u32, out: &mut Vec<(String, u64)>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() && depth < 10 => go(root, &p, depth + 1, out),
                Ok(t) if t.is_file() => {
                    if let Ok(rel) = p.strip_prefix(root) {
                        out.push((rel.to_string_lossy().replace('\\', "/"), e.metadata().map(|m| m.len()).unwrap_or(0)));
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    go(root, root, 0, &mut out);
    out
}

/// One file as a player's game would fetch it: what's wrong with the answer, if anything.
fn check_one(http: &reqwest::blocking::Client, url: &str, f: &File) -> (Option<String>, Option<String>, Option<String>) {
    // as the game asks (no compression), and as a browser does (Cloudflare may compress a
    // text type for it: the game's downloader can't unpack that)
    let r = match http.head(url).header("Accept-Encoding", "gzip, br").timeout(Duration::from_secs(10)).send() {
        Ok(r) => r,
        Err(e) => return (Some(format!("didn't answer ({})", if e.is_timeout() { "timed out".to_string() } else { "no connection".to_string() })), None, None),
    };
    let h = r.headers();
    let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let cache = Some(get("cf-cache-status")).filter(|s| !s.is_empty());
    let control = Some(get("cache-control")).filter(|s| !s.is_empty());
    if !r.status().is_success() {
        return (Some(format!("isn't there (HTTP {})", r.status().as_u16())), cache, control);
    }
    let ctype = get("content-type").to_lowercase();
    let why = if ctype.starts_with("text/html") {
        Some("comes back as a web page, not the file".to_string())
    } else if !get("content-encoding").is_empty() && get("content-encoding") != "identity" {
        Some(format!("is sent compressed ({}), which the game can't unpack: upload it as application/octet-stream", get("content-encoding")))
    } else if get("content-length").parse::<u64>().ok().is_some_and(|n| n != f.size) {
        Some(format!("is {} bytes there, {} here", get("content-length"), f.size))
    } else if !f.etag.is_empty() && !get("etag").is_empty() && get("etag").trim_start_matches("W/").trim_matches('"') != f.etag.trim_matches('"') {
        Some("is an older copy: Cloudflare may still have it cached (it refreshes within 5 minutes)".to_string())
    } else {
        None
    };
    (why, cache, control)
}

/// Does the mirror serve exactly this server's files, the way the game downloads them?
pub fn check(id: &str, g: &Game, game: &str, set_url: &str) -> Result<Value, String> {
    let files = collect(g, game)?;
    let base = mirror_url(id);
    let http = crate::sys::http();
    let mut problems: Vec<Value> = Vec::new();
    if set_url.trim_end_matches('/') != base {
        problems.push(json!({"path": "", "why": format!("the server's download address is {}, not the mirror's {base}", if set_url.is_empty() { "empty".to_string() } else { set_url.to_string() })}));
    }
    // the manifest past Cloudflare's cache: what was uploaded last
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let up: Option<Manifest> = http
        .get(format!("{base}/manifest.json?fresh={now}"))
        .timeout(Duration::from_secs(10))
        .send()
        .ok()
        .filter(|r| r.status().is_success())
        .and_then(|r| crate::caps::read_up_to(r, "fast download manifest", 8 << 20).ok())
        .and_then(|b| serde_json::from_slice(&b).ok());
    let Some(up) = up else {
        return Ok(json!({"ok": false, "url": base, "files": files.len(), "uploaded": false,
            "problems": [{"path": "", "why": "nothing is uploaded for this server yet: prepare its files and run the upload script"}]}));
    };
    let on_mirror: BTreeMap<&str, &File> = up.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut to_check = Vec::new();
    for f in &files {
        match on_mirror.get(f.path.as_str()) {
            None => problems.push(json!({"path": f.path, "why": "isn't uploaded"})),
            Some(m) if m.sha256 != f.sha256 => problems.push(json!({"path": f.path, "why": "changed on the server since the upload"})),
            Some(m) => to_check.push((*m).clone()),
        }
    }
    let extra = up.files.iter().filter(|m| !files.iter().any(|f| f.path == m.path)).count();
    let checked = to_check.len().min(MAX_CHECK);
    let results: Vec<(String, (Option<String>, Option<String>, Option<String>))> = std::thread::scope(|s| {
        let chunks: Vec<Vec<File>> = to_check.into_iter().take(MAX_CHECK).collect::<Vec<_>>().chunks(1.max(checked.div_ceil(8))).map(|c| c.to_vec()).collect();
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|c| {
                let base = base.clone();
                s.spawn(move || c.iter().map(|f| (f.path.clone(), check_one(http, &format!("{base}/{}", f.path), f))).collect::<Vec<_>>())
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut cache: BTreeMap<String, usize> = BTreeMap::new();
    let mut controls: BTreeMap<String, usize> = BTreeMap::new();
    for (path, (why, c, cc)) in results {
        *cache.entry(c.unwrap_or_else(|| "none".into())).or_default() += 1;
        *controls.entry(cc.unwrap_or_else(|| "none".into())).or_default() += 1;
        if let Some(why) = why {
            problems.push(json!({"path": path, "why": why}));
        }
    }
    if controls.keys().any(|c| c == "none") {
        problems.push(json!({"path": "", "why": "some files have no Cache-Control, so Cloudflare may keep an old copy for hours: upload them again with the script"}));
    }
    Ok(json!({"ok": problems.is_empty(), "url": base, "files": files.len(), "uploaded": true, "checked": checked, "extra": extra,
        "problems": problems, "cloudflare": cache, "cache_control": controls}))
}

// ---- API (this computer only) ---------------------------------------------------------

type Reply = (StatusCode, Json<Value>);

fn server(id: &str) -> Result<(crate::servers::Entry, std::sync::Arc<Game>), String> {
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || id.is_empty() || id.len() > 48 {
        return Err("Unknown server.".into());
    }
    let e = crate::servers::get(id).filter(|e| e.ready).ok_or("That server isn't there any more, or isn't set up yet.")?;
    let g = crate::servers::game_for(id, || None).ok_or("That server isn't there any more, or isn't set up yet.")?;
    Ok((e, g))
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Reply {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => (StatusCode::OK, Json(v)),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": e}))),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": "Loner-Strike hit an internal error. Try again."}))),
    }
}

async fn api_info(UrlPath(id): UrlPath<String>) -> Reply {
    blocking(move || {
        let (_, _) = server(&id)?;
        let staged = read_manifest(&stage_dir(&id).join("manifest.json"));
        Ok(json!({"can_upload": can_upload(), "url": mirror_url(&id), "prepared": staged.map(|m| json!({"files": m.files.len(), "made": m.made}))}))
    })
    .await
}

async fn api_prepare(headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Reply {
    if !crate::panel_api::from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    blocking(move || {
        if !can_upload() {
            return Err("Only Loner-Strike's own servers use its mirror.".into());
        }
        let (e, g) = server(&id)?;
        prepare(&id, &g, &e.game)
    })
    .await
}

async fn api_check(headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Reply {
    if !crate::panel_api::from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    blocking(move || {
        let (e, g) = server(&id)?;
        check(&id, &g, &e.game, &crate::extras::fastdl_url_of(&g))
    })
    .await
}

pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/fastdl/:id", get(api_info))
        .route("/api/fastdl/:id/prepare", post(api_prepare))
        .route("/api/fastdl/:id/check", post(api_check))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fastdl_paths_and_stock() {
        assert!(safe_path("maps/br_valley.bsp"));
        for bad in ["", "/maps/x.bsp", "maps/../x.bsp", "maps//x.bsp", "maps/x y.bsp", "maps/x\".bsp", "./maps/x.bsp"] {
            assert!(!safe_path(bad), "{bad}");
        }
        assert!(wanted("maps/br_valley.bsp", "cstrike"));
        assert!(wanted("models/loner/heli.mdl", "cstrike"));
        assert!(!wanted("maps/de_dust2.bsp", "cstrike"), "a stock map");
        assert!(!wanted("maps/br_valley.nav", "cstrike"), "the game never sends a nav");
        assert!(!wanted("sound/weapons/ak47-1.wav", "cstrike"), "a stock sound");
        assert!(!wanted("addons/amxmodx/plugins/x.amxx", "cstrike"));
        assert!(wanted("loner.wad", "cstrike"));
        assert!(!wanted("halflife.wad", "cstrike"));
        // Valve's files for either game never go up, whichever game the server runs
        assert!(!wanted("czcs_italy.wad", "cstrike"), "Condition Zero's own texture file on a CS 1.6 server");
        assert!(!wanted("czcs_italy.wad", "czero"));
        // Condition Zero's naming, even off the lists
        for n in ["czcs_miami.wad", "czde_quarry.wad", "CZDE_Quarry.WAD", "maps/czcs_x.wad"] {
            assert!(!wanted(n, "cstrike"), "{n}");
        }
        assert!(wanted("my_czcs_miami.wad", "cstrike") && wanted("czcs.wad", "cstrike"));
        let cz = BTreeMap::from([
            ("cz_map".to_string(), MapNeeds { ent: "{\n\"wad\" \"\\\\x\\\\czcs_italy.wad;\\\\x\\\\mine.wad\"\n}".into(), res: String::new() }),
            ("plain".to_string(), MapNeeds { ent: "{\n\"wad\" \"\\\\x\\\\halflife.wad\"\n}".into(), res: String::new() }),
            ("miami".to_string(), MapNeeds { ent: "{\n\"wad\" \"\\\\x\\\\czcs_miami.wad\"\n}".into(), res: String::new() }),
        ]);
        assert_eq!(needs_czero(&cz, "cstrike"), ["cz_map", "miami"]);
        assert!(needs_czero(&cz, "czero").is_empty());
    }

    /// A game folder with links pointing outside it (a secret next to it): (root, game dir).
    #[cfg(unix)]
    fn linked_folder(tag: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::symlink;
        let root = crate::sys::test_root().join(format!("fastdl-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("cstrike");
        let outside = root.join("outside");
        std::fs::create_dir_all(dir.join("maps")).unwrap();
        std::fs::create_dir_all(dir.join("sound")).unwrap();
        std::fs::create_dir_all(outside.join("keys")).unwrap();
        std::fs::write(outside.join("id_rsa.wav"), "SECRET").unwrap();
        std::fs::write(outside.join("keys/k.mdl"), "SECRET").unwrap();
        std::fs::write(outside.join("secret.wad"), "SECRET").unwrap();
        std::fs::write(dir.join("maps/real_map.bsp"), "bsp").unwrap();
        symlink(outside.join("id_rsa.wav"), dir.join("sound/linked.wav")).unwrap(); // a linked file
        symlink(outside.join("keys"), dir.join("sound/linkdir")).unwrap(); // a linked folder
        symlink(outside.join("keys"), dir.join("models")).unwrap(); // a download folder that's a link
        symlink(outside.join("secret.wad"), dir.join("linked.wad")).unwrap();
        (root, dir)
    }

    #[cfg(unix)]
    #[test]
    fn fastdl_refuses_links_out_of_the_folder_locally() {
        let (root, dir) = linked_folder("local");
        let g = Game::on_port(&root, "cstrike", 27098, true);
        let got: Vec<String> = collect(&g, "cstrike").unwrap().into_iter().map(|f| f.path).collect();
        assert_eq!(got, ["maps/real_map.bsp"], "only the real file, never a link");
        for rel in ["sound/linked.wav", "sound/linkdir/k.mdl", "models/k.mdl", "linked.wad", "maps/../../outside/id_rsa.wav"] {
            assert!(open_inside(&dir, rel).is_err(), "{rel} opened");
        }
        assert_eq!(read_inside(&dir, "maps/real_map.bsp").unwrap().0, b"bsp");
        // a file swapped for a link after the listing: refused when it's read
        let id = format!("fdlink{}", std::process::id());
        std::fs::remove_file(dir.join("maps/real_map.bsp")).unwrap();
        std::os::unix::fs::symlink(root.join("outside/id_rsa.wav"), dir.join("maps/real_map.bsp")).unwrap();
        assert!(open_inside(&dir, "maps/real_map.bsp").is_err());
        let r = prepare(&id, &g, "cstrike").unwrap();
        assert_eq!(r["files"], 0);
        let staged = walk_all(&stage_dir(&id));
        assert!(staged.iter().all(|(p, _)| p == "manifest.json"), "{staged:?}");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(stage_dir(&id));
    }

    /// The remote side: HASH_PY (what runs on the server's machine) run here on a folder with
    /// links, as the machine would; only the real file comes back.
    #[cfg(unix)]
    #[test]
    fn fastdl_refuses_links_out_of_the_folder_remotely() {
        use std::io::Write;
        let (root, dir) = linked_folder("remote");
        let asked: std::collections::BTreeSet<String> =
            ["maps/real_map.bsp", "sound/linked.wav", "sound/linkdir/k.mdl", "models/k.mdl", "linked.wad", "maps/../../outside/id_rsa.wav"].iter().map(|s| s.to_string()).collect();
        let input: Vec<u8> = asked.iter().flat_map(|k| k.bytes().chain([0u8])).collect();
        // the very command the machine runs, started from inside the game folder (the worst case),
        // with modules planted there that would give it away if imported
        for m in ["hashlib", "os", "stat", "sys", "sitecustomize"] {
            std::fs::write(dir.join(format!("{m}.py")), format!("open({:?}, 'w').write('imported')\nraise SystemExit(7)\n", root.join("planted_ran").to_string_lossy())).unwrap();
        }
        let cmd = hash_cmd(&crate::remote::shell_quote(&dir.to_string_lossy()));
        assert!(cmd.starts_with("python3 -I -S -c ") && !cmd.contains("cd "), "{cmd}");
        let mut child = std::process::Command::new("sh").arg("-c").arg(&cmd).current_dir(&dir).env("PYTHONPATH", &dir).env("PYTHONSTARTUP", dir.join("os.py")).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().expect("sh");
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{out:?}");
        assert!(!root.join("planted_ran").exists(), "a module planted in the game folder was imported");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(!text.contains(&hex(&Sha256::digest(b"SECRET"))), "a secret was hashed: {text}");
        let (files, refused) = parse_hashes(&text, &asked);
        assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["maps/real_map.bsp"]);
        assert_eq!(files[0].sha256, hex(&Sha256::digest(b"bsp")));
        assert_eq!(refused, 5);
        // a line for a path it wasn't asked about is ignored
        let (f2, _) = parse_hashes(&format!("3\t{}\tetc/passwd\n", "a".repeat(64)), &asked);
        assert!(f2.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// End to end against a fake mirror (run by hand, see docs/notes/W2.md):
    /// LS_FASTDL_E2E=<server folder holding cstrike/> LS_FASTDL_E2E_ID=<id> LS_FASTDL_E2E_STEP=prepare|check
    /// LS_FASTDL_MIRROR=http://127.0.0.1:<port>/fastdl cargo test fastdl_e2e -- --ignored --nocapture
    /// prepare writes the folder for scripts/r2-fastdl.sh --from (printed); check compares the
    /// mirror with the server and fetches what a player joining on each custom map asks for.
    #[test]
    #[ignore]
    fn fastdl_e2e() {
        // a dry run from a saved listing (find's "path<TAB>size" lines) and NEEDS_PY's answer, as
        // collect() makes on a Linux machine: what would be offered, with sizes
        if let (Ok(list), Ok(needs)) = (std::env::var("LS_FASTDL_LIST"), std::env::var("LS_FASTDL_NEEDS")) {
            let present: Vec<(String, u64)> = std::fs::read_to_string(list).unwrap().lines().filter_map(|l| l.split_once('\t')).map(|(r, n)| (r.strip_prefix("./").unwrap_or(r).to_string(), n.trim().parse().unwrap_or(0))).filter(|(r, n)| *n > 0 && *n <= MAX_FILE && wanted(r, "cstrike")).collect();
            let v: BTreeMap<String, Value> = serde_json::from_str(&std::fs::read_to_string(needs).unwrap()).unwrap();
            let maps: BTreeMap<String, MapNeeds> = custom_maps(&present).into_iter().map(|m| { let i = &v[&m]; (m, MapNeeds { ent: i["ent"].as_str().unwrap_or("").into(), res: i["res"].as_str().unwrap_or("").into() }) }).collect();
            let sel = select(&present, &maps);
            for (r, n) in &sel {
                println!("WOULD\t{n}\t{r}");
            }
            println!("MAPS\t{}", maps.keys().cloned().collect::<Vec<_>>().join(" "));
            println!("NEEDS_CZERO\t{}", needs_czero(&maps, "cstrike").join(" "));
            return;
        }
        let (Ok(hl), Ok(id), Ok(step)) = (std::env::var("LS_FASTDL_E2E"), std::env::var("LS_FASTDL_E2E_ID"), std::env::var("LS_FASTDL_E2E_STEP")) else { return };
        let g = Game::on_port(Path::new(&hl), "cstrike", 27015, true);
        if step == "prepare" {
            let r = prepare(&id, &g, "cstrike").unwrap();
            println!("PREPARED {r}");
            return;
        }
        let r = check(&id, &g, "cstrike", &mirror_url(&id)).unwrap();
        println!("CHECK {}", serde_json::to_string_pretty(&r).unwrap());
        assert_eq!(r["ok"], true);
        // a joining player's list for each custom map: the map, what its .res names, its WADs and
        // sky (extras::map_files, the same list the game builds), each fetched as sv_downloadurl + "/" + path
        let roots = vec![g.dir.clone()];
        let http = crate::sys::http();
        let mut fetched = 0;
        for e in std::fs::read_dir(g.dir.join("maps")).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(map) = name.strip_suffix(".bsp") else { continue };
            if !wanted(&format!("maps/{name}"), "cstrike") {
                continue;
            }
            for rel in crate::extras::map_files(&roots, map) {
                if !wanted(&rel, "cstrike") {
                    continue;
                }
                let url = format!("{}/{rel}", mirror_url(&id));
                let body = http.get(&url).send().unwrap().error_for_status().unwrap_or_else(|e| panic!("{url}: {e}")).bytes().unwrap();
                let want = std::fs::read(g.dir.join(&rel)).unwrap();
                assert!(body.as_ref() == want.as_slice(), "{rel} differs on the mirror");
                fetched += 1;
                println!("resolved {rel} ({} bytes)", body.len());
            }
        }
        assert!(fetched > 0, "no custom map to try");
    }

    #[test]
    fn fastdl_offers_what_maps_and_modes_need() {
        let present: Vec<(String, u64)> = ["maps/my_map.bsp", "maps/my_map.res", "maps/My_Map.txt", "overviews/my_map.bmp", "overviews/gone_map.bmp", "MyTex.wad", "loose.wad",
            "gfx/env/skyaup.tga", "gfx/env/otherup.tga", "sound/loner/hi.wav", "models/loner/heli.mdl", "sprites/x.spr", "sound/ambience/res_only.wav"]
            .iter().map(|s| (s.to_string(), 1)).collect();
        let maps = BTreeMap::from([("my_map".to_string(), MapNeeds { ent: "{\n\"wad\" \"\\\\half-life\\\\cstrike\\\\mytex.wad;\\\\x\\\\halflife.wad\"\n\"skyname\" \"skya\"\n}".into(), res: "// res\nsound/ambience/res_only.wav\n".into() })]);
        let got: Vec<String> = select(&present, &maps).into_iter().map(|(r, _)| r).collect();
        assert_eq!(got, ["MyTex.wad", "gfx/env/skyaup.tga", "maps/My_Map.txt", "maps/my_map.bsp", "maps/my_map.res", "models/loner/heli.mdl", "overviews/my_map.bmp", "sound/ambience/res_only.wav", "sound/loner/hi.wav", "sprites/x.spr"]);
        assert_eq!(custom_maps(&present), ["my_map"]);
    }

    /// NEEDS_PY (run on the server's machine) reads a map's worldspawn and .res, isolated like
    /// HASH_PY, and never through a link.
    #[cfg(unix)]
    #[test]
    fn fastdl_remote_needs_reader() {
        use std::io::Write;
        let (root, dir) = linked_folder("needs");
        let ent = b"{\n\"wad\" \"\\\\x\\\\mytex.wad\"\n\"skyname\" \"skya\"\n}\n{\n}";
        let mut bsp = vec![30u8, 0, 0, 0, 12, 0, 0, 0];
        bsp.extend_from_slice(&(ent.len() as i32).to_le_bytes());
        bsp.extend_from_slice(ent);
        std::fs::write(dir.join("maps/real_map.bsp"), &bsp).unwrap();
        std::fs::write(dir.join("maps/real_map.res"), "sound/a.wav\n").unwrap();
        std::os::unix::fs::symlink(root.join("outside/id_rsa.wav"), dir.join("maps/linked.res")).unwrap();
        std::fs::write(dir.join("os.py"), format!("open({:?}, 'w').write('x')\n", root.join("planted_ran").to_string_lossy())).unwrap();
        let cmd = needs_cmd(&crate::remote::shell_quote(&dir.to_string_lossy()));
        let mut child = std::process::Command::new("sh").arg("-c").arg(&cmd).current_dir(&dir).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"real_map\nlinked\n../x\n").unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success() && !root.join("planted_ran").exists());
        let v: BTreeMap<String, Value> = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(worldspawn_of(v["real_map"]["ent"].as_str().unwrap()), (vec!["mytex.wad".to_string()], Some("skya".to_string())));
        assert_eq!(v["real_map"]["res"], "sound/a.wav\n");
        assert_eq!(v["linked"]["res"], "", "a linked .res isn't read");
        assert!(!v.contains_key("../x"));
        // and the local reader agrees
        let n = local_needs(&dir, "real_map");
        assert_eq!(worldspawn_of(&n.ent).0, ["mytex.wad"]);
        assert_eq!(local_needs(&dir, "linked").res, "");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fastdl_collects_and_prepares_a_local_server() {
        let root = crate::sys::test_root().join(format!("fastdl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("cstrike");
        for (rel, body) in [("maps/my_map.bsp", "bsp"), ("maps/my_map.res", "res"), ("maps/my_map.nav", "nav"), ("maps/de_dust2.bsp", "stock"), ("sound/loner/hi.wav", "wav"), ("my.wad", "wad"), ("addons/x.amxx", "plugin")] {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let g = Game::on_port(&root, "cstrike", 27099, true);
        let got: Vec<String> = collect(&g, "cstrike").unwrap().into_iter().map(|f| f.path).collect();
        // (my.wad: no map names it, so it stays off the mirror)
        assert_eq!(got, ["maps/my_map.bsp", "maps/my_map.res", "sound/loner/hi.wav"]);
        let id = format!("fdtest{}", std::process::id());
        let r = prepare(&id, &g, "cstrike").unwrap();
        assert_eq!(r["files"], 3);
        assert_eq!(r["copied"], 3);
        let st = stage_dir(&id);
        assert_eq!(std::fs::read_to_string(st.join("files/maps/my_map.bsp")).unwrap(), "bsp");
        // nothing changed: nothing copied again; a file gone from the server leaves the folder
        std::fs::remove_file(dir.join("sound/loner/hi.wav")).unwrap();
        let r = prepare(&id, &g, "cstrike").unwrap();
        assert_eq!((r["files"].as_u64(), r["copied"].as_u64(), r["removed"].as_u64()), (Some(2), Some(0), Some(1)));
        let m = read_manifest(&st.join("manifest.json")).unwrap();
        assert_eq!(m.files[0].sha256, hex(&Sha256::digest(b"bsp")));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&st);
    }
}
