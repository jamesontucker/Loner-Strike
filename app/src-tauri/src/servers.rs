//! Dedicated servers: Valve's Half-Life Dedicated Server (HLDS), downloaded with
//! SteamCMD and set up like the player's own game, each with its own modes,
//! settings, maps, admins and port. The panel switches between "My game" and
//! any server (AppState::set_game).
//!
//! On a Mac the server is the Windows HLDS, run through CrossOver like the game
//! (Valve makes no Mac server); its folder sits in the game's bottle, under
//! C:\Loner-Strike Servers. SteamCMD itself runs natively (Valve's Mac build).

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::setup::{self, Job};
use crate::steam::Install;
use crate::sys;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get as route_get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CROSSOVER_WINE: &str = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";
const FOLDER: &str = "Loner-Strike Servers";
/// The player's own game hosts on 27015, so servers start above it and never clash
const FIRST_PORT: u16 = 27016;
/// Restarts after a crash before giving up (within CRASH_WINDOW)
const MAX_RESTARTS: usize = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(600);

// ---- the list of servers ---------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// "cstrike" or "czero"
    pub game: String,
    pub port: u16,
    /// The server's folder (hlds.exe and the game folder inside)
    pub dir: PathBuf,
    /// "crossover" (Mac) or "windows", like the game it was made from
    pub kind: String,
    pub bottle: Option<String>,
    /// "lan": this network only; "unlisted": anyone with the address; "listed": in Steam's server browser
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default = "default_map")]
    pub map: String,
    #[serde(default = "default_slots")]
    pub maxplayers: u32,
    /// false until setup has finished
    #[serde(default)]
    pub ready: bool,
    /// a server folder the player already had: never deleted, only forgotten
    #[serde(default)]
    pub imported: bool,
}

fn default_visibility() -> String {
    "unlisted".into()
}
fn default_map() -> String {
    "de_dust2".into()
}
fn default_slots() -> u32 {
    20
}

impl Entry {
    pub fn install(&self) -> Install {
        Install { hl: self.dir.clone(), kind: self.kind.clone(), bottle: self.bottle.clone(), games: vec![self.game.clone()], dedicated: true }
    }

    fn hlds(&self) -> PathBuf {
        self.dir.join("hlds.exe")
    }

    /// The panel's RCON password, shared with the plugin (game.rs writes it).
    fn rcon(&self) -> Option<crate::rcon::Rcon> {
        let pw = std::fs::read_to_string(self.dir.join(&self.game).join("addons/amxmodx/data/gamemodes/panel_rcon.txt")).ok()?;
        Some(crate::rcon::Rcon::new(pw.trim().to_string(), self.port))
    }
}

fn list_file() -> PathBuf {
    sys::app_dir().join("servers.json")
}

pub fn load() -> Vec<Entry> {
    let mut list: Vec<Entry> = std::fs::read_to_string(list_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // servers made before they started at 27016 shared the game's port: move them
    if list.iter().any(|e| !e.imported && e.port < FIRST_PORT) {
        for i in 0..list.len() {
            if !list[i].imported && list[i].port < FIRST_PORT {
                list[i].port = free_port(&list);
            }
        }
        let _ = save(&list);
    }
    list
}

/// A running server on `port`, by name (the game's own port is 27015).
pub fn running_on(port: u16) -> Option<String> {
    load().into_iter().find(|e| e.port == port && running(&e.id)).map(|e| e.name)
}

fn save(list: &[Entry]) -> Result<(), String> {
    let _ = std::fs::create_dir_all(sys::app_dir());
    let tmp = list_file().with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(list).unwrap_or_default()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, list_file()).map_err(|e| e.to_string())
}

static LIST_LOCK: Mutex<()> = Mutex::new(());

fn edit(f: impl FnOnce(&mut Vec<Entry>)) -> Result<(), String> {
    let _g = LIST_LOCK.lock().unwrap();
    let mut list = load();
    f(&mut list);
    save(&list)
}

pub fn get(id: &str) -> Option<Entry> {
    load().into_iter().find(|e| e.id == id)
}

/// Servers count as installs for setup (adding modes later, repairing).
pub fn find_install(hl: &Path) -> Option<Install> {
    load().into_iter().find(|e| e.dir == hl && e.ready).map(|e| e.install())
}

/// Where servers live: inside the game's CrossOver bottle on a Mac (so wine sees
/// them as C:\Loner-Strike Servers), the app's local data folder on Windows.
fn root_for(client: &Install) -> Result<PathBuf, String> {
    if client.kind == "crossover" {
        let s = client.hl.to_string_lossy().to_string();
        let i = s.find("/drive_c/").ok_or("The game isn't in a CrossOver bottle.")?;
        Ok(PathBuf::from(&s[..i + 8]).join(FOLDER))
    } else {
        Ok(dirs::data_local_dir().ok_or("Couldn't find the app data folder.")?.join("Loner-Strike").join("Servers"))
    }
}

/// "Friday Night Frags" -> "friday-night-frags" (unique among the servers).
fn slug(name: &str, taken: &[Entry]) -> String {
    let mut base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(32);
    if base.is_empty() {
        base = "server".into();
    }
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|e| e.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

fn free_port(taken: &[Entry]) -> u16 {
    (FIRST_PORT..FIRST_PORT + 100).find(|p| !taken.iter().any(|e| e.port == *p && e.port >= FIRST_PORT)).unwrap_or(FIRST_PORT)
}

fn safe_map(m: &str) -> bool {
    !m.is_empty() && m.len() <= 31 && m.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
}

// ---- SteamCMD ----------------------------------------------------------------------

fn steamcmd_dir() -> PathBuf {
    sys::app_dir().join("steamcmd")
}

fn steamcmd_exe() -> PathBuf {
    if cfg!(windows) {
        steamcmd_dir().join("steamcmd.exe")
    } else {
        steamcmd_dir().join("steamcmd.sh")
    }
}

/// Valve's SteamCMD: the Mac build on a Mac (it downloads the Windows server
/// fine), the Windows one on Windows. Valve doesn't publish checksums; it's
/// fetched over HTTPS from Valve's CDN and updates itself on first run.
fn ensure_steamcmd(job: &Job) -> Result<(), String> {
    if steamcmd_exe().is_file() {
        return Ok(());
    }
    let url = if cfg!(windows) {
        "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip"
    } else if cfg!(target_os = "macos") {
        "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_osx.tar.gz"
    } else {
        "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz"
    };
    let archive = setup::fetch::download(url, None, "SteamCMD", job)?;
    setup::fetch::unpack(&archive, &steamcmd_dir())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for f in ["steamcmd.sh", "steamcmd"] {
            let _ = std::fs::set_permissions(steamcmd_dir().join(f), std::fs::Permissions::from_mode(0o755));
        }
    }
    if steamcmd_exe().is_file() {
        Ok(())
    } else {
        Err("SteamCMD didn't unpack properly. Try again.".into())
    }
}

/// "Update state (0x61) downloading, progress: 59.91 (542930389 / 906205254)" -> (0.5991, "542.9 of 906.2 MB")
fn parse_progress(line: &str) -> Option<(f64, String)> {
    let i = line.find("progress: ")?;
    let rest = &line[i + 10..];
    let pct: f64 = rest.split_whitespace().next()?.parse().ok()?;
    let (a, b) = rest.split_once('(')?.1.split_once(')')?.0.split_once('/')?;
    let (got, total): (f64, f64) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    let what = if line.contains("downloading") { "Downloading" } else if line.contains("verifying") { "Checking" } else { "Preparing" };
    Some((pct / 100.0, format!("{what} the server ({:.0} of {:.0} MB)", got / 1e6, total / 1e6)))
}

/// Downloads (or updates) HLDS for `game` into `dir`. SteamCMD often needs more
/// than one go for app 90, so it retries until the game folder is there.
fn download_hlds(job: &Job, dir: &Path, game: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut last = String::new();
    for attempt in 1..=4 {
        let mut cmd = sys::command(steamcmd_exe());
        cmd.current_dir(steamcmd_dir())
            .arg("+@sSteamCmdForcePlatformType")
            .arg("windows")
            .arg("+force_install_dir")
            .arg(dir)
            .arg("+login")
            .arg("anonymous")
            .arg("+app_set_config")
            .arg("90")
            .arg("mod")
            .arg(game)
            .arg("+app_update")
            .arg("90")
            .arg("validate")
            .arg("+quit")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| format!("SteamCMD didn't start ({e})."))?;
        sys::tie_to_app(&child);
        let mut ok = false;
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).split(b'\n').map_while(Result::ok) {
                let line = String::from_utf8_lossy(&line).trim().to_string();
                if let Some((f, text)) = parse_progress(&line) {
                    job_progress(job, f, &text);
                } else if line.contains("Success! App '90' fully installed") {
                    ok = true;
                } else if line.starts_with("Error!") || line.starts_with("ERROR") {
                    last = line.clone();
                }
            }
        }
        let _ = child.wait();
        sys::untie(child.id());
        if ok && dir.join("hlds.exe").is_file() && dir.join(game).join("liblist.gam").is_file() {
            return Ok(());
        }
        setup::Progress::detail(job, &format!("Trying again ({attempt} of 4)…"));
    }
    Err(if last.is_empty() {
        "SteamCMD couldn't download the server. Check your internet connection and free disk space (it needs about 1 GB), then try again.".into()
    } else {
        format!("SteamCMD couldn't download the server: {last}")
    })
}

fn job_progress(job: &Job, f: f64, text: &str) {
    setup::Progress::fraction(job, f);
    setup::Progress::detail(job, text);
}

/// Copies everything in `from` into `to` (files that are already there are kept
/// unless `overwrite`). On a Mac's APFS disk, copies are clones and take no space.
fn copy_tree(from: &Path, to: &Path, overwrite: bool, filter: &dyn Fn(&Path) -> bool) -> Result<u64, String> {
    let mut n = 0;
    for f in crate::archive::walk(from) {
        let Ok(rel) = f.strip_prefix(from) else { continue };
        if !filter(rel) {
            continue;
        }
        let dest = to.join(rel);
        if dest.exists() && !overwrite {
            continue;
        }
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make {} ({e}).", d.display()))?;
        }
        std::fs::copy(&f, &dest).map_err(|e| format!("Couldn't copy {} ({e}).", rel.display()))?;
        n += 1;
    }
    Ok(n)
}

// ---- making a server -----------------------------------------------------------------

#[derive(Deserialize)]
pub struct NewServer {
    pub name: String,
    pub game: String,
    /// "local" (copy My game), a server id, or "fresh"
    #[serde(default)]
    pub template: String,
    /// for "fresh": what to install
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub visibility: Option<String>,
}

/// The game folder a template copies from, and the components installed there.
fn template_source(template: &str, game: &str, client: &Install, catalog: &[Value]) -> Option<(PathBuf, Vec<String>)> {
    let dir = match template {
        "local" | "" => client.hl.join(game),
        "fresh" => return None,
        id => {
            let e = get(id)?;
            if e.game != game {
                return None;
            }
            e.dir.join(&e.game)
        }
    };
    let rec = setup::record::Record::open(&dir);
    let comps = catalog
        .iter()
        .filter_map(|c| c["id"].as_str())
        .filter(|id| rec.installed(id))
        .map(String::from)
        .collect();
    Some((dir, comps))
}

/// Game Mode Menu settings worth carrying over: presets, saved settings, admins…
/// not the panel's password, live status or logs.
fn settings_filter(rel: &Path) -> bool {
    let name = rel.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    !matches!(name.as_str(), "panel_rcon.txt" | "status.json" | "friends.json" | "friends.cfg") && !name.ends_with(".log")
}

/// Maps and what custom maps need (textures, models, sounds, sprites, overviews).
fn content_filter(rel: &Path) -> bool {
    let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).unwrap_or_default();
    let root_wad = rel.components().count() == 1 && rel.extension().map(|e| e.eq_ignore_ascii_case("wad")).unwrap_or(false);
    root_wad || matches!(first.as_str(), "maps" | "models" | "sound" | "sprites" | "gfx" | "overviews" | "resource")
}

pub fn create(req: NewServer, client: Install, res: PathBuf) -> Result<(String, String), String> {
    let name = req.name.trim().chars().filter(|c| !c.is_control() && *c != '"').take(48).collect::<String>();
    if name.is_empty() {
        return Err("Give the server a name.".into());
    }
    if !["cstrike", "czero"].contains(&req.game.as_str()) {
        return Err("Pick Counter-Strike 1.6 or Condition Zero.".into());
    }
    if client.kind == "crossover" && !Path::new(CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The server runs through it, like the game.".into());
    }
    let catalog = setup::components::load_catalog(&res)?;
    let (source, components) = match template_source(&req.template, &req.game, &client, &catalog) {
        Some((dir, comps)) => (Some(dir), comps),
        None if req.template == "fresh" => (None, req.components.clone()),
        None => return Err("That template isn't there any more, or it's for the other game.".into()),
    };
    let root = root_for(&client)?;
    let list = load();
    let id = slug(&name, &list);
    let entry = Entry {
        id: id.clone(),
        name: name.clone(),
        game: req.game.clone(),
        port: free_port(&list),
        dir: root.join(&id),
        kind: client.kind.clone(),
        bottle: client.bottle.clone(),
        visibility: req.visibility.filter(|v| ["lan", "unlisted", "listed"].contains(&v.as_str())).unwrap_or_else(default_visibility),
        map: default_map(),
        maxplayers: default_slots(),
        ready: false,
        imported: false,
    };
    let base = root.join(".base");
    let g = req.game.clone();
    let steps = vec![
        setup::server_step(&g, "srv-steamcmd", "Getting SteamCMD (Valve's server downloader)", 1.0),
        setup::server_step(&g, "srv-download", "Downloading the server from Valve", 8.0),
        setup::server_step(&g, "srv-copy", "Making the server's folder", 1.0),
        setup::server_step(&g, "srv-settings", "Copying settings and admins", 0.3),
        setup::server_step(&g, "srv-maps", "Copying maps", 2.0),
        setup::server_step(&g, "srv-done", "Finishing up", 0.1),
    ];
    edit(|l| l.push(entry.clone()))?;
    let job = setup::start_job("server", steps, move |job| {
        job.run(&format!("{g}/srv-steamcmd"), || ensure_steamcmd(job))?;
        job.run(&format!("{g}/srv-download"), || download_hlds(job, &base, &g))?;
        job.run(&format!("{g}/srv-copy"), || {
            // the engine and this game's folder (not the other game's)
            let other = if g == "cstrike" { "czero" } else { "cstrike" };
            let keep_other = g == "czero"; // Condition Zero uses CS 1.6's files too
            copy_tree(&base, &entry.dir, false, &|rel: &Path| {
                let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy().to_string()).unwrap_or_default();
                first != "steamapps" && (keep_other || first != other)
            })
            .map(|_| ())
        })?;
        // Loner-Strike itself, with the same modes as the template
        let plan = setup::plan_install(entry.install(), &[g.clone()], &components, &res, false)?;
        job.insert_before(&format!("{g}/srv-settings"), plan.step_list());
        let store = Arc::new(crate::gamebanana::MapStore::new(&entry.dir, &entry.dir.join(&g), &sys::app_dir(), Arc::new(Mutex::new(None))));
        setup::install_into(job, &plan, &res, Some(store))?;
        let data = entry.dir.join(&g).join("addons/amxmodx/data/gamemodes");
        job.run(&format!("{g}/srv-settings"), || {
            if let Some(src) = &source {
                copy_tree(&src.join("addons/amxmodx/data/gamemodes"), &data, true, &settings_filter)?;
            }
            let _ = std::fs::create_dir_all(&data);
            std::fs::write(data.join("friends.json"), json!({"name": entry.name, "password": ""}).to_string()).map_err(|e| e.to_string())?;
            // empty ban lists, so the server doesn't complain it has none
            for f in ["banned.cfg", "listip.cfg"] {
                let p = entry.dir.join(&g).join(f);
                if !p.exists() {
                    let _ = std::fs::write(p, "");
                }
            }
            std::fs::write(data.join("friends.cfg"), format!("// Written by Loner-Strike\nhostname \"{}\"\n", entry.name)).map_err(|e| e.to_string())
        })?;
        job.run(&format!("{g}/srv-maps"), || match &source {
            Some(src) => copy_tree(src, &entry.dir.join(&g), false, &content_filter).map(|_| ()),
            None => Ok(()),
        })?;
        job.run(&format!("{g}/srv-done"), || edit(|l| l.iter_mut().filter(|e| e.id == entry.id).for_each(|e| e.ready = true)))
    })?;
    Ok((id, job))
}

/// Re-downloads Valve's files into the base copy and refreshes the server's engine files.
pub fn update_files(e: Entry) -> Result<String, String> {
    if running(&e.id) {
        return Err("Stop the server first.".into());
    }
    let g = e.game.clone();
    let base = e.dir.parent().ok_or("Bad server folder.")?.join(".base");
    let steps = vec![
        setup::server_step(&g, "srv-steamcmd", "Getting SteamCMD", 0.5),
        setup::server_step(&g, "srv-download", "Checking Valve's server files", 8.0),
        setup::server_step(&g, "srv-copy", "Updating the server", 1.0),
    ];
    setup::start_job("server", steps, move |job| {
        job.run(&format!("{g}/srv-steamcmd"), || ensure_steamcmd(job))?;
        job.run(&format!("{g}/srv-download"), || download_hlds(job, &base, &g))?;
        // engine files only: never the game folders, which hold Loner-Strike's changes
        job.run(&format!("{g}/srv-copy"), || {
            copy_tree(&base, &e.dir, true, &|rel: &Path| rel.components().count() == 1 && rel.extension().map(|x| x != "gam").unwrap_or(true)).map(|_| ())
        })
    })
}

// ---- bringing in a server the player already runs ------------------------------------

/// `key "value"` or `key value` from a .cfg
fn cfg_value(text: &str, key: &str) -> Option<String> {
    text.lines().rev().find_map(|l| {
        let l = l.split("//").next().unwrap_or("").trim();
        let rest = l.strip_prefix(key)?;
        if !rest.starts_with([' ', '\t']) {
            return None;
        }
        Some(rest.trim().trim_matches('"').to_string())
    })
}

/// Registers an existing HLDS folder (nothing in it is moved or deleted) and brings
/// its name, password and SteamID admins into the app.
pub fn import(dir: &Path) -> Result<Value, String> {
    let dir = dir.canonicalize().map_err(|_| "That folder isn't there.".to_string())?;
    if !dir.join("hlds.exe").is_file() {
        return Err("That folder has no hlds.exe. Pick the folder the dedicated server is in.".into());
    }
    let game = ["cstrike", "czero"]
        .into_iter()
        .find(|g| dir.join(g).join("liblist.gam").is_file() && (g == &"cstrike" || !dir.join("cstrike/addons").is_dir()))
        .ok_or("No Counter-Strike 1.6 or Condition Zero folder in there.")?;
    let (kind, bottle) = if cfg!(windows) {
        ("windows".to_string(), None)
    } else {
        let b = bottle_of(&dir).ok_or("On a Mac the server has to be inside a CrossOver bottle (its drive_c), so it can run.")?;
        ("crossover".to_string(), Some(b))
    };
    let list = load();
    if list.iter().any(|e| e.dir == dir) {
        return Err("That server is in the list already.".into());
    }
    let cfg = sys::read_latin1(&dir.join(game).join("server.cfg")).unwrap_or_default();
    let name = cfg_value(&cfg, "hostname").filter(|n| !n.is_empty()).unwrap_or_else(|| "Imported server".into());
    let name: String = name.chars().filter(|c| !c.is_control() && *c != '"').take(48).collect();
    let password = cfg_value(&cfg, "sv_password").unwrap_or_default();
    let id = slug(&name, &list);
    let entry = Entry {
        id: id.clone(),
        name: name.clone(),
        game: game.into(),
        port: free_port(&list),
        dir: dir.clone(),
        kind,
        bottle,
        visibility: if cfg_value(&cfg, "sv_lan").as_deref() == Some("1") { "lan".into() } else { "listed".into() },
        map: default_map(),
        maxplayers: default_slots(),
        ready: true,
        imported: true,
    };
    // name and password as the Friends tab keeps them (it writes friends.cfg only when
    // something changes there, so the server's own server.cfg keeps working meanwhile)
    let data = dir.join(game).join("addons/amxmodx/data/gamemodes");
    let _ = std::fs::create_dir_all(&data);
    if !data.join("friends.json").exists() {
        let _ = std::fs::write(data.join("friends.json"), json!({"name": name, "password": password}).to_string());
    }
    let admins = crate::admins::import_users(&dir.join(game), &data);
    edit(|l| l.push(entry.clone()))?;
    let loner = setup::record::Record::open(&dir.join(game)).installed("core");
    Ok(json!({"ok": true, "id": id, "admins": admins, "loner": loner}))
}

// ---- running servers ------------------------------------------------------------------

struct Proc {
    child: Child,
    started: Instant,
    /// stopped on purpose (don't restart it)
    stopping: bool,
}

#[derive(Default)]
struct Runs {
    procs: HashMap<String, Proc>,
    crashes: HashMap<String, Vec<Instant>>,
    /// why a server isn't running ("crashed 3 times")
    errors: HashMap<String, String>,
}

static RUNS: Mutex<Option<Runs>> = Mutex::new(None);

fn with_runs<T>(f: impl FnOnce(&mut Runs) -> T) -> T {
    let mut r = RUNS.lock().unwrap();
    f(r.get_or_insert_with(Runs::default))
}

pub fn running(id: &str) -> bool {
    with_runs(|r| r.procs.contains_key(id))
}

fn launch_args(e: &Entry) -> Vec<String> {
    let mut a: Vec<String> = vec!["-console".into(), "-game".into(), e.game.clone(), "-port".into(), e.port.to_string(), "-condebug".into()];
    if e.visibility != "listed" {
        a.push("-nomaster".into()); // no heartbeats to Steam's server list
    }
    let map = if safe_map(&e.map) { e.map.clone() } else { default_map() };
    a.extend([
        "+sv_lan".into(),
        if e.visibility == "lan" { "1" } else { "0" }.into(),
        "+maxplayers".into(),
        e.maxplayers.clamp(2, 32).to_string(),
        "+map".into(),
        map,
    ]);
    a
}

/// .../Bottles/<bottle>/drive_c/Some Folder/hlds.exe -> C:\Some Folder\hlds.exe
fn windows_path(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    let i = s.find("/drive_c/")?;
    Some(format!("C:\\{}", s[i + 9..].replace('/', "\\")))
}

/// The CrossOver bottle a path is in.
fn bottle_of(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    let i = s.find("/Bottles/")?;
    let rest = &s[i + 9..];
    rest.find("/drive_c/").map(|j| rest[..j].to_string())
}

fn spawn(e: &Entry) -> Result<Child, String> {
    if !e.hlds().is_file() {
        return Err("The server's files are missing. Delete it and make it again.".into());
    }
    let log = std::fs::File::create(e.dir.join("loner-strike-server.log")).map_err(|err| err.to_string())?;
    let mut cmd = if e.kind == "crossover" {
        let mut c = sys::command(CROSSOVER_WINE);
        let win = windows_path(&e.hlds()).ok_or("The server has to be inside a CrossOver bottle's C: drive.")?;
        c.arg("--bottle").arg(e.bottle.as_deref().unwrap_or("Steam")).arg(win);
        c
    } else {
        sys::command(e.hlds())
    };
    cmd.args(launch_args(e))
        .current_dir(&e.dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|err| err.to_string())?)
        .stderr(log);
    let child = cmd.spawn().map_err(|err| format!("The server didn't start ({err})."))?;
    sys::tie_to_app(&child);
    Ok(child)
}

pub fn start(id: &str) -> Result<(), String> {
    let e = get(id).ok_or("That server isn't there any more.")?;
    if !e.ready {
        return Err("This server isn't finished setting up.".into());
    }
    if running(id) {
        return Ok(());
    }
    if let Some(other) = load().iter().find(|o| o.id != e.id && o.port == e.port && running(&o.id)) {
        return Err(format!("\"{}\" is using port {} already. Stop it first, or change this server's port.", other.name, e.port));
    }
    let child = spawn(&e)?;
    with_runs(|r| {
        r.errors.remove(id);
        r.procs.insert(id.into(), Proc { child, started: Instant::now(), stopping: false });
    });
    watch();
    Ok(())
}

/// Asks the server to quit (RCON), then ends it if it hasn't after a few seconds.
pub fn stop(id: &str) {
    let e = get(id);
    let child = with_runs(|r| {
        r.procs.get_mut(id).map(|p| p.stopping = true);
        r.procs.remove(id).map(|p| p.child)
    });
    let Some(mut child) = child else { return };
    if let Some(rcon) = e.as_ref().and_then(|e| e.rcon()) {
        let _ = rcon.send("quit");
    }
    for _ in 0..50 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            sys::untie(child.id());
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    sys::stop_child(child, Duration::from_secs(3));
}

pub fn stop_all() {
    let ids: Vec<String> = with_runs(|r| r.procs.keys().cloned().collect());
    for id in ids {
        stop(&id);
    }
}

/// Restarts servers that crash (up to MAX_RESTARTS times in CRASH_WINDOW).
fn watch() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(3));
            let died: Vec<String> = with_runs(|r| {
                let mut out = Vec::new();
                for (id, p) in r.procs.iter_mut() {
                    if !p.stopping && matches!(p.child.try_wait(), Ok(Some(_))) {
                        out.push(id.clone());
                    }
                }
                for id in &out {
                    if let Some(p) = r.procs.remove(id) {
                        sys::untie(p.child.id());
                    }
                }
                out
            });
            for id in died {
                let recent = with_runs(|r| {
                    let list = r.crashes.entry(id.clone()).or_default();
                    list.retain(|t| t.elapsed() < CRASH_WINDOW);
                    list.push(Instant::now());
                    list.len()
                });
                if recent > MAX_RESTARTS {
                    with_runs(|r| r.errors.insert(id.clone(), "The server keeps crashing, so it was left off. Check its console.".into()));
                } else if let Err(e) = start(&id) {
                    with_runs(|r| r.errors.insert(id.clone(), e));
                }
            }
        });
    });
}

/// The last lines of the server's console (qconsole.log, from -condebug).
fn console(e: &Entry) -> Vec<String> {
    let mut lines = Vec::new();
    for p in [e.dir.join("qconsole.log"), e.dir.join(&e.game).join("qconsole.log"), e.dir.join("loner-strike-server.log")] {
        let Ok(mut f) = std::fs::File::open(&p) else { continue };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let _ = f.seek(SeekFrom::Start(len.saturating_sub(48 * 1024)));
        let mut buf = Vec::new();
        let _ = f.read_to_end(&mut buf);
        lines = crate::rcon::latin1(&buf).lines().map(String::from).collect();
        if !lines.is_empty() {
            break;
        }
    }
    let skip = lines.len().saturating_sub(300);
    lines.split_off(skip).into_iter().map(|l| hide_passwords(&l)).collect()
}

/// The server logs every RCON command with its password ("rcon 1234 "secret" status"),
/// and rcon_password / sv_password changes too: blank them out before showing the console.
fn hide_passwords(line: &str) -> String {
    let lower = line.to_lowercase();
    for key in ["rcon ", "rcon_password", "sv_password", "password "] {
        if let Some(i) = lower.find(key) {
            if let Some(q) = line[i..].find('"').map(|q| i + q) {
                if let Some(end) = line[q + 1..].find('"').map(|e| q + 1 + e) {
                    return format!("{}\"••••\"{}", &line[..q], hide_passwords(&line[end + 1..]));
                }
            }
        }
    }
    line.to_string()
}

fn view(e: &Entry) -> Value {
    let loner = setup::record::Record::open(&e.dir.join(&e.game)).installed("core");
    let (state, uptime, error) = with_runs(|r| match r.procs.get(&e.id) {
        Some(p) => ("running", p.started.elapsed().as_secs(), None),
        None => (if r.errors.contains_key(&e.id) { "crashed" } else { "stopped" }, 0, r.errors.get(&e.id).cloned()),
    });
    let state = if e.ready { state } else { "setting-up" };
    json!({"id": e.id, "name": e.name, "game": e.game, "port": e.port, "visibility": e.visibility, "map": e.map,
           "maxplayers": e.maxplayers, "ready": e.ready, "state": state, "uptime": uptime, "error": error,
           "imported": e.imported, "loner": loner, "dir": e.dir})
}

// ---- which game the panel controls -----------------------------------------------------

static GAMES: Mutex<Option<HashMap<String, Arc<Game>>>> = Mutex::new(None);

/// The Game for "local" (the player's own game) or a server id. Kept, so switching
/// back and forth keeps caches and running installs.
pub fn game_for(target: &str, local: impl FnOnce() -> Option<Game>) -> Option<Arc<Game>> {
    let mut games = GAMES.lock().unwrap();
    let games = games.get_or_insert_with(HashMap::new);
    if let Some(g) = games.get(target) {
        return Some(g.clone());
    }
    let g = if target == "local" {
        local()?
    } else {
        let e = get(target).filter(|e| e.ready)?;
        Game::on_port(&e.dir, &e.game, e.port, true)
    };
    let g = Arc::new(g);
    games.insert(target.into(), g.clone());
    Some(g)
}

/// Switches the panel to the player's own CS 1.6 or Condition Zero, live.
pub fn use_local(st: &Shared, hl: &str, game: &str) {
    crate::app_api::set_local_game(hl, game);
    forget("local");
    let s = crate::app_api::load_settings();
    st.set_game(game_for("local", || crate::app_api::pick_game(&s)));
}

fn forget(target: &str) {
    if let Some(g) = GAMES.lock().unwrap().as_mut() {
        g.remove(target);
    }
}

// ---- API -----------------------------------------------------------------------------

fn forbidden() -> (StatusCode, Json<Value>) {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn answer(r: Result<Value, String>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

/// The player's own game install (servers are made from it and joined with it).
fn client(st: &Shared) -> Option<Install> {
    let local = st.local_game()?;
    crate::steam::find_installs().into_iter().find(|i| i.hl == local.hl)
}

async fn api_list() -> Json<Value> {
    let list: Vec<Value> = load().iter().map(view).collect();
    Json(json!({"servers": list, "steamcmd": steamcmd_exe().is_file()}))
}

#[derive(Deserialize)]
struct Select {
    id: String,
}

async fn api_select(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<Select>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    // "local:cstrike" / "local:czero": the player's own game
    if let Some(game) = req.id.strip_prefix("local:") {
        let game = game.to_string();
        let st2 = st.clone();
        let ok = tokio::task::spawn_blocking(move || {
            let installs = crate::steam::find_installs();
            let cur = st2.game().filter(|g| !g.dedicated).map(|g| g.hl.clone());
            let pick = installs
                .iter()
                .filter(|i| i.games.contains(&game))
                .max_by_key(|i| Some(&i.hl) == cur.as_ref())
                .map(|i| i.hl.to_string_lossy().to_string());
            pick.map(|hl| use_local(&st2, &hl, &game)).is_some()
        })
        .await
        .unwrap_or(false);
        return answer(if ok { Ok(json!({"ok": true})) } else { Err("That game isn't installed.".into()) });
    }
    answer(Err("Pick CS 1.6 or Condition Zero.".into()))
}

async fn api_create(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<NewServer>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let res = crate::setup_api::resources_for(&st);
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || {
        let client = client(&st2).ok_or("Set up the game first: servers are made from it.")?;
        create(req, client, res?).map(|(id, job)| json!({"ok": true, "id": id, "job": job}))
    })
    .await
    .unwrap_or_else(|_| Err("Something went wrong.".into()));
    answer(r)
}

#[derive(Deserialize, Default)]
struct Change {
    name: Option<String>,
    visibility: Option<String>,
    map: Option<String>,
    maxplayers: Option<u32>,
    port: Option<u16>,
}

async fn api_change(UrlPath(id): UrlPath<String>, headers: HeaderMap, Json(req): Json<Change>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = (|| {
        let e = get(&id).ok_or("That server isn't there any more.")?;
        if let Some(v) = &req.visibility {
            if !["lan", "unlisted", "listed"].contains(&v.as_str()) {
                return Err("Unknown visibility.".to_string());
            }
        }
        if let Some(m) = &req.map {
            if !safe_map(m) {
                return Err("That isn't a map name.".into());
            }
        }
        if let Some(p) = req.port {
            if !(1024..=65000).contains(&p) || load().iter().any(|o| o.id != id && o.port == p) {
                return Err("Pick a port from 1024 to 65000 that no other server uses.".into());
            }
            if running(&id) && p != e.port {
                return Err("Stop the server to change its port.".into());
            }
        }
        let name = req.name.as_ref().map(|n| n.trim().chars().filter(|c| !c.is_control() && *c != '"').take(48).collect::<String>()).filter(|n| !n.is_empty());
        edit(|l| {
            if let Some(e) = l.iter_mut().find(|e| e.id == id) {
                if let Some(n) = &name {
                    e.name = n.clone();
                }
                if let Some(v) = &req.visibility {
                    e.visibility = v.clone();
                }
                if let Some(m) = &req.map {
                    e.map = m.clone();
                }
                if let Some(m) = req.maxplayers {
                    e.maxplayers = m.clamp(2, 32);
                }
                if let Some(p) = req.port {
                    e.port = p;
                }
            }
        })?;
        if req.port.is_some() {
            forget(&id);
        }
        Ok(json!({"ok": true, "restart": running(&id)}))
    })();
    answer(r)
}

async fn api_action(State(st): State<Shared>, UrlPath((id, action)): UrlPath<(String, String)>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        match action.as_str() {
            "start" => start(&id).map(|_| json!({"ok": true})),
            "stop" => {
                stop(&id);
                Ok(json!({"ok": true}))
            }
            "restart" => {
                stop(&id);
                start(&id).map(|_| json!({"ok": true}))
            }
            "update" => update_files(get(&id).ok_or("That server isn't there any more.")?).map(|job| json!({"ok": true, "job": job})),
            "join" => {
                let e = get(&id).ok_or("That server isn't there any more.")?;
                let client = client(&st2).ok_or("Couldn't find the game to join with.")?;
                let pw = std::fs::read_to_string(e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes/friends.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v["password"].as_str().map(String::from))
                    .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)));
                let mut args = Vec::new();
                if let Some(p) = pw {
                    args.extend(["+password".to_string(), p]);
                }
                args.extend(["+connect".to_string(), format!("127.0.0.1:{}", e.port)]);
                let appid = if e.game == "cstrike" { "10" } else { "80" };
                crate::steam::launch(&client.hl, appid, &args).map(|_| json!({"ok": true}))
            }
            "delete" => {
                let e = get(&id).ok_or("That server isn't there any more.")?;
                stop(&id);
                // only ever a folder we made: <servers root>/<id>
                let ok_dir = !e.imported
                    && e.dir.file_name().map(|n| n.to_string_lossy() == e.id).unwrap_or(false)
                    && e.dir.parent().and_then(|p| p.file_name()).map(|n| n == FOLDER || n == "Servers").unwrap_or(false);
                if ok_dir && e.dir.is_dir() {
                    std::fs::remove_dir_all(&e.dir).map_err(|err| format!("Couldn't delete the server's folder ({err})."))?;
                }
                edit(|l| l.retain(|x| x.id != id))?;
                forget(&id);
                Ok(json!({"ok": true}))
            }
            _ => Err("Unknown action.".into()),
        }
    })
    .await
    .unwrap_or_else(|_| Err("Something went wrong.".into()));
    answer(r)
}

async fn api_console(UrlPath(id): UrlPath<String>) -> Json<Value> {
    Json(match get(&id) {
        Some(e) => json!({"lines": tokio::task::spawn_blocking(move || console(&e)).await.unwrap_or_default()}),
        None => json!({"lines": []}),
    })
}

#[derive(Deserialize)]
struct ConsoleCmd {
    command: String,
}

/// Any console command, for the server's owner at this computer only (not phones).
async fn api_console_cmd(UrlPath(id): UrlPath<String>, headers: HeaderMap, Json(req): Json<ConsoleCmd>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let cmd = req.command.trim().to_string();
    if cmd.is_empty() || cmd.len() > 200 || cmd.chars().any(|c| c.is_control()) || cmd.starts_with("rcon_password") {
        return answer(Err("That command can't be sent.".into()));
    }
    let r = tokio::task::spawn_blocking(move || {
        let e = get(&id).ok_or("That server isn't there any more.")?;
        let rcon = e.rcon().ok_or("The server isn't set up yet.")?;
        rcon.send(&cmd).map(|out| json!({"reply": out})).map_err(|_| "The server didn't answer. Is it running?".to_string())
    })
    .await
    .unwrap_or_else(|_| Err("Something went wrong.".into()));
    answer(r)
}

#[derive(Deserialize)]
struct ImportReq {
    dir: String,
}

async fn api_import(headers: HeaderMap, Json(req): Json<ImportReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || import(Path::new(req.dir.trim()))).await.unwrap_or_else(|_| Err("Something went wrong.".into()));
    answer(r)
}

/// The system's folder picker (the app window only).
async fn api_pick_folder(State(st): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err("No window.".into())) };
    let picked = tokio::task::spawn_blocking(move || app.dialog().file().set_title("Pick the dedicated server's folder (with hlds.exe)").blocking_pick_folder())
        .await
        .ok()
        .flatten();
    answer(Ok(json!({"dir": picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().to_string())})))
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/servers", route_get(api_list).post(api_create))
        .route("/api/servers/select", post(api_select))
        .route("/api/servers/:id", post(api_change))
        .route("/api/servers/:id/console", route_get(api_console))
        .route("/api/servers/:id/:action", post(api_action))
}

/// Only for the app window, not the phone link: a free-form console, and importing a
/// folder (which can then be started, so it runs whatever hlds.exe is in it).
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/servers/import", post(api_import))
        .route("/api/servers/:id/console/send", post(api_console_cmd))
        .route("/api/servers/pick-folder", post(api_pick_folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines() {
        let (f, t) = parse_progress(" Update state (0x61) downloading, progress: 59.91 (542930389 / 906205254)").unwrap();
        assert!((f - 0.5991).abs() < 1e-6);
        assert_eq!(t, "Downloading the server (543 of 906 MB)");
        assert!(parse_progress("Success! App '90' fully installed.").is_none());
    }

    #[test]
    fn slugs_are_unique() {
        let e = |id: &str| Entry {
            id: id.into(), name: String::new(), game: "cstrike".into(), port: 27015, dir: PathBuf::new(), kind: String::new(),
            bottle: None, visibility: default_visibility(), map: default_map(), maxplayers: 20, ready: true, imported: false,
        };
        assert_eq!(slug("Friday Night Frags!", &[]), "friday-night-frags");
        assert_eq!(slug("Friday Night Frags", &[e("friday-night-frags")]), "friday-night-frags-2");
        assert_eq!(slug("¿?", &[]), "server");
        assert_eq!(free_port(&[e("a")]), 27016, "the game's 27015 is never handed out");
        let mut b = e("b");
        b.port = 27016;
        assert_eq!(free_port(&[b]), 27017);
    }

    #[test]
    fn listing_flags() {
        let mut e = Entry {
            id: "x".into(), name: "x".into(), game: "cstrike".into(), port: 27016, dir: PathBuf::new(), kind: String::new(),
            bottle: None, visibility: "lan".into(), map: "de_dust2;quit".into(), maxplayers: 99, ready: true, imported: false,
        };
        let a = launch_args(&e).join(" ");
        assert!(a.contains("-nomaster") && a.contains("+sv_lan 1") && a.contains("+maxplayers 32") && a.contains("+map de_dust2") && !a.contains("quit"));
        e.visibility = "listed".into();
        let a = launch_args(&e).join(" ");
        assert!(!a.contains("-nomaster") && a.contains("+sv_lan 0") && a.contains("-port 27016"));
    }

    #[test]
    fn paths_and_cfg() {
        let p = Path::new("/Users/x/Library/Application Support/CrossOver/Bottles/Steam/drive_c/HLDS/hlds.exe");
        assert_eq!(windows_path(p).unwrap(), "C:\\HLDS\\hlds.exe");
        assert_eq!(bottle_of(p).unwrap(), "Steam");
        let cfg = "hostname \"My \"Server\"\nsv_password secret // comment\nsv_passwordx 1\n";
        assert_eq!(cfg_value(cfg, "hostname").unwrap(), "My \"Server");
        assert_eq!(cfg_value(cfg, "sv_password").unwrap(), "secret");
    }

    #[test]
    fn console_hides_passwords() {
        assert_eq!(hide_passwords(r#"rcon 350334764 "6QO7jG8" quit"#), r#"rcon 350334764 "••••" quit"#);
        assert_eq!(hide_passwords(r#"sv_password "abc""#), r#"sv_password "••••""#);
        assert_eq!(hide_passwords("Connection to Steam servers successful."), "Connection to Steam servers successful.");
    }

    #[test]
    fn filters() {
        assert!(content_filter(Path::new("maps/de_x.bsp")) && content_filter(Path::new("halflife.wad")) && content_filter(Path::new("sound/a/b.wav")));
        assert!(!content_filter(Path::new("addons/amxmodx/x.amxx")) && !content_filter(Path::new("dlls/mp.dll")) && !content_filter(Path::new("sub/x.wad")));
        assert!(settings_filter(Path::new("presets/a.cfg")) && !settings_filter(Path::new("panel_rcon.txt")) && !settings_filter(Path::new("friends.json")));
    }
}
