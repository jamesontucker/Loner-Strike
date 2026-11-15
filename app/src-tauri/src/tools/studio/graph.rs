//! The Game Mode Studio's graph: our own versioned JSON (the source of truth; React Flow only
//! edits it), the node registry shared with the editor (`app/ui/src/views/tools/studio/nodes.json`,
//! read here at build time), and the checks a graph must pass before it's turned into Pawn.
//!
//! A graph file, version 3 (versions 1 and 2 still open: what's new is all optional):
//! ```json
//! { "schema": "loner-strike/graph", "version": 3,
//!   "mode": { "id": "teamdm", "name": "Team DM", "description": "" },
//!   "variables": [{ "id": "v1", "name": "Kills", "type": "number", "scope": "player", "default": 0 }],
//!   "nodes": [{ "id": "n1", "type": "event.player_killed", "pos": {"x":0,"y":0}, "params": {}, "breakpoint": false }],
//!   "edges": [{ "id": "e1", "from": {"node":"n1","pin":"then"}, "to": {"node":"n2","pin":"exec"} }],
//!   "comments": [{ "id": "c1", "text": "", "pos": {"x":0,"y":0}, "size": {"w":300,"h":200} }],
//!   "enums": [{ "id": "e1", "name": "Class", "values": ["Scout", "Tank"] }],
//!   "structs": [{ "id": "s1", "name": "Loadout", "fields": [{ "id": "f1", "name": "Ammo", "type": "number" }] }],
//!   "functions": [{ "id": "f1", "name": "Reward", "pure": false, "inputs": [{ "id": "a1", "name": "Who", "type": "player" }],
//!                   "outputs": [], "locals": [] }],
//!   "macros": [{ "id": "m1", "name": "Every Living", "inputs": [{ "id": "a1", "name": "In", "type": "exec" }], "outputs": [] }] }
//! ```
//! A node or comment with `"graph": "f1"` belongs to that Function's (or Macro's) own graph: its
//! tab in the editor. Without one it's on the Event Graph. Wires stay inside one graph. A
//! Function's graph has one Function Entry (its inputs as outputs) and Return nodes; a Macro's
//! has one Macro Entry and at most one Macro Exit, and is copied into the graph where it's used
//! (expand.rs) before the plugin is made.
//! Wires go from an output pin to an input pin. A flow ("exec") output has at most one wire;
//! a data input has at most one wire; flow inputs and data outputs can have many.
//!
//! Pin types: exec, player, entity, team, number, bool, text, vector, timer, `enum:<id>`,
//! `struct:<id>`, `array<T>` and `map<K,T>` (K text, number or player); `any` (a reroute, or a
//! Value that takes anything), `var` (the type of the variable the node is set to) and, in the
//! registry only, `$T` (the node's `of` setting), `$K` (its `key`), `$E` (its enum), `$S` (its
//! struct) and `$F` (the type of the struct field it sets). Function, macro, struct-making and
//! switch nodes get their pins from the graph (View::pins).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

pub const SCHEMA: &str = "loner-strike/graph";
pub const VERSION: u32 = 3;
/// Limits that keep a graph (and the plugin made from it) sane.
pub const MAX_NODES: usize = 400;
pub const MAX_EDGES: usize = 1200;
pub const MAX_COMMENTS: usize = 100;
pub const MAX_VARIABLES: usize = 64;
pub const MAX_FUNCTIONS: usize = 64;
pub const MAX_MACROS: usize = 32;
pub const MAX_ENUMS: usize = 32;
pub const MAX_STRUCTS: usize = 32;
/// Inputs or outputs of a Function or Macro, fields of a struct, values of an enum.
pub const MAX_MEMBERS: usize = 12;
/// Text fields of one struct (each is 64 characters of every record).
pub const MAX_TEXT_FIELDS: usize = 4;
/// The most times a loop goes round, and the most items an array holds.
pub const LOOP_LIMIT: u32 = 10_000;
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
    pub fn param(&self, id: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.id == id)
    }
    #[cfg(test)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<EnumDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structs: Vec<StructDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<Function>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<Macro>,
    /// what a map for this mode places (`ls_marker` entities): the mode map kit's markers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<MarkerDef>,
    /// the kit's advice for map makers (size, players, notes)
    #[serde(default, rename = "mapKit", skip_serializing_if = "Option::is_none")]
    pub map_kit: Option<MapKitInfo>,
    /// more event graphs after the first ("Event Graph"), only to organize: their nodes are the
    /// event graph's (a node's `graph` names one); two of the same event run in tab order
    #[serde(default, rename = "eventGraphs", skip_serializing_if = "Vec::is_empty")]
    pub event_graphs: Vec<EventGraph>,
    /// the mode's own events: a Custom Event node runs when they're called (Call Custom Event,
    /// Set Timer by Event, a map entity)
    #[serde(default, rename = "customEvents", skip_serializing_if = "Vec::is_empty")]
    pub custom_events: Vec<CustomEvent>,
    /// values over time (number, position, color tracks) and events at set moments, played by
    /// Timeline nodes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timelines: Vec<Timeline>,
    /// the mode's data tables: typed columns and rows, read by the Data table nodes (tables.rs)
    #[serde(default, rename = "dataTables", skip_serializing_if = "Vec::is_empty")]
    pub data_tables: Vec<super::tables::DataTable>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Timeline {
    pub id: String,
    pub name: String,
    /// seconds
    pub length: f64,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    #[serde(default)]
    pub autoplay: bool,
    /// the length is the last key's time
    #[serde(default, rename = "useLastKey")]
    pub use_last_key: bool,
    /// seconds between updates on the server (cost: lower is smoother and dearer)
    #[serde(default = "tick_default")]
    pub tick: f64,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

/// (0.1 s: the shortest repeat AMX Mod X's set_task runs; a graph asking for less gets 0.1)
/// The shortest Timeline tick that really happens: AMX Mod X runs a repeating task at most every
/// 0.1 s, so a tick set lower (older graphs: 0.05) runs at 0.1 (QA4-B).
pub const MIN_TICK: f64 = 0.1;

fn tick_default() -> f64 {
    0.1
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub name: String,
    /// "float", "vector", "color" or "event"
    pub kind: String,
    #[serde(default)]
    pub keys: Vec<Key>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Key {
    pub t: f64,
    /// a number (float), [x, y, z] (vector), [r, g, b] (color); nothing for an event
    #[serde(default)]
    pub v: Value,
    /// how it goes to the next key: "linear" (or none), "constant" (a step), "in", "out", "ease"
    /// (in and out), "custom" (a cubic curve by the tangents)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<String>,
    /// Custom curves: the key's own tangents (value per second; [x, y, z] for a position or
    /// color), coming in and going out; none: Auto (through the neighbors)
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub tin: Value,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub tout: Value,
}

/// The ways a key goes to the next (Key::ease).
pub const EASES: [&str; 6] = ["linear", "constant", "in", "out", "ease", "custom"];

impl Key {
    /// A tangent's value for one component (none: Auto).
    pub fn own_tangent(&self, side_in: bool, comp: Option<usize>) -> Option<f64> {
        let t = if side_in { &self.tin } else { &self.tout };
        match comp {
            None => t.as_f64(),
            Some(c) => t.as_array().and_then(|a| a.get(c)).and_then(Value::as_f64),
        }
    }
}

impl Timeline {
    /// How long it plays: its length, or the last key's time (Use Last Keyframe).
    pub fn play_length(&self) -> f64 {
        if self.use_last_key {
            let last = self.tracks.iter().flat_map(|t| t.keys.iter().map(|k| k.t)).fold(0.0f64, f64::max);
            if last > 0.0 {
                return last;
            }
        }
        self.length
    }
}

impl Track {
    /// Its keys in time order.
    pub fn sorted(&self) -> Vec<&Key> {
        let mut k: Vec<&Key> = self.keys.iter().collect();
        k.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        k
    }
    /// The output pin's type: number, vector (vector and color) or exec (event).
    pub fn pin_type(&self) -> &'static str {
        match self.kind.as_str() {
            "vector" | "color" => "vector",
            "event" => "exec",
            _ => "number",
        }
    }
}

/// The checks on the Timelines: sizes, keys within the length, values of the track's kind.
fn timeline_problems(g: &Graph, p: &mut Vec<Problem>) {
    if g.timelines.len() > 16 {
        p.push(Problem::err(None, "A mode has at most 16 Timelines."));
    }
    let mut ids = HashSet::new();
    for tl in &g.timelines {
        let name = short(&tl.name);
        if !valid_id(&tl.id) || !ids.insert(tl.id.as_str()) {
            p.push(Problem::err(None, "A Timeline has a bad or repeated id."));
        }
        if tl.name.trim().is_empty() || !valid_label(&tl.name, 30) {
            p.push(Problem::err(None, "Give each Timeline a name (up to 30 letters, digits, spaces or plain punctuation)."));
        }
        if !(0.1..=600.0).contains(&tl.length) {
            p.push(Problem::err(None, format!("The Timeline \u{201c}{name}\u{201d} lasts 0.1 to 600 seconds.")));
        }
        if !(0.02..=0.5).contains(&tl.tick) {
            p.push(Problem::err(None, format!("The Timeline \u{201c}{name}\u{201d} updates every 0.02 to 0.5 seconds.")));
        }
        if tl.tracks.len() > 8 {
            p.push(Problem::err(None, format!("The Timeline \u{201c}{name}\u{201d} has more than 8 tracks.")));
        }
        let mut tids = HashSet::new();
        for tr in &tl.tracks {
            if !valid_id(&tr.id) || !tids.insert(tr.id.as_str()) || RESERVED_PINS.contains(&tr.id.as_str()) || ["update", "finished", "forward", "time", "play_start", "stop", "reverse", "reverse_end", "set_time", "new_time"].contains(&tr.id.as_str()) {
                p.push(Problem::err(None, format!("The Timeline \u{201c}{name}\u{201d} has a bad or repeated track id.")));
            }
            if !valid_label(&tr.name, 30) || tr.name.trim().is_empty() {
                p.push(Problem::err(None, format!("Give each track of \u{201c}{name}\u{201d} a name.")));
            }
            if !["float", "vector", "color", "event"].contains(&tr.kind.as_str()) {
                p.push(Problem::err(None, format!("A track of \u{201c}{name}\u{201d} isn't a number, position, color or event track.")));
            }
            if tr.keys.len() > 64 {
                p.push(Problem::err(None, format!("The track \u{201c}{}\u{201d} has more than 64 keys.", short(&tr.name))));
            }
            for k in &tr.keys {
                let in_range = |x: f64| x.is_finite() && x.abs() <= NUMBER_LIMIT;
                let ok_v = match tr.kind.as_str() {
                    "float" => k.v.as_f64().is_some_and(in_range),
                    "vector" => k.v.as_array().is_some_and(|a| a.len() == 3 && a.iter().all(|x| x.as_f64().is_some_and(in_range))),
                    "color" => k.v.as_array().is_some_and(|a| a.len() == 3 && a.iter().all(|x| x.as_f64().is_some_and(|c| (0.0..=255.0).contains(&c)))),
                    _ => true,
                };
                if !k.t.is_finite() || k.t < 0.0 || k.t > tl.length.max(0.1) || !ok_v || k.ease.as_deref().is_some_and(|e| !EASES.contains(&e)) || !tangent_ok(&k.tin, &tr.kind) || !tangent_ok(&k.tout, &tr.kind) {
                    p.push(Problem::err(None, format!("A key of the track \u{201c}{}\u{201d} is out of range: its time must be between 0 and the length, and its value must fit the track (a number, a position, or a color from 0 to 255), with a known curve.", short(&tr.name))));
                    break;
                }
            }
        }
    }
}

/// A tangent: none, or a number (a number track), or three (a position or color), in range.
fn tangent_ok(t: &Value, kind: &str) -> bool {
    let ok = |x: &Value| x.as_f64().is_some_and(|f| f.is_finite() && f.abs() <= NUMBER_LIMIT * 100.0);
    match (t, kind) {
        (Value::Null, _) => true,
        (Value::Number(_), "float") => ok(t),
        (Value::Array(a), "vector" | "color") => a.len() == 3 && a.iter().all(ok),
        _ => false,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CustomEvent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub inputs: Vec<Member>,
    /// the map can fire it (an ls_event entity, or an info_target named lsev_<map name>)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub map: bool,
}

impl CustomEvent {
    /// The name a map uses for it: small letters, digits and _ ("Open Gate" -> open_gate).
    pub fn map_name(&self) -> String {
        let t: String = self.name.trim().to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        t.chars().take(26).collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EventGraph {
    pub id: String,
    pub name: String,
}

/// The graph with its extra event graphs folded into the first: their nodes and comments on the
/// Event Graph, the events in tab order and top to bottom (the order the plugin runs two of the
/// same event in). The same graph when there are none.
pub fn flatten(g: &Graph) -> std::borrow::Cow<'_, Graph> {
    if g.event_graphs.is_empty() {
        return std::borrow::Cow::Borrowed(g);
    }
    let rank = |h: &Option<String>| match h.as_deref() {
        None | Some("") => Some(0),
        Some(h) => g.event_graphs.iter().position(|e| e.id == h).map(|i| i + 1),
    };
    let mut out = g.clone();
    out.event_graphs.clear();
    let mut nodes: Vec<(usize, f64, usize, Node)> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let r = rank(&n.graph);
            let mut n = n.clone();
            if r.is_some() {
                n.graph = None;
            }
            let ev = registry().get(&n.kind).is_some_and(|d| d.event);
            (r.unwrap_or(0), if ev { n.pos.y } else { 0.0 }, i, n)
        })
        .collect();
    // (events only, in the places events were: the rest keep their order)
    let is_ev = |n: &Node| registry().get(&n.kind).is_some_and(|d| d.event);
    let slots: Vec<usize> = nodes.iter().enumerate().filter(|(_, x)| is_ev(&x.3)).map(|(i, _)| i).collect();
    let mut evs: Vec<(usize, f64, usize, Node)> = slots.iter().map(|&i| nodes[i].clone()).collect();
    evs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)).then(a.2.cmp(&b.2)));
    for (slot, ev) in slots.into_iter().zip(evs) {
        nodes[slot] = ev;
    }
    out.nodes = nodes.into_iter().map(|x| x.3).collect();
    for c in out.comments.iter_mut() {
        if rank(&c.graph).is_some() {
            c.graph = None;
        }
    }
    std::borrow::Cow::Owned(out)
}

/// A marker a map places for this mode (docs/notes/X.md, "Mode map kit format").
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MarkerDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// point, area or path
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub min: u32,
    /// None: no limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<MarkerKey>,
    #[serde(default)]
    pub colour: String,
    #[serde(default)]
    pub icon: String,
    /// an area's default box (x y z) or radius, for the editor
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
}

/// A key a marker carries in the map (wave=2, side=north).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MarkerKey {
    pub name: String,
    /// number, text, bool or choice
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub help: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// The kit's advice for map makers.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MapKitInfo {
    /// small, medium, large or empty
    #[serde(default)]
    pub size: String,
    #[serde(default, rename = "playersMin", skip_serializing_if = "Option::is_none")]
    pub players_min: Option<u32>,
    #[serde(default, rename = "playersMax", skip_serializing_if = "Option::is_none")]
    pub players_max: Option<u32>,
    #[serde(default)]
    pub notes: String,
}

pub const MARKER_TYPES: [&str; 3] = ["point", "area", "path"];
pub const MARKER_KEY_TYPES: [&str; 4] = ["number", "text", "bool", "choice"];
pub const MARKER_ICONS: [&str; 10] = ["flag", "skull", "star", "crate", "door", "person", "arrow", "circle", "bolt", "heart"];
/// keys the map entity uses itself (a marker's own keys can't be these)
pub const MARKER_RESERVED: [&str; 14] = ["classname", "origin", "angles", "angle", "model", "targetname", "target", "mode", "kind", "order", "path", "size", "radius", "spawnflags"];

/// A UUID v4 as text (lower case, 8-4-4-4-12).
pub fn valid_uid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { *c == b'-' } else { c.is_ascii_digit() || (b'a'..=b'f').contains(c) })
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// A new UUID v4 for a mode.
pub fn new_uid() -> String {
    use rand::RngCore;
    let mut r = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut r);
    r[6] = (r[6] & 0x0f) | 0x40;
    r[8] = (r[8] & 0x3f) | 0x80;
    let h: String = r.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// A marker id or key name: 1 to 24 small letters, digits or _ (written in maps by hand).
pub fn valid_marker_id(s: &str) -> bool {
    (1..=24).contains(&s.len()) && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// What's wrong with the graph's markers and map advice (empty: nothing).
pub fn marker_problems(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    if g.markers.len() > 32 {
        out.push("A mode can have at most 32 map markers.".to_string());
        return out;
    }
    let mut ids = HashSet::new();
    for m in &g.markers {
        let name = short(&m.label);
        if !valid_marker_id(&m.id) || !ids.insert(m.id.as_str()) || m.id.starts_with("ls_") {
            out.push(format!("The map marker \u{201c}{name}\u{201d} needs its own id: 1 to 24 small letters, digits or _ (not starting with ls_)."));
        }
        if m.label.trim().is_empty() || !valid_label(&m.label, 30) || !valid_label(&m.description, 200) {
            out.push(format!("The map marker \u{201c}{name}\u{201d} needs a name (up to 30 letters) and a description of up to 200 (plain punctuation)."));
        }
        if !MARKER_TYPES.contains(&m.ty.as_str()) {
            out.push(format!("The map marker \u{201c}{name}\u{201d} is a point, an area or a path."));
        }
        if m.min > 64 || m.max.is_some_and(|x| x == 0 || x > 64 || x < m.min) {
            out.push(format!("The map marker \u{201c}{name}\u{201d}: at least 0 to 64, at most 1 to 64 and not under the least."));
        }
        if !(m.colour.is_empty() || (m.colour.len() == 7 && m.colour.starts_with('#') && m.colour[1..].chars().all(|c| c.is_ascii_hexdigit()))) || !(m.icon.is_empty() || MARKER_ICONS.contains(&m.icon.as_str())) {
            out.push(format!("The map marker \u{201c}{name}\u{201d} has a color or icon this app doesn't know."));
        }
        if m.size.is_some_and(|s| s.iter().any(|x| !x.is_finite() || *x < 1.0 || *x > 16384.0)) || m.radius.is_some_and(|r| !r.is_finite() || !(1.0..=16384.0).contains(&r)) {
            out.push(format!("The map marker \u{201c}{name}\u{201d}: a size or radius is 1 to 16384 units."));
        }
        if m.keys.len() > 8 {
            out.push(format!("The map marker \u{201c}{name}\u{201d} can have at most 8 keys."));
        }
        let mut kn = HashSet::new();
        for k in &m.keys {
            let ok_default = match k.ty.as_str() {
                "number" => k.default.is_null() || k.default.as_f64().is_some_and(|x| x.is_finite() && x.abs() <= NUMBER_LIMIT),
                "text" => k.default.is_null() || k.default.as_str().is_some_and(|x| valid_label(x, 60)),
                "bool" => k.default.is_null() || k.default.is_boolean(),
                "choice" => k.default.is_null() || k.default.as_str().is_some_and(|x| k.choices.iter().any(|(v, _)| v == x)),
                _ => false,
            };
            let ok_choices = k.choices.len() <= 16 && k.choices.iter().all(|(v, l)| valid_marker_id(v) && valid_label(l, 30)) && (k.ty != "choice" || !k.choices.is_empty());
            let ok_range = k.min.map_or(true, f64::is_finite) && k.max.map_or(true, f64::is_finite) && k.min.zip(k.max).map_or(true, |(a, b)| a <= b);
            if !valid_marker_id(&k.name) || MARKER_RESERVED.contains(&k.name.as_str()) || !kn.insert(k.name.as_str()) || !MARKER_KEY_TYPES.contains(&k.ty.as_str()) || !ok_default || !ok_choices || !ok_range || !valid_label(&k.help, 120) {
                out.push(format!("The map marker \u{201c}{name}\u{201d}: the key \u{201c}{}\u{201d} needs its own name (small letters, digits, _; not a key maps use themselves), a type and a default that fits it.", short(&k.name)));
            }
        }
    }
    if let Some(k) = &g.map_kit {
        let players = |x: Option<u32>| x.map_or(true, |p| (1..=32).contains(&p));
        if !["", "small", "medium", "large"].contains(&k.size.as_str()) || !players(k.players_min) || !players(k.players_max) || k.players_min.zip(k.players_max).is_some_and(|(a, b)| a > b) || !valid_label(&k.notes, 600) {
            out.push("The map advice: a size of small, medium or large, players 1 to 32, and notes of up to 600 letters.".to_string());
        }
    }
    out
}

/// A named list of choices (Class: Scout, Tank, Medic): its values are 0, 1, 2...
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EnumDef {
    pub id: String,
    pub name: String,
    pub values: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<LibFrom>,
}

/// Where a Function, Macro, struct or enum came from: an item of the player's library and its
/// version, so the Studio can offer "Update from library" when that item changes.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LibFrom {
    pub lib: String,
    pub version: u32,
}

/// A record of named values (a Loadout: Weapon, Ammo, Armor).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StructDef {
    pub id: String,
    pub name: String,
    pub fields: Vec<Member>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<LibFrom>,
}

/// A struct's field, or a Function's or Macro's input or output.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Member {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// A Function: its own graph (nodes with `graph` = its id) from Function Entry to Return.
/// A pure one has no flow pins where it's called and works out its outputs where they're used.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Function {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pure: bool,
    #[serde(default)]
    pub inputs: Vec<Member>,
    #[serde(default)]
    pub outputs: Vec<Member>,
    /// variables only this Function sees, back to their start value each time it runs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locals: Vec<Variable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<LibFrom>,
}

/// A Macro: a piece of graph copied in wherever it's used. Its inputs and outputs can be flow
/// pins (type exec) too, several of each.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Macro {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub inputs: Vec<Member>,
    #[serde(default)]
    pub outputs: Vec<Member>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<LibFrom>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModeInfo {
    /// the short game id (modes.ini, the cfg and plugin names); can differ once installed when
    /// another mode has it
    pub id: String,
    /// the mode's permanent identity (a UUID v4 made when the mode is made; a rename keeps it)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uid: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// game settings for the mode's cfg (run after base.cfg each map): the same allow-list as
    /// the Set Setting node
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<Setting>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Setting {
    pub cvar: String,
    pub value: f64,
}

/// The settings a mode may set: Set Setting's choices.
pub fn allowed_settings() -> Vec<&'static str> {
    registry().get("game.set_cvar").and_then(|d| d.param("cvar")).map(|p| p.choices.iter().map(|(k, _)| k.as_str()).collect()).unwrap_or_default()
}

/// A value the mode keeps: one for the whole mode, one per player or one per team. Kept for the
/// map (per player: reset when the player joins). Any type (`types`); arrays, maps and structs
/// only for the whole mode. A Function's locals are Variables too (scope "mode").
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Variable {
    pub id: String,
    pub name: String,
    /// any variable type (type_ok)
    #[serde(rename = "type")]
    pub ty: String,
    /// mode | player | team
    pub scope: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub default: Value,
    /// never (kept for the map) | round (back to its start value at each new round)
    #[serde(default = "never", skip_serializing_if = "is_never")]
    pub reset: String,
    /// a mode setting: hosts can change its start value (a cvar of the mode, kept per server)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setting: Option<SettingDef>,
}

/// A variable a host can set (Blueprint's Instance Editable): its label, help, range or choices.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SettingDef {
    pub label: String,
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub min: f64,
    #[serde(default = "setting_max")]
    pub max: f64,
    /// a number's step in menus (0: a tenth of the range, at least 1)
    #[serde(default)]
    pub step: f64,
    /// a number picked from named choices (its value is the choice's place, 0, 1, 2...)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
    #[serde(default)]
    pub group: String,
}

fn setting_max() -> f64 {
    100.0
}

impl SettingDef {
    /// The range a value is clamped to: 0/1 for yes-no, 0..choices-1 for choices.
    pub fn range(&self, ty: &str) -> (f64, f64) {
        if ty == "bool" {
            (0.0, 1.0)
        } else if !self.choices.is_empty() {
            (0.0, (self.choices.len() - 1) as f64)
        } else {
            (self.min, self.max)
        }
    }
}

/// The admin menu's command for a mode's settings (the Game Mode Menu's "options", 31 letters at most).
pub fn settings_menu_cmd(mode: &str) -> String {
    format!("lsmm_{}", mode).to_ascii_lowercase()
}

/// The mode settings a graph has: whole-mode numbers and yes/no's marked as settings.
pub fn mode_settings(g: &Graph) -> Vec<&Variable> {
    g.variables.iter().filter(|x| x.setting.is_some() && x.scope == "mode" && matches!(x.ty.as_str(), "number" | "bool")).collect()
}

/// A mode setting's cvar: `lsm_<mode>_<variable id>` (small letters).
pub fn setting_cvar(mode: &str, var: &str) -> String {
    format!("lsm_{}_{}", mode, var).to_ascii_lowercase()
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
    /// "Don't warn for this one": no Problems note about bots for this node (bot_awareness)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
    /// the Function or Macro whose graph this node is on (none: the Event Graph)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
}

/// Reads a graph file: the JSON, then its schema and version.
pub fn parse(text: &str) -> Result<Graph, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("That isn't a graph file ({e})."))?;
    from_value(v)
}

/// A graph from JSON. Version 1 (no variables) and 2 (no functions, macros, enums or structs)
/// files are read as version 3: everything they lack starts empty.
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

/// An entity's name as the map has it (Find Component): a name, and spaces.
/// A file under the game folder for a model or a sound (`models/x.mdl`, `ambience/x.wav`): letters,
/// digits, `_ - . /`, up to 64, never `..` or a leading `/`.
pub fn valid_path(s: &str) -> bool {
    s.len() <= 64 && !s.contains("..") && !s.starts_with('/') && s.chars().all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c))
}

pub fn valid_ent_name(s: &str) -> bool {
    s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric() || " _-.".contains(c))
}

/// Component keys (`radius=160 capture_secs=8 name=B`): words of name characters and `=`,
/// separated by spaces. No quotes, so no value can carry anything else.
pub fn valid_keys(s: &str, max: usize) -> bool {
    s.len() <= max && s.chars().all(|c| c.is_ascii_alphanumeric() || " _-.=:,|;@".contains(c))
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

// ---- types ---------------------------------------------------------------------------------

/// The plain types: a value of one fits in a pin, a variable, an array's item.
pub const BASE_TYPES: [&str; 8] = ["number", "bool", "text", "player", "entity", "team", "vector", "timer"];
/// What a map's keys can be.
pub const KEY_TYPES: [&str; 3] = ["text", "number", "player"];

/// `array<T>` -> ("array", None, "T"); `map<K,T>` -> ("map", Some("K"), "T").
pub fn container(t: &str) -> Option<(&str, Option<&str>, &str)> {
    if let Some(inner) = t.strip_prefix("array<").and_then(|r| r.strip_suffix('>')) {
        return Some(("array", None, inner));
    }
    let inner = t.strip_prefix("map<").and_then(|r| r.strip_suffix('>'))?;
    let (k, v) = inner.split_once(',')?;
    Some(("map", Some(k), v))
}

/// The enum or struct a type names (`enum:e1` -> ("enum", "e1")).
pub fn named_type(t: &str) -> Option<(&str, &str)> {
    t.strip_prefix("enum:").map(|id| ("enum", id)).or_else(|| t.strip_prefix("struct:").map(|id| ("struct", id)))
}

/// A plain type, an enum or (with `structs`) a struct of this graph.
pub fn item_type_ok(g: &Graph, t: &str, structs: bool) -> bool {
    BASE_TYPES.contains(&t)
        || match named_type(t) {
            Some(("enum", id)) => g.enums.iter().any(|e| e.id == id),
            Some(("struct", id)) => structs && g.structs.iter().any(|s| s.id == id),
            _ => false,
        }
}

/// A type a variable, a Function's input or output or a node's `of` can have: an item type,
/// an array of them, or a map from text, numbers or players to plain types or enums.
pub fn type_ok(g: &Graph, t: &str) -> bool {
    match container(t) {
        Some(("array", None, item)) => item_type_ok(g, item, true),
        Some(("map", Some(k), item)) => KEY_TYPES.contains(&k) && item_type_ok(g, item, false),
        _ => item_type_ok(g, t, true),
    }
}

/// Values that live in a ring of slots (a vector, a struct): they don't last long when not in
/// a variable, like text.
pub fn short_lived(t: &str) -> bool {
    t == "text" || t == "vector" || t.starts_with("struct:")
}

pub fn compatible(from: &str, to: &str) -> bool {
    if from == to || from == "any" || to == "any" || (from == "player" && to == "entity") {
        return true;
    }
    match (container(from), container(to)) {
        (Some((a, ka, ia)), Some((b, kb, ib))) => a == b && (ka == kb || ka == Some("any") || kb == Some("any")) && (ia == ib || ia == "any" || ib == "any"),
        _ => false,
    }
}

/// The graph as the checks and the generator see it: lookups by id, wires by pin.
pub struct View<'a> {
    pub g: &'a Graph,
    pub defs: HashMap<&'a str, &'static NodeDef>,
    pub nodes: HashMap<&'a str, &'a Node>,
    /// the mode's variables and every Function's locals
    pub vars: HashMap<&'a str, &'a Variable>,
    /// a local variable -> its Function
    pub local_of: HashMap<&'a str, &'a str>,
    pub funcs: HashMap<&'a str, &'a Function>,
    pub macros: HashMap<&'a str, &'a Macro>,
    pub enums: HashMap<&'a str, &'a EnumDef>,
    pub structs: HashMap<&'a str, &'a StructDef>,
    /// each node's pins (from the registry, or the graph for Function, Macro, struct and
    /// switch nodes), types resolved
    pub pins: HashMap<&'a str, Pins>,
    /// output pin -> wires leaving it
    pub out_edges: HashMap<(String, String), Vec<&'a Edge>>,
    /// input pin -> wires arriving
    pub in_edges: HashMap<(String, String), Vec<&'a Edge>>,
}

#[derive(Debug, Clone, Default)]
pub struct Pins {
    pub inputs: Vec<PinDef>,
    pub outputs: Vec<PinDef>,
}

fn exec_pin(id: &str, label: &str) -> PinDef {
    PinDef { id: id.into(), ty: "exec".into(), label: label.into(), required: false, optional: false, param: None }
}

fn member_pin(m: &Member, required: bool) -> PinDef {
    PinDef { id: m.id.clone(), ty: m.ty.clone(), label: m.name.clone(), required: required && m.ty != "exec", optional: !required, param: None }
}

/// A count setting of a node (Sequence's outputs, Make Array's items), clamped.
pub fn count_param(n: &Node, def: &NodeDef, id: &str, max: usize) -> usize {
    param(n, def, id).and_then(Value::as_f64).filter(|x| x.is_finite()).map_or(0, |x| x.round().clamp(0.0, max as f64) as usize)
}

/// A Switch's cases (its `cases` setting: words or numbers split by commas), at most 16.
pub fn switch_cases(n: &Node, def: &NodeDef) -> Vec<String> {
    let text = param(n, def, "cases").and_then(Value::as_str).unwrap_or("");
    text.split(',').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).take(16).collect()
}

impl<'a> View<'a> {
    pub fn new(g: &'a Graph) -> View<'a> {
        let reg = registry();
        let mut v = View {
            g,
            defs: HashMap::new(),
            nodes: HashMap::new(),
            vars: HashMap::new(),
            local_of: HashMap::new(),
            funcs: HashMap::new(),
            macros: HashMap::new(),
            enums: HashMap::new(),
            structs: HashMap::new(),
            pins: HashMap::new(),
            out_edges: HashMap::new(),
            in_edges: HashMap::new(),
        };
        for n in &g.nodes {
            v.nodes.entry(n.id.as_str()).or_insert(n);
            if let Some(d) = reg.get(&n.kind) {
                v.defs.entry(n.id.as_str()).or_insert(d);
            }
        }
        for x in &g.variables {
            v.vars.entry(x.id.as_str()).or_insert(x);
        }
        for f in &g.functions {
            v.funcs.entry(f.id.as_str()).or_insert(f);
            for x in &f.locals {
                if !v.vars.contains_key(x.id.as_str()) {
                    v.vars.insert(x.id.as_str(), x);
                    v.local_of.insert(x.id.as_str(), f.id.as_str());
                }
            }
        }
        for m in &g.macros {
            v.macros.entry(m.id.as_str()).or_insert(m);
        }
        for e in &g.enums {
            v.enums.entry(e.id.as_str()).or_insert(e);
        }
        for s in &g.structs {
            v.structs.entry(s.id.as_str()).or_insert(s);
        }
        let ids: Vec<&'a str> = v.nodes.keys().copied().collect();
        for id in ids {
            if let Some(p) = v.make_pins(id) {
                v.pins.insert(id, p);
            }
        }
        for e in &g.edges {
            v.out_edges.entry((e.from.node.clone(), e.from.pin.clone())).or_default().push(e);
            v.in_edges.entry((e.to.node.clone(), e.to.pin.clone())).or_default().push(e);
        }
        v
    }

    /// The Function or Macro whose graph a node is on ("" for the Event Graph).
    pub fn graph_of(&self, node: &str) -> &'a str {
        self.nodes.get(node).and_then(|n| n.graph.as_deref()).unwrap_or("")
    }

    fn param_str(&self, node: &str, id: &str) -> Option<&'a str> {
        self.nodes.get(node)?.params.get(id)?.as_str()
    }

    /// The Function a call (or timer) node is set to.
    pub fn func_of(&self, node: &str) -> Option<&'a Function> {
        self.param_str(node, "func").and_then(|f| self.funcs.get(f).copied())
    }

    /// The struct a node works with: its `struct` setting.
    pub fn struct_of(&self, node: &str) -> Option<&'a StructDef> {
        self.param_str(node, "struct").and_then(|s| self.structs.get(s).copied())
    }

    pub fn enum_of(&self, node: &str) -> Option<&'a EnumDef> {
        self.param_str(node, "enum").and_then(|s| self.enums.get(s).copied())
    }

    /// A struct type's definition (`struct:s1`).
    pub fn struct_type(&self, ty: &str) -> Option<&'a StructDef> {
        ty.strip_prefix("struct:").and_then(|s| self.structs.get(s).copied())
    }

    pub fn enum_type(&self, ty: &str) -> Option<&'a EnumDef> {
        ty.strip_prefix("enum:").and_then(|s| self.enums.get(s).copied())
    }

    /// The field a Set Field node sets (in the struct of its variable).
    pub fn field_of(&self, node: &str) -> Option<&'a Member> {
        let var = self.var_of(node)?;
        let s = self.struct_type(&var.ty)?;
        let f = self.param_str(node, "field")?;
        s.fields.iter().find(|m| m.id == f)
    }

    fn make_pins(&self, id: &str) -> Option<Pins> {
        let n = self.nodes.get(id)?;
        let def = self.defs.get(id)?;
        let mut p = Pins { inputs: def.inputs.clone(), outputs: def.outputs.clone() };
        let home = n.graph.as_deref().unwrap_or("");
        match def.kind.as_str() {
            "fn.entry" => {
                if let Some(f) = self.funcs.get(home) {
                    p.outputs.extend(f.inputs.iter().map(|m| member_pin(m, false)));
                }
            }
            "table.get_row" | "table.for_each" | "table.get_cell" | "table.find" | "table.keys" => {
                if let Some((_, t)) = super::tables::table_of(&self.g.data_tables, n) {
                    let col_pin = |c: &super::tables::Column| PinDef { id: c.id.clone(), ty: c.pin_type(), label: c.name.clone(), required: false, optional: true, param: None };
                    let key_ty = t.key_col().map(|(_, c)| c.pin_type()).unwrap_or_else(|| "any".into());
                    let picked = n.params.get("column").and_then(Value::as_str).and_then(|c| t.col(c)).map(|(_, c)| c.pin_type()).unwrap_or_else(|| "any".into());
                    match def.kind.as_str() {
                        "table.get_row" | "table.for_each" => p.outputs.extend(t.columns.iter().map(col_pin)),
                        _ => {}
                    }
                    // (the registry says "any" for these; the table says what they are)
                    for pin in p.inputs.iter_mut() {
                        match (def.kind.as_str(), pin.id.as_str()) {
                            ("table.get_row", "key") => pin.ty = key_ty.clone(),
                            ("table.find", "value") => pin.ty = picked.clone(),
                            _ => {}
                        }
                    }
                    for pin in p.outputs.iter_mut() {
                        match (def.kind.as_str(), pin.id.as_str()) {
                            ("table.get_cell", "out") => pin.ty = picked.clone(),
                            ("table.keys", "out") => pin.ty = format!("array<{key_ty}>"),
                            ("table.find", "out") => pin.ty = "array<number>".into(),
                            ("table.for_each", "key") => pin.ty = key_ty.clone(),
                            _ => {}
                        }
                    }
                }
            }
            "timeline.play" => {
                let tl = n.params.get("timeline").and_then(Value::as_str).and_then(|c| self.g.timelines.iter().find(|x| x.id == c));
                if let Some(t) = tl {
                    p.outputs.extend(t.tracks.iter().map(|tr| PinDef { id: tr.id.clone(), ty: tr.pin_type().into(), label: tr.name.clone(), required: false, optional: true, param: None }));
                }
            }
            "event.custom" | "event.call" => {
                let ce = n.params.get("event").and_then(Value::as_str).and_then(|c| self.g.custom_events.iter().find(|x| x.id == c));
                if let Some(c) = ce {
                    if def.kind == "event.custom" {
                        p.outputs.extend(c.inputs.iter().map(|m| member_pin(m, false)));
                    } else {
                        p.inputs.extend(c.inputs.iter().map(|m| member_pin(m, false)));
                    }
                }
            }
            "fn.return" => {
                if let Some(f) = self.funcs.get(home) {
                    p.inputs.extend(f.outputs.iter().map(|m| member_pin(m, false)));
                }
            }
            "fn.call" | "fn.call_pure" => {
                if let Some(f) = self.func_of(id) {
                    p.inputs.extend(f.inputs.iter().map(|m| member_pin(m, false)));
                    p.outputs.extend(f.outputs.iter().map(|m| member_pin(m, false)));
                }
            }
            "macro.entry" => {
                if let Some(m) = self.macros.get(home) {
                    p.outputs = m.inputs.iter().map(|m| member_pin(m, false)).collect();
                }
            }
            "macro.exit" => {
                if let Some(m) = self.macros.get(home) {
                    p.inputs = m.outputs.iter().map(|m| member_pin(m, false)).collect();
                }
            }
            "macro.use" => {
                if let Some(m) = self.param_str(id, "macro").and_then(|m| self.macros.get(m)) {
                    p.inputs = m.inputs.iter().map(|m| member_pin(m, true)).collect();
                    p.outputs = m.outputs.iter().map(|m| member_pin(m, false)).collect();
                }
            }
            "struct.make" => {
                if let Some(s) = self.struct_of(id) {
                    p.inputs = s.fields.iter().map(|m| member_pin(m, false)).collect();
                }
            }
            "struct.break" => {
                if let Some(s) = self.struct_of(id) {
                    p.outputs = s.fields.iter().map(|m| member_pin(m, false)).collect();
                }
            }
            "flow.switch_enum" => {
                if let Some(e) = self.enum_of(id) {
                    p.outputs = e.values.iter().enumerate().map(|(i, l)| exec_pin(&format!("v{i}"), l)).collect();
                }
            }
            "flow.switch_num" | "flow.switch_text" => {
                p.outputs = switch_cases(n, def).iter().enumerate().map(|(i, c)| exec_pin(&format!("c{i}"), c)).collect();
                p.outputs.push(exec_pin("default", "Default"));
            }
            "flow.sequence" => {
                p.outputs = (0..count_param(n, def, "count", 8).max(1)).map(|i| exec_pin(&format!("then{i}"), &format!("Then {i}"))).collect();
            }
            "array.make" => {
                let item = p.inputs.first().cloned();
                if let Some(item) = item {
                    p.inputs = (0..count_param(n, def, "count", 8)).map(|i| PinDef { id: format!("i{i}"), label: format!("[{i}]"), ..item.clone() }).collect();
                }
            }
            _ => {}
        }
        for pin in p.inputs.iter_mut().chain(p.outputs.iter_mut()) {
            pin.ty = self.resolve(n, def, &pin.ty);
        }
        Some(p)
    }

    /// A pin type with `var`, `$T`, `$K`, `$E`, `$S` and `$F` worked out from the node's
    /// settings ("any" while one isn't set or isn't right).
    fn resolve(&self, n: &Node, def: &NodeDef, ty: &str) -> String {
        if ty == "var" {
            return self.var_of(&n.id).map(|v| v.ty.clone()).unwrap_or_else(|| "any".into());
        }
        if !ty.contains('$') {
            return ty.to_string();
        }
        let p = |k: &str| param(n, def, k).and_then(Value::as_str).unwrap_or("");
        let item = p("of");
        let item = if item_type_ok(self.g, item, !ty.starts_with("map<")) { item.to_string() } else { "any".into() };
        let key = if KEY_TYPES.contains(&p("key")) { p("key") } else { "text" };
        let en = if self.enums.contains_key(p("enum")) { format!("enum:{}", p("enum")) } else { "any".into() };
        let st = if self.structs.contains_key(p("struct")) { format!("struct:{}", p("struct")) } else { "any".into() };
        let field = self.field_of(&n.id).map(|f| f.ty.clone()).unwrap_or_else(|| "any".into());
        ty.replace("$T", &item).replace("$K", key).replace("$E", &en).replace("$S", &st).replace("$F", &field)
    }

    pub fn inputs(&self, node: &str) -> &[PinDef] {
        self.pins.get(node).map_or(&[], |p| p.inputs.as_slice())
    }

    pub fn outputs(&self, node: &str) -> &[PinDef] {
        self.pins.get(node).map_or(&[], |p| p.outputs.as_slice())
    }

    pub fn input(&self, node: &str, pin: &str) -> Option<&PinDef> {
        self.inputs(node).iter().find(|p| p.id == pin)
    }

    pub fn output(&self, node: &str, pin: &str) -> Option<&PinDef> {
        self.outputs(node).iter().find(|p| p.id == pin)
    }

    /// No flow pins: its outputs are worked out where they're used.
    pub fn pure(&self, node: &str) -> bool {
        match self.defs.get(node) {
            Some(d) if d.kind == "macro.use" => !self.inputs(node).iter().chain(self.outputs(node)).any(|p| p.ty == "exec"),
            Some(d) => d.pure,
            None => false,
        }
    }

    /// Something flow comes into.
    pub fn has_exec_in(&self, node: &str) -> bool {
        self.inputs(node).iter().any(|p| p.ty == "exec")
    }

    /// Where a graph's flow starts: events, Function and Macro entries.
    pub fn is_root(&self, node: &str) -> bool {
        self.defs.get(node).is_some_and(|d| d.event)
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

    /// The type an output really carries: a reroute carries what comes into it.
    pub fn out_type(&self, node: &str, pin: &str) -> Option<String> {
        let mut seen = HashSet::new();
        let (mut node, mut pin) = (node.to_string(), pin.to_string());
        loop {
            let def = self.defs.get(node.as_str())?;
            if !def.reroute {
                return self.output(&node, &pin).map(|p| p.ty.clone());
            }
            if !seen.insert(node.clone()) {
                return None;
            }
            let e = *self.ins(&node, "in").first()?;
            node = e.from.node.clone();
            pin = e.from.pin.clone();
        }
    }

    /// The type an input wants.
    pub fn in_type(&self, node: &str, pin: &str) -> Option<String> {
        self.input(node, pin).map(|p| p.ty.clone())
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

    /// The flow outputs of a node, in order.
    pub fn exec_outs(&self, node: &str) -> Vec<String> {
        if self.defs.get(node).is_some_and(|d| d.reroute) {
            return Vec::new();
        }
        self.outputs(node).iter().filter(|p| p.ty == "exec").map(|p| p.id.clone()).collect()
    }

    /// Does flow coming into this pin go on through the node's outputs? Not into Reset, Open,
    /// Close, Toggle or Break: those only change the node.
    pub fn passes(&self, node: &str, pin: &str) -> bool {
        pin == "exec" || !self.defs.get(node).is_some_and(|d| matches!(d.kind.as_str(), "flow.do_once" | "flow.do_n" | "flow.gate" | "flow.for_loop_break" | "flow.for_each_array"))
    }

    /// The nodes flow goes on to from a node's outputs, and whether each passes on.
    fn next_nodes(&self, node: &str) -> Vec<(&'a str, bool)> {
        self.exec_outs(node).iter().filter_map(|p| self.exec_target(node, p)).map(|(t, tp)| (t, self.passes(t, tp))).collect()
    }

    /// Does `to` run after `from` (by flow wires, from any of `from`'s flow outputs)?
    pub fn runs_after(&self, from: &str, to: &str) -> bool {
        let mut stack: Vec<(&str, bool)> = self.next_nodes(from);
        let mut seen = HashSet::new();
        while let Some((cur, on)) = stack.pop() {
            if cur == to {
                return true;
            }
            if !on || !seen.insert(cur) {
                continue;
            }
            stack.extend(self.next_nodes(cur));
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
            if !self.defs.contains_key(src) {
                continue;
            }
            if !self.pure(src) {
                out.push((src, spin));
            } else if seen.insert(src) {
                for ip in self.inputs(src).iter().filter(|i| i.ty != "exec") {
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
            if !self.is_root(&n.id) {
                continue;
            }
            let mut stack = vec![(n.id.as_str(), false, true)];
            let mut seen = HashSet::new();
            while let Some((cur, timed, on)) = stack.pop() {
                let e = out.entry(cur).or_default().entry(n.id.as_str()).or_insert(false);
                *e |= timed;
                if !on || !seen.insert((cur, timed)) {
                    continue;
                }
                let kind = self.defs.get(cur).map(|d| d.kind.as_str()).unwrap_or("");
                for p in self.exec_outs(cur) {
                    if let Some((t, tp)) = self.exec_target(cur, &p) {
                        stack.push((t, timed || deferred(kind, &p), self.passes(t, tp)));
                    }
                }
            }
        }
        out
    }

    /// A type's name for people: Number, Array of Players, Loadout...
    pub fn type_label(&self, ty: &str) -> String {
        if let Some((_, key, item)) = container(ty) {
            return match key {
                Some(k) => format!("Map of {} to {}", self.type_label(k), self.type_label(item)),
                None => format!("Array of {}", self.type_label(item)),
            };
        }
        if let Some(e) = self.enum_type(ty) {
            return e.name.clone();
        }
        if let Some(s) = self.struct_type(ty) {
            return s.name.clone();
        }
        type_label(ty).to_string()
    }
}

/// Does this flow output run later (from a timer), not straight away?
pub fn deferred(kind: &str, pin: &str) -> bool {
    match kind {
        "timer.delay" | "timer.retrigger" => true,
        "timer.repeat" => pin == "tick",
        "timer.countdown" => pin == "done",
        "msg.menu" => pin != "then",
        // (the replies come later; it has no Then)
        "npc.choose" => true,
        _ => false,
    }
}

/// Nodes that wait (or run their flow later): not inside a Function.
pub fn latent(kind: &str) -> bool {
    matches!(kind, "timer.delay" | "timer.retrigger" | "timer.repeat" | "timer.countdown" | "msg.menu" | "npc.choose")
}

/// Pin ids a Function's or Macro's own pins can't have (the flow pins' ids).
const RESERVED_PINS: [&str; 4] = ["exec", "then", "in", "out"];

fn short(s: &str) -> String {
    s.chars().take(30).collect()
}

/// The members of a Function or Macro (or a struct's fields): ids, names, types.
fn check_members(g: &Graph, what: &str, owner: &str, ms: &[Member], exec_ok: bool, p: &mut Vec<Problem>) {
    if ms.len() > MAX_MEMBERS {
        p.push(Problem::err(None, format!("{what} \u{201c}{}\u{201d} has more than {MAX_MEMBERS} inputs, outputs or fields.", short(owner))));
    }
    let mut ids = HashSet::new();
    for m in ms {
        if !valid_id(&m.id) || !ids.insert(m.id.as_str()) || RESERVED_PINS.contains(&m.id.as_str()) {
            p.push(Problem::err(None, format!("{what} \u{201c}{}\u{201d} has a bad or repeated input, output or field id.", short(owner))));
        }
        if m.name.trim().is_empty() || !valid_label(&m.name, 30) {
            p.push(Problem::err(None, format!("Name each input, output and field of {what} \u{201c}{}\u{201d} (up to 30 letters, digits, spaces or plain punctuation).", short(owner))));
        }
        if !(type_ok(g, &m.ty) || (exec_ok && m.ty == "exec")) {
            p.push(Problem::err(None, format!("{what} \u{201c}{}\u{201d}: \u{201c}{}\u{201d} has a type this app doesn't know.", short(owner), short(&m.name))));
        }
    }
}

/// A variable (or local): its id is checked by the caller.
fn check_variable(g: &Graph, x: &Variable, local: bool, p: &mut Vec<Problem>) {
    let name = short(&x.name);
    if x.name.trim().is_empty() || !valid_label(&x.name, 30) {
        p.push(Problem::err(None, "Give each variable a name (up to 30 letters, digits, spaces or plain punctuation)."));
    }
    let complex = container(&x.ty).is_some() || x.ty.starts_with("struct:");
    if !type_ok(g, &x.ty) || !["mode", "player", "team"].contains(&x.scope.as_str()) || !["never", "round"].contains(&x.reset.as_str()) {
        p.push(Problem::err(None, format!("The variable \u{201c}{name}\u{201d} has a type or scope this app doesn't know.")));
        return;
    }
    if complex && x.scope != "mode" {
        p.push(Problem::err(None, format!("The variable \u{201c}{name}\u{201d}: arrays, maps and structs are kept for the whole mode (not per player or team).")));
    }
    if local && (x.scope != "mode" || x.reset != "never") {
        p.push(Problem::err(None, format!("The local variable \u{201c}{name}\u{201d} is the Function's own: not per player, team or round.")));
    }
    if !x.default.is_null() && !default_fits(g, &x.ty, &x.default) {
        p.push(Problem::err(None, format!("The variable \u{201c}{name}\u{201d} starts with a value that doesn't fit its type.")));
    }
}

/// A start value: numbers, yes/no, text, and an enum's value by its number; the other types
/// start empty (nobody, no items, 0 0 0).
pub fn default_fits(g: &Graph, ty: &str, v: &Value) -> bool {
    if let Some(("enum", id)) = named_type(ty) {
        let n = g.enums.iter().find(|e| e.id == id).map_or(0, |e| e.values.len());
        return v.as_f64().is_some_and(|x| x.fract() == 0.0 && x >= 0.0 && (x as usize) < n);
    }
    value_fits(ty, v)
}

/// Every problem with a graph: errors stop a compile, warnings are shown on the nodes.
pub fn validate(g: &Graph) -> Vec<Problem> {
    let mut p = event_graph_problems(g);
    let flat = flatten(g);
    let g = flat.as_ref();
    p.extend(validate_core(g));
    advanced_problems(g, &View::new(g), &mut p);
    if !has_errors(&p) {
        p.extend(cost_warnings(g, &View::new(g)));
    }
    p.extend(nothing_happens(g));
    p.extend(bot_awareness(g, &View::new(g)));
    p
}

/// The start of the note about something bots could use but aren't told about (the Studio
/// offers its fix and "Don't warn for this one" on it).
pub const BOTS_NOTE: &str = "Bots won't know about";

/// Things a mode makes that bots could act on, that nothing tells them about: a warning each
/// (never an error), so a mode where bots play along isn't missed. What gm_components already
/// tells bots by itself isn't flagged: capture points, extraction zones, checkpoints, loot
/// containers, supply drops, perk machines, upgrade benches, mystery boxes and traps. Shops aren't
/// either: a bot can't pick from a shop's menu, so an offer wouldn't help. Flagged: an
/// interactable bots may not use (Spawn Interactable with Bots may use off, or Create Component
/// without bots=1), and an NPC a mode talks through (Talked To NPC in the mode) with no Bots: Goal
/// or Bots: Offer at it. A node marked "Don't warn for this one" (quiet) is left alone.
pub fn bot_awareness(g: &Graph, v: &View) -> Vec<Problem> {
    let talks = g.nodes.iter().any(|n| n.kind == "npc.on_talk");
    // what a Bots: Goal or Bots: Offer points at (the node whose output is wired into At)
    let told: HashSet<&str> = g.nodes.iter().filter(|n| n.kind == "bots.goal" || n.kind == "bots.offer").filter_map(|n| v.data_source(&n.id, "at").map(|(from, _)| from)).collect();
    let text = |n: &Node, k: &str| n.params.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let key = |keys: &str, k: &str| keys.split_whitespace().find_map(|kv| kv.strip_prefix(&format!("{k}=")).map(str::to_string));
    let mut p = Vec::new();
    for n in g.nodes.iter().filter(|n| !n.quiet && !told.contains(n.id.as_str())) {
        let name = match n.kind.as_str() {
            "npc.spawn_use" if !n.params.get("bots").and_then(Value::as_bool).unwrap_or(false) => {
                let prompt = text(n, "prompt");
                if prompt.is_empty() { "this interactable".to_string() } else { format!("the interactable \u{201c}{prompt}\u{201d}") }
            }
            "npc.spawn" if talks => {
                let name = text(n, "name");
                if name.is_empty() { "this NPC".to_string() } else { format!("\u{201c}{name}\u{201d}") }
            }
            "comp.create" => {
                let keys = text(n, "keys");
                let named = key(&keys, "name").map(|x| format!("\u{201c}{x}\u{201d}"));
                match text(n, "kind").as_str() {
                    "ls_interactable" if key(&keys, "bots").as_deref() != Some("1") => named.unwrap_or_else(|| "this interactable".into()),
                    "ls_npc" if talks => named.unwrap_or_else(|| "this NPC".into()),
                    _ => continue,
                }
            }
            _ => continue,
        };
        p.push(Problem::warn(Some(&n.id), format!("{BOTS_NOTE} {name}. Add an Offer to bots or a Bot goal.")));
    }
    p
}

/// A mode that does nothing yet: no event at all (and no game settings, which are a mode by
/// themselves), or events with nothing wired after them (UX review H7).
fn nothing_happens(g: &Graph) -> Option<Problem> {
    let reg = registry();
    // (a Function's or Macro's entry isn't something happening in the game)
    let events: Vec<&Node> = g.nodes.iter().filter(|n| n.kind != "fn.entry" && n.kind != "macro.entry" && reg.get(&n.kind).is_some_and(|d| d.event)).collect();
    if events.is_empty() {
        // (with other nodes but no event, "Nothing starts this graph yet" says it already)
        let only_entries = g.nodes.iter().all(|n| n.kind == "fn.entry" || n.kind == "macro.entry" || n.kind == "fn.return" || n.kind == "macro.exit");
        return (only_entries && g.mode.settings.is_empty()).then(|| Problem::warn(None, "Nothing happens yet: add an event (right-click the Event Graph: Round Started, Player Killed...) and wire what it does after it."));
    }
    let wired = events.iter().any(|n| g.edges.iter().any(|e| e.from.node == n.id));
    (!wired).then(|| Problem::warn(Some(&events[0].id), "Nothing happens yet: none of the events has anything wired after it."))
}

/// The extra event graphs: good ids and names, no wire from one graph to another, and a note
/// when the same event is on more than one (they all run, in tab order).
fn event_graph_problems(g: &Graph) -> Vec<Problem> {
    let mut p = Vec::new();
    if g.event_graphs.is_empty() {
        return p;
    }
    if g.event_graphs.len() > 16 {
        p.push(Problem::err(None, "A mode has at most 16 event graphs."));
    }
    let mut seen = HashSet::new();
    let parts: HashSet<&str> = g.functions.iter().map(|f| f.id.as_str()).chain(g.macros.iter().map(|m| m.id.as_str())).collect();
    for e in &g.event_graphs {
        if !valid_id(&e.id) || !seen.insert(e.id.as_str()) || parts.contains(e.id.as_str()) {
            p.push(Problem::err(None, "An event graph has a bad or repeated id."));
        }
        if e.name.trim().is_empty() || !valid_label(&e.name, 30) {
            p.push(Problem::err(None, "Give each event graph a name (up to 30 letters, digits, spaces or plain punctuation)."));
        }
    }
    let home: HashMap<&str, &str> = g.nodes.iter().map(|n| (n.id.as_str(), n.graph.as_deref().unwrap_or(""))).collect();
    for e in &g.edges {
        if let (Some(a), Some(b)) = (home.get(e.from.node.as_str()), home.get(e.to.node.as_str())) {
            if a != b {
                p.push(Problem { level: "error", node: None, pin: None, edge: Some(e.id.clone()), message: "A wire can't go from one graph to another.".into() });
            }
        }
    }
    let name = |h: &str| if h.is_empty() { "Event Graph".to_string() } else { g.event_graphs.iter().find(|e| e.id == h).map(|e| e.name.clone()).unwrap_or_default() };
    let reg = registry();
    let mut by_kind: Vec<(&str, Vec<&str>)> = Vec::new();
    for n in &g.nodes {
        let h = n.graph.as_deref().unwrap_or("");
        let in_event_graph = h.is_empty() || g.event_graphs.iter().any(|e| e.id == h);
        // (named events, custom events and chat commands differ by their settings: not doubles)
        if !in_event_graph || !reg.get(&n.kind).is_some_and(|d| d.event && d.params.is_empty()) {
            continue;
        }
        match by_kind.iter_mut().find(|(k, _)| *k == n.kind) {
            Some((_, hs)) => {
                if !hs.contains(&h) {
                    hs.push(h)
                }
            }
            None => by_kind.push((n.kind.as_str(), vec![h])),
        }
    }
    for (k, mut hs) in by_kind.into_iter().filter(|(_, hs)| hs.len() > 1) {
        hs.sort_by_key(|h| if h.is_empty() { 0 } else { g.event_graphs.iter().position(|e| e.id == *h).map(|i| i + 1).unwrap_or(99) });
        let title = reg.get(k).map(|d| d.title.clone()).unwrap_or_default();
        let names: Vec<String> = hs.iter().map(|h| name(h)).collect();
        p.push(Problem { level: "warning", node: g.nodes.iter().find(|n| n.kind == k && n.graph.as_deref().unwrap_or("") == hs[1]).map(|n| n.id.clone()), pin: None, edge: None, message: format!("{title} is on {}: they all run, in that order (the tabs' order, then top to bottom).", names.join(", ")) });
    }
    p
}

/// The checks without the cost advice (tests of graphs that loop on purpose use this).
pub fn validate_core(g: &Graph) -> Vec<Problem> {
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
    if !g.mode.uid.is_empty() && !valid_uid(&g.mode.uid) {
        p.push(Problem::err(None, "The mode's uid isn't a UUID this app knows."));
    }
    // the data tables' own checks (cells, keys, the size limits)
    p.extend(super::tables::validate(&g.data_tables, &g.enums.iter().map(|e| (e.id.clone(), e.values.len())).collect::<Vec<_>>()));
    if !valid_label(&g.mode.description, 60) {
        p.push(Problem::err(None, "The mode's description can be up to 60 letters, digits, spaces or plain punctuation (no * / \\ \" ^)."));
    }
    let allowed = allowed_settings();
    for st in &g.mode.settings {
        if !allowed.contains(&st.cvar.as_str()) || !st.value.is_finite() || !(0.0..=NUMBER_LIMIT).contains(&st.value) {
            p.push(Problem::err(None, format!("The mode's setting \u{201c}{}\u{201d} isn't one the Studio can set (or its value is out of range).", short(&st.cvar))));
        }
    }
    let mut n_settings = 0;
    for x in g.variables.iter().filter(|x| x.setting.is_some()) {
        let st = x.setting.as_ref().unwrap();
        n_settings += 1;
        let name = short(&x.name);
        if x.scope != "mode" || !matches!(x.ty.as_str(), "number" | "bool") {
            p.push(Problem::err(None, format!("\u{201c}{name}\u{201d} can't be a mode setting: only a whole-mode number or yes/no can.")));
            continue;
        }
        if st.label.trim().is_empty() || !valid_label(&st.label, 30) || !valid_label(&st.help, 120) || !valid_label(&st.group, 20) {
            p.push(Problem::err(None, format!("The setting \u{201c}{name}\u{201d} needs a label (30), and its help (120) and group (20) plain text.")));
        }
        if st.choices.len() > 12 || st.choices.iter().any(|c| c.trim().is_empty() || !valid_label(c, 30)) || (!st.choices.is_empty() && x.ty != "number") {
            p.push(Problem::err(None, format!("The setting \u{201c}{name}\u{201d} has up to 12 named choices (30 letters each), for a number.")));
        }
        let (lo, hi) = st.range(&x.ty);
        let in_range = |v: f64| v.is_finite() && v.abs() <= NUMBER_LIMIT;
        if !in_range(st.min) || !in_range(st.max) || lo >= hi || !in_range(st.step) || st.step < 0.0 {
            p.push(Problem::err(None, format!("The setting \u{201c}{name}\u{201d} needs its smallest below its largest (and a step of 0 or more).")));
        }
        let d = match &x.default {
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Number(n) => n.as_f64(),
            Value::Null => Some(0.0),
            _ => None,
        };
        if d.is_none_or(|d| d < lo || d > hi) {
            p.push(Problem::err(None, format!("The setting \u{201c}{name}\u{201d} starts at a value outside its range.")));
        }
        if setting_cvar(&g.mode.id, &x.id).len() > 60 {
            p.push(Problem::err(None, format!("The setting \u{201c}{name}\u{201d}'s name is too long: give the variable a shorter id.")));
        }
    }
    if n_settings > 32 {
        p.push(Problem::err(None, "A mode has at most 32 settings hosts can change."));
    }
    if g.mode.settings.len() > 32 {
        p.push(Problem::err(None, "A mode can have at most 32 settings."));
    }
    if g.nodes.len() > MAX_NODES || g.edges.len() > MAX_EDGES || g.comments.len() > MAX_COMMENTS || g.variables.len() > MAX_VARIABLES {
        p.push(Problem::err(None, format!("That graph is too big (at most {MAX_NODES} nodes, {MAX_EDGES} wires, {MAX_COMMENTS} comments and {MAX_VARIABLES} variables).")));
        return p;
    }
    if g.functions.len() > MAX_FUNCTIONS || g.macros.len() > MAX_MACROS || g.enums.len() > MAX_ENUMS || g.structs.len() > MAX_STRUCTS {
        p.push(Problem::err(None, format!("That graph has too many parts (at most {MAX_FUNCTIONS} Functions, {MAX_MACROS} Macros, {MAX_ENUMS} enums and {MAX_STRUCTS} structs).")));
        return p;
    }

    // enums and structs
    let mut tids = HashSet::new();
    for e in &g.enums {
        if !valid_id(&e.id) || !tids.insert(("enum", e.id.as_str())) {
            p.push(Problem::err(None, "An enum has a bad or repeated id."));
        }
        if e.name.trim().is_empty() || !valid_label(&e.name, 30) {
            p.push(Problem::err(None, "Give each enum a name (up to 30 letters, digits, spaces or plain punctuation)."));
        }
        if e.values.is_empty() || e.values.len() > 32 || e.values.iter().any(|x| x.trim().is_empty() || !valid_label(x, 30)) {
            p.push(Problem::err(None, format!("The enum \u{201c}{}\u{201d} needs 1 to 32 values, each a name of up to 30 letters, digits, spaces or plain punctuation.", short(&e.name))));
        }
    }
    for s in &g.structs {
        if !valid_id(&s.id) || !tids.insert(("struct", s.id.as_str())) {
            p.push(Problem::err(None, "A struct has a bad or repeated id."));
        }
        if s.name.trim().is_empty() || !valid_label(&s.name, 30) {
            p.push(Problem::err(None, "Give each struct a name (up to 30 letters, digits, spaces or plain punctuation)."));
        }
        if s.fields.is_empty() {
            p.push(Problem::err(None, format!("The struct \u{201c}{}\u{201d} needs at least one field.", short(&s.name))));
        }
        check_members(g, "The struct", &s.name, &s.fields, false, &mut p);
        if s.fields.iter().any(|f| !item_type_ok(g, &f.ty, false)) {
            p.push(Problem::err(None, format!("The struct \u{201c}{}\u{201d}: a field can be a number, yes/no, text, player, entity, team, vector, timer or enum (not an array, map or struct).", short(&s.name))));
        }
        if s.fields.iter().filter(|f| f.ty == "text").count() > MAX_TEXT_FIELDS {
            p.push(Problem::err(None, format!("The struct \u{201c}{}\u{201d} can have at most {MAX_TEXT_FIELDS} text fields.", short(&s.name))));
        }
    }

    for m in marker_problems(g) {
        p.push(Problem::err(None, m));
    }

    // library marks: an item id and a version
    let froms = g.enums.iter().map(|x| &x.from).chain(g.structs.iter().map(|x| &x.from)).chain(g.functions.iter().map(|x| &x.from)).chain(g.macros.iter().map(|x| &x.from));
    if froms.flatten().any(|f| !valid_mode_id(&f.lib) || f.version > 100_000) {
        p.push(Problem::err(None, "A part's library mark isn't one this app knows."));
    }

    // Functions and Macros: their graphs share one set of ids
    let mut gids = HashSet::new();
    let mut fnames = HashSet::new();
    timeline_problems(g, &mut p);
    let mut cnames = HashSet::new();
    for c in &g.custom_events {
        if !valid_id(&c.id) || !gids.insert(c.id.as_str()) {
            p.push(Problem::err(None, "A Custom Event has a bad or repeated id."));
        }
        if c.name.trim().is_empty() || !valid_label(&c.name, 26) || !cnames.insert(c.map_name()) {
            p.push(Problem::err(None, format!("Give each Custom Event its own name (up to 26 letters, digits, spaces or plain punctuation): \u{201c}{}\u{201d}.", short(&c.name))));
        }
        check_members(g, "The Custom Event", &c.name, &c.inputs, false, &mut p);
        if c.inputs.len() > 8 {
            p.push(Problem::err(None, format!("The Custom Event \u{201c}{}\u{201d} has more than 8 inputs.", short(&c.name))));
        }
        if c.map && c.inputs.iter().any(|m| !["player", "number", "text", "bool"].contains(&m.ty.as_str())) {
            p.push(Problem::err(None, format!("The map can fire \u{201c}{}\u{201d} only with Player, number, text and yes/no inputs.", short(&c.name))));
        }
    }
    if g.custom_events.len() > 32 {
        p.push(Problem::err(None, "A mode has at most 32 Custom Events."));
    }
    for f in &g.functions {
        if !valid_id(&f.id) || !gids.insert(f.id.as_str()) {
            p.push(Problem::err(None, "A Function has a bad or repeated id."));
        }
        if f.name.trim().is_empty() || !valid_label(&f.name, 30) || !fnames.insert(f.name.trim().to_lowercase()) {
            p.push(Problem::err(None, format!("Give each Function its own name (up to 30 letters, digits, spaces or plain punctuation): \u{201c}{}\u{201d}.", short(&f.name))));
        }
        check_members(g, "The Function", &f.name, &f.inputs, false, &mut p);
        check_members(g, "The Function", &f.name, &f.outputs, false, &mut p);
        if f.pure && f.outputs.is_empty() {
            p.push(Problem::warn(None, format!("The pure Function \u{201c}{}\u{201d} has no outputs, so nothing ever runs it.", short(&f.name))));
        }
        if f.locals.len() > 16 {
            p.push(Problem::err(None, format!("The Function \u{201c}{}\u{201d} has more than 16 local variables.", short(&f.name))));
        }
    }
    for m in &g.macros {
        if !valid_id(&m.id) || !gids.insert(m.id.as_str()) {
            p.push(Problem::err(None, "A Macro has a bad or repeated id."));
        }
        if m.name.trim().is_empty() || !valid_label(&m.name, 30) || !fnames.insert(m.name.trim().to_lowercase()) {
            p.push(Problem::err(None, format!("Give each Macro its own name (up to 30 letters, digits, spaces or plain punctuation): \u{201c}{}\u{201d}.", short(&m.name))));
        }
        check_members(g, "The Macro", &m.name, &m.inputs, true, &mut p);
        check_members(g, "The Macro", &m.name, &m.outputs, true, &mut p);
    }

    // variables (and locals)
    let mut vids = HashSet::new();
    for x in &g.variables {
        if !valid_id(&x.id) || !vids.insert(x.id.as_str()) {
            p.push(Problem::err(None, "A variable has a bad or repeated id."));
            continue;
        }
        check_variable(g, x, false, &mut p);
    }
    for f in &g.functions {
        for x in &f.locals {
            if !valid_id(&x.id) || !vids.insert(x.id.as_str()) {
                p.push(Problem::err(None, format!("A local variable of \u{201c}{}\u{201d} has a bad or repeated id.", short(&f.name))));
                continue;
            }
            check_variable(g, x, true, &mut p);
        }
    }
    if g.variables.len() + g.functions.iter().map(|f| f.locals.len()).sum::<usize>() > MAX_VARIABLES * 2 {
        p.push(Problem::err(None, "That graph has too many variables and locals."));
    }

    // nodes and their params
    let v = View::new(g);
    let mut ids = HashSet::new();
    let mut entries: HashMap<&str, usize> = HashMap::new();
    let mut exits: HashMap<&str, usize> = HashMap::new();
    for n in &g.nodes {
        if !valid_id(&n.id) {
            p.push(Problem::err(None, format!("A node has a bad id ({:?}).", short(&n.id))));
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
        // where it is
        let home = n.graph.as_deref().unwrap_or("");
        let in_fn = v.funcs.contains_key(home);
        let in_macro = v.macros.contains_key(home);
        if !home.is_empty() && !in_fn && !in_macro {
            p.push(Problem::err(Some(&n.id), "This node is on the graph of a Function or Macro that isn't there."));
            continue;
        }
        match def.kind.as_str() {
            "fn.entry" | "fn.return" if !in_fn => p.push(Problem::err(Some(&n.id), format!("{} only goes in a Function's graph.", def.title))),
            "macro.entry" | "macro.exit" if !in_macro => p.push(Problem::err(Some(&n.id), format!("{} only goes in a Macro's graph.", def.title))),
            "fn.entry" | "macro.entry" => *entries.entry(home).or_default() += 1,
            "macro.exit" => *exits.entry(home).or_default() += 1,
            k if def.event && !home.is_empty() => p.push(Problem::err(Some(&n.id), format!("{} goes on the Event Graph, not in a Function or Macro.", reg.get(k).map_or("An event", |d| d.title.as_str())))),
            k if in_fn && latent(k) => p.push(Problem::err(Some(&n.id), format!("{} waits, and a Function runs straight through: use it on the Event Graph or in a Macro.", def.title))),
            _ => {}
        }
        for k in n.params.keys() {
            if def.param(k).is_none() {
                p.push(Problem::warn(Some(&n.id), format!("\u{201c}{}\u{201d} isn't a setting of {}; it's ignored.", short(k), def.title)));
            }
        }
        for prm in &def.params {
            let val = n.params.get(&prm.id);
            let sval = val.and_then(Value::as_str);
            let res = match prm.kind.as_str() {
                "var" => {
                    let chosen = sval.and_then(|id| v.vars.get(id));
                    match chosen {
                        None => Err(format!("Pick a variable for {} (add one under My mode).", def.title)),
                        Some(x) if prm.var_type.as_deref().is_some_and(|t| !(t == x.ty || (t == "struct" && x.ty.starts_with("struct:")) || (t == "array" && x.ty.starts_with("array<")))) => {
                            Err(format!("{} needs a {} variable.", def.title, prm.var_type.as_deref().unwrap_or("")))
                        }
                        Some(x) if v.local_of.get(x.id.as_str()).is_some_and(|f| *f != home) => Err(format!("\u{201c}{}\u{201d} is a local variable of another Function.", short(&x.name))),
                        _ => Ok(()),
                    }
                }
                "func" => match sval.and_then(|id| v.defs.get(id).map(|d| d.kind.as_str())) {
                    Some("func.define") => Ok(()),
                    _ => Err("Pick which Function to call (add a Function node first).".into()),
                },
                "fn" => match sval.and_then(|id| v.funcs.get(id)) {
                    None => Err("Pick which Function to call.".into()),
                    Some(f) if def.kind == "fn.call_pure" && !f.pure => Err(format!("\u{201c}{}\u{201d} isn't a pure Function: call it with flow.", short(&f.name))),
                    Some(f) if def.kind != "fn.call_pure" && f.pure => Err(format!("\u{201c}{}\u{201d} is a pure Function: it has no flow pins.", short(&f.name))),
                    _ => Ok(()),
                },
                "macro" => match sval.and_then(|id| v.macros.get(id)) {
                    None => Err("Pick which Macro to use.".into()),
                    _ => Ok(()),
                },
                "table" => match sval.and_then(|id| g.data_tables.iter().find(|t| t.id == id)) {
                    None => Err(format!("Pick a data table for {} (add one under My mode, Data tables).", def.title)),
                    _ => Ok(()),
                },
                "column" => match super::tables::table_of(&g.data_tables, n).map(|(_, t)| t.col(sval.unwrap_or("")).is_some()) {
                    Some(false) => Err(format!("Pick a column for {}.", def.title)),
                    _ => Ok(()),
                },
                "tline" => match sval.and_then(|id| g.timelines.iter().find(|t| t.id == id)) {
                    None => Err(format!("Pick a Timeline for {} (add one under My mode, Timelines).", def.title)),
                    _ => Ok(()),
                },
                "cevent" => match sval.and_then(|id| g.custom_events.iter().find(|c| c.id == id)) {
                    None => Err(format!("Pick a Custom Event for {} (add one under My mode, Custom events).", def.title)),
                    Some(c) if def.kind == "timer.by_event" && !c.inputs.is_empty() => Err(format!("\u{201c}{}\u{201d} has inputs: a timer can only run a Custom Event without any (call it from a Delay instead).", short(&c.name))),
                    _ => Ok(()),
                },
                "marker" => match sval {
                    Some(id) if g.markers.iter().any(|m| m.id == id) => Ok(()),
                    _ => Err(format!("Pick a map marker for {} (add one under Map markers).", def.title)),
                },
                "struct" => match sval.and_then(|id| v.structs.get(id)) {
                    None => Err(format!("Pick a struct for {} (add one under My mode).", def.title)),
                    _ => Ok(()),
                },
                "enum" => match sval.and_then(|id| v.enums.get(id)) {
                    None => Err(format!("Pick an enum for {} (add one under My mode).", def.title)),
                    _ => Ok(()),
                },
                "type" => match sval {
                    Some(t) if item_type_ok(g, t, !def.kind.starts_with("map.")) => Ok(()),
                    _ => Err(format!("{}: pick what kind of value it holds.", def.title)),
                },
                "field" => match v.field_of(&n.id) {
                    None => Err(format!("Pick which field {} sets.", def.title)),
                    _ => Ok(()),
                },
                "enumvalue" => {
                    let count = v.enum_of(&n.id).map_or(0, |e| e.values.len());
                    match param(n, def, &prm.id).and_then(Value::as_f64) {
                        Some(x) if x.fract() == 0.0 && x >= 0.0 && (x as usize) < count => Ok(()),
                        _ => Err(format!("Pick one of the enum's values for {}.", def.title)),
                    }
                }
                "timer" => match sval.and_then(|id| v.defs.get(id).map(|d| d.kind.as_str())) {
                    Some("timer.delay" | "timer.repeat" | "timer.countdown" | "timer.retrigger") => Ok(()),
                    _ => Err("Pick which timer to stop.".into()),
                },
                "varvalue" => match (val, v.var_of(&n.id)) {
                    (Some(x), Some(var)) if !(x.is_string() && x.as_str() == Some("")) && !default_fits(g, &var.ty, x) => Err(format!("{} doesn't fit the variable \u{201c}{}\u{201d}.", prm.label, short(&var.name))),
                    _ => Ok(()),
                },
                "cases" => match sval {
                    Some(t) if valid_label(t, 120) && (def.kind != "flow.switch_num" || t.split(',').map(str::trim).filter(|c| !c.is_empty()).all(|c| c.parse::<f64>().is_ok_and(|x| x.is_finite() && x.abs() <= NUMBER_LIMIT))) => Ok(()),
                    None => Ok(()),
                    _ => Err(format!("{}: the cases are {} split by commas.", def.title, if def.kind == "flow.switch_num" { "numbers" } else { "words" })),
                },
                _ => val.map_or(Ok(()), |x| check_param(prm, x)),
            };
            if let Err(why) = res {
                p.push(Problem::err(Some(&n.id), why).pin(&prm.id));
            }
        }
        // a Macro can't be used inside itself
        if def.kind == "macro.use" && v.param_str(&n.id, "macro") == Some(home) && in_macro {
            p.push(Problem::err(Some(&n.id), "A Macro can't use itself."));
        }
    }
    for f in &g.functions {
        match entries.get(f.id.as_str()) {
            Some(1) => {}
            Some(_) => p.push(Problem::err(None, format!("The Function \u{201c}{}\u{201d} has more than one Function Entry.", short(&f.name)))),
            None => p.push(Problem::err(None, format!("The Function \u{201c}{}\u{201d} needs its Function Entry node (open its tab).", short(&f.name)))),
        }
    }
    for m in &g.macros {
        if entries.get(m.id.as_str()) != Some(&1) {
            p.push(Problem::err(None, format!("The Macro \u{201c}{}\u{201d} needs exactly one Macro Entry node.", short(&m.name))));
        }
        if exits.get(m.id.as_str()).copied().unwrap_or(0) > 1 || (!m.outputs.is_empty() && exits.get(m.id.as_str()).is_none()) {
            p.push(Problem::err(None, format!("The Macro \u{201c}{}\u{201d} needs one Macro Exit node (for its outputs).", short(&m.name))));
        }
    }
    if let Some(m) = find_macro_cycle(&v) {
        p.push(Problem::err(None, format!("The Macro \u{201c}{}\u{201d} ends up using itself.", short(&m))));
    }
    let mut cids = HashSet::new();
    for c in &g.comments {
        if !valid_id(&c.id) || !cids.insert(c.id.as_str()) {
            p.push(Problem::err(None, "A comment has a bad or repeated id."));
        }
        if c.color.as_deref().is_some_and(|k| !(k.len() == 7 && k.starts_with('#') && k[1..].chars().all(|h| h.is_ascii_hexdigit()))) {
            p.push(Problem::err(None, "A comment's color isn't a color (#rrggbb)."));
        }
        if !valid_note(&c.text) {
            p.push(Problem::err(None, "A comment is too long (2000 characters at most) or uses a character that isn't allowed (* / \\ \" ^)."));
        }
        if c.graph.as_deref().is_some_and(|h| !gids.contains(h)) {
            p.push(Problem::err(None, "A comment is on the graph of a Function or Macro that isn't there."));
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
        if v.graph_of(&e.from.node) != v.graph_of(&e.to.node) {
            p.push(Problem::err(Some(&e.to.node), "A wire can't go from one graph to another.").edge(&e.id));
            continue;
        }
        if v.output(&e.from.node, &e.from.pin).is_none() {
            p.push(Problem::err(Some(&e.from.node), format!("{} has no output \u{201c}{}\u{201d}.", fd.title, short(&e.from.pin))).edge(&e.id));
            continue;
        }
        let Some(tp) = v.input(&e.to.node, &e.to.pin) else {
            p.push(Problem::err(Some(&e.to.node), format!("{} has no input \u{201c}{}\u{201d}.", td.title, short(&e.to.pin))).edge(&e.id));
            continue;
        };
        if e.from.node == e.to.node {
            p.push(Problem::err(Some(&e.to.node), "A wire can't go back into the same node.").edge(&e.id));
            continue;
        }
        let from_ty = v.out_type(&e.from.node, &e.from.pin).unwrap_or_else(|| "any".into());
        let to_ty = if td.reroute { "any".to_string() } else { tp.ty.clone() };
        if !compatible(&from_ty, &to_ty) {
            let (fl, tl) = (v.type_label(&from_ty), v.type_label(&to_ty));
            p.push(Problem::err(Some(&e.to.node), format!("{fl} can't go into {} (a {fl} wire into a {tl} pin).", pin_label(td, tp))).pin(&tp.id).edge(&e.id));
        }
    }
    // one wire per flow output and per data input
    for ((node, pin), es) in &v.out_edges {
        if es.len() > 1 && v.out_type(node, pin).as_deref() == Some("exec") {
            p.push(Problem::err(Some(node), "A flow output can only go to one place. Add the steps one after another instead (or a Sequence).").pin(pin));
        }
    }
    for ((node, pin), es) in &v.in_edges {
        let Some(def) = v.defs.get(node.as_str()) else { continue };
        let data_in = v.input(node, pin).is_some_and(|ip| ip.ty != "exec");
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
        for ip in v.inputs(&n.id) {
            if ip.ty == "exec" {
                continue;
            }
            let wired = !v.ins(&n.id, &ip.id).is_empty();
            if ip.required && !wired {
                p.push(Problem::err(Some(&n.id), format!("{} needs something wired into {}.", def.title, pin_label(def, ip))).pin(&ip.id));
            }
        }
        // Set Variable of a type without a written value: the value has to be wired
        if def.kind == "var.set" {
            if let Some(var) = v.var_of(&n.id) {
                let literal = matches!(var.ty.as_str(), "number" | "bool" | "text") || var.ty.starts_with("enum:");
                if !literal && v.ins(&n.id, "value").is_empty() {
                    p.push(Problem::err(Some(&n.id), format!("Wire in the value to put in \u{201c}{}\u{201d}.", short(&var.name))).pin("value"));
                }
            }
        }
        if let (Some(var), Some(_)) = (v.var_of(&n.id), v.input(&n.id, "player")) {
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

    // (old) Function nodes: names are unique; no Function can end up calling itself
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
    if !g.nodes.iter().any(|n| v.defs.get(n.id.as_str()).is_some_and(|d| d.event && d.kind != "fn.entry" && d.kind != "macro.entry")) && !g.nodes.is_empty() {
        p.push(Problem::warn(None, "Nothing starts this graph yet: add an event, like Round Started."));
    }
    if let Some(node) = find_cycle(&v) {
        p.push(Problem::err(Some(node), "The flow loops back to here. Wires can't go round in a circle: use a For Loop, While Loop or Repeat."));
    }
    if let Some(node) = find_data_cycle(&v) {
        p.push(Problem::err(Some(node), "This value depends on itself."));
    }
    let reached = v.reached_from();
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        if def.event || def.reroute || v.pure(&n.id) {
            continue;
        }
        if v.has_exec_in(&n.id) && !reached.contains_key(n.id.as_str()) {
            p.push(Problem::warn(Some(&n.id), "Nothing runs this node: wire an event's flow into it."));
        }
        let Some(events) = reached.get(n.id.as_str()) else { continue };
        // damage can only be changed while it's about to happen
        if def.kind.starts_with("dmg.") && events.iter().any(|(ev, timed)| *timed || v.defs.get(ev).map(|d| d.kind.as_str()) != Some("event.player_about_hurt")) {
            p.push(Problem::err(Some(&n.id), format!("{} only works in a Player About To Be Hurt flow, before any Delay, Repeat or Countdown.", def.title)));
        }
        for ip in v.inputs(&n.id).iter().filter(|i| i.ty != "exec") {
            for (src, spin) in v.flow_values(&n.id, &ip.id) {
                let Some(sdef) = v.defs.get(src) else { continue };
                let label = v.output(src, spin).map(|o| o.label.clone()).unwrap_or_default();
                if !sdef.event {
                    // a loop's or a pick's value, or what an action made: only after it has run
                    if !v.runs_after(src, &n.id) {
                        p.push(Problem::err(Some(&n.id), format!("{label} is only known after {} runs: wire the flow through it first.", sdef.title)).pin(&ip.id));
                    }
                    continue;
                }
                if events.len() != 1 || !events.contains_key(src) {
                    p.push(Problem::err(Some(&n.id), format!("{label} is only known while {} runs, but this node also runs from another event.", sdef.title)).pin(&ip.id));
                } else if events[src] && v.output(src, spin).is_some_and(|o| short_lived(&o.ty)) {
                    p.push(Problem::err(Some(&n.id), format!("{label} ({}) doesn't last through a Delay, Repeat or Countdown: keep it in a variable first.", v.type_label(&v.output(src, spin).map(|o| o.ty.clone()).unwrap_or_default()))).pin(&ip.id));
                }
            }
        }
    }
    p
}

/// The commands a mode may send (Run Server Command and Run AMX Command). One list for the app's
/// check (`cmd_refused`) and the plugin's (`lsg_safe_cmd`, generated from these by
/// `gen::safe_cmd_stock`), so the two can't drift apart.
///
/// The engine splits a command into words on spaces and also on `{ } ( ) ' : ,` and drops quotes;
/// a mode's command may hold none of those (nor `;`, control characters or `"`), so the first
/// word the check sees is the one the engine runs.
///
/// Run Server Command: an allowlist of first words. AMX Mod X commands go through Run AMX
/// Command only, which has its own list; amx_cvar may only set what Run Server Command may.
pub mod cmd_rules {
    /// the characters a command may have (one space between words: no tabs or other whitespace)
    pub fn allowed_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || " _.-+!?@#".contains(c)
    }
    /// the longest first or second word (the plugin's buffers hold one more, the end): a longer
    /// one is refused, so both checks look at the same whole words
    pub const WORD_MAX: usize = 47;
    /// game rules: any mp_ setting not named below as off limits
    pub const SERVER_PREFIXES: [&str; 1] = ["mp_"];
    /// and these exactly: gameplay sv_ settings, bot settings and commands, our plugins' gameplay
    /// commands, say and humans_join_team (bot_nav_*, ls_comp_save/del/reset and the like are not)
    pub const SERVER_WORDS: [&str; 51] = [
        "say", "humans_join_team",
        "sv_gravity", "sv_airaccelerate", "sv_accelerate", "sv_maxspeed", "sv_friction", "sv_stopspeed", "sv_alltalk", "sv_maxvelocity", "sv_stepsize", "sv_wateraccelerate", "sv_waterfriction", "sv_bounce", "sv_restart", "sv_restartround", "sv_voiceenable",
        "bot_quota", "bot_quota_mode", "bot_difficulty", "bot_add", "bot_add_t", "bot_add_ct", "bot_kick", "bot_kill", "bot_knives_only", "bot_pistols_only", "bot_snipers_only", "bot_all_weapons",
        "bot_allow_rogues", "bot_allow_pistols", "bot_allow_shotguns", "bot_allow_sub_machine_guns", "bot_allow_rifles", "bot_allow_machine_guns", "bot_allow_grenades", "bot_allow_snipers", "bot_allow_shield",
        "bot_join_team", "bot_join_after_player", "bot_chatter", "bot_stop", "bot_zombie", "bot_defer_to_human", "bot_auto_vacate", "bot_walk",
        "ls_comp_start", "ls_comp_stop", "ls_comp_spawn", "ls_comp_heal", "ls_comp_players",
    ];
    /// never, even when allowed above: any word with one of these in it
    pub const DENY_PARTS: [&str; 26] = ["file", "cfg", "log", "password", "rcon", "downloadurl", "motd", "exec", "dump", "alias", "plugin", "write", "save", "load", "admin", "ban", "sql", "nav", "del", "remove", "reset", "trace", "record", "demo", "map", "list"];
    /// never: these words exactly
    pub const DENY_WORDS: [&str; 11] = ["quit", "exit", "restart", "_restart", "changelevel", "changelevel2", "map", "meta", "amxx", "condump", "logaddress"];
    /// Run AMX Command: only these (amx_cvar only on what Run Server Command may set; amx_map only
    /// to a map name that passes Change Map's check)
    pub const AMX_WORDS: [&str; 11] = ["amx_slap", "amx_slay", "amx_kick", "amx_say", "amx_psay", "amx_tsay", "amx_csay", "amx_chat", "amx_cvar", "amx_map", "amx_nextmap"];

    pub fn denied(w: &str) -> bool {
        DENY_WORDS.contains(&w) || DENY_PARTS.iter().any(|p| w.contains(p)) || w.starts_with("logaddress")
    }
    /// a first word Run Server Command (and amx_cvar) may use
    pub fn server_word(w: &str) -> bool {
        !denied(w) && (SERVER_WORDS.contains(&w) || SERVER_PREFIXES.iter().any(|p| w.starts_with(p) && w.len() > p.len()))
    }
    /// a map name Change Map would take (the plugin also checks the map is there)
    pub fn map_name(w: &str) -> bool {
        !w.is_empty() && w.len() <= 31 && w.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
    }
}

/// Why a mode's server command (or, with `amx`, AMX Mod X command) is refused; None: it's fine.
/// The same rules as the plugin's `lsg_safe_cmd` (both from `cmd_rules`).
pub fn cmd_refused(cmd: &str, amx: bool) -> Option<&'static str> {
    use cmd_rules::*;
    // (trimmed of ASCII whitespace at the ends, as the plugin's trim() does)
    let c = cmd.trim_ascii();
    if c.is_empty() {
        return Some("it's empty");
    }
    if c.len() > 120 || !c.chars().all(allowed_char) {
        return Some("only letters, digits, spaces and _ . - + ! ? @ # (no quotes, ; or other punctuation), up to 120");
    }
    let low = c.to_ascii_lowercase();
    let mut words = low.split(' ').filter(|w| !w.is_empty());
    let w = words.next().unwrap_or("");
    let w2 = words.next().unwrap_or("");
    if w.len() > WORD_MAX || w2.len() > WORD_MAX {
        return Some("a word is too long");
    }
    if amx {
        if !AMX_WORDS.contains(&w) {
            return Some("only amx_slap, amx_slay, amx_kick, amx_say, amx_psay, amx_tsay, amx_csay, amx_chat, amx_cvar, amx_map and amx_nextmap");
        }
        if w == "amx_cvar" && !server_word(w2) {
            return Some("amx_cvar can only change the settings Run Server Command may (mp_, gameplay sv_ and bot_ settings)");
        }
        if w == "amx_map" && !map_name(w2) {
            return Some("amx_map takes a map name (letters, digits, _ - .)");
        }
        return None;
    }
    if w.starts_with("amx_") {
        return Some("AMX Mod X commands go in Run AMX Command");
    }
    if !server_word(w) {
        return Some("only game rules (mp_), gameplay sv_ settings, bot settings and commands, our plugins' gameplay commands, say and humans_join_team");
    }
    None
}

/// What a mode does beyond its own game, for community badges and the agents' asks.
pub fn capabilities(g: &Graph) -> Vec<&'static str> {
    let mut out = Vec::new();
    let has = |k: &str| g.nodes.iter().any(|n| n.kind == k);
    if has("game.run_command") || has("game.change_map") {
        out.push("server_commands");
    }
    if has("game.amx_command") {
        out.push("amx_commands");
    }
    if has("amx.call") {
        out.push("plugin_calls");
    }
    if has("player.kick") {
        out.push("kicks_players");
    }
    out
}

/// Checks for the advanced nodes: what they can't do (an error) and what they are (a note).
fn advanced_problems(g: &Graph, v: &View, p: &mut Vec<Problem>) {
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        match def.kind.as_str() {
            "game.get_setting" => {
                let c = n.params.get("cvar").and_then(Value::as_str).unwrap_or("");
                if super::gen::secret_cvar(c) {
                    p.push(Problem::err(Some(&n.id), "That setting can't be read by a mode (passwords, rcon, files and logs)."));
                }
            }
            "game.run_command" | "game.amx_command" => {
                let amx = def.kind == "game.amx_command";
                if v.data_source(&n.id, "cmd").is_none() {
                    let c = n.params.get("cmd").and_then(Value::as_str).unwrap_or("");
                    if let Some(why) = cmd_refused(c, amx) {
                        p.push(Problem::err(Some(&n.id), format!("That command is refused: {why}.")));
                    }
                }
                p.push(Problem::warn(Some(&n.id), "Advanced: this runs a command on the server. Commands that reach passwords, files, plugins, bans or quit the server are refused when it runs; community players see \"runs server commands\"."));
            }
            "amx.call" => p.push(Problem::warn(Some(&n.id), "Advanced: this calls into another plugin, which can do anything. Community players see \"calls other plugins\".")),
            "player.kick" => p.push(Problem::warn(Some(&n.id), "This kicks people from the server. Community players see \"kicks players\".")),
            "game.change_map" => p.push(Problem::warn(Some(&n.id), "This changes the map for everyone.")),
            "fx.marker" => {
                let spr = n.params.get("sprite").and_then(Value::as_str).unwrap_or("sprites/arrow1.spr").to_ascii_lowercase();
                if !(spr.starts_with("sprites/") && spr.ends_with(".spr")) {
                    p.push(Problem::err(Some(&n.id), "Objective Marker's sprite is a sprites/....spr file."));
                }
                if v.data_source(&n.id, "follow").is_none() && v.data_source(&n.id, "pos").is_none() {
                    p.push(Problem::err(Some(&n.id), "Objective Marker needs something to follow (an entity wired into Follow) or a Position."));
                }
                if n.params.get("who").and_then(Value::as_str) == Some("player") && v.data_source(&n.id, "player").is_none() {
                    p.push(Problem::err(Some(&n.id), "Objective Marker is for one player: wire the player into Player."));
                }
            }
            "fun.burst" => {
                if v.data_source(&n.id, "at").is_none() && v.data_source(&n.id, "pos").is_none() {
                    p.push(Problem::err(Some(&n.id), "Effect Burst needs an entity wired into At or a Position."));
                }
            }
            "fun.swap" => {
                if v.data_source(&n.id, "a").is_some() && v.data_source(&n.id, "a") == v.data_source(&n.id, "b") {
                    p.push(Problem::warn(Some(&n.id), "Swap Places has the same player in A and B: nothing happens."));
                }
            }
            "msg.status_icon" | "msg.killfeed" => {
                let (k, list): (&str, &[&str]) = if def.kind == "msg.status_icon" { ("icon", &super::gen::STATUS_ICONS) } else { ("weapon", &super::gen::KILL_ICONS) };
                if n.params.get(k).and_then(Value::as_str).is_some_and(|x| !list.contains(&x)) {
                    p.push(Problem::err(Some(&n.id), format!("{} only shows the game's own icons: pick one from the list.", def.title)));
                }
            }
            _ => {}
        }
    }
}

/// The loops (and the output pins their body hangs off).
const LOOPS: [(&str, &str); 8] = [
    ("creature.for_each", "each"),
    ("flow.for_each_player", "each"),
    ("flow.for_each_entity", "each"),
    ("flow.for_loop", "body"),
    ("flow.for_loop_break", "body"),
    ("flow.while", "body"),
    ("flow.for_each_array", "body"),
    ("marker.for_each", "each"),
];

/// Events that can fire many times a second (a hit, a kill in a busy fight, a zone tick, a spawn).
const FREQUENT: [&str; 7] = ["event.player_hurt", "event.player_about_hurt", "up.on_creature_hurt", "comp.on_zone_tick", "creature.on_killed", "comp.on_wave_spawn", "comp.on_barricade_damaged"];
const HURT: [&str; 3] = ["event.player_hurt", "event.player_about_hurt", "up.on_creature_hurt"];

/// Everything the flow from these output pins runs (through any further flow pins).
fn flow_from<'a>(v: &View<'a>, node: &'a str, pins: &[&str]) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut todo: Vec<&'a str> = Vec::new();
    for pin in pins {
        if let Some((to, _)) = v.exec_target(node, pin) {
            todo.push(to);
        }
    }
    while let Some(n) = todo.pop() {
        if !seen.insert(n) {
            continue;
        }
        for pin in v.exec_outs(n) {
            if let Some((to, _)) = v.exec_target(n, &pin) {
                todo.push(to);
            }
        }
    }
    seen
}

/// Warnings about what costs the server (docs/notes/PerfAudit.md, the 9 patterns): each says
/// what happens and what to do instead. They never stop a compile.
pub fn cost_warnings(g: &Graph, v: &View) -> Vec<Problem> {
    let mut p = Vec::new();
    let kind = |id: &str| v.defs.get(id).map(|d| d.kind.as_str()).unwrap_or("");
    let is_loop = |id: &str| LOOPS.iter().any(|(k, _)| *k == kind(id));
    let is_msg = |id: &str| kind(id).starts_with("msg.");
    let mut said: BTreeSet<(String, &str)> = BTreeSet::new();
    let mut warn = |p: &mut Vec<Problem>, id: &str, what: &'static str, text: String| {
        if said.insert((id.to_string(), what)) {
            p.push(Problem::warn(Some(id), text));
        }
    };
    // frequent events: what they run, every time
    for e in g.nodes.iter().filter(|n| FREQUENT.contains(&kind(&n.id))) {
        let id = e.id.as_str();
        let title = v.defs.get(id).map(|d| d.title.clone()).unwrap_or_else(|| "This event".into());
        let body = flow_from(v, id, &["then"]);
        for n in &body {
            match kind(n) {
                "flow.for_each_entity" | "flow.pick_entity" => warn(&mut p, n, "scan", format!("This runs on every {title}, and each time it looks through every entity on the map (about 0.1 ms, more on big maps). Keep what it finds in a variable at round start instead.")),
                "creature.for_each" => warn(&mut p, n, "scan", format!("This runs on every {title}, and each time it goes through every creature (with 60 alive, every frame adds up). Use Creature Idle or a Repeat timer once a second instead.")),
                "timer.delay" => warn(&mut p, n, "delay", format!("A Delay in {title} starts another timer on every fire (they pile up). Use Retrigger Delay, which starts over instead.")),
                _ => {}
            }
        }
        if HURT.contains(&kind(id)) && (body.len() > 15 || body.iter().any(|n| is_loop(n))) {
            warn(&mut p, id, "heavy", format!("{title} runs once per hit: with shotguns and creatures, hundreds of times a second. This flow is long or loops; keep it short (a few nodes), or move the work to a timer."));
        }
        for l in body.iter().filter(|n| kind(n) == "flow.for_each_player") {
            let pin = LOOPS.iter().find(|(k, _)| *k == kind(l)).map(|(_, p)| *p).unwrap_or("each");
            for m in flow_from(v, l, &[pin]).into_iter().filter(|m| is_msg(m)) {
                warn(&mut p, m, "msgs", format!("A message to every player on every {title}: about 32 network messages each time. Show it once a second at most (a Repeat timer), or only to the players who need it."));
            }
        }
    }
    // loops: inside another loop, a Delay inside
    for l in g.nodes.iter().filter(|n| is_loop(&n.id)) {
        let pin = LOOPS.iter().find(|(k, _)| *k == kind(&l.id)).map(|(_, p)| *p).unwrap_or("each");
        // (For Each Player and For Each Marker are short: 32 players, 128 markers at most)
        let big = |id: &str| matches!(kind(id), "flow.for_loop" | "flow.for_loop_break" | "flow.while" | "flow.for_each_array" | "flow.for_each_entity" | "creature.for_each");
        for n in flow_from(v, &l.id, &[pin]) {
            if big(&l.id) && big(n) {
                warn(&mut p, n, "nested", "A loop inside a loop: up to 10,000 x 10,000 passes in one server frame can freeze the server. Loop over less, or Break out early.".into());
            }
            if kind(n) == "timer.delay" {
                warn(&mut p, n, "delay", "A Delay inside a loop starts another timer on every pass (they pile up). Use Retrigger Delay, or do the waiting outside the loop.".into());
            }
        }
    }
    // Timelines: updates a second = copies (a player's own in a Function with a Player input: up
    // to 32) x ticks a second; each runs Update's nodes and its tracks
    for t in g.nodes.iter().filter(|n| kind(&n.id) == "timeline.play") {
        let Some(tl) = t.params.get("timeline").and_then(Value::as_str).and_then(|id| g.timelines.iter().find(|x| x.id == id)) else { continue };
        let per_player = t.graph.as_deref().and_then(|f| g.functions.iter().find(|x| x.id == f)).is_some_and(|f| f.inputs.iter().any(|m| m.ty == "player"));
        let copies = if per_player { 32.0 } else { 1.0 };
        let rate = 1.0 / tl.tick.max(MIN_TICK);
        let work = 1 + flow_from(v, &t.id, &["update"]).len();
        let ups = copies * rate;
        if ups > 200.0 || ups * work as f64 > 1000.0 {
            let who = if per_player { format!("once per player (up to 32 copies) x {} updates a second", rate.round()) } else { format!("{} updates a second", rate.round()) };
            warn(&mut p, &t.id, "timeline", format!("This Timeline runs {who}: up to {} updates a second, each running {work} node{}. Raise its Tick (0.2 s still looks smooth), keep Update short, or play one Timeline for everyone.", ups.round(), if work == 1 { "" } else { "s" }));
        }
    }
    // fast timers: real work many times a second
    for t in g.nodes.iter().filter(|n| matches!(kind(&n.id), "timer.repeat" | "timer.retrigger")) {
        let every = t.params.get("every").or_else(|| t.params.get("secs")).and_then(Value::as_f64).unwrap_or(1.0);
        if every >= 1.0 {
            continue;
        }
        let body = flow_from(v, &t.id, &["tick", "then"]);
        let scans = body.iter().any(|n| matches!(kind(n), "flow.for_each_entity" | "flow.pick_entity"));
        let work = body.iter().any(|n| is_loop(n) || is_msg(n) || kind(n).starts_with("bots.") || kind(n).starts_with("bot."));
        if scans || (every < 0.5 && work) {
            warn(&mut p, &t.id, "fast", format!("This runs {} times a second{}. Every 1 s is usually enough; HUD text faster than that is also network spam.", (1.0 / every.max(0.1)).round(), if scans { " and looks through every entity each time" } else { " with loops, messages or bot orders inside" }));
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
        // (a Timeline's outputs run later, on its own ticks: flow round to it isn't a loop)
        // (Say and Choose's replies come later, from the player's pick: a reply wired back is a
        // conversation going on, not a loop)
        let later = v.defs.get(n).is_some_and(|d| d.kind == "timeline.play" || d.kind == "npc.choose");
        for (t, on) in if later { Vec::new() } else { v.next_nodes(n) } {
            if on {
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

/// A Function that calls (through other Functions) back into itself: old Function nodes (what
/// their flow reaches) and Functions with their own graph (every node on it).
fn find_call_cycle<'a>(v: &View<'a>) -> Option<&'a str> {
    // a call node -> the function it runs (an old Function node's id, or a Function's id)
    let target = |n: &'a str| -> Option<&'a str> {
        let node = v.nodes.get(n)?;
        match node.kind.as_str() {
            "func.call" => node.params.get("func").and_then(Value::as_str).and_then(|t| v.nodes.get_key_value(t).map(|(k, _)| *k)),
            "fn.call" | "fn.call_pure" | "timer.by_function" => node.params.get("func").and_then(Value::as_str).and_then(|t| v.funcs.get_key_value(t).map(|(k, _)| *k)),
            _ => None,
        }
    };
    let calls_from = |f: &'a str| -> Vec<&'a str> {
        let body: Vec<&'a str> = if v.funcs.contains_key(f) {
            v.g.nodes.iter().filter(|n| n.graph.as_deref() == Some(f)).map(|n| n.id.as_str()).collect()
        } else {
            let mut out = Vec::new();
            let mut stack = vec![f];
            let mut seen = HashSet::new();
            while let Some(cur) = stack.pop() {
                if !seen.insert(cur) {
                    continue;
                }
                out.push(cur);
                for p in v.exec_outs(cur) {
                    if let Some((t, _)) = v.exec_target(cur, &p) {
                        stack.push(t);
                    }
                }
            }
            out
        };
        body.into_iter().filter_map(target).collect()
    };
    let starts: Vec<&'a str> = v.g.nodes.iter().filter(|n| n.kind == "func.define").map(|n| n.id.as_str()).chain(v.g.functions.iter().map(|f| f.id.as_str())).collect();
    for start in &starts {
        let mut stack = calls_from(start);
        let mut seen = HashSet::new();
        while let Some(f) = stack.pop() {
            if f == *start {
                // point at a node: the old Function node, or the new Function's entry
                return v.nodes.get_key_value(f).map(|(k, _)| *k).or_else(|| v.g.nodes.iter().find(|n| n.kind == "fn.entry" && n.graph.as_deref() == Some(f)).map(|n| n.id.as_str())).or(Some(start));
            }
            if seen.insert(f) {
                stack.extend(calls_from(f));
            }
        }
    }
    None
}

/// A Macro that uses (through other Macros) itself: its name.
fn find_macro_cycle(v: &View) -> Option<String> {
    let uses = |m: &str| -> Vec<String> {
        v.g.nodes.iter().filter(|n| n.graph.as_deref() == Some(m) && n.kind == "macro.use").filter_map(|n| n.params.get("macro").and_then(Value::as_str).map(str::to_string)).collect()
    };
    for m in &v.g.macros {
        let mut stack = uses(&m.id);
        let mut seen = HashSet::new();
        while let Some(x) = stack.pop() {
            if x == m.id {
                return Some(m.name.clone());
            }
            if seen.insert(x.clone()) {
                stack.extend(uses(&x));
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
        for ip in v.inputs(n).iter().filter(|i| i.ty != "exec") {
            if let Some((src, _)) = v.data_source(n, &ip.id) {
                if v.pure(src) {
                    if let Some(hit) = visit(v, src, state) {
                        return Some(hit);
                    }
                }
            }
        }
        state.insert(n, 2);
        None
    }
    let mut state: HashMap<&str, u8> = HashMap::new();
    for n in v.g.nodes.iter().filter(|n| v.pure(&n.id)) {
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
        "vector" => "Vector",
        "timer" => "Timer",
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
        "path" => match v.as_str() {
            Some(s) if valid_path(s) => Ok(()),
            _ => Err(format!("{} is a file path of up to 64 letters, digits, _ - . or /.", def.label)),
        },
        "entname" => match v.as_str() {
            Some(s) if valid_ent_name(s) => Ok(()),
            _ => Err(format!("{} is a name of up to 32 letters, digits, spaces, _ - or .", def.label)),
        },
        "keys" => match v.as_str() {
            Some(s) if valid_keys(s, def.max.unwrap_or(120.0) as usize) => Ok(()),
            _ => Err(format!("{} are words like radius=160 (letters, digits, _ - . = : and spaces; no quotes).", def.label)),
        },
        "var" | "varvalue" | "func" | "timer" | "fn" | "macro" | "struct" | "enum" | "type" | "field" | "enumvalue" | "cases" | "marker" | "cevent" | "tline" => Ok(()),
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

    /// Things bots could use that nothing tells them about get one warning each (never an error);
    /// what's told (Bots may use, bots=1, a Bots: Offer or Goal wired at it), what gm_components
    /// tells bots by itself, and a quiet node get none.
    #[test]
    fn bots_are_told_or_warned() {
        let notes = |g: &Graph| validate(g).into_iter().filter(|p| p.message.starts_with(BOTS_NOTE)).map(|p| (p.level, p.node.unwrap_or_default(), p.message)).collect::<Vec<_>>();
        let n = |id: &str, t: &str, params: Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": params});
        let base = vec![
            n("rs", "event.round_started", json!({})),
            n("lever", "npc.spawn_use", json!({"at": "spot1", "prompt": "Pull the lever"})),
            n("note", "npc.spawn_use", json!({"at": "spot1", "bots": true})),
            n("term", "comp.create", json!({"kind": "ls_interactable", "at": "spot1", "keys": "name=terminal"})),
            n("ok", "comp.create", json!({"kind": "ls_interactable", "at": "spot1", "keys": "bots=1"})),
            n("cap", "comp.create", json!({"kind": "ls_capture_point", "at": "spot1"})),
            n("shop", "comp.create", json!({"kind": "ls_shop", "at": "spot1"})),
            n("mara", "npc.spawn", json!({"at": "spot1", "name": "Mara"})),
        ];
        let flow = json!([e("w1", "rs", "then", "lever", "exec"), e("w2", "lever", "then", "note", "exec"), e("w3", "note", "then", "term", "exec"), e("w4", "term", "then", "ok", "exec"), e("w5", "ok", "then", "cap", "exec"), e("w6", "cap", "then", "shop", "exec"), e("w7", "shop", "then", "mara", "exec")]);
        let g = graph(Value::Array(base.clone()), flow.clone());
        let got = notes(&g);
        assert_eq!(got.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), ["lever", "term"], "{got:#?}");
        assert!(got.iter().all(|x| x.0 == "warning"));
        assert_eq!(got[0].2, "Bots won't know about the interactable \u{201c}Pull the lever\u{201d}. Add an Offer to bots or a Bot goal.");
        assert!(got[1].2.contains("\u{201c}terminal\u{201d}"));
        // an NPC counts once the mode talks through NPCs; a Bots: Offer at it tells the bots
        let mut talk = base.clone();
        talk.push(n("talk", "npc.on_talk", json!({})));
        let g = graph(Value::Array(talk.clone()), flow.clone());
        assert!(notes(&g).iter().any(|x| x.1 == "mara" && x.2.contains("\u{201c}Mara\u{201d}")));
        talk.push(n("offer", "bots.offer", json!({"kind": "use"})));
        let mut wires = flow.as_array().unwrap().clone();
        wires.extend([e("w8", "mara", "then", "offer", "exec"), e("w9", "mara", "out", "offer", "at")]);
        let g = graph(Value::Array(talk.clone()), Value::Array(wires.clone()));
        assert!(!notes(&g).iter().any(|x| x.1 == "mara"));
        // "Don't warn for this one"
        let mut quiet = talk.clone();
        quiet[1]["quiet"] = json!(true);
        let g = graph(Value::Array(quiet), Value::Array(wires));
        assert!(!notes(&g).iter().any(|x| x.1 == "lever"));
        assert!(serde_json::to_value(&g).unwrap()["nodes"][1]["quiet"] == json!(true), "kept in the file");
    }

    #[test]
    fn nothing_happens_yet() {
        let nothing = |g: &Graph| validate(g).iter().any(|p| p.message.starts_with("Nothing happens yet"));
        assert!(nothing(&graph(json!([]), json!([]))));
        let lone = graph(json!([{"id": "rs", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}}]), json!([]));
        assert!(nothing(&lone));
        let wired = graph(
            json!([{"id": "rs", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}}, {"id": "p", "type": "debug.print", "pos": {"x": 300, "y": 0}, "params": {"text": "hi"}}]),
            json!([e("w", "rs", "then", "p", "exec")]),
        );
        assert!(!nothing(&wired));
        // game settings alone are a mode
        let mut set = graph(json!([]), json!([]));
        set.mode.settings = vec![Setting { cvar: "mp_freeforall".into(), value: 1.0 }];
        assert!(!nothing(&set));
    }

    fn e(id: &str, a: &str, ap: &str, b: &str, bp: &str) -> Value {
        json!({"id": id, "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
    }

    fn msgs(p: &[Problem]) -> Vec<String> {
        p.iter().map(|p| format!("{}:{}:{}", p.level, p.node.clone().unwrap_or_default(), p.message)).collect()
    }

    const TYPES: [&str; 18] = ["exec", "player", "entity", "team", "number", "bool", "text", "any", "var", "vector", "timer", "$T", "$K", "$E", "$S", "$F", "array<$T>", "map<$K,$T>"];
    /// Nodes whose pins come from the graph (a Macro's, a Function's): the registry only has
    /// their fixed part.
    const DYNAMIC: [&str; 4] = ["macro.use", "macro.exit", "macro.entry", "event.call"];
    // (Timeline: its tracks add outputs, event tracks flow ones)

    #[test]
    fn registry_is_complete() {
        let r = registry();
        assert_eq!(r.version, 4);
        for kind in ["event.round_started", "event.player_killed", "cond.is_team", "player.give_weapon", "score.end_round", "msg.broadcast", "flow.reroute", "timer.delay", "var.set", "comp.on_capture", "creature.spawn"] {
            assert!(r.get(kind).is_some(), "{kind}");
        }
        let mut seen = HashSet::new();
        for n in &r.nodes {
            assert!(seen.insert(n.kind.as_str()), "{} twice", n.kind);
            assert!(!(n.pure && n.event), "{}", n.kind);
            if !DYNAMIC.contains(&n.kind.as_str()) {
                assert_eq!(n.pure, !n.event && !n.reroute && !n.has_exec_in(), "{}: pure means no flow in", n.kind);
            }
            for prm in &n.params {
                if !["var", "varvalue", "func", "timer", "fn", "macro", "struct", "enum", "type", "field", "enumvalue", "table", "column"].contains(&prm.kind.as_str()) {
                    assert!(check_param(prm, &prm.default).is_ok(), "{} {} default", n.kind, prm.id);
                }
            }
            for pin in n.inputs.iter().chain(&n.outputs) {
                assert!(TYPES.contains(&pin.ty.as_str()) || pin.ty == "array<player>" || pin.ty == "array<entity>" || pin.ty == "array<$K>", "{} {} {}", n.kind, pin.id, pin.ty);
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
        // version 1 and 2 files open as version 3
        let g = parse(r#"{"schema":"loner-strike/graph","version":1,"mode":{"id":"abc","name":"ABC"}}"#).unwrap();
        assert!(g.nodes.is_empty() && g.version == 3 && g.variables.is_empty() && g.functions.is_empty());
        let g = parse(r#"{"schema":"loner-strike/graph","version":2,"mode":{"id":"abc","name":"ABC"},"variables":[]}"#).unwrap();
        assert!(g.version == 3 && g.enums.is_empty() && g.structs.is_empty() && g.macros.is_empty());
        // and nothing new is written for them
        let back = serde_json::to_string(&g).unwrap();
        assert!(!back.contains("functions") && !back.contains("enums") && !back.contains("\"graph\""), "{back}");
        let t = include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json");
        let g = parse(t).unwrap();
        let back = parse(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(g, back);
    }

    #[test]
    fn templates_are_clean() {
        for t in [
            include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json"),
            include_str!("../../../../ui/src/views/tools/studio/templates/last-stand.graph.json"),
            include_str!("../../../../ui/src/views/tools/studio/templates/battle-royale.graph.json"),
        ] {
            let g = parse(t).unwrap();
            let p = validate(&g);
            assert!(p.is_empty(), "{}: {:?}", g.mode.id, msgs(&p));
        }
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
    fn damage_nodes_only_before_the_damage() {
        let g = graph(
            json!([{"id":"h","type":"event.player_about_hurt"},{"id":"s","type":"dmg.scale"},{"id":"k","type":"event.player_killed"},{"id":"b","type":"dmg.block"},{"id":"d","type":"timer.delay"},{"id":"x","type":"dmg.set"}]),
            json!([e("e1","h","then","s","exec"), e("e2","k","then","b","exec"), e("e3","s","then","d","exec"), e("e4","d","then","x","exec")]),
        );
        let m = msgs(&validate(&g));
        assert!(!m.iter().any(|m| m.starts_with("error:s:")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("error:b:") && m.contains("only works in a Player About To Be Hurt")), "{m:?}");
        assert!(m.iter().any(|m| m.starts_with("error:x:") && m.contains("before any Delay")), "{m:?}");
    }

    #[test]
    fn variables_checked() {
        let mut g = graph(
            json!([{"id":"r","type":"event.player_spawned"},{"id":"s","type":"var.set","params":{"var":"kills","value":"x"}},{"id":"a","type":"var.add","params":{"var":"name"}},{"id":"q","type":"var.get"}]),
            json!([e("e1","r","then","s","exec"), e("e2","s","then","a","exec")]),
        );
        g.variables = vec![
            Variable { id: "kills".into(), name: "Kills".into(), ty: "number".into(), scope: "player".into(), default: json!(0), reset: "never".into(), setting: None },
            Variable { id: "name".into(), name: "Last name".into(), ty: "text".into(), scope: "mode".into(), default: json!(""), reset: "never".into(), setting: None },
            Variable { id: "bad".into(), name: "Bad /*".into(), ty: "number".into(), scope: "mode".into(), default: json!("x"), reset: "never".into(), setting: None },
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

    fn v3(extra: Value) -> Graph {
        let mut base = json!({"schema": SCHEMA, "version": 3, "mode": {"id": "testmode", "name": "Test"}});
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        from_value(base).unwrap()
    }

    #[test]
    fn functions_and_their_graphs() {
        let fns = json!([{"id": "f1", "name": "Give", "inputs": [{"id": "p", "name": "Who", "type": "player"}], "outputs": [{"id": "n", "name": "N", "type": "number"}],
                          "locals": [{"id": "l1", "name": "Count", "type": "number", "scope": "mode"}]},
                         {"id": "f2", "name": "Half", "pure": true, "inputs": [{"id": "x", "name": "X", "type": "number"}], "outputs": [{"id": "y", "name": "Y", "type": "number"}]}]);
        // a clean one: entry, a local, return; called with flow and as pure
        let g = v3(json!({"functions": fns, "nodes": [
            {"id": "r", "type": "event.player_spawned"}, {"id": "c", "type": "fn.call", "params": {"func": "f1"}}, {"id": "h", "type": "fn.call_pure", "params": {"func": "f2"}},
            {"id": "s", "type": "player.set_health"},
            {"id": "e", "type": "fn.entry", "graph": "f1"}, {"id": "gl", "type": "var.get", "graph": "f1", "params": {"var": "l1"}}, {"id": "ret", "type": "fn.return", "graph": "f1"},
            {"id": "e2", "type": "fn.entry", "graph": "f2"}, {"id": "m", "type": "math.op", "graph": "f2", "params": {"op": "div", "b": 2}}, {"id": "ret2", "type": "fn.return", "graph": "f2"}],
            "edges": [e("w1","r","then","c","exec"), e("w2","r","player","c","p"), e("w3","c","then","s","exec"), e("w4","r","player","s","player"), e("w5","c","n","h","x"), e("w6","h","y","s","amount"),
                      e("w7","e","then","ret","exec"), e("w8","gl","out","ret","n"), e("w9","e2","then","ret2","exec"), e("w10","e2","x","m","a"), e("w11","m","out","ret2","y")]}));
        assert!(validate(&g).is_empty(), "{:?}", msgs(&validate(&g)));
        let v = View::new(&g);
        assert_eq!(v.out_type("c", "n").as_deref(), Some("number"));
        assert!(v.pure("h") && !v.pure("c"));
        assert_eq!(v.local_of.get("l1"), Some(&"f1"));
        // what's wrong: no entry, a Delay inside, a local used outside, a pure call to a flow Function, a wire across graphs
        let g = v3(json!({"functions": fns, "nodes": [
            {"id": "r", "type": "event.round_started"}, {"id": "gl", "type": "var.get", "params": {"var": "l1"}}, {"id": "h", "type": "fn.call_pure", "params": {"func": "f1"}},
            {"id": "e", "type": "fn.entry", "graph": "f1"}, {"id": "d", "type": "timer.delay", "graph": "f1"}, {"id": "k", "type": "event.round_started", "graph": "f1"}],
            "edges": [e("w1","e","then","d","exec"), e("w2","r","then","d","exec")]}));
        let m = msgs(&validate(&g));
        for want in ["Half\u{201d} needs its Function Entry", "Delay waits", "local variable of another Function", "isn't a pure Function", "from one graph to another", "goes on the Event Graph"] {
            assert!(m.iter().any(|x| x.contains(want)), "{want}: {m:?}");
        }
    }

    #[test]
    fn functions_cant_call_themselves() {
        let g = v3(json!({"functions": [{"id": "f1", "name": "Loop", "inputs": [], "outputs": []}], "nodes": [
            {"id": "e", "type": "fn.entry", "graph": "f1"}, {"id": "c", "type": "fn.call", "graph": "f1", "params": {"func": "f1"}}],
            "edges": [e("w1","e","then","c","exec")]}));
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("ends up calling itself")));
    }

    #[test]
    fn container_types() {
        let g = v3(json!({"structs": [{"id": "s1", "name": "Kit", "fields": [{"id": "a", "name": "Ammo", "type": "number"}, {"id": "t", "name": "Tag", "type": "text"}]}],
            "enums": [{"id": "e1", "name": "Class", "values": ["A", "B"]}],
            "variables": [
                {"id": "nums", "name": "Nums", "type": "array<number>", "scope": "mode"},
                {"id": "names", "name": "Names", "type": "array<text>", "scope": "mode"},
                {"id": "kits", "name": "Kits", "type": "array<struct:s1>", "scope": "mode"},
                {"id": "badscope", "name": "Bad", "type": "map<text,number>", "scope": "player"},
                {"id": "badtype", "name": "Bad2", "type": "array<array<number>>", "scope": "mode"},
                {"id": "badmap", "name": "Bad3", "type": "map<struct:s1,number>", "scope": "mode"},
                {"id": "cls", "name": "Cls", "type": "enum:e1", "scope": "mode", "default": 5}],
            "nodes": [{"id": "r", "type": "event.round_started"}, {"id": "gn", "type": "var.get", "params": {"var": "nums"}},
                      {"id": "add", "type": "array.add", "params": {"of": "text"}}, {"id": "len", "type": "array.length", "params": {"of": "number"}},
                      {"id": "sf", "type": "struct.set_field", "params": {"var": "nums", "field": "a"}}],
            "edges": [e("w1","r","then","add","exec"), e("w2","gn","out","add","array"), e("w3","gn","out","len","array"), e("w4","add","then","sf","exec")]}));
        let m = msgs(&validate(&g));
        assert!(m.iter().any(|x| x.contains("Array of Number can't go into Array")), "{m:?}");
        assert!(!m.iter().any(|x| x.starts_with("error:len:")), "{m:?}");
        assert!(m.iter().any(|x| x.contains("arrays, maps and structs are kept for the whole mode")), "{m:?}");
        assert!(m.iter().filter(|x| x.contains("type or scope this app doesn't know")).count() == 2, "{m:?}");
        assert!(m.iter().any(|x| x.contains("\u{201c}Cls\u{201d} starts with a value")), "{m:?}");
        assert!(m.iter().any(|x| x.starts_with("error:sf:") && x.contains("needs a struct variable")), "{m:?}");
        assert!(compatible("array<any>", "array<number>") && !compatible("array<number>", "array<text>") && compatible("player", "entity") && !compatible("array<player>", "array<entity>"));
    }

    #[test]
    fn macros_expand_in_place() {
        let g = v3(json!({"macros": [{"id": "m1", "name": "Twice", "inputs": [{"id": "go", "name": "In", "type": "exec"}, {"id": "v", "name": "V", "type": "number"}],
                                      "outputs": [{"id": "done", "name": "Out", "type": "exec"}, {"id": "r", "name": "R", "type": "number"}]}],
            "nodes": [{"id": "r", "type": "event.player_spawned"}, {"id": "u", "type": "macro.use", "params": {"macro": "m1"}}, {"id": "n", "type": "math.number", "params": {"value": 3}},
                      {"id": "s", "type": "player.set_health"},
                      {"id": "me", "type": "macro.entry", "graph": "m1"}, {"id": "mm", "type": "math.op", "graph": "m1", "params": {"op": "mul", "b": 2}},
                      {"id": "mp", "type": "debug.print", "graph": "m1"}, {"id": "mx", "type": "macro.exit", "graph": "m1"}],
            "edges": [e("w1","r","then","u","go"), e("w2","n","out","u","v"), e("w3","u","done","s","exec"), e("w4","u","r","s","amount"), e("w5","r","player","s","player"),
                      e("w6","me","go","mp","exec"), e("w7","me","v","mm","a"), e("w8","mp","then","mx","done"), e("w9","mm","out","mx","r")]}));
        assert!(validate(&g).is_empty(), "{:?}", msgs(&validate(&g)));
        let x = super::super::expand::expand(&g);
        let ids: Vec<&str> = x.graph.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(!ids.contains(&"u") && !ids.contains(&"me") && !ids.contains(&"mp") && ids.contains(&"x1_mp") && ids.contains(&"x1_mm"), "{ids:?}");
        assert_eq!(x.origin.get("x1_mp").map(String::as_str), Some("u"));
        let v = View::new(&x.graph);
        assert_eq!(v.exec_target("r", "then"), Some(("x1_mp", "exec")));
        assert_eq!(v.exec_target("x1_mp", "then"), Some(("s", "exec")));
        assert_eq!(v.data_source("x1_mm", "a"), Some(("n", "out")));
        assert_eq!(v.data_source("s", "amount"), Some(("x1_mm", "out")));
        assert!(validate(&x.graph).iter().all(|p| p.level != "error"), "{:?}", msgs(&validate(&x.graph)));
        // a Macro that uses itself
        let g = v3(json!({"macros": [{"id": "m1", "name": "Self", "inputs": [], "outputs": []}],
            "nodes": [{"id": "me", "type": "macro.entry", "graph": "m1"}, {"id": "u", "type": "macro.use", "graph": "m1", "params": {"macro": "m1"}}], "edges": []}));
        assert!(msgs(&validate(&g)).iter().any(|m| m.contains("can't use itself")));
    }

    #[test]
    fn commands_from_a_mode_cant_break_out() {
        let evil = "x\";rcon_password hacked;\"";
        assert!(cmd_refused(evil, false).is_some());
        assert!(cmd_refused(evil, true).is_some());
        assert!(cmd_refused(&format!("say {evil}"), false).is_some());
        assert!(cmd_refused("say hi\nquit", false).is_some());
        assert!(cmd_refused("say \"hi", false).is_some(), "an unpaired quote");
        assert!(cmd_refused("quit", false).is_some());
        assert!(cmd_refused("exec evil.cfg", false).is_some());
        assert!(cmd_refused("meta unload amxx", false).is_some());
        assert!(cmd_refused("amx_rcon quit", true).is_some());
        assert!(cmd_refused("amx_cvar rcon_password x", true).is_some());
        assert!(cmd_refused("amx_ban 0 x", true).is_some());
        // the engine drops quotes: a quoted first word is still that word (refused: no quotes at all)
        for q in ["\"rcon_password\" x", "\"exec\" foo.cfg", "\"quit\"", "\"sv_gravity\" 1", "'exec' x", "exec:x", "mp_x(exec"] {
            assert!(cmd_refused(q, false).is_some(), "{q}");
            assert!(cmd_refused(q, true).is_some(), "amx {q}");
        }
        // AMX Mod X commands only through Run AMX Command (with its block list)
        for a in ["amx_rcon quit", "amx_ban 0 x", "amx_addadmin x", "amx_execcfg x", "amx_slap x", "amx_cvar sv_gravity 1"] {
            assert!(cmd_refused(a, false).is_some(), "{a}");
        }
        for a in ["amx_rcon quit", "amx_ban 0 x", "amx_addadmin x", "amx_execcfg x", "amx_plugins", "amx_last", "amx_cvar rcon_password x", "amx_cvar sv_password x", "amx_cvar mp_logfile 1", "amx_cvar hostname x", "amx_cvar", "amx_restrict save", "amx_restrict load x", "amx_who", "amx_map", "amx_map de_dust#2", "amx_cvar bot_nav_save 1"] {
            assert!(cmd_refused(a, true).is_some(), "{a}");
        }
        // names with file, cfg, log and the like; commands that quit, change the map or alias
        for w in ["mp_logfile 1", "mp_logdetail 3", "ls_cfg_write x", "bot_profile_file x", "ls_savenav", "mp_motdfile x", "writecfg x", "motd_write x", "condump", "logaddress_add 1.2.3.4 1", "sv_downloadurl x", "changelevel de_dust", "map de_dust", "restart", "_restart", "alias x", "meta list", "hostname x", "sv_cheats 1", "rcon_password x", "plugin_load x"] {
            assert!(cmd_refused(w, false).is_some(), "{w}");
        }
        // bot_nav_* writes and deletes nav files; our plugins' save/delete/reset; other bot_ and ls_ words
        for w in ["bot_nav_save", "bot_nav_load", "bot_nav_delete", "bot_nav_analyze", "bot_ls_trace 1", "bot_profile_db x", "ls_comp_save", "ls_comp_del 3", "ls_comp_reset", "ls_comp_tracestats", "ls_wave 3", "mp_mapcycle x"] {
            assert!(cmd_refused(w, false).is_some(), "{w}");
        }
        // a word longer than the plugin's buffer, tabs and other whitespace
        let long = format!("mp_{} 1", "a".repeat(45));
        assert!(cmd_refused(&long, false).is_some(), "48 letters");
        assert!(cmd_refused(&format!("mp_{} 1", "a".repeat(44)), false).is_none(), "47 letters");
        assert!(cmd_refused(&format!("amx_cvar mp_{} 1", "a".repeat(45)), true).is_some(), "a long second word");
        for ws in ["sv_gravity\t1", "sv_gravity\u{a0}1", "\u{a0}sv_gravity 1", "sv_gravity\r1", "sv_gravity\x0b1"] {
            assert!(cmd_refused(ws, false).is_some(), "{ws:?}");
        }
        // (the ends are trimmed, as the plugin does)
        assert!(cmd_refused("\tsv_gravity 1\n", false).is_none());
        for ok in ["sv_gravity 400", "sv_airaccelerate 100", "mp_friendlyfire 1", "bot_quota 10", "bot_add_t", "bot_knives_only", "ls_comp_start 4", "say hello there!", "humans_join_team CT", "sv_restart 1", "sv_gravity  400"] {
            assert!(cmd_refused(ok, false).is_none(), "{ok}");
        }
        assert!(cmd_refused("say \"hello there\"", false).is_some(), "no quotes");
        for ok in ["amx_slap @T 0", "amx_say hello", "amx_cvar sv_gravity 400", "amx_cvar mp_roundtime 3", "amx_kick bob bye", "amx_map de_dust2", "amx_nextmap", "amx_psay bob hi", "amx_slay @CT"] {
            assert!(cmd_refused(ok, true).is_none(), "{ok}");
        }
        // a node with a refused command doesn't build
        let g = graph(
            json!([
                {"id":"e","type":"event.round_started","params":{}},
                {"id":"c","type":"game.run_command","params":{"cmd":"sv_gravity 400; quit"}}
            ]),
            json!([{"id":"w","from":{"node":"e","pin":"then"},"to":{"node":"c","pin":"exec"}}]),
        );
        assert!(validate(&g).iter().any(|p| p.level == "error" && p.node.as_deref() == Some("c")), "{:?}", validate(&g));
    }

    #[test]
    fn the_bomb_setting_is_allowed_and_the_list_still_holds() {
        let mut g = graph(json!([{"id":"e","type":"event.round_started","params":{}}]), json!([]));
        g.mode.settings = serde_json::from_value(json!([{"cvar": "mp_give_player_c4", "value": 0}])).unwrap();
        assert!(!has_errors(&validate(&g)), "{:?}", msgs(&validate(&g)));
        for bad in ["sv_cheats", "rcon_password", "mp_logfile", "mp_give_player_c5"] {
            g.mode.settings = serde_json::from_value(json!([{"cvar": bad, "value": 1}])).unwrap();
            assert!(has_errors(&validate(&g)), "{bad}");
        }
    }

    #[test]
    fn event_graphs_fold_into_one_in_tab_order() {
        let mut g = graph(
            json!([
                {"id":"r1","type":"event.round_started","params":{},"graph":"shop","pos":{"x":0,"y":0}},
                {"id":"m1","type":"msg.broadcast","params":{"text":"Shop"},"graph":"shop"},
                {"id":"r2","type":"event.round_started","params":{},"graph":"rounds","pos":{"x":0,"y":300}},
                {"id":"m2","type":"msg.broadcast","params":{"text":"Rounds"},"graph":"rounds"},
                {"id":"r0","type":"event.round_started","params":{},"pos":{"x":0,"y":900}},
                {"id":"m0","type":"msg.broadcast","params":{"text":"First"}}
            ]),
            json!([e("w1","r1","then","m1","exec"), e("w2","r2","then","m2","exec"), e("w0","r0","then","m0","exec")]),
        );
        g.event_graphs = vec![EventGraph { id: "rounds".into(), name: "Rounds".into() }, EventGraph { id: "shop".into(), name: "Shop".into() }];
        let p = validate(&g);
        assert!(!has_errors(&p), "{:?}", msgs(&p));
        assert!(p.iter().any(|x| x.level == "warning" && x.message.starts_with("Round Started is on Event Graph, Rounds, Shop")), "{:?}", msgs(&p));
        // the first graph's, then Rounds', then Shop's
        let flat = flatten(&g);
        let order: Vec<&str> = flat.nodes.iter().filter(|n| n.kind == "event.round_started").map(|n| n.id.as_str()).collect();
        assert_eq!(order, ["r0", "r2", "r1"]);
        assert!(flat.nodes.iter().all(|n| n.graph.is_none()));
        let src = super::super::gen::generate(&g, false).source;
        // (the round start calls them in this order; each event's function follows in that order)
        let (a, b, c) = (src.find("// Round Started (r0)").unwrap(), src.find("// Round Started (r2)").unwrap(), src.find("// Round Started (r1)").unwrap());
        assert!(a < b && b < c, "the plugin runs them in tab order");
        // a wire from one graph to another; a graph that isn't there
        g.edges.push(serde_json::from_value(e("wx", "r1", "then", "m2", "exec")).unwrap());
        assert!(validate(&g).iter().any(|x| x.level == "error" && x.edge.as_deref() == Some("wx")));
        g.edges.pop();
        g.nodes[1].graph = Some("gone".into());
        assert!(has_errors(&validate(&g)));
    }

    #[test]
    fn cost_warnings_name_the_costly_patterns() {
        let g = graph(
            json!([
                {"id":"h","type":"event.player_hurt","params":{}},
                {"id":"fe","type":"flow.for_each_entity","params":{"by":"class","name":"info_target"}},
                {"id":"d","type":"timer.delay","params":{"secs":1}},
                {"id":"rs","type":"event.round_started","params":{}},
                {"id":"rp","type":"timer.repeat","params":{"every":0.2,"times":0}},
                {"id":"hud","type":"msg.hud","params":{"text":"x"}},
                {"id":"l1","type":"flow.for_loop","params":{"first":1,"last":100}},
                {"id":"l2","type":"flow.for_loop","params":{"first":1,"last":100}}
            ]),
            json!([
                {"id":"w1","from":{"node":"h","pin":"then"},"to":{"node":"fe","pin":"exec"}},
                {"id":"w2","from":{"node":"fe","pin":"then"},"to":{"node":"d","pin":"exec"}},
                {"id":"w3","from":{"node":"rs","pin":"then"},"to":{"node":"rp","pin":"exec"}},
                {"id":"w4","from":{"node":"rp","pin":"tick"},"to":{"node":"hud","pin":"exec"}},
                {"id":"w5","from":{"node":"hud","pin":"then"},"to":{"node":"l1","pin":"exec"}},
                {"id":"w6","from":{"node":"l1","pin":"body"},"to":{"node":"l2","pin":"exec"}}
            ]),
        );
        let w: Vec<(String, String)> = cost_warnings(&g, &View::new(&g)).into_iter().map(|p| (p.node.unwrap_or_default(), p.message)).collect();
        let has = |node: &str, words: &str| w.iter().any(|(n, m)| n == node && m.contains(words));
        assert!(has("fe", "every entity"), "{w:#?}");
        assert!(has("d", "Retrigger"), "{w:#?}");
        assert!(has("rp", "times a second"), "{w:#?}");
        assert!(has("l2", "loop inside a loop"), "{w:#?}");
        // advice only: never an error
        assert!(cost_warnings(&g, &View::new(&g)).iter().all(|p| p.level == "warning"));
    }

    #[test]
    fn timeline_cost_counts_copies_and_ticks() {
        let tl = |tick: f64| json!([{"id": "t1", "name": "Pulse", "length": 2, "tick": tick, "tracks": []}]);
        let mk = |tick: f64, per_player: bool| {
            let mut g = v3(json!({
                "nodes": [{"id": "r", "type": "event.round_started"}, {"id": "tp", "type": "timeline.play", "params": {"timeline": "t1"}}, {"id": "p", "type": "debug.print"}],
                "edges": [{"id": "w1", "from": {"node": "r", "pin": "then"}, "to": {"node": "tp", "pin": "exec"}}, {"id": "w2", "from": {"node": "tp", "pin": "update"}, "to": {"node": "p", "pin": "exec"}}],
            }));
            g.timelines = serde_json::from_value(tl(tick)).unwrap();
            if per_player {
                g.functions = serde_json::from_value(json!([{"id": "f", "name": "Glow", "inputs": [{"id": "who", "name": "Who", "type": "player"}], "outputs": []}])).unwrap();
                for n in g.nodes.iter_mut().filter(|n| n.id == "tp" || n.id == "p") {
                    n.graph = Some("f".into());
                }
            }
            cost_warnings(&g, &View::new(&g)).into_iter().filter(|p| p.node.as_deref() == Some("tp")).map(|p| p.message).collect::<Vec<_>>()
        };
        // one copy at 10 a second: fine; one per player: 320 a second (a tick under 0.1 s runs at
        // 0.1 s, AMX Mod X's shortest repeat, so 0.05 counts the same)
        assert!(mk(0.1, false).is_empty(), "{:?}", mk(0.1, false));
        let w = mk(0.1, true);
        assert!(w.iter().any(|m| m.contains("per player") && m.contains("320")), "{w:?}");
        assert_eq!(mk(0.05, true), w);
        // a slower tick per player is fine
        assert!(mk(0.2, true).is_empty(), "{:?}", mk(0.2, true));
    }

    #[test]
    fn loops_with_break_and_gates() {
        // a body wired back into Break, a Gate opened from later in its own flow: not a flow loop
        let g = v3(json!({"nodes": [{"id": "r", "type": "event.round_started"}, {"id": "l", "type": "flow.for_loop_break", "params": {"last": 3}},
                      {"id": "b", "type": "flow.branch"}, {"id": "g", "type": "flow.gate"}, {"id": "p", "type": "debug.print"}],
            "edges": [e("w1","r","then","l","exec"), e("w2","l","body","b","exec"), e("w3","b","true","l","break"), e("w4","l","then","g","exec"), e("w5","g","then","p","exec"), e("w6","p","then","g","open")]}));
        assert!(validate_core(&g).is_empty(), "{:?}", msgs(&validate_core(&g)));
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
