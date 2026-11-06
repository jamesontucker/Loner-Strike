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
}

fn settings_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Loner-Strike").join("settings.json")
}

pub fn load_settings() -> Settings {
    let mut s: Settings = std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if s.token.len() < 16 {
        use rand::Rng;
        const C: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
        let mut r = rand::thread_rng();
        s.token = (0..20).map(|_| C[r.gen_range(0..C.len())] as char).collect();
        save_settings(&s);
    }
    s
}

fn save_settings(s: &Settings) {
    let p = settings_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(s).unwrap_or_default());
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
            phone.error = Some(format!("Port {PHONE_PORT} is busy (is the old panel running for phones?)."));
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

fn phone_status(st: &Shared) -> Value {
    let phone = st.phone.lock().unwrap();
    match &phone.host {
        Some(h) => {
            let url = format!("http://{h}/?t={}", st.token);
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

async fn api_phone_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<PhoneReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    if req.enabled {
        start_phone(&st);
    } else {
        stop_phone(&st);
    }
    let mut s = load_settings();
    s.phone = req.enabled;
    save_settings(&s);
    (StatusCode::OK, Json(phone_status(&st)))
}

// ---- version and updates ---------------------------------------------------
static PENDING: Mutex<Option<tauri_plugin_updater::Update>> = Mutex::new(None);

async fn api_app(State(st): State<Shared>) -> Json<Value> {
    let version = st.app.get().map(|a| a.package_info().version.to_string()).unwrap_or_default();
    let pending = PENDING.lock().unwrap().as_ref().map(|u| json!({"version": u.version, "notes": u.body}));
    let game = st.game.as_ref().map(|g| json!({"id": g.name, "dir": g.dir}));
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
async fn api_lan() -> Json<Value> {
    Json(json!({"ip": lan_ip(), "port": 27015}))
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
    let ok = crate::steam::find_installs().iter().any(|i| i.hl.to_string_lossy() == req.hl && i.games.contains(&req.game));
    if !ok {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "That game isn't on this computer."})));
    }
    let mut s = load_settings();
    s.game_hl = Some(req.hl);
    s.game_id = Some(req.game);
    save_settings(&s);
    restart_soon(&st);
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
    let g = st.game.as_ref()?;
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
}

async fn api_launch(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<LaunchReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(g) = st.game.as_ref() else {
        return (StatusCode::OK, Json(json!({"error": "Couldn't find the game."})));
    };
    let appid = if g.name == "cstrike" { "10" } else { "80" };
    let map = req
        .map
        .filter(|m| m.len() <= 31 && m.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)))
        .or_else(|| last_map(&st));
    let mut game_args: Vec<String> = Vec::new();
    if let (false, Some(m)) = (req.menu, map) {
        game_args = vec!["+maxplayers".into(), "32".into(), "+map".into(), m];
    }
    match crate::steam::launch(&g.hl, appid, &game_args) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/app", get(api_app))
        .route("/api/app/update/check", post(api_update_check))
        .route("/api/app/update/install", post(api_update_install))
        .route("/api/app/phone", get(api_phone).post(api_phone_set))
        .route("/api/app/launch", post(api_launch))
        .route("/api/app/game", post(api_use_game))
        .route("/api/app/restart", post(api_restart))
        .route("/api/app/lan", get(api_lan))
}
