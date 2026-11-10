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
//!
//! On a Linux server (a VPS) the app puts its Linux ls-voice there (shipped in the app, like
//! ls-control) and the machine downloads the model itself; a systemd service runs it with the
//! server's (started and stopped with it), sandboxed to talking to the machine itself.

use crate::panel_api::from_panel;
use crate::remote::{self, shell_quote, RemoteInfo};
use sha2::{Digest, Sha256};
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
/// The understanding model (ls-intent): Loner-Strike's fine-tune of Gemma 3 270M, on its mirror
/// (models/GEMMA-NOTICE.txt beside it: Gemma's terms)
const INTENT_URL: &str = "https://dl.lonerstrike.com/models/gemma3-270m-ls-intent-1-Q4_K_M.gguf";
const INTENT_SHA: &str = "e550a1cbe3d84a3448194c4080a54c58c84d25fce0bfad4b4ca38d4f13b0fa50";
const INTENT_FILE: &str = "gemma3-270m-ls-intent-1-Q4_K_M.gguf";
pub const INTENT_BYTES: u64 = 368_458_496;
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

fn intent_model() -> PathBuf {
    dir().join(INTENT_FILE)
}

/// Installed: both models are here, whole.
pub fn installed() -> bool {
    std::fs::metadata(model()).map(|m| m.len() == MODEL_BYTES).unwrap_or(false)
        && std::fs::metadata(intent_model()).map(|m| m.len() == INTENT_BYTES).unwrap_or(false)
}

/// A helper program for this computer: next to the app (bundled like playit's agent), or the
/// repo's build in dev ("voice": ls-voice, "intent": ls-intent).
fn program(base: &str, crate_dir: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) { format!("{base}.exe") } else { base.to_string() };
    let mut places = sys::bundled_places(&name);
    if cfg!(debug_assertions) {
        places.push(crate::setup::repo_root().join("app").join(crate_dir).join("target/release").join(&name));
    }
    places.into_iter().find(|p| p.is_file())
}

fn exe(_st: &Shared) -> Option<PathBuf> {
    program("ls-voice", "voice-agent")
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
    .into_iter()
    // (the understanding model, when both it and its program are here)
    .chain(match program("ls-intent", "intent-agent") {
        Some(p) if intent_model().is_file() => vec![
            "--intent".to_string(),
            p.to_string_lossy().into_owned(),
            "--intent-model".to_string(),
            intent_model().to_string_lossy().into_owned(),
        ],
        _ => vec![],
    })
    .collect()
}

// ---- on a Linux server ---------------------------------------------------------------------

/// The program in the app's resources (its sha256 file beside it), and on the machine
const RES: &str = "linux/ls-voice/ls-voice";
const BIN: &str = "loner-strike/bin/ls-voice";
const INTENT_RES: &str = "linux/ls-intent/ls-intent";
const INTENT_BIN: &str = "loner-strike/bin/ls-intent";
const REMOTE_INTENT: &str = "loner-strike/voice/gemma3-270m-ls-intent-1-Q4_K_M.gguf";
/// The model on the machine, shared by its servers
const REMOTE_MODEL: &str = "loner-strike/voice/ggml-base.en.bin";
/// Raised whenever the unit's text changes: a unit without it is written again
const UNIT_REV: &str = "loner-strike voice unit 2";

pub fn unit(id: &str) -> String {
    format!("lonerstrike-{id}-voice")
}

/// A Linux program as the app ships it, checked against its sha256 (sha256, bytes).
fn shipped(res: &std::path::Path, rel: &str) -> Result<(String, Vec<u8>), String> {
    let mut places = vec![res.join(rel)];
    if cfg!(debug_assertions) {
        places.push(crate::setup::repo_root().join("app/src-tauri/resources").join(rel));
    }
    let missing = "This copy of Loner-Strike doesn't include the voice program for Linux servers. Reinstalling the app should fix it; in a build from source, run scripts/build-voice-agent.sh.";
    let bin = places.into_iter().find(|p| p.is_file()).ok_or(missing)?;
    let bytes = std::fs::read(&bin).map_err(|e| format!("Couldn't read the voice program ({e})."))?;
    let want = std::fs::read_to_string(PathBuf::from(format!("{}.sha256", bin.display()))).map_err(|_| missing.to_string())?;
    let want = want.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    let have: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if want.len() != 64 || want != have || !bytes.starts_with(b"\x7fELF\x02") {
        return Err("The app's copy of the voice program doesn't match its checksum. Reinstalling the app should fix it.".into());
    }
    Ok((have, bytes))
}

/// A program, sent beside the old one, checked there, renamed over it (only when it changed).
fn send_program(r: &RemoteInfo, bin: &str, sha: &str, bytes: &[u8]) -> Result<(), String> {
    let q = shell_quote(bin);
    let (_, out) = remote::exec(r, &format!("cd \"$HOME\" && mkdir -p loner-strike/bin && if [ -f {q} ]; then sha256sum {q} | cut -c1-64; else echo none; fi"))?;
    if out.trim() == sha {
        return Ok(());
    }
    let new = format!("{bin}.new");
    remote::write_file(r, &new, bytes)?;
    let n = shell_quote(&new);
    let (code, out) = remote::exec(r, &format!("cd \"$HOME\" && [ \"$(sha256sum {n} | cut -c1-64)\" = {} ] && chmod 755 {n} && mv -f {n} {q}", shell_quote(sha)))?;
    if code != 0 {
        return Err(format!("The voice program didn't arrive whole on the server ({}). Try again.", out.trim()));
    }
    Ok(())
}

/// The model (downloaded there, checked), and the service: on with the server, or off.
fn remote_sh(e: &Entry, on: bool) -> String {
    let u = unit(&e.id);
    if !on {
        return format!(
            "set -u\nfail() {{ echo \"@@FAIL $*\"; exit 3; }}\n[ \"$(id -u)\" = 0 ] && ! command -v sudo >/dev/null && sudo() {{ [ \"$1\" = -n ] && shift; \"$@\"; }}\n\
             sudo -n true 2>/dev/null || fail \"The user $(whoami) can't use sudo without a password, which this needs. Give it passwordless sudo, or connect as root.\"\n\
             sudo -n systemctl disable --now {u} >/dev/null 2>&1; sudo -n rm -f /etc/systemd/system/{u}.service; sudo -n systemctl daemon-reload\n\
             [ -e /etc/systemd/system/{u}.service ] && fail \"Its service couldn't be removed.\"\n\
             # (the model goes when no server on the machine uses it)\n\
             ls /etc/systemd/system/lonerstrike-*-voice.service >/dev/null 2>&1 || rm -f \"$HOME/{REMOTE_MODEL}\" \"$HOME/{REMOTE_INTENT}\"\necho \"@@STEP off\"\n"
        );
    }
    let server = remote::unit(&e.id);
    let pw = format!("{}/{}/addons/amxmodx/data/gamemodes/panel_rcon.txt", remote::server_dir(&e.id), e.game);
    format!(
        r#"set -eu
fail() {{ echo "@@FAIL $*"; exit 3; }}
[ "$(id -u)" = 0 ] && ! command -v sudo >/dev/null && sudo() {{ [ "$1" = -n ] && shift; "$@"; }}
sudo -n true 2>/dev/null || fail "The user $(whoami) can't use sudo without a password, which this needs. Give it passwordless sudo, or connect as root."
grep -qw avx2 /proc/cpuinfo && grep -qw fma /proc/cpuinfo || fail "This machine's processor is too old for voice commands: they need AVX2."
M="$HOME/{model}"
mkdir -p "$(dirname "$M")"
if [ "$(sha256sum "$M" 2>/dev/null | cut -c1-64)" != "{sha}" ]; then
  echo "@@STEP model"
  curl -fsSL --retry 3 -o "$M.part" "{url}" || fail "Couldn't download the speech model."
  [ "$(sha256sum "$M.part" | cut -c1-64)" = "{sha}" ] || {{ rm -f "$M.part"; fail "The speech model didn't download whole."; }}
  mv -f "$M.part" "$M"
fi
I="$HOME/{intent}"
if [ "$(sha256sum "$I" 2>/dev/null | cut -c1-64)" != "{isha}" ]; then
  echo "@@STEP intent"
  curl -fsSL --retry 3 -o "$I.part" "{iurl}" || fail "Couldn't download the understanding model."
  [ "$(sha256sum "$I.part" | cut -c1-64)" = "{isha}" ] || {{ rm -f "$I.part"; fail "The understanding model didn't download whole."; }}
  mv -f "$I.part" "$I"
fi
sudo tee /etc/systemd/system/{u}.service >/dev/null <<EOF
[Unit]
# {rev}
Description=Loner-Strike voice for the bots {id}
After={server}.service
PartOf={server}.service

[Service]
User=$(whoami)
ExecStart=$HOME/{bin} --feed {feed} --rcon-port {port} --rcon-password-file $HOME/{pw} --model $M --threads 2 --intent $HOME/{ibin} --intent-model $I
Restart=always
RestartSec=5
Nice=10
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=yes
PrivateDevices=yes
RestrictAddressFamilies=AF_INET AF_UNIX
IPAddressDeny=any
IPAddressAllow=localhost
CapabilityBoundingSet=
AmbientCapabilities=
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
RestrictNamespaces=yes
RestrictSUIDSGID=yes
RestrictRealtime=yes
LockPersonality=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service
UMask=077
TasksMax=64
MemoryMax=1536M

[Install]
WantedBy={server}.service
EOF
sudo systemctl daemon-reload
sudo systemctl enable {u} >/dev/null 2>&1
# (with the server running, now; otherwise it starts with it)
if systemctl is-active --quiet {server}; then sudo systemctl restart {u}; fi
echo "@@STEP ready"
"#,
        model = REMOTE_MODEL,
        sha = MODEL_SHA,
        url = MODEL_URL,
        rev = UNIT_REV,
        id = e.id,
        bin = BIN,
        feed = feed_port(e),
        port = e.port,
        pw = pw,
        intent = REMOTE_INTENT,
        isha = INTENT_SHA,
        iurl = INTENT_URL,
        ibin = INTENT_BIN,
    )
}

/// What takes its service off the machine when the server is removed or rebuilt (in
/// remote::remove's command; the shared model stays while another server uses it).
pub fn remove_sh(id: &str) -> String {
    let u = unit(id);
    format!("sudo systemctl disable --now {u} >/dev/null 2>&1; sudo rm -f /etc/systemd/system/{u}.service; ")
}

/// Voice on or off for a server on a Linux machine.
fn remote_apply(e: &Entry, on: bool, res: &std::path::Path) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("That server is on this computer.")?;
    if on {
        let (sha, bytes) = shipped(res, RES)?;
        send_program(r, BIN, &sha, &bytes)?;
        let (sha, bytes) = shipped(res, INTENT_RES)?;
        send_program(r, INTENT_BIN, &sha, &bytes)?;
    }
    let mut last = String::new();
    let code = remote::exec_stream(r, "bash -s", remote_sh(e, on).as_bytes(), |l| {
        if let Some(why) = l.strip_prefix("@@FAIL ") {
            last = why.to_string();
        } else if !l.starts_with("@@") && !l.trim().is_empty() {
            last = l.trim().to_string();
        }
    })?;
    if code != 0 {
        return Err(format!("Couldn't {} voice on the server ({last}). Try again; if it keeps failing, check the server's Security and health panel.", if on { "set up" } else { "turn off" }));
    }
    Ok(())
}

/// On a Linux server: running, and the last lines it wrote.
fn remote_state(e: &Entry) -> (bool, Vec<String>) {
    let Some(r) = &e.remote else { return (false, vec![]) };
    let u = unit(&e.id);
    let cmd = format!("systemctl is-active {u} 2>/dev/null; sudo -n journalctl -u {u} -n 12 --no-pager -o cat 2>/dev/null");
    match remote::exec_quick(r, &cmd) {
        Ok((_, out)) => {
            let mut lines = out.lines();
            let running = lines.next().map(|l| l.trim() == "active").unwrap_or(false);
            (running, lines.map(|l| l.trim_start_matches("ls-voice: ").to_string()).filter(|l| !l.is_empty()).collect())
        }
        Err(_) => (false, vec![]),
    }
}

// ---- running beside a server on this computer ---------------------------------------------

struct Proc {
    child: Child,
}

#[derive(Default)]
struct Runs {
    procs: HashMap<String, Proc>,
    errors: HashMap<String, String>,
    /// servers whose voice is waiting for them to answer (one waiter each)
    pending: std::collections::HashSet<String>,
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
                r.errors.insert(id.into(), "Voice stopped by itself. Its log below may say why. Turn it off and on again to restart it.".into());
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

/// Speech-to-text needs AVX2 and FMA (Intel from 2013, AMD from 2015): an older processor
/// would crash it, so it's said plainly instead.
pub fn cpu_too_old() -> Option<&'static str> {
    #[cfg(target_arch = "x86_64")]
    if !(std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma")) {
        return Some("This computer's processor is too old for voice commands: they need AVX2, which Intel processors have from 2013 and AMD from 2015.");
    }
    None
}

/// Starts ls-voice for a server on this computer, once the server answers.
pub fn start(e: &Entry) -> Result<(), String> {
    if e.remote.is_some() {
        return Err("Voice on a Linux server is set up from its page.".into());
    }
    if !installed() {
        return Err("Install bot voice commands first, from this tab.".into());
    }
    let exe = EXE.lock().unwrap().clone().ok_or(
        "The voice program is missing from the app. An antivirus may have removed it: allow Loner-Strike's folder in it, then reinstall Loner-Strike.",
    )?;
    if let Some(why) = cpu_too_old() {
        return Err(why.into());
    }
    let id = e.id.clone();
    // (already running, or already waiting for its server)
    if with_runs(|r| alive(r, &id) || !r.pending.insert(id.clone())) {
        return Ok(());
    }
    std::thread::spawn(move || {
        let done = |r: &mut Runs| {
            r.pending.remove(&id);
        };
        // (its server first: it needs the console to tell the server where to send voice)
        let mut up = false;
        for _ in 0..90 {
            if servers::get(&id).and_then(|e| e.rcon()).map(|r| r.send("status").is_ok()).unwrap_or(false) {
                up = true;
                break;
            }
            if !servers::running(&id) || !with_runs(|r| r.pending.contains(&id)) {
                with_runs(done);
                return;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        let result = match (up, servers::get(&id)) {
            (false, _) => Err("The server didn't come up, so voice wasn't started.".to_string()),
            (true, None) => Err("That server isn't there any more.".to_string()),
            (true, Some(e)) => spawn(&e, &exe),
        };
        // (stopped while it waited, or the server's gone again: the new one isn't wanted)
        let wanted = servers::running(&id);
        let extra = with_runs(|r| {
            let still = r.pending.remove(&id) && wanted;
            match result {
                // (not tied to the app, like its server: one kept running when the app quits keeps
                // its voice; a leftover ends itself once its server is gone, or at the next start)
                Ok(child) if still => {
                    r.errors.remove(&id);
                    r.procs.insert(id.clone(), Proc { child });
                    None
                }
                Ok(child) => Some(child),
                Err(err) => {
                    if still {
                        r.errors.insert(id.clone(), err);
                    }
                    None
                }
            }
        });
        if let Some(child) = extra {
            sys::stop_child(child, Duration::from_secs(2));
        }
    });
    Ok(())
}

fn spawn(e: &Entry, exe: &PathBuf) -> Result<Child, String> {
    let port = feed_port(e);
    end_leftover(e);
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
        .map_err(|err| format!("Voice didn't start ({err}). Try turning it off and on again, or Remove and install it again."))
        .inspect(|child| {
            let _ = std::fs::write(pid_file(e), child.id().to_string());
        })
}

fn pid_file(e: &Entry) -> PathBuf {
    e.dir.join("loner-strike-voice.pid")
}

/// Ends this server's ls-voice from an earlier session the app lost track of (it crashed, or
/// the server was kept running when it quit), so the new one can have the feed port: only
/// when that pid is still an ls-voice.
fn end_leftover(e: &Entry) {
    let file = pid_file(e);
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let _ = std::fs::remove_file(&file);
    #[cfg(unix)]
    if let Ok(pid) = text.trim().parse::<i32>() {
        let mut c = sys::command("ps");
        c.args(["-p", &pid.to_string(), "-o", "command="]);
        if let Some((_, out, _)) = sys::run_output(c, Duration::from_secs(3)) {
            if out.contains("ls-voice") {
                unsafe { libc::kill(pid, libc::SIGKILL) };
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }
    #[cfg(windows)]
    if let Ok(pid) = text.trim().parse::<u32>() {
        let mut c = sys::command("tasklist");
        c.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
        if let Some((_, out, _)) = sys::run_output(c, Duration::from_secs(3)) {
            if out.to_lowercase().contains("ls-voice") {
                let mut k = sys::command("taskkill");
                k.args(["/PID", &pid.to_string(), "/T", "/F"]);
                let _ = sys::run_output(k, Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }
}

/// Stops ls-voice for a server (the server stops sending voice when it's gone: nothing listens).
pub fn stop(e: &Entry) {
    if e.remote.is_some() {
        return;
    }
    // (one from an earlier session the app lost track of, too)
    end_leftover(e);
    let child = with_runs(|r| {
        r.errors.remove(&e.id);
        r.pending.remove(&e.id);
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
    let (mut running, mut error) = with_runs(|r| (alive(r, &e.id), r.errors.get(&e.id).cloned()));
    if e.remote.is_none() && error.is_none() {
        error = cpu_too_old().map(String::from);
    }
    let mut log = if e.remote.is_none() { log_tail(e) } else { vec![] };
    if e.remote.is_some() && e.voice.enabled {
        (running, log) = remote_state(e);
    }
    json!({
        // (a Linux server's model goes on the machine itself: nothing to install here)
        "installed": e.remote.is_some() || installed(),
        "remote": e.remote.is_some(),
        "installing": INSTALLING.lock().unwrap().is_some(),
        "progress": *PROGRESS.lock().unwrap(),
        "model_bytes": MODEL_BYTES + INTENT_BYTES,
        "enabled": e.voice.enabled,
        "supported": true,
        "running": running,
        "error": error,
        "log": log,
    })
}

// ---- installing and removing --------------------------------------------------------------

static INSTALLING: Mutex<Option<Instant>> = Mutex::new(None);
/// How far the download is (0..1), for the page
static PROGRESS: Mutex<f64> = Mutex::new(0.0);

/// One download's progress, as part of the whole (it starts at `from` and is `share` of it)
struct Download {
    from: f64,
    share: f64,
}
impl crate::setup::Progress for Download {
    fn detail(&self, _: &str) {}
    fn fraction(&self, f: f64) {
        *PROGRESS.lock().unwrap() = self.from + self.share * f;
    }
    fn note(&self, _: &str) {}
}

fn install() -> Result<(), String> {
    if let Some(why) = cpu_too_old() {
        return Err(why.into());
    }
    {
        let mut busy = INSTALLING.lock().unwrap();
        if busy.is_some() {
            return Err("It's being installed already.".into());
        }
        *busy = Some(Instant::now());
    }
    let result = (|| {
        std::fs::create_dir_all(dir()).map_err(|err| format!("Couldn't make a folder for the models ({err}). Check there's free disk space and try again."))?;
        let whole = (MODEL_BYTES + INTENT_BYTES) as f64;
        let speech = MODEL_BYTES as f64 / whole;
        for (url, sha, what, to, from, share) in [
            (MODEL_URL, MODEL_SHA, "the speech model", model(), 0.0, speech),
            (INTENT_URL, INTENT_SHA, "the understanding model", intent_model(), speech, 1.0 - speech),
        ] {
            let got = crate::setup::fetch::download(url, Some(sha), what, &Download { from, share })?;
            // (moved, not copied: no second copy of a few hundred MB on the disk)
            if std::fs::rename(&got, &to).is_err() {
                std::fs::copy(&got, &to).map_err(|err| format!("Couldn't save {what} ({err})."))?;
                let _ = std::fs::remove_file(&got);
            }
            let _ = std::fs::remove_file(&got);
        }
        Ok(())
    })();
    *INSTALLING.lock().unwrap() = None;
    result
}

/// Removes the add-on: voice off on every server (stopped where it runs), the model deleted.
fn uninstall() -> Result<(), String> {
    // (every server on this computer: a leftover holds the model files open on Windows)
    for e in servers::load().into_iter().filter(|e| e.remote.is_none()) {
        stop(&e);
    }
    servers::edit(|l| {
        for x in l.iter_mut().filter(|x| x.remote.is_none()) {
            x.voice.enabled = false;
        }
    })?;
    match std::fs::remove_dir_all(dir()) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("Couldn't delete the models ({err}). Stop any server using voice and try again.")),
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
    let st = st.clone();
    blocking(move || {
        let e = entry(&id)?;
        if e.remote.is_some() {
            // (the machine first: if it can't be reached nothing changes, and trying again works)
            let res = crate::setup_api::resources_for(&st)?;
            remote_apply(&e, req.enabled, &res)?;
            servers::edit(|l| {
                if let Some(x) = l.iter_mut().find(|x| x.id == id) {
                    x.voice.enabled = req.enabled;
                }
            })?;
            return Ok(status(&entry(&id)?));
        }
        if req.enabled && !installed() {
            return Err("Install bot voice commands first, from this tab.".into());
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
    fn linux_service() {
        let mut e = server("a", 27017);
        e.remote = serde_json::from_value(json!({"host": "203.0.113.5", "key": "/k"})).unwrap();
        let on = remote_sh(&e, true);
        assert!(on.contains("lonerstrike-a-voice.service") && on.contains("PartOf=lonerstrike-a.service"));
        assert!(on.contains("--feed 27617 --rcon-port 27017") && on.contains("IPAddressAllow=localhost") && on.contains("IPAddressDeny=any"));
        assert!(on.contains(MODEL_SHA) && on.contains(INTENT_SHA) && on.contains("# loner-strike voice unit 2"));
        assert!(on.contains("--intent $HOME/loner-strike/bin/ls-intent --intent-model $I"));
        let off = remote_sh(&e, false);
        assert!(off.contains("disable --now lonerstrike-a-voice") && off.contains("rm -f /etc/systemd/system/lonerstrike-a-voice.service"));
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
