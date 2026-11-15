//! Compiling the map editor's document into a playable .bsp with SDHLT, and putting the
//! result into the game for a playtest (`/api/tools/compile/*`, the app window only).
//!
//! The compiler is seedee's SDHLT (github.com/seedee/SDHLT), pinned to v1.3.1. Its licence
//! (GPL v2 with the Valve SDK 2.3 EULA overriding where needed) means it's never bundled: it's
//! downloaded from the official GitHub release the first time someone compiles, after they
//! agree, checked against a pinned SHA-256, and kept with its LICENSE.md in
//! `<app dir>/tools/sdhlt/v1.3.1/`. It's run as separate programs, unmodified.
//!
//! The tools are Windows programs. On Windows they run directly; on a Mac they run through
//! CrossOver's wine in the game's bottle, the way the app runs HLDS: wine only sees the
//! bottle's C: drive, so the tools are copied to `C:\Loner-Strike\Compiler\` and the map is
//! compiled in `C:\Loner-Strike\Compiler\work\<map>\` (on Windows: `<app dir>/tools/compile/work`).
//!
//! One job at a time (a download or a compile), run on its own thread, stoppable, its output
//! kept for the page to stream (`progress?from=N`). The output becomes plain-language problems
//! (leaks with the leak line, BSP limits, missing textures and WADs, broken brushes) that point
//! back at the editor's brushes and entities.
//!
//! Errors are `{error}` with status 200 like the rest of the app; POSTs need the panel header.

use crate::editor::doc::{BrushId, EntityId, MapDocument};
use crate::editor::geom::brush_bounds;
use crate::editor::mapfile;
use crate::editor::math::{v3, Bounds, Vec3};
use crate::editor::ops;
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

// ---- the pinned compiler ----------------------------------------------------------------

pub const SDHLT_VERSION: &str = "1.3.1";
pub const SDHLT_URL: &str = "https://github.com/seedee/SDHLT/releases/download/v1.3.1/sdhlt-v1.3.1.zip";
/// SHA-256 of sdhlt-v1.3.1.zip as published on 2026-10-16 (checked 2026-11-12).
pub const SDHLT_SHA256: &str = "d206ddb73dc0d37f577b70b7e2503277a5092e7ab3e5b198a949fb49273ea8ea";
pub const SDHLT_BYTES: u64 = 3_030_448;
/// The folder everything is in inside the zip.
const ZIP_ROOT: &str = "sdhlt-v1.3.1";
const LICENCE: &str = "SDHLT is free software under the GPL v2, with the Valve SDK 2.3 license taking priority where needed. It isn't part of Loner-Strike: it's downloaded from its official GitHub release and kept with its license (LICENSE.md).";

const INTERNAL: &str = "Loner-Strike hit an internal error with the map compiler. Try again, and copy the diagnostics from Settings if it keeps happening.";
const NO_GAME: &str = "Set up the game on this computer first: the compiler needs its texture files.";
const NO_DOC: &str = "No map is open in the editor.";
const BUSY: &str = "The compiler is already busy. Wait for it to finish, or stop it.";
const NEEDS_SDHLT: &str = "Compiling maps needs SDHLT, a free GoldSrc map compiler. Download it first.";

/// Most log lines kept (the page streams them; the start goes first).
const MAX_LOG: usize = 20_000;
const MAX_LINE: usize = 400;
/// Most of one stage's output kept for reading problems.
const MAX_STAGE_TEXT: usize = 8 << 20;
const MAX_PROBLEMS: usize = 200;
/// Most points sent for a leak line.
const MAX_LEAK_POINTS: usize = 4000;
/// Backups kept per map when a playtest replaces it.
const KEEP_BACKUPS: usize = 5;

// ---- where things go ----------------------------------------------------------------------

/// `<app dir>/tools/sdhlt/v1.3.1`: the unpacked release (tools/, LICENSE.md, README.md).
pub fn install_dir() -> PathBuf {
    crate::localdata::dir("tools/sdhlt").join(format!("v{SDHLT_VERSION}"))
}

fn compile_dir() -> PathBuf {
    crate::localdata::dir("tools/compile")
}

// ---- which compile each document ran last (Parity A, QA4-A A-1) ---------------------------
// The Budget's "last compile" must be this document's, whatever name it was compiled under (the
// Compile panel's name field), never an older log left by another map of the same name.

const LAST_MAX: usize = 200;

fn last_compiles_file() -> PathBuf {
    compile_dir().join("last-compiles.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Notes that the document `uid` is being compiled as `name`, its log at `log`. A log path
/// another document compiled to before is that document's no more (the new compile overwrites it).
fn note_compile_in(file: &Path, uid: &str, name: &str, log: &Path, started: u64) {
    let mut list: Vec<Value> = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let log_s = log.to_string_lossy().to_string();
    list.retain(|e| e["uid"] != uid && e["log"] != log_s.as_str());
    list.push(json!({"uid": uid, "name": name, "log": log_s, "started": started}));
    if list.len() > LAST_MAX {
        list.drain(..list.len() - LAST_MAX);
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = crate::sys::write_atomic(file, serde_json::to_string(&list).unwrap_or_default().as_bytes());
}

/// The log of the compile the document `uid` ran last, and the name it was compiled as: None
/// when it has none, or the log is gone or older than that compile.
fn last_compile_in(file: &Path, uid: &str) -> Option<(PathBuf, String)> {
    let list: Vec<Value> = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok())?;
    let e = list.iter().rev().find(|e| e["uid"] == uid)?;
    let log = PathBuf::from(e["log"].as_str()?);
    let modified = std::fs::metadata(&log).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    // (a second's slack: file times are rounded on some file systems)
    (modified + 1 >= e["started"].as_u64()?).then(|| (log, e["name"].as_str().unwrap_or_default().to_string()))
}

/// The compile the document `uid` ran last (its log and the name it was compiled as).
pub(crate) fn last_compile_of(uid: &str) -> Option<(PathBuf, String)> {
    last_compile_in(&last_compiles_file(), uid)
}

const MARKER: &str = "loner-strike-verified.json";

/// The programs and files a compile needs, relative to the install folder.
fn required_files() -> Vec<String> {
    let mut v = vec!["LICENSE.md".to_string(), "tools/sdhlt.wad".into(), "tools/settings.txt".into(), "tools/lights.rad".into()];
    for s in Stage::ALL {
        v.push(format!("tools/Win64/{}_x64.exe", s.program()));
        v.push(format!("tools/Win32/{}.exe", s.program()));
    }
    v
}

/// Whether the pinned release is unpacked in `dir`, checked (its marker names the pinned sum).
pub fn is_installed(dir: &Path) -> bool {
    let marked = std::fs::read_to_string(dir.join(MARKER)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|v| v["sha256"] == SDHLT_SHA256);
    marked && required_files().iter().all(|f| dir.join(f).is_file())
}

/// Whether a file's SHA-256 is the expected one.
pub fn checksum_ok(path: &Path, expected: &str) -> bool {
    crate::setup::fetch::sha256(path).map(|h| h.eq_ignore_ascii_case(expected)).unwrap_or(false)
}

/// Unpacks a downloaded release zip (already checked) into `dir`: the zip's own top folder
/// is dropped, the Linux builds aren't kept, and a marker says it was checked.
pub fn install_zip(zip: &Path, dir: &Path) -> Result<(), String> {
    if !checksum_ok(zip, SDHLT_SHA256) {
        return Err("The SDHLT download isn't the file Loner-Strike was tested with, so it wasn't used. Try again later.".into());
    }
    let parent = dir.parent().ok_or(INTERNAL)?;
    let tmp = parent.join(format!(".unpack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::setup::fetch::unpack(zip, &tmp)?;
    let root = tmp.join(ZIP_ROOT);
    let from = if root.is_dir() { root } else { tmp.clone() };
    let _ = std::fs::remove_dir_all(from.join("tools").join("Linux"));
    let _ = std::fs::remove_dir_all(from.join("media"));
    let _ = std::fs::remove_dir_all(dir);
    std::fs::rename(&from, dir).map_err(|e| format!("Couldn't put SDHLT in place ({e})."))?;
    let _ = std::fs::remove_dir_all(&tmp);
    let marker = json!({"version": SDHLT_VERSION, "sha256": SDHLT_SHA256, "from": SDHLT_URL});
    std::fs::write(dir.join(MARKER), marker.to_string()).map_err(|e| format!("Couldn't finish installing SDHLT ({e})."))?;
    if !is_installed(dir) {
        return Err("The SDHLT download is missing some of its programs, so it wasn't used.".into());
    }
    Ok(())
}

// ---- stages, presets, command lines -------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Csg,
    Bsp,
    Vis,
    Rad,
}

impl Stage {
    pub const ALL: [Stage; 4] = [Stage::Csg, Stage::Bsp, Stage::Vis, Stage::Rad];
    pub fn program(self) -> &'static str {
        match self {
            Stage::Csg => "sdHLCSG",
            Stage::Bsp => "sdHLBSP",
            Stage::Vis => "sdHLVIS",
            Stage::Rad => "sdHLRAD",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Stage::Csg => "csg",
            Stage::Bsp => "bsp",
            Stage::Vis => "vis",
            Stage::Rad => "rad",
        }
    }
    /// What the page says while it runs.
    pub fn label(self) -> &'static str {
        match self {
            Stage::Csg => "Compiling brushes…",
            Stage::Bsp => "Building the map…",
            Stage::Vis => "Working out what can be seen from where…",
            Stage::Rad => "Lighting…",
        }
    }
    /// How much of the whole compile it usually is.
    fn weight(self) -> f64 {
        match self {
            Stage::Csg => 1.0,
            Stage::Bsp => 1.0,
            Stage::Vis => 2.0,
            Stage::Rad => 5.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    /// No visibility pass, quick lighting: to try something out
    Fast,
    Normal,
    /// Full visibility and extra lighting: for release
    Final,
}

impl Preset {
    pub fn stages(self) -> Vec<Stage> {
        match self {
            Preset::Fast => vec![Stage::Csg, Stage::Bsp, Stage::Rad],
            _ => Stage::ALL.to_vec(),
        }
    }
    pub fn flags(self, stage: Stage) -> Vec<&'static str> {
        match (self, stage) {
            (Preset::Final, Stage::Vis) => vec!["-full"],
            (Preset::Fast, Stage::Rad) => vec!["-fast"],
            (Preset::Final, Stage::Rad) => vec!["-extra"],
            _ => vec![],
        }
    }
}

/// How the Windows programs are run here.
#[derive(Clone, Debug, PartialEq)]
pub enum Runner {
    /// Windows: directly
    Native,
    /// macOS: through CrossOver's wine in a bottle (paths given as C:\...)
    Wine { wine: PathBuf, bottle: String },
}

/// Where the tools are and where the map is compiled, for this computer.
#[derive(Clone, Debug)]
pub struct Layout {
    pub runner: Runner,
    /// the release's tools folder (Win32/, Win64/, sdhlt.wad); on a Mac, the copy in the bottle
    pub tools: PathBuf,
    /// compiles happen in work/<map>/
    pub work: PathBuf,
    pub x64: bool,
    /// how many threads each tool may use (`-threads N`); None: the tool's own choice (all cores)
    pub threads: Option<usize>,
}

/// How many threads the compiler gets: all cores when asked, else all but two (so the
/// computer stays usable while it lights the map), at least one.
pub fn compile_threads(cores: usize, all_cores: bool) -> usize {
    let cores = cores.max(1);
    if all_cores {
        cores
    } else {
        cores.saturating_sub(2).max(1)
    }
}

/// This computer's cores.
pub fn cores() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// The compiler runs below normal priority: on Unix (macOS through CrossOver's wine, which
/// passes it on to the Windows tool) a nice value of 10 set on the child before it starts; on
/// Windows BELOW_NORMAL_PRIORITY_CLASS (plus no console window). SDHLT's own `-low` is passed too.
pub const NICE: i32 = 10;
#[cfg(any(windows, test))]
pub const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(unix)]
fn low_priority(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setpriority only changes the new process's own scheduling, between fork and exec
    unsafe {
        cmd.pre_exec(|| {
            libc::setpriority(libc::PRIO_PROCESS, 0, NICE);
            Ok(())
        });
    }
}

#[cfg(windows)]
fn low_priority(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
}

/// .../Bottles/<bottle>/drive_c/rest -> C:\rest
fn c_drive_path(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    let i = s.find("/drive_c/")?;
    Some(format!("C:\\{}", s[i + 9..].trim_end_matches('/').replace('/', "\\")))
}

impl Layout {
    /// A path as the tools must be given it.
    pub fn arg_path(&self, p: &Path) -> Result<String, String> {
        match self.runner {
            Runner::Native => Ok(p.to_string_lossy().to_string()),
            Runner::Wine { .. } => c_drive_path(p).ok_or_else(|| format!("{} isn't inside the game's CrossOver bottle, so the compiler can't reach it.", p.display())),
        }
    }

    pub fn exe(&self, stage: Stage) -> PathBuf {
        if self.x64 {
            self.tools.join("Win64").join(format!("{}_x64.exe", stage.program()))
        } else {
            self.tools.join("Win32").join(format!("{}.exe", stage.program()))
        }
    }

    /// The program and its arguments for one stage on `map` (the .map's path without the
    /// extension). `extra` are added after the preset's flags (CSG's -wadinclude list).
    pub fn command_line(&self, stage: Stage, preset: Preset, map: &Path, extra: &[String]) -> Result<(PathBuf, Vec<String>), String> {
        let exe = self.exe(stage);
        let mut args = Vec::new();
        let program = match &self.runner {
            Runner::Native => exe.clone(),
            Runner::Wine { wine, bottle } => {
                args.extend(["--bottle".to_string(), bottle.clone(), "--wait-children".into(), self.arg_path(&exe)?]);
                wine.clone()
            }
        };
        // below normal priority inside the tool too, and a thread cap
        args.push("-low".into());
        if let Some(n) = self.threads {
            args.extend(["-threads".to_string(), n.max(1).to_string()]);
        }
        args.extend(preset.flags(stage).iter().map(|s| s.to_string()));
        args.extend(extra.iter().cloned());
        // the map comes last (SDHLT reads its options first)
        args.push(self.arg_path(map)?);
        Ok((program, args))
    }
}

/// Whether a CrossOver bottle is 64-bit (cxbottle.conf's "WineArch"; new bottles are).
fn bottle_is_64(bottle_dir: &Path) -> bool {
    match std::fs::read_to_string(bottle_dir.join("cxbottle.conf")) {
        Ok(t) => !t.lines().any(|l| {
            let l = l.trim();
            !l.starts_with(';') && l.contains("\"WineArch\"") && l.contains("win32")
        }),
        Err(_) => true,
    }
}

/// The layout for the player's game install, with the tools from `install` (copied into the
/// bottle on a Mac when they aren't there yet).
pub fn layout_for(client: &crate::steam::Install, install: &Path, copy: bool) -> Result<Layout, String> {
    if client.kind == "crossover" {
        let wine = PathBuf::from(crate::servers::CROSSOVER_WINE);
        if !wine.is_file() {
            return Err("CrossOver isn't installed in Applications. The compiler runs through it, like the game.".into());
        }
        let s = client.hl.to_string_lossy().to_string();
        let i = s.find("/drive_c/").ok_or("The game isn't in a CrossOver bottle.")?;
        let drive_c = PathBuf::from(&s[..i + 8]);
        let bottle_dir = drive_c.parent().ok_or("The game isn't in a CrossOver bottle.")?.to_path_buf();
        let base = drive_c.join("Loner-Strike").join("Compiler");
        let tools = base.join(format!("sdhlt-v{SDHLT_VERSION}")).join("tools");
        if copy {
            mirror_tools(&install.join("tools"), &tools)?;
        }
        Ok(Layout { runner: Runner::Wine { wine, bottle: client.bottle.clone().unwrap_or_else(|| "Steam".into()) }, tools, work: base.join("work"), x64: bottle_is_64(&bottle_dir), threads: None })
    } else if cfg!(windows) {
        Ok(Layout { runner: Runner::Native, tools: install.join("tools"), work: compile_dir().join("work"), x64: std::env::consts::ARCH != "x86", threads: None })
    } else {
        Err("Compiling maps works on Windows, and on a Mac with CrossOver.".into())
    }
}

/// Copies the tools folder (not the Linux builds) where wine can see it; files already there
/// with the same size are left alone.
fn mirror_tools(from: &Path, to: &Path) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("Couldn't copy the compiler into the game's bottle ({e}).");
    std::fs::create_dir_all(to).map_err(fail)?;
    for entry in std::fs::read_dir(from).map_err(fail)?.flatten() {
        let name = entry.file_name();
        let (src, dest) = (entry.path(), to.join(&name));
        if src.is_dir() {
            if name != "Linux" {
                mirror_tools(&src, &dest)?;
            }
        } else if std::fs::metadata(&dest).map(|m| m.len()).ok() != std::fs::metadata(&src).map(|m| m.len()).ok() {
            std::fs::copy(&src, &dest).map_err(fail)?;
        }
    }
    Ok(())
}

// ---- the .map the compiler reads --------------------------------------------------------

/// Which editor entity and brushes each `.map` entity is, in file order (entity 0 is the
/// world), so "Entity 3, Brush 2" in the compiler's output points back into the editor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Index {
    pub entities: Vec<(Option<EntityId>, Vec<BrushId>)>,
}

impl Index {
    /// The same order `mapfile::write_map` writes (groups flattened).
    pub fn of(doc: &MapDocument) -> Index {
        let owner = |b: &crate::editor::doc::Brush| b.entity.filter(|e| doc.entities.contains_key(e));
        let mut entities = vec![(None, doc.brushes.iter().filter(|(_, b)| owner(b).is_none()).map(|(id, _)| *id).collect())];
        for id in doc.entities.keys() {
            entities.push((Some(*id), doc.brushes.iter().filter(|(_, b)| owner(b) == Some(*id)).map(|(bid, _)| *bid).collect()));
        }
        Index { entities }
    }
    pub fn brush(&self, entity: usize, brush: usize) -> Option<BrushId> {
        self.entities.get(entity).and_then(|(_, b)| b.get(brush).copied())
    }
    pub fn entity(&self, entity: usize) -> Option<EntityId> {
        self.entities.get(entity).and_then(|(e, _)| *e)
    }
}

/// A box to compile on its own (Hammer's cordon), in GoldSrc units.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Cordon {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Cordon {
    fn check(&self) -> Result<Bounds, String> {
        let ok = (0..3).all(|i| self.min[i].is_finite() && self.max[i].is_finite() && self.min[i].abs() <= 32768.0 && self.max[i].abs() <= 32768.0 && self.max[i] - self.min[i] >= 16.0);
        ok.then(|| Bounds { min: v3(self.min[0], self.min[1], self.min[2]), max: v3(self.max[0], self.max[1], self.max[2]) }).ok_or_else(|| "That area is too small or too far out to compile.".to_string())
    }
}

/// Entities kept whatever the cordon (they light or set up the whole map).
const ALWAYS_KEPT: [&str; 2] = ["light_environment", "info_map_parameters"];

fn origin_of(e: &crate::editor::doc::Entity) -> Option<Vec3> {
    let v: Vec<f64> = e.get("origin")?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    (v.len() == 3).then(|| v3(v[0], v[1], v[2]))
}

fn inside(b: &Bounds, p: Vec3) -> bool {
    p.x >= b.min.x && p.x <= b.max.x && p.y >= b.min.y && p.y <= b.max.y && p.z >= b.min.z && p.z <= b.max.z
}

/// One box with the default options (the tests' shorthand).
#[cfg(test)]
pub fn apply_cordon(doc: &MapDocument, c: &Cordon) -> Result<MapDocument, String> {
    apply_cordons(doc, &[*c], &CordonOptions::default()).map(|r| r.doc)
}

/// How a cordon is sealed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Seal {
    /// a box of sky: the sun reaches in (outdoor areas)
    #[default]
    Sky,
    /// a solid black box: no sun, so indoor lighting stays as it is in the full map
    Black,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CordonOptions {
    #[serde(default)]
    pub seal: Seal,
    /// lights near the boxes are compiled too (the boxes grow to take them in, with the
    /// brushes between, so their light reaches the area as in the full map)
    #[serde(default)]
    pub keep_lights: bool,
    /// put spawns in the area for a playtest when it has none (at this point when it's free,
    /// e.g. the camera, else on a floor near the middle)
    #[serde(default)]
    pub spawn: bool,
    #[serde(default)]
    pub spawn_at: Option<[f64; 3]>,
}

/// What a cordon compile is made from, and what's worth saying about it.
#[derive(Debug)]
pub struct Cordoned {
    pub doc: MapDocument,
    /// the boxes as compiled (merged, grown for lights); read by the tests
    #[cfg_attr(not(test), allow(dead_code))]
    pub boxes: Vec<Bounds>,
    pub warnings: Vec<Problem>,
}

const LIGHTS: [&str; 3] = ["light", "light_spot", "light_environment"];
/// How far outside the boxes lights are still taken in.
pub const LIGHT_REACH: f64 = 1024.0;
const LINK_KEYS: [&str; 4] = ["target", "killtarget", "netname", "message"];

fn touches_or_overlaps(a: &Bounds, b: &Bounds) -> bool {
    a.min.x <= b.max.x && b.min.x <= a.max.x && a.min.y <= b.max.y && b.min.y <= a.max.y && a.min.z <= b.max.z && b.min.z <= a.max.z
}

/// Boxes that overlap or touch become one box round both (so seals never cut through an area).
pub fn merge_boxes(mut v: Vec<Bounds>) -> Vec<Bounds> {
    loop {
        let mut merged = false;
        'outer: for i in 0..v.len() {
            for j in i + 1..v.len() {
                if touches_or_overlaps(&v[i], &v[j]) {
                    let u = v[i].union(&v[j]);
                    v.remove(j);
                    v[i] = u;
                    merged = true;
                    break 'outer;
                }
            }
        }
        if !merged {
            return v;
        }
    }
}

fn in_any(boxes: &[Bounds], p: Vec3) -> bool {
    boxes.iter().any(|b| inside(b, p))
}

fn dist_to_box(b: &Bounds, p: Vec3) -> f64 {
    let d = |lo: f64, hi: f64, x: f64| if x < lo { lo - x } else if x > hi { x - hi } else { 0.0 };
    let v = v3(d(b.min.x, b.max.x, p.x), d(b.min.y, b.max.y, p.y), d(b.min.z, b.max.z, p.z));
    v.len()
}

fn in_brush(doc: &MapDocument, p: Vec3, half: Vec3) -> bool {
    let me = Bounds { min: p - half, max: p + half };
    doc.brushes.values().any(|b| b.entity.is_none_or(|e| !doc.entities.get(&e).is_some_and(|x| x.classname().starts_with("trigger") || x.classname().starts_with("func_buyzone") || x.classname().starts_with("func_bomb") || x.classname() == "func_ladder" || x.classname() == "func_illusionary")) && brush_bounds(b).overlaps(&me))
}

/// A free spot for a player inside `b`: `at` when it's inside and clear, else standing on the
/// highest floor under the box's middle (or a ring round it), else the box's middle.
pub fn spawn_spot(doc: &MapDocument, b: &Bounds, at: Option<Vec3>) -> Option<Vec3> {
    let half = v3(16.0, 16.0, 36.0);
    if let Some(p) = at.filter(|p| p.is_finite() && inside(b, *p)) {
        if !in_brush(doc, p, half) {
            return Some(p);
        }
    }
    let c = b.center();
    let mut tries = vec![(c.x, c.y)];
    for r in [64.0, 128.0, 256.0] {
        for k in 0..8 {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            tries.push((c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    for (x, y) in tries {
        if x - 16.0 < b.min.x || x + 16.0 > b.max.x || y - 16.0 < b.min.y || y + 16.0 > b.max.y {
            continue;
        }
        // floors under this column: brush tops, highest first, with room to stand above
        let mut tops: Vec<f64> = doc
            .brushes
            .values()
            .filter(|br| br.entity.is_none())
            .map(brush_bounds)
            .filter(|bb| bb.min.x <= x && bb.max.x >= x && bb.min.y <= y && bb.max.y >= y && bb.max.z >= b.min.z && bb.max.z + 73.0 <= b.max.z)
            .map(|bb| bb.max.z)
            .collect();
        tops.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        for t in tops {
            let p = v3(x.round(), y.round(), t + 37.0);
            if !in_brush(doc, p, half) {
                return Some(p);
            }
        }
    }
    let p = c;
    (!in_brush(doc, p, half)).then_some(p)
}

/// Only what's inside the boxes, each sealed in its own hollow box of sky or black. Brushes
/// touching a box are kept whole; point entities outside are dropped (lights near the boxes
/// too, unless kept); brush entities left with no brushes are dropped; the sun is kept and
/// moved inside. Says which target links cross the edge.
pub fn apply_cordons(doc: &MapDocument, cs: &[Cordon], o: &CordonOptions) -> Result<Cordoned, String> {
    if cs.is_empty() {
        return Err("Add a cordon box first.".into());
    }
    if cs.len() > 32 {
        return Err("Use at most 32 cordon boxes.".into());
    }
    let mut boxes: Vec<Bounds> = cs.iter().map(Cordon::check).collect::<Result<_, _>>()?;
    boxes = merge_boxes(boxes);
    // lights within reach: each box grows to take in the ones near it
    if o.keep_lights {
        for e in doc.entities.values().filter(|e| LIGHTS[..2].contains(&e.classname())) {
            let Some(p) = origin_of(e) else { continue };
            if in_any(&boxes, p) {
                continue;
            }
            if let Some(b) = boxes.iter_mut().filter(|b| dist_to_box(b, p) <= LIGHT_REACH).min_by(|a, b| dist_to_box(a, p).partial_cmp(&dist_to_box(b, p)).unwrap_or(std::cmp::Ordering::Equal)) {
                b.add(p - v3(16.0, 16.0, 16.0));
                b.add(p + v3(16.0, 16.0, 16.0));
            }
        }
        boxes = merge_boxes(boxes);
    }
    let mut d = doc.clone();
    d.editor = Default::default();
    d.brushes.retain(|_, br| {
        let bb = brush_bounds(br);
        boxes.iter().any(|b| bb.overlaps(b))
    });
    let has_brush: HashSet<EntityId> = d.brushes.values().filter_map(|br| br.entity).collect();
    let brushy: HashSet<EntityId> = doc.brushes.values().filter_map(|br| br.entity).collect();
    d.entities.retain(|id, e| {
        if ALWAYS_KEPT.contains(&e.classname()) {
            return true;
        }
        if brushy.contains(id) {
            return has_brush.contains(id);
        }
        origin_of(e).map_or(true, |p| in_any(&boxes, p))
    });
    // the sun works from anywhere: put it inside so it can't leak
    let first = boxes[0];
    for e in d.entities.values_mut().filter(|e| ALWAYS_KEPT.contains(&e.classname())) {
        if origin_of(e).is_some_and(|p| !in_any(&boxes, p)) {
            let c = first.center();
            e.set("origin", &format!("{} {} {}", c.x.round(), c.y.round(), c.z.round()));
        }
    }

    // links across the edge
    let mut warnings = Vec::new();
    let name_of = |m: &MapDocument, n: &str| m.entities.iter().filter(|(_, e)| e.get("targetname") == Some(n)).map(|(i, _)| *i).collect::<Vec<_>>();
    for (id, e) in &d.entities {
        for k in LINK_KEYS {
            let Some(t) = e.get(k).filter(|t| !t.is_empty()) else { continue };
            if (k == "message" || k == "netname") && !e.classname().starts_with("trigger") && e.classname() != "multi_manager" {
                continue;
            }
            let there = name_of(doc, t);
            if !there.is_empty() && name_of(&d, t).is_empty() {
                warnings.push(Problem {
                    level: "warning".into(),
                    kind: "cordon".into(),
                    message: format!("{} inside the cordon sets off “{t}”, which is outside it: that won't happen in this compile.", e.classname()),
                    detail: String::new(),
                    help: Some("Make the cordon box bigger, or add a box round the other end.".into()),
                    brush: None,
                    entity: Some(id.0),
                    at: None,
                });
            }
        }
    }
    let kept_names: HashSet<String> = d.entities.values().filter_map(|e| e.get("targetname").map(String::from)).collect();
    for (id, e) in &doc.entities {
        if d.entities.contains_key(id) {
            continue;
        }
        let Some(t) = e.get("target").filter(|t| kept_names.contains(*t)) else { continue };
        warnings.push(Problem {
            level: "warning".into(),
            kind: "cordon".into(),
            message: format!("“{t}” inside the cordon is set off by a {} outside it: nothing will set it off in this compile.", e.classname()),
            detail: String::new(),
            help: Some("Make the cordon box bigger to take in what sets it off.".into()),
            brush: None,
            entity: Some(id.0),
            at: origin_of(e).map(|p| [p.x, p.y, p.z]),
        });
        if warnings.len() > 50 {
            break;
        }
    }

    // spawns for a playtest
    if o.spawn {
        let has = |m: &MapDocument, c: &str| m.entities.values().any(|e| e.classname() == c);
        if !has(&d, "info_player_start") || !has(&d, "info_player_deathmatch") {
            let at = o.spawn_at.map(|a| v3(a[0], a[1], a[2]));
            let spot = boxes.iter().find_map(|b| spawn_spot(&d, b, at).map(|p| (p, *b))).ok_or("There's no free floor in the cordon for the player to spawn on.")?;
            let (p, b) = spot;
            let offsets = [(0.0, 0.0), (48.0, 0.0), (0.0, 48.0), (48.0, 48.0), (-48.0, 0.0), (0.0, -48.0), (-48.0, -48.0), (48.0, -48.0)];
            for class in ["info_player_start", "info_player_deathmatch"] {
                if has(&d, class) {
                    continue;
                }
                let mut n = 0;
                for (dx, dy) in offsets {
                    let q = v3(p.x + dx, p.y + dy, p.z);
                    if n == 4 || !inside(&b, q) || in_brush(&d, q, v3(16.0, 16.0, 36.0)) {
                        continue;
                    }
                    let mut e = crate::editor::doc::Entity::new(class);
                    e.set("origin", &format!("{} {} {}", q.x.round(), q.y.round(), q.z.round()));
                    e.set("angle", "0");
                    d.insert_entity(e);
                    n += 1;
                }
                if n == 0 {
                    let mut e = crate::editor::doc::Entity::new(class);
                    e.set("origin", &format!("{} {} {}", p.x.round(), p.y.round(), p.z.round()));
                    d.insert_entity(e);
                }
            }
        }
    }

    // the seals
    let (tex, t) = match o.seal {
        Seal::Sky => ("SKY", 16.0),
        Seal::Black => ("BLACK", 16.0),
    };
    if o.seal == Seal::Black && !d.world.wad_names().iter().any(|w| w == "halflife.wad") {
        d.world.wads.push("halflife.wad".into());
    }
    for b in &boxes {
        let (lo, hi) = (b.min, b.max);
        let slabs = [
            (v3(lo.x - t, lo.y - t, lo.z - t), v3(hi.x + t, hi.y + t, lo.z)),
            (v3(lo.x - t, lo.y - t, hi.z), v3(hi.x + t, hi.y + t, hi.z + t)),
            (v3(lo.x - t, lo.y, lo.z), v3(lo.x, hi.y, hi.z)),
            (v3(hi.x, lo.y, lo.z), v3(hi.x + t, hi.y, hi.z)),
            (v3(lo.x - t, lo.y - t, lo.z), v3(hi.x + t, lo.y, hi.z)),
            (v3(lo.x - t, hi.y, lo.z), v3(hi.x + t, hi.y + t, hi.z)),
        ];
        for (a, z) in slabs {
            d.insert_brush(ops::make_box(a, z, tex));
        }
    }
    Ok(Cordoned { doc: d, boxes, warnings })
}

/// The map's WADs as real files for the compiler, in its `wad` order, plus SDHLT's own;
/// and which of them go inside the .bsp (any that isn't one of the game's own, so players
/// don't need them).
#[derive(Debug, Default, PartialEq)]
pub struct Wads {
    pub paths: Vec<String>,
    pub include: Vec<String>,
    pub missing: Vec<String>,
    /// there but not a texture file the compiler can read: (name, why)
    pub bad: Vec<(String, String)>,
}

/// Why a file isn't a WAD the compiler can read, if it isn't.
pub fn wad_problem(path: &Path) -> Option<String> {
    let mut head = [0u8; 12];
    let n = std::fs::File::open(path).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    if n == 0 {
        return Some("it's empty (0 bytes)".into());
    }
    if n < 12 || !head.starts_with(b"WAD3") {
        return Some("it isn't a texture file (WAD3)".into());
    }
    None
}

/// A network path (UNC `\\host\share`, `//host/share`, `\\?\UNC\...`): never touched, so a map from the
/// internet can't make Windows connect out (and hand over the player's credentials).
pub fn is_network_path(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("\\\\") || t.starts_with("//") || t.starts_with("\\/") || t.starts_with("/\\") || t.to_ascii_lowercase().contains("\\?\\unc")
}

/// `p` with links resolved, when it's inside one of `roots` (also resolved); else None.
fn contained(p: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let real = std::fs::canonicalize(p).ok()?;
    let ok = roots.iter().filter_map(|r| std::fs::canonicalize(r).ok()).any(|r| real.starts_with(&r));
    (ok && real.is_file()).then_some(real)
}

/// Where a WAD the map lists is on this computer. The list comes from map files that may be
/// from the internet, so only these places are ever used: the map's folder (`folder`) and the
/// known WAD places (`roots`: the game's folders). In order: an absolute path that's inside
/// them; a path relative to the map's folder (no `..`) that stays inside it; the name in the
/// map's folder; the name in the game's folders (`in_game`). Network paths are refused.
/// Links are resolved before every check; the result is the real file.
pub fn find_wad(listed: &str, folder: Option<&Path>, roots: &[PathBuf], in_game: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if is_network_path(listed) {
        return None;
    }
    let unix = listed.trim().replace('\\', "/");
    let base = unix.rsplit('/').next().unwrap_or(&unix).to_string();
    if base.is_empty() || base == "." || base == ".." || base.contains(':') || base.contains('\0') {
        return None;
    }
    let mut known: Vec<PathBuf> = roots.to_vec();
    if let Some(dir) = folder {
        known.push(dir.to_path_buf());
    }
    let as_written = Path::new(&unix);
    // an absolute path counts only inside the known places (a Windows drive path means nothing here)
    if as_written.is_absolute() {
        if let Some(p) = contained(as_written, &known) {
            return Some(p);
        }
    } else if let Some(dir) = folder {
        let relative = !unix.contains(':') && as_written.components().all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir));
        if relative {
            if let Some(p) = contained(&dir.join(as_written), &[dir.to_path_buf()]) {
                return Some(p);
            }
        }
    }
    if let Some(dir) = folder {
        let finder = crate::mapcopy::Finder::default();
        if let Some(p) = finder.find(dir, &base).and_then(|p| contained(&p, &[dir.to_path_buf()])) {
            return Some(p);
        }
    }
    in_game(&base).and_then(|p| contained(&p, roots))
}

/// The map's WADs as real files for the compiler. `find` gets each name as the map lists it.
pub fn resolve_wads(names: &[String], find: &dyn Fn(&str) -> Option<PathBuf>, layout: &Layout, copy_dir: &Path) -> Result<Wads, String> {
    let mut w = Wads::default();
    for name in names {
        let base = name.rsplit(['\\', '/']).next().unwrap_or(name).to_string();
        if base.is_empty() || base.eq_ignore_ascii_case("sdhlt.wad") || base.eq_ignore_ascii_case("zhlt.wad") {
            continue;
        }
        let Some(found) = find(name) else {
            w.missing.push(base);
            continue;
        };
        if let Some(why) = wad_problem(&found) {
            w.bad.push((base, why));
            continue;
        }
        let arg = match layout.arg_path(&found) {
            Ok(a) => a,
            // (somewhere wine can't see: copied next to the map)
            Err(_) => {
                std::fs::create_dir_all(copy_dir).map_err(|e| e.to_string())?;
                let dest = copy_dir.join(&base);
                std::fs::copy(&found, &dest).map_err(|e| format!("Couldn't copy {base} for the compiler ({e})."))?;
                layout.arg_path(&dest)?
            }
        };
        w.paths.push(arg);
        if !crate::mapcopy::stock_wad(&base.to_lowercase()) {
            w.include.push(base);
        }
    }
    w.paths.push(layout.arg_path(&layout.tools.join("sdhlt.wad"))?);
    Ok(w)
}

/// The game's limit for planes (MAX_MAP_PLANES), for hlbsp's "Reduced N planes to M".
const MAX_MAP_PLANES: f64 = 32768.0;

fn limit_message(name: &str, pct: f64) -> String {
    format!("The map is over the limit for {name} ({pct:.1}% of what the game allows).")
}

/// Each limit's share of what the game allows, as the LAST report in `text` says it (hlcsg's
/// chart counts planes before hlbsp throws most away: its "Reduced N planes to M" and later
/// charts win): (name, percent, the line).
pub fn chart_values(text: &str) -> Vec<(String, f64, String)> {
    static CHART: OnceLock<regex::Regex> = OnceLock::new();
    static REDUCED: OnceLock<regex::Regex> = OnceLock::new();
    let chart = re(&CHART, r"^\*?\s*([a-zA-Z]+)\s+\d+/\d+\s+\d+/\d+\s+\(\s*([\d.]+)%\)");
    let reduced = re(&REDUCED, r"(?i)reduced\s+(\d+)\s+planes\s+to\s+(\d+)");
    let mut out: Vec<(String, f64, String)> = Vec::new();
    let mut put = |name: String, pct: f64, row: &str| match out.iter_mut().find(|(n, _, _)| n.eq_ignore_ascii_case(&name)) {
        Some(e) => {
            e.1 = pct;
            e.2 = row.to_string();
        }
        None => out.push((name, pct, row.to_string())),
    };
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(c) = chart.captures(line) {
            put(c[1].to_string(), c[2].parse().unwrap_or(0.0), line);
        } else if let Some(c) = reduced.captures(line) {
            let after: f64 = c[2].parse().unwrap_or(0.0);
            put("planes".into(), after / MAX_MAP_PLANES * 100.0, line);
        }
    }
    out
}

/// A later stage's report settles a limit an earlier stage was over: drop the old problem.
pub fn settle_limits(problems: &mut Vec<Problem>, later: &[(String, f64, String)]) {
    for (name, pct, _) in later {
        if *pct <= 100.0 {
            let old = limit_message(name, 0.0);
            let head = &old[..old.find('(').unwrap_or(old.len())];
            problems.retain(|p| !(p.kind == "limit" && p.message.starts_with(head)));
        }
    }
}

/// What stops a compile in the WAD list, in plain words.
pub fn wad_errors(w: &Wads, has_folder: bool) -> Option<String> {
    let mut out = Vec::new();
    if !w.missing.is_empty() {
        let where_ = if has_folder { "the map's project folder or the game folder" } else { "the game folder (or save the map as a project beside them)" };
        out.push(format!("The map lists texture files that aren't on this computer: {}. Put them in {where_}, or remove them in Map properties.", w.missing.join(", ")));
    }
    if !w.bad.is_empty() {
        let list: Vec<String> = w.bad.iter().map(|(n, why)| format!("{n} ({why})")).collect();
        out.push(format!("These texture files can't be read: {}. Replace them with working copies, or remove them in Map properties.", list.join(", ")));
    }
    (!out.is_empty()).then(|| out.join(" "))
}

/// CSG's extra arguments: each custom WAD goes inside the .bsp.
pub fn wadinclude_args(include: &[String]) -> Vec<String> {
    include.iter().flat_map(|w| ["-wadinclude".to_string(), w.clone()]).collect()
}

/// The `.map` text for the compiler: the document with its WAD list replaced by `paths`.
pub fn map_text(doc: &MapDocument, paths: &[String]) -> Result<String, String> {
    let mut d = doc.clone();
    d.world.wads = paths.to_vec();
    // (no editor groups for the compilers; "omit from export" layers left out: EditorCoverage TB-2)
    mapfile::write_map(&d, &mapfile::WriteOptions::compile())
}

/// A map file name from the document's name: lower case letters, digits, `_` and `-`.
/// Parity A: the name a project compiles to: the one kept in it (Save as `<name>_edit`), else the
/// project's own name made into a map name.
pub fn project_map_name(doc: &crate::editor::doc::MapDocument, doc_name: &str) -> String {
    doc.editor.map_name.clone().filter(|n| valid_map_name(n)).unwrap_or_else(|| map_name(doc_name))
}

pub fn map_name(doc_name: &str) -> String {
    let mut s: String = doc_name.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_').chars().take(31).collect::<String>();
    if s.is_empty() || s == "untitled" {
        "ls_newmap".into()
    } else if map_name_problem(&s).is_some() {
        // (a document named "con" or "-": a name Windows or the compiler can't take)
        format!("ls_{s}").chars().take(31).collect::<String>().replace('-', "_")
    } else {
        s
    }
}

pub(crate) fn valid_map_name(n: &str) -> bool {
    map_name_problem(n).is_none()
}

/// What's wrong with a map name, in a sentence (None: it's fine). Besides letters, numbers, _ and
/// - up to 31: not a name Windows keeps for devices (CON, NUL, COM1...: the .bsp couldn't be
/// written there), and not only _ and - (P5, QA4-A).
pub(crate) fn map_name_problem(n: &str) -> Option<&'static str> {
    if n.is_empty() || n.len() > 31 || !n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') || !crate::maps::safe_arg(n) {
        return Some("Map names can only use letters, numbers, _ and - (up to 31).");
    }
    if !n.bytes().any(|c| c.is_ascii_alphanumeric()) {
        return Some("A map name needs at least one letter or number.");
    }
    const RESERVED: &[&str] = &["con", "prn", "aux", "nul", "conin$", "conout$"];
    let l = n.to_ascii_lowercase();
    let device = RESERVED.contains(&l.as_str())
        || ((l.starts_with("com") || l.starts_with("lpt")) && l.len() == 4 && l.as_bytes()[3].is_ascii_digit());
    if device {
        return Some("Windows keeps that name for a device (like CON, NUL, AUX or COM1), so the map couldn't be saved. Pick another name.");
    }
    None
}

// ---- reading what the compiler said -------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// "error" | "warning"
    pub level: String,
    /// "leak" | "limit" | "texture" | "wad" | "brush" | "other"
    pub kind: String,
    pub message: String,
    /// the compiler's own line
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brush: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<[f64; 3]>,
}

fn re(cell: &'static OnceLock<regex::Regex>, pat: &str) -> &'static regex::Regex {
    cell.get_or_init(|| regex::Regex::new(pat).expect("a fixed pattern"))
}

/// What a limit means, in plain words.
fn limit_words(name: &str) -> String {
    let what = match name {
        "MAX_MAP_CLIPNODES" => "too much collision detail (clip nodes). Make detailed shapes players can't touch into func_illusionary or func_wall, or cover them with CLIP brushes",
        "MAX_MAP_PLANES" | "MAX_INTERNAL_MAP_PLANES" => "too many different surface angles (planes). Snap brushes to the grid and simplify odd angles",
        "MAX_MAP_FACES" | "MAX_MAP_SURFEDGES" | "MAX_MAP_EDGES" | "MAX_MAP_VERTS" | "MAX_MAP_MARKSURFACES" => "too many faces. Cover faces nobody sees with NULL, and simplify detailed brushes",
        "MAX_MAP_LEAFS" | "MAX_MAP_NODES" => "too many separate spaces (leaves). Turn small detail into func_wall or func_illusionary",
        "MAX_MAP_MODELS" => "too many brush entities (the game allows 512)",
        "MAX_MAP_ENTITIES" | "MAX_ENGINE_ENTITIES" => "too many entities",
        "MAX_MAP_TEXTURES" | "MAX_MAP_MIPTEX" | "MAX_TEXFILES" => "too many textures, or too much texture data",
        "MAX_MAP_TEXINFO" => "too many different texture alignments",
        "MAX_MAP_LIGHTING" => "too much lighting data. Raise the texture scale on big surfaces",
        "MAX_PATCHES" | "MAX_SINGLEMAP" => "lighting too fine for the compiler. Raise the texture scale on big surfaces",
        "MAX_MAP_BRUSHES" | "MAX_MAP_SIDES" => "too many brushes",
        "MAX_SWITCHED_LIGHTS" => "too many lights that switch on and off (the game allows 32 named light styles)",
        "MAX_TEXLIGHTS" => "too many light-giving textures",
        _ => "",
    };
    if what.is_empty() {
        format!("The map is over one of GoldSrc's limits ({name}).")
    } else {
        format!("The map has {what}.")
    }
}

/// A brush or entity message, in plain words.
fn brush_words(tail: &str) -> String {
    let t = tail.trim();
    let low = t.to_lowercase();
    if low.contains("outside world") {
        "This brush is outside the area maps can use.".into()
    } else if low.contains("mixed face contents") {
        "This brush mixes special textures (like water, sky or CLIP) with normal ones. Use one kind on all its faces.".into()
    } else if low.contains("plane with no normal") {
        "This brush has a broken face (its corners are in a line). Snap it to the grid or remake it.".into()
    } else if low.contains("no sides") {
        "This brush has no faces left. Delete it.".into()
    } else if low.contains("coplanar plane") {
        "This brush has two faces on the same plane. Snap it to the grid or remake it.".into()
    } else if low.contains("edge without opposite face") {
        "This brush isn't a closed shape. Snap it to the grid or remake it.".into()
    } else if low.contains("not allowed in world") {
        format!("{} Tie it to an entity.", t)
    } else {
        format!("Problem with this brush: {t}")
    }
}

/// Brushes in the document that use a texture (the first few), for a link.
fn brushes_with_texture(doc: &MapDocument, tex: &str) -> Option<u64> {
    doc.brushes.iter().find(|(_, b)| b.faces.iter().any(|f| f.texture.eq_ignore_ascii_case(tex))).map(|(id, _)| id.0)
}

/// The entity with this class at about this spot.
fn entity_at(doc: &MapDocument, class: &str, at: [f64; 3]) -> Option<u64> {
    doc.entities
        .iter()
        .find(|(_, e)| e.classname().eq_ignore_ascii_case(class) && origin_of(e).is_some_and(|o| (o.x - at[0]).abs() <= 1.0 && (o.y - at[1]).abs() <= 1.0 && (o.z - at[2]).abs() <= 1.0))
        .map(|(id, _)| id.0)
}

/// The problems in a compiler's output, in plain words, pointing at the editor's brushes and
/// entities where it can (`doc` is the document as compiled, `idx` its .map order).
pub fn parse_output(text: &str, idx: &Index, doc: &MapDocument) -> Vec<Problem> {
    static LEAK: OnceLock<regex::Regex> = OnceLock::new();
    static LEAK_AT: OnceLock<regex::Regex> = OnceLock::new();
    static LIMIT: OnceLock<regex::Regex> = OnceLock::new();
    static CHART: OnceLock<regex::Regex> = OnceLock::new();
    static TEX: OnceLock<regex::Regex> = OnceLock::new();
    static WAD: OnceLock<regex::Regex> = OnceLock::new();
    static BRUSH: OnceLock<regex::Regex> = OnceLock::new();
    static ILLEGAL: OnceLock<regex::Regex> = OnceLock::new();
    let leak = re(&LEAK, r"=== LEAK in hull (\d+) ===");
    let leak_at = re(&LEAK_AT, r"^Entity (\S+) at \(\s*(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s*\)");
    let limit = re(&LIMIT, r"(?i)exceeded\s+(MAX_[A-Z_]+)");
    let chart = re(&CHART, r"^\*?\s*([a-zA-Z]+)\s+\d+/\d+\s+\d+/\d+\s+\(\s*([\d.]+)%\)");
    let tex = re(&TEX, r"(?i)texture (\S+) not found");
    let wad = re(&WAD, r"(?i)could not (?:open|locate) wad file\s*(.*)");
    let brush = re(&BRUSH, r"Entity (\d+), Brush (\d+)(?:, Side \d+)?: (.+)$");
    let illegal = re(&ILLEGAL, r"^(?:Error: |Warning: )?(Illegal Brush[^:]*): Entity (\d+), Brush (\d+)");

    let mut out: Vec<Problem> = Vec::new();
    let add = |out: &mut Vec<Problem>, p: Problem| {
        if out.len() < MAX_PROBLEMS && !out.iter().any(|q| q.message == p.message && q.brush == p.brush && q.entity == p.entity) {
            out.push(p);
        }
    };
    let mut leaked = false;
    let mut last: Option<usize> = None;
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    for (n, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let level = if line.starts_with("Error") { "error" } else { "warning" };
        let before = out.len();
        // (QA-3) no .prt: BSP found no inside to work from, which happens when no entity is inside the map
        if line.contains("Portal file") && line.contains("does not exist") {
            add(&mut out, Problem {
                level: "error".into(),
                kind: "other".into(),
                message: "The map has nothing inside it for the compiler to start from. Put a player spawn (info_player_start) inside a sealed room.".into(),
                detail: line.to_string(),
                help: Some("The compiler fills the map from its entities; with none inside, everything counts as outside.".into()),
                brush: None,
                entity: None,
                at: None,
            });
            continue;
        }
        if leak.is_match(line) {
            if !leaked {
                leaked = true;
                let mut p = Problem {
                    level: "error".into(),
                    kind: "leak".into(),
                    message: "The map leaks: there's a gap between the inside and the void outside. Follow the red leak line from the entity to the gap and close it with a brush.".into(),
                    detail: line.to_string(),
                    ..Default::default()
                };
                if let Some(c) = lines.get(n + 1).and_then(|l| leak_at.captures(l.trim())) {
                    let at = [c[2].parse().unwrap_or(0.0), c[3].parse().unwrap_or(0.0), c[4].parse().unwrap_or(0.0)];
                    p.entity = entity_at(doc, &c[1], at);
                    p.at = Some(at);
                    p.detail = format!("{line} {}", lines[n + 1].trim());
                }
                add(&mut out, p);
            }
        } else if let Some(c) = limit.captures(line) {
            add(&mut out, Problem { level: "error".into(), kind: "limit".into(), message: limit_words(&c[1]), detail: line.into(), ..Default::default() });
        } else if line.contains("AllocBlock") && line.to_lowercase().contains("full") {
            add(&mut out, Problem { level: "error".into(), kind: "limit".into(), message: "The map has too many lightmaps (AllocBlock). Raise the texture scale on big surfaces, or cover hidden faces with NULL.".into(), detail: line.into(), ..Default::default() });
        } else if chart.is_match(line) {
            // (said after the loop: the last row for each limit counts, see chart_values)
        } else if let Some(c) = tex.captures(line) {
            let name = c[1].trim_end_matches('!').to_string();
            add(&mut out, Problem {
                level: "error".into(),
                kind: "texture".into(),
                message: format!("The texture \"{name}\" isn't in any of the map's texture files (WADs). Add its WAD in Map settings, or paint those faces with another texture."),
                detail: line.into(),
                brush: brushes_with_texture(doc, &name),
                ..Default::default()
            });
        } else if let Some(c) = wad.captures(line) {
            let file = c[1].trim();
            let base = file.rsplit(['\\', '/']).next().unwrap_or(file);
            let message = if base.is_empty() { "A texture file (WAD) the map lists couldn't be found.".to_string() } else { format!("The texture file {base} couldn't be opened. Check it's in the game folder, or remove it in Map settings.") };
            add(&mut out, Problem { level: "error".into(), kind: "wad".into(), message, detail: line.into(), ..Default::default() });
        } else if line.contains("Could not find wad file") {
            // (the line after "Could not open wad file ...": said already)
        } else if let Some(c) = illegal.captures(line) {
            let (e, b) = (c[2].parse().unwrap_or(usize::MAX), c[3].parse().unwrap_or(usize::MAX));
            add(&mut out, Problem { level: level.into(), kind: "brush".into(), message: brush_words(&c[1]), detail: line.into(), brush: idx.brush(e, b).map(|x| x.0), entity: idx.entity(e).map(|x| x.0), ..Default::default() });
        } else if let Some(c) = brush.captures(line) {
            let (e, b) = (c[1].parse().unwrap_or(usize::MAX), c[2].parse().unwrap_or(usize::MAX));
            add(&mut out, Problem { level: level.into(), kind: "brush".into(), message: brush_words(&c[3]), detail: line.into(), brush: idx.brush(e, b).map(|x| x.0), entity: idx.entity(e).map(|x| x.0), ..Default::default() });
        } else if let Some(help) = line.strip_prefix("Description:").or_else(|| line.strip_prefix("Howto Fix:")) {
            if let Some(p) = last.and_then(|i| out.get_mut(i)) {
                let h = help.trim();
                p.help = Some(match p.help.take() {
                    Some(old) => format!("{old} {h}"),
                    None => h.to_string(),
                });
            }
            continue;
        } else if let Some(rest) = line.strip_prefix("Error:").or_else(|| line.strip_prefix("ERROR:")) {
            let rest = rest.trim();
            // (the leak's own "Error:" heading has nothing after it)
            if !rest.is_empty() {
                add(&mut out, Problem { level: "error".into(), kind: "other".into(), message: format!("The compiler said: {rest}"), detail: line.into(), ..Default::default() });
            }
        } else if let Some(rest) = line.strip_prefix("Warning:") {
            let rest = rest.trim();
            // (no vis information is what the Fast preset asks for)
            if !rest.is_empty() && !rest.starts_with("===") && !rest.starts_with("No vis information") {
                add(&mut out, Problem { level: "warning".into(), kind: "other".into(), message: format!("The compiler warned: {rest}"), detail: line.into(), ..Default::default() });
            }
        }
        if out.len() > before {
            last = Some(out.len() - 1);
        }
    }
    for (name, pct, row) in chart_values(text) {
        if pct > 100.0 {
            add(&mut out, Problem { level: "error".into(), kind: "limit".into(), message: limit_message(&name, pct), detail: row, ..Default::default() });
        }
    }
    out
}

/// The last progress mark ("10%...") in some output, 0..1.
pub fn last_percent(text: &str) -> Option<f64> {
    static P: OnceLock<regex::Regex> = OnceLock::new();
    re(&P, r"(\d{1,3})%\.\.\.").captures_iter(text).last().and_then(|c| c[1].parse::<f64>().ok()).filter(|p| *p <= 100.0).map(|p| p / 100.0)
}

/// The leak line from a `.lin` file (segments "x y z - x y z") as one path of points.
pub fn parse_lin(text: &str) -> Vec<[f64; 3]> {
    let mut pts: Vec<[f64; 3]> = Vec::new();
    for line in text.lines() {
        let nums: Vec<f64> = line.split_whitespace().filter(|s| *s != "-").filter_map(|s| s.parse().ok()).collect();
        if nums.len() != 6 || nums.iter().any(|n| !n.is_finite()) {
            continue;
        }
        let (a, b) = ([nums[0], nums[1], nums[2]], [nums[3], nums[4], nums[5]]);
        if pts.last() != Some(&a) {
            pts.push(a);
        }
        pts.push(b);
        if pts.len() >= MAX_LEAK_POINTS {
            break;
        }
    }
    pts
}

/// The leak line from a `.pts` file (a point per line, every couple of units), thinned out.
pub fn parse_pts(text: &str) -> Vec<[f64; 3]> {
    let all: Vec<[f64; 3]> = text
        .lines()
        .filter_map(|l| {
            let n: Vec<f64> = l.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            (n.len() == 3 && n.iter().all(|x| x.is_finite())).then(|| [n[0], n[1], n[2]])
        })
        .collect();
    if all.len() <= MAX_LEAK_POINTS {
        return all;
    }
    let step = all.len().div_ceil(MAX_LEAK_POINTS - 1);
    let mut out: Vec<[f64; 3]> = all.iter().step_by(step).copied().collect();
    if out.last() != all.last() {
        out.push(*all.last().unwrap_or(&[0.0; 3]));
    }
    out
}

/// The leak line next to a compiled map (`.lin` first: fewer points, same path).
pub fn read_leak(base: &Path) -> Option<(Vec<[f64; 3]>, PathBuf)> {
    for (ext, parse) in [("lin", parse_lin as fn(&str) -> Vec<[f64; 3]>), ("pts", parse_pts)] {
        let p = base.with_extension(ext);
        if let Ok(t) = std::fs::read_to_string(&p) {
            let pts = parse(&t);
            if pts.len() >= 2 {
                return Some((pts, p));
            }
        }
    }
    None
}

// ---- putting the map in the game ---------------------------------------------------------

/// Copies a compiled map into the game's maps folder as `<name>.bsp`. A different map already
/// there is backed up first (the newest few kept). Returns the backup's path, if one was made.
/// Parity A: a map installed from Community is never written over by a compile (its updates
/// would replace the changes): the message, when `name` is one.
pub fn community_refusal(lib: &Path, name: &str) -> Option<String> {
    super::communitylib::map_owner(lib, name).map(|_| format!("{name} came from Community. Its updates would replace your changes."))
}

/// Parity A: a free name for your own version of a Community map: `<name>_edit`, then `_edit2`...
/// (never a stock map, another Community map, or a map already in the game's maps folder).
pub fn edit_name(lib: &Path, maps: &Path, name: &str) -> String {
    let base: String = name.chars().take(31 - "_edit99".len()).collect();
    let taken = |n: &str| {
        crate::maps::is_stock_file(&format!("maps/{n}.bsp")) || super::communitylib::map_owner(lib, n).is_some() || std::fs::read_dir(maps).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&format!("{n}.bsp")))
    };
    let mut n = format!("{base}_edit");
    let mut i = 2;
    while taken(&n) && i < 100 {
        n = format!("{base}_edit{i}");
        i += 1;
    }
    n
}

pub fn install_bsp(src: &Path, maps: &Path, name: &str, backups: &Path) -> Result<Option<PathBuf>, String> {
    // P5: under test, never into the real game (the route checks first; this is the write)
    super::studio::guard_real_game(maps)?;
    if let Some(why) = map_name_problem(name) {
        return Err(why.into());
    }
    if crate::maps::is_stock_file(&format!("maps/{name}.bsp")) {
        return Err(format!("{name} is one of the game's own maps. Give your map another name."));
    }
    if let Some(why) = community_refusal(&super::communitylib::lib_dir(), name) {
        return Err(why);
    }
    let bytes = std::fs::read(src).map_err(|e| format!("Couldn't read the compiled map ({e})."))?;
    let dest = maps.join(format!("{name}.bsp"));
    let mut backup = None;
    if let Ok(old) = std::fs::read(&dest) {
        if old == bytes {
            return Ok(None);
        }
        std::fs::create_dir_all(backups).map_err(|e| format!("Couldn't make the backups folder ({e})."))?;
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let b = backups.join(format!("{name}-{secs}.bsp"));
        std::fs::write(&b, &old).map_err(|e| format!("Couldn't back up the map that was there ({e})."))?;
        backup = Some(b);
        // the newest few of this map's backups are kept
        let prefix = format!("{name}-");
        let mut mine: Vec<PathBuf> = std::fs::read_dir(backups)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".bsp") && n[prefix.len()..n.len() - 4].bytes().all(|c| c.is_ascii_digit())))
            .collect();
        mine.sort();
        while mine.len() > KEEP_BACKUPS {
            let _ = std::fs::remove_file(mine.remove(0));
        }
    }
    crate::safety::before("Compile", &format!("Put the compiled {name}.bsp in the maps folder"), &dest);
    crate::sys::write_atomic(&dest, &bytes).map_err(|e| format!("Couldn't put the map in the game's maps folder ({e})."))?;
    Ok(backup)
}

// ---- the job ------------------------------------------------------------------------------

#[derive(Default)]
struct Job {
    id: u64,
    kind: &'static str,
    running: bool,
    stage: String,
    label: String,
    fraction: f64,
    log: Vec<String>,
    /// lines dropped from the front (the page counts lines from the start)
    dropped: usize,
    partial: String,
    problems: Vec<Problem>,
    leak: Option<Value>,
    result: Option<Value>,
    error: Option<String>,
    stopped: bool,
    started: Option<Instant>,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);
static STOP: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn with_job<T>(f: impl FnOnce(&mut Job) -> T) -> Option<T> {
    JOB.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(f)
}

fn running() -> bool {
    JOB.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|j| j.running)
}

/// Starts a job unless one is running.
fn begin(kind: &'static str, label: &str) -> Result<u64, String> {
    let mut g = JOB.lock().unwrap_or_else(|e| e.into_inner());
    if g.as_ref().is_some_and(|j| j.running) {
        return Err(BUSY.into());
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    STOP.store(false, Ordering::SeqCst);
    *g = Some(Job { id, kind, running: true, stage: "prepare".into(), label: label.into(), started: Some(Instant::now()), ..Default::default() });
    Ok(id)
}

fn push_line(j: &mut Job, line: &str) {
    let mut l: String = line.chars().take(MAX_LINE).collect();
    if l.len() < line.len() {
        l.push('…');
    }
    j.log.push(l);
    if j.log.len() > MAX_LOG {
        let cut = j.log.len() - MAX_LOG;
        j.log.drain(..cut);
        j.dropped += cut;
    }
}

/// Adds output: whole lines go in the log, the unfinished one ("10%...20%...") is kept apart.
fn feed(j: &mut Job, text: &str) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    j.partial.push_str(&text);
    while let Some(i) = j.partial.find('\n') {
        let line: String = j.partial[..i].to_string();
        j.partial.drain(..=i);
        push_line(j, &line);
    }
    if j.partial.len() > 4 * MAX_LINE {
        let line = std::mem::take(&mut j.partial);
        push_line(j, &line);
    }
}

fn note(text: &str) {
    with_job(|j| push_line(j, text));
}

fn finish(result: Result<Option<Value>, String>) {
    with_job(|j| {
        if !j.partial.is_empty() {
            let line = std::mem::take(&mut j.partial);
            push_line(j, &line);
        }
        j.running = false;
        let stopped = STOP.load(Ordering::SeqCst);
        match result {
            Ok(v) => {
                j.result = v;
                j.fraction = 1.0;
                j.stage = "done".into();
                j.label = "Done".into();
            }
            Err(_) if stopped => {
                j.stopped = true;
                j.stage = "stopped".into();
                j.label = "Stopped".into();
            }
            Err(e) => {
                j.error = Some(e);
                j.stage = "failed".into();
                j.label = "Didn't finish".into();
            }
        }
    });
}

fn progress_json(from: Option<usize>) -> Value {
    let g = JOB.lock().unwrap_or_else(|e| e.into_inner());
    let Some(j) = g.as_ref() else { return json!({"job": null}) };
    let next = j.dropped + j.log.len();
    let lines: Vec<&String> = match from {
        Some(f) => j.log.iter().skip(f.saturating_sub(j.dropped)).collect(),
        None => Vec::new(),
    };
    json!({
        "job": {
            "id": j.id,
            "kind": j.kind,
            "running": j.running,
            "stage": j.stage,
            "label": j.label,
            "fraction": (j.fraction * 1000.0).round() / 1000.0,
            "seconds": j.started.map_or(0, |s| s.elapsed().as_secs()),
            "problems": j.problems,
            "leak": j.leak,
            "result": j.result,
            "error": j.error,
            "stopped": j.stopped,
        },
        "lines": lines,
        "from": from.map(|f| f.max(j.dropped)),
        "next": next,
        "partial": j.partial,
    })
}

// ---- downloading the compiler --------------------------------------------------------------

/// Downloads the pinned release straight from GitHub (stoppable), checks it and unpacks it.
fn download_and_install(dir: &Path) -> Result<(), String> {
    let parent = dir.parent().ok_or(INTERNAL)?;
    std::fs::create_dir_all(parent).map_err(|e| format!("Couldn't make the compiler's folder ({e})."))?;
    let zip = parent.join(format!("sdhlt-v{SDHLT_VERSION}.zip.part"));
    let fail = |e: String| format!("Couldn't download SDHLT ({e}). Check your internet connection and try again.");
    let mut resp = crate::sys::http_download().get(SDHLT_URL).header("User-Agent", "Loner-Strike").send().map_err(|e| fail(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(fail(format!("GitHub answered {}", resp.status())));
    }
    let size = resp.content_length().unwrap_or(SDHLT_BYTES).max(1);
    let mut out = std::fs::File::create(&zip).map_err(|e| fail(e.to_string()))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0u64;
    loop {
        if STOP.load(Ordering::SeqCst) {
            drop(out);
            let _ = std::fs::remove_file(&zip);
            return Err("stopped".into());
        }
        let n = resp.read(&mut buf).map_err(|e| fail(e.to_string()))?;
        if n == 0 {
            break;
        }
        got += n as u64;
        // (the release is 3 MB: anything far bigger isn't it)
        if got > 64 << 20 {
            let _ = std::fs::remove_file(&zip);
            return Err(fail("it's far bigger than expected".into()));
        }
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| fail(e.to_string()))?;
        with_job(|j| {
            j.fraction = (got as f64 / size as f64).min(1.0) * 0.9;
            j.label = format!("Downloading SDHLT ({:.1} of {:.1} MB)…", got as f64 / 1_048_576.0, size as f64 / 1_048_576.0);
        });
    }
    drop(out);
    with_job(|j| {
        j.stage = "unpack".into();
        j.label = "Checking and unpacking SDHLT…".into();
    });
    let r = install_zip(&zip, dir);
    let _ = std::fs::remove_file(&zip);
    r
}

fn start_setup() -> Result<Value, String> {
    let dir = install_dir();
    if is_installed(&dir) {
        return Ok(json!({"ok": true, "installed": true}));
    }
    let id = begin("setup", "Downloading SDHLT…")?;
    with_job(|j| j.stage = "download".into());
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(|| download_and_install(&dir)).unwrap_or_else(|_| Err(INTERNAL.into()));
        if r.is_ok() {
            note(&format!("SDHLT {SDHLT_VERSION} is ready ({}).", dir.display()));
        }
        finish(r.map(|_| Some(json!({"installed": true}))));
    });
    Ok(json!({"ok": true, "job": id}))
}

// ---- compiling ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct StartReq {
    #[serde(default = "normal")]
    preset: Preset,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    cordon: Option<Cordon>,
    /// several boxes compiled together (with `cordon`, all of them)
    #[serde(default)]
    cordons: Vec<Cordon>,
    /// seal, lights, playtest spawn
    #[serde(default, alias = "cordonOptions")]
    cordon_options: CordonOptions,
    /// put the map in the game's maps folder when it compiles
    #[serde(default)]
    install: bool,
    /// let the compiler use every core (faster; the computer may lag)
    #[serde(default, alias = "allCores")]
    all_cores: bool,
}

fn normal() -> Preset {
    Preset::Normal
}

/// Everything a compile needs, worked out before its thread starts.
struct Plan {
    layout: Layout,
    install_from: PathBuf,
    name: String,
    preset: Preset,
    doc: MapDocument,
    wad_dirs: Vec<PathBuf>,
    /// the game's maps folder, when the map goes into the game afterwards
    maps: Option<PathBuf>,
    /// said before compiling (cordon links crossing the edge)
    warnings: Vec<Problem>,
    /// the folder the map came from: WADs beside it are found first
    folder: Option<PathBuf>,
    /// P5: Playtest: the models, sprites and sounds the map names, copied into the game after
    assets: Option<super::editor::assets::ForPlaytest>,
}

fn plan(st: &Shared, req: StartReq) -> Result<Plan, String> {
    // P5: Playtest: a missing model/sprite/sound stops the game loading the map, so say so first
    let assets = if req.install { Some(super::editor::assets::for_playtest(st)?) } else { None };
    let install_from = install_dir();
    if !is_installed(&install_from) {
        return Err(NEEDS_SDHLT.into());
    }
    let (doc, doc_name) = super::editor::open_document().ok_or(NO_DOC)?;
    if doc.brushes.is_empty() {
        return Err("The map has no brushes yet. Draw a room first.".into());
    }
    let game = st.local_game().ok_or(NO_GAME)?;
    let client = crate::servers::client(st).ok_or(NO_GAME)?;
    let name = match req.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        Some(n) if valid_map_name(&n) => n,
        Some(n) => return Err(map_name_problem(&n).unwrap_or("That isn't a map name.").into()),
        None => project_map_name(&doc, &doc_name),
    };
    let mut boxes = req.cordons.clone();
    boxes.extend(req.cordon);
    let (doc, mut warnings) = if boxes.is_empty() {
        (doc, Vec::new())
    } else {
        let c = apply_cordons(&doc, &boxes, &req.cordon_options)?;
        (c.doc, c.warnings)
    };
    // P5: models, sprites, sounds or a sky the entities name that aren't on this computer
    if let Some(n) = super::editor::assets::compile_note(st) {
        warnings.push(Problem { level: "warning".into(), kind: "other".into(), message: n, ..Default::default() });
    }
    let maps = if req.install {
        if crate::maps::is_stock_file(&format!("maps/{name}.bsp")) {
            return Err(format!("{name} is one of the game's own maps. Give your map another name."));
        }
        if let Some(why) = community_refusal(&super::communitylib::lib_dir(), &name) {
            return Err(why);
        }
        // P5: under test (the dev server the browser tests use), never into the real game
        super::studio::guard_real_game(&game.dir)?;
        Some(game.dir.join("maps"))
    } else {
        None
    };
    let mut layout = layout_for(&client, &install_from, false)?;
    layout.threads = Some(compile_threads(cores(), req.all_cores));
    // Parity A: test builds (the dev server agents use) can cap the threads, so compiles there keep
    // to the crew's limit (TOOLS-CREW.md: -threads 6) on a shared Mac; the app ignores it
    if cfg!(test) {
        if let Some(n) = std::env::var("LS_COMPILE_THREADS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| *n >= 1) {
            layout.threads = Some(n);
        }
    }
    let wad_dirs = crate::goldsrc::wad::search_dirs(&game.hl, game.is_czero());
    let folder = super::editor::open_document_folder();
    Ok(Plan { layout, install_from, name, preset: req.preset, doc, wad_dirs, maps, warnings, folder, assets })
}

/// Runs one stage, streaming its output into the job; Ok(its whole output) when it exits
/// cleanly, Err(output) when it fails or is stopped.
fn run_stage(layout: &Layout, stage: Stage, preset: Preset, base: &Path, extra: &[String], done: f64, total: f64) -> Result<String, String> {
    let (program, args) = layout.command_line(stage, preset, base, extra)?;
    // (the program's own arguments: through wine, after --bottle B --wait-children exe)
    let own = match args.iter().position(|a| a.to_ascii_lowercase().ends_with(".exe")) {
        Some(i) if matches!(layout.runner, Runner::Wine { .. }) => &args[i + 1..],
        _ => &args[..],
    };
    note(&format!("> {} {}", stage.program(), own.join(" ")));
    let mut cmd = crate::sys::command(&program);
    low_priority(&mut cmd);
    cmd.args(&args).current_dir(base.parent().unwrap_or(Path::new("."))).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    if matches!(layout.runner, Runner::Wine { .. }) {
        cmd.env("WINEDEBUG", "-all");
    }
    let mut child = cmd.spawn().map_err(|e| format!("{} didn't start ({e}).", stage.program()))?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    for pipe in [child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>), child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>)].into_iter().flatten() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut pipe = pipe;
            let mut buf = vec![0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);
    let mut text = String::new();
    let w = stage.weight();
    let mut killed = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                let s = crate::rcon::latin1(&chunk);
                if text.len() < MAX_STAGE_TEXT {
                    text.push_str(&s);
                }
                with_job(|j| {
                    feed(j, &s);
                    if let Some(p) = last_percent(&j.partial).or_else(|| last_percent(&s)) {
                        j.fraction = ((done + w * p) / total).min(0.99);
                    }
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if STOP.load(Ordering::SeqCst) && !killed {
            killed = true;
            let _ = child.kill();
            end_leftovers(layout);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if killed {
        return Err(text);
    }
    if status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

/// Under CrossOver the Windows program can outlive the wine launcher that was stopped.
fn end_leftovers(layout: &Layout) {
    if matches!(layout.runner, Runner::Wine { .. }) {
        let mut c = crate::sys::command("pkill");
        c.arg("-f").arg(r"Loner-Strike.Compiler.sdhlt-v[0-9.]+.tools.Win(32|64).sdHL");
        let _ = crate::sys::run_output(c, Duration::from_secs(5));
    }
}

fn run_compile(p: Plan) -> Result<Option<Value>, String> {
    // on a Mac the tools go into the bottle first (once; later only checked)
    let layout = if matches!(p.layout.runner, Runner::Wine { .. }) {
        with_job(|j| j.label = "Getting the compiler ready…".into());
        mirror_tools(&p.install_from.join("tools"), &p.layout.tools)?;
        p.layout.clone()
    } else {
        p.layout.clone()
    };
    let dir = layout.work.join(&p.name);
    let _ = std::fs::remove_dir_all(&dir);
    // Parity A (QA4-A A-1): this document's last compile, for the Budget
    if let Some(uid) = p.doc.editor.uid.as_deref() {
        note_compile_in(&last_compiles_file(), uid, &p.name, &dir.join(format!("{}.log", p.name)), now_secs());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the compile folder ({e})."))?;
    let base = dir.join(&p.name);

    // the .map, with the WADs as files on this computer
    let finder = crate::mapcopy::Finder::default();
    // beside the map's project (or the file it came from) first, then the game's folders
    let in_game = |base: &str| -> Option<PathBuf> { p.wad_dirs.iter().find_map(|d| finder.find(d, base)) };
    let find = |name: &str| -> Option<PathBuf> { find_wad(name, p.folder.as_deref(), &p.wad_dirs, &in_game) };
    let wads = resolve_wads(&p.doc.world.wads, &find, &layout, &dir.join("wads"))?;
    if let Some(e) = wad_errors(&wads, p.folder.is_some()) {
        return Err(e);
    }
    let text = map_text(&p.doc, &wads.paths)?;
    // entity text in the game's code page (GoldSrc shows bytes, not UTF-8)
    let (bytes, cp, lost) = mapfile::encode_for_game(&text);
    if let Some(note) = super::editor::code_page_note(cp, &lost) {
        with_job(|j| j.problems.push(Problem { level: "warning".into(), kind: "other".into(), message: note, ..Default::default() }));
    }
    std::fs::write(base.with_extension("map"), &bytes).map_err(|e| format!("Couldn't write the map for the compiler ({e})."))?;
    let idx = Index::of(&p.doc);
    if !p.warnings.is_empty() {
        with_job(|j| j.problems.extend(p.warnings.iter().take(MAX_PROBLEMS).cloned()));
    }
    note(&format!("Compiling {} ({} brushes, {} entities) with SDHLT {SDHLT_VERSION}, {} preset.", p.name, p.doc.brushes.len(), p.doc.entities.len() + 1, format!("{:?}", p.preset).to_lowercase()));

    let stages = p.preset.stages();
    let total: f64 = stages.iter().map(|s| s.weight()).sum();
    let mut done = 0.0;
    for stage in stages {
        if STOP.load(Ordering::SeqCst) {
            return Err("stopped".into());
        }
        with_job(|j| {
            j.stage = stage.id().into();
            j.label = stage.label().into();
            j.fraction = done / total;
        });
        let extra = if stage == Stage::Csg { wadinclude_args(&wads.include) } else { Vec::new() };
        let r = run_stage(&layout, stage, p.preset, &base, &extra, done, total);
        let out = match &r {
            Ok(t) | Err(t) => t.clone(),
        };
        let found = parse_output(&out, &idx, &p.doc);
        let leaked = found.iter().any(|x| x.kind == "leak");
        let charts = chart_values(&out);
        with_job(|j| {
            settle_limits(&mut j.problems, &charts);
            for f in found.iter() {
                if j.problems.len() < MAX_PROBLEMS && !j.problems.contains(f) {
                    j.problems.push(f.clone());
                }
            }
        });
        if STOP.load(Ordering::SeqCst) {
            return Err("stopped".into());
        }
        if leaked {
            if let Some((pts, file)) = read_leak(&base) {
                let shown = file.to_string_lossy().to_string();
                // P5: the editor's leak tracing (Go to leak, the hole, MCP editor_leak)
                let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| shown.clone());
                super::editor::leak::set(Some(super::editor::leak::Leak { points: pts.clone(), pointfile: name, from: "compile" }));
                with_job(|j| {
                    j.leak = Some(json!({"points": pts, "pointfile": shown}));
                    if let Some(l) = j.problems.iter_mut().find(|x| x.kind == "leak") {
                        l.help = Some(format!("The leak line is drawn in the 3D view (pointfile: {shown})."));
                    }
                });
            }
            return Err("The map leaks, so it can't be finished. Close the gap the red line goes through, then compile again.".into());
        }
        if r.is_err() {
            let first = found.iter().find(|x| x.level == "error").map(|x| x.message.clone());
            return Err(first.unwrap_or_else(|| format!("{} stopped with an error. The log says what it found.", stage.program())));
        }
        if stage == Stage::Bsp && !base.with_extension("bsp").is_file() {
            return Err("The compiler didn't make a .bsp. The log says what it found.".into());
        }
        // P5: past BSP without a leak: the old leak line is gone
        if stage == Stage::Bsp {
            super::editor::leak::set(None);
        }
        done += stage.weight();
    }
    let bsp = base.with_extension("bsp");
    if !bsp.is_file() {
        return Err("The compiler didn't make a .bsp. The log says what it found.".into());
    }
    // (what's left of the work besides the map, the .map and the logs)
    for ext in ["p0", "p1", "p2", "p3", "b0", "b1", "b2", "b3", "wa_", "hsz", "pln", "ext", "prt", "lin", "pts"] {
        let _ = std::fs::remove_file(base.with_extension(ext));
    }
    let size = std::fs::metadata(&bsp).map(|m| m.len()).unwrap_or(0);
    let mut result = json!({"map": p.name, "bsp": bsp.to_string_lossy(), "bytes": size, "preset": p.preset, "installed": false});
    if let Some(maps) = &p.maps {
        with_job(|j| {
            j.stage = "install".into();
            j.label = "Putting the map in the game…".into();
        });
        let backup = install_bsp(&bsp, maps, &p.name, &compile_dir().join("backups"))?;
        if let Some(b) = &backup {
            note(&format!("The map that was there is backed up as {}.", b.display()));
        }
        note(&format!("Copied to {}.", maps.join(format!("{}.bsp", p.name)).display()));
        // P5: the map's own models, sprites and sounds into the game's download folder, and its .res
        for n in p.assets.as_ref().map(|a| a.install(&p.name)).unwrap_or_default() {
            note(&n);
        }
        result["installed"] = json!(true);
        result["backup"] = json!(backup.map(|b| b.to_string_lossy().to_string()));
        result["hasNav"] = json!(maps.join(format!("{}.nav", p.name)).is_file());
    }
    Ok(Some(result))
}

fn start_compile(st: &Shared, req: StartReq) -> Result<Value, String> {
    if running() {
        return Err(BUSY.into());
    }
    // Parity A: a Community map's name, refused with the two ways on (your own copy under
    // another name, or a credited remix when the map was shared with its source)
    if req.install {
        let want = req.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).or_else(|| super::editor::open_document().map(|(d, n)| project_map_name(&d, &n)));
        if let Some(want) = want {
            let lib = super::communitylib::lib_dir();
            if let Some(it) = super::communitylib::map_owner(&lib, &want) {
                let maps = st.local_game().map(|g| g.dir.join("maps")).unwrap_or_default();
                let known = super::community::index_known();
                let remixable = known.items.iter().any(|i| i.model == it.model && i.gb == it.gb && i.source);
                return Ok(json!({
                    "error": community_refusal(&lib, &want).unwrap_or_default(),
                    "community": {
                        "map": want, "name": it.name, "author": it.author, "model": it.model, "gb": it.gb,
                        "editName": edit_name(&lib, &maps, &want), "remixable": remixable,
                    },
                }));
            }
        }
    }
    let p = plan(st, req)?;
    let name = p.name.clone();
    let id = begin("compile", "Getting ready…")?;
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_compile(p))).unwrap_or_else(|_| Err(INTERNAL.into()));
        finish(r);
    });
    Ok(json!({"ok": true, "job": id, "map": name}))
}

fn status(st: &Shared) -> Value {
    let dir = install_dir();
    let installed = is_installed(&dir);
    let can_run = match crate::servers::client(st) {
        None => Err(NO_GAME.to_string()),
        Some(c) => layout_for(&c, &dir, false).map(|_| ()),
    };
    let suggest = super::editor::open_document().map(|(d, n)| project_map_name(&d, &n));
    json!({
        "installed": installed,
        "version": SDHLT_VERSION,
        "bytes": SDHLT_BYTES,
        "url": SDHLT_URL,
        "licence": LICENCE,
        "folder": dir.to_string_lossy(),
        "canRun": can_run.is_ok(),
        "cantRun": can_run.err(),
        "suggestName": suggest,
        "cores": cores(),
        "threads": compile_threads(cores(), false),
        "busy": running(),
    })
}

// ---- routes ------------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

fn forbidden() -> Response {
    (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(INTERNAL.into())))
}

async fn api_status(State(st): State<Shared>) -> Response {
    blocking(move || Ok(status(&st))).await
}

async fn api_setup(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(start_setup).await
}

async fn api_start(State(st): State<Shared>, h: HeaderMap, Json(req): Json<StartReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || start_compile(&st, req)).await
}

#[derive(Deserialize)]
struct FromQuery {
    from: Option<usize>,
}

async fn api_progress(Query(q): Query<FromQuery>) -> Response {
    Json(progress_json(q.from)).into_response()
}

async fn api_cancel(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    if !running() {
        return answer(Err("The compiler isn't running.".into()));
    }
    STOP.store(true, Ordering::SeqCst);
    with_job(|j| j.label = "Stopping…".into());
    answer(Ok(json!({"ok": true})))
}

/// Parity A: the last compile's map put in the game (a Final compile, before "Teach bots and send to
/// my servers"): the same checks and backup as a playtest's install.
fn install_last(st: &Shared) -> Result<Value, String> {
    let (map, bsp) = with_job(|j| {
        let r = j.result.as_ref().filter(|_| j.kind == "compile" && !j.running)?;
        Some((r["map"].as_str()?.to_string(), PathBuf::from(r["bsp"].as_str()?)))
    })
    .flatten()
    .ok_or("Compile the map first.")?;
    if !bsp.is_file() {
        return Err("The compiled map isn't there any more. Compile it again.".into());
    }
    let game = st.local_game().ok_or(NO_GAME)?;
    super::studio::guard_real_game(&game.dir)?;
    let maps = game.dir.join("maps");
    let backup = install_bsp(&bsp, &maps, &map, &compile_dir().join("backups"))?;
    with_job(|j| {
        if let Some(r) = j.result.as_mut() {
            r["installed"] = json!(true);
        }
    });
    Ok(json!({"ok": true, "map": map, "backup": backup.map(|b| b.to_string_lossy().to_string())}))
}

async fn api_install(State(st): State<Shared>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || install_last(&st)).await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/compile/install", post(api_install))
        .route("/api/tools/compile/status", get(api_status))
        .route("/api/tools/compile/setup", post(api_setup))
        .route("/api/tools/compile/start", post(api_start))
        .route("/api/tools/compile/progress", get(api_progress))
        .route("/api/tools/compile/cancel", post(api_cancel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::Entity;

    /// Parity A: a Community map's name is refused (any case, the exact name only), and
    /// `<name>_edit` is the next free name.
    #[test]
    fn community_maps_are_never_written_over() {
        let root = std::env::temp_dir().join(format!("ls-compile-community-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let lib = root.join("community");
        let maps = root.join("maps");
        std::fs::create_dir_all(&maps).unwrap();
        let it = super::super::communitylib::Installed { gb: 4242, name: "Lonely".into(), author: "jam".into(), maps: vec!["de_lonely".into()], ..Default::default() };
        super::super::communitylib::save(&lib, &it).unwrap();
        assert_eq!(community_refusal(&lib, "DE_Lonely").as_deref(), Some("DE_Lonely came from Community. Its updates would replace your changes."));
        assert_eq!(community_refusal(&lib, "de_lonely2"), None, "only the exact name");
        assert_eq!(community_refusal(&lib, "de_lone"), None);
        assert_eq!(edit_name(&lib, &maps, "de_lonely"), "de_lonely_edit");
        std::fs::write(maps.join("de_lonely_edit.bsp"), b"x").unwrap();
        assert_eq!(edit_name(&lib, &maps, "de_lonely"), "de_lonely_edit2");
        // a long name still fits the game's 31 characters
        assert!(edit_name(&lib, &maps, "de_a_very_long_map_name_for_qa_x").len() <= 31);
        let _ = std::fs::remove_dir_all(&root);
    }

    const LEAK_LOG: &str = include_str!("testdata/compile/leak_bsp.log");
    const TEX_LOG: &str = include_str!("testdata/compile/missing_texture_csg.log");
    const OK_LOG: &str = include_str!("testdata/compile/ok_rad.log");
    const LIMITS_LOG: &str = include_str!("testdata/compile/limits_and_entities.log");
    const LIN: &str = include_str!("testdata/compile/leak.lin");
    const PTS: &str = include_str!("testdata/compile/leak.pts");

    /// A room like the fixtures' (a floor, a sky lid, walls), a sun and a spawn, with a
    /// func_wall holding one brush.
    fn room() -> MapDocument {
        let mut d = MapDocument::new();
        d.world.wads = vec!["halflife.wad".into(), "\\sierra\\mine.wad".into()];
        d.insert_brush(ops::make_box(v3(-272.0, -272.0, -16.0), v3(272.0, 272.0, 0.0), "CRETE3_FLR04"));
        d.insert_brush(ops::make_box(v3(-272.0, -256.0, 0.0), v3(-256.0, 256.0, 256.0), "NOTEXISTS"));
        let mut sun = Entity::new("light_environment");
        sun.set("origin", "0 0 128");
        let sun = d.insert_entity(sun);
        let mut spawn = Entity::new("info_player_start");
        spawn.set("origin", "2000 0 36");
        d.insert_entity(spawn);
        let wall = d.insert_entity(Entity::new("func_wall"));
        let mut b = ops::make_box(v3(0.0, 0.0, 0.0), v3(32.0, 32.0, 32.0), "C1A0_W1");
        b.entity = Some(wall);
        d.insert_brush(b);
        let _ = sun;
        d
    }

    fn native(tools: &str) -> Layout {
        Layout { runner: Runner::Native, tools: PathBuf::from(tools), work: PathBuf::from("/w"), x64: true, threads: None }
    }

    fn wine() -> Layout {
        let b = "/Users/x/Library/Application Support/CrossOver/Bottles/Steam/drive_c";
        Layout {
            runner: Runner::Wine { wine: PathBuf::from(crate::servers::CROSSOVER_WINE), bottle: "Steam".into() },
            tools: PathBuf::from(format!("{b}/Loner-Strike/Compiler/sdhlt-v1.3.1/tools")),
            work: PathBuf::from(format!("{b}/Loner-Strike/Compiler/work")),
            x64: true,
            threads: None,
        }
    }

    /// Parity A (QA4-A A-1): the Budget's last compile is the document's own, whatever name it
    /// was compiled as; another document compiling to the same log takes it over; a log older
    /// than the compile (one left from before) doesn't count.
    #[test]
    fn last_compile_is_the_documents_own() {
        let dir = std::env::temp_dir().join(format!("ls-compile-last-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("work/qa_name")).unwrap();
        std::fs::create_dir_all(dir.join("work/ls_outpost")).unwrap();
        let file = dir.join("last-compiles.json");
        let mine = dir.join("work/qa_name/qa_name.log");
        let other = dir.join("work/ls_outpost/ls_outpost.log");
        // an old compile of a map with the document's name is there; this document never compiled
        std::fs::write(&other, "old").unwrap();
        assert_eq!(last_compile_in(&file, "doc-a"), None);
        // compiled under another name: that log, with the name it was compiled as
        let t = now_secs();
        note_compile_in(&file, "doc-a", "qa_name", &mine, t);
        assert_eq!(last_compile_in(&file, "doc-a"), None, "no log yet (still compiling, or it stopped early)");
        std::fs::write(&mine, "new").unwrap();
        assert_eq!(last_compile_in(&file, "doc-a"), Some((mine.clone(), "qa_name".to_string())));
        // a log left from before the compile started isn't this compile's
        note_compile_in(&file, "doc-b", "ls_outpost", &other, t + 3600);
        assert_eq!(last_compile_in(&file, "doc-b"), None);
        // another document compiles to the same name: the log is its now
        note_compile_in(&file, "doc-c", "qa_name", &mine, t);
        assert_eq!(last_compile_in(&file, "doc-a"), None);
        assert_eq!(last_compile_in(&file, "doc-c"), Some((mine.clone(), "qa_name".to_string())));
        // compiling again under the document's own name moves it there
        note_compile_in(&file, "doc-c", "ls_outpost", &other, t.saturating_sub(10));
        assert_eq!(last_compile_in(&file, "doc-c"), Some((other.clone(), "ls_outpost".to_string())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_leak_log() {
        let d = room();
        let idx = Index::of(&d);
        let p = parse_output(LEAK_LOG, &idx, &d);
        let leaks: Vec<_> = p.iter().filter(|x| x.kind == "leak").collect();
        assert_eq!(leaks.len(), 1, "{p:#?}");
        assert_eq!(leaks[0].at, Some([0.0, 0.0, 128.0]));
        // the sun in the document is where the leak starts
        let sun = d.entities.iter().find(|(_, e)| e.classname() == "light_environment").map(|(id, _)| id.0);
        assert_eq!(leaks[0].entity, sun);
        // nothing else in a leak log is an error (the chart is well under the limits)
        assert!(p.iter().all(|x| x.kind == "leak" || x.level != "error"), "{p:#?}");
    }

    #[test]
    fn the_missing_texture_log() {
        let d = room();
        let p = parse_output(TEX_LOG, &Index::of(&d), &d);
        let tex: Vec<_> = p.iter().filter(|x| x.kind == "texture").collect();
        // "FindTexture() ... not found" and "LoadLump() ... not found" say the same: one problem
        assert_eq!(tex.len(), 1, "{p:#?}");
        assert!(tex[0].message.contains("\"NOTEXISTS\""));
        assert_eq!(tex[0].brush, d.brushes.iter().find(|(_, b)| b.faces[0].texture == "NOTEXISTS").map(|(id, _)| id.0));
        assert!(p.iter().all(|x| x.kind != "leak"));
    }

    #[test]
    fn a_clean_log_has_no_problems() {
        let d = room();
        let p = parse_output(OK_LOG, &Index::of(&d), &d);
        assert!(p.iter().all(|x| x.level != "error"), "{p:#?}");
        assert!(parse_output("Warning: No vis information.\n", &Index::of(&d), &d).is_empty());
    }

    #[test]
    fn limits_wads_and_brushes() {
        let d = room();
        let idx = Index::of(&d);
        let p = parse_output(LIMITS_LOG, &idx, &d);
        let by = |k: &str| p.iter().filter(|x| x.kind == k).collect::<Vec<_>>();
        // Exceeded MAX_MAP_CLIPNODES, the clipnodes and worldleaves rows over 100%, AllocBlock
        let limits = by("limit");
        assert_eq!(limits.len(), 4, "{limits:#?}");
        assert!(limits[0].message.contains("collision detail"));
        assert!(limits[0].help.as_deref().unwrap_or("").contains("CLIP brushes"));
        assert!(limits.iter().any(|l| l.message.contains("worldleaves")));
        assert!(limits.iter().any(|l| l.message.contains("lightmaps")));
        let wads = by("wad");
        assert_eq!(wads.len(), 1);
        assert!(wads[0].message.contains("custom.wad"));
        assert!(wads[0].help.is_some());
        // Entity 0, Brush 1 is the world's second brush; Entity 2 is the second entity (the
        // spawn) which has no brushes
        let brushes = by("brush");
        assert_eq!(brushes.len(), 2, "{brushes:#?}");
        assert!(brushes[0].message.contains("mixes special textures"));
        assert_eq!(brushes[0].level, "error");
        assert_eq!(brushes[0].brush, idx.brush(0, 1).map(|b| b.0));
        assert!(brushes[0].brush.is_some() && brushes[0].entity.is_none());
        assert_eq!(brushes[1].level, "warning");
        assert!(brushes[1].message.contains("broken face"));
        assert_eq!(brushes[1].entity, idx.entity(2).map(|e| e.0));
        assert_eq!(brushes[1].brush, None);
    }

    #[test]
    fn index_follows_the_map_file() {
        let d = room();
        let idx = Index::of(&d);
        let text = mapfile::write_map(&d, &mapfile::WriteOptions::default()).unwrap();
        let back = mapfile::read_map(&text).unwrap();
        // the same number of entities, and the same brush counts in the same order
        assert_eq!(idx.entities.len(), back.entities.len() + 1);
        assert_eq!(idx.entities[0].1.len(), back.brushes.values().filter(|b| b.entity.is_none()).count());
        // the func_wall is the 4th .map entity (world, sun, spawn, wall) and owns one brush
        let wall = idx.entities[3].1[0];
        assert_eq!(d.brushes[&wall].entity, idx.entity(3));
        assert_eq!(idx.brush(9, 0), None);
    }

    #[test]
    fn progress_marks() {
        assert_eq!(last_percent("SolidBSP [hull 0] 10%...20%...30%..."), Some(0.3));
        assert_eq!(last_percent("models   1/512   64/32768    ( 0.2%)"), None);
        assert_eq!(last_percent("nothing"), None);
        let mut j = Job::default();
        feed(&mut j, "line one\r\nhalf");
        feed(&mut j, " done\r\n10%...20%...");
        assert_eq!(j.log, vec!["line one", "half done"]);
        assert_eq!(j.partial, "10%...20%...");
    }

    #[test]
    fn leak_lines() {
        let lin = parse_lin(LIN);
        // 7 chained segments -> 8 points, starting at the sun's leak start
        assert_eq!(lin.len(), 8);
        assert_eq!(lin[0], [0.0, 256.0, 128.0]);
        assert_eq!(lin[1], [284.0, 256.0, 128.0]);
        assert_eq!(lin[4], [-284.0, -20.0, 0.0]);
        assert_eq!(*lin.last().unwrap(), [272.0, -284.0, -8.0]);
        let pts = parse_pts(PTS);
        assert_eq!(pts.len(), 200);
        assert_eq!(pts[2], [2.0, 256.0, 128.0]);
        // a long pointfile is thinned out but keeps its ends
        let long: String = (0..10_000).map(|i| format!("{i}.0 0.0 0.0\n")).collect();
        let thin = parse_pts(&long);
        assert!(thin.len() <= MAX_LEAK_POINTS);
        assert_eq!(thin[0], [0.0, 0.0, 0.0]);
        assert_eq!(*thin.last().unwrap(), [9999.0, 0.0, 0.0]);
        assert!(parse_lin("garbage\n1 2 3 - nan 5 6\n").is_empty());

        let dir = std::env::temp_dir().join(format!("ls-compile-leak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.pts"), PTS).unwrap();
        let (p, file) = read_leak(&dir.join("m")).unwrap();
        assert_eq!((p.len(), file.extension().unwrap().to_str().unwrap()), (200, "pts"));
        std::fs::write(dir.join("m.lin"), LIN).unwrap();
        assert_eq!(read_leak(&dir.join("m")).unwrap().0.len(), 8);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn checksums() {
        let dir = std::env::temp_dir().join(format!("ls-compile-sum-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("abc.zip");
        std::fs::write(&f, b"abc").unwrap();
        assert!(checksum_ok(&f, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert!(checksum_ok(&f, "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"));
        assert!(!checksum_ok(&f, SDHLT_SHA256));
        assert!(!checksum_ok(&dir.join("nothere.zip"), SDHLT_SHA256));
        // a file that isn't the pinned release is refused before anything is unpacked
        let dest = dir.join("v1.3.1");
        let e = install_zip(&f, &dest).unwrap_err();
        assert!(e.contains("isn't the file"), "{e}");
        assert!(!dest.exists());
        assert!(!is_installed(&dest));
        // a folder with every file but no marker isn't trusted
        for r in required_files() {
            let p = dest.join(r);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        assert!(!is_installed(&dest));
        std::fs::write(dest.join(MARKER), json!({"sha256": SDHLT_SHA256}).to_string()).unwrap();
        assert!(is_installed(&dest));
        std::fs::remove_file(dest.join("tools/Win64/sdHLRAD_x64.exe")).unwrap();
        assert!(!is_installed(&dest));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(SDHLT_SHA256.len(), 64);
    }

    #[test]
    fn command_lines() {
        // Windows: the x64 exe directly; -low and the options first, the map path last
        let w = native("C:\\Users\\a\\AppData\\Roaming\\Loner-Strike\\tools\\sdhlt\\v1.3.1\\tools");
        let map = Path::new("C:\\work\\room\\room");
        let (prog, args) = w.command_line(Stage::Csg, Preset::Normal, map, &wadinclude_args(&["mine.wad".into()])).unwrap();
        assert!(prog.to_string_lossy().ends_with("sdHLCSG_x64.exe"));
        assert!(prog.to_string_lossy().contains("Win64"));
        assert_eq!(args, vec!["-low", "-wadinclude", "mine.wad", "C:\\work\\room\\room"]);
        let (_, args) = w.command_line(Stage::Rad, Preset::Fast, map, &[]).unwrap();
        assert_eq!(args, vec!["-low", "-fast", "C:\\work\\room\\room"]);
        let (_, args) = w.command_line(Stage::Rad, Preset::Final, map, &[]).unwrap();
        assert!(args.contains(&"-extra".to_string()));
        let (_, args) = w.command_line(Stage::Vis, Preset::Final, map, &[]).unwrap();
        assert!(args.contains(&"-full".to_string()));
        let (_, args) = w.command_line(Stage::Vis, Preset::Normal, map, &[]).unwrap();
        assert_eq!(args, vec!["-low", "C:\\work\\room\\room"]);
        // a thread cap
        let mut wt = w.clone();
        wt.threads = Some(6);
        let (_, args) = wt.command_line(Stage::Rad, Preset::Normal, map, &[]).unwrap();
        assert_eq!(args, vec!["-low", "-threads", "6", "C:\\work\\room\\room"]);
        let mut w32 = w.clone();
        w32.x64 = false;
        assert!(w32.exe(Stage::Bsp).ends_with("Win32/sdHLBSP.exe") || w32.exe(Stage::Bsp).to_string_lossy().ends_with("sdHLBSP.exe"));

        // macOS: CrossOver's wine with the bottle, everything as C:\ paths, the cap after the exe
        let mut m = wine();
        m.threads = Some(8);
        let base = m.work.join("room").join("room");
        let (prog, args) = m.command_line(Stage::Bsp, Preset::Normal, &base, &[]).unwrap();
        assert_eq!(prog, PathBuf::from(crate::servers::CROSSOVER_WINE));
        assert_eq!(
            args,
            vec!["--bottle", "Steam", "--wait-children", "C:\\Loner-Strike\\Compiler\\sdhlt-v1.3.1\\tools\\Win64\\sdHLBSP_x64.exe", "-low", "-threads", "8", "C:\\Loner-Strike\\Compiler\\work\\room\\room"]
        );
        // a path outside the bottle can't be given to wine
        assert!(m.arg_path(Path::new("/Users/x/elsewhere/a.wad")).is_err());

        // the presets' stages
        assert_eq!(Preset::Fast.stages(), vec![Stage::Csg, Stage::Bsp, Stage::Rad]);
        assert_eq!(Preset::Final.stages().len(), 4);
    }

    #[test]
    fn threads_and_priority() {
        // all but two cores by default, at least one; all of them when asked
        assert_eq!(compile_threads(12, false), 10);
        assert_eq!(compile_threads(12, true), 12);
        assert_eq!(compile_threads(2, false), 1);
        assert_eq!(compile_threads(1, false), 1);
        assert_eq!(compile_threads(0, true), 1);
        assert!(cores() >= 1);
        assert_eq!(BELOW_NORMAL_PRIORITY_CLASS, 0x4000);
        assert!(NICE > 0);
        // the child really starts below normal priority (Unix: its nice value)
        #[cfg(unix)]
        {
            let mut c = std::process::Command::new("/bin/sh");
            c.args(["-c", "ps -o nice= -p $$"]).stdout(std::process::Stdio::piped());
            low_priority(&mut c);
            let out = c.output().unwrap();
            let n: i32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
            assert!(n >= NICE, "nice {n}");
        }
    }

    #[test]
    fn bottles() {
        let dir = std::env::temp_dir().join(format!("ls-compile-bottle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(bottle_is_64(&dir));
        std::fs::write(dir.join("cxbottle.conf"), "[Bottle]\n\"Template\" = \"win10_64\"\n\"WineArch\" = \"win64\"\n").unwrap();
        assert!(bottle_is_64(&dir));
        std::fs::write(dir.join("cxbottle.conf"), ";; \"WineArch\" = \"win32\" is a comment\n\"WineArch\" = \"win32\"\n").unwrap();
        assert!(!bottle_is_64(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wads_for_the_compiler() {
        let dir = std::env::temp_dir().join(format!("ls-compile-wads-{}", std::process::id()));
        let game = dir.join("Half-Life");
        std::fs::create_dir_all(game.join("valve")).unwrap();
        std::fs::write(game.join("valve/halflife.wad"), b"WAD3\0\0\0\0\0\0\0\0").unwrap();
        std::fs::write(game.join("valve/mine.wad"), b"WAD3\0\0\0\0\0\0\0\0").unwrap();
        let find = |n: &str| Some(game.join("valve").join(n.rsplit(['\\', '/']).next().unwrap())).filter(|p| p.is_file());
        let mut l = native(&dir.join("sdhlt/tools").to_string_lossy());
        let names = vec!["\\half-life\\valve\\HALFLIFE.WAD".to_string(), "D:\\sierra\\mine.wad".into(), "gone.wad".into(), "zhlt.wad".into()];
        let w = resolve_wads(&names, &find, &l, &dir.join("copies")).unwrap();
        assert_eq!(w.missing, vec!["gone.wad"]);
        assert_eq!(w.paths.len(), 3);
        assert!(w.paths[0].ends_with("halflife.wad") || w.paths[0].to_lowercase().ends_with("halflife.wad"));
        assert!(w.paths[2].ends_with("sdhlt.wad"));
        // only the custom WAD goes inside the map
        assert_eq!(w.include, vec!["mine.wad"]);
        assert_eq!(wadinclude_args(&w.include), vec!["-wadinclude", "mine.wad"]);
        // through wine, a WAD outside the bottle is copied next to the map first
        l.runner = Runner::Wine { wine: PathBuf::from("/wine"), bottle: "B".into() };
        let bottle_work = dir.join("Bottles/B/drive_c/Loner-Strike/Compiler/work/m");
        l.tools = dir.join("Bottles/B/drive_c/Loner-Strike/Compiler/sdhlt-v1.3.1/tools");
        let w = resolve_wads(&names[..2], &find, &l, &bottle_work.join("wads")).unwrap();
        assert_eq!(w.paths[0], "C:\\Loner-Strike\\Compiler\\work\\m\\wads\\HALFLIFE.WAD");
        assert!(bottle_work.join("wads/mine.wad").is_file());
        assert_eq!(w.paths[2], "C:\\Loner-Strike\\Compiler\\sdhlt-v1.3.1\\tools\\sdhlt.wad");
        // and the .map names exactly those
        let text = map_text(&room(), &w.paths).unwrap();
        assert!(text.contains("\"wad\" \"C:\\Loner-Strike\\Compiler\\work\\m\\wads\\HALFLIFE.WAD;"), "{}", &text[..300]);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// QA-1/QA-2: a WAD kept beside the project (or named relative to it) is found, copied for
    /// the compiler and packed into the map; an empty one is said plainly.
    #[test]
    fn wads_beside_the_project() {
        let dir = std::env::temp_dir().join(format!("ls-compile-projwads-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = dir.join("projects/regamedll");
        let game = dir.join("Half-Life/valve");
        std::fs::create_dir_all(project.join("textures")).unwrap();
        std::fs::create_dir_all(&game).unwrap();
        let wad3 = b"WAD3\x01\0\0\0\x0c\0\0\0";
        std::fs::write(project.join("regamedll_zedit.wad"), wad3).unwrap();
        std::fs::write(project.join("textures/extra.wad"), wad3).unwrap();
        std::fs::write(project.join("gunshop.wad"), b"").unwrap();
        std::fs::write(game.join("halflife.wad"), wad3).unwrap();
        let in_game = |b: &str| Some(game.join(b)).filter(|p| p.is_file());
        let roots = vec![game.clone()];
        // as Hammer writes them: someone else's drive, a path relative to the project, a stock one
        let names: Vec<String> = vec![
            "C:\\Users\\mapper\\regamedll_zedit.wad".into(),
            "textures\\extra.wad".into(),
            "\\half-life\\valve\\halflife.wad".into(),
            "gunshop.wad".into(),
            "nowhere.wad".into(),
        ];
        let folder = Some(project.as_path());
        let real = |p: PathBuf| std::fs::canonicalize(p).unwrap();
        assert_eq!(find_wad(&names[0], folder, &roots, &in_game), Some(real(project.join("regamedll_zedit.wad"))));
        assert_eq!(find_wad(&names[1], folder, &roots, &in_game), Some(real(project.join("textures/extra.wad"))));
        assert_eq!(find_wad(&names[2], folder, &roots, &in_game), Some(real(game.join("halflife.wad"))));
        assert_eq!(find_wad(&names[0], None, &roots, &in_game), None, "without the project's folder it isn't found (the old bug)");
        // through CrossOver: copied next to the map (wine can't see the project), packed into the .bsp
        let mut l = wine();
        let b = dir.join("Bottles/Steam/drive_c");
        l.tools = b.join("Loner-Strike/Compiler/sdhlt-v1.3.1/tools");
        let work = b.join("Loner-Strike/Compiler/work/regamedll");
        let find = |n: &str| find_wad(n, folder, &roots, &in_game);
        let w = resolve_wads(&names, &find, &l, &work.join("wads")).unwrap();
        assert_eq!(w.missing, vec!["nowhere.wad"]);
        assert_eq!(w.bad, vec![("gunshop.wad".to_string(), "it's empty (0 bytes)".to_string())]);
        assert_eq!(w.paths[0], "C:\\Loner-Strike\\Compiler\\work\\regamedll\\wads\\regamedll_zedit.wad");
        assert_eq!(w.paths[1], "C:\\Loner-Strike\\Compiler\\work\\regamedll\\wads\\extra.wad");
        assert!(work.join("wads/regamedll_zedit.wad").is_file());
        assert_eq!(w.include, vec!["regamedll_zedit.wad", "extra.wad"]);
        let e = wad_errors(&w, true).unwrap();
        assert!(e.contains("nowhere.wad") && e.contains("project folder") && e.contains("gunshop.wad (it's empty (0 bytes))"), "{e}");
        assert!(wad_errors(&Wads::default(), true).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A map's WAD list can come from the internet: nothing outside the map's folder and the
    /// game's folders is ever found, copied or packed; network paths are never touched.
    #[test]
    fn wad_list_tricks_are_refused() {
        let dir = std::env::temp_dir().join(format!("ls-compile-waddos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = dir.join("home/me/maps/proj");
        let game = dir.join("Half-Life/valve");
        let secret_dir = dir.join("home/me/.ssh");
        for d in [&project, &game, &secret_dir] {
            std::fs::create_dir_all(d).unwrap();
        }
        let wad3 = b"WAD3\x01\0\0\0\x0c\0\0\0";
        std::fs::write(secret_dir.join("id_rsa"), b"-----BEGIN OPENSSH PRIVATE KEY-----").unwrap();
        std::fs::write(secret_dir.join("id_rsa.wad"), wad3).unwrap();
        std::fs::write(project.join("fake.wad"), b"just some text, not a wad").unwrap();
        std::fs::write(game.join("halflife.wad"), wad3).unwrap();
        let roots = vec![game.clone()];
        let in_game = |b: &str| Some(game.join(b)).filter(|p| p.is_file());
        let folder = Some(project.as_path());
        let f = |n: &str| find_wad(n, folder, &roots, &in_game);
        // ../ out of the project
        assert_eq!(f("../../.ssh/id_rsa"), None);
        assert_eq!(f("..\\..\\.ssh\\id_rsa.wad"), None);
        // absolute paths outside the known places (then only the name is looked up, in the known places)
        assert_eq!(f(&secret_dir.join("id_rsa.wad").to_string_lossy()), None);
        assert_eq!(f("/etc/passwd"), None);
        // network paths, every spelling
        for n in ["\\\\evil\\share\\x.wad", "//evil/share/x.wad", "\\\\?\\UNC\\evil\\share\\x.wad", "\\/evil/x.wad"] {
            assert!(is_network_path(n), "{n}");
            assert_eq!(f(n), None, "{n}");
        }
        // a link in the project pointing out
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(secret_dir.join("id_rsa.wad"), project.join("link.wad")).unwrap();
            std::os::unix::fs::symlink(&secret_dir, project.join("textures")).unwrap();
            assert_eq!(f("link.wad"), None);
            assert_eq!(f("textures/id_rsa.wad"), None);
        }
        // an absolute path inside the game's folder is fine
        assert!(f(&game.join("halflife.wad").to_string_lossy()).is_some());
        // a file named .wad that isn't one is never copied or packed
        let mut l = wine();
        let b = dir.join("Bottles/Steam/drive_c");
        l.tools = b.join("Loner-Strike/Compiler/sdhlt-v1.3.1/tools");
        let work = b.join("Loner-Strike/Compiler/work/proj");
        let w = resolve_wads(&["fake.wad".to_string(), "../../.ssh/id_rsa".into(), "//evil/share/x.wad".into()], &|n: &str| f(n), &l, &work.join("wads")).unwrap();
        assert_eq!(w.bad.len(), 1);
        assert_eq!(w.missing.len(), 2);
        assert!(w.include.is_empty());
        assert!(!work.join("wads").exists(), "nothing was copied");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// P5's de_kobbl: hlcsg's chart counts planes before hlbsp throws most away; the last
    /// report for each limit counts.
    #[test]
    fn limits_use_the_last_report() {
        let d = room();
        let csg = "Object names  Objects/Maxobjs  Memory / Maxmem  Fullness\n\
                   planes         61780/32768     1235600/655360  (188.5%)\n\
                   texinfos         120/32767       4800/1310680  (  0.4%)\n";
        let p = parse_output(csg, &Index::of(&d), &d);
        assert_eq!(p.iter().filter(|x| x.kind == "limit").count(), 1);
        let mut problems = p.clone();
        let bsp = "Reduced 61780 planes to 14670\n";
        let later = chart_values(bsp);
        assert_eq!(later.len(), 1);
        assert!((later[0].1 - 44.77).abs() < 0.1, "{later:?}");
        settle_limits(&mut problems, &later);
        assert!(problems.iter().all(|x| x.kind != "limit"), "{problems:?}");
        // within one log the last row wins (and a row still over stays)
        let two = "planes 61780/32768 1/1 (188.5%)\nplanes 14670/32768 1/1 ( 44.8%)\nleafs 9000/8192 1/1 (109.9%)\n";
        let p = parse_output(two, &Index::of(&d), &d);
        let limits: Vec<&str> = p.iter().filter(|x| x.kind == "limit").map(|x| x.message.as_str()).collect();
        assert_eq!(limits.len(), 1);
        assert!(limits[0].contains("leafs"));
    }

    #[test]
    fn a_map_with_nothing_inside() {
        let d = room();
        let p = parse_output("Error: Portal file 'C:\\work\\carver1\\carver1.prt' does not exist, cannot vis the map\n", &Index::of(&d), &d);
        assert_eq!(p.len(), 1);
        assert!(p[0].message.contains("info_player_start"), "{:?}", p[0]);
    }

    #[test]
    fn names() {
        assert_eq!(map_name("Untitled"), "ls_newmap");
        assert_eq!(map_name("  My Cool Map!! "), "my_cool_map");
        assert_eq!(map_name("de_test-2"), "de_test-2");
        assert_eq!(map_name("ünïcode"), "n_code");
        assert_eq!(map_name(&"a".repeat(60)).len(), 31);
        assert!(valid_map_name("de_test"));
        assert!(!valid_map_name("../x") && !valid_map_name("a b") && !valid_map_name(""));
        // P5 (QA4-A): Windows device names and names of only _ and -
        for bad in ["con", "CON", "nul", "Aux", "prn", "com1", "COM9", "lpt3", "_", "-", "__-"] {
            assert!(!valid_map_name(bad), "{bad}");
        }
        assert!(map_name_problem("nul").unwrap().contains("Windows keeps that name"));
        assert!(map_name_problem("--").unwrap().contains("at least one letter"));
        assert_eq!(map_name("CON"), "ls_con");
        assert!(valid_map_name(&map_name("-")) && valid_map_name(&map_name("Nul")));
        for ok in ["con_rats", "com10", "aux_1", "nul2x", "de_dust2", "a"] {
            assert!(valid_map_name(ok), "{ok}");
        }
    }

    #[test]
    fn cordon() {
        let d = room();
        // a box around the func_wall's brush and the sun, not the walls' far side or the spawn
        let c = Cordon { min: [-64.0, -64.0, -8.0], max: [128.0, 128.0, 192.0] };
        let out = apply_cordon(&d, &c).unwrap();
        // the floor touches the box (kept), the west wall (x -272..-256) doesn't
        assert!(out.brushes.values().any(|b| b.faces[0].texture == "CRETE3_FLR04"));
        assert!(!out.brushes.values().any(|b| b.faces[0].texture == "NOTEXISTS"));
        // six sky slabs seal it
        assert_eq!(out.brushes.values().filter(|b| b.faces[0].texture == "SKY").count(), 6);
        assert!(out.brushes.values().all(crate::editor::geom::is_valid));
        let classes: Vec<&str> = out.entities.values().map(|e| e.classname()).collect();
        assert!(classes.contains(&"light_environment") && classes.contains(&"func_wall"));
        assert!(!classes.contains(&"info_player_start"), "{classes:?}");
        // the original ids survive, so problems still point at the editor's brushes
        for id in out.brushes.keys().filter(|id| d.brushes.contains_key(id)) {
            assert_eq!(out.brushes[id], d.brushes[id]);
        }
        assert!(mapfile::write_map(&out, &mapfile::WriteOptions::default()).is_ok());
        // too small or too far out
        assert!(apply_cordon(&d, &Cordon { min: [0.0; 3], max: [8.0, 64.0, 64.0] }).is_err());
        assert!(apply_cordon(&d, &Cordon { min: [0.0; 3], max: [1e9, 64.0, 64.0] }).is_err());
        assert!(apply_cordon(&d, &Cordon { min: [0.0; 3], max: [f64::NAN, 64.0, 64.0] }).is_err());
    }

    #[test]
    fn cordon_seals_lights_boxes_links_and_spawns() {
        let mut d = room();
        let mut lamp = Entity::new("light");
        lamp.set("origin", "400 0 64");
        d.insert_entity(lamp);
        let mut far = Entity::new("light");
        far.set("origin", "5000 0 64");
        d.insert_entity(far);
        let mut btn = Entity::new("trigger_once");
        btn.set("target", "door1");
        let bt = d.insert_entity(btn);
        let mut tb = ops::make_box(v3(8.0, 8.0, 0.0), v3(24.0, 24.0, 16.0), "AAATRIGGER");
        tb.entity = Some(bt);
        d.insert_brush(tb);
        let mut door = Entity::new("func_door");
        door.set("targetname", "door1");
        let de = d.insert_entity(door);
        let mut db = ops::make_box(v3(1500.0, 0.0, 0.0), v3(1504.0, 64.0, 112.0), "LAB1_DOOR1");
        db.entity = Some(de);
        d.insert_brush(db);
        let c = Cordon { min: [-64.0, -64.0, -8.0], max: [128.0, 128.0, 192.0] };

        // black seal: BLACK slabs, halflife.wad listed once, no sun from the sky
        let mut d2 = d.clone();
        d2.world.wads.retain(|w| !w.contains("halflife"));
        let out = apply_cordons(&d2, &[c], &CordonOptions { seal: Seal::Black, ..Default::default() }).unwrap();
        assert_eq!(out.doc.brushes.values().filter(|b| b.faces[0].texture == "BLACK").count(), 6);
        assert_eq!(out.doc.world.wad_names().iter().filter(|w| *w == "halflife.wad").count(), 1);
        assert!(mapfile::write_map(&out.doc, &mapfile::WriteOptions::default()).is_ok());

        // lights: dropped by default, kept (box grown) when asked; the far one never
        let plain = apply_cordons(&d, &[c], &CordonOptions::default()).unwrap();
        let lights = |m: &MapDocument| m.entities.values().filter(|e| e.classname() == "light").count();
        assert_eq!(lights(&plain.doc), 0);
        let kept = apply_cordons(&d, &[c], &CordonOptions { keep_lights: true, ..Default::default() }).unwrap();
        assert_eq!(lights(&kept.doc), 1);
        assert!(kept.boxes[0].max.x >= 400.0);
        assert!(kept.doc.brushes.values().all(crate::editor::geom::is_valid));

        // links across the edge: the trigger inside sets off a door outside
        assert!(plain.warnings.iter().any(|w| w.kind == "cordon" && w.entity == Some(bt.0) && w.message.contains("door1")), "{:?}", plain.warnings);

        // several boxes: apart they're sealed one by one, overlapping ones become one
        let c2 = Cordon { min: [1400.0, -64.0, -8.0], max: [1600.0, 128.0, 192.0] };
        let two = apply_cordons(&d, &[c, c2], &CordonOptions::default()).unwrap();
        assert_eq!(two.boxes.len(), 2);
        assert_eq!(two.doc.brushes.values().filter(|b| b.faces[0].texture == "SKY").count(), 12);
        assert!(two.warnings.is_empty(), "the door is inside the second box: {:?}", two.warnings);
        let c3 = Cordon { min: [100.0, -64.0, -8.0], max: [300.0, 128.0, 192.0] };
        assert_eq!(apply_cordons(&d, &[c, c3], &CordonOptions::default()).unwrap().boxes.len(), 1);
        assert!(apply_cordons(&d, &[], &CordonOptions::default()).is_err());

        // playtest spawns: at the camera when it's free, on a floor otherwise
        let sp = apply_cordons(&d, &[c], &CordonOptions { spawn: true, spawn_at: Some([64.0, 64.0, 100.0]), ..Default::default() }).unwrap();
        let starts: Vec<Vec3> = sp.doc.entities.values().filter(|e| e.classname() == "info_player_start").filter_map(origin_of).collect();
        assert!(!starts.is_empty() && starts.iter().all(|p| inside(&sp.boxes[0], *p)));
        assert!(starts.contains(&v3(64.0, 64.0, 100.0)));
        assert!(sp.doc.entities.values().any(|e| e.classname() == "info_player_deathmatch"));
        let sp2 = apply_cordons(&d, &[c], &CordonOptions { spawn: true, spawn_at: Some([9999.0, 0.0, 0.0]), ..Default::default() }).unwrap();
        let p = sp2.doc.entities.values().find(|e| e.classname() == "info_player_start").and_then(origin_of).unwrap();
        // standing on the floor (top at z 0)
        assert_eq!(p.z, 37.0);
        // the sun is moved inside when it was outside
        let mut d3 = d.clone();
        for e in d3.entities.values_mut().filter(|e| e.classname() == "light_environment") {
            e.set("origin", "3000 3000 3000");
        }
        let o3 = apply_cordons(&d3, &[c], &CordonOptions::default()).unwrap();
        let sun = o3.doc.entities.values().find(|e| e.classname() == "light_environment").and_then(origin_of).unwrap();
        assert!(inside(&o3.boxes[0], sun));
    }

    #[test]
    fn installing_into_the_game() {
        let dir = std::env::temp_dir().join(format!("ls-compile-install-{}", std::process::id()));
        let maps = dir.join("cstrike/maps");
        let backups = dir.join("backups");
        std::fs::create_dir_all(&maps).unwrap();
        let src = dir.join("built.bsp");
        std::fs::write(&src, b"new map").unwrap();
        // under test, never the real game
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let real = home.join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/maps");
        assert!(install_bsp(&src, &real, "my_map", &backups).unwrap_err().contains("real game"));
        // nothing there: copied, no backup
        assert_eq!(install_bsp(&src, &maps, "my_map", &backups).unwrap(), None);
        assert_eq!(std::fs::read(maps.join("my_map.bsp")).unwrap(), b"new map");
        // the same again: nothing to back up
        assert_eq!(install_bsp(&src, &maps, "my_map", &backups).unwrap(), None);
        // a different one there: backed up first
        std::fs::write(maps.join("my_map.bsp"), b"older").unwrap();
        let b = install_bsp(&src, &maps, "my_map", &backups).unwrap().unwrap();
        assert_eq!(std::fs::read(&b).unwrap(), b"older");
        assert_eq!(std::fs::read(maps.join("my_map.bsp")).unwrap(), b"new map");
        // only the newest few backups stay
        for i in 0..8 {
            std::fs::write(maps.join("my_map.bsp"), format!("old {i}")).unwrap();
            std::thread::sleep(Duration::from_millis(2));
            install_bsp(&src, &maps, "my_map", &backups).unwrap();
        }
        let n = std::fs::read_dir(&backups).unwrap().count();
        assert_eq!(n, KEEP_BACKUPS);
        // never over the game's own maps, never a bad name
        assert!(install_bsp(&src, &maps, "de_dust2", &backups).unwrap_err().contains("game's own"));
        assert!(install_bsp(&src, &maps, "../evil", &backups).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn routes_answer_in_plain_words() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use std::sync::{Arc, RwLock};
        use tower::ServiceExt;
        let st: Shared = Arc::new(crate::server::AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let call = |method: &str, uri: &str, panel: bool, body: &str| {
            let mut r = Request::builder().method(method).uri(uri).header("host", "127.0.0.1:8027").header("content-type", "application/json");
            if panel {
                r = r.header("x-panel", "1");
            }
            let req = r.body(Body::from(body.to_string())).unwrap();
            let app = routes().with_state(st.clone());
            async move {
                let res = app.oneshot(req).await.unwrap();
                let s = res.status();
                let b = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
                (s, serde_json::from_slice::<Value>(&b).unwrap_or(Value::Null))
            }
        };
        let (s, v) = call("GET", "/api/tools/compile/status", false, "").await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["version"], SDHLT_VERSION);
        assert_eq!(v["bytes"], SDHLT_BYTES);
        assert_eq!(v["canRun"], false, "no game set up");
        // POSTs need the panel header
        let (s, _) = call("POST", "/api/tools/compile/start", false, "{}").await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        let (s, _) = call("POST", "/api/tools/compile/setup", false, "").await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        // nothing to stop; a compile without SDHLT or a map says why
        let (_, v) = call("POST", "/api/tools/compile/cancel", true, "").await;
        assert!(v["error"].as_str().is_some());
        let (_, v) = call("POST", "/api/tools/compile/start", true, r#"{"preset":"fast"}"#).await;
        assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()), "{v}");
        let (s, _) = call("POST", "/api/tools/compile/start", true, r#"{"preset":"ultra"}"#).await;
        assert!(s.is_client_error());
        let (s, v) = call("GET", "/api/tools/compile/progress?from=0", false, "").await;
        assert_eq!(s, StatusCode::OK);
        assert!(v.get("job").is_some());
    }

    /// P5 (share and remix), opt-in: the round trip. The starter room as a project, its editable
    /// source built as a package would carry it, read back by Remix, saved as the remix, opened
    /// (the same brushes and entities) and compiled for real. Needs the game and an installed
    /// SDHLT folder: `LS_TEST_GAME=<Half-Life folder> LS_TEST_SDHLT_DIR=<.../tools/sdhlt/v1.3.1>
    /// cargo test real_compile_of_a_remix -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_compile_of_a_remix() {
        let (Ok(hl), Ok(install)) = (std::env::var("LS_TEST_GAME"), std::env::var("LS_TEST_SDHLT_DIR")) else {
            println!("set LS_TEST_GAME and LS_TEST_SDHLT_DIR to run this");
            return;
        };
        let (hl, install) = (PathBuf::from(hl), PathBuf::from(install));
        assert!(is_installed(&install));
        let root = std::env::temp_dir().join(format!("ls-remix-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("proj")).unwrap();
        // publish with source: the project's source folder, as the package carries it
        let doc = super::super::editor::starter_map();
        let project = root.join("proj/ls_remix_src.lsmap");
        std::fs::write(&project, doc.to_json()).unwrap();
        let src = super::super::mapsource::build(&project, &hl, false, "ls_remix_src").unwrap();
        for (n, b) in &src.files {
            let p = root.join("pkg").join(n);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        }
        // remix: read back, saved with the credit, opened the same
        let (back, wad) = super::super::mapsource::read_package(&root.join("pkg")).unwrap();
        let credit = crate::editor::doc::RemixCredit { model: "Mod".into(), gb: 1, uid: String::new(), name: "Source".into(), author: "qa".into(), ..Default::default() };
        let path = super::super::mapsource::save_remix(&root.join("remixes"), "ls_remix_src", back, wad, credit).unwrap();
        let (opened, _) = super::super::editor::load_file(&path).unwrap();
        assert_eq!((opened.brushes.len(), opened.entities.len()), (doc.brushes.len(), doc.entities.len()));
        assert_eq!(opened.brushes.values().map(|b| b.faces.len()).sum::<usize>(), doc.brushes.values().map(|b| b.faces.len()).sum::<usize>());
        // and it compiles
        let crossover = hl.to_string_lossy().contains("/Bottles/");
        let bottle = crossover.then(|| {
            let s = hl.to_string_lossy().to_string();
            let i = s.find("/Bottles/").unwrap() + 9;
            s[i..].split('/').next().unwrap().to_string()
        });
        let client = crate::steam::Install { hl: hl.clone(), kind: if crossover { "crossover".into() } else { "windows".into() }, bottle, games: vec!["cstrike".into()], dedicated: false };
        let mut layout = layout_for(&client, &install, true).unwrap();
        layout.work = layout.work.join(format!("test-remix-{}", std::process::id()));
        layout.threads = Some(compile_threads(cores(), false));
        let dirs = crate::goldsrc::wad::search_dirs(&hl, false);
        let p = Plan { layout: layout.clone(), install_from: install.clone(), name: "ls_remix_test".into(), preset: Preset::Fast, doc: opened, wad_dirs: dirs, maps: None, warnings: vec![], folder: path.parent().map(Path::to_path_buf), assets: None };
        begin("compile", "test").unwrap();
        let r = run_compile(p);
        let problems = with_job(|j| j.problems.clone()).unwrap();
        finish(r.clone());
        let v = r.unwrap_or_else(|e| panic!("{e}\n{problems:#?}")).unwrap();
        let bsp = PathBuf::from(v["bsp"].as_str().unwrap());
        assert!(crate::goldsrc::bsp::Bsp::open(&bsp, false).is_ok());
        assert!(problems.iter().all(|p| p.level != "error"), "{problems:#?}");
        let _ = std::fs::remove_dir_all(&layout.work);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Opt-in: a real compile of the editor's starter room. Needs the game and SDHLT:
    /// `LS_TEST_GAME=<Half-Life folder> LS_TEST_SDHLT=<sdhlt-v1.3.1.zip> cargo test real_compile -- --ignored --nocapture`.
    /// On a Mac the game must be in a CrossOver bottle (the compile runs there).
    #[test]
    #[ignore]
    fn real_compile_of_the_starter_room() {
        let (Ok(hl), Ok(zip)) = (std::env::var("LS_TEST_GAME"), std::env::var("LS_TEST_SDHLT")) else {
            println!("set LS_TEST_GAME and LS_TEST_SDHLT to run this");
            return;
        };
        let hl = PathBuf::from(hl);
        let root = std::env::temp_dir().join(format!("ls-compile-real-{}", std::process::id()));
        let install = root.join("sdhlt").join("v1.3.1");
        install_zip(Path::new(&zip), &install).unwrap();
        assert!(is_installed(&install));
        let crossover = hl.to_string_lossy().contains("/Bottles/");
        let bottle = crossover.then(|| {
            let s = hl.to_string_lossy().to_string();
            let i = s.find("/Bottles/").unwrap() + 9;
            s[i..].split('/').next().unwrap().to_string()
        });
        let client = crate::steam::Install { hl: hl.clone(), kind: if crossover { "crossover".into() } else { "windows".into() }, bottle, games: vec!["cstrike".into()], dedicated: false };
        let mut layout = layout_for(&client, &install, true).unwrap();
        layout.work = layout.work.join(format!("test-{}", std::process::id()));
        layout.threads = Some(compile_threads(cores(), false));
        let doc = super::super::editor::starter_map();
        let dirs = crate::goldsrc::wad::search_dirs(&hl, false);
        let p = Plan { layout: layout.clone(), install_from: install.clone(), name: "ls_test_room".into(), preset: Preset::Fast, doc, wad_dirs: dirs, maps: None, warnings: vec![], folder: None, assets: None };
        begin("compile", "test").unwrap();
        let r = run_compile(p);
        let (log, problems) = with_job(|j| (j.log.join("\n"), j.problems.clone())).unwrap();
        finish(r.clone());
        println!("{log}");
        let v = r.unwrap_or_else(|e| panic!("{e}\n{problems:#?}")).unwrap();
        let bsp = PathBuf::from(v["bsp"].as_str().unwrap());
        let bytes = std::fs::read(&bsp).unwrap();
        assert_eq!(&bytes[0..4], &30u32.to_le_bytes(), "a GoldSrc BSP (version 30)");
        assert!(crate::goldsrc::bsp::Bsp::open(&bsp, false).is_ok());
        assert!(problems.iter().all(|p| p.level != "error"), "{problems:#?}");
        let _ = std::fs::remove_dir_all(&layout.work);
        let _ = std::fs::remove_dir_all(&root);
    }
}
