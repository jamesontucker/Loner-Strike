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
/// `target` is read before spawn_blocking: the request's X-Target isn't visible there.
fn store_for(target: Option<Arc<crate::game::Game>>, hl: &Path, game: &str) -> Arc<MapStore> {
    if let Some(g) = target.filter(|g| g.hl == hl) {
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

/// Whether the game is open (setup asks people to close it first).
async fn running() -> Response {
    let r = tokio::task::spawn_blocking(crate::sys::game_running).await.unwrap_or(false);
    reply(StatusCode::OK, json!({"running": r}))
}

const CLOSE_GAME: &str = "Counter-Strike is open. Close it, then try again: setup can't change the game's files while it's running.";

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
    let target = st.game();
    let r = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let res = resources(&st2)?;
        let install = find_install(req["hl"].as_str().unwrap_or(""))?;
        if !install.dedicated && crate::sys::game_running() {
            return Err(CLOSE_GAME.into());
        }
        // a server on this computer that's running has its files open (Windows won't
        // let them be replaced, and a server shouldn't change under its players anyway)
        if install.dedicated && crate::servers::running_at(&install.hl) {
            return Err("Stop the server first, then try again.".into());
        }
        let plan = setup::plan_install(install, &list(&req["games"]), &list(&req["components"]), &res, req["repair"].as_bool() == Some(true))?;
        let store = store_for(target, &plan.install.hl, &plan.games[0]);
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
    let target = st.game();
    let r = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let res = resources(&st2)?;
        let install = find_install(req["hl"].as_str().unwrap_or(""))?;
        if !install.dedicated && crate::sys::game_running() {
            return Err(CLOSE_GAME.into());
        }
        // a server on this computer that's running has its files open (Windows won't
        // let them be replaced, and a server shouldn't change under its players anyway)
        if install.dedicated && crate::servers::running_at(&install.hl) {
            return Err("Stop the server first, then try again.".into());
        }
        let comps = req.get("components").filter(|c| c.is_array()).map(list);
        let plan = setup::plan_uninstall(install, &list(&req["games"]), comps, &res)?;
        let store = store_for(target, &plan.install.hl, &plan.games[0]);
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
        .route("/api/setup/running", get(running))
}

/// The app window only: installing and removing change the game's files, so a phone
/// can watch (scan, job) but not start them.
pub fn local_routes() -> Router<Shared> {
    Router::new()
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
            token: RwLock::new("testtoken0123456789".into()),
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

// ---- servers' updates -----------------------------------------------------------------
//
// After an app update, each server's Loner-Strike parts (its plugins, the bot brain, mods
// with our fixes) are older than the app's. They go in by themselves once nobody is playing
// (bots don't count): a stopped server's files at once, a running one stopped, updated and
// started again while it's empty. Or now, from the servers list, asking first if people are on.
// (The plumbing, its service and shared control, goes in at its next start without this.)

/// The servers being updated now, and when each failed last (tried again an hour later)
static UPDATING: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
static FAILED_AT: Mutex<Option<std::collections::HashMap<String, std::time::Instant>>> = Mutex::new(None);
/// Servers asked to update now while another was: in order (one at a time)
static QUEUED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Whether this server's updates are going in now.
pub fn updating(id: &str) -> bool {
    UPDATING.lock().unwrap().as_ref().is_some_and(|s| s.contains(id))
}

/// A server's Loner-Strike parts older than this app's (none for one Loner-Strike didn't set up).
pub fn server_outdated(res: &Path, e: &crate::servers::Entry) -> Vec<String> {
    if !e.ready {
        return vec![];
    }
    let rec = setup::record::Record::open(&e.dir.join(&e.game));
    if !rec.data.components.contains_key("core") {
        return vec![];
    }
    setup::outdated(res, &rec)
}

/// People (not bots) on a running server now; None when it doesn't answer.
fn people_on(e: &crate::servers::Entry) -> Option<u32> {
    let (info, _) = crate::browser::info(&e.join_address())?;
    Some(u32::from(info.players.saturating_sub(info.bots)))
}

/// Puts the server's outdated parts in (a setup job): on a Linux machine with a new bot
/// brain, the machine's own update (it takes the Linux build); otherwise the parts again.
fn update_server(res: &Path, e: &crate::servers::Entry, outdated: &[String]) -> Result<String, String> {
    if e.remote.is_some() && outdated.iter().any(|c| c == "foundation") {
        return crate::servers::update_files(e.clone(), Some(res.to_path_buf()));
    }
    let install = find_install(&e.dir.to_string_lossy())?;
    let comps: Vec<String> = outdated.iter().filter(|c| *c != "core" && *c != "foundation").cloned().collect();
    let plan = setup::plan_install(install, &[e.game.clone()], &comps, res, true)?;
    let store = store_for(crate::servers::game_for(&e.id, || None), &plan.install.hl, &plan.games[0]);
    setup::start_install(plan, res.to_path_buf(), Some(store))
}

/// Stops it if it's running, puts its updates in, waits for that, and starts it again if it
/// was running. On its own thread; the server is marked as updating meanwhile.
fn run_update(res: PathBuf, id: String) {
    UPDATING.lock().unwrap().get_or_insert_with(Default::default).insert(id.clone());
    std::thread::spawn(move || {
        let outcome = (|| -> Result<(), String> {
            let e = crate::servers::get(&id).ok_or("That server isn't there any more.")?;
            let outdated = server_outdated(&res, &e);
            if outdated.is_empty() {
                return Ok(());
            }
            let was_running = crate::servers::view(&e)["state"] == "running";
            if was_running {
                crate::servers::stop(&id);
                for _ in 0..60 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    if crate::servers::view(&e)["state"] != "running" {
                        break;
                    }
                }
            }
            let job = update_server(&res, &e, &outdated);
            if let Ok(job) = &job {
                // (a machine's update takes minutes: up to 45 of them)
                for _ in 0..(45 * 60 / 2) {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let now = setup::current();
                    if now.id != *job || now.state != "running" {
                        break;
                    }
                }
            }
            // started again whatever happened: a server that was running stays running
            if was_running {
                crate::servers::start(&id)?;
            }
            let job = job?;
            let done = setup::current();
            if done.id == job && done.state == "error" {
                return Err(done.error.unwrap_or_else(|| "The update didn't finish.".into()));
            }
            Ok(())
        })();
        if let Err(why) = outcome {
            crate::diagnostics::log_error("server update", &format!("{id}: {why}"));
            FAILED_AT.lock().unwrap().get_or_insert_with(Default::default).insert(id.clone(), std::time::Instant::now());
        }
        UPDATING.lock().unwrap().get_or_insert_with(Default::default).remove(&id);
        // the next one asked for meanwhile
        let next = {
            let mut q = QUEUED.lock().unwrap();
            (!q.is_empty()).then(|| q.remove(0))
        };
        if let Some(next) = next {
            run_update(res, next);
        }
    });
}

/// The servers list's "Update now": at once if nobody's on (or the person said so), else
/// the number of people playing, for the app to ask about.
pub fn apply_updates(st: &Shared, id: &str, force: bool) -> Result<Value, String> {
    let res = resources(st)?;
    let e = crate::servers::get(id).ok_or("That server isn't there any more.")?;
    if updating(id) || QUEUED.lock().unwrap().iter().any(|q| q == id) {
        return Ok(json!({"ok": true}));
    }
    if server_outdated(&res, &e).is_empty() {
        return Err("It's up to date.".into());
    }
    if !force && crate::servers::view(&e)["state"] == "running" {
        let people = people_on(&e).unwrap_or(0);
        if people > 0 {
            return Ok(json!({"ok": false, "people": people}));
        }
    }
    // another server updating, or other setup running: this one goes next
    let busy = UPDATING.lock().unwrap().as_ref().is_some_and(|s| !s.is_empty()) || setup::current().state == "running";
    if busy {
        QUEUED.lock().unwrap().push(id.to_string());
        return Ok(json!({"ok": true, "queued": true}));
    }
    run_update(res, id.to_string());
    Ok(json!({"ok": true}))
}

/// Every few minutes: one server with updates waiting, its updates on, and nobody playing
/// (stopped, or only bots), gets them. Never while a setup job runs.
pub fn auto_update_loop(st: Shared) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(5 * 60));
            let Ok(res) = resources(&st) else { continue };
            if setup::current().state == "running" || UPDATING.lock().unwrap().as_ref().is_some_and(|s| !s.is_empty()) {
                continue;
            }
            for e in crate::servers::load() {
                if e.auto_update == Some(false) || server_outdated(&res, &e).is_empty() {
                    continue;
                }
                let failed = FAILED_AT.lock().unwrap().as_ref().and_then(|m| m.get(&e.id).copied());
                if failed.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(3600)) {
                    continue;
                }
                let state = crate::servers::view(&e)["state"].as_str().unwrap_or("").to_string();
                // (setting up, failed, or a machine not answering: not now)
                if state != "running" && state != "stopped" {
                    continue;
                }
                if state == "running" && people_on(&e) != Some(0) {
                    continue;
                }
                run_update(res.clone(), e.id.clone());
                // (one at a time)
                break;
            }
        });
    });
}
