//! The Plugin Studio (Tools > Make): write AMX Mod X plugins in the app. Lists, reads and
//! saves plugin sources in two folders (the player's "My plugins" in the app's folder, and
//! the game's addons/amxmodx/scripting), reads the game's include files into a word list for
//! the editor (completions, hover docs, signatures), compiles with the game's own amxxpc
//! (through CrossOver on a Mac, like setup does), and "tries" a plugin in the player's own
//! game: the .amxx goes in its plugins folder, a list of its own (configs/plugins-studio.ini,
//! which AMX Mod X loads beside plugins.ini) loads it, and the map restarts through the
//! Game Mode Menu (gm_restartmap). The log route follows AMX Mod X's logs for that plugin.
//!
//! Only the player's own game on this computer (`local_game`, never a remote or a dedicated
//! server), only on the app window's listener, only files with safe names inside these
//! folders. Changes need the X-Panel header.

pub mod diag;
pub mod inc;

use crate::game::Game;
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// The helper include, shipped with the app and handed to the compiler.
pub const LONER_INC: &str = include_str!("../../../../plugins/scripting/include/loner.inc");
/// The game-mode components' include (gm_components), which loner.inc pulls in.
pub const LS_COMPONENTS_INC: &str = include_str!("../../../../plugins/scripting/include/ls_components.inc");
/// The includes the app ships: shown in the include folder and handed to the compiler.
pub const APP_INCS: &[(&str, &str)] = &[("loner.inc", LONER_INC), ("ls_components.inc", LS_COMPONENTS_INC)];

const NO_GAME: &str = "Set up the game on this computer first.";
const INTERNAL: &str = "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.";
const BAD_NAME: &str = "Use a file name with only letters, numbers, - and _, ending in .sma (or .inc).";
const RESERVED_NAME: &str = "Windows keeps that name for a device (like CON or NUL). Pick another name.";
const MAX_SOURCE: usize = 2 << 20;
const STUDIO_INI: &str = "plugins-studio.ini";
const CROSSOVER_WINE: &str = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";

// ---- folders and names ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    /// The player's own: <app>/plugin-studio/my-plugins
    Mine,
    /// The game's addons/amxmodx/scripting
    Game,
    /// The game's include folder (read only), plus loner.inc
    Include,
}

impl Folder {
    pub fn parse(s: &str) -> Option<Folder> {
        match s {
            "mine" => Some(Folder::Mine),
            "game" => Some(Folder::Game),
            "include" => Some(Folder::Include),
            _ => None,
        }
    }
    fn id(self) -> &'static str {
        match self {
            Folder::Mine => "mine",
            Folder::Game => "game",
            Folder::Include => "include",
        }
    }
}

fn studio_dir(g: &Game) -> PathBuf {
    g.app.join("plugin-studio")
}

fn amxx_dir(g: &Game) -> PathBuf {
    g.dir.join("addons").join("amxmodx")
}

pub fn folder_dir(g: &Game, f: Folder) -> PathBuf {
    match f {
        Folder::Mine => studio_dir(g).join("my-plugins"),
        Folder::Game => amxx_dir(g).join("scripting"),
        Folder::Include => amxx_dir(g).join("scripting").join("include"),
    }
}

/// A source file's name: letters, numbers, - _ and dots (not first), ending .sma or .inc.
pub fn safe_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.len() <= 64
        && (lower.ends_with(".sma") || lower.ends_with(".inc"))
        && name.len() > 4
        && name.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
        && !name.contains("..")
        && !crate::setup::record::windows_unsafe(name)
}

/// Why a file name can't be used, in a sentence (None: it's fine).
pub fn name_problem(name: &str) -> Option<&'static str> {
    if safe_file(name) {
        None
    } else if crate::setup::record::windows_unsafe(name) {
        Some(RESERVED_NAME)
    } else {
        Some(BAD_NAME)
    }
}

/// A plugin's name (the file name without .sma): what goes in plugins.ini and the logs.
pub fn stem(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

/// The file's path, checked: a safe name, directly inside the folder, and (if it's there) a
/// real file whose folder is that folder (no link out of it).
pub fn resolve(dir: &Path, name: &str) -> Result<PathBuf, String> {
    if let Some(why) = name_problem(name) {
        return Err(why.into());
    }
    let p = dir.join(name);
    if let Ok(meta) = std::fs::symlink_metadata(&p) {
        if !meta.is_file() {
            return Err("That isn't a plugin source file.".into());
        }
        let (Ok(real), Ok(real_dir)) = (p.canonicalize(), dir.canonicalize()) else { return Err("That file can't be opened.".into()) };
        if real.parent() != Some(real_dir.as_path()) {
            return Err("That file can't be opened.".into());
        }
    }
    Ok(p)
}

// ---- what the Studio never writes ------------------------------------------------------

/// AMX Mod X's own plugins (its base package).
pub const AMXX_BASE: &[&str] = &[
    "admin", "admin_sql", "adminchat", "admincmd", "adminhelp", "adminslots", "adminvote", "antiflood", "cmdmenu", "imessage", "mapchooser", "mapsmenu", "menufront", "multilingual",
    "nextmap", "pausecfg", "plmenu", "pluginmenu", "restmenu", "scrollmsg", "statscfg", "telemenu", "timeleft", "csstats", "miscstats", "stats_logging", "statsx", "statsx_shell",
];

/// Whose plugin a name is, when it isn't the player's to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// one of Loner-Strike's own (setup builds it from the app's copy)
    Ours,
    /// made by the Game Mode Studio from a graph (`studio_*`)
    GameModeStudio,
    /// comes with AMX Mod X
    AmxModX,
    /// part of a mode or add-on Loner-Strike installs (the catalog's plugins)
    Mod,
}

static RESOURCES: OnceLock<PathBuf> = OnceLock::new();

/// Where the app's own files are (its plugin sources and catalog), remembered from a request.
fn remember_resources(st: &Shared) {
    if RESOURCES.get().is_none() {
        if let Ok(r) = crate::setup_api::resources_for(st) {
            let _ = RESOURCES.set(r);
        }
    }
}

fn resources() -> Option<PathBuf> {
    RESOURCES.get().cloned().or_else(|| crate::setup::resource_root(None).ok())
}

/// Every plugin the catalog's modes and add-ons build ("compile": src and out), lower case.
pub fn catalog_stems(res: &Path) -> Arc<std::collections::HashSet<String>> {
    static CACHE: OnceLock<Mutex<Option<(PathBuf, Arc<std::collections::HashSet<String>>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((p, set)) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if p == res {
            return set.clone();
        }
    }
    fn walk(v: &Value, out: &mut std::collections::HashSet<String>) {
        match v {
            Value::Object(m) => {
                if let Some(list) = m.get("compile").and_then(|c| c.as_array()) {
                    for c in list {
                        for key in ["src", "out"] {
                            if let Some(s) = c[key].as_str() {
                                let base = diag::base_name(s);
                                out.insert(stem(base).to_ascii_lowercase());
                            }
                        }
                    }
                }
                m.values().for_each(|x| walk(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut set = std::collections::HashSet::new();
    let cat = res.join("catalog");
    let mut files = vec![cat.join("components.json")];
    files.extend(std::fs::read_dir(cat.join("parts")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")));
    for f in files {
        if let Some(v) = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            walk(&v, &mut set);
        }
    }
    let set = Arc::new(set);
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((res.to_path_buf(), set.clone()));
    set
}

/// Whose plugin this is, by its name, if it isn't the player's.
pub fn owner_of(stem: &str) -> Option<Owner> {
    let s = stem.to_ascii_lowercase();
    if crate::setup::core::OUR_PLUGINS.iter().any(|p| *p == s) {
        Some(Owner::Ours)
    } else if s.starts_with("studio_") {
        Some(Owner::GameModeStudio)
    } else if AMXX_BASE.contains(&s.as_str()) {
        Some(Owner::AmxModX)
    } else if resources().is_some_and(|r| catalog_stems(&r).contains(&s)) {
        Some(Owner::Mod)
    } else {
        None
    }
}

/// A compiled plugin in the game that neither the Studio nor the Mods page put there.
fn installed_by_someone_else(g: &Game, stem: &str) -> bool {
    amxx_dir(g).join("plugins").join(format!("{stem}.amxx")).exists() && !trying(g).iter().any(|n| n.eq_ignore_ascii_case(stem)) && !crate::plugins::load(g).iter().any(|n| n.eq_ignore_ascii_case(stem))
}

fn owner_words(o: Owner) -> &'static str {
    match o {
        Owner::Ours => "This is one of Loner-Strike's own plugins, so it's read only here (setup builds it from Loner-Strike's copy). Save a copy in My plugins to make your own version.",
        Owner::GameModeStudio => "Made by the Game Mode Studio from a graph: change the graph and compile it again. Save a copy in My plugins to make it a plugin of your own.",
        Owner::AmxModX => "This plugin comes with AMX Mod X, so it's read only here. Save a copy in My plugins to make your own version.",
        Owner::Mod => "This plugin is part of a mode or add-on Loner-Strike installs, so it's read only here. Save a copy in My plugins to make your own version.",
    }
}

/// Why the Studio won't change this file, or None when it's the player's to change.
pub fn protection(g: &Game, f: Folder, name: &str) -> Option<String> {
    match f {
        Folder::Mine => None,
        Folder::Include if APP_INCS.iter().any(|(n, _)| *n == name) => Some("This include comes with Loner-Strike, so it's read only. Save a copy in My plugins to experiment with it.".into()),
        Folder::Include => Some("Include files are the game's own, so they're read only. Save a copy in My plugins to change one.".into()),
        Folder::Game if name.to_ascii_lowercase().ends_with(".inc") => Some("Save include files in My plugins.".into()),
        Folder::Game => match owner_of(stem(name)) {
            Some(o) => Some(owner_words(o).into()),
            None if installed_by_someone_else(g, stem(name)) => Some("A plugin with this name is installed in your game, and neither the Plugin Studio nor the Mods page put it there, so it's read only here. Save a copy in My plugins to make your own version.".into()),
            None => None,
        },
    }
}

// ---- Loner-Strike's versions (reset) ----------------------------------------------------

/// Loner-Strike's own copy of a file, if it ships one: our plugins' sources, and its includes.
pub fn shipped_copy(f: Folder, name: &str) -> Option<Vec<u8>> {
    match f {
        Folder::Game if owner_of(stem(name)) == Some(Owner::Ours) && name.to_ascii_lowercase().ends_with(".sma") => {
            let s = crate::setup::core::OUR_PLUGINS.iter().find(|p| p.eq_ignore_ascii_case(stem(name)))?;
            std::fs::read(resources()?.join("plugins").join("scripting").join(format!("{s}.sma"))).ok()
        }
        Folder::Include => APP_INCS.iter().find(|(n, _)| *n == name).map(|(_, t)| t.as_bytes().to_vec()),
        _ => None,
    }
}

fn same_text(a: &[u8], b: &[u8]) -> bool {
    let norm = |x: &[u8]| -> Vec<u8> { x.iter().copied().filter(|&c| c != b'\r').collect() };
    norm(a) == norm(b)
}

/// Whether the file on disk differs from Loner-Strike's copy: None when there's no copy (or no
/// file on disk to put back).
pub fn differs_from_shipped(g: &Game, f: Folder, name: &str) -> Option<bool> {
    let want = shipped_copy(f, name)?;
    let path = file_path(g, f, name).ok()?;
    let now = std::fs::read(&path).ok()?;
    Some(!same_text(&now, &want))
}

/// Puts Loner-Strike's version of a file back (kept first by the safety net). For one of our
/// plugins the compiled plugin is rebuilt from it too, like setup does.
pub fn reset(g: &Game, f: Folder, name: &str) -> Result<Value, String> {
    let want = shipped_copy(f, name).ok_or("Loner-Strike doesn't ship a version of this file to go back to.")?;
    let path = file_path(g, f, name)?;
    if !path.is_file() {
        return Err("That file isn't in your game, so there's nothing to reset: Loner-Strike's own copy is used.".into());
    }
    crate::safety::before("Plugin Studio", &format!("Reset {name} to Loner-Strike's version"), &path);
    write_quiet(&path, &want)?;
    if f != Folder::Game {
        return Ok(json!({"ok": true, "message": format!("{name} is back to Loner-Strike's version.")}));
    }
    // rebuild the compiled plugin from it
    let built = compiler(g).and_then(|(exe, install)| {
        let inc = studio_include(g)?;
        let out = studio_dir(g).join("build").join(format!("{}.amxx", stem(name)));
        run_compiler(&exe, &install, &path, &out, &inc)
    });
    let message = match built {
        Ok(c) if c.ok => {
            let dest = amxx_dir(g).join("plugins").join(format!("{}.amxx", stem(name)));
            let bytes = std::fs::read(&c.amxx).map_err(|e| format!("Couldn't read the compiled plugin ({e})."))?;
            crate::safety::before("Plugin Studio", &format!("Rebuilt {} from Loner-Strike's version", stem(name)), &dest);
            write_quiet(&dest, &bytes)?;
            format!("{name} is back to Loner-Strike's version, and {}.amxx was rebuilt from it. It loads with the next map.", stem(name))
        }
        Ok(_) => format!("{name} is back to Loner-Strike's version, but it didn't compile here, so the plugin in your game wasn't rebuilt. Repair on the Mods page builds it."),
        Err(e) => format!("{name} is back to Loner-Strike's version. The plugin in your game wasn't rebuilt ({e}) Repair on the Mods page builds it."),
    };
    Ok(json!({"ok": true, "message": message}))
}

fn modified(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

// ---- include files ---------------------------------------------------------------------

/// A folder or file name that's plain enough to show and look up.
fn plain_part(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && !s.starts_with('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
}

/// Every include file on the install: (name shown, path, size, modified). `include/*.inc` as
/// "x.inc"; its subfolders and the mods' `scripting/*_include` folders as "folder/x.inc".
/// No links, two folders deep at most.
pub fn include_files(g: &Game) -> Vec<(String, PathBuf, u64, u64)> {
    fn walk(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<(String, PathBuf, u64, u64)>) {
        let mut list: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().collect();
        list.sort_by_key(|e| e.file_name());
        for e in list {
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
            if !plain_part(&name) || meta.file_type().is_symlink() {
                continue;
            }
            let shown = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            if meta.is_dir() && depth < 2 {
                walk(&e.path(), &shown, depth + 1, out);
            } else if meta.is_file() && name.to_ascii_lowercase().ends_with(".inc") && meta.len() < 4 << 20 {
                out.push((shown, e.path(), meta.len(), modified(&meta)));
            }
        }
    }
    let scripting = amxx_dir(g).join("scripting");
    let mut out = Vec::new();
    walk(&scripting.join("include"), "", 0, &mut out);
    let mut mods: Vec<_> = std::fs::read_dir(&scripting).into_iter().flatten().flatten().filter(|e| e.file_name().to_string_lossy().ends_with("_include")).collect();
    mods.sort_by_key(|e| e.file_name());
    for d in mods {
        let name = d.file_name().to_string_lossy().to_string();
        if plain_part(&name) && std::fs::symlink_metadata(d.path()).is_ok_and(|m| m.is_dir()) {
            walk(&d.path(), &name, 1, &mut out);
        }
    }
    out
}

/// The path of a file the Studio shows: in My plugins or the game's folder by name, an include
/// only if it's one of the include files found (so a name never becomes a path of its own).
fn file_path(g: &Game, f: Folder, name: &str) -> Result<PathBuf, String> {
    match f {
        Folder::Include => include_files(g).into_iter().find(|(n, ..)| n == name).map(|(_, p, ..)| p).ok_or_else(|| "That include file isn't there.".to_string()),
        _ => resolve(&folder_dir(g, f), name),
    }
}

/// Source files in a folder the Studio leaves out for their names (spaces, accents, CON...),
/// so the page can say so.
fn hidden(g: &Game, f: Folder) -> usize {
    if f == Folder::Include {
        return 0;
    }
    std::fs::read_dir(folder_dir(g, f))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            (name.ends_with(".sma") || name.ends_with(".inc")) && !safe_file(&e.file_name().to_string_lossy()) && e.path().is_file()
        })
        .count()
}

fn list(g: &Game, f: Folder) -> Vec<Value> {
    let mut files: Vec<Value> = if f == Folder::Include {
        include_files(g).into_iter().map(|(name, _, size, m)| json!({"name": name, "size": size, "modified": m, "readOnly": true, "changed": differs_from_shipped(g, f, &name) == Some(true)})).collect()
    } else {
        std::fs::read_dir(folder_dir(g, f))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let meta = std::fs::symlink_metadata(e.path()).ok()?;
                (meta.is_file() && safe_file(&name)).then(|| json!({"name": name, "size": meta.len(), "modified": modified(&meta), "readOnly": protection(g, f, &name).is_some(), "changed": differs_from_shipped(g, f, &name) == Some(true)}))
            })
            .collect()
    };
    if f == Folder::Include {
        for (inc, text) in APP_INCS {
            if !files.iter().any(|v| v["name"] == *inc) {
                files.push(json!({"name": inc, "size": text.len(), "modified": 0, "readOnly": true, "changed": false}));
            }
        }
    }
    files.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_lowercase());
    files
}

/// Text from a file: UTF-8 if it is, else Latin-1 (old plugins).
fn decode(bytes: &[u8]) -> (String, &'static str) {
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.trim_start_matches('\u{feff}').to_string(), "utf8"),
        Err(_) => (bytes.iter().map(|&b| b as char).collect(), "latin1"),
    }
}

/// The file's facts for the page: why it's read only, whether Loner-Strike's version can be put
/// back, and the path the safety net knows it by (for History).
fn facts(g: &Game, f: Folder, name: &str) -> Value {
    let differs = differs_from_shipped(g, f, name);
    let history = match f {
        // (the safety net shows My plugins from the plugin-studio folder: keep_mine)
        Folder::Mine => format!("my-plugins/{name}"),
        Folder::Game => format!("addons/amxmodx/scripting/{name}"),
        Folder::Include => format!("addons/amxmodx/scripting/include/{name}"),
    };
    let hint = match (f, owner_of(stem(name))) {
        (Folder::Game, Some(Owner::Mod)) | (Folder::Game, Some(Owner::AmxModX)) => Some("To put it back as it came, use Repair on the Mods page."),
        (Folder::Game, Some(Owner::GameModeStudio)) => Some("To rebuild it, open its graph in the Game Mode Studio and compile it."),
        _ => None,
    };
    json!({"readOnly": protection(g, f, name), "reset": differs.map(|d| json!({"differs": d})), "resetHint": hint, "history": history})
}

pub fn read(g: &Game, f: Folder, name: &str) -> Result<Value, String> {
    let path = match f {
        Folder::Include => file_path(g, f, name).ok(),
        _ => Some(resolve(&folder_dir(g, f), name)?),
    };
    let mut v = match path.filter(|p| p.is_file()) {
        None => match APP_INCS.iter().find(|(inc, _)| f == Folder::Include && *inc == name) {
            Some((_, text)) => json!({"name": name, "folder": f.id(), "text": text, "encoding": "utf8"}),
            None => return Err("That file isn't there any more.".into()),
        },
        Some(path) => {
            // [Caps] size checked before reading
            let bytes = crate::caps::read_file(&path, "file", MAX_SOURCE as u64).map_err(|e| if e.contains("too big") { format!("{e} Open it in another editor.") } else { e })?;
            let (text, encoding) = decode(&bytes);
            json!({"name": name, "folder": f.id(), "text": text, "encoding": encoding})
        }
    };
    if let (Value::Object(m), Value::Object(extra)) = (&mut v, facts(g, f, name)) {
        m.extend(extra);
    }
    Ok(v)
}

/// Writes next to the file as <name>.part, then renames it over the file (kept first).
fn write_part(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::safety::before("Plugin Studio", &format!("Saved {}", path.file_name().unwrap_or_default().to_string_lossy()), path);
    write_quiet(path, bytes)
}

/// write_part without keeping a copy (the caller kept one, or it's the app's own scratch file).
fn write_quiet(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make the folder ({e})."))?;
    }
    let part = path.with_file_name(format!("{}.part", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    std::fs::write(&part, bytes).map_err(|e| format!("Couldn't save it ({e})."))?;
    std::fs::rename(&part, path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("Couldn't save it ({e}).")
    })
}

/// My plugins live in the app's folder: the safety net keeps their versions too.
fn keep_mine(g: &Game) {
    let dir = folder_dir(g, Folder::Mine);
    let _ = std::fs::create_dir_all(&dir);
    crate::safety::store().allow_root(&studio_dir(g));
}

pub fn save(g: &Game, f: Folder, name: &str, text: &str, encoding: &str, create: bool) -> Result<Value, String> {
    if let Some(why) = protection(g, f, name) {
        return Err(why);
    }
    if text.len() > MAX_SOURCE {
        return Err("That's too big to save (over 2 MB).".into());
    }
    let path = resolve(&folder_dir(g, f), name)?;
    if create && path.exists() {
        return Err(format!("There's already a file called {name}. Pick another name."));
    }
    if f == Folder::Game && !amxx_dir(g).join("scripting").is_dir() {
        return Err("AMX Mod X isn't installed in your game yet. Install the basics first.".into());
    }
    if f == Folder::Mine {
        keep_mine(g);
    }
    let bytes: Vec<u8> = if encoding == "latin1" && text.chars().all(|c| (c as u32) < 256) { text.chars().map(|c| c as u8).collect() } else { text.as_bytes().to_vec() };
    write_part(&path, &bytes)?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true, "name": name, "folder": f.id(), "size": meta.len(), "modified": modified(&meta)}))
}

// ---- export and import (Community doesn't take plugins: they're shared by hand) ------------

/// The files `#include` names that sit in the plugin's own folder as .inc files (and theirs),
/// for Export: the game's and AMX Mod X's own includes aren't copied.
pub fn own_includes(dir: &Path, text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut todo: Vec<String> = vec![text.to_string()];
    while let Some(t) = todo.pop() {
        for line in t.lines() {
            let l = line.trim_start();
            let Some(rest) = l.strip_prefix("#include").or_else(|| l.strip_prefix("#tryinclude")) else { continue };
            let name = rest.trim().trim_matches(|c| c == '<' || c == '>' || c == '"').trim();
            let file = if name.to_ascii_lowercase().ends_with(".inc") { name.to_string() } else { format!("{name}.inc") };
            if !safe_file(&file) || out.iter().any(|x| x.eq_ignore_ascii_case(&file)) {
                continue;
            }
            let p = dir.join(&file);
            if p.is_file() && std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file()) {
                if let Ok(b) = crate::caps::read_file(&p, "include", MAX_SOURCE as u64) {
                    todo.push(String::from_utf8_lossy(&b).into_owned());
                }
                out.push(file);
            }
        }
    }
    out
}

/// Copies the .sma to `dest` (the save dialog's pick) and its own .inc files beside it. An
/// .inc already there with other contents is left alone (and named in `kept`).
pub fn export_to(dir: &Path, name: &str, dest: &Path) -> Result<Value, String> {
    let src = resolve(dir, name)?;
    if !src.is_file() {
        return Err("That file isn't there any more.".into());
    }
    let bytes = crate::caps::read_file(&src, "file", MAX_SOURCE as u64)?;
    std::fs::write(dest, &bytes).map_err(|e| format!("Couldn't save it there ({e})."))?;
    let mut saved = vec![dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()];
    let mut kept = Vec::new();
    let to = dest.parent().unwrap_or(Path::new("."));
    for inc in own_includes(dir, &String::from_utf8_lossy(&bytes)) {
        let b = crate::caps::read_file(&dir.join(&inc), "include", MAX_SOURCE as u64)?;
        let d = to.join(&inc);
        if d.exists() && std::fs::read(&d).ok().as_deref() != Some(&b[..]) {
            kept.push(inc);
            continue;
        }
        std::fs::write(&d, &b).map_err(|e| format!("Couldn't save {inc} there ({e})."))?;
        saved.push(inc);
    }
    let msg = if kept.is_empty() { format!("Saved {}.", saved.join(", ")) } else { format!("Saved {}. Left {} as it was there (a different file of that name).", saved.join(", "), kept.join(", ")) };
    Ok(json!({"ok": true, "saved": saved, "kept": kept, "message": msg}))
}

/// Why a picked file can't come in (None: it can): a plugin source or include, plain text,
/// 2 MB at most, a name Windows and the compiler can use.
pub fn import_problem(name: &str, bytes: &[u8]) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".sma") || lower.ends_with(".inc")) {
        return Some(format!("{name}: only .sma and .inc files."));
    }
    if crate::setup::record::windows_unsafe(name) {
        return Some(format!("{name}: {RESERVED_NAME}"));
    }
    if !safe_file(name) {
        return Some(format!("{name}: {BAD_NAME}"));
    }
    if bytes.len() > MAX_SOURCE {
        return Some(format!("{name} is too big (over 2 MB)."));
    }
    // text: no NUL bytes, and nearly all printable (a binary renamed .sma isn't)
    let odd = bytes.iter().filter(|&&b| b == 0 || (b < 0x20 && !b"\t\r\n\x0c".contains(&b))).count();
    if bytes.contains(&0) || odd * 100 > bytes.len().max(1) {
        return Some(format!("{name} isn't a text file."));
    }
    None
}

/// Files picked for Import, waiting for new names when theirs are taken (by pick id).
static PICKED: std::sync::Mutex<Vec<(String, Vec<PathBuf>)>> = std::sync::Mutex::new(Vec::new());

/// What Import would do with these files: each one's name, why it can't come in, or that its
/// name is taken in My plugins.
pub fn import_plan(dir: &Path, files: &[PathBuf]) -> Vec<Value> {
    files
        .iter()
        .map(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let problem = match std::fs::symlink_metadata(p) {
                Ok(m) if m.is_file() && m.len() <= MAX_SOURCE as u64 => std::fs::read(p).ok().and_then(|b| import_problem(&name, &b)),
                Ok(m) if m.is_file() => Some(format!("{name} is too big (over 2 MB).")),
                _ => Some(format!("{name} isn't a file Import can read.")),
            };
            let taken = problem.is_none() && dir.join(&name).exists();
            json!({"name": name, "problem": problem, "taken": taken})
        })
        .collect()
}

/// Copies the picked files into My plugins, under `renames` for taken names; never over a file
/// that's there. Nothing is copied if any name is still taken or a new name can't be used.
pub fn import_into(dir: &Path, files: &[PathBuf], renames: &std::collections::HashMap<String, String>) -> Result<Value, String> {
    let mut plan: Vec<(PathBuf, String)> = Vec::new();
    let mut refused = Vec::new();
    for v in import_plan(dir, files).into_iter().zip(files) {
        let (info, p) = v;
        if let Some(why) = info["problem"].as_str() {
            refused.push(why.to_string());
            continue;
        }
        let name = info["name"].as_str().unwrap_or_default().to_string();
        let to = renames.get(&name).cloned().unwrap_or_else(|| name.clone());
        if to != name {
            if let Some(why) = import_problem(&to, b"") {
                return Err(why);
            }
            if !to.to_ascii_lowercase().ends_with(&name.to_ascii_lowercase()[name.len() - 4..]) {
                return Err(format!("{to}: keep the {} ending.", &name[name.len() - 4..]));
            }
        }
        if dir.join(&to).exists() || plan.iter().any(|(_, n)| n.eq_ignore_ascii_case(&to)) {
            return Err(format!("There's already a file called {to} in My plugins."));
        }
        plan.push((p.clone(), to));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make My plugins ({e})."))?;
    let mut done = Vec::new();
    for (p, to) in &plan {
        let b = std::fs::read(p).map_err(|e| format!("Couldn't read {} ({e}).", p.display()))?;
        let dest = dir.join(to);
        // (create_new: a file that turned up meanwhile is never overwritten)
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&dest).map_err(|e| format!("Couldn't add {to} ({e})."))?;
        std::io::Write::write_all(&mut f, &b).map_err(|e| format!("Couldn't add {to} ({e})."))?;
        done.push(to.clone());
    }
    let first = done.iter().find(|n| n.to_ascii_lowercase().ends_with(".sma")).or(done.first()).cloned();
    let mut msg = match done.len() {
        0 => "Nothing was imported.".to_string(),
        1 => format!("Imported {}.", done[0]),
        n => format!("Imported {n} files."),
    };
    if !refused.is_empty() {
        msg.push_str(&format!(" Left out: {}", refused.join(" ")));
    }
    Ok(json!({"ok": !done.is_empty(), "imported": done, "refused": refused, "first": first, "message": msg}))
}

/// Shows the source file in Finder / Explorer (the folder on Linux), so it can be posted on
/// GameBanana or AlliedModders: the community list doesn't take plugins.
pub fn reveal(g: &Game, f: Folder, name: &str) -> Result<Value, String> {
    let path = resolve(&folder_dir(g, f), name)?;
    if !path.is_file() {
        return Err("That file isn't there any more.".into());
    }
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(&path).spawn().map(crate::sys::reap);
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn().map(crate::sys::reap);
    #[cfg(not(any(target_os = "macos", windows)))]
    let r = std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(&path)).spawn().map(crate::sys::reap);
    r.map(|_| json!({"ok": true})).map_err(|e| format!("Couldn't show the folder ({e})."))
}

pub fn delete(g: &Game, f: Folder, name: &str) -> Result<Value, String> {
    if f != Folder::Mine {
        return Err("Only files in My plugins can be deleted here.".into());
    }
    let path = resolve(&folder_dir(g, f), name)?;
    if !path.is_file() {
        return Err("That file isn't there any more.".into());
    }
    keep_mine(g);
    crate::safety::before("Plugin Studio", &format!("Deleted {name}"), &path);
    std::fs::remove_file(&path).map_err(|e| format!("Couldn't delete it ({e})."))?;
    Ok(json!({"ok": true}))
}

/// Renames a file in My plugins (the only folder the Studio renames in).
pub fn rename(g: &Game, f: Folder, from: &str, to: &str) -> Result<Value, String> {
    if f != Folder::Mine {
        return Err("Only files in My plugins can be renamed here.".into());
    }
    let dir = folder_dir(g, f);
    let (src, dest) = (resolve(&dir, from)?, resolve(&dir, to)?);
    if !src.is_file() {
        return Err("That file isn't there any more.".into());
    }
    if from.rsplit('.').next().map(str::to_ascii_lowercase) != to.rsplit('.').next().map(str::to_ascii_lowercase) {
        return Err("Keep the same ending (.sma or .inc).".into());
    }
    if dest.exists() && !from.eq_ignore_ascii_case(to) {
        return Err(format!("There's already a file called {to}. Pick another name."));
    }
    keep_mine(g);
    let bytes = crate::caps::read_file(&src, "file", MAX_SOURCE as u64 * 4)?; // [Caps]
    crate::safety::before("Plugin Studio", &format!("Renamed {from} to {to}"), &dest);
    write_quiet(&dest, &bytes)?;
    if !from.eq_ignore_ascii_case(to) || src.file_name() != dest.file_name() {
        if src.canonicalize().ok() != dest.canonicalize().ok() {
            crate::safety::before("Plugin Studio", &format!("Renamed {from} to {to}"), &src);
            std::fs::remove_file(&src).map_err(|e| format!("Couldn't remove the old name ({e})."))?;
        }
    }
    Ok(json!({"ok": true, "name": to}))
}

// ---- the word list (include files) ----------------------------------------------------

struct ApiCache {
    key: String,
    value: Arc<Value>,
}

/// Every include on the install (and the app's own), read once per change.
pub fn api(g: &Game) -> Value {
    static CACHE: OnceLock<Mutex<Option<ApiCache>>> = OnceLock::new();
    let mut files = include_files(g);
    // amxmodx.inc and its friends first, then the rest of include/, then the mods' folders, so
    // the AMX Mod X definitions win over a mod's copy
    files.sort_by_key(|(n, ..)| (!matches!(n.as_str(), "amxmodx.inc" | "amxconst.inc" | "core.inc" | "float.inc" | "string.inc"), n.contains('/'), n.to_lowercase()));
    let key = files.iter().map(|(n, p, s, m)| format!("{}:{n}:{s}:{m}", p.display())).collect::<Vec<_>>().join(",");
    let cache = CACHE.get_or_init(Default::default);
    if let Some(c) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if c.key == key {
            return (*c.value).clone();
        }
    }
    let mut unread = Vec::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    for (n, p, ..) in &files {
        match std::fs::read(p) {
            Ok(b) => texts.push((n.clone(), decode(&b).0)),
            Err(_) => unread.push(n.clone()),
        }
    }
    for (inc, text) in APP_INCS {
        if !texts.iter().any(|(n, _)| diag::base_name(n) == *inc) {
            texts.push((inc.to_string(), text.to_string()));
        }
    }
    // files with nothing in them the reader could use (all commented out, or only #includes)
    let empty: Vec<&str> = texts.iter().filter(|(n, t)| inc::parse(t, n).is_empty()).map(|(n, _)| n.as_str()).collect();
    let symbols = inc::parse_all(&texts);
    let mut includes: Vec<String> = texts.iter().map(|(n, _)| diag::base_name(n).trim_end_matches(".inc").to_string()).collect();
    includes.sort_by_key(|n| n.to_lowercase());
    includes.dedup();
    let value = Arc::new(json!({"includes": includes, "files": texts.len(), "unread": unread, "empty": empty, "symbols": symbols}));
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(ApiCache { key, value: value.clone() });
    (*value).clone()
}

// ---- compiling ------------------------------------------------------------------------

/// The install a game folder belongs to (to run the compiler in its bottle).
fn install_of(g: &Game) -> Option<crate::steam::Install> {
    crate::steam::find_installs().into_iter().find(|i| i.hl == g.hl).or_else(|| crate::servers::find_install(&g.hl))
}

/// A line Wine or CrossOver printed while the compiler ran, not the compiler itself.
fn wine_noise(line: &str) -> bool {
    let l = line.trim();
    l.contains("fixme:") || l.contains("err:") || l.contains("warn:") || l.contains("[mvk-") || l.contains("MoltenVK") || l.contains("Vulkan") || l.starts_with("VK_") || l.starts_with("wine:")
}

/// What the compiler needs, checked: (compiler, install), or why it can't run.
fn compiler(g: &Game) -> Result<(PathBuf, crate::steam::Install), String> {
    let exe = amxx_dir(g).join("scripting").join(crate::gamebin::AMXXPC);
    if !exe.is_file() {
        return Err("Your game has no AMX Mod X compiler yet. Install the basics (Mods page) first.".into());
    }
    let install = install_of(g).ok_or("Couldn't find your game's install to run the compiler in. Set up your game on the Setup page first.")?;
    if install.kind == "crossover" && !Path::new(CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications, so plugins can't be compiled on this Mac.".into());
    }
    Ok((exe, install))
}

/// The compiler's result for one source.
pub struct Compiled {
    pub ok: bool,
    pub diagnostics: Vec<diag::Diag>,
    pub output: String,
    pub amxx: PathBuf,
}

impl Compiled {
    fn json(&self, ms: u128) -> Value {
        let errors = self.diagnostics.iter().filter(|d| d.severity != "warning").count();
        let warnings = self.diagnostics.len() - errors;
        let size = std::fs::metadata(&self.amxx).map(|m| m.len()).unwrap_or(0);
        let message = if self.ok {
            if warnings == 0 { "Compiled with no problems.".to_string() } else { format!("Compiled, with {warnings} warning{}.", if warnings == 1 { "" } else { "s" }) }
        } else if errors > 0 {
            format!("It didn't compile: {errors} error{} to fix.", if errors == 1 { "" } else { "s" })
        } else {
            format!("It didn't compile. {}", diag::summary(&self.output))
        };
        json!({"ok": self.ok, "message": message, "diagnostics": self.diagnostics, "errors": errors, "warnings": warnings, "output": self.output, "size": size, "ms": ms})
    }
}

/// Runs amxxpc on `src` (with loner.inc on the include path), writing `out`.
pub fn run_compiler(exe: &Path, install: &crate::steam::Install, src: &Path, out: &Path, inc_dir: &Path) -> Result<Compiled, String> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make the build folder ({e})."))?;
    }
    let _ = std::fs::remove_file(out);
    let go = || {
        let mut cmd = crate::setup::core::windows_command(install, exe);
        cmd.arg(src).arg(format!("-o{}", out.display())).arg(format!("-i{}", inc_dir.display()));
        if let Some(d) = exe.parent() {
            cmd.current_dir(d);
        }
        crate::sys::run_output(cmd, Duration::from_secs(120))
    };
    let clean = |r: &Option<(bool, String, String)>| r.as_ref().map(|(_, o, e)| format!("{o}{e}").lines().filter(|l| !wine_noise(l)).collect::<Vec<_>>().join("\n"));
    let mut ran = go();
    // under Wine the compiler now and then stops part way: one more go
    let crashed = |t: &Option<String>| t.as_ref().map(|t| !t.contains("Done.") && !t.to_lowercase().contains("error")).unwrap_or(true);
    if crashed(&clean(&ran)) {
        ran = go();
    }
    let output = clean(&ran).ok_or("The compiler didn't run, or took too long.")?;
    if crate::setup::core::missing_library(&output) {
        return Err(crate::setup::core::LINUX_32BIT_LIBS.into());
    }
    if crashed(&Some(output.clone())) {
        crate::diagnostics::log_error("plugin studio: compile", &output);
        return Err(format!("The compiler stopped part way through{}. Try again.", if install.kind == "crossover" { " (this happens now and then under CrossOver)" } else { "" }));
    }
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let diagnostics = diag::parse(&output, &name);
    Ok(Compiled { ok: output.contains("Done.") && out.is_file(), diagnostics, output, amxx: out.to_path_buf() })
}

/// The include folder handed to the compiler, holding the app's loner.inc.
fn studio_include(g: &Game) -> Result<PathBuf, String> {
    let dir = studio_dir(g).join("include");
    for (inc, text) in APP_INCS {
        let file = dir.join(inc);
        if std::fs::read_to_string(&file).ok().as_deref() != Some(*text) {
            write_quiet(&file, text.as_bytes())?;
        }
    }
    Ok(dir)
}

pub fn compile(g: &Game, f: Folder, name: &str) -> Result<Compiled, String> {
    if !name.to_ascii_lowercase().ends_with(".sma") {
        return Err("Only plugin sources (.sma) compile; include files are compiled with the plugins that use them.".into());
    }
    let src = resolve(&folder_dir(g, f), name)?;
    if !src.is_file() {
        return Err("Save the file first.".into());
    }
    let (exe, install) = compiler(g)?;
    let inc = studio_include(g)?;
    let out = studio_dir(g).join("build").join(format!("{}.amxx", stem(name)));
    run_compiler(&exe, &install, &src, &out, &inc)
}

// ---- trying it in the game ------------------------------------------------------------

fn trying_path(g: &Game) -> PathBuf {
    g.data.join("studio_plugins.json")
}

pub fn trying(g: &Game) -> Vec<String> {
    std::fs::read_to_string(trying_path(g)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// configs/plugins-studio.ini for these plugins (with "debug", so errors name the line).
pub fn studio_ini(names: &[String]) -> String {
    let mut s = String::from("; Plugins you're trying from Loner-Strike's Plugin Studio (Tools > Make).\n; Loaded beside plugins.ini. Take them out in the Plugin Studio.\n");
    for n in names {
        s.push_str(&format!("{n}.amxx debug\n"));
    }
    s
}

fn set_trying(g: &Game, names: &[String]) -> Result<(), String> {
    let am = amxx_dir(g);
    crate::sys::write_atomic(&trying_path(g), serde_json::to_string_pretty(names).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't save the list ({e})."))?;
    let ini = am.join("configs").join(STUDIO_INI);
    if names.is_empty() {
        crate::safety::before("Plugin Studio", "Stopped trying plugins", &ini);
        let _ = std::fs::remove_file(&ini);
        return Ok(());
    }
    write_part(&ini, studio_ini(names).as_bytes())
}

/// The player's own game on this computer, or why plugins can't be tried in it.
fn own_game(st: &Shared) -> Result<Arc<Game>, String> {
    remember_resources(st);
    let g = st.local_game().ok_or(NO_GAME)?;
    if g.remote.is_some() || g.dedicated {
        return Err("The Plugin Studio only tries plugins in your own game on this computer.".into());
    }
    // (so History can list and restore My plugins' versions from the start)
    keep_mine(&g);
    Ok(g)
}

/// Restarts the map through the Game Mode Menu, if a map is up. Ok(false): no map running.
fn restart_map(g: &Game) -> Result<bool, String> {
    if g.state()["online"] != json!(true) {
        return Ok(false);
    }
    match g.command("gm_restartmap", &[]) {
        Ok(_) => Ok(true),
        Err(crate::game::CmdError::Bad(e) | crate::game::CmdError::Game(e)) => Err(e),
    }
}

/// Puts a compiled plugin in the game and loads it (map restart).
pub fn install_trial(g: &Game, name: &str, amxx: &Path) -> Result<(), String> {
    let stem = stem(name).to_string();
    let am = amxx_dir(g);
    if !am.join("configs").is_dir() {
        return Err("AMX Mod X isn't installed in your game yet. Install the basics (Mods page) first.".into());
    }
    trial_allowed(g, &stem)?;
    let mut names = trying(g);
    let dest = am.join("plugins").join(format!("{stem}.amxx"));
    let bytes = std::fs::read(amxx).map_err(|e| format!("Couldn't read the compiled plugin ({e})."))?;
    write_part(&dest, &bytes)?;
    if !names.iter().any(|n| n.eq_ignore_ascii_case(&stem)) {
        names.push(stem);
    }
    set_trying(g, &names)
}

/// Whether a plugin by this name may go in the game: never over one that isn't the Studio's.
pub fn trial_allowed(g: &Game, stem: &str) -> Result<(), String> {
    let dest = amxx_dir(g).join("plugins").join(format!("{stem}.amxx"));
    if let Some(o) = owner_of(stem) {
        let whose = match o {
            Owner::Ours => "one of Loner-Strike's own plugins",
            Owner::GameModeStudio => "a name the Game Mode Studio uses",
            Owner::AmxModX => "one of AMX Mod X's own plugins",
            Owner::Mod => "a plugin of a mode or add-on Loner-Strike installs",
        };
        return Err(format!("{stem} is {whose}, so the Plugin Studio won't put a plugin by that name in your game. Save yours under another name to try it."));
    }
    if dest.exists() && !trying(g).iter().any(|n| n.eq_ignore_ascii_case(stem)) {
        return Err(format!("Your game already has a plugin called {stem}.amxx that the Plugin Studio didn't put there. Save yours under another name to try it."));
    }
    Ok(())
}

pub fn remove_trial(g: &Game, stem: &str) -> Result<(), String> {
    let mut names = trying(g);
    if !names.iter().any(|n| n == stem) || owner_of(stem).is_some() {
        return Err("That plugin isn't one you're trying.".into());
    }
    names.retain(|n| n != stem);
    set_trying(g, &names)?;
    let amxx = amxx_dir(g).join("plugins").join(format!("{stem}.amxx"));
    crate::safety::before("Plugin Studio", &format!("Stopped trying {stem}"), &amxx);
    let _ = std::fs::remove_file(amxx);
    Ok(())
}

// ---- the plugin's log -----------------------------------------------------------------

/// The newest log of each kind (L*.log, error_*.log) in addons/amxmodx/logs.
fn newest_logs(dir: &Path) -> Vec<(String, u64)> {
    let mut best: [Option<(String, std::time::SystemTime, u64)>; 2] = [None, None];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let kind = if name.starts_with('L') && name.ends_with(".log") {
            0
        } else if name.starts_with("error_") && name.ends_with(".log") {
            1
        } else {
            continue;
        };
        let Ok(meta) = e.metadata() else { continue };
        let t = meta.modified().unwrap_or(UNIX_EPOCH);
        if best[kind].as_ref().map(|(_, bt, _)| t > *bt).unwrap_or(true) {
            best[kind] = Some((name, t, meta.len()));
        }
    }
    best.into_iter().flatten().map(|(n, _, len)| (n, len)).collect()
}

/// "L20261112.log:123,error_20261112.log:45" -> pairs.
fn parse_cursor(c: &str) -> Vec<(String, u64)> {
    c.split(',').filter_map(|p| p.rsplit_once(':')).filter_map(|(f, o)| Some((f.to_string(), o.parse().ok()?))).collect()
}

/// The lines in `text` (from a log) about plugin `stem`: its own log lines, load failures
/// naming it, and run time error traces that start with it.
pub fn plugin_lines(text: &str, stem: &str, errors: bool) -> Vec<Value> {
    let amxx = format!("{stem}.amxx").to_lowercase();
    let sma = format!("{stem}.sma").to_lowercase();
    let mut out = Vec::new();
    let mut in_trace = false;
    for raw in text.lines() {
        // "L 11/12/2026 - 20:15:04: text"
        let (at, body) = match raw.strip_prefix("L ").and_then(|r| r.split_once(": ")) {
            Some((stamp, body)) => (stamp.rsplit(" - ").next().unwrap_or("").to_string(), body),
            None => (String::new(), raw),
        };
        let lower = body.to_lowercase();
        let mentions = lower.contains(&amxx) || lower.contains(&sma);
        if lower.contains("displaying debug trace") || lower.contains("run time error") && lower.contains("plugin \"") {
            in_trace = mentions;
        } else if in_trace && !lower.starts_with("[amxx]") {
            in_trace = false;
        }
        if mentions || in_trace {
            // debug mode's "performance issue ... executed more than 1.1ms" notes: harmless, the page folds them
            let slow = lower.contains("performance issue") || lower.contains("executed more than");
            out.push(json!({"at": at, "text": body.trim_end(), "error": !slow && (errors || lower.contains("error") || lower.contains("failed")), "slow": slow}));
        }
    }
    out
}

pub fn log(g: &Game, stem: &str, cursor: Option<&str>) -> Value {
    let dir = amxx_dir(g).join("logs");
    let seen = cursor.map(parse_cursor);
    let mut lines = Vec::new();
    let mut next = Vec::new();
    for (file, len) in newest_logs(&dir) {
        let errors = file.starts_with("error_");
        let from = match &seen {
            // first look: the end of the file (its last 64 KB)
            None => len.saturating_sub(64 << 10),
            Some(list) => match list.iter().find(|(f, _)| *f == file) {
                Some((_, o)) if *o <= len => *o,
                // a new file (new day or map), or one that was cut: from the start
                _ => 0,
            },
        };
        let to = len.min(from + (1 << 20));
        let mut got = Vec::new();
        if to > from {
            use std::io::{Read, Seek, SeekFrom};
            if let Ok(mut fh) = std::fs::File::open(dir.join(&file)) {
                let mut buf = vec![0; (to - from) as usize];
                if fh.seek(SeekFrom::Start(from)).is_ok() && fh.read_exact(&mut buf).is_ok() {
                    // stop at the last whole line; the rest comes next time
                    let end = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1).unwrap_or(0);
                    got = plugin_lines(&decode(&buf[..end]).0, stem, errors);
                    next.push(format!("{file}:{}", from + end as u64));
                } else {
                    next.push(format!("{file}:{from}"));
                }
            }
        } else {
            next.push(format!("{file}:{to}"));
        }
        if seen.is_none() {
            let skip = got.len().saturating_sub(50);
            got.drain(..skip);
        }
        lines.extend(got);
    }
    json!({"lines": lines, "cursor": next.join(",")})
}

// ---- routes ---------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    Json(r.unwrap_or_else(|e| json!({"error": e}))).into_response()
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "Not allowed."}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(INTERNAL.into())))
}

async fn api_overview(State(st): State<Shared>) -> Response {
    let Ok(g) = own_game(&st) else { return answer(Err(NO_GAME.into())) };
    blocking(move || {
        let ready = compiler(&g);
        Ok(json!({
            "game": g.name,
            "compiler": ready.is_ok(),
            "compilerProblem": ready.err(),
            "running": g.state()["online"] == json!(true),
            "trying": trying(&g),
            "folders": [
                {"id": "mine", "label": "My plugins", "path": folder_dir(&g, Folder::Mine), "files": list(&g, Folder::Mine), "hidden": hidden(&g, Folder::Mine)},
                {"id": "game", "label": "Game's plugins", "path": folder_dir(&g, Folder::Game), "files": list(&g, Folder::Game), "hidden": hidden(&g, Folder::Game)},
                {"id": "include", "label": "Include files", "path": folder_dir(&g, Folder::Include), "files": list(&g, Folder::Include), "hidden": hidden(&g, Folder::Include)},
            ],
        }))
    })
    .await
}

#[derive(Deserialize)]
struct FileQ {
    folder: String,
    name: String,
}

fn folder_of(s: &str) -> Result<Folder, String> {
    Folder::parse(s).ok_or_else(|| "That folder isn't one the Plugin Studio uses.".to_string())
}

async fn api_read(State(st): State<Shared>, Query(q): Query<FileQ>) -> Response {
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || read(&g, folder_of(&q.folder)?, &q.name)).await
}

#[derive(Deserialize)]
struct SaveReq {
    folder: String,
    name: String,
    text: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    create: bool,
}

async fn api_save(State(st): State<Shared>, h: HeaderMap, Json(r): Json<SaveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || save(&g, folder_of(&r.folder)?, &r.name, &r.text, &r.encoding, r.create)).await
}

#[derive(Deserialize)]
struct FileReq {
    folder: String,
    name: String,
}

async fn api_delete(State(st): State<Shared>, h: HeaderMap, Json(r): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || delete(&g, folder_of(&r.folder)?, &r.name)).await
}

async fn api_reveal(State(st): State<Shared>, h: HeaderMap, Json(r): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || reveal(&g, folder_of(&r.folder)?, &r.name)).await
}

async fn api_export(State(st): State<Shared>, h: HeaderMap, Json(r): Json<FileReq>) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    let Some(app) = st.app.get().cloned() else { return answer(Err("The save dialog isn't available.".into())) };
    blocking(move || {
        let dir = folder_dir(&g, folder_of(&r.folder)?);
        let Some(picked) = app.dialog().file().set_title("Export the plugin").set_file_name(&r.name).add_filter("Plugin source", &["sma", "inc"]).blocking_save_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let dest = picked.into_path().map_err(|e| e.to_string())?;
        export_to(&dir, &r.name, &dest)
    })
    .await
}

async fn api_import_pick(State(st): State<Shared>, h: HeaderMap) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    let Some(app) = st.app.get().cloned() else { return answer(Err("The file picker isn't available.".into())) };
    blocking(move || {
        let Some(picked) = app.dialog().file().set_title("Import plugins").add_filter("Plugin source", &["sma", "inc"]).blocking_pick_files() else {
            return Ok(json!({"cancelled": true}));
        };
        let files: Vec<PathBuf> = picked.into_iter().filter_map(|f| f.into_path().ok()).take(50).collect();
        let plan = import_plan(&folder_dir(&g, Folder::Mine), &files);
        let id = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let mut list = PICKED.lock().unwrap_or_else(|e| e.into_inner());
        list.push((id.clone(), files));
        if list.len() > 4 {
            list.remove(0);
        }
        Ok(json!({"id": id, "files": plan}))
    })
    .await
}

#[derive(Deserialize)]
struct ImportReq {
    id: String,
    #[serde(default)]
    renames: std::collections::HashMap<String, String>,
}

async fn api_import(State(st): State<Shared>, h: HeaderMap, Json(r): Json<ImportReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let files = PICKED.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(id, _)| *id == r.id).map(|(_, f)| f.clone()).ok_or("Pick the files again.")?;
        keep_mine(&g);
        let v = import_into(&folder_dir(&g, Folder::Mine), &files, &r.renames)?;
        PICKED.lock().unwrap_or_else(|e| e.into_inner()).retain(|(id, _)| *id != r.id);
        Ok(v)
    })
    .await
}

async fn api_reset(State(st): State<Shared>, h: HeaderMap, Json(r): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || reset(&g, folder_of(&r.folder)?, &r.name)).await
}

#[derive(Deserialize)]
struct RenameReq {
    folder: String,
    name: String,
    to: String,
}

async fn api_rename(State(st): State<Shared>, h: HeaderMap, Json(r): Json<RenameReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || rename(&g, folder_of(&r.folder)?, &r.name, &r.to)).await
}

async fn api_words(State(st): State<Shared>) -> Response {
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || Ok(api(&g))).await
}

async fn api_compile(State(st): State<Shared>, h: HeaderMap, Json(r): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let t0 = Instant::now();
        Ok(compile(&g, folder_of(&r.folder)?, &r.name)?.json(t0.elapsed().as_millis()))
    })
    .await
}

async fn api_try(State(st): State<Shared>, h: HeaderMap, Json(r): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let t0 = Instant::now();
        trial_allowed(&g, stem(&r.name))?;
        let built = compile(&g, folder_of(&r.folder)?, &r.name)?;
        let mut v = built.json(t0.elapsed().as_millis());
        if !built.ok {
            return Ok(v);
        }
        install_trial(&g, &r.name, &built.amxx)?;
        let stem = stem(&r.name);
        let (reloaded, message) = match restart_map(&g) {
            Ok(true) => (true, format!("{stem} is in your game. The map is restarting to load it.")),
            Ok(false) => (false, format!("{stem} is in your game. It loads when you start a map (Play).")),
            Err(e) => (false, format!("{stem} is in your game, but the map couldn't be restarted: {e} It loads with the next map.")),
        };
        v["tried"] = json!(true);
        v["reloaded"] = json!(reloaded);
        v["message"] = json!(message);
        v["trying"] = json!(trying(&g));
        Ok(v)
    })
    .await
}

#[derive(Deserialize)]
struct StopReq {
    name: String,
    #[serde(default)]
    restart: bool,
}

async fn api_untry(State(st): State<Shared>, h: HeaderMap, Json(r): Json<StopReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let stem = stem(&r.name).to_string();
        remove_trial(&g, &stem)?;
        // the map restarts the same way Try it does (QA4-T1 L13)
        let (reloaded, message) = match if r.restart { restart_map(&g) } else { Ok(false) } {
            Ok(true) => (true, format!("{stem} is out of your game. The map is restarting without it.")),
            Ok(false) if g.state()["online"] != json!(true) => (false, format!("{stem} is out of your game.")),
            Ok(false) => (false, format!("{stem} is out of your game. It stops with the next map.")),
            Err(e) => (false, format!("{stem} is out of your game, but the map couldn't be restarted: {e} It stops with the next map.")),
        };
        Ok(json!({"ok": true, "message": message, "reloaded": reloaded, "trying": trying(&g)}))
    })
    .await
}

#[derive(Deserialize)]
struct LogQ {
    name: String,
    #[serde(default)]
    cursor: Option<String>,
}

async fn api_log(State(st): State<Shared>, Query(q): Query<LogQ>) -> Response {
    let g = match own_game(&st) {
        Ok(g) => g,
        Err(e) => return answer(Err(e)),
    };
    let stem = stem(&q.name).to_string();
    if !safe_file(&format!("{stem}.sma")) {
        return answer(Err(BAD_NAME.into()));
    }
    blocking(move || Ok(log(&g, &stem, q.cursor.as_deref()))).await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/plugins", get(api_overview))
        .route("/api/tools/plugins/file", get(api_read).post(api_save))
        .route("/api/tools/plugins/delete", post(api_delete))
        .route("/api/tools/plugins/reveal", post(api_reveal))
        .route("/api/tools/plugins/export", post(api_export))
        .route("/api/tools/plugins/import/pick", post(api_import_pick))
        .route("/api/tools/plugins/import", post(api_import))
        .route("/api/tools/plugins/rename", post(api_rename))
        .route("/api/tools/plugins/reset", post(api_reset))
        .route("/api/tools/plugins/words", get(api_words))
        .route("/api/tools/plugins/compile", post(api_compile))
        .route("/api/tools/plugins/try", post(api_try))
        .route("/api/tools/plugins/untry", post(api_untry))
        .route("/api/tools/plugins/log", get(api_log))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(tag: &str) -> (PathBuf, Game) {
        let root = std::env::temp_dir().join(format!("ls-studio-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/addons/amxmodx/scripting/include")).unwrap();
        std::fs::create_dir_all(hl.join("cstrike/addons/amxmodx/configs")).unwrap();
        std::fs::create_dir_all(hl.join("cstrike/addons/amxmodx/plugins")).unwrap();
        std::fs::create_dir_all(hl.join("cstrike/addons/amxmodx/logs")).unwrap();
        let g = Game::with_app_dir(&hl, "cstrike", &root.join("app"));
        (root, g)
    }

    #[test]
    fn names_and_paths_are_safe() {
        for ok in ["my_plugin.sma", "a.sma", "Mode-2.SMA", "helpers.inc", "x.y.sma"] {
            assert!(safe_file(ok), "{ok}");
        }
        for bad in ["../x.sma", "a/b.sma", "a\\b.sma", ".hidden.sma", "x.txt", ".sma", "x..sma", "a b.sma", "", "x.sma.part", &format!("{}.sma", "a".repeat(70)), "ü.sma", "-x.sma"] {
            assert!(!safe_file(bad), "{bad}");
        }
        // names Windows keeps for devices, any case, with or without more after a dot (QA4-T1 T1-4)
        for bad in ["con.sma", "CON.sma", "Nul.inc", "aux.sma", "prn.sma", "com1.sma", "COM9.sma", "lpt3.inc", "con.x.sma"] {
            assert!(!safe_file(bad), "{bad}");
            assert_eq!(name_problem(bad), Some(RESERVED_NAME), "{bad}");
        }
        for ok in ["console.sma", "com0.sma", "com10.sma", "lpt.sma", "nul_mode.sma"] {
            assert!(safe_file(ok), "{ok}");
        }
        assert_eq!(name_problem("a;quit.sma"), Some(BAD_NAME));
        let (root, g) = game("paths");
        let dir = folder_dir(&g, Folder::Mine);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(resolve(&dir, "../../etc.sma").is_err());
        assert_eq!(resolve(&dir, "new.sma").unwrap(), dir.join("new.sma"));
        // a link out of the folder is refused
        #[cfg(unix)]
        {
            std::fs::write(root.join("outside.sma"), "x").unwrap();
            std::os::unix::fs::symlink(root.join("outside.sma"), dir.join("link.sma")).unwrap();
            assert!(resolve(&dir, "link.sma").is_err());
            assert!(read(&g, Folder::Mine, "link.sma").is_err());
            assert!(save(&g, Folder::Mine, "link.sma", "y", "utf8", false).is_err());
            assert_eq!(std::fs::read_to_string(root.join("outside.sma")).unwrap(), "x");
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn save_read_list_delete() {
        let (root, g) = game("files");
        let r = save(&g, Folder::Mine, "first.sma", "#include <amxmodx>\n", "utf8", true).unwrap();
        assert_eq!(r["ok"], true);
        assert!(save(&g, Folder::Mine, "first.sma", "again", "utf8", true).unwrap_err().contains("already"));
        save(&g, Folder::Mine, "first.sma", "// changed é\n", "utf8", false).unwrap();
        assert_eq!(read(&g, Folder::Mine, "first.sma").unwrap()["text"], "// changed é\n");
        // no .part left behind
        let names: Vec<String> = std::fs::read_dir(folder_dir(&g, Folder::Mine)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["first.sma"]);
        // Latin-1 files stay Latin-1
        std::fs::write(folder_dir(&g, Folder::Mine).join("old.sma"), b"// caf\xe9\n").unwrap();
        let v = read(&g, Folder::Mine, "old.sma").unwrap();
        assert_eq!((v["text"].as_str().unwrap(), v["encoding"].as_str().unwrap()), ("// café\n", "latin1"));
        save(&g, Folder::Mine, "old.sma", "// café!\n", "latin1", false).unwrap();
        assert_eq!(std::fs::read(folder_dir(&g, Folder::Mine).join("old.sma")).unwrap(), b"// caf\xe9!\n");

        // the game's folder: Loner-Strike's own plugins are read only
        std::fs::write(folder_dir(&g, Folder::Game).join("gamemodes.sma"), "x").unwrap();
        assert!(save(&g, Folder::Game, "gamemodes.sma", "y", "utf8", false).unwrap_err().contains("Loner-Strike's own"));
        assert_eq!(read(&g, Folder::Game, "gamemodes.sma").unwrap()["readOnly"].as_str().map(|s| s.contains("copy")), Some(true));
        save(&g, Folder::Game, "theirs.sma", "z", "utf8", true).unwrap();
        assert!(delete(&g, Folder::Game, "theirs.sma").is_err());
        // includes: read only; loner.inc is there even if the game doesn't have it
        assert!(save(&g, Folder::Include, "loner.inc", "x", "utf8", false).is_err());
        assert_eq!(read(&g, Folder::Include, "loner.inc").unwrap()["text"], LONER_INC);
        assert!(list(&g, Folder::Include).iter().any(|v| v["name"] == "loner.inc"));

        let mine = list(&g, Folder::Mine);
        assert_eq!(mine.iter().map(|v| v["name"].as_str().unwrap()).collect::<Vec<_>>(), ["first.sma", "old.sma"]);
        delete(&g, Folder::Mine, "first.sma").unwrap();
        assert!(read(&g, Folder::Mine, "first.sma").is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn trying_a_plugin() {
        let (root, g) = game("try");
        let built = root.join("built.amxx");
        std::fs::write(&built, b"AMXX").unwrap();
        install_trial(&g, "mymode.sma", &built).unwrap();
        let am = amxx_dir(&g);
        assert_eq!(std::fs::read(am.join("plugins/mymode.amxx")).unwrap(), b"AMXX");
        assert_eq!(std::fs::read_to_string(am.join("configs/plugins-studio.ini")).unwrap().lines().last(), Some("mymode.amxx debug"));
        assert_eq!(trying(&g), ["mymode"]);
        // again: replaced, listed once
        install_trial(&g, "mymode.sma", &built).unwrap();
        assert_eq!(trying(&g), ["mymode"]);
        // a plugin that was already there, and Loner-Strike's own, are left alone
        std::fs::write(am.join("plugins/someone.amxx"), b"theirs").unwrap();
        assert!(install_trial(&g, "someone.sma", &built).is_err());
        assert_eq!(std::fs::read(am.join("plugins/someone.amxx")).unwrap(), b"theirs");
        assert!(install_trial(&g, "gamemodes.sma", &built).is_err());
        // out again
        assert!(remove_trial(&g, "someone").is_err());
        remove_trial(&g, "mymode").unwrap();
        assert!(!am.join("plugins/mymode.amxx").exists());
        assert!(!am.join("configs/plugins-studio.ini").exists());
        assert!(am.join("plugins/someone.amxx").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn following_the_log() {
        let (root, g) = game("log");
        let logs = amxx_dir(&g).join("logs");
        std::fs::write(logs.join("L20261112.log"), "L 11/12/2026 - 20:00:00: Log file started\nL 11/12/2026 - 20:00:01: [mymode.amxx] hello\nL 11/12/2026 - 20:00:02: [other.amxx] not mine\n").unwrap();
        std::fs::write(
            logs.join("error_20261112.log"),
            "L 11/12/2026 - 20:00:03: [AMXX] Displaying debug trace (plugin \"mymode.amxx\", version \"1.0\")\nL 11/12/2026 - 20:00:03: [AMXX] Run time error 4: index out of bounds \nL 11/12/2026 - 20:00:03: [AMXX]    [0] mymode.sma::on_spawn (line 12)\nL 11/12/2026 - 20:00:04: [AMXX] Displaying debug trace (plugin \"other.amxx\", version \"1.0\")\nL 11/12/2026 - 20:00:04: [AMXX] Run time error 10: native error\n",
        )
        .unwrap();
        let v = log(&g, "mymode", None);
        let texts: Vec<&str> = v["lines"].as_array().unwrap().iter().map(|l| l["text"].as_str().unwrap()).collect();
        assert_eq!(texts.len(), 4, "{texts:?}");
        assert_eq!(texts[0], "[mymode.amxx] hello");
        assert!(texts[3].contains("(line 12)"));
        assert_eq!(v["lines"][0]["at"], "20:00:01");
        assert_eq!(v["lines"][1]["error"], true);
        // nothing new: nothing; a new line: just that
        let cur = v["cursor"].as_str().unwrap().to_string();
        assert_eq!(log(&g, "mymode", Some(&cur))["lines"].as_array().unwrap().len(), 0);
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(logs.join("L20261112.log")).unwrap();
        f.write_all(b"L 11/12/2026 - 20:01:00: [mymode.amxx] again\nL 11/12/2026 - 20:01:01: [mymode.amxx] half").unwrap();
        drop(f);
        let v = log(&g, "mymode", Some(&cur));
        assert_eq!(v["lines"].as_array().unwrap().iter().map(|l| l["text"].as_str().unwrap()).collect::<Vec<_>>(), ["[mymode.amxx] again"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn studio_ini_lists_with_debug() {
        assert_eq!(studio_ini(&["a".into(), "b".into()]).lines().filter(|l| !l.starts_with(';')).collect::<Vec<_>>(), ["a.amxx debug", "b.amxx debug"]);
    }

    #[test]
    fn words_from_loner_inc() {
        let syms = inc::parse(LONER_INC, "loner.inc");
        let find = |n: &str| syms.iter().find(|s| s.name == n).unwrap_or_else(|| panic!("{n}"));
        assert_eq!(find("ls_give_weapon").kind, "stock");
        assert!(find("ls_give_weapon").doc.contains("spare ammo"));
        assert_eq!(find("ls_give_weapon").params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["id", "weapon", "ammo"]);
        assert_eq!(find("mk_role_set").kind, "native");
        assert_eq!(find("LS_HUD_CENTER").kind, "constant");
        assert_eq!(find("ls_hud").params.last().unwrap().name, "...");
    }

    /// Every plugin Loner-Strike (or AMX Mod X, a mode or add-on, the Game Mode Studio) ships is read
    /// only in every folder it shows in, and nothing (save, try it, rename, delete) writes over it.
    #[test]
    fn shipped_plugins_are_read_only_everywhere() {
        let (root, g) = game("protect");
        let am = amxx_dir(&g);
        let res = crate::setup::repo_root();
        // some of every kind, in the game folder with their compiled plugins
        let mut names: Vec<String> = crate::setup::core::OUR_PLUGINS.iter().map(|s| s.to_string()).collect();
        names.extend(["admin", "restmenu", "statsx", "hidenseek", "gungame", "amx_gore_ultimate", "studio_mymode"].map(String::from));
        for n in &names {
            std::fs::write(folder_dir(&g, Folder::Game).join(format!("{n}.sma")), "// theirs\n").unwrap();
            std::fs::write(am.join("plugins").join(format!("{n}.amxx")), b"theirs").unwrap();
        }
        // a mod's plugin the catalog doesn't name (copied in by a mode's files) and the player's own
        std::fs::write(folder_dir(&g, Folder::Game).join("zombie_plague40.sma"), "// theirs\n").unwrap();
        std::fs::write(am.join("plugins/zombie_plague40.amxx"), b"theirs").unwrap();
        std::fs::write(folder_dir(&g, Folder::Game).join("mine_in_game.sma"), "// mine\n").unwrap();
        std::fs::write(folder_dir(&g, Folder::Include).join("loner.inc"), "// old copy\n").unwrap();
        names.push("zombie_plague40".into());
        assert!(catalog_stems(&res).contains("hidenseek") && catalog_stems(&res).contains("sh_batman"));

        let listed = list(&g, Folder::Game);
        for n in &names {
            let file = format!("{n}.sma");
            let row = listed.iter().find(|v| v["name"] == file.as_str()).unwrap_or_else(|| panic!("{file} not listed"));
            assert_eq!(row["readOnly"], true, "{file} listed as writable");
            assert!(read(&g, Folder::Game, &file).unwrap()["readOnly"].is_string(), "{file} opened writable");
            assert!(save(&g, Folder::Game, &file, "// changed\n", "utf8", false).is_err(), "{file} saved over");
            assert!(delete(&g, Folder::Game, &file).is_err());
            assert!(rename(&g, Folder::Game, &file, "other.sma").is_err());
            // a copy in My plugins is fine, but it can't go in the game under that name
            save(&g, Folder::Mine, &file, "// my copy\n", "utf8", false).unwrap();
            assert!(trial_allowed(&g, n).is_err(), "{n} could be tried over");
            assert!(install_trial(&g, &file, &root.join("nothing.amxx")).is_err());
            assert_eq!(std::fs::read_to_string(folder_dir(&g, Folder::Game).join(&file)).unwrap(), "// theirs\n");
            assert_eq!(std::fs::read(am.join("plugins").join(format!("{n}.amxx"))).unwrap(), b"theirs");
            // case doesn't get round it
            assert!(trial_allowed(&g, &n.to_uppercase()).is_err());
        }
        // and taking a plugin out never removes one of those
        std::fs::create_dir_all(&g.data).unwrap();
        std::fs::write(trying_path(&g), r#"["gamemodes","admin"]"#).unwrap();
        assert!(remove_trial(&g, "gamemodes").is_err() && remove_trial(&g, "admin").is_err());
        assert!(am.join("plugins/gamemodes.amxx").exists() && am.join("plugins/admin.amxx").exists());
        std::fs::remove_file(trying_path(&g)).unwrap();
        // includes: all read only, the app's own too (in the game folder or not)
        for inc in ["loner.inc", "ls_components.inc"] {
            assert!(list(&g, Folder::Include).iter().any(|v| v["name"] == inc && v["readOnly"] == true));
            assert!(read(&g, Folder::Include, inc).unwrap()["readOnly"].is_string());
            assert!(save(&g, Folder::Include, inc, "x", "utf8", false).is_err());
            assert!(save(&g, Folder::Game, inc, "x", "utf8", true).is_err());
            assert!(delete(&g, Folder::Include, inc).is_err());
        }
        // the player's own file in the game folder stays theirs
        assert!(protection(&g, Folder::Game, "mine_in_game.sma").is_none());
        save(&g, Folder::Game, "mine_in_game.sma", "// changed\n", "utf8", false).unwrap();
        // a new file with a shipped name can't be made in the game folder either
        assert!(save(&g, Folder::Game, "gm_battleroyale.sma", "x", "utf8", true).is_err() || folder_dir(&g, Folder::Game).join("gm_battleroyale.sma").exists());
        std::fs::remove_file(folder_dir(&g, Folder::Game).join("gm_battleroyale.sma")).unwrap();
        assert!(save(&g, Folder::Game, "gm_battleroyale.sma", "x", "utf8", true).is_err());
        assert!(save(&g, Folder::Game, "studio_other.sma", "x", "utf8", true).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    /// The same through the routes (what the page and AI agents use).
    #[tokio::test]
    async fn routes_refuse_shipped_plugins() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;
        let (root, g) = game("protect-routes");
        let am = amxx_dir(&g);
        std::fs::write(folder_dir(&g, Folder::Game).join("gamemodes.sma"), "// ours\n").unwrap();
        std::fs::write(am.join("plugins/gamemodes.amxx"), b"ours").unwrap();
        let g = Arc::new(g);
        let st: Shared = Arc::new(crate::server::AppState {
            current: std::sync::RwLock::new(Some(g.clone())),
            hosts: std::sync::RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: std::sync::RwLock::new(vec![]),
            token: std::sync::RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let post = |uri: &str, body: Value| {
            let st = st.clone();
            let req = Request::builder().method("POST").uri(uri).header("host", "127.0.0.1:8027").header("x-panel", "1").header("content-type", "application/json").body(Body::from(body.to_string())).unwrap();
            async move {
                let res = routes().with_state(st).oneshot(req).await.unwrap();
                serde_json::from_slice::<Value>(&axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap()
            }
        };
        let r = post("/api/tools/plugins/file", json!({"folder": "game", "name": "gamemodes.sma", "text": "// mine"})).await;
        assert!(r["error"].as_str().unwrap().contains("Loner-Strike's own"), "{r}");
        let r = post("/api/tools/plugins/delete", json!({"folder": "game", "name": "gamemodes.sma"})).await;
        assert!(r["error"].is_string());
        let r = post("/api/tools/plugins/rename", json!({"folder": "game", "name": "gamemodes.sma", "to": "x.sma"})).await;
        assert!(r["error"].is_string());
        post("/api/tools/plugins/file", json!({"folder": "mine", "name": "gamemodes.sma", "text": "// mine", "create": true})).await;
        let r = post("/api/tools/plugins/try", json!({"folder": "mine", "name": "gamemodes.sma"})).await;
        assert!(r["error"].as_str().unwrap().contains("Loner-Strike's own"), "{r}");
        let r = post("/api/tools/plugins/rename", json!({"folder": "mine", "name": "gamemodes.sma", "to": "../x.sma"})).await;
        assert!(r["error"].is_string());
        let r = post("/api/tools/plugins/untry", json!({"name": "gamemodes"})).await;
        assert!(r["error"].is_string());
        assert_eq!(std::fs::read_to_string(folder_dir(&g, Folder::Game).join("gamemodes.sma")).unwrap(), "// ours\n");
        assert_eq!(std::fs::read(am.join("plugins/gamemodes.amxx")).unwrap(), b"ours");
        // without the app's header nothing changes at all
        let req = Request::builder().method("POST").uri("/api/tools/plugins/file").header("host", "127.0.0.1:8027").header("content-type", "application/json").body(Body::from(json!({"folder": "mine", "name": "a.sma", "text": "x"}).to_string())).unwrap();
        assert_eq!(routes().with_state(st.clone()).oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
        std::fs::remove_dir_all(&root).ok();
    }

    /// My plugins live outside the game folder, but the safety net keeps their versions too.
    #[test]
    fn my_plugins_are_kept_by_the_safety_net() {
        let (root, g) = game("kept");
        save(&g, Folder::Mine, "keepme.sma", "// one\n", "utf8", true).unwrap();
        let path = folder_dir(&g, Folder::Mine).join("keepme.sma");
        assert!(crate::safety::before("Plugin Studio", "test", &path).is_some(), "My plugins aren't kept");
        assert_eq!(read(&g, Folder::Mine, "keepme.sma").unwrap()["history"], "my-plugins/keepme.sma");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn slow_function_notes_are_marked() {
        let text = "L 11/14/2026 - 10:00:00: [AMXX] Plugin (\"qa.amxx\") has a performance issue. Function cmd_x executed more than 1.1ms.\nL 11/14/2026 - 10:00:01: [qa.amxx] hello\n";
        let l = plugin_lines(text, "qa", false);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0]["slow"].as_bool(), l[0]["error"].as_bool()), (Some(true), Some(false)));
        assert_eq!(l[1]["slow"], false);
    }

    #[test]
    fn export_carries_own_includes_and_import_never_overwrites() {
        let root = std::env::temp_dir().join(format!("ls-plugins-io-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (mine, out, pick) = (root.join("mine"), root.join("out"), root.join("pick"));
        for d in [&mine, &out, &pick] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(mine.join("hello.sma"), "#include <amxmodx>\n#include \"helpers\"\npublic plugin_init() {}\n").unwrap();
        std::fs::write(mine.join("helpers.inc"), "#include <more.inc>\nstock x() {}\n").unwrap();
        std::fs::write(mine.join("more.inc"), "stock y() {}\n").unwrap();
        assert_eq!(own_includes(&mine, &std::fs::read_to_string(mine.join("hello.sma")).unwrap()), vec!["helpers.inc", "more.inc"]);
        // export: the .sma where picked, its own includes beside it; a different file there stays
        std::fs::write(out.join("more.inc"), "theirs").unwrap();
        let v = export_to(&mine, "hello.sma", &out.join("hello_v2.sma")).unwrap();
        assert_eq!(v["saved"], json!(["hello_v2.sma", "helpers.inc"]));
        assert_eq!(v["kept"], json!(["more.inc"]));
        assert_eq!(std::fs::read_to_string(out.join("more.inc")).unwrap(), "theirs");
        // import: refusals
        assert!(import_problem("con.sma", b"x").unwrap().contains("Windows keeps"));
        assert!(import_problem("NUL.inc", b"x").is_some() && import_problem("lpt3.sma", b"x").is_some());
        assert!(import_problem("a;b.sma", b"x").is_some());
        assert!(import_problem("x.amxx", b"x").unwrap().contains(".sma and .inc"));
        assert!(import_problem("bin.sma", &[0x4d, 0x5a, 0, 1, 2, 3]).unwrap().contains("isn't a text file"));
        assert!(import_problem("big.sma", &vec![b'a'; MAX_SOURCE + 1]).unwrap().contains("too big"));
        assert_eq!(import_problem("ok.sma", b"public plugin_init() {}\r\n\tx();\n"), None);
        // import: a taken name is never overwritten; renamed, it comes in
        std::fs::write(pick.join("hello.sma"), "new one").unwrap();
        std::fs::write(pick.join("con.inc"), "x").unwrap();
        let files = vec![pick.join("hello.sma"), pick.join("con.inc")];
        let plan = import_plan(&mine, &files);
        assert_eq!(plan[0]["taken"], true);
        assert!(plan[1]["problem"].as_str().unwrap().contains("Windows keeps"));
        let none = std::collections::HashMap::new();
        assert!(import_into(&mine, &files, &none).unwrap_err().contains("already a file called hello.sma"));
        assert!(std::fs::read_to_string(mine.join("hello.sma")).unwrap().contains("plugin_init"));
        let mut ren = std::collections::HashMap::new();
        ren.insert("hello.sma".to_string(), "hello2.inc".to_string());
        assert!(import_into(&mine, &files, &ren).unwrap_err().contains("keep the .sma ending"));
        ren.insert("hello.sma".to_string(), "hello2.sma".to_string());
        let v = import_into(&mine, &files, &ren).unwrap();
        assert_eq!(v["imported"], json!(["hello2.sma"]));
        assert_eq!(v["first"], "hello2.sma");
        assert_eq!(std::fs::read_to_string(mine.join("hello2.sma")).unwrap(), "new one");
        assert!(!mine.join("con.inc").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rename_in_my_plugins() {
        let (root, g) = game("rename");
        save(&g, Folder::Mine, "a.sma", "// a\n", "utf8", true).unwrap();
        save(&g, Folder::Mine, "b.sma", "// b\n", "utf8", true).unwrap();
        assert!(rename(&g, Folder::Mine, "a.sma", "b.sma").unwrap_err().contains("already"));
        assert!(rename(&g, Folder::Mine, "a.sma", "a.inc").is_err());
        assert!(rename(&g, Folder::Mine, "a.sma", "../a.sma").is_err());
        assert!(rename(&g, Folder::Mine, "nothere.sma", "c.sma").is_err());
        // QA4-T1 T1-4: a name Windows keeps for a device, or with ; in it
        assert_eq!(rename(&g, Folder::Mine, "a.sma", "con.sma").unwrap_err(), RESERVED_NAME);
        assert_eq!(rename(&g, Folder::Mine, "a.sma", "LPT1.sma").unwrap_err(), RESERVED_NAME);
        assert!(save(&g, Folder::Mine, "Nul.sma", "// x\n", "utf8", true).is_err());
        assert!(rename(&g, Folder::Mine, "a.sma", "a;quit.sma").unwrap_err().contains("only letters"));
        rename(&g, Folder::Mine, "a.sma", "c.sma").unwrap();
        assert_eq!(read(&g, Folder::Mine, "c.sma").unwrap()["text"], "// a\n");
        assert!(read(&g, Folder::Mine, "a.sma").is_err());
        assert_eq!(read(&g, Folder::Mine, "b.sma").unwrap()["text"], "// b\n");
        std::fs::remove_dir_all(&root).ok();
    }

    /// Loner-Strike's version back: for our plugins changed by hand and for an old copy of an
    /// include; never for the player's files, mods or Studio plugins.
    #[test]
    fn reset_to_loner_strikes_version() {
        let (root, g) = game("reset");
        let shipped = std::fs::read(crate::setup::repo_root().join("plugins/scripting/gm_vip.sma")).unwrap();
        let game_vip = folder_dir(&g, Folder::Game).join("gm_vip.sma");
        std::fs::write(&game_vip, &shipped).unwrap();
        assert_eq!(differs_from_shipped(&g, Folder::Game, "gm_vip.sma"), Some(false));
        // the same with Windows line ends is the same
        std::fs::write(&game_vip, String::from_utf8_lossy(&shipped).replace('\n', "\r\n")).unwrap();
        assert_eq!(differs_from_shipped(&g, Folder::Game, "gm_vip.sma"), Some(false));
        std::fs::write(&game_vip, "// edited by hand\n").unwrap();
        assert_eq!(differs_from_shipped(&g, Folder::Game, "gm_vip.sma"), Some(true));
        assert!(list(&g, Folder::Game).iter().any(|v| v["name"] == "gm_vip.sma" && v["changed"] == true));
        let v = read(&g, Folder::Game, "gm_vip.sma").unwrap();
        assert_eq!(v["reset"]["differs"], true);
        assert_eq!(v["history"], "addons/amxmodx/scripting/gm_vip.sma");
        // (no compiler here: the source comes back, and it says the plugin wasn't rebuilt)
        let r = reset(&g, Folder::Game, "gm_vip.sma").unwrap();
        assert!(r["message"].as_str().unwrap().contains("back to Loner-Strike's version"), "{r}");
        assert_eq!(std::fs::read(&game_vip).unwrap(), shipped);
        assert_eq!(differs_from_shipped(&g, Folder::Game, "gm_vip.sma"), Some(false));
        // an old loner.inc in the game's include folder
        std::fs::write(folder_dir(&g, Folder::Include).join("loner.inc"), "// old\n").unwrap();
        assert_eq!(read(&g, Folder::Include, "loner.inc").unwrap()["reset"]["differs"], true);
        reset(&g, Folder::Include, "loner.inc").unwrap();
        assert_eq!(std::fs::read_to_string(folder_dir(&g, Folder::Include).join("loner.inc")).unwrap(), LONER_INC);
        // no reset for what Loner-Strike doesn't ship, or isn't there
        save(&g, Folder::Mine, "mine.sma", "x", "utf8", true).unwrap();
        std::fs::write(folder_dir(&g, Folder::Game).join("admin.sma"), "x").unwrap();
        std::fs::write(folder_dir(&g, Folder::Game).join("studio_x.sma"), "x").unwrap();
        for (f, n) in [(Folder::Mine, "mine.sma"), (Folder::Game, "admin.sma"), (Folder::Game, "studio_x.sma"), (Folder::Game, "gm_people.sma"), (Folder::Include, "ls_components.inc")] {
            assert!(reset(&g, f, n).is_err(), "{n}");
            assert!(read(&g, f, n).map(|v| v["reset"].is_null()).unwrap_or(true), "{n}");
        }
        assert!(read(&g, Folder::Game, "admin.sma").unwrap()["resetHint"].as_str().unwrap().contains("Repair"));
        std::fs::remove_dir_all(&root).ok();
    }

    /// Every include folder on the install: include/ and its subfolders, and the mods' *_include.
    #[test]
    fn include_files_from_every_folder() {
        let (root, g) = game("incs");
        let s = amxx_dir(&g).join("scripting");
        std::fs::create_dir_all(s.join("include/sub")).unwrap();
        std::fs::create_dir_all(s.join("zp5_include")).unwrap();
        std::fs::create_dir_all(s.join("notinc")).unwrap();
        std::fs::write(s.join("include/amxmodx.inc"), "native get_user_name(index, name[], len);\n").unwrap();
        std::fs::write(s.join("include/sub/deep.inc"), "native deep_one();\n").unwrap();
        std::fs::write(s.join("zp5_include/zp50_core.inc"), "/** Zombie? */\nnative zp_core_is_zombie(id);\n").unwrap();
        std::fs::write(s.join("notinc/x.inc"), "native nope();\n").unwrap();
        std::fs::write(s.join("zp5_include/empty.inc"), "/* nothing */\n").unwrap();
        let names: Vec<String> = include_files(&g).into_iter().map(|(n, ..)| n).collect();
        assert_eq!(names, ["amxmodx.inc", "sub/deep.inc", "zp5_include/empty.inc", "zp5_include/zp50_core.inc"]);
        let v = api(&g);
        let syms = v["symbols"].as_array().unwrap();
        let z = syms.iter().find(|s| s["name"] == "zp_core_is_zombie").unwrap();
        assert_eq!((z["file"].as_str(), z["doc"].as_str()), (Some("zp5_include/zp50_core.inc"), Some("Zombie?")));
        assert!(syms.iter().any(|s| s["name"] == "deep_one") && !syms.iter().any(|s| s["name"] == "nope"));
        assert!(v["includes"].as_array().unwrap().iter().any(|n| n == "zp50_core"));
        assert_eq!(v["empty"], json!(["zp5_include/empty.inc"]));
        // they open by the name shown, and only those
        assert!(read(&g, Folder::Include, "zp5_include/zp50_core.inc").unwrap()["text"].as_str().unwrap().contains("zp_core_is_zombie"));
        for bad in ["../include/amxmodx.inc", "notinc/x.inc", "zp5_include/../../x.inc", "/etc/passwd"] {
            assert!(read(&g, Folder::Include, bad).is_err(), "{bad}");
        }
        assert!(list(&g, Folder::Include).iter().any(|v| v["name"] == "zp5_include/zp50_core.inc"));
        std::fs::remove_dir_all(&root).ok();
    }

    /// The real include files: `LS_TEST_GAME=<Half-Life folder> cargo test real_includes -- --nocapture`
    #[test]
    fn real_includes() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let g = Game::with_app_dir(Path::new(&hl), "cstrike", &std::env::temp_dir().join("ls-studio-real"));
        let v = api(&g);
        let syms = v["symbols"].as_array().unwrap();
        let find = |n: &str| syms.iter().find(|s| s["name"] == n).unwrap_or_else(|| panic!("no {n}")).clone();
        println!("{} include files ({} names), {} symbols; unread {}, nothing usable in {}", v["files"], v["includes"].as_array().unwrap().len(), syms.len(), v["unread"], v["empty"]);
        assert_eq!(v["unread"], json!([]));
        assert!(syms.len() > 2000);
        // LS_TEST_DUMP=<folder>: the word list as JSON, to look at
        if let Ok(dir) = std::env::var("LS_TEST_DUMP") {
            std::fs::write(Path::new(&dir).join("plugin-words.json"), serde_json::to_string_pretty(&v).unwrap()).ok();
        }
        let s = find("get_user_name");
        assert_eq!(s["kind"], "native");
        assert_eq!(s["file"], "amxmodx.inc");
        assert!(!s["doc"].as_str().unwrap().is_empty());
        assert_eq!(s["params"].as_array().unwrap().len(), 3);
        assert!(!s["params"][0]["doc"].as_str().unwrap().is_empty());
        assert_eq!(find("plugin_init")["kind"], "forward");
        assert_eq!(find("client_putinserver")["kind"], "forward");
        assert_eq!(find("PLUGIN_HANDLED")["kind"], "define");
        assert_eq!(find("get_gametime")["tag"], "Float");
        assert_eq!(find("CS_TEAM_CT")["kind"], "constant");
        assert_eq!(find("RegisterHam")["kind"], "native");
        assert_eq!(find("Ham_Spawn")["kind"], "constant");
        assert_eq!(find("charsmax")["kind"], "macro");
        assert_eq!(find("fm_set_user_health")["kind"], "stock");
        assert_eq!(find("ls_give_weapon")["file"], "loner.inc");
        // the second time it's the cache
        let t0 = Instant::now();
        let again = api(&g);
        assert!(t0.elapsed() < Duration::from_millis(500));
        assert_eq!(again["symbols"].as_array().unwrap().len(), syms.len());
        for name in ["set_task", "register_clcmd", "client_print_color", "give_item", "cs_set_user_money", "ArrayCreate", "formatex"] {
            let s = find(name);
            println!("{}: {} params, doc {} chars", s["detail"], s["params"].as_array().unwrap().len(), s["doc"].as_str().unwrap().len());
        }
    }

    /// Compiles loner.inc's example with the real compiler, when there is one:
    /// `LS_TEST_GAME=<Half-Life folder> cargo test real_compile -- --nocapture`
    #[test]
    fn real_compile() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = PathBuf::from(hl);
        let exe = hl.join("cstrike/addons/amxmodx/scripting/amxxpc.exe");
        let crossover = cfg!(target_os = "macos");
        if !exe.is_file() || (crossover && !Path::new(CROSSOVER_WINE).is_file()) {
            println!("no compiler: skipped");
            return;
        }
        let install = crate::steam::Install { hl: hl.clone(), kind: if crossover { "crossover".into() } else { "windows".into() }, bottle: crossover.then(|| "Steam".to_string()), games: vec!["cstrike".into()], dedicated: false };
        let tmp = std::env::temp_dir().join(format!("ls-studio-compile-{}", std::process::id()));
        let inc = tmp.join("include");
        std::fs::create_dir_all(&inc).unwrap();
        for (n, text) in APP_INCS {
            std::fs::write(inc.join(n), text).unwrap();
        }
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/scripting/examples/ls_example_mode.sma");
        let src = tmp.join("ls_example_mode.sma");
        std::fs::copy(&example, &src).unwrap();
        let r = run_compiler(&exe, &install, &src, &tmp.join("ls_example_mode.amxx"), &inc).unwrap();
        println!("{}", r.output);
        assert!(r.ok, "{}", r.output);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        // a broken one: errors on the right lines, in plain words
        std::fs::write(tmp.join("broken.sma"), "#include <amxmodx>\n#include <loner>\npublic plugin_init()\n{\n\tls_give_weapon(1)\n\tnope()\n}\n").unwrap();
        let r = run_compiler(&exe, &install, &tmp.join("broken.sma"), &tmp.join("broken.amxx"), &inc).unwrap();
        assert!(!r.ok);
        assert!(r.diagnostics.iter().any(|d| d.line == 6 && d.code == 17 && d.in_source), "{:?}", r.diagnostics);
        assert!(r.diagnostics.iter().any(|d| d.line == 5 && d.in_source), "{:?}", r.diagnostics);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
