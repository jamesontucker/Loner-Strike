//! Smarter chat (experimental): a small model on this computer writes bots' replies when
//! the player talks to them, and short back-and-forths between bots. Game events keep
//! their written lines (gm_botchat.sma, botchat.ini).
//!
//! - catalog.rs: the models and the llama.cpp build, pinned by checksum
//! - runtime.rs: downloading, unpacking, running llama-server on 127.0.0.1
//! - reply.rs: the prompt, and the filter that drops anything that reads like an assistant
//!
//! llama-server only runs while the feature is on and the player's own game is hosting a
//! map (or while the Try it sheet is open), and stops when that ends or the app quits.
//! When it's ready, the game's cvar gm_ai_url says where to ask ("127.0.0.1:<app port>");
//! the plugin POSTs to /api/botchat/reply there, and gets {"text"} or 204 (use a written line).

pub mod catalog;
pub mod reply;
pub mod runtime;

use crate::app_api::{edit_settings, load_settings};
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use catalog::{Model, MODELS};
use reply::ReplyReq;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a reply may take inside llama-server (the plugin waits about 5 s in all).
const GEN_TIMEOUT: Duration = Duration::from_millis(4000);
/// How long loading a model may take before it counts as not working.
const LOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// The Try it sheet keeps the server up while it's open; it says so every 20 s.
const TRIAL_KEEPALIVE: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Runner {
    server: Option<runtime::Server>,
    started: Option<Instant>,
    ready: bool,
    /// the model that failed to run, and why (not retried until something changes)
    failed: Option<(String, String)>,
    /// the Try it sheet: its model and when it last said it's still open
    trial: Option<(&'static Model, Instant)>,
    /// what gm_ai_url was last seen or set to in each game, and when it was checked
    cvar: std::collections::HashMap<String, (String, Instant)>,
    /// whether a game was being hosted at the last check
    game_on: bool,
}

static RUN: Mutex<Option<Runner>> = Mutex::new(None);

fn with_run<T>(f: impl FnOnce(&mut Runner) -> T) -> T {
    let mut g = RUN.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Runner::default))
}

// ---- downloads ----------------------------------------------------------------------

#[derive(Clone)]
struct Download {
    id: String,
    got: u64,
    total: u64,
    done: bool,
    error: Option<String>,
}

static DOWNLOAD: Mutex<Option<Download>> = Mutex::new(None);

fn download_json() -> Value {
    match DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(d) => json!({"id": d.id, "percent": if d.total > 0 { (d.got * 100 / d.total).min(100) } else { 0 }, "done": d.done, "error": d.error}),
        None => Value::Null,
    }
}

fn start_download(m: &'static Model) -> Result<(), String> {
    {
        let mut d = DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner());
        if d.as_ref().is_some_and(|d| !d.done) {
            return Err("Another download is still going. Try again when it's done.".into());
        }
        let rt = if runtime::server_exe().is_some() { 0 } else { catalog::runtime().map(|r| r.size).unwrap_or(0) };
        *d = Some(Download { id: m.id.into(), got: 0, total: m.size + rt, done: false, error: None });
    }
    std::thread::spawn(move || {
        let set = |f: &dyn Fn(&mut Download)| {
            if let Some(d) = DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                f(d);
            }
        };
        let result = (|| {
            runtime::ensure_server(&|n| set(&|d| d.got = n))?;
            let base = DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|d| d.total - m.size).unwrap_or(0);
            if !runtime::has_model(m) {
                runtime::download(m.url, m.sha256, &runtime::model_path(m), m.name, &|n| set(&|d| d.got = base + n))?;
            }
            Ok::<(), String>(())
        })();
        set(&|d| {
            d.done = true;
            d.got = if result.is_ok() { d.total } else { d.got };
            d.error = result.clone().err();
        });
        // the first model downloaded is the one used
        if result.is_ok() {
            edit_settings(|s| {
                if s.smart_chat_model.as_deref().and_then(catalog::model).map_or(true, |x| !runtime::has_model(x)) {
                    s.smart_chat_model = Some(m.id.into());
                }
            });
        }
    });
    Ok(())
}

// ---- running llama-server -----------------------------------------------------------

/// The model the feature uses: the chosen one, if it's downloaded.
pub(crate) fn chosen() -> Option<&'static Model> {
    load_settings().smart_chat_model.as_deref().and_then(catalog::model).filter(|m| runtime::has_model(m))
}

type Hosted = Vec<(String, std::sync::Arc<crate::game::Game>)>;

/// Games on this computer hosting a map: the player's own game and dedicated servers
/// running here. Never a server on another machine (127.0.0.1 there is that machine).
fn hosting(st: &Shared) -> Hosted {
    let mut out = Vec::new();
    if let Some(g) = st.local_game().filter(|g| !g.dedicated && g.remote.is_none() && g.state()["online"] == true) {
        out.push(("local".to_string(), g));
    }
    for e in crate::servers::load().into_iter().filter(|e| e.ready && e.remote.is_none() && crate::servers::running(&e.id)) {
        if let Some(g) = crate::servers::game_for(&e.id, || None).filter(|g| g.remote.is_none() && g.state()["online"] == true) {
            out.push((e.id.clone(), g));
        }
    }
    out
}

/// "127.0.0.1:<port>": where the plugin reaches this app.
fn app_addr(st: &Shared) -> Option<String> {
    st.hosts.read().unwrap().iter().find(|h| h.starts_with("127.0.0.1:")).cloned()
}

/// Starts, checks or stops llama-server to match what's wanted now, and tells the game.
/// Called every couple of seconds and right after a change.
pub fn tick(st: &Shared) {
    let settings = load_settings();
    let games = if settings.smart_chat { hosting(st) } else { Vec::new() };
    let mut want: Option<&'static Model> = None;
    let exe = runtime::server_exe();
    with_run(|r| {
        if r.trial.is_some_and(|(_, at)| at.elapsed() > TRIAL_KEEPALIVE) {
            r.trial = None;
        }
        // a new game is a new try for a model that didn't work last time
        if !games.is_empty() != r.game_on {
            r.game_on = !games.is_empty();
            if r.game_on {
                r.failed = None;
            }
        }
        want = r.trial.map(|(m, _)| m).or_else(|| if games.is_empty() { None } else { chosen() });
        if want.is_some_and(|m| r.failed.as_ref().is_some_and(|(id, _)| id == m.id)) {
            want = None;
        }
        // stop what's not wanted (or is the wrong model)
        if r.server.as_ref().is_some_and(|s| want.map_or(true, |m| m.id != s.model.id)) {
            if let Some(s) = r.server.take() {
                s.stop();
            }
            r.ready = false;
        }
        // check on what's running
        if let Some(s) = r.server.as_mut() {
            if let Err(why) = s.alive() {
                r.failed = Some((s.model.id.into(), why));
                r.server = None;
                r.ready = false;
            } else if !r.ready {
                if s.healthy() {
                    r.ready = true;
                } else if r.started.is_some_and(|t| t.elapsed() > LOAD_TIMEOUT) {
                    r.failed = Some((s.model.id.into(), "The model took too long to load. Try a smaller one.".into()));
                    if let Some(s) = r.server.take() {
                        s.stop();
                    }
                }
            }
        }
        if let (None, Some(m)) = (&r.server, want) {
            if r.failed.is_none() {
                match exe.as_ref().ok_or_else(|| "llama.cpp isn't downloaded. Download a model again.".to_string()).and_then(|e| runtime::Server::start(e, m)) {
                    Ok(s) => {
                        r.server = Some(s);
                        r.started = Some(Instant::now());
                        r.ready = false;
                    }
                    Err(e) => r.failed = Some((m.id.into(), e)),
                }
            }
        }
    });
    with_run(|r| r.cvar.retain(|k, _| games.iter().any(|(id, _)| id == k)));
    for (id, g) in &games {
        sync_cvar(st, id, g, settings.smart_chat);
    }
}

/// gm_ai_url in the game: "127.0.0.1:<app port>" while replies can be written, else "".
/// Asked every 10 s (a map change or a plugin reload may reset it) or when it should change.
fn sync_cvar(st: &Shared, id: &str, g: &crate::game::Game, on: bool) {
    let ready = with_run(|r| r.ready);
    let want = if on && ready { app_addr(st).unwrap_or_default() } else { String::new() };
    let fresh = with_run(|r| r.cvar.get(id).is_some_and(|(v, at)| *v == want && at.elapsed() < Duration::from_secs(10)));
    if fresh {
        return;
    }
    let Ok(reply) = g.rcon.send("gm_ai_url") else { return };
    // "gm_ai_url" is "127.0.0.1:8027"; an older plugin without it says Unknown command
    let Some(now) = reply.split_once("\"gm_ai_url\" is \"").and_then(|(_, r)| r.split('"').next()).map(String::from) else {
        with_run(|r| r.cvar.insert(id.to_string(), (want, Instant::now())));
        return;
    };
    if now != want {
        let _ = g.rcon.send(&format!("gm_ai_url \"{want}\""));
    }
    with_run(|r| r.cvar.insert(id.to_string(), (want, Instant::now())));
}

/// Every 2 seconds, from app start. Costs nothing while the feature is off and the Try
/// it sheet is closed (no server, no game queries).
pub fn watch(st: Shared) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        let idle = !load_settings().smart_chat && with_run(|r| r.server.is_none() && r.trial.is_none());
        if !idle {
            tick(&st);
        }
    });
}

/// When the app quits: stop llama-server, and tell the game there's nothing to ask.
pub fn shutdown(st: &Shared) {
    let (server, told) = with_run(|r| (r.server.take(), r.cvar.iter().filter(|(_, (v, _))| !v.is_empty()).map(|(k, _)| k.clone()).collect::<Vec<_>>()));
    if let Some(s) = server {
        s.stop();
    }
    for id in told {
        let g = if id == "local" { st.local_game() } else { crate::servers::game_for(&id, || None) };
        if let Some(g) = g.filter(|g| g.remote.is_none()) {
            let _ = g.rcon.send("gm_ai_url \"\"");
        }
    }
}

fn status_json() -> Value {
    let s = load_settings();
    with_run(|r| {
        let (state, reason) = if let Some((id, why)) = &r.failed {
            ("failed", Some(format!("{}: {why}", catalog::model(id).map(|m| m.name).unwrap_or("The model"))))
        } else if r.server.is_some() && r.ready {
            ("ready", None)
        } else if r.server.is_some() {
            ("starting", None)
        } else if s.smart_chat && chosen().is_some() {
            ("waiting", None)
        } else {
            ("off", None)
        };
        json!({"state": state, "reason": reason, "running": r.server.as_ref().map(|s| s.model.id), "trial": r.trial.is_some()})
    })
}

// ---- writing a reply ------------------------------------------------------------------

/// Replies being written right now; more than this and the rest get a written line.
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

pub struct Written {
    pub text: Option<String>,
    /// something to do as well (one the plugin offered), only with a text
    pub action: Option<String>,
    /// what the model said, when the filter dropped it
    pub raw: Option<String>,
    pub ms: u128,
}

/// One reply from the running model (blocking: call from a worker thread).
fn write_reply(req: &ReplyReq) -> Result<Written, String> {
    let (port, model) = with_run(|r| match (&r.server, r.ready) {
        (Some(s), true) => Ok((s.port, s.model)),
        _ => Err("not ready".to_string()),
    })?;
    if IN_FLIGHT.fetch_add(1, Ordering::SeqCst) >= 4 {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        return Err("busy".into());
    }
    let res = generate(port, model, req);
    IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    res
}

/// Asks llama-server on `port` for one reply and filters it.
fn generate(port: u16, model: &Model, req: &ReplyReq) -> Result<Written, String> {
    let started = Instant::now();
    let (path, body) = reply::request_body(model.mode, req);
    let res = crate::sys::http().post(format!("http://127.0.0.1:{port}{path}")).timeout(GEN_TIMEOUT).json(&body).send();
    let v: Value = res.map_err(|e| e.to_string())?.json().map_err(|e| e.to_string())?;
    let raw = reply::answer_text(model.mode, &v).unwrap_or_default();
    // asked for {"say", "do"} (chat models, with actions offered): the line and the action
    let (said, action) = if model.mode == catalog::PromptMode::Chat && !req.actions.is_empty() {
        reply::split_action(&raw, &req.actions)
    } else {
        (raw.clone(), None)
    };
    let mut text = reply::clean_reply(&said, &req.bot, reply::is_loud(&req.persona));
    // parroting the line it's answering isn't a reply; with "Bots can be rude" off, an insult
    // or swearing isn't either (the written line goes instead)
    if text.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(req.message.trim()) || (!req.rude && reply::has_insult(t))) {
        text = None;
    }
    let action = action.filter(|_| text.is_some());
    let raw = text.is_none().then(|| reply::plain(&raw, 120));
    Ok(Written { text, action, raw, ms: started.elapsed().as_millis() })
}

/// POST /api/botchat/reply, for the plugin. 200 {"text"} or 204 when there's nothing good
/// (feature off, model not ready, rejected, too slow): the plugin then uses a written line.
async fn api_reply(headers: HeaderMap, body: axum::body::Bytes) -> Response {
    // only the app's own pages or a JSON request (the plugin) get in: a web page can't send
    // either to 127.0.0.1 without the browser asking first, which nothing answers
    let json_type = headers.get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|v| v.to_ascii_lowercase().starts_with("application/json"));
    if !from_panel(&headers) && !json_type {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    if body.len() > 16 * 1024 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let Ok(req) = serde_json::from_slice::<ReplyReq>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "expected the reply JSON"}))).into_response();
    };
    if !load_settings().smart_chat {
        return StatusCode::NO_CONTENT.into_response();
    }
    let req = req.tidy();
    let job = tokio::task::spawn_blocking(move || write_reply(&req));
    match tokio::time::timeout(Duration::from_millis(4200), job).await {
        Ok(Ok(Ok(Written { text: Some(t), action: Some(a), .. }))) => (StatusCode::OK, Json(json!({"text": t, "action": a}))).into_response(),
        Ok(Ok(Ok(Written { text: Some(t), .. }))) => (StatusCode::OK, Json(json!({"text": t}))).into_response(),
        _ => StatusCode::NO_CONTENT.into_response(),
    }
}

// ---- the Bots page --------------------------------------------------------------------

async fn api_status() -> Json<Value> {
    let s = load_settings();
    let models: Vec<Value> = MODELS
        .iter()
        .map(|m| {
            json!({"id": m.id, "name": m.name, "size": m.size, "speed": m.speed, "good_at": m.good_at,
                   "credit": m.credit, "page": m.page, "mode": m.mode.key(), "downloaded": runtime::has_model(m)})
        })
        .collect();
    Json(json!({
        "enabled": s.smart_chat,
        "model": s.smart_chat_model,
        "models": models,
        "supported": catalog::runtime().is_some(),
        "download": download_json(),
        "status": status_json(),
    }))
}

fn forbidden() -> (StatusCode, Json<Value>) {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

#[derive(Deserialize)]
struct SetReq {
    enabled: Option<bool>,
    model: Option<String>,
}

async fn api_set(State(st): State<Shared>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<SetReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) {
        return forbidden();
    }
    if let Some(id) = &req.model {
        match catalog::model(id) {
            Some(m) if runtime::has_model(m) => {}
            Some(_) => return (StatusCode::OK, Json(json!({"error": "Download that model first."}))),
            None => return (StatusCode::OK, Json(json!({"error": "That model isn't in the list."}))),
        }
    }
    edit_settings(|s| {
        if let Some(v) = req.enabled {
            s.smart_chat = v;
        }
        if let Some(m) = &req.model {
            s.smart_chat_model = Some(m.clone());
        }
    });
    // a change is a new try
    with_run(|r| r.failed = None);
    let _ = tokio::task::spawn_blocking(move || tick(&st)).await;
    (StatusCode::OK, api_status().await)
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

async fn api_download(headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<IdReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) {
        return forbidden();
    }
    let Some(m) = catalog::model(&req.id) else { return (StatusCode::OK, Json(json!({"error": "That model isn't in the list."}))) };
    if catalog::runtime().is_none() {
        return (StatusCode::OK, Json(json!({"error": "Smarter chat doesn't run on this kind of computer yet."})));
    }
    match start_download(m) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

async fn api_remove(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<IdReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(m) = catalog::model(&req.id) else { return (StatusCode::OK, Json(json!({"error": "That model isn't in the list."}))) };
    if DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|d| d.id == m.id && !d.done) {
        return (StatusCode::OK, Json(json!({"error": "It's still downloading."})));
    }
    let res = tokio::task::spawn_blocking(move || {
        // stop it first if it's running (Windows can't delete an open file)
        let running = with_run(|r| {
            if r.trial.is_some_and(|(t, _)| t.id == m.id) {
                r.trial = None;
            }
            if r.server.as_ref().is_some_and(|s| s.model.id == m.id) {
                r.ready = false;
                r.server.take()
            } else {
                None
            }
        });
        if let Some(s) = running {
            s.stop();
        }
        let r = std::fs::remove_file(runtime::model_path(m));
        // nothing left to run: llama.cpp goes too
        if MODELS.iter().all(|x| !runtime::has_model(x)) {
            let _ = std::fs::remove_dir_all(runtime::dir());
        }
        edit_settings(|s| {
            if s.smart_chat_model.as_deref() == Some(m.id) {
                s.smart_chat_model = MODELS.iter().find(|x| runtime::has_model(x)).map(|x| x.id.to_string());
            }
        });
        tick(&st);
        r
    })
    .await;
    match res {
        Ok(Ok(())) | Ok(Err(_)) if !runtime::has_model(m) => (StatusCode::OK, Json(json!({"ok": true}))),
        _ => (StatusCode::OK, Json(json!({"error": "Couldn't remove it. Close anything using it and try again."}))),
    }
}

// ---- Try it -------------------------------------------------------------------------

/// Three sample bots, different personas and moods, for the Try it sheet.
const SAMPLES: &[(&str, &str, &str)] = &[("Gordon", "s", "tilted"), ("xX_Sn1p3r_Xx", "k", "cocky"), ("Dave", "D", "polite")];

#[derive(Deserialize)]
struct TryStart {
    model: Option<String>,
}

/// Opens (or keeps open) the Try it sheet's server.
async fn api_try_start(State(st): State<Shared>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<TryStart>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) {
        return forbidden();
    }
    let m = req.model.as_deref().and_then(catalog::model).filter(|m| runtime::has_model(m)).or_else(chosen);
    let Some(m) = m else { return (StatusCode::OK, Json(json!({"error": "Download a model first."}))) };
    let changed = with_run(|r| {
        let changed = r.trial.map_or(true, |(t, _)| t.id != m.id);
        if changed {
            r.failed = None;
        }
        r.trial = Some((m, Instant::now()));
        changed
    });
    if changed {
        let _ = tokio::task::spawn_blocking(move || tick(&st)).await;
    }
    (StatusCode::OK, Json(json!({"ok": true, "model": m.id, "status": status_json()})))
}

async fn api_try_stop(State(st): State<Shared>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) {
        return forbidden();
    }
    with_run(|r| r.trial = None);
    let _ = tokio::task::spawn_blocking(move || tick(&st)).await;
    (StatusCode::OK, Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct TryReq {
    message: String,
    #[serde(default)]
    map: Option<String>,
}

/// Replies from the three sample bots to one message, with how long each took.
async fn api_try(headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<TryReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) {
        return forbidden();
    }
    with_run(|r| {
        if let Some((_, at)) = r.trial.as_mut() {
            *at = Instant::now();
        }
    });
    let message = reply::plain(&req.message, 160);
    if message.is_empty() {
        return (StatusCode::OK, Json(json!({"error": "Type something first."})));
    }
    let map = req.map.map(|m| reply::plain(&m, 31)).filter(|m| !m.is_empty()).unwrap_or_else(|| "de_dust2".into());
    let jobs: Vec<_> = SAMPLES
        .iter()
        .map(|(bot, persona, mood)| {
            let r = ReplyReq {
                bot: bot.to_string(),
                persona: persona.to_string(),
                mood: mood.to_string(),
                kind: "reply".into(),
                message: message.clone(),
                human: "Player".into(),
                history: vec![],
                context: serde_json::from_value(json!({"map": map, "score": "4-6", "alive": true})).unwrap_or_default(),
                ..Default::default()
            }
            .tidy();
            tokio::task::spawn_blocking(move || (r.clone(), write_reply(&r)))
        })
        .collect();
    let mut out = Vec::new();
    for j in jobs {
        let Ok((r, res)) = j.await else { continue };
        let base = json!({"bot": r.bot, "persona": reply::persona_text(&r.persona), "mood": r.mood});
        let extra = match res {
            Ok(w) => json!({"text": w.text, "action": w.action, "rejected": w.raw, "ms": w.ms}),
            Err(e) if e == "not ready" => json!({"error": "The model isn't ready yet."}),
            Err(e) if e.contains("timed out") || e.contains("timeout") => json!({"error": "Too slow, so a written line would be used."}),
            Err(_) => json!({"error": "No answer from the model."}),
        };
        let mut v = base;
        if let (Value::Object(a), Value::Object(b)) = (&mut v, extra) {
            a.extend(b);
        }
        out.push(v);
    }
    (StatusCode::OK, Json(json!({"replies": out})))
}

/// The app window only: the plugin's endpoint (the game runs on this computer and asks
/// 127.0.0.1; phones never reach it), and removing a model.
pub fn local_routes() -> Router<Shared> {
    Router::new().route("/api/botchat/reply", post(api_reply)).route("/api/smartchat/remove", post(api_remove))
}

/// The app window and the owner's phone (never the sharing listener): the settings, getting
/// a model, and trying it.
pub fn owner_routes() -> Router<Shared> {
    Router::new()
        .route("/api/smartchat", get(api_status))
        .route("/api/smartchat/set", post(api_set))
        .route("/api/smartchat/download", post(api_download))
        .route("/api/smartchat/try/start", post(api_try_start))
        .route("/api/smartchat/try/stop", post(api_try_stop))
        .route("/api/smartchat/try", post(api_try))
}

#[cfg(test)]
mod tests {
    use crate::server::{self, AppState, Listener};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::{Arc, Mutex, OnceLock, RwLock};
    use tower::ServiceExt;

    fn state(host: &str) -> server::Shared {
        Arc::new(AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec![host.into()]),
            lan_hosts: RwLock::new(vec!["192.168.1.5:8028".into()]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        })
    }

    async fn post(listener: Listener, host: &str, ctype: Option<&str>, body: &str) -> StatusCode {
        let st = state("127.0.0.1:8027");
        let mut b = Request::builder().method("POST").uri("/api/botchat/reply").header("host", host);
        if let Some(c) = ctype {
            b = b.header("content-type", c);
        }
        let req = b.body(Body::from(body.to_string())).unwrap();
        server::router(st, listener).oneshot(req).await.unwrap().status()
    }

    #[tokio::test]
    async fn reply_endpoint_is_local_only() {
        let body = r#"{"bot":"Gordon","persona":"s","mood":"tilted","kind":"reply","message":"hi","human":"me","history":[],"context":{"map":"de_dust2","bomb":true}}"#;
        // from the game on this computer: answers (204 here: no model running in tests)
        let s = post(Listener::Local, "127.0.0.1:8027", Some("application/json"), body).await;
        assert!(s == StatusCode::NO_CONTENT || s == StatusCode::OK, "{s}");
        // a web page's plain form post can't use it
        assert_eq!(post(Listener::Local, "127.0.0.1:8027", Some("text/plain"), body).await, StatusCode::FORBIDDEN);
        // nor a page pretending to be another host
        assert_eq!(post(Listener::Local, "evil.example:8027", Some("application/json"), body).await, StatusCode::FORBIDDEN);
        // and the phone listener doesn't have it, even signed in
        let st = state("127.0.0.1:8027");
        let req = Request::builder()
            .method("POST")
            .uri("/api/botchat/reply?t=testtoken0123456789")
            .header("host", "192.168.1.5:8028")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        assert_eq!(server::router(st, Listener::Lan).oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
    }

    /// Runs every downloaded model on sample prompts and prints replies and timings.
    /// LONER_STRIKE_AI_DIR=<folder> cargo test live_models -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_models() {
        use super::*;
        let exe = runtime::ensure_server(&|_| {}).expect("llama-server");
        let prompts: &[(&str, &str, &str, &str, &[&str])] = &[
            ("Gordon", "s", "tilted", "are u a bot", &[]),
            ("xX_Sn1p3r_Xx", "k", "cocky", "nice shot", &["Gordon: ns", "xX_Sn1p3r_Xx: ez"]),
            ("Dave", "D", "polite", "where is everyone going", &[]),
            ("Wolf", "c", "normal", "u guys are so bad lol", &["Player: lol"]),
            ("ZeRo", "t", "cocky", "can you help me with my homework", &[]),
            ("bob", "o", "normal", "what's the best gun", &[]),
        ];
        for m in MODELS.iter().filter(|m| runtime::has_model(m)) {
            let mut s = runtime::Server::start(&exe, m).expect("start");
            let t = Instant::now();
            while !s.healthy() {
                s.alive().expect("running");
                assert!(t.elapsed() < LOAD_TIMEOUT, "load");
                std::thread::sleep(Duration::from_millis(200));
            }
            println!("\n== {} ({:?}) loaded in {} ms", m.name, m.mode, t.elapsed().as_millis());
            for round in 0..2 {
                for (bot, persona, mood, msg, hist) in prompts {
                    let r = ReplyReq {
                        bot: bot.to_string(),
                        persona: persona.to_string(),
                        mood: mood.to_string(),
                        kind: "reply".into(),
                        message: msg.to_string(),
                        human: "Player".into(),
                        history: hist.iter().map(|h| h.to_string()).collect(),
                        context: serde_json::from_value(json!({"map": "de_dust2", "score": "4-6", "alive": true})).unwrap(),
                        ..Default::default()
                    }
                    .tidy();
                    match generate(s.port, m, &r) {
                        Ok(w) => println!("  [{round}] {bot:>13} ({mood:>6}) <- {msg:<32} {:>5} ms  {:?}  {}", w.ms, w.text, w.raw.map(|r| format!("rejected: {r:?}")).unwrap_or_default()),
                        Err(e) => println!("  [{round}] {bot} error {e}"),
                    }
                }
            }
            s.stop();
        }
    }

    /// The Try it sheet through the routes: start, wait until ready, ask, stop.
    /// LONER_STRIKE_AI_DIR=<folder with a model> cargo test live_try -- --ignored --nocapture
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn live_try() {
        let st = state("127.0.0.1:8027");
        let call = |uri: &'static str, body: &'static str| {
            let st = st.clone();
            async move {
                let req = Request::builder().method(if body.is_empty() { "GET" } else { "POST" }).uri(uri).header("host", "127.0.0.1:8027").header("x-panel", "1").header("content-type", "application/json").body(Body::from(body)).unwrap();
                let res = server::router(st, Listener::Local).oneshot(req).await.unwrap();
                let b = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
                serde_json::from_slice::<serde_json::Value>(&b).unwrap_or_default()
            }
        };
        println!("start: {}", call("/api/smartchat/try/start", r#"{"model":"llama32-1b"}"#).await);
        let t = std::time::Instant::now();
        loop {
            let st2 = st.clone();
            tokio::task::spawn_blocking(move || super::tick(&st2)).await.unwrap();
            let v = call("/api/smartchat", "").await;
            if v["status"]["state"] != "starting" {
                println!("status after {} ms: {}", t.elapsed().as_millis(), v["status"]);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        println!("try: {}", call("/api/smartchat/try", r#"{"message":"why do u always camp"}"#).await);
        println!("stop: {}", call("/api/smartchat/try/stop", "{}").await);
        println!("status: {}", call("/api/smartchat", "").await["status"]);
    }
}
