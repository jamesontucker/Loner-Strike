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
        self.installer().is_some() || self == Kind::Prefab
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
    /// nobody has looked at it
    #[default]
    None,
    /// a known author, source shipped, not read line by line
    Verified,
    /// a Loner-Strike reviewer read the source and built the plugin from it (hash pinned)
    Reviewed,
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
    /// plugins it needs; none: a data-only mode (settings, graphs, components)
    pub plugins: Vec<PluginMeta>,
    pub review: Review,
    /// maps it's made for
    pub maps: Vec<String>,
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
    /// kept out of lists (asked by the author, or waiting on a question), still openable
    Hidden,
    /// pulled: never offered, never installed
    Revoked,
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
        if let Some(m) = &it.map {
            if !crate::maps::safe_arg(&m.name) || m.name.len() > 31 {
                return false;
            }
        }
        if let Some(m) = &mut it.mode {
            if !safe_mode_id(&m.id) {
                return false;
            }
            m.maps.retain(|x| crate::maps::safe_arg(x));
        }
        it.name = one_line(&it.name, 80);
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

fn index(refresh: bool) -> (Index, Source) {
    let mut g = INDEX.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, idx, src)) = g.as_ref() {
        if !refresh && at.elapsed() < INDEX_FRESH && *src == Source::Live {
            return (idx.clone(), *src);
        }
    }
    let (idx, src) = load_index(&|| http_get(INDEX_URL, MAX_INDEX), &dir().join("index.json"));
    *g = Some((Instant::now(), idx.clone(), src));
    (idx, src)
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
    /// a question to answer before installing (plugins that will run on your server)
    pub confirm: Option<String>,
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
    let t = |level, label, allowed, confirm: Option<String>, note: &str| Trust { level, label, allowed, confirm, note: note.into() };
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
        return t("data", "Settings only", true, None, "Settings and rules only: no plugins, nothing that runs code.");
    }
    let complete = mode.plugins.iter().all(|p| p.sha256.len() == 64 && p.sha256.bytes().all(|c| c.is_ascii_hexdigit()) && !p.source.trim().is_empty());
    let names = mode.plugins.iter().map(|p| p.file.as_str()).collect::<Vec<_>>().join(", ");
    if !complete {
        return t("unreviewed", "Not checked", false, None, "It runs plugins without their source, so the app won't install it.");
    }
    match mode.review {
        Review::Reviewed => t(
            "reviewed",
            "Reviewed",
            true,
            Some(format!("This mode adds plugins to your server: {names}. The Loner-Strike team read their source. Install it?")),
            "Its plugins were read and built from source by the Loner-Strike team.",
        ),
        Review::Verified => t(
            "verified",
            "Known author",
            true,
            Some(format!(
                "This mode adds plugins to your server that nobody at Loner-Strike has read: {names}. Plugins can do anything a server can. Only install it if you trust its author."
            )),
            "Made by a known author with its source included, but not reviewed.",
        ),
        Review::None => t("unreviewed", "Not checked", false, None, "It runs plugins nobody has checked yet, so the app won't install it."),
    }
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
    if tr.level == "blocked" || it.is_some_and(|i| i.status == Status::Hidden) {
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
        "modes": map.map(|m| m.modes.clone()).unwrap_or_default(),
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
            }))
        })
        .await,
    )
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
}

/// The file to install: the one asked for, else the index's, else the newest clean supported one.
pub fn pick_file(files: &[Value], asked: Option<i64>, listed: i64) -> Result<i64, &'static str> {
    let ok = |f: &&Value| f["clean"] == json!(true) && f["supported"] == json!(true);
    let want = asked.filter(|a| *a > 0).or((listed > 0).then_some(listed));
    if let Some(w) = want {
        return files.iter().filter(ok).find(|f| f["id"].as_i64() == Some(w)).and_then(|f| f["id"].as_i64()).ok_or("That file isn't on the page, or GameBanana flagged it.");
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
    let Some(game) = st.local_game() else { return json_answer(Err(Fail::new(StatusCode::CONFLICT, NO_GAME))) };
    json_answer(
        blocking(move || {
            let fail = |m: &str| Fail::new(StatusCode::OK, m);
            let (idx, _) = index(false);
            let it = idx.items.iter().find(|i| i.model == model && i.gb == req.id).cloned();
            let page = profile(model, req.id).map_err(|e| fail(&e))?;
            if !has_tag(&page["_aTags"]) {
                return Err(fail("This page doesn't carry the Loner-Strike tag, so it isn't installed from here. Use Get more instead."));
            }
            let cat = s(&page["_aCategory"]["_sName"]);
            let root = s(&page["_aSuperCategory"]["_sName"]);
            let kind = it.as_ref().map(|i| i.kind).unwrap_or_else(|| kind_of(model.path(), if root.is_empty() { &cat } else { &root }, &cat));
            let tr = trust(kind, it.as_ref(), idx.revoked.iter().any(|r| *r == key(model, req.id)));
            if !tr.allowed {
                return Err(fail(&tr.note));
            }
            if tr.confirm.is_some() && !req.confirmed {
                return Err(fail("Confirm first."));
            }
            if kind == Kind::Prefab {
                // prefabs go into the map editor's library (small: installed here, no job)
                // (a prefab can be a bare .map file)
                let mut files = files_of(&page);
                for f in files.iter_mut().filter(|f| s(&f["file"]).to_lowercase().ends_with(".map")) {
                    f["supported"] = json!(true);
                }
                let file = pick_file(&files, req.file, it.as_ref().map(|i| i.file).unwrap_or(0)).map_err(fail)?;
                let md5 = files.iter().find(|f| f["id"].as_i64() == Some(file)).map(|f| s(&f["md5"])).unwrap_or_default();
                let fname = files.iter().find(|f| f["id"].as_i64() == Some(file)).map(|f| s(&f["file"])).unwrap_or_default();
                let author = s(&page["_aSubmitter"]["_sName"]);
                let v = try_prefab(file, &fname, &md5, &author, Some(&game.dir)).map_err(|e| fail(&e))?;
                return Ok(json!({"ok": true, "kind": "prefab", "prefabs": v["prefabs"], "wads": v["wads"]}));
            }
            let section = kind.installer().ok_or_else(|| fail(kind.no_installer()))?;
            let file = pick_file(&files_of(&page), req.file, it.as_ref().map(|i| i.file).unwrap_or(0)).map_err(fail)?;
            let job = game.store.start_section_install(section, req.id, file).map_err(|e| fail(&e.message()))?;
            Ok(json!({"ok": true, "job": job, "section": section, "map": it.and_then(|i| i.map).map(|m| m.name)}))
        })
        .await,
    )
}

/// Downloads a prefab file from GameBanana (checked against the page's MD5), unpacks it and
/// hands it to the map editor's prefab library (`tools::editor::prefabs::install_download`).
fn try_prefab(file: i64, fname: &str, md5: &str, author: &str, game_dir: Option<&Path>) -> Result<Value, String> {
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
    if !r.status().is_success() {
        return Err("GameBanana didn't send the file. Try again in a minute.".into());
    }
    let limit: u64 = 128 << 20;
    let mut bytes = Vec::new();
    r.take(limit + 1).read_to_end(&mut bytes).map_err(|_| "The download was cut off. Try again.".to_string())?;
    if bytes.len() as u64 > limit {
        return Err("That prefab download is over 128 MB.".into());
    }
    if md5.len() == 32 && md5_hex(&bytes) != md5 {
        return Err("The download doesn't match the file GameBanana lists. Try again.".into());
    }
    let work = dir().join("work");
    let tmp = crate::sys::make_temp_dir(&work, "prefab-").map_err(|e| e.to_string())?;
    let res = (|| {
        let low = fname.to_lowercase();
        let ext = if crate::archive::is_archive(&low) { crate::archive::ext_of(&low).to_string() } else if low.ends_with(".map") { ".map".into() } else { return Err("That file isn't a prefab the editor can read (a .map or an archive of them).".to_string()) };
        let src = tmp.join("x");
        std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
        if ext == ".map" {
            let base = Path::new(&fname.replace('\\', "/")).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "prefab.map".into());
            std::fs::write(src.join(base), &bytes).map_err(|e| e.to_string())?;
        } else {
            let a = tmp.join(format!("download{ext}"));
            std::fs::write(&a, &bytes).map_err(|e| e.to_string())?;
            crate::archive::extract(&a, &src, 512 << 20)?;
            crate::archive::extract_nested(&src)?;
        }
        super::editor::prefabs::install_download(&src, author, game_dir)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res
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
        r.warnings.push(format!("{missing_tex} textures aren't in any texture file here; players would see a checkerboard. Fix it in the map doctor first."));
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
    let nav = &doc["nav"];
    r.map.nav = nav["exists"] == json!(true) && nav.get("error").is_none() && nav["matchesMap"] != json!(false);
    r.map.places = r.map.nav && nav["namedAreas"].as_u64().unwrap_or(0) > 0;
    r.map.lsv = r.map.nav && doc["lsv"]["done"] == json!(true) && doc["lsv"]["fits"] != json!(false);
    if !r.map.nav {
        r.warnings.push("Bots don't know this map yet (no nav that fits it). Teach them in the map doctor so it gets the \"Works with bots\" badge.".into());
    } else if !r.map.lsv {
        r.tips.push("The bots' map knowledge isn't finished; play the map with bots for a while (or run a health check) and it fills in.".into());
    }
    if r.map.nav && !r.map.places {
        r.tips.push("Name places on the nav (Callouts) so bots say where they are.".into());
    }
    if !r.map.overview {
        r.tips.push("There's no overview (radar) picture for this map.".into());
    }
    r.health = latest_health(reports, &m.name);
    match &r.health {
        None => r.tips.push("Run a map health check: a good result earns the \"Checked with bots\" badge.".into()),
        Some(h) if h.verdict == "bad" => r.warnings.push("The last health check found real problems (falls, stuck bots or errors). Have a look before sharing.".into()),
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
    // a picture that says it's one but isn't is refused too
    image::load_from_memory(&b).map_err(|_| "A screenshot is damaged.".to_string())?;
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
    let item = Item {
        kind: Kind::Map,
        name: title,
        author,
        summary: one_line(&req.summary, 400),
        games: vec![r.game.clone()],
        map: Some(map),
        health: r.health.clone(),
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
        text.push_str(&format!("\n- Loner-Strike health check: {} ({} bots, {} min)", h.verdict, h.bots, h.minutes));
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
    json!({
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
    /// sent (the inbox took it) or waiting (kept here, sent next time)
    pub state: String,
    pub when: u64,
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

fn send(item: &Item) -> bool {
    let body = json!({"schema": SCHEMA, "app": env!("CARGO_PKG_VERSION"), "item": item});
    crate::sys::http()
        .post(SUBMIT_URL)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(10))
        .body(body.to_string())
        .send()
        .is_ok_and(|r| r.status().is_success())
}

#[derive(Deserialize)]
struct RegisterReq {
    url: String,
    /// the package's zip name (from publish); the newest package with a matching MD5 otherwise
    #[serde(default)]
    file: String,
}

fn packages_dir() -> PathBuf {
    dir().join("packages")
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
            let item = check_page(&page, model, id, pkg).map_err(fail)?;
            let sent = send(&item);
            let mut list = load_mine(&mine_path());
            let entry = Mine { item: item.clone(), url: model.url(id), state: if sent { "sent" } else { "waiting" }.into(), when: now() };
            remember(&mut list, entry);
            save_mine(&mine_path(), &list);
            Ok(json!({"ok": true, "state": if sent { "sent" } else { "waiting" }, "item": item,
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
                if send(&m.item) {
                    m.state = "sent".into();
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
                let suggested: Vec<String> = super::health::preset_mode(&presets, &m.name).into_iter().chain(r.health.as_ref().map(|h| h.mode.clone())).filter(|x| !x.is_empty()).collect::<Vec<_>>();
                let mut suggested = suggested;
                suggested.dedup();
                Ok(json!({"ready": r, "suggestedModes": suggested, "tag": format!("{TAG_TITLE}: {TAG_VALUE}")}))
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

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/community/browse", get(api_browse))
        .route("/api/tools/community/item/:model/:id", get(api_item))
        .route("/api/tools/community/try", post(api_try))
        .route("/api/tools/community/publish/:map", get(api_ready).post(api_publish))
        .route("/api/tools/community/register", post(api_register))
        .route("/api/tools/community/mine", get(api_mine))
        .route("/api/tools/community/reveal", post(api_reveal))
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
        // reviewed plugins: a confirm naming them
        let t = trust(Kind::Mode, get(103), false);
        assert!(t.allowed && t.level == "reviewed" && t.confirm.as_ref().unwrap().contains("jet.amxx"));
        // verified author: a stronger warning
        let mut it = get(103).unwrap().clone();
        it.mode.as_mut().unwrap().review = Review::Verified;
        let t = trust(Kind::Mode, Some(&it), false);
        assert!(t.allowed && t.level == "verified" && t.confirm.unwrap().contains("nobody at Loner-Strike has read"));
        // nobody looked: no
        it.mode.as_mut().unwrap().review = Review::None;
        assert!(!trust(Kind::Mode, Some(&it), false).allowed);
        // reviewed but a plugin without source (or a bad hash): no
        it.mode.as_mut().unwrap().review = Review::Reviewed;
        it.mode.as_mut().unwrap().plugins[0].source = " ".into();
        assert!(!trust(Kind::Mode, Some(&it), false).allowed);
        it.mode.as_mut().unwrap().plugins[0].source = "jet.sma".into();
        it.mode.as_mut().unwrap().plugins[0].sha256 = "short".into();
        assert!(!trust(Kind::Mode, Some(&it), false).allowed);
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
        let req = PublishReq { title: "De Lonely".into(), summary: "Quiet".into(), author: "jam".into(), modes: vec!["Classic".into(), "bad mode!".into()], screenshots: vec![Shot { name: "a".into(), data: png() }] };
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
