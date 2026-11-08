//! Backups of a server, or of this computer's own game, in one .lsbackup file (a zip),
//! and putting them back: after a reinstall, on another computer, or on a Linux machine.
//!
//! Inside: manifest.json (what it is, the server's own choices, the mods and GameBanana
//! downloads it had, and every packed file with the part it belongs to), files/ (settings
//! files, by their path in the game folder) and custom/ (everything else that's packed).
//! "Export all" puts several in one file, each under sources/<n>/, with backups.json at
//! the top saying how many.
//!
//! Two kinds, both restorable anywhere:
//! - Light: settings, lists and the player's own files (maps they added by hand, their
//!   plugins, round music). Mods and GameBanana downloads are only listed, and installed
//!   and downloaded again on restore, so the file stays small.
//! - Everything included: also every file setup installed (with its record, so Update,
//!   Repair and Remove still work after) and every downloaded map and model, so a restore
//!   downloads nothing but Valve's own server files for a brand-new server (never packed).
//!
//! Passwords and webhooks are only in a backup when the player picks a password: they're
//! then sealed with a key stretched from it (Argon2id, then ChaCha20-Poly1305), and a
//! restore without the right password can go on without them.
//!
//! Nothing in a backup is trusted: every path is checked (no absolute paths, no "..",
//! plain names only), program files only come with an Everything included backup's mods,
//! and everything is written inside the target's own game folder.

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::servers::{self, Entry};
use crate::setup::{self, Job, Progress};
use crate::steam::Install;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const FORMAT: &str = "loner-strike-server-backup";
/// backups.json at the top of an "Export all" file
const MANY: &str = "loner-strike-server-backups";
const VERSION: u32 = 1;
pub const EXT: &str = "lsbackup";
/// The setup files Settings exported before backups (data folder files, no passwords)
const LEGACY: &str = "loner-strike-setup";
const DATA: &str = "addons/amxmodx/data/gamemodes/";
/// Settings files bigger than this aren't settings (a log someone renamed)
const MAX_CONFIG: u64 = 2_000_000;
/// No single file in a backup is written out past this (a damaged or hostile zip)
const MAX_FILE: u64 = 2_000_000_000;
/// Nor more than this in all
const MAX_TOTAL: u64 = 100_000_000_000;
/// The game's own few settings files a backup packs (besides the data folder and the map cycle)
const CONFIG_FILES: &[&str] = &["server.cfg", "banned.cfg", "listip.cfg", "addons/amxmodx/configs/users.ini", "addons/amxmodx/configs/plugins-gmaddons.ini"];
const RECORD: &str = "addons/loner-strike/record.json";
/// Settings lines with a password in them (any case, quoted or not)
const PASSWORD_CVARS: &[&str] = &["rcon_password", "sv_password", "adminpassword"];
/// What a line with a password becomes in a backup
const LEFT_OUT: &str = "was here: it was left out of the backup, and comes back with the backup's password)";
const NOT_BACKUP: &str = "That isn't a Loner-Strike backup.";
const DAMAGED: &str = "The backup is damaged, so it can't be used.";
pub const WRONG_PASSWORD: &str = "That password isn't right. Try again, or restore without the passwords and webhooks.";
const INTERNAL: &str = "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.";

/// The parts a backup is made of, which can each be left out of a backup or a restore:
/// (id, what the page calls it).
pub const PARTS: &[(&str, &str)] = &[
    ("settings", "Settings"),
    ("admins", "Admins & VIPs"),
    ("bans", "Bans"),
    ("rotation", "Map rotation"),
    ("plugins", "Your own plugins"),
    ("content", "Custom content and maps"),
    ("mods", "Mods"),
];

/// The app's own preferences that make sense on another computer or after a reinstall
/// (app-preferences.json at the top of the file). Never phone access or its token, which
/// game is picked, or folders: those belong to this computer.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct AppPrefs {
    pub manual_updates: bool,
    pub keep_servers: bool,
    pub mute_joins: bool,
    /// (None: a backup from before this setting, which leaves it as it is)
    pub mute_helpers: Option<bool>,
    pub smart_chat: bool,
}

const PREFS_FILE: &str = "app-preferences.json";

impl AppPrefs {
    fn now() -> AppPrefs {
        let s = crate::app_api::load_settings();
        AppPrefs { manual_updates: s.manual_updates, keep_servers: s.keep_servers, mute_joins: s.mute_joins, mute_helpers: Some(s.mute_helpers), smart_chat: s.smart_chat }
    }

    /// Puts them back; says what's left to do.
    fn apply(&self) -> Vec<String> {
        crate::app_api::edit_settings(|s| {
            s.manual_updates = self.manual_updates;
            s.keep_servers = self.keep_servers;
            s.mute_joins = self.mute_joins;
            if let Some(m) = self.mute_helpers {
                s.mute_helpers = m;
            }
            s.smart_chat = self.smart_chat;
        });
        let mut notes = Vec::new();
        if self.smart_chat && crate::smartchat::chosen().is_none() {
            notes.push("Smarter chat is on, but its model isn't on this computer yet: download it in Settings.".into());
        }
        notes
    }
}

// ---- the format ------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub app_version: String,
    /// "YYYYmmdd-HHMMSS" (UTC)
    pub created: String,
    /// "server" (a dedicated server) or "game" (this computer's own game)
    pub source: String,
    /// "light" or "full" (Everything included)
    pub content: String,
    /// "cstrike" or "czero"
    pub game: String,
    pub name: String,
    /// The server's id when it was backed up ("local:<game>" for the game): shared lists name it
    pub old_id: String,
    /// What's in it (PARTS ids)
    pub parts: Vec<String>,
    /// A server's own choices (none for the game)
    pub server: Option<Saved>,
    /// Modes and add-ons installed (catalog ids, without the foundation and core)
    pub components: Vec<String>,
    /// Light: GameBanana downloads to download again
    pub maps: Vec<MapItem>,
    pub files: Vec<Item>,
    /// The shared lists (lists.rs) it was in
    pub shared_lists: Vec<String>,
    /// Everything included: the server's record of downloaded maps (.installed_maps.json)
    /// and of other downloads (content/record.json), so Remove still works on them
    pub map_record: Option<Value>,
    pub content_records: Option<Value>,
    /// Bytes of the files in it
    pub bytes: u64,
    pub secrets: Option<Sealed>,
    /// Which install of the app made it (`this_computer`): shared lists are only rejoined
    /// on the computer they're shared on
    pub computer: String,
}

/// A file in a backup: its path in the game folder, the part it belongs to, and what it
/// is: "config" (a settings file, in files/), "custom" (the player's own or a download, in
/// custom/) or "mod" (installed by setup, in custom/, Everything included only).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub path: String,
    pub part: String,
    pub kind: String,
    pub size: u64,
}

/// A GameBanana download to get again: maps, skins, sounds... (`section` as in content.rs)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapItem {
    pub section: String,
    #[serde(rename = "mod")]
    pub mod_id: i64,
    #[serde(default)]
    pub name: String,
    /// the download's file name, to pick the same one again
    #[serde(default)]
    pub file: String,
}

/// What the player chose for a server. Never its id, folder, machine or ports (those
/// belong to where it runs), and never a password (those are secrets).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Saved {
    pub name: String,
    pub visibility: String,
    pub map: String,
    pub maxplayers: u32,
    pub hltv: crate::hltv::Settings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<servers::DailyRestart>,
    pub extra_args: String,
    pub bots_when_empty: Option<bool>,
    /// this app's own switches for it (backups from before them: the app-wide settings)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_running: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_joins: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_helpers: Option<bool>,
    pub ls_listed: bool,
    pub public_address: Option<String>,
    pub web_status: crate::webstatus::Settings,
}

impl Saved {
    fn of(e: &Entry) -> Saved {
        let mut hltv = e.hltv.clone();
        hltv.port = None;
        hltv.password.clear();
        let mut web_status = e.web_status.clone();
        web_status.port = None;
        Saved {
            name: e.name.clone(),
            visibility: e.visibility.clone(),
            map: e.map.clone(),
            maxplayers: e.maxplayers,
            hltv,
            restart: e.restart.clone(),
            extra_args: e.extra_args.clone(),
            bots_when_empty: e.bots_when_empty,
            keep_running: e.keep_running,
            notify_joins: e.notify_joins,
            notify_helpers: e.notify_helpers,
            ls_listed: e.ls_listed,
            public_address: e.public_address.clone(),
            web_status,
        }
    }

    /// Puts the backed-up choices on a server. Its id, folder, machine, ports and
    /// passwords stay its own; anything that doesn't check out stays as it was.
    fn apply(&self, e: &mut Entry, notes: &mut Vec<String>) {
        let name = clean_server_name(&self.name);
        if !name.is_empty() {
            e.name = name;
        }
        if ["lan", "unlisted", "listed"].contains(&self.visibility.as_str()) {
            e.visibility = self.visibility.clone();
        }
        if servers::safe_map(&self.map) {
            e.map = self.map.clone();
        }
        if self.maxplayers > 0 {
            e.maxplayers = self.maxplayers.clamp(2, 32);
        }
        match servers::extra_args(&self.extra_args) {
            Ok(_) => e.extra_args = self.extra_args.split_whitespace().collect::<Vec<_>>().join(" "),
            Err(why) => notes.push(format!("The extra start options weren't used: {why}")),
        }
        e.bots_when_empty = self.bots_when_empty;
        e.keep_running = self.keep_running;
        e.notify_joins = self.notify_joins;
        e.notify_helpers = self.notify_helpers;
        e.ls_listed = self.ls_listed;
        e.public_address = self.public_address.as_deref().and_then(|a| servers::clean_public_address(a).ok().flatten());
        e.restart = self.restart.clone();
        let (port, password) = (e.hltv.port, e.hltv.password.clone());
        e.hltv = self.hltv.clone();
        e.hltv.port = port;
        e.hltv.password = password;
        if e.remote.is_some() {
            let port = e.web_status.port;
            e.web_status = self.web_status.clone();
            e.web_status.port = port;
        } else if self.web_status.enabled {
            notes.push("Website status is for servers on a Linux machine, so it's off here.".into());
        }
    }
}

fn clean_server_name(name: &str) -> String {
    name.trim().chars().filter(|c| !c.is_control() && *c != '"').take(48).collect()
}

// ---- passwords and webhooks --------------------------------------------------------------

/// Everything secret about a server or game. Only ever in a backup sealed (Sealed).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Secrets {
    /// Discord webhook links by kind (webstatus::HOOKS)
    pub discord: BTreeMap<String, String>,
    /// The Cloudflare Tunnel token (website status)
    pub tunnel_token: String,
    /// The console (RCON) password the panel and plugin share
    pub rcon: String,
    pub join_password: String,
    /// HLTV's spectator password
    pub hltv_password: String,
    /// Lines taken out of packed settings files because they hold a password (rcon_password
    /// in server.cfg, an admin's password in users.ini): path -> (line number, the line)
    pub lines: BTreeMap<String, Vec<(usize, String)>>,
}

impl Secrets {
    fn is_empty(&self) -> bool {
        *self == Secrets::default()
    }
}

/// Secrets locked with the player's password: the key is stretched from it with Argon2id
/// (a random salt per backup), and the secrets sealed with ChaCha20-Poly1305 (a random
/// nonce), which also tells a wrong password from the right one. Bytes are hex.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Sealed {
    pub kdf: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub salt: String,
    pub cipher: String,
    pub nonce: String,
    pub data: String,
}

/// Ties the sealed secrets to this format, so they can't be passed off as something else
const AAD: &[u8] = b"loner-strike-server-backup secrets v1";
/// Argon2id as OWASP suggests: 19 MiB, 2 passes, 1 lane
const KDF_COST: (u32, u32, u32) = (19_456, 2, 1);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

fn key_from(password: &str, salt: &[u8], (m, t, p): (u32, u32, u32)) -> Result<[u8; 32], String> {
    let params = argon2::Params::new(m, t, p, Some(32)).map_err(|_| DAMAGED.to_string())?;
    let mut key = [0u8; 32];
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| DAMAGED.to_string())?;
    Ok(key)
}

pub fn seal(secrets: &Secrets, password: &str) -> Result<Sealed, String> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use rand::RngCore;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let key = key_from(password, &salt, KDF_COST)?;
    let cipher = chacha20poly1305::ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key));
    let plain = serde_json::to_vec(secrets).map_err(|e| e.to_string())?;
    let data = cipher.encrypt(chacha20poly1305::Nonce::from_slice(&nonce), Payload { msg: &plain, aad: AAD }).map_err(|_| "Couldn't lock the passwords.".to_string())?;
    let (m_cost, t_cost, p_cost) = KDF_COST;
    Ok(Sealed { kdf: "argon2id".into(), m_cost, t_cost, p_cost, salt: hex(&salt), cipher: "chacha20poly1305".into(), nonce: hex(&nonce), data: hex(&data) })
}

pub fn unseal(sealed: &Sealed, password: &str) -> Result<Secrets, String> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    if sealed.kdf != "argon2id" || sealed.cipher != "chacha20poly1305" {
        return Err("The passwords in this backup were locked by a newer Loner-Strike. Update the app, or restore without them.".into());
    }
    // a backup file can't make this computer spend minutes or gigabytes on its lock
    if sealed.m_cost > 262_144 || sealed.t_cost > 10 || sealed.p_cost > 8 {
        return Err(DAMAGED.into());
    }
    let salt = unhex(&sealed.salt).filter(|s| s.len() >= 8).ok_or(DAMAGED)?;
    let nonce = unhex(&sealed.nonce).filter(|n| n.len() == 12).ok_or(DAMAGED)?;
    let data = unhex(&sealed.data).ok_or(DAMAGED)?;
    let key = key_from(password, &salt, (sealed.m_cost, sealed.t_cost, sealed.p_cost))?;
    let cipher = chacha20poly1305::ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key));
    let plain = cipher.decrypt(chacha20poly1305::Nonce::from_slice(&nonce), Payload { msg: &data, aad: AAD }).map_err(|_| WRONG_PASSWORD.to_string())?;
    serde_json::from_slice(&plain).map_err(|_| DAMAGED.to_string())
}

// ---- which files ---------------------------------------------------------------------

/// The part a file in the game folder belongs to. `ours`: setup installed it.
pub fn part_of(rel: &str, ours: bool) -> &'static str {
    let l = rel.replace('\\', "/").to_ascii_lowercase();
    if ours || l.starts_with("addons/loner-strike/") {
        return "mods";
    }
    if let Some(d) = l.strip_prefix(DATA) {
        return match d {
            "admins.json" | "vip.cfg" | "regulars.ini" => "admins",
            "gags.txt" => "bans",
            "favorites.txt" | "hidden_maps.txt" => "rotation",
            "my_plugins.json" => "plugins",
            "content.json" | "content.ini" => "content",
            _ if d.starts_with("rotation_") && d.ends_with(".txt") => "rotation",
            _ => "settings",
        };
    }
    match l.as_str() {
        "addons/amxmodx/configs/users.ini" => "admins",
        "banned.cfg" | "listip.cfg" => "bans",
        "server.cfg" | "addons/amxmodx/configs/plugins-gmaddons.ini" => "settings",
        _ if l.starts_with("addons/amxmodx/plugins/") || l.starts_with("addons/amxmodx/scripting/") => "plugins",
        // the map cycle (the only .txt picked from the top of the game folder)
        _ if !l.contains('/') && l.ends_with(".txt") => "rotation",
        _ => "content",
    }
}

/// What the server or the app writes as it runs, which belongs to where it ran: live
/// status, stats and history, logs, half-written files, the bots' learned maps.
pub fn runtime(rel: &str) -> bool {
    const MORE: &[&str] = &["content_status.json", "loner-strike-hltv.cfg", ".uploaded.json", ".installed_maps.json", ".loner-strike-server"];
    let l = rel.replace('\\', "/").to_ascii_lowercase();
    let name = l.rsplit('/').next().unwrap_or("");
    l.starts_with("addons/amxmodx/logs/")
        || [".log", ".reading", ".taking", ".nav", ".part", ".lsrestore"].iter().any(|x| name.ends_with(x))
        || MORE.contains(&name)
        || (l.starts_with(DATA) && crate::prefs::SETUP_SKIP.contains(&name))
}

/// A path from a backup, if it's safe to write inside a game folder: relative, "/"
/// separated, plain names (no "..", no drive letters), and no scripts or programs that
/// could be run. Program libraries are checked separately (`library`).
pub fn safe_rel(rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.len() > 240 {
        return None;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() > 12 || !parts.iter().all(|p| crate::content::safe_part(p) && !reserved_name(p)) {
        return None;
    }
    let name = parts.last()?.to_ascii_lowercase();
    if [".exe", ".bat", ".cmd", ".com", ".scr", ".ps1", ".sh", ".vbs", ".lnk", ".app"].iter().any(|x| name.ends_with(x)) {
        return None;
    }
    Some(parts.iter().collect())
}

/// Names Windows keeps for devices (CON, NUL, COM1...), whatever follows the first dot.
fn reserved_name(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit() && stem.as_bytes()[3] != b'0')
}

/// Program code: what the game or the computer could load or run (metamod, AMX Mod X and
/// its modules and plugins, ReGameDLL...). Never written from a backup, which anyone could
/// have made: setup installs its own, checked copies. The one exception is the player's own
/// compiled plugins, when they choose to restore them.
fn code(rel: &str) -> bool {
    let l = rel.to_ascii_lowercase();
    let name = l.rsplit('/').next().unwrap_or("");
    [".dll", ".so", ".dylib", ".exe", ".asi", ".amxx", ".amx", ".com", ".scr", ".sys", ".drv", ".ocx", ".cpl", ".msi", ".bat", ".cmd", ".ps1", ".sh", ".vbs", ".jar"].iter().any(|x| name.ends_with(x))
        || name.contains(".so.")
}

/// Never taken from a backup's files: they decide what the game loads (liblist.gam,
/// metamod's plugin list) or what setup thinks it installed (its record, merged instead).
fn never_restored(rel: &str) -> bool {
    let l = rel.to_ascii_lowercase();
    l.ends_with(".gam") || l == "addons/metamod/plugins.ini" || l.starts_with("addons/loner-strike/")
}

/// A compiled plugin of the player's own, or its source: straight in AMX Mod X's folders.
fn own_plugin_path(low: &str) -> bool {
    let one = |dir: &str, ext: &str| low.strip_prefix(dir).and_then(|n| n.strip_suffix(ext)).is_some_and(|n| !n.is_empty() && !n.contains('/'));
    one("addons/amxmodx/plugins/", ".amxx") || one("addons/amxmodx/scripting/", ".sma")
}

/// One of the player's own plugins from a backup (compiled or source) may be restored: its
/// name is in the backup's my_plugins.json (`mine`, lowercase), it isn't one of
/// Loner-Strike's own plugins, and setup didn't install that file here (`setups`).
fn own_plugin_ok(path: &str, mine: &HashSet<String>, setups: &HashSet<String>) -> bool {
    let low = path.to_ascii_lowercase();
    let name = low.rsplit('/').next().and_then(|n| n.rsplit_once('.')).map(|(n, _)| n).unwrap_or("");
    let ours = setup::core::OUR_PLUGINS.iter().chain(setup::core::ALWAYS_ON).any(|p| p.eq_ignore_ascii_case(name));
    own_plugin_path(&low) && !name.is_empty() && mine.contains(name) && !ours && !setups.contains(&low)
}

/// Where a mod file from a backup may go: setup's files under addons/ and the maps, models
/// and sounds components bring, but never a settings file (those only come back through
/// their own parts, with the passwords handled) or anything else at the top.
fn mod_path_ok(rel: &str) -> bool {
    let low = rel.to_ascii_lowercase();
    let settings = low.ends_with(".cfg") || low.ends_with(".ini") || low.starts_with(DATA) || low.starts_with("addons/amxmodx/configs/") || CONFIG_FILES.contains(&low.as_str());
    !settings && (low.starts_with("addons/") || servers::content_filter(Path::new(rel)))
}

/// The part a file from a backup may be restored as, worked out again from its path and
/// never taken on the backup's word; None when it can't be restored at all. Mod files must
/// also be listed in the backup's install record (checked where they're restored), and the
/// player's own compiled plugins (`code`) only go back when the player chose that.
fn checked(it: &Item) -> Option<&'static str> {
    let low = it.path.to_ascii_lowercase();
    if safe_rel(&it.path).is_none() || runtime(&it.path) || never_restored(&it.path) {
        return None;
    }
    let part = match it.kind.as_str() {
        "mod" if !code(&low) && mod_path_ok(&it.path) => "mods",
        "config" if !code(&low) => {
            let known = it.path.get(DATA.len()..).filter(|_| low.starts_with(DATA)).is_some_and(|r| crate::prefs::exportable(Path::new(r)))
                || CONFIG_FILES.contains(&low.as_str())
                || (!low.contains('/') && low.ends_with(".txt"));
            if !known {
                return None;
            }
            part_of(&it.path, false)
        }
        "custom" => match part_of(&it.path, false) {
            "plugins" if own_plugin_path(&low) => "plugins",
            "content" if !code(&low) && servers::content_filter(Path::new(&it.path)) => "content",
            _ => return None,
        },
        _ => return None,
    };
    (part == it.part).then_some(part)
}

/// Takes plain passwords out of a settings file before it goes in a backup: a .cfg's lines
/// setting rcon_password, sv_password or adminpassword, and users.ini's admins with a
/// password (outside Loner-Strike's own block). Each such line is swapped for a comment, so
/// the line numbers stay; returns the new text and the lines as they were, or None when
/// there were none.
pub fn strip_passwords(rel: &str, text: &str) -> Option<(String, Vec<(usize, String)>)> {
    let users = is_users_ini(rel);
    if !users && !rel.to_ascii_lowercase().ends_with(".cfg") {
        return None;
    }
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    let mut taken = Vec::new();
    let mut ours = false;
    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim();
        if users {
            if t.starts_with(crate::admins::BEGIN) {
                ours = true;
            } else if t.starts_with(crate::admins::END) {
                ours = false;
            }
            if ours {
                continue;
            }
        }
        if let Some(key) = secret_key(users, t) {
            let end = if line.ends_with('\r') { "\r" } else { "" };
            let mark = if users { format!("; (admin \"{key}\" {LEFT_OUT}") } else { format!("// ({key} {LEFT_OUT}") };
            taken.push((i, std::mem::replace(line, format!("{mark}{end}"))));
        }
    }
    (!taken.is_empty()).then(|| (lines.join("\n"), taken))
}

fn is_users_ini(rel: &str) -> bool {
    rel.to_ascii_lowercase().ends_with("users.ini")
}

/// What a line holding a password sets: the cvar (lowercase) in a .cfg, the admin's name in
/// users.ini ("name" "password" "flags" "account flags"); None for any other line.
fn secret_key(users: bool, line: &str) -> Option<String> {
    let t = line.trim();
    if users {
        let f: Vec<&str> = t.split('"').collect();
        let ok = !t.starts_with(';') && !t.starts_with("//") && f.get(3).is_some_and(|pw| !pw.is_empty());
        return ok.then(|| f[1].chars().filter(|c| !c.is_control()).take(64).collect());
    }
    t.split(';')
        .find_map(|cmd| {
            let c = cmd.trim().trim_start_matches('"');
            PASSWORD_CVARS.iter().find(|v| {
                c.get(..v.len()).is_some_and(|h| h.eq_ignore_ascii_case(v))
                    && c[v.len()..].chars().next().is_none_or(|ch| ch.is_whitespace() || ch == '"')
                    && !c[v.len()..].trim().trim_matches('"').trim().is_empty()
            })
        })
        .map(|v| v.to_string())
}

/// The key of a comment `strip_passwords` left, if the line is one.
fn placeholder(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = t.strip_prefix("// (").or_else(|| t.strip_prefix("; (admin \""))?.strip_suffix(LEFT_OUT)?.trim_end();
    Some(inner.strip_suffix('"').unwrap_or(inner).to_string())
}

/// Puts the lines `strip_passwords` took out back (where the comment it left still is).
fn put_back_passwords(text: &str, taken: &[(usize, String)]) -> String {
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    for (i, line) in taken {
        if let Some(l) = lines.get_mut(*i) {
            if placeholder(l).is_some() && !line.contains('\n') {
                *l = line.clone();
            }
        }
    }
    lines.join("\n")
}

/// A file restored without the backup's passwords: the comments `strip_passwords` left are
/// filled with this computer's own lines for the same setting or admin (`mine`, from the
/// file as it was here before), and its other password lines are kept at the end, so a
/// restore never loses the passwords that were here.
fn keep_own_passwords(rel: &str, text: &str, mine: &[(usize, String)]) -> String {
    let users = is_users_ini(rel);
    let mut left: Vec<(String, String)> = mine.iter().filter_map(|(_, l)| Some((secret_key(users, l)?, l.trim_end_matches('\r').to_string()))).collect();
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    for l in lines.iter_mut() {
        let Some(key) = placeholder(l) else { continue };
        if let Some(pos) = left.iter().position(|(k, _)| k.eq_ignore_ascii_case(&key)) {
            let cr = if l.ends_with('\r') { "\r" } else { "" };
            *l = format!("{}{cr}", left.remove(pos).1);
        }
    }
    if !left.is_empty() {
        let cr = if text.contains("\r\n") { "\r" } else { "" };
        if lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.extend(left.into_iter().map(|(_, l)| format!("{l}{cr}")));
        lines.push(String::new());
    }
    lines.join("\n")
}

/// The player's own plugin (listed in my_plugins.json), compiled or as source.
fn own_plugin(low: &str, mine: &HashSet<String>) -> bool {
    let named = |dir: &str, ext: &str| low.strip_prefix(dir).and_then(|n| n.strip_suffix(ext)).is_some_and(|n| mine.contains(n));
    named("addons/amxmodx/plugins/", ".amxx") || named("addons/amxmodx/scripting/", ".sma")
}

/// What a backup of a game folder looks at.
pub struct Pick<'a> {
    pub game_dir: &'a Path,
    /// Everything included
    pub full: bool,
    pub parts: &'a [String],
    /// Valve's own copies of the game folder (the servers' base download): never packed
    pub stock: &'a [PathBuf],
    /// Files GameBanana downloads put there (lowercase): listed, not packed, in Light
    pub known: &'a HashSet<String>,
    /// This computer's own game: only maps are told apart from the game's own files
    pub own_game: bool,
}

pub struct Picked {
    pub items: Vec<Item>,
    /// files left out because of unusual names
    pub odd_names: usize,
}

/// The files a backup packs.
pub fn pick(p: &Pick) -> Picked {
    let g = p.game_dir;
    let rec = setup::record::Record::open(g);
    let ours: HashSet<String> = rec.data.files.keys().map(|k| k.replace('\\', "/").to_ascii_lowercase()).collect();
    let mine: HashSet<String> = std::fs::read_to_string(g.join(DATA).join("my_plugins.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|n| n.to_ascii_lowercase())
        .collect();
    let wanted = |part: &str| p.parts.iter().any(|x| x == part);
    let mut items = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut odd = 0;

    // settings files: the Game Mode Menu's data folder, then the game's own few
    let data = g.join(DATA);
    let mut config: Vec<String> = crate::archive::walk(&data)
        .iter()
        .filter_map(|f| f.strip_prefix(&data).ok().map(|r| r.to_string_lossy().replace('\\', "/")))
        .filter(|r| crate::prefs::exportable(Path::new(r)))
        .map(|r| format!("{DATA}{r}"))
        .collect();
    let cycle = crate::extras::cycle_name(g);
    for f in CONFIG_FILES.iter().copied().chain([cycle.as_str()]) {
        if g.join(f).is_file() {
            config.push(f.to_string());
        }
    }
    config.sort();
    for rel in config {
        if runtime(&rel) || !seen.insert(rel.to_ascii_lowercase()) {
            continue;
        }
        let size = std::fs::metadata(g.join(&rel)).map(|m| m.len()).unwrap_or(0);
        if size > MAX_CONFIG {
            continue;
        }
        if safe_rel(&rel).is_none() {
            odd += 1;
            continue;
        }
        let part = part_of(&rel, false);
        if wanted(part) {
            items.push(Item { path: rel, part: part.into(), kind: "config".into(), size });
        }
    }

    // everything else: setup's files (Everything included), the player's own plugins,
    // and maps and content that aren't Valve's
    let mut rest = crate::archive::walk(g);
    rest.sort();
    for f in rest {
        let Ok(r) = f.strip_prefix(g) else { continue };
        let rel = r.to_string_lossy().replace('\\', "/");
        let low = rel.to_ascii_lowercase();
        if runtime(&rel) || seen.contains(&low) {
            continue;
        }
        let first = low.split('/').next().unwrap_or("");
        let maps_or_wad = first == "maps" || (!low.contains('/') && low.ends_with(".wad"));
        let (kind, part) = if ours.contains(&low) || low.starts_with("addons/loner-strike/") {
            // program files, and what decides what the game loads, never come back from a
            // backup (setup installs them again), so they aren't packed; setup's record is
            if !p.full || (low != RECORD && (code(&low) || never_restored(&low) || !mod_path_ok(&low))) {
                continue;
            }
            ("mod", "mods")
        } else if own_plugin(&low, &mine) {
            ("custom", "plugins")
        } else if servers::content_filter(r) && (!p.own_game || maps_or_wad || p.known.contains(&low)) {
            if p.known.contains(&low) {
                // a download: listed and downloaded again, unless everything is included
                if !p.full {
                    continue;
                }
            } else if p.stock.iter().any(|s| s.join(r).is_file()) || (p.own_game && p.stock.is_empty()) {
                // Valve's own (or, for the game without a server's copy of Valve's files to
                // compare with, possibly Valve's own)
                continue;
            }
            ("custom", "content")
        } else {
            continue;
        };
        if !wanted(part) {
            continue;
        }
        if safe_rel(&rel).is_none() {
            odd += 1;
            continue;
        }
        let size = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
        seen.insert(low);
        items.push(Item { path: rel, part: part.into(), kind: kind.into(), size });
    }
    Picked { items, odd_names: odd }
}

// ---- what to back up -----------------------------------------------------------------

/// A server, or this computer's own game ("local:<game>").
pub struct Source {
    pub id: String,
    pub name: String,
    pub game: String,
    pub game_dir: PathBuf,
    pub entry: Option<Entry>,
}

fn game_title(g: &str) -> &'static str {
    if g == "czero" {
        "Condition Zero"
    } else {
        "CS 1.6"
    }
}

/// This computer's install with `game`: the panel's own game first.
fn own_install(st: &Shared, game: &str) -> Option<Install> {
    let cur = st.local_game().map(|g| g.hl.clone());
    let all = crate::steam::find_installs();
    let has = |i: &&Install| !i.dedicated && i.games.iter().any(|g| g == game);
    all.iter().filter(has).find(|i| Some(&i.hl) == cur.as_ref()).or_else(|| all.iter().find(has)).cloned()
}

fn source(st: &Shared, id: &str) -> Result<Source, String> {
    if let Some(game) = id.strip_prefix("local:") {
        let i = own_install(st, game).ok_or("That game isn't installed on this computer.")?;
        return Ok(Source { id: id.into(), name: format!("Your game ({})", game_title(game)), game: game.into(), game_dir: i.hl.join(game), entry: None });
    }
    let e = servers::get(id).filter(|e| e.ready).ok_or("That server isn't there any more, or isn't set up yet.")?;
    Ok(Source { id: id.into(), name: e.name.clone(), game: e.game.clone(), game_dir: e.dir.join(&e.game), entry: Some(e) })
}

/// Valve's own files to leave out: the servers' base download, next to the server or
/// where new servers go.
fn stock_dirs(st: &Shared, src: &Source) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = src.entry.as_ref().and_then(|e| e.dir.parent()) {
        v.push(p.join(".base").join(&src.game));
    }
    if let Some(root) = servers::client(st).and_then(|c| servers::root_for(&c).ok()) {
        v.push(root.join(".base").join(&src.game));
    }
    v.retain(|d| d.is_dir());
    v.dedup();
    v
}

#[derive(Default)]
struct Downloads {
    maps: Vec<MapItem>,
    known: HashSet<String>,
    map_record: Option<Value>,
    content_records: Option<Value>,
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// What came from GameBanana: the maps (the server's own record, or the app's for the
/// game, whose maps always go in CS 1.6's folder), other downloads (content/record.json)
/// and a server's skins and player models (servercontent.rs).
fn downloads(src: &Source) -> Downloads {
    let app = crate::sys::app_dir();
    let mut d = Downloads::default();
    let record = match &src.entry {
        Some(e) => Some(e.dir.join(".installed_maps.json")),
        None if src.game == "cstrike" => Some(app.join("installed_maps.json")),
        None => None,
    };
    if let Some(path) = record.filter(|p| p.is_file()) {
        let rec = crate::sys::read_json_map(&path);
        for (k, r) in &rec {
            let id = k.parse::<i64>().unwrap_or(0);
            if id <= 0 {
                continue;
            }
            d.known.extend(r["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(|f| f.replace('\\', "/").to_ascii_lowercase()));
            d.maps.push(MapItem { section: "maps".into(), mod_id: id, name: text(&r["name"]), file: text(&r["file"]) });
        }
        if !rec.is_empty() {
            d.map_record = Some(Value::Object(rec));
        }
    }
    let content = crate::content::Records::new(&app).for_game(&src.game_dir);
    for (k, r) in &content {
        let id = crate::content::key_id(k);
        if id <= 0 {
            continue;
        }
        d.known.extend(r["files"].as_array().into_iter().flatten().filter_map(|f| f["path"].as_str()).map(|f| f.replace('\\', "/").to_ascii_lowercase()));
        d.maps.push(MapItem { section: text(&r["section"]), mod_id: id, name: text(&r["name"]), file: text(&r["file"]) });
    }
    if !content.is_empty() {
        d.content_records = Some(Value::Object(content));
    }
    if src.entry.is_some() {
        for it in crate::servercontent::load(&src.game_dir) {
            let id = it["mod"].as_i64().unwrap_or(0);
            if id <= 0 {
                continue;
            }
            d.known.extend(it["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(|f| f.replace('\\', "/").to_ascii_lowercase()));
            d.maps.push(MapItem { section: text(&it["section"]), mod_id: id, name: text(&it["name"]), file: String::new() });
        }
    }
    d
}

/// Everything secret about a source, as it is on this computer.
fn gather_secrets(src: &Source) -> Secrets {
    let data = src.game_dir.join(DATA);
    let mut s = Secrets {
        rcon: std::fs::read_to_string(data.join("panel_rcon.txt")).map(|t| t.trim().to_string()).unwrap_or_default(),
        join_password: std::fs::read_to_string(data.join("friends.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| text(&v["password"]))
            .unwrap_or_default(),
        ..Default::default()
    };
    if let Some(e) = &src.entry {
        for k in crate::webstatus::HOOKS {
            if let Some(url) = crate::webstatus::read_hook(k, &e.id) {
                s.discord.insert(k.to_string(), url);
            }
        }
        s.tunnel_token = crate::webstatus::read_token(&e.id).unwrap_or_default();
        s.hltv_password = e.hltv.password.clone();
    }
    s
}

/// A random id for this install of the app, made once.
fn this_computer() -> String {
    let path = crate::sys::app_dir().join("backup-computer-id.txt");
    if let Some(id) = std::fs::read_to_string(&path).ok().map(|t| t.trim().to_string()).filter(|t| t.len() == 32) {
        return id;
    }
    let id = hex(&rand::random::<[u8; 16]>());
    let _ = crate::sys::write_atomic(&path, id.as_bytes());
    id
}

/// Only known parts, in order; none given means all of them.
fn clean_parts(parts: &[String]) -> Vec<String> {
    PARTS.iter().map(|(id, _)| id.to_string()).filter(|id| parts.is_empty() || parts.contains(id)).collect()
}

/// One source, ready to be written.
struct Planned {
    manifest: Manifest,
    /// files whose bytes differ from the copy here: read from the machine, or with the
    /// passwords taken out
    config: HashMap<String, Vec<u8>>,
    game_dir: PathBuf,
    odd_names: usize,
    /// the lines with passwords taken out of the packed settings files (`strip_passwords`)
    stripped: BTreeMap<String, Vec<(usize, String)>>,
}

fn latin1_bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
}

/// What a backup of `src` holds. `read_machine`: for a server on another machine, its
/// settings as the machine has them (the plugin saves settings there that this
/// computer's copy doesn't have); left out for a size estimate.
fn plan(st: &Shared, src: &Source, full: bool, parts: &[String], read_machine: bool) -> Result<Planned, String> {
    let dl = downloads(src);
    let stock = stock_dirs(st, src);
    let picked = pick(&Pick { game_dir: &src.game_dir, full, parts, stock: &stock, known: &dl.known, own_game: src.entry.is_none() });
    let mut items = picked.items;
    let mut config: HashMap<String, Vec<u8>> = HashMap::new();
    let wanted = |part: &str| parts.iter().any(|p| p == part);
    let put = |items: &mut Vec<Item>, config: &mut HashMap<String, Vec<u8>>, rel: String, bytes: Vec<u8>| {
        let part = part_of(&rel, false);
        if !wanted(part) || runtime(&rel) || safe_rel(&rel).is_none() {
            return;
        }
        match items.iter_mut().find(|i| i.path == rel) {
            Some(i) => i.size = bytes.len() as u64,
            None => items.push(Item { path: rel.clone(), part: part.into(), kind: "config".into(), size: bytes.len() as u64 }),
        }
        config.insert(rel, bytes);
    };
    if let Some(e) = src.entry.as_ref().filter(|e| read_machine && e.remote.is_some()) {
        let r = e.remote.as_ref().unwrap();
        let g = servers::game_for(&e.id, || None).ok_or("That server isn't set up yet.")?;
        let there = crate::prefs::machine_setup_files(&g).ok_or_else(|| format!("Couldn't reach {} to read {}'s settings. Try again when it's back.", r.host, e.name))?;
        for (name, body) in there {
            if crate::prefs::exportable(Path::new(&name)) {
                put(&mut items, &mut config, format!("{DATA}{name}"), latin1_bytes(&body));
            }
        }
        // the ban lists and admins live on the machine too
        let keys = ["banned.cfg", "listip.cfg", "addons/amxmodx/configs/users.ini"];
        let got = crate::remote::cat_files(r, &format!("{}/{}", crate::remote::server_dir(&e.id), e.game), &keys)?;
        for (k, bytes) in keys.iter().zip(got) {
            if let Some(b) = bytes {
                put(&mut items, &mut config, k.to_string(), b);
            }
        }
    }
    // the name travels without the password; Light leaves the downloaded skins out of the
    // server's list (they're downloaded again, and add themselves back)
    for it in items.iter_mut().filter(|i| i.kind == "config") {
        let low = it.path.to_ascii_lowercase();
        let changed = if low == format!("{DATA}friends.json") {
            let raw = config.get(&it.path).cloned().or_else(|| std::fs::read(src.game_dir.join(&it.path)).ok()).unwrap_or_default();
            let name = serde_json::from_slice::<Value>(&raw).ok().map(|v| text(&v["name"])).unwrap_or_default();
            Some(json!({"name": name}).to_string().into_bytes())
        } else if low == format!("{DATA}content.json") && !full {
            let raw = config.get(&it.path).cloned().or_else(|| std::fs::read(src.game_dir.join(&it.path)).ok()).unwrap_or_default();
            let list: Vec<Value> = serde_json::from_slice(&raw).unwrap_or_default();
            let own: Vec<Value> = list.into_iter().filter(|i| i["mod"].as_i64().unwrap_or(0) < 0).collect();
            Some(serde_json::to_vec_pretty(&own).unwrap_or_default())
        } else {
            None
        };
        if let Some(b) = changed {
            it.size = b.len() as u64;
            config.insert(it.path.clone(), b);
        }
    }
    // no plain passwords in a backup: those lines only travel sealed, with the secrets
    let mut stripped = BTreeMap::new();
    for it in items.iter_mut().filter(|i| i.size <= MAX_CONFIG && !code(&i.path)) {
        let low = it.path.to_ascii_lowercase();
        if !low.ends_with(".cfg") && !low.ends_with("users.ini") {
            continue;
        }
        let Some(raw) = config.get(&it.path).cloned().or_else(|| std::fs::read(src.game_dir.join(&it.path)).ok()) else { continue };
        if let Some((text, taken)) = strip_passwords(&it.path, &crate::rcon::latin1(&raw)) {
            let b = latin1_bytes(&text);
            it.size = b.len() as u64;
            config.insert(it.path.clone(), b);
            stripped.insert(it.path.clone(), taken);
        }
    }
    let member = match &src.entry {
        Some(e) => e.id.clone(),
        None => src.id.clone(),
    };
    let components = if wanted("mods") {
        setup::record::Record::open(&src.game_dir).component_ids().into_iter().filter(|c| c != "foundation" && c != "core").collect()
    } else {
        vec![]
    };
    let content = wanted("content");
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created: crate::sys::stamp(),
        source: if src.entry.is_some() { "server" } else { "game" }.into(),
        content: if full { "full" } else { "light" }.into(),
        game: src.game.clone(),
        name: src.name.clone(),
        old_id: member.clone(),
        parts: parts.to_vec(),
        server: src.entry.as_ref().filter(|_| wanted("settings")).map(Saved::of),
        components,
        maps: if content && !full { dl.maps } else { vec![] },
        bytes: items.iter().map(|i| i.size).sum(),
        files: items,
        shared_lists: crate::lists::memberships(&member),
        computer: this_computer(),
        map_record: dl.map_record.filter(|_| content && full),
        content_records: dl.content_records.filter(|_| content && full),
        secrets: None,
    };
    Ok(Planned { manifest, config, game_dir: src.game_dir.clone(), odd_names: picked.odd_names, stripped })
}

fn folder(kind: &str) -> &'static str {
    if kind == "config" {
        "files/"
    } else {
        "custom/"
    }
}

fn zip_name(prefix: &str, it: &Item) -> String {
    format!("{prefix}{}{}", folder(&it.kind), it.path)
}

/// Files that are already compressed are stored as they are.
fn zip_options(path: &str, size: u64) -> zip::write::SimpleFileOptions {
    let l = path.to_ascii_lowercase();
    let packed = [".mp3", ".png", ".jpg", ".jpeg", ".zip", ".rar", ".7z", ".gz", ".bz2", ".xz"].iter().any(|x| l.ends_with(x));
    let method = if packed { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
    zip::write::SimpleFileOptions::default().compression_method(method).large_file(size >= 0xF000_0000)
}

/// Writes the backup file: next to it first, then put in place, so a backup that stops
/// half way never leaves a broken file under the name picked. Big files stream through.
/// Each file's size in the manifest is what was actually packed (files change between
/// planning and saving); a file that's gone by then is left out and named in a note.
/// Returns the file's size and the notes.
fn write_backup(path: &Path, planned: &mut [Planned], prefs: Option<&AppPrefs>) -> Result<(u64, Vec<String>), String> {
    let mut notes = Vec::new();
    let zerr = |e: zip::result::ZipError| format!("Couldn't write the backup ({e}).");
    let ioerr = |e: std::io::Error| format!("Couldn't write the backup ({e}).");
    let tmp = path.with_extension(format!("{EXT}.part"));
    let file = std::fs::File::create(&tmp).map_err(ioerr)?;
    let r = (|| -> Result<(), String> {
        let mut z = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let many = planned.len() > 1;
        if many {
            z.start_file("backups.json", zip_options("backups.json", 0)).map_err(zerr)?;
            let top = json!({"format": MANY, "version": VERSION, "count": planned.len(), "created": crate::sys::stamp()});
            z.write_all(serde_json::to_string_pretty(&top).unwrap_or_default().as_bytes()).map_err(ioerr)?;
        }
        if let Some(prefs) = prefs {
            z.start_file(PREFS_FILE, zip_options(PREFS_FILE, 0)).map_err(zerr)?;
            z.write_all(serde_json::to_string_pretty(prefs).unwrap_or_default().as_bytes()).map_err(ioerr)?;
        }
        for (i, p) in planned.iter_mut().enumerate() {
            let prefix = if many { format!("sources/{i}/") } else { String::new() };
            let mut kept = Vec::with_capacity(p.manifest.files.len());
            for mut it in std::mem::take(&mut p.manifest.files) {
                let n = match p.config.get(&it.path) {
                    Some(b) => {
                        z.start_file(zip_name(&prefix, &it), zip_options(&it.path, b.len() as u64)).map_err(zerr)?;
                        z.write_all(b).map_err(ioerr)?;
                        b.len() as u64
                    }
                    None => {
                        let full = p.game_dir.join(&it.path);
                        let Ok(mut f) = std::fs::File::open(&full) else {
                            notes.push(format!("{}: {} was gone by the time it was saved, so it was left out.", p.manifest.name, it.path));
                            continue;
                        };
                        let now = f.metadata().map(|m| m.len()).unwrap_or(it.size);
                        z.start_file(zip_name(&prefix, &it), zip_options(&it.path, now.max(it.size))).map_err(zerr)?;
                        let mut limited = Read::take(&mut f, MAX_FILE);
                        std::io::copy(&mut limited, &mut z).map_err(|e| format!("Couldn't pack {} ({e}).", it.path))?
                    }
                };
                it.size = n;
                kept.push(it);
            }
            p.manifest.bytes = kept.iter().map(|i| i.size).sum();
            p.manifest.files = kept;
            z.start_file(format!("{prefix}manifest.json"), zip_options("manifest.json", 0)).map_err(zerr)?;
            z.write_all(serde_json::to_string_pretty(&p.manifest).unwrap_or_default().as_bytes()).map_err(ioerr)?;
        }
        let mut out = z.finish().map_err(zerr)?;
        out.flush().map_err(ioerr)
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't save the backup ({e}).")
    })?;
    Ok((std::fs::metadata(path).map(|m| m.len()).unwrap_or(0), notes))
}

/// A settings file from a backup, written in one go: its password lines are put back in
/// memory first (the backup's own, when they could be unlocked; otherwise this computer's,
/// from the file as it is here now), so it's never left without them or half written.
fn write_settings(rel: &str, dest: &Path, packed: &[u8], theirs: Option<&[(usize, String)]>) -> Result<(), String> {
    let text = crate::rcon::latin1(packed);
    let new = match theirs {
        Some(t) => put_back_passwords(&text, t),
        None => match crate::sys::read_latin1(dest).ok().and_then(|t| strip_passwords(rel, &t)) {
            Some((_, own)) => keep_own_passwords(rel, &text, &own),
            None => text,
        },
    };
    crate::sys::write_atomic(dest, &latin1_bytes(&new)).map_err(|e| format!("Couldn't write {rel} ({e})."))
}

/// After a restore: the newest 5 copies kept before restores of the same server or game as
/// `made` stay, and so does anything in `keep` (the one restored from); half-written ones
/// left by a backup that stopped are cleared.
fn prune_kept(made: &Path, keep: &[PathBuf]) {
    const MARK: &str = "-before-restore-";
    let Some(dir) = made.parent() else { return };
    let name = made.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let Some(i) = name.find(MARK) else { return };
    let stem = &name[..i + MARK.len()];
    let ext = format!(".{EXT}");
    let all: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    let named = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut old: Vec<&PathBuf> = all.iter().filter(|p| named(p).starts_with(stem) && named(p).ends_with(&ext)).collect();
    old.sort();
    let keep: Vec<PathBuf> = keep.iter().filter_map(|p| p.canonicalize().ok()).collect();
    for p in old.iter().take(old.len().saturating_sub(5)) {
        if p.canonicalize().is_ok_and(|c| keep.contains(&c)) {
            continue;
        }
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(key_path(p));
    }
    for p in all.iter().filter(|p| named(p).ends_with(&format!("{ext}.part"))) {
        let stale = std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 3600);
        if stale {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// New servers whose restore hasn't finished (kept across restarts): Try again would only
/// set up an empty server, so it's refused for them.
fn unfinished(id: Option<&str>, add: bool) {
    static LOCK: Mutex<()> = Mutex::new(());
    let Some(id) = id else { return };
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = crate::sys::app_dir().join("unfinished-restores.json");
    let mut ids: Vec<String> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    ids.retain(|x| x != id);
    if add {
        ids.push(id.to_string());
    }
    let _ = crate::sys::write_atomic(&path, serde_json::to_string(&ids).unwrap_or_default().as_bytes());
}

/// Whether a server was made by a restore that didn't finish.
pub fn restore_unfinished(id: &str) -> bool {
    std::fs::read_to_string(crate::sys::app_dir().join("unfinished-restores.json")).ok().and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok()).is_some_and(|ids| ids.iter().any(|x| x == id))
}

/// A server is gone: it's no longer an unfinished restore.
pub fn forget_server(id: &str) {
    if restore_unfinished(id) {
        unfinished(Some(id), false);
    }
}

/// Whether anything is being restored right now.
pub fn restoring_any() -> bool {
    RESTORING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| !s.is_empty())
}

/// Where the key of a copy kept on this computer is: next to it.
fn key_path(backup: &Path) -> PathBuf {
    let mut s = backup.as_os_str().to_os_string();
    s.push(".key");
    PathBuf::from(s)
}

/// A copy kept on this computer before a restore (it never leaves it): its passwords and
/// the password lines taken out of its settings files are sealed with a random key saved
/// next to it (readable by this user only), which opening it there uses by itself, so
/// rolling back brings the passwords back too.
fn write_kept(path: &Path, mut p: Planned, mut secrets: Secrets) -> Result<(), String> {
    secrets.lines = std::mem::take(&mut p.stripped);
    let key = if secrets.is_empty() {
        None
    } else {
        let key = hex(&rand::random::<[u8; 32]>());
        p.manifest.secrets = Some(seal(&secrets, &key)?);
        Some(key)
    };
    write_backup(path, std::slice::from_mut(&mut p), None)?;
    if let Some(key) = key {
        let kp = key_path(path);
        // a copy whose key couldn't be saved can't bring its passwords back: none is kept
        if let Err(e) = crate::sys::write_atomic(&kp, key.as_bytes()) {
            let _ = std::fs::remove_file(path);
            return Err(e.to_string());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&kp, std::fs::Permissions::from_mode(0o600));
        }
    }
    Ok(())
}

// ---- reading a backup -------------------------------------------------------------------

enum Backing {
    Zip(PathBuf),
    /// an old setup file from Settings: zip names -> bytes
    Legacy(HashMap<String, Vec<u8>>),
}

pub struct Opened {
    backing: Backing,
    /// the key saved next to a copy kept on this computer (`write_kept`)
    key: Option<String>,
    /// (where it is in the file, what it is)
    sources: Vec<(String, Manifest)>,
    /// the app's own preferences, when Export all took them
    prefs: Option<AppPrefs>,
}

/// Backups opened in the Import window, by token (the page never sends a file path).
static OPENED: Mutex<Option<HashMap<String, (u64, Arc<Opened>)>>> = Mutex::new(None);

fn manifest_from(v: Value) -> Result<Manifest, String> {
    if v["format"] != FORMAT {
        return Err(NOT_BACKUP.into());
    }
    if v["version"].as_u64().unwrap_or(0) > VERSION as u64 {
        return Err("That backup was made by a newer Loner-Strike. Update the app to restore it.".into());
    }
    let m: Manifest = serde_json::from_value(v).map_err(|_| DAMAGED.to_string())?;
    if !["cstrike", "czero"].contains(&m.game.as_str()) || m.files.len() > 500_000 {
        return Err(DAMAGED.into());
    }
    Ok(m)
}

fn read_json(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Value> {
    let f = z.by_name(name).ok()?;
    if f.size() > 50_000_000 {
        return None;
    }
    let mut s = String::new();
    f.take(50_000_000).read_to_string(&mut s).ok()?;
    serde_json::from_str(&s).ok()
}

/// An old setup file (Settings' Export before backups): the game's data folder files and
/// its mods, as a Light backup of the game without passwords.
fn open_legacy(v: &Value) -> Result<Opened, String> {
    let files = v["files"].as_object().ok_or(NOT_BACKUP)?;
    let mut items = Vec::new();
    let mut bytes = HashMap::new();
    for (k, body) in files.iter().take(500) {
        let (Some(rel), Some(body)) = (crate::prefs::safe_rel(k), body.as_str()) else { continue };
        let path = format!("{DATA}{}", rel.to_string_lossy().replace('\\', "/"));
        if safe_rel(&path).is_none() || body.len() as u64 > MAX_CONFIG {
            continue;
        }
        let b = latin1_bytes(body);
        items.push(Item { path: path.clone(), part: part_of(&path, false).into(), kind: "config".into(), size: b.len() as u64 });
        bytes.insert(format!("files/{path}"), b);
    }
    let game = if v["game"] == "czero" { "czero" } else { "cstrike" };
    let m = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        source: "game".into(),
        content: "light".into(),
        game: game.into(),
        name: format!("Your game ({})", game_title(game)),
        old_id: format!("local:{game}"),
        parts: clean_parts(&[]),
        components: v["components"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect(),
        bytes: items.iter().map(|i| i.size).sum(),
        files: items,
        ..Default::default()
    };
    Ok(Opened { backing: Backing::Legacy(bytes), key: None, sources: vec![(String::new(), m)], prefs: None })
}

pub fn open_backup(path: &Path) -> Result<Opened, String> {
    let mut head = [0u8; 2];
    let mut f = std::fs::File::open(path).map_err(|e| format!("Couldn't open it ({e})."))?;
    let _ = f.read(&mut head);
    if head != *b"PK" {
        if f.metadata().map(|m| m.len() > 20_000_000).unwrap_or(true) {
            return Err(NOT_BACKUP.into());
        }
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|_| NOT_BACKUP.to_string())?).map_err(|_| NOT_BACKUP.to_string())?;
        if v["format"] == LEGACY {
            return open_legacy(&v);
        }
        return Err(NOT_BACKUP.into());
    }
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| e.to_string())?).map_err(|_| NOT_BACKUP.to_string())?;
    let mut sources = Vec::new();
    if let Some(v) = read_json(&mut z, "manifest.json") {
        sources.push((String::new(), manifest_from(v)?));
    } else if let Some(top) = read_json(&mut z, "backups.json").filter(|t| t["format"] == MANY) {
        if top["version"].as_u64().unwrap_or(0) > VERSION as u64 {
            return Err("That backup was made by a newer Loner-Strike. Update the app to restore it.".into());
        }
        for i in 0..top["count"].as_u64().unwrap_or(0).min(200) {
            let prefix = format!("sources/{i}/");
            if let Some(v) = read_json(&mut z, &format!("{prefix}manifest.json")) {
                sources.push((prefix, manifest_from(v)?));
            }
        }
    }
    if sources.is_empty() {
        return Err(NOT_BACKUP.into());
    }
    // only a key that's no more than a key
    let key = std::fs::read_to_string(key_path(path)).ok().map(|k| k.trim().to_string()).filter(|k| k.len() == 64 && k.chars().all(|c| c.is_ascii_hexdigit()));
    let prefs = read_json(&mut z, PREFS_FILE).and_then(|v| serde_json::from_value(v).ok());
    Ok(Opened { backing: Backing::Zip(path.to_path_buf()), key, sources, prefs })
}

/// Reads files out of an opened backup (the zip opened once for a whole restore).
enum Reader<'a> {
    Zip(zip::ZipArchive<std::fs::File>),
    Legacy(&'a HashMap<String, Vec<u8>>),
}

impl Opened {
    /// The backup file, unless it's an old setup file read into memory.
    fn path(&self) -> Option<&Path> {
        match &self.backing {
            Backing::Zip(p) => Some(p),
            Backing::Legacy(_) => None,
        }
    }

    fn reader(&self) -> Result<Reader<'_>, String> {
        Ok(match &self.backing {
            Backing::Zip(p) => Reader::Zip(zip::ZipArchive::new(std::fs::File::open(p).map_err(|e| format!("Couldn't open the backup ({e}). Is it still there?"))?).map_err(|_| DAMAGED.to_string())?),
            Backing::Legacy(m) => Reader::Legacy(m),
        })
    }
}

/// Writes what `src` gives to `dest`, next to it first, then in place. It must be exactly
/// `size` bytes (what the backup's list says), or nothing is written.
fn write_stream(src: &mut dyn Read, dest: &Path, size: u64) -> Result<u64, String> {
    let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if size > MAX_FILE {
        return Err(DAMAGED.into());
    }
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make {} ({e}).", d.display()))?;
    }
    let tmp = dest.with_file_name(format!("{name}.lsrestore"));
    // one byte more than it should be tells a longer file from the right one
    let mut limited = Read::take(src, size + 1);
    let n = std::fs::File::create(&tmp)
        .and_then(|mut out| {
            let n = std::io::copy(&mut limited, &mut out)?;
            out.flush()?;
            Ok(n)
        })
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Couldn't write {name} ({e}).")
        })?;
    if n != size {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{name} in the backup isn't the size the backup says it is. {DAMAGED}"));
    }
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't write {name} ({e}). If the server or game is running, stop it and try again.")
    })?;
    Ok(n)
}

impl Reader<'_> {
    fn copy(&mut self, name: &str, dest: &Path, size: u64) -> Result<u64, String> {
        match self {
            Reader::Zip(z) => {
                let mut f = z.by_name(name).map_err(|_| format!("The backup is missing {name}."))?;
                write_stream(&mut f, dest, size)
            }
            Reader::Legacy(m) => write_stream(&mut m.get(name).ok_or_else(|| format!("The backup is missing {name}."))?.as_slice(), dest, size),
        }
    }

    fn bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        self.bytes_up_to(name, MAX_CONFIG)
    }

    fn bytes_up_to(&mut self, name: &str, max: u64) -> Option<Vec<u8>> {
        match self {
            Reader::Zip(z) => {
                let f = z.by_name(name).ok()?;
                let mut out = Vec::new();
                f.take(max).read_to_end(&mut out).ok()?;
                Some(out)
            }
            Reader::Legacy(m) => m.get(name).cloned(),
        }
    }
}

// ---- restoring ---------------------------------------------------------------------------

enum Target {
    /// a new server on this computer
    NewLocal(Entry),
    /// a new server on a Linux machine the app already uses
    NewRemote(Entry),
    /// over a server that's there (its settings, mods and files replaced)
    Server(Entry),
    /// this computer's own game
    Game(Install),
}

struct Restore {
    st: Shared,
    /// restore the player's own compiled plugins (code) too
    plugin_code: bool,
    /// bring mods from an older version in an Everything included backup up to this one
    update_mods: bool,
    opened: Arc<Opened>,
    prefix: String,
    m: Manifest,
    parts: Vec<String>,
    secrets: Option<Secrets>,
    target: Target,
    res: PathBuf,
    /// worth knowing before it started (shown with the rest at the end)
    notes: Vec<String>,
    /// the panel's own game, for a restore onto it
    local: Option<Arc<Game>>,
}

/// The machines servers are on: ("user@host:port", how to reach it).
fn machines() -> Vec<(String, crate::remote::RemoteInfo)> {
    let mut out: Vec<(String, crate::remote::RemoteInfo)> = Vec::new();
    for r in servers::load().into_iter().filter_map(|e| e.remote) {
        let label = format!("{}@{}:{}", r.user, r.host, r.ssh_port);
        if !out.iter().any(|(l, _)| *l == label) {
            out.push((label, r));
        }
    }
    out
}

/// HLTV and the website status, checked again where the server now is (ports, another
/// server's address); what doesn't fit is turned off and said why.
fn check_extras(e: &mut Entry, all: &[Entry], notes: &mut Vec<String>) {
    if e.hltv.enabled {
        match crate::hltv::validate(e.hltv.clone(), e, all) {
            Ok(s) => e.hltv = s,
            Err(why) => {
                e.hltv.enabled = false;
                notes.push(format!("HLTV was turned off: {why}"));
            }
        }
    }
    if e.remote.is_none() {
        e.web_status = Default::default();
    } else if !e.web_status.is_unset() {
        match crate::webstatus::validate(e.web_status.clone(), e, all) {
            Ok(s) => e.web_status = s,
            Err(why) => {
                e.web_status.enabled = false;
                notes.push(format!("Website status was turned off: {why}"));
            }
        }
    }
}

/// A new server for a restore: fresh id, folder and port, with the backup's choices.
fn new_entry(st: &Shared, m: &Manifest, saved: Option<&Saved>, name: &str, remote: Option<crate::remote::RemoteInfo>, notes: &mut Vec<String>) -> Result<Entry, String> {
    let client = servers::client(st).ok_or("Set up the game on this computer first: servers are made from it, and their plugins are built with it.")?;
    if client.kind == "crossover" && !Path::new(servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. It's needed to run and build servers on a Mac.".into());
    }
    let root = servers::root_for(&client)?;
    let list = servers::load();
    let id = servers::slug_in(name, &list, &root);
    let port = match &remote {
        Some(r) => servers::REMOTE_PORTS
            .clone()
            .find(|p| !list.iter().any(|o| o.remote.as_ref().is_some_and(|x| x.host == r.host) && o.port == *p))
            .ok_or_else(|| format!("{} already has a server on every port it can use (27015 to 27019). Delete one there first.", r.host))?,
        None => servers::free_port(&list),
    };
    let mut e = Entry {
        id: id.clone(),
        name: name.to_string(),
        game: m.game.clone(),
        port,
        dir: root.join(&id),
        kind: client.kind.clone(),
        bottle: client.bottle.clone(),
        visibility: servers::default_visibility(),
        map: servers::default_map(),
        maxplayers: servers::default_slots(),
        ready: false,
        imported: false,
        hltv: Default::default(),
        restart: None,
        remote: remote.clone(),
        setup_with: None,
        listen_ip: String::new(),
        extra_args: String::new(),
        bots_when_empty: None,
        ls_listed: false,
        public_address: None,
        web_status: Default::default(),
        pending_restart: Default::default(),
        service_stale: false,
        keep_running: None,
        notify_joins: None,
        notify_helpers: None,
    };
    if let Some(s) = saved {
        s.apply(&mut e, notes);
        e.name = name.to_string();
    }
    // so Try again and Rebuild from scratch know what to put on it
    if remote.is_some() {
        e.setup_with = Some(servers::SetupWith { template: "fresh".into(), components: m.components.clone(), password: String::new() });
    }
    check_extras(&mut e, &list, notes);
    Ok(e)
}

/// "1.2 GB"
fn gb(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1} GB", n as f64 / 1e9)
    } else {
        format!("{} MB", (n / 1_000_000).max(1))
    }
}

/// "a, b and c"
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        _ => format!("{} and {}", items[..items.len() - 1].join(", "), items[items.len() - 1]),
    }
}

fn part_name(id: &str) -> &'static str {
    PARTS.iter().find(|(p, _)| *p == id).map(|(_, n)| *n).unwrap_or("Other")
}

impl Restore {
    fn game(&self) -> &str {
        &self.m.game
    }

    fn entry(&self) -> Option<&Entry> {
        match &self.target {
            Target::NewLocal(e) | Target::NewRemote(e) | Target::Server(e) => Some(e),
            Target::Game(_) => None,
        }
    }

    /// The server as the list has it now (settings may have changed since the start).
    fn entry_now(&self) -> Option<Entry> {
        self.entry().map(|e| servers::get(&e.id).unwrap_or_else(|| e.clone()))
    }

    fn is_new(&self) -> bool {
        matches!(self.target, Target::NewLocal(_) | Target::NewRemote(_))
    }

    fn wants(&self, part: &str) -> bool {
        self.parts.iter().any(|p| p == part)
    }

    fn full(&self) -> bool {
        self.m.content == "full"
    }

    fn install(&self) -> Install {
        match &self.target {
            Target::NewLocal(e) | Target::NewRemote(e) | Target::Server(e) => e.install(),
            Target::Game(i) => i.clone(),
        }
    }

    fn game_dir(&self) -> PathBuf {
        self.install().hl.join(self.game())
    }

    fn own_game(&self) -> Option<Arc<Game>> {
        let Target::Game(i) = &self.target else { return None };
        self.local.clone().filter(|g| g.hl == i.hl && g.name == self.m.game).or_else(|| Some(Arc::new(Game::new(&i.hl, &self.m.game))))
    }

    fn store(&self) -> Arc<crate::gamebanana::MapStore> {
        match (&self.target, self.own_game()) {
            (_, Some(g)) => g.store.clone(),
            _ => {
                let hl = self.install().hl;
                Arc::new(crate::gamebanana::MapStore::new_server(&hl, &hl.join(self.game()), &crate::sys::app_dir(), Arc::new(Mutex::new(None))))
            }
        }
    }

    fn steps(&self) -> (&'static str, Vec<setup::Step>) {
        let g = self.game().to_string();
        let s = |id: &str, label: String, w: f64| setup::server_step(&g, id, label, w);
        let mut v = Vec::new();
        match &self.target {
            Target::NewLocal(_) => {
                v.push(s("srv-steamcmd", "Getting SteamCMD (Valve's server downloader)".into(), 1.0));
                v.push(s("srv-download", "Downloading the server from Valve".into(), 8.0));
                v.push(s("srv-copy", "Making the server's folder".into(), 1.0));
            }
            Target::NewRemote(e) => {
                let host = e.remote.as_ref().map(|r| r.host.clone()).unwrap_or_default();
                v.push(s("srv-machine", format!("Setting up {host}: libraries, firewall, Valve's server"), 10.0));
            }
            _ => v.push(s("rs-safety", "Saving a copy of what's there now, so you can undo".into(), 0.5)),
        }
        if self.full() && self.wants("mods") {
            v.push(s("rs-mods", "Putting the mods back from the backup".into(), 2.0));
        } else {
            v.push(s("rs-mods", "Working out which mods to put back".into(), 0.1));
        }
        v.push(s("rs-files", "Restoring settings and files".into(), 1.0));
        if self.entry().is_some_and(|e| e.remote.is_some()) {
            v.push(s("srv-upload", "Uploading to the server".into(), 1.5));
        }
        if matches!(self.target, Target::NewRemote(_)) {
            v.push(s("srv-service", "Starting it as a service".into(), 0.3));
        }
        v.push(s("rs-done", "Finishing up".into(), 0.3));
        (if self.is_new() { "server" } else { "restore" }, v)
    }

    fn run(self, job: &Job) -> Result<(), String> {
        let g = self.game().to_string();
        let step = |id: &str| format!("{g}/{id}");
        for n in &self.notes {
            job.note(n);
        }
        // the backup is opened once, before anything is downloaded or changed, and kept open
        // to the end: one that's gone or damaged stops it here, and a copy being restored
        // can't be pruned away part way
        let mut reader = self.opened.reader()?;
        let mut kept = None;
        match &self.target {
            Target::NewLocal(e) => {
                let base = e.dir.parent().ok_or("That isn't a server folder Loner-Strike can use.")?.join(".base");
                job.run(&step("srv-steamcmd"), || servers::ensure_steamcmd(job))?;
                job.run(&step("srv-download"), || servers::download_hlds(job, &base, &g))?;
                job.run(&step("srv-copy"), || {
                    // the engine and this game's folder (Condition Zero uses CS 1.6's too)
                    let other = if g == "cstrike" { "czero" } else { "cstrike" };
                    let keep_other = g == "czero";
                    servers::copy_tree(&base, &e.dir, false, &|rel: &Path| {
                        let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy().to_string()).unwrap_or_default();
                        first != "steamapps" && (keep_other || first != other)
                    })
                    .map(|_| ())
                })?;
            }
            Target::NewRemote(e) => {
                let r = e.remote.clone().ok_or(INTERNAL)?;
                job.run(&step("srv-machine"), || servers::machine_setup(job, &r, &e.id, &g))?;
                // this computer's copy starts with the game's own liblist.gam from the machine
                let gdir = e.dir.join(&g);
                std::fs::create_dir_all(&gdir).map_err(|err| err.to_string())?;
                let liblist = crate::remote::read_file(&r, &format!("{}/{g}/liblist.gam", crate::remote::server_dir(&e.id)))?;
                std::fs::write(gdir.join("liblist.gam"), liblist).map_err(|err| err.to_string())?;
            }
            _ => {
                let (saved, local_only) = job.run(&step("rs-safety"), || self.save_first())?;
                job.note(&format!("A copy of what was there before is in {}. To undo, restore that file (Settings, Backup & restore; its passwords come back too).", saved.display()));
                if local_only {
                    job.note("The server's machine couldn't be reached, so that copy has this computer's copy of its settings, which may be a little behind the machine's.");
                }
                kept = Some(saved);
            }
        }
        let gdir = self.game_dir();
        let plan = job.run(&step("rs-mods"), || self.mods(job, &gdir, &mut reader))?;
        if let Some(plan) = plan {
            job.insert_before(&step("rs-files"), plan.step_list());
            setup::install_into(job, &plan, &self.res, Some(self.store()))?;
        }
        let owned = job.run(&step("rs-files"), || self.files(job, &gdir, &mut reader))?;
        if let Some(e) = self.entry_now().filter(|e| e.remote.is_some()) {
            let r = e.remote.clone().ok_or(INTERNAL)?;
            job.run(&step("srv-upload"), || {
                let n = if self.is_new() { servers::upload(&e)? } else { servers::sync(&e, false)?.0 as u64 };
                // what the machine keeps itself (bans, admins, mutes) goes straight there:
                // the background sync never sends those
                for key in &owned {
                    let bytes = std::fs::read(gdir.join(key)).map_err(|err| format!("Couldn't read {key} ({err})."))?;
                    crate::remote::write_file(&r, &format!("{}/{}/{key}", crate::remote::server_dir(&e.id), e.game), &bytes)?;
                }
                job.detail(&format!("{} files uploaded", n + owned.len() as u64));
                Ok(())
            })?;
            if matches!(self.target, Target::NewRemote(_)) {
                job.run(&step("srv-service"), || {
                    let mut failed = String::new();
                    let code = crate::remote::exec_stream(&r, "bash -s", crate::remote::service_sh(&e.id, &e.game, &servers::launch_args(&e)).as_bytes(), |line| {
                        if !line.starts_with("@@") && !line.trim().is_empty() {
                            failed = line.trim().to_string();
                        }
                    })?;
                    if code != 0 {
                        return Err(format!("Couldn't set up the service ({failed})."));
                    }
                    crate::remote::systemctl(&r, &e.id, "start")
                })?;
            }
        }
        job.run(&step("rs-done"), || self.finish(job))?;
        // the older copies go once it's all done: never the one just made, nor the one
        // that was restored
        if let Some(kept) = kept {
            let mut keep = vec![kept.clone()];
            keep.extend(self.opened.path().map(Path::to_path_buf));
            prune_kept(&kept, &keep);
        }
        unfinished(self.entry().map(|e| e.id.as_str()).filter(|_| self.is_new()), false);
        Ok(())
    }

    /// Before anything changes on a server or game that's there: a Light backup of it in
    /// the app's folder (its passwords sealed with a key kept next to it). Returns where it
    /// is, and whether a server's machine couldn't be reached (this computer's copy used).
    fn save_first(&self) -> Result<(PathBuf, bool), String> {
        let g = self.game().to_string();
        let src = match &self.target {
            Target::Server(e) => Source { id: e.id.clone(), name: e.name.clone(), game: g.clone(), game_dir: e.dir.join(&g), entry: Some(servers::get(&e.id).unwrap_or_else(|| e.clone())) },
            Target::Game(i) => Source { id: format!("local:{g}"), name: format!("Your game ({})", game_title(&g)), game: g.clone(), game_dir: i.hl.join(&g), entry: None },
            _ => return Err(INTERNAL.into()),
        };
        let parts = clean_parts(&[]);
        let r = (|| -> Result<(PathBuf, bool), String> {
            // a machine that can't be reached: what this computer has of it
            let (p, local_only) = match plan(&self.st, &src, false, &parts, true) {
                Ok(p) => (p, false),
                Err(_) => (plan(&self.st, &src, false, &parts, false)?, src.entry.as_ref().is_some_and(|e| e.remote.is_some())),
            };
            let dir = crate::sys::app_dir().join("backups");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let stem = format!("{}-before-restore-", src.id.replace(':', "-"));
            let path = dir.join(format!("{stem}{}.{EXT}", crate::sys::stamp()));
            let secrets = gather_secrets(&src);
            write_kept(&path, p, secrets)?;
            Ok((path, local_only))
        })();
        r.map_err(|why| format!("Couldn't save a copy of what's there first, so nothing was changed: {why}"))
    }

    /// The mods: Everything included puts back the files setup lists in the backup's
    /// install record (never program files: setup installs those from its own checked
    /// downloads) and merges that record into this one; Light, or a backup without them,
    /// installs the ones it lists that aren't there (a new server always gets Loner-Strike
    /// itself). Returns the install to run, if one is needed.
    fn mods(&self, job: &Job, gdir: &Path, reader: &mut Reader) -> Result<Option<setup::InstallPlan>, String> {
        let g = self.game().to_string();
        let catalog = setup::components::load_catalog(&self.res)?;
        let can = |id: &str| catalog.iter().any(|c| c["id"] == id && setup::components::installable(c) && setup::components::games_of(c).contains(&g));
        let want = self.wants("mods");
        if !want && !self.is_new() {
            job.detail("Left out");
            job.note("Mods were left out as you chose, so none were installed or changed.");
            return Ok(None);
        }
        if want {
            let unknown: Vec<String> = self.m.components.iter().filter(|c| !can(c)).cloned().collect();
            if !unknown.is_empty() {
                job.note(&format!("Not put back, as this version of Loner-Strike can't install them: {}.", and_list(&unknown)));
            }
        }
        let theirs = if want && self.full() { self.backup_record(reader)? } else { None };
        if let Some(theirs) = theirs {
            let mut rec = setup::record::Record::open(gdir);
            if rec.broken {
                return Err("Loner-Strike's install record here is damaged, so the mods couldn't be put back.".into());
            }
            let Merged { missing, base, written: n, refused, failed } = self.merge_mods(job, &mut rec, &theirs, reader)?;
            if !failed.is_empty() {
                job.note(&format!("{} mod file{} couldn't be put back, so setup installs {} again: {}.", failed.len(), if failed.len() == 1 { "" } else { "s" }, if failed.len() == 1 { "its mod" } else { "their mods" }, failed.iter().take(5).cloned().collect::<Vec<_>>().join("; ")));
            }
            job.note(&format!("Put back {n} mod file{} from the backup, without downloading.", if n == 1 { "" } else { "s" }));
            if refused > 0 {
                job.note(&format!("{refused} mod file{} in the backup weren't used: program files, and anything setup's record doesn't list, only ever come from setup.", if refused == 1 { "" } else { "s" }));
            }
            let (mut redo, cant): (Vec<String>, Vec<String>) = missing.into_iter().partition(|c| can(c));
            if !cant.is_empty() {
                job.note(&format!("Not put back, as this version of Loner-Strike can't install them: {}.", and_list(&cant)));
            }
            // mods from an older version of Loner-Strike: this one's fixes and patches
            let older = setup::outdated(&self.res, &setup::record::Record::open(gdir));
            let mut base = base;
            if !older.is_empty() {
                let names: Vec<String> = older.iter().map(|c| if c == "core" { "Loner-Strike's own plugins".to_string() } else { c.clone() }).collect();
                if self.update_mods {
                    job.note(&format!("Updated to this version, as the backup had older ones: {}.", and_list(&names)));
                    // setup's own parts come with any install it runs
                    base |= older.iter().any(|c| c == "core" || c == "foundation");
                    for c in older.into_iter().filter(|c| c != "core" && c != "foundation" && can(c)) {
                        if !redo.contains(&c) {
                            redo.push(c);
                        }
                    }
                } else {
                    job.note(&format!("From an older version of Loner-Strike, left as they were: {}. Update all on its page brings them up to date.", and_list(&names)));
                }
            }
            if redo.is_empty() && !base {
                return Ok(None);
            }
            job.note(&format!("Setup installs {} from its own checked downloads (the program files a backup never brings).", if redo.is_empty() { "Loner-Strike".to_string() } else { and_list(&redo) }));
            return setup::plan_install(self.install(), &[g.clone()], &redo, &self.res, true).map(Some);
        }
        let have = setup::record::Record::open(gdir).component_ids();
        let todo: Vec<String> = if want { self.m.components.iter().filter(|c| can(c) && !have.contains(c)).cloned().collect() } else { vec![] };
        if !self.is_new() && todo.is_empty() && have.iter().any(|c| c == "core") {
            job.detail("Every mod is already there");
            return Ok(None);
        }
        setup::plan_install(self.install(), &[g.clone()], &todo, &self.res, false).map(Some)
    }

    /// Setup's install record from an Everything included backup, if it has one.
    fn backup_record(&self, reader: &mut Reader) -> Result<Option<setup::record::Data>, String> {
        let Some(it) = self.m.files.iter().find(|i| i.kind == "mod" && i.path.eq_ignore_ascii_case(RECORD)) else { return Ok(None) };
        let bytes = reader.bytes_up_to(&zip_name(&self.prefix, it), 100_000_000).ok_or_else(|| format!("The backup is missing {RECORD}."))?;
        serde_json::from_slice(&bytes).map(Some).map_err(|_| "The install record in the backup is damaged, so the mods couldn't be put back.".to_string())
    }

    /// See `merge_mods`: the backup's mod files that may be restored, written through this
    /// game's record.
    fn merge_mods(&self, job: &Job, rec: &mut setup::record::Record, theirs: &setup::record::Data, reader: &mut Reader) -> Result<Merged, String> {
        let mods: Vec<&Item> = self.m.files.iter().filter(|i| i.kind == "mod").collect();
        let total = mods.len().max(1);
        let mut done = 0;
        merge_mods(rec, theirs, &mods, &mut |it, dest| {
            done += 1;
            job.fraction(done as f64 / total as f64);
            reader.copy(&zip_name(&self.prefix, it), dest, it.size).map(|_| ())
        })
    }

    /// Settings and files, the name and passwords, and what's written from them. Returns
    /// the files a server's machine keeps itself, to be sent straight there.
    fn files(&self, job: &Job, gdir: &Path, reader: &mut Reader) -> Result<Vec<String>, String> {
        let mut notes = Vec::new();
        let remote = self.entry().is_some_and(|e| e.remote.is_some());
        let friends = format!("{DATA}friends.json");
        let items: Vec<&Item> = self.m.files.iter().filter(|i| i.kind != "mod" && self.wants(&i.part)).collect();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let (mut odd, mut owned, mut compiled) = (0, Vec::new(), 0);
        let lines = self.secrets.as_ref().map(|s| &s.lines);
        // the player's own plugins: only names the backup lists as theirs, never one of
        // Loner-Strike's or a file setup installed here
        let mine: HashSet<String> = self
            .m
            .files
            .iter()
            .find(|i| i.path.eq_ignore_ascii_case(&format!("{DATA}my_plugins.json")))
            .and_then(|i| reader.bytes(&zip_name(&self.prefix, i)))
            .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|n| n.to_ascii_lowercase())
            .collect();
        let setups: HashSet<String> = setup::record::Record::open(gdir).data.files.keys().map(|k| k.replace('\\', "/").to_ascii_lowercase()).collect();
        let (mut failed, mut not_theirs) = (Vec::new(), 0);
        for (i, it) in items.iter().enumerate() {
            job.fraction(i as f64 / items.len().max(1) as f64);
            // the part worked out again from the path: what the backup claims doesn't count
            let Some((rel, part)) = checked(it).and_then(|p| Some((safe_rel(&it.path)?, p))) else {
                odd += 1;
                continue;
            };
            if it.path.eq_ignore_ascii_case(&friends) {
                continue;
            }
            if own_plugin_path(&it.path.to_ascii_lowercase()) && !own_plugin_ok(&it.path, &mine, &setups) {
                not_theirs += 1;
                continue;
            }
            // the player's own compiled plugins are code: only when they chose to
            if code(&it.path) && !self.plugin_code {
                compiled += 1;
                continue;
            }
            let dest = gdir.join(rel);
            let name = zip_name(&self.prefix, it);
            let settings_file = (is_users_ini(&it.path) || it.path.to_ascii_lowercase().ends_with(".cfg")) && it.size <= MAX_CONFIG;
            let r = if !settings_file {
                reader.copy(&name, &dest, it.size).map(|_| ())
            } else {
                // a settings file: its passwords are put in in memory and it's written once
                let theirs = lines.and_then(|l| l.iter().find(|(k, _)| k.eq_ignore_ascii_case(&it.path))).map(|(_, v)| v.as_slice());
                match reader.bytes_up_to(&name, MAX_CONFIG + 1).filter(|b| b.len() as u64 == it.size) {
                    None => Err(format!("{} in the backup is missing, or isn't the size the backup says it is", it.path)),
                    Some(packed) => write_settings(&it.path, &dest, &packed, theirs),
                }
            };
            if let Err(why) = r {
                // one file that can't be restored doesn't stop the rest
                failed.push(format!("{} ({why})", it.path));
                continue;
            }
            if remote && servers::server_owned(&it.path) {
                owned.push(it.path.clone());
            }
            *counts.entry(part.to_string()).or_default() += 1;
        }
        if !failed.is_empty() {
            let shown: Vec<String> = failed.iter().take(5).cloned().collect();
            notes.push(format!("{} file{} couldn't be restored: {}{}.", failed.len(), if failed.len() == 1 { "" } else { "s" }, shown.join("; "), if failed.len() > 5 { "; and more" } else { "" }));
        }
        if not_theirs > 0 {
            notes.push(format!("{not_theirs} plugin file{} in the backup weren't restored: the backup doesn't list them as your own, or they're Loner-Strike's.", if not_theirs == 1 { "" } else { "s" }));
        }
        if compiled > 0 {
            notes.push(format!("{compiled} compiled plugin{} from the backup {} restored, as you chose. Their source (.sma) is back if it was in the backup: build them again on the Mods page.", if compiled == 1 { "" } else { "s" }, if compiled == 1 { "wasn't" } else { "weren't" }));
        }
        // the files are in: a server that's there takes the backup's choices (applied to it
        // as it is now, so an edit made meanwhile stays)
        if let (Target::Server(e), Some(s)) = (&self.target, self.m.server.as_ref().filter(|_| self.wants("settings"))) {
            servers::edit(|l| {
                let all = l.clone();
                if let Some(x) = l.iter_mut().find(|x| x.id == e.id) {
                    s.apply(x, &mut notes);
                    check_extras(x, &all, &mut notes);
                }
            })?;
        } else if self.wants("settings") && self.m.server.is_none() && self.entry().is_some() {
            notes.push("The backup is of a game, so the server keeps its own name, slots and start options.".into());
        } else if self.wants("settings") && self.m.server.is_some() && self.entry().is_none() {
            notes.push("The server's own settings (name, slots, start options, HLTV, website status) don't apply to your game, so they were left out.".into());
        }
        let entry = self.entry_now();
        let backup_name = self
            .m
            .files
            .iter()
            .find(|i| i.path.eq_ignore_ascii_case(&friends) && self.wants(&i.part))
            .and_then(|i| reader.bytes(&zip_name(&self.prefix, i)))
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|v| text(&v["name"]))
            .filter(|n| !n.trim().is_empty());
        let secrets = self.secrets.as_ref();
        let join_pw = secrets.map(|s| s.join_password.clone()).filter(|p| !p.is_empty() && crate::sharing::password_ok(p));
        let data = gdir.join(DATA);
        std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
        match &entry {
            Some(e) => {
                // the name and join password (friends.json and friends.cfg, as Server settings writes them)
                let pw = join_pw.clone().unwrap_or_else(|| servers::join_password(e));
                servers::write_join(&data, &e.name, &pw, servers::bots_when_empty(e), e.ls_listed)?;
                for f in ["banned.cfg", "listip.cfg"] {
                    if !gdir.join(f).exists() {
                        let _ = std::fs::write(gdir.join(f), "");
                    }
                }
                if let Some(s) = secrets {
                    if !s.rcon.is_empty() && s.rcon.len() <= 64 && s.rcon.chars().all(|c| c.is_ascii_graphic() && c != '"') {
                        crate::sys::write_atomic(&data.join("panel_rcon.txt"), format!("{}\n", s.rcon).as_bytes()).map_err(|err| err.to_string())?;
                    }
                    for (kind, url) in &s.discord {
                        if let (true, Ok(url)) = (crate::webstatus::HOOKS.contains(&kind.as_str()), crate::webstatus::clean_hook(url)) {
                            crate::webstatus::save_hook(kind, &e.id, Some(&url))?;
                        }
                    }
                    if let Ok(t) = crate::webstatus::clean_token(&s.tunnel_token) {
                        crate::webstatus::save_token(&e.id, &t)?;
                    }
                    if !s.hltv_password.is_empty() && s.hltv_password.len() <= 32 && s.hltv_password.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
                        servers::edit(|l| l.iter_mut().filter(|x| x.id == e.id).for_each(|x| x.hltv.password = s.hltv_password.clone()))?;
                    }
                }
                // the panel's console password, which the plugin reads on the machine
                if remote && !data.join("panel_rcon.txt").is_file() {
                    use rand::Rng;
                    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
                    let mut rng = rand::thread_rng();
                    let pw: String = (0..24).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
                    std::fs::write(data.join("panel_rcon.txt"), format!("{pw}\n")).map_err(|err| err.to_string())?;
                }
            }
            None => {
                if let Some(g) = self.own_game().filter(|_| backup_name.is_some() || join_pw.is_some()) {
                    let now: Value = std::fs::read_to_string(data.join("friends.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
                    let name = backup_name.clone().unwrap_or_else(|| text(&now["name"]));
                    let pw = join_pw.clone().unwrap_or_else(|| text(&now["password"]));
                    if let Err(why) = g.sharing.set_server(&g.rcon, &g.data, &name, &pw) {
                        notes.push(format!("The name and join password weren't set: {why}"));
                    }
                }
                if let Some(s) = secrets {
                    if !s.rcon.is_empty() {
                        notes.push("Your game keeps its own console password (the app and the game share it).".into());
                    }
                    if !s.discord.is_empty() || !s.tunnel_token.is_empty() || !s.hltv_password.is_empty() {
                        notes.push("Discord webhooks, the tunnel token and the HLTV password are for servers, so they were left out.".into());
                    }
                }
            }
        }
        // your own plugins load in every mode again (their block in plugins.ini)
        if counts.contains_key("plugins") {
            // only plain names whose compiled plugin is here (it may have been left out)
            let names: Vec<String> = std::fs::read_to_string(data.join("my_plugins.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
                .unwrap_or_default()
                .into_iter()
                .filter(|n| !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c)) && !n.starts_with('.'))
                .filter(|n| gdir.join("addons/amxmodx/plugins").join(format!("{n}.amxx")).is_file())
                .collect();
            let ini = gdir.join("addons/amxmodx/configs/plugins.ini");
            if ini.parent().is_some_and(|d| d.is_dir()) {
                let old = crate::sys::read_latin1(&ini).unwrap_or_default();
                crate::sys::write_atomic(&ini, &latin1_bytes(&crate::plugins::rewrite_ini(&old, &names))).map_err(|e| format!("Couldn't update plugins.ini ({e})."))?;
            }
        }
        // the server's skins and player models: the plugin's list written again
        if entry.is_some() && data.join("content.json").is_file() {
            let items = crate::servercontent::load(gdir);
            let _ = crate::servercontent::save(gdir, &items);
        }
        if self.full() && self.wants("content") {
            self.restore_records(gdir, entry.as_ref());
        }
        let done: Vec<String> = counts.iter().map(|(p, n)| format!("{} ({n} file{})", part_name(p), if *n == 1 { "" } else { "s" })).collect();
        if !done.is_empty() {
            job.note(&format!("Restored: {}.", and_list(&done)));
        }
        if odd > 0 {
            job.note(&format!("{odd} file{} in the backup couldn't be used safely, so {} left out.", if odd == 1 { "" } else { "s" }, if odd == 1 { "it was" } else { "they were" }));
        }
        let skipped: Vec<String> = PARTS.iter().filter(|(p, _)| self.m.parts.iter().any(|x| x == p) && !self.wants(p)).map(|(_, n)| n.to_string()).collect();
        if !skipped.is_empty() {
            job.note(&format!("Left out as you chose: {}.", and_list(&skipped)));
        }
        for n in notes {
            job.note(&n);
        }
        Ok(owned)
    }

    /// Everything included: the records of what was downloaded, so the Maps and Get more
    /// pages list them and Remove still takes them away. Each file a record names is
    /// checked like any path from a backup; records naming anything else are dropped.
    fn restore_records(&self, gdir: &Path, entry: Option<&Entry>) {
        let app = crate::sys::app_dir();
        // only downloads' own kinds of files: a record can't claim server.cfg or a plugin
        // (Remove on it would delete that later)
        let path_ok = |p: &str| safe_rel(p).is_some() && !code(p) && !never_restored(p) && servers::content_filter(Path::new(p));
        let paths_ok = |files: &Value, key: Option<&str>| files.as_array().into_iter().flatten().all(|f| path_ok(key.map_or(f.as_str(), |k| f[k].as_str()).unwrap_or("")));
        if let Some(Value::Object(rec)) = &self.m.map_record {
            let path = match entry {
                Some(e) => Some(e.dir.join(".installed_maps.json")),
                None if self.game() == "cstrike" => Some(app.join("installed_maps.json")),
                None => None,
            };
            if let Some(path) = path {
                let mut now = crate::sys::read_json_map(&path);
                for (k, r) in rec.iter().filter(|(_, r)| paths_ok(&r["files"], None)) {
                    now.insert(k.clone(), r.clone());
                }
                let _ = crate::sys::write_atomic(&path, serde_json::to_string_pretty(&now).unwrap_or_default().as_bytes());
            }
        }
        if let Some(Value::Object(recs)) = &self.m.content_records {
            let store = crate::content::Records::new(&app);
            let mut all = store.load();
            let tag = crate::sounds::folder_tag(gdir);
            let mut seq = all.values().filter_map(|r| r["seq"].as_i64()).max().unwrap_or(0);
            for (k, r) in recs.iter().filter(|(_, r)| paths_ok(&r["files"], Some("path"))) {
                let id = crate::content::key_id(k);
                let kind = k.split(':').next().unwrap_or("");
                if id == 0 || !["Mod", "Sound", "Spray"].contains(&kind) {
                    continue;
                }
                let mut r = r.clone();
                seq += 1;
                r["game"] = json!(gdir.to_string_lossy());
                r["seq"] = json!(seq);
                // what they replaced here wasn't kept by this computer: Remove just takes them away
                if let Some(files) = r["files"].as_array_mut() {
                    for f in files {
                        f["backup"] = Value::Null;
                    }
                }
                all.insert(format!("{kind}:{id}:{tag}"), r);
            }
            let _ = store.save(&all);
        }
        if let Some(g) = self.own_game() {
            *g.maps_cache.lock().unwrap() = None;
        }
    }

    /// The server is ready: admins written out, shared lists, what runs on its machine,
    /// and the downloads queued.
    fn finish(&self, job: &Job) -> Result<(), String> {
        if let Some(e) = self.entry().filter(|_| self.is_new()) {
            servers::edit(|l| l.iter_mut().filter(|x| x.id == e.id).for_each(|x| x.ready = true))?;
        }
        let game = match self.entry() {
            Some(e) => {
                // its cached connection has the old console password
                servers::forget(&e.id);
                servers::game_for(&e.id, || None)
            }
            None => self.own_game(),
        };
        // admins and VIP perks take effect (users.ini and vip.cfg are written from admins.json)
        if let Some(g) = &game {
            crate::admins::sync(g);
        }
        if !self.m.shared_lists.is_empty() && (self.m.computer.is_empty() || self.m.computer != this_computer()) {
            job.note("It shared some lists (admins, bans...) with other servers on the computer it was backed up on. Set up sharing again in Admins if you want.");
        } else if !self.m.shared_lists.is_empty() {
            let member = self.entry().map(|e| e.id.clone()).unwrap_or_else(|| format!("local:{}", self.game()));
            let r = crate::lists::rejoin(&self.m.old_id, &member, &self.m.shared_lists);
            if r.not_current {
                job.note(&format!("It shared some lists (admins, bans...) with other servers. {} isn't the game the app is using now, so it wasn't added back: switch to it and set that up again in Admins if you want.", setup::game_name(self.game())));
            } else if !r.skipped.is_empty() {
                let names: Vec<String> = r.skipped.iter().map(|l| crate::lists::LISTS.iter().find(|(id, _)| id == l).map(|(_, n)| n.to_string()).unwrap_or_else(|| l.clone())).collect();
                job.note(&format!("It shared {} with other servers, but not with any here, so it keeps its own. Set up sharing again in Admins if you want.", and_list(&names).to_lowercase()));
            }
        }
        if let Some(e) = self.entry_now().filter(|e| e.remote.is_some()) {
            if !self.is_new() {
                if let Err(why) = servers::write_service(&e) {
                    job.note(&why);
                }
            }
            let mut e = e;
            if e.web_status.enabled && e.web_status.own_tunnel && e.web_status.mode == "tunnel" && crate::webstatus::read_token(&e.id).is_none() {
                e.web_status.enabled = false;
                servers::edit(|l| l.iter_mut().filter(|x| x.id == e.id).for_each(|x| x.web_status.enabled = false))?;
                job.note("Website status was turned off: its tunnel token wasn't restored. Paste it in Website status to turn it on again.");
            }
            if e.hltv.enabled {
                if let Err(why) = crate::hltv::remote_apply(&e, true) {
                    job.note(&format!("HLTV isn't set up on the machine yet: {why}"));
                }
            }
            if e.web_status.enabled {
                if let Err(why) = crate::webstatus::apply(&e, true) {
                    job.note(&format!("Website status isn't set up on the machine yet: {why}"));
                }
            }
        }
        if self.wants("content") && !self.m.maps.is_empty() {
            match &game {
                Some(g) => queue_downloads(job, g, &self.m.maps),
                None => job.note("The maps and downloads couldn't be queued. Get them again in Maps and Get more."),
            }
        }
        Ok(())
    }
}

/// An Everything included restore's mod files: those that may be restored (`checked`, and
/// listed in the backup's own install record `theirs`) are written (`write`) through this
/// game's record `rec`, which backs up a file that's here and isn't setup's before it's
/// replaced, as setup does. The backup's record is merged in, never copied over: components
/// whose files are now all here are added, everything already here stays, and the rest are
/// returned for setup to install. Returns (components to install, whether Loner-Strike
/// itself is incomplete, files written, files refused).
fn merge_mods(rec: &mut setup::record::Record, theirs: &setup::record::Data, mods: &[&Item], write: &mut dyn FnMut(&Item, &Path) -> Result<(), String>) -> Result<Merged, String> {
    let gdir = rec.game.clone();
    let listed: HashMap<String, (&String, &setup::record::FileInfo)> = theirs.files.iter().map(|(k, v)| (k.replace('\\', "/").to_ascii_lowercase(), (k, v))).collect();
    // a file this record already has keeps its spelling here (no second entry for it)
    let ours: HashMap<String, String> = rec.data.files.keys().map(|k| (k.replace('\\', "/").to_ascii_lowercase(), k.clone())).collect();
    let (mut n, mut refused, mut failed) = (0, 0, Vec::new());
    for it in mods.iter().filter(|i| !i.path.eq_ignore_ascii_case(RECORD)) {
        let found = listed.get(&it.path.to_ascii_lowercase()).filter(|(k, _)| safe_rel(k).is_some());
        let (Some("mods"), Some((key, info))) = (checked(it), found) else {
            refused += 1;
            continue;
        };
        let key: &String = ours.get(&key.replace('\\', "/").to_ascii_lowercase()).unwrap_or(key);
        let had = rec.data.files.get(key.as_str()).cloned();
        let mut dest = Ok(None);
        for o in info.owners().iter().filter(|o| !o.is_empty()) {
            rec.begin(o);
            dest = rec.prepare(key).map(Some);
            if dest.is_err() {
                break;
            }
        }
        // one file that can't be written doesn't stop the rest: its component is left for
        // setup to install, and the record says what it did before
        match dest.and_then(|d| d.ok_or_else(|| "no owner".to_string()).and_then(|d| write(it, &d))) {
            Ok(()) => n += 1,
            Err(why) if why == "no owner" => refused += 1,
            Err(why) => {
                match had {
                    Some(info) => rec.data.files.insert(key.to_string(), info),
                    None => rec.data.files.remove(key.as_str()),
                };
                failed.push(format!("{} ({why})", it.path));
            }
        }
    }
    // what the backup's record lists that isn't setup's here now is setup's to install
    let (mut missing, mut base) = (Vec::new(), false);
    let now: HashSet<String> = rec.data.files.keys().map(|k| k.replace('\\', "/").to_ascii_lowercase()).collect();
    for (rel, info) in &theirs.files {
        let here = now.contains(&rel.replace('\\', "/").to_ascii_lowercase()) && setup::record::inside(&gdir, rel).is_some_and(|p| p.exists());
        if here {
            continue;
        }
        for o in info.owners() {
            if o == "foundation" || o == "core" {
                base = true;
            } else if !o.is_empty() && !missing.contains(&o) {
                missing.push(o);
            }
        }
    }
    for (c, v) in &theirs.components {
        let incomplete = missing.contains(c) || (base && (c == "foundation" || c == "core"));
        if incomplete || rec.data.components.contains_key(c) || !c.chars().all(|ch| ch.is_ascii_alphanumeric() || "-_.".contains(ch)) {
            continue;
        }
        rec.data.components.insert(c.clone(), v.clone());
        if let Some(m) = theirs.maps.get(c) {
            rec.data.maps.insert(c.clone(), m.clone());
        }
    }
    rec.save()?;
    Ok(Merged { missing, base, written: n, refused, failed })
}

/// What `merge_mods` did.
struct Merged {
    /// components for setup to install
    missing: Vec<String>,
    /// Loner-Strike itself is incomplete
    base: bool,
    written: usize,
    refused: usize,
    /// files that couldn't be written, and why
    failed: Vec<String>,
}

/// Downloads a Light backup's GameBanana items again: the same file when it's still
/// there, else the newest one the app can open. They download in the background.
fn queue_downloads(job: &Job, g: &Game, maps: &[MapItem]) {
    let mut queued = 0;
    let mut failed = Vec::new();
    for (i, it) in maps.iter().enumerate() {
        job.fraction(i as f64 / maps.len() as f64);
        job.detail(&format!("Looking up {} on GameBanana", it.name));
        let r = (|| -> Result<(), String> {
            crate::content::section(&it.section).ok_or("not a kind of download the app knows")?;
            let info = g.store.details(&it.section, it.mod_id).map_err(|e| e.message())?;
            let files = info["files"].as_array().cloned().unwrap_or_default();
            let usable = |f: &&Value| f["supported"] == json!(true);
            let f = files.iter().filter(usable).find(|f| !it.file.is_empty() && f["file"].as_str() == Some(it.file.as_str())).or_else(|| files.iter().find(usable)).ok_or("it has no download the app can use any more")?;
            g.store.start_section_install(&it.section, it.mod_id, f["id"].as_i64().unwrap_or(0)).map(|_| ()).map_err(|e| e.message())
        })();
        match r {
            Ok(()) => queued += 1,
            Err(why) => failed.push(format!("{} ({why})", if it.name.is_empty() { it.mod_id.to_string() } else { it.name.clone() })),
        }
    }
    if queued > 0 {
        job.note(&format!("{queued} map{} and download{} from GameBanana are downloading again in the background (Maps and Get more show them).", if queued == 1 { "" } else { "s" }, if queued == 1 { "" } else { "s" }));
    }
    if !failed.is_empty() {
        job.note(&format!("Couldn't download again: {}.", and_list(&failed)));
    }
}

// ---- API ------------------------------------------------------------------------------

type Reply = (StatusCode, Json<Value>);

fn answer(r: Result<Value, String>) -> Reply {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

/// What can be backed up (this computer's games and every server that's set up), the
/// machines a new server can go on, and the parts.
// A read: only lists what can be backed up (no X-Panel check, like the app's other GETs,
// which the page's getJson sends without it; the POSTs that act keep theirs).
async fn api_info(State(st): State<Shared>) -> Reply {
    let v = tokio::task::spawn_blocking(move || {
        let mut sources: Vec<Value> = Vec::new();
        if let Some(c) = servers::client(&st) {
            for g in c.games.iter().filter(|g| ["cstrike", "czero"].contains(&g.as_str())) {
                sources.push(json!({"id": format!("local:{g}"), "name": format!("Your game ({})", game_title(g)), "game": g, "kind": "game"}));
            }
        }
        for e in servers::load().into_iter().filter(|e| e.ready) {
            sources.push(json!({"id": e.id, "name": e.name, "game": e.game, "kind": "server", "host": e.remote.as_ref().map(|r| r.host.clone())}));
        }
        let machines: Vec<Value> = machines().into_iter().map(|(id, r)| json!({"id": id, "host": r.host, "user": r.user})).collect();
        let parts: Vec<Value> = PARTS.iter().map(|(id, name)| json!({"id": id, "name": name})).collect();
        json!({"sources": sources, "machines": machines, "parts": parts, "kept_dir": crate::sys::app_dir().join("backups").to_string_lossy(), "app_version": env!("CARGO_PKG_VERSION")})
    })
    .await
    .unwrap_or_default();
    (StatusCode::OK, Json(v))
}

#[derive(Deserialize, Default)]
struct ExportReq {
    #[serde(default)]
    sources: Vec<String>,
    /// "light" or "full"
    #[serde(default)]
    content: String,
    #[serde(default)]
    parts: Vec<String>,
    /// locks the passwords and webhooks in (empty: none are included)
    #[serde(default)]
    password: String,
    /// the app's own preferences too (AppPrefs)
    #[serde(default)]
    app_prefs: bool,
}

fn sources_of(st: &Shared, ids: &[String]) -> Result<Vec<Source>, String> {
    if ids.is_empty() {
        return Err("Pick what to back up.".into());
    }
    let mut out: Vec<Source> = Vec::new();
    for id in ids {
        if !out.iter().any(|s| s.id == *id) {
            out.push(source(st, id)?);
        }
    }
    Ok(out)
}

/// How big a backup would be (Everything included can be large).
async fn api_estimate(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ExportReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let parts = clean_parts(&req.parts);
        let mut total = 0;
        let mut each = Vec::new();
        for src in sources_of(&st, &req.sources)? {
            let p = plan(&st, &src, req.content == "full", &parts, false)?;
            total += p.manifest.bytes;
            each.push(json!({"id": src.id, "bytes": p.manifest.bytes, "files": p.manifest.files.len(), "maps": p.manifest.maps.len(), "components": p.manifest.components.len()}));
        }
        Ok(json!({"bytes": total, "sources": each}))
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

async fn api_export(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ExportReq>) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err("Not available.".into())) };
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let parts = clean_parts(&req.parts);
        if parts.is_empty() {
            return Err("Pick at least one part to back up.".into());
        }
        // used exactly as typed (spaces too), as restoring does
        let password = req.password.clone();
        if !password.is_empty() && password.chars().count() < 6 {
            return Err("Pick a password of at least 6 characters for the passwords and webhooks.".into());
        }
        let sources = sources_of(&st, &req.sources)?;
        let mut planned = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        for src in &sources {
            // Export all goes on without a machine that can't be reached
            let mut p = match plan(&st, src, req.content == "full", &parts, true) {
                Ok(p) => p,
                Err(why) if sources.len() > 1 => {
                    skipped.push(format!("{} was left out: {why}", src.name));
                    continue;
                }
                Err(why) => return Err(why),
            };
            if !password.is_empty() {
                let mut secrets = gather_secrets(src);
                secrets.lines = std::mem::take(&mut p.stripped);
                if !secrets.is_empty() {
                    p.manifest.secrets = Some(seal(&secrets, &password)?);
                }
            }
            planned.push(p);
        }
        if planned.is_empty() {
            return Err(skipped.join(" "));
        }
        let name = match planned.as_slice() {
            [one] => format!("{}-{}.{EXT}", one.manifest.old_id.replace(':', "-"), crate::sys::stamp()),
            _ => format!("loner-strike-backup-{}.{EXT}", crate::sys::stamp()),
        };
        let Some(path) = app.dialog().file().set_title("Save the backup").set_file_name(&name).add_filter("Loner-Strike backup", &[EXT]).blocking_save_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let prefs = req.app_prefs.then(AppPrefs::now);
        let (bytes, gone) = write_backup(&path, &mut planned, prefs.as_ref())?;
        skipped.extend(gone);
        let each: Vec<Value> = planned
            .iter()
            .map(|p| json!({"name": p.manifest.name, "files": p.manifest.files.len(), "maps": p.manifest.maps.len(), "components": p.manifest.components.len(), "secrets": p.manifest.secrets.is_some(), "odd_names": p.odd_names, "passwords_left_out": p.stripped.len()}))
            .collect();
        Ok(json!({"ok": true, "bytes": bytes, "path": path.to_string_lossy(), "sources": each, "skipped": skipped, "app_prefs": prefs.is_some()}))
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

/// Picks a backup (or an old setup file) and says what's in it.
async fn api_open(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err("Not available.".into())) };
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let Some(path) = app.dialog().file().set_title("Open a Loner-Strike backup").add_filter("Loner-Strike backup", &[EXT, "json"]).blocking_pick_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let opened = open_backup(&path)?;
        let sources: Vec<Value> = opened
            .sources
            .iter()
            .enumerate()
            .map(|(i, (_, m))| {
                json!({"index": i, "source": m.source, "name": m.name, "game": m.game, "content": m.content, "secrets": m.secrets.is_some(), "secrets_kept": m.secrets.is_some() && opened.key.is_some(),
                       "bytes": m.bytes, "app_version": m.app_version, "created": m.created, "parts": m.parts,
                       "components": m.components.len(), "maps": m.maps.len(), "files": m.files.len(),
                       "plugin_code": m.files.iter().filter(|i| i.part == "plugins" && code(&i.path) && checked(i).is_some()).count()})
            })
            .collect();
        let prefs = opened.prefs.clone();
        let token = format!("{:x}", rand::random::<u64>());
        let mut all = OPENED.lock().unwrap_or_else(|e| e.into_inner());
        let all = all.get_or_insert_with(HashMap::new);
        // only the last few stay open: the oldest goes
        while all.len() >= 8 {
            let Some(oldest) = all.iter().min_by_key(|(_, (n, _))| *n).map(|(k, _)| k.clone()) else { break };
            all.remove(&oldest);
        }
        let n = all.values().map(|(n, _)| *n).max().unwrap_or(0) + 1;
        all.insert(token.clone(), (n, Arc::new(opened)));
        let machines: Vec<Value> = machines().into_iter().map(|(id, r)| json!({"id": id, "host": r.host, "user": r.user})).collect();
        Ok(json!({"ok": true, "file": token, "name": path.file_name().map(|n| n.to_string_lossy().to_string()), "sources": sources, "machines": machines, "app_prefs": prefs}))
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

#[derive(Deserialize)]
struct RestoreReq {
    file: String,
    #[serde(default)]
    index: usize,
    /// "new-local", "new-remote", "server" (over one that's there) or "game"
    target: String,
    /// new-remote: the machine ("user@host:port", from machines)
    #[serde(default)]
    machine: String,
    /// server: which one
    #[serde(default)]
    server: String,
    /// a new server's name (empty: the backup's)
    #[serde(default)]
    name: String,
    #[serde(default)]
    parts: Vec<String>,
    #[serde(default)]
    password: String,
    /// go on without the passwords and webhooks
    #[serde(default)]
    skip_secrets: bool,
    /// restore the player's own compiled plugins (code from the backup) too
    #[serde(default)]
    plugin_code: bool,
    /// update mods an Everything included backup has from an older version (on by default)
    #[serde(default = "yes")]
    update_mods: bool,
}

fn yes() -> bool {
    true
}

/// Servers ("<id>") and games ("game:<folder>") being restored: they can't be started.
static RESTORING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Marks a server or game as being restored until it's dropped (the end of the job).
struct Restoring(String);

impl Restoring {
    fn new(key: String) -> Result<Restoring, String> {
        let mut all = RESTORING.lock().unwrap_or_else(|e| e.into_inner());
        if !all.get_or_insert_with(HashSet::new).insert(key.clone()) {
            return Err("That's being restored already.".into());
        }
        Ok(Restoring(key))
    }
}

impl Drop for Restoring {
    fn drop(&mut self) {
        if let Some(all) = RESTORING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            all.remove(&self.0);
        }
    }
}

fn game_key(hl: &Path) -> String {
    format!("game:{}", hl.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase())
}

/// Whether a server (by id) is being restored, so Start can say why it won't.
pub fn restoring_server(id: &str) -> bool {
    RESTORING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.contains(id))
}

/// Whether the game in this Half-Life folder is being restored, so Play can say why it won't.
pub fn restoring_game(hl: &Path) -> bool {
    RESTORING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.contains(&game_key(hl)))
}

pub const RESTORING_SERVER: &str = "This server is being restored from a backup. Start it when that's finished.";
pub const RESTORING_SERVER_BUSY: &str = "This server is being restored from a backup. Wait for that to finish first.";
pub const RESTORING_GAME: &str = "The game is being restored from a backup. Play when that's finished.";

/// Bytes free where `path` is (or its nearest folder that's there).
fn free_space(path: &Path) -> Option<u64> {
    let p = path.ancestors().find(|p| p.is_dir())?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut free = 0u64;
        // SAFETY: a nul-terminated path, and out-pointers that live through the call
        let ok = unsafe { windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
        (ok != 0).then_some(free)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
        // SAFETY: a nul-terminated path and a statvfs that lives through the call
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
            return None;
        }
        Some(s.f_bavail as u64 * s.f_frsize as u64)
    }
}

/// The bytes a restore of `parts` writes from the backup, if the list adds up.
fn restore_bytes(m: &Manifest, parts: &[String]) -> Result<u64, String> {
    let mut total: u64 = 0;
    for it in m.files.iter().filter(|i| parts.contains(&i.part)) {
        if it.size > MAX_FILE {
            return Err(DAMAGED.into());
        }
        total = total.saturating_add(it.size);
    }
    if total > MAX_TOTAL {
        return Err(DAMAGED.into());
    }
    Ok(total)
}

fn start_restore(st: &Shared, req: RestoreReq) -> Result<Value, String> {
    let opened = OPENED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&req.file).map(|(_, o)| o.clone())).ok_or("Open the backup again: Loner-Strike lost track of it.")?;
    let (prefix, m) = opened.sources.get(req.index).cloned().ok_or("That isn't in the backup.")?;
    let parts: Vec<String> = clean_parts(&req.parts).into_iter().filter(|p| m.parts.contains(p)).collect();
    if parts.is_empty() {
        return Err("Pick at least one part to restore.".into());
    }
    let mut notes = Vec::new();
    let secrets = match (&m.secrets, req.skip_secrets) {
        (Some(sealed), false) => {
            // a copy kept on this computer opens with the key saved next to it
            let password = if req.password.is_empty() { opened.key.clone().unwrap_or_default() } else { req.password.clone() };
            if password.is_empty() {
                return Ok(json!({"error": "This backup has passwords and webhooks locked with a password. Type it, or restore without them.", "need_password": true}));
            }
            match unseal(sealed, &password) {
                Ok(s) => Some(s),
                Err(e) if e == WRONG_PASSWORD => return Ok(json!({"error": e, "wrong_password": true})),
                Err(e) => return Err(e),
            }
        }
        (Some(_), true) => {
            notes.push("Passwords and webhooks were left out, as you chose.".to_string());
            None
        }
        (None, _) => None,
    };
    if setup::current().state == "running" {
        return Err("Something else is being set up. Wait for it to finish, then restore.".into());
    }
    let res = crate::setup_api::resources_for(st)?;
    let saved = m.server.as_ref().filter(|_| parts.iter().any(|p| p == "settings"));
    let name = [req.name.as_str(), saved.map(|s| s.name.as_str()).unwrap_or(""), m.name.as_str()].iter().map(|n| clean_server_name(n)).find(|n| !n.is_empty() && !n.starts_with("Your game")).unwrap_or_else(|| format!("{} server", game_title(&m.game)));
    let target = match req.target.as_str() {
        "new-local" => Target::NewLocal(new_entry(st, &m, saved, &name, None, &mut notes)?),
        "new-remote" => {
            let (_, r) = machines().into_iter().find(|(id, _)| *id == req.machine).ok_or("Pick one of your Linux machines.")?;
            Target::NewRemote(new_entry(st, &m, saved, &name, Some(r), &mut notes)?)
        }
        "server" => {
            let e = servers::get(&req.server).filter(|e| e.ready).ok_or("That server isn't there any more, or isn't set up yet.")?;
            if e.game != m.game {
                return Err(format!("That backup is for {} and {} runs {}. Pick a server for the same game.", setup::game_name(&m.game), e.name, setup::game_name(&e.game)));
            }
            if e.remote.is_none() && servers::running(&e.id) {
                return Err(format!("Stop {} first, then restore.", e.name));
            }
            Target::Server(e)
        }
        "game" => {
            if crate::sys::game_running() {
                return Err("Counter-Strike is open. Close it, then try again: the game's files can't change while it's running.".into());
            }
            Target::Game(own_install(st, &m.game).ok_or_else(|| format!("{} isn't installed on this computer.", setup::game_name(&m.game)))?)
        }
        _ => return Err("Pick where to restore it.".into()),
    };
    // room for it: what it writes, a copy of what's there, and Valve's server for a new one
    let need = restore_bytes(&m, &parts)?;
    let (dir, extra) = match &target {
        Target::NewLocal(e) => (e.dir.clone(), 1_500_000_000),
        Target::NewRemote(e) | Target::Server(e) => (e.dir.clone(), 0),
        Target::Game(i) => (i.hl.clone(), 0),
    };
    let want = need + need / 10 + extra + 200_000_000;
    if let Some(free) = free_space(&dir).filter(|f| *f < want) {
        return Err(format!("There isn't enough free space for this restore: it needs about {} and the drive has {} free. Make some room, then try again.", gb(want), gb(free)));
    }
    let busy = {
        // with starts held, so none slips in between the mark and the check
        let _starts = servers::STARTING.lock().unwrap_or_else(|e| e.into_inner());
        let busy = Restoring::new(match &target {
            Target::Game(i) => game_key(&i.hl),
            Target::NewLocal(e) | Target::NewRemote(e) | Target::Server(e) => e.id.clone(),
        })?;
        match &target {
            Target::Server(e) if e.remote.is_none() && servers::running(&e.id) => return Err(format!("Stop {} first, then restore.", e.name)),
            Target::Game(_) if crate::sys::game_running() => return Err("Counter-Strike is open. Close it, then try again: the game's files can't change while it's running.".into()),
            _ => {}
        }
        busy
    };
    // a server on another machine: asked there, now that it can't be started from here
    // (and without holding up local starts while the machine answers)
    if let Target::Server(e) = &target {
        if let Some(r) = &e.remote {
            match crate::remote::active(r, &e.id) {
                Err(why) => return Err(format!("Couldn't reach {} to check the server is stopped ({why}). Try again when it's back.", r.host)),
                Ok((true, _)) => return Err(format!("Stop {} first, then restore.", e.name)),
                Ok(_) => {}
            }
        }
    }
    let r = Restore { st: st.clone(), plugin_code: req.plugin_code, update_mods: req.update_mods, opened, prefix, m, parts, secrets, target, res, notes, local: st.local_game() };
    let (kind, steps) = r.steps();
    let id = r.entry().map(|e| e.id.clone());
    let new = r.is_new();
    // a new server joins the list first, so its card shows the setup (like New server)
    let made = r.entry().filter(|_| new).map(|e| e.dir.clone());
    // a new server that didn't get going leaves nothing behind
    let undo = |listed: bool| {
        if let Some(id) = id.as_ref().filter(|_| listed) {
            let _ = servers::edit(|l| l.retain(|e| e.id != *id));
            unfinished(Some(id), false);
        }
        if let Some(d) = &made {
            let _ = std::fs::remove_dir_all(d);
        }
    };
    if let Some(e) = r.entry().filter(|_| new) {
        if let Err(why) = servers::make_server_dir(e) {
            undo(false);
            return Err(why);
        }
        let e = e.clone();
        unfinished(Some(&e.id), true);
        if let Err(why) = servers::edit(|l| l.push(e)) {
            undo(true);
            return Err(why);
        }
    }
    let job = setup::start_job(kind, steps, move |job| {
        let _busy = busy;
        r.run(job)
    })
    .inspect_err(|_| undo(true))?;
    Ok(json!({"ok": true, "job": job, "id": id}))
}

async fn api_restore(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<RestoreReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || start_restore(&st, req)).await.unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

#[derive(Deserialize)]
struct PrefsReq {
    file: String,
}

/// Puts back the app preferences from an opened backup.
async fn api_restore_prefs(headers: HeaderMap, Json(req): Json<PrefsReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let opened = OPENED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&req.file).map(|(_, o)| o.clone())).ok_or("Open the backup again: Loner-Strike lost track of it.")?;
        let prefs = opened.prefs.clone().ok_or("That backup has no app preferences.")?;
        Ok(json!({"ok": true, "notes": prefs.apply()}))
    })
    .await
    .unwrap_or_else(|_| Err(INTERNAL.into()));
    answer(r)
}

/// The app window only: these open file dialogs, read every server's secrets, and reach
/// machines over SSH.
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/backup/info", get(api_info))
        .route("/api/backup/estimate", post(api_estimate))
        .route("/api/backup/export", post(api_export))
        .route("/api/backup/open", post(api_open))
        .route("/api/backup/restore", post(api_restore))
        .route("/api/backup/app-prefs", post(api_restore_prefs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn all_parts() -> Vec<String> {
        clean_parts(&[])
    }

    #[test]
    fn manifest_round_trip() {
        let m = Manifest {
            format: FORMAT.into(),
            version: VERSION,
            source: "server".into(),
            content: "light".into(),
            game: "cstrike".into(),
            name: "Friday".into(),
            old_id: "friday".into(),
            parts: all_parts(),
            server: Some(Saved { name: "Friday".into(), visibility: "listed".into(), maxplayers: 16, ..Default::default() }),
            components: vec!["mode-zp5".into()],
            maps: vec![MapItem { section: "maps".into(), mod_id: 123, name: "de_x".into(), file: "de_x.zip".into() }],
            files: vec![Item { path: format!("{DATA}global.cfg"), part: "settings".into(), kind: "config".into(), size: 3 }],
            secrets: Some(seal(&Secrets { rcon: "abc".into(), ..Default::default() }, "hunter22").unwrap()),
            ..Default::default()
        };
        let text = serde_json::to_string(&m).unwrap();
        assert!(text.contains("\"mod\":123"));
        let back = manifest_from(serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(back, m);
        // not ours, newer, damaged
        assert!(manifest_from(json!({"format": "something-else"})).is_err());
        assert!(manifest_from(json!({"format": FORMAT, "version": 99, "game": "cstrike"})).unwrap_err().contains("newer"));
        assert!(manifest_from(json!({"format": FORMAT, "version": 1, "game": "../x"})).is_err());
    }

    #[test]
    fn paths_from_a_backup() {
        assert!(safe_rel("maps/de_dust2_2x2.bsp").is_some() && safe_rel("sound/music/My Song (1).wav").is_some());
        assert!(safe_rel(&format!("{DATA}global.cfg")).is_some());
        for bad in ["", "/etc/passwd", "../x.cfg", "maps/../../x", "C:/Windows/x.dll", "a\\b.cfg", "maps/./x.bsp", ".hidden", "maps//x.bsp", "hlds.exe", "x/run.sh", "x/y.bat"] {
            assert!(safe_rel(bad).is_none(), "{bad}");
        }
        assert!(safe_rel(&"a/".repeat(13).trim_end_matches('/').to_string()).is_none());
        assert!(code("dlls/mp.dll") && code("addons/metamod/dlls/metamod_i386.so") && code("addons/amxmodx/plugins/x.AMXX") && code("x.asi") && code("libsteam_api.so.1") && !code("maps/x.bsp"));
        // Windows' device names, whatever the case and extension
        for bad in ["maps/CON.bsp", "nul", "sound/aux.wav.wav", "maps/com1.bsp", "LPT9.txt", "models/Con .mdl"] {
            assert!(safe_rel(bad).is_none(), "{bad}");
        }
        for good in ["maps/console.bsp", "maps/com0.bsp", "maps/com10.bsp", "sound/lpt.wav", "maps/nul_arena.bsp"] {
            assert!(safe_rel(good).is_some(), "{good}");
        }
    }

    fn item(path: &str, part: &str, kind: &str) -> Item {
        Item { path: path.into(), part: part.into(), kind: kind.into(), size: 1 }
    }

    /// What a backup claims about a file doesn't count: its part is worked out again, and
    /// code, or what decides what the game loads, is never restored.
    #[test]
    fn untrusted_items_are_checked() {
        assert_eq!(checked(&item("maps/x.bsp", "content", "custom")), Some("content"));
        assert_eq!(checked(&item("server.cfg", "settings", "config")), Some("settings"));
        assert_eq!(checked(&item(&format!("{DATA}admins.json"), "admins", "config")), Some("admins"));
        assert_eq!(checked(&item("addons/amxmodx/plugins/mine.amxx", "plugins", "custom")), Some("plugins"));
        assert_eq!(checked(&item("addons/amxmodx/scripting/mine.sma", "plugins", "custom")), Some("plugins"));
        assert_eq!(checked(&item("addons/amxmodx/data/lang/gm.txt", "mods", "mod")), Some("mods"));
        assert_eq!(checked(&item("sound/zp/x.wav", "mods", "mod")), Some("mods"));
        for bad in [
            // claims a part it isn't
            item("server.cfg", "content", "config"),
            item("maps/x.bsp", "settings", "custom"),
            item("addons/amxmodx/configs/users.ini", "settings", "config"),
            // binaries, wherever they claim to be
            item("dlls/mp.dll", "mods", "mod"),
            item("addons/metamod/dlls/metamod.so", "mods", "mod"),
            item("addons/amxmodx/plugins/gm_core.amxx", "mods", "mod"),
            item("maps/evil.dll", "content", "custom"),
            item("sound/x.asi", "content", "custom"),
            item("addons/amxmodx/plugins/sub/x.amxx", "plugins", "custom"),
            item("addons/amxmodx/modules/x.dll", "content", "custom"),
            // settings files passed off as mod files
            item("addons/amxmodx/configs/maps/x.cfg", "mods", "mod"),
            item("server.cfg", "mods", "mod"),
            item("addons/amxmodx/configs/users.ini", "mods", "mod"),
            item(&format!("{DATA}global.cfg"), "mods", "mod"),
            item(&format!("{DATA}admins.json"), "mods", "mod"),
            item("addons/amxmodx/configs/modes.txt", "mods", "mod"),
            item("addons/metamod/config.ini", "mods", "mod"),
            item("mapcycle.txt", "mods", "mod"),
            // what the game loads, or what setup thinks it installed
            item("liblist.gam", "mods", "mod"),
            item("liblist.gam", "content", "custom"),
            item("addons/metamod/plugins.ini", "mods", "mod"),
            item(RECORD, "mods", "mod"),
            item("addons/amxmodx/configs/plugins.ini", "content", "custom"),
            // not a settings file a backup packs
            item("addons/amxmodx/configs/amxx.cfg", "content", "config"),
            item("autoexec.cfg", "content", "config"),
            item("x.cfg", "settings", "custom"),
            item("maps/x.bsp", "content", "other"),
        ] {
            assert_eq!(checked(&bad), None, "{bad:?}");
        }
    }

    #[test]
    fn passwords_stay_out_of_settings_files() {
        let cfg = "hostname \"Friday\"\r\nRCON_PASSWORD \"hunter2\"\r\n\"sv_password\" abc\r\nsv_password \"\"\r\nsv_passwordx 1\r\nexec x.cfg; adminpassword secret // hi\r\nsv_gravity 800\r\n";
        let (text, taken) = strip_passwords("server.cfg", cfg).unwrap();
        assert!(!text.contains("hunter2") && !text.contains("abc") && !text.contains("secret"), "{text}");
        assert!(text.contains("hostname \"Friday\"\r\n") && text.contains("sv_gravity 800\r\n") && text.contains("sv_passwordx 1") && text.contains("sv_password \"\""));
        assert_eq!(taken.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![1, 2, 5]);
        assert_eq!(text.lines().count(), cfg.lines().count());
        assert_eq!(put_back_passwords(&text, &taken), cfg);
        // users.ini: admins with a password outside Loner-Strike's block
        let ini = format!("; comment \"a\" \"b\"\n\"Boss\" \"pw123\" \"abcdefghijklmnopqrstu\" \"a\"\n\"STEAM_0:1:2\" \"\" \"abc\" \"ce\"\n{}\n\"Them\" \"x\" \"b\" \"a\"\n{}\n", crate::admins::BEGIN, crate::admins::END);
        let (text, taken) = strip_passwords("addons/amxmodx/configs/users.ini", &ini).unwrap();
        assert!(!text.contains("pw123") && text.contains("STEAM_0:1:2") && text.contains("\"Them\" \"x\""));
        assert_eq!(taken.len(), 1);
        assert_eq!(put_back_passwords(&text, &taken), ini);
        assert!(strip_passwords("server.cfg", "hostname x\n").is_none());
        assert!(strip_passwords("maps/x.bsp", "rcon_password x").is_none());
        // a backup made of a folder with them: none in the packed file, all in the secrets
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backuppw-").unwrap();
        let g = root.join("cstrike");
        put(&g, "server.cfg", cfg);
        put(&g, "addons/amxmodx/configs/users.ini", &ini);
        let mut items = pick(&Pick { game_dir: &g, full: false, parts: &all_parts(), stock: &[], known: &HashSet::new(), own_game: false }).items;
        let mut config = HashMap::new();
        let mut stripped = BTreeMap::new();
        for it in items.iter_mut() {
            let raw = std::fs::read(g.join(&it.path)).unwrap();
            if let Some((text, taken)) = strip_passwords(&it.path, &crate::rcon::latin1(&raw)) {
                config.insert(it.path.clone(), latin1_bytes(&text));
                stripped.insert(it.path.clone(), taken);
            }
        }
        assert_eq!(stripped.len(), 2);
        assert!(config.values().all(|b| !String::from_utf8_lossy(b).contains("hunter2") && !String::from_utf8_lossy(b).contains("pw123")));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Files change between planning a backup and saving it: the manifest says what was
    /// packed, and a file that's gone is left out instead of stopping the backup.
    #[test]
    fn saved_sizes_are_what_was_packed() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupgrow-").unwrap();
        let g = root.join("cstrike");
        put(&g, "maps/grows.bsp", "abc");
        put(&g, "maps/goes.bsp", "x");
        put(&g, "maps/stays.bsp", "same");
        let items = pick(&Pick { game_dir: &g, full: false, parts: &all_parts(), stock: &[], known: &HashSet::new(), own_game: false }).items;
        assert_eq!(items.len(), 3);
        put(&g, "maps/grows.bsp", "abcdefgh");
        std::fs::remove_file(g.join("maps/goes.bsp")).unwrap();
        let m = Manifest { format: FORMAT.into(), version: VERSION, game: "cstrike".into(), name: "Friday".into(), content: "light".into(), parts: all_parts(), files: items, ..Default::default() };
        let mut p = [Planned { manifest: m, config: HashMap::new(), game_dir: g.clone(), odd_names: 0, stripped: BTreeMap::new() }];
        let file = root.join(format!("x.{EXT}"));
        let (_, notes) = write_backup(&file, &mut p, None).unwrap();
        assert!(notes.len() == 1 && notes[0].contains("maps/goes.bsp"), "{notes:?}");
        let opened = open_backup(&file).unwrap();
        let (prefix, m) = &opened.sources[0];
        assert_eq!(m.files.len(), 2);
        assert_eq!(m.bytes, 12);
        let mut r = opened.reader().unwrap();
        for it in &m.files {
            r.copy(&zip_name(prefix, it), &root.join("out").join(&it.path), it.size).unwrap();
        }
        assert_eq!(std::fs::read_to_string(root.join("out/maps/grows.bsp")).unwrap(), "abcdefgh");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Restoring without the backup's passwords keeps the ones that were here; a copy kept
    /// before a restore brings its own back by itself.
    #[test]
    fn own_passwords_survive_a_restore() {
        let backed_up = "hostname a\r\nrcon_password \"theirs\"\r\nsv_password x\r\n";
        let (text, _) = strip_passwords("server.cfg", backed_up).unwrap();
        let here = "rcon_password \"mine\"\r\nadminpassword \"adm\"\r\nhostname b\r\n";
        let (_, mine) = strip_passwords("server.cfg", here).unwrap();
        let out = keep_own_passwords("server.cfg", &text, &mine);
        assert!(out.contains("hostname a\r\nrcon_password \"mine\"\r\n"), "{out}");
        assert!(out.ends_with("adminpassword \"adm\"\r\n") && !out.contains("theirs"), "{out}");
        // sv_password wasn't set here: its comment stays, with no password
        assert!(out.contains(&format!("// (sv_password {LEFT_OUT}")));
        let ini = "\"Boss\" \"pw\" \"abc\" \"a\"\n\"Other\" \"o\" \"z\" \"a\"\n";
        let (text, _) = strip_passwords("users.ini", ini).unwrap();
        let (_, mine) = strip_passwords("users.ini", "\"Boss\" \"local\" \"abc\" \"a\"\n").unwrap();
        let out = keep_own_passwords("users.ini", &text, &mine);
        assert!(out.starts_with("\"Boss\" \"local\" \"abc\" \"a\"\n") && !out.contains("\"o\""), "{out}");
        assert_eq!(placeholder(out.lines().nth(1).unwrap()).as_deref(), Some("Other"));

        // the copy kept before a restore: sealed with a key saved next to it
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupkept-").unwrap();
        let g = root.join("cstrike");
        put(&g, "server.cfg", here);
        let mut items = pick(&Pick { game_dir: &g, full: false, parts: &all_parts(), stock: &[], known: &HashSet::new(), own_game: false }).items;
        let (text, taken) = strip_passwords("server.cfg", here).unwrap();
        items[0].size = text.len() as u64;
        let m = Manifest { format: FORMAT.into(), version: VERSION, game: "cstrike".into(), content: "light".into(), parts: all_parts(), files: items, ..Default::default() };
        let p = Planned { manifest: m, config: [("server.cfg".to_string(), latin1_bytes(&text))].into_iter().collect(), game_dir: g.clone(), odd_names: 0, stripped: [("server.cfg".to_string(), taken)].into_iter().collect() };
        let file = root.join(format!("game-before-restore-1.{EXT}"));
        write_kept(&file, p, Secrets { rcon: "r".into(), ..Default::default() }).unwrap();
        assert!(key_path(&file).is_file());
        let opened = open_backup(&file).unwrap();
        let (prefix, m) = &opened.sources[0];
        let secrets = unseal(m.secrets.as_ref().unwrap(), opened.key.as_ref().unwrap()).unwrap();
        assert_eq!(secrets.rcon, "r");
        let packed = String::from_utf8(opened.reader().unwrap().bytes(&zip_name(prefix, &m.files[0])).unwrap()).unwrap();
        assert!(!packed.contains("mine"));
        assert_eq!(put_back_passwords(&packed, &secrets.lines["server.cfg"]), here);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The player's own plugins: only names the backup lists, never Loner-Strike's own or
    /// a file setup installed here.
    #[test]
    fn own_plugins_only() {
        let mine: HashSet<String> = ["mine".to_string(), "gamemodes".to_string(), "setups".to_string()].into_iter().collect();
        let setups: HashSet<String> = ["addons/amxmodx/plugins/setups.amxx".to_string()].into_iter().collect();
        assert!(own_plugin_ok("addons/amxmodx/plugins/Mine.amxx", &mine, &setups));
        assert!(own_plugin_ok("addons/amxmodx/scripting/mine.sma", &mine, &setups));
        for bad in ["addons/amxmodx/plugins/other.amxx", "addons/amxmodx/plugins/gamemodes.amxx", "addons/amxmodx/scripting/gamemodes.sma", "addons/amxmodx/plugins/setups.amxx", "addons/amxmodx/plugins/x/mine.amxx", "maps/mine.amxx"] {
            assert!(!own_plugin_ok(bad, &mine, &setups), "{bad}");
        }
    }

    /// The copies kept before restores: the newest 5 stay, and so does the one being
    /// restored from, however old; half-written ones go.
    #[test]
    fn the_copy_restored_from_is_kept() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupprune-").unwrap();
        let name = |i: u32| root.join(format!("srv-before-restore-2026010{i}-000000.{EXT}"));
        for i in 1..=7 {
            std::fs::write(name(i), "zip").unwrap();
            std::fs::write(key_path(&name(i)), "key").unwrap();
        }
        std::fs::write(root.join(format!("other-before-restore-20250101-000000.{EXT}")), "zip").unwrap();
        let part = root.join(format!("x.{EXT}.part"));
        std::fs::write(&part, "half").unwrap();
        prune_kept(&name(7), &[name(7), name(1)]);
        assert!(name(1).is_file() && key_path(&name(1)).is_file(), "the one restored from");
        assert!(!name(2).exists() && !key_path(&name(2)).exists());
        for i in 3..=7 {
            assert!(name(i).is_file(), "{i}");
        }
        assert!(root.join(format!("other-before-restore-20250101-000000.{EXT}")).is_file());
        // a fresh .part may belong to a backup being written right now
        assert!(part.is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A settings file is written once, with its passwords already in.
    #[test]
    fn settings_files_are_written_whole() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupfill-").unwrap();
        let dest = root.join("cstrike").join("server.cfg");
        let original = "hostname a\nrcon_password \"theirs\"\n";
        let (packed, taken) = strip_passwords("server.cfg", original).unwrap();
        // the backup's own, unlocked
        write_settings("server.cfg", &dest, packed.as_bytes(), Some(&taken)).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), original);
        // without them: this computer's own stay
        std::fs::write(&dest, "rcon_password \"mine\"\n").unwrap();
        write_settings("server.cfg", &dest, packed.as_bytes(), None).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hostname a\nrcon_password \"mine\"\n");
        // nothing half-written is left beside it
        let left: Vec<String> = std::fs::read_dir(dest.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(left, vec!["server.cfg".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sizes_must_match() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupsize-").unwrap();
        let out = root.join("a").join("x.bsp");
        assert!(write_stream(&mut &b"12345"[..], &out, 4).unwrap_err().contains("isn't the size"));
        assert!(!out.exists() && !root.join("a").join("x.bsp.lsrestore").exists());
        assert!(write_stream(&mut &b"123"[..], &out, 4).is_err());
        assert_eq!(write_stream(&mut &b"1234"[..], &out, 4).unwrap(), 4);
        assert!(write_stream(&mut &b""[..], &out, MAX_FILE + 1).is_err());
        let m = Manifest { files: vec![Item { size: MAX_FILE + 1, ..item("maps/x.bsp", "content", "custom") }], ..Default::default() };
        assert!(restore_bytes(&m, &all_parts()).is_err());
        assert_eq!(restore_bytes(&m, &["settings".to_string()]).unwrap(), 0);
        assert!(free_space(&root.join("not-there-yet")).is_some_and(|f| f > 0));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Everything included over a game that's there: setup's record is merged, program
    /// files are left to setup, and files that aren't setup's here are backed up first.
    #[test]
    fn mods_merge_into_the_record() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupmods-").unwrap();
        let g = root.join("cstrike");
        put(&g, "liblist.gam", "valve");
        put(&g, "addons/amxmodx/data/lang/zp.txt", "the player's own");
        put(&g, "addons/amxmodx/configs/zp.cfg", "here");
        put(&g, RECORD, r#"{"files": {"addons/amxmodx/configs/mine.cfg": {"components": ["mode-x"]}}, "components": {"foundation": {}, "core": {}, "mode-x": {}}}"#);
        put(&g, "addons/amxmodx/configs/mine.cfg", "here");
        let theirs: setup::record::Data = serde_json::from_value(json!({
            "files": {
                "liblist.gam": {"components": ["foundation"]},
                "dlls/mp.dll": {"components": ["regamedll"]},
                "addons/amxmodx/configs/zp.cfg": {"components": ["mode-cfgx"]},
                "addons/amxmodx/data/lang/zp.txt": {"components": ["mode-zp5"]},
                "sound/zp/extra.wav": {"components": ["mode-zp5"]}
            },
            "components": {"foundation": {}, "core": {}, "mode-zp5": {"version": 1}, "regamedll": {}, "mode-cfgx": {}}
        }))
        .unwrap();
        let items = vec![
            item("addons/amxmodx/data/lang/zp.txt", "mods", "mod"),
            item("sound/zp/extra.wav", "mods", "mod"),
            item("addons/amxmodx/configs/zp.cfg", "mods", "mod"),
            item("dlls/mp.dll", "mods", "mod"),
            item("liblist.gam", "mods", "mod"),
            item("addons/x/unlisted.txt", "mods", "mod"),
            item(RECORD, "mods", "mod"),
        ];
        let refs: Vec<&Item> = items.iter().collect();
        let mut rec = setup::record::Record::open(&g);
        let mut written = Vec::new();
        let Merged { missing, base, written: n, refused, failed } = merge_mods(&mut rec, &theirs, &refs, &mut |it, dest| {
            written.push(it.path.clone());
            std::fs::write(dest, "from the backup").map_err(|e| e.to_string())
        })
        .unwrap();
        assert!(failed.is_empty());
        assert_eq!(written, vec!["addons/amxmodx/data/lang/zp.txt", "sound/zp/extra.wav"]);
        assert_eq!((n, refused), (2, 4));
        // a settings file passed off as a mod file, and the dll, come from setup; liblist.gam
        // isn't setup's here, so Loner-Strike is repaired
        assert_eq!(missing, vec!["mode-cfgx".to_string(), "regamedll".to_string()]);
        assert!(base);
        assert_eq!(std::fs::read_to_string(g.join("addons/amxmodx/configs/zp.cfg")).unwrap(), "here");
        let rec = setup::record::Record::open(&g);
        // the target's own component and file stay; mode-zp5 is complete, the others aren't
        assert!(rec.installed("mode-x") && rec.ours("addons/amxmodx/configs/mine.cfg"));
        assert!(rec.installed("mode-zp5") && !rec.installed("regamedll") && !rec.installed("mode-cfgx"));
        // the player's own file was kept, to be put back if it's ever removed
        let info = &rec.data.files["addons/amxmodx/data/lang/zp.txt"];
        assert_eq!(std::fs::read_to_string(rec.dir.join(info.backup.as_ref().unwrap())).unwrap(), "the player's own");
        assert!(rec.data.files["sound/zp/extra.wav"].backup.is_none());
        assert_eq!(std::fs::read_to_string(g.join("liblist.gam")).unwrap(), "valve");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn runtime_files_stay_behind() {
        for f in ["addons/amxmodx/logs/L0101.log", "maps/de_x.nav", "stats.json.part", "addons/amxmodx/data/gamemodes/status.json", "addons/amxmodx/data/gamemodes/stats.json", "addons/amxmodx/data/gamemodes/panel_rcon.txt", "addons/amxmodx/data/gamemodes/friends.cfg", "addons/amxmodx/data/gamemodes/seen.log.reading", "addons/amxmodx/data/gamemodes/content_status.json", "qconsole.log"] {
            assert!(runtime(f), "{f}");
        }
        for f in ["addons/amxmodx/data/gamemodes/global.cfg", "addons/amxmodx/data/gamemodes/admins.json", "addons/amxmodx/data/gamemodes/friends.json", "maps/de_x.bsp", "server.cfg"] {
            assert!(!runtime(f), "{f}");
        }
        assert_eq!(part_of("addons/amxmodx/configs/users.ini", false), "admins");
        assert_eq!(part_of("banned.cfg", false), "bans");
        assert_eq!(part_of("mapcycle.txt", false), "rotation");
        assert_eq!(part_of(&format!("{DATA}rotation_zp5.txt"), false), "rotation");
        assert_eq!(part_of(&format!("{DATA}global.cfg"), false), "settings");
        assert_eq!(part_of("addons/amxmodx/plugins/mine.amxx", false), "plugins");
        assert_eq!(part_of("addons/amxmodx/plugins/gm_core.amxx", true), "mods");
        assert_eq!(part_of("maps/de_x.bsp", false), "content");
    }

    #[test]
    fn secrets_lock_and_unlock() {
        let s = Secrets {
            discord: [("live".to_string(), "https://discord.com/api/webhooks/1/abc".to_string())].into_iter().collect(),
            tunnel_token: "token".into(),
            rcon: "rcon-pw".into(),
            join_password: "join".into(),
            hltv_password: "tv".into(),
            lines: [("server.cfg".to_string(), vec![(3, "rcon_password x".to_string())])].into_iter().collect(),
        };
        let sealed = seal(&s, "correct horse").unwrap();
        // nothing readable in the file
        let text = serde_json::to_string(&sealed).unwrap();
        assert!(!text.contains("rcon-pw") && !text.contains("discord"));
        assert_eq!(unseal(&sealed, "correct horse").unwrap(), s);
        assert_eq!(unseal(&sealed, "wrong horse").unwrap_err(), WRONG_PASSWORD);
        // two seals of the same secrets differ (fresh salt and nonce)
        let again = seal(&s, "correct horse").unwrap();
        assert!(again.salt != sealed.salt && again.nonce != sealed.nonce);
        // tampered with, or asking for an absurd amount of work
        let mut bad = sealed.clone();
        bad.data.replace_range(0..2, if &bad.data[0..2] == "00" { "01" } else { "00" });
        assert!(unseal(&bad, "correct horse").is_err());
        let mut greedy = sealed.clone();
        greedy.m_cost = 4_000_000;
        assert_eq!(unseal(&greedy, "correct horse").unwrap_err(), DAMAGED);
    }

    /// A small server folder: what Light and Everything included pack.
    #[test]
    fn light_and_full_pick_different_files() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backup-").unwrap();
        let g = root.join("srv").join("cstrike");
        let stock = root.join(".base").join("cstrike");
        put(&g, &format!("{DATA}global.cfg"), "bot_quota 5");
        put(&g, &format!("{DATA}stats.json"), "{}");
        put(&g, &format!("{DATA}my_plugins.json"), "[\"mine\"]");
        put(&g, "server.cfg", "hostname x");
        put(&g, "maps/custom.bsp", "bsp");
        put(&g, "maps/custom.nav", "nav");
        put(&g, "maps/de_dust2.bsp", "valve");
        put(&stock, "maps/de_dust2.bsp", "valve");
        put(&g, "maps/fromgb.bsp", "gb");
        put(&g, "addons/amxmodx/plugins/mine.amxx", "amxx");
        put(&g, "addons/amxmodx/plugins/gm_core.amxx", "ours");
        put(&g, "addons/amxmodx/logs/L1.log", "log");
        put(&g, "dlls/mp.dll", "regamedll");
        put(&g, "liblist.gam", "ours");
        put(&g, "addons/amxmodx/configs/gm.cfg", "ours");
        put(&g, "addons/amxmodx/data/lang/gm.txt", "ours");
        put(&g, "addons/loner-strike/backup/liblist.gam", "valve");
        put(&g, "addons/loner-strike/record.json", r#"{"files": {"addons/amxmodx/plugins/gm_core.amxx": {}, "dlls/mp.dll": {}, "liblist.gam": {}, "addons/amxmodx/configs/gm.cfg": {}, "addons/amxmodx/data/lang/gm.txt": {}}, "components": {"foundation": 1, "core": 1}}"#);
        let known: HashSet<String> = ["maps/fromgb.bsp".to_string()].into_iter().collect();
        let parts = all_parts();
        let stock_dirs = vec![stock];
        let paths = |full: bool, parts: &[String]| -> Vec<(String, String, String)> {
            let mut v: Vec<_> = pick(&Pick { game_dir: &g, full, parts, stock: &stock_dirs, known: &known, own_game: false }).items.into_iter().map(|i| (i.path, i.part, i.kind)).collect();
            v.sort();
            v
        };
        let light = paths(false, &parts);
        let names: Vec<&str> = light.iter().map(|(p, _, _)| p.as_str()).collect();
        let (global, mine) = (format!("{DATA}global.cfg"), format!("{DATA}my_plugins.json"));
        assert_eq!(names, vec![global.as_str(), mine.as_str(), "addons/amxmodx/plugins/mine.amxx", "maps/custom.bsp", "server.cfg"]);
        let full = paths(true, &parts);
        let names: Vec<&str> = full.iter().map(|(p, _, _)| p.as_str()).collect();
        for want in ["addons/loner-strike/record.json", "addons/amxmodx/data/lang/gm.txt", "maps/fromgb.bsp", "maps/custom.bsp"] {
            assert!(names.contains(&want), "{want}");
        }
        // program files and liblist.gam come from setup when restoring, so they aren't packed
        for never in ["maps/de_dust2.bsp", "maps/custom.nav", "addons/amxmodx/logs/L1.log", &format!("{DATA}stats.json") as &str, "dlls/mp.dll", "addons/amxmodx/plugins/gm_core.amxx", "liblist.gam", "addons/loner-strike/backup/liblist.gam", "addons/amxmodx/configs/gm.cfg"] {
            assert!(!names.contains(&never), "{never}");
        }
        assert!(full.iter().any(|(p, part, kind)| p == "addons/amxmodx/data/lang/gm.txt" && part == "mods" && kind == "mod"));
        // parts left out stay out
        let only: Vec<String> = vec!["settings".into()];
        assert!(paths(true, &only).iter().all(|(_, part, _)| part == "settings"));
        // the game itself, without Valve's files to compare with: only what came from downloads
        let own = pick(&Pick { game_dir: &g, full: true, parts: &parts, stock: &[], known: &known, own_game: true }).items;
        assert!(own.iter().any(|i| i.path == "maps/fromgb.bsp") && !own.iter().any(|i| i.path == "maps/custom.bsp"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_file_round_trip() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "backupzip-").unwrap();
        let g = root.join("cstrike");
        put(&g, &format!("{DATA}global.cfg"), "bot_quota 5");
        put(&g, "maps/custom.bsp", "bsp");
        let items = pick(&Pick { game_dir: &g, full: false, parts: &all_parts(), stock: &[], known: &HashSet::new(), own_game: false }).items;
        let m = Manifest { format: FORMAT.into(), version: VERSION, game: "cstrike".into(), content: "light".into(), parts: all_parts(), files: items, ..Default::default() };
        let one = Planned { manifest: m.clone(), config: HashMap::new(), game_dir: g.clone(), odd_names: 0, stripped: BTreeMap::new() };
        let two = Planned { manifest: m, config: HashMap::new(), game_dir: g.clone(), odd_names: 0, stripped: BTreeMap::new() };
        let file = root.join(format!("x.{EXT}"));
        let prefs = AppPrefs { mute_joins: true, smart_chat: true, ..Default::default() };
        write_backup(&file, &mut [one, two], Some(&prefs)).unwrap();
        let opened = open_backup(&file).unwrap();
        assert_eq!(opened.sources.len(), 2);
        assert_eq!(opened.prefs, Some(prefs), "the app preferences come back as they went in");
        let (prefix, m) = &opened.sources[1];
        assert_eq!(prefix, "sources/1/");
        let it = m.files.iter().find(|i| i.path == "maps/custom.bsp").unwrap();
        let mut r = opened.reader().unwrap();
        let out = root.join("out").join("custom.bsp");
        r.copy(&zip_name(prefix, it), &out, it.size).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "bsp");
        // an old setup file opens as a backup of the game
        let legacy = root.join("old.json");
        std::fs::write(&legacy, json!({"format": LEGACY, "version": 1, "game": "czero", "components": ["mode-zp5"], "files": {"global.cfg": "x", "../evil.cfg": "y"}}).to_string()).unwrap();
        let old = open_backup(&legacy).unwrap();
        let (_, m) = &old.sources[0];
        assert_eq!((m.game.as_str(), m.source.as_str(), m.files.len()), ("czero", "game", 1));
        assert!(open_backup(&root.join("out").join("custom.bsp")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
