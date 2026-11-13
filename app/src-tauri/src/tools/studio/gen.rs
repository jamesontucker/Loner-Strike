//! Graph → Pawn. A checked graph becomes one AMX Mod X plugin (`studio_<mode id>.sma`) built on
//! `loner.inc`: one function per flow node (`lsg_n<index>`), called along the flow wires, with
//! the event's values (victim, attacker...) passed down in a small `ctx` array. Values from pure
//! nodes (Compare, Math, Health Of...) are worked out where they're used, as expressions.
//! Event handlers are registered once per event kind and run every event node of that kind, in
//! graph order. Timers carry `ctx` through `set_task`.
//!
//! Never graph text as code: function and variable names are ours (`lsg_n3`, `g_v1`), names
//! of things only appear in comments after `comment_text`, every string literal goes through
//! `pawn_str` (after the same filter the checks use), choices come from the registry, numbers
//! are clamped. The checks refuse anything else before it gets here; this is the second guard.
//!
//! With `debug`, the plugin also carries `debug_runtime.pawn` and every node reports itself:
//! `node_enter` (with its inputs), `node_exit` (with its result), `node_error`. A node marked as
//! a breakpoint freezes the world instead of running (`breakpoint_hit`, `world_paused`) until
//! the app says `resume`; then it runs and the flow carries on. Timers wait while paused.
//! Without `debug` none of that is in the plugin (no cost on real servers).
//!
//! The channel is plain files in the game's `addons/amxmodx/data/studio/` (debug.rs):
//! `<mode>.events` (one JSON object per line), `<mode>.cmd` (commands, read and deleted every
//! 0.25 s) and `<mode>.bp` (the breakpoints, read when a map loads). Files work the same on
//! Windows and under CrossOver, need no ports, rcon password or network, and survive the app
//! restarting.

use super::graph::{container, param, Function, Graph, Node, NodeDef, Variable, View, LOOP_LIMIT, NUMBER_LIMIT};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;

/// How many event cell values a flow can carry (victim, attacker...).
const CTX: usize = 4;
/// How many event text values (a creature's type) a flow can carry.
const TXT: usize = 2;
/// Task ids for timers: LSG_TASK + the node's index.
const TASK_BASE: u32 = 7_310_000;

/// The plugin's file name (no extension) for a mode.
pub fn plugin_name(mode_id: &str) -> String {
    format!("studio_{mode_id}")
}

/// Which lines of the generated source belong to which node (1-based, inclusive), so a
/// compiler error can be shown on the node it came from.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Span {
    pub node: String,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Generated {
    pub source: String,
    pub spans: Vec<Span>,
}

impl Generated {
    /// The node a source line belongs to.
    pub fn node_at(&self, line: usize) -> Option<&str> {
        self.spans.iter().find(|s| (s.from..=s.to).contains(&line)).map(|s| s.node.as_str())
    }
}

struct Out {
    text: String,
    lines: usize,
}

impl Out {
    fn line(&mut self, s: impl AsRef<str>) {
        for l in s.as_ref().split('\n') {
            self.text.push_str(l);
            self.text.push('\n');
            self.lines += 1;
        }
    }
    fn blank(&mut self) {
        self.line("");
    }
}

// ---- text that reaches the source -----------------------------------------------------------

/// A Pawn string literal: AMX Mod X escapes with `^`; quotes and control characters never get
/// this far (the checks refuse them), but they're dropped here too.
pub fn pawn_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '^' => o.push_str("^^"),
            '"' => {}
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Text for a `//` or `/* */` comment in generated code: only the characters names may use
/// (graph::label_char), so it can never end the comment or start a new line of code.
pub fn comment_text(s: &str) -> String {
    safe_text(s, 80)
}

/// An id from the graph (mode, node, variable) for a string literal or a comment: letters,
/// digits, _ only (the checks allow nothing else; this is the generator's own guard).
pub fn id_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(32).collect()
}

/// Graph text for a string literal: only the characters names may use, then `pawn_str`.
pub fn safe_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| super::graph::label_char(*c)).take(max).collect()
}

/// An entity name or creature type for a string literal.
fn name_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || "_-.".contains(*c)).take(32).collect()
}

/// An entity's name as the map has it (spaces kept) for a string literal.
fn ent_name_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || " _-.".contains(*c)).take(32).collect()
}

/// Component keys for a string literal.
fn keys_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || " _-.=:,".contains(*c)).take(120).collect()
}

/// A Pawn float literal, finite and inside the graph's limits.
fn float_lit(x: f64) -> String {
    let x = if x.is_finite() { x.clamp(-NUMBER_LIMIT, NUMBER_LIMIT) } else { 0.0 };
    let s = format!("{x:.3}");
    let s = s.trim_end_matches('0');
    if s.ends_with('.') { format!("{s}0") } else { s.to_string() }
}

fn team_lit(t: &str) -> &'static str {
    match t {
        "t" => "1",
        "ct" => "2",
        _ => "0",
    }
}

const HUD_SLOTS: [&str; 4] = ["top", "left", "right", "center"];

// ---- events ---------------------------------------------------------------------------------

/// How an event kind is caught and what it puts into `ctx`.
struct EventKind {
    kind: &'static str,
    /// the handler's heading
    sig: &'static str,
    /// lines for plugin_init (none for forwards)
    register: &'static [&'static str],
    /// code before the handler (wrappers)
    prelude: &'static str,
    /// the handler's first lines (skip conditions)
    guard: &'static str,
    /// output pin -> how it's read (cells; text: copied into a text slot)
    values: &'static [(&'static str, &'static str)],
    /// the handler's last line
    tail: &'static str,
    /// a `name` param filters which nodes run (by the entity's targetname)
    named: Option<&'static str>,
}

const EVENTS: &[EventKind] = &[
    EventKind { kind: "event.round_started", sig: "public lsg_on_round_started()", register: &["register_logevent(\"lsg_on_round_started\", 2, \"1=Round_Start\")"], prelude: "", guard: "", values: &[], tail: "", named: None },
    EventKind {
        kind: "event.round_ended",
        sig: "lsg_on_round_ended(winner)",
        register: &[
            "register_event(\"SendAudio\", \"lsg_on_round_end_t\", \"a\", \"2&%!MRAD_terwin\")",
            "register_event(\"SendAudio\", \"lsg_on_round_end_ct\", \"a\", \"2&%!MRAD_ctwin\")",
            "register_event(\"SendAudio\", \"lsg_on_round_end_draw\", \"a\", \"2&%!MRAD_rounddraw\")",
        ],
        prelude: "public lsg_on_round_end_t()\n\tlsg_on_round_ended(1)\n\npublic lsg_on_round_end_ct()\n\tlsg_on_round_ended(2)\n\npublic lsg_on_round_end_draw()\n\tlsg_on_round_ended(0)\n",
        guard: "",
        values: &[("winner", "winner")],
        tail: "",
        named: None,
    },
    EventKind {
        kind: "event.player_killed",
        sig: "public lsg_on_player_killed()",
        register: &["register_event(\"DeathMsg\", \"lsg_on_player_killed\", \"a\")"],
        prelude: "",
        guard: "",
        values: &[("victim", "read_data(2)"), ("attacker", "read_data(1)"), ("headshot", "read_data(3) ? 1 : 0")],
        tail: "",
        named: None,
    },
    EventKind {
        kind: "event.player_spawned",
        sig: "public lsg_on_player_spawned(id)",
        register: &["RegisterHamPlayer(Ham_Spawn, \"lsg_on_player_spawned\", 1)"],
        prelude: "",
        guard: "\tif (!is_user_alive(id))\n\t\treturn HAM_IGNORED",
        values: &[("player", "id")],
        tail: "\treturn HAM_IGNORED",
        named: None,
    },
    EventKind { kind: "event.player_joined", sig: "lsg_on_player_joined(id)", register: &[], prelude: "", guard: "", values: &[("player", "id")], tail: "", named: None },
    EventKind { kind: "event.player_left", sig: "lsg_on_player_left(id)", register: &[], prelude: "", guard: "", values: &[("player", "id")], tail: "", named: None },
    EventKind {
        kind: "event.bomb_planted",
        sig: "public lsg_on_bomb_planted()",
        register: &["register_logevent(\"lsg_on_bomb_planted\", 3, \"2=Planted_The_Bomb\")"],
        prelude: "",
        guard: "",
        values: &[("player", "lsg_log_player()")],
        tail: "",
        named: None,
    },
    EventKind {
        kind: "event.bomb_defused",
        sig: "public lsg_on_bomb_defused()",
        register: &["register_logevent(\"lsg_on_bomb_defused\", 3, \"2=Defused_The_Bomb\")"],
        prelude: "",
        guard: "",
        values: &[("player", "lsg_log_player()")],
        tail: "",
        named: None,
    },
    EventKind { kind: "event.bomb_exploded", sig: "public lsg_on_bomb_exploded()", register: &["register_logevent(\"lsg_on_bomb_exploded\", 6, \"3=Target_Bombed\")"], prelude: "", guard: "", values: &[], tail: "", named: None },
    EventKind {
        kind: "event.player_used",
        sig: "public lsg_on_player_used(ent, caller, activator)",
        register: &["RegisterHam(Ham_Use, \"func_button\", \"lsg_on_player_used\", 0)"],
        prelude: "",
        guard: "\tif (activator < 1 || activator > MaxClients)\n\t\treturn HAM_IGNORED",
        values: &[("player", "activator"), ("entity", "ent")],
        tail: "\treturn HAM_IGNORED",
        named: Some("ent"),
    },
    EventKind {
        kind: "event.player_touched",
        sig: "public lsg_on_player_touched(ent, id)",
        register: &["register_touch(\"trigger_multiple\", \"player\", \"lsg_on_player_touched\")"],
        prelude: "",
        guard: "\tstatic Float:last[33]\n\tif (id < 1 || id > MaxClients || get_gametime() - last[id] < 1.0)\n\t\treturn PLUGIN_CONTINUE\n\tlast[id] = get_gametime()",
        values: &[("player", "id"), ("entity", "ent")],
        tail: "\treturn PLUGIN_CONTINUE",
        named: Some("ent"),
    },
    EventKind {
        kind: "event.player_hurt",
        sig: "public lsg_on_player_hurt(victim, inflictor, attacker, Float:damage, bits)",
        register: &["RegisterHamPlayer(Ham_TakeDamage, \"lsg_hurt_before\", 0)", "RegisterHamPlayer(Ham_TakeDamage, \"lsg_on_player_hurt\", 1)"],
        // the health before the hit, so the event says what was really taken: nothing when the
        // damage was blocked, less when it was scaled down or armour took some
        prelude: "new Float:g_lsgHurtHp[33]\n\npublic lsg_hurt_before(victim, inflictor, attacker, Float:damage, bits)\n{\n\t#pragma unused inflictor\n\t#pragma unused attacker\n\t#pragma unused damage\n\t#pragma unused bits\n\tif (victim >= 1 && victim <= MaxClients)\n\t\tpev(victim, pev_health, g_lsgHurtHp[victim])\n\treturn HAM_IGNORED\n}",
        guard: "\t#pragma unused inflictor\n\t#pragma unused bits\n\tif (victim < 1 || victim > MaxClients)\n\t\treturn HAM_IGNORED\n\tnew Float:hp\n\tpev(victim, pev_health, hp)\n\tdamage = g_lsgHurtHp[victim] - hp\n\tif (damage <= 0.0)\n\t\treturn HAM_IGNORED",
        values: &[("victim", "victim"), ("attacker", "attacker >= 1 && attacker <= MaxClients ? attacker : 0"), ("damage", "_:damage")],
        tail: "\treturn HAM_IGNORED",
        named: None,
    },
    EventKind {
        kind: "event.team_changed",
        sig: "public lsg_on_team_changed()",
        register: &["register_event(\"TeamInfo\", \"lsg_on_team_changed\", \"a\")"],
        prelude: "",
        // (the game sends TeamInfo several times per join, the first "UNASSIGNED", and again on
        // spawns: once per real change; UNASSIGNED starts a slot over for the next player)
        guard: "\tstatic last[33]\n\tnew id = read_data(1), tn[12]\n\tread_data(2, tn, charsmax(tn))\n\tif (id < 1 || id > MaxClients)\n\t\treturn\n\tif (tn[0] == 'U' || !tn[0])\n\t{\n\t\tlast[id] = 0\n\t\treturn\n\t}\n\tnew team = tn[0] == 'T' ? 1 : (tn[0] == 'C' ? 2 : 3)\n\tif (team == last[id])\n\t\treturn\n\tlast[id] = team",
        values: &[("player", "id"), ("team", "team == 3 ? 0 : team")],
        tail: "",
        named: None,
    },
    EventKind { kind: "event.map_started", sig: "public lsg_on_map_started()", register: &["set_task(1.0, \"lsg_on_map_started\")"], prelude: "", guard: "", values: &[], tail: "", named: None },
    EventKind { kind: "event.new_round", sig: "public lsg_on_new_round()", register: &["register_event(\"HLTV\", \"lsg_on_new_round\", \"a\", \"1=0\", \"2=0\")"], prelude: "", guard: "", values: &[], tail: "", named: None },
    EventKind { kind: "comp.on_capture", sig: "public ls_on_capture(ent, side, id)", register: &[], prelude: "", guard: "", values: &[("point", "ent"), ("team", "side == 1 || side == 2 ? side : 0"), ("player", "id")], tail: "", named: None },
    EventKind { kind: "comp.on_capture_lost", sig: "public ls_on_capture_lost(ent, side)", register: &[], prelude: "", guard: "", values: &[("point", "ent"), ("team", "side == 1 || side == 2 ? side : 0")], tail: "", named: None },
    EventKind { kind: "comp.on_zone_stage", sig: "public ls_on_zone_stage(ent, stage)", register: &[], prelude: "", guard: "", values: &[("zone", "ent"), ("stage", "_:float(stage)")], tail: "", named: None },
    EventKind { kind: "comp.on_zone_tick", sig: "public ls_on_zone_tick(ent, id, Float:dmg)", register: &[], prelude: "", guard: "", values: &[("zone", "ent"), ("player", "id"), ("damage", "_:dmg")], tail: "", named: None },
    EventKind { kind: "comp.on_extract", sig: "public ls_on_extract(ent, id)", register: &[], prelude: "", guard: "", values: &[("point", "ent"), ("player", "id")], tail: "", named: None },
    EventKind { kind: "comp.on_wave_start", sig: "public ls_on_wave_start(wave)", register: &[], prelude: "", guard: "", values: &[("wave", "_:float(wave)")], tail: "", named: None },
    EventKind {
        kind: "comp.on_wave_spawn",
        sig: "public ls_on_wave_spawn(ent, wave, const Float:origin[3], const types[])",
        register: &[],
        prelude: "",
        guard: "\t#pragma unused origin\n\t#pragma unused types",
        values: &[("spawned", "ent"), ("wave", "_:float(wave)")],
        tail: "",
        named: None,
    },
    EventKind { kind: "comp.on_wave_cleared", sig: "public ls_on_wave_cleared(wave)", register: &[], prelude: "", guard: "", values: &[("wave", "_:float(wave)")], tail: "", named: None },
    EventKind { kind: "comp.on_loot", sig: "public ls_on_loot(ent, id)", register: &[], prelude: "", guard: "", values: &[("container", "ent"), ("player", "id")], tail: "", named: None },
    EventKind { kind: "comp.on_checkpoint", sig: "public ls_on_checkpoint(ent, id, order)", register: &[], prelude: "", guard: "", values: &[("point", "ent"), ("player", "id"), ("order", "_:float(order)")], tail: "", named: None },
    EventKind { kind: "comp.on_barricade_broken", sig: "public ls_on_barricade_broken(ent, attacker)", register: &[], prelude: "", guard: "", values: &[("barricade", "ent"), ("attacker", "attacker")], tail: "", named: None },
    EventKind { kind: "comp.on_meeting_end", sig: "public ls_on_meeting_end(ent, ejected)", register: &[], prelude: "", guard: "", values: &[("meeting", "ent"), ("ejected", "ejected")], tail: "", named: None },
    EventKind {
        kind: "creature.on_killed",
        sig: "public lsg_on_creature_killed(ent, killer)",
        register: &["if (get_cvar_pointer(\"ls_creature_ver\"))\n\t\tRegisterHam(Ham_Killed, \"ls_creature\", \"lsg_on_creature_killed\", 1)"],
        prelude: "",
        guard: "",
        values: &[("creature", "ent"), ("killer", "killer >= 1 && killer <= MaxClients ? killer : 0"), ("kind", "@netname")],
        tail: "\treturn HAM_IGNORED",
        named: None,
    },
];

fn event_kind(kind: &str) -> Option<&'static EventKind> {
    EVENTS.iter().find(|e| e.kind == kind)
}

/// The slot an event output lives in: (is text, index).
fn event_slot(def: &NodeDef, pin: &str) -> Option<(bool, usize)> {
    let (mut cell, mut txt) = (0, 0);
    for o in def.outputs.iter().filter(|o| o.ty != "exec") {
        let text = o.ty == "text";
        if o.id == pin {
            return Some((text, if text { txt } else { cell }));
        }
        if text { txt += 1 } else { cell += 1 }
    }
    None
}

// ---- the generator --------------------------------------------------------------------------

struct Gen<'a> {
    v: View<'a>,
    /// ctx slots of flow values (loops, picks, a Function's inputs), after the events' own
    /// CTX slots
    slots: Vec<(&'a str, String)>,
    /// kept values of other outputs (`g_lsgO[k]`), and text ones (`g_lsgOT[k]`, also text flow
    /// values: they don't fit in ctx)
    outs: Vec<(&'a str, String)>,
    touts: Vec<(&'a str, String)>,
    /// the nodes that become functions, in graph order (their index is their number)
    live: Vec<&'a str>,
    /// the mode's variables, then every Function's locals, in order (their index is their number)
    vars: Vec<&'a Variable>,
    /// a vector or struct variable's first reserved slot (after the ring), by variable index
    reserved: Vec<usize>,
    /// a struct's layout: each field's cell offset (vectors take 3) or text slot
    layouts: Vec<Vec<usize>>,
    /// a Macro's copied nodes -> the Macro node they stand for (for the debugger)
    origin: &'a std::collections::HashMap<String, String>,
    /// (debug builds) the node being written: its inputs worked out once into locals, so the
    /// debugger shows the very values the node uses
    pinned: std::cell::RefCell<Vec<(String, String, String)>>,
    debug: bool,
}

/// Where values of a type live in the plugin: c (a cell), s (text), v (a vector's slot), r (a
/// struct's slot).
fn kind_of(ty: &str) -> char {
    match ty {
        "text" => 's',
        "vector" => 'v',
        t if t.starts_with("struct:") => 'r',
        _ => 'c',
    }
}

/// Outputs kept in ctx (they go down the flow: loops, picks, a Function's inputs).
const CTX_KINDS: [&str; 9] = ["flow.for_each_player", "flow.for_each_entity", "flow.pick_player", "flow.pick_entity", "func.define", "fn.entry", "flow.for_loop", "flow.for_loop_break", "flow.for_each_array"];
/// Outputs kept in g_lsgO / g_lsgOT (what an action made).
const STORE_KINDS: [&str; 5] = ["array.add", "array.remove", "flow.do_n", "flow.flipflop", "fn.call"];
/// Nodes with more than one flow input: one Pawn function each.
const MULTI_IN: [&str; 5] = ["flow.do_once", "flow.do_n", "flow.gate", "flow.for_loop_break", "flow.for_each_array"];
/// The vector and struct rings: values that aren't in variables live here for a while.
const VEC_RING: usize = 256;
const REC_RING: usize = 64;
/// Arrays made where they're used (Make Array, Players Array, Map Keys...) take turns in this
/// many temporary arrays.
const TMP_ARRAYS: usize = 32;

/// A worked-out data value: the Pawn expression and its type.
struct Val {
    expr: String,
    ty: String,
}

impl<'a> Gen<'a> {
    fn slot(&self, node: &str, pin: &str) -> Option<usize> {
        self.slots.iter().position(|(n, p)| *n == node && p == pin).map(|i| CTX + i)
    }
    fn index(&self, id: &str) -> usize {
        self.live.iter().position(|x| *x == id).unwrap_or(0)
    }
    fn fname(&self, id: &str) -> String {
        format!("lsg_n{}", self.index(id))
    }
    /// The function a flow wire into this pin calls (a node with several flow inputs has one
    /// per input; the first is the node's own).
    fn pin_fname(&self, id: &str, pin: &str) -> String {
        let def = self.v.defs[id];
        if MULTI_IN.contains(&def.kind.as_str()) && pin != "exec" {
            format!("lsg_n{}_{}", self.index(id), id_text(pin))
        } else {
            self.fname(id)
        }
    }
    fn out_index(&self, node: &str, pin: &str) -> Option<usize> {
        self.outs.iter().position(|(n, p)| *n == node && p == pin)
    }
    fn tout_index(&self, node: &str, pin: &str) -> Option<usize> {
        self.touts.iter().position(|(n, p)| *n == node && p == pin)
    }
    /// The Function (index, definition) a node's graph belongs to.
    fn home_fn(&self, id: &str) -> Option<(usize, &'a Function)> {
        let home = self.v.graph_of(id);
        self.v.g.functions.iter().enumerate().find(|(_, f)| f.id == home)
    }
    fn fn_index(&self, f: &str) -> usize {
        self.v.g.functions.iter().position(|x| x.id == f).unwrap_or(0)
    }
    fn struct_index(&self, s: &str) -> usize {
        self.v.g.structs.iter().position(|x| x.id == s).unwrap_or(0)
    }
    fn enum_index(&self, e: &str) -> usize {
        self.v.g.enums.iter().position(|x| x.id == e).unwrap_or(0)
    }
    /// The id the debugger knows a node by (a Macro's copies are the Macro node).
    fn report_id(&self, id: &str) -> String {
        id_text(self.origin.get(id).map(String::as_str).unwrap_or(id))
    }
    fn var_index(&self, id: &str) -> usize {
        self.vars.iter().position(|x| x.id == id).unwrap_or(0)
    }
    fn node(&self, id: &str) -> (&'a Node, &'static NodeDef) {
        (self.v.nodes[id], self.v.defs[id])
    }

    fn num(&self, n: &Node, def: &NodeDef, id: &str) -> f64 {
        let p = def.param(id);
        let v = param(n, def, id).and_then(Value::as_f64).filter(|x| x.is_finite()).or_else(|| p.and_then(|p| p.default.as_f64())).unwrap_or(0.0);
        v.clamp(p.and_then(|p| p.min).unwrap_or(-NUMBER_LIMIT), p.and_then(|p| p.max).unwrap_or(NUMBER_LIMIT))
    }

    fn boolean(&self, n: &Node, def: &NodeDef, id: &str) -> bool {
        param(n, def, id).and_then(Value::as_bool).or_else(|| def.param(id).and_then(|p| p.default.as_bool())).unwrap_or(false)
    }

    /// A choice setting: one of the registry's own choices (never the graph's text), or the default.
    fn choice(&self, n: &Node, def: &'static NodeDef, id: &str) -> &'static str {
        let Some(p) = def.param(id) else { return "" };
        let want = n.params.get(id).and_then(Value::as_str);
        p.choices.iter().find(|(k, _)| Some(k.as_str()) == want).or_else(|| p.choices.iter().find(|(k, _)| Some(k.as_str()) == p.default.as_str())).map(|(k, _)| k.as_str()).unwrap_or("")
    }

    fn text(&self, n: &'a Node, def: &'static NodeDef, id: &str) -> &'a str {
        n.params.get(id).and_then(Value::as_str).or_else(|| def.param(id).and_then(|p| p.default.as_str())).unwrap_or("")
    }

    /// A literal of a variable's type from a param value.
    fn var_lit(ty: &str, v: Option<&Value>) -> String {
        match ty {
            "number" => float_lit(v.and_then(Value::as_f64).unwrap_or(0.0)),
            "bool" => if v.and_then(Value::as_bool).unwrap_or(false) { "1".into() } else { "0".into() },
            "text" => pawn_str(&safe_text(v.and_then(Value::as_str).unwrap_or(""), 60)),
            t if t.starts_with("enum:") => (v.and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 31.0) as i64).to_string(),
            _ => "0".into(),
        }
    }

    /// An empty value of a type (an input nothing is wired into).
    fn zero(ty: &str) -> String {
        match kind_of(ty) {
            's' => "\"\"".into(),
            'v' => "lsg_vec(0.0, 0.0, 0.0)".into(),
            'r' => "lsg_rnew()".into(),
            _ if ty == "number" => "0.0".into(),
            _ => "0".into(),
        }
    }

    /// A value as an argument for a cell (an array item, a map value): numbers lose their tag.
    fn cell_arg(ty: &str, expr: &str) -> String {
        if ty == "number" { format!("_:({expr})") } else { expr.to_string() }
    }

    /// A cell read back as its type (a number gets its tag again).
    fn cell_read(ty: &str, expr: String) -> String {
        if ty == "number" { format!("Float:{expr}") } else { expr }
    }

    /// The statement that puts a variable back to its start value: `at` is the player or team
    /// index for per-player and per-team ones.
    fn reset_line(&self, x: &Variable, at: &str) -> String {
        let i = self.var_index(&x.id);
        let init = Self::var_lit(&x.ty, Some(&x.default).filter(|d| !d.is_null()));
        let r = match x.scope.as_str() {
            "player" => format!("g_pv{i}[{at}]"),
            "team" => format!("g_tv{i}[{at}]"),
            _ => format!("g_v{i}"),
        };
        match (kind_of(&x.ty), container(&x.ty)) {
            ('s', _) => format!("copy({r}, 63, {init})"),
            ('v', _) => format!("lsg_vzero({})", self.var_ref(x, at, at)),
            ('r', _) => format!("lsg_rzero({})", self.var_ref(x, at, at)),
            (_, Some(("array", _, _))) => format!("lsg_aclear({r})"),
            (_, Some(("map", _, _))) => format!("lsg_mclear({r})"),
            _ => format!("{r} = {init}"),
        }
    }

    /// The lines that put a variable back to its start value (all players or teams).
    fn reset_all(&self, x: &Variable) -> String {
        match x.scope.as_str() {
            "player" => format!("\tfor (new id = 0; id <= 32; id++)\n\t\t{}", self.reset_line(x, "id")),
            "team" => format!("\tfor (new t = 0; t < 4; t++)\n\t\t{}", self.reset_line(x, "t")),
            _ => format!("\t{}", self.reset_line(x, "")),
        }
    }

    /// Setting a variable to a value (copies for text, vectors, structs, arrays and maps).
    fn set_var(&self, var: &Variable, r: &str, val: &str) -> String {
        match (kind_of(&var.ty), container(&var.ty)) {
            ('s', _) => format!("copy({r}, 63, {val})"),
            ('v', _) => format!("lsg_vset({r}, {val})"),
            ('r', _) => format!("lsg_rset({r}, {val})"),
            (_, Some(("array", _, _))) => format!("lsg_acopy({r}, {val})"),
            (_, Some(("map", _, item))) => format!("lsg_mcopy({r}, {val}, {})", map_kind(item)),
            _ if var.ty == "bool" => format!("{r} = {}", bool01(val)),
            _ => format!("{r} = {val}"),
        }
    }

    /// Where a variable lives: `g_v1`, `g_pv1[lsg_slot(P)]` (per player) or
    /// `g_tv1[clamp(T, 0, 3)]` (per team).
    fn var_ref(&self, var: &Variable, player: &str, team: &str) -> String {
        let i = self.var_index(&var.id);
        let k = kind_of(&var.ty);
        if k == 'v' || k == 'r' {
            // a vector or a struct: its own slots after the ring
            let ring = if k == 'v' { "LSG_VR" } else { "LSG_RR" };
            let base = self.reserved.get(i).copied().unwrap_or(0);
            return match var.scope.as_str() {
                "player" => format!("({ring} + {base} + lsg_slot({player}))"),
                "team" => format!("({ring} + {base} + clamp({team}, 0, 3))"),
                _ => format!("({ring} + {base})"),
            };
        }
        match var.scope.as_str() {
            "player" => format!("g_pv{i}[lsg_slot({player})]"),
            "team" => format!("g_tv{i}[clamp({team}, 0, 3)]"),
            _ => format!("g_v{i}"),
        }
    }

    /// The value of a data input: what's wired in, or the param it falls back to.
    fn input(&self, node: &str, pin: &str) -> Option<Val> {
        if let Some((_, _, local)) = self.pinned.borrow().iter().find(|(n, p, _)| n == node && p == pin) {
            return Some(Val { expr: local.clone(), ty: self.v.in_type(node, pin).filter(|t| t != "any").unwrap_or_else(|| self.input_unpinned_ty(node, pin)) });
        }
        self.input_unpinned(node, pin)
    }

    fn input_unpinned_ty(&self, node: &str, pin: &str) -> String {
        self.input_unpinned(node, pin).map(|v| v.ty).unwrap_or_else(|| "any".into())
    }

    fn input_unpinned(&self, node: &str, pin: &str) -> Option<Val> {
        let (n, def) = self.node(node);
        let want = self.v.in_type(node, pin).unwrap_or_else(|| "any".into());
        if let Some((src, spin)) = self.v.data_source(node, pin) {
            return Some(self.output(src, spin));
        }
        let ip = self.v.input(node, pin)?;
        let prm = ip.param.as_deref()?;
        let pdef = def.param(prm)?;
        let v = param(n, def, prm);
        let expr = match (pdef.kind.as_str(), want.as_str()) {
            ("varvalue", ty) => Self::var_lit(ty, n.params.get(prm).filter(|x| x.as_str() != Some(""))),
            ("choice", "team") => team_lit(self.choice(n, def, prm)).into(),
            ("number", _) => float_lit(self.num(n, def, prm)),
            ("bool", _) => if self.boolean(n, def, prm) { "1".into() } else { "0".into() },
            ("text", _) => pawn_str(&safe_text(v.and_then(Value::as_str).unwrap_or(""), 100)),
            _ => return None,
        };
        Some(Val { expr, ty: want })
    }

    /// An input that must have a value (required pins are checked; this is the fallback).
    fn need(&self, node: &str, pin: &str, ty: &str) -> String {
        self.input(node, pin).map(|v| v.expr).unwrap_or_else(|| Self::zero(ty))
    }

    /// An input of the type the pin wants (generic pins: their node's type).
    fn needt(&self, node: &str, pin: &str) -> String {
        let ty = self.v.in_type(node, pin).unwrap_or_else(|| "any".into());
        self.need(node, pin, &ty)
    }

    /// A map key as text: text as it is, a number written out, a player by their user id.
    fn key_expr(&self, node: &str) -> String {
        let k = self.needt(node, "key");
        match self.v.in_type(node, "key").as_deref() {
            Some("number") => format!("lsg_kn({k})"),
            Some("player") => format!("lsg_kp({k})"),
            _ => k,
        }
    }

    /// A Function's arguments from a call node's inputs, in the Function's order.
    fn call_args(&self, node: &str, f: &Function) -> String {
        f.inputs.iter().map(|m| self.need(node, &m.id, &m.ty)).collect::<Vec<_>>().join(", ")
    }

    /// The value of an output: an event's (from ctx), a pure node's (an expression), or an
    /// action's (the last value it made).
    fn output(&self, node: &str, pin: &str) -> Val {
        let (n, def) = self.node(node);
        let ty = self.v.out_type(node, pin).unwrap_or_else(|| "any".into());
        if let Some(k) = self.slot(node, pin) {
            let expr = if ty == "number" { format!("Float:ctx[{k}]") } else { format!("ctx[{k}]") };
            return Val { expr, ty };
        }
        if let Some(k) = self.tout_index(node, pin) {
            return Val { expr: format!("g_lsgOT[{k}]"), ty };
        }
        if let Some(k) = self.out_index(node, pin) {
            return Val { expr: Self::cell_read(&ty, format!("g_lsgO[{k}]")), ty };
        }
        if pin == "handle" && !def.pure {
            return Val { expr: format!("(LSG_TASK + {})", self.index(node)), ty };
        }
        if def.event {
            let expr = match event_slot(def, pin) {
                Some((true, i)) => format!("g_lsgTxt[{i}]"),
                Some((false, i)) if ty == "number" => format!("Float:ctx[{i}]"),
                Some((false, i)) => format!("ctx[{i}]"),
                None => "0".into(),
            };
            return Val { expr, ty };
        }
        if !def.pure {
            return Val { expr: format!("g_lsgOut[{}]", self.index(node)), ty };
        }
        let i = |p: &str, t: &str| self.need(node, p, t);
        let it = |p: &str| self.needt(node, p);
        // the item type of an array or map node, and where its items live
        let item = self.v.in_type(node, "item").or_else(|| self.v.out_type(node, "out")).unwrap_or_default();
        let ik = kind_of(&item);
        let expr = match def.kind.as_str() {
            "math.compare" => {
                let op = match self.choice(n, def, "op") {
                    "ge" => ">=",
                    "lt" => "<",
                    "le" => "<=",
                    "eq" => "==",
                    "ne" => "!=",
                    _ => ">",
                };
                format!("(({}) {op} ({}) ? 1 : 0)", i("a", "number"), i("b", "number"))
            }
            "logic.and_or" => format!("(({}) {} ({}) ? 1 : 0)", i("a", "bool"), if self.choice(n, def, "op") == "or" { "||" } else { "&&" }, i("b", "bool")),
            "logic.not" => format!("(({}) ? 0 : 1)", i("a", "bool")),
            "math.op" => {
                let (a, b) = (i("a", "number"), i("b", "number"));
                match self.choice(n, def, "op") {
                    "sub" => format!("(({a}) - ({b}))"),
                    "mul" => format!("(({a}) * ({b}))"),
                    "div" => format!("lsg_div({a}, {b})"),
                    "min" => format!("lsg_min({a}, {b})"),
                    "max" => format!("lsg_max({a}, {b})"),
                    "mod" => format!("lsg_mod({a}, {b})"),
                    "pow" => format!("lsg_pow({a}, {b})"),
                    _ => format!("(({a}) + ({b}))"),
                }
            }
            "math.random" => format!("lsg_random({}, {}, {})", i("min", "number"), i("max", "number"), if self.boolean(n, def, "frac") { 1 } else { 0 }),
            "player.get_health" => format!("lsg_health({})", i("player", "player")),
            "player.get_team" => format!("lsg_team({})", i("player", "player")),
            "player.is_alive" => format!("(ls_alive({}) ? 1 : 0)", i("player", "player")),
            "player.is_bot" => format!("lsg_is_bot({})", i("player", "player")),
            "player.get_name" => format!("lsg_name({})", i("player", "player")),
            "game.count_alive" => format!("float(ls_count_alive(CsTeams:({})))", i("team", "team")),
            "var.get" => match self.v.var_of(node) {
                Some(var) => self.var_ref(var, &i("player", "player"), &i("team", "team")),
                None => "0".into(),
            },
            "text.equals" => format!("(equali({}, {}) ? 1 : 0)", i("a", "text"), i("b", "text")),
            "player.get_money" => format!("lsg_get_money({})", i("player", "player")),
            "player.get_frags" => format!("lsg_get_frags({})", i("player", "player")),
            "player.get_deaths" => format!("lsg_get_deaths({})", i("player", "player")),
            "player.is_role" => format!("(ls_is_role({}, {}) ? 1 : 0)", i("player", "player"), pawn_str(&name_text(self.text(n, def, "role")))),
            "game.count_role" => format!("float(ls_count_role({}))", pawn_str(&name_text(self.text(n, def, "role")))),
            "comp.in_zone" => format!("(ls_components_running() && ls_zone_inside({}, {}) ? 1 : 0)", i("zone", "entity"), i("player", "player")),
            "creature.count" => "float(lsg_creatures_alive())".into(),
            "player.is_downed" => format!("(g_lsgDown[lsg_slot({})] ? 1 : 0)", i("player", "player")),
            "squad.get" => format!("float(g_lsgSquad[lsg_slot({})])", i("player", "player")),
            "squad.alive" => "float(lsg_squads_alive())".into(),
            "save.get" => format!("lsg_saved_get({}, {})", i("player", "player"), pawn_str(&name_text(self.text(n, def, "key")))),
            "clock.left" => "lsg_clock_left()".into(),
            "comp.find" => format!("lsg_find({})", pawn_str(&ent_name_text(self.text(n, def, "name")))),
            // graph version 3
            "logic.equal" => {
                let t = self.v.in_type(node, "a").unwrap_or_default();
                let (a, b) = (it("a"), it("b"));
                let eq = match kind_of(&t) {
                    's' => format!("equal({a}, {b})"),
                    'v' => format!("lsg_veq({a}, {b})"),
                    'r' => format!("lsg_req({a}, {b})"),
                    _ => format!("({a}) == ({b})"),
                };
                if self.choice(n, def, "op") == "ne" { format!("({eq} ? 0 : 1)") } else { format!("({eq} ? 1 : 0)") }
            }
            "logic.select" => {
                let (c, a, b) = (bool01(&i("cond", "bool")), it("a"), it("b"));
                if kind_of(&ty) == 's' { format!("lsg_tsel({c}, {a}, {b})") } else { format!("({c} ? ({a}) : ({b}))") }
            }
            "cond.is_valid" => format!("(lsg_valid({}) ? 1 : 0)", i("in", "entity")),
            "player.is_player" => format!("(lsg_ok({}) ? 1 : 0)", i("in", "entity")),
            "creature.is" => format!("(lsg_is_creature({}) ? 1 : 0)", i("in", "entity")),
            "timer.is_active" => format!("(lsg_tactive({}) ? 1 : 0)", i("handle", "timer")),
            "math.fn" => {
                let a = i("a", "number");
                match self.choice(n, def, "op") {
                    "round" => format!("float(floatround({a}))"),
                    "floor" => format!("float(floatround({a}, floatround_floor))"),
                    "ceil" => format!("float(floatround({a}, floatround_ceil))"),
                    "sqrt" => format!("floatsqroot(floatmax({a}, 0.0))"),
                    "neg" => format!("(-({a}))"),
                    "sign" => format!("lsg_sign({a})"),
                    _ => format!("floatabs({a})"),
                }
            }
            "math.clamp" => format!("lsg_clampf({}, {}, {})", i("a", "number"), i("min", "number"), i("max", "number")),
            "math.lerp" => {
                let (a, b, t) = (i("a", "number"), i("b", "number"), i("alpha", "number"));
                format!("(({a}) + (({b}) - ({a})) * ({t}))")
            }
            "math.in_range" => {
                let x = i("a", "number");
                format!("(({x}) >= ({}) && ({x}) <= ({}) ? 1 : 0)", i("min", "number"), i("max", "number"))
            }
            "math.number" => float_lit(self.num(n, def, "value")),
            "text.literal" => pawn_str(&safe_text(self.text(n, def, "value"), 100)),
            "text.append" => format!("lsg_tcat({}, {})", i("a", "text"), i("b", "text")),
            "text.format" => format_text(self, node, self.text(n, def, "text")),
            "text.contains" => format!("(containi({}, {}) >= 0 ? 1 : 0)", i("a", "text"), i("b", "text")),
            "text.length" => format!("float(strlen({}))", i("a", "text")),
            "text.to_number" => format!("str_to_float({})", i("a", "text")),
            "text.from" => to_text(self, &Val { expr: it("a"), ty: self.v.in_type(node, "a").unwrap_or_default() }),
            "vec.make" => format!("lsg_vec({}, {}, {})", i("x", "number"), i("y", "number"), i("z", "number")),
            "vec.break" => format!("lsg_vc({}, {})", i("in", "vector"), match pin { "y" => 1, "z" => 2, _ => 0 }),
            "vec.op" => format!("lsg_vadd({}, {}, {})", i("a", "vector"), i("b", "vector"), if self.choice(n, def, "op") == "sub" { "-1.0" } else { "1.0" }),
            "vec.scale" => format!("lsg_vscale({}, {})", i("a", "vector"), i("n", "number")),
            "vec.length" => format!("lsg_vlen({})", i("a", "vector")),
            "vec.distance" => format!("lsg_vdist({}, {})", i("a", "vector"), i("b", "vector")),
            "vec.direction" => format!("lsg_vdir({}, {})", i("from", "vector"), i("to", "vector")),
            "vec.normalize" => format!("lsg_vnorm({})", i("a", "vector")),
            "vec.player" => {
                let what = ["origin", "eyes", "aim", "aim_point", "velocity"].iter().position(|w| *w == self.choice(n, def, "what")).unwrap_or(0);
                format!("lsg_vplayer({}, {what})", i("player", "player"))
            }
            "vec.entity" => format!("lsg_vent({})", i("in", "entity")),
            "struct.make" => match self.v.struct_of(node) {
                Some(st) => format!("lsg_mks{}({})", self.struct_index(&st.id), st.fields.iter().map(|f| self.need(node, &f.id, &f.ty)).collect::<Vec<_>>().join(", ")),
                None => "lsg_rnew()".into(),
            },
            "struct.break" => match self.v.struct_of(node) {
                Some(st) => {
                    let r = i("in", &format!("struct:{}", st.id));
                    let lay = &self.layouts[self.struct_index(&st.id)];
                    match st.fields.iter().position(|f| f.id == pin) {
                        Some(k) => field_read(&st.fields[k].ty, &r, lay[k]),
                        None => "0".into(),
                    }
                }
                None => "0".into(),
            },
            "enum.value" => (self.num(n, def, "value").max(0.0) as i64).to_string(),
            "enum.to_number" => format!("float({})", it("in")),
            "array.make" => format!("lsg_mka{}({})", self.v.g.nodes.iter().position(|x| x.id == node).unwrap_or(0), (0..self.v.inputs(node).len()).map(|k| self.needt(node, &format!("i{k}"))).collect::<Vec<_>>().join(", ")),
            "array.players" => format!("lsg_aplayers({})", which_code(self.choice(n, def, "which"))),
            "array.get" => {
                let item = ty.clone();
                Self::cell_read(&item, format!("lsg_aget{}({}, {})", kind_of(&item), it("array"), i("at", "number")))
            }
            "array.length" => format!("float(lsg_alen({}))", it("array")),
            "array.contains" => format!("(lsg_afind{ik}({}, {}) >= 0 ? 1 : 0)", it("array"), Self::cell_arg(&item, &it("item"))),
            "array.find" => format!("float(lsg_afind{ik}({}, {}))", it("array"), Self::cell_arg(&item, &it("item"))),
            "array.random" => Self::cell_read(&ty, format!("lsg_arand{}({})", kind_of(&ty), it("array"))),
            "map.find" if pin == "found" => format!("(lsg_mhas({}, {}) ? 1 : 0)", it("map"), self.key_expr(node)),
            "map.find" => Self::cell_read(&ty, format!("lsg_mget{}({}, {})", kind_of(&ty), it("map"), self.key_expr(node))),
            "map.contains" => format!("(lsg_mhas({}, {}) ? 1 : 0)", it("map"), self.key_expr(node)),
            "map.length" => format!("float(lsg_mlen({}))", it("map")),
            "map.keys" => format!("lsg_mkeys({}, {})", it("map"), ["text", "number", "player"].iter().position(|k| *k == self.choice(n, def, "key")).unwrap_or(0)),
            "map.values" => {
                let t = self.v.in_type(node, "map").unwrap_or_default();
                format!("lsg_mvalues({}, {})", it("map"), map_kind(container(&t).map_or("", |c| c.2)))
            }
            "fn.call_pure" => match self.v.func_of(node) {
                Some(f) => {
                    let k = f.outputs.iter().position(|m| m.id == pin).unwrap_or(0);
                    format!("lsg_pf{}_{k}({})", self.fn_index(&f.id), self.call_args(node, f))
                }
                None => Self::zero(&ty),
            },
            _ => "0".into(),
        };
        Val { expr, ty }
    }
}

/// What a mode needs on the server besides its own plugin: gm_components (any component node),
/// and the creature types to load as the map starts (creatures can only come in mid-map when
/// their type was precached; "all" when waves of components bring their own).
pub struct Needs {
    pub components: bool,
    pub creatures: Vec<String>,
}

pub fn needs(checked: &Graph) -> Needs {
    let ex = super::expand::expand(checked);
    let reg = super::graph::registry();
    let components = ex.graph.nodes.iter().any(|n| reg.get(&n.kind).is_some_and(|d| d.needs.as_deref() == Some("components")));
    let mut creatures: Vec<String> = Vec::new();
    for n in ex.graph.nodes.iter().filter(|n| n.kind == "creature.spawn") {
        let k = name_text(n.params.get("kind").and_then(Value::as_str).unwrap_or("zombie"));
        if !k.is_empty() && !creatures.contains(&k) {
            creatures.push(k);
        }
    }
    if ex.graph.nodes.iter().any(|n| n.kind.starts_with("comp.wave") || n.kind == "comp.on_wave_spawn" || n.kind == "comp.load_pack") {
        creatures = vec!["all".into()];
    }
    Needs { components, creatures }
}

/// Turns a checked graph into the plugin's source. The graph must have passed
/// `graph::validate` without errors; nodes nothing runs are left out.
pub fn generate(checked: &Graph, debug: bool) -> Generated {
    // Macros are copied in first (their nodes report as the Macro node)
    let ex = super::expand::expand(checked);
    let g = &ex.graph;
    let v = View::new(g);
    let reached = v.reached_from();
    // the nodes that become functions: every event, and whatever their flow reaches
    let live: Vec<&str> = g
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .filter(|id| v.defs.get(id).is_some_and(|d| !d.reroute && !v.pure(id) && (d.event || reached.contains_key(id))))
        .collect();
    let vars: Vec<&Variable> = g.variables.iter().chain(g.functions.iter().flat_map(|f| f.locals.iter())).collect();
    // flow values get their own ctx slots (text: a kept text); what actions make is kept
    let mut slots: Vec<(&str, String)> = Vec::new();
    let mut outs: Vec<(&str, String)> = Vec::new();
    let mut touts: Vec<(&str, String)> = Vec::new();
    for id in &live {
        let d = v.defs[id];
        let ctx = CTX_KINDS.contains(&d.kind.as_str());
        if !ctx && !STORE_KINDS.contains(&d.kind.as_str()) {
            continue;
        }
        for o in v.outputs(id).iter().filter(|o| o.ty != "exec") {
            if o.ty == "text" {
                touts.push((id, o.id.clone()));
            } else if ctx {
                slots.push((id, o.id.clone()));
            } else {
                outs.push((id, o.id.clone()));
            }
        }
    }
    // vector and struct variables: their own slots after the rings
    let (mut nv, mut nr) = (0usize, 0usize);
    let reserved: Vec<usize> = vars
        .iter()
        .map(|x| {
            let size = match x.scope.as_str() {
                "player" => 33,
                "team" => 4,
                _ => 1,
            };
            match kind_of(&x.ty) {
                'v' => {
                    nv += size;
                    nv - size
                }
                'r' => {
                    nr += size;
                    nr - size
                }
                _ => 0,
            }
        })
        .collect();
    // struct layouts: cells (a vector takes 3) and text slots
    let layouts: Vec<Vec<usize>> = g
        .structs
        .iter()
        .map(|st| {
            let (mut c, mut t) = (0usize, 0usize);
            st.fields
                .iter()
                .map(|f| match kind_of(&f.ty) {
                    's' => {
                        t += 1;
                        t - 1
                    }
                    'v' => {
                        c += 3;
                        c - 3
                    }
                    _ => {
                        c += 1;
                        c - 1
                    }
                })
                .collect()
        })
        .collect();
    let rec_cells = g.structs.iter().map(|st| st.fields.iter().map(|f| if f.ty == "vector" { 3 } else if f.ty == "text" { 0 } else { 1 }).sum::<usize>()).max().unwrap_or(0).max(1);
    let rec_texts = g.structs.iter().map(|st| st.fields.iter().filter(|f| f.ty == "text").count()).max().unwrap_or(0).max(1);
    let gen = Gen { v, slots, outs, touts, live, vars, reserved, layouts, origin: &ex.origin, pinned: Default::default(), debug };
    let v = &gen.v;
    let kinds_live: BTreeSet<&str> = gen.live.iter().map(|id| v.defs[id].kind.as_str()).collect();
    let has = |k: &str| kinds_live.contains(k);
    let timers: Vec<&str> = gen.live.iter().copied().filter(|id| matches!(v.defs[id].kind.as_str(), "timer.delay" | "timer.repeat" | "timer.countdown")).collect();
    let round_vars: Vec<(usize, &&Variable)> = gen.vars.iter().enumerate().filter(|(_, x)| x.reset == "round").collect();
    let any = |ks: &[&str]| ks.iter().any(|k| has(k));
    let uses_down = any(&["player.down", "event.player_revived", "event.down_timeout"]) || pure_used(&gen, "player.is_downed");
    let uses_squad = any(&["squad.set", "squad.friendly_fire"]) || pure_used(&gen, "squad.get") || pure_used(&gen, "squad.alive");
    let uses_nofall = has("player.no_fall");
    let uses_pre = has("event.player_about_hurt") || uses_nofall || uses_squad;
    let uses_buy = any(&["buy.where", "buy.allow"]);
    let uses_save = any(&["save.set", "save.add"]) || pure_used(&gen, "save.get");
    let uses_clock = any(&["clock.set", "clock.add", "clock.pause", "event.time_up"]) || pure_used(&gen, "clock.left");
    let new_round = !timers.is_empty() || !round_vars.is_empty() || uses_clock || uses_down || uses_save;
    // what graph version 3 brought: the values that need the bigger helpers
    let uses_v3 = !g.functions.is_empty()
        || !g.structs.is_empty()
        || !g.enums.is_empty()
        || gen.vars.iter().any(|x| !matches!(x.ty.as_str(), "number" | "bool" | "text"))
        || g.nodes.iter().any(|n| v.defs.get(n.id.as_str()).is_some_and(|d| V3_KINDS.iter().any(|k| d.kind.starts_with(k))));

    let mut o = Out { text: String::new(), lines: 0 };
    let mut spans = Vec::new();
    o.line(format!(
        "/*\n\t{} - made with Loner-Strike's Game Mode Studio (graph version {}).\n\tGenerated from {}.graph.json: change the graph and compile it again; edits here are lost.{}\n*/",
        comment_text(&g.mode.name),
        g.version,
        id_text(&g.mode.id),
        if debug { "\n\tThis is a debug build (Playtest): it reports every node to the Studio." } else { "" }
    ));
    o.blank();
    o.line("#include <amxmodx>\n#include <cstrike>\n#include <fakemeta>\n#include <engine>\n#include <hamsandwich>\n#include <loner>");
    if graph_uses(g, &["save.get", "save.set", "save.add"]) || uses_v3 {
        o.line("#include <celltrie>");
    }
    if uses_v3 {
        o.line("#include <cellarray>");
    }
    o.blank();
    o.line(format!("#define LSG_MODE {}\n#define LSG_CTX {}\n#define LSG_TXT {TXT}\n#define LSG_N {}\n#define LSG_TASK {TASK_BASE}", pawn_str(&id_text(&g.mode.id)), CTX + gen.slots.len(), gen.live.len().max(1)));
    o.line("stock g_lsgTxt[LSG_TXT][64]\nstock g_lsgOut[LSG_N]\nstock g_lsgLeft[LSG_N]\nstock const g_lsgYesNo[][] = { \"no\", \"yes\" }\nstock const g_lsgTeamName[][] = { \"no team\", \"Terrorists\", \"Counter-Terrorists\", \"Spectators\" }");
    if uses_v3 {
        o.line(format!(
            "#define LSG_VR {VEC_RING}\n#define LSG_VP {}\n#define LSG_RR {REC_RING}\n#define LSG_RP {}\n#define LSG_RC {rec_cells}\n#define LSG_RT {rec_texts}\n#define LSG_AC {}\n#define LSG_AMAX {LOOP_LIMIT}\n#define LSG_TMPA {TMP_ARRAYS}\n#define LSG_O {}\n#define LSG_OT {}",
            VEC_RING + nv,
            REC_RING + nr,
            (rec_cells + rec_texts * 64).max(64),
            gen.outs.len().max(1),
            gen.touts.len().max(1)
        ));
    }
    o.blank();

    // variables
    if !gen.vars.is_empty() {
        o.line("// The mode's variables");
        for (i, x) in gen.vars.iter().enumerate() {
            let note = format!("// {} ({}{}{})", comment_text(&x.name), if matches!(x.ty.as_str(), "number" | "bool" | "text") { x.ty.clone() } else { comment_text(&gen.v.type_label(&x.ty)) }, match x.scope.as_str() { "player" => ", per player", "team" => ", per team", _ => "" }, if x.reset == "round" { ", each round" } else { "" });
            let init = Gen::var_lit(&x.ty, Some(&x.default).filter(|d| !d.is_null()));
            if matches!(kind_of(&x.ty), 'v' | 'r') {
                o.line(format!("// {} is in slot {}", comment_text(&x.name), gen.var_ref(x, "0", "0")));
                continue;
            }
            let decl = match (x.ty.as_str(), x.scope.as_str()) {
                ("number", "player") => format!("stock Float:g_pv{i}[33]"),
                ("text", "player") => format!("stock g_pv{i}[33][64]"),
                (_, "player") => format!("stock g_pv{i}[33]"),
                ("number", "team") => format!("stock Float:g_tv{i}[4] = {{ {init}, {init}, {init}, {init} }}"),
                ("text", "team") => format!("stock g_tv{i}[4][64] = {{ {init}, {init}, {init}, {init} }}"),
                (_, "team") => format!("stock g_tv{i}[4] = {{ {init}, {init}, {init}, {init} }}"),
                ("number", _) => format!("stock Float:g_v{i} = {init}"),
                ("text", _) => format!("stock g_v{i}[64] = {init}"),
                _ => format!("stock g_v{i} = {init}"),
            };
            o.line(format!("{decl} {note}"));
        }
        o.blank();
    }

    if has("event.player_about_hurt") || graph_uses(g, &["dmg.block", "dmg.scale", "dmg.set"]) {
        o.line("stock Float:g_lsgDmg, bool:g_lsgDmgBlock");
    }
    if uses_nofall {
        o.line("stock g_lsgNoFall[33]");
    }
    // menus remember who was shown what
    for id in gen.live.iter().filter(|id| v.defs[*id].kind == "msg.menu") {
        o.line(format!("stock g_lsgM{}[33][LSG_CTX]", gen.index(id)));
    }

    if debug {
        let ids: Vec<String> = if gen.live.is_empty() { vec![pawn_str("")] } else { gen.live.iter().map(|id| pawn_str(&gen.report_id(id))).collect() };
        o.line(format!("#define LSD_N {}", gen.live.len().max(1)));
        o.line(format!("new const g_lsdIds[LSD_N][] = {{ {} }}", ids.join(", ")));
        let bps: Vec<&str> = if gen.live.is_empty() { vec!["false"] } else { gen.live.iter().map(|id| if v.nodes[id].breakpoint { "true" } else { "false" }).collect() };
        o.line(format!("new bool:g_lsdBp[LSD_N] = {{ {} }}", bps.join(", ")));
        o.line(include_str!("debug_runtime.pawn").trim_end());
        o.blank();
    }

    o.line(HELPERS);
    o.blank();
    if uses_v3 {
        o.line(V3_HELPERS);
        o.blank();
        v3_stocks(&gen, &mut o);
    }
    if debug {
        // the graph's variables for the live values
        o.line("lsd_graph_vars()\n{");
        for (i, x) in gen.vars.iter().enumerate() {
            let vid = pawn_str(&id_text(&x.id));
            if !matches!(x.ty.as_str(), "number" | "bool" | "text") {
                // the other types as text: a name, a count of items, a position...
                let shown = |at: &str| to_text(&gen, &Val { expr: gen.var_ref(x, at, at), ty: x.ty.clone() });
                match x.scope.as_str() {
                    "player" => o.line(format!("\tfor (new id = 1; id <= MaxClients; id++)\n\t\tif (is_user_connected(id))\n\t\t\tlsd_var_text({vid}, id, {})", shown("id"))),
                    "team" => {
                        for (t, tn) in [(1, "T"), (2, "CT")] {
                            o.line(format!("\tlsd_var_text({}, 0, {})", pawn_str(&format!("{}:{tn}", id_text(&x.id))), shown(&t.to_string())));
                        }
                    }
                    _ => o.line(format!("\tlsd_var_text({vid}, 0, {})", shown("0"))),
                }
                continue;
            }
            let f = match x.ty.as_str() {
                "number" => "lsd_var_number",
                "bool" => "lsd_var_bool",
                _ => "lsd_var_text",
            };
            if x.scope == "player" {
                o.line(format!("\tfor (new id = 1; id <= MaxClients; id++)\n\t\tif (is_user_connected(id))\n\t\t\t{f}({vid}, id, g_pv{i}[id])"));
            } else if x.scope == "team" {
                for (t, tn) in [(1, "T"), (2, "CT")] {
                    o.line(format!("\t{f}({}, 0, g_tv{i}[{t}])", pawn_str(&format!("{}:{tn}", id_text(&x.id)))));
                }
            } else {
                o.line(format!("\t{f}({vid}, 0, g_v{i})"));
            }
        }
        o.line("}");
        o.blank();
    }
    if uses_down || uses_squad {
        o.line(SQUADS);
        o.blank();
    }
    if uses_down {
        o.line(DOWNED);
        o.blank();
    }
    if uses_buy {
        o.line(BUYING);
        o.blank();
    }
    if uses_save {
        o.line(SAVED);
        o.blank();
    }
    if uses_clock {
        o.line(CLOCK);
        o.blank();
    }

    // registration
    let kinds: Vec<&EventKind> = EVENTS.iter().filter(|k| has(k.kind)).collect();
    let need = needs(checked);
    if !need.creatures.is_empty() {
        o.line(format!("// creature types this mode spawns: loaded as the map starts (with what other plugins asked for)\npublic plugin_precache()\n{{\n\tif (get_cvar_pointer(\"ls_creature_precache\"))\n\t\tlsg_precache_types({})\n}}\n", pawn_str(&need.creatures.join(" "))));
        o.line("stock lsg_precache_types(const want[])\n{\n\tnew cur[256], w[32], rest[256], padded[280], pw[36]\n\tget_cvar_string(\"ls_creature_precache\", cur, charsmax(cur))\n\tif (equali(cur, \"all\"))\n\t\treturn\n\tif (equali(want, \"all\"))\n\t{\n\t\tset_cvar_string(\"ls_creature_precache\", \"all\")\n\t\treturn\n\t}\n\tcopy(rest, charsmax(rest), want)\n\twhile (rest[0])\n\t{\n\t\tstrtok(rest, w, charsmax(w), rest, charsmax(rest), ' ')\n\t\tformatex(padded, charsmax(padded), \" %s \", cur)\n\t\tformatex(pw, charsmax(pw), \" %s \", w)\n\t\tif (w[0] && containi(padded, pw) < 0)\n\t\t{\n\t\t\tif (cur[0])\n\t\t\t\tadd(cur, charsmax(cur), \" \")\n\t\t\tadd(cur, charsmax(cur), w)\n\t\t}\n\t}\n\tset_cvar_string(\"ls_creature_precache\", cur)\n}\n");
    }
    o.line("public plugin_init()\n{");
    o.line(format!("\tls_register_mode(LSG_MODE, {}, \"1.0\", \"Game Mode Studio\")", pawn_str(&safe_text(&g.mode.name, 40))));
    if debug {
        o.line("\tlsd_init()");
    }
    for k in &kinds {
        for r in k.register {
            o.line(format!("\t{r}"));
        }
    }
    if new_round {
        o.line("\tregister_event(\"HLTV\", \"lsg_new_round\", \"a\", \"1=0\", \"2=0\")");
    }
    if uses_pre {
        o.line("\tRegisterHamPlayer(Ham_TakeDamage, \"lsg_pre_damage\", 0)");
    }
    if uses_down {
        o.line("\tset_task(0.25, \"lsg_revive_tick\", LSG_TASK - 1, _, _, \"b\")");
    }
    if uses_clock {
        o.line("\tset_task(1.0, \"lsg_clock_tick\", LSG_TASK - 2, _, _, \"b\")");
    }
    if uses_save {
        o.line("\tlsg_saved_load()");
    }
    // arrays and maps are made once; vectors and structs start at their start value (0)
    for (i, x) in gen.vars.iter().enumerate() {
        match container(&x.ty) {
            Some(("array", _, _)) => o.line(format!("\tg_v{i} = _:ArrayCreate(LSG_AC)")),
            Some(("map", _, _)) => o.line(format!("\tg_v{i} = _:TrieCreate()")),
            _ => {}
        }
    }
    // gates that start open, Do Once that starts closed
    for id in &gen.live {
        let (n, d) = gen.node(id);
        let closed = gen.boolean(n, d, "closed");
        if (d.kind == "flow.gate" && !closed) || (d.kind == "flow.do_once" && closed) {
            o.line(format!("\tg_lsgLeft[{}] = 1", gen.index(id)));
        }
    }
    // chat commands: one command each
    for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.chat_command") {
        let cmd = name_text(gen.text(v.nodes[id], v.defs[id], "command"));
        let i = gen.index(id);
        if !cmd.is_empty() {
            for say in ["say", "say_team"] {
                for mark in ["/", "!"] {
                    o.line(format!("\tregister_clcmd({}, \"lsg_chat{i}\")", pawn_str(&format!("{say} {mark}{cmd}"))));
                }
            }
        }
    }
    o.line("}");
    o.blank();
    if new_round {
        o.line("// a new round: timers stop, round variables start again\npublic lsg_new_round()\n{\n\tlsg_stop_timers()");
        if uses_clock {
            o.line("\tlsg_clock_off()");
        }
        if uses_down {
            o.line("\tfor (new p = 1; p <= MaxClients; p++)\n\t\tlsg_clear_down(p)");
        }
        if uses_save {
            o.line("\tlsg_saved_flush()");
        }
        for (_, x) in &round_vars {
            o.line(gen.reset_all(x));
        }
        o.line("}\n");
    }
    for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.chat_command") {
        let i = gen.index(id);
        o.line(format!("public lsg_chat{i}(id)\n{{\n\tnew ctx[LSG_CTX]\n\tctx[0] = id // player\n\t{}(ctx)\n\treturn PLUGIN_CONTINUE\n}}\n", gen.fname(id)));
    }
    if debug || uses_save || uses_clock {
        o.line("public plugin_end()\n{");
        if debug {
            o.line("\tlsd_end()");
        }
        if uses_save {
            o.line("\tlsg_saved_flush()");
        }
        if uses_clock {
            o.line("\tlsg_clock_off()");
        }
        o.line("}\n");
    }
    // the pre-damage hook: no fall damage, squad mates, Player About To Be Hurt
    if uses_pre {
        o.line("public lsg_pre_damage(victim, inflictor, attacker, Float:damage, bits)\n{\n\t#pragma unused inflictor");
        if uses_nofall {
            o.line("\tif ((bits & DMG_FALL) && victim >= 1 && victim <= MaxClients && g_lsgNoFall[victim])\n\t{\n\t\tif (g_lsgNoFall[victim] == 2)\n\t\t\tg_lsgNoFall[victim] = 0\n\t\treturn HAM_SUPERCEDE\n\t}");
        }
        if uses_squad {
            o.line("\tif (!g_lsgSquadFF && attacker != victim && attacker >= 1 && attacker <= MaxClients && g_lsgSquad[victim] && g_lsgSquad[victim] == g_lsgSquad[attacker])\n\t\treturn HAM_SUPERCEDE");
        }
        let hurt: Vec<&&str> = gen.live.iter().filter(|id| v.defs[*id].kind == "event.player_about_hurt").collect();
        if hurt.is_empty() {
            o.line("\t#pragma unused damage\n\t#pragma unused bits\n\treturn HAM_IGNORED\n}\n");
        } else {
            o.line("\t// (saved and put back: damage done inside the flow comes back here)\n\tnew Float:was = g_lsgDmg, bool:wasBlock = g_lsgDmgBlock\n\tg_lsgDmg = damage\n\tg_lsgDmgBlock = false\n\tnew ctx[LSG_CTX]\n\tctx[0] = victim // victim\n\tctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker\n\tctx[2] = _:damage // damage\n\tlsg_dmg_kind(bits, g_lsgTxt[0], 63) // kind");
            for id in hurt {
                o.line(format!("\t{}(ctx)", gen.fname(id)));
            }
            o.line("\tnew Float:now = g_lsgDmg, bool:block = g_lsgDmgBlock\n\tg_lsgDmg = was\n\tg_lsgDmgBlock = wasBlock\n\tif (block)\n\t\treturn HAM_SUPERCEDE\n\tif (now != damage)\n\t{\n\t\tSetHamParamFloat(4, now)\n\t\treturn HAM_HANDLED\n\t}\n\treturn HAM_IGNORED\n}\n");
        }
    }
    // hooks the shared pieces call: revived, out, time up
    if uses_down {
        o.line("lsg_revived_hook(id, by)\n{\n\tnew ctx[LSG_CTX]\n\tctx[0] = id // player\n\tctx[1] = by // by");
        let mut any_call = false;
        for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.player_revived") {
            o.line(format!("\t{}(ctx)", gen.fname(id)));
            any_call = true;
        }
        if !any_call {
            o.line("\t#pragma unused ctx");
        }
        o.line("}\n\nlsg_down_out_hook(id)\n{\n\tnew ctx[LSG_CTX]\n\tctx[0] = id // player");
        let mut any_call = false;
        for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.down_timeout") {
            o.line(format!("\t{}(ctx)", gen.fname(id)));
            any_call = true;
        }
        if !any_call {
            o.line("\t#pragma unused ctx");
        }
        o.line("}\n");
    }
    if uses_clock {
        o.line("lsg_time_up_hook()\n{\n\tnew ctx[LSG_CTX]");
        let mut any_call = false;
        for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.time_up") {
            o.line(format!("\t{}(ctx)", gen.fname(id)));
            any_call = true;
        }
        if !any_call {
            o.line("\t#pragma unused ctx");
        }
        o.line("}\n");
    }
    o.line("stock lsg_stop_timers()\n{\n\tfor (new i = 0; i < LSG_N; i++)\n\t\tremove_task(LSG_TASK + i)\n}");
    o.blank();

    // joining and leaving: per-player variables start again, and the events
    let player_vars: Vec<(usize, &&Variable)> = gen.vars.iter().enumerate().filter(|(_, x)| x.scope == "player").collect();
    if !player_vars.is_empty() || has("event.player_joined") || uses_squad || uses_down || uses_nofall {
        o.line("public client_putinserver(id)\n{");
        if uses_squad || uses_down {
            o.line("\tg_lsgSquad[id] = 0");
        }
        if uses_down {
            o.line("\tlsg_clear_down(id)");
        }
        if uses_nofall {
            o.line("\tg_lsgNoFall[id] = 0");
        }
        for (_, x) in &player_vars {
            o.line(format!("\t{}", gen.reset_line(x, "id")));
        }
        if has("event.player_joined") {
            o.line("\tlsg_on_player_joined(id)");
        }
        o.line("}");
        o.blank();
    }
    if has("event.player_left") || uses_save {
        o.line("public client_disconnected(id)\n{");
        if has("event.player_left") {
            o.line("\tlsg_on_player_left(id)");
        }
        if uses_save {
            o.line("\tlsg_saved_flush()");
        } else {
            o.line("\t#pragma unused id");
        }
        o.line("}\n");
    }

    // handlers
    for k in &kinds {
        if !k.prelude.is_empty() {
            o.line(k.prelude.trim_end());
            o.blank();
        }
        o.line(format!("{}\n{{", k.sig));
        if !k.guard.is_empty() {
            o.line(k.guard);
        }
        o.line("\tnew ctx[LSG_CTX]");
        let def = registry_def(k.kind);
        for (pin, read) in k.values {
            match event_slot(def, pin) {
                Some((true, i)) => {
                    if *read == "@netname" {
                        o.line(format!("\tpev(ent, pev_netname, g_lsgTxt[{i}], 63) // {pin}"));
                    }
                }
                Some((false, i)) => o.line(format!("\tctx[{i}] = {read} // {pin}")),
                None => {}
            }
        }
        for id in gen.live.iter().filter(|id| v.defs[*id].kind == k.kind) {
            let call = format!("{}(ctx)", gen.fname(id));
            match k.named {
                Some(ent) => {
                    let name = name_text(gen.text(v.nodes[id], v.defs[id], "name"));
                    if name.is_empty() {
                        o.line(format!("\t{call}"));
                    } else {
                        o.line(format!("\tif (lsg_named({ent}, {}))\n\t\t{call}", pawn_str(&name)));
                    }
                }
                None => o.line(format!("\t{call}")),
            }
        }
        if !k.tail.is_empty() {
            o.line(k.tail);
        }
        o.line("}");
        o.blank();
    }

    // nodes
    for id in &gen.live {
        let from = o.lines + 1;
        node_code(&gen, id, &mut o);
        spans.push(Span { node: gen.origin.get(*id).cloned().unwrap_or_else(|| id.to_string()), from, to: o.lines });
        o.blank();
    }

    if debug {
        // a paused node runs again from here when the app says resume
        if gen.live.is_empty() {
            o.line("lsd_call(n, const ctx[])\n{\n\t#pragma unused n\n\t#pragma unused ctx\n}");
        } else {
            o.line("lsd_call(n, const data[])\n{\n\tnew ctx[LSG_CTX]\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tctx[k] = data[k]\n\tswitch (n)\n\t{");
            for (i, id) in gen.live.iter().enumerate() {
                o.line(format!("\t\tcase {i}: {}(ctx)", gen.fname(id)));
            }
            o.line("\t}\n}");
        }
    }
    Generated { source: o.text, spans }
}

fn registry_def(kind: &str) -> &'static NodeDef {
    super::graph::registry().get(kind).expect("every event kind is in the registry (tested)")
}

/// The code for one flow node: its function (and its timer's callback).
fn node_code(gen: &Gen, id: &str, o: &mut Out) {
    let v = &gen.v;
    let (n, def) = gen.node(id);
    let i = gen.index(id);
    let debug = gen.debug;
    // (in a Function's graph: compiled only when the Function is called)
    let stock = if v.graph_of(id).is_empty() { "" } else { "stock " };
    o.line(format!("// {} ({})", comment_text(&def.title), gen.report_id(id)));
    o.line(format!("{stock}{}(ctx[])\n{{", gen.fname(id)));
    let mut b = Out { text: String::new(), lines: 0 };
    if debug {
        // each input worked out once, into a local: what's reported is what's used
        for (k, ip) in v.inputs(id).iter().filter(|p| p.ty != "exec").enumerate() {
            if def.kind == "flow.while" && ip.id == "cond" {
                continue;
            }
            let Some(val) = gen.input(id, &ip.id) else { continue };
            let local = format!("lsi{k}");
            match kind_of(&val.ty) {
                's' => b.line(format!("\tnew {local}[128]\n\tcopy({local}, 127, {})", val.expr)),
                _ if val.ty == "number" => b.line(format!("\tnew Float:{local} = {}", val.expr)),
                _ => b.line(format!("\tnew {local} = {}", val.expr)),
            }
            gen.pinned.borrow_mut().push((id.to_string(), ip.id.clone(), local));
        }
        b.line("\tlsd_begin()");
        for ip in v.inputs(id).iter().filter(|p| p.ty != "exec") {
            let Some(val) = gen.input(id, &ip.id) else { continue };
            let label = pawn_str(&safe_text(&ip.label, 30));
            let f = match val.ty.as_str() {
                "player" => "lsd_in_player",
                "entity" => "lsd_in_entity",
                "team" => "lsd_in_team",
                "number" => "lsd_in_number",
                "bool" => "lsd_in_bool",
                "text" => "lsd_in_text",
                "any" => continue,
                t if t.starts_with("struct:") => continue,
                _ => {
                    b.line(format!("\tlsd_in_text({label}, {})", to_text(gen, &val)));
                    continue;
                }
            };
            b.line(format!("\t{f}({label}, {})", val.expr));
        }
        b.line(format!("\tif (lsd_enter({i}, ctx))\n\t\treturn"));
    }
    let next = |pin: &str| -> Option<String> { v.exec_target(id, pin).map(|(t, tp)| format!("{}(ctx)", gen.pin_fname(t, tp))) };
    // in a Function: a Return stops loops and Sequences
    let fn_done = gen.home_fn(id).map(|(fi, _)| format!("g_lsgFD{fi}"));
    // more functions after this one (other flow inputs, timer callbacks)
    let mut after: Vec<String> = Vec::new();
    let exit = |result: &str| -> String { if debug { format!("\tlsd_exit({i}, {})", pawn_str(result)) } else { String::new() } };
    let err = |msg: &str| -> String { if debug { format!("lsd_error({i}, {})", pawn_str(msg)) } else { String::new() } };
    // a check that ends in an error message (debug) or nothing (release)
    let check = |b: &mut Out, cond: &str, msg: &str, body: &[String]| {
        if debug {
            b.line(format!("\tif ({cond})\n\t\t{}\n\telse\n\t{{", err(msg)));
            for l in body {
                b.line(format!("\t{l}"));
            }
            b.line("\t}");
        } else {
            b.line(format!("\tif (!({cond}))\n\t{{"));
            for l in body.iter().filter(|l| !l.is_empty()) {
                b.line(format!("\t{l}"));
            }
            b.line("\t}");
        }
    };
    let then_line = |b: &mut Out, pin: &str| {
        if let Some(c) = next(pin) {
            b.line(format!("\t{c}"));
        }
    };
    let inp = |pin: &str, ty: &str| gen.need(id, pin, ty);
    let comps = "!ls_components_running()";
    let mut menus_after: Vec<(usize, String)> = Vec::new();
    match def.kind.as_str() {
        k if event_kind(k).is_some() => {
            let e = exit("ran");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "cond.is_team" | "flow.branch" => {
            let cond = if def.kind == "cond.is_team" {
                let p = inp("player", "player");
                format!("lsg_team({p}) == {}", inp("team", "team"))
            } else {
                bool01(&inp("cond", "bool"))
            };
            b.line(format!("\tnew ok = {}", bool01(&cond)));
            if debug {
                b.line(format!("\tlsd_exit({i}, g_lsdBool[ok])"));
            }
            match (next("true"), next("false")) {
                (Some(y), Some(n)) => b.line(format!("\tif (ok)\n\t\t{y}\n\telse\n\t\t{n}")),
                (Some(y), None) => b.line(format!("\tif (ok)\n\t\t{y}")),
                (None, Some(n)) => b.line(format!("\tif (!ok)\n\t\t{n}")),
                (None, None) => {
                    if !debug {
                        b.line("\t#pragma unused ok");
                    }
                }
            }
        }
        "player.give_weapon" => {
            let p = inp("player", "player");
            let w = pawn_str(gen.choice(n, def, "weapon"));
            let ammo = gen.num(n, def, "ammo").round() as i64;
            if debug {
                b.line(format!("\tif (ls_give_weapon({p}, {w}, {ammo}))\n\t{}\n\telse\n\t\t{}", exit("given"), err("Not given: that player is dead or already has one.")));
            } else {
                b.line(format!("\tls_give_weapon({p}, {w}, {ammo})"));
            }
            then_line(&mut b, "then");
        }
        "player.strip" | "player.set_health" | "player.set_armor" | "player.set_speed" | "player.teleport" => {
            let p = inp("player", "player");
            let (call, done) = match def.kind.as_str() {
                "player.strip" => (format!("ls_strip({p}, {})", if gen.boolean(n, def, "knife") { "true" } else { "false" }), "stripped"),
                "player.set_health" => (format!("ls_set_health({p}, max(floatround({}), 1))", inp("amount", "number")), "set"),
                "player.set_armor" => (format!("ls_set_armor({p}, max(floatround({}), 0), true)", inp("amount", "number")), "set"),
                "player.set_speed" => (format!("set_user_maxspeed({p}, {})", inp("speed", "number")), "set"),
                _ => (String::new(), "teleported"),
            };
            if def.kind == "player.teleport" {
                let call = match v.data_source(id, "to") {
                    Some(_) => format!("lsg_teleport_to({p}, {})", inp("to", "entity")),
                    None => format!("lsg_teleport({p}, {})", pawn_str(&name_text(gen.text(n, def, "name")))),
                };
                if debug {
                    b.line(format!("\tif (!ls_alive({p}))\n\t\t{}\n\telse if (!{call})\n\t\t{}\n\telse\n\t{}", err("Not moved: that player isn't alive."), err("Not moved: no such entity on this map."), exit(done)));
                } else {
                    b.line(format!("\t{call}"));
                }
            } else {
                check(&mut b, &format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", &[format!("\t{call}"), exit(done)]);
            }
            then_line(&mut b, "then");
        }
        "player.set_team" => {
            let p = inp("player", "player");
            check(&mut b, &format!("{p} < 1 || {p} > MaxClients || !is_user_connected({p})"), "Nothing done: nobody there.", &[format!("\tls_set_team({p}, CsTeams:({}))", inp("team", "team")), exit("moved")]);
            then_line(&mut b, "then");
        }
        "score.end_round" => {
            let w = inp("winner", "team");
            // (a literal winner reads as plain words; a wired one as a number)
            let cmd = match w.as_str() {
                "1" => "\tserver_cmd(\"endround T\")".to_string(),
                "2" => "\tserver_cmd(\"endround CT\")".to_string(),
                "0" => "\tserver_cmd(\"endround\")".to_string(),
                _ => format!("\tlsg_end_round({w})"),
            };
            b.line(cmd);
            let e = exit(match w.as_str() {
                "1" => "Terrorists win",
                "2" => "Counter-Terrorists win",
                "0" => "draw",
                _ => "ended",
            });
            if !e.is_empty() {
                b.line(e);
            }
        }
        "msg.broadcast" => {
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            b.line(format!("\tls_announce({who}, {}, \"%s\", {})", float_lit(gen.num(n, def, "seconds")), pawn_str(&safe_text(gen.text(n, def, "text"), 100))));
            let e = exit("shown");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "msg.hud" => {
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            let slot = HUD_SLOTS.iter().position(|s| *s == gen.choice(n, def, "slot")).unwrap_or(1);
            let text = pawn_str(&safe_text(gen.text(n, def, "text"), 100));
            let secs = float_lit(gen.num(n, def, "seconds"));
            let args = text_args(gen, &text, gen.input(id, "value"));
            b.line(format!("\tls_hud({who}, LsHud:{slot}, {secs}, {args})"));
            let e = exit("shown");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "var.set" | "var.add" => {
            if let Some(var) = v.var_of(id) {
                let r = gen.var_ref(var, &inp("player", "player"), &inp("team", "team"));
                let stmt = if def.kind == "var.add" {
                    format!("\t{r} += {}", inp("amount", "number"))
                } else {
                    format!("\t{}", gen.set_var(var, &r, &inp("value", &var.ty)))
                };
                b.line(stmt);
                if debug {
                    match var.ty.as_str() {
                        "number" => b.line(format!("\tlsd_exit({i}, fmt(\"%.2f\", {r}))")),
                        "bool" => b.line(format!("\tlsd_exit({i}, g_lsdBool[{r} ? 1 : 0])")),
                        _ => b.line(exit("set")),
                    }
                }
            }
            then_line(&mut b, "then");
        }
        "timer.delay" => {
            b.line(format!("\tset_task({}, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)", lsg_secs(&gen.input(id, "secs"))));
            let e = exit("waiting");
            if !e.is_empty() {
                b.line(e);
            }
        }
        "timer.repeat" => {
            let times = gen.num(n, def, "times").round() as i64;
            b.line(format!("\tremove_task(LSG_TASK + {i})\n\tg_lsgLeft[{i}] = {times}\n\tset_task({}, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)", lsg_secs(&gen.input(id, "every"))));
            let e = exit("started");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "timer.countdown" => {
            let secs = gen.num(n, def, "secs").round() as i64;
            b.line(format!("\tremove_task(LSG_TASK + {i})\n\tg_lsgLeft[{i}] = {secs}\n\tlsg_c{i}_show()\n\tset_task(1.0, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)"));
            let e = exit("started");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "timer.stop_all" => {
            b.line("\tlsg_stop_timers()");
            let e = exit("stopped");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "flow.for_each_player" | "flow.for_each_entity" => {
            let out = if def.kind == "flow.for_each_player" { "player" } else { "entity" };
            let k = gen.slot(id, out).unwrap_or(0);
            let body = next("each").map(|c| c.replace("(ctx)", "(c)"));
            b.line("\tnew c[LSG_CTX], count");
            if def.kind == "flow.for_each_player" {
                let which = which_code(gen.choice(n, def, "which"));
                b.line(format!("\tfor (new p = 1; p <= MaxClients; p++)\n\t{{\n\t\tif (!lsg_match(p, {which}))\n\t\t\tcontinue"));
            } else {
                let name = pawn_str(&name_text(gen.text(n, def, "name")));
                let find = if gen.choice(n, def, "by") == "name" { "find_ent_by_tname" } else { "find_ent_by_class" };
                b.line(format!("\tnew p = -1\n\twhile (count < 512 && (p = {find}(p, {name})) > 0)\n\t{{"));
            }
            b.line(format!("\t\tcount++\n\t\tfor (new k = 0; k < LSG_CTX; k++)\n\t\t\tc[k] = ctx[k]\n\t\tc[{k}] = p"));
            match body {
                Some(c) => b.line(format!("\t\t{c}")),
                None => b.line("\t\t#pragma unused c"),
            }
            b.line("\t}");
            if debug {
                b.line(format!("\tlsd_exit({i}, fmt(\"%d times\", count))"));
            } else {
                b.line("\t#pragma unused count");
            }
            then_line(&mut b, "then");
        }
        "flow.pick_player" | "flow.pick_entity" => {
            let out = if def.kind == "flow.pick_player" { "player" } else { "entity" };
            let k = gen.slot(id, out).unwrap_or(0);
            b.line("\tnew list[256], count");
            if def.kind == "flow.pick_player" {
                let which = which_code(gen.choice(n, def, "which"));
                b.line(format!("\tfor (new p = 1; p <= MaxClients; p++)\n\t\tif (lsg_match(p, {which}))\n\t\t\tlist[count++] = p"));
            } else {
                let name = pawn_str(&name_text(gen.text(n, def, "name")));
                let find = if gen.choice(n, def, "by") == "name" { "find_ent_by_tname" } else { "find_ent_by_class" };
                b.line(format!("\tnew p = -1\n\twhile (count < 256 && (p = {find}(p, {name})) > 0)\n\t\tlist[count++] = p"));
            }
            b.line(format!("\tif (count > 0)\n\t{{\n\t\tctx[{k}] = list[random(count)]"));
            if debug {
                b.line(format!("\t\tlsd_exit({i}, fmt(\"picked %d of %d\", ctx[{k}], count))"));
            }
            if let Some(c) = next("then") {
                b.line(format!("\t\t{c}"));
            }
            b.line("\t}");
            let none = next("none");
            if debug || none.is_some() {
                b.line("\telse\n\t{");
                if debug {
                    b.line(format!("\t\tlsd_exit({i}, \"none\")"));
                }
                if let Some(c) = none {
                    b.line(format!("\t\t{c}"));
                }
                b.line("\t}");
            }
        }
        "func.define" | "fn.entry" | "event.chat_command" | "event.player_about_hurt" | "event.player_revived" | "event.down_timeout" | "event.time_up" => {
            let e = exit("called");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "func.call" => {
            let target = n.params.get("func").and_then(Value::as_str).filter(|t| gen.live.contains(t));
            if let Some(t) = target {
                let player = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
                let (sp, sn) = (gen.slot(t, "player").unwrap_or(0), gen.slot(t, "number").unwrap_or(0));
                b.line(format!("\tnew c[LSG_CTX]\n\tc[{sp}] = {player}\n\tc[{sn}] = _:({})\n\t{}(c)", inp("number", "number"), gen.fname(t)));
            }
            let e = exit("called");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "timer.stop" => {
            if let Some(t) = n.params.get("timer").and_then(Value::as_str).filter(|t| gen.live.contains(t)) {
                b.line(format!("\tremove_task(LSG_TASK + {})", gen.index(t)));
            }
            let e = exit("stopped");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "player.add_money" | "player.add_frags" | "player.launch" | "player.give_item" | "player.drop_weapon" | "player.set_gravity" | "player.set_glow" | "player.set_model" | "player.kill" | "player.freeze" | "player.set_role" => {
            let p = inp("player", "player");
            let (cond, msg, call) = match def.kind.as_str() {
                "player.add_money" => (format!("!lsg_ok({p})"), "Nothing done: nobody there.", format!("lsg_add_money({p}, floatround({}))", inp("amount", "number"))),
                "player.add_frags" => (format!("!lsg_ok({p})"), "Nothing done: nobody there.", format!("lsg_add_frags({p}, floatround({}))", inp("amount", "number"))),
                "player.set_role" => (format!("!lsg_ok({p})"), "Nothing done: nobody there.", format!("ls_set_role({p}, {})", pawn_str(&name_text(gen.text(n, def, "role"))))),
                "player.give_item" => (format!("!ls_alive({p})"), "Not given: that player isn't alive.", match gen.choice(n, def, "item") {
                    "vest" => format!("cs_set_user_armor({p}, 100, CS_ARMOR_KEVLAR)"),
                    "defuser" => format!("cs_set_user_defuse({p}, 1)"),
                    "nvg" => format!("cs_set_user_nvg({p}, 1)"),
                    _ => format!("cs_set_user_armor({p}, 100, CS_ARMOR_VESTHELM)"),
                }),
                "player.drop_weapon" => (format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", format!("lsg_drop({p})")),
                "player.set_gravity" => (format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", format!("set_user_gravity({p}, {})", inp("gravity", "number"))),
                "player.launch" => (format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", format!("lsg_launch({p}, {})", inp("speed", "number"))),
                "player.set_glow" => (format!("!lsg_ok({p})"), "Nothing done: nobody there.", match gen.choice(n, def, "color") {
                    "off" => format!("set_user_rendering({p})"),
                    c => {
                        let (r, g, bl) = match c { "green" => (0, 255, 0), "blue" => (0, 80, 255), "yellow" => (255, 220, 0), "white" => (255, 255, 255), _ => (255, 0, 0) };
                        format!("set_user_rendering({p}, kRenderFxGlowShell, {r}, {g}, {bl}, kRenderNormal, 16)")
                    }
                }),
                "player.set_model" => (format!("!lsg_ok({p})"), "Nothing done: nobody there.", match gen.choice(n, def, "model") {
                    "reset" => format!("cs_reset_user_model({p})"),
                    m => format!("cs_set_user_model({p}, {})", pawn_str(m)),
                }),
                "player.kill" => (format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", format!("user_kill({p}, 1)")),
                _ => (format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", format!("lsg_freeze({p}, {})", bool01(&inp("on", "bool")))),
            };
            check(&mut b, &cond, msg, &[format!("\t{call}"), exit("done")]);
            then_line(&mut b, "then");
        }
        "player.respawn" | "comp.respawn_at" => {
            let p = inp("player", "player");
            if def.kind == "player.respawn" {
                check(&mut b, &format!("!lsg_ok({p}) || is_user_alive({p}) || lsg_team({p}) < 1 || lsg_team({p}) > 2"), "Nothing done: only a dead player on a team comes back.", &[format!("\tExecuteHamB(Ham_CS_RoundRespawn, {p})"), exit("respawned")]);
            } else {
                check(&mut b, &format!("{comps} || !lsg_ok({p})"), "gm_components isn't running, or nobody there.", &[format!("\tls_respawn_at({p}, 0)"), exit("done")]);
            }
            then_line(&mut b, "then");
        }
        "msg.chat" => {
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            let text = pawn_str(&safe_text(gen.text(n, def, "text"), 120));
            b.line(format!("\tls_chat({who}, {})", text_args(gen, &text, gen.input(id, "value"))));
            let e = exit("said");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "msg.bar" => {
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            b.line(format!("\tlsg_bar({who}, {})", gen.num(n, def, "secs").round() as i64));
            let e = exit("shown");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "msg.menu" => {
            let p = inp("player", "player");
            let title = pawn_str(&safe_text(gen.text(n, def, "title"), 40));
            let opts: Vec<(usize, String)> = (1..=4).filter_map(|k| {
                let t = safe_text(gen.text(n, def, &format!("o{k}")), 40);
                (!t.trim().is_empty()).then_some((k, t))
            }).collect();
            let mut body = vec![format!("\tfor (new k = 0; k < LSG_CTX; k++)\n\t\t\tg_lsgM{i}[{p}][k] = ctx[k]"), format!("\tnew m = menu_create({title}, \"lsg_mh{i}\")")];
            for (_, t) in &opts {
                body.push(format!("\tmenu_additem(m, {})", pawn_str(t)));
            }
            body.push(format!("\tmenu_display({p}, m)"));
            body.push(exit("shown"));
            check(&mut b, &format!("!lsg_ok({p}) || is_user_bot({p})"), "Not shown: nobody there, or a bot.", &body);
            then_line(&mut b, "then");
            // the choice comes back later
            let mut cb = format!("\n// {} ({}): the choice\npublic lsg_mh{i}(id, menu, item)\n{{\n\tmenu_destroy(menu)\n\tif (item < 0 || id < 1 || id > MaxClients)\n\t\treturn PLUGIN_HANDLED\n\tnew ctx[LSG_CTX]\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tctx[k] = g_lsgM{i}[id][k]\n\tswitch (item)\n\t{{\n", comment_text(&def.title), id_text(id));
            for (pos, (k, _)) in opts.iter().enumerate() {
                let call = next(&format!("c{k}")).unwrap_or_else(|| "#pragma unused ctx".into());
                let dbg = if debug { format!("\n\t\t\tlsd_exit({i}, \"choice {k}\")") } else { String::new() };
                cb.push_str(&format!("\t\tcase {pos}:\n\t\t{{{dbg}\n\t\t\t{call}\n\t\t}}\n"));
            }
            cb.push_str("\t}\n\treturn PLUGIN_HANDLED\n}");
            menus_after.push((i, cb));
        }
        "game.play_sound" => {
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            b.line(format!("\tclient_cmd({who}, \"spk %s\", {})", pawn_str(gen.choice(n, def, "sound"))));
            let e = exit("played");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "game.set_cvar" => {
            b.line(format!("\tset_cvar_float({}, {})", pawn_str(gen.choice(n, def, "cvar")), inp("value", "number")));
            let e = exit("set");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "entity.use" => {
            let name = pawn_str(&name_text(gen.text(n, def, "name")));
            if debug {
                b.line(format!("\tif (!lsg_use({name}))\n\t\t{}\n\telse\n\t{}", err("No entity has that name on this map."), exit("used")));
            } else {
                b.line(format!("\tlsg_use({name})"));
            }
            then_line(&mut b, "then");
        }
        "bot.goto" | "bot.hold" | "bot.follow" | "bot.release" => {
            let bot = inp("bot", "player");
            let call = match def.kind.as_str() {
                "bot.goto" => format!("lsg_bot_goto({bot}, {}, {})", pawn_str(&name_text(gen.text(n, def, "name"))), gen.num(n, def, "secs").round() as i64),
                "bot.hold" => format!("ls_bot_hold({bot}, max(floatround({}), 1))", inp("secs", "number")),
                "bot.follow" => format!("ls_bot_follow({bot}, {}, {})", inp("who", "player"), gen.num(n, def, "secs").round() as i64),
                _ => format!("ls_bot_release({bot})"),
            };
            check(&mut b, &format!("!ls_alive({bot}) || !is_user_bot({bot})"), "Nothing done: that player isn't a living bot.", &[format!("\t{call}"), exit("ordered")]);
            then_line(&mut b, "then");
        }
        "dmg.block" | "dmg.scale" | "dmg.set" => {
            let stmt = match def.kind.as_str() {
                "dmg.block" => "\tg_lsgDmgBlock = true".to_string(),
                "dmg.scale" => format!("\tg_lsgDmg = g_lsgDmg * floatmax({}, 0.0)", inp("factor", "number")),
                _ => format!("\tg_lsgDmg = floatmax({}, 0.0)", inp("amount", "number")),
            };
            b.line(stmt);
            if debug {
                b.line(format!("\tif (g_lsgDmgBlock)\n\t\tlsd_exit({i}, \"blocked\")\n\telse\n\t\tlsd_exit({i}, fmt(\"%.1f\", g_lsgDmg))"));
            }
            then_line(&mut b, "then");
        }
        "player.no_fall" => {
            let p = inp("player", "player");
            let v2 = match gen.choice(n, def, "mode") { "on" => 1, "off" => 0, _ => 2 };
            b.line(format!("\tg_lsgNoFall[lsg_slot({p})] = {v2}"));
            let e = exit("set");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "player.down" => {
            let p = inp("player", "player");
            let (secs, hold, hp) = (float_lit(gen.num(n, def, "secs")), float_lit(gen.num(n, def, "hold")), gen.num(n, def, "health").round() as i64);
            if debug {
                b.line(format!("\tif (!lsg_make_down({p}, {secs}, {hold}, {hp}))\n\t\t{}\n\telse\n\t{}", err("Not downed: that player isn't dead (wire it from Player Killed)."), exit("downed")));
            } else {
                b.line(format!("\tlsg_make_down({p}, {secs}, {hold}, {hp})"));
            }
            then_line(&mut b, "then");
        }
        "squad.set" | "squad.friendly_fire" => {
            if def.kind == "squad.set" {
                let p = inp("player", "player");
                check(&mut b, &format!("!lsg_ok({p})"), "Nothing done: nobody there.", &[format!("\tlsg_set_squad({p}, clamp(floatround({}), 0, 32))", inp("squad", "number")), exit("set")]);
            } else {
                b.line(format!("\tg_lsgSquadFF = bool:{}", bool01(&inp("on", "bool"))));
                let e = exit("set");
                if !e.is_empty() {
                    b.line(e);
                }
            }
            then_line(&mut b, "then");
        }
        "buy.where" | "buy.allow" => {
            let stmt = if def.kind == "buy.where" {
                match gen.choice(n, def, "where") {
                    "zones" => "\tg_lsgBuyOff = false\n\tset_cvar_num(\"mp_buy_anywhere\", 0)",
                    "nowhere" => "\tg_lsgBuyOff = true",
                    _ => "\tg_lsgBuyOff = false\n\tset_cvar_num(\"mp_buy_anywhere\", 1)",
                }
                .to_string()
            } else {
                let code = ["all", "pistols", "no_snipers", "no_awp", "no_grenades", "no_mg", "armor", "knife"].iter().position(|k| *k == gen.choice(n, def, "allow")).unwrap_or(0);
                format!("\tg_lsgBuyAllow = {code}")
            };
            b.line(stmt);
            let e = exit("set");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "save.set" | "save.add" => {
            let p = inp("player", "player");
            let key = pawn_str(&name_text(gen.text(n, def, "key")));
            let call = if def.kind == "save.set" { format!("lsg_saved_set({p}, {key}, {})", inp("value", "number")) } else { format!("lsg_saved_set({p}, {key}, lsg_saved_get({p}, {key}) + ({}))", inp("amount", "number")) };
            check(&mut b, &format!("!lsg_ok({p}) || g_lsgSaved == Invalid_Trie"), "Not saved: nobody there, or the store didn't open.", &[format!("\t{call}"), if debug { format!("\tlsd_exit({i}, fmt(\"%.1f\", lsg_saved_get({p}, {key})))") } else { String::new() }]);
            then_line(&mut b, "then");
        }
        "clock.set" | "clock.add" | "clock.pause" => {
            let stmt = match def.kind.as_str() {
                "clock.set" => format!("\tlsg_clock_set({})", inp("secs", "number")),
                "clock.add" => format!("\tlsg_clock_add({})", inp("secs", "number")),
                _ => format!("\tlsg_clock_pause({})", if gen.choice(n, def, "do") == "resume" { 0 } else { 1 }),
            };
            b.line(stmt);
            if debug {
                b.line(format!("\tlsd_exit({i}, fmt(\"%.0f s left\", lsg_clock_left()))"));
            }
            then_line(&mut b, "then");
        }
        "comp.drop_call" => {
            let at = at_origin(gen, id, n, def);
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!{at})\n\t\t{}\n\telse\n\t{{\n\t\tls_drop_call(o, \"\")\n\t{}\n\t}}", err("gm_components isn't running on this server."), err("No entity has that name on this map."), exit("called")));
            } else {
                b.line(format!("\tif (!{comps} && {at})\n\t\tls_drop_call(o, \"\")"));
            }
            then_line(&mut b, "then");
        }
        "comp.create" => {
            // (the choices' keys are the Studio's own; two differ from gm_components' classnames)
            let kind = pawn_str(match gen.choice(n, def, "kind") {
                "ls_trap" => "ls_trigger_trap",
                "ls_extraction" => "ls_extraction_zone",
                k => k,
            });
            let at = at_origin(gen, id, n, def);
            let keys = pawn_str(&keys_text(gen.text(n, def, "keys")));
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!{at})\n\t\t{}\n\telse\n\t{{\n\t\tg_lsgOut[{i}] = ls_comp_create({kind}, o, {keys})\n\t\tif (g_lsgOut[{i}])\n\t\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))\n\t\telse\n\t\t\t{}\n\t}}", err("gm_components isn't running on this server."), err("No entity has that name on this map."), err("gm_components couldn't make that component (it has no room, or the keys are wrong).")));
            } else {
                b.line(format!("\tif (!{comps} && {at})\n\t\tg_lsgOut[{i}] = ls_comp_create({kind}, o, {keys})"));
            }
            then_line(&mut b, "then");
        }
        "comp.zone_set" | "comp.zone_shrink" => {
            let z = inp("zone", "entity");
            // the centre: an entity wired into Centre, else the one with that name, else (no name)
            // where the zone is now
            let at = match gen.v.data_source(id, "at_ent") {
                Some(_) => format!("lsg_ent_origin({}, o)", gen.need(id, "at_ent", "entity")),
                None if name_text(gen.text(n, def, "at")).is_empty() => format!("ls_zone_center({z}, o)"),
                None => format!("lsg_origin_of({}, o)", pawn_str(&name_text(gen.text(n, def, "at")))),
            };
            let r = inp("radius", "number");
            let call = if def.kind == "comp.zone_set" { format!("ls_zone_set({z}, o, {r})") } else { format!("ls_zone_shrink({z}, o, {r}, {})", float_lit(gen.num(n, def, "secs"))) };
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!{at})\n\t\t{}\n\telse\n\t{{\n\t\t{call}\n\t{}\n\t}}", err("gm_components isn't running on this server."), err(if at.starts_with("ls_zone_center") { "That isn't a zone (wire a Shrinking zone into Zone)." } else { "No entity has that name on this map." }), exit("done")));
            } else {
                b.line(format!("\tif (!{comps} && {at})\n\t\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "comp.load_pack" => {
            let pack = pawn_str(&name_text(gen.text(n, def, "pack")));
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\tnew map[32]\n\t\tget_mapname(map, charsmax(map))\n\t\tif (!ls_comp_load_pack(fmt(\"gamemodes/components/%s.%s.ini\", map, {pack})))"), if debug { format!("\t\tlsd_error({i}, \"This map has no such pack.\")\n\t\telse\n\t\t\tlsd_exit({i}, \"loaded\")") } else { "\t\t{}".into() }]);
            then_line(&mut b, "then");
        }
        "comp.crate_open" | "comp.wave_start" | "comp.wave_stop" | "comp.extract_open" | "comp.door" | "comp.trap_fire" | "comp.capture_lock" | "comp.reset" | "comp.start" | "comp.stop" => {
            let call = match def.kind.as_str() {
                "comp.crate_open" => format!("ls_loot_open({}, {})", inp("container", "entity"), inp("player", "player")),
                "comp.wave_start" => format!("ls_wave_start(max(floatround({}), 0))", inp("wave", "number")),
                "comp.wave_stop" => "ls_wave_stop()".into(),
                "comp.extract_open" => format!("ls_extract_open({}, bool:{})", inp("point", "entity"), bool01(&inp("open", "bool"))),
                "comp.door" => {
                    if gen.choice(n, def, "do") == "close" { format!("ls_door_close({})", inp("door", "entity")) } else { format!("ls_door_open({})", inp("door", "entity")) }
                }
                "comp.trap_fire" => format!("ls_trap_fire({})", inp("trap", "entity")),
                "comp.start" => format!("ls_comp_start({})", inp("comp", "entity")),
                "comp.stop" => format!("ls_comp_stop({})", inp("comp", "entity")),
                "comp.capture_lock" => format!("ls_cp_lock({}, {})", inp("point", "entity"), inp("secs", "number")),
                _ => "ls_comp_reset()".into(),
            };
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\t{call}"), exit("done")]);
            then_line(&mut b, "then");
        }
        "creature.spawn" => {
            let kind = pawn_str(&name_text(gen.text(n, def, "kind")));
            // (at a wired entity, or the one with the name)
            let at = match v.data_source(id, "at_ent") {
                Some(_) => format!("lsg_ent_find_origin({})", inp("at_ent", "entity")),
                None => pawn_str(&name_text(gen.text(n, def, "at"))),
            };
            if debug {
                b.line(format!("\tif (!get_cvar_pointer(\"ls_creature_ver\"))\n\t\t{}\n\telse if (!(g_lsgOut[{i}] = lsg_creature({kind}, {at})))\n\t\t{}\n\telse\n\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))", err("This server's bot brain has no creatures (ls_creature)."), err("Not spawned: no entity has that name, or that type isn't loaded on this map.")));
            } else {
                b.line(format!("\tg_lsgOut[{i}] = lsg_creature({kind}, {at})"));
            }
            then_line(&mut b, "then");
        }
        // ---- graph version 3
        "fn.return" => {
            if let Some((fi, f)) = gen.home_fn(id) {
                for (k, m) in f.outputs.iter().enumerate() {
                    let val = inp(&m.id, &m.ty);
                    b.line(if m.ty == "text" { format!("\tcopy(g_lsgR{fi}_{k}, 127, {val})") } else { format!("\tg_lsgR{fi}_{k} = {val}") });
                }
                b.line(format!("\tg_lsgFD{fi} = true"));
            }
            let e = exit("returned");
            if !e.is_empty() {
                b.line(e);
            }
        }
        "fn.call" => {
            if let Some(f) = v.func_of(id) {
                let fi = gen.fn_index(&f.id);
                b.line(format!("\tlsg_fr{fi}({})", gen.call_args(id, f)));
                for (k, m) in f.outputs.iter().enumerate() {
                    if let Some(t) = gen.tout_index(id, &m.id) {
                        b.line(format!("\tcopy(g_lsgOT[{t}], 127, g_lsgR{fi}_{k})"));
                    } else if let Some(c) = gen.out_index(id, &m.id) {
                        b.line(format!("\tg_lsgO[{c}] = _:g_lsgR{fi}_{k}"));
                    }
                }
            }
            let e = exit("called");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "flow.sequence" => {
            let e = exit("ran");
            if !e.is_empty() {
                b.line(e);
            }
            for (k, p) in v.exec_outs(id).iter().enumerate() {
                if let (Some(stop), true) = (&fn_done, k > 0) {
                    b.line(format!("\tif ({stop})\n\t\treturn"));
                }
                then_line(&mut b, p);
            }
        }
        "flow.do_once" | "flow.do_n" | "flow.gate" => {
            let gate = format!("g_lsgLeft[{i}]");
            match def.kind.as_str() {
                "flow.do_once" => {
                    b.line(format!("\tif ({gate})\n\t{{\n\t{}\n\t\treturn\n\t}}\n\t{gate} = 1", exit("closed")));
                    after.push(multi_fn(gen, id, "reset", &format!("\t{gate} = 0")));
                }
                "flow.do_n" => {
                    let c = gen.out_index(id, "counter").unwrap_or(0);
                    b.line(format!("\tif ({gate} >= floatround({}))\n\t{{\n\t{}\n\t\treturn\n\t}}\n\t{gate}++\n\tg_lsgO[{c}] = _:float({gate})", inp("n", "number"), exit("done N times")));
                    after.push(multi_fn(gen, id, "reset", &format!("\t{gate} = 0")));
                }
                _ => {
                    b.line(format!("\tif (!{gate})\n\t{{\n\t{}\n\t\treturn\n\t}}", exit("closed")));
                    after.push(multi_fn(gen, id, "open", &format!("\t{gate} = 1")));
                    after.push(multi_fn(gen, id, "close", &format!("\t{gate} = 0")));
                    after.push(multi_fn(gen, id, "toggle", &format!("\t{gate} ^= 1")));
                }
            }
            let e = exit("through");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "flow.flipflop" => {
            let c = gen.out_index(id, "is_a").unwrap_or(0);
            b.line(format!("\tg_lsgLeft[{i}] ^= 1\n\tg_lsgO[{c}] = g_lsgLeft[{i}]"));
            let (a, bb) = (next("a"), next("b"));
            let dbg_a = if debug { format!("\n\t\tlsd_exit({i}, \"A\")") } else { String::new() };
            let dbg_b = if debug { format!("\n\t\tlsd_exit({i}, \"B\")") } else { String::new() };
            b.line(format!("\tif (g_lsgLeft[{i}])\n\t{{{dbg_a}\n\t\t{}\n\t}}\n\telse\n\t{{{dbg_b}\n\t\t{}\n\t}}", a.unwrap_or_default(), bb.unwrap_or_default()));
        }
        "flow.for_loop" | "flow.for_loop_break" | "flow.while" | "flow.for_each_array" => {
            let body = next("body").map(|c| c.replace("(ctx)", "(c)"));
            let brk = def.kind != "flow.for_loop" && def.kind != "flow.while";
            b.line("\tnew c[LSG_CTX], n");
            if brk {
                b.line(format!("\tg_lsgLeft[{i}] = 0"));
                after.push(multi_fn(gen, id, "break", &format!("\tg_lsgLeft[{i}] = 1")));
            }
            let copy = "\t\tfor (new j = 0; j < LSG_CTX; j++)\n\t\t\tc[j] = ctx[j]";
            match def.kind.as_str() {
                "flow.while" => {
                    let limit = gen.num(n, def, "limit").round().clamp(1.0, LOOP_LIMIT as f64) as i64;
                    b.line(format!("\twhile (n < {limit} && {})\n\t{{\n\t\tn++\n{copy}", bool01(&inp("cond", "bool"))));
                }
                "flow.for_each_array" => {
                    let item = v.out_type(id, "item").unwrap_or_default();
                    b.line(format!("\tstatic snap\n\tif (!snap)\n\t\tsnap = _:ArrayCreate(LSG_AC)\n\tlsg_acopy(snap, {})\n\tnew size = lsg_alen(snap)", inp("array", "any")));
                    b.line(format!("\tfor (new k = 0; k < size && k < LSG_AMAX; k++)\n\t{{\n\t\tn++\n{copy}"));
                    if let Some(s) = gen.slot(id, "index") {
                        b.line(format!("\t\tc[{s}] = _:float(k)"));
                    }
                    if let Some(t) = gen.tout_index(id, "item") {
                        b.line(format!("\t\tArrayGetString(Array:snap, k, g_lsgOT[{t}], 127)"));
                    } else if let Some(s) = gen.slot(id, "item") {
                        b.line(format!("\t\tc[{s}] = {}", match kind_of(&item) {
                            'v' => "lsg_agetv(snap, float(k))".to_string(),
                            'r' => "lsg_agetr(snap, float(k))".to_string(),
                            _ => "ArrayGetCell(Array:snap, k)".to_string(),
                        }));
                    }
                }
                _ => {
                    let s = gen.slot(id, "index").unwrap_or(0);
                    b.line(format!("\tnew last = floatround({})\n\tfor (new k = floatround({}); k <= last && n < LSG_AMAX; k++)\n\t{{\n\t\tn++\n{copy}\n\t\tc[{s}] = _:float(k)", inp("last", "number"), inp("first", "number")));
                }
            }
            match body {
                Some(c) => b.line(format!("\t\t{c}")),
                None => b.line("\t\t#pragma unused c"),
            }
            if brk {
                b.line(format!("\t\tif (g_lsgLeft[{i}])\n\t\t\tbreak"));
            }
            if let Some(stop) = &fn_done {
                b.line(format!("\t\tif ({stop})\n\t\t\treturn"));
            }
            b.line("\t}");
            if debug {
                b.line(format!("\tlsd_exit({i}, fmt(\"%d times\", n))"));
            } else {
                b.line("\t#pragma unused n");
            }
            then_line(&mut b, "then");
        }
        "flow.switch_num" | "flow.switch_text" | "flow.switch_enum" => {
            let outs: Vec<(String, String)> = v.outputs(id).iter().filter(|p| p.id != "default").map(|p| (p.id.clone(), p.label.clone())).collect();
            let call = |pin: &str| next(pin).map(|c| format!("\t\t{c}\n")).unwrap_or_default();
            if def.kind == "flow.switch_enum" {
                b.line(format!("\tswitch ({})\n\t{{", inp("in", "any")));
                for (k, (pin, _)) in outs.iter().enumerate() {
                    b.line(format!("\t\tcase {k}:\n\t\t{{\n\t{}\t\t}}", call(pin)));
                }
                b.line("\t}");
            } else {
                let num = def.kind == "flow.switch_num";
                b.line(if num { format!("\tnew Float:x = {}", inp("in", "number")) } else { format!("\tnew x[128]\n\tcopy(x, 127, {})", inp("in", "text")) });
                for (k, (pin, label)) in outs.iter().enumerate() {
                    let test = if num { format!("x == {}", float_lit(label.parse::<f64>().unwrap_or(0.0))) } else { format!("equali(x, {})", pawn_str(&safe_text(label, 60))) };
                    b.line(format!("\t{}if ({test})\n\t{{\n{}\t}}", if k > 0 { "else " } else { "" }, call(pin)));
                }
                let d = call("default");
                if outs.is_empty() {
                    b.line(format!("\t#pragma unused x\n{d}"));
                } else {
                    b.line(format!("\telse\n\t{{\n{d}\t}}"));
                }
            }
        }
        "flow.if_valid" => {
            b.line(format!("\tif (lsg_valid({}))\n\t{{\n\t{}\n\t\t{}\n\t}}\n\telse\n\t{{\n\t{}\n\t\t{}\n\t}}", inp("in", "entity"), exit("valid"), next("valid").unwrap_or_default(), exit("not valid"), next("invalid").unwrap_or_default()));
        }
        "timer.retrigger" => {
            b.line(format!("\tremove_task(LSG_TASK + {i})\n\tset_task({}, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)", lsg_secs(&gen.input(id, "secs"))));
            let e = exit("waiting");
            if !e.is_empty() {
                b.line(e);
            }
        }
        "timer.by_function" => {
            if let Some(f) = v.func_of(id) {
                let fi = gen.fn_index(&f.id);
                let looping = gen.boolean(n, def, "loop");
                b.line(format!("\tremove_task(LSG_TASK + {i})\n\tset_task({}, \"lsg_tf{i}\", LSG_TASK + {i}, _, _, {})", lsg_secs(&gen.input(id, "secs")), if looping { "\"b\"" } else { "\"\"" }));
                let args = f.inputs.iter().map(|m| Gen::zero(&m.ty)).collect::<Vec<_>>().join(", ");
                let hold = if debug { "\tif (lsd_hold_timer())\n\t\treturn\n".to_string() } else { String::new() };
                after.push(format!("public lsg_tf{i}()\n{{\n{hold}\tlsg_fr{fi}({args})\n}}"));
            }
            let e = exit("started");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "timer.clear" => {
            b.line(format!("\tlsg_tclear({})", inp("handle", "timer")));
            let e = exit("cleared");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "debug.print" => {
            let text = pawn_str(&safe_text(gen.text(n, def, "text"), 100));
            b.line(format!("\tnew t[192]\n\tformatex(t, charsmax(t), {})\n\tserver_print(\"[%s] %s\", LSG_MODE, t)", text_args(gen, &text, gen.input(id, "value"))));
            if gen.boolean(n, def, "chat") {
                b.line("\tclient_print(0, print_chat, \"%s\", t)");
            }
            if debug {
                b.line(format!("\tnew esc[200]\n\tlsd_esc(t, esc, charsmax(esc))\n\tlsd_exit({i}, esc)"));
            }
            then_line(&mut b, "then");
        }
        "player.set_position" | "player.set_velocity" => {
            let p = inp("player", "player");
            let call = if def.kind == "player.set_position" { format!("lsg_vsetpos({p}, {})", inp("at", "vector")) } else { format!("lsg_vsetvel({p}, {})", inp("v", "vector")) };
            check(&mut b, &format!("!ls_alive({p})"), "Nothing done: that player isn't alive.", &[format!("\t{call}"), exit("done")]);
            then_line(&mut b, "then");
        }
        "struct.set_field" => {
            if let (Some(var), Some(field)) = (v.var_of(id), v.field_of(id)) {
                if let Some(st) = v.struct_type(&var.ty) {
                    let si = gen.struct_index(&st.id);
                    let k = st.fields.iter().position(|f| f.id == field.id).unwrap_or(0);
                    let off = gen.layouts[si][k];
                    let r = gen.var_ref(var, "0", "0");
                    let val = inp("value", &field.ty);
                    b.line(match kind_of(&field.ty) {
                        's' => format!("\tcopy(g_lsgRecT[lsg_ri({r})][{off}], 63, {val})"),
                        'v' => format!("\tlsg_rsetvec({r}, {off}, {val})"),
                        _ => format!("\tg_lsgRec[lsg_ri({r})][{off}] = _:({val})"),
                    });
                }
            }
            let e = exit("set");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        k if k.starts_with("array.") => {
            let a = inp("array", "any");
            let item = v.in_type(id, "item").or_else(|| v.in_type(id, "array").and_then(|t| container(&t).map(|c| c.2.to_string()))).unwrap_or_default();
            let ik = kind_of(&item);
            let val = || Gen::cell_arg(&item, &inp("item", &item));
            let stmt = match k {
                "array.add" => format!("g_lsgO[{}] = _:float(lsg_apush{ik}({a}, {}))", gen.out_index(id, "index").unwrap_or(0), val()),
                "array.insert" => format!("lsg_ains{ik}({a}, {}, {})", inp("at", "number"), val()),
                "array.remove" => format!("g_lsgO[{}] = lsg_arem{ik}({a}, {}) ? 1 : 0", gen.out_index(id, "removed").unwrap_or(0), val()),
                "array.remove_index" => format!("lsg_aremi({a}, {})", inp("at", "number")),
                "array.set" => format!("lsg_aset{ik}({a}, {}, {})", inp("at", "number"), val()),
                "array.shuffle" => format!("lsg_ashuffle({a})"),
                "array.sort" => format!("lsg_asort({a}, {}, {})", match item.as_str() { "number" => 0, "text" => 2, _ => 1 }, if gen.choice(n, def, "order") == "desc" { 1 } else { 0 }),
                _ => format!("lsg_aclear({a})"),
            };
            b.line(format!("\t{stmt}"));
            if debug {
                b.line(format!("\tlsd_exit({i}, fmt(\"%d items\", lsg_alen({a})))"));
            }
            then_line(&mut b, "then");
        }
        k if k.starts_with("map.") => {
            let m = inp("map", "any");
            let stmt = match k {
                "map.add" => {
                    let vt = v.in_type(id, "value").unwrap_or_default();
                    format!("lsg_mset{}({m}, {}, {})", kind_of(&vt), gen.key_expr(id), Gen::cell_arg(&vt, &inp("value", &vt)))
                }
                "map.remove" => format!("lsg_mdel({m}, {})", gen.key_expr(id)),
                _ => format!("lsg_mclear({m})"),
            };
            b.line(format!("\t{stmt}"));
            if debug {
                b.line(format!("\tlsd_exit({i}, fmt(\"%d keys\", lsg_mlen({m})))"));
            }
            then_line(&mut b, "then");
        }
        _ => {}
    }
    gen.pinned.borrow_mut().clear();
    if !b.text.contains("ctx") {
        b.line("\t#pragma unused ctx");
    }
    o.text.push_str(&b.text);
    o.lines += b.lines;
    o.line("}");
    for (_, cb) in &menus_after {
        o.line(cb);
    }
    for f in &after {
        o.blank();
        o.line(f);
    }

    // timer callbacks (they belong to the node, for compiler messages)
    let hold = if debug { format!("\tif (lsd_hold_timer())\n\t{{\n\t\tset_task(0.25, \"lsg_t{i}\", LSG_TASK + {i}, data, LSG_CTX)\n\t\treturn\n\t}}\n") } else { String::new() };
    let cb = format!("public lsg_t{i}(const data[], task)\n{{\n{hold}\t#pragma unused task\n\tnew ctx[LSG_CTX]\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tctx[k] = data[k]");
    match def.kind.as_str() {
        "timer.delay" | "timer.retrigger" => {
            o.blank();
            o.line(&cb);
            if debug {
                o.line(format!("\tlsd_exit({i}, \"fired\")"));
            }
            if let Some(c) = next("then") {
                o.line(format!("\t{c}"));
            } else {
                o.line("\t#pragma unused ctx");
            }
            o.line("}");
        }
        "timer.repeat" => {
            let every = lsg_secs(&gen.input(id, "every"));
            o.blank();
            o.line(&cb);
            // 0 times: until the round ends (timers stop at a new round)
            o.line(format!("\tif (g_lsgLeft[{i}] > 0 && --g_lsgLeft[{i}] == 0)\n\t\tremove_task(LSG_TASK + {i})\n\telse\n\t\tset_task({every}, \"lsg_t{i}\", LSG_TASK + {i}, data, LSG_CTX)"));
            if debug {
                o.line(format!("\tlsd_exit({i}, \"each\")"));
            }
            if let Some(c) = next("tick") {
                o.line(format!("\t{c}"));
            }
            o.line("}");
        }
        "timer.countdown" => {
            let slot = HUD_SLOTS.iter().position(|s| *s == gen.choice(n, def, "slot")).unwrap_or(0);
            let text = pawn_str(&safe_text(gen.text(n, def, "text"), 60));
            o.blank();
            o.line(format!("lsg_c{i}_show()\n\tls_hud(0, LsHud:{slot}, 1.2, \"%s %d\", {text}, g_lsgLeft[{i}])"));
            o.blank();
            o.line(&cb);
            o.line(format!("\tif (--g_lsgLeft[{i}] > 0)\n\t{{\n\t\tlsg_c{i}_show()\n\t\tset_task(1.0, \"lsg_t{i}\", LSG_TASK + {i}, data, LSG_CTX)\n\t\treturn\n\t}}"));
            if debug {
                o.line(format!("\tlsd_exit({i}, \"done\")"));
            }
            if let Some(c) = next("done") {
                o.line(format!("\t{c}"));
            } else {
                o.line("\t#pragma unused ctx");
            }
            o.line("}");
        }
        _ => {}
    }
}

/// The format and arguments for text with an optional Value after it.
fn text_args(gen: &Gen, text: &str, value: Option<Val>) -> String {
    match value {
        None => format!("\"%s\", {text}"),
        Some(val) => match val.ty.as_str() {
            "number" => format!("\"%s %d\", {text}, floatround({})", val.expr),
            "text" => format!("\"%s %s\", {text}, {}", val.expr),
            "player" => format!("\"%s %s\", {text}, lsg_name({})", val.expr),
            "bool" => format!("\"%s %s\", {text}, g_lsgYesNo[({}) ? 1 : 0]", val.expr),
            "team" => format!("\"%s %s\", {text}, g_lsgTeamName[clamp({}, 0, 3)]", val.expr),
            "entity" | "any" => format!("\"%s %d\", {text}, {}", val.expr),
            _ => format!("\"%s %s\", {text}, {}", to_text(gen, &val)),
        },
    }
}

/// Any value as a text expression (Print String, Format Text, To Text, the debugger).
fn to_text(gen: &Gen, val: &Val) -> String {
    let e = &val.expr;
    match val.ty.as_str() {
        "text" => e.clone(),
        "number" => format!("lsg_ntext({e})"),
        "player" => format!("lsg_name({e})"),
        "entity" => format!("lsg_etext({e})"),
        "team" => format!("g_lsgTeamName[clamp({e}, 0, 3)]"),
        "bool" => format!("g_lsgYesNo[({e}) ? 1 : 0]"),
        "vector" => format!("lsg_vtext({e})"),
        "timer" => format!("g_lsgRun[lsg_tactive({e}) ? 1 : 0]"),
        t if t.starts_with("enum:") => match gen.v.enum_type(t) {
            Some(en) => format!("g_lsgE{}[clamp({e}, 0, {})]", gen.enum_index(&en.id), en.values.len().max(1) - 1),
            None => "\"\"".into(),
        },
        t if t.starts_with("array<") => format!("fmt(\"%d items\", lsg_alen({e}))"),
        t if t.starts_with("map<") => format!("fmt(\"%d keys\", lsg_mlen({e}))"),
        t if t.starts_with("struct:") => pawn_str(&safe_text(&gen.v.type_label(t), 30)),
        _ => format!("fmt(\"%d\", {e})"),
    }
}

/// Format Text: [0] to [3] in the text become the values wired in (as text); a % stays a %.
fn format_text(gen: &Gen, node: &str, template: &str) -> String {
    let chars: Vec<char> = safe_text(template, 100).chars().collect();
    let (mut f, mut args) = (String::new(), Vec::new());
    let mut k = 0;
    while k < chars.len() {
        if chars[k] == '[' && k + 2 < chars.len() && chars[k + 2] == ']' && ('0'..='3').contains(&chars[k + 1]) {
            f.push_str("%s");
            args.push(match gen.input(node, &format!("a{}", chars[k + 1])) {
                Some(v) => to_text(gen, &v),
                None => "\"\"".into(),
            });
            k += 3;
            continue;
        }
        if chars[k] == '%' {
            f.push_str("%%");
        } else {
            f.push(chars[k]);
        }
        k += 1;
    }
    let mut call = format!("fmt({}", pawn_str(&f));
    for a in args {
        call.push_str(", ");
        call.push_str(&a);
    }
    call.push(')');
    call
}

/// A struct's field read from a struct value `r` (a slot), by its layout offset.
fn field_read(ty: &str, r: &str, off: usize) -> String {
    match kind_of(ty) {
        's' => format!("g_lsgRecT[lsg_ri({r})][{off}]"),
        'v' => format!("lsg_rvec({r}, {off})"),
        _ if ty == "number" => format!("Float:g_lsgRec[lsg_ri({r})][{off}]"),
        _ => format!("g_lsgRec[lsg_ri({r})][{off}]"),
    }
}

/// How a map keeps its values: 0 cells, 1 text, 2 vectors.
fn map_kind(item: &str) -> u8 {
    match kind_of(item) {
        's' => 1,
        'v' => 2,
        _ => 0,
    }
}

/// Another flow input's function of a node (Reset, Open, Break...): always a stock, so an
/// input nothing is wired into costs nothing.
fn multi_fn(gen: &Gen, id: &str, pin: &str, body: &str) -> String {
    format!("stock {}(ctx[])\n{{\n\t#pragma unused ctx\n{body}\n}}", gen.pin_fname(id, pin))
}

/// Node kinds that need the version 3 helpers (by prefix).
const V3_KINDS: &[&str] = &[
    "fn.", "macro.", "struct.", "enum.", "array.", "map.", "vec.", "flow.sequence", "flow.do_", "flow.gate", "flow.flipflop", "flow.for_loop", "flow.while", "flow.for_each_array", "flow.switch_", "flow.if_valid", "timer.retrigger", "timer.by_function",
    "timer.clear", "timer.is_active", "debug.print", "logic.equal", "logic.select", "cond.is_valid", "player.is_player", "creature.is", "math.fn", "math.clamp", "math.lerp", "math.in_range", "math.number", "text.literal", "text.append", "text.format", "text.contains", "text.length", "text.to_number", "text.from",
];

/// The pieces made for this graph: enum names, struct makers, Make Array, Functions.
fn v3_stocks(gen: &Gen, o: &mut Out) {
    let v = &gen.v;
    let g = v.g;
    let typed = |ty: &str, name: &str| match kind_of(ty) {
        's' => format!("const {name}[]"),
        _ if ty == "number" => format!("Float:{name}"),
        _ => name.to_string(),
    };
    for (k, e) in g.enums.iter().enumerate() {
        let names: Vec<String> = e.values.iter().map(|x| pawn_str(&safe_text(x, 30))).collect();
        o.line(format!("// enum {}\nstock const g_lsgE{k}[][] = {{ {} }}", comment_text(&e.name), if names.is_empty() { pawn_str("") } else { names.join(", ") }));
    }
    for (k, st) in g.structs.iter().enumerate() {
        let params: Vec<String> = st.fields.iter().enumerate().map(|(j, f)| typed(&f.ty, &format!("f{j}"))).collect();
        o.line(format!("// struct {}\nstock lsg_mks{k}({})\n{{\n\tnew r = lsg_rnew()", comment_text(&st.name), params.join(", ")));
        for (j, f) in st.fields.iter().enumerate() {
            let off = gen.layouts[k][j];
            o.line(match kind_of(&f.ty) {
                's' => format!("\tcopy(g_lsgRecT[r][{off}], 63, f{j})"),
                'v' => format!("\tlsg_rsetvec(r, {off}, f{j})"),
                _ => format!("\tg_lsgRec[r][{off}] = _:f{j}"),
            });
        }
        o.line("\treturn r\n}");
    }
    for (j, n) in g.nodes.iter().enumerate().filter(|(_, n)| n.kind == "array.make") {
        let item = v.out_type(&n.id, "out").and_then(|t| container(&t).map(|c| c.2.to_string())).unwrap_or_else(|| "number".into());
        let count = v.inputs(&n.id).len();
        let params: Vec<String> = (0..count).map(|k| typed(&item, &format!("p{k}"))).collect();
        o.line(format!("stock lsg_mka{j}({})\n{{\n\tnew a = lsg_atmp()", params.join(", ")));
        for k in 0..count {
            o.line(format!("\tlsg_apush{}(a, {})", kind_of(&item), Gen::cell_arg(&item, &format!("p{k}"))));
        }
        o.line("\treturn a\n}");
    }
    for (fi, f) in g.functions.iter().enumerate() {
        let Some(entry) = gen.live.iter().copied().find(|id| v.defs[id].kind == "fn.entry" && v.graph_of(id) == f.id) else { continue };
        o.line(format!("// Function: {}", comment_text(&f.name)));
        for (k, m) in f.outputs.iter().enumerate() {
            o.line(match kind_of(&m.ty) {
                's' => format!("stock g_lsgR{fi}_{k}[128]"),
                _ if m.ty == "number" => format!("stock Float:g_lsgR{fi}_{k}"),
                _ => format!("stock g_lsgR{fi}_{k}"),
            });
        }
        o.line(format!("stock bool:g_lsgFD{fi}"));
        let params: Vec<String> = f.inputs.iter().enumerate().map(|(j, m)| typed(&m.ty, &format!("a{j}"))).collect();
        o.line(format!("stock lsg_fr{fi}({})\n{{\n\tnew ctx[LSG_CTX]", params.join(", ")));
        for (j, m) in f.inputs.iter().enumerate() {
            if let Some(t) = gen.tout_index(entry, &m.id) {
                o.line(format!("\tcopy(g_lsgOT[{t}], 127, a{j})"));
            } else if let Some(sl) = gen.slot(entry, &m.id) {
                o.line(format!("\tctx[{sl}] = _:a{j}"));
            } else {
                o.line(format!("\t#pragma unused a{j}"));
            }
        }
        for (k, m) in f.outputs.iter().enumerate() {
            o.line(if m.ty == "text" { format!("\tg_lsgR{fi}_{k}[0] = 0") } else { format!("\tg_lsgR{fi}_{k} = {}", Gen::zero(&m.ty)) });
        }
        o.line(format!("\tg_lsgFD{fi} = false"));
        for x in &f.locals {
            o.line(gen.reset_all(x));
        }
        o.line(format!("\t{}(ctx)\n}}", gen.fname(entry)));
        if f.pure {
            let args: Vec<String> = (0..f.inputs.len()).map(|j| format!("a{j}")).collect();
            for (k, m) in f.outputs.iter().enumerate() {
                let head = if m.ty == "number" { "Float:" } else { "" };
                o.line(format!("stock {head}lsg_pf{fi}_{k}({})\n{{\n\tlsg_fr{fi}({})", params.join(", "), args.join(", ")));
                o.line(if m.ty == "text" { format!("\tnew t[128]\n\tcopy(t, 127, g_lsgR{fi}_{k})\n\treturn t\n}}") } else { format!("\treturn g_lsgR{fi}_{k}\n}}") });
            }
        }
        o.blank();
    }
}

/// Is a pure node of this kind used by something that runs?
fn pure_used(gen: &Gen, kind: &str) -> bool {
    gen.v.g.nodes.iter().any(|n| n.kind == kind)
}

/// Does the graph have a node of one of these kinds anywhere?
fn graph_uses(g: &Graph, kinds: &[&str]) -> bool {
    g.nodes.iter().any(|n| kinds.contains(&n.kind.as_str()))
}

/// The origin test for "at an entity": the one wired into At, else the one with the name.
fn at_origin(gen: &Gen, id: &str, n: &Node, def: &'static NodeDef) -> String {
    match gen.v.data_source(id, "at_ent") {
        Some(_) => format!("lsg_ent_origin({}, o)", gen.need(id, "at_ent", "entity")),
        None => format!("lsg_origin_of({}, o)", pawn_str(&name_text(gen.text(n, def, "at")))),
    }
}

/// Who a loop or a pick takes, as lsg_match's number.
fn which_code(w: &str) -> u8 {
    match w {
        "all" => 0,
        "dead" => 2,
        "t" => 3,
        "ct" => 4,
        "alive_t" => 5,
        "alive_ct" => 6,
        "bots" => 7,
        "humans" => 8,
        _ => 1,
    }
}

/// A yes/no value as 0 or 1 (a literal stays a literal: no constant tests for the compiler).
fn bool01(e: &str) -> String {
    if e == "0" || e == "1" { e.to_string() } else { format!("(({e}) ? 1 : 0)") }
}

/// A timer's seconds: at least 0.1.
fn lsg_secs(v: &Option<Val>) -> String {
    match v {
        Some(v) => format!("floatmax({}, 0.1)", v.expr),
        None => "1.0".into(),
    }
}

/// Graph version 3's helpers: vectors and structs in slots, arrays, maps, text, timers.
/// Every index is clamped and every array or map call checks its handle and size, so a
/// graph can't read or write outside them.
const V3_HELPERS: &str = include_str!("v3_helpers.pawn");

/// Helpers every plugin gets (stocks: the ones a graph doesn't use cost nothing).
const HELPERS: &str = r##"// counted now (the brain's ls_creature_alive is only brought up to date a moment later)
stock lsg_creatures_alive()
{
	new n, e = MaxClients, Float:hp
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		pev(e, pev_health, hp)
		if (pev(e, pev_deadflag) == DEAD_NO && hp > 0.0)
			n++
	}
	return n
}

stock lsg_slot(id)
	return id >= 1 && id <= MaxClients ? id : 0

stock bool:lsg_ok(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id)

stock bool:lsg_match(p, which)
{
	if (!is_user_connected(p))
		return false
	switch (which)
	{
		case 1: return bool:is_user_alive(p)
		case 2: return !is_user_alive(p) && (lsg_team(p) == 1 || lsg_team(p) == 2)
		case 3: return lsg_team(p) == 1
		case 4: return lsg_team(p) == 2
		case 5: return is_user_alive(p) && lsg_team(p) == 1
		case 6: return is_user_alive(p) && lsg_team(p) == 2
		case 7: return bool:is_user_bot(p)
		case 8: return !is_user_bot(p)
	}
	return true
}

stock lsg_end_round(team)
{
	if (team == 1)
		server_cmd("endround T")
	else if (team == 2)
		server_cmd("endround CT")
	else
		server_cmd("endround")
}

stock bool:lsg_use(const name[])
{
	new ent = -1, used
	while (name[0] && used < 64 && (ent = find_ent_by_tname(ent, name)) > 0)
	{
		ExecuteHamB(Ham_Use, ent, 0, 0, 2, 1.0)
		used++
	}
	return used > 0
}

stock lsg_bar(id, secs)
{
	if (id && !lsg_ok(id))
		return
	message_begin(id ? MSG_ONE : MSG_ALL, get_user_msgid("BarTime"), _, id)
	write_short(secs)
	message_end()
}

stock lsg_add_money(id, amount)
{
	if (pev_valid(id) == 2)
		cs_set_user_money(id, clamp(cs_get_user_money(id) + amount, 0, 16000), 1)
}

stock Float:lsg_get_money(id)
	return lsg_ok(id) && pev_valid(id) == 2 ? float(cs_get_user_money(id)) : 0.0

stock lsg_add_frags(id, amount)
{
	set_user_frags(id, get_user_frags(id) + amount)
	// (setting the deaths again sends the scoreboard its new numbers)
	if (pev_valid(id) == 2)
		cs_set_user_deaths(id, cs_get_user_deaths(id))
}

stock Float:lsg_get_frags(id)
	return lsg_ok(id) ? float(get_user_frags(id)) : 0.0

stock Float:lsg_get_deaths(id)
	return lsg_ok(id) && pev_valid(id) == 2 ? float(cs_get_user_deaths(id)) : 0.0

stock lsg_freeze(id, on)
{
	new flags = pev(id, pev_flags)
	set_pev(id, pev_flags, on ? (flags | FL_FROZEN) : (flags & ~FL_FROZEN))
}

stock lsg_bot_goto(bot, const name[], secs)
{
	new Float:o[3]
	if (!lsg_origin_of(name, o))
		return
	ls_bot_goto(bot, o)
	// a timed goal: the bot is free again after it
	remove_task(LSG_TASK - 100 - bot)
	if (secs > 0)
		set_task(float(secs), "lsg_bot_free", LSG_TASK - 100 - bot)
}

public lsg_bot_free(task)
	ls_bot_release(LSG_TASK - 100 - task)

// Drop Weapon: a person drops it; a bot loses it (a bot's "drop" can crash this game's DLL)
stock lsg_drop(id)
{
	if (!is_user_bot(id))
	{
		engclient_cmd(id, "drop")
		return
	}
	new wid = get_user_weapon(id)
	if (!wid || wid == CSW_KNIFE)
		return
	new name[32]
	get_weaponname(wid, name, charsmax(name))
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", name)) > 0)
	{
		if (pev(ent, pev_owner) != id)
			continue
		ExecuteHamB(Ham_Weapon_RetireWeapon, ent)
		if (ExecuteHamB(Ham_RemovePlayerItem, id, ent))
		{
			set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << wid))
			ExecuteHamB(Ham_Item_Kill, ent)
		}
		break
	}
}

stock Float:lsg_mod(Float:a, Float:b)
{
	if (b == 0.0)
		return 0.0
	return a - b * float(floatround(a / b, floatround_floor))
}

stock Float:lsg_pow(Float:a, Float:b)
{
	new Float:r = floatpower(a, b)
	if (r != r)
		return 0.0
	return floatclamp(r, -100000.0, 100000.0)
}

stock Float:lsg_div(Float:a, Float:b)
	return b == 0.0 ? 0.0 : a / b

stock Float:lsg_min(Float:a, Float:b)
	return a < b ? a : b

stock Float:lsg_max(Float:a, Float:b)
	return a > b ? a : b

stock Float:lsg_random(Float:a, Float:b, frac)
{
	if (a > b)
	{
		new Float:t = a
		a = b
		b = t
	}
	return frac ? random_float(a, b) : float(random_num(floatround(a), floatround(b)))
}

stock Float:lsg_health(id)
	return ls_alive(id) ? float(get_user_health(id)) : 0.0

stock lsg_team(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) && pev_valid(id) == 2 ? (_:cs_get_user_team(id)) : 0

stock lsg_is_bot(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) && is_user_bot(id) ? 1 : 0

stock lsg_name(id)
{
	new name[32]
	if (id >= 1 && id <= MaxClients && is_user_connected(id))
		get_user_name(id, name, charsmax(name))
	else
		copy(name, charsmax(name), "nobody")
	return name
}

stock lsg_find(const name[])
	return name[0] ? max(find_ent_by_tname(-1, name), 0) : 0

stock bool:lsg_named(ent, const name[])
{
	new tn[32]
	pev(ent, pev_targetname, tn, charsmax(tn))
	return bool:equal(tn, name)
}

stock bool:lsg_origin_of(const name[], Float:o[3])
{
	// "#12": entity 12 (an entity wired in, not a name)
	if (name[0] == '#')
		return lsg_ent_origin(str_to_num(name[1]), o)
	return lsg_ent_origin(lsg_find(name), o)
}

stock bool:lsg_ent_origin(ent, Float:o[3])
{
	if (ent <= 0 || !pev_valid(ent))
		return false
	pev(ent, pev_origin, o)
	// (a brush entity's origin is often 0 0 0: its middle instead)
	if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
	{
		new Float:mins[3], Float:maxs[3]
		pev(ent, pev_absmin, mins)
		pev(ent, pev_absmax, maxs)
		for (new k = 0; k < 3; k++)
			o[k] = (mins[k] + maxs[k]) / 2.0
	}
	return true
}

stock bool:lsg_teleport_to(id, ent)
{
	new Float:o[3]
	if (!ls_alive(id) || !lsg_ent_origin(ent, o))
		return false
	ls_teleport(id, o)
	return true
}

stock bool:lsg_teleport(id, const name[])
{
	new Float:o[3]
	if (!ls_alive(id) || !lsg_origin_of(name, o))
		return false
	ls_teleport(id, o)
	return true
}

stock lsg_log_player()
{
	new who[96], name[32], uid
	read_logargv(0, who, charsmax(who))
	parse_loguser(who, name, charsmax(name), uid)
	return find_player("k", uid)
}

stock lsg_creature(const kind[], const at[])
{
	new Float:o[3]
	if (!get_cvar_pointer("ls_creature_ver") || !lsg_origin_of(at, o))
		return 0
	return lsg_creature_o(kind, o)
}

/// (an entity's name for lsg_creature: "" picks no entity, so its origin is looked up as #index)
stock lsg_ent_find_origin(ent)
{
	new s[16]
	formatex(s, charsmax(s), "#%d", ent)
	return s
}

stock lsg_creature_o(const kind[], const Float:o[3])
{
	if (!get_cvar_pointer("ls_creature_ver"))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	return is_valid_ent(ent) && !(entity_get_int(ent, EV_INT_flags) & FL_KILLME) ? ent : 0
}

stock lsg_dmg_kind(bits, out[], len)
{
	if (bits & DMG_FALL)
		copy(out, len, "fall")
	else if (bits & DMG_BLAST)
		copy(out, len, "blast")
	else if (bits & DMG_BURN)
		copy(out, len, "burn")
	else if (bits & DMG_BULLET)
		copy(out, len, "shot")
	else if (bits & DMG_SLASH)
		copy(out, len, "slash")
	else
		copy(out, len, "other")
}

stock lsg_launch(id, Float:speed)
{
	new Float:v[3]
	pev(id, pev_velocity, v)
	v[2] = speed
	set_pev(id, pev_velocity, v)
}

stock lsg_round_time(secs)
{
	message_begin(MSG_ALL, get_user_msgid("RoundTime"))
	write_short(max(secs, 0))
	message_end()
}"##;

/// Squads (also the "same side" a revive needs).
const SQUADS: &str = r#"stock g_lsgSquad[33], bool:g_lsgSquadFF

stock lsg_set_squad(id, squad)
{
	g_lsgSquad[id] = squad
	// the bots' brain: a squad is one side, both ways
	if (!get_cvar_pointer("ls_mode_ver"))
		return
	if (squad > 0)
	{
		server_cmd("ls_mode role %d sq%d", id, squad)
		server_cmd("ls_mode friend @sq%d @sq%d 1", squad, squad)
	}
	else
		server_cmd("ls_mode role %d -", id)
}

stock bool:lsg_same_side(a, b)
{
	if (g_lsgSquad[a] || g_lsgSquad[b])
		return g_lsgSquad[a] == g_lsgSquad[b]
	return !get_cvar_num("mp_freeforall") && lsg_team(a) > 0 && lsg_team(a) == lsg_team(b)
}

stock lsg_squads_alive()
{
	new seen[34], n
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue
		new key = g_lsgSquad[p] ? min(g_lsgSquad[p], 32) : 0
		if (key == 0)
			n++
		else if (!seen[key])
		{
			seen[key] = 1
			n++
		}
	}
	return n
}"#;

/// Downed players and revives (Make Downed).
const DOWNED: &str = r#"stock bool:g_lsgDown[33], Float:g_lsgDownAt[33], Float:g_lsgDownOrg[33][3], g_lsgRevBy[33], Float:g_lsgRevProg[33]
stock Float:g_lsgDownSecs[33], Float:g_lsgDownHold[33], g_lsgDownHp[33]

stock lsg_make_down(p, Float:secs, Float:hold, hp)
{
	if (!lsg_ok(p) || is_user_alive(p))
		return 0
	g_lsgDown[p] = true
	g_lsgDownAt[p] = get_gametime()
	pev(p, pev_origin, g_lsgDownOrg[p])
	g_lsgRevBy[p] = 0
	g_lsgRevProg[p] = 0.0
	g_lsgDownSecs[p] = secs
	g_lsgDownHold[p] = hold
	g_lsgDownHp[p] = hp
	ls_chat(0, "%s is down! Hold USE on them to bring them back.", lsg_name(p))
	lsg_send_revive(p)
	return 1
}

// the two nearest bots on the same side go and stand on the downed player (the brain's "use" goal)
stock lsg_send_revive(victim)
{
	if (!get_cvar_pointer("ls_mode_ver"))
		return
	new near[2], Float:nd[2] = { 99999.0, 99999.0 }
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == victim || !is_user_alive(p) || !is_user_bot(p) || !lsg_same_side(p, victim))
			continue
		new Float:o[3]
		pev(p, pev_origin, o)
		new Float:d = get_distance_f(o, g_lsgDownOrg[victim])
		if (d < nd[0])
		{
			nd[1] = nd[0]
			near[1] = near[0]
			nd[0] = d
			near[0] = p
		}
		else if (d < nd[1])
		{
			nd[1] = d
			near[1] = p
		}
	}
	new left = max(floatround(g_lsgDownSecs[victim] - (get_gametime() - g_lsgDownAt[victim])), 1)
	for (new i = 0; i < 2; i++)
		if (near[i])
			server_cmd("ls_mode goal %d use %.0f %.0f %.0f %d 75 %d 64", near[i], g_lsgDownOrg[victim][0], g_lsgDownOrg[victim][1], g_lsgDownOrg[victim][2], victim, left)
}

stock lsg_clear_down(p)
{
	if (g_lsgDown[p] && get_cvar_pointer("ls_mode_ver"))
		server_cmd("ls_mode goal_clear all %d", p)
	if (g_lsgRevBy[p] && lsg_ok(g_lsgRevBy[p]) && !is_user_bot(g_lsgRevBy[p]))
		lsg_bar(g_lsgRevBy[p], 0)
	g_lsgDown[p] = false
	g_lsgRevBy[p] = 0
	g_lsgRevProg[p] = 0.0
}

public lsg_revive_tick()
{
	new Float:now = get_gametime()
	for (new d = 1; d <= MaxClients; d++)
	{
		if (!g_lsgDown[d])
			continue
		if (!is_user_connected(d) || is_user_alive(d))
		{
			lsg_clear_down(d)
			continue
		}
		if (now - g_lsgDownAt[d] > g_lsgDownSecs[d])
		{
			ls_chat(0, "%s is out.", lsg_name(d))
			lsg_clear_down(d)
			lsg_down_out_hook(d)
			continue
		}
		// someone on the same side close by: people hold USE, bots just stay there
		new who = 0
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == d || !is_user_alive(p) || !lsg_same_side(p, d))
				continue
			new Float:o[3]
			pev(p, pev_origin, o)
			if (get_distance_f(o, g_lsgDownOrg[d]) > 72.0)
				continue
			if (!is_user_bot(p) && !(pev(p, pev_button) & IN_USE))
				continue
			who = p
			break
		}
		if (who != g_lsgRevBy[d])
		{
			if (g_lsgRevBy[d] && lsg_ok(g_lsgRevBy[d]) && !is_user_bot(g_lsgRevBy[d]))
				lsg_bar(g_lsgRevBy[d], 0)
			g_lsgRevBy[d] = who
			g_lsgRevProg[d] = 0.0
			if (who && !is_user_bot(who))
				lsg_bar(who, floatround(g_lsgDownHold[d], floatround_ceil))
		}
		if (!who)
			continue
		g_lsgRevProg[d] += 0.25
		if (g_lsgRevProg[d] >= g_lsgDownHold[d])
			lsg_revive(d, who)
	}
}

stock lsg_revive(d, by)
{
	new Float:o[3]
	o = g_lsgDownOrg[d]
	new hp = g_lsgDownHp[d]
	lsg_clear_down(d)
	ExecuteHamB(Ham_CS_RoundRespawn, d)
	if (!is_user_alive(d))
		return
	// back where they fell, when a player fits there
	engfunc(EngFunc_TraceHull, o, o, DONT_IGNORE_MONSTERS, HULL_HUMAN, d, 0)
	if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
		engfunc(EngFunc_SetOrigin, d, o)
	set_user_health(d, hp)
	ls_chat(0, "%s brought %s back.", lsg_name(by), lsg_name(d))
	lsg_revived_hook(d, by)
}"#;

/// Buying rules (Buying, Allow Buying): the cstrike module's buy forward.
const BUYING: &str = r#"stock bool:g_lsgBuyOff, g_lsgBuyAllow

stock bool:lsg_buy_ok(item)
{
	new bool:armor = item == CSI_VEST || item == CSI_VESTHELM
	new bit = item >= 0 && item < 31 ? (1 << item) : 0
	switch (g_lsgBuyAllow)
	{
		case 1: return armor || item >= CSI_DEFUSER || bool:(bit & (CSI_ALL_PISTOLS | CSI_ALL_GRENADES))
		case 2: return !(bit & CSI_ALL_SNIPERRIFLES)
		case 3: return item != CSI_AWP
		case 4: return !(bit & CSI_ALL_GRENADES)
		case 5: return item != CSI_M249
		case 6: return armor
		case 7: return false
	}
	return true
}

public CS_OnBuyAttempt(id, item)
{
	if (g_lsgBuyOff || !lsg_buy_ok(item))
	{
		client_print(id, print_center, "You can't buy that in this mode.")
		return PLUGIN_HANDLED
	}
	return PLUGIN_CONTINUE
}"#;

/// Saved values: one file per mode in the game's addons/amxmodx/data/studio/<mode>.saved, a line
/// "key<TAB>value" each, kept in memory and written on every change (and again at a new round, a
/// disconnect and the map's end), through a .new file renamed over the old one so a crash leaves
/// one whole file. A player is their SteamID, else their name (bots, LAN and pending ids).
const SAVED: &str = r#"stock Trie:g_lsgSaved = Invalid_Trie, bool:g_lsgSavedDirty

stock lsg_saved_path(out[], len, const ext[])
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(out, len, "%s/studio/%s.saved%s", dir, LSG_MODE, ext)
}

stock lsg_saved_load()
{
	g_lsgSaved = TrieCreate()
	new path[192], line[160], key[128], val[24]
	lsg_saved_path(path, charsmax(path), "")
	// (a crash between writing the new file and renaming it: the new one is whole)
	if (!file_exists(path))
		lsg_saved_path(path, charsmax(path), ".new")
	new f = fopen(path, "rt")
	if (!f)
		return
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		new tab = contain(line, "^t")
		if (tab <= 0)
			continue
		line[tab] = 0
		copy(key, charsmax(key), line)
		copy(val, charsmax(val), line[tab + 1])
		TrieSetCell(g_lsgSaved, key, str_to_float(val))
	}
	fclose(f)
}

public lsg_saved_flush()
{
	if (!g_lsgSavedDirty || g_lsgSaved == Invalid_Trie)
		return
	new path[192], tmp[200], dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_saved_path(path, charsmax(path), "")
	lsg_saved_path(tmp, charsmax(tmp), ".new")
	new f = fopen(tmp, "wt")
	if (!f)
		return
	new Snapshot:snap = TrieSnapshotCreate(g_lsgSaved), key[128], Float:v
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, key, charsmax(key))
		TrieGetCell(g_lsgSaved, key, v)
		fprintf(f, "%s^t%f^n", key, v)
	}
	TrieSnapshotDestroy(snap)
	fclose(f)
	// Windows won't rename over a file: the old one goes first (the .new one is read if a crash lands here)
	if (!rename_file(tmp, path, 1))
	{
		delete_file(path)
		rename_file(tmp, path, 1)
	}
	g_lsgSavedDirty = false
}

stock lsg_saved_key(id, const key[], out[], len)
{
	new who[44]
	get_user_authid(id, who, charsmax(who))
	// bots, LAN and pending ids are the same for everyone: their name instead
	if (is_user_bot(id) || !who[0] || containi(who, "LAN") >= 0 || containi(who, "PENDING") >= 0 || containi(who, "BOT") >= 0 || equali(who, "VALVE_ID_LOOPBACK"))
	{
		get_user_name(id, who, charsmax(who))
		formatex(out, len, "%s@name:%s", key, who)
	}
	else
		formatex(out, len, "%s@%s", key, who)
	// (a tab or a line end in a name would break the file's lines)
	for (new i = 0; out[i]; i++)
		if (out[i] < 32)
			out[i] = '_'
}

stock Float:lsg_saved_get(id, const key[])
{
	if (g_lsgSaved == Invalid_Trie || !lsg_ok(id))
		return 0.0
	new k[96], Float:v
	lsg_saved_key(id, key, k, charsmax(k))
	if (!TrieGetCell(g_lsgSaved, k, v))
		return 0.0
	return v
}

stock lsg_saved_set(id, const key[], Float:value)
{
	if (g_lsgSaved == Invalid_Trie || !lsg_ok(id))
		return
	new k[96]
	lsg_saved_key(id, key, k, charsmax(k))
	TrieSetCell(g_lsgSaved, k, value)
	g_lsgSavedDirty = true
	// written straight away: a crash or a killed server keeps it
	lsg_saved_flush()
}"#;

/// The mode's own round clock (the game's time limit is held off while it runs).
const CLOCK: &str = r#"stock bool:g_lsgClockOn, bool:g_lsgClockPaused, bool:g_lsgClockInf, Float:g_lsgClockEnd, Float:g_lsgClockLeft

stock Float:lsg_clock_left()
{
	if (!g_lsgClockOn)
		return 0.0
	if (g_lsgClockPaused)
		return g_lsgClockLeft
	return floatmax(g_lsgClockEnd - get_gametime(), 0.0)
}

stock lsg_clock_set(Float:secs)
{
	// (rounds mustn't end on the game's own clock while ours runs: its "f" flag holds them)
	if (!g_lsgClockInf && !get_cvar_num("mp_round_infinite"))
	{
		new cur[16]
		get_cvar_string("mp_round_infinite", cur, charsmax(cur))
		if (!cur[0] || equal(cur, "0"))
		{
			set_cvar_string("mp_round_infinite", "f")
			g_lsgClockInf = true
		}
	}
	g_lsgClockOn = true
	g_lsgClockPaused = false
	g_lsgClockEnd = get_gametime() + floatmax(secs, 1.0)
	lsg_round_time(floatround(secs))
}

stock lsg_clock_add(Float:secs)
{
	if (!g_lsgClockOn)
		return
	if (g_lsgClockPaused)
		g_lsgClockLeft = floatmax(g_lsgClockLeft + secs, 0.0)
	else
		g_lsgClockEnd += secs
	lsg_round_time(floatround(lsg_clock_left()))
}

stock lsg_clock_pause(pause)
{
	if (!g_lsgClockOn || bool:pause == g_lsgClockPaused)
		return
	if (pause)
	{
		g_lsgClockLeft = lsg_clock_left()
		g_lsgClockPaused = true
	}
	else
	{
		g_lsgClockEnd = get_gametime() + g_lsgClockLeft
		g_lsgClockPaused = false
	}
	lsg_round_time(floatround(lsg_clock_left()))
}

stock lsg_clock_off()
{
	g_lsgClockOn = false
	g_lsgClockPaused = false
	if (g_lsgClockInf)
	{
		set_cvar_string("mp_round_infinite", "0")
		g_lsgClockInf = false
	}
}

public lsg_clock_tick()
{
	if (!g_lsgClockOn)
		return
	// paused: the round timer on screen keeps counting by itself, so it's set again each second
	if (g_lsgClockPaused)
	{
		lsg_round_time(floatround(g_lsgClockLeft))
		return
	}
	if (lsg_clock_left() <= 0.0)
	{
		g_lsgClockOn = false
		lsg_time_up_hook()
	}
}"#;

#[cfg(test)]
mod tests {
    use super::super::graph::{has_errors, parse, registry, validate};
    use super::*;

    const TEMPLATE: &str = include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json");
    pub const ALL_NODES: &str = include_str!("testdata/all_nodes.graph.json");
    pub const EVERY_NODE: &str = include_str!("testdata/every_node.graph.json");
    pub const MALICIOUS: &str = include_str!("testdata/malicious.graph.json");

    fn snapshot(name: &str, got: &str) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/studio/testdata").join(name);
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() || !path.exists() {
            std::fs::write(&path, got).unwrap();
        }
        let want = std::fs::read_to_string(&path).unwrap();
        assert!(want == got, "{name} changed; run with UPDATE_SNAPSHOTS=1 if that's intended.\n--- got ---\n{got}");
    }

    /// Writes a graph's plugin for trying it by hand (a test server, amxxpc):
    /// `LS_STUDIO_GRAPH=x.graph.json LS_STUDIO_OUT=x.sma [LS_STUDIO_DEBUG=1] cargo test studio_write_plugin -- --ignored`
    #[test]
    #[ignore]
    fn studio_write_plugin() {
        let g = parse(&std::fs::read_to_string(std::env::var("LS_STUDIO_GRAPH").unwrap()).unwrap()).unwrap();
        let p = validate(&g);
        assert!(!has_errors(&p), "{p:?}");
        std::fs::write(std::env::var("LS_STUDIO_OUT").unwrap(), generate(&g, std::env::var("LS_STUDIO_DEBUG").is_ok()).source).unwrap();
    }

    #[test]
    fn strings_and_numbers() {
        assert_eq!(pawn_str("a^b"), "\"a^^b\"");
        assert_eq!(pawn_str("q\"x\ny"), "\"qxy\"");
        assert_eq!(float_lit(3.0), "3.0");
        assert_eq!(float_lit(2.5), "2.5");
        assert_eq!(float_lit(f64::INFINITY), "0.0");
        assert_eq!(float_lit(1e9), "100000.0");
        assert_eq!(name_text("a\"b);x"), "abx");
        assert_eq!(keys_text("radius=1 \"x\""), "radius=1 x");
    }

    #[test]
    fn every_event_kind_is_registered_and_known() {
        for k in EVENTS {
            let d = registry().get(k.kind).unwrap_or_else(|| panic!("{}", k.kind));
            assert!(d.event);
            for (pin, _) in k.values {
                assert!(d.outputs.iter().any(|o| o.id == *pin), "{} {}", k.kind, pin);
            }
            let cells = d.outputs.iter().filter(|o| o.ty != "exec" && o.ty != "text").count();
            let texts = d.outputs.iter().filter(|o| o.ty == "text").count();
            assert!(cells <= CTX && texts <= TXT, "{}", k.kind);
        }
        let special = ["func.define", "fn.entry", "macro.entry", "event.chat_command", "event.player_about_hurt", "event.player_revived", "event.down_timeout", "event.time_up"];
        for d in registry().nodes.iter().filter(|d| d.event && !special.contains(&d.kind.as_str())) {
            assert!(event_kind(&d.kind).is_some(), "{} has no handler", d.kind);
        }
    }

    /// Graph version 3's names (enums, structs, Functions, Macros, their pins, locals) are data
    /// too: refused by the checks, and only ever comments or string literals if they got through.
    #[test]
    fn hostile_v3_names_stay_data() {
        let evil = "x\"); server_cmd(\"quit\"); /* ^\"";
        let g = serde_json::json!({
            "schema": "loner-strike/graph", "version": 3, "mode": {"id": "evil3", "name": "Evil"},
            "enums": [{"id": "e1", "name": evil, "values": [evil, "ok"]}],
            "structs": [{"id": "s1", "name": evil, "fields": [{"id": "f1", "name": evil, "type": "text"}, {"id": "f2", "name": "n", "type": "number"}]}],
            "functions": [{"id": "fn1", "name": evil, "inputs": [{"id": "a1", "name": evil, "type": "text"}], "outputs": [{"id": "o1", "name": evil, "type": "struct:s1"}],
                           "locals": [{"id": "l1", "name": evil, "type": "enum:e1", "scope": "mode"}]}],
            "macros": [{"id": "m1", "name": evil, "inputs": [{"id": "go", "name": evil, "type": "exec"}], "outputs": []}],
            "variables": [{"id": "v1", "name": evil, "type": "array<struct:s1>", "scope": "mode"}],
            "nodes": [
                {"id": "r", "type": "event.round_started"},
                {"id": "c", "type": "fn.call", "params": {"func": "fn1"}},
                {"id": "t", "type": "text.format", "params": {"text": evil}},
                {"id": "p", "type": "debug.print", "params": {"text": evil}},
                {"id": "sw", "type": "flow.switch_text", "params": {"cases": evil}},
                {"id": "fe", "type": "fn.entry", "graph": "fn1"},
                {"id": "ev", "type": "enum.value", "graph": "fn1", "params": {"enum": "e1", "value": 0}},
                {"id": "tx", "type": "text.from", "graph": "fn1", "params": {"of": "enum:e1"}},
                {"id": "mk", "type": "struct.make", "graph": "fn1", "params": {"struct": "s1"}},
                {"id": "fr", "type": "fn.return", "graph": "fn1"},
                {"id": "me", "type": "macro.entry", "graph": "m1"}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "r", "pin": "then"}, "to": {"node": "c", "pin": "exec"}},
                {"id": "w2", "from": {"node": "c", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                {"id": "w3", "from": {"node": "t", "pin": "out"}, "to": {"node": "p", "pin": "value"}},
                {"id": "w4", "from": {"node": "p", "pin": "then"}, "to": {"node": "sw", "pin": "exec"}},
                {"id": "w5", "from": {"node": "t", "pin": "out"}, "to": {"node": "sw", "pin": "in"}},
                {"id": "w6", "from": {"node": "fe", "pin": "then"}, "to": {"node": "fr", "pin": "exec"}},
                {"id": "w7", "from": {"node": "ev", "pin": "out"}, "to": {"node": "tx", "pin": "a"}},
                {"id": "w8", "from": {"node": "tx", "pin": "out"}, "to": {"node": "mk", "pin": "f1"}},
                {"id": "w9", "from": {"node": "mk", "pin": "out"}, "to": {"node": "fr", "pin": "o1"}},
                {"id": "w10", "from": {"node": "fe", "pin": "a1"}, "to": {"node": "c", "pin": "a1"}}
            ]
        });
        let g = super::super::graph::from_value(g).unwrap();
        let p = validate(&g);
        let errors: Vec<&str> = p.iter().filter(|p| p.level == "error").map(|p| p.message.as_str()).collect();
        for want in ["Give each Function its own name", "The enum", "Name each input", "Give each struct a name", "Give each Macro its own name", "Give each variable a name", "a wire can't go from one graph to another"] {
            assert!(errors.iter().any(|m| m.to_lowercase().contains(&want.to_lowercase())), "{want}: {errors:#?}");
        }
        for debug in [false, true] {
            let src = generate(&g, debug).source;
            assert!(!src.contains("server_cmd(\"quit"), "{src}");
            assert!(src.contains("server_cmd(quit)"), "the words stay, as text");
            assert_eq!(src.matches("*/").count(), 1, "only the header comment closes");
            for l in src.lines().filter(|l| !l.trim_start().starts_with("//")) {
                assert_eq!(l.replace("^\"", "").replace("'\"'", "").matches('"').count() % 2, 0, "{l}");
            }
            assert!(src.contains("lsg_fr0(") && src.contains("lsg_mks0("), "our own names");
        }
    }

    /// A mode with component nodes loads gm_components (mod.rs writes it into its plugins list);
    /// creature types it spawns are precached as the map loads; Drop Weapon never asks a bot to drop.
    #[test]
    fn modes_bring_what_they_need() {
        let rifle = parse(TEMPLATE).unwrap();
        let n = needs(&rifle);
        assert!(!n.components && n.creatures.is_empty());
        let ls = parse(include_str!("../../../../ui/src/views/tools/studio/templates/last-stand.graph.json")).unwrap();
        let n = needs(&ls);
        assert!(n.components && n.creatures == ["all"]);
        assert!(generate(&ls, false).source.contains("public plugin_precache()"));
        let every = parse(EVERY_NODE).unwrap();
        let src = generate(&every, false).source;
        assert!(src.contains("lsg_drop(") && !src.contains("engclient_cmd(ctx") && src.contains("if (!is_user_bot(id))\n\t{\n\t\tengclient_cmd(id, \"drop\")"));
        // the debugger reports the value the node then uses (worked out once)
        let dbg = generate(&every, true).source;
        assert!(dbg.contains("new Float:lsi"), "inputs into locals");
    }

    #[test]
    fn template_release_snapshot() {
        let g = parse(TEMPLATE).unwrap();
        assert!(validate(&g).is_empty());
        let out = generate(&g, false);
        assert!(!out.source.contains("lsd_"), "no debug code in a release build");
        snapshot("rifle_reward.sma", &out.source);
    }

    #[test]
    fn template_debug_snapshot() {
        let g = parse(TEMPLATE).unwrap();
        let out = generate(&g, true);
        assert!(out.source.contains("lsd_enter(") && out.source.contains("node_enter"));
        snapshot("rifle_reward.debug.sma", &out.source);
    }

    #[test]
    fn all_nodes_snapshot() {
        let g = parse(ALL_NODES).unwrap();
        let p = validate(&g);
        assert!(!has_errors(&p), "{p:?}");
        snapshot("all_nodes.sma", &generate(&g, false).source);
        let dbg = generate(&g, true);
        snapshot("all_nodes.debug.sma", &dbg.source);
        // breakpoints marked in the graph start enabled
        assert!(dbg.source.contains("new bool:g_lsdBp[LSD_N] = { false, false, true"), "{}", dbg.source.lines().find(|l| l.contains("g_lsdBp[LSD_N]")).unwrap_or(""));
    }

    /// One graph with every node in the registry: it checks clean, every flow node becomes a
    /// function, and both builds are snapshotted (and compiled with amxxpc in mod.rs).
    #[test]
    fn every_node_snapshot() {
        let g = parse(EVERY_NODE).unwrap();
        let p = validate(&g);
        assert!(p.is_empty(), "{:#?}", p);
        let used: BTreeSet<&str> = g.nodes.iter().map(|n| n.kind.as_str()).collect();
        for d in &registry().nodes {
            assert!(used.contains(d.kind.as_str()), "every_node.graph.json lacks {}", d.kind);
        }
        let rel = generate(&g, false);
        let dbg = generate(&g, true);
        let in_macro = |n: &super::super::graph::Node| n.graph.as_deref().is_some_and(|h| g.macros.iter().any(|m| m.id == h));
        for n in g.nodes.iter().filter(|n| { let d = registry().get(&n.kind).unwrap(); !d.pure && !d.reroute && !in_macro(n) && !n.kind.starts_with("macro.") }) {
            assert!(rel.spans.iter().any(|s| s.node == n.id), "{} isn't in the plugin", n.id);
        }
        assert!(!rel.source.contains("lsd_"));
        snapshot("every_node.sma", &rel.source);
        snapshot("every_node.debug.sma", &dbg.source);
    }

    /// Community graphs are data: text in them is refused by the checks, and even a graph that
    /// skipped them can only ever become literal text in the plugin.
    #[test]
    fn hostile_text_never_becomes_code() {
        let g = parse(MALICIOUS).unwrap();
        let p = validate(&g);
        let errors = p.iter().filter(|p| p.level == "error").count();
        assert!(errors >= 9, "{p:#?}");
        for debug in [false, true] {
            let src = generate(&g, debug).source;
            assert!(!src.contains("server_cmd(\"quit"), "{src}");
            // the hostile words survive only inside the header comment or a string literal
            let header_end = src.find("*/").unwrap();
            for (i, _) in src.match_indices("plugin_init() {") {
                assert!(i < header_end, "{src}");
            }
            for l in src.lines().filter(|l| l.contains("#include <evil>")) {
                assert!(l.starts_with("\tls_announce(0, 20.0, \"%s\", \"") && l.ends_with("\")"), "{l}");
            }
            assert_eq!(src.lines().filter(|l| l.trim_start().starts_with("#include")).count(), 6);
            assert_eq!(src.matches("*/").count(), src.matches("/*").count());
            assert_eq!(src.matches("*/").count(), 1, "only the header comment closes");
            // every quote in the plugin's own lines is balanced (no string was cut open)
            for l in src.lines().filter(|l| !l.trim_start().starts_with("//")) {
                assert_eq!(l.replace("^\"", "").replace("'\"'", "").matches('"').count() % 2, 0, "{l}");
            }
            assert!(src.contains("ls_give_weapon(ctx[1], \"ak47\", 0)") || debug);
            assert!(src.contains("ls_announce(0, 20.0,"));
            assert!(src.contains("server_cmd(\"endround\")"));
            // variable names are ours, whatever the graph calls them
            assert!(src.contains("g_v0"));
            for l in src.lines().filter(|l| l.contains("evil_var")) {
                assert!(l.contains("// evil_var") && !l.contains("*/"), "{l}");
            }
            assert!(src.contains("lsg_teleport(ctx[0], \"aserver_cmdquit\")") || (debug && src.contains("lsg_teleport(lsi0, \"aserver_cmdquit\")")), "{src}");
            snapshot(if debug { "malicious.debug.sma" } else { "malicious.sma" }, &src);
        }
    }

    #[test]
    fn mode_templates_snapshot() {
        for (name, t) in [
            ("last_stand.sma", include_str!("../../../../ui/src/views/tools/studio/templates/last-stand.graph.json")),
            ("royale.sma", include_str!("../../../../ui/src/views/tools/studio/templates/battle-royale.graph.json")),
        ] {
            let g = parse(t).unwrap();
            assert!(validate(&g).is_empty());
            snapshot(name, &generate(&g, false).source);
        }
    }

    #[test]
    fn spans_point_at_nodes() {
        let g = parse(TEMPLATE).unwrap();
        let out = generate(&g, true);
        let lines: Vec<&str> = out.source.lines().collect();
        for s in &out.spans {
            assert_eq!(lines[s.from - 1], format!("// {} ({})", registry().get(&g.nodes.iter().find(|n| n.id == s.node).unwrap().kind).unwrap().title, s.node));
            assert_eq!(lines[s.to - 1], "}");
        }
        let give = out.spans.iter().find(|s| s.node == "give_ak").unwrap();
        assert_eq!(out.node_at(give.from + 2), Some("give_ak"));
        assert_eq!(out.node_at(1), None);
    }

    #[test]
    fn unreached_nodes_left_out() {
        let mut g = parse(TEMPLATE).unwrap();
        g.edges.retain(|e| e.id != "w8");
        let out = generate(&g, false);
        assert!(out.spans.iter().all(|s| s.node != "give_ak"));
    }
}
