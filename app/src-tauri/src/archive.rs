//! Unpacking map downloads safely, and picking out the files that belong in the game.
//! The rules:
//!
//! - nothing may unpack to more than 3 GB (checked before, watched during, checked after);
//! - archives with symbolic or hard links aren't unpacked, and any link left over is removed;
//! - a partial unpack is a failure (a map missing its models crashes the game);
//! - only map content goes into the game, maps go straight into maps/, and map names
//!   must be plain (they end up in console commands).

use crate::sys;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const MAX_UNPACKED: u64 = 3 * 1024 * 1024 * 1024;
pub const ARCHIVES: &[&str] = &[".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2"];
const SKY_SIDES: &[&str] = &["up", "dn", "lf", "rt", "ft", "bk"];
const TOO_BIG: &str = "That download unpacks to more than 3 GB.";
const HAS_LINKS: &str = "That download contains links to other files, so it wasn't unpacked.";

/// Where each kind of file belongs inside cstrike/, and which extensions are allowed there.
fn content_exts(dir: &str) -> Option<&'static [&'static str]> {
    Some(match dir {
        "maps" => &[".bsp", ".nav", ".txt", ".res"],
        "gfx" => &[".tga", ".bmp"],
        "models" => &[".mdl"],
        "sound" => &[".wav", ".mp3"],
        "sprites" => &[".spr"],
        "overviews" => &[".txt", ".bmp", ".tga"],
        _ => return None,
    })
}

/// Map names end up in console commands (changelevel, votes), so only plain names are installed.
pub fn safe_map(name: &str) -> bool {
    !name.is_empty() && name.len() <= 31 && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

pub fn is_archive(name: &str) -> bool {
    let n = name.to_lowercase();
    ARCHIVES.iter().any(|e| n.ends_with(e))
}

/// Python's os.path.splitext(name)[1]: leading dots don't start an extension.
pub fn ext_of(name: &str) -> &str {
    let lead = name.len() - name.trim_start_matches('.').len();
    match name[lead..].rfind('.') {
        Some(i) => &name[lead + i..],
        None => "",
    }
}

fn stem_of(name: &str) -> &str {
    &name[..name.len() - ext_of(name).len()]
}

// ---- walking the unpacked files ---------------------------------------------

/// Every regular file under root (links aren't followed or returned), in a stable order.
pub fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries.into_iter().rev() {
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => stack.push(p),
                Ok(m) if m.is_file() => out.push(p),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Total size of the files under root, not following links.
pub fn tree_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            // unpackers rename and remove temporary files as they go
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else {
                total += m.len();
            }
        }
    }
    total
}

/// Removes symbolic links an archive brought along, so nothing points outside the download.
pub fn strip_links(root: &Path) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
            if m.file_type().is_symlink() {
                // a link to a folder on Windows is removed like a folder
                if std::fs::remove_file(&p).is_err() {
                    let _ = std::fs::remove_dir(&p);
                }
            } else if m.is_dir() {
                stack.push(p);
            }
        }
    }
}

pub fn same_file(a: &Path, b: &Path) -> bool {
    use std::io::Read;
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else { return false };
    if ma.len() != mb.len() {
        return false;
    }
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let (mut ba, mut bb) = (vec![0u8; 256 * 1024], vec![0u8; 256 * 1024]);
    loop {
        let na = fa.read(&mut ba).unwrap_or(0);
        if na == 0 {
            return true;
        }
        if fb.read_exact(&mut bb[..na]).is_err() || ba[..na] != bb[..na] {
            return false;
        }
    }
}

fn clear_dir(d: &Path) {
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::create_dir_all(d);
}

// ---- unpacking ---------------------------------------------------------------

/// Unpacks archive into dest, refusing more than budget bytes.
pub fn extract(archive: &Path, dest: &Path, budget: u64) -> Result<(), String> {
    if archive.to_string_lossy().to_lowercase().ends_with(".zip") {
        match extract_zip(archive, dest, budget) {
            Ok(true) => return Ok(()),
            Ok(false) => clear_dir(dest), // some ".zip" files are really rar; let bsdtar try
            Err(e) => return Err(e),
        }
    }
    // bsdtar (built into macOS and Windows 10+) reads most rar, 7z and tarballs and refuses ../ paths.
    let tar = tar_path();
    let listing = tar.as_ref().and_then(|t| {
        let mut c = sys::command(t);
        c.arg("-tvf").arg(archive);
        sys::run_output(c, Duration::from_secs(300))
    });
    let (list_ok, list_out, list_err) = listing.unwrap_or((false, String::new(), "tar isn't available".into()));
    let (total, links, readable) = read_listing(list_ok, &list_out);
    let mut tar_err: Option<String> = None;
    if readable && !links {
        if total > budget {
            return Err(TOO_BIG.into());
        }
        let mut c = sys::command(tar.as_ref().unwrap());
        c.arg("-xf").arg(archive).arg("-C").arg(dest);
        let (ok, err) = run_limited(c, dest, budget)?;
        strip_links(dest);
        if ok {
            return Ok(());
        }
        tar_err = Some(err);
    }
    let unar = sys::find_tool("unar");
    if let (Some(unar), Some(lsar)) = (unar.as_ref(), sys::find_tool("lsar")) {
        if let Some(entries) = lsar_entries(&lsar, archive) {
            let flag = |e: &serde_json::Value, k: &str| e.get(k).map(|v| v.as_bool() == Some(true) || v.as_i64() == Some(1)).unwrap_or(false);
            if entries.iter().any(|e| flag(e, "XADIsLink") || flag(e, "XADIsHardLink")) {
                return Err(HAS_LINKS.into());
            }
            let size: u64 = entries.iter().map(|e| e.get("XADFileSize").and_then(|v| v.as_u64()).unwrap_or(0)).sum();
            if size > budget {
                return Err(TOO_BIG.into());
            }
            clear_dir(dest);
            let mut c = sys::command(unar);
            c.args(["-q", "-f", "-D", "-o"]).arg(dest).arg(archive);
            let (ok, _) = run_limited(c, dest, budget)?;
            strip_links(dest);
            if ok {
                return Ok(());
            }
        }
    }
    if readable && links {
        return Err(HAS_LINKS.into());
    }
    let why_src = tar_err.unwrap_or(list_err);
    let why = why_src.trim().lines().last().unwrap_or("unknown error").to_string();
    let why = if why.is_empty() { "unknown error".to_string() } else { why };
    let hint = if unar.is_none() && cfg!(target_os = "macos") {
        " Installing unar (brew install unar) lets Loner-Strike open newer RAR files."
    } else {
        ""
    };
    Err(format!("Couldn't fully unpack the download ({why}).{hint}"))
}

/// (total size, has links, readable) from `tar -tvf` output. Anything we can't
/// read means "don't let tar unpack it".
pub fn read_listing(ok: bool, out: &str) -> (u64, bool, bool) {
    let (mut total, mut links, mut readable) = (0u64, false, ok);
    for line in out.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        match cols.get(4).and_then(|c| if c.bytes().all(|b| b.is_ascii_digit()) { c.parse::<u64>().ok() } else { None }) {
            Some(n) if cols.len() >= 5 => total += n,
            _ => {
                readable = false;
                break;
            }
        }
        links = links || line.starts_with('l') || line.starts_with('h');
    }
    (total, links, readable)
}

fn tar_path() -> Option<PathBuf> {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        let p = root.join("System32").join("tar.exe");
        return p.is_file().then_some(p).or_else(|| sys::find_tool("tar"));
    }
    let p = PathBuf::from("/usr/bin/tar");
    if cfg!(target_os = "macos") && p.is_file() {
        return Some(p);
    }
    sys::find_tool("bsdtar").or_else(|| p.is_file().then_some(p))
}

fn lsar_entries(lsar: &Path, archive: &Path) -> Option<Vec<serde_json::Value>> {
    let mut c = sys::command(lsar);
    c.arg("-j").arg(archive);
    let (ok, out, _) = sys::run_output(c, Duration::from_secs(300))?;
    if !ok {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&out).ok()?;
    v.get("lsarContents")?.as_array().cloned()
}

/// Zip files are read here directly. Ok(false): not a zip we can read, let tar try.
fn extract_zip(archive: &Path, dest: &Path, budget: u64) -> Result<bool, String> {
    use std::io::{Read, Write};
    let Ok(f) = std::fs::File::open(archive) else { return Ok(false) };
    let Ok(mut z) = zip::ZipArchive::new(f) else { return Ok(false) };
    let mut listed: u64 = 0;
    for i in 0..z.len() {
        let Ok(m) = z.by_index_raw(i) else { return Ok(false) };
        listed = listed.saturating_add(m.size());
    }
    if listed > budget {
        return Err(TOO_BIG.into());
    }
    let mut written: u64 = 0; // the sizes in a zip's index can lie, so count while writing too
    let mut buf = vec![0u8; 1024 * 1024];
    for i in 0..z.len() {
        let Ok(mut m) = z.by_index(i) else { return Ok(false) };
        if m.is_dir() || m.is_symlink() {
            continue; // links are never unpacked
        }
        let Some(rel) = m.enclosed_name() else { continue };
        let target = dest.join(rel);
        if !target.starts_with(dest) || target == dest {
            continue;
        }
        if let Some(d) = target.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        loop {
            let n = match m.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => return Ok(false),
            };
            written += n as u64;
            if written > budget {
                return Err(TOO_BIG.into());
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
    }
    Ok(true)
}

/// Runs an unpacker, stopping it as soon as dest grows past budget bytes.
///
/// The listing checked beforehand can lie about sizes, so the folder is watched
/// while the unpacker works; on Unix no single file may exceed the budget either.
/// Returns (succeeded, stderr).
pub fn run_limited(mut cmd: Command, dest: &Path, budget: u64) -> Result<(bool, String), String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0); // its own group, so stopping it also stops anything it started
        let limit = budget as libc::rlim_t;
        unsafe {
            cmd.pre_exec(move || {
                let r = libc::rlimit { rlim_cur: limit, rlim_max: limit };
                libc::setrlimit(libc::RLIMIT_FSIZE, &r);
                Ok(())
            });
        }
    }
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't run the unpacker ({e})."))?;
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            let _ = std::io::Read::read_to_string(e, &mut s);
        }
        s
    });
    let kill = |child: &mut std::process::Child| {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(500));
                if tree_size(dest) > budget {
                    kill(&mut child);
                    let _ = reader.join();
                    return Err(TOO_BIG.into());
                }
            }
            Err(e) => {
                kill(&mut child);
                return Err(e.to_string());
            }
        }
    };
    // anything it left running in its group goes too
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    let err = reader.join().unwrap_or_default();
    if tree_size(dest) > budget {
        return Err(TOO_BIG.into());
    }
    Ok((status.success(), err))
}

/// Some uploads put the real archive inside the archive.
pub fn extract_nested(root: &Path) -> Result<(), String> {
    for path in walk(root) {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if is_archive(&name) {
            let inner = sys::make_temp_dir(root, "nested-").map_err(|e| e.to_string())?;
            let size = tree_size(root);
            if size >= MAX_UNPACKED {
                return Err(TOO_BIG.into());
            }
            let _ = extract(&path, &inner, MAX_UNPACKED - size);
        }
    }
    Ok(())
}

// ---- choosing what to install --------------------------------------------------

/// Python's os.path.normpath for a relative, '/'-separated path.
fn normpath(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if out.last().map(|l| *l != "..").unwrap_or(false) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            c => out.push(c),
        }
    }
    out.join("/")
}

/// [(source file, path relative to cstrike/)] for every map file in the download,
/// and what was left out (at most 40).
pub fn plan(root: &Path) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let files = walk(root);
    let rels: Vec<Vec<String>> = files.iter().map(|p| rel_parts(root, p)).collect();
    let (picked, skipped) = plan_rels(&rels);
    (picked.into_iter().map(|(i, d)| (files[i].clone(), d)).collect(), skipped)
}

/// The parts of path below root ("a/b/c.bsp" -> ["a", "b", "c.bsp"]).
pub fn rel_parts(root: &Path, path: &Path) -> Vec<String> {
    let Ok(relp) = path.strip_prefix(root) else { return vec![] };
    relp.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .map(String::from)
        .collect()
}

/// A path from an archive listing ("dir\\x/y.bsp") as parts, dropping "", "." and anything with "..".
pub fn listed_parts(rel: &str) -> Vec<String> {
    let parts: Vec<String> = rel.split(['/', '\\']).filter(|s| !s.is_empty() && *s != ".").map(String::from).collect();
    if parts.iter().any(|p| p == "..") {
        return vec![];
    }
    parts
}

/// plan() on paths alone (from the unpacked folder, or from GameBanana's listing of an
/// archive): [(index into rels, destination)], and what was left out.
pub fn plan_rels(rels: &[Vec<String>]) -> (Vec<(usize, String)>, Vec<String>) {
    let (mut plan, mut seen, mut skipped) = (Vec::new(), std::collections::HashSet::new(), Vec::new());
    let bsps: std::collections::HashSet<String> = rels
        .iter()
        .filter_map(|p| p.last().map(|n| n.to_lowercase()))
        .filter(|n| n.ends_with(".bsp"))
        .map(|n| n[..n.len() - 4].to_string())
        .collect();
    for (index, parts) in rels.iter().enumerate() {
        if parts.is_empty() {
            continue;
        }
        let rel = parts.join("/");
        let low: Vec<String> = parts.iter().map(|p| p.to_lowercase()).collect();
        let name = low.last().unwrap().clone();
        let ext = ext_of(&name).to_string();
        let file = parts.last().unwrap().clone();
        let dirs = &low[..low.len() - 1];
        // the outermost folder that takes this kind of file, so models/dx/maps/x.mdl
        // stays a model rather than counting as something under maps/
        let idx = dirs
            .iter()
            .position(|p| content_exts(p).map(|e| e.contains(&ext.as_str())).unwrap_or(false))
            .or_else(|| dirs.iter().rposition(|p| content_exts(p).is_some()));
        let at = idx.map(|i| low[i].as_str());
        let fits = |d: &str| content_exts(d).map(|e| e.contains(&ext.as_str())).unwrap_or(false);
        let dest: Option<String> = if at == Some("maps") && fits("maps") {
            Some(format!("maps/{file}")) // the game only loads maps straight from maps/
        } else if let (Some(i), true) = (idx, at.map(fits).unwrap_or(false)) {
            Some(std::iter::once(low[i].clone()).chain(parts[i + 1..].iter().cloned()).collect::<Vec<_>>().join("/"))
        } else if ext == ".wad" {
            Some(file.clone())
        } else if ext == ".bsp" || ext == ".nav" || ext == ".res" {
            Some(format!("maps/{file}"))
        } else if ext == ".txt" && bsps.contains(&name[..name.len() - 4]) {
            Some(format!("maps/{file}"))
        } else if ext == ".spr" {
            Some(format!("sprites/{file}"))
        } else if (ext == ".tga" || ext == ".bmp") && SKY_SIDES.iter().any(|s| name[..name.len() - 4].ends_with(s)) {
            Some(format!("gfx/env/{file}"))
        } else {
            None
        };
        let Some(dest) = dest else {
            if !is_archive(&name) {
                skipped.push(rel);
            }
            continue;
        };
        let dest = normpath(&dest);
        if dest.is_empty() || dest.starts_with("..") || seen.contains(&dest.to_lowercase()) {
            continue;
        }
        if dest.starts_with("maps/") && !safe_map(stem_of(&file)) {
            skipped.push(format!("{rel} (name has characters the game console can't take)"));
            continue;
        }
        seen.insert(dest.to_lowercase());
        plan.push((index, dest));
    }
    skipped.truncate(40);
    (plan, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join("loner-strike-tests");
        sys::make_temp_dir(&base, tag).unwrap()
    }

    fn put(root: &Path, rel: &str, data: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }

    #[test]
    fn map_names() {
        assert!(safe_map("de_dust2"));
        assert!(safe_map("aim_map-v2.1"));
        assert!(!safe_map(""));
        assert!(!safe_map("de dust"));
        assert!(!safe_map("de_dust;quit"));
        assert!(!safe_map("de_\"x"));
        assert!(!safe_map(&"a".repeat(32)));
        assert!(safe_map(&"a".repeat(31)));
        assert!(!safe_map("dé_dust"));
    }

    #[test]
    fn splitext_like_python() {
        assert_eq!(ext_of("a.bsp"), ".bsp");
        assert_eq!(ext_of(".bashrc"), "");
        assert_eq!(ext_of("a.tar.gz"), ".gz");
        assert_eq!(ext_of("noext"), "");
        assert_eq!(normpath("maps/./x/../y.bsp"), "maps/y.bsp");
        assert_eq!(normpath("../x"), "../x");
    }

    #[test]
    fn listing() {
        let ok = "-rw-r--r--  0 user staff    1234 Jan  1 12:00 maps/a.bsp\n-rw-r--r--  0 user staff 10 Jan  1 12:00 b.wad\n";
        assert_eq!(read_listing(true, ok), (1244, false, true));
        let link = "lrwxr-xr-x  0 user staff       0 Jan  1 12:00 x -> /etc/passwd\n";
        assert_eq!(read_listing(true, link), (0, true, true));
        let hard = "hrw-r--r--  0 user staff       0 Jan  1 12:00 y link to x\n";
        assert!(read_listing(true, hard).1);
        assert!(!read_listing(true, "garbage line\n").2);
        assert!(!read_listing(false, "").2);
    }

    #[test]
    fn plan_flattens_and_filters() {
        let r = tmp("plan-");
        put(&r, "MyPack/cstrike/maps/sub/de_test.bsp", b"bsp");
        put(&r, "MyPack/cstrike/maps/sub/de_test.txt", b"briefing");
        put(&r, "MyPack/cstrike/maps/de_test.nav", b"nav");
        put(&r, "MyPack/de_test.wad", b"wad");
        put(&r, "MyPack/models/dx/maps/thing.mdl", b"mdl");
        put(&r, "MyPack/sound/ambience/wind.wav", b"wav");
        put(&r, "MyPack/gfx/env/skyup.tga", b"tga");
        put(&r, "loose/coolskyft.bmp", b"bmp");
        put(&r, "loose/glow.spr", b"spr");
        put(&r, "loose/readme.txt", b"readme");
        put(&r, "loose/evil.dll", b"dll");
        put(&r, "loose/server.cfg", b"cfg");
        put(&r, "loose/bad name.bsp", b"bsp");
        put(&r, "loose/inner.zip", b"zip");
        put(&r, "other/maps/de_test.bsp", b"dupe");
        let (plan, skipped) = plan(&r);
        let dests: Vec<&str> = plan.iter().map(|(_, d)| d.as_str()).collect();
        for want in [
            "maps/de_test.bsp",
            "maps/de_test.txt",
            "maps/de_test.nav",
            "de_test.wad",
            "models/dx/maps/thing.mdl",
            "sound/ambience/wind.wav",
            "gfx/env/skyup.tga",
            "gfx/env/coolskyft.bmp",
            "sprites/glow.spr",
        ] {
            assert!(dests.contains(&want), "missing {want} in {dests:?}");
        }
        assert_eq!(dests.iter().filter(|d| **d == "maps/de_test.bsp").count(), 1);
        assert!(!dests.iter().any(|d| d.contains("dll") || d.contains("cfg") || d.contains("readme")));
        assert!(skipped.iter().any(|s| s == "loose/evil.dll"));
        assert!(skipped.iter().any(|s| s == "loose/server.cfg"));
        assert!(skipped.iter().any(|s| s.starts_with("loose/bad name.bsp (name has")));
        assert!(!skipped.iter().any(|s| s.contains("inner.zip")));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_followed_or_kept() {
        let r = tmp("links-");
        put(&r, "maps/de_ok.bsp", b"x");
        std::os::unix::fs::symlink("/etc/passwd", r.join("maps/de_evil.bsp")).unwrap();
        std::os::unix::fs::symlink("/etc", r.join("models")).unwrap();
        let (p, _) = plan(&r);
        assert_eq!(p.len(), 1, "{p:?}");
        strip_links(&r);
        assert!(std::fs::symlink_metadata(r.join("maps/de_evil.bsp")).is_err());
        assert!(std::fs::symlink_metadata(r.join("models")).is_err());
        assert!(r.join("maps/de_ok.bsp").exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn tar_with_link_is_refused() {
        let r = tmp("tarlink-");
        let src = r.join("src");
        put(&src, "maps/de_a.bsp", b"bsp");
        std::os::unix::fs::symlink("/etc/passwd", src.join("maps/de_a.txt")).unwrap();
        let archive = r.join("x.tar");
        let st = Command::new(tar_path().unwrap()).arg("-cf").arg(&archive).arg("-C").arg(&src).arg("maps").status().unwrap();
        assert!(st.success());
        let dest = r.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let e = extract(&archive, &dest, MAX_UNPACKED);
        // tar's listing shows the link, and lsar's does too when unar is installed
        assert!(e.unwrap_err().contains("links"));
        assert!(std::fs::symlink_metadata(dest.join("maps/de_a.txt")).map(|m| !m.file_type().is_symlink()).unwrap_or(true));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn zip_budget_and_traversal() {
        use std::io::Write;
        let r = tmp("zip-");
        let archive = r.join("a.zip");
        {
            let f = std::fs::File::create(&archive).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("maps/de_a.bsp", o).unwrap();
            z.write_all(&[7u8; 5000]).unwrap();
            z.start_file("../../escape.txt", o).unwrap();
            z.write_all(b"nope").unwrap();
            z.finish().unwrap();
        }
        let dest = r.join("ok");
        std::fs::create_dir_all(&dest).unwrap();
        extract(&archive, &dest, MAX_UNPACKED).unwrap();
        assert_eq!(std::fs::read(dest.join("maps/de_a.bsp")).unwrap().len(), 5000);
        assert!(!r.join("escape.txt").exists() && !r.parent().unwrap().join("escape.txt").exists());

        let small = r.join("small");
        std::fs::create_dir_all(&small).unwrap();
        assert_eq!(extract(&archive, &small, 1000).unwrap_err(), TOO_BIG);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn watchdog_stops_runaway_unpacker() {
        let r = tmp("budget-");
        // an "unpacker" that keeps writing far past what its listing said
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(format!("i=0; while [ $i -lt 400 ]; do head -c 65536 /dev/zero > {}/f$i; i=$((i+1)); sleep 0.01; done", r.display()));
        let res = run_limited(c, &r, 1024 * 1024);
        assert_eq!(res.unwrap_err(), TOO_BIG);
        let after = tree_size(&r);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(tree_size(&r), after, "unpacker still running after the budget stop");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn rlimit_caps_single_file() {
        let r = tmp("rlimit-");
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(format!("head -c 3000000 /dev/zero > {}/big", r.display()));
        let res = run_limited(c, &r, 1024 * 1024);
        // either the file-size limit killed it or the watchdog caught it; never more than the budget on disk
        match res {
            Ok((ok, _)) => assert!(!ok),
            Err(e) => assert_eq!(e, TOO_BIG),
        }
        assert!(tree_size(&r) <= 1024 * 1024);
        let _ = std::fs::remove_dir_all(&r);
    }
}
