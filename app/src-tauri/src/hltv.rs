//! HLTV for dedicated servers: Valve's spectator relay (hltv.exe, shipped with HLDS
//! in the server's folder). It joins the game server like a special client and
//! passes the match on, delayed, to spectators who connect to its own port. It can
//! also record the match to demo files.
//!
//! It starts after its server is up (servers::start calls server_started) and stops
//! before it (servers::stop calls stop), through the same wine/bottle as the server.
//! Everything it needs goes in loner-strike-hltv.cfg, run with `+exec` after Valve's
//! own hltv.cfg (Proxy::Init runs hltv.cfg, then the command line's + commands run).
//! The panel talks to it over RCON with a random adminpassword from that file.
//!
//! On a Linux server (a VPS) it's Valve's Linux `hltv` instead, run by a systemd service
//! that starts and stops with the server's (remote::hltv_sh), on port 27020 unless set
//! otherwise. Its demos stay on the machine; the app lists them and copies one into the
//! player's game over SFTP.

use crate::panel_api::from_panel;
use crate::rcon::Rcon;
use crate::server::Shared;
use crate::servers::{self, Entry};
use crate::sys;
use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get as route_get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

const CFG: &str = "loner-strike-hltv.cfg";
const OUT_LOG: &str = "loner-strike-hltv.log";
const CONSOLE_LOG: &str = "loner-strike-hltv-console.log";
/// Demo files are named <this>-<YYMMDDhhmm>-<map>.dem by HLTV
const DEMO_NAME: &str = "hltv";
/// Without a port of its own, HLTV sits this far above its server's port (27016 -> 28016),
/// clear of the game ports (27015 and up) that servers use.
const PORT_OFFSET: u16 = 1000;
/// Ports HLTV must never take: the player's own game, the game client's, Steam/VAC's
const RESERVED: [u16; 3] = [27015, 27005, 26900];

// ---- settings --------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    /// None: the server's port + 1000
    pub port: Option<u16>,
    /// Seconds the broadcast runs behind the game (below 10 turns off the auto director)
    pub delay: u32,
    pub max_spectators: u32,
    /// Shown in the scoreboard and server browser; empty: "<server name> TV"
    pub name: String,
    /// Spectators need this to watch; empty: anyone with the address
    pub password: String,
    /// Records every map to a demo file
    pub record: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { enabled: false, port: None, delay: 30, max_spectators: 32, name: String::new(), password: String::new(), record: false }
    }
}

/// The port spectators connect to.
pub fn port_of(e: &Entry) -> u16 {
    e.hltv.port.unwrap_or_else(|| default_port(e))
}

fn default_port(e: &Entry) -> u16 {
    if e.remote.is_some() {
        return REMOTE_PORT;
    }
    e.port.saturating_add(PORT_OFFSET)
}

/// HLTV's port on a Linux server: the top of the range setup opens there
const REMOTE_PORT: u16 = 27020;

fn display_name(e: &Entry) -> String {
    let n = clean_name(&e.hltv.name);
    if n.is_empty() {
        clean_name(&format!("{} TV", e.name))
    } else {
        n
    }
}

/// Safe inside a quoted cfg value: no quotes, semicolons or control characters.
fn clean_name(n: &str) -> String {
    n.trim().chars().filter(|c| !c.is_control() && *c != '"' && *c != ';').take(32).collect::<String>().trim().to_string()
}

fn safe_password(p: &str) -> bool {
    !p.is_empty() && p.len() <= 32 && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// Checks and tidies new settings for server `me` (`all` is every server, `me` included).
pub fn validate(mut s: Settings, me: &Entry, all: &[Entry]) -> Result<Settings, String> {
    s.name = clean_name(&s.name);
    s.password = s.password.trim().to_string();
    if !s.password.is_empty() && !safe_password(&s.password) {
        return Err("Use up to 32 letters, numbers, dots, dashes or underscores for the spectator password.".into());
    }
    if s.delay > 600 {
        return Err("Pick a delay from 0 to 600 seconds.".into());
    }
    if s.max_spectators > 255 {
        return Err("HLTV takes at most 255 spectators.".into());
    }
    if s.max_spectators == 0 && !s.record {
        return Err("Allow at least one spectator, or turn on recording.".into());
    }
    // the default port follows the server's port
    if s.port == Some(default_port(me)) {
        s.port = None;
    }
    if s.enabled {
        let mut probe = me.clone();
        probe.hltv = s.clone();
        let p = port_of(&probe);
        port_free_in_list(p, me, all)?;
    }
    Ok(s)
}

fn port_free_in_list(p: u16, me: &Entry, all: &[Entry]) -> Result<(), String> {
    if let Some(r) = &me.remote {
        if !(27015..=REMOTE_PORT).contains(&p) || p == me.port {
            return Err(format!("On a Linux server HLTV uses a port from 27015 to 27020 that the server doesn't (27020 if the server is on {}).", me.port));
        }
        let same = |o: &&Entry| o.id != me.id && o.remote.as_ref().is_some_and(|x| x.host == r.host);
        if let Some(o) = all.iter().filter(same).find(|o| o.port == p || (o.hltv.enabled && port_of(o) == p)) {
            return Err(format!("\"{}\" uses port {p} on that machine. Pick another one.", o.name));
        }
        return Ok(());
    }
    if !(1024..=65000).contains(&p) {
        return Err("Pick an HLTV port from 1024 to 65000.".into());
    }
    if RESERVED.contains(&p) {
        return Err(format!("Port {p} is used by the game itself. Pick another HLTV port."));
    }
    if p == me.port {
        return Err("HLTV needs a port of its own, not the server's.".into());
    }
    if let Some(o) = all.iter().find(|o| o.id != me.id && o.port == p) {
        return Err(format!("\"{}\" uses port {p}. Pick another HLTV port.", o.name));
    }
    if let Some(o) = all.iter().find(|o| o.id != me.id && o.hltv.enabled && port_of(o) == p) {
        return Err(format!("HLTV on \"{}\" uses port {p}. Pick another one.", o.name));
    }
    Ok(())
}

// ---- starting and stopping -------------------------------------------------------------

/// hltv.exe's own command line: what can't go in a cfg (the port), then our cfg.
fn launch_args(e: &Entry) -> Vec<String> {
    vec!["-port".into(), port_of(e).to_string(), "+exec".into(), CFG.into()]
}

/// The commands HLTV runs at start (after Valve's hltv.cfg, so these win).
fn config(e: &Entry, admin: &str, server_password: Option<&str>) -> String {
    let s = &e.hltv;
    let name = display_name(e);
    let mut c = vec![
        "// Written by Loner-Strike each time HLTV starts. Change the settings in the app.".to_string(),
        format!("logfile {CONSOLE_LOG}"),
        format!("name \"{name}\""),
        format!("hostname \"{name}\""),
        format!("delay {}", s.delay.min(600)),
        format!("maxclients {}", s.max_spectators.min(255)),
        format!("adminpassword \"{admin}\""),
        // spectators can't see the game server's address or jump into the game from HLTV
        "publicgame 0".into(),
        format!("nomaster {}", if e.visibility == "listed" { 0 } else { 1 }),
        "offlinetext \"The broadcast starts in a moment. Try again shortly.\"".into(),
        // Valve's hltv.cfg shows an old advert every minute
        "loopcmd 1 none".into(),
        // follow the server through restarts and map changes
        "autoretry 1".into(),
    ];
    if safe_password(&s.password) {
        c.push(format!("spectatorpassword \"{}\"", s.password));
    }
    if let Some(pw) = server_password.filter(|p| safe_password(p)) {
        c.push(format!("serverpassword \"{pw}\""));
    }
    c.push(format!("connect {}:{}", e.local_host(), e.port));
    if s.record {
        c.push(format!("record {DEMO_NAME}"));
    }
    c.push(String::new());
    c.join("\n")
}

struct Proc {
    child: Child,
    started: Instant,
}

#[derive(Default)]
struct Runs {
    procs: HashMap<String, Proc>,
    /// waiting for the server to come up before starting (the number tells waits apart)
    pending: HashMap<String, u64>,
    errors: HashMap<String, String>,
    next: u64,
}

static RUNS: Mutex<Option<Runs>> = Mutex::new(None);

fn with_runs<T>(f: impl FnOnce(&mut Runs) -> T) -> T {
    let mut r = RUNS.lock().unwrap();
    f(r.get_or_insert_with(Runs::default))
}

fn exe(e: &Entry) -> PathBuf {
    e.dir.join("hltv.exe")
}

/// RCON to HLTV, with the admin password from the cfg it was started with.
fn admin(e: &Entry) -> Option<Rcon> {
    let text = std::fs::read_to_string(e.dir.join(CFG)).ok()?;
    let pw = text.lines().find_map(|l| l.strip_prefix("adminpassword "))?.trim().trim_matches('"').to_string();
    if pw.is_empty() {
        return None;
    }
    Some(match &e.remote {
        Some(r) => Rcon::over_ssh(r.clone(), format!("{}/hltv_admin.txt", crate::remote::server_dir(&e.id)), pw, port_of(e)),
        None => Rcon::new(pw, port_of(e)),
    })
}

fn new_admin() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..20).map(|_| char::from(b"abcdefghijklmnopqrstuvwxyz0123456789"[rng.gen_range(0..36)])).collect()
}

/// On a Linux server: writes HLTV's settings and service there, and turns it on or off.
/// A copy of the settings stays here, for the admin password.
pub(crate) fn remote_apply(e: &Entry, on: bool) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("That server is on this computer.")?;
    let admin = new_admin();
    let cfg = config(e, &admin, server_password(e).as_deref());
    if on {
        allow_proxies(e);
    }
    let code = crate::remote::exec_stream(r, "bash -s", crate::remote::hltv_sh(&e.id, cfg.trim_end(), &admin, port_of(e), on).as_bytes(), |_| {})?;
    match code {
        // the copy here only once the machine has it: refresh_remote compares with it, so
        // a write that didn't reach the machine is tried again
        0 => std::fs::write(e.dir.join(CFG), &cfg).map_err(|err| format!("HLTV is set up on the server, but its settings couldn't be saved here ({err}).")),
        3 => Err("HLTV isn't in this server's files. Stop the server and press Update in Overview, Maintenance, then try again.".into()),
        _ => Err("Couldn't set up HLTV on the server.".into()),
    }
}

/// After a change to a server on another machine: if HLTV's settings there no longer
/// match it (the join password from Play with friends, the name, who can join), they're
/// written again. Nothing happens when they still match.
pub fn refresh_remote(id: &str) {
    let Some(e) = servers::get(id).filter(|e| e.remote.is_some() && e.hltv.enabled && e.ready) else { return };
    // (no copy here: it never reached the machine, so it's written now)
    let now = std::fs::read_to_string(e.dir.join(CFG)).unwrap_or_default();
    let admin = now.lines().find_map(|l| l.strip_prefix("adminpassword ")).map(|p| p.trim().trim_matches('"').to_string()).unwrap_or_default();
    if config(&e, &admin, server_password(&e).as_deref()) != now {
        let _ = remote_apply(&e, true);
    }
}

/// The server's join password (Play with friends keeps it), which HLTV needs too.
fn server_password(e: &Entry) -> Option<String> {
    let t = std::fs::read_to_string(e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes/friends.json")).ok()?;
    serde_json::from_str::<Value>(&t).ok()?["password"].as_str().map(String::from).filter(|p| !p.is_empty())
}

fn alive(r: &mut Runs, id: &str) -> bool {
    match r.procs.get_mut(id) {
        Some(p) => match p.child.try_wait() {
            Ok(None) => true,
            _ => {
                if let Some(p) = r.procs.remove(id) {
                    sys::untie(p.child.id());
                }
                r.errors.insert(id.into(), "HLTV closed by itself. Its console below may say why.".into());
                false
            }
        },
        None => false,
    }
}

/// Called when the server has just been started: HLTV follows once it's up.
pub fn server_started(e: &Entry) {
    // on a Linux server its service starts with the server's
    if e.hltv.enabled && e.remote.is_none() {
        let _ = start(e);
    }
}

/// Starts HLTV as soon as the server answers (it needs a map loaded to connect).
pub fn start(e: &Entry) -> Result<(), String> {
    if e.remote.is_some() {
        return remote_apply(e, true);
    }
    if !exe(e).is_file() {
        return Err("This server's folder doesn't have HLTV. Update Valve's server files in Overview, under Maintenance, to get it.".into());
    }
    let id = e.id.clone();
    let gen = with_runs(|r| {
        if alive(r, &id) || r.pending.contains_key(&id) {
            return None;
        }
        r.next += 1;
        let n = r.next;
        r.pending.insert(id.clone(), n);
        r.errors.remove(&id);
        Some(n)
    });
    let Some(gen) = gen else { return Ok(()) };
    std::thread::spawn(move || {
        let still = || with_runs(|r| r.pending.get(&id) == Some(&gen));
        let mut up = false;
        for _ in 0..90 {
            if !still() {
                return;
            }
            if servers::get(&id).and_then(|e| e.rcon()).map(|r| r.send("status").is_ok()).unwrap_or(false) {
                up = true;
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        let result = if !up {
            Err("The server didn't come up, so HLTV wasn't started.".to_string())
        } else {
            match servers::get(&id) {
                Some(e) => {
                    allow_proxies(&e);
                    // spawn while holding the claim, so a stop meanwhile can't miss it
                    with_runs(|r| {
                        if r.pending.get(&id) != Some(&gen) {
                            return Ok(());
                        }
                        r.pending.remove(&id);
                        spawn(&e).map(|child| {
                            r.procs.insert(id.clone(), Proc { child, started: Instant::now() });
                        })
                    })
                }
                None => Err("That server isn't there any more.".into()),
            }
        };
        with_runs(|r| {
            if r.pending.get(&id) == Some(&gen) {
                r.pending.remove(&id);
            }
            if let Err(err) = result {
                r.errors.insert(id.clone(), err);
            }
        });
    });
    Ok(())
}

/// The server lets one HLTV proxy in unless its config set sv_proxies 0.
fn allow_proxies(e: &Entry) {
    if let Some(r) = e.rcon() {
        if r.send("sv_proxies").map(|t| t.contains("is \"0\"")).unwrap_or(false) {
            let _ = r.send("sv_proxies 1");
        }
    }
}

fn spawn(e: &Entry) -> Result<Child, String> {
    let port = port_of(e);
    // anything else on the port (another program) would make HLTV give up quietly
    if std::net::UdpSocket::bind(("0.0.0.0", port)).is_err() {
        return Err(format!("Port {port} is in use by another program. Pick another HLTV port."));
    }
    let admin = new_admin();
    let cfg = config(e, &admin, server_password(e).as_deref());
    std::fs::write(e.dir.join(CFG), cfg).map_err(|err| format!("Couldn't write HLTV's settings ({err})."))?;
    let log = std::fs::File::create(e.dir.join(OUT_LOG)).map_err(|err| err.to_string())?;
    let mut cmd = if e.kind == "crossover" {
        let mut c = sys::command(servers::CROSSOVER_WINE);
        let win = servers::windows_path(&exe(e)).ok_or("The server has to be inside a CrossOver bottle's C: drive.")?;
        c.arg("--bottle").arg(e.bottle.as_deref().unwrap_or("Steam")).arg(win);
        c
    } else {
        sys::command(exe(e))
    };
    cmd.args(launch_args(e))
        .current_dir(&e.dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|err| err.to_string())?)
        .stderr(log);
    let child = cmd.spawn().map_err(|err| format!("HLTV didn't start ({err})."))?;
    // like its server: stopped with it when the app quits, unless servers are kept running
    Ok(child)
}

/// Finishes the demo being recorded and quits HLTV (ending it if it doesn't).
pub fn stop(e: &Entry) {
    if let Some(r) = &e.remote {
        // the demo finished first, then the service (it starts again with the server)
        if let Some(a) = admin(e) {
            let _ = a.send("stoprecording");
        }
        let _ = crate::remote::exec(r, &format!("sudo systemctl stop {}", crate::remote::hltv_unit(&e.id)));
        return;
    }
    let (child, waiting) = with_runs(|r| {
        let waiting = r.pending.remove(&e.id).is_some();
        r.errors.remove(&e.id);
        (r.procs.remove(&e.id).map(|p| p.child), waiting)
    });
    if waiting && child.is_none() {
        return;
    }
    // one left running from before the app restarted only answers if HLTV is on
    if child.is_none() && !(e.hltv.enabled && e.dir.join(CFG).is_file()) {
        return;
    }
    if let Some(r) = admin(e) {
        if r.send("stoprecording").is_ok() {
            let _ = r.send("quit");
        }
    }
    let Some(mut child) = child else { return };
    for _ in 0..30 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            sys::untie(child.id());
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    sys::stop_child(child, Duration::from_secs(3));
}

// ---- what it's doing ---------------------------------------------------------------------

#[derive(Default, Debug, PartialEq)]
struct Live {
    spectators: Option<u32>,
    slots: Option<u32>,
    connected: bool,
    recording: Option<String>,
}

/// Reads HLTV's `status` reply.
fn parse_status(text: &str) -> Live {
    let num = |re: &str| regex::Regex::new(re).ok()?.captures(text)?.get(1)?.as_str().parse().ok();
    Live {
        spectators: num(r"Local Slots \d+, Spectators (\d+)"),
        slots: num(r"Local Slots (\d+)"),
        connected: text.contains("Connected to Game Server"),
        recording: regex::Regex::new(r"Recording to (.+?), Length")
            .ok()
            .and_then(|re| re.captures(text))
            .map(|c| c[1].trim().rsplit(['/', '\\']).next().unwrap_or("").to_string()),
    }
}

fn demo_dirs(e: &Entry) -> Vec<PathBuf> {
    let mut d = vec![e.dir.join(&e.game), e.dir.clone()];
    if e.game != "cstrike" {
        d.push(e.dir.join("cstrike"));
    }
    d
}

/// A demo's file name as the page sends it: a plain .dem name, nothing else.
fn safe_demo(name: &str) -> bool {
    name.len() <= 128
        && name.to_lowercase().ends_with(".dem")
        && name.len() > 4
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.starts_with('.')
}

fn find_demo(e: &Entry, name: &str) -> Option<PathBuf> {
    if !safe_demo(name) {
        return None;
    }
    demo_dirs(e).into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

fn demos(e: &Entry) -> Vec<Value> {
    if let Some(r) = &e.remote {
        let mut d = crate::remote::demos(r, &e.id, &e.game);
        d.retain(|(n, _, _)| safe_demo(n));
        d.sort_by(|a, b| b.2.cmp(&a.2));
        return d.into_iter().map(|(name, size, when)| json!({"name": name, "size": size, "modified": when})).collect();
    }
    let mut out: Vec<(u64, Value)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in demo_dirs(e) {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for f in rd.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            let Ok(meta) = f.metadata() else { continue };
            if !meta.is_file() || !safe_demo(&name) || !seen.insert(name.clone()) {
                continue;
            }
            let when = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            out.push((when, json!({"name": name, "size": meta.len(), "modified": when})));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|x| x.1).collect()
}

/// HLTV's console without the panel's own status checks: the "Executing rcon" line and the
/// status report that follows it, every few seconds. Lines with a password never show.
fn drop_status_checks(text: &str) -> Vec<String> {
    const REPORT: [&str; 8] = ["--- HLTV Status ---", "Online ", "Local IP ", "Local Slots ", "Total Slots ", "Connected to Game Server ", "Server Name ", "Game Time "];
    let mut out = Vec::new();
    let mut in_report = false;
    for l in text.lines() {
        if l.contains("Executing rcon") {
            in_report = true;
            continue;
        }
        if in_report && (REPORT.iter().any(|p| l.trim_start().starts_with(p)) || l.trim_start().starts_with("Recording to ")) {
            continue;
        }
        in_report = false;
        if !l.to_lowercase().contains("password") && !l.trim().is_empty() {
            out.push(l.to_string());
        }
    }
    out
}

/// The last lines of HLTV's console, without the panel's own status checks.
fn console(e: &Entry) -> Vec<String> {
    if let Some(r) = &e.remote {
        // Linux HLTV prints its console to the service's journal rather than the log file
        let cmd = format!(
            "f=\"$HOME/{}/{CONSOLE_LOG}\"; if [ -s \"$f\" ]; then tail -c 24576 \"$f\"; else sudo -n journalctl -u {} -n 400 --no-pager -o cat 2>/dev/null; fi",
            crate::remote::server_dir(&e.id),
            crate::remote::hltv_unit(&e.id)
        );
        let out = crate::remote::exec_quick(r, &cmd).map(|(_, o)| o).unwrap_or_default();
        let lines: Vec<String> = drop_status_checks(&out);
        let skip = lines.len().saturating_sub(80);
        return lines[skip..].to_vec();
    }
    for p in [e.dir.join(CONSOLE_LOG), e.dir.join(OUT_LOG)] {
        let Ok(mut f) = std::fs::File::open(&p) else { continue };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let _ = f.seek(SeekFrom::Start(len.saturating_sub(24 * 1024)));
        let mut buf = Vec::new();
        let _ = f.read_to_end(&mut buf);
        let lines: Vec<String> = crate::rcon::latin1(&buf)
            .lines()
            .filter(|l| !l.contains("Executing rcon") && !l.to_lowercase().contains("password") && !l.trim().is_empty())
            .map(String::from)
            .collect();
        if !lines.is_empty() {
            let skip = lines.len().saturating_sub(80);
            return lines[skip..].to_vec();
        }
    }
    Vec::new()
}

/// This computer's address on the local network.
fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("10.255.255.255:1").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

/// Where spectators connect: the server's public address (its host name with HLTV's port)
/// when one is set, else the VPS's address, or this computer's and this network's.
fn addresses(e: &Entry, port: u16, lan_ip: impl FnOnce() -> Option<String>) -> Value {
    let public = e.public_at(port);
    match &e.remote {
        Some(r) => json!({"computer": null, "network": null, "internet": public.unwrap_or_else(|| format!("{}:{port}", r.host))}),
        None => json!({"computer": format!("127.0.0.1:{port}"), "network": lan_ip().map(|ip| format!("{ip}:{port}")), "internet": public}),
    }
}

/// HLTV's state as last seen on the HLTV page, by server: (when, "running" / "stopped" / "starting").
static LAST_SEEN: Mutex<Option<HashMap<String, (Instant, String)>>> = Mutex::new(None);

/// HLTV's state for diagnostics, without asking the server's machine: on this computer
/// from its process, on another machine as the HLTV page last saw it.
pub fn last_state(e: &Entry) -> String {
    if e.remote.is_none() {
        let (on, pending, err) = with_runs(|r| (alive(r, &e.id), r.pending.contains_key(&e.id), r.errors.get(&e.id).cloned()));
        return match (on, pending, err) {
            (true, _, _) => "running".into(),
            (_, true, _) => "starting".into(),
            (_, _, Some(why)) => format!("stopped: {why}"),
            _ => "stopped".into(),
        };
    }
    match LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&e.id).cloned()) {
        Some((at, state)) => format!("{state} ({})", crate::diagnostics::ago(at.elapsed().as_secs())),
        None => "not checked yet".into(),
    }
}

/// HLTV's admin password (in the cfg it was started with), so diagnostics can leave it out.
pub fn admin_password(e: &Entry) -> Option<String> {
    let text = std::fs::read_to_string(e.dir.join(CFG)).ok()?;
    text.lines().find_map(|l| l.strip_prefix("adminpassword ")).map(|p| p.trim().trim_matches('"').to_string())
}

fn status(e: &Entry) -> Value {
    let (running, pending, error, uptime) = match &e.remote {
        Some(r) => {
            let (on, secs) = if e.hltv.enabled { crate::remote::unit_active(r, &crate::remote::hltv_unit(&e.id)) } else { (false, 0) };
            (on, false, None, secs)
        }
        None => with_runs(|r| {
            let running = alive(r, &e.id);
            let uptime = r.procs.get(&e.id).map(|p| p.started.elapsed().as_secs()).unwrap_or(0);
            (running, r.pending.contains_key(&e.id), r.errors.get(&e.id).cloned(), uptime)
        }),
    };
    let rcon = if running || (e.hltv.enabled && e.dir.join(CFG).is_file()) { admin(e) } else { None };
    let reply = rcon.and_then(|r| r.send("status").ok()).filter(|t| t.contains("HLTV"));
    // still up from before the app last closed
    let running = running || reply.is_some();
    let live = reply.as_deref().map(parse_status).unwrap_or_default();
    let state = if running {
        "running"
    } else if pending {
        "starting"
    } else {
        "stopped"
    };
    LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(e.id.clone(), (Instant::now(), state.to_string()));
    let port = port_of(e);
    json!({
        "available": e.remote.is_some() || exe(e).is_file(),
        "remote": e.remote.is_some(),
        "settings": {
            "enabled": e.hltv.enabled, "port": port, "delay": e.hltv.delay, "max_spectators": e.hltv.max_spectators,
            "name": e.hltv.name, "password": e.hltv.password, "record": e.hltv.record,
        },
        "default_name": display_name(&Entry { hltv: Settings::default(), ..e.clone() }),
        "default_port": default_port(e),
        "state": state,
        "error": if running { None } else { error },
        "uptime": uptime,
        "connected": live.connected,
        "spectators": live.spectators,
        "slots": live.slots,
        "recording": live.recording,
        "server_running": servers::running(&e.id),
        "addresses": addresses(e, port, lan_ip),
        "visibility": e.visibility,
        "demos": demos(e),
        "console": console(e),
    })
}

// ---- API -----------------------------------------------------------------------------------

fn forbidden() -> (StatusCode, Json<Value>) {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn answer(r: Result<Value, String>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> (StatusCode, Json<Value>) {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into())))
}

fn entry(id: &str) -> Result<Entry, String> {
    servers::get(id).ok_or_else(|| "That server isn't there any more.".to_string())
}

async fn api_status(UrlPath(id): UrlPath<String>) -> (StatusCode, Json<Value>) {
    blocking(move || entry(&id).map(|e| status(&e))).await
}

/// What the page sends: the effective port, which becomes "automatic" when it's the default.
#[derive(Deserialize)]
struct SaveReq {
    enabled: bool,
    port: u16,
    delay: u32,
    max_spectators: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    password: String,
    record: bool,
}

async fn api_save(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<SaveReq>) -> (StatusCode, Json<Value>) {
    // settings and turning it on or off: the owner (the computer or their phone), not helpers
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) {
        return forbidden();
    }
    blocking(move || {
        let e = entry(&id)?;
        let wanted = Settings { enabled: req.enabled, port: Some(req.port), delay: req.delay, max_spectators: req.max_spectators, name: req.name, password: req.password, record: req.record };
        let s = validate(wanted, &e, &servers::load())?;
        let changed = s != e.hltv;
        let save = || {
            servers::edit(|l| {
                if let Some(x) = l.iter_mut().find(|x| x.id == id) {
                    x.hltv = s.clone();
                }
            })
        };
        // on a Linux server: the service written again, and on or off with these settings,
        // before they're saved here (if the machine can't be reached nothing changes, and
        // saving again tries again)
        if e.remote.is_some() {
            if changed || s.enabled {
                remote_apply(&Entry { hltv: s.clone(), ..e.clone() }, s.enabled)?;
            }
            save()?;
            return Ok(json!({"ok": true, "restart": false}));
        }
        save()?;
        let e = entry(&id)?;
        let running = with_runs(|r| alive(r, &id) || r.pending.contains_key(&id));
        // turned off: stop it now; turned on with the server up: start it now
        if !s.enabled && running {
            stop(&Entry { hltv: Settings { enabled: true, ..s.clone() }, ..e.clone() });
        } else if s.enabled && !running && servers::running(&id) {
            start(&e)?;
        }
        let restart = s.enabled && running && changed;
        if restart {
            // kept with the server, so the page still says so after a reload
            servers::set_pending_restart(&id, servers::Pending::Hltv);
        }
        Ok(json!({"ok": true, "restart": restart}))
    })
    .await
}

#[derive(Deserialize, Default)]
struct ActionReq {
    #[serde(default)]
    name: String,
}

async fn api_action(State(st): State<Shared>, UrlPath((id, action)): UrlPath<(String, String)>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<ActionReq>) -> (StatusCode, Json<Value>) {
    // (copying a demo into your game is for the computer; deleting one too, not the phone)
    if !from_panel(&headers) || (action == "fetch-demo" && on.is_none()) || (action == "delete-demo" && phone.is_some()) {
        return forbidden();
    }
    blocking(move || {
        let e = entry(&id)?;
        match action.as_str() {
            "start" => {
                if !e.hltv.enabled {
                    return Err("Turn HLTV on first.".into());
                }
                if !servers::running(&id) {
                    return Err("Start the server first. HLTV starts with it.".into());
                }
                start(&e).map(|_| json!({"ok": true}))
            }
            "stop" => {
                stop(&Entry { hltv: Settings { enabled: true, ..e.hltv.clone() }, ..e });
                Ok(json!({"ok": true}))
            }
            "restart" => {
                stop(&Entry { hltv: Settings { enabled: true, ..e.hltv.clone() }, ..e.clone() });
                start(&e).map(|_| json!({"ok": true}))
            }
            "delete-demo" if e.remote.is_some() => {
                let r = e.remote.as_ref().unwrap();
                let path = (safe_demo(&req.name)).then(|| crate::remote::find_demo(r, &e.id, &e.game, &req.name)).flatten().ok_or("That demo isn't there any more.")?;
                let busy = admin(&e).and_then(|a| a.send("status").ok()).map(|t| parse_status(&t).recording.as_deref() == Some(req.name.as_str())).unwrap_or(false);
                if busy {
                    return Err("HLTV is still recording that demo. Stop HLTV first.".into());
                }
                let (code, out) = crate::remote::exec(r, &format!("rm -f -- \"$HOME/\"{}", crate::remote::shell_quote(&path)))?;
                if code != 0 {
                    return Err(format!("Couldn't delete it ({}).", out.trim()));
                }
                Ok(json!({"ok": true}))
            }
            "fetch-demo" => {
                // from the Linux server into the player's game folder, ready for viewdemo
                let r = e.remote.as_ref().ok_or("That demo is on this computer already.")?;
                let path = (safe_demo(&req.name)).then(|| crate::remote::find_demo(r, &e.id, &e.game, &req.name)).flatten().ok_or("That demo isn't there any more.")?;
                let game = st.local_game().ok_or("Set up the game on this computer first.")?;
                let data = crate::remote::read_file(r, &path)?;
                let to = game.hl.join(&e.game).join(&req.name);
                std::fs::write(&to, data).map_err(|err| format!("Couldn't save it ({err})."))?;
                Ok(json!({"ok": true}))
            }
            "delete-demo" => {
                let path = find_demo(&e, &req.name).ok_or("That demo isn't there any more.")?;
                let busy = with_runs(|r| alive(r, &id))
                    && admin(&e).and_then(|r| r.send("status").ok()).map(|t| parse_status(&t).recording.as_deref() == Some(req.name.as_str())).unwrap_or(false);
                if busy {
                    return Err("HLTV is still recording that demo. Stop HLTV first.".into());
                }
                std::fs::remove_file(path).map_err(|err| format!("Couldn't delete it ({err})."))?;
                Ok(json!({"ok": true}))
            }
            _ => Err("Unknown action.".into()),
        }
    })
    .await
}

/// A recorded demo, as a download (phones on the Wi-Fi too).
async fn api_demo(UrlPath((id, name)): UrlPath<(String, String)>) -> Response {
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    let found = tokio::task::spawn_blocking(move || {
        let e = servers::get(&id)?;
        if let Some(r) = &e.remote {
            let path = safe_demo(&name).then(|| crate::remote::find_demo(r, &e.id, &e.game, &name)).flatten()?;
            return crate::remote::read_file(r, &path).ok().map(|d| (name, d));
        }
        let path = find_demo(&e, &name)?;
        std::fs::read(path).ok().map(|d| (name, d))
    })
    .await
    .ok()
    .flatten();
    let Some((name, data)) = found else { return not_found() };
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", name.replace('"', "")))
        .header(header::CONTENT_LENGTH, data.len())
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(data))
        .unwrap_or_else(|_| not_found())
}

/// Shows the demos in Finder or Explorer (a demo picked, if one is named). App window only.
async fn api_folder(UrlPath(id): UrlPath<String>, headers: HeaderMap, Json(req): Json<ActionReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    blocking(move || {
        let e = entry(&id)?;
        if e.remote.is_some() {
            return Err("The demos are on the server. Use Copy to my game to copy one to your game.".into());
        }
        let demo = (!req.name.is_empty()).then(|| find_demo(&e, &req.name)).flatten();
        let dir = demos(&e)
            .first()
            .and_then(|d| d["name"].as_str().and_then(|n| find_demo(&e, n)))
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| e.dir.join(&e.game));
        #[cfg(target_os = "macos")]
        let r = match &demo {
            Some(p) => std::process::Command::new("open").arg("-R").arg(p).spawn().map(crate::sys::reap),
            None => std::process::Command::new("open").arg(&dir).spawn().map(crate::sys::reap),
        };
        #[cfg(windows)]
        let r = match &demo {
            Some(p) => std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn().map(crate::sys::reap),
            None => std::process::Command::new("explorer").arg(&dir).spawn().map(crate::sys::reap),
        };
        #[cfg(not(any(target_os = "macos", windows)))]
        let r: std::io::Result<()> = {
            let _ = (&demo, &dir);
            Err(std::io::Error::other("unsupported"))
        };
        r.map(|_| json!({"ok": true})).map_err(|err| format!("Couldn't open the folder ({err})."))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/servers/:id/hltv", route_get(api_status).post(api_save))
        .route("/api/servers/:id/hltv/demos/:name", route_get(api_demo))
        .route("/api/servers/:id/hltv/:action", post(api_action))
}

/// Only for the app window: opening a folder on this computer.
pub fn local_routes() -> Router<Shared> {
    Router::new().route("/api/servers/:id/hltv-folder", post(api_folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str, port: u16) -> Entry {
        serde_json::from_value(json!({"id": id, "name": format!("Server {id}"), "game": "cstrike", "port": port, "dir": "/tmp/x", "kind": "crossover", "ready": true}))
            .unwrap()
    }

    #[test]
    fn spectator_addresses() {
        let mut e = server("a", 27016);
        let lan = || Some("192.168.1.20".to_string());
        let a = addresses(&e, 27020, lan);
        assert_eq!(a["computer"], "127.0.0.1:27020");
        assert_eq!(a["network"], "192.168.1.20:27020");
        assert!(a["internet"].is_null(), "nothing set: no internet address");
        e.public_address = Some("home.example.com".into());
        assert_eq!(addresses(&e, 27020, lan)["internet"], "home.example.com:27020");
        e.remote = serde_json::from_value(json!({"host": "178.156.241.180", "key": "/k"})).unwrap();
        e.public_address = None;
        let a = addresses(&e, 27020, lan);
        assert_eq!(a["internet"], "178.156.241.180:27020");
        assert!(a["network"].is_null());
        // a port in the public address is the game's, not HLTV's
        e.public_address = Some("play.lonerstrike.com:27016".into());
        assert_eq!(addresses(&e, 27020, lan)["internet"], "play.lonerstrike.com:27020");
    }

    #[test]
    fn old_servers_get_default_settings() {
        let e = server("a", 27016);
        assert_eq!(e.hltv, Settings::default());
        assert!(!e.hltv.enabled);
        assert_eq!(port_of(&e), 28016);
    }

    #[test]
    fn command_line_and_config() {
        let mut e = server("a", 27017);
        e.visibility = "lan".into(); // answers on 127.0.0.1 (see servers::Entry::bind_ip)
        e.hltv = Settings { enabled: true, port: Some(27500), delay: 45, max_spectators: 10, name: "Cup \"Final\"; quit".into(), password: "watch-me".into(), record: true };
        assert_eq!(launch_args(&e), vec!["-port", "27500", "+exec", CFG]);
        let c = config(&e, "adminpw", Some("joinpw"));
        assert!(c.contains("connect 127.0.0.1:27017\n"));
        assert!(c.contains("delay 45\n") && c.contains("maxclients 10\n"));
        assert!(c.contains("name \"Cup Final quit\"\n"), "{c}");
        assert!(c.contains("spectatorpassword \"watch-me\"") && c.contains("serverpassword \"joinpw\"") && c.contains("adminpassword \"adminpw\""));
        assert!(c.contains("record hltv\n") && c.contains("nomaster 1") && c.contains("autoretry 1"));
        // record comes after connect; nothing in it starts a second line by itself
        assert!(c.find("connect ").unwrap() < c.find("record ").unwrap());

        e.hltv = Settings::default();
        e.visibility = "listed".into();
        let c = config(&e, "x", Some("bad \"pw\""));
        assert!(!c.contains("record") && !c.contains("spectatorpassword") && !c.contains("serverpassword"));
        assert!(c.contains("name \"Server a TV\"") && c.contains("nomaster 0"));
        assert_eq!(launch_args(&e)[1], "28017");
    }

    #[test]
    fn ports_are_checked() {
        let a = server("a", 27016);
        let mut b = server("b", 27017);
        b.hltv.enabled = true; // on 28017
        let all = vec![a.clone(), b.clone()];
        let on = |port: u16| Settings { enabled: true, port: Some(port), ..Settings::default() };
        assert_eq!(validate(on(28016), &a, &all).unwrap().port, None, "the default port is stored as automatic");
        assert_eq!(validate(on(28100), &a, &all).unwrap().port, Some(28100));
        assert!(validate(on(27016), &a, &all).is_err(), "its own server's port");
        assert!(validate(on(27017), &a, &all).is_err(), "another server's port");
        assert!(validate(on(28017), &a, &all).is_err(), "another server's HLTV");
        assert!(validate(on(27015), &a, &all).is_err(), "the player's own game");
        assert!(validate(on(80), &a, &all).is_err() && validate(on(65535), &a, &all).is_err());
        // off: the port isn't checked, so other settings still save
        assert!(validate(Settings { enabled: false, ..on(27017) }, &a, &all).is_ok());
    }

    #[test]
    fn other_settings_are_checked() {
        let a = server("a", 27016);
        let all = vec![a.clone()];
        let s = |f: &dyn Fn(&mut Settings)| {
            let mut x = Settings { enabled: true, ..Settings::default() };
            f(&mut x);
            validate(x, &a, &all)
        };
        assert!(s(&|x| x.password = "has space".into()).is_err());
        assert!(s(&|x| x.password = "q\"uote".into()).is_err());
        assert_eq!(s(&|x| x.password = "  ok_pw.1  ".into()).unwrap().password, "ok_pw.1");
        assert!(s(&|x| x.delay = 601).is_err() && s(&|x| x.delay = 0).is_ok());
        assert!(s(&|x| x.max_spectators = 256).is_err());
        assert!(s(&|x| x.max_spectators = 0).is_err(), "nobody to watch and nothing recorded");
        assert!(s(&|x| { x.max_spectators = 0; x.record = true; }).is_ok());
        assert_eq!(s(&|x| x.name = "  A\tB\"C;D  ".into()).unwrap().name, "ABCD");
    }

    #[test]
    fn status_reply() {
        let text = "\u{1}--- HLTV Status ---\nOnline 00:23, FPS 79.0, Version 2435 (Win32)\nLocal IP 192.168.1.2:27020, Network In 1.7, Out 1.0, Loss 0.00\n\
                    Local Slots 32, Spectators 3 (max 5), Proxies 0\nTotal Slots 32, Spectators 3 (max 5), Proxies 1\n\
                    Connected to Game Server 127.0.0.1:27016, Delay 30\nRecording to cstrike/hltv-2611061830-de_dust2.dem, Length 12.5 sec.\n";
        let l = parse_status(text);
        assert_eq!(l, Live { spectators: Some(3), slots: Some(32), connected: true, recording: Some("hltv-2611061830-de_dust2.dem".into()) });
        assert_eq!(parse_status("Not connected.\n"), Live::default());
    }

    #[test]
    fn routes_fit_with_the_servers_routes() {
        let _ = crate::servers::routes().merge(routes()).merge(crate::servers::local_routes()).merge(local_routes());
    }

    #[test]
    fn demo_names() {
        assert!(safe_demo("hltv-2611061830-de_dust2.dem") && safe_demo("A.DEM"));
        assert!(!safe_demo("../server.cfg") && !safe_demo("x/../../a.dem") && !safe_demo("..\\a.dem") && !safe_demo(".dem") && !safe_demo("a.dem.txt") && !safe_demo("C:a.dem"));
    }
    #[test]
    fn console_drops_status_checks() {
        let t = "Connected to Game Server 127.0.0.1:27015\nExecuting rcon \"status\" from 127.0.0.1:1.\n--- HLTV Status ---\nOnline 00:46, FPS 94.1\nLocal IP 127.0.1.1:27020\nGame Time 22:18, Mod \"cstrike\"\nSpectator joined: bob\nserverpassword secret\n";
        assert_eq!(drop_status_checks(t), ["Connected to Game Server 127.0.0.1:27015", "Spectator joined: bob"]);
    }
}
