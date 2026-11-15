//! Performance hints for the Problems panel (X2): warnings only, never errors, so they never stop
//! Compile, Test or Publish. Each names its node (a click jumps there) and gives a one-line tip.
//! Light on purpose: a reachability walk over the flow wires (graph.rs flow_from), no data flow.
//!   1. loops inside loops that run about a million steps or more (unknown counts: three deep)
//!   2. On Tick under 0.05 s whose flow spawns, shows an effect to everyone or loops
//!   3. spawning from On Tick, a Repeat or a loop body with nothing in the mode that removes it
//!   4. an effect or message to everyone from On Tick under 0.1 s
//!   5. a graph over 1,000 nodes

use super::graph::{flow_from, Graph, Problem, View, LOOPS};
use serde_json::Value;
use std::collections::BTreeSet;

/// Loops this many steps (all together) or more get hint 1.
pub const BIG_STEPS: f64 = 1_000_000.0;
/// Graphs over this many nodes get hint 5.
pub const BIG_GRAPH: usize = 1000;

/// The spawn nodes, and the nodes that remove what they make (Remove Entity takes any of them).
const SPAWNS: [(&str, &[&str]); 10] = [
    ("comp.create", &["ent.remove"]),
    ("creature.spawn", &["creature.remove", "ent.remove"]),
    ("npc.spawn", &["ent.remove"]),
    ("npc.spawn_use", &["ent.remove"]),
    ("proj.spawn", &["proj.remove", "ent.remove"]),
    ("ent.spawn_prop", &["ent.remove"]),
    ("ent.spawn_entity", &["ent.remove"]),
    ("ent.spawn_weapon", &["ent.remove"]),
    ("vol.spawn", &["vol.remove", "ent.remove"]),
    ("fx.marker", &["fx.marker_remove"]),
];

fn num(v: &View, id: &str, key: &str) -> Option<f64> {
    v.g.nodes.iter().find(|n| n.id == id)?.params.get(key).and_then(Value::as_f64)
}

/// A setting's number when its pin isn't wired (or there's no pin), else None (not known here).
fn known(v: &View, id: &str, key: &str, default: f64) -> Option<f64> {
    if v.data_source(id, key).is_some() {
        return None;
    }
    Some(num(v, id, key).unwrap_or(default))
}

/// How many times a loop goes round, when that's known without running it.
fn loop_count(v: &View, id: &str, kind: &str) -> Option<f64> {
    match kind {
        "flow.for_loop" | "flow.for_loop_break" => Some((known(v, id, "last", 9.0)? - known(v, id, "first", 0.0)? + 1.0).max(0.0)),
        "flow.while" => known(v, id, "limit", 1000.0),
        "flow.for_each_player" => Some(32.0),
        _ => None,
    }
}

/// An effect, sound or message everyone gets (not one aimed at a wired player).
fn to_everyone(v: &View, id: &str, kind: &str) -> bool {
    let aimed = |pin: &str| v.data_source(id, pin).is_some();
    match kind {
        "fx.sound_to" | "fx.marker_remove" | "msg.notify_admins" | "msg.team_chat" => false,
        "fx.icon_over" => !aimed("viewer"),
        "snd.play_2d" => !aimed("player"),
        k if k.starts_with("fx.") || matches!(k, "snd.play_at" | "snd.play_on" | "snd.loop") => true,
        k if k.starts_with("msg.") => !aimed("player"),
        _ => false,
    }
}

fn kind_of(v: &View, id: &str) -> &'static str {
    v.defs.get(id).map(|d| d.kind.as_str()).unwrap_or("")
}
fn body_pin(v: &View, id: &str) -> Option<&'static str> {
    let k = kind_of(v, id);
    LOOPS.iter().find(|(x, _)| *x == k).map(|(_, p)| *p)
}
fn body<'a>(v: &View<'a>, id: &'a str) -> BTreeSet<&'a str> {
    flow_from(v, id, &[body_pin(v, id).unwrap_or("body")])
}
/// The loops a loop's body runs (inside it, however deep).
fn inner<'a>(v: &View<'a>, loops: &[&'a str], id: &'a str) -> Vec<&'a str> {
    let b = body(v, id);
    loops.iter().copied().filter(|m| *m != id && b.contains(m)).collect()
}
/// (steps, any count unknown, depth): this loop's count x the biggest loop inside it.
fn steps<'a>(v: &View<'a>, loops: &[&'a str], id: &'a str, seen: &mut Vec<&'a str>) -> (f64, bool, usize) {
    seen.push(id);
    let (mut best, mut unknown, mut depth) = (1.0, false, 0);
    for m in inner(v, loops, id) {
        if seen.contains(&m) {
            continue;
        }
        let (s, u, d) = steps(v, loops, m, seen);
        best = f64::max(best, s);
        unknown |= u;
        depth = depth.max(d);
    }
    seen.pop();
    let c = loop_count(v, id, kind_of(v, id));
    (c.unwrap_or(1.0) * best, unknown || c.is_none(), depth + 1)
}

pub fn hints<'a>(g: &'a Graph, v: &View<'a>) -> Vec<Problem> {
    let mut p = Vec::new();
    let kind = |id: &str| kind_of(v, id);
    let loops: Vec<&'a str> = g.nodes.iter().map(|n| n.id.as_str()).filter(|id| body_pin(v, id).is_some()).collect();

    // 1. nested loops (said once, on the outermost)
    let inside_another: BTreeSet<&str> = loops.iter().flat_map(|l| inner(v, &loops, l)).collect();
    for l in loops.iter().copied().filter(|l| !inside_another.contains(l)) {
        let (s, unknown, depth) = steps(v, &loops, l, &mut vec![]);
        if depth >= 2 && s > BIG_STEPS {
            p.push(Problem::warn(Some(l), format!("This runs about {} steps; big loops spread over frames and may run slowly.", group(s))));
        } else if depth >= 3 && unknown {
            p.push(Problem::warn(Some(l), "Loops three deep: this can run a great many steps; big loops spread over frames and may run slowly."));
        }
    }

    // 2. and 4. On Tick
    for t in g.nodes.iter().filter(|n| kind(&n.id) == "event.on_tick") {
        let every = num(v, &t.id, "interval").unwrap_or(0.1);
        let flow = flow_from(v, &t.id, &["then"]);
        let busy = flow.iter().any(|n| SPAWNS.iter().any(|(k, _)| *k == kind(n)) || to_everyone(v, n, kind(n)) || body_pin(v, n).is_some());
        if every < 0.05 && busy {
            p.push(Problem::warn(Some(&t.id), "This runs every frame or nearly: what it spawns, shows to everyone or loops over adds up fast. Half a second or more is usually plenty."));
        }
        if every < 0.1 {
            for n in flow.iter().filter(|n| to_everyone(v, n, kind(n))) {
                p.push(Problem::warn(Some(n), "Players on slow connections may see some effects skipped."));
            }
        }
    }

    // 3. spawning again and again with nothing that removes it
    let mut again: BTreeSet<&str> = BTreeSet::new();
    for n in &g.nodes {
        match kind(&n.id) {
            "event.on_tick" => again.extend(flow_from(v, &n.id, &["then"])),
            "timer.repeat" => again.extend(flow_from(v, &n.id, &["tick"])),
            _ if body_pin(v, &n.id).is_some() => again.extend(body(v, &n.id)),
            _ => {}
        }
    }
    for s in again {
        let Some((_, removers)) = SPAWNS.iter().find(|(k, _)| *k == kind(s)) else { continue };
        // (a projectile with a lifetime goes by itself)
        if kind(s) == "proj.spawn" && known(v, s, "life", 5.0).is_none_or(|l| l > 0.0) {
            continue;
        }
        if !g.nodes.iter().any(|n| removers.contains(&kind(&n.id))) {
            p.push(Problem::warn(Some(s), "This keeps spawning; once the entity limit is reached, spawns are skipped."));
        }
    }

    // 5. a very big graph
    if g.nodes.len() > BIG_GRAPH {
        p.push(Problem::warn(None, format!("This graph has {} nodes. Consider moving parts into Functions or Macros.", g.nodes.len())));
    }
    p
}

/// 1234567 -> "1,234,567"
fn group(x: f64) -> String {
    let s = format!("{}", x.round() as u64);
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::studio::graph;
    use serde_json::json;

    fn g(nodes: Value, edges: Value) -> Graph {
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "perf_test", "name": "Perf"}, "nodes": nodes, "edges": edges})).unwrap()
    }
    fn w(i: usize, a: &str, ap: &str, b: &str, bp: &str) -> Value {
        json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
    }
    fn found(gr: &Graph, text: &str) -> Vec<Problem> {
        let all = graph::validate(gr);
        assert!(!all.iter().any(|p| p.level == "error"), "a hint graph has errors: {all:#?}");
        all.into_iter().filter(|p| p.message.contains(text)).collect()
    }

    #[test]
    fn hint_1_nested_loops_of_a_million_steps() {
        let gr = g(
            json!([{"id": "rs", "type": "event.round_started"}, {"id": "a", "type": "flow.for_loop", "params": {"first": 0, "last": 999}}, {"id": "b", "type": "flow.for_loop", "params": {"first": 0, "last": 1999}}, {"id": "p", "type": "debug.print", "params": {"text": "x", "chat": false}}]),
            json!([w(1, "rs", "then", "a", "exec"), w(2, "a", "body", "b", "exec"), w(3, "b", "body", "p", "exec")]),
        );
        let h = found(&gr, "steps; big loops spread over frames");
        assert_eq!(h.len(), 1, "{h:#?}");
        assert_eq!(h[0].node.as_deref(), Some("a"));
        assert!(h[0].message.contains("about 2,000,000 steps"), "{}", h[0].message);
        assert_eq!(h[0].level, "warning");
        // small ones: nothing
        let small = g(
            json!([{"id": "rs", "type": "event.round_started"}, {"id": "a", "type": "flow.for_loop", "params": {"first": 0, "last": 9}}, {"id": "b", "type": "flow.for_loop", "params": {"first": 0, "last": 9}}, {"id": "p", "type": "debug.print", "params": {"text": "x", "chat": false}}]),
            json!([w(1, "rs", "then", "a", "exec"), w(2, "a", "body", "b", "exec"), w(3, "b", "body", "p", "exec")]),
        );
        assert!(found(&small, "big loops spread over frames").is_empty());
    }

    #[test]
    fn hint_2_a_fast_tick_that_spawns() {
        let gr = g(json!([{"id": "t", "type": "event.on_tick", "params": {"interval": 0.01}}, {"id": "s", "type": "ent.spawn_prop"}, {"id": "r", "type": "ent.remove"}]), json!([w(1, "t", "then", "s", "exec"), w(2, "s", "then", "r", "exec"), w(3, "s", "out", "r", "in")]));
        let h = found(&gr, "runs every frame or nearly");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].node.as_deref(), Some("t"));
    }

    #[test]
    fn hint_3_spawning_with_nothing_that_removes_it() {
        let gr = g(json!([{"id": "rs", "type": "event.round_started"}, {"id": "rp", "type": "timer.repeat", "params": {"every": 1, "times": 0}}, {"id": "s", "type": "ent.spawn_prop"}]), json!([w(1, "rs", "then", "rp", "exec"), w(2, "rp", "tick", "s", "exec")]));
        let h = found(&gr, "This keeps spawning");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].node.as_deref(), Some("s"));
        // with a Remove Entity somewhere in the mode: nothing
        let mut ok = gr.clone();
        ok.nodes.push(serde_json::from_value(json!({"id": "x", "type": "ent.remove", "pos": {"x": 0, "y": 0}})).unwrap());
        assert!(hints(&ok, &View::new(&ok)).iter().all(|p| !p.message.contains("keeps spawning")));
    }

    #[test]
    fn hint_4_effects_to_everyone_on_a_fast_tick() {
        let gr = g(json!([{"id": "t", "type": "event.on_tick", "params": {"interval": 0.07}}, {"id": "e", "type": "fx.lights"}]), json!([w(1, "t", "then", "e", "exec")]));
        let h = found(&gr, "slow connections");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].node.as_deref(), Some("e"));
        // aimed at one player: nothing
        let one = g(
            json!([{"id": "t", "type": "event.on_tick", "params": {"interval": 0.07}}, {"id": "pk", "type": "flow.pick_player"}, {"id": "m", "type": "msg.chat", "params": {"text": "hi"}}]),
            json!([w(1, "t", "then", "pk", "exec"), w(2, "pk", "then", "m", "exec"), w(3, "pk", "player", "m", "player")]),
        );
        assert!(hints(&one, &View::new(&one)).iter().all(|p| !p.message.contains("slow connections")));
    }

    #[test]
    fn hint_5_a_very_big_graph() {
        let mut nodes = vec![json!({"id": "rs", "type": "event.round_started"})];
        let mut edges = vec![];
        for k in 0..BIG_GRAPH {
            nodes.push(json!({"id": format!("p{k}"), "type": "debug.print", "params": {"text": "x", "chat": false}}));
            edges.push(w(k, &if k == 0 { "rs".to_string() } else { format!("p{}", k - 1) }, "then", &format!("p{k}"), "exec"));
        }
        let gr = g(json!(nodes), json!(edges));
        let h = found(&gr, "Consider moving parts into Functions or Macros");
        assert_eq!(h.len(), 1);
        assert!(h[0].node.is_none());
    }

    #[test]
    fn templates_and_example_modes_get_no_hints() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/views/tools/studio/templates");
        let mut n = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".graph.json") {
                continue;
            }
            let gr = graph::parse(&std::fs::read_to_string(e.path()).unwrap()).unwrap();
            let flat = graph::flatten(&gr);
            let h = hints(flat.as_ref(), &View::new(flat.as_ref()));
            assert!(h.is_empty(), "{name}: {h:#?}");
            n += 1;
        }
        assert!(n >= 9, "only {n} templates found");
    }
}
