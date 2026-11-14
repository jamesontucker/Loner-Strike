//! Community creations (Tools, Beta): find, try and publish maps, game modes, prefabs, sprays
//! and textures made with Loner-Strike. The full design is in docs/COMMUNITY.md.
//!
//! Files live on GameBanana (the app already browses it and installs from it safely). A
//! creation is a Loner-Strike one when its GameBanana page carries the tag
//! `Software Used: LonerStrike`; GameBanana's search finds those (`_csvFields=attribs`).
//! Loner-Strike's own index (`dl.lonerstrike.com/community/index.json`, a static file) adds
//! what GameBanana can't say: the modes a map supports, whether it ships a nav and map
//! knowledge, health check results, a game mode's plugins and how far they're trusted, and
//! which pages were pulled. When the index can't be reached, the last copy that could be read
//! is used; with none, GameBanana's tag search alone (no badges).
//!
//! - `GET  /api/tools/community/browse?kind=&q=&sort=&refresh=1`  creations, newest index first
//! - `GET  /api/tools/community/item/:model/:id`                  one creation (`model`: mod|spray)
//! - `POST /api/tools/community/try {model, id, file?}`           installs it with the app's own
//!   installer (gamebanana.rs: only map content, nothing that runs code); the page follows the
//!   job on `/api/gb/jobs`, then starts the game with `/api/app/launch`
//! - `GET  /api/tools/community/publish/:map`                     is the map ready to share?
//! - `POST /api/tools/community/publish/:map {title, summary, author, modes, screenshots}`
//!   builds the upload package (zip) and the text for GameBanana's submit page
//! - `POST /api/tools/community/register {url}`                   the GameBanana page made from
//!   the package: checked (tag, game, the file's MD5) and sent to the index's inbox, or kept
//!   here until it can be
//! - `GET  /api/tools/community/mine`                             what this computer published
//! - `POST /api/tools/community/reveal {file}`                    shows a package in Finder/Explorer
//!
//! Uploading is done on GameBanana's site: it has no public upload API, and the app never signs
//! in to it.

use super::mapinfo::MapFile;
use super::{blocking, find_map, json_answer, locate, Fail};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const INDEX_URL: &str = "https://dl.lonerstrike.com/community/index.json";
/// Where registrations go (a small service next to the server list; see docs/COMMUNITY.md).
/// Until it answers, registrations wait on this computer.
pub const SUBMIT_URL: &str = "https://list.lonerstrike.com/community/submit";
/// Where the Report button's reports go (the team's private moderation channel hears about them).
pub const REPORT_URL: &str = "https://list.lonerstrike.com/community/report";
/// What Try it says for a creation the author unlisted.
pub const UNLISTED: &str = "The author unlisted this creation, so it isn't installed from Community any more.";

/// Why Try it won't install an entry with this status (unlisted, withdrawn), if it won't.
pub fn try_refusal(s: Status) -> Option<&'static str> {
    match s {
        Status::Unlisted => Some(UNLISTED),
        Status::Withdrawn => Some(WITHDRAWN),
        _ => None,
    }
}

/// What Get says for a creation whose page or file is gone.
pub const WITHDRAWN: &str = "This creation was withdrawn from GameBanana (its page or file is gone), so it can't be downloaded any more.";

/// Unlisting and listing again (the author, with the page's proof: docs/COMMUNITY.md "Unlisting").
pub const UNLIST_URL: &str = "https://list.lonerstrike.com/community/unlist";
/// What a report can say is wrong (the list service takes exactly these).
pub const REPORT_REASONS: [&str; 6] = ["broken", "malware", "stolen", "offensive", "not-loner-strike", "other"];
const MAX_NOTE: usize = 500;
/// For 1.0 the community shares content and Game Mode Studio modes only (settings and graphs;
/// the app builds a Studio mode's code on each player's computer), never plugins.
pub const PLUGIN_REFUSAL: &str = "Plugin modes can't be shared yet. Make it in Game Mode Studio, which builds a mode's code on each player's computer.";
/// Files that run code (or hide other files) never go in a shared package. The list service
/// (services/serverlist/community.py, CODE_EXT / ARCHIVE_EXT) refuses the same list: keep them identical.
const CODE_EXT: [&str; 26] = [
    ".amxx", ".amx", ".sma", ".inc", ".dll", ".so", ".dylib", ".exe", ".com", ".scr", ".msi", ".bat", ".cmd", ".sh", ".ps1", ".py", ".pyc", ".js", ".vbs", ".jar", ".lua", ".pl", ".rb",
    ".php", ".app", ".command",
];
const ARCHIVE_EXT: [&str; 9] = [".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2", ".xz", ".cab"];
/// The tag that marks a GameBanana page as a Loner-Strike creation (GameBanana's own
/// "Software Used" attribute group, so it reads naturally on the page too). One word: the
/// search splits "Loner-Strike" into "Loner" OR "Strike" and finds every page with "Strike".
pub const TAG_TITLE: &str = "Software Used";
pub const TAG_VALUE: &str = "LonerStrike";
/// Also accepted when reading a page's tags (people will type the name as it's written).
const TAG_ALSO: &str = "Loner-Strike";
/// The index format this app reads; a bigger major number is a format it doesn't know.
pub const SCHEMA: u32 = 1;
const GB_API: &str = "https://gamebanana.com/apiv11";
const CS16_GAME: i64 = 4254;
const USER_AGENT: &str = "LonerStrike/1.0 (community browser)";
const MAX_INDEX: u64 = 8 << 20;
const MAX_ITEMS: usize = 5000;
const INDEX_FRESH: Duration = Duration::from_secs(15 * 60);
const SEARCH_FRESH: Duration = Duration::from_secs(10 * 60);
/// GameBanana's search answers 15 a page; this many pages are read (newest first)
const SEARCH_PAGES: i64 = 6;
const MAX_SHOTS: usize = 6;
const MAX_SHOT: usize = 8 << 20;
const MAX_PACKAGE: u64 = 1 << 30;
/// [Caps] The finished zip: the community list checks every file in packages up to 256 MB
/// (services/serverlist/community.py CHECK_MAX), so a bigger one could never be listed.
/// GameBanana itself takes about 1.4 GB in Mods (200 MB for WIPs).
pub const MAX_PACKAGE_ZIP: u64 = 256 << 20;
/// Hosts screenshots in the index may come from
const SHOT_HOSTS: [&str; 2] = ["images.gamebanana.com", "dl.lonerstrike.com"];
const NO_GAME: &str = "Set up the game on this computer first.";

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn dir() -> PathBuf {
    crate::sys::app_dir().join("tools").join("community")
}

// ---- the index -------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Map,
    Mode,
    Prefab,
    Spray,
    Texture,
    #[default]
    #[serde(other)]
    Other,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "map" => Kind::Map,
            "mode" => Kind::Mode,
            "prefab" => Kind::Prefab,
            "spray" => Kind::Spray,
            "texture" => Kind::Texture,
            _ => return None,
        })
    }

    /// The app's installer section for this kind (gamebanana.rs / content.rs), if it has one.
    pub fn installer(self) -> Option<&'static str> {
        match self {
            Kind::Map => Some("maps"),
            Kind::Spray => Some("sprays"),
            Kind::Texture => Some("textures"),
            _ => None,
        }
    }

    /// Try it works for this kind (prefabs go to the map editor's library, not an installer section).
    pub fn can_install(self) -> bool {
        // (modes: Game Mode Studio graphs, built on this computer by the Studio's compiler)
        self.installer().is_some() || self == Kind::Prefab || self == Kind::Mode
    }

    /// Why there's no one-click install for this kind yet.
    fn no_installer(self) -> &'static str {
        match self {
            Kind::Mode => "Game modes install with the Game Mode Studio, which is still coming. Open the page to read about it.",
            Kind::Prefab => "Prefabs go into the map editor's prefab library.",
            _ => "The app can't install this kind of file. Open the page to get it.",
        }
    }
}

/// Which GameBanana item type a page is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Model {
    #[default]
    Mod,
    Spray,
}

impl Model {
    pub fn parse(s: &str) -> Option<Model> {
        match s.to_ascii_lowercase().as_str() {
            "mod" | "mods" => Some(Model::Mod),
            "spray" | "sprays" => Some(Model::Spray),
            _ => None,
        }
    }
    fn path(self) -> &'static str {
        match self {
            Model::Mod => "Mod",
            Model::Spray => "Spray",
        }
    }
    fn url(self, id: i64) -> String {
        match self {
            Model::Mod => format!("https://gamebanana.com/mods/{id}"),
            Model::Spray => format!("https://gamebanana.com/sprays/{id}"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MapMeta {
    /// the map's name (its .bsp without the extension)
    pub name: String,
    /// the .bsp's size and sha256: the exact version the nav, knowledge and checks are for
    pub bsp_size: u64,
    pub bsp_sha256: String,
    /// game modes it's made for (mode ids: classic, surf, deathrun...)
    pub modes: Vec<String>,
    /// the same modes with their permanent ids and names, when the map says them (`ls_modes`
    /// "uid:id" pairs, set in the map editor): a mode is matched by its uid, the id is for show
    pub mode_refs: Vec<ModeRef>,
    pub nav: bool,
    /// the nav names places (bots call out "B site")
    pub places: bool,
    /// map knowledge (.lsv) for that nav, finished
    pub lsv: bool,
    pub res: bool,
    pub overview: bool,
}

/// How far a game mode's plugins are trusted.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Review {
    /// a known author, source shipped, not read line by line
    Verified,
    /// a Loner-Strike reviewer read the source and built the plugin from it (hash pinned)
    Reviewed,
    /// nobody has looked at it (also what a newer index's unknown value reads as; serde wants
    /// the catch-all last)
    #[default]
    #[serde(other)]
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginMeta {
    /// the .amxx file's name
    pub file: String,
    pub sha256: String,
    /// where its source is (a path in the package, or an https link); empty: no source
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ModeMeta {
    /// its mode id (letters, digits, _)
    pub id: String,
    /// its permanent id (a UUID; the Studio gives every mode one), "" for older entries
    pub uid: String,
    /// plugins it needs; none: a data-only mode (settings, graphs, components)
    pub plugins: Vec<PluginMeta>,
    pub review: Review,
    /// maps it's made for
    pub maps: Vec<String>,
    /// what it does beyond its own game (studio::graph::capabilities): server_commands,
    /// amx_commands, plugin_calls, kicks_players
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// the maps it plays on and the content it needs (requirements.rs; the Studio's Details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<ModeRequirements>,
}

/// What a mode plays on and needs, as listed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ModeRequirements {
    pub default_map: String,
    pub maps: Vec<String>,
    pub content: Vec<ContentNeed>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentNeed {
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<super::requirements::Link>,
}

impl ModeRequirements {
    /// From a requirements report (requirements::report): its maps and the content that isn't the game's own.
    pub fn from_report(r: &Value) -> ModeRequirements {
        let name = |m: &Value| m["name"].as_str().unwrap_or("").to_string();
        ModeRequirements {
            default_map: if r["defaultMap"].is_null() { String::new() } else { name(&r["defaultMap"]) },
            maps: r["maps"].as_array().into_iter().flatten().map(name).collect(),
            content: r["content"].as_array().into_iter().flatten().map(|c| ContentNeed { path: c["path"].as_str().unwrap_or("").into(), kind: c["kind"].as_str().unwrap_or("").into(), link: serde_json::from_value(c["link"].clone()).ok() }).collect(),
        }
    }

    /// Only plain map names, clean paths of the three kinds, real page links; capped.
    pub fn clean(&mut self) {
        let ok = |m: &String| crate::maps::safe_arg(m) && m.len() <= 31;
        if !ok(&self.default_map) {
            self.default_map.clear();
        }
        self.maps.retain(ok);
        self.maps.truncate(24);
        self.content.retain_mut(|c| {
            let Some(p) = super::requirements::clean_path(&c.path) else { return false };
            c.kind = super::requirements::kind_of(&p).unwrap_or("").into();
            c.path = p;
            if let Some(l) = &mut c.link {
                if !matches!(l.model.as_str(), "Mod" | "Spray") || l.id <= 0 || l.id >= 100_000_000 {
                    c.link = None;
                } else {
                    l.name = one_line(&l.name, 80);
                }
            }
            true
        });
        self.content.truncate(64);
    }
}

/// What a mode can do beyond its own game, in plain words (badges and the question before
/// installing). Unknown codes are dropped when the index is read.
pub const RISKS: &[(&str, &str)] = &[
    ("server_commands", "Runs server commands"),
    ("amx_commands", "Runs AMX Mod X commands"),
    ("plugin_calls", "Calls other plugins"),
    ("kicks_players", "Can kick players"),
];

/// The plain words for these capability codes (known ones only, in RISKS order).
pub fn risk_words<S: AsRef<str>>(codes: &[S]) -> Vec<&'static str> {
    RISKS.iter().filter(|(c, _)| codes.iter().any(|x| x.as_ref() == *c)).map(|(_, w)| *w).collect()
}

/// The known codes among these, in RISKS order.
pub fn known_codes<S: AsRef<str>>(codes: &[S]) -> Vec<String> {
    RISKS.iter().filter(|(c, _)| codes.iter().any(|x| x.as_ref() == *c)).map(|(c, _)| c.to_string()).collect()
}

/// What a downloaded mode's graph does (`graph`) against what its list entry said (`listed`)
/// and what the player said yes to (`said_yes_to`): None to install; else the question to ask
/// again, naming what it really does, with a warning when the listing said less (or other).
pub fn risk_gate(graph: &[String], listed: &[String], said_yes_to: &[String]) -> Option<Value> {
    let real = known_codes(graph);
    if real.iter().all(|c| said_yes_to.iter().any(|y| y == c)) {
        return None;
    }
    let words = risk_words(&real);
    let differs = known_codes(listed) != real;
    let mut q = risk_question(&words);
    if differs {
        q = format!("This mode does more than its listing says. {q}");
    }
    Some(json!({"ok": false, "needsConfirm": q, "risks": words, "riskCodes": real, "differs": differs}))
}

/// The question before installing a mode that does these things.
pub fn risk_question(words: &[&str]) -> String {
    format!(
        "This mode does more than its own game: {}. A Game Mode Studio mode can only run the commands its checks allow, but it acts on your server while it runs. Install it?",
        words.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join("; ")
    )
}

/// A mode a map is made for: its permanent id, its short game id and its name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ModeRef {
    pub uid: String,
    pub id: String,
    pub name: String,
}

/// A permanent id: a UUID, lower case.
pub fn valid_uid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { *c == b'-' } else { c.is_ascii_digit() || (b'a'..=b'f').contains(c) })
}

/// A fresh permanent id (UUID version 4).
pub fn new_uid() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

fn clean_uid(s: &str) -> String {
    let l = s.trim().to_ascii_lowercase();
    if valid_uid(&l) { l } else { String::new() }
}

/// The health check's numbers (tools/health.rs), the newest run for this map version.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthMeta {
    /// good, warn or bad
    pub verdict: String,
    pub when: u64,
    pub mode: String,
    pub minutes: u32,
    pub bots: usize,
    pub stuck_pct: f32,
    pub falls: usize,
    pub kills_per_bot_min: f32,
    pub coverage_pct: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Listed,
    /// pulled: never offered, never installed
    Revoked,
    /// the author unlisted it (with the page's proof): out of lists, installed copies stay
    Unlisted,
    /// its GameBanana page (or its file) is gone; back by itself if the page returns
    Withdrawn,
    /// kept out of lists (asked by the author, or waiting on a question), still openable;
    /// also what a newer index's unknown value reads as, so one entry can't break the list
    /// (serde wants the catch-all last)
    #[serde(other)]
    Hidden,
}

impl Status {
    /// Kept out of the browser's lists (still openable by link; installed copies stay).
    pub fn kept_out(self) -> bool {
        matches!(self, Status::Hidden | Status::Unlisted | Status::Withdrawn)
    }

    /// The quiet note for someone who has it installed.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Status::Unlisted => Some("The author unlisted this. You can keep yours, or remove it."),
            Status::Withdrawn => Some("This creation's page was withdrawn from GameBanana. You can keep yours, or remove it."),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub kind: Kind,
    /// the GameBanana page
    pub model: Model,
    pub gb: i64,
    /// the file on that page this entry describes (0: the newest)
    pub file: i64,
    /// that file's MD5 as GameBanana lists it
    pub md5: String,
    pub name: String,
    pub author: String,
    pub summary: String,
    /// cstrike, czero
    pub games: Vec<String>,
    pub map: Option<MapMeta>,
    pub mode: Option<ModeMeta>,
    pub health: Option<HealthMeta>,
    /// https links, GameBanana's image host or ours
    pub screenshots: Vec<String>,
    pub tags: Vec<String>,
    /// picked by the Loner-Strike team
    pub featured: bool,
    pub added: u64,
    pub updated: u64,
    pub status: Status,
    /// a remix: the creation it was made from (Game Mode Studio modes)
    pub remix_of: Option<super::communitylib::Credit>,
    /// the creation's permanent id (a Studio mode's uid), "" for older entries: installs,
    /// updates and remixes match by it
    pub uid: String,
    /// the GameBanana member the list service registered it to (who owns its uid there), 0 unknown:
    /// a uid only links two entries of the same owner
    pub owner_id: i64,
    /// how well bots play it (game modes): great, ok, weak or none (the same scale as the app's
    /// own modes, games.ts BOT_RATING), "" not said
    pub bots: String,
    /// who made the assets it ships (tools/credits.rs), ≤ 64 entries
    pub credits: Vec<super::credits::AssetCredit>,
}

/// The bot-support scale (games.ts BotRating): anything else reads as "" (not said).
pub fn clean_bots(s: &str) -> String {
    let l = s.trim().to_ascii_lowercase();
    if ["great", "ok", "weak", "none"].contains(&l.as_str()) { l } else { String::new() }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Index {
    pub schema: u32,
    pub updated: u64,
    pub items: Vec<Item>,
    /// GameBanana pages pulled ("mod:123", "spray:45"): never offered, even when tagged
    pub revoked: Vec<String>,
    /// GameBanana member names whose game modes count as verified
    pub verified_authors: Vec<String>,
}

fn key(model: Model, id: i64) -> String {
    format!("{}:{id}", model.path().to_ascii_lowercase())
}

fn safe_mode_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn safe_shot(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split('/').next().unwrap_or("");
    SHOT_HOSTS.contains(&host) && url.len() < 400 && !url.contains(['"', '<', '>', ' ', '\\'])
}

fn one_line(s: &str, max: usize) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().chars().take(max).collect()
}

/// A path in a package that runs code or hides other files (nested archives, Metamod, dlls/).
pub fn code_file(path: &str) -> bool {
    let low = path.replace('\\', "/").to_ascii_lowercase();
    if low.ends_with('/') {
        return false;
    }
    let base = low.rsplit('/').next().unwrap_or("");
    CODE_EXT.iter().chain(ARCHIVE_EXT.iter()).any(|e| base.ends_with(e)) || low.contains("addons/metamod/") || format!("/{low}").contains("/dlls/")
}

/// Why a package can't be shared, if it can't: plugins, or files that run code.
pub fn package_refusal(item: &Item, contents: &[String]) -> Option<String> {
    if item.mode.as_ref().is_some_and(|m| !m.plugins.is_empty()) {
        return Some(PLUGIN_REFUSAL.into());
    }
    let bad: Vec<&String> = contents.iter().filter(|c| code_file(c)).collect();
    if bad.iter().any(|b| [".amxx", ".amx", ".sma", ".inc"].iter().any(|e| b.to_ascii_lowercase().ends_with(e))) {
        return Some(PLUGIN_REFUSAL.into());
    }
    (!bad.is_empty()).then(|| {
        format!(
            "The package has files that run code or can't be checked ({}). Only maps, textures, models, sounds, sprays and Game Mode Studio settings can be shared.",
            bad.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )
    })
}

/// Reads the index. Never panics; entries that don't check out are left out (a bad entry
/// can't break the list), and a format newer than this app is an error.
pub fn parse_index(bytes: &[u8]) -> Result<Index, String> {
    if bytes.len() as u64 > MAX_INDEX {
        return Err("The community list is too big.".into());
    }
    let mut idx: Index = serde_json::from_slice(bytes).map_err(|_| "The community list couldn't be read.".to_string())?;
    if idx.schema == 0 {
        return Err("The community list couldn't be read.".into());
    }
    if idx.schema > SCHEMA {
        return Err("The community list is newer than this app. Update Loner-Strike to see it.".into());
    }
    idx.items.truncate(MAX_ITEMS);
    let mut seen = HashSet::new();
    idx.items.retain_mut(|it| {
        if it.gb <= 0 || it.gb > 99_999_999 || it.kind == Kind::Other || !seen.insert(key(it.model, it.gb)) {
            return false;
        }
        if let Some(m) = &mut it.map {
            if !crate::maps::safe_arg(&m.name) || m.name.len() > 31 {
                return false;
            }
            m.mode_refs.retain(|r| safe_mode_id(&r.id));
            m.mode_refs.truncate(16);
            for r in &mut m.mode_refs {
                r.uid = clean_uid(&r.uid);
                r.name = one_line(&r.name, 40);
            }
        }
        it.uid = clean_uid(&it.uid);
        it.bots = clean_bots(&it.bots);
        it.credits = super::credits::clean_credits(std::mem::take(&mut it.credits));
        if let Some(m) = &mut it.mode {
            m.uid = clean_uid(&m.uid);
            if !safe_mode_id(&m.id) {
                return false;
            }
            m.maps.retain(|x| crate::maps::safe_arg(x));
            let known: Vec<String> = RISKS.iter().filter(|(c, _)| m.capabilities.iter().any(|x| x == c)).map(|(c, _)| c.to_string()).collect();
            m.capabilities = known;
            if let Some(r) = &mut m.requirements {
                r.clean();
            }
        }
        it.name = one_line(&it.name, 80);
        if let Some(r) = &mut it.remix_of {
            r.uid = clean_uid(&r.uid);
            r.name = one_line(&r.name, 80);
            r.author = one_line(&r.author, 40);
        }
        it.author = one_line(&it.author, 40);
        it.summary = one_line(&it.summary, 400);
        it.md5 = if it.md5.len() == 32 && it.md5.bytes().all(|c| c.is_ascii_hexdigit()) { it.md5.to_ascii_lowercase() } else { String::new() };
        it.screenshots.retain(|u| safe_shot(u));
        it.screenshots.truncate(MAX_SHOTS);
        it.games.retain(|g| g == "cstrike" || g == "czero");
        it.tags = it.tags.iter().map(|t| one_line(t, 24)).filter(|t| !t.is_empty()).take(12).collect();
        !it.name.is_empty()
    });
    idx.revoked = idx.revoked.iter().map(|r| r.trim().to_ascii_lowercase()).filter(|r| !r.is_empty()).collect();
    Ok(idx)
}

/// Where the index came from this time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Live,
    /// the last copy that could be read (the download server didn't answer, or sent junk)
    Cached,
    /// none at all: GameBanana's tag search only
    None,
}

/// The index: downloaded (and kept at `cache`), else the kept copy, else empty.
pub fn load_index(fetch: &dyn Fn() -> Result<Vec<u8>, String>, cache: &Path) -> (Index, Source) {
    if let Ok(b) = fetch() {
        if let Ok(idx) = parse_index(&b) {
            if let Some(d) = cache.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = crate::sys::write_atomic(cache, &b);
            return (idx, Source::Live);
        }
    }
    match std::fs::read(cache).ok().and_then(|b| parse_index(&b).ok()) {
        Some(idx) => (idx, Source::Cached),
        None => (Index { schema: SCHEMA, ..Default::default() }, Source::None),
    }
}

fn http_get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let r = crate::sys::http()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send()
        .map_err(|_| "unreachable".to_string())?;
    if !r.status().is_success() {
        return Err(format!("status {}", r.status()));
    }
    let mut out = Vec::new();
    r.take(limit + 1).read_to_end(&mut out).map_err(|e| e.to_string())?;
    if out.len() as u64 > limit {
        return Err("too big".into());
    }
    Ok(out)
}

static INDEX: Mutex<Option<(Instant, Index, Source)>> = Mutex::new(None);

/// The index as last read (downloaded at most every 15 minutes), for the library's update checks.
pub fn index_cached() -> Index {
    index(false).0
}

fn index(refresh: bool) -> (Index, Source) {
    let mut g = INDEX.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, idx, src)) = g.as_ref() {
        if !refresh && at.elapsed() < INDEX_FRESH && *src == Source::Live {
            return (idx.clone(), *src);
        }
    }
    let fetch = || {
        let body = http_get(INDEX_URL, MAX_INDEX)?;
        let sum = http_get(&format!("{INDEX_URL}.sha256"), 1024)?;
        checked_index(body, &sum)
    };
    let (idx, src) = load_index(&fetch, &dir().join("index.json"));
    *g = Some((Instant::now(), idx.clone(), src));
    (idx, src)
}

/// The index as downloaded, only when it matches the sha256 the list service publishes beside it
/// (`index.json.sha256`, "<hex>  index.json"). A mismatch (a half-finished upload, or a file
/// changed on the way) counts as not reaching the index: the kept copy is used instead.
pub fn checked_index(body: Vec<u8>, sum: &[u8]) -> Result<Vec<u8>, String> {
    let want = std::str::from_utf8(sum).ok().and_then(|t| t.split_whitespace().next()).map(|h| h.to_ascii_lowercase()).unwrap_or_default();
    if want.len() != 64 || !want.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("the index's checksum couldn't be read".into());
    }
    let have: String = Sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect();
    if have != want {
        return Err("the index doesn't match its checksum".into());
    }
    Ok(body)
}

// ---- GameBanana --------------------------------------------------------------------------------

/// A Loner-Strike creation as GameBanana's search lists it.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub model: Model,
    pub id: i64,
    pub kind: Kind,
    pub name: String,
    pub author: String,
    pub category: String,
    pub image: Option<String>,
    pub likes: i64,
    pub views: i64,
    pub added: u64,
    pub updated: u64,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Whether GameBanana's tags (strings "Title: Value" in lists, {_sTitle, _sValue} on pages)
/// carry the Loner-Strike tag.
pub fn has_tag(tags: &Value) -> bool {
    let ours = |v: &str| v.trim().eq_ignore_ascii_case(TAG_VALUE) || v.trim().eq_ignore_ascii_case(TAG_ALSO);
    let want = |t: &str, v: &str| t.trim().eq_ignore_ascii_case(TAG_TITLE) && v.split(',').any(ours);
    tags.as_array().into_iter().flatten().any(|t| match t {
        Value::String(x) => match x.split_once(':') {
            Some((a, b)) => want(a, b),
            None => ours(x),
        },
        Value::Object(_) => want(&s(&t["_sTitle"]), &s(&t["_sValue"])),
        _ => false,
    })
}

/// What kind of creation a GameBanana record is, from its item type and categories.
pub fn kind_of(model: &str, root: &str, sub: &str) -> Kind {
    if model.eq_ignore_ascii_case("Spray") {
        return Kind::Spray;
    }
    let (r, sb) = (root.to_ascii_lowercase(), sub.to_ascii_lowercase());
    if r == "maps" || r == "scraps" && sb == "maps" {
        Kind::Map
    } else if r == "prefabs" {
        Kind::Prefab
    } else if r == "textures" {
        Kind::Texture
    } else if sb.contains("amxx") {
        Kind::Mode
    } else {
        Kind::Other
    }
}

fn first_image(media: &Value, size: &str) -> Option<String> {
    let i = media.get("_aImages")?.as_array()?.first()?;
    let file = i.get(size).and_then(|v| v.as_str()).or_else(|| i.get("_sFile").and_then(|v| v.as_str()))?;
    let url = format!("{}/{}", s(&i["_sBaseUrl"]), file);
    safe_shot(&url).then_some(url)
}

/// One search record, if it's a tagged Counter-Strike creation.
pub fn hit_from_record(r: &Value) -> Option<Hit> {
    let model = Model::parse(r["_sModelName"].as_str()?)?;
    let id = r["_idRow"].as_i64().filter(|i| *i > 0)?;
    if !has_tag(&r["_aTags"]) {
        return None;
    }
    if let Some(g) = r["_aGame"]["_idRow"].as_i64() {
        if g != CS16_GAME {
            return None;
        }
    }
    let root = s(&r["_aRootCategory"]["_sName"]);
    let sub = s(&r["_aSubCategory"]["_sName"]);
    let kind = kind_of(r["_sModelName"].as_str().unwrap_or(""), &root, &sub);
    let n = |k: &str| r[k].as_i64().unwrap_or(0);
    Some(Hit {
        model,
        id,
        kind,
        name: one_line(&s(&r["_sName"]), 80),
        author: one_line(&s(&r["_aSubmitter"]["_sName"]), 40),
        category: if sub.is_empty() { root } else { sub },
        image: first_image(&r["_aPreviewMedia"], "_sFile220"),
        likes: n("_nLikeCount"),
        views: n("_nViewCount"),
        added: n("_tsDateAdded").max(0) as u64,
        updated: n("_tsDateModified").max(0) as u64,
    })
}

pub fn hits_from_page(v: &Value) -> Vec<Hit> {
    v["_aRecords"].as_array().into_iter().flatten().filter_map(hit_from_record).collect()
}

static SEARCH: Mutex<Option<(Instant, Vec<Hit>)>> = Mutex::new(None);

/// Every tagged creation GameBanana's search finds (cached a few minutes). Err: unreachable,
/// with the last list when there was one.
fn search(refresh: bool) -> Result<Vec<Hit>, Option<Vec<Hit>>> {
    let mut g = SEARCH.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, hits)) = g.as_ref() {
        if !refresh && at.elapsed() < SEARCH_FRESH {
            return Ok(hits.clone());
        }
    }
    let mut all: Vec<Hit> = Vec::new();
    for page in 1..=SEARCH_PAGES {
        let url = format!(
            "{GB_API}/Util/Search/Results?_sOrder=date&_idGameRow={CS16_GAME}&_csvFields=attribs&_sSearchString={TAG_VALUE}&_nPage={page}"
        );
        let v = match http_get(&url, 4 << 20).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            Some(v) => v,
            None if page == 1 => return Err(g.as_ref().map(|(_, h)| h.clone())),
            None => break,
        };
        let recs = v["_aRecords"].as_array().map(|a| a.len()).unwrap_or(0);
        for h in hits_from_page(&v) {
            if !all.iter().any(|x| x.model == h.model && x.id == h.id) {
                all.push(h);
            }
        }
        if recs == 0 || v["_aMetadata"]["_bIsComplete"].as_bool() == Some(true) {
            break;
        }
    }
    *g = Some((Instant::now(), all.clone()));
    Ok(all)
}

// ---- trust ---------------------------------------------------------------------------------------

/// What the app lets you do with a creation, and what it tells you first.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Trust {
    /// content | data | reviewed | verified | unreviewed | blocked
    pub level: &'static str,
    /// a short label for the badge
    pub label: &'static str,
    /// the trust rules allow installing it from the app
    pub allowed: bool,
    /// a question to answer before installing (plugins that will run on your server, a mode
    /// that runs server commands...)
    pub confirm: Option<String>,
    /// what a mode does beyond its own game, in plain words (a badge each)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<&'static str>,
    /// the same as codes, sent back with the yes (the install checks the mode against them)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub risk_codes: Vec<String>,
    /// one plain sentence about why
    pub note: String,
}

/// The trust rules (docs/COMMUNITY.md, "Trust"):
/// - pulled (revoked) pages are never offered;
/// - maps, prefabs, sprays and textures are content: the installer copies only content files,
///   so they install with one click whoever made them;
/// - a game mode without plugins is data (settings, graphs, components): one click too;
/// - a game mode with plugins needs every plugin's source and hash in the index, and then
///   either a review (reviewed: installs after a short confirm naming the plugins) or a
///   verified author (verified: installs after a stronger warning). Anything else, including
///   a tagged mode the index doesn't know, isn't installed from the app.
pub fn trust(kind: Kind, item: Option<&Item>, revoked: bool) -> Trust {
    let t = |level, label, allowed, confirm: Option<String>, note: &str| Trust { level, label, allowed, confirm, note: note.into(), risks: vec![], risk_codes: vec![] };
    if revoked || item.is_some_and(|i| i.status == Status::Revoked) {
        return t("blocked", "Pulled", false, None, "The Loner-Strike team pulled this one, so the app won't install it.");
    }
    if kind != Kind::Mode {
        return t("content", "Content", true, None, "Only map content is installed (maps, textures, models, sounds); nothing that runs code.");
    }
    let Some(mode) = item.and_then(|i| i.mode.as_ref()) else {
        return t("unreviewed", "Not checked", false, None, "This mode isn't in Loner-Strike's list yet, so nobody has said what's inside it.");
    };
    if mode.plugins.is_empty() {
        let risks = risk_words(&mode.capabilities);
        if risks.is_empty() {
            return t("data", "Settings only", true, None, "Settings and rules only: no plugins, nothing that runs code.");
        }
        let mut tr = t("data", "Settings only", true, Some(risk_question(&risks)), "A Game Mode Studio mode: no plugins of its own, but it does more than its own game (see the badges).");
        tr.risks = risks;
        tr.risk_codes = known_codes(&mode.capabilities);
        return tr;
    }
    // plugin modes aren't shared or installed from the community for now (whatever an index
    // says about their review); Game Mode Studio modes are settings only and install like content
    t(
        "unreviewed",
        "Plugins",
        false,
        None,
        "It runs plugins, and plugin modes can't be installed from the community yet. Modes made in Game Mode Studio can.",
    )
}

// ---- the list ----------------------------------------------------------------------------------

fn badges(it: Option<&Item>, kind: Kind) -> Vec<&'static str> {
    let mut b = Vec::new();
    let Some(it) = it else { return b };
    if it.featured {
        b.push("featured");
    }
    if let Some(m) = &it.map {
        if m.nav && m.lsv {
            b.push("bots");
        } else if m.nav {
            b.push("nav");
        }
    }
    if let Some(h) = &it.health {
        if h.verdict == "good" {
            b.push("checked");
        }
    }
    if kind == Kind::Mode && it.mode.as_ref().is_some_and(|m| m.plugins.is_empty()) {
        b.push("data");
    }
    b
}

/// One row of the list: GameBanana's facts, the index's extras, the trust rules' answer.
pub fn entry(hit: Option<&Hit>, it: Option<&Item>, revoked: bool, have: &HashSet<String>) -> Option<Value> {
    let (model, id) = match (hit, it) {
        (Some(h), _) => (h.model, h.id),
        (None, Some(i)) => (i.model, i.gb),
        _ => return None,
    };
    let kind = it.map(|i| i.kind).or(hit.map(|h| h.kind)).unwrap_or_default();
    let tr = trust(kind, it, revoked);
    if tr.level == "blocked" || it.is_some_and(|i| i.status.kept_out()) {
        return None;
    }
    let map = it.and_then(|i| i.map.as_ref());
    let image = hit.and_then(|h| h.image.clone()).or_else(|| it.and_then(|i| i.screenshots.first().cloned()));
    Some(json!({
        "model": model, "id": id, "key": key(model, id), "kind": kind,
        "name": it.map(|i| i.name.clone()).filter(|n| !n.is_empty()).or(hit.map(|h| h.name.clone())).unwrap_or_default(),
        "author": hit.map(|h| h.author.clone()).filter(|a| !a.is_empty()).or(it.map(|i| i.author.clone())).unwrap_or_default(),
        "summary": it.map(|i| i.summary.clone()).unwrap_or_default(),
        "category": hit.map(|h| h.category.clone()).unwrap_or_default(),
        "image": image,
        "likes": hit.map(|h| h.likes).unwrap_or(0),
        "added": hit.map(|h| h.added).filter(|a| *a > 0).or(it.map(|i| i.added)).unwrap_or(0),
        "updated": hit.map(|h| h.updated).filter(|a| *a > 0).or(it.map(|i| i.updated)).unwrap_or(0),
        "listed": it.is_some(),
        "map": map.map(|m| m.name.clone()),
        "bots": it.map(|i| i.bots.clone()).unwrap_or_default(),
        "modes": map.map(|m| if m.mode_refs.is_empty() { m.modes.clone() } else { m.mode_refs.iter().map(|r| if r.name.is_empty() { r.id.clone() } else { r.name.clone() }).collect() }).unwrap_or_default(),
        "have": map.is_some_and(|m| have.contains(&m.name.to_ascii_lowercase())),
        "badges": badges(it, kind),
        "trust": tr,
        "canTry": tr.allowed && kind.can_install(),
        "url": model.url(id),
    }))
}

/// Index entries and GameBanana hits as one list (each page once), filtered and sorted.
pub fn merge(idx: &Index, hits: &[Hit], have: &HashSet<String>, kind: Option<Kind>, q: &str, sort: &str) -> Vec<Value> {
    let revoked: HashSet<&str> = idx.revoked.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<Value> = Vec::new();
    let mut done = HashSet::new();
    for h in hits {
        let k = key(h.model, h.id);
        let it = idx.items.iter().find(|i| i.model == h.model && i.gb == h.id);
        if let Some(v) = entry(Some(h), it, revoked.contains(k.as_str()), have) {
            out.push(v);
        }
        done.insert(k);
    }
    // listed in the index but not in this search (GameBanana unreachable or slow to index)
    for it in &idx.items {
        let k = key(it.model, it.gb);
        if done.insert(k.clone()) {
            if let Some(v) = entry(None, Some(it), revoked.contains(k.as_str()), have) {
                out.push(v);
            }
        }
    }
    let q = q.trim().to_lowercase();
    out.retain(|v| {
        kind.is_none_or(|k| v["kind"] == json!(k))
            && (q.is_empty() || ["name", "author", "summary", "map"].iter().any(|f| v[f].as_str().is_some_and(|x| x.to_lowercase().contains(&q))) || v["modes"].as_array().is_some_and(|m| m.iter().any(|x| x.as_str() == Some(q.as_str()))))
    });
    let num = |v: &Value, k: &str| v[k].as_i64().unwrap_or(0);
    match sort {
        "liked" => out.sort_by_key(|v| -num(v, "likes")),
        "name" => out.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_lowercase()),
        _ => out.sort_by_key(|v| -num(v, "updated").max(num(v, "added"))),
    }
    // the team's picks first, then pages in the index, keeping the chosen order within each
    out.sort_by_key(|v| (!v["badges"].as_array().is_some_and(|b| b.contains(&json!("featured"))), !v["listed"].as_bool().unwrap_or(false)));
    out
}

/// Lower-case names of the maps in the player's game (and cstrike, for Condition Zero).
fn maps_here(g: &crate::game::Game) -> HashSet<String> {
    let mut set = HashSet::new();
    for d in [g.dir.join("maps"), g.hl.join("cstrike").join("maps")] {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if let Some(stem) = n.strip_suffix(".bsp") {
                set.insert(stem.to_string());
            }
        }
    }
    set
}

#[derive(Deserialize)]
struct BrowseQ {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    refresh: Option<String>,
}

async fn api_browse(State(st): State<Shared>, Query(q): Query<BrowseQ>) -> Response {
    let game = st.local_game();
    let refresh = q.refresh.is_some_and(|r| r == "1");
    json_answer(
        blocking(move || {
            let (idx, src) = index(refresh);
            let (hits, gb) = match search(refresh) {
                Ok(h) => (h, "live"),
                Err(Some(h)) => (h, "cached"),
                Err(None) => (Vec::new(), "offline"),
            };
            let have = game.as_deref().map(maps_here).unwrap_or_default();
            let kind = Kind::parse(&q.kind);
            let items = merge(&idx, &hits, &have, kind, &q.q.chars().take(60).collect::<String>(), &q.sort);
            Ok(json!({"items": items, "index": src, "gamebanana": gb, "indexUpdated": idx.updated, "tag": format!("{TAG_TITLE}: {TAG_VALUE}")}))
        })
        .await,
    )
}

// ---- one creation --------------------------------------------------------------------------

/// A GameBanana page's files, newest first (clean, supported ones marked).
pub fn files_of(page: &Value) -> Vec<Value> {
    let mut v: Vec<Value> = page["_aFiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let id = f["_idRow"].as_i64()?;
            let name = s(&f["_sFile"]);
            let low = name.to_lowercase();
            let av = s(&f["_sAvResult"]).to_lowercase();
            let clean = av.is_empty() || av == "clean";
            let supported = crate::archive::is_archive(&low) || [".bsp", ".wad", ".bmp", ".tga", ".png", ".jpg"].iter().any(|e| low.ends_with(e));
            Some(json!({"id": id, "file": name, "size": f["_nFilesize"].as_i64().unwrap_or(0), "added": f["_tsDateAdded"].as_i64().unwrap_or(0),
                        "md5": s(&f["_sMd5Checksum"]).to_ascii_lowercase(), "clean": clean, "supported": supported}))
        })
        .collect();
    v.sort_by_key(|f| -f["added"].as_i64().unwrap_or(0));
    v
}

fn profile(model: Model, id: i64) -> Result<Value, String> {
    let b = http_get(&format!("{GB_API}/{}/{id}/ProfilePage", model.path()), 4 << 20).map_err(|_| "Couldn't reach GameBanana. Check your internet connection.".to_string())?;
    let v: Value = serde_json::from_slice(&b).map_err(|_| "GameBanana sent something the app couldn't read. Try again in a minute.".to_string())?;
    if v.get("_sErrorCode").is_some() || v["_idRow"].as_i64() != Some(id) {
        return Err("That page isn't on GameBanana (any more).".into());
    }
    if ["_bIsTrashed", "_bIsPrivate", "_bIsWithheld"].iter().any(|k| v[*k] == json!(true)) {
        return Err(WITHDRAWN.into());
    }
    Ok(v)
}

async fn api_item(State(st): State<Shared>, UrlPath((model, id)): UrlPath<(String, i64)>) -> Response {
    let Some(model) = Model::parse(&model).filter(|_| id > 0) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "That isn't a GameBanana page."}))).into_response();
    };
    let game = st.local_game();
    json_answer(
        blocking(move || {
            let (idx, _) = index(false);
            let it = idx.items.iter().find(|i| i.model == model && i.gb == id).cloned();
            let page = profile(model, id).map_err(|e| Fail::new(StatusCode::OK, e))?;
            let tagged = has_tag(&page["_aTags"]);
            let root = s(&page["_aSuperCategory"]["_sName"]);
            let cat = s(&page["_aCategory"]["_sName"]);
            let kind = it.as_ref().map(|i| i.kind).unwrap_or_else(|| kind_of(model.path(), if root.is_empty() { &cat } else { &root }, &cat));
            let revoked = idx.revoked.iter().any(|r| *r == key(model, id));
            let tr = trust(kind, it.as_ref(), revoked);
            let images: Vec<String> = page["_aPreviewMedia"]["_aImages"]
                .as_array()
                .into_iter()
                .flatten()
                .take(MAX_SHOTS)
                .filter_map(|i| {
                    let f = i.get("_sFile530").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| s(&i["_sFile"]));
                    let u = format!("{}/{}", s(&i["_sBaseUrl"]), f);
                    safe_shot(&u).then_some(u)
                })
                .collect();
            let have = game.as_deref().map(maps_here).unwrap_or_default();
            Ok(json!({
                "model": model, "id": id, "kind": kind, "name": one_line(&s(&page["_sName"]), 80),
                "author": one_line(&s(&page["_aSubmitter"]["_sName"]), 40), "category": cat,
                "text": crate::gamebanana::plain(&s(&page["_sText"]), 1500),
                "images": images, "files": files_of(&page), "url": model.url(id), "tagged": tagged,
                "likes": page["_nLikeCount"].as_i64().unwrap_or(0), "downloads": page["_nDownloadCount"].as_i64().unwrap_or(0),
                "item": it, "trust": tr, "canTry": tr.allowed && kind.can_install() && tagged,
                "why": if !tagged { Some("This page doesn't carry the Loner-Strike tag any more.") } else if !kind.can_install() { Some(kind.no_installer()) } else { None },
                "have": it.as_ref().and_then(|i| i.map.as_ref()).is_some_and(|m| have.contains(&m.name.to_ascii_lowercase())),
                "installed": installed_json(model, id, &idx, &page),
                // a mode's "Plays on" and "Needs", each with whether you have it and where to get it
                "needs": it.as_ref().and_then(|i| i.mode.as_ref()).and_then(|m| m.requirements.as_ref()).map(|r| needs_json(r, &idx, &have, &|p: &str| game.as_deref().is_some_and(|g| g.dir.join(p).is_file() || crate::tools::doctor::is_stock(&crate::mapcopy::Finder::default(), &g.hl, p)))),
            }))
        })
        .await,
    )
}

/// A mode's requirements for its page: each map and file, whether it's here, and the item
/// that provides it (a community map by its name; content by its link).
pub fn needs_json(r: &ModeRequirements, idx: &Index, maps_have: &HashSet<String>, file_here: &dyn Fn(&str) -> bool) -> Value {
    let map = |name: &str| {
        let base = crate::maps::is_stock_file(&format!("maps/{name}.bsp"));
        let item = idx.items.iter().find(|i| i.map.as_ref().is_some_and(|m| m.name.eq_ignore_ascii_case(name)));
        json!({"name": name, "have": base || maps_have.contains(&name.to_ascii_lowercase()), "base": base,
               "item": item.map(|i| json!({"model": i.model, "id": i.gb, "name": i.name}))})
    };
    let mut maps: Vec<String> = vec![];
    for m in std::iter::once(&r.default_map).chain(r.maps.iter()).filter(|m| !m.is_empty()) {
        if !maps.iter().any(|x| x.eq_ignore_ascii_case(m)) {
            maps.push(m.clone());
        }
    }
    json!({
        "defaultMap": if r.default_map.is_empty() { Value::Null } else { json!(r.default_map) },
        "maps": maps.iter().map(|m| map(m)).collect::<Vec<_>>(),
        "content": r.content.iter().map(|c| json!({"path": c.path, "kind": c.kind, "have": file_here(&c.path), "link": c.link})).collect::<Vec<_>>(),
    })
}

#[derive(Deserialize)]
struct TryReq {
    model: String,
    id: i64,
    #[serde(default)]
    file: Option<i64>,
    /// the player answered the trust question
    #[serde(default)]
    confirmed: bool,
    /// the risk codes the player was shown and said yes to (a mode installs only when what its
    /// graph really does is among them)
    #[serde(default)]
    risks: Vec<String>,
}

/// The file to install: the one asked for, else the index's, else the newest clean supported one.
pub fn pick_file(files: &[Value], asked: Option<i64>, listed: i64) -> Result<i64, &'static str> {
    let ok = |f: &&Value| f["clean"] == json!(true) && f["supported"] == json!(true);
    let want = asked.filter(|a| *a > 0).or((listed > 0).then_some(listed));
    if let Some(w) = want {
        return files.iter().filter(ok).find(|f| f["id"].as_i64() == Some(w)).and_then(|f| f["id"].as_i64()).ok_or("The file this creation lists isn't on its GameBanana page any more (the author removed or replaced it), or GameBanana flagged it.");
    }
    files.iter().find(ok).and_then(|f| f["id"].as_i64()).ok_or("There's no file on the page the app can install.")
}

async fn api_try(State(st): State<Shared>, h: HeaderMap, Json(req): Json<TryReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let Some(model) = Model::parse(&req.model).filter(|_| req.id > 0) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "That isn't a GameBanana page."}))).into_response();
    };
    if st.local_game().is_none() {
        return json_answer(Err(Fail::new(StatusCode::CONFLICT, NO_GAME)));
    }
    json_answer(blocking(move || start_try(&st, model, req.id, req.file, req.confirmed, &req.risks).map_err(|e| Fail::new(StatusCode::OK, e))).await)
}

/// The library entry for a page, and whether a newer file is out (from the index, else the page).
fn installed_json(model: Model, id: i64, idx: &Index, page: &Value) -> Value {
    let Some(it) = super::communitylib::load(&super::communitylib::lib_dir(), model, id) else { return Value::Null };
    let from_page = files_of(page).iter().filter(|f| f["clean"] == json!(true) && f["supported"] == json!(true)).filter_map(|f| f["id"].as_i64()).max().filter(|f| *f > it.file);
    let listed = idx.items.iter().any(|e| e.model == model && e.gb == id);
    let update = super::communitylib::newer(&it, idx).or(if listed { None } else { from_page });
    json!({"key": it.key(), "file": it.file, "installed": it.installed, "maps": it.maps, "mode": it.mode, "prefabs": it.prefabs, "update": update})
}

/// Try it (and Update): the page read again, the trust rules, the file picked, then the
/// installer for its kind. Every install is noted in the community library.
pub fn start_try(st: &Shared, model: Model, id: i64, asked: Option<i64>, confirmed: bool, said_yes_to: &[String]) -> Result<Value, String> {
    let game = st.local_game().ok_or(NO_GAME)?;
    let (idx, _) = index(false);
    let it = idx.items.iter().find(|i| i.model == model && i.gb == id).cloned();
    if let Some(why) = it.as_ref().and_then(|i| try_refusal(i.status)) {
        return Err(why.into());
    }
    let page = profile(model, id)?;
    if !has_tag(&page["_aTags"]) {
        return Err("This page doesn't carry the Loner-Strike tag, so it isn't installed from here. Use Get more instead.".into());
    }
    let cat = s(&page["_aCategory"]["_sName"]);
    let root = s(&page["_aSuperCategory"]["_sName"]);
    let kind = it.as_ref().map(|i| i.kind).unwrap_or_else(|| kind_of(model.path(), if root.is_empty() { &cat } else { &root }, &cat));
    let tr = trust(kind, it.as_ref(), idx.revoked.iter().any(|r| *r == key(model, id)));
    if it.as_ref().and_then(|i| i.mode.as_ref()).is_some_and(|m| !m.plugins.is_empty()) {
        return Err(PLUGIN_REFUSAL.into());
    }
    if !tr.allowed {
        return Err(tr.note);
    }
    if tr.confirm.is_some() && !confirmed {
        return Err("Confirm first.".into());
    }
    let author = one_line(&s(&page["_aSubmitter"]["_sName"]), 40);
    let name = it.as_ref().map(|i| i.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| one_line(&s(&page["_sName"]), 80));
    let listed = it.as_ref().map(|i| i.file).unwrap_or(0);
    let mut files = files_of(&page);
    let mut note = super::communitylib::Installed {
        kind,
        model,
        gb: id,
        name,
        author: author.clone(),
        game: game.dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        uid: it.as_ref().map(|i| i.uid.clone()).unwrap_or_default(),
        // (who uploaded it: an update by uid has to come from the same member)
        owner_id: page["_aSubmitter"]["_idRow"].as_i64().filter(|i| *i > 0).unwrap_or(0),
        ..Default::default()
    };
    // the creation's own credits (who made the assets it ships), kept so reusing them keeps the chain
    if let Some(c) = it.as_ref().map(|i| &i.credits).filter(|c| !c.is_empty()) {
        let d = super::communitylib::lib_dir().join(super::communitylib::dir_name(model, id));
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("credits.json"), serde_json::to_vec_pretty(c).unwrap_or_default());
    }
    if kind == Kind::Prefab || kind == Kind::Mode {
        // small downloads installed here, no job: prefabs into the editor's library, modes built
        // by the Studio (a prefab can be a bare .map file, a mode is a zip with its graph)
        if kind == Kind::Prefab {
            for f in files.iter_mut().filter(|f| s(&f["file"]).to_lowercase().ends_with(".map")) {
                f["supported"] = json!(true);
            }
        }
        let file = pick_file(&files, asked, listed)?;
        let f = files.iter().find(|f| f["id"].as_i64() == Some(file)).cloned().unwrap_or_default();
        note.file = file;
        note.md5 = s(&f["md5"]);
        let (tmp, src) = fetch_unpacked(file, &s(&f["file"]), &note.md5, if kind == Kind::Prefab { 128 << 20 } else { 64 << 20 }, kind == Kind::Prefab)?;
        let r = (|| {
            if kind == Kind::Mode {
                let names: Vec<String> = crate::archive::walk(&src).iter().filter_map(|p| p.strip_prefix(&src).ok().map(|r| r.to_string_lossy().replace('\\', "/"))).collect();
                if let Some(why) = package_refusal(&Item { kind, ..Default::default() }, &names) {
                    return Err(why);
                }
                // what the mode really does (its graph, whatever the list said); installed only
                // when that's among what the player said yes to, else asked again
                let g = super::communitylib::mode_graph(&src)?;
                let caps: Vec<String> = super::studio::graph::capabilities(&g).iter().map(|c| c.to_string()).collect();
                let listed: Vec<String> = it.as_ref().and_then(|i| i.mode.as_ref()).map(|m| m.capabilities.clone()).unwrap_or_default();
                if let Some(ask) = risk_gate(&caps, &listed, said_yes_to) {
                    return Ok(ask);
                }
                return super::communitylib::install_mode(st, &src, note.clone());
            }
            // P5: the creation's page and uploader, so only a later version of this one can update
            // the prefabs (after the player says yes); a uid alone never replaces anything
            let origin = crate::editor::prefab::Origin::community(&format!("{}:{}", format!("{:?}", note.model).to_lowercase(), note.gb), note.owner_id);
            let v = super::editor::prefabs::install_download(&src, &author, Some(&game.dir), &origin)?;
            note.prefabs = v["ids"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
            note.installed = now();
            super::communitylib::save(&super::communitylib::lib_dir(), &note)?;
            Ok(json!({"ok": true, "kind": "prefab", "prefabs": v["prefabs"], "wads": v["wads"]}))
        })();
        let _ = std::fs::remove_dir_all(&tmp);
        return r;
    }
    let section = kind.installer().ok_or_else(|| kind.no_installer().to_string())?;
    let file = pick_file(&files, asked, listed)?;
    note.file = file;
    note.md5 = files.iter().find(|f| f["id"].as_i64() == Some(file)).map(|f| s(&f["md5"])).unwrap_or_default();
    note.section = section.into();
    note.maps = it.as_ref().and_then(|i| i.map.as_ref()).map(|m| vec![m.name.clone()]).unwrap_or_default();
    let job = game.store.start_section_install(section, id, file).map_err(|e| e.message())?;
    super::communitylib::note_after_job(game.store.clone(), job.clone(), note);
    Ok(json!({"ok": true, "job": job, "section": section, "map": it.and_then(|i| i.map).map(|m| m.name)}))
}

/// Downloads a file from GameBanana (its own hosts only, checked against the page's MD5) and
/// unpacks it into a fresh work folder: (the work folder to delete afterwards, the unpacked files).
/// `bare_map`: a single .map file is taken as it is (prefabs).
fn fetch_unpacked(file: i64, fname: &str, md5: &str, limit: u64, bare_map: bool) -> Result<(PathBuf, PathBuf), String> {
    const HOSTS: &[&str] = &["gamebanana.com", "files.gamebanana.com"];
    let r = crate::sys::http()
        .get(format!("https://gamebanana.com/dl/{file}"))
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(300))
        .send()
        .map_err(|_| "Couldn't reach GameBanana. Check your internet connection.".to_string())?;
    let host = r.url().host_str().unwrap_or("").to_lowercase();
    if !HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}"))) {
        return Err(format!("The download was redirected somewhere unexpected ({host})."));
    }
    if matches!(r.status().as_u16(), 404 | 410) {
        return Err("The file isn't on GameBanana any more (the author may have removed it).".into());
    }
    if !r.status().is_success() {
        return Err("GameBanana didn't send the file. Try again in a minute.".into());
    }
    let mut bytes = Vec::new();
    r.take(limit + 1).read_to_end(&mut bytes).map_err(|_| "The download was cut off. Try again.".to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("That download is over {} MB.", limit >> 20));
    }
    if md5.len() == 32 && md5_hex(&bytes) != md5 {
        return Err("The download doesn't match the file GameBanana lists. Try again.".into());
    }
    let tmp = crate::sys::make_temp_dir(&dir().join("work"), "dl-").map_err(|e| e.to_string())?;
    let res = (|| {
        let low = fname.to_lowercase();
        let src = tmp.join("x");
        std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
        if bare_map && low.ends_with(".map") {
            let base = Path::new(&fname.replace('\\', "/")).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "prefab.map".into());
            std::fs::write(src.join(base), &bytes).map_err(|e| e.to_string())?;
        } else if crate::archive::is_archive(&low) {
            let a = tmp.join(format!("download{}", crate::archive::ext_of(&low)));
            std::fs::write(&a, &bytes).map_err(|e| e.to_string())?;
            crate::archive::extract(&a, &src, 512 << 20)?;
            crate::archive::extract_nested(&src)?;
        } else {
            return Err("That file isn't something the app can install here (it wants a zip).".to_string());
        }
        Ok(src)
    })();
    match res {
        Ok(src) => Ok((tmp, src)),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

// ---- publishing ------------------------------------------------------------------------------

/// What publishing would ship, and what stands in the way.
#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    /// things that stop it (a broken map, one of Valve's own)
    pub blockers: Vec<String>,
    /// things players would notice (missing files, no nav for the bots)
    pub warnings: Vec<String>,
    /// nice to have
    pub tips: Vec<String>,
    pub map: MapMeta,
    pub health: Option<HealthMeta>,
    /// game paths of the files that go in the package (beside the map)
    pub files: Vec<String>,
    pub title: String,
    pub game: String,
    /// where each shipped custom file came from, with its terms (tools/credits.rs)
    pub assets: Vec<super::credits::Asset>,
    /// who to credit (README, GameBanana's Credits module, the index)
    pub credits: Vec<super::credits::AssetCredit>,
}

/// The newest health check of this map (any mode), from the reports folder.
pub fn latest_health(reports: &Path, map: &str) -> Option<HealthMeta> {
    let mut best: Option<super::health::Report> = None;
    for e in std::fs::read_dir(reports).ok()?.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") || e.metadata().map(|m| m.len() > 4 << 20).unwrap_or(true) {
            continue;
        }
        let Some(r) = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<super::health::Report>(&b).ok()) else { continue };
        if r.map.eq_ignore_ascii_case(map) && best.as_ref().is_none_or(|b| r.when > b.when) {
            best = Some(r);
        }
    }
    best.map(|r| HealthMeta {
        verdict: r.verdict,
        when: r.when,
        mode: r.mode,
        minutes: r.minutes,
        bots: r.bots_seen,
        stuck_pct: r.stuck_pct,
        falls: r.falls,
        kills_per_bot_min: r.kills_per_bot_min,
        coverage_pct: r.coverage_pct,
    })
}

fn sha256_file(p: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The modes a map can say it's made for, in Publish: the game's mode list without leftovers. A
/// Game Mode Studio mode (its plugins.ini names `studio_<id>.amxx`) counts only when you still have
/// its graph or it came from Community (Playtest and test builds leave modes.ini lines like
/// "V3 check" behind). Each with its permanent id when the Studio gave it one; names one line.
pub fn mode_choices(game_dir: &Path, graphs: &Path, lib: &[super::communitylib::Installed]) -> Vec<Value> {
    let gm = game_dir.join("addons/amxmodx/configs/gamemodes");
    let mut seen = std::collections::HashSet::new();
    crate::extras::parse_modes(&crate::sys::read_latin1(&gm.join("modes.ini")).unwrap_or_default())
        .into_iter()
        .filter(|(id, _)| safe_mode_id(id) && seen.insert(id.clone()))
        .filter_map(|(id, name)| {
            let studio = crate::sys::read_latin1(&gm.join(format!("{id}.plugins.ini"))).is_ok_and(|t| t.to_lowercase().contains(&format!("studio_{}.amxx", id.to_lowercase())));
            let community = lib.iter().any(|i| i.kind == Kind::Mode && i.mode == id);
            let graph = graphs.join(format!("{id}.graph.json")).is_file();
            if studio && !community && !graph {
                return None;
            }
            let uid = if graph { graph_uid(graphs, &id) } else { lib.iter().find(|i| i.mode == id).map(|i| i.uid.clone()).unwrap_or_default() };
            let name = one_line(&name, 40);
            Some(json!({"id": id, "name": if name.is_empty() { id.clone() } else { name }, "uid": uid, "studio": studio}))
        })
        .collect()
}

/// The modes a compiled map says it's made for: worldspawn `ls_modes` (mode ids separated by
/// spaces, commas or semicolons; the map editor's Made for). Plain mode ids only, at most 12.
pub fn made_for(bsp: &Path) -> Vec<String> {
    made_for_refs(bsp).into_iter().map(|r| r.id).collect()
}

/// The same, with each mode's permanent id when the map gives one (`uid:id` pairs).
pub fn made_for_refs(bsp: &Path) -> Vec<ModeRef> {
    let Ok(l) = crate::goldsrc::bsp::read_lumps(bsp, &[crate::goldsrc::bsp::LUMP_ENTITIES]) else { return vec![] };
    let text = crate::rcon::latin1(&l[0]);
    let ents = crate::goldsrc::entities::parse(text.trim_end_matches('\0'));
    let Some(world) = ents.iter().find(|e| e.get("classname") == Some("worldspawn")) else { return vec![] };
    let mut v: Vec<ModeRef> = Vec::new();
    for tok in world.get("ls_modes").unwrap_or("").split([' ', ',', ';']).map(|m| m.trim().to_ascii_lowercase()) {
        let (uid, m) = match tok.rsplit_once(':') {
            Some((u, m)) => (clean_uid(u), m.to_string()),
            None => (String::new(), tok.clone()),
        };
        if safe_mode_id(&m) && m.len() <= 24 && !v.iter().any(|r| r.id == m) && v.len() < 12 {
            v.push(ModeRef { uid, id: m, name: String::new() });
        }
    }
    v
}

/// Checks a map the way the map doctor does and lists what would go in the package.
pub fn readiness(m: &MapFile, reports: &Path) -> Readiness {
    let mut r = Readiness { game: m.game.clone(), ..Default::default() };
    let doc = super::doctor::diagnose(m);
    r.title = doc["title"].as_str().filter(|t| !t.is_empty()).unwrap_or(&m.name).to_string();
    if super::doctor::stock_map(&m.name, &m.game) {
        r.blockers.push("This is one of the game's own maps. Only maps you made (or have the right to share) can be published.".into());
    }
    if doc["bsp"]["ok"] != json!(true) {
        r.blockers.push(format!("The map file is damaged: {}", doc["bsp"]["error"].as_str().unwrap_or("it can't be read")));
    }
    let missing_tex = doc["textures"]["missing"].as_array().map(|a| a.len()).unwrap_or(0);
    if missing_tex > 0 {
        r.warnings.push(if missing_tex == 1 { "1 texture isn't in any texture file here; players would see a checkerboard. Fix it in the map doctor first.".to_string() } else { format!("{missing_tex} textures aren't in any texture file here; players would see a checkerboard. Fix it in the map doctor first.") });
    }
    // the files that go with it: where the game looks and not something every player has
    let f = crate::mapcopy::Finder::default();
    let ents = crate::goldsrc::bsp::read_lumps(&m.bsp, &[crate::goldsrc::bsp::LUMP_ENTITIES]).map(|l| crate::rcon::latin1(&l[0]).trim_end_matches('\0').to_string()).unwrap_or_default();
    let checks = super::doctor::file_checks(&f, &m.hl, &m.game, &m.name, &ents);
    let mut missing = 0;
    for c in &checks {
        match &c.found {
            super::doctor::Found::Here { stock: false, .. } => r.files.push(c.rel.clone()),
            super::doctor::Found::Here { stock: true, .. } => {}
            _ => missing += 1,
        }
    }
    if missing > 0 {
        r.warnings.push(format!("{missing} files the map uses aren't where the game looks; they'd be missing for players. The map doctor can copy or find them."));
    }
    r.map.name = m.name.clone();
    r.map.bsp_size = std::fs::metadata(&m.bsp).map(|x| x.len()).unwrap_or(0);
    r.map.bsp_sha256 = sha256_file(&m.bsp).unwrap_or_default();
    r.map.overview = r.files.iter().any(|x| x.to_lowercase().starts_with("overviews/"));
    r.map.res = true; // a fresh .res is always written into the package
    r.map.mode_refs = made_for_refs(&m.bsp);
    let nav = &doc["nav"];
    r.map.nav = nav["exists"] == json!(true) && nav.get("error").is_none() && nav["matchesMap"] != json!(false);
    r.map.places = r.map.nav && nav["namedAreas"].as_u64().unwrap_or(0) > 0;
    r.map.lsv = r.map.nav && doc["lsv"]["done"] == json!(true) && doc["lsv"]["fits"] != json!(false);
    if !r.map.nav {
        r.warnings.push("Bots don't know this map yet (no nav that fits it). Teach them in the map doctor so it gets the \"Works with bots\" badge.".into());
    } else if !r.map.lsv {
        r.tips.push("The bots' map knowledge isn't finished; play the map with bots for a while (or run the map simulator) and it fills in.".into());
    }
    if r.map.nav && !r.map.places {
        r.tips.push("Name places on the nav (Callouts) so bots say where they are.".into());
    }
    if !r.map.overview {
        r.tips.push("There's no overview (radar) picture for this map.".into());
    }
    r.health = latest_health(reports, &m.name);
    // where each shipped custom file came from, and whether its terms let it be shared
    let ctx = super::credits::Ctx::load(&crate::sys::app_dir());
    r.assets = super::credits::assets_for(m, &r.files, &ctx);
    let (credits, warnings, blockers) = super::credits::for_publish(&r.assets);
    r.credits = credits;
    r.warnings.extend(warnings);
    r.blockers.extend(blockers);
    match &r.health {
        None => r.tips.push("Run the map simulator: a good result earns the \"Checked with bots\" badge.".into()),
        Some(h) if h.verdict == "bad" => r.warnings.push("The last map simulation found real problems (falls, stuck bots or errors). Have a look before sharing.".into()),
        _ => {}
    }
    r
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Shot {
    pub name: String,
    /// base64 (a data: URL is fine)
    pub data: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct PublishReq {
    pub title: String,
    pub summary: String,
    pub author: String,
    pub modes: Vec<String>,
    pub screenshots: Vec<Shot>,
    /// a game mode: maps it's made for
    pub maps: Vec<String>,
    /// a game mode: how well bots play it (great / ok / weak / none)
    pub bots: String,
}

/// The finished package.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Package {
    /// the zip's file name (in the packages folder)
    pub file: String,
    pub size: u64,
    pub md5: String,
    pub sha256: String,
    /// paths inside the zip
    pub contents: Vec<String>,
    /// screenshots saved beside it, for GameBanana's picture upload
    pub screenshots: Vec<String>,
    /// what goes in the index once it's on GameBanana
    pub item: Item,
    pub warnings: Vec<String>,
}

/// [Caps] The publish request: its screenshots as base64, plus the text.
pub const PUBLISH_BODY: usize = MAX_SHOTS * (MAX_SHOT * 4 / 3 + 64) + (64 << 10);

/// [Caps]
pub(crate) fn package_too_big(zipped: u64) -> String {
    format!(
        "The package is {} — the most is {} (the community list checks every file in packages up to that size). Leave some files out, or share it on GameBanana only.",
        crate::caps::size(zipped),
        crate::caps::size(MAX_PACKAGE_ZIP)
    )
}

/// A screenshot from the page: PNG or JPEG, not too big.
pub fn decode_shot(s: &Shot) -> Result<(Vec<u8>, &'static str), String> {
    use base64::Engine;
    let data = s.data.split_once(";base64,").map(|x| x.1).unwrap_or(&s.data);
    if data.len() > MAX_SHOT * 4 / 3 + 8 {
        return Err("A screenshot is over 8 MB.".into());
    }
    let b = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| "A screenshot couldn't be read.".to_string())?;
    let ext = match image::guess_format(&b) {
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::Jpeg) => "jpg",
        _ => return Err("Screenshots have to be PNG or JPEG pictures.".into()),
    };
    // a picture that says it's one but isn't is refused too; [Caps] by pixels as well
    crate::caps::decode_picture(&b, None, crate::caps::MAX_PICTURE_SIDE).map_err(|e| if e.contains("too big") { e.replacen("That picture", "A screenshot", 1) } else { "A screenshot is damaged.".to_string() })?;
    Ok((b, ext))
}

fn readme(item: &Item, r: &Readiness) -> String {
    let m = &r.map;
    let mut t = format!("{}\r\nby {}\r\n\r\n{}\r\n\r\nMade with Loner-Strike (lonerstrike.com).\r\n\r\n", item.name, item.author, item.summary);
    t.push_str("To install by hand: copy everything except loner-strike.json and this file into your\r\ncstrike folder (or czero for Condition Zero), keeping the folders.\r\n");
    t.push_str("With Loner-Strike: find it under Tools > Community and click Try it.\r\n\r\n");
    if m.nav {
        t.push_str(if m.lsv { "Bots: ships with a nav and finished bot map knowledge.\r\n" } else { "Bots: ships with a nav.\r\n" });
    }
    if !m.modes.is_empty() {
        t.push_str(&format!("Game modes: {}\r\n", m.modes.join(", ")));
    }
    t.push_str(&super::credits::readme_section(&item.credits));
    t
}

/// Builds the upload package for a map: the map, its nav and knowledge, a fresh .res, its
/// overview and every custom file it uses (at their game paths, so GameBanana's usual layout and
/// the app's installer both read it), a README and loner-strike.json (the index entry to be).
/// Screenshots go beside the zip. Refuses when `readiness` has blockers.
pub fn build_package(m: &MapFile, r: &Readiness, req: &PublishReq, out_dir: &Path) -> Result<Package, String> {
    if let Some(b) = r.blockers.first() {
        return Err(b.clone());
    }
    let title = one_line(&req.title, 80);
    if title.is_empty() {
        return Err("Give it a name.".into());
    }
    let author = one_line(&req.author, 40);
    if author.is_empty() {
        return Err("Say who made it (your GameBanana name).".into());
    }
    if req.screenshots.len() > MAX_SHOTS {
        return Err(format!("At most {MAX_SHOTS} screenshots."));
    }
    let shots = req.screenshots.iter().map(decode_shot).collect::<Result<Vec<_>, _>>()?;
    let mut map = r.map.clone();
    map.modes = req.modes.iter().map(|x| x.trim().to_ascii_lowercase()).filter(|x| safe_mode_id(x)).take(12).collect();
    map.modes.dedup();
    let names = crate::extras::parse_modes(&crate::sys::read_latin1(&m.hl.join(&m.game).join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default());
    let known = r.map.mode_refs.clone();
    map.mode_refs = map
        .modes
        .iter()
        .map(|id| ModeRef {
            uid: known.iter().find(|k| k.id == *id).map(|k| k.uid.clone()).unwrap_or_default(),
            id: id.clone(),
            name: names.iter().find(|(n, _)| n == id).map(|(_, n)| one_line(n, 40)).unwrap_or_default(),
        })
        .collect();
    let item = Item {
        kind: Kind::Map,
        name: title,
        author,
        summary: one_line(&req.summary, 400),
        games: vec![r.game.clone()],
        map: Some(map),
        health: r.health.clone(),
        credits: r.credits.clone(),
        added: now(),
        updated: now(),
        ..Default::default()
    };

    std::fs::create_dir_all(out_dir).map_err(|e| format!("Couldn't make the packages folder ({e})."))?;
    let stem = format!("{}-loner-strike", m.name);
    let zip_path = out_dir.join(format!("{stem}.zip"));
    let part = out_dir.join(format!("{stem}.zip.part"));
    let game_dir = m.hl.join(&m.game);
    let path = super::doctor::game_path(&m.hl, &m.game);
    let f = crate::mapcopy::Finder::default();

    // (zip path, file on disk) for everything that ships; a fresh .res, the README and the manifest are written directly
    let mut files: Vec<(String, PathBuf)> = vec![(format!("maps/{}.bsp", m.name), m.bsp.clone())];
    if item.map.as_ref().is_some_and(|x| x.nav) {
        if let Some(p) = m.sibling("nav") {
            files.push((format!("maps/{}.nav", m.name), p));
        }
        if let (true, Some(p)) = (item.map.as_ref().is_some_and(|x| x.lsv), m.sibling("lsv")) {
            files.push((format!("maps/{}.lsv", m.name), p));
        }
    }
    for rel in &r.files {
        if let Some(p) = path.iter().find_map(|d| f.find(d, rel)) {
            files.push((rel.replace('\\', "/"), p));
        }
    }
    let names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    if let Some(why) = package_refusal(&item, &names) {
        return Err(why);
    }
    let wanted = r.files.clone();
    let res = super::doctor::res_text(&m.name, &wanted, &[]);
    let manifest = json!({"schema": SCHEMA, "app": env!("CARGO_PKG_VERSION"), "item": item});
    let readme_text = readme(&item, r);

    let mut total: u64 = 0;
    let mut contents = Vec::new();
    let mut write = || -> Result<(), String> {
        let out = std::fs::File::create(&part).map_err(|e| format!("Couldn't write the package ({e})."))?;
        let mut z = zip::ZipWriter::new(std::io::BufWriter::new(out));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(false);
        for (name, p) in &files {
            // nothing from outside the game folders (a link pointing elsewhere)
            let real = std::fs::canonicalize(p).map_err(|e| format!("Couldn't read {name} ({e})."))?;
            if !real.starts_with(std::fs::canonicalize(&m.hl).unwrap_or_else(|_| m.hl.clone())) {
                return Err(format!("{name} isn't inside the game folder."));
            }
            total += std::fs::metadata(&real).map(|x| x.len()).unwrap_or(0);
            if total > MAX_PACKAGE {
                return Err("The package would be over 1 GB.".into());
            }
            z.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            let mut src = std::fs::File::open(&real).map_err(|e| format!("Couldn't read {name} ({e})."))?;
            std::io::copy(&mut src, &mut z).map_err(|e| format!("Couldn't write {name} ({e})."))?;
            contents.push(name.clone());
        }
        for (name, body) in [
            (format!("maps/{}.res", m.name), res.clone().into_bytes()),
            ("README.txt".to_string(), readme_text.clone().into_bytes()),
            ("loner-strike.json".to_string(), serde_json::to_vec_pretty(&manifest).unwrap_or_default()),
        ] {
            z.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(&body).map_err(|e| e.to_string())?;
            contents.push(name);
        }
        z.finish().map_err(|e| format!("Couldn't finish the package ({e})."))?.flush().map_err(|e| e.to_string())?;
        Ok(())
    };
    let done = write();
    if let Err(e) = done {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let _ = std::fs::remove_file(&zip_path);
    std::fs::rename(&part, &zip_path).map_err(|e| format!("Couldn't save the package ({e})."))?;
    let _ = game_dir;

    // screenshots beside it
    let shot_dir = out_dir.join(format!("{stem}-screenshots"));
    let _ = std::fs::remove_dir_all(&shot_dir);
    let mut shot_names = Vec::new();
    if !shots.is_empty() {
        std::fs::create_dir_all(&shot_dir).map_err(|e| format!("Couldn't save the screenshots ({e})."))?;
        for (i, (b, ext)) in shots.iter().enumerate() {
            let n = format!("{}-{}.{ext}", m.name, i + 1);
            std::fs::write(shot_dir.join(&n), b).map_err(|e| format!("Couldn't save the screenshots ({e})."))?;
            shot_names.push(n);
        }
    }
    // [Caps] checked before it's read back whole for its MD5
    let zipped = std::fs::metadata(&zip_path).map(|x| x.len()).unwrap_or(0);
    if zipped > MAX_PACKAGE_ZIP {
        let _ = std::fs::remove_file(&zip_path);
        return Err(package_too_big(zipped));
    }
    let bytes = std::fs::read(&zip_path).map_err(|e| e.to_string())?;
    let pkg = Package {
        file: format!("{stem}.zip"),
        size: bytes.len() as u64,
        md5: md5_hex(&bytes),
        sha256: Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect(),
        contents,
        screenshots: shot_names,
        item,
        warnings: r.warnings.clone(),
    };
    // kept for registering later
    let _ = std::fs::write(out_dir.join(format!("{stem}.json")), serde_json::to_vec_pretty(&pkg).unwrap_or_default());
    Ok(pkg)
}

/// What to type into GameBanana's submit form.
pub fn gamebanana_form(p: &Package) -> Value {
    let it = &p.item;
    if it.kind == Kind::Prefab {
        let mut text = format!("{}\n\nA map editor prefab made with Loner-Strike (lonerstrike.com): open Tools > Community in the app and click Try it to add it to your prefab library. prefabs/*.map is a plain Valve 220 .map for any GoldSrc editor.\n", it.summary);
        if !it.credits.is_empty() {
            let who: Vec<String> = it.credits.iter().flat_map(|c| c.authors.iter().map(|a| a.name.clone())).collect::<std::collections::BTreeSet<_>>().into_iter().take(8).collect();
            text.push_str(&format!("\n- Uses assets by {}", who.join(", ")));
        }
        return json!({
            "credits": super::credits::gamebanana_rows(&it.credits),
            "submitUrl": "https://gamebanana.com/mods/add", "game": "Counter-Strike 1.6", "category": "Prefabs",
            "title": it.name, "text": text, "tag": {"title": TAG_TITLE, "value": TAG_VALUE}, "file": p.file, "md5": p.md5,
        });
    }
    if it.kind == Kind::Mode {
        let mut text = format!("{}\n\nA Game Mode Studio mode made with Loner-Strike (lonerstrike.com): open Tools > Community in the app and click Try it; the app builds it on your computer and you can play it with bots in one click. Nothing in the download runs code by itself: it's the mode's graph.\n", it.summary);
        if let Some(r) = &it.remix_of {
            text.push_str(&format!("\n- A remix of {} by {} (https://gamebanana.com/{}/{})", r.name, r.author, if r.model == Model::Spray { "sprays" } else { "mods" }, r.gb));
        }
        let bots = match it.bots.as_str() {
            "great" => "Bots play this well",
            "ok" => "Bots are so-so",
            "weak" => "Best with a friend",
            "none" => "Solo, no bots",
            _ => "",
        };
        if !bots.is_empty() {
            text.push_str(&format!("\n- Bots: {bots}"));
        }
        if let Some(m) = it.mode.as_ref().filter(|m| !m.maps.is_empty()) {
            text.push_str(&format!("\n- Made for: {}", m.maps.join(", ")));
        }
        return json!({
            "submitUrl": "https://gamebanana.com/mods/add", "game": "Counter-Strike 1.6", "category": "Resources > AMXX Plugins & Mods",
            "title": it.name, "text": text, "tag": {"title": TAG_TITLE, "value": TAG_VALUE}, "file": p.file, "md5": p.md5,
        });
    }
    let m = it.map.clone().unwrap_or_default();
    let mut text = format!("{}\n\nMade with Loner-Strike (lonerstrike.com): open Tools > Community in the app and click Try it to play it with bots in one click.\n", it.summary);
    if m.nav {
        text.push_str(if m.lsv { "\n- Works with bots: ships with a nav and finished bot map knowledge" } else { "\n- Ships with a bot nav" });
        if m.places {
            text.push_str(" (places named)");
        }
    }
    if !m.modes.is_empty() {
        text.push_str(&format!("\n- Game modes: {}", m.modes.join(", ")));
    }
    if let Some(h) = &it.health {
        text.push_str(&format!("\n- Loner-Strike map simulation: {} ({} bots, {} min)", h.verdict, h.bots, h.minutes));
    }
    text.push_str("\n\nInstall by hand: copy the folders into cstrike (or czero).");
    let category = match m.modes.first().map(|x| x.as_str()) {
        Some("surf") => "Maps > Surf",
        Some("deathrun") => "Maps > Deathrun",
        Some("hns") => "Maps > Hide and Seek",
        Some("jailbreak") => "Maps > Jailbreak",
        Some("zombie") | Some("zp5") => "Maps > Zombie",
        Some("gungame") => "Maps > Gun Game",
        Some("kreedz") => "Maps > Climb",
        _ if m.name.starts_with("de_") => "Maps > Bomb/Defuse",
        _ if m.name.starts_with("cs_") => "Maps > Hostage Rescue",
        _ if m.name.starts_with("aim_") => "Maps > Aim",
        _ if m.name.starts_with("fy_") => "Maps > Fight Yard",
        _ => "Maps",
    };
    if !it.credits.is_empty() {
        let who: Vec<String> = it.credits.iter().flat_map(|c| c.authors.iter().map(|a| a.name.clone())).collect::<std::collections::BTreeSet<_>>().into_iter().take(8).collect();
        text.push_str(&format!("\n- Uses assets by {}", who.join(", ")));
    }
    json!({
        "credits": super::credits::gamebanana_rows(&it.credits),
        "submitUrl": "https://gamebanana.com/mods/add",
        "game": "Counter-Strike 1.6",
        "category": category,
        "title": it.name,
        "text": text,
        "tag": {"title": TAG_TITLE, "value": TAG_VALUE},
        "file": p.file,
        "md5": p.md5,
    })
}

/// A Game Mode Studio mode, ready to share? The graph must pass the Studio's checks; a remix
/// carries its credit.
pub fn mode_readiness(graphs: &Path, lib: &Path, id: &str) -> Result<Value, String> {
    let g = read_studio_graph(graphs, id)?;
    let problems = super::studio::graph::validate(&g);
    let blockers: Vec<String> = problems.iter().filter(|p| p.level == "error").map(|p| p.message.clone()).take(5).collect();
    let credit = super::communitylib::credits(lib).get(id).cloned();
    Ok(json!({"id": g.mode.id, "name": g.mode.name, "description": g.mode.description, "nodes": g.nodes.len(), "blockers": blockers, "remixOf": credit,
              "botsSuggested": graph_bots(graphs, id)}))
}

/// The bot rating the Studio suggests for a graph (`mode.bots` in the graph file, when it says one).
pub fn graph_bots(graphs: &Path, id: &str) -> String {
    let p = graphs.join(format!("{id}.graph.json"));
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX) > 2 << 20 {
        return String::new();
    }
    std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| clean_bots(v["mode"]["bots"].as_str().unwrap_or(""))).unwrap_or_default()
}

/// The Studio's map kit for a mode (`<graphs>/<id>.kit.json`), when it's a kit for that mode.
fn mode_kit(graphs: &Path, id: &str) -> Option<Vec<u8>> {
    let p = graphs.join(format!("{id}.kit.json"));
    if std::fs::metadata(&p).ok()?.len() > 256 << 10 {
        return None;
    }
    let b = std::fs::read(&p).ok()?;
    let v: Value = serde_json::from_slice(&b).ok()?;
    (v["schema"] == "loner-strike/mode-kit" && v["mode"]["id"] == id).then_some(b)
}

/// A Studio graph's permanent id (`mode.uid`), read from the file itself.
pub fn graph_uid(graphs: &Path, id: &str) -> String {
    let p = graphs.join(format!("{id}.graph.json"));
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX) > 2 << 20 {
        return String::new();
    }
    std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| clean_uid(v["mode"]["uid"].as_str().unwrap_or(""))).unwrap_or_default()
}

fn read_studio_graph(graphs: &Path, id: &str) -> Result<super::studio::graph::Graph, String> {
    if !super::studio::graph::valid_mode_id(id) {
        return Err("That isn't one of your Game Mode Studio modes.".into());
    }
    let p = graphs.join(format!("{id}.graph.json"));
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX) > 2 << 20 {
        return Err("That isn't one of your Game Mode Studio modes.".into());
    }
    super::studio::graph::parse(&std::fs::read_to_string(&p).map_err(|_| "That isn't one of your Game Mode Studio modes.".to_string())?)
}

/// Builds the upload package for a Studio mode: `modes/<id>/<id>.graph.json` (only the graph:
/// each player's app builds the code), README.txt and loner-strike.json; screenshots beside it.
pub fn build_mode_package(graphs: &Path, lib: &Path, id: &str, req: &PublishReq, out_dir: &Path, needs: Option<&Value>) -> Result<Package, String> {
    let g = read_studio_graph(graphs, id)?;
    if super::studio::graph::has_errors(&super::studio::graph::validate(&g)) {
        return Err("The graph has problems. Fix them in Game Mode Studio first.".into());
    }
    let title = one_line(&req.title, 80);
    if title.is_empty() {
        return Err("Give it a name.".into());
    }
    let author = one_line(&req.author, 40);
    if author.is_empty() {
        return Err("Say who made it (your GameBanana name).".into());
    }
    if req.screenshots.len() > MAX_SHOTS {
        return Err(format!("At most {MAX_SHOTS} screenshots."));
    }
    let shots = req.screenshots.iter().map(decode_shot).collect::<Result<Vec<_>, _>>()?;
    let maps: Vec<String> = req.maps.iter().map(|m| m.trim().to_string()).filter(|m| crate::maps::safe_arg(m) && m.len() <= 31).take(24).collect();
    let uid = graph_uid(graphs, id);
    let item = Item {
        kind: Kind::Mode,
        uid: uid.clone(),
        name: title,
        author,
        summary: one_line(&req.summary, 400),
        mode: Some(ModeMeta {
            id: g.mode.id.clone(),
            uid,
            plugins: vec![],
            review: Review::None,
            maps,
            capabilities: super::studio::graph::capabilities(&g).iter().map(|c| c.to_string()).collect(),
            requirements: needs.map(|r| {
                let mut m = ModeRequirements::from_report(r);
                m.clean();
                m
            }),
        }),
        remix_of: super::communitylib::credits(lib).get(id).cloned(),
        bots: clean_bots(&req.bots),
        added: now(),
        updated: now(),
        ..Default::default()
    };
    let graph_text = serde_json::to_string_pretty(&g).map_err(|e| e.to_string())?;
    let mut readme = format!("{}\r\nby {}\r\n\r\n{}\r\n\r\nA Game Mode Studio mode made with Loner-Strike (lonerstrike.com).\r\nWith Loner-Strike: find it under Tools > Community and click Try it; the app builds it on your computer.\r\n", item.name, item.author, item.summary);
    if let Some(r) = &item.remix_of {
        readme.push_str(&format!("A remix of {} by {}.\r\n", r.name, r.author));
    }
    let manifest = json!({"schema": SCHEMA, "app": env!("CARGO_PKG_VERSION"), "item": item});
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        (format!("modes/{id}/{id}.graph.json"), graph_text.into_bytes()),
        ("README.txt".into(), readme.into_bytes()),
        ("loner-strike.json".into(), serde_json::to_vec_pretty(&manifest).unwrap_or_default()),
    ];
    // the mode's map kit, when the Studio made one (docs/notes/X.md: kits travel beside the graph)
    if let Some(kit) = mode_kit(graphs, id) {
        entries.insert(1, (format!("modes/{id}/{id}.kit.json"), kit));
    }
    let contents: Vec<String> = entries.iter().map(|e| e.0.clone()).collect();
    if let Some(why) = package_refusal(&item, &contents) {
        return Err(why);
    }
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Couldn't make the packages folder ({e})."))?;
    let stem = format!("{id}-mode-loner-strike");
    let zip_path = out_dir.join(format!("{stem}.zip"));
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in &entries {
            z.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(body).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    let bytes = buf.into_inner();
    crate::sys::write_atomic(&zip_path, &bytes).map_err(|e| format!("Couldn't save the package ({e})."))?;
    let shot_dir = out_dir.join(format!("{stem}-screenshots"));
    let _ = std::fs::remove_dir_all(&shot_dir);
    let mut shot_names = Vec::new();
    if !shots.is_empty() {
        std::fs::create_dir_all(&shot_dir).map_err(|e| format!("Couldn't save the screenshots ({e})."))?;
        for (i, (b, ext)) in shots.iter().enumerate() {
            let n = format!("{id}-{}.{ext}", i + 1);
            std::fs::write(shot_dir.join(&n), b).map_err(|e| format!("Couldn't save the screenshots ({e})."))?;
            shot_names.push(n);
        }
    }
    let pkg = Package {
        file: format!("{stem}.zip"),
        size: bytes.len() as u64,
        md5: md5_hex(&bytes),
        sha256: Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect(),
        contents,
        screenshots: shot_names,
        item,
        warnings: needs.map(|r| r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(String::from)).collect()).unwrap_or_default(),
    };
    let _ = std::fs::write(out_dir.join(format!("{stem}.json")), serde_json::to_vec_pretty(&pkg).unwrap_or_default());
    Ok(pkg)
}

/// A prefab of your library, ready to share? Its details and the credits of the assets it uses.
pub fn prefab_readiness(game: Option<&crate::game::Game>, id: &str) -> Result<Value, String> {
    let dir = super::editor::prefabs::lib_dir(&crate::sys::app_dir().join("editor"));
    if !id.starts_with("mine:") {
        return Err("Only prefabs you made (or added) can be published, not the starter ones.".into());
    }
    let (frag, meta) = super::editor::prefabs::load(&dir, id)?;
    let credits = game.map(|g| super::editor::prefabs::prefab_credits(&g.hl, if g.is_czero() { "czero" } else { "cstrike" }, &frag, &meta)).unwrap_or_default();
    Ok(json!({"id": id, "name": meta.name, "description": meta.description, "author": meta.author, "tags": meta.tags,
              "brushes": meta.brushes, "entities": meta.entities, "credits": credits, "blockers": []}))
}

/// Builds the upload package for a prefab (the editor's own package format, with its credits) in
/// the packages folder, plus the details registering needs.
pub fn build_prefab_package(game: Option<&crate::game::Game>, id: &str, req: &PublishReq, out_dir: &Path) -> Result<Package, String> {
    let dir = super::editor::prefabs::lib_dir(&crate::sys::app_dir().join("editor"));
    if !id.starts_with("mine:") {
        return Err("Only prefabs you made (or added) can be published.".into());
    }
    let (frag, meta) = super::editor::prefabs::load(&dir, id)?;
    let title = one_line(&req.title, 80);
    let author = one_line(&req.author, 40);
    if title.is_empty() || author.is_empty() {
        return Err("Give it a name and say who made it (your GameBanana name).".into());
    }
    let credits = game.map(|g| super::editor::prefabs::prefab_credits(&g.hl, if g.is_czero() { "czero" } else { "cstrike" }, &frag, &meta));
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Couldn't make the packages folder ({e})."))?;
    let slug = id.trim_start_matches("mine:");
    let stem = format!("{slug}-prefab-loner-strike");
    let zip_path = out_dir.join(format!("{stem}.zip"));
    super::editor::prefabs::package(&dir, id, &zip_path, credits.clone())?;
    let bytes = std::fs::read(&zip_path).map_err(|e| e.to_string())?;
    let contents: Vec<String> = zip::ZipArchive::new(std::io::Cursor::new(&bytes))
        .map(|mut z| (0..z.len()).filter_map(|i| z.by_index(i).ok().and_then(|f| f.name().ok().map(|n| n.to_string()))).collect())
        .unwrap_or_default();
    let item = Item {
        kind: Kind::Prefab,
        name: title,
        author,
        summary: one_line(&req.summary, 400),
        credits: super::credits::clean_credits(credits.unwrap_or_default()),
        tags: meta.tags.iter().map(|t| one_line(t, 24)).take(12).collect(),
        added: now(),
        updated: now(),
        ..Default::default()
    };
    if let Some(why) = package_refusal(&item, &contents) {
        let _ = std::fs::remove_file(&zip_path);
        return Err(why);
    }
    let pkg = Package {
        file: format!("{stem}.zip"),
        size: bytes.len() as u64,
        md5: md5_hex(&bytes),
        sha256: Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect(),
        contents,
        screenshots: vec![],
        item,
        warnings: vec![],
    };
    let _ = std::fs::write(out_dir.join(format!("{stem}.json")), serde_json::to_vec_pretty(&pkg).unwrap_or_default());
    Ok(pkg)
}

// ---- registering ------------------------------------------------------------------------------

/// A GameBanana page link: (item type, id).
pub fn parse_gb_link(url: &str) -> Option<(Model, i64)> {
    let u = url.trim();
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(u);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let rest = rest.strip_prefix("gamebanana.com/")?;
    let mut parts = rest.split(['/', '?', '#']);
    let model = Model::parse(parts.next()?)?;
    let id: i64 = parts.next()?.parse().ok().filter(|i| *i > 0 && *i < 100_000_000)?;
    Some((model, id))
}

/// Checks a GameBanana page against the package: the tag, the game, and one file with the
/// package's MD5. Returns the entry for the index.
pub fn check_page(page: &Value, model: Model, id: i64, pkg: &Package) -> Result<Item, String> {
    if let Some(why) = package_refusal(&pkg.item, &pkg.contents) {
        return Err(why);
    }
    if page["_aGame"]["_idRow"].as_i64().is_some_and(|g| g != CS16_GAME) {
        return Err("That page isn't in GameBanana's Counter-Strike 1.6 section.".into());
    }
    if !has_tag(&page["_aTags"]) {
        return Err(format!("Add the tag \"{TAG_TITLE}: {TAG_VALUE}\" to the page (Edit > Attributes), then try again."));
    }
    let files = files_of(page);
    let Some(f) = files.iter().find(|f| f["md5"].as_str() == Some(pkg.md5.as_str())) else {
        return Err("None of the page's files is the package the app made. Upload that zip (unchanged), wait for GameBanana to check it, then try again.".into());
    };
    let mut it = pkg.item.clone();
    it.model = model;
    it.gb = id;
    it.file = f["id"].as_i64().unwrap_or(0);
    it.md5 = pkg.md5.clone();
    let author = one_line(&s(&page["_aSubmitter"]["_sName"]), 40);
    if !author.is_empty() {
        it.author = author;
    }
    it.screenshots = page["_aPreviewMedia"]["_aImages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let u = format!("{}/{}", s(&i["_sBaseUrl"]), s(&i["_sFile"]));
            safe_shot(&u).then_some(u)
        })
        .take(MAX_SHOTS)
        .collect();
    it.status = Status::Listed;
    Ok(it)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Mine {
    pub item: Item,
    pub url: String,
    /// listed (the list took it and lists it), pending (a moderator looks first), refused (the
    /// list said no: `error` says why; not sent again), waiting (the list didn't answer: kept
    /// here, sent next time); "sent" from older versions means listed or pending
    pub state: String,
    pub when: u64,
    /// why the list refused it
    pub error: String,
}

fn mine_path() -> PathBuf {
    dir().join("mine.json")
}

fn load_mine(p: &Path) -> Vec<Mine> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_mine(p: &Path, list: &[Mine]) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = crate::sys::write_atomic(p, &serde_json::to_vec_pretty(list).unwrap_or_default());
}

/// Puts a registration in the list (one per page; the newest wins).
pub fn remember(list: &mut Vec<Mine>, m: Mine) {
    list.retain(|x| !(x.item.model == m.item.model && x.item.gb == m.item.gb));
    list.insert(0, m);
    list.truncate(200);
}

/// What the list said to a registration.
#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    /// "listed" or "pending"
    Took(String),
    /// a 4xx other than 429: final, with the list's reason
    Refused(String),
    /// no answer, 429 or 5xx: tried again later
    Waiting,
}

/// Reads the list service's answer (status, body) to a registration.
pub fn sent_from(status: u16, body: &[u8]) -> Sent {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    match status {
        200..=299 => Sent::Took(match v["state"].as_str() {
            Some("pending") => "pending".into(),
            _ => "listed".into(),
        }),
        429 => Sent::Waiting,
        400..=499 => Sent::Refused(v["error"].as_str().map(|e| one_line(e, 300)).filter(|e| !e.is_empty()).unwrap_or_else(|| "The Loner-Strike list turned it down.".into())),
        _ => Sent::Waiting,
    }
}

/// The registration the list service takes: which page and which file, nothing more.
pub fn submit_body(item: &Item) -> Value {
    json!({"schema": SCHEMA, "app": env!("CARGO_PKG_VERSION"), "page": {"model": item.model, "id": item.gb, "file": item.file}})
}

fn send(item: &Item) -> Sent {
    if let Some(why) = package_refusal(item, &[]) {
        return Sent::Refused(why);
    }
    // only which page and file: the list reads everything else from GameBanana and from the
    // package's own loner-strike.json, never from what an app says
    let body = submit_body(item);
    let r = crate::sys::http()
        .post(SUBMIT_URL)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(30))
        .body(body.to_string())
        .send();
    match r {
        Ok(r) => {
            let status = r.status().as_u16();
            let mut out = Vec::new();
            let _ = r.take(16 << 10).read_to_end(&mut out);
            sent_from(status, &out)
        }
        Err(_) => Sent::Waiting,
    }
}

/// Puts the list's answer on a remembered registration.
fn mark(m: &mut Mine, sent: &Sent) {
    match sent {
        Sent::Took(state) => {
            m.state = state.clone();
            m.error.clear();
        }
        Sent::Refused(e) => {
            m.state = "refused".into();
            m.error = e.clone();
        }
        Sent::Waiting => m.state = "waiting".into(),
    }
}

#[derive(Deserialize)]
struct RegisterReq {
    url: String,
    /// the package's zip name (from publish); the newest package with a matching MD5 otherwise
    #[serde(default)]
    file: String,
}

fn packages_dir() -> PathBuf {
    crate::localdata::join(&dir(), "packages")
}

fn load_packages(d: &Path) -> Vec<Package> {
    let mut v: Vec<Package> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice::<Package>(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    v.sort_by_key(|p| std::cmp::Reverse(p.item.added));
    v
}

async fn api_register(h: HeaderMap, Json(req): Json<RegisterReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let fail = |m: String| Fail::new(StatusCode::OK, m);
            let (model, id) = parse_gb_link(&req.url).ok_or_else(|| fail("Paste the link of your GameBanana page (gamebanana.com/mods/…).".into()))?;
            let page = profile(model, id).map_err(fail)?;
            let pkgs = load_packages(&packages_dir());
            if pkgs.is_empty() {
                return Err(fail("Make the package first (Publish), upload it, then paste the link.".into()));
            }
            let mds: Vec<String> = files_of(&page).iter().filter_map(|f| f["md5"].as_str().map(String::from)).collect();
            let pkg = pkgs
                .iter()
                .find(|p| !req.file.is_empty() && p.file == req.file && mds.contains(&p.md5))
                .or_else(|| pkgs.iter().find(|p| mds.contains(&p.md5)))
                .or_else(|| pkgs.iter().find(|p| p.file == req.file))
                .unwrap_or(&pkgs[0]);
            let item = match check_page(&page, model, id, pkg) {
                Ok(it) => it,
                // a Studio mode can be made again without plugins: the page offers to open the Studio
                Err(e) if e == PLUGIN_REFUSAL => return Ok(json!({"error": e, "studio": true})),
                Err(e) => return Err(fail(e)),
            };
            let sent = send(&item);
            let mut list = load_mine(&mine_path());
            let mut entry = Mine { item: item.clone(), url: model.url(id), when: now(), ..Default::default() };
            mark(&mut entry, &sent);
            remember(&mut list, entry.clone());
            save_mine(&mine_path(), &list);
            Ok(json!({"ok": true, "state": entry.state, "error": if entry.error.is_empty() { Value::Null } else { json!(entry.error) }, "item": item,
                      "copy": serde_json::to_string_pretty(&json!({"schema": SCHEMA, "item": item})).unwrap_or_default()}))
        })
        .await,
    )
}

async fn api_mine() -> Response {
    json_answer(
        blocking(|| {
            let p = mine_path();
            let mut list = load_mine(&p);
            // anything still waiting gets another go
            let mut changed = false;
            for m in list.iter_mut().filter(|m| m.state == "waiting").take(5) {
                let sent = send(&m.item);
                if sent != Sent::Waiting {
                    mark(m, &sent);
                    changed = true;
                }
            }
            if changed {
                save_mine(&p, &list);
            }
            let pkgs = load_packages(&packages_dir());
            Ok(json!({"mine": list, "packages": pkgs.iter().map(|p| json!({"file": p.file, "name": p.item.name, "size": p.size, "md5": p.md5, "made": p.item.added, "map": p.item.map.as_ref().map(|m| m.name.clone())})).collect::<Vec<_>>()}))
        })
        .await,
    )
}

async fn api_ready(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let r = readiness(&m, &crate::sys::app_dir().join("tools").join("health"));
                let presets = crate::sys::read_latin1(&game.dir.join("addons/amxmodx/configs/gamemodes/mappresets.ini")).unwrap_or_default();
                // the modes the map says it's made for (worldspawn ls_modes, set in the map editor) first
                let mut suggested: Vec<String> = Vec::new();
                for x in made_for(&m.bsp).into_iter().chain(super::health::preset_mode(&presets, &m.name)).chain(r.health.as_ref().map(|h| h.mode.clone())) {
                    if !x.is_empty() && !suggested.contains(&x) {
                        suggested.push(x);
                    }
                }
                let choices = mode_choices(&game.dir, &super::studio::graphs_dir(), &super::communitylib::load_all(&super::communitylib::lib_dir()));
                Ok(json!({"ready": r, "suggestedModes": suggested, "modeChoices": choices, "tag": format!("{TAG_TITLE}: {TAG_VALUE}")}))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_publish(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>, Json(req): Json<PublishReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let r = readiness(&m, &crate::sys::app_dir().join("tools").join("health"));
                let pkg = build_package(&m, &r, &req, &packages_dir()).map_err(|e| Fail::new(StatusCode::OK, e))?;
                Ok(json!({"package": pkg, "form": gamebanana_form(&pkg)}))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_prefab_ready(State(st): State<Shared>, UrlPath(id): UrlPath<String>) -> Response {
    let game = st.local_game();
    json_answer(blocking(move || prefab_readiness(game.as_deref(), &id).map_err(|e| Fail::new(StatusCode::OK, e))).await)
}

async fn api_prefab_publish(State(st): State<Shared>, h: HeaderMap, UrlPath(id): UrlPath<String>, Json(req): Json<PublishReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game();
    json_answer(
        blocking(move || {
            let pkg = build_prefab_package(game.as_deref(), &id, &req, &packages_dir()).map_err(|e| Fail::new(StatusCode::OK, e))?;
            Ok(json!({"package": pkg, "form": gamebanana_form(&pkg)}))
        })
        .await,
    )
}

async fn api_mode_ready(State(st): State<Shared>, UrlPath(id): UrlPath<String>) -> Response {
    json_answer(
        blocking(move || {
            let mut v = mode_readiness(&super::studio::graphs_dir(), &super::communitylib::lib_dir(), &id).map_err(|e| Fail::new(StatusCode::OK, e))?;
            // the mode's requirements: files that can't be found anywhere stop it; the rest are warnings
            if let Ok(r) = super::requirements::report(&st, &id) {
                if let Some(b) = v["blockers"].as_array_mut() {
                    for p in r["missing"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        b.push(json!(format!("{p} isn't on this computer and isn't linked to an item that has it. Link it in My mode › Details › Requirements, or remove what uses it.")));
                    }
                }
                v["requirementWarnings"] = r["warnings"].clone();
                v["requirements"] = r;
            }
            Ok(v)
        })
        .await,
    )
}

async fn api_mode_publish(State(st): State<Shared>, h: HeaderMap, UrlPath(id): UrlPath<String>, Json(req): Json<PublishReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let needs = super::requirements::report(&st, &id).ok();
            if let Some(missing) = needs.as_ref().and_then(|r| r["missing"].as_array()).filter(|m| !m.is_empty()) {
                let list: Vec<&str> = missing.iter().filter_map(Value::as_str).collect();
                return Err(Fail::new(StatusCode::OK, format!("Not on this computer and not linked to an item that has them: {}. Link them in My mode › Details › Requirements first.", list.join(", "))));
            }
            let pkg = build_mode_package(&super::studio::graphs_dir(), &super::communitylib::lib_dir(), &id, &req, &packages_dir(), needs.as_ref()).map_err(|e| Fail::new(StatusCode::OK, e))?;
            Ok(json!({"package": pkg, "form": gamebanana_form(&pkg)}))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct RevealReq {
    file: String,
}

async fn api_reveal(h: HeaderMap, Json(req): Json<RevealReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let ok_name = !req.file.is_empty() && req.file.len() < 80 && req.file.ends_with(".zip") && req.file.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-$".contains(&c)) && !req.file.contains("..");
    let p = packages_dir().join(&req.file);
    if !ok_name || !p.is_file() {
        return json_answer(Err(Fail::new(StatusCode::OK, "That package isn't there any more.")));
    }
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(&p).spawn().map(crate::sys::reap);
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn().map(crate::sys::reap);
    #[cfg(not(any(target_os = "macos", windows)))]
    let r: std::io::Result<()> = Err(std::io::Error::other("unsupported"));
    json_answer(r.map(|_| json!({"ok": true})).map_err(|e| Fail::new(StatusCode::OK, format!("Couldn't show the folder ({e})."))))
}

#[derive(Deserialize)]
struct ReportReq {
    model: String,
    id: i64,
    reason: String,
    #[serde(default)]
    note: String,
}

/// The report the list service is sent: the page, a reason, an optional note and the app's
/// version; nothing about the player.
pub fn report_body(model: &str, id: i64, reason: &str, note: &str) -> Result<Value, String> {
    let model = Model::parse(model).filter(|_| id > 0 && id < 100_000_000).ok_or("That isn't a GameBanana page.")?;
    if !REPORT_REASONS.contains(&reason) {
        return Err("Pick what's wrong with it.".into());
    }
    if note.chars().count() > MAX_NOTE {
        return Err(format!("Keep the note under {MAX_NOTE} characters."));
    }
    Ok(json!({"schema": 1, "app": env!("CARGO_PKG_VERSION"), "model": model, "id": id, "reason": reason, "note": one_line(note, MAX_NOTE)}))
}

async fn api_report(h: HeaderMap, Json(req): Json<ReportReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let fail = |m: String| Fail::new(StatusCode::OK, m);
            let body = report_body(&req.model, req.id, &req.reason, &req.note).map_err(fail)?;
            let r = crate::sys::http()
                .post(REPORT_URL)
                .header("User-Agent", USER_AGENT)
                .header("Content-Type", "application/json")
                .timeout(Duration::from_secs(15))
                .body(body.to_string())
                .send()
                .map_err(|_| fail("Couldn't reach the Loner-Strike list. Check your internet connection, or tell us in the Discord.".into()))?;
            let status = r.status();
            let mut out = Vec::new();
            let _ = r.take(16 << 10).read_to_end(&mut out);
            if status.is_success() {
                return Ok(json!({"ok": true}));
            }
            let v: Value = serde_json::from_slice(&out).unwrap_or(Value::Null);
            Err(fail(v["error"].as_str().map(|e| one_line(e, 300)).unwrap_or_else(|| "The Loner-Strike list couldn't take the report right now. Try again later.".into())))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct UnlistReq {
    key: String,
    /// "unlist" or "list"
    action: String,
}

/// The author's Unlist / List again for an entry in Yours: sent to the list service, which
/// answers with a code to put on the GameBanana page first, then does it once the page shows it.
async fn api_unlist(h: HeaderMap, Json(req): Json<UnlistReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let fail = |m: String| Fail::new(StatusCode::OK, m);
            let ok_key = req.key.split_once(':').is_some_and(|(m, id)| matches!(m, "mod" | "spray") && id.parse::<i64>().is_ok_and(|i| i > 0));
            if !ok_key || !matches!(req.action.as_str(), "unlist" | "list") {
                return Err(fail("That isn't one of your creations.".into()));
            }
            let r = crate::sys::http()
                .post(UNLIST_URL)
                .header("User-Agent", USER_AGENT)
                .header("Content-Type", "application/json")
                .timeout(Duration::from_secs(20))
                .body(json!({"schema": 1, "key": req.key, "action": req.action}).to_string())
                .send()
                .map_err(|_| fail("Couldn't reach the Loner-Strike list. Check your internet connection, or tell us in the Discord.".into()))?;
            let status = r.status();
            let mut out = Vec::new();
            let _ = r.take(16 << 10).read_to_end(&mut out);
            let v: Value = serde_json::from_slice(&out).unwrap_or(Value::Null);
            if status.is_success() {
                // (only the fields the page needs, each a short line)
                return Ok(json!({"state": v["state"].as_str().map(|x| one_line(x, 20)), "code": v["code"].as_str().map(|x| one_line(x, 40)), "how": v["how"].as_str().map(|x| one_line(x, 400)), "note": v["note"].as_str().map(|x| one_line(x, 300))}));
            }
            Err(fail(v["error"].as_str().map(|e| one_line(e, 300)).unwrap_or_else(|| "The Loner-Strike list couldn't do that right now. Try again later.".into())))
        })
        .await,
    )
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/community/browse", get(api_browse))
        .route("/api/tools/community/item/:model/:id", get(api_item))
        .route("/api/tools/community/try", post(api_try))
        // [Caps] up to MAX_SHOTS screenshots of MAX_SHOT each, as base64 (axum's 2 MB default refused real ones)
        .route("/api/tools/community/publish/:map", get(api_ready).post(api_publish).layer(axum::extract::DefaultBodyLimit::max(PUBLISH_BODY)))
        .route("/api/tools/community/publish-mode/:id", get(api_mode_ready).post(api_mode_publish))
        .route("/api/tools/community/publish-prefab/:id", get(api_prefab_ready).post(api_prefab_publish))
        .route("/api/tools/community/register", post(api_register))
        .route("/api/tools/community/mine", get(api_mine))
        .route("/api/tools/community/reveal", post(api_reveal))
        .route("/api/tools/community/report", post(api_report))
        .route("/api/tools/community/unlist", post(api_unlist))
}

// ---- MD5 (GameBanana lists each file's MD5; the package is matched by it) ---------------------------

pub fn md5_hex(data: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32).collect();
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = chunk.chunks(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f2 = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f2.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    [a0, b0, c0, d0].iter().flat_map(|x| x.to_le_bytes()).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::{bsp::fixture, nav, wad};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-community-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const INDEX: &str = r#"{
      "schema": 1, "updated": 1794000000,
      "revoked": ["Mod:900"],
      "items": [
        {"kind": "map", "gb": 101, "name": "de_lonely", "author": "jam", "summary": "A quiet bomb map",
         "file": 5, "md5": "ABCDEFABCDEFABCDEFABCDEFABCDEFAB",
         "map": {"name": "de_lonely", "bspSize": 1000, "modes": ["classic"], "nav": true, "lsv": true, "places": true},
         "health": {"verdict": "good", "bots": 12, "minutes": 5},
         "screenshots": ["https://images.gamebanana.com/img/ss/mods/a.jpg", "http://evil.example/x.jpg", "https://evil.example/y.jpg"],
         "featured": true},
        {"kind": "mode", "gb": 102, "name": "Floor is lava", "mode": {"id": "lava", "plugins": []}},
        {"kind": "mode", "gb": 103, "name": "Jetpacks", "mode": {"id": "jet", "review": "reviewed",
          "plugins": [{"file": "jet.amxx", "sha256": "0000000000000000000000000000000000000000000000000000000000000000", "source": "jet.sma"}]}},
        {"kind": "map", "gb": 104, "name": "bad name", "map": {"name": "de_x;quit"}},
        {"kind": "map", "gb": 101, "name": "duplicate"},
        {"kind": "rocket", "gb": 105, "name": "unknown kind"},
        {"kind": "map", "gb": -3, "name": "bad id"},
        {"kind": "spray", "model": "Spray", "gb": 101, "name": "a spray with the same number", "status": "hidden"},
        {"kind": "map", "gb": 106, "name": "pulled", "status": "revoked"},
        {"kind": "mode", "gb": 107, "name": "bad mode id", "mode": {"id": "x y"}},
        {"kind": "texture", "gb": 108, "name": "Bricks", "futureField": {"a": 1}}
      ]
    }"#;

    #[test]
    fn index_parsing_keeps_good_entries_and_drops_bad_ones() {
        let idx = parse_index(INDEX.as_bytes()).unwrap();
        let names: Vec<&str> = idx.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["de_lonely", "Floor is lava", "Jetpacks", "a spray with the same number", "pulled", "Bricks"]);
        let m = &idx.items[0];
        assert_eq!(m.md5, "abcdefabcdefabcdefabcdefabcdefab");
        assert_eq!(m.screenshots, ["https://images.gamebanana.com/img/ss/mods/a.jpg"], "only https from the allowed hosts");
        assert_eq!(m.map.as_ref().unwrap().modes, ["classic"]);
        assert_eq!(idx.revoked, ["mod:900"]);
        assert_eq!(idx.items[3].model, Model::Spray);
    }

    #[test]
    fn index_parsing_refuses_junk_and_newer_formats() {
        assert!(parse_index(b"not json").is_err());
        assert!(parse_index(b"{}").is_err(), "no schema");
        assert!(parse_index(br#"{"schema": 2, "items": []}"#).unwrap_err().contains("newer"));
        assert!(parse_index(&vec![b' '; (MAX_INDEX + 1) as usize]).is_err());
        assert_eq!(parse_index(br#"{"schema": 1}"#).unwrap().items.len(), 0);
    }

    #[test]
    fn index_must_match_its_checksum() {
        let body = br#"{"schema": 1, "items": []}"#.to_vec();
        let sum: String = Sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(checked_index(body.clone(), format!("{sum}  index.json\n").as_bytes()).unwrap(), body);
        assert_eq!(checked_index(body.clone(), sum.to_uppercase().as_bytes()).unwrap(), body);
        assert!(checked_index(body.clone(), format!("{}  index.json", "0".repeat(64)).as_bytes()).is_err());
        assert!(checked_index(body.clone(), b"<html>not found</html>").is_err());
        assert!(checked_index(body, b"").is_err());
        // a mismatch keeps the copy from before
        let d = tmp("sum");
        let cache = d.join("index.json");
        std::fs::write(&cache, br#"{"schema": 1, "items": [], "revoked": ["mod:1"]}"#).unwrap();
        let (idx, src) = load_index(&|| checked_index(br#"{"schema": 1, "items": []}"#.to_vec(), "0".repeat(64).as_bytes()), &cache);
        assert_eq!((src, idx.revoked), (Source::Cached, vec!["mod:1".to_string()]));
    }

    #[test]
    fn index_falls_back_to_the_kept_copy_then_to_nothing() {
        let d = tmp("cache");
        let cache = d.join("index.json");
        let (_, src) = load_index(&|| Err("offline".into()), &cache);
        assert_eq!(src, Source::None);
        let (idx, src) = load_index(&|| Ok(INDEX.as_bytes().to_vec()), &cache);
        assert_eq!((src, idx.items.len()), (Source::Live, 6));
        assert!(cache.is_file(), "a good copy is kept");
        let (idx, src) = load_index(&|| Err("offline".into()), &cache);
        assert_eq!((src, idx.items.len()), (Source::Cached, 6));
        // junk from the server doesn't replace the good copy
        let (idx, src) = load_index(&|| Ok(b"<html>502</html>".to_vec()), &cache);
        assert_eq!((src, idx.items.len()), (Source::Cached, 6));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn trust_rules() {
        let idx = parse_index(INDEX.as_bytes()).unwrap();
        let get = |gb: i64| idx.items.iter().find(|i| i.gb == gb && i.model == Model::Mod);
        // content installs whoever made it, listed or not
        let t = trust(Kind::Map, None, false);
        assert!(t.allowed && t.confirm.is_none() && t.level == "content");
        assert_eq!(trust(Kind::Map, get(101), false).level, "content");
        // pulled: never
        assert!(!trust(Kind::Map, get(101), true).allowed);
        assert!(!trust(Kind::Map, get(106), false).allowed);
        // a mode the index doesn't know isn't installed
        assert!(!trust(Kind::Mode, None, false).allowed);
        // data-only: one click
        let t = trust(Kind::Mode, get(102), false);
        assert!(t.allowed && t.confirm.is_none() && t.level == "data");
        // plugin modes: never from the community, whatever the index says about their review
        let mut it = get(103).unwrap().clone();
        for review in [Review::Reviewed, Review::Verified, Review::None] {
            it.mode.as_mut().unwrap().review = review;
            let t = trust(Kind::Mode, Some(&it), false);
            assert!(!t.allowed && t.level == "unreviewed" && t.confirm.is_none(), "{review:?}");
            assert!(t.note.contains("Game Mode Studio"));
        }
    }

    #[test]
    fn unknown_values_from_a_newer_index_keep_the_list() {
        let idx = parse_index(br#"{"schema": 1, "items": [
            {"kind": "map", "gb": 1, "name": "a", "status": "quarantined"},
            {"kind": "mode", "gb": 2, "name": "b", "mode": {"id": "m", "review": "safe", "plugins": [{"file": "x.amxx", "sha256": "", "source": ""}]}},
            {"kind": "map", "gb": 3, "name": "c"}]}"#).unwrap();
        assert_eq!(idx.items.len(), 3);
        assert_eq!(idx.items[0].status, Status::Hidden, "an unknown status keeps it out of lists");
        assert_eq!(idx.items[1].mode.as_ref().unwrap().review, Review::None);
        assert!(!trust(Kind::Mode, Some(&idx.items[1]), false).allowed);
    }

    #[test]
    fn nothing_that_runs_code_is_shared() {
        for ok in ["maps/a.bsp", "sound/x.wav", "addons/amxmodx/configs/gamemodes/a.ini", "modes/x/graph.json", "gfx/env/skyup.tga", "dir/"] {
            assert!(!code_file(ok), "{ok}");
        }
        for bad in ["addons/amxmodx/plugins/x.amxx", "SCRIPTING/X.SMA", "a.dll", "b.so", "c.dylib", "d.exe", "e.bat", "f.sh", "inner.zip", "x.rar",
                    "addons/metamod/plugins.ini", "cstrike/dlls/mp.dll", "tools\\run.ps1", "x.inc"] {
            assert!(code_file(bad), "{bad}");
        }
        let map = Item { kind: Kind::Map, ..Default::default() };
        assert_eq!(package_refusal(&map, &["maps/a.bsp".into()]), None);
        assert_eq!(package_refusal(&map, &["maps/a.bsp".into(), "addons/amxmodx/plugins/a.amxx".into()]).as_deref(), Some(PLUGIN_REFUSAL));
        assert!(package_refusal(&map, &["a.7z".into()]).unwrap().contains("a.7z"));
        let mut mode = Item { kind: Kind::Mode, mode: Some(ModeMeta { id: "m".into(), ..Default::default() }), ..Default::default() };
        assert_eq!(package_refusal(&mode, &[]), None, "a Game Mode Studio (settings only) mode");
        mode.mode.as_mut().unwrap().plugins.push(PluginMeta { file: "x.amxx".into(), ..Default::default() });
        assert_eq!(package_refusal(&mode, &[]).as_deref(), Some(PLUGIN_REFUSAL));
    }

    #[test]
    fn what_the_list_says_to_a_registration() {
        assert_eq!(sent_from(200, br#"{"ok":true,"state":"listed"}"#), Sent::Took("listed".into()));
        assert_eq!(sent_from(200, br#"{"ok":true,"state":"pending"}"#), Sent::Took("pending".into()));
        assert_eq!(sent_from(200, b""), Sent::Took("listed".into()));
        assert_eq!(sent_from(422, br#"{"error":"Add the tag."}"#), Sent::Refused("Add the tag.".into()));
        assert!(matches!(sent_from(409, b"<html>"), Sent::Refused(_)));
        for waiting in [429, 500, 502, 503] {
            assert_eq!(sent_from(waiting, br#"{"error":"busy"}"#), Sent::Waiting, "{waiting}");
        }
        let mut m = Mine::default();
        mark(&mut m, &Sent::Refused("no".into()));
        assert_eq!((m.state.as_str(), m.error.as_str()), ("refused", "no"));
        mark(&mut m, &Sent::Took("listed".into()));
        assert_eq!((m.state.as_str(), m.error.as_str()), ("listed", ""));
    }

    #[test]
    fn reports_carry_only_the_page_and_the_reason() {
        let b = report_body("mod", 12, "broken", "crashes\non load").unwrap();
        assert_eq!(b["model"], "Mod");
        assert_eq!((b["id"].as_i64(), b["reason"].as_str(), b["note"].as_str()), (Some(12), Some("broken"), Some("crashes on load")));
        let keys: Vec<&String> = b.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 6, "{keys:?}");
        assert!(report_body("spray", 3, "offensive", "").is_ok());
        assert!(report_body("wip", 3, "broken", "").is_err());
        assert!(report_body("mod", 0, "broken", "").is_err());
        assert!(report_body("mod", 3, "boring", "").is_err());
        assert!(report_body("mod", 3, "other", &"x".repeat(501)).is_err());
    }

    fn record(model: &str, id: i64, name: &str, tags: Value, root: &str, sub: &str, added: i64) -> Value {
        json!({"_sModelName": model, "_idRow": id, "_sName": name, "_aTags": tags, "_aGame": {"_idRow": 4254},
               "_aRootCategory": {"_sName": root}, "_aSubCategory": {"_sName": sub}, "_aSubmitter": {"_sName": "jam"},
               "_tsDateAdded": added, "_tsDateModified": added, "_nLikeCount": 3,
               "_aPreviewMedia": {"_aImages": [{"_sBaseUrl": "https://images.gamebanana.com/img/ss/mods", "_sFile": "a.jpg", "_sFile220": "220-a.jpg"}]}})
    }

    #[test]
    fn gamebanana_records_need_the_tag() {
        let page = json!({"_aRecords": [
            record("Mod", 101, "de_lonely", json!(["Software Used: LonerStrike"]), "Maps", "Bomb/Defuse", 10),
            record("Mod", 200, "de_other", json!(["Software Used: Hammer"]), "Maps", "Bomb/Defuse", 11),
            record("Mod", 201, "fence", json!(["software used : loner-strike, Hammer"]), "Prefabs", "Doors & Windows", 12),
            record("Spray", 7, "smile", json!(["Loner-Strike"]), "", "", 13),
            record("Mod", 202, "jetpack mode", json!(["Software Used: Loner-Strike"]), "Resources", "AMXX Plugins & Mods", 14),
            record("Wip", 203, "a wip", json!(["Software Used: Loner-Strike"]), "", "", 15),
        ]});
        let hits = hits_from_page(&page);
        let got: Vec<(i64, Kind)> = hits.iter().map(|h| (h.id, h.kind)).collect();
        assert_eq!(got, [(101, Kind::Map), (201, Kind::Prefab), (7, Kind::Spray), (202, Kind::Mode)]);
        assert_eq!(hits[0].image.as_deref(), Some("https://images.gamebanana.com/img/ss/mods/220-a.jpg"));
        // a page's tags are objects
        assert!(has_tag(&json!([{"_sTitle": "Software Used", "_sValue": "Loner-Strike"}])));
        assert!(!has_tag(&json!([{"_sTitle": "Software Used", "_sValue": "Loner-Striker"}])));
        assert!(!has_tag(&json!(["cs: counter strike", "Software Used: Hammer"])), "what the search also finds");
        // another game
        let mut r = record("Mod", 300, "x", json!(["Software Used: Loner-Strike"]), "Maps", "", 1);
        r["_aGame"]["_idRow"] = json!(1);
        assert!(hit_from_record(&r).is_none());
    }

    #[test]
    fn merged_list() {
        let idx = parse_index(INDEX.as_bytes()).unwrap();
        let hits = hits_from_page(&json!({"_aRecords": [
            record("Mod", 101, "de_lonely (GB name)", json!(["Software Used: Loner-Strike"]), "Maps", "Bomb/Defuse", 10),
            record("Mod", 900, "pulled by the team", json!(["Software Used: Loner-Strike"]), "Maps", "", 50),
            record("Mod", 201, "fence", json!(["Software Used: Loner-Strike"]), "Prefabs", "Other/Misc", 40),
            record("Mod", 202, "unknown mode", json!(["Software Used: Loner-Strike"]), "Resources", "AMXX Plugins & Mods", 30),
        ]}));
        let have: HashSet<String> = ["de_lonely".to_string()].into();
        let all = merge(&idx, &hits, &have, None, "", "new");
        let ids: Vec<i64> = all.iter().map(|v| v["id"].as_i64().unwrap()).collect();
        // featured first, then listed ones, then GameBanana-only; pulled, hidden and revoked never
        assert_eq!(ids[0], 101);
        assert!(!ids.contains(&900) && !ids.contains(&106));
        assert!(all.iter().all(|v| v["name"] != "a spray with the same number"));
        let first = &all[0];
        assert_eq!(first["name"], "de_lonely", "the index's name wins");
        assert_eq!(first["have"], true);
        assert_eq!(first["canTry"], true);
        let b: Vec<&str> = first["badges"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        assert_eq!(b, ["featured", "bots", "checked"]);
        let unknown = all.iter().find(|v| v["id"] == 202).unwrap();
        assert_eq!((unknown["canTry"].clone(), unknown["listed"].clone()), (json!(false), json!(false)));
        let fence = all.iter().find(|v| v["id"] == 201).unwrap();
        assert_eq!(fence["canTry"], true, "prefabs install into the editor's library");
        // filters
        assert_eq!(merge(&idx, &hits, &have, Some(Kind::Mode), "", "new").len(), 3);
        assert_eq!(merge(&idx, &hits, &have, None, "lava", "new").len(), 1);
        assert_eq!(merge(&idx, &hits, &have, None, "classic", "new")[0]["id"], 101, "a mode name finds its maps");
        let by_name = merge(&idx, &hits, &have, Some(Kind::Mode), "", "name");
        let names: Vec<&str> = by_name.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Floor is lava", "Jetpacks", "unknown mode"]);
        // offline: the index alone still lists
        assert_eq!(merge(&idx, &[], &have, None, "", "new").len(), 4);
    }

    #[test]
    fn picks_a_clean_file() {
        let page = json!({"_aFiles": [
            {"_idRow": 1, "_sFile": "old.zip", "_tsDateAdded": 1, "_sAvResult": "clean", "_sMd5Checksum": "AA"},
            {"_idRow": 2, "_sFile": "new.zip", "_tsDateAdded": 3, "_sAvResult": "contains a virus"},
            {"_idRow": 3, "_sFile": "readme.pdf", "_tsDateAdded": 4, "_sAvResult": "clean"},
            {"_idRow": 4, "_sFile": "mid.rar", "_tsDateAdded": 2, "_sAvResult": "clean"},
        ]});
        let f = files_of(&page);
        assert_eq!(f[0]["id"], 3, "newest first");
        assert_eq!(f.iter().find(|x| x["id"] == 1).unwrap()["md5"], "aa");
        assert_eq!(pick_file(&f, None, 0), Ok(4));
        assert_eq!(pick_file(&f, Some(1), 0), Ok(1));
        assert_eq!(pick_file(&f, None, 1), Ok(1), "the index's file");
        assert!(pick_file(&f, Some(2), 0).is_err(), "flagged by GameBanana");
        assert!(pick_file(&f, Some(3), 0).is_err(), "not something the installer takes");
        assert!(pick_file(&[], None, 0).is_err());
    }

    #[test]
    fn gamebanana_links() {
        assert_eq!(parse_gb_link("https://gamebanana.com/mods/739741"), Some((Model::Mod, 739741)));
        assert_eq!(parse_gb_link(" gamebanana.com/sprays/12?tab=x "), Some((Model::Spray, 12)));
        assert_eq!(parse_gb_link("https://www.gamebanana.com/mods/5#updates"), Some((Model::Mod, 5)));
        assert_eq!(parse_gb_link("https://evil.com/gamebanana.com/mods/5"), None);
        assert_eq!(parse_gb_link("https://gamebanana.com/tools/5"), None);
        assert_eq!(parse_gb_link("https://gamebanana.com/mods/abc"), None);
    }

    #[test]
    fn md5_matches_known_values() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"The quick brown fox jumps over the lazy dog"), "9e107d9d372bb6826bd81d3542a419d6");
        assert_eq!(md5_hex(&[b'a'; 1000]), "cabe45dcc9ae5b66ba86600cca6b8ba8");
    }

    fn put(p: PathBuf, b: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b).unwrap();
    }

    /// A game with a custom map: its texture file and sky in cstrike, a nav and finished
    /// knowledge, an overview, and Half-Life's own files in valve.
    fn game(tag: &str, map: &str) -> (PathBuf, MapFile) {
        let root = tmp(tag);
        let hl = root.join("Half-Life");
        let bsp = fixture::build();
        put(hl.join(format!("cstrike/maps/{map}.bsp")), &bsp);
        put(hl.join("cstrike/test.wad"), &wad::build_wad(&[("brick", fixture::miptex("brick", false))]));
        for side in ["up", "dn", "lf", "rt", "ft", "bk"] {
            put(hl.join(format!("cstrike/gfx/env/desert{side}.tga")), b"tga");
        }
        put(hl.join(format!("cstrike/overviews/{map}.bmp")), b"bmp");
        let n = nav::Nav { version: 5, bsp_size: bsp.len() as u32, places: vec!["Mid".into()], areas: vec![] };
        put(hl.join(format!("cstrike/maps/{map}.nav")), &n.write());
        put(hl.join("valve/liblist.gam"), b"");
        let m = MapFile { name: map.into(), bsp: hl.join(format!("cstrike/maps/{map}.bsp")), game: "cstrike".into(), hl, czero: false };
        (root, m)
    }

    fn png() -> String {
        use base64::Engine;
        let mut b = Vec::new();
        image::RgbImage::new(4, 4).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
    }

    #[test]
    fn builds_a_package_the_installer_reads() {
        let (root, m) = game("pkg", "de_lonely");
        let reports = root.join("reports");
        let rep = super::super::health::Report { id: "1".into(), map: "de_lonely".into(), mode: "classic".into(), when: 5, verdict: "good".into(), bots_seen: 10, minutes: 5, ..Default::default() };
        put(reports.join("1.json"), &serde_json::to_vec(&rep).unwrap());
        let old = super::super::health::Report { id: "0".into(), map: "de_lonely".into(), when: 1, verdict: "bad".into(), ..Default::default() };
        put(reports.join("0.json"), &serde_json::to_vec(&old).unwrap());

        let r = readiness(&m, &reports);
        assert!(r.blockers.is_empty(), "{:?}", r.blockers);
        assert!(r.map.nav && !r.map.lsv && r.map.overview && r.map.res);
        assert_eq!(r.health.as_ref().unwrap().verdict, "good", "the newest check");
        assert_eq!(r.map.bsp_sha256.len(), 64);
        let mut files = r.files.clone();
        files.sort();
        assert!(files.contains(&"test.wad".to_string()) && files.contains(&"overviews/de_lonely.bmp".to_string()), "{files:?}");
        assert!(files.iter().any(|f| f.starts_with("gfx/env/desert")));

        let out = root.join("packages");
        let req = PublishReq { title: "De Lonely".into(), summary: "Quiet".into(), author: "jam".into(), modes: vec!["Classic".into(), "bad mode!".into()], screenshots: vec![Shot { name: "a".into(), data: png() }], ..Default::default() };
        let pkg = build_package(&m, &r, &req, &out).unwrap();
        assert_eq!(pkg.file, "de_lonely-loner-strike.zip");
        assert_eq!(pkg.item.map.as_ref().unwrap().modes, ["classic"]);
        assert_eq!(pkg.screenshots, ["de_lonely-1.png"]);
        assert!(out.join("de_lonely-loner-strike-screenshots/de_lonely-1.png").is_file());
        let bytes = std::fs::read(out.join(&pkg.file)).unwrap();
        assert_eq!(pkg.md5, md5_hex(&bytes));
        assert!(!out.join("de_lonely-loner-strike.zip.part").exists());

        // what's inside, and where the app's own installer would put it
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
        for want in ["maps/de_lonely.bsp", "maps/de_lonely.nav", "maps/de_lonely.res", "test.wad", "overviews/de_lonely.bmp", "README.txt", "loner-strike.json"] {
            assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
        }
        assert!(!names.iter().any(|n| n.ends_with(".lsv")), "unfinished knowledge stays out");
        let rels: Vec<Vec<String>> = names.iter().map(|n| crate::archive::listed_parts(n)).collect();
        let (plan, skipped) = crate::archive::plan_rels(&rels);
        let dests: Vec<&str> = plan.iter().map(|(_, d)| d.as_str()).collect();
        assert!(dests.contains(&"maps/de_lonely.bsp") && dests.contains(&"maps/de_lonely.nav") && dests.contains(&"test.wad") && dests.contains(&"overviews/de_lonely.bmp"), "{dests:?}");
        assert!(skipped.iter().all(|s| s == "README.txt" || s == "loner-strike.json"), "{skipped:?}");
        let mut res = String::new();
        z.by_name("maps/de_lonely.res").unwrap().read_to_string(&mut res).unwrap();
        assert!(res.contains("test.wad") && res.contains("overviews/de_lonely.bmp"));
        let mut man = String::new();
        z.by_name("loner-strike.json").unwrap().read_to_string(&mut man).unwrap();
        let man: Value = serde_json::from_str(&man).unwrap();
        assert_eq!(man["item"]["map"]["name"], "de_lonely");

        // the GameBanana form
        let form = gamebanana_form(&pkg);
        assert_eq!(form["category"], "Maps > Bomb/Defuse");
        let text = form["text"].as_str().unwrap();
        assert!(text.contains("Ships with a bot nav") && !text.contains("places named"), "no area has a place yet");
        assert_eq!(form["tag"]["value"], "LonerStrike");

        // registering: the page must carry the tag and the package
        let mut page = json!({"_aGame": {"_idRow": 4254}, "_aTags": [], "_aSubmitter": {"_sName": "jamGB"},
                              "_aFiles": [{"_idRow": 77, "_sFile": "x.zip", "_sMd5Checksum": pkg.md5.to_uppercase(), "_sAvResult": "clean"}]});
        assert!(check_page(&page, Model::Mod, 555, &pkg).unwrap_err().contains("Add the tag"));
        page["_aTags"] = json!([{"_sTitle": "Software Used", "_sValue": "Loner-Strike"}]);
        let it = check_page(&page, Model::Mod, 555, &pkg).unwrap();
        assert_eq!((it.gb, it.file, it.author.as_str()), (555, 77, "jamGB"));
        page["_aFiles"][0]["_sMd5Checksum"] = json!("00");
        assert!(check_page(&page, Model::Mod, 555, &pkg).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn builds_a_mode_package_with_credit() {
        let root = tmp("modepkg");
        let graphs = root.join("graphs");
        let lib = root.join("lib");
        std::fs::create_dir_all(&graphs).unwrap();
        let Ok(good) = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tools/studio/testdata/every_node.graph.json")) else { return };
        let mut g: Value = serde_json::from_str(&good).unwrap();
        g["mode"]["id"] = json!("lava_remix");
        std::fs::write(graphs.join("lava_remix.graph.json"), g.to_string()).unwrap();
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("remixes.json"), r#"{"lava_remix": {"model": "Mod", "gb": 102, "name": "Floor is lava", "author": "jam"}}"#).unwrap();
        let ready = mode_readiness(&graphs, &lib, "lava_remix").unwrap();
        assert_eq!(ready["remixOf"]["author"], "jam");
        assert!(mode_readiness(&graphs, &lib, "../x").is_err());
        let out = root.join("p");
        let req = PublishReq { title: "Lava, mine".into(), author: "me".into(), maps: vec!["de_dust2".into(), "bad;map".into()], bots: "OK".into(), ..Default::default() };
        let pkg = build_mode_package(&graphs, &lib, "lava_remix", &req, &out, None).unwrap();
        assert_eq!(pkg.contents[0], "modes/lava_remix/lava_remix.graph.json");
        assert_eq!(pkg.item.kind, Kind::Mode);
        assert_eq!(pkg.item.remix_of.as_ref().unwrap().gb, 102);
        assert_eq!(pkg.item.mode.as_ref().unwrap().maps, ["de_dust2"]);
        assert!(package_refusal(&pkg.item, &pkg.contents).is_none());
        let bytes = std::fs::read(out.join(&pkg.file)).unwrap();
        assert_eq!(md5_hex(&bytes), pkg.md5);
        let form = gamebanana_form(&pkg);
        assert_eq!(form["category"], "Resources > AMXX Plugins & Mods");
        assert!(form["text"].as_str().unwrap().contains("A remix of Floor is lava by jam"));
        assert_eq!(pkg.item.bots, "ok", "the bot rating goes in the package");
        assert!(form["text"].as_str().unwrap().contains("Bots: Bots are so-so"));
        // the graph unpacks and checks as the installer reads it
        let un = root.join("un");
        std::fs::create_dir_all(&un).unwrap();
        crate::archive::extract(&out.join(&pkg.file), &un, 16 << 20).unwrap();
        assert_eq!(super::super::communitylib::mode_graph(&un).unwrap().mode.id, "lava_remix");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_the_modes_a_map_is_made_for() {
        let root = tmp("lsmodes");
        let ents = fixture::ENTITIES.replacen("\"classname\" \"worldspawn\"", "\"classname\" \"worldspawn\"\n\"ls_modes\" \"3F2B8C1E-9A4D-4E6F-8B2A-1C3D5E7F9A0B:laststand, br;br  Bad-Id surf\"", 1);
        let mut b = fixture::build();
        while b.len() % 4 != 0 {
            b.push(0);
        }
        let at = b.len() as u32;
        let mut text = ents.into_bytes();
        text.push(0);
        b[4..8].copy_from_slice(&at.to_le_bytes());
        b[8..12].copy_from_slice(&(text.len() as u32).to_le_bytes());
        b.extend(text);
        let p = root.join("x.bsp");
        std::fs::write(&p, &b).unwrap();
        assert_eq!(made_for(&p), ["laststand", "br", "surf"]);
        let refs = made_for_refs(&p);
        assert_eq!((refs[0].uid.as_str(), refs[0].id.as_str()), ("3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b", "laststand"), "uid:id pairs");
        assert_eq!(refs[1].uid, "", "bare ids still work");
        std::fs::write(&p, fixture::build()).unwrap();
        assert!(made_for(&p).is_empty());
        assert!(made_for(&root.join("none.bsp")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn uids() {
        let u = new_uid();
        assert!(valid_uid(&u), "{u}");
        assert_ne!(u, new_uid());
        assert!(!valid_uid("3f2b8c1e9a4d4e6f8b2a1c3d5e7f9a0b") && !valid_uid("3F2B8C1E-9A4D-4E6F-8B2A-1C3D5E7F9A0B") && !valid_uid(""));
        let idx = parse_index(br#"{"schema": 1, "items": [
            {"kind": "mode", "gb": 1, "name": "A", "uid": "3F2B8C1E-9A4D-4E6F-8B2A-1C3D5E7F9A0B", "mode": {"id": "a", "uid": "junk"}},
            {"kind": "map", "gb": 2, "name": "B", "map": {"name": "de_b", "modes": ["a"], "modeRefs": [{"uid": "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b", "id": "a", "name": "Mode A"}, {"id": "bad id"}]}},
            {"kind": "map", "gb": 3, "name": "C (older, no uid)", "map": {"name": "de_c"}}]}"#).unwrap();
        assert_eq!(idx.items[0].uid, "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b");
        assert_eq!(idx.items[0].mode.as_ref().unwrap().uid, "", "a bad one is dropped, not the entry");
        assert_eq!(idx.items[1].map.as_ref().unwrap().mode_refs.len(), 1);
        assert_eq!(idx.items.len(), 3);
        let have = HashSet::new();
        let row = entry(None, Some(&idx.items[1]), false, &have).unwrap();
        assert_eq!(row["modes"], json!(["Mode A"]), "cards show the modes' names");
    }

    #[test]
    fn publish_mode_choices_leave_test_leftovers_out() {
        let d = tmp("choices");
        let gm = d.join("game/addons/amxmodx/configs/gamemodes");
        std::fs::create_dir_all(&gm).unwrap();
        std::fs::write(gm.join("modes.ini"), "classic \"Classic\" \"x\" \"\" \"\"\nscoutz \"Scoutz & Knivez\" \"y\" \"\" \"\"\npw_v3 \"V3 check\" \"z\" \"\" \"\"\nmine \"My mode\" \"z\" \"\" \"\"\nlava \"Lava\" \"\" \"\" \"\"\nclassic \"Classic again\" \"\" \"\" \"\"\n").unwrap();
        for id in ["pw_v3", "mine", "lava"] {
            std::fs::write(gm.join(format!("{id}.plugins.ini")), format!("; studio\nstudio_{id}.amxx\n")).unwrap();
        }
        let graphs = d.join("graphs");
        std::fs::create_dir_all(&graphs).unwrap();
        std::fs::write(graphs.join("mine.graph.json"), r#"{"mode": {"id": "mine", "uid": "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"}}"#).unwrap();
        let lib = vec![super::super::communitylib::Installed { kind: Kind::Mode, mode: "lava".into(), gb: 5, ..Default::default() }];
        let c = mode_choices(&d.join("game"), &graphs, &lib);
        let ids: Vec<&str> = c.iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["classic", "scoutz", "mine", "lava"], "no Studio leftover without a graph, no repeats");
        assert_eq!(c[1]["name"], "Scoutz & Knivez");
        assert_eq!(c[2]["uid"], "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bot_ratings() {
        assert_eq!((clean_bots(" Great "), clean_bots("superb"), clean_bots("")), ("great".into(), String::new(), String::new()));
        let idx = parse_index(br#"{"schema": 1, "items": [{"kind": "mode", "gb": 1, "name": "A", "bots": "WEAK", "mode": {"id": "a"}}, {"kind": "mode", "gb": 2, "name": "B", "bots": "amazing", "mode": {"id": "b"}}]}"#).unwrap();
        assert_eq!((idx.items[0].bots.as_str(), idx.items[1].bots.as_str()), ("weak", ""), "an unknown value is 'not said', not a dropped list");
        let row = entry(None, Some(&idx.items[0]), false, &HashSet::new()).unwrap();
        assert_eq!(row["bots"], "weak");
        let d = tmp("bots");
        std::fs::write(d.join("lava.graph.json"), r#"{"mode": {"id": "lava", "bots": "ok"}}"#).unwrap();
        assert_eq!(graph_bots(&d, "lava"), "ok");
        assert_eq!(graph_bots(&d, "none"), "");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn package_refusals() {
        let (root, m) = game("refuse", "de_dust2");
        let r = readiness(&m, &root.join("none"));
        assert!(!r.blockers.is_empty(), "the game's own map");
        let req = PublishReq { title: "x".into(), author: "y".into(), ..Default::default() };
        assert!(build_package(&m, &r, &req, &root.join("p")).is_err());
        let _ = std::fs::remove_dir_all(&root);

        let (root, m) = game("refuse2", "de_mine");
        let r = readiness(&m, &root.join("none"));
        let out = root.join("p");
        assert!(build_package(&m, &r, &PublishReq { author: "y".into(), ..Default::default() }, &out).unwrap_err().contains("name"));
        assert!(build_package(&m, &r, &PublishReq { title: "x".into(), ..Default::default() }, &out).unwrap_err().contains("made it"));
        let bad = PublishReq { title: "x".into(), author: "y".into(), screenshots: vec![Shot { name: "a".into(), data: "aGVsbG8=".into() }], ..Default::default() };
        assert!(build_package(&m, &r, &bad, &out).unwrap_err().contains("PNG or JPEG"));
        assert!(!out.join("de_mine-loner-strike.zip").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// GameBanana answers the tag search without an error (`cargo test community_live -- --ignored`).
    #[test]
    #[ignore]
    fn community_live() {
        *SEARCH.lock().unwrap() = None;
        let hits = search(true).expect("GameBanana answered");
        assert!(hits.iter().all(|h| h.id > 0));
        println!("{} tagged creations", hits.len());
    }

    #[test]
    fn a_registration_says_only_which_page() {
        let it = Item { kind: Kind::Map, model: Model::Mod, gb: 739197, file: 1885247, name: "de_x".into(), summary: "s".into(), ..Default::default() };
        let b = submit_body(&it);
        assert_eq!(b["page"], json!({"model": "Mod", "id": 739197, "file": 1885247}));
        assert!(b.get("item").is_none());
        assert_eq!(b.as_object().unwrap().len(), 3);
    }

    #[test]
    fn remembers_one_registration_per_page() {
        let mut list = Vec::new();
        let it = |gb| Item { gb, ..Default::default() };
        remember(&mut list, Mine { item: it(1), state: "waiting".into(), ..Default::default() });
        remember(&mut list, Mine { item: it(2), ..Default::default() });
        remember(&mut list, Mine { item: it(1), state: "sent".into(), ..Default::default() });
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].item.gb, list[0].state.as_str()), (1, "sent"));
    }
}

#[cfg(test)]
mod risk_tests {
    use super::*;

    #[test]
    fn a_mode_that_runs_server_commands_shows_it_and_asks_first() {
        let idx = parse_index(br#"{"schema": 1, "items": [
            {"kind": "mode", "gb": 1, "name": "Calm", "mode": {"id": "calm"}},
            {"kind": "mode", "gb": 2, "name": "Loud", "mode": {"id": "loud", "capabilities": ["kicks_players", "server_commands", "wipe_disk"]}}]}"#).unwrap();
        let calm = idx.items.iter().find(|i| i.gb == 1).unwrap();
        let loud = idx.items.iter().find(|i| i.gb == 2).unwrap();
        assert_eq!(loud.mode.as_ref().unwrap().capabilities, vec!["server_commands", "kicks_players"], "known ones only, in order");
        let t = trust(Kind::Mode, Some(calm), false);
        assert!(t.allowed && t.confirm.is_none() && t.risks.is_empty());
        let t = trust(Kind::Mode, Some(loud), false);
        assert!(t.allowed);
        assert_eq!(t.risks, vec!["Runs server commands", "Can kick players"]);
        let q = t.confirm.unwrap();
        assert!(q.contains("runs server commands; can kick players") && q.ends_with("Install it?"), "{q}");
        // shown on the card and the details as badges
        assert_eq!(serde_json::to_value(trust(Kind::Mode, Some(loud), false)).unwrap()["risks"], json!(["Runs server commands", "Can kick players"]));
        assert!(serde_json::to_value(trust(Kind::Mode, Some(calm), false)).unwrap().get("risks").is_none());
    }

    fn v(x: &[&str]) -> Vec<String> {
        x.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_install_is_bound_to_what_the_player_saw() {
        // a calm graph: nothing to ask
        assert!(risk_gate(&[], &[], &[]).is_none());
        // confirmed nothing (a calm listing), the download runs server commands: asked again
        let a = risk_gate(&v(&["server_commands"]), &[], &[]).expect("asks");
        assert_eq!(a["riskCodes"], json!(["server_commands"]));
        assert_eq!(a["differs"], true);
        assert!(a["needsConfirm"].as_str().unwrap().starts_with("This mode does more than its listing says."));
        // said yes to exactly that: installs
        assert!(risk_gate(&v(&["server_commands"]), &v(&["server_commands"]), &v(&["server_commands"])).is_none());
        // said yes to the listing's server commands, the graph also kicks: asked again, naming both
        let a = risk_gate(&v(&["kicks_players", "server_commands"]), &v(&["server_commands"]), &v(&["server_commands"])).expect("asks");
        assert_eq!(a["risks"], json!(["Runs server commands", "Can kick players"]));
        assert_eq!(a["differs"], true);
        // the listing was right but nobody said yes (a bare confirmed: true): asked, no warning
        let a = risk_gate(&v(&["amx_commands"]), &v(&["amx_commands"]), &[]).expect("asks");
        assert_eq!(a["differs"], false);
        // a yes to more than it does is fine; unknown codes count for nothing
        assert!(risk_gate(&v(&["plugin_calls"]), &[], &v(&["plugin_calls", "kicks_players"])).is_none());
        assert!(risk_gate(&v(&["plugin_calls"]), &[], &v(&["everything"])).is_some());
    }

    #[test]
    fn risk_words_follow_the_studio() {
        assert_eq!(risk_words(&["amx_commands", "plugin_calls"]), vec!["Runs AMX Mod X commands", "Calls other plugins"]);
        assert!(risk_words::<&str>(&[]).is_empty());
        // every code the Studio reports has words here
        for c in ["server_commands", "amx_commands", "plugin_calls", "kicks_players"] {
            assert_eq!(risk_words(&[c]).len(), 1, "{c}");
        }
    }
}

#[cfg(test)]
mod requirement_tests {
    use super::*;

    #[test]
    fn a_modes_page_says_what_it_plays_on_and_needs() {
        let idx = parse_index(br#"{"schema": 1, "items": [
            {"kind": "map", "gb": 101, "name": "de_lonely", "map": {"name": "de_lonely"}},
            {"kind": "mode", "gb": 2, "name": "Crates", "mode": {"id": "crates", "requirements": {
                "defaultMap": "de_lonely", "maps": ["de_dust2", "de_lonely", "bad map"],
                "content": [{"path": "Models/Crate.mdl", "kind": "x", "link": {"model": "Mod", "id": 77, "name": "Crate pack"}},
                            {"path": "../../etc/passwd.mdl"}, {"path": "sound/beep.wav", "link": {"model": "Skin", "id": 1}}]}}}]}"#).unwrap();
        let r = idx.items.iter().find(|i| i.gb == 2).unwrap().mode.as_ref().unwrap().requirements.clone().unwrap();
        assert_eq!(r.maps, vec!["de_dust2", "de_lonely"], "bad names dropped");
        assert_eq!(r.content.len(), 2, "a path out of the game dropped");
        assert_eq!(r.content[0].path, "models/crate.mdl");
        assert_eq!(r.content[0].kind, "model");
        assert!(r.content[1].link.is_none(), "not a real page link");
        let have: HashSet<String> = ["de_lonely".to_string()].into();
        let v = needs_json(&r, &idx, &have, &|p| p == "sound/beep.wav");
        assert_eq!(v["defaultMap"], "de_lonely");
        assert_eq!(v["maps"][0]["name"], "de_lonely");
        assert_eq!(v["maps"][0]["item"]["id"], 101, "the community map that provides it");
        assert_eq!(v["maps"][1]["base"], true);
        assert_eq!(v["content"][0]["have"], false);
        assert_eq!(v["content"][0]["link"]["id"], 77);
        assert_eq!(v["content"][1]["have"], true);
    }
}

#[cfg(test)]
mod unlist_tests {
    use super::*;

    #[test]
    fn unlisted_and_withdrawn_stay_out_of_lists_and_get_no_updates() {
        let idx = parse_index(br#"{"schema": 1, "items": [
            {"kind": "map", "gb": 1, "name": "a", "file": 9, "map": {"name": "a"}, "status": "unlisted"},
            {"kind": "map", "gb": 2, "name": "b", "file": 9, "map": {"name": "b"}, "status": "withdrawn"},
            {"kind": "map", "gb": 3, "name": "c", "file": 9, "map": {"name": "c"}, "status": "listed"}]}"#).unwrap();
        let st = |gb: i64| idx.items.iter().find(|i| i.gb == gb).unwrap().status;
        assert_eq!(st(1), Status::Unlisted);
        assert_eq!(st(2), Status::Withdrawn);
        assert!(st(1).kept_out() && st(2).kept_out() && !st(3).kept_out());
        assert!(st(1).note().unwrap().contains("The author unlisted this"));
        // Try it refuses both, with a plain reason; listed ones go on
        assert_eq!(try_refusal(st(1)), Some(UNLISTED));
        assert_eq!(try_refusal(st(2)), Some(WITHDRAWN));
        assert_eq!(try_refusal(st(3)), None);
        assert!(st(2).note().unwrap().contains("withdrawn"));
        // installed older files: an update only for the listed one
        let lib = |gb: i64| super::super::communitylib::Installed { kind: Kind::Map, model: Model::Mod, gb, file: 1, ..Default::default() };
        assert_eq!(super::super::communitylib::newer(&lib(1), &idx), None);
        assert_eq!(super::super::communitylib::newer(&lib(2), &idx), None);
        assert_eq!(super::super::communitylib::newer(&lib(3), &idx), Some(9));
        // an older app's reader would take both as hidden; ours too where it matters (out of lists)
        let old = parse_index(br#"{"schema": 1, "items": [{"kind": "map", "gb": 4, "name": "d", "map": {"name": "d"}, "status": "something-new"}]}"#).unwrap();
        assert_eq!(old.items[0].status, Status::Hidden);
    }
}
