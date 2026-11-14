//! The map editor's Budget panel (agent P5): how close the map is to GoldSrc's limits.
//! - **Compile limits**: the last compile's own final numbers (SDHLT's chart in the work
//!   folder's `<map>.log`, read with compile.rs `chart_values`: models, planes, clipnodes,
//!   faces, leaves, marksurfaces, lighting…), and live counts that need no compile (brush
//!   entities, entities, an edicts estimate, unique textures).
//! - **Precache** (512 models, 512 sounds): what the map's entities name (models, sprites,
//!   sounds, `noise*`, and each brush entity's inline model); the page adds the game's
//!   baseline and the chosen mode's creatures (Y's numbers, from the creature types).
//! - For each bar, the biggest contributors with the usual fix (the page picks them in the map).
//!   GET /api/tools/editor/budget

use super::{blocking, Ed, Session, NO_DOC};
use crate::editor::doc::{EntityId, MapDocument};
use crate::server::Shared;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// GoldSrc's limits for what can be counted without compiling.
pub const MAX_MODELS: u64 = 512; // inline (brush) models + the world, MAX_MAP_MODELS
pub const MAX_ENTITIES: u64 = 1024; // MAX_MAP_ENTITIES (the entity lump)
pub const MAX_EDICTS: u64 = 900; // HLDS' default edict count
pub const MAX_TEXTURES: u64 = 512; // MAX_MAP_TEXTURES
/// Players' edicts (and the game's own) on top of the map's.
const PLAYER_EDICTS: u64 = 32 + 1;

/// Point entities the compiler takes out (they never become edicts).
fn compiled_away(class: &str, has_name: bool) -> bool {
    matches!(class, "func_detail" | "func_group" | "info_texlights" | "info_compile_parameters" | "light_environment" | "info_null")
        || ((class == "light" || class == "light_spot") && !has_name)
}

/// A bar: name, what it counts, used, the limit, and why it matters.
fn bar(id: &str, name: &str, used: u64, max: u64, what: &str) -> Value {
    let pct = if max > 0 { used as f64 / max as f64 * 100.0 } else { 0.0 };
    json!({"id": id, "name": name, "used": used, "max": max, "pct": (pct * 10.0).round() / 10.0, "what": what})
}

/// How much a brush costs the BSP (planes, faces, clipnodes): its faces, sloped ones counted
/// three times (each adds planes and splits the hull clipping more).
fn brush_cost(b: &crate::editor::doc::Brush) -> u64 {
    b.faces
        .iter()
        .map(|f| match f.plane() {
            Some(p) if [p.normal.x, p.normal.y, p.normal.z].iter().filter(|c| c.abs() > 0.999).count() == 1 => 1,
            Some(_) => 3,
            None => 0,
        })
        .sum()
}

/// What makes up a geometry limit: world brushes one by one, brush entities as one each.
fn geometry_contributors(d: &MapDocument, fix: &str) -> Value {
    let mut by_ent: BTreeMap<u64, (u64, Vec<u64>)> = BTreeMap::new();
    let mut world: Vec<(u64, u64)> = Vec::new();
    let mut total = 0u64;
    for (id, b) in &d.brushes {
        let c = brush_cost(b);
        total += c;
        match b.entity {
            Some(e) => {
                let x = by_ent.entry(e.0).or_default();
                x.0 += c;
                x.1.push(id.0);
            }
            None => world.push((id.0, c)),
        }
    }
    let mut rows: Vec<(u64, Value)> = Vec::new();
    for (e, (c, bs)) in by_ent {
        let class = d.entities.get(&EntityId(e)).map(|x| x.classname().to_string()).unwrap_or_default();
        rows.push((c, json!({"label": format!("{class} #{e} ({} brush{})", bs.len(), if bs.len() == 1 { "" } else { "es" }), "entities": [e], "brushes": bs, "cost": c})));
    }
    for (b, c) in world {
        rows.push((c, json!({"label": format!("world brush #{b}"), "brushes": [b], "entities": [], "cost": c})));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let top: Vec<Value> = rows.into_iter().take(15).map(|(c, mut v)| {
        v["share"] = json!(if total > 0 { (c as f64 / total as f64 * 1000.0).round() / 10.0 } else { 0.0 });
        v
    }).collect();
    json!({"fix": fix, "rows": top, "basis": "an estimate from each brush's faces (sloped faces cost more); the compile's own numbers are above"})
}

fn ents_by_class_ids(d: &MapDocument, ids: &std::collections::HashSet<u64>) -> Vec<Value> {
    rows_by_class(d, |id, _| ids.contains(&id))
}

fn ents_by_class(d: &MapDocument, keep: impl Fn(&crate::editor::doc::Entity) -> bool) -> Vec<Value> {
    rows_by_class(d, |_, e| keep(e))
}

fn rows_by_class(d: &MapDocument, keep: impl Fn(u64, &crate::editor::doc::Entity) -> bool) -> Vec<Value> {
    let mut by: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for (id, e) in &d.entities {
        if keep(id.0, e) {
            by.entry(e.classname().to_string()).or_default().push(id.0);
        }
    }
    let total: usize = by.values().map(|v| v.len()).sum();
    let mut rows: Vec<Value> = by
        .into_iter()
        .map(|(c, ids)| json!({"label": format!("{c} ×{}", ids.len()), "entities": ids, "brushes": [], "share": if total > 0 { ((ids.len() as f64 / total as f64) * 1000.0).round() / 10.0 } else { 0.0 }, "cost": ids.len()}))
        .collect();
    rows.sort_by(|a, b| b["cost"].as_u64().cmp(&a["cost"].as_u64()));
    rows.truncate(15);
    rows
}

/// The live bars and their contributors.
pub fn live(d: &MapDocument) -> Vec<Value> {
    let brush_ents: Vec<u64> = d.entities.iter().filter(|(id, _)| d.brushes.values().any(|b| b.entity == Some(**id))).map(|(id, _)| id.0).collect();
    let detail = |id: &u64| d.entities.get(&EntityId(*id)).is_some_and(|e| matches!(e.classname(), "func_detail" | "func_group"));
    let models = brush_ents.iter().filter(|id| !detail(id)).count() as u64 + 1;
    let edicts = d.entities.values().filter(|e| !compiled_away(e.classname(), e.get("targetname").is_some_and(|t| !t.is_empty()))).count() as u64 + PLAYER_EDICTS;
    let mut tex: HashMap<String, u64> = HashMap::new();
    for b in d.brushes.values() {
        for f in &b.faces {
            *tex.entry(f.texture.to_ascii_lowercase()).or_default() += 1;
        }
    }
    let mut trows: Vec<(String, u64)> = tex.iter().map(|(k, v)| (k.clone(), *v)).collect();
    trows.sort_by(|a, b| b.1.cmp(&a.1));
    let faces: u64 = trows.iter().map(|x| x.1).sum();
    let mut out = vec![
        {
            let mut v = bar("live_models", "Brush entities (models)", models, MAX_MODELS, "every brush entity is a model, plus the world (func_detail and func_group aren't)");
            let owners: std::collections::HashSet<u64> = brush_ents.iter().copied().filter(|id| !detail(id)).collect();
            v["contributors"] = json!({"fix": "Merge func_walls that never move or change into one (or make them func_detail); each door, trigger and breakable is one model.", "rows": ents_by_class_ids(d, &owners)});
            v
        },
        {
            let mut v = bar("live_entities", "Entities", d.entities.len() as u64 + 1, MAX_ENTITIES, "every entity in the map, the world included");
            v["contributors"] = json!({"fix": "Lights without a name are compiled away; use fewer info_ and env_ helpers, or one multi_manager for many targets.", "rows": ents_by_class(d, |_| true)});
            v
        },
        {
            let mut v = bar("live_edicts", "Edicts at the start (estimate)", edicts, MAX_EDICTS, "entities that live in the game (lights without a name and func_detail are compiled away) plus 32 players");
            v["contributors"] = json!({"fix": "Fewer live entities: unnamed lights, func_detail and func_group don't count; env_sprite, cycler_sprite and ambient_generic each do.", "rows": ents_by_class(d, |e| !compiled_away(e.classname(), e.get("targetname").is_some_and(|t| !t.is_empty())))});
            v
        },
        {
            let mut v = bar("live_textures", "Different textures", tex.len() as u64, MAX_TEXTURES, "each texture a face uses, counted once");
            v["contributors"] = json!({"fix": "Replace rarely used textures with ones already in the map (Edit > Find and replace).", "rows": trows.iter().take(15).map(|(t, n)| json!({"label": format!("{t} on {n} face{}", if *n == 1 { "" } else { "s" }), "texture": t, "share": if faces > 0 { (*n as f64 / faces as f64 * 1000.0).round() / 10.0 } else { 0.0 }, "entities": [], "brushes": [], "cost": n})).collect::<Vec<_>>()});
            v
        },
    ];
    out.retain(|v| v["max"].as_u64().unwrap_or(0) > 0);
    out
}

/// What the map's entities precache: (models incl. sprites and inline models, sounds), each
/// with its users.
pub fn precache(d: &MapDocument) -> Value {
    let named = super::assets::named_files(d);
    let inline = d.entities.iter().filter(|(id, e)| !matches!(e.classname(), "func_detail" | "func_group") && d.brushes.values().any(|b| b.entity == Some(**id))).count();
    let rows = |kinds: &[&str]| -> Vec<Value> {
        let mut v: Vec<Value> = named.iter().filter(|n| kinds.contains(&n.kind)).map(|n| json!({"label": n.rel, "entities": n.users.iter().filter(|u| **u != 0).collect::<Vec<_>>(), "brushes": [], "cost": 1, "kind": n.kind})).collect();
        v.sort_by(|a, b| b["entities"].as_array().map(|x| x.len()).cmp(&a["entities"].as_array().map(|x| x.len())));
        v
    };
    let models = rows(&["model", "sprite"]);
    let sounds = rows(&["sound"]);
    json!({"max": 512, "models": models.len() + inline, "inlineModels": inline, "sounds": sounds.len(), "modelRows": models.into_iter().take(40).collect::<Vec<_>>(), "soundRows": sounds.into_iter().take(40).collect::<Vec<_>>()})
}

/// The compile work folders a map's log can be in (the bottle's, then the app's).
pub(super) fn work_dirs(st: &Shared, map: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(g) = st.local_game() {
        let hl = g.hl.to_string_lossy().to_string();
        if let Some(i) = hl.find("/drive_c/") {
            v.push(PathBuf::from(&hl[..i + 8]).join("Loner-Strike").join("Compiler").join("work").join(map));
        }
    }
    v.push(crate::localdata::dir("tools/compile").join("work").join(map));
    v
}

/// The last compile's final numbers, from its log: [{name, used, max, pct}] and when.
pub fn last_compile(log: &Path) -> Option<Value> {
    let md = std::fs::metadata(log).ok()?;
    if md.len() > 16 << 20 {
        return None;
    }
    let text = crate::rcon::latin1(&std::fs::read(log).ok()?);
    let charts = crate::tools::compile::chart_values(&text);
    if charts.is_empty() {
        return None;
    }
    static NUM: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let num = NUM.get_or_init(|| regex::Regex::new(r"^\*?\s*[a-zA-Z]+\s+(\d+)/(\d+)").unwrap());
    let rows: Vec<Value> = charts
        .into_iter()
        .map(|(name, pct, line)| {
            let (used, max) = num.captures(&line).map(|c| (c[1].parse::<u64>().unwrap_or(0), c[2].parse::<u64>().unwrap_or(0))).unwrap_or((0, 0));
            json!({"id": format!("bsp_{}", name.to_lowercase()), "name": name, "used": used, "max": max, "pct": (pct * 10.0).round() / 10.0, "what": "from the last compile"})
        })
        .collect();
    let when = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    Some(json!({"rows": rows, "when": when, "log": log.to_string_lossy()}))
}

/// The fix and contributors for a compile limit (by its name in SDHLT's chart).
fn compile_contributors(d: &MapDocument, name: &str) -> Value {
    let n = name.to_lowercase();
    let fix = match n.as_str() {
        "clipnodes" => "Cover detailed props with a CLIP brush and make them func_illusionary (or func_detail with zhlt_noclip 1); simplify sloped and round brushes players touch.",
        "planes" | "faces" | "marksurfaces" | "surfedges" | "edges" | "vertexes" | "nodes" | "leaves" | "leafs" | "worldleaves" => "Tie small detail to func_detail so it doesn't split the world's leaves; fewer sides on round shapes; NULL on faces nobody sees.",
        "models" => "Merge func_walls that never move or change into one, or make them func_detail.",
        "texinfos" | "texinfo" => "Fewer different texture alignments: Snap to grid and the same scale on matching faces.",
        "lightdata" | "lighting" | "allocblock" | "lightmaps" => "Bigger texture scale on large faces (fewer lightmap samples); NULL or SKY on faces nobody sees.",
        "texdata" | "textures" => "Fewer different textures, or smaller ones; don't wadinclude big WADs.",
        "entdata" | "entities" => "Fewer entities or shorter keys.",
        _ => "",
    };
    match n.as_str() {
        "models" => {
            let owners: std::collections::HashSet<u64> = d.brushes.values().filter_map(|b| b.entity.map(|e| e.0)).filter(|e| d.entities.get(&EntityId(*e)).is_some_and(|x| !matches!(x.classname(), "func_detail" | "func_group"))).collect();
            json!({"fix": fix, "rows": ents_by_class_ids(d, &owners)})
        }
        "entdata" | "entities" => json!({"fix": fix, "rows": ents_by_class(d, |_| true)}),
        _ => geometry_contributors(d, fix),
    }
}

pub(super) fn report(st: &Shared, s: &Session) -> Value {
    let d = &s.doc;
    let map = crate::tools::compile::map_name(&s.name);
    let compile = work_dirs(st, &map).into_iter().map(|w| w.join(format!("{map}.log"))).filter(|p| p.is_file()).max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok()).and_then(|p| last_compile(&p));
    let compile = compile.map(|mut c| {
        if let Some(rows) = c["rows"].as_array_mut() {
            for r in rows {
                let name = r["name"].as_str().unwrap_or("").to_string();
                r["contributors"] = compile_contributors(d, &name);
            }
        }
        c
    });
    // (where the app's own compiles keep their log, for "where does this come from")
    let log_at = crate::localdata::dir("tools/compile").join("work").join(&map).join(format!("{map}.log"));
    json!({"map": map, "compile": compile, "live": live(d), "precache": precache(d), "revision": d.revision, "logAt": log_at.to_string_lossy()})
}

async fn api_budget(axum::extract::State(st): axum::extract::State<Shared>, Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        Ok(report(&st, s))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/editor/budget", get(api_budget))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::Entity;
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;

    #[test]
    fn live_counts_and_precache() {
        let mut d = MapDocument::new();
        let mut h = crate::editor::history::History::default();
        let wall = crate::editor::history::cmd_add_brush(&mut d, &mut h, make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "CRATE1"), "Box");
        crate::editor::history::cmd_add_brush(&mut d, &mut h, make_box(v3(100.0, 0.0, 0.0), v3(164.0, 64.0, 64.0), "WALL"), "Box");
        let fw = crate::editor::history::cmd_add_entity(&mut d, &mut h, Entity::new("func_wall"));
        let mut b = d.brushes[&wall].clone();
        b.entity = Some(fw);
        d.brushes.insert(wall, b);
        for (c, k, v) in [("light", "", ""), ("light", "targetname", "l1"), ("env_sprite", "model", "sprites/glow01.spr"), ("ambient_generic", "message", "ambience/wind1.wav"), ("cycler_sprite", "model", "models/crate.mdl"), ("func_door", "noise1", "doors/doormove1.wav")] {
            let mut e = Entity::new(c);
            if !k.is_empty() {
                e.keys.push((k.into(), v.into()));
            }
            crate::editor::history::cmd_add_entity(&mut d, &mut h, e);
        }
        let bars = live(&d);
        let get = |id: &str| bars.iter().find(|b| b["id"] == id).unwrap().clone();
        assert_eq!(get("live_models")["used"], 2, "the func_wall + the world");
        assert_eq!(get("live_entities")["used"], 8);
        // the unnamed light is compiled away; 32 players + 1
        assert_eq!(get("live_edicts")["used"], 6 + 33);
        assert_eq!(get("live_textures")["used"], 2);
        let p = precache(&d);
        assert_eq!((p["models"].as_u64(), p["inlineModels"].as_u64(), p["sounds"].as_u64()), (Some(3), Some(1), Some(2)));
        let c = geometry_contributors(&d, "x");
        assert_eq!(c["rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn reads_the_last_compile() {
        let dir = std::env::temp_dir().join(format!("ls-budget-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("m.log");
        std::fs::write(&log, "Object names  Objects/Maxobjs  Memory / Maxmem  Fullness\n------------  ---------------  ---------------  --------\nmodels            30/512          1920/32768     ( 5.9%)\nclipnodes      29000/32767      232000/262136    (88.5%)\nplanes         20000/32768      400000/655360    (61.0%)\n").unwrap();
        let c = last_compile(&log).unwrap();
        let rows = c["rows"].as_array().unwrap();
        let clip = rows.iter().find(|r| r["name"] == "clipnodes").unwrap();
        assert_eq!((clip["used"].as_u64(), clip["max"].as_u64()), (Some(29000), Some(32767)));
        assert!(clip["pct"].as_f64().unwrap() > 88.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
