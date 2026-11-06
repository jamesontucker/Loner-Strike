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
pub fn resources_for(st: &Shared) -> Result<PathBuf, String> {
    resources(st)
}

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
    if let Some(g) = st.game().filter(|g| g.hl == hl) {
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
        .or_else(|| crate::servers::find_install(Path::new(hl)))
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

#[cfg(test)]
mod tests {
    use crate::server::{self, AppState, Listener};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex, OnceLock, RwLock};
    use tower::ServiceExt;

    const HOST: &str = "127.0.0.1:8027";

    fn state() -> server::Shared {
        Arc::new(AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec![HOST.into()]),
            lan_hosts: RwLock::new(vec![]),
            token: "testtoken0123456789".into(),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        })
    }

    async fn call(st: &server::Shared, method: &str, uri: &str, body: Option<Value>, panel: bool) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri).header("host", HOST);
        if panel {
            req = req.header("x-panel", "1");
        }
        if body.is_some() {
            req = req.header("content-type", "application/json");
        }
        let req = req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap();
        let res = server::router(st.clone(), Listener::Local).oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    /// Every per-server route reaches its handler (not the page fallback).
    #[tokio::test]
    async fn server_routes_match() {
        let st = state();
        let (_, v) = call(&st, "POST", "/api/servers/no-such-server/bogus", Some(json!({})), true).await;
        assert_eq!(v["error"], "Unknown action.");
        let (_, v) = call(&st, "GET", "/api/servers/no-such-server/console", None, false).await;
        assert!(v["lines"].is_array());
        let (_, v) = call(&st, "POST", "/api/servers/no-such-server", Some(json!({})), true).await;
        assert!(v["error"].is_string());
        let (_, v) = call(&st, "POST", "/api/servers/no-such-server/console/send", Some(json!({"command": "status"})), true).await;
        assert!(v["error"].is_string());
        let (_, v) = call(&st, "GET", "/api/welcome", None, false).await;
        assert!(v.is_object());
        // a request for a server that doesn't exist never falls back to your game
        let req = axum::http::Request::builder().uri("/api/state").header("host", HOST).header("x-target", "no-such-server").body(Body::empty()).unwrap();
        let res = server::router(st.clone(), Listener::Local).oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let req = axum::http::Request::builder().uri("/api/state").header("host", HOST).header("x-target", "../etc").body(Body::empty()).unwrap();
        let res = server::router(st.clone(), Listener::Local).oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn scan_job_and_guards() {
        let st = state();
        let (code, v) = call(&st, "GET", "/api/setup/scan", None, false).await;
        assert_eq!(code, StatusCode::OK);
        assert!(v["installs"].is_array() && v["native_mac"].is_boolean());
        for i in v["installs"].as_array().unwrap() {
            let ids: Vec<&str> = i["games"].as_array().unwrap().iter().filter_map(|g| g["id"].as_str()).collect();
            assert!(ids != ["czero", "cstrike"], "CS 1.6 comes first");
            for g in i["games"].as_array().unwrap() {
                assert!(g["installed"].is_boolean() && g["foreign"].is_boolean() && g["components"].is_array());
            }
        }
        let pb = v["catalog"].as_array().unwrap().iter().find(|c| c["id"] == "mode-paintball").unwrap();
        assert_eq!(pb["kind"], "mode");
        assert_eq!(pb["games"], json!(["cstrike", "czero"]));
        assert_eq!(pb["modes"], json!(["paintball"]));

        let (code, v) = call(&st, "GET", "/api/setup/job", None, false).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(v["state"], "idle");
        assert!(v["steps"].is_array() && v["failed_compiles"].is_array());

        let body = json!({"hl": "/nowhere/Half-Life", "games": ["czero"], "components": []});
        let (code, _) = call(&st, "POST", "/api/setup/install", Some(body.clone()), false).await;
        assert_eq!(code, StatusCode::FORBIDDEN);
        let (code, v) = call(&st, "POST", "/api/setup/install", Some(body.clone()), true).await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
        assert!(v["error"].as_str().unwrap().contains("wasn't found"));
        let (code, _) = call(&st, "POST", "/api/setup/uninstall", Some(body.clone()), false).await;
        assert_eq!(code, StatusCode::FORBIDDEN);
        let (code, _) = call(&st, "POST", "/api/setup/uninstall", Some(body), true).await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
        let (code, _) = call(&st, "POST", "/api/setup/install", Some(json!([1])), true).await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
    }
}
