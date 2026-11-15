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

/// A file players may need from this server: in a download folder, a kind the game sends, not
/// one every player of `game` already has.
pub fn wanted(rel: &str, game: &str) -> bool {
    safe_path(rel) && crate::extras::downloadable(rel) && !crate::tools::doctor::is_stock(rel, game)
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha_file(p: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(p).map_err(|e| format!("Couldn't read {} ({e}).", p.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("Couldn't read {} ({e}).", p.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
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
    for d in DIRS {
        go(root, &root.join(d), 1, &mut out);
    }
    for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.to_lowercase().ends_with(".wad") && e.file_type().is_ok_and(|t| t.is_file()) {
            out.push((name, e.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
    out
}

/// The custom files of a server (local or on a Linux machine), with their SHA-256.
pub fn collect(g: &Game, game: &str) -> Result<Vec<File>, String> {
    let keep = |rel: &str, size: u64| size > 0 && size <= MAX_FILE && wanted(rel, game);
    let mut files = Vec::new();
    match &g.remote {
        None => {
            for (rel, size) in walk_local(&g.dir) {
                if keep(&rel, size) {
                    let sha256 = sha_file(&g.dir.join(&rel))?;
                    files.push(File { path: rel, size, sha256, etag: String::new() });
                }
            }
        }
        Some((r, dir)) => {
            let q = crate::remote::shell_quote(dir);
            let list = format!(
                "cd {q} || exit 3; find {} -type f -printf '%p\\t%s\\n' 2>/dev/null; find . -maxdepth 1 -type f -iname '*.wad' -printf '%P\\t%s\\n'; true",
                DIRS.join(" ")
            );
            let (code, out) = crate::remote::exec(r, &list)?;
            if code == 3 {
                return Err("The server's game folder isn't on that machine.".into());
            }
            let mut sizes = BTreeMap::new();
            for l in out.lines() {
                if let Some((rel, size)) = l.split_once('\t') {
                    let size = size.trim().parse::<u64>().unwrap_or(0);
                    if keep(rel, size) {
                        sizes.insert(rel.to_string(), size);
                    }
                }
            }
            if sizes.is_empty() {
                return Ok(files);
            }
            let input: Vec<u8> = sizes.keys().flat_map(|k| k.bytes().chain([0u8])).collect();
            let mut sums = BTreeMap::new();
            let code = crate::remote::exec_stream(r, &format!("cd {q} && xargs -0 sha256sum --"), &input, |l| {
                if let Some((sha, rel)) = l.split_once("  ") {
                    if sha.len() == 64 && sha.bytes().all(|c| c.is_ascii_hexdigit()) {
                        sums.insert(rel.trim_end().to_string(), sha.to_lowercase());
                    }
                }
            })?;
            if code != 0 && sums.len() < sizes.len() {
                return Err("Couldn't read all of the server's files on that machine.".into());
            }
            for (rel, size) in sizes {
                if let Some(sha256) = sums.remove(&rel) {
                    files.push(File { path: rel, size, sha256, etag: String::new() });
                }
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn read_manifest(p: &Path) -> Option<Manifest> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

/// Copies a server's custom files into its folder on this computer (only what changed since the
/// last time) and writes manifest.json, ready for scripts/r2-fastdl.sh.
pub fn prepare(id: &str, g: &Game, game: &str) -> Result<Value, String> {
    let files = collect(g, game)?;
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
        let data = match &g.remote {
            None => std::fs::read(g.dir.join(&f.path)).map_err(|e| format!("Couldn't read {} ({e}).", f.path))?,
            Some((r, rdir)) => crate::remote::read_file(r, &format!("{rdir}/{}", f.path))?,
        };
        // changed while it was being copied: the next Prepare picks it up
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
        "ok": true, "files": m.files.len(), "copied": copied, "removed": removed,
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
    } else if !f.etag.is_empty() && get("etag").trim_start_matches("W/").trim_matches('"') != f.etag.trim_matches('"') {
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
        assert_eq!(got, ["maps/my_map.bsp", "maps/my_map.res", "my.wad", "sound/loner/hi.wav"]);
        let id = format!("fdtest{}", std::process::id());
        let r = prepare(&id, &g, "cstrike").unwrap();
        assert_eq!(r["files"], 4);
        assert_eq!(r["copied"], 4);
        let st = stage_dir(&id);
        assert_eq!(std::fs::read_to_string(st.join("files/maps/my_map.bsp")).unwrap(), "bsp");
        // nothing changed: nothing copied again; a file gone from the server leaves the folder
        std::fs::remove_file(dir.join("my.wad")).unwrap();
        let r = prepare(&id, &g, "cstrike").unwrap();
        assert_eq!((r["files"].as_u64(), r["copied"].as_u64(), r["removed"].as_u64()), (Some(3), Some(0), Some(1)));
        let m = read_manifest(&st.join("manifest.json")).unwrap();
        assert_eq!(m.files[0].sha256, hex(&Sha256::digest(b"bsp")));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&st);
    }
}
