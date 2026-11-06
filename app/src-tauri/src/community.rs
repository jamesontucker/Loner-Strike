//! Community: stats and ranks, mutes, a swear filter, the chat log and player reports,
//! per game or dedicated server. gm_community.amxx does the work in the game:
//!
//! - Stats: csx's ranks (what statsx's /rank and /top15 show), written by the plugin
//!   to <data>/stats.json (JSON lines) when a map ends and on gm_statsdump. Bots are
//!   left out (authid BOT, or a name a bot has used: <data>/bots.txt).
//!   Resetting sends gm_statsreset (csx clears the ranks at the next map change), or,
//!   with the game off, deletes csstats.dat.
//! - Mutes: <data>/gags.txt, by SteamID, written here and by amx_gag in the game.
//! - Swear filter and switches: <data>/community.json, community.cfg and filter.txt.
//! - Chat log: <data>/chat.log and chat.old.log, written by the plugin.
//! - Reports (say /report): <data>/reports.log, folded into reports.json here. New
//!   ones raise a desktop notification (notify.rs calls new_reports).
//!
//! The game only gets fixed commands; everything written is built from checked SteamIDs,
//! numbers and cleaned words.

use crate::admins::valid_steamid;
use crate::game::Game;
use crate::panel_api::from_panel;
use crate::rcon::RconError;
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

const RELOAD: &str = "gm_commreload";
const DUMP: &str = "gm_statsdump";
const RESET: &str = "gm_statsreset";
const MAX_WORDS: usize = 300;
const MAX_WORD: usize = 31;
const MAX_REPORTS: usize = 500;
const MAX_GAG_MINUTES: u32 = 525_600;
static LOCK: Mutex<()> = Mutex::new(());

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// A line the game wrote: UTF-8 (what Steam clients send), else Latin-1.
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => crate::rcon::latin1(bytes),
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else { return vec![] };
    bytes.split(|&b| b == b'\n').map(|l| decode(l.strip_suffix(b"\r").unwrap_or(l))).filter(|l| !l.trim().is_empty()).collect()
}

/// One line, no control characters or tabs, at most `max` characters.
fn clean(s: &str, max: usize) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().chars().take(max).collect::<String>().trim().to_string()
}

// ---- settings -------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FilterMode {
    Off,
    Replace,
    Block,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub filter: FilterMode,
    pub words: Vec<String>,
    pub chatlog: bool,
    pub reports: bool,
    /// games against bots count toward the ranks (csstats_rankbots)
    pub stats_bots: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { filter: FilterMode::Replace, words: vec![], chatlog: true, reports: true, stats_bots: true }
    }
}

/// Filter words: lowercase, one word each (no spaces), 2 to 31 characters, no repeats.
pub fn tidy_words(words: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for w in words {
        let w = w.trim().to_lowercase();
        if w.is_empty() {
            continue;
        }
        if w.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!("\"{}\" has a space in it. Add one word at a time.", clean(&w, 40)));
        }
        if w.chars().count() < 2 || w.len() > MAX_WORD {
            return Err(format!("\"{}\" is too short or too long. Words are 2 to {MAX_WORD} letters.", clean(&w, 40)));
        }
        if w.starts_with(';') {
            return Err("A word can't start with ;.".into());
        }
        if !out.contains(&w) {
            out.push(w);
        }
    }
    if out.len() > MAX_WORDS {
        return Err(format!("Keep the list to {MAX_WORDS} words."));
    }
    out.sort();
    Ok(out)
}

pub fn cfg(s: &Settings) -> String {
    let b = |v: bool| if v { 1 } else { 0 };
    let filter = match s.filter {
        FilterMode::Off => 0,
        FilterMode::Replace => 1,
        FilterMode::Block => 2,
    };
    [
        "// Written by Loner-Strike's Community page. Change it there.".to_string(),
        format!("gm_filter {filter}"),
        format!("gm_chatlog {}", b(s.chatlog)),
        format!("gm_report {}", b(s.reports)),
        format!("gm_stats_bots {}", b(s.stats_bots)),
        String::new(),
    ]
    .join("\r\n")
}

fn filter_txt(words: &[String]) -> String {
    let mut out = vec!["; Written by Loner-Strike's Community page. One word per line.".to_string()];
    out.extend(words.iter().cloned());
    out.push(String::new());
    out.join("\r\n")
}

fn load(data: &Path) -> Settings {
    std::fs::read_to_string(data.join("community.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(data: &Path, s: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(data.join("community.json"), text).map_err(|e| format!("Couldn't save the settings ({e})."))?;
    std::fs::write(data.join("community.cfg"), cfg(s)).map_err(|e| format!("Couldn't write community.cfg ({e})."))?;
    // chat arrives as UTF-8, so the words are written that way too
    std::fs::write(data.join("filter.txt"), filter_txt(&s.words)).map_err(|e| format!("Couldn't write the word list ({e})."))
}

/// The settings files exist from the start, so the plugin's defaults never apply alone.
fn ensure_files(data: &Path) {
    if !data.join("community.cfg").exists() {
        let _ = save(data, &load(data));
    }
}

// ---- mutes ------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Gag {
    pub id: String,
    pub name: String,
    /// unix time it ends, 0 = until removed
    pub until: i64,
    pub chat: bool,
    pub voice: bool,
}

/// gags.txt: "SteamID\tuntil\tflags\tname"; ; comments. Ended and broken lines are dropped.
pub fn parse_gags(text: &str, now: i64) -> Vec<Gag> {
    let mut out: Vec<Gag> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with(';') {
            continue;
        }
        let mut p = line.splitn(4, '\t');
        let (Some(id), Some(until), Some(flags)) = (p.next(), p.next(), p.next()) else { continue };
        let name = p.next().unwrap_or("");
        let id = id.trim();
        let Ok(until) = until.trim().parse::<i64>() else { continue };
        let (chat, voice) = (flags.contains('c'), flags.contains('v'));
        if !valid_steamid(id) || !(chat || voice) || until < 0 || (until != 0 && until <= now) {
            continue;
        }
        out.retain(|g| g.id != id);
        out.push(Gag { id: id.to_string(), name: clean(name, 31), until, chat, voice });
    }
    out
}

pub fn gags_txt(gags: &[Gag]) -> String {
    let mut out = String::from("; Mutes from Loner-Strike's Community page and amx_gag: SteamID, until (unix time, 0 = until removed), c chat / v voice, name\n");
    for g in gags {
        let flags = format!("{}{}", if g.chat { "c" } else { "" }, if g.voice { "v" } else { "" });
        out.push_str(&format!("{}\t{}\t{}\t{}\n", g.id, g.until, flags, clean(&g.name, 31).replace('\t', " ")));
    }
    out
}

fn read_gags(data: &Path) -> Vec<Gag> {
    parse_gags(&decode(&std::fs::read(data.join("gags.txt")).unwrap_or_default()), now())
}

fn write_gags(data: &Path, gags: &[Gag]) -> Result<(), String> {
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    std::fs::write(data.join("gags.txt"), gags_txt(gags)).map_err(|e| format!("Couldn't save the mutes ({e})."))
}

// ---- stats ------------------------------------------------------------------------------

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct StatLine {
    pub rank: u32,
    pub name: String,
    pub auth: String,
    pub bot: u8,
    pub k: i64,
    pub d: i64,
    pub hs: i64,
    pub tk: i64,
    pub shots: i64,
    pub hits: i64,
    pub dmg: i64,
    pub body: Vec<i64>,
    pub defusing: i64,
    pub defused: i64,
    pub planted: i64,
    pub exploded: i64,
    pub time: i64,
}

/// stats.json: an "info" line, then one "p" line per ranked player.
pub fn parse_stats(lines: &[String]) -> (Value, Vec<StatLine>) {
    let mut info = json!({});
    let mut players = Vec::new();
    for l in lines {
        let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
        match v["t"].as_str() {
            Some("info") => info = v,
            Some("p") => {
                if let Ok(p) = serde_json::from_value::<StatLine>(v) {
                    players.push(p);
                }
            }
            _ => {}
        }
    }
    (info, players)
}

/// Bots: marked by the plugin, the BOT authid, or (when ranks go by name) a bot's name.
pub fn is_bot(p: &StatLine, bot_names: &HashSet<String>) -> bool {
    let a = p.auth.to_uppercase();
    p.bot == 1 || a == "BOT" || (!a.starts_with("STEAM_") && !a.starts_with("VALVE_") && bot_names.contains(&p.name))
}

fn pct(a: i64, b: i64) -> f64 {
    if b <= 0 { 0.0 } else { (a as f64 * 1000.0 / b as f64).round() / 10.0 }
}

pub fn stat_row(p: &StatLine, pos: usize) -> Value {
    let kd = if p.d <= 0 { p.k as f64 } else { (p.k as f64 * 100.0 / p.d as f64).round() / 100.0 };
    let mut body = p.body.clone();
    body.resize(8, 0);
    json!({
        "pos": pos, "rank": p.rank, "name": p.name, "auth": p.auth, "steam": valid_steamid(&p.auth),
        "k": p.k, "d": p.d, "kd": kd, "hs": p.hs, "hs_pct": pct(p.hs, p.k), "acc": pct(p.hits, p.shots),
        "shots": p.shots, "hits": p.hits, "dmg": p.dmg, "tk": p.tk, "time": p.time,
        "body": {"head": body[1], "chest": body[2], "stomach": body[3], "arms": body[4] + body[5], "legs": body[6] + body[7]},
        "bomb": {"planted": p.planted, "exploded": p.exploded, "defused": p.defused, "defusing": p.defusing},
    })
}

fn bot_names(data: &Path) -> HashSet<String> {
    read_lines(&data.join("bots.txt")).into_iter().map(|l| l.trim().to_string()).collect()
}

fn stats(g: &Game, refresh: bool) -> Value {
    let online = if refresh { g.rcon.send(DUMP).is_ok() } else { false };
    let lines = read_lines(&g.data.join("stats.json"));
    let (info, all) = parse_stats(&lines);
    let bots = bot_names(&g.data);
    let total = all.len();
    let humans: Vec<Value> = all.iter().filter(|p| !is_bot(p, &bots)).enumerate().map(|(i, p)| stat_row(p, i + 1)).collect();
    json!({
        "players": humans,
        "bots_hidden": total - humans.len(),
        "updated": info["time"].as_i64().unwrap_or(0),
        "reset_pending": info["reset_pending"].as_i64() == Some(1),
        "csx": info["csx"].as_i64() != Some(0),
        "rank_by": info["rank_by"].as_i64().unwrap_or(1),
        "online": online,
        "has_file": !lines.is_empty(),
    })
}

/// Where csx keeps the ranks: core.ini's "csstats" entry, inside the game folder.
pub fn csstats_path(core_ini: &str) -> String {
    for line in core_ini.lines() {
        let mut w = line.split_whitespace();
        if w.next() == Some("csstats") {
            if let Some(v) = w.next() {
                let ok = v.ends_with(".dat") && !v.contains("..") && !v.starts_with('/') && !v.contains(':');
                if ok {
                    return v.replace('\\', "/");
                }
            }
        }
    }
    "addons/amxmodx/data/csstats.dat".into()
}

fn reset_stats(g: &Game) -> Result<Value, String> {
    match g.rcon.send(RESET) {
        Ok(_) => Ok(json!({"ok": true, "message": "Stats clear when the map changes."})),
        Err(RconError::BadPassword) => Err("The game rejected the panel password. Restart the map once.".into()),
        Err(RconError::Offline) => {
            // the game is off, so nothing holds the ranks in memory
            let core = crate::sys::read_latin1(&g.dir.join("addons/amxmodx/configs/core.ini")).unwrap_or_default();
            if let Some(p) = crate::setup::record::inside(&g.dir, &csstats_path(&core)) {
                match std::fs::remove_file(&p) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("Couldn't delete the stats file ({e}).")),
                    _ => {}
                }
            }
            for f in ["stats.json", "playtime.txt"] {
                let _ = std::fs::remove_file(g.data.join(f));
            }
            Ok(json!({"ok": true, "message": "Stats cleared."}))
        }
    }
}

// ---- chat log ---------------------------------------------------------------------------

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ChatLine {
    pub t: i64,
    pub id: String,
    pub name: String,
    pub team: String,
    /// "all" or "team"
    pub scope: String,
    /// "", "muted", "filtered", "blocked" or "report"
    pub status: String,
    pub text: String,
}

/// "time\tSteamID\tname\tteam\tscope\tstatus\ttext"
pub fn parse_chat(line: &str) -> Option<ChatLine> {
    let mut p = line.splitn(7, '\t');
    let t = p.next()?.trim().parse::<i64>().ok()?;
    let id = p.next()?.trim().to_string();
    let name = clean(p.next()?, 32);
    let team = p.next()?.trim().to_string();
    let scope = p.next()?.trim().to_string();
    let status = p.next()?.trim().to_string();
    let text = clean(p.next()?, 200);
    Some(ChatLine { t, id, name, team, scope, status, text })
}

/// Newest first, matching every word of q in the name, SteamID or text.
pub fn search_chat(lines: &[String], q: &str, limit: usize) -> Vec<ChatLine> {
    let words: Vec<String> = q.split_whitespace().map(|w| w.to_lowercase()).collect();
    lines
        .iter()
        .rev()
        .filter_map(|l| parse_chat(l))
        .filter(|c| {
            let hay = format!("{} {} {}", c.name, c.id, c.text).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .take(limit)
        .collect()
}

fn chat(data: &Path, q: &str, limit: usize) -> Value {
    let mut lines = read_lines(&data.join("chat.old.log"));
    lines.extend(read_lines(&data.join("chat.log")));
    let found = search_chat(&lines, q, limit);
    json!({"lines": found, "total": lines.len(), "more": found.len() == limit})
}

// ---- reports ------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Report {
    pub t: i64,
    pub from_id: String,
    pub from: String,
    pub about_id: String,
    pub about: String,
    #[serde(default)]
    pub map: String,
    pub reason: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub notified: bool,
}

/// "time\tfrom SteamID\tfrom\tabout SteamID\tabout\tmap\treason"
pub fn parse_report(line: &str) -> Option<Report> {
    let mut p = line.splitn(7, '\t');
    let t = p.next()?.trim().parse::<i64>().ok()?;
    Some(Report {
        t,
        from_id: clean(p.next()?, 40),
        from: clean(p.next()?, 32),
        about_id: clean(p.next()?, 40),
        about: clean(p.next()?, 32),
        map: clean(p.next()?, 32),
        reason: clean(p.next().unwrap_or(""), 140),
        done: false,
        notified: false,
    })
}

/// Old reports plus the log's new ones, newest first.
pub fn fold_reports(mut list: Vec<Report>, log: &[String]) -> Vec<Report> {
    list.extend(log.iter().filter_map(|l| parse_report(l)));
    list.sort_by(|a, b| b.t.cmp(&a.t));
    list.truncate(MAX_REPORTS);
    list
}

fn save_reports(data: &Path, list: &[Report]) -> Result<(), String> {
    std::fs::write(data.join("reports.json"), serde_json::to_string(list).unwrap_or_default()).map_err(|e| format!("Couldn't save the reports ({e})."))
}

/// The reports, after taking in what the plugin wrote since last time. Call with LOCK held.
fn reports(data: &Path) -> Vec<Report> {
    let file = data.join("reports.json");
    let list: Vec<Report> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // move the log aside first, so lines the plugin writes meanwhile start a new one
    let log = data.join("reports.log");
    let taking = data.join("reports.log.reading");
    if !taking.exists() && std::fs::rename(&log, &taking).is_err() {
        return list;
    }
    let list = fold_reports(list, &read_lines(&taking));
    if save_reports(data, &list).is_ok() {
        let _ = std::fs::remove_file(&taking);
    }
    list
}

/// For notify.rs: reports no notification has been shown for yet (marked shown now).
pub fn new_reports(data: &Path) -> Vec<Report> {
    if !data.join("reports.log").exists() && !data.join("reports.log.reading").exists() && !data.join("reports.json").exists() {
        return vec![];
    }
    let _l = LOCK.lock().unwrap();
    let mut list = reports(data);
    let fresh: Vec<Report> = list.iter().filter(|r| !r.notified).cloned().collect();
    if !fresh.is_empty() {
        for r in list.iter_mut() {
            r.notified = true;
        }
        let _ = save_reports(data, &list);
    }
    fresh
}

/// The notification text for some new reports.
pub fn report_note(fresh: &[Report], label: &str) -> Option<String> {
    match fresh {
        [] => None,
        [r] => Some(format!("Report: {} reported {}{} in {label}", r.from, r.about, if r.reason.is_empty() { String::new() } else { format!(" ({})", r.reason) })),
        many => Some(format!("{} new reports in {label}. The latest: {} reported {}", many.len(), many[0].from, many[0].about)),
    }
}

// ---- API ------------------------------------------------------------------------------

type Reply = (StatusCode, Json<Value>);

fn bad(msg: impl Into<String>) -> Reply {
    (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": msg.into()})))
}

fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn answer(r: Option<Result<Value, String>>) -> Reply {
    match r {
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"ok": false, "error": "No game found."}))),
        Some(Ok(v)) => (StatusCode::OK, Json(v)),
        Some(Err(e)) => bad(e),
    }
}

async fn with_game<T: Send + 'static>(st: &Shared, f: impl FnOnce(&Game) -> T + Send + 'static) -> Option<T> {
    let g = st.game()?;
    tokio::task::spawn_blocking(move || f(&g)).await.ok()
}

/// A fixed command for the running game. Offline is fine.
fn tell(g: &Game, cmd: &str) -> Result<bool, String> {
    match g.rcon.send(cmd) {
        Ok(_) => Ok(true),
        Err(RconError::Offline) => Ok(false),
        Err(RconError::BadPassword) => Err("The game rejected the panel password. Restart the map once.".into()),
    }
}

fn listing(g: &Game) -> Value {
    let open = reports(&g.data).iter().filter(|r| !r.done).count();
    json!({
        "settings": load(&g.data),
        "gags": read_gags(&g.data),
        "now": now(),
        "open_reports": open,
        "plugin": g.dir.join("addons/amxmodx/plugins/gm_community.amxx").exists(),
        "dedicated": g.dedicated,
    })
}

async fn api_get(State(st): State<Shared>) -> Reply {
    answer(
        with_game(&st, |g| {
            let _l = LOCK.lock().unwrap();
            ensure_files(&g.data);
            Ok(listing(g))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct StatsQuery {
    #[serde(default)]
    refresh: Option<String>,
}

async fn api_stats(State(st): State<Shared>, Query(q): Query<StatsQuery>) -> Reply {
    let refresh = q.refresh.as_deref() == Some("1");
    answer(with_game(&st, move |g| Ok(stats(g, refresh))).await)
}

async fn api_reset(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    answer(with_game(&st, reset_stats).await)
}

#[derive(Deserialize)]
struct ChatQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    limit: Option<usize>,
}

async fn api_chat(State(st): State<Shared>, Query(q): Query<ChatQuery>) -> Reply {
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    let text = clean(&q.q, 80);
    answer(with_game(&st, move |g| Ok(chat(&g.data, &text, limit))).await)
}

async fn api_reports(State(st): State<Shared>) -> Reply {
    answer(
        with_game(&st, |g| {
            let _l = LOCK.lock().unwrap();
            Ok(json!({"reports": reports(&g.data)}))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct ReportReq {
    /// "done", "open" or "delete" for one report; "clear_done" for all handled ones
    action: String,
    #[serde(default)]
    t: i64,
    #[serde(default)]
    from_id: String,
    #[serde(default)]
    about: String,
}

async fn api_report(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ReportReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    if !["done", "open", "delete", "clear_done"].contains(&req.action.as_str()) {
        return bad("Unknown action.");
    }
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap();
            let mut list = reports(&g.data);
            let same = |r: &Report| r.t == req.t && r.from_id == req.from_id && r.about == req.about;
            match req.action.as_str() {
                "clear_done" => list.retain(|r| !r.done),
                "delete" => list.retain(|r| !same(r)),
                a => {
                    for r in list.iter_mut().filter(|r| same(r)) {
                        r.done = a == "done";
                    }
                }
            }
            save_reports(&g.data, &list)?;
            Ok(json!({"reports": list}))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct GagReq {
    id: String,
    #[serde(default)]
    name: String,
    /// 0 = until removed
    #[serde(default)]
    minutes: u32,
    #[serde(default = "yes")]
    chat: bool,
    #[serde(default)]
    voice: bool,
}

fn yes() -> bool {
    true
}

/// A checked mute, ending `minutes` after `now` (0 = until removed).
pub fn make_gag(id: &str, name: &str, minutes: u32, chat: bool, voice: bool, now: i64) -> Result<Gag, String> {
    let id = id.trim().to_uppercase();
    if !valid_steamid(&id) {
        return Err("That isn't a SteamID. It looks like STEAM_0:1:12345. Bots and LAN players can't be muted from here.".into());
    }
    if !chat && !voice {
        return Err("Pick chat, voice or both.".into());
    }
    if minutes > MAX_GAG_MINUTES {
        return Err("A mute can last a year at most. Pick \"until removed\" for longer.".into());
    }
    let until = if minutes == 0 { 0 } else { now + minutes as i64 * 60 };
    Ok(Gag { id, name: clean(name, 31), until, chat, voice })
}

async fn api_gag(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<GagReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let gag = match make_gag(&req.id, &req.name, req.minutes, req.chat, req.voice, now()) {
        Ok(g) => g,
        Err(e) => return bad(e),
    };
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap();
            let mut gags = read_gags(&g.data);
            match gags.iter_mut().find(|x| x.id == gag.id) {
                Some(x) => {
                    let name = if gag.name.is_empty() { x.name.clone() } else { gag.name.clone() };
                    *x = Gag { name, ..gag };
                }
                None => gags.push(gag),
            }
            write_gags(&g.data, &gags)?;
            tell(g, RELOAD)?;
            Ok(listing(g))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

async fn api_ungag(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<IdReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let id = req.id.trim().to_uppercase();
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap();
            let mut gags = read_gags(&g.data);
            gags.retain(|x| x.id != id);
            write_gags(&g.data, &gags)?;
            tell(g, RELOAD)?;
            Ok(listing(g))
        })
        .await,
    )
}

async fn api_settings(State(st): State<Shared>, headers: HeaderMap, Json(mut req): Json<Settings>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    req.words = match tidy_words(&req.words) {
        Ok(w) => w,
        Err(e) => return bad(e),
    };
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap();
            save(&g.data, &req)?;
            tell(g, RELOAD)?;
            Ok(listing(g))
        })
        .await,
    )
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/community", get(api_get))
        .route("/api/community/stats", get(api_stats))
        .route("/api/community/stats/reset", post(api_reset))
        .route("/api/community/chat", get(api_chat))
        .route("/api/community/reports", get(api_reports).post(api_report))
        .route("/api/community/gag", post(api_gag))
        .route("/api/community/ungag", post(api_ungag))
        .route("/api/community/settings", post(api_settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ls-community-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn community_words_are_tidied() {
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(tidy_words(&v(&[" Darn ", "heck", "darn", "", "HECK"])).unwrap(), v(&["darn", "heck"]));
        assert!(tidy_words(&v(&["two words"])).is_err());
        assert!(tidy_words(&v(&["a"])).is_err(), "too short");
        assert!(tidy_words(&v(&[&"x".repeat(40)])).is_err(), "too long");
        assert!(tidy_words(&v(&["bad\nline"])).is_err());
        assert!(tidy_words(&v(&[";comment"])).is_err());
        let many: Vec<String> = (0..301).map(|i| format!("w{i}")).collect();
        assert!(tidy_words(&many).is_err());
        assert_eq!(tidy_words(&v(&["über"])).unwrap(), v(&["über"]), "non-English words are fine");
    }

    #[test]
    fn community_cfg_and_files() {
        let mut s = Settings::default();
        let c = cfg(&s);
        for line in ["gm_filter 1", "gm_chatlog 1", "gm_report 1", "gm_stats_bots 1"] {
            assert!(c.contains(&format!("{line}\r\n")), "{line}");
        }
        s.filter = FilterMode::Block;
        s.stats_bots = false;
        s.words = vec!["darn".into()];
        assert!(cfg(&s).contains("gm_filter 2\r\n") && cfg(&s).contains("gm_stats_bots 0\r\n"));
        assert!(filter_txt(&s.words).ends_with("\r\ndarn\r\n"));
        s.filter = FilterMode::Off;
        assert!(cfg(&s).contains("gm_filter 0\r\n"));
        let dir = tmp("save");
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir), s);
        assert!(dir.join("community.cfg").exists() && dir.join("filter.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(serde_json::from_str::<Settings>(r#"{"filter":"block"}"#).unwrap().filter, FilterMode::Block);
    }

    #[test]
    fn community_gags_parse_and_write() {
        let text = "; header\nSTEAM_0:1:123\t0\tc\tBob\nSTEAM_0:0:5\t2000\tcv\tAnn the\ttabbed\nSTEAM_0:0:6\t50\tv\tOld\nSTEAM_ID_LAN\t0\tc\tLan\nBOT\t0\tc\tBot\nSTEAM_0:0:7\t0\t\tNothing\nSTEAM_0:0:8\tsoon\tc\tBad\n";
        let g = parse_gags(text, 100);
        assert_eq!(g.len(), 2, "{g:?}");
        assert_eq!(g[0], Gag { id: "STEAM_0:1:123".into(), name: "Bob".into(), until: 0, chat: true, voice: false });
        assert!(g[1].chat && g[1].voice && g[1].until == 2000);
        let again = parse_gags(&gags_txt(&g), 100);
        assert_eq!(again, g, "round trip");
        assert!(!gags_txt(&g).contains("the\ttabbed"), "no tabs inside a name");

        let m = make_gag(" steam_0:1:42 ", "Carl\n", 30, true, false, 1000).unwrap();
        assert_eq!((m.id.as_str(), m.name.as_str(), m.until), ("STEAM_0:1:42", "Carl", 1000 + 1800));
        assert_eq!(make_gag("STEAM_0:1:42", "", 0, false, true, 1000).unwrap().until, 0, "until removed");
        assert!(make_gag("BOT", "", 10, true, false, 0).is_err(), "never bots");
        assert!(make_gag("STEAM_ID_LAN", "", 10, true, false, 0).is_err());
        assert!(make_gag("STEAM_0:1:42", "", 10, false, false, 0).is_err(), "nothing to mute");
        assert!(make_gag("STEAM_0:1:42", "", MAX_GAG_MINUTES + 1, true, false, 0).is_err());
    }

    #[test]
    fn community_stats_file() {
        let lines: Vec<String> = [
            r#"{"t":"info","time":1700000000,"csx":1,"rank_by":1,"rankbots":1,"reset_pending":0,"total":4}"#,
            r#"{"t":"p","rank":1,"name":"BOT","auth":"BOT","bot":1,"k":500,"d":10,"hs":100,"tk":0,"shots":1000,"hits":500,"dmg":50000,"body":[0,100,200,50,20,20,30,30],"defusing":0,"defused":0,"planted":0,"exploded":0,"time":0}"#,
            r#"{"t":"p","rank":2,"name":"Jamie \"J\"","auth":"STEAM_0:1:42","bot":0,"k":120,"d":40,"hs":60,"tk":1,"shots":900,"hits":300,"dmg":15000,"body":[0,60,120,50,10,10,20,30],"defusing":3,"defused":2,"planted":5,"exploded":4,"time":7200}"#,
            r#"{"t":"p","rank":3,"name":"[ACE] Hawk","auth":"[ACE] Hawk","bot":0,"k":50,"d":0,"hs":10,"tk":0,"shots":0,"hits":0,"dmg":0,"body":[],"defusing":0,"defused":0,"planted":0,"exploded":0,"time":0}"#,
            "not json",
            r#"{"t":"p","rank":4,"name":"Lan guy","auth":"STEAM_ID_LAN","k":1,"d":3}"#,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (info, players) = parse_stats(&lines);
        assert_eq!(info["time"], 1700000000);
        assert_eq!(players.len(), 4);
        let bots: HashSet<String> = ["[ACE] Hawk".to_string()].into_iter().collect();
        let humans: Vec<&StatLine> = players.iter().filter(|p| !is_bot(p, &bots)).collect();
        assert_eq!(humans.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Jamie \"J\"", "Lan guy"]);
        // a bot's name never hides someone with a real SteamID
        let named: HashSet<String> = ["Jamie \"J\"".to_string()].into_iter().collect();
        assert!(!is_bot(&players[1], &named));

        let r = stat_row(&players[1], 1);
        assert_eq!(r["kd"], 3.0);
        assert_eq!(r["hs_pct"], 50.0);
        assert_eq!(r["acc"], 33.3);
        assert_eq!(r["body"]["arms"], 20);
        assert_eq!(r["body"]["legs"], 50);
        assert_eq!(r["steam"], true);
        let r = stat_row(&players[2], 2);
        assert_eq!(r["kd"], 50.0, "no deaths: K/D is the kills");
        assert_eq!(r["acc"], 0.0);
        assert_eq!(r["body"]["head"], 0, "short body list is padded");
    }

    #[test]
    fn community_csstats_path() {
        assert_eq!(csstats_path("; x\ncsstats         addons/amxmodx/data/csstats.dat\ncsstats_score   addons/amxmodx/data/csstats.amxx\n"), "addons/amxmodx/data/csstats.dat");
        assert_eq!(csstats_path("csstats addons\\amxmodx\\data\\ranks.dat\n"), "addons/amxmodx/data/ranks.dat");
        for bad in ["csstats ../../etc/x.dat", "csstats /etc/x.dat", "csstats C:/x.dat", "csstats addons/x.txt"] {
            assert_eq!(csstats_path(bad), "addons/amxmodx/data/csstats.dat", "{bad}");
        }
    }

    #[test]
    fn community_chat_log() {
        let lines: Vec<String> = [
            "1700000000\tSTEAM_0:1:42\tJamie\tCT\tall\t\tgg everyone",
            "1700000010\tSTEAM_0:0:7\tBob\tT\tteam\tfiltered\trush **** b",
            "garbage line",
            "1700000020\tSTEAM_ID_LAN\tLan\tSPEC\tall\tmuted\thello\tthere",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let all = search_chat(&lines, "", 10);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "Lan", "newest first");
        assert_eq!(all[0].text, "hello there", "a tab in the text is cleaned");
        assert_eq!(all[1].status, "filtered");
        assert_eq!(search_chat(&lines, "BOB rush", 10).len(), 1);
        assert_eq!(search_chat(&lines, "steam_0:1:42", 10)[0].name, "Jamie");
        assert!(search_chat(&lines, "nobody", 10).is_empty());
        assert_eq!(search_chat(&lines, "", 2).len(), 2);
    }

    #[test]
    fn community_reports() {
        let log: Vec<String> = ["1700000100\tSTEAM_0:1:42\tJamie\tSTEAM_0:0:7\tBob\tde_dust2\twallhacking", "1700000200\tSTEAM_0:0:9\tAnn\tSTEAM_0:0:7\tBob\tcs_office\t", "nope"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let list = fold_reports(vec![], &log);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].from, "Ann", "newest first");
        assert_eq!(list[1].reason, "wallhacking");
        assert_eq!(report_note(&list[1..], "your game").unwrap(), "Report: Jamie reported Bob (wallhacking) in your game");
        assert_eq!(report_note(&list[..1], "Pub").unwrap(), "Report: Ann reported Bob in Pub");
        assert!(report_note(&list, "x").unwrap().starts_with("2 new reports"));
        assert!(report_note(&[], "x").is_none());

        let dir = tmp("reports");
        std::fs::write(dir.join("reports.log"), log.join("\n")).unwrap();
        assert_eq!(new_reports(&dir).len(), 2);
        assert!(new_reports(&dir).is_empty(), "shown once");
        assert!(!dir.join("reports.log").exists());
        std::fs::write(dir.join("reports.log"), "1700000300\tSTEAM_0:0:9\tAnn\tSTEAM_0:0:8\tCy\tde_nuke\tcamping\n").unwrap();
        let fresh = new_reports(&dir);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].about, "Cy");
        let _l = LOCK.lock().unwrap();
        assert_eq!(reports(&dir).len(), 3);
        drop(_l);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
