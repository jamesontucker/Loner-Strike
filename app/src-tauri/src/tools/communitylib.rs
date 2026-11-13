//! The community library: every creation installed from Community, so the app knows what came
//! from where. `<app dir>/community/<model>-<id>/meta.json` per creation (and, for a Game Mode
//! Studio mode, its graph beside it). The files themselves live where they always do: maps,
//! sprays and textures in the game (installed and recorded by gamebanana.rs/content.rs, removed
//! through them), prefabs in the editor's library, Studio modes in the game's mode list (built
//! here from the graph by the Studio's own compiler).
//!
//! Other pages read `GET /api/tools/community/library` to show a small "Community" chip (who made
//! it, a link to its page) and a Community filter: maps by name, modes by id, prefabs by id,
//! sprays and textures by page.
//!
//! - `GET  /api/tools/community/library`            what's installed, with "update available"
//! - `POST /api/tools/community/uninstall {key}`    removes it the way it was installed (the
//!   safety net keeps a copy of each file first)
//! - `POST /api/tools/community/update {key}`       the newer file, through the same checks as Try it
//! - `POST /api/tools/community/remix {key}`        a Studio mode as your own graph (credit kept)

use super::community::{Index, Kind, Model};
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

const TOOL: &str = "Community";
const MAX_ITEMS: usize = 2000;

pub fn lib_dir() -> PathBuf {
    crate::sys::app_dir().join("community")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Who made a creation a remix was made from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Credit {
    /// the original's permanent id (its mode uid), "" for older ones
    pub uid: String,
    pub model: Model,
    pub gb: i64,
    pub name: String,
    pub author: String,
}

/// One installed creation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Installed {
    pub kind: Kind,
    pub model: Model,
    pub gb: i64,
    /// the GameBanana file installed, and its MD5 as the page listed it
    pub file: i64,
    pub md5: String,
    pub name: String,
    pub author: String,
    /// the game folder it went into (cstrike / czero)
    pub game: String,
    /// the installer section (maps, sprays, textures) for kinds the store installs
    pub section: String,
    pub maps: Vec<String>,
    /// prefab ids in the editor's library ("mine:<id>")
    pub prefabs: Vec<String>,
    /// a Studio mode's id
    pub mode: String,
    pub installed: u64,
    /// the original, for a remix published from this computer
    pub remix_of: Option<Credit>,
    /// its permanent id (a Studio mode's uid), "" for older installs: updates and remixes match by it
    pub uid: String,
    /// the GameBanana member who uploaded the installed file (0: unknown, from an older install)
    pub owner_id: i64,
    /// a newer file waiting to go in: the map or mode is in use now (applied at the next map change)
    pub pending: i64,
    /// the file installed before the last update (Go back to the previous version)
    pub previous_file: i64,
}

impl Installed {
    pub fn key(&self) -> String {
        dir_name(self.model, self.gb)
    }
    pub fn url(&self) -> String {
        match self.model {
            Model::Mod => format!("https://gamebanana.com/mods/{}", self.gb),
            Model::Spray => format!("https://gamebanana.com/sprays/{}", self.gb),
        }
    }
}

/// The folder name for a page: `mod-123`, `spray-45`.
pub fn dir_name(model: Model, gb: i64) -> String {
    format!("{}-{gb}", if model == Model::Spray { "spray" } else { "mod" })
}

/// A key from the page (`mod-123`), checked.
pub fn parse_key(k: &str) -> Option<(Model, i64)> {
    let (m, id) = k.split_once('-')?;
    let model = Model::parse(m)?;
    let id: i64 = id.parse().ok().filter(|i| *i > 0 && *i < 100_000_000)?;
    (dir_name(model, id) == k).then_some((model, id))
}

pub fn load_all(dir: &Path) -> Vec<Installed> {
    let mut v: Vec<Installed> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| parse_key(&e.file_name().to_string_lossy()).is_some())
        .filter_map(|e| serde_json::from_slice::<Installed>(&std::fs::read(e.path().join("meta.json")).ok()?).ok())
        .filter(|i| i.gb > 0)
        .take(MAX_ITEMS)
        .collect();
    v.sort_by(|a, b| b.installed.cmp(&a.installed).then(a.name.cmp(&b.name)));
    v
}

pub fn load(dir: &Path, model: Model, gb: i64) -> Option<Installed> {
    serde_json::from_slice(&std::fs::read(dir.join(dir_name(model, gb)).join("meta.json")).ok()?).ok()
}

pub fn save(dir: &Path, it: &Installed) -> Result<(), String> {
    let d = dir.join(it.key());
    std::fs::create_dir_all(&d).map_err(|e| format!("Couldn't note the install ({e})."))?;
    crate::sys::write_atomic(&d.join("meta.json"), &serde_json::to_vec_pretty(it).unwrap_or_default()).map_err(|e| format!("Couldn't note the install ({e})."))
}

fn forget(dir: &Path, model: Model, gb: i64) {
    let d = dir.join(dir_name(model, gb));
    if let Ok(rd) = std::fs::read_dir(&d) {
        for e in rd.flatten() {
            crate::safety::before(TOOL, "remove a community creation", &e.path());
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

/// A newer file than the installed one: the index lists another file for the page (GameBanana's
/// file ids only grow), or the installed one's MD5 no longer matches what it lists.
pub fn newer(it: &Installed, idx: &Index) -> Option<i64> {
    let e = listed_entry(it, idx)?;
    if e.model != it.model || e.gb != it.gb {
        // (the same creation, now on another page: any listed file there is newer for us)
        return (e.file > 0).then_some(e.file);
    }
    if e.file > 0 && e.file > it.file {
        return Some(e.file);
    }
    None
}

/// The index entry for an installed creation: its own page, else (moved to another page) an entry
/// with the same permanent id *and the same owner*. uids are public, so a uid alone links nothing:
/// anyone could declare one, and their package would show as someone else's update. The list service
/// also refuses a uid registered by another member; this check holds even without it.
pub fn listed_entry<'a>(it: &Installed, idx: &'a Index) -> Option<&'a super::community::Item> {
    if let Some(e) = idx.items.iter().find(|e| e.model == it.model && e.gb == it.gb) {
        return Some(e);
    }
    if it.uid.is_empty() || it.owner_id <= 0 {
        return None;
    }
    idx.items.iter().find(|e| e.uid == it.uid && e.owner_id == it.owner_id && e.status != super::community::Status::Revoked)
}

/// Whether a library entry was pulled by the Loner-Strike team since it was installed.
pub fn pulled(it: &Installed, idx: &Index) -> bool {
    let k = format!("{}:{}", if it.model == Model::Spray { "spray" } else { "mod" }, it.gb);
    idx.revoked.iter().any(|r| *r == k) || idx.items.iter().any(|e| e.model == it.model && e.gb == it.gb && e.status == super::community::Status::Revoked)
}

/// The chip other pages show: who made it and where its page is.
pub fn chip(it: &Installed) -> Value {
    json!({"key": it.key(), "name": it.name, "author": it.author, "url": it.url(), "kind": it.kind})
}

/// The library as the pages read it: the list (with updates), plus lookups for the chips.
pub fn summary(items: &[Installed], idx: &Index) -> Value {
    let mut maps = BTreeMap::new();
    let mut modes = BTreeMap::new();
    let mut prefabs = BTreeMap::new();
    let mut pages = BTreeMap::new();
    let list: Vec<Value> = items
        .iter()
        .map(|it| {
            let c = chip(it);
            for m in &it.maps {
                maps.insert(m.to_ascii_lowercase(), c.clone());
            }
            if !it.mode.is_empty() {
                modes.insert(it.mode.clone(), c.clone());
            }
            for p in &it.prefabs {
                prefabs.insert(p.clone(), c.clone());
            }
            pages.insert(it.key(), c.clone());
            let mut v = serde_json::to_value(it).unwrap_or_default();
            v["key"] = json!(it.key());
            v["url"] = json!(it.url());
            v["update"] = json!(newer(it, idx).filter(|f| *f != it.pending));
            v["pulled"] = json!(pulled(it, idx));
            v["canGoBack"] = json!(it.previous_file > 0 || lib_dir().join(it.key()).join("previous").join("meta.json").is_file());
            v
        })
        .collect();
    let updates = list.iter().filter(|v| !v["update"].is_null()).count();
    let waiting = items.iter().filter(|i| i.pending > 0).count();
    json!({"items": list, "maps": maps, "modes": modes, "prefabs": prefabs, "pages": pages, "updates": updates, "waiting": waiting})
}

/// Removes an installed creation the way it went in. The store's own records (maps, sprays,
/// textures) say which files are its; each is kept by the safety net before it goes.
pub fn uninstall(game: Option<&crate::game::Game>, dir: &Path, it: &Installed) -> Result<Value, String> {
    let removed = remove_files(game, it)?;
    forget(dir, it.model, it.gb);
    Ok(json!({"ok": true, "removed": removed}))
}

/// Takes an installed creation's files out of the game (or the editor's library), keeping its
/// library entry (an update or a rollback puts another version in its place).
fn remove_files(game: Option<&crate::game::Game>, it: &Installed) -> Result<Vec<String>, String> {
    let mut removed: Vec<String> = Vec::new();
    match it.kind {
        Kind::Map | Kind::Spray | Kind::Texture => {
            let g = game.ok_or("Set up the game on this computer first.")?;
            let section = if it.section.is_empty() { it.kind.installer().unwrap_or("maps") } else { it.section.as_str() };
            // (the safety net keeps each file the store is about to delete)
            if section == "maps" {
                if let Some(r) = g.store.records().get(&it.gb.to_string()) {
                    for f in r["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) {
                        let p = g.store.maps_dir().parent().map(|d| d.join(f));
                        if let Some(p) = p.filter(|p| p.is_file()) {
                            crate::safety::before(TOOL, "remove a community creation", &p);
                        }
                    }
                }
            }
            match g.store.uninstall_section(section, it.gb) {
                Ok(v) => {
                    removed.extend(v["maps"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(String::from)));
                    removed.extend(v["files"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(String::from)));
                }
                // removed by hand (or in Get more) already: just forget it
                Err(e) if e.message().contains("isn't installed") => {}
                Err(e) => return Err(e.message()),
            }
        }
        Kind::Prefab => {
            let lib = super::editor::prefabs::lib_dir(&crate::sys::app_dir().join("editor"));
            for id in &it.prefabs {
                let Some(stem) = id.strip_prefix("mine:") else { continue };
                if !crate::editor::prefab::valid_id(stem) {
                    continue;
                }
                for ext in ["map", "json", "png"] {
                    let p = lib.join(format!("{stem}.{ext}"));
                    if p.is_file() {
                        crate::safety::remove(TOOL, "remove a community prefab", &p)?;
                        removed.push(format!("{stem}.{ext}"));
                    }
                }
            }
        }
        Kind::Mode => {
            if let Some(g) = game {
                if !it.mode.is_empty() && super::studio::installed(&g.dir, &it.mode) {
                    super::studio::uninstall(&g.dir, &it.mode)?;
                    removed.push(it.mode.clone());
                }
                // (its map kit, which the Studio put in the kits folder when it built the mode)
                let kit = crate::sys::app_dir().join("kits").join(format!("{}.kit.json", it.mode));
                if !it.mode.is_empty() && kit.is_file() && !super::studio::graphs_dir().join(format!("{}.graph.json", it.mode)).exists() {
                    crate::safety::before(TOOL, "remove a community mode's map kit", &kit);
                    let _ = std::fs::remove_file(kit);
                }
            }
        }
        Kind::Other => {}
    }
    Ok(removed)
}

/// Waits for a store install job, then notes the creation (only when it finished).
pub fn note_after_job(store: std::sync::Arc<crate::gamebanana::MapStore>, job: String, mut it: Installed) {
    std::thread::spawn(move || {
        for _ in 0..3600 {
            std::thread::sleep(Duration::from_secs(1));
            let Some(j) = store.job_list().into_iter().find(|j| j.id == job) else { continue };
            match j.state.as_str() {
                "done" => {
                    if it.maps.is_empty() {
                        it.maps = j.maps.clone();
                    }
                    it.installed = now();
                    let _ = save(&lib_dir(), &it);
                    return;
                }
                "error" => {
                    // an update that didn't go in: the previous version comes back
                    if let Some(g) = store_game(&store) {
                        let _ = restore_previous(Some(&g), None, &lib_dir(), &it.key());
                    }
                    return;
                }
                _ => {}
            }
        }
    });
}

fn store_game(store: &std::sync::Arc<crate::gamebanana::MapStore>) -> Option<std::sync::Arc<crate::game::Game>> {
    // (the store is the player's own game's)
    let _ = store;
    UPDATE_GAME.lock().ok().and_then(|g| g.clone())
}

/// The player's game, for jobs that end after their request did (set by the routes).
static UPDATE_GAME: std::sync::Mutex<Option<std::sync::Arc<crate::game::Game>>> = std::sync::Mutex::new(None);

// ---- Studio modes ------------------------------------------------------------------------------

/// The Studio graph inside an unpacked mode package: `modes/<id>/<id>.graph.json` (or the only
/// `*.graph.json` in it), parsed and checked like the Studio checks its own.
pub fn mode_graph(src: &Path) -> Result<super::studio::graph::Graph, String> {
    let found: Vec<PathBuf> = crate::archive::walk(src).into_iter().filter(|p| p.to_string_lossy().to_lowercase().ends_with(".graph.json")).collect();
    let p = match found.as_slice() {
        [one] => one.clone(),
        [] => return Err("There's no Game Mode Studio graph in it.".into()),
        _ => return Err("It has more than one Game Mode Studio graph; a mode package carries one.".into()),
    };
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX) > 2 << 20 {
        return Err("Its graph is too big to be a graph.".into());
    }
    let text = std::fs::read_to_string(&p).map_err(|_| "Its graph couldn't be read.".to_string())?;
    let g = super::studio::graph::parse(&text)?;
    let problems = super::studio::graph::validate(&g);
    if super::studio::graph::has_errors(&problems) {
        let first = problems.iter().find(|p| p.level == "error").map(|p| p.message.clone()).unwrap_or_default();
        return Err(format!("Its graph has problems the Studio won't build ({first})."));
    }
    Ok(g)
}

/// The `mode.uid` of the graph in an unpacked mode package ("" without one).
pub fn mode_graph_uid(src: &Path) -> String {
    crate::archive::walk(src)
        .into_iter()
        .find(|p| p.to_string_lossy().to_lowercase().ends_with(".graph.json"))
        .filter(|p| std::fs::metadata(p).map(|m| m.len() <= 2 << 20).unwrap_or(false))
        .and_then(|p| serde_json::from_slice::<Value>(&std::fs::read(p).ok()?).ok())
        .map(|v| v["mode"]["uid"].as_str().unwrap_or("").trim().to_ascii_lowercase())
        .filter(|u| super::community::valid_uid(u))
        .unwrap_or_default()
}

/// Why a community mode can't go in under its id: one of the game's modes, or one of your own
/// Studio graphs, already has it (a reinstall of the same creation is fine).
pub fn mode_clash(game_dir: &Path, graphs: &Path, lib: &[Installed], id: &str, model: Model, gb: i64) -> Option<String> {
    let ours = lib.iter().any(|i| i.kind == Kind::Mode && i.mode == id && i.model == model && i.gb == gb);
    if ours {
        return None;
    }
    if lib.iter().any(|i| i.kind == Kind::Mode && i.mode == id) {
        return Some(format!("Another community mode already uses the id \"{id}\". Remove it first."));
    }
    if graphs.join(format!("{id}.graph.json")).is_file() {
        return Some(format!("One of your own Game Mode Studio modes is called \"{id}\". Rename yours (or remove it) first."));
    }
    let modes = crate::extras::parse_modes(&crate::sys::read_latin1(&game_dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default());
    if modes.iter().any(|(m, _)| m == id) {
        return Some(format!("Your game already has a mode called \"{id}\"."));
    }
    None
}

/// Installs a community Studio mode: the graph kept in the library, then built and installed by
/// the Studio's compiler like one of your own (nothing compiled is ever downloaded).
pub fn install_mode(st: &Shared, src: &Path, mut it: Installed) -> Result<Value, String> {
    let g = mode_graph(src)?;
    let game = st.local_game().ok_or("Set up the game on this computer first.")?;
    let dir = lib_dir();
    let lib = load_all(&dir);
    if let Some(why) = mode_clash(&game.dir, &super::studio::graphs_dir(), &lib, &g.mode.id, it.model, it.gb) {
        return Err(why);
    }
    let r = super::studio::compile(st, g.clone(), false)?;
    if r["ok"] != json!(true) {
        let first = r["problems"].as_array().and_then(|p| p.iter().find(|x| x["level"] == "error")).and_then(|x| x["message"].as_str()).unwrap_or("the compiler stopped");
        return Err(format!("The mode couldn't be built on this computer ({first})."));
    }
    it.mode = g.mode.id.clone();
    if it.uid.is_empty() {
        it.uid = mode_graph_uid(src);
    }
    it.installed = now();
    save(&dir, &it)?;
    let gp = dir.join(it.key()).join(format!("{}.graph.json", g.mode.id));
    crate::sys::write_atomic(&gp, serde_json::to_string_pretty(&g).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't keep the graph ({e})."))?;
    Ok(json!({"ok": true, "kind": "mode", "mode": g.mode.id, "name": g.mode.name}))
}

// ---- remix ---------------------------------------------------------------------------------------

/// Remix credits: your Studio graph id → the creation it came from.
fn credits_path(dir: &Path) -> PathBuf {
    dir.join("remixes.json")
}

pub fn credits(dir: &Path) -> BTreeMap<String, Credit> {
    std::fs::read(credits_path(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// A free id for your copy: `<id>_remix`, `<id>_remix2`… (the Studio's id rules: 2-24 small
/// letters, digits, _).
pub fn remix_id(id: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base: String = id.chars().take(17).collect();
    let mut n = 1;
    loop {
        let c = if n == 1 { format!("{base}_remix") } else { format!("{base}_remix{n}") };
        if !taken(&c) {
            return c;
        }
        n += 1;
    }
}

/// Copies a community mode's graph into your Studio graphs under a new id and name, noting
/// where it came from. Returns the new id.
pub fn remix(dir: &Path, graphs: &Path, game_dir: Option<&Path>, it: &Installed) -> Result<(String, String), String> {
    if it.kind != Kind::Mode || it.mode.is_empty() {
        return Err("Only Game Mode Studio modes can be remixed.".into());
    }
    let src = dir.join(it.key()).join(format!("{}.graph.json", it.mode));
    // [Caps] the same 2 MB a graph may be when it's installed
    let text = crate::caps::read_text(&src, "mode's graph", 2 << 20).map_err(|e| if e.contains("too big") { e } else { "This mode's graph isn't in the library any more. Install it again.".to_string() })?;
    let mut raw: Value = serde_json::from_str(&text).map_err(|_| "This mode's graph couldn't be read.".to_string())?;
    let g0 = super::studio::graph::parse(&text)?;
    let modes = game_dir.map(|d| crate::extras::parse_modes(&crate::sys::read_latin1(&d.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default())).unwrap_or_default();
    let id = remix_id(&g0.mode.id, &|c| graphs.join(format!("{c}.graph.json")).exists() || modes.iter().any(|(m, _)| m == c));
    let name: String = format!("{} remix", g0.mode.name).chars().take(40).collect();
    raw["mode"]["id"] = json!(id);
    raw["mode"]["name"] = json!(name);
    // your copy is a new mode: its own permanent id (the original's goes in the credit)
    raw["mode"]["uid"] = json!(super::community::new_uid());
    let g = raw;
    std::fs::create_dir_all(graphs).map_err(|e| format!("Couldn't save your copy ({e})."))?;
    let path = graphs.join(format!("{id}.graph.json"));
    crate::sys::write_atomic(&path, serde_json::to_string_pretty(&g).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't save your copy ({e})."))?;
    let mut all = credits(dir);
    all.insert(id.clone(), Credit { uid: it.uid.clone(), model: it.model, gb: it.gb, name: it.name.clone(), author: it.author.clone() });
    std::fs::create_dir_all(dir).ok();
    crate::sys::write_atomic(&credits_path(dir), &serde_json::to_vec_pretty(&all).unwrap_or_default()).map_err(|e| format!("Couldn't note the credit ({e})."))?;
    Ok((id, name))
}


// ---- servers -------------------------------------------------------------------------------------

/// Which community maps and modes a dedicated server carries (keys of library entries).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct ServerPick {
    pub maps: Vec<String>,
    pub modes: Vec<String>,
    /// carried creations updated here that the server doesn't have yet (sent when it's not in use)
    pub stale: Vec<String>,
}

fn picks_path(dir: &Path, server: &str) -> PathBuf {
    dir.join("servers").join(format!("{server}.json"))
}

pub fn load_picks(dir: &Path, server: &str) -> ServerPick {
    std::fs::read(picks_path(dir, server)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_picks(dir: &Path, server: &str, p: &ServerPick) -> Result<(), String> {
    let path = picks_path(dir, server);
    std::fs::create_dir_all(path.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&path, &serde_json::to_vec_pretty(p).unwrap_or_default()).map_err(|e| format!("Couldn't save the choice ({e})."))
}

/// What changes on a server: (maps to copy, library entries whose maps go, modes to add, modes that go).
pub fn pick_changes<'a>(lib: &'a [Installed], old: &ServerPick, new: &ServerPick) -> (Vec<&'a Installed>, Vec<&'a Installed>, Vec<&'a Installed>, Vec<&'a Installed>) {
    let find = |k: &String, kind: Kind| lib.iter().find(|i| i.key() == *k && i.kind == kind);
    let add_maps = new.maps.iter().filter(|k| !old.maps.contains(k)).filter_map(|k| find(k, Kind::Map)).collect();
    let drop_maps = old.maps.iter().filter(|k| !new.maps.contains(k)).filter_map(|k| find(k, Kind::Map)).collect();
    let add_modes = new.modes.iter().filter(|k| !old.modes.contains(k)).filter_map(|k| find(k, Kind::Mode)).collect();
    let drop_modes = old.modes.iter().filter(|k| !new.modes.contains(k)).filter_map(|k| find(k, Kind::Mode)).collect();
    (add_maps, drop_maps, add_modes, drop_modes)
}

/// Puts a community Studio mode on a server: built on this computer first (the Studio's
/// compiler, so the server never gets anything but what the graph makes), then its plugin, its
/// plugins.ini/cfg and its modes.ini line in the server's folder.
fn mode_to_server(st: &Shared, it: &Installed, server_game: &Path) -> Result<(), String> {
    let gp = lib_dir().join(it.key()).join(format!("{}.graph.json", it.mode));
    let g = super::studio::graph::parse(&crate::caps::read_text(&gp, "mode's graph", 2 << 20).map_err(|e| if e.contains("too big") { e } else { format!("{}'s graph isn't in the library any more. Install it again.", it.name) })?)?; // [Caps]
    let r = super::studio::compile(st, g.clone(), false)?;
    if r["ok"] != json!(true) {
        return Err(format!("{} couldn't be built on this computer.", it.name));
    }
    let local = st.local_game().ok_or("Set up the game on this computer first.")?;
    let plugin = super::studio::gen::plugin_name(&g.mode.id);
    let am = server_game.join("addons/amxmodx");
    if !am.join("configs/gamemodes/modes.ini").is_file() {
        return Err("This server has no Game Mode Menu yet. Set it up (Mods) first.".into());
    }
    let dest = am.join("plugins").join(format!("{plugin}.amxx"));
    crate::safety::before(TOOL, "put a community mode on a server", &dest);
    std::fs::copy(local.dir.join("addons/amxmodx/plugins").join(format!("{plugin}.amxx")), &dest).map_err(|e| format!("Couldn't copy {} to the server ({e}).", it.name))?;
    super::studio::register_mode(&am.join("configs"), &g, &plugin)
}

/// Takes a community map off a server through the server's own map records (which also stops
/// FastDL offering it: FastDL serves what the server's records list).
fn map_off_server(server_id: &str, it: &Installed) -> Result<(), String> {
    let Some(g) = crate::servers::game_for(server_id, || None) else { return Ok(()) };
    match g.store.uninstall_section("maps", it.gb) {
        Ok(_) => Ok(()),
        Err(e) if e.message().contains("isn't installed") => Ok(()),
        Err(e) => Err(e.message()),
    }
}

/// Applies a server's new choice. Removals and modes happen now; maps to add are copied by the
/// app's map copy job (local servers: copied; Linux servers: copied, then uploaded), whose id
/// comes back for the page to follow.
pub fn apply_picks(st: &Shared, server: &crate::servers::Entry, new: ServerPick) -> Result<Value, String> {
    let dir = lib_dir();
    let lib = load_all(&dir);
    let old = load_picks(&dir, &server.id);
    let new = ServerPick {
        maps: new.maps.into_iter().filter(|k| lib.iter().any(|i| i.key() == *k && i.kind == Kind::Map)).collect(),
        modes: new.modes.into_iter().filter(|k| lib.iter().any(|i| i.key() == *k && i.kind == Kind::Mode)).collect(),
        stale: vec![],
    };
    let (add_maps, drop_maps, add_modes, drop_modes) = pick_changes(&lib, &old, &new);
    let game_dir = server.dir.join(&server.game);
    let mut problems: Vec<String> = Vec::new();
    let mut changed = false;
    for it in &drop_maps {
        match map_off_server(&server.id, it) {
            Ok(()) => changed = true,
            Err(e) => problems.push(format!("{}: {e}", it.name)),
        }
    }
    for it in &drop_modes {
        if super::studio::installed(&game_dir, &it.mode) {
            match super::studio::uninstall(&game_dir, &it.mode) {
                Ok(()) => changed = true,
                Err(e) => problems.push(format!("{}: {e}", it.name)),
            }
        }
    }
    let mut kept = new.clone();
    // (updates still to be sent stay owed for what the server keeps carrying)
    kept.stale = old.stale.iter().filter(|k| new.maps.contains(k) || new.modes.contains(k)).cloned().collect();
    for it in &add_modes {
        match mode_to_server(st, it, &game_dir) {
            Ok(()) => changed = true,
            Err(e) => {
                kept.modes.retain(|k| *k != it.key());
                problems.push(format!("{}: {e}", it.name));
            }
        }
    }
    save_picks(&dir, &server.id, &kept)?;
    let mut uploaded = None;
    if changed {
        crate::servers::mark_changed(&server.id);
        if server.remote.is_some() {
            uploaded = Some(crate::servers::sync(server, false).is_ok());
        }
    }
    let maps: Vec<String> = add_maps.iter().flat_map(|i| i.maps.clone()).collect();
    let job = if maps.is_empty() {
        None
    } else {
        let local = st.local_game().ok_or("Set up the game on this computer first.")?;
        match crate::mapcopy::start(local.hl.clone(), local.store.records(), maps, vec![server.id.clone()]) {
            Ok(j) => Some(j),
            Err(e) => {
                problems.push(e);
                None
            }
        }
    };
    Ok(json!({"ok": true, "job": job, "problems": problems, "uploaded": uploaded, "picks": kept}))
}

/// What a server carries and could carry: the library's maps and Studio modes.
pub fn server_view(server: &crate::servers::Entry) -> Value {
    let dir = lib_dir();
    let picks = load_picks(&dir, &server.id);
    let game_dir = server.dir.join(&server.game);
    let items: Vec<Value> = load_all(&dir)
        .iter()
        .filter(|i| i.kind == Kind::Map || i.kind == Kind::Mode)
        .map(|i| {
            let on = if i.kind == Kind::Map { picks.maps.contains(&i.key()) } else { picks.modes.contains(&i.key()) };
            let there = if i.kind == Kind::Map {
                !i.maps.is_empty() && i.maps.iter().all(|m| game_dir.join("maps").join(format!("{m}.bsp")).is_file() || game_dir.join("maps").join(format!("{}.bsp", m.to_lowercase())).is_file())
            } else {
                super::studio::installed(&game_dir, &i.mode)
            };
            json!({"key": i.key(), "kind": i.kind, "name": i.name, "author": i.author, "url": i.url(), "maps": i.maps, "mode": i.mode, "on": on, "there": there})
        })
        .collect();
    json!({"server": {"id": server.id, "name": server.name, "remote": server.remote.is_some(), "ready": server.ready, "game": server.game}, "items": items})
}

/// Takes an uninstalled creation off every server that carried it (best effort).
fn drop_everywhere(st: &Shared, key: &str) {
    let dir = lib_dir();
    for e in crate::servers::load() {
        let p = load_picks(&dir, &e.id);
        if p.maps.iter().any(|k| k == key) || p.modes.iter().any(|k| k == key) {
            let new = ServerPick { maps: p.maps.iter().filter(|k| *k != key).cloned().collect(), modes: p.modes.iter().filter(|k| *k != key).cloned().collect(), stale: p.stale.iter().filter(|k| *k != key).cloned().collect() };
            let _ = apply_picks(st, &e, new);
        }
    }
}

fn server_of(id: &str) -> Result<crate::servers::Entry, String> {
    crate::servers::get(id).filter(|e| e.ready).ok_or_else(|| "That server isn't set up (any more).".to_string())
}

async fn api_server(axum::extract::Path(id): axum::extract::Path<String>) -> Response {
    run(move || Ok(server_view(&server_of(&id)?))).await
}

async fn api_server_set(State(st): State<Shared>, h: HeaderMap, axum::extract::Path(id): axum::extract::Path<String>, Json(req): Json<ServerPick>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    run(move || {
        let e = server_of(&id)?;
        apply_picks(&st, &e, req)
    })
    .await
}


// ---- updates ---------------------------------------------------------------------------------------
//
// An update never pulls a map or mode out from under a game: in use here (the map loaded, the mode
// running) it waits (`pending`) and goes in at the next map change; servers carrying it get it when
// nobody's playing it there (`ServerPick::stale`). The version before is kept in
// `<key>/previous/` (its files, its graph, its meta) until the next update, so a failed update puts
// it straight back and "Go back to the previous version" is one click. Files another map still
// needs (listed in its .res) are kept when the old version goes, whoever installed them.

fn prev_dir(dir: &Path, key: &str) -> PathBuf {
    dir.join(key).join("previous")
}

/// Whether the creation is in use in the player's game right now (its map loaded, its mode running).
pub fn in_use_here(g: &crate::game::Game, it: &Installed) -> bool {
    if it.maps.iter().any(|m| super::doctor::map_in_use(g, m)) {
        return true;
    }
    it.kind == Kind::Mode && !it.mode.is_empty() && crate::sys::game_running() && crate::extras::current_mode(g) == it.mode
}

/// Whether a server is playing the creation with people on it (bots don't count).
pub fn busy_on(state: &Value, it: &Installed, current_mode: &str) -> bool {
    if state["online"] != json!(true) {
        return false;
    }
    let people = state["state"]["players"].as_array().map(|p| p.iter().filter(|x| x["bot"] != json!(true)).count()).unwrap_or(0);
    if people == 0 {
        return false;
    }
    let map = state["state"]["map"].as_str().unwrap_or("");
    it.maps.iter().any(|m| m.eq_ignore_ascii_case(map)) || (it.kind == Kind::Mode && !it.mode.is_empty() && current_mode == it.mode)
}

/// Every file other maps' .res lists (lower case, relative to the game folder), leaving out the
/// creation's own maps: shared files (a WAD, a model) another map needs stay when it's replaced.
pub fn needed_by_others(maps_dirs: &[PathBuf], own: &[String]) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for d in maps_dirs {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let Some(stem) = name.strip_suffix(".res") else { continue };
            if own.iter().any(|m| m.eq_ignore_ascii_case(stem)) || e.metadata().map(|m| m.len() > 256 << 10).unwrap_or(true) {
                continue;
            }
            for line in crate::sys::read_latin1(&e.path()).unwrap_or_default().lines() {
                let l = line.trim().trim_matches('"').replace('\\', "/").to_lowercase();
                if !l.is_empty() && !l.starts_with("//") {
                    out.insert(l);
                }
            }
        }
    }
    out
}

/// Keeps the installed version aside (`<key>/previous/`): its meta, the files its record lists,
/// its prefabs and its graph. Replaces what an earlier update kept.
pub fn keep_previous(game_root: Option<&Path>, records: &serde_json::Map<String, Value>, dir: &Path, it: &Installed) -> Result<(), String> {
    let pd = prev_dir(dir, &it.key());
    let _ = std::fs::remove_dir_all(&pd);
    std::fs::create_dir_all(pd.join("files")).map_err(|e| format!("Couldn't keep the version you have ({e})."))?;
    if let (Some(root), Some(r)) = (game_root, records.get(&it.gb.to_string())) {
        if it.kind == Kind::Map {
            for f in r["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) {
                let rel = f.replace('\\', "/");
                if rel.split('/').any(|p| p == ".." || p.is_empty()) {
                    continue;
                }
                let src = root.join(&rel);
                if src.is_file() {
                    let dst = pd.join("files").join(&rel);
                    if let Some(p) = dst.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    std::fs::copy(&src, &dst).map_err(|e| format!("Couldn't keep the version you have ({e})."))?;
                }
            }
        }
    }
    if it.kind == Kind::Prefab {
        let lib = super::editor::prefabs::lib_dir(&crate::sys::app_dir().join("editor"));
        for id in &it.prefabs {
            let Some(stem) = id.strip_prefix("mine:").filter(|s| crate::editor::prefab::valid_id(s)) else { continue };
            for ext in ["map", "json", "png"] {
                let p = lib.join(format!("{stem}.{ext}"));
                if p.is_file() {
                    let _ = std::fs::create_dir_all(pd.join("prefabs"));
                    let _ = std::fs::copy(&p, pd.join("prefabs").join(format!("{stem}.{ext}")));
                }
            }
        }
    }
    if it.kind == Kind::Mode && !it.mode.is_empty() {
        let g = dir.join(it.key()).join(format!("{}.graph.json", it.mode));
        if g.is_file() {
            let _ = std::fs::copy(&g, pd.join(format!("{}.graph.json", it.mode)));
        }
    }
    let rec = records.get(&it.gb.to_string()).cloned().unwrap_or(Value::Null);
    std::fs::write(pd.join("record.json"), serde_json::to_vec(&rec).unwrap_or_default()).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&pd.join("meta.json"), &serde_json::to_vec_pretty(it).unwrap_or_default()).map_err(|e| e.to_string())
}

/// Puts the kept version back (the current one must already be out): its files where they were
/// (never over a file that's there), its record, its prefabs, its graph (built again when a
/// Studio mode: `st` given). Returns the restored entry.
pub fn restore_previous(game: Option<&crate::game::Game>, st: Option<&Shared>, dir: &Path, key: &str) -> Result<Installed, String> {
    let pd = prev_dir(dir, key);
    let prev: Installed = std::fs::read(pd.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).ok_or("There's no earlier version kept.")?;
    if prev.kind == Kind::Map {
        let g = game.ok_or("Set up the game on this computer first.")?;
        let root = g.store.maps_dir().parent().map(Path::to_path_buf).ok_or("The game folder isn't there.")?;
        let mut copied: Vec<String> = Vec::new();
        for f in crate::archive::walk(&pd.join("files")) {
            let Ok(rel) = f.strip_prefix(pd.join("files")) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/");
            let dst = root.join(&rel);
            if !dst.exists() {
                if let Some(p) = dst.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                std::fs::copy(&f, &dst).map_err(|e| format!("Couldn't put the earlier version back ({e})."))?;
            }
            copied.push(rel);
        }
        let rec: Value = std::fs::read(pd.join("record.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        let info = json!({"name": prev.name, "author": prev.author, "url": prev.url()});
        let info = if rec.is_object() { rec.clone() } else { info };
        for m in &prev.maps {
            g.store.record_copy(&prev.gb.to_string(), &info, m, &copied, &[]).map_err(|e| format!("Couldn't note the earlier version ({e})."))?;
        }
    }
    if prev.kind == Kind::Prefab && pd.join("prefabs").is_dir() {
        let lib = super::editor::prefabs::lib_dir(&crate::sys::app_dir().join("editor"));
        let _ = std::fs::create_dir_all(&lib);
        for f in crate::archive::walk(&pd.join("prefabs")) {
            if let Some(n) = f.file_name() {
                let _ = std::fs::copy(&f, lib.join(n));
            }
        }
    }
    if prev.kind == Kind::Mode && !prev.mode.is_empty() {
        let gp = pd.join(format!("{}.graph.json", prev.mode));
        let text = std::fs::read_to_string(&gp).map_err(|_| "The earlier version's graph isn't kept.".to_string())?;
        crate::sys::write_atomic(&dir.join(key).join(format!("{}.graph.json", prev.mode)), text.as_bytes()).map_err(|e| e.to_string())?;
        if let Some(st) = st {
            let g = super::studio::graph::parse(&text)?;
            let r = super::studio::compile(st, g, false)?;
            if r["ok"] != json!(true) {
                return Err("The earlier version couldn't be built again.".into());
            }
        }
    }
    let mut back = prev.clone();
    back.pending = 0;
    back.previous_file = 0;
    save(dir, &back)?;
    let _ = std::fs::remove_dir_all(&pd);
    Ok(back)
}

/// Marks a creation as changed for every server carrying it (sent when nobody's playing it there).
fn mark_servers_stale(dir: &Path, key: &str) {
    for e in crate::servers::load() {
        let mut p = load_picks(dir, &e.id);
        if (p.maps.iter().any(|k| k == key) || p.modes.iter().any(|k| k == key)) && !p.stale.iter().any(|k| k == key) {
            p.stale.push(key.to_string());
            let _ = save_picks(dir, &e.id, &p);
        }
    }
}

/// Updates an installed creation to `file`: waits when it's in use; else keeps the version you have,
/// takes it out (keeping files other maps need), puts the new one in through Try it's checks, and
/// puts the old one back if the new one fails.
pub fn apply_update(st: &Shared, it: &Installed, file: i64) -> Result<Value, String> {
    let dir = lib_dir();
    let game = st.local_game();
    if let Some(g) = &game {
        *UPDATE_GAME.lock().unwrap_or_else(|e| e.into_inner()) = Some(g.clone());
        if in_use_here(g, it) {
            let mut w = it.clone();
            w.pending = file;
            save(&dir, &w)?;
            return Ok(json!({"ok": true, "state": "waiting", "message": format!("{} is in use right now: the new version goes in at the next map change.", it.name)}));
        }
    }
    let records = game.as_ref().map(|g| g.store.records()).unwrap_or_default();
    let root = game.as_ref().and_then(|g| g.store.maps_dir().parent().map(Path::to_path_buf));
    keep_previous(root.as_deref(), &records, &dir, it)?;
    // files another map's .res needs stay (they're kept aside above; put back after the removal)
    let keep: Vec<String> = if it.kind == Kind::Map {
        let dirs: Vec<PathBuf> = game.as_ref().map(|g| vec![g.dir.join("maps"), g.hl.join("cstrike").join("maps")]).unwrap_or_default();
        let others = needed_by_others(&dirs, &it.maps);
        records.get(&it.gb.to_string()).map(|r| r["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).filter(|f| others.contains(&f.to_lowercase())).map(String::from).collect()).unwrap_or_default()
    } else {
        vec![]
    };
    remove_files(game.as_deref(), it)?;
    if let Some(root) = &root {
        for rel in &keep {
            let src = prev_dir(&dir, &it.key()).join("files").join(rel);
            let dst = root.join(rel);
            if src.is_file() && !dst.exists() {
                let _ = std::fs::copy(&src, &dst);
            }
        }
    }
    let mut marked = it.clone();
    marked.pending = 0;
    marked.previous_file = it.file;
    save(&dir, &marked)?;
    match super::community::start_try(st, it.model, it.gb, Some(file), true) {
        Ok(mut v) => {
            // (the new entry is noted by the installer; it keeps where it came from)
            if let Some(mut now_it) = load(&dir, it.model, it.gb) {
                now_it.previous_file = it.file;
                let _ = save(&dir, &now_it);
            }
            mark_servers_stale(&dir, &it.key());
            v["state"] = json!(if v["job"].is_string() { "installing" } else { "done" });
            Ok(v)
        }
        Err(e) => {
            let _ = restore_previous(game.as_deref(), Some(st), &dir, &it.key());
            Err(format!("The new version couldn't be installed ({e}); you still have the one you had."))
        }
    }
}

/// Goes back to the version kept before the last update (or, without one kept here, installs
/// the previous file from its page again with the same checks).
pub fn go_back(st: &Shared, it: &Installed) -> Result<Value, String> {
    let dir = lib_dir();
    let game = st.local_game();
    if game.as_ref().is_some_and(|g| in_use_here(g, it)) {
        return Err(format!("{} is in use right now. Change the map (or mode) first.", it.name));
    }
    if prev_dir(&dir, &it.key()).join("meta.json").is_file() {
        remove_files(game.as_deref(), it)?;
        let back = restore_previous(game.as_deref(), Some(st), &dir, &it.key())?;
        mark_servers_stale(&dir, &it.key());
        return Ok(json!({"ok": true, "state": "done", "file": back.file}));
    }
    if it.previous_file > 0 {
        remove_files(game.as_deref(), it)?;
        let v = super::community::start_try(st, it.model, it.gb, Some(it.previous_file), true)?;
        mark_servers_stale(&dir, &it.key());
        return Ok(v);
    }
    Err("There's no earlier version to go back to.".into())
}

/// The creator's update notes from GameBanana (newest first): title, version, date, changes.
pub fn notes_of(v: &Value) -> Vec<Value> {
    v["_aRecords"]
        .as_array()
        .into_iter()
        .flatten()
        .take(5)
        .map(|r| {
            let changes: Vec<Value> = r["_aChangeLog"]
                .as_array()
                .into_iter()
                .flatten()
                .take(30)
                .map(|c| json!({"cat": crate::gamebanana::plain(c["cat"].as_str().unwrap_or(""), 30), "text": crate::gamebanana::plain(c["text"].as_str().unwrap_or(""), 300)}))
                .collect();
            json!({
                "title": crate::gamebanana::plain(r["_sName"].as_str().unwrap_or(""), 100),
                "version": crate::gamebanana::plain(r["_sVersion"].as_str().unwrap_or(""), 30),
                "date": r["_tsDateAdded"].as_i64().unwrap_or(0),
                "text": crate::gamebanana::plain(r["_sText"].as_str().unwrap_or(""), 800),
                "changes": changes,
            })
        })
        .collect()
}

/// One pass of the waiting work: updates waiting for their map/mode to be free here, and servers
/// that should get an updated creation when nobody's playing it there. Runs every 30 s.
pub fn tick(st: &Shared) {
    let dir = lib_dir();
    let lib = load_all(&dir);
    for it in lib.iter().filter(|i| i.pending > 0) {
        if st.local_game().is_some_and(|g| !in_use_here(&g, it)) {
            let _ = apply_update(st, it, it.pending);
        }
    }
    for e in crate::servers::load().into_iter().filter(|e| e.ready) {
        let mut p = load_picks(&dir, &e.id);
        if p.stale.is_empty() {
            continue;
        }
        let Some(g) = crate::servers::game_for(&e.id, || None) else { continue };
        let state = g.state();
        let mode = crate::extras::current_mode(&g);
        let game_dir = e.dir.join(&e.game);
        let mut maps: Vec<String> = Vec::new();
        let mut done: Vec<String> = Vec::new();
        for key in p.stale.clone() {
            let Some(it) = lib.iter().find(|i| i.key() == key) else {
                done.push(key);
                continue;
            };
            if busy_on(&state, it, &mode) {
                continue;
            }
            let ok = match it.kind {
                Kind::Map => map_off_server(&e.id, it).map(|_| maps.extend(it.maps.clone())).is_ok(),
                Kind::Mode => mode_to_server(st, it, &game_dir).is_ok(),
                _ => true,
            };
            if ok {
                done.push(key);
            }
        }
        if !maps.is_empty() {
            let Some(local) = st.local_game() else { continue };
            if crate::mapcopy::start(local.hl.clone(), local.store.records(), maps, vec![e.id.clone()]).is_err() {
                // (another job is running: the maps are tried again next time)
                done.retain(|k| lib.iter().find(|i| i.key() == *k).is_some_and(|i| i.kind != Kind::Map));
            }
        }
        if !done.is_empty() {
            p.stale.retain(|k| !done.contains(k));
            let _ = save_picks(&dir, &e.id, &p);
            crate::servers::mark_changed(&e.id);
            if e.remote.is_some() {
                let _ = crate::servers::sync(&e, false);
            }
        }
    }
}

static TICKING: std::sync::Once = std::sync::Once::new();

/// Starts the 30-second tick (once, with the first community request after the app starts).
fn start_tick(st: &Shared) {
    if cfg!(test) {
        return;
    }
    let st = st.clone();
    TICKING.call_once(move || {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            tick(&st);
        });
    });
}


// ---- origins: made by me / from the community / remixed ----------------------------------------------

/// Where things in the player's library came from, for the origin chips and filters on My library
/// pages: maps by lower-case name, modes by id, prefabs by id (`mine:<id>`). Each value is
/// `{kind: "mine" | "community" | "remix" | "file", author?, url?, name?, of?}`.
/// - community: the community library (this file);
/// - remix: a Studio graph with a remix credit (`remixes.json`), with what it's a remix of;
/// - mine: your other Studio graphs, maps you published (packages) or compiled in the map editor
///   (`<app dir>/tools/compile/made.json` when the compiler keeps one), prefabs saved in the editor;
/// - file: prefabs added from a file (P5's origin).
pub fn origins(lib: &[Installed], credits: &BTreeMap<String, Credit>, graphs: &Path, packages: &Path, made: &Path, prefabs: &[(String, crate::editor::prefab::PrefabMeta)]) -> Value {
    let mut maps = serde_json::Map::new();
    let mut modes = serde_json::Map::new();
    let mut pre = serde_json::Map::new();
    let mine = json!({"kind": "mine"});
    // maps you made: published packages, and the compiler's own list
    for e in std::fs::read_dir(packages).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_lowercase();
        if let Some(m) = n.strip_suffix("-loner-strike.zip").filter(|m| !m.ends_with("-mode")) {
            maps.insert(m.to_string(), mine.clone());
        }
    }
    if let Some(list) = std::fs::read(made).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
        for m in list["maps"].as_array().into_iter().flatten().filter_map(|m| m.as_str()).take(5000) {
            if crate::maps::safe_arg(m) {
                maps.insert(m.to_lowercase(), mine.clone());
            }
        }
    }
    // your Studio modes, and the ones that are remixes
    for e in std::fs::read_dir(graphs).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        let Some(id) = n.strip_suffix(".graph.json") else { continue };
        let v = match credits.get(id) {
            Some(c) => json!({"kind": "remix", "of": c.name, "author": c.author, "url": if c.gb > 0 { format!("https://gamebanana.com/{}/{}", if c.model == Model::Spray { "sprays" } else { "mods" }, c.gb) } else { String::new() }}),
            None => mine.clone(),
        };
        modes.insert(id.to_string(), v);
    }
    for (id, m) in prefabs {
        let kind = m.origin.as_ref().map(|o| o.kind.as_str()).unwrap_or("local");
        let v = match kind {
            "community" => json!({"kind": "community", "author": m.author}),
            "file" => json!({"kind": "file"}),
            _ => mine.clone(),
        };
        pre.insert(format!("mine:{id}"), v);
    }
    // the community library wins: it knows the creator and the page
    for it in lib {
        let v = json!({"kind": "community", "author": it.author, "url": it.url(), "name": it.name});
        for m in &it.maps {
            maps.insert(m.to_lowercase(), v.clone());
        }
        if !it.mode.is_empty() {
            modes.insert(it.mode.clone(), v.clone());
        }
        for p in &it.prefabs {
            pre.insert(p.clone(), v.clone());
        }
    }
    json!({"maps": maps, "modes": modes, "prefabs": pre})
}

async fn api_origins() -> Response {
    run(|| {
        let dir = lib_dir();
        let prefab_dir = super::editor::prefabs::lib_dir(&crate::sys::app_dir().join("editor"));
        Ok(origins(
            &load_all(&dir),
            &credits(&dir),
            &super::studio::graphs_dir(),
            &crate::sys::app_dir().join("tools").join("community").join("packages"),
            &crate::sys::app_dir().join("tools").join("compile").join("made.json"),
            &super::editor::prefabs::mine(&prefab_dir),
        ))
    })
    .await
}

// ---- routes ------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct KeyReq {
    key: String,
}

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

async fn run(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into())))
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

fn entry(key: &str) -> Result<Installed, String> {
    let (model, gb) = parse_key(key).ok_or("That isn't a community creation.")?;
    load(&lib_dir(), model, gb).ok_or_else(|| "That isn't installed from Community (any more).".to_string())
}

async fn api_library(State(st): State<Shared>) -> Response {
    start_tick(&st);
    run(|| {
        let idx = super::community::index_cached();
        Ok(summary(&load_all(&lib_dir()), &idx))
    })
    .await
}

async fn api_uninstall(State(st): State<Shared>, h: HeaderMap, Json(req): Json<KeyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game();
    run(move || {
        let it = entry(&req.key)?;
        if let Some(g) = &game {
            for m in &it.maps {
                if super::doctor::map_in_use(g, m) {
                    return Err(super::doctor::IN_USE.into());
                }
            }
        }
        drop_everywhere(&st, &it.key());
        uninstall(game.as_deref(), &lib_dir(), &it)
    })
    .await
}

async fn api_update(State(st): State<Shared>, h: HeaderMap, Json(req): Json<KeyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    start_tick(&st);
    run(move || {
        let it = entry(&req.key)?;
        let idx = super::community::index_cached();
        let file = newer(&it, &idx).ok_or("There's no newer version listed.")?;
        apply_update(&st, &it, file)
    })
    .await
}

async fn api_update_all(State(st): State<Shared>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    start_tick(&st);
    run(move || {
        let idx = super::community::index_cached();
        let mut out = Vec::new();
        for it in load_all(&lib_dir()) {
            let Some(file) = newer(&it, &idx).filter(|f| *f != it.pending) else { continue };
            let r = apply_update(&st, &it, file);
            out.push(json!({"key": it.key(), "name": it.name, "result": r.as_ref().ok(), "error": r.err()}));
        }
        Ok(json!({"ok": true, "updates": out}))
    })
    .await
}

async fn api_go_back(State(st): State<Shared>, h: HeaderMap, Json(req): Json<KeyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    run(move || {
        let it = entry(&req.key)?;
        go_back(&st, &it)
    })
    .await
}

async fn api_notes(axum::extract::Path((model, id)): axum::extract::Path<(String, i64)>) -> Response {
    run(move || {
        let model = Model::parse(&model).filter(|_| id > 0 && id < 100_000_000).ok_or("That isn't a GameBanana page.")?;
        let url = format!("https://gamebanana.com/apiv11/{}/{id}/Updates?_nPage=1&_nPerpage=5", if model == Model::Spray { "Spray" } else { "Mod" });
        let r = crate::sys::http().get(url).header("User-Agent", "LonerStrike/1.0 (community browser)").timeout(Duration::from_secs(15)).send().map_err(|_| "Couldn't reach GameBanana.".to_string())?;
        let mut b = Vec::new();
        use std::io::Read;
        r.take(2 << 20).read_to_end(&mut b).map_err(|_| "GameBanana's answer was cut off.".to_string())?;
        let v: Value = serde_json::from_slice(&b).map_err(|_| "GameBanana sent something the app couldn't read.".to_string())?;
        Ok(json!({"notes": notes_of(&v)}))
    })
    .await
}

async fn api_remix(State(st): State<Shared>, h: HeaderMap, Json(req): Json<KeyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game();
    run(move || {
        let it = entry(&req.key)?;
        let (id, name) = remix(&lib_dir(), &super::studio::graphs_dir(), game.as_ref().map(|g| g.dir.as_path()), &it)?;
        Ok(json!({"ok": true, "id": id, "name": name}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/community/library", get(api_library))
        .route("/api/tools/community/origins", get(api_origins))
        .route("/api/tools/community/uninstall", post(api_uninstall))
        .route("/api/tools/community/update", post(api_update))
        .route("/api/tools/community/update-all", post(api_update_all))
        .route("/api/tools/community/go-back", post(api_go_back))
        .route("/api/tools/community/notes/:model/:id", get(api_notes))
        .route("/api/tools/community/remix", post(api_remix))
        .route("/api/tools/community/server/:id", get(api_server).post(api_server_set))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::community::{parse_index, Item, Status};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-commlib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn map_item(gb: i64, file: i64) -> Installed {
        Installed { kind: Kind::Map, model: Model::Mod, gb, file, name: "De Lonely".into(), author: "jam".into(), game: "cstrike".into(), section: "maps".into(), maps: vec!["de_Lonely".into()], installed: 5, ..Default::default() }
    }

    #[test]
    fn keys() {
        assert_eq!(dir_name(Model::Mod, 12), "mod-12");
        assert_eq!(parse_key("spray-4"), Some((Model::Spray, 4)));
        for bad in ["mod-0", "mod--1", "mod-1x", "x-1", "mod-01", "../mod-1", "mod-1/..", "mod-"] {
            assert_eq!(parse_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn library_round_trip_updates_and_chips() {
        let d = tmp("lib");
        save(&d, &map_item(101, 5)).unwrap();
        let mut spray = map_item(7, 1);
        spray.model = Model::Spray;
        spray.kind = Kind::Spray;
        spray.maps.clear();
        spray.installed = 9;
        save(&d, &spray).unwrap();
        let mut mode = map_item(102, 3);
        mode.kind = Kind::Mode;
        mode.maps.clear();
        mode.mode = "lava".into();
        mode.installed = 1;
        save(&d, &mode).unwrap();
        // junk folders and files are ignored
        std::fs::create_dir_all(d.join("mod-999")).unwrap();
        std::fs::write(d.join("mod-999/meta.json"), b"{nope").unwrap();
        std::fs::create_dir_all(d.join("other")).unwrap();
        let all = load_all(&d);
        assert_eq!(all.iter().map(|i| i.key()).collect::<Vec<_>>(), ["spray-7", "mod-101", "mod-102"], "newest first");

        let idx: Index = parse_index(br#"{"schema": 1, "revoked": ["spray:7"], "items": [
            {"kind": "map", "gb": 101, "file": 9, "name": "De Lonely"},
            {"kind": "mode", "gb": 102, "file": 3, "name": "Lava", "mode": {"id": "lava"}}]}"#).unwrap();
        assert_eq!(newer(&all[1], &idx), Some(9));
        assert_eq!(newer(&all[2], &idx), None, "same file");
        assert!(pulled(&all[0], &idx) && !pulled(&all[1], &idx));
        let s = summary(&all, &idx);
        assert_eq!(s["maps"]["de_lonely"]["author"], "jam", "maps by lower-case name");
        assert_eq!(s["modes"]["lava"]["key"], "mod-102");
        assert_eq!(s["pages"]["spray-7"]["url"], "https://gamebanana.com/sprays/7");
        assert_eq!(s["items"][1]["update"], 9);
        assert_eq!(s["items"][0]["pulled"], true);

        // matched by uid: the same mode re-uploaded on another page
        let idx2: Index = parse_index(br#"{"schema": 1, "items": [{"kind": "mode", "gb": 500, "file": 2, "name": "Lava", "uid": "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b", "mode": {"id": "lava"}}]}"#).unwrap();
        let mut moved = all[2].clone();
        moved.uid = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b".into();
        moved.owner_id = 42;
        assert_eq!(newer(&moved, &idx2), None, "a uid alone links nothing: the other page has no owner we know");
        let idx3: Index = parse_index(br#"{"schema": 1, "items": [{"kind": "mode", "gb": 500, "file": 2, "name": "Lava", "uid": "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b", "ownerId": 42, "mode": {"id": "lava"}}]}"#).unwrap();
        assert_eq!(newer(&moved, &idx3), Some(2), "same uid, same owner: it moved pages");
        // the hijack: someone else's page declaring the same uid is never an update
        let idx4: Index = parse_index(br#"{"schema": 1, "items": [{"kind": "mode", "gb": 666, "file": 9, "name": "Lava 2.0", "uid": "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b", "ownerId": 666, "mode": {"id": "lava"}}]}"#).unwrap();
        assert_eq!(newer(&moved, &idx4), None, "another owner's entry with our uid isn't our update");
        let mut unknown = moved.clone();
        unknown.owner_id = 0;
        assert_eq!(newer(&unknown, &idx3), None, "an install without an owner only follows its own page");
        assert_eq!(newer(&all[2], &idx2), None, "without a uid only the page counts");
        // an older file listed never counts as an update
        let mut it = all[1].clone();
        it.file = 20;
        assert_eq!(newer(&it, &idx), None);
        let _ = Item { status: Status::Listed, ..Default::default() };
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn uninstalling_a_prefab_removes_only_its_files() {
        let d = tmp("pre");
        let mut it = map_item(55, 1);
        it.kind = Kind::Prefab;
        it.maps.clear();
        it.prefabs = vec!["mine:../../evil".into(), "builtin:crate".into()];
        save(&d, &it).unwrap();
        let r = uninstall(None, &d, &it).unwrap();
        assert_eq!(r["removed"].as_array().unwrap().len(), 0, "bad ids touch nothing");
        assert!(load(&d, Model::Mod, 55).is_none(), "forgotten");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn mode_clashes_and_remix_ids() {
        let d = tmp("clash");
        let game = d.join("cstrike");
        let gm = game.join("addons/amxmodx/configs/gamemodes");
        std::fs::create_dir_all(&gm).unwrap();
        std::fs::write(gm.join("modes.ini"), "classic \"Classic\"\nsurf \"Surf\"\n").unwrap();
        let graphs = d.join("graphs");
        std::fs::create_dir_all(&graphs).unwrap();
        std::fs::write(graphs.join("mine.graph.json"), "{}").unwrap();
        let mut lava = map_item(102, 3);
        lava.kind = Kind::Mode;
        lava.mode = "lava".into();
        let lib = vec![lava.clone()];
        assert!(mode_clash(&game, &graphs, &lib, "surf", Model::Mod, 200).unwrap().contains("already has a mode"));
        assert!(mode_clash(&game, &graphs, &lib, "mine", Model::Mod, 200).unwrap().contains("your own"));
        assert!(mode_clash(&game, &graphs, &lib, "lava", Model::Mod, 200).unwrap().contains("Another community mode"));
        assert_eq!(mode_clash(&game, &graphs, &lib, "lava", Model::Mod, 102), None, "a reinstall of the same page");
        assert_eq!(mode_clash(&game, &graphs, &lib, "fresh", Model::Mod, 200), None);

        let taken = |c: &str| c == "lava_remix" || c == "lava_remix2";
        assert_eq!(remix_id("lava", &taken), "lava_remix3");
        let long = remix_id("abcdefghijklmnopqrstuvwx", &|_| false);
        assert!(long.len() <= 24 && crate::tools::studio::graph::valid_mode_id(&long), "{long}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn remix_copies_the_graph_with_credit() {
        let d = tmp("remix");
        let lib = d.join("lib");
        let graphs = d.join("graphs");
        let mut it = map_item(102, 3);
        it.kind = Kind::Mode;
        it.maps.clear();
        it.mode = "lava".into();
        it.name = "Floor is lava".into();
        it.uid = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b".into();
        save(&lib, &it).unwrap();
        let g = json!({"schema": "loner-strike/graph", "version": 2, "mode": {"id": "lava", "name": "Floor is lava", "description": ""}, "nodes": [], "edges": []});
        std::fs::write(lib.join("mod-102/lava.graph.json"), g.to_string()).unwrap();
        let (id, name) = remix(&lib, &graphs, None, &it).unwrap();
        assert_eq!((id.as_str(), name.as_str()), ("lava_remix", "Floor is lava remix"));
        let back: Value = serde_json::from_str(&std::fs::read_to_string(graphs.join("lava_remix.graph.json")).unwrap()).unwrap();
        assert_eq!(back["mode"]["id"], "lava_remix");
        assert!(crate::tools::community::valid_uid(back["mode"]["uid"].as_str().unwrap()), "a uid of its own");
        assert_eq!(credits(&lib)["lava_remix"].uid, "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b", "remixOf = the original's uid");
        assert_eq!(credits(&lib)["lava_remix"].author, "jam");
        let (id2, _) = remix(&lib, &graphs, None, &it).unwrap();
        assert_eq!(id2, "lava_remix2", "never over your first copy");
        let mut map = it.clone();
        map.kind = Kind::Map;
        assert!(remix(&lib, &graphs, None, &map).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }


    #[test]
    fn busy_servers_and_maps_in_use() {
        let mut it = map_item(1, 1);
        it.maps = vec!["de_lonely".into()];
        let st = |map: &str, players: Value| json!({"online": true, "state": {"map": map, "players": players}});
        assert!(busy_on(&st("de_Lonely", json!([{"name": "a"}])), &it, ""), "people on that map");
        assert!(!busy_on(&st("de_lonely", json!([{"name": "b", "bot": true}])), &it, ""), "bots only: free");
        assert!(!busy_on(&st("de_dust2", json!([{"name": "a"}])), &it, ""), "another map");
        assert!(!busy_on(&json!({"online": false}), &it, ""));
        let mut m = it.clone();
        m.kind = Kind::Mode;
        m.maps.clear();
        m.mode = "lava".into();
        assert!(busy_on(&st("de_dust2", json!([{"name": "a"}])), &m, "lava"), "a mode in play");
        assert!(!busy_on(&st("de_dust2", json!([{"name": "a"}])), &m, "classic"));
    }

    #[test]
    fn shared_files_other_maps_need_are_counted() {
        let d = tmp("res");
        let maps = d.join("maps");
        std::fs::create_dir_all(&maps).unwrap();
        std::fs::write(maps.join("de_other.res"), "// other\r\nshared.wad\r\nmodels/props/barrel.mdl\r\n").unwrap();
        std::fs::write(maps.join("de_lonely.res"), "lonely.wad\r\nshared.wad\r\n").unwrap();
        let need = needed_by_others(&[maps.clone()], &["de_lonely".to_string()]);
        assert!(need.contains("shared.wad") && need.contains("models/props/barrel.mdl"));
        assert!(!need.contains("lonely.wad"), "its own .res doesn't count");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn previous_version_kept_and_put_back() {
        let d = tmp("prev");
        let lib = d.join("lib");
        let root = d.join("cstrike");
        std::fs::create_dir_all(root.join("maps")).unwrap();
        std::fs::write(root.join("maps/de_lonely.bsp"), b"v1").unwrap();
        std::fs::write(root.join("shared.wad"), b"WAD3").unwrap();
        let mut it = map_item(101, 5);
        it.maps = vec!["de_lonely".into()];
        save(&lib, &it).unwrap();
        let mut recs = serde_json::Map::new();
        recs.insert("101".into(), json!({"name": "De Lonely", "files": ["maps/de_lonely.bsp", "shared.wad", "../evil"], "maps": ["de_lonely"]}));
        keep_previous(Some(&root), &recs, &lib, &it).unwrap();
        let pd = prev_dir(&lib, "mod-101");
        assert_eq!(std::fs::read(pd.join("files/maps/de_lonely.bsp")).unwrap(), b"v1");
        assert!(pd.join("files/shared.wad").is_file() && !d.join("evil").exists());
        assert!(pd.join("meta.json").is_file() && pd.join("record.json").is_file());
        // a mode's graph is kept too; restoring it without the game rebuilds nothing but puts the graph back
        let mut m = map_item(102, 3);
        m.kind = Kind::Mode;
        m.maps.clear();
        m.mode = "lava".into();
        save(&lib, &m).unwrap();
        std::fs::write(lib.join("mod-102/lava.graph.json"), b"{\"v\":1}").unwrap();
        keep_previous(None, &serde_json::Map::new(), &lib, &m).unwrap();
        std::fs::write(lib.join("mod-102/lava.graph.json"), b"{\"v\":2}").unwrap();
        let mut newer_m = m.clone();
        newer_m.file = 4;
        newer_m.previous_file = 3;
        save(&lib, &newer_m).unwrap();
        let back = restore_previous(None, None, &lib, "mod-102").unwrap();
        assert_eq!((back.file, back.previous_file), (3, 0), "the earlier file is the one installed again");
        assert_eq!(std::fs::read(lib.join("mod-102/lava.graph.json")).unwrap(), b"{\"v\":1}");
        assert!(!prev_dir(&lib, "mod-102").exists(), "the kept copy is used up");
        assert!(restore_previous(None, None, &lib, "mod-102").is_err(), "nothing more to go back to");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn update_notes_from_gamebanana() {
        let v = json!({"_aRecords": [{"_sName": "Night version", "_sVersion": "1.1", "_tsDateAdded": 5, "_sText": "<b>Darker</b>",
                                      "_aChangeLog": [{"cat": "Feature", "text": "Night <i>sky</i>"}, {"cat": "Bugfix", "text": "Ladder"}]}]});
        let n = notes_of(&v);
        assert_eq!((n[0]["title"].as_str(), n[0]["version"].as_str()), (Some("Night version"), Some("1.1")));
        assert_eq!(n[0]["changes"][0]["text"], "Night sky", "plain text only");
        assert_eq!(n[0]["text"], "Darker");
        assert!(notes_of(&json!({})).is_empty());
    }

    #[test]
    fn waiting_updates_and_stale_servers_in_the_library() {
        let d = tmp("pend");
        let mut it = map_item(101, 5);
        it.pending = 9;
        save(&d, &it).unwrap();
        let idx: Index = parse_index(br#"{"schema": 1, "items": [{"kind": "map", "gb": 101, "file": 9, "name": "De Lonely"}]}"#).unwrap();
        let s = summary(&load_all(&d), &idx);
        assert_eq!((s["updates"].as_u64(), s["waiting"].as_u64()), (Some(0), Some(1)), "a waiting update isn't offered again");
        let mut p = ServerPick { maps: vec!["mod-101".into()], ..Default::default() };
        p.stale.push("mod-101".into());
        save_picks(&d, "srv", &p).unwrap();
        assert_eq!(load_picks(&d, "srv").stale, ["mod-101"]);
        let _ = std::fs::remove_dir_all(&d);
    }


    #[test]
    fn origins_of_things_in_the_library() {
        let d = tmp("orig");
        let graphs = d.join("graphs");
        let packages = d.join("packages");
        std::fs::create_dir_all(&graphs).unwrap();
        std::fs::create_dir_all(&packages).unwrap();
        std::fs::write(graphs.join("mine.graph.json"), "{}").unwrap();
        std::fs::write(graphs.join("lava_remix.graph.json"), "{}").unwrap();
        std::fs::write(packages.join("de_mine-loner-strike.zip"), b"").unwrap();
        std::fs::write(packages.join("lava_remix-mode-loner-strike.zip"), b"").unwrap();
        std::fs::write(d.join("made.json"), r#"{"maps": ["de_Built", "bad;map"]}"#).unwrap();
        let mut credits = BTreeMap::new();
        credits.insert("lava_remix".to_string(), Credit { model: Model::Mod, gb: 102, name: "Floor is lava".into(), author: "jam".into(), ..Default::default() });
        let mut it = map_item(101, 5);
        it.maps = vec!["De_Lonely".into()];
        let pf = |kind: &str| crate::editor::prefab::PrefabMeta { origin: Some(crate::editor::prefab::Origin { kind: kind.into(), ..Default::default() }), author: "x".into(), ..Default::default() };
        let prefabs = vec![("crate".to_string(), pf("local")), ("door".to_string(), pf("file")), ("fence".to_string(), pf("community"))];
        let o = origins(&[it], &credits, &graphs, &packages, &d.join("made.json"), &prefabs);
        assert_eq!(o["maps"]["de_mine"]["kind"], "mine", "a map you published");
        assert!(o["maps"]["lava_remix-mode"].is_null(), "a mode package isn't a map");
        assert_eq!(o["maps"]["de_built"]["kind"], "mine", "a map you compiled");
        assert!(o["maps"]["bad;map"].is_null());
        assert_eq!((o["maps"]["de_lonely"]["kind"].as_str(), o["maps"]["de_lonely"]["author"].as_str()), (Some("community"), Some("jam")));
        assert_eq!(o["modes"]["mine"]["kind"], "mine");
        assert_eq!((o["modes"]["lava_remix"]["kind"].as_str(), o["modes"]["lava_remix"]["of"].as_str()), (Some("remix"), Some("Floor is lava")));
        assert_eq!((o["prefabs"]["mine:crate"]["kind"].as_str(), o["prefabs"]["mine:door"]["kind"].as_str(), o["prefabs"]["mine:fence"]["kind"].as_str()), (Some("mine"), Some("file"), Some("community")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn server_pick_changes() {
        let mut a = map_item(1, 1);
        a.maps = vec!["de_a".into()];
        let b = map_item(2, 1);
        let mut m = map_item(3, 1);
        m.kind = Kind::Mode;
        m.mode = "lava".into();
        let lib = vec![a, b, m];
        let old = ServerPick { maps: vec!["mod-1".into()], ..Default::default() };
        let new = ServerPick { maps: vec!["mod-2".into(), "mod-3".into(), "mod-77".into()], modes: vec!["mod-3".into(), "mod-1".into()], ..Default::default() };
        let (am, dm, ao, dmo) = pick_changes(&lib, &old, &new);
        assert_eq!(am.iter().map(|i| i.gb).collect::<Vec<_>>(), [2], "only maps that are maps and installed");
        assert_eq!(dm.iter().map(|i| i.gb).collect::<Vec<_>>(), [1]);
        assert_eq!(ao.iter().map(|i| i.gb).collect::<Vec<_>>(), [3], "only modes");
        assert!(dmo.is_empty());
        let d = tmp("picks");
        save_picks(&d, "srv1", &new).unwrap();
        assert_eq!(load_picks(&d, "srv1"), new);
        assert_eq!(load_picks(&d, "other"), ServerPick::default());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn mode_packages_carry_one_good_graph() {
        let d = tmp("pkg");
        assert!(mode_graph(&d).unwrap_err().contains("no Game Mode Studio graph"));
        std::fs::create_dir_all(d.join("modes/lava")).unwrap();
        let good = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tools/studio/testdata/every_node.graph.json")).ok();
        let Some(good) = good else { return };
        std::fs::write(d.join("modes/lava/lava.graph.json"), &good).unwrap();
        let g = mode_graph(&d).unwrap();
        assert!(!g.mode.id.is_empty());
        std::fs::write(d.join("modes/lava/two.graph.json"), &good).unwrap();
        assert!(mode_graph(&d).unwrap_err().contains("more than one"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
