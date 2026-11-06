//! The setup wizard's API: find games, install Loner-Strike and its modes, undo it.
//!
//!   GET  /api/setup/scan       installs found, what's on them, the catalog
//!   POST /api/setup/install    { hl, games, components, repair? } -> { job }
//!   POST /api/setup/uninstall  { hl, games, components? }         -> { job }
//!   GET  /api/setup/job        the current (or last) job's progress
//!
//! POSTs need the X-Panel: 1 header, like the rest of the panel API.

use crate::gamebanana::MapStore;
use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::setup;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn reply(code: StatusCode, v: Value) -> Response {
    (code, [(header::CACHE_CONTROL, "no-store")], Json(v)).into_response()
}

fn bad(msg: impl Into<String>) -> Response {
    reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": msg.into()}))
}

/// Loner-Strike's mod files: the app's bundled resources, or the repo in dev.
fn resources(st: &Shared) -> Result<PathBuf, String> {
    use tauri::path::BaseDirectory;
    use tauri::Manager;
    let bundled = st
        .app
        .get()
        .and_then(|app| app.path().resolve("catalog", BaseDirectory::Resource).ok())
        .and_then(|p| p.parent().map(Path::to_path_buf));
    setup::resource_root(bundled)
}

/// The map store for an install: the panel's own when it's the same game folder,
/// so its map list refreshes too.
fn store_for(st: &Shared, hl: &Path, game: &str) -> Arc<MapStore> {
    if let Some(g) = st.game.as_ref().filter(|g| g.hl == hl) {
        return g.store.clone();
    }
    Arc::new(MapStore::new(hl, &hl.join(game), &crate::sys::app_dir(), Arc::new(Mutex::new(None))))
}

fn body(body: &Bytes) -> Result<Value, Response> {
    if body.len() > 64 * 1024 {
        return Err(bad("request too big"));
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(v @ Value::Object(_)) => Ok(v),
        _ => Err(bad("expected a JSON object")),
    }
}

fn list(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect()
}

/// Only installs we found ourselves can be changed (never an arbitrary folder).
fn find_install(hl: &str) -> Result<crate::steam::Install, String> {
    crate::steam::find_installs()
        .into_iter()
        .find(|i| i.hl == Path::new(hl))
        .ok_or_else(|| "That game folder wasn't found any more. Scan again.".into())
}

async fn scan(State(st): State<Shared>) -> Response {
    let res = match resources(&st) {
        Ok(r) => r,
        Err(e) => return reply(StatusCode::INTERNAL_SERVER_ERROR, json!({"ok": false, "error": e})),
    };
    match tokio::task::spawn_blocking(move || setup::scan(&res)).await {
        Ok(v) => reply(StatusCode::OK, v),
        Err(_) => reply(StatusCode::INTERNAL_SERVER_ERROR, json!({"ok": false, "error": "Couldn't look for the game."})),
    }
}

async fn job() -> Response {
    reply(StatusCode::OK, serde_json::to_value(setup::current()).unwrap_or(Value::Null))
}

async fn install(State(st): State<Shared>, headers: HeaderMap, raw: Bytes) -> Response {
    if !from_panel(&headers) {
        return reply(StatusCode::FORBIDDEN, json!({"error": "forbidden"}));
    }
    let req = match body(&raw) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let res = resources(&st2)?;
        let install = find_install(req["hl"].as_str().unwrap_or(""))?;
        let plan = setup::plan_install(install, &list(&req["games"]), &list(&req["components"]), &res, req["repair"].as_bool() == Some(true))?;
        let store = store_for(&st2, &plan.install.hl, &plan.games[0]);
        setup::start_install(plan, res, Some(store))
    })
    .await;
    started(r)
}

async fn uninstall(State(st): State<Shared>, headers: HeaderMap, raw: Bytes) -> Response {
    if !from_panel(&headers) {
        return reply(StatusCode::FORBIDDEN, json!({"error": "forbidden"}));
    }
    let req = match body(&raw) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let res = resources(&st2)?;
        let install = find_install(req["hl"].as_str().unwrap_or(""))?;
        let comps = req.get("components").filter(|c| c.is_array()).map(list);
        let plan = setup::plan_uninstall(install, &list(&req["games"]), comps, &res)?;
        let store = store_for(&st2, &plan.install.hl, &plan.games[0]);
        setup::start_uninstall(plan, res, Some(store))
    })
    .await;
    started(r)
}

fn started(r: Result<Result<String, String>, tokio::task::JoinError>) -> Response {
    match r {
        Ok(Ok(id)) => reply(StatusCode::OK, json!({"ok": true, "job": id})),
        Ok(Err(e)) if e.starts_with("Setup is already") => reply(StatusCode::CONFLICT, json!({"ok": false, "error": e})),
        Ok(Err(e)) => bad(e),
        Err(_) => reply(StatusCode::INTERNAL_SERVER_ERROR, json!({"ok": false, "error": "Setup couldn't start."})),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/setup/scan", get(scan))
        .route("/api/setup/job", get(job))
        .route("/api/setup/install", post(install))
        .route("/api/setup/uninstall", post(uninstall))
}
