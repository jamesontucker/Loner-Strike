//! The Settings page's app-wide settings: updates, keeping servers running, phone
//! sign-out, storage, game folders added by hand, diagnostics, and which of a game's
//! settings files a backup takes (backup.rs).

use crate::app_api::{edit_settings, load_settings};
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

type Reply = (StatusCode, Json<Value>);

fn ok(v: Value) -> Reply {
    (StatusCode::OK, Json(v))
}
fn err(msg: impl Into<String>) -> Reply {
    (StatusCode::OK, Json(json!({"error": msg.into()})))
}
fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

// ---- simple switches --------------------------------------------------------------

async fn api_prefs() -> Json<Value> {
    let s = load_settings();
    Json(json!({"auto_update": !s.manual_updates, "keep_servers": s.keep_servers, "game_folders": s.game_folders, "notify_joins": !s.mute_joins}))
}

#[derive(Deserialize)]
struct PrefsReq {
    auto_update: Option<bool>,
    keep_servers: Option<bool>,
    notify_joins: Option<bool>,
}

async fn api_prefs_set(headers: HeaderMap, Json(req): Json<PrefsReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let s = edit_settings(|s| {
        if let Some(v) = req.auto_update {
            s.manual_updates = !v;
        }
        if let Some(v) = req.keep_servers {
            s.keep_servers = v;
        }
        if let Some(v) = req.notify_joins {
            s.mute_joins = !v;
        }
    });
    ok(json!({"auto_update": !s.manual_updates, "keep_servers": s.keep_servers, "game_folders": s.game_folders, "notify_joins": !s.mute_joins}))
}

// ---- phones -----------------------------------------------------------------------

/// A new phone access key: every phone signed in until now has to scan the new code.
async fn api_phone_reset(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let token = crate::app_api::new_token();
    edit_settings(|s| s.token = token.clone());
    *st.token.write().unwrap() = token;
    ok(crate::app_api::phone_status(&st))
}

// ---- storage ----------------------------------------------------------------------

fn dir_size(dir: &Path) -> u64 {
    crate::archive::walk(dir).iter().filter_map(|p| std::fs::symlink_metadata(p).ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

/// Downloaded mods and maps (kept so reinstalls don't download again).
fn download_dirs() -> Vec<PathBuf> {
    vec![crate::setup::fetch::cache_dir(), crate::sys::app_dir().join("cache").join("downloads")]
}

/// Map pictures: GameBanana screenshots and the converted thumbnails.
fn picture_files() -> Vec<PathBuf> {
    let cache = crate::sys::app_dir().join("cache");
    let mut out: Vec<PathBuf> = crate::archive::walk(&cache.join("gbshots"));
    for e in std::fs::read_dir(&cache).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_file() && p.extension().is_some_and(|x| x == "png" || x == "jpg") {
            out.push(p);
        }
    }
    out
}

async fn api_storage() -> Json<Value> {
    let v = tokio::task::spawn_blocking(|| {
        let downloads: u64 = download_dirs().iter().map(|d| dir_size(d)).sum();
        let pictures: u64 = picture_files().iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
        json!({"downloads": downloads, "pictures": pictures, "folder": crate::sys::app_dir()})
    })
    .await
    .unwrap_or(json!({}));
    Json(v)
}

#[derive(Deserialize)]
struct ClearReq {
    what: String,
}

async fn api_storage_clear(headers: HeaderMap, Json(req): Json<ClearReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    if crate::setup::current().state == "running" {
        return err("Wait for setup to finish first.");
    }
    let what = req.what.clone();
    tokio::task::spawn_blocking(move || match what.as_str() {
        "downloads" => {
            for d in download_dirs() {
                let _ = std::fs::remove_dir_all(&d);
                let _ = std::fs::create_dir_all(&d);
            }
        }
        "pictures" => {
            for p in picture_files() {
                let _ = std::fs::remove_file(p);
            }
        }
        _ => {}
    })
    .await
    .ok();
    ok(json!({"ok": true}))
}

#[derive(Deserialize)]
struct OpenUrlReq {
    url: String,
}

/// A web address that may be handed to the system: http(s) only, one plain line.
fn openable(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/')) && url.len() <= 2048 && url.bytes().all(|c| c.is_ascii_graphic() && c != b'"' && c != b'\\')
}

/// Opens an outside link in the default browser: the app's window is a web view that
/// can't open new tabs, so the page sends its links here (main.tsx). The computer only.
async fn api_open_url(headers: HeaderMap, Json(req): Json<OpenUrlReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    if !openable(&req.url) {
        return err("That address can't be opened.");
    }
    // the address goes straight to the program, never through a shell
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&req.url).spawn().map(crate::sys::reap);
    #[cfg(windows)]
    let r = crate::sys::command("rundll32").args(["url.dll,FileProtocolHandler", &req.url]).spawn().map(crate::sys::reap);
    #[cfg(not(any(target_os = "macos", windows)))]
    let r: std::io::Result<()> = Err(std::io::Error::other("unsupported"));
    match r {
        Ok(_) => ok(json!({"ok": true})),
        Err(e) => err(format!("Couldn't open the link ({e}).")),
    }
}

/// Shows the app's own folder (settings, caches, records) in Finder or Explorer.
async fn api_open_folder(headers: HeaderMap) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let dir = crate::sys::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&dir).spawn().map(crate::sys::reap);
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&dir).spawn().map(crate::sys::reap);
    #[cfg(not(any(target_os = "macos", windows)))]
    let r: std::io::Result<()> = Err(std::io::Error::other("unsupported"));
    match r {
        Ok(_) => ok(json!({"ok": true})),
        Err(e) => err(format!("Couldn't open the folder ({e}).")),
    }
}

// ---- game folders added by hand -----------------------------------------------------

/// A Half-Life folder the scan didn't find, checked like the ones it does.
pub fn custom_install(dir: &str) -> Option<crate::steam::Install> {
    let hl = PathBuf::from(dir);
    let games: Vec<String> = ["cstrike", "czero"].iter().filter(|g| hl.join(g).join("liblist.gam").is_file()).map(|g| g.to_string()).collect();
    if games.is_empty() {
        return None;
    }
    if cfg!(windows) {
        return Some(crate::steam::Install { hl, kind: "windows".into(), bottle: None, games, dedicated: false });
    }
    // on a Mac it has to be in a CrossOver bottle, so it can run
    let s = hl.to_string_lossy().to_string();
    let i = s.find("/CrossOver/Bottles/")?;
    let bottle = s[i + 19..].split('/').next()?.to_string();
    s.contains("/drive_c/").then(|| crate::steam::Install { hl, kind: "crossover".into(), bottle: Some(bottle), games, dedicated: false })
}

#[derive(Deserialize)]
struct FolderReq {
    dir: String,
}

async fn api_folder_add(headers: HeaderMap, Json(req): Json<FolderReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let dir = crate::sys::clean_path(&req.dir);
    if custom_install(&dir).is_none() {
        let mac = if cfg!(target_os = "macos") { " On a Mac it also has to be inside a CrossOver bottle." } else { "" };
        return err(format!("That isn't a Half-Life folder with Counter-Strike 1.6 or Condition Zero in it (it should have a cstrike or czero folder).{mac}"));
    }
    let s = edit_settings(|s| {
        if !s.game_folders.contains(&dir) {
            s.game_folders.push(dir);
        }
    });
    ok(json!({"ok": true, "game_folders": s.game_folders}))
}

async fn api_folder_remove(headers: HeaderMap, Json(req): Json<FolderReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let s = edit_settings(|s| s.game_folders.retain(|d| *d != crate::sys::clean_path(&req.dir)));
    ok(json!({"ok": true, "game_folders": s.game_folders}))
}

// ---- where new servers go -------------------------------------------------------------

#[derive(Deserialize)]
struct ServersDirReq {
    /// empty: back to the usual place
    #[serde(default)]
    dir: String,
}

fn servers_dir_info(st: &Shared) -> Value {
    let client = crate::servers::client(st);
    let default = client.as_ref().and_then(|c| crate::servers::default_root(c).ok());
    let chosen = crate::app_api::load_settings().servers_dir;
    json!({"dir": chosen, "default": default, "mac": client.as_ref().is_some_and(|c| c.kind == "crossover")})
}

async fn api_servers_dir(State(st): State<Shared>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || servers_dir_info(&st)).await.unwrap_or_default())
}

/// The folder new servers go in. Servers made before stay where they are.
async fn api_servers_dir_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ServersDirReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let dir = crate::sys::clean_path(&req.dir);
        if !dir.is_empty() {
            let client = crate::servers::client(&st).ok_or("Set up the game on this computer first.")?;
            crate::servers::servers_dir_ok(Path::new(&dir), &client)?;
        }
        edit_settings(|s| s.servers_dir = (!dir.is_empty()).then_some(dir));
        Ok(servers_dir_info(&st))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into()));
    match r {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

// ---- diagnostics --------------------------------------------------------------------

/// A plain-text summary for bug reports (diagnostics.rs). No passwords, tokens or keys.
async fn api_diagnostics(State(st): State<Shared>) -> Json<Value> {
    let version = st.app.get().map(|a| a.package_info().version.to_string()).unwrap_or_default();
    let text = tokio::task::spawn_blocking(move || crate::diagnostics::build(&crate::diagnostics::gather(&st, version))).await.unwrap_or_default();
    Json(json!({"text": text}))
}

// ---- a game's setup files (backup.rs packs them; old setup files are read there too) --

/// Files the Game Mode Menu keeps that are worth moving to another computer or server.
const SETUP_EXT: &[&str] = &["cfg", "ini", "json", "txt", "html"];
/// Not these: the panel's password, live status, logs, and who joined; the server's
/// history (stats, reports, matches, recent maps); and what the plugin writes for itself
/// as it runs (the mode it's in, the last map, a pending reload).
pub(crate) const SETUP_SKIP: &[&str] = &[
    "panel_rcon.txt", "status.json", "seen.json", "seen.log", "seen.log.reading", "friends.cfg", "navqueue.txt",
    "stats.json", "stats.json.part", "playtime.txt", "reports.json", "matches.json", "scrim_status.json", "recent.txt", "bots.txt",
    "current.txt", "premode.txt", "lastmap.txt", "session.txt", "lastpreset.txt", "crashmap.txt", "reloadnote.txt",
];

/// Separates the files machine_setup_files prints.
const FILE_MARK: &str = "\n@@LS-SETUP-FILE@@\n";

/// For a server on another machine: the data files as the machine has them (the plugin
/// saves settings there: presets, mode settings, map lists, mutes), name -> text, in one
/// command. None if the machine can't be reached.
pub(crate) fn machine_setup_files(g: &crate::game::Game) -> Option<Vec<(String, String)>> {
    let (r, dir) = g.remote.as_ref()?;
    let cmd = format!(
        "cd {}/addons/amxmodx/data/gamemodes 2>/dev/null || exit 0; find . -maxdepth 3 -type f -size -1000k \\( -name '*.cfg' -o -name '*.ini' -o -name '*.json' -o -name '*.txt' -o -name '*.html' \\) | while IFS= read -r f; do printf {}; printf '%s\\n' \"${{f#./}}\"; cat \"$f\"; done",
        crate::remote::shell_quote(dir),
        crate::remote::shell_quote(&FILE_MARK.replace('\n', "\\n"))
    );
    let (code, out) = crate::remote::exec_quick_for(r, &cmd, 60).ok()?;
    if code != 0 {
        return None;
    }
    Some(parse_machine_files(&out))
}

/// machine_setup_files' output: (name, text) for each file.
fn parse_machine_files(out: &str) -> Vec<(String, String)> {
    out.split(FILE_MARK).skip(1).filter_map(|p| p.split_once('\n')).map(|(name, text)| (name.to_string(), text.to_string())).collect()
}

pub(crate) fn exportable(rel: &Path) -> bool {
    let name = rel.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let ext = rel.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    SETUP_EXT.contains(&ext.as_str()) && !SETUP_SKIP.contains(&name.as_str()) && rel.components().count() <= 3
}

/// A relative path from an imported file, if it's safe to write.
pub(crate) fn safe_rel(rel: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    if !parts.iter().all(|p| !p.is_empty() && *p != "." && *p != ".." && p.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))) {
        return None;
    }
    let p: PathBuf = parts.iter().collect();
    exportable(&p).then_some(p)
}

#[derive(Deserialize)]
struct AddFileReq {
    section: String,
}

/// Picks a file on this computer and installs it like a GameBanana download.
async fn api_add_file(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<AddFileReq>) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let (Some(g), Some(app)) = (st.game(), st.app.get().cloned()) else { return err("No game found.") };
    let Some(sec) = crate::content::section(&req.section) else { return err("Unknown section.") };
    let r = tokio::task::spawn_blocking(move || {
        let mut exts: Vec<&str> = crate::gamebanana::local_single(sec.key).iter().map(|e| e.trim_start_matches('.')).collect();
        exts.extend(["zip", "rar", "7z"]);
        let title = if sec.key == "maps" { "Add a map".to_string() } else { format!("Add {}", sec.label.to_lowercase()) };
        let Some(path) = app.dialog().file().set_title(&title).add_filter("Maps and downloads", &exts).blocking_pick_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        g.store.start_local_install(sec.key, &path).map(|job| json!({"ok": true, "job": job})).map_err(|e| e.message())
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    match r {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/app/prefs", get(api_prefs).post(api_prefs_set))
        .route("/api/app/storage", get(api_storage))
        .route("/api/app/diagnostics", get(api_diagnostics))
}

/// The app window only: these open dialogs or folders on this computer, or change who can connect.
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/app/phone/reset", post(api_phone_reset))
        .route("/api/app/storage/clear", post(api_storage_clear))
        .route("/api/app/open-folder", post(api_open_folder))
        .route("/api/app/open-url", post(api_open_url))
        .route("/api/app/game-folder", post(api_folder_add))
        .route("/api/app/servers-folder", get(api_servers_dir).post(api_servers_dir_set))
        .route("/api/app/game-folder/remove", post(api_folder_remove))
        .route("/api/gb/add-file", post(api_add_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openable_links() {
        assert!(openable("https://lonerstrike.com") && openable("http://example.com/a?b=c&d=e#f") && openable("https://discord.gg/K9eBHVVwZE"));
        for bad in ["", "https://", "file:///etc/passwd", "javascript:alert(1)", "steam://connect/1.2.3.4", "https://a b", "https://a\nb", "https://x\"y", "https:///x", "ftp://x"] {
            assert!(!openable(bad), "{bad}");
        }
    }

    #[test]
    fn setup_files() {
        assert!(safe_rel("global.cfg").is_some() && safe_rel("presets/fun.cfg").is_some() && safe_rel("admins.json").is_some());
        assert!(safe_rel("../x.cfg").is_none() && safe_rel("a/../../x.cfg").is_none() && safe_rel("/etc/x.cfg").is_none());
        assert!(safe_rel("panel_rcon.txt").is_none() && safe_rel("evil.exe").is_none() && safe_rel("a b.cfg").is_none());
        assert!(safe_rel("a/b/c/d.cfg").is_none());
        // the server's history and what the plugin writes as it runs stay where they are
        for f in ["stats.json", "reports.json", "matches.json", "recent.txt", "current.txt", "seen.json"] {
            assert!(safe_rel(f).is_none(), "{f}");
        }
        assert!(safe_rel("gags.txt").is_some() && safe_rel("favorites.txt").is_some());
        let out = format!("{FILE_MARK}global.cfg\nbot_quota 5\n{FILE_MARK}presets/a.cfg\n");
        assert_eq!(parse_machine_files(&out), vec![("global.cfg".to_string(), "bot_quota 5\n".to_string()), ("presets/a.cfg".to_string(), String::new())]);
    }
}
