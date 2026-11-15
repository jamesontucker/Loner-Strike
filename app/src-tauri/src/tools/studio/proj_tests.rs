//! Projectiles (T1fix): Spawn Projectile with its transform inputs and its own On Hit / On
//! Expired, the Projectile Hit event, Remove, Velocity get and set. Graph text never reaches a
//! command, a bad model path falls back to a grenade, two Spawn Projectile nodes keep their own
//! callbacks, and the plugin compiles with the real amxxpc when the CrossOver game is there.

use super::{gen, graph, parse_compiler, run_amxxpc, write_includes};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Every projectile node (the every-node check counts them as covered by this file's graph).
pub const KINDS: [&str; 5] = ["proj.spawn", "proj.on_hit", "proj.remove", "proj.velocity", "proj.set_velocity"];

fn proj_graph() -> graph::Graph {
    let e = |id: &str, a: &str, ap: &str, b: &str, bp: &str| json!({"id": id, "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
    graph::parse(
        &json!({
            "schema": graph::SCHEMA, "version": graph::VERSION,
            "mode": {"id": "proj_test", "uid": "6a1f0c2e-9b3d-4e55-8c21-7d4e2a9b0f31", "name": "Projectiles", "description": "Every projectile node"},
            "variables": [], "comments": [],
            "nodes": [
                {"id": "k", "type": "event.player_spawned"},
                // a rocket from the player's eyes, 16 forward, with its own hit flow
                {"id": "rocket", "type": "proj.spawn", "params": {"model": "models/rpgrocket.mdl", "speed": 1200, "damage": 100, "radius": 200, "explode": true, "trail": true}},
                {"id": "boom", "type": "msg.broadcast", "params": {"text": "rocket hit"}},
                {"id": "gone", "type": "msg.broadcast", "params": {"text": "rocket expired"}},
                // a bouncing sprite from a point along a direction, a different hit flow, and a hostile path
                {"id": "pt", "type": "vec.make", "params": {"x": 10, "y": 20, "z": 30}},
                {"id": "dir", "type": "vec.make", "params": {"x": 0, "y": 0, "z": 1}},
                {"id": "orb", "type": "proj.spawn", "params": {"model": "../../evil\";quit;\".spr", "bounce": 3, "gravity": 1, "scale": 2}},
                {"id": "bonk", "type": "msg.broadcast", "params": {"text": "orb hit"}},
                {"id": "fast", "type": "proj.velocity"},
                {"id": "twice", "type": "vec.scale", "params": {"n": 2}},
                {"id": "setv", "type": "proj.set_velocity"},
                // the global event, and Remove
                {"id": "any", "type": "proj.on_hit"},
                {"id": "rm", "type": "proj.remove"}
            ],
            "edges": [
                e("w1", "k", "then", "rocket", "exec"), e("w2", "k", "player", "rocket", "owner"),
                e("w3", "rocket", "on_hit", "boom", "exec"), e("w4", "rocket", "on_expire", "gone", "exec"),
                e("w5", "rocket", "then", "orb", "exec"), e("w6", "pt", "out", "orb", "pos"), e("w7", "dir", "out", "orb", "dir"),
                e("w8", "orb", "on_hit", "bonk", "exec"),
                e("w9", "bonk", "then", "setv", "exec"), e("w10", "orb", "out", "setv", "proj"), e("w11", "orb", "out", "fast", "proj"),
                e("w12", "fast", "out", "twice", "a"), e("w13", "twice", "out", "setv", "vel"),
                e("w14", "any", "then", "rm", "exec"), e("w15", "any", "proj", "rm", "proj")
            ]
        })
        .to_string(),
    )
    .unwrap()
}

#[test]
fn projectiles_generate_with_their_own_callbacks() {
    let g = proj_graph();
    // with the Limits setting the Studio adds, hosts' value is used (clamped to what the plugin holds)
    let mut lim = g.clone();
    lim.variables.push(serde_json::from_value(super::limits::var_json(super::limits::get("projectiles").unwrap())).unwrap());
    let src = gen::generate(&lim, false).source;
    assert!(src.contains("set_cvar_num(\"ls_proj_max\", clamp(floatround("), "{src}");
    // (the hostile path is refused by the check; the plugin is generated from it anyway, to show
    // nothing of it gets through)
    let problems = graph::validate(&g);
    assert!(problems.iter().filter(|p| p.level == "error").all(|p| p.node.as_deref() == Some("orb") && p.pin.as_deref() == Some("model")), "{problems:?}");
    for debug in [false, true] {
        let src = gen::generate(&g, debug).source;
        // loaded with the map; a bad path flies as a grenade, nothing of it in the source
        assert!(src.contains("ls_proj_precache(\"models/rpgrocket.mdl\")") && src.contains("ls_proj_precache(\"models/w_hegrenade.mdl\")"), "{src}");
        assert!(!src.contains("evil") && !src.contains(";quit;"), "the path never reaches the plugin");
        // how many fly at once is the mode's Limits setting (the default without one); the engine's
        // entity slots are checked before each spawn
        assert!(src.contains("set_cvar_num(\"ls_proj_max\", 64)") && src.contains("lsg_ent_room(\"Spawn Projectile\")"), "{src}");
        // a trail waits for room in the mode's message budget; the model goes through the precache budget
        assert!(src.contains("lsg_msg_room(\"Spawn Projectile\", 1, 0, ") && src.contains("if (lsg_pre_model(\"models/rpgrocket.mdl\"))"), "{src}");
        // each Spawn Projectile keeps its own hit flow: the dispatch goes by the node that made it
        assert!(src.contains("public ls_on_proj_hit_node(") && src.contains("public ls_on_proj_expire(") && src.contains("public ls_on_proj_hit(proj, owner"));
        // (each node's hit stock: from its heading to the next stock or node; Show Message's code
        // may be its own stock that the hit flow calls, so look for the call or the text)
        let bodies: Vec<&str> = src.split("stock lsg_pjhit").skip(1).map(|b| &b[..b.find("\nstock ").unwrap_or(b.len())]).collect();
        assert_eq!(bodies.len(), 2, "one hit flow per Spawn Projectile");
        assert_ne!(bodies[0], bodies[1], "the two Spawn Projectile nodes run different hit flows");
        // the projectile is noted with its node and context; the From point and offsets are used
        assert!(src.contains("lsg_pj_note(g_lsgOut[") && src.contains("lsg_proj_from(") && src.contains("ls_proj_set_velocity("));
        assert!(src.contains("ls_proj_remove(") && src.contains("lsg_proj_vel("));
        assert!(!src.contains("server_cmd(") || !src.contains("ls_proj_test"));
    }
}

/// A graph saved before the spawn transform standard (docs/notes/FX.md): Rotation offset was
/// `rot_off`, Forward/Right/Up offset the settings off_f/off_r/off_u. It loads with the standard
/// pins (angles_offset, a Local offset from a Make Vector) and fires from the same spot.
#[test]
fn old_spawn_projectile_graphs_load_with_the_standard_pins() {
    let e = |id: &str, a: &str, ap: &str, b: &str, bp: &str| json!({"id": id, "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
    let old = json!({
        "schema": graph::SCHEMA, "version": graph::VERSION,
        "mode": {"id": "proj_old", "uid": "0c8d2e4a-6b1f-4c3d-9e5a-7f2b1d3c4e50", "name": "Old projectiles", "description": "Saved before"},
        "variables": [], "comments": [],
        "nodes": [
            {"id": "k", "type": "event.player_spawned"},
            {"id": "turn", "type": "vec.make", "params": {"x": 0, "y": 15, "z": 0}},
            // moved 32 forward, 8 right, 4 up, turned by a wire into the old Rotation offset
            {"id": "a", "type": "proj.spawn", "params": {"model": "models/rpgrocket.mdl", "off_f": 32, "off_r": 8, "off_u": 4}},
            // the old defaults (16, 0, 0): no wire needed
            {"id": "b", "type": "proj.spawn", "params": {"model": "models/rpgrocket.mdl", "off_f": 16, "off_r": 0, "off_u": 0}}
        ],
        "edges": [e("w1", "k", "then", "a", "exec"), e("w2", "k", "player", "a", "owner"), e("w3", "turn", "out", "a", "rot_off"), e("w4", "a", "then", "b", "exec"), e("w5", "k", "player", "b", "owner")]
    });
    let g = graph::parse(&old.to_string()).unwrap();
    assert!(g.edges.iter().any(|x| x.from.node == "turn" && x.to.node == "a" && x.to.pin == "angles_offset"));
    assert!(!g.edges.iter().any(|x| x.to.pin == "rot_off"));
    assert!(g.nodes.iter().filter(|n| n.kind == "proj.spawn").all(|n| !n.params.contains_key("off_f") && !n.params.contains_key("off_r") && !n.params.contains_key("off_u")));
    let v = g.nodes.iter().find(|n| n.id == "a_offset").expect("a Make Vector for the old offsets");
    assert_eq!((v.params["x"].as_f64(), v.params["y"].as_f64(), v.params["z"].as_f64()), (Some(32.0), Some(8.0), Some(4.0)));
    assert!(g.edges.iter().any(|x| x.from.node == "a_offset" && x.to.node == "a" && x.to.pin == "offset"));
    assert!(!g.nodes.iter().any(|n| n.id == "b_offset"), "the default needs no wire");
    let problems = graph::validate(&g);
    assert!(problems.iter().all(|p| p.level != "error"), "{problems:?}");
    // the same spot: the offset from the vector, the default 16 forward, the facing kept
    let src = gen::generate(&g, false).source;
    assert!(src.contains("lsg_proj_from(") && src.contains(", 16.0, 0.0, 0.0, -1, 1, o, d)"), "{src}");
    assert!(src.matches("lsg_vc(").count() >= 3, "{src}");
    // loading again changes nothing
    let again = graph::parse(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(again.nodes.len(), g.nodes.len());
    assert_eq!(again.edges.len(), g.edges.len());
}

#[test]
fn spawn_projectile_waits_like_say_and_choose() {
    // (the graph above uses every one of them)
    let g = proj_graph();
    for k in KINDS {
        assert!(g.nodes.iter().any(|n| n.kind == k), "{k}");
    }
    assert!(graph::latent("proj.spawn"));
    assert!(graph::deferred("proj.spawn", "on_hit") && graph::deferred("proj.spawn", "on_expire") && !graph::deferred("proj.spawn", "then"));
}

#[test]
fn projectiles_compile_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped the compile: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls-studio-proj-{}", std::process::id()));
    let inc = work.join("include");
    write_includes(&inc).unwrap();
    let g = proj_graph();
    for (name, debug) in [("proj_test", false), ("proj_test_dbg", true)] {
        let file = format!("studio_{name}.sma");
        let src = work.join(&file);
        std::fs::write(&src, gen::generate(&g, debug).source).unwrap();
        let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &work.join(file.replace(".sma", ".amxx"))).unwrap();
        assert!(ok, "{file} didn't compile:\n{output}");
        assert!(parse_compiler(&output, &file).is_empty(), "{file} has warnings:\n{output}");
    }
    let _ = std::fs::remove_dir_all(&work);
}
