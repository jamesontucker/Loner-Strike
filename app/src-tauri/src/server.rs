//! The local web server the app window (and a phone) talks to. It answers the
//! same requests as panel/panel.py, so the existing web page works unchanged.

use crate::game::Game;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

const INDEX: &str = include_str!("../../../panel/web/index.html");

pub struct AppState {
    pub game: Option<Game>,
    /// Host headers we answer to; anything else is a DNS-rebinding attempt.
    pub hosts: Vec<String>,
}

type Shared = Arc<AppState>;

async fn only_our_host(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_lowercase();
    if !st.hosts.contains(&host) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    next.run(req).await
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

fn no_game() -> Value {
    json!({"online": false, "reason": "Couldn't find Condition Zero or Counter-Strike 1.6 on this computer."})
}

async fn api_state(State(st): State<Shared>) -> Json<Value> {
    let st2 = st.clone();
    let v = tokio::task::spawn_blocking(move || st2.game.as_ref().map(|g| g.state()).unwrap_or_else(no_game))
        .await
        .unwrap_or_else(|_| no_game());
    Json(v)
}

async fn api_health(State(st): State<Shared>) -> Json<Value> {
    Json(st.game.as_ref().map(|g| g.health()).unwrap_or_else(|| json!({"ok": false, "checks": [], "fixed": []})))
}

async fn api_maps() -> Json<Value> {
    Json(json!([])) // map list: next step of the port
}

async fn not_yet() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "Not in the app yet."})))
}

#[derive(Deserialize)]
struct Cmd {
    cmd: String,
    #[serde(default)]
    args: Vec<Value>,
}

async fn api_cmd(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<Cmd>) -> (StatusCode, Json<Value>) {
    // Only the panel sends this header; other sites can't add it without a CORS preflight
    if headers.get("x-panel").and_then(|v| v.to_str().ok()) != Some("1") {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(_) = st.game.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"ok": false, "error": "No game found."})));
    };
    let args: Vec<String> = req.args.iter().map(|a| a.as_str().map(String::from).unwrap_or_else(|| a.to_string())).collect();
    let st2 = st.clone();
    let res = tokio::task::spawn_blocking(move || st2.game.as_ref().unwrap().command(&req.cmd, &args)).await;
    match res {
        Ok(Ok(lines)) => (StatusCode::OK, Json(json!({"ok": true, "output": lines}))),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": e}))),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": "internal error"}))),
    }
}

pub async fn run(state: Shared, listener: std::net::TcpListener) {
    let app = Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/api/state", get(api_state))
        .route("/api/health", get(api_health))
        .route("/api/maps", get(api_maps))
        .route("/api/cmd", post(api_cmd))
        .route("/api/share", get(not_yet).post(not_yet))
        .route("/api/mapflag", post(not_yet))
        .layer(middleware::from_fn_with_state(state.clone(), only_our_host))
        .with_state(state);
    listener.set_nonblocking(true).ok();
    let listener = tokio::net::TcpListener::from_std(listener).expect("listener");
    axum::serve(listener, app).await.ok();
}
