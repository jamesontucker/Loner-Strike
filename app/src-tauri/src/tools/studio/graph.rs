//! The Game Mode Studio's graph: our own versioned JSON (the source of truth; React Flow only
//! edits it), the node registry shared with the editor (`app/ui/src/views/tools/studio/nodes.json`,
//! read here at build time), and the checks a graph must pass before it's turned into Pawn.
//!
//! A graph file, version 2 (version 1 files, without variables, still open):
//! ```json
//! { "schema": "loner-strike/graph", "version": 2,
//!   "mode": { "id": "teamdm", "name": "Team DM", "description": "" },
//!   "variables": [{ "id": "v1", "name": "Kills", "type": "number", "scope": "player", "default": 0 }],
//!   "nodes": [{ "id": "n1", "type": "event.player_killed", "pos": {"x":0,"y":0}, "params": {}, "breakpoint": false }],
//!   "edges": [{ "id": "e1", "from": {"node":"n1","pin":"then"}, "to": {"node":"n2","pin":"exec"} }],
//!   "comments": [{ "id": "c1", "text": "", "pos": {"x":0,"y":0}, "size": {"w":300,"h":200} }] }
//! ```
//! Wires go from an output pin to an input pin. A flow ("exec") output has at most one wire;
//! a data input has at most one wire; flow inputs and data outputs can have many.
//!
//! Pin types: exec, player, entity, team, number, bool, text; `any` (a reroute, or a Value that
//! takes anything) and `var` (the type of the variable the node is set to).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

pub const SCHEMA: &str = "loner-strike/graph";
pub const VERSION: u32 = 2;
/// Limits that keep a graph (and the plugin made from it) sane.
pub const MAX_NODES: usize = 400;
pub const MAX_EDGES: usize = 1200;
pub const MAX_COMMENTS: usize = 100;
pub const MAX_VARIABLES: usize = 64;
/// Numbers a graph may hold (settings, variable defaults).
pub const NUMBER_LIMIT: f64 = 100_000.0;

// ---- the node registry ---------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Registry {
    pub version: u32,
    pub nodes: Vec<NodeDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeDef {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub category: String,
    #[serde(default)]
    pub event: bool,
    #[serde(default)]
    pub reroute: bool,
    /// no flow pins: worked out where its output is used
    #[serde(default)]
    pub pure: bool,
    /// "components" (gm_components) or "creatures" (the bot brain's ls_creature)
    #[serde(default)]
    pub needs: Option<String>,
    pub inputs: Vec<PinDef>,
    pub outputs: Vec<PinDef>,
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinDef {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub required: bool,
    /// may be left unwired (it then means "nobody" / "everyone" / nothing)
    #[serde(default)]
    pub optional: bool,
    /// an unconnected input takes this param's value instead
    #[serde(default)]
    pub param: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    /// choice | number | text | bool | name (an entity's name) | keys (component keys) |
    /// var (one of the graph's variables) | varvalue (a value of that variable's type)
    pub kind: String,
    pub default: Value,
    #[serde(default)]
    pub choices: Vec<(String, String)>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// a `var` param that only takes variables of this type
    #[serde(default, rename = "varType")]
    pub var_type: Option<String>,
}

const NODES_JSON: &str = include_str!("../../../../ui/src/views/tools/studio/nodes.json");

pub fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(NODES_JSON).expect("studio nodes.json is valid (checked by a test)"))
}

impl Registry {
    pub fn get(&self, kind: &str) -> Option<&NodeDef> {
        self.nodes.iter().find(|n| n.kind == kind)
    }
}

impl NodeDef {
    pub fn input(&self, id: &str) -> Option<&PinDef> {
        self.inputs.iter().find(|p| p.id == id)
    }
    pub fn output(&self, id: &str) -> Option<&PinDef> {
        self.outputs.iter().find(|p| p.id == id)
    }
    pub fn param(&self, id: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.id == id)
    }
    pub fn has_exec_in(&self) -> bool {
        self.inputs.iter().any(|i| i.ty == "exec")
    }
}

// ---- the graph -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Graph {
    pub schema: String,
    pub version: u32,
    pub mode: ModeInfo,
    #[serde(default)]
    pub variables: Vec<Variable>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModeInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// A value the mode keeps: one for the whole mode, or one per player. Kept for the map (per
/// player: reset when the player joins).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Variable {
    pub id: String,
    pub name: String,
    /// number | bool | text
    #[serde(rename = "type")]
    pub ty: String,
    /// mode | player | team
    pub scope: String,
    #[serde(default)]
    pub default: Value,
    /// never (kept for the map) | round (back to its start value at each new round)
    #[serde(default = "never", skip_serializing_if = "is_never")]
    pub reset: String,
}

fn never() -> String {
    "never".into()
}

fn is_never(s: &String) -> bool {
    s == "never"
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
pub struct Pos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub pos: Pos,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub breakpoint: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PinRef {
    pub node: String,
    pub pin: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Edge {
    pub id: String,
    pub from: PinRef,
    pub to: PinRef,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub pos: Pos,
    pub size: Size,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Reads a graph file: the JSON, then its schema and version.
pub fn parse(text: &str) -> Result<Graph, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("That isn't a graph file ({e})."))?;
    from_value(v)
}

/// A graph from JSON. Version 1 files (no variables) are read as version 2.
pub fn from_value(v: Value) -> Result<Graph, String> {
    if v.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err("That isn't a Game Mode Studio graph.".into());
    }
    match v.get("version").and_then(Value::as_u64) {
        Some(n) if (1..=VERSION as u64).contains(&n) => {}
        Some(n) if n > VERSION as u64 => return Err(format!("That graph was made by a newer Loner-Strike (graph version {n}). Update the app to open it.")),
        _ => return Err("That graph's version isn't one this app knows.".into()),
    }
    let mut g: Graph = serde_json::from_value(v).map_err(|e| format!("That graph file is damaged ({e})."))?;
    g.version = VERSION;
    Ok(g)
}

// ---- checks --------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Problem {
    /// "error" stops a compile; "warning" doesn't
    pub level: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    pub message: String,
}

impl Problem {
    fn err(node: Option<&str>, msg: impl Into<String>) -> Problem {
        Problem { level: "error", node: node.map(str::to_string), pin: None, edge: None, message: msg.into() }
    }
    fn warn(node: Option<&str>, msg: impl Into<String>) -> Problem {
        Problem { level: "warning", node: node.map(str::to_string), pin: None, edge: None, message: msg.into() }
    }
    fn pin(mut self, pin: &str) -> Problem {
        self.pin = Some(pin.to_string());
        self
    }
    fn edge(mut self, edge: &str) -> Problem {
        self.edge = Some(edge.to_string());
        self
    }
}

pub fn has_errors(p: &[Problem]) -> bool {
    p.iter().any(|p| p.level == "error")
}

/// Node, edge, comment and variable ids: short and plain, so they're safe in generated code
/// and logs.
pub fn valid_id(id: &str) -> bool {
    (1..=24).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A mode id: what modes.ini, the cfg and the plugin are named after.
pub fn valid_mode_id(id: &str) -> bool {
    (2..=24).contains(&id.len()) && id.starts_with(|c: char| c.is_ascii_lowercase()) && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The characters names, labels and text settings may use: letters, digits, space and plain
/// punctuation. Never `*`, `/`, `\\`, `"`, `^` (Pawn's escape), newlines or control characters,
/// so graph text can't end a comment, a string or a line in generated code or modes.ini, even
/// before the generator escapes it (community graphs are data, never code).
pub fn label_char(c: char) -> bool {
    c.is_alphanumeric() || " .,!?'()-:;+&%#@=<>[]_~$".contains(c)
}

/// Text that goes into modes.ini (quoted) or a Pawn string.
pub fn valid_label(s: &str, max: usize) -> bool {
    s.chars().count() <= max && s.chars().all(label_char)
}

/// An entity's name (targetname), a creature type: plain ASCII letters, digits, `_ - .`.
pub fn valid_name(s: &str) -> bool {
    s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
}

/// Component keys (`radius=160 capture_secs=8 name=B`): words of name characters and `=`,
/// separated by spaces. No quotes, so no value can carry anything else.
pub fn valid_keys(s: &str, max: usize) -> bool {
    s.len() <= max && s.chars().all(|c| c.is_ascii_alphanumeric() || " _-.=:,".contains(c))
}

/// A comment box's note: never put into generated code, but held to the same characters
/// (plus new lines).
pub fn valid_note(s: &str) -> bool {
    s.chars().count() <= 2000 && s.chars().all(|c| c == '\n' || label_char(c))
}

/// Does a value fit a variable of this type?
pub fn value_fits(ty: &str, v: &Value) -> bool {
    match ty {
        "number" => v.as_f64().is_some_and(|n| n.is_finite() && n.abs() <= NUMBER_LIMIT),
        "bool" => v.is_boolean(),
        "text" => v.as_str().is_some_and(|s| valid_label(s, 60)),
        _ => false,
    }
}

/// The graph as the checks and the generator see it: lookups by id, wires by pin.
pub struct View<'a> {
    pub g: &'a Graph,
    pub defs: HashMap<&'a str, &'static NodeDef>,
    pub nodes: HashMap<&'a str, &'a Node>,
    pub vars: HashMap<&'a str, &'a Variable>,
    /// output pin -> wires leaving it
    pub out_edges: HashMap<(String, String), Vec<&'a Edge>>,
    /// input pin -> wires arriving
    pub in_edges: HashMap<(String, String), Vec<&'a Edge>>,
}

impl<'a> View<'a> {
    pub fn new(g: &'a Graph) -> View<'a> {
        let reg = registry();
        let mut v = View { g, defs: HashMap::new(), nodes: HashMap::new(), vars: HashMap::new(), out_edges: HashMap::new(), in_edges: HashMap::new() };
        for n in &g.nodes {
            v.nodes.entry(n.id.as_str()).or_insert(n);
            if let Some(d) = reg.get(&n.kind) {
                v.defs.entry(n.id.as_str()).or_insert(d);
            }
        }
        for x in &g.variables {
            v.vars.entry(x.id.as_str()).or_insert(x);
        }
        for e in &g.edges {
            v.out_edges.entry((e.from.node.clone(), e.from.pin.clone())).or_default().push(e);
            v.in_edges.entry((e.to.node.clone(), e.to.pin.clone())).or_default().push(e);
        }
        v
    }

    /// The wires leaving an output pin.
    pub fn outs(&self, node: &str, pin: &str) -> &[&'a Edge] {
        self.out_edges.get(&(node.to_string(), pin.to_string())).map_or(&[], |v| v.as_slice())
    }

    /// The wires arriving at an input pin.
    pub fn ins(&self, node: &str, pin: &str) -> &[&'a Edge] {
        self.in_edges.get(&(node.to_string(), pin.to_string())).map_or(&[], |v| v.as_slice())
    }

    /// The variable a node is set to (its `var` param).
    pub fn var_of(&self, node: &str) -> Option<&'a Variable> {
        let id = self.nodes.get(node)?.params.get("var")?.as_str()?;
        self.vars.get(id).copied()
    }

    /// A pin's declared type with `var` resolved ("any" when the node has no variable yet).
    fn resolve(&self, node: &str, ty: &str) -> String {
        if ty == "var" {
            self.var_of(node).map(|v| v.ty.clone()).unwrap_or_else(|| "any".into())
        } else {
            ty.to_string()
        }
    }

    /// The type an output really carries: a reroute carries what comes into it.
    pub fn out_type(&self, node: &str, pin: &str) -> Option<String> {
        let mut seen = HashSet::new();
        let (mut node, mut pin) = (node.to_string(), pin.to_string());
        loop {
            let def = self.defs.get(node.as_str())?;
            if !def.reroute {
                return def.output(&pin).map(|p| self.resolve(&node, &p.ty));
            }
            if !seen.insert(node.clone()) {
                return None;
            }
            let e = *self.ins(&node, "in").first()?;
            node = e.from.node.clone();
            pin = e.from.pin.clone();
        }
    }

    /// The type an input wants (`var` resolved).
    pub fn in_type(&self, node: &str, pin: &str) -> Option<String> {
        let def = self.defs.get(node)?;
        def.input(pin).map(|p| self.resolve(node, &p.ty))
    }

    /// Where a flow wire from this output really goes (through reroutes): the node and pin.
    pub fn exec_target(&self, node: &str, pin: &str) -> Option<(&'a str, &'a str)> {
        let mut seen = HashSet::new();
        let mut e = *self.outs(node, pin).first()?;
        loop {
            let to = e.to.node.as_str();
            let def = self.defs.get(to)?;
            if !def.reroute {
                return Some((to, e.to.pin.as_str()));
            }
            if !seen.insert(to) {
                return None;
            }
            e = *self.outs(to, "out").first()?;
        }
    }

    /// Where a data input really comes from (through reroutes): the node and output pin.
    pub fn data_source(&self, node: &str, pin: &str) -> Option<(&'a str, &'a str)> {
        let mut seen = HashSet::new();
        let mut e = *self.ins(node, pin).first()?;
        loop {
            let from = e.from.node.as_str();
            let def = self.defs.get(from)?;
            if !def.reroute {
                return Some((from, e.from.pin.as_str()));
            }
            if !seen.insert(from) {
                return None;
            }
            e = *self.ins(from, "in").first()?;
        }
    }

    /// The flow outputs of a node, in the registry's order.
    pub fn exec_outs(&self, node: &str) -> Vec<&'static str> {
        self.defs.get(node).map(|d| d.outputs.iter().filter(|p| p.ty == "exec").map(|p| p.id.as_str()).collect()).unwrap_or_default()
    }

    /// Does `to` run after `from` (by flow wires, from any of `from`'s flow outputs)?
    pub fn runs_after(&self, from: &str, to: &str) -> bool {
        let mut stack: Vec<&str> = self.exec_outs(from).into_iter().filter_map(|p| self.exec_target(from, p).map(|(t, _)| t)).collect();
        let mut seen = HashSet::new();
        while let Some(cur) = stack.pop() {
            if cur == to {
                return true;
            }
            if !seen.insert(cur) {
                continue;
            }
            for p in self.exec_outs(cur) {
                if let Some((t, _)) = self.exec_target(cur, p) {
                    stack.push(t);
                }
            }
        }
        false
    }

    /// The flow values a data input depends on (directly, or through pure nodes): events'
    /// outputs, and outputs of flow nodes (loops, picks, actions that make something).
    pub fn flow_values(&self, node: &str, pin: &str) -> Vec<(&'a str, &'a str)> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<(String, String)> = vec![(node.to_string(), pin.to_string())];
        while let Some((n, p)) = stack.pop() {
            let Some((src, spin)) = self.data_source(&n, &p) else { continue };
            let Some(def) = self.defs.get(src) else { continue };
            if !def.pure {
                out.push((src, spin));
            } else if seen.insert(src) {
                for ip in def.inputs.iter().filter(|i| i.ty != "exec") {
                    stack.push((src.to_string(), ip.id.clone()));
                }
            }
        }
        out
    }

    /// The events (node ids) from which each node is reached by flow wires, and whether a
    /// timer (Delay, Repeat, Countdown) lies on the way.
    pub fn reached_from(&self) -> BTreeMap<&'a str, BTreeMap<&'a str, bool>> {
        let mut out: BTreeMap<&str, BTreeMap<&str, bool>> = BTreeMap::new();
        for n in &self.g.nodes {
            if !self.defs.get(n.id.as_str()).is_some_and(|d| d.event) {
                continue;
            }
            let mut stack = vec![(n.id.as_str(), false)];
            let mut seen = HashSet::new();
            while let Some((cur, timed)) = stack.pop() {
                if !seen.insert((cur, timed)) {
                    continue;
                }
                let e = out.entry(cur).or_default().entry(n.id.as_str()).or_insert(false);
                *e |= timed;
                let kind = self.defs.get(cur).map(|d| d.kind.as_str()).unwrap_or("");
                for p in self.exec_outs(cur) {
                    if let Some((t, _)) = self.exec_target(cur, p) {
                        stack.push((t, timed || deferred(kind, p)));
                    }
                }
            }
        }
        out
    }
}

/// Does this flow output run later (from a timer), not straight away?
pub fn deferred(kind: &str, pin: &str) -> bool {
    match kind {
        "timer.delay" => true,
        "timer.repeat" => pin == "tick",
        "timer.countdown" => pin == "done",
        "msg.menu" => pin != "then",
        _ => false,
    }
}

pub fn compatible(from: &str, to: &str) -> bool {
    from == to || from == "any" || to == "any" || (from == "player" && to == "entity")
}

/// Every problem with a graph: errors stop a compile, warnings are shown on the nodes.
pub fn validate(g: &Graph) -> Vec<Problem> {
    let reg = registry();
    let mut p = Vec::new();
    if g.schema != SCHEMA || g.version != VERSION {
        p.push(Problem::err(None, "That graph's version isn't one this app knows."));
        return p;
    }
    if !valid_mode_id(&g.mode.id) {
        p.push(Problem::err(None, "The mode's id needs 2 to 24 small letters, digits or _ (starting with a letter)."));
    }
    if g.mode.name.trim().is_empty() || !valid_label(&g.mode.name, 40) {
        p.push(Problem::err(None, "Give the mode a name (up to 40 letters, digits, spaces or plain punctuation; no * / \\ \" ^)."));
    }
    if !valid_label(&g.mode.description, 60) {
        p.push(Problem::err(None, "The mode's description can be up to 60 letters, digits, spaces or plain punctuation (no * / \\ \" ^)."));
    }
    if g.nodes.len() > MAX_NODES || g.edges.len() > MAX_EDGES || g.comments.len() > MAX_COMMENTS || g.variables.len() > MAX_VARIABLES {
        p.push(Problem::err(None, format!("That graph is too big (at most {MAX_NODES} nodes, {MAX_EDGES} wires, {MAX_COMMENTS} comments and {MAX_VARIABLES} variables).")));
        return p;
    }

    // variables
    let mut vids = HashSet::new();
    for x in &g.variables {
        if !valid_id(&x.id) || !vids.insert(x.id.as_str()) {
            p.push(Problem::err(None, "A variable has a bad or repeated id."));
            continue;
        }
        if x.name.trim().is_empty() || !valid_label(&x.name, 30) {
            p.push(Problem::err(None, "Give each variable a name (up to 30 letters, digits, spaces or plain punctuation)."));
        }
        if !["number", "bool", "text"].contains(&x.ty.as_str()) || !["mode", "player", "team"].contains(&x.scope.as_str()) || !["never", "round"].contains(&x.reset.as_str()) {
            p.push(Problem::err(None, format!("The variable \u{201c}{}\u{201d} has a type or scope this app doesn't know.", x.name.chars().take(30).collect::<String>())));
        } else if !x.default.is_null() && !value_fits(&x.ty, &x.default) {
            p.push(Problem::err(None, format!("The variable \u{201c}{}\u{201d} starts with a value that doesn't fit its type.", x.name.chars().take(30).collect::<String>())));
        }
    }

    // nodes and their params
    let v = View::new(g);
    let mut ids = HashSet::new();
    for n in &g.nodes {
        if !valid_id(&n.id) {
            p.push(Problem::err(None, format!("A node has a bad id ({:?}).", n.id.chars().take(30).collect::<String>())));
            continue;
        }
        if !ids.insert(n.id.as_str()) {
            p.push(Problem::err(Some(&n.id), "Two nodes share this id."));
            continue;
        }
        let Some(def) = reg.get(&n.kind) else {
            p.push(Problem::err(Some(&n.id), format!("This app doesn't know the node \u{201c}{}\u{201d}.", n.kind.chars().take(40).collect::<String>())));
            continue;
        };
        for k in n.params.keys() {
            if def.param(k).is_none() {
                p.push(Problem::warn(Some(&n.id), format!("\u{201c}{}\u{201d} isn't a setting of {}; it's ignored.", k.chars().take(30).collect::<String>(), def.title)));
            }
        }
        for prm in &def.params {
            let val = n.params.get(&prm.id);
            let res = match prm.kind.as_str() {
                "var" => {
                    let chosen = val.and_then(Value::as_str).and_then(|id| v.vars.get(id));
                    match chosen {
                        None => Err(format!("Pick a variable for {} (add one under This mode).", def.title)),
                        Some(x) if prm.var_type.as_deref().is_some_and(|t| t != x.ty) => Err(format!("{} needs a {} variable.", def.title, prm.var_type.as_deref().unwrap_or(""))),
                        _ => Ok(()),
                    }
                }
                "func" => match val.and_then(Value::as_str).and_then(|id| v.defs.get(id).map(|d| d.kind.as_str())) {
                    Some("func.define") => Ok(()),
                    _ => Err("Pick which Function to call (add a Function node first).".into()),
                },
                "timer" => match val.and_then(Value::as_str).and_then(|id| v.defs.get(id).map(|d| d.kind.as_str())) {
                    Some("timer.delay" | "timer.repeat" | "timer.countdown") => Ok(()),
                    _ => Err("Pick which timer to stop.".into()),
                },
                "varvalue" => match (val, v.var_of(&n.id)) {
                    (Some(x), Some(var)) if !(x.is_string() && x.as_str() == Some("")) && !value_fits(&var.ty, x) => Err(format!("{} doesn't fit the variable \u{201c}{}\u{201d}.", prm.label, var.name)),
                    _ => Ok(()),
                },
                _ => val.map_or(Ok(()), |x| check_param(prm, x)),
            };
            if let Err(why) = res {
                p.push(Problem::err(Some(&n.id), why).pin(&prm.id));
            }
        }
    }
    let mut cids = HashSet::new();
    for c in &g.comments {
        if !valid_id(&c.id) || !cids.insert(c.id.as_str()) {
            p.push(Problem::err(None, "A comment has a bad or repeated id."));
        }
        if c.color.as_deref().is_some_and(|k| !(k.len() == 7 && k.starts_with('#') && k[1..].chars().all(|h| h.is_ascii_hexdigit()))) {
            p.push(Problem::err(None, "A comment's colour isn't a colour (#rrggbb)."));
        }
        if !valid_note(&c.text) {
            p.push(Problem::err(None, "A comment is too long (2000 characters at most) or uses a character that isn't allowed (* / \\ \" ^)."));
        }
    }

    // wires
    let mut eids = HashSet::new();
    for e in &g.edges {
        if !valid_id(&e.id) || !eids.insert(e.id.as_str()) {
            p.push(Problem::err(None, "A wire has a bad or repeated id.").edge(&e.id));
            continue;
        }
        let (Some(fd), Some(td)) = (v.defs.get(e.from.node.as_str()), v.defs.get(e.to.node.as_str())) else {
            p.push(Problem::err(None, "A wire goes to a node that isn't there.").edge(&e.id));
            continue;
        };
        if fd.output(&e.from.pin).is_none() {
            p.push(Problem::err(Some(&e.from.node), format!("{} has no output \u{201c}{}\u{201d}.", fd.title, e.from.pin.chars().take(30).collect::<String>())).edge(&e.id));
            continue;
        }
        let Some(tp) = td.input(&e.to.pin) else {
            p.push(Problem::err(Some(&e.to.node), format!("{} has no input \u{201c}{}\u{201d}.", td.title, e.to.pin.chars().take(30).collect::<String>())).edge(&e.id));
            continue;
        };
        if e.from.node == e.to.node {
            p.push(Problem::err(Some(&e.to.node), "A wire can't go back into the same node.").edge(&e.id));
            continue;
        }
        let from_ty = v.out_type(&e.from.node, &e.from.pin).unwrap_or_else(|| "any".into());
        let to_ty = if td.reroute { "any".to_string() } else { v.in_type(&e.to.node, &e.to.pin).unwrap_or_else(|| "any".into()) };
        if !compatible(&from_ty, &to_ty) {
            p.push(Problem::err(Some(&e.to.node), format!("{} can't go into {} (a {} wire into a {} pin).", type_label(&from_ty), pin_label(td, tp), type_label(&from_ty), type_label(&to_ty))).pin(&tp.id).edge(&e.id));
        }
    }
    // one wire per flow output and per data input
    for ((node, pin), es) in &v.out_edges {
        if es.len() > 1 && v.out_type(node, pin).as_deref() == Some("exec") {
            p.push(Problem::err(Some(node), "A flow output can only go to one place. Add the steps one after another instead.").pin(pin));
        }
    }
    for ((node, pin), es) in &v.in_edges {
        let Some(def) = v.defs.get(node.as_str()) else { continue };
        // (a reroute takes one wire in, whatever it carries)
        let data_in = def.input(pin).is_some_and(|ip| ip.ty != "exec" && ip.ty != "any") || def.input(pin).is_some_and(|ip| ip.ty == "any" && !def.reroute);
        if es.len() > 1 && (data_in || def.reroute) {
            p.push(Problem::err(Some(node), "Only one wire can go into this pin.").pin(pin));
        }
    }

    // required inputs, reroutes that carry nothing, per-player variables
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        if def.reroute {
            if v.ins(&n.id, "in").is_empty() {
                p.push(Problem::warn(Some(&n.id), "Nothing goes into this reroute."));
            }
            continue;
        }
        for ip in &def.inputs {
            if ip.ty == "exec" {
                continue;
            }
            let wired = !v.ins(&n.id, &ip.id).is_empty();
            if ip.required && !wired {
                p.push(Problem::err(Some(&n.id), format!("{} needs something wired into {}.", def.title, pin_label(def, ip))).pin(&ip.id));
            }
        }
        if let (Some(var), Some(_)) = (v.var_of(&n.id), def.input("player")) {
            let wired = !v.ins(&n.id, "player").is_empty();
            let team_wired = !v.ins(&n.id, "team").is_empty();
            if var.scope == "player" && !wired {
                p.push(Problem::err(Some(&n.id), format!("\u{201c}{}\u{201d} is kept per player: wire in whose.", var.name)).pin("player"));
            } else if var.scope != "player" && wired {
                p.push(Problem::warn(Some(&n.id), format!("\u{201c}{}\u{201d} isn't kept per player: the Player wired in is ignored.", var.name)).pin("player"));
            }
            if var.scope == "team" && !team_wired {
                p.push(Problem::err(Some(&n.id), format!("\u{201c}{}\u{201d} is kept per team: wire in which.", var.name)).pin("team"));
            } else if var.scope != "team" && team_wired {
                p.push(Problem::warn(Some(&n.id), format!("\u{201c}{}\u{201d} isn't kept per team: the Team wired in is ignored.", var.name)).pin("team"));
            }
        }
    }

    // functions: names are unique, and a function can't end up calling itself
    let mut fnames = HashSet::new();
    for n in g.nodes.iter().filter(|n| n.kind == "func.define") {
        let name = n.params.get("name").and_then(Value::as_str).unwrap_or("my_function");
        if !fnames.insert(name) {
            p.push(Problem::err(Some(&n.id), "Two Functions have this name."));
        }
    }
    if let Some(node) = find_call_cycle(&v) {
        p.push(Problem::err(Some(node), "This Function ends up calling itself, which would never end."));
    }

    // flow: loops (flow and data), nodes nothing reaches, values used outside their event
    if !g.nodes.iter().any(|n| v.defs.get(n.id.as_str()).is_some_and(|d| d.event)) && !g.nodes.is_empty() {
        p.push(Problem::warn(None, "Nothing starts this graph yet: add an event, like Round Started."));
    }
    if let Some(node) = find_cycle(&v) {
        p.push(Problem::err(Some(node), "The flow loops back to here. Loops aren't possible yet: the flow has to end (a Repeat does the same job)."));
    }
    if let Some(node) = find_data_cycle(&v) {
        p.push(Problem::err(Some(node), "This value depends on itself."));
    }
    let reached = v.reached_from();
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        if def.event || def.reroute || def.pure {
            continue;
        }
        if def.has_exec_in() && !reached.contains_key(n.id.as_str()) {
            p.push(Problem::warn(Some(&n.id), "Nothing runs this node: wire an event's flow into it."));
        }
        let Some(events) = reached.get(n.id.as_str()) else { continue };
        for ip in def.inputs.iter().filter(|i| i.ty != "exec") {
            for (src, spin) in v.flow_values(&n.id, &ip.id) {
                let Some(sdef) = v.defs.get(src) else { continue };
                let label = sdef.output(spin).map(|o| o.label.clone()).unwrap_or_default();
                if !sdef.event {
                    // a loop's or a pick's value, or what an action made: only after it has run
                    if !v.runs_after(src, &n.id) {
                        p.push(Problem::err(Some(&n.id), format!("{label} is only known after {} runs: wire the flow through it first.", sdef.title)).pin(&ip.id));
                    }
                    continue;
                }
                if events.len() != 1 || !events.contains_key(src) {
                    p.push(Problem::err(Some(&n.id), format!("{label} is only known while {} runs, but this node also runs from another event.", sdef.title)).pin(&ip.id));
                } else if events[src] && sdef.output(spin).is_some_and(|o| o.ty == "text") {
                    p.push(Problem::err(Some(&n.id), format!("{label} (text) doesn't last through a Delay, Repeat or Countdown: keep it in a variable first.")).pin(&ip.id));
                }
            }
        }
    }
    p
}

/// The first node found on a flow loop.
fn find_cycle<'a>(v: &View<'a>) -> Option<&'a str> {
    // 0 unseen, 1 on the stack, 2 done
    fn visit<'a>(v: &View<'a>, n: &'a str, state: &mut HashMap<&'a str, u8>) -> Option<&'a str> {
        match state.get(n) {
            Some(1) => return Some(n),
            Some(2) => return None,
            _ => {}
        }
        state.insert(n, 1);
        for p in v.exec_outs(n) {
            if let Some((t, _)) = v.exec_target(n, p) {
                if let Some(hit) = visit(v, t, state) {
                    return Some(hit);
                }
            }
        }
        state.insert(n, 2);
        None
    }
    let mut state: HashMap<&str, u8> = HashMap::new();
    for n in &v.g.nodes {
        if let Some(hit) = visit(v, n.id.as_str(), &mut state) {
            return Some(hit);
        }
    }
    None
}

/// A Function whose flow calls (through other Functions) back into itself.
fn find_call_cycle<'a>(v: &View<'a>) -> Option<&'a str> {
    // the Functions each Function's flow calls
    let calls_from = |def_id: &'a str| -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut stack = vec![def_id];
        let mut seen = HashSet::new();
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur) {
                continue;
            }
            if v.defs.get(cur).is_some_and(|d| d.kind == "func.call") {
                if let Some(target) = v.nodes[cur].params.get("func").and_then(Value::as_str).and_then(|t| v.nodes.get_key_value(t).map(|(k, _)| *k)) {
                    out.push(target);
                }
            }
            for p in v.exec_outs(cur) {
                if let Some((t, _)) = v.exec_target(cur, p) {
                    stack.push(t);
                }
            }
        }
        out
    };
    let defs: Vec<&'a str> = v.g.nodes.iter().filter(|n| n.kind == "func.define").map(|n| n.id.as_str()).collect();
    for start in &defs {
        let mut stack = calls_from(start);
        let mut seen = HashSet::new();
        while let Some(f) = stack.pop() {
            if f == *start {
                return Some(start);
            }
            if seen.insert(f) {
                stack.extend(calls_from(f));
            }
        }
    }
    None
}

/// The first pure node whose value depends on itself.
fn find_data_cycle<'a>(v: &View<'a>) -> Option<&'a str> {
    fn visit<'a>(v: &View<'a>, n: &'a str, state: &mut HashMap<&'a str, u8>) -> Option<&'a str> {
        match state.get(n) {
            Some(1) => return Some(n),
            Some(2) => return None,
            _ => {}
        }
        state.insert(n, 1);
        if let Some(def) = v.defs.get(n) {
            for ip in def.inputs.iter().filter(|i| i.ty != "exec") {
                if let Some((src, _)) = v.data_source(n, &ip.id) {
                    if v.defs.get(src).is_some_and(|d| d.pure) {
                        if let Some(hit) = visit(v, src, state) {
                            return Some(hit);
                        }
                    }
                }
            }
        }
        state.insert(n, 2);
        None
    }
    let mut state: HashMap<&str, u8> = HashMap::new();
    for n in v.g.nodes.iter().filter(|n| v.defs.get(n.id.as_str()).is_some_and(|d| d.pure)) {
        if let Some(hit) = visit(v, n.id.as_str(), &mut state) {
            return Some(hit);
        }
    }
    None
}

fn type_label(ty: &str) -> &str {
    match ty {
        "exec" => "Flow",
        "player" => "Player",
        "entity" => "Entity",
        "team" => "Team",
        "number" => "Number",
        "bool" => "Yes/No",
        "text" => "Text",
        "any" => "Anything",
        _ => ty,
    }
}

fn pin_label(def: &NodeDef, pin: &PinDef) -> String {
    if pin.label.is_empty() {
        format!("{}'s flow input", def.title)
    } else {
        pin.label.clone()
    }
}

/// A param value against its definition (`var` and `varvalue` need the graph: see validate).
pub fn check_param(def: &ParamDef, v: &Value) -> Result<(), String> {
    let range = || match (def.min, def.max) {
        (Some(a), Some(b)) => format!(" from {a} to {b}"),
        _ => String::new(),
    };
    match def.kind.as_str() {
        "choice" => match v.as_str() {
            Some(s) if def.choices.iter().any(|(k, _)| k == s) => Ok(()),
            _ => Err(format!("Pick one of the choices for {}.", def.label)),
        },
        "number" => match v.as_f64() {
            Some(n) if n.is_finite() && n.abs() <= NUMBER_LIMIT && def.min.map_or(true, |m| n >= m) && def.max.map_or(true, |m| n <= m) => Ok(()),
            _ => Err(format!("{} needs a number{}.", def.label, range())),
        },
        "bool" => match v {
            Value::Bool(_) => Ok(()),
            _ => Err(format!("{} is yes or no.", def.label)),
        },
        "text" => match v.as_str() {
            Some(s) if valid_label(s, def.max.unwrap_or(100.0) as usize) => Ok(()),
            _ => Err(format!("{} can be up to {} letters, digits, spaces or plain punctuation (no * / \\ \" ^).", def.label, def.max.unwrap_or(100.0))),
        },
        "name" => match v.as_str() {
            Some(s) if valid_name(s) => Ok(()),
            _ => Err(format!("{} is a name of up to 32 letters, digits, _ - or .", def.label)),
        },
        "keys" => match v.as_str() {
            Some(s) if valid_keys(s, def.max.unwrap_or(120.0) as usize) => Ok(()),
            _ => Err(format!("{} are words like radius=160 (letters, digits, _ - . = : and spaces; no quotes).", def.label)),
        },
        "var" | "varvalue" | "func" | "timer" => Ok(()),
        _ => Err(format!("{} has a setting this app doesn't know.", def.label)),
    }
}

/// A param's value, or its default.
pub fn param<'a>(n: &'a Node, def: &'a NodeDef, id: &str) -> Option<&'a Value> {
    n.params.get(id).or_else(|| def.param(id).map(|p| &p.default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub fn graph(nodes: Value, edges: Value) -> Graph {
        from_value(json!({"schema": SCHEMA, "version": 2, "mode": {"id": "testmode", "name": "Test"}, "nodes": nodes, "edges": edges})).unwrap()
    }

    fn e(id: &str, a: &str, ap: &str, b: &str, bp: &str) -> Value {
        json!({"id": id, "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
    }

    fn msgs(p: &[Problem]) -> Vec<String> {
        p.iter().map(|p| format!("{}:{}:{}", p.level, p.node.clone().unwrap_or_default(), p.message)).collect()
    }

    const TYPES: [&str; 9] = ["exec", "player", "entity", "team", "number", "bool", "text", "any", "var"];

    #[test]
    fn registry_is_complete() {
        let r = registry();
        assert_eq!(r.version, 3);
        for kind in ["event.round_started", "event.player_killed", "cond.is_team", "player.give_weapon", "score.end_round", "msg.broadcast", "flow.reroute", "timer.delay", "var.set", "comp.on_capture", "creature.spawn"] {
            assert!(r.get(kind).is_some(), "{kind}");
        }
        let mut seen = HashSet::new();
        for n in &r.nodes {
            assert!(seen.insert(n.kind.as_str()), "{} twice", n.kind);
            assert!(!(n.pure && n.event), "{}", n.kind);
            assert_eq!(n.pure, !n.event && !n.reroute && !n.has_exec_in(), "{}: pure means no flow in", n.kind);
            for prm in &n.params {
                if !["var", "varvalue", "func", "timer"].contains(&prm.kind.as_str()) {
                    assert!(check_param(prm, &prm.default).is_ok(), "{} {} default", n.kind, prm.id);
                }
            }
            for pin in n.inputs.iter().chain(&n.outputs) {
                assert!(TYPES.contains(&pin.ty.as_str()), "{} {}", n.kind, pin.id);
                if let Some(pp) = &pin.param {
                    assert!(n.param(pp).is_some(), "{} {}", n.kind, pin.id);
                }
            }
        }
    }

    #[test]
    fn schema_and_version() {
        assert!(parse("nope").is_err());
        assert!(parse(r#"{"schema":"other","version":1}"#).unwrap_err().contains("isn't a Game Mode Studio graph"));
        assert!(parse(r#"{"schema":"loner-strike/graph","version":9,"mode":{"id":"a","name":"b"}}"#).unwrap_err().contains("newer"));
        // a version 1 file opens as version 2
        let g = parse(r#"{"schema":"loner-strike/graph","version":1,"mode":{"id":"abc","name":"ABC"}}"#).unwrap();
        assert!(g.nodes.is_empty() && g.version == 2 && g.variables.is_empty());
        let t = include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json");
        let g = parse(t).unwrap();
        let back = parse(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(g, back);
    }

    #[test]
    fn template_is_clean() {
        let t = include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json");
        let g = parse(t).unwrap();
        let p = validate(&g);
        assert!(p.is_empty(), "{:?}", msgs(&p));
    }

    #[test]
    fn mode_names() {
        assert!(valid_mode_id("teamdm") && valid_mode_id("a1_b"));
        assert!(!valid_mode_id("A") && !valid_mode_id("1abc") && !valid_mode_id("a-b") && !valid_mode_id("x"));
        let mut g = graph(json!([]), json!([]));
        g.mode.name = "Bad \"name\"".into();
        assert!(has_errors(&validate(&g)));
    }

    #[test]
    fn wrong_types_refused() {
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"c","type":"cond.is_team"}]),
            json!([e("e1","k","victim","c","exec"), e("e2","k","then","c","player")]),
        );
        let p = validate(&g);
        assert!(p.iter().filter(|p| p.message.contains("can't go into")).count() == 2, "{:?}", msgs(&p));
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"c","type":"cond.is_team"}]),
            json!([e("e1","k","then","c","exec"), e("e2","k","victim","c","player"), e("e3","k","attacker","c","team")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("Player can't go into Team")));
        // a player is an entity; an entity isn't a player; a number isn't a yes/no
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"o","type":"comp.crate_open"},{"id":"b","type":"flow.branch"},{"id":"h","type":"player.get_health"}]),
            json!([e("e1","k","then","o","exec"), e("e2","k","victim","o","container"), e("e3","k","attacker","o","player"), e("e4","o","then","b","exec"), e("e5","k","victim","h","player"), e("e6","h","out","b","cond")]),
        );
        let m = msgs(&validate(&g));
        assert!(!m.iter().any(|m| m.contains("Player can't go into Container")), "{m:?}");
        assert!(m.iter().any(|m| m.contains("Number can't go into Condition")), "{m:?}");
    }

    #[test]
    fn required_and_unknown() {
        let g = graph(
            json!([{"id":"r","type":"event.round_started"},{"id":"g","type":"player.give_weapon"},{"id":"x","type":"nope.node"}]),
            json!([e("e1","r","then","g","exec")]),
        );
        let m = msgs(&validate(&g));
        assert!(m.iter().any(|m| m.starts_with("error:g:") && m.contains("needs something wired into Player")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("error:x:") && m.contains("doesn't know")));
    }

    #[test]
    fn params_checked() {
        let g = graph(json!([
            {"id":"x","type":"score.end_round","params":{"winner":"bots"}},
            {"id":"m","type":"msg.broadcast","params":{"text":"a\"b","seconds":99}},
            {"id":"t","type":"player.teleport","params":{"name":"a b"}},
            {"id":"c","type":"comp.create","params":{"keys":"name=\"x\""}},
            {"id":"b","type":"flow.branch","params":{"cond":"yes"}}
        ]), json!([]));
        let p = validate(&g);
        assert!(p.iter().any(|p| p.node.as_deref() == Some("x") && p.pin.as_deref() == Some("winner")));
        assert_eq!(p.iter().filter(|p| p.node.as_deref() == Some("m") && p.level == "error").count(), 2);
        for n in ["t", "c", "b"] {
            assert!(p.iter().any(|p| p.node.as_deref() == Some(n) && p.level == "error"), "{n} {:?}", msgs(&p));
        }
    }

    #[test]
    fn one_wire_rules() {
        let g = graph(
            json!([{"id":"r","type":"event.round_started"},{"id":"a","type":"score.end_round"},{"id":"b","type":"score.end_round"}]),
            json!([e("e1","r","then","a","exec"), e("e2","r","then","b","exec")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("only go to one place")));
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"c","type":"cond.is_team"}]),
            json!([e("e1","k","then","c","exec"), e("e2","k","victim","c","player"), e("e3","k","attacker","c","player")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("Only one wire")));
    }

    #[test]
    fn loops_and_unreached() {
        let g = graph(
            json!([{"id":"r","type":"event.round_started"},{"id":"a","type":"msg.broadcast"},{"id":"b","type":"msg.broadcast"},{"id":"lone","type":"msg.broadcast"}]),
            json!([e("e1","r","then","a","exec"), e("e2","a","then","b","exec"), e("e3","b","then","a","exec")]),
        );
        let m = msgs(&validate(&g));
        assert!(m.iter().any(|m| m.contains("loops back")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("warning:lone:") && m.contains("Nothing runs")));
        let g = graph(json!([{"id":"a","type":"msg.broadcast"}]), json!([]));
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("Nothing starts this graph")));
        // a value that depends on itself
        let g = graph(
            json!([{"id":"a","type":"math.op"},{"id":"b","type":"math.op"}]),
            json!([e("e1","a","out","b","a"), e("e2","b","out","a","a")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("depends on itself")));
    }

    #[test]
    fn data_outside_its_event() {
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"r","type":"event.round_started"},{"id":"m","type":"msg.broadcast"},{"id":"g","type":"player.give_weapon"}]),
            json!([e("e1","k","then","m","exec"), e("e2","r","then","m","exec"), e("e3","m","then","g","exec"), e("e4","k","victim","g","player")]),
        );
        let m = msgs(&validate(&g));
        assert!(m.iter().any(|m| m.starts_with("error:g:") && m.contains("Victim is only known while Player Killed runs")), "{m:?}");
        // also through a pure node
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"r","type":"event.round_started"},{"id":"h","type":"player.get_health"},{"id":"s","type":"player.set_health"}]),
            json!([e("e1","r","then","s","exec"), e("e2","k","victim","h","player"), e("e3","h","out","s","amount"), e("e4","k","victim","s","player")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("only known while Player Killed")));
    }

    #[test]
    fn text_doesnt_cross_timers() {
        let g = graph(
            json!([{"id":"k","type":"creature.on_killed"},{"id":"d","type":"timer.delay"},{"id":"h","type":"msg.hud"}]),
            json!([e("e1","k","then","d","exec"), e("e2","d","then","h","exec"), e("e3","k","kind","h","value")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("doesn't last through a Delay")));
        // a player does
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"d","type":"timer.delay"},{"id":"h","type":"msg.hud"}]),
            json!([e("e1","k","then","d","exec"), e("e2","d","then","h","exec"), e("e3","k","victim","h","player")]),
        );
        assert!(validate(&g).is_empty(), "{:?}", msgs(&validate(&g)));
    }

    #[test]
    fn variables_checked() {
        let mut g = graph(
            json!([{"id":"r","type":"event.player_spawned"},{"id":"s","type":"var.set","params":{"var":"kills","value":"x"}},{"id":"a","type":"var.add","params":{"var":"name"}},{"id":"q","type":"var.get"}]),
            json!([e("e1","r","then","s","exec"), e("e2","s","then","a","exec")]),
        );
        g.variables = vec![
            Variable { id: "kills".into(), name: "Kills".into(), ty: "number".into(), scope: "player".into(), default: json!(0), reset: "never".into() },
            Variable { id: "name".into(), name: "Last name".into(), ty: "text".into(), scope: "mode".into(), default: json!(""), reset: "never".into() },
            Variable { id: "bad".into(), name: "Bad /*".into(), ty: "number".into(), scope: "mode".into(), default: json!("x"), reset: "never".into() },
        ];
        let m = msgs(&validate(&g));
        assert!(m.iter().any(|m| m.starts_with("error:s:") && m.contains("doesn't fit")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("error:s:") && m.contains("wire in whose")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("error:a:") && m.contains("needs a number variable")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("error:q:") && m.contains("Pick a variable")), "{m:?}");
        assert!(m.iter().any(|m| m.contains("Give each variable a name")), "{m:?}");
        assert!(m.iter().any(|m| m.contains("doesn't fit its type")), "{m:?}");
        // the Value pin takes the variable's type
        let v = View::new(&g);
        assert_eq!(v.in_type("s", "value").as_deref(), Some("number"));
    }

    #[test]
    fn reroutes_pass_types_through() {
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"x","type":"flow.reroute"},{"id":"y","type":"flow.reroute"},{"id":"g","type":"player.give_weapon"}]),
            json!([e("e1","k","then","x","in"), e("e2","x","out","g","exec"), e("e3","k","attacker","y","in"), e("e4","y","out","g","player")]),
        );
        let p = validate(&g);
        assert!(p.is_empty(), "{:?}", msgs(&p));
        let v = View::new(&g);
        assert_eq!(v.exec_target("k", "then"), Some(("g", "exec")));
        assert_eq!(v.data_source("g", "player"), Some(("k", "attacker")));
        let g = graph(
            json!([{"id":"k","type":"event.player_killed"},{"id":"x","type":"flow.reroute"},{"id":"g","type":"player.give_weapon"}]),
            json!([e("e1","k","then","x","in"), e("e2","x","out","g","player")]),
        );
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("Flow can't go into Player")));
    }
}
