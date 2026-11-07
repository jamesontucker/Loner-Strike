//! Bot navigation files (<map>.nav): what zBot knows about a map (CS 1.6 with ReGameDLL
//! and Condition Zero use the same format). Two things here:
//! - sending the navs this computer's games have to servers (every sync, a local server's
//!   start, and "Send to servers" on the Maps page)
//! - "Teach bots these maps": a hidden dedicated server on this computer loads each map,
//!   a bot learns it, and the .nav it saves goes into the game's maps folder (from where
//!   the first part sends it on)

use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::servers::{self, Entry};
use crate::setup::{self, Job, Progress};
use crate::steam::Install;
use crate::sys;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get as route_get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// The setup job's kind while bots learn maps (server syncs carry on meanwhile)
pub const JOB_KIND: &str = "navlearn";
/// zBot's nav files start with this, then the format version, then (version 4 and up)
/// the size of the .bsp they were made from (ReGameDLL game_shared/bot/nav_file.cpp)
const NAV_MAGIC: u32 = 0xFEED_FACE;
/// Smaller than this, a nav is a partial or broken file: never sent
pub const MIN_NAV: u64 = 1024;
/// A nav changed this recently may still be being written by the game: left for later
pub const SETTLE: Duration = Duration::from_secs(60);
const GAMES: [&str; 2] = ["cstrike", "czero"];

// ---- reading nav files ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavHead {
    pub version: u32,
    /// the size of the map file it was learned on (None before version 4)
    pub bsp_size: Option<u32>,
}

pub fn parse_head(b: &[u8]) -> Option<NavHead> {
    let word = |i: usize| b.get(i..i + 4).map(|x| u32::from_le_bytes(x.try_into().unwrap()));
    if word(0)? != NAV_MAGIC {
        return None;
    }
    let version = word(4)?;
    if version == 0 || version > 64 {
        return None;
    }
    let bsp_size = if version >= 4 { Some(word(8)?) } else { None };
    Some(NavHead { version, bsp_size })
}

fn read_head(path: &Path) -> Option<NavHead> {
    let mut b = Vec::with_capacity(12);
    std::fs::File::open(path).ok()?.take(12).read_to_end(&mut b).ok()?;
    parse_head(&b)
}

/// A nav file on this computer that's fit to send.
#[derive(Debug, Clone)]
pub struct Nav {
    pub path: PathBuf,
    /// the file's own name ("de_Cbble.nav")
    pub file: String,
    /// the game folder it's in ("cstrike" or "czero")
    pub game: String,
    pub modified: SystemTime,
    pub bsp_size: Option<u32>,
}

/// This computer's navs, by map (lower case): the ones fit to send, and maps whose nav
/// changed under a minute ago (maybe still being written).
#[derive(Default, Debug)]
pub struct LocalNavs {
    pub usable: HashMap<String, Vec<Nav>>,
    pub fresh: HashSet<String>,
}

/// The navs in <hl>/cstrike/maps and <hl>/czero/maps: whole files (the right kind, 1 KB
/// or more) not changed in the last minute.
pub fn local_navs(hl: &Path, now: SystemTime) -> LocalNavs {
    let mut out = LocalNavs::default();
    for g in GAMES {
        for f in std::fs::read_dir(hl.join(g).join("maps")).into_iter().flatten().flatten() {
            let file = f.file_name().to_string_lossy().to_string();
            let Some(stem) = file.to_lowercase().strip_suffix(".nav").map(String::from) else { continue };
            let Ok(meta) = f.metadata() else { continue };
            let Ok(modified) = meta.modified() else { continue };
            if !meta.is_file() || meta.len() < MIN_NAV {
                continue;
            }
            // (a time in the future is a clock that was off, not a file being written)
            if now.duration_since(modified).is_ok_and(|age| age < SETTLE) {
                out.fresh.insert(stem);
                continue;
            }
            let Some(head) = read_head(&f.path()) else { continue };
            out.usable.entry(stem).or_default().push(Nav { path: f.path(), file, game: g.into(), modified, bsp_size: head.bsp_size });
        }
    }
    out
}

/// Which of a map's navs suits a server: one learned on the same version of the map (the
/// map size inside it matches), or, when the map can't be checked, one from the server's
/// own game. The newest of those.
pub fn pick<'a>(navs: &'a [Nav], server_game: &str, map_size: Option<u64>) -> Option<&'a Nav> {
    navs.iter()
        .filter(|n| match (map_size, n.bsp_size) {
            (Some(size), Some(b)) => b as u64 == size,
            _ => n.game == server_game,
        })
        .max_by_key(|n| n.modified)
}

/// A file in `dir` named <stem>.<ext>, whatever its case.
fn find_file(dir: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    let want = format!("{}.{ext}", stem.to_lowercase());
    std::fs::read_dir(dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == want).map(|e| e.path())
}

/// The size of the first <dir>/maps/<map>.bsp there is.
fn bsp_size(dirs: &[PathBuf], stem: &str) -> Option<u64> {
    dirs.iter().find_map(|d| find_file(&d.join("maps"), stem, "bsp")).and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len())
}

/// Where a game looks for maps: its own folder, and CS 1.6's for Condition Zero.
fn game_dirs(base: &Path, game: &str) -> Vec<PathBuf> {
    let mut v = vec![base.join(game)];
    if game == "czero" {
        v.push(base.join("cstrike"));
    }
    v
}

/// The navs in a folder: map (lower case) -> (file name, modified).
fn navs_in(dir: &Path) -> HashMap<String, (String, SystemTime)> {
    let mut m = HashMap::new();
    for f in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = f.file_name().to_string_lossy().to_string();
        if let Some(stem) = name.to_lowercase().strip_suffix(".nav") {
            if let Some(t) = f.metadata().ok().and_then(|m| m.modified().ok()) {
                m.insert(stem.to_string(), (name, t));
            }
        }
    }
    m
}

/// Copies `src` over `dest` in one step (a running server never reads half a file),
/// with the source's time, so it only counts as newer when it is.
fn place(src: &Path, dest: &Path, modified: SystemTime) -> std::io::Result<()> {
    let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = dest.with_file_name(format!(".{name}.part"));
    let r = (|| {
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::copy(src, &tmp)?;
        std::fs::File::options().write(true).open(&tmp)?.set_modified(modified)?;
        std::fs::rename(&tmp, dest)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// This computer's game: the one picked in the app, else the first one found.
pub fn local_hl() -> Option<PathBuf> {
    if let Some(hl) = crate::app_api::load_settings().game_hl.map(PathBuf::from).filter(|p| p.is_dir()) {
        return Some(hl);
    }
    crate::steam::find_installs().into_iter().find(|i| !i.dedicated && !i.games.is_empty()).map(|i| i.hl)
}

// ---- sending navs to servers --------------------------------------------------------------

/// What happened for one server (maps, lower case).
#[derive(Default, Debug)]
pub struct Sent {
    /// put in the server's folder (this computer's copy, for one on another machine)
    pub sent: Vec<String>,
    /// the server's own is the same or newer
    pub had: Vec<String>,
    /// none of ours was learned on the server's version of the map
    pub other_version: Vec<String>,
}

/// Maps a server on another machine has a newer nav for than ours (its bots learned
/// them), by "<server>/<map>" -> our file's time: not asked again until ours changes.
static MACHINE_NEWER: Mutex<Option<HashMap<String, SystemTime>>> = Mutex::new(None);
/// One copy into a server's folder at a time (a sync and Send to servers at once)
static PLACING: Mutex<()> = Mutex::new(());

/// Puts this computer's navs (newer than the server's) into the server's maps folder:
/// all of them, or just `only`. Nothing there is ever deleted. `quiet`: from a sync,
/// where a machine that already said it has a newer one isn't asked again.
pub fn copy_to_server(e: &Entry, only: Option<&str>, quiet: bool) -> Sent {
    let Some(hl) = local_hl() else { return Sent::default() };
    let local = local_navs(&hl, SystemTime::now());
    copy_with(e, &hl, &local, only, quiet)
}

pub fn copy_with(e: &Entry, hl: &Path, local: &LocalNavs, only: Option<&str>, quiet: bool) -> Sent {
    let _g = PLACING.lock().unwrap_or_else(|e| e.into_inner());
    let dest_dir = e.dir.join(&e.game).join("maps");
    let there = navs_in(&dest_dir);
    let mut out = Sent::default();
    let mut copy: Vec<(&String, &Nav)> = Vec::new();
    let mut ask: Vec<(&String, &Nav)> = Vec::new();
    for (stem, navs) in &local.usable {
        if only.is_some_and(|m| !m.eq_ignore_ascii_case(stem)) {
            continue;
        }
        // the server's map, or (not in its folder: a server on another machine has Valve's
        // own maps there) this computer's map for the server's game
        let size = bsp_size(&game_dirs(&e.dir, &e.game), stem).or_else(|| bsp_size(&game_dirs(hl, &e.game), stem));
        let Some(nav) = pick(navs, &e.game, size) else {
            out.other_version.push(stem.clone());
            continue;
        };
        match there.get(stem) {
            Some((_, t)) if nav.modified <= *t => out.had.push(stem.clone()),
            Some(_) => copy.push((stem, nav)),
            // a server on another machine may have learned the map itself: that file isn't
            // in this computer's copy, so ask the machine before sending ours over it
            None if e.remote.is_some() => ask.push((stem, nav)),
            None => copy.push((stem, nav)),
        }
    }
    if let (false, Some(r)) = (ask.is_empty(), &e.remote) {
        let mut newer = MACHINE_NEWER.lock().unwrap_or_else(|e| e.into_inner());
        let newer = newer.get_or_insert_with(HashMap::new);
        let key = |stem: &str| format!("{}/{stem}", e.id);
        if quiet {
            ask.retain(|(stem, nav)| {
                let known = newer.get(&key(stem)) == Some(&nav.modified);
                if known {
                    out.had.push((*stem).clone());
                }
                !known
            });
        }
        if !ask.is_empty() {
            // (can't tell: nothing sent now, asked again next time)
            if let Ok(on_machine) = machine_navs(r, &e.id, &e.game) {
                for (stem, nav) in ask {
                    let ours = nav.modified.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
                    match on_machine.get(stem.as_str()) {
                        Some(t) if *t >= ours => {
                            newer.insert(key(stem), nav.modified);
                            out.had.push(stem.clone());
                        }
                        _ => copy.push((stem, nav)),
                    }
                }
            }
        }
    }
    for (stem, nav) in copy {
        let name = there.get(stem).map(|(n, _)| n.clone()).unwrap_or_else(|| nav.file.clone());
        if place(&nav.path, &dest_dir.join(name), nav.modified).is_ok() {
            out.sent.push(stem.clone());
        }
    }
    out
}

/// The navs on a server's machine: map (lower case) -> modified (seconds since 1970).
fn machine_navs(r: &crate::remote::RemoteInfo, id: &str, game: &str) -> Result<HashMap<String, f64>, String> {
    let dir = format!("{}/{game}/maps", crate::remote::server_dir(id));
    let (_, out) = crate::remote::exec(r, &format!("cd {} 2>/dev/null && find . -maxdepth 1 -type f -iname '*.nav' -printf '%f %T@\\n'; true", crate::remote::shell_quote(&dir)))?;
    Ok(parse_machine_navs(&out))
}

fn parse_machine_navs(out: &str) -> HashMap<String, f64> {
    out.lines()
        .filter_map(|l| {
            let (name, t) = l.trim().rsplit_once(' ')?;
            Some((name.to_lowercase().strip_suffix(".nav")?.to_string(), t.parse().ok()?))
        })
        .collect()
}

/// "de_a", "de_a and de_b", "de_a, de_b and 3 more"
fn names(list: &[String]) -> String {
    match list.len() {
        0 => String::new(),
        1 => list[0].clone(),
        2..=3 => format!("{} and {}", list[..list.len() - 1].join(", "), list[list.len() - 1]),
        n => format!("{} and {} more", list[..2].join(", "), n - 2),
    }
}

/// Sends one map's nav to every server now (the ones on other machines get it uploaded).
fn send_map(hl: &Path, map: &str) -> Result<Value, String> {
    let local = local_navs(hl, SystemTime::now());
    let stem = map.to_lowercase();
    if !local.usable.contains_key(&stem) {
        return Err(if local.fresh.contains(&stem) {
            format!("The bots saved what they know about {map} less than a minute ago. Send it again in a minute.")
        } else {
            format!("Bots haven't learned {map} on this computer yet.")
        });
    }
    let list: Vec<Entry> = servers::load().into_iter().filter(|e| e.ready).collect();
    if list.is_empty() {
        return Err("You have no servers yet.".into());
    }
    let (mut sent, mut had, mut other, mut failed) = (vec![], vec![], vec![], vec![]);
    for e in list {
        let s = copy_with(&e, hl, &local, Some(map), false);
        if !s.sent.is_empty() {
            match e.remote.is_some().then(|| servers::sync(&e, false)) {
                Some(Err(_)) => failed.push(e.name),
                _ => sent.push(e.name),
            }
        } else if !s.had.is_empty() {
            had.push(e.name);
        } else {
            other.push(e.name);
        }
    }
    let mut parts = Vec::new();
    if !sent.is_empty() {
        parts.push(format!("Sent the bots' file for {map} to {}.", names(&sent)));
    }
    if !had.is_empty() {
        parts.push(format!("{} already {} it or a newer one.", names(&had), if had.len() == 1 { "has" } else { "have" }));
    }
    if !other.is_empty() {
        parts.push(format!("{} {} a different version of the map, so {} left as it was.", names(&other), if other.len() == 1 { "has" } else { "have" }, if other.len() == 1 { "it was" } else { "they were" }));
    }
    if !failed.is_empty() {
        parts.push(format!("Couldn't reach {}. It goes up with the next sync.", names(&failed)));
    }
    Ok(json!({"ok": true, "sent": sent.len(), "message": parts.join(" ")}))
}

// ---- teaching bots maps -------------------------------------------------------------------

/// A map on this computer to learn.
#[derive(Debug, Clone)]
pub struct LocalMap {
    /// as the .bsp is named
    pub name: String,
    /// the game folder the .bsp is in
    pub game: String,
    pub bsp: PathBuf,
}

pub enum Missed {
    Stopped,
    Failed(String),
}

/// Where bots learn maps. This computer's hidden server (LocalLearner) for now; a server
/// on another machine can do the same later: get ready, learn each map, hand back the
/// .nav it saved. Everything around it (the job, where results go) stays the same.
pub trait Learner: Send {
    /// Gets a server ready to run (downloaded or copied). Once, before the first map.
    fn prepare(&mut self, job: &Job, games: &[String]) -> Result<(), String>;
    /// Learns one map: the .nav file on this computer when it's done.
    fn learn(&mut self, job: &Job, map: &LocalMap, stop: &AtomicBool) -> Result<PathBuf, Missed>;
    /// After the last map (or a stop).
    fn finish(&mut self) {}
}

static STOP: AtomicBool = AtomicBool::new(false);
/// How the last run went (for the toast at its end)
static RESULT: Mutex<Option<Value>> = Mutex::new(None);
/// The longest a map may take (the biggest maps take 20 minutes or so on a slow computer)
const LEARN_LIMIT: Duration = Duration::from_secs(45 * 60);

fn find_map(hl: &Path, name: &str) -> Option<LocalMap> {
    // Condition Zero's own version of a map first: what CZ plays when both are there
    for g in ["czero", "cstrike"] {
        if let Some(p) = find_file(&hl.join(g).join("maps"), name, "bsp") {
            let stem = p.file_stem()?.to_string_lossy().to_string();
            return Some(LocalMap { name: stem, game: g.into(), bsp: p });
        }
    }
    None
}

fn has_nav(hl: &Path, name: &str) -> bool {
    GAMES.iter().any(|g| find_file(&hl.join(g).join("maps"), name, "nav").is_some())
}

/// What a run did, in a sentence or two.
pub fn summary(learned: &[String], failed: &[(String, String)], skipped: &[(String, String)], stopped: bool, asked: usize) -> String {
    let mut parts = Vec::new();
    if stopped {
        parts.push(format!("Stopped. Bots learned {} of {asked} map{}.", learned.len(), if asked == 1 { "" } else { "s" }));
    } else if !learned.is_empty() {
        parts.push(format!("Bots learned {}. Servers get {} with the next sync.", names(learned), if learned.len() == 1 { "it" } else { "them" }));
    }
    for (m, why) in failed.iter().take(3) {
        parts.push(format!("Couldn't learn {m}: {why}"));
    }
    if failed.len() > 3 {
        parts.push(format!("{} more couldn't be learned.", failed.len() - 3));
    }
    let known: Vec<String> = skipped.iter().filter(|(_, w)| w == KNOWN).map(|(m, _)| m.clone()).collect();
    if !known.is_empty() {
        parts.push(format!("Skipped {}: bots know {} already.", names(&known), if known.len() == 1 { "it" } else { "them" }));
    }
    parts.join(" ")
}

const KNOWN: &str = "known";

/// Starts teaching bots `maps` (the ones they already know only with `relearn`). Returns
/// the job id.
pub fn start_learning(client: Install, maps: Vec<String>, relearn: bool) -> Result<String, String> {
    if client.kind == "crossover" && !Path::new(servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The hidden server runs through it, like the game.".into());
    }
    let hl = client.hl.clone();
    let mut todo: Vec<LocalMap> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    for m in maps.into_iter().take(500) {
        if !servers::safe_map(&m) || !seen.insert(m.to_lowercase()) {
            continue;
        }
        match find_map(&hl, &m) {
            None => skipped.push((m, "missing".into())),
            Some(_) if !relearn && has_nav(&hl, &m) => skipped.push((m, KNOWN.into())),
            Some(lm) => todo.push(lm),
        }
    }
    if todo.is_empty() {
        return Err(if skipped.iter().any(|(_, w)| w == KNOWN) {
            "Bots know these maps already. Turn on Re-learn to teach them again.".into()
        } else {
            "Pick maps that are on this computer.".into()
        });
    }
    let n = todo.len();
    let mut steps = vec![setup::server_step(&todo[0].game, "nav-prep", "Getting a hidden server ready", 1.0)];
    for (i, m) in todo.iter().enumerate() {
        steps.push(setup::server_step(&m.game, &format!("nav-{i}"), format!("Learning {} ({} of {n})", m.name, i + 1), 4.0));
    }
    let mut learner = LocalLearner::new(client)?;
    STOP.store(false, Ordering::SeqCst);
    *RESULT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let mut games: Vec<String> = todo.iter().map(|m| m.game.clone()).collect();
    games.dedup();
    games.sort();
    games.dedup();
    setup::start_job(JOB_KIND, steps, move |job| {
        let first = todo[0].game.clone();
        let prepared = job.run(&format!("{first}/nav-prep"), || learner.prepare(job, &games));
        if let Err(e) = prepared {
            learner.finish();
            return Err(e);
        }
        let (mut learned, mut failed, mut stopped) = (Vec::new(), Vec::new(), false);
        for (i, m) in todo.iter().enumerate() {
            if STOP.load(Ordering::SeqCst) {
                stopped = true;
                break;
            }
            let _ = job.run(&format!("{}/nav-{i}", m.game), || {
                match learner.learn(job, m, &STOP) {
                    Ok(nav) => match install_nav(&hl, m, &nav, relearn) {
                        Ok(()) => learned.push(m.name.clone()),
                        Err(e) => failed.push((m.name.clone(), e)),
                    },
                    Err(Missed::Stopped) => stopped = true,
                    Err(Missed::Failed(why)) => failed.push((m.name.clone(), why)),
                }
                // (a map that failed doesn't stop the others)
                Ok::<(), String>(())
            });
            if stopped {
                break;
            }
        }
        learner.finish();
        let text = summary(&learned, &failed, &skipped, stopped, n);
        *RESULT.lock().unwrap_or_else(|e| e.into_inner()) = Some(json!({"learned": learned, "failed": failed.len(), "stopped": stopped, "message": text}));
        if !learned.is_empty() {
            // servers on other machines look for the new files soon (once they've settled)
            for e in servers::load().into_iter().filter(|e| e.remote.is_some()) {
                servers::mark_changed(&e.id);
            }
        }
        if !stopped && learned.is_empty() {
            return Err(text);
        }
        Ok(())
    })
}

/// The learned nav into this computer's game: next to the map's .bsp, and over the one in
/// the other game folder too when that one is being learned again (the game would find
/// that one first otherwise). Nothing is deleted.
fn install_nav(hl: &Path, m: &LocalMap, nav: &Path, relearn: bool) -> Result<(), String> {
    let modified = std::fs::metadata(nav).and_then(|x| x.modified()).map_err(|e| format!("the saved file couldn't be read ({e})."))?;
    let mut dests = vec![m.bsp.with_file_name(format!("{}.nav", m.name))];
    if relearn {
        for g in GAMES.iter().filter(|g| **g != m.game) {
            if let Some(p) = find_file(&hl.join(g).join("maps"), &m.name, "nav") {
                dests.push(p);
            }
        }
    }
    // a differently written name of the same file is written over, not added next to it
    if let Some(p) = find_file(&hl.join(&m.game).join("maps"), &m.name, "nav") {
        dests[0] = p;
    }
    for d in dests {
        place(nav, &d, modified).map_err(|e| format!("the file couldn't be saved in the game's maps folder ({e})."))?;
    }
    Ok(())
}

/// What the hidden server's console says about the learning.
#[derive(Debug, PartialEq)]
pub enum LogSays {
    Nothing,
    Saved,
    Failed(String),
}

/// ReGameDLL prints "Navigation file 'cstrike\maps\<map>.nav' saved." when the bot has
/// learned and analysed the map (cs_bot_learn.cpp UpdateSaveProcess).
pub fn read_log(text: &str, map: &str) -> LogSays {
    let lower = text.to_lowercase();
    let map = map.to_lowercase();
    if lower.lines().any(|l| l.contains("navigation file") && l.contains(&format!("{map}.nav")) && l.contains("saved")) {
        return LogSays::Saved;
    }
    if lower.contains("start position invalid") {
        return LogSays::Failed("bots couldn't find a place to start on it.".into());
    }
    if let Some(l) = text.lines().find(|l| l.contains("Host_Error") || l.to_lowercase().contains("couldn't spawn server")) {
        return LogSays::Failed(format!("the server couldn't load it ({}).", l.trim().chars().take(120).collect::<String>()));
    }
    LogSays::Nothing
}

/// The models and sprites a map's entities name ("models/props/barrel.mdl"): the server
/// loads those with the map, and stops if one is missing.
pub fn referenced_files(entities: &str) -> Vec<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r#"(?i)"((?:models|sprites)[/\\][A-Za-z0-9_./\\ -]{1,100}\.(?:mdl|spr))""#).unwrap());
    let mut out: Vec<String> = Vec::new();
    for c in re.captures_iter(entities) {
        let p = c[1].replace('\\', "/");
        if !p.contains("..") && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// The entity text of a .bsp (lump 0).
fn entity_text(bsp: &Path) -> Option<String> {
    let mut f = std::fs::File::open(bsp).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    let offset = u32::from_le_bytes(head[4..8].try_into().unwrap()) as u64;
    let len = (u32::from_le_bytes(head[8..12].try_into().unwrap()) as u64).min(8 << 20);
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = Vec::new();
    f.take(len).read_to_end(&mut buf).ok()?;
    Some(crate::rcon::latin1(&buf))
}

/// The hidden server's start options: this computer only, never listed, and a password
/// for asking it to quit.
pub fn learn_args(game: &str, port: u16, password: &str, map: &str) -> Vec<String> {
    [
        "-console", "-game", game, "-port", &port.to_string(), "+ip", "127.0.0.1", "-nomaster", "-insecure", "-condebug", "+sv_lan", "1", "+maxplayers", "4",
        "+bot_join_after_player", "0", "+bot_quota", "1", "+rcon_password", password, "+map", map,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// server.cfg of the hidden server (run at every map start): one bot, joining with nobody
/// on, and nothing that changes the map while it learns.
const LEARN_CFG: &str = "// Written by Loner-Strike: the hidden server that teaches bots maps\n\
hostname \"Loner-Strike bot teacher\"\nsv_lan 1\nmp_timelimit 0\nmp_maxrounds 0\nmp_winlimit 0\n\
bot_quota 1\nbot_quota_mode normal\nbot_join_after_player 0\nbot_join_delay 0\nbot_join_team any\n";

/// The hidden server on this computer: a copy of Valve's server (the one servers are made
/// from, downloaded first if there isn't one) in <servers folder>/.nav-learner, with no
/// plugins. It runs one map at a time, without a window and at low priority.
pub struct LocalLearner {
    client: Install,
    base: PathBuf,
    dir: PathBuf,
    /// maps copied in for one run (not Valve's own), and their navs: cleared at the end
    leftovers: Vec<PathBuf>,
}

impl LocalLearner {
    pub fn new(client: Install) -> Result<LocalLearner, String> {
        let root = servers::root_for(&client)?;
        Ok(LocalLearner { client, base: root.join(".base"), dir: root.join(".nav-learner"), leftovers: Vec::new() })
    }

    fn spawn(&self, args: &[String], log: std::fs::File) -> Result<Child, String> {
        let hlds = self.dir.join("hlds.exe");
        if !hlds.is_file() {
            return Err("The hidden server's files are missing.".into());
        }
        let mut cmd = if self.client.kind == "crossover" {
            let mut c = sys::command(servers::CROSSOVER_WINE);
            let win = servers::windows_path(&hlds).ok_or("The hidden server has to be inside the game's CrossOver bottle.")?;
            c.arg("--bottle").arg(self.client.bottle.as_deref().unwrap_or("Steam")).arg(win);
            c.env("WINEDEBUG", "-all");
            c
        } else {
            sys::command(&hlds)
        };
        low_priority(&mut cmd);
        cmd.args(args)
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log);
        cmd.spawn().map_err(|e| format!("The hidden server didn't start ({e})."))
    }

    /// The models and sprites the map names that the hidden server doesn't have, from
    /// this computer's game.
    fn copy_resources(&self, m: &LocalMap) {
        let Some(hl) = m.bsp.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) else { return };
        let Some(text) = entity_text(&m.bsp) else { return };
        let have = [self.dir.join(&m.game), self.dir.join("cstrike"), self.dir.join("valve")];
        for rel in referenced_files(&text) {
            if have.iter().any(|d| d.join(&rel).is_file()) {
                continue;
            }
            if let Some(src) = [hl.join(&m.game), hl.join("cstrike"), hl.join("valve")].iter().map(|d| d.join(&rel)).find(|p| p.is_file()) {
                let dest = self.dir.join(&m.game).join(&rel);
                if let Some(d) = dest.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::copy(src, dest);
            }
        }
    }
}

#[cfg(unix)]
fn low_priority(cmd: &mut Command) {
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
fn low_priority(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS
    cmd.creation_flags(0x0800_0000 | 0x0000_4000);
}

/// A UDP port nothing on this computer is using (the servers' own ports aside).
fn free_port() -> Result<u16, String> {
    let taken: HashSet<u16> = servers::load().iter().filter(|e| e.remote.is_none()).map(|e| e.port).collect();
    (27090..27140)
        .find(|p| !taken.contains(p) && std::net::UdpSocket::bind(("0.0.0.0", *p)).is_ok())
        .ok_or_else(|| "No free port for the hidden server.".to_string())
}

/// New text in a log since `from` (moved on to its end).
fn read_new(path: &Path, from: &mut u64) -> String {
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *from {
        *from = 0;
    }
    if f.seek(SeekFrom::Start(*from)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = f.take(1 << 20).read_to_end(&mut buf);
    *from += buf.len() as u64;
    crate::rcon::latin1(&buf)
}

impl Learner for LocalLearner {
    fn prepare(&mut self, job: &Job, games: &[String]) -> Result<(), String> {
        for g in games {
            if !(self.base.join("hlds.exe").is_file() && self.base.join(g).join("liblist.gam").is_file()) {
                job.detail("Downloading the server from Valve (the first time only, about 1 GB)");
                servers::ensure_steamcmd(job)?;
                servers::download_hlds(job, &self.base, g)?;
            }
        }
        job.detail("Copying the server's files");
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("Couldn't make the hidden server's folder ({e})."))?;
        let _ = std::fs::write(self.dir.join(".loner-strike-learner"), "bots learn maps here");
        servers::copy_tree(&self.base, &self.dir, false, &|rel: &Path| rel.components().next().is_some_and(|c| c.as_os_str() != "steamapps"))?;
        for g in games {
            let gdir = self.dir.join(g);
            if !gdir.join("botprofile.db").is_file() {
                return Err(format!("Valve's server for {} came without bots. Update the server files and try again.", setup::game_name(g)));
            }
            std::fs::write(gdir.join("server.cfg"), LEARN_CFG).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn learn(&mut self, job: &Job, m: &LocalMap, stop: &AtomicBool) -> Result<PathBuf, Missed> {
        let fail = |e: String| Missed::Failed(e);
        let maps = self.dir.join(&m.game).join("maps");
        std::fs::create_dir_all(&maps).map_err(|e| fail(e.to_string()))?;
        job.detail("Loading the map");
        let bsp = maps.join(format!("{}.bsp", m.name));
        let stock = self.base.join(&m.game).join("maps").join(format!("{}.bsp", m.name)).is_file();
        // this computer's own version, so the nav matches the map players have
        std::fs::copy(&m.bsp, &bsp).map_err(|e| fail(format!("the map couldn't be copied ({e}).")))?;
        self.copy_resources(m);
        // the hidden server's own folder: an old nav there (Condition Zero ships some) goes
        let nav = maps.join(format!("{}.nav", m.name));
        while let Some(p) = find_file(&maps, &m.name, "nav") {
            if std::fs::remove_file(&p).is_err() {
                return Err(fail("an old file in the hidden server's folder couldn't be cleared.".into()));
            }
        }
        let port = free_port().map_err(fail)?;
        let password: String = {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            (0..20).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect()
        };
        let logs = [self.dir.join("qconsole.log"), self.dir.join(&m.game).join("qconsole.log"), self.dir.join("learn-server.log")];
        let mut from: Vec<u64> = logs.iter().map(|p| std::fs::metadata(p).map(|x| x.len()).unwrap_or(0)).collect();
        let out = std::fs::File::create(&logs[2]).map_err(|e| fail(e.to_string()))?;
        from[2] = 0;
        let mut child = self.spawn(&learn_args(&m.game, port, &password, &m.name), out).map_err(fail)?;
        sys::tie_to_app(&child);
        let started = Instant::now();
        let mut text = String::new();
        let mut last: Option<(u64, SystemTime)> = None;
        let result = loop {
            for _ in 0..10 {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            if stop.load(Ordering::SeqCst) {
                break Err(Missed::Stopped);
            }
            for (p, f) in logs.iter().zip(from.iter_mut()) {
                text.push_str(&read_new(p, f));
            }
            let says = read_log(&text, &m.name);
            if let LogSays::Failed(why) = says {
                break Err(fail(why));
            }
            // done: the file is there and whole (said so, or the same on two looks)
            let now = std::fs::metadata(&nav).ok().filter(|x| x.len() >= MIN_NAV).and_then(|x| Some((x.len(), x.modified().ok()?)));
            if let Some(now) = now.filter(|_| read_head(&nav).is_some()) {
                if says == LogSays::Saved || last == Some(now) {
                    break Ok(nav.clone());
                }
                last = Some(now);
            }
            if matches!(child.try_wait(), Ok(Some(_))) {
                break Err(fail("the hidden server closed while loading it.".into()));
            }
            if started.elapsed() > LEARN_LIMIT {
                break Err(fail("it took longer than 45 minutes.".into()));
            }
            let mins = started.elapsed().as_secs() / 60;
            job.detail(&if mins == 0 { "A bot is walking the map".to_string() } else { format!("A bot is walking the map ({mins} min so far)") });
        };
        // asked to quit, then ended if it hasn't
        let _ = crate::rcon::Rcon::at(password, "127.0.0.1", port).send("quit");
        for _ in 0..25 {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        sys::stop_child(child, Duration::from_secs(3));
        // a map that isn't Valve's goes again at the end (no second copy kept)
        if !stock {
            self.leftovers.extend([bsp, nav]);
        }
        result
    }

    fn finish(&mut self) {
        for p in self.leftovers.drain(..) {
            let _ = std::fs::remove_file(p);
        }
    }
}

// ---- API (the app window only) ------------------------------------------------------------

fn forbidden() -> (StatusCode, Json<Value>) {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn answer(r: Result<Value, String>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

const INTERNAL: &str = "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.";

#[derive(Deserialize)]
struct SendReq {
    map: String,
}

async fn api_send(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<SendReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(game) = st.local_game() else { return answer(Err("Set up the game first.".into())) };
    let r = tokio::task::spawn_blocking(move || {
        if !servers::safe_map(&req.map) {
            return Err("That isn't a map name.".to_string());
        }
        send_map(&game.hl, &req.map)
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

#[derive(Deserialize)]
struct LearnReq {
    maps: Vec<String>,
    #[serde(default)]
    relearn: bool,
}

async fn api_learn(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<LearnReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || {
        let client = servers::client(&st).ok_or("Set up the game on this computer first.")?;
        start_learning(client, req.maps, req.relearn).map(|job| json!({"ok": true, "job": job}))
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

async fn api_learn_stop(headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let job = setup::current();
    if job.kind != JOB_KIND || job.state != "running" {
        return answer(Err("Bots aren't learning maps right now.".into()));
    }
    STOP.store(true, Ordering::SeqCst);
    answer(Ok(json!({"ok": true})))
}

async fn api_learn_result() -> Json<Value> {
    Json(json!({"result": RESULT.lock().unwrap_or_else(|e| e.into_inner()).clone()}))
}

/// The app window's listener only (merged with the other computer-only routes).
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/navs/send", post(api_send))
        .route("/api/navs/learn", route_get(api_learn_result).post(api_learn))
        .route("/api/navs/learn/stop", post(api_learn_stop))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav_bytes(version: u32, bsp: u32, len: usize) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(NAV_MAGIC.to_le_bytes());
        b.extend(version.to_le_bytes());
        b.extend(bsp.to_le_bytes());
        b.resize(len, 7);
        b
    }

    fn write_at(path: &Path, bytes: &[u8], age: Duration) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        std::fs::File::options().write(true).open(path).unwrap().set_modified(SystemTime::now() - age).unwrap();
    }

    fn temp(prefix: &str) -> PathBuf {
        sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), prefix).unwrap()
    }

    fn server(dir: &Path, game: &str) -> Entry {
        serde_json::from_value(json!({"id": "t", "name": "Test", "game": game, "port": 27016, "dir": dir, "kind": "windows", "bottle": null, "ready": true})).unwrap()
    }

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn nav_heads() {
        assert_eq!(parse_head(&nav_bytes(5, 1234, 12)), Some(NavHead { version: 5, bsp_size: Some(1234) }));
        assert_eq!(parse_head(&nav_bytes(3, 1234, 12)), Some(NavHead { version: 3, bsp_size: None }));
        assert_eq!(parse_head(b"not a nav file"), None);
        assert_eq!(parse_head(&nav_bytes(5, 1, 12)[..6]), None, "cut short");
    }

    #[test]
    fn picks_the_right_version_newest_first() {
        let now = SystemTime::now();
        let n = |game: &str, size: Option<u32>, age: u64| Nav { path: PathBuf::new(), file: "x.nav".into(), game: game.into(), modified: now - Duration::from_secs(age), bsp_size: size };
        let navs = vec![n("cstrike", Some(100), 50), n("czero", Some(200), 10)];
        // the server's map decides, whichever game folder the nav is in
        assert_eq!(pick(&navs, "cstrike", Some(100)).unwrap().game, "cstrike");
        assert_eq!(pick(&navs, "cstrike", Some(200)).unwrap().game, "czero");
        assert!(pick(&navs, "cstrike", Some(300)).is_none(), "a different version of the map");
        // no map to check against: only the server's own game
        assert_eq!(pick(&navs, "czero", None).unwrap().game, "czero");
        let both = vec![n("cstrike", Some(100), 50), n("czero", Some(100), 10)];
        assert_eq!(pick(&both, "cstrike", Some(100)).unwrap().game, "czero", "the newest");
    }

    #[test]
    fn local_navs_leave_out_partial_and_fresh_files() {
        let hl = temp("navs-local-");
        write_at(&hl.join("cstrike/maps/de_ok.nav"), &nav_bytes(5, 10, 4000), HOUR);
        write_at(&hl.join("czero/maps/de_ok.nav"), &nav_bytes(5, 20, 4000), HOUR);
        write_at(&hl.join("cstrike/maps/de_small.nav"), &nav_bytes(5, 10, 200), HOUR);
        write_at(&hl.join("cstrike/maps/de_new.nav"), &nav_bytes(5, 10, 4000), Duration::from_secs(5));
        write_at(&hl.join("cstrike/maps/de_junk.nav"), &[0u8; 4000], HOUR);
        let l = local_navs(&hl, SystemTime::now());
        assert_eq!(l.usable.len(), 1);
        assert_eq!(l.usable["de_ok"].len(), 2, "from both games");
        assert!(l.fresh.contains("de_new"));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn copies_only_newer_and_never_deletes() {
        let root = temp("navs-copy-");
        let (hl, srv) = (root.join("game"), root.join("srv"));
        write_at(&hl.join("cstrike/maps/de_a.bsp"), &[0u8; 10], HOUR);
        write_at(&hl.join("cstrike/maps/de_a.nav"), &nav_bytes(5, 10, 3000), 2 * HOUR);
        write_at(&hl.join("cstrike/maps/de_b.nav"), &nav_bytes(5, 10, 3000), 2 * HOUR);
        write_at(&hl.join("cstrike/maps/de_c.nav"), &nav_bytes(5, 10, 3000), 2 * HOUR);
        write_at(&hl.join("cstrike/maps/de_d.nav"), &nav_bytes(5, 99, 3000), 2 * HOUR);
        // the server: an older de_b, a newer de_c (its bots learned it), its own de_z,
        // and a de_d.bsp of another version
        write_at(&srv.join("cstrike/maps/de_b.nav"), &nav_bytes(5, 10, 2000), 3 * HOUR);
        write_at(&srv.join("cstrike/maps/de_c.nav"), &nav_bytes(5, 10, 2500), HOUR);
        write_at(&srv.join("cstrike/maps/de_z.nav"), &nav_bytes(5, 10, 2500), HOUR);
        write_at(&srv.join("cstrike/maps/de_d.bsp"), &[0u8; 50], HOUR);
        let e = server(&srv, "cstrike");
        let local = local_navs(&hl, SystemTime::now());
        let mut s = copy_with(&e, &hl, &local, None, true);
        s.sent.sort();
        assert_eq!(s.sent, ["de_a", "de_b"]);
        assert_eq!(s.had, ["de_c"]);
        assert_eq!(s.other_version, ["de_d"]);
        let len = |m: &str| std::fs::metadata(srv.join("cstrike/maps").join(m)).unwrap().len();
        assert_eq!((len("de_a.nav"), len("de_b.nav"), len("de_c.nav"), len("de_z.nav")), (3000, 3000, 2500, 2500));
        assert!(!srv.join("cstrike/maps/de_d.nav").exists());
        // a second round has nothing to do
        let again = copy_with(&e, &hl, &local, None, true);
        assert!(again.sent.is_empty() && again.had.len() == 3);
        // one map only
        write_at(&hl.join("cstrike/maps/de_a.nav"), &nav_bytes(5, 10, 3100), Duration::from_secs(120));
        write_at(&hl.join("cstrike/maps/de_b.nav"), &nav_bytes(5, 10, 3100), Duration::from_secs(120));
        let local = local_navs(&hl, SystemTime::now());
        assert_eq!(copy_with(&e, &hl, &local, Some("DE_A"), false).sent, ["de_a"]);
        assert_eq!(len("de_b.nav"), 3000);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn condition_zero_servers_check_cs_maps_too() {
        let root = temp("navs-cz-");
        let (hl, srv) = (root.join("game"), root.join("srv"));
        write_at(&hl.join("cstrike/maps/de_x.nav"), &nav_bytes(5, 10, 3000), HOUR);
        write_at(&hl.join("czero/maps/de_x.nav"), &nav_bytes(5, 20, 3000), 2 * HOUR);
        write_at(&srv.join("cstrike/maps/de_x.bsp"), &[0u8; 10], HOUR);
        let e = server(&srv, "czero");
        let s = copy_with(&e, &hl, &local_navs(&hl, SystemTime::now()), None, true);
        assert_eq!(s.sent, ["de_x"]);
        assert_eq!(read_head(&srv.join("czero/maps/de_x.nav")).unwrap().bsp_size, Some(10), "the one made on CS 1.6's de_x");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn machine_listing() {
        let m = parse_machine_navs("de_dust2.nav 1700000000.5\nDe_Cbble.NAV 1600000000\nbad line\n");
        assert_eq!(m.len(), 2);
        assert_eq!(m["de_cbble"], 1_600_000_000.0);
    }

    #[test]
    fn console_lines() {
        assert_eq!(read_log("Size of bsp file 'maps\\de_x.bsp' is 1 bytes.\nNavigation file 'cstrike\\maps\\de_x.nav' saved.\n", "DE_X"), LogSays::Saved);
        assert_eq!(read_log("Navigation file 'cstrike\\maps\\de_y.nav' saved.\n", "de_x"), LogSays::Nothing, "another map");
        assert!(matches!(read_log("ERROR: Start position invalid\n", "de_x"), LogSays::Failed(_)));
        assert!(matches!(read_log("Host_Error: PF_precache_model_I: Model 'models/x.mdl' failed to load\n", "de_x"), LogSays::Failed(_)));
        assert_eq!(read_log("Adding bot\n", "de_x"), LogSays::Nothing);
    }

    #[test]
    fn map_resources() {
        let ents = r#"{ "classname" "worldspawn" "wad" "\half-life\cstrike.wad" }
{ "classname" "env_sprite" "model" "sprites/glow01.spr" }
{ "classname" "cycler" "model" "models\props\barrel.mdl" }
{ "classname" "func_wall" "model" "*3" }
{ "classname" "cycler" "model" "models/../../evil.mdl" }
{ "classname" "env_sprite" "model" "sprites/glow01.spr" }"#;
        assert_eq!(referenced_files(ents), ["sprites/glow01.spr", "models/props/barrel.mdl"]);
    }

    #[test]
    fn hidden_server_options() {
        let a = learn_args("czero", 27090, "pw", "de_x").join(" ");
        assert!(a.contains("-game czero") && a.contains("+ip 127.0.0.1") && a.contains("-nomaster") && a.contains("+sv_lan 1") && a.contains("+bot_join_after_player 0"));
        assert!(a.ends_with("+map de_x"), "the map last, after the settings");
    }

    #[test]
    fn summaries() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(summary(&s(&["de_a"]), &[], &[], false, 1), "Bots learned de_a. Servers get it with the next sync.");
        assert_eq!(summary(&s(&["de_a"]), &[], &[], true, 3), "Stopped. Bots learned 1 of 3 maps.");
        let t = summary(&[], &[("de_b".into(), "it took longer than 45 minutes.".into())], &[("de_c".into(), KNOWN.into())], false, 1);
        assert_eq!(t, "Couldn't learn de_b: it took longer than 45 minutes. Skipped de_c: bots know it already.");
        assert_eq!(names(&s(&["a", "b", "c", "d", "e"])), "a, b and 3 more");
    }
}
