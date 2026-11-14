//! The map doctor (Tools, Beta): what a map on this computer is missing or has broken, and the
//! fixes that are safe to make in one click.
//!
//! - `GET  /api/tools/doctor/:name`        the check (JSON, below)
//! - `POST /api/tools/doctor/:name/res`    writes (or brings up to date) the map's .res file,
//!   the list of files players joining a server download with the map
//! - `POST /api/tools/doctor/:name/copy`   `{files}`: copies files the map needs that are only
//!   in another game folder (a Condition Zero texture file for a CS 1.6 map, say) into the
//!   map's own game folder. Never replaces a file.
//!
//! What's checked: the map file itself (agent A's parser), its texture files and textures,
//! the models, sprites, sounds and sky its entities name (as mapcopy.rs and navpack/res.py
//! find them), its .res file, and the bots' nav and map knowledge. Files are looked for the
//! way the game looks: its own folder and its downloads folder, then cstrike (for Condition
//! Zero), then valve.
//!
//! Writes go to `<file>.part` and are renamed into place; a file that's replaced is kept as
//! `<file>.bak` the first time. Nothing is written while the game has that map loaded.

use super::mapinfo::{self, MapFile};
use super::{blocking, find_map, json_answer, locate, Fail};
use crate::goldsrc::{bsp, entities, lsv, nav, wad};
use crate::mapcopy::{self, Finder};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const IN_USE: &str = "The game has this map loaded right now. Change to another map (or close the game) and try again.";
const MAX_COPY: usize = 200;
/// Game folders a file may be found in when it isn't where the game looks.
const OTHER_FOLDERS: [&str; 7] = ["cstrike", "czero", "valve", "czeror", "cstrike_downloads", "czero_downloads", "valve_downloads"];

// ---- where the game looks ---------------------------------------------------------------

/// The folders the game reads a map's files from, in order (a CZ map: czero, then cstrike,
/// then valve; downloads folders after each game's own).
pub fn game_path(hl: &Path, game: &str) -> Vec<PathBuf> {
    let mut names = vec![game.to_string(), format!("{game}_downloads")];
    if game != "cstrike" {
        names.push("cstrike".into());
        names.push("cstrike_downloads".into());
    }
    names.push("valve".into());
    names.iter().map(|n| hl.join(n)).filter(|d| d.is_dir()).collect()
}

fn folder_name(p: &Path) -> String {
    p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
}

/// What kind of file a game path is, in words the page shows.
pub fn kind_of(rel: &str) -> &'static str {
    let l = rel.to_lowercase();
    if l.ends_with(".wad") {
        "texture file"
    } else if l.starts_with("gfx/env/") {
        "sky"
    } else if l.starts_with("models/") {
        "model"
    } else if l.starts_with("sprites/") {
        "sprite"
    } else if l.starts_with("sound/") {
        "sound"
    } else if l.starts_with("overviews/") {
        "overview"
    } else {
        "file"
    }
}

/// Where one file the map needs is.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// where the game looks (the folder's name); `stock`: on Valve's own lists for this game
    /// (is_stock), so every player has it and it doesn't go in the .res
    Here { folder: String, stock: bool },
    /// not where the game looks, but in another game folder here (that folder's name)
    Elsewhere { folder: String, path: PathBuf },
    Missing,
}

#[derive(Debug, Clone)]
pub struct FileCheck {
    pub rel: String,
    pub found: Found,
}

/// A file every player of `game` has: on Valve's own lists only (Steam's depot manifests for
/// Half-Life, CS 1.6 and Condition Zero: stock_valve.txt, stock_cstrike.txt, stock_czero.txt), never
/// "it's in the other game's folder too" (mods install into both, and a CZ prop isn't a CS 1.6
/// player's: QA4-T2). CS 1.6 plays on Half-Life's and its own; Condition Zero also falls back to
/// CS 1.6's (its liblist's fallback_dir), so CS 1.6's count there too. Texture files: Half-Life's
/// and CS 1.6's, and for Condition Zero its own czcs_/czde_ ones as well.
pub fn is_stock(rel: &str, game: &str) -> bool {
    let key = rel.trim_start_matches(['/', '\\']).replace('\\', "/").to_lowercase();
    let czero = game.eq_ignore_ascii_case("czero");
    if key.ends_with(".wad") {
        return mapcopy::stock_wad(&key) || (czero && !key.contains('/') && on_list(STOCK_CZERO, &key));
    }
    on_list(STOCK_VALVE, &key) || on_list(STOCK_CSTRIKE, &key) || (czero && on_list(STOCK_CZERO, &key))
}

const STOCK_VALVE: usize = 0;
const STOCK_CSTRIKE: usize = 1;
const STOCK_CZERO: usize = 2;

/// Whether a lower-case game path is on one of Valve's lists (read once).
fn on_list(which: usize, key: &str) -> bool {
    use std::collections::HashSet;
    use std::sync::OnceLock;
    static LISTS: OnceLock<[HashSet<&'static str>; 3]> = OnceLock::new();
    let read = |t: &'static str| t.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<HashSet<_>>();
    let lists = LISTS.get_or_init(|| [read(include_str!("stock_valve.txt")), read(include_str!("stock_cstrike.txt")), read(include_str!("stock_czero.txt"))]);
    lists[which].contains(key)
}

/// The game's own texture files by name (Half-Life's and CS 1.6's; Condition Zero's too for it).
pub fn stock_wad_names(game: &str) -> Vec<String> {
    use std::sync::OnceLock;
    static CZ: OnceLock<Vec<String>> = OnceLock::new();
    let mut v: Vec<String> = mapcopy::stock_wads().iter().map(|s| s.to_string()).collect();
    if game.eq_ignore_ascii_case("czero") {
        v.extend(CZ.get_or_init(|| include_str!("stock_czero.txt").lines().map(str::trim).filter(|l| l.ends_with(".wad") && !l.contains('/')).map(String::from).collect()).iter().cloned());
    }
    v
}

/// One of the games' own maps (CS 1.6's classics, Condition Zero's _cz maps): every player
/// has it and its files, so it needs no .res.
pub fn stock_map(name: &str, game: &str) -> bool {
    let l = name.to_lowercase();
    crate::maps::STOCK_CS.contains(&l.as_str()) || (game == "czero" && l.ends_with("_cz"))
}

/// The game a search path is for (its first folder: "czero" or "cstrike").
fn game_of(path: &[PathBuf]) -> String {
    path.first().map(|d| folder_name(d)).filter(|n| n == "czero").unwrap_or_else(|| "cstrike".into())
}

pub fn find_file(f: &Finder, hl: &Path, path: &[PathBuf], rel: &str) -> Found {
    for d in path {
        if f.find(d, rel).is_some() {
            let folder = folder_name(d);
            return Found::Here { stock: is_stock(rel, &game_of(path)), folder };
        }
    }
    for name in OTHER_FOLDERS {
        let d = hl.join(name);
        if path.contains(&d) {
            continue;
        }
        if let Some(p) = f.find(&d, rel) {
            return Found::Elsewhere { folder: name.into(), path: p };
        }
    }
    Found::Missing
}

// ---- the .res file ----------------------------------------------------------------------

/// The order files are listed in: texture files, sky, models, sprites, sounds, overviews.
fn res_order(rel: &str) -> usize {
    ["texture file", "sky", "model", "sprite", "sound", "overview"].iter().position(|k| *k == kind_of(rel)).unwrap_or(9)
}

/// The files that belong in the .res: everything the map names that's where the game looks,
/// except what every game has already. Sorted by kind, then name.
pub fn res_wanted(files: &[FileCheck]) -> Vec<String> {
    let mut v: Vec<String> = files.iter().filter(|c| matches!(c.found, Found::Here { stock: false, .. })).map(|c| c.rel.clone()).collect();
    v.sort_by(|a, b| res_order(a).cmp(&res_order(b)).then_with(|| a.to_lowercase().cmp(&b.to_lowercase())));
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// A new .res: the wanted files, then lines from the old one still worth keeping (files the
/// map's maker listed that exist here, a hand-added overview and the like).
pub fn res_text(map: &str, wanted: &[String], keep: &[String]) -> String {
    let mut out = format!("// {map}.res: the files players download with this map\r\n// Made by Loner-Strike's map doctor. It keeps lines you add when it updates this file.\r\n\r\n");
    let mut seen: Vec<String> = Vec::new();
    for r in wanted.iter().chain(keep.iter()) {
        if seen.iter().any(|s| s.eq_ignore_ascii_case(r)) {
            continue;
        }
        seen.push(r.clone());
        out.push_str(r);
        out.push_str("\r\n");
    }
    out
}

/// The .res as it stands: entries missing from it (wanted, not listed), and listed files not
/// on this computer.
pub struct ResCheck {
    pub exists: bool,
    pub listed: Vec<String>,
    pub missing: Vec<String>,
    pub broken: Vec<String>,
}

pub fn res_check(text: Option<&str>, wanted: &[String], exists_here: impl Fn(&str) -> bool) -> ResCheck {
    let listed = text.map(mapcopy::res_files).unwrap_or_default();
    let has = |r: &str| listed.iter().any(|l| l.eq_ignore_ascii_case(r));
    let missing = wanted.iter().filter(|w| !has(w)).cloned().collect();
    let broken = listed.iter().filter(|l| !exists_here(l)).cloned().collect();
    ResCheck { exists: text.is_some(), listed, missing, broken }
}

// ---- writing safely ---------------------------------------------------------------------

/// Writes `bytes` to `path`: to `<path>.part` first, then renamed over it. The file it
/// replaces is kept as `<path>.bak` the first time. Returns whether a .bak was made.
pub fn save_file(path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    // the app-wide history too (safety.rs): the .bak above is only the very first version
    let tool = if name.ends_with(".nav") { "Callouts" } else if path.to_string_lossy().replace('\\', "/").contains("/overviews/") { "Radar maker" } else { "Map doctor" };
    crate::safety::before(tool, &format!("Saved {name}"), path);
    let bak = path.with_file_name(format!("{name}.bak"));
    let mut backed = false;
    if path.is_file() && !bak.exists() {
        std::fs::copy(path, &bak)?;
        backed = true;
    }
    let part = path.with_file_name(format!("{name}.part"));
    std::fs::write(&part, bytes)?;
    crate::sys::rename_over(&part, path)?;
    Ok(backed)
}

/// Whether the player's game is running with this map loaded (its own server's state says
/// so). Asked only while the game is open.
pub fn map_in_use(game: &crate::game::Game, name: &str) -> bool {
    if !crate::sys::game_running() {
        return false;
    }
    let st = game.state();
    st["online"] == json!(true) && st["state"]["map"].as_str().is_some_and(|m| m.eq_ignore_ascii_case(name))
}

// ---- the check --------------------------------------------------------------------------

/// The map's raw entity text and textures (no pixels).
fn read_map_parts(bsp_path: &Path) -> crate::goldsrc::Result<(String, Vec<bsp::MipTex>)> {
    let l = bsp::read_lumps(bsp_path, &[bsp::LUMP_ENTITIES, bsp::LUMP_TEXTURES])?;
    let text = crate::rcon::latin1(&l[0]).trim_end_matches('\0').to_string();
    Ok((text, bsp::textures(&l[1], false)?))
}

/// Every file the map needs (texture files, sky, models, sprites, sounds) plus its overview
/// when there is one, each looked for.
pub fn file_checks(f: &Finder, hl: &Path, game: &str, name: &str, ents: &str) -> Vec<FileCheck> {
    let path = game_path(hl, game);
    let mut rels = mapcopy::entity_files(ents);
    // an overview isn't named by the map: it goes in the .res when there is one
    for ext in ["bmp", "tga", "txt"] {
        let rel = format!("overviews/{name}.{ext}");
        if path.iter().any(|d| f.find(d, &rel).is_some()) {
            rels.push(rel);
        }
    }
    rels.into_iter().map(|rel| FileCheck { found: find_file(f, hl, &path, &rel), rel }).collect()
}

fn file_json(c: &FileCheck) -> Value {
    let (status, folder) = match &c.found {
        Found::Here { folder, .. } => ("ok", Some(folder.clone())),
        Found::Elsewhere { folder, .. } => ("elsewhere", Some(folder.clone())),
        Found::Missing => ("missing", None),
    };
    json!({"path": c.rel, "kind": kind_of(&c.rel), "status": status, "folder": folder,
           "stock": matches!(c.found, Found::Here { stock: true, .. })})
}

fn head_of(p: &Path, n: u64) -> Option<Vec<u8>> {
    let mut b = Vec::new();
    std::fs::File::open(p).ok()?.take(n).read_to_end(&mut b).ok()?;
    Some(b)
}

/// The bots' nav and knowledge for the map.
fn bots_json(m: &MapFile) -> (Value, Value, usize) {
    let mut problems = 0;
    let bsp_size = std::fs::metadata(&m.bsp).map(|x| x.len()).ok();
    let mut parsed: Option<nav::Nav> = None;
    let nav_v = match m.sibling("nav") {
        None => {
            problems += 1;
            json!({"exists": false})
        }
        Some(p) => match crate::goldsrc::read_capped(&p, "navigation file", nav::MAX_NAV).map_err(|e| e.to_string()).and_then(|b| nav::Nav::parse(&b).map_err(|e| e.to_string())) {
            Err(e) => {
                problems += 1;
                json!({"exists": true, "file": folder_name(&p), "error": e})
            }
            Ok(n) => {
                let matches = bsp_size.map(|s| n.version < 4 || s == n.bsp_size as u64);
                if matches == Some(false) {
                    problems += 1;
                }
                let named = n.areas.iter().filter(|a| a.place != 0).count();
                let v = json!({"exists": true, "file": folder_name(&p), "version": n.version, "areas": n.areas.len(),
                               "places": n.places.len(), "namedAreas": named, "matchesMap": matches});
                parsed = Some(n);
                v
            }
        },
    };
    let lsv_v = match m.sibling("lsv") {
        None => json!({"exists": false}),
        Some(p) => match head_of(&p, lsv::HEAD_SIZE as u64) {
            Some(b) if b.len() >= 20 && u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == lsv::LSV_MAGIC => {
                let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
                let (key, count, stage) = (u(8), u(12), u(16));
                let fits = parsed.as_ref().map(|n| lsv::nav_key(n) == key && n.areas.len() as u32 == count);
                if fits == Some(false) {
                    problems += 1;
                }
                json!({"exists": true, "stage": lsv::STAGES.get(stage as usize).copied().unwrap_or("unknown"),
                       "done": stage as usize == lsv::STAGES.len() - 1, "fits": fits})
            }
            _ => {
                problems += 1;
                json!({"exists": true, "error": "It isn't a map knowledge file the bots can read."})
            }
        },
    };
    (nav_v, lsv_v, problems)
}

/// The GameBanana page the map was installed from, when it came from there.
fn gamebanana_of(game: &crate::game::Game, name: &str) -> Value {
    for (key, r) in game.store.records() {
        let url = r["url"].as_str().unwrap_or("");
        let has = r["maps"].as_array().into_iter().flatten().any(|m| m.as_str().is_some_and(|m| m.eq_ignore_ascii_case(name)));
        if has && key.parse::<i64>().is_ok_and(|k| k > 0) && url.starts_with("https://gamebanana.com/") {
            return json!({"url": url, "name": r["name"]});
        }
    }
    Value::Null
}

/// The whole check for one map (the page's answer, without the GameBanana link and the
/// in-use flag, which need the running app).
pub fn diagnose(m: &MapFile) -> Value {
    let f = Finder::default();
    let mut problems = 0usize;
    let size = std::fs::metadata(&m.bsp).map(|x| x.len()).ok();

    // the map file: the full parse finds what the game would choke on
    let bsp_error = bsp::Bsp::open(&m.bsp, false).err().map(|e| e.to_string());
    if bsp_error.is_some() {
        problems += 1;
    }
    let parts = read_map_parts(&m.bsp);
    let (ents_text, textures) = match &parts {
        Ok((t, x)) => (t.as_str(), x.as_slice()),
        Err(_) => ("", &[][..]),
    };
    let ents = entities::parse(ents_text);

    // texture files and textures, as the game finds them
    let path = game_path(&m.hl, &m.game);
    let wad_names = entities::wad_list(&ents);
    let wads = wad::WadSet::open(&f, &path, &wad_names);
    let all_dirs: Vec<PathBuf> = path.iter().cloned().chain(OTHER_FOLDERS.iter().map(|n| m.hl.join(n)).filter(|d| d.is_dir() && !path.contains(d))).collect();
    let wads_anywhere = wad::WadSet::open(&f, &all_dirs, &wad_names);
    let lite = mapinfo::Lite { textures: textures.to_vec(), entities: Vec::new(), models: Vec::new() };
    let sources = mapinfo::texture_sources(&lite, &wads);
    let missing_tex: Vec<String> = sources.iter().filter(|s| s.3 == wad::Source::Missing).map(|s| s.0.clone()).collect();
    let fixable_tex = missing_tex.iter().filter(|t| wads_anywhere.source_of(t) != wad::Source::Missing).count();
    let embedded = sources.iter().filter(|s| s.3 == wad::Source::Embedded).count();
    if !missing_tex.is_empty() {
        problems += 1;
    }

    // every file the entities name
    let files = file_checks(&f, &m.hl, &m.game, &m.name, ents_text);
    let file_problems = files.iter().filter(|c| !matches!(c.found, Found::Here { .. })).count();
    problems += file_problems;
    let wad_json: Vec<Value> = wad_names
        .iter()
        .map(|n| {
            let c = files.iter().find(|c| c.rel.eq_ignore_ascii_case(n));
            let mut v = c.map(file_json).unwrap_or_else(|| json!({"path": n, "status": "missing"}));
            let used = sources.iter().filter(|s| s.3 == wad::Source::Wad(n.clone())).count();
            v["textures"] = json!(used);
            if let Some((_, Err(e))) = wads.wads.iter().find(|(w, _)| w == n) {
                if e != "missing" {
                    v["error"] = json!(e);
                }
            }
            v
        })
        .collect();

    // the .res
    let wanted = res_wanted(&files);
    let res_path = m.sibling("res");
    let res_text_now = res_path.as_ref().and_then(|p| crate::sys::read_latin1(p).ok());
    let rc = res_check(res_text_now.as_deref(), &wanted, |r| path.iter().any(|d| f.find(d, r).is_some()));
    let stock = stock_map(&m.name, &m.game);
    if !stock && !rc.missing.is_empty() {
        problems += 1;
    }

    let (nav_v, lsv_v, bot_problems) = bots_json(m);
    problems += bot_problems;

    json!({
        "name": m.name, "game": m.game, "size": size,
        "title": crate::maps::title_from_bsp(&m.bsp, &m.name).unwrap_or_default(),
        "bsp": {"ok": bsp_error.is_none() && parts.is_ok(), "error": bsp_error.or_else(|| parts.as_ref().err().map(|e| e.to_string()))},
        "textures": {"total": sources.len(), "embedded": embedded, "missing": missing_tex, "fixable": fixable_tex, "wads": wad_json},
        "files": files.iter().filter(|c| kind_of(&c.rel) != "texture file").map(file_json).collect::<Vec<_>>(),
        "res": {"stockMap": stock, "exists": rc.exists, "listed": rc.listed.len(), "missing": rc.missing, "broken": rc.broken, "wanted": wanted.len()},
        "nav": nav_v, "lsv": lsv_v,
        "looksIn": path.iter().map(|d| folder_name(d)).collect::<Vec<_>>(),
        "problems": problems,
    })
}

// ---- fixes ------------------------------------------------------------------------------

/// Writes the map's .res: what it needs, plus the old file's lines for files that exist.
pub fn write_res(m: &MapFile) -> Result<Value, String> {
    // P5: under test (the editor's Playtest writes one), never into the real game
    super::studio::guard_real_game(&m.hl)?;
    // QA4-T2 L6: a stock map's own .res is Valve's, and every player has the map's files
    if stock_map(&m.name, &m.game) {
        return Err(format!("{} came with the game, so it needs no download list.", m.name));
    }
    let f = Finder::default();
    let (ents, _) = read_map_parts(&m.bsp).map_err(|e| format!("The map file can't be read: {e}"))?;
    let files = file_checks(&f, &m.hl, &m.game, &m.name, &ents);
    let wanted = res_wanted(&files);
    let path = game_path(&m.hl, &m.game);
    let dest = m.sibling("res").unwrap_or_else(|| m.bsp.with_file_name(format!("{}.res", m.name)));
    let old = crate::sys::read_latin1(&dest).ok();
    let keep: Vec<String> = old.as_deref().map(mapcopy::res_files).unwrap_or_default().into_iter().filter(|r| path.iter().any(|d| f.find(d, r).is_some())).collect();
    let dropped = old.as_deref().map(mapcopy::res_files).unwrap_or_default().len() - keep.len();
    let text = res_text(&m.name, &wanted, &keep);
    let listed = mapcopy::res_files(&text).len();
    let backed = save_file(&dest, text.as_bytes()).map_err(|e| format!("The file couldn't be saved in the maps folder ({e})."))?;
    let mut msg = if listed == 0 {
        format!("Saved {}.res. The map only uses files every player has, so the list is empty.", m.name)
    } else {
        format!("Saved {}.res: players joining get {listed} file{} with the map.", m.name, if listed == 1 { "" } else { "s" })
    };
    if dropped > 0 {
        msg.push_str(&format!(" Left out {dropped} line{} for files that aren't on this computer.", if dropped == 1 { "" } else { "s" }));
    }
    if backed {
        msg.push_str(" The old one is kept next to it as .res.bak.");
    }
    Ok(json!({"ok": true, "message": msg, "listed": listed}))
}

/// Copies the asked-for files that are only in another game folder into the map's game
/// folder (the same game path). Returns what was copied.
pub fn copy_files(m: &MapFile, asked: &[String]) -> Result<Value, String> {
    let f = Finder::default();
    let path = game_path(&m.hl, &m.game);
    let dest_root = m.hl.join(&m.game);
    let (mut copied, mut skipped): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    // each path once, whatever its case (QA4-T2 L7: duplicates were each echoed as skipped)
    let mut seen = std::collections::HashSet::new();
    let asked: Vec<&String> = asked.iter().filter(|r| seen.insert(r.to_lowercase())).collect();
    for &rel in asked.iter().take(MAX_COPY) {
        let Some(rel) = mapcopy::clean_rel(rel) else {
            skipped.push(rel.clone());
            continue;
        };
        let Found::Elsewhere { path: src, .. } = find_file(&f, &m.hl, &path, &rel) else {
            skipped.push(rel);
            continue;
        };
        let dest = dest_root.join(&rel);
        if dest.exists() {
            skipped.push(rel);
            continue;
        }
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make a folder for {rel} ({e})."))?;
        }
        // [Caps] a map's own files (the biggest WAD in the game is 38 MB; the parsers stop at 256 MB)
        let bytes = crate::caps::read_file(&src, &rel, crate::goldsrc::MAX_WAD).map_err(|e| format!("Couldn't copy {rel}: {e}"))?;
        save_file(&dest, &bytes).map_err(|e| format!("Couldn't save {rel} ({e})."))?;
        copied.push(rel);
    }
    if copied.is_empty() {
        return Err("Nothing needed copying: those files are where the game looks already, or aren't on this computer.".into());
    }
    let n = copied.len();
    Ok(json!({"ok": true, "copied": copied, "skipped": skipped,
              "message": format!("Copied {n} file{} into {}.", if n == 1 { "" } else { "s" }, m.game)}))
}

// ---- routes -----------------------------------------------------------------------------

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn api_check(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let mut v = diagnose(&m);
                v["gamebanana"] = gamebanana_of(&game, &m.name);
                v["inUse"] = json!(map_in_use(&game, &m.name));
                Ok(v)
            })
            .await
        }
    };
    json_answer(r)
}

/// A fix: the map found, not in use, then `fix`.
async fn fix_route(st: Shared, h: HeaderMap, name: String, fix: impl FnOnce(&MapFile) -> Result<Value, String> + Send + 'static) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                if map_in_use(&game, &m.name) {
                    return Err(Fail::new(StatusCode::CONFLICT, IN_USE));
                }
                fix(&m).map_err(|e| Fail::new(StatusCode::OK, e))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_res(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>) -> Response {
    fix_route(st, h, name, write_res).await
}

#[derive(Deserialize)]
struct CopyReq {
    #[serde(default)]
    files: Vec<String>,
}

async fn api_copy(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>, Json(req): Json<CopyReq>) -> Response {
    fix_route(st, h, name, move |m| copy_files(m, &req.files)).await
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/doctor/:name", get(api_check))
        .route("/api/tools/doctor/:name/res", post(api_res))
        .route("/api/tools/doctor/:name/copy", post(api_copy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::bsp::fixture;

    /// P5 (QA4-A, QA4-T2): with no Condition Zero installed, CS 1.6's and Half-Life's own files
    /// are stock (on Valve's lists, whatever the case or slashes); an added model isn't
    #[test]
    fn cs16_stock_without_czero() {
        let root = std::env::temp_dir().join(format!("ls-doctor-stock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        for rel in ["cstrike/models/v_ak47.mdl", "cstrike/models/player/gign/gign.mdl", "cstrike/sound/weapons/ak47-1.wav", "cstrike/models/zombie_custom.mdl", "cstrike/models/hostage.mdl", "valve/gfx/env/desertup.tga"] {
            put(hl.join(rel), b"x");
        }
        let f = Finder::default();
        let path = game_path(&hl, "cstrike");
        assert!(is_stock("models/V_AK47.mdl", "cstrike") && is_stock("\\models\\v_ak47.mdl", "cstrike"));
        for rel in ["models/v_ak47.mdl", "models/player/gign/gign.mdl", "sound/weapons/ak47-1.wav", "models/hostage.mdl", "gfx/env/desertup.tga"] {
            assert!(matches!(find_file(&f, &hl, &path, rel), Found::Here { stock: true, .. }), "{rel}");
        }
        assert!(matches!(find_file(&f, &hl, &path, "models/zombie_custom.mdl"), Found::Here { stock: false, .. }), "an added model");
        assert!(is_stock("halflife.wad", "cstrike") && is_stock("cs_dust.wad", "cstrike"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// QA4-T2: with Condition Zero installed next to CS 1.6, stock means on Valve's lists, never
    /// "it's in the other game too": a CZ prop on a CS 1.6 map isn't stock, nor is a mod's file
    /// copied into both games; CZ's own files and texture files are stock on a CZ map only
    #[test]
    fn stock_with_czero_installed() {
        let root = std::env::temp_dir().join(format!("ls-doctor-stockcz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        for rel in ["cstrike/models/v_ak47.mdl", "cstrike/models/hostage.mdl", "czero/models/gibs_crate.mdl", "czero/czde_aztec.wad", "cstrike/models/mod_thing.mdl", "czero/models/mod_thing.mdl", "valve/halflife.wad"] {
            put(hl.join(rel), b"x");
        }
        let f = Finder::default();
        let cs = game_path(&hl, "cstrike");
        let cz = game_path(&hl, "czero");
        // a CZ prop: stock on a CZ map, not on a CS 1.6 one (and not where CS 1.6 looks)
        assert!(is_stock("models/gibs_crate.mdl", "czero") && !is_stock("models/gibs_crate.mdl", "cstrike"));
        assert!(matches!(find_file(&f, &hl, &cz, "models/gibs_crate.mdl"), Found::Here { stock: true, .. }));
        assert!(matches!(find_file(&f, &hl, &cs, "models/gibs_crate.mdl"), Found::Elsewhere { .. }));
        // a mod's file in both games: in neither game's own list
        for path in [&cs, &cz] {
            assert!(matches!(find_file(&f, &hl, path, "models/mod_thing.mdl"), Found::Here { stock: false, .. }));
        }
        // CS 1.6's files are a CZ player's too (CZ falls back to cstrike)
        assert!(matches!(find_file(&f, &hl, &cz, "models/hostage.mdl"), Found::Here { stock: true, .. }));
        assert!(is_stock("models/v_ak47.mdl", "czero"));
        // CZ's own texture files: stock on a CZ map, custom on a CS 1.6 one
        assert!(is_stock("czde_aztec.wad", "czero") && is_stock("CZDE_AZTEC.WAD", "czero"));
        assert!(!is_stock("czde_aztec.wad", "cstrike"));
        assert!(stock_wad_names("czero").iter().any(|w| w == "czde_aztec.wad"));
        assert!(!stock_wad_names("cstrike").iter().any(|w| w == "czde_aztec.wad"));
        assert!(is_stock("halflife.wad", "czero") && !is_stock("my_custom.wad", "czero"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The test room with its own entity text (lump 0 moved to the end of the file).
    pub(crate) fn room_with(ents: &str) -> Vec<u8> {
        let mut b = fixture::build();
        while b.len() % 4 != 0 {
            b.push(0);
        }
        let at = b.len() as u32;
        let mut text = ents.as_bytes().to_vec();
        text.push(0);
        b[4..8].copy_from_slice(&at.to_le_bytes());
        b[8..12].copy_from_slice(&(text.len() as u32).to_le_bytes());
        b.extend(text);
        b
    }

    const ENTS: &str = "{\n\"classname\" \"worldspawn\"\n\"wad\" \"\\\\half-life\\\\cstrike\\\\test.wad;czonly.wad;gone.wad;cstrike.wad\"\n\"skyname\" \"mysky\"\n}\n\
{\n\"classname\" \"env_sprite\"\n\"model\" \"sprites/glow_x.spr\"\n}\n\
{\n\"classname\" \"cycler\"\n\"model\" \"models/props/barrel.mdl\"\n}\n\
{\n\"classname\" \"ambient_generic\"\n\"message\" \"ambience/wind_x.wav\"\n}\n\
{\n\"classname\" \"cycler\"\n\"model\" \"models/props/lost.mdl\"\n}\n\
{\n\"classname\" \"ambient_generic\"\n\"message\" \"!SENTENCE\"\n}\n";

    struct Fx {
        root: PathBuf,
        m: MapFile,
    }

    impl Drop for Fx {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn put(p: PathBuf, b: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b).unwrap();
    }

    /// A Half-Life folder: the map in cstrike/maps, test.wad in valve (has "{grate"),
    /// czonly.wad only in czero, the sky in cstrike_downloads, the barrel in cstrike, the
    /// sprite only in czero, the sound in valve; gone.wad and lost.mdl nowhere.
    fn fixture_game(tag: &str) -> Fx {
        let root = std::env::temp_dir().join(format!("ls-doctor-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        put(hl.join("cstrike/maps/de_doc.bsp"), &room_with(ENTS));
        put(hl.join("valve/test.wad"), &wad::build_wad(&[("{grate", fixture::miptex("{grate", true))]));
        put(hl.join("czero/czonly.wad"), &wad::build_wad(&[("other", fixture::miptex("other", true))]));
        put(hl.join("cstrike/cstrike.wad"), &wad::build_wad(&[]));
        for side in ["up", "dn", "lf", "rt", "ft", "bk"] {
            put(hl.join(format!("cstrike_downloads/gfx/env/mysky{side}.tga")), b"tga");
        }
        put(hl.join("cstrike/models/props/Barrel.mdl"), b"mdl");
        put(hl.join("czero/sprites/glow_x.spr"), b"spr");
        put(hl.join("valve/sound/ambience/wind_x.wav"), b"wav");
        let m = MapFile { name: "de_doc".into(), bsp: hl.join("cstrike/maps/de_doc.bsp"), game: "cstrike".into(), hl: hl.clone(), czero: false };
        Fx { root, m }
    }

    fn status_of(v: &Value, path: &str) -> String {
        let all: Vec<&Value> = v["files"].as_array().unwrap().iter().chain(v["textures"]["wads"].as_array().unwrap()).collect();
        all.iter().find(|f| f["path"].as_str().unwrap().eq_ignore_ascii_case(path)).map(|f| f["status"].as_str().unwrap().to_string()).unwrap_or_else(|| panic!("{path} not listed"))
    }

    #[test]
    fn finds_whats_missing_and_where() {
        let fx = fixture_game("find");
        let v = diagnose(&fx.m);
        assert_eq!(v["bsp"]["ok"], true, "{v}");
        assert_eq!(status_of(&v, "test.wad"), "ok");
        assert_eq!(status_of(&v, "czonly.wad"), "elsewhere");
        assert_eq!(status_of(&v, "gone.wad"), "missing");
        assert_eq!(status_of(&v, "models/props/barrel.mdl"), "ok");
        assert_eq!(status_of(&v, "models/props/lost.mdl"), "missing");
        assert_eq!(status_of(&v, "sprites/glow_x.spr"), "elsewhere");
        assert_eq!(status_of(&v, "sound/ambience/wind_x.wav"), "ok");
        assert_eq!(status_of(&v, "gfx/env/myskyup.tga"), "ok");
        // a sentence ("!SENTENCE") isn't a file
        assert!(!v["files"].to_string().contains("SENTENCE"));
        // the room's textures: brick embedded, {grate from test.wad
        assert_eq!(v["textures"]["embedded"], 1);
        assert_eq!(v["textures"]["missing"].as_array().unwrap().len(), 0);
        assert_eq!(v["nav"]["exists"], false);
        assert_eq!(v["res"]["exists"], false);
        assert!(v["problems"].as_u64().unwrap() >= 5);
    }

    #[test]
    fn res_lists_custom_files_only() {
        let fx = fixture_game("res");
        let f = Finder::default();
        let ents = read_map_parts(&fx.m.bsp).unwrap().0;
        let files = file_checks(&f, &fx.m.hl, "cstrike", "de_doc", &ents);
        let wanted = res_wanted(&files);
        // cstrike.wad (every game has it) and missing ones are left out; files someone put in
        // valve/ aren't Valve's (not on its list: QA4-T2), so they're listed
        assert!(!wanted.iter().any(|w| w == "cstrike.wad" || w.contains("lost") || w.contains("glow_x")), "{wanted:?}");
        assert!(wanted.iter().any(|w| w == "test.wad") && wanted.iter().any(|w| w.contains("wind_x")), "{wanted:?}");
        assert_eq!(wanted.iter().filter(|w| w.starts_with("gfx/env/mysky")).count(), 6, "{wanted:?}");
        assert_eq!(wanted.last().unwrap(), "sound/ambience/wind_x.wav", "sorted by kind: sounds last");

        let out = write_res(&fx.m).unwrap();
        assert_eq!(out["listed"], 9);
        let res = fx.m.hl.join("cstrike/maps/de_doc.res");
        let text = std::fs::read_to_string(&res).unwrap();
        assert_eq!(mapcopy::res_files(&text), wanted);
        assert!(!res.with_file_name("de_doc.res.bak").exists());
        assert_eq!(diagnose(&fx.m)["res"]["missing"].as_array().unwrap().len(), 0);

        // an old one: a hand-added line for a file that's here stays, one for a missing
        // file goes, and the old file is kept once
        put(fx.m.hl.join("cstrike/overviews/extra.txt"), b"x");
        std::fs::write(&res, "// mine\noverviews/extra.txt\nmodels/nothere.mdl\n").unwrap();
        let v = diagnose(&fx.m);
        assert_eq!(v["res"]["broken"], json!(["models/nothere.mdl"]));
        assert_eq!(v["res"]["missing"].as_array().unwrap().len(), 9);
        let out = write_res(&fx.m).unwrap();
        assert!(out["message"].as_str().unwrap().contains("Left out 1 line"), "{out}");
        let text = std::fs::read_to_string(&res).unwrap();
        assert!(text.contains("overviews/extra.txt") && !text.contains("nothere"));
        assert_eq!(std::fs::read_to_string(res.with_file_name("de_doc.res.bak")).unwrap(), "// mine\noverviews/extra.txt\nmodels/nothere.mdl\n");
        write_res(&fx.m).unwrap();
        // (the .bak is the first one, not the last)
        assert!(std::fs::read_to_string(res.with_file_name("de_doc.res.bak")).unwrap().contains("nothere"));
        assert!(!res.with_file_name("de_doc.res.part").exists());
    }

    /// QA4-T2 L6 and L7: a stock map's .res isn't rewritten; a path asked twice is copied or
    /// skipped once.
    #[test]
    fn stock_maps_keep_their_res_and_copies_dedupe() {
        let fx = fixture_game("stockres");
        let stock = MapFile { name: "de_dust2".into(), bsp: fx.m.bsp.clone(), ..fx.m.clone() };
        assert!(write_res(&stock).unwrap_err().contains("needs no download list"));
        let asked: Vec<String> = ["czonly.wad", "CZONLY.WAD", "gone.wad", "gone.wad", "Gone.wad"].iter().map(|s| s.to_string()).collect();
        let out = copy_files(&fx.m, &asked).unwrap();
        assert_eq!(out["copied"], json!(["czonly.wad"]));
        assert_eq!(out["skipped"], json!(["gone.wad"]));
    }

    #[test]
    fn copies_from_other_game_folders_only() {
        let fx = fixture_game("copy");
        let out = copy_files(&fx.m, &["czonly.wad".into(), "sprites/glow_x.spr".into(), "gone.wad".into(), "../escape.wad".into(), "test.wad".into()]).unwrap();
        assert_eq!(out["copied"], json!(["czonly.wad", "sprites/glow_x.spr"]));
        assert_eq!(out["skipped"].as_array().unwrap().len(), 3);
        assert!(fx.m.hl.join("cstrike/czonly.wad").is_file());
        assert!(fx.m.hl.join("cstrike/sprites/glow_x.spr").is_file());
        let v = diagnose(&fx.m);
        assert_eq!(status_of(&v, "czonly.wad"), "ok");
        // nothing left to copy
        assert!(copy_files(&fx.m, &["czonly.wad".into()]).is_err());
    }

    #[test]
    fn czero_maps_look_in_cstrike_too() {
        let fx = fixture_game("cz");
        let path: Vec<String> = game_path(&fx.m.hl, "czero").iter().map(|d| folder_name(d)).collect();
        assert_eq!(path, ["czero", "cstrike", "cstrike_downloads", "valve"]);
        let f = Finder::default();
        assert!(matches!(find_file(&f, &fx.m.hl, &game_path(&fx.m.hl, "czero"), "czonly.wad"), Found::Here { .. }));
    }

    /// Three areas, two with places.
    pub(crate) fn test_nav(bsp_size: u32) -> nav::Nav {
        let area = |id: u32, x: f32, y: f32, z: f32, place: u16| nav::Area {
            id, attr: 0, lo: [x, y, z], hi: [x + 100.0, y + 100.0, z], ne_z: z, sw_z: z,
            conns: Default::default(), hiding: vec![], approach: vec![], encounters: vec![], place,
        };
        let mut a = area(1, 0.0, 0.0, 0.0, 1);
        a.conns[1] = vec![2];
        let mut b = area(2, 100.0, 0.0, 0.0, 2);
        b.conns[3] = vec![1];
        b.conns[2] = vec![3];
        let mut c = area(3, 100.0, 100.0, -64.0, 0);
        c.conns[0] = vec![2];
        nav::Nav { version: 5, bsp_size, places: vec!["BombsiteA".into(), "Long".into()], areas: vec![a, b, c] }
    }

    #[test]
    fn nav_and_knowledge_are_checked() {
        let fx = fixture_game("nav");
        let size = std::fs::metadata(&fx.m.bsp).unwrap().len() as u32;
        let n = test_nav(size);
        std::fs::write(fx.m.hl.join("cstrike/maps/de_doc.nav"), n.write()).unwrap();
        let mut head = Vec::new();
        for v in [lsv::LSV_MAGIC, lsv::LSV_VERSION, lsv::nav_key(&n), n.areas.len() as u32, 4] {
            head.extend(v.to_le_bytes());
        }
        head.resize(lsv::HEAD_SIZE, 0);
        std::fs::write(fx.m.hl.join("cstrike/maps/de_doc.lsv"), &head).unwrap();
        let v = diagnose(&fx.m);
        assert_eq!(v["nav"]["matchesMap"], true);
        assert_eq!(v["nav"]["namedAreas"], 2);
        assert_eq!(v["lsv"]["fits"], true);
        assert_eq!(v["lsv"]["done"], true);
        // a damaged nav is an error in words, not a crash
        std::fs::write(fx.m.hl.join("cstrike/maps/de_doc.nav"), b"\xce\xfa\xed\xfe\x05\0\0\0junk").unwrap();
        let v = diagnose(&fx.m);
        assert!(v["nav"]["error"].as_str().is_some(), "{v}");
    }

    #[test]
    fn damaged_maps_are_reported() {
        let fx = fixture_game("bad");
        let mut b = room_with(ENTS);
        b.truncate(200);
        std::fs::write(&fx.m.bsp, b).unwrap();
        let v = diagnose(&fx.m);
        assert_eq!(v["bsp"]["ok"], false);
        assert!(v["bsp"]["error"].as_str().unwrap().len() > 5);
    }

    /// The real game: `LS_TEST_GAME=<Half-Life folder> cargo test real_doctor -- --nocapture`
    /// (LS_TEST_MAPS=a,b to pick maps; default a few).
    #[test]
    fn real_doctor() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = PathBuf::from(hl);
        let maps = std::env::var("LS_TEST_MAPS").unwrap_or_else(|_| "de_dust2,cs_rats2,surf_ski_2".into());
        for name in maps.split(',') {
            let Some(lm) = mapcopy::find_local(&Finder::default(), &hl, name, "cstrike") else {
                println!("{name}: not here");
                continue;
            };
            let m = MapFile { name: lm.name, bsp: lm.bsp, game: lm.game, hl: hl.clone(), czero: false };
            let t = std::time::Instant::now();
            let v = diagnose(&m);
            let bad: Vec<String> = v["files"].as_array().unwrap().iter().chain(v["textures"]["wads"].as_array().unwrap()).filter(|f| f["status"] != "ok").map(|f| format!("{}={}", f["path"], f["status"])).collect();
            println!("{name} ({} ms): problems {} bsp {} missing tex {} res {} nav {} lsv {} not ok {:?}", t.elapsed().as_millis(), v["problems"], v["bsp"], v["textures"]["missing"].as_array().unwrap().len(), v["res"], v["nav"], v["lsv"], bad);
        }
    }
}
