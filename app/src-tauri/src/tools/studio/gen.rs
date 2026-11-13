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

use super::graph::{param, Graph, Node, NodeDef, Variable, View, NUMBER_LIMIT};
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
    /// the nodes that become functions, in graph order (their index is their number)
    live: Vec<&'a str>,
    /// the graph's variables in order (their index is their number)
    vars: Vec<&'a Variable>,
    debug: bool,
}

/// A worked-out data value: the Pawn expression and its type.
struct Val {
    expr: String,
    ty: String,
}

impl<'a> Gen<'a> {
    fn index(&self, id: &str) -> usize {
        self.live.iter().position(|x| *x == id).unwrap_or(0)
    }
    fn fname(&self, id: &str) -> String {
        format!("lsg_n{}", self.index(id))
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
            _ => pawn_str(&safe_text(v.and_then(Value::as_str).unwrap_or(""), 60)),
        }
    }

    /// Where a variable lives: `g_v1`, or `g_pv1[lsg_slot(P)]` for a per-player one.
    fn var_ref(&self, var: &Variable, player: &str) -> String {
        let i = self.var_index(&var.id);
        if var.scope == "player" { format!("g_pv{i}[lsg_slot({player})]") } else { format!("g_v{i}") }
    }

    /// The value of a data input: what's wired in, or the param it falls back to.
    fn input(&self, node: &str, pin: &str) -> Option<Val> {
        let (n, def) = self.node(node);
        let want = self.v.in_type(node, pin).unwrap_or_else(|| "any".into());
        if let Some((src, spin)) = self.v.data_source(node, pin) {
            return Some(self.output(src, spin));
        }
        let ip = def.input(pin)?;
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
        self.input(node, pin).map(|v| v.expr).unwrap_or_else(|| if ty == "number" { "0.0".into() } else if ty == "text" { "\"\"".into() } else { "0".into() })
    }

    /// The value of an output: an event's (from ctx), a pure node's (an expression), or an
    /// action's (the last value it made).
    fn output(&self, node: &str, pin: &str) -> Val {
        let (n, def) = self.node(node);
        let ty = self.v.out_type(node, pin).unwrap_or_else(|| "any".into());
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
                Some(var) => self.var_ref(var, &i("player", "player")),
                None => "0".into(),
            },
            "comp.find" => format!("lsg_find({})", pawn_str(&name_text(self.text(n, def, "name")))),
            _ => "0".into(),
        };
        Val { expr, ty }
    }
}

/// Turns a checked graph into the plugin's source. The graph must have passed
/// `graph::validate` without errors; nodes nothing runs are left out.
pub fn generate(g: &Graph, debug: bool) -> Generated {
    let v = View::new(g);
    let reached = v.reached_from();
    // the nodes that become functions: every event, and whatever their flow reaches
    let live: Vec<&str> = g
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .filter(|id| v.defs.get(id).is_some_and(|d| !d.reroute && !d.pure && (d.event || reached.contains_key(id))))
        .collect();
    let vars: Vec<&Variable> = g.variables.iter().collect();
    let gen = Gen { v, live, vars, debug };
    let v = &gen.v;
    let kinds_live: BTreeSet<&str> = gen.live.iter().map(|id| v.defs[id].kind.as_str()).collect();
    let has = |k: &str| kinds_live.contains(k);
    let timers: Vec<&str> = gen.live.iter().copied().filter(|id| v.defs[id].kind.starts_with("timer.") && v.defs[id].kind != "timer.stop_all").collect();

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
    o.blank();
    o.line(format!("#define LSG_MODE {}\n#define LSG_CTX {CTX}\n#define LSG_TXT {TXT}\n#define LSG_N {}\n#define LSG_TASK {TASK_BASE}", pawn_str(&id_text(&g.mode.id)), gen.live.len().max(1)));
    o.line("stock g_lsgTxt[LSG_TXT][64]\nstock g_lsgOut[LSG_N]\nstock g_lsgLeft[LSG_N]\nstock const g_lsgYesNo[][] = { \"no\", \"yes\" }\nstock const g_lsgTeamName[][] = { \"no team\", \"Terrorists\", \"Counter-Terrorists\", \"Spectators\" }");
    o.blank();

    // variables
    if !gen.vars.is_empty() {
        o.line("// The mode's variables");
        for (i, x) in gen.vars.iter().enumerate() {
            let note = format!("// {} ({}{})", comment_text(&x.name), x.ty, if x.scope == "player" { ", per player" } else { "" });
            let init = Gen::var_lit(&x.ty, Some(&x.default).filter(|d| !d.is_null()));
            let decl = match (x.ty.as_str(), x.scope.as_str()) {
                ("number", "player") => format!("stock Float:g_pv{i}[33]"),
                ("text", "player") => format!("stock g_pv{i}[33][64]"),
                (_, "player") => format!("stock g_pv{i}[33]"),
                ("number", _) => format!("stock Float:g_v{i} = {init}"),
                ("text", _) => format!("stock g_v{i}[64] = {init}"),
                _ => format!("stock g_v{i} = {init}"),
            };
            o.line(format!("{decl} {note}"));
        }
        o.blank();
    }

    if debug {
        let ids: Vec<String> = if gen.live.is_empty() { vec![pawn_str("")] } else { gen.live.iter().map(|id| pawn_str(&id_text(id))).collect() };
        o.line(format!("#define LSD_N {}", gen.live.len().max(1)));
        o.line(format!("new const g_lsdIds[LSD_N][] = {{ {} }}", ids.join(", ")));
        let bps: Vec<&str> = if gen.live.is_empty() { vec!["false"] } else { gen.live.iter().map(|id| if v.nodes[id].breakpoint { "true" } else { "false" }).collect() };
        o.line(format!("new bool:g_lsdBp[LSD_N] = {{ {} }}", bps.join(", ")));
        o.line(include_str!("debug_runtime.pawn").trim_end());
        o.blank();
        // the graph's variables for the live values
        o.line("lsd_graph_vars()\n{");
        for (i, x) in gen.vars.iter().enumerate() {
            let vid = pawn_str(&id_text(&x.id));
            let f = match x.ty.as_str() {
                "number" => "lsd_var_number",
                "bool" => "lsd_var_bool",
                _ => "lsd_var_text",
            };
            if x.scope == "player" {
                o.line(format!("\tfor (new id = 1; id <= MaxClients; id++)\n\t\tif (is_user_connected(id))\n\t\t\t{f}({vid}, id, g_pv{i}[id])"));
            } else {
                o.line(format!("\t{f}({vid}, 0, g_v{i})"));
            }
        }
        o.line("}");
        o.blank();
    }

    o.line(HELPERS);
    o.blank();

    // registration
    let kinds: Vec<&EventKind> = EVENTS.iter().filter(|k| has(k.kind)).collect();
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
    if !timers.is_empty() {
        o.line("\tregister_event(\"HLTV\", \"lsg_new_round\", \"a\", \"1=0\", \"2=0\")");
    }
    o.line("}");
    o.blank();
    if !timers.is_empty() {
        o.line("// timers stop when a new round starts\npublic lsg_new_round()\n\tlsg_stop_timers()\n");
    }
    o.line("stock lsg_stop_timers()\n{\n\tfor (new i = 0; i < LSG_N; i++)\n\t\tremove_task(LSG_TASK + i)\n}");
    o.blank();

    // joining and leaving: per-player variables start again, and the events
    let player_vars: Vec<(usize, &&Variable)> = gen.vars.iter().enumerate().filter(|(_, x)| x.scope == "player").collect();
    if !player_vars.is_empty() || has("event.player_joined") {
        o.line("public client_putinserver(id)\n{");
        for (i, x) in &player_vars {
            let init = Gen::var_lit(&x.ty, Some(&x.default).filter(|d| !d.is_null()));
            if x.ty == "text" {
                o.line(format!("\tcopy(g_pv{i}[id], 63, {init})"));
            } else {
                o.line(format!("\tg_pv{i}[id] = {init}"));
            }
        }
        if has("event.player_joined") {
            o.line("\tlsg_on_player_joined(id)");
        }
        o.line("}");
        o.blank();
    }
    if has("event.player_left") {
        o.line("public client_disconnected(id)\n\tlsg_on_player_left(id)\n");
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
        spans.push(Span { node: id.to_string(), from, to: o.lines });
        o.blank();
    }

    if debug {
        // a paused node runs again from here when the app says resume
        if gen.live.is_empty() {
            o.line("lsd_call(n, const ctx[])\n{\n\t#pragma unused n\n\t#pragma unused ctx\n}");
        } else {
            o.line("lsd_call(n, const ctx[])\n{\n\tswitch (n)\n\t{");
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
    o.line(format!("// {} ({})", comment_text(&def.title), id_text(id)));
    o.line(format!("{}(const ctx[])\n{{", gen.fname(id)));
    let mut b = Out { text: String::new(), lines: 0 };
    if debug {
        b.line("\tlsd_begin()");
        for ip in def.inputs.iter().filter(|p| p.ty != "exec") {
            let Some(val) = gen.input(id, &ip.id) else { continue };
            let label = pawn_str(&safe_text(&ip.label, 30));
            let f = match val.ty.as_str() {
                "player" => "lsd_in_player",
                "entity" => "lsd_in_entity",
                "team" => "lsd_in_team",
                "number" => "lsd_in_number",
                "bool" => "lsd_in_bool",
                "text" => "lsd_in_text",
                _ => continue,
            };
            b.line(format!("\t{f}({label}, {})", val.expr));
        }
        b.line(format!("\tif (lsd_enter({i}, ctx))\n\t\treturn"));
    }
    let next = |pin: &str| -> Option<String> { v.exec_target(id, pin).map(|(t, _)| format!("{}(ctx)", gen.fname(t))) };
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
                let name = pawn_str(&name_text(gen.text(n, def, "name")));
                if debug {
                    b.line(format!("\tif (!ls_alive({p}))\n\t\t{}\n\telse if (!lsg_teleport({p}, {name}))\n\t\t{}\n\telse\n\t{}", err("Not moved: that player isn't alive."), err("Not moved: no entity has that name on this map."), exit(done)));
                } else {
                    b.line(format!("\tlsg_teleport({p}, {name})"));
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
            let cmd = match gen.choice(n, def, "winner") {
                "t" => "endround T",
                "ct" => "endround CT",
                _ => "endround",
            };
            b.line(format!("\tserver_cmd({})", pawn_str(cmd)));
            let e = exit(match gen.choice(n, def, "winner") {
                "t" => "Terrorists win",
                "ct" => "Counter-Terrorists win",
                _ => "draw",
            });
            if !e.is_empty() {
                b.line(e);
            }
        }
        "msg.broadcast" => {
            b.line(format!("\tls_announce(0, {}, \"%s\", {})", float_lit(gen.num(n, def, "seconds")), pawn_str(&safe_text(gen.text(n, def, "text"), 100))));
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
            let args = match gen.input(id, "value") {
                None => format!("\"%s\", {text}"),
                Some(val) => match val.ty.as_str() {
                    "number" => format!("\"%s %d\", {text}, floatround({})", val.expr),
                    "text" => format!("\"%s %s\", {text}, {}", val.expr),
                    "player" => format!("\"%s %s\", {text}, lsg_name({})", val.expr),
                    "bool" => format!("\"%s %s\", {text}, g_lsgYesNo[({}) ? 1 : 0]", val.expr),
                    "team" => format!("\"%s %s\", {text}, g_lsgTeamName[clamp({}, 0, 3)]", val.expr),
                    _ => format!("\"%s %d\", {text}, {}", val.expr),
                },
            };
            b.line(format!("\tls_hud({who}, LsHud:{slot}, {secs}, {args})"));
            let e = exit("shown");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "var.set" | "var.add" => {
            if let Some(var) = v.var_of(id) {
                let r = gen.var_ref(var, &inp("player", "player"));
                let stmt = if def.kind == "var.add" {
                    format!("\t{r} += {}", inp("amount", "number"))
                } else {
                    match var.ty.as_str() {
                        "text" => format!("\tcopy({r}, 63, {})", inp("value", "text")),
                        "bool" => format!("\t{r} = {}", bool01(&inp("value", "bool"))),
                        _ => format!("\t{r} = {}", inp("value", "number")),
                    }
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
        "comp.create" => {
            let kind = pawn_str(gen.choice(n, def, "kind"));
            let at = pawn_str(&name_text(gen.text(n, def, "at")));
            let keys = pawn_str(&keys_text(gen.text(n, def, "keys")));
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!lsg_origin_of({at}, o))\n\t\t{}\n\telse\n\t{{\n\t\tg_lsgOut[{i}] = ls_comp_create({kind}, o, {keys})\n\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))\n\t}}", err("gm_components isn't running on this server."), err("No entity has that name on this map.")));
            } else {
                b.line(format!("\tif (!{comps} && lsg_origin_of({at}, o))\n\t\tg_lsgOut[{i}] = ls_comp_create({kind}, o, {keys})"));
            }
            then_line(&mut b, "then");
        }
        "comp.zone_set" | "comp.zone_shrink" => {
            let z = inp("zone", "entity");
            let at = pawn_str(&name_text(gen.text(n, def, "at")));
            let r = inp("radius", "number");
            let call = if def.kind == "comp.zone_set" { format!("ls_zone_set({z}, o, {r})") } else { format!("ls_zone_shrink({z}, o, {r}, {})", float_lit(gen.num(n, def, "secs"))) };
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!lsg_origin_of({at}, o))\n\t\t{}\n\telse\n\t{{\n\t\t{call}\n\t{}\n\t}}", err("gm_components isn't running on this server."), err("No entity has that name on this map."), exit("done")));
            } else {
                b.line(format!("\tif (!{comps} && lsg_origin_of({at}, o))\n\t\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "comp.crate_open" | "comp.wave_start" | "comp.wave_stop" | "comp.extract_open" | "comp.door" | "comp.trap_fire" | "comp.capture_lock" | "comp.reset" => {
            let call = match def.kind.as_str() {
                "comp.crate_open" => format!("ls_loot_open({}, {})", inp("container", "entity"), inp("player", "player")),
                "comp.wave_start" => format!("ls_wave_start(max(floatround({}), 0))", inp("wave", "number")),
                "comp.wave_stop" => "ls_wave_stop()".into(),
                "comp.extract_open" => format!("ls_extract_open({}, bool:{})", inp("point", "entity"), bool01(&inp("open", "bool"))),
                "comp.door" => {
                    if gen.choice(n, def, "do") == "close" { format!("ls_door_close({})", inp("door", "entity")) } else { format!("ls_door_open({})", inp("door", "entity")) }
                }
                "comp.trap_fire" => format!("ls_trap_fire({})", inp("trap", "entity")),
                "comp.capture_lock" => format!("ls_cp_lock({}, {})", inp("point", "entity"), inp("secs", "number")),
                _ => "ls_comp_reset()".into(),
            };
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\t{call}"), exit("done")]);
            then_line(&mut b, "then");
        }
        "creature.spawn" => {
            let kind = pawn_str(&name_text(gen.text(n, def, "kind")));
            let at = pawn_str(&name_text(gen.text(n, def, "at")));
            if debug {
                b.line(format!("\tif (!get_cvar_pointer(\"ls_creature_ver\"))\n\t\t{}\n\telse if (!(g_lsgOut[{i}] = lsg_creature({kind}, {at})))\n\t\t{}\n\telse\n\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))", err("This server's bot brain has no creatures (ls_creature)."), err("Not spawned: no entity has that name, or that type isn't loaded on this map.")));
            } else {
                b.line(format!("\tg_lsgOut[{i}] = lsg_creature({kind}, {at})"));
            }
            then_line(&mut b, "then");
        }
        _ => {}
    }
    if !b.text.contains("ctx") {
        b.line("\t#pragma unused ctx");
    }
    o.text.push_str(&b.text);
    o.lines += b.lines;
    o.line("}");

    // timer callbacks (they belong to the node, for compiler messages)
    let hold = if debug { format!("\tif (lsd_hold_timer())\n\t{{\n\t\tset_task(0.25, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)\n\t\treturn\n\t}}\n") } else { String::new() };
    match def.kind.as_str() {
        "timer.delay" => {
            o.blank();
            o.line(format!("public lsg_t{i}(const ctx[], task)\n{{\n{hold}\t#pragma unused task"));
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
            o.line(format!("public lsg_t{i}(const ctx[], task)\n{{\n{hold}\t#pragma unused task"));
            // 0 times: until the round ends (timers stop at a new round)
            o.line(format!("\tif (g_lsgLeft[{i}] > 0 && --g_lsgLeft[{i}] == 0)\n\t\tremove_task(LSG_TASK + {i})\n\telse\n\t\tset_task({every}, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)"));
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
            o.line(format!("public lsg_t{i}(const ctx[], task)\n{{\n{hold}\t#pragma unused task"));
            o.line(format!("\tif (--g_lsgLeft[{i}] > 0)\n\t{{\n\t\tlsg_c{i}_show()\n\t\tset_task(1.0, \"lsg_t{i}\", LSG_TASK + {i}, ctx, LSG_CTX)\n\t\treturn\n\t}}"));
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

/// Helpers every plugin gets (stocks: the ones a graph doesn't use cost nothing).
const HELPERS: &str = r#"stock lsg_slot(id)
	return id >= 1 && id <= MaxClients ? id : 0

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
	new ent = lsg_find(name)
	if (ent <= 0)
		return false
	pev(ent, pev_origin, o)
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
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	return is_valid_ent(ent) && !(entity_get_int(ent, EV_INT_flags) & FL_KILLME) ? ent : 0
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
                assert!(d.output(pin).is_some(), "{} {}", k.kind, pin);
            }
            let cells = d.outputs.iter().filter(|o| o.ty != "exec" && o.ty != "text").count();
            let texts = d.outputs.iter().filter(|o| o.ty == "text").count();
            assert!(cells <= CTX && texts <= TXT, "{}", k.kind);
        }
        for d in registry().nodes.iter().filter(|d| d.event) {
            assert!(event_kind(&d.kind).is_some(), "{} has no handler", d.kind);
        }
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
        for n in g.nodes.iter().filter(|n| { let d = registry().get(&n.kind).unwrap(); !d.pure && !d.reroute }) {
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
            assert!(src.contains("lsg_teleport(ctx[0], \"aserver_cmdquit\")"), "{src}");
            snapshot(if debug { "malicious.debug.sma" } else { "malicious.sma" }, &src);
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
