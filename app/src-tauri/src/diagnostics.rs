//! "Copy diagnostics" in Settings: a short plain-text report for bug reports, and the
//! app's error log it ends with.
//!
//! Nothing secret goes in: no passwords, tokens, RCON, SSH keys or the home's public
//! address. Known secrets (RCON and join passwords, HLTV's, tunnel tokens, the playit key,
//! the phone token) are taken out by value, and everything is masked by pattern too
//! (`mask`): IP addresses keep only their first number (178.x.x.x), "password ..." and
//! "token ..." lose their value, and long key-like strings are hidden.

use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ---- the error log ---------------------------------------------------------------------

/// Errors the app showed or met in the background, one line each, masked as they're written.
fn log_path() -> PathBuf {
    crate::sys::app_dir().join("errors.log")
}

/// Kept at most this big: past it, the older half goes.
const LOG_MAX: u64 = 256 * 1024;
/// The same message from the same place is written once in this long (a page that polls
/// an error would fill the log otherwise).
const REPEAT_AFTER: Duration = Duration::from_secs(600);

/// Notes an error in the app's log. `place`: where it came from (an API path, "sync").
pub fn log_error(place: &str, msg: &str) {
    log_error_to(&log_path(), place, msg)
}

fn log_error_to(path: &Path, place: &str, msg: &str) {
    let msg = msg.trim();
    if msg.is_empty() {
        return;
    }
    static SEEN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
    {
        let key = format!("{}|{place}|{msg}", path.display());
        let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        let seen = seen.get_or_insert_with(HashMap::new);
        if seen.get(&key).is_some_and(|t| t.elapsed() < REPEAT_AFTER) {
            return;
        }
        if seen.len() > 500 {
            seen.retain(|_, t| t.elapsed() < REPEAT_AFTER);
        }
        seen.insert(key, Instant::now());
    }
    let line = format!("{} {}: {}\n", crate::sys::stamp(), place, mask(&msg.replace(['\r', '\n'], " ")));
    static WRITE: Mutex<()> = Mutex::new(());
    let _w = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > LOG_MAX) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let keep = lines[lines.len() / 2..].join("\n");
        let _ = crate::sys::write_atomic(path, format!("{keep}\n").as_bytes());
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// The last `n` lines of the error log.
fn recent_errors(path: &Path, n: usize) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].iter().map(|l| l.to_string()).collect()
}

/// Middleware: an answer with an "error" (what the page shows the player) goes in the log.
pub async fn log_errors(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    use axum::body::{Body, HttpBody};
    let place = format!("{} {}", req.method(), req.uri().path());
    let res = next.run(req).await;
    let json = res.headers().get(axum::http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|t| t.starts_with("application/json"));
    // only small, whole answers (never a stream or a download)
    let small = res.body().size_hint().exact().is_some_and(|n| n <= 64 * 1024);
    if !json || !small {
        return res;
    }
    let (parts, body) = res.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, 64 * 1024).await else {
        return axum::response::Response::from_parts(parts, Body::empty());
    };
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        if let Some(e) = v.get("error").and_then(|e| e.as_str()).filter(|e| !e.is_empty() && *e != "forbidden") {
            let (place, e) = (place.clone(), e.to_string());
            tokio::task::spawn_blocking(move || log_error(&place, &e));
        }
    }
    axum::response::Response::from_parts(parts, Body::from(bytes))
}

// ---- masking ---------------------------------------------------------------------------

/// Takes secrets out of text by pattern: IP addresses (but 127.0.0.1 and 0.0.0.0) keep
/// only their first number, IPv6 addresses go, "password x" / "token=x" / "rcon_password
/// x" lose their value, private key blocks and long key-like strings are hidden.
pub fn mask(text: &str) -> String {
    static RE: OnceLock<[Regex; 5]> = OnceLock::new();
    let [pem, kv, ip4, ip6, long] = RE.get_or_init(|| {
        [
            Regex::new(r"-----BEGIN [A-Z0-9 ]+-----[\s\S]*?(-----END [A-Z0-9 ]+-----|$)").unwrap(),
            Regex::new(r#"(?i)\b(rcon_password|sv_password|adminpassword|proxypassword|spectatorpassword|password|passwd|pass|pw|token|secret|api_?key|authorization|bearer)\b(\s*[:=]\s*|\s+)("[^"]*"|'[^']*'|[^\s,;]+)"#).unwrap(),
            Regex::new(r"\b(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\b").unwrap(),
            Regex::new(r"(?i)\b(?:[0-9a-f]{1,4}:){3,7}[0-9a-f]{1,4}\b").unwrap(),
            Regex::new(r"[A-Za-z0-9_\-+=]{24,}").unwrap(),
        ]
    });
    let t = pem.replace_all(text, "[private key hidden]");
    let t = kv.replace_all(&t, "$1$2[hidden]");
    let t = ip4.replace_all(&t, |c: &regex::Captures| {
        let n: Vec<u32> = (1..=4).map(|i| c[i].parse().unwrap_or(999)).collect();
        if n.iter().any(|x| *x > 255) || n == [127, 0, 0, 1] || n == [0, 0, 0, 0] {
            c[0].to_string()
        } else {
            format!("{}.x.x.x", n[0])
        }
    });
    let t = ip6.replace_all(&t, "[ipv6]");
    let t = long.replace_all(&t, |c: &regex::Captures| {
        let s = &c[0];
        let digit = s.chars().any(|c| c.is_ascii_digit());
        let letter = s.chars().any(|c| c.is_ascii_alphabetic());
        if digit && letter {
            "[hidden]".to_string()
        } else {
            s.to_string()
        }
    });
    t.into_owned()
}

/// `mask`, after taking out each of `secrets` by value (the short ones too, which no
/// pattern would catch).
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut t = text.to_string();
    let mut s: Vec<&String> = secrets.iter().filter(|s| s.trim().len() >= 3).collect();
    // longer first, so one that contains another goes whole
    s.sort_by_key(|x| std::cmp::Reverse(x.len()));
    for x in s {
        t = t.replace(x.trim(), "[hidden]");
    }
    mask(&t)
}

// ---- the report ------------------------------------------------------------------------

/// "12s ago", "5 min ago", "3 h ago", "2 days ago".
pub fn ago(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=172_799 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// What the report says about one server, gathered without asking its machine anything.
pub struct ServerFacts {
    pub entry: crate::servers::Entry,
    /// "running", "stopped", "crashed", "not checked yet", "setting up", "setup failed"
    pub state: String,
    /// why the machine didn't answer, or why the server stopped
    pub error: Option<String>,
    /// HLTV: "running", "stopped", "starting", "not checked yet"
    pub hltv: String,
    /// the status website's last check: (seconds ago, state, why)
    pub web: Option<(u64, String, Option<String>)>,
    /// the last upload: (seconds ago, Ok((sent, removed)) or Err(why))
    pub sync: Option<(u64, Result<(usize, usize), String>)>,
}

/// Everything the report is made from (gathered by `gather`, or by hand in tests).
pub struct Facts {
    pub version: String,
    /// (folder, name) of the game the app is set to
    pub active: Option<(PathBuf, String)>,
    pub installed: Vec<String>,
    pub outdated: Vec<String>,
    /// one line per game install found
    pub installs: Vec<String>,
    pub extra_folders: usize,
    pub servers: Vec<ServerFacts>,
    pub settings: String,
    pub smart_chat: bool,
    pub game_running: bool,
    pub map_copy: Option<String>,
    /// (which file, its last lines)
    pub console: Option<(String, Vec<String>)>,
    /// the last setup job: (kind, state, error, notes, didn't compile)
    pub job: Option<(String, String, Option<String>, Vec<String>, Vec<String>)>,
    pub errors: Vec<String>,
    /// values to take out wherever they show up
    pub secrets: Vec<String>,
}

fn server_lines(f: &ServerFacts) -> Vec<String> {
    let e = &f.entry;
    let kind = match &e.remote {
        Some(r) => format!("remote at {}", r.host),
        None => format!("local ({})", e.kind),
    };
    let mut out = vec![format!(
        "Server \"{}\": {kind}, {}, port {}, {}, {}",
        e.name,
        e.game,
        e.port,
        e.visibility,
        if e.ready { f.state.clone() } else { format!("not ready ({})", f.state) },
    )];
    let p = &e.pending_restart;
    let pending: Vec<&str> = [(p.files, "files"), (p.options, "options"), (p.hltv, "hltv"), (p.sounds, "sounds")].iter().filter(|x| x.0).map(|x| x.1).collect();
    out.push(format!(
        "  pending restart: {}; service stale: {}; bots when empty: {}; public address set: {}{}",
        if pending.is_empty() { "none".into() } else { pending.join(", ") },
        yes(e.service_stale),
        yes(crate::servers::bots_when_empty(e)),
        yes(e.public_address.is_some()),
        if e.imported { "; imported" } else { "" },
    ));
    let w = &e.web_status;
    let web = if !w.enabled {
        "off".to_string()
    } else {
        let mode = if w.mode == "tunnel" { if w.own_tunnel { "tunnel (app's own)" } else { "tunnel (existing)" } } else { "caddy" };
        let last = match &f.web {
            Some((secs, state, why)) => format!("{state} ({}){}", ago(*secs), why.as_ref().map(|w| format!(": {w}")).unwrap_or_default()),
            None => "not checked yet".into(),
        };
        format!("on, {mode}, {}, last check {last}", if w.domain.is_empty() { "no domain" } else { &w.domain })
    };
    out.push(format!("  website status: {web}"));
    out.push(format!("  HLTV: {}", if e.hltv.enabled { format!("on, {}", f.hltv) } else { "off".into() }));
    if e.remote.is_some() {
        out.push(format!(
            "  last sync: {}",
            match &f.sync {
                Some((secs, Ok((sent, removed)))) => format!("ok {}, {sent} sent, {removed} removed", ago(*secs)),
                Some((secs, Err(why))) => format!("failed {}: {why}", ago(*secs)),
                None => "none this session".into(),
            }
        ));
    }
    if let Some(err) = &f.error {
        out.push(format!("  last error: {err}"));
    }
    out
}

/// The report, secrets taken out.
pub fn build(f: &Facts) -> String {
    let mut out = vec![format!("Loner-Strike {} on {} {}", f.version, std::env::consts::OS, std::env::consts::ARCH)];
    match &f.active {
        Some((dir, name)) => {
            out.push(format!("Active game: {name} at {}", dir.display()));
            out.push(format!("Installed: {}", if f.installed.is_empty() { "nothing".into() } else { f.installed.join(", ") }));
            out.push(format!("Outdated: {}", if f.outdated.is_empty() { "none".into() } else { f.outdated.join(", ") }));
        }
        None => out.push("Active game: none found".into()),
    }
    out.extend(f.installs.iter().map(|i| format!("Game folder: {i}")));
    out.push(format!("Game folders added by hand: {}", f.extra_folders));
    out.push(f.settings.clone());
    out.push(format!("Smarter chat enabled: {}", yes(f.smart_chat)));
    out.push(format!("Game running: {}", yes(f.game_running)));
    out.push(format!("Servers: {}", f.servers.len()));
    for s in &f.servers {
        out.extend(server_lines(s));
    }
    out.push(format!("Last map copy: {}", f.map_copy.clone().unwrap_or_else(|| "none this session".into())));
    if let Some((kind, state, error, notes, failed)) = &f.job {
        out.push(format!("Last setup: {kind} ({state}){}", error.as_ref().map(|e| format!(": {e}")).unwrap_or_default()));
        out.extend(notes.iter().take(10).map(|n| format!("  note: {n}")));
        if !failed.is_empty() {
            out.push(format!("  didn't compile: {}", failed.join(", ")));
        }
    }
    if let Some((file, lines)) = &f.console {
        out.push(format!("Last game console lines ({file}):"));
        out.extend(lines.iter().map(|l| format!("  {l}")));
    }
    out.push(format!("Recent errors ({}):", f.errors.len()));
    out.extend(f.errors.iter().map(|l| format!("  {l}")));
    redact(&out.join("\n"), &f.secrets)
}

/// The secrets a game or server folder keeps: the panel's RCON password and the join password.
fn folder_secrets(game_dir: &Path) -> Vec<String> {
    let data = game_dir.join("addons/amxmodx/data/gamemodes");
    let mut out = Vec::new();
    if let Ok(t) = std::fs::read_to_string(data.join("panel_rcon.txt")) {
        out.push(t.trim().to_string());
    }
    if let Some(p) = std::fs::read_to_string(data.join("friends.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v["password"].as_str().map(String::from)) {
        out.push(p);
    }
    out
}

/// Gathers the report's facts. Never asks a server's machine anything (that can take
/// long, and the report is for when things are broken): states are the last ones seen.
pub fn gather(st: &crate::server::Shared, version: String) -> Facts {
    let s = crate::app_api::load_settings();
    let game = st.local_game();
    let mut secrets = vec![st.token.read().unwrap().clone()];
    if let Ok(t) = std::fs::read_to_string(crate::sys::app_dir().join("playit/secret.txt")) {
        secrets.push(t.trim().to_string());
    }
    let (active, installed, outdated) = match &game {
        Some(g) => {
            let rec = crate::setup::record::Record::open(&g.dir);
            let outdated = crate::setup_api::resources_for(st).map(|r| crate::setup::outdated(&r, &rec)).unwrap_or_default();
            secrets.extend(folder_secrets(&g.dir));
            (Some((g.dir.clone(), g.name.clone())), rec.component_ids(), outdated)
        }
        None => (None, vec![], vec![]),
    };
    let installs = crate::steam::find_installs()
        .into_iter()
        .map(|i| format!("{} ({}{}{}) games: {}", i.hl.display(), i.kind, i.bottle.map(|b| format!(", bottle {b}")).unwrap_or_default(), if i.dedicated { ", server" } else { "" }, i.games.join(", ")))
        .collect();
    let servers = crate::servers::load()
        .into_iter()
        .map(|e| {
            secrets.extend(folder_secrets(&e.dir.join(&e.game)));
            secrets.push(e.hltv.password.clone());
            secrets.extend(crate::hltv::admin_password(&e));
            secrets.extend(crate::webstatus::saved_token(&e.id));
            let (state, error) = crate::servers::last_state(&e);
            ServerFacts {
                hltv: crate::hltv::last_state(&e),
                web: crate::webstatus::last_check(&e.id),
                sync: crate::servers::last_sync(&e.id),
                state,
                error,
                entry: e,
            }
        })
        .collect();
    let settings = format!(
        "Settings: phone access {}, updates {}, keep servers running {}",
        if s.phone { "on" } else { "off" },
        if s.manual_updates { "manual" } else { "automatic" },
        if s.keep_servers { "on" } else { "off" },
    );
    // the end of the game's console (written with -condebug), for crashes
    let console = game.as_ref().and_then(|g| {
        [g.hl.join("qconsole.log"), g.dir.join("qconsole.log")].into_iter().find_map(|p| {
            let text = crate::sys::read_latin1(&p).ok()?;
            let lines: Vec<String> = text.lines().filter(|l| !l.to_lowercase().contains("password")).map(String::from).collect();
            Some((p.display().to_string(), lines[lines.len().saturating_sub(30)..].to_vec()))
        })
    });
    let job = crate::setup::current();
    let job = (job.state != "idle").then(|| (job.kind, job.state, job.error, job.notes, job.failed_compiles));
    Facts {
        version,
        active,
        installed,
        outdated,
        installs,
        extra_folders: s.game_folders.len(),
        servers,
        settings,
        smart_chat: s.smart_chat,
        game_running: crate::sys::game_running(),
        map_copy: crate::mapcopy::last_summary(),
        console,
        job,
        errors: recent_errors(&log_path(), 30),
        secrets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RCON: &str = "rc0nSecret!";
    const JOIN: &str = "joinme42";
    const HLTV_PW: &str = "tvpass";
    const TUNNEL: &str = "eyJhIjoiNGQ2ZTFhYjM0NTY3ODkwYWJjZGVmIiwidCI6IjEyMzQ1Njc4LWFiY2QtZWZnaCJ9";
    const PHONE: &str = "Qm9yZWQ1dGhpc2lzYXRva2Vu";
    const HOME_IP: &str = "178.62.14.201";
    const KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\n-----END OPENSSH PRIVATE KEY-----";

    fn fixture() -> Facts {
        let mut local: crate::servers::Entry = serde_json::from_value(serde_json::json!({
            "id": "s1", "name": "Alone Together", "game": "cstrike", "port": 27016, "dir": "/tmp/s1", "kind": "windows",
            "bottle": null, "visibility": "listed", "ready": true,
        }))
        .unwrap();
        local.hltv.enabled = true;
        local.hltv.password = HLTV_PW.into();
        let mut remote = local.clone();
        remote.id = "s2".into();
        remote.name = "VPS".into();
        remote.remote = Some(serde_json::from_value(serde_json::json!({"host": HOME_IP, "key": "/Users/me/.ssh/id_ed25519"})).unwrap());
        remote.service_stale = true;
        remote.pending_restart.options = true;
        remote.public_address = Some("play.example.com".into());
        remote.web_status.enabled = true;
        remote.web_status.mode = "tunnel".into();
        remote.web_status.own_tunnel = true;
        remote.web_status.domain = "status.example.com".into();
        Facts {
            version: "0.1.0".into(),
            active: Some((PathBuf::from("/games/Half-Life/czero"), "czero".into())),
            installed: vec!["foundation".into(), "core".into()],
            outdated: vec!["core".into()],
            installs: vec!["/games/Half-Life (windows) games: czero, cstrike".into()],
            extra_folders: 1,
            servers: vec![
                ServerFacts { entry: local, state: "crashed".into(), error: Some(format!("rcon_password {RCON} was refused")), hltv: "running".into(), web: None, sync: None },
                ServerFacts {
                    entry: remote,
                    state: "running".into(),
                    error: Some(format!("ssh to {HOME_IP}:22 timed out")),
                    hltv: "not checked yet".into(),
                    web: Some((30, "not_answering".into(), Some(format!("status.example.com points at {HOME_IP}, not at this server")))),
                    sync: Some((5, Err(format!("cloudflared service install {TUNNEL} failed")))),
                },
            ],
            settings: "Settings: phone access on".into(),
            smart_chat: false,
            game_running: false,
            map_copy: Some("copied 2, already there 1, skipped 0, failed 0".into()),
            console: Some(("qconsole.log".into(), vec![format!("sv_password \"{JOIN}\""), format!("Connection from {HOME_IP}:27005")])),
            job: Some(("server".into(), "failed".into(), Some(format!("Couldn't log in with {KEY}")), vec![], vec![])),
            errors: vec![format!("20261106-101010 POST /api/share: token={PHONE} expired"), format!("20261106-101011 sync: {JOIN} rejected")],
            secrets: vec![RCON.into(), JOIN.into(), HLTV_PW.into(), TUNNEL.into(), PHONE.into()],
        }
    }

    #[test]
    fn never_shows_a_secret() {
        let text = build(&fixture());
        for s in [RCON, JOIN, HLTV_PW, TUNNEL, PHONE, HOME_IP, "b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW", "62.14.201"] {
            assert!(!text.contains(s), "{s} leaked:\n{text}");
        }
        // what it should still say
        for s in ["Loner-Strike 0.1.0", "Active game: czero", "Outdated: core", "remote at 178.x.x.x", "service stale: yes", "pending restart: options",
                  "public address set: yes", "tunnel (app's own), status.example.com, last check not_answering", "HLTV: on, running", "last sync: failed 5s ago",
                  "Smarter chat enabled: no", "Last map copy: copied 2", "Recent errors (2):"] {
            assert!(text.contains(s), "missing {s:?}:\n{text}");
        }
    }

    #[test]
    fn masks_by_pattern_alone() {
        // with no list of secrets to go by
        let t = mask("rcon_password \"hunter2\" sv_password=abc token: xyz from 81.2.69.160 and 127.0.0.1, ReGameDLL 5.30.0.814, key 9f74a1455538ca9d6f3bbf4d91cfca515c1dbb48 at fe80:0:0:0:200:f8ff:fe21:67cf");
        for s in ["hunter2", "abc", "xyz", "81.2.69.160", "9f74a1455538", "fe21:67cf"] {
            assert!(!t.contains(s), "{s} in {t}");
        }
        for s in ["81.x.x.x", "127.0.0.1", "5.30.0.814"] {
            assert!(t.contains(s), "{s} not in {t}");
        }
    }

    #[test]
    fn log_skips_repeats_and_masks() {
        let dir = crate::sys::make_temp_dir(&std::env::temp_dir(), "ls-diag").unwrap();
        let log = dir.join("errors.log");
        log_error_to(&log, "GET /api/x", "Couldn't reach 203.0.113.9 (password nope)");
        log_error_to(&log, "GET /api/x", "Couldn't reach 203.0.113.9 (password nope)");
        log_error_to(&log, "sync", "another");
        let lines = recent_errors(&log, 30);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("203.x.x.x") && !lines[0].contains("nope"), "{lines:?}");
        assert_eq!(recent_errors(&log, 1).len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
