//! Server extras, like real servers have, per game or dedicated server:
//!
//! - Map rotation. The main list is the game's own map cycle: the file the mapcyclefile
//!   cvar names (mapcycle.txt unless server.cfg says otherwise), which the game,
//!   nextmap.amxx and mapchooser.amxx all read. A Game Mode Menu mode can have its own
//!   list, <data>/rotation_<mode>.txt, used while that mode is on. gm_extras.amxx picks
//!   the next map from them (amx_nextmap); a map vote still wins.
//! - Timed chat messages: <data>/ads.txt.
//! - Away (AFK) and high-ping kicks, and fast downloads (sv_downloadurl): cvars in
//!   <data>/extras.cfg, which gm_extras.amxx runs on every map and on gm_extrasreload.
//! - Join and leave messages (with the country, from AMX Mod X's geoip module and the
//!   GeoLite2 database its package ships), vote kick and vote ban, and going back to a
//!   default map and mode when the server empties: cvars in extras.cfg too.
//! - A dedicated server's daily restart, kept with the server (servers.rs runs it).
//!
//! The settings themselves live in <data>/extras.json. Everything written is built
//! from validated map names, numbers, a checked URL and cleaned one-line messages.

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The one command sent to the game: re-read extras.cfg, ads.txt and the rotation.
const RELOAD: &str = "gm_extrasreload";
const MAX_ROTATION: usize = 200;
const MAX_ADS: usize = 40;
const MAX_AD: usize = 150;
const MAX_PREFIX: usize = 24;
const COLORS: &[(&str, char)] = &[("default", 'd'), ("green", 'g'), ("red", 'r'), ("blue", 'b'), ("grey", 's')];
static LOCK: Mutex<()> = Mutex::new(());

// ---- settings ---------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ad {
    pub text: String,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_color() -> String {
    "default".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Ads {
    pub on: bool,
    /// minutes between messages
    pub minutes: u32,
    pub random: bool,
    pub prefix: String,
    pub messages: Vec<Ad>,
    /// now and then, a line saying the server runs Loner-Strike (on unless turned off)
    pub credit: bool,
}

impl Default for Ads {
    fn default() -> Self {
        Ads { on: false, minutes: 3, random: false, prefix: "[Server]".into(), messages: vec![], credit: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Afk {
    pub on: bool,
    /// seconds without a key press before spectator
    pub spec: u32,
    /// seconds in spectator before a kick; 0 never
    pub kick: u32,
}

impl Default for Afk {
    fn default() -> Self {
        Afk { on: false, spec: 120, kick: 180 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Ping {
    pub on: bool,
    /// milliseconds
    pub max: u32,
    /// checks in a row (10 seconds apart) before a kick
    pub checks: u32,
}

impl Default for Ping {
    fn default() -> Self {
        Ping { on: false, max: 250, checks: 5 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Joins {
    pub on: bool,
    /// "(Germany)" after the name, when the geoip data is there
    pub country: bool,
    pub skip_bots: bool,
}

impl Default for Joins {
    fn default() -> Self {
        Joins { on: false, country: true, skip_bots: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Votes {
    pub kick: bool,
    pub ban: bool,
    /// share of the players who can vote that must say yes
    pub percent: u32,
    /// players on the server before a vote can start
    pub min_players: u32,
    pub ban_minutes: u32,
    /// minutes between votes
    pub cooldown: u32,
    /// bots can be vote-kicked
    pub kick_bots: bool,
}

impl Default for Votes {
    fn default() -> Self {
        Votes { kick: false, ban: false, percent: 60, min_players: 3, ban_minutes: 30, cooldown: 3, kick_bots: false }
    }
}

/// Back to a map (and mode) once the last real player has left.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Empty {
    pub on: bool,
    pub minutes: u32,
    pub map: String,
    /// a Game Mode Menu mode id; "" keeps the mode
    pub mode: String,
}

impl Default for Empty {
    fn default() -> Self {
        Empty { on: false, minutes: 5, map: String::new(), mode: String::new() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Settings {
    /// a random map from the rotation instead of the next one
    pub rotation_random: bool,
    /// rtv and nominations offer only the rotation's maps
    pub rtv_cycle: bool,
    pub ads: Ads,
    pub afk: Afk,
    pub ping: Ping,
    /// sv_downloadurl; empty: off
    pub fastdl: String,
    pub joins: Joins,
    pub votes: Votes,
    pub empty: Empty,
}

/// One line for the game's chat: no control characters, at most `max` characters.
fn one_line(s: &str, max: usize) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().chars().take(max).collect::<String>().trim().to_string()
}

/// An http(s) address that can sit inside a quoted cfg value.
pub fn valid_url(s: &str) -> bool {
    let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/') && !r.starts_with('.'))
        && s.len() <= 120
        && s.bytes().all(|c| c.is_ascii_graphic() && !b"\"';\\<>`{}|^".contains(&c))
}

/// Settings brought into range; Err for what can't be fixed quietly.
pub fn tidy(mut s: Settings) -> Result<Settings, String> {
    s.ads.minutes = s.ads.minutes.clamp(1, 60);
    s.ads.prefix = one_line(&s.ads.prefix, MAX_PREFIX);
    if s.ads.messages.len() > MAX_ADS {
        return Err(format!("Keep it to {MAX_ADS} messages."));
    }
    s.ads.messages = s
        .ads
        .messages
        .into_iter()
        .map(|a| Ad { text: one_line(&a.text, MAX_AD), color: if COLORS.iter().any(|c| c.0 == a.color) { a.color } else { default_color() } })
        .filter(|a| !a.text.is_empty())
        .collect();
    s.afk.spec = s.afk.spec.clamp(30, 3600);
    if s.afk.kick > 0 {
        s.afk.kick = s.afk.kick.clamp(30, 3600);
    }
    s.ping.max = s.ping.max.clamp(50, 1000);
    s.ping.checks = s.ping.checks.clamp(2, 20);
    s.fastdl = s.fastdl.trim().to_string();
    if !s.fastdl.is_empty() && !valid_url(&s.fastdl) {
        return Err("The download address has to start with http:// or https://, with no spaces or quotes.".into());
    }
    s.votes.percent = s.votes.percent.clamp(30, 100);
    s.votes.min_players = s.votes.min_players.clamp(2, 32);
    s.votes.ban_minutes = s.votes.ban_minutes.clamp(1, 10080);
    s.votes.cooldown = s.votes.cooldown.min(60);
    s.empty.minutes = s.empty.minutes.clamp(1, 120);
    s.empty.map = s.empty.map.trim().to_string();
    s.empty.mode = s.empty.mode.trim().to_string();
    if !s.empty.map.is_empty() && !valid_map(&s.empty.map) {
        return Err("That isn't a map name.".into());
    }
    if !s.empty.mode.is_empty() && !valid_mode(&s.empty.mode) {
        return Err("Unknown game mode.".into());
    }
    if s.empty.on && s.empty.map.is_empty() {
        return Err("Pick the map to go back to.".into());
    }
    Ok(s)
}

/// extras.cfg, run by gm_extras.amxx.
pub fn cfg(s: &Settings) -> String {
    let b = |v: bool| if v { 1 } else { 0 };
    let ads = if s.ads.on && !s.ads.messages.is_empty() { s.ads.minutes } else { 0 };
    let url = if valid_url(&s.fastdl) { s.fastdl.as_str() } else { "" };
    let empty_ok = s.empty.on && valid_map(&s.empty.map);
    [
        "// Written by Loner-Strike's Server extras page. Change it there.".to_string(),
        format!("gm_rotation_random {}", b(s.rotation_random)),
        format!("gm_rtv_cycle {}", b(s.rtv_cycle)),
        format!("gm_ads_interval {ads}"),
        format!("gm_ads_random {}", b(s.ads.random)),
        format!("gm_credit {}", b(s.ads.credit)),
        format!("gm_afk {}", b(s.afk.on)),
        format!("gm_afk_spec {}", s.afk.spec),
        format!("gm_afk_kick {}", s.afk.kick),
        format!("gm_ping {}", b(s.ping.on)),
        format!("gm_ping_max {}", s.ping.max),
        format!("gm_ping_checks {}", s.ping.checks),
        format!("gm_fastdl \"{url}\""),
        format!("gm_joins {}", b(s.joins.on)),
        format!("gm_joins_country {}", b(s.joins.country)),
        format!("gm_joins_skipbots {}", b(s.joins.skip_bots)),
        format!("gm_votekick {}", b(s.votes.kick)),
        format!("gm_voteban {}", b(s.votes.ban)),
        format!("gm_vote_percent {}", s.votes.percent),
        format!("gm_vote_min {}", s.votes.min_players),
        format!("gm_vote_bantime {}", s.votes.ban_minutes),
        format!("gm_vote_cooldown {}", s.votes.cooldown),
        format!("gm_vote_bots {}", b(s.votes.kick_bots)),
        format!("gm_empty {}", if empty_ok { s.empty.minutes } else { 0 }),
        format!("gm_empty_map \"{}\"", if empty_ok { s.empty.map.as_str() } else { "" }),
        format!("gm_empty_mode \"{}\"", if empty_ok && valid_mode(&s.empty.mode) { s.empty.mode.as_str() } else { "" }),
        String::new(),
    ]
    .join("\r\n")
}

/// ads.txt: the prefix, then one "<color letter> <message>" per line.
pub fn ads_txt(s: &Settings) -> String {
    let mut out = vec!["; Written by Loner-Strike's Server extras page. Change it there.".to_string()];
    let prefix = one_line(&s.ads.prefix, MAX_PREFIX);
    if !prefix.is_empty() {
        out.push(format!("prefix {prefix}"));
    }
    for a in &s.ads.messages {
        let text = one_line(&a.text, MAX_AD);
        if text.is_empty() {
            continue;
        }
        let letter = COLORS.iter().find(|c| c.0 == a.color).map(|c| c.1).unwrap_or('d');
        out.push(format!("{letter} {text}"));
    }
    out.push(String::new());
    out.join("\r\n")
}

fn load(data: &Path) -> Settings {
    std::fs::read_to_string(data.join("extras.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(data: &Path, s: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(data.join("extras.json"), text).map_err(|e| format!("Couldn't save the settings ({e})."))?;
    std::fs::write(data.join("extras.cfg"), cfg(s)).map_err(|e| format!("Couldn't write extras.cfg ({e})."))?;
    // the plugin reads Latin-1, like everything the game shows
    crate::sys::write_latin1(&data.join("ads.txt"), &ads_txt(s)).map_err(|e| format!("Couldn't write the chat messages ({e})."))
}

/// Website status's fast downloads: sets the download address to `url`, or with None
/// clears it, but only when it's still `was` (one the host typed in stays).
pub fn set_fastdl(g: &Game, url: Option<&str>, was: Option<&str>) -> Result<(), String> {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&g.data);
    match url {
        Some(u) => s.fastdl = u.to_string(),
        None if was.is_some_and(|w| s.fastdl == w) => s.fastdl = String::new(),
        None => return Ok(()),
    }
    save(&g.data, &s)?;
    reload(g);
    Ok(())
}

// ---- map rotation -------------------------------------------------------------------

/// A map name the plugins and the game take: plain characters, fits the game's 32.
pub fn valid_map(name: &str) -> bool {
    crate::maps::safe_arg(name) && name.len() <= 31 && !name.starts_with('.')
}

/// Mode ids are file names too (rotation_<id>.txt).
fn valid_mode(id: &str) -> bool {
    !id.is_empty() && id.len() <= 31 && id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

/// The maps in a map cycle file: one per line, ; and // comments, a .bsp ending is fine.
pub fn parse_cycle(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with("//"))
        .filter_map(|l| l.split_whitespace().next())
        .map(|m| m.trim_matches('"'))
        .map(|m| if m.to_lowercase().ends_with(".bsp") { &m[..m.len() - 4] } else { m })
        .map(String::from)
        .collect()
}

/// The map cycle file the game uses: mapcyclefile from server.cfg / listenserver.cfg,
/// else mapcycle.txt. Only a plain .txt path inside the game folder counts.
pub fn cycle_name(game_dir: &Path) -> String {
    let mut name = "mapcycle.txt".to_string();
    for cfg in ["listenserver.cfg", "server.cfg"] {
        let text = crate::sys::read_latin1(&game_dir.join(cfg)).unwrap_or_default();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            if words.next().is_some_and(|w| w.eq_ignore_ascii_case("mapcyclefile")) {
                let v = words.next().unwrap_or("").trim_matches('"');
                let ok = v.len() <= 64
                    && v.to_lowercase().ends_with(".txt")
                    && !v.contains("..")
                    && !v.starts_with('/')
                    && v.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-/".contains(&c));
                if ok {
                    name = v.to_string();
                }
            }
        }
    }
    name
}

fn rotation_path(g: &Game, mode: &str) -> PathBuf {
    if mode.is_empty() {
        g.dir.join(cycle_name(&g.dir))
    } else {
        g.data.join(format!("rotation_{mode}.txt"))
    }
}

/// Checks a new rotation against the installed maps; returns it with each map's
/// installed spelling.
pub fn check_rotation(maps: &[String], installed: &BTreeMap<String, String>, main: bool) -> Result<Vec<String>, String> {
    if maps.len() > MAX_ROTATION {
        return Err(format!("Keep the rotation to {MAX_ROTATION} maps."));
    }
    if main && maps.is_empty() {
        return Err("Keep at least one map in the rotation.".into());
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in maps {
        let m = m.trim();
        if !valid_map(m) {
            return Err(format!("\"{}\" isn't a map name.", one_line(m, 40)));
        }
        let Some(name) = installed.get(&m.to_lowercase()) else {
            return Err(format!("{m} isn't installed."));
        };
        if !seen.insert(m.to_lowercase()) {
            return Err(format!("{m} is in the list twice."));
        }
        out.push(name.clone());
    }
    Ok(out)
}

/// Writes a rotation. The main one keeps the line endings it had, and the game's
/// original list is kept once in the data folder. An empty mode list is removed (the
/// mode then uses the main rotation).
pub fn write_rotation(path: &Path, data: &Path, maps: &[String], main: bool) -> Result<(), String> {
    if !main && maps.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    let old = std::fs::read(path).ok();
    if main {
        let keep = data.join("mapcycle-original.txt");
        if let (Some(old), false) = (&old, keep.exists()) {
            let _ = std::fs::create_dir_all(data);
            let _ = std::fs::write(&keep, old);
        }
    }
    let nl = if old.as_ref().is_some_and(|o| o.windows(2).any(|w| w == b"\r\n")) { "\r\n" } else { "\n" };
    let text: String = maps.iter().map(|m| format!("{m}{nl}")).collect();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| format!("Couldn't write {} ({e}).", path.file_name().unwrap_or_default().to_string_lossy()))
}

/// Game Mode Menu modes from modes.ini: (id, name).
pub fn parse_modes(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with(';') && !l.starts_with("//"))
        .filter_map(|l| {
            let (id, rest) = l.split_once(char::is_whitespace)?;
            let name = rest.trim().strip_prefix('"')?.split('"').next()?;
            valid_mode(id).then(|| (id.to_string(), name.to_string()))
        })
        .collect()
}

fn modes(g: &Game) -> Vec<(String, String)> {
    parse_modes(&crate::sys::read_latin1(&g.dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default())
}

/// Countries in join messages need AMX Mod X's geoip module and a MaxMind country
/// database (AMX Mod X 1.10's package ships GeoLite2-Country.mmdb). The module loads by
/// itself when a plugin uses it; gm_extras runs without it.
fn geoip_ready(g: &Game) -> bool {
    let am = g.dir.join("addons/amxmodx");
    let module = ["geoip_amxx.dll", "geoip_amxx_i386.so"].iter().any(|m| am.join("modules").join(m).is_file());
    let db = ["GeoLite2-Country.mmdb", "GeoIP2-Country.mmdb", "GeoLite2-City.mmdb", "GeoIP2-City.mmdb"].iter().any(|d| am.join("data").join(d).is_file());
    module && db
}

/// The daily restart of a dedicated server (none for the player's own game).
fn schedule(g: &Game) -> Value {
    if !g.dedicated {
        return Value::Null;
    }
    match crate::servers::restart_of(&g.hl, &g.name) {
        // needs_app: on another machine the app does the restart, so only while it's open
        Some(r) => json!({"on": r.on, "time": r.time(), "needs_app": g.remote.is_some()}),
        None => Value::Null,
    }
}

fn installed_maps(g: &Game) -> BTreeMap<String, String> {
    g.maps(false).iter().filter_map(|m| m["name"].as_str()).map(|n| (n.to_lowercase(), n.to_string())).collect()
}

fn listing(g: &Game) -> Value {
    let read = |p: PathBuf| parse_cycle(&crate::sys::read_latin1(&p).unwrap_or_default());
    let modes = modes(g);
    let mut lists = serde_json::Map::new();
    for (id, _) in &modes {
        let p = rotation_path(g, id);
        if p.exists() {
            lists.insert(id.clone(), json!(read(p)));
        }
    }
    let current = current_mode(g);
    json!({
        "settings": load(&g.data),
        "rotation": {"file": cycle_name(&g.dir), "main": read(rotation_path(g, "")), "modes": lists},
        "modes": modes.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "current_mode": current,
        "plugin": g.dir.join("addons/amxmodx/plugins/gm_extras.amxx").exists(),
        "dedicated": g.dedicated,
        "geoip": geoip_ready(g),
        "schedule": schedule(g),
    })
}

/// The mode being played now. On another machine the plugin switches modes there, so
/// this computer's current.txt can be behind: the running server says, else the
/// machine's own file (this computer's copy only if the machine can't be reached).
pub fn current_mode(g: &Game) -> String {
    let local = || crate::sys::read_latin1(&g.data.join("current.txt")).unwrap_or_default().trim().to_string();
    if g.remote.is_none() {
        return local();
    }
    let st = g.state();
    if st["online"] == json!(true) {
        if let Some(m) = st["state"]["mode"].as_str().filter(|m| !m.is_empty()) {
            return m.to_string();
        }
    }
    match crate::remote::game_file(g, "addons/amxmodx/data/gamemodes/current.txt") {
        Ok(Some(b)) => crate::rcon::latin1(&b).trim().to_string(),
        Ok(None) => String::new(),
        Err(_) => local(),
    }
}

/// Tells the running game to re-read everything (a fixed command; offline is fine).
fn reload(g: &Game) {
    let _ = g.rcon.send(RELOAD);
}

// ---- fast downloads: the files a web server needs -----------------------------------

/// Wads every Counter-Strike / Condition Zero install has.
pub(crate) const STOCK_WADS: &[&str] = &[
    "cached.wad", "cs_747.wad", "cs_assault.wad", "cs_bdog.wad", "cs_cbble.wad", "cs_dust.wad", "cs_havana.wad", "cs_office.wad",
    "cstraining.wad", "cstrike.wad", "czero.wad", "de_airstrip.wad", "de_aztec.wad", "de_piranesi.wad", "de_storm.wad",
    "de_vertigo.wad", "decals.wad", "fonts.wad", "gfx.wad", "halflife.wad", "itsitaly.wad", "liquids.wad", "n0th1ng.wad",
    "pldecal.wad", "spraypaint.wad", "tempdecal.wad", "torntextures.wad", "xeno.wad",
];
/// What players can download from a server.
const DL_EXT: &[&str] = &["bsp", "res", "wad", "wav", "mp3", "mdl", "spr", "tga", "bmp", "txt"];
const DL_DIRS: &[&str] = &["maps/", "sound/", "models/", "sprites/", "gfx/", "overviews/"];

fn downloadable(rel: &str) -> bool {
    let r = rel.replace('\\', "/").to_lowercase();
    let ext = r.rsplit('.').next().unwrap_or("");
    DL_EXT.contains(&ext) && (DL_DIRS.iter().any(|d| r.starts_with(d)) || (ext == "wad" && !r.contains('/'))) && !crate::maps::is_stock_file(&r)
}

/// worldspawn's "wad" (file names) and "skyname" from a map's entity lump.
pub fn worldspawn(path: &Path) -> (Vec<String>, Option<String>) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return (vec![], None) };
    let mut header = [0u8; 12];
    if f.read_exact(&mut header).is_err() {
        return (vec![], None);
    }
    let offset = i32::from_le_bytes(header[4..8].try_into().unwrap());
    let length = i32::from_le_bytes(header[8..12].try_into().unwrap());
    let mut head = vec![0u8; length.clamp(0, 65536) as usize];
    let n = if offset >= 0 && f.seek(SeekFrom::Start(offset as u64)).is_ok() { f.read(&mut head).unwrap_or(0) } else { 0 };
    let text: String = head[..n].iter().map(|&b| b as char).collect();
    let first = text.split('}').next().unwrap_or("");
    let key = |k: &str| -> Option<String> {
        let at = first.find(&format!("\"{k}\""))? + k.len() + 2;
        let rest = &first[at..];
        let start = rest.find('"')? + 1;
        let end = rest[start..].find('"')? + start;
        Some(rest[start..end].to_string())
    };
    let wads = key("wad")
        .unwrap_or_default()
        .split(';')
        .filter_map(|w| w.replace('\\', "/").rsplit('/').next().map(|s| s.trim().to_string()))
        .filter(|w| w.to_lowercase().ends_with(".wad"))
        .collect();
    let sky = key("skyname").map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)));
    (wads, sky)
}

/// The files a .res lists (one relative path per line, // comments).
pub fn parse_res(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split("//").next().unwrap_or("").trim().trim_matches('"').replace('\\', "/"))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Everything a custom map needs from `roots` (the game folder first, then cstrike).
pub fn map_files(roots: &[PathBuf], map: &str) -> BTreeSet<String> {
    let find = |rel: &str| roots.iter().map(|r| r.join(rel)).find(|p| p.is_file());
    let mut out = BTreeSet::new();
    let mut add = |rel: String| {
        if find(&rel).is_some() {
            out.insert(rel);
        }
    };
    let bsp = format!("maps/{map}.bsp");
    let Some(bsp_path) = find(&bsp) else { return BTreeSet::new() };
    add(bsp);
    add(format!("maps/{map}.txt"));
    for ext in ["bmp", "tga", "txt"] {
        add(format!("overviews/{map}.{ext}"));
    }
    let res = format!("maps/{map}.res");
    if let Some(p) = find(&res) {
        add(res);
        for rel in parse_res(&crate::sys::read_latin1(&p).unwrap_or_default()) {
            if crate::setup::record::inside(Path::new("/x"), &rel).is_some() && downloadable(&rel) {
                add(rel);
            }
        }
    }
    let (wads, sky) = worldspawn(&bsp_path);
    for w in wads.into_iter().filter(|w| !STOCK_WADS.contains(&w.to_lowercase().as_str())) {
        add(w);
    }
    if let Some(sky) = sky {
        for side in ["up", "dn", "lf", "rt", "ft", "bk"] {
            add(format!("gfx/env/{sky}{side}.tga"));
        }
    }
    out
}

/// The custom files of a game or server, relative to its game folder: non-stock maps
/// and what they need, plus the sounds, models and sprites Loner-Strike installed
/// (game modes and add-ons precache them, so players download them too).
fn fastdl_files(g: &Game) -> BTreeSet<String> {
    let mut roots = vec![g.dir.clone()];
    let cs = g.hl.join("cstrike");
    if cs != g.dir {
        roots.push(cs);
    }
    let mut out = BTreeSet::new();
    for m in g.maps(false) {
        if m["source"] != "stock" {
            if let Some(name) = m["name"].as_str().filter(|n| valid_map(n)) {
                out.extend(map_files(&roots, name));
            }
        }
    }
    let exists = |rel: &str| roots.iter().any(|r| crate::setup::record::inside(r, rel).is_some_and(|p| p.is_file()));
    let rec = crate::setup::record::Record::open(&g.dir);
    for rel in rec.data.files.keys() {
        if downloadable(rel) && exists(rel) {
            out.insert(rel.replace('\\', "/"));
        }
    }
    for (_, r) in g.store.records() {
        for f in r["files"].as_array().into_iter().flatten().chain(r["shared"].as_array().into_iter().flatten()) {
            if let Some(rel) = f.as_str().filter(|r| downloadable(r) && exists(r)) {
                out.insert(rel.replace('\\', "/"));
            }
        }
    }
    out
}

/// Copies the files into `dest`, mirroring the game folder. (files, bytes)
fn export_to(g: &Game, dest: &Path) -> Result<(usize, u64), String> {
    let mut roots = vec![g.dir.clone()];
    roots.push(g.hl.join("cstrike"));
    let (mut files, mut bytes) = (0, 0);
    for rel in fastdl_files(g) {
        let Some(src) = roots.iter().filter_map(|r| crate::setup::record::inside(r, &rel)).find(|p| p.is_file()) else { continue };
        let Some(dst) = crate::setup::record::inside(dest, &rel) else { continue };
        let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        // already there from an earlier export
        if std::fs::metadata(&dst).map(|m| m.len()).ok() != Some(size) {
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make a folder there ({e})."))?;
            }
            std::fs::copy(&src, &dst).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
        }
        files += 1;
        bytes += size;
    }
    Ok((files, bytes))
}

// ---- API ------------------------------------------------------------------------------

type Reply = (StatusCode, Json<Value>);

fn bad(msg: impl Into<String>) -> Reply {
    (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": msg.into()})))
}

fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn answer(r: Option<Result<Value, String>>) -> Reply {
    match r {
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"ok": false, "error": "No game found."}))),
        Some(Ok(v)) => (StatusCode::OK, Json(v)),
        Some(Err(e)) => bad(e),
    }
}

/// Runs f with the request's game on a blocking thread (file and RCON work).
async fn with_game<T: Send + 'static>(st: &Shared, f: impl FnOnce(&Game) -> T + Send + 'static) -> Option<T> {
    let g = st.game()?;
    tokio::task::spawn_blocking(move || f(&g)).await.ok()
}

async fn api_get(State(st): State<Shared>, helper: Option<axum::Extension<crate::control::Helper>>) -> Reply {
    let helper = helper.is_some();
    answer(with_game(&st, move |g| Ok(for_helper(listing(g), helper))).await)
}

/// The listing as someone helping run the server sees it: no FastDL address.
fn for_helper(mut v: Value, helper: bool) -> Value {
    if helper {
        v["settings"]["fastdl"] = json!("");
    }
    v
}

async fn api_settings(State(st): State<Shared>, headers: HeaderMap, helper: Option<axum::Extension<crate::control::Helper>>, Json(req): Json<Settings>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let mut s = match tidy(req) {
        Ok(s) => s,
        Err(e) => return bad(e),
    };
    // where players download maps from (sv_downloadurl) stays the owner's: a helper's save
    // keeps it as it is
    let keep_fastdl = helper.is_some();
    answer(
        with_game(&st, move |g| {
            if keep_fastdl {
                s.fastdl = load(&g.data).fastdl;
            }
            if !s.empty.map.is_empty() {
                s.empty.map = installed_maps(g).get(&s.empty.map.to_lowercase()).cloned().ok_or(format!("{} isn't installed.", s.empty.map))?;
            }
            if !s.empty.mode.is_empty() && !modes(g).iter().any(|(id, _)| *id == s.empty.mode) {
                return Err("That game mode isn't installed.".to_string());
            }
            let _l = LOCK.lock().unwrap();
            save(&g.data, &s)?;
            reload(g);
            Ok(for_helper(listing(g), keep_fastdl))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct RotationReq {
    /// "" for the main rotation, else a mode id
    #[serde(default)]
    mode: String,
    maps: Vec<String>,
}

async fn api_rotation(State(st): State<Shared>, headers: HeaderMap, helper: Option<axum::Extension<crate::control::Helper>>, Json(req): Json<RotationReq>) -> Reply {
    let helper = helper.is_some();
    if !from_panel(&headers) {
        return forbidden();
    }
    let mode = req.mode.trim().to_string();
    if !mode.is_empty() && !valid_mode(&mode) {
        return bad("Unknown game mode.");
    }
    answer(
        with_game(&st, move |g| {
            if !mode.is_empty() && !modes(g).iter().any(|(id, _)| *id == mode) {
                return Err("That game mode isn't installed.".to_string());
            }
            let maps = check_rotation(&req.maps, &installed_maps(g), mode.is_empty())?;
            let _l = LOCK.lock().unwrap();
            write_rotation(&rotation_path(g, &mode), &g.data, &maps, mode.is_empty())?;
            reload(g);
            Ok(for_helper(listing(g), helper))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct ScheduleReq {
    on: bool,
    /// "HH:MM", local time
    time: String,
}

/// A dedicated server's daily restart.
async fn api_schedule(State(st): State<Shared>, headers: HeaderMap, helper: Option<axum::Extension<crate::control::Helper>>, Json(req): Json<ScheduleReq>) -> Reply {
    let helper = helper.is_some();
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some((hour, minute)) = crate::servers::parse_time(&req.time) else { return bad("Pick a time like 05:00.") };
    answer(
        with_game(&st, move |g| {
            if !g.dedicated {
                return Err("Only dedicated servers restart on a schedule.".to_string());
            }
            crate::servers::set_restart(&g.hl, &g.name, crate::servers::DailyRestart { on: req.on, hour, minute })?;
            Ok(for_helper(listing(g), helper))
        })
        .await,
    )
}

/// The app window only: copies the custom content into a folder the user picks.
async fn api_export(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    let (Some(g), Some(app)) = (st.game(), st.app.get().cloned()) else { return bad("No game found.") };
    let r = tokio::task::spawn_blocking(move || {
        let Some(picked) = app.dialog().file().set_title("Pick a folder for your web server's files").blocking_pick_folder() else {
            return Ok(json!({"cancelled": true}));
        };
        let dest = picked.into_path().map_err(|e| e.to_string())?;
        if dest.starts_with(&g.hl) {
            return Err("Pick a folder outside the game's folder.".to_string());
        }
        let (files, bytes) = export_to(&g, &dest)?;
        Ok(json!({"ok": true, "files": files, "mb": (bytes as f64 / 1_048_576.0 * 10.0).round() / 10.0, "dir": dest.to_string_lossy()}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    answer(Some(r))
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/extras", get(api_get))
        .route("/api/extras/settings", post(api_settings))
        .route("/api/extras/rotation", post(api_rotation))
        .route("/api/extras/schedule", post(api_schedule))
}

/// Only for the app window (a folder picker on this computer).
pub fn local_routes() -> Router<Shared> {
    Router::new().route("/api/extras/export", post(api_export))
}

// ---- shared between servers (lists.rs) ----------------------------------------------

pub(crate) fn shared_ads(g: &Game) -> String {
    serde_json::to_string(&load(&g.data).ads).unwrap_or_default()
}

/// Gives `to` the same timed chat messages as `from` (its other settings stay).
pub(crate) fn copy_ads(from: &Game, to: &Game) -> Result<(), String> {
    let _l = LOCK.lock().unwrap();
    let mut s = load(&to.data);
    s.ads = load(&from.data).ads;
    save(&to.data, &s)?;
    reload(to);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-extras-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn extras_settings_are_tidied() {
        let mut s = Settings::default();
        s.ads.minutes = 0;
        s.ads.prefix = "  [My\nServer] with a very long prefix indeed ".into();
        s.ads.messages = vec![
            Ad { text: "Join our Discord\r\n: discord.gg/x".into(), color: "green".into() },
            Ad { text: "   ".into(), color: "red".into() },
            Ad { text: "x".repeat(400), color: "purple".into() },
        ];
        s.afk.spec = 5;
        s.afk.kick = 10;
        s.ping.max = 5000;
        s.ping.checks = 0;
        let t = tidy(s).unwrap();
        assert_eq!(t.ads.minutes, 1);
        assert!(t.ads.prefix.chars().count() <= MAX_PREFIX && t.ads.prefix.starts_with("[My Server]"));
        assert!(!t.ads.prefix.contains('\n'));
        assert_eq!(t.ads.messages.len(), 2, "empty messages are dropped");
        assert!(!t.ads.messages[0].text.contains('\n'));
        assert_eq!(t.ads.messages[1].text.len(), MAX_AD);
        assert_eq!(t.ads.messages[1].color, "default");
        assert_eq!((t.afk.spec, t.afk.kick), (30, 30));
        assert_eq!((t.ping.max, t.ping.checks), (1000, 2));
        let mut s = Settings::default();
        s.afk.kick = 0;
        assert_eq!(tidy(s).unwrap().afk.kick, 0, "0 means never kick");
        assert!(tidy(Settings { ads: Ads { messages: vec![Ad { text: "a".into(), color: "default".into() }; 41], ..Ads::default() }, ..Settings::default() }).is_err());
    }

    #[test]
    fn extras_fastdl_urls() {
        assert!(valid_url("https://dl.example.com/czero"));
        assert!(valid_url("http://1.2.3.4:8080/cstrike/"));
        for bad in ["ftp://x.com", "https://", "https:///x", "https://a b.com", "https://x.com/\"; quit", "https://x.com/;rcon", "javascript:alert(1)", "https://x.com\\y"] {
            assert!(!valid_url(bad), "{bad}");
        }
        assert!(!valid_url(&format!("https://x.com/{}", "a".repeat(120))));
        let s = Settings { fastdl: " https://dl.example.com/czero ".into(), ..Settings::default() };
        let t = tidy(s).unwrap();
        assert_eq!(t.fastdl, "https://dl.example.com/czero");
        assert!(cfg(&t).contains("gm_fastdl \"https://dl.example.com/czero\"\r\n"));
        assert!(tidy(Settings { fastdl: "dl.example.com".into(), ..Settings::default() }).is_err());
        assert!(cfg(&Settings::default()).contains("gm_fastdl \"\""), "empty clears what the app set before");
    }

    #[test]
    fn extras_cfg_and_ads_files() {
        let mut s = Settings::default();
        s.rotation_random = true;
        s.ads.on = true;
        s.ads.minutes = 4;
        s.ads.messages = vec![
            Ad { text: "Join our Discord: discord.gg/abc".into(), color: "green".into() },
            Ad { text: "Next map: {nextmap}, {timeleft} left. 100% fun".into(), color: "grey".into() },
        ];
        s.afk = Afk { on: true, spec: 90, kick: 0 };
        let c = cfg(&s);
        for line in ["gm_rotation_random 1", "gm_rtv_cycle 0", "gm_ads_interval 4", "gm_afk 1", "gm_afk_spec 90", "gm_afk_kick 0", "gm_ping 0", "gm_ping_max 250", "gm_ping_checks 5"] {
            assert!(c.contains(&format!("{line}\r\n")), "{line}");
        }
        let a = ads_txt(&s);
        assert!(a.contains("\r\nprefix [Server]\r\n"));
        assert!(a.contains("\r\ng Join our Discord: discord.gg/abc\r\n"));
        assert!(a.contains("\r\ns Next map: {nextmap}, {timeleft} left. 100% fun\r\n"));
        // switched off, or nothing to show: interval 0
        s.ads.on = false;
        assert!(cfg(&s).contains("gm_ads_interval 0\r\n"));
        s.ads.on = true;
        s.ads.messages.clear();
        assert!(cfg(&s).contains("gm_ads_interval 0\r\n"));

        let dir = tmp("save");
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir), s);
        assert!(dir.join("extras.cfg").exists() && dir.join("ads.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extras_map_cycle_files() {
        assert_eq!(parse_cycle("// comment\r\nde_dust2\r\n\r\n; note\r\ncs_office.bsp\r\nde_aztec \"\" \r\n"), vec!["de_dust2", "cs_office", "de_aztec"]);
        let dir = tmp("cycle");
        assert_eq!(cycle_name(&dir), "mapcycle.txt");
        std::fs::write(dir.join("server.cfg"), "hostname \"x\"\nmapcyclefile \"cycles/pub.txt\"\n").unwrap();
        assert_eq!(cycle_name(&dir), "cycles/pub.txt");
        for bad in ["../outside.txt", "/etc/passwd.txt", "cycle.cfg", "x;quit.txt"] {
            std::fs::write(dir.join("server.cfg"), format!("mapcyclefile {bad}\n")).unwrap();
            assert_eq!(cycle_name(&dir), "mapcycle.txt", "{bad}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extras_rotation_checks_and_writes() {
        let installed: BTreeMap<String, String> = ["de_dust2", "cs_office", "surf_ski_2"].iter().map(|m| (m.to_lowercase(), m.to_string())).collect();
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(check_rotation(&v(&["DE_DUST2", "cs_office"]), &installed, true).unwrap(), v(&["de_dust2", "cs_office"]));
        assert!(check_rotation(&v(&["de_dust2", "de_dust2"]), &installed, true).is_err(), "twice");
        assert!(check_rotation(&v(&["de_nuke"]), &installed, true).is_err(), "not installed");
        assert!(check_rotation(&v(&["de_dust2; quit"]), &installed, true).is_err(), "not a name");
        assert!(check_rotation(&v(&["../x"]), &installed, true).is_err());
        assert!(check_rotation(&[], &installed, true).is_err(), "the main rotation can't be empty");
        assert!(check_rotation(&[], &installed, false).is_ok(), "a mode's can: it then uses the main one");

        let dir = tmp("rotation");
        let data = dir.join("data");
        let cycle = dir.join("mapcycle.txt");
        std::fs::write(&cycle, "de_aztec\r\ncs_italy\r\n").unwrap();
        write_rotation(&cycle, &data, &v(&["de_dust2", "cs_office"]), true).unwrap();
        assert_eq!(std::fs::read_to_string(&cycle).unwrap(), "de_dust2\r\ncs_office\r\n", "keeps CRLF");
        assert_eq!(std::fs::read_to_string(data.join("mapcycle-original.txt")).unwrap(), "de_aztec\r\ncs_italy\r\n");
        write_rotation(&cycle, &data, &v(&["cs_office"]), true).unwrap();
        assert_eq!(std::fs::read_to_string(data.join("mapcycle-original.txt")).unwrap(), "de_aztec\r\ncs_italy\r\n", "the original is kept once");
        let mode = data.join("rotation_surf.txt");
        write_rotation(&mode, &data, &v(&["surf_ski_2"]), false).unwrap();
        assert_eq!(std::fs::read_to_string(&mode).unwrap(), "surf_ski_2\n");
        write_rotation(&mode, &data, &[], false).unwrap();
        assert!(!mode.exists(), "an empty mode list is removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extras_joins_votes_and_empty() {
        let mut s = Settings::default();
        assert!(s.joins.skip_bots && s.joins.country && !s.votes.kick_bots, "bots left out by default");
        s.votes = Votes { kick: true, ban: true, percent: 5, min_players: 99, ban_minutes: 0, cooldown: 500, kick_bots: false };
        s.empty = Empty { on: true, minutes: 0, map: " de_dust2 ".into(), mode: "classic".into() };
        let t = tidy(s.clone()).unwrap();
        assert_eq!((t.votes.percent, t.votes.min_players, t.votes.ban_minutes, t.votes.cooldown), (30, 32, 1, 60));
        assert_eq!((t.empty.minutes, t.empty.map.as_str()), (1, "de_dust2"));
        let c = cfg(&t);
        for line in ["gm_votekick 1", "gm_voteban 1", "gm_vote_percent 30", "gm_vote_bots 0", "gm_joins 0", "gm_joins_skipbots 1", "gm_empty 1", "gm_empty_map \"de_dust2\"", "gm_empty_mode \"classic\""] {
            assert!(c.contains(&format!("{line}\r\n")), "{line}");
        }
        // nothing that could end the cfg line
        for (map, mode) in [("de_dust2\"; quit", ""), ("de_dust2", "Bad Mode"), ("", "")] {
            let mut e = s.clone();
            e.empty.map = map.into();
            e.empty.mode = mode.into();
            assert!(tidy(e).is_err(), "{map} {mode}");
        }
        // switched off: no map needed, and the plugin gets 0
        let mut off = s.clone();
        off.empty = Empty { on: false, ..Empty::default() };
        let t = tidy(off).unwrap();
        assert!(cfg(&t).contains("gm_empty 0\r\ngm_empty_map \"\"\r\ngm_empty_mode \"\"\r\n"));
        // settings saved before these existed still load
        let old: Settings = serde_json::from_str(r#"{"rotation_random": true, "fastdl": ""}"#).unwrap();
        assert_eq!((old.joins, old.votes, old.empty), (Joins::default(), Votes::default(), Empty::default()));
    }

    #[test]
    fn extras_modes_ini() {
        let m = parse_modes("; comment\nclassic       \"Classic\"   \"Bomb defusal\"  \"\"  \"w\"\n\nzombie \"Zombie Plague\" \"x\"\nBad-Id \"No\"\n");
        assert_eq!(m, vec![("classic".into(), "Classic".into()), ("zombie".into(), "Zombie Plague".into())]);
    }

    #[test]
    fn extras_map_resources() {
        let dir = tmp("res");
        let cz = dir.join("czero");
        let cs = dir.join("cstrike");
        for d in [&cz, &cs] {
            std::fs::create_dir_all(d.join("maps")).unwrap();
        }
        // a tiny BSP: version, then the entity lump's offset and length
        let ents = b"{\n\"classname\" \"worldspawn\"\n\"wad\" \"\\half-life\\cstrike\\cstrike.wad;\\half-life\\cstrike\\custom_tex.wad\"\n\"skyname\" \"night\"\n}\n{\n\"classname\" \"info_player_start\"\n}\n";
        let mut bsp = vec![30, 0, 0, 0];
        bsp.extend_from_slice(&12i32.to_le_bytes());
        bsp.extend_from_slice(&(ents.len() as i32).to_le_bytes());
        bsp.extend_from_slice(ents);
        std::fs::write(cs.join("maps/aim_test.bsp"), &bsp).unwrap();
        std::fs::write(cs.join("maps/aim_test.res"), "// resources\r\nsound/aim/beep.wav\r\nmodels\\aim\\crate.mdl // a crate\r\n../../secret.txt\r\naddons/amxmodx/plugins/x.amxx\r\n").unwrap();
        for f in ["sound/aim/beep.wav", "models/aim/crate.mdl", "custom_tex.wad", "cstrike.wad", "gfx/env/nightup.tga"] {
            let p = cs.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        std::fs::create_dir_all(dir.join("addons/amxmodx/plugins")).unwrap();
        let (wads, sky) = worldspawn(&cs.join("maps/aim_test.bsp"));
        assert_eq!(wads, vec!["cstrike.wad", "custom_tex.wad"]);
        assert_eq!(sky.as_deref(), Some("night"));
        let files: Vec<String> = map_files(&[cz.clone(), cs.clone()], "aim_test").into_iter().collect();
        assert_eq!(files, vec!["custom_tex.wad", "gfx/env/nightup.tga", "maps/aim_test.bsp", "maps/aim_test.res", "models/aim/crate.mdl", "sound/aim/beep.wav"]);
        assert!(map_files(&[cz, cs], "missing").is_empty());
        assert!(downloadable("sound/zombie/scream.wav") && downloadable("custom.wad"));
        assert!(!downloadable("maps/de_dust2.bsp"), "stock maps stay out");
        assert!(!downloadable("addons/amxmodx/plugins/x.amxx") && !downloadable("dlls/mp.dll") && !downloadable("server.cfg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
