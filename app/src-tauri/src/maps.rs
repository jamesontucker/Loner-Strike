//! The map list (with favourites, hidden and recent), map titles and authors, and
//! map pictures. Mirrors the map half of panel/panel.py's Game.

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

/// Same rule as panel.py's SAFE_ARG.
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
        let fav: HashSet<String> = self.read_list("fav").iter().map(|m| m.to_lowercase()).collect();
        let hidden: HashSet<String> = self.read_list("hidden").iter().map(|m| m.to_lowercase()).collect();
        let recent: Vec<String> = self.read_list("recent").iter().map(|m| m.to_lowercase()).collect();
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

    pub fn read_list(&self, which: &str) -> Vec<String> {
        let Some(file) = list_file(which) else { return vec![] };
        sys::read_latin1(&self.data.join(file))
            .map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default()
    }

    pub fn set_flag(&self, which: &str, name: &str, on: bool) -> Result<(), String> {
        if !(which == "fav" || which == "hidden") || !safe_arg(name) {
            return Err("invalid map".into());
        }
        let mut items: Vec<String> = self.read_list(which).into_iter().filter(|m| m.to_lowercase() != name.to_lowercase()).collect();
        if on {
            items.insert(0, name.to_string());
        }
        let text: String = items.iter().map(|m| format!("{m}\n")).collect();
        sys::write_latin1(&self.data.join(list_file(which).unwrap()), &text).map_err(|e| e.to_string())
    }

    fn scan_maps(&self) -> Vec<Value> {
        let mut found: BTreeMap<String, String> = BTreeMap::new();
        let mut navs: HashSet<String> = HashSet::new();
        for g in self.game_dirs() {
            for e in std::fs::read_dir(g.join("maps")).into_iter().flatten().flatten() {
                let f = e.file_name().to_string_lossy().to_string();
                let low = f.to_lowercase();
                if low.ends_with(".bsp") {
                    found.entry(low[..low.len() - 4].to_string()).or_insert_with(|| f[..f.len() - 4].to_string());
                } else if low.ends_with(".nav") {
                    navs.insert(low[..low.len() - 4].to_string());
                }
            }
        }
        let mut by = self.map_sources();
        by.extend(self.store.authors());
        found
            .into_iter()
            .map(|(key, name)| {
                let (kind, _) = self.thumb_source(&name);
                json!({"name": name, "cat": map_category(&name), "nav": navs.contains(&key), "thumb": kind,
                       "by": by.get(&key).cloned().unwrap_or_else(|| "Other".into()), "title": self.map_title(&name)})
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

    /// ("shot" | "radar" | None, path) for a map's picture.
    pub fn thumb_source(&self, name: &str) -> (Option<&'static str>, Option<PathBuf>) {
        for g in self.game_dirs() {
            for sub in ["gfx/thumbnails/maps_wide", "gfx/thumbnails/maps"] {
                if let Some(p) = find_in(&g.join(sub), name, &[".tga", ".bmp"]) {
                    return (Some("shot"), Some(p));
                }
            }
        }
        // a GameBanana screenshot (saved or looked up) beats the radar overview
        if let Some(p) = self.store.shot_for(name).or_else(|| self.store.find_shot(name)) {
            return (Some("shot"), Some(p));
        }
        for g in self.game_dirs() {
            if let Some(p) = find_in(&g.join("overviews"), name, &[".bmp", ".tga"]) {
                return (Some("radar"), Some(p));
            }
        }
        (None, None)
    }

    /// The map's picture as a PNG (converted once, then cached).
    pub fn thumbnail(&self, name: &str) -> Option<PathBuf> {
        let (kind, src) = self.thumb_source(name);
        let (kind, src) = (kind?, src?);
        let cache = self.app.join("cache");
        let _ = std::fs::create_dir_all(&cache);
        let out = cache.join(format!("{kind}-{}.png", name.to_lowercase()));
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if !out.exists() || mtime(&out) < mtime(&src) {
            let _ = convert(&src, &out, kind == "radar");
        }
        out.exists().then_some(out)
    }
}

/// TGA/BMP/JPEG -> PNG; radar overviews are shrunk to at most 512 pixels.
pub fn convert(src: &Path, out: &Path, shrink: bool) -> Result<(), String> {
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
    if shrink && (img.width() > 512 || img.height() > 512) {
        img = img.resize(512, 512, image::imageops::FilterType::Triangle);
    }
    let tmp = out.with_extension("png.tmp");
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
