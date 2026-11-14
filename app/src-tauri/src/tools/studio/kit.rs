//! Mode map kits (docs/notes/X.md, "Mode map kit format"): what a map needs for one game mode,
//! as data. A Studio mode's kit is made from its graph (its map markers, the components its
//! nodes act on, the creature types it spawns, the author's advice), saved next to the graph and
//! put in `<app dir>/kits/` when the mode is installed. Hand-written and community kits are
//! checked with `check_kit` before they're kept.

use super::graph::{self, Graph, MarkerDef};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const KIT_SCHEMA: &str = "loner-strike/mode-kit";
pub const KIT_VERSION: u64 = 1;

/// `<app dir>/kits`: the kits of the modes this computer has.
pub fn kits_dir() -> PathBuf {
    crate::sys::app_dir().join("kits")
}

/// The components a node kind acts on in the map (what the map must have for it to do anything).
fn component_of(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "comp.on_capture" | "comp.on_capture_lost" | "comp.capture_lock" => "ls_capture_point",
        "comp.on_zone_stage" | "comp.on_zone_tick" | "comp.zone_stage" | "comp.zone_next" | "comp.zone_skip" => "ls_zone_shrinker",
        "comp.on_extract" | "comp.extract_open" => "ls_extraction_zone",
        "comp.on_wave_start" | "comp.on_wave_spawn" | "comp.on_wave_cleared" | "comp.wave_start" | "comp.wave_stop" | "wave.table" => "ls_wave_spawner",
        "comp.on_loot" | "comp.crate_open" => "ls_loot_container",
        "comp.on_checkpoint" => "ls_checkpoint",
        "comp.on_barricade_broken" | "comp.on_barricade_damaged" | "comp.on_barricade_repaired" | "comp.barricade_hp" | "comp.barricade_set_hp" | "comp.barricade_repair" => "ls_barricade",
        "comp.on_meeting_end" => "ls_meeting_point",
        "comp.trap_fire" => "ls_trigger_trap",
        "comp.door" => "ls_timed_door",
        "shop.menu" | "shop.open" | "shop.price" | "shop.on_buy" => "ls_shop",
        _ => return None,
    })
}

/// Create Component's choice keys as gm_components' classnames (as gen.rs writes them).
fn created_class(choice: &str) -> &str {
    match choice {
        "ls_trap" => "ls_trigger_trap",
        "ls_extraction" => "ls_extraction_zone",
        k => k,
    }
}

fn marker_json(m: &MarkerDef) -> Value {
    let shape = match m.ty.as_str() {
        "area" => "box",
        "path" => "line",
        _ => "diamond",
    };
    let mut draw = json!({
        "colour": if m.colour.is_empty() { "#d9c27a" } else { m.colour.as_str() },
        "icon": if m.icon.is_empty() { "flag" } else { m.icon.as_str() },
        "shape": shape,
    });
    if let Some(s) = m.size {
        draw["size"] = json!(s);
    }
    if let Some(r) = m.radius {
        draw["radius"] = json!(r);
    }
    let keys: Vec<Value> = m
        .keys
        .iter()
        .map(|k| {
            let mut o = json!({"name": k.name, "type": k.ty, "default": k.default, "help": k.help});
            if !k.choices.is_empty() {
                o["choices"] = json!(k.choices);
            }
            if let Some(x) = k.min {
                o["min"] = json!(x);
            }
            if let Some(x) = k.max {
                o["max"] = json!(x);
            }
            o
        })
        .collect();
    json!({
        "id": m.id,
        "label": m.label,
        "description": m.description,
        "type": m.ty,
        "required": {"min": m.min, "max": m.max},
        "keys": keys,
        "draw": draw,
    })
}

/// The mode map kit of a graph (one that passed `graph::validate`).
pub fn kit_of(g: &Graph) -> Value {
    let created: Vec<&str> = g.nodes.iter().filter(|n| n.kind == "comp.create").map(|n| created_class(n.params.get("kind").and_then(Value::as_str).unwrap_or("ls_capture_point"))).collect();
    let mut components: Vec<&str> = Vec::new();
    for n in &g.nodes {
        if let Some(c) = component_of(&n.kind) {
            if !created.contains(&c) && !components.contains(&c) {
                components.push(c);
            }
        }
    }
    components.sort_unstable();
    let creatures = super::gen::needs(g).creatures;
    let mut kit = json!({
        "schema": KIT_SCHEMA,
        "version": KIT_VERSION,
        "mode": {"uid": g.mode.uid, "id": g.mode.id, "name": g.mode.name, "description": g.mode.description},
        "source": "studio",
        "markers": g.markers.iter().map(marker_json).collect::<Vec<_>>(),
        "components": components.iter().map(|c| json!({"classname": c, "min": 1, "max": Value::Null, "note": ""})).collect::<Vec<_>>(),
        "creatures": creatures,
        "notes": g.map_kit.as_ref().map(|k| k.notes.clone()).unwrap_or_default(),
        // what the mode does beyond its own game (community badges: "runs server commands"...)
        "capabilities": graph::capabilities(g),
    });
    if let Some(k) = &g.map_kit {
        let mut map = json!({});
        if !k.size.is_empty() {
            map["size"] = json!(k.size);
        }
        if k.players_min.is_some() || k.players_max.is_some() {
            map["players"] = json!({"min": k.players_min.unwrap_or(1), "max": k.players_max.unwrap_or(32)});
        }
        kit["map"] = map;
    }
    kit
}

/// Checks a kit from anywhere (a file, a community package): Ok, or what's wrong with it.
pub fn check_kit(v: &Value) -> Result<(), String> {
    let bad = |why: &str| Err(format!("That mode map kit isn't one this app can use: {why}."));
    if v.get("schema").and_then(Value::as_str) != Some(KIT_SCHEMA) {
        return bad("it isn't a kit");
    }
    if v.get("version").and_then(Value::as_u64) != Some(KIT_VERSION) {
        return bad("it's a newer version");
    }
    let mode = &v["mode"];
    let id = mode.get("id").and_then(Value::as_str).unwrap_or("");
    let name = mode.get("name").and_then(Value::as_str).unwrap_or("");
    let desc = mode.get("description").and_then(Value::as_str).unwrap_or("");
    if !mode.get("uid").and_then(Value::as_str).is_some_and(graph::valid_uid) {
        return bad("its mode has no uid (a UUID v4)");
    }
    if !graph::valid_mode_id(id) || name.trim().is_empty() || !graph::valid_label(name, 40) || !graph::valid_label(desc, 60) {
        return bad("its mode id or name");
    }
    if !matches!(v.get("source").and_then(Value::as_str), Some("studio" | "hand")) {
        return bad("its source (studio or hand)");
    }
    // the markers and map advice: the same rules as a graph's
    let mut markers = Vec::new();
    for m in v.get("markers").and_then(Value::as_array).cloned().unwrap_or_default() {
        let req = &m["required"];
        let draw = &m["draw"];
        let as_def = json!({
            "id": m["id"], "label": m["label"], "description": m.get("description").cloned().unwrap_or(json!("")), "type": m["type"],
            "min": req.get("min").cloned().unwrap_or(json!(0)), "max": req.get("max").cloned().unwrap_or(Value::Null),
            "keys": m.get("keys").cloned().unwrap_or(json!([])),
            "colour": draw.get("colour").cloned().unwrap_or(json!("")), "icon": draw.get("icon").cloned().unwrap_or(json!("")),
            "size": draw.get("size").cloned().unwrap_or(Value::Null), "radius": draw.get("radius").cloned().unwrap_or(Value::Null),
        });
        let shape = draw.get("shape").and_then(Value::as_str).unwrap_or("diamond");
        if !["diamond", "box", "sphere", "cylinder", "line"].contains(&shape) {
            return bad("a marker's shape");
        }
        markers.push(serde_json::from_value::<MarkerDef>(as_def).map_err(|_| "That mode map kit isn't one this app can use: a marker's fields.".to_string())?);
    }
    let map = v.get("map").cloned().unwrap_or(json!({}));
    let players = map.get("players").cloned().unwrap_or(json!({}));
    let g = Graph {
        schema: graph::SCHEMA.into(),
        version: graph::VERSION,
        mode: serde_json::from_value(json!({"id": id, "name": name})).map_err(|e| e.to_string())?,
        variables: vec![],
        nodes: vec![],
        edges: vec![],
        comments: vec![],
        enums: vec![],
        structs: vec![],
        functions: vec![],
        macros: vec![],
        markers,
        event_graphs: vec![],
        map_kit: Some(graph::MapKitInfo {
            size: map.get("size").and_then(Value::as_str).unwrap_or("").into(),
            players_min: players.get("min").and_then(Value::as_u64).map(|x| x.min(1000) as u32),
            players_max: players.get("max").and_then(Value::as_u64).map(|x| x.min(1000) as u32),
            notes: v.get("notes").and_then(Value::as_str).unwrap_or("").into(),
        }),
    };
    if let Some(why) = graph::marker_problems(&g).into_iter().next() {
        return Err(why);
    }
    let comps = v.get("components").and_then(Value::as_array).cloned().unwrap_or_default();
    if comps.len() > 32 {
        return bad("too many components");
    }
    for c in &comps {
        let cls = c.get("classname").and_then(Value::as_str).unwrap_or("");
        let min = c.get("min").and_then(Value::as_u64).unwrap_or(0);
        let max = c.get("max").and_then(Value::as_u64);
        let note = c.get("note").and_then(Value::as_str).unwrap_or("");
        if !cls.starts_with("ls_") || !graph::valid_marker_id(cls) || min > 64 || max.is_some_and(|x| x == 0 || x > 64 || x < min) || !graph::valid_label(note, 120) {
            return bad("a component");
        }
    }
    let creatures = v.get("creatures").and_then(Value::as_array).cloned().unwrap_or_default();
    if creatures.len() > 32 || creatures.iter().any(|c| !c.as_str().is_some_and(|s| !s.is_empty() && graph::valid_name(s))) {
        return bad("its creature types");
    }
    Ok(())
}

/// Puts a kit in the kits folder (after checking it).
pub fn install_kit_in(dir: &Path, kit: &Value) -> Result<PathBuf, String> {
    check_kit(kit)?;
    let id = kit["mode"]["id"].as_str().unwrap_or_default();
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't keep the mode's map kit ({e})."))?;
    let path = dir.join(format!("{id}.kit.json"));
    let text = serde_json::to_string_pretty(kit).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&path, text.as_bytes()).map_err(|e| format!("Couldn't keep the mode's map kit ({e})."))?;
    Ok(path)
}

#[cfg_attr(test, allow(dead_code))]
pub fn install_kit(kit: &Value) -> Result<PathBuf, String> {
    install_kit_in(&kits_dir(), kit)
}

/// Every good kit in a folder (bad ones are left out), by mode id.
pub fn list_kits_in(dir: &Path) -> Vec<Value> {
    let mut out: Vec<Value> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".kit.json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter(|t| t.len() < 512 * 1024)
        .filter_map(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(|v| check_kit(v).is_ok())
        .collect();
    out.sort_by(|a, b| a["mode"]["id"].as_str().cmp(&b["mode"]["id"].as_str()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_with_markers() -> Graph {
        graph::parse(
            &json!({
                "schema": graph::SCHEMA, "version": graph::VERSION,
                "mode": {"id": "kit_test", "uid": "0b6a7a1e-2f43-4c1d-9e55-3b1f8f6c2d10", "name": "Kit test", "description": "Markers"},
                "variables": [], "comments": [],
                "nodes": [
                    {"id": "e", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}},
                    {"id": "fe", "type": "marker.for_each", "pos": {"x": 300, "y": 0}, "params": {"marker": "spawn"}},
                    {"id": "cs", "type": "creature.spawn", "pos": {"x": 600, "y": 0}, "params": {"kind": "zombie"}},
                    {"id": "cap", "type": "comp.on_capture", "pos": {"x": 0, "y": 300}, "params": {}},
                    {"id": "mk", "type": "comp.create", "pos": {"x": 0, "y": 600}, "params": {"kind": "ls_trap"}},
                    {"id": "tr", "type": "comp.trap_fire", "pos": {"x": 300, "y": 600}, "params": {}}
                ],
                "edges": [
                    {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "fe", "pin": "exec"}},
                    {"id": "w2", "from": {"node": "fe", "pin": "each"}, "to": {"node": "cs", "pin": "exec"}},
                    {"id": "w3", "from": {"node": "fe", "pin": "entity"}, "to": {"node": "cs", "pin": "at_ent"}},
                    {"id": "w4", "from": {"node": "cap", "pin": "then"}, "to": {"node": "mk", "pin": "exec"}},
                    {"id": "w5", "from": {"node": "mk", "pin": "then"}, "to": {"node": "tr", "pin": "exec"}},
                    {"id": "w6", "from": {"node": "mk", "pin": "out"}, "to": {"node": "tr", "pin": "trap"}}
                ],
                "markers": [
                    {"id": "spawn", "label": "Creature spawn", "description": "Where they come in", "type": "point", "min": 2, "max": 16,
                     "keys": [{"name": "wave", "type": "number", "default": 1, "help": "First wave", "min": 1, "max": 99},
                              {"name": "side", "type": "choice", "default": "north", "help": "", "choices": [["north", "North"], ["south", "South"]]}],
                     "colour": "#c0392b", "icon": "skull"},
                    {"id": "base", "label": "Base", "type": "area", "min": 1, "max": 1, "size": [512, 512, 128]},
                    {"id": "patrol", "label": "Patrol", "type": "path"}
                ],
                "mapKit": {"size": "medium", "playersMin": 2, "playersMax": 16, "notes": "Keep the base near the middle."}
            })
            .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn kit_from_a_graph() {
        let g = graph_with_markers();
        let p = graph::validate(&g);
        assert!(!graph::has_errors(&p), "{p:#?}");
        let kit = kit_of(&g);
        check_kit(&kit).unwrap();
        assert_eq!(kit["mode"]["id"], "kit_test");
        assert_eq!(kit["mode"]["uid"], "0b6a7a1e-2f43-4c1d-9e55-3b1f8f6c2d10");
        assert_eq!(kit["markers"].as_array().unwrap().len(), 3);
        let spawn = &kit["markers"][0];
        assert_eq!(spawn["required"], json!({"min": 2, "max": 16}));
        assert_eq!(spawn["draw"], json!({"colour": "#c0392b", "icon": "skull", "shape": "diamond"}));
        assert_eq!(spawn["keys"][1]["choices"], json!([["north", "North"], ["south", "South"]]));
        assert_eq!(kit["markers"][1]["draw"]["shape"], "box");
        assert_eq!(kit["markers"][1]["draw"]["size"], json!([512.0, 512.0, 128.0]));
        assert_eq!(kit["markers"][2]["draw"]["shape"], "line");
        assert_eq!(kit["markers"][2]["required"]["max"], Value::Null);
        // the capture event needs a capture point in the map; the trap is made at run time
        let comps: Vec<&str> = kit["components"].as_array().unwrap().iter().map(|c| c["classname"].as_str().unwrap()).collect();
        assert_eq!(comps, ["ls_capture_point"]);
        assert_eq!(kit["creatures"], json!(["zombie"]));
        assert_eq!(kit["map"], json!({"size": "medium", "players": {"min": 2, "max": 16}}));
        assert_eq!(kit["notes"], "Keep the base near the middle.");
    }

    #[test]
    fn bad_markers_and_kits_are_refused() {
        let mut g = graph_with_markers();
        g.markers[1].id = "spawn".into();
        assert!(graph::has_errors(&graph::validate(&g)));
        let mut g = graph_with_markers();
        g.markers[0].keys[0].name = "origin".into();
        assert!(graph::has_errors(&graph::validate(&g)));
        let mut g = graph_with_markers();
        g.markers[0].id = "ls_capture_point".into();
        assert!(graph::has_errors(&graph::validate(&g)));
        // a node naming a marker the graph doesn't have
        let mut g = graph_with_markers();
        g.nodes[1].params.insert("marker".into(), json!("nope"));
        assert!(graph::has_errors(&graph::validate(&g)));

        let good = kit_of(&graph_with_markers());
        for (path, v) in [
            ("/schema", json!("x")),
            ("/version", json!(2)),
            ("/mode/id", json!("Bad Id")),
            ("/mode/uid", json!("not-a-uuid")),
            ("/source", json!("other")),
            ("/markers/0/type", json!("cube")),
            ("/markers/0/draw/shape", json!("star")),
            ("/markers/0/keys/0/default", json!("x")),
            ("/components/0/classname", json!("func_wall")),
            ("/creatures/0", json!("a;b")),
            ("/notes", json!("x".repeat(601))),
        ] {
            let mut k = good.clone();
            *k.pointer_mut(path).unwrap() = v;
            assert!(check_kit(&k).is_err(), "{path}");
        }
    }

    #[test]
    fn kits_install_and_list() {
        let dir = std::env::temp_dir().join(format!("ls-kits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let kit = kit_of(&graph_with_markers());
        let p = install_kit_in(&dir, &kit).unwrap();
        assert!(p.ends_with("kit_test.kit.json"));
        std::fs::write(dir.join("junk.kit.json"), "{\"schema\":\"x\"}").unwrap();
        let all = list_kits_in(&dir);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], kit);
        assert!(install_kit_in(&dir, &json!({"schema": "x"})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
