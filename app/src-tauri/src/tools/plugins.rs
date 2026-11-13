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
}

/// A plugin's name (the file name without .sma): what goes in plugins.ini and the logs.
pub fn stem(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

/// The file's path, checked: a safe name, directly inside the folder, and (if it's there) a
/// real file whose folder is that folder (no link out of it).
pub fn resolve(dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !safe_file(name) {
        return Err(BAD_NAME.into());
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

/// Loner-Strike's own plugins in the game folder are put back by setup: open them, but
/// save changes as a copy in My plugins.
fn read_only(f: Folder, name: &str) -> Option<&'static str> {
    match f {
        Folder::Include => Some("Include files are the game's own. Save a copy in My plugins to change one."),
        Folder::Game if crate::setup::core::OUR_PLUGINS.iter().any(|p| stem(name).eq_ignore_ascii_case(p)) => {
            Some("This is one of Loner-Strike's own plugins; setup puts it back as it was. Save a copy in My plugins to change it.")
        }
        Folder::Game if name.to_ascii_lowercase().ends_with(".inc") => Some("Save include files in My plugins."),
        _ => None,
    }
}

fn modified(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

fn list(g: &Game, f: Folder) -> Vec<Value> {
    let dir = folder_dir(g, f);
    let mut files: Vec<Value> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let meta = e.metadata().ok()?;
            let wanted = match f {
                Folder::Include => name.to_ascii_lowercase().ends_with(".inc"),
                _ => true,
            };
            (wanted && meta.is_file() && safe_file(&name)).then(|| json!({"name": name, "size": meta.len(), "modified": modified(&meta), "readOnly": read_only(f, &name).is_some()}))
        })
        .collect();
    if f == Folder::Include {
        for (inc, text) in APP_INCS {
            if !files.iter().any(|v| v["name"] == *inc) {
                files.push(json!({"name": inc, "size": text.len(), "modified": 0, "readOnly": true}));
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

pub fn read(g: &Game, f: Folder, name: &str) -> Result<Value, String> {
    let path = resolve(&folder_dir(g, f), name)?;
    if !path.is_file() {
        if let Some((_, text)) = APP_INCS.iter().find(|(inc, _)| f == Folder::Include && *inc == name) {
            return Ok(json!({"name": name, "folder": f.id(), "text": text, "encoding": "utf8", "readOnly": read_only(f, name)}));
        }
        return Err("That file isn't there any more.".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("Couldn't read it ({e})."))?;
    if bytes.len() > MAX_SOURCE {
        return Err("That file is too big to open here (over 2 MB).".into());
    }
    let (text, encoding) = decode(&bytes);
    Ok(json!({"name": name, "folder": f.id(), "text": text, "encoding": encoding, "readOnly": read_only(f, name)}))
}

/// Writes next to the file as <name>.part, then renames it over the file.
fn write_part(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make the folder ({e})."))?;
    }
    let part = path.with_file_name(format!("{}.part", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    crate::safety::before("Plugin Studio", &format!("Saved {}", path.file_name().unwrap_or_default().to_string_lossy()), path);
    std::fs::write(&part, bytes).map_err(|e| format!("Couldn't save it ({e})."))?;
    std::fs::rename(&part, path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("Couldn't save it ({e}).")
    })
}

pub fn save(g: &Game, f: Folder, name: &str, text: &str, encoding: &str, create: bool) -> Result<Value, String> {
    if let Some(why) = read_only(f, name) {
        return Err(why.into());
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
    let bytes: Vec<u8> = if encoding == "latin1" && text.chars().all(|c| (c as u32) < 256) { text.chars().map(|c| c as u8).collect() } else { text.as_bytes().to_vec() };
    write_part(&path, &bytes)?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true, "name": name, "folder": f.id(), "size": meta.len(), "modified": modified(&meta)}))
}

pub fn delete(g: &Game, f: Folder, name: &str) -> Result<Value, String> {
    if f != Folder::Mine {
        return Err("Only files in My plugins can be deleted here.".into());
    }
    let path = resolve(&folder_dir(g, f), name)?;
    crate::safety::before("Plugin Studio", &format!("Deleted {name}"), &path);
    std::fs::remove_file(&path).map_err(|e| format!("Couldn't delete it ({e})."))?;
    Ok(json!({"ok": true}))
}

// ---- the word list (include files) ----------------------------------------------------

struct ApiCache {
    key: String,
    value: Arc<Value>,
}

/// Every include in the game's include folder (and loner.inc), read once per change.
pub fn api(g: &Game) -> Value {
    static CACHE: OnceLock<Mutex<Option<ApiCache>>> = OnceLock::new();
    let dir = folder_dir(g, Folder::Include);
    let mut files: Vec<(String, PathBuf, u64, u64)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let meta = e.metadata().ok()?;
            (meta.is_file() && name.to_ascii_lowercase().ends_with(".inc") && meta.len() < 4 << 20).then(|| (name, e.path(), meta.len(), modified(&meta)))
        })
        .collect();
    // amxmodx.inc and its friends first, so their definitions win over a mod's copy
    files.sort_by_key(|(n, ..)| (!matches!(n.as_str(), "amxmodx.inc" | "amxconst.inc" | "core.inc" | "float.inc" | "string.inc"), n.to_lowercase()));
    let key = format!("{}|{}", dir.display(), files.iter().map(|(n, _, s, m)| format!("{n}:{s}:{m}")).collect::<Vec<_>>().join(","));
    let cache = CACHE.get_or_init(Default::default);
    if let Some(c) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if c.key == key {
            return (*c.value).clone();
        }
    }
    let mut texts: Vec<(String, String)> = files.iter().filter_map(|(n, p, ..)| std::fs::read(p).ok().map(|b| (n.clone(), decode(&b).0))).collect();
    for (inc, text) in APP_INCS {
        if !texts.iter().any(|(n, _)| n == inc) {
            texts.push((inc.to_string(), text.to_string()));
        }
    }
    let symbols = inc::parse_all(&texts);
    let value = Arc::new(json!({"includes": texts.iter().map(|(n, _)| n.trim_end_matches(".inc")).collect::<Vec<_>>(), "symbols": symbols}));
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
    let exe = amxx_dir(g).join("scripting").join("amxxpc.exe");
    if !exe.is_file() {
        return Err("Your game has no AMX Mod X compiler yet. Install the basics (Mods page) first.".into());
    }
    let install = install_of(g).ok_or("Couldn't find your game's install to run the compiler in.")?;
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
            write_part(&file, text.as_bytes())?;
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
    let g = st.local_game().ok_or(NO_GAME)?;
    if g.remote.is_some() || g.dedicated {
        return Err("The Plugin Studio only tries plugins in your own game on this computer.".into());
    }
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
    let mut names = trying(g);
    let dest = am.join("plugins").join(format!("{stem}.amxx"));
    let ours = crate::setup::core::OUR_PLUGINS.iter().any(|p| p.eq_ignore_ascii_case(&stem));
    if ours || (dest.exists() && !names.contains(&stem)) {
        return Err(format!("Your game already has a plugin called {stem}.amxx that the Plugin Studio didn't put there. Save yours under another name to try it."));
    }
    let bytes = std::fs::read(amxx).map_err(|e| format!("Couldn't read the compiled plugin ({e})."))?;
    write_part(&dest, &bytes)?;
    if !names.contains(&stem) {
        names.push(stem);
    }
    set_trying(g, &names)
}

pub fn remove_trial(g: &Game, stem: &str) -> Result<(), String> {
    let mut names = trying(g);
    if !names.iter().any(|n| n == stem) {
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
            out.push(json!({"at": at, "text": body.trim_end(), "error": errors || lower.contains("error") || lower.contains("failed")}));
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
                {"id": "mine", "label": "My plugins", "path": folder_dir(&g, Folder::Mine), "files": list(&g, Folder::Mine)},
                {"id": "game", "label": "Game's plugins", "path": folder_dir(&g, Folder::Game), "files": list(&g, Folder::Game)},
                {"id": "include", "label": "Include files", "path": folder_dir(&g, Folder::Include), "files": list(&g, Folder::Include)},
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
        let reloaded = r.restart && restart_map(&g).unwrap_or(false);
        let message = if reloaded { format!("{stem} is out of your game. The map is restarting.") } else { format!("{stem} is out of your game. It stops with the next map.") };
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

    /// The real include files: `LS_TEST_GAME=<Half-Life folder> cargo test real_includes -- --nocapture`
    #[test]
    fn real_includes() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let g = Game::with_app_dir(Path::new(&hl), "cstrike", &std::env::temp_dir().join("ls-studio-real"));
        let v = api(&g);
        let syms = v["symbols"].as_array().unwrap();
        let find = |n: &str| syms.iter().find(|s| s["name"] == n).unwrap_or_else(|| panic!("no {n}")).clone();
        println!("{} includes, {} symbols", v["includes"].as_array().unwrap().len(), syms.len());
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
        std::fs::write(inc.join("loner.inc"), LONER_INC).unwrap();
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
