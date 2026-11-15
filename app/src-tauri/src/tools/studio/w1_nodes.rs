//! W1's utility nodes: what they compile to. The pure ones (spatial queries, random picks, math,
//! text, number arrays, vectors, relative space and aim) are expressions over the helpers in
//! w1_helpers.pawn; the flow ones (Multi Gate, Do Once Per Player, Toggle Variable, Reverse, On
//! Tick) are in gen.rs's node_code with the other flow nodes. Settings reach the plugin only as
//! numbers, choices from the registry, or quoted and cleaned names: graph text never becomes code.

use super::*;

/// The pure node kinds made here.
pub(super) const PURE: &[&str] = &[
    "spatial.players_in_radius", "spatial.nearest_player", "spatial.entities_in_radius", "spatial.nearest_entity", "spatial.line_of_sight",
    "random.chance", "random.bool", "random.weighted",
    "math.map_range", "math.trig", "math.atan2", "math.angle_convert", "flow.nearly_equal",
    "text.split", "text.replace", "text.substring", "text.upper", "text.lower", "text.trim", "text.starts_with", "text.ends_with", "text.join", "text.format_time", "text.format_number",
    "array.sum", "array.min", "array.max", "array.average", "array.slice",
    "vec.dot", "vec.cross", "vec.random_in_radius", "vec.rotate", "vec.lerp", "vec.to_angles", "vec.forward",
    "rel.to_local", "rel.to_world", "rel.position_of", "rel.angle_to", "rel.is_in_front", "rel.is_behind",
    "aim.eye_pos", "aim.dir", "aim.point",
    "math.bitwise", "spatial.free_spot", "player.language",
];

/// The flow node kinds (and the event) made in node_code.
pub(super) const FLOW: &[&str] = &["array.reverse", "flow.multi_gate", "flow.do_once_per_player", "var.toggle", "event.on_tick", "debug.draw_line", "debug.draw_box", "debug.draw_point", "debug.log_line"];

/// Flow nodes whose outputs are kept (g_lsgO), and those with more than one flow input.
pub(super) const STORE: &[&str] = &["var.toggle"];
pub(super) const MULTI: &[&str] = &["flow.multi_gate", "flow.do_once_per_player"];

/// Does the graph use any of them (they need the version 3 helpers and these)?
pub(super) fn used(g: &Graph) -> bool {
    g.nodes.iter().any(|n| PURE.contains(&n.kind.as_str()) || FLOW.contains(&n.kind.as_str()))
}

pub(super) const HELPERS: &str = include_str!("w1_helpers.pawn");

fn team_code(c: &str) -> i32 {
    match c {
        "t" => 1,
        "ct" => 2,
        _ => 0,
    }
}

/// A pure node's output as an expression.
pub(super) fn pure(gen: &Gen, node: &str, pin: &str) -> String {
    let (n, def) = gen.node(node);
    let i = |p: &str, t: &str| gen.need(node, p, t);
    let wired = |p: &str| gen.v.data_source(node, p).is_some();
    // the point a query is around: a point wired in, else an entity, else nothing (-1 / 0)
    let around = || -> (String, String) {
        let pos = if wired("pos") { i("pos", "vector") } else { "-1".into() };
        let ent = if wired("to") { i("to", "entity") } else { "0".into() };
        (pos, ent)
    };
    let num = |k: &str| float_lit(gen.num(n, def, k));
    let flag = |k: &str| if gen.boolean(n, def, k) { 1 } else { 0 };
    let unit = |k: &str| if gen.choice(n, def, k) == "rad" { "radian" } else { "degrees" };
    match def.kind.as_str() {
        "spatial.players_in_radius" => {
            let (pos, ent) = around();
            format!("lsg_w1_pin({pos}, {ent}, {}, {}, {})", i("radius", "number"), flag("alive"), team_code(gen.choice(n, def, "team")))
        }
        "spatial.nearest_player" => {
            let (pos, ent) = around();
            let bots = match gen.choice(n, def, "bots") {
                "humans" => 1,
                "bots" => 2,
                _ => 0,
            };
            let call = format!("lsg_w1_np({pos}, {ent}, {}, {}, {}, {bots}", num("max"), flag("alive"), team_code(gen.choice(n, def, "team")));
            if pin == "dist" { format!("Float:{call}, true)") } else { format!("{call})") }
        }
        "spatial.entities_in_radius" => {
            let (pos, ent) = around();
            format!("lsg_w1_ein({pos}, {ent}, {}, {}, {})", i("radius", "number"), pawn_str(&ent_name_text(gen.text(n, def, "class"))), gen.limit_expr("search_found"))
        }
        "spatial.nearest_entity" => {
            let (pos, ent) = around();
            let call = format!("lsg_w1_ne({pos}, {ent}, {}, {}", pawn_str(&ent_name_text(gen.text(n, def, "class"))), num("max"));
            if pin == "dist" { format!("Float:{call}, true)") } else { format!("{call})") }
        }
        "spatial.line_of_sight" => {
            let side = |e: &str, p: &str| (if wired(e) { i(e, "entity") } else { "0".into() }, if wired(p) { i(p, "vector") } else { "-1".into() });
            let ((ae, ap), (be, bp)) = (side("a", "a_pos"), side("b", "b_pos"));
            let m = flag("monsters");
            if pin == "hit" { format!("lsg_w1_los({ae}, {ap}, {be}, {bp}, {m}, true)") } else { format!("lsg_w1_los({ae}, {ap}, {be}, {bp}, {m})") }
        }
        "random.chance" => format!("(random_float(0.0, 100.0) < ({}) ? 1 : 0)", i("pct", "number")),
        "random.bool" => "random_num(0, 1)".into(),
        "random.weighted" => {
            let k = gen.v.g.nodes.iter().position(|x| x.id == node).unwrap_or(0);
            let pick = format!("lsg_w1_wpick({}, {k})", i("weights", "array<number>"));
            if pin == "item" {
                let ty = gen.v.out_type(node, "item").unwrap_or_default();
                if !wired("items") {
                    return Gen::zero(&ty);
                }
                Gen::cell_read(&ty, format!("lsg_aget{}({}, float({pick}))", kind_of(&ty), gen.needt(node, "items")))
            } else {
                format!("float({pick})")
            }
        }
        "math.map_range" => format!("lsg_w1_map({}, {}, {}, {}, {}, {})", i("a", "number"), i("in_min", "number"), i("in_max", "number"), i("out_min", "number"), i("out_max", "number"), flag("clamp")),
        "math.trig" => {
            let f = match gen.choice(n, def, "op") {
                "cos" => "floatcos",
                "tan" => "floattan",
                _ => "floatsin",
            };
            format!("{f}({}, {})", i("a", "number"), unit("unit"))
        }
        "math.atan2" => format!("floatatan2({}, {}, {})", i("y", "number"), i("x", "number"), unit("unit")),
        "math.angle_convert" => {
            let a = i("a", "number");
            if gen.choice(n, def, "op") == "to_deg" { format!("(({a}) * 57.2957795)") } else { format!("(({a}) * 0.0174532925)") }
        }
        "flow.nearly_equal" => format!("(floatabs(({}) - ({})) <= floatabs({}) ? 1 : 0)", i("a", "number"), i("b", "number"), i("tol", "number")),
        "text.split" => format!("lsg_w1_split({}, {})", i("a", "text"), i("sep", "text")),
        "text.replace" => format!("lsg_w1_repl({}, {}, {})", i("a", "text"), i("find", "text"), i("with", "text")),
        "text.substring" => format!("lsg_w1_sub({}, {}, {})", i("a", "text"), i("start", "number"), i("len", "number")),
        "text.upper" => format!("lsg_w1_case({}, 1)", i("a", "text")),
        "text.lower" => format!("lsg_w1_case({}, 0)", i("a", "text")),
        "text.trim" => format!("lsg_w1_trim({})", i("a", "text")),
        "text.starts_with" => format!("(lsg_w1_starts({}, {}) ? 1 : 0)", i("a", "text"), i("b", "text")),
        "text.ends_with" => format!("(lsg_w1_ends({}, {}) ? 1 : 0)", i("a", "text"), i("b", "text")),
        "text.join" => format!("lsg_w1_join({}, {})", i("array", "array<text>"), i("sep", "text")),
        "text.format_time" => {
            let style = match gen.choice(n, def, "style") {
                "ms" => 1,
                "hms" => 2,
                _ => 0,
            };
            format!("lsg_w1_time({}, {style})", i("secs", "number"))
        }
        "text.format_number" => format!("lsg_w1_num({}, {}, {})", i("a", "number"), gen.num(n, def, "decimals").round() as i64, flag("thousands")),
        "array.sum" | "array.min" | "array.max" | "array.average" => {
            let op = match def.kind.as_str() {
                "array.min" => 1,
                "array.max" => 2,
                "array.average" => 3,
                _ => 0,
            };
            let a = i("array", "array<number>");
            if pin == "index" { format!("lsg_w1_astat({a}, {op}, true)") } else { format!("lsg_w1_astat({a}, {op})") }
        }
        "array.slice" => format!("lsg_w1_slice({}, {}, {})", gen.needt(node, "array"), i("start", "number"), i("count", "number")),
        "vec.dot" => format!("lsg_w1_dot({}, {})", i("a", "vector"), i("b", "vector")),
        "vec.cross" => format!("lsg_w1_cross({}, {})", i("a", "vector"), i("b", "vector")),
        "vec.random_in_radius" => format!("lsg_w1_rnd({}, {}, {})", i("c", "vector"), i("radius", "number"), if gen.choice(n, def, "mode") == "3d" { 1 } else { 0 }),
        "vec.rotate" => format!("lsg_w1_rot({}, {}, {})", i("a", "vector"), i("yaw", "number"), i("pitch", "number")),
        "vec.lerp" => format!("lsg_w1_vlerp({}, {}, {})", i("a", "vector"), i("b", "vector"), i("alpha", "number")),
        "vec.to_angles" => format!("lsg_w1_toang({})", i("a", "vector")),
        "vec.forward" => format!("lsg_w1_fwd({})", i("a", "vector")),
        "rel.to_local" => format!("lsg_w1_local({}, {})", i("thing", "entity"), i("pos", "vector")),
        "rel.to_world" => format!("lsg_w1_world({}, {})", i("thing", "entity"), i("off", "vector")),
        "rel.position_of" => format!("lsg_w1_relc({}, {}, {})", i("a", "entity"), i("b", "entity"), match pin {
            "right" => 1,
            "up" => 2,
            _ => 0,
        }),
        "rel.angle_to" => format!("lsg_w1_angto({}, {})", i("a", "entity"), i("b", "entity")),
        "rel.is_in_front" | "rel.is_behind" => format!("(lsg_w1_cone({}, {}, {}, {}) ? 1 : 0)", i("a", "entity"), i("b", "entity"), num("cone"), if def.kind == "rel.is_behind" { 1 } else { 0 }),
        "aim.eye_pos" => format!("lsg_vplayer({}, 1)", i("player", "player")),
        "aim.dir" => format!("lsg_vplayer({}, 2)", i("player", "player")),
        "aim.point" => format!("lsg_w1_aim({}, floatmin({}, float({})), {})", i("player", "player"), num("max"), gen.limit_expr("aim_trace"), match pin {
            "ent" => 1,
            "normal" => 2,
            "hit" => 3,
            _ => 0,
        }),
        "math.bitwise" => {
            let op = match gen.choice(n, def, "op") {
                "or" => 1,
                "xor" => 2,
                "not" => 3,
                "shl" => 4,
                "shr" => 5,
                _ => 0,
            };
            format!("lsg_w1_bits({}, {}, {op})", i("a", "number"), i("b", "number"))
        }
        "spatial.free_spot" => {
            let hull = match gen.choice(n, def, "hull") {
                "duck" => "HULL_HEAD",
                "large" => "HULL_LARGE",
                "point" => "HULL_POINT",
                _ => "HULL_HUMAN",
            };
            let k = gen.v.g.nodes.iter().position(|x| x.id == node).unwrap_or(0);
            format!("lsg_w1_free({k}, {}, {hull}, {}, {}, {})", i("pos", "vector"), num("max"), gen.limit_expr("free_spot_tries"), if pin == "out" { 1 } else { 0 })
        }
        "player.language" => format!("lsg_w1_lang({}, {})", i("player", "player"), if gen.choice(n, def, "source") == "game" { 1 } else { 0 }),
        _ => "0".into(),
    }
}

/// A debug drawing's color.
pub(super) fn rgb(c: &str) -> (u8, u8, u8) {
    match c {
        "green" => (0, 255, 0),
        "blue" => (60, 120, 255),
        "yellow" => (255, 230, 0),
        "white" => (255, 255, 255),
        "orange" => (255, 140, 0),
        "purple" => (190, 60, 255),
        _ => (255, 40, 40),
    }
}

/// On Tick: its interval, but no faster than the mode's Limits setting of runs a second.
pub(super) fn tick_secs(gen: &Gen, id: &str) -> String {
    let (n, def) = gen.node(id);
    format!("floatmax({}, 1.0 / float({}))", float_lit(gen.num(n, def, "interval").max(0.0)), gen.limit_expr("tick_rate"))
}

#[cfg(test)]
mod tests {
    use super::super::generate;
    use super::{FLOW, PURE};
    use crate::tools::studio::graph::{self, parse, registry};

    const G: &str = include_str!("testdata/every_node_w1.graph.json");

    fn snapshot(name: &str, got: &str) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/studio/testdata").join(name);
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() || !path.exists() {
            std::fs::write(&path, got).unwrap();
        }
        let want = std::fs::read_to_string(&path).unwrap();
        assert!(want == got, "{name} changed; run with UPDATE_SNAPSHOTS=1 if that's intended.");
    }

    /// The graph with every utility node: it checks clean (but for the fast On Tick's warning),
    /// every one is in it and in the registry, every flow node becomes a function, both builds
    /// are snapshotted (and compiled with amxxpc in mod.rs).
    #[test]
    fn every_utility_node_generates() {
        let g = parse(G).unwrap();
        assert!(graph::validate_core(&g).is_empty(), "{:#?}", graph::validate_core(&g));
        let all = graph::validate(&g);
        let errs: Vec<_> = all.iter().filter(|p| p.level == "error").collect();
        assert!(errs.is_empty(), "{errs:#?}");
        let ticks: Vec<_> = all.iter().filter(|p| p.message.contains("On Tick runs")).collect();
        assert_eq!(ticks.len(), 1, "only the 0.02 s tick with a Line of Sight is warned: {all:#?}");
        assert!(ticks[0].message.contains("50 times a second") && ticks[0].message.contains("Line of Sight"), "{:?}", ticks[0].message);
        for k in PURE.iter().chain(FLOW) {
            assert!(registry().get(k).is_some(), "{k} isn't in the registry");
            assert!(g.nodes.iter().any(|n| n.kind == *k), "every_node_w1 lacks {k}");
        }
        for debug in [false, true] {
            let r = generate(&g, debug);
            for n in g.nodes.iter().filter(|n| { let d = registry().get(&n.kind).unwrap(); !d.pure && !d.reroute }) {
                assert!(r.spans.iter().any(|s| s.node == n.id), "{} isn't in the plugin", n.id);
            }
            snapshot(if debug { "every_node_w1.debug.sma" } else { "every_node_w1.sma" }, &r.source);
        }
        let src = generate(&g, false).source;
        // spatial: around a point (-1 entity 0) or an entity; the class quoted; caps in the helpers
        assert!(src.contains("lsg_w1_np(-1, ") && src.contains("lsg_w1_pin("), "nearest player around a point, players in radius");
        assert!(src.contains("\"info_player_start\"") && src.contains("stock lsg_w1_ein(") && src.contains("n < most && looked++ < 4096") && src.contains("\"info_player_start\", 64)"));
        // aim: one trace per player per frame, at most 8192 units
        assert!(src.contains("floatmax(maxd, 16.0)") && src.contains("lsg_w1_aim("));
        // On Tick: every frame, each one when its interval has gone by, Delta in ctx[0]
        assert!(src.contains("register_forward(FM_StartFrame, \"lsg_w1_frame\")") && src.contains("public lsg_w1_frame()"));
        assert!(src.contains("now - at[0] >= floatmax(0.1, 1.0 / float(100))") && src.contains("now - at[1] >= floatmax(0.02, 1.0 / float(100))"), "intervals");
        // Multi Gate: a bit per output, Reset its own function; Do Once Per Player: a bit per player
        assert!(src.contains("g_lsgLeft[") && src.contains("_reset(ctx[])") && src.contains("_reset_player(ctx[])"));
        // Toggle Variable flips and keeps the new value
        assert!(src.contains(" = !g_v"));
        // Truncate: toward zero
        assert!(src.contains("floatround_tozero"));
        // debug drawings only while testing; Log Line in both, rate-limited; bits, free spot, language
        let dbg = generate(&g, true).source;
        assert!(!src.contains("\tif (!lsg_msg_room(\"Draw Debug") && dbg.matches("\tif (!lsg_msg_room(\"Draw Debug").count() == 3 && dbg.contains("\t\tlsg_w1_line(lsi") && dbg.contains("lsg_msg_room(\"Draw Debug Line\", 1, 0, 48)"));
        assert!(src.contains("lsg_w1_log(") && src.contains(", 5)") && src.contains("lsg_w1_bits(12.0, 2.0, 4)") && src.contains("HULL_HUMAN, 256.0, 64, 1)"));
        assert!(src.contains("lsg_w1_lang(ctx[0], 1)") && src.contains("query_client_cvar(id, \"cl_language\""));
        // a mode without them has none of it
        let plain = generate(&parse(include_str!("testdata/markers.graph.json")).unwrap(), false).source;
        assert!(!plain.contains("lsg_w1_"));
    }

    /// Text and names in their settings only ever become quoted, cleaned literals.
    #[test]
    fn utility_settings_never_become_code() {
        let evil = "\"); server_cmd(\"quit\"); //";
        let g = graph::from_value(serde_json::json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "w1_evil", "name": "Evil"},
            "nodes": [{"id": "rs", "type": "event.round_started"},
                {"id": "t", "type": "text.literal", "params": {"value": "abc"}},
                {"id": "r", "type": "text.replace", "params": {"find": evil, "with": evil}},
                {"id": "s", "type": "text.split", "params": {"sep": evil}},
                {"id": "j", "type": "text.join", "params": {"sep": evil}},
                {"id": "v", "type": "vec.make"},
                {"id": "e", "type": "spatial.nearest_entity", "params": {"class": evil}},
                {"id": "fmt", "type": "text.format_time", "params": {"style": evil}},
                {"id": "lg", "type": "debug.log_line", "params": {"text": evil}},
                {"id": "fs", "type": "spatial.free_spot", "params": {"hull": evil}},
                {"id": "p1", "type": "debug.print", "params": {"text": "[0]"}},
                {"id": "p2", "type": "debug.print", "params": {"text": "[0]"}},
                {"id": "p3", "type": "debug.print", "params": {"text": "[0]"}},
                {"id": "p4", "type": "debug.print", "params": {"text": "[0]"}},
                {"id": "p5", "type": "debug.print", "params": {"text": "[0]"}}],
            "edges": [{"id": "w1", "from": {"node": "t", "pin": "out"}, "to": {"node": "r", "pin": "a"}},
                {"id": "w2", "from": {"node": "t", "pin": "out"}, "to": {"node": "s", "pin": "a"}},
                {"id": "w3", "from": {"node": "s", "pin": "out"}, "to": {"node": "j", "pin": "array"}},
                {"id": "w4", "from": {"node": "v", "pin": "out"}, "to": {"node": "e", "pin": "pos"}},
                {"id": "w5", "from": {"node": "rs", "pin": "then"}, "to": {"node": "p1", "pin": "exec"}},
                {"id": "w6", "from": {"node": "r", "pin": "out"}, "to": {"node": "p1", "pin": "value"}},
                {"id": "w7", "from": {"node": "p1", "pin": "then"}, "to": {"node": "p2", "pin": "exec"}},
                {"id": "w8", "from": {"node": "j", "pin": "out"}, "to": {"node": "p2", "pin": "value"}},
                {"id": "w9", "from": {"node": "p2", "pin": "then"}, "to": {"node": "p3", "pin": "exec"}},
                {"id": "w10", "from": {"node": "e", "pin": "out"}, "to": {"node": "p3", "pin": "value"}},
                {"id": "w11", "from": {"node": "p3", "pin": "then"}, "to": {"node": "p4", "pin": "exec"}},
                {"id": "w12", "from": {"node": "fmt", "pin": "out"}, "to": {"node": "p4", "pin": "value"}},
                {"id": "w13", "from": {"node": "p4", "pin": "then"}, "to": {"node": "lg", "pin": "exec"}},
                {"id": "w14", "from": {"node": "v", "pin": "out"}, "to": {"node": "fs", "pin": "pos"}},
                {"id": "w15", "from": {"node": "lg", "pin": "then"}, "to": {"node": "p5", "pin": "exec"}},
                {"id": "w16", "from": {"node": "fs", "pin": "out"}, "to": {"node": "p5", "pin": "value"}}]})).unwrap();
        for debug in [false, true] {
            let src = generate(&g, debug).source;
            assert!(!src.contains("server_cmd(\"quit"), "{src}");
            for l in src.lines().filter(|l| !l.trim_start().starts_with("//")) {
                assert_eq!(l.replace("^\"", "").replace("'\"'", "").matches('"').count() % 2, 0, "{l}");
            }
            // (a choice outside the registry's own is the default)
            assert!(src.contains("lsg_w1_time(") && src.contains(", 0)") && src.contains("HULL_HUMAN"));
        }
    }

    /// The registry: help, keywords, and what Compare, Math, Flip Flop and Math Function are
    /// found by.
    #[test]
    fn utility_nodes_are_findable() {
        let r = registry();
        let raw: serde_json::Value = serde_json::from_str(include_str!("../../../../ui/src/views/tools/studio/nodes.json")).unwrap();
        let kw = |k: &str| raw["nodes"].as_array().unwrap().iter().find(|d| d["type"] == k).and_then(|d| d["keywords"].as_str()).unwrap_or("").to_string();
        for w in ["mod", "modulo", "%"] {
            assert!(kw("math.op").split_whitespace().any(|x| x == w), "math.op: {w}");
        }
        assert!(kw("math.fn").contains("sign") && kw("math.fn").contains("truncate"));
        assert!(kw("flow.flipflop").contains("toggle") && kw("flow.flipflop").contains("switch"));
        for w in [">", ">=", "<", "<=", "==", "!=", "greater", "less"] {
            assert!(kw("math.compare").split_whitespace().any(|x| x == w), "compare: {w}");
        }
        let cmp = r.get("math.compare").unwrap();
        let op = cmp.params.iter().find(|p| p.id == "op").unwrap();
        assert!(op.choices.iter().any(|(k, l)| k == "gt" && l == "> more than") && op.choices.iter().any(|(k, l)| k == "ne" && l == "≠ not equal to"));
        let f = r.get("math.fn").unwrap().params.iter().find(|p| p.id == "op").unwrap();
        assert!(f.choices.iter().any(|(k, _)| k == "trunc"));
        for k in PURE.iter().chain(FLOW) {
            assert!(!kw(k).is_empty(), "{k} has no keywords");
        }
    }
}
