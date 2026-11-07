//! The map list (with favourites, hidden and recent), map titles and authors, and
//! map pictures.

use crate::game::Game;
use crate::sys;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Names people search for that the file names don't contain
const MAP_TITLES: &[(&str, &str)] = &[
    ("de_cbble", "Cobblestone"), ("de_cbble_cz", "Cobblestone"), ("cs_747", "Boeing 747 hijack"),
    ("as_oilrig", "Oil rig"), ("de_dust2", "Dust II"), ("de_dust2_cz", "Dust II"),
    ("cs_militia", "Militia"), ("de_chateau", "Chateau"),
];

/// The maps Counter-Strike 1.6 ships with (Valve's depot): stock in cstrike/maps.
/// Everything in czero/maps that no download brought is Condition Zero's own.
pub const STOCK_CS: &[&str] = &[
    "as_oilrig", "cs_747", "cs_assault", "cs_backalley", "cs_estate", "cs_havana", "cs_italy", "cs_militia",
    "cs_office", "cs_siege", "de_airstrip", "de_aztec", "de_cbble", "de_chateau", "de_dust", "de_dust2",
    "de_inferno", "de_nuke", "de_piranesi", "de_prodigy", "de_storm", "de_survivor", "de_torn", "de_train",
    "de_vertigo",
];

/// maps/<stock map>.* (never removed by an uninstall).
pub fn is_stock_file(rel: &str) -> bool {
    let r = rel.replace('\\', "/").to_lowercase();
    let Some(file) = r.strip_prefix("maps/") else { return false };
    let stem = file.split('.').next().unwrap_or("");
    !file.contains('/') && STOCK_CS.contains(&stem)
}

const FUN_PREFIXES: &[&str] = &[
    "fy_", "aim_", "awp_", "ka_", "35hp", "scout", "he_", "gg_", "zm_", "dm_", "mg_", "fun_", "knife", "arena",
    "pool_", "deathrun", "jail", "bhop", "kz_",
];

pub fn map_category(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.starts_with("de_") {
        "defuse"
    } else if n.starts_with("cs_") {
        "hostage"
    } else if n.starts_with("as_") || n.starts_with("es_") {
        "objective"
    } else if n.starts_with("surf") {
        "surf"
    } else if FUN_PREFIXES.iter().any(|p| n.starts_with(p)) {
        "arena"
    } else {
        "other"
    }
}

/// A command argument the panel may send: plain words only (the UI checks the same rule).
pub fn safe_arg(a: &str) -> bool {
    !a.is_empty() && a.len() <= 64 && a.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

fn list_file(which: &str) -> Option<&'static str> {
    match which {
        "fav" => Some("favorites.txt"),
        "hidden" => Some("hidden_maps.txt"),
        "recent" => Some("recent.txt"),
        _ => None,
    }
}

/// The map's own name for itself (worldspawn "message"), from the BSP's entity lump.
pub fn title_from_bsp(path: &Path, name: &str) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 12];
    if f.read_exact(&mut header).is_err() {
        return None; // too short: try the other game folder
    }
    let offset = i32::from_le_bytes(header[4..8].try_into().unwrap());
    let length = i32::from_le_bytes(header[8..12].try_into().unwrap());
    let mut head = vec![0u8; length.clamp(0, 4096) as usize];
    let n = if offset >= 0 && f.seek(SeekFrom::Start(offset as u64)).is_ok() { f.read(&mut head).unwrap_or(0) } else { 0 };
    let text: String = head[..n].iter().map(|&b| b as char).collect();
    static RES: OnceLock<(Regex, Regex)> = OnceLock::new();
    let (msg, junk) = RES.get_or_init(|| {
        (Regex::new(r#""message"\s+"([^"]*)""#).unwrap(), Regex::new(r"(?i)\\n.*|\s*\(press fire.*").unwrap())
    });
    let first = text.split('}').next().unwrap_or("");
    let Some(m) = msg.captures(first) else { return Some(String::new()) };
    let title = junk.replace_all(&m[1], "").to_string();
    let title = title.trim_matches(|c| c == ' ' || c == '-').to_string();
    if title.to_lowercase() == name.to_lowercase() {
        return Some(String::new());
    }
    Some(title.chars().take(60).collect())
}

fn find_in(dir: &Path, name: &str, exts: &[&str]) -> Option<PathBuf> {
    let want: HashSet<String> = exts.iter().map(|e| format!("{}{e}", name.to_lowercase())).collect();
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| want.contains(&e.file_name().to_string_lossy().to_lowercase()))
        .map(|e| e.path())
}

impl Game {
    fn game_dirs(&self) -> [PathBuf; 2] {
        [self.hl.join("czero"), self.hl.join("cstrike")]
    }

    pub fn maps(&self, refresh: bool) -> Vec<Value> {
        let list = {
            let mut cache = self.maps_cache.lock().unwrap();
            if cache.is_none() || refresh {
                *cache = Some(self.scan_maps());
            }
            cache.clone().unwrap()
        };
        let [fav, hidden, recent] = self.read_lists(["fav", "hidden", "recent"]);
        let fav: HashSet<String> = fav.iter().map(|m| m.to_lowercase()).collect();
        let hidden: HashSet<String> = hidden.iter().map(|m| m.to_lowercase()).collect();
        let recent: Vec<String> = recent.iter().map(|m| m.to_lowercase()).collect();
        list.into_iter()
            .map(|mut m| {
                let k = m["name"].as_str().unwrap_or("").to_lowercase();
                m["fav"] = json!(fav.contains(&k));
                m["hidden"] = json!(hidden.contains(&k));
                m["recent"] = json!(recent.iter().position(|r| *r == k).map(|i| i as i64).unwrap_or(-1));
                m
            })
            .collect()
    }

    /// A map list (favourites, hidden, recently played) as the server has it: the plugin
    /// changes them too (the in-game menu, every map change), so for a server on another
    /// machine they're the machine's (this computer's copy if it can't be reached).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn read_list(&self, which: &str) -> Vec<String> {
        let [l] = self.read_lists([which]);
        l
    }

    /// Several lists at once (one SSH command for a server on another machine).
    fn read_lists<const N: usize>(&self, which: [&str; N]) -> [Vec<String>; N] {
        let parse = |t: &str| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>();
        let local = |w: &str| list_file(w).and_then(|f| sys::read_latin1(&self.data.join(f)).ok()).map(|t| parse(&t)).unwrap_or_default();
        if let Some((r, dir)) = &self.remote {
            let files: Vec<&str> = which.iter().map(|w| list_file(w).unwrap_or("-")).collect();
            if let Ok(got) = crate::remote::cat_files(r, &format!("{dir}/addons/amxmodx/data/gamemodes"), &files) {
                let mut got = got.into_iter();
                return which.map(|_| got.next().flatten().map(|b| parse(&crate::rcon::latin1(&b))).unwrap_or_default());
            }
        }
        which.map(local)
    }

    pub fn set_flag(&self, which: &str, name: &str, on: bool) -> Result<(), String> {
        if !(which == "fav" || which == "hidden") || !safe_arg(name) {
            return Err("invalid map".into());
        }
        let rel = format!("addons/amxmodx/data/gamemodes/{}", list_file(which).unwrap());
        // read and written where the server keeps it (on a Linux server, the machine: a
        // failed read is an error, never an empty list written back)
        let now = crate::remote::game_file(self, &rel)?.map(|b| crate::rcon::latin1(&b)).unwrap_or_default();
        let mut items: Vec<String> = now.lines().map(|l| l.trim().to_string()).filter(|m| !m.is_empty() && m.to_lowercase() != name.to_lowercase()).collect();
        if on {
            items.insert(0, name.to_string());
        }
        let text: String = items.iter().map(|m| format!("{m}\n")).collect();
        let bytes: Vec<u8> = text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
        crate::remote::put_game_file(self, &rel, &bytes)
    }

    /// The map list, from the disk only: no network (pictures GameBanana might have
    /// are marked "lookup" and fetched when /thumb asks for them).
    fn scan_maps(&self) -> Vec<Value> {
        // name -> (file name, in czero/maps)
        let mut found: BTreeMap<String, (String, bool)> = BTreeMap::new();
        let mut navs: HashSet<String> = HashSet::new();
        for g in self.game_dirs() {
            let is_cz = g.file_name().map(|n| n == "czero").unwrap_or(false);
            for e in std::fs::read_dir(g.join("maps")).into_iter().flatten().flatten() {
                let f = e.file_name().to_string_lossy().to_string();
                let low = f.to_lowercase();
                if low.ends_with(".bsp") {
                    let entry = found.entry(low[..low.len() - 4].to_string()).or_insert_with(|| (f[..f.len() - 4].to_string(), false));
                    entry.1 |= is_cz;
                } else if low.ends_with(".nav") {
                    navs.insert(low[..low.len() - 4].to_string());
                }
            }
        }
        // a server on another machine: the maps that came with Valve's server download are
        // only there (this computer's copy has just what Loner-Strike adds)
        for name in self.remote_maps() {
            let low = name.to_lowercase();
            found.entry(low).or_insert_with(|| (name.clone(), false));
        }
        let mut by = self.map_sources();
        by.extend(self.store.authors());
        // where each downloaded map came from, and what else that download brought
        let mut from: HashMap<String, (Value, String, Vec<String>)> = HashMap::new();
        for (id, r) in self.store.records() {
            let maps: Vec<String> = r["maps"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(String::from)).collect();
            let mod_id = id.parse::<i64>().map(|n| json!(n)).unwrap_or(json!(id));
            for m in &maps {
                from.insert(m.to_lowercase(), (mod_id.clone(), r["name"].as_str().unwrap_or("").to_string(), maps.clone()));
            }
        }
        let pics = Pictures::index(self);
        let misses = self.store.misses();
        found
            .into_iter()
            .map(|(key, (name, in_cz))| {
                let thumb = if pics.shots.contains_key(&key) || self.store.shot_for(&key).is_some() {
                    Some("shot")
                } else if !crate::gamebanana::MapStore::missed_recently(&misses, &key) {
                    Some("lookup")
                } else if pics.radars.contains_key(&key) {
                    Some("radar")
                } else {
                    None
                };
                let mut m = json!({"name": name, "cat": map_category(&name), "nav": navs.contains(&key), "thumb": thumb,
                                   "by": by.get(&key).cloned().unwrap_or_else(|| "Other".into()), "title": self.map_title(&name)});
                match from.get(&key) {
                    Some((id, mod_name, pack)) => {
                        m["source"] = json!("download");
                        m["mod"] = id.clone();
                        m["modName"] = json!(mod_name);
                        m["pack"] = json!(pack);
                    }
                    None if in_cz || STOCK_CS.contains(&key.as_str()) => m["source"] = json!("stock"),
                    None => m["source"] = json!("added"),
                }
                m
            })
            .collect()
    }

    pub fn map_title(&self, name: &str) -> String {
        if let Some((_, t)) = MAP_TITLES.iter().find(|(k, _)| *k == name.to_lowercase()) {
            return t.to_string();
        }
        for g in self.game_dirs() {
            if let Some(t) = title_from_bsp(&g.join("maps").join(format!("{name}.bsp")), name) {
                return t;
            }
        }
        String::new()
    }

    /// Who made each map, from map_sources.json in the app's folder.
    pub fn map_sources(&self) -> HashMap<String, String> {
        let Ok(text) = std::fs::read_to_string(self.app.join("map_sources.json")) else { return HashMap::new() };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { return HashMap::new() };
        let mut out = HashMap::new();
        for g in v["groups"].as_array().into_iter().flatten() {
            let Some(name) = g["name"].as_str() else { continue };
            for m in g["maps"].as_array().into_iter().flatten().filter_map(|m| m.as_str()) {
                out.insert(m.to_lowercase(), name.to_string());
            }
        }
        out
    }

    /// ("shot" | "radar" | None, path, looking) for a map's picture. With no screenshot on
    /// disk GameBanana is asked in the background (once a week at most per map), and
    /// `looking` says one may still turn up: until then it's the radar or nothing.
    pub fn thumb_source(&self, name: &str) -> (Option<&'static str>, Option<PathBuf>, bool) {
        for g in self.game_dirs() {
            for sub in ["gfx/thumbnails/maps_wide", "gfx/thumbnails/maps"] {
                if let Some(p) = find_in(&g.join(sub), name, &[".tga", ".bmp"]) {
                    return (Some("shot"), Some(p), false);
                }
            }
        }
        // a GameBanana screenshot (saved or looked up) beats the radar overview
        if let Some(p) = self.store.shot_for(name) {
            return (Some("shot"), Some(p), false);
        }
        let looking = self.store.find_shot_later(name);
        for g in self.game_dirs() {
            if let Some(p) = find_in(&g.join("overviews"), name, &[".bmp", ".tga"]) {
                return (Some("radar"), Some(p), looking);
            }
        }
        // a dedicated server's files have no pictures (Valve's server download leaves them
        // out): this computer's game has them for the maps it has too
        if self.dedicated {
            for hl in local_games() {
                for g in [hl.join("czero"), hl.join("cstrike")] {
                    for (sub, kind) in [("gfx/thumbnails/maps_wide", "shot"), ("gfx/thumbnails/maps", "shot"), ("overviews", "radar")] {
                        if let Some(p) = find_in(&g.join(sub), name, &[".tga", ".bmp"]) {
                            return (Some(kind), Some(p), looking);
                        }
                    }
                }
            }
        }
        (None, None, looking)
    }

    /// The map's picture, converted once and then cached: screenshots as a small JPEG
    /// (shown at a fraction of GameBanana's 530 pixels), radars as PNG. Also its kind,
    /// and whether a better picture may still turn up (see thumb_source).
    pub fn thumbnail(&self, name: &str) -> (Option<(PathBuf, &'static str)>, bool) {
        let (kind, src, looking) = self.thumb_source(name);
        let (Some(kind), Some(src)) = (kind, src) else { return (None, looking) };
        let cache = self.app.join("cache");
        let _ = std::fs::create_dir_all(&cache);
        let base = format!("{kind}-{}", name.to_lowercase());
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let fresh = |p: &Path| p.exists() && mtime(p) >= mtime(&src);
        if kind == "shot" {
            let jpg = cache.join(format!("{base}.jpg"));
            // a PNG made by an earlier version is still good
            let old = cache.join(format!("{base}.png"));
            if !fresh(&jpg) && fresh(&old) {
                return (Some((old, kind)), looking);
            }
            if !fresh(&jpg) {
                let _ = convert(&src, &jpg, Shrink::Width(SHOT_WIDTH));
            }
            return (jpg.exists().then_some((jpg, kind)), looking);
        }
        // Windows: radars with their green background made see-through (a new name, so the
        // green ones cached before aren't used)
        let out = cache.join(if cfg!(windows) { format!("{base}-clear.png") } else { format!("{base}.png") });
        if !fresh(&out) {
            let _ = if cfg!(windows) { convert_radar(&src, &out, 512) } else { convert(&src, &out, Shrink::Fit(512)) };
        }
        (out.exists().then_some((out, kind)), looking)
    }
}

impl Game {
    /// The maps in a remote server's own maps folder, asked over SSH at most every 5 minutes
    /// per server (kept from the last answer if the machine doesn't reply).
    fn remote_maps(&self) -> Vec<String> {
        static CACHE: std::sync::Mutex<Option<HashMap<String, (std::time::Instant, Vec<String>)>>> = std::sync::Mutex::new(None);
        let Some((r, dir)) = &self.remote else { return Vec::new() };
        if let Some((at, list)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(dir) {
            if at.elapsed() < std::time::Duration::from_secs(300) {
                return list.clone();
            }
        }
        let cmd = format!("cd \"$HOME/{dir}/maps\" 2>/dev/null && ls -1 | grep -i '\\.bsp$'");
        let fresh: Option<Vec<String>> = crate::remote::exec_quick(r, &cmd).ok().map(|(_, out)| {
            out.lines()
                .filter_map(|l| l.trim().strip_suffix(".bsp").or_else(|| l.trim().strip_suffix(".BSP")).map(String::from))
                .filter(|n| safe_arg(n))
                .collect()
        });
        let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let c = c.get_or_insert_with(HashMap::new);
        match fresh {
            Some(list) => {
                c.insert(dir.clone(), (std::time::Instant::now(), list.clone()));
                list
            }
            None => c.get(dir).map(|(_, l)| l.clone()).unwrap_or_default(),
        }
    }
}

/// This computer's Half-Life folders, looked up at most every 5 minutes (it scans Steam's
/// folders).
fn local_games() -> Vec<PathBuf> {
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, Vec<PathBuf>)>> = std::sync::Mutex::new(None);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, list)) = c.as_ref() {
        if at.elapsed() < std::time::Duration::from_secs(300) {
            return list.clone();
        }
    }
    let list: Vec<PathBuf> = crate::steam::find_installs().into_iter().map(|i| i.hl).collect();
    *c = Some((std::time::Instant::now(), list.clone()));
    list
}

/// Screenshots are shown about 170 pixels wide: twice that stays sharp on a phone
const SHOT_WIDTH: u32 = 340;

/// Screenshots and radar images on disk, read once per scan (not once per map).
struct Pictures {
    shots: HashMap<String, PathBuf>,
    radars: HashMap<String, PathBuf>,
}

impl Pictures {
    fn index(g: &Game) -> Pictures {
        let collect = |dirs: Vec<PathBuf>, exts: &[&str]| {
            let mut out = HashMap::new();
            for d in dirs {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let f = e.file_name().to_string_lossy().to_lowercase();
                    if let Some(ext) = exts.iter().find(|x| f.ends_with(*x)) {
                        out.entry(f[..f.len() - ext.len()].to_string()).or_insert(e.path());
                    }
                }
            }
            out
        };
        let mut dirs = g.game_dirs().to_vec();
        // a dedicated server's files have no pictures: this computer's game has them (the
        // same fallback thumb_source uses, so the card and the picture agree)
        if g.dedicated {
            dirs.extend(local_games().into_iter().flat_map(|hl| [hl.join("czero"), hl.join("cstrike")]));
        }
        Pictures {
            shots: collect(
                dirs.iter().flat_map(|d| [d.join("gfx/thumbnails/maps_wide"), d.join("gfx/thumbnails/maps")]).collect(),
                &[".tga", ".bmp"],
            ),
            radars: collect(dirs.iter().map(|d| d.join("overviews")).collect(), &[".bmp", ".tga"]),
        }
    }
}

/// How convert sizes a picture down (never up).
pub enum Shrink {
    /// at most this wide (screenshots)
    Width(u32),
    /// at most this wide and high (radar overviews)
    Fit(u32),
}

/// TGA/BMP/JPEG -> PNG, or JPEG when `out` ends in .jpg.
pub fn convert(src: &Path, out: &Path, shrink: Shrink) -> Result<(), String> {
    let data = std::fs::read(src).map_err(|e| e.to_string())?;
    let fmt = match src.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("tga") => Some(image::ImageFormat::Tga), // TGA has no magic number to guess from
        Some("bmp") => Some(image::ImageFormat::Bmp),
        _ => None,
    };
    let mut img = match fmt {
        Some(f) => image::load_from_memory_with_format(&data, f),
        None => image::load_from_memory(&data),
    }
    .map_err(|e| e.to_string())?;
    match shrink {
        Shrink::Width(w) if img.width() > w => img = img.resize(w, img.height(), image::imageops::FilterType::Triangle),
        Shrink::Fit(n) if img.width() > n || img.height() > n => img = img.resize(n, n, image::imageops::FilterType::Triangle),
        _ => {}
    }
    let jpeg = out.extension().is_some_and(|x| x == "jpg");
    let tmp = out.with_extension("tmp");
    if jpeg {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 80);
        img.to_rgb8().write_with_encoder(enc).map_err(|e| e.to_string())?;
    } else {
        img.save_with_format(&tmp, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())
}

/// A radar overview (TGA/BMP) -> PNG at most `fit` wide and high, its background made
/// transparent: the game draws overviews over pure green (0, 255, 0) and treats that as
/// see-through. Keyed before shrinking, so no green fringe is blended into the edges.
pub fn convert_radar(src: &Path, out: &Path, fit: u32) -> Result<(), String> {
    let data = std::fs::read(src).map_err(|e| e.to_string())?;
    let img = match src.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("tga") => image::load_from_memory_with_format(&data, image::ImageFormat::Tga),
        Some("bmp") => image::load_from_memory_with_format(&data, image::ImageFormat::Bmp),
        _ => image::load_from_memory(&data),
    }
    .map_err(|e| e.to_string())?;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let [r, g, b, _] = p.0;
        if g >= 240 && r <= 16 && b <= 16 {
            p.0 = [0, 0, 0, 0];
        }
    }
    let mut img = image::DynamicImage::ImageRgba8(rgba);
    if img.width() > fit || img.height() > fit {
        img = img.resize(fit, fit, image::imageops::FilterType::Triangle);
    }
    let tmp = out.with_extension("tmp");
    img.save_with_format(&tmp, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories() {
        assert_eq!(map_category("de_dust2"), "defuse");
        assert_eq!(map_category("CS_office"), "hostage");
        assert_eq!(map_category("es_trinity"), "objective");
        assert_eq!(map_category("surf_ski"), "surf");
        assert_eq!(map_category("awp_india"), "arena");
        assert_eq!(map_category("random"), "other");
    }

    #[test]
    fn small_jpeg_shots() {
        let dir = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "shot-").unwrap();
        let src = dir.join("de_x.bmp");
        image::RgbImage::from_pixel(530, 300, image::Rgb([90, 120, 30])).save(&src).unwrap();
        let out = dir.join("shot-de_x.jpg");
        convert(&src, &out, Shrink::Width(SHOT_WIDTH)).unwrap();
        let data = std::fs::read(&out).unwrap();
        assert_eq!(image::guess_format(&data).unwrap(), image::ImageFormat::Jpeg);
        let img = image::load_from_memory(&data).unwrap();
        assert_eq!((img.width(), img.height()), (340, 192));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bsp_title() {
        let dir = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "bsp-").unwrap();
        let ents = b"{\n\"classname\" \"worldspawn\"\n\"message\" \"Snow Fort\\n by someone\"\n}\n{\"classname\" \"x\"}";
        let mut bsp = Vec::new();
        bsp.extend_from_slice(&30i32.to_le_bytes());
        bsp.extend_from_slice(&12i32.to_le_bytes());
        bsp.extend_from_slice(&(ents.len() as i32).to_le_bytes());
        bsp.extend_from_slice(ents);
        let p = dir.join("de_snow.bsp");
        std::fs::write(&p, &bsp).unwrap();
        assert_eq!(title_from_bsp(&p, "de_snow").as_deref(), Some("Snow Fort"));
        assert_eq!(title_from_bsp(&dir.join("missing.bsp"), "x"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
