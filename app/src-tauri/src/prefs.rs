//! The Settings page's app-wide settings: updates, keeping servers running, phone
//! sign-out, storage, game folders added by hand, diagnostics, and exporting or
//! importing a game's or server's setup.

use crate::app_api::{edit_settings, load_settings};
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
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
    let dir = req.dir.trim().trim_end_matches(['/', '\\']).to_string();
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
    let s = edit_settings(|s| s.game_folders.retain(|d| d != req.dir.trim()));
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
        let dir = req.dir.trim().trim_end_matches(['/', '\\']).to_string();
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

/// A plain-text summary for bug reports. No passwords or keys.
async fn api_diagnostics(State(st): State<Shared>) -> Json<Value> {
    let version = st.app.get().map(|a| a.package_info().version.to_string()).unwrap_or_default();
    let game = st.local_game();
    let text = tokio::task::spawn_blocking(move || {
        let s = load_settings();
        let mut out = vec![format!("Loner-Strike {version} on {} {}", std::env::consts::OS, std::env::consts::ARCH)];
        match &game {
            Some(g) => {
                let installed = crate::setup::record::Record::open(&g.dir).component_ids();
                out.push(format!("Game: {} at {}", g.name, g.dir.display()));
                out.push(format!("Installed: {}", if installed.is_empty() { "nothing".into() } else { installed.join(", ") }));
            }
            None => out.push("Game: none found".into()),
        }
        for i in crate::steam::find_installs() {
            out.push(format!("Install: {} ({}{}) games: {}", i.hl.display(), i.kind, i.bottle.map(|b| format!(" bottle {b}")).unwrap_or_default(), i.games.join(", ")));
        }
        for e in crate::servers::load() {
            out.push(format!("Server: {} ({}, port {}, {}{})", e.name, e.game, e.port, if crate::servers::running(&e.id) { "running" } else { "stopped" }, if e.imported { ", imported" } else { "" }));
        }
        out.push(format!(
            "Settings: phone access {}, updates {}, keep servers running {}, extra game folders {}",
            if s.phone { "on" } else { "off" },
            if s.manual_updates { "manual" } else { "automatic" },
            if s.keep_servers { "on" } else { "off" },
            s.game_folders.len()
        ));
        out.push(format!("Game running: {}", crate::sys::game_running()));
        // the end of the game's console (written with -condebug), for crashes
        if let Some(g) = &game {
            for p in [g.hl.join("qconsole.log"), g.dir.join("qconsole.log")] {
                if let Ok(text) = crate::sys::read_latin1(&p) {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.to_lowercase().contains("password")).collect();
                    out.push(format!("Last game console lines ({}):", p.display()));
                    for l in &lines[lines.len().saturating_sub(40)..] {
                        out.push(format!("  {l}"));
                    }
                    break;
                }
            }
        }
        let job = crate::setup::current();
        if job.state != "idle" {
            out.push(format!("Last setup: {} ({}){}", job.kind, job.state, job.error.map(|e| format!(": {e}")).unwrap_or_default()));
            for n in job.notes.iter().take(10) {
                out.push(format!("  note: {n}"));
            }
            if !job.failed_compiles.is_empty() {
                out.push(format!("  didn't compile: {}", job.failed_compiles.join(", ")));
            }
        }
        out.join("\n")
    })
    .await
    .unwrap_or_default();
    Json(json!({"text": text}))
}

// ---- export and import a setup ------------------------------------------------------

const SETUP_FORMAT: &str = "loner-strike-setup";
/// Files the Game Mode Menu keeps that are worth moving to another computer or server.
const SETUP_EXT: &[&str] = &["cfg", "ini", "json", "txt", "html"];
/// Not these: the panel's password, live status, logs, and who joined.
const SETUP_SKIP: &[&str] = &["panel_rcon.txt", "status.json", "seen.json", "seen.log", "seen.log.reading", "friends.cfg", "navqueue.txt"];

fn exportable(rel: &Path) -> bool {
    let name = rel.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let ext = rel.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    SETUP_EXT.contains(&ext.as_str()) && !SETUP_SKIP.contains(&name.as_str()) && rel.components().count() <= 3
}

/// A relative path from an imported file, if it's safe to write.
fn safe_rel(rel: &str) -> Option<PathBuf> {
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

pub fn export_setup(g: &crate::game::Game) -> Value {
    let mut files = Map::new();
    for path in crate::archive::walk(&g.data) {
        let Ok(rel) = path.strip_prefix(&g.data) else { continue };
        if !exportable(rel) || std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(true) {
            continue;
        }
        let Ok(text) = crate::sys::read_latin1(&path) else { continue };
        let key = rel.to_string_lossy().replace('\\', "/");
        let text = if key == "friends.json" {
            // the server name travels, its password doesn't
            let name = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["name"].as_str().map(String::from)).unwrap_or_default();
            json!({"name": name}).to_string()
        } else {
            text
        };
        files.insert(key, json!(text));
    }
    let components: Vec<String> = crate::setup::record::Record::open(&g.dir).component_ids().into_iter().filter(|c| c != "foundation" && c != "core").collect();
    json!({"format": SETUP_FORMAT, "version": 1, "game": g.name, "components": components, "files": files})
}

/// Writes an exported setup into a game's data folder. Returns what isn't installed here.
pub fn import_setup(g: &crate::game::Game, v: &Value) -> Result<Vec<String>, String> {
    if v["format"] != SETUP_FORMAT {
        return Err("That isn't a Loner-Strike setup file.".into());
    }
    let files = v["files"].as_object().ok_or("The setup file is empty.")?;
    if files.len() > 500 {
        return Err("The setup file is too big.".into());
    }
    let mut written = 0;
    for (rel, text) in files {
        let (Some(rel), Some(text)) = (safe_rel(rel), text.as_str()) else { continue };
        if text.len() > 1_000_000 {
            continue;
        }
        let dest = g.data.join(&rel);
        let text = if rel == Path::new("friends.json") {
            // keep this server's own password
            let name = serde_json::from_str::<Value>(text).ok().and_then(|x| x["name"].as_str().map(String::from)).unwrap_or_default();
            let pw = std::fs::read_to_string(&dest).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|x| x["password"].as_str().map(String::from)).unwrap_or_default();
            json!({"name": name, "password": pw}).to_string()
        } else {
            text.to_string()
        };
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        crate::sys::write_latin1(&dest, &text).map_err(|e| format!("Couldn't write {} ({e}).", rel.display()))?;
        written += 1;
    }
    if written == 0 {
        return Err("Nothing in that file could be used.".into());
    }
    // admins and VIP perks take effect (users.ini and vip.cfg are written from admins.json)
    crate::admins::sync(g);
    let have = crate::setup::record::Record::open(&g.dir).component_ids();
    Ok(v["components"].as_array().into_iter().flatten().filter_map(|c| c.as_str()).filter(|c| !have.iter().any(|h| h == c)).map(String::from).collect())
}

async fn api_export(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let (Some(g), Some(app)) = (st.game(), st.app.get().cloned()) else { return err("No game found.") };
    let r = tokio::task::spawn_blocking(move || {
        let data = export_setup(&g);
        let name = format!("loner-strike-{}.json", if g.dedicated { "server" } else { g.name.as_str() });
        let Some(path) = app.dialog().file().set_title("Save your Loner-Strike setup").set_file_name(&name).add_filter("Loner-Strike setup", &["json"]).blocking_save_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        std::fs::write(&path, serde_json::to_string_pretty(&data).unwrap_or_default()).map_err(|e| format!("Couldn't save it ({e})."))?;
        Ok::<Value, String>(json!({"ok": true, "files": data["files"].as_object().map(|f| f.len()).unwrap_or(0)}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    match r {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

async fn api_import(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let (Some(g), Some(app)) = (st.game(), st.app.get().cloned()) else { return err("No game found.") };
    let r = tokio::task::spawn_blocking(move || {
        let Some(path) = app.dialog().file().set_title("Open a Loner-Strike setup").add_filter("Loner-Strike setup", &["json"]).blocking_pick_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        if std::fs::metadata(&path).map(|m| m.len() > 20_000_000).unwrap_or(true) {
            return Err("That file is too big to be a setup.".into());
        }
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|_| "That isn't a Loner-Strike setup file.".to_string())?;
        let missing = import_setup(&g, &v)?;
        Ok::<Value, String>(json!({"ok": true, "missing": missing, "hl": g.hl, "game": g.name}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    match r {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

// ---- the game's menu music ------------------------------------------------------------

/// The player's own Half-Life folders, this game's first.
fn half_lifes(st: &Shared) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    if let Some(g) = st.local_game() {
        out.push((g.hl.clone(), g.name.clone()));
    }
    for i in crate::steam::find_installs() {
        out.push((i.hl.clone(), i.games.first().cloned().unwrap_or_default()));
    }
    out
}

/// A file from the player's own game (never shipped with the app): the first of `folders`
/// (game folders under Half-Life, "" for this game's) that has `rel`.
fn game_file(st: &Shared, folders: &[&str], rel: &str) -> Option<PathBuf> {
    half_lifes(st).into_iter().find_map(|(hl, game)| {
        folders.iter().map(|g| if g.is_empty() { game.as_str() } else { g }).filter(|g| !g.is_empty()).map(|g| hl.join(g).join(rel)).find(|p| p.is_file())
    })
}

#[derive(Deserialize)]
/// The menu music: this game's, then Counter-Strike 1.6's, then Half-Life's; or with
/// track=hl / hl01 / hl02 / hl12 / hl13 / hl17, Half-Life's theme or a piece of its soundtrack (setup
/// shuffles them)
struct ThemeReq {
    /// "hl": Half-Life's theme
    #[serde(default)]
    track: String,
}

async fn api_theme(State(st): State<Shared>, axum::extract::Query(q): axum::extract::Query<ThemeReq>) -> axum::response::Response {
    use axum::response::IntoResponse;
    // setup's playlist: the two menu themes and three pieces from Half-Life's soundtrack
    let (folders, rel): (&'static [&'static str], &'static str) = match q.track.as_str() {
        "hl" => (&["valve"], "media/gamestartup.mp3"),
        "hl01" => (&["valve"], "media/Half-Life01.mp3"),
        "hl02" => (&["valve"], "media/Half-Life02.mp3"),
        "hl12" => (&["valve"], "media/Half-Life12.mp3"),
        "hl13" => (&["valve"], "media/Half-Life13.mp3"),
        "hl17" => (&["valve"], "media/Half-Life17.mp3"),
        _ => (&["", "cstrike", "valve"], "media/gamestartup.mp3"),
    };
    let file = tokio::task::spawn_blocking(move || game_file(&st, folders, rel).and_then(|p| std::fs::read(p).ok())).await.ok().flatten();
    match file {
        Some(data) if data.len() < 20_000_000 => (
            [(axum::http::header::CONTENT_TYPE, "audio/mpeg"), (axum::http::header::CACHE_CONTROL, "max-age=3600")],
            data,
        )
            .into_response(),
        _ => (StatusCode::NOT_FOUND, "no music").into_response(),
    }
}

/// The game's own menu sounds (hovering and clicking a button), for setup.
async fn api_menu_sound(State(st): State<Shared>, axum::extract::Path(name): axum::extract::Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let rel = match name.as_str() {
        "rollover" => "sound/UI/buttonrollover.wav",
        "press" => "sound/UI/buttonclick.wav",
        "click" => "sound/UI/buttonclickrelease.wav",
        _ => return (StatusCode::NOT_FOUND, "no sound").into_response(),
    };
    let file = tokio::task::spawn_blocking(move || game_file(&st, &["valve"], rel).and_then(|p| std::fs::read(p).ok())).await.ok().flatten();
    match file {
        Some(data) if data.len() < 2_000_000 => ([(axum::http::header::CONTENT_TYPE, "audio/wav"), (axum::http::header::CACHE_CONTROL, "max-age=3600")], data).into_response(),
        _ => (StatusCode::NOT_FOUND, "no sound").into_response(),
    }
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
        .route("/api/app/theme.mp3", get(api_theme))
        .route("/api/app/menu-sound/:name", get(api_menu_sound))
}

/// The app window only: these open dialogs or folders on this computer, or change who can connect.
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/app/phone/reset", post(api_phone_reset))
        .route("/api/app/storage/clear", post(api_storage_clear))
        .route("/api/app/open-folder", post(api_open_folder))
        .route("/api/app/game-folder", post(api_folder_add))
        .route("/api/app/servers-folder", get(api_servers_dir).post(api_servers_dir_set))
        .route("/api/app/game-folder/remove", post(api_folder_remove))
        .route("/api/app/export", post(api_export))
        .route("/api/app/import", post(api_import))
        .route("/api/gb/add-file", post(api_add_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_files() {
        assert!(safe_rel("global.cfg").is_some() && safe_rel("presets/fun.cfg").is_some() && safe_rel("admins.json").is_some());
        assert!(safe_rel("../x.cfg").is_none() && safe_rel("a/../../x.cfg").is_none() && safe_rel("/etc/x.cfg").is_none());
        assert!(safe_rel("panel_rcon.txt").is_none() && safe_rel("evil.exe").is_none() && safe_rel("a b.cfg").is_none());
        assert!(safe_rel("a/b/c/d.cfg").is_none());
    }
}
