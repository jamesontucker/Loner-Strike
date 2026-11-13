//! Live map checks: what to flag before compiling (GoldSrc limits, broken brushes, missing
//! textures, missing spawns, dangling targets). Cheap enough to run after every edit.
//! Leak detection (open hulls) needs the compiler's view of the world and comes later.

use super::doc::{BrushId, EntityId, MapDocument};
use super::geom::{brush_bounds, validate};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

/// Limits worth warning about. Sources: the HL SDK's bspfile.h values the classic ZHLT
/// lineage still enforces, the engine's world size and default edict count, and WAD3's
/// 16-byte texture names.
pub mod limits {
    /// Game world extent: things beyond +-4096 aren't drawn or collide badly.
    pub const WORLD_EXTENT: f64 = 4096.0;
    /// Entities in the BSP (bspfile.h MAX_MAP_ENTITIES).
    pub const MAX_ENTITIES: usize = 1024;
    /// The server's default edict count (players, weapons and temporary entities share it).
    pub const DEFAULT_EDICTS: usize = 900;
    /// Brush models including the world (bspfile.h MAX_MAP_MODELS 400; newer tools allow 512).
    pub const MAX_MODELS_CLASSIC: usize = 400;
    pub const MAX_MODELS: usize = 512;
    /// Brushes in the .map (classic hlcsg 4096 (bspfile.h); current VHLT/SDHLT 32768).
    pub const MAX_BRUSHES_CLASSIC: usize = 4096;
    pub const MAX_BRUSHES: usize = 32768;
    /// Distinct textures (MAX_MAP_TEXTURES).
    pub const MAX_TEXTURES: usize = 512;
    /// Texture name length (WAD3 names are 16 bytes with the terminator).
    pub const MAX_TEXTURE_NAME: usize = 15;
    /// Entity text in the BSP (bspfile.h MAX_MAP_ENTSTRING, 128 KB).
    pub const MAX_ENTSTRING: usize = 128 * 1024;
    /// Key and value lengths the compilers and engine accept.
    pub const MAX_KEY: usize = 31;
    pub const MAX_VALUE: usize = 1023;
}

/// Textures the compilers handle themselves or ship in their own WAD (zhlt.wad / sdhlt.wad).
pub const TOOL_TEXTURES: &[&str] = &[
    "null", "skip", "hint", "solidhint", "bevel", "bevelbrush", "clipbevel", "clipbevelbrush", "boundingbox",
    "splitface", "contentempty", "contentsolid", "contentwater", "contentsky", "noclip", "noclipbevel", "nocull",
    "cliphull1", "cliphull2", "cliphull3",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Issue {
    pub level: Level,
    /// Stable id for the UI ("missing-texture", ...).
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brush: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<u64>,
    /// The texture it's about (missing or badly named).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// A one-click fix the editor offers (see `fix_label`), when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<&'static str>,
    /// P5: what to place to fix it, `<mode>:<marker or component>` (a mode map kit's; the page
    /// places it on a click: tools/editor/kits.rs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

pub(crate) fn issue(level: Level, code: &'static str, message: String) -> Issue {
    Issue { level, code, message, brush: None, entity: None, texture: None, fix: fix_for(code), place: None }
}

/// The fix offered for an issue code. `place:<class>` fixes are done by the page (it hands
/// the mapper the entity tool); the rest are `history::cmd_fix`.
pub fn fix_for(code: &str) -> Option<&'static str> {
    Some(match code {
        "bad-brush" | "outside-world" => "delete-brush",
        "no-classname" | "extra-worldspawn" => "to-world",
        "empty-brush-entity" | "no-origin" => "delete-entity",
        "dangling-target" => "clear-target",
        "no-wads" => "add-wads",
        "missing-texture" | "long-texture-name" => "replace-texture",
        "no-ct-spawns" | "no-spawns" => "place:info_player_start",
        "no-t-spawns" => "place:info_player_deathmatch",
        _ => return None,
    })
}

/// Plain words for a fix button.
pub fn fix_label(fix: &str) -> &'static str {
    match fix {
        "delete-brush" => "Delete the brush",
        "to-world" => "Make its brushes world brushes",
        "delete-entity" => "Delete it",
        "clear-target" => "Clear the target",
        "add-wads" => "Add halflife.wad and cstrike.wad",
        "replace-texture" => "Replace with the current texture",
        "place:info_player_start" => "Place a CT spawn",
        "place:info_player_deathmatch" => "Place a T spawn",
        _ => "Fix",
    }
}

/// Counts for the status bar.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Stats {
    pub brushes: usize,
    pub faces: usize,
    pub entities: usize,
    pub point_entities: usize,
    pub brush_entities: usize,
    pub textures: usize,
}

pub fn stats(doc: &MapDocument) -> Stats {
    let with_brushes: HashSet<EntityId> = doc.brushes.values().filter_map(|b| b.entity).collect();
    let textures: HashSet<String> = doc.brushes.values().flat_map(|b| b.faces.iter().map(|f| f.texture.to_ascii_lowercase())).collect();
    Stats {
        brushes: doc.brushes.len(),
        faces: doc.brushes.values().map(|b| b.faces.len()).sum(),
        entities: doc.entities.len() + 1,
        brush_entities: doc.entities.keys().filter(|e| with_brushes.contains(e)).count(),
        point_entities: doc.entities.keys().filter(|e| !with_brushes.contains(e)).count(),
        textures: textures.len(),
    }
}

/// Run every check. `known_textures` is the lower-case texture names in the map's WADs; when
/// None, the document's materials are used (and texture checks are skipped if it has none).
pub fn run(doc: &MapDocument, known_textures: Option<&HashSet<String>>) -> Vec<Issue> {
    let mut out = Vec::new();
    let st = stats(doc);

    // Brushes.
    let mut out_of_world = 0usize;
    for (id, b) in &doc.brushes {
        if let Some(p) = validate(b).first() {
            out.push(Issue { brush: Some(id.0), ..issue(Level::Error, "bad-brush", format!("Broken brush: {}", p.message())) });
            continue;
        }
        let bb = brush_bounds(b);
        if bb.min.x < -limits::WORLD_EXTENT
            || bb.min.y < -limits::WORLD_EXTENT
            || bb.min.z < -limits::WORLD_EXTENT
            || bb.max.x > limits::WORLD_EXTENT
            || bb.max.y > limits::WORLD_EXTENT
            || bb.max.z > limits::WORLD_EXTENT
        {
            out_of_world += 1;
            if out_of_world <= 20 {
                out.push(Issue {
                    brush: Some(id.0),
                    ..issue(Level::Warning, "outside-world", "This brush goes past the edge of the world (4096 units from the centre).".into())
                });
            }
        }
    }
    if out_of_world > 20 {
        out.push(Issue { fix: None, ..issue(Level::Warning, "outside-world", format!("{} more brushes go past the edge of the world.", out_of_world - 20)) });
    }
    if st.brushes > limits::MAX_BRUSHES {
        out.push(issue(Level::Error, "too-many-brushes", format!("{} brushes; the compilers stop at {}.", st.brushes, limits::MAX_BRUSHES)));
    } else if st.brushes > limits::MAX_BRUSHES_CLASSIC {
        out.push(issue(
            Level::Info,
            "many-brushes",
            format!("{} brushes: fine for current compilers, too many for old ZHLT ({}).", st.brushes, limits::MAX_BRUSHES_CLASSIC),
        ));
    }

    // Entities.
    // P5: func_detail is merged into the world by the compiler (SDHLT): not an entity or a
    // brush model in the compiled map, so it doesn't count toward those limits
    let detail: HashSet<EntityId> = doc.entities.iter().filter(|(_, e)| e.classname().eq_ignore_ascii_case("func_detail")).map(|(id, _)| *id).collect();
    let detail_models = doc.brushes.values().filter_map(|b| b.entity).filter(|e| detail.contains(e)).collect::<HashSet<_>>().len();
    let st = Stats { entities: st.entities - detail.len(), brush_entities: st.brush_entities - detail_models, ..st };
    if st.entities > limits::MAX_ENTITIES {
        out.push(issue(Level::Error, "too-many-entities", format!("{} entities; a map can hold {}.", st.entities, limits::MAX_ENTITIES)));
    } else if st.entities > limits::DEFAULT_EDICTS - 100 {
        out.push(issue(
            Level::Warning,
            "many-entities",
            format!("{} entities: close to the server's default of {} (players and dropped guns count too).", st.entities, limits::DEFAULT_EDICTS),
        ));
    }
    let models = st.brush_entities + 1;
    if models > limits::MAX_MODELS - 1 {
        out.push(issue(Level::Error, "too-many-models", format!("{} brush entities; the limit is {}.", st.brush_entities, limits::MAX_MODELS - 1)));
    } else if models > limits::MAX_MODELS_CLASSIC {
        out.push(issue(Level::Warning, "many-models", format!("{} brush entities: over the classic limit of {}.", st.brush_entities, limits::MAX_MODELS_CLASSIC - 1)));
    }
    let mut entstring = 40 + doc.world.keys.iter().map(|(k, v)| k.len() + v.len() + 6).sum::<usize>() + doc.world.wad_key().len();
    let with_brushes: HashSet<EntityId> = doc.brushes.values().filter_map(|b| b.entity).collect();
    let mut targetnames: HashSet<&str> = HashSet::new();
    for e in doc.entities.values() {
        if let Some(t) = e.get("targetname") {
            targetnames.insert(t);
        }
    }
    for (id, e) in &doc.entities {
        let eid = Some(id.0);
        entstring += 4 + e.keys.iter().map(|(k, v)| k.len() + v.len() + 6).sum::<usize>();
        let cls = e.classname();
        if cls.is_empty() {
            out.push(Issue { entity: eid, ..issue(Level::Error, "no-classname", "An entity has no classname.".into()) });
            continue;
        }
        if cls == "worldspawn" {
            out.push(Issue { entity: eid, ..issue(Level::Error, "extra-worldspawn", "There's a second worldspawn.".into()) });
        }
        // trigger_relay, trigger_auto, trigger_camera and trigger_changetarget are point entities
        let point_trigger = matches!(cls, "trigger_relay" | "trigger_auto" | "trigger_camera" | "trigger_changetarget");
        let needs_brushes = ((cls.starts_with("func_") || cls.starts_with("trigger_")) && !point_trigger) || cls == "momentary_rot_button";
        let has_brushes = with_brushes.contains(id);
        if needs_brushes && !has_brushes {
            out.push(Issue { entity: eid, ..issue(Level::Error, "empty-brush-entity", format!("{cls} has no brushes.")) });
        }
        if !has_brushes && !needs_brushes && e.origin().is_none() && !matches!(cls, "info_map_parameters" | "env_fog" | "game_player_equip") {
            out.push(Issue { entity: eid, ..issue(Level::Warning, "no-origin", format!("{cls} has no position (origin).")) });
        }
        for (k, v) in &e.keys {
            if k.len() > limits::MAX_KEY || v.len() > limits::MAX_VALUE {
                out.push(Issue { entity: eid, ..issue(Level::Warning, "long-key", format!("{cls}: the key \"{k}\" or its value is too long.")) });
            }
        }
        for key in ["target", "killtarget"] {
            if let Some(t) = e.get(key) {
                if !t.is_empty() && !targetnames.contains(t) {
                    out.push(Issue { entity: eid, ..issue(Level::Info, "dangling-target", format!("{cls} points at \"{t}\", which nothing is named.")) });
                }
            }
        }
    }
    if entstring > limits::MAX_ENTSTRING {
        out.push(issue(Level::Warning, "entstring", format!("About {} KB of entity text; older tools stop at 128 KB.", entstring / 1024)));
    }

    // Spawns (Counter-Strike: CTs use info_player_start, Ts info_player_deathmatch).
    let count = |c: &str| doc.entities.values().filter(|e| e.classname() == c).count();
    let (ct, t) = (count("info_player_start"), count("info_player_deathmatch"));
    if ct == 0 && t == 0 {
        out.push(issue(Level::Warning, "no-spawns", "No spawn points: add info_player_start (CT) and info_player_deathmatch (T).".into()));
    } else {
        if ct == 0 {
            out.push(issue(Level::Warning, "no-ct-spawns", "No CT spawns (info_player_start).".into()));
        }
        if t == 0 {
            out.push(issue(Level::Warning, "no-t-spawns", "No T spawns (info_player_deathmatch).".into()));
        }
    }

    // Textures.
    let mut used: BTreeMap<String, Vec<BrushId>> = BTreeMap::new();
    for (id, b) in &doc.brushes {
        for f in &b.faces {
            let e = used.entry(f.texture.to_ascii_lowercase()).or_default();
            if e.last() != Some(id) {
                e.push(*id);
            }
        }
    }
    if used.len() > limits::MAX_TEXTURES {
        out.push(issue(Level::Error, "too-many-textures", format!("{} different textures; a map can use {}.", used.len(), limits::MAX_TEXTURES)));
    }
    for (name, ids) in &used {
        if name.len() > limits::MAX_TEXTURE_NAME {
            out.push(Issue { brush: Some(ids[0].0), texture: Some(name.clone()), ..issue(Level::Error, "long-texture-name", format!("Texture name \"{name}\" is longer than 15 letters.")) });
        }
    }
    let from_doc: HashSet<String>;
    let known = match known_textures {
        Some(k) => Some(k),
        None if !doc.materials.is_empty() => {
            from_doc = doc.materials.keys().cloned().collect();
            Some(&from_doc)
        }
        None => None,
    };
    if let Some(known) = known {
        for (name, ids) in &used {
            if known.contains(name) {
                continue;
            }
            let (level, msg) = if TOOL_TEXTURES.contains(&name.as_str()) {
                (Level::Info, format!("\"{name}\" is a compiler tool texture; it comes from the compiler's own WAD."))
            } else {
                (Level::Error, format!("Texture \"{name}\" isn't in any of the map's WADs ({} brush{}).", ids.len(), if ids.len() == 1 { "" } else { "es" }))
            };
            let fix = if level == Level::Error { fix_for("missing-texture") } else { None };
            out.push(Issue { brush: Some(ids[0].0), texture: Some(name.clone()), fix, ..issue(level, "missing-texture", msg) });
        }
    }
    if doc.world.wads.is_empty() && !used.is_empty() {
        out.push(issue(Level::Warning, "no-wads", "The map lists no WAD files, so the compiler won't find any textures.".into()));
    }

    out.sort_by_key(|i| i.level);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::{Entity, Material};
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;

    fn codes(v: &[Issue]) -> Vec<&'static str> {
        v.iter().map(|i| i.code).collect()
    }

    #[test]
    fn clean_room_has_no_errors() {
        let mut d = MapDocument::new();
        d.world.set_wad_key("halflife.wad");
        d.insert_brush(make_box(v3(-128.0, -128.0, -16.0), v3(128.0, 128.0, 0.0), "CRATE"));
        for (c, o) in [("info_player_start", "0 0 36"), ("info_player_deathmatch", "64 0 36")] {
            let mut e = Entity::new(c);
            e.set("origin", o);
            d.insert_entity(e);
        }
        d.add_material(Material { name: "CRATE".into(), width: 64, height: 64, wad: None });
        assert!(run(&d, None).is_empty(), "{:?}", run(&d, None));
        let s = stats(&d);
        assert_eq!((s.brushes, s.faces, s.entities, s.point_entities, s.textures), (1, 6, 3, 2, 1));
    }

    #[test]
    fn flags_problems() {
        let mut d = MapDocument::new();
        let mut broken = make_box(v3(0.0, 0.0, 0.0), v3(8.0, 8.0, 8.0), "CRATE");
        broken.faces.pop();
        d.insert_brush(broken);
        d.insert_brush(make_box(v3(4000.0, 0.0, 0.0), v3(4200.0, 8.0, 8.0), "A_VERY_LONG_TEXTURE_NAME"));
        d.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(8.0, 8.0, 8.0), "NULL"));
        let door = d.insert_entity(Entity::new("func_door"));
        let mut btn = Entity::new("info_target");
        btn.set("origin", "0 0 0");
        btn.set("target", "nowhere");
        d.insert_entity(btn);
        d.insert_entity(Entity::new("info_player_start"));
        let known: HashSet<String> = ["crate".to_string()].into_iter().collect();
        let issues = run(&d, Some(&known));
        let c = codes(&issues);
        for want in ["bad-brush", "outside-world", "long-texture-name", "missing-texture", "empty-brush-entity", "dangling-target", "no-t-spawns", "no-origin", "no-wads"] {
            assert!(c.contains(&want), "missing {want}: {c:?}");
        }
        assert!(issues.iter().any(|i| i.code == "empty-brush-entity" && i.entity == Some(door.0)));
        assert!(issues.iter().any(|i| i.code == "missing-texture" && i.level == Level::Info && i.message.contains("null")));
        // Errors come first.
        assert!(issues.windows(2).all(|w| w[0].level <= w[1].level));
        // Without any texture list, missing-texture checks are skipped.
        assert!(!codes(&run(&d, None)).contains(&"missing-texture"));
    }

    #[test]
    fn limit_counts() {
        let mut d = MapDocument::new();
        for i in 0..1030 {
            let mut e = Entity::new("info_target");
            e.set("origin", &format!("{i} 0 0"));
            d.insert_entity(e);
        }
        assert!(codes(&run(&d, None)).contains(&"too-many-entities"));
    }
}
