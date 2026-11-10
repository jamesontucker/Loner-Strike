//! Advanced Bot Interaction: players' voice to text for the bots (ls-voice, app/voice-agent).
//!
//! An optional add-on, per server. Installed once on this computer: the speech model (Whisper
//! base.en, ~148 MB) downloaded into the app's folder; the program itself ships with the app
//! and does nothing until a server turns voice on. Removing it deletes the model and turns
//! voice off everywhere.
//!
//! On a server with voice on, ls-voice starts after the server is up (servers::start calls
//! server_started) and stops before it (servers::stop calls stop). It tells the server to
//! hand it players' voice (Loner-Strike's engine: sv_ls_voice_feed, 127.0.0.1 only), turns
//! each player's talking into text on this computer, and gives that to the bots as team chat
//! through the server's console. Nothing is recorded or sent anywhere.
//!
//! Only dedicated servers: the engine's voice feed is in Loner-Strike's server engine, not in
//! the game itself (playing from the game's own Play page).

use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::servers::{self, Entry};
use crate::sys;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get as route_get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The speech model: where it comes from, its sha256 and size (whisper.cpp's own copy)
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";
const MODEL_SHA: &str = "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002";
const MODEL_FILE: &str = "ggml-base.en.bin";
pub const MODEL_BYTES: u64 = 147_964_211;
const OUT_LOG: &str = "loner-strike-voice.log";
/// ls-voice listens this far above its server's port, on 127.0.0.1 (27016 -> 27616)
const FEED_OFFSET: u16 = 600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Settings {
    /// The bots hear what people say on voice
    pub enabled: bool,
}

/// The add-on's folder on this computer.
fn dir() -> PathBuf {
    sys::app_dir().join("voice")
}

fn model() -> PathBuf {
    dir().join(MODEL_FILE)
}

/// Installed: the model is here, whole.
pub fn installed() -> bool {
    std::fs::metadata(model()).map(|m| m.len() == MODEL_BYTES).unwrap_or(false)
}

/// ls-voice for this computer: next to the app (bundled like playit's agent), or the repo's
/// build in dev.
fn exe(_st: &Shared) -> Option<PathBuf> {
    let name = if cfg!(windows) { "ls-voice.exe" } else { "ls-voice" };
    let mut places = sys::bundled_places(name);
    if cfg!(debug_assertions) {
        places.push(crate::setup::repo_root().join("app/voice-agent/target/release").join(name));
    }
    places.into_iter().find(|p| p.is_file())
}

/// The port it listens on for its server's voice.
pub fn feed_port(e: &Entry) -> u16 {
    e.port.saturating_add(FEED_OFFSET)
}

fn password_file(e: &Entry) -> PathBuf {
    e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes/panel_rcon.txt")
}

fn args(e: &Entry) -> Vec<String> {
    vec![
        "--feed".into(),
        feed_port(e).to_string(),
        "--rcon-port".into(),
        e.port.to_string(),
        "--rcon-password-file".into(),
        password_file(e).to_string_lossy().into_owned(),
        "--model".into(),
        model().to_string_lossy().into_owned(),
        "--threads".into(),
        "2".into(),
    ]
}

// ---- running beside a server on this computer ---------------------------------------------

struct Proc {
    child: Child,
}

#[derive(Default)]
struct Runs {
    procs: HashMap<String, Proc>,
    errors: HashMap<String, String>,
}

static RUNS: Mutex<Option<Runs>> = Mutex::new(None);
/// Where ls-voice is, worked out once the app's resources are known
static EXE: Mutex<Option<PathBuf>> = Mutex::new(None);

fn with_runs<T>(f: impl FnOnce(&mut Runs) -> T) -> T {
    let mut r = RUNS.lock().unwrap();
    f(r.get_or_insert_with(Runs::default))
}

fn alive(r: &mut Runs, id: &str) -> bool {
    match r.procs.get_mut(id) {
        Some(p) => match p.child.try_wait() {
            Ok(None) => true,
            _ => {
                if let Some(p) = r.procs.remove(id) {
                    sys::untie(p.child.id());
                }
                r.errors.insert(id.into(), "Voice stopped by itself. The log below may say why.".into());
                false
            }
        },
        None => false,
    }
}

/// Called when a server on this computer has just been started: voice follows once it's up.
pub fn server_started(e: &Entry) {
    if e.voice.enabled && e.remote.is_none() {
        let _ = start(e);
    }
}

/// Starts ls-voice for a server on this computer, once the server answers.
pub fn start(e: &Entry) -> Result<(), String> {
    if e.remote.is_some() {
        return Err("Voice on a Linux server is set up from its page.".into());
    }
    if !installed() {
        return Err("Install Advanced Bot Interaction first.".into());
    }
    let exe = EXE.lock().unwrap().clone().ok_or("This copy of the app doesn't include the voice program. Update the app.")?;
    let id = e.id.clone();
    if with_runs(|r| alive(r, &id)) {
        return Ok(());
    }
    std::thread::spawn(move || {
        // (its server first: it needs the console to tell the server where to send voice)
        let mut up = false;
        for _ in 0..90 {
            if servers::get(&id).and_then(|e| e.rcon()).map(|r| r.send("status").is_ok()).unwrap_or(false) {
                up = true;
                break;
            }
            if !servers::running(&id) {
                return;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        let result = match (up, servers::get(&id)) {
            (false, _) => Err("The server didn't come up, so voice wasn't started.".to_string()),
            (true, None) => Err("That server isn't there any more.".to_string()),
            (true, Some(e)) => spawn(&e, &exe),
        };
        with_runs(|r| match result {
            Ok(child) => {
                r.errors.remove(&id);
                r.procs.insert(id.clone(), Proc { child });
            }
            Err(err) => {
                r.errors.insert(id.clone(), err);
            }
        });
    });
    Ok(())
}

fn spawn(e: &Entry, exe: &PathBuf) -> Result<Child, String> {
    let port = feed_port(e);
    if std::net::UdpSocket::bind(("127.0.0.1", port)).is_err() {
        return Err(format!("Port {port} on this computer is in use by another program, so voice couldn't start."));
    }
    let log = std::fs::File::create(e.dir.join(OUT_LOG)).map_err(|err| err.to_string())?;
    sys::command(exe)
        .args(args(e))
        .current_dir(&e.dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|err| err.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|err| format!("Voice didn't start ({err})."))
}

/// Stops ls-voice for a server (the server stops sending voice when it's gone: nothing listens).
pub fn stop(e: &Entry) {
    if e.remote.is_some() {
        return;
    }
    let child = with_runs(|r| {
        r.errors.remove(&e.id);
        r.procs.remove(&e.id).map(|p| p.child)
    });
    if let Some(r) = e.rcon() {
        let _ = r.send("sv_ls_voice_feed 0");
    }
    if let Some(child) = child {
        sys::stop_child(child, Duration::from_secs(2));
    }
}

/// The last lines it wrote (what it heard and what went to the bots), newest last.
fn log_tail(e: &Entry) -> Vec<String> {
    let text = std::fs::read_to_string(e.dir.join(OUT_LOG)).unwrap_or_default();
    let lines: Vec<String> = text.lines().map(|l| l.trim_start_matches("ls-voice: ").to_string()).collect();
    lines[lines.len().saturating_sub(12)..].to_vec()
}

fn status(e: &Entry) -> Value {
    let (running, error) = with_runs(|r| (alive(r, &e.id), r.errors.get(&e.id).cloned()));
    json!({
        "installed": installed(),
        "installing": INSTALLING.lock().unwrap().is_some(),
        "progress": *PROGRESS.lock().unwrap(),
        "model_bytes": MODEL_BYTES,
        "enabled": e.voice.enabled,
        "supported": e.remote.is_none(),
        "running": running,
        "error": error,
        "log": if e.remote.is_none() { log_tail(e) } else { vec![] },
    })
}

// ---- installing and removing --------------------------------------------------------------

static INSTALLING: Mutex<Option<Instant>> = Mutex::new(None);
/// How far the download is (0..1), for the page
static PROGRESS: Mutex<f64> = Mutex::new(0.0);

struct Download;
impl crate::setup::Progress for Download {
    fn detail(&self, _: &str) {}
    fn fraction(&self, f: f64) {
        *PROGRESS.lock().unwrap() = f;
    }
    fn note(&self, _: &str) {}
}

fn install() -> Result<(), String> {
    {
        let mut busy = INSTALLING.lock().unwrap();
        if busy.is_some() {
            return Err("It's being installed already.".into());
        }
        *busy = Some(Instant::now());
    }
    let result = (|| {
        std::fs::create_dir_all(dir()).map_err(|err| format!("Couldn't make its folder ({err})."))?;
        let got = crate::setup::fetch::download(MODEL_URL, Some(MODEL_SHA), "the speech model", &Download)?;
        std::fs::copy(&got, model()).map_err(|err| format!("Couldn't save the speech model ({err})."))?;
        let _ = std::fs::remove_file(&got);
        Ok(())
    })();
    *INSTALLING.lock().unwrap() = None;
    result
}

/// Removes the add-on: voice off on every server (stopped where it runs), the model deleted.
fn uninstall() -> Result<(), String> {
    for e in servers::load().into_iter().filter(|e| e.voice.enabled) {
        stop(&e);
    }
    servers::edit(|l| {
        for x in l.iter_mut() {
            x.voice.enabled = false;
        }
    })?;
    match std::fs::remove_dir_all(dir()) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("Couldn't delete its files ({err}).")),
    }
}

// ---- the page ------------------------------------------------------------------------------

fn forbidden() -> (StatusCode, Json<Value>) {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> (StatusCode, Json<Value>) {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => (StatusCode::OK, Json(v)),
        Ok(Err(e)) => (StatusCode::OK, Json(json!({"error": e}))),
        Err(_) => (StatusCode::OK, Json(json!({"error": "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening."}))),
    }
}

fn entry(id: &str) -> Result<Entry, String> {
    servers::get(id).ok_or_else(|| "That server isn't there any more.".to_string())
}

fn remember_exe(st: &Shared) {
    let mut x = EXE.lock().unwrap();
    if x.is_none() {
        *x = exe(st);
    }
}

async fn api_status(State(st): State<Shared>, UrlPath(id): UrlPath<String>) -> (StatusCode, Json<Value>) {
    remember_exe(&st);
    blocking(move || entry(&id).map(|e| status(&e))).await
}

#[derive(Deserialize)]
struct SaveReq {
    enabled: bool,
}

/// Voice on or off for a server (from this computer only, not the phone).
async fn api_save(State(st): State<Shared>, UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, Json(req): Json<SaveReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    remember_exe(&st);
    blocking(move || {
        let e = entry(&id)?;
        if e.remote.is_some() {
            return Err("Voice for a Linux server comes in a later update.".into());
        }
        if req.enabled && !installed() {
            return Err("Install Advanced Bot Interaction first.".into());
        }
        servers::edit(|l| {
            if let Some(x) = l.iter_mut().find(|x| x.id == id) {
                x.voice.enabled = req.enabled;
            }
        })?;
        let e = entry(&id)?;
        if req.enabled && servers::running(&id) {
            start(&e)?;
        } else if !req.enabled {
            stop(&e);
        }
        Ok(status(&e))
    })
    .await
}

/// Install or remove the add-on on this computer.
async fn api_addon(State(st): State<Shared>, UrlPath(action): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    remember_exe(&st);
    blocking(move || {
        match action.as_str() {
            "install" => install()?,
            "uninstall" => uninstall()?,
            _ => return Err("Unknown action.".into()),
        }
        Ok(json!({"ok": true, "installed": installed()}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/servers/:id/voice", route_get(api_status).post(api_save))
        .route("/api/voice/:action", post(api_addon))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str, port: u16) -> Entry {
        serde_json::from_value(json!({"id": id, "name": format!("Server {id}"), "game": "cstrike", "port": port, "dir": "/tmp/x", "kind": "crossover", "ready": true}))
            .unwrap()
    }

    #[test]
    fn old_servers_have_voice_off() {
        let e = server("a", 27016);
        assert_eq!(e.voice, Settings::default());
        assert!(!e.voice.enabled);
    }

    #[test]
    fn command_line() {
        let e = server("a", 27017);
        assert_eq!(feed_port(&e), 27617);
        let a = args(&e);
        assert_eq!(&a[..4], &["--feed", "27617", "--rcon-port", "27017"]);
        assert!(a[5].ends_with("cstrike/addons/amxmodx/data/gamemodes/panel_rcon.txt"));
        assert!(a[7].ends_with("voice/ggml-base.en.bin"));
    }
}
