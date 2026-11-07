//! Get maps: browse GameBanana's Counter-Strike 1.6 maps and install them.
//!
//! Only map content is copied into the game (the map, its textures, models, sounds,
//! sprites, sky and radar images). Anything that could run code or change settings
//! is skipped, and files you already have are never replaced. Each install is
//! recorded in installed_maps.json so it can be uninstalled cleanly.

use crate::archive::{self, MAX_UNPACKED};
use crate::content::{self, Section};
use crate::sys;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

const API: &str = "https://gamebanana.com/apiv11";
pub const MAPS_CATEGORY: i64 = 5474; // Counter-Strike 1.6 > Maps
/// Maps nobody had a picture for (the name changes when the search gets better, so
/// earlier misses get another try)
pub const MISS_FILE: &str = "not_found_v3.json";
/// Marks that pictures found by older, looser searches were cleared (see forget_old_lookups)
const LOOKUPS_CLEARED: &str = "lookups_v3";
const MISS_DAYS: f64 = 7.0;
/// Words a map page's title may have around the map's name ("aim_map (CS 1.6)")
const TITLE_TAGS: &[&str] = &["cs", "16", "cs16", "cz", "czero", "condition", "zero", "counterstrike", "map", "final", "fixed", "remake"];
/// Result pages read when looking for a map's picture (50 a page)
const SHOT_PAGES: i64 = 4;
const PER_PAGE: i64 = 24;
const MAX_DOWNLOAD: u64 = 1536 * 1024 * 1024;
const USER_AGENT: &str = "LonerStrike/1.0 (local map browser)";
const DOWNLOAD_HOSTS: &[&str] = &["gamebanana.com", "files.gamebanana.com"];

/// GameBanana map types, in the order the panel shows them
pub const CATEGORIES: &[(i64, &str)] = &[
    (5474, "All maps"), (5484, "Bomb defusal"), (5493, "Hostage rescue"), (5483, "Deathmatch"),
    (5488, "Zombie"), (5510, "Zombie escape"), (5503, "Gun Game"), (5501, "Surf"), (5521, "Scout"),
    (5486, "AWP"), (5497, "Aim"), (5485, "Fight yard"), (5502, "Knife"), (5507, "Bunny hop"),
    (5504, "Climb"), (5492, "Deathrun"), (5489, "Jailbreak"), (5491, "Soccer Jam"),
    (5495, "Hide and seek"), (5513, "Vehicles"), (5496, "Fun"), (5506, "Map packs"),
];

fn sort_key(sort: &str) -> &'static str {
    match sort {
        "new" => "Generic_Newest",
        "liked" => "Generic_MostLiked",
        _ => "Generic_MostDownloaded",
    }
}

#[derive(Debug)]
pub enum StoreError {
    /// Something about the request or the download (HTTP 400 / job message)
    Bad(String),
    /// GameBanana couldn't be reached (HTTP 502)
    Net,
}

impl StoreError {
    pub fn message(&self) -> String {
        match self {
            StoreError::Bad(m) => m.clone(),
            StoreError::Net => "Couldn't reach GameBanana. Check your internet connection.".into(),
        }
    }
}

fn get_json(path: &str, params: &[(&str, String)]) -> Result<Value, StoreError> {
    let r = sys::http()
        .get(format!("{API}{path}"))
        .query(params)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send()
        .map_err(|_| StoreError::Net)?;
    if !r.status().is_success() {
        return Err(StoreError::Net);
    }
    let text = r.text().map_err(|_| StoreError::Net)?;
    serde_json::from_str(&text).map_err(|e| StoreError::Bad(e.to_string()))
}

fn get_bytes(url: &str, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    let r = sys::http().get(url).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(20)).send().ok()?;
    if !r.status().is_success() {
        return None;
    }
    let mut out = Vec::new();
    r.take(limit).read_to_end(&mut out).ok()?;
    Some(out)
}

fn s(v: &Value) -> String {
    v.as_str().map(String::from).unwrap_or_default()
}

fn image(media: &Value, size: &str) -> Option<String> {
    let i = media.get("_aImages")?.as_array()?.first()?;
    let file = i.get(size).and_then(|v| v.as_str()).or_else(|| i.get("_sFile").and_then(|v| v.as_str()))?;
    Some(format!("{}/{}", s(&i["_sBaseUrl"]), file))
}

fn html_unescape(t: &str) -> String {
    static ENT: OnceLock<Regex> = OnceLock::new();
    let re = ENT.get_or_init(|| Regex::new(r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);").unwrap());
    re.replace_all(t, |c: &regex::Captures| {
        let e = &c[1];
        let ch = if let Some(h) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
            u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
        } else if let Some(d) = e.strip_prefix('#') {
            d.parse().ok().and_then(char::from_u32)
        } else {
            match e {
                "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'), "hellip" => Some('…'), "mdash" => Some('—'), "ndash" => Some('–'),
                "rsquo" => Some('’'), "lsquo" => Some('‘'), "rdquo" => Some('”'), "ldquo" => Some('“'), "copy" => Some('©'),
                _ => None,
            }
        };
        ch.map(String::from).unwrap_or_else(|| c[0].to_string())
    })
    .to_string()
}

/// GameBanana descriptions are HTML; the panel only shows text.
pub fn plain(text: &str, limit: usize) -> String {
    static RES: OnceLock<[Regex; 4]> = OnceLock::new();
    let [br, tag, sp, nl] = RES.get_or_init(|| {
        [
            Regex::new(r"(?i)<br\s*/?>|</p>|</li>|</h\d>").unwrap(),
            Regex::new(r"<[^>]+>").unwrap(),
            Regex::new(r"[ \t]+").unwrap(),
            Regex::new(r"\n{3,}").unwrap(),
        ]
    });
    let t = br.replace_all(text, "\n");
    let t = html_unescape(&tag.replace_all(&t, ""));
    let t = nl.replace_all(&sp.replace_all(&t, " "), "\n\n").trim().to_string();
    let n = t.chars().count();
    let mut out: String = t.chars().take(limit).collect();
    if n > limit {
        out.push('…');
    }
    out
}

/// A few at a time: the map grid asks for many pictures at once.
struct Semaphore {
    left: Mutex<usize>,
    cv: Condvar,
}

impl Semaphore {
    fn acquire(&self) -> impl Drop + '_ {
        let mut n = self.left.lock().unwrap();
        while *n == 0 {
            n = self.cv.wait(n).unwrap();
        }
        *n -= 1;
        struct G<'a>(&'a Semaphore);
        impl Drop for G<'_> {
            fn drop(&mut self) {
                *self.0.left.lock().unwrap() += 1;
                self.0.cv.notify_one();
            }
        }
        G(self)
    }
}

#[derive(Clone, Serialize)]
pub struct Job {
    pub id: String,
    #[serde(rename = "mod")]
    pub mod_id: i64,
    pub file: i64,
    /// maps, skins, sounds... (see content::SECTIONS)
    pub section: String,
    pub state: String,
    pub got: u64,
    pub size: u64,
    pub message: String,
    pub maps: Vec<String>,
    pub kept: Vec<String>,
    pub skipped: Vec<String>,
    /// Game files this download replaced (backed up; put back when it's removed)
    pub replaced: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<f64>,
    /// a file the player picked on their computer, instead of a GameBanana download
    #[serde(skip)]
    pub local: Option<PathBuf>,
}

/// Single files a section takes as they are (anything else has to be an archive).
pub fn local_single(section: &str) -> &'static [&'static str] {
    if section == "maps" {
        &[".bsp"]
    } else {
        &[".mdl", ".wav", ".spr", ".wad", ".bmp", ".tga"]
    }
}

/// Maps waiting for a picture lookup, and how many threads are working through them.
#[derive(Default)]
struct Later {
    queue: VecDeque<String>,
    /// queued or being looked up (each map once)
    wanted: HashSet<String>,
    workers: usize,
}

/// Picture lookups at once in the background
const LOOKUP_WORKERS: usize = 3;

/// installed_maps.json and content/record.json are app-wide: every game, server and
/// temporary store writes them under this one lock.
pub static RECORDS: Mutex<()> = Mutex::new(());

pub struct MapStore {
    /// A dedicated server's store: its weapon skins and player models go in under their own
    /// names for gm_content.amxx (servercontent.rs) instead of replacing the game's files
    pub server: bool,
    cstrike: PathBuf,
    game_dir: PathBuf,
    record_path: PathBuf,
    pub shots: PathBuf,
    work: PathBuf,
    /// Guards installed_maps.json and the misses file
    lookups: Semaphore,
    /// Picture lookups waiting or under way in the background (see find_shot_later)
    later: Mutex<Later>,
    jobs: Mutex<HashMap<String, Job>>,
    /// The game's cached map list: emptied after an install so the next list is fresh
    maps_cache: Arc<Mutex<Option<Vec<Value>>>>,
    /// Skins, sounds... installed into the game folder
    pub content: content::Records,
}

impl MapStore {
    pub fn new(hl: &Path, game_dir: &Path, app_dir: &Path, maps_cache: Arc<Mutex<Option<Vec<Value>>>>) -> MapStore {
        let shots = app_dir.join("cache").join("gbshots");
        let _ = std::fs::create_dir_all(&shots);
        let store = MapStore {
            server: false,
            cstrike: hl.join("cstrike"),
            game_dir: game_dir.to_path_buf(),
            record_path: app_dir.join("installed_maps.json"),
            shots,
            work: app_dir.join("cache").join("downloads"),
            lookups: Semaphore { left: Mutex::new(3), cv: Condvar::new() },
            later: Mutex::new(Later::default()),
            jobs: Mutex::new(HashMap::new()),
            maps_cache,
            content: content::Records::new(app_dir),
        };
        store.forget_old_lookups();
        store
    }

    /// The store for a dedicated server: its maps go in its own game folder (the one
    /// that's uploaded to a server on another machine, Condition Zero's included), with a
    /// record of its own, so your game and each server list and remove only their own.
    pub fn new_server(hl: &Path, game_dir: &Path, app_dir: &Path, maps_cache: Arc<Mutex<Option<Vec<Value>>>>) -> MapStore {
        let mut store = MapStore::new(hl, game_dir, app_dir, maps_cache);
        store.cstrike = game_dir.to_path_buf();
        store.record_path = hl.join(".installed_maps.json");
        store.server = true;
        store
    }

        // ---- install record -------------------------------------------------
    pub fn records(&self) -> Map<String, Value> {
        sys::read_json_map(&self.record_path)
    }

    fn save_records(&self, rec: &Map<String, Value>) -> std::io::Result<()> {
        sys::write_atomic(&self.record_path, serde_json::to_string_pretty(rec).unwrap_or_default().as_bytes())
    }

    /// Records a map copied in from this computer's game (mapcopy.rs) under `key`, merged
    /// with what that key already has, so it lists as downloaded and Remove takes away
    /// `files` (the ones copied in) but never `shared` (ones the server already had).
    pub fn record_copy(&self, key: &str, info: &Value, map: &str, files: &[String], shared: &[String]) -> std::io::Result<()> {
        {
            let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
            let mut rec = self.records();
            let old = rec.get(key).cloned().unwrap_or(json!({}));
            let merge = |k: &str, new: &[String]| {
                let mut set: Vec<String> = old[k].as_array().into_iter().flatten().map(s).chain(new.iter().cloned()).collect();
                set.sort();
                set.dedup();
                set
            };
            let pick = |k: &str| if old[k].is_null() { info[k].clone() } else { old[k].clone() };
            rec.insert(
                key.to_string(),
                json!({
                    "name": pick("name"), "author": pick("author"), "url": pick("url"), "file": "copied from your game",
                    "date": sys::now() as i64, "maps": merge("maps", &[map.to_string()]), "files": merge("files", files),
                    "shared": merge("shared", shared),
                }),
            );
            self.save_records(&rec)?;
        }
        *self.maps_cache.lock().unwrap() = None;
        Ok(())
    }

    /// map name -> author, for maps installed from here.
    pub fn authors(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for r in self.records().values() {
            let author = s(&r["author"]);
            for m in r["maps"].as_array().into_iter().flatten() {
                out.insert(s(m).to_lowercase(), author.clone());
            }
        }
        out
    }

    /// Once: drops pictures earlier versions found by searching, which sometimes took a
    /// similarly named map's page. Pictures that came with installed maps stay; the
    /// rest are looked up again with the stricter search.
    pub fn forget_old_lookups(&self) {
        let marker = self.shots.join(LOOKUPS_CLEARED);
        if marker.exists() || !self.shots.is_dir() {
            return;
        }
        let installed: std::collections::HashSet<String> = self
            .records()
            .values()
            .flat_map(|r| r["maps"].as_array().cloned().unwrap_or_default())
            .filter_map(|m| m.as_str().map(|m| m.to_lowercase()))
            .collect();
        for e in std::fs::read_dir(&self.shots).into_iter().flatten().flatten() {
            let p = e.path();
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if p.extension().map(|x| x == "jpg").unwrap_or(false) && !installed.contains(&stem) {
                let _ = std::fs::remove_file(&p);
            }
        }
        let _ = std::fs::write(marker, "");
    }

    pub fn shot_for(&self, name: &str) -> Option<PathBuf> {
        let p = self.shots.join(format!("{}.jpg", name.to_lowercase()));
        p.is_file().then_some(p)
    }

    /// Everything installed from GameBanana (or from a file) into this game: maps and
    /// the rest, newest first, with what's needed to remove each.
    pub fn installed_list(&self) -> Vec<Value> {
        let mut out: Vec<Value> = self
            .records()
            .into_iter()
            .filter_map(|(id, r)| {
                let maps = r["maps"].as_array().cloned().unwrap_or_default();
                (!maps.is_empty()).then(|| {
                    let m = id.parse::<i64>().unwrap_or(0);
                    json!({"mod": m, "section": "maps", "name": r["name"], "author": if m < 0 { Value::Null } else { r["author"].clone() },
                           "url": r["url"], "date": r["date"], "maps": maps, "local": m < 0})
                })
            })
            .collect();
        for (key, r) in self.content.for_game(&self.game_dir) {
            let id = content::key_id(&key);
            out.push(json!({"mod": id, "section": r["section"], "name": r["name"], "author": if id < 0 { Value::Null } else { r["author"].clone() },
                            "url": r["url"], "date": r["date"], "local": id < 0, "files": r["files"].as_array().map(|f| f.len()).unwrap_or(0)}));
        }
        if self.server {
            for it in crate::servercontent::load(&self.game_dir) {
                out.push(json!({"mod": it["mod"], "section": it["section"], "name": it["name"], "author": it["author"], "url": it["url"],
                                "date": it["date"], "local": false, "files": it["files"].as_array().map(|f| f.len()).unwrap_or(0),
                                "who": it["who"], "on": it["on"], "server": true}));
            }
        }
        out.sort_by_key(|v| std::cmp::Reverse(v["date"].as_i64().unwrap_or(0)));
        out
    }

    // ---- pictures for maps that came without one ------------------------
    pub fn misses(&self) -> Map<String, Value> {
        std::fs::read_to_string(self.shots.join(MISS_FILE))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// True if GameBanana had no picture for this map in the last week.
    pub fn missed_recently(misses: &Map<String, Value>, name: &str) -> bool {
        sys::now() - misses.get(&name.to_lowercase()).and_then(|v| v.as_f64()).unwrap_or(0.0) < MISS_DAYS * 86400.0
    }

    /// Asks GameBanana for a map's picture in the background, so a picture request never
    /// waits on its searches (a grid of them would take every connection the page has).
    /// Each map once, a few at a time; when one is found the map list says so. True while
    /// a lookup for it is waiting or under way.
    pub fn find_shot_later(self: &Arc<Self>, name: &str) -> bool {
        let name = name.to_lowercase();
        if self.shot_for(&name).is_some() || MapStore::missed_recently(&self.misses(), &name) {
            return false;
        }
        let mut later = self.later.lock().unwrap_or_else(|e| e.into_inner());
        if !later.wanted.insert(name.clone()) {
            return true;
        }
        // newest first: the pictures on screen now are the ones asked for last
        later.queue.push_front(name);
        self.start_workers(&mut later);
        true
    }

    /// Queues the lookups for a whole map list behind the ones asked for on screen, so
    /// the pictures fill in without scrolling to every map first. `names` are the maps
    /// the list marked "lookup".
    pub fn find_shots_soon(self: &Arc<Self>, names: Vec<String>) {
        let mut later = self.later.lock().unwrap_or_else(|e| e.into_inner());
        for name in names {
            if later.wanted.insert(name.clone()) {
                later.queue.push_back(name);
            }
        }
        self.start_workers(&mut later);
    }

    fn start_workers(self: &Arc<Self>, later: &mut Later) {
        while later.workers < LOOKUP_WORKERS.min(later.queue.len()) {
            later.workers += 1;
            let store = self.clone();
            std::thread::spawn(move || store.lookup_worker());
        }
    }

    fn lookup_worker(&self) {
        loop {
            let name = {
                let mut later = self.later.lock().unwrap_or_else(|e| e.into_inner());
                match later.queue.pop_front() {
                    Some(n) => n,
                    None => {
                        later.workers -= 1;
                        return;
                    }
                }
            };
            // the cached map list said "lookup": now it's the screenshot, or the radar
            // overview (a card shows the map's name when there's no radar either)
            let kind = if self.find_shot(&name).is_some() { "shot" } else { "radar" };
            if let Some(list) = self.maps_cache.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                for m in list.iter_mut().filter(|m| m["name"].as_str().is_some_and(|n| n.to_lowercase() == name)) {
                    m["thumb"] = json!(kind);
                }
            }
            self.later.lock().unwrap_or_else(|e| e.into_inner()).wanted.remove(&name);
        }
    }

    /// Looks for the map on GameBanana and saves its first screenshot.
    ///
    /// Only takes a page whose title is the map's own name, so a search for "aim_map"
    /// doesn't pick up "aim_map_usp". Common names match hundreds of uploads, so it
    /// reads a few pages of results. Misses are remembered for a week so GameBanana
    /// isn't asked again on every visit.
    pub fn find_shot(&self, name: &str) -> Option<PathBuf> {
        let name = name.to_lowercase();
        if let Some(p) = self.shot_for(&name) {
            return Some(p);
        }
        let missed_at = {
            let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
            self.misses().get(&name).and_then(|v| v.as_f64()).unwrap_or(0.0)
        };
        if sys::now() - missed_at < MISS_DAYS * 86400.0 {
            return None;
        }
        {
            let _slot = self.lookups.acquire();
            let alnum = |t: &str| t.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect::<String>();
            let mut found: Option<String> = None;
            let mut close: Option<String> = None;
            // CZ remakes (de_dust_cz) share the original map's page
            let mut wants = vec![name.clone()];
            if let Some(base) = name.strip_suffix("_cz") {
                wants.push(base.to_string());
            }
            for want in wants {
                let key = alnum(&want);
                for page in 1..=SHOT_PAGES {
                    let d = match get_json(
                        "/Mod/Index",
                        &[
                            ("_nPage", page.to_string()),
                            ("_nPerpage", "50".into()),
                            ("_aFilters[Generic_Category]", MAPS_CATEGORY.to_string()),
                            ("_aFilters[Generic_Name]", format!("contains,{want}")),
                        ],
                    ) {
                        Ok(d) => d,
                        Err(_) => return None, // offline or GameBanana busy: try again next time
                    };
                    let records = d["_aRecords"].as_array().cloned().unwrap_or_default();
                    for r in &records {
                        let title = s(&r["_sName"]).trim().to_lowercase();
                        // "aim_map" itself beats "Aim_Map Night Version", which only counts if nothing better turns up
                        if alnum(&title) == key {
                            if let Some(url) = image(&r["_aPreviewMedia"], "_sFile530") {
                                found = Some(url);
                                break;
                            }
                        }
                        // "aim_map (CS 1.6)" or "[CZ] de_dust2_cz" also count, but not "Aim_Map Night Version"
                        let words: Vec<String> = title
                            .split(|c: char| c.is_whitespace() || "[](),|".contains(c))
                            .map(alnum)
                            .filter(|w| !w.is_empty())
                            .collect();
                        let only_tags = words.iter().all(|w| *w == key || TITLE_TAGS.contains(&w.as_str()));
                        if only_tags && words.iter().any(|w| *w == key) && close.is_none() {
                            close = image(&r["_aPreviewMedia"], "_sFile530");
                        }
                    }
                    if found.is_some() || records.len() < 50 {
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            let found = found.or(close);
            if let Some(url) = found {
                let data = get_bytes(&url, 4 * 1024 * 1024)?;
                // (atomic: a thumbnail request meanwhile never gets half a picture)
                sys::write_atomic(&self.shots.join(format!("{name}.jpg")), &data).ok()?;
                return self.shot_for(&name);
            }
        }
        let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = self.misses();
        m.insert(name, json!(sys::now()));
        let _ = std::fs::write(self.shots.join(MISS_FILE), serde_json::to_string(&m).unwrap_or_default());
        None
    }

    // ---- browsing -------------------------------------------------------
    /// One page of a section (maps, skins, sounds...). `category` None means the section's first.
    pub fn browse(&self, section: &str, category: Option<i64>, sort: &str, query: &str, page: i64) -> Result<Value, StoreError> {
        let sec = content::section(section).ok_or_else(|| StoreError::Bad("unknown section".into()))?;
        let category = category.unwrap_or(sec.cats[0].0);
        if !sec.cats.iter().any(|(c, _)| *c == category) {
            return Err(StoreError::Bad("unknown category".into()));
        }
        let page = page.clamp(1, 500);
        let mut params = vec![
            ("_nPage", page.to_string()),
            ("_nPerpage", PER_PAGE.to_string()),
            ("_sSort", sort_key(sort).to_string()),
        ];
        if category == 0 {
            params.push(("_aFilters[Generic_Game]", content::CS16_GAME.to_string()));
        } else {
            params.push(("_aFilters[Generic_Category]", category.to_string()));
        }
        let query: String = query.trim().chars().take(60).collect();
        if !query.is_empty() {
            params.push(("_aFilters[Generic_Name]", format!("contains,{query}")));
        }
        let d = get_json(&format!("/{}/Index", sec.kind), &params)?;
        let is_maps = sec.key == "maps";
        let installed = if is_maps { self.records() } else { self.content.for_game(&self.game_dir) };
        let have: HashSet<String> = if is_maps { self.installed_map_names().iter().map(|m| m.to_lowercase()).collect() } else { HashSet::new() };
        let mut items = Vec::new();
        for r in d["_aRecords"].as_array().into_iter().flatten() {
            let id = r["_idRow"].clone();
            let Some(n) = id.as_i64() else { continue };
            let key = if is_maps { n.to_string() } else { content::record_key(sec, n, &self.game_dir) };
            let name = s(&r["_sName"]);
            let guess = name.trim().split(|c: char| c.is_whitespace() || c == '[' || c == '(').next().unwrap_or("").to_lowercase();
            let kind = r["_aSubCategory"]["_sName"].as_str().or(r["_aRootCategory"]["_sName"].as_str()).unwrap_or("");
            items.push(json!({
                "id": id,
                "name": name,
                "author": s(&r["_aSubmitter"]["_sName"]),
                "type": kind,
                "image": image(&r["_aPreviewMedia"], "_sFile220"),
                "audio": r["_aPreviewMedia"]["_aMetadata"]["_sAudioUrl"].as_str(),
                "likes": r.get("_nLikeCount").cloned().unwrap_or(json!(0)),
                "views": r.get("_nViewCount").cloned().unwrap_or(json!(0)),
                "added": r.get("_tsDateAdded").cloned().unwrap_or(json!(0)),
                "installed": installed.contains_key(&key) || have.contains(&guess),
            }));
        }
        let meta = &d["_aMetadata"];
        let cats = |sec: &Section| sec.cats.iter().map(|(i, n)| json!([i, n])).collect::<Vec<_>>();
        Ok(json!({
            "items": items,
            "total": meta.get("_nRecordCount").cloned().unwrap_or(json!(0)),
            "done": meta.get("_bIsComplete").cloned().unwrap_or(json!(true)),
            "page": page,
            "section": sec.key,
            "category": category,
            "categories": cats(sec),
            "sections": content::SECTIONS.iter().map(|x| json!([x.key, x.label, cats(x)])).collect::<Vec<_>>(),
        }))
    }

    /// GameBanana's own listing of what's inside an uploaded archive.
    fn archive_listing(file_id: i64) -> Option<Vec<String>> {
        let r = sys::http()
            .get("https://api.gamebanana.com/Core/Item/Data")
            .query(&[("itemtype", "File".to_string()), ("itemid", file_id.to_string()), ("fields", "aFlattenedFileList()".to_string())])
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(10))
            .send()
            .ok()?;
        let v: Value = serde_json::from_str(&r.text().ok()?).ok()?;
        Some(v.get(0)?.as_array()?.iter().filter_map(|x| x.as_str().map(String::from)).collect())
    }

    /// The game folders whose files a download can replace: the game, then CZ's fallback (cstrike).
    fn game_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![self.game_dir.clone()];
        if self.cstrike != self.game_dir {
            v.push(self.cstrike.clone());
        }
        v
    }

    pub fn details(&self, section: &str, mod_id: i64) -> Result<Value, StoreError> {
        let sec = content::section(section).ok_or_else(|| StoreError::Bad("unknown section".into()))?;
        let d = get_json(&format!("/{}/{mod_id}/ProfilePage", sec.kind), &[])?;
        let is_maps = sec.key == "maps";
        let rec = if is_maps {
            self.records().get(&mod_id.to_string()).cloned()
        } else if self.server && (sec.key == "skins" || sec.key == "players") {
            crate::servercontent::load(&self.game_dir).into_iter().find(|i| i["mod"].as_i64() == Some(mod_id) && i["section"] == sec.key)
        } else {
            self.content.for_game(&self.game_dir).get(&content::record_key(sec, mod_id, &self.game_dir)).cloned()
        };
        let images: Vec<String> = d["_aPreviewMedia"]["_aImages"]
            .as_array()
            .into_iter()
            .flatten()
            .take(6)
            .map(|i| {
                let f = i.get("_sFile530").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| s(&i["_sFile"]));
                format!("{}/{}", s(&i["_sBaseUrl"]), f)
            })
            .collect();
        // for skins and the like: what each file would put in the game, and what it would replace
        let existing = if is_maps { None } else { Some(content::Existing::scan(&self.game_dirs())) };
        let owners = if is_maps { HashMap::new() } else { self.content.owners(&self.game_dir) };
        let mut all_replaces: Vec<String> = Vec::new();
        let files: Vec<Value> = d["_aFiles"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(n, f)| {
                let name = s(&f["_sFile"]);
                let low = name.to_lowercase();
                let av = s(&f["_sAvResult"]).to_lowercase();
                let single = [".mdl", ".wav", ".spr", ".wad", ".bmp", ".tga"].iter().any(|e| low.ends_with(e));
                let supported = archive::is_archive(&low) || if is_maps { low.ends_with(".bsp") } else { single };
                let mut v = json!({
                    "id": f["_idRow"], "file": name, "size": f.get("_nFilesize").cloned().unwrap_or(json!(0)),
                    "downloads": f.get("_nDownloadCount").cloned().unwrap_or(json!(0)),
                    "added": f.get("_tsDateAdded").cloned().unwrap_or(json!(0)),
                    "supported": supported,
                    "virus": !(av.is_empty() || av == "clean"),
                });
                if let (Some(ex), true, Some(fid)) = (&existing, supported && n < 6, f["_idRow"].as_i64()) {
                    let listing = if single { Some(vec![name.clone()]) } else { Self::archive_listing(fid) };
                    match listing {
                        Some(list) => {
                            let rels: Vec<Vec<String>> = list.iter().map(|r| archive::listed_parts(r)).collect();
                            let (plan, _) = content::plan(sec.key, &rels, ex);
                            let dests: Vec<String> = plan.into_iter().map(|(_, d)| d).collect();
                            let dirs = self.game_dirs();
                            let replaces: Vec<String> = dests
                                .iter()
                                .filter_map(|d| dirs.iter().map(|g| content::resolve(g, d)).find(|r| r.2).map(|r| r.1))
                                .collect();
                            let conflicts: Vec<Value> = replaces
                                .iter()
                                .filter_map(|r| owners.get(&r.to_lowercase()).map(|(k, n)| json!({"path": r, "mod": k, "name": n})))
                                .collect();
                            for r in &replaces {
                                if !all_replaces.contains(r) {
                                    all_replaces.push(r.clone());
                                }
                            }
                            v["contents"] = json!(dests);
                            v["replaces"] = json!(replaces);
                            v["conflicts"] = json!(conflicts);
                            // nested archives can't be looked into before downloading
                            v["listed"] = json!(!list.iter().any(|r| archive::is_archive(r)));
                        }
                        None => v["listed"] = json!(false),
                    }
                }
                v
            })
            .collect();
        Ok(json!({
            "id": mod_id, "section": sec.key, "name": s(&d["_sName"]), "author": s(&d["_aSubmitter"]["_sName"]),
            "type": s(&d["_aCategory"]["_sName"]), "text": plain(&s(&d["_sText"]), 1200),
            "images": images, "audio": d["_aPreviewMedia"]["_aMetadata"]["_sAudioUrl"].as_str(),
            "files": files, "replaces": all_replaces, "downloads": d.get("_nDownloadCount").cloned().unwrap_or(json!(0)),
            "likes": d.get("_nLikeCount").cloned().unwrap_or(json!(0)), "url": s(&d["_sProfileUrl"]),
            "installed": rec,
        }))
    }

    fn installed_map_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for d in [self.cstrike.join("maps"), self.game_dir.join("maps")] {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let f = e.file_name().to_string_lossy().to_string();
                if f.to_lowercase().ends_with(".bsp") {
                    names.push(f[..f.len() - 4].to_string());
                }
            }
        }
        names
    }

    // ---- installing -----------------------------------------------------
    /// Installs a map download (kept for setup: same as start_section_install("maps", ...)).
    pub fn start_install(self: &Arc<Self>, mod_id: i64, file_id: i64) -> String {
        self.start_section_install("maps", mod_id, file_id).unwrap_or_default()
    }

    pub fn start_section_install(self: &Arc<Self>, section: &str, mod_id: i64, file_id: i64) -> Result<String, StoreError> {
        let sec = content::section(section).ok_or_else(|| StoreError::Bad("unknown section".into()))?;
        let job_id = {
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(j) = jobs.values().find(|j| {
                j.mod_id == mod_id && j.section == sec.key && ["queued", "downloading", "unpacking"].contains(&j.state.as_str())
            }) {
                return Ok(j.id.clone());
            }
            let id = format!("{}-{mod_id}-{file_id}-{}", sec.key, sys::now() as i64);
            jobs.insert(
                id.clone(),
                Job {
                    id: id.clone(), mod_id, file: file_id, section: sec.key.into(), state: "queued".into(), got: 0, size: 0,
                    message: "Waiting".into(), maps: vec![], kept: vec![], skipped: vec![], replaced: vec![], ended: None, local: None,
                },
            );
            id
        };
        let me = self.clone();
        let id = job_id.clone();
        me.spawn_install(id);
        Ok(job_id)
    }

    /// Installs a file from the player's computer (a .bsp, a sound, a model, or an
    /// archive of them) the same way as a GameBanana download: checked, recorded, and
    /// removable later. It gets a negative id so it can't clash with GameBanana's.
    pub fn start_local_install(self: &Arc<Self>, section: &str, path: &Path) -> Result<String, StoreError> {
        let sec = content::section(section).ok_or_else(|| StoreError::Bad("unknown section".into()))?;
        let meta = std::fs::metadata(path).map_err(|_| StoreError::Bad("That file isn't there.".into()))?;
        if !meta.is_file() || meta.len() > MAX_DOWNLOAD {
            return Err(StoreError::Bad("That isn't a file the app can add (or it's over 1.5 GB).".into()));
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let ext = archive::ext_of(&name).to_lowercase();
        if !(archive::is_archive(&name) || local_single(sec.key).contains(&ext.as_str())) {
            return Err(StoreError::Bad(format!("Loner-Strike can't add a {ext} file here. Pick a {} or an archive (zip, rar, 7z).", local_single(sec.key).join(", "))));
        }
        let mod_id = -(sys::now() * 1000.0) as i64;
        let id = format!("{}-local-{}", sec.key, -mod_id);
        self.jobs.lock().unwrap().insert(
            id.clone(),
            Job {
                id: id.clone(), mod_id, file: 0, section: sec.key.into(), state: "queued".into(), got: 0, size: meta.len(),
                message: format!("Adding {name}"), maps: vec![], kept: vec![], skipped: vec![], replaced: vec![], ended: None, local: Some(path.to_path_buf()),
            },
        );
        let me = self.clone();
        let job = id.clone();
        me.spawn_install(job);
        Ok(id)
    }

    /// The page info and the unpacked files for a job: from GameBanana, or the player's own file.
    fn source(&self, id: &str, section: &str, mod_id: i64, file_id: i64, tmp: &Path, single: &[&str]) -> Result<(Value, PathBuf, String), StoreError> {
        let local = self.jobs.lock().unwrap().get(id).and_then(|j| j.local.clone());
        let Some(path) = local else {
            let info = self.details(section, mod_id)?;
            let (src, fname) = self.fetch(id, &info, file_id, tmp, single)?;
            return Ok((info, src, fname));
        };
        let fname = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let ext = archive::ext_of(&fname).to_lowercase();
        self.update(id, |j| {
            j.state = "unpacking".into();
            j.message = "Unpacking".into();
        });
        let src = tmp.join("x");
        std::fs::create_dir_all(&src).map_err(|e| StoreError::Bad(e.to_string()))?;
        if archive::is_archive(&fname) {
            let copy = tmp.join(format!("download{ext}"));
            std::fs::copy(&path, &copy).map_err(|e| StoreError::Bad(format!("Couldn't read the file ({e}).")))?;
            archive::extract(&copy, &src, MAX_UNPACKED).map_err(StoreError::Bad)?;
            archive::extract_nested(&src).map_err(StoreError::Bad)?;
        } else {
            std::fs::copy(&path, src.join(&fname)).map_err(|e| StoreError::Bad(format!("Couldn't read the file ({e}).")))?;
        }
        let stem = fname.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_else(|| fname.clone());
        Ok((json!({"name": stem, "author": "", "url": null}), src, fname))
    }

    pub fn job_list(&self) -> Vec<Job> {
        let mut jobs = self.jobs.lock().unwrap();
        // finished jobs are kept for ten minutes so the panel can show the result
        let now = sys::now();
        jobs.retain(|_, j| j.ended.map(|e| now - e <= 600.0).unwrap_or(true));
        let mut v: Vec<Job> = jobs.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

/// Runs an install on its own thread. A panic fails the job instead of leaving it
    /// "unpacking" forever (and blocking that mod's installs until a restart).
    fn spawn_install(self: Arc<Self>, id: String) {
        std::thread::spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.install(&id))).is_err() {
                self.update(&id, |j| {
                    j.state = "error".into();
                    j.message = "Something went wrong installing it. Try again.".into();
                    j.ended = Some(sys::now());
                });
            }
        });
    }

        fn update(&self, id: &str, f: impl FnOnce(&mut Job)) {
        if let Some(j) = self.jobs.lock().unwrap().get_mut(id) {
            f(j);
        }
    }

    fn install(&self, id: &str) {
        let (mod_id, file_id, section) = {
            let jobs = self.jobs.lock().unwrap();
            let j = &jobs[id];
            (j.mod_id, j.file, j.section.clone())
        };
        let _ = std::fs::create_dir_all(&self.work);
        let tmp = sys::make_temp_dir(&self.work, "gb-");
        let res = match &tmp {
            Ok(tmp) if section == "maps" => self.install_into(id, mod_id, file_id, tmp).map(|(m, a, b, c)| (m, a, b, c, vec![])),
            Ok(tmp) if self.server && (section == "skins" || section == "players") => self.install_for_server(id, &section, mod_id, file_id, tmp),
            Ok(tmp) => self.install_content(id, &section, mod_id, file_id, tmp),
            Err(e) => Err(StoreError::Bad(e.to_string())),
        };
        if let Ok(tmp) = &tmp {
            let _ = std::fs::remove_dir_all(tmp);
        }
        // a server on another machine whose copy this is: what came goes up soon
        crate::servers::mark_changed_dir(&self.game_dir);
        let now = sys::now();
        match res {
            Ok((msg, maps, kept, skipped, replaced)) => self.update(id, |j| {
                j.replaced = replaced;
                j.state = "done".into();
                j.message = msg;
                j.maps = maps;
                j.kept = kept;
                j.skipped = skipped;
                j.ended = Some(now);
            }),
            Err(e) => self.update(id, |j| {
                j.state = "error".into();
                j.message = e.message();
                j.ended = Some(now);
            }),
        }
    }

    #[allow(clippy::type_complexity)]
    fn install_into(&self, id: &str, mod_id: i64, file_id: i64, tmp: &Path) -> Result<(String, Vec<String>, Vec<String>, Vec<String>), StoreError> {
        let bad = |m: &str| StoreError::Bad(m.to_string());
        let (info, src, fname) = self.source(id, "maps", mod_id, file_id, tmp, &[".bsp"])?;

        self.update(id, |j| j.message = "Copying map files".into());
        let (plan, skipped) = archive::plan(&src);
        if !plan.iter().any(|(_, d)| d.to_lowercase().ends_with(".bsp")) {
            return Err(bad("No map (.bsp) found in that download."));
        }
        let stem = |d: &str| {
            let b = d.rsplit('/').next().unwrap_or(d);
            b[..b.len() - 4].to_string()
        };
        let (mut installed, mut kept, mut maps, mut already, mut shared) = (vec![], vec![], vec![], vec![], vec![]);
        for (path, dest) in &plan {
            let full = self.cstrike.join(dest);
            if full.exists() {
                if dest.to_lowercase().ends_with(".bsp") {
                    already.push(stem(dest));
                }
                if archive::same_file(path, &full) {
                    shared.push(dest.clone());
                } else {
                    kept.push(dest.clone());
                }
                continue;
            }
            if let Some(d) = full.parent() {
                std::fs::create_dir_all(d).map_err(|e| StoreError::Bad(e.to_string()))?;
            }
            content::replace_with(path, &full).map_err(|e| StoreError::Bad(e.to_string()))?;
            installed.push(dest.clone());
            if dest.to_lowercase().ends_with(".bsp") {
                maps.push(stem(dest));
            }
        }

        if !maps.is_empty() || !installed.is_empty() {
            self.save_shot(&info, &maps);
            let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
            let mut rec = self.records();
            let old = rec.get(&mod_id.to_string()).cloned().unwrap_or(json!({}));
            let merge = |key: &str, new: &[String]| {
                let mut set: Vec<String> = old[key].as_array().into_iter().flatten().map(s).chain(new.iter().cloned()).collect();
                set.sort();
                set.dedup();
                set
            };
            let author = s(&info["author"]);
            rec.insert(
                mod_id.to_string(),
                json!({
                    "name": info["name"], "author": if author.is_empty() { "GameBanana".into() } else { author },
                    "url": info["url"], "file": fname, "date": sys::now() as i64,
                    "maps": merge("maps", &maps), "files": merge("files", &installed), "shared": merge("shared", &shared),
                }),
            );
            self.save_records(&rec).map_err(|e| StoreError::Bad(e.to_string()))?;
            *self.maps_cache.lock().unwrap() = None;
        }
        let msg = if !maps.is_empty() {
            let mut m = format!("Installed {}", maps.join(", "));
            if !already.is_empty() {
                m += &format!(". You already had {}.", already.join(", "));
            }
            m
        } else if !already.is_empty() {
            format!("You already have {}. Nothing was replaced.", already.join(", "))
        } else {
            "Nothing new to install.".into()
        };
        Ok((msg, maps, kept, skipped))
    }

    fn download(&self, id: &str, file_id: i64, out: &Path, size: u64) -> Result<(), StoreError> {
        use std::io::{Read, Write};
        self.update(id, |j| {
            j.state = "downloading".into();
            j.message = "Downloading".into();
            j.size = size;
        });
        let mut r = sys::http()
            .get(format!("https://gamebanana.com/dl/{file_id}"))
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(60 * 60))
            .send()
            .map_err(|_| StoreError::Net)?;
        let host = r.url().host_str().unwrap_or("").to_lowercase();
        if !DOWNLOAD_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}"))) {
            return Err(StoreError::Bad(format!("The download was redirected somewhere unexpected ({host}).")));
        }
        if !r.status().is_success() {
            return Err(StoreError::Net);
        }
        let mut f = std::fs::File::create(out).map_err(|e| StoreError::Bad(e.to_string()))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut got: u64 = 0;
        loop {
            let n = r.read(&mut buf).map_err(|_| StoreError::Net)?;
            if n == 0 {
                break;
            }
            got += n as u64;
            if got > MAX_DOWNLOAD {
                return Err(StoreError::Bad("That download is over 1.5 GB.".into()));
            }
            f.write_all(&buf[..n]).map_err(|e| StoreError::Bad(e.to_string()))?;
            self.update(id, |j| j.got = got);
        }
        Ok(())
    }

    fn save_shot(&self, info: &Value, maps: &[String]) {
        let Some(url) = info["images"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()) else { return };
        // a missing picture isn't worth failing the install over
        if let Some(data) = get_bytes(url, 4 * 1024 * 1024) {
            for m in maps {
                let _ = sys::write_atomic(&self.shots.join(format!("{}.jpg", m.to_lowercase())), &data);
            }
        }
    }

    /// Downloads and unpacks a file into tmp/x (shared by maps and the rest).
    fn fetch(&self, id: &str, info: &Value, file_id: i64, tmp: &Path, single: &[&str]) -> Result<(PathBuf, String), StoreError> {
        let f = info["files"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|x| x["id"].as_i64() == Some(file_id))
            .cloned()
            .ok_or_else(|| StoreError::Bad("That file is no longer on GameBanana.".into()))?;
        let fname = s(&f["file"]);
        if f["supported"] != json!(true) {
            return Err(StoreError::Bad(format!("{fname} isn't an archive the app can open.")));
        }
        let size = f["size"].as_u64().unwrap_or(0);
        if size > MAX_DOWNLOAD {
            return Err(StoreError::Bad("That download is over 1.5 GB.".into()));
        }
        let ext = archive::ext_of(&fname).to_lowercase();
        let download = tmp.join(format!("download{ext}"));
        self.download(id, file_id, &download, size)?;
        self.update(id, |j| {
            j.state = "unpacking".into();
            j.message = "Unpacking".into();
        });
        let src = tmp.join("x");
        std::fs::create_dir_all(&src).map_err(|e| StoreError::Bad(e.to_string()))?;
        if single.contains(&ext.as_str()) {
            // only the file name from GameBanana, never a path
            let base = Path::new(&fname.replace('\\', "/")).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| format!("file{ext}"));
            std::fs::rename(&download, src.join(base)).map_err(|e| StoreError::Bad(e.to_string()))?;
        } else {
            archive::extract(&download, &src, MAX_UNPACKED).map_err(StoreError::Bad)?;
            archive::extract_nested(&src).map_err(StoreError::Bad)?;
        }
        Ok((src, fname))
    }

    /// A weapon skin or player model for a dedicated server: under its own names, for
    /// gm_content.amxx to swap in (see servercontent.rs).
    #[allow(clippy::type_complexity)]
    fn install_for_server(&self, id: &str, section: &str, mod_id: i64, file_id: i64, tmp: &Path) -> Result<(String, Vec<String>, Vec<String>, Vec<String>, Vec<String>), StoreError> {
        let (info, src, _) = self.source(id, section, mod_id, file_id, tmp, &[".mdl"])?;
        self.update(id, |j| j.message = "Adding it to the server".into());
        let files = archive::walk(&src);
        let info = json!({"name": info["name"], "author": info["author"], "url": info["url"]});
        let placed = crate::servercontent::add(&self.game_dir, section, mod_id, &info, &files).map_err(StoreError::Bad)?;
        let what = if section == "skins" { "Skin" } else { "Player model" };
        Ok((format!("{what} added to this server. Players download it when they join, from the next map."), vec![], placed, vec![], vec![]))
    }

    #[allow(clippy::type_complexity)]
    fn install_content(&self, id: &str, section: &str, mod_id: i64, file_id: i64, tmp: &Path) -> Result<(String, Vec<String>, Vec<String>, Vec<String>, Vec<String>), StoreError> {
        let sec = content::section(section).ok_or_else(|| StoreError::Bad("unknown section".into()))?;
        let key = content::record_key(sec, mod_id, &self.game_dir);
        if self.content.for_game(&self.game_dir).contains_key(&key) {
            return Err(StoreError::Bad("That's already installed. Remove it first to install another version.".into()));
        }
        let (info, src, fname) = self.source(id, section, mod_id, file_id, tmp, &[".mdl", ".wav", ".spr", ".wad", ".bmp", ".tga"])?;

        self.update(id, |j| j.message = "Copying files".into());
        let files = archive::walk(&src);
        let rels: Vec<Vec<String>> = files.iter().map(|p| archive::rel_parts(&src, p)).collect();
        let existing = content::Existing::scan(&self.game_dirs());
        let (plan, mut skipped) = content::plan(sec.key, &rels, &existing);
        // never replace what the installer or a map download put there
        let theirs = content::installer_files(&self.game_dir);
        let maps_files: HashSet<String> = if self.game_dir == self.cstrike {
            self.records()
                .values()
                .flat_map(|r| r["files"].as_array().cloned().unwrap_or_default())
                .map(|f| s(&f).to_lowercase())
                .collect()
        } else {
            HashSet::new()
        };
        let mut chosen = Vec::new();
        for (i, dest) in plan {
            let actual = content::resolve(&self.game_dir, &dest).1.to_lowercase();
            if theirs.contains(&actual) || maps_files.contains(&actual) {
                skipped.push(format!("{dest} (part of Loner-Strike or a map, so it wasn't replaced)"));
                continue;
            }
            chosen.push((files[i].clone(), dest));
        }
        if chosen.is_empty() {
            return Err(StoreError::Bad(format!("Nothing in that download fits {}.", sec.label.to_lowercase())));
        }
        let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
        let (entries, replaced) = self.content.install(&self.game_dir, &key, &chosen).map_err(StoreError::Bad)?;
        let mut rec = self.content.load();
        let seq = rec.values().filter_map(|r| r["seq"].as_i64()).max().unwrap_or(0) + 1;
        let author = s(&info["author"]);
        let count = entries.len();
        rec.insert(
            key,
            json!({
                "section": sec.key, "name": info["name"], "author": if author.is_empty() { "GameBanana".into() } else { author },
                "url": info["url"], "file": fname, "date": sys::now() as i64, "seq": seq,
                "game": self.game_dir.to_string_lossy(), "files": entries,
            }),
        );
        if let Err(e) = self.content.save(&rec) {
            // without a record it couldn't be undone later, so undo it now
            let _ = self.content.uninstall_entry(&self.game_dir, &rec[&content::record_key(sec, mod_id, &self.game_dir)]);
            return Err(StoreError::Bad(e.to_string()));
        }
        let msg = match (count, replaced.len()) {
            (0, _) => "Nothing new to install: you already have exactly these files.".to_string(),
            (n, 0) => format!("Installed {} ({n} file{}).", s(&info["name"]), if n == 1 { "" } else { "s" }),
            (n, r) => format!("Installed {} ({n} file{}, replacing {r}; the originals come back if you remove it).", s(&info["name"]), if n == 1 { "" } else { "s" }),
        };
        skipped.truncate(40);
        Ok((msg, vec![], vec![], skipped, replaced))
    }

    // ---- uninstalling ---------------------------------------------------
    /// Removes a map download (kept for setup): the maps it removed.
    pub fn uninstall(&self, mod_id: i64) -> Result<Vec<String>, StoreError> {
        self.uninstall_maps(mod_id)
    }

    /// Removes a download: {"maps": [...]} for a map download, {"maps": [], "files": [...]} for the rest.
    pub fn uninstall_section(&self, section: &str, mod_id: i64) -> Result<Value, StoreError> {
        let sec = content::section(section).ok_or_else(|| StoreError::Bad("unknown section".into()))?;
        if sec.key == "maps" {
            return self.uninstall_maps(mod_id).map(|m| json!({"maps": m}));
        }
        if self.server && (sec.key == "skins" || sec.key == "players") {
            let files = crate::servercontent::remove(&self.game_dir, sec.key, mod_id).map_err(StoreError::Bad)?;
            return Ok(json!({"maps": [], "files": files}));
        }
        let key = content::record_key(sec, mod_id, &self.game_dir);
        if !self.content.for_game(&self.game_dir).contains_key(&key) {
            return Err(StoreError::Bad("That download isn't installed from the app.".into()));
        }
        let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
        let r = self.content.uninstall(&key).map_err(StoreError::Bad)?;
        let files: Vec<String> = r["files"].as_array().into_iter().flatten().map(|f| s(&f["path"])).collect();
        Ok(json!({"maps": [], "files": files}))
    }

    fn uninstall_maps(&self, mod_id: i64) -> Result<Vec<String>, StoreError> {
        let maps = {
            let _g = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
            let mut rec = self.records();
            let r = rec.remove(&mod_id.to_string()).ok_or_else(|| StoreError::Bad("That download isn't installed from the app.".into()))?;
            // files another download also needs stay
            let needed: HashSet<String> = rec
                .values()
                .flat_map(|o| o["files"].as_array().into_iter().flatten().chain(o["shared"].as_array().into_iter().flatten()))
                .map(|f| s(f).to_lowercase())
                .collect();
            for dest in r["files"].as_array().into_iter().flatten().map(s) {
                if needed.contains(&dest.to_lowercase()) || crate::maps::is_stock_file(&dest) {
                    continue;
                }
                if let Some(full) = inside(&self.cstrike, &dest) {
                    if full.is_file() {
                        let _ = std::fs::remove_file(full);
                    }
                }
            }
            let maps: Vec<String> = r["maps"].as_array().into_iter().flatten().map(s).collect();
            for m in &maps {
                if !archive::safe_map(m) {
                    continue;
                }
                // navigation the bots learned for it, and the cached picture
                for p in [
                    self.cstrike.join("maps").join(format!("{m}.nav")),
                    self.game_dir.join("maps").join(format!("{m}.nav")),
                    self.shots.join(format!("{}.jpg", m.to_lowercase())),
                ] {
                    if p.is_file() {
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
            self.save_records(&rec).map_err(|e| StoreError::Bad(e.to_string()))?;
            maps
        };
        *self.maps_cache.lock().unwrap() = None;
        Ok(maps)
    }
}

/// base/rel, only if rel stays inside base (no .., no absolute paths).
fn inside(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            p if p.contains(':') => return None,
            p => out.push(p),
        }
    }
    (out != base).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_text() {
        assert_eq!(plain("<p>Hello&nbsp;<b>world</b></p><br/>A &amp; B", 1200), "Hello\u{a0}world\n\nA & B");
        assert_eq!(plain("abcdef", 3), "abc…");
    }

    #[test]
    fn uninstall_stays_inside_cstrike() {
        let base = Path::new("/game/cstrike");
        assert_eq!(inside(base, "maps/a.bsp"), Some(base.join("maps").join("a.bsp")));
        assert_eq!(inside(base, "../liblist.gam"), None);
        assert_eq!(inside(base, "maps/../../x"), None);
        assert_eq!(inside(base, "C:/Windows/x"), None);
        assert_eq!(inside(base, ""), None);
    }

    #[test]
    fn uninstall_keeps_shared_files() {
        let root = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "store-").unwrap();
        let hl = root.join("hl");
        let app = root.join("app");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::create_dir_all(&app).unwrap();
        for f in ["maps/de_a.bsp", "maps/de_a.nav", "shared.wad", "maps/de_b.bsp"] {
            std::fs::write(hl.join("cstrike").join(f), b"x").unwrap();
        }
        std::fs::write(hl.join("liblist.gam"), b"keep").unwrap();
        let rec = json!({
            "1": {"name": "A", "author": "x", "maps": ["de_a"], "files": ["maps/de_a.bsp", "shared.wad", "../liblist.gam"], "shared": []},
            "2": {"name": "B", "author": "y", "maps": ["de_b"], "files": ["maps/de_b.bsp"], "shared": ["shared.wad"]},
        });
        std::fs::write(app.join("installed_maps.json"), rec.to_string()).unwrap();
        let store = MapStore::new(&hl, &hl.join("czero"), &app, Arc::new(Mutex::new(None)));
        assert_eq!(store.authors().get("de_b").map(String::as_str), Some("y"));
        assert_eq!(store.uninstall_section("maps", 1).unwrap(), json!({"maps": ["de_a"]}));
        assert!(!hl.join("cstrike/maps/de_a.bsp").exists());
        assert!(!hl.join("cstrike/maps/de_a.nav").exists());
        assert!(hl.join("cstrike/shared.wad").exists());
        assert!(hl.join("liblist.gam").exists());
        assert!(store.uninstall_section("maps", 1).is_err());
        assert!(store.records().contains_key("2"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A map file from the player's computer: placed, recorded with a local id, listed, and removable.
    #[test]
    fn add_map_from_a_file() {
        let root = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "local-").unwrap();
        let hl = root.join("hl");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::create_dir_all(hl.join("czero/maps")).unwrap();
        let file = root.join("de_mytest.bsp");
        std::fs::write(&file, b"BSP30 not really a map").unwrap();
        let store = Arc::new(MapStore::new(&hl, &hl.join("czero"), &root.join("app"), Arc::new(Mutex::new(None))));
        assert!(store.start_local_install("maps", &root.join("nope.exe")).is_err(), "only maps and archives");
        let job = store.start_local_install("maps", &file).unwrap();
        let j = loop {
            std::thread::sleep(Duration::from_millis(100));
            let j = store.job_list().into_iter().find(|j| j.id == job).unwrap();
            if j.ended.is_some() {
                break j;
            }
        };
        assert_eq!(j.state, "done", "{}", j.message);
        assert!(hl.join("cstrike/maps/de_mytest.bsp").is_file());
        let listed = store.installed_list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["local"], true);
        let m = listed[0]["mod"].as_i64().unwrap();
        assert!(m < 0);
        store.uninstall_section("maps", m).unwrap();
        assert!(!hl.join("cstrike/maps/de_mytest.bsp").exists());
        assert!(store.installed_list().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Talks to GameBanana for real: `cargo test -- --ignored gamebanana_live`
    #[test]
    #[ignore]
    fn gamebanana_live() {
        let root = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "live-").unwrap();
        let hl = root.join("hl");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        let store = Arc::new(MapStore::new(&hl, &hl.join("czero"), &root.join("app"), Arc::new(Mutex::new(None))));
        let page = store.browse("maps", Some(5497), "popular", "", 1).unwrap();
        let items = page["items"].as_array().unwrap();
        assert!(!items.is_empty());
        // the smallest supported file among the first few maps, one zip and one rar/7z if we can
        let mut tried = 0;
        for it in items.iter().take(12) {
            let d = store.details("maps", it["id"].as_i64().unwrap()).unwrap();
            let Some(f) = d["files"].as_array().unwrap().iter().filter(|f| f["supported"] == true && f["size"].as_u64().unwrap_or(0) < 8_000_000).next() else { continue };
            let job = store.start_section_install("maps", d["id"].as_i64().unwrap(), f["id"].as_i64().unwrap()).unwrap();
            let j = loop {
                std::thread::sleep(Duration::from_millis(300));
                let j = store.job_list().into_iter().find(|j| j.id == job).unwrap();
                if j.ended.is_some() {
                    break j;
                }
            };
            eprintln!("{} [{}]: {} {} maps={:?} skipped={:?}", d["name"], f["file"], j.state, j.message, j.maps, j.skipped.len());
            if j.state == "done" && !j.maps.is_empty() {
                assert!(hl.join("cstrike/maps").join(format!("{}.bsp", j.maps[0])).exists());
                assert!(store.shot_for(&j.maps[0]).is_some());
                let removed = store.uninstall_section("maps", d["id"].as_i64().unwrap()).unwrap();
                assert_eq!(removed["maps"], json!(j.maps));
                assert!(!hl.join("cstrike/maps").join(format!("{}.bsp", j.maps[0])).exists());
            }
            tried += 1;
            if tried == 3 {
                break;
            }
        }
        assert!(tried > 0);
        assert!(store.find_shot("de_dust2").is_some() || store.find_shot("de_inferno").is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Skins, sounds, sprays, HUDs for real, into a throwaway game folder:
    /// `cargo test -- --ignored content_live --nocapture`
    #[test]
    #[ignore]
    fn content_live() {
        let root = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "clive-").unwrap();
        let hl = root.join("hl");
        let cz = hl.join("czero");
        for (p, d) in [("czero/models/v_awp.mdl", "cz awp"), ("cstrike/models/v_m4a1.mdl", "cs m4"), ("czero/tempdecal.wad", "my spray"),
                       ("cstrike/sound/weapons/awp1.wav", "awp shot"), ("czero/resource/background/800_1_a_loading.tga", "bg")] {
            std::fs::create_dir_all(hl.join(p).parent().unwrap()).unwrap();
            std::fs::write(hl.join(p), d).unwrap();
        }
        let store = Arc::new(MapStore::new(&hl, &cz, &root.join("app"), Arc::new(Mutex::new(None))));
        let wait = |job: String| loop {
            std::thread::sleep(Duration::from_millis(300));
            let j = store.job_list().into_iter().find(|j| j.id == job).unwrap();
            if j.ended.is_some() {
                break j;
            }
        };
        for sec in ["skins", "players", "sounds", "sprays", "sprites", "gui"] {
            let page = store.browse(sec, None, "popular", "", 1).unwrap();
            eprintln!("{sec}: {} items, first {:?}", page["total"], page["items"][0]["name"]);
            assert!(page["items"].as_array().unwrap().len() > 0);
        }
        // two AWP skins on top of each other, then removed in the "wrong" order
        let d = store.details("skins", 223124).unwrap();
        eprintln!("replaces {:?}, contents {:?}", d["replaces"], d["files"][0]["contents"]);
        assert!(d["replaces"].as_array().unwrap().iter().any(|r| r == "models/v_awp.mdl"));
        let fid = d["files"][0]["id"].as_i64().unwrap();
        let j = wait(store.start_section_install("skins", 223124, fid).unwrap());
        eprintln!("{} {} replaced={:?} skipped={:?}", j.state, j.message, j.replaced, j.skipped);
        assert_eq!(j.state, "done");
        assert!(j.replaced.contains(&"models/v_awp.mdl".to_string()));
        let first = std::fs::read(cz.join("models/v_awp.mdl")).unwrap();
        let other = store.browse("skins", Some(8320), "popular", "awp", 1).unwrap();
        let second = other["items"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).find(|&i| i != 223124).unwrap();
        let d2 = store.details("skins", second).unwrap();
        eprintln!("second {} conflicts {:?}", d2["name"], d2["files"][0]["conflicts"]);
        let j2 = wait(store.start_section_install("skins", second, d2["files"][0]["id"].as_i64().unwrap()).unwrap());
        eprintln!("{} {} replaced={:?}", j2.state, j2.message, j2.replaced);
        store.uninstall_section("skins", 223124).unwrap();
        if j2.replaced.iter().any(|r| r == "models/v_awp.mdl") {
            assert_ne!(std::fs::read(cz.join("models/v_awp.mdl")).unwrap(), first, "the later skin stays");
        }
        store.uninstall_section("skins", second).unwrap();
        assert_eq!(std::fs::read_to_string(cz.join("models/v_awp.mdl")).unwrap(), "cz awp");
        assert!(store.content.load().is_empty());

        // a spray, a sound pack, a menu background: install, then remove
        for (sec, id) in [("sprays", 82585), ("sounds", 31434), ("gui", 28432)] {
            let d = store.details(sec, id).unwrap();
            let j = wait(store.start_section_install(sec, id, d["files"][0]["id"].as_i64().unwrap()).unwrap());
            eprintln!("{sec} {}: {} {} replaced={} skipped={}", d["name"], j.state, j.message, j.replaced.len(), j.skipped.len());
            assert_eq!(j.state, "done", "{}", j.message);
            store.uninstall_section(sec, id).unwrap();
        }
        assert_eq!(std::fs::read_to_string(cz.join("tempdecal.wad")).unwrap(), "my spray");
        assert_eq!(std::fs::read_to_string(cz.join("resource/background/800_1_a_loading.tga")).unwrap(), "bg");
        let left: Vec<_> = archive::walk(&cz).into_iter().map(|p| p.strip_prefix(&cz).unwrap().to_string_lossy().to_string()).collect();
        eprintln!("left in czero: {left:?}");
        assert_eq!(left.len(), 3, "only the original files are left");
        let _ = std::fs::remove_dir_all(&root);
    }
}
