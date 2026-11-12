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

/// Set on requests from the owner's signed-in phone (the LAN listener), only once its token
/// checked out. Never set by the app window's guard or the sharing listener's (helpers).
#[derive(Clone, Copy)]
pub struct OnPhone;

/// Whether a request is the owner's: the app window, or their signed-in phone. A helper
/// (shared control) is neither. For what the phone may do as well as the computer; the
/// handler still refuses the phone what stays on the computer (deleting, making servers...).
pub fn owner(on: &Option<axum::Extension<OnComputer>>, phone: &Option<axum::Extension<OnPhone>>) -> bool {
    on.is_some() || phone.is_some()
}

/// The phone listener (LAN): always needs the token, whatever the request says
/// about itself, and only answers to the phone address.
async fn guard_lan(State(st): State<Shared>, mut req: Request, next: Next) -> Response {
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
        // a phone opening the page: a readable page instead of one bare line
        let page = req.method() == axum::http::Method::GET && matches!(req.uri().path(), "/" | "/index.html");
        if page {
            return (StatusCode::FORBIDDEN, axum::response::Html(SIGNED_OUT_PAGE)).into_response();
        }
        return (StatusCode::FORBIDDEN, "Scan the QR code in Loner-Strike's Settings to connect.").into_response();
    }
    // signed in: the owner's phone (handlers that allow it check owner())
    req.extensions_mut().insert(OnPhone);
    let mut res = next.run(req).await;
    if query_ok {
        // (HttpOnly: the page's own scripts never need it, so nothing on the page can read it)
        let c = format!("ls_token={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000", st.token.read().unwrap());
        res.headers_mut().insert(header::SET_COOKIE, c.parse().unwrap());
    }
    res
}

/// What a phone sees when it isn't signed in (a new code, or never scanned).
const SIGNED_OUT_PAGE: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Loner-Strike</title>
<style>html,body{height:100%;margin:0}body{display:grid;place-items:center;padding:24px;box-sizing:border-box;background:#3e4637;color:#d8ded3;font:500 17px/1.45 "Barlow Semi Condensed","Avenir Next Condensed","Arial Narrow",system-ui,sans-serif;text-align:center}
main{max-width:420px;background:#4c5844;padding:22px 24px;border-radius:3px;box-shadow:inset 1px 1px 0 rgba(136,145,128,.55),inset -1px -1px 0 #292c21}h1{margin:0 0 10px;font-size:22px;color:#c4b550}p{margin:0;color:#c2c9b7}</style></head>
<body><main><h1>Not signed in</h1><p>This phone isn't signed in any more. Scan the QR code in Settings on the computer.</p></main></body></html>"#;

fn host_of(req: &Request) -> String {
    req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_lowercase()
}

/// Whether a change request (POST) may write the server's files, so a server on another
/// machine needs a sync after it. Requests that only talk to the game over RCON (a game
/// command, a kick, a match button) or touch nothing of the server's (smarter chat runs on
/// this computer) don't.
pub(crate) fn writes_files(path: &str) -> bool {
    !matches!(path, "/api/cmd" | "/api/admins/kick" | "/api/scrim/action") && !path.starts_with("/api/smartchat") && !path.starts_with("/api/botchat") && !path.starts_with("/api/backup/")
}

/// A request naming a dedicated server (X-Target: <id>, or ?target=<id> for images)
/// runs with that server as its game.
async fn with_target(req: Request, next: Next) -> Response {
    let header = req.headers().get("x-target").and_then(|v| v.to_str().ok()).map(String::from);
    let query = req.uri().query().and_then(|q| q.split('&').find_map(|p| p.strip_prefix("target=").map(String::from)));
    // a server a friend shared with you (control::client): the app window's requests for it
    // go to the owner's app, which decides what you may do there
    if req.extensions().get::<OnComputer>().is_some() {
        let p = req.uri().path();
        // a path that names a server decides by itself (one of yours stays here even with a
        // shared server open); only a path naming none goes by the open server
        let path_id = p.strip_prefix("/api/servers/").and_then(|r| r.split('/').next()).filter(|id| !id.is_empty());
        let named = match path_id {
            Some(id) => crate::control::client::is_shared(id).then(|| id.to_string()),
            None => header.clone().or(query.clone()).filter(|t| crate::control::client::is_shared(t)),
        };
        if let Some(id) = named {
            // this app's own business (its info, setup, the list): answered here, for your game
            if !crate::control::client::about_the_server(p) {
                return next.run(req).await;
            }
            let (parts, body) = req.into_parts();
            let pq = parts.uri.path_and_query().map(|x| x.as_str().to_string()).unwrap_or_default();
            return crate::control::client::forward(id, parts.method, pq, parts.headers, body).await;
        }
    }
    // the server list and per-server routes name their server in the path: they never
    // depend on (or fail because of) the one that's open
    let path = req.uri().path();
    let by_path = path == "/api/servers" || path.starts_with("/api/servers/") || path.starts_with("/api/backup/");
    let post = req.method() == axum::http::Method::POST;
    // a request that edits a list shared between servers passes the edit on
    // (a helper's change stays on the server they were given: it isn't passed to the owner's
    // other servers or game)
    let helper = req.extensions().get::<crate::control::Helper>().is_some();
    let list = if post && !helper { crate::lists::list_for(path) } else { None };
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
    // nothing changes on a server while a backup is put back on it
    if post && writes_files(path) && crate::backup::restoring_server(&id) {
        return (StatusCode::OK, axum::Json(serde_json::json!({"error": crate::backup::RESTORING_SERVER_BUSY}))).into_response();
    }
    let id2 = id.clone();
    let writes_files = writes_files(path);
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
        // an API path this listener doesn't serve (computer-only, asked from a phone):
        // say so, so the page can explain, instead of answering with the HTML page
        None if file.starts_with("api/") => return (StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "forbidden"}))).into_response(),
        None => match Ui::get("index.html") {
            Some(f) => ("index.html", f),
            None => return (StatusCode::NOT_FOUND, "The app's pages are missing. Rebuild the UI (npm run build in app/ui).").into_response(),
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

/// An embedded file gzipped, made once on first ask and kept with a fingerprint of what it
/// was made from: a release's files never change, but a dev build reads them from disk and
/// a rebuilt page must not be answered with the old one. None if it doesn't get smaller.
fn gzipped(file: &str, data: &[u8]) -> Option<axum::body::Bytes> {
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::io::Write;
    static GZ: Mutex<Option<HashMap<String, (u64, Option<axum::body::Bytes>)>>> = Mutex::new(None);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    let print = h.finish();
    if let Some((p, v)) = GZ.lock().unwrap().as_ref().and_then(|m| m.get(file)) {
        if *p == print {
            return v.clone();
        }
    }
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let gz = enc.write_all(data).and_then(|_| enc.finish()).ok().filter(|g| g.len() + 64 < data.len()).map(axum::body::Bytes::from);
    GZ.lock().unwrap().get_or_insert_with(HashMap::new).insert(file.to_string(), (print, gz.clone()));
    gz
}

/// Which listener a router is for decides its guard (never the request's own headers).
#[derive(Clone, Copy, PartialEq)]
pub enum Listener {
    Local,
    Lan,
    /// friends' Loner-Strike helping run a shared server (control/): TLS, their key, their role
    Share,
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
        .merge(crate::voice::routes())
        .merge(crate::webstatus::routes())
        .merge(crate::servercontent::routes())
        .merge(crate::welcome::routes())
        .merge(crate::extras::routes())
        .merge(crate::modevote::routes())
        .merge(crate::scrim::routes())
        .merge(crate::community::routes())
        .merge(crate::prefs::routes())
        .merge(crate::plugins::routes())
        .merge(crate::sounds::routes())
        .merge(crate::saved::routes())
        .fallback(ui);
    // X-Target is applied after the computer-only routes are merged (a layer only covers
    // routes already there), and the listener's guard stays outermost
    let routes = match listener {
        Listener::Local => owner_routes(routes)
            .merge(crate::setup_api::local_routes())
            .merge(crate::app_api::local_routes())
            .merge(crate::servers::local_routes())
            .merge(crate::mapcopy::local_routes())
            .merge(crate::hltv::local_routes())
            .merge(crate::prefs::local_routes())
            .merge(crate::backup::local_routes())
            .merge(crate::plugins::local_routes())
            .merge(crate::sounds::local_routes())
            .merge(crate::extras::local_routes())
            .merge(crate::browser::routes())
            .merge(crate::smartchat::local_routes())
            .merge(crate::control::local_routes())
            .layer(middleware::from_fn(crate::diagnostics::log_errors))
            .layer(middleware::from_fn(with_target))
            .layer(middleware::from_fn_with_state(state.clone(), guard_local)),
        Listener::Lan => owner_routes(routes)
            .layer(middleware::from_fn(crate::diagnostics::log_errors))
            .layer(middleware::from_fn(with_target))
            .layer(middleware::from_fn_with_state(state.clone(), guard_lan)),
        // only the shared routes (never the computer's), and the guard decides each request
        Listener::Share => routes
            .merge(crate::control::share_routes())
            .layer(middleware::from_fn(crate::diagnostics::log_errors))
            .layer(middleware::from_fn(with_target))
            .layer(middleware::from_fn_with_state(state.clone(), crate::control::guard)),
    };
    routes.with_state(state)
}

/// Routes for the owner on the app window and on their signed-in phone, never the sharing
/// listener (helpers): each handler checks owner() as well.
fn owner_routes(routes: Router<Shared>) -> Router<Shared> {
    routes
        .merge(crate::servers::owner_routes())
        .merge(crate::navs::owner_routes())
        .merge(crate::smartchat::owner_routes())
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
        assert_eq!(gzipped("test/x.js", text.as_bytes()).unwrap(), gz, "made once for the same file");
        let changed = "const y = 2;\n".repeat(500);
        let gz2 = gzipped("test/x.js", changed.as_bytes()).unwrap();
        let mut back2 = String::new();
        std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(&gz2[..]), &mut back2).unwrap();
        assert_eq!(back2, changed, "a rebuilt file isn't answered with the old copy");
        assert!(gzipped("test/tiny.js", b"x").is_none(), "not worth it");
        assert!(compressible("application/javascript") && compressible("text/css") && !compressible("image/png"));
    }

    const LAN: &str = "192.168.1.5:8028";
    const TOKEN: &str = "testtoken0123456789";

    fn state() -> Shared {
        Arc::new(AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec![LAN.into()]),
            token: RwLock::new(TOKEN.into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        })
    }

    /// Says which of the owner's markers a request got past the listener's guard.
    async fn probe(on: Option<axum::Extension<OnComputer>>, phone: Option<axum::Extension<OnPhone>>) -> String {
        format!("computer={} phone={} owner={}", on.is_some(), phone.is_some(), owner(&on, &phone))
    }

    async fn status_and_text(app: Router, req: Request) -> (StatusCode, String) {
        use tower::ServiceExt;
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    fn get(host: &str, uri: &str) -> Request {
        Request::builder().uri(uri).header("host", host).body(axum::body::Body::empty()).unwrap()
    }

    #[test]
    fn owner_is_the_computer_or_the_phone() {
        assert!(owner(&Some(axum::Extension(OnComputer)), &None));
        assert!(owner(&None, &Some(axum::Extension(OnPhone))));
        // a helper (shared control) carries neither
        assert!(!owner(&None, &None));
    }

    #[tokio::test]
    async fn only_a_signed_in_phone_is_marked() {
        let st = state();
        let lan = || Router::new().route("/probe", axum::routing::get(probe)).layer(middleware::from_fn_with_state(st.clone(), guard_lan));
        let local = || Router::new().route("/probe", axum::routing::get(probe)).layer(middleware::from_fn_with_state(st.clone(), guard_local));
        // signed in: the phone, not the computer
        let (s, t) = status_and_text(lan(), get(LAN, &format!("/probe?t={TOKEN}"))).await;
        assert_eq!((s, t.as_str()), (StatusCode::OK, "computer=false phone=true owner=true"));
        let mut req = get(LAN, "/probe");
        req.headers_mut().insert(header::COOKIE, format!("ls_token={TOKEN}").parse().unwrap());
        assert_eq!(status_and_text(lan(), req).await.1, "computer=false phone=true owner=true");
        // no token, a wrong one, or the wrong host: never reaches the handler
        for req in [get(LAN, "/probe"), get(LAN, "/probe?t=wrongtoken0123456789"), get("evil.example:8028", &format!("/probe?t={TOKEN}"))] {
            assert_eq!(status_and_text(lan(), req).await.0, StatusCode::FORBIDDEN);
        }
        // the cookie it's given: out of the page's scripts' reach
        let res = tower::ServiceExt::oneshot(lan(), get(LAN, &format!("/probe?t={TOKEN}"))).await.unwrap();
        let cookie = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().to_string();
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"), "{cookie}");
        // the app window: the computer, never the phone
        assert_eq!(status_and_text(local(), get("127.0.0.1:8027", "/probe")).await.1, "computer=true phone=false owner=true");
    }

    fn phone_post(uri: &str, body: &str) -> Request {
        let sep = if uri.contains('?') { '&' } else { '?' };
        Request::builder()
            .method("POST")
            .uri(format!("{uri}{sep}t={TOKEN}"))
            .header("host", LAN)
            .header("x-panel", "1")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    const HLTV_BODY: &str = r#"{"enabled": false, "port": 27021, "delay": 30, "max_spectators": 4, "record": false}"#;
    const GONE: &str = "That server isn't there any more.";

    #[tokio::test]
    async fn the_phone_reaches_what_was_opened_to_it() {
        let st = state();
        // each answered by its handler (here: about a server that isn't there, or a request it
        // turns down on its merits), never refused as the phone
        crate::voice::pretend_installing(true);
        let rows: &[(&str, &str, &str, &[&str])] = &[
            ("POST", "/api/servers/nosuchserver", r#"{"name": "New name", "map": "de_dust2", "maxplayers": 12, "auto_update": false}"#, &[GONE]),
            ("POST", "/api/servers/nosuchserver/rcon-password", "{}", &[GONE]),
            ("GET", "/api/servers/nosuchserver/security", "", &[GONE]),
            ("POST", "/api/servers/nosuchserver/security/fix", r#"{"item": "logs"}"#, &[GONE]),
            ("POST", "/api/servers/nosuchserver/console/send", r#"{"command": "status"}"#, &[GONE]),
            ("POST", "/api/servers/nosuchserver/console/hints", "{}", &[GONE]),
            ("POST", "/api/servers/nosuchserver/hltv", HLTV_BODY, &[GONE]),
            ("POST", "/api/servers/nosuchserver/webstatus", r#"{"enabled": false}"#, &[GONE]),
            ("POST", "/api/servers/nosuchserver/voice", r#"{"enabled": false}"#, &[GONE]),
            ("POST", "/api/servers/nosuchserver/update", "{}", &[GONE]),
            ("POST", "/api/voice/install", "{}", &["It's being installed already.", "too old for voice"]),
            ("POST", "/api/sounds/pack", r#"{"id": 999999999, "action": "install"}"#, &["Unknown pack."]),
            ("POST", "/api/lists", r#"{"list": "nosuchlist", "members": [], "from": ""}"#, &["Unknown list."]),
            ("POST", "/api/navs/learn", r#"{"maps": ["de_dust2"]}"#, &["Set up the game on this computer first."]),
            ("POST", "/api/navs/learn/stop", "{}", &["Bots aren't learning maps right now."]),
        ];
        for (method, uri, body, expect) in rows {
            let req = if *method == "GET" { get(LAN, &format!("{uri}?t={TOKEN}")) } else { phone_post(uri, body) };
            let (s, t) = status_and_text(router(st.clone(), Listener::Lan), req).await;
            assert_ne!(s, StatusCode::FORBIDDEN, "{uri}: {t}");
            assert!(!t.contains("forbidden") && !t.contains("Do that from the computer"), "{uri}: {t}");
            assert!(expect.iter().any(|e| t.contains(e)), "{uri}: {s} {t}");
        }
        crate::voice::pretend_installing(false);
        let (s, t) = status_and_text(router(st.clone(), Listener::Lan), get(LAN, &format!("/api/smartchat?t={TOKEN}"))).await;
        assert_eq!(s, StatusCode::OK);
        assert!(t.contains("\"models\""), "{t}");
    }

    #[tokio::test]
    async fn the_phone_doesnt_get_secrets_or_the_whole_console() {
        let st = state();
        // showing the console password and the website status secrets: refused
        for (uri, body) in [("/api/servers/nosuchserver/rcon-password/show", "{}"), ("/api/servers/nosuchserver/webstatus/secret", r#"{"kind": "token"}"#)] {
            let (s, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post(uri, body)).await;
            assert_eq!(s, StatusCode::FORBIDDEN, "{uri}: {t}");
        }
        // a console password of its own choosing: refused (a random one is made instead)
        let (_, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post("/api/servers/nosuchserver/rcon-password", r#"{"password": "known-password-1"}"#)).await;
        assert!(t.contains("Type your own console password on the computer"), "{t}");
        // console lines: refused before the server is looked up; a plain one goes on
        for (line, why) in [
            ("exec x.cfg", "only be sent from the computer"),
            ("amxx unload x", "only be sent from the computer"),
            ("sv_downloadurl http://example.com", "only be sent from the computer"),
            ("status;quit", "One command at a time"),
            ("RCON_PASSWORD x", "can't be sent"),
        ] {
            let body = serde_json::json!({ "command": line }).to_string();
            let (_, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post("/api/servers/nosuchserver/console/send", &body)).await;
            assert!(t.contains(why), "{line}: {t}");
        }
        let (_, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post("/api/servers/nosuchserver/console/send", r#"{"command": "status"}"#)).await;
        assert!(t.contains(GONE), "{t}");
    }

    #[tokio::test]
    async fn the_phone_still_cant_delete_or_make_servers() {
        let st = state();
        // deleting, rebuilding and setting up again: refused before anything is looked up
        for action in ["delete", "rebuild", "retry"] {
            let (s, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post(&format!("/api/servers/nosuchserver/{action}"), "{}")).await;
            assert_eq!(s, StatusCode::OK, "{action}");
            assert!(t.contains("Do that from the computer"), "{action}: {t}");
        }
        // removing what's installed: refused
        for (uri, body) in [
            ("/api/voice/uninstall", "{}"),
            ("/api/sounds/pack", r#"{"id": 1, "action": "remove"}"#),
            ("/api/gb/uninstall", r#"{"mod": 1, "section": "maps"}"#),
            ("/api/servers/nosuchserver/hltv/delete-demo", r#"{"name": "x.dem"}"#),
        ] {
            assert_eq!(status_and_text(router(st.clone(), Listener::Lan), phone_post(uri, body)).await.0, StatusCode::FORBIDDEN, "{uri}");
        }
        // computer-only paths under a server's own: on the phone listener they fall to a
        // server's settings (for a server of that name: none) or its actions (refused), never
        // to importing, picking or making a server
        let body = r#"{"dir": "/tmp", "host": "203.0.113.1"}"#;
        for (uri, expect) in [
            ("/api/servers/remote", GONE),
            ("/api/servers/import", GONE),
            ("/api/servers/pick-folder", GONE),
            ("/api/servers/pick-key", GONE),
            ("/api/servers/remote/probe", "Do that from the computer running Loner-Strike."),
            ("/api/servers/nosuchserver/hltv-folder", "Do that from the computer running Loner-Strike."),
        ] {
            let (_, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post(uri, body)).await;
            assert_eq!(t, serde_json::json!({ "error": expect }).to_string(), "{uri}");
        }
        // computer-only routes the phone listener doesn't serve at all
        for uri in [
            "/api/smartchat/remove",
            "/api/backup/restore",
            "/api/backup/export",
            "/api/control/server/nosuchserver/invite",
            "/api/app/remove-everything",
            "/api/setup/uninstall",
            "/api/mods/plugins/remove",
        ] {
            let (s, t) = status_and_text(router(st.clone(), Listener::Lan), phone_post(uri, "{}")).await;
            assert_eq!(s, StatusCode::FORBIDDEN, "{uri}");
            assert!(t.contains("forbidden"), "{uri}: {t}");
        }
    }

    /// The routes the owner's phone was given, behind a layer that marks every request as a
    /// helper's (what control::guard does once a helper's key and role check out).
    fn as_helper(st: Shared) -> Router {
        let m = crate::control::test_member(crate::control::Role::Admin, "nosuchserver");
        Router::new()
            .merge(crate::servers::routes())
            .merge(crate::servers::owner_routes())
            .merge(crate::hltv::routes())
            .merge(crate::webstatus::routes())
            .merge(crate::voice::routes())
            .merge(crate::sounds::routes())
            .merge(crate::lists::routes())
            .merge(crate::navs::owner_routes())
            .merge(crate::smartchat::owner_routes())
            .layer(axum::Extension(crate::control::Helper(m)))
            .with_state(st)
    }

    fn helper_req(method: &str, uri: &str, body: &str) -> Request {
        Request::builder().method(method).uri(uri).header("x-panel", "1").header("content-type", "application/json").body(axum::body::Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn helpers_get_none_of_the_phones_powers() {
        let st = state();
        for (method, uri, body) in [
            ("GET", "/api/servers/nosuchserver/security", ""),
            ("POST", "/api/servers/nosuchserver/security/fix", r#"{"item": "logs"}"#),
            ("POST", "/api/servers/nosuchserver/rcon-password/show", "{}"),
            ("POST", "/api/servers/nosuchserver/rcon-password", "{}"),
            ("POST", "/api/servers/nosuchserver/console/send", r#"{"command": "status"}"#),
            ("POST", "/api/servers/nosuchserver/console/hints", "{}"),
            ("POST", "/api/servers/nosuchserver/hltv", HLTV_BODY),
            ("POST", "/api/servers/nosuchserver/webstatus", r#"{"enabled": false}"#),
            ("POST", "/api/servers/nosuchserver/webstatus/secret", r#"{"kind": "token"}"#),
            ("POST", "/api/servers/nosuchserver/voice", r#"{"enabled": false}"#),
            ("POST", "/api/voice/install", "{}"),
            ("POST", "/api/sounds/pack", r#"{"id": 1, "action": "install"}"#),
            ("POST", "/api/lists", r#"{"list": "bans", "members": [], "from": ""}"#),
            ("POST", "/api/navs/learn", r#"{"maps": []}"#),
            ("POST", "/api/navs/learn/stop", "{}"),
            ("POST", "/api/navs/send", r#"{"map": "de_dust2"}"#),
            ("POST", "/api/smartchat/set", "{}"),
            ("POST", "/api/smartchat/download", r#"{"id": "x"}"#),
        ] {
            let (s, t) = status_and_text(as_helper(st.clone()), helper_req(method, uri, body)).await;
            assert_eq!(s, StatusCode::FORBIDDEN, "{uri}: {t}");
        }
        // updates and deleting stay out of a helper's reach, as before
        for action in ["update", "apply-updates", "delete"] {
            let (_, t) = status_and_text(as_helper(st.clone()), helper_req("POST", &format!("/api/servers/nosuchserver/{action}"), "{}")).await;
            assert!(t.contains("Do that from the computer"), "{action}: {t}");
        }
        // an Admin helper still changes the server's basics (answered by the handler)
        let (s, t) = status_and_text(as_helper(st.clone()), helper_req("POST", "/api/servers/nosuchserver", r#"{"name": "New name"}"#)).await;
        assert_eq!((s, t), (StatusCode::OK, serde_json::json!({ "error": GONE }).to_string()));
    }

    #[test]
    fn rcon_only_requests_dont_sync() {
        for p in ["/api/cmd", "/api/admins/kick", "/api/scrim/action", "/api/smartchat/set", "/api/smartchat/try/start", "/api/botchat/reply"] {
            assert!(!writes_files(p), "{p}");
        }
        for p in ["/api/admins/ban", "/api/scrim/settings", "/api/community/gag", "/api/mapflag"] {
            assert!(writes_files(p), "{p}");
        }
    }
}
