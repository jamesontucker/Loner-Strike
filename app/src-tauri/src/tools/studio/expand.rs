//! Macros are copied in where they're used before the plugin is made: each Macro node becomes
//! the nodes of the Macro's graph (new ids), wired where the Macro node was. A Macro used in a
//! Macro is copied in turn. The checks (graph.rs) have already refused Macros that use
//! themselves; the limits here are a second guard.

use super::graph::{Edge, Graph, PinRef};
use std::collections::HashMap;

/// The graph with every Macro copied in, and where each copied node came from (the Macro
/// node it stands in for, in the graph that was checked) so the debugger shows that node.
pub struct Expanded {
    pub graph: Graph,
    pub origin: HashMap<String, String>,
}

/// Copies stop here: a graph can't grow past this many nodes from Macros.
const MAX_EXPANDED_NODES: usize = 4000;

pub fn expand(g: &Graph) -> Expanded {
    let macro_ids: Vec<&str> = g.macros.iter().map(|m| m.id.as_str()).collect();
    let in_macro = |graph: &Option<String>| graph.as_deref().is_some_and(|h| macro_ids.contains(&h));
    let mut out = g.clone();
    let mut origin: HashMap<String, String> = HashMap::new();
    let mut k = 0usize;
    loop {
        // the next Macro node outside a Macro's own graph
        let Some(ui) = out.nodes.iter().position(|n| n.kind == "macro.use" && !in_macro(&n.graph)) else { break };
        let u = out.nodes.remove(ui);
        let home = u.graph.clone();
        let reports_as = origin.get(&u.id).cloned().unwrap_or_else(|| u.id.clone());
        let mid = u.params.get("macro").and_then(|v| v.as_str()).unwrap_or("").to_string();
        // the wires at the Macro node
        let (at_use, rest): (Vec<Edge>, Vec<Edge>) = out.edges.drain(..).partition(|e| e.from.node == u.id || e.to.node == u.id);
        out.edges = rest;
        if out.nodes.len() > MAX_EXPANDED_NODES || !macro_ids.contains(&mid.as_str()) {
            continue;
        }
        k += 1;
        let body: Vec<_> = g.nodes.iter().filter(|n| n.graph.as_deref() == Some(mid.as_str())).collect();
        let entry = body.iter().find(|n| n.kind == "macro.entry").map(|n| n.id.clone());
        let exit = body.iter().find(|n| n.kind == "macro.exit").map(|n| n.id.clone());
        let new_id = |id: &str| format!("x{k}_{id}");
        for n in &body {
            if Some(&n.id) == entry.as_ref() || Some(&n.id) == exit.as_ref() || n.kind == "macro.exit" || n.kind == "macro.entry" {
                continue;
            }
            let mut c = (*n).clone();
            c.id = new_id(&n.id);
            c.graph = home.clone();
            origin.insert(c.id.clone(), reports_as.clone());
            out.nodes.push(c);
        }
        // outside ends of the Macro node's pins: what comes into an input, where an output goes
        let sources = |pin: &str| -> Vec<PinRef> { at_use.iter().filter(|e| e.to.node == u.id && e.to.pin == pin).map(|e| e.from.clone()).collect() };
        let targets = |pin: &str| -> Vec<PinRef> { at_use.iter().filter(|e| e.from.node == u.id && e.from.pin == pin).map(|e| e.to.clone()).collect() };
        let mut j = 0usize;
        let mut add = |out: &mut Graph, from: PinRef, to: PinRef| {
            j += 1;
            out.edges.push(Edge { id: format!("x{k}e{j}"), from, to });
        };
        let ids: Vec<&str> = body.iter().map(|n| n.id.as_str()).collect();
        for e in g.edges.iter().filter(|e| ids.contains(&e.from.node.as_str()) && ids.contains(&e.to.node.as_str())) {
            let from_entry = Some(&e.from.node) == entry.as_ref();
            let to_exit = Some(&e.to.node) == exit.as_ref();
            let froms = if from_entry { sources(&e.from.pin) } else { vec![PinRef { node: new_id(&e.from.node), pin: e.from.pin.clone() }] };
            let tos = if to_exit { targets(&e.to.pin) } else { vec![PinRef { node: new_id(&e.to.node), pin: e.to.pin.clone() }] };
            for f in &froms {
                for t in &tos {
                    add(&mut out, f.clone(), t.clone());
                }
            }
        }
    }
    // the Macros' own graphs aren't part of the plugin
    out.nodes.retain(|n| !in_macro(&n.graph));
    let live: std::collections::HashSet<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
    let edges: Vec<Edge> = out.edges.iter().filter(|e| live.contains(e.from.node.as_str()) && live.contains(e.to.node.as_str())).cloned().collect();
    out.edges = edges;
    out.comments.retain(|c| !in_macro(&c.graph));
    out.macros.clear();
    Expanded { graph: out, origin }
}
