//! Finding a map's files and summing a map up: the `/info` answer and the facts the
//! Tools map list shows.

use crate::game::Game;
use crate::goldsrc::{bsp, entities, wad};
use crate::mapcopy::Finder;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A map in the player's game, found by name.
#[derive(Debug, Clone)]
pub struct MapFile {
    /// as its file is named
    pub name: String,
    pub bsp: PathBuf,
    /// "cstrike" or "czero": the folder it's in
    pub game: String,
    pub hl: PathBuf,
    pub czero: bool,
}

pub const NO_MAP: &str = "That map isn't on this computer.";

/// The map's .bsp in the player's game (their game's own folder first, like the game).
pub fn locate(game: &Game, name: &str) -> Option<MapFile> {
    let czero = game.is_czero();
    let f = Finder::default();
    let m = crate::mapcopy::find_local(&f, &game.hl, name, if czero { "czero" } else { "cstrike" })?;
    Some(MapFile { name: m.name, bsp: m.bsp, game: m.game, hl: game.hl.clone(), czero })
}

impl MapFile {
    pub fn wad_dirs(&self) -> Vec<PathBuf> {
        wad::search_dirs(&self.hl, self.czero)
    }

    /// The WADs the map names, found and opened.
    pub fn wads(&self, finder: &Finder, names: &[String]) -> wad::WadSet {
        wad::WadSet::open(finder, &self.wad_dirs(), names)
    }

    /// A file next to the map (its nav, its knowledge), whatever its case.
    pub fn sibling(&self, ext: &str) -> Option<PathBuf> {
        Finder::default().find(self.bsp.parent()?, &format!("{}.{ext}", self.name))
    }
}

/// A stamp of the texture files a map names (path, size, time; "missing" for those not
/// found), so cached textures and info are rebuilt when one is added or changed.
pub fn wads_stamp(set: &wad::WadSet) -> String {
    let parts: Vec<String> = set
        .wads
        .iter()
        .map(|(n, w)| match w {
            Ok(w) => {
                let s = super::cache::Stamp::of(&w.path).map(|s| s.key()).unwrap_or_default();
                format!("{n}={}:{s}", w.path.display())
            }
            Err(_) => format!("{n}=missing"),
        })
        .collect();
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    super::cache::short_hash(&refs)
}

/// The map's textures, entities and models (no geometry): what info and the texture route
/// need, read without loading the whole file.
pub struct Lite {
    pub textures: Vec<bsp::MipTex>,
    pub entities: Vec<entities::Entity>,
    pub models: Vec<bsp::Model>,
}

pub fn read_lite(path: &Path, pixels: bool) -> crate::goldsrc::Result<Lite> {
    let l = bsp::read_lumps(path, &[bsp::LUMP_TEXTURES, bsp::LUMP_ENTITIES, bsp::LUMP_MODELS])?;
    Ok(Lite {
        textures: bsp::textures(&l[0], pixels)?,
        entities: entities::parse(&crate::rcon::latin1(&l[1])),
        models: bsp::models(&l[2])?,
    })
}

/// Where each texture comes from (embedded, a WAD's name, or missing).
pub fn texture_sources(lite: &Lite, wads: &wad::WadSet) -> Vec<(String, u32, u32, wad::Source)> {
    let mut seen = std::collections::HashSet::new();
    lite.textures
        .iter()
        .filter(|t| !t.name.is_empty() && seen.insert(t.name.to_lowercase()))
        .map(|t| {
            let from = if t.embedded.is_some() { wad::Source::Embedded } else { wads.source_of(&t.name) };
            (t.name.clone(), t.width, t.height, from)
        })
        .collect()
}

fn yaw_of(e: &entities::Entity) -> f32 {
    e.angles().map(|a| a[1]).unwrap_or(0.0)
}

/// The `/info` answer.
pub fn info(m: &MapFile, lite: &Lite, wads: &wad::WadSet) -> Value {
    let world = lite.entities.first().filter(|e| e.classname() == "worldspawn");
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    for e in &lite.entities {
        *classes.entry(e.classname().to_string()).or_default() += 1;
    }
    let spawns = |class: &str| -> Vec<Value> {
        lite.entities
            .iter()
            .filter(|e| e.classname() == class)
            .filter_map(|e| Some(json!({"origin": e.origin()?, "yaw": yaw_of(e)})))
            .collect()
    };
    let textures: Vec<Value> = texture_sources(lite, wads)
        .into_iter()
        .map(|(name, w, h, from)| json!({"name": name, "width": w, "height": h, "from": from.label()}))
        .collect();
    let missing_textures: Vec<&Value> = textures.iter().filter(|t| t["from"] == "missing").map(|t| &t["name"]).collect();
    let wad_list: Vec<Value> = wads
        .wads
        .iter()
        .map(|(n, w)| match w {
            Ok(w) => json!({"name": n, "found": true, "folder": w.path.parent().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_string())}),
            Err(e) if e == "missing" => json!({"name": n, "found": false}),
            Err(e) => json!({"name": n, "found": false, "error": e}),
        })
        .collect();
    let (mins, maxs) = lite.models.first().map(|w| (w.mins, w.maxs)).unwrap_or_default();
    let stamp = super::cache::Stamp::of(&m.bsp).ok();
    json!({
        "name": m.name,
        "game": m.game,
        "size": stamp.map(|s| s.size),
        "bounds": {"mins": mins, "maxs": maxs},
        "models": lite.models.len(),
        "brushEntities": lite.entities.iter().filter(|e| e.brush_model().is_some_and(|n| n > 0)).count(),
        "entities": lite.entities.len(),
        "classes": classes,
        "textures": textures,
        "missingTextures": missing_textures,
        "wads": wad_list,
        "missingWads": wads.missing(),
        "spawns": {"t": spawns("info_player_deathmatch"), "ct": spawns("info_player_start")},
        "message": world.and_then(|w| w.get("message")).unwrap_or(""),
        "skyname": world.and_then(|w| w.get("skyname")).unwrap_or(""),
        "title": crate::maps::title_from_bsp(&m.bsp, &m.name).unwrap_or_default(),
    })
}

/// What the Tools map list says about one map: size, nav, knowledge, missing WADs.
/// Reads only the file heads and the entity text.
pub fn facts(m: &MapFile, finder: &Finder) -> Value {
    let size = std::fs::metadata(&m.bsp).map(|x| x.len()).ok();
    let head = |p: &Path, n: u64| -> Option<Vec<u8>> {
        let mut b = Vec::new();
        std::fs::File::open(p).ok()?.take(n).read_to_end(&mut b).ok()?;
        Some(b)
    };
    let nav = m.sibling("nav").and_then(|p| head(&p, 14)).and_then(|b| {
        let h = crate::navs::parse_head(&b)?;
        Some(json!({"version": h.version, "places": crate::navpack::place_count(&b), "bspSize": h.bsp_size,
                    "matchesMap": h.bsp_size.zip(size).map(|(a, b)| a as u64 == b)}))
    });
    let lsv = m.sibling("lsv").and_then(|p| head(&p, 20)).map(|b| {
        let stage = b.get(16..20).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
        json!({"stage": stage, "done": crate::navpack::lsv_done(&b)})
    });
    let ents = crate::navs::entity_text(&m.bsp).map(|t| entities::parse(&t)).unwrap_or_default();
    let names = entities::wad_list(&ents);
    let mut v = json!({"name": m.name, "game": m.game, "bspSize": size, "nav": nav, "lsv": lsv});
    v["missingWads"] = json!(missing_wads(m, finder, &names));
    v["wads"] = json!(names);
    v
}

/// The named WADs not found in the game folders.
fn missing_wads(m: &MapFile, finder: &Finder, names: &[String]) -> Vec<String> {
    let dirs = m.wad_dirs();
    names.iter().filter(|n| !dirs.iter().any(|d| finder.find(d, n).is_some())).cloned().collect()
}

/// The maps the Tools list offers: the game's map list, plus the maps it downloaded from
/// servers (`cstrike_downloads/maps`, `czero_downloads/maps`), which the game plays but the
/// play lists leave out. Each downloaded one says `"source": "server"`.
pub fn tool_maps(game: &Game) -> Vec<Value> {
    let mut list = game.maps(false);
    let mut have: std::collections::HashSet<String> = list.iter().filter_map(|m| m["name"].as_str().map(str::to_lowercase)).collect();
    let czero = game.is_czero();
    let folders: [&str; 2] = if czero { ["czero_downloads", "cstrike_downloads"] } else { ["cstrike_downloads", "czero_downloads"] };
    for folder in folders {
        let mut names: Vec<String> = std::fs::read_dir(game.hl.join(folder).join("maps"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                let f = e.file_name().to_string_lossy().to_string();
                f.to_lowercase().ends_with(".bsp").then(|| f[..f.len() - 4].to_string())
            })
            .filter(|n| crate::maps::safe_arg(n))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        for name in names {
            if !have.insert(name.to_lowercase()) {
                continue;
            }
            let bsp = game.hl.join(folder).join("maps").join(format!("{name}.bsp"));
            let mut m = json!({"name": name, "cat": crate::maps::map_category(&name), "nav": false, "thumb": null, "by": "Other",
                               "title": crate::maps::title_from_bsp(&bsp, &name).unwrap_or_default(), "source": "server",
                               "fav": false, "hidden": false, "recent": -1});
            // a Condition Zero download in a 1.6 game: 1.6 can't load it
            if !czero && folder.starts_with("czero") {
                m["other"] = json!(true);
                m["otherGame"] = json!("Condition Zero");
            }
            list.push(m);
        }
    }
    list
}

/// `facts` for every map, remembered per map file version (the list asks often).
pub fn all_facts(game: &Game) -> Vec<Value> {
    use std::sync::{Mutex, OnceLock};
    static MEMO: OnceLock<Mutex<std::collections::HashMap<PathBuf, (super::cache::Stamp, Option<super::cache::Stamp>, Option<super::cache::Stamp>, Value)>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let finder = Finder::default();
    let czero = game.is_czero();
    let mut out = Vec::new();
    for m in tool_maps(game) {
        let Some(name) = m["name"].as_str().filter(|n| crate::maps::safe_arg(n)) else { continue };
        let Some(lm) = crate::mapcopy::find_local(&finder, &game.hl, name, if czero { "czero" } else { "cstrike" }) else { continue };
        let mf = MapFile { name: lm.name, bsp: lm.bsp, game: lm.game, hl: game.hl.clone(), czero };
        let Ok(st) = super::cache::Stamp::of(&mf.bsp) else { continue };
        let side = |ext: &str| mf.sibling(ext).and_then(|p| super::cache::Stamp::of(&p).ok());
        let (nav, lsv) = (side("nav"), side("lsv"));
        let hit = memo.lock().unwrap_or_else(|e| e.into_inner()).get(&mf.bsp).filter(|c| c.0 == st && c.1 == nav && c.2 == lsv).map(|c| c.3.clone());
        let mut v = match hit {
            // the map is the same; a WAD may have been added since
            Some(mut v) => {
                let names: Vec<String> = serde_json::from_value(v["wads"].clone()).unwrap_or_default();
                v["missingWads"] = json!(missing_wads(&mf, &finder, &names));
                v
            }
            None => {
                let v = facts(&mf, &finder);
                memo.lock().unwrap_or_else(|e| e.into_inner()).insert(mf.bsp.clone(), (st, nav, lsv, v.clone()));
                v
            }
        };
        if let (Some(o), Some(src)) = (v.as_object_mut(), m.as_object()) {
            for k in ["title", "cat", "source", "other"] {
                if let Some(x) = src.get(k) {
                    o.insert(k.to_string(), x.clone());
                }
            }
        }
        out.push(v);
    }
    out
}

/// Entities as the page wants them.
pub fn entities_json(lite: &Lite) -> Value {
    entities::to_json(&lite.entities)
}
