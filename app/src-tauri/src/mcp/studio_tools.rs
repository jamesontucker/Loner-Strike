//! The Game Mode Studio for agents, node by node: an agent opens a graph as a draft (the
//! document it edits, kept in the app's memory like the editor page keeps its graph), adds
//! and removes nodes and wires, sets pin values, makes variables, Functions, Macros, structs,
//! enums, comments and map markers, drops in snippets, undoes and redoes, and reads back every
//! node with its pins resolved. The draft is checked with the Studio's own rules after each
//! batch (`/api/tools/studio/validate`); saving it over the graph file asks first, and
//! compiling, playtesting and the debugger take the draft too (more.rs, `draft: true`).
//!
//! Also here: the Studio's kits, snippets and library, the Creature Studio's waves, Callouts
//! and Publish's checks.

use super::catalog::{ask, err, get, map_arg, obj, post, s, Done, Fail, Kind, Spec};
use super::Ctx;
use crate::tools::studio::graph::{self, View};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// An agent's open graph: the graph, its undo and redo, and whether it's saved.
struct Draft {
    graph: Value,
    past: Vec<Value>,
    future: Vec<Value>,
    dirty: bool,
}

const UNDO_LIMIT: usize = 200;
const MAX_DRAFTS: usize = 16;

fn drafts() -> &'static Mutex<HashMap<String, Draft>> {
    static D: OnceLock<Mutex<HashMap<String, Draft>>> = OnceLock::new();
    D.get_or_init(Default::default)
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<String, Draft>> {
    drafts().lock().unwrap_or_else(|e| e.into_inner())
}

/// The draft's graph (for studio_validate, studio_code, studio_compile and studio_playtest).
pub fn draft_graph(id: &str) -> Option<Value> {
    lock().get(id).map(|d| d.graph.clone())
}

fn mode_id(a: &Value) -> Result<String, Fail> {
    let id = s(a, "id").ok_or_else(|| err("Say the graph's id (studio_graphs lists them)."))?;
    if !graph::valid_mode_id(id) {
        return Err(err("A graph id is 2 to 24 small letters, digits or _."));
    }
    Ok(id.to_string())
}

const NO_DRAFT: &str = "That graph isn't open as a draft: open it with studio_draft_open first.";

// ---- schemas -----------------------------------------------------------------------------

fn id_prop() -> Value {
    json!({"type": "string", "description": "The graph's id (its mode id), e.g. rifle_reward"})
}

pub fn specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        Spec { name: "studio_draft_open", title: "Mode Studio: open a draft", kind: Change,
            schema: || obj(json!({"id": id_prop(),
                "from": {"type": "string", "enum": ["saved", "new", "template"], "description": "saved: the graph file (default); new: an empty graph; template: a starter"},
                "template": {"type": "string", "enum": ["team-dm", "last-stand", "battle-royale"]},
                "name": {"type": "string", "description": "The mode's name (new and template)"},
                "discard": {"type": "boolean", "description": "Throw away a draft of this id with unsaved edits"}}), &["id"]),
            description: "Opens a game mode graph as a draft to edit node by node (nothing is written until studio_draft_save). Returns the draft like studio_draft." },
        Spec { name: "studio_draft", title: "Mode Studio: read the draft", kind: Read,
            schema: || obj(json!({"id": id_prop(), "graph": {"type": "string", "description": "Only this tab: a Function's or Macro's id ('' the Event Graph)"}}), &["id"]),
            description: "The draft: mode, every node (id, type, title, tab, position, settings, breakpoint) with its pins resolved (id, type, label, wired or its value, where its wires go), wires, variables, Functions, Macros, structs, enums, comments, markers, unsaved, undo/redo, and the Studio's problems with the node and pin each is on." },
        Spec { name: "studio_edit", title: "Mode Studio: edit the draft", kind: Change,
            schema: || obj(json!({"id": id_prop(), "ops": {"type": "array", "items": {"type": "object"}, "description": "Edits in order, all or nothing (one undo step). Each is {op, ...}:\n\
add_node {type, x?, y?, params?, graph?} (graph: a Function/Macro id) -> node id;\n\
remove {ids} (nodes, wires, comments);\n\
connect {from: {node, pin}, to: {node, pin}} (an output to an input; a flow output or a data input keeps one wire; a Player or an entity handle such as Creature or NPC goes into Entity pins, the other way needs an As Creature / As NPC... cast) -> wire id;\n\
disconnect {wire} or {from, to};\n\
set_value {node, pin or param, value} (an unwired input's value, or a node setting);\n\
breakpoint {node, on};\n\
move {node, x, y};\n\
add_variable {name, type, scope?: mode|player|team, default?, reset?: never|round} -> id; update_variable {var, ...}; remove_variable {var};\n\
add_function {name, pure?, inputs?: [{name, type}], outputs?} -> id (its Entry and Return nodes are made); add_macro {name, inputs?, outputs?} -> id; update_part {part, name?, inputs?, outputs?, pure?}; remove_part {part};\n\
add_struct {name, fields: [{name, type}]} -> id; add_enum {name, values} -> id; update_type {type_id, name?, fields?, values?}; remove_type {type_id};\n\
add_comment {text, x, y, w?, h?, graph?} -> id;\n\
add_marker {id?, label, type: point|area|path, min?, max?, keys?} -> id; update_marker {marker, ...}; remove_marker {marker};\n\
set_mode {name?, description?, settings?: [{cvar, value}]};\n\
insert_snippet {snippet, x?, y?, graph?} (a snippet id from studio_snippets) -> node ids;\n\
add_event_graph {name} -> id (another event graph tab: put nodes on it with graph: its id); rename_event_graph {event_graph, name}; remove_event_graph {event_graph} (its nodes move to the first);\n\
add_custom_event {name, inputs?: [{name, type}], map?} -> id (Custom Event / Call Custom Event nodes take it as their event setting); update_custom_event {custom_event, ...}; remove_custom_event {custom_event};\n\
add_timeline {name, length?, loop?, autoplay?, tick?, tracks?} -> id (Timeline nodes take it as their timeline setting); update_timeline {timeline, ...}; remove_timeline {timeline}.\n\
add_table {name, columns: [{id, name, type: number|text|bool|weapon|entname|enum:<id>}], key: <column id>, rows?: [[cell, ...], ...]} -> id (Get Row, Get Cell, For Each Row, Find Rows, Row Count, Row Keys take it as their table setting; Get Cell and Find Rows a column id too; at most 16 tables, 12 columns, 256 rows, text cells 48 letters); update_table {table, name?, columns?, key?, rows?}; remove_table {table}.\n\
Pin types and node types: studio_nodes."}}), &["id", "ops"]),
            description: "Edits the draft like the Studio's canvas: nodes, wires, pin values, variables, Functions, Macros, structs, enums, comments, map markers, mode settings and snippets. Returns the new ids and the problems left." },
        Spec { name: "studio_draft_undo", title: "Mode Studio: undo", kind: Change, schema: || obj(json!({"id": id_prop()}), &["id"]),
            description: "Undoes the last studio_edit batch on the draft." },
        Spec { name: "studio_draft_redo", title: "Mode Studio: redo", kind: Change, schema: || obj(json!({"id": id_prop()}), &["id"]),
            description: "Redoes an undone studio_edit batch." },
        Spec { name: "studio_draft_save", title: "Mode Studio: save the draft", kind: Risky, schema: || obj(json!({"id": id_prop()}), &["id"]),
            description: "Saves the draft as the graph file (asks the person first when it replaces one)." },
        Spec { name: "studio_check", title: "Mode Studio: Compile (check only)", kind: Change,
            schema: || obj(json!({"id": id_prop(), "draft": {"type": "boolean", "description": "Check the agent's open draft instead of the saved graph"}}), &["id"]),
            description: "The Studio's Compile button: the full check with the game's real compiler (amxxpc), nothing installed. Answers ok and the problems, each tied to its node and pin when it can be." },
        Spec { name: "studio_test_run_status", title: "Mode Studio: test run status", kind: Read, schema: none,
            description: "The Test run going on or the last one: phase, steps so far and, when done, the result (pass/warn/fail, run-time errors tied to nodes, crash, cost against the baseline, nodes that never ran)." },
        Spec { name: "studio_test_run_cancel", title: "Mode Studio: stop the test run", kind: Change, schema: none,
            description: "Stops the running Test run (its hidden server is closed)." },
        Spec { name: "studio_node_help", title: "Mode Studio: node help", kind: Read,
            schema: || obj(json!({"type": {"type": "string", "description": "A node type, e.g. event.player_killed"}, "search": {"type": "string", "description": "Words in a node's title or help"}}), &[]),
            description: "The Studio's node help (F1 on a node): what a node does, each pin and setting, more advice and related nodes." },
        Spec { name: "studio_requirements", title: "Mode Studio: requirements", kind: Read, schema: || obj(json!({"id": id_prop()}), &["id"]),
            description: "What a saved mode plays on and needs (My mode › Details › Requirements): the default and recommended maps with where each comes from (base game, a community item, or only on this computer: share it first), every model, sound and sprite its nodes use that isn't the game's own with whether it's here, its link to a community or GameBanana item and a suggested one, the components, creature types and bot brain it needs, warnings, and the files that can't be found anywhere (Publish stops on those)." },
        Spec { name: "studio_requirements_set", title: "Mode Studio: set requirements", kind: Change,
            schema: || obj(json!({"id": id_prop(), "defaultMap": {"type": "string"}, "maps": {"type": "array", "items": {"type": "string"}, "description": "Recommended maps (installed map names)"},
                "links": {"type": "object", "additionalProperties": {"type": ["object", "null"]}, "description": "content path -> {model: Mod|Spray, id: GameBanana page number, name} (null removes the link)"}}), &["id"]),
            description: "Sets a saved mode's default map, recommended maps and the items that provide its content. Fields left out keep what's there. Returns the same as studio_requirements." },
        Spec { name: "studio_snippets", title: "Mode Studio: snippets", kind: Read, schema: none,
            description: "The person's snippets (pieces of graph to drop in): id, name, description, the nodes they hold. Insert one with studio_edit insert_snippet." },
        Spec { name: "studio_library", title: "Mode Studio: My library", kind: Read, schema: none,
            description: "My library: Functions, Macros, structs and enums the person keeps to reuse across modes." },
        Spec { name: "studio_kits", title: "Mode Studio: map kits", kind: Read,
            schema: || obj(json!({"id": {"type": "string", "description": "One mode's kit (leave out for the list)"}}), &[]),
            description: "Mode map kits: what a map needs for each mode (markers, components, creatures, advice). A graph's own markers are in studio_draft." },
        Spec { name: "creature_waves", title: "Creatures: waves", kind: Read, schema: none,
            description: "The Creature Studio's waves per mode: which creature types come in which round, how many, tiers and enrage." },
        Spec { name: "creature_waves_save", title: "Creatures: save waves", kind: Change,
            schema: || obj(json!({"waves": {"type": "object", "description": "One mode's wave set as creature_waves shows it ({mode, ...})"}}), &["waves"]),
            description: "Saves one mode's waves into the game (checked first; the old file is kept in History)." },
        Spec { name: "callouts_read", title: "Callouts: read", kind: Read, schema: map_only,
            description: "A map's callouts: the place names on its nav areas (bots say them), with area ids, and whether the map is in use." },
        Spec { name: "callouts_suggest", title: "Callouts: suggest", kind: Read, schema: map_only,
            description: "Suggested place names for a map's nav areas (from its entities and shape)." },
        Spec { name: "callouts_save", title: "Callouts: save", kind: Risky,
            schema: || obj(json!({"map": super::catalog::map_prop(), "changes": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "integer"}, "place": {"type": "string"}}}, "description": "[{id: area id, place: name ('' clears)}]"}}), &["map", "changes"]),
            description: "Writes place names into the map's nav (asks the person first; the nav is kept in History)." },
        Spec { name: "publish_check", title: "Publish: checks", kind: Read,
            schema: || obj(json!({"kind": {"type": "string", "enum": ["map", "mode", "prefab"]}, "id": {"type": "string", "description": "The map name, mode id or prefab id"}}), &["kind", "id"]),
            description: "What Publish would say about a map, game mode or prefab before packaging it: checks with reasons, credits, warnings and blockers. Building the package goes through app_call POST /api/tools/community/publish/... and asks the person." },
    ]
}

fn none() -> Value {
    super::catalog::none()
}
fn map_only() -> Value {
    super::catalog::map_only()
}

// ---- edits -------------------------------------------------------------------------------

fn arr<'a>(g: &'a mut Value, k: &str) -> &'a mut Vec<Value> {
    if !g[k].is_array() {
        g[k] = json!([]);
    }
    g[k].as_array_mut().unwrap()
}

/// A fresh id for a node, wire or comment: prefix + the lowest free number (graphOps freshId).
fn fresh_id(g: &Value, prefix: &str) -> String {
    let used: Vec<&str> = ["nodes", "edges", "comments"].iter().flat_map(|k| g[*k].as_array().into_iter().flatten().filter_map(|x| x["id"].as_str())).collect();
    (1..).map(|i| format!("{prefix}{i}")).find(|id| !used.contains(&id.as_str())).unwrap()
}

/// A fresh id for a variable, local, Function, Macro, struct or enum (graphOps freshPartId).
fn fresh_part(g: &Value, prefix: &str) -> String {
    let mut used: Vec<String> = vec![];
    for k in ["variables", "functions", "macros", "structs", "enums", "eventGraphs", "customEvents", "timelines", "dataTables"] {
        for x in g[k].as_array().into_iter().flatten() {
            used.extend(x["id"].as_str().map(String::from));
            for l in x["locals"].as_array().into_iter().flatten() {
                used.extend(l["id"].as_str().map(String::from));
            }
        }
    }
    (1..).map(|i| format!("{prefix}{i}")).find(|id| !used.contains(id)).unwrap()
}

/// Members ({name, type}) with ids (prefix + n) where they have none.
fn members(v: Option<&Value>, prefix: &str) -> Value {
    let mut out: Vec<Value> = vec![];
    for (i, m) in v.and_then(Value::as_array).into_iter().flatten().enumerate() {
        let mut m = m.clone();
        if m["id"].as_str().is_none() {
            m["id"] = json!(format!("{prefix}{}", i + 1));
        }
        out.push(m);
    }
    json!(out)
}

fn text(o: &Value, k: &str) -> Result<String, String> {
    o[k].as_str().map(String::from).ok_or_else(|| format!("{k} is missing"))
}

fn pin_ref(v: &Value) -> Result<(String, String), String> {
    if let Some(s) = v.as_str() {
        let (n, p) = s.split_once('.').ok_or("a pin is {node, pin} or \"node.pin\"")?;
        return Ok((n.to_string(), p.to_string()));
    }
    Ok((text(v, "node")?, text(v, "pin")?))
}

fn add_node(g: &mut Value, kind: &str, x: f64, y: f64, params: Option<&Value>, home: Option<&str>) -> Result<String, String> {
    let def = graph::registry().get(kind).ok_or_else(|| format!("there's no node type {kind} (studio_nodes lists them)"))?;
    let id = fresh_id(g, "n");
    let mut p = Map::new();
    for d in &def.params {
        p.insert(d.id.clone(), d.default.clone());
    }
    for (k, v) in params.and_then(Value::as_object).into_iter().flatten() {
        p.insert(k.clone(), v.clone());
    }
    let mut n = json!({"id": id, "type": kind, "pos": {"x": x.round(), "y": y.round()}, "params": p});
    if let Some(h) = home.filter(|h| !h.is_empty()) {
        n["graph"] = json!(h);
    }
    arr(g, "nodes").push(n);
    // (a node with a comfort limit brings its Limits setting once, as the Studio's Add node does:
    // limits.rs, graphOps.ts withLimitVars)
    for l in crate::tools::studio::limits::LIMITS.iter().filter(|l| l.kinds.contains(&kind)) {
        let want = crate::tools::studio::limits::var_id(l.id);
        if !g["variables"].as_array().is_some_and(|vs| vs.iter().any(|v| v["id"] == want.as_str())) {
            arr(g, "variables").push(crate::tools::studio::limits::var_json(l));
        }
    }
    Ok(id)
}

fn edge(g: &mut Value, from: (&str, &str), to: (&str, &str)) -> String {
    let id = fresh_id(g, "w");
    arr(g, "edges").push(json!({"id": id, "from": {"node": from.0, "pin": from.1}, "to": {"node": to.0, "pin": to.1}}));
    id
}

fn parsed(g: &Value) -> Result<graph::Graph, String> {
    graph::from_value(g.clone())
}

/// Wires an output to an input, by the Studio's rules: pins exist and their types fit, both on
/// one tab; a flow output and a data input keep one wire (the old one goes).
fn connect(g: &mut Value, from: (String, String), to: (String, String)) -> Result<String, String> {
    let pg = parsed(g)?;
    let v = View::new(&pg);
    let out = v.output(&from.0, &from.1).ok_or_else(|| format!("{}.{} isn't an output pin (studio_draft lists each node's pins)", from.0, from.1))?;
    let inp = v.input(&to.0, &to.1).ok_or_else(|| format!("{}.{} isn't an input pin", to.0, to.1))?;
    if v.graph_of(&from.0) != v.graph_of(&to.0) {
        return Err("wires stay inside one graph tab".into());
    }
    if from.0 == to.0 {
        return Err("a node can't be wired to itself".into());
    }
    let (ot, it) = (v.out_type(&from.0, &from.1).unwrap_or_else(|| out.ty.clone()), v.in_type(&to.0, &to.1).unwrap_or_else(|| inp.ty.clone()));
    if (ot == "exec") != (it == "exec") || !graph::compatible(&ot, &it) {
        return Err(format!("{}.{} ({ot}) can't go into {}.{} ({it})", from.0, from.1, to.0, to.1));
    }
    let exec = ot == "exec";
    arr(g, "edges").retain(|e| {
        let same_out = exec && e["from"]["node"] == from.0.as_str() && e["from"]["pin"] == from.1.as_str();
        let same_in = !exec && e["to"]["node"] == to.0.as_str() && e["to"]["pin"] == to.1.as_str();
        !(same_out || same_in)
    });
    Ok(edge(g, (&from.0, &from.1), (&to.0, &to.1)))
}

/// A node's setting or an unwired input's value (the input's param).
fn set_value(g: &mut Value, node: &str, key: &str, value: Value) -> Result<(), String> {
    let pg = parsed(g)?;
    let v = View::new(&pg);
    let def = v.defs.get(node).copied().ok_or_else(|| format!("there's no node {node}"))?;
    let param = match v.input(node, key).and_then(|p| p.param.clone()) {
        Some(p) => p,
        None if def.param(key).is_some() => key.to_string(),
        None => return Err(format!("{node} has no setting or valued input called {key}")),
    };
    let n = arr(g, "nodes").iter_mut().find(|n| n["id"] == node).ok_or("no such node")?;
    n["params"][param] = value;
    Ok(())
}

/// A snippet (or any graph piece) dropped in with fresh ids (graphOps insertPart): variables
/// join the graph's when name, type and scope match.
fn insert_part(g: &mut Value, part: &Value, x: f64, y: f64, home: Option<&str>) -> Result<Vec<String>, String> {
    let skip = ["fn.entry", "macro.entry", "macro.exit"];
    let nodes: Vec<Value> = part["nodes"].as_array().into_iter().flatten().filter(|n| n["type"].as_str().is_some_and(|t| graph::registry().get(t).is_some() && !skip.contains(&t))).cloned().collect();
    if nodes.is_empty() {
        return Err("that snippet has no nodes".into());
    }
    let xs = nodes.iter().filter_map(|n| n["pos"]["x"].as_f64()).fold(f64::INFINITY, f64::min);
    let ys = nodes.iter().filter_map(|n| n["pos"]["y"].as_f64()).fold(f64::INFINITY, f64::min);
    let (dx, dy) = (x - if xs.is_finite() { xs } else { 0.0 }, y - if ys.is_finite() { ys } else { 0.0 });
    let mut var_map: HashMap<String, String> = HashMap::new();
    for v in part["variables"].as_array().into_iter().flatten() {
        let same = g["variables"].as_array().into_iter().flatten().find(|w| w["name"] == v["name"] && w["type"] == v["type"] && w["scope"] == v["scope"]).and_then(|w| w["id"].as_str().map(String::from));
        let id = match same {
            Some(id) => id,
            None => {
                let id = fresh_part(g, "v");
                let mut nv = v.clone();
                nv["id"] = json!(id);
                arr(g, "variables").push(nv);
                id
            }
        };
        if let Some(old) = v["id"].as_str() {
            var_map.insert(old.into(), id);
        }
    }
    let mut remap: HashMap<String, String> = HashMap::new();
    let mut made = vec![];
    for n in &nodes {
        let id = fresh_id(g, "n");
        remap.insert(n["id"].as_str().unwrap_or("").into(), id.clone());
        let mut nn = n.clone();
        nn["id"] = json!(id);
        nn["pos"] = json!({"x": (n["pos"]["x"].as_f64().unwrap_or(0.0) + dx).round(), "y": (n["pos"]["y"].as_f64().unwrap_or(0.0) + dy).round()});
        if let Some(o) = nn.as_object_mut() {
            o.remove("breakpoint");
            o.remove("graph");
        }
        if let Some(h) = home.filter(|h| !h.is_empty()) {
            nn["graph"] = json!(h);
        }
        arr(g, "nodes").push(nn);
        made.push(id);
    }
    for n in arr(g, "nodes").iter_mut().filter(|n| n["id"].as_str().is_some_and(|i| made.iter().any(|m| m == i))) {
        if let Some(v) = n["params"]["var"].as_str().and_then(|v| var_map.get(v)).cloned() {
            n["params"]["var"] = json!(v);
        }
        for k in ["func", "timer"] {
            if let Some(t) = n["params"][k].as_str().and_then(|t| remap.get(t)).cloned() {
                n["params"][k] = json!(t);
            }
        }
    }
    for e in part["edges"].as_array().into_iter().flatten() {
        let (Some(a), Some(b)) = (e["from"]["node"].as_str().and_then(|n| remap.get(n)).cloned(), e["to"]["node"].as_str().and_then(|n| remap.get(n)).cloned()) else { continue };
        let (pa, pb) = (e["from"]["pin"].as_str().unwrap_or("").to_string(), e["to"]["pin"].as_str().unwrap_or("").to_string());
        edge(g, (&a, &pa), (&b, &pb));
    }
    Ok(made)
}

/// One edit; what it made (an id, ids, or nothing).
async fn edit(ctx: &Ctx, g: &mut Value, o: &Value) -> Result<Value, String> {
    let f = |k: &str| o[k].as_f64().filter(|x| x.is_finite());
    let home = o["graph"].as_str();
    Ok(match o["op"].as_str().unwrap_or("") {
        "add_node" => json!(add_node(g, &text(o, "type")?, f("x").unwrap_or(0.0), f("y").unwrap_or(0.0), o.get("params"), home)?),
        "remove" => {
            let ids: Vec<&str> = o["ids"].as_array().ok_or("ids is a list")?.iter().filter_map(Value::as_str).collect();
            arr(g, "nodes").retain(|n| !n["id"].as_str().is_some_and(|i| ids.contains(&i)));
            arr(g, "comments").retain(|n| !n["id"].as_str().is_some_and(|i| ids.contains(&i)));
            arr(g, "edges").retain(|e| ![&e["id"], &e["from"]["node"], &e["to"]["node"]].iter().any(|x| x.as_str().is_some_and(|i| ids.contains(&i))));
            Value::Null
        }
        "connect" => json!(connect(g, pin_ref(&o["from"])?, pin_ref(&o["to"])?)?),
        "disconnect" => {
            let before = g["edges"].as_array().map(Vec::len).unwrap_or(0);
            if let Some(w) = o["wire"].as_str() {
                arr(g, "edges").retain(|e| e["id"] != w);
            } else {
                let (a, b) = (pin_ref(&o["from"])?, pin_ref(&o["to"])?);
                arr(g, "edges").retain(|e| !(e["from"]["node"] == a.0.as_str() && e["from"]["pin"] == a.1.as_str() && e["to"]["node"] == b.0.as_str() && e["to"]["pin"] == b.1.as_str()));
            }
            if g["edges"].as_array().map(Vec::len).unwrap_or(0) == before {
                return Err("there's no such wire".into());
            }
            Value::Null
        }
        "set_value" => {
            let key = o["pin"].as_str().or(o["param"].as_str()).ok_or("say pin or param")?;
            set_value(g, &text(o, "node")?, key, o.get("value").cloned().unwrap_or(Value::Null))?;
            Value::Null
        }
        "breakpoint" => {
            let node = text(o, "node")?;
            let on = o["on"].as_bool().unwrap_or(true);
            let n = arr(g, "nodes").iter_mut().find(|n| n["id"] == node.as_str()).ok_or("there's no such node")?;
            if on {
                n["breakpoint"] = json!(true);
            } else if let Some(m) = n.as_object_mut() {
                m.remove("breakpoint");
            }
            Value::Null
        }
        "move" => {
            let node = text(o, "node")?;
            let n = arr(g, "nodes").iter_mut().find(|n| n["id"] == node.as_str()).ok_or("there's no such node")?;
            n["pos"] = json!({"x": f("x").unwrap_or(0.0).round(), "y": f("y").unwrap_or(0.0).round()});
            Value::Null
        }
        "add_variable" => {
            let id = fresh_part(g, "v");
            let ty = o["type"].as_str().unwrap_or("number").to_string();
            let mut v = json!({"id": id, "name": text(o, "name")?, "type": ty, "scope": o["scope"].as_str().unwrap_or("mode")});
            let start = match ty.as_str() {
                "number" => json!(0),
                "bool" => json!(false),
                "text" => json!(""),
                t if t.starts_with("enum:") => json!(0),
                _ => Value::Null,
            };
            v["default"] = o.get("default").cloned().unwrap_or(start);
            if v["default"].is_null() {
                v.as_object_mut().unwrap().remove("default");
            }
            if let Some(r) = o["reset"].as_str() {
                v["reset"] = json!(r);
            }
            arr(g, "variables").push(v);
            json!(id)
        }
        "update_variable" => {
            let id = text(o, "var")?;
            let x = arr(g, "variables").iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such variable")?;
            for k in ["name", "type", "scope", "default", "reset"] {
                if let Some(v) = o.get(k) {
                    x[k] = v.clone();
                }
            }
            Value::Null
        }
        "remove_variable" => {
            let id = text(o, "var")?;
            arr(g, "variables").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "add_function" => {
            let id = fresh_part(g, "f");
            let pure = o["pure"].as_bool().unwrap_or(false);
            let mut fun = json!({"id": id, "name": o["name"].as_str().unwrap_or(if pure { "Pure Function" } else { "New Function" }), "inputs": members(o.get("inputs"), "a"), "outputs": members(o.get("outputs"), "r")});
            if pure {
                fun["pure"] = json!(true);
            }
            arr(g, "functions").push(fun);
            let e = add_node(g, "fn.entry", 0.0, 0.0, None, Some(&id))?;
            let r = add_node(g, "fn.return", 420.0, 0.0, None, Some(&id))?;
            if !pure {
                edge(g, (&e, "then"), (&r, "exec"));
            }
            json!({"id": id, "entry": e, "return": r})
        }
        "add_macro" => {
            let id = fresh_part(g, "m");
            let ins = if o.get("inputs").is_some() { members(o.get("inputs"), "a") } else { json!([{"id": "a1", "name": "In", "type": "exec"}]) };
            let outs = if o.get("outputs").is_some() { members(o.get("outputs"), "r") } else { json!([{"id": "r1", "name": "Out", "type": "exec"}]) };
            arr(g, "macros").push(json!({"id": id, "name": o["name"].as_str().unwrap_or("New Macro"), "inputs": ins, "outputs": outs}));
            let e = add_node(g, "macro.entry", 0.0, 0.0, None, Some(&id))?;
            let x = add_node(g, "macro.exit", 420.0, 0.0, None, Some(&id))?;
            json!({"id": id, "entry": e, "exit": x})
        }
        "update_part" => {
            let id = text(o, "part")?;
            let list = if g["functions"].as_array().into_iter().flatten().any(|x| x["id"] == id.as_str()) { "functions" } else { "macros" };
            let x = arr(g, list).iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such Function or Macro")?;
            for (k, p) in [("name", ""), ("pure", ""), ("inputs", "a"), ("outputs", "r")] {
                if let Some(v) = o.get(k) {
                    x[k] = if p.is_empty() { v.clone() } else { members(Some(v), p) };
                }
            }
            Value::Null
        }
        "remove_part" => {
            let id = text(o, "part")?;
            let gone: Vec<String> = g["nodes"].as_array().into_iter().flatten().filter(|n| n["graph"] == id.as_str()).filter_map(|n| n["id"].as_str().map(String::from)).collect();
            arr(g, "nodes").retain(|n| n["graph"] != id.as_str());
            arr(g, "comments").retain(|n| n["graph"] != id.as_str());
            arr(g, "edges").retain(|e| !gone.iter().any(|n| e["from"]["node"] == n.as_str() || e["to"]["node"] == n.as_str()));
            arr(g, "functions").retain(|x| x["id"] != id.as_str());
            arr(g, "macros").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "add_struct" => {
            let id = fresh_part(g, "s");
            arr(g, "structs").push(json!({"id": id, "name": o["name"].as_str().unwrap_or("New Struct"), "fields": members(o.get("fields").or(Some(&json!([{"name": "Value", "type": "number"}]))), "f")}));
            json!(id)
        }
        "add_enum" => {
            let id = fresh_part(g, "e");
            arr(g, "enums").push(json!({"id": id, "name": o["name"].as_str().unwrap_or("New Enum"), "values": o.get("values").cloned().unwrap_or(json!(["First", "Second"]))}));
            json!(id)
        }
        "update_type" => {
            let id = text(o, "type_id")?;
            let list = if g["structs"].as_array().into_iter().flatten().any(|x| x["id"] == id.as_str()) { "structs" } else { "enums" };
            let x = arr(g, list).iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such struct or enum")?;
            if let Some(n) = o.get("name") {
                x["name"] = n.clone();
            }
            if let Some(f) = o.get("fields") {
                x["fields"] = members(Some(f), "f");
            }
            if let Some(v) = o.get("values") {
                x["values"] = v.clone();
            }
            Value::Null
        }
        "remove_type" => {
            let id = text(o, "type_id")?;
            arr(g, "structs").retain(|x| x["id"] != id.as_str());
            arr(g, "enums").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "add_comment" => {
            let id = fresh_id(g, "c");
            let mut c = json!({"id": id, "text": o["text"].as_str().unwrap_or("Comment").chars().take(2000).collect::<String>(), "pos": {"x": f("x").unwrap_or(0.0).round(), "y": f("y").unwrap_or(0.0).round()}, "size": {"w": f("w").unwrap_or(300.0).max(160.0).round(), "h": f("h").unwrap_or(200.0).max(80.0).round()}});
            if let Some(h) = home.filter(|h| !h.is_empty()) {
                c["graph"] = json!(h);
            }
            arr(g, "comments").push(c);
            json!(id)
        }
        "add_marker" => {
            let n = g["markers"].as_array().map(Vec::len).unwrap_or(0) + 1;
            let id = o["id"].as_str().map(String::from).unwrap_or_else(|| format!("marker_{n}"));
            if g["markers"].as_array().into_iter().flatten().any(|m| m["id"] == id.as_str()) {
                return Err(format!("there's a marker called {id} already"));
            }
            let ty = o["type"].as_str().unwrap_or("point");
            let mut m = json!({"id": id, "label": o["label"].as_str().unwrap_or("Marker"), "description": o["description"].as_str().unwrap_or(""), "type": ty, "min": o["min"].as_u64().unwrap_or(1), "colour": "", "icon": ""});
            for k in ["max", "keys", "colour", "icon", "size", "radius"] {
                if let Some(v) = o.get(k) {
                    m[k] = v.clone();
                }
            }
            if ty == "area" && m.get("size").is_none() && m.get("radius").is_none() {
                m["size"] = json!([256, 256, 128]);
            }
            arr(g, "markers").push(m);
            json!(id)
        }
        "update_marker" => {
            let id = text(o, "marker")?;
            let to = o["id"].as_str().map(String::from);
            let m = arr(g, "markers").iter_mut().find(|m| m["id"] == id.as_str()).ok_or("there's no such marker")?;
            for (k, v) in o.as_object().into_iter().flatten().filter(|(k, _)| !["op", "marker"].contains(&k.as_str())) {
                m[k] = v.clone();
            }
            // (a new id goes to the nodes that use it too)
            if let Some(to) = to.filter(|t| *t != id) {
                for n in arr(g, "nodes").iter_mut().filter(|n| n["params"]["marker"] == id.as_str()) {
                    n["params"]["marker"] = json!(to);
                }
            }
            Value::Null
        }
        "remove_marker" => {
            let id = text(o, "marker")?;
            arr(g, "markers").retain(|m| m["id"] != id.as_str());
            Value::Null
        }
        "add_event_graph" => {
            let id = fresh_part(g, "eg");
            arr(g, "eventGraphs").push(json!({"id": id, "name": o["name"].as_str().unwrap_or("Graph")}));
            json!(id)
        }
        "rename_event_graph" => {
            let id = text(o, "event_graph")?;
            let x = arr(g, "eventGraphs").iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such event graph")?;
            x["name"] = json!(text(o, "name")?);
            Value::Null
        }
        "remove_event_graph" => {
            // (its nodes and comments move onto the first Event Graph, as in the Studio)
            let id = text(o, "event_graph")?;
            for k in ["nodes", "comments"] {
                for n in arr(g, k).iter_mut().filter(|n| n["graph"] == id.as_str()) {
                    if let Some(m) = n.as_object_mut() {
                        m.remove("graph");
                    }
                }
            }
            arr(g, "eventGraphs").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "add_custom_event" => {
            let id = fresh_part(g, "ce");
            let mut c = json!({"id": id, "name": o["name"].as_str().unwrap_or("My Event"), "inputs": members(o.get("inputs"), "a")});
            if o["map"].as_bool() == Some(true) {
                c["map"] = json!(true);
            }
            arr(g, "customEvents").push(c);
            json!(id)
        }
        "update_custom_event" => {
            let id = text(o, "custom_event")?;
            let x = arr(g, "customEvents").iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such custom event")?;
            if let Some(n) = o.get("name") {
                x["name"] = n.clone();
            }
            if let Some(i) = o.get("inputs") {
                x["inputs"] = members(Some(i), "a");
            }
            if let Some(m) = o.get("map") {
                x["map"] = m.clone();
            }
            Value::Null
        }
        "remove_custom_event" => {
            let id = text(o, "custom_event")?;
            arr(g, "customEvents").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "add_timeline" => {
            let id = fresh_part(g, "tl");
            let mut t = json!({"id": id, "name": o["name"].as_str().unwrap_or("Timeline"), "length": f("length").unwrap_or(3.0), "tick": f("tick").unwrap_or(0.05), "tracks": o.get("tracks").cloned().unwrap_or(json!([]))});
            for k in ["loop", "autoplay", "useLastKey"] {
                if let Some(v) = o.get(k) {
                    t[k] = v.clone();
                }
            }
            arr(g, "timelines").push(t);
            json!(id)
        }
        "update_timeline" => {
            let id = text(o, "timeline")?;
            let x = arr(g, "timelines").iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such timeline")?;
            for k in ["name", "length", "tick", "tracks", "loop", "autoplay", "useLastKey"] {
                if let Some(v) = o.get(k) {
                    x[k] = v.clone();
                }
            }
            Value::Null
        }
        "add_table" => {
            let id = fresh_part(g, "dt");
            let t = json!({"id": id, "name": o["name"].as_str().unwrap_or("Table"), "key": o.get("key").cloned().unwrap_or(json!("")), "columns": o.get("columns").cloned().unwrap_or(json!([])), "rows": o.get("rows").cloned().unwrap_or(json!([]))});
            arr(g, "dataTables").push(t);
            json!(id)
        }
        "update_table" => {
            let id = text(o, "table")?;
            let x = arr(g, "dataTables").iter_mut().find(|x| x["id"] == id.as_str()).ok_or("there's no such data table")?;
            for k in ["name", "columns", "key", "rows"] {
                if let Some(v) = o.get(k) {
                    x[k] = v.clone();
                }
            }
            Value::Null
        }
        "remove_table" => {
            let id = text(o, "table")?;
            arr(g, "dataTables").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "remove_timeline" => {
            let id = text(o, "timeline")?;
            arr(g, "timelines").retain(|x| x["id"] != id.as_str());
            Value::Null
        }
        "set_mode" => {
            for k in ["name", "description", "settings", "bots"] {
                if let Some(v) = o.get(k) {
                    g["mode"][k] = v.clone();
                }
            }
            if let Some(k) = o.get("mapKit") {
                g["mapKit"] = k.clone();
            }
            Value::Null
        }
        "insert_snippet" => {
            let sid = text(o, "snippet")?;
            let list = get(ctx, "/api/tools/studio/snippets").await.map_err(|e| fail_text(&e))?;
            let sn = list["snippets"].as_array().into_iter().flatten().find(|x| x["id"] == sid.as_str()).ok_or_else(|| format!("there's no snippet {sid} (studio_snippets lists them)"))?;
            json!(insert_part(g, &sn["snippet"], f("x").unwrap_or(0.0), f("y").unwrap_or(0.0), home)?)
        }
        "" => return Err("each edit needs an op".into()),
        other => return Err(format!("{other} isn't an edit (the tool's description lists them)")),
    })
}

fn fail_text(f: &Fail) -> String {
    match f {
        Fail::Refused(m) | Fail::Error(m) => m.clone(),
    }
}

// ---- reading -----------------------------------------------------------------------------

/// The draft as agents read it: nodes with pins resolved, and the problems.
async fn view(ctx: &Ctx, id: &str, tab: Option<&str>) -> Done {
    let (g, dirty, undo, redo) = {
        let d = lock();
        let x = d.get(id).ok_or_else(|| err(NO_DRAFT))?;
        (x.graph.clone(), x.dirty, x.past.len(), x.future.len())
    };
    let problems = post(ctx, "/api/tools/studio/validate", json!({"graph": g})).await.map(|v| v["problems"].clone()).unwrap_or_else(|e| json!([{"level": "error", "message": fail_text(&e)}]));
    let pg = parsed(&g).map_err(err)?;
    let v = View::new(&pg);
    let mut nodes = vec![];
    for n in &pg.nodes {
        let home = n.graph.clone().unwrap_or_default();
        if tab.is_some_and(|t| t != home) {
            continue;
        }
        let def = v.defs.get(n.id.as_str());
        let wired_in = |pin: &str| v.ins(&n.id, pin).iter().map(|e| json!({"wire": e.id, "from": format!("{}.{}", e.from.node, e.from.pin)})).collect::<Vec<_>>();
        let inputs: Vec<Value> = v.inputs(&n.id).iter().map(|p| {
            let ty = v.in_type(&n.id, &p.id).unwrap_or_else(|| p.ty.clone());
            let w = wired_in(&p.id);
            let mut x = json!({"id": p.id, "type": ty, "label": p.label});
            if p.required {
                x["required"] = json!(true);
            }
            if !w.is_empty() {
                x["wires"] = json!(w);
            } else if let Some(prm) = &p.param {
                x["value"] = n.params.get(prm).cloned().unwrap_or(Value::Null);
                x["param"] = json!(prm);
            }
            x
        }).collect();
        let outputs: Vec<Value> = v.outputs(&n.id).iter().map(|p| {
            let ty = v.out_type(&n.id, &p.id).unwrap_or_else(|| p.ty.clone());
            let w: Vec<Value> = v.outs(&n.id, &p.id).iter().map(|e| json!({"wire": e.id, "to": format!("{}.{}", e.to.node, e.to.pin)})).collect();
            let mut x = json!({"id": p.id, "type": ty, "label": p.label});
            if !w.is_empty() {
                x["wires"] = json!(w);
            }
            x
        }).collect();
        let mut x = json!({"id": n.id, "type": n.kind, "title": def.map(|d| d.title.clone()), "pos": [n.pos.x, n.pos.y], "settings": n.params, "inputs": inputs, "outputs": outputs});
        if !home.is_empty() {
            x["graph"] = json!(home);
        }
        if n.breakpoint {
            x["breakpoint"] = json!(true);
        }
        nodes.push(x);
    }
    let errors = problems.as_array().map(|p| p.iter().filter(|x| x["level"] == "error").count()).unwrap_or(0);
    Ok(json!({"id": id, "mode": g["mode"], "unsaved": dirty, "undo": undo, "redo": redo, "nodes": nodes, "wires": g["edges"],
        "variables": g["variables"], "functions": g["functions"], "macros": g["macros"], "structs": g["structs"], "enums": g["enums"],
        "comments": g["comments"], "markers": g["markers"], "mapKit": g["mapKit"],
        "eventGraphs": g["eventGraphs"], "customEvents": g["customEvents"], "timelines": g["timelines"], "dataTables": g["dataTables"], "problems": problems, "errors": errors}))
}

// ---- running them ------------------------------------------------------------------------

/// The Studio's node help (the same file its help panel reads).
const NODE_HELP: &str = include_str!("../../../ui/src/views/tools/studio/nodehelp.json");

const TEMPLATES: &[(&str, &str)] = &[
    ("team-dm", include_str!("../../../ui/src/views/tools/studio/templates/team-dm.graph.json")),
    ("last-stand", include_str!("../../../ui/src/views/tools/studio/templates/last-stand.graph.json")),
    ("battle-royale", include_str!("../../../ui/src/views/tools/studio/templates/battle-royale.graph.json")),
];

pub async fn run(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "studio_draft_open" => async {
            let id = mode_id(a)?;
            if lock().get(&id).is_some_and(|d| d.dirty) && a["discard"].as_bool() != Some(true) {
                return Err(err("That draft has unsaved edits. Save it with studio_draft_save, or open again with discard: true."));
            }
            let mut g = match s(a, "from").unwrap_or("saved") {
                "saved" => get(ctx, &format!("/api/tools/studio/graph/{id}")).await?["graph"].clone(),
                "new" => json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": id, "name": id, "description": ""}, "variables": [], "nodes": [], "edges": [], "comments": []}),
                "template" => {
                    let t = s(a, "template").unwrap_or("team-dm");
                    let text = TEMPLATES.iter().find(|(n, _)| *n == t).map(|(_, x)| *x).ok_or_else(|| err("template is team-dm, last-stand or battle-royale."))?;
                    serde_json::from_str(text).map_err(|_| err("That template couldn't be read."))?
                }
                _ => return Err(err("from is saved, new or template.")),
            };
            let fresh = s(a, "from").is_some_and(|f| f != "saved");
            if fresh {
                g["mode"]["id"] = json!(id);
                g["mode"]["uid"] = json!(graph::new_uid());
                if let Some(n) = s(a, "name") {
                    g["mode"]["name"] = json!(n);
                }
            }
            parsed(&g).map_err(err)?;
            {
                let mut d = lock();
                if d.len() >= MAX_DRAFTS && !d.contains_key(&id) {
                    let old = d.iter().find(|(_, x)| !x.dirty).map(|(k, _)| k.clone());
                    match old {
                        Some(k) => {
                            d.remove(&k);
                        }
                        None => return Err(err("Too many drafts with unsaved edits: save or discard one first.")),
                    }
                }
                d.insert(id.clone(), Draft { graph: g, past: vec![], future: vec![], dirty: fresh });
            }
            view(ctx, &id, None).await
        }
        .await,
        "studio_draft" => async { view(ctx, &mode_id(a)?, s(a, "graph")).await }.await,
        "studio_edit" => async {
            let id = mode_id(a)?;
            let ops = a["ops"].as_array().filter(|o| !o.is_empty()).ok_or_else(|| err("ops is a list of edits."))?;
            if ops.len() > 200 {
                return Err(err("At most 200 edits at once."));
            }
            let start = lock().get(&id).map(|d| d.graph.clone()).ok_or_else(|| err(NO_DRAFT))?;
            let mut g = start.clone();
            let mut made = vec![];
            for (i, o) in ops.iter().enumerate() {
                match edit(ctx, &mut g, o).await {
                    Ok(v) => made.push(json!({"op": o["op"], "result": v})),
                    Err(e) => return Err(err(format!("Edit {} ({}) didn't work: {e}. Nothing was changed.", i + 1, o["op"].as_str().unwrap_or("?")))),
                }
            }
            if g["nodes"].as_array().map(Vec::len).unwrap_or(0) > graph::MAX_NODES || g["edges"].as_array().map(Vec::len).unwrap_or(0) > graph::MAX_EDGES {
                return Err(err("That's more nodes or wires than a graph can hold. Nothing was changed."));
            }
            parsed(&g).map_err(|e| err(format!("{e} Nothing was changed.")))?;
            {
                let mut d = lock();
                let x = d.get_mut(&id).ok_or_else(|| err(NO_DRAFT))?;
                if x.graph != start {
                    return Err(err("The draft changed while these edits ran. Read it again and retry."));
                }
                x.past.push(start);
                if x.past.len() > UNDO_LIMIT {
                    x.past.remove(0);
                }
                x.future.clear();
                x.graph = g.clone();
                x.dirty = true;
            }
            let problems = post(ctx, "/api/tools/studio/validate", json!({"graph": g})).await.map(|v| v["problems"].clone()).unwrap_or(Value::Null);
            Ok(json!({"results": made, "problems": problems, "next": "studio_draft shows the nodes and pins; studio_draft_save saves; studio_compile {id, draft: true} builds it."}))
        }
        .await,
        "studio_draft_undo" | "studio_draft_redo" => async {
            let id = mode_id(a)?;
            {
                let mut d = lock();
                let x = d.get_mut(&id).ok_or_else(|| err(NO_DRAFT))?;
                let (from, to) = if name == "studio_draft_undo" { (&mut x.past, &mut x.future) } else { (&mut x.future, &mut x.past) };
                let g = from.pop().ok_or_else(|| err("Nothing to undo or redo."))?;
                to.push(std::mem::replace(&mut x.graph, g));
                x.dirty = true;
            }
            view(ctx, &id, None).await
        }
        .await,
        "studio_draft_save" => async {
            let id = mode_id(a)?;
            let g = draft_graph(&id).ok_or_else(|| err(NO_DRAFT))?;
            super::more::ask_if_graph_there(ctx, &id).await?;
            let v = post(ctx, &format!("/api/tools/studio/graph/{id}"), json!({"graph": g})).await?;
            if let Some(x) = lock().get_mut(&id) {
                x.dirty = false;
            }
            Ok(v)
        }
        .await,
        "studio_check" => async {
            let id = mode_id(a)?;
            let g = if a["draft"].as_bool() == Some(true) { draft_graph(&id).ok_or_else(|| err(NO_DRAFT))? } else { get(ctx, &format!("/api/tools/studio/graph/{id}")).await?["graph"].clone() };
            post(ctx, "/api/tools/studio/check", json!({"graph": g})).await
        }
        .await,
        "studio_test_run_status" => get(ctx, "/api/tools/studio/testrun").await,
        "studio_test_run_cancel" => post(ctx, "/api/tools/studio/testrun/cancel", json!({})).await,
        "studio_node_help" => async {
            let all: Value = serde_json::from_str(NODE_HELP).map_err(|_| err("The node help couldn't be read."))?;
            let help = all["help"].as_object().cloned().unwrap_or_default();
            if let Some(t) = s(a, "type") {
                return help.get(t).cloned().map(|h| json!({"type": t, "help": h})).ok_or_else(|| err(format!("There's no node type {t} (studio_nodes lists them).")));
            }
            let words: Vec<String> = s(a, "search").unwrap_or("").to_lowercase().split_whitespace().map(String::from).collect();
            if words.is_empty() {
                return Err(err("Say a node type, or search words."));
            }
            let hits: Vec<Value> = help.iter().filter(|(k, h)| {
                let t = format!("{k} {} {}", h["title"], h["what"]).to_lowercase();
                words.iter().all(|w| t.contains(w.as_str()))
            }).take(30).map(|(k, h)| json!({"type": k, "title": h["title"], "what": h["what"]})).collect();
            Ok(json!({"matched": hits.len(), "nodes": hits}))
        }
        .await,
        "studio_requirements" => async { get(ctx, &format!("/api/tools/requirements/{}", mode_id(a)?)).await }.await,
        "studio_requirements_set" => async {
            let id = mode_id(a)?;
            let now = get(ctx, &format!("/api/tools/requirements/{id}")).await?;
            // (fields left out keep what's saved)
            let maps: Vec<Value> = match a.get("maps") {
                Some(m) => m.as_array().cloned().unwrap_or_default(),
                None => now["maps"].as_array().into_iter().flatten().map(|m| m["name"].clone()).collect(),
            };
            let default_map = a.get("defaultMap").cloned().unwrap_or_else(|| now["defaultMap"]["name"].clone());
            let mut links = serde_json::Map::new();
            for c in now["content"].as_array().into_iter().flatten() {
                if let (Some(p), false) = (c["path"].as_str(), c["link"].is_null()) {
                    links.insert(p.into(), c["link"].clone());
                }
            }
            for (k, v) in a["links"].as_object().into_iter().flatten() {
                links.insert(k.clone(), v.clone());
            }
            post(ctx, &format!("/api/tools/requirements/{id}"), json!({"defaultMap": default_map, "maps": maps, "links": links})).await
        }
        .await,
        "studio_snippets" => get(ctx, "/api/tools/studio/snippets").await,
        "studio_library" => get(ctx, "/api/tools/studio/library").await,
        "studio_kits" => async {
            match s(a, "id") {
                Some(_) => get(ctx, &format!("/api/tools/studio/kit/{}", mode_id(a)?)).await,
                None => get(ctx, "/api/tools/studio/kits").await,
            }
        }
        .await,
        "creature_waves" => get(ctx, "/api/tools/creatures/waves").await,
        "creature_waves_save" => async {
            let w = a.get("waves").filter(|w| w["mode"].is_string()).ok_or_else(|| err("waves is one mode's wave set with its mode."))?;
            post(ctx, "/api/tools/creatures/waves/save", w.clone()).await
        }
        .await,
        "callouts_read" => async { get(ctx, &format!("/api/tools/callouts/{}", map_arg(a)?)).await }.await,
        "callouts_suggest" => async { get(ctx, &format!("/api/tools/callouts/{}/suggest", map_arg(a)?)).await }.await,
        "callouts_save" => async {
            let m = map_arg(a)?;
            let changes = a["changes"].as_array().filter(|c| !c.is_empty()).ok_or_else(|| err("changes is a list of {id, place}."))?.clone();
            let now = get(ctx, &format!("/api/tools/callouts/{m}")).await?;
            ask(ctx, format!("An AI agent wants to write {} place name{} into {m}'s nav (the bots' map). The old nav is kept in History. Allow it?", changes.len(), if changes.len() == 1 { "" } else { "s" })).await?;
            post(ctx, &format!("/api/tools/callouts/{m}"), json!({"stamp": now["stamp"], "changes": changes})).await
        }
        .await,
        "publish_check" => async {
            let id = s(a, "id").unwrap_or("");
            match s(a, "kind") {
                Some("map") => get(ctx, &format!("/api/tools/community/publish/{}", map_arg(&json!({"map": id}))?)).await,
                Some("mode") => get(ctx, &format!("/api/tools/community/publish-mode/{}", super::catalog::id_arg(a, "id")?)).await,
                Some("prefab") => {
                    let p = id.replacen(':', "~", 1);
                    if p.is_empty() || p.len() > 80 || !p.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_~".contains(&c)) {
                        return Err(err("That isn't a prefab id."));
                    }
                    get(ctx, &format!("/api/tools/community/publish-prefab/{p}")).await
                }
                _ => Err(err("kind is map, mode or prefab.")),
            }
        }
        .await,
        _ => return None,
    })
}
