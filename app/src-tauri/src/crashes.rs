//! Crash tracking for the player's servers (agent MCP; the capture on the servers is Y's,
//! docs/notes/Y.md "crash tracking"). For each server: its crashes, newest first (time, map,
//! the fault, the stack, the last log lines), how many in the last 24 hours, and a warning
//! when it runs a module known to crash servers (the official Ham Sandwich first of all).
//!
//! Where the crashes come from, in the server's game folder (the same relative paths on a
//! Linux machine, a Windows PC and a Mac's CrossOver server):
//! - `crashes/*.json`, one file per crash (`loner-strike/crash` version 1), and
//!   `crashes/modules.json` (`loner-strike/modules`, the module check at each start);
//! - until those exist, the bot brain's own `ls_crash*.log` (time, exception, stack, marks).
//!
//! Routes (the app window's and the phone's, like the security check):
//!   GET  /api/servers/:id/crashes            the list, the 24-hour count, warnings, alerts
//!   POST /api/servers/:id/crashes/alerts     {discord: bool}: post new crashes to the server's
//!                                            Discord status channel (its "live" webhook)
//!   GET  /api/crashes/summary                {id: {last24h, warnings}} from what's known (for
//!                                            badges; stale ones are read again in the background)
//! A watcher posts new crashes to Discord for servers that asked for it, while the app runs.

use crate::servers::Entry;
use axum::extract::{Path as UrlPath, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Loner-Strike's fixed Ham Sandwich (resources/*/hamsandwich: the official module uses
/// freed memory on map changes and crashes servers). Test `fixed_ham_hashes_match_the_resources`.
pub const FIXED_HAM_WINDOWS: &str = "d1ebbf716f3a7a65dbc59eacb4da4a00ab7eb84b1308a2a8b79338ea4b3ebdeb";
pub const FIXED_HAM_LINUX: &str = "dd398607f0dd15d22493dce920eb59f5bc62d5f6710205bff8bbd42c1202510e";

/// Most crash files read per server (newest first) and the most bytes of one.
const MAX_FILES: usize = 60;
const MAX_FILE: u64 = 1 << 20;
/// How long a server's crash list is kept before it's read again.
const FRESH: Duration = Duration::from_secs(120);
/// The bot brain's fault when a debugger attaches, or at a stop (Y.md "the 0xfff4fadc crash").
const NOT_A_CRASH: &str = "0xfff4fadc";

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---- reading what a server has --------------------------------------------------------------

/// One file of a server's: its name (relative to the game folder), when it was written, its bytes.
struct Found {
    rel: String,
    mtime: u64,
    bytes: Vec<u8>,
}

/// What a server has: crash files, the module check, and Ham Sandwich's hash (when it's there).
struct Raw {
    files: Vec<Found>,
    modules: Option<Vec<u8>>,
    ham: Option<String>,
    /// "windows" (a Windows PC, a Mac's CrossOver server) or "linux"
    os: &'static str,
}

fn ham_rel(os: &str) -> &'static str {
    if os == "linux" {
        "addons/amxmodx/modules/hamsandwich_amxx_i386.so"
    } else {
        "addons/amxmodx/modules/hamsandwich_amxx.dll"
    }
}

fn wanted(name: &str) -> bool {
    let safe = !name.is_empty() && name.len() <= 96 && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c));
    safe && ((name.starts_with("ls_crash") && name.ends_with(".log")) || name.ends_with(".json"))
}

fn read_local(game: &Path) -> Raw {
    let mut list: Vec<(u64, String, PathBuf)> = vec![];
    for (sub, dir) in [("crashes/", game.join("crashes")), ("", game.to_path_buf())] {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten().take(5000) {
            let name = e.file_name().to_string_lossy().to_string();
            let ok = if sub.is_empty() { name.starts_with("ls_crash") && name.ends_with(".log") } else { name.ends_with(".json") && name != "modules.json" };
            if !ok || !wanted(&name) {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            if !m.is_file() || m.len() > MAX_FILE {
                continue;
            }
            let t = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            list.push((t, format!("{sub}{name}"), e.path()));
        }
    }
    list.sort_by(|a, b| b.0.cmp(&a.0));
    let files = list.into_iter().take(MAX_FILES).filter_map(|(mtime, rel, p)| std::fs::read(p).ok().map(|bytes| Found { rel, mtime, bytes })).collect();
    let modules = std::fs::read(game.join("crashes/modules.json")).ok().filter(|b| b.len() as u64 <= MAX_FILE);
    let ham = std::fs::read(game.join(ham_rel("windows"))).ok().map(|b| hex(&Sha256::digest(&b)));
    Raw { files, modules, ham, os: "windows" }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn read_remote(r: &crate::remote::RemoteInfo, id: &str, game: &str) -> Result<Raw, String> {
    let dir = format!("{}/{game}", crate::remote::server_dir(id));
    // newest first: "<mtime> <size> <name>" per file, then Ham Sandwich's hash
    let cmd = format!(
        "cd {d} 2>/dev/null || exit 0; ls -1t crashes/*.json ls_crash*.log 2>/dev/null | grep -v '^crashes/modules.json$' | head -{MAX_FILES} | while read -r f; do echo \"F $(stat -c '%Y %s' \"$f\") $f\"; done; \
         [ -f {h} ] && echo \"H $(sha256sum {h} | cut -c1-64)\"; true",
        d = crate::remote::shell_quote(&dir),
        h = ham_rel("linux")
    );
    let (code, out) = crate::remote::exec_quick(r, &cmd)?;
    if code != 0 {
        return Err(format!("Couldn't read the server's crash files ({}).", out.trim()));
    }
    let mut ham = None;
    let mut list: Vec<(u64, String)> = vec![];
    for l in out.lines() {
        let parts: Vec<&str> = l.split_whitespace().collect();
        match parts.as_slice() {
            ["F", t, size, rel] => {
                let name = rel.rsplit('/').next().unwrap_or(rel);
                if wanted(name) && (rel.starts_with("crashes/") || !rel.contains('/')) && size.parse::<u64>().is_ok_and(|s| s <= MAX_FILE) {
                    list.push((t.parse().unwrap_or(0), rel.to_string()));
                }
            }
            ["H", h] if h.len() == 64 => ham = Some(h.to_string()),
            _ => {}
        }
    }
    let mut rels: Vec<&str> = list.iter().map(|(_, r)| r.as_str()).collect();
    rels.push("crashes/modules.json");
    let got = crate::remote::cat_files(r, &dir, &rels)?;
    let mut files = vec![];
    for ((mtime, rel), bytes) in list.iter().zip(got.iter()) {
        if let Some(b) = bytes {
            files.push(Found { rel: rel.clone(), mtime: *mtime, bytes: b.clone() });
        }
    }
    let modules = got.last().cloned().flatten();
    Ok(Raw { files, modules, ham, os: "linux" })
}

// ---- making sense of them --------------------------------------------------------------------

/// Seconds since 1970 of "YYYY-MM-DDTHH:MM:SSZ" (UTC; fractions and offsets other than Z refused).
fn iso_secs(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || !s.ends_with('Z') {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    // days from civil (Howard Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + se).ok()
}

/// "2026-11-14 06:12 UTC" for a time.
fn shown(t: u64) -> String {
    let days = (t / 86400) as i64;
    // civil from days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let s = t % 86400;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", s / 3600, (s % 3600) / 60)
}

fn text_list(v: &Value, max: usize, each: usize) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str).take(max).map(|s| s.chars().take(each).collect()).collect()
}

fn word(v: &Value, max: usize) -> Option<String> {
    v.as_str().map(|s| s.chars().filter(|c| !c.is_control()).take(max).collect::<String>()).filter(|s| !s.is_empty())
}

/// A crash file (Y's format) as the app shows it.
fn from_json(f: &Found) -> Option<Value> {
    let v: Value = serde_json::from_slice(&f.bytes).ok()?;
    if v["schema"] != "loner-strike/crash" || v["version"].as_u64() != Some(1) {
        return None;
    }
    let time = v["time"].as_str().and_then(iso_secs).unwrap_or(f.mtime);
    let address = word(&v["address"], 24);
    let at_stop = v["atStop"].as_bool() == Some(true) || address.as_deref() == Some(NOT_A_CRASH);
    let mut c = json!({
        "id": f.rel.trim_start_matches("crashes/").trim_end_matches(".json"),
        "time": time, "when": shown(time), "source": "crash file", "file": f.rel,
        "kind": word(&v["kind"], 16).unwrap_or_else(|| "crash".into()),
        "map": word(&v["map"], 32), "mode": word(&v["mode"], 32),
        "uptimeSec": v["uptimeSec"].as_u64(), "mapSec": v["mapSec"].as_u64(), "players": v["players"].as_u64(), "bots": v["bots"].as_u64(),
        "fault": word(&v["fault"], 32), "address": address, "module": word(&v["module"], 64), "offset": word(&v["offset"], 24),
        "stack": text_list(&v["stack"], 64, 200), "marks": text_list(&v["marks"], 50, 120), "lastLines": text_list(&v["lastLines"], 40, 300),
        "atStop": at_stop,
    });
    if let Some(b) = v["build"].as_object() {
        c["build"] = Value::Object(b.iter().filter_map(|(k, x)| Some((k.chars().take(24).collect(), json!(word(x, 64)?)))).take(8).collect());
    }
    Some(c)
}

/// The bot brain's own crash log (`ls_crash*.log`): time, exception, stack, last marks.
fn from_log(f: &Found) -> Option<Value> {
    let text = String::from_utf8_lossy(&f.bytes);
    let mut fault = None;
    let mut address = None;
    let mut stack = vec![];
    let mut marks = vec![];
    let mut part = "";
    for l in text.lines().take(400) {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("exception ") {
            let mut w = rest.split_whitespace();
            fault = w.next().map(String::from);
            address = rest.split(" at ").nth(1).and_then(|a| a.split_whitespace().next()).map(String::from);
        } else if t.starts_with("return addresses") {
            part = "stack";
        } else if t.starts_with("last marks") {
            part = "marks";
        } else if part == "stack" && t.starts_with("0x") {
            stack.push(t.split_whitespace().skip(1).collect::<Vec<_>>().join(" "));
        } else if part == "marks" && !t.is_empty() && marks.len() < 50 {
            marks.push(t.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    let fault = fault?;
    let at_stop = address.as_deref() == Some(NOT_A_CRASH);
    let module = stack.first().and_then(|s| s.split('+').next()).map(String::from);
    let name = f.rel.rsplit('/').next().unwrap_or(&f.rel);
    Some(json!({
        "id": name.trim_end_matches(".log"), "time": f.mtime, "when": shown(f.mtime), "source": "bot brain's crash log", "file": f.rel,
        "kind": "crash", "fault": fault, "address": address, "module": module, "stack": stack.into_iter().take(64).collect::<Vec<_>>(), "marks": marks,
        "lastLines": [], "atStop": at_stop,
        "note": if at_stop { Some("This fault happens when a debugger attaches or the server is being stopped: not counted as a crash.") } else { None },
    }))
}

/// The warnings: a Ham Sandwich that isn't the fixed one, and what the module check says is bad.
fn warnings(raw: &Raw, remote: bool) -> Vec<Value> {
    let mut w = vec![];
    if let Some(h) = &raw.ham {
        let fixed = if raw.os == "linux" { FIXED_HAM_LINUX } else { FIXED_HAM_WINDOWS };
        if h != fixed {
            let fix = if remote {
                "Restart the server: it puts Loner-Strike's fixed one back before every start. If this stays, update the server's Loner-Strike files (Health)."
            } else {
                "Update the server's Loner-Strike files (Health, Updates): they put the fixed one back."
            };
            w.push(json!({"id": "hamsandwich", "module": "Ham Sandwich", "file": ham_rel(raw.os),
                "message": "This server runs a Ham Sandwich module that isn't Loner-Strike's fixed one. The official module uses freed memory when the map changes, and servers crash on it.", "fix": fix}));
        }
    }
    if let Some(v) = raw.modules.as_deref().and_then(|b| serde_json::from_slice::<Value>(b).ok()).filter(|v| v["schema"] == "loner-strike/modules") {
        for m in v["modules"].as_array().into_iter().flatten().filter(|m| m["verdict"] == "bad").take(20) {
            let name = word(&m["name"], 48).or_else(|| word(&m["file"], 96)).unwrap_or_else(|| "A module".into());
            if name.eq_ignore_ascii_case("hamsandwich") && w.iter().any(|x| x["id"] == "hamsandwich") {
                continue;
            }
            w.push(json!({"id": format!("module:{}", name.to_ascii_lowercase()), "module": name, "file": word(&m["file"], 160), "version": word(&m["version"], 32),
                "message": word(&m["why"], 300).unwrap_or_else(|| "This module is known to crash servers.".into()), "fix": "Ask in Loner-Strike's Discord which build to use, or turn the module off."}));
        }
    }
    w
}

/// Everything about one server's crashes.
fn report(e: &Entry) -> Result<Value, String> {
    let raw = match &e.remote {
        Some(r) => read_remote(r, &e.id, &e.game)?,
        None => read_local(&e.dir.join(&e.game)),
    };
    let mut crashes: Vec<Value> = raw.files.iter().filter_map(|f| if f.rel.ends_with(".json") { from_json(f) } else { from_log(f) }).collect();
    crashes.sort_by(|a, b| b["time"].as_u64().cmp(&a["time"].as_u64()));
    let day = now().saturating_sub(86400);
    let counted = |c: &&Value| c["atStop"] != true;
    let last24h = crashes.iter().filter(counted).filter(|c| c["time"].as_u64().unwrap_or(0) >= day).count();
    let warnings = warnings(&raw, e.remote.is_some());
    let alerts = alerts_for(&e.id);
    Ok(json!({
        "server": e.id, "name": e.name, "checkedAt": now(),
        "last24h": last24h, "total": crashes.iter().filter(counted).count(),
        "crashes": crashes, "warnings": warnings,
        "hamsandwich": raw.ham.as_ref().map(|h| if *h == (if raw.os == "linux" { FIXED_HAM_LINUX } else { FIXED_HAM_WINDOWS }) { "fixed" } else { "not the fixed one" }),
        "alerts": {"discord": alerts, "discordLinked": crate::webstatus::read_hook("live", &e.id).is_some()},
        "capture": if raw.files.iter().any(|f| f.rel.starts_with("crashes/")) { "crash files" } else { "bot brain's crash log" },
    }))
}

// ---- what's known, for badges -----------------------------------------------------------------

fn cache() -> &'static Mutex<HashMap<String, (u64, Value)>> {
    static C: OnceLock<Mutex<HashMap<String, (u64, Value)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

fn refreshing() -> &'static Mutex<HashSet<String>> {
    static R: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Reads a server's crashes again and keeps the answer.
fn fresh(e: &Entry) -> Result<Value, String> {
    let v = report(e)?;
    cache().lock().unwrap_or_else(|x| x.into_inner()).insert(e.id.clone(), (now(), v.clone()));
    Ok(v)
}

/// The kept answer when it's recent, else a fresh one.
fn current(e: &Entry, force: bool) -> Result<Value, String> {
    if !force {
        if let Some((at, v)) = cache().lock().unwrap_or_else(|x| x.into_inner()).get(&e.id) {
            if now().saturating_sub(*at) < FRESH.as_secs() {
                return Ok(v.clone());
            }
        }
    }
    fresh(e)
}

fn summary() -> Value {
    let list = crate::servers::load();
    let mut out = serde_json::Map::new();
    for e in list.iter().filter(|e| e.ready) {
        let known = cache().lock().unwrap_or_else(|x| x.into_inner()).get(&e.id).cloned();
        let stale = known.as_ref().is_none_or(|(at, _)| now().saturating_sub(*at) >= FRESH.as_secs());
        if stale && refreshing().lock().unwrap_or_else(|x| x.into_inner()).insert(e.id.clone()) {
            let e = e.clone();
            std::thread::spawn(move || {
                let _ = fresh(&e);
                refreshing().lock().unwrap_or_else(|x| x.into_inner()).remove(&e.id);
            });
        }
        if let Some((_, v)) = known {
            out.insert(e.id.clone(), json!({"last24h": v["last24h"], "warnings": v["warnings"].as_array().map(Vec::len).unwrap_or(0)}));
        }
    }
    Value::Object(out)
}

// ---- Discord alerts ----------------------------------------------------------------------------

#[derive(Default, serde::Serialize, Deserialize)]
struct Alerts {
    #[serde(default)]
    discord: BTreeMap<String, bool>,
    /// crash ids already posted (or there when alerts were turned on), per server
    #[serde(default)]
    seen: BTreeMap<String, Vec<String>>,
}

fn alerts_file() -> PathBuf {
    crate::sys::app_dir().join("crash-alerts.json")
}

fn alerts_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(Default::default)
}

fn load_alerts() -> Alerts {
    std::fs::read(alerts_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_alerts(a: &Alerts) -> Result<(), String> {
    let text = serde_json::to_vec_pretty(a).map_err(|_| "Couldn't save the setting.".to_string())?;
    crate::sys::write_atomic(&alerts_file(), &text).map_err(|e| format!("Couldn't save the setting ({e})."))
}

fn alerts_for(id: &str) -> bool {
    load_alerts().discord.get(id).copied().unwrap_or(false)
}

fn set_alerts(e: &Entry, on: bool) -> Result<(), String> {
    if on && crate::webstatus::read_hook("live", &e.id).is_none() {
        return Err("Link the server's Discord status channel first (Website and Discord), then turn this on.".into());
    }
    let _g = alerts_lock().lock().unwrap_or_else(|x| x.into_inner());
    let mut a = load_alerts();
    a.discord.insert(e.id.clone(), on);
    if on {
        // what's there already isn't news
        let known: Vec<String> = current(e, true).map(|v| v["crashes"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_str().map(String::from)).collect()).unwrap_or_default();
        a.seen.insert(e.id.clone(), known);
    }
    save_alerts(&a)
}

/// The Discord message for a crash (no mentions; plain words).
fn message(name: &str, c: &Value) -> String {
    let mut m = format!("**{}** crashed", name.chars().filter(|c| !"*_`~|<>@".contains(*c)).take(64).collect::<String>());
    if let Some(map) = c["map"].as_str() {
        m.push_str(&format!(" on {map}"));
    }
    m.push_str(&format!(" at {}", c["when"].as_str().unwrap_or("")));
    match (c["fault"].as_str(), c["module"].as_str()) {
        (Some(f), Some(md)) => m.push_str(&format!(": {f} in {md}")),
        (Some(f), None) => m.push_str(&format!(": {f}")),
        _ => {}
    }
    m.push_str(". The details are in Loner-Strike (Servers, Health).");
    m
}

/// One look: new crashes of servers with alerts on, posted. Returns how many were posted.
fn post_new() -> usize {
    let a = load_alerts();
    let on: Vec<String> = a.discord.iter().filter(|(_, v)| **v).map(|(k, _)| k.clone()).collect();
    let mut posted = 0;
    for id in on {
        let (Some(e), Some(hook)) = (crate::servers::get(&id), crate::webstatus::read_hook("live", &id)) else { continue };
        let Ok(v) = current(&e, true) else { continue };
        let _g = alerts_lock().lock().unwrap_or_else(|x| x.into_inner());
        let mut a = load_alerts();
        let seen = a.seen.entry(id.clone()).or_default();
        let day = now().saturating_sub(86400);
        for c in v["crashes"].as_array().into_iter().flatten().rev() {
            let Some(cid) = c["id"].as_str() else { continue };
            if seen.iter().any(|s| s == cid) {
                continue;
            }
            seen.push(cid.to_string());
            if c["atStop"] == true || c["time"].as_u64().unwrap_or(0) < day {
                continue;
            }
            let body = json!({"content": message(&e.name, c), "allowed_mentions": {"parse": []}, "username": "Loner-Strike"});
            // (never log the webhook's address: it carries its token)
            match crate::sys::http().post(&hook).timeout(Duration::from_secs(10)).json(&body).send() {
                Ok(r) if r.status().is_success() => posted += 1,
                Ok(r) => crate::diagnostics::log_error("crash alert", &format!("Discord answered HTTP {}", r.status().as_u16())),
                Err(_) => crate::diagnostics::log_error("crash alert", "couldn't reach Discord"),
            }
        }
        let n = seen.len();
        if n > 500 {
            seen.drain(..n - 500);
        }
        let _ = save_alerts(&a);
    }
    posted
}

/// While the app runs: new crashes of servers with Discord alerts on, every five minutes.
pub fn watch() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(300));
        if load_alerts().discord.values().any(|v| *v) {
            post_new();
        }
    });
}

// ---- routes ----------------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

#[derive(Deserialize)]
struct FreshQ {
    #[serde(default)]
    fresh: Option<u8>,
}

/// Only the player's own servers (not ones shared with them: their files aren't here).
fn own(id: &str) -> Result<Entry, String> {
    crate::servers::get(id).ok_or_else(|| "That server isn't there any more.".to_string())
}

async fn api_crashes(UrlPath(id): UrlPath<String>, q: Option<Query<FreshQ>>) -> (StatusCode, Json<Value>) {
    let force = q.and_then(|q| q.0.fresh).unwrap_or(0) == 1;
    answer(tokio::task::spawn_blocking(move || own(&id).and_then(|e| current(&e, force))).await.unwrap_or_else(|_| Err("The check didn't finish.".into())))
}

#[derive(Deserialize)]
struct AlertsReq {
    discord: bool,
}

async fn api_alerts(UrlPath(id): UrlPath<String>, h: HeaderMap, Json(req): Json<AlertsReq>) -> (StatusCode, Json<Value>) {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    answer(
        tokio::task::spawn_blocking(move || {
            let e = own(&id)?;
            set_alerts(&e, req.discord)?;
            current(&e, false)
        })
        .await
        .unwrap_or_else(|_| Err("That didn't finish.".into())),
    )
}

async fn api_summary() -> Json<Value> {
    Json(tokio::task::spawn_blocking(summary).await.unwrap_or_else(|_| json!({})))
}

pub fn routes() -> Router<crate::server::Shared> {
    Router::new()
        .route("/api/servers/:id/crashes", get(api_crashes))
        .route("/api/servers/:id/crashes/alerts", post(api_alerts))
        .route("/api/crashes/summary", get(api_summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_ham_hashes_match_the_resources() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        assert_eq!(hex(&Sha256::digest(std::fs::read(root.join("windows/hamsandwich/hamsandwich_amxx.dll")).unwrap())), FIXED_HAM_WINDOWS);
        assert_eq!(hex(&Sha256::digest(std::fs::read(root.join("linux/hamsandwich/hamsandwich_amxx_i386.so")).unwrap())), FIXED_HAM_LINUX);
    }

    #[test]
    fn times() {
        assert_eq!(iso_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_secs("2026-11-14T06:12:03Z"), Some(1_794_636_723));
        assert_eq!(shown(1_794_636_723), "2026-11-14 06:12 UTC");
        assert_eq!(iso_secs("2026-13-14T06:12:03Z"), None);
        assert_eq!(iso_secs("yesterday"), None);
    }

    fn game(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-crashes-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("crashes")).unwrap();
        std::fs::create_dir_all(d.join("addons/amxmodx/modules")).unwrap();
        d
    }

    /// Y's crash files, the brain's own log (a real one's shape), the debugger fault left out of
    /// the count, the official Ham Sandwich warned about, a bad module from the module check.
    #[test]
    fn reads_crash_files_logs_and_module_warnings() {
        let g = game("read");
        let t = now() - 3600;
        let iso = {
            let s = shown(t);
            format!("{}T{}:00Z", &s[0..10], &s[11..16])
        };
        std::fs::write(g.join("crashes/20261114-061203-4242.json"), json!({"schema": "loner-strike/crash", "version": 1, "time": iso, "map": "de_dust2", "mode": "last_stand",
            "kind": "crash", "fault": "SIGSEGV", "address": "0xf1d2", "module": "amxmodx_mm_i386.so", "offset": "0x21ebd",
            "stack": ["amxmodx_mm_i386.so+0x21ebd CLangMngr::CLang::Clear"], "marks": ["1704.88 bot think end 2"], "lastLines": ["L 11/14/2026 - 06:12:01: changelevel"],
            "atStop": false, "build": {"brain": "ls-next3 797048d"}}).to_string()).unwrap();
        std::fs::write(g.join("ls_crash.20261113-0051.log"), "Fri Nov 13 00:51:39 2026\nexception 0xc0000005 at 0xfff4fadc (first-chance)\nwhere:\nreturn addresses on the stack (newest first):\n  0x7bebea40  kernel32.dll+0xea40\nlast marks:\n     1704.86  frame end   0\n").unwrap();
        std::fs::write(g.join("ls_crash.log"), "Sat Nov 14 01:00:00 2026\nexception 0xc0000005 at 0x0a1b2c3d\nreturn addresses on the stack (newest first):\n  0x0a1b2c3d  mp.dll+0x1b2c3d\n  0x0a000010  hl.exe+0x10\n").unwrap();
        std::fs::write(g.join("addons/amxmodx/modules/hamsandwich_amxx.dll"), b"the official one").unwrap();
        std::fs::write(g.join("crashes/modules.json"), json!({"schema": "loner-strike/modules", "version": 1, "modules": [{"name": "fun", "file": "addons/amxmodx/modules/fun_amxx.dll", "verdict": "bad", "why": "This fun build crashes on map changes."}, {"name": "cstrike", "verdict": "ok"}]}).to_string()).unwrap();
        let raw = read_local(&g);
        assert_eq!(raw.files.len(), 3, "modules.json isn't a crash");
        let mut crashes: Vec<Value> = raw.files.iter().filter_map(|f| if f.rel.ends_with(".json") { from_json(f) } else { from_log(f) }).collect();
        crashes.sort_by(|a, b| b["time"].as_u64().cmp(&a["time"].as_u64()));
        let file = crashes.iter().find(|c| c["source"] == "crash file").unwrap();
        assert_eq!(file["map"], "de_dust2");
        assert_eq!(file["module"], "amxmodx_mm_i386.so");
        assert_eq!(file["time"].as_u64(), Some(t - t % 60));
        let dbg = crashes.iter().find(|c| c["id"] == "ls_crash.20261113-0051").unwrap();
        assert_eq!(dbg["atStop"], true, "the debugger fault isn't counted: {dbg}");
        let real = crashes.iter().find(|c| c["id"] == "ls_crash").unwrap();
        assert_eq!(real["module"], "mp.dll");
        assert_eq!(real["stack"][0], "mp.dll+0x1b2c3d");
        let w = warnings(&raw, false);
        assert!(w.iter().any(|x| x["id"] == "hamsandwich"), "{w:?}");
        assert!(w.iter().any(|x| x["id"] == "module:fun" && x["message"].as_str().unwrap().contains("fun build")), "{w:?}");
        assert!(!w.iter().any(|x| x["module"] == "cstrike"));
        // the fixed one: no Ham Sandwich warning
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/windows/hamsandwich/hamsandwich_amxx.dll"), g.join("addons/amxmodx/modules/hamsandwich_amxx.dll")).unwrap();
        assert!(!warnings(&read_local(&g), false).iter().any(|x| x["id"] == "hamsandwich"));
        let _ = std::fs::remove_dir_all(&g);
    }

    #[test]
    fn odd_files_are_skipped() {
        let g = game("odd");
        std::fs::write(g.join("crashes/bad name.json"), "{}").unwrap();
        std::fs::write(g.join("crashes/other.json"), json!({"schema": "something/else"}).to_string()).unwrap();
        std::fs::write(g.join("crashes/broken.json"), "{not json").unwrap();
        std::fs::write(g.join("ls_crash.log"), "no exception in here").unwrap();
        let raw = read_local(&g);
        let crashes: Vec<Value> = raw.files.iter().filter_map(|f| if f.rel.ends_with(".json") { from_json(f) } else { from_log(f) }).collect();
        assert!(crashes.is_empty(), "{crashes:?}");
        assert!(warnings(&raw, false).is_empty(), "no module there, nothing to warn about");
        let _ = std::fs::remove_dir_all(&g);
    }

    #[test]
    fn the_discord_message_is_plain_and_mentions_nobody() {
        let m = message("My @everyone *server*", &json!({"map": "de_dust2", "when": "2026-11-14 06:12 UTC", "fault": "SIGSEGV", "module": "mp.dll"}));
        assert_eq!(m, "**My everyone server** crashed on de_dust2 at 2026-11-14 06:12 UTC: SIGSEGV in mp.dll. The details are in Loner-Strike (Servers, Health).");
    }

    /// QA: what the app makes of a real game folder's crash files (read only).
    /// `LS_CRASH_GAME=<.../cstrike> cargo test real_crash_files -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_crash_files() {
        let Ok(dir) = std::env::var("LS_CRASH_GAME") else { return };
        let raw = read_local(Path::new(&dir));
        let crashes: Vec<Value> = raw.files.iter().filter_map(|f| if f.rel.ends_with(".json") { from_json(f) } else { from_log(f) }).collect();
        let counted = crashes.iter().filter(|c| c["atStop"] != true).count();
        println!("{dir}: {} files, {} crashes read, {} counted, ham {:?}, warnings {}", raw.files.len(), crashes.len(), counted, raw.ham.as_deref().map(|h| &h[..12]), warnings(&raw, false).len());
        for c in crashes.iter().take(3) {
            println!("  {} {} {} {} stack {}", c["when"], c["fault"], c["address"], c["module"], c["stack"].as_array().map(Vec::len).unwrap_or(0));
        }
        assert_eq!(crashes.len(), raw.files.len(), "every crash file read");
    }
}
