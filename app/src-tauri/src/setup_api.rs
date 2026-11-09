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
// (bots and HLTV don't count): a stopped server's files at once, a running one stopped,
// updated and started again while it's empty. Or now, from the servers list, asking first if
// people are on. (The plumbing, its service and shared control, goes in at its next start.)

/// The updater's state: the server being updated (one at a time, and whether it was running),
/// servers asked for meanwhile (in order, and whether the person said to go ahead with people
/// on), and when each failed last (tried again an hour later)
#[derive(Default)]
struct Updates {
    now: Option<(String, bool)>,
    queued: Vec<(String, bool)>,
    failed_at: std::collections::HashMap<String, std::time::Instant>,
}
static UPDATES: Mutex<Option<Updates>> = Mutex::new(None);

fn updates<T>(f: impl FnOnce(&mut Updates) -> T) -> T {
    f(UPDATES.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default))
}

/// Whether this server's updates are going in now, and whether it'll be started again after.
pub fn updating(id: &str) -> Option<bool> {
    updates(|u| u.now.as_ref().filter(|(n, _)| n == id).map(|(_, w)| *w))
}

/// Whether this server is waiting its turn for its updates.
pub fn queued(id: &str) -> bool {
    updates(|u| u.queued.iter().any(|(q, _)| q == id))
}

/// A server's Loner-Strike parts older than this app's (none for one Loner-Strike didn't set
/// up). The record is read again only when it changed (the list asks every few seconds).
pub fn server_outdated(res: &Path, e: &crate::servers::Entry) -> Vec<String> {
    static CACHE: Mutex<Option<std::collections::HashMap<PathBuf, (std::time::SystemTime, Vec<String>)>>> = Mutex::new(None);
    if !e.ready {
        return vec![];
    }
    let gdir = e.dir.join(&e.game);
    let stamp = std::fs::metadata(setup::record::Record::path_in(&gdir)).and_then(|m| m.modified()).ok();
    if let Some(t) = stamp {
        if let Some((at, v)) = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(&gdir).cloned()) {
            if at == t {
                return v;
            }
        }
    }
    let rec = setup::record::Record::open(&gdir);
    let v = if rec.data.components.contains_key("core") { setup::outdated(res, &rec) } else { vec![] };
    if let Some(t) = stamp {
        CACHE.lock().unwrap().get_or_insert_with(Default::default).insert(gdir, (t, v.clone()));
    }
    v
}

/// People (not bots, not HLTV) on a running server now; None when it doesn't answer.
fn people_on(e: &crate::servers::Entry) -> Option<u32> {
    let (info, _) = crate::browser::info(&e.join_address())?;
    let hltv = u8::from(e.hltv.enabled);
    Some(u32::from(info.players.saturating_sub(info.bots).saturating_sub(hltv)))
}

/// Waits for a setup job to end (up to 45 minutes): its error, if it had one.
fn wait_job(job: &str) -> Result<(), String> {
    for _ in 0..(45 * 60 / 2) {
        let now = setup::current();
        if now.id != job {
            // (another job took its place: this one ended before it was seen)
            return Ok(());
        }
        match now.state.as_str() {
            "running" => std::thread::sleep(std::time::Duration::from_secs(2)),
            "error" => return Err(now.error.unwrap_or_else(|| "The update didn't finish.".into())),
            _ => return Ok(()),
        }
    }
    Err("The update was still going after 45 minutes.".into())
}

/// Puts a server's updates in, the server stopped: its parts again in its copy here (setup's
/// own install, repair), and with a new bot brain the engine side too (on a Linux machine its
/// machine update, which also sends the copy up; here, the engine files). A Linux server's
/// copy goes up before it's started again.
fn update_server(res: &Path, e: &crate::servers::Entry, outdated: &[String]) -> Result<(), String> {
    let install = find_install(&e.dir.to_string_lossy())?;
    let comps: Vec<String> = outdated.iter().filter(|c| *c != "core" && *c != "foundation").cloned().collect();
    let plan = setup::plan_install(install, &[e.game.clone()], &comps, res, true)?;
    let store = store_for(crate::servers::game_for(&e.id, || None), &plan.install.hl, &plan.games[0]);
    wait_job(&setup::start_install(plan, res.to_path_buf(), Some(store))?)?;
    if outdated.iter().any(|c| c == "foundation") {
        wait_job(&crate::servers::update_files(e.clone(), Some(res.to_path_buf()))?)?;
    } else if e.remote.is_some() {
        crate::servers::sync(e, false)?;
    }
    Ok(())
}

/// The updater for one server, on its own thread: checks it can go ahead, stops it if it's
/// running, puts the updates in, starts it again if it was running. False: another server's
/// update is going (asked again later).
fn run_update(res: PathBuf, id: String) -> bool {
    let Some(e) = crate::servers::get(&id) else { return true };
    let was_running = crate::servers::view(&e)["state"] == "running";
    // (claimed under the one lock: never two at once)
    if !updates(|u| {
        if u.now.is_some() {
            return false;
        }
        u.now = Some((id.clone(), was_running));
        u.queued.retain(|(q, _)| *q != id);
        true
    }) {
        return false;
    }
    std::thread::spawn(move || {
        // whatever happens (even a panic), the server isn't left marked as updating, and the
        // next one asked for goes
        struct Done(PathBuf);
        impl Drop for Done {
            fn drop(&mut self) {
                updates(|u| u.now = None);
                drain_queue(&self.0);
            }
        }
        let _done = Done(res.clone());
        let outcome = (|| -> Result<(), String> {
            let outdated = server_outdated(&res, &e);
            if outdated.is_empty() {
                return Ok(());
            }
            if crate::servers::unreachable(&id).is_some() {
                return Err("its machine isn't answering".into());
            }
            if setup::current().state == "running" {
                return Err("other setup was running".into());
            }
            // (it can go ahead, before anything is stopped)
            find_install(&e.dir.to_string_lossy())?;
            if was_running {
                crate::servers::stop(&id);
                let mut stopped = false;
                for _ in 0..60 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    if crate::servers::view(&e)["state"] != "running" {
                        stopped = true;
                        break;
                    }
                }
                if !stopped {
                    return Err("it didn't stop".into());
                }
            }
            let put_in = update_server(&res, &e, &outdated);
            // started again whatever happened, unless someone stopped it meanwhile on purpose
            // (it was running; the person's Stop marks it so in the servers list)
            if was_running {
                updates(|u| {
                    if let Some(n) = u.now.as_mut() {
                        n.1 = false;
                    }
                });
                crate::servers::start(&id)?;
            }
            put_in?;
            // (still the same versions after a run that said it worked: counted as failed, so
            // it isn't tried every few minutes)
            if server_outdated(&res, &e) == outdated {
                return Err("its parts were still the old versions afterwards".into());
            }
            Ok(())
        })();
        if let Err(why) = outcome {
            crate::diagnostics::log_error("server update", &format!("{}: {why}", e.name));
            updates(|u| u.failed_at.insert(id.clone(), std::time::Instant::now()));
        }
    });
    true
}

/// The next server asked for, if nothing is updating and no setup is running. One that wasn't
/// said to go ahead with people on is asked about again (someone may have joined): it waits.
fn drain_queue(res: &Path) {
    if setup::current().state == "running" {
        return;
    }
    let next = updates(|u| if u.now.is_none() && !u.queued.is_empty() { Some(u.queued[0].clone()) } else { None });
    let Some((id, force)) = next else { return };
    let Some(e) = crate::servers::get(&id) else {
        updates(|u| u.queued.retain(|(q, _)| *q != id));
        return;
    };
    if !force && crate::servers::view(&e)["state"] == "running" && people_on(&e) != Some(0) {
        return;
    }
    run_update(res.to_path_buf(), id);
}

/// The servers list's "Update now": at once if nobody's on (or the person said so), queued if
/// another update or setup is going, else the number of people playing for the app to ask about
/// ("people": null when the server didn't say).
pub fn apply_updates(st: &Shared, id: &str, force: bool) -> Result<Value, String> {
    let res = resources(st)?;
    let e = crate::servers::get(id).ok_or("That server isn't there any more.")?;
    if updating(id).is_some() || queued(id) {
        return Ok(json!({"ok": true, "queued": queued(id)}));
    }
    if server_outdated(&res, &e).is_empty() {
        return Err("It's up to date.".into());
    }
    if crate::servers::unreachable(id).is_some() {
        return Err(format!("{} isn't answering, so its updates wait until it is.", e.name));
    }
    if !force && crate::servers::view(&e)["state"] == "running" {
        match people_on(&e) {
            Some(0) => {}
            people => return Ok(json!({"ok": false, "people": people})),
        }
    }
    let busy = setup::current().state == "running" || !run_update(res, id.to_string());
    if busy {
        updates(|u| u.queued.push((id.to_string(), force)));
        return Ok(json!({"ok": true, "queued": true}));
    }
    Ok(json!({"ok": true}))
}

/// Every few minutes: the queue first, then one server with updates waiting, its updates on,
/// reachable, and nobody playing (stopped, or only bots) gets them. Never while setup runs.
pub fn auto_update_loop(st: Shared) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(5 * 60));
            let Ok(res) = resources(&st) else { continue };
            if setup::current().state == "running" || updates(|u| u.now.is_some()) {
                continue;
            }
            drain_queue(&res);
            if updates(|u| u.now.is_some()) {
                continue;
            }
            for e in crate::servers::load() {
                if e.auto_update == Some(false) || server_outdated(&res, &e).is_empty() || crate::servers::unreachable(&e.id).is_some() {
                    continue;
                }
                let failed = updates(|u| u.failed_at.get(&e.id).copied());
                if failed.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(3600)) {
                    continue;
                }
                let state = crate::servers::view(&e)["state"].as_str().unwrap_or("").to_string();
                // (setting up, failed or checking: not now)
                if state != "running" && state != "stopped" {
                    continue;
                }
                if state == "running" && people_on(&e) != Some(0) {
                    continue;
                }
                if run_update(res.clone(), e.id.clone()) {
                    break;
                }
            }
        });
    });
}
