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
const MAX_MODES: usize = 32;
/// Graph files bigger than this aren't graphs anyone drew.
const MAX_GRAPH_BYTES: u64 = 2 << 20;

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
pub fn check_snippet(v: &Value) -> Result<(), String> {
    if v.get("schema").and_then(Value::as_str) != Some(SNIPPET_SCHEMA) {
        return Err("That isn't a Game Mode Studio snippet.".into());
    }
    let name = v.get("name").and_then(Value::as_str).unwrap_or("");
    let desc = v.get("description").and_then(Value::as_str).unwrap_or("");
    if name.trim().is_empty() || !graph::valid_label(name, 40) || !graph::valid_label(desc, 160) {
        return Err("A snippet needs a name (up to 40 letters, digits, spaces or plain punctuation) and a short description.".into());
    }
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
    let text = serde_json::to_string_pretty(g).map_err(|_| INTERNAL.to_string())?;
    if text.len() as u64 > MAX_GRAPH_BYTES {
        return Err("That graph is too big to save.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the graph ({e})."))?;
    let path = dir.join(format!("{id}.graph.json"));
    // (graphs live in the app's own folder; the safety net keeps a copy where it looks after files)
    crate::safety::before("Game Mode Studio", "save a graph", &path);
    crate::sys::write_atomic(&path, text.as_bytes()).map_err(|e| format!("Couldn't save the graph ({e})."))
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
            message: format!("The compiler said (line {}): {}", d.line, d.message),
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
    let line = format!("{id}  \"{}\"  \"{}\"  \"\"  \"\"", g.mode.name, g.mode.description);
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
    write(gm.join(format!("{id}.plugins.ini")), &format!("; Made with the Game Mode Studio from {id}.graph.json: compile the graph again instead of editing this.{nl}{plugin}.amxx{nl}"))?;
    // the mode's settings (the graph's own; written every compile, after base.cfg each map)
    let mut cfg_text = format!("// {} (Game Mode Studio): its settings, written from the graph each compile. Runs after base.cfg.{nl}", gen::comment_text(&g.mode.name));
    let allowed = graph::allowed_settings();
    for st in g.mode.settings.iter().filter(|s| allowed.contains(&s.cvar.as_str()) && s.value.is_finite()) {
        let v = st.value.clamp(0.0, graph::NUMBER_LIMIT);
        let v = if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") };
        cfg_text.push_str(&format!("{} \"{v}\"{nl}", st.cvar));
    }
    write(gm.join(format!("{id}.cfg")), &cfg_text)?;
    write(ini_path, &text)
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
    for p in [gm.join(format!("{id}.plugins.ini")), gm.join(format!("{id}.cfg")), am.join("plugins").join(format!("{plugin}.amxx"))] {
        if p.is_file() {
            crate::safety::before("Game Mode Studio", "remove a mode", &p);
        }
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

/// Checks, generates, compiles and installs. `{ok, problems, output, plugin}`; ok is false
/// (with problems on the nodes) when the graph or the compiler said no.
pub fn compile(st: &Shared, g: Graph, debug: bool) -> Result<Value, String> {
    let mut problems = graph::validate(&g);
    if graph::has_errors(&problems) {
        return Ok(json!({"ok": false, "problems": problems}));
    }
    let game = local_game(st)?;
    let install = crate::servers::client(st).ok_or(NO_GAME)?;
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
        return Ok(json!({"ok": true, "problems": problems, "output": "Unchanged since the last compile: the installed plugin is used as it is.", "plugin": plugin, "debug": debug, "cached": true, "secs": 0.0}));
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
    Ok(json!({"ok": true, "problems": problems, "output": output, "plugin": plugin, "debug": debug, "secs": (started.elapsed().as_secs_f64() * 10.0).round() / 10.0}))
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
    blocking(move || read_graph(&graphs_dir(), &id).map(|g| json!({"graph": g}))).await
}

async fn api_save(UrlPath(id): UrlPath<String>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let g = graph_of(body.get("graph").cloned().unwrap_or(Value::Null))?;
        save_graph(&graphs_dir(), &id, &g)?;
        Ok(json!({"ok": true}))
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
        let id = g.mode.id.clone();
        let r = compile(&st, g, true)?;
        if r["ok"] == true {
            let game = local_game(&st)?;
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
        uninstall(&local_game(&st)?.dir, &id)?;
        Ok(json!({"ok": true}))
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

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/studio/status", get(api_status))
        .route("/api/tools/studio/graphs", get(api_graphs))
        .route("/api/tools/studio/graph/:id", get(api_graph).post(api_save))
        .route("/api/tools/studio/graph/:id/delete", post(api_delete))
        .route("/api/tools/studio/validate", post(api_validate))
        .route("/api/tools/studio/generate", post(api_generate))
        .route("/api/tools/studio/compile", post(api_compile))
        .route("/api/tools/studio/source", post(api_source))
        .route("/api/tools/studio/snippets", get(api_snippets))
        .route("/api/tools/studio/snippet/:id", post(api_snippet_save))
        .route("/api/tools/studio/snippet/:id/delete", post(api_snippet_delete))
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

    pub fn snippet_as_graph(id: &str, v: &Value) -> Graph {
        let mut g = v.clone();
        let o = g.as_object_mut().unwrap();
        o.insert("schema".into(), json!(graph::SCHEMA));
        o.insert("version".into(), json!(2));
        o.insert("mode".into(), json!({"id": format!("sn_{id}").chars().take(24).collect::<String>(), "name": "Snippet"}));
        graph::from_value(g).unwrap()
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
        uninstall(&game, "rifle_reward").unwrap();
        let ini = std::fs::read_to_string(gm.join("modes.ini")).unwrap();
        assert!(!ini.contains("rifle_reward") && ini.contains("classic"));
        assert!(!gm.join("rifle_reward.plugins.ini").exists());
        let _ = std::fs::remove_dir_all(&game);
    }

    /// The generated plugins compile with the real amxxpc (the game's, through CrossOver on a
    /// Mac), with no warnings. Skipped when this computer has no game with AMX Mod X.
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
        let mut graphs: Vec<(String, String)> = [("rifle_reward", TEMPLATE), ("all_nodes", all), ("malicious", evil), ("every_node", every), ("last_stand", ls), ("royale", br)].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        for (id, v) in builtin_snippets() {
            graphs.push((format!("sn_{id}"), serde_json::to_string(&snippet_as_graph(&id, &v)).unwrap()));
        }
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
        let _ = std::fs::remove_dir_all(&work);
    }
}
