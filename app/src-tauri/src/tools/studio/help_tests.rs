//! The node help (nodehelp.json, made by node_help.py from the registry) stays in step with the
//! registry, and every example in it passes the quick check, so the help can't go stale.

use super::graph;
use serde_json::{json, Value};

const HELP: &str = include_str!("../../../../ui/src/views/tools/studio/nodehelp.json");

fn help() -> serde_json::Map<String, Value> {
    let v: Value = serde_json::from_str(HELP).expect("nodehelp.json is JSON");
    v["help"].as_object().expect("a help object").clone()
}

#[test]
fn every_node_has_help_for_each_pin_and_setting() {
    let h = help();
    let reg = graph::registry();
    let mut missing = vec![];
    for d in &reg.nodes {
        let Some(e) = h.get(&d.kind) else {
            missing.push(d.kind.clone());
            continue;
        };
        assert!(!e["what"].as_str().unwrap_or("").is_empty(), "{}: no text", d.kind);
        let pins: Vec<&str> = e["pins"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
        for p in d.inputs.iter().chain(&d.outputs) {
            assert!(pins.contains(&p.id.as_str()), "{}: pin {} has no help (run make_nodes.py)", d.kind, p.id);
        }
        let sets: Vec<&str> = e["settings"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
        for p in &d.params {
            assert!(sets.contains(&p.id.as_str()), "{}: setting {} has no help (run make_nodes.py)", d.kind, p.id);
        }
        for r in e["related"].as_array().unwrap() {
            assert!(reg.get(r.as_str().unwrap()).is_some(), "{}: related {} isn't a node", d.kind, r);
        }
    }
    assert!(missing.is_empty(), "nodes without help (run make_nodes.py, which writes nodehelp.json): {missing:?}");
    assert_eq!(h.len(), reg.nodes.len(), "nodehelp.json has nodes the registry doesn't (run make_nodes.py)");
}

#[test]
fn every_help_example_passes_the_quick_check() {
    let h = help();
    let mut with = 0;
    let mut bad = vec![];
    for (kind, e) in &h {
        let Some(ex) = e.get("example") else { continue };
        with += 1;
        let g = json!({
            "schema": "loner-strike/graph", "version": 3,
            "mode": {"id": "help_example", "name": "Help example"},
            "nodes": ex["nodes"], "edges": ex["edges"],
        });
        let g = graph::from_value(g).unwrap_or_else(|err| panic!("{kind}: {err}"));
        assert!(g.nodes.iter().any(|n| &n.kind == kind), "{kind}: its example doesn't use it");
        let errs: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level == "error").collect();
        if !errs.is_empty() {
            bad.push(format!("{kind}: {}", errs.iter().map(|p| p.message.as_str()).collect::<Vec<_>>().join("; ")));
        }
        assert!(!ex["caption"].as_str().unwrap_or("").is_empty(), "{kind}: an example with no caption");
    }
    assert!(bad.is_empty(), "examples with errors (node_help.py):\n{}", bad.join("\n"));
    // most nodes have one (the ones without need the mode's own variables, Functions and such)
    assert!(with * 10 >= h.len() * 8, "only {with} of {} nodes have an example", h.len());
}

/// A pin's help says what the value is in words: never just the type's id ("Entity: npc.",
/// "Kind: enum:ls_weapon.") and never the "Holdss" slip of the array template.
#[test]
fn pin_help_is_words_not_a_type_id() {
    let h = help();
    let reg = graph::registry();
    let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for d in &reg.nodes {
        for p in d.inputs.iter().chain(&d.outputs) {
            let mut t = p.ty.as_str();
            ids.insert(t.to_string());
            while let Some(inner) = t.strip_prefix("array<").and_then(|x| x.strip_suffix('>')) {
                t = inner;
                ids.insert(t.to_string());
            }
        }
    }
    let mut bad = vec![];
    for (kind, e) in &h {
        for p in e["pins"].as_array().unwrap() {
            let text = p["help"].as_str().unwrap_or("");
            // the words after "Label: ", up to the first sentence's end
            let words = text.split_once(": ").map(|(_, w)| w).unwrap_or(text);
            let words = words.split(". ").next().unwrap_or(words).trim_end_matches('.');
            // ("text" is its own word: "Name: text." reads fine)
            let raw = (words != "text" && ids.contains(words)) || ["enum:", "struct:", "array<", "map<", "$"].iter().any(|x| words.starts_with(x));
            if raw || text.contains("Holdss") {
                bad.push(format!("{kind}.{}: {text}", p["id"].as_str().unwrap_or("")));
            }
        }
    }
    assert!(bad.is_empty(), "pin help that's only a type id (add words in node_help.py TYPE_WORDS / ENUM_WORDS, then make_nodes.py):\n{}", bad.join("\n"));
}
