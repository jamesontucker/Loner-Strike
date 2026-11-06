//! The local web server behind the app window, and behind a phone on the same Wi-Fi.
//!
//! - 127.0.0.1:<port> serves the app window: no password, but only requests addressed
//!   to our own host name get in (a web page can point its domain at 127.0.0.1).
//! - With phone access on, a second listener on the LAN needs the access token
//!   (from the QR code), after which a cookie keeps the phone signed in.
//!
//! Routes: panel_api (the game: state, commands, maps...) and app_api (updates, phone).

use crate::game::Game;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use rust_embed::RustEmbed;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// The built React UI (app/ui/dist), compiled into the app.
#[derive(RustEmbed)]
#[folder = "../ui/dist/"]
struct Ui;

pub struct AppState {
    /// The game or dedicated server the panel controls right now (see servers.rs)
    pub current: RwLock<Option<Arc<Game>>>,
    /// Host names the app window's listener answers to (127.0.0.1 / localhost).
    pub hosts: RwLock<Vec<String>>,
    /// Host names the phone listener answers to (this computer's LAN address).
    pub lan_hosts: RwLock<Vec<String>>,
    /// Phone access token, kept between runs so a saved QR keeps working.
    pub token: String,
    pub app: OnceLock<tauri::AppHandle>,
    pub phone: Mutex<crate::app_api::Phone>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn game(&self) -> Option<Arc<Game>> {
        self.current.read().unwrap().clone()
    }

    /// Switches the panel to another game or server.
    pub fn set_game(&self, g: Option<Arc<Game>>) {
        if let Some(g) = &g {
            g.sharing.retarget(g.rcon.clone(), &g.data, g.port);
            // admins/VIPs whose time ran out leave users.ini
            crate::admins::sync(g);
        }
        *self.current.write().unwrap() = g;
    }
}

/// The app window's listener (127.0.0.1): only requests addressed to our own local
/// host names get in (a web page can point its own domain at 127.0.0.1).
async fn guard_local(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let host = host_of(&req);
    if !st.hosts.read().unwrap().contains(&host) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    next.run(req).await
}

/// The phone listener (LAN): always needs the token, whatever the request says
/// about itself, and only answers to the phone address.
async fn guard_lan(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let host = host_of(&req);
    if !st.lan_hosts.read().unwrap().contains(&host) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    let cookie_ok = req
        .headers()
        .get(header::COOKIE)
        .and_then(|c| c.to_str().ok())
        .map(|c| c.split(';').any(|p| p.trim() == format!("ls_token={}", st.token)))
        .unwrap_or(false);
    let query_ok = req.uri().query().map(|q| q.split('&').any(|p| p == format!("t={}", st.token))).unwrap_or(false);
    if !cookie_ok && !query_ok {
        return (StatusCode::FORBIDDEN, "Scan the QR code in Loner-Strike's Settings to connect.").into_response();
    }
    let mut res = next.run(req).await;
    if query_ok {
        let c = format!("ls_token={}; Path=/; SameSite=Strict; Max-Age=31536000", st.token);
        res.headers_mut().insert(header::SET_COOKIE, c.parse().unwrap());
    }
    res
}

fn host_of(req: &Request) -> String {
    req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_lowercase()
}

/// Serves the UI files; unknown paths get index.html (the app routes on the client).
async fn ui(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file = if path.is_empty() { "index.html" } else { path };
    match Ui::get(file).or_else(|| Ui::get("index.html")) {
        Some(f) => {
            let mime = f.metadata.mimetype().to_string();
            // only Vite's hashed build files never change; everything else (logo, icons) may
            let cache = if file.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
            ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache.to_string())], f.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "The app's pages are missing - rebuild the UI (npm run build in app/ui).").into_response(),
    }
}

/// Which listener a router is for decides its guard (never the request's own headers).
#[derive(Clone, Copy, PartialEq)]
pub enum Listener {
    Local,
    Lan,
}

pub fn router(state: Shared, listener: Listener) -> Router {
    let routes = Router::new()
        .merge(crate::panel_api::routes())
        .merge(crate::app_api::routes())
        .merge(crate::admins::routes())
        .merge(crate::setup_api::routes())
        .merge(crate::servers::routes())
        .merge(crate::welcome::routes())
        .fallback(ui);
    let routes = match listener {
        Listener::Local => routes.merge(crate::servers::local_routes()).layer(middleware::from_fn_with_state(state.clone(), guard_local)),
        Listener::Lan => routes.layer(middleware::from_fn_with_state(state.clone(), guard_lan)),
    };
    routes.with_state(state)
}

pub async fn serve(state: Shared, listener: std::net::TcpListener) {
    listener.set_nonblocking(true).ok();
    let listener = tokio::net::TcpListener::from_std(listener).expect("listener");
    axum::serve(listener, router(state, Listener::Local)).await.ok();
}
