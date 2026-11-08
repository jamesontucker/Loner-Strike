//! Setting up a game: Loner-Strike's foundation, plugins and mods in, and out again.
//!
//! - foundation: Metamod-P, AMX Mod X and ReGameDLL (pinned, sha256-checked)
//! - core: Loner-Strike's plugins, the Game Mode Menu settings, bot styles, F3/F4 keys
//! - components: community modes and add-ons from the catalog
//!
//! Everything written goes through record::Record, so it can all be undone, and the
//! record is the same file the Python installer keeps (either can undo the other).
//! Installs and uninstalls run as one background job at a time; the setup wizard
//! (setup_api.rs) polls it.

pub mod botprofile;
pub mod components;
pub mod core;
pub mod fetch;
pub mod foundation;
pub mod patch;
pub mod pyrandom;
pub mod record;

use crate::gamebanana::MapStore;
use crate::steam::Install;
use record::Record;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// CS 1.6 first, everywhere.
pub const GAMES: &[(&str, &str)] = &[("cstrike", "Counter-Strike 1.6"), ("czero", "Counter-Strike: Condition Zero")];

pub fn game_name(id: &str) -> &'static str {
    GAMES.iter().find(|(g, _)| *g == id).map(|(_, n)| *n).unwrap_or("the game")
}

// ---- small shared helpers ------------------------------------------------------

/// Python's str.splitlines(keepends): \r\n, \n, \r, \v, \f, \x1c-\x1e and \x85 end lines.
pub fn splitlines(text: &str, keepends: bool) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let end_len = match c {
            '\r' => {
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    2
                } else {
                    1
                }
            }
            '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}' => c.len_utf8(),
            _ => continue,
        };
        let next = if c == '\r' && end_len == 2 { i + 2 } else { i + c.len_utf8() };
        out.push(if keepends { &text[start..next] } else { &text[start..i] });
        start = next;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// What the install steps report while they work.
pub trait Progress: Sync {
    /// What's happening now ("Downloading AMX Mod X (2.1 of 8.0 MB)")
    fn detail(&self, text: &str);
    /// How far the current step is (0..1)
    fn fraction(&self, f: f64);
    /// Something the player should know that didn't stop setup (a plugin that didn't compile)
    fn note(&self, text: &str);
}

#[cfg(test)]
pub struct Quiet;
#[cfg(test)]
impl Progress for Quiet {
    fn detail(&self, _: &str) {}
    fn fraction(&self, _: f64) {}
    fn note(&self, _: &str) {}
}

/// Everything an install step needs.
/// A fingerprint of what Loner-Strike ships for a part ("core" is our plugins, the rest
/// are catalog components): a hash of its files. Kept in the install record when it's
/// installed, so after an app update the parts whose files changed can be found and
/// put in again (`outdated`). Computed once per app run.
pub fn shipped_version(res: &Path, id: &str) -> String {
    use sha2::{Digest, Sha256};
    static CACHE: Mutex<Option<std::collections::HashMap<String, String>>> = Mutex::new(None);
    let key = format!("{}|{id}", res.display());
    if let Some(v) = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(&key).cloned()) {
        return v;
    }
    let mut h = Sha256::new();
    let tree = |h: &mut Sha256, root: &Path| {
        let mut files = crate::archive::walk(root);
        files.sort();
        for f in files {
            h.update(f.strip_prefix(root).unwrap_or(&f).to_string_lossy().replace('\\', "/").as_bytes());
            h.update(std::fs::read(&f).unwrap_or_default());
        }
    };
    if id == "core" {
        tree(&mut h, &res.join("plugins"));
    } else if let Some(c) = components::load_catalog(res).ok().and_then(|cat| cat.into_iter().find(|c| c["id"] == id)) {
        h.update(c.to_string().as_bytes());
        if let Some(v) = c["vendor"].as_str() {
            tree(&mut h, &res.join(v));
        }
        // our bot fixes for a downloaded mod: a change to them is an update too
        for p in c["patches"].as_array().into_iter().flatten().filter_map(|p| p.as_str()) {
            h.update(p.as_bytes());
            h.update(std::fs::read(res.join(p)).unwrap_or_default());
        }
    }
    let v = format!("{:x}", h.finalize())[..16].to_string();
    CACHE.lock().unwrap().get_or_insert_with(Default::default).insert(key, v.clone());
    v
}

/// The foundation's version: the ReGameDLL it puts in (a new release of our fork is an update)
pub fn foundation_version() -> String {
    foundation::REGAMEDLL.sha256[..16].to_string()
}

/// Installed parts whose shipped files have changed since they were put in.
pub fn outdated(res: &Path, rec: &Record) -> Vec<String> {
    rec.data
        .components
        .iter()
        .filter(|(id, info)| {
            let want = if id.as_str() == "foundation" { foundation_version() } else { shipped_version(res, id) };
            info.get("version").and_then(|v| v.as_str()) != Some(want.as_str())
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// Catalog mods that are in the game already but weren't put there by Loner-Strike
/// (the player had them before): their plugin is there, without our bot fixes.
/// Installing ours backs theirs up first.
pub fn unmanaged(res: &Path, game_dir: &Path, rec: &Record) -> Vec<String> {
    let plugins = game_dir.join("addons/amxmodx/plugins");
    let Ok(cat) = components::load_catalog(res) else { return vec![] };
    cat.iter()
        // only what Loner-Strike can put in itself (not modes that come with another part)
        .filter(|c| components::installable(c) && matches!(c["kind"].as_str(), Some("mode") | Some("addon")))
        .filter(|c| !c["id"].as_str().is_some_and(|id| rec.data.components.contains_key(id)))
        .filter(|c| {
            let listed = c["plugins"].as_array().into_iter().flatten().filter_map(|p| p.as_str()).map(String::from);
            let built = c["compile"].as_array().into_iter().flatten().filter_map(|b| b["out"].as_str()).map(|o| format!("{o}.amxx"));
            // there, and not a file Loner-Strike put there for something else
            listed.chain(built).any(|p| plugins.join(&p).is_file() && !rec.data.files.contains_key(&format!("addons/amxmodx/plugins/{p}")))
        })
        .filter_map(|c| c["id"].as_str().map(String::from))
        .collect()
}

pub struct Ctx<'a> {
    pub install: &'a Install,
    /// Loner-Strike's files: plugins/, mods/, catalog/
    pub res: &'a Path,
    pub progress: &'a dyn Progress,
    /// GameBanana map store, for modes that come with maps (None: maps are skipped)
    pub store: Option<Arc<MapStore>>,
}

/// A scratch folder for unpacking, removed when dropped.
pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new() -> Result<TempDir, String> {
        let parent = crate::sys::cache_root().join("work");
        crate::sys::make_temp_dir(&parent, "setup-").map(TempDir).map_err(|e| format!("Couldn't make a work folder ({e})."))
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Where Loner-Strike's files are: the app's resources, or the repo checkout in dev.
pub fn resource_root(bundled: Option<PathBuf>) -> Result<PathBuf, String> {
    let ok = |p: &Path| p.join("catalog").join("components.json").is_file();
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(bundled);
    candidates.extend(std::env::var_os("LONER_STRIKE_RESOURCES").map(PathBuf::from));
    if cfg!(debug_assertions) {
        candidates.push(repo_root());
    }
    candidates
        .into_iter()
        .find(|p| ok(p))
        .ok_or_else(|| "Loner-Strike's mod files are missing from the app. Reinstalling the app should fix it.".into())
}

/// The repo checkout this was built from (dev and tests).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

// ---- scanning ----------------------------------------------------------------------

fn game_status(install: &Install, game: &str) -> Value {
    let dir = install.hl.join(game);
    let rec = Record::open(&dir);
    let installed = rec.installed("foundation");
    let comps: Vec<&String> = rec.data.components.keys().filter(|c| !matches!(c.as_str(), "foundation" | "core")).collect();
    let amxx = "addons/amxmodx/dlls/amxmodx_mm.dll";
    let foreign = dir.join(amxx).exists() && !rec.ours(amxx);
    // installed, but a plugin of theirs didn't build (it won't load until it's tried again)
    let unbuilt: Vec<&String> = rec.data.components.iter().filter(|(_, v)| v["unbuilt"] == Value::Bool(true)).map(|(k, _)| k).collect();
    json!({"id": game, "name": game_name(game), "installed": installed, "components": comps, "foreign": foreign, "unbuilt": unbuilt})
}

/// How well bots play a component, from what the catalog says about it.
fn bots_rating(res: &Path, c: &Value) -> Option<&'static str> {
    let desc = c["description"].as_str().unwrap_or("").to_lowercase();
    if desc.contains("no bots") {
        return Some("none");
    }
    if ["bots roll", "bots press", "bots pick", "bots chase"].iter().any(|k| desc.contains(k)) {
        return Some("great");
    }
    let base = components::base_entry(res, c["id"].as_str().unwrap_or(""))?;
    match base["bot_patched"].as_bool() {
        Some(true) => Some("ok"),
        _ => None,
    }
}

fn ordered_games(list: Vec<String>) -> Vec<String> {
    GAMES.iter().map(|(g, _)| g.to_string()).filter(|g| list.contains(g)).collect()
}

pub fn catalog_summary(res: &Path) -> Result<Vec<Value>, String> {
    Ok(components::load_catalog(res)?
        .into_iter()
        .filter(components::installable)
        .map(|c| {
            let mut v = json!({
                "id": c["id"], "name": c["name"], "kind": c["kind"],
                "description": c["description"].as_str().unwrap_or(""),
                "games": ordered_games(components::games_of(&c)),
                "modes": components::strs(&c["modes"]), "addons": components::strs(&c["addons"]),
            });
            if let Some(b) = bots_rating(res, &c) {
                v["bots"] = json!(b);
            }
            v
        })
        .collect())
}

pub fn native_mac() -> bool {
    cfg!(target_os = "macos")
        && dirs::home_dir().map(|h| h.join("Library/Application Support/Steam/steamapps/common/Half-Life").is_dir()).unwrap_or(false)
}

pub fn scan(res: &Path) -> Value {
    let installs: Vec<Value> = crate::steam::find_installs()
        .into_iter()
        .map(|i| {
            let games: Vec<Value> = GAMES.iter().filter(|(g, _)| i.games.iter().any(|x| x == g)).map(|(g, _)| game_status(&i, g)).collect();
            json!({"hl": i.hl, "kind": i.kind, "bottle": i.bottle, "games": games})
        })
        .collect();
    let catalog = catalog_summary(res).unwrap_or_default();
    json!({"installs": installs, "catalog": catalog, "native_mac": native_mac()})
}

// ---- the job ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub id: String,
    pub label: String,
    /// "pending" | "running" | "done" | "error"
    pub state: String,
    pub detail: String,
    pub game: String,
    /// While files go up to a server's machine: {sent, total, rate, left} (bytes, bytes,
    /// bytes a second, seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer: Option<serde_json::Value>,
    #[serde(skip)]
    weight: f64,
    #[serde(skip)]
    frac: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: String,
    /// "idle" | "running" | "done" | "error"
    pub state: String,
    pub kind: String,
    pub game: Option<String>,
    pub steps: Vec<Step>,
    pub progress: f64,
    pub error: Option<String>,
    pub failed_compiles: Vec<String>,
    /// Things worth knowing that didn't stop setup (why a plugin didn't compile...)
    pub notes: Vec<String>,
}

impl JobView {
    pub fn idle() -> JobView {
        JobView { id: String::new(), state: "idle".into(), kind: String::new(), game: None, steps: vec![], progress: 0.0, error: None, failed_compiles: vec![], notes: vec![] }
    }
}

pub struct Job {
    view: Mutex<JobView>,
    current: Mutex<Option<usize>>,
}

impl Job {
    fn new(kind: &str, steps: Vec<Step>) -> Arc<Job> {
        let mut v = JobView::idle();
        v.id = format!("{kind}-{}", (crate::sys::now() * 1000.0) as i64);
        v.state = "running".into();
        v.kind = kind.into();
        v.steps = steps;
        Arc::new(Job { view: Mutex::new(v), current: Mutex::new(None) })
    }

    pub fn view(&self) -> JobView {
        self.view.lock().unwrap().clone()
    }

    fn recompute(v: &mut JobView) {
        let total: f64 = v.steps.iter().map(|s| s.weight).sum();
        let done: f64 = v.steps.iter().map(|s| s.weight * if s.state == "done" { 1.0 } else if s.state == "running" { s.frac } else { 0.0 }).sum();
        v.progress = if total > 0.0 { (done / total).clamp(0.0, 1.0) } else { 1.0 };
    }

    fn start(&self, id: &str) {
        let mut v = self.view.lock().unwrap();
        if let Some(i) = v.steps.iter().position(|s| s.id == id) {
            v.steps[i].state = "running".into();
            v.steps[i].frac = 0.0;
            v.game = Some(v.steps[i].game.clone());
            *self.current.lock().unwrap() = Some(i);
        }
        Self::recompute(&mut v);
    }

    /// The running step's upload progress (None: not uploading).
    pub fn transfer(&self, t: Option<serde_json::Value>) {
        let cur = *self.current.lock().unwrap();
        if let Some(i) = cur {
            self.view.lock().unwrap().steps[i].transfer = t;
        }
    }

    fn finish(&self, id: &str, detail: Option<&str>) {
        let mut v = self.view.lock().unwrap();
        if let Some(s) = v.steps.iter_mut().find(|s| s.id == id) {
            s.transfer = None;
            s.state = "done".into();
            s.detail = detail.unwrap_or("").into();
        }
        *self.current.lock().unwrap() = None;
        Self::recompute(&mut v);
    }

    fn fail(&self, error: &str) {
        let cur = *self.current.lock().unwrap();
        let mut v = self.view.lock().unwrap();
        if v.state == "error" {
            return;
        }
        if let Some(i) = cur {
            v.steps[i].state = "error".into();
            v.steps[i].detail = error.into();
        }
        v.state = "error".into();
        v.error = Some(error.into());
        Self::recompute(&mut v);
    }

    fn complete(&self) {
        let mut v = self.view.lock().unwrap();
        v.state = "done".into();
        v.progress = 1.0;
        v.game = None;
        for s in v.steps.iter_mut().filter(|s| s.state == "pending") {
            s.state = "done".into();
        }
    }

    fn failed_compile(&self, name: &str) {
        let mut v = self.view.lock().unwrap();
        if !v.failed_compiles.iter().any(|n| n == name) {
            v.failed_compiles.push(name.into());
        }
    }
}

impl Progress for Job {
    fn detail(&self, text: &str) {
        let cur = *self.current.lock().unwrap();
        if let Some(i) = cur {
            self.view.lock().unwrap().steps[i].detail = text.into();
        }
    }
    fn fraction(&self, f: f64) {
        let cur = *self.current.lock().unwrap();
        if let Some(i) = cur {
            let mut v = self.view.lock().unwrap();
            v.steps[i].frac = f.clamp(0.0, 0.99);
            Self::recompute(&mut v);
        }
    }
    fn note(&self, text: &str) {
        let game = self.view.lock().unwrap().game.clone();
        let prefix = game.map(|g| format!("{}: ", game_name(&g))).unwrap_or_default();
        self.view.lock().unwrap().notes.push(format!("{prefix}{text}"));
        self.detail(text);
    }
}

fn step(game: &str, id: &str, label: impl Into<String>, weight: f64) -> Step {
    Step { id: format!("{game}/{id}"), label: label.into(), state: "pending".into(), detail: String::new(), game: game.into(), transfer: None, weight, frac: 0.0 }
}

// ---- installing ------------------------------------------------------------------------

/// What an install request asks for, checked before anything changes.
pub struct InstallPlan {
    pub install: Install,
    pub games: Vec<String>,
    pub components: Vec<Value>,
    pub catalog: Vec<Value>,
    steps: Vec<Step>,
}

fn open_record(install: &Install, game: &str) -> Result<Record, String> {
    let rec = Record::open(&install.hl.join(game));
    if rec.broken {
        return Err(format!(
            "Loner-Strike's install record in {} is damaged, so setup won't change anything there.",
            game_name(game)
        ));
    }
    Ok(rec)
}

/// Checks a request and lists the steps it will take.
pub fn plan_install(install: Install, games: &[String], component_ids: &[String], res: &Path, repair: bool) -> Result<InstallPlan, String> {
    if games.is_empty() {
        return Err("Pick a game to set up.".into());
    }
    let games: Vec<String> = GAMES.iter().map(|(g, _)| g.to_string()).filter(|g| games.contains(g)).collect();
    if games.is_empty() {
        return Err("Pick Counter-Strike 1.6 or Condition Zero.".into());
    }
    for g in &games {
        if !install.games.contains(g) {
            return Err(format!("{} isn't installed in {}.", game_name(g), install.hl.display()));
        }
    }
    let catalog = components::load_catalog(res)?;
    let mut chosen = Vec::new();
    for id in component_ids {
        match catalog.iter().find(|c| c["id"].as_str() == Some(id.as_str())) {
            Some(c) if components::installable(c) => chosen.push(c.clone()),
            _ => return Err(format!("\"{id}\" isn't something Loner-Strike can install.")),
        }
    }
    let mut steps = Vec::new();
    for g in &games {
        let rec = open_record(&install, g)?;
        if !install.hl.join(g).join("liblist.gam").is_file() {
            return Err(foundation::no_liblist(&install.hl.join(g)));
        }
        if !rec.installed("foundation") || repair {
            steps.push(step(g, "metamod", "Installing Metamod-P", 1.0));
            steps.push(step(g, "amxmodx", "Installing AMX Mod X", 2.0));
            steps.push(step(g, "regamedll", "Installing ReGameDLL", 1.0));
            steps.push(step(g, "liblist", "Pointing the game at Metamod", 0.2));
        }
        for c in &chosen {
            let id = c["id"].as_str().unwrap_or_default();
            if components::games_of(c).contains(g) && (repair || !rec.installed(id)) {
                steps.push(step(g, id, format!("Installing {}", c["name"].as_str().unwrap_or(id)), 3.0));
            }
        }
        steps.push(step(g, "core-plugins", "Building Loner-Strike's plugins", 2.0));
        steps.push(step(g, "core-configs", "Setting up the game modes", 0.3));
        steps.push(step(g, "core-quake", "Adding the Quake sounds", 0.5));
        steps.push(step(g, "core-bots", "Making the bot styles", 0.3));
        steps.push(step(g, "core-keys", "Adding the F3 and F4 menu keys", 0.1));
    }
    Ok(InstallPlan { install, games, components: chosen, catalog, steps })
}

/// Modes and add-ons the Game Mode Menu should offer: the built-in ones plus
/// those of every installed component.
fn menu_contents(rec: &Record, catalog: &[Value]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut modes: BTreeSet<String> = core::CORE_MODES.iter().map(|s| s.to_string()).collect();
    let mut addons: BTreeSet<String> = core::CORE_ADDONS.iter().map(|s| s.to_string()).collect();
    for c in catalog {
        if rec.installed(c["id"].as_str().unwrap_or_default()) {
            modes.extend(components::strs(&c["modes"]));
            addons.extend(components::strs(&c["addons"]));
        }
    }
    (modes, addons)
}

/// Runs a step, marking it running then done (or failed: the error stops the job).
fn run_step<T>(job: &Job, id: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    job.start(id);
    let r = f();
    match &r {
        Ok(_) => job.finish(id, None),
        Err(e) => job.fail(e),
    }
    r
}

fn install_game(job: &Job, plan: &InstallPlan, ctx: &Ctx, g: &str) -> Result<(), String> {
    let mut rec = open_record(&plan.install, g)?;
    let r = install_game_with(job, plan, ctx, g, &mut rec);
    if r.is_err() {
        // keep track of what was written so far, so it can still be undone
        let _ = rec.save();
    }
    r
}

fn install_game_with(job: &Job, plan: &InstallPlan, ctx: &Ctx, g: &str, rec: &mut Record) -> Result<(), String> {
    let has = |id: &str| plan.steps.iter().any(|s| s.id == format!("{g}/{id}"));
    if has("metamod") {
        rec.begin("foundation");
        let tmp = TempDir::new()?;
        run_step(job, &format!("{g}/metamod"), || foundation::metamod(rec, ctx, &tmp))?;
        run_step(job, &format!("{g}/amxmodx"), || foundation::amxmodx(rec, ctx, &tmp))?;
        run_step(job, &format!("{g}/regamedll"), || foundation::regamedll(rec, ctx, &tmp))?;
        drop(tmp);
        run_step(job, &format!("{g}/liblist"), || foundation::liblist(rec))?;
        rec.done("foundation", json!(foundation_version()))?;
    }
    for c in &plan.components {
        let id = c["id"].as_str().unwrap_or_default();
        if !has(id) {
            continue;
        }
        let ok = run_step(job, &format!("{g}/{id}"), || components::install_component(rec, ctx, c))?;
        if !ok {
            job.failed_compile(c["name"].as_str().unwrap_or(id));
            // remembered, so the Mods page can say so and offer to try again
            if let Some(Value::Object(m)) = rec.data.components.get_mut(id) {
                m.insert("unbuilt".into(), Value::Bool(true));
            }
        }
    }
    let (modes, addons) = menu_contents(rec, &plan.catalog);
    rec.begin("core");
    let ok = run_step(job, &format!("{g}/core-plugins"), || core::plugins(rec, ctx))?;
    if !ok {
        job.failed_compile("Loner-Strike plugins");
    }
    run_step(job, &format!("{g}/core-configs"), || core::configs(rec, ctx, &modes, &addons))?;
    run_step(job, &format!("{g}/core-quake"), || core::quake_sounds(rec, ctx))?;
    // a first setup starts with Quake sounds on (the old pub-server announcer); once the
    // player has switched any add-on, addons.txt is theirs and stays as it is
    if !rec.exists("addons/amxmodx/data/gamemodes/addons.txt") {
        if let Some(ini) = rec.read("addons/amxmodx/configs/gamemodes/addons.ini") {
            if let Ok((on, plugins)) = crate::sounds::write_addons_offline(&ini, "", "quakesounds", true) {
                rec.write("addons/amxmodx/data/gamemodes/addons.txt", &on)?;
                rec.write("addons/amxmodx/configs/plugins-gmaddons.ini", &plugins)?;
            }
        }
    }
    run_step(job, &format!("{g}/core-bots"), || core::bot_styles(rec, ctx, g))?;
    run_step(job, &format!("{g}/core-keys"), || core::binds(rec))?;
    rec.done("core", json!(shipped_version(ctx.res, "core")))
}

/// Runs an install plan on this thread, reporting to job.
pub fn run_install(job: &Job, plan: &InstallPlan, res: &Path, store: Option<Arc<MapStore>>) {
    let ctx = Ctx { install: &plan.install, res, progress: job, store };
    for g in &plan.games {
        if let Err(e) = install_game(job, plan, &ctx, g) {
            job.fail(&e);
            return;
        }
    }
    job.complete();
}

// ---- uninstalling ---------------------------------------------------------------------

pub struct UninstallPlan {
    pub install: Install,
    pub games: Vec<String>,
    /// None: everything
    pub components: Option<Vec<String>>,
    pub catalog: Vec<Value>,
    steps: Vec<Step>,
}

pub fn plan_uninstall(install: Install, games: &[String], components: Option<Vec<String>>, res: &Path) -> Result<UninstallPlan, String> {
    let games: Vec<String> = GAMES.iter().map(|(g, _)| g.to_string()).filter(|g| games.contains(g) && install.games.contains(g)).collect();
    if games.is_empty() {
        return Err("Pick a game to remove Loner-Strike from.".into());
    }
    let catalog = components::load_catalog(res)?;
    // removing the foundation or core means removing everything
    let components = components.filter(|c| !c.iter().any(|id| id == "foundation" || id == "core"));
    let name_of = |id: &str| {
        catalog.iter().find(|c| c["id"].as_str() == Some(id)).and_then(|c| c["name"].as_str()).unwrap_or(id).to_string()
    };
    let mut steps = Vec::new();
    for g in &games {
        let rec = open_record(&install, g)?;
        match &components {
            None => {
                if rec.data.files.is_empty() {
                    continue;
                }
                steps.push(step(g, "remove", "Removing Loner-Strike and putting back the original files", 2.0));
                if !rec.all_map_mods().is_empty() {
                    steps.push(step(g, "maps", "Removing the maps that came with the modes", 1.0));
                }
            }
            Some(ids) => {
                let mut any = false;
                for id in ids {
                    let owned = rec.installed(id) || rec.data.files.values().any(|f| f.owners().contains(id));
                    if owned {
                        any = true;
                        steps.push(step(g, &format!("remove-{id}"), format!("Removing {}", name_of(id)), 1.0));
                    }
                }
                if any && rec.installed("core") {
                    steps.push(step(g, "core-configs", "Updating the game modes", 0.3));
                }
            }
        }
    }
    Ok(UninstallPlan { install, games, components, catalog, steps })
}

fn remove_maps(store: &Option<Arc<MapStore>>, mods: &[i64]) -> usize {
    let Some(store) = store else { return 0 };
    mods.iter().filter(|m| store.uninstall(**m).is_ok()).count()
}

fn uninstall_game(job: &Job, plan: &UninstallPlan, ctx: &Ctx, g: &str) -> Result<(), String> {
    let has = |id: &str| plan.steps.iter().any(|s| s.id == format!("{g}/{id}"));
    let mut rec = open_record(&plan.install, g)?;
    match &plan.components {
        None => {
            if !has("remove") {
                return Ok(());
            }
            let maps = rec.all_map_mods();
            run_step(job, &format!("{g}/remove"), || {
                rec.uninstall(None)?;
                // what the Game Mode Menu and the admin list write themselves, outside the
                // record (left in place only when AMX Mod X was the player's own)
                let configs = plan.install.hl.join(g).join("addons/amxmodx/configs");
                for f in ["plugins-gamemode.ini", "plugins-gmaddons.ini"] {
                    let _ = std::fs::remove_file(configs.join(f));
                }
                let users = configs.join("users.ini");
                if let Ok(text) = crate::sys::read_latin1(&users) {
                    let stripped = crate::admins::strip_users(&text);
                    if stripped != text {
                        let _ = crate::sys::write_latin1(&users, &stripped);
                    }
                }
                Ok(())
            })?;
            if has("maps") {
                job.start(&format!("{g}/maps"));
                let n = remove_maps(&ctx.store, &maps);
                job.finish(&format!("{g}/maps"), Some(&format!("Removed {n} map download{}", if n == 1 { "" } else { "s" })));
            }
        }
        Some(ids) => {
            for id in ids {
                if !has(&format!("remove-{id}")) {
                    continue;
                }
                run_step(job, &format!("{g}/remove-{id}"), || {
                    let maps = rec.map_mods(id);
                    rec.uninstall(Some(std::slice::from_ref(id)))?;
                    if !maps.is_empty() {
                        ctx.progress.detail("Removing the maps that came with it");
                        remove_maps(&ctx.store, &maps);
                        rec.data.maps.remove(id);
                        rec.save()?;
                    }
                    Ok(())
                })?;
            }
            if has("core-configs") {
                let (modes, addons) = menu_contents(&rec, &plan.catalog);
                let version = rec.data.components.get("core").and_then(|c| c.get("version").cloned()).unwrap_or(json!(1));
                rec.begin("core");
                run_step(job, &format!("{g}/core-configs"), || core::configs(&mut rec, ctx, &modes, &addons))?;
                rec.done("core", version)?;
            }
        }
    }
    Ok(())
}

pub fn run_uninstall(job: &Job, plan: &UninstallPlan, res: &Path, store: Option<Arc<MapStore>>) {
    let ctx = Ctx { install: &plan.install, res, progress: job, store };
    for g in &plan.games {
        if let Err(e) = uninstall_game(job, plan, &ctx, g) {
            job.fail(&e);
            return;
        }
    }
    job.complete();
}

// ---- the one job at a time ---------------------------------------------------------------

static CURRENT: Mutex<Option<Arc<Job>>> = Mutex::new(None);

pub fn current() -> JobView {
    CURRENT.lock().unwrap().as_ref().map(|j| j.view()).unwrap_or_else(JobView::idle)
}

fn begin(kind: &str, steps: Vec<Step>) -> Result<Arc<Job>, String> {
    let mut cur = CURRENT.lock().unwrap();
    if cur.as_ref().map(|j| j.view().state == "running").unwrap_or(false) {
        return Err("Setup is already working on something. Wait for it to finish.".into());
    }
    let job = Job::new(kind, steps);
    *cur = Some(job.clone());
    Ok(job)
}

/// Runs a job on its own thread. A panic fails the job instead of leaving it
/// "running" forever (which would block every later install).
fn spawn_job(job: Arc<Job>, f: impl FnOnce(&Job) + Send + 'static) {
    std::thread::spawn(move || {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&job))).is_err() && job.view().state == "running" {
            job.fail("Something went wrong inside setup. Try again, and copy the diagnostics from Settings if it keeps happening.");
        }
    });
}

/// Starts an install in the background. Returns the job id.
pub fn start_install(plan: InstallPlan, res: PathBuf, store: Option<Arc<MapStore>>) -> Result<String, String> {
    let job = begin("install", plan.steps.clone())?;
    let id = job.view().id;
    spawn_job(job, move |job| run_install(job, &plan, &res, store));
    Ok(id)
}

pub fn start_uninstall(plan: UninstallPlan, res: PathBuf, store: Option<Arc<MapStore>>) -> Result<String, String> {
    let job = begin("uninstall", plan.steps.clone())?;
    let id = job.view().id;
    spawn_job(job, move |job| run_uninstall(job, &plan, &res, store));
    Ok(id)
}

/// A job that runs on the caller's thread (tests).
#[cfg(test)]
pub fn job_for_test(kind: &str, steps: &[Step]) -> Arc<Job> {
    Job::new(kind, steps.to_vec())
}

#[cfg(test)]
impl InstallPlan {
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}
#[cfg(test)]
impl UninstallPlan {
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

// ---- dedicated servers (servers.rs) ---------------------------------------------------------

pub fn server_step(game: &str, id: &str, label: impl Into<String>, weight: f64) -> Step {
    step(game, id, label, weight)
}

/// Runs f as the one setup job (its steps show in the same progress view).
pub fn start_job(kind: &str, steps: Vec<Step>, f: impl FnOnce(&Job) -> Result<(), String> + Send + 'static) -> Result<String, String> {
    let job = begin(kind, steps)?;
    let id = job.view().id;
    spawn_job(job, move |job| match f(job) {
        Ok(()) => job.complete(),
        Err(e) => job.fail(&e),
    });
    Ok(id)
}

impl Job {
    /// Runs one step of a custom job ("<game>/<id>").
    pub fn run<T>(&self, id: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        run_step(self, id, f)
    }

    /// Renames a step while it runs ("Copying de_cbble to Alone Together (2 of 5)").
    pub fn relabel(&self, id: &str, label: impl Into<String>) {
        let mut v = self.view.lock().unwrap();
        if let Some(s) = v.steps.iter_mut().find(|s| s.id == id) {
            s.label = label.into();
        }
    }

    /// Adds steps before the step `id` once it's known what they'll be.
    pub fn insert_before(&self, id: &str, steps: Vec<Step>) {
        let mut v = self.view.lock().unwrap();
        let at = v.steps.iter().position(|s| s.id == id).unwrap_or(v.steps.len());
        v.steps.splice(at..at, steps);
        Self::recompute(&mut v);
    }
}

impl InstallPlan {
    pub fn step_list(&self) -> Vec<Step> {
        self.steps.clone()
    }
}

/// Runs an install plan as part of a bigger job (no complete() at the end).
pub fn install_into(job: &Job, plan: &InstallPlan, res: &Path, store: Option<Arc<MapStore>>) -> Result<(), String> {
    let ctx = Ctx { install: &plan.install, res, progress: job, store };
    for g in &plan.games {
        install_game(job, plan, &ctx, g)?;
    }
    Ok(())
}

// ---- test helpers ------------------------------------------------------------------------

/// Runs python3 -I -c script and returns its stdout, or None if Python isn't available
/// (comparison tests then skip).
#[cfg(test)]
pub fn test_python(script: &str) -> Option<String> {
    let out = std::process::Command::new("python3").arg("-c").arg(script).output().ok()?;
    if !out.status.success() {
        panic!("python failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Some(out.stdout.iter().map(|&b| b as char).collect())
}

#[cfg(test)]
pub fn py_repr_float(f: f64) -> String {
    format!("{f:?}")
}

#[cfg(test)]
mod tests;
