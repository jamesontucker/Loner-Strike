//! X2: nodes with many pins (Sequence and Multi Gate up to 32 outputs) and wireable For Each /
//! Pick Random (Who, Team, the kind or name, Only these, Index, Break): they check clean, generate
//! what they should, keep old graphs' code, and compile with amxxpc.

use super::*;
use std::path::{Path, PathBuf};

fn g(nodes: Value, edges: Value, vars: Value) -> Graph {
    graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "pins_test", "name": "Pins"}, "nodes": nodes, "edges": edges, "variables": vars})).unwrap()
}
fn w(i: usize, a: &str, ap: &str, b: &str, bp: &str) -> Value {
    json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
}
fn clean(gr: &Graph) {
    let p: Vec<_> = graph::validate(gr).into_iter().filter(|p| p.level != "info").collect();
    assert!(p.is_empty(), "{p:#?}");
}

/// Sequence with 12 outputs and Multi Gate with 20, each output to its own Print.
pub fn many_outputs() -> Graph {
    let mut nodes = vec![json!({"id": "rs", "type": "event.round_started"}), json!({"id": "seq", "type": "flow.sequence", "params": {"count": 12}}), json!({"id": "mg", "type": "flow.multi_gate", "params": {"count": 20, "loop": true}})];
    let mut edges = vec![w(0, "rs", "then", "seq", "exec")];
    for k in 0..12 {
        nodes.push(json!({"id": format!("p{k}"), "type": "debug.print", "params": {"text": format!("seq {k}"), "chat": false}}));
        edges.push(w(edges.len(), "seq", &format!("then{k}"), &format!("p{k}"), "exec"));
    }
    edges.push(w(edges.len(), "p11", "then", "mg", "exec"));
    for k in 0..20 {
        nodes.push(json!({"id": format!("m{k}"), "type": "debug.print", "params": {"text": format!("gate {k}"), "chat": false}}));
        edges.push(w(edges.len(), "mg", &format!("then{k}"), &format!("m{k}"), "exec"));
    }
    g(json!(nodes), json!(edges), json!([]))
}

/// For Each Player with Who from a variable (and one with Team, one with Only these), Index and
/// Break wired; For Each Entity and Pick Random Entity with a wired name; Pick Random Player
/// with a wired Who.
pub fn wired_loops() -> Graph {
    let nodes = json!([
        {"id": "rs", "type": "event.round_started"},
        {"id": "who", "type": "var.get", "params": {"var": "v_who"}},
        {"id": "fe", "type": "flow.for_each_player", "params": {"which": "alive"}},
        {"id": "pr", "type": "debug.print", "params": {"text": "player", "chat": false}},
        {"id": "big", "type": "math.compare", "params": {"op": "ge", "b": 3}},
        {"id": "br", "type": "flow.branch"},
        {"id": "team", "type": "player.get_team"},
        {"id": "fe2", "type": "flow.for_each_player", "params": {"which": "all"}},
        {"id": "pr2", "type": "debug.print", "params": {"text": "teammate", "chat": false}},
        {"id": "list", "type": "array.players", "params": {"which": "bots"}},
        {"id": "fe3", "type": "flow.for_each_player"},
        {"id": "pr3", "type": "debug.print", "params": {"text": "bot", "chat": false}},
        {"id": "nm", "type": "text.literal", "params": {"value": "info_player_start"}},
        {"id": "fe4", "type": "flow.for_each_entity", "params": {"by": "class", "name": "ls_respawn_point"}},
        {"id": "pr4", "type": "debug.print", "params": {"text": "spawn", "chat": false}},
        {"id": "pk", "type": "flow.pick_player", "params": {"which": "all"}},
        {"id": "pr5", "type": "debug.print", "params": {"text": "picked", "chat": false}},
        {"id": "pe", "type": "flow.pick_entity", "params": {"by": "class", "name": "info_player_deathmatch"}},
        {"id": "pr6", "type": "debug.print", "params": {"text": "spot", "chat": false}}
    ]);
    let edges = json!([
        w(1, "rs", "then", "fe", "exec"), w(2, "who", "out", "fe", "who"), w(3, "fe", "each", "pr", "exec"), w(4, "fe", "index", "pr", "value"),
        w(5, "fe", "index", "big", "a"), w(6, "pr", "then", "br", "exec"), w(7, "big", "out", "br", "cond"), w(8, "br", "true", "fe", "break"),
        w(9, "fe", "then", "fe2", "exec"), w(10, "fe", "player", "team", "player"), w(11, "team", "out", "fe2", "team"), w(12, "fe2", "each", "pr2", "exec"), w(13, "fe2", "player", "pr2", "value"),
        w(14, "fe2", "then", "fe3", "exec"), w(15, "list", "out", "fe3", "only"), w(16, "fe3", "each", "pr3", "exec"), w(17, "fe3", "player", "pr3", "value"),
        w(18, "fe3", "then", "fe4", "exec"), w(19, "nm", "out", "fe4", "name"), w(20, "fe4", "each", "pr4", "exec"), w(21, "fe4", "index", "pr4", "value"),
        w(22, "fe4", "then", "pk", "exec"), w(23, "who", "out", "pk", "who"), w(24, "pk", "then", "pr5", "exec"), w(25, "pk", "player", "pr5", "value"),
        w(26, "pr5", "then", "pe", "exec"), w(27, "nm", "out", "pe", "name"), w(28, "pe", "then", "pr6", "exec"), w(29, "pe", "entity", "pr6", "value")
    ]);
    g(nodes, edges, json!([{"id": "v_who", "name": "Who to count", "type": "enum:ls_who", "scope": "mode", "default": 7}]))
}

#[test]
fn sequence_and_multi_gate_take_up_to_32_outputs() {
    let gr = many_outputs();
    clean(&gr);
    for debug in [false, true] {
        let src = gen::generate(&gr, debug).source;
        for k in 0..12 {
            assert!(src.contains(&format!("\"seq {k}\"")), "Then {k}");
        }
        for k in 0..20 {
            assert!(src.contains(&format!("\"gate {k}\"")), "Out {k}");
        }
        // 20 outputs: the used-bits mask has 20 ones
        assert!(src.contains(&format!("== {}", (1u32 << 20) - 1)), "the Multi Gate mask");
    }
    // the count is clamped at 32 (and the pins follow it)
    let mut big = many_outputs();
    big.nodes.iter_mut().find(|n| n.id == "seq").unwrap().params.insert("count".into(), json!(40));
    assert_eq!(graph::View::new(&big).exec_outs("seq").len(), 32);
}

#[test]
fn for_each_and_pick_take_wired_filters() {
    let gr = wired_loops();
    clean(&gr);
    for debug in [false, true] {
        let src = gen::generate(&gr, debug).source;
        // Who from the variable (the enum's number is lsg_match's code), Team instead, Only these
        // (a wired input is used as it is, or read into a local first in a debug build: lsi<n>)
        let either = |a: &str, b: &str| src.contains(a) || src.contains(b);
        assert!(src.contains("if (!(lsg_match(p, a)))") && either("lsgA = g_v", "lsgA = lsi"), "a wired Who (read once, at the start): {src}");
        assert!(src.contains("is_user_connected(p) && lsg_team(p) == "), "a wired Team");
        assert!(either("lsg_acopy(snap, lsg_aplayers(7))", "lsg_acopy(snap, lsi"), "Only these");
        assert!(!src.contains("lsg_match(p, 1)") && !src.contains("lsg_match(p, 0)"), "the Who settings aren't used when wired");
        // the kind or name from a wire, Index and Break
        assert!(either("find_ent_by_class(p, \"info_player_start\")", "find_ent_by_class(p, lsi"), "a wired name");
        assert!(src.contains("_:float(n)"), "Index");
        assert!(src.contains("_break(") || src.contains("break(c"), "Break's own function");
    }
}

#[test]
fn old_graphs_keep_their_settings() {
    // nothing wired: the settings, as before
    let gr = g(
        json!([{"id": "rs", "type": "event.round_started"}, {"id": "fe", "type": "flow.for_each_player", "params": {"which": "ct"}}, {"id": "p", "type": "debug.print", "params": {"text": "x", "chat": false}},
               {"id": "fe2", "type": "flow.for_each_entity", "params": {"by": "name", "name": "door1"}}, {"id": "pk", "type": "flow.pick_player", "params": {"which": "bots"}}]),
        json!([w(1, "rs", "then", "fe", "exec"), w(2, "fe", "each", "p", "exec"), w(3, "fe", "then", "fe2", "exec"), w(4, "fe2", "then", "pk", "exec")]),
        json!([]),
    );
    clean(&gr);
    let src = gen::generate(&gr, false).source;
    assert!(src.contains("lsg_match(p, 4)") && src.contains("find_ent_by_tname(k, \"door1\")") && src.contains("lsg_match(p, 7)"), "{src}");
}

#[test]
fn many_pins_and_wired_loops_compile_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls-x2pins-amxxpc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let inc = work.join("include");
    write_includes(&inc).unwrap();
    for (name, gr) in [("many_outputs", many_outputs()), ("wired_loops", wired_loops())] {
        for debug in [false, true] {
            let out = gen::generate(&gr, debug);
            let file = format!("x2_{name}{}.sma", if debug { "_dbg" } else { "" });
            let src = work.join(&file);
            std::fs::write(&src, &out.source).unwrap();
            let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &work.join(file.replace(".sma", ".amxx"))).unwrap();
            assert!(ok, "{file} didn't compile:\n{output}");
            assert!(parse_compiler(&output, &file).is_empty(), "{file} has warnings:\n{output}");
        }
    }
    let _ = std::fs::remove_dir_all(&work);
}

/// A graph of `n` Print nodes in a chain from Round Started (n - 1 wires after the first).
fn chain(n: usize) -> Graph {
    let mut nodes = vec![json!({"id": "rs", "type": "event.round_started"})];
    let mut edges = vec![w(0, "rs", "then", "p0", "exec")];
    for k in 0..n - 1 {
        nodes.push(json!({"id": format!("p{k}"), "type": "debug.print", "params": {"text": "x", "chat": false}}));
        if k > 0 {
            edges.push(w(k, &format!("p{}", k - 1), "then", &format!("p{k}"), "exec"));
        }
    }
    g(json!(nodes), json!(edges), json!([]))
}

#[test]
fn a_graph_at_the_node_cap_saves_and_opens_again_and_one_over_is_refused_at_save() {
    let dir = std::env::temp_dir().join(format!("ls-x2-cap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let at = chain(graph::MAX_NODES);
    assert_eq!(at.nodes.len(), graph::MAX_NODES);
    assert!(graph::size_error(&at).is_none());
    save_graph(&dir, "pins_test", &at).unwrap();
    let back = read_graph(&dir, "pins_test").unwrap();
    assert_eq!(back.nodes.len(), graph::MAX_NODES);
    assert!(!graph::validate(&back).iter().any(|p| p.level == "error" && p.message.contains("too big")));
    let over = chain(graph::MAX_NODES + 1);
    let e = save_graph(&dir, "pins_test", &over).unwrap_err();
    assert!(e.contains("too big to save") && e.contains(&graph::MAX_NODES.to_string()), "{e}");
    // the file on disk is still the one that fits
    assert_eq!(read_graph(&dir, "pins_test").unwrap().nodes.len(), graph::MAX_NODES);
    let _ = std::fs::remove_dir_all(&dir);
}
