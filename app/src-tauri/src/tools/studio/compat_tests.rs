//! The 1.0 compatibility suite (docs/notes/StudioCompat.md). testdata/compat_v3 holds frozen copies
//! of what shipped with graph version 3: the templates and example modes, the library premades, the
//! snippets, the test graphs and hand-made graphs with the wires later versions must keep working.
//! These files are never edited or regenerated: each must keep loading (migrated forward), checking
//! with no errors it didn't have, and generating a plugin that compiles. Its frozen plugin
//! (compat_v3/out) says what it made in 1.0; a codegen change that alters one is a deliberate
//! decision, refreshed with UPDATE_COMPAT=1 and reviewed (the graph files themselves stay).

use super::{gen, graph, graph::Graph, parse_compiler, run_amxxpc, write_includes};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/studio/testdata/compat_v3")
}

/// Every fixture: (name, kind, file text).
fn fixtures() -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = std::fs::read_dir(dir())
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().strip_suffix(".json")?.to_string();
            let kind = name.split('-').next()?.to_string();
            Some((name, kind, std::fs::read_to_string(e.path()).ok()?))
        })
        .collect();
    out.sort();
    out
}

/// A fixture as the graph the app would open: a mode as it is; a library item or a snippet put
/// into an empty mode (as dropping it in does).
pub fn as_graph(name: &str, kind: &str, text: &str) -> Result<Graph, String> {
    match kind {
        "template" | "graph" => graph::parse(text),
        "library" | "snippet" => {
            let mut v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            let o = v.as_object_mut().ok_or("not an object")?;
            for k in ["name", "description", "category", "kind", "root"] {
                o.remove(k);
            }
            let version = if kind == "snippet" { 2 } else { 3 };
            o.insert("schema".into(), json!(graph::SCHEMA));
            o.insert("version".into(), json!(version));
            let id: String = format!("c_{}", name.split_once('-').map(|x| x.1).unwrap_or(name)).chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(24).collect();
            o.insert("mode".into(), json!({"id": id, "name": "Compat"}));
            for k in ["nodes", "edges"] {
                o.entry(k).or_insert(json!([]));
            }
            graph::from_value(v)
        }
        _ => Err(format!("unknown fixture kind {kind}")),
    }
}

fn errors(g: &Graph) -> Vec<String> {
    let mut e: Vec<String> = graph::validate(g).into_iter().filter(|p| p.level == "error").map(|p| format!("{} {}", p.node.unwrap_or_default(), p.message)).collect();
    e.sort();
    e.dedup();
    e
}

#[test]
fn every_1_0_graph_still_loads_checks_and_generates_what_it_did() {
    let update = std::env::var("UPDATE_COMPAT").is_ok();
    let out = dir().join("out");
    std::fs::create_dir_all(&out).unwrap();
    let all = fixtures();
    assert!(all.len() >= 50, "{} fixtures", all.len());
    for kind in ["template", "library", "snippet", "graph"] {
        assert!(all.iter().any(|f| f.1 == kind), "no {kind} fixtures");
    }
    let mut changed = Vec::new();
    for (name, kind, text) in &all {
        let g = as_graph(name, kind, text).unwrap_or_else(|e| panic!("{name} doesn't load any more: {e}"));
        assert_eq!(g.version, graph::VERSION, "{name}: read forward to the current version");
        // no errors it didn't have in 1.0
        let now = errors(&g);
        let ef = out.join(format!("{name}.errors"));
        if update || !ef.exists() {
            std::fs::write(&ef, now.join("\n")).unwrap();
        } else {
            let then: Vec<String> = std::fs::read_to_string(&ef).unwrap().lines().map(str::to_string).collect();
            let new: Vec<&String> = now.iter().filter(|e| !then.contains(e)).collect();
            assert!(new.is_empty(), "{name} has errors it didn't have in 1.0: {new:#?}");
        }
        // the same plugin
        for debug in [false, true] {
            let src = gen::generate(&g, debug).source;
            assert!(src.contains("plugin_init"), "{name}: no plugin");
            if debug {
                continue;
            }
            let sf = out.join(format!("{name}.sma"));
            if update || !sf.exists() {
                std::fs::write(&sf, &src).unwrap();
            } else if std::fs::read_to_string(&sf).unwrap() != src {
                changed.push(name.clone());
            }
        }
    }
    assert!(changed.is_empty(), "these 1.0 graphs make a different plugin now (if that's intended, review the diff and refresh with UPDATE_COMPAT=1): {changed:?}");
}

/// A graph saved by this version opens again unchanged (nodes, wires, variables), and the 1.0 files
/// read as the current version with nothing but the version changed.
#[test]
fn a_1_0_graph_read_and_saved_keeps_everything() {
    for (name, kind, text) in fixtures().iter().filter(|f| f.1 == "template" || f.1 == "graph") {
        let g = as_graph(name, kind, text).unwrap();
        let saved = serde_json::to_string(&g).unwrap();
        let again = graph::parse(&saved).unwrap_or_else(|e| panic!("{name}: saved, it doesn't open: {e}"));
        assert_eq!(serde_json::to_value(&again).unwrap(), serde_json::to_value(&g).unwrap(), "{name}");
        assert_eq!(g.nodes.len(), serde_json::from_str::<Value>(text).unwrap()["nodes"].as_array().map_or(0, |a| a.len()), "{name}: nodes kept");
    }
}

#[test]
fn every_1_0_graph_compiles_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls-studio-compat-{}", std::process::id()));
    let inc = work.join("include");
    write_includes(&inc).unwrap();
    for (name, kind, text) in fixtures() {
        let g = as_graph(&name, &kind, &text).unwrap();
        let file = format!("studio_compat_{}.sma", name.replace('-', "_"));
        let src = work.join(&file);
        std::fs::write(&src, gen::generate(&g, false).source).unwrap();
        let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &work.join(file.replace(".sma", ".amxx"))).unwrap();
        assert!(ok, "{file} didn't compile:\n{output}");
        let _ = parse_compiler(&output, &file);
    }
    let _ = std::fs::remove_dir_all(&work);
}

/// Orphans (StudioCompat.md): a wire to a pin the node doesn't have any more, and a value of a
/// setting it doesn't have, are kept, named by an error (so Compile, Test and Publish stop) and
/// saved again as they were; nothing is dropped.
#[test]
fn orphaned_pins_and_values_are_kept_and_named() {
    let text = json!({"schema": graph::SCHEMA, "version": 3, "mode": {"id": "orphans", "name": "Orphans"},
        "nodes": [{"id": "rs", "type": "event.round_started"}, {"id": "p", "type": "debug.print", "params": {"text": "hi", "chat": false, "gone_setting": 5}},
                  {"id": "n", "type": "math.number", "params": {"value": 2}}],
        "edges": [{"id": "w1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                  {"id": "w2", "from": {"node": "n", "pin": "out"}, "to": {"node": "p", "pin": "removed_pin"}},
                  {"id": "w3", "from": {"node": "rs", "pin": "old_out"}, "to": {"node": "p", "pin": "value"}}]}).to_string();
    let g = graph::parse(&text).unwrap();
    assert_eq!(g.edges.len(), 3, "no wire dropped");
    let p = graph::validate(&g);
    let orphan = |pin: &str, edge: Option<&str>| p.iter().any(|x| x.level == "error" && x.pin.as_deref() == Some(pin) && x.edge.as_deref() == edge && x.message.contains("orphaned"));
    assert!(orphan("removed_pin", Some("w2")), "{p:#?}");
    assert!(orphan("old_out", Some("w3")), "{p:#?}");
    assert!(p.iter().any(|x| x.level == "error" && x.pin.as_deref() == Some("gone_setting") && x.message.contains(graph::ORPHAN_VALUE)), "{p:#?}");
    assert!(graph::has_errors(&p), "Compile, Test and Publish stop on errors");
    // saved and read again: all still there
    let again = graph::parse(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(again.edges.len(), 3);
    assert_eq!(again.nodes.iter().find(|n| n.id == "p").unwrap().params.get("gone_setting"), Some(&json!(5)));
    // deleting the wire ends the orphan
    let mut fixed = again.clone();
    fixed.edges.retain(|e| e.id != "w2");
    assert!(!graph::validate(&fixed).iter().any(|x| x.pin.as_deref() == Some("removed_pin")));
}

/// Renamed ids read forward through the registry's alias maps (none are needed yet: the map is
/// there for the first rename), and graph versions 1-3 read as the current one.
#[test]
fn old_versions_read_forward() {
    for v in 1..=graph::VERSION {
        let g = graph::parse(&json!({"schema": graph::SCHEMA, "version": v, "mode": {"id": "old", "name": "Old"}, "nodes": [], "edges": []}).to_string()).unwrap();
        assert_eq!(g.version, graph::VERSION);
    }
    let newer = graph::parse(&json!({"schema": graph::SCHEMA, "version": graph::VERSION + 1, "mode": {"id": "new", "name": "New"}, "nodes": [], "edges": []}).to_string());
    assert!(newer.unwrap_err().contains("newer Loner-Strike"));
    // every alias points at something that exists
    let r = graph::registry();
    for (old, new) in &r.aliases {
        assert!(r.get(new).is_some() && r.get(old).is_none(), "{old} -> {new}");
    }
    for d in &r.nodes {
        for (old, new) in &d.aliases {
            assert!(d.inputs.iter().chain(&d.outputs).any(|p| &p.id == new) || d.param(new).is_some(), "{}: {old} -> {new}", d.kind);
        }
    }
}
