//! AI agents: Loner-Strike's Tools for coding agents (Claude Code and others) over the Model
//! Context Protocol. docs/MCP.md has the setup and a manual test.
//!
//! - Off by default (Settings > AI agents). On, it listens on 127.0.0.1:8029 (or any free
//!   port) at `/mcp`, its own listener, apart from the app window's.
//! - Every request needs this run's token (`Authorization: Bearer ...`), a local Host and no
//!   foreign Origin (a web page can't reach it, even by pointing its name at 127.0.0.1).
//! - Read-only unless "Allow changes" is on too; anything that overwrites a file or stops
//!   people playing asks the person at the computer first (catalog.rs).
//! - What agents did is listed in Settings (the activity log, this run only).
//!
//! rpc.rs is the protocol, catalog.rs the tools, stdio.rs a tiny bridge for agents that only
//! start local programs (`loner-strike --mcp-stdio`).

pub mod catalog;
pub mod conn;
pub mod debug;
pub mod editor_tools;
pub mod more;
pub mod routes;
pub mod rpc;
pub mod selection_tools;
pub mod stdio;
pub mod studio_tools;

use crate::server::Shared;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// The port agents are told to use (another free one if it's taken).
pub const PORT: u16 = 8029;

/// The person at the computer is asked (true: allowed). Blocks until they answer.
pub type Confirm = Arc<dyn Fn(String) -> bool + Send + Sync>;

/// What a tool call runs with.
pub struct Ctx {
    /// The app's own routes, as the app window reaches them (catalog::call).
    pub app: Router,
    /// The Host those requests carry (one the app window's guard lets in).
    pub host: String,
    /// "Allow changes" is on.
    pub changes: bool,
    pub confirm: Confirm,
    pub log: Arc<Log>,
    /// The app's state, for what has no route (the game's own console); None in tests.
    pub st: Option<Shared>,
}

// ---- the activity log -----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    Done,
    Refused,
    Failed,
}

/// What agents did this run, newest last (200 kept).
#[derive(Default)]
pub struct Log {
    rows: Mutex<VecDeque<Value>>,
    seq: AtomicU64,
}

impl Log {
    pub fn add(&self, tool: &str, detail: &str, outcome: Outcome, note: &str) {
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let n = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let outcome = match outcome {
            Outcome::Done => "done",
            Outcome::Refused => "refused",
            Outcome::Failed => "failed",
        };
        let mut rows = self.rows.lock().unwrap_or_else(|e| e.into_inner());
        if rows.len() >= 200 {
            rows.pop_front();
        }
        rows.push_back(json!({"n": n, "at": at, "tool": tool, "detail": detail, "outcome": outcome, "note": note}));
    }
    pub fn rows(&self) -> Vec<Value> {
        self.rows.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
    }
    pub fn clear(&self) {
        self.rows.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// A tool's arguments in one short line for the log.
pub fn short(args: &Value) -> String {
    let parts: Vec<String> = args
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| {
            let v = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{k}: {v}")
        })
        .collect();
    let line = parts.join(", ");
    if line.chars().count() > 120 {
        format!("{}…", line.chars().take(119).collect::<String>())
    } else {
        line
    }
}

/// A random token (letters and digits that can't be mistaken for each other).
pub fn random_token(n: usize) -> String {
    use rand::Rng;
    const C: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut r = rand::thread_rng();
    (0..n).map(|_| C[r.gen_range(0..C.len())] as char).collect()
}

// ---- the connector's state ------------------------------------------------------------

struct Running {
    port: u16,
    stop: tokio::sync::oneshot::Sender<()>,
}

struct Mcp {
    /// this run's token (new each time the app starts, or when asked)
    token: RwLock<String>,
    enabled: AtomicBool,
    changes: AtomicBool,
    /// the token is kept across restarts (conn::token_file)
    keep: AtomicBool,
    /// the port asked for (Settings), and why another one is used if it was taken
    port_pref: Mutex<u16>,
    port_note: Mutex<Option<String>>,
    running: Mutex<Option<Running>>,
    error: Mutex<Option<String>>,
    sessions: rpc::Sessions,
    log: Arc<Log>,
    st: OnceLock<Shared>,
}

fn mcp() -> &'static Mcp {
    static M: OnceLock<Mcp> = OnceLock::new();
    M.get_or_init(|| Mcp {
        token: RwLock::new(random_token(32)),
        enabled: AtomicBool::new(false),
        changes: AtomicBool::new(false),
        keep: AtomicBool::new(false),
        port_pref: Mutex::new(PORT),
        port_note: Mutex::new(None),
        running: Mutex::new(None),
        error: Mutex::new(None),
        sessions: rpc::Sessions::default(),
        log: Arc::new(Log::default()),
        st: OnceLock::new(),
    })
}

/// At app start: listens if it was left on.
pub fn start(st: &Shared) {
    let m = mcp();
    let _ = m.st.set(st.clone());
    let s = crate::app_api::load_settings();
    m.changes.store(s.mcp_changes, Ordering::Relaxed);
    m.enabled.store(s.mcp, Ordering::Relaxed);
    m.keep.store(s.mcp_keep_token, Ordering::Relaxed);
    *lock(&m.port_pref) = s.mcp_port.filter(|p| *p >= 1024).unwrap_or(PORT);
    if s.mcp_keep_token {
        let file = conn::token_file();
        match conn::kept_token(&file) {
            Some(t) => *m.token.write().unwrap() = t,
            None => {
                let _ = conn::keep_token(&file, &m.token.read().unwrap());
            }
        }
    }
    if s.mcp {
        switch(true);
    } else {
        // (left from a run that ended with it on)
        conn::remove(&conn::connection_file());
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Tells the stdio bridge where to connect (or that it can't), after any change.
fn publish() {
    let m = mcp();
    let port = lock(&m.running).as_ref().map(|r| r.port);
    let file = conn::connection_file();
    match port {
        Some(p) => {
            if let Err(e) = conn::write(&file, p, &m.token.read().unwrap()) {
                *lock(&m.error) = Some(format!("Couldn't save the connection details for the stdio bridge ({e}). HTTP still works."));
            }
        }
        None => conn::remove(&file),
    }
}

/// Starts or stops the listener to match `on`.
fn switch(on: bool) {
    let m = mcp();
    let mut running = m.running.lock().unwrap_or_else(|e| e.into_inner());
    if !on {
        if let Some(r) = running.take() {
            let _ = r.stop.send(());
        }
        m.sessions.clear();
        *m.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *lock(&m.port_note) = None;
        drop(running);
        publish();
        return;
    }
    if running.is_some() {
        return;
    }
    let Some(st) = m.st.get() else { return };
    let pref = *lock(&m.port_pref);
    match listen(st, pref) {
        Ok(r) => {
            *lock(&m.port_note) = (r.port != pref).then(|| format!("Port {pref} is taken by another program, so AI agents use {} this time. The stdio setup doesn't mind; for HTTP setups pick a free port.", r.port));
            *running = Some(r);
            *m.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        Err(e) => *m.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e),
    }
    drop(running);
    publish();
}

struct Endpoint {
    port: u16,
    app: Router,
    host: String,
    st: Shared,
}

fn listen(st: &Shared, port: u16) -> Result<Running, String> {
    let l = std::net::TcpListener::bind(("127.0.0.1", port))
        .or_else(|_| std::net::TcpListener::bind(("127.0.0.1", 0)))
        .map_err(|e| format!("Couldn't open a local port for AI agents: {e}"))?;
    l.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let host = st.hosts.read().unwrap().first().cloned().unwrap_or_else(|| "127.0.0.1:8027".into());
    let ep = Arc::new(Endpoint { port, app: crate::server::router(st.clone(), crate::server::Listener::Local), host, st: st.clone() });
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tauri::async_runtime::spawn(async move {
        let Ok(l) = tokio::net::TcpListener::from_std(l) else { return };
        let _ = axum::serve(l, routes(ep)).with_graceful_shutdown(async move {
            let _ = rx.await;
        })
        .await;
    });
    Ok(Running { port, stop: tx })
}

fn routes(ep: Arc<Endpoint>) -> Router {
    Router::new()
        .route("/mcp", post(endpoint).get(not_allowed).delete(not_allowed))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not here: the MCP endpoint is /mcp.") })
        .with_state(ep)
}

async fn not_allowed() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")], "This endpoint takes POST only.").into_response()
}

/// Who may talk to the endpoint: a local Host, no foreign Origin, this run's token.
pub fn check_access(h: &HeaderMap, port: u16, token: &str) -> Result<(), (StatusCode, &'static str)> {
    let host = h.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let local = [format!("127.0.0.1:{port}"), format!("localhost:{port}"), format!("[::1]:{port}")];
    if !local.contains(&host) {
        return Err((StatusCode::FORBIDDEN, "Only this computer can connect (Host must be 127.0.0.1)."));
    }
    if let Some(o) = h.get(header::ORIGIN) {
        let o = o.to_str().unwrap_or("").to_ascii_lowercase();
        if !local.iter().any(|l| o == format!("http://{l}")) {
            return Err((StatusCode::FORBIDDEN, "Web pages can't connect."));
        }
    }
    let given = h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
    if token.len() < 16 || !same(given.trim().as_bytes(), token.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Wrong or missing token. Copy the setup command from Settings > AI agents in Loner-Strike again: the token changes each time the app starts."));
    }
    Ok(())
}

/// Compares without stopping at the first difference.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

pub fn reply_response(r: rpc::Reply) -> Response {
    let mut res = match r.body {
        Some(b) => (r.status, Json(b)).into_response(),
        None => r.status.into_response(),
    };
    if let Some(s) = r.session.and_then(|s| s.parse().ok()) {
        res.headers_mut().insert("mcp-session-id", s);
    }
    res
}

async fn endpoint(State(ep): State<Arc<Endpoint>>, h: HeaderMap, body: Bytes) -> Response {
    let m = mcp();
    let token = m.token.read().unwrap().clone();
    if let Err((status, why)) = check_access(&h, ep.port, &token) {
        return reply_response(rpc::Reply::error(status, None, rpc::INVALID_REQUEST, why, None));
    }
    // (turned off while a request was on its way)
    if !m.enabled.load(Ordering::Relaxed) {
        return reply_response(rpc::Reply::error(StatusCode::FORBIDDEN, None, rpc::INVALID_REQUEST, "AI agents are off in Loner-Strike's Settings.", None));
    }
    let st = ep.st.clone();
    let ctx = Ctx {
        app: ep.app.clone(),
        host: ep.host.clone(),
        changes: m.changes.load(Ordering::Relaxed),
        confirm: Arc::new(move |text| ask_person(&st, text)),
        log: m.log.clone(),
        st: Some(ep.st.clone()),
    };
    reply_response(rpc::handle(&ctx, &m.sessions, &h, &body).await)
}

/// Asks the person at the computer, one question at a time; no window means no.
fn ask_person(st: &Shared, text: String) -> bool {
    use tauri::Manager;
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    static ONE: Mutex<()> = Mutex::new(());
    let _turn = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(app) = st.app.get() else { return false };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    app.dialog()
        .message(text)
        .title("An AI agent is asking")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Allow".into(), "Don't allow".into()))
        .blocking_show()
}

// ---- Settings (the app window's listener) ---------------------------------------------

fn view() -> Value {
    let m = mcp();
    let port = m.running.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|r| r.port);
    let token = m.token.read().unwrap().clone();
    let url = port.map(|p| format!("http://127.0.0.1:{p}/mcp"));
    let exe = std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "loner-strike".into());
    json!({
        "enabled": m.enabled.load(Ordering::Relaxed),
        "changes": m.changes.load(Ordering::Relaxed),
        "running": port.is_some(),
        "port": port,
        "url": url,
        "token": token,
        "error": m.error.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        "keep": m.keep.load(Ordering::Relaxed),
        "port_pref": *lock(&m.port_pref),
        "port_note": lock(&m.port_note).clone(),
        "setups": conn::snippets(&exe, url.as_deref(), &token),
        "userOnly": routes::USER_ONLY_LIST,
        "log": m.log.rows(),
    })
}

async fn api_get() -> Json<Value> {
    Json(view())
}

async fn api_set(h: HeaderMap, Json(b): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let enabled = b.get("enabled").and_then(Value::as_bool);
    let changes = b.get("changes").and_then(Value::as_bool);
    let keep = b.get("keep").and_then(Value::as_bool);
    let port = match b.get("port") {
        None => None,
        Some(p) => match p.as_u64().filter(|p| (1024..=65535).contains(p)) {
            Some(p) => Some(p as u16),
            None => return Json(json!({"error": "Pick a port from 1024 to 65535."})).into_response(),
        },
    };
    let v = tokio::task::spawn_blocking(move || {
        let m = mcp();
        if let Some(k) = keep {
            let file = conn::token_file();
            let saved = if k { conn::keep_token(&file, &m.token.read().unwrap()).map_err(|e| e.to_string()) } else { Ok(conn::remove(&file)) };
            if let Err(e) = saved {
                return json!({"error": format!("Couldn't keep the token: {e}")});
            }
            m.keep.store(k, Ordering::Relaxed);
            crate::app_api::edit_settings(|s| s.mcp_keep_token = k);
        }
        if let Some(p) = port {
            *lock(&m.port_pref) = p;
            crate::app_api::edit_settings(|s| s.mcp_port = (p != PORT).then_some(p));
            // listening already: moved to the new port
            if lock(&m.running).is_some() {
                switch(false);
                switch(true);
            }
        }
        crate::app_api::edit_settings(|s| {
            if let Some(e) = enabled {
                s.mcp = e;
                // turned off: changes go off with it, so turning it on again starts read-only
                if !e {
                    s.mcp_changes = false;
                }
            }
            if let Some(c) = changes {
                s.mcp_changes = c && s.mcp;
            }
            m.enabled.store(s.mcp, Ordering::Relaxed);
            m.changes.store(s.mcp_changes, Ordering::Relaxed);
        });
        if let Some(e) = enabled {
            switch(e);
        }
        view()
    })
    .await
    .unwrap_or_else(|_| json!({"error": "Couldn't change that setting."}));
    Json(v).into_response()
}

async fn api_token(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let m = mcp();
    *m.token.write().unwrap() = random_token(32);
    m.sessions.clear();
    if m.keep.load(Ordering::Relaxed) {
        let _ = conn::keep_token(&conn::token_file(), &m.token.read().unwrap());
    }
    publish();
    Json(view()).into_response()
}

async fn api_clear(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    mcp().log.clear();
    Json(view()).into_response()
}

/// The app window's listener only (merged in server.rs): the token is never on the phone's.
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/mcp", get(api_get).post(api_set))
        .route("/api/mcp/token", post(api_token))
        .route("/api/mcp/log/clear", post(api_clear))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod e2e_tests;
