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
        let held = held_server(&plan, &res);
        let store = store_for(target, &plan.install.hl, &plan.games[0]);
        match held {
            Some(e) => crate::servers::hold_engine_update_for(&e, || setup::start_install(plan, res, Some(store))),
            None => setup::start_install(plan, res, Some(store)),
        }
    })
    .await;
    started(r)
}

/// A server's copy here getting a new bot brain from setup itself (the Files bar's update):
/// its engine side (on a Linux machine, the machine) only follows with the server's own update
/// (update_server), so it's marked as waiting for that (servers::hold_engine_update_for): the
/// server to mark, if there's one.
fn held_server(plan: &setup::InstallPlan, res: &Path) -> Option<crate::servers::Entry> {
    if !plan.install.dedicated {
        return None;
    }
    let e = crate::servers::load().into_iter().find(|e| e.ready && e.dir == plan.install.hl && plan.games.contains(&e.game))?;
    let rec = setup::record::Record::open(&e.dir.join(&e.game));
    (rec.installed("foundation") && setup::outdated(res, &rec).iter().any(|c| c == "foundation")).then_some(e)
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

    /// A server whose updates keep failing isn't stopped every hour: 6 hours, a day, then only
    /// Update now; another app version's failures don't count.
    #[test]
    fn failed_updates_back_off() {
        use super::{parse_failures, retry_due};
        let v = env!("CARGO_PKG_VERSION");
        assert_eq!(parse_failures(&format!("2 1000 {v}\n")), Some((2, 1000)));
        assert_eq!(parse_failures("2 1000 0.0.0-old\n"), None);
        assert_eq!(parse_failures("nonsense"), None);
        let h = 3600;
        assert!(retry_due(None, 0));
        assert!(!retry_due(Some((1, 1000)), 1000 + h));
        assert!(retry_due(Some((1, 1000)), 1000 + 6 * h));
        assert!(!retry_due(Some((2, 1000)), 1000 + 6 * h));
        assert!(retry_due(Some((2, 1000)), 1000 + 24 * h));
        assert!(!retry_due(Some((3, 1000)), 1000 + 1000 * h));
        // (a clock put back doesn't make it due)
        assert!(!retry_due(Some((1, 1000)), 10));
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
/// on), and when each failed last (tried again an hour later at the soonest; one that failed
/// after it was stopped or while its updates went in waits longer, see retry_due)
#[derive(Default)]
struct Updates {
    now: Option<(String, bool)>,
    /// what the one going now is doing ("stopping", "installing", "uploading", "starting"), and
    /// the setup job it started (the progress shown is that job's, never another's)
    phase: String,
    job: String,
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

/// What the server updating now is doing, and the setup job it's in.
pub fn update_phase(id: &str) -> Option<(String, String)> {
    updates(|u| u.now.as_ref().filter(|(n, _)| n == id).map(|_| (u.phase.clone(), u.job.clone())))
}

fn set_phase(phase: &str, job: Option<&str>) {
    updates(|u| {
        u.phase = phase.to_string();
        if let Some(j) = job {
            u.job = j.to_string();
        }
    })
}

/// Whether this server is waiting its turn for its updates.
pub fn queued(id: &str) -> bool {
    updates(|u| u.queued.iter().any(|(q, _)| q == id))
}

/// A server whose updates failed once tried for real (it was stopped, or they went in and
/// didn't work): how many times in a row, when last (Unix seconds) and the app version that
/// tried, in its folder beside the game (never uploaded), so an app restart doesn't try again at
/// once. Cleared when its updates go in.
fn failed_path(e: &crate::servers::Entry) -> PathBuf {
    e.dir.join(".update-failed")
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// (failures, when) from the file; nothing from another app version's (a newer app may have
/// fixed what went wrong: its own tries start again)
fn parse_failures(text: &str) -> Option<(u32, u64)> {
    let mut w = text.split_whitespace();
    let n = w.next()?.parse().ok()?;
    let at = w.next()?.parse().ok()?;
    (w.next() == Some(env!("CARGO_PKG_VERSION"))).then_some((n, at))
}

fn update_failures(e: &crate::servers::Entry) -> Option<(u32, u64)> {
    parse_failures(&std::fs::read_to_string(failed_path(e)).ok()?)
}

fn note_update_failed(e: &crate::servers::Entry) {
    let n = update_failures(e).map_or(0, |(n, _)| n).saturating_add(1);
    let _ = crate::sys::write_atomic(&failed_path(e), format!("{n} {} {}\n", unix_now(), env!("CARGO_PKG_VERSION")).as_bytes());
}

/// Whether the updater may try a server again by itself (Update now always can): 6 hours after
/// its first failure, a day after its second, and after the third not by itself any more (each
/// try can stop a server with bots on and keep it down while the update runs).
fn retry_due(failures: Option<(u32, u64)>, now: u64) -> bool {
    let wait = match failures {
        None | Some((0, _)) => return true,
        Some((1, _)) => 6 * 3600,
        Some((2, _)) => 24 * 3600,
        Some(_) => return false,
    };
    failures.is_some_and(|(_, at)| now.saturating_sub(at) >= wait)
}

/// Whether the updater stopped trying this server by itself (it failed too often).
pub fn auto_update_stopped(e: &crate::servers::Entry) -> bool {
    update_failures(e).is_some_and(|(n, _)| n >= 3)
}

/// A server's Loner-Strike parts older than this app's (none for one Loner-Strike didn't set
/// up). The record is read again only when it changed (the list asks every few seconds).
pub fn server_outdated(res: &Path, e: &crate::servers::Entry) -> Vec<String> {
    let mut v = record_outdated(res, e);
    // its copy here has the new bot brain but the engine side didn't follow (an update that
    // stopped part way, or one put in from the Files bar): offered again until it does
    if !v.iter().any(|c| c == "foundation") && e.ready && crate::servers::engine_update_pending(e) {
        v.push("foundation".into());
    }
    v
}

/// server_outdated from its install record alone.
fn record_outdated(res: &Path, e: &crate::servers::Entry) -> Vec<String> {
    static CACHE: Mutex<Option<std::collections::HashMap<PathBuf, ((std::time::SystemTime, u64), Vec<String>)>>> = Mutex::new(None);
    if !e.ready {
        return vec![];
    }
    let gdir = e.dir.join(&e.game);
    // (its time and size; one written in the last two seconds isn't kept, as a coarse clock
    // could give the next save the same time)
    let stamp = std::fs::metadata(setup::record::Record::path_in(&gdir))
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
        .filter(|(t, _)| t.elapsed().is_ok_and(|e| e > std::time::Duration::from_secs(2)));
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

/// People (not bots, not HLTV) on a running server now; None when it doesn't answer. From
/// the game's own list (status: a person has a Steam or Valve ID, bots BOT, HLTV HLTV); if
/// RCON doesn't answer, the server browser's count (players less bots: HLTV counted as a person).
fn people_on(e: &crate::servers::Entry) -> Option<u32> {
    if let Some(status) = e.rcon().and_then(|rc| rc.send("status").ok()) {
        let ids: Vec<&str> = status
            .lines()
            .filter(|l| l.trim_start().starts_with('#'))
            .filter_map(|l| l.rsplit_once('"').and_then(|(_, rest)| rest.split_whitespace().nth(1)))
            .collect();
        if status.contains("players :") || !ids.is_empty() {
            return Some(ids.iter().filter(|id| !matches!(**id, "BOT" | "HLTV")).count() as u32);
        }
    }
    let (info, _) = crate::browser::info(&e.join_address())?;
    Some(u32::from(info.players.saturating_sub(info.bots)))
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
    // a new bot brain: noted before the copy here changes, so if the engine side (the machine)
    // doesn't follow, the plugins for it aren't sent there and the update is offered again
    let store = store_for(crate::servers::game_for(&e.id, || None), &plan.install.hl, &plan.games[0]);
    let start = || setup::start_install(plan, res.to_path_buf(), Some(store));
    let job = if outdated.iter().any(|c| c == "foundation") { crate::servers::hold_engine_update_for(e, start)? } else { start()? };
    set_phase("installing", Some(&job));
    wait_job(&job)?;
    if outdated.iter().any(|c| c == "foundation") {
        let job = crate::servers::update_files(e.clone(), Some(res.to_path_buf()))?;
        set_phase(if e.remote.is_some() { "machine" } else { "engine" }, Some(&job));
        wait_job(&job)?;
    } else if e.remote.is_some() {
        set_phase("uploading", None);
        crate::servers::sync(e, false)?;
    }
    Ok(())
}

/// The updater for one server, on its own thread: checks it can go ahead, stops it if it's
/// running, puts the updates in, starts it again if it was running. False: another server's
/// update is going (asked again later).
fn run_update(res: PathBuf, id: String) -> bool {
    let Some(e) = crate::servers::get(&id) else { return true };
    let state = crate::servers::view(&e)["state"].as_str().unwrap_or("").to_string();
    // (a machine not heard from yet: whether it's running isn't known, so not now)
    if state != "running" && state != "stopped" && state != "crashed" {
        return false;
    }
    let was_running = state == "running";
    // (claimed under the one lock: never two at once)
    if !updates(|u| {
        if u.now.is_some() {
            return false;
        }
        u.now = Some((id.clone(), was_running));
        u.phase = if was_running { "stopping".into() } else { "preparing".into() };
        u.job.clear();
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
        // (past the checks: the server was stopped or its updates started going in)
        let mut tried = false;
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
            tried = true;
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
                    // (it may still stop a moment later: started again, which does nothing if
                    // it's still up)
                    let _ = crate::servers::start(&id);
                    return Err("it didn't stop".into());
                }
            }
            let put_in = update_server(&res, &e, &outdated);
            // started again whatever happened, as it was running
            if was_running {
                updates(|u| {
                    if let Some(n) = u.now.as_mut() {
                        n.1 = false;
                    }
                    u.phase = "starting".into();
                    u.job.clear();
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
        match outcome {
            Ok(()) => {
                let _ = std::fs::remove_file(failed_path(&e));
            }
            Err(why) => {
                crate::diagnostics::log_error("server update", &format!("{}: {why}", e.name));
                updates(|u| u.failed_at.insert(id.clone(), std::time::Instant::now()));
                // (a check that said not now isn't counted: nothing was stopped or changed)
                if tried {
                    note_update_failed(&e);
                }
            }
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
    if updates(|u| u.now.is_some()) {
        return;
    }
    // (the first that can go: one with people on that wasn't forced waits, the rest don't)
    let queue = updates(|u| u.queued.clone());
    for (id, force) in queue {
        let Some(e) = crate::servers::get(&id) else {
            updates(|u| u.queued.retain(|(q, _)| *q != id));
            continue;
        };
        if !force && crate::servers::view(&e)["state"] == "running" && people_on(&e) != Some(0) {
            continue;
        }
        if run_update(res.to_path_buf(), id) {
            return;
        }
    }
}

/// The servers list's "Update now": at once if nobody's on (or the person said so), queued if
/// another update or setup is going, else the number of people playing for the app to ask about
/// ("people": null when the server didn't say).
pub fn apply_updates(st: &Shared, id: &str, force: bool) -> Result<Value, String> {
    let res = resources(st)?;
    let e = crate::servers::get(id).ok_or("That server isn't there any more.")?;
    if updating(id).is_some() {
        return Ok(json!({"ok": true}));
    }
    // (asked again with the go-ahead: the queued one gets it)
    if queued(id) {
        if force {
            updates(|u| u.queued.iter_mut().filter(|(q, _)| q == id).for_each(|(_, f)| *f = true));
        }
        return Ok(json!({"ok": true, "queued": true}));
    }
    let state = crate::servers::view(&e)["state"].as_str().unwrap_or("").to_string();
    if state != "running" && state != "stopped" && state != "crashed" {
        return Err(format!("{} isn't ready for that yet. Try again in a moment.", e.name));
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
                // (failed after it was stopped: much later, then not by itself; see retry_due)
                if !retry_due(update_failures(&e), unix_now()) {
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
