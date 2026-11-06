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

tokio::task_local! {
    /// The dedicated server a request is about (X-Target header or ?target=), if any.
    static TARGET: Option<Arc<Game>>;
}

pub struct AppState {
    /// The player's own game (CS 1.6 or Condition Zero). Requests for a dedicated
    /// server name it with X-Target instead (see with_target).
    pub current: RwLock<Option<Arc<Game>>>,
    /// Host names the app window's listener answers to (127.0.0.1 / localhost).
    pub hosts: RwLock<Vec<String>>,
    /// Host names the phone listener answers to (this computer's LAN address).
    pub lan_hosts: RwLock<Vec<String>>,
    /// Phone access token, kept between runs so a saved QR keeps working.
    pub token: RwLock<String>,
    pub app: OnceLock<tauri::AppHandle>,
    pub phone: Mutex<crate::app_api::Phone>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    /// The game this request is about: the server it names, else the player's own game.
    /// (Read it before spawn_blocking: the request's target doesn't follow into other threads.)
    pub fn game(&self) -> Option<Arc<Game>> {
        TARGET.try_with(|t| t.clone()).ok().flatten().or_else(|| self.local_game())
    }

    /// The player's own game, whatever the request is about.
    pub fn local_game(&self) -> Option<Arc<Game>> {
        self.current.read().unwrap().clone()
    }

    /// Switches the player's own game (CS 1.6 / Condition Zero).
    pub fn set_game(&self, g: Option<Arc<Game>>) {
        if let Some(g) = &g {
            g.sharing.retarget(&g.data, g.port);
            // admins/VIPs whose time ran out leave users.ini
            crate::admins::sync(g);
        }
        *self.current.write().unwrap() = g;
    }
}

/// The app window's listener (127.0.0.1): only requests addressed to our own local
/// host names get in (a web page can point its own domain at 127.0.0.1).
async fn guard_local(State(st): State<Shared>, mut req: Request, next: Next) -> Response {
    let host = host_of(&req);
    if !st.hosts.read().unwrap().contains(&host) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    // handlers shared with the phone listener can tell the app window apart
    req.extensions_mut().insert(OnComputer);
    next.run(req).await
}

/// Set on requests from the app window (the local listener), never on the phone's.
#[derive(Clone, Copy)]
pub struct OnComputer;

/// The phone listener (LAN): always needs the token, whatever the request says
/// about itself, and only answers to the phone address.
async fn guard_lan(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let host = host_of(&req);
    if !st.lan_hosts.read().unwrap().contains(&host) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    // never open without a real key (an empty one would match "ls_token=")
    let token = st.token.read().unwrap().clone();
    if token.len() < 16 {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    let cookie_ok = req
        .headers()
        .get(header::COOKIE)
        .and_then(|c| c.to_str().ok())
        .map(|c| c.split(';').any(|p| p.trim() == format!("ls_token={}", st.token.read().unwrap())))
        .unwrap_or(false);
    let query_ok = req.uri().query().map(|q| q.split('&').any(|p| p == format!("t={}", st.token.read().unwrap()))).unwrap_or(false);
    if !cookie_ok && !query_ok {
        return (StatusCode::FORBIDDEN, "Scan the QR code in Loner-Strike's Settings to connect.").into_response();
    }
    let mut res = next.run(req).await;
    if query_ok {
        let c = format!("ls_token={}; Path=/; SameSite=Strict; Max-Age=31536000", st.token.read().unwrap());
        res.headers_mut().insert(header::SET_COOKIE, c.parse().unwrap());
    }
    res
}

fn host_of(req: &Request) -> String {
    req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_lowercase()
}

/// A request naming a dedicated server (X-Target: <id>, or ?target=<id> for images)
/// runs with that server as its game.
async fn with_target(req: Request, next: Next) -> Response {
    let header = req.headers().get("x-target").and_then(|v| v.to_str().ok()).map(String::from);
    let query = req.uri().query().and_then(|q| q.split('&').find_map(|p| p.strip_prefix("target=").map(String::from)));
    // the server list and per-server routes name their server in the path: they never
    // depend on (or fail because of) the one that's open
    let path = req.uri().path();
    let by_path = path == "/api/servers" || path.starts_with("/api/servers/");
    let post = req.method() == axum::http::Method::POST;
    // a request that edits a list shared between servers passes the edit on
    let list = if post { crate::lists::list_for(path) } else { None };
    let Some(id) = header.or(query).filter(|t| !t.is_empty() && t != "local" && !by_path) else {
        let res = next.run(req).await;
        // the player's own game: a list it shares with servers goes to them
        if let Some(list) = list.filter(|_| res.status().is_success()) {
            tokio::task::spawn_blocking(move || crate::lists::after_change("local", list));
        }
        return res;
    };
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || id.len() > 48 {
        return (StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error": "Unknown server."}))).into_response();
    }
    let id2 = id.clone();
    let writes_files = path != "/api/cmd";
    let game = tokio::task::spawn_blocking(move || crate::servers::game_for(&id, || None)).await.ok().flatten();
    match game {
        Some(g) => {
            let remote = g.remote.is_some();
            let res = TARGET.scope(Some(g), next.run(req)).await;
            // a server on another machine: a change made here goes up to it
            if post && res.status().is_success() {
                tokio::task::spawn_blocking(move || {
                    // a list it shares goes to the others first, then a server on another
                    // machine gets the change uploaded
                    if let Some(list) = list {
                        crate::lists::after_change(&id2, list);
                    }
                    // (plain game commands only talk to the game: nothing to send)
                    if remote && writes_files {
                        crate::servers::sync_settings(&id2);
                        crate::hltv::refresh_remote(&id2);
                    }
                });
            }
            res
        }
        None => (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"online": false, "error": "That server isn't there any more, or isn't set up yet.", "reason": "That server isn't there any more."}))).into_response(),
    }
}

/// Serves the UI files; unknown paths get index.html (the app routes on the client).
/// Text files go gzipped to a browser that takes it (a phone on Wi-Fi gets the bundle
/// at a fraction of its size).
async fn ui(uri: Uri, headers: axum::http::HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file = if path.is_empty() { "index.html" } else { path };
    let (file, f) = match Ui::get(file) {
        Some(f) => (file, f),
        // an old build's file (a page left open across an update): say so, so the page
        // can reload, instead of answering with the HTML page
        None if file.starts_with("assets/") => return (StatusCode::NOT_FOUND, "not found").into_response(),
        None => match Ui::get("index.html") {
            Some(f) => ("index.html", f),
            None => return (StatusCode::NOT_FOUND, "The app's pages are missing - rebuild the UI (npm run build in app/ui).").into_response(),
        },
    };
    let mime = f.metadata.mimetype().to_string();
    // only Vite's hashed build files never change; everything else (logo, icons) may
    let cache = if file.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    let gzip_ok = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|e| e.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case("gzip")));
    let vary = (header::VARY, "accept-encoding".to_string());
    if gzip_ok && compressible(&mime) {
        if let Some(gz) = gzipped(file, &f.data) {
            return ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache.to_string()), (header::CONTENT_ENCODING, "gzip".to_string()), vary], gz).into_response();
        }
    }
    ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache.to_string()), vary], f.data).into_response()
}

/// Text worth compressing (pictures and fonts already are).
fn compressible(mime: &str) -> bool {
    mime.starts_with("text/") || mime.contains("javascript") || mime.contains("json") || mime.contains("svg") || mime.contains("xml")
}

/// An embedded file gzipped, made once on first ask (the files never change while the
/// app runs). None if it doesn't get smaller.
fn gzipped(file: &str, data: &[u8]) -> Option<axum::body::Bytes> {
    use std::collections::HashMap;
    use std::io::Write;
    static GZ: Mutex<Option<HashMap<String, Option<axum::body::Bytes>>>> = Mutex::new(None);
    if let Some(v) = GZ.lock().unwrap().as_ref().and_then(|m| m.get(file)) {
        return v.clone();
    }
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let gz = enc.write_all(data).and_then(|_| enc.finish()).ok().filter(|g| g.len() + 64 < data.len()).map(axum::body::Bytes::from);
    GZ.lock().unwrap().get_or_insert_with(HashMap::new).insert(file.to_string(), gz.clone());
    gz
}

/// Which listener a router is for decides its guard (never the request's own headers).
#[derive(Clone, Copy, PartialEq)]
pub enum Listener {
    Local,
    Lan,
}

pub fn router(state: Shared, listener: Listener) -> Router {
    crate::lists::init(&state);
    let routes = Router::new()
        .merge(crate::panel_api::routes())
        .merge(crate::app_api::routes())
        .merge(crate::lists::routes())
        .merge(crate::admins::routes())
        .merge(crate::setup_api::routes())
        .merge(crate::servers::routes())
        .merge(crate::hltv::routes())
        .merge(crate::welcome::routes())
        .merge(crate::extras::routes())
        .merge(crate::scrim::routes())
        .merge(crate::community::routes())
        .merge(crate::prefs::routes())
        .merge(crate::plugins::routes())
        .merge(crate::sounds::routes())
        .fallback(ui);
    // X-Target is applied after the computer-only routes are merged (a layer only covers
    // routes already there), and the listener's guard stays outermost
    let routes = match listener {
        Listener::Local => routes
            .merge(crate::servers::local_routes())
            .merge(crate::hltv::local_routes())
            .merge(crate::prefs::local_routes())
            .merge(crate::plugins::local_routes())
            .merge(crate::sounds::local_routes())
            .merge(crate::extras::local_routes())
            .layer(middleware::from_fn(with_target))
            .layer(middleware::from_fn_with_state(state.clone(), guard_local)),
        Listener::Lan => routes.layer(middleware::from_fn(with_target)).layer(middleware::from_fn_with_state(state.clone(), guard_lan)),
    };
    routes.with_state(state)
}

pub async fn serve(state: Shared, listener: std::net::TcpListener) {
    listener.set_nonblocking(true).ok();
    let listener = tokio::net::TcpListener::from_std(listener).expect("listener");
    axum::serve(listener, router(state, Listener::Local)).await.ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_once() {
        let text = "const x = 1;\n".repeat(500);
        let gz = gzipped("test/x.js", text.as_bytes()).unwrap();
        let mut back = String::new();
        std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(&gz[..]), &mut back).unwrap();
        assert_eq!(back, text);
        assert_eq!(gzipped("test/x.js", b"different").unwrap(), gz, "made once");
        assert!(gzipped("test/tiny.js", b"x").is_none(), "not worth it");
        assert!(compressible("application/javascript") && compressible("text/css") && !compressible("image/png"));
    }
}
