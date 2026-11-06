//! Get maps: browse GameBanana's Counter-Strike 1.6 maps and install them.
//! Mirrors panel/getmaps.py (same answers to the panel).
//!
//! Only map content is copied into the game (the map, its textures, models, sounds,
//! sprites, sky and radar images). Anything that could run code or change settings
//! is skipped, and files you already have are never replaced. Each install is
//! recorded in installed_maps.json so it can be uninstalled cleanly.

use crate::archive::{self, MAX_UNPACKED};
use crate::sys;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

const API: &str = "https://gamebanana.com/apiv11";
pub const MAPS_CATEGORY: i64 = 5474; // Counter-Strike 1.6 > Maps
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
    pub state: String,
    pub got: u64,
    pub size: u64,
    pub message: String,
    pub maps: Vec<String>,
    pub kept: Vec<String>,
    pub skipped: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<f64>,
}

pub struct MapStore {
    cstrike: PathBuf,
    game_dir: PathBuf,
    record_path: PathBuf,
    pub shots: PathBuf,
    work: PathBuf,
    /// Guards installed_maps.json and the misses file
    lock: Mutex<()>,
    lookups: Semaphore,
    jobs: Mutex<HashMap<String, Job>>,
    /// The game's cached map list: emptied after an install so the next list is fresh
    maps_cache: Arc<Mutex<Option<Vec<Value>>>>,
}

impl MapStore {
    pub fn new(hl: &Path, game_dir: &Path, app_dir: &Path, maps_cache: Arc<Mutex<Option<Vec<Value>>>>) -> MapStore {
        let shots = app_dir.join("cache").join("gbshots");
        let _ = std::fs::create_dir_all(&shots);
        MapStore {
            cstrike: hl.join("cstrike"),
            game_dir: game_dir.to_path_buf(),
            record_path: app_dir.join("installed_maps.json"),
            shots,
            work: app_dir.join("cache").join("downloads"),
            lock: Mutex::new(()),
            lookups: Semaphore { left: Mutex::new(3), cv: Condvar::new() },
            jobs: Mutex::new(HashMap::new()),
            maps_cache,
        }
    }

    // ---- install record -------------------------------------------------
    pub fn records(&self) -> Map<String, Value> {
        std::fs::read_to_string(&self.record_path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    fn save_records(&self, rec: &Map<String, Value>) -> std::io::Result<()> {
        if let Some(d) = self.record_path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = self.record_path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(rec).unwrap_or_default())?;
        std::fs::rename(&tmp, &self.record_path)
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

    pub fn shot_for(&self, name: &str) -> Option<PathBuf> {
        let p = self.shots.join(format!("{}.jpg", name.to_lowercase()));
        p.is_file().then_some(p)
    }

    // ---- pictures for maps that came without one ------------------------
    fn misses(&self) -> Map<String, Value> {
        std::fs::read_to_string(self.shots.join("not_found.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Looks for the map on GameBanana and saves its first screenshot.
    ///
    /// Only takes a page whose title is the map's own name, so a search for "aim_map"
    /// doesn't pick up "aim_map_usp". Misses are remembered for a month so GameBanana
    /// isn't asked again on every visit.
    pub fn find_shot(&self, name: &str) -> Option<PathBuf> {
        let name = name.to_lowercase();
        if let Some(p) = self.shot_for(&name) {
            return Some(p);
        }
        let missed_at = {
            let _g = self.lock.lock().unwrap();
            self.misses().get(&name).and_then(|v| v.as_f64()).unwrap_or(0.0)
        };
        if sys::now() - missed_at < 30.0 * 86400.0 {
            return None;
        }
        {
            let _slot = self.lookups.acquire();
            let alnum = |t: &str| t.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect::<String>();
            let mut found: Option<String> = None;
            // CZ remakes (de_dust_cz) share the original map's page
            let mut wants = vec![name.clone()];
            if let Some(base) = name.strip_suffix("_cz") {
                wants.push(base.to_string());
            }
            for want in wants {
                let d = match get_json(
                    "/Mod/Index",
                    &[
                        ("_nPage", "1".into()),
                        ("_nPerpage", "40".into()),
                        ("_aFilters[Generic_Category]", MAPS_CATEGORY.to_string()),
                        ("_aFilters[Generic_Name]", format!("contains,{want}")),
                    ],
                ) {
                    Ok(d) => d,
                    Err(_) => return None, // offline or GameBanana busy: try again next time
                };
                let key = alnum(&want);
                for r in d["_aRecords"].as_array().into_iter().flatten() {
                    let title = s(&r["_sName"]).trim().to_lowercase();
                    let words_match = title
                        .split(|c: char| c.is_whitespace() || c == '[' || c == '(' || c == ',')
                        .filter(|w| !w.is_empty())
                        .any(|w| alnum(w) == key);
                    if words_match || alnum(&title) == key {
                        found = image(&r["_aPreviewMedia"], "_sFile530");
                        if found.is_some() {
                            break;
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some(url) = found {
                let data = get_bytes(&url, 4 * 1024 * 1024)?;
                std::fs::write(self.shots.join(format!("{name}.jpg")), data).ok()?;
                return self.shot_for(&name);
            }
        }
        let _g = self.lock.lock().unwrap();
        let mut m = self.misses();
        m.insert(name, json!(sys::now()));
        let _ = std::fs::write(self.shots.join("not_found.json"), serde_json::to_string(&m).unwrap_or_default());
        None
    }

    // ---- browsing -------------------------------------------------------
    pub fn browse(&self, category: i64, sort: &str, query: &str, page: i64) -> Result<Value, StoreError> {
        if !CATEGORIES.iter().any(|(c, _)| *c == category) {
            return Err(StoreError::Bad("unknown category".into()));
        }
        let page = page.clamp(1, 500);
        let mut params = vec![
            ("_nPage", page.to_string()),
            ("_nPerpage", PER_PAGE.to_string()),
            ("_aFilters[Generic_Category]", category.to_string()),
            ("_sSort", sort_key(sort).to_string()),
        ];
        let query: String = query.trim().chars().take(60).collect();
        if !query.is_empty() {
            params.push(("_aFilters[Generic_Name]", format!("contains,{query}")));
        }
        let d = get_json("/Mod/Index", &params)?;
        let installed = self.records();
        let have: HashSet<String> = self.installed_map_names().iter().map(|m| m.to_lowercase()).collect();
        let mut items = Vec::new();
        for r in d["_aRecords"].as_array().into_iter().flatten() {
            let id = r["_idRow"].clone();
            let mid = match &id {
                Value::Number(n) => n.to_string(),
                v => s(v),
            };
            let name = s(&r["_sName"]);
            let guess = name.trim().split(|c: char| c.is_whitespace() || c == '[' || c == '(').next().unwrap_or("").to_lowercase();
            items.push(json!({
                "id": id,
                "name": name,
                "author": s(&r["_aSubmitter"]["_sName"]),
                "type": s(&r["_aSubCategory"]["_sName"]),
                "image": image(&r["_aPreviewMedia"], "_sFile220"),
                "likes": r.get("_nLikeCount").cloned().unwrap_or(json!(0)),
                "views": r.get("_nViewCount").cloned().unwrap_or(json!(0)),
                "added": r.get("_tsDateAdded").cloned().unwrap_or(json!(0)),
                "installed": installed.contains_key(&mid) || have.contains(&guess),
            }));
        }
        let meta = &d["_aMetadata"];
        Ok(json!({
            "items": items,
            "total": meta.get("_nRecordCount").cloned().unwrap_or(json!(0)),
            "done": meta.get("_bIsComplete").cloned().unwrap_or(json!(true)),
            "page": page,
            "categories": CATEGORIES.iter().map(|(i, n)| json!([i, n])).collect::<Vec<_>>(),
        }))
    }

    pub fn details(&self, mod_id: i64) -> Result<Value, StoreError> {
        let d = get_json(&format!("/Mod/{mod_id}/ProfilePage"), &[])?;
        let rec = self.records().get(&mod_id.to_string()).cloned();
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
        let files: Vec<Value> = d["_aFiles"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                let name = s(&f["_sFile"]);
                let low = name.to_lowercase();
                let av = s(&f["_sAvResult"]).to_lowercase();
                json!({
                    "id": f["_idRow"], "file": name, "size": f.get("_nFilesize").cloned().unwrap_or(json!(0)),
                    "downloads": f.get("_nDownloadCount").cloned().unwrap_or(json!(0)),
                    "added": f.get("_tsDateAdded").cloned().unwrap_or(json!(0)),
                    "supported": archive::is_archive(&low) || low.ends_with(".bsp"),
                    "virus": !(av.is_empty() || av == "clean"),
                })
            })
            .collect();
        Ok(json!({
            "id": mod_id, "name": s(&d["_sName"]), "author": s(&d["_aSubmitter"]["_sName"]),
            "type": s(&d["_aCategory"]["_sName"]), "text": plain(&s(&d["_sText"]), 1200),
            "images": images, "files": files, "downloads": d.get("_nDownloadCount").cloned().unwrap_or(json!(0)),
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
    pub fn start_install(self: &Arc<Self>, mod_id: i64, file_id: i64) -> String {
        let job_id = {
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(j) = jobs.values().find(|j| j.mod_id == mod_id && ["queued", "downloading", "unpacking"].contains(&j.state.as_str())) {
                return j.id.clone();
            }
            let id = format!("{mod_id}-{file_id}-{}", sys::now() as i64);
            jobs.insert(
                id.clone(),
                Job {
                    id: id.clone(), mod_id, file: file_id, state: "queued".into(), got: 0, size: 0,
                    message: "Waiting".into(), maps: vec![], kept: vec![], skipped: vec![], ended: None,
                },
            );
            id
        };
        let me = self.clone();
        let id = job_id.clone();
        std::thread::spawn(move || me.install(&id));
        job_id
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

    fn update(&self, id: &str, f: impl FnOnce(&mut Job)) {
        if let Some(j) = self.jobs.lock().unwrap().get_mut(id) {
            f(j);
        }
    }

    fn install(&self, id: &str) {
        let (mod_id, file_id) = {
            let jobs = self.jobs.lock().unwrap();
            let j = &jobs[id];
            (j.mod_id, j.file)
        };
        let _ = std::fs::create_dir_all(&self.work);
        let tmp = sys::make_temp_dir(&self.work, "gb-");
        let res = match &tmp {
            Ok(tmp) => self.install_into(id, mod_id, file_id, tmp),
            Err(e) => Err(StoreError::Bad(e.to_string())),
        };
        if let Ok(tmp) = &tmp {
            let _ = std::fs::remove_dir_all(tmp);
        }
        let now = sys::now();
        match res {
            Ok((msg, maps, kept, skipped)) => self.update(id, |j| {
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
        let info = self.details(mod_id)?;
        let f = info["files"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|x| x["id"].as_i64() == Some(file_id))
            .cloned()
            .ok_or_else(|| bad("That file is no longer on GameBanana."))?;
        let fname = s(&f["file"]);
        if f["supported"] != json!(true) {
            return Err(StoreError::Bad(format!("{fname} isn't an archive the app can open.")));
        }
        let size = f["size"].as_u64().unwrap_or(0);
        if size > MAX_DOWNLOAD {
            return Err(bad("That download is over 1.5 GB."));
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
        if ext == ".bsp" {
            // only the file name from GameBanana, never a path
            let base = Path::new(&fname).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "map.bsp".into());
            std::fs::rename(&download, src.join(base)).map_err(|e| StoreError::Bad(e.to_string()))?;
        } else {
            archive::extract(&download, &src, MAX_UNPACKED).map_err(StoreError::Bad)?;
            archive::extract_nested(&src).map_err(StoreError::Bad)?;
        }

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
            std::fs::copy(path, &full).map_err(|e| StoreError::Bad(e.to_string()))?;
            installed.push(dest.clone());
            if dest.to_lowercase().ends_with(".bsp") {
                maps.push(stem(dest));
            }
        }

        if !maps.is_empty() || !installed.is_empty() {
            self.save_shot(&info, &maps);
            let _g = self.lock.lock().unwrap();
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
                let _ = std::fs::write(self.shots.join(format!("{}.jpg", m.to_lowercase())), &data);
            }
        }
    }

    // ---- uninstalling ---------------------------------------------------
    pub fn uninstall(&self, mod_id: i64) -> Result<Vec<String>, StoreError> {
        let maps = {
            let _g = self.lock.lock().unwrap();
            let mut rec = self.records();
            let r = rec.remove(&mod_id.to_string()).ok_or_else(|| StoreError::Bad("That download isn't installed from the app.".into()))?;
            // files another download also needs stay
            let needed: HashSet<String> = rec
                .values()
                .flat_map(|o| o["files"].as_array().into_iter().flatten().chain(o["shared"].as_array().into_iter().flatten()))
                .map(|f| s(f).to_lowercase())
                .collect();
            for dest in r["files"].as_array().into_iter().flatten().map(s) {
                if needed.contains(&dest.to_lowercase()) {
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
        assert_eq!(store.uninstall(1).unwrap(), vec!["de_a".to_string()]);
        assert!(!hl.join("cstrike/maps/de_a.bsp").exists());
        assert!(!hl.join("cstrike/maps/de_a.nav").exists());
        assert!(hl.join("cstrike/shared.wad").exists());
        assert!(hl.join("liblist.gam").exists());
        assert!(store.uninstall(1).is_err());
        assert!(store.records().contains_key("2"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
