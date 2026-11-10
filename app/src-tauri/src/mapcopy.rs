//! "Copy to a server": maps from this computer's game (cstrike/maps and czero/maps, with
//! whatever each needs: textures, sky, models, sprites, sounds, overview, bot nav) into a
//! dedicated server's folder here. A server on another machine gets them uploaded right
//! after, like any change to its copy. Nothing the server already has is replaced: a file
//! it has that differs from ours is kept and said so. Each copied map is recorded in the
//! server's own map record (.installed_maps.json), so Remove takes it off again.

use crate::gamebanana::MapStore;
use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::servers::{self, Entry};
use crate::setup::{self, Progress};
use crate::sys;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get as route_get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The setup job's kind while maps are copied
pub const JOB_KIND: &str = "mapcopy";
/// Half-Life's and CS 1.6's own texture files: every server has them (Condition Zero's
/// aren't here on purpose: a CS 1.6 server doesn't have those).
const STOCK_WADS: &[&str] = &[
    // valve
    "cached.wad", "decals.wad", "fonts.wad", "gfx.wad", "halflife.wad", "liquids.wad", "pldecal.wad", "spraypaint.wad", "tempdecal.wad", "xeno.wad",
    // cstrike
    "cs_747.wad", "cs_assault.wad", "cs_bdog.wad", "cs_cbble.wad", "cs_dust.wad", "cs_havana.wad", "cs_office.wad", "cstraining.wad", "cstrike.wad",
    "de_airstrip.wad", "de_aztec.wad", "de_piranesi.wad", "de_storm.wad", "de_vertigo.wad", "itsitaly.wad", "n0th1ng.wad", "prodigy.wad", "torntextures.wad",
];
const SKY_SIDES: [&str; 6] = ["up", "dn", "lf", "rt", "ft", "bk"];
/// GoldSrc's map format (Condition Zero's maps use it too, so most play on CS 1.6)
const BSP_VERSION: u32 = 30;
/// Maps in one request at most
const MAX_MAPS: usize = 3000;

// ---- finding files whatever their case ---------------------------------------------------

/// Finds files by a game path whatever the case of its folders and name (a map names
/// "Models/Props/Barrel.mdl", the file is models/props/barrel.mdl). Folder listings are
/// kept for the finder's life.
#[derive(Default)]
pub struct Finder {
    dirs: RefCell<HashMap<PathBuf, Option<HashMap<String, String>>>>,
}

impl Finder {
    /// The file as it's named on disk (so a copy keeps its real name).
    pub fn find(&self, base: &Path, rel: &str) -> Option<PathBuf> {
        let mut cur = base.to_path_buf();
        for part in rel.split('/').filter(|p| !p.is_empty()) {
            match self.entry(&cur, part) {
                Some(name) => cur.push(name),
                None => {
                    let exact = base.join(rel);
                    return exact.is_file().then_some(exact);
                }
            }
        }
        cur.is_file().then_some(cur)
    }

    fn entry(&self, dir: &Path, part: &str) -> Option<String> {
        let mut dirs = self.dirs.borrow_mut();
        let list = dirs.entry(dir.to_path_buf()).or_insert_with(|| {
            std::fs::read_dir(dir).ok().map(|rd| {
                rd.flatten()
                    .map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        (n.to_lowercase(), n)
                    })
                    .collect()
            })
        });
        list.as_ref()?.get(&part.to_lowercase()).cloned()
    }
}

// ---- what a map needs ------------------------------------------------------------------------

/// A game path a map may bring along: under the folders map content lives in (or a
/// texture file at the top), without .. or drive letters. Back slashes become slashes.
pub fn clean_rel(rel: &str) -> Option<String> {
    let r = rel.trim().trim_matches('"').replace('\\', "/");
    let r = r.trim_start_matches('/');
    let parts: Vec<&str> = r.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() || r.len() > 160 || parts.iter().any(|p| *p == ".." || p.contains(':')) {
        return None;
    }
    if !r.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-/ ()+!#~".contains(&c)) {
        return None;
    }
    let first = parts[0].to_lowercase();
    let ok = if parts.len() == 1 {
        first.ends_with(".wad")
    } else {
        matches!(first.as_str(), "maps" | "models" | "sound" | "sprites" | "gfx" | "overviews" | "resource")
    };
    ok.then(|| parts.join("/"))
}

/// The game paths in a map's .res file (one per line, // comments).
pub fn res_files(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(r) = clean_rel(line) {
            if !out.contains(&r) {
                out.push(r);
            }
        }
    }
    out
}

/// What a map's entities name: its texture files (worldspawn "wad", just the file names),
/// its sky (gfx/env/<sky><side>.tga), and the models, sprites and sounds entities use.
pub fn entity_files(entities: &str) -> Vec<String> {
    use regex::Regex;
    use std::sync::OnceLock;
    static RES: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    let (wad, sky, sound) = RES.get_or_init(|| {
        (
            Regex::new(r#"(?i)"wad"\s+"([^"]*)""#).unwrap(),
            Regex::new(r#"(?i)"skyname"\s+"([^"]*)""#).unwrap(),
            Regex::new(r#"(?i)"[*!#]?([A-Za-z0-9_./\\ -]{1,100}\.(?:wav|mp3))""#).unwrap(),
        )
    });
    let mut out: Vec<String> = Vec::new();
    let mut add = |r: Option<String>| {
        if let Some(r) = r {
            if !out.iter().any(|o| o.eq_ignore_ascii_case(&r)) {
                out.push(r);
            }
        }
    };
    let world = entities.split('}').next().unwrap_or("");
    if let Some(c) = wad.captures(world) {
        for w in c[1].split(';') {
            let name = w.replace('\\', "/").rsplit('/').next().unwrap_or("").trim().to_string();
            if name.to_lowercase().ends_with(".wad") {
                add(clean_rel(&name));
            }
        }
    }
    if let Some(c) = sky.captures(world) {
        let s = c[1].trim();
        if !s.is_empty() && s.len() <= 40 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)) {
            for side in SKY_SIDES {
                add(clean_rel(&format!("gfx/env/{s}{side}.tga")));
            }
        }
    }
    for m in crate::navs::referenced_files(entities) {
        add(clean_rel(&m));
    }
    for c in sound.captures_iter(entities) {
        let p = c[1].replace('\\', "/");
        let p = if p.to_lowercase().starts_with("sound/") { p } else { format!("sound/{p}") };
        add(clean_rel(&p));
    }
    out
}

/// A texture file every server has (Half-Life's and CS 1.6's own).
pub fn stock_wad(rel: &str) -> bool {
    !rel.contains('/') && STOCK_WADS.contains(&rel.to_lowercase().as_str())
}

/// A map in this computer's game.
#[derive(Debug, Clone)]
pub struct LocalMap {
    /// as its .bsp is named
    pub name: String,
    pub bsp: PathBuf,
    /// "cstrike" or "czero": the folder the .bsp is in
    pub game: String,
    /// only in czero/maps (going to a CS 1.6 server)
    pub cz_only: bool,
}

/// The map in this computer's game for a server of `target_game`: its own game's version
/// first (a CS 1.6 server takes the cstrike one, Condition Zero the czero one).
pub fn find_local(f: &Finder, hl: &Path, map: &str, target_game: &str) -> Option<LocalMap> {
    let order: [&str; 2] = if target_game == "czero" { ["czero", "cstrike"] } else { ["cstrike", "czero"] };
    for g in order {
        if let Some(p) = f.find(&hl.join(g), &format!("maps/{map}.bsp")) {
            let name = p.file_stem()?.to_string_lossy().to_string();
            let cz_only = target_game == "cstrike" && g == "czero";
            return Some(LocalMap { name, bsp: p, game: g.into(), cz_only });
        }
    }
    None
}

fn bsp_version(p: &Path) -> Option<u32> {
    let mut b = [0u8; 4];
    std::fs::File::open(p).ok()?.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}

/// The bots' nav for a map: next to its .bsp, else the other game's when it was learned on
/// this same map file (the size inside it matches).
fn find_nav(f: &Finder, hl: &Path, m: &LocalMap) -> Option<PathBuf> {
    let size = std::fs::metadata(&m.bsp).ok()?.len();
    let fit = |p: &Path, any_version: bool| {
        let mut b = Vec::with_capacity(12);
        std::fs::File::open(p).ok()?.take(12).read_to_end(&mut b).ok()?;
        let head = crate::navs::parse_head(&b)?;
        let big = std::fs::metadata(p).ok()?.len() >= crate::navs::MIN_NAV;
        (big && (any_version || head.bsp_size.is_some_and(|b| b as u64 == size))).then_some(())
    };
    let own = m.bsp.with_extension("nav");
    if let Some(p) = f.find(m.bsp.parent()?, &format!("{}.nav", m.name)).or_else(|| own.is_file().then_some(own)) {
        if fit(&p, true).is_some() {
            return Some(p);
        }
    }
    ["cstrike", "czero"]
        .iter()
        .filter(|g| **g != m.game)
        .filter_map(|g| f.find(&hl.join(g), &format!("maps/{}.nav", m.name)))
        .find(|p| fit(p, false).is_some())
}

/// Every file `m` needs that this computer has, as (game path, file): the map and its
/// own files in maps/, its overview, and what its .res file lists and its entities name.
/// Valve's texture files and files only in Half-Life's own folder (valve/, which every
/// server has) are left out. Also the game paths it names that aren't on this computer.
pub fn needed_files(f: &Finder, hl: &Path, m: &LocalMap) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let mut have: Vec<(String, PathBuf)> = vec![(format!("maps/{}.bsp", m.name), m.bsp.clone())];
    let mut missing = Vec::new();
    let maps_dir = m.bsp.parent().map(Path::to_path_buf).unwrap_or_default();
    for ext in ["txt", "res"] {
        if let Some(p) = f.find(&maps_dir, &format!("{}.{ext}", m.name)) {
            have.push((format!("maps/{}.{ext}", m.name), p));
        }
    }
    if let Some(p) = find_nav(f, hl, m) {
        have.push((format!("maps/{}.nav", m.name), p));
    }
    let mut dirs: Vec<PathBuf> = vec![hl.join(&m.game)];
    if m.game != "cstrike" {
        dirs.push(hl.join("cstrike"));
    }
    let valve = hl.join("valve");
    for ext in ["bmp", "tga", "txt"] {
        let rel = format!("overviews/{}.{ext}", m.name);
        if let Some(p) = dirs.iter().find_map(|d| f.find(d, &rel)) {
            have.push((rel, p));
        }
    }
    let mut wanted: Vec<String> = Vec::new();
    if let Some(p) = f.find(&maps_dir, &format!("{}.res", m.name)) {
        wanted.extend(std::fs::read(&p).map(|b| res_files(&crate::rcon::latin1(&b))).unwrap_or_default());
    }
    if let Some(text) = crate::navs::entity_text(&m.bsp) {
        wanted.extend(entity_files(&text));
    }
    for rel in wanted {
        if stock_wad(&rel) || have.iter().any(|(h, _)| h.eq_ignore_ascii_case(&rel)) {
            continue;
        }
        match dirs.iter().find_map(|d| f.find(d, &rel)) {
            Some(p) => have.push((rel, p)),
            // Half-Life's own files: every server has them
            None if f.find(&valve, &rel).is_some() => {}
            None => {
                if !missing.contains(&rel) {
                    missing.push(rel)
                }
            }
        }
    }
    (have, missing)
}

// ---- what the server has, and the plan for one map --------------------------------------------

/// A server as seen from here.
pub struct Target {
    pub name: String,
    pub game: String,
    /// this computer's copy of its folder (hlds and the game folders inside)
    pub dir: PathBuf,
    /// a server on another machine: every file there (lower case, from its folder:
    /// "cstrike/maps/de_dust2.bsp"). None for one on this computer, or when not asked.
    pub machine: Option<HashSet<String>>,
}

impl Target {
    pub fn of(e: &Entry) -> Target {
        Target { name: e.name.clone(), game: e.game.clone(), dir: e.dir.clone(), machine: None }
    }

    /// The game folders the server reads, its own first.
    fn folders(&self) -> Vec<&str> {
        let mut v = vec![self.game.as_str()];
        if self.game != "cstrike" {
            v.push("cstrike");
        }
        v.push("valve");
        v
    }
}

/// What the server has of one file.
#[derive(Debug, PartialEq)]
pub enum There {
    /// nothing: it's copied
    Nothing,
    /// the same file in its folder here
    Same,
    /// a different file in its folder here: the server's is kept
    Different,
    /// one in a folder it falls back on (cstrike, valve) or on its machine: left as it is
    Own,
}

pub fn on_server(f: &Finder, t: &Target, rel: &str, src: &Path) -> There {
    for (i, g) in t.folders().into_iter().enumerate() {
        if let Some(p) = f.find(&t.dir.join(g), rel) {
            return match (i, crate::archive::same_file(src, &p)) {
                (_, true) => There::Same,
                (0, false) => There::Different,
                _ => There::Own,
            };
        }
    }
    if let Some(m) = &t.machine {
        if t.folders().iter().any(|g| m.contains(&format!("{g}/{rel}").to_lowercase())) {
            return There::Own;
        }
    }
    There::Nothing
}

/// What copying one map to one server comes to.
#[derive(Debug, Default)]
pub struct Plan {
    pub map: String,
    /// to copy: (game path, file here)
    pub copy: Vec<(String, PathBuf)>,
    /// the server has these already, the same
    pub same: Vec<String>,
    /// the server has its own, different version: kept
    pub kept: Vec<String>,
    /// named by the map but not on this computer
    pub missing: Vec<String>,
    /// a Condition Zero map going to a CS 1.6 server
    pub cz_only: bool,
    pub bytes: u64,
}

/// Why a map isn't copied.
#[derive(Debug, PartialEq)]
pub enum Skip {
    /// the server has this very map already
    There,
    Why(String),
}

pub fn plan(f: &Finder, hl: &Path, map: &str, t: &Target) -> Result<Plan, Skip> {
    let m = find_local(f, hl, map, &t.game).ok_or_else(|| Skip::Why("it isn't in your game any more".into()))?;
    if bsp_version(&m.bsp) != Some(BSP_VERSION) {
        return Err(Skip::Why("its map file isn't one Counter-Strike servers can load".into()));
    }
    let (files, missing) = needed_files(f, hl, &m);
    let mut p = Plan { map: m.name.clone(), missing, cz_only: m.cz_only, ..Plan::default() };
    for (i, (rel, src)) in files.into_iter().enumerate() {
        let there = on_server(f, t, &rel, &src);
        // the map file itself decides: the server's own version stays, and nothing of
        // ours goes with it (its nav and textures belong to our version)
        if i == 0 {
            match there {
                There::Different => return Err(Skip::Why("the server has a different map by that name".into())),
                There::Own => return Err(Skip::There),
                _ => {}
            }
        }
        match there {
            There::Nothing => {
                p.bytes += std::fs::metadata(&src).map(|x| x.len()).unwrap_or(0);
                p.copy.push((rel, src));
            }
            There::Same => p.same.push(rel),
            There::Different => p.kept.push(rel),
            There::Own => {}
        }
    }
    if p.copy.is_empty() {
        return Err(Skip::There);
    }
    Ok(p)
}

/// Copies a plan's files into the server's game folder here (each whole, never half a
/// file). On a failure what this map had copied so far is taken out again.
pub fn apply(p: &Plan, t: &Target) -> Result<(), String> {
    let base = t.dir.join(&t.game);
    let mut done: Vec<PathBuf> = Vec::new();
    for (rel, src) in &p.copy {
        let dest = base.join(rel);
        let r = (|| {
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d)?;
            }
            crate::content::replace_with(src, &dest)
        })();
        if let Err(e) = r {
            for d in done {
                let _ = std::fs::remove_file(d);
            }
            return Err(format!("{rel} couldn't be copied ({e})"));
        }
        done.push(dest);
    }
    Ok(())
}

// ---- the job ---------------------------------------------------------------------------------

static STOP: AtomicBool = AtomicBool::new(false);
/// How the last run went (for the toast at its end)
static RESULT: Mutex<Option<Value>> = Mutex::new(None);

/// The last run's counts, for diagnostics ("copied 3, already there 1, ...").
pub fn last_summary() -> Option<String> {
    let r = RESULT.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    let n = |k: &str| r[k].as_u64().or_else(|| r[k].as_array().map(|a| a.len() as u64)).unwrap_or(0);
    Some(format!(
        "copied {}, already there {}, skipped {}, failed {}, files kept {}, not uploaded {}{}",
        n("copied"), n("already"), n("skipped"), n("failed"), n("kept"), n("not_uploaded"),
        if r["stopped"].as_bool() == Some(true) { ", stopped" } else { "" }
    ))
}

/// What a run did, by map and server.
#[derive(Default, Debug)]
pub struct Outcome {
    /// (map, server)
    pub copied: Vec<(String, String)>,
    pub already: Vec<(String, String)>,
    /// (map, server, why)
    pub skipped: Vec<(String, String, String)>,
    pub failed: Vec<(String, String, String)>,
    /// (file, server)
    pub kept: Vec<(String, String)>,
    /// Condition Zero maps sent to a CS 1.6 server
    pub cz: Vec<String>,
    /// (map, file) not on this computer
    pub missing: Vec<(String, String)>,
    /// servers whose machine didn't take the upload yet
    pub not_uploaded: Vec<String>,
    pub stopped: bool,
}

fn uniq(v: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for x in v {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What a run did, in a few sentences.
pub fn summary(o: &Outcome, servers: usize) -> String {
    let mut parts = Vec::new();
    if o.stopped {
        parts.push("Stopped.".to_string());
    }
    let maps = |v: &[(String, String)]| uniq(v.iter().map(|(m, _)| m.clone()));
    let on = |v: &[(String, String)]| uniq(v.iter().map(|(_, s)| s.clone()));
    if !o.copied.is_empty() {
        let (m, s) = (maps(&o.copied), on(&o.copied));
        let what = if m.len() <= 3 { crate::navs::names(&m) } else { plural(m.len(), "map", "maps") };
        parts.push(format!("Copied {what} to {}.", crate::navs::names(&s)));
    } else if !o.stopped && o.skipped.is_empty() && o.failed.is_empty() {
        parts.push("Nothing to copy.".to_string());
    }
    if !o.already.is_empty() {
        let n = o.already.len();
        let m = maps(&o.already);
        parts.push(if servers == 1 && m.len() <= 3 {
            format!("{} {} already there.", crate::navs::names(&m), if m.len() == 1 { "was" } else { "were" })
        } else if servers == 1 {
            format!("{} already there.", plural(n, "map was", "maps were"))
        } else {
            format!("{} already there.", plural(n, "copy was", "copies were"))
        });
    }
    for (m, s, why) in o.skipped.iter().take(3) {
        parts.push(format!("Skipped {m}{}: {why}.", if servers > 1 { format!(" on {s}") } else { String::new() }));
    }
    if o.skipped.len() > 3 {
        parts.push(format!("Skipped {} more.", o.skipped.len() - 3));
    }
    for (m, s, why) in o.failed.iter().take(3) {
        parts.push(format!("Couldn't copy {m}{}: {why}.", if servers > 1 { format!(" to {s}") } else { String::new() }));
    }
    if o.failed.len() > 3 {
        parts.push(format!("{} more couldn't be copied.", o.failed.len() - 3));
    }
    if !o.kept.is_empty() {
        let files = uniq(o.kept.iter().map(|(f, _)| f.rsplit('/').next().unwrap_or(f).to_string()));
        parts.push(format!(
            "Kept the server's version of {} (it differs from yours).",
            if files.len() <= 3 { crate::navs::names(&files) } else { plural(files.len(), "file", "files") }
        ));
    }
    if !o.cz.is_empty() {
        let cz = uniq(o.cz.iter().cloned());
        parts.push(format!(
            "{} {} from Condition Zero. {} usually {} on CS 1.6, but check {} on the server.",
            crate::navs::names(&cz),
            if cz.len() == 1 { "is a map" } else { "are maps" },
            if cz.len() == 1 { "It" } else { "They" },
            if cz.len() == 1 { "works" } else { "work" },
            if cz.len() == 1 { "it loads" } else { "they load" }
        ));
    }
    if !o.missing.is_empty() {
        let m = uniq(o.missing.iter().map(|(m, _)| m.clone()));
        parts.push(format!(
            "{} {} files your game doesn't have either, so players may see missing textures or hear nothing there.",
            crate::navs::names(&m),
            if m.len() == 1 { "names" } else { "name" }
        ));
    }
    if !o.not_uploaded.is_empty() {
        parts.push(format!("Couldn't reach {} to upload. The maps go up with the next sync.", crate::navs::names(&o.not_uploaded)));
    }
    parts.join(" ")
}

impl Outcome {
    fn to_json(&self, servers: usize) -> Value {
        json!({
            "message": summary(self, servers),
            "copied": self.copied.len(), "already": self.already.len(),
            "skipped": self.skipped.iter().map(|(m, s, w)| json!({"map": m, "server": s, "why": w})).collect::<Vec<_>>(),
            "failed": self.failed.iter().map(|(m, s, w)| json!({"map": m, "server": s, "why": w})).collect::<Vec<_>>(),
            "kept": self.kept.len(), "not_uploaded": self.not_uploaded.len(), "stopped": self.stopped,
        })
    }
}

/// Every file on a server's machine (its folder there, lower case).
fn machine_files(r: &crate::remote::RemoteInfo, id: &str) -> Result<HashSet<String>, String> {
    crate::remote::list_files(r, &crate::remote::server_dir(id)).map(|s| s.into_iter().map(|f| f.replace('\\', "/").to_lowercase()).collect())
}

/// The server's map record (its Game's store when it has one, so its map list updates).
fn store_for(e: &Entry) -> Arc<MapStore> {
    match servers::game_for(&e.id, || None) {
        Some(g) => g.store.clone(),
        None => Arc::new(MapStore::new_server(&e.dir, &e.dir.join(&e.game), &sys::app_dir(), Arc::new(Mutex::new(None)))),
    }
}

/// The record a map goes under on a server: the GameBanana download it came with (so
/// its author and page carry over), else one of its own.
fn record_for(local: &Map<String, Value>, map: &str, fresh: &mut i64) -> (String, Value) {
    for (id, r) in local {
        let named = r["maps"].as_array().into_iter().flatten().any(|m| m.as_str().is_some_and(|m| m.eq_ignore_ascii_case(map)));
        if named && id.parse::<i64>().is_ok_and(|n| n > 0) {
            return (id.clone(), json!({"name": r["name"], "author": r["author"], "url": r["url"]}));
        }
    }
    *fresh -= 1;
    (fresh.to_string(), json!({"name": map, "author": "", "url": null}))
}

/// Copies one map to one server and records it. Where it went in `out`.
fn copy_one(f: &Finder, hl: &Path, map: &str, t: &Target, store: &MapStore, local: &Map<String, Value>, fresh: &mut i64, out: &mut Outcome) -> bool {
    let p = match plan(f, hl, map, t) {
        Ok(p) => p,
        Err(Skip::There) => {
            out.already.push((map.into(), t.name.clone()));
            return false;
        }
        Err(Skip::Why(w)) => {
            out.skipped.push((map.into(), t.name.clone(), w));
            return false;
        }
    };
    if let Err(e) = apply(&p, t) {
        out.failed.push((p.map.clone(), t.name.clone(), e));
        return false;
    }
    let (key, info) = record_for(local, &p.map, fresh);
    let files: Vec<String> = p.copy.iter().map(|(r, _)| r.clone()).collect();
    let shared: Vec<String> = p.same.iter().chain(p.kept.iter()).cloned().collect();
    if let Err(e) = store.record_copy(&key, &info, &p.map, &files, &shared) {
        // without a record it couldn't be removed the normal way: taken out again
        for (rel, _) in &p.copy {
            let _ = std::fs::remove_file(t.dir.join(&t.game).join(rel));
        }
        out.failed.push((p.map.clone(), t.name.clone(), format!("the server's map list couldn't be saved ({e})")));
        return false;
    }
    out.copied.push((p.map.clone(), t.name.clone()));
    out.kept.extend(p.kept.iter().map(|k| (k.clone(), t.name.clone())));
    if p.cz_only {
        out.cz.push(p.map.clone());
    }
    out.missing.extend(p.missing.iter().map(|m| (p.map.clone(), m.clone())));
    true
}

/// Starts copying `maps` from the game at `hl` to the servers `ids`. Returns the job id.
pub fn start(hl: PathBuf, local: Map<String, Value>, maps: Vec<String>, ids: Vec<String>) -> Result<String, String> {
    let mut seen = HashSet::new();
    let maps: Vec<String> = maps.into_iter().filter(|m| servers::safe_map(m) && seen.insert(m.to_lowercase())).take(MAX_MAPS).collect();
    if maps.is_empty() {
        return Err("Pick the maps to copy.".into());
    }
    let list: Vec<Entry> = servers::load().into_iter().filter(|e| e.ready && ids.contains(&e.id)).collect();
    if list.is_empty() {
        return Err("Pick a server that's set up.".into());
    }
    let n = maps.len();
    let steps: Vec<setup::Step> = list
        .iter()
        .enumerate()
        .map(|(i, e)| setup::server_step(&e.game, &format!("copy-{i}"), format!("Copying maps to {}", e.name), n as f64 + if e.remote.is_some() { (n as f64 / 4.0).max(2.0) } else { 0.0 }))
        .collect();
    STOP.store(false, Ordering::SeqCst);
    *RESULT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let count = list.len();
    setup::start_job(JOB_KIND, steps, move |job| {
        let mut out = Outcome::default();
        let mut fresh = -(sys::now() * 1000.0) as i64;
        for (i, e) in list.iter().enumerate() {
            if STOP.load(Ordering::SeqCst) {
                out.stopped = true;
                break;
            }
            let id = format!("{}/copy-{i}", e.game);
            let weight = n as f64 + if e.remote.is_some() { (n as f64 / 4.0).max(2.0) } else { 0.0 };
            let _ = job.run(&id, || {
                let mut t = Target::of(e);
                if let Some(r) = &e.remote {
                    job.relabel(&id, format!("Checking what {} has", e.name));
                    match machine_files(r, &e.id) {
                        Ok(files) => t.machine = Some(files),
                        Err(_) => {
                            for m in &maps {
                                out.failed.push((m.clone(), e.name.clone(), format!("couldn't reach {}", e.name)));
                            }
                            return Ok(());
                        }
                    }
                }
                let store = store_for(e);
                let finder = Finder::default();
                let mut any = false;
                for (k, m) in maps.iter().enumerate() {
                    if STOP.load(Ordering::SeqCst) {
                        out.stopped = true;
                        break;
                    }
                    job.relabel(&id, format!("Copying {m} to {} ({} of {n})", e.name, k + 1));
                    job.fraction(k as f64 / weight);
                    any |= copy_one(&finder, &hl, m, &t, &store, &local, &mut fresh, &mut out);
                }
                if any {
                    servers::mark_changed(&e.id);
                    if e.remote.is_some() {
                        job.relabel(&id, format!("Uploading maps to {}", e.name));
                        job.fraction(n as f64 / weight);
                        let started = std::time::Instant::now();
                        let last = std::cell::Cell::new(None::<std::time::Instant>);
                        let report = |sent: u64, total: u64| {
                            // a few times a second is plenty
                            if last.get().is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(400)) && sent < total {
                                return;
                            }
                            last.set(Some(std::time::Instant::now()));
                            let (label, transfer) = upload_progress(sent, total, started.elapsed().as_secs_f64());
                            job.relabel(&id, format!("{label} to {}", e.name));
                            job.transfer(Some(transfer));
                            if total > 0 {
                                job.fraction((n as f64 + (weight - n as f64) * sent as f64 / total as f64) / weight);
                            }
                        };
                        let sent = servers::sync_with_progress(e, &report);
                        job.transfer(None);
                        if sent.is_err() {
                            out.not_uploaded.push(e.name.clone());
                        }
                    }
                }
                Ok::<(), String>(())
            });
            job.relabel(&id, format!("Copied maps to {}", e.name));
            if out.stopped {
                break;
            }
        }
        let result = out.to_json(count);
        let text = result["message"].as_str().unwrap_or_default().to_string();
        *RESULT.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        if !out.stopped && out.copied.is_empty() && !out.failed.is_empty() {
            return Err(text);
        }
        Ok(())
    })
}

/// The upload's label ("Uploading 280 MB of 970 MB (4 MB/s)") and the job step's
/// transfer: {sent, total, rate, left} (bytes, bytes, bytes a second, seconds; rate and
/// left are null until there's a measure).
pub fn upload_progress(sent: u64, total: u64, secs: f64) -> (String, Value) {
    let mb = |b: u64| (b as f64 / 1_000_000.0).round() as u64;
    let rate = (secs >= 1.0 && sent > 0).then(|| sent as f64 / secs);
    let left = rate.filter(|r| *r > 0.0).map(|r| ((total.saturating_sub(sent)) as f64 / r).ceil() as u64);
    let speed = match rate {
        Some(r) if r >= 1_000_000.0 => format!(" ({:.0} MB/s)", r / 1_000_000.0),
        Some(r) => format!(" ({:.0} KB/s)", (r / 1000.0).max(1.0)),
        None => String::new(),
    };
    (format!("Uploading {} MB of {} MB{speed}", mb(sent), mb(total)), json!({"sent": sent, "total": total, "rate": rate.map(|r| r.round() as u64), "left": left}))
}

// ---- what the pickers show ----------------------------------------------------------------

/// Your game's maps a server doesn't have, with the files each would bring (for the
/// size: a texture file two maps share counts once, so the page adds them up itself).
fn local_for_server(local: &crate::game::Game, e: &Entry) -> Value {
    let server_maps: HashSet<String> = servers::game_for(&e.id, || None)
        .map(|g| g.own_maps(false).iter().filter_map(|m| m["name"].as_str().map(|n| n.to_lowercase())).collect())
        .unwrap_or_default();
    let t = Target::of(e);
    let f = Finder::default();
    let mut out = Vec::new();
    for mut m in local.maps(false) {
        let name = m["name"].as_str().unwrap_or_default().to_string();
        let low = name.to_lowercase();
        // every server has CS 1.6's own maps
        if server_maps.contains(&low) || m["hidden"] == json!(true) || crate::maps::STOCK_CS.contains(&low.as_str()) || !servers::safe_map(&name) {
            continue;
        }
        // a Condition Zero only map isn't offered to a 1.6 server, which can't load it (a
        // Condition Zero server takes 1.6's maps too: it plays them)
        let Some(lm) = find_local(&f, &local.hl, &name, &e.game) else { continue };
        if e.game == "cstrike" && lm.game != "cstrike" {
            continue;
        }
        let Ok(p) = plan(&f, &local.hl, &name, &t) else { continue };
        let files: Vec<Value> = p.copy.iter().map(|(rel, src)| json!([rel.to_lowercase(), std::fs::metadata(src).map(|x| x.len()).unwrap_or(0)])).collect();
        m["files"] = json!(files);
        m["czOnly"] = json!(p.cz_only);
        out.push(m);
    }
    json!({"maps": out, "game": e.game, "remote": e.remote.is_some()})
}

/// The servers a map could go to, and which have it already.
fn servers_for_map(local: &crate::game::Game, map: &str) -> Value {
    let f = Finder::default();
    let list: Vec<Value> = servers::load()
        .into_iter()
        .filter(|e| e.ready)
        .map(|e| {
            let has = servers::game_for(&e.id, || None).is_some_and(|g| g.own_maps(false).iter().any(|m| m["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(map))));
            let found = find_local(&f, &local.hl, map, &e.game);
            json!({"id": e.id, "name": e.name, "game": e.game, "remote": e.remote.is_some(), "has": has,
                   "czOnly": found.as_ref().is_some_and(|m| m.cz_only), "missing": found.is_none()})
        })
        .collect();
    json!({"servers": list})
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
const NO_GAME: &str = "Set up the game on this computer first.";

#[derive(Deserialize)]
struct CopyReq {
    maps: Vec<String>,
    servers: Vec<String>,
}

async fn api_copy(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<CopyReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(game) = st.local_game() else { return answer(Err(NO_GAME.into())) };
    let r = tokio::task::spawn_blocking(move || start(game.hl.clone(), game.store.records(), req.maps, req.servers).map(|job| json!({"ok": true, "job": job})))
        .await
        .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

async fn api_stop(headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let job = setup::current();
    if job.kind != JOB_KIND || job.state != "running" {
        return answer(Err("No maps are being copied right now.".into()));
    }
    STOP.store(true, Ordering::SeqCst);
    answer(Ok(json!({"ok": true})))
}

async fn api_result() -> Json<Value> {
    Json(json!({"result": RESULT.lock().unwrap_or_else(|e| e.into_inner()).clone()}))
}

async fn api_local(State(st): State<Shared>, Query(q): Query<HashMap<String, String>>) -> (StatusCode, Json<Value>) {
    let Some(game) = st.local_game() else { return answer(Err(NO_GAME.into())) };
    let id = q.get("server").cloned().unwrap_or_default();
    let r = tokio::task::spawn_blocking(move || {
        let e = servers::get(&id).filter(|e| e.ready).ok_or("That server isn't there any more, or isn't set up yet.")?;
        Ok(local_for_server(&game, &e))
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

async fn api_servers(State(st): State<Shared>, Query(q): Query<HashMap<String, String>>) -> (StatusCode, Json<Value>) {
    let Some(game) = st.local_game() else { return answer(Err(NO_GAME.into())) };
    let map = q.get("map").cloned().unwrap_or_default();
    if !servers::safe_map(&map) {
        return answer(Err("That isn't a map name.".into()));
    }
    let r = tokio::task::spawn_blocking(move || Ok(servers_for_map(&game, &map))).await.unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

/// The app window's listener only (merged with the other computer-only routes).
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/mapcopy", route_get(api_result).post(api_copy))
        .route("/api/mapcopy/stop", post(api_stop))
        .route("/api/mapcopy/local", route_get(api_local))
        .route("/api/mapcopy/servers", route_get(api_servers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), tag).unwrap()
    }

    fn put(root: &Path, rel: &str, data: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }

    /// A tiny map file: version 30, the entity lump right after the 15-lump header.
    fn bsp(entities: &str) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&BSP_VERSION.to_le_bytes());
        b.extend_from_slice(&124u32.to_le_bytes());
        b.extend_from_slice(&(entities.len() as u32).to_le_bytes());
        b.resize(124, 0);
        b.extend_from_slice(entities.as_bytes());
        b
    }

    fn nav(bsp_size: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0xFEED_FACEu32.to_le_bytes());
        b.extend_from_slice(&5u32.to_le_bytes());
        b.extend_from_slice(&bsp_size.to_le_bytes());
        b.resize(2048, 7);
        b
    }

    const ENTS: &str = r#"{
"classname" "worldspawn"
"wad" "\half-life\valve\halflife.wad;\half-life\cstrike\cs_dust.wad;C:\maps\wads\MyTextures.wad;\x\..\evil.wad"
"skyname" "mysky"
}
{ "classname" "ambient_generic" "message" "ambience/myloop.wav" }
{ "classname" "ambient_generic" "message" "*sound/music/track.mp3" }
{ "classname" "env_sprite" "model" "sprites/myglow.spr" }
{ "classname" "cycler" "model" "models/props/barrel.mdl" }
{ "classname" "func_door" "noise1" "doors/doormove1.wav" }
{ "classname" "trigger_once" "target" "!HG_ALERT" }"#;

    #[test]
    fn entity_lump_names_what_the_map_needs() {
        let f = entity_files(ENTS);
        assert!(f.contains(&"halflife.wad".to_string()) && f.contains(&"cs_dust.wad".to_string()) && f.contains(&"MyTextures.wad".to_string()));
        assert!(f.contains(&"evil.wad".to_string()), "just the file name, at the top of the game folder");
        for side in SKY_SIDES {
            assert!(f.contains(&format!("gfx/env/mysky{side}.tga")));
        }
        for want in ["sound/ambience/myloop.wav", "sound/music/track.mp3", "sprites/myglow.spr", "models/props/barrel.mdl", "sound/doors/doormove1.wav"] {
            assert!(f.contains(&want.to_string()), "{want} in {f:?}");
        }
        assert!(stock_wad("halflife.wad") && stock_wad("CS_DUST.WAD") && !stock_wad("MyTextures.wad") && !stock_wad("maps/halflife.wad"));
        assert_eq!(res_files("// made by\n\"maps/x.txt\"\nsound\\a\\b.wav // loop\n../../etc/passwd\nC:/x.wad\nfoo.exe\n\nmy.wad"), ["maps/x.txt", "sound/a/b.wav", "my.wad"]);
        assert_eq!(clean_rel("dlls/mp.dll"), None);
        assert_eq!(clean_rel("models/../../x.mdl"), None);
    }

    /// A game with one custom map: everything it needs is found, Valve's stuff is left out.
    fn game_with_map(root: &Path) -> PathBuf {
        let hl = root.join("hl");
        let map = bsp(ENTS);
        put(&hl, "cstrike/maps/de_test.bsp", &map);
        put(&hl, "cstrike/maps/de_test.txt", b"about");
        put(&hl, "cstrike/maps/de_test.nav", &nav(map.len() as u32));
        put(&hl, "cstrike/overviews/de_test.bmp", b"radar");
        put(&hl, "cstrike/mytextures.wad", b"WAD3 mine");
        put(&hl, "cstrike/cs_dust.wad", b"WAD3 valve");
        put(&hl, "valve/halflife.wad", b"WAD3 hl");
        put(&hl, "valve/sound/doors/doormove1.wav", b"RIFF door");
        put(&hl, "cstrike/gfx/env/myskyup.tga", b"tga");
        put(&hl, "cstrike/sound/ambience/myloop.wav", b"RIFF loop");
        put(&hl, "cstrike/sprites/myglow.spr", b"IDSP");
        put(&hl, "cstrike/models/props/barrel.mdl", b"IDST");
        hl
    }

    #[test]
    fn finds_every_file_a_map_needs() {
        let root = temp("mapcopy-find-");
        let hl = game_with_map(&root);
        let f = Finder::default();
        let m = find_local(&f, &hl, "DE_TEST", "cstrike").unwrap();
        assert_eq!(m.name, "de_test");
        let (files, missing) = needed_files(&f, &hl, &m);
        let rels: Vec<&str> = files.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(rels[0], "maps/de_test.bsp", "the map first");
        for want in ["maps/de_test.txt", "maps/de_test.nav", "overviews/de_test.bmp", "MyTextures.wad", "gfx/env/myskyup.tga", "sound/ambience/myloop.wav", "sprites/myglow.spr", "models/props/barrel.mdl"] {
            assert!(rels.contains(&want), "{want} in {rels:?}");
        }
        assert!(files.iter().any(|(r, p)| r == "MyTextures.wad" && p.ends_with("mytextures.wad")), "found whatever its case");
        for not in ["halflife.wad", "cs_dust.wad", "sound/doors/doormove1.wav"] {
            assert!(!rels.contains(&not), "{not}: every server has it");
        }
        assert!(missing.contains(&"gfx/env/myskydn.tga".to_string()) && missing.contains(&"evil.wad".to_string()));
        assert!(!missing.contains(&"sound/doors/doormove1.wav".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_what_the_server_has_and_keeps_its_own() {
        let root = temp("mapcopy-plan-");
        let hl = game_with_map(&root);
        let srv = root.join("srv");
        // the server has the same model, and its own (different) sprite
        put(&srv, "cstrike/models/props/barrel.mdl", b"IDST");
        put(&srv, "cstrike/sprites/myglow.spr", b"IDSP theirs");
        let mut t = Target { name: "Alone Together".into(), game: "cstrike".into(), dir: srv.clone(), machine: None };
        let f = Finder::default();
        let p = plan(&f, &hl, "de_test", &t).unwrap();
        let copy: Vec<&str> = p.copy.iter().map(|(r, _)| r.as_str()).collect();
        assert!(copy.contains(&"maps/de_test.bsp") && copy.contains(&"MyTextures.wad"));
        assert!(!copy.contains(&"models/props/barrel.mdl") && !copy.contains(&"sprites/myglow.spr"));
        assert_eq!(p.same, ["models/props/barrel.mdl"]);
        assert_eq!(p.kept, ["sprites/myglow.spr"]);
        assert!(!p.cz_only);
        apply(&p, &t).unwrap();
        assert!(srv.join("cstrike/maps/de_test.bsp").is_file() && srv.join("cstrike/MyTextures.wad").is_file());
        assert_eq!(std::fs::read(srv.join("cstrike/sprites/myglow.spr")).unwrap(), b"IDSP theirs", "never replaced");

        // a second time: all there
        let f = Finder::default();
        assert_eq!(plan(&f, &hl, "de_test", &t).unwrap_err(), Skip::There);

        // a different map by the same name on the server: left alone
        let other = root.join("other");
        put(&other, "cstrike/maps/de_test.bsp", &bsp("{ \"classname\" \"worldspawn\" }"));
        let t2 = Target { name: "B".into(), game: "cstrike".into(), dir: other, machine: None };
        assert!(matches!(plan(&Finder::default(), &hl, "de_test", &t2), Err(Skip::Why(_))));

        // on another machine: what's there (not in this computer's copy) isn't sent
        let empty = root.join("remote");
        std::fs::create_dir_all(&empty).unwrap();
        t.dir = empty;
        t.machine = Some(["cstrike/mytextures.wad".to_string()].into_iter().collect());
        let p = plan(&Finder::default(), &hl, "de_test", &t).unwrap();
        assert!(!p.copy.iter().any(|(r, _)| r == "MyTextures.wad"));
        t.machine = Some(["cstrike/maps/de_test.bsp".to_string()].into_iter().collect());
        assert_eq!(plan(&Finder::default(), &hl, "de_test", &t).unwrap_err(), Skip::There);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn condition_zero_maps_and_records() {
        let root = temp("mapcopy-cz-");
        let hl = root.join("hl");
        put(&hl, "czero/maps/de_czonly.bsp", &bsp("{ \"classname\" \"worldspawn\" \"wad\" \"cz_special.wad\" }"));
        put(&hl, "czero/cz_special.wad", b"WAD3 cz");
        put(&hl, "cstrike/maps/de_bad.bsp", b"VBSP not goldsrc");
        let srv = root.join("srv");
        std::fs::create_dir_all(srv.join("cstrike")).unwrap();
        let t = Target { name: "S".into(), game: "cstrike".into(), dir: srv.clone(), machine: None };
        let p = plan(&Finder::default(), &hl, "de_czonly", &t).unwrap();
        assert!(p.cz_only);
        assert!(p.copy.iter().any(|(r, _)| r == "cz_special.wad"), "CZ's textures go with it to a CS 1.6 server");
        assert!(matches!(plan(&Finder::default(), &hl, "de_bad", &t), Err(Skip::Why(_))));
        assert!(matches!(plan(&Finder::default(), &hl, "de_none", &t), Err(Skip::Why(_))));

        // copied and recorded: listed as the server's, and removable
        let store = MapStore::new_server(&srv, &srv.join("cstrike"), &root.join("app"), Arc::new(Mutex::new(None)));
        let mut out = Outcome::default();
        let mut fresh = -1000;
        let local: Map<String, Value> = serde_json::from_value(json!({"77": {"name": "Pack", "author": "Someone", "url": "u", "maps": ["de_czonly", "de_x"]}})).unwrap();
        assert!(copy_one(&Finder::default(), &hl, "de_czonly", &t, &store, &local, &mut fresh, &mut out));
        let rec = store.records();
        assert_eq!(rec["77"]["author"], "Someone");
        assert_eq!(rec["77"]["maps"], json!(["de_czonly"]));
        assert!(rec["77"]["files"].as_array().unwrap().contains(&json!("cz_special.wad")));
        assert_eq!(out.cz, ["de_czonly"]);
        assert!(!copy_one(&Finder::default(), &hl, "de_czonly", &t, &store, &local, &mut fresh, &mut out));
        assert_eq!(out.already.len(), 1);
        store.uninstall_section("maps", 77).unwrap();
        assert!(!srv.join("cstrike/maps/de_czonly.bsp").exists() && !srv.join("cstrike/cz_special.wad").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn upload_progress_label() {
        let (l, t) = upload_progress(280_000_000, 970_000_000, 70.0);
        assert_eq!(l, "Uploading 280 MB of 970 MB (4 MB/s)");
        assert_eq!(t["sent"], 280_000_000u64);
        assert_eq!(t["rate"], 4_000_000u64);
        assert_eq!(t["left"], 173u64);
        let (l, t) = upload_progress(0, 10_000_000, 0.2);
        assert_eq!(l, "Uploading 0 MB of 10 MB");
        assert!(t["rate"].is_null() && t["left"].is_null());
    }

    #[test]
    fn summaries() {
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        let mut o = Outcome { copied: vec![s("de_a", "Alone Together")], ..Outcome::default() };
        assert_eq!(summary(&o, 1), "Copied de_a to Alone Together.");
        o.already.push(s("de_b", "Alone Together"));
        o.kept.push(s("sprites/x.spr", "Alone Together"));
        o.skipped.push(("de_c".into(), "Alone Together".into(), "the server has a different map by that name".into()));
        assert_eq!(
            summary(&o, 1),
            "Copied de_a to Alone Together. de_b was already there. Skipped de_c: the server has a different map by that name. Kept the server's version of x.spr (it differs from yours)."
        );
        let o = Outcome { stopped: true, ..Outcome::default() };
        assert_eq!(summary(&o, 2), "Stopped.");
        let o = Outcome { already: vec![s("de_a", "A"), s("de_a", "B")], ..Outcome::default() };
        assert_eq!(summary(&o, 2), "Nothing to copy. 2 copies were already there.");
        for t in [summary(&o, 2)] {
            assert!(!t.contains('!') && !t.contains('—'));
        }
    }
}
