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
    st.hosts.write().unwrap().push(host.clone());
    st.lan_hosts.write().unwrap().push(host.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let state = st.clone();
    tauri::async_runtime::spawn(async move {
        listener.set_nonblocking(true).ok();
        if let Ok(l) = tokio::net::TcpListener::from_std(listener) {
            let _ = axum::serve(l, server::router(state))
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
        st.hosts.write().unwrap().retain(|x| *x != h);
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
    Json(json!({"version": version, "update": pending, "game": game}))
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
        Err(e) => (StatusCode::OK, Json(json!({"error": format!("Couldn't check for updates ({e}).")}))),
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

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/app", get(api_app))
        .route("/api/app/update/check", post(api_update_check))
        .route("/api/app/update/install", post(api_update_install))
        .route("/api/app/phone", get(api_phone).post(api_phone_set))
}
