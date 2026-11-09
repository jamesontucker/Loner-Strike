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

pub(crate) const CROSSOVER_WINE: &str = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";
/// Where servers went before they moved into Loner-Strike/Servers (still recognised)
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
    /// HLTV spectator relay (hltv.rs)
    #[serde(default)]
    pub hltv: crate::hltv::Settings,
    /// a daily restart at a set time (the Schedule tab of Server extras)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<DailyRestart>,
    /// On another machine (a Linux VPS) over SSH; `dir` is then this computer's copy of
    /// its Loner-Strike files (where plugins are compiled), uploaded after changes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<crate::remote::RemoteInfo>,
    /// How it was set up (template and mods), so a setup that stopped can run again
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_with: Option<SetupWith>,
    /// The address it listens on (empty: every address the machine has)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub listen_ip: String,
    /// More start options the player added (checked by `extra_args`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extra_args: String,
    /// Bots keep playing with nobody on (bot_join_after_player 0). None: what the
    /// visibility suggests (on for "listed"), until the host picks (see bots_when_empty)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bots_when_empty: Option<bool>,
    /// On the Loner-Strike server list (ls_list 1 in friends.cfg; see browser.rs)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ls_listed: bool,
    /// A domain name (or IP) pointing at the server, with an optional :port, shown and
    /// shared in place of its IP (display only: the server itself doesn't use it)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_address: Option<String>,
    /// The website status page on a Linux machine (webstatus.rs)
    #[serde(default, skip_serializing_if = "crate::webstatus::Settings::is_unset")]
    pub web_status: crate::webstatus::Settings,
    /// Something only a restart (or a map change) brings in is waiting for one: new start
    /// options, HLTV settings, plugins or maps sent to the machine. Kept here, so the page
    /// still says so after a reload; cleared when it starts or stops.
    #[serde(default, deserialize_with = "pending_from", skip_serializing_if = "PendingRestart::is_clear")]
    pub pending_restart: PendingRestart,
    /// On another machine: its service still has the old start options (writing it there
    /// failed). Written again before the next start, and by the background sync.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub service_stale: bool,
    /// on this computer: keeps running after the app closes (None: the app-wide setting
    /// from before this was per server)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_running: Option<bool>,
    /// a notification when someone joins (None: the app-wide setting from before this was
    /// per server)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_joins: Option<bool>,
    /// a notification when someone helping run it (shared control) makes a big change
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_helpers: Option<bool>,
    /// Loner-Strike's updates (plugins, the bot brain) put in by themselves once nobody is
    /// playing (None: on)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
}

/// What's waiting for the server to restart (or, for sounds, the map to change).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct PendingRestart {
    /// plugins, modules, models or maps sent to the machine
    pub files: bool,
    /// start options (Server settings)
    pub options: bool,
    /// HLTV's settings
    pub hltv: bool,
    /// round-end music and sounds: the next map takes them
    pub sounds: bool,
    /// the map being played when `sounds` was set (another one means it changed)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sounds_map: String,
}

impl PendingRestart {
    pub fn is_clear(&self) -> bool {
        !(self.files || self.options || self.hltv || self.sounds)
    }

    /// What the page gets: {files, options, hltv, sounds}.
    fn view(&self) -> Value {
        json!({"files": self.files, "options": self.options, "hltv": self.hltv, "sounds": self.sounds})
    }
}

/// Which change waits for a restart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pending {
    Files,
    Options,
    Hltv,
    Sounds,
}

/// A pending restart as saved (an older `true` counts as files).
fn pending_from<'de, D: serde::Deserializer<'de>>(d: D) -> Result<PendingRestart, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Saved {
        Old(bool),
        Now(PendingRestart),
    }
    Ok(match Saved::deserialize(d)? {
        Saved::Old(b) => PendingRestart { files: b, ..Default::default() },
        Saved::Now(p) => p,
    })
}

const PUBLIC_ADDRESS_HELP: &str = "Enter an address like play.example.com or play.example.com:27016.";

/// Checks a public address the host typed: a host name or IPv4 address, optionally with
/// :port. Host names come back lowercase; empty clears it (None).
pub(crate) fn clean_public_address(text: &str) -> Result<Option<String>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let bad = || PUBLIC_ADDRESS_HELP.to_string();
    if t.len() > 253 || !t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':')) {
        return Err(bad());
    }
    let (host, port) = match t.split_once(':') {
        Some((h, p)) => {
            // digits only (no sign), 1 to 65535
            if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
                return Err(bad());
            }
            let n = p.parse::<u16>().ok().filter(|n| *n > 0).ok_or_else(bad)?;
            (h, Some(n))
        }
        None => (t, None),
    };
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let ip = host.parse::<std::net::Ipv4Addr>().is_ok();
    let label_ok = |l: &str| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-');
    let labels: Vec<&str> = host.split('.').collect();
    // the last label of a name is never all digits (that's a broken IP address like 1.2.3.999)
    let name = labels.iter().all(|l| label_ok(l)) && !labels.last().unwrap().chars().all(|c| c.is_ascii_digit());
    if host.is_empty() || !(ip || name) {
        return Err(bad());
    }
    Ok(Some(match port {
        Some(p) => format!("{host}:{p}"),
        None => host,
    }))
}

/// The host part of a public address (without any :port).
fn public_host(a: &str) -> &str {
    a.split_once(':').map_or(a, |(h, _)| h)
}

pub(crate) fn default_visibility() -> String {
    "unlisted".into()
}
pub(crate) fn default_map() -> String {
    "de_dust2".into()
}
pub(crate) fn default_slots() -> u32 {
    20
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SetupWith {
    pub template: String,
    #[serde(default)]
    pub components: Vec<String>,
    /// the join password picked in the New server form
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
}

impl Entry {
    pub fn install(&self) -> Install {
        Install { hl: self.dir.clone(), kind: self.kind.clone(), bottle: self.bottle.clone(), games: vec![self.game.clone()], dedicated: true }
    }

    fn hlds(&self) -> PathBuf {
        self.dir.join("hlds.exe")
    }

    /// The panel's RCON password, shared with the plugin (game.rs writes it).
    pub(crate) fn rcon(&self) -> Option<crate::rcon::Rcon> {
        let pw = std::fs::read_to_string(self.dir.join(&self.game).join("addons/amxmodx/data/gamemodes/panel_rcon.txt")).ok()?;
        Some(match &self.remote {
            Some(r) => crate::rcon::Rcon::over_ssh(
                r.clone(),
                format!("{}/{}/addons/amxmodx/data/gamemodes/panel_rcon.txt", crate::remote::server_dir(&self.id), self.game),
                pw.trim().to_string(),
                self.port,
            ),
            None => crate::rcon::Rcon::at(pw.trim().to_string(), &self.local_host(), self.port),
        })
    }

    /// Where on this computer the server answers: its listen address, or 127.0.0.1.
    pub(crate) fn local_host(&self) -> String {
        match self.bind_ip() {
            Some(ip) if ip != "0.0.0.0" => ip,
            _ => "127.0.0.1".into(),
        }
    }

    /// The address the server is started on (+ip): the one set in its settings, else, for a
    /// Windows server on this computer that needs Steam (not "This network"), the network
    /// adapter the internet goes through. Left to itself hlds can take a virtual adapter
    /// (WSL, Hyper-V, VirtualBox), can't reach Steam from there, and then answers no one.
    pub(crate) fn bind_ip(&self) -> Option<String> {
        if !self.listen_ip.is_empty() {
            return Some(self.listen_ip.clone());
        }
        if !cfg!(windows) || self.remote.is_some() || self.visibility == "lan" {
            return None;
        }
        // the one it was started on, so RCON and Join still find it if the network changes
        let pinned = PINNED.lock().unwrap().as_ref().and_then(|m| m.get(&self.id).cloned());
        pinned.or_else(route_ip)
    }

    /// Fixes the automatic address for a start of this server (see bind_ip).
    fn pin_address(&self) {
        if !cfg!(windows) || self.remote.is_some() {
            return;
        }
        let now = route_ip();
        let before = {
            let mut m = PINNED.lock().unwrap();
            let m = m.get_or_insert_with(HashMap::new);
            match &now {
                Some(ip) => m.insert(self.id.clone(), ip.clone()),
                None => m.remove(&self.id),
            }
        };
        // a new address (the router handed out another): the kept Game's RCON points at the old one
        if before != now {
            forget(&self.id);
        }
    }

    /// The address players connect to.
    pub fn address(&self) -> String {
        match &self.remote {
            Some(r) => format!("{}:{}", r.host, self.port),
            None => format!("{}:{}", self.local_host(), self.port),
        }
    }

    /// The address shown and shared: the public address when one is set (with the port
    /// added unless it names one or the port is the game's usual 27015), else `address`.
    pub fn connect(&self) -> String {
        match &self.public_address {
            Some(a) if a.contains(':') || self.port == 27015 => a.clone(),
            Some(a) => format!("{a}:{}", self.port),
            None => self.address(),
        }
    }

    /// The public address's host name with another port (HLTV's), if one is set.
    pub fn public_at(&self, port: u16) -> Option<String> {
        self.public_address.as_deref().map(|a| format!("{}:{port}", public_host(a)))
    }

    /// What Join on this computer connects to: the shared address for a server on another
    /// machine; a server on this computer is joined directly (a router may not loop its
    /// own internet address back)
    pub(crate) fn join_address(&self) -> String {
        if self.remote.is_some() { self.connect() } else { self.address() }
    }
}

/// The automatic address each server on this computer was last started on (bind_ip)
static PINNED: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// This computer's address on the network its internet traffic leaves through (nothing is
/// sent: a UDP socket only picks its route on connect).
fn route_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("1.1.1.1:53").ok()?;
    let ip = s.local_addr().ok()?.ip();
    if !ip.is_ipv4() || ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    // a VPN carrying all traffic: not an address players or the router reach, so left to
    // the server as before (it answers on every address then)
    let vpn = if_addrs::get_if_addrs().unwrap_or_default().into_iter().find(|i| i.ip() == ip).is_some_and(|i| {
        let n = i.name.to_lowercase();
        ["vpn", "tap", "tun", "wireguard", "wg", "tailscale", "zerotier", "nord", "proton", "mullvad", "express", "surfshark", "openvpn", "cisco", "fortinet"].iter().any(|w| n.contains(w))
    });
    let cgnat = matches!(ip, std::net::IpAddr::V4(v) if v.octets()[0] == 100 && (64..128).contains(&v.octets()[1]));
    (!vpn && !cgnat).then(|| ip.to_string())
}

/// Ports a server on another machine can use: setup opens 27015-27020 there, and 27020
/// is HLTV's
pub(crate) const REMOTE_PORTS: std::ops::RangeInclusive<u16> = 27015..=27019;

/// Start options the app sets itself, so they can't be added again by hand
const OWN_ARGS: &[&str] = &[
    "-game", "-port", "+port", "-ip", "+ip", "+hostport", "+clientport", "-console", "-condebug", "-nomaster", "+map", "+maxplayers", "-maxplayers", "+sv_lan", "+rcon_password", "+sv_password", "+exec",
    "+bot_join_after_player",
];

/// The player's extra start options, split up, or why they can't be used.
pub fn extra_args(text: &str) -> Result<Vec<String>, String> {
    let words: Vec<String> = text.split_whitespace().map(String::from).collect();
    if text.len() > 300 || words.len() > 30 {
        return Err("That's too many start options.".into());
    }
    for w in &words {
        if !w.chars().all(|c| c.is_ascii_alphanumeric() || "+-_.:/".contains(c)) {
            return Err(format!("\"{w}\" can't be used: start options are letters, numbers and + - _ . : /"));
        }
        if OWN_ARGS.contains(&w.to_ascii_lowercase().as_str()) {
            return Err(format!("{w} is set by the app. Change it with the settings above."));
        }
    }
    if words.first().is_some_and(|w| !w.starts_with(['-', '+'])) {
        return Err("Start options begin with - or +, like -pingboost 2. Game settings (gravity, round time) belong in the Game Mode Menu: the server sets them again on every map.".into());
    }
    Ok(words)
}

/// The join password, as the Friends tab keeps it (empty: none).
pub(crate) fn join_password(e: &Entry) -> String {
    std::fs::read_to_string(e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes/friends.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["password"].as_str().map(String::from))
        .filter(|p| crate::sharing::password_ok(p))
        .unwrap_or_default()
}

/// The name and join password in friends.json and friends.cfg (which the plugin runs
/// after every map change), the same files the Friends tab writes. friends.cfg also says
/// whether bots play with nobody on: ReGameDLL's game.cfg sets bot_join_after_player back
/// to 1 on every map, and this runs after it.
pub(crate) fn write_join(data: &Path, name: &str, password: &str, bots_empty: bool, listed: bool) -> Result<(), String> {
    if !crate::sharing::password_ok(password) {
        return Err("Passwords can use letters, numbers, - and _ (up to 24).".into());
    }
    let name = crate::sharing::clean_name(name);
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    std::fs::write(data.join("friends.json"), json!({"name": name, "password": password}).to_string()).map_err(|e| e.to_string())?;
    let cfg = format!("// Written by Loner-Strike\nhostname \"{name}\"\nsv_password \"{password}\"\nbot_join_after_player {}\n", if bots_empty { 0 } else { 1 });
    std::fs::write(data.join("friends.cfg"), with_list_line(&cfg, listed)).map_err(|e| e.to_string())
}

/// friends.cfg with its ls_list line set: 1 while the server is on the Loner-Strike server
/// list, 0 once it was (a file that never had the line stays as it is). The list only shows
/// servers whose ls_list is 1, so only someone who runs a server can list it.
pub(crate) fn with_list_line(text: &str, listed: bool) -> String {
    let had = text.lines().any(|l| l.trim_start().starts_with("ls_list"));
    if !listed && !had {
        return text.to_string();
    }
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("ls_list")).collect();
    if lines.is_empty() {
        lines.push("// Written by Loner-Strike");
    }
    format!("{}\nls_list {}\n", lines.join("\n"), if listed { 1 } else { 0 })
}

/// friends.cfg with its bot_join_after_player line set to match `bots_empty`, the rest
/// kept as it is (an imported server's file may have nothing else: its own server.cfg
/// keeps the name and password).
pub(crate) fn with_bots_line(text: &str, bots_empty: bool) -> String {
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("bot_join_after_player")).collect();
    if lines.is_empty() {
        lines.push("// Written by Loner-Strike");
    }
    let line = format!("bot_join_after_player {}", if bots_empty { 0 } else { 1 });
    format!("{}\n{line}\n", lines.join("\n"))
}

/// Brings friends.cfg's bots line in step with the server's setting (start options can't
/// carry it: ReGameDLL's game.cfg sets it back to 1 on every map, and friends.cfg runs
/// after it). Whether the file changed.
fn write_bots_line(e: &Entry) -> Result<bool, String> {
    let data = e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes");
    let path = data.join("friends.cfg");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = with_list_line(&with_bots_line(&old, bots_when_empty(e)), e.ls_listed);
    if new == old {
        return Ok(false);
    }
    std::fs::create_dir_all(&data).map_err(|err| err.to_string())?;
    std::fs::write(&path, new).map_err(|err| format!("Couldn't write friends.cfg ({err})."))?;
    Ok(true)
}

fn list_file() -> PathBuf {
    sys::app_dir().join("servers.json")
}

pub fn load() -> Vec<Entry> {
    load_checked().unwrap_or_default()
}

/// The list, or an error if the file is there but couldn't be read (another program
/// holding it for a moment): a change must not save an empty list over it then.
fn load_checked() -> Result<Vec<Entry>, String> {
    let file = list_file();
    let mut list: Vec<Entry> = match sys::read_text(&file) {
        Ok(Some(t)) => serde_json::from_str(&t).unwrap_or_else(|_| {
            sys::set_aside(&file);
            Vec::new()
        }),
        Ok(None) => Vec::new(),
        Err(e) => return Err(format!("Couldn't read the server list ({e}). Try again in a moment.")),
    };
    // servers made before they started at 27016 shared the game's port: move them
    // (saved with the next change)
    // (not servers on another machine: they're alone there, on 27015 to 27019)
    for e in list.iter_mut().filter(|e| e.remote.is_some() && !REMOTE_PORTS.contains(&e.port)) {
        e.port = 27015;
    }
    for i in 0..list.len() {
        if !list[i].imported && list[i].remote.is_none() && list[i].port < FIRST_PORT {
            list[i].port = free_port(&list);
        }
    }
    Ok(list)
}

/// A running server on `port`, by name (the game's own port is 27015).
/// Whether the server in this folder (on this computer) is running.
pub fn running_at(dir: &Path) -> bool {
    load().into_iter().any(|e| e.remote.is_none() && crate::steam::same_dir(&e.dir, dir) && running(&e.id))
}

pub fn running_on(port: u16) -> Option<String> {
    // (servers on other machines use their machine's ports, not this computer's)
    load().into_iter().find(|e| e.remote.is_none() && e.port == port && running(&e.id)).map(|e| e.name)
}

fn save(list: &[Entry]) -> Result<(), String> {
    sys::write_atomic(&list_file(), serde_json::to_string_pretty(list).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

static LIST_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn edit(f: impl FnOnce(&mut Vec<Entry>)) -> Result<(), String> {
    let _g = LIST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load_checked()?;
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
pub(crate) fn root_for(client: &Install) -> Result<PathBuf, String> {
    // the folder picked in Settings, if it suits this game (on a Mac: in its bottle)
    if let Some(dir) = crate::app_api::load_settings().servers_dir.map(PathBuf::from) {
        if servers_dir_ok(&dir, client).is_ok() {
            return Ok(dir);
        }
    }
    default_root(client)
}

/// Loner-Strike/Servers: in the game's CrossOver bottle on a Mac (the servers run
/// there), in the app's data folder on Windows.
pub fn default_root(client: &Install) -> Result<PathBuf, String> {
    if client.kind == "crossover" {
        Ok(drive_c(client)?.join("Loner-Strike").join("Servers"))
    } else {
        let local = dirs::data_local_dir().ok_or("Couldn't find the app data folder.")?;
        // the game server can't open paths with letters outside ASCII (a user name like
        // José), so those computers keep servers at the top of the system drive instead
        let base = if local.to_string_lossy().is_ascii() { local } else { PathBuf::from(std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()) + "\\") };
        Ok(base.join("Loner-Strike").join("Servers"))
    }
}

/// The C: drive of the bottle the game is in.
fn drive_c(client: &Install) -> Result<PathBuf, String> {
    let s = client.hl.to_string_lossy().to_string();
    let i = s.find("/drive_c/").ok_or("The game isn't in a CrossOver bottle.")?;
    Ok(PathBuf::from(&s[..i + 8]))
}

/// Whether new servers can go in `dir` for this game: an absolute folder, and on a Mac
/// inside the same CrossOver bottle as the game (servers run through it).
pub fn servers_dir_ok(dir: &Path, client: &Install) -> Result<(), String> {
    if !dir.is_absolute() {
        return Err("Pick a folder, not a relative path.".into());
    }
    if client.kind == "crossover" && !dir.starts_with(drive_c(client)?) {
        return Err("On a Mac the servers have to be inside the same CrossOver bottle as the game (its drive_c folder).".into());
    }
    if dir.exists() && !dir.is_dir() {
        return Err("That's a file, not a folder.".into());
    }
    // what the server sees: on a Mac only the part inside the bottle's C: drive
    let seen = if client.kind == "crossover" { dir.strip_prefix(drive_c(client)?).unwrap_or(dir) } else { dir };
    if !seen.to_string_lossy().is_ascii() {
        return Err("The game server can't use a folder whose path has accents or other special letters in it. Pick one with plain letters, like C:\\Loner-Strike.".into());
    }
    Ok(())
}

/// A folder Loner-Strike made for a server (only those are ever deleted): named after
/// it, in one of the servers folders it uses.
fn made_here(e: &Entry) -> bool {
    let named = e.dir.file_name().map(|n| n.to_string_lossy() == e.id).unwrap_or(false);
    // marked when it was made; servers from before the marker are only in the servers
    // folders Loner-Strike used then
    let marked = std::fs::read_to_string(e.dir.join(MARKER)).is_ok_and(|t| t.trim() == e.id);
    let legacy = e.dir.parent().and_then(|p| p.file_name()).map(|n| n == FOLDER || n == "Servers").unwrap_or(false);
    !e.imported && named && (marked || legacy)
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
    // "sh-" starts the ids of servers friends share with you (shared control)
    if base.starts_with("sh-") {
        base.insert_str(0, "my-");
        base.truncate(32);
    }
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|e| e.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// A new server's id, also free on disk: its folder is always a new one, never a folder
/// that's already there (a game, someone's files), which Delete would then remove.
pub(crate) fn slug_in(name: &str, taken: &[Entry], root: &Path) -> String {
    let base = slug(name, taken);
    let mut id = base.clone();
    let mut n = 2;
    while root.join(&id).exists() || taken.iter().any(|e| e.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Written into every server folder Loner-Strike makes; only folders with it (or in
/// the servers folders of earlier versions) are ever deleted.
const MARKER: &str = ".loner-strike-server";

/// Makes a new server's folder and marks it as ours.
pub(crate) fn make_server_dir(e: &Entry) -> Result<(), String> {
    std::fs::create_dir_all(&e.dir).map_err(|err| format!("Couldn't make the server's folder ({err})."))?;
    std::fs::write(e.dir.join(MARKER), &e.id).map_err(|err| format!("Couldn't make the server's folder ({err})."))
}

pub(crate) fn free_port(taken: &[Entry]) -> u16 {
    (FIRST_PORT..FIRST_PORT + 100).find(|p| !taken.iter().any(|e| e.port == *p && e.port >= FIRST_PORT)).unwrap_or(FIRST_PORT)
}

pub(crate) fn safe_map(m: &str) -> bool {
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
pub(crate) fn ensure_steamcmd(job: &Job) -> Result<(), String> {
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
pub(crate) fn download_hlds(job: &Job, dir: &Path, game: &str) -> Result<(), String> {
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
pub(crate) fn copy_tree(from: &Path, to: &Path, overwrite: bool, filter: &dyn Fn(&Path) -> bool) -> Result<u64, String> {
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
    /// to join it (empty: none)
    #[serde(default)]
    pub password: String,
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
    // the mods and add-ons it has (setup puts in the foundation and our own plugins itself)
    let comps = catalog
        .iter()
        .filter(|c| setup::components::installable(c))
        .filter_map(|c| c["id"].as_str())
        .filter(|id| rec.installed(id))
        .map(String::from)
        .collect();
    Some((dir, comps))
}

/// Game Mode Menu settings worth carrying over: presets, saved settings, admins…
/// not the panel's password, live status or logs, nor the template's history (its
/// stats, reports, matches and who played there belong to it, not to a new server).
fn settings_filter(rel: &Path) -> bool {
    let name = rel.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let history = matches!(
        name.as_str(),
        "stats.json" | "stats.json.part" | "playtime.txt" | "seen.json" | "reports.json" | "matches.json" | "scrim_status.json" | "recent.txt"
    );
    !matches!(name.as_str(), "panel_rcon.txt" | "status.json" | "friends.json" | "friends.cfg")
        && !history
        && !name.ends_with(".log")
        && !name.ends_with(".reading")
        && !half_written(&name)
}

/// Maps and what custom maps need (textures, models, sounds, sprites, overviews).
/// Files a restore or a backup is still writing (backup.rs): never synced.
fn half_written(name: &str) -> bool {
    name.ends_with(".lsrestore") || name.ends_with(".lsbackup.part") || name.ends_with(".tmp")
}

pub(crate) fn content_filter(rel: &Path) -> bool {
    let name = rel.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if half_written(&name) {
        return false;
    }
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
    let join_pw = req.password.trim().to_string();
    if !crate::sharing::password_ok(&join_pw) {
        return Err("Passwords can use letters, numbers, - and _ (up to 24).".into());
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
    let id = slug_in(&name, &list, &root);
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
        hltv: Default::default(),
        restart: None,
        remote: None,
        setup_with: None,
        listen_ip: String::new(),
        extra_args: String::new(),
        bots_when_empty: None,
        ls_listed: false,
        public_address: None, web_status: Default::default(), pending_restart: Default::default(), service_stale: false, keep_running: None, notify_joins: None, notify_helpers: None, auto_update: None,
    };
    let base = root.join(".base");
    let g = req.game.clone();
    let steps = vec![
        setup::server_step(&g, "srv-steamcmd", "Getting SteamCMD (Valve's server downloader)", 1.0),
        setup::server_step(&g, "srv-download", "Downloading the server from Valve", 8.0),
        setup::server_step(&g, "srv-copy", "Making the server's folder", 1.0),
        setup::server_step(&g, "srv-engine", "Installing Loner-Strike's server engine", 0.5),
        setup::server_step(&g, "srv-settings", "Copying settings and admins", 0.3),
        setup::server_step(&g, "srv-maps", "Copying maps", 2.0),
        setup::server_step(&g, "srv-done", "Finishing up", 0.1),
    ];
    make_server_dir(&entry)?;
    edit(|l| l.push(entry.clone()))?;
    let entry_id = entry.id.clone();
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
        job.insert_before(&format!("{g}/srv-engine"), plan.step_list());
        let store = Arc::new(crate::gamebanana::MapStore::new_server(&entry.dir, &entry.dir.join(&g), &sys::app_dir(), Arc::new(Mutex::new(None))));
        setup::install_into(job, &plan, &res, Some(store))?;
        job.run(&format!("{g}/srv-engine"), || install_engine(job, &entry.dir, &g))?;
        let data = entry.dir.join(&g).join("addons/amxmodx/data/gamemodes");
        job.run(&format!("{g}/srv-settings"), || {
            if let Some(src) = &source {
                copy_tree(&src.join("addons/amxmodx/data/gamemodes"), &data, true, &settings_filter)?;
            }
            let _ = std::fs::create_dir_all(&data);
            write_join(&data, &entry.name, &join_pw, bots_when_empty(&entry), false)?;
            // empty ban lists, so the server doesn't complain it has none
            for f in ["banned.cfg", "listip.cfg"] {
                let p = entry.dir.join(&g).join(f);
                if !p.exists() {
                    let _ = std::fs::write(p, "");
                }
            }
            Ok(())
        })?;
        job.run(&format!("{g}/srv-maps"), || match &source {
            Some(src) => copy_tree(src, &entry.dir.join(&g), false, &content_filter).map(|_| ()),
            None => Ok(()),
        })?;
        job.run(&format!("{g}/srv-done"), || edit(|l| l.iter_mut().filter(|e| e.id == entry.id).for_each(|e| e.ready = true)))
    })
    .inspect_err(|_| {
        // setup was busy with something else: don't leave a server that never gets set up
        let _ = edit(|l| l.retain(|e| e.id != entry_id));
    })?;
    Ok((id, job))
}

// ---- a server on another machine (a Linux VPS) ------------------------------------------

#[derive(Deserialize)]
pub struct NewRemote {
    pub name: String,
    pub game: String,
    pub host: String,
    #[serde(default)]
    pub user: String,
    pub key: String,
    #[serde(default)]
    pub ssh_port: Option<u16>,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    /// The fingerprint the player confirmed (from the probe)
    #[serde(default)]
    pub host_key: String,    /// to join it (empty: none)
    #[serde(default)]
    pub password: String,
}

/// The machine a NewRemote describes (checked; no identity yet).
fn remote_of(req: &NewRemote) -> Result<crate::remote::RemoteInfo, String> {
    let host = req.host.trim().to_string();
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:".contains(c)) {
        return Err("Enter the machine's address, like 146.235.20.10 or myserver.example.com.".into());
    }
    let user = if req.user.trim().is_empty() { "ubuntu".to_string() } else { req.user.trim().to_string() };
    if !user.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err("That user name has characters it can't have.".into());
    }
    Ok(crate::remote::RemoteInfo { host, user, key: PathBuf::from(crate::sys::clean_path(&req.key)), ssh_port: req.ssh_port.unwrap_or(22), host_key: None })
}

/// What of this computer's copy goes up to the machine: Loner-Strike's plugins, configs,
/// data and content. Not the Windows engine files (the machine has its own Linux ones),
/// the plugin sources, or the game's own liblist.gam (set up there for Metamod).
fn upload_filter(rel: &Path) -> bool {
    let p = rel.to_string_lossy().replace('\\', "/").to_lowercase();
    !(p.starts_with("dlls/")
        || p.starts_with("addons/metamod/")
        || p.starts_with("addons/amxmodx/dlls/")
        || p.starts_with("addons/amxmodx/modules/")
        || p.starts_with("addons/amxmodx/scripting/")
        || p == "liblist.gam"
        || p.ends_with(".dll")
        || p.ends_with(".exe"))
}

/// After a change on a settings page (or anything else here): what changed goes up.
pub fn sync_settings(id: &str) {
    // a request may have started something that's still writing (a download): the
    // background sync keeps looking closely for a while
    mark_changed(id);
    if let Some(e) = get(id).filter(|e| e.ready && e.remote.is_some()) {
        let _ = sync(&e, false);
    }
}

/// Servers whose copy here was changed lately, and until when sync_loop looks at them
/// every round instead of once a minute.
static CHANGED: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
/// How long after a change a server is looked at every round
const CHANGED_FOR: Duration = Duration::from_secs(180);
/// How often sync_loop looks at a server nothing was known to change
const IDLE_CHECK: Duration = Duration::from_secs(60);

/// This server's copy here was (or is being) changed: sync_loop looks at it closely.
pub fn mark_changed(id: &str) {
    CHANGED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id.to_string(), Instant::now() + CHANGED_FOR);
}

/// mark_changed for whichever server on another machine has its game folder here
/// (a map or a skin downloaded straight into it).
pub fn mark_changed_dir(game_dir: &Path) {
    for e in load().into_iter().filter(|e| e.remote.is_some() && e.dir.join(&e.game) == game_dir) {
        mark_changed(&e.id);
    }
}

/// Whether sync_loop should walk this server's files now: changed lately, or not
/// looked at for a minute.
fn due(id: &str, last_walk: Option<Instant>) -> bool {
    let hot = CHANGED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(id).copied()).is_some_and(|t| Instant::now() < t);
    hot || last_walk.is_none_or(|t| t.elapsed() >= IDLE_CHECK)
}

// ---- keeping a server on another machine in step with this computer's copy --------------

/// What was last uploaded: path -> (size, modified seconds).
type Manifest = std::collections::BTreeMap<String, (u64, u64)>;

fn manifest_path(e: &Entry) -> PathBuf {
    e.dir.join(".uploaded.json")
}

/// The same file as recorded (records from before kept whole seconds)
fn same_file(old: &(u64, u64), now: &(u64, u64)) -> bool {
    old.0 == now.0 && (old.1 == now.1 || (old.1 < 100_000_000_000 && now.1 / 1_000_000_000 == old.1))
}

/// Files the server on the machine keeps itself: the plugin or a command typed in the game
/// writes them there, and the app reads and edits them on the machine directly
/// (remote::game_file / put_game_file). Sent once when the server is set up, so a
/// template's admins, mutes and map lists come along; never again after that, and never
/// removed from the machine.
const MACHINE_KEPT: &[&str] = &[
    "addons/amxmodx/configs/users.ini",
    "addons/amxmodx/data/gamemodes/gags.txt",
    "addons/amxmodx/data/gamemodes/favorites.txt",
    "addons/amxmodx/data/gamemodes/hidden_maps.txt",
];

/// Files the server on the machine writes itself, or the app keeps only here: never sent
/// from this computer's copy, and never removed from the machine (the ban lists are
/// edited on the machine directly, see admins.rs; stats, matches and the map history are
/// written by the plugin; seen.json and reports.json are this computer's record of what
/// it read from the machine's logs).
pub(crate) fn server_owned(key: &str) -> bool {
    const DATA: &str = "addons/amxmodx/data/gamemodes/";
    let data_file = key.strip_prefix(DATA).filter(|f| !f.contains('/'));
    matches!(key, "banned.cfg" | "listip.cfg")
        || MACHINE_KEPT.contains(&key)
        || data_file.is_some_and(|f| {
            matches!(
                f,
                "status.json" | "recent.txt" | "stats.json" | "stats.json.part" | "playtime.txt" | "matches.json" | "scrim_status.json" | "bots.txt" | "seen.json" | "reports.json" | "content_status.json"
            )
        })
        || key.starts_with("addons/amxmodx/logs/")
        || key.ends_with(".log")
        || key.ends_with(".reading")
        || key.ends_with(".taking")
}

/// Files the plugin writes that aren't the server's own record: sent when this computer
/// changed them (an add-on switched with the server off), but not when everything goes
/// up again after Valve's update, where the copy here may be older than the machine's.
fn plugin_written(key: &str) -> bool {
    matches!(key, "addons/amxmodx/configs/plugins-gamemode.ini" | "addons/amxmodx/configs/plugins-gmaddons.ini")
}

/// What a sync sends and removes, from the record of the last upload (`saved`) and what's
/// here now. (changed, gone)
fn plan_sync(mode: Send, saved: Option<&Manifest>, now: &Manifest) -> (std::collections::HashSet<String>, Vec<String>) {
    match (saved, mode) {
        // (first: after Valve's update put stock configs back, the record can't be trusted)
        (_, Send::AllButData) => (now.keys().filter(|k| !k.starts_with("addons/amxmodx/data/") && !plugin_written(k)).cloned().collect(), vec![]),
        (_, Send::Everything) => (now.keys().cloned().collect(), vec![]),
        (Some(old), Send::Changes) => (
            now.iter().filter(|(k, v)| !old.get(*k).is_some_and(|o| same_file(o, v))).map(|(k, _)| k.clone()).collect(),
            // a record from before a file became the server's own: it stays on the machine
            old.keys().filter(|k| !now.contains_key(*k) && !server_owned(k)).cloned().collect(),
        ),
        (None, Send::Changes) => (std::collections::HashSet::new(), vec![]),
    }
}

/// Writes the record of what was uploaded, unless it's what's there already (most
/// rounds nothing changed). Whether it wrote.
fn save_manifest(path: &Path, saved: Option<&Manifest>, now: &Manifest) -> bool {
    if saved == Some(now) {
        return false;
    }
    sys::write_atomic(path, serde_json::to_string(now).unwrap_or_default().as_bytes()).is_ok()
}

/// This computer's copy as it is now (what upload_filter lets through, minus files the
/// app brings back from the machine).
fn current_files(e: &Entry) -> Manifest {
    let gdir = e.dir.join(&e.game);
    let mut m = Manifest::new();
    for f in crate::archive::walk(&gdir) {
        let Ok(rel) = f.strip_prefix(&gdir) else { continue };
        if !upload_filter(rel) {
            continue;
        }
        let key = rel.to_string_lossy().replace('\\', "/");
        if server_owned(&key) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&f) else { continue };
        // nanoseconds: two saves within the same second still differ
        let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as u64).unwrap_or(0);
        m.insert(key, (meta.len(), modified));
    }
    m
}

/// Notes that a change waits for the server's next restart (kept in the server list).
pub fn set_pending_restart(id: &str, what: Pending) {
    let Some(e) = get(id) else { return };
    let p = &e.pending_restart;
    let already = match what {
        Pending::Files => p.files,
        Pending::Options => p.options,
        Pending::Hltv => p.hltv,
        Pending::Sounds => p.sounds,
    };
    if already {
        return;
    }
    // sounds come in with the next map: remember which map is on now
    let map = if what == Pending::Sounds { current_map(&e).unwrap_or_default() } else { String::new() };
    let _ = edit(|l| {
        for x in l.iter_mut().filter(|x| x.id == id) {
            let p = &mut x.pending_restart;
            match what {
                Pending::Files => p.files = true,
                Pending::Options => p.options = true,
                Pending::Hltv => p.hltv = true,
                Pending::Sounds => {
                    p.sounds = true;
                    p.sounds_map = map.clone();
                }
            }
        }
    });
}

/// The map a running server is playing (None if it doesn't answer).
fn current_map(e: &Entry) -> Option<String> {
    let g = game_for(&e.id, || None)?;
    let st = g.state();
    (st["online"] == json!(true)).then(|| st["state"]["map"].as_str().unwrap_or("").to_string())
}

/// Sounds waiting for a map change: cleared once the server plays another map (asked
/// only while something's waiting, and the server is up).
pub fn check_sounds_pending(e: &Entry) {
    if !e.pending_restart.sounds || !running(&e.id) {
        return;
    }
    let Some(now) = current_map(e) else { return };
    if !now.is_empty() && !now.eq_ignore_ascii_case(&e.pending_restart.sounds_map) {
        clear_sounds_pending(&e.id);
    }
}

/// The map changed (or restarted): sounds are in.
pub fn clear_sounds_pending(id: &str) {
    if get(id).is_some_and(|e| e.pending_restart.sounds) {
        let _ = edit(|l| {
            for x in l.iter_mut().filter(|x| x.id == id) {
                x.pending_restart.sounds = false;
                x.pending_restart.sounds_map.clear();
            }
        });
    }
}

/// The server started or stopped: everything waiting is in (or will be at its start).
fn clear_pending_restart(id: &str) {
    // and ask the machine again next time rather than show what it was a moment ago
    if let Some(m) = SEEN.lock().unwrap().as_mut() {
        m.remove(id);
    }
    if get(id).is_some_and(|e| e.pending_restart != PendingRestart::default()) {
        let _ = edit(|l| l.iter_mut().filter(|e| e.id == id).for_each(|e| e.pending_restart = PendingRestart::default()));
    }
}

/// The server a game is (None for the player's own game).
pub fn id_of(g: &Game) -> Option<String> {
    if !g.dedicated {
        return None;
    }
    load().into_iter().find(|e| e.dir == g.hl && e.game == g.name).map(|e| e.id)
}

/// Whether a file sent to the machine is only used once the map changes or the server
/// restarts (plugins, modules, models, sounds, maps), not straight away like the
/// settings the app reloads. The game folder's own .txt files (mapcycle.txt, motd.txt)
/// are read again as they're needed.
fn needs_restart(key: &str) -> bool {
    let root_txt = !key.contains('/') && key.to_ascii_lowercase().ends_with(".txt");
    !key.starts_with("addons/amxmodx/data/") && !key.starts_with("addons/amxmodx/configs/") && !key.ends_with(".nav") && !root_txt
}

/// Brings the machine in step with this computer's copy: uploads files that are new or
/// changed since the last upload and removes ones the app uploaded that are gone here.
/// Files the server writes itself (stats, its saved settings, logs) were never uploaded
/// by the app after setup, so they're left alone. `everything`: send it all (setup).
/// Returns (sent, removed).
pub fn sync(e: &Entry, everything: bool) -> Result<(usize, usize), String> {
    sync_as(e, if everything { Send::Everything } else { Send::Changes }, &|_, _| {})
}

/// sync, telling `progress` (bytes sent, bytes in all) while files go up.
pub fn sync_with_progress(e: &Entry, progress: &dyn Fn(u64, u64)) -> Result<(usize, usize), String> {
    sync_as(e, Send::Changes, progress)
}

/// What a sync sends.
#[derive(Clone, Copy, PartialEq)]
enum Send {
    /// what changed since the last one (or, with no record of one, what the machine lacks)
    Changes,
    /// every file (a fresh setup)
    Everything,
    /// every file but the settings, which the machine keeps (after Valve's files were updated)
    AllButData,
}

/// How each server's last upload went: (when, Ok((sent, removed)) or Err(why)), for diagnostics.
static LAST_SYNC: Mutex<Option<HashMap<String, (Instant, Result<(usize, usize), String>)>>> = Mutex::new(None);

/// How a server's last upload went this session: (seconds ago, result).
pub fn last_sync(id: &str) -> Option<(u64, Result<(usize, usize), String>)> {
    LAST_SYNC.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(id).map(|(at, r)| (at.elapsed().as_secs(), r.clone()))
}

fn sync_as(e: &Entry, mode: Send, progress: &dyn Fn(u64, u64)) -> Result<(usize, usize), String> {
    let r = sync_files(e, mode, progress);
    if e.remote.is_some() {
        if let Err(why) = &r {
            crate::diagnostics::log_error(&format!("sync {}", e.name), why);
        }
        LAST_SYNC.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(e.id.clone(), (Instant::now(), r.clone()));
    }
    r
}

fn sync_files(e: &Entry, mode: Send, progress: &dyn Fn(u64, u64)) -> Result<(usize, usize), String> {
    let everything = mode == Send::Everything;
    let _g = sync_guard();
    let r = e.remote.as_ref().ok_or("That server is on this computer.")?;
    // deleted or being rebuilt since the caller looked: nothing to send (and nothing to
    // make again on either side)
    match get(&e.id) {
        Some(now) if (everything || now.ready) && e.dir.join(&e.game).is_dir() => {}
        _ => return Ok((0, 0)),
    }
    copy_navs(e);
    let to = format!("{}/{}", crate::remote::server_dir(&e.id), e.game);
    let saved: Option<Manifest> = std::fs::read_to_string(manifest_path(e)).ok().and_then(|t| serde_json::from_str(&t).ok());
    let now = current_files(e);
    let (mut changed, gone) = plan_sync(mode, saved.as_ref(), &now);
    if saved.is_none() && mode == Send::Changes {
        // a server from before the app kept a record: only what the machine doesn't have
        // goes up. What's there stays (it may have changed on the machine, and sending all
        // of it, maps included, would tie up the connection for minutes)
        let there = crate::remote::list_files(r, &to)?;
        changed = now.keys().filter(|k| !there.contains(*k)).cloned().collect();
    }
    if everything {
        // a new server: the template's admins, mutes and map lists go up once
        changed.extend(MACHINE_KEPT.iter().filter(|k| e.dir.join(&e.game).join(k).is_file()).map(|k| k.to_string()));
    }
    if !changed.is_empty() {
        crate::remote::upload_dir_with(r, &e.dir.join(&e.game), &to, &|rel: &Path| changed.contains(&rel.to_string_lossy().replace('\\', "/")), progress)?;
    }
    for chunk in gone.chunks(80) {
        let list = chunk.iter().map(|g| crate::remote::shell_quote(g)).collect::<Vec<_>>().join(" ");
        crate::remote::exec(r, &format!("cd {} && rm -f -- {list}", crate::remote::shell_quote(&to)))?;
    }
    save_manifest(&manifest_path(e), saved.as_ref(), &now);
    let touched = || changed.iter().chain(gone.iter());
    if touched().any(|p| p.starts_with("addons/amxmodx/data/") || p.starts_with("addons/amxmodx/configs/")) {
        if let Some(rc) = e.rcon() {
            let _ = rc.send("amx_reloadadmins;gm_vipreload;gm_extrasreload;gm_commreload;gm_scrimreload;gm_botchat_reload");
        }
    }
    // plugins, modules, models, sounds, maps: only loaded when a map starts
    if !everything {
        // sounds: the next map precaches them; the rest only at a restart
        if touched().any(|p| needs_restart(p) && !p.starts_with("sound/")) {
            set_pending_restart(&e.id, Pending::Files);
        }
        if touched().any(|p| p.starts_with("sound/")) {
            set_pending_restart(&e.id, Pending::Sounds);
        }
    }
    Ok((changed.len(), gone.len()))
}

/// Renames the server whose data folder is `data` (Play with friends sets the name players
/// see; the server list keeps the same one). Nothing for the player's own game.
pub fn rename_by_data(data: &Path, name: &str) {
    let name: String = name.trim().chars().filter(|c| !c.is_control() && *c != '"').take(48).collect();
    let of = |e: &Entry| e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes");
    if name.is_empty() || !load().iter().any(|e| of(e) == data && e.name != name) {
        return;
    }
    let _ = edit(|l| l.iter_mut().filter(|e| of(e) == data).for_each(|e| e.name = name.clone()));
}

/// The server on another machine a Game is about (None for any other game).
pub fn entry_for(g: &Game) -> Option<Entry> {
    g.remote.as_ref()?;
    load().into_iter().find(|e| e.remote.is_some() && e.dir == g.hl && e.game == g.name)
}

/// Brings files the plugin rewrote on the machine (`keys`, relative to the game folder)
/// back into this computer's copy, and records them as uploaded, so the next sync
/// doesn't send the old copy back over them. A file that isn't on the machine is
/// removed here too.
pub fn pull_files(e: &Entry, keys: &[&str]) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("That server is on this computer.")?;
    let _g = sync_guard();
    let gdir = e.dir.join(&e.game);
    let got = crate::remote::cat_files(r, &format!("{}/{}", crate::remote::server_dir(&e.id), e.game), keys)?;
    let path = manifest_path(e);
    let saved: Option<Manifest> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let mut m = saved.clone().unwrap_or_default();
    for (k, data) in keys.iter().zip(got) {
        let p = gdir.join(k);
        match data {
            Some(bytes) => {
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                sys::write_atomic(&p, &bytes).map_err(|err| format!("Couldn't save {k} ({err})."))?;
            }
            None => {
                let _ = std::fs::remove_file(&p);
            }
        }
        // (only with a record: without one the next sync compares with the machine itself)
        if saved.is_some() {
            match std::fs::metadata(&p) {
                Ok(meta) => {
                    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as u64).unwrap_or(0);
                    m.insert(k.to_string(), (meta.len(), modified));
                }
                Err(_) => {
                    m.remove(*k);
                }
            }
        }
    }
    if saved.is_some() {
        save_manifest(&path, saved.as_ref(), &m);
    }
    Ok(())
}

/// Every few seconds: servers on other machines get what changed here (a mod installed,
/// a map downloaded, a plugin added), whichever page did it.
static SYNCING: Mutex<()> = Mutex::new(());
/// When sync_loop tries a server again after it failed
static RETRY_AT: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

/// Held while a server's files go up, and while one is deleted or wiped, so the two
/// never cross.
fn sync_guard() -> std::sync::MutexGuard<'static, ()> {
    SYNCING.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn sync_loop() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        // when each server's files were last walked
        let mut walked: HashMap<String, Instant> = HashMap::new();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(6));
            // not while a setup job runs (it may be writing the copy); teaching bots maps
            // only writes this computer's game, so syncs go on meanwhile
            let job = setup::current();
            if job.state == "running" && job.kind != crate::navs::JOB_KIND {
                continue;
            }
            // a machine that failed is left alone for a minute
            let mut wait = RETRY_AT.lock().unwrap_or_else(|e| e.into_inner());
            let wait = wait.get_or_insert_with(HashMap::new);
            for e in load().into_iter().filter(|e| e.ready && e.remote.is_some()) {
                if wait.get(&e.id).is_some_and(|t| Instant::now() < *t) {
                    continue;
                }
                // walking every file is the costly part: only for a server changed lately,
                // else once a minute (a change nothing told us about still goes up)
                if !due(&e.id, walked.get(&e.id).copied()) {
                    continue;
                }
                walked.insert(e.id.clone(), Instant::now());
                match sync(&e, false) {
                    Ok(_) => {
                        wait.remove(&e.id);
                        // start options saved while the machine couldn't be reached
                        if e.service_stale {
                            let _ = write_service(&e);
                        }
                    }
                    Err(_) => {
                        wait.insert(e.id.clone(), Instant::now() + Duration::from_secs(60));
                    }
                }
            }
        });
    });
}

/// The bots' navigation files this computer's games have learned (CS 1.6 ships none),
/// so the server's bots know those maps without learning them again: a newer one here
/// replaces the server's copy, nothing is ever deleted (navs.rs has the rules).
fn copy_navs(e: &Entry) {
    // reading the game's maps folders (and, for a new map on a machine, asking it): once
    // a minute is plenty
    static LAST: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        let last = last.get_or_insert_with(HashMap::new);
        if last.get(&e.id).is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        last.insert(e.id.clone(), Instant::now());
    }
    crate::navs::copy_to_server(e, None, true);
}

/// Writes a server's service on its machine again (its start options), and notes
/// whether that worked: if not, it's tried again before the next start and by the
/// background sync, so a change made while the machine was away isn't lost.
pub fn write_service(e: &Entry) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("That server is on this computer.")?;
    let set = |stale: bool| {
        if get(&e.id).is_some_and(|x| x.service_stale != stale) {
            let _ = edit(|l| l.iter_mut().filter(|x| x.id == e.id).for_each(|x| x.service_stale = stale));
        }
    };
    // (from the list as it is now: the start options may have changed since `e` was read)
    let now = get(&e.id).unwrap_or_else(|| e.clone());
    let ok = crate::remote::exec_stream(r, "bash -s", crate::remote::service_sh(&now.id, &now.game, &launch_args(&now)).as_bytes(), |_| {});
    match ok {
        Ok(0) => {
            set(false);
            Ok(())
        }
        Ok(_) => {
            set(true);
            Err("Saved here, but the server's service couldn't be updated. It's tried again before the server next starts.".into())
        }
        Err(err) => {
            set(true);
            Err(format!("Saved here, but the server's machine couldn't be reached to update its service ({err}). It's tried again before the server next starts."))
        }
    }
}

/// Puts this computer's copy of a server's files on its machine, all of it (setup).
pub fn upload(e: &Entry) -> Result<u64, String> {
    sync(e, true).map(|(n, _)| n as u64)
}

/// Sets up a server on a Linux machine over SSH: the machine and Valve's server first
/// (remote::SETUP_SH), then Loner-Strike built here like any server, uploaded, and run
/// as a service. Returns (id, job).
pub fn create_remote(req: NewRemote, client: Install, res: PathBuf) -> Result<(String, String), String> {
    let name = req.name.trim().chars().filter(|c| !c.is_control() && *c != '"').take(48).collect::<String>();
    if name.is_empty() {
        return Err("Give the server a name.".into());
    }
    if !["cstrike", "czero"].contains(&req.game.as_str()) {
        return Err("Pick Counter-Strike 1.6 or Condition Zero.".into());
    }
    let join_pw = req.password.trim().to_string();
    if !crate::sharing::password_ok(&join_pw) {
        return Err("Passwords can use letters, numbers, - and _ (up to 24).".into());
    }
    let mut info = remote_of(&req)?;
    // the machine the player confirmed, and no other: connecting checks its identity
    // against this fingerprint, now and every time after
    if !req.host_key.starts_with("SHA256:") {
        return Err("Check the server first.".into());
    }
    info.host_key = Some(req.host_key.clone());
    let (_, arch) = crate::remote::exec(&info, "uname -m")
        .map_err(|e| if e.contains("identity") { "The server's fingerprint isn't the one you confirmed. Check the server again.".to_string() } else { e })?;
    if arch.trim() != "x86_64" {
        return Err(format!("That machine is {}. The Counter-Strike server only runs on x86 (AMD or Intel) machines.", arch.trim()));
    }
    if client.kind == "crossover" && !Path::new(CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. It's needed here to build the server's plugins.".into());
    }
    let catalog = setup::components::load_catalog(&res)?;
    let (source, components) = match template_source(&req.template, &req.game, &client, &catalog) {
        Some((dir, comps)) => (Some(dir), comps),
        None if req.template == "fresh" || req.template.is_empty() => (None, req.components.clone()),
        None => return Err("That template isn't there any more, or it's for the other game.".into()),
    };
    let root = root_for(&client)?;
    let list = load();
    let id = slug_in(&name, &list, &root);
    let entry = Entry {
        id: id.clone(),
        name: name.clone(),
        game: req.game.clone(),
        // the only server on the machine: the usual port
        port: 27015,
        dir: root.join(&id),
        kind: client.kind.clone(),
        bottle: client.bottle.clone(),
        visibility: req.visibility.filter(|v| ["lan", "unlisted", "listed"].contains(&v.as_str())).unwrap_or_else(|| "unlisted".into()),
        map: default_map(),
        maxplayers: default_slots(),
        ready: false,
        imported: false,
        hltv: Default::default(),
        restart: None,
        remote: Some(info.clone()),
        setup_with: Some(SetupWith { template: req.template.clone(), components: req.components.clone(), password: join_pw.clone() }),
        listen_ip: String::new(),
        extra_args: String::new(),
        bots_when_empty: None,
        ls_listed: false,
        public_address: None, web_status: Default::default(), pending_restart: Default::default(), service_stale: false, keep_running: None, notify_joins: None, notify_helpers: None, auto_update: None,
    };
    make_server_dir(&entry)?;
    edit(|l| l.push(entry.clone()))?;
    let job = remote_setup(entry, source, components, res).inspect_err(|_| {
        let _ = edit(|l| l.retain(|e| e.id != id));
    })?;
    Ok((id, job))
}

/// Runs remote::SETUP_SH on the machine (the machine, Valve's server and the engine
/// add-ons; safe to run again, which is also how Valve's updates come in), showing its
/// stages and Valve's download on the job.
pub(crate) fn machine_setup(job: &setup::Job, r: &crate::remote::RemoteInfo, id: &str, g: &str) -> Result<(), String> {
    // the script reports its stages; Valve's download reports its own progress
    const STAGES: &[(&str, f64, &str)] = &[
        ("packages", 0.03, "Installing the 32-bit libraries the server needs"),
        ("firewall", 0.07, "Opening the game ports in the machine's firewall, closing RCON to the internet"),
        ("security", 0.08, "Turning on automatic security updates and fail2ban"),
        ("steamcmd", 0.1, "Getting SteamCMD"),
        ("server", 0.15, "Downloading the server from Valve (a few minutes)"),
        ("foundation", 0.9, "Installing Metamod-P, AMX Mod X and ReGameDLL"),
        ("ready", 1.0, "The machine is ready"),
    ];
    let mut failed = String::new();
    let mut last = String::new();
    let script = format!("{} bash -s", crate::remote::setup_env(id, g));
    let code = crate::remote::exec_stream(r, &script, crate::remote::SETUP_SH.as_bytes(), |line| {
        if let Some(stage) = line.strip_prefix("@@STEP ") {
            if let Some((_, f, text)) = STAGES.iter().find(|(s, _, _)| *s == stage) {
                job_progress(job, *f, text);
            }
        } else if let Some(why) = line.strip_prefix("@@FAIL ") {
            failed = why.to_string();
        } else if let Some((f, text)) = parse_progress(line) {
            job_progress(job, 0.15 + f * 0.75, &text);
        } else if !line.trim().is_empty() {
            last = line.trim().chars().take(160).collect();
        }
    })?;
    if code != 0 {
        return Err(if !failed.is_empty() { failed } else if !last.is_empty() { format!("Setup on the server stopped: {last}") } else { "Setup on the server stopped.".into() });
    }
    Ok(())
}

/// The setup job for a server on another machine (also "Try again" after a setup that
/// stopped: every step is safe to run again). Returns the job id.
fn remote_setup(entry: Entry, source: Option<PathBuf>, components: Vec<String>, res: PathBuf) -> Result<String, String> {
    let g = entry.game.clone();
    let host = entry.remote.as_ref().map(|r| r.host.clone()).unwrap_or_default();
    let steps = vec![
        setup::server_step(&g, "srv-machine", &format!("Setting up {host}: libraries, firewall, Valve's server"), 10.0),
        setup::server_step(&g, "srv-settings", "Copying settings and admins", 0.3),
        setup::server_step(&g, "srv-upload", "Uploading Loner-Strike to the server", 1.5),
        setup::server_step(&g, "srv-service", "Starting it as a service", 0.3),
        setup::server_step(&g, "srv-done", "Finishing up", 0.1),
    ];
    setup::start_job("server", steps, move |job| {
        let r = entry.remote.clone().unwrap();
        job.run(&format!("{g}/srv-machine"), || machine_setup(job, &r, &entry.id, &g))?;
        // this computer's copy: the game's own liblist.gam from the machine, then Loner-Strike
        // built into it like any server
        let gdir = entry.dir.join(&g);
        std::fs::create_dir_all(&gdir).map_err(|e| e.to_string())?;
        let liblist = crate::remote::read_file(&r, &format!("{}/{g}/liblist.gam", crate::remote::server_dir(&entry.id)))?;
        std::fs::write(gdir.join("liblist.gam"), liblist).map_err(|e| e.to_string())?;
        let plan = setup::plan_install(entry.install(), &[g.clone()], &components, &res, false)?;
        job.insert_before(&format!("{g}/srv-settings"), plan.step_list());
        let store = Arc::new(crate::gamebanana::MapStore::new_server(&entry.dir, &gdir, &sys::app_dir(), Arc::new(Mutex::new(None))));
        setup::install_into(job, &plan, &res, Some(store))?;
        let data = gdir.join("addons/amxmodx/data/gamemodes");
        job.run(&format!("{g}/srv-settings"), || {
            if let Some(src) = &source {
                copy_tree(&src.join("addons/amxmodx/data/gamemodes"), &data, true, &settings_filter)?;
            }
            let _ = std::fs::create_dir_all(&data);
            // the password from the form, or the one it has now when this runs again
            let pw = Some(join_password(&entry)).filter(|p| !p.is_empty()).or(entry.setup_with.as_ref().map(|w| w.password.clone())).unwrap_or_default();
            write_join(&data, &entry.name, &pw, bots_when_empty(&entry), false)?;
            // the panel's RCON password, which the plugin reads on the machine
            if !data.join("panel_rcon.txt").is_file() {
                use rand::Rng;
                const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
                let mut rng = rand::thread_rng();
                let pw: String = (0..24).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
                std::fs::write(data.join("panel_rcon.txt"), format!("{pw}\n")).map_err(|e| e.to_string())?;
            }
            for f in ["banned.cfg", "listip.cfg"] {
                let p = gdir.join(f);
                if !p.exists() {
                    let _ = std::fs::write(p, "");
                }
            }
            Ok(())
        })?;
        job.run(&format!("{g}/srv-upload"), || upload(&entry).map(|n| setup::Progress::detail(job, &format!("{n} files uploaded"))))?;
        job.run(&format!("{g}/srv-service"), || {
            let mut failed = String::new();
            let code = crate::remote::exec_stream(&r, "bash -s", crate::remote::service_sh(&entry.id, &entry.game, &launch_args(&entry)).as_bytes(), |line| {
                if !line.starts_with("@@") && !line.trim().is_empty() {
                    failed = line.trim().to_string();
                }
            })?;
            if code != 0 {
                return Err(format!("Couldn't set up the service ({failed})."));
            }
            crate::remote::systemctl(&r, &entry.id, "start")?;
            // shared control on the server itself (helpers don't need this computer on);
            // the server works without it, so a problem here is only noted
            if let Err(why) = crate::control::agent::install(&entry, &res) {
                setup::Progress::note(job, &format!("Shared control isn't set up on the server yet: {why} It's tried again when you invite someone."));
            }
            Ok(())
        })?;
        job.run(&format!("{g}/srv-done"), || edit(|l| l.iter_mut().filter(|e| e.id == entry.id).for_each(|e| e.ready = true)))?;
        // a rebuilt machine gets its website status page back (if it can't, its status says why)
        crate::webstatus::refresh(&entry.id);
        Ok(())
    })
}

/// Re-downloads Valve's files into the base copy and refreshes the server's engine files.
pub fn update_files(e: Entry, res: Option<PathBuf>) -> Result<String, String> {
    if running(&e.id) {
        return Err("Stop the server first.".into());
    }
    let g = e.game.clone();
    if let Some(r) = e.remote.clone() {
        // on the machine: the setup script again (SteamCMD checks Valve's files, the add-ons
        // are put back), then every Loner-Strike file sent again over whatever it replaced
        let host = r.host.clone();
        let steps = vec![
            setup::server_step(&g, "srv-machine", &format!("Updating {host}: Valve's server files"), 10.0),
            setup::server_step(&g, "srv-upload", "Sending Loner-Strike's files again", 1.5),
            setup::server_step(&g, "srv-control", "Updating the service and shared control", 0.2),
        ];
        let res = res.ok_or("Loner-Strike's mod files are missing from the app. Reinstalling the app should fix it.")?;
        return setup::start_job("server", steps, move |job| {
            job.run(&format!("{g}/srv-machine"), || machine_setup(job, &r, &e.id, &g))?;
            // everything but the settings goes up again over what setup put back (the
            // machine's own stats and bans stay as they are)
            job.run(&format!("{g}/srv-upload"), || sync_as(&e, Send::AllButData, &|_, _| {}).map(|_| ()))?;
            // (the server works without it: only noted)
            job.run(&format!("{g}/srv-control"), || {
                // the game's service as this version writes it (its sandbox), from the next start
                if let Err(why) = write_service(&e) {
                    setup::Progress::note(job, &format!("The server's service wasn't updated: {why}"));
                }
                if let Err(why) = crate::control::agent::install(&e, &res) {
                    setup::Progress::note(job, &format!("Shared control on the server wasn't updated: {why}"));
                }
                Ok(())
            })
        });
    }
    let base = e.dir.parent().ok_or("That isn't a server folder Loner-Strike can use.")?.join(".base");
    let steps = vec![
        setup::server_step(&g, "srv-steamcmd", "Getting SteamCMD", 0.5),
        setup::server_step(&g, "srv-download", "Checking Valve's server files", 8.0),
        setup::server_step(&g, "srv-copy", "Updating the server", 1.0),
        setup::server_step(&g, "srv-engine", "Installing Loner-Strike's server engine", 0.5),
    ];
    setup::start_job("server", steps, move |job| {
        job.run(&format!("{g}/srv-steamcmd"), || ensure_steamcmd(job))?;
        job.run(&format!("{g}/srv-download"), || download_hlds(job, &base, &g))?;
        // engine files only: never the game folders, which hold Loner-Strike's changes
        job.run(&format!("{g}/srv-copy"), || {
            copy_tree(&base, &e.dir, true, &|rel: &Path| rel.components().count() == 1 && rel.extension().map(|x| x != "gam").unwrap_or(true)).map(|_| ())
        })?;
        // Valve's engine files just came back: Loner-Strike's go over them again
        job.run(&format!("{g}/srv-engine"), || install_engine(job, &e.dir, &g))
    })
}

/// Loner-Strike's server engine on a dedicated server on this computer: our ReHLDS (Valve's
/// engine with ReHLDS's fixes, and it tells the bots who is talking on voice), ReAPI for
/// plugins, and relocalizebugfix (blocks names and chat that crash other players' games).
/// The Valve files it replaces are kept in .valve-engine the first time, to go back to.
fn install_engine(job: &Job, dir: &Path, game: &str) -> Result<(), String> {
    use setup::foundation::{REAPI, REHLDS, RELOCALIZEBUGFIX};
    let tmp = setup::TempDir::new()?;
    let get = |p: &setup::foundation::Pinned, sub: &str| -> Result<PathBuf, String> {
        let file = setup::fetch::download(p.url, Some(p.sha256), p.what, job)?;
        let to = tmp.0.join(sub);
        setup::fetch::unpack(&file, &to)?;
        Ok(to)
    };
    let copy = |from: &Path, to: &Path| -> Result<(), String> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(from, to).map(|_| ()).map_err(|e| format!("Couldn't copy {} ({e}). If the server is running, stop it and try again.", to.display()))
    };

    let rh = get(&REHLDS, "rh")?;
    let bin = rh.join("bin").join("win32");
    if !bin.join("swds.dll").is_file() {
        return Err("The ReHLDS download is missing its engine. Try again.".into());
    }
    let keep = dir.join(".valve-engine");
    for (full, rel) in setup::foundation::tree(&bin) {
        let dest = dir.join(&rel);
        let kept = keep.join(&rel);
        if dest.is_file() && !kept.exists() {
            copy(&dest, &kept)?;
        }
        copy(&full, &dest)?;
    }

    let gdir = dir.join(game);
    let ra = get(&REAPI, "reapi")?;
    copy(&ra.join("addons/amxmodx/modules/reapi_amxx.dll"), &gdir.join("addons/amxmodx/modules/reapi_amxx.dll"))?;

    let rl = get(&RELOCALIZEBUGFIX, "rlb")?;
    copy(&rl.join("addons/relocalizebugfix/relocalizebugfix_mm.dll"), &gdir.join("addons/relocalizebugfix/relocalizebugfix_mm.dll"))?;
    let ini = gdir.join("addons/metamod/plugins.ini");
    // as bytes: the file may not be UTF-8, and its other lines must stay as they are
    let text = std::fs::read(&ini).unwrap_or_default();
    if !text.windows(16).any(|w| w.eq_ignore_ascii_case(b"relocalizebugfix")) {
        let line = br"win32 addons\relocalizebugfix\relocalizebugfix_mm.dll";
        let mut out = text.clone();
        while matches!(out.last(), Some(b'\r' | b'\n')) {
            out.pop();
        }
        if !out.is_empty() {
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(line);
        out.extend_from_slice(b"\r\n");
        sys::write_atomic(&ini, &out).map_err(|e| e.to_string())?;
    }
    Ok(())
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
/// The server's own welcome page (motd.txt, motd.html or its motdfile) becomes the
/// welcome page, since Loner-Strike's own replaces it once it's added. Valve's
/// stock one is left out. Returns "url", "page" or "" (nothing, or too long).
fn import_motd(game_dir: &Path, cfg: &str, data: &Path) -> &'static str {
    if data.join("welcome.html").exists() || data.join("welcome_url.txt").exists() {
        return "";
    }
    let named = cfg_value(cfg, "motdfile").filter(|f| !f.is_empty() && !f.contains("..") && !f.contains(':'));
    for name in named.into_iter().chain(["motd.html".to_string(), "motd.txt".to_string()]) {
        let Ok(text) = sys::read_latin1(&game_dir.join(&name)) else { continue };
        let t = text.trim();
        let lower = t.to_lowercase();
        if t.is_empty() || lower.contains("counter-strike.net") || lower.contains("steampowered.com") || lower.contains("no_motd") {
            continue;
        }
        if (lower.starts_with("http://") || lower.starts_with("https://")) && !t.contains(char::is_whitespace) {
            let _ = sys::write_latin1(&data.join("welcome_url.txt"), t);
            return "url";
        }
        if t.len() <= crate::welcome::MAX_HTML {
            let _ = sys::write_latin1(&data.join("welcome.html"), t);
            return "page";
        }
        return "";
    }
    ""
}

pub fn import(dir: &Path) -> Result<Value, String> {
    let dir = sys::canonical(dir).map_err(|_| "That folder isn't there.".to_string())?;
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
        hltv: Default::default(),
        restart: None,
        remote: None,
        setup_with: None,
        listen_ip: String::new(),
        extra_args: String::new(),
        bots_when_empty: None,
        ls_listed: false,
        public_address: None, web_status: Default::default(), pending_restart: Default::default(), service_stale: false, keep_running: None, notify_joins: None, notify_helpers: None, auto_update: None,
    };
    // name and password as the Friends tab keeps them (it writes friends.cfg only when
    // something changes there, so the server's own server.cfg keeps working meanwhile)
    let data = dir.join(game).join("addons/amxmodx/data/gamemodes");
    let _ = std::fs::create_dir_all(&data);
    if !data.join("friends.json").exists() {
        let _ = std::fs::write(data.join("friends.json"), json!({"name": name, "password": password}).to_string());
    }
    // the panel uses the server's own RCON password, so existing admin tools keep working
    if let Some(pw) = cfg_value(&cfg, "rcon_password").filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_graphic() && c != '"')) {
        let _ = std::fs::write(data.join("panel_rcon.txt"), format!("{pw}\n"));
    }
    // whether bots play with nobody on: only that line in friends.cfg (and again before
    // each start), so the server's own name and password still come from server.cfg
    let _ = write_bots_line(&entry);
    let welcome = import_motd(&dir.join(game), &cfg, &data);
    let admins = crate::admins::import_users(&dir.join(game), &data);
    // (a server you already had: never marked, so never deleted)
    edit(|l| l.push(entry.clone()))?;
    let loner = setup::record::Record::open(&dir.join(game)).installed("core");
    Ok(json!({"ok": true, "id": id, "admins": admins, "loner": loner, "welcome": welcome}))
}

// ---- running servers ------------------------------------------------------------------

struct Proc {
    child: Child,
    started: Instant,
    /// when it last answered a query (the frozen-server check)
    answered: Instant,
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
    if let Some(e) = get(id).filter(|e| e.remote.is_some()) {
        return remote_active(&e);
    }
    with_runs(|r| r.procs.contains_key(id)) || get(id).map(|e| left_running(&e)).unwrap_or(false)
}

/// A server on another machine: is its service running? (asked over SSH, kept a few seconds)
fn remote_active(e: &Entry) -> bool {
    remote_state(e).0
}

static SEEN: Mutex<Option<HashMap<String, (Instant, (bool, u64))>>> = Mutex::new(None);
/// Servers whose state is being asked right now (one question at a time per server)
static ASKING: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

/// Servers on other machines that didn't answer the last time they were asked (and why)
static UNREACHABLE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Why the machine a server is on didn't answer last time, if it didn't.
pub(crate) fn unreachable(id: &str) -> Option<String> {
    UNREACHABLE.lock().unwrap().as_ref().and_then(|m| m.get(id).cloned())
}

/// (running, seconds up), asked over SSH and kept a few seconds. While a question is
/// already out, or the connection is busy, the last answer stands. A machine that
/// doesn't answer isn't shown as running, and `unreachable` says why.
fn remote_state(e: &Entry) -> (bool, u64) {
    let Some(r) = &e.remote else { return (false, 0) };
    let last = SEEN.lock().unwrap().as_ref().and_then(|m| m.get(&e.id).cloned());
    let known = || last.map(|(at, (on, secs))| (on, if on { secs + at.elapsed().as_secs() } else { 0 })).unwrap_or((false, 0));
    if last.is_some_and(|(at, _)| at.elapsed() < Duration::from_secs(4)) {
        return known();
    }
    if !ASKING.lock().unwrap().get_or_insert_with(Default::default).insert(e.id.clone()) {
        return if unreachable(&e.id).is_some() { (false, 0) } else { known() };
    }
    let state = crate::remote::active(r, &e.id);
    ASKING.lock().unwrap().get_or_insert_with(Default::default).remove(&e.id);
    match state {
        Ok(state) => {
            SEEN.lock().unwrap().get_or_insert_with(HashMap::new).insert(e.id.clone(), (Instant::now(), state));
            if let Some(m) = UNREACHABLE.lock().unwrap().as_mut() {
                m.remove(&e.id);
            }
            state
        }
        Err(err) if crate::remote::is_busy(&err) => known(),
        Err(err) => {
            UNREACHABLE.lock().unwrap().get_or_insert_with(HashMap::new).insert(e.id.clone(), err);
            (false, 0)
        }
    }
}


/// A server's state for diagnostics, without asking its machine: (state, last error).
/// A server on another machine says what it said last time it was asked.
pub fn last_state(e: &Entry) -> (String, Option<String>) {
    if !e.ready {
        let job = setup::current();
        let busy = job.kind == "server" && job.state == "running";
        return (if busy { "setting up" } else { "setup failed" }.into(), None);
    }
    if e.remote.is_some() {
        let seen = SEEN.lock().unwrap().as_ref().and_then(|m| m.get(&e.id).cloned());
        let state = match seen {
            Some((at, (on, _))) => format!("{} ({})", if on { "running" } else { "stopped" }, crate::diagnostics::ago(at.elapsed().as_secs())),
            None => "not checked yet".into(),
        };
        return (state, unreachable(&e.id).map(|w| format!("machine didn't answer: {w}")));
    }
    let (on, err) = with_runs(|r| (r.procs.contains_key(&e.id), r.errors.get(&e.id).cloned()));
    if on {
        ("running".into(), None)
    } else if left_running(e) {
        ("running (from before the app started)".into(), None)
    } else {
        (if err.is_some() { "crashed" } else { "stopped" }.into(), err)
    }
}

/// A server still up from before the app last closed ("keep servers running"):
/// it answers on its port with its RCON password.
fn left_running(e: &Entry) -> bool {
    // (gm_status: the app's own poll, which the console leaves out; any answer will do,
    // "unknown command" too)
    e.ready && e.rcon().map(|r| r.send("gm_status").is_ok()).unwrap_or(false)
}

pub(crate) fn launch_args(e: &Entry) -> Vec<String> {
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
    if let Some(ip) = e.bind_ip() {
        a.extend(["+ip".into(), ip]);
    }
    // (bots on an empty server aren't a start option: ReGameDLL's game.cfg sets
    // bot_join_after_player back on every map, so friends.cfg carries it, see write_bots_line)
    let extra = extra_args(&e.extra_args).unwrap_or_default();
    let cfg = sys::read_latin1(&e.dir.join(&e.game).join("server.cfg")).unwrap_or_default();
    a.extend(netcode_args(&extra, &cfg, e.remote.is_some()));
    a.extend(extra);
    a
}

/// Whether bots play while nobody's on: the host's choice, else on for a server listed
/// in Steam's browser (so it shows players there), off for the others.
pub fn bots_when_empty(e: &Entry) -> bool {
    e.bots_when_empty.unwrap_or(e.visibility == "listed")
}

/// Netcode defaults for a dedicated server (never the player's own listen server, which
/// doesn't come through here): a higher tick rate and update/rate limits that suit
/// today's connections, so what players see is closer to their real ping. A higher
/// sys_ticrate costs CPU, and bots think every tick too: 300 (players get at most 101 updates a
/// second, and on shared-CPU hosts 300 holds steadier than 500; measured on a Hetzner CPX21).
/// Anything the host set in the extra start options or the server's own server.cfg wins
/// (left out here). On a Linux machine -pingboost 1 too (not under Windows or wine,
/// where it does nothing useful).
pub(crate) const NETCODE: &[(&str, &str)] = &[
    ("sys_ticrate", "300"),
    ("sv_maxupdaterate", "101"),
    ("sv_minupdaterate", "30"),
    ("sv_maxrate", "100000"),
    ("sv_minrate", "25000"),
];

fn netcode_args(extra: &[String], server_cfg: &str, linux: bool) -> Vec<String> {
    let given = |name: &str| extra.iter().any(|w| w.trim_start_matches(['+', '-']).eq_ignore_ascii_case(name));
    let mut a = Vec::new();
    for (cvar, value) in NETCODE {
        if !given(cvar) && cfg_value(server_cfg, cvar).is_none() {
            a.extend([format!("+{cvar}"), value.to_string()]);
        }
    }
    if linux && !given("pingboost") {
        a.extend(["-pingboost".into(), "1".into()]);
    }
    a
}

/// .../Bottles/<bottle>/drive_c/Some Folder/hlds.exe -> C:\Some Folder\hlds.exe
pub(crate) fn windows_path(p: &Path) -> Option<String> {
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
    e.pin_address();
    // it may come up with other plugins: its console suggestions are asked for again
    crate::consolehints::forget(&e.id);
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
    // not tied to the app: whether servers stop with it is decided when it quits
    // (main.rs), so turning "Keep servers running" on also keeps the ones already going.
    // Its pid is kept, so one the app lost track of (a crash) can still be ended.
    let _ = std::fs::write(pid_file(e), child.id().to_string());
    Ok(child)
}

fn pid_file(e: &Entry) -> PathBuf {
    e.dir.join("loner-strike-server.pid")
}

/// Ends this server's process from an earlier session that the app lost track of
/// (it crashed or was force-quit), if it's still there: only when the pid is still
/// this server (its path in the command line on a Mac, hlds.exe on Windows).
fn end_leftover(e: &Entry) {
    let file = pid_file(e);
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let _ = std::fs::remove_file(&file);
    #[cfg(unix)]
    if let Ok(pid) = text.trim().parse::<i32>() {
        let mut c = sys::command("ps");
        c.args(["-p", &pid.to_string(), "-o", "command="]);
        if let Some((_, out, _)) = sys::run_output(c, Duration::from_secs(3)) {
            // CrossOver's wine is given the server's Windows path (C:\...\hlds.exe)
            let win = windows_path(&e.hlds());
            if out.contains(&*e.dir.to_string_lossy()) || win.is_some_and(|w| !w.is_empty() && out.contains(&w)) {
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
    }
    #[cfg(windows)]
    if let Ok(pid) = text.trim().parse::<u32>() {
        let mut c = sys::command("tasklist");
        c.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
        if let Some((_, out, _)) = sys::run_output(c, Duration::from_secs(3)) {
            if out.to_lowercase().contains("hlds.exe") {
                let mut k = sys::command("taskkill");
                k.args(["/PID", &pid.to_string(), "/F"]);
                let _ = sys::run_output(k, Duration::from_secs(5));
            }
        }
    }
}

/// One start at a time, so a double click (or the crash watcher racing a restart) can't
/// launch two copies; a restore (backup.rs) takes it too while it marks a server as busy.
pub(crate) static STARTING: Mutex<()> = Mutex::new(());

/// Servers on other machines being started, and how many starts of each are going (sync,
/// service and systemctl take a while over SSH, and local starts mustn't wait on them, so
/// STARTING isn't held for it). A restore checks this with STARTING held and refuses one
/// that's starting.
static REMOTE_STARTS: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);

/// Marks a server on another machine as starting until it's dropped (the end of start()).
struct RemoteStart(String);

impl RemoteStart {
    fn new(id: &str) -> RemoteStart {
        *REMOTE_STARTS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).entry(id.to_string()).or_insert(0) += 1;
        RemoteStart(id.to_string())
    }
}

impl Drop for RemoteStart {
    fn drop(&mut self) {
        if let Some(all) = REMOTE_STARTS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            if let Some(n) = all.get_mut(&self.0) {
                *n -= 1;
                if *n == 0 {
                    all.remove(&self.0);
                }
            }
        }
    }
}

/// Whether a server on another machine is being started right now (call with STARTING held).
pub(crate) fn remote_starting(id: &str) -> bool {
    REMOTE_STARTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.contains_key(id))
}

pub fn start(id: &str) -> Result<(), String> {
    let e = get(id).ok_or("That server isn't there any more.")?;
    if crate::backup::restoring_server(id) {
        return Err(crate::backup::RESTORING_SERVER.into());
    }
    if !e.ready {
        return Err("This server isn't finished setting up.".into());
    }
    // whether bots play on an empty server and whether it's listed (friends.cfg runs after every map)
    let bots_changed = write_bots_line(&e).unwrap_or(false);
    if e.ls_listed {
        // the list checks the server itself, so once it's had time to load a map
        let address = e.connect();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(45));
            if let Err(why) = crate::browser::register(&address) {
                crate::diagnostics::log_error("server list", &why);
            }
        });
    }
    // (systemd starts one copy at most, and a slow machine mustn't hold up local starts)
    if let Some(r) = &e.remote {
        // it may come up with other plugins: its console suggestions are asked for again
        crate::consolehints::forget(id);
        // checked with starts held, and marked as starting until systemctl has answered, so a
        // restore can't begin anywhere in between (it refuses a server that's starting)
        let _starting = {
            let _g = STARTING.lock().unwrap_or_else(|e| e.into_inner());
            if crate::backup::restoring_server(id) {
                return Err(crate::backup::RESTORING_SERVER.into());
            }
            RemoteStart::new(id)
        };
        if bots_changed {
            let _ = sync(&e, false);
        }
        // start options saved while the machine couldn't be reached go there first; and a
        // unit an older Loner-Strike wrote (its sandbox) is written again, without asking
        if e.service_stale || !crate::remote::service_current(r, &e.id).unwrap_or(true) {
            write_service(&e)?;
        }
        // (and its HLTV unit, the same way)
        if e.hltv.enabled && !crate::remote::hltv_current(r, &e.id).unwrap_or(true) {
            let _ = crate::hltv::remote_apply(&e, true);
        }
        // the fixed Ham Sandwich (the official one crashes on map changes): a server set
        // up before it existed gets it here, at a start
        if let Err(why) = crate::remote::ensure_ham(r, &e.id, &e.game) {
            crate::diagnostics::log_error("ham sandwich", &why);
        }
        clear_pending_restart(&e.id);
        return crate::remote::systemctl(r, &e.id, "start");
    }
    let _g = STARTING.lock().unwrap_or_else(|e| e.into_inner());
    // checked again now that a restore can't begin in between
    if crate::backup::restoring_server(id) {
        return Err(crate::backup::RESTORING_SERVER.into());
    }
    if with_runs(|r| r.procs.contains_key(id)) || left_running(&e) {
        return Ok(());
    }
    // one from before that doesn't answer (hung, or still shutting down) would hold the port
    end_leftover(&e);
    // the bots' newer navigation files from this computer's games
    crate::navs::copy_to_server(&e, None, true);
    if let Some(other) = load().iter().find(|o| o.id != e.id && o.port == e.port && with_runs(|r| r.procs.contains_key(&o.id))) {
        return Err(format!("\"{}\" is using port {} already. Stop it first, or change this server's port.", other.name, e.port));
    }
    let child = spawn(&e)?;
    with_runs(|r| {
        r.errors.remove(id);
        r.procs.insert(id.into(), Proc { child, started: Instant::now(), answered: Instant::now(), stopping: false });
    });
    clear_pending_restart(&e.id);
    crate::hltv::server_started(&e);
    watch();
    Ok(())
}

/// Asks the server to quit (RCON), then ends it if it hasn't after a few seconds.
pub fn stop(id: &str) {
    let e = get(id);
    if let Some(e) = e.as_ref().filter(|e| e.remote.is_some()) {
        clear_pending_restart(&e.id);
        // HLTV's demo finished first (its service stops with the server's anyway)
        if e.hltv.enabled {
            crate::hltv::stop(e);
        }
        let _ = crate::remote::systemctl(e.remote.as_ref().unwrap(), &e.id, "stop");
        return;
    }
    // HLTV first, so its demo is finished properly
    if let Some(e) = &e {
        crate::hltv::stop(e);
        clear_pending_restart(&e.id);
    }
    let child = with_runs(|r| {
        r.procs.get_mut(id).map(|p| p.stopping = true);
        r.procs.remove(id).map(|p| p.child)
    });
    let Some(mut child) = child else {
        // one left running from before: ask it to quit, and wait until it has (so a
        // restart right after doesn't find it still there and skip starting it)
        if let Some(e) = &e {
            if let Some(rcon) = e.rcon() {
                let _ = rcon.send("quit");
            }
            for _ in 0..30 {
                if !left_running(e) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            end_leftover(e);
        }
        return;
    };
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

/// Stops every server this session started, all at once (each can take seconds).
/// Remove everything: stops and deletes every server on this computer (only folders it
/// made), and the servers folders' own working copies when they're left empty. Servers on
/// other machines aren't touched. What couldn't be deleted, for the summary.
pub fn remove_local_all() -> (Vec<String>, Vec<String>) {
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    let guard = sync_guard();
    let mut roots = std::collections::BTreeSet::new();
    for e in load().into_iter().filter(|e| e.remote.is_none()) {
        stop(&e.id);
        if let Some(root) = e.dir.parent() {
            roots.insert(root.to_path_buf());
        }
        if made_here(&e) && e.dir.is_dir() {
            match std::fs::remove_dir_all(&e.dir) {
                Ok(()) => done.push(e.name.clone()),
                Err(err) => failed.push(format!("{} ({err})", e.dir.display())),
            }
        } else {
            done.push(e.name.clone());
        }
    }
    drop(guard);
    // the servers folders: their shared download and the bots' map learner, then the
    // folders themselves if nothing else is in them
    for root in roots {
        for sub in [".base", ".nav-learner"] {
            let d = root.join(sub);
            if d.is_dir() {
                if let Err(err) = std::fs::remove_dir_all(&d) {
                    failed.push(format!("{} ({err})", d.display()));
                }
            }
        }
        let _ = std::fs::remove_dir(&root);
        if let Some(parent) = root.parent().filter(|p| p.file_name().is_some_and(|n| n == "Loner-Strike")) {
            let _ = std::fs::remove_dir(parent);
        }
    }
    (done, failed)
}

/// Whether two versions of a server differ only in this app's own switches.
fn same_but_app_switches(a: &Entry, b: &Entry) -> bool {
    let strip = |e: &Entry| {
        let mut v = serde_json::to_value(e).unwrap_or_default();
        if let Some(o) = v.as_object_mut() {
            for k in ["keep_running", "notify_joins", "notify_helpers", "auto_update"] {
                o.remove(k);
            }
        }
        v
    };
    strip(a) == strip(b)
}

/// Whether a server's joins show as notifications.
pub(crate) fn notifies_joins(e: &Entry) -> bool {
    e.notify_joins.unwrap_or_else(|| !crate::app_api::load_settings().mute_joins)
}

/// Whether what helpers do on a server shows as notifications.
pub(crate) fn notifies_helpers(e: &Entry) -> bool {
    e.notify_helpers.unwrap_or_else(|| !crate::app_api::load_settings().mute_helpers)
}

/// Whether a server on this computer keeps running after the app closes.
pub(crate) fn keeps_running(e: &Entry) -> bool {
    e.keep_running.unwrap_or_else(|| crate::app_api::load_settings().keep_servers)
}

/// The app is closing: servers on this computer stop, except the ones set to keep running.
pub fn stop_on_exit() {
    let ids: Vec<String> = with_runs(|r| r.procs.keys().cloned().collect());
    let threads: Vec<_> = ids.into_iter().filter(|id| !get(id).is_some_and(|e| keeps_running(&e))).map(|id| std::thread::spawn(move || stop(&id))).collect();
    for t in threads {
        let _ = t.join();
    }
}


/// Switches a server that's just been started to `mode` once it answers (a server starts in
/// the mode it was last in; the plugin reloads the map for a new one). Gives up after a few
/// minutes: a server that never came up has its own message.
fn start_in_mode(id: &str, mode: &str) {
    let (id, mode) = (id.to_string(), mode.to_string());
    std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(240);
        while Instant::now() < until {
            std::thread::sleep(Duration::from_secs(3));
            let Some(g) = game_for(&id, || None) else { continue };
            let st = g.state();
            if st["online"] != json!(true) {
                continue;
            }
            if st["state"]["mode"].as_str() != Some(mode.as_str()) {
                if let Err(e) = g.command("gm_mode", &[mode.clone()]) {
                    eprintln!("server {id}: couldn't switch to {mode} after starting: {e:?}");
                }
            }
            return;
        }
    });
}

/// A server that froze (still running, no longer answering) for this long is ended, and
/// the crash watcher starts it again. Any answer counts; a map change takes far less.
const FROZEN_AFTER: Duration = Duration::from_secs(90);
/// Not checked this soon after it starts (loading the first map)
const FROZEN_GRACE: Duration = Duration::from_secs(120);

/// Ends servers that stopped answering (on their own thread: a query can take a moment).
fn watch_frozen() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(15));
        let due: Vec<String> = with_runs(|r| r.procs.iter().filter(|(_, p)| !p.stopping && p.started.elapsed() > FROZEN_GRACE).map(|(id, _)| id.clone()).collect());
        for id in due {
            let Some(e) = get(&id) else { continue };
            let answers = crate::browser::info(&format!("{}:{}", e.local_host(), e.port)).is_some();
            with_runs(|r| {
                let Some(p) = r.procs.get_mut(&id) else { return };
                if answers {
                    p.answered = Instant::now();
                } else if p.answered.elapsed() > FROZEN_AFTER && p.started.elapsed() > FROZEN_GRACE {
                    eprintln!("server {id} hasn't answered for {} s: ending it so it starts again", FROZEN_AFTER.as_secs());
                    let _ = p.child.kill();
                    // (the crash watcher sees it end and starts it again)
                    p.answered = Instant::now();
                }
            });
        }
    });
}

/// Restarts servers that crash (up to MAX_RESTARTS times in CRASH_WINDOW).
fn watch() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        watch_frozen();
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
                    // a restore stopped it on purpose: nothing to report on its card
                    if e != crate::backup::RESTORING_SERVER {
                        with_runs(|r| r.errors.insert(id.clone(), e));
                    }
                }
            }
        });
    });
}

/// The last lines of the server's console (qconsole.log, from -condebug).
fn console(e: &Entry) -> Vec<String> {
    if let Some(r) = &e.remote {
        return without_polls(crate::remote::console(r, &e.id, &e.game), "127.0.0.1", &|n| crate::consolehints::is_setting(&e.id, n)).iter().map(|l| hide_passwords(l)).collect();
    }
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
    let mut lines = without_polls(lines, &e.local_host(), &|n| crate::consolehints::is_setting(&e.id, n));
    let skip = lines.len().saturating_sub(300);
    lines.split_off(skip).into_iter().map(|l| hide_passwords(&l)).collect()
}

// (shared with ls-control, the shared control service on a Linux server)
use crate::control::common::{hide_passwords, without_polls};

pub(crate) fn view(e: &Entry) -> Value {
    // sounds wait for a map change: looked at only while something's waiting
    let pending = if e.pending_restart.sounds {
        check_sounds_pending(e);
        get(&e.id).map(|x| x.pending_restart).unwrap_or_else(|| e.pending_restart.clone())
    } else {
        e.pending_restart.clone()
    };
    let installed = setup::record::component_ids_cached(&e.dir.join(&e.game));
    let loner = installed.iter().any(|c| c == "core");
    let (state, uptime, error) = if e.remote.is_some() && e.ready {
        let (on, secs) = remote_state(e);
        // never had an answer yet (the connection busy with an upload, say): not known
        // to be stopped either
        let asked = SEEN.lock().unwrap().as_ref().is_some_and(|m| m.contains_key(&e.id)) || unreachable(&e.id).is_some();
        (if on { "running" } else if asked { "stopped" } else { "checking" }, secs, None)
    } else {
        with_runs(|r| match r.procs.get(&e.id) {
            Some(p) => ("running", p.started.elapsed().as_secs(), None),
            None => (if r.errors.contains_key(&e.id) { "crashed" } else { "stopped" }, 0, r.errors.get(&e.id).cloned()),
        })
    };
    // still up from before the app last closed
    let (state, error) = if state == "stopped" && e.remote.is_none() && left_running(e) { ("running", None) } else { (state, error) };
    // not ready: setting up while a server setup job runs; otherwise it stopped part way
    // (an error, or the app closed during it) and can only be deleted
    let (state, error) = if e.ready {
        (state, error)
    } else {
        let job = setup::current();
        if job.kind == "server" && job.state == "running" {
            ("setting-up", None)
        } else {
            let why = (job.kind == "server").then_some(job.error).flatten();
            ("failed", Some(why.map(|w| format!("Setting it up stopped: {w}")).unwrap_or_else(|| "Setting it up didn't finish. Delete it and make it again.".into())))
        }
    };
    json!({"id": e.id, "name": e.name, "game": e.game, "port": e.port, "visibility": e.visibility, "map": e.map,
           "maxplayers": e.maxplayers, "ready": e.ready, "state": state, "uptime": uptime, "error": error,
           "imported": e.imported, "loner": loner, "dir": e.dir,
           "host": e.remote.as_ref().map(|r| r.host.clone()), "address": e.address(),
           "public_address": e.public_address, "connect": e.connect(), "pending_restart": pending.view(),
           "listen_ip": e.listen_ip, "extra_args": e.extra_args, "password": join_password(e),
           "bots_when_empty": bots_when_empty(e), "bots_when_empty_set": e.bots_when_empty.is_some(), "keep_running": e.remote.is_none() && keeps_running(e), "notify_joins": notifies_joins(e), "notify_helpers": notifies_helpers(e), "ls_listed": e.ls_listed,
           "unreachable": e.remote.as_ref().and_then(|_| unreachable(&e.id)),
           // on another machine the daily restart is done by the app: only while it's open
           "restart_needs_app": e.remote.is_some() && e.restart.as_ref().is_some_and(|r| r.on),
           "service_stale": e.service_stale,
           "auto_update": e.auto_update != Some(false),
           "installed": installed})
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
        let mut g = Game::on_port(&e.dir, &e.game, e.port, true);
        if let Some(r) = &e.remote {
            if let Some(rc) = e.rcon() {
                g.rcon = Arc::new(rc);
            }
            // remote paths are relative to the user's home (sftp starts there)
            g.remote = Some((r.clone(), format!("{}/{}", crate::remote::server_dir(&e.id), e.game)));
        } else if e.bind_ip().is_some() {
            // a server bound to one address doesn't answer on 127.0.0.1
            if let Some(rc) = e.rcon() {
                g.rcon = Arc::new(rc);
            }
        }
        g
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

pub(crate) fn forget(target: &str) {
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
pub(crate) fn client(st: &Shared) -> Option<Install> {
    let local = st.local_game()?;
    crate::steam::find_installs().into_iter().find(|i| i.hl == local.hl)
}

async fn api_list(State(st): State<Shared>, on: Option<axum::Extension<crate::server::OnComputer>>) -> Json<Value> {
    // servers friends shared with you are this computer's app's (a phone can't reach them)
    let with_shared = on.is_some();
    let res = crate::setup_api::resources_for(&st).ok();
    // view() can ask a server left running over RCON: not on the async workers
    let v = tokio::task::spawn_blocking(move || {
        let mut list: Vec<Value> = load()
            .iter()
            .map(|e| {
                let mut v = view(e);
                // Loner-Strike's parts older than this app's, and whether they're being put in
                let outdated = res.as_ref().map(|r| crate::setup_api::server_outdated(r, e)).unwrap_or_default();
                v["outdated"] = json!(outdated);
                let updating = crate::setup_api::updating(&e.id);
                v["updating"] = json!(updating.is_some());
                // (started again when its updates are in: it was running)
                v["restarts_after"] = json!(updating == Some(true));
                v["update_queued"] = json!(crate::setup_api::queued(&e.id));
                let (phase, job) = crate::setup_api::update_phase(&e.id).unzip();
                v["update_phase"] = json!(phase);
                v["update_job"] = json!(job.filter(|j| !j.is_empty()));
                v
            })
            .collect();
        // servers friends shared with you, after your own
        if with_shared {
            list.extend(crate::control::client::list());
        }
        json!({"servers": list, "steamcmd": steamcmd_exe().is_file()})
    })
    .await
    .unwrap_or_else(|_| json!({"servers": [], "steamcmd": false}));
    Json(v)
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

async fn api_create_remote(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<NewRemote>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let res = crate::setup_api::resources_for(&st);
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || {
        let client = client(&st2).ok_or("Set up the game on this computer first: the server's plugins are built with it.")?;
        create_remote(req, client, res?).map(|(id, job)| json!({"ok": true, "id": id, "job": job}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

/// The app window only: a first look at a machine (its fingerprint to confirm, and
/// whether it can run the server) before setting it up.
async fn api_probe_remote(headers: HeaderMap, Json(req): Json<NewRemote>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || {
        let info = remote_of(&req)?;
        let (fingerprint, known, arch) = crate::remote::probe(&info)?;
        Ok(json!({"ok": true, "fingerprint": fingerprint, "known": known, "arch": arch, "x86": arch == "x86_64"}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

/// The app window only: a file picker for the SSH key.
async fn api_pick_key(State(st): State<Shared>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err("Not available.".into())) };
    let r = tokio::task::spawn_blocking(move || {
        let start = dirs::home_dir().map(|h| h.join(".ssh")).filter(|p| p.is_dir());
        let mut d = app.dialog().file().set_title("Pick the SSH key you log in to the server with (the file without .pub)");
        if let Some(s) = start {
            d = d.set_directory(s);
        }
        match d.blocking_pick_file().and_then(|p| p.into_path().ok()) {
            Some(p) if p.extension().map(|e| e == "pub").unwrap_or(false) => Err("That's the public half (.pub). Pick the file with the same name without .pub.".to_string()),
            Some(p) => Ok(json!({"ok": true, "path": p})),
            None => Ok(json!({"cancelled": true})),
        }
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

async fn api_create(State(st): State<Shared>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, Json(req): Json<NewServer>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let res = crate::setup_api::resources_for(&st);
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || {
        let client = client(&st2).ok_or("Set up the game first: servers are made from it.")?;
        create(req, client, res?).map(|(id, job)| json!({"ok": true, "id": id, "job": job}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

#[derive(Deserialize, Default)]
struct Change {
    name: Option<String>,
    visibility: Option<String>,
    map: Option<String>,
    maxplayers: Option<u32>,
    port: Option<u16>,
    /// the join password ("" for none)
    password: Option<String>,
    listen_ip: Option<String>,
    extra_args: Option<String>,
    /// bots keep playing with nobody on
    bots_when_empty: Option<bool>,
    /// on this computer: keep running after the app closes
    keep_running: Option<bool>,
    notify_joins: Option<bool>,
    notify_helpers: Option<bool>,
    /// updates put in by themselves once nobody is playing
    auto_update: Option<bool>,
    /// a domain name shown in place of the IP ("" clears it)
    public_address: Option<String>,
    /// on the Loner-Strike server list
    ls_listed: Option<bool>,
}

async fn api_change(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, helper: Option<axum::Extension<crate::control::Helper>>, Json(req): Json<Change>) -> (StatusCode, Json<Value>) {
    // the app window, or an Admin helping run this server (their request only carries the
    // settings their role changes: control::rules trimmed it)
    if !from_panel(&headers) || (on.is_none() && helper.is_none()) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
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
        let host = |o: &Entry| o.remote.as_ref().map(|r| r.host.clone());
        if let Some(p) = req.port {
            let all = load();
            let taken = all.iter().any(|o| o.id != id && o.port == p && host(o) == host(&e));
            // HLTV's ports count too, this server's own included
            let hltv = all.iter().any(|o| host(o) == host(&e) && o.hltv.enabled && (if o.id == id { o.hltv.port == Some(p) } else { crate::hltv::port_of(o) == p }));
            if hltv {
                return Err(format!("Port {p} is used by HLTV. Pick another one."));
            }
            if e.remote.is_some() && (!REMOTE_PORTS.contains(&p) || taken) {
                return Err("Pick a port from 27015 to 27019 that no other server on that machine uses (27020 is for HLTV).".into());
            }
            // (servers this app made start at 27016: 27015 is your own game's)
            let lowest = if e.imported { 1024 } else { FIRST_PORT };
            if e.remote.is_none() && (!(lowest..=65000).contains(&p) || taken) {
                return Err(format!("Pick a port from {lowest} to 65000 that no other server uses."));
            }
            if running(&id) && p != e.port {
                return Err("Stop the server to change its port.".into());
            }
        }
        let listen_ip = req.listen_ip.as_ref().map(|t| t.trim().to_string());
        if let Some(ip) = listen_ip.as_ref().filter(|t| !t.is_empty()) {
            if e.remote.is_some() {
                return Err("A server on a Linux machine listens on all of its addresses. Leave the listen address empty.".into());
            }
            if ip.parse::<std::net::Ipv4Addr>().is_err() {
                return Err("The listen address is an IP address like 192.168.1.20, or empty for all of them.".into());
            }
        }
        let public_address = req.public_address.as_deref().map(clean_public_address).transpose()?;
        let extra = req.extra_args.as_ref().map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
        if let Some(t) = &extra {
            extra_args(t)?;
        }
        let password = req.password.as_ref().map(|p| p.trim().to_string());
        if password.as_ref().is_some_and(|p| !crate::sharing::password_ok(p)) {
            return Err("Passwords can use letters, numbers, - and _ (up to 24).".into());
        }
        let restart_needed = |old: &Entry, new: &Entry| old.port != new.port || old.visibility != new.visibility || old.maxplayers != new.maxplayers || old.listen_ip != new.listen_ip || old.extra_args != new.extra_args;
        let name = req.name.as_ref().map(|n| n.trim().chars().filter(|c| !c.is_control() && *c != '"').take(48).collect::<String>());
        if name.as_ref().is_some_and(|n| n.is_empty()) {
            return Err("Give the server a name.".into());
        }
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
                if let Some(ip) = &listen_ip {
                    e.listen_ip = ip.clone();
                }
                if let Some(t) = &extra {
                    e.extra_args = t.clone();
                }
                if let Some(k) = req.keep_running {
                    e.keep_running = Some(k);
                }
                if let Some(n) = req.notify_joins {
                    e.notify_joins = Some(n);
                }
                if let Some(n) = req.notify_helpers {
                    e.notify_helpers = Some(n);
                }
                if let Some(a) = req.auto_update {
                    e.auto_update = Some(a);
                }
                if let Some(b) = req.bots_when_empty {
                    e.bots_when_empty = Some(b);
                }
                if let Some(a) = &public_address {
                    e.public_address = a.clone();
                }
                if let Some(l) = req.ls_listed {
                    e.ls_listed = l;
                }
            }
        })?;
        let now = get(&id).ok_or("That server isn't there any more.")?;
        // bots on an empty server: a cvar, so a running server takes it now (and the start
        // options carry it from the next start); only if it can't be told, a restart
        let bots_live = if bots_when_empty(&e) != bots_when_empty(&now) && running(&id) {
            now.rcon().is_some_and(|rc| rc.send(&format!("bot_join_after_player {}", if bots_when_empty(&now) { 0 } else { 1 })).is_ok())
        } else {
            true
        };
        // the name players see and the join password, applied now if it's running
        // the list setting, told to a running server now (friends.cfg keeps it for later maps)
        if e.ls_listed != now.ls_listed && running(&id) {
            if let Some(rc) = now.rcon() {
                let _ = rc.send(&format!("ls_list {}", if now.ls_listed { 1 } else { 0 }));
            }
        }
        if name.is_some() || password.is_some() || bots_when_empty(&e) != bots_when_empty(&now) || e.ls_listed != now.ls_listed {
            let pw = password.clone().unwrap_or_else(|| join_password(&now));
            write_join(&now.dir.join(&now.game).join("addons/amxmodx/data/gamemodes"), &now.name, &pw, bots_when_empty(&now), now.ls_listed)?;
            if running(&id) {
                if let Some(rc) = now.rcon() {
                    let _ = rc.send(&format!("hostname \"{}\";sv_password \"{pw}\"", crate::sharing::clean_name(&now.name)));
                }
            }
        }
        // the cached connection to it knows the old port and address (bind_ip: the listen
        // address, or the automatic one, which visibility turns on and off)
        if now.port != e.port || now.bind_ip() != e.bind_ip() {
            forget(&id);
        }
        // on another machine the start options live in its service: write it again (and
        // bring its files up to date with this computer's copy while at it)
        let old = e.clone();
        let restart = running(&id) && (restart_needed(&e, &now) || !bots_live);
        if restart {
            set_pending_restart(&id, Pending::Options);
        }
        // (only this app's own switches changed, notifications or keeping it running: nothing
        // on the machine changes, so it needn't be reachable)
        if let Some(e) = get(&id).filter(|e| e.ready && e.remote.is_some() && !same_but_app_switches(&old, e)) {
            if restart_needed(&old, &e) {
                // (noted first: if the machine can't be reached it's written before the next start)
                let _ = edit(|l| l.iter_mut().filter(|x| x.id == id).for_each(|x| x.service_stale = true));
            }
            sync(&e, false)?;
            write_service(&e)?;
            // HLTV's settings name the server's port and password: written again to match
            if e.hltv.enabled && (old.port != e.port || old.visibility != e.visibility || password.is_some()) {
                crate::hltv::remote_apply(&e, true)?;
            }
            // the website status asks the game on its port
            if e.web_status.enabled && old.port != e.port {
                crate::webstatus::apply(&e, true)?;
            }
        }
        // just put on the list: the list checks the server itself now (if it's down, the
        // next start asks again)
        let listing = if now.ls_listed && !e.ls_listed && running(&id) {
            match crate::browser::register(&now.connect()) {
                Ok(()) => json!({"ok": true}),
                Err(why) => json!({"ok": false, "error": why}),
            }
        } else {
            Value::Null
        };
        // (a visibility change alone needs a restart anyway; the bots part was applied above)
        Ok(json!({"ok": true, "restart": restart, "public_address": now.public_address, "connect": now.connect(), "listing": listing}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

/// Starts this computer's game joining a server: told by Steam when the game is open
/// (closed and started again for an unlisted server once `restart` says so), else started
/// with the address. Your own servers' Join and a shared server's (control::client::join).
pub(crate) fn launch_join(st: &Shared, game: &str, address: &str, password: Option<String>, unlisted: bool, restart: bool, on_computer: bool) -> Result<Value, String> {
    let client = client(st).ok_or("Couldn't find the game to join with.")?;
    // the game already open: Steam tells it to join; otherwise it starts joining
    if sys::game_running() && unlisted {
        // an unlisted server doesn't answer the game's lookups, so the open game
        // can't be told to join it: it has to start again with the address
        if !restart {
            return Ok(json!({"ok": true, "need_restart_game": true}));
        }
        // closing the game is for the person at the computer, not a phone
        if !on_computer {
            return Err("Close the game on the computer first, then press Join.".into());
        }
        sys::close_game()?;
    } else if sys::game_running() {
        let url = match &password {
            Some(p) => format!("steam://connect/{address}/{p}"),
            None => format!("steam://connect/{address}"),
        };
        return crate::steam::open_steam_url(&client.hl, &url).map(|_| json!({"ok": true}));
    }
    let mut args = Vec::new();
    if let Some(p) = password {
        args.extend(["+password".to_string(), p]);
    }
    args.extend(["+connect".to_string(), address.to_string()]);
    let appid = if game == "cstrike" { "10" } else { "80" };
    crate::steam::launch(&client.hl, appid, &args).map(|_| json!({"ok": true}))
}

/// The modes a server has (from its copy of modes.ini on this computer) and the one it was
/// last in, for starting it in a mode
async fn api_modes(UrlPath(id): UrlPath<String>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let e = get(&id).ok_or("That server isn't there any more.")?;
        let am = e.dir.join(&e.game).join("addons/amxmodx");
        let modes = crate::extras::parse_modes(&sys::read_latin1(&am.join("configs/gamemodes/modes.ini")).unwrap_or_default());
        let current = std::fs::read_to_string(am.join("data/gamemodes/current.txt")).ok().map(|t| t.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "classic".into());
        Ok(json!({"modes": modes.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(), "mode": current}))
    })
    .await
    .unwrap_or_else(|_| Err("Couldn't read the server's modes.".into()));
    answer(r)
}

async fn api_action(State(st): State<Shared>, UrlPath((id, action)): UrlPath<(String, String)>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, body: Option<Json<Value>>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return forbidden();
    }
    let body = body.map(|Json(b)| b).unwrap_or_default();
    // a phone may start, stop and restart servers; changing what's installed or deleting
    // one (which also reaches a VPS through the SSH key) is for the computer
    if on.is_none() && !matches!(action.as_str(), "start" | "stop" | "restart" | "join") {
        return answer(Err("Do that from the computer running Loner-Strike.".into()));
    }
    if matches!(action.as_str(), "delete" | "rebuild" | "retry") && crate::backup::restoring_server(&id) {
        return answer(Err(crate::backup::RESTORING_SERVER_BUSY.into()));
    }
    let st2 = st.clone();
    let on_computer = on.is_some();
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        match action.as_str() {
            "start" => {
                // (its files are being changed: it's started again when they're in if it was running)
                if crate::setup_api::updating(&id).is_some() {
                    return Err("Its updates are going in. It starts again by itself if it was running; otherwise start it when they're in.".into());
                }
                start(&id)?;
                // started in a mode picked beforehand: switched to it once it's up
                if let Some(mode) = body["mode"].as_str().filter(|m| !m.is_empty() && m.len() <= 31 && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
                    start_in_mode(&id, mode);
                }
                Ok(json!({"ok": true}))
            }
            "stop" => {
                stop(&id);
                Ok(json!({"ok": true}))
            }
            "restart" => {
                if crate::setup_api::updating(&id).is_some() {
                    return Err("Its updates are going in. It starts again by itself when they're in.".into());
                }
                stop(&id);
                start(&id).map(|_| json!({"ok": true}))
            }
            // Loner-Strike's updates for it now (its plugins and bot brain), stopped and started
            // again around it if it's running; asks first when people are playing
            "apply-updates" => crate::setup_api::apply_updates(&st2, &id, body["force"].as_bool() == Some(true)),
            "update" => {
                let e = get(&id).ok_or("That server isn't there any more.")?;
                // (a Linux server's shared control program comes from the app's resources)
                let res = e.remote.as_ref().map(|_| crate::setup_api::resources_for(&st2)).transpose()?;
                update_files(e, res).map(|job| json!({"ok": true, "job": job}))
            }
            "join" => {
                let e = get(&id).ok_or("That server isn't there any more.")?;
                let pw = Some(join_password(&e)).filter(|p| !p.is_empty());
                launch_join(&st2, &e.game, &e.join_address(), pw, e.visibility == "unlisted", body["restart"].as_bool().unwrap_or(false), on_computer)
            }
            "rebuild" => {
                // a server on another machine, wiped and set up again with the same details
                let e = get(&id).ok_or("That server isn't there any more.")?;
                let r = e.remote.clone().ok_or("Rebuild from scratch is for servers on another machine.")?;
                let client = client(&st2).ok_or("Set up the game on this computer first.")?;
                let res = crate::setup_api::resources_for(&st2)?;
                // everything that can say no goes first, so a refusal leaves the server as it was
                if setup::current().state == "running" {
                    return Err("Something else is being set up. Wait for it to finish, then rebuild.".into());
                }
                let mut with = e.setup_with.clone().unwrap_or(SetupWith { template: "local".into(), components: vec![], password: String::new() });
                // the join password lives in this computer's copy, which is about to go
                let pw = join_password(&e);
                if !pw.is_empty() {
                    with.password = pw;
                }
                let catalog = setup::components::load_catalog(&res)?;
                let (source, components) = match template_source(&with.template, &e.game, &client, &catalog) {
                    Some((dir, comps)) => (Some(dir), comps),
                    None => (None, with.components.clone()),
                };
                let guard = sync_guard();
                crate::remote::remove(&r, &e.id, false)?;
                edit(|l| {
                    l.iter_mut().filter(|x| x.id == id).for_each(|x| {
                        x.ready = false;
                        x.setup_with = Some(with.clone());
                    })
                })?;
                // this computer's copy too (only ever a folder we made: <servers root>/<id>)
                let ok_dir = made_here(&e);
                if ok_dir && e.dir.is_dir() {
                    std::fs::remove_dir_all(&e.dir).map_err(|err| format!("Couldn't clear this computer's copy ({err})."))?;
                }
                drop(guard);
                // (after the sync lock: shared lists take theirs first, then sync)
                crate::lists::forget(&id);
                forget(&id);
                let mut e = e;
                e.ready = false;
                e.setup_with = Some(with);
                let job = remote_setup(e, source, components, res)?;
                Ok(json!({"ok": true, "job": job}))
            }
            "retry" => {
                // a setup that stopped part way, again with the same details
                let e = get(&id).ok_or("That server isn't there any more.")?;
                // a fresh setup wouldn't bring the backup's settings, admins or passwords
                if crate::backup::restore_unfinished(&id) {
                    return Err("This server was made by a restore that didn't finish. Delete it in the list, then restore the backup again.".into());
                }
                if e.ready || e.remote.is_none() {
                    return Err("Only a server on another machine whose setup didn't finish can be tried again.".into());
                }
                let client = client(&st2).ok_or("Set up the game on this computer first.")?;
                let res = crate::setup_api::resources_for(&st2)?;
                let with = e.setup_with.clone().unwrap_or(SetupWith { template: "local".into(), components: vec![], password: String::new() });
                let catalog = setup::components::load_catalog(&res)?;
                let (source, components) = match template_source(&with.template, &e.game, &client, &catalog) {
                    Some((dir, comps)) => (Some(dir), comps),
                    None => (None, with.components.clone()),
                };
                let job = remote_setup(e, source, components, res)?;
                Ok(json!({"ok": true, "job": job}))
            }
            "delete" => {
                let e = get(&id).ok_or("That server isn't there any more.")?;
                // no upload starts while it goes (one would make its folders again)
                let guard = sync_guard();
                // on another machine: its service and folder there go too (if it can be reached)
                let mut note = None;
                if let Some(r) = &e.remote {
                    if let Err(err) = crate::remote::remove(r, &e.id, true) {
                        note = Some(format!("{err} Its files are still on {}.", r.host));
                    }
                }
                stop(&id);
                // only ever a folder we made: <servers root>/<id>
                let ok_dir = made_here(&e);
                if ok_dir && e.dir.is_dir() {
                    std::fs::remove_dir_all(&e.dir).map_err(|err| format!("Couldn't delete the server's folder ({err})."))?;
                }
                edit(|l| l.retain(|x| x.id != id))?;
                drop(guard);
                // people helping run it lose access with it (shared control)
                crate::control::forget_server(&id);
                // (after the sync lock: shared lists take theirs first, then sync)
                crate::lists::forget(&id);
                forget(&id);
                crate::webstatus::forget_token(&id);
                crate::backup::forget_server(&id);
                Ok(json!({"ok": true, "note": note}))
            }
            _ => Err("Unknown action.".into()),
        }
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

/// The console (RCON) password, for using the server from another tool. Computer only.
// ---- the security check (a server on a Linux machine) ------------------------------------

/// What's checked on the machine, each with whether it's right and, if not, what the app can
/// do about it ("fix": the item to send back) or what the person can.
fn security_items(e: &Entry) -> Result<Value, String> {
    let r = e.remote.as_ref().ok_or("The security check is for servers on a Linux machine.")?;
    let (_, out) = crate::remote::exec_quick(r, &crate::remote::security_check_sh(&e.id))?;
    let kv: HashMap<&str, &str> = out.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
    let get = |k: &str| kv.get(k).copied().unwrap_or("");
    let item = |id: &str, label: &str, ok: bool, why: &str, fix: Option<&str>| json!({"id": id, "label": label, "ok": ok, "why": why, "fix": fix});
    let known = !get("passwordauthentication").is_empty();
    let items = vec![
        item("passwords", "SSH logins with a key only", !known || (get("passwordauthentication") == "no" && get("kbdinteractiveauthentication") != "yes"),
            "Password logins are on, so anyone on the internet can keep guessing. Loner-Strike uses your key, so turning passwords off is safe.", Some("passwords")),
        item("user", "Not running as root", get("uid") != "0",
            "The server runs as root, the most powerful account. It still works, but a bug in the game could reach the whole machine. A separate user is safer (Loner-Strike will do this for you in a later version).", None),
        item("rcon", "RCON closed to the internet", get("rcon_rules").parse::<u32>().unwrap_or(0) >= 1 && get("firewall_boot") == "enabled",
            "Anyone can try the console password from the internet. Only this machine needs it (Loner-Strike reaches it over SSH).", Some("firewall")),
        item("updates", "Automatic security updates", get("updates") == "yes",
            "Security fixes for the machine aren't installed by themselves.", Some("security")),
        item("fail2ban", "fail2ban slowing down login guessing", get("fail2ban") == "active",
            "Bots try SSH logins all day. fail2ban blocks an address after a few failures.", Some("security")),
        item("reboot", "No restart waiting", get("reboot") != "yes",
            "An update (usually a new kernel) is waiting for the machine to restart. Restart it when nobody's playing; the server starts again by itself.", Some("reboot")),
        item("sandbox", "The game in its sandbox", get("sandbox") != "0",
            "The game's service is from an older Loner-Strike. It's updated at the server's next start.", None),
    ];
    // the machine's health: disk, memory, CPU, and the logs kept in check
    let num = |k: &str| get(k).parse::<f64>().ok();
    let mut health = vec![];
    if let Some(used) = num("disk_used") {
        let free = num("disk_free_mb").unwrap_or(0.0) / 1024.0;
        let server = num("server_mb").map(|m| format!(" This server takes {:.1} GB.", m / 1024.0)).unwrap_or_default();
        health.push(item("disk", &format!("Disk space ({used:.0}% used, {free:.1} GB free)"), used < 85.0,
            &format!("The disk is getting full. When it's full the server can't write its logs or stats and may stop.{server} Cleaning up old logs helps; otherwise remove maps you don't use or move to a bigger plan."), Some("logs")));
    }
    if let (Some(total), Some(avail)) = (num("mem_total_mb"), num("mem_avail_mb")) {
        health.push(item("memory", &format!("Memory ({:.0} MB free of {:.0} MB)", avail, total), avail >= total * 0.1 && avail >= 150.0,
            "Memory is nearly used up, so the machine may slow down or stop the server. Fewer bots or servers on this machine help, or a plan with more memory.", None));
    }
    if let (Some(load), Some(cores)) = (num("load"), num("cores")) {
        health.push(item("cpu", &format!("CPU (load {load:.1} on {cores:.0} core{})", if cores == 1.0 { "" } else { "s" }), load <= cores * 1.5,
            "The machine is busier than its processors keep up with, which players feel as lag. Fewer bots or servers on it help, or a plan with more CPU.", None));
    }
    health.push(item("logs", "Logs kept in check", get("logs_timer") == "enabled",
        "Nothing trims the server's logs, so they grow until the disk is full. Loner-Strike can trim them once a day.", Some("security")));
    let items: Vec<Value> = items.into_iter().chain(health).collect();
    Ok(json!({"ok": true, "items": items, "user": get("user")}))
}

async fn api_security(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    answer(tokio::task::spawn_blocking(move || get(&id).ok_or("That server isn't there any more.".to_string()).and_then(|e| security_items(&e))).await.unwrap_or_else(|_| Err("The check didn't finish.".into())))
}

#[derive(Deserialize)]
struct SecurityFix {
    item: String,
}

async fn api_security_fix(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, Json(req): Json<SecurityFix>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    answer(
        tokio::task::spawn_blocking(move || -> Result<Value, String> {
            let e = get(&id).ok_or("That server isn't there any more.")?;
            let r = e.remote.clone().ok_or("The security check is for servers on a Linux machine.")?;
            let mut last = String::new();
            let run = |script: String, last: &mut String| -> Result<(), String> {
                let code = crate::remote::exec_stream(&r, &format!("{} bash -s", crate::remote::setup_env(&e.id, &e.game)), script.as_bytes(), |l| {
                    if !l.starts_with("@@") && !l.trim().is_empty() {
                        *last = l.trim().chars().take(160).collect();
                    }
                })?;
                if code != 0 {
                    return Err(format!("It didn't work on the machine ({last})."));
                }
                Ok(())
            };
            match req.item.as_str() {
                "passwords" => run(crate::remote::SSH_KEYS_ONLY_SH.to_string(), &mut last)?,
                "logs" => crate::remote::logs_now(&r, &e.id, &e.game)?,
                "firewall" | "security" => run(crate::remote::setup_stage(&req.item).ok_or("That isn't something the app can fix.")?, &mut last)?,
                "reboot" => {
                    // (never with people on; the game starts again by itself after)
                    if view(&e)["state"] == "running" {
                        if let Some((info, _)) = crate::browser::info(&e.join_address()) {
                            if info.players.saturating_sub(info.bots) > 0 {
                                return Err("People are playing on it. Restart the machine when they've left.".into());
                            }
                        }
                    }
                    let _ = crate::remote::exec_quick(&r, "sudo -n systemctl reboot");
                    return Ok(json!({"ok": true, "rebooting": true}));
                }
                _ => return Err("That isn't something the app can fix.".into()),
            }
            security_items(&e)
        })
        .await
        .unwrap_or_else(|_| Err("It didn't finish.".into())),
    )
}

async fn api_rcon_password(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let pw = get(&id).and_then(|e| std::fs::read_to_string(e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes/panel_rcon.txt")).ok());
    answer(pw.map(|p| json!({"ok": true, "password": p.trim()})).ok_or_else(|| "This server has no console password yet.".to_string()))
}

#[derive(Deserialize)]
struct NewRconPassword {
    #[serde(default)]
    password: Option<String>,
}

/// A new console password: the one given, or a random one. The running server gets it
/// straight away (over the old one), then the file the app and the plugin read it from.
async fn api_set_rcon_password(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, Json(req): Json<NewRconPassword>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let e = get(&id).ok_or("That server isn't there any more.")?;
        let pw = match req.password.map(|p| p.trim().to_string()) {
            Some(p) => {
                if !(8..=32).contains(&p.len()) || !p.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
                    return Err("Console passwords are 8 to 32 letters, numbers, - and _.".into());
                }
                p
            }
            None => {
                use rand::Rng;
                const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
                let mut rng = rand::thread_rng();
                (0..24).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect()
            }
        };
        // the running server takes it first: if it can't be told, nothing changes (the app
        // would otherwise lose its way in)
        if running(&id) {
            let told = e.rcon().map(|rc| rc.send(&format!("rcon_password \"{pw}\"")).is_ok()).unwrap_or(false);
            if !told {
                return Err("The server didn't take the new password, so it wasn't changed. Try again, or stop the server and change it then.".into());
            }
        }
        let data = e.dir.join(&e.game).join("addons/amxmodx/data/gamemodes");
        std::fs::create_dir_all(&data).map_err(|err| err.to_string())?;
        sys::write_atomic(&data.join("panel_rcon.txt"), format!("{pw}\n").as_bytes()).map_err(|err| err.to_string())?;
        forget(&id);
        // on another machine: the file goes up, and the service's copy (read at start) too
        if e.remote.is_some() && e.ready {
            // (noted first: if the machine can't be reached now, the background sync sends
            // the file and writes the service again, and a start does before starting)
            let _ = edit(|l| l.iter_mut().filter(|x| x.id == id).for_each(|x| x.service_stale = true));
            sync(&e, false)?;
            write_service(&e)?;
        }
        Ok(json!({"ok": true, "password": pw}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
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

#[derive(Deserialize, Default)]
struct HintsReq {
    #[serde(default)]
    refresh: bool,
    /// just this setting's current value
    #[serde(default)]
    value: Option<String>,
}

/// The running server's commands and settings, for suggestions while typing in its console,
/// or with `value`, one setting's current value (null for a password or an unknown name).
/// A POST so the page's request carries X-Panel; this computer only (it shows settings' values).
async fn api_console_hints(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, req: Option<Json<HintsReq>>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let req = req.map(|Json(r)| r).unwrap_or_default();
    let r = tokio::task::spawn_blocking(move || {
        let e = get(&id).ok_or("That server isn't there any more.")?;
        let rcon = e.rcon().ok_or("The server isn't set up yet.")?;
        if let Some(name) = req.value {
            return Ok(json!({"ok": true, "value": crate::consolehints::value(&id, &rcon, name.trim())}));
        }
        let refresh = req.refresh;
        crate::consolehints::for_server(&id, &rcon, refresh).map(|items| json!({"ok": true, "items": items}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
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
        // (keys and passwords blanked, as in the console: cvarlist prints them all)
        rcon.send(&cmd).map(|out| json!({"reply": out.lines().map(hide_passwords).collect::<Vec<_>>().join("\n")})).map_err(|_| "The server didn't answer. Check that it's running.".to_string())
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
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
    let r = tokio::task::spawn_blocking(move || import(Path::new(&crate::sys::clean_path(&req.dir)))).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(r)
}

/// The system's folder picker (the app window only).
async fn api_pick_folder(State(st): State<Shared>, headers: HeaderMap, body: Option<Json<Value>>) -> (StatusCode, Json<Value>) {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err("No window.".into())) };
    // the same picker for a server you already run, a game folder, or where servers go
    let title = match body.as_ref().and_then(|Json(b)| b["purpose"].as_str()) {
        Some("servers") => "Pick the folder new servers go in",
        Some("game") => "Pick the Half-Life folder (with cstrike or czero inside)",
        _ => "Pick the dedicated server's folder (with hlds.exe)",
    };
    let picked = tokio::task::spawn_blocking(move || app.dialog().file().set_title(title).blocking_pick_folder())
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
        .route("/api/servers/:id/modes", route_get(api_modes))
        .route("/api/servers/:id/rcon-password", post(api_set_rcon_password))
        .route("/api/servers/:id/security", route_get(api_security))
        .route("/api/servers/:id/security/fix", post(api_security_fix))
        // a POST, so it carries X-Panel (a plain GET from the page doesn't)
        .route("/api/servers/:id/rcon-password/show", post(api_rcon_password))
        .route("/api/servers/:id/:action", post(api_action))
}

/// Only for the app window, not the phone link: a free-form console, and importing a
/// folder (which can then be started, so it runs whatever hlds.exe is in it).
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/servers/import", post(api_import))
        .route("/api/servers/:id/console/send", post(api_console_cmd))
        .route("/api/servers/:id/console/hints", post(api_console_hints))
        .route("/api/servers/pick-folder", post(api_pick_folder))
        .route("/api/servers/remote", post(api_create_remote))
        .route("/api/servers/remote/probe", post(api_probe_remote))
        .route("/api/servers/pick-key", post(api_pick_key))
}

// ---- daily restart (Server extras' Schedule tab) ------------------------------------
//
// A running server can restart itself once a day at a set local time, while the app is
// open. Players get a warning 5 minutes and 1 minute before, through gm_extras'
// gm_extraswarn (a fixed command with a number), and once more as it restarts.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DailyRestart {
    #[serde(default)]
    pub on: bool,
    #[serde(default = "default_restart_hour")]
    pub hour: u8,
    #[serde(default)]
    pub minute: u8,
}

fn default_restart_hour() -> u8 {
    5
}

impl Default for DailyRestart {
    fn default() -> Self {
        DailyRestart { on: false, hour: default_restart_hour(), minute: 0 }
    }
}

impl DailyRestart {
    pub fn time(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

/// "04:30" -> (4, 30)
pub fn parse_time(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.trim().split_once(':')?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 || !h.bytes().chain(m.bytes()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (h, m): (u8, u8) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

/// The daily restart of the server in `dir` (the folder with hlds.exe) running `game`.
pub fn restart_of(dir: &Path, game: &str) -> Option<DailyRestart> {
    load().into_iter().find(|e| e.dir == dir && e.game == game).map(|e| e.restart.unwrap_or_default())
}

pub fn set_restart(dir: &Path, game: &str, r: DailyRestart) -> Result<(), String> {
    if get_by_dir(dir, game).is_none() {
        return Err("That server isn't there any more.".into());
    }
    edit(|l| l.iter_mut().filter(|e| e.dir == dir && e.game == game).for_each(|e| e.restart = Some(r.clone())))?;
    schedule_restarts();
    Ok(())
}

fn get_by_dir(dir: &Path, game: &str) -> Option<Entry> {
    load().into_iter().find(|e| e.dir == dir && e.game == game)
}

/// What to do now (minute of the day) for a restart at `at`: Some(5) and Some(1) warn,
/// Some(0) restarts.
pub fn restart_step(now: u32, at: u32) -> Option<u32> {
    let left = (at + 1440 - now % 1440) % 1440;
    [5, 1, 0].contains(&left).then_some(left)
}

/// (a key for today, minute of the day), local time.
#[cfg(unix)]
fn local_now() -> (u32, u32) {
    // SAFETY: localtime_r only writes into the tm we pass
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        ((tm.tm_year as u32) * 1000 + tm.tm_yday as u32, (tm.tm_hour * 60 + tm.tm_min) as u32)
    }
}

#[cfg(windows)]
fn local_now() -> (u32, u32) {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime only fills in the struct we pass
    let st = unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    (st.wYear as u32 * 10000 + st.wMonth as u32 * 100 + st.wDay as u32, st.wHour as u32 * 60 + st.wMinute as u32)
}

/// Starts the one thread that runs every server's daily restart (main.rs, and when a
/// schedule is saved).
pub fn schedule_restarts() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
            loop {
                std::thread::sleep(Duration::from_secs(15));
                let (day, now) = local_now();
                done.retain(|k| k.ends_with(&format!("@{day}")));
                for e in load() {
                    let Some(r) = e.restart.clone().filter(|r| r.on && e.ready) else { continue };
                    let Some(left) = restart_step(now, r.hour as u32 * 60 + r.minute as u32) else { continue };
                    // each step once a day
                    if !done.insert(format!("{}#{left}@{day}", e.id)) || !running(&e.id) {
                        continue;
                    }
                    if left > 0 {
                        if let Some(rcon) = e.rcon() {
                            let _ = rcon.send(&format!("gm_extraswarn {left}"));
                        }
                    } else {
                        let id = e.id.clone();
                        std::thread::spawn(move || restart_now(&id));
                    }
                }
            }
        });
    });
}

fn restart_now(id: &str) {
    if let Some(rcon) = get(id).and_then(|e| e.rcon()) {
        let _ = rcon.send("gm_extraswarn 0");
    }
    std::thread::sleep(Duration::from_secs(3));
    stop(id);
    if let Err(err) = start(id) {
        if err != crate::backup::RESTORING_SERVER {
            with_runs(|r| r.errors.insert(id.to_string(), format!("The daily restart couldn't start it again: {err}")));
        }
    }
}

#[cfg(test)]
mod restart_tests {
    use super::*;

    #[test]
    fn restart_times() {
        assert_eq!(parse_time("05:00"), Some((5, 0)));
        assert_eq!(parse_time(" 4:30 "), Some((4, 30)));
        assert_eq!(parse_time("23:59"), Some((23, 59)));
        for bad in ["24:00", "12:60", "12", "1:5", "ab:cd", "-1:00", "12:00:00", ""] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
        assert_eq!(DailyRestart::default().time(), "05:00");
        let at = 5 * 60;
        assert_eq!(restart_step(at - 5, at), Some(5));
        assert_eq!(restart_step(at - 1, at), Some(1));
        assert_eq!(restart_step(at, at), Some(0));
        assert_eq!(restart_step(at - 3, at), None);
        assert_eq!(restart_step(at + 1, at), None, "after the time: nothing until tomorrow");
        // just after midnight, warnings come the evening before
        assert_eq!(restart_step(1440 - 4, 1), Some(5));
        assert_eq!(restart_step(0, 1), Some(1));
        // servers.json from before schedules existed still loads
        let e: Entry = serde_json::from_str(r#"{"id":"x","name":"x","game":"czero","port":27016,"dir":"/x","kind":"crossover","bottle":null}"#).unwrap();
        assert!(e.restart.is_none());
        let r: DailyRestart = serde_json::from_str(r#"{"on":true}"#).unwrap();
        assert_eq!(r.time(), "05:00");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_written_only_on_change() {
        let dir = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "manifest-").unwrap();
        let path = dir.join(".uploaded.json");
        let mut now = Manifest::new();
        now.insert("maps/de_x.bsp".into(), (10, 5));
        assert!(save_manifest(&path, None, &now));
        let saved: Manifest = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!save_manifest(&path, Some(&saved), &now), "nothing changed: not written again");
        let mut later = now.clone();
        later.insert("maps/de_y.bsp".into(), (3, 6));
        assert!(save_manifest(&path, Some(&saved), &later));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn walks_when_changed_or_once_a_minute() {
        let id = "test-due-server";
        assert!(due(id, None), "never walked");
        assert!(!due(id, Some(Instant::now())));
        assert!(due(id, Instant::now().checked_sub(IDLE_CHECK)));
        mark_changed(id);
        assert!(due(id, Some(Instant::now())), "changed lately");
    }

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
            bottle: None, visibility: default_visibility(), map: default_map(), maxplayers: 20, ready: true, imported: false, hltv: Default::default(), restart: None, remote: None, setup_with: None, listen_ip: String::new(), extra_args: String::new(), bots_when_empty: None, ls_listed: false, public_address: None, web_status: Default::default(), pending_restart: Default::default(), service_stale: false, keep_running: None, notify_joins: None, notify_helpers: None, auto_update: None,
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
    fn public_addresses() {
        let ok = |t: &str| clean_public_address(t).unwrap();
        assert_eq!(ok(""), None);
        assert_eq!(ok("   "), None, "empty clears it");
        assert_eq!(ok(" Play.LonerStrike.com "), Some("play.lonerstrike.com".into()));
        assert_eq!(ok("play.example.com:27016"), Some("play.example.com:27016".into()));
        assert_eq!(ok("play.example.com."), Some("play.example.com".into()));
        assert_eq!(ok("178.156.241.180"), Some("178.156.241.180".into()));
        assert_eq!(ok("178.156.241.180:27015"), Some("178.156.241.180:27015".into()));
        assert_eq!(ok("my-home.duckdns.org:65535"), Some("my-home.duckdns.org:65535".into()));
        assert_eq!(ok("localhost"), Some("localhost".into()));
        let long = format!("{}.com", vec!["a".repeat(60); 5].join("."));
        assert!(long.len() > 253);
        for bad in [
            "http://play.example.com", "https://play.example.com", "play.example.com/join", "play example.com", "play.example.com:0",
            "play.example.com:65536", "play.example.com:", "play.example.com:+1", "play.example.com:27016:1", ":27016", "-play.example.com",
            "play-.example.com", "play..example.com", ".example.com", "1.2.3.999", "1.2.3", "play_x.example.com", "user@play.example.com",
            "play.example.com?x", "[::1]:27015", &"a".repeat(64), &long,
        ] {
            assert_eq!(clean_public_address(bad), Err(PUBLIC_ADDRESS_HELP.to_string()), "{bad}");
        }
    }

    #[test]
    fn connect_addresses() {
        let mut e: Entry = serde_json::from_str(r#"{"id":"x","name":"x","game":"cstrike","port":27016,"dir":"/x","kind":"windows","bottle":null}"#).unwrap();
        assert!(e.public_address.is_none(), "servers.json from before public addresses still loads");
        // on Windows a server that needs Steam is started on the internet adapter (bind_ip)
        if cfg!(windows) {
            assert_eq!(e.bind_ip(), route_ip());
            assert_eq!(launch_args(&e).iter().any(|w| w == "+ip"), route_ip().is_some());
        }
        e.visibility = "lan".into();
        assert_eq!(e.bind_ip(), None, "This network: left to hlds");
        // unset: the same as before
        assert_eq!(e.connect(), "127.0.0.1:27016");
        assert_eq!(e.connect(), e.address());
        assert_eq!(e.join_address(), "127.0.0.1:27016");
        e.listen_ip = "192.168.1.20".into();
        assert_eq!(e.connect(), "192.168.1.20:27016");
        e.public_address = Some("home.example.com".into());
        assert_eq!(e.connect(), "home.example.com:27016");
        assert_eq!(e.join_address(), "192.168.1.20:27016", "a server on this computer is joined directly");
        e.public_address = Some("home.example.com:28000".into());
        assert_eq!(e.connect(), "home.example.com:28000", "a port it names is kept");
        assert_eq!(e.public_at(27020).as_deref(), Some("home.example.com:27020"));

        e.listen_ip.clear();
        e.public_address = None;
        e.port = 27015;
        e.remote = serde_json::from_value(serde_json::json!({"host": "178.156.241.180", "key": "/k"})).unwrap();
        assert_eq!(e.connect(), "178.156.241.180:27015");
        assert_eq!(e.join_address(), "178.156.241.180:27015");
        assert_eq!(e.public_at(27020), None);
        e.public_address = Some("play.lonerstrike.com".into());
        assert_eq!(e.connect(), "play.lonerstrike.com", "27015 is the game's usual port: left off");
        assert_eq!(e.join_address(), "play.lonerstrike.com");
        e.port = 27017;
        assert_eq!(e.connect(), "play.lonerstrike.com:27017");
        // saved and loaded again
        let back: Entry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back.public_address.as_deref(), Some("play.lonerstrike.com"));
    }

    #[test]
    fn listing_flags() {
        let mut e = Entry {
            id: "x".into(), name: "x".into(), game: "cstrike".into(), port: 27016, dir: PathBuf::new(), kind: String::new(),
            bottle: None, visibility: "lan".into(), map: "de_dust2;quit".into(), maxplayers: 99, ready: true, imported: false, hltv: Default::default(), restart: None, remote: None, setup_with: None, listen_ip: String::new(), extra_args: String::new(), bots_when_empty: None, ls_listed: false, public_address: None, web_status: Default::default(), pending_restart: Default::default(), service_stale: false, keep_running: None, notify_joins: None, notify_helpers: None, auto_update: None,
        };
        let a = launch_args(&e).join(" ");
        assert!(a.contains("-nomaster") && a.contains("+sv_lan 1") && a.contains("+maxplayers 32") && a.contains("+map de_dust2") && !a.contains("quit"));
        e.visibility = "listed".into();
        let a = launch_args(&e).join(" ");
        assert!(!a.contains("-nomaster") && a.contains("+sv_lan 0") && a.contains("-port 27016"));
    }

    #[test]
    fn bots_on_an_empty_server() {
        let mut e: Entry = serde_json::from_str(r#"{"id":"x","name":"x","game":"cstrike","port":27016,"dir":"/nonexistent-ls","kind":"windows","bottle":null,"visibility":"unlisted"}"#).unwrap();
        assert!(!bots_when_empty(&e), "existing unlisted servers keep waiting for a human");
        assert!(!launch_args(&e).join(" ").contains("bot_join_after_player"));
        e.visibility = "listed".into();
        assert!(bots_when_empty(&e), "listed: on unless the host turned it off");
        // never a start option (game.cfg sets it back on every map): friends.cfg has it
        assert!(!launch_args(&e).join(" ").contains("bot_join_after_player"));
        e.bots_when_empty = Some(false);
        assert!(!bots_when_empty(&e));
        e.visibility = "lan".into();
        e.bots_when_empty = Some(true);
        assert!(bots_when_empty(&e));
        assert!(extra_args("+bot_join_after_player 1").is_err(), "set by the app");
        // the setting is kept, and left out of servers.json while unset
        assert!(serde_json::to_string(&e).unwrap().contains("\"bots_when_empty\":true"));
        e.bots_when_empty = None;
        assert!(!serde_json::to_string(&e).unwrap().contains("bots_when_empty"));
    }

    #[test]
    fn netcode_defaults() {
        let a = netcode_args(&[], "", false).join(" ");
        assert_eq!(a, "+sys_ticrate 300 +sv_maxupdaterate 101 +sv_minupdaterate 30 +sv_maxrate 100000 +sv_minrate 25000");
        assert!(netcode_args(&[], "", true).join(" ").ends_with("-pingboost 1"), "Linux only");
        // the host's own values win: extra start options and server.cfg
        let extra: Vec<String> = ["+sys_ticrate", "1000", "-pingboost", "3"].iter().map(|s| s.to_string()).collect();
        let a = netcode_args(&extra, "sv_maxrate \"50000\"\n// sv_minrate 1\n", true).join(" ");
        assert!(!a.contains("sys_ticrate") && !a.contains("pingboost") && !a.contains("sv_maxrate"));
        assert!(a.contains("+sv_minrate 25000"), "a commented-out line isn't a setting");
        // in the full command line, before the host's extra options
        let mut e: Entry = serde_json::from_str(r#"{"id":"x","name":"x","game":"cstrike","port":27016,"dir":"/nonexistent-ls","kind":"windows","bottle":null}"#).unwrap();
        e.extra_args = "+sv_maxupdaterate 60".into();
        let a = launch_args(&e).join(" ");
        assert!(a.contains("+sys_ticrate 300") && !a.contains("+sv_maxupdaterate 101") && a.ends_with("+sv_maxupdaterate 60") && !a.contains("pingboost"));
    }

    #[test]
    fn extra_start_options() {
        assert_eq!(extra_args("  -pingboost 2   +sv_gravity 600 ").unwrap(), ["-pingboost", "2", "+sv_gravity", "600"]);
        assert!(extra_args("").unwrap().is_empty());
        assert!(extra_args("+map de_dust2").is_err(), "the app sets the map");
        assert!(extra_args("+RCON_PASSWORD x").is_err());
        assert!(extra_args("+say $(reboot)").is_err());
        assert!(extra_args("+say \"hi\"").is_err());
        assert!(extra_args("hello").is_err(), "starts with - or +");
        assert!(extra_args("+port 27030").is_err(), "the app sets the port");
    }

    #[test]
    fn sync_records() {
        // the machine's own files never go up from this computer's copy
        assert!(server_owned("banned.cfg") && server_owned("listip.cfg") && server_owned("addons/amxmodx/logs/L1106.log"));
        assert!(!server_owned("addons/amxmodx/configs/plugins.ini") && !server_owned("maps/de_dust2.bsp"));
        // a record from before (whole seconds) still matches the same file
        assert!(same_file(&(10, 1_700_000_000), &(10, 1_700_000_000_123_456_789)));
        assert!(!same_file(&(10, 1_700_000_000), &(11, 1_700_000_000_123_456_789)));
        assert!(!same_file(&(10, 1_700_000_000_000_000_001), &(10, 1_700_000_000_000_000_002)), "two saves in one second");
    }

    #[test]
    fn machine_kept_files_are_never_sent_or_removed() {
        for k in ["addons/amxmodx/configs/users.ini", "addons/amxmodx/data/gamemodes/gags.txt", "addons/amxmodx/data/gamemodes/favorites.txt",
                  "addons/amxmodx/data/gamemodes/hidden_maps.txt", "addons/amxmodx/data/gamemodes/recent.txt", "addons/amxmodx/data/gamemodes/stats.json",
                  "addons/amxmodx/data/gamemodes/seen.json", "addons/amxmodx/data/gamemodes/reports.log.reading", "addons/amxmodx/data/gamemodes/chat.log"] {
            assert!(server_owned(k), "{k}");
        }
        assert!(!server_owned("addons/amxmodx/data/gamemodes/global.cfg") && !server_owned("addons/amxmodx/data/gamemodes/addons.txt"));
        // an old record still lists files that are the machine's own now: they're not "gone"
        let mut old = Manifest::new();
        old.insert("addons/amxmodx/configs/users.ini".into(), (5, 1));
        old.insert("addons/amxmodx/data/gamemodes/gags.txt".into(), (5, 1));
        old.insert("maps/de_old.bsp".into(), (5, 1));
        old.insert("maps/de_x.bsp".into(), (5, 1));
        let mut now = Manifest::new();
        now.insert("maps/de_x.bsp".into(), (6, 2));
        now.insert("addons/amxmodx/configs/plugins-gmaddons.ini".into(), (1, 1));
        let (changed, gone) = plan_sync(Send::Changes, Some(&old), &now);
        assert_eq!(gone, vec!["maps/de_old.bsp".to_string()]);
        assert!(changed.contains("maps/de_x.bsp") && changed.contains("addons/amxmodx/configs/plugins-gmaddons.ini"));
        // everything again after Valve's update: not the settings, not what the plugin writes
        let (changed, gone) = plan_sync(Send::AllButData, Some(&old), &now);
        assert!(gone.is_empty() && changed.contains("maps/de_x.bsp") && !changed.contains("addons/amxmodx/configs/plugins-gmaddons.ini"));
        // a map cycle at the game folder's root is read as it's needed
        assert!(!needs_restart("mapcycle.txt") && !needs_restart("motd.txt"));
        assert!(needs_restart("maps/de_x.bsp") && needs_restart("addons/amxmodx/plugins/x.amxx") && !needs_restart("maps/de_x.nav"));
    }

    #[test]
    fn friends_cfg_list_line() {
        let cfg = "// Written by Loner-Strike\nhostname \"A\"\nbot_join_after_player 0\n";
        assert_eq!(with_list_line(cfg, false), cfg, "never listed: left alone");
        let on = with_list_line(cfg, true);
        assert_eq!(on, format!("{cfg}ls_list 1\n"));
        assert_eq!(with_list_line(&on, true), on, "the same twice");
        let off = with_list_line(&on, false);
        assert_eq!(off, format!("{cfg}ls_list 0\n"), "taken off: it says so");
        assert_eq!(with_list_line(&with_bots_line(&on, false), true).matches("ls_list").count(), 1);
    }

    #[test]
    fn friends_cfg_bots_line() {
        assert_eq!(with_bots_line("", true), "// Written by Loner-Strike\nbot_join_after_player 0\n");
        let cfg = "// Written by Loner-Strike's Friends tab\nhostname \"A\"\nsv_password \"\"\nbot_join_after_player 0\n";
        let off = with_bots_line(cfg, false);
        assert_eq!(off, "// Written by Loner-Strike's Friends tab\nhostname \"A\"\nsv_password \"\"\nbot_join_after_player 1\n");
        assert_eq!(with_bots_line(&off, false), off, "the same twice");
        assert_eq!(off.matches("bot_join_after_player").count(), 1);
    }

    #[test]
    fn pending_restart_kept() {
        let mut e: Entry = serde_json::from_str(r#"{"id":"x","name":"x","game":"cstrike","port":27016,"dir":"/x","kind":"windows","bottle":null}"#).unwrap();
        assert!(e.pending_restart.is_clear());
        assert!(!serde_json::to_string(&e).unwrap().contains("pending_restart"), "left out while nothing waits");
        e.pending_restart.options = true;
        e.pending_restart.sounds = true;
        e.pending_restart.sounds_map = "de_dust2".into();
        let back: Entry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back.pending_restart, e.pending_restart);
        assert_eq!(back.pending_restart.view(), json!({"files": false, "options": true, "hltv": false, "sounds": true}));
        // a list saved with the plain flag still loads
        let old: Entry = serde_json::from_str(r#"{"id":"x","name":"x","game":"cstrike","port":27016,"dir":"/x","kind":"windows","bottle":null,"pending_restart":true}"#).unwrap();
        assert!(old.pending_restart.files && !old.pending_restart.options);
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
        // a template's history stays with it
        for f in ["stats.json", "seen.json", "reports.json", "chat.log", "matches.json"] {
            assert!(!settings_filter(Path::new(f)), "{f}");
        }
        assert!(settings_filter(Path::new("gags.txt")) && settings_filter(Path::new("favorites.txt")));
    }

    #[test]
    fn console_hides_the_apps_own_checks() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let got = without_polls(l(&["Rcon from 127.0.0.1:34335:", "rcon 352348701 \"x\" gm_status", "[TFR] Tyler : ace haha", "rcon 1 \"x\" amxx plugins"]), "127.0.0.1", &|_| false);
        // a server started on the network adapter: the app knocks from there (and from
        // 127.0.0.1 in older lines of the same log); someone else's RCON stays
        assert_eq!(without_polls(l(&["Rcon from 192.168.1.20:5000:", "Rcon from 127.0.0.1:1:", "Rcon from 10.0.0.9:7:"]), "192.168.1.20", &|_| false), l(&["Rcon from 10.0.0.9:7:"]));
        assert_eq!(got, l(&["[TFR] Tyler : ace haha", "rcon 1 \"x\" amxx plugins"]));
        // the console suggestions' lists and setting lookups; a real command stays
        let got = without_polls(l(&["Rcon from 127.0.0.1:2:", "rcon 5 \"x\" cmdlist", "Rcon from 127.0.0.1:3:", "rcon 6 \"x\" amx_help 11", "Rcon from 127.0.0.1:4:", "rcon 7 \"x\" mp_roundtime", "Rcon from 127.0.0.1:5:", "rcon 8 \"x\" changelevel de_inferno"]), "127.0.0.1", &|n| n == "mp_roundtime");
        // a one-word command someone typed stays
        let typed = without_polls(l(&["Rcon from 127.0.0.1:9:", "rcon 9 \"x\" status"]), "127.0.0.1", &|n| n == "mp_roundtime");
        assert_eq!(typed, l(&["rcon 9 \"x\" status"]));
        assert_eq!(got, l(&["rcon 8 \"x\" changelevel de_inferno"]));
    }
}
