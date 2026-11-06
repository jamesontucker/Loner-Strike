//! The game control API: same requests and answers as panel/panel.py, so the
//! web panel and the app share one protocol.

use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

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
    Json(json!([])) // map list: being ported
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

/// Only the app's own pages send X-Panel; another site can't add it without a CORS preflight.
pub fn from_panel(headers: &HeaderMap) -> bool {
    headers.get("x-panel").and_then(|v| v.to_str().ok()) == Some("1")
}

async fn api_cmd(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<Cmd>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    if st.game.is_none() {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"ok": false, "error": "No game found."})));
    }
    let args: Vec<String> = req.args.iter().map(|a| a.as_str().map(String::from).unwrap_or_else(|| a.to_string())).collect();
    let st2 = st.clone();
    let res = tokio::task::spawn_blocking(move || st2.game.as_ref().unwrap().command(&req.cmd, &args)).await;
    match res {
        Ok(Ok(lines)) => (StatusCode::OK, Json(json!({"ok": true, "output": lines}))),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": e}))),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": "internal error"}))),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/state", get(api_state))
        .route("/api/health", get(api_health))
        .route("/api/maps", get(api_maps))
        .route("/api/cmd", post(api_cmd))
        .route("/api/share", get(not_yet).post(not_yet))
        .route("/api/mapflag", post(not_yet))
}
