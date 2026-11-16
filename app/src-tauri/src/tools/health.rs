//! The map simulator (was "map health check"; the routes keep the old name): bots play a map on a hidden server for a few minutes, recorded by
//! gm_simrec, and the recording and the server's console become a plain-language report
//! (falls, stuck spots, how much of the map they used, spawn sitters, plugin errors,
//! crashes). It's the simulation's sweep (loner-strike-sim/sweep.sh, run-wine.sh and
//! sweepreport.py) inside the app.
//!
//! The hidden server is a copy of one of the player's own servers on this computer (so it
//! has Loner-Strike's engine, the bot brain, the plugins and the modes), kept in
//! `<servers folder>/.health-check` and brought up to date before each run (only changed
//! files are copied; maps aren't: the map being checked comes from the player's game, with
//! its nav and knowledge). gm_simrec is compiled there once. Nothing of the copy is ever a
//! server the app lists; it listens on 127.0.0.1 only and isn't announced anywhere.
//!
//! One check at a time, never while the game or one of the player's servers is running
//! (it stops itself if either starts), stoppable, and it tidies up after itself. Reports
//! are kept as JSON in `<app dir>/tools/health/`, the recording (and the bots' trace) in
//! `<app dir>/tools/recordings/` so the Replay tool lists it.
//!
//! App window only (merged into tools::local_routes). Errors are `{error}` with status 200;
//! POSTs need the panel header.

use super::replay::{self, Recording};
use crate::goldsrc::nav::Nav;
use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const NO_GAME: &str = "Set up the game on this computer first.";
const BAD_NAME: &str = "That isn't a map name.";
const INTERNAL: &str = "Loner-Strike hit an internal error with the map simulator. Try again, and copy the diagnostics from Settings if it keeps happening.";

/// Seconds between recorded samples (gm_simrec's SAMPLE).
const SAMPLE: f32 = 0.1;
/// Run length and bot count limits.
pub const MINUTES: (u32, u32) = (1, 15);
pub const BOTS: (u32, u32) = (2, 20);
/// Reports kept, and health-check recordings kept in the recordings folder.
const KEEP_REPORTS: usize = 100;
const KEEP_RECORDINGS: usize = 20;
/// The most of the console we read (its end).
const MAX_CONSOLE: u64 = 8 << 20;
/// The map picture's grids: at most this many cells a side (the cell grows past 128 units).
const HEAT_SIDE: usize = 80;
/// Unvisited nav areas kept for the picture (the biggest first).
const MAX_UNVISITED: usize = 4000;

// ---- what modes expect ---------------------------------------------------------------------
// (as sweepreport.py: modes where few kills are normal, deaths to the world are the game, or
// bots barely move)

const FEW_KILLS: [&str; 5] = ["kreedz", "soccerjam", "basebuilder", "hns", "deathrun"];
const WORLD_DEATHS_OK: [&str; 3] = ["deathrun", "kreedz", "surf"];
const STILL_OK: [&str; 3] = ["basebuilder", "hns", "kreedz"];
/// Modes whose own plugin drives the bots (bot_ls_fight 0): no map knowledge by design.
const NO_FIGHT: [&str; 8] = ["basebuilder", "deathrun", "hns", "kreedz", "soccerjam", "surf", "zombie", "zp5"];

// ---- the report ----------------------------------------------------------------------------

/// A place on the map where something happened, more than once maybe.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Spot {
    /// GoldSrc units: where the bot stood (for a fall, where it last stood before it)
    pub at: [f32; 3],
    /// the nav's place name there ("" when the nav names none)
    pub place: String,
    pub count: usize,
    /// for stuck spots: seconds stuck in all; for falls: the biggest drop
    pub seconds: f32,
    pub drop: f32,
    /// the moment to watch in Replay (game time in the kept recording) and who: the worst
    /// case here, when it's in that recording
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<u8>,
}

/// One thing that happened somewhere: (where, weight, drop, the moment and who, if it's in
/// the kept recording).
type Item = ([f32; 3], f32, f32, Option<(f32, u8)>);

/// One plugin's errors in the console.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginErrors {
    pub plugin: String,
    pub count: usize,
    pub first: String,
}

/// A plain sentence for the top of the report: "bad" (something broke), "warn" (worth a
/// look) or "good".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub level: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot: Option<Spot>,
    /// "engine": a crash or engine error line; the page offers "It's fine" to put it away
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// A place where many fights ended: deaths to enemies there, and which side died.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FightSpot {
    /// where most of them died
    pub at: [f32; 3],
    pub place: String,
    pub deaths: usize,
    pub t_deaths: usize,
    pub ct_deaths: usize,
}

/// Grids over the area played in, for the map picture: cell (cx, cy) covers x0 + cx*cell ..
/// and y0 + cy*cell .., row-major.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct HeatGrid {
    pub cell: f32,
    pub x0: f32,
    pub y0: f32,
    pub w: usize,
    pub h: usize,
    /// seconds bots spent there (alive, in play)
    pub time: Vec<f32>,
    /// where bots were killed by enemies
    pub deaths: Vec<u16>,
    /// where the killers stood
    pub kills: Vec<u16>,
}

/// One finished round: how long it was played (after the freeze) and who won.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub seconds: f32,
    /// 1 the Terrorists, 2 the CTs, 0 not known (a draw, or a recording without it)
    pub winner: u8,
}

/// What the console said.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Console {
    pub fatal: Vec<String>,
    pub plugin_errors: usize,
    pub by_plugin: Vec<PluginErrors>,
    /// "loaded", "rebuilt" (the shipped file didn't fit) or "none"
    pub knowledge: &'static str,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Report {
    pub id: String,
    pub map: String,
    pub mode: String,
    pub mode_name: String,
    pub game: String,
    /// the server the hidden copy was made from
    pub server: String,
    /// unix seconds, when the run started
    pub when: u64,
    pub minutes: u32,
    pub bots_wanted: u32,
    /// seconds of game recorded
    pub seconds: f32,
    pub bots_seen: usize,
    pub bot_minutes: f32,
    pub kills: usize,
    pub kills_per_bot_min: f32,
    /// stuck (trying to move but staying within 160 units for 8 s+, no enemy in view, in
    /// play; replay.rs), % of bot time
    pub stuck_pct: f32,
    /// standing still with no enemy in view, % of calm time
    pub still_pct: f32,
    /// nav areas walked, % of all
    pub coverage_pct: Option<f32>,
    pub nav_areas: Option<usize>,
    pub spawn_sitters: usize,
    pub bot_rounds: usize,
    pub deaths: usize,
    pub world_deaths: usize,
    pub falls: usize,
    pub fall_spots: Vec<Spot>,
    /// deaths to the map that weren't falls (hurt areas, crushers, explosions)
    pub hurt_spots: Vec<Spot>,
    pub stuck_spots: Vec<Spot>,
    /// where fights end most (the map's choke points, as bots play it)
    pub fight_spots: Vec<FightSpot>,
    /// kills of enemies by each side
    pub t_kills: usize,
    pub ct_kills: usize,
    /// the rounds that finished, in order
    pub rounds: Vec<RoundResult>,
    pub heat: Option<HeatGrid>,
    /// nav areas nobody walked over: [x0, y0, x1, y1, z] (at most MAX_UNVISITED)
    pub unvisited: Vec<[f32; 5]>,
    /// named places where nobody went at all
    pub unvisited_places: Vec<String>,
    pub knowledge: String,
    pub fatal: Vec<String>,
    pub plugin_errors: usize,
    pub plugin_error_list: Vec<PluginErrors>,
    /// why it ended before its time, if it did ("the server closed by itself")
    pub ended_early: Option<String>,
    /// compared with the other maps of the same mode checked here
    pub compared: Vec<String>,
    pub findings: Vec<Finding>,
    /// "good", "warn" or "bad"
    pub verdict: String,
    /// the recording's file in the recordings folder (for Replay)
    pub recording: Option<String>,
}

/// What a run was asked for.
#[derive(Debug, Clone, Default)]
pub struct Asked {
    pub map: String,
    pub mode: String,
    pub bots: u32,
    pub minutes: u32,
}

fn bad_line() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)FATAL ERROR|Host_Error|Segmentation fault|Mod_LoadModel|Unhandled exception").unwrap())
}

/// What an engine error line in the console says to the person. "Unable to initialize Steam" isn't a
/// crash: the hidden server couldn't reach Steam's library as it started, on Windows usually because
/// the firewall is asking about it the first time (the user's Windows test of the 1.0.0 draft).
pub fn engine_finding(line: &str, windows: bool) -> String {
    if line.contains("Unable to initialize Steam") {
        return if windows {
            "The hidden server couldn't reach Steam as it started. On Windows that's usually the firewall asking about it the first time: choose Allow access in the Windows Firewall prompt, and the next run works.".into()
        } else {
            "The hidden server couldn't reach Steam's library as it started. Run it again; it usually works the second time.".into()
        };
    }
    format!("The server crashed or hit an engine error: \u{201c}{line}\u{201d}")
}

/// Written into the hidden server's console log before a retry: the report reads only what comes
/// after the last one, so a start that failed and was tried again isn't reported as a crash.
pub const RESTART_MARK: &str = "=== Loner-Strike: the hidden server was started again ===";

/// A console log's text from the last restart on.
pub fn after_restart(text: &str) -> &str {
    text.rfind(RESTART_MARK).map(|i| &text[i + RESTART_MARK.len()..]).unwrap_or(text)
}

/// Why to start the hidden server again (None: don't). Steam that didn't answer as the server came
/// up (it closed): on Windows the firewall prompt, tried up to twice. A start that hung: Valve's
/// HLDS under CrossOver sometimes stops at `Failed to load "SDL3.dll"` on the first map change
/// (final check 2026-11-15), or nothing came up in 4 minutes: tried once.
pub fn retry_reason(log: &str, closed: bool, waited: Duration, steam_retries: u32, hang_retries: u32) -> Option<&'static str> {
    retry_reason_with(log, closed, waited, steam_retries, hang_retries, Some(Duration::from_secs(240)))
}

/// retry_reason for a hidden server that works without answering for minutes (the map knowledge
/// builder): `silent_limit` None retries a hang only on the SDL3 line.
pub fn retry_reason_with(log: &str, closed: bool, waited: Duration, steam_retries: u32, hang_retries: u32, silent_limit: Option<Duration>) -> Option<&'static str> {
    let log = after_restart(log);
    // (Steam: when it closed, or sitting with its FATAL ERROR: under CrossOver it doesn't end)
    if log.contains("Unable to initialize Steam") {
        return (steam_retries < 2).then_some("steam");
    }
    if closed {
        return None;
    }
    if hang_retries >= 1 {
        return None;
    }
    if log.contains("Failed to load \"SDL3.dll\"") && waited >= Duration::from_secs(30) {
        return Some("hang");
    }
    silent_limit.is_some_and(|l| waited > l).then_some("hang")
}

/// What a hidden server that closed by itself said, for the error a tool shows: Steam that didn't
/// answer is the firewall prompt on Windows (engine_finding), anything else is `otherwise`.
pub fn closed_why(log: &str, otherwise: &str) -> String {
    let last = after_restart(log);
    match last.lines().find(|l| l.contains("Unable to initialize Steam")) {
        Some(l) => engine_finding(l, cfg!(windows)),
        None => otherwise.to_string(),
    }
}

/// The hidden server started again after a retry_reason: the old one ended, the mark written into
/// its console log (`log`, appended to), a new process from `spawn`. On Windows a Steam retry waits
/// 15 s first: time to answer the firewall prompt.
pub fn restart_hidden(child: &mut std::process::Child, hidden: &Path, log: &Path, why: &str, spawn: impl FnOnce(std::fs::File) -> Result<std::process::Child, String>) -> Result<(), String> {
    pkill_hidden(hidden);
    let _ = child.kill();
    let _ = child.wait();
    // (under CrossOver the server can outlive wine's launcher: ended by its own path)
    #[cfg(target_os = "macos")]
    if let Some(win) = crate::servers::windows_path(&hidden.join("hlds.exe")) {
        crate::procmatch::end_wine_side(&win, Duration::from_secs(3));
    }
    if why == "steam" {
        std::thread::sleep(Duration::from_secs(if cfg!(windows) { 15 } else { 3 }));
    }
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(log).map_err(|e| e.to_string())?;
    use std::io::Write;
    let _ = writeln!(out, "\n{RESTART_MARK}");
    *child = spawn(out)?;
    crate::sys::tie_to_app(child);
    Ok(())
}

/// What the progress line says while a hidden server is started again.
pub fn retry_detail(why: &str) -> String {
    match why {
        "steam" if cfg!(windows) => "Steam didn't answer the hidden server: if Windows Firewall asks about it, choose Allow access. Trying again…".into(),
        "steam" => "Steam didn't answer the hidden server: trying again…".into(),
        _ => "The hidden server got stuck starting: starting it again…".into(),
    }
}

fn amx_line() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)Run time error|\[AMXX\].*(error|failed)|Plugin .* failed").unwrap())
}

/// "L 11/12/2026 - 01:33:38: text" -> "text"
fn without_stamp(line: &str) -> &str {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^L \d\d/\d\d/\d{4} - \d\d:\d\d:\d\d: ").unwrap());
    let l = line.trim();
    re.find(l).map(|m| l[m.end()..].trim()).unwrap_or(l)
}

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// The plugin a console error line names (`plugin "x.amxx"` or `[x.amxx]`).
fn plugin_of(line: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r#"(?i)plugin "([^"]{1,64})"|\[([a-z0-9_\-]{1,64}\.amxx)\]"#).unwrap());
    re.captures(line).and_then(|c| c.get(1).or_else(|| c.get(2))).map(|m| m.as_str().to_string()).unwrap_or_else(|| "AMX Mod X".into())
}

/// Crashes, plugin errors and whether the bots' map knowledge loaded, from the console.
pub fn read_console(text: &str) -> Console {
    let mut c = Console { knowledge: "none", ..Default::default() };
    let mut fatal: Vec<String> = Vec::new();
    let mut plugins: BTreeMap<String, PluginErrors> = BTreeMap::new();
    for line in text.lines() {
        if bad_line().is_match(line) {
            let l = cut(without_stamp(line), 140);
            if !fatal.contains(&l) {
                fatal.push(l);
            }
        }
        if amx_line().is_match(line) {
            c.plugin_errors += 1;
            let name = plugin_of(line);
            let e = plugins.entry(name.clone()).or_insert_with(|| PluginErrors { plugin: name, count: 0, first: cut(without_stamp(line), 160) });
            e.count += 1;
        }
    }
    fatal.sort();
    fatal.truncate(3);
    c.fatal = fatal;
    let mut list: Vec<PluginErrors> = plugins.into_values().collect();
    list.sort_by(|a, b| b.count.cmp(&a.count).then(a.plugin.cmp(&b.plugin)));
    c.by_plugin = list;
    static LOADED: OnceLock<regex::Regex> = OnceLock::new();
    static REBUILT: OnceLock<regex::Regex> = OnceLock::new();
    if LOADED.get_or_init(|| regex::Regex::new(r"map knowledge for \S+ loaded").unwrap()).is_match(text) {
        c.knowledge = "loaded";
    } else if REBUILT.get_or_init(|| regex::Regex::new(r"map knowledge for \S+: \d+ areas").unwrap()).is_match(text) {
        c.knowledge = "rebuilt";
    }
    c
}

/// The nav's place name at a point (the area under it, else the nearest area within 256
/// units); "" when there's none.
pub fn place_at(nav: Option<&Nav>, p: [f32; 3]) -> String {
    let Some(nav) = nav else { return String::new() };
    let i = nav.area_at(p, 120.0).or_else(|| {
        nav.areas
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let c = a.closest_point(p);
                (i, (c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + ((c[2] - p[2]) * 2.0).powi(2))
            })
            .filter(|(_, d)| *d < 256.0 * 256.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    i.map(|i| nav.place_name(&nav.areas[i]).to_string()).unwrap_or_default()
}

/// Spots grouped by place name (or, without one, by a grid cell of `cell` units), biggest
/// first. Each item: (where, weight, drop).
fn group_spots(items: &[Item], nav: Option<&Nav>, cell: f32) -> Vec<Spot> {
    let mut groups: Vec<(String, Spot, f32)> = Vec::new();
    for &(at, weight, drop, moment) in items {
        let (t, bot) = moment.map(|(t, b)| (Some(t), Some(b))).unwrap_or((None, None));
        let place = place_at(nav, at);
        let key = if place.is_empty() {
            format!("{},{},{}", (at[0] / cell).floor(), (at[1] / cell).floor(), (at[2] / cell).floor())
        } else {
            place.clone()
        };
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => {
                g.1.count += 1;
                g.1.seconds += weight;
                g.1.drop = g.1.drop.max(drop);
                // the spot shown is the one with the most in it
                if weight > g.2 {
                    g.1.at = at;
                    g.2 = weight;
                    if t.is_some() {
                        (g.1.t, g.1.bot) = (t, bot);
                    }
                } else if g.1.t.is_none() && t.is_some() {
                    (g.1.t, g.1.bot) = (t, bot);
                }
            }
            None => groups.push((key, Spot { at, place, count: 1, seconds: weight, drop, t, bot }, weight)),
        }
    }
    let mut out: Vec<Spot> = groups.into_iter().map(|g| g.1).collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then(b.seconds.total_cmp(&a.seconds)));
    for s in &mut out {
        s.at = s.at.map(|v| v.round());
        s.seconds = (s.seconds * 10.0).round() / 10.0;
        s.drop = s.drop.round();
        s.t = s.t.map(|t| (t * 10.0).round() / 10.0);
    }
    out
}

/// In play: not between a round's end and the next start (round-end pause, freeze time).
fn in_play(rec: &Recording, t: f32) -> bool {
    let last_end = rec.rounds.iter().filter_map(|r| r.end).filter(|&e| e <= t).fold(None, |m: Option<f32>, e| Some(m.map_or(e, |m| m.max(e))));
    match last_end {
        None => true,
        Some(e) => rec.rounds.iter().any(|r| r.start > e && r.start <= t),
    }
}

fn round1(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// The numbers from the recordings (one run can leave more than one: the map loads twice
/// at the start) and the console, and what they mean.
pub fn build_report(recs: &[Recording], console: &str, nav: Option<&Nav>, asked: &Asked) -> Report {
    let mut r = Report { map: asked.map.clone(), mode: asked.mode.clone(), minutes: asked.minutes, bots_wanted: asked.bots, ..Default::default() };
    let (mut alive, mut calm, mut still) = (0usize, 0usize, 0usize);
    let mut stuck_secs = 0.0f32;
    let mut visited: HashSet<usize> = HashSet::new();
    let mut ids: HashSet<u8> = HashSet::new();
    let mut stuck_items = Vec::new();
    let mut fall_items = Vec::new();
    let mut hurt_items = Vec::new();
    // the recording kept for Replay (run() keeps the one with the most samples)
    let main = recs.iter().enumerate().max_by_key(|(_, r)| r.cols.t.len()).map(|(i, _)| i);
    for (ri, rec) in recs.iter().enumerate() {
        let here = |t: f32, id: u8| (Some(ri) == main).then_some((t, id));
        r.seconds += rec.seconds();
        let any_bots = rec.players.iter().any(|p| p.bot);
        let players: Vec<&replay::Player> = rec.players.iter().filter(|p| p.count > 0 && (p.bot || !any_bots)).collect();
        let who: HashSet<u8> = players.iter().map(|p| p.id).collect();
        ids.extend(who.iter().copied());
        let c = &rec.cols;
        let mut starts: Vec<f32> = rec.rounds.iter().map(|r| r.start).collect();
        if starts.is_empty() {
            starts.push(rec.first_time().unwrap_or(0.0));
        }
        for p in &players {
            let range = p.start..p.start + p.count;
            for i in range.clone() {
                if (i - p.start) % 2 == 0 {
                    if let Some(a) = nav.and_then(|n| n.area_at([c.x[i], c.y[i], c.z[i]], 120.0)) {
                        visited.insert(a);
                    }
                }
                if !in_play(rec, c.t[i]) {
                    continue;
                }
                alive += 1;
                if c.venemy[i] == 0 {
                    calm += 1;
                    if c.vx[i].hypot(c.vy[i]) < 5.0 {
                        still += 1;
                    }
                }
            }
            // spawn sitters: alive 30 s or more into a round and never 300 units from where it began
            for (k, &t0) in starts.iter().enumerate() {
                let t1 = starts.get(k + 1).copied().unwrap_or(f32::INFINITY);
                let rs: Vec<usize> = range.clone().filter(|&i| c.t[i] >= t0 && c.t[i] < t1 && c.hp[i] > 0).collect();
                let (Some(&a), Some(&b)) = (rs.first(), rs.last()) else { continue };
                if rs.len() < 2 || c.t[b] - c.t[a] < 30.0 {
                    continue;
                }
                r.bot_rounds += 1;
                let far = rs.iter().map(|&i| (c.x[i] - c.x[a]).hypot(c.y[i] - c.y[a])).fold(0.0f32, f32::max);
                if far < 300.0 {
                    r.spawn_sitters += 1;
                }
            }
        }
        for s in rec.stuck.iter().filter(|s| who.contains(&s.id)) {
            let secs = s.to - s.from;
            stuck_secs += secs;
            stuck_items.push((s.at, secs, 0.0, here(s.from, s.id)));
        }
        for k in &rec.kills {
            r.deaths += 1;
            if k.cause == "enemy" {
                continue;
            }
            r.world_deaths += 1;
            match (&k.fall, k.at) {
                (Some(f), _) => {
                    r.falls += 1;
                    fall_items.push((f.from, 1.0, f.drop, here(f.from_t, k.victim)));
                }
                (None, Some(at)) if k.cause != "suicide" => hurt_items.push((at, 1.0, 0.0, here(k.t, k.victim))),
                _ => {}
            }
        }
    }
    r.bots_seen = ids.len();
    r.kills = r.deaths;
    let alive_min = alive as f32 * SAMPLE / 60.0;
    r.bot_minutes = (alive_min * 10.0).round() / 10.0;
    if alive > 0 {
        r.kills_per_bot_min = round1(r.kills as f32 / alive_min);
        r.stuck_pct = round1(100.0 * stuck_secs / (alive as f32 * SAMPLE));
        r.still_pct = round1(100.0 * still as f32 / calm.max(1) as f32);
    }
    if let Some(n) = nav.filter(|n| !n.areas.is_empty()) {
        r.nav_areas = Some(n.areas.len());
        r.coverage_pct = Some(round1(100.0 * visited.len() as f32 / n.areas.len() as f32));
    }
    r.fall_spots = group_spots(&fall_items, nav, 256.0);
    r.hurt_spots = group_spots(&hurt_items, nav, 256.0);
    for s in r.fall_spots.iter_mut().chain(r.hurt_spots.iter_mut()) {
        s.seconds = 0.0;
    }
    let mut stuck = group_spots(&stuck_items, nav, 128.0);
    stuck.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
    stuck.truncate(8);
    r.stuck_spots = stuck;
    insights(&mut r, recs, nav, &visited);
    let con = read_console(console);
    r.knowledge = con.knowledge.to_string();
    r.fatal = con.fatal;
    r.plugin_errors = con.plugin_errors;
    r.plugin_error_list = con.by_plugin.into_iter().take(5).collect();
    r.findings = findings(&r);
    r.verdict = verdict(&r.findings);
    r
}

/// The team in player `id`'s sample nearest `t` (0 when there's none).
fn team_at(rec: &Recording, id: u8, t: f32) -> u8 {
    rec.sample_at(id, t).filter(|&i| (rec.cols.t[i] - t).abs() < 2.0).map(|i| rec.cols.team[i]).unwrap_or(0)
}

/// What the simulation's own analysis gets from a recording beyond the problems: rounds and
/// who won them, kills by side, where fights end (choke points), grids of where bots spent
/// their time, died and killed from, and the nav areas and places nobody went.
fn insights(r: &mut Report, recs: &[Recording], nav: Option<&Nav>, visited: &HashSet<usize>) {
    for rec in recs {
        for rd in &rec.rounds {
            if let Some(e) = rd.end.filter(|&e| e > rd.start) {
                r.rounds.push(RoundResult { seconds: ((e - rd.start) * 10.0).round() / 10.0, winner: rd.winner.unwrap_or(0) });
            }
        }
    }

    // the grids: over everywhere anyone stood
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for rec in recs {
        for i in 0..rec.cols.t.len() {
            lo = [lo[0].min(rec.cols.x[i]), lo[1].min(rec.cols.y[i])];
            hi = [hi[0].max(rec.cols.x[i]), hi[1].max(rec.cols.y[i])];
        }
    }
    let mut grid = None;
    if lo[0] <= hi[0] {
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]);
        let cell = ((span / HEAT_SIDE as f32 / 32.0).ceil() * 32.0).max(128.0);
        let (x0, y0) = ((lo[0] / cell).floor() * cell, (lo[1] / cell).floor() * cell);
        let w = (((hi[0] - x0) / cell).floor() as usize + 1).min(HEAT_SIDE * 2);
        let h = (((hi[1] - y0) / cell).floor() as usize + 1).min(HEAT_SIDE * 2);
        grid = Some(HeatGrid { cell, x0, y0, w, h, time: vec![0.0; w * h], deaths: vec![0; w * h], kills: vec![0; w * h] });
    }
    let cell_of = |g: &HeatGrid, x: f32, y: f32| -> Option<usize> {
        let (cx, cy) = (((x - g.x0) / g.cell).floor(), ((y - g.y0) / g.cell).floor());
        (cx >= 0.0 && cy >= 0.0 && (cx as usize) < g.w && (cy as usize) < g.h).then(|| cy as usize * g.w + cx as usize)
    };

    let mut fights: Vec<([f32; 3], u8)> = Vec::new();
    for rec in recs {
        let c = &rec.cols;
        if let Some(g) = grid.as_mut() {
            for i in 0..c.t.len() {
                if c.hp[i] > 0 && in_play(rec, c.t[i]) {
                    if let Some(k) = cell_of(g, c.x[i], c.y[i]) {
                        g.time[k] += SAMPLE;
                    }
                }
            }
        }
        for k in rec.kills.iter().filter(|k| k.cause == "enemy" && !k.team_kill) {
            let victim_team = team_at(rec, k.victim, k.t);
            let killer = rec.sample_at(k.killer, k.t).filter(|&i| (c.t[i] - k.t).abs() < 1.0);
            match killer.map(|i| c.team[i]).filter(|&t| t != 0).unwrap_or(if victim_team == 1 { 2 } else if victim_team == 2 { 1 } else { 0 }) {
                1 => r.t_kills += 1,
                2 => r.ct_kills += 1,
                _ => {}
            }
            if let Some(g) = grid.as_mut() {
                if let Some(at) = k.at {
                    if let Some(x) = cell_of(g, at[0], at[1]) {
                        g.deaths[x] = g.deaths[x].saturating_add(1);
                    }
                }
                if let Some(i) = killer {
                    if let Some(x) = cell_of(g, c.x[i], c.y[i]) {
                        g.kills[x] = g.kills[x].saturating_add(1);
                    }
                }
            }
            if let Some(at) = k.at {
                fights.push((at, victim_team));
            }
        }
    }
    if let Some(g) = grid.as_mut() {
        for v in &mut g.time {
            *v = (*v * 10.0).round() / 10.0;
        }
    }
    r.heat = grid;

    // where fights end, by place (or a 256-unit cell without one)
    let mut groups: Vec<(String, FightSpot, HashMap<(i32, i32), usize>)> = Vec::new();
    for (at, team) in fights {
        let place = place_at(nav, at);
        let key = if place.is_empty() { format!("{},{}", (at[0] / 256.0).floor(), (at[1] / 256.0).floor()) } else { place.clone() };
        let i = match groups.iter().position(|g| g.0 == key) {
            Some(i) => i,
            None => {
                groups.push((key, FightSpot { at, place, ..Default::default() }, HashMap::new()));
                groups.len() - 1
            }
        };
        let g = &mut groups[i];
        g.1.deaths += 1;
        match team {
            1 => g.1.t_deaths += 1,
            2 => g.1.ct_deaths += 1,
            _ => {}
        }
        // the spot shown: the busiest 128-unit cell in the group
        let cellkey = ((at[0] / 128.0).floor() as i32, (at[1] / 128.0).floor() as i32);
        let n = g.2.entry(cellkey).or_insert(0);
        *n += 1;
        if *n == g.2.values().copied().max().unwrap_or(0) {
            g.1.at = at.map(|v| v.round());
        }
    }
    let mut spots: Vec<FightSpot> = groups.into_iter().map(|g| g.1).collect();
    spots.sort_by(|a, b| b.deaths.cmp(&a.deaths));
    spots.truncate(8);
    r.fight_spots = spots;

    // where nobody went
    if let Some(n) = nav.filter(|n| !n.areas.is_empty() && r.seconds > 0.0) {
        let mut un: Vec<[f32; 5]> = n
            .areas
            .iter()
            .enumerate()
            .filter(|(i, _)| !visited.contains(i))
            .map(|(_, a)| [a.lo[0].min(a.hi[0]).round(), a.lo[1].min(a.hi[1]).round(), a.lo[0].max(a.hi[0]).round(), a.lo[1].max(a.hi[1]).round(), ((a.lo[2] + a.hi[2]) / 2.0).round()])
            .collect();
        un.sort_by(|a, b| ((b[2] - b[0]) * (b[3] - b[1])).total_cmp(&((a[2] - a[0]) * (a[3] - a[1]))));
        un.truncate(MAX_UNVISITED);
        r.unvisited = un;
        let mut seen: BTreeMap<String, bool> = BTreeMap::new();
        for (i, a) in n.areas.iter().enumerate() {
            let name = n.place_name(a);
            if !name.is_empty() {
                *seen.entry(name.to_string()).or_insert(false) |= visited.contains(&i);
            }
        }
        r.unvisited_places = seen.into_iter().filter(|(_, v)| !v).map(|(k, _)| k).collect();
    }
}

fn verdict(f: &[Finding]) -> String {
    if f.iter().any(|f| f.level == "bad") {
        "bad"
    } else if f.iter().any(|f| f.level == "warn") {
        "warn"
    } else {
        "good"
    }
    .into()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "1 min 20 s", "45 s"
pub fn duration(secs: f32) -> String {
    let s = secs.round() as u64;
    if s < 60 {
        format!("{s} s")
    } else if s % 60 == 0 {
        format!("{} min", s / 60)
    } else {
        format!("{} min {} s", s / 60, s % 60)
    }
}

fn near(s: &Spot) -> String {
    if s.place.is_empty() {
        format!("the spot at {:.0} {:.0} {:.0}", s.at[0], s.at[1], s.at[2])
    } else {
        s.place.clone()
    }
}

fn minutes_text(secs: f32) -> String {
    let m = (secs / 60.0).round().max(1.0) as usize;
    plural(m, "minute", "minutes")
}

/// The share of the map (%) under which bots "only used" it: 20 % from 40 bot-minutes on,
/// less for shorter runs.
fn coverage_floor(bot_minutes: f32) -> f32 {
    20.0 * (bot_minutes / 40.0).clamp(0.0, 1.0).sqrt()
}

/// The sentences at the top: what broke first, then what's worth a look, else that it's fine.
pub fn findings(r: &Report) -> Vec<Finding> {
    let mode = r.mode.as_str();
    let mut bad = Vec::new();
    let mut warn = Vec::new();
    let say = |v: &mut Vec<Finding>, level: &str, text: String, spot: Option<Spot>| v.push(Finding { level: level.into(), text, spot, kind: None });
    if let Some(why) = &r.ended_early {
        say(&mut bad, "bad", format!("The check ended early: {why}"), None);
    }
    if let Some(l) = r.fatal.first() {
        bad.push(Finding { level: "bad".into(), text: engine_finding(l, cfg!(windows)), spot: None, kind: Some("engine".into()) });
    }
    for p in r.plugin_error_list.iter().take(2) {
        say(&mut bad, "bad", format!("{} had {} in the console. The first: \u{201c}{}\u{201d}", p.plugin, plural(p.count, "error", "errors"), p.first), None);
    }
    let few_wanted = (r.bots_wanted as usize / 2).max(2);
    if r.bots_seen < few_wanted && mode != "kreedz" {
        say(&mut bad, "bad", format!("Only {} of {} bots joined the game.", r.bots_seen, r.bots_wanted), None);
    }
    let little = (0.15 * r.bots_wanted as f32 * r.minutes as f32).max(2.0);
    if r.bot_minutes < little && mode != "kreedz" {
        say(&mut bad, "bad", "Bots hardly played: there's little or no recording. The map may not have loaded, or bots couldn't spawn.".into(), None);
    }
    let mins = minutes_text(r.seconds);
    if !WORLD_DEATHS_OK.contains(&mode) {
        for s in r.fall_spots.iter().filter(|s| s.count >= 3).take(3) {
            say(&mut warn, "warn", format!("Bots fall a lot near {} \u{2014} {} times in {mins}.", near(s), s.count), Some(s.clone()));
        }
        for s in r.hurt_spots.iter().filter(|s| s.count >= 3).take(2) {
            say(&mut warn, "warn", format!("The map kills bots near {} \u{2014} {} times in {mins}.", near(s), s.count), Some(s.clone()));
        }
        if r.deaths >= 6 && r.world_deaths as f32 / r.deaths as f32 > 0.2 {
            say(&mut warn, "warn", format!("{} of {} deaths were falls or the map itself, not other players.", r.world_deaths, r.deaths), None);
        }
    }
    if !STILL_OK.contains(&mode) {
        for s in r.stuck_spots.iter().filter(|s| s.seconds >= 45.0).take(3) {
            say(&mut warn, "warn", format!("Bots get stuck near {}: {} in all, {}.", near(s), duration(s.seconds), plural(s.count, "time", "times")), Some(s.clone()));
        }
        if r.bot_rounds >= 6 && r.spawn_sitters as f32 / r.bot_rounds as f32 > 0.25 {
            say(&mut warn, "warn", format!("Bots often never leave their spawn: {} of {} times in a round.", r.spawn_sitters, r.bot_rounds), None);
        }
    }
    let fights = !NO_FIGHT.contains(&mode);
    if r.knowledge == "rebuilt" {
        say(&mut warn, "warn", "The bots' map knowledge didn't fit this map, so they worked it out again. Re-teach the map so the saved file matches.".into(), None);
    } else if r.knowledge == "none" && fights && r.bot_minutes >= little {
        say(&mut warn, "warn", "The bots' map knowledge for this map never loaded, so they played it without knowing its cover and angles.".into(), None);
    }
    // how much of the map bots cover grows with play: judge it only on a run long enough (3+
    // minutes, 20+ bot-minutes), and expect less from shorter ones (QA4 T1-5)
    if let Some(c) = r.coverage_pct.filter(|_| r.bot_minutes >= little.max(20.0) && r.seconds >= 180.0 && !STILL_OK.contains(&mode)) {
        if c < coverage_floor(r.bot_minutes) {
            say(&mut warn, "warn", format!("Bots only used {c:.0}% of the map. Parts of it may be hard to reach, or not in the nav."), None);
        }
    }
    let (t_wins, ct_wins) = (r.rounds.iter().filter(|x| x.winner == 1).count(), r.rounds.iter().filter(|x| x.winner == 2).count());
    let decided = t_wins + ct_wins;
    if decided >= 8 && fights {
        let (side, won) = if t_wins >= ct_wins { ("Terrorists", t_wins) } else { ("CTs", ct_wins) };
        if won as f32 / decided as f32 >= 0.75 {
            say(&mut warn, "warn", format!("The {side} won {won} of {decided} rounds. With bots on both sides, that points at the map favouring them (how far each side runs, a choke one side can't take)."), None);
        }
    }
    if !r.unvisited_places.is_empty() && r.seconds >= 240.0 && r.bot_minutes >= little && !STILL_OK.contains(&mode) {
        let n = r.unvisited_places.len();
        let shown: Vec<&str> = r.unvisited_places.iter().take(5).map(|s| s.as_str()).collect();
        let more = if n > 5 { format!(" and {} more", n - 5) } else { String::new() };
        say(&mut warn, "warn", format!("Bots never went to {}{more}. {} may be hard to reach, cut off in the nav, or not worth going to.", shown.join(", "), if n == 1 { "It" } else { "They" }), None);
    }
    for c in &r.compared {
        say(&mut warn, "warn", c.clone(), None);
    }
    let mut out = bad;
    out.extend(warn);
    if out.is_empty() {
        let used = r.coverage_pct.map(|c| format!(", used {c:.0}% of the map")).unwrap_or_default();
        let fought = if FEW_KILLS.contains(&mode) { String::new() } else { format!(" ({:.2} kills a bot-minute)", r.kills_per_bot_min) };
        out.push(Finding { level: "good".into(), text: format!("No problems found. Bots played{fought}{used} and rarely got stuck or fell."), spot: None, kind: None });
    }
    out
}

/// Notes against the other maps of the same mode checked on this computer (or all fighting
/// modes when fewer than 4 maps of this one were): stuck, standing still, kills, how much of
/// the map was used. Far off the middle (3 MADs, at least a margin) is worth saying.
pub fn compare(r: &Report, others: &[Report]) -> Vec<String> {
    let mut out = Vec::new();
    let mode = r.mode.as_str();
    // one report per other map: the newest
    let mut latest: HashMap<String, &Report> = HashMap::new();
    for o in others.iter().filter(|o| !o.map.eq_ignore_ascii_case(&r.map) && o.bot_minutes >= 5.0 && o.ended_early.is_none()) {
        let k = o.map.to_lowercase();
        if latest.get(&k).map(|x| o.when > x.when).unwrap_or(true) {
            latest.insert(k, o);
        }
    }
    let pool: Vec<&Report> = latest.into_values().collect();
    type Get = fn(&Report) -> Option<f32>;
    let checks: [(Get, f32, f32, &str); 4] = [
        (|x| Some(x.stuck_pct), 1.0, 4.0, "Bots get stuck far more than on other {g} maps"),
        (|x| Some(x.still_pct), 1.0, 15.0, "Bots stand still far more than on other {g} maps"),
        (|x| Some(x.kills_per_bot_min), -1.0, 0.1, "Far fewer kills than on other {g} maps"),
        (|x| x.coverage_pct, -1.0, 15.0, "Bots use far less of the map than on other {g} maps"),
    ];
    for (i, (get, worse, margin, label)) in checks.iter().enumerate() {
        let Some(v) = get(r) else { continue };
        if i == 2 && FEW_KILLS.contains(&mode) || i == 1 && STILL_OK.contains(&mode) {
            continue;
        }
        let mut group = mode.to_string();
        let mut vals: Vec<f32> = pool.iter().filter(|o| o.mode == mode).filter_map(|o| get(o)).collect();
        if vals.len() < 4 {
            group = "fighting".into();
            vals = pool.iter().filter(|o| !FEW_KILLS.contains(&o.mode.as_str()) && !STILL_OK.contains(&o.mode.as_str())).filter_map(|o| get(o)).collect();
        }
        if vals.len() < 4 {
            continue;
        }
        let med = median(&mut vals);
        let mut dev: Vec<f32> = vals.iter().map(|x| (x - med).abs()).collect();
        let mad = median(&mut dev);
        if (v - med) * worse > (3.0 * mad).max(*margin) {
            out.push(format!("{} ({v:.2} here, {med:.2} usually).", label.replace("{g}", &group)));
        }
    }
    out
}

fn median(v: &mut [f32]) -> f32 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// The mode a map starts in by the server's map presets ("surf*  surf"), else classic.
pub fn preset_mode(presets: &str, map: &str) -> Option<String> {
    let map = map.to_lowercase();
    presets.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with("//")).find_map(|l| {
        let mut it = l.split_whitespace();
        let (pat, mode) = (it.next()?.to_lowercase(), it.next()?);
        let hit = match pat.strip_suffix('*') {
            Some(prefix) => map.starts_with(prefix),
            None => map == pat,
        };
        hit.then(|| mode.to_string())
    })
}

// ---- the hidden server ----------------------------------------------------------------------

/// The recorder (loner-strike-sim/gm_simrec.sma), compiled in the hidden server once.
const SIMREC_SMA: &str = include_str!("health/gm_simrec.sma");
const HIDDEN: &str = ".health-check";
const MARK: &str = ".loner-strike-health";

/// How the run in progress (or the last one) is going.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    id: String,
    map: String,
    mode: String,
    mode_name: String,
    minutes: u32,
    bots: u32,
    server: String,
    /// "preparing", "starting", "playing", "reading" while it runs; then "done", "failed"
    /// or "stopped"
    phase: &'static str,
    detail: String,
    /// unix ms
    started: u64,
    playing_since: Option<u64>,
    error: Option<String>,
    report: Option<String>,
}

impl Run {
    fn active(&self) -> bool {
        matches!(self.phase, "preparing" | "starting" | "playing" | "reading")
    }
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);
static STOP: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn update(f: impl FnOnce(&mut Run)) {
    if let Some(r) = RUN.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        f(r);
    }
}

fn running_now() -> bool {
    RUN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(Run::active)
}

/// Why a check can't run right now: the game or one of the player's servers is running, or
/// setup is busy. None when it can.
fn user_busy() -> Option<String> {
    if crate::sys::game_running() {
        return Some("Close the game first. The map simulator runs bots on a hidden server, and doesn't while you play.".into());
    }
    if let Some(e) = crate::servers::load().into_iter().filter(|e| e.remote.is_none()).find(|e| crate::servers::running(&e.id)) {
        return Some(format!("Stop your server \u{201c}{}\u{201d} first. The map simulator doesn't run while one of your servers is running on this computer.", e.name));
    }
    None
}

fn setup_busy() -> Option<String> {
    (crate::setup::current().state == "running").then(|| "Setup is busy right now (installing, or teaching bots a map). Try again when it's done.".to_string())
}

/// The player's servers on this computer a hidden copy can be made of, for a game.
pub(crate) fn sources(game: &str) -> Vec<crate::servers::Entry> {
    crate::servers::load().into_iter().filter(|e| e.remote.is_none() && e.ready && e.game == game && e.dir.join("hlds.exe").is_file()).collect()
}

pub(crate) fn game_folder(g: &crate::game::Game) -> &'static str {
    if g.is_czero() {
        "czero"
    } else {
        "cstrike"
    }
}

/// The map's nav and knowledge in the player's game: next to the map, else in the other
/// game's maps folder.
pub(crate) fn nav_files(hl: &Path, m: &super::mapinfo::MapFile) -> (Option<PathBuf>, Option<PathBuf>) {
    let f = crate::mapcopy::Finder::default();
    let find = |ext: &str| {
        m.sibling(ext).or_else(|| ["czero", "cstrike"].iter().find_map(|g| f.find(&hl.join(g).join("maps"), &format!("{}.{ext}", m.name))))
    };
    (find("nav"), find("lsv"))
}

fn modes_of(server_game_dir: &Path) -> Vec<(String, String)> {
    crate::extras::parse_modes(&crate::sys::read_latin1(&server_game_dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default())
}

/// The modes a source has: the server's, or the game's own (Classic when it has none listed:
/// setting the hidden server up installs the modes).
fn modes_from(src: &Source, hl: &Path) -> Vec<(String, String)> {
    let m = modes_of(&src.dir.clone().unwrap_or_else(|| hl.to_path_buf()).join(&src.game));
    if m.is_empty() && src.dir.is_none() {
        return vec![("classic".into(), "Classic".into())];
    }
    m
}

/// What a run needs, checked before it starts.
/// What the hidden server is made from: one of the player's servers, or (with none on this
/// computer) the game's own files, into a temporary server removed after the run.
struct Source {
    id: String,
    name: String,
    game: String,
    /// the server's folder; None: build one from the game
    dir: Option<PathBuf>,
}

/// The mark of a hidden server built from the game (not a copy of a server).
const FROM_GAME: &str = "game";
/// How the page says it, when there's no server to copy.
const FROM_GAME_NOTE: &str = "There's no server on this computer, so the simulator makes a hidden temporary one from your game's files and removes it afterward (the first time it downloads Valve's server, about 1 GB).";

struct Plan {
    asked: Asked,
    mode_name: String,
    client: crate::steam::Install,
    src: Source,
    /// the app's resources (to set up a server from the game)
    res: Option<PathBuf>,
    map: super::mapinfo::MapFile,
    nav: PathBuf,
    lsv: Option<PathBuf>,
    hidden: PathBuf,
    id: String,
    when: u64,
}

/// Leaves out what the copy doesn't need or mustn't share: Valve's download records, maps
/// (the map being checked is copied in by itself), logs, recordings, demos, the panel's
/// password and live status.
fn copy_filter(rel: &Path) -> bool {
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    let name = parts.last().cloned().unwrap_or_default();
    let p = parts.join("/");
    !(parts.first().is_some_and(|f| f == "steamapps")
        || (parts.len() >= 3 && parts[1] == "maps")
        || (parts.len() >= 2 && parts[1] == "logs")
        || p.contains("addons/amxmodx/logs/")
        || p.contains("addons/amxmodx/data/simrec/")
        || name.ends_with(".log")
        || name.ends_with(".dem")
        || name.ends_with(".part")
        || name.ends_with(".tmp")
        || matches!(name.as_str(), "lstrace.csv" | "panel_rcon.txt" | "status.json" | "friends.cfg" | "health.cfg"))
}

/// Copies what changed (by size and time) from `from` into `to`. Never deletes.
pub(crate) fn sync_copy(from: &Path, to: &Path, stop: &AtomicBool) -> Result<usize, String> {
    let mut n = 0;
    for (k, f) in crate::archive::walk(from).into_iter().enumerate() {
        if k % 256 == 0 && stop.load(Ordering::SeqCst) {
            return Err(STOPPED.into());
        }
        let Ok(rel) = f.strip_prefix(from) else { continue };
        if !copy_filter(rel) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&f) else { continue };
        let dest = to.join(rel);
        if let Ok(d) = std::fs::metadata(&dest) {
            if d.len() == meta.len() && d.modified().ok() == meta.modified().ok() {
                continue;
            }
        }
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make the hidden server's folders ({e})."))?;
        }
        std::fs::copy(&f, &dest).map_err(|e| format!("Couldn't copy {} into the hidden server ({e}).", rel.display()))?;
        if let (Ok(t), Ok(file)) = (meta.modified(), std::fs::File::options().write(true).open(&dest)) {
            let _ = file.set_modified(t);
        }
        n += 1;
    }
    Ok(n)
}

const STOPPED: &str = "stopped";

fn write(p: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(p, text).map_err(|e| format!("Couldn't write the hidden server's {} ({e}).", p.file_name().unwrap_or_default().to_string_lossy()))
}

/// `key "value"` set in a cfg's text (replaced where it is, else added).
pub fn set_cvar(text: &str, key: &str, value: &str) -> String {
    let line = format!("{key} \"{value}\"");
    let mut found = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|l| {
            if l.split_whitespace().next() == Some(key) {
                found = true;
                line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        out.push(line);
    }
    out.join("\n") + "\n"
}

/// plugins.ini with the recorder in it.
pub fn with_recorder(ini: &str) -> String {
    if ini.lines().any(|l| l.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("gm_simrec.amxx"))) {
        return ini.to_string();
    }
    let mut s = ini.trim_end().to_string();
    s.push_str("\ngm_simrec.amxx ; Loner-Strike health check recorder\n");
    s
}

fn hash_hex(s: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Builds gm_simrec.amxx in the hidden server (once per version of its source).
fn ensure_recorder(install: &crate::steam::Install, am: &Path, server: &str) -> Result<(), String> {
    let out = am.join("plugins").join("gm_simrec.amxx");
    let stamp = am.join("plugins").join("gm_simrec.amxx.src");
    let hash = hash_hex(SIMREC_SMA);
    if out.is_file() && std::fs::read_to_string(&stamp).map(|s| s.trim() == hash).unwrap_or(false) {
        return Ok(());
    }
    let exe = am.join("scripting").join("amxxpc.exe");
    if !exe.is_file() {
        return Err(format!("\u{201c}{server}\u{201d} has no AMX Mod X compiler, which the map simulator needs once to build its recorder. Update that server's files and try again."));
    }
    if install.kind == "crossover" && !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The hidden server runs through it, like the game.".into());
    }
    let src = am.join("scripting").join("gm_simrec.sma");
    write(&src, SIMREC_SMA)?;
    let _ = std::fs::remove_file(&out);
    let mut cmd = crate::setup::core::windows_command(install, &exe);
    cmd.arg(&src).arg(format!("-o{}", out.display())).current_dir(am.join("scripting"));
    let (_, o, e) = crate::sys::run_output(cmd, Duration::from_secs(180)).ok_or("The recorder couldn't be built: the compiler didn't run.")?;
    let all = format!("{o}{e}");
    if !all.contains("Done.") || !out.is_file() {
        crate::diagnostics::log_error("health check: compile gm_simrec", &cut(&all, 3000));
        let errors: Vec<&str> = all.lines().filter(|l| l.to_lowercase().contains("error")).take(3).collect();
        return Err(format!("The recorder couldn't be built: {}", if errors.is_empty() { "the compiler gave no reason.".to_string() } else { errors.join(" / ") }));
    }
    let _ = std::fs::write(stamp, hash);
    Ok(())
}

/// The run's settings, as run-wine.sh writes them: the mode, the map, bots only (they join
/// with nobody on, and the count is the one asked for), no time or round limits.
fn configure(gdir: &Path, a: &Asked, password: &str) -> Result<(), String> {
    let am = gdir.join("addons/amxmodx");
    let data = am.join("data/gamemodes");
    write(&data.join("current.txt"), &format!("{}\n", a.mode))?;
    let mode_plugins = crate::sys::read_latin1(&am.join(format!("configs/gamemodes/{}.plugins.ini", a.mode))).unwrap_or_default();
    write(&am.join("configs/plugins-gamemode.ini"), &format!("; Generated by gamemodes.amxx - edit configs/gamemodes/{}.plugins.ini instead\n{mode_plugins}", a.mode))?;
    for f in ["lastpreset.txt", "premode.txt"] {
        let _ = std::fs::remove_file(data.join(f));
    }
    write(&data.join("lastmap.txt"), &format!("{}\n", a.map))?;
    write(&data.join("panel_rcon.txt"), password)?;
    let limits = "mp_timelimit 0\nmp_fraglimit 0\nmp_maxrounds 0\nmp_winlimit 0\n";
    // (run after every map, after ReGameDLL's game.cfg puts bot_join_after_player back)
    write(&data.join("friends.cfg"), &format!("// Loner-Strike health check\nhostname \"Loner-Strike health check\"\nsv_password \"\"\nsv_lan 1\nbot_join_after_player 0\n{limits}"))?;
    let global = crate::sys::read_latin1(&data.join("global.cfg")).unwrap_or_else(|_| "// Saved by the Game Mode Menu\n".into());
    write(&data.join("global.cfg"), &set_cvar(&global, "bot_quota", &a.bots.to_string()))?;
    write(
        &gdir.join("health.cfg"),
        &format!("sv_lan 1\nbot_join_after_player 0\nbot_auto_vacate 0\nbot_quota_mode normal\nbot_quota {}\n{limits}", a.bots),
    )?;
    let ini = crate::sys::read_latin1(&am.join("configs/plugins.ini")).unwrap_or_default();
    write(&am.join("configs/plugins.ini"), &with_recorder(&ini))?;
    Ok(())
}

/// The hidden server's start options: this computer only, never listed.
pub fn server_args(game: &str, port: u16, password: &str, map: &str) -> Vec<String> {
    [
        "-console", "-game", game, "-port", &port.to_string(), "+ip", "127.0.0.1", "-nomaster", "-insecure", "-condebug", "+sv_lan", "1", "+maxplayers", "24",
        "+rcon_password", password, "+map", map, "+exec", "health.cfg",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(unix)]
fn low_priority(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setpriority only changes the new process's own scheduling, between fork and exec
    unsafe {
        cmd.pre_exec(|| {
            libc::setpriority(libc::PRIO_PROCESS, 0, 15);
            Ok(())
        });
    }
}

#[cfg(windows)]
fn low_priority(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
    cmd.creation_flags(0x0800_0000 | 0x0000_4000);
}

pub(crate) fn spawn(client: &crate::steam::Install, dir: &Path, args: &[String], log: std::fs::File) -> Result<std::process::Child, String> {
    let hlds = dir.join("hlds.exe");
    let mut cmd = if client.kind == "crossover" {
        let mut c = crate::sys::command(crate::servers::CROSSOVER_WINE);
        let win = crate::servers::windows_path(&hlds).ok_or("The hidden server has to be inside the game's CrossOver bottle.")?;
        c.arg("--bottle").arg(client.bottle.as_deref().unwrap_or("Steam")).arg(win);
        c.env("WINEDEBUG", "-all");
        c
    } else {
        crate::sys::command(&hlds)
    };
    low_priority(&mut cmd);
    cmd.args(args).current_dir(dir).stdin(std::process::Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log);
    cmd.spawn().map_err(|e| format!("The hidden server didn't start ({e})."))
}

/// A UDP port nothing on this computer is using (the servers' own ports aside).
pub(crate) fn free_port() -> Result<u16, String> {
    let taken: HashSet<u16> = crate::servers::load().iter().filter(|e| e.remote.is_none()).map(|e| e.port).collect();
    (27150..27200)
        .find(|p| !taken.contains(p) && std::net::UdpSocket::bind(("0.0.0.0", *p)).is_ok())
        .ok_or_else(|| "No free port for the hidden server.".to_string())
}

/// The end of a text file (at most `max` bytes), as text.
pub(crate) fn tail(path: &Path, max: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(max)));
    let mut b = Vec::new();
    let _ = f.take(max).read_to_end(&mut b);
    crate::rcon::latin1(&b)
}

/// The map, its nav and knowledge, and the models and sprites it names that the hidden
/// server doesn't have, into the hidden server (whose maps folder holds only this map).
fn copy_map(p: &Plan, gdir: &Path) -> Result<(), String> {
    let maps = gdir.join("maps");
    let _ = std::fs::remove_dir_all(&maps);
    std::fs::create_dir_all(&maps).map_err(|e| format!("Couldn't make the hidden server's maps folder ({e})."))?;
    let name = &p.map.name;
    std::fs::copy(&p.map.bsp, maps.join(format!("{name}.bsp"))).map_err(|e| format!("The map couldn't be copied ({e})."))?;
    std::fs::copy(&p.nav, maps.join(format!("{name}.nav"))).map_err(|e| format!("The map's nav couldn't be copied ({e})."))?;
    if let Some(l) = &p.lsv {
        let _ = std::fs::copy(l, maps.join(format!("{name}.lsv")));
    }
    let Some(text) = crate::navs::entity_text(&p.map.bsp) else { return Ok(()) };
    let hl = &p.client.hl;
    let root = gdir.parent().unwrap_or(gdir);
    for rel in crate::navs::referenced_files(&text) {
        if [gdir.to_path_buf(), root.join("cstrike"), root.join("valve")].iter().any(|d| d.join(&rel).is_file()) {
            continue;
        }
        // where the game looks (a map it downloaded has its files in a _downloads folder)
        if let Some(src) = super::doctor::game_path(hl, &p.map.game).iter().map(|d| d.join(&rel)).find(|x| x.is_file()) {
            let dest = gdir.join(&rel);
            if let Some(d) = dest.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::copy(src, dest);
        }
    }
    Ok(())
}

/// How the play part ended.
enum Ended {
    Done,
    Stopped,
    Busy(String),
    Closed,
    NoStart,
}

fn pkill_hidden(hidden: &Path) {
    // under CrossOver the server can outlive the wine launcher we started (only this one:
    // its servers folder's name is in the pattern, so a test run elsewhere leaves the app's be)
    #[cfg(target_os = "macos")]
    {
        let parent = hidden.parent().and_then(|p| p.file_name()).map(|n| regex::escape(&n.to_string_lossy())).unwrap_or_default();
        let mut c = crate::sys::command("pkill");
        c.arg("-f").arg(format!(r"{parent}.\.health-check.hlds\.exe"));
        let _ = crate::sys::run_output(c, Duration::from_secs(5));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = hidden;
}

/// Runs the bots and records them; the recordings and the console's text.
fn play(p: &Plan, gdir: &Path) -> Result<(Ended, Vec<PathBuf>, String), String> {
    let password: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..20).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect()
    };
    configure(gdir, &p.asked, &password)?;
    let simrec = gdir.join("addons/amxmodx/data/simrec");
    let _ = std::fs::remove_dir_all(&simrec);
    let logs = [p.hidden.join("qconsole.log"), gdir.join("qconsole.log"), p.hidden.join("health-server.log")];
    for l in &logs {
        let _ = std::fs::remove_file(l);
    }
    let _ = std::fs::remove_file(p.hidden.join("lstrace.csv"));
    let port = free_port()?;
    let out = std::fs::File::create(&logs[2]).map_err(|e| e.to_string())?;
    update(|r| {
        r.phase = "starting";
        r.detail = format!("Starting the hidden server on {}", p.asked.map);
    });
    let game = p.src.game.clone();
    let mut child = spawn(&p.client, &p.hidden, &server_args(&game, port, &password, &p.asked.map), out)?;
    crate::sys::tie_to_app(&child);
    let rcon = crate::rcon::Rcon::at(password.clone(), "127.0.0.1", port);
    let total = Duration::from_secs(p.asked.minutes as u64 * 60);
    let started = Instant::now();
    let mut playing: Option<Instant> = None;
    let (mut nudges, mut last_nudge, mut last_check) = (0, started, started);
    let (mut retries, mut hang_retries) = (0u32, 0u32);
    let mut started = started;
    // (a retry: the console log gets the mark, the server's own logs start again, a new process)
    let restart = |child: &mut std::process::Child, what: String| -> Result<(), String> {
        pkill_hidden(&p.hidden);
        let _ = child.kill();
        update(|r| r.detail = what);
        for l in &logs[..2] {
            let _ = std::fs::remove_file(l);
        }
        let mut out = std::fs::OpenOptions::new().append(true).open(&logs[2]).map_err(|e| e.to_string())?;
        use std::io::Write;
        let _ = writeln!(out, "\n{RESTART_MARK}");
        *child = spawn(&p.client, &p.hidden, &server_args(&game, port, &password, &p.asked.map), out)?;
        crate::sys::tie_to_app(child);
        Ok(())
    };
    let ended = loop {
        std::thread::sleep(Duration::from_millis(500));
        if STOP.load(Ordering::SeqCst) {
            break Ended::Stopped;
        }
        if matches!(child.try_wait(), Ok(Some(_))) {
            // a server that couldn't reach Steam's library as it came up (seen on the first
            // start of a freshly set up one under CrossOver) usually does on a second try
            if playing.is_none() && retry_reason(&tail(&logs[2], 1 << 16), true, started.elapsed(), retries, hang_retries).is_some() {
                retries += 1;
                // (on Windows the firewall prompt is likely up: time to answer it)
                let wait = if cfg!(windows) { 15 } else { 3 };
                update(|r| {
                    r.detail = if cfg!(windows) {
                        "Steam didn't answer the hidden server: if Windows Firewall asks about it, choose Allow access. Trying again…".into()
                    } else {
                        format!("Starting the hidden server on {} again (Steam didn't answer)", p.asked.map)
                    }
                });
                std::thread::sleep(Duration::from_secs(wait));
                restart(&mut child, format!("Starting the hidden server on {} again", p.asked.map))?;
                started = Instant::now();
                continue;
            }
            break Ended::Closed;
        }
        if last_check.elapsed() >= Duration::from_secs(10) {
            last_check = Instant::now();
            if let Some(why) = user_busy() {
                break Ended::Busy(why);
            }
        }
        // the bot count, set directly a few times (the plugins ramp it, or hold it for people)
        if nudges < 6 && last_nudge.elapsed() >= Duration::from_secs(8) {
            last_nudge = Instant::now();
            if rcon.send(&format!("bot_join_after_player 0;bot_quota {}", p.asked.bots)).is_ok() {
                nudges += 1;
                if playing.is_none() {
                    playing = Some(Instant::now());
                    update(|r| {
                        r.phase = "playing";
                        r.playing_since = Some(now_ms());
                    });
                }
            }
        }
        match playing {
            None if retry_reason(&tail(&logs[2], 1 << 16), false, started.elapsed(), retries, hang_retries).is_some() => {
                if retry_reason(&tail(&logs[2], 1 << 16), false, started.elapsed(), retries, hang_retries) == Some("steam") {
                    retries += 1;
                } else {
                    hang_retries += 1;
                }
                restart(&mut child, format!("The hidden server got stuck starting: starting it again on {}", p.asked.map))?;
                started = Instant::now();
                last_nudge = started;
            }
            None if started.elapsed() > Duration::from_secs(240) => break Ended::NoStart,
            None => {}
            Some(t) if t.elapsed() >= total => break Ended::Done,
            Some(t) => {
                let left = total.saturating_sub(t.elapsed()).as_secs();
                update(|r| r.detail = format!("{} bots playing {} \u{00b7} {} left", p.asked.bots, p.asked.map, duration(left as f32)));
            }
        }
    };
    let _ = rcon.send("quit");
    for _ in 0..25 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    crate::sys::stop_child(child, Duration::from_secs(3));
    pkill_hidden(&p.hidden);
    let mut recs: Vec<PathBuf> = std::fs::read_dir(&simrec)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("csv"))).collect())
        .unwrap_or_default();
    recs.sort();
    let console: String = logs.iter().map(|l| after_restart(&tail(l, MAX_CONSOLE)).to_string()).collect::<Vec<_>>().join("\n");
    Ok((ended, recs, console))
}

fn reports_dir() -> PathBuf {
    crate::sys::app_dir().join("tools").join("health")
}

fn recordings_dir() -> PathBuf {
    crate::localdata::dir("tools/recordings")
}

/// Every saved report (damaged files skipped), newest first.
fn load_reports(dir: &Path) -> Vec<Report> {
    let mut v: Vec<Report> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter(|e| e.metadata().map(|m| m.len() < 4 << 20).unwrap_or(false))
                .filter_map(|e| serde_json::from_slice::<Report>(&std::fs::read(e.path()).ok()?).ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.when.cmp(&a.when).then(b.id.cmp(&a.id)));
    v
}

fn save_report(dir: &Path, r: &Report) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the report ({e})."))?;
    let tmp = dir.join(format!("{}.json.part", r.id));
    std::fs::write(&tmp, serde_json::to_vec_pretty(r).unwrap_or_default()).map_err(|e| format!("Couldn't save the report ({e})."))?;
    std::fs::rename(&tmp, dir.join(format!("{}.json", r.id))).map_err(|e| format!("Couldn't save the report ({e})."))?;
    for old in load_reports(dir).into_iter().skip(KEEP_REPORTS) {
        let _ = std::fs::remove_file(dir.join(format!("{}.json", old.id)));
    }
    Ok(())
}

/// The run's biggest recording (and the bots' trace) into the recordings folder, so Replay
/// lists it: `health-<id>_<map>_<n>.csv` + `trace_health-<id>.csv`. Old health-check
/// recordings beyond the newest few go.
fn keep_recording(id: &str, rec: &Path, trace: &Path) -> Option<String> {
    let dir = recordings_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!("health-{id}_{}", rec.file_name()?.to_string_lossy());
    std::fs::copy(rec, dir.join(&name)).ok()?;
    if std::fs::metadata(trace).map(|m| m.len() > 0 && m.len() <= replay::MAX_FILE).unwrap_or(false) {
        let _ = std::fs::copy(trace, dir.join(format!("trace_health-{id}.csv")));
    }
    let mut ours: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            let id = n.strip_prefix("health-")?.split('_').next()?.to_string();
            Some((id, e.path()))
        })
        .collect();
    ours.sort_by(|a, b| b.0.cmp(&a.0));
    let mut seen: Vec<String> = Vec::new();
    for (rid, path) in ours {
        if !seen.contains(&rid) {
            seen.push(rid.clone());
        }
        if seen.len() > KEEP_RECORDINGS {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(dir.join(format!("trace_health-{rid}.csv")));
        }
    }
    Some(name)
}

fn read_recording(p: &Path) -> Option<Recording> {
    let meta = std::fs::metadata(p).ok()?;
    if meta.len() == 0 || meta.len() > replay::MAX_FILE {
        return None;
    }
    let f = std::fs::File::open(p).ok()?;
    replay::parse_recording(std::io::BufReader::new(f)).ok()
}

/// The whole run, on its own thread. The report's id when there is one.
fn run(p: Plan) -> Result<Option<String>, String> {
    let gdir = p.hidden.join(&p.src.game);
    // a copy of another server goes, so nothing of it carries over
    let mark = p.hidden.join(MARK);
    if p.hidden.exists() && std::fs::read_to_string(&mark).map(|s| s.trim() != p.src.id).unwrap_or(true) && p.hidden.file_name().is_some_and(|n| n == HIDDEN) {
        update(|r| r.detail = "Clearing the old hidden server".into());
        std::fs::remove_dir_all(&p.hidden).map_err(|e| format!("Couldn't clear the old hidden server ({e})."))?;
    }
    let first = !p.hidden.join("hlds.exe").is_file();
    match &p.src.dir {
        Some(dir) => {
            update(|r| r.detail = if first { format!("Making a hidden copy of \u{201c}{}\u{201d} (the first time takes a while)", p.src.name) } else { format!("Bringing the hidden copy of \u{201c}{}\u{201d} up to date", p.src.name) });
            std::fs::create_dir_all(&p.hidden).map_err(|e| format!("Couldn't make the hidden server's folder ({e})."))?;
            let _ = std::fs::write(&mark, &p.src.id);
            sync_copy(dir, &p.hidden, &STOP)?;
        }
        None if first => {
            build_from_game(&p)?;
            let _ = std::fs::write(&mark, &p.src.id);
        }
        None => {}
    }
    update(|r| r.detail = format!("Copying {} and its nav", p.map.name));
    copy_map(&p, &gdir)?;
    let install = crate::steam::Install { hl: p.hidden.clone(), kind: p.client.kind.clone(), bottle: p.client.bottle.clone(), games: vec![p.src.game.clone()], dedicated: true };
    update(|r| r.detail = "Getting the recorder ready".into());
    ensure_recorder(&install, &gdir.join("addons/amxmodx"), &p.src.name)?;
    if STOP.load(Ordering::SeqCst) {
        return Err(STOPPED.into());
    }
    let (ended, recs, console) = play(&p, &gdir)?;
    let mut early = None;
    match &ended {
        Ended::Stopped => return Err(STOPPED.into()),
        Ended::Busy(why) => return Err(why.clone()),
        Ended::Done => {}
        Ended::Closed => early = Some("the server closed by itself before the time was up.".to_string()),
        Ended::NoStart => early = Some("the hidden server didn't come up within 4 minutes.".to_string()),
    }
    update(|r| {
        r.phase = "reading";
        r.detail = "Reading the recording".into();
    });
    let parsed: Vec<(PathBuf, Recording)> = recs.iter().filter_map(|f| Some((f.clone(), read_recording(f)?))).collect();
    if parsed.is_empty() {
        let con = read_console(&console);
        let why = con.fatal.first().map(|l| format!(" The server said: \u{201c}{l}\u{201d}")).unwrap_or_default();
        return Err(match ended {
            Ended::Done => format!("Nothing was recorded. The recorder may not have loaded on the hidden server.{why}"),
            _ => format!("The check ended early: {}{why}", early.unwrap_or_default()),
        });
    }
    let nav = crate::caps::read_file(&p.nav, "nav", crate::goldsrc::nav::MAX_NAV).ok().and_then(|b| Nav::parse(&b).ok()); // [Caps]
    let only: Vec<Recording> = parsed.iter().map(|(_, r)| r.clone()).collect();
    let mut rep = build_report(&only, &console, nav.as_ref(), &p.asked);
    rep.id = p.id.clone();
    rep.when = p.when;
    rep.game = p.src.game.clone();
    rep.server = p.src.name.clone();
    rep.mode_name = p.mode_name.clone();
    rep.map = p.map.name.clone();
    rep.ended_early = early;
    let dir = reports_dir();
    rep.compared = compare(&rep, &load_reports(&dir));
    rep.findings = findings(&rep);
    rep.verdict = verdict(&rep.findings);
    // (the one with the most samples: the same one the report's "Watch in Replay" moments come from)
    let biggest = parsed.iter().max_by_key(|(_, r)| r.cols.t.len()).map(|(f, _)| f.clone());
    rep.recording = biggest.and_then(|f| keep_recording(&p.id, &f, &p.hidden.join("lstrace.csv")));
    save_report(&dir, &rep)?;
    Ok(Some(rep.id))
}

/// After a run, whatever happened: the map, recordings, trace and password go.
/// A hidden temporary server from the game's own files (servers::start_hidden_server, the
/// setup job), waited for here with its progress shown as the run's.
fn build_from_game(p: &Plan) -> Result<(), String> {
    let res = p.res.clone().ok_or(INTERNAL)?;
    update(|r| r.detail = "Setting up a hidden temporary server from your game's files".into());
    let job = crate::servers::start_hidden_server(p.client.clone(), p.hidden.clone(), p.src.game.clone(), res)?;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let v = crate::setup::current();
        if v.id != job {
            return Err("The hidden server's setup was replaced by another setup job. Try again.".into());
        }
        let step = v.steps.iter().find(|s| s.state == "running").map(|s| s.label.clone()).unwrap_or_default();
        let stopping = STOP.load(Ordering::SeqCst);
        update(|r| {
            r.detail = if stopping {
                "Stopping when the server setup step in progress finishes".into()
            } else if step.is_empty() {
                "Setting up a hidden temporary server from your game's files".into()
            } else {
                format!("Hidden server: {step} ({:.0}%)", v.progress * 100.0)
            }
        });
        match v.state.as_str() {
            "running" => continue,
            "done" if stopping => return Err(STOPPED.into()),
            "done" => return Ok(()),
            _ => return Err(format!("The hidden server couldn't be set up: {}", v.error.unwrap_or_else(|| "setup stopped.".into()))),
        }
    }
}

fn tidy(hidden: &Path, game: &str) {
    let gdir = hidden.join(game);
    let _ = std::fs::remove_dir_all(gdir.join("maps"));
    let _ = std::fs::remove_dir_all(gdir.join("addons/amxmodx/data/simrec"));
    let _ = std::fs::remove_file(hidden.join("lstrace.csv"));
    let _ = std::fs::remove_file(gdir.join("addons/amxmodx/data/gamemodes/panel_rcon.txt"));
}

#[derive(Deserialize)]
struct StartReq {
    map: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    minutes: Option<u32>,
    #[serde(default)]
    bots: Option<u32>,
    #[serde(default)]
    server: Option<String>,
}

/// Checks a start and makes its plan.
fn plan(st: &Shared, req: StartReq) -> Result<Plan, String> {
    if !crate::maps::safe_arg(&req.map) {
        return Err(BAD_NAME.into());
    }
    let game = st.local_game().ok_or(NO_GAME)?;
    let client = crate::servers::client(st).ok_or(NO_GAME)?;
    if client.kind == "crossover" && !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The hidden server runs through it, like the game.".into());
    }
    if let Some(why) = setup_busy().or_else(user_busy) {
        return Err(why);
    }
    let map = super::mapinfo::locate(&game, &req.map).ok_or(super::mapinfo::NO_MAP)?;
    let (nav, lsv) = nav_files(&game.hl, &map);
    let nav = nav.ok_or_else(|| format!("Bots don't know {} yet: it has no nav. Teach them the map first (Maps, then Teach bots), then check it.", map.name))?;
    let gf = game_folder(&game);
    let list = sources(gf);
    let entry = match req.server.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => Some(list.into_iter().find(|e| e.id == id).ok_or("That server isn't on this computer any more.")?),
        None => list.into_iter().next(),
    };
    let (src, res) = match entry {
        Some(e) => (Source { id: e.id.clone(), name: e.name.clone(), game: e.game.clone(), dir: Some(e.dir.clone()) }, None),
        None => (
            Source { id: FROM_GAME.into(), name: "your game".into(), game: gf.to_string(), dir: None },
            Some(crate::setup_api::resources_for(st).map_err(|_| "Loner-Strike's own files couldn't be found, so a hidden server can't be set up. Reinstall the app.".to_string())?),
        ),
    };
    let modes = modes_from(&src, &game.hl);
    let mode = if req.mode.is_empty() { "classic".to_string() } else { req.mode.clone() };
    let mode_name = modes.iter().find(|(id, _)| *id == mode).map(|(_, n)| n.clone()).ok_or("Pick one of the server's game modes.")?;
    let root = crate::servers::root_for(&client)?;
    let when = now_ms() / 1000;
    Ok(Plan {
        asked: Asked {
            map: map.name.clone(),
            mode,
            bots: req.bots.unwrap_or(12).clamp(BOTS.0, BOTS.1),
            minutes: req.minutes.unwrap_or(5).clamp(MINUTES.0, MINUTES.1),
        },
        mode_name,
        client,
        src,
        res,
        map,
        nav,
        lsv,
        hidden: root.join(HIDDEN),
        id: format!("{}", now_ms()),
        when,
    })
}

fn start(st: &Shared, req: StartReq) -> Result<Value, String> {
    let p = plan(st, req)?;
    {
        let mut cur = RUN.lock().unwrap_or_else(|e| e.into_inner());
        if cur.as_ref().is_some_and(Run::active) {
            return Err("A simulation is already running. Wait for it, or stop it first.".into());
        }
        *cur = Some(Run {
            id: p.id.clone(),
            map: p.asked.map.clone(),
            mode: p.asked.mode.clone(),
            mode_name: p.mode_name.clone(),
            minutes: p.asked.minutes,
            bots: p.asked.bots,
            server: p.src.name.clone(),
            phase: "preparing",
            detail: "Getting ready".into(),
            started: now_ms(),
            ..Default::default()
        });
    }
    STOP.store(false, Ordering::SeqCst);
    let id = p.id.clone();
    std::thread::spawn(move || {
        let (hidden, game, temporary) = (p.hidden.clone(), p.src.game.clone(), p.src.dir.is_none());
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(p))).unwrap_or_else(|_| Err(INTERNAL.into()));
        pkill_hidden(&hidden);
        if temporary {
            // made from the game for this run: it goes (Valve's download in .base stays)
            update(|run| run.detail = "Removing the hidden server".into());
            // (LS_KEEP_SIM_SERVER: kept, to look into a failed run)
            if hidden.file_name().is_some_and(|n| n == HIDDEN) && std::env::var_os("LS_KEEP_SIM_SERVER").is_none() {
                let _ = std::fs::remove_dir_all(&hidden);
            }
        } else {
            tidy(&hidden, &game);
        }
        update(|run| match r {
            Ok(rep) => {
                run.phase = "done";
                run.detail = "Done".into();
                run.report = rep;
            }
            Err(e) if e == STOPPED => {
                run.phase = "stopped";
                run.detail = "Stopped".into();
            }
            Err(e) => {
                run.phase = if e.starts_with("Close the game") || e.starts_with("Stop your server") { "stopped" } else { "failed" };
                run.detail = String::new();
                run.error = Some(e);
            }
        });
    });
    Ok(json!({"ok": true, "id": id}))
}

/// The run (if any) with its time so far and an estimate of what's left.
fn progress() -> Value {
    let Some(r) = RUN.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return json!({"run": null}) };
    let now = now_ms();
    let elapsed = now.saturating_sub(r.started) / 1000;
    let total = r.minutes as u64 * 60;
    let played = r.playing_since.map(|t| now.saturating_sub(t) / 1000).unwrap_or(0);
    let left = match r.phase {
        "preparing" => total + 60,
        "starting" => total + 30,
        "playing" => total.saturating_sub(played) + 10,
        "reading" => 5,
        _ => 0,
    };
    let fraction = match r.phase {
        "preparing" => 0.02,
        "starting" => 0.05,
        "playing" => 0.05 + 0.9 * (played as f64 / total.max(1) as f64).min(1.0),
        "reading" => 0.97,
        _ => 1.0,
    };
    json!({"run": r, "active": r.active(), "elapsed": elapsed, "left": left, "fraction": fraction})
}

fn options(st: &Shared, map: &str) -> Result<Value, String> {
    if !crate::maps::safe_arg(map) {
        return Err(BAD_NAME.into());
    }
    let game = st.local_game().ok_or(NO_GAME)?;
    let m = super::mapinfo::locate(&game, map).ok_or(super::mapinfo::NO_MAP)?;
    let (nav, lsv) = nav_files(&game.hl, &m);
    let gf = game_folder(&game);
    let list = sources(gf);
    let first = list.first();
    let src = match first {
        Some(e) => Source { id: e.id.clone(), name: e.name.clone(), game: e.game.clone(), dir: Some(e.dir.clone()) },
        None => Source { id: FROM_GAME.into(), name: "your game".into(), game: gf.to_string(), dir: None },
    };
    let gdir = src.dir.clone().unwrap_or_else(|| game.hl.clone()).join(&src.game);
    let modes: Vec<Value> = modes_from(&src, &game.hl).into_iter().map(|(id, name)| json!({"id": id, "name": name})).collect();
    let presets = crate::sys::read_latin1(&gdir.join("addons/amxmodx/configs/gamemodes/mappresets.ini")).unwrap_or_default();
    let mode = preset_mode(&presets, &m.name).filter(|x| modes.iter().any(|v| v["id"] == json!(x))).unwrap_or_else(|| "classic".into());
    let why = nav.is_none().then(|| format!("Bots don't know {} yet: it has no nav. Teach them the map first (Maps, then Teach bots), then check it.", m.name));
    Ok(json!({
        "map": m.name, "game": gf, "hasNav": nav.is_some(), "hasKnowledge": lsv.is_some(),
        "servers": list.iter().map(|e| json!({"id": e.id, "name": e.name})).collect::<Vec<_>>(),
        "server": first.map(|e| e.id.clone()), "modes": modes, "mode": mode,
        "fromGame": first.is_none().then_some(FROM_GAME_NOTE),
        "bots": 12, "minutes": 5, "limits": {"minutes": [MINUTES.0, MINUTES.1], "bots": [BOTS.0, BOTS.1]},
        "why": why, "busy": setup_busy().or_else(user_busy), "running": running_now(),
    }))
}

fn summary(r: &Report) -> Value {
    json!({
        "id": r.id, "map": r.map, "mode": r.mode, "modeName": r.mode_name, "when": r.when, "minutes": r.minutes,
        "bots": r.bots_wanted, "verdict": r.verdict, "headline": r.findings.first().map(|f| f.text.clone()).unwrap_or_default(),
        "problems": r.findings.iter().filter(|f| f.level != "good").count(),
    })
}

fn valid_id(id: &str) -> bool {
    (6..=20).contains(&id.len()) && id.bytes().all(|c| c.is_ascii_digit())
}

// ---- routes ---------------------------------------------------------------------------------

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    let r = tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(INTERNAL.into()));
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

fn forbidden() -> Response {
    (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

#[derive(Deserialize)]
struct MapQuery {
    map: String,
}

async fn api_options(State(st): State<Shared>, Query(q): Query<MapQuery>) -> Response {
    blocking(move || options(&st, &q.map)).await
}

async fn api_start(State(st): State<Shared>, h: HeaderMap, Json(req): Json<StartReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || start(&st, req)).await
}

async fn api_progress() -> Response {
    Json(progress()).into_response()
}

async fn api_cancel(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    if !running_now() {
        return Json(json!({"error": "There's no simulation running."})).into_response();
    }
    STOP.store(true, Ordering::SeqCst);
    update(|r| r.detail = "Stopping".into());
    Json(json!({"ok": true})).into_response()
}

async fn api_reports() -> Response {
    blocking(|| Ok(json!({"reports": load_reports(&reports_dir()).iter().map(summary).collect::<Vec<_>>()}))).await
}

async fn api_report(UrlPath(id): UrlPath<String>) -> Response {
    if !valid_id(&id) {
        return (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": "That isn't a report."}))).into_response();
    }
    blocking(move || {
        let p = reports_dir().join(format!("{id}.json"));
        let b = std::fs::read(&p).map_err(|_| "That report isn't here any more.".to_string())?;
        let r: Report = serde_json::from_slice(&b).map_err(|_| "That report couldn't be read.".to_string())?;
        Ok(json!({"report": r}))
    })
    .await
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/health/options", get(api_options))
        .route("/api/tools/health/start", post(api_start))
        .route("/api/tools/health/progress", get(api_progress))
        .route("/api/tools/health/cancel", post(api_cancel))
        .route("/api/tools/health/reports", get(api_reports))
        .route("/api/tools/health/report/:id", get(api_report))
}

#[cfg(test)]
mod tests {
    use std::time::Duration as D;

    // the user's Windows test: the firewall blocked the first start, the retry worked; the report
    // must not call the first attempt a crash (only what came after the restart is read)
    #[test]
    fn a_start_retried_isnt_a_crash() {
        let log = format!("FATAL ERROR (shutting down): Unable to initialize Steam\n{RESTART_MARK}\nL 11/16/2026 - 10:00:00: Started map \"de_dust2\"\n");
        assert!(read_console(after_restart(&log)).fatal.is_empty());
        assert_eq!(read_console(&log).fatal.len(), 1, "without the mark it would have been reported");
        assert_eq!(after_restart("no restart here"), "no restart here");
    }

    #[test]
    fn steam_not_answering_is_the_firewall_on_windows_not_a_crash() {
        let l = "FATAL ERROR (shutting down): Unable to initialize Steam";
        let w = engine_finding(l, true);
        assert!(w.contains("Allow access") && w.contains("Windows Firewall") && !w.contains("crashed"), "{w}");
        assert!(!engine_finding(l, false).contains("crashed"));
        assert!(engine_finding("Host_Error: PF_precache_model_I: x", true).starts_with("The server crashed or hit an engine error"));
    }

    #[test]
    fn engine_findings_are_marked_for_the_dismiss_button() {
        let r = Report { fatal: vec!["Host_Error: x".into()], ..Default::default() };
        let f = findings(&r);
        let e = f.iter().find(|f| f.text.contains("engine error")).expect("the engine finding");
        assert_eq!(e.kind.as_deref(), Some("engine"));
        assert!(f.iter().filter(|f| !f.text.contains("engine error")).all(|f| f.kind.is_none()));
    }

    // the other hidden servers (Studio Test run, the nav learner, the map knowledge builder) share
    // the rules and the words
    #[test]
    fn the_shared_rules_for_every_hidden_server() {
        let steam = "FATAL ERROR (shutting down): Unable to initialize Steam";
        // under CrossOver the server sits with the error instead of closing: still a Steam retry
        assert_eq!(retry_reason(steam, false, D::from_secs(5), 0, 0), Some("steam"));
        // the knowledge builder works for minutes without answering: no silent-hang retry, the SDL3 line still
        assert_eq!(retry_reason_with("", false, D::from_secs(600), 0, 0, None), None);
        let sdl = "Assertion Failed: Failed to load \"SDL3.dll\"";
        assert_eq!(retry_reason_with(sdl, false, D::from_secs(31), 0, 0, None), Some("hang"));
        // the error a tool shows when it gives up
        let w = closed_why(&format!("x\n{steam}\n"), "the hidden server closed while loading it.");
        assert!(!w.contains("crashed") && w.contains("Steam"), "{w}");
        assert_eq!(closed_why("Host_Error: y", "the hidden server closed while loading it."), "the hidden server closed while loading it.");
        // an old attempt's Steam line after a restart: the tool's own words
        assert_eq!(closed_why(&format!("{steam}\n{RESTART_MARK}\nnothing"), "closed."), "closed.");
        assert!(retry_detail("steam").contains("trying again") || retry_detail("steam").contains("Trying again"));
        assert!(retry_detail("hang").contains("stuck"));
    }

    #[test]
    fn retries_steam_twice_and_a_hang_once() {
        let steam = "FATAL ERROR (shutting down): Unable to initialize Steam";
        assert_eq!(retry_reason(steam, true, D::from_secs(5), 0, 0), Some("steam"));
        assert_eq!(retry_reason(steam, true, D::from_secs(5), 2, 0), None, "two retries at most");
        assert_eq!(retry_reason("closed for another reason", true, D::from_secs(5), 0, 0), None);
        // after a restart only the new attempt counts: the old Steam line doesn't retry again
        assert_eq!(retry_reason(&format!("{steam}\n{RESTART_MARK}\nsomething else"), true, D::from_secs(5), 1, 0), None);
        // the SDL3 hang (Valve's HLDS 4599 under CrossOver): retried once, after 30 s with no answer
        let sdl = "src\\public\\tier1\\interface.h (273) : Assertion Failed: Failed to load \"SDL3.dll\"";
        assert_eq!(retry_reason(sdl, false, D::from_secs(10), 0, 0), None, "too soon");
        assert_eq!(retry_reason(sdl, false, D::from_secs(31), 0, 0), Some("hang"));
        assert_eq!(retry_reason(sdl, false, D::from_secs(31), 0, 1), None, "once");
        // nothing at all in 4 minutes: once too
        assert_eq!(retry_reason("", false, D::from_secs(241), 0, 0), Some("hang"));
        assert_eq!(retry_reason("", false, D::from_secs(100), 0, 0), None);
        assert_eq!(retry_reason("", false, D::from_secs(241), 0, 1), None);
    }

    use super::*;

    const FALLS: &str = include_str!("health/testdata/rats2_falls.csv");

    fn rec(text: &str) -> Recording {
        replay::parse_recording(std::io::Cursor::new(text.as_bytes())).unwrap()
    }

    fn asked(mode: &str) -> Asked {
        Asked { map: "cs_rats2".into(), mode: mode.into(), bots: 12, minutes: 1 }
    }

    /// A nav of flat areas: (place, lo, hi) with the floor at lo.z.
    fn nav(areas: &[(&str, [f32; 3], [f32; 3])]) -> Nav {
        let mut places: Vec<String> = Vec::new();
        let areas = areas
            .iter()
            .enumerate()
            .map(|(i, (place, lo, hi))| {
                let place = if place.is_empty() {
                    0
                } else {
                    if !places.iter().any(|p| p == place) {
                        places.push(place.to_string());
                    }
                    places.iter().position(|p| p == place).unwrap() as u16 + 1
                };
                crate::goldsrc::nav::Area {
                    id: i as u32 + 1,
                    attr: 0,
                    lo: *lo,
                    hi: [hi[0], hi[1], lo[2]],
                    ne_z: lo[2],
                    sw_z: lo[2],
                    conns: Default::default(),
                    hiding: vec![],
                    approach: vec![],
                    encounters: vec![],
                    place,
                }
            })
            .collect();
        Nav { version: 5, bsp_size: 0, places, areas }
    }

    /// Where the snippet's three bots stood before they fell (rats2_v5, W1-1, 340-400 s):
    /// bot 4 from (-917, 318, 484), bot 10 from (-750, 374, -107), bot 8 from (-300, -659, 228).
    fn fall_nav() -> Nav {
        nav(&[
            ("Shelf", [-1000.0, 250.0, 440.0], [-850.0, 400.0, 0.0]),
            ("Shelf", [-800.0, 300.0, -150.0], [-700.0, 450.0, 0.0]),
            ("Pipe", [-350.0, -700.0, 190.0], [-250.0, -600.0, 0.0]),
        ])
    }

    #[test]
    fn falls_found_and_grouped_by_place() {
        let r = rec(FALLS);
        let rep = build_report(&[r], "", Some(&fall_nav()), &asked("classic"));
        assert_eq!(rep.deaths, 13);
        assert_eq!(rep.world_deaths, 3);
        assert_eq!(rep.falls, 3);
        assert_eq!(rep.bots_seen, 3);
        // two falls from the shelf (two areas, one place), one from the pipe
        assert_eq!(rep.fall_spots.len(), 2);
        assert_eq!(rep.fall_spots[0].place, "Shelf");
        assert_eq!(rep.fall_spots[0].count, 2);
        assert_eq!(rep.fall_spots[0].drop, 1011.0);
        assert_eq!(rep.fall_spots[0].seconds, 0.0);
        assert_eq!(rep.fall_spots[1].place, "Pipe");
        assert_eq!(rep.fall_spots[1].at, [-300.0, -659.0, 228.0]);
        assert!(rep.hurt_spots.is_empty());
        // the bots' walk covers some of the three areas
        assert_eq!(rep.nav_areas, Some(3));
        assert!(rep.coverage_pct.unwrap() > 0.0);
        // in play only: 350.4 (round end) to 361.4 (start) doesn't count
        let samples = FALLS.lines().filter(|l| l.starts_with("S,")).count() as f32;
        assert!(rep.bot_minutes < samples * SAMPLE / 60.0);
        assert!(rep.kills_per_bot_min > 0.0 && rep.still_pct > 0.0);
    }

    #[test]
    fn without_a_nav_spots_group_by_grid_and_name_their_point() {
        let rep = build_report(&[rec(FALLS)], "", None, &asked("classic"));
        assert_eq!(rep.fall_spots.len(), 3);
        assert!(rep.fall_spots.iter().all(|s| s.place.is_empty() && s.count == 1));
        assert_eq!(rep.coverage_pct, None);
        assert_eq!(near(&rep.fall_spots[0]), "the spot at -917 318 484");
    }

    /// Two rounds (the T win, then the CT win), a T killing a CT at "Mid" from "Long", and
    /// a named place nobody walked into.
    const DUEL: &str = "P,1,Tee,1\nP,2,Cee,1\nR,10.00,start\n\
S,10.00,1,1,100,100,0,0,0,0,0.0,0.0,0,1,28,30,0,100,0,99999,99999\n\
S,10.10,1,1,110,100,0,100,0,0,0.0,0.0,0,1,28,30,0,100,1,500,99999\n\
S,10.00,2,2,600,100,0,0,0,0,0.0,0.0,0,1,28,30,0,100,0,99999,99999\n\
S,10.10,2,2,600,100,0,0,0,0,0.0,0.0,0,1,28,30,0,100,1,500,99999\n\
K,10.15,1,2,1,ak47\nR,40.00,end,1\nR,50.00,start\n\
S,50.00,1,1,100,100,0,0,0,0,0.0,0.0,0,1,28,30,0,100,0,99999,99999\n\
R,95.50,end,2\nR,100.00,start\nR,104.00,end\n";

    #[test]
    fn rounds_sides_fights_and_places_nobody_went() {
        let n = nav(&[("Long", [0.0, 0.0, 0.0], [300.0, 300.0, 0.0]), ("Mid", [500.0, 0.0, 0.0], [800.0, 300.0, 0.0]), ("Pit", [2000.0, 2000.0, 0.0], [2300.0, 2300.0, 0.0])]);
        let rep = build_report(&[rec(DUEL)], "", Some(&n), &asked("classic"));
        assert_eq!(rep.rounds, vec![RoundResult { seconds: 30.0, winner: 1 }, RoundResult { seconds: 45.5, winner: 2 }, RoundResult { seconds: 4.0, winner: 0 }]);
        assert_eq!((rep.t_kills, rep.ct_kills), (1, 0));
        assert_eq!(rep.fight_spots.len(), 1);
        assert_eq!(rep.fight_spots[0].place, "Mid");
        assert_eq!((rep.fight_spots[0].deaths, rep.fight_spots[0].ct_deaths, rep.fight_spots[0].t_deaths), (1, 1, 0));
        let g = rep.heat.as_ref().unwrap();
        assert_eq!(g.cell, 128.0);
        assert_eq!(g.deaths.iter().map(|&v| v as u32).sum::<u32>(), 1);
        assert_eq!(g.kills.iter().map(|&v| v as u32).sum::<u32>(), 1);
        // the victim died where it stood (600, 100), the killer was at (110, 100)
        let at = |x: f32, y: f32| (((y - g.y0) / g.cell).floor() as usize) * g.w + ((x - g.x0) / g.cell).floor() as usize;
        assert_eq!(g.deaths[at(600.0, 100.0)], 1);
        assert_eq!(g.kills[at(110.0, 100.0)], 1);
        assert!(g.time.iter().sum::<f32>() > 0.0);
        assert_eq!(rep.unvisited_places, vec!["Pit".to_string()]);
        assert_eq!(rep.unvisited, vec![[2000.0, 2000.0, 2300.0, 2300.0, 0.0]]);
        // a report saved before these existed still reads
        let old: Report = serde_json::from_str(r#"{"id":"x","map":"de_dust2","verdict":"good"}"#).unwrap();
        assert!(old.rounds.is_empty() && old.heat.is_none());
    }

    #[test]
    fn one_sided_maps_and_places_nobody_went_are_said() {
        let mut r = Report { mode: "classic".into(), bots_wanted: 12, minutes: 5, bots_seen: 12, bot_minutes: 50.0, seconds: 300.0, knowledge: "loaded".into(), ..Default::default() };
        r.rounds = (0..10).map(|i| RoundResult { seconds: 60.0, winner: if i < 8 { 2 } else { 1 } }).collect();
        r.unvisited_places = vec!["Pit".into()];
        let f = findings(&r);
        assert!(f.iter().any(|f| f.text.starts_with("The CTs won 8 of 10 rounds")), "{f:?}");
        assert!(f.iter().any(|f| f.text.starts_with("Bots never went to Pit. It may")), "{f:?}");
        r.rounds.iter_mut().take(3).for_each(|x| x.winner = 1);
        assert!(!findings(&r).iter().any(|f| f.text.contains("rounds.")));
    }

    /// A report from a sim recording, for trying the report page with real data (the
    /// dev harness reads reports from its temp app folder):
    ///   LS_REPORT_REC=<gm_simrec .csv> LS_REPORT_NAV=<.nav> LS_REPORT_MAP=de_dust2
    ///   LS_REPORT_OUT=$TMPDIR/ls-tools-dev/tools/health [LS_REPORT_RECDIR=$TMPDIR/ls-tools-dev/tools/recordings]
    ///   cargo test report_from_recording -- --ignored
    #[test]
    #[ignore]
    fn report_from_recording() {
        let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let rec = read_recording(Path::new(&var("LS_REPORT_REC"))).expect("a recording");
        let nav = std::env::var("LS_REPORT_NAV").ok().and_then(|p| Nav::parse(&std::fs::read(p).ok()?).ok());
        let mut rep = build_report(&[rec], "", nav.as_ref(), &Asked { map: var("LS_REPORT_MAP"), mode: "classic".into(), bots: 12, minutes: 5 });
        rep.id = format!("{}", now_ms());
        rep.when = now_ms() / 1000;
        rep.mode_name = "Classic".into();
        rep.knowledge = "loaded".into();
        rep.findings = findings(&rep);
        rep.verdict = verdict(&rep.findings);
        // LS_REPORT_RECDIR: also put the recording there (the app's recordings folder), for Replay
        if let Ok(dir) = std::env::var("LS_REPORT_RECDIR") {
            let name = format!("health-{}_{}.csv", rep.id, rep.map);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::copy(var("LS_REPORT_REC"), Path::new(&dir).join(&name)).unwrap();
            rep.recording = Some(name);
        }
        save_report(Path::new(&var("LS_REPORT_OUT")), &rep).unwrap();
        println!("{} rounds, T {} CT {}, {} fight spots, {} unvisited areas, places nobody went: {:?}", rep.rounds.len(),
            rep.rounds.iter().filter(|r| r.winner == 1).count(), rep.rounds.iter().filter(|r| r.winner == 2).count(),
            rep.fight_spots.len(), rep.unvisited.len(), rep.unvisited_places);
        for f in &rep.findings {
            println!("{}: {}", f.level, f.text);
        }
    }

    #[test]
    fn plain_findings_for_falls() {
        // the snippet's falls three times over from one place, as a long run would show
        let mut r = rec(FALLS);
        let falls: Vec<_> = r.kills.iter().filter(|k| k.fall.is_some()).cloned().collect();
        for _ in 0..2 {
            for k in &falls {
                let mut k = k.clone();
                k.fall.as_mut().unwrap().from = [-917.0, 318.0, 484.0];
                r.kills.push(k);
            }
        }
        let rep = build_report(&[r], "Loner-Strike: map knowledge for cs_rats2 loaded (586 areas).", Some(&fall_nav()), &asked("classic"));
        assert_eq!(rep.knowledge, "loaded");
        let texts: Vec<&str> = rep.findings.iter().map(|f| f.text.as_str()).collect();
        assert!(texts.contains(&"Bots fall a lot near Shelf \u{2014} 8 times in 1 minute."), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("9 of 19 deaths were falls")), "{texts:?}");
        let f = rep.findings.iter().find(|f| f.text.starts_with("Bots fall")).unwrap();
        assert_eq!(f.level, "warn");
        assert_eq!(f.spot.as_ref().unwrap().place, "Shelf");
        // (and only 3 of 12 bots were in the snippet)
        assert_eq!(rep.findings[0].text, "Only 3 of 12 bots joined the game.");
        assert_eq!(rep.verdict, "bad");
        // surf: falling is the game
        let mut s = rep.clone();
        s.mode = "surf".into();
        assert!(!findings(&s).iter().any(|f| f.text.contains("fall")));
    }

    #[test]
    fn stuck_spots_and_good_runs() {
        let mut r = Report { mode: "classic".into(), bots_wanted: 12, bots_seen: 12, minutes: 5, bot_minutes: 55.0, kills_per_bot_min: 1.1, coverage_pct: Some(44.0), knowledge: "loaded".into(), seconds: 300.0, ..Default::default() };
        let f = findings(&r);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, "good");
        assert_eq!(f[0].text, "No problems found. Bots played (1.10 kills a bot-minute), used 44% of the map and rarely got stuck or fell.");
        r.stuck_spots = vec![Spot { at: [-832.0, -320.0, -576.0], place: "Sink".into(), count: 6, seconds: 291.0, drop: 0.0, ..Default::default() }, Spot { seconds: 20.0, count: 2, ..Default::default() }];
        let f = findings(&r);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].text, "Bots get stuck near Sink: 4 min 51 s in all, 6 times.");
        assert_eq!(verdict(&f), "warn");
        // hide and seek: standing about is the game
        r.mode = "hns".into();
        assert_eq!(findings(&r)[0].level, "good");
    }

    #[test]
    fn coverage_judged_by_run_length() {
        // QA4 T1-5: de_dust2, 6 bots, 2 min: 6.8 bot-minutes, 17% used, isn't worth a look
        let short = Report { map: "de_dust2".into(), mode: "classic".into(), bots_wanted: 6, bots_seen: 6, minutes: 2, bot_minutes: 6.8, kills_per_bot_min: 0.9, coverage_pct: Some(17.0), knowledge: "loaded".into(), seconds: 113.0, unvisited_places: vec!["Catwalk".into()], ..Default::default() };
        let f = findings(&short);
        assert_eq!(f[0].level, "good", "{f:?}");
        assert_eq!(verdict(&f), "good");
        // 5 min, 6 bots (~28 bot-minutes): the floor is lower than 20%
        let mid = Report { minutes: 5, bot_minutes: 28.0, seconds: 300.0, ..short.clone() };
        assert!(findings(&mid).iter().all(|f| !f.text.contains("only used")));
        let low = Report { coverage_pct: Some(12.0), ..mid.clone() };
        assert!(findings(&low).iter().any(|f| f.text.contains("Bots only used 12%")));
        // a long, full run: 20%
        let long = Report { bots_wanted: 12, bots_seen: 12, minutes: 10, bot_minutes: 110.0, seconds: 600.0, coverage_pct: Some(18.0), ..short };
        assert!(findings(&long).iter().any(|f| f.text.contains("Bots only used 18%")));
        assert!((coverage_floor(40.0) - 20.0).abs() < 0.01 && coverage_floor(10.0) < 11.0);
    }

    #[test]
    fn stuck_spans_from_a_recording() {
        // one bot pushing into a wall for 12 s in play, then walking; another holding an
        // angle the whole time (no movement keys), which isn't stuck
        let mut text = String::from("P,1,Bot,1\nP,2,Holder,1\nR,0.0,start\n");
        for i in 0..200 {
            let t = i as f32 * 0.1;
            let (x, vx) = if t < 12.0 { (100.0, 0.0) } else { (100.0 + (t - 12.0) * 200.0, 200.0) };
            text.push_str(&format!("S,{t:.1},1,2,{x},50,36,{vx},0,0,0,0,0,1,16,12,8,100,0,99999,99999\n"));
            text.push_str(&format!("S,{t:.1},2,2,150,150,36,0,0,0,0,0,1,1,16,12,0,100,0,99999,99999\n"));
        }
        let n = nav(&[("Tunnel", [0.0, 0.0, 0.0], [200.0, 200.0, 0.0]), ("", [200.0, 0.0, 0.0], [2000.0, 200.0, 0.0])]);
        let rep = build_report(&[rec(&text)], "", Some(&n), &Asked { map: "x".into(), mode: "classic".into(), bots: 1, minutes: 1 });
        assert_eq!(rep.stuck_spots.len(), 1);
        assert_eq!(rep.stuck_spots[0].place, "Tunnel");
        // (12 s, and the moment it took to walk out of the 160-unit circle)
        assert!(rep.stuck_spots[0].seconds > 11.8 && rep.stuck_spots[0].seconds < 12.9, "{:?}", rep.stuck_spots);
        assert_eq!(rep.stuck_spots[0].count, 1);
        assert!((rep.stuck_pct - 31.5).abs() < 2.0, "{}", rep.stuck_pct);
        assert_eq!(rep.coverage_pct, Some(100.0));
        assert_eq!(rep.bot_rounds, 0);
        // the spot remembers who and when, for "Watch in Replay"
        assert_eq!((rep.stuck_spots[0].bot, rep.stuck_spots[0].t), (Some(1), Some(0.0)));
    }

    #[test]
    fn spawn_sitters() {
        // two rounds of 40 s: in the first the bot stays put, in the second it walks off
        let mut text = String::from("P,1,Bot,1\nR,0.0,start\nR,40.0,end\nR,41.0,start\n");
        for i in 0..810 {
            let t = i as f32 * 0.1;
            let x = if t < 41.0 { 10.0 } else { (t - 41.0) * 20.0 };
            text.push_str(&format!("S,{t:.1},1,2,{x},0,36,0,0,0,0,0,0,1,16,12,0,100,1,500,99999\n"));
        }
        let rep = build_report(&[rec(&text)], "", None, &Asked { map: "x".into(), mode: "classic".into(), bots: 1, minutes: 1 });
        assert_eq!(rep.bot_rounds, 2);
        assert_eq!(rep.spawn_sitters, 1);
    }

    #[test]
    fn console_errors_crashes_and_knowledge() {
        // lines from the sweeps' consoles (results/*/console_*.log)
        let text = "Protocol version 48\n\
L 11/12/2026 - 14:57:21: [miscstats.amxx] Could not locate <sound/fvox/flatline.wav> file\n\
L 11/12/2026 - 01:33:38: [AMXX] Run time error 4 (plugin \"basebuilder.amxx\") - debug not enabled!\n\
L 11/12/2026 - 01:33:39: [AMXX] Run time error 4 (plugin \"basebuilder.amxx\") - debug not enabled!\n\
L 11/12/2026 - 01:33:40: [AMXX] Plugin \"gm_vip.amxx\" failed to load: Module/Library \"hamsandwich\" required\n\
Loner-Strike: map knowledge for de_dust2: 737 areas, building\n\
Host_Error: PF_precache_model_I: Model 'models/barrel.mdl' failed to precache\n";
        let c = read_console(text);
        assert_eq!(c.plugin_errors, 3);
        assert_eq!(c.by_plugin[0].plugin, "basebuilder.amxx");
        assert_eq!(c.by_plugin[0].count, 2);
        assert_eq!(c.by_plugin[0].first, "[AMXX] Run time error 4 (plugin \"basebuilder.amxx\") - debug not enabled!");
        assert_eq!(c.by_plugin[1].plugin, "gm_vip.amxx");
        assert_eq!(c.fatal, vec!["Host_Error: PF_precache_model_I: Model 'models/barrel.mdl' failed to precache".to_string()]);
        assert_eq!(c.knowledge, "rebuilt");
        assert_eq!(read_console("Loner-Strike: map knowledge for cs_rats2 loaded (586 areas).").knowledge, "loaded");
        assert_eq!(read_console("").knowledge, "none");
        let rep = build_report(&[rec(FALLS)], text, None, &asked("classic"));
        assert_eq!(rep.findings[0].text, "The server crashed or hit an engine error: \u{201c}Host_Error: PF_precache_model_I: Model 'models/barrel.mdl' failed to precache\u{201d}");
        assert!(rep.findings[1].text.starts_with("basebuilder.amxx had 2 errors in the console."));
        assert!(rep.findings.iter().any(|f| f.text.starts_with("The bots' map knowledge didn't fit")));
    }

    #[test]
    fn compared_with_other_maps_of_the_mode() {
        let other = |map: &str, stuck: f32, when: u64| Report { map: map.into(), mode: "classic".into(), bot_minutes: 50.0, stuck_pct: stuck, still_pct: 30.0, kills_per_bot_min: 1.0, coverage_pct: Some(45.0), when, ..Default::default() };
        let others = vec![other("de_dust2", 3.0, 1), other("de_inferno", 4.0, 1), other("de_nuke", 3.5, 1), other("cs_office", 4.5, 1), other("de_aztec", 99.0, 1), other("de_aztec", 3.8, 2)];
        let mut r = other("cs_rats2", 12.0, 3);
        let c = compare(&r, &others);
        assert_eq!(c, vec!["Bots get stuck far more than on other classic maps (12.00 here, 3.80 usually).".to_string()]);
        r.stuck_pct = 6.0;
        assert!(compare(&r, &others).is_empty(), "within the margin");
        // too few of the mode: the fighting modes together; still too few: nothing
        assert!(compare(&r, &others[..3]).is_empty());
        // its own earlier reports don't count
        let mine: Vec<Report> = (0..5).map(|i| other("cs_rats2", 1.0, i)).collect();
        assert!(compare(&other("cs_rats2", 50.0, 9), &mine).is_empty());
    }

    #[test]
    fn map_presets() {
        let ini = "; comment\nsurf*            surf\nka_*             knife\nde_dust2 classic\n";
        assert_eq!(preset_mode(ini, "surf_ski_2").as_deref(), Some("surf"));
        assert_eq!(preset_mode(ini, "KA_soccer").as_deref(), Some("knife"));
        assert_eq!(preset_mode(ini, "de_dust2").as_deref(), Some("classic"));
        assert_eq!(preset_mode(ini, "de_dust2x"), None);
    }

    #[test]
    fn server_settings() {
        assert_eq!(set_cvar("// saved\nbot_quota \"4\"\ngm_botstyle \"1\"\n", "bot_quota", "12"), "// saved\nbot_quota \"12\"\ngm_botstyle \"1\"\n");
        assert_eq!(set_cvar("// saved\n", "bot_quota", "8"), "// saved\nbot_quota \"8\"\n");
        let ini = "; plugins\ngamemodes.amxx\n";
        let with = with_recorder(ini);
        assert!(with.ends_with("gm_simrec.amxx ; Loner-Strike health check recorder\n"));
        assert_eq!(with_recorder(&with), with);
        let a = server_args("cstrike", 27150, "pw", "cs_rats2");
        let s = a.join(" ");
        assert!(s.contains("+ip 127.0.0.1") && s.contains("-nomaster") && s.contains("+sv_lan 1") && s.ends_with("+map cs_rats2 +exec health.cfg"));
    }

    #[test]
    fn what_the_copy_leaves_out() {
        let ok = |p: &str| copy_filter(Path::new(p));
        assert!(ok("hlds.exe"));
        assert!(ok("cstrike/addons/amxmodx/plugins/gamemodes.amxx"));
        assert!(ok("cstrike/addons/amxmodx/data/gamemodes/global.cfg"));
        assert!(ok("cstrike/models/player/vip/vip.mdl"));
        assert!(!ok("steamapps/appmanifest_90.acf"));
        assert!(!ok("cstrike/maps/de_dust2.bsp"));
        assert!(!ok("czero/maps/de_dust2_cz.nav"));
        assert!(!ok("cstrike/logs/L1112000.log"));
        assert!(!ok("cstrike/addons/amxmodx/logs/error.log"));
        assert!(!ok("cstrike/addons/amxmodx/data/simrec/de_dust2_1.csv"));
        assert!(!ok("cstrike/addons/amxmodx/data/gamemodes/panel_rcon.txt"));
        assert!(!ok("cstrike/addons/amxmodx/data/gamemodes/status.json"));
        assert!(!ok("qconsole.log"));
        assert!(!ok("lstrace.csv"));
    }

    #[test]
    fn copies_only_what_changed() {
        let root = std::env::temp_dir().join(format!("ls-health-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(a.join("cstrike/maps")).unwrap();
        std::fs::write(a.join("hlds.exe"), b"x").unwrap();
        std::fs::write(a.join("cstrike/liblist.gam"), b"game").unwrap();
        std::fs::write(a.join("cstrike/maps/de_dust2.bsp"), b"map").unwrap();
        let stop = AtomicBool::new(false);
        assert_eq!(sync_copy(&a, &b, &stop).unwrap(), 2);
        assert!(!b.join("cstrike/maps/de_dust2.bsp").exists());
        assert_eq!(sync_copy(&a, &b, &stop).unwrap(), 0, "nothing changed");
        std::fs::write(a.join("cstrike/liblist.gam"), b"game 2").unwrap();
        assert_eq!(sync_copy(&a, &b, &stop).unwrap(), 1);
        assert_eq!(std::fs::read(b.join("cstrike/liblist.gam")).unwrap(), b"game 2");
        stop.store(true, Ordering::SeqCst);
        assert_eq!(sync_copy(&a, &b, &stop).unwrap_err(), STOPPED);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reports_saved_listed_and_pruned() {
        let dir = std::env::temp_dir().join(format!("ls-health-reports-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut r = build_report(&[rec(FALLS)], "", None, &asked("classic"));
        for i in 0..(KEEP_REPORTS as u64 + 3) {
            r.id = format!("{}", 1_700_000_000_000u64 + i);
            r.when = 1_700_000_000 + i;
            save_report(&dir, &r).unwrap();
        }
        std::fs::write(dir.join("9999999.json"), b"{not json").unwrap();
        let all = load_reports(&dir);
        assert_eq!(all.len(), KEEP_REPORTS);
        assert_eq!(all[0].when, 1_700_000_000 + KEEP_REPORTS as u64 + 2);
        assert_eq!(all[0].falls, 3);
        assert_eq!(all[0], { let mut x = r.clone(); x.id = all[0].id.clone(); x.when = all[0].when; x });
        assert!(valid_id(&all[0].id));
        assert!(!valid_id("../x") && !valid_id("12"));
        let s = summary(&all[0]);
        assert_eq!(s["headline"], "Only 3 of 12 bots joined the game.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn small_words() {
        assert_eq!(duration(45.4), "45 s");
        assert_eq!(duration(120.0), "2 min");
        assert_eq!(duration(291.0), "4 min 51 s");
        assert_eq!(without_stamp("L 11/12/2026 - 01:33:38: [AMXX] Run time error 4"), "[AMXX] Run time error 4");
        assert_eq!(without_stamp("Host_Error: x"), "Host_Error: x");
        assert_eq!(plugin_of("[miscstats.amxx] failed"), "miscstats.amxx");
        assert_eq!(plugin_of("[AMXX] Run time error"), "AMX Mod X");
    }

    /// The whole W1-1 run of the rats2_v5 sweep with the fixed nav it ran with:
    /// `LS_TEST_SIM=~/loner-strike-sim cargo test real_sweep -- --nocapture`.
    /// sweepreport.py said: 12/12 bots, 63 deaths, 14 to the world, 4 of 77 bot-rounds sat in spawn.
    #[test]
    fn real_sweep() {
        let Ok(sim) = std::env::var("LS_TEST_SIM") else { return };
        let sim = PathBuf::from(sim);
        let dir = sim.join("results/rats2_v5");
        let recs: Vec<Recording> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("W1-1_"))
            .filter_map(|p| read_recording(&p))
            .collect();
        let console = std::fs::read_to_string(dir.join("console_W1-1_classic_cs_rats2_150726.log")).unwrap();
        let nav = Nav::parse(&std::fs::read(sim.join("navpack/fixes/cs_rats2.nav")).unwrap()).unwrap();
        let rep = build_report(&recs, &console, Some(&nav), &Asked { map: "cs_rats2".into(), mode: "classic".into(), bots: 12, minutes: 10 });
        println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        assert_eq!(rep.bots_seen, 12);
        assert_eq!(rep.deaths, 63);
        assert_eq!(rep.world_deaths, 14);
        assert_eq!(rep.knowledge, "loaded");
        assert_eq!(rep.plugin_errors, 0);
        assert!(rep.fatal.is_empty());
        assert!((rep.spawn_sitters as i64 - 4).abs() <= 2 && (rep.bot_rounds as i64 - 77).abs() <= 6, "{} / {}", rep.spawn_sitters, rep.bot_rounds);
        assert!(rep.coverage_pct.unwrap() > 30.0 && rep.coverage_pct.unwrap() < 60.0);
        assert!(rep.falls >= 10);
    }
}
