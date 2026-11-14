//! Timeline curves: every way a key goes to the next (Linear, Constant, Ease In, Ease Out, Ease
//! In-Out, Custom with own or Auto tangents) checks clean, compiles into small stocks with no
//! tables, and with the real amxxpc without warnings; bad tangents are refused.

use super::{gen, graph, parse_compiler, run_amxxpc, write_includes};
use serde_json::json;
use std::path::{Path, PathBuf};

pub fn curves_graph() -> graph::Graph {
    graph::from_value(json!({
        "schema": "loner-strike/graph", "version": 3, "mode": {"id": "curves_test", "name": "Curves"},
        "timelines": [{"id": "tl1", "name": "Lift", "length": 6, "tick": 0.05, "tracks": [
            {"id": "h", "name": "Height", "kind": "float", "keys": [
                {"t": 0, "v": 0, "ease": "constant"}, {"t": 1, "v": 10, "ease": "in"}, {"t": 2, "v": 20, "ease": "out"},
                {"t": 3, "v": 30, "ease": "ease"}, {"t": 4, "v": 40, "ease": "custom", "tout": 50}, {"t": 5, "v": 0, "ease": "custom", "tin": -10}, {"t": 6, "v": 5}]},
            {"id": "p", "name": "Spot", "kind": "vector", "keys": [{"t": 0, "v": [0, 0, 0], "ease": "custom", "tout": [100, 0, 0]}, {"t": 6, "v": [300, 0, 60]}]}]}],
        "nodes": [
            {"id": "rs", "type": "event.round_started", "params": {}},
            {"id": "tp", "type": "timeline.play", "params": {"timeline": "tl1"}},
            {"id": "pr", "type": "debug.print", "params": {"text": "h", "chat": false}}
        ],
        "edges": [
            {"id": "w1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "tp", "pin": "exec"}},
            {"id": "w2", "from": {"node": "tp", "pin": "update"}, "to": {"node": "pr", "pin": "exec"}},
            {"id": "w3", "from": {"node": "tp", "pin": "h"}, "to": {"node": "pr", "pin": "value"}}
        ]
    }))
    .unwrap()
}

#[test]
fn every_curve_checks_clean_and_compiles_to_small_stocks() {
    let g = curves_graph();
    let p = graph::validate(&g);
    assert!(!graph::has_errors(&p), "{p:?}");
    let src = gen::generate(&g, false).source;
    // a step returns the key's value; in and out are x*x and x*(2-x); custom is a Hermite cubic
    assert!(src.contains("return 0.0\n"), "constant");
    assert!(src.contains("10.0 * x * x"), "ease in");
    assert!(src.contains("x * (2.0 - x)"), "ease out");
    assert!(src.contains("lsg_tl_ease("), "ease in-out");
    assert!(src.contains("(2.0 * x * x * x - 3.0 * x * x + 1.0)"), "custom");
    // the user tangent: 50 a second over a 1 s span; Auto at the last key: (5 - 40) / 2
    assert!(src.contains("50.0 * (x * x * x - 2.0 * x * x + x)"), "{src}");
    // (no big tables: a handful of lines per segment)
    assert!(src.lines().filter(|l| l.contains("lsg_tl")).count() < 200);
}

#[test]
fn bad_tangents_and_eases_are_refused() {
    let mut g = curves_graph();
    g.timelines[0].tracks[0].keys[4].tout = json!([1, 2, 3]);
    assert!(graph::has_errors(&graph::validate(&g)), "a number track takes one number");
    let mut g = curves_graph();
    g.timelines[0].tracks[0].keys[1].ease = Some("bouncy".into());
    assert!(graph::has_errors(&graph::validate(&g)));
}

#[test]
fn curves_compile_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls-studio-curves-{}", std::process::id()));
    write_includes(&work.join("include")).unwrap();
    for debug in [false, true] {
        let file = format!("studio_curves{}.sma", if debug { "_dbg" } else { "" });
        std::fs::write(work.join(&file), gen::generate(&curves_graph(), debug).source).unwrap();
        let (ok, out) = run_amxxpc(&install, &scripting, &work.join(&file), &work.join("include"), &work.join(file.replace(".sma", ".amxx"))).unwrap();
        assert!(ok, "{file}:\n{out}");
        assert!(parse_compiler(&out, &file).is_empty(), "{file} has warnings:\n{out}");
    }
    let _ = std::fs::remove_dir_all(&work);
}
