//! The app's own endpoints: version and updates, and phone access (LAN + QR code).

use crate::panel_api::from_panel;
use crate::server::{self, Shared};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri_plugin_updater::UpdaterExt;

pub const PHONE_PORT: u16 = 8028;

// ---- settings kept between runs ----------------------------------------
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Settings {
    #[serde(default)]
    pub phone: bool,
    #[serde(default)]
    pub token: String,
    /// The game the panel controls: its Half-Life folder and "cstrike" or "czero"
    #[serde(default)]
    pub game_hl: Option<String>,
    #[serde(default)]
    pub game_id: Option<String>,
    /// What the panel controls: "local" (the game) or a dedicated server's id
    #[serde(default)]
    pub target: Option<String>,
    /// don't look for updates when the app starts
    #[serde(default)]
    pub manual_updates: bool,
    /// dedicated servers keep running when the app closes
    #[serde(default)]
    pub keep_servers: bool,
    /// Half-Life folders added by hand (the scan didn't find them)
    #[serde(default)]
    pub game_folders: Vec<String>,
    /// no desktop notification when someone joins your game or a server
    #[serde(default)]
    pub mute_joins: bool,
}

/// A new phone access key.
pub fn new_token() -> String {
    use rand::Rng;
    const C: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut r = rand::thread_rng();
    (0..20).map(|_| C[r.gen_range(0..C.len())] as char).collect()
}

fn settings_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Loner-Strike").join("settings.json")
}

// one change to settings.json at a time (read, change, write)
static SETTINGS: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn load_settings() -> Settings {
    let p = settings_path();
    let mut s: Settings = match crate::sys::read_text(&p) {
        Ok(Some(t)) => serde_json::from_str(&t).unwrap_or_else(|_| {
            crate::sys::set_aside(&p);
            Settings::default()
        }),
        Ok(None) => Settings::default(),
        // there but unreadable for now: use defaults this time (with a fresh phone key,
        // never an empty one), and never save them over it
        Err(_) => return Settings { token: new_token(), ..Settings::default() },
    };
    if s.token.len() < 16 {
        let _g = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        s.token = new_token();
        save_settings(&s);
    }
    s
}

/// Changes settings in one step, so two changes at once can't undo each other.
pub fn edit_settings(f: impl FnOnce(&mut Settings)) -> Settings {
    let mut s = load_settings();
    let _g = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    // read again under the lock: load_settings may have written a new token
    match crate::sys::read_text(&settings_path()) {
        Ok(Some(t)) => {
            if let Ok(fresh) = serde_json::from_str::<Settings>(&t) {
                s = fresh;
            }
        }
        Ok(None) => {}
        // couldn't read it: don't write a default over it
        Err(_) => return s,
    }
    f(&mut s);
    save_settings(&s);
    s
}

/// Remembers which game or server the panel controls.
pub fn set_local_game(hl: &str, game: &str) {
    edit_settings(|s| {
        s.game_hl = Some(hl.into());
        s.game_id = Some(game.into());
        s.target = Some("local".into());
    });
}

fn save_settings(s: &Settings) {
    let _ = crate::sys::write_atomic(&settings_path(), serde_json::to_string_pretty(s).unwrap_or_default().as_bytes());
}

// ---- phone access --------------------------------------------------------
#[derive(Default)]
pub struct Phone {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    host: Option<String>,
    error: Option<String>,
}

/// This computer's address on the local network.
fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("10.255.255.255:1").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

pub fn start_phone(st: &Shared) {
    let mut phone = st.phone.lock().unwrap();
    if phone.stop.is_some() {
        return;
    }
    phone.error = None;
    let Some(ip) = lan_ip() else {
        phone.error = Some("This computer isn't on a network right now.".into());
        return;
    };
    let listener = match std::net::TcpListener::bind(("0.0.0.0", PHONE_PORT)) {
        Ok(l) => l,
        Err(_) => {
            phone.error = Some(format!("Port {PHONE_PORT} is busy. Close the old phone panel if it's running, then try again."));
            return;
        }
    };
    let host = format!("{ip}:{PHONE_PORT}");
    st.lan_hosts.write().unwrap().push(host.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let state = st.clone();
    tauri::async_runtime::spawn(async move {
        listener.set_nonblocking(true).ok();
        if let Ok(l) = tokio::net::TcpListener::from_std(listener) {
            let _ = axum::serve(l, server::router(state, server::Listener::Lan))
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        }
    });
    phone.stop = Some(tx);
    phone.host = Some(host);
}

fn stop_phone(st: &Shared) {
    let mut phone = st.phone.lock().unwrap();
    if let Some(tx) = phone.stop.take() {
        let _ = tx.send(());
    }
    if let Some(h) = phone.host.take() {
        st.lan_hosts.write().unwrap().retain(|x| *x != h);
    }
}

fn qr_svg(text: &str) -> String {
    use qrcode::render::svg;
    qrcode::QrCode::new(text.as_bytes())
        .map(|c| c.render::<svg::Color>().min_dimensions(220, 220).quiet_zone(true).build())
        .unwrap_or_default()
}

pub fn phone_status(st: &Shared) -> Value {
    let phone = st.phone.lock().unwrap();
    match &phone.host {
        Some(h) => {
            let url = format!("http://{h}/?t={}", st.token.read().unwrap());
            json!({"enabled": true, "address": format!("http://{h}"), "url": url, "qr": qr_svg(&url)})
        }
        None => json!({"enabled": false, "error": phone.error}),
    }
}

async fn api_phone(State(st): State<Shared>) -> Json<Value> {
    Json(phone_status(&st))
}

#[derive(Deserialize)]
struct PhoneReq {
    enabled: bool,
}

async fn api_phone_set(State(st): State<Shared>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, Json(req): Json<PhoneReq>) -> (StatusCode, Json<Value>) {
    // a phone switching phone access off would lock itself out: the computer decides
    if on.is_none() {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Do that from the computer running Loner-Strike."})));
    }
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    if req.enabled {
        start_phone(&st);
    } else {
        stop_phone(&st);
    }
    edit_settings(|s| s.phone = req.enabled);
    (StatusCode::OK, Json(phone_status(&st)))
}

// ---- version and updates ---------------------------------------------------
static PENDING: Mutex<Option<tauri_plugin_updater::Update>> = Mutex::new(None);

async fn api_app(State(st): State<Shared>) -> Json<Value> {
    let version = st.app.get().map(|a| a.package_info().version.to_string()).unwrap_or_default();
    let pending = PENDING.lock().unwrap().as_ref().map(|u| json!({"version": u.version, "notes": u.body}));
    let res = crate::setup_api::resources_for(&st).ok();
    let game = match st.game() {
        Some(g) => tokio::task::spawn_blocking(move || {
            let rec = crate::setup::record::Record::open(&g.dir);
            // parts whose files changed in an app update (hashing is cached after the first time)
            let outdated = res.as_ref().map(|r| crate::setup::outdated(r, &rec)).unwrap_or_default();
            // only once Loner-Strike is set up here: before that, setup shows what it'll replace
            let theirs = match (&res, rec.data.components.contains_key("core")) {
                (Some(r), true) => crate::setup::unmanaged(r, &g.dir, &rec),
                _ => vec![],
            };
            json!({"id": g.name, "dir": g.dir, "hl": g.hl, "dedicated": g.dedicated, "port": g.port, "installed": rec.component_ids(), "outdated": outdated, "unmanaged": theirs})
        })
        .await
        .ok(),
        None => None,
    };
    Json(json!({"version": version, "update": pending, "game": game,
                "bots": saved_bots(&st), "lastmap": last_map(&st)}))
}

async fn api_update_check(State(st): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(app) = st.app.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "not ready"})));
    };
    let found = match app.updater() {
        Ok(u) => u.check().await,
        Err(e) => return (StatusCode::OK, Json(json!({"error": e.to_string()}))),
    };
    match found {
        Ok(Some(update)) => {
            let v = json!({"available": true, "version": update.version, "notes": update.body});
            *PENDING.lock().unwrap() = Some(update);
            (StatusCode::OK, Json(v))
        }
        Ok(None) => (StatusCode::OK, Json(json!({"available": false}))),
        // no published release yet answers 404, which is the same as "nothing new"
        Err(e) if e.to_string().contains("valid release JSON") => (StatusCode::OK, Json(json!({"available": false}))),
        Err(_) => (StatusCode::OK, Json(json!({"error": "Couldn't check for updates. Check your internet connection."}))),
    }
}

async fn api_update_install(State(st): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let update = PENDING.lock().unwrap().take();
    let (Some(update), Some(app)) = (update, st.app.get().cloned()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Check for updates first."})));
    };
    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(()) => {
            // answer first, then restart into the new version
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                app.restart();
            });
            (StatusCode::OK, Json(json!({"ok": true})))
        }
        Err(e) => (StatusCode::OK, Json(json!({"error": format!("The update didn't install ({e}).")}))),
    }
}

// ---- LAN parties -------------------------------------------------------------------
/// This computer's address on the local network, for friends in the same room.
async fn api_lan(State(st): State<Shared>) -> Json<Value> {
    Json(json!({"ip": lan_ip(), "port": game_port(&st)}))
}

fn game_port(st: &Shared) -> u16 {
    st.game().map(|g| g.port).unwrap_or(crate::connect::GAME_PORT)
}

/// Direct (UPnP): GET the status, POST {on} to open or close the router port.
async fn api_direct() -> Json<Value> {
    Json(crate::connect::direct_status())
}

#[derive(Deserialize)]
struct DirectReq {
    on: bool,
}

async fn api_direct_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<DirectReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let port = game_port(&st);
    let v = tokio::task::spawn_blocking(move || if req.on { crate::connect::open_direct(port) } else { crate::connect::close_direct() })
        .await
        .unwrap_or_else(|_| json!({"on": false, "error": "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening."}));
    (StatusCode::OK, Json(v))
}

/// This computer's address on ZeroTier, Radmin VPN or Hamachi, when they're running.
async fn api_vlans(State(st): State<Shared>) -> Json<Value> {
    let port = game_port(&st);
    Json(tokio::task::spawn_blocking(move || crate::connect::virtual_lans(port)).await.unwrap_or_else(|_| json!({})))
}

// ---- choosing the game ---------------------------------------------------------
/// Which game the panel controls: the one picked last, else one Loner-Strike set up
/// (CS 1.6 first), else the first one found.
pub fn pick_game(settings: &Settings) -> Option<crate::game::Game> {
    let installs = crate::steam::find_installs();
    if let (Some(hl), Some(id)) = (&settings.game_hl, &settings.game_id) {
        if let Some(i) = installs.iter().find(|i| i.hl.to_string_lossy() == *hl && i.games.contains(id)) {
            return Some(crate::game::Game::new(&i.hl, id));
        }
    }
    let set_up = |i: &crate::steam::Install, g: &str| i.hl.join(g).join("addons/loner-strike/record.json").is_file();
    for g in ["cstrike", "czero"] {
        if let Some(i) = installs.iter().find(|i| i.games.iter().any(|x| x == g) && set_up(i, g)) {
            return Some(crate::game::Game::new(&i.hl, g));
        }
    }
    installs.into_iter().find_map(|i| {
        let g = if i.games.iter().any(|x| x == "cstrike") { "cstrike".to_string() } else { i.games.first()?.clone() };
        Some(crate::game::Game::new(&i.hl, &g))
    })
}

#[derive(Deserialize)]
struct UseGame {
    hl: String,
    game: String,
}

/// Switches the panel to another game (remembered), then restarts the app to load it.
async fn api_use_game(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<UseGame>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    // finding installs and switching sharing over can take seconds
    let ok = tokio::task::spawn_blocking(move || {
        let ok = crate::steam::find_installs().iter().any(|i| i.hl.to_string_lossy() == req.hl && i.games.contains(&req.game));
        if ok {
            crate::servers::use_local(&st, &req.hl, &req.game);
        }
        ok
    })
    .await
    .unwrap_or(false);
    if !ok {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "That game isn't on this computer."})));
    }
    (StatusCode::OK, Json(json!({"ok": true})))
}

async fn api_restart(State(st): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    restart_soon(&st);
    (StatusCode::OK, Json(json!({"ok": true})))
}

fn restart_soon(st: &Shared) {
    if let Some(app) = st.app.get().cloned() {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            app.restart();
        });
    }
}

// ---- launching the game -----------------------------------------------------
fn data_file(st: &Shared, name: &str) -> Option<String> {
    let g = st.game()?;
    std::fs::read_to_string(g.dir.join("addons/amxmodx/data/gamemodes").join(name)).ok()
}

/// The bot count saved by the Game Mode Menu (global.cfg: bot_quota "14").
fn saved_bots(st: &Shared) -> Option<u32> {
    data_file(st, "global.cfg")?
        .lines()
        .find_map(|l| l.trim().strip_prefix("bot_quota").map(|v| v.trim().trim_matches('"').parse().ok()))
        .flatten()
}

/// The last map played (only plain map names; it goes on a command line).
fn last_map(st: &Shared) -> Option<String> {
    let m = data_file(st, "lastmap.txt")?.trim().to_string();
    (!m.is_empty() && m.len() <= 31 && m.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))).then_some(m)
}

#[derive(Deserialize, Default)]
struct LaunchReq {
    /// start a server on this map (or the last one) instead of the main menu
    #[serde(default)]
    map: Option<String>,
    #[serde(default)]
    menu: bool,
    /// how many bots to start with (saved, like the Bots page does)
    #[serde(default)]
    bots: Option<u32>,
    /// the mode to start in, and a few of its settings (saved the way the menu saves them)
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    settings: std::collections::HashMap<String, String>,
}

/// The settings the Start a game screen offers: cvar, saved for every mode (global.cfg)
/// or per mode (mode_<id>.cfg), and the values the in-game menu offers.
const START_SETTINGS: &[(&str, bool, &[&str])] = &[
    ("bot_difficulty", true, &["0", "1", "2", "3"]),
    ("gm_botstyle", true, &["0", "1", "2", "3", "4", "5", "6", "7", "8"]),
    ("mp_timelimit", false, &["0", "10", "20", "30", "45", "60"]),
    ("mp_friendlyfire", false, &["0", "1"]),
];

/// gm_botstyle's personality files, by its value (as gamemodes.sma lists them).
const BOT_STYLE_FILES: &[&str] = &["", "BotProfile_Pub.db", "BotProfile_Matchmaking.db", "BotProfile_Veterans.db", "BotProfile_Newbie.db", "BotProfile_LockedIn.db", "BotProfile_Hackers.db", "BotProfile_Gods.db", "BotProfile_Pub.db"];

/// `cvar "value"` lines from a config file.
fn cfg_values(path: &std::path::Path) -> Vec<(String, String)> {
    crate::sys::read_latin1(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let (k, v) = l.split_once(char::is_whitespace)?;
            Some((k.to_lowercase(), v.trim().trim_matches('"').to_string()))
        })
        .collect()
}

/// Sets `cvar "value"` lines in a config file, keeping the rest.
fn set_cfg_values(path: &std::path::Path, values: &[(String, String)]) -> std::io::Result<()> {
    let old = crate::sys::read_latin1(path).unwrap_or_default();
    let mut lines: Vec<String> = old
        .lines()
        .filter(|l| !values.iter().any(|(k, _)| l.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case(k))))
        .map(String::from)
        .collect();
    lines.extend(values.iter().map(|(k, v)| format!("{k} \"{v}\"")));
    crate::sys::write_atomic(path, (lines.join("\n") + "\n").as_bytes())
}

/// What a setting is when the mode starts: the same order the plugin applies them
/// (base.cfg, the mode's cfg, saved global settings, saved mode settings).
fn effective(g: &crate::game::Game, mode: &str, cvar: &str) -> Option<String> {
    let cfg = g.dir.join("addons/amxmodx/configs/gamemodes");
    [cfg.join("base.cfg"), cfg.join(format!("{mode}.cfg")), g.data.join("global.cfg"), g.data.join(format!("mode_{mode}.cfg"))]
        .iter()
        .flat_map(|p| cfg_values(p))
        .filter(|(k, _)| k == cvar)
        .last()
        .map(|(_, v)| v)
}

fn current_mode(g: &crate::game::Game) -> String {
    std::fs::read_to_string(g.data.join("current.txt")).ok().map(|t| t.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "classic".into())
}

/// Modes and their quick settings, for starting the game.
async fn api_start_options(State(st): State<Shared>) -> Json<Value> {
    let Some(g) = st.game() else { return Json(json!({"modes": []})) };
    let v = tokio::task::spawn_blocking(move || {
        let modes: Vec<Value> = crate::extras::parse_modes(&crate::sys::read_latin1(&g.dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default())
            .into_iter()
            .map(|(id, name)| {
                let settings: serde_json::Map<String, Value> = START_SETTINGS.iter().map(|(c, _, _)| (c.to_string(), json!(effective(&g, &id, c)))).collect();
                json!({"id": id, "name": name, "settings": settings})
            })
            .collect();
        json!({"modes": modes, "mode": current_mode(&g)})
    })
    .await
    .unwrap_or_else(|_| json!({"modes": []}));
    Json(v)
}

/// Switches the mode before the game starts, the way the plugin does it between maps:
/// the mode's plugin list becomes plugins-gamemode.ini, and current.txt names it.
fn start_in(g: &crate::game::Game, mode: &str, settings: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let cfg = g.dir.join("addons/amxmodx/configs");
    let known = crate::extras::parse_modes(&crate::sys::read_latin1(&cfg.join("gamemodes/modes.ini")).unwrap_or_default());
    if !known.iter().any(|(id, _)| id == mode) {
        return Err("That mode isn't installed.".into());
    }
    let mut global = Vec::new();
    let mut own = Vec::new();
    for (k, v) in settings {
        let Some((_, is_global, allowed)) = START_SETTINGS.iter().find(|(c, _, _)| c == k) else { continue };
        if !allowed.contains(&v.as_str()) {
            return Err(format!("{v} isn't a value for {k}."));
        }
        if *is_global { global.push((k.clone(), v.clone())) } else { own.push((k.clone(), v.clone())) };
    }
    if current_mode(g) != mode {
        let list = std::fs::read(cfg.join(format!("gamemodes/{mode}.plugins.ini"))).unwrap_or_default();
        let mut text = format!("; Generated by gamemodes.amxx - edit configs/gamemodes/{mode}.plugins.ini instead\n").into_bytes();
        text.extend(list);
        crate::sys::write_atomic(&cfg.join("plugins-gamemode.ini"), &text).map_err(|e| format!("Couldn't switch the mode ({e})."))?;
        crate::sys::write_atomic(&g.data.join("current.txt"), format!("{mode}\n").as_bytes()).map_err(|e| format!("Couldn't switch the mode ({e})."))?;
    }
    let save = |p: std::path::PathBuf, v: &[(String, String)]| if v.is_empty() { Ok(()) } else { set_cfg_values(&p, v).map_err(|e| format!("Couldn't save the settings ({e}).")) };
    save(g.data.join("global.cfg"), &global)?;
    save(g.data.join(format!("mode_{mode}.cfg")), &own)
}

/// Saves the bot count where the Game Mode Menu keeps it (global.cfg: bot_quota "9").
fn save_bots(dir: &std::path::Path, bots: u32) -> std::io::Result<()> {
    let data = dir.join("addons/amxmodx/data/gamemodes");
    std::fs::create_dir_all(&data)?;
    let path = data.join("global.cfg");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = old.lines().filter(|l| !l.trim_start().starts_with("bot_quota")).map(String::from).collect();
    lines.push(format!("bot_quota \"{}\"", bots.min(31)));
    std::fs::write(path, lines.join("\n") + "\n")
}

async fn api_launch(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<LaunchReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(g) = st.game() else {
        return (StatusCode::OK, Json(json!({"error": "Couldn't find the game."})));
    };
    if g.dedicated {
        return (StatusCode::OK, Json(json!({"error": "Start dedicated servers from the Servers tab."})));
    }
    if let Some(n) = req.bots {
        if let Err(e) = save_bots(&g.dir, n) {
            return (StatusCode::OK, Json(json!({"error": format!("Couldn't save the bot count ({e}).")})));
        }
    }
    if let Some(m) = req.mode.as_deref().filter(|m| !m.is_empty()) {
        if let Err(e) = start_in(&g, m, &req.settings) {
            return (StatusCode::OK, Json(json!({"error": e})));
        }
    }
    let appid = if g.name == "cstrike" { "10" } else { "80" };
    let map = req
        .map
        .filter(|m| m.len() <= 31 && m.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)))
        .or_else(|| last_map(&st));
    // the game's console goes to qconsole.log, so a crash can be looked into afterwards
    let mut game_args: Vec<String> = vec!["-condebug".into()];
    if let (false, Some(m)) = (req.menu, map) {
        // a bot style's personalities are read when the map loads, so they go in first
        // (the plugin only switches them from the next map)
        let style = req.settings.get("gm_botstyle").cloned().or_else(|| effective(&g, &current_mode(&g), "gm_botstyle")).and_then(|s| s.parse::<usize>().ok()).and_then(|i| BOT_STYLE_FILES.get(i)).filter(|f| !f.is_empty() && g.dir.join(f).is_file());
        if let Some(f) = style {
            game_args.extend(["+bot_profile_db".into(), f.to_string()]);
        }
        game_args.extend(["+maxplayers".into(), "32".into(), "+map".into(), m]);
    }
    match crate::steam::launch(&g.hl, appid, &game_args) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

/// The app window only: updating or restarting the app and switching its game.
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/app/update/install", post(api_update_install))
        .route("/api/app/game", post(api_use_game))
        .route("/api/app/restart", post(api_restart))
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/app", get(api_app))
        .route("/api/app/update/check", post(api_update_check))
        .route("/api/app/phone", get(api_phone).post(api_phone_set))
        .route("/api/app/launch", post(api_launch))
        .route("/api/app/start-options", get(api_start_options))
        .route("/api/app/lan", get(api_lan))
        .route("/api/app/direct", get(api_direct).post(api_direct_set))
        .route("/api/app/vlans", get(api_vlans))
}

#[cfg(test)]
mod start_tests {
    use super::*;

    #[test]
    fn cfg_lines_replaced() {
        let dir = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "start-").unwrap();
        let p = dir.join("global.cfg");
        std::fs::write(&p, "bot_quota \"9\"\nBOT_DIFFICULTY \"1\"\n").unwrap();
        set_cfg_values(&p, &[("bot_difficulty".into(), "3".into())]).unwrap();
        let v = cfg_values(&p);
        assert!(v.contains(&("bot_quota".into(), "9".into())));
        assert_eq!(v.iter().filter(|(k, _)| k == "bot_difficulty").collect::<Vec<_>>(), vec![&("bot_difficulty".to_string(), "3".to_string())]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
