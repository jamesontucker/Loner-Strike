//! The Game Mode Studio (Beta), server side: graphs saved as files in the app's data folder,
//! checks, graph → Pawn (gen.rs), compiling with the game's own amxxpc, installing the result
//! as an ordinary mode of the Game Mode Menu, and the debug link with a Playtest build
//! (debug.rs). `/api/tools/studio/*`, on the app window's listener only.
//!
//! Why the generator is Rust and not TypeScript: compiling and installing happen here anyway
//! (amxxpc, the game's files), the checks have to run here before anything is written into the
//! game, and the MCP connector (Rust) can then build modes without the page. The editor reads
//! the same node registry (nodes.json) and asks `/validate` for the problems it shows on nodes.
//!
//! What a compile installs (in the player's game, `addons/amxmodx/`):
//! - `plugins/studio_<id>.amxx` (the source is kept at `scripting/studio_<id>.sma`, where the
//!   Plugin Studio opens it read only, with the `loner.inc` it was built against in
//!   `scripting/studio/include/`),
//! - `configs/gamemodes/<id>.plugins.ini` (just that plugin) and `<id>.cfg`,
//! - a line in `configs/gamemodes/modes.ini`, so the mode shows in the Game Mode Menu, the Play
//!   page and Playtest. A Loner-Strike update rewrites modes.ini: compiling again puts it back.
//!
//! Errors are `{error}` with status 200 like the rest of the app; POSTs need the panel header.

pub mod debug;
pub mod gen;
pub mod expand;
pub mod graph;
pub mod limits;
pub mod kit;
pub mod testrun;
pub mod tables;
pub mod history;
pub mod settings;
pub mod modes_reload;
// the bones and attachments of a model, for the Studio's picker (Y)
pub mod modelpoints;
// the library items that ship with the app (make_starter.py writes the list; X2)
mod builtin_library;
#[cfg(test)]
mod starter_tests; // X2: the Starter library
#[cfg(test)]
mod x2_pins_tests; // X2: many pins, wireable For Each / Pick Random
#[cfg(test)]
mod help_tests;
#[cfg(test)]
mod examples_tests;
#[cfg(test)]
mod compat_tests; // the 1.0 compatibility suite (docs/notes/StudioCompat.md)
#[cfg(test)]
mod timeline_tests;
#[cfg(test)]
mod finalcheck_tests; // P5: the final check's installer (docs/notes/FinalCheck.md)
#[cfg(test)]
mod proj_tests; // T1fix: projectiles

use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use graph::{Graph, Problem};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

const NO_GAME: &str = "Set up the game on this computer first.";
const NOT_HERE: &str = "The Game Mode Studio installs modes into the game on this computer. Switch to it (CS 1.6 or Condition Zero on this computer) first.";
const BAD_ID: &str = "That isn't a mode id (2 to 24 small letters, digits or _, starting with a letter).";
const INTERNAL: &str = "Loner-Strike hit an internal error in the Game Mode Studio. Try again, and copy the diagnostics from Settings if it keeps happening.";
/// The Game Mode Menu reads at most this many modes (MAX_MODES in gamemodes.sma).
const MAX_MODES: usize = 128;
/// Graph files bigger than this aren't graphs anyone drew.
// (room for a graph at graph::MAX_NODES / MAX_EDGES)
const MAX_GRAPH_BYTES: u64 = 8 << 20;

/// The includes a generated plugin is built against (loner.inc and what it pulls in), written
/// next to the plugin's source before each compile so they always match this app.
const INCLUDES: &[(&str, &str)] = &[
    ("loner.inc", include_str!("../../../../../plugins/scripting/include/loner.inc")),
    ("ls_components.inc", include_str!("../../../../../plugins/scripting/include/ls_components.inc")),
];

fn write_includes(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, text) in INCLUDES {
        crate::sys::write_atomic(&dir.join(name), text.as_bytes())?;
    }
    Ok(())
}

/// One compile at a time (they write the same files).
static COMPILING: Mutex<()> = Mutex::new(());

pub fn graphs_dir() -> PathBuf {
    crate::sys::app_dir().join("tools").join("studio").join("graphs")
}

/// The player's own snippets (pieces of graph): `<app>/tools/studio/snippets/<id>.snippet.json`.
pub fn snippets_dir() -> PathBuf {
    crate::sys::app_dir().join("tools").join("studio").join("snippets")
}

pub const SNIPPET_SCHEMA: &str = "loner-strike/graph-snippet";

/// A snippet checked like a graph (same shapes, ids, node kinds, settings and text rules), its
/// name and description included. Graph data only: it never carries code.
/// A snippet's or library item's `fromMode`: the uid of the mode it was taken from (optional).
fn from_mode_ok(v: &Value) -> Result<(), String> {
    match v.get("fromMode") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(u)) if graph::valid_uid(u) => Ok(()),
        _ => Err("That part says it came from a mode, but not with a uid this app knows.".into()),
    }
}

pub fn check_snippet(v: &Value) -> Result<(), String> {
    if v.get("schema").and_then(Value::as_str) != Some(SNIPPET_SCHEMA) {
        return Err("That isn't a Game Mode Studio snippet.".into());
    }
    let name = v.get("name").and_then(Value::as_str).unwrap_or("");
    let desc = v.get("description").and_then(Value::as_str).unwrap_or("");
    if name.trim().is_empty() || !graph::valid_label(name, 40) || !graph::valid_label(desc, 160) {
        return Err("A snippet needs a name (up to 40 letters, digits, spaces or plain punctuation) and a short description.".into());
    }
    from_mode_ok(v)?;
    let mut g = v.clone();
    if let Some(o) = g.as_object_mut() {
        o.insert("schema".into(), json!(graph::SCHEMA));
        o.insert("version".into(), json!(graph::VERSION));
        o.insert("mode".into(), json!({"id": "snippet", "name": "Snippet"}));
    }
    let g = graph::from_value(g)?;
    // (a piece of a graph may leave pins unwired or nodes unrun: only its own text and shape count)
    let bad: Vec<String> = graph::validate(&g)
        .into_iter()
        .filter(|p| p.level == "error" && !p.message.contains("needs something wired") && !p.message.contains("only known") && !p.message.contains("Pick"))
        .map(|p| p.message)
        .collect();
    match bad.first() {
        Some(m) => Err(format!("That snippet has a problem: {m}")),
        None => Ok(()),
    }
}

/// The player's library of reusable parts (a Function, Macro, struct or enum with everything it
/// needs): `<app>/tools/studio/library/<id>.lib.json`.
pub fn library_dir() -> PathBuf {
    crate::sys::app_dir().join("tools").join("studio").join("library")
}

pub const LIBRARY_SCHEMA: &str = "loner-strike/graph-library";
const LIBRARY_KINDS: [&str; 4] = ["function", "macro", "struct", "enum"];

/// A library item checked like a graph (graph data only, the same rules), its name,
/// description and category included. Its nodes are all on its Functions' and Macros' graphs
/// (an item never brings events), and its root is there.
pub fn check_library_item(v: &Value) -> Result<(), String> {
    if v.get("schema").and_then(Value::as_str) != Some(LIBRARY_SCHEMA) || v.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("That isn't a Game Mode Studio library item (or it's from a newer app).".into());
    }
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("");
    if s("name").trim().is_empty() || !graph::valid_label(s("name"), 40) || !graph::valid_label(s("description"), 160) || !graph::valid_label(s("category"), 30) {
        return Err("A library item needs a name (up to 40 letters, digits, spaces or plain punctuation), a short description and a category.".into());
    }
    from_mode_ok(v)?;
    let kind = s("kind");
    if !LIBRARY_KINDS.contains(&kind) {
        return Err("A library item is a Function, a Macro, a struct or an enum.".into());
    }
    let mut g = v.clone();
    let root = s("root").to_string();
    if let Some(o) = g.as_object_mut() {
        for k in ["name", "description", "category", "kind", "root", "itemVersion", "fromMode"] {
            o.remove(k);
        }
        o.insert("schema".into(), json!(graph::SCHEMA));
        o.insert("version".into(), json!(graph::VERSION));
        o.insert("mode".into(), json!({"id": "library", "name": "Library"}));
    }
    let g = graph::from_value(g)?;
    let has_root = match kind {
        "function" => g.functions.iter().any(|f| f.id == root),
        "macro" => g.macros.iter().any(|m| m.id == root),
        "struct" => g.structs.iter().any(|x| x.id == root),
        _ => g.enums.iter().any(|x| x.id == root),
    };
    if !has_root {
        return Err("That library item doesn't hold the part it's named after.".into());
    }
    if g.nodes.iter().any(|n| n.graph.is_none()) || !g.comments.iter().all(|c| c.graph.is_some()) {
        return Err("A library item only holds Functions, Macros, structs and enums (no events).".into());
    }
    let bad: Vec<String> = graph::validate(&g).into_iter().filter(|p| p.level == "error").map(|p| p.message).collect();
    match bad.first() {
        Some(m) => Err(format!("That library item has a problem: {m}")),
        None => Ok(()),
    }
}

/// The item without its version (to tell a real change from saving the same again).
fn library_body(v: &Value) -> Value {
    let mut b = v.clone();
    if let Some(o) = b.as_object_mut() {
        o.remove("itemVersion");
    }
    b
}

fn graph_path(id: &str) -> PathBuf {
    graphs_dir().join(format!("{id}.graph.json"))
}

// ---- graphs on disk ------------------------------------------------------------------------

fn list_graphs(dir: &Path, game_dir: Option<&Path>) -> Vec<Value> {
    let mut out: Vec<(u64, Value)> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let id = name.strip_suffix(".graph.json")?.to_string();
                    if !graph::valid_mode_id(&id) || e.metadata().ok()?.len() > MAX_GRAPH_BYTES {
                        return None;
                    }
                    let modified = e.metadata().ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
                    let g = std::fs::read_to_string(e.path()).ok().and_then(|t| graph::parse(&t).ok());
                    let installed = game_dir.is_some_and(|gd| installed(gd, &id));
                    Some((modified, match g {
                        Some(g) => json!({"id": id, "name": g.mode.name, "description": g.mode.description, "nodes": g.nodes.len(), "modified": modified, "installed": installed}),
                        None => json!({"id": id, "name": id, "damaged": true, "modified": modified, "installed": installed}),
                    }))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, v)| v).collect()
}

fn read_graph(dir: &Path, id: &str) -> Result<Graph, String> {
    if !graph::valid_mode_id(id) {
        return Err(BAD_ID.into());
    }
    let p = dir.join(format!("{id}.graph.json"));
    match std::fs::metadata(&p) {
        Err(_) => return Err("That graph isn't there any more.".into()),
        Ok(m) if m.len() > MAX_GRAPH_BYTES => return Err("That graph file is too big to be a graph.".into()),
        _ => {}
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("Couldn't read that graph ({e})."))?;
    graph::parse(&text)
}

fn save_graph(dir: &Path, id: &str, g: &Graph) -> Result<(), String> {
    if !graph::valid_mode_id(id) || g.mode.id != id {
        return Err(BAD_ID.into());
    }
    if let Some(e) = graph::size_error(g) {
        return Err(e);
    }
    let text = serde_json::to_string_pretty(g).map_err(|_| INTERNAL.to_string())?;
    if text.len() as u64 > MAX_GRAPH_BYTES {
        return Err("That graph is too big to save.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the graph ({e})."))?;
    let path = dir.join(format!("{id}.graph.json"));
    // (graphs live in the app's own folder; the safety net keeps a copy where it looks after files)
    crate::safety::before("Game Mode Studio", "save a graph", &path);
    crate::sys::write_atomic(&path, text.as_bytes()).map_err(|e| format!("Couldn't save the graph ({e})."))?;
    // the version history (the History panel): only what changed since the last save
    if let Ok(v) = serde_json::to_value(g) {
        let _ = history::record(dir, id, &v);
    }
    // its mode map kit beside it (made again from the graph each save)
    if !graph::has_errors(&graph::validate(g)) {
        let kit = serde_json::to_string_pretty(&kit::kit_of(g)).map_err(|_| INTERNAL.to_string())?;
        let _ = crate::sys::write_atomic(&dir.join(format!("{id}.kit.json")), kit.as_bytes());
    }
    Ok(())
}

// ---- compiling -----------------------------------------------------------------------------

/// One compiler message, mapped back to a node when it came from one.
#[derive(Debug, PartialEq)]
pub struct Diag {
    pub line: usize,
    pub level: &'static str,
    pub code: u32,
    pub message: String,
}

/// amxxpc's `file.sma(12) : error 017: undefined symbol "x"` lines (also `(12 -- 14)` and
/// `fatal error`), for our own source file only.
pub fn parse_compiler(output: &str, file: &str) -> Vec<Diag> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^(.*?)\((\d+)(?: -- (\d+))?\) : (fatal error|error|warning) (\d+): (.*)$").unwrap());
    output
        .lines()
        .filter_map(|l| {
            let c = re.captures(l.trim_end())?;
            let path = c.get(1)?.as_str();
            if !path.replace('\\', "/").rsplit('/').next().unwrap_or(path).eq_ignore_ascii_case(file) {
                return None;
            }
            let line = c.get(3).or(c.get(2))?.as_str().parse().ok()?;
            let level = if c.get(4)?.as_str() == "warning" { "warning" } else { "error" };
            Some(Diag { line, level, code: c.get(5)?.as_str().parse().ok()?, message: c.get(6)?.as_str().trim().to_string() })
        })
        .collect()
}

/// Turns compiler messages into problems on nodes (or on the graph, when a line belongs to the
/// shared code).
fn diag_problems(diags: &[Diag], out: &gen::Generated) -> Vec<Problem> {
    diags
        .iter()
        .map(|d| Problem {
            level: d.level,
            node: out.node_at(d.line).map(str::to_string),
            pin: None,
            edge: None,
            message: if out.node_at(d.line).is_some() {
                format!("The compiler said (line {}): {}", d.line, d.message)
            } else {
                format!("The compiler said (line {} of the plugin, not tied to a node; Show code in the menu shows that line): {}", d.line, d.message)
            },
        })
        .collect()
}

fn local_game(st: &Shared) -> Result<std::sync::Arc<crate::game::Game>, String> {
    let g = st.local_game().ok_or(NO_GAME)?;
    if g.dedicated || g.remote.is_some() {
        return Err(NOT_HERE.into());
    }
    if crate::backup::restoring_game(&g.hl) {
        return Err(crate::backup::RESTORING_GAME.into());
    }
    Ok(g)
}

/// How to run the compiler for this game: the player's own install (the app), or, for another
/// folder (a sim copy under the dev server, a hidden server), from where it is: inside a CrossOver
/// bottle it runs through that bottle, on Windows as it is.
pub fn compiler_install(st: &Shared, game_dir: &Path) -> Option<crate::steam::Install> {
    if let Some(i) = crate::servers::client(st) {
        return Some(i);
    }
    let text = game_dir.to_string_lossy().replace('\\', "/");
    if let Some(rest) = text.split("/CrossOver/Bottles/").nth(1) {
        let bottle = rest.split('/').next().filter(|b| !b.is_empty())?;
        return Some(crate::steam::Install { hl: game_dir.parent()?.to_path_buf(), kind: "crossover".into(), bottle: Some(bottle.into()), games: vec![], dedicated: false });
    }
    cfg!(windows).then(|| crate::steam::Install { hl: game_dir.parent().map(Path::to_path_buf).unwrap_or_default(), kind: "windows".into(), bottle: None, games: vec![], dedicated: false })
}

/// Runs amxxpc on a source file; Ok((compiled, output)).
pub fn run_amxxpc(install: &crate::steam::Install, scripting: &Path, src: &Path, inc: &Path, out: &Path) -> Result<(bool, String), String> {
    let exe = scripting.join("amxxpc.exe");
    if !exe.is_file() {
        return Err("The game has no AMX Mod X compiler. Run setup again (Settings), then compile.".into());
    }
    if install.kind == "crossover" && !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The compiler runs through it, like the game.".into());
    }
    let _ = std::fs::remove_file(out);
    let run = || {
        let mut cmd = crate::setup::core::windows_command(install, &exe);
        cmd.arg(src).arg(format!("-o{}", out.display())).arg(format!("-i{}", inc.display())).current_dir(scripting);
        crate::sys::run_output(cmd, Duration::from_secs(120))
    };
    let mut ran = run();
    // a compiler that crashed under wine (no "Done." and no errors of its own) gets one more go
    let crashed = |r: &Option<(bool, String, String)>| r.as_ref().map_or(true, |(_, o, e)| {
        let all = format!("{o}{e}");
        !all.contains("Done.") && !all.to_lowercase().contains("error")
    });
    if crashed(&ran) {
        ran = run();
    }
    let (_, o, e) = ran.ok_or("The compiler didn't run, or took too long.")?;
    let all: String = format!("{o}{e}").lines().filter(|l| !wine_noise(l)).collect::<Vec<_>>().join("\n");
    Ok((all.contains("Done.") && out.is_file(), all))
}

/// A line Wine or CrossOver printed while the compiler ran (as setup/core.rs filters them);
/// the compiler's own messages always have " : " in them.
fn wine_noise(line: &str) -> bool {
    let l = line.trim();
    if l.contains(") : ") {
        return false;
    }
    l.is_empty() || ["fixme:", "err:", "warn:", "[mvk-", "MoltenVK", "Vulkan", "macOS GPU Family"].iter().any(|n| l.contains(n)) || l.starts_with("VK_") || l.starts_with("wine:")
}

/// Writes the mode's files and its modes.ini line. Refuses an id a mode not made here uses.
pub fn register_mode(cfg: &Path, g: &Graph, plugin: &str) -> Result<(), String> {
    // (cfg is <game>/addons/amxmodx/configs)
    if let Some(game) = cfg.ancestors().nth(3) {
        guard_real_game(game)?;
    }
    let id = &g.mode.id;
    let gm = cfg.join("gamemodes");
    let ini_path = gm.join("modes.ini");
    let ini = crate::sys::read_latin1(&ini_path).map_err(|_| "The Game Mode Menu isn't set up in this game (no modes.ini). Run setup again first.".to_string())?;
    let ours = |gm: &Path| crate::sys::read_latin1(&gm.join(format!("{id}.plugins.ini"))).is_ok_and(|t| t.lines().any(|l| l.trim() == format!("{plugin}.amxx")));
    let modes = crate::extras::parse_modes(&ini);
    let listed = modes.iter().any(|(m, _)| m == id);
    if listed && !ours(&gm) {
        return Err(format!("The game already has a mode called \u{201c}{id}\u{201d} that wasn't made in the Studio. Give this mode another id."));
    }
    if !listed && modes.len() >= MAX_MODES {
        return Err(format!("The Game Mode Menu holds {MAX_MODES} modes and this game has that many. Remove one first."));
    }
    let nl = if ini.contains("\r\n") { "\r\n" } else { "\n" };
    // (mode settings: the Game Mode Menu's "options" opens their admin menu)
    let options = if graph::mode_settings(g).is_empty() { String::new() } else { graph::settings_menu_cmd(id) };
    let line = format!("{id}  \"{}\"  \"{}\"  \"{options}\"  \"\"", g.mode.name, g.mode.description);
    let mut lines: Vec<String> = ini.lines().map(|l| l.trim_end_matches('\r').to_string()).collect();
    match lines.iter().position(|l| l.split_whitespace().next() == Some(id.as_str())) {
        Some(i) => lines[i] = line,
        None => lines.push(line),
    }
    let text = lines.join(nl) + nl;
    let write = |p: PathBuf, t: &str| {
        crate::safety::before("Game Mode Studio", "install a mode", &p);
        crate::sys::write_atomic(&p, t.as_bytes()).map_err(|e| format!("Couldn't install the mode ({e})."))
    };
    // gm_components first when a node needs it (as the shipped modes list it)
    let comps = if gen::needs(g).components { format!("gm_components.amxx{nl}") } else { String::new() };
    // (NPC Move To / Follow Path walk the bot navigation mesh's routes: gm_botnav finds them)
    let comps = if gen::coverage::needs_botnav(g) { format!("{comps}gm_botnav.amxx{nl}") } else { comps };
    let uid = if g.mode.uid.is_empty() { String::new() } else { format!(" (uid {})", g.mode.uid) };
    write(gm.join(format!("{id}.plugins.ini")), &format!("; Made with the Game Mode Studio{uid}: compile the graph again instead of editing this.{nl}{comps}{plugin}.amxx{nl}"))?;
    // the mode settings hosts can change: what they are (the app reads it for the Play and server
    // pages and MCP); the values are the server's own (data/studio_settings/<mode>.cfg)
    let manifest = settings::manifest(g);
    let mpath = gm.join(format!("{id}.settings.json"));
    if manifest["settings"].as_array().is_some_and(|a| !a.is_empty()) {
        write(mpath, &serde_json::to_string_pretty(&manifest).unwrap_or_default())?;
    } else {
        let _ = std::fs::remove_file(&mpath);
    }
    // the mode's settings (the graph's own; written every compile, after base.cfg each map)
    let mut cfg_text = format!("// {} (Game Mode Studio): its settings, written from the graph each compile. Runs after base.cfg.{nl}", gen::comment_text(&g.mode.name));
    let allowed = graph::allowed_settings();
    for st in g.mode.settings.iter().filter(|s| allowed.contains(&s.cvar.as_str()) && s.value.is_finite()) {
        let v = st.value.clamp(0.0, graph::NUMBER_LIMIT);
        let v = if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") };
        cfg_text.push_str(&format!("{} \"{v}\"{nl}", st.cvar));
    }
    write(gm.join(format!("{id}.cfg")), &cfg_text)?;
    write(ini_path, &text)?;
    // the mode's map kit, for the map editor (a kit that can't be kept doesn't stop the install;
    // tests don't write into the app's folder)
    #[cfg(not(test))]
    if let Err(e) = kit::install_kit(&kit::kit_of(g)) {
        eprintln!("Game Mode Studio: {e}");
    }
    Ok(())
}

/// The uid an installed Studio mode was made with (from its plugin list's first line).
fn installed_uid(gm: &Path, id: &str) -> Option<String> {
    let t = crate::sys::read_latin1(&gm.join(format!("{id}.plugins.ini"))).ok()?;
    let first = t.lines().next()?;
    let at = first.find("(uid ")? + 5;
    let u = first.get(at..at + 36)?;
    graph::valid_uid(u).then(|| u.to_string())
}

/// The short id a graph installs under in this game: the one its uid is installed as already;
/// else its own, unless another mode (one not made in the Studio, or another uid) has it; then
/// the first free `<id>_2`, `<id>_3`...
pub fn install_id(game_dir: &Path, g: &Graph) -> String {
    let gm = game_dir.join("addons/amxmodx/configs/gamemodes");
    let modes: Vec<String> = crate::extras::parse_modes(&crate::sys::read_latin1(&gm.join("modes.ini")).unwrap_or_default()).into_iter().map(|(m, _)| m).collect();
    let id = &g.mode.id;
    if g.mode.uid.is_empty() {
        return id.clone();
    }
    if let Some(m) = modes.iter().find(|m| installed(game_dir, m) && installed_uid(&gm, m).as_deref() == Some(g.mode.uid.as_str())) {
        return m.clone();
    }
    // free, or our own install from before uids
    let mine = |m: &str| installed(game_dir, m) && installed_uid(&gm, m).is_none();
    if !modes.contains(id) || mine(id) {
        return id.clone();
    }
    for n in 2..1000 {
        let suffix = format!("_{n}");
        let cand = format!("{}{suffix}", &id[..id.len().min(24 - suffix.len())]);
        if !modes.contains(&cand) {
            return cand;
        }
    }
    id.clone()
}

/// Under test (unit tests, and the Tools dev server the browser tests use, which is a test
/// binary), the Studio never writes into a real Half-Life install (the usual Steam places):
/// installs, compiles, uninstalls, Open as code and switching the game go to a throwaway game
/// folder or a sim copy (LS_TEST_GAME) instead. Outside tests it lets everything through.
#[cfg(test)]
pub fn guard_real_game(game_dir: &Path) -> Result<(), String> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let real = [
        home.join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life"),
        home.join("Library/Application Support/Steam/steamapps/common/Half-Life"),
        home.join(".steam/steam/steamapps/common/Half-Life"),
        home.join(".local/share/Steam/steamapps/common/Half-Life"),
        PathBuf::from("C:/Program Files (x86)/Steam/steamapps/common/Half-Life"),
    ];
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let me = canon(game_dir);
    // (the Half-Life folder or anything in it: game.dir is its mod folder, cstrike or czero)
    if real.iter().any(|r| me.starts_with(canon(r))) {
        eprintln!("studio: refused to write into the real game under test ({})", game_dir.display());
        return Err("Under test the Game Mode Studio doesn't install into your real game. Point LS_TEST_GAME at a copy (a sim server) for that.".into());
    }
    Ok(())
}

#[cfg(not(test))]
pub fn guard_real_game(_game_dir: &Path) -> Result<(), String> {
    Ok(())
}

/// Is a Studio mode installed in this game (its line, its plugin list naming our plugin)?
// (pub: the community library builds and removes community Studio modes with these)
pub fn installed(game_dir: &Path, id: &str) -> bool {
    let gm = game_dir.join("addons/amxmodx/configs/gamemodes");
    let listed = crate::extras::parse_modes(&crate::sys::read_latin1(&gm.join("modes.ini")).unwrap_or_default()).iter().any(|(m, _)| m == id);
    listed && crate::sys::read_latin1(&gm.join(format!("{id}.plugins.ini"))).is_ok_and(|t| t.contains(&format!("{}.amxx", gen::plugin_name(id))))
}

/// Removes a Studio mode from the game (never one it didn't make).
pub fn uninstall(game_dir: &Path, id: &str) -> Result<(), String> {
    guard_real_game(game_dir)?;
    if !installed(game_dir, id) {
        return Err("That mode isn't installed (or wasn't made in the Studio).".into());
    }
    let am = game_dir.join("addons/amxmodx");
    let gm = am.join("configs/gamemodes");
    let ini = crate::sys::read_latin1(&gm.join("modes.ini")).map_err(|e| format!("Couldn't remove the mode ({e})."))?;
    let nl = if ini.contains("\r\n") { "\r\n" } else { "\n" };
    let kept: Vec<&str> = ini.lines().map(|l| l.trim_end_matches('\r')).filter(|l| l.split_whitespace().next() != Some(id)).collect();
    crate::safety::before("Game Mode Studio", "remove a mode", &gm.join("modes.ini"));
    crate::sys::write_atomic(&gm.join("modes.ini"), (kept.join(nl) + nl).as_bytes()).map_err(|e| format!("Couldn't remove the mode ({e})."))?;
    let plugin = gen::plugin_name(id);
    // (with its Mode settings: what they are and this server's values for them)
    for p in [gm.join(format!("{id}.plugins.ini")), gm.join(format!("{id}.cfg")), am.join("plugins").join(format!("{plugin}.amxx")), game_dir.join(settings::manifest_rel(id)), game_dir.join(settings::values_rel(id))] {
        if p.is_file() {
            crate::safety::before("Game Mode Studio", "remove a mode", &p);
        }
        let _ = std::fs::remove_file(p);
    }
    // what compiling and the debugger left (made again from the graph: no backup needed)
    let work = am.join("scripting/studio");
    for p in [
        am.join("scripting").join(format!("{plugin}.sma")),
        work.join(format!("{plugin}.amxx")),
        work.join(format!("{plugin}.hash")),
        debug::bp_file(game_dir, id),
        debug::cmd_file(game_dir, id),
        debug::events_file(game_dir, id),
    ] {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

/// Checks, generates, compiles and installs. `{ok, problems, output, plugin}`; ok is false
/// (with problems on the nodes) when the graph or the compiler said no.
pub fn compile(st: &Shared, mut g: Graph, debug: bool) -> Result<Value, String> {
    let mut problems = graph::validate(&g);
    if graph::has_errors(&problems) {
        return Ok(json!({"ok": false, "problems": problems}));
    }
    let game = local_game(st)?;
    guard_real_game(&game.dir)?;
    // installed under the short id this game has for its uid (another mode may have its own)
    let installed_as = install_id(&game.dir, &g);
    g.mode.id = installed_as.clone();
    // (a mode new to the game: the open game's menu reads modes.ini again once it's in)
    let new_here = !installed(&game.dir, &installed_as);
    let reload = |mut v: Value| {
        if new_here {
            if let (Value::Object(o), Value::Object(r)) = (&mut v, modes_reload::for_local_game(&game).json()) {
                o.extend(r);
            }
        }
        v
    };
    let install = compiler_install(st, &game.dir).ok_or(NO_GAME)?;
    let _one = COMPILING.lock().unwrap_or_else(|e| e.into_inner());
    let out = gen::generate(&g, debug);
    let plugin = gen::plugin_name(&g.mode.id);
    let am = game.dir.join("addons/amxmodx");
    let scripting = am.join("scripting");
    let work = scripting.join("studio");
    let inc = work.join("include");
    write_includes(&inc).map_err(|e| format!("Couldn't write the plugin ({e})."))?;
    let src = scripting.join(format!("{plugin}.sma"));
    crate::safety::before("Game Mode Studio", "install a mode", &src);
    crate::sys::write_atomic(&src, out.source.as_bytes()).map_err(|e| format!("Couldn't write the plugin ({e})."))?;
    let built = work.join(format!("{plugin}.amxx"));
    let dest = am.join("plugins").join(format!("{plugin}.amxx"));
    // an unchanged graph isn't compiled again (amxxpc under CrossOver takes seconds): the same
    // source and includes as the plugin that's installed
    let started = std::time::Instant::now();
    let stamp_path = work.join(format!("{plugin}.hash"));
    let stamp = source_hash(&out.source);
    if dest.is_file() && built.is_file() && std::fs::read_to_string(&stamp_path).is_ok_and(|s| s.trim() == stamp) {
        register_mode(&am.join("configs"), &g, &plugin)?;
        return Ok(reload(json!({"ok": true, "problems": problems, "output": "Unchanged since the last compile: the installed plugin is used as it is.", "plugin": plugin, "installedAs": installed_as, "debug": debug, "cached": true, "secs": 0.0})));
    }
    let _ = std::fs::remove_file(&stamp_path);
    let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &built)?;
    let diags = parse_compiler(&output, &format!("{plugin}.sma"));
    problems.extend(diag_problems(&diags, &out));
    if !ok {
        if diags.iter().all(|d| d.level == "warning") {
            crate::diagnostics::log_error("studio: compile", &output.chars().rev().take(3000).collect::<String>().chars().rev().collect::<String>());
            problems.push(Problem { level: "error", node: None, pin: None, edge: None, message: "The compiler stopped without saying why. Try again; Copy diagnostics in Settings has its output.".into() });
        }
        return Ok(json!({"ok": false, "problems": problems, "output": output}));
    }
    crate::safety::before("Game Mode Studio", "install a mode", &dest);
    std::fs::copy(&built, &dest).map_err(|e| format!("Couldn't install the plugin ({e})."))?;
    let _ = std::fs::write(&stamp_path, &stamp);
    register_mode(&am.join("configs"), &g, &plugin)?;
    Ok(reload(json!({"ok": true, "problems": problems, "output": output, "plugin": plugin, "installedAs": installed_as, "debug": debug, "secs": (started.elapsed().as_secs_f64() * 10.0).round() / 10.0})))
}

/// Compile (the full check): the real amxxpc on the mode's plugin, without installing it or
/// starting anything. Built in `scripting/studio/check/` (never `plugins/` or the configs), so an
/// installed mode keeps running as it was. `compiler` is what the compiler found, each tied to the
/// node whose code it is on (or `node: null` with the line when it can't be); `problems` are the
/// quick check's as well. An unchanged plugin isn't compiled again (the last answer is kept).
pub fn check_compile(st: &Shared, mut g: Graph) -> Result<Value, String> {
    let problems = graph::validate(&g);
    if graph::has_errors(&problems) {
        return Ok(json!({"ok": false, "problems": problems, "compiler": [], "stopped": true}));
    }
    let game = local_game(st)?;
    guard_real_game(&game.dir)?;
    g.mode.id = install_id(&game.dir, &g);
    let install = compiler_install(st, &game.dir).ok_or(NO_GAME)?;
    let out = gen::generate(&g, false);
    let plugin = gen::plugin_name(&g.mode.id);
    let scripting = game.dir.join("addons/amxmodx/scripting");
    let work = scripting.join("studio");
    let check = work.join("check");
    let inc = work.join("include");
    let started = std::time::Instant::now();
    let stamp = source_hash(&out.source);
    let last = check.join(format!("{plugin}.json"));
    if let Some(v) = std::fs::read_to_string(&last).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(|v| v["hash"] == stamp.as_str()) {
        return Ok(json!({"ok": v["ok"], "problems": problems, "compiler": v["compiler"], "output": v["output"], "cached": true, "secs": 0.0}));
    }
    let _one = COMPILING.lock().unwrap_or_else(|e| e.into_inner());
    std::fs::create_dir_all(&check).map_err(|e| format!("Couldn't write the plugin ({e})."))?;
    write_includes(&inc).map_err(|e| format!("Couldn't write the plugin ({e})."))?;
    let src = check.join(format!("{plugin}.sma"));
    crate::sys::write_atomic(&src, out.source.as_bytes()).map_err(|e| format!("Couldn't write the plugin ({e})."))?;
    let built = check.join(format!("{plugin}.amxx"));
    let (ran, output) = run_amxxpc(&install, &scripting, &src, &inc, &built)?;
    let _ = std::fs::remove_file(&built);
    let diags = parse_compiler(&output, &format!("{plugin}.sma"));
    let mut compiler = diag_problems(&diags, &out);
    let ok = ran && !diags.iter().any(|d| d.level == "error");
    if !ran && diags.iter().all(|d| d.level == "warning") {
        compiler.push(Problem { level: "error", node: None, pin: None, edge: None, message: "The compiler stopped without saying why. Try again; Copy diagnostics in Settings has its output.".into() });
    }
    let _ = std::fs::write(&last, json!({"hash": stamp, "ok": ok, "compiler": compiler, "output": output}).to_string());
    Ok(json!({"ok": ok, "problems": problems, "compiler": compiler, "output": output, "secs": (started.elapsed().as_secs_f64() * 10.0).round() / 10.0}))
}

/// What decides whether a compile can be skipped: the source and the includes it's built with.
fn source_hash(source: &str) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(source.as_bytes());
    for (name, text) in INCLUDES {
        h.update(name.as_bytes());
        h.update(text.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Playtest on a game that's already open: the open game's listen server goes to the mode on
/// that map (the mode and its plugins set as a start does, then a map change over the console),
/// instead of closing and starting the game again.
fn switch_open_game(st: &Shared, mode: &str, map: &str) -> Result<Value, String> {
    let g = local_game(st)?;
    guard_real_game(&g.dir)?;
    if !installed(&g.dir, mode) {
        return Err("That mode isn't installed yet.".into());
    }
    if !crate::maps::safe_arg(map) {
        return Err("That isn't a map name.".into());
    }
    let cfg = g.dir.join("addons/amxmodx/configs");
    let list = std::fs::read(cfg.join(format!("gamemodes/{mode}.plugins.ini"))).unwrap_or_default();
    let mut text = format!("; Generated by gamemodes.amxx - edit configs/gamemodes/{mode}.plugins.ini instead\n").into_bytes();
    text.extend(list);
    crate::sys::write_atomic(&cfg.join("plugins-gamemode.ini"), &text).map_err(|e| format!("Couldn't switch the mode ({e})."))?;
    crate::sys::write_atomic(&g.data.join("current.txt"), format!("{mode}\n").as_bytes()).map_err(|e| format!("Couldn't switch the mode ({e})."))?;
    match g.command("gm_map", &[map.to_string()]) {
        Ok(_) => Ok(json!({"ok": true})),
        Err(crate::game::CmdError::Bad(e)) | Err(crate::game::CmdError::Game(e)) => Err(format!("The open game didn't take the change ({e}). Restart it instead.")),
    }
}

// ---- routes --------------------------------------------------------------------------------

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => Json(json!({"error": e})).into_response(),
        Err(_) => Json(json!({"error": INTERNAL})).into_response(),
    }
}

fn forbidden() -> Response {
    (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

/// A graph sent by the page: checked against the schema here, so the page gets readable words.
fn graph_of(v: Value) -> Result<Graph, String> {
    graph::from_value(v)
}

async fn api_status(State(st): State<Shared>) -> Response {
    blocking(move || {
        let game = local_game(&st).ok();
        let compiler = game.as_ref().is_some_and(|g| g.dir.join("addons/amxmodx/scripting/amxxpc.exe").is_file());
        Ok(json!({"game": game.is_some(), "compiler": compiler, "graphsDir": graphs_dir().display().to_string()}))
    })
    .await
}

async fn api_graphs(State(st): State<Shared>) -> Response {
    blocking(move || {
        let game = local_game(&st).ok();
        Ok(json!({"graphs": list_graphs(&graphs_dir(), game.as_ref().map(|g| g.dir.as_path()))}))
    })
    .await
}

async fn api_graph(UrlPath(id): UrlPath<String>) -> Response {
    blocking(move || {
        let mut g = read_graph(&graphs_dir(), &id)?;
        // a graph from before uids gets its own the first time it's opened
        if g.mode.uid.is_empty() {
            g.mode.uid = graph::new_uid();
            save_graph(&graphs_dir(), &id, &g)?;
        }
        Ok(json!({"graph": g}))
    })
    .await
}

async fn api_save(UrlPath(id): UrlPath<String>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let mut g = graph_of(body.get("graph").cloned().unwrap_or(Value::Null))?;
        // (a mode keeps its uid: the saved one, else a new one)
        if g.mode.uid.is_empty() {
            g.mode.uid = read_graph(&graphs_dir(), &id).ok().map(|x| x.mode.uid).filter(|u| !u.is_empty()).unwrap_or_else(graph::new_uid);
        }
        save_graph(&graphs_dir(), &id, &g)?;
        Ok(json!({"ok": true}))
    })
    .await
}

/// The kits of the modes this computer has (`<app dir>/kits`).
async fn api_kits() -> Response {
    blocking(move || Ok(json!({"kits": kit::list_kits_in(&kit::kits_dir())}))).await
}

/// One mode's kit: a Studio graph's made from it now, else the kits folder's.
async fn api_kit(UrlPath(id): UrlPath<String>) -> Response {
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err(BAD_ID.into());
        }
        if let Ok(g) = read_graph(&graphs_dir(), &id) {
            return Ok(json!({"kit": kit::kit_of(&g)}));
        }
        kit::list_kits_in(&kit::kits_dir()).into_iter().find(|k| k["mode"]["id"] == id.as_str()).map(|k| json!({"kit": k})).ok_or_else(|| "There's no map kit for that mode.".to_string())
    })
    .await
}

/// The mode's kept saves, newest first, with what changed in each (the History panel).
async fn api_history(UrlPath(id): UrlPath<String>) -> Response {
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err(BAD_ID.into());
        }
        Ok(history::list(&graphs_dir(), &id))
    })
    .await
}

/// One kept save, whole.
async fn api_history_version(UrlPath((id, n)): UrlPath<(String, usize)>) -> Response {
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err(BAD_ID.into());
        }
        history::get(&graphs_dir(), &id, n).map(|g| json!({"graph": g})).ok_or_else(|| "That version isn't kept any more.".to_string())
    })
    .await
}

async fn api_delete(UrlPath(id): UrlPath<String>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err(BAD_ID.into());
        }
        crate::safety::before("Game Mode Studio", "delete a graph", &graph_path(&id));
        let _ = std::fs::remove_file(graphs_dir().join(format!("{id}.kit.json")));
        history::delete(&graphs_dir(), &id);
        match std::fs::remove_file(graph_path(&id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Couldn't delete the graph ({e}).")),
            _ => Ok(json!({"ok": true})),
        }
    })
    .await
}

async fn api_validate(h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let g = graph_of(body.get("graph").cloned().unwrap_or(Value::Null))?;
        Ok(json!({"problems": graph::validate(&g)}))
    })
    .await
}

#[derive(Deserialize)]
struct BuildReq {
    graph: Value,
    #[serde(default)]
    debug: bool,
}

async fn api_generate(h: HeaderMap, Json(req): Json<BuildReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let g = graph_of(req.graph)?;
        let problems = graph::validate(&g);
        if graph::has_errors(&problems) {
            return Ok(json!({"problems": problems}));
        }
        let out = gen::generate(&g, req.debug);
        Ok(json!({"problems": problems, "source": out.source, "spans": out.spans, "plugin": gen::plugin_name(&g.mode.id)}))
    })
    .await
}

async fn api_testrun(State(st): State<Shared>, h: HeaderMap, Json(req): Json<BuildReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || testrun::start(&st, graph_of(req.graph)?)).await
}

async fn api_testrun_progress() -> Response {
    Json(testrun::progress()).into_response()
}

async fn api_testrun_cancel(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    Json(testrun::cancel()).into_response()
}

async fn api_check(State(st): State<Shared>, h: HeaderMap, Json(req): Json<BuildReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || check_compile(&st, graph_of(req.graph)?)).await
}

async fn api_compile(State(st): State<Shared>, h: HeaderMap, Json(req): Json<BuildReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || compile(&st, graph_of(req.graph)?, req.debug)).await
}

/// "Open as code": writes the graph's plugin source (a release build) where the Plugin Studio
/// opens it read only; the page then goes to `plugins:game/studio_<id>.sma`.
fn write_source(st: &Shared, g: Graph) -> Result<Value, String> {
    let problems = graph::validate(&g);
    if graph::has_errors(&problems) {
        return Ok(json!({"problems": problems}));
    }
    let game = local_game(st)?;
    guard_real_game(&game.dir)?;
    let scripting = game.dir.join("addons/amxmodx/scripting");
    if !scripting.is_dir() {
        return Err("AMX Mod X isn't installed in your game yet. Install the basics first.".into());
    }
    let name = format!("{}.sma", gen::plugin_name(&g.mode.id));
    let path = scripting.join(&name);
    crate::safety::before("Game Mode Studio", "open as code", &path);
    crate::sys::write_atomic(&path, gen::generate(&g, false).source.as_bytes()).map_err(|e| format!("Couldn't write the code ({e})."))?;
    Ok(json!({"ok": true, "folder": "game", "name": name}))
}

async fn api_source(State(st): State<Shared>, h: HeaderMap, Json(req): Json<BuildReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || write_source(&st, graph_of(req.graph)?)).await
}

#[derive(Deserialize)]
struct PlaytestReq {
    graph: Value,
    #[serde(default)]
    breakpoints: Vec<String>,
}

/// Compiles a debug build, installs it and readies the debug link; the page then starts the
/// game in the mode (`/api/app/launch`, as the Play page does).
async fn api_playtest(State(st): State<Shared>, h: HeaderMap, Json(req): Json<PlaytestReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let g = graph_of(req.graph)?;
        let r = compile(&st, g, true)?;
        if r["ok"] == true {
            let game = local_game(&st)?;
            let id = r["installedAs"].as_str().unwrap_or_default().to_string();
            debug::start(&game.dir, &id, req.breakpoints).map_err(|e| format!("Couldn't get the debugger ready ({e})."))?;
        }
        Ok(r)
    })
    .await
}

#[derive(Deserialize)]
struct SwitchReq {
    mode: String,
    map: String,
}

async fn api_switch(State(st): State<Shared>, h: HeaderMap, Json(req): Json<SwitchReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || switch_open_game(&st, &req.mode, &req.map)).await
}

async fn api_uninstall(State(st): State<Shared>, UrlPath(id): UrlPath<String>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err(BAD_ID.into());
        }
        let game = local_game(&st)?;
        let dir = game.dir.clone();
        // a Studio graph's mode: wherever its uid is installed
        let short = read_graph(&graphs_dir(), &id).map(|g| install_id(&dir, &g)).unwrap_or(id);
        uninstall(&dir, &short)?;
        let mut v = json!({"ok": true});
        if let (Value::Object(o), Value::Object(r)) = (&mut v, modes_reload::for_local_game(&game).json()) {
            o.extend(r);
        }
        Ok(v)
    })
    .await
}

#[derive(Deserialize)]
struct EventsQuery {
    mode: String,
    #[serde(default)]
    from: u64,
}

async fn api_events(State(st): State<Shared>, Query(q): Query<EventsQuery>) -> Response {
    blocking(move || {
        if !graph::valid_mode_id(&q.mode) {
            return Err(BAD_ID.into());
        }
        let game = local_game(&st)?;
        let b = debug::read_events(&debug::events_file(&game.dir, &q.mode), q.from).map_err(|e| format!("Couldn't read the debugger's events ({e})."))?;
        // (only for a Playtest that's running: the plugin made the folder)
        if debug::dir(&game.dir).is_dir() {
            debug::mark_seen(&game.dir, &q.mode, b.next);
        }
        Ok(json!({"events": b.events, "next": b.next, "reset": b.reset}))
    })
    .await
}

#[derive(Deserialize)]
struct CommandReq {
    mode: String,
    #[serde(default)]
    resume: bool,
    #[serde(default)]
    vars: bool,
    #[serde(default)]
    breakpoints: Option<Vec<String>>,
    /// a live tweak: {var, who, value} (debug builds only; the graph says the variable's type)
    #[serde(default)]
    set: Option<SetReq>,
}

#[derive(Deserialize)]
struct SetReq {
    var: String,
    #[serde(default)]
    who: u32,
    value: Value,
    graph: Value,
}

async fn api_command(State(st): State<Shared>, h: HeaderMap, Json(req): Json<CommandReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if !graph::valid_mode_id(&req.mode) {
            return Err(BAD_ID.into());
        }
        let game = local_game(&st)?;
        guard_real_game(&game.dir)?;
        let mut cmds = Vec::new();
        // breakpoints first: Resume with them all off doesn't stop again at the next one
        if let Some(b) = req.breakpoints {
            debug::set_breakpoints(&game.dir, &req.mode, b).map_err(|e| format!("Couldn't reach the plugin ({e})."))?;
        }
        if req.vars {
            cmds.push(debug::Command::Vars);
        }
        if req.resume {
            cmds.push(debug::Command::Resume);
        }
        if let Some(set) = req.set {
            let g = graph_of(set.graph)?;
            let v = g.variables.iter().chain(g.functions.iter().flat_map(|f| f.locals.iter())).find(|x| x.id == set.var).ok_or("That variable isn't in the mode.")?;
            if set.who > 32 || (v.scope == "team" && !(1..=2).contains(&set.who)) || (v.scope == "player" && set.who == 0) {
                return Err("That player or team isn't right for this variable.".into());
            }
            let raw = match &set.value {
                Value::String(s) => s.clone(),
                Value::Bool(b) => (if *b { "1" } else { "0" }).into(),
                other => other.to_string(),
            };
            cmds.push(debug::Command::Set { var: v.id.clone(), who: if v.scope == "mode" { 0 } else { set.who }, value: debug::set_value(&v.ty, &raw)? });
        }
        debug::send(&debug::cmd_file(&game.dir, &req.mode), &cmds).map_err(|e| format!("Couldn't reach the plugin ({e})."))?;
        Ok(json!({"ok": true}))
    })
    .await
}

async fn api_snippets() -> Response {
    blocking(move || {
        let list: Vec<Value> = std::fs::read_dir(snippets_dir())
            .map(|d| {
                d.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let id = name.strip_suffix(".snippet.json")?.to_string();
                        if !graph::valid_mode_id(&id) || e.metadata().ok()?.len() > MAX_GRAPH_BYTES {
                            return None;
                        }
                        let v: Value = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
                        check_snippet(&v).ok()?;
                        Some(json!({"id": id, "snippet": v}))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(json!({"snippets": list}))
    })
    .await
}

async fn api_snippet_save(UrlPath(id): UrlPath<String>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err("A snippet's id is 2 to 24 small letters, digits or _.".into());
        }
        let v = body.get("snippet").cloned().unwrap_or(Value::Null);
        check_snippet(&v)?;
        let text = serde_json::to_string_pretty(&v).map_err(|_| INTERNAL.to_string())?;
        if text.len() as u64 > MAX_GRAPH_BYTES {
            return Err("That snippet is too big.".into());
        }
        let dir = snippets_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't save the snippet ({e})."))?;
        let path = dir.join(format!("{id}.snippet.json"));
        crate::safety::before("Game Mode Studio", "save a snippet", &path);
        crate::sys::write_atomic(&path, text.as_bytes()).map_err(|e| format!("Couldn't save the snippet ({e})."))?;
        Ok(json!({"ok": true}))
    })
    .await
}

async fn api_snippet_delete(UrlPath(id): UrlPath<String>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err("That isn't a snippet.".into());
        }
        let path = snippets_dir().join(format!("{id}.snippet.json"));
        crate::safety::before("Game Mode Studio", "delete a snippet", &path);
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Couldn't delete the snippet ({e}).")),
            _ => Ok(json!({"ok": true})),
        }
    })
    .await
}

async fn api_library() -> Response {
    blocking(move || {
        let list: Vec<Value> = std::fs::read_dir(library_dir())
            .map(|d| {
                d.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let id = name.strip_suffix(".lib.json")?.to_string();
                        if !graph::valid_mode_id(&id) || e.metadata().ok()?.len() > MAX_GRAPH_BYTES {
                            return None;
                        }
                        let v: Value = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
                        check_library_item(&v).ok()?;
                        Some(json!({"id": id, "item": v}))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // the app's own items (built in: read-only), after the player's, ids builtin_<name>
        let mut list = list;
        for (name, text) in builtin_library::BUILTIN {
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                list.push(json!({"id": format!("builtin_{name}"), "builtIn": true, "item": v}));
            }
        }
        Ok(json!({"items": list}))
    })
    .await
}

/// Saves a library item; its version goes up by one when what it holds changed (the copies in
/// modes remember the version they came from).
pub fn save_library_item(id: &str, item: &Value) -> Result<u64, String> {
    save_library_item_in(&library_dir(), id, item)
}

pub fn save_library_item_in(dir: &Path, id: &str, item: &Value) -> Result<u64, String> {
    if !graph::valid_mode_id(id) {
        return Err("A library item's id is 2 to 24 small letters, digits or _.".into());
    }
    check_library_item(item)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save to the library ({e})."))?;
    let path = dir.join(format!("{id}.lib.json"));
    let old: Option<Value> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let version = match &old {
        Some(o) if library_body(o) == library_body(item) => o.get("itemVersion").and_then(Value::as_u64).unwrap_or(1),
        Some(o) => o.get("itemVersion").and_then(Value::as_u64).unwrap_or(1) + 1,
        None => 1,
    };
    let mut v = item.clone();
    v["itemVersion"] = json!(version);
    let text = serde_json::to_string_pretty(&v).map_err(|_| INTERNAL.to_string())?;
    if text.len() as u64 > MAX_GRAPH_BYTES {
        return Err("That library item is too big.".into());
    }
    crate::safety::before("Game Mode Studio", "save to the library", &path);
    crate::sys::write_atomic(&path, text.as_bytes()).map_err(|e| format!("Couldn't save to the library ({e})."))?;
    Ok(version)
}

async fn api_library_save(UrlPath(id): UrlPath<String>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let v = body.get("item").cloned().unwrap_or(Value::Null);
        let version = save_library_item(&id, &v)?;
        Ok(json!({"ok": true, "version": version}))
    })
    .await
}

async fn api_library_delete(UrlPath(id): UrlPath<String>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if !graph::valid_mode_id(&id) {
            return Err("That isn't a library item.".into());
        }
        let path = library_dir().join(format!("{id}.lib.json"));
        crate::safety::before("Game Mode Studio", "delete from the library", &path);
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Couldn't delete the library item ({e}).")),
            _ => Ok(json!({"ok": true})),
        }
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/studio/status", get(api_status))
        .merge(modelpoints::routes())
        .route("/api/tools/studio/graphs", get(api_graphs))
        // (a graph at the node cap is more than axum's 2 MB default)
        .route("/api/tools/studio/graph/:id", get(api_graph).post(api_save).layer(axum::extract::DefaultBodyLimit::max(MAX_GRAPH_BYTES as usize + (1 << 20))))
        .route("/api/tools/studio/graph/:id/delete", post(api_delete))
        .route("/api/tools/studio/history/:id", get(api_history))
        .route("/api/tools/studio/history/:id/:n", get(api_history_version))
        .route("/api/tools/studio/validate", post(api_validate))
        .route("/api/tools/studio/generate", post(api_generate))
        .route("/api/tools/studio/compile", post(api_compile))
        .route("/api/tools/studio/check", post(api_check))
        .route("/api/tools/studio/mode-settings", get(settings::api_get).post(settings::api_set))
        .route("/api/tools/studio/testrun", get(api_testrun_progress).post(api_testrun))
        .route("/api/tools/studio/testrun/cancel", post(api_testrun_cancel))
        .route("/api/tools/studio/source", post(api_source))
        .route("/api/tools/studio/snippets", get(api_snippets))
        .route("/api/tools/studio/snippet/:id", post(api_snippet_save))
        .route("/api/tools/studio/snippet/:id/delete", post(api_snippet_delete))
        .route("/api/tools/studio/kits", get(api_kits))
        .route("/api/tools/studio/kit/:id", get(api_kit))
        .route("/api/tools/studio/library", get(api_library))
        .route("/api/tools/studio/library/:id", post(api_library_save))
        .route("/api/tools/studio/library/:id/delete", post(api_library_delete))
        .route("/api/tools/studio/playtest", post(api_playtest))
        .route("/api/tools/studio/switch", post(api_switch))
        .route("/api/tools/studio/uninstall/:id", post(api_uninstall))
        .route("/api/tools/studio/debug/events", get(api_events))
        .route("/api/tools/studio/debug/command", post(api_command))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json");

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-studio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Every built-in snippet passes the snippet check, and dropped into an empty mode it checks
    /// clean and makes a plugin (compiled with amxxpc in the compile test below).
    pub fn builtin_snippets() -> Vec<(String, Value)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/views/tools/studio/snippets");
        let mut out: Vec<(String, Value)> = std::fs::read_dir(dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".snippet.json")).map(|e| (e.file_name().to_string_lossy().replace(".snippet.json", ""), serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap())).collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// The library items that ship with the app (ui/.../studio/library, make_library.py).
    pub fn builtin_library() -> Vec<(String, Value)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/views/tools/studio/library");
        let mut out: Vec<(String, Value)> = std::fs::read_dir(dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".lib.json")).map(|e| (e.file_name().to_string_lossy().replace(".lib.json", ""), serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap())).collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Run Dialog in a mode, as a player would wire it: Talked To NPC -> Run Dialog (row
    /// "start") -> a Print of the row it ended at.
    pub fn dialogue_mode() -> Graph {
        let (_, item) = builtin_library().into_iter().find(|(id, _)| id == "run_dialog").unwrap();
        let mut g = item.clone();
        let o = g.as_object_mut().unwrap();
        for k in ["name", "description", "category", "kind", "root"] {
            o.remove(k);
        }
        o.insert("schema".into(), json!(graph::SCHEMA));
        o.insert("version".into(), json!(graph::VERSION));
        o.insert("mode".into(), json!({"id": "dialogue_test", "name": "Dialog test"}));
        let nodes = o.get_mut("nodes").unwrap().as_array_mut().unwrap();
        nodes.extend([
            json!({"id": "talk", "type": "npc.on_talk", "pos": {"x": 0, "y": 0}, "params": {}}),
            json!({"id": "from", "type": "text.literal", "pos": {"x": 0, "y": 200}, "params": {"value": "start"}}),
            json!({"id": "run", "type": "macro.use", "pos": {"x": 300, "y": 0}, "params": {"macro": "rundlg"}}),
            json!({"id": "said", "type": "debug.print", "pos": {"x": 600, "y": 0}, "params": {"text": "Dialog ended at", "chat": false}}),
        ]);
        let edges = o.get_mut("edges").unwrap().as_array_mut().unwrap();
        for (i, (a, ap, b, bp)) in [("talk", "then", "run", "go"), ("talk", "npc", "run", "npc"), ("talk", "player", "run", "who"), ("from", "out", "run", "start"), ("run", "end", "said", "exec"), ("run", "last", "said", "value")].iter().enumerate() {
            edges.push(json!({"id": format!("u{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}}));
        }
        graph::from_value(g).unwrap()
    }

    /// Every built-in library item passes the library check (graph data only, no events), and
    /// Run Dialog wired into a mode checks clean (its Dialog table's rows link up).
    #[test]
    fn builtin_library_items_check_clean() {
        let items = builtin_library();
        assert!(items.iter().any(|(id, _)| id == "run_dialog"));
        for (id, v) in &items {
            check_library_item(v).unwrap_or_else(|e| panic!("{id}: {e}"));
        }
        let g = dialogue_mode();
        let p: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level != "info").collect();
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, false).source;
        // the guard and the clean end
        assert!(src.contains("Run Dialog: no row of Dialog has the id") && src.contains("stopped after 64 lines"));
    }

    /// Status Icon, Objective Marker (on a bot, and at a spot for that bot only), Kill Feed Line
    /// and Remove Marker in one mode (the compile test and the sim run use it).
    pub fn ui_nodes_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let nodes = vec![
            n("rs", "event.round_started", json!({})),
            n("d", "timer.delay", json!({"secs": 3})),
            n("pk", "flow.pick_player", json!({"which": "bots"})),
            n("ic1", "msg.status_icon", json!({"icon": "c4", "show": "flash", "r": 255, "g": 0, "b": 0, "death": true})),
            n("ic2", "msg.status_icon", json!({"icon": "buyzone", "show": "on", "death": false})),
            n("mk1", "fx.marker", json!({"sprite": "sprites/arrow1.spr", "who": "all", "height": 64})),
            n("vp", "vec.player", json!({})),
            n("mk2", "fx.marker", json!({"sprite": "sprites/glow01.spr", "who": "player", "pulse": false})),
            n("kf", "msg.killfeed", json!({"weapon": "ak47", "hs": true})),
            n("p1", "debug.print", json!({"text": "UI marker", "chat": false})),
            n("d2", "timer.delay", json!({"secs": 4})),
            n("rm1", "fx.marker_remove", json!({})),
            n("rm2", "fx.marker_remove", json!({})),
        ];
        let edges = vec![
            e(1, "rs", "then", "d", "exec"), e(2, "d", "then", "pk", "exec"), e(3, "pk", "then", "ic1", "exec"), e(4, "pk", "player", "ic1", "player"),
            e(5, "ic1", "then", "ic2", "exec"), e(6, "ic2", "then", "mk1", "exec"), e(7, "pk", "player", "mk1", "follow"),
            e(8, "pk", "player", "vp", "player"), e(9, "mk1", "then", "mk2", "exec"), e(10, "vp", "out", "mk2", "pos"), e(11, "pk", "player", "mk2", "player"),
            e(12, "mk2", "then", "kf", "exec"), e(13, "pk", "player", "kf", "killer"), e(14, "pk", "player", "kf", "victim"),
            e(15, "kf", "then", "p1", "exec"), e(16, "mk1", "out", "p1", "value"), e(17, "p1", "then", "d2", "exec"),
            e(18, "d2", "then", "rm1", "exec"), e(19, "mk1", "out", "rm1", "marker"), e(20, "rm1", "then", "rm2", "exec"),
        ];
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "ui_nodes", "name": "UI nodes"}, "nodes": nodes, "edges": edges})).unwrap()
    }

    /// The UI nodes check clean and make what they say: the icon message (to one, to all), the
    /// marker's sprite loaded with the map only when used, the task that moves markers, the
    /// one-player marker's send filter, the death hook for icons until death, and the kill feed
    /// sent to each game one at a time (never a global DeathMsg other plugins would count).
    #[test]
    fn ui_nodes_generate() {
        let g = ui_nodes_mode();
        let p = graph::validate(&g);
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, false).source;
        for want in [
            "lsg_icon(g_lsgCtx", "lsg_pre_model(\"sprites/arrow1.spr\")", "lsg_pre_model(\"sprites/glow01.spr\")", "set_task(0.1, \"lsg_mk_tick\"", "register_forward(FM_AddToFullPack, \"lsg_mk_full\", 0)",
            "RegisterHamPlayer(Ham_Killed, \"lsg_icon_dead\", 1)", "\"ak47\")", "message_begin(MSG_ONE, msg, _, p)", "lsg_marker_remove(0)",
        ] {
            assert!(src.contains(want) || (want.starts_with("lsg_icon(") && src.contains("lsg_icon(")), "{want}");
        }
        assert!(!src.contains("message_begin(MSG_ALL, get_user_msgid(\"DeathMsg\")"));
        // no marker: none of it
        let mut bare = g.clone();
        bare.nodes.retain(|n| !n.kind.starts_with("fx.marker"));
        bare.edges.retain(|e| bare.nodes.iter().any(|n| n.id == e.from.node) && bare.nodes.iter().any(|n| n.id == e.to.node));
        let src = gen::generate(&bare, false).source;
        assert!(!src.contains("lsg_mk_tick\"") && !src.contains("sprites/arrow1.spr") && !src.contains("FM_AddToFullPack"));
        // the checks
        let mut bad = g.clone();
        for n in bad.nodes.iter_mut().filter(|n| n.id == "mk1") {
            n.params.insert("sprite".into(), json!("models/x.mdl"));
        }
        bad.edges.retain(|e| !(e.to.node == "mk2" && (e.to.pin == "pos" || e.to.pin == "player")));
        let p: Vec<String> = graph::validate(&bad).into_iter().map(|p| p.message).collect();
        assert!(p.iter().any(|m| m.contains("sprites/....spr")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("needs something to follow")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("wire the player into Player")), "{p:?}");
    }

    /// Player By Steam ID (its player and Found) and Is Supporter, printed when a player spawns.
    pub fn steam_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "steam_nodes", "name": "Steam nodes"},
            "nodes": [n("ps", "event.player_spawned", json!({})), n("sid", "player.steam_id", json!({})), n("by", "player.by_steam", json!({})), n("found", "debug.print", json!({"text": "found", "chat": false})),
                      n("who", "debug.print", json!({"text": "who", "chat": false})), n("sup", "player.is_supporter", json!({})), n("ps2", "debug.print", json!({"text": "supporter", "chat": false})), n("lit", "player.by_steam", json!({"id": "steam_1:0:42"}))],
            "edges": [e(1, "ps", "then", "found", "exec"), e(2, "ps", "player", "sid", "player"), e(3, "sid", "out", "by", "id"), e(4, "by", "found", "found", "value"),
                      e(5, "found", "then", "who", "exec"), e(6, "lit", "out", "who", "value"), e(7, "who", "then", "ps2", "exec"), e(8, "ps", "player", "sup", "player"), e(9, "sup", "out", "ps2", "value")]})).unwrap()
    }

    /// Player By Steam ID matches on the X:Y after STEAM_n: (case and the universe digit aside),
    /// Found is whether there's one; Is Supporter reads access flag s (gm_vip's supporter), never
    /// for bots, HLTV or the listen-server host.
    #[test]
    fn steam_and_supporter_nodes_generate() {
        let g = steam_mode();
        let p = graph::validate(&g);
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, false).source;
        assert!(src.contains("lsg_by_steam(\"steam_1:0:42\")"), "the typed id as text");
        assert!(src.contains("(lsg_by_steam(lsg_steam_id(") && src.contains(") > 0 ? 1 : 0)"), "Found");
        assert!(src.contains("equali(s[8], id[8])"), "X:Y compared, any case, either universe");
        assert!(src.contains("(get_user_flags(id) & ADMIN_LEVEL_G) != 0") && src.contains("id == 1 && !is_dedicated_server()"));
    }

    /// Every fun node, on a spawned player (and a second bot for Swap Places), Set Camera both ways.
    pub fn fun_mode(reset: bool) -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let mut nodes = vec![
            n("ps", "event.player_spawned", json!({})), n("pk", "flow.pick_player", json!({"which": "bots"})),
            n("burn", "fun.burn", json!({"secs": 3, "dps": 2})), n("out", "fun.extinguish", json!({})), n("trail", "fun.trail", json!({"on": true, "width": 8, "life": 2})),
            n("b1", "fun.burst", json!({"kind": "smoke", "count": 2})), n("b2", "fun.burst", json!({"kind": "firework"})), n("b3", "fun.burst", json!({"kind": "blood"})), n("vp", "vec.player", json!({})),
            n("swap", "fun.swap", json!({})), n("pr", "debug.print", json!({"text": "swapped", "chat": false})), n("bury", "fun.bury", json!({})), n("up", "fun.unbury", json!({})),
            n("cam", "fun.camera", json!({"view": "third", "reset": reset})), n("third", "fun.is_third", json!({})), n("pr2", "debug.print", json!({"text": "third", "chat": false})),
            n("cam1", "fun.camera", json!({"view": "toggle", "reset": reset})), n("drunk", "fun.drunk", json!({"secs": 4, "strength": 5})),
        ];
        nodes.push(n("cam0", "fun.camera", json!({"view": "first"})));
        nodes.push(n("vf", "fun.view_from", json!({"who": "player", "secs": 3, "move": 2, "freeze": true})));
        nodes.push(n("vf2", "fun.view_from", json!({"who": "all", "secs": 0, "offset": 80})));
        // (looking at a position from one; framing a conversation with an NPC, until it ends)
        nodes.push(n("vf3", "fun.view_from", json!({"who": "ct", "secs": 4})));
        nodes.push(n("vf4", "fun.view_from", json!({"who": "player", "secs": 0, "frame": true, "talk": true})));
        nodes.push(n("vr", "fun.view_return", json!({})));
        let chain = ["ps", "pk", "burn", "out", "trail", "b1", "b2", "b3", "swap", "pr", "bury", "up", "cam", "pr2", "cam1", "drunk", "cam0", "vf", "vf2", "vf3", "vf4", "vr"];
        let mut edges: Vec<Value> = chain.windows(2).enumerate().map(|(i, w)| json!({"id": format!("c{i}"), "from": {"node": w[0], "pin": "then"}, "to": {"node": w[1], "pin": "exec"}})).collect();
        let mut wire = |a: &str, ap: &str, b: &str, bp: &str| {
            let id = format!("d{}", edges.len());
            edges.push(json!({"id": id, "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}}));
        };
        for t in ["burn", "out", "bury", "up", "cam", "third", "cam1", "drunk", "cam0"] {
            wire("ps", "player", t, "player");
        }
        wire("pk", "player", "burn", "by");
        wire("ps", "player", "trail", "ent");
        wire("ps", "player", "b1", "at");
        wire("ps", "player", "vp", "player");
        wire("vp", "out", "b2", "pos");
        wire("ps", "player", "b3", "at");
        wire("ps", "player", "swap", "a");
        wire("pk", "player", "swap", "b");
        wire("swap", "ok", "pr", "value");
        wire("third", "out", "pr2", "value");
        wire("ps", "player", "vf", "player");
        wire("vp", "out", "vf", "pos");
        wire("vp", "out", "vf", "to");
        wire("pk", "player", "vf2", "from");
        wire("vp", "out", "vf3", "pos");
        wire("vp", "out", "vf3", "look_pos");
        wire("ps", "player", "vf4", "player");
        wire("pk", "player", "vf4", "look");
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "fun_nodes", "name": "Fun nodes"}, "nodes": nodes, "edges": edges})).unwrap()
    }

    /// The fun nodes check clean and make what they say, sprites loaded only when used, one task
    /// for all of them, the death/spawn/leaving hooks; Set Camera's "back to first person on
    /// death/respawn" both ways (kept: set again after a respawn; leaving always ends it).
    #[test]
    fn fun_nodes_generate() {
        for reset in [true, false] {
            let g = fun_mode(reset);
            let p = graph::validate(&g);
            assert!(p.is_empty(), "{p:#?}");
            let src = gen::generate(&g, false).source;
            for want in [
                "lsg_pre_model(\"sprites/fire.spr\")", "lsg_pre_model(\"sprites/steam1.spr\")", "lsg_pre_model(\"sprites/blood.spr\")", "g_lsgSprBeam = precache_model(\"sprites/laserbeam.spr\")",
                "set_task(0.1, \"lsg_fun_tick\"", "RegisterHamPlayer(Ham_Killed, \"lsg_fun_dead\", 1)", "RegisterHamPlayer(Ham_Spawn, \"lsg_fun_spawn\", 1)", "lsg_fun_left(id)",
                "lsg_swap(", "lsg_bury(", "lsg_unbury(", "lsg_drunk(", "lsg_burst(1, lsgAt, 2", "lsg_burst(3, lsgAt, 1",
                "lsg_view_from(0, ", "lsg_view_from(3, 0, ", "lsg_view_return(0)",
                "lsg_view_look(0, lsgL, 0, 0, 0)", "lsg_view_look(0, lsgL, 1, 0, 0)", "lsgL, 0, 1, 1)", "lsg_view_cam(p, from, a, aa, b, ba, hasB, off, move)",
                "if (ls_npc_talking(p))", "return g_lsgViewTalked[p]", "lsg_cam_aim(cam)", "register_forward(FM_CmdStart, \"lsg_view_cmd\", 0)", "lsg_view_back(p)",
            ] {
                assert!(src.contains(want), "{want}");
            }
            assert!(src.contains(&format!(", 1, {})", if reset { 1 } else { 0 })), "third person, reset {reset}");
            assert!(src.contains(&format!(", 2, {})", if reset { 1 } else { 0 })), "switch, reset {reset}");
            assert!(src.contains("if (g_lsgCam[id] && !g_lsgCamKeep[id])") && src.contains("set_task(0.2, \"lsg_cam_again\""));
        }
        // nothing fun: none of it
        let g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "plain", "name": "Plain"}, "nodes": [{"id": "rs", "type": "event.round_started"}, {"id": "p", "type": "debug.print", "params": {"text": "x"}}], "edges": [{"id": "w", "from": {"node": "rs", "pin": "then"}, "to": {"node": "p", "pin": "exec"}}]})).unwrap();
        let src = gen::generate(&g, false).source;
        assert!(!src.contains("lsg_fun_tick\"") && !src.contains("lsg_pre_model(\"sprites/fire.spr\")") && !src.contains("lsg_fun_dead\""));
        // an Effect Burst with nowhere to go
        let mut bad = fun_mode(true);
        bad.edges.retain(|e| e.to.node != "b2");
        bad.edges.retain(|e| !(e.from.node == "b1" || e.to.node == "b2"));
        let p: Vec<String> = graph::validate(&bad).into_iter().map(|p| p.message).collect();
        assert!(p.iter().any(|m| m.contains("Effect Burst needs")), "{p:?}");
    }

    /// The casts: Find Component's entity As Player (Success prints its name, Failed says so),
    /// As Creature (its type), and Is NPC / Is Interactable / Is Component (a kind and any).
    pub fn cast_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "cast_nodes", "name": "Casts"},
            "nodes": [n("rs", "event.round_started", json!({})), n("pe", "flow.pick_entity", json!({"by": "class", "name": "player"})),
                      n("ap", "cast.player", json!({})), n("name", "player.get_name", json!({})), n("ok", "debug.print", json!({"text": "a player", "chat": false})), n("no", "debug.print", json!({"text": "not a player", "chat": false})),
                      n("ac", "cast.creature", json!({})), n("ty", "debug.print", json!({"text": "creature", "chat": false})),
                      n("npc", "ent.is_npc", json!({})), n("use", "ent.is_interactable", json!({})), n("shop", "ent.is_component", json!({"kind": "ls_shop"})), n("any", "ent.is_component", json!({"kind": "any"})),
                      n("p1", "debug.print", json!({"text": "npc", "chat": false})), n("p2", "debug.print", json!({"text": "use", "chat": false})), n("p3", "debug.print", json!({"text": "shop", "chat": false})), n("p4", "debug.print", json!({"text": "any", "chat": false}))],
            "edges": [e(1, "rs", "then", "pe", "exec"), e(2, "pe", "then", "ap", "exec"), e(3, "pe", "entity", "ap", "in"), e(4, "ap", "ok", "ok", "exec"), e(5, "ap", "out", "name", "player"), e(6, "name", "out", "ok", "value"),
                      e(7, "ap", "fail", "no", "exec"), e(8, "ok", "then", "ac", "exec"), e(9, "pe", "entity", "ac", "in"), e(10, "ac", "ok", "ty", "exec"), e(11, "ac", "type", "ty", "value"),
                      e(12, "ac", "fail", "p1", "exec"), e(13, "pe", "entity", "npc", "in"), e(14, "npc", "out", "p1", "value"), e(15, "p1", "then", "p2", "exec"), e(16, "pe", "entity", "use", "in"), e(17, "use", "out", "p2", "value"),
                      e(18, "p2", "then", "p3", "exec"), e(19, "pe", "entity", "shop", "in"), e(20, "shop", "out", "p3", "value"), e(21, "p3", "then", "p4", "exec"), e(22, "pe", "entity", "any", "in"), e(23, "any", "out", "p4", "value")]})).unwrap()
    }

    /// As Player succeeds for a connected slot 1-32 (bots too) with its Player kept for what reads
    /// it, else Failed; As Creature keeps the creature and its type; the Is checks ask gm_components
    /// the entity's kind (a picked one, or any).
    #[test]
    fn casts_generate() {
        let g = cast_mode();
        let p = graph::validate(&g);
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, true).source;
        for want in [
            "if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))", "lsd_exit(", "\"success\"", "\"failed\"", "if (lsg_is_creature(lsgE))", "copy(g_lsgOT[",
            "lsg_cr_type(lsgE)", "lsg_kind_ok(", "\"ls_npc\") ? 1 : 0)", "\"ls_interactable\") ? 1 : 0)", "\"ls_shop\") ? 1 : 0)", "\"\") ? 1 : 0)",
        ] {
            assert!(src.contains(want), "{want}");
        }
        assert!(src.contains("g_lsgO[0] = lsgE") && src.contains("g_lsgO[0] = 0"), "the Player kept on Success, cleared on Failed");
    }

    /// Kind Of into Switch on Enum (the built-in Entity Kind: nine outputs, nothing typed) and
    /// Select, and Component Kind Of into another Switch on Enum.
    pub fn kind_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "kind_nodes", "name": "Kinds"},
            "nodes": [n("ps", "event.player_spawned", json!({})), n("k", "ent.kind_of", json!({})), n("sw", "flow.switch_enum", json!({"enum": "ls_kind"})),
                      n("pp", "debug.print", json!({"text": "a player", "chat": false})), n("pb", "debug.print", json!({"text": "a bot", "chat": false})),
                      n("sel", "logic.select", json!({"of": "enum:ls_kind"})), n("k2", "ent.kind_of", json!({})), n("ps2", "debug.print", json!({"text": "kind", "chat": false})),
                      n("pe", "flow.pick_entity", json!({"by": "class", "name": "ls_shop"})), n("ck", "ent.comp_kind", json!({})), n("sw2", "flow.switch_enum", json!({"enum": "ls_comp_kind"})), n("shop", "debug.print", json!({"text": "a shop", "chat": false})), n("bot", "player.is_bot", json!({}))],
            "edges": [e(1, "ps", "then", "sw", "exec"), e(2, "ps", "player", "k", "in"), e(3, "k", "out", "sw", "in"), e(4, "sw", "v0", "pp", "exec"), e(5, "sw", "v1", "pb", "exec"),
                      e(6, "pb", "then", "ps2", "exec"), e(7, "k", "out", "sel", "a"), e(8, "k2", "out", "sel", "b"), e(9, "sel", "out", "ps2", "value"),
                      e(10, "ps2", "then", "pe", "exec"), e(11, "pe", "entity", "ck", "in"), e(12, "pe", "then", "sw2", "exec"), e(13, "ck", "out", "sw2", "in"), e(14, "sw2", "v3", "shop", "exec"), e(15, "ps", "player", "bot", "player"), e(16, "bot", "out", "sel", "cond")]})).unwrap()
    }

    /// The app's own enums are in every graph and never saved: Kind Of's Entity Kind gives Switch
    /// on Enum its nine outputs and plugs into Select; Component Kind Of's follows gm_components'
    /// kinds in order (None last); a saved graph doesn't carry them.
    #[test]
    fn builtin_enums_switch_and_select() {
        let g = kind_mode();
        let p = graph::validate(&g);
        assert!(p.is_empty(), "{p:#?}");
        let v = graph::View::new(&g);
        let sw = v.outputs("sw").iter().map(|p| p.label.clone()).collect::<Vec<_>>();
        assert_eq!(sw, ["Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None"]);
        assert_eq!(v.outputs("sw2").len(), gen::COMP_KINDS.len() + 1);
        assert_eq!(graph::BUILTIN_ENUMS[1].2.len(), gen::COMP_KINDS.len() + 1, "a value per kind, and None");
        let src = gen::generate(&g, false).source;
        assert!(src.contains("stock lsg_kind_of(e)") && src.contains("stock lsg_comp_kind_of(e)") && src.contains("\"ls_shop\""), "the helpers");
        assert!(src.contains("switch (lsg_kind_of("), "the switch on its value");
        // saved: the app's own enums stay out of the file
        let saved = serde_json::to_value(&g).unwrap();
        assert!(saved.get("enums").is_none(), "{:?}", saved.get("enums"));
    }

    /// Comfort limits (limits.rs): the plugin reads the mode's Limits setting when it has one
    /// (hosts can change it), else the default; a big raise is a warning, never an error; and the
    /// spawners keep the entity-slot reserve.
    #[test]
    fn limits_are_settings_and_spawns_keep_a_reserve() {
        let base = ui_nodes_mode();
        let src = gen::generate(&base, false).source;
        assert!(src.contains(", 32)\n") || src.contains(", 32)"), "no setting: the default");
        assert!(src.contains("stock bool:lsg_ent_room(const what[])") && src.contains("lsg_ent_room(\"Objective Marker\")"));
        let mut g = base.clone();
        let l = limits::get("markers").unwrap();
        let mut v: graph::Variable = serde_json::from_value(limits::var_json(l)).unwrap();
        v.default = json!(50);
        g.variables.push(v.clone());
        assert!(!graph::validate(&g).iter().any(|p| p.message.contains("may lag")), "50: fine");
        let src = gen::generate(&g, false).source;
        assert!(src.contains("clamp(floatround(") && src.contains(", 1, 256))"), "the setting's value, clamped");
        let mut big = g.clone();
        big.variables.last_mut().unwrap().default = json!(200);
        let p = graph::validate(&big);
        assert!(p.iter().any(|p| p.level == "warning" && p.message.contains("Objective markers at once is set to 200") && p.message.contains("may lag")), "{p:#?}");
        assert!(!p.iter().any(|p| p.level == "error"));
        // the setting itself is a mode setting in the Limits group
        assert_eq!(v.setting.as_ref().unwrap().group, "Limits");
        assert!(settings::manifest(&g).to_string().contains("Objective markers at once"));
    }

    /// Transform: the app's own struct, put in only when used; the transform nodes' Transform pins
    /// (the separate pins win), a Transform variable that keeps where someone was, Make / Break
    /// Transform, Set Entity Transform.
    pub fn transform_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let nodes = vec![
            n("sp", "event.player_spawned", json!({})),
            n("get", "player.transform", json!({})),
            n("keep", "var.set", json!({"var": "was"})),
            n("d", "timer.delay", json!({"secs": 5})),
            n("gv", "var.get", json!({"var": "was"})),
            n("back", "player.set_transform", json!({})),
            n("prop", "ent.spawn_prop", json!({"model": "models/w_c4.mdl"})),
            n("mk", "struct.make", json!({"struct": "ls_transform"})),
            n("place", "ent.set_transform", json!({})),
            n("br", "struct.break", json!({"struct": "ls_transform"})),
            n("pr", "debug.print", json!({"text": "was at", "chat": false})),
            n("et", "ent.transform", json!({})),
        ];
        let edges = vec![
            e(1, "sp", "then", "keep", "exec"), e(2, "sp", "player", "get", "player"), e(3, "get", "transform", "keep", "value"), e(4, "keep", "then", "d", "exec"),
            e(5, "d", "then", "back", "exec"), e(6, "sp", "player", "back", "player"), e(7, "gv", "out", "back", "transform"),
            e(8, "back", "then", "prop", "exec"), e(9, "sp", "player", "prop", "at"), e(10, "prop", "spawned", "place", "exec"), e(11, "prop", "out", "place", "in"),
            e(12, "et", "transform", "br", "in"), e(13, "br", "pos", "mk", "pos"), e(14, "br", "rot", "mk", "rot"), e(15, "mk", "out", "place", "transform"),
            e(16, "place", "then", "pr", "exec"), e(17, "prop", "out", "et", "in"),
        ];
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "transforms", "name": "Transforms"}, "nodes": nodes, "edges": edges,
            "variables": [{"id": "was", "name": "Was at", "type": "struct:ls_transform", "scope": "mode"}]})).unwrap()
    }

    #[test]
    fn transforms_keep_where_something_was() {
        let g = transform_mode();
        assert!(g.structs.iter().any(|s| s.id == "ls_transform" && s.builtin));
        let p: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level != "info").collect();
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, false).source;
        for want in ["// struct Transform", "stock lsg_ent_transform(e, pos, rot, Float:scale, vel)", "lsg_escale(", "lsg_set_transform("] {
            assert!(src.contains(want), "{want}");
        }
        // never saved with the mode, and not put in where it isn't used
        assert!(!serde_json::to_string(&g).unwrap().contains("\"Transform\""));
        assert!(!graph::parse(TEMPLATE).unwrap().structs.iter().any(|s| s.builtin));
        // both wired: a warning, the separate pin wins
        let mut both = g.clone();
        both.edges.push(serde_json::from_value(json!({"id": "wb", "from": {"node": "get", "pin": "pos"}, "to": {"node": "back", "pin": "pos"}})).unwrap());
        assert!(graph::validate(&both).iter().any(|p| p.level == "warning" && p.message.contains("Both Transform and Position")));
    }

    /// Color: the Color nodes, a Color variable, and a wired Color winning over the old settings on
    /// Set Glow, Screen Fade, Beam, Trail, Effect Burst, Status Icon and Objective Marker.
    pub fn color_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let nodes = vec![
            n("sp", "event.player_spawned", json!({})),
            n("lit", "color.literal", json!({"value": "#20c060"})),
            n("glow", "player.set_glow", json!({"color": "red"})),
            n("mk", "color.make", json!({"r": 10, "g": 20, "b": 250, "a": 255})),
            n("fade", "player.fade", json!({"secs": 1, "alpha": 100})),
            n("hsv", "color.hsv", json!({"h": 200, "s": 1, "v": 1})),
            n("trail", "fun.trail", json!({"on": true})),
            n("mix", "color.lerp", json!({"t": 0.25})),
            n("burst", "fun.burst", json!({"kind": "sparks", "count": 2})),
            n("gv", "var.get", json!({"var": "team_col"})),
            n("ic", "msg.status_icon", json!({"icon": "dollar", "show": "on", "death": false})),
            n("br", "color.break", json!({})),
            n("pr", "debug.print", json!({"text": "green is", "chat": false})),
            n("pc", "debug.print", json!({"text": "color", "chat": false})),
        ];
        let edges = vec![
            e(1, "sp", "then", "glow", "exec"), e(2, "sp", "player", "glow", "player"), e(3, "lit", "out", "glow", "color"),
            e(4, "glow", "then", "fade", "exec"), e(5, "sp", "player", "fade", "player"), e(6, "mk", "out", "fade", "color"),
            e(7, "fade", "then", "trail", "exec"), e(8, "sp", "player", "trail", "ent"), e(9, "hsv", "out", "trail", "color"),
            e(10, "trail", "then", "burst", "exec"), e(11, "sp", "player", "burst", "at"), e(12, "lit", "out", "mix", "a"), e(13, "gv", "out", "mix", "b"), e(14, "mix", "out", "burst", "color"),
            e(15, "burst", "then", "ic", "exec"), e(16, "sp", "player", "ic", "player"), e(17, "gv", "out", "ic", "color"),
            e(18, "ic", "then", "pr", "exec"), e(19, "gv", "out", "br", "in"), e(20, "br", "g", "pr", "value"),
            e(21, "pr", "then", "pc", "exec"), e(22, "mix", "out", "pc", "value"),
        ];
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "colors", "name": "Colors"}, "nodes": nodes, "edges": edges,
            "variables": [{"id": "team_col", "name": "Team color", "type": "color", "scope": "mode", "default": "#ffcc00"}]})).unwrap()
    }

    #[test]
    fn colors_are_one_value_and_win_over_the_old_settings() {
        let g = color_mode();
        let p: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level != "info").collect();
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, false).source;
        for want in ["stock lsg_col(Float:r, Float:g, Float:b, Float:a)", "kRenderFxGlowShell, lsg_col_r(", "lsg_col_mix(", "lsg_col_hsv(", "float(lsg_col_g(", "lsg_col_text(", "lsg_fade("] {
            assert!(src.contains(want), "{want}");
        }
        // #20c060 packed with full alpha, the variable's #ffcc00
        assert!(src.contains(&(0xff20c060u32 as i32).to_string()) && src.contains(&(0xffffcc00u32 as i32).to_string()));
        // unwired, the settings as before; and no Color helpers in a plugin without colors
        let plain = gen::generate(&graph::parse(TEMPLATE).unwrap(), false).source;
        assert!(!plain.contains("lsg_col("));
        assert!(graph::color_hex("#12345").is_none() && graph::color_hex("#a1b2c3").is_some() && graph::color_hex("#a1b2c3d4") == Some((0xa1, 0xb2, 0xc3, 0xd4)));
        let mut bad = g.clone();
        bad.variables[0].default = json!("red");
        assert!(graph::validate(&bad).iter().any(|p| p.level == "error"), "a color start value is #rrggbb");
    }

    /// The built-in Weapon, Hit Zone and Damage Kind enums from the typed outputs beside the old
    /// text ones: into Switch on Enum (one output per value), into a text pin (a Weapon is its name).
    pub fn typed_enums_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let nodes = vec![
            n("hurt", "event.player_hurt", json!({})),
            n("sw", "flow.switch_enum", json!({"enum": "ls_hitzone"})),
            n("head", "debug.print", json!({"text": "headshot zone", "chat": false})),
            n("kill", "event.player_killed", json!({})),
            n("give", "player.give_weapon", json!({"weapon": "ak47", "ammo": 30})),
            n("held", "player.weapon_held", json!({})),
            n("sw2", "flow.switch_enum", json!({"enum": "ls_weapon"})),
            n("knife", "debug.print", json!({"text": "knife out", "chat": false})),
            n("pre", "event.player_about_hurt", json!({})),
            n("pk", "debug.print", json!({"text": "kind", "chat": false})),
            n("hit", "proj.on_hit", json!({})),
            n("pz", "debug.print", json!({"text": "zone", "chat": false})),
        ];
        let edges = vec![
            e(1, "hurt", "then", "sw", "exec"), e(2, "hurt", "zone", "sw", "in"), e(3, "sw", "v1", "head", "exec"),
            e(4, "kill", "then", "give", "exec"), e(5, "kill", "attacker", "give", "player"), e(6, "kill", "weapon_kind", "give", "weapon"),
            e(7, "give", "then", "sw2", "exec"), e(8, "kill", "attacker", "held", "player"), e(9, "held", "weapon", "sw2", "in"), e(10, "sw2", "v26", "knife", "exec"),
            e(11, "pre", "then", "pk", "exec"), e(12, "pre", "dmg_kind", "pk", "value"),
            e(13, "hit", "then", "pz", "exec"), e(14, "hit", "zone", "pz", "value"),
        ];
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "typed_enums", "name": "Typed enums"}, "nodes": nodes, "edges": edges})).unwrap()
    }

    #[test]
    fn typed_enums_switch_and_flow_into_text() {
        let g = typed_enums_mode();
        let p: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level != "info").collect();
        assert!(p.is_empty(), "{p:#?}");
        let src = gen::generate(&g, false).source;
        for want in ["stock lsg_wpn_k(const n[])", "stock lsg_hz_of(v, bits)", "stock lsg_dmg_k(bits)", "lsg_hz_of(victim, bits) // zone", "lsg_wpn_k(wpn) // weapon_kind",
            "ctx[5] = lsg_dmg_k(bits) // dmg_kind", "clamp(hitgroup, 0, 7) // zone", "lsg_wpn_k(lsg_held("] {
            assert!(src.contains(want), "{want}");
        }
        // the Weapon into Give Weapon's text pin goes as its name
        assert!(src.contains("g_lsgE") && src.contains("\"knife\""), "the Weapon names table");
        // a plugin that doesn't use them has none of it (1.0 plugins stay as they were)
        let plain = gen::generate(&graph::parse(TEMPLATE).unwrap(), false).source;
        assert!(!plain.contains("lsg_wpn_k") && !plain.contains("\"Hit Zone\"") && !plain.contains("// enum Weapon"));
    }

    /// A burst of state at once (32 players spawning in the same frame, each told their scoreboard
    /// team, tag, a team score and an icon): state messages are never dropped, the budget's
    /// overflow is queued per resend and key and goes out on the next frames.
    pub fn spawn_burst_mode() -> Graph {
        let n = |id: &str, t: &str, p: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let nodes = vec![
            n("sp", "event.player_spawned", json!({})),
            n("col", "eng.score_color", json!({"team": "ct"})),
            n("tag", "eng.score_tag", json!({"tag": "vip"})),
            n("sc", "hud.team_score", json!({"team": "ct", "score": 7})),
            n("ic", "msg.status_icon", json!({"icon": "dollar", "show": "on", "death": false})),
        ];
        let edges = vec![
            e(1, "sp", "then", "col", "exec"), e(2, "sp", "player", "col", "player"), e(3, "col", "then", "tag", "exec"), e(4, "sp", "player", "tag", "player"),
            e(5, "tag", "then", "sc", "exec"), e(6, "sc", "then", "ic", "exec"), e(7, "sp", "player", "ic", "player"),
        ];
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "spawn_burst", "name": "Spawn burst"}, "nodes": nodes, "edges": edges})).unwrap()
    }

    /// The queue as the plugin runs it, step for step (lsg_msg_fits / lsg_msg_state / lsg_msg_flush):
    /// 32 spawns in one frame, each sending 4 state messages (team color 2, tag 2, score 2, icon 1)
    /// against the default budget of 48 a frame.
    #[test]
    fn a_spawn_burst_drops_no_state_and_arrives_in_a_few_frames() {
        let src = gen::generate(&spawn_burst_mode(), false).source;
        for s in ["lsg_msg_state(\"Scoreboard Team Color\", \"lsg_eng_color_again\", id, 2)", "lsg_msg_state(\"Scoreboard Tag\", \"lsg_eng_tag_again\", id, 2)",
            "lsg_msg_state(\"Show Team Score\", \"lsg_eng_score_again\", team, 2)", "lsg_msg_state(\"Status Icon\", \"lsg_icon_again\", to * 64 + k, 1, to)",
            "public lsg_msg_flush()", "RequestFrame(\"lsg_msg_flush\")"] {
            assert!(src.contains(s), "{s}");
        }
        assert!(!src.contains("lsg_msg_room(\"Scoreboard") && !src.contains("lsg_msg_room(\"Show Team Score") && !src.contains("lsg_msg_room(\"Status Icon"), "state isn't dropped");
        // the model: (resend, key, cost, to)
        let lim = 48;
        let mut frame_n = 0;
        let mut per: [i32; 33] = [0; 33];
        let mut q: Vec<(&str, i32, i32, usize)> = Vec::new();
        let mut sent: Vec<(&str, i32)> = Vec::new();
        let fits = |n: &mut i32, per: &mut [i32; 33], cost: i32, to: usize| -> bool {
            if *n + cost > lim || (to >= 1 && per[to] + cost > lim / 2) { return false; }
            *n += cost;
            if to >= 1 { per[to] += cost; }
            true
        };
        let mut state = |q: &mut Vec<(&'static str, i32, i32, usize)>, n: &mut i32, per: &mut [i32; 33], sent: &mut Vec<(&'static str, i32)>, f: &'static str, key: i32, cost: i32, to: usize| {
            if q.is_empty() && fits(n, per, cost, to) { sent.push((f, key)); return; }
            if !q.iter().any(|e| e.0 == f && e.1 == key) { q.push((f, key, cost, to)); }
        };
        for p in 1..=32usize {
            state(&mut q, &mut frame_n, &mut per, &mut sent, "color", p as i32, 2, 0);
            state(&mut q, &mut frame_n, &mut per, &mut sent, "tag", p as i32, 2, 0);
            state(&mut q, &mut frame_n, &mut per, &mut sent, "score", 2, 2, 0);
            state(&mut q, &mut frame_n, &mut per, &mut sent, "icon", p as i32 * 64 + 11, 1, p);
        }
        let mut frames = 0;
        while !q.is_empty() {
            frames += 1;
            frame_n = 0;
            per = [0; 33];
            let mut done = 0;
            while done < q.len() && fits(&mut frame_n, &mut per, q[done].2, q[done].3) {
                sent.push((q[done].0, q[done].1));
                done += 1;
            }
            if done == 0 { sent.push((q[0].0, q[0].1)); done = 1; }
            q.drain(..done);
        }
        for p in 1..=32 {
            assert!(sent.contains(&("color", p)) && sent.contains(&("tag", p)) && sent.contains(&("icon", p * 64 + 11)), "player {p}");
        }
        assert!(sent.contains(&("score", 2)));
        // 32 * 5 + 2 cells of state a frame at 48: four more frames
        assert!(frames <= 4, "{frames} frames");
        // (it compiles: generated_plugins_compile_with_amxxpc has spawn_burst)
    }

    /// The shared engine guards: effects check the per-frame message budget (the Effect messages
    /// a frame limit), the mode's own files are counted (a warning past 64, an error past 160, and
    /// the plugin refuses past 160 too).
    #[test]
    fn message_budget_and_precache_count() {
        let src = gen::generate(&fun_mode(true), false).source;
        assert!(src.contains("stock bool:lsg_msg_room(const what[], cost = 1, to = 0, lim = 0)"));
        assert!(src.contains("lsg_msg_room(\"Effect Burst\", 2, 0, 48)"), "smoke twice, the default budget");
        assert!(src.contains("lsg_msg_room(\"Effect Burst\", 2, 0, 48)") && src.contains("g_lsgMsgLim = 48"), "the trail sets the budget");
        let src = gen::generate(&ui_nodes_mode(), false).source;
        assert!(src.contains("lsg_msg_room(\"Kill Feed Line\", 1, p)") && src.contains("lsg_msg_state(\"Status Icon\", \"lsg_icon_again\""));
        assert!(src.contains("stock bool:lsg_pre_room(const p[])") && src.contains("if (!lsg_pre_room(p))"));
        // the mode's own files
        let mut g = ui_nodes_mode();
        let n = g.nodes.iter().find(|n| n.kind == "fx.marker").unwrap().clone();
        for i in 0..70 {
            let mut m = n.clone();
            m.id = format!("many{i}");
            m.params.insert("sprite".into(), json!(format!("sprites/many/s{i}.spr")));
            g.nodes.push(m);
        }
        let p = graph::validate(&g);
        assert!(p.iter().any(|p| p.level == "warning" && p.message.contains("files of its own")), "{p:#?}");
        for i in 70..170 {
            let mut m = n.clone();
            m.id = format!("many{i}");
            m.params.insert("sprite".into(), json!(format!("sprites/many/s{i}.spr")));
            g.nodes.push(m);
        }
        assert!(graph::validate(&g).iter().any(|p| p.level == "error" && p.message.contains("the most is 160")));
    }

    pub fn snippet_as_graph(id: &str, v: &Value) -> Graph {
        let mut g = v.clone();
        let o = g.as_object_mut().unwrap();
        o.insert("schema".into(), json!(graph::SCHEMA));
        o.insert("version".into(), json!(2));
        o.insert("mode".into(), json!({"id": format!("sn_{id}").chars().take(24).collect::<String>(), "name": "Snippet"}));
        graph::from_value(g).unwrap()
    }

    /// A library item: the every-node graph's Function "Reward" with what it needs, saved twice
    /// (the same: one version; changed: the next), and hostile text refused.
    #[test]
    fn library_items_are_checked_and_versioned() {
        let every: Value = serde_json::from_str(include_str!("testdata/every_node.graph.json")).unwrap();
        let reward_nodes: Vec<Value> = every["nodes"].as_array().unwrap().iter().filter(|n| n["graph"] == "fa").cloned().collect();
        let ids: Vec<&str> = reward_nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
        let edges: Vec<Value> = every["edges"].as_array().unwrap().iter().filter(|e| ids.contains(&e["from"]["node"].as_str().unwrap()) && ids.contains(&e["to"]["node"].as_str().unwrap())).cloned().collect();
        let item = json!({"schema": LIBRARY_SCHEMA, "version": 1, "name": "Reward", "description": "Gives and counts", "category": "Scoring", "kind": "function", "root": "fa",
            "functions": [every["functions"][0].clone()], "nodes": reward_nodes, "edges": edges});
        check_library_item(&item).unwrap();
        let dir = temp("library");
        assert_eq!(save_library_item_in(&dir, "reward", &item).unwrap(), 1);
        assert_eq!(save_library_item_in(&dir, "reward", &item).unwrap(), 1, "the same again: the same version");
        let mut changed = item.clone();
        changed["description"] = json!("Gives, counts, says");
        assert_eq!(save_library_item_in(&dir, "reward", &changed).unwrap(), 2);
        // never events, never code
        let mut bad = item.clone();
        bad["nodes"].as_array_mut().unwrap().push(json!({"id": "ev", "type": "event.round_started"}));
        assert!(check_library_item(&bad).is_err());
        let mut bad = item.clone();
        bad["name"] = json!("x\"); server_cmd(\"quit");
        assert!(check_library_item(&bad).is_err());
        let mut bad = item.clone();
        bad["root"] = json!("nope");
        assert!(check_library_item(&bad).is_err());
        assert!(save_library_item_in(&dir, "../x", &item).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snippets_check_clean() {
        let all = builtin_snippets();
        assert!(all.len() >= 14);
        for (id, v) in &all {
            check_snippet(v).unwrap_or_else(|e| panic!("{id}: {e}"));
            let g = snippet_as_graph(id, v);
            let p = graph::validate(&g);
            assert!(p.iter().all(|p| p.level != "error"), "{id}: {p:?}");
            let _ = gen::generate(&g, false);
        }
        // hostile text in a snippet is refused
        let mut bad = all[0].1.clone();
        bad["name"] = json!("x */ y");
        assert!(check_snippet(&bad).is_err());
        let mut bad = all[0].1.clone();
        bad["nodes"][0]["params"]["text"] = json!("a\"); server_cmd(\"quit");
        bad["nodes"][0]["type"] = json!("msg.broadcast");
        assert!(check_snippet(&bad).is_err());
        assert!(check_snippet(&json!({"schema": "other"})).is_err());
    }

    #[test]
    fn compiler_messages() {
        let out = "Welcome to the AMX Mod X 1.10.0.5487 Compiler.\nZ:\\a\\studio_x.sma(41) : error 017: undefined symbol \"foo\"\n/x/studio_x.sma(50 -- 52) : warning 217: loose indentation\nloner.inc(3) : error 001: elsewhere\n";
        let d = parse_compiler(out, "studio_x.sma");
        assert_eq!(d, vec![
            Diag { line: 41, level: "error", code: 17, message: "undefined symbol \"foo\"".into() },
            Diag { line: 52, level: "warning", code: 217, message: "loose indentation".into() },
        ]);
    }

    #[test]
    fn graphs_save_list_read() {
        let d = temp("graphs");
        let g = graph::parse(TEMPLATE).unwrap();
        assert!(save_graph(&d, "other", &g).is_err(), "the file is named after the mode");
        save_graph(&d, "rifle_reward", &g).unwrap();
        std::fs::write(d.join("broken.graph.json"), "{").unwrap();
        std::fs::write(d.join("Bad Name.graph.json"), "{}").unwrap();
        let list = list_graphs(&d, None);
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|v| v["id"] == "broken" && v["damaged"] == true));
        assert!(list.iter().any(|v| v["id"] == "rifle_reward" && v["name"] == "Rifle Reward" && v["installed"] == false));
        assert_eq!(read_graph(&d, "rifle_reward").unwrap(), g);
        assert!(read_graph(&d, "../etc").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn installs_match_by_uid_and_clashes_get_a_new_short_id() {
        let game = temp("game_uid");
        let gm = game.join("addons/amxmodx/configs/gamemodes");
        std::fs::create_dir_all(&gm).unwrap();
        std::fs::create_dir_all(game.join("addons/amxmodx/plugins")).unwrap();
        std::fs::write(gm.join("modes.ini"), "; modes\nclassic  \"Classic\"  \"Bombs\"  \"\"  \"w\"\n").unwrap();
        let cfg = game.join("addons/amxmodx/configs");
        let mut a = graph::parse(TEMPLATE).unwrap();
        a.mode.uid = "11111111-1111-4111-8111-111111111111".into();
        // free: its own id
        assert_eq!(install_id(&game, &a), "rifle_reward");
        // a mode not made in the Studio has the id: the next free one
        a.mode.id = "classic".into();
        assert_eq!(install_id(&game, &a), "classic_2");
        let mut inst = a.clone();
        inst.mode.id = "classic_2".into();
        register_mode(&cfg, &inst, &gen::plugin_name("classic_2")).unwrap();
        assert!(std::fs::read_to_string(gm.join("classic_2.plugins.ini")).unwrap().contains("(uid 11111111-1111-4111-8111-111111111111)"));
        // found again by its uid, under whatever short id
        assert_eq!(install_id(&game, &a), "classic_2");
        a.mode.id = "renamed".into();
        assert_eq!(install_id(&game, &a), "classic_2");
        // another mode (another uid) asking for classic_2 gets its own
        let mut b = graph::parse(TEMPLATE).unwrap();
        b.mode.uid = "22222222-2222-4222-9222-222222222222".into();
        b.mode.id = "classic_2".into();
        assert_eq!(install_id(&game, &b), "classic_2_2");
        // a Studio install from before uids is the same mode
        std::fs::write(gm.join("old.plugins.ini"), "; Made with the Game Mode Studio from old.graph.json\nstudio_old.amxx\n").unwrap();
        std::fs::write(gm.join("modes.ini"), "; modes\nclassic  \"Classic\"  \"\"  \"\"  \"\"\nclassic_2  \"C\"  \"\"  \"\"  \"\"\nold  \"Old\"  \"\"  \"\"  \"\"\n").unwrap();
        b.mode.id = "old".into();
        assert_eq!(install_id(&game, &b), "old");
        // ids stay within 24 letters
        b.mode.id = "classic".into();
        b.mode.uid = "33333333-3333-4333-a333-333333333333".into();
        assert_eq!(install_id(&game, &b), "classic_3");
        let _ = std::fs::remove_dir_all(&game);
    }

    #[test]
    fn tests_never_write_into_the_real_game() {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let real = home.join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life");
        assert!(guard_real_game(&real).is_err());
        assert!(uninstall(&real, "anything").unwrap_err().contains("real game"));
        let g = graph::parse(TEMPLATE).unwrap();
        assert!(register_mode(&real.join("cstrike/addons/amxmodx/configs"), &g, "studio_x").unwrap_err().contains("real game"));
        // a throwaway copy is fine
        assert!(guard_real_game(&temp("throwaway")).is_ok());
    }

    #[test]
    fn uids() {
        for _ in 0..50 {
            let u = graph::new_uid();
            assert!(graph::valid_uid(&u), "{u}");
        }
        assert!(!graph::valid_uid("11111111-1111-1111-1111-111111111111"));
        assert!(!graph::valid_uid("11111111-1111-4111-8111-11111111111G"));
        let mut g = graph::parse(TEMPLATE).unwrap();
        g.mode.uid = "nope".into();
        assert!(graph::has_errors(&graph::validate(&g)));
    }

    #[test]
    fn registering_and_removing_a_mode() {
        let game = temp("game");
        let gm = game.join("addons/amxmodx/configs/gamemodes");
        std::fs::create_dir_all(&gm).unwrap();
        std::fs::create_dir_all(game.join("addons/amxmodx/plugins")).unwrap();
        std::fs::write(gm.join("modes.ini"), "; modes\r\nclassic  \"Classic\"  \"Bombs\"  \"\"  \"w\"\r\n").unwrap();
        let mut g = graph::parse(TEMPLATE).unwrap();
        let cfg = game.join("addons/amxmodx/configs");
        register_mode(&cfg, &g, "studio_rifle_reward").unwrap();
        let ini = std::fs::read_to_string(gm.join("modes.ini")).unwrap();
        assert_eq!(ini, "; modes\r\nclassic  \"Classic\"  \"Bombs\"  \"\"  \"w\"\r\nrifle_reward  \"Rifle Reward\"  \"Every kill earns the killer a rifle\"  \"\"  \"\"\r\n");
        assert!(std::fs::read_to_string(gm.join("rifle_reward.plugins.ini")).unwrap().contains("\r\nstudio_rifle_reward.amxx\r\n"));
        assert!(installed(&game, "rifle_reward"));
        // again, renamed: the line is replaced, not added
        g.mode.name = "Rifles".into();
        register_mode(&cfg, &g, "studio_rifle_reward").unwrap();
        let ini = std::fs::read_to_string(gm.join("modes.ini")).unwrap();
        assert_eq!(ini.matches("rifle_reward").count(), 1);
        assert!(ini.contains("\"Rifles\""));
        // a mode the Studio didn't make is never taken over or removed
        g.mode.id = "classic".into();
        assert!(register_mode(&cfg, &g, "studio_classic").unwrap_err().contains("already has a mode"));
        assert!(uninstall(&game, "classic").is_err());
        // (what compiling and the debugger leave behind goes too)
        let left = [
            game.join("addons/amxmodx/scripting/studio_rifle_reward.sma"),
            game.join("addons/amxmodx/scripting/studio/studio_rifle_reward.hash"),
            debug::bp_file(&game, "rifle_reward"),
            debug::cmd_file(&game, "rifle_reward"),
            game.join(settings::manifest_rel("rifle_reward")),
            game.join(settings::values_rel("rifle_reward")),
        ];
        for p in &left {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        uninstall(&game, "rifle_reward").unwrap();
        assert!(left.iter().all(|p| !p.exists()));
        let ini = std::fs::read_to_string(gm.join("modes.ini")).unwrap();
        assert!(!ini.contains("rifle_reward") && ini.contains("classic"));
        assert!(!gm.join("rifle_reward.plugins.ini").exists());
        let _ = std::fs::remove_dir_all(&game);
    }

    /// The generated plugins compile with the real amxxpc (the game's, through CrossOver on a
    /// Mac), with no warnings. Skipped when this computer has no game with AMX Mod X.
    /// The app's limit is the menu's own.
    #[test]
    fn max_modes_matches_the_menu() {
        let sma = include_str!("../../../../../plugins/scripting/gamemodes.sma");
        let n: usize = sma.lines().find_map(|l| l.strip_prefix("#define MAX_MODES")).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap();
        assert_eq!(n, MAX_MODES);
    }

    #[test]
    fn generated_plugins_compile_with_amxxpc() {
        let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
        if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
            eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
            return;
        }
        let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
        let work = temp("amxxpc");
        let inc = work.join("include");
        write_includes(&inc).unwrap();
        let all = include_str!("testdata/all_nodes.graph.json");
        let evil = include_str!("testdata/malicious.graph.json");
        let every = include_str!("testdata/every_node.graph.json");
        let ls = include_str!("../../../../ui/src/views/tools/studio/templates/last-stand.graph.json");
        let br = include_str!("../../../../ui/src/views/tools/studio/templates/battle-royale.graph.json");
        let mk = include_str!("testdata/markers.graph.json");
        let every2 = include_str!("testdata/every_node_2.graph.json");
        let every3 = include_str!("testdata/every_node_3.graph.json");
        let every4 = include_str!("testdata/every_node_4.graph.json");
        let everyfx = include_str!("testdata/every_node_fx.graph.json");
        let everyassets = include_str!("testdata/every_node_assets.graph.json"); // X-fx: files as values
        let mut graphs: Vec<(String, String)> = [("rifle_reward", TEMPLATE), ("all_nodes", all), ("malicious", evil), ("every_node", every), ("last_stand", ls), ("royale", br), ("markers", mk), ("every_node_2", every2), ("every_node_3", every3), ("every_node_4", every4), ("every_node_fx", everyfx), ("every_node_assets", everyassets)].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        for (id, v) in builtin_snippets() {
            graphs.push((format!("sn_{id}"), serde_json::to_string(&snippet_as_graph(&id, &v)).unwrap()));
        }
        graphs.push(("every_node_w1".into(), include_str!("testdata/every_node_w1.graph.json").into())); // W1
        graphs.push(("dialogue".into(), serde_json::to_string(&dialogue_mode()).unwrap()));
        graphs.push(("ui_nodes".into(), serde_json::to_string(&ui_nodes_mode()).unwrap()));
        graphs.push(("spawn_burst".into(), serde_json::to_string(&spawn_burst_mode()).unwrap()));
        graphs.push(("typed_enums".into(), serde_json::to_string(&typed_enums_mode()).unwrap()));
        graphs.push(("colors".into(), serde_json::to_string(&color_mode()).unwrap()));
        graphs.push(("transforms".into(), serde_json::to_string(&transform_mode()).unwrap()));
        graphs.push(("steam_nodes".into(), serde_json::to_string(&steam_mode()).unwrap()));
        graphs.push(("every_node_p5".into(), include_str!("testdata/every_node_p5.graph.json").into())); // P5
        graphs.push(("fun_nodes".into(), serde_json::to_string(&fun_mode(false)).unwrap()));
        graphs.push(("cast_nodes".into(), serde_json::to_string(&cast_mode()).unwrap()));
        graphs.push(("kind_nodes".into(), serde_json::to_string(&kind_mode()).unwrap()));
        // the round clock read with no Round Time Up in the graph (its empty hook didn't compile:
        // the node coverage sweep found it)
        graphs.push(("clock_no_time_up".into(), serde_json::json!({"schema": "loner-strike/graph", "version": 3, "mode": {"id": "clock_no_time_up", "name": "Clock"},
            "nodes": [{"id": "rs", "type": "event.round_started"}, {"id": "left", "type": "clock.left"}, {"id": "p", "type": "debug.print", "params": {"text": "[0]", "chat": false}}],
            "edges": [{"id": "w1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "p", "pin": "exec"}}, {"id": "w2", "from": {"node": "left", "pin": "out"}, "to": {"node": "p", "pin": "value"}}]}).to_string()));
        for (name, text) in graphs.iter().map(|(a, b)| (a.as_str(), b.as_str())) {
            for debug in [false, true] {
                let g = graph::parse(text).unwrap();
                let out = gen::generate(&g, debug);
                let file = format!("studio_{name}{}.sma", if debug { "_dbg" } else { "" });
                let src = work.join(&file);
                std::fs::write(&src, &out.source).unwrap();
                let built = work.join(file.replace(".sma", ".amxx"));
                let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &built).unwrap();
                let diags = parse_compiler(&output, &file);
                assert!(ok, "{file} didn't compile:\n{output}");
                assert!(diags.is_empty(), "{file} has warnings:\n{output}");
            }
        }
        // Compile's mapping with the real compiler: an error inside a node's code names that node;
        // one outside every node says so, with the line
        let g = graph::parse(TEMPLATE).unwrap();
        let out = gen::generate(&g, false);
        let span = out.spans.iter().find(|s| s.to > s.from).expect("a node with a body");
        let mut lines: Vec<String> = out.source.lines().map(str::to_string).collect();
        lines[span.to - 2].push_str("\n\tlsx_no_such_thing()");
        lines.push("public lsx_outside() { lsx_also_missing(); }".into());
        // (the inserted line shifts what follows by one: the generated spans count from the original)
        let text = lines.join("\n");
        let src = work.join("studio_broken.sma");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(&src, &text).unwrap();
        let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &work.join("studio_broken.amxx")).unwrap();
        assert!(!ok, "{output}");
        let diags = parse_compiler(&output, "studio_broken.sma");
        let found = diag_problems(&diags, &out);
        assert!(found.iter().any(|p| p.level == "error" && p.node.as_deref() == Some(span.node.as_str())), "{found:?}\n{output}");
        assert!(found.iter().any(|p| p.level == "error" && p.node.is_none() && p.message.contains("not tied to a node")), "{found:?}\n{output}");
        let _ = std::fs::remove_dir_all(&work);
    }
}
