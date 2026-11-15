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

// collision, overlap and hits (W2): collide.rs, a child of this file
#[path = "collide.rs"]
mod collide;
pub(crate) mod p5; // P5: input, cooldowns, pickups, weapon tuning, saving (gen/p5.rs)

// HUD & View: the engine nodes (Y), their own file
pub(crate) mod engine;
// Effects, sounds and spawning (the effects agent), their own file
pub(crate) mod fx;
// engine coverage nodes (EC), their own file
pub(crate) mod coverage;

/// How many event cell values a flow can carry (victim, attacker...).
const CTX: usize = 8;
/// How many event text values (a creature's type) a flow can carry.
const TXT: usize = 2;
/// Task ids for timers: LSG_TASK + the node's index.
const TASK_BASE: u32 = 7_310_000;

/// The plugin's stack and heap in cells: room for every node's frame at once (a flow runs as one
/// call chain), 16384 at least (AMX Mod X's default is 4096), 1 MB at most.
fn stack_cells(nodes: usize) -> usize {
    (4096 + nodes * 640).clamp(16_384, 262_144)
}

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

/// A model path for a string literal: models/....mdl only.
fn model_text(s: &str) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || "_-./".contains(*c)).take(64).collect();
    if t.starts_with("models/") && t.ends_with(".mdl") && !t.contains("..") { t } else { "models/w_c4.mdl".into() }
}

/// A sprite path (under sprites/, .spr) for a string literal: the marker's default otherwise.
fn sprite_text(s: &str) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || "_-./".contains(*c)).take(64).collect();
    if t.starts_with("sprites/") && t.ends_with(".spr") && !t.contains("..") { t } else { "sprites/arrow1.spr".into() }
}

/// Spawn Projectile's start (T1fix): an entity's place (a player's eyes) moved by a local
/// offset along its facing (turned by a rotation offset), and that facing as the direction.
const PROJ_FROM: &str = "\nstock bool:lsg_proj_from(e, Float:f, Float:r, Float:u, rot, copy, Float:o[3], Float:d[3])\n{\n\tif (e <= 0 || !pev_valid(e) || (e <= MaxClients && !is_user_alive(e)))\n\t\treturn false\n\tnew Float:a[3], Float:fw[3], Float:rt[3], Float:up[3]\n\tpev(e, pev_origin, o)\n\tif (e <= MaxClients)\n\t{\n\t\tnew Float:vo[3]\n\t\tpev(e, pev_view_ofs, vo)\n\t\tfor (new k = 0; k < 3; k++)\n\t\t\to[k] += vo[k]\n\t\tpev(e, pev_v_angle, a)\n\t}\n\telse\n\t\tpev(e, pev_angles, a)\n\t// Local offset: along the entity's own forward, right and up\n\tangle_vector(a, ANGLEVECTOR_FORWARD, fw)\n\tangle_vector(a, ANGLEVECTOR_RIGHT, rt)\n\tangle_vector(a, ANGLEVECTOR_UP, up)\n\tfor (new k = 0; k < 3; k++)\n\t\to[k] += fw[k] * f + rt[k] * r + up[k] * u\n\t// which way: its facing (Face the way At entity faces) or none, turned by Rotation offset\n\tif (!copy)\n\t\ta[0] = a[1] = a[2] = 0.0\n\tif (rot >= 0)\n\t\tfor (new k = 0; k < 3; k++)\n\t\t\ta[k] += lsg_vc(rot, k)\n\tangle_vector(a, ANGLEVECTOR_FORWARD, d)\n\treturn true\n}";

/// Live projectiles a Spawn Projectile made (T1fix): which node, and the flow's context, by the
/// projectile's entity; a note for an entity already in the table replaces it (a reused index).
const PROJ_TABLE: &str = "\n#define LSG_PJ 64\nstock g_lsgPjEnt[LSG_PJ], g_lsgPjNode[LSG_PJ], g_lsgPjCtx[LSG_PJ][LSG_CTX], g_lsgPjAt\n\nstock lsg_pj_find(e)\n{\n\tif (e <= 0)\n\t\treturn -1\n\tfor (new s = 0; s < LSG_PJ; s++)\n\t\tif (g_lsgPjEnt[s] == e)\n\t\t\treturn s\n\treturn -1\n}\n\nstock lsg_pj_note(e, node, const ctx[])\n{\n\tnew s = lsg_pj_find(e)\n\tif (s < 0)\n\t{\n\t\tfor (new k = 0; k < LSG_PJ && s < 0; k++)\n\t\t\tif (!g_lsgPjEnt[k] || !pev_valid(g_lsgPjEnt[k]))\n\t\t\t\ts = k\n\t\tif (s < 0)\n\t\t{\n\t\t\ts = g_lsgPjAt\n\t\t\tg_lsgPjAt = (g_lsgPjAt + 1) % LSG_PJ\n\t\t}\n\t}\n\tg_lsgPjEnt[s] = e\n\tg_lsgPjNode[s] = node\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tg_lsgPjCtx[s][k] = ctx[k]\n}\n";

/// A projectile's model or sprite: models/....mdl or sprites/....spr, else a grenade (T1fix).
fn proj_text(s: &str) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || "_-./".contains(*c)).take(64).collect();
    if !t.contains("..") && ((t.starts_with("models/") && t.ends_with(".mdl")) || (t.starts_with("sprites/") && t.ends_with(".spr"))) { t } else { "models/w_hegrenade.mdl".into() }
}

/// A sound path (under sound/, .wav) for a string literal.
fn sound_text(s: &str) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || "_-./".contains(*c)).take(64).collect();
    if t.ends_with(".wav") && !t.contains("..") && !t.starts_with('/') { t } else { "buttons/bell1.wav".into() }
}

/// An entity's name as the map has it (spaces kept) for a string literal.
fn ent_name_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || " _-.".contains(*c)).take(32).collect()
}

/// Component keys for a string literal.
fn keys_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || " _-.=:,|;@".contains(*c)).take(400).collect()
}

/// A Pawn float literal, finite and inside the graph's limits.
pub fn float_lit(x: f64) -> String {
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
        values: &[("winner", "winner"), ("round", "_:float(g_lsgRounds)")],
        tail: "",
        named: None,
    },
    EventKind {
        kind: "event.player_killed",
        sig: "public lsg_on_player_killed()",
        register: &["register_event(\"DeathMsg\", \"lsg_on_player_killed\", \"a\")"],
        prelude: "",
        guard: "\tnew v = read_data(2), a = read_data(1), wpn[24]\n\tread_data(4, wpn, charsmax(wpn))",
        values: &[("victim", "v"), ("attacker", "a"), ("headshot", "read_data(3) ? 1 : 0"), ("weapon", "@str:wpn"), ("distance", "_:lsg_dist(a, v)"), ("team_kill", "a != v && lsg_ok(a) && lsg_ok(v) && get_user_team(a) == get_user_team(v) ? 1 : 0"), ("suicide", "a == v ? 1 : 0"), ("world", "lsg_ok(a) ? 0 : 1")],
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
        values: &[("victim", "victim"), ("attacker", "attacker >= 1 && attacker <= MaxClients ? attacker : 0"), ("damage", "_:damage"), ("headshot", "bits & DMG_BULLET && get_ent_data(victim, \"CBaseMonster\", \"m_LastHitGroup\") == HIT_HEAD ? 1 : 0"), ("left", "_:hp"), ("hitgroup", "@fn:lsg_hitgroup(victim, bits)"), ("weapon", "@fn:lsg_wpn_of(attacker, inflictor)"), ("hit_pos", "lsg_hit_pos(victim)"), ("hit_dir", "lsg_hit_dir(victim, attacker >= 1 && attacker <= MaxClients ? attacker : inflictor)")],
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
    EventKind { kind: "comp.on_barricade_damaged", sig: "public ls_on_barricade_damaged(ent, attacker, hp)", register: &[], prelude: "", guard: "", values: &[("barricade", "ent"), ("attacker", "attacker"), ("hp", "_:float(hp)")], tail: "", named: None },
    EventKind { kind: "comp.on_barricade_repaired", sig: "public ls_on_barricade_repaired(ent, id, hp)", register: &[], prelude: "", guard: "", values: &[("barricade", "ent"), ("player", "id >= 1 && id <= MaxClients ? id : 0"), ("hp", "_:float(hp)")], tail: "", named: None },
    EventKind { kind: "comp.on_drop_landed", sig: "public ls_on_drop(ent, stage, const Float:origin[3])", register: &[], prelude: "", guard: "\t#pragma unused origin\n\tif (stage != 1)\n\t\treturn", values: &[("drop", "ent")], tail: "", named: None },
    EventKind { kind: "up.on_perk", sig: "public ls_on_perk(id, const perk[], price)", register: &[], prelude: "", guard: "", values: &[("player", "id"), ("perk", "@str:perk"), ("price", "_:float(price)")], tail: "\treturn PLUGIN_CONTINUE", named: None },
    EventKind { kind: "up.on_upgraded", sig: "public ls_on_weapon_upgraded(id, weapon, level, price)", register: &[], prelude: "", guard: "\t#pragma unused weapon", values: &[("player", "id"), ("level", "_:float(level)"), ("price", "_:float(price)")], tail: "", named: None },
    EventKind { kind: "up.on_box", sig: "public ls_on_box(ent, id, const weapon[])", register: &[], prelude: "", guard: "\t#pragma unused ent", values: &[("player", "id"), ("weapon", "@str:weapon")], tail: "\treturn PLUGIN_CONTINUE", named: None },
    EventKind { kind: "up.on_pickup", sig: "public ls_on_pickup(ent, id, const kind[])", register: &[], prelude: "", guard: "", values: &[("pickup", "ent"), ("player", "id"), ("kind", "@str:kind")], tail: "\treturn PLUGIN_CONTINUE", named: None },
    EventKind { kind: "up.on_creature_hurt", sig: "public ls_on_creature_hurt(ent, attacker, Float:dmg, headshot)", register: &[], prelude: "", guard: "", values: &[("creature", "ent"), ("attacker", "attacker >= 1 && attacker <= MaxClients ? attacker : 0"), ("damage", "_:dmg"), ("headshot", "headshot ? 1 : 0"), ("hit_pos", "lsg_hit_pos(ent)"), ("hit_dir", "lsg_hit_dir(ent, attacker)")], tail: "", named: None },
    EventKind { kind: "kd.on_down", sig: "public ls_on_down(id, attacker)", register: &[], prelude: "", guard: "", values: &[("player", "id"), ("attacker", "attacker")], tail: "\treturn PLUGIN_CONTINUE", named: None },
    EventKind { kind: "kd.on_up", sig: "public ls_on_revive(id, by)", register: &[], prelude: "", guard: "", values: &[("player", "id"), ("by", "by >= 1 && by <= MaxClients ? by : 0")], tail: "", named: None },
    EventKind { kind: "kd.on_bled_out", sig: "public ls_on_bled_out(id)", register: &[], prelude: "", guard: "", values: &[("player", "id")], tail: "", named: None },
    EventKind { kind: "shop.on_buy", sig: "public ls_on_shop_buy(ent, id, const item[], price)", register: &[], prelude: "", guard: "", values: &[("shop", "ent"), ("player", "id"), ("item", "@str:item"), ("price", "_:float(price)")], tail: "\treturn PLUGIN_CONTINUE", named: None },
    // NPCs and interactables (gm_components ls_npc / ls_interactable)
    EventKind { kind: "npc.on_talk", sig: "public ls_on_npc_talk(id, npc)", register: &[], prelude: "", guard: "\tif (id < 1 || id > MaxClients)\n\t\treturn PLUGIN_CONTINUE", values: &[("player", "id"), ("npc", "npc")], tail: "\treturn PLUGIN_CONTINUE", named: Some("npc") },
    EventKind { kind: "npc.on_use", sig: "public ls_on_interact(id, ent)", register: &[], prelude: "", guard: "\tif (id < 1 || id > MaxClients)\n\t\treturn PLUGIN_CONTINUE", values: &[("player", "id"), ("entity", "ent")], tail: "\treturn PLUGIN_CONTINUE", named: Some("ent") },
    // T1fix: projectiles (gm_components)
    EventKind { kind: "proj.on_hit", sig: "public ls_on_proj_hit(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)", register: &[], prelude: "", guard: "\t#pragma unused pos\n\t#pragma unused normal", values: &[("proj", "proj"), ("owner", "owner"), ("pos", "lsg_vfrom(pos)"), ("normal", "lsg_vfrom(normal)"), ("hit", "hit"), ("world", "(hit == 0 ? 1 : 0)"), ("hitgroup", "_:float(hitgroup)")], tail: "", named: None },
    EventKind { kind: "br.on_landed", sig: "public ls_on_landed(id)", register: &[], prelude: "", guard: "", values: &[("player", "id")], tail: "", named: None },
    EventKind { kind: "br.on_lobby_end", sig: "public ls_on_lobby_end(squads)", register: &[], prelude: "", guard: "", values: &[("squads", "_:float(squads)")], tail: "", named: None },
    EventKind { kind: "turret.on_placed", sig: "public ls_on_turret(ent, owner)", register: &[], prelude: "", guard: "", values: &[("turret", "ent"), ("owner", "owner >= 1 && owner <= MaxClients ? owner : 0")], tail: "", named: None },
    EventKind { kind: "event.player_said", sig: "public lsg_on_said(id)", register: &["register_clcmd(\"say\", \"lsg_on_said\")", "register_clcmd(\"say_team\", \"lsg_on_said\")"], prelude: "", guard: "\tnew txt[128], cmd[12]\n\tread_argv(0, cmd, charsmax(cmd))\n\tread_args(txt, charsmax(txt))\n\tremove_quotes(txt)\n\tif (!lsg_ok(id))\n\t\treturn PLUGIN_CONTINUE", values: &[("player", "id"), ("text", "@str:txt"), ("team", "equal(cmd, \"say_team\") ? 1 : 0")], tail: "\treturn PLUGIN_CONTINUE", named: None },
    EventKind { kind: "event.freeze_ended", sig: "public lsg_on_freeze_ended()", register: &["register_logevent(\"lsg_on_freeze_ended\", 2, \"1=Round_Start\")"], prelude: "", guard: "", values: &[], tail: "", named: None },
    EventKind { kind: "event.map_ending", sig: "public lsg_on_map_ending()", register: &["register_event(\"30\", \"lsg_on_map_ending\", \"a\")"], prelude: "", guard: "", values: &[], tail: "", named: None },
    EventKind { kind: "event.weapon_fired", sig: "public lsg_on_fired(wpn)", register: &["lsg_hook_weapons(\"lsg_on_fired\", Ham_Weapon_PrimaryAttack)"], prelude: "", guard: "\tstatic Float:last[33]\n\tnew id = pev_valid(wpn) ? pev(wpn, pev_owner) : 0, wn[24]\n\tif (!lsg_ok(id) || get_gametime() - last[id] < 0.1)\n\t\treturn HAM_IGNORED\n\tlast[id] = get_gametime()\n\tpev(wpn, pev_classname, wn, charsmax(wn))\n\treplace(wn, charsmax(wn), \"weapon_\", \"\")", values: &[("player", "id"), ("weapon", "@str:wn")], tail: "\treturn HAM_IGNORED", named: None },
    EventKind { kind: "event.reloaded", sig: "public lsg_on_reloaded(wpn)", register: &["lsg_hook_weapons(\"lsg_on_reloaded\", Ham_Weapon_Reload)"], prelude: "", guard: "\tnew id = pev_valid(wpn) ? pev(wpn, pev_owner) : 0, wn[24]\n\tif (!lsg_ok(id))\n\t\treturn HAM_IGNORED\n\tpev(wpn, pev_classname, wn, charsmax(wn))\n\treplace(wn, charsmax(wn), \"weapon_\", \"\")", values: &[("player", "id"), ("weapon", "@str:wn")], tail: "\treturn HAM_IGNORED", named: None },
    EventKind { kind: "event.hostage_rescued", sig: "public lsg_on_hostage_rescued()", register: &["register_logevent(\"lsg_on_hostage_rescued\", 3, \"2=Rescued_A_Hostage\")"], prelude: "", guard: "", values: &[("player", "lsg_log_player()")], tail: "", named: None },
    EventKind { kind: "event.vip_escaped", sig: "public lsg_on_vip_escaped()", register: &["register_logevent(\"lsg_on_vip_escaped\", 3, \"2=Escaped_As_VIP\")"], prelude: "", guard: "", values: &[("player", "lsg_log_player()")], tail: "", named: None },
    EventKind { kind: "amx.on_event", sig: "public ls_studio_event(const name[], a, b)", register: &[], prelude: "", guard: "", values: &[("a", "_:float(a)"), ("b", "_:float(b)")], tail: "", named: Some("@name") },
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
const CTX_KINDS: [&str; 14] = ["table.for_each", "creature.for_each", "timeline.play", "marker.for_each", "flow.for_each_player", "flow.for_each_entity", "flow.pick_player", "flow.pick_entity", "func.define", "fn.entry", "flow.for_loop", "flow.for_loop_break", "flow.for_each_array", "event.custom"];
/// Outputs kept in g_lsgO / g_lsgOT (what an action made).
const STORE_KINDS: [&str; 15] = ["save.top", "array.add", "array.remove", "flow.do_n", "flow.flipflop", "fn.call", "comp.loot_roll", "game.run_command", "game.amx_command", "amx.call", "up.perk_buy", "npc.choose", "fun.swap", "cast.player", "cast.creature"];
/// Nodes with more than one flow input: one Pawn function each.
const MULTI_IN: [&str; 6] = ["timeline.play", "flow.do_once", "flow.do_n", "flow.gate", "flow.for_loop_break", "flow.for_each_array"];
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
        if (MULTI_IN.contains(&def.kind.as_str()) || w1::MULTI.contains(&def.kind.as_str())) && pin != "exec" {
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
    /// A comfort limit's value in the plugin (limits.rs): its mode-setting variable when the mode
    /// has it (hosts can change it), else the default; clamped to what the plugin can hold.
    pub fn limit_expr(&self, id: &str) -> String {
        let l = super::limits::get(id).expect("a limit in limits.rs");
        match super::limits::var(self.v.g, id) {
            Some(v) => format!("clamp(floatround({}), 1, {})", self.var_ref(v, "0", "0"), l.max),
            None => l.default.to_string(),
        }
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
        // (a mode setting: back to the host's value)
        if x.setting.is_some() && x.scope == "mode" {
            if let Some(k) = self.v.g.variables.iter().filter(|y| y.setting.is_some() && y.scope == "mode" && matches!(y.ty.as_str(), "number" | "bool")).position(|y| y.id == x.id) {
                return if x.ty == "bool" { format!("g_v{i} = floatround(lsg_sv({k})) != 0") } else { format!("g_v{i} = lsg_sv({k})") };
            }
        }
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
        // Get Bone / Attachment Position (engine.rs)
        if let Some(e) = p5::pure_value(self, node, n, def, pin) {
            return Val { expr: e, ty }; // P5
        }
        if let Some(e) = engine::pure_value(self, node, n, def, pin) {
            return Val { expr: e, ty };
        }
        let expr = match def.kind.as_str() {
            k if coverage::is_pure(k) => coverage::pure(self, node, pin), // EC
            k if w1::PURE.contains(&k) => w1::pure(self, node, pin), // W1
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
            "up.mod_get" => format!("(ls_components_running() ? ls_player_mod_get({}, {}) : 1.0)", i("player", "player"), pawn_str(self.choice(n, def, "what"))),
            "up.perk_has" => format!("(ls_components_running() && ls_perk_has({}, {}) ? 1 : 0)", i("player", "player"), pawn_str(self.choice(n, def, "perk"))),
            "up.perk_count" => format!("float(ls_components_running() ? ls_perk_count({}) : 0)", i("player", "player")),
            "up.weapon_level" => format!("float(ls_components_running() && lsg_ok({p}) ? ls_weapon_level({p}) : 0)", p = i("player", "player")),
            "kd.is" => format!("(ls_components_running() && ls_is_down({}) ? 1 : 0)", i("player", "player")),
            "kd.left" => format!("(ls_components_running() ? ls_down_left({}) : 0.0)", i("player", "player")),
            "shop.price" => format!("float(lsg_kind_ok({0}, \"ls_shop\") ? ls_shop_price({0}, {1}) : -1)", i("shop", "entity"), pawn_str(&safe_text(self.text(n, def, "item"), 40))),
            "br.heal_count" => format!("float(ls_components_running() ? ls_heal_count({}) : 0)", i("player", "player")),
            "br.kills" => format!("float(ls_components_running() ? ls_kills({}) : 0)", i("player", "player")),
            "br.squads_alive" => "float(ls_components_running() ? ls_squads_alive() : 0)".into(),
            "game.map_name" => "lsg_map_name()".into(),
            "game.next_map" => "lsg_cvar_text(\"amx_nextmap\")".into(),
            "game.server_name" => "lsg_cvar_text(\"hostname\")".into(),
            "game.max_players" => "float(MaxClients)".into(),
            "game.player_count" => format!("float(lsg_count({}))", match self.choice(n, def, "which") { "humans" => 1, "bots" => 2, "t" => 3, "ct" => 4, "spec" => 5, "alive" => 6, _ => 0 }),
            "game.time" => "get_gametime()".into(),
            "game.clock" => if pin == "unix" { "float(get_systime())".into() } else { format!("lsg_clock({})", pawn_str(match self.choice(n, def, "format") { "date" => "%Y-%m-%d", "time" => "%H:%M", "weekday" => "%A", "hour" => "%H", _ => "%Y-%m-%d %H:%M" })) },
            "game.mode_id" => "LSG_MODE".into(),
            "game.round_number" => "float(g_lsgRounds)".into(),
            "game.team_score" => format!("float(g_lsgScore[{}])", if self.choice(n, def, "team") == "t" { 1 } else { 2 }),
            "game.freeze_time" => "(g_lsgFreeze ? 1 : 0)".into(),
            "game.get_setting" => {
                let c = pawn_str(&name_text(self.text(n, def, "cvar")));
                if pin == "text" { format!("lsg_cvar_text({c})") } else { format!("lsg_cvar_num({c})") }
            }
            k if k.starts_with("world.") && !k.starts_with("world.set_") => match world_of(k) {
                Some((cvar, true, _)) => format!("(get_cvar_num({}) ? 1 : 0)", pawn_str(cvar)),
                Some((cvar, false, _)) => format!("get_cvar_float({})", pawn_str(cvar)),
                None => "0".into(),
            },
            "player.armor" => format!("float(lsg_ok({p}) ? get_user_armor({p}) : 0)", p = i("player", "player")),
            "player.has_helmet" => format!("(lsg_helmet({}) ? 1 : 0)", i("player", "player")),
            "player.weapon_held" => format!("lsg_held({})", i("player", "player")),
            "player.ammo" => format!("float(lsg_ammo({}, {}))", i("player", "player"), if pin == "reserve" { 1 } else { 0 }),
            "player.has_weapon" => format!("(lsg_ok({p}) && user_has_weapon({p}, get_weaponid({})) ? 1 : 0)", pawn_str(&format!("weapon_{}", self.choice(n, def, "weapon"))), p = i("player", "player")),
            "player.is_admin" => format!("(lsg_ok({p}) && (get_user_flags({p}) & ADMIN_KICK) ? 1 : 0)", p = i("player", "player")),
            "player.is_vip" => format!("(lsg_ok({p}) && (get_user_flags({p}) & ADMIN_RESERVATION) ? 1 : 0)", p = i("player", "player")),
            "player.ping" => format!("float(lsg_ping({}))", i("player", "player")),
            "player.steam_id" => format!("lsg_steam_id({})", i("player", "player")),
            "player.is_spectator" => format!("(lsg_ok({p}) && (get_user_team({p}) != 1 && get_user_team({p}) != 2) ? 1 : 0)", p = i("player", "player")),
            "player.time_connected" => format!("float(lsg_ok({p}) ? get_user_time({p}) : 0)", p = i("player", "player")),
            "player.by_name" => format!("lsg_by_name({})", i("name", "text")),
            "player.by_steam" => {
                let who = format!("lsg_by_steam({})", i("id", "text"));
                if pin == "found" { format!("({who} > 0 ? 1 : 0)") } else { who }
            }
            "player.is_supporter" => format!("(lsg_supporter({}) ? 1 : 0)", i("player", "player")),
            "ent.kind_of" => format!("lsg_kind_of({})", i("in", "entity")),
            "ent.comp_kind" => format!("lsg_comp_kind_of({})", i("in", "entity")),
            "ent.is_npc" => format!("(lsg_kind_ok({}, \"ls_npc\") ? 1 : 0)", i("in", "entity")),
            "ent.is_interactable" => format!("(lsg_kind_ok({}, \"ls_interactable\") ? 1 : 0)", i("in", "entity")),
            "ent.is_component" => {
                let k = self.choice(n, def, "kind");
                // (one of the list, else any: never a typed name)
                let k = if COMP_KINDS.contains(&k) { k } else { "" };
                format!("(lsg_kind_ok({}, {}) ? 1 : 0)", i("in", "entity"), pawn_str(k))
            }
            "fun.is_third" => format!("(lsg_ok({p}) && g_lsgCam[{p}] ? 1 : 0)", p = i("player", "player")),
            // collision, overlap and hits (W2)
            "col.health" => format!("lsg_ent_health({})", i("ent", "entity")),
            "vol.is_overlapping" => format!("lsg_vol_has({}, {})", i("volume", "entity"), i("ent", "entity")),
            "player.by_index" => format!("lsg_slot_ok(floatround({}))", i("i", "number")),
            "player.gravity" => format!("lsg_pevf({}, pev_gravity)", i("player", "player")),
            "player.speed" => format!("lsg_pevf({}, pev_maxspeed)", i("player", "player")),
            "player.model" => format!("lsg_model({})", i("player", "player")),
            "player.transform" => {
                let p = i("player", "player");
                match pin {
                    "angles" => format!("lsg_vpev({p}, pev_v_angle)"),
                    "vel" => format!("lsg_vpev({p}, pev_velocity)"),
                    "ground" => format!("(lsg_ok({p}) && (pev({p}, pev_flags) & FL_ONGROUND) ? 1 : 0)"),
                    "duck" => format!("(lsg_ok({p}) && (pev({p}, pev_flags) & FL_DUCKING) ? 1 : 0)"),
                    _ => format!("lsg_vpev({p}, pev_origin)"),
                }
            }
            "ent.class" => format!("lsg_eclass({})", i("in", "entity")),
            "ent.name" => format!("lsg_ename({})", i("in", "entity")),
            "ent.find_all" => format!("lsg_find_all({}, {})", if self.choice(n, def, "by") == "name" { 1 } else { 0 }, pawn_str(&ent_name_text(self.text(n, def, "name")))),
            "amx.plugin_running" => format!("(lsg_plugin_on({}) ? 1 : 0)", pawn_str(&name_text(self.text(n, def, "plugin")))),
            "wave.info" => {
                let on = "ls_components_running()";
                match pin {
                    "spawned" => format!("float({on} ? ls_wave_spawned() : 0)"),
                    "hp" => format!("({on} ? ls_wave_hp_mult() : 1.0)"),
                    "boss" => "(lsg_wave_kind() == 1 ? 1 : 0)".into(),
                    "pack" => "(lsg_wave_kind() == 2 ? 1 : 0)".into(),
                    "type" => "lsg_wave_type()".into(),
                    _ => format!("float({on} ? ls_wave_planned() : 0)"),
                }
            }
            "wave.type_pay" => format!("float(ls_components_running() ? ls_wave_type_pay({}) : 0)", i("type", "text")),
            "wave.creatures_ok" => "(ls_components_running() && ls_wave_creatures_ok() ? 1 : 0)".into(),
            "turret.kits" => format!("float(ls_components_running() ? ls_turret_kits({}) : 0)", i("player", "player")),
            "turret.count" => format!("float(ls_components_running() ? ls_turret_count({}) : 0)", i("player", "player")),
            "comp.barricade_hp" => format!("float(lsg_kind_ok({0}, \"ls_barricade\") ? ls_barricade_hp({0}) : 0)", i("barricade", "entity")),
            "comp.zone_stage" => format!("float(ls_components_running() ? ls_zone_stage({}) : 0)", i("zone", "entity")),
            "comp.zone_next" => format!("{}lsg_zone_next({}, {})", if pin == "closing" { "_:" } else { "" }, i("zone", "entity"), match pin { "radius" => 1, "closing" => 2, _ => 0 }),
            "comp.in_zone" => format!("(ls_components_running() && ls_zone_inside({}, {}) ? 1 : 0)", i("zone", "entity"), i("player", "player")),
            "creature.health" => format!("lsg_ehp({})", i("in", "entity")),
            "creature.count" => "float(lsg_creatures_alive())".into(),
            "player.is_downed" => format!("(g_lsgDown[lsg_slot({})] ? 1 : 0)", i("player", "player")),
            "squad.get" => format!("float(g_lsgSquad[lsg_slot({})])", i("player", "player")),
            "squad.alive" => "float(lsg_squads_alive())".into(),
            "save.get" => format!("lsg_saved_get({}, {})", i("player", "player"), pawn_str(&name_text(self.text(n, def, "key")))),
            "clock.left" => "lsg_clock_left()".into(),
            "marker.count" => format!("float(lsg_mk_count({}))", marker_index(self.v.g, self.text(n, def, "marker"))),
            "table.get_row" | "table.get_cell" | "table.find" | "table.count" | "table.keys" => match super::tables::table_of(&self.v.g.data_tables, n) {
                None => Self::zero(&ty),
                Some((t, tb)) => {
                    let col = |c: &str| tb.col(c).map(|(k, _)| k);
                    match def.kind.as_str() {
                        "table.get_row" => {
                            let row = format!("lsg_dt{t}_find({})", self.needt(node, "key"));
                            match pin {
                                "found" => format!("({row} >= 0.0 ? 1 : 0)"),
                                "row" => row,
                                c => col(c).map(|k| format!("lsg_dt{t}_{k}({row})")).unwrap_or_else(|| Self::zero(&ty)),
                            }
                        }
                        "table.get_cell" => col(self.text(n, def, "column")).map(|k| format!("lsg_dt{t}_{k}({})", self.need(node, "row", "number"))).unwrap_or_else(|| Self::zero(&ty)),
                        "table.find" => col(self.text(n, def, "column")).map(|k| format!("lsg_dt{t}_where{k}({})", self.needt(node, "value"))).unwrap_or_else(|| "lsg_atmp()".into()),
                        "table.count" => format!("float(LSG_DT{t}_N)"),
                        _ => format!("lsg_dt{t}_keys()"),
                    }
                }
            },
            "marker.find" => {
                let get = format!("lsg_mk_get({}, floatround({}))", marker_index(self.v.g, self.text(n, def, "marker")), i("n", "number"));
                match pin {
                    "pos" => format!("lsg_vent({get})"),
                    "found" => format!("({get} > 0 ? 1 : 0)"),
                    _ => get,
                }
            }
            "marker.get" => {
                let key = pawn_str(&name_text(self.text(n, def, "key")));
                if pin == "num" { format!("str_to_float(lsg_mk_key({}, {key}))", i("in", "entity")) } else { format!("lsg_mk_key({}, {key})", i("in", "entity")) }
            }
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
            "bot.skill" => match pin {
                "style" => format!("lsg_bot_style({})", i("bot", "player")),
                p => format!("lsg_bot_nat({}, {})", i("bot", "player"), match p { "aggression" => 1, "teamwork" => 2, "greed" => 3, _ => 0 }),
            },
            "bot.trait" => format!("(lsg_bot_trait({}, {}) ? 1 : 0)", i("bot", "player"), match self.choice(n, def, "trait") { "cautious" => 2, "team" => 4, "loner" => 8, "greedy" => 16, "skilled" => 32, "novice" => 64, _ => 1 }),
            "creature.of_type" => format!("lsg_cr_all({})", pawn_str(&name_text(self.text(n, def, "type")))),
            "creature.nearest" => {
                let pos = if self.v.data_source(node, "pos").is_some() { i("pos", "vector") } else { "-1".into() };
                let to = if self.v.data_source(node, "to").is_some() { i("to", "entity") } else { "0".into() };
                format!("lsg_cr_nearest({}, {pos}, {to})", pawn_str(&name_text(self.text(n, def, "type"))))
            }
            "creature.type" => format!("lsg_cr_type({})", i("creature", "entity")),
            "creature.state" => if pin == "name" { format!("lsg_cr_state_name({})", i("creature", "entity")) } else { format!("float(lsg_cr_state({}))", i("creature", "entity")) },
            "creature.target" => format!("lsg_cr_target({})", i("creature", "entity")),
            "creature.idle_for" => format!("lsg_cr_idle({})", i("creature", "entity")),
            "creature.can_see" => format!("(lsg_cr_sees({}, {}) ? 1 : 0)", i("creature", "entity"), i("what", "entity")),
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
                    "trunc" => format!("float(floatround({a}, floatround_tozero))"), // W1
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
            "vec.entity" | "ent.position" => format!("lsg_vent({})", i("in", "entity")),
            "proj.velocity" => format!("lsg_proj_vel({})", i("proj", "entity")), // T1fix
            "ent.rotation" => format!("lsg_erot({})", i("in", "entity")),
            "ent.transform" => {
                let e = i("in", "entity");
                match pin {
                    "angles" => format!("lsg_erot({e})"),
                    "vel" => format!("lsg_vpev({e}, pev_velocity)"),
                    "mins" => format!("lsg_vpev({e}, pev_absmin)"),
                    "maxs" => format!("lsg_vpev({e}, pev_absmax)"),
                    "center" => format!("lsg_ecenter({e})"),
                    "ground" => format!("({e} > 0 && pev_valid({e}) && (pev({e}, pev_flags) & FL_ONGROUND) ? 1 : 0)"),
                    _ => format!("lsg_vent({e})"),
                }
            }
            "ent.distance" | "ent.direction" => {
                let side = |ent: &str, pos: &str| if self.v.data_source(node, pos).is_some() { i(pos, "vector") } else if self.v.data_source(node, ent).is_some() { format!("lsg_vent({})", i(ent, "entity")) } else { "lsg_vec(0.0, 0.0, 0.0)".into() };
                let (a, b) = (side("a", "a_pos"), side("b", "b_pos"));
                if def.kind == "ent.distance" { format!("lsg_vlen(lsg_vadd({b}, {a}, -1.0))") } else { format!("lsg_vdir({a}, {b})") }
            }
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
    if ex.graph.nodes.iter().any(|n| n.kind.starts_with("comp.wave") || n.kind.starts_with("wave.") || n.kind.starts_with("turret.") || n.kind == "comp.on_wave_spawn" || n.kind == "comp.load_pack") {
        creatures = vec!["all".into()];
    }
    Needs { components, creatures }
}

/// Turns a checked graph into the plugin's source. The graph must have passed
/// `graph::validate` without errors; nodes nothing runs are left out.
pub fn generate(checked: &Graph, debug: bool) -> Generated {
    // the extra event graphs fold into the first (events in tab order)
    let flat = super::graph::flatten(checked);
    // Macros are copied in first (their nodes report as the Macro node)
    let ex = super::expand::expand(flat.as_ref());
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
        if !ctx && !STORE_KINDS.contains(&d.kind.as_str()) && !w1::STORE.contains(&d.kind.as_str()) {
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
    let uses_save = any(&["save.set", "save.add"]) || pure_used(&gen, "save.get") || p5::uses_saved_numbers(g); // P5: server values, Top Players
    let uses_clock = any(&["clock.set", "clock.add", "clock.pause", "event.time_up"]) || pure_used(&gen, "clock.left");
    // mode settings: variables hosts can change (cvars, kept per server)
    let settings: Vec<&Variable> = g.variables.iter().filter(|x| x.setting.is_some() && x.scope == "mode" && matches!(x.ty.as_str(), "number" | "bool")).collect();
    let new_round = !timers.is_empty() || !round_vars.is_empty() || uses_clock || uses_down || uses_save || !settings.is_empty();
    // what graph version 3 brought: the values that need the bigger helpers
    // Player Hurt's / Creature Hurt's Hit Position and Direction: only kept when something uses them
    let hit_used = |kind: &str| g.nodes.iter().filter(|n| n.kind == kind).any(|n| g.edges.iter().any(|e| e.from.node == n.id && (e.from.pin == "hit_pos" || e.from.pin == "hit_dir")));
    let (hit_players, hit_creatures) = (hit_used("event.player_hurt"), hit_used("up.on_creature_hurt"));
    let uses_v3 = hit_players
        || hit_creatures
        || !g.functions.is_empty()
        || !g.structs.is_empty()
        || g.enums.iter().any(|e| !e.builtin)
        || gen.vars.iter().any(|x| !matches!(x.ty.as_str(), "number" | "bool" | "text"))
        || w1::used(g) // W1
        || coverage::used(g) // EC
        || g.nodes.iter().any(|n| v.defs.get(n.id.as_str()).is_some_and(|d| V3_KINDS.iter().any(|k| d.kind.starts_with(k))))
        || g.nodes.iter().any(|n| fx::is_action(&n.kind))
        || fx::xf_used(g);

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
    if graph_uses(g, &["save.get", "save.set", "save.add"]) || p5::used(g) || uses_v3 {
        o.line("#include <celltrie>");
    }
    if uses_v3 {
        o.line("#include <cellarray>");
    }
    o.blank();
    o.line(format!("#define LSG_MODE {}\n#define LSG_CTX {}\n#define LSG_TXT {TXT}\n#define LSG_N {}\n#define LSG_TASK {TASK_BASE}", pawn_str(&id_text(&g.mode.id)), CTX + gen.slots.len(), gen.live.len().max(1)));
    // (each node calls the next, so a flow's nodes are all on the stack at once, a Print's two
    // text buffers about 400 cells: AMX Mod X's default 4096 cells ran out after some ten of them
    // in a row and the plugin died with "stack error" or, without debug, garbage)
    o.line(format!("#pragma dynamic {}", stack_cells(gen.live.len())));
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
    if !settings.is_empty() {
        o.line(settings_block(&gen, &g.mode.id, &settings));
    }

    if has("event.player_about_hurt") || graph_uses(g, &["dmg.block", "dmg.scale", "dmg.set"]) {
        o.line("stock Float:g_lsgDmg, bool:g_lsgDmgBlock");
    }
    if uses_nofall {
        o.line("stock g_lsgNoFall[33]");
    }
    // menus remember who was shown what
    for id in gen.live.iter().filter(|id| v.defs[*id].kind == "msg.menu" || v.defs[*id].kind == "npc.choose") {
        o.line(format!("stock g_lsgM{}[33][LSG_CTX]", gen.index(id)));
    }
    // Say and Choose: which node's question each player has open (its index + 1; 0 none)
    if gen.live.iter().any(|id| v.defs[*id].kind == "npc.choose") {
        o.line("stock g_lsgAsk[33], g_lsgAskK[33][6]");
    }
    if gen.live.iter().any(|id| matches!(v.defs[*id].kind.as_str(), "npc.say" | "npc.choose")) {
        // a line with its speaker ("Old Sam: ..."); a reply added to the |-list, its number noted
        o.line("\nstock lsg_npc_line(out[], len, const speaker[], const text[])\n{\n\tif (speaker[0])\n\t\tformatex(out, len, \"%s: %s\", speaker, text)\n\telse\n\t\tcopy(out, len, text)\n}");
    }
    if gen.live.iter().any(|id| v.defs[*id].kind == "npc.choose") {
        o.line("\nstock lsg_npc_rep(out[], len, id, &n, cap, k, const r[])\n{\n\tif (n >= cap || !r[0] || id < 1 || id > MaxClients)\n\t\treturn\n\tnew t[48]\n\tcopy(t, charsmax(t), r)\n\treplace_all(t, charsmax(t), \"|\", \"/\")\n\tif (n)\n\t\tadd(out, len, \"|\")\n\tadd(out, len, t)\n\tg_lsgAskK[id][n++] = k\n}");
    }
    // Say and Choose: a reply (0-5), -1 timed out, -2 walked away: to the node that asked
    let asks: Vec<usize> = gen.live.iter().filter(|id| v.defs[*id].kind == "npc.choose").map(|id| gen.index(id)).collect();
    if !asks.is_empty() {
        let mut d = String::from("\npublic ls_on_npc_choice(id, npc, choice)\n{\n\t#pragma unused npc\n\tif (id < 1 || id > MaxClients)\n\t\treturn PLUGIN_CONTINUE\n\tnew k = g_lsgAsk[id]\n\tg_lsgAsk[id] = 0\n\tswitch (k)\n\t{\n");
        for i in &asks {
            d.push_str(&format!("\t\tcase {}:\n\t\t\tlsg_ask{i}(id, choice)\n", i + 1));
        }
        d.push_str("\t}\n\treturn PLUGIN_CONTINUE\n}");
        o.line(d);
    }
    if g.nodes.iter().any(|n| n.kind == "proj.spawn") {
        o.line(PROJ_FROM); // T1fix
        // which Spawn Projectile made each live projectile, and its flow's context
        let spawns: Vec<usize> = gen.live.iter().filter(|id| v.defs[*id].kind == "proj.spawn").map(|id| gen.index(id)).collect();
        let mut d = String::from(PROJ_TABLE);
        d.push_str("\npublic ls_on_proj_hit_node(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)\n{\n\tnew s = lsg_pj_find(proj)\n\tif (s < 0)\n\t\treturn\n\tnew ctx[LSG_CTX]\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tctx[k] = g_lsgPjCtx[s][k]\n\tswitch (g_lsgPjNode[s])\n\t{\n");
        for i in &spawns {
            d.push_str(&format!("\t\tcase {}:\n\t\t\tlsg_pjhit{i}(ctx, owner, pos, normal, hit, hitgroup)\n", i + 1));
        }
        d.push_str("\t}\n}\n\npublic ls_on_proj_expire(proj, owner)\n{\n\t#pragma unused owner\n\tnew s = lsg_pj_find(proj)\n\tif (s < 0)\n\t\treturn\n\tnew ctx[LSG_CTX], k = g_lsgPjNode[s]\n\tfor (new c = 0; c < LSG_CTX; c++)\n\t\tctx[c] = g_lsgPjCtx[s][c]\n\tg_lsgPjEnt[s] = 0\n\tswitch (k)\n\t{\n");
        for i in &spawns {
            d.push_str(&format!("\t\tcase {}:\n\t\t\tlsg_pjexp{i}(ctx)\n", i + 1));
        }
        d.push_str("\t}\n}");
        o.line(d);
    }
    if g.nodes.iter().any(|n| n.kind == "proj.velocity") {
        o.line("\nstock lsg_proj_vel(e)\n{\n\tnew Float:v[3]\n\tif (e > 0)\n\t\tls_proj_velocity(e, v)\n\treturn lsg_vfrom(v)\n}"); // T1fix
    }
    if gen.live.iter().any(|id| matches!(v.defs[*id].kind.as_str(), "npc.spawn" | "npc.spawn_use") && v.data_source(id, "pos").is_some()) {
        o.line("\nstock bool:lsg_npc_at(v, Float:o[3])\n{\n\tfor (new k = 0; k < 3; k++)\n\t\to[k] = lsg_vc(v, k)\n\treturn true\n}");
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
    // (the bigger node families' helpers only where they're used: a plugin's source stays short)
    if ["msg.status_icon", "fx.marker", "fx.marker_remove", "msg.killfeed"].iter().any(|k| has(k)) {
        o.line(UI_HELPERS);
    }
    if g.nodes.iter().any(|n| n.kind.starts_with("fun.")) {
        o.line(FUN_HELPERS);
    }
    o.line(&safe_cmd_stock());
    o.blank();
    if hit_players || hit_creatures {
        o.line(HIT_BLOCK);
    }
    // collision, overlap and hits (W2): only in a plugin that has one of those nodes
    if collide::used(g) {
        o.line(collide::PAWN);
        o.blank();
    }
    if uses_v3 {
        o.line(V3_HELPERS);
        o.blank();
        if w1::used(g) {
            o.line(w1::HELPERS); // W1
            o.blank();
        }
        v3_stocks(&gen, &mut o);
        // the data tables the mode reads, as constant arrays
        let used = |tid: &str| g.nodes.iter().any(|n| n.kind.starts_with("table.") && n.params.get("table").and_then(Value::as_str) == Some(tid));
        o.line(&super::tables::pawn(&g.data_tables, &used));
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
        // live tweaks: the debugger sets a variable while the game runs (numbers, yes/no, text,
        // enum values; the app checked the value)
        o.line("lsd_graph_set(const vid[], who, const val[])\n{");
        let mut any = false;
        for (i, x) in gen.vars.iter().enumerate() {
            let enum_ty = x.ty.starts_with("enum:");
            if !matches!(x.ty.as_str(), "number" | "bool" | "text") && !enum_ty {
                continue;
            }
            any = true;
            let at = match x.scope.as_str() {
                "player" => format!("g_pv{i}[clamp(who, 1, 32)]"),
                "team" => format!("g_tv{i}[clamp(who, 1, 2)]"),
                _ => format!("g_v{i}"),
            };
            let set = match x.ty.as_str() {
                "number" => format!("{at} = str_to_float(val)"),
                "bool" => format!("{at} = str_to_num(val) ? 1 : 0"),
                "text" => format!("copy({at}, 63, val)"),
                _ => format!("{at} = str_to_num(val)"),
            };
            o.line(format!("\tif (equal(vid, {}))\n\t{{\n\t\t{set}\n\t\treturn true\n\t}}", pawn_str(&id_text(&x.id))));
        }
        if !any {
            o.line("\t#pragma unused vid, who, val");
        } else if !gen.vars.iter().any(|x| x.scope != "mode" && (matches!(x.ty.as_str(), "number" | "bool" | "text") || x.ty.starts_with("enum:"))) {
            o.line("\t#pragma unused who");
        }
        o.line("\treturn false\n}");
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
    engine::blocks(g, &mut o);
    fx::blocks(g, &mut o);
    p5::helpers(g, &mut o); // P5 (after the saved values it builds on)
    coverage::blocks(g, &mut o); // EC
    if uses_clock {
        o.line(CLOCK);
        o.blank();
    }

    // registration
    let kinds: Vec<&EventKind> = EVENTS.iter().filter(|k| has(k.kind)).collect();
    let need = needs(checked);
    let uses_markers = has("marker.for_each") || ["marker.find", "marker.count", "marker.get"].iter().any(|k| pure_used(&gen, k));
    if uses_markers {
        o.line(markers_block(g));
    }
    // custom events (and the map's ones)
    custom_events_block(&gen, &mut o);
    let mut auto_tl: Vec<usize> = Vec::new();
    let map_events = g.custom_events.iter().any(|c| c.map);
    let uses_spawn = has("creature.spawn");
    // creature control: the helpers, and the once-a-second watch (Idle For, Creature Idle, Go To then resume)
    let cr_ctl = g.nodes.iter().any(|n| (n.kind.starts_with("creature.") && !["creature.spawn", "creature.health", "creature.count", "creature.is", "creature.on_killed"].contains(&n.kind.as_str())) || n.kind == "cast.creature");
    let cr_watch = has("creature.on_idle") || pure_used(&gen, "creature.idle_for") || g.nodes.iter().any(|n| n.kind == "creature.go_to");
    if cr_ctl {
        o.line(CREATURE_CTL);
    }
    if cr_watch {
        let mut t = String::from("// once a second: how long each creature has stood still; Go To's \"then resume\"; Creature Idle\npublic lsg_cr_tick()\n{\n\tnew Float:now = get_gametime(), e = -1, Float:o[3]\n\twhile ((e = find_ent_by_class(e, \"ls_creature\")) > 0)\n\t{\n\t\tif (e >= 2048 || !lsg_cr_match(e, \"\"))\n\t\t\tcontinue\n\t\tpev(e, pev_origin, o)\n\t\tif (g_lsgCrSeen[e] != g_lsgCrSerial[e] || get_distance_f(o, g_lsgCrPos[e]) > 32.0)\n\t\t{\n\t\t\tg_lsgCrPos[e] = o\n\t\t\tg_lsgCrStill[e] = now\n\t\t\tg_lsgCrSeen[e] = g_lsgCrSerial[e]\n\t\t}\n\t\tif (g_lsgCrGoalOn[e] && get_distance_f(o, g_lsgCrGoal[e]) < 96.0)\n\t\t{\n\t\t\tg_lsgCrGoalOn[e] = false\n\t\t\tlsg_cr_resume(e)\n\t\t}");
        let idle: Vec<&str> = gen.live.iter().copied().filter(|id| gen.v.defs[id].kind == "creature.on_idle").collect();
        for id in &idle {
            let nd = gen.v.nodes[id];
            let d = gen.v.defs[id];
            let secs = float_lit(gen.num(nd, d, "secs").clamp(2.0, 600.0));
            let unseen = if gen.boolean(nd, d, "unseen") { " && !lsg_cr_sees_any(e)" } else { "" };
            let k = gen.index(id);
            t.push_str(&format!("\n\t\tif (now - g_lsgCrStill[e] >= {secs} && now >= g_lsgCrNext{k}[e]{unseen})\n\t\t{{\n\t\t\tg_lsgCrNext{k}[e] = now + {secs}\n\t\t\tnew ctx[LSG_CTX]\n\t\t\tctx[0] = e\n\t\t\tctx[1] = _:(now - g_lsgCrStill[e])\n\t\t\t{}(ctx)\n\t\t}}", gen.fname(id)));
            o.line(format!("stock Float:g_lsgCrNext{k}[2048]"));
        }
        t.push_str("\n\t}\n}\n");
        o.line(t);
    }
    if uses_spawn {
        o.line(CREATURES_X);
        fx::creature_block(g, &mut o);
    }
    // the game and server nodes' own hooks
    let uses_rounds = has("event.round_ended") || pure_used(&gen, "game.round_number");
    let uses_score = pure_used(&gen, "game.team_score");
    let uses_freeze = pure_used(&gen, "game.freeze_time");
    let uses_mute = has("player.mute");
    let uses_gag = has("player.gag");
    let uses_exp = has("fx.explosion");
    let uses_beam = has("fx.beam") || has("fun.trail");
    let npc_models: BTreeSet<String> = g.nodes.iter().filter(|n| n.kind == "npc.spawn").map(|n| model_text(n.params.get("model").and_then(Value::as_str).unwrap_or("models/barney.mdl"))).collect();
    // T1fix: a projectile's model or sprite, loaded with the map through gm_components
    let proj_models: BTreeSet<String> = g.nodes.iter().filter(|n| n.kind == "proj.spawn").map(|n| proj_text(n.params.get("model").and_then(Value::as_str).unwrap_or(""))).collect();
    let mut props: BTreeSet<String> = g.nodes.iter().filter(|n| n.kind == "ent.spawn_prop" || n.kind == "npc.spawn_use").map(|n| model_text(n.params.get("model").and_then(Value::as_str).unwrap_or(""))).collect();
    // an Objective Marker's sprite: loaded with the map, only when a marker is used (and there)
    props.extend(g.nodes.iter().filter(|n| n.kind == "fx.marker").map(|n| sprite_text(n.params.get("sprite").and_then(Value::as_str).unwrap_or(""))));
    let uses_mk = has("fx.marker");
    // the fun nodes: one 0.1 s task for fire, a drunk view and trails; death, spawn and leaving hooks
    let fun_tick = has("fun.burn") || has("fun.drunk") || has("fun.trail") || has("fun.view_from");
    let fun_dead = has("fun.burn") || has("fun.bury") || has("fun.camera") || has("fun.drunk") || has("fun.view_from");
    let fun_view = has("fun.view_from");
    let fun_spawn = has("fun.camera") || has("fun.trail") || has("fun.bury");
    let fun_left = has("fun.burn") || has("fun.camera") || has("fun.drunk") || has("fun.bury") || has("fun.trail") || has("fun.view_from");
    let fun_round = has("fun.burn") || has("fun.bury") || has("fun.view_from");
    let bursts: BTreeSet<&str> = g.nodes.iter().filter(|n| n.kind == "fun.burst").map(|n| n.params.get("kind").and_then(Value::as_str).unwrap_or("sparks")).collect();
    let uses_fire = has("fun.burn");
    let uses_smoke = bursts.contains("smoke");
    let uses_blood = bursts.contains("blood");
    let mk_some = g.nodes.iter().any(|n| n.kind == "fx.marker" && n.params.get("who").and_then(Value::as_str).is_some_and(|w| w != "all"));
    let icon_death = g.nodes.iter().any(|n| n.kind == "msg.status_icon" && n.params.get("death").and_then(Value::as_bool).unwrap_or(true));
    let sounds: BTreeSet<String> = g.nodes.iter().filter(|n| n.kind == "fx.sound_at").map(|n| sound_text(n.params.get("sound").and_then(Value::as_str).unwrap_or(""))).collect();
    if uses_rounds {
        o.line("public lsg_count_round()\n\tg_lsgRounds++\n");
    }
    if uses_score {
        o.line("public lsg_team_score()\n{\n\tnew t[2]\n\tread_data(1, t, charsmax(t))\n\tg_lsgScore[t[0] == 'T' ? 1 : 2] = read_data(2)\n}\n");
    }
    if uses_freeze {
        o.line("public lsg_freeze_on()\n\tg_lsgFreeze = true\n\npublic lsg_freeze_off()\n\tg_lsgFreeze = false\n");
    }
    if uses_mute {
        o.line("stock lsg_mute(id, Float:secs)\n{\n\tif (!lsg_ok(id))\n\t\treturn\n\tset_speak(id, SPEAK_MUTED)\n\tremove_task(LSG_TASK - 300 - id)\n\tif (secs > 0.0)\n\t\tset_task(secs, \"lsg_unmute\", LSG_TASK - 300 - id)\n}\n\npublic lsg_unmute(task)\n{\n\tnew id = LSG_TASK - 300 - task\n\tif (lsg_ok(id))\n\t\tset_speak(id, SPEAK_NORMAL)\n}\n");
    }
    if pure_used(&gen, "ent.kind_of") {
        o.line("// Kind Of: the built-in Entity Kind enum's value (Player, Bot, Creature, NPC, Interactable,\n// Component, Prop, Map entity, None)\nstock lsg_kind_of(e)\n{\n\tif (e <= 0 || !pev_valid(e))\n\t\treturn 8\n\tif (e <= MaxClients)\n\t\treturn is_user_connected(e) ? (is_user_bot(e) ? 1 : 0) : 8\n\tif (lsg_is_creature(e))\n\t\treturn 2\n\tif (lsg_kind_ok(e, \"ls_npc\"))\n\t\treturn 3\n\tif (lsg_kind_ok(e, \"ls_interactable\"))\n\t\treturn 4\n\tif (lsg_kind_ok(e, \"\"))\n\t\treturn 5\n\tnew c[24]\n\tpev(e, pev_classname, c, charsmax(c))\n\treturn equal(c, \"ls_studio_prop\") ? 6 : 7\n}\n");
    }
    // Component Kind Of: gm_components' kind, as the Component Kind enum's value (its order)
    if pure_used(&gen, "ent.comp_kind") {
        let names: Vec<String> = COMP_KINDS.iter().map(|k| pawn_str(k)).collect();
        o.line(format!("stock const g_lsgCompKinds[][] = {{ {} }}\nstock lsg_comp_kind_of(e)\n{{\n\tnew k[24]\n\tif (e <= 0 || !ls_components_running() || !ls_comp_kind(e, k, charsmax(k)))\n\t\treturn {none}\n\tfor (new i = 0; i < sizeof g_lsgCompKinds; i++)\n\t\tif (equal(k, g_lsgCompKinds[i]))\n\t\t\treturn i\n\treturn {none}\n}}\n", names.join(", "), none = COMP_KINDS.len()));
    }
    if fun_tick {
        o.line("// fire, a drunk view and trails, ten times a second (one task for all of them)\npublic lsg_fun_tick()\n\tlsg_fun_ticks()\n");
    }
    if fun_view {
        o.line("// View From with Freeze: no shooting while viewing (moving is stopped by FL_FROZEN)\npublic lsg_view_cmd(id, uc, seed)\n{\n\tif (id < 1 || id > 32 || !g_lsgViewCam[id] || !g_lsgViewFreeze[id])\n\t\treturn FMRES_IGNORED\n\tset_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) & ~(IN_ATTACK | IN_ATTACK2 | IN_USE))\n\treturn FMRES_HANDLED\n}\n");
    }
    if fun_dead {
        o.line("// a death ends fire, burying, a drunk view, and third person when it's set to\npublic lsg_fun_dead(id)\n\tlsg_fun_died(id)\n");
    }
    if fun_spawn {
        o.line("// a respawn: the trail again, third person again (or not), out of the ground\npublic lsg_fun_spawn(id)\n\tlsg_fun_spawned(id)\n\npublic lsg_cam_again(task)\n\tlsg_cam_reapply(task - (LSG_TASK - 480))\n");
    }
    if fun_round {
        o.line("// a new round puts out fires, unburies and gives views back\npublic lsg_fun_round()\n{\n\tfor (new p = 1; p <= MaxClients; p++)\n\t{\n\t\tlsg_burn_off(p)\n\t\tg_lsgBuried[p] = false\n\t\tlsg_view_back(p)\n\t}\n}\n");
    }
    if uses_mk {
        o.line("// Objective Markers follow what they're on, ten times a second (one task for all of them)\npublic lsg_mk_tick()\n\tlsg_mk_follow()\n");
    }
    if mk_some {
        o.line("// a marker for one team or one player: the others' games aren't sent it\npublic lsg_mk_full(es, e, ent, host, flags, player, pset)\n{\n\tif (ent < 1 || ent >= 2048 || !g_lsgMkWho[ent])\n\t\treturn FMRES_IGNORED\n\tif (lsg_mk_sees(ent, host))\n\t\treturn FMRES_IGNORED\n\tforward_return(FMV_CELL, 0)\n\treturn FMRES_SUPERCEDE\n}\n");
    }
    if icon_death {
        o.line("// Status Icon (until death): it goes when they die\npublic lsg_icon_dead(id)\n\tlsg_icon_clear(id)\n");
    }
    if uses_gag {
        o.line("stock lsg_gag(id, Float:secs)\n{\n\tif (lsg_ok(id))\n\t\tg_lsgGagTill[id] = secs > 0.0 ? get_gametime() + secs : 99999999.0\n}\n\npublic lsg_gag_say(id)\n\treturn lsg_ok(id) && get_gametime() < g_lsgGagTill[id] ? PLUGIN_HANDLED : PLUGIN_CONTINUE\n");
    }
    // a shop the mode makes at run time with a keeper: the keeper's model is loaded with the map
    let keepers: Vec<&str> = g.nodes.iter().filter(|n| n.kind == "comp.create" && n.params.get("kind").and_then(Value::as_str) == Some("ls_shop"))
        .filter_map(|n| n.params.get("keys").and_then(Value::as_str))
        .filter_map(|k| k.split_whitespace().find_map(|w| w.strip_prefix("keeper=")))
        .filter_map(|k| match k { "barney" => Some("models/barney.mdl"), "gman" => Some("models/gman.mdl"), _ => None })
        .collect::<BTreeSet<_>>().into_iter().collect();
    if !need.creatures.is_empty() || uses_markers || map_events || !keepers.is_empty() || uses_exp || uses_beam || !props.is_empty() || !sounds.is_empty() || !npc_models.is_empty() || !proj_models.is_empty() || engine::needs_precache(g) || fx::needs_precache(g) || uses_fire || uses_smoke || uses_blood {
        let mut pre = String::from("public plugin_precache()\n{");
        for m in &proj_models {
            // (through the mode's precache budget: a model that doesn't fit isn't loaded, and flies as the grenade)
            pre.push_str(&format!("\n\t// a projectile the mode fires: loaded with the map\n\tif (lsg_pre_model({m}))\n\t\tls_proj_precache({m})", m = pawn_str(m)));
        }
        for m in &npc_models {
            pre.push_str(&format!("\n\t// an NPC the mode makes: its model is loaded with the map\n\tls_npc_precache({})", pawn_str(m)));
        }
        if uses_exp {
            pre.push_str("\n\tg_lsgSprExp = precache_model(\"sprites/zerogxplode.spr\")");
        }
        if uses_beam {
            pre.push_str("\n\tg_lsgSprBeam = precache_model(\"sprites/laserbeam.spr\")");
        }
        // (the fun nodes' sprites: only those a node uses)
        if uses_fire {
            pre.push_str("\n\tg_lsgSprFire = lsg_pre_model(\"sprites/fire.spr\")");
        }
        if uses_smoke {
            pre.push_str("\n\tg_lsgSprSmoke = lsg_pre_model(\"sprites/steam1.spr\")");
        }
        if uses_blood {
            pre.push_str("\n\tg_lsgSprBlood = lsg_pre_model(\"sprites/blood.spr\")\n\tg_lsgSprSpray = lsg_pre_model(\"sprites/bloodspray.spr\")");
        }
        // (only what's there: a missing file would stop the map loading)
        for m in &props {
            pre.push_str(&format!("\n\tlsg_pre_model({})", pawn_str(m)));
        }
        for snd in &sounds {
            pre.push_str(&format!("\n\tlsg_pre_sound({})", pawn_str(snd)));
        }
        // (the view models a Set Weapon View Model names)
        for l in engine::precache(g) {
            pre.push_str(&format!("\n{l}"));
        }
        for l in fx::precache(g) {
            pre.push_str(&format!("\n{l}"));
        }
        for m in &keepers {
            pre.push_str(&format!("\n\t// the shopkeeper of a shop the mode makes\n\tlsg_pre_model({})", pawn_str(m)));
        }
        if map_events {
            pre.push_str("\n\t// the map's custom event entities, as they load\n\tregister_forward(FM_KeyValue, \"lsg_ev_kv\")\n\tregister_forward(FM_Spawn, \"lsg_ev_spawn\", 1)");
        }
        if uses_markers {
            pre.push_str("\n\t// the map's ls_marker entities, as they load\n\tregister_forward(FM_KeyValue, \"lsg_mk_kv\")\n\tregister_forward(FM_Spawn, \"lsg_mk_spawn\", 1)");
        }
        if !need.creatures.is_empty() {
            pre.push_str(&format!("\n\t// creature types this mode spawns: loaded as the map starts (with what other plugins asked for)\n\tif (get_cvar_pointer(\"ls_creature_precache\"))\n\t\tlsg_precache_types({})", pawn_str(&need.creatures.join(" "))));
        }
        o.line(format!("{pre}\n}}\n"));
    }
    if !need.creatures.is_empty() {
        o.line("stock lsg_precache_types(const want[])\n{\n\tnew cur[256], w[32], rest[256], padded[280], pw[36]\n\tget_cvar_string(\"ls_creature_precache\", cur, charsmax(cur))\n\tif (equali(cur, \"all\"))\n\t\treturn\n\tif (equali(want, \"all\"))\n\t{\n\t\tset_cvar_string(\"ls_creature_precache\", \"all\")\n\t\treturn\n\t}\n\tcopy(rest, charsmax(rest), want)\n\twhile (rest[0])\n\t{\n\t\tstrtok(rest, w, charsmax(w), rest, charsmax(rest), ' ')\n\t\tformatex(padded, charsmax(padded), \" %s \", cur)\n\t\tformatex(pw, charsmax(pw), \" %s \", w)\n\t\tif (w[0] && containi(padded, pw) < 0)\n\t\t{\n\t\t\tif (cur[0])\n\t\t\t\tadd(cur, charsmax(cur), \" \")\n\t\t\tadd(cur, charsmax(cur), w)\n\t\t}\n\t}\n\tset_cvar_string(\"ls_creature_precache\", cur)\n}\n");
    }
    o.line("public plugin_init()\n{");
    o.line(format!("\tls_register_mode(LSG_MODE, {}, \"1.0\", \"Game Mode Studio\")", pawn_str(&safe_text(&g.mode.name, 40))));
    if debug {
        o.line("\tlsd_init()");
    }
    // (the mode's own bookkeeping first: AMX Mod X calls a plugin's hooks in the order they were
    // made, so New Round's nodes start their timers after the old round's are stopped, and Round
    // Started sees Is Freeze Time off and this round's number)
    if new_round {
        o.line("\tregister_event(\"HLTV\", \"lsg_new_round\", \"a\", \"1=0\", \"2=0\")");
    }
    if uses_rounds {
        o.line("\tregister_event(\"HLTV\", \"lsg_count_round\", \"a\", \"1=0\", \"2=0\")");
    }
    if uses_score {
        o.line("\tregister_event(\"TeamScore\", \"lsg_team_score\", \"a\")");
    }
    if uses_freeze {
        o.line("\tregister_event(\"HLTV\", \"lsg_freeze_on\", \"a\", \"1=0\", \"2=0\")\n\tregister_logevent(\"lsg_freeze_off\", 2, \"1=Round_Start\")");
    }
    for k in &kinds {
        for r in k.register {
            o.line(format!("\t{r}"));
        }
    }
    if map_events {
        o.line("\tRegisterHam(Ham_Use, \"info_target\", \"lsg_ev_use\", 1)");
    }
    if !settings.is_empty() {
        o.line("\tlsg_settings_init()");
    }
    if hit_players {
        o.line("\tRegisterHamPlayer(Ham_TraceAttack, \"lsg_hit_trace\", 0)");
    }
    if hit_creatures {
        o.line("\tif (get_cvar_pointer(\"ls_creature_ver\"))\n\t\tRegisterHam(Ham_TraceAttack, \"ls_creature\", \"lsg_hit_trace\", 0)");
    }
    if cr_watch {
        o.line("\tset_task(1.0, \"lsg_cr_tick\", LSG_TASK + 60000, _, _, \"b\")");
    }
    // (State, Target, Speed and Tier read and change what only creature control 2 has)
    if g.nodes.iter().any(|n| CREATURE_V2.contains(&n.kind.as_str())) {
        o.line("\tif (get_cvar_pointer(\"ls_creature_ver\") && get_cvar_num(\"ls_creature_ver\") < 2)\n\t\tlog_amx(\"This mode's Creature State, Target, Speed and Tier nodes need a newer Loner-Strike bot brain (creature control 2): update the server. Until then they do nothing.\")");
    }
    // Timelines with AutoPlay: from the start at each round's start (the one instance)
    let autoplay: Vec<usize> = gen
        .live
        .iter()
        .filter(|id| gen.v.defs[*id].kind == "timeline.play")
        .filter(|id| gen.v.nodes[*id].params.get("timeline").and_then(Value::as_str).and_then(|t| g.timelines.iter().find(|x| x.id == t)).is_some_and(|t| t.autoplay))
        .map(|id| gen.index(id))
        .collect();
    if !autoplay.is_empty() {
        o.line("\tregister_logevent(\"lsg_tl_auto\", 2, \"1=Round_Start\")");
        auto_tl = autoplay;
    }
    if uses_pre {
        o.line("\tRegisterHamPlayer(Ham_TakeDamage, \"lsg_pre_damage\", 0)");
    }
    if uses_spawn {
        o.line("\tif (get_cvar_pointer(\"ls_creature_ver\"))\n\t\tRegisterHam(Ham_Killed, \"ls_creature\", \"lsg_pay_killer\", 1)");
    }
    if uses_gag {
        o.line("\tregister_clcmd(\"say\", \"lsg_gag_say\")\n\tregister_clcmd(\"say_team\", \"lsg_gag_say\")");
    }
    if uses_mk {
        o.line("\tset_task(0.1, \"lsg_mk_tick\", LSG_TASK - 400, _, _, \"b\")");
    }
    if fun_tick {
        o.line("\tset_task(0.1, \"lsg_fun_tick\", LSG_TASK - 410, _, _, \"b\")");
    }
    if fun_view {
        o.line("\tregister_forward(FM_CmdStart, \"lsg_view_cmd\", 0)");
    }
    // collision, overlap and hits (W2)
    collide::init(&gen, &mut o);
    p5::init(&gen, &mut o); // P5
    if fun_dead {
        o.line("\tRegisterHamPlayer(Ham_Killed, \"lsg_fun_dead\", 1)");
    }
    if fun_spawn {
        o.line("\tRegisterHamPlayer(Ham_Spawn, \"lsg_fun_spawn\", 1)");
    }
    if fun_round {
        o.line("\tregister_event(\"HLTV\", \"lsg_fun_round\", \"a\", \"1=0\", \"2=0\")");
    }
    if mk_some {
        o.line("\tregister_forward(FM_AddToFullPack, \"lsg_mk_full\", 0)");
    }
    if icon_death {
        o.line("\tRegisterHamPlayer(Ham_Killed, \"lsg_icon_dead\", 1)");
    }
    if uses_down {
        o.line("\tset_task(0.25, \"lsg_revive_tick\", LSG_TASK - 1, _, _, \"b\")");
    }
    engine::init(g, &mut o);
    fx::init(g, &mut o);
    coverage::init(&gen, &mut o); // EC
    if uses_clock {
        o.line("\tset_task(1.0, \"lsg_clock_tick\", LSG_TASK - 2, _, _, \"b\")");
    }
    if uses_save {
        o.line("\tset_task(5.0, \"lsg_saved_flush\", LSG_TASK - 3, _, _, \"b\")");
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
    // W1: On Tick: a check every server frame (set_task can't go under 0.1 s)
    if gen.live.iter().any(|id| v.defs[*id].kind == "event.on_tick") {
        o.line("\tregister_forward(FM_StartFrame, \"lsg_w1_frame\")");
    }
    // chat commands: on all of say (a "say /shop" registration only matches the bare word, so
    // "/give 5" never came: node coverage run); the handler checks the first word
    for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.chat_command") {
        let cmd = name_text(gen.text(v.nodes[id], v.defs[id], "command"));
        let i = gen.index(id);
        if !cmd.is_empty() {
            for say in ["say", "say_team"] {
                o.line(format!("\tregister_clcmd(\"{say}\", \"lsg_chat{i}\")"));
            }
        }
    }
    o.line("}");
    o.blank();
    if !auto_tl.is_empty() {
        let mut f = String::from("// Timelines with AutoPlay\npublic lsg_tl_auto()\n{\n\tnew c[LSG_CTX]");
        for i in &auto_tl {
            f.push_str(&format!("\n\tlsg_tl{i}_save(0, c)\n\tlsg_tl{i}_go(0, 0.0, 1)"));
        }
        o.line(format!("{f}\n}}\n"));
    }
    if !settings.is_empty() {
        o.line("// the host's values for the mode settings, as the map starts\npublic plugin_cfg()\n{\n\tlsg_settings_load()\n\tlsg_settings_apply(true)\n}\n");
    }
    if new_round {
        o.line("// a new round: timers stop, round variables start again\npublic lsg_new_round()\n{\n\tlsg_stop_timers()");
        if !settings.is_empty() {
            // (a host's change since: the app writes the file, an admin uses the menu)
            o.line("\tlsg_settings_load()\n\tlsg_settings_apply(false)");
        }
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
    // W1: On Tick's handler: each one when its Interval has gone by; Delta is the time since
    // its last run (the interval the first time; a new map starts the clocks again)
    let ticks: Vec<&str> = gen.live.iter().copied().filter(|id| v.defs[*id].kind == "event.on_tick").collect();
    if !ticks.is_empty() {
        o.line(format!("public lsg_w1_frame()\n{{\n\tstatic Float:at[{}]\n\tnew Float:now = get_gametime()", ticks.len()));
        for (k, id) in ticks.iter().enumerate() {
            let s = w1::tick_secs(&gen, id);
            o.line(format!("\tif (now < at[{k}] || now - at[{k}] >= {s})\n\t{{\n\t\tnew ctx[LSG_CTX]\n\t\tctx[0] = _:(at[{k}] > 0.0 && now > at[{k}] ? now - at[{k}] : {s})\n\t\tat[{k}] = now\n\t\t{}(ctx)\n\t}}", gen.fname(id)));
        }
        o.line("\treturn FMRES_IGNORED\n}\n");
    }
    for id in gen.live.iter().filter(|id| v.defs[*id].kind == "event.chat_command") {
        let i = gen.index(id);
        let cmd = pawn_str(&name_text(gen.text(v.nodes[id], v.defs[id], "command")));
        o.line(format!("public lsg_chat{i}(id)\n{{\n\tnew ctx[LSG_CTX], s[64], w[40]\n\tread_args(s, charsmax(s))\n\tremove_quotes(s)\n\ttrim(s)\n\tcopyc(w, charsmax(w), s, ' ')\n\tif ((w[0] != '/' && w[0] != '!') || !equali(w[1], {cmd}))\n\t\treturn PLUGIN_CONTINUE\n\tctx[0] = id // player\n\tcopy(g_lsgTxt[0], 63, s) // args\n\tlsg_after_word(g_lsgTxt[0])\n\t{}(ctx)\n\treturn PLUGIN_CONTINUE\n}}\n", gen.fname(id)));
    }
    p5::handlers(&gen, &mut o); // P5
    if debug || uses_save || uses_clock || p5::needs_end(g) {
        o.line("public plugin_end()\n{");
        p5::end(g, &mut o); // P5
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
            o.line("\t// (saved and put back: damage done inside the flow comes back here)\n\tnew Float:was = g_lsgDmg, bool:wasBlock = g_lsgDmgBlock\n\tg_lsgDmg = damage\n\tg_lsgDmgBlock = false\n\tnew ctx[LSG_CTX]\n\tctx[0] = victim // victim\n\tctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker\n\tctx[2] = _:damage // damage\n\tlsg_dmg_kind(bits, g_lsgTxt[0], 63) // kind\n\tctx[3] = bits & DMG_BULLET && get_ent_data(victim, \"CBaseMonster\", \"m_LastHitGroup\") == HIT_HEAD ? 1 : 0 // headshot (W2: as Player Hurt)\n\tcopy(g_lsgTxt[1], 63, lsg_hitgroup(victim, bits)) // hitgroup");
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
        // (no Round Time Up in the graph: an empty hook; "new ctx" with only "#pragma unused ctx"
        // after it doesn't compile, the declaration isn't finished when the pragma is read)
        let calls: Vec<String> = gen.live.iter().filter(|id| v.defs[*id].kind == "event.time_up").map(|id| format!("\t{}(ctx)", gen.fname(id))).collect();
        if calls.is_empty() {
            o.line("lsg_time_up_hook()\n{\n}\n");
        } else {
            o.line(format!("lsg_time_up_hook()\n{{\n\tnew ctx[LSG_CTX]\n{}\n}}\n", calls.join("\n")));
        }
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
    if has("event.player_left") || uses_save || fun_left {
        o.line("public client_disconnected(id)\n{");
        if has("event.player_left") {
            o.line("\tlsg_on_player_left(id)");
        }
        // (leaving always ends the fun nodes' state: the camera, fire, a drunk view, a trail)
        if fun_left {
            o.line("\tlsg_fun_left(id)");
        }
        if uses_save {
            o.line("\tlsg_saved_flush()");
        } else if !has("event.player_left") && !fun_left {
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
        for (pin, read) in k.values.iter().filter(|(pin, _)| !matches!(*pin, "hit_pos" | "hit_dir") || (k.kind == "event.player_hurt" && hit_players) || (k.kind == "up.on_creature_hurt" && hit_creatures)) {
            match event_slot(def, pin) {
                Some((true, i)) => {
                    if *read == "@netname" {
                        o.line(format!("\tpev(ent, pev_netname, g_lsgTxt[{i}], 63) // {pin}"));
                    } else if let Some(param) = read.strip_prefix("@str:") {
                        o.line(format!("\tcopy(g_lsgTxt[{i}], 63, {param}) // {pin}"));
                    } else if let Some(call) = read.strip_prefix("@fn:") {
                        o.line(format!("\tcopy(g_lsgTxt[{i}], 63, {call}) // {pin}"));
                    }
                }
                Some((false, i)) => o.line(format!("\tctx[{i}] = {read} // {pin}")),
                None => {}
            }
        }
        for id in gen.live.iter().filter(|id| v.defs[*id].kind == k.kind) {
            let call = format!("{}(ctx)", gen.fname(id));
            match k.named {
                Some("@name") => {
                    // (a custom event: only the ones with this node's name)
                    let name = name_text(gen.text(v.nodes[id], v.defs[id], "name"));
                    o.line(format!("\tif (equal(name, {}))\n\t\t{call}", pawn_str(&name)));
                }
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
    // collision, overlap and hits (W2): their forwards and hooks
    if collide::used(g) {
        collide::handlers(&gen, &mut o);
    }
    coverage::handlers(&gen, &mut o); // EC

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
    // (in a Function's graph: compiled only when the Function is called; a node with several
    // flow inputs, as a Timeline's Play / Play from Start: its first input's function only when
    // something is wired into it, so no "never used" warning)
    let stock = if v.graph_of(id).is_empty() && !MULTI_IN.contains(&def.kind.as_str()) && !w1::MULTI.contains(&def.kind.as_str()) { "" } else { "stock " };
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
        // (the inputs as JSON only for a run that's written: hot nodes are sampled and nothing is
        // written while nobody watches, StudioPerfAudit #5)
        b.line(format!("\tif (lsd_want({i}))\n\t{{\n\tlsd_begin()"));
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
        b.line("\t}");
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
        // (collision, overlap and hits (W2): their handlers are collide.rs's)
        k if p5::is_event(k) => {
            let e = exit("ran");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        k if collide::is_event(k) => {
            let e = exit("ran");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        // ---- collision, overlap and hits (W2) ----
        "col.semiclip" => {
            let mode = match gen.choice(n, def, "mode") { "off" => 0, "team" => 2, "chosen" => 3, _ => 1 };
            let p = if mode == 3 && v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            b.line(format!("\tlsg_semiclip({mode}, {p}, {})", if gen.boolean(n, def, "on") { 1 } else { 0 }));
            let e = exit("set");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "col.set_solid" | "col.set_size" | "col.takes_damage" | "col.set_health" => {
            let e = inp("ent", "entity");
            let call = match def.kind.as_str() {
                "col.set_solid" => format!("lsg_set_solid({e}, {})", match gen.choice(n, def, "kind") { "trigger" => 1, "none" => 2, _ => 0 }),
                "col.set_size" => {
                    let f = |k: &str| float_lit(gen.num(n, def, k).clamp(-4096.0, 4096.0));
                    format!("lsg_set_size({e}, {}, {}, {}, {}, {}, {})", f("minx"), f("miny"), f("minz"), f("maxx"), f("maxy"), f("maxz"))
                }
                "col.takes_damage" => format!("lsg_takes_damage({e}, {})", if gen.boolean(n, def, "on") { 1 } else { 0 }),
                _ => format!("lsg_set_ent_health({e}, {})", inp("hp", "number")),
            };
            b.line(format!("\tg_lsgOut[{i}] = {call}"));
            if debug {
                let why = if def.kind == "col.set_size" { "Nothing done: no entity there, it's a player, or a min isn't below its max." } else { "Nothing done: no entity there, or it's a player (or the world)." };
                b.line(format!("\tif (!g_lsgOut[{i}])\n\t\t{}\n\telse\n\t{}", err(why), exit("done")));
            }
            then_line(&mut b, "then");
        }
        "vol.spawn" => {
            let at = if v.data_source(id, "at_ent").is_some() { inp("at_ent", "entity") } else { "0".into() };
            b.line("\tnew Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]");
            for (pin, arr) in [("pos", "lsgVp"), ("angles", "lsgVa"), ("offset", "lsgVo")] {
                if v.data_source(id, pin).is_some() {
                    b.line(format!("\tlsg_vto({}, {arr})", inp(pin, "vector")));
                }
            }
            let sphere = gen.choice(n, def, "shape") == "sphere";
            let f = |k: &str, hi: f64| float_lit(gen.num(n, def, k).clamp(8.0, hi));
            let size = if sphere { format!("{}, 0.0, 0.0", f("radius", 4096.0)) } else { format!("{}, {}, {}", f("sx", 8192.0), f("sy", 8192.0), f("sz", 8192.0)) };
            let filter = match gen.choice(n, def, "filter") { "creatures" => 1, "any" => 2, _ => 0 };
            let name = name_text(gen.text(n, def, "name"));
            b.line(format!("\tg_lsgOut[{i}] = lsg_vol_spawn({at}, lsgVp, lsgVa, lsgVo, {}, {size}, {filter}, {}, {}, {})", if sphere { 1 } else { 0 }, pawn_str(&name), gen.limit_expr("trigger_volumes"), gen.limit_expr("volume_members")));
            if debug {
                b.line(format!("\tif (!g_lsgOut[{i}])\n\t\t{}\n\telse\n\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))", err("No volume: as many are out as Trigger volumes at once allows (Limits), or the server is out of entity slots.")));
            }
            then_line(&mut b, "then");
        }
        "vol.remove" => {
            let call = format!("lsg_vol_remove({})", inp("volume", "entity"));
            if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("Nothing removed: that isn't a trigger volume."), exit("removed")));
            } else {
                b.line(format!("\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "bots.avoid" | "bots.avoid_stop" => {
            let tag = pawn_str(&name_text(gen.text(n, def, "tag")));
            let call = if def.kind == "bots.avoid_stop" {
                format!("lsg_avoid_gone({tag})\n\t\tls_bots_avoid_stop({tag})")
            } else {
                let num = |k: &str, lo: f64, hi: f64| float_lit(gen.num(n, def, k).clamp(lo, hi));
                let team = match gen.choice(n, def, "team") { "t" => 1, "ct" => 2, _ => 0 };
                let (r, st, secs) = (num("radius", 16.0, 4096.0), num("strength", 1.0, 10.0), num("secs", 0.0, 3600.0));
                if v.data_source(id, "at_ent").is_some() {
                    format!("ls_bots_avoid_ent({tag}, {}, {r}, {st}, {secs}, {team})", inp("at_ent", "entity"))
                } else {
                    b.line("\tnew Float:lsgAv[3]");
                    b.line(format!("\tlsg_vto({}, lsgAv)", inp("pos", "vector")));
                    format!("ls_bots_avoid({tag}, lsgAv, {r}, {st}, {secs}, {team})")
                }
            };
            let cond = if def.kind == "bots.avoid_stop" { comps.to_string() } else { format!("{comps} || !lsg_avoid_room({tag}, {})", gen.limit_expr("avoid_areas")) };
            check(&mut b, &cond, "Not done: gm_components isn't running on this server, or as many areas are avoided as Areas bots avoid at once allows (Limits).", &[format!("\t{call}"), exit("done")]);
            then_line(&mut b, "then");
        }
        // ---- end collision, overlap and hits ----
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
            // a weapon's name wired in (a data table's, say): only a real weapon is given
            let wired = v.data_source(id, "weapon").is_some();
            let w = if wired { inp("weapon", "text") } else { pawn_str(gen.choice(n, def, "weapon")) };
            let ammo = if v.data_source(id, "ammo").is_some() { format!("clamp(floatround({}), 0, 250)", inp("ammo", "number")) } else { (gen.num(n, def, "ammo").round() as i64).to_string() };
            let give = if wired { format!("lsg_give_named({p}, {w}, {ammo})") } else { format!("ls_give_weapon({p}, {w}, {ammo})") };
            if debug {
                b.line(format!("\tif ({give})\n\t{}\n\telse\n\t\t{}", exit("given"), err(if wired { "Not given: that player is dead, already has one, or that isn't a weapon's name." } else { "Not given: that player is dead or already has one." })));
            } else {
                b.line(format!("\t{give}"));
            }
            then_line(&mut b, "then");
        }
        "player.strip" | "player.set_health" | "player.set_armor" | "player.set_speed" | "player.teleport" => {
            let p = inp("player", "player");
            let (call, done) = match def.kind.as_str() {
                "player.strip" => (format!("ls_strip({p}, {})", if gen.boolean(n, def, "knife") { "true" } else { "false" }), "stripped"),
                "player.set_health" => (format!("ls_set_health({p}, max(floatround({}), 1))", inp("amount", "number")), "set"),
                "player.set_armor" => (format!("ls_set_armor({p}, max(floatround({}), 0), {})", inp("amount", "number"), if gen.choice(n, def, "type") == "vest" { "false" } else { "true" }), "set"),
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
                b.line(format!("\tlsd_exit({i}, lsd_times(count))"));
            } else {
                b.line("\t#pragma unused count");
            }
            then_line(&mut b, "then");
        }
        "creature.for_each" => {
            let k = gen.slot(id, "creature").unwrap_or(0);
            let body = next("each").map(|c| c.replace("(ctx)", "(c)"));
            let ty = pawn_str(&name_text(gen.text(n, def, "type")));
            b.line(format!("\tnew c[LSG_CTX], count, p = -1\n\twhile (count < 512 && (p = find_ent_by_class(p, \"ls_creature\")) > 0)\n\t{{\n\t\tif (!lsg_cr_match(p, {ty}))\n\t\t\tcontinue"));
            b.line(format!("\t\tcount++\n\t\tfor (new k = 0; k < LSG_CTX; k++)\n\t\t\tc[k] = ctx[k]\n\t\tc[{k}] = p"));
            match body {
                Some(c) => b.line(format!("\t\t{c}")),
                None => b.line("\t\t#pragma unused c"),
            }
            b.line("\t}");
            if debug {
                b.line(format!("\tlsd_exit({i}, lsd_times(count))"));
            } else {
                b.line("\t#pragma unused count");
            }
            then_line(&mut b, "then");
        }
        "creature.on_idle" => {
            let e = exit("ran");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "table.for_each" => {
            // each row: its number, its key and its cells in the flow's values (text ones kept aside)
            let body = next("each").map(|c| c.replace("(ctx)", "(c)"));
            let tb = super::tables::table_of(&v.g.data_tables, n);
            let t = tb.map(|x| x.0).unwrap_or(0);
            let rows = if tb.is_some() { format!("LSG_DT{t}_N") } else { "0".into() };
            b.line(format!("\tnew c[LSG_CTX], count\n\tfor (new r = 0; r < {rows}; r++)\n\t{{\n\t\tcount++\n\t\tfor (new j = 0; j < LSG_CTX; j++)\n\t\t\tc[j] = ctx[j]"));
            if let Some((t, tb)) = tb {
                let mut put = |pin: &str, k: usize, text: bool| {
                    if let Some(s) = gen.slot(id, pin) {
                        b.line(format!("\t\tc[{s}] = {}", if text { "0".into() } else { format!("_:lsg_dt{t}_{k}(float(r))") }));
                    } else if let Some(o) = gen.tout_index(id, pin) {
                        b.line(format!("\t\tcopy(g_lsgOT[{o}], 127, lsg_dt{t}_{k}(float(r)))"));
                    }
                };
                if let Some((k, kc)) = tb.key_col() {
                    put("key", k, kc.pin_type() == "text");
                }
                for (k, c) in tb.columns.iter().enumerate() {
                    put(&c.id, k, c.pin_type() == "text");
                }
                if let Some(s) = gen.slot(id, "row") {
                    b.line(format!("\t\tc[{s}] = _:float(r)"));
                }
            }
            match body {
                Some(c) => b.line(format!("\t\t{c}")),
                None => b.line("\t\t#pragma unused c"),
            }
            if let Some(stop) = &fn_done {
                b.line(format!("\t\tif ({stop})\n\t\t\treturn"));
            }
            b.line("\t}");
            if debug {
                b.line(format!("\tlsd_exit({i}, lsd_times(count))"));
            } else {
                b.line("\t#pragma unused count");
            }
            then_line(&mut b, "then");
        }
        "marker.for_each" => {
            let k = marker_index(v.g, gen.text(n, def, "marker"));
            let body = next("each").map(|c| c.replace("(ctx)", "(c)"));
            b.line(format!("\tnew c[LSG_CTX], count\n\tfor (new m = 0; m < g_lsgMkN; m++)\n\t{{\n\t\tif (g_lsgMkKind[m] != {k})\n\t\t\tcontinue\n\t\tcount++\n\t\tfor (new j = 0; j < LSG_CTX; j++)\n\t\t\tc[j] = ctx[j]"));
            if let Some(s) = gen.slot(id, "entity") {
                b.line(format!("\t\tc[{s}] = g_lsgMkEnt[m]"));
            }
            if let Some(s) = gen.slot(id, "index") {
                b.line(format!("\t\tc[{s}] = _:float(count)"));
            }
            match body {
                Some(c) => b.line(format!("\t\t{c}")),
                None => b.line("\t\t#pragma unused c"),
            }
            if let Some(stop) = &fn_done {
                b.line(format!("\t\tif ({stop})\n\t\t\treturn"));
            }
            b.line("\t}");
            if debug {
                b.line(format!("\tlsd_exit({i}, lsd_times(count))"));
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
        "func.define" | "fn.entry" | "event.custom" | "event.chat_command" | "event.player_about_hurt" | "event.player_revived" | "event.down_timeout" | "event.time_up" => {
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
        // NPCs and interactables (gm_components): text goes only to players (chat, menus)
        "npc.say" => {
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            let npc = if v.data_source(id, "npc").is_some() { inp("npc", "entity") } else { "0".into() };
            b.line("\tnew lsgL[192]");
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\tlsg_npc_line(lsgL, charsmax(lsgL), {}, {})\n\t\tls_npc_say({npc}, {who}, lsgL)", inp("speaker", "text"), inp("text", "text")), exit("said")]);
            then_line(&mut b, "then");
        }
        "npc.behave" => {
            let npc = inp("npc", "entity");
            let follow = if v.data_source(id, "follow").is_some() { inp("follow", "player") } else { "0".into() };
            let what = match gen.choice(n, def, "do") { "wander" => "wander", "patrol" => "patrol", "follow" => "follow", _ => "stand" };
            check(&mut b, &format!("{comps} || {npc} <= 0"), "Not done: no NPC wired in (or gm_components isn't running).", &[format!("\tls_npc_behave({npc}, {}, {follow})", pawn_str(what)), exit("done")]);
            then_line(&mut b, "then");
        }
        "npc.look_at" => {
            let npc = inp("npc", "entity");
            check(&mut b, &format!("{comps} || {npc} <= 0"), "Not done: no NPC wired in (or gm_components isn't running).", &[format!("\tls_npc_look_at({npc}, {})", inp("target", "entity")), exit("done")]);
            then_line(&mut b, "then");
        }
        "npc.spawn" | "npc.spawn_use" => {
            // where: a point wired in, else an entity wired in, else the one with that name
            // (the shared transform: At position, or At entity with a Local offset; else the named one)
            let at = fx::place(gen, id, "at_ent", &at_origin(gen, id, n, def));
            let model = pawn_str(&model_text(gen.text(n, def, "model")));
            let call = if def.kind == "npc.spawn" {
                let keys = format!("behavior={} unkillable={}", if gen.choice(n, def, "behavior") == "wander" { "wander" } else { "stand" }, if gen.boolean(n, def, "unkillable") { 1 } else { 0 });
                format!("ls_npc_spawn({model}, o, {}, {}, {})", float_lit(gen.num(n, def, "yaw")), pawn_str(&safe_text(gen.text(n, def, "name"), 31)), pawn_str(&keys))
            } else {
                let keys = format!("once={} hide={} bots={}", if gen.boolean(n, def, "once") { 1 } else { 0 }, if gen.boolean(n, def, "hide") { 1 } else { 0 }, if gen.boolean(n, def, "bots") { 1 } else { 0 });
                format!("ls_interactable_spawn({model}, o, {}, {})", pawn_str(&safe_text(gen.text(n, def, "prompt"), 60)), pawn_str(&keys))
            };
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!{at})\n\t\t{}\n\telse\n\t{{\n\t\tg_lsgOut[{i}] = {call}\n\t\tif (g_lsgOut[{i}])\n\t\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))\n\t\telse\n\t\t\t{}\n\t}}", err("gm_components isn't running on this server."), err("No entity has that name on this map."), err("Not made: its model isn't loaded with this map, or there's no room.")));
            } else {
                b.line(format!("\tif (!{comps} && {at})\n\t\tg_lsgOut[{i}] = {call}"));
            }
            // (an NPC's Facing setting is a yaw offset on top, so saved graphs face the same way)
            if let Some(l) = fx::apply(gen, n, def, id, &format!("g_lsgOut[{i}]"), "at_ent", true, if def.kind == "npc.spawn" { Some("yaw") } else { None }) {
                b.line(l);
            }
            then_line(&mut b, "then");
        }
        // T1fix: projectiles (gm_components ls_proj_*): graph text never reaches a command
        "proj.spawn" => {
            let wired = |k: &str| v.data_source(id, k).is_some();
            let owner = if wired("owner") { inp("owner", "player") } else { "0".into() };
            let model = pawn_str(&proj_text(gen.text(n, def, "model")));
            let flag = |k: &str| if gen.boolean(n, def, k) { 1 } else { 0 };
            // (a trail is a message: one more this frame only when the mode's message budget has room)
            let trail = if flag("trail") == 1 { "%d" } else { "0" };
            let keys = format!("trail={trail} spin={} bounce={} radius={} falloff={} explode={} dangerous={}", flag("spin"), gen.num(n, def, "bounce").clamp(0.0, 10.0).round(), gen.num(n, def, "radius").clamp(0.0, 2000.0).round(), flag("falloff"), flag("explode"), flag("dangerous"));
            b.line("\tnew Float:o[3], Float:d[3], lsgK[160], bool:lsgA = true");
            // where from and which way: At entity (or the owner's eyes) with its offsets, then
            // what's wired overrides (Position, Direction or Rotation)
            // (the spawn transform pins every spawn node takes, docs/notes/FX.md: Local offset along
            // At entity's own forward, right and up, 16 forward when unwired to clear the shooter;
            // Rotation offset turns the facing; Face the way At entity faces off: Rotation offset alone)
            let rot = if wired("angles_offset") { inp("angles_offset", "vector") } else { "-1".into() };
            let base = if wired("at_ent") { inp("at_ent", "entity") } else { owner.clone() };
            let (of, or_, ou) = if wired("offset") {
                let o = inp("offset", "vector");
                (format!("lsg_vc({o}, 0)"), format!("lsg_vc({o}, 1)"), format!("lsg_vc({o}, 2)"))
            } else {
                ("16.0".to_string(), "0.0".to_string(), "0.0".to_string())
            };
            let copy = if n.params.get("copy_rot").and_then(serde_json::Value::as_bool).unwrap_or(true) { 1 } else { 0 };
            b.line(format!("\tlsgA = lsg_proj_from({base}, {of}, {or_}, {ou}, {rot}, {copy}, o, d)"));
            if wired("pos") {
                b.line(format!("\tlsg_vto({}, o)\n\tlsgA = true", inp("pos", "vector")));
            }
            if wired("dir") {
                b.line(format!("\tlsg_vto({}, d)", inp("dir", "vector")));
            } else if wired("angles") {
                b.line(format!("\tnew Float:pa[3]\n\tlsg_vto({}, pa)\n\tangle_vector(pa, ANGLEVECTOR_FORWARD, d)", inp("angles", "vector")));
            }
            let trail_arg = if flag("trail") == 1 { format!("lsg_msg_room(\"Spawn Projectile\", 1, 0, {}) ? 1 : 0, ", gen.limit_expr("messages")) } else { String::new() };
            b.line(format!("\tformatex(lsgK, charsmax(lsgK), \"{keys} damage=%.0f scale=%.2f\", {trail_arg}floatclamp({}, 0.0, 5000.0), floatclamp({}, 0.05, 4.0))", inp("damage", "number"), inp("scale", "number")));
            // how many fly at once is the mode's Limits setting (gm_components' ls_proj_max); the engine's
            // own ceiling, free entity slots, is checked first (the larger reserve wins: the plugin keeps 128)
            b.line(format!("\tif (get_cvar_num(\"ls_proj_max\") != {lim})\n\t\tset_cvar_num(\"ls_proj_max\", {lim})", lim = gen.limit_expr("projectiles")));
            let call = format!("(lsg_ent_room(\"Spawn Projectile\") ? ls_proj_spawn({owner}, {model}, o, d, floatclamp({}, 1.0, 5000.0), {}, {}, lsgK) : 0)", inp("speed", "number"), float_lit(gen.num(n, def, "gravity").clamp(0.0, 4.0)), float_lit(gen.num(n, def, "life").clamp(0.1, 60.0)));
            // Velocity, when wired, sets the motion outright
            // (On Hit / On Expired come later, for this projectile only: its node and the flow's
            // context are kept by the projectile's entity, cleared when it's gone or reused)
            let note = format!("\n\t\tif (g_lsgOut[{i}])\n\t\t\tlsg_pj_note(g_lsgOut[{i}], {}, ctx)", i + 1);
            let vel = if wired("vel") { format!("\n\t\tif (g_lsgOut[{i}])\n\t\t{{\n\t\t\tnew Float:pv[3]\n\t\t\tlsg_vto({}, pv)\n\t\t\tls_proj_set_velocity(g_lsgOut[{i}], pv)\n\t\t}}", inp("vel", "vector")) } else { String::new() };
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!lsgA)\n\t\t{}\n\telse\n\t{{\n\t\tg_lsgOut[{i}] = {call}{vel}{note}\n\t\tif (g_lsgOut[{i}])\n\t\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))\n\t\telse\n\t\t\t{}\n\t}}", err("gm_components isn't running on this server."), err("Not fired: no Position, and no At entity or living owner to fire from."), err("Spawn Projectile skipped: the server is out of entity slots.")));
            } else {
                b.line(format!("\tg_lsgOut[{i}] = 0\n\tif (!{comps} && lsgA)\n\t{{\n\t\tg_lsgOut[{i}] = {call}{vel}{note}\n\t}}"));
            }
            // the callbacks (stocks after the handlers, like Say and Choose's replies)
            let mut set = String::new();
            for (pin, val) in [("hit_pos", "lsg_vfrom(pos)"), ("hit_normal", "lsg_vfrom(normal)"), ("hit_player", "(hit >= 1 && hit <= MaxClients) ? hit : 0"), ("hit_ent", "(hit > MaxClients) ? hit : 0"), ("hit_world", "hit == 0 ? 1 : 0"), ("hit_zone", "_:float(hitgroup)"), ("hit_owner", "owner")] {
                if let Some(k) = gen.out_index(id, pin) {
                    set.push_str(&format!("\n\tg_lsgO[{k}] = {val}"));
                }
            }
            let dbg_hit = if debug { format!("\n\tlsd_exit({i}, \"hit\")") } else { String::new() };
            let dbg_exp = if debug { format!("\n\tlsd_exit({i}, \"expired\")") } else { String::new() };
            let on_hit = next("on_hit").unwrap_or_else(|| "#pragma unused ctx".into());
            let on_exp = next("on_expire").unwrap_or_else(|| "#pragma unused ctx".into());
            let cb = format!("\n// {} ({}): its hits and its end\nstock lsg_pjhit{i}(ctx[], owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)\n{{\n\t#pragma unused owner\n\t#pragma unused pos\n\t#pragma unused normal\n\t#pragma unused hit\n\t#pragma unused hitgroup{set}{dbg_hit}\n\t{on_hit}\n}}\n\nstock lsg_pjexp{i}(ctx[])\n{{{dbg_exp}\n\t{on_exp}\n}}", comment_text(&def.title), id_text(id));
            menus_after.push((i, cb));
            then_line(&mut b, "then");
        }
        "proj.remove" => {
            let p = inp("proj", "entity");
            check(&mut b, &format!("{comps} || {p} <= 0"), "Not done: no projectile wired in (or gm_components isn't running).", &[format!("\tls_proj_remove({p})"), exit("done")]);
            then_line(&mut b, "then");
        }
        "proj.set_velocity" => {
            let p = inp("proj", "entity");
            check(&mut b, &format!("{comps} || {p} <= 0"), "Not done: no projectile wired in (or gm_components isn't running).", &[format!("\tnew Float:pv[3]\n\t\tlsg_vto({}, pv)\n\t\tls_proj_set_velocity({p}, pv)", inp("vel", "vector")), exit("done")]);
            then_line(&mut b, "then");
        }
        "npc.choose" => {
            let p = inp("player", "player");
            let npc = if v.data_source(id, "npc").is_some() { inp("npc", "entity") } else { "0".into() };
            // every text may come from a wire (a variable, a table cell): built here, at run time;
            // the replies are joined with | (one inside a reply becomes /), empty ones left out
            b.line("\tnew lsgL[192], lsgR[256], lsgN = 0");
            let mut body = vec![format!("\tnew lsgC = clamp(floatround({}), 1, 6)\n\t\tlsg_npc_line(lsgL, charsmax(lsgL), {}, {})", inp("count", "number"), inp("speaker", "text"), inp("text", "text"))];
            for k in 1..=6 {
                body.push(format!("\tlsg_npc_rep(lsgR, charsmax(lsgR), {p}, lsgN, lsgC, {k}, {})", inp(&format!("o{k}"), "text")));
            }
            body.push(format!("\tfor (new k = 0; k < LSG_CTX; k++)\n\t\t\tg_lsgM{i}[{p}][k] = ctx[k]"));
            // (an old question is answered -2 by the native before this one is noted)
            body.push(format!("\tif (lsgN && ls_npc_choose({npc}, {p}, lsgL, lsgR, floatclamp({}, 0.0, 300.0)))\n\t\t\tg_lsgAsk[{p}] = {}", inp("secs", "number"), i + 1));
            body.push(exit("asked"));
            // (a bot is asked too: gm_components picks one of the replies for it after a moment)
            check(&mut b, &format!("{comps} || !lsg_ok({p})"), "Not asked: nobody there, or gm_components isn't running.", &body);
            // the reply comes back later (ls_on_npc_choice): Picked is 1-6 (0 none)
            let picked = gen.out_index(id, "picked").map_or(String::new(), |k| format!("\n\tg_lsgO[{k}] = _:float(r)"));
            let mut cb = format!("\n// {} ({}): the reply\nstock lsg_ask{i}(id, choice)\n{{\n\tnew ctx[LSG_CTX]\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tctx[k] = g_lsgM{i}[id][k]\n\tnew r = choice >= 0 && choice < 6 ? g_lsgAskK[id][choice] : 0{picked}\n\tswitch (choice < 0 ? choice : r)\n\t{{\n", comment_text(&def.title), id_text(id));
            let mut arm = |label: &str, pin: &str, result: &str| {
                let call = next(pin).unwrap_or_else(|| "#pragma unused ctx".into());
                let dbg = if debug { format!("\n\t\t\tlsd_exit({i}, {})", pawn_str(result)) } else { String::new() };
                cb.push_str(&format!("\t\tcase {label}:\n\t\t{{{dbg}\n\t\t\t{call}\n\t\t}}\n"));
            };
            for k in 1..=6 {
                arm(&k.to_string(), &format!("r{k}"), &format!("reply {k}"));
            }
            arm("-1", "timeout", "timed out");
            arm("-2, 0", "away", "walked away");
            cb.push_str("\t}\n}");
            menus_after.push((i, cb));
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
                "bot.follow" => format!("lsg_bot_follow({bot}, {}, {}, clamp(floatround({}), 0, 100))", inp("who", "player"), gen.num(n, def, "secs").round() as i64, inp("influence", "number")),
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
            let at = fx::place(gen, id, "at_ent", &at_origin(gen, id, n, def));
            let keys = pawn_str(&keys_text(gen.text(n, def, "keys")));
            b.line("\tnew Float:o[3]");
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!{at})\n\t\t{}\n\telse\n\t{{\n\t\tg_lsgOut[{i}] = ls_comp_create({kind}, o, {keys})\n\t\tif (g_lsgOut[{i}])\n\t\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))\n\t\telse\n\t\t\t{}\n\t}}", err("gm_components isn't running on this server."), err("No entity has that name on this map."), err("gm_components couldn't make that component (it has no room, or the keys are wrong).")));
            } else {
                b.line(format!("\tif (!{comps} && {at})\n\t\tg_lsgOut[{i}] = ls_comp_create({kind}, o, {keys})"));
            }
            // (a component turns, but isn't pushed or sized: it's a zone)
            if let Some(l) = fx::apply(gen, n, def, id, &format!("g_lsgOut[{i}]"), "at_ent", false, None) {
                b.line(l);
            }
            then_line(&mut b, "then");
        }
        "comp.zone_set" | "comp.zone_shrink" => {
            // the safe zone: the one wired in, or the one a wired Shrinking zone drives
            let zin = inp("zone", "entity");
            let z = "zz";
            // the centre: an entity wired into Centre, else the one with that name, else (no name)
            // where the zone is now
            let at = match gen.v.data_source(id, "at_ent") {
                Some(_) => format!("lsg_ent_origin({}, o)", gen.need(id, "at_ent", "entity")),
                None if name_text(gen.text(n, def, "at")).is_empty() => format!("ls_zone_center({z}, o)"),
                None => format!("lsg_origin_of({}, o)", pawn_str(&name_text(gen.text(n, def, "at")))),
            };
            let r = inp("radius", "number");
            let call = if def.kind == "comp.zone_set" { format!("ls_zone_set({z}, o, {r})") } else { format!("ls_zone_shrink({z}, o, {r}, {})", float_lit(gen.num(n, def, "secs"))) };
            b.line(format!("\tnew Float:o[3], zz = {comps} ? 0 : lsg_zone_of({zin})"));
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse if (!zz)\n\t\t{}\n\telse if (!{at})\n\t\t{}\n\telse\n\t{{\n\t\t{call}\n\t{}\n\t}}", err("gm_components isn't running on this server."), err("That isn't a zone: wire a Safe zone or a Shrinking zone into Zone."), err("No entity has that name on this map."), exit("done")));
            } else {
                b.line(format!("\tif (zz && {at})\n\t\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "comp.load_pack" => {
            let pack = pawn_str(&name_text(gen.text(n, def, "pack")));
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\tnew map[32]\n\t\tget_mapname(map, charsmax(map))\n\t\tif (!ls_comp_load_pack(fmt(\"gamemodes/components/%s.%s.ini\", map, {pack})))"), if debug { format!("\t\tlsd_error({i}, \"This map has no such pack.\")\n\t\telse\n\t\t\tlsd_exit({i}, \"loaded\")") } else { "\t\t{}".into() }]);
            then_line(&mut b, "then");
        }
        "comp.crate_open" | "comp.wave_start" | "comp.wave_stop" | "comp.extract_open" | "comp.door" | "comp.trap_fire" | "comp.capture_lock" | "comp.reset" | "comp.start" | "comp.stop" | "comp.barricade_set_hp" | "comp.barricade_repair" | "comp.zone_skip"
        | "up.mod" | "up.perk_give" | "up.bench" | "up.box" | "kd.enable" | "kd.down" | "kd.revive" | "shop.menu" | "shop.open" | "br.heal_give" | "br.armor" | "br.glow" | "br.hud" | "br.hud_text" | "br.feed" | "br.drop_in" | "br.lobby" | "br.squad_size" | "comp.revive_pay"
        | "wave.table" | "wave.table_off" | "wave.scale" | "wave.cap" | "wave.tiers" | "turret.setup" | "turret.give" => {
            let p = || inp("player", "player");
            let call = match def.kind.as_str() {
                "up.mod" => format!("ls_player_mod({}, {}, {})", p(), pawn_str(gen.choice(n, def, "what")), inp("mult", "number")),
                "up.perk_give" => format!("ls_perk_give({}, {})", p(), pawn_str(gen.choice(n, def, "perk"))),
                "up.bench" => format!("ls_bench_use({})", p()),
                "up.box" => format!("ls_box_use({}, {})", if v.data_source(id, "box").is_some() { inp("box", "entity") } else { "0".into() }, p()),
                "kd.enable" => format!("ls_down_enable({}, {}, {}, {})", if gen.boolean(n, def, "on") { "true" } else { "false" }, float_lit(gen.num(n, def, "bleed")), gen.choice(n, def, "team").parse::<i64>().unwrap_or(0).clamp(0, 2), if gen.boolean(n, def, "perks") { "true" } else { "false" }),
                "kd.down" => format!("ls_down({})", p()),
                "kd.revive" => format!("ls_revive({}, {})", p(), if v.data_source(id, "by").is_some() { inp("by", "player") } else { "0".into() }),
                "shop.menu" => format!("ls_shop_menu({}, {})", inp("shop", "entity"), p()),
                "shop.open" => format!("ls_shop_open({}, bool:{})", inp("shop", "entity"), bool01(&inp("open", "bool"))),
                "br.heal_give" => format!("ls_heal_give({}, {}, max(floatround({}), 1))", p(), pawn_str(match gen.choice(n, def, "kind") { "medkit" => "medkit", "drink" => "drink", _ => "bandage" }), inp("n", "number")),
                "br.armor" => format!("ls_armor_set({}, {}, {})", p(), gen.choice(n, def, "vest").parse::<i64>().unwrap_or(-1).clamp(-1, 3), gen.choice(n, def, "helmet").parse::<i64>().unwrap_or(-1).clamp(-1, 3)),
                "br.glow" => format!("ls_glow({}, clamp(floatround({}), 0, 5))", inp("ent", "entity"), inp("tier", "number")),
                "br.hud" => format!("ls_match_hud({})", if gen.boolean(n, def, "on") { "true" } else { "false" }),
                "br.hud_text" => format!("ls_match_hud_text({})", inp("text", "text")),
                "br.feed" => format!("ls_feed({})", inp("text", "text")),
                "br.drop_in" => match v.data_source(id, "at") {
                    Some(_) => format!("lsg_drop_in({}, {})", p(), inp("at", "entity")),
                    None => format!("ls_drop_in({})", p()),
                },
                "br.lobby" => format!("ls_squad_lobby(floatmax({}, 0.0))", inp("secs", "number")),
                "wave.table" => format!("ls_wave_table({}, {}, {})", pawn_str(&name_text(gen.text(n, def, "section"))), if gen.choice(n, def, "team") == "1" { 1 } else { 2 }, if gen.boolean(n, def, "creatures") { "true" } else { "false" }),
                "wave.table_off" => "ls_wave_table_off()".into(),
                "wave.scale" => format!("ls_wave_scale(floatclamp({}, 0.1, 10.0), floatclamp({}, 0.1, 10.0))", inp("count", "number"), inp("hp", "number")),
                "wave.cap" => format!("ls_wave_cap(clamp(floatround({}), 0, 64))", inp("n", "number")),
                "wave.tiers" => format!("ls_wave_tiers({})", if gen.boolean(n, def, "on") { "true" } else { "false" }),
                "turret.setup" => format!("ls_turret_setup({}, {}, {}, {}, 0, {})", gen.num(n, def, "price").round().clamp(0.0, 100000.0) as i64, float_lit(gen.num(n, def, "life")), gen.num(n, def, "per").round().clamp(1.0, 8.0) as i64, pawn_str(&name_text(gen.text(n, def, "type"))), if gen.boolean(n, def, "bots") { "true" } else { "false" }),
                "turret.give" => format!("ls_turret_give({}, clamp(floatround({}), 1, 8))", p(), inp("n", "number")),
                "br.squad_size" => format!("ls_squad_max(clamp(floatround({}), 1, 8))", inp("size", "number")),
                "comp.revive_pay" => format!("ls_down_revive_pay(clamp(floatround({}), 0, 16000))", inp("amount", "number")),
                "comp.barricade_set_hp" => format!("ls_barricade_set_hp({}, max(floatround({}), 0))", inp("barricade", "entity"), inp("hp", "number")),
                "comp.barricade_repair" => format!("ls_barricade_repair({}, {})", inp("barricade", "entity"), if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() }),
                "comp.zone_skip" => format!("ls_zone_skip({})", inp("zone", "entity")),
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
            // (an entity pin wired with another kind of component: said, not a native error)
            let kind = match def.kind.as_str() {
                "comp.door" => Some(("door", "ls_timed_door", "a Timed door")),
                "comp.trap_fire" => Some(("trap", "ls_trigger_trap", "a Trap")),
                "comp.capture_lock" => Some(("point", "ls_capture_point", "a Capture point")),
                "comp.extract_open" => Some(("point", "ls_extraction_zone", "an Extraction zone")),
                "comp.barricade_set_hp" | "comp.barricade_repair" => Some(("barricade", "ls_barricade", "a Barricade")),
                "comp.zone_skip" => Some(("zone", "ls_zone_shrinker", "a Shrinking zone")),
                "shop.menu" | "shop.open" => Some(("shop", "ls_shop", "a Shop")),
                "up.box" if v.data_source(id, "box").is_some() => Some(("box", "ls_mystery_box", "a Mystery box")),
                "comp.start" | "comp.stop" => Some(("comp", "", "a component")),
                _ => None,
            };
            match kind {
                Some((pin, class, what)) => {
                    let e = inp(pin, "entity");
                    check(&mut b, &format!("{comps} || !lsg_kind_ok({e}, {})", pawn_str(class)), &format!("Not done: that isn't {what} (or gm_components isn't running)."), &[format!("\t{call}"), exit("done")]);
                }
                None => check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\t{call}"), exit("done")]),
            }
            then_line(&mut b, "then");
        }
        "up.perk_buy" => {
            let k = gen.out_index(id, "bought");
            let store = k.map_or(String::new(), |k| format!("g_lsgO[{k}] = "));
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\t{store}ls_perk_buy({}, {}) ? 1 : 0", inp("player", "player"), pawn_str(gen.choice(n, def, "perk"))), exit("done")]);
            then_line(&mut b, "then");
        }
        k if (k.starts_with("world.set_")) => {
            let (cvar, yes_no, _) = world_of(k).unwrap_or(("sv_gravity", false, true));
            let val = inp("v", if yes_no { "bool" } else { "number" });
            // (a fixed yes/no is written as 1 or 0: amxxpc warns on a constant test)
            let val = if yes_no && (val == "0" || val == "1") { val.clone() } else if yes_no { format!("({val}) ? 1 : 0") } else { val };
            b.line(if yes_no { format!("\tset_cvar_num({}, {val})", pawn_str(cvar)) } else { format!("\tset_cvar_float({}, {val})", pawn_str(cvar)) });
            let e = exit("done");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "game.change_map" | "game.restart_round" | "player.set_transform" | "player.set_rotation" | "player.look_at" | "player.slap" | "player.fade" | "player.shake" | "player.godmode" | "player.noclip" | "player.render"
        | "player.set_ammo" | "player.give_ammo" | "player.take_weapon" | "player.switch_weapon" | "player.set_frags" | "player.set_deaths" | "player.to_spectators" | "player.kick" | "player.mute" | "player.gag"
        | "msg.team_chat" | "msg.notify_admins" | "ent.remove" | "ent.set_position" | "ent.set_angles" | "ent.set_visible" | "fx.explosion" | "fx.beam" | "fx.sound_at" | "fx.sound_to" | "fx.lights" | "amx.fire"
        | "creature.go_to" | "creature.attack" | "creature.hold" | "creature.resume" | "creature.flee_to" | "creature.set_health" | "creature.remove" | "creature.set_speed" | "creature.set_tier" => {
            let p = || inp("player", "player");
            let num = |k: &str| float_lit(gen.num(n, def, k));
            let on = |k: &str| if gen.boolean(n, def, k) { "1" } else { "0" };
            let wired = |pin: &str| v.data_source(id, pin).is_some();
            let call = match def.kind.as_str() {
                "game.change_map" => format!("lsg_change_map({})", pawn_str(&name_text(gen.text(n, def, "map")))),
                "game.restart_round" => "server_cmd(\"sv_restartround 1\")".into(),
                "player.set_transform" => format!("lsg_set_transform({}, {}, {}, {})", p(), if wired("pos") { inp("pos", "vector") } else { "-1".into() }, if wired("angles") { inp("angles", "vector") } else { "-1".into() }, if wired("vel") { inp("vel", "vector") } else { "-1".into() }),
                "player.set_rotation" => format!("lsg_set_transform({}, -1, {}, -1)", p(), inp("angles", "vector")),
                "player.look_at" => format!("lsg_look_at({}, {}, {})", p(), if wired("at") { inp("at", "entity") } else { "0".into() }, if wired("pos") { inp("pos", "vector") } else { "-1".into() }),
                "player.slap" => format!("lsg_slap({}, floatround({}))", p(), inp("dmg", "number")),
                "player.fade" => {
                    let (r, g_, bl) = match gen.choice(n, def, "colour") { "black" => (0, 0, 0), "red" => (220, 20, 20), "green" => (20, 200, 40), _ => (255, 255, 255) };
                    format!("lsg_fade({}, {}, {r}, {g_}, {bl}, {})", p(), num("secs"), gen.num(n, def, "alpha").round().clamp(0.0, 255.0) as i64)
                }
                "player.shake" => format!("lsg_shake({}, {}, {})", p(), num("amp"), num("secs")),
                "player.godmode" => format!("lsg_godmode({}, {})", p(), on("on")),
                "player.noclip" => format!("lsg_noclip({}, {})", p(), on("on")),
                "player.render" => format!("lsg_render({}, {}, {})", p(), match gen.choice(n, def, "mode") { "normal" => 0, "see" => 1, _ => 2 }, gen.num(n, def, "amount").round().clamp(0.0, 255.0) as i64),
                "player.set_ammo" => format!("lsg_set_ammo({}, floatround({}), floatround({}))", p(), inp("clip", "number"), inp("reserve", "number")),
                "player.give_ammo" => format!("lsg_give_ammo({}, floatround({}))", p(), inp("n", "number")),
                "player.take_weapon" => format!("lsg_take_weapon({}, {})", p(), pawn_str(&format!("weapon_{}", gen.choice(n, def, "weapon")))),
                "player.switch_weapon" => format!("lsg_switch_weapon({}, {})", p(), pawn_str(&format!("weapon_{}", gen.choice(n, def, "weapon")))),
                "player.set_frags" => format!("lsg_set_frags({}, floatround({}))", p(), inp("n", "number")),
                "player.set_deaths" => format!("lsg_set_deaths({}, floatround({}))", p(), inp("n", "number")),
                "player.to_spectators" => format!("lsg_to_spec({})", p()),
                "player.kick" => format!("lsg_kick({}, {})", p(), pawn_str(&safe_text(gen.text(n, def, "reason"), 60))),
                "player.mute" => format!("lsg_mute({}, {})", p(), num("secs")),
                "player.gag" => format!("lsg_gag({}, {})", p(), num("secs")),
                "msg.team_chat" => {
                    let text = pawn_str(&safe_text(gen.text(n, def, "text"), 100));
                    let args = text_args(gen, &text, gen.input(id, "value"));
                    format!("lsg_team_chat({}, {args})", if gen.choice(n, def, "team") == "t" { 1 } else { 2 })
                }
                "msg.notify_admins" => {
                    let text = pawn_str(&safe_text(gen.text(n, def, "text"), 100));
                    let args = text_args(gen, &text, gen.input(id, "value"));
                    format!("lsg_admins_chat({args})")
                }
                "ent.remove" => format!("lsg_remove({})", inp("in", "entity")),
                "ent.set_position" => format!("lsg_ent_pos({}, {})", inp("in", "entity"), inp("pos", "vector")),
                "ent.set_angles" => format!("lsg_ent_angles({}, {})", inp("in", "entity"), inp("angles", "vector")),
                "ent.set_visible" => format!("lsg_ent_visible({}, {})", inp("in", "entity"), on("on")),
                "fx.explosion" => format!("lsg_explode({}, {}, {})", inp("pos", "vector"), inp("dmg", "number"), num("radius")),
                "fx.beam" => {
                    let (r, g_, bl) = match gen.choice(n, def, "colour") { "green" => (40, 220, 60), "blue" => (60, 120, 255), "white" => (255, 255, 255), "gold" => (255, 200, 40), _ => (230, 30, 30) };
                    format!("lsg_beam({}, {}, {r}, {g_}, {bl}, {})", inp("a", "vector"), inp("b", "vector"), num("secs"))
                }
                "creature.go_to" | "creature.flee_to" => {
                    let to = if wired("to") { inp("to", "entity") } else { "0".into() };
                    let pos = if wired("pos") { inp("pos", "vector") } else { "-1".into() };
                    let how = if def.kind == "creature.flee_to" { 2 } else if gen.choice(n, def, "then") == "resume" { 1 } else { 0 };
                    format!("lsg_cr_goto({}, {to}, {pos}, {}, {how})", inp("creature", "entity"), pawn_str(&ent_name_text(gen.text(n, def, "name"))))
                }
                "creature.attack" => format!("lsg_cr_attack({}, {})", inp("creature", "entity"), inp("target", "entity")),
                "creature.hold" => format!("lsg_cr_hold({})", inp("creature", "entity")),
                "creature.resume" => format!("lsg_cr_resume({})", inp("creature", "entity")),
                "creature.set_health" => format!("lsg_cr_health({}, {})", inp("creature", "entity"), inp("hp", "number")),
                "creature.set_speed" => format!("lsg_cr_speed({}, {})", inp("creature", "entity"), inp("mult", "number")),
                "creature.set_tier" => format!("lsg_cr_tier({}, floatround({}))", inp("creature", "entity"), inp("tier", "number")),
                "creature.remove" => format!("lsg_cr_cmd({}, {})", inp("creature", "entity"), pawn_str(if gen.choice(n, def, "how") == "remove" { "remove %d" } else { "kill %d" })),
                "fx.sound_at" => format!("lsg_sound_at({}, {})", inp("pos", "vector"), pawn_str(&sound_text(gen.text(n, def, "sound")))),
                "fx.sound_to" => format!("lsg_sound_to({}, {})", p(), pawn_str(&sound_text(gen.text(n, def, "sound")))),
                "fx.lights" => format!("set_lights({})", pawn_str(match gen.choice(n, def, "style") { "dark" => "e", "darkest" => "b", "bright" => "z", "flicker" => "mmnmmommommnonmmonqnmmo", _ => "#OFF" })),
                _ => format!("lsg_fire_event({}, floatround({}), floatround({}))", pawn_str(&name_text(gen.text(n, def, "name"))), inp("a", "number"), inp("b", "number")),
            };
            b.line(format!("\t{call}"));
            let e = exit("done");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "game.run_command" | "game.amx_command" => {
            let amx = def.kind == "game.amx_command";
            let k = gen.out_index(id, "ok");
            let store = k.map_or(String::new(), |k| format!("g_lsgO[{k}] = "));
            b.line(format!("\t{store}lsg_safe_cmd({}, {}) ? 1 : 0", inp("cmd", "text"), if amx { 1 } else { 0 }));
            if debug {
                b.line(format!("\tif (!lsg_cmd_ok)\n\t\t{}\n\telse\n\t{}", err("Refused: a mode may only send game rules (mp_), gameplay sv_ settings, bot settings and commands, our plugins' gameplay commands, say and humans_join_team; AMX commands: slap, slay, kick, the say commands, amx_cvar on those settings, amx_map and amx_nextmap."), exit("sent")));
            }
            then_line(&mut b, "then");
        }
        "fun.burn" | "fun.extinguish" | "fun.bury" | "fun.unbury" | "fun.drunk" | "fun.camera" => {
            let p = inp("player", "player");
            let (call, ok, refusal): (String, &str, &str) = match def.kind.as_str() {
                "fun.burn" => {
                    let by = if v.data_source(id, "by").is_some() { inp("by", "player") } else { "0".into() };
                    (format!("lsg_burn({p}, {by}, {}, {})", float_lit(gen.num(n, def, "secs").clamp(1.0, 60.0)), float_lit(gen.num(n, def, "dps").clamp(0.0, 100.0))), "burning", "Not lit: that player isn't alive.")
                }
                "fun.extinguish" => (format!("lsg_burn_off({p})"), "put out", ""),
                "fun.bury" => (format!("lsg_bury({p})"), "buried", "Not buried: that player isn't alive, or is buried already."),
                "fun.unbury" => (format!("lsg_unbury({p})"), "out", "Nothing done: that player isn't buried."),
                "fun.drunk" => (format!("lsg_drunk({p}, {}, {})", float_lit(gen.num(n, def, "secs").clamp(1.0, 60.0)), float_lit(gen.num(n, def, "strength").clamp(1.0, 10.0))), "drunk", "Nothing done: that player isn't alive."),
                _ => {
                    let view = match gen.choice(n, def, "view") { "first" => 0, "toggle" => 2, _ => 1 };
                    (format!("lsg_camera({p}, {view}, {})", if gen.boolean(n, def, "reset") { 1 } else { 0 }), "set", "Nothing done: nobody there.")
                }
            };
            if refusal.is_empty() {
                b.line(format!("\t{call}"));
                let e = exit(ok);
                if !e.is_empty() {
                    b.line(e);
                }
            } else if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err(refusal), exit(ok)));
            } else {
                b.line(format!("\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "cast.player" | "cast.creature" => {
            let e = inp("in", "entity");
            let k = gen.out_index(id, "out");
            let player = def.kind == "cast.player";
            let test = if player { "lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE)" } else { "lsg_is_creature(lsgE)" };
            let mut yes = vec![];
            if let Some(k) = k {
                yes.push(format!("\t\tg_lsgO[{k}] = lsgE"));
            }
            if !player {
                if let Some(t) = gen.tout_index(id, "type") {
                    yes.push(format!("\t\tcopy(g_lsgOT[{t}], 127, lsg_cr_type(lsgE))"));
                }
            }
            let ex = exit("success");
            if !ex.is_empty() {
                yes.push(format!("\t{ex}"));
            }
            if let Some(c) = next("ok") {
                yes.push(format!("\t\t{c}"));
            }
            let mut no = vec![];
            if let Some(k) = k {
                no.push(format!("\t\tg_lsgO[{k}] = 0"));
            }
            let ex = exit("failed");
            if !ex.is_empty() {
                no.push(format!("\t{ex}"));
            }
            if let Some(c) = next("fail") {
                no.push(format!("\t\t{c}"));
            }
            b.line(format!("\tnew lsgE = {e}\n\tif ({test})\n\t{{\n{}\n\t}}\n\telse\n\t{{\n{}\n\t}}", if yes.is_empty() { "\t\t#pragma unused lsgE".into() } else { yes.join("\n") }, if no.is_empty() { "".into() } else { no.join("\n") }));
        }
        "fun.view_from" => {
            let who = match gen.choice(n, def, "who") { "t" => 1, "ct" => 2, "all" => 3, _ => 0 };
            let p = if who == 0 && v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            let from = if v.data_source(id, "from").is_some() { inp("from", "entity") } else { "0".into() };
            let vec_in = |pin: &str, var: &str| if v.data_source(id, pin).is_some() { format!("\tlsg_vto({}, {var})\n", inp(pin, "vector")) } else { String::new() };
            b.line(format!(
                "\tnew Float:lsgA[3], Float:lsgAa[3], Float:lsgB[3], Float:lsgBa[3]\n{}{}{}{}\tlsg_view_from({who}, {p}, {from}, lsgA, lsgAa, {}, lsgB, lsgBa, {}, {}, {}, {}, {})",
                vec_in("pos", "lsgA"),
                vec_in("angles", "lsgAa"),
                vec_in("to", "lsgB"),
                vec_in("to_angles", "lsgBa"),
                if v.data_source(id, "pos").is_some() { 1 } else { 0 },
                if v.data_source(id, "to").is_some() { 1 } else { 0 },
                float_lit(gen.num(n, def, "offset").clamp(-256.0, 512.0)),
                float_lit(gen.num(n, def, "move").clamp(0.1, 60.0)),
                float_lit(gen.num(n, def, "secs").clamp(0.0, 120.0)),
                if gen.boolean(n, def, "freeze") { 1 } else { 0 }
            ));
            let e = exit("viewing");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "fun.view_return" => {
            let p = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            b.line(format!("\tlsg_view_return({p})"));
            let e = exit("returned");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "fun.trail" => {
            b.line(format!("\tg_lsgMsgLim = {}", gen.limit_expr("messages")));
            let c = |k: &str| gen.num(n, def, k).round().clamp(0.0, 255.0) as i64;
            let call = format!(
                "lsg_trail({}, {}, {}, {}, {}, {}, {}, {})",
                inp("ent", "entity"),
                if gen.boolean(n, def, "on") { 1 } else { 0 },
                c("r"),
                c("g"),
                c("b"),
                gen.num(n, def, "width").round().clamp(1.0, 50.0) as i64,
                (gen.num(n, def, "life") * 10.0).round().clamp(1.0, 250.0) as i64,
                gen.limit_expr("trails")
            );
            if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("No trail: nothing there, or as many trails are on as Trails at once allows (Limits)."), exit("done")));
            } else {
                b.line(format!("\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "fun.burst" => {
            let at = v.data_source(id, "at").is_some();
            let pos = v.data_source(id, "pos").is_some();
            b.line("\tnew Float:lsgAt[3], lsgOk");
            if at {
                b.line(format!("\tlsgOk = lsg_ent_origin({}, lsgAt)", inp("at", "entity")));
            } else if pos {
                b.line(format!("\tlsg_vto({}, lsgAt)\n\tlsgOk = 1", inp("pos", "vector")));
            }
            let kind = ["sparks", "smoke", "blood", "firework", "implosion", "tar", "teleport", "lava", "flash"].iter().position(|k| *k == gen.choice(n, def, "kind")).unwrap_or(0);
            let c = |k: &str| gen.num(n, def, k).round().clamp(0.0, 255.0) as i64;
            let count = gen.num(n, def, "count").round().clamp(1.0, 10.0) as i64;
            // (a firework is two messages a burst)
            let cost = if kind == 3 { count * 2 } else { count };
            b.line(format!("\tif (lsgOk && !lsg_msg_room(\"Effect Burst\", {cost}, 0, {}))\n\t\tlsgOk = 0", gen.limit_expr("messages")));
            b.line(format!("\tif (lsgOk)\n\t\tlsg_burst({kind}, lsgAt, {count}, {}, {}, {})", c("r"), c("g"), c("b")));
            if debug {
                b.line(format!("\tif (!lsgOk)\n\t\t{}\n\telse\n\t{}", err("No burst: no entity there and no Position, or too many effects this frame (Effect messages a frame, Limits)."), exit("done")));
            }
            then_line(&mut b, "then");
        }
        "fun.swap" => {
            let k = gen.out_index(id, "ok");
            let call = format!("lsg_swap({}, {})", inp("a", "player"), inp("b", "player"));
            b.line(match k {
                Some(k) => format!("\tg_lsgO[{k}] = {call}"),
                None => format!("\t{call}"),
            });
            if debug {
                let got = k.map_or(call.clone(), |k| format!("g_lsgO[{k}]"));
                b.line(format!("\tif (!{got})\n\t\t{}\n\telse\n\t{}", err("Not swapped: one isn't alive, it's the same player, or one would be stuck in a wall."), exit("swapped")));
            }
            then_line(&mut b, "then");
        }
        "msg.status_icon" => {
            b.line(format!("\tg_lsgMsgLim = {}", gen.limit_expr("messages")));
            let who = if v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            let icon = gen.choice(n, def, "icon");
            // (one of the game's own hud.txt names, from the list: never a typed name)
            let icon = if STATUS_ICONS.contains(&icon) { icon } else { "buyzone" };
            let show = match gen.choice(n, def, "show") { "off" => 0, "flash" => 2, _ => 1 };
            let c = |k: &str| gen.num(n, def, k).round().clamp(0.0, 255.0) as i64;
            b.line(format!("\tlsg_icon({who}, {show}, {}, {}, {}, {}, {})", pawn_str(icon), c("r"), c("g"), c("b"), if gen.boolean(n, def, "death") { 1 } else { 0 }));
            let e = exit(if show == 0 { "hidden" } else { "shown" });
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "fx.marker" => {
            let follow = if v.data_source(id, "follow").is_some() { inp("follow", "entity") } else { "0".into() };
            let pos = v.data_source(id, "pos").is_some();
            if pos {
                b.line(format!("\tnew Float:lsgAt[3]\n\tlsg_vto({}, lsgAt)", inp("pos", "vector")));
            } else {
                b.line("\tnew Float:lsgAt[3]");
            }
            let who = match gen.choice(n, def, "who") { "t" => 1, "ct" => 2, "player" => 3, _ => 0 };
            let forp = if who == 3 && v.data_source(id, "player").is_some() { inp("player", "player") } else { "0".into() };
            let c = |k: &str| gen.num(n, def, k).round().clamp(0.0, 255.0) as i64;
            b.line(format!(
                "\tg_lsgOut[{i}] = lsg_marker({}, {follow}, lsgAt, {}, {}, {}, {}, {}, {}, {}, {who}, {forp}, {})",
                pawn_str(&sprite_text(gen.text(n, def, "sprite"))),
                if pos { 1 } else { 0 },
                float_lit(gen.num(n, def, "height").clamp(0.0, 512.0)),
                float_lit(gen.num(n, def, "scale").clamp(0.05, 5.0)),
                c("r"),
                c("g"),
                c("b"),
                if gen.boolean(n, def, "pulse") { 1 } else { 0 },
                gen.limit_expr("markers")
            ));
            if debug {
                b.line(format!("\tif (!g_lsgOut[{i}])\n\t\t{}\n\telse\n\t{}", err("No marker: its sprite isn't loaded, nothing to follow or no position, as many are up as Objective markers at once allows (Limits), or the server is out of entity slots."), exit("made")));
            }
            then_line(&mut b, "then");
        }
        "fx.marker_remove" => {
            let m = if v.data_source(id, "marker").is_some() { inp("marker", "entity") } else { "0".into() };
            b.line(format!("\tlsg_marker_remove({m})"));
            let e = exit("removed");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "msg.killfeed" => {
            b.line(format!("\tg_lsgMsgLim = {}", gen.limit_expr("messages")));
            let killer = if v.data_source(id, "killer").is_some() { inp("killer", "player") } else { "0".into() };
            let w = gen.choice(n, def, "weapon");
            let w = if KILL_ICONS.contains(&w) { w } else { "skull" };
            let call = format!("lsg_killfeed({killer}, {}, {}, {})", inp("victim", "player"), bool01(&inp("hs", "bool")), pawn_str(w));
            if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("Not sent: the Victim isn't a player in the game."), exit("sent")));
            } else {
                b.line(format!("\t{call}"));
            }
            then_line(&mut b, "then");
        }
        "ent.spawn_prop" => {
            if fx::place_wired(gen, id, "at") || v.data_source(id, "at").is_none() || fx::copies(n, def) {
                // (the shared transform: At position, or At entity with a Local offset)
                let fallback = if v.data_source(id, "at").is_some() { format!("lsg_ent_origin({}, o)", inp("at", "entity")) } else { "false".into() };
                b.line(format!("\tnew Float:o[3]\n\tg_lsgOut[{i}] = {} ? lsg_prop_o(o, {}, {}) : 0", fx::place(gen, id, "at", &fallback), pawn_str(&model_text(gen.text(n, def, "model"))), if gen.boolean(n, def, "solid") { 1 } else { 0 }));
            } else {
                b.line(format!("\tg_lsgOut[{i}] = lsg_prop({}, {}, {})", inp("at", "entity"), pawn_str(&model_text(gen.text(n, def, "model"))), if gen.boolean(n, def, "solid") { 1 } else { 0 }));
            }
            if let Some(l) = fx::apply(gen, n, def, id, &format!("g_lsgOut[{i}]"), "at", true, None) {
                b.line(l);
            }
            if debug {
                b.line(format!("\tif (!g_lsgOut[{i}])\n\t\t{}\n\telse\n\t{}", err("No prop: the model isn't loaded, or there's no such entity."), exit("spawned")));
            }
            then_line(&mut b, "then");
        }
        "amx.call" => {
            let k = gen.out_index(id, "out");
            let ok = gen.out_index(id, "ok");
            let nargs = gen.choice(n, def, "args").parse::<i64>().unwrap_or(0).clamp(0, 2);
            b.line(format!("\tnew r, called = lsg_call({}, {}, {nargs}, floatround({}), floatround({}), r)", pawn_str(&name_text(gen.text(n, def, "plugin"))), pawn_str(&name_text(gen.text(n, def, "func"))), inp("a", "number"), inp("b", "number")));
            if let Some(k) = k {
                b.line(format!("\tg_lsgO[{k}] = _:float(r)"));
            }
            if let Some(ok) = ok {
                b.line(format!("\tg_lsgO[{ok}] = called"));
            }
            if debug {
                b.line(format!("\tif (!called)\n\t\t{}\n\telse\n\t{}", err("Not called: no such plugin running, or no such public function in it."), exit("called")));
            } else if k.is_none() && ok.is_none() {
                b.line("\t#pragma unused r\n\t#pragma unused called");
            }
            then_line(&mut b, "then");
        }
        "turret.place" => {
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\tg_lsgOut[{i}] = ls_turret_place({}, {})", inp("player", "player"), if gen.boolean(n, def, "pay") { "true" } else { "false" }), exit("done")]);
            then_line(&mut b, "then");
        }
        "up.pickup" | "br.crate" => {
            let at = inp("at", "entity");
            let call = if def.kind == "up.pickup" {
                let (r, g_, bl) = match gen.choice(n, def, "colour") { "white" => (255, 255, 255), "green" => (80, 220, 80), "blue" => (80, 140, 255), "purple" => (190, 90, 255), "red" => (255, 70, 60), _ => (255, 200, 40) };
                format!("ls_pickup_spawn({}, o, {}, \"\", {r}, {g_}, {bl})", pawn_str(&name_text(gen.text(n, def, "kind"))), float_lit(gen.num(n, def, "life")))
            } else {
                format!("ls_crate_make(o, {})", pawn_str(&keys_text(gen.text(n, def, "items"))))
            };
            b.line("\tnew Float:o[3]");
            check(&mut b, &format!("{comps} || !lsg_ent_origin({at}, o)"), "gm_components isn't running, or there's no such entity.", &[format!("\tg_lsgOut[{i}] = {call}"), exit("done")]);
            then_line(&mut b, "then");
        }
        "comp.loot_roll" => {
            let table = pawn_str(&name_text(gen.text(n, def, "table")));
            let t = gen.tout_index(id, "item");
            let k = gen.out_index(id, "rolled");
            let item = t.map_or("x".to_string(), |t| format!("g_lsgOT[{t}]"));
            b.line(format!("\tnew x[64], ok\n\tif (!{comps})\n\t\tok = ls_loot_roll({table}, x, charsmax(x), max(floatround({}), 0))", inp("tier", "number")));
            if let Some(t) = t {
                b.line(format!("\tcopy(g_lsgOT[{t}], 127, ok ? x : \"\")"));
            }
            if let Some(k) = k {
                b.line(format!("\tg_lsgO[{k}] = ok ? 1 : 0"));
            }
            if debug {
                b.line(format!("\tif ({comps})\n\t\t{}\n\telse\n\t\tlsd_exit({i}, ok ? {item} : \"nothing\")", err("gm_components isn't running on this server.")));
            } else if t.is_none() {
                b.line("\t#pragma unused x");
            }
            then_line(&mut b, "then");
        }
        // an interactable offered to bots: gm_components keeps the offer (as the map key bots=1)
        "bots.offer" if gen.choice(n, def, "kind") == "use" => {
            check(&mut b, &format!("{comps} || !lsg_kind_ok({}, \"ls_interactable\")", inp("at", "entity")), "Not done: that isn't an interactable (or gm_components isn't running).", &[format!("\tls_interactable_offer({}, true)", inp("at", "entity")), exit("offered")]);
            then_line(&mut b, "then");
        }
        "bots.phase" | "bots.no_buy" | "bots.friends" | "bots.tag" | "bots.layer" | "bots.creature_enemies" | "bots.goal" | "bots.goal_clear" | "bots.offer" | "bots.offer_clear" => {
            // who: all / ct / t / the wired bot / @tag (a tag is a name: letters, digits, _ - .)
            let who = |choice_id: &str, tag_id: &str, player_pin: &str| -> String {
                let tag = name_text(gen.text(n, def, tag_id));
                let (code, player) = match gen.choice(n, def, choice_id) {
                    "ct" => (1, "0".to_string()),
                    "t" => (2, "0".to_string()),
                    "player" if v.data_source(id, player_pin).is_some() => (3, inp(player_pin, "player")),
                    "tag" if !tag.is_empty() => (4, "0".to_string()),
                    _ => (0, "0".to_string()),
                };
                format!("lsg_who({code}, {player}, {})", pawn_str(&tag))
            };
            let num = |p: &str| inp(p, "number");
            let infl = || format!("clamp(floatround({}), 0, 100)", inp("influence", "number"));
            let line = match def.kind.as_str() {
                "bots.phase" => format!("\"phase %s %d\", {}, {}", pawn_str(match gen.choice(n, def, "phase") { "fight" => "fight", "off" => "off", _ => "loot" }), infl()),
                "bots.no_buy" => format!("\"nobuy %d\", {}", bool01(&inp("on", "bool"))),
                "bots.creature_enemies" => format!("\"enemyclass ls_creature %d\", {}", bool01(&inp("on", "bool"))),
                "bots.tag" => format!("\"role %d %s\", {}, {}", inp("player", "player"), pawn_str(&name_text(gen.text(n, def, "tag")))),
                "bots.friends" => format!("\"friend %s %s %d\", {}, {}, {}", who("a", "a_tag", "a_player"), who("b", "b_tag", "b_player"), bool01(&inp("on", "bool"))),
                "bots.layer" => format!("\"layer %s %s %d\", {}, {}, {}", who("who", "tag", "player"), pawn_str(match gen.choice(n, def, "layer") { "below" => "below", "high" => "high", "-" => "-", _ => "surface" }), infl()),
                "bots.goal" => {
                    let kind = match gen.choice(n, def, "kind") { "capture" => "capture", "use" => "use", "loot" => "loot", "go_hold" => "go_hold", _ => "defend" };
                    let at = inp("at", "entity");
                    format!("\"goal %s {kind} %s %d %d %d %d %d\", {}, lsg_ptext({at}), {at}, clamp(floatround({}), 0, 100), max(floatround({}), 1), {}, {}", who("who", "tag", "player"), num("prio"), num("secs"), gen.num(n, def, "radius").round().clamp(8.0, 4000.0) as i64, infl())
                }
                "bots.goal_clear" => format!("\"goal_clear %s %s\", {}, {}", who("who", "tag", "player"), pawn_str(match gen.choice(n, def, "kind") { "defend" => "defend", "capture" => "capture", "use" => "use", "loot" => "loot", "go_hold" => "go_hold", _ => "0" })),
                "bots.offer" => {
                    let kind = match gen.choice(n, def, "kind") { "bench" => "bench", "box" => "box", "trap" => "trap", "turret" => "turret", _ => "perk" };
                    let at = inp("at", "entity");
                    format!("\"offer %s {kind} %s %d %d %d %d\", {}, lsg_ptext({at}), {at}, max(floatround({}), 0), {}, {}", who("who", "tag", "player"), num("cost"), gen.num(n, def, "order").round().clamp(0.0, 20.0) as i64, infl())
                }
                _ => {
                    if v.data_source(id, "at").is_some() {
                        format!("\"offer_clear %s %d\", {}, {}", who("who", "tag", "player"), inp("at", "entity"))
                    } else {
                        format!("\"offer_clear %s\", {}", who("who", "tag", "player"))
                    }
                }
            };
            check(&mut b, comps, "gm_components isn't running on this server.", &[format!("\tif (!lsg_hint(fmt({line})))"), if debug { format!("\t\tlsd_error({i}, \"This server's bot brain doesn't take hints (ls_mode).\")\n\t\telse\n\t\t\tlsd_exit({i}, \"told\")") } else { "\t\t{}".into() }]);
            then_line(&mut b, "then");
        }
        "creature.spawn" => {
            let kind = pawn_str(&name_text(gen.text(n, def, "kind")));
            let tier = gen.choice(n, def, "tier").parse::<i64>().unwrap_or(0).clamp(0, 3);
            let (hp, pay) = (inp("health", "number"), inp("pay", "number"));
            // (at a wired entity, or the one with the name)
            let at = match v.data_source(id, "at_ent") {
                Some(_) => format!("lsg_ent_find_origin({})", inp("at_ent", "entity")),
                None => pawn_str(&name_text(gen.text(n, def, "at"))),
            };
            // (the shared transform: At position, or At entity with a Local offset)
            let xf = fx::place_wired(gen, id, "at_ent") || (v.data_source(id, "at_ent").is_some() && fx::copies(n, def));
            if xf {
                b.line("\tnew Float:o[3]");
            }
            let at_call = |hp: &str, pay: &str| if xf { format!("({} ? lsg_creature_o2({kind}, o, {tier}, {hp}, floatround({pay})) : 0)", fx::place(gen, id, "at_ent", &format!("lsg_ent_origin({}, o)", inp("at_ent", "entity")))) } else { format!("lsg_creature_x({kind}, {at}, {tier}, {hp}, floatround({pay}))") };
            if debug {
                b.line(format!("\tif (!get_cvar_pointer(\"ls_creature_ver\"))\n\t\t{}\n\telse if (!(g_lsgOut[{i}] = {}))\n\t\t{}\n\telse\n\t\tlsd_exit({i}, fmt(\"entity %d\", g_lsgOut[{i}]))", err("This server's bot brain has no creatures (ls_creature)."), at_call(&hp, &pay), err("Not spawned: no entity has that name, or that type isn't loaded on this map.")));
            } else {
                b.line(format!("\tg_lsgOut[{i}] = {}", at_call(&hp, &pay)));
            }
            if let Some(l) = fx::apply(gen, n, def, id, &format!("g_lsgOut[{i}]"), "at_ent", true, None) {
                b.line(l);
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
        "timeline.play" => {
            if let Some(tl) = n.params.get("timeline").and_then(Value::as_str).and_then(|t| g_of(gen).timelines.iter().find(|x| x.id == t)) {
                let inst = timeline_instance(gen, id);
                // (its state first: the other inputs' functions use it)
                after.push(timeline_code(gen, id, tl, &next));
                b.line(format!("\tnew inst = {inst}\n\tlsg_tl{i}_save(inst, ctx)\n\tlsg_tl{i}_go(inst, -1.0, 1)"));
                let mut ctl = |pin: &str, body: &str| after.push(format!("stock {}(ctx[])\n{{\n\tnew inst = {inst}\n{body}\n}}", gen.pin_fname(id, pin)));
                ctl("play_start", &format!("\tlsg_tl{i}_save(inst, ctx)\n\tlsg_tl{i}_go(inst, 0.0, 1)"));
                ctl("stop", &format!("\t#pragma unused ctx\n\tlsg_tl{i}_stop(inst)"));
                ctl("reverse", &format!("\tlsg_tl{i}_save(inst, ctx)\n\tlsg_tl{i}_go(inst, -1.0, -1)"));
                ctl("reverse_end", &format!("\tlsg_tl{i}_save(inst, ctx)\n\tlsg_tl{i}_go(inst, {}, -1)", float_lit(tl.play_length())));
                let nt = gen.input(id, "new_time").map(|x| x.expr).unwrap_or_else(|| "0.0".into());
                ctl("set_time", &format!("\tlsg_tl{i}_save(inst, ctx)\n\tg_lsgTlT{i}[inst] = floatclamp({nt}, 0.0, {})", float_lit(tl.play_length())));
            }
            let e = exit("playing");
            if !e.is_empty() {
                b.line(e);
            }
        }
        "event.call" => {
            if let Some((ci, c)) = custom_event_of(g_of(gen), n) {
                let args: Vec<String> = c.inputs.iter().map(|m| gen.need(id, &m.id, &m.ty)).collect();
                b.line(format!("\tlsg_ce{ci}({})", args.join(", ")));
            }
            let e = exit("called");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "timer.by_event" => {
            if let Some((ci, _)) = custom_event_of(g_of(gen), n) {
                let looping = gen.boolean(n, def, "loop");
                b.line(format!("\tremove_task(LSG_TASK + {i})\n\tset_task({}, \"lsg_te{i}\", LSG_TASK + {i}, _, _, {})", lsg_secs(&gen.input(id, "secs")), if looping { "\"b\"" } else { "\"\"" }));
                let hold = if debug { "\tif (lsd_hold_timer())\n\t\treturn\n".to_string() } else { String::new() };
                after.push(format!("public lsg_te{i}()\n{{\n{hold}\tlsg_ce{ci}()\n}}"));
            }
            let e = exit("started");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
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
        // ---- W1: utility flow nodes
        "debug.draw_line" | "debug.draw_box" | "debug.draw_point" => {
            // (only while testing: a release build draws nothing and works nothing out)
            if debug {
                let (r, g, bl) = w1::rgb(gen.choice(n, def, "color"));
                let secs = float_lit(gen.num(n, def, "secs"));
                let call = match def.kind.as_str() {
                    "debug.draw_line" => format!("lsg_w1_line({}, {}, {r}, {g}, {bl}, {secs})", inp("a", "vector"), inp("b", "vector")),
                    "debug.draw_box" => format!("lsg_w1_box({}, {}, 0.0, {r}, {g}, {bl}, {secs})", inp("a", "vector"), inp("b", "vector")),
                    _ => format!("lsg_w1_box({}, -1, {}, {r}, {g}, {bl}, {secs})", inp("pos", "vector"), float_lit(gen.num(n, def, "size") / 2.0)),
                };
                b.line(format!("\tif (!lsg_msg_room({}, 1, 0, {}))\n\t\t{}\n\telse\n\t{{\n\t\t{call}\n\t{}\n\t}}", pawn_str(&def.title), gen.limit_expr("messages"), err("Skipped: too many effects this server frame (Effect messages a frame, Limits)."), exit("drawn")));
            }
            then_line(&mut b, "then");
        }
        "debug.log_line" => {
            let most = gen.num(n, def, "most").round().clamp(1.0, 100.0) as i64;
            if debug {
                b.line(format!("\tif (lsg_w1_log({i}, {}, {most}))\n\t{}\n\telse\n\t{}", inp("text", "text"), exit("logged"), exit("skipped: too many a second")));
            } else {
                b.line(format!("\tlsg_w1_log({i}, {}, {most})", inp("text", "text")));
            }
            then_line(&mut b, "then");
        }
        "event.on_tick" => {
            let e = exit("tick");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "array.reverse" => {
            b.line(format!("\tlsg_w1_areverse({})", inp("array", "any")));
            let e = exit("reversed");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "var.toggle" => {
            if let Some(var) = v.var_of(id).filter(|x| x.ty == "bool") {
                let r = gen.var_ref(var, &inp("player", "player"), &inp("team", "team"));
                b.line(format!("\t{r} = !{r}"));
                if let Some(c) = gen.out_index(id, "value") {
                    b.line(format!("\tg_lsgO[{c}] = {r} ? 1 : 0"));
                }
                if debug {
                    b.line(format!("\tlsd_exit({i}, g_lsdBool[{r} ? 1 : 0])"));
                }
            }
            then_line(&mut b, "then");
        }
        "flow.do_once_per_player" => {
            // (one bit per player in g_lsgLeft)
            let p = inp("player", "player");
            b.line(format!("\tnew p = {p}\n\tif (p < 1 || p > 32)\n\t{{\n\t{}\n\t\treturn\n\t}}\n\tif (g_lsgLeft[{i}] & (1 << (p - 1)))\n\t{{\n\t{}\n\t\treturn\n\t}}\n\tg_lsgLeft[{i}] |= 1 << (p - 1)", exit("no player"), exit("done for this player")));
            after.push(multi_fn(gen, id, "reset", &format!("\tg_lsgLeft[{i}] = 0")));
            // (its own function: the Player as wired, not the main function's local copy of it)
            let rp = gen.input_unpinned(id, "player").map(|x| x.expr).unwrap_or_else(|| "0".into());
            after.push(multi_fn(gen, id, "reset_player", &format!("\tnew p = {rp}\n\tif (p >= 1 && p <= 32)\n\t\tg_lsgLeft[{i}] &= ~(1 << (p - 1))")));
            let e = exit("through");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        "flow.multi_gate" => {
            // (g_lsgLeft: a bit per output already used)
            let outs = v.exec_outs(id);
            let cnt = outs.len().clamp(1, 8);
            let all = (1u32 << cnt) - 1;
            let start = (gen.num(n, def, "start").round() as usize).min(cnt - 1);
            let refill = if gen.boolean(n, def, "loop") { format!("\t\tg_lsgLeft[{i}] = 0") } else { format!("\t{}\n\t\treturn", exit("all used")) };
            b.line(format!("\tif ((g_lsgLeft[{i}] & {all}) == {all})\n\t{{\n{refill}\n\t}}\n\tnew k = -1"));
            if gen.boolean(n, def, "random") {
                b.line(format!("\tnew free[8], nf\n\tfor (new j = 0; j < {cnt}; j++)\n\t\tif (!(g_lsgLeft[{i}] & (1 << j)))\n\t\t\tfree[nf++] = j\n\tif (nf)\n\t\tk = free[random(nf)]"));
            } else {
                b.line(format!("\tfor (new j = 0; j < {cnt}; j++)\n\t{{\n\t\tnew q = ({start} + j) % {cnt}\n\t\tif (!(g_lsgLeft[{i}] & (1 << q)))\n\t\t{{\n\t\t\tk = q\n\t\t\tbreak\n\t\t}}\n\t}}"));
            }
            b.line(format!("\tif (k < 0)\n\t\treturn\n\tg_lsgLeft[{i}] |= 1 << k\n\tswitch (k)\n\t{{"));
            for (k, pin) in outs.iter().enumerate().take(cnt) {
                let e = exit(&format!("Out {k}"));
                let call = next(pin).map(|c| format!("\t\t\t{c}\n")).unwrap_or_default();
                b.line(format!("\t\tcase {k}:\n\t\t{{\n{}{call}\t\t}}", if e.is_empty() { String::new() } else { format!("\t\t{e}\n") }));
            }
            b.line("\t}");
            after.push(multi_fn(gen, id, "reset", &format!("\tg_lsgLeft[{i}] = 0")));
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
                b.line(format!("\tlsd_exit({i}, lsd_times(n))"));
            } else {
                b.line("\t#pragma unused n");
            }
            then_line(&mut b, "then");
        }
        "flow.switch_num" | "flow.switch_text" | "flow.switch_enum" => {
            let outs: Vec<(String, String)> = v.outputs(id).iter().filter(|p| p.id != "default").map(|p| (p.id.clone(), p.label.clone())).collect();
            // (the debugger is told which way it went)
            let enumed = def.kind == "flow.switch_enum";
            let call = |pin: &str, label: &str| {
                let ind = if enumed { "\t" } else { "" };
                // (the label shown, a carets-free copy: it's only for the debugger's eyes)
                let e = exit(&label.replace('^', ""));
                format!("{}{}", if e.is_empty() { String::new() } else { format!("{ind}\t{e}\n") }, next(pin).map(|c| format!("{ind}\t\t{c}\n")).unwrap_or_default())
            };
            if enumed {
                b.line(format!("\tswitch ({})\n\t{{", inp("in", "any")));
                for (k, (pin, label)) in outs.iter().enumerate() {
                    b.line(format!("\t\tcase {k}:\n\t\t{{\n{}\t\t}}", call(pin, label)));
                }
                b.line(format!("\t\tdefault:\n\t\t{{\n{}\t\t}}\n\t}}", call("default", "default")));
            } else {
                let num = def.kind == "flow.switch_num";
                b.line(if num { format!("\tnew Float:x = {}", inp("in", "number")) } else { format!("\tnew x[128]\n\tcopy(x, 127, {})", inp("in", "text")) });
                for (k, (pin, label)) in outs.iter().enumerate() {
                    let test = if num { format!("x == {}", float_lit(label.parse::<f64>().unwrap_or(0.0))) } else { format!("equali(x, {})", pawn_str(&safe_text(label, 60))) };
                    b.line(format!("\t{}if ({test})\n\t{{\n{}\t}}", if k > 0 { "else " } else { "" }, call(pin, label)));
                }
                let d = call("default", "default");
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
        // engine coverage nodes (EC)
        k if coverage::is_event(k) => {
            let e = exit("ran");
            if !e.is_empty() {
                b.line(e);
            }
            then_line(&mut b, "then");
        }
        k if coverage::is_action(k) => {
            coverage::action(gen, id, n, def, &mut b, &inp, &exit, &err);
            then_line(&mut b, "then");
        }
        k if engine::is_action(k) => {
            engine::action(gen, id, n, def, &mut b, &inp, &exit, &err);
            then_line(&mut b, "then");
        }
        k if p5::is_action(k) => {
            p5::action(gen, id, n, def, &mut b, &inp, &exit, &next);
            then_line(&mut b, "then");
        }
        k if fx::is_action(k) => {
            fx::action(gen, id, n, def, &mut b, &inp, &exit, &err);
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
    // (paused: it waits, and after Resume keeps its schedule: what it still had to wait)
    let hold = if debug { format!("\tif (lsd_timer_wait({i}, \"lsg_t{i}\", data))\n\t\treturn\n") } else { String::new() };
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
            "number" => format!("\"%s %s\", {text}, lsg_ntext({})", val.expr),
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
    "creature.", "cast.", "fun.", "col.", "vol.", "timeline.", "table.", "player.transform", "player.set_transform", "player.set_rotation", "player.look_at", "ent.", "fx.",
    "marker.",
    "proj.", // T1fix
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

/// A Timeline node's instance: in a Function with a Player input, that player (each player has
/// their own); else the one.
fn timeline_instance(gen: &Gen, id: &str) -> String {
    let home = gen.v.graph_of(id);
    if let Some(f) = gen.v.g.functions.iter().find(|f| f.id == home) {
        if let Some(m) = f.inputs.iter().find(|m| m.ty == "player") {
            let entry = gen.live.iter().copied().find(|e| gen.v.defs[e].kind == "fn.entry" && gen.v.graph_of(e) == f.id);
            if let Some(sl) = entry.and_then(|e| gen.slot(e, &m.id)) {
                return format!("clamp(ctx[{sl}], 0, 32)");
            }
        }
    }
    "0".into()
}

/// A track's value at time t, for one component: from each key to the next as the key says
/// (straight, a step, eased in, out or both, or a cubic Hermite curve by the tangents), worked
/// out with a few multiplications: no tables.
fn track_eval(name: &str, keys: &[&super::graph::Key], comp: Option<usize>) -> String {
    let val = |k: &super::graph::Key| -> f64 {
        match comp {
            Some(c) => k.v.as_array().and_then(|a| a.get(c)).and_then(Value::as_f64).unwrap_or(0.0),
            None => k.v.as_f64().unwrap_or(0.0),
        }
    };
    // a tangent: the key's own, else through its neighbors (Auto)
    let tangent = |i: usize, side_in: bool| -> f64 {
        keys[i].own_tangent(side_in, comp).unwrap_or_else(|| {
            let (a, b) = (keys[i.saturating_sub(1)], keys[(i + 1).min(keys.len() - 1)]);
            let dt = b.t - a.t;
            if dt > 1e-6 { (val(b) - val(a)) / dt } else { 0.0 }
        })
    };
    let mut b = format!("stock Float:{name}(Float:t)\n{{");
    if keys.is_empty() {
        b.push_str("\n\t#pragma unused t\n\treturn 0.0\n}");
        return b;
    }
    b.push_str(&format!("\n\tif (t <= {})\n\t\treturn {}", float_lit(keys[0].t), float_lit(val(keys[0]))));
    for (i, w) in keys.windows(2).enumerate() {
        let (a, z) = (w[0], w[1]);
        let span = (z.t - a.t).max(0.0001);
        let x = format!("(t - {}) / {}", float_lit(a.t), float_lit(span));
        let (p0, p1) = (val(a), val(z));
        let body = match a.ease.as_deref() {
            Some("constant") => float_lit(p0),
            Some("ease") => format!("{} + {} * lsg_tl_ease({x})", float_lit(p0), float_lit(p1 - p0)),
            Some("in") => format!("{} + {} * x * x", float_lit(p0), float_lit(p1 - p0)),
            Some("out") => format!("{} + {} * x * (2.0 - x)", float_lit(p0), float_lit(p1 - p0)),
            Some("custom") => {
                let (m0, m1) = (tangent(i, false) * span, tangent(i + 1, true) * span);
                format!(
                    "{} * (2.0 * x * x * x - 3.0 * x * x + 1.0) + {} * (x * x * x - 2.0 * x * x + x) + {} * (3.0 * x * x - 2.0 * x * x * x) + {} * (x * x * x - x * x)",
                    float_lit(p0), float_lit(m0), float_lit(p1), float_lit(m1)
                )
            }
            _ => format!("{} + {} * ({x})", float_lit(p0), float_lit(p1 - p0)),
        };
        if body.contains(" x") || body.contains("(x") {
            b.push_str(&format!("\n\tif (t <= {})\n\t{{\n\t\tnew Float:x = {x}\n\t\treturn {body}\n\t}}", float_lit(z.t)));
        } else {
            b.push_str(&format!("\n\tif (t <= {})\n\t\treturn {body}", float_lit(z.t)));
        }
    }
    b.push_str(&format!("\n\treturn {}\n}}", float_lit(val(keys[keys.len() - 1]))));
    b
}

/// A Timeline node's own pieces: its state per instance, play/stop, the tick (the values, the
/// event keys crossed, Update, Finished at the end or around again when Looping).
fn timeline_code(gen: &Gen, id: &str, tl: &super::graph::Timeline, next: &dyn Fn(&str) -> Option<String>) -> String {
    let i = gen.index(id);
    let len = float_lit(tl.play_length());
    // (never under AMX Mod X's shortest repeat: what's asked is what runs)
    let tick = float_lit(tl.tick.max(super::graph::MIN_TICK));
    let mut o = format!(
        "// Timeline: {name}\nstock Float:g_lsgTlT{i}[33], g_lsgTlDir{i}[33], bool:g_lsgTlOn{i}[33], g_lsgTlC{i}[33][LSG_CTX], Float:g_lsgTlAt{i}[33], bool:g_lsgTlNew{i}[33]\n\
stock lsg_tl{i}_save(inst, ctx[])\n{{\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tg_lsgTlC{i}[inst][k] = ctx[k]\n}}\n\
stock lsg_tl{i}_go(inst, Float:from, dir)\n{{\n\tif (from >= 0.0)\n\t\tg_lsgTlT{i}[inst] = from\n\telse if (!g_lsgTlOn{i}[inst] && ((dir > 0 && g_lsgTlT{i}[inst] >= {len}) || (dir < 0 && g_lsgTlT{i}[inst] <= 0.0)))\n\t\tg_lsgTlT{i}[inst] = dir > 0 ? 0.0 : {len}\n\tg_lsgTlDir{i}[inst] = dir\n\tg_lsgTlNew{i}[inst] = true\n\tif (!g_lsgTlOn{i}[inst])\n\t{{\n\t\tg_lsgTlOn{i}[inst] = true\n\t\tg_lsgTlAt{i}[inst] = get_gametime()\n\t\tset_task({tick}, \"lsg_tl{i}_tick\", LSG_TASK + 50000 + {i} * 33 + inst, _, _, \"b\")\n\t}}\n}}\n\
stock lsg_tl{i}_stop(inst)\n{{\n\tg_lsgTlOn{i}[inst] = false\n\tremove_task(LSG_TASK + 50000 + {i} * 33 + inst)\n}}\n",
        name = comment_text(&tl.name)
    );
    // the tracks' values
    for (k, tr) in tl.tracks.iter().enumerate() {
        let keys = tr.sorted();
        match tr.kind.as_str() {
            "float" => o.push_str(&format!("{}\n", track_eval(&format!("lsg_tl{i}_v{k}"), &keys, None))),
            "vector" | "color" => {
                for c in 0..3 {
                    o.push_str(&format!("{}\n", track_eval(&format!("lsg_tl{i}_v{k}_{c}"), &keys, Some(c))));
                }
            }
            _ => {}
        }
    }
    // (stopped at a breakpoint: the time held there doesn't count)
    let hold = if gen.debug { format!("\tif (lsd_hold_timer())\n\t{{\n\t\tg_lsgTlAt{i}[inst] = get_gametime()\n\t\treturn\n\t}}\n") } else { String::new() };
    // by the game time since the last tick, not the tick's nominal step (a timer under load or
    // CrossOver comes late: a 4 s Timeline took 9), at most half a second at once after a hitch;
    // a fresh Play or Reverse also counts a key right where it starts
    o.push_str(&format!("public lsg_tl{i}_tick(task)\n{{\n\tnew inst = task - (LSG_TASK + 50000 + {i} * 33)\n\tif (inst < 0 || inst > 32)\n\t\treturn\n{hold}\tnew Float:now = get_gametime(), Float:step = floatclamp(now - g_lsgTlAt{i}[inst], 0.0, 0.5)\n\tg_lsgTlAt{i}[inst] = now\n\tnew bool:fresh = g_lsgTlNew{i}[inst]\n\tg_lsgTlNew{i}[inst] = false\n\tnew Float:was = g_lsgTlT{i}[inst], dir = g_lsgTlDir{i}[inst]\n\tnew Float:t = was + step * float(dir)\n\tnew bool:done, bool:wrapped\n"));
    if tl.looping {
        o.push_str(&format!("\tif (dir > 0 && t > {len})\n\t{{\n\t\tt -= {len}\n\t\twrapped = true\n\t}}\n\telse if (dir < 0 && t < 0.0)\n\t{{\n\t\tt += {len}\n\t\twrapped = true\n\t}}\n"));
    } else {
        o.push_str(&format!("\tif (dir > 0 && t >= {len})\n\t{{\n\t\tt = {len}\n\t\tdone = true\n\t}}\n\telse if (dir < 0 && t <= 0.0)\n\t{{\n\t\tt = 0.0\n\t\tdone = true\n\t}}\n"));
    }
    o.push_str(&format!("\tg_lsgTlT{i}[inst] = t\n\tnew c[LSG_CTX]\n\tfor (new k = 0; k < LSG_CTX; k++)\n\t\tc[k] = g_lsgTlC{i}[inst][k]\n"));
    let mut uses_c = false;
    if let Some(sl) = gen.slot(id, "forward") {
        o.push_str(&format!("\tc[{sl}] = dir > 0 ? 1 : 0\n"));
    }
    if let Some(sl) = gen.slot(id, "time") {
        o.push_str(&format!("\tc[{sl}] = _:t\n"));
    }
    for (k, tr) in tl.tracks.iter().enumerate() {
        let Some(sl) = gen.slot(id, &tr.id) else { continue };
        match tr.kind.as_str() {
            "float" => o.push_str(&format!("\tc[{sl}] = _:lsg_tl{i}_v{k}(t)\n")),
            "vector" | "color" => o.push_str(&format!("\tc[{sl}] = lsg_vec(lsg_tl{i}_v{k}_0(t), lsg_tl{i}_v{k}_1(t), lsg_tl{i}_v{k}_2(t))\n")),
            _ => {}
        }
    }
    // event keys the playhead crossed this tick (both ways; around the end when Looping)
    for tr in tl.tracks.iter().filter(|t| t.kind == "event") {
        let Some(call) = next(&tr.id) else { continue };
        let call = call.replace("(ctx)", "(c)");
        uses_c = true;
        for k in tr.sorted() {
            let kt = float_lit(k.t);
            o.push_str(&format!("\tif ((!wrapped && ((dir > 0 && (was < {kt} || (fresh && was == {kt})) && t >= {kt}) || (dir < 0 && (was > {kt} || (fresh && was == {kt})) && t <= {kt}))) || (wrapped && ((dir > 0 && (was < {kt} || t >= {kt})) || (dir < 0 && (was > {kt} || t <= {kt})))))\n\t\t{call}\n"));
        }
    }
    if let Some(call) = next("update") {
        uses_c = true;
        o.push_str(&format!("\t{}\n", call.replace("(ctx)", "(c)")));
    }
    if gen.debug {
        // the debugger: where it is, which way, and each track's value, at most 4 times a second
        // (and at the end) so the log isn't flooded
        let mut fmt = String::from("%.2f of {len_s} s, %s");
        let mut args = format!("t, dir > 0 ? \"forward\" : \"backward\"");
        for (k, tr) in tl.tracks.iter().enumerate().take(6) {
            let name = safe_text(&tr.name, 24).replace('%', "%%");
            match tr.kind.as_str() {
                "float" => {
                    fmt.push_str(&format!("; {name}: %.2f"));
                    args.push_str(&format!(", lsg_tl{i}_v{k}(t)"));
                }
                "vector" | "color" => {
                    fmt.push_str(&format!("; {name}: %.0f %.0f %.0f"));
                    args.push_str(&format!(", lsg_tl{i}_v{k}_0(t), lsg_tl{i}_v{k}_1(t), lsg_tl{i}_v{k}_2(t)"));
                }
                _ => {}
            }
        }
        let fmt = fmt.replace("{len_s}", &format!("{:.2}", tl.play_length()));
        // (Finished always; on the way at most 4 times a second a copy, within the node's share of
        // the events file like every node's runs: StudioPerfAudit #5)
        o.push_str(&format!("\tstatic Float:dbg[33]\n\tif (done)\n\t\tlsd_note({i}, fmt({f}, {args}))\n\telse if (get_gametime() - dbg[inst] >= 0.25 && lsd_want({i}, false))\n\t{{\n\t\tdbg[inst] = get_gametime()\n\t\tlsd_exit({i}, fmt({f}, {args}))\n\t}}\n", f = pawn_str(&fmt)));
    }
    o.push_str(&format!("\tif (done)\n\t{{\n\t\tlsg_tl{i}_stop(inst)\n"));
    if let Some(call) = next("finished") {
        uses_c = true;
        o.push_str(&format!("\t\t{}\n", call.replace("(ctx)", "(c)")));
    }
    o.push_str("\t}\n");
    if !uses_c {
        o.push_str("\t#pragma unused c\n");
    }
    o.push_str("\t#pragma unused wrapped, done, was, fresh\n}\n");
    o
}

/// The graph a generator works on.
fn g_of<'a>(gen: &Gen<'a>) -> &'a Graph {
    gen.v.g
}

/// The Custom Event a node names, with its place in the list.
fn custom_event_of<'a>(g: &'a Graph, n: &Node) -> Option<(usize, &'a super::graph::CustomEvent)> {
    let id = n.params.get("event").and_then(Value::as_str)?;
    g.custom_events.iter().enumerate().find(|(_, c)| c.id == id)
}

/// Each Custom Event: `lsg_ce<i>(inputs...)` runs every live Custom Event node for it, in the
/// graph's order (the event graphs' order). Ones the map can fire: the map's ls_event entities
/// (made info_target as they load) and info_targets named lsev_<map name>, when used.
fn custom_events_block(gen: &Gen, o: &mut Out) {
    let g = gen.v.g;
    let typed = |ty: &str, name: &str| match kind_of(ty) {
        's' => format!("const {name}[]"),
        _ if ty == "number" => format!("Float:{name}"),
        _ => name.to_string(),
    };
    for (ci, c) in g.custom_events.iter().enumerate() {
        let params: Vec<String> = c.inputs.iter().enumerate().map(|(j, m)| typed(&m.ty, &format!("a{j}"))).collect();
        o.line(format!("// Custom Event: {}\nstock lsg_ce{ci}({})\n{{", comment_text(&c.name), params.join(", ")));
        let nodes: Vec<&str> = gen.live.iter().copied().filter(|id| gen.v.defs[id].kind == "event.custom" && gen.v.nodes[id].params.get("event").and_then(Value::as_str) == Some(c.id.as_str())).collect();
        let mut used = vec![false; c.inputs.len()];
        for id in &nodes {
            o.line("\t{\n\t\tnew ctx[LSG_CTX]");
            for (j, m) in c.inputs.iter().enumerate() {
                if let Some(t) = gen.tout_index(id, &m.id) {
                    o.line(format!("\t\tcopy(g_lsgOT[{t}], 127, a{j})"));
                    used[j] = true;
                } else if let Some(sl) = gen.slot(id, &m.id) {
                    o.line(format!("\t\tctx[{sl}] = _:a{j}"));
                    used[j] = true;
                }
            }
            o.line(format!("\t\t{}(ctx)\n\t}}", gen.fname(id)));
        }
        for (j, u) in used.iter().enumerate() {
            if !u {
                o.line(format!("\t#pragma unused a{j}"));
            }
        }
        o.line("}");
        o.blank();
    }
    let mapped: Vec<(usize, &super::graph::CustomEvent)> = g.custom_events.iter().enumerate().filter(|(_, c)| c.map).collect();
    if mapped.is_empty() {
        return;
    }
    let names = mapped.iter().map(|(_, c)| pawn_str(&c.map_name())).collect::<Vec<_>>().join(", ");
    let mut calls = String::new();
    for (k, (ci, c)) in mapped.iter().enumerate() {
        // the map's values: Player gets who set it off; numbers and texts take keys a, b in turn
        let mut ab = 0;
        let args: Vec<String> = c
            .inputs
            .iter()
            .map(|m| match m.ty.as_str() {
                "player" => "who".to_string(),
                "number" | "text" | "bool" if ab < 2 => {
                    let key = if ab == 0 { "va" } else { "vb" };
                    ab += 1;
                    match m.ty.as_str() {
                        "number" => format!("str_to_float({key})"),
                        "bool" => format!("(str_to_num({key}) != 0)"),
                        _ => key.to_string(),
                    }
                }
                t => Gen::zero(t),
            })
            .collect();
        calls.push_str(&format!("\t\tcase {k}: lsg_ce{ci}({})\n", args.join(", ")));
    }
    o.line(format!(
        r#"// ---- custom events the map fires: ls_event entities and info_targets named lsev_<name> ----
new const g_lsgEvNames[][] = {{ {names} }}
#define LSG_EV_MAX 64
new g_lsgEvN, g_lsgEvEnt[LSG_EV_MAX], g_lsgEvK[LSG_EV_MAX], g_lsgEvA[LSG_EV_MAX][32], g_lsgEvB[LSG_EV_MAX][32]
new g_lsgEvParse, bool:g_lsgEvPend, g_lsgEvName[32], g_lsgEvTn[40], g_lsgEvVa[32], g_lsgEvVb[32]

public lsg_ev_kv(ent, kvd)
{{
	if (ent <= 0)
		return FMRES_IGNORED
	new key[32], val[64]
	get_kvd(kvd, KV_KeyName, key, charsmax(key))
	get_kvd(kvd, KV_Value, val, charsmax(val))
	if (equal(key, "classname"))
	{{
		if (ent != g_lsgEvParse)
		{{
			g_lsgEvParse = ent
			g_lsgEvPend = equal(val, "ls_event") || equal(val, "info_target")
			g_lsgEvName[0] = g_lsgEvTn[0] = g_lsgEvVa[0] = g_lsgEvVb[0] = 0
		}}
		if (g_lsgEvPend && equal(val, "ls_event"))
			set_kvd(kvd, KV_Value, "info_target")
		return FMRES_IGNORED
	}}
	if (!g_lsgEvPend || ent != g_lsgEvParse)
		return FMRES_IGNORED
	if (equal(key, "event"))
		copy(g_lsgEvName, charsmax(g_lsgEvName), val)
	else if (equal(key, "targetname"))
		copy(g_lsgEvTn, charsmax(g_lsgEvTn), val)
	else if (equal(key, "a"))
		copy(g_lsgEvVa, charsmax(g_lsgEvVa), val)
	else if (equal(key, "b"))
		copy(g_lsgEvVb, charsmax(g_lsgEvVb), val)
	return FMRES_IGNORED
}}

public lsg_ev_spawn(ent)
{{
	if (ent != g_lsgEvParse || !g_lsgEvPend)
		return FMRES_IGNORED
	g_lsgEvPend = false
	g_lsgEvParse = 0
	// the event's name: its event key, else the name after lsev_
	new name[32]
	if (g_lsgEvName[0])
		copy(name, charsmax(name), g_lsgEvName)
	else if (equal(g_lsgEvTn, "lsev_", 5))
		copy(name, charsmax(name), g_lsgEvTn[5])
	strtolower(name)
	for (new k = 0; k < sizeof g_lsgEvNames; k++)
	{{
		if (name[0] && equal(name, g_lsgEvNames[k]) && g_lsgEvN < LSG_EV_MAX && pev_valid(ent))
		{{
			g_lsgEvEnt[g_lsgEvN] = ent
			g_lsgEvK[g_lsgEvN] = k
			copy(g_lsgEvA[g_lsgEvN], 31, g_lsgEvVa)
			copy(g_lsgEvB[g_lsgEvN], 31, g_lsgEvVb)
			g_lsgEvN++
			break
		}}
	}}
	return FMRES_IGNORED
}}

public lsg_ev_use(ent, caller, activator)
{{
	new k = -1, va[32], vb[32]
	for (new i = 0; i < g_lsgEvN; i++)
	{{
		if (g_lsgEvEnt[i] == ent)
		{{
			k = g_lsgEvK[i]
			copy(va, charsmax(va), g_lsgEvA[i])
			copy(vb, charsmax(vb), g_lsgEvB[i])
			break
		}}
	}}
	// (one made while playing, by another plugin: by its name, lsev_<event>, without values)
	if (k < 0 && pev_valid(ent))
	{{
		new tn[40]
		pev(ent, pev_targetname, tn, charsmax(tn))
		if (equal(tn, "lsev_", 5))
		{{
			strtolower(tn)
			for (new j = 0; j < sizeof g_lsgEvNames; j++)
				if (equal(tn[5], g_lsgEvNames[j]))
					k = j
		}}
	}}
	if (k < 0)
		return HAM_IGNORED
	// who set it off: Ham_Use's first value is really the activator (the engine's Use(pActivator,
	// pCaller): ham_const.inc's names are the wrong way round), the second the trigger; take
	// whichever is a player (QA4-B: it was always nobody)
	new who = caller >= 1 && caller <= MaxClients && is_user_connected(caller) ? caller : activator >= 1 && activator <= MaxClients && is_user_connected(activator) ? activator : 0
	switch (k)
	{{
{calls}	}}
	#pragma unused va, vb, who
	return HAM_IGNORED
}}
"#
    ));
    o.blank();
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

/// Creatures with the mode's own health, tier and pay for their killer (Spawn Creature).
/// Mode settings (Blueprint's Instance Editable): each a cvar `lsm_<mode>_<variable id>`, kept in
/// the server's data/studio_settings/<mode>.cfg (read here, never exec'd: only these names and
/// numbers in range are taken), applied as the map starts and at each new round when changed,
/// and an admin menu (the Game Mode Menu's "options" for this mode).
fn settings_block(gen: &Gen, mode: &str, settings: &[&Variable]) -> String {
    let names: Vec<String> = settings.iter().map(|x| pawn_str(&super::graph::setting_cvar(mode, &x.id))).collect();
    let mut lo = Vec::new();
    let mut hi = Vec::new();
    let mut step = Vec::new();
    let mut defs = Vec::new();
    let mut labels = Vec::new();
    let mut apply = String::new();
    let mut choices = String::new();
    for (k, x) in settings.iter().enumerate() {
        let st = x.setting.as_ref().unwrap();
        let (a, b) = st.range(&x.ty);
        lo.push(float_lit(a));
        hi.push(float_lit(b));
        let sp = if x.ty == "bool" || !st.choices.is_empty() { 1.0 } else if st.step > 0.0 { st.step } else { ((b - a) / 10.0).max(1.0).round() };
        step.push(float_lit(sp));
        let d = match &x.default { Value::Bool(t) => if *t { 1.0 } else { 0.0 }, Value::Number(n) => n.as_f64().unwrap_or(0.0), _ => 0.0 };
        defs.push(format!("\"{}\"", if d.fract() == 0.0 { format!("{}", d as i64) } else { format!("{d}") }));
        labels.push(pawn_str(&safe_text(&st.label, 30)));
        let i = gen.var_index(&x.id);
        let set = if x.ty == "bool" { format!("g_v{i} = floatround(v) != 0") } else { format!("g_v{i} = v") };
        apply.push_str(&format!("\t\tcase {k}: {set}\n"));
        if !st.choices.is_empty() {
            let cs: Vec<String> = st.choices.iter().map(|c| pawn_str(&safe_text(c, 30))).collect();
            choices.push_str(&format!("\t\tcase {k}:\n\t\t{{\n\t\t\tstatic const C[][] = {{ {} }}\n\t\t\tnew c = clamp(floatround(v), 0, sizeof C - 1)\n\t\t\tcopy(out, len, C[c])\n\t\t\treturn\n\t\t}}\n", cs.join(", ")));
        }
        if x.ty == "bool" {
            choices.push_str(&format!("\t\tcase {k}:\n\t\t{{\n\t\t\tcopy(out, len, floatround(v) ? \"Yes\" : \"No\")\n\t\t\treturn\n\t\t}}\n"));
        }
    }
    let n = settings.len();
    let menu = pawn_str(&super::graph::settings_menu_cmd(mode));
    let file = pawn_str(&format!("{}/studio_settings/{}.cfg", "%s", mode));
    format!(
        r#"// ---- mode settings (hosts change them: the app's mode settings, the admin menu) ----
#define LSG_SN {n}
stock const g_lsgSn[LSG_SN][] = {{ {names} }}
stock const g_lsgSd[LSG_SN][] = {{ {defs} }}
stock const g_lsgSlab[LSG_SN][] = {{ {labels} }}
stock const Float:g_lsgSlo[LSG_SN] = {{ {lo} }}
stock const Float:g_lsgShi[LSG_SN] = {{ {hi} }}
stock const Float:g_lsgSst[LSG_SN] = {{ {step} }}
stock g_lsgSc[LSG_SN], Float:g_lsgSl[LSG_SN], g_lsgSmenu[33]

stock lsg_settings_init()
{{
	for (new k = 0; k < LSG_SN; k++)
		g_lsgSc[k] = register_cvar(g_lsgSn[k], g_lsgSd[k])
	register_clcmd({menu}, "lsg_settings_menu", ADMIN_CFG)
}}

// the host's value as it is (hosts are trusted: outside the author's range is theirs to choose)
stock Float:lsg_sv(k)
	return get_pcvar_float(g_lsgSc[k])

stock lsg_settings_path(path[], len)
{{
	new dir[160]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(path, len, {file}, dir)
}}

// the server's own values: only these names, as numbers (never run as commands); a setting the
// file leaves out is at the mode's default
stock lsg_settings_load()
{{
	new path[200], line[96], key[64], val[32]
	for (new k = 0; k < LSG_SN; k++)
		set_pcvar_float(g_lsgSc[k], str_to_float(g_lsgSd[k]))
	lsg_settings_path(path, charsmax(path))
	new f = fopen(path, "rt")
	if (!f)
		return
	while (!feof(f))
	{{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == '/' || line[0] == ';')
			continue
		parse(line, key, charsmax(key), val, charsmax(val))
		for (new k = 0; k < LSG_SN; k++)
		{{
			if (equali(key, g_lsgSn[k]))
			{{
				set_pcvar_float(g_lsgSc[k], str_to_float(val))
				break
			}}
		}}
	}}
	fclose(f)
}}

stock lsg_settings_save()
{{
	new path[200], dir[160]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio_settings")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_settings_path(path, charsmax(path))
	new f = fopen(path, "wt")
	if (!f)
		return
	fputs(f, "// this server's values for the mode's settings (the app and the admin menu write it)^n")
	for (new k = 0; k < LSG_SN; k++)
		fprintf(f, "%s ^"%.3f^"^n", g_lsgSn[k], lsg_sv(k))
	fclose(f)
}}

// the variables take the values (all, or the ones changed since)
stock lsg_settings_apply(bool:all)
{{
	for (new k = 0; k < LSG_SN; k++)
	{{
		new Float:v = lsg_sv(k)
		if (!all && v == g_lsgSl[k])
			continue
		g_lsgSl[k] = v
		switch (k)
		{{
{apply}		}}
	}}
}}

stock lsg_settings_text(k, Float:v, out[], len)
{{
	switch (k)
	{{
{choices}	}}
	if (v == float(floatround(v)))
		formatex(out, len, "%d", floatround(v))
	else
		formatex(out, len, "%.2f", v)
}}

public lsg_settings_menu(id, level, cid)
{{
	if (!(get_user_flags(id) & level))
		return PLUGIN_HANDLED
	new m = menu_create("\yMode settings^n\dSaved for this server", "lsg_settings_menu_h"), t[96], v[40], info[4]
	menu_additem(m, "Reset all to the mode's defaults", "all")
	for (new k = 0; k < LSG_SN; k++)
	{{
		lsg_settings_text(k, lsg_sv(k), v, charsmax(v))
		// (a * marks one that isn't the mode's default)
		formatex(t, charsmax(t), "%s: \y%s%s", g_lsgSlab[k], v, lsg_sv(k) == str_to_float(g_lsgSd[k]) ? "" : " \r*")
		num_to_str(k, info, charsmax(info))
		menu_additem(m, t, info)
	}}
	menu_display(id, m)
	return PLUGIN_HANDLED
}}

public lsg_settings_menu_h(id, menu, item)
{{
	if (item < 0)
	{{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}}
	new info[4], acc, cb
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb)
	menu_destroy(menu)
	if (equal(info, "all"))
	{{
		if (!(get_user_flags(id) & ADMIN_CFG))
			return PLUGIN_HANDLED
		for (new k = 0; k < LSG_SN; k++)
			set_pcvar_float(g_lsgSc[k], str_to_float(g_lsgSd[k]))
		lsg_settings_save()
		lsg_settings_apply(false)
		lsg_settings_menu(id, ADMIN_CFG, 0)
		return PLUGIN_HANDLED
	}}
	g_lsgSmenu[id] = str_to_num(info)
	lsg_settings_one(id)
	return PLUGIN_HANDLED
}}

stock lsg_settings_one(id)
{{
	new k = g_lsgSmenu[id], v[40], t[96]
	lsg_settings_text(k, lsg_sv(k), v, charsmax(v))
	formatex(t, charsmax(t), "\y%s: %s", g_lsgSlab[k], v)
	new m = menu_create(t, "lsg_settings_one_h")
	menu_additem(m, "Less", "-")
	menu_additem(m, "More", "+")
	menu_additem(m, "Smallest", "lo")
	menu_additem(m, "Largest", "hi")
	menu_additem(m, "Reset to the mode's default", "d")
	menu_display(id, m)
}}

public lsg_settings_one_h(id, menu, item)
{{
	if (item < 0)
	{{
		menu_destroy(menu)
		lsg_settings_menu(id, ADMIN_CFG, 0)
		return PLUGIN_HANDLED
	}}
	new info[4], acc, cb, k = g_lsgSmenu[id]
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb)
	menu_destroy(menu)
	if (!(get_user_flags(id) & ADMIN_CFG) || k < 0 || k >= LSG_SN)
		return PLUGIN_HANDLED
	new Float:v = lsg_sv(k)
	if (equal(info, "-"))
		v -= g_lsgSst[k]
	else if (equal(info, "+"))
		v += g_lsgSst[k]
	else if (equal(info, "lo"))
		v = g_lsgSlo[k]
	else if (equal(info, "hi"))
		v = g_lsgShi[k]
	else
		v = str_to_float(g_lsgSd[k])
	set_pcvar_float(g_lsgSc[k], v)
	lsg_settings_save()
	lsg_settings_apply(false)
	lsg_settings_one(id)
	return PLUGIN_HANDLED
}}
"#,
        names = names.join(", "),
        defs = defs.join(", "),
        labels = labels.join(", "),
        lo = lo.join(", "),
        hi = hi.join(", "),
        step = step.join(", "),
    )
}

/// Where a hit struck and which way it was going (Player Hurt's and Creature Hurt's Hit Position
/// and Direction): kept from the trace of the bullet or knife, the same frame; else the middle of
/// the one hit, and the way from whoever did it.
const HIT_BLOCK: &str = r##"// ---- where a hit struck ----
stock Float:g_lsgHitPos[2048][3], Float:g_lsgHitDir[2048][3], Float:g_lsgHitAt[2048]

public lsg_hit_trace(victim, attacker, Float:damage, Float:dir[3], tr, bits)
{
	if (victim < 1 || victim >= 2048)
		return HAM_IGNORED
	get_tr2(tr, TR_vecEndPos, g_lsgHitPos[victim])
	g_lsgHitDir[victim] = dir
	g_lsgHitAt[victim] = get_gametime()
	return HAM_IGNORED
}

stock lsg_hit_pos(v)
{
	if (v >= 1 && v < 2048 && get_gametime() - g_lsgHitAt[v] < 0.05)
		return lsg_vfrom(g_lsgHitPos[v])
	return lsg_ecenter(v)
}

stock lsg_hit_dir(v, from)
{
	if (v >= 1 && v < 2048 && get_gametime() - g_lsgHitAt[v] < 0.05)
		return lsg_vnorm(lsg_vfrom(g_lsgHitDir[v]))
	if (from > 0 && pev_valid(from) && from != v)
		return lsg_vdir(lsg_vent(from), lsg_vent(v))
	return lsg_vec(0.0, 0.0, 0.0)
}
"##;

/// Creature control (the bot brain's `ls_creature` commands; numbers only, so nothing else can
/// ride along): finding, sending, attacking, holding, health, removing, and what the watch keeps.
/// Nodes that need creature control 2 in the bot brain (ls_creature_ver 2).
const CREATURE_V2: [&str; 4] = ["creature.state", "creature.target", "creature.set_speed", "creature.set_tier"];

const CREATURE_CTL: &str = r##"// ---- creature control ----
stock Float:g_lsgCrPos[2048][3], Float:g_lsgCrStill[2048], g_lsgCrSerial[2048], g_lsgCrSeen[2048]
stock bool:g_lsgCrGoalOn[2048], Float:g_lsgCrGoal[2048][3]

// alive, not friendly (a turret), of the type (empty: any)
stock bool:lsg_cr_match(e, const type[])
{
	if (!lsg_is_creature(e) || pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
		return false
	new Float:hp
	pev(e, pev_health, hp)
	if (hp <= 0.0)
		return false
	if (!type[0])
		return true
	new t[24]
	pev(e, pev_netname, t, charsmax(t))
	return bool:equali(t, type)
}

stock lsg_cr_cmd(e, const fmt[])
{
	if (lsg_cr_match(e, "") && get_cvar_pointer("ls_creature_ver"))
		server_cmd(fmt, e)
}

stock lsg_cr_all(const type[])
{
	new a = lsg_atmp(), e = -1, n
	while (n < 512 && (e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (!lsg_cr_match(e, type))
			continue
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_cr_nearest(const type[], pos, to)
{
	new Float:at[3]
	if (pos >= 0)
		lsg_vto(pos, at)
	else if (to > 0 && pev_valid(to))
		pev(to, pev_origin, at)
	else
		return 0
	new e = -1, best, Float:bd = 999999.0, Float:o[3]
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e == to || !lsg_cr_match(e, type))
			continue
		pev(e, pev_origin, o)
		new Float:d = get_distance_f(o, at)
		if (d < bd)
		{
			bd = d
			best = e
		}
	}
	return best
}

stock lsg_cr_type(e)
{
	new t[24]
	if (lsg_is_creature(e))
		pev(e, pev_netname, t, charsmax(t))
	return t
}

stock Float:lsg_cr_idle(e)
{
	if (e <= 0 || e >= 2048 || !lsg_cr_match(e, "") || g_lsgCrSeen[e] != g_lsgCrSerial[e] || g_lsgCrStill[e] <= 0.0)
		return 0.0
	return get_gametime() - g_lsgCrStill[e]
}

// a clear line from its eyes to the thing's middle (or the thing itself in the way)
stock bool:lsg_cr_sees(e, what)
{
	if (!lsg_cr_match(e, "") || what <= 0 || !pev_valid(what))
		return false
	new Float:a[3], Float:v[3], Float:b[3]
	pev(e, pev_origin, a)
	pev(e, pev_view_ofs, v)
	a[2] += v[2] > 0.0 ? v[2] : 24.0
	pev(what, pev_origin, b)
	if (what <= MaxClients)
	{
		pev(what, pev_view_ofs, v)
		b[2] += v[2]
	}
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, e, tr)
	new Float:f
	get_tr2(tr, TR_flFraction, f)
	new hit = get_tr2(tr, TR_pHit)
	free_tr2(tr)
	return f >= 1.0 || hit == what
}

stock bool:lsg_cr_sees_any(e)
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && lsg_cr_sees(e, p))
			return true
	return false
}

// how: 0 guard there, 1 then resume hunting, 2 flee there
stock lsg_cr_goto(e, to, pos, const name[], how)
{
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	new Float:t[3]
	if (pos >= 0)
		lsg_vto(pos, t)
	else
	{
		if (to <= 0 && name[0])
			to = find_ent_by_tname(-1, name)
		if (to <= 0 || !pev_valid(to))
			return
		// (a brush entity, a gate: its middle)
		new Float:mn[3], Float:mx[3]
		pev(to, pev_absmin, mn)
		pev(to, pev_absmax, mx)
		for (new k = 0; k < 3; k++)
			t[k] = (mn[k] + mx[k]) / 2.0
	}
	server_cmd("ls_creature behave %d %s %.0f %.0f %.0f;ls_creature target %d 0", e, how == 2 ? "flee" : "guard", t[0], t[1], t[2], e)
	g_lsgCrGoalOn[e] = how == 1
	g_lsgCrGoal[e] = t
	g_lsgCrSerial[e]++
}

stock lsg_cr_attack(e, t)
{
	if (!lsg_cr_match(e, "") || t <= 0 || !pev_valid(t) || !get_cvar_pointer("ls_creature_ver"))
		return
	g_lsgCrGoalOn[e] = false
	server_cmd("ls_creature target %d %d;ls_creature behave %d chase", e, t, e)
}

// stays where it is (guarding its spot: it still fights what comes close, then comes back)
stock lsg_cr_hold(e)
{
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	new Float:o[3]
	pev(e, pev_origin, o)
	g_lsgCrGoalOn[e] = false
	server_cmd("ls_creature behave %d guard %.0f %.0f %.0f;ls_creature target %d 0", e, o[0], o[1], o[2], e)
}

// creature control 2 in the bot brain: state, target, default behavior, speed, tier
stock bool:lsg_cr_v2()
{
	return get_cvar_pointer("ls_creature_ver") && get_cvar_num("ls_creature_ver") >= 2
}

// back to its own AI: its type's spawn behavior, its spawn spot as home, no forced target
// (an older brain has no "default": it hunts)
stock lsg_cr_resume(e)
{
	if (e > 0 && e < 2048)
		g_lsgCrGoalOn[e] = false
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	if (lsg_cr_v2())
		server_cmd("ls_creature behave %d default", e)
	else
		server_cmd("ls_creature behave %d chase;ls_creature target %d 0", e, e)
}

// 1 idle, 2 wander, 3 patrol, 4 guard, 5 chase, 6 flee, 7 attack, 8 search, 9 dead; 0 not a creature
stock lsg_cr_state(e)
{
	if (!lsg_cr_v2() || !lsg_is_creature(e))
		return 0
	new s = pev(e, pev_iuser3)
	return s >= 1 && s <= 9 ? s : 0
}

stock lsg_cr_state_name(e)
{
	static const names[][] = { "", "idle", "wander", "patrol", "guard", "chase", "flee", "attack", "search", "dead" }
	new t[8]
	copy(t, charsmax(t), names[lsg_cr_state(e)])
	return t
}

stock lsg_cr_target(e)
{
	if (!lsg_cr_v2() || !lsg_cr_match(e, ""))
		return 0
	new t = pev(e, pev_enemy)
	return t > 0 && pev_valid(t) ? t : 0
}

stock lsg_cr_speed(e, Float:x)
{
	if (lsg_cr_v2() && lsg_cr_match(e, ""))
		server_cmd("ls_creature speed %d %.2f", e, floatclamp(x, 0.2, 3.0))
}

stock lsg_cr_tier(e, t)
{
	if (lsg_cr_v2() && lsg_cr_match(e, ""))
		server_cmd("ls_creature tier %d %d", e, clamp(t, 0, 3))
}

stock lsg_cr_health(e, Float:hp)
{
	if (!lsg_cr_match(e, ""))
		return
	hp = floatclamp(hp, 1.0, 100000.0)
	set_pev(e, pev_health, hp)
	set_pev(e, pev_max_health, hp)
}

"##;

const CREATURES_X: &str = r##"// a creature with the mode's own health, tier and pay for its killer
new g_lsgPay[2048]
stock lsg_creature_x(const kind[], const at[], tier, Float:hp, pay)
{
	new Float:o[3]
	if (!get_cvar_pointer("ls_creature_ver") || !lsg_origin_of(at, o))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	if (tier > 0)
	{
		new v[4]
		num_to_str(tier, v, charsmax(v))
		DispatchKeyValue(ent, "tier", v)
	}
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	if (!is_valid_ent(ent) || (entity_get_int(ent, EV_INT_flags) & FL_KILLME))
		return 0
	if (hp > 0.0)
	{
		entity_set_float(ent, EV_FL_health, hp)
		entity_set_float(ent, EV_FL_max_health, hp)
	}
	if (ent < sizeof g_lsgPay)
		g_lsgPay[ent] = clamp(pay, 0, 16000)
	// (Creatures Alive counts again)
	g_lsgCrAt = -1.0
	return ent
}

public lsg_pay_killer(ent, killer)
{
	if (ent > 0 && ent < sizeof g_lsgPay && g_lsgPay[ent])
	{
		if (lsg_ok(killer))
			cs_set_user_money(killer, min(cs_get_user_money(killer) + g_lsgPay[ent], 16000))
		g_lsgPay[ent] = 0
	}
	return HAM_IGNORED
}
"##;

/// World settings: node key -> (cvar, a yes/no setting).
pub const WORLD: [(&str, &str, bool); 17] = [
    ("gravity", "sv_gravity", false), ("friction", "sv_friction", false), ("max_speed", "sv_maxspeed", false), ("round_time", "mp_roundtime", false), ("freeze_time", "mp_freezetime", false),
    ("buy_time", "mp_buytime", false), ("friendly_fire", "mp_friendlyfire", true), ("team_balance", "mp_autoteambalance", true), ("team_limit", "mp_limitteams", false), ("start_money", "mp_startmoney", false),
    ("c4_timer", "mp_c4timer", false), ("fall_damage", "mp_falldamage", true), ("footsteps", "mp_footsteps", true), ("alltalk", "sv_alltalk", true), ("time_limit", "mp_timelimit", false),
    ("win_limit", "mp_winlimit", false), ("frag_limit", "mp_fraglimit", false),
];
fn world_of(kind: &str) -> Option<(&'static str, bool, bool)> {
    let key = kind.strip_prefix("world.")?;
    let (set, key) = match key.strip_prefix("set_") { Some(k) => (true, k), None => (false, key) };
    WORLD.iter().find(|(k, _, _)| *k == key).map(|(_, c, b)| (*c, *b, set))
}

/// Cvars no mode may read: passwords, the rcon, and where files and logs go.
pub fn secret_cvar(c: &str) -> bool {
    let c = c.to_ascii_lowercase();
    c.contains("password") || c.contains("rcon") || c.starts_with("sv_downloadurl") || c.starts_with("log") || c.contains("_file") || c.starts_with("amx_sql") || c.starts_with("sv_contact") || c == "hostport" || c == "ip"
}

/// A marker id's place in the graph's markers (what the plugin's tables use).
fn marker_index(g: &Graph, id: &str) -> usize {
    g.markers.iter().position(|m| m.id == id).unwrap_or(0)
}

/// The map's ls_marker entities (docs/notes/X.md, "Mode map kit format"): taken as the map loads
/// (the class becomes info_target so the engine keeps them; not drawn, not solid), kept in kind,
/// path, order order, with their keys; the kit's defaults for keys the map left out.
fn markers_block(g: &Graph) -> String {
    let ids = g.markers.iter().map(|m| pawn_str(&m.id)).collect::<Vec<_>>().join(", ");
    let ids = if ids.is_empty() { "\"\"".to_string() } else { ids };
    let mut defaults = String::new();
    for (k, m) in g.markers.iter().enumerate() {
        for key in &m.keys {
            let v = match &key.default {
                Value::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
                Value::Number(x) => x.to_string(),
                Value::String(t) => t.clone(),
                _ => continue,
            };
            defaults.push_str(&format!("\tif (k == {k} && equal(key, {}))\n\t\tcopy(t, charsmax(t), {})\n", pawn_str(&key.name), pawn_str(&safe_text(&v, 60))));
        }
    }
    format!(
        r##"// ---- map markers ----
#define LSG_MK_MAX 128
new const g_lsgMkIds[][] = {{ {ids} }}
new g_lsgMkN, g_lsgMkEnt[LSG_MK_MAX], g_lsgMkKind[LSG_MK_MAX], g_lsgMkPath[LSG_MK_MAX], g_lsgMkOrder[LSG_MK_MAX], Trie:g_lsgMkKeys[LSG_MK_MAX]
new g_lsgMkParse, bool:g_lsgMkPend, Trie:g_lsgMkPendKeys = Invalid_Trie

public lsg_mk_kv(ent, kvd)
{{
	if (ent <= 0)
		return FMRES_IGNORED
	new key[32], val[64]
	get_kvd(kvd, KV_KeyName, key, charsmax(key))
	get_kvd(kvd, KV_Value, val, charsmax(val))
	if (equal(key, "classname"))
	{{
		// the engine sends the class first on its own, then again among the keys
		if (ent != g_lsgMkParse)
		{{
			g_lsgMkParse = ent
			g_lsgMkPend = false
			if (g_lsgMkPendKeys != Invalid_Trie)
				TrieDestroy(g_lsgMkPendKeys)
			g_lsgMkPendKeys = Invalid_Trie
			// (another plugin may have made an ls_marker an info_target already: its kind decides)
			if (equal(val, "ls_marker") || equal(val, "info_target"))
			{{
				g_lsgMkPend = true
				g_lsgMkPendKeys = TrieCreate()
			}}
		}}
		if (g_lsgMkPend && equal(val, "ls_marker"))
			set_kvd(kvd, KV_Value, "info_target")
		return FMRES_IGNORED
	}}
	if (g_lsgMkPend && ent == g_lsgMkParse)
		TrieSetString(g_lsgMkPendKeys, key, val)
	return FMRES_IGNORED
}}

public lsg_mk_spawn(ent)
{{
	if (ent != g_lsgMkParse || !g_lsgMkPend)
		return FMRES_IGNORED
	g_lsgMkPend = false
	g_lsgMkParse = 0
	new Trie:keys = g_lsgMkPendKeys
	g_lsgMkPendKeys = Invalid_Trie
	new kind[32], mode[32], num[12], k = -1
	TrieGetString(keys, "kind", kind, charsmax(kind))
	TrieGetString(keys, "mode", mode, charsmax(mode))
	for (new i = 0; i < sizeof g_lsgMkIds; i++)
	{{
		if (kind[0] && equal(kind, g_lsgMkIds[i]))
		{{
			k = i
			break
		}}
	}}
	if (k < 0 || !lsg_mk_mode_ok(mode) || g_lsgMkN >= LSG_MK_MAX || !pev_valid(ent))
	{{
		TrieDestroy(keys)
		return FMRES_IGNORED
	}}
	set_pev(ent, pev_effects, pev(ent, pev_effects) | EF_NODRAW)
	set_pev(ent, pev_solid, SOLID_NOT)
	new path, order
	if (TrieGetString(keys, "path", num, charsmax(num)))
		path = str_to_num(num)
	if (TrieGetString(keys, "order", num, charsmax(num)))
		order = str_to_num(num)
	// in kind, path, order order (then the map's)
	new at = g_lsgMkN
	while (at > 0 && (g_lsgMkKind[at - 1] > k || (g_lsgMkKind[at - 1] == k && (g_lsgMkPath[at - 1] > path || (g_lsgMkPath[at - 1] == path && g_lsgMkOrder[at - 1] > order)))))
	{{
		g_lsgMkEnt[at] = g_lsgMkEnt[at - 1]
		g_lsgMkKind[at] = g_lsgMkKind[at - 1]
		g_lsgMkPath[at] = g_lsgMkPath[at - 1]
		g_lsgMkOrder[at] = g_lsgMkOrder[at - 1]
		g_lsgMkKeys[at] = g_lsgMkKeys[at - 1]
		at--
	}}
	g_lsgMkEnt[at] = ent
	g_lsgMkKind[at] = k
	g_lsgMkPath[at] = path
	g_lsgMkOrder[at] = order
	g_lsgMkKeys[at] = keys
	g_lsgMkN++
	return FMRES_IGNORED
}}

// a marker's mode: empty, "any", this mode's id, or a comma list with it
stock bool:lsg_mk_mode_ok(const mode[])
{{
	if (!mode[0] || equal(mode, "any") || equal(mode, LSG_MODE))
		return true
	new list[40], me[32]
	formatex(list, charsmax(list), ",%s,", mode)
	replace_all(list, charsmax(list), " ", "")
	formatex(me, charsmax(me), ",%s,", LSG_MODE)
	return contain(list, me) >= 0 || contain(list, ",any,") >= 0
}}

stock lsg_mk_count(k)
{{
	new n
	for (new m = 0; m < g_lsgMkN; m++)
		if (g_lsgMkKind[m] == k)
			n++
	return n
}}

// the n-th (from 1) marker of a kind, or 0
stock lsg_mk_get(k, n)
{{
	for (new m = 0; m < g_lsgMkN; m++)
		if (g_lsgMkKind[m] == k && --n == 0)
			return pev_valid(g_lsgMkEnt[m]) ? g_lsgMkEnt[m] : 0
	return 0
}}

stock lsg_mk_key(ent, const key[])
{{
	new t[64]
	for (new m = 0; m < g_lsgMkN; m++)
	{{
		if (g_lsgMkEnt[m] != ent || !ent)
			continue
		if (!TrieGetString(g_lsgMkKeys[m], key, t, charsmax(t)))
			lsg_mk_default(g_lsgMkKind[m], key, t)
		break
	}}
	return t
}}

stock lsg_mk_default(k, const key[], t[64])
{{
{defaults}	#pragma unused k
	#pragma unused key
	#pragma unused t
}}
"##
    )
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
// W1: the utility nodes (w1_nodes.rs, w1_helpers.pawn)
#[path = "w1_nodes.rs"]
mod w1;

/// The plugin's command check, from the same lists as the app's (`graph::cmd_rules`).
pub fn safe_cmd_stock() -> String {
    use super::graph::cmd_rules::*;
    let arr = |l: &[&str]| l.iter().map(|w| format!("\"{w}\"")).collect::<Vec<_>>().join(", ");
    format!(r#"// one server command, only an allowed one (graph::cmd_rules: the app checks the same). The
// engine splits words on spaces and on {{ }} ( ) ' : , and drops quotes: none of those may be in
// it, so the first word checked is the one that runs. amx: an AMX Mod X command instead.
stock bool:lsg_cmd_denied(const w[])
{{
	static const DENY_WORDS[][] = {{ {dw} }}
	static const DENY_PARTS[][] = {{ {dp} }}
	for (new i = 0; i < sizeof DENY_WORDS; i++)
		if (equal(w, DENY_WORDS[i]))
			return true
	for (new i = 0; i < sizeof DENY_PARTS; i++)
		if (contain(w, DENY_PARTS[i]) >= 0)
			return true
	return equal(w, "logaddress", 10) != 0
}}

stock bool:lsg_server_word(const w[])
{{
	static const WORDS[][] = {{ {sw} }}
	static const PREFIXES[][] = {{ {sp} }}
	if (lsg_cmd_denied(w))
		return false
	for (new i = 0; i < sizeof WORDS; i++)
		if (equal(w, WORDS[i]))
			return true
	for (new i = 0; i < sizeof PREFIXES; i++)
		if (equal(w, PREFIXES[i], strlen(PREFIXES[i])) && strlen(w) > strlen(PREFIXES[i]))
			return true
	return false
}}

stock bool:lsg_safe_cmd(const cmd[], amx)
{{
	lsg_cmd_ok = false
	new c[128], w[{wl}], w2[{wl}], n, k
	copy(c, charsmax(c), cmd)
	trim(c)
	if (!c[0] || strlen(c) > 120)
		return false
	for (new i = 0; c[i]; i++)
		if (!isalnum(c[i]) && c[i] != ' ' && c[i] != '_' && c[i] != '.' && c[i] != '-' && c[i] != '+' && c[i] != '!' && c[i] != '?' && c[i] != '@' && c[i] != '#')
			return false
	// the first two words, lower case (a word that doesn't fit is refused: the same whole words
	// as the app's check, WORD_MAX = {wm})
	for (k = 0; c[k] == ' '; k++) {{}}
	for (n = 0; c[k] && c[k] != ' ' && n < charsmax(w); k++)
		w[n++] = tolower(c[k])
	w[n] = 0
	if (c[k] && c[k] != ' ')
		return false
	for (; c[k] == ' '; k++) {{}}
	for (n = 0; c[k] && c[k] != ' ' && n < charsmax(w2); k++)
		w2[n++] = tolower(c[k])
	w2[n] = 0
	if (c[k] && c[k] != ' ')
		return false
	if (amx)
	{{
		static const AMX_WORDS[][] = {{ {ab} }}
		new found
		for (new i = 0; i < sizeof AMX_WORDS; i++)
			if (equal(w, AMX_WORDS[i]))
				found = 1
		if (!found)
			return false
		if (equal(w, "amx_cvar") && !lsg_server_word(w2))
			return false
		if (equal(w, "amx_map") && (!w2[0] || strlen(w2) > 31 || !is_map_valid(w2)))
			return false
	}}
	else if (equal(w, "amx_", 4) || !lsg_server_word(w))
		return false
	server_cmd("%s", c)
	lsg_cmd_ok = true
	return true
}}
"#, dw = arr(&DENY_WORDS), dp = arr(&DENY_PARTS), sw = arr(&SERVER_WORDS), sp = arr(&SERVER_PREFIXES), ab = arr(&AMX_WORDS), wm = WORD_MAX, wl = WORD_MAX + 1)
}

/// The game's own HUD status icons (cstrike/sprites/hud.txt; Condition Zero uses the same file).
pub const STATUS_ICONS: [&str; 25] = [
    "buyzone", "c4", "defuser", "rescue", "escape", "vipsafety", "stopwatch", "smallskull", "smallc4", "smallvip", "hostage", "dollar", "cross", "item_healthkit", "item_battery", "item_longjump", "dmg_poison", "dmg_bio", "dmg_chem", "dmg_drown", "dmg_gas", "dmg_cold", "dmg_heat", "dmg_rad", "dmg_shock",
];
/// gm_components' component kinds (its classnames), for Is Component.
pub const COMP_KINDS: [&str; 25] = [
    "ls_capture_point", "ls_extraction_zone", "ls_checkpoint", "ls_shop", "ls_barricade", "ls_trigger_trap", "ls_loot_container", "ls_supply_drop", "ls_supply_crate", "ls_perk_machine", "ls_upgrade_bench", "ls_mystery_box", "ls_meeting_point", "ls_moving_platform", "ls_timed_door", "ls_wave_spawner", "ls_safe_zone", "ls_zone_shrinker", "ls_damage_zone", "ls_weapon_spawner", "ls_respawn_point", "ls_player_class", "ls_pickup", "ls_npc", "ls_interactable",
];

/// The kill feed's weapon icons (hud.txt's d_<name> sprites).
pub const KILL_ICONS: [&str; 28] = [
    "knife", "ak47", "m4a1", "awp", "deagle", "usp", "glock18", "p228", "fiveseven", "elite", "scout", "g3sg1", "sg550", "sg552", "aug", "famas", "galil", "mp5navy", "tmp", "mac10", "ump45", "p90", "m3", "xm1014", "m249", "grenade", "skull", "tracktrain",
];

/// Helpers every plugin gets (stocks: the ones a graph doesn't use cost nothing).
const HELPERS: &str = r##"// ---- the game and the server (Game, World, Players, Entities, Effects, Advanced nodes) ----
stock g_lsgRounds, g_lsgScore[3], bool:g_lsgFreeze, Float:g_lsgGagTill[33], g_lsgFw = -1, bool:lsg_cmd_ok, g_lsgSprExp, g_lsgSprBeam, Trie:g_lsgHave = Invalid_Trie

// a model or sound the graph names, loaded only if the file is there (on the server or in the
// game's folders): a missing one would stop the map loading. What uses it does without.
// (a mode loads at most 160 files of its own: the game's 512 models and 512 sounds are for
// everything on the server; limits.rs PRECACHE_MAX)
stock g_lsgPreN
stock bool:lsg_pre_room(const p[])
{
	if (g_lsgPreN < 160)
	{
		g_lsgPreN++
		return true
	}
	log_amx("[%s] %s not loaded: the mode has loaded 160 files of its own already", LSG_MODE, p)
	return false
}

stock lsg_pre_model(const p[])
{
	if (!file_exists(p, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, p)
		return 0
	}
	if (!lsg_pre_room(p))
		return 0
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_model(p)
}

stock lsg_pre_sound(const p[])
{
	new full[128]
	formatex(full, charsmax(full), "sound/%s", p)
	if (!file_exists(full, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, full)
		return 0
	}
	if (!lsg_pre_room(full))
		return 0
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_sound(p)
}

// room for more messages this server frame (temp entities, HUD, chat): past the budget (the
// Effect messages a frame limit) the effect is skipped, said in the log once a second; a unicast
// (to a player) counts against that player too, at half the budget
stock Float:g_lsgMsgAt, g_lsgMsgN, g_lsgMsgP[33], Float:g_lsgMsgSaid, g_lsgMsgLim = 48
stock bool:lsg_msg_room(const what[], cost = 1, to = 0, lim = 0)
{
	if (lim <= 0)
		lim = g_lsgMsgLim
	new Float:now = get_gametime()
	if (now != g_lsgMsgAt)
	{
		g_lsgMsgAt = now
		g_lsgMsgN = 0
		arrayset(g_lsgMsgP, 0, sizeof g_lsgMsgP)
	}
	if (g_lsgMsgN + cost > lim || (to >= 1 && to <= 32 && g_lsgMsgP[to] + cost > lim / 2))
	{
		if (now - g_lsgMsgSaid >= 1.0)
		{
			g_lsgMsgSaid = now
			log_amx("[%s] %s skipped: too many effects this frame", LSG_MODE, what)
		}
		return false
	}
	g_lsgMsgN += cost
	if (to >= 1 && to <= 32)
		g_lsgMsgP[to] += cost
	return true
}

// room for one more entity: free slots are kept for the game (it crashes with "no free edicts");
// a mode's spawn that would eat into them is skipped, and the server log says what and why
#define LSG_EDICT_RESERVE 96
stock bool:lsg_ent_room(const what[])
{
	if (engfunc(EngFunc_NumberOfEntities) < global_get(glb_maxEntities) - LSG_EDICT_RESERVE)
		return true
	log_amx("[%s] %s skipped: the server is out of entity slots", LSG_MODE, what)
	return false
}

// was it loaded (lsg_pre_model / lsg_pre_sound)?
stock bool:lsg_have(const p[])
	return g_lsgHave != Invalid_Trie && TrieKeyExists(g_lsgHave, p)

stock lsg_map_name()
{
	new m[32]
	get_mapname(m, charsmax(m))
	return m
}

// cvars no mode may read (passwords, rcon, where files and logs go)
stock bool:lsg_secret(const c[])
{
	return containi(c, "password") >= 0 || containi(c, "rcon") >= 0 || equali(c, "sv_downloadurl") || equali(c, "log", 3) || containi(c, "_file") >= 0 || equali(c, "amx_sql", 7) || equali(c, "hostport") || equali(c, "ip")
}

stock lsg_cvar_text(const c[])
{
	new t[64]
	if (!lsg_secret(c) && get_cvar_pointer(c))
		get_cvar_string(c, t, charsmax(t))
	return t
}

stock Float:lsg_cvar_num(const c[])
	return !lsg_secret(c) && get_cvar_pointer(c) ? get_cvar_float(c) : 0.0

// 0 all, 1 people, 2 bots, 3 T, 4 CT, 5 spectators, 6 alive
stock lsg_count(which)
{
	new n
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p))
			continue
		new t = get_user_team(p)
		switch (which)
		{
			case 1: if (!is_user_bot(p)) n++
			case 2: if (is_user_bot(p)) n++
			case 3: if (t == 1) n++
			case 4: if (t == 2) n++
			case 5: if (t != 1 && t != 2) n++
			case 6: if (is_user_alive(p)) n++
			default: n++
		}
	}
	return n
}

stock lsg_clock(const fmt[])
{
	new t[40]
	get_time(fmt, t, charsmax(t))
	return t
}

stock bool:lsg_helmet(id)
{
	if (!lsg_ok(id))
		return false
	new CsArmorType:t
	cs_get_user_armor(id, t)
	return t == CS_ARMOR_VESTHELM
}

stock lsg_held(id)
{
	new w[24]
	if (lsg_ok(id) && is_user_alive(id))
	{
		new clip, ammo, wid = get_user_weapon(id, clip, ammo)
		if (wid > 0)
			get_weaponname(wid, w, charsmax(w))
		replace(w, charsmax(w), "weapon_", "")
	}
	return w
}

stock lsg_ammo(id, reserve)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return 0
	new clip, ammo
	get_user_weapon(id, clip, ammo)
	return reserve ? ammo : max(clip, 0)
}

stock lsg_ping(id)
{
	new ping, loss
	if (lsg_ok(id))
		get_user_ping(id, ping, loss)
	return ping
}

stock lsg_steam_id(id)
{
	new s[44]
	if (lsg_ok(id) && !is_user_bot(id))
	{
		get_user_authid(id, s, charsmax(s))
		if (!equal(s, "STEAM_", 6))
			s[0] = 0
	}
	return s
}

// the connected player with this Steam ID: case and the universe digit (STEAM_0 / STEAM_1) aside,
// as the app's roles compare them (only the X:Y after STEAM_n: counts)
stock lsg_by_steam(const id[])
{
	if (!equali(id, "STEAM_", 6) || !id[6] || id[7] != ':')
		return 0
	new s[44]
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		get_user_authid(p, s, charsmax(s))
		if (equali(s, "STEAM_", 6) && s[6] && s[7] == ':' && equali(s[8], id[8]))
			return p
	}
	return 0
}

// a Loner-Strike supporter: access flag "s" (gm_vip's SUPPORTER_FLAG; the app writes it from the
// roles sync). Not bots or HLTV, and not the listen-server host, who has every flag
stock bool:lsg_supporter(id)
{
	if (!lsg_ok(id) || is_user_bot(id) || is_user_hltv(id))
		return false
	if (id == 1 && !is_dedicated_server())
		return false
	return (get_user_flags(id) & ADMIN_LEVEL_G) != 0
}

stock lsg_by_name(const name[])
	return name[0] ? max(find_player("bl", name), 0) : 0

stock lsg_slot_ok(i)
	return lsg_ok(i) ? i : 0

stock Float:lsg_pevf(id, field)
{
	new Float:f
	if (lsg_ok(id))
		pev(id, field, f)
	return f
}

stock lsg_model(id)
{
	new m[32]
	if (lsg_ok(id))
		cs_get_user_model(id, m, charsmax(m))
	return m
}

stock lsg_eclass(e)
{
	new t[32]
	if (e > 0 && pev_valid(e))
		pev(e, pev_classname, t, charsmax(t))
	return t
}

stock lsg_ename(e)
{
	new t[32]
	if (e > 0 && pev_valid(e))
		pev(e, pev_targetname, t, charsmax(t))
	return t
}

stock bool:lsg_plugin_on(const file[])
	return is_plugin_loaded(file, true) != -1

// (a map name is letters, digits, _ - . only, and the server must have it: nothing else reaches the console)
stock lsg_change_map(const map[])
{
	if (!map[0])
		return
	for (new i = 0; map[i]; i++)
		if (!isalnum(map[i]) && map[i] != '_' && map[i] != '-' && map[i] != '.')
			return
	if (is_map_valid(map))
		server_cmd("changelevel %s", map)
}

// text that goes into a console command's quotes: letters, digits, spaces and . , ! ? - ' ( ) only
// (no quote, semicolon, new line or other control character can end the quotes or add a command)
stock lsg_clean(const src[], dst[], len)
{
	new j
	for (new i = 0; src[i] && j < len; i++)
	{
		new c = src[i]
		if (isalnum(c) || c == ' ' || c == '.' || c == ',' || c == '!' || c == '?' || c == '-' || c == 39 || c == '(' || c == ')')
			dst[j++] = c
	}
	dst[j] = 0
}

stock lsg_slap(id, dmg)
{
	if (lsg_ok(id) && is_user_alive(id))
		user_slap(id, clamp(dmg, 0, 100))
}

stock lsg_fade(id, Float:secs, r, g, b, alpha)
{
	if (!lsg_ok(id))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("ScreenFade")
	new units = clamp(floatround(secs * 4096.0), 0, 65535)
	message_begin(MSG_ONE_UNRELIABLE, msg, _, id)
	write_short(units)
	write_short(units)
	write_short(0)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(alpha)
	message_end()
}

stock lsg_shake(id, Float:amp, Float:secs)
{
	if (!lsg_ok(id))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("ScreenShake")
	message_begin(MSG_ONE_UNRELIABLE, msg, _, id)
	write_short(clamp(floatround(amp * 4096.0), 0, 65535))
	write_short(clamp(floatround(secs * 4096.0), 0, 65535))
	write_short(clamp(floatround(amp * 4096.0), 0, 65535))
	message_end()
}

stock lsg_godmode(id, on)
{
	if (lsg_ok(id))
		set_pev(id, pev_takedamage, on ? DAMAGE_NO : DAMAGE_AIM)
}

stock lsg_noclip(id, on)
{
	if (lsg_ok(id) && is_user_alive(id))
		set_pev(id, pev_movetype, on ? MOVETYPE_NOCLIP : MOVETYPE_WALK)
}

// 0 normal, 1 see-through by amount, 2 invisible
stock lsg_render(id, mode, amount)
{
	if (!lsg_ok(id))
		return
	set_pev(id, pev_rendermode, mode ? kRenderTransAlpha : kRenderNormal)
	set_pev(id, pev_renderamt, mode == 2 ? 0.0 : (mode ? float(amount) : 255.0))
}

stock lsg_set_ammo(id, clip, reserve)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	// (the held weapon from the active item: get_user_weapon says 0 while a gun is being drawn,
	// and the reserve can't be set for weapon 0)
	new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	new wid = w > 0 && pev_valid(w) ? cs_get_weapon_id(w) : 0
	if (wid > 0 && wid != CSW_KNIFE)
	{
		cs_set_weapon_ammo(w, clamp(clip, 0, 200))
		cs_set_user_bpammo(id, wid, clamp(reserve, 0, 255))
	}
}

stock lsg_give_ammo(id, n)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	new wid = w > 0 && pev_valid(w) ? cs_get_weapon_id(w) : 0
	if (wid > 0 && wid != CSW_KNIFE)
		cs_set_user_bpammo(id, wid, clamp(cs_get_user_bpammo(id, wid) + n, 0, 255))
}

stock lsg_take_weapon(id, const wname[])
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new wid = get_weaponid(wname), e = -1
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", wname)) > 0)
		if (pev(e, pev_owner) == id)
			break
	if (e <= 0)
		return
	if (get_user_weapon(id) == wid)
		ExecuteHamB(Ham_Weapon_RetireWeapon, e)
	if (!ExecuteHamB(Ham_RemovePlayerItem, id, e))
		return
	ExecuteHamB(Ham_Item_Kill, e)
	set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << wid))
}

stock lsg_switch_weapon(id, const wname[])
{
	if (lsg_ok(id) && is_user_alive(id) && user_has_weapon(id, get_weaponid(wname)))
		engclient_cmd(id, wname)
}

stock lsg_set_frags(id, n)
{
	if (!lsg_ok(id))
		return
	set_pev(id, pev_frags, float(n))
	static msg
	if (!msg)
		msg = get_user_msgid("ScoreInfo")
	message_begin(MSG_ALL, msg)
	write_byte(id)
	write_short(n)
	write_short(cs_get_user_deaths(id))
	write_short(0)
	write_short(get_user_team(id))
	message_end()
}

stock lsg_set_deaths(id, n)
{
	if (lsg_ok(id))
		cs_set_user_deaths(id, max(n, 0))
}

stock lsg_to_spec(id)
{
	if (!lsg_ok(id))
		return
	if (is_user_alive(id))
		user_kill(id, 1)
	cs_set_user_team(id, CS_TEAM_SPECTATOR)
}

stock lsg_kick(id, const reason[])
{
	if (!lsg_ok(id))
		return
	new r[61]
	lsg_clean(reason, r, charsmax(r))
	server_cmd("kick #%d ^"%s^"", get_user_userid(id), r)
}

stock lsg_team_chat(team, const fmt[], any:...)
{
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 3)
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && get_user_team(p) == team)
			client_print(p, print_chat, "%s", msg)
}

stock lsg_admins_chat(const fmt[], any:...)
{
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 2)
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && (get_user_flags(p) & ADMIN_KICK))
			client_print(p, print_chat, "[Admins] %s", msg)
}

stock lsg_remove(e)
{
	if (e > MaxClients && pev_valid(e))
		engfunc(EngFunc_RemoveEntity, e)
}

stock lsg_ent_visible(e, on)
{
	if (e > 0 && pev_valid(e))
		set_pev(e, pev_effects, on ? (pev(e, pev_effects) & ~EF_NODRAW) : (pev(e, pev_effects) | EF_NODRAW))
}

stock lsg_sound_to(id, const snd[])
{
	if (lsg_ok(id))
		client_cmd(id, "spk ^"%s^"", snd)
}

stock lsg_fire_event(const name[], a, b)
{
	if (g_lsgFw < 0)
		g_lsgFw = CreateMultiForward("ls_studio_event", ET_IGNORE, FP_STRING, FP_CELL, FP_CELL)
	new ret
	ExecuteForward(g_lsgFw, ret, name, a, b)
}

// a public function in another plugin with up to two numbers; 1 if it was called
stock lsg_call(const plugin[], const func[], nargs, a, b, &r)
{
	r = 0
	if (callfunc_begin(func, plugin) != 1)
		return 0
	if (nargs >= 1)
		callfunc_push_int(a)
	if (nargs >= 2)
		callfunc_push_int(b)
	r = callfunc_end()
	return 1
}

stock lsg_hitgroup(v, bits)
{
	new t[12]
	if (!lsg_ok(v))
		return t
	// (only a bullet or the knife hits a place; the game keeps the last one, so a grenade or a
	// fall after a headshot would say head again)
	switch (bits & DMG_BULLET ? get_ent_data(v, "CBaseMonster", "m_LastHitGroup") : 0)
	{
		case HIT_HEAD: copy(t, charsmax(t), "head")
		case HIT_CHEST: copy(t, charsmax(t), "chest")
		case HIT_STOMACH: copy(t, charsmax(t), "stomach")
		case HIT_LEFTARM, HIT_RIGHTARM: copy(t, charsmax(t), "arm")
		case HIT_LEFTLEG, HIT_RIGHTLEG: copy(t, charsmax(t), "leg")
		default: copy(t, charsmax(t), "body")
	}
	return t
}

// the weapon that did the damage: a grenade by what hit (the attacker may hold a gun by then),
// else what the attacker holds; empty when no player did it. (Its own body: a stock that returns
// another stock's text fails at run time with "memory access".)
stock lsg_wpn_of(a, inflictor)
{
	new w[24]
	if (inflictor > MaxClients && pev_valid(inflictor))
	{
		pev(inflictor, pev_classname, w, charsmax(w))
		if (equal(w, "grenade"))
		{
			copy(w, charsmax(w), "hegrenade")
			return w
		}
		w[0] = 0
	}
	if (lsg_ok(a) && is_user_alive(a))
	{
		new clip, ammo, wid = get_user_weapon(a, clip, ammo)
		if (wid > 0)
			get_weaponname(wid, w, charsmax(w))
		replace(w, charsmax(w), "weapon_", "")
	}
	return w
}

// Give Weapon with a name wired in: only a weapon the game has (ak47, deagle, knife...)
stock lsg_give_named(id, const name[], ammo)
{
	new w[32]
	formatex(w, charsmax(w), "weapon_%s", name)
	if (equal(name, "weapon_", 7) || !get_weaponid(w))
		return 0
	return ls_give_weapon(id, w, ammo)
}

stock Float:lsg_dist(a, b)
{
	if (!lsg_ok(a) || !lsg_ok(b))
		return 0.0
	new Float:x[3], Float:y[3]
	pev(a, pev_origin, x)
	pev(b, pev_origin, y)
	return get_distance_f(x, y)
}

// what's after the first word ("/give 5" -> "5")
stock lsg_after_word(s[])
{
	new i
	while (s[i] && s[i] != ' ')
		i++
	while (s[i] == ' ')
		i++
	copy(s, 63, s[i])
}

stock lsg_hook_weapons(const fn[], Ham:h)
{
	static const W[][] = { "weapon_p228", "weapon_scout", "weapon_hegrenade", "weapon_xm1014", "weapon_mac10", "weapon_aug", "weapon_smokegrenade", "weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas", "weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3", "weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_flashbang", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_knife", "weapon_p90" }
	for (new i = 0; i < sizeof W; i++)
		RegisterHam(h, W[i], fn, 1)
}

stock lsg_prop(at, const model[], solid)
{
	new Float:o[3]
	if (!lsg_ent_origin(at, o))
		return 0
	// (a model that wasn't there when the map loaded: no prop)
	if (!lsg_have(model))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_prop")
	engfunc(EngFunc_SetModel, e, model)
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_solid, solid ? SOLID_BBOX : SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NONE)
	if (solid)
		engfunc(EngFunc_SetSize, e, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 32.0})
	return e
}

// the round table's kind of round (0 normal, 1 boss, 2 pack) and its type
stock lsg_wave_kind()
{
	new t[32]
	return ls_components_running() ? ls_wave_kind(t, charsmax(t)) : 0
}

stock lsg_wave_type()
{
	new t[32]
	if (ls_components_running())
		ls_wave_kind(t, charsmax(t))
	return t
}

// a drop-in over an entity's place
stock lsg_drop_in(id, ent)
{
	new Float:o[3]
	if (!lsg_ent_origin(ent, o))
		return ls_drop_in(id)
	return ls_drop_in(id, o)
}

stock Float:lsg_ehp(e)
	return e > MaxClients && pev_valid(e) ? entity_get_float(e, EV_FL_health) : 0.0

// ---- bot hints (ls_mode, through gm_components) ----
stock bool:lsg_hint(const line[])
	return ls_components_running() && ls_comp_bots_hint(line) != 0

// who a hint is for: 0 all, 1 ct, 2 t, 3 a player, 4 @tag
stock lsg_who(code, p, const tag[])
{
	new w[40]
	switch (code)
	{
		case 1: copy(w, charsmax(w), "ct")
		case 2: copy(w, charsmax(w), "t")
		case 3: formatex(w, charsmax(w), "%d", lsg_ok(p) ? p : 0)
		case 4: formatex(w, charsmax(w), "@%s", tag)
		default: copy(w, charsmax(w), "all")
	}
	return w
}

// an entity's place as "x y z" (a brush entity: its middle)
stock lsg_ptext(ent)
{
	new t[48], Float:o[3]
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
		{
			new Float:mins[3], Float:maxs[3]
			pev(ent, pev_absmin, mins)
			pev(ent, pev_absmax, maxs)
			for (new k = 0; k < 3; k++)
				o[k] = (mins[k] + maxs[k]) / 2.0
		}
	}
	formatex(t, charsmax(t), "%.0f %.0f %.0f", o[0], o[1], o[2])
	return t
}

// the zone's next move: 0 seconds (-1 none), 1 the next radius, 2 closing now
stock Float:lsg_zone_next(ent, what)
{
	if (!ls_components_running() || ent <= 0)
		return what == 0 ? -1.0 : 0.0
	new Float:c[3], Float:r, closing
	new Float:secs = ls_zone_next(ent, c, r, closing)
	switch (what)
	{
		case 1: return r
		case 2: return closing ? 1.0 : 0.0
	}
	return secs
}

// is ent a component of this kind ("": any component)? gm_components' natives raise an error on
// another kind, which ends the whole flow; the node checks first and says so instead
stock bool:lsg_kind_ok(ent, const kind[])
{
	new k[24]
	if (ent <= 0 || !ls_components_running() || !ls_comp_kind(ent, k, charsmax(k)))
		return false
	return !kind[0] || equal(k, kind)
}

// the safe zone a Zone pin means: a Safe zone itself, or the one a Shrinking zone drives (0: neither)
stock lsg_zone_of(ent)
{
	new k[24], name[32], tn[32], z
	if (ent <= 0 || !ls_comp_kind(ent, k, charsmax(k)))
		return 0
	if (equal(k, "ls_safe_zone"))
		return ent
	if (!equal(k, "ls_zone_shrinker"))
		return 0
	ls_comp_get(ent, "zone", name, charsmax(name))
	while ((z = ls_comp_find("ls_safe_zone", z)) > 0)
	{
		pev(z, pev_targetname, tn, charsmax(tn))
		if (!name[0] || equal(tn, name))
			return z
	}
	return 0
}

// counted now (the brain's ls_creature_alive is only brought up to date a moment later); once a
// server frame (one scan however often it's read), again after the mode makes a creature
stock Float:g_lsgCrAt = -1.0, g_lsgCrN
stock lsg_creatures_alive()
{
	if (get_gametime() == g_lsgCrAt)
		return g_lsgCrN
	new n, e = MaxClients, Float:hp
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		pev(e, pev_health, hp)
		if (pev(e, pev_deadflag) == DEAD_NO && hp > 0.0)
			n++
	}
	g_lsgCrAt = get_gametime()
	g_lsgCrN = n
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

// a bot's nature as the bot brain plays it (W2: pev_vuser2 = skill, aggression, teamwork 0-1;
// pev_vuser3 = greed 0-1, playstyle, trait bits); people read 0
stock Float:lsg_bot_nat(id, which)
{
	if (!lsg_ok(id) || !is_user_bot(id))
		return 0.0
	new Float:v[3]
	pev(id, which == 3 ? pev_vuser3 : pev_vuser2, v)
	return floatclamp((which == 3 ? v[0] : v[which]) * 100.0, 0.0, 100.0)
}

stock bool:lsg_bot_trait(id, bit)
{
	if (!lsg_ok(id) || !is_user_bot(id))
		return false
	new Float:v[3]
	pev(id, pev_vuser3, v)
	return (floatround(v[2]) & bit) != 0
}

stock lsg_bot_style(id)
{
	static const S[][] = { "rusher", "roamer", "lurker", "careful", "camper", "awper", "knifer", "nader", "ninja", "objective", "weirdgun" }
	new t[16]
	if (lsg_ok(id) && is_user_bot(id))
	{
		new Float:v[3]
		pev(id, pev_vuser3, v)
		new k = floatround(v[1])
		if (k >= 0 && k < sizeof S)
			copy(t, charsmax(t), S[k])
	}
	return t
}

// a follow with a personality influence (0-100: 50 as usual; W2's ls_mode_ver 2)
stock lsg_bot_follow(bot, who, secs, influence)
{
	if (ls_is_bot(bot) && is_user_connected(who))
		server_cmd("ls_bot %d follow %d %d %d", bot, who, secs, influence)
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

// a number as text: whole numbers as they are, others with two decimals (0.5, not 1)
stock lsg_ntext(Float:n)
{
	new t[24]
	if (floatfract(n) == 0.0 && floatabs(n) < 1000000.0)
		formatex(t, charsmax(t), "%d", floatround(n))
	else
		formatex(t, charsmax(t), "%.2f", n)
	return t
}

stock lsg_name(id)
{
	new name[32]
	if (id >= 1 && id <= MaxClients && is_user_connected(id))
		get_user_name(id, name, charsmax(name))
	else
		copy(name, charsmax(name), "nobody")
	return name
}

// (a scan of every entity: the answer is kept for the rest of the server frame, so a Find in a
// loop or read many times costs one scan; PerfAudit #9)
stock lsg_find(const name[])
{
	static Trie:seen = Invalid_Trie, Float:at = -1.0
	if (!name[0])
		return 0
	new Float:now = get_gametime(), e
	if (seen == Invalid_Trie)
		seen = TrieCreate()
	if (now != at)
	{
		TrieClear(seen)
		at = now
	}
	if (TrieGetCell(seen, name, e) && (!e || pev_valid(e)))
		return e
	e = max(find_ent_by_tname(-1, name), 0)
	TrieSetCell(seen, name, e)
	return e
}

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

/// Status Icon, Objective Marker and Kill Feed Line's helpers: only in a plugin that uses one
const UI_HELPERS: &str = r##"// ---- Status Icon, Objective Marker, Kill Feed Line ----
stock g_lsgIcon[33][16]
stock lsg_icon_send(to, status, const name[], r, g, b)
{
	if ((to && !is_user_connected(to)) || !lsg_msg_room("Status Icon", 1, to))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("StatusIcon")
	message_begin(to ? MSG_ONE : MSG_ALL, msg, _, to)
	write_byte(status)
	write_string(name)
	if (status)
	{
		write_byte(r)
		write_byte(g)
		write_byte(b)
	}
	message_end()
}

// to 0: everyone; death: noted, so it's taken away when they die
stock lsg_icon(to, status, const name[], r, g, b, death)
{
	if (to && !lsg_ok(to))
		return
	lsg_icon_send(to, status, name, r, g, b)
	if (!death)
		return
	for (new p = to ? to : 1; p <= (to ? to : MaxClients); p++)
		if (is_user_connected(p))
			copy(g_lsgIcon[p], 15, status ? name : "")
}

stock lsg_icon_clear(id)
{
	if (id < 1 || id > MaxClients || !g_lsgIcon[id][0])
		return
	lsg_icon_send(id, 0, g_lsgIcon[id], 0, 0, 0)
	g_lsgIcon[id][0] = 0
}

// markers: a sprite on an info_target, moved by one task to what it follows (no think each frame)
#define LSG_MKS 256
stock g_lsgMk[LSG_MKS], g_lsgMkF[LSG_MKS], Float:g_lsgMkH[LSG_MKS], g_lsgMkWho[2048], g_lsgMkFor[2048]

stock lsg_marker(const spr[], follow, const Float:at[3], hasAt, Float:h, Float:scale, r, g, b, pulse, who, forp, lim = 32)
{
	// (a sprite that wasn't there when the map loaded: no marker)
	if (!lsg_have(spr) || (follow <= 0 && !hasAt) || (follow > 0 && !pev_valid(follow)))
		return 0
	new slot = -1
	for (new k = 0; k < min(lim, LSG_MKS); k++)
	{
		if (g_lsgMk[k] && !pev_valid(g_lsgMk[k]))
			g_lsgMk[k] = 0
		if (!g_lsgMk[k])
		{
			slot = k
			break
		}
	}
	if (slot < 0 || !lsg_ent_room("Objective Marker"))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_marker")
	engfunc(EngFunc_SetModel, e, spr)
	set_pev(e, pev_movetype, MOVETYPE_NOCLIP)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_rendermode, kRenderTransAdd)
	set_pev(e, pev_renderamt, 255.0)
	new Float:c[3]
	c[0] = float(r)
	c[1] = float(g)
	c[2] = float(b)
	set_pev(e, pev_rendercolor, c)
	set_pev(e, pev_scale, scale)
	if (pulse)
		set_pev(e, pev_renderfx, kRenderFxPulseSlow)
	new Float:o[3]
	if (follow > 0)
		pev(follow, pev_origin, o)
	else
		o = at
	o[2] += h
	engfunc(EngFunc_SetOrigin, e, o)
	g_lsgMk[slot] = e
	g_lsgMkF[slot] = follow
	g_lsgMkH[slot] = h
	g_lsgMkWho[e] = who
	g_lsgMkFor[e] = forp
	return e
}

stock lsg_mk_free(k)
{
	new e = g_lsgMk[k]
	g_lsgMk[k] = 0
	if (e > 0 && e < 2048)
	{
		g_lsgMkWho[e] = 0
		g_lsgMkFor[e] = 0
	}
	if (e > 0 && pev_valid(e))
		remove_entity(e)
}

stock lsg_mk_follow()
{
	for (new k = 0; k < LSG_MKS; k++)
	{
		new e = g_lsgMk[k], f = g_lsgMkF[k]
		if (!e)
			continue
		if (!pev_valid(e))
		{
			g_lsgMk[k] = 0
			continue
		}
		if (f <= 0)
			continue
		// what it was on is gone (or the player left): so is the marker
		if (!pev_valid(f) || (f <= MaxClients && !is_user_connected(f)))
		{
			lsg_mk_free(k)
			continue
		}
		// (a dead player's marker waits, out of sight)
		new fx = pev(e, pev_effects)
		if (f <= MaxClients && !is_user_alive(f))
		{
			set_pev(e, pev_effects, fx | EF_NODRAW)
			continue
		}
		set_pev(e, pev_effects, fx & ~EF_NODRAW)
		new Float:o[3]
		pev(f, pev_origin, o)
		o[2] += g_lsgMkH[k]
		engfunc(EngFunc_SetOrigin, e, o)
	}
}

// 0 or a marker: that one, or all of them
stock lsg_marker_remove(m)
{
	for (new k = 0; k < LSG_MKS; k++)
		if (g_lsgMk[k] && (!m || g_lsgMk[k] == m))
			lsg_mk_free(k)
}

stock bool:lsg_mk_sees(ent, host)
{
	new w = g_lsgMkWho[ent]
	if (w == 3)
		return host == g_lsgMkFor[ent]
	return host >= 1 && host <= MaxClients && get_user_team(host) == w
}

// a line in the kill feed, to each person's game (one at a time: plugins that count deaths from
// the game's own DeathMsg, the mode's Player Killed among them, don't see it as a death)
stock lsg_killfeed(killer, victim, hs, const w[])
{
	if (!lsg_ok(victim))
		return 0
	if (!lsg_ok(killer))
		killer = 0
	static msg
	if (!msg)
		msg = get_user_msgid("DeathMsg")
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p) || !lsg_msg_room("Kill Feed Line", 1, p))
			continue
		message_begin(MSG_ONE, msg, _, p)
		write_byte(killer)
		write_byte(victim)
		write_byte(hs ? 1 : 0)
		write_string(w)
		message_end()
	}
	return 1
}"##;

/// The fun nodes' helpers (fire, trails, bursts, swaps, burying, the camera, a drunk view): only
/// in a plugin that uses one
const FUN_HELPERS: &str = r##"// ---- fun: fire, trails, bursts, swaps, burying, the camera, a drunk view ----
stock g_lsgSprFire, g_lsgSprSmoke, g_lsgSprBlood, g_lsgSprSpray
stock Float:g_lsgBurnTill[33], Float:g_lsgBurnDps[33], Float:g_lsgBurnNext[33], g_lsgBurnBy[33], g_lsgBurnFx[33]
stock bool:g_lsgBuried[33], g_lsgCam[33], g_lsgCamKeep[33], Float:g_lsgDrunkTill[33], Float:g_lsgDrunk[33], g_lsgFunN
#define LSG_TRAILS 256
stock g_lsgTrail[LSG_TRAILS], g_lsgTrailC[LSG_TRAILS][5], bool:g_lsgTrailStill[LSG_TRAILS]

stock lsg_burn(id, by, Float:secs, Float:dps)
{
	if (!ls_alive(id))
		return 0
	new Float:now = get_gametime()
	g_lsgBurnTill[id] = now + secs
	g_lsgBurnDps[id] = dps
	g_lsgBurnBy[id] = lsg_ok(by) ? by : 0
	if (g_lsgBurnNext[id] < now)
		g_lsgBurnNext[id] = now + 1.0
	// flames on them: a sprite that rides along (the engine moves it: MOVETYPE_FOLLOW)
	if (g_lsgSprFire && !(g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id])) && lsg_ent_room("Set On Fire's flames"))
	{
		new e = create_entity("info_target")
		if (e > 0)
		{
			set_pev(e, pev_classname, "ls_studio_fire")
			engfunc(EngFunc_SetModel, e, "sprites/fire.spr")
			set_pev(e, pev_movetype, MOVETYPE_FOLLOW)
			set_pev(e, pev_aiment, id)
			set_pev(e, pev_solid, SOLID_NOT)
			set_pev(e, pev_rendermode, kRenderTransAdd)
			set_pev(e, pev_renderamt, 255.0)
			g_lsgBurnFx[id] = e
		}
	}
	return 1
}

stock lsg_burn_off(id)
{
	if (id < 1 || id > 32)
		return
	g_lsgBurnTill[id] = 0.0
	if (g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id]))
		remove_entity(g_lsgBurnFx[id])
	g_lsgBurnFx[id] = 0
}

stock lsg_drunk(id, Float:secs, Float:strength)
{
	if (!ls_alive(id))
		return 0
	g_lsgDrunkTill[id] = get_gametime() + secs
	g_lsgDrunk[id] = strength
	return 1
}

stock lsg_drunk_off(id)
{
	if (g_lsgDrunkTill[id] > 0.0 && is_user_connected(id))
		set_pev(id, pev_punchangle, Float:{0.0, 0.0, 0.0})
	g_lsgDrunkTill[id] = 0.0
}

stock lsg_trail_send(k)
{
	new e = g_lsgTrail[k]
	if (!lsg_msg_room("Trail", 2))
		return
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_KILLBEAM)
	write_short(e)
	message_end()
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_BEAMFOLLOW)
	write_short(e)
	write_short(g_lsgSprBeam)
	write_byte(g_lsgTrailC[k][4])
	write_byte(g_lsgTrailC[k][3])
	write_byte(g_lsgTrailC[k][0])
	write_byte(g_lsgTrailC[k][1])
	write_byte(g_lsgTrailC[k][2])
	write_byte(200)
	message_end()
}

stock lsg_trail(e, on, r, g, b, width, life, lim = 32)
{
	if (e <= 0 || !pev_valid(e))
		return 0
	new k = -1
	for (new i = 0; i < LSG_TRAILS; i++)
		if (g_lsgTrail[i] == e)
			k = i
	if (!on)
	{
		if (k >= 0)
		{
			message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
			write_byte(TE_KILLBEAM)
			write_short(e)
			message_end()
			g_lsgTrail[k] = 0
		}
		return 1
	}
	for (new i = 0; i < min(lim, LSG_TRAILS) && k < 0; i++)
		if (!g_lsgTrail[i] || !pev_valid(g_lsgTrail[i]))
			k = i
	if (k < 0)
		return 0
	g_lsgTrail[k] = e
	g_lsgTrailC[k][0] = r
	g_lsgTrailC[k][1] = g
	g_lsgTrailC[k][2] = b
	g_lsgTrailC[k][3] = width
	g_lsgTrailC[k][4] = life
	g_lsgTrailStill[k] = false
	lsg_trail_send(k)
	return 1
}

// a trail dies with a teleport, a respawn or a long stop: sent again when they move once more
stock lsg_trail_check()
{
	for (new k = 0; k < LSG_TRAILS; k++)
	{
		new e = g_lsgTrail[k]
		if (!e)
			continue
		if (!pev_valid(e) || (e <= MaxClients && !is_user_connected(e)))
		{
			g_lsgTrail[k] = 0
			continue
		}
		new Float:vel[3]
		pev(e, pev_velocity, vel)
		if (vector_length(vel) < 1.0)
			g_lsgTrailStill[k] = true
		else if (g_lsgTrailStill[k])
		{
			g_lsgTrailStill[k] = false
			lsg_trail_send(k)
		}
	}
}

stock lsg_fun_ticks()
{
	new Float:now = get_gametime()
	lsg_view_tick(now)
	for (new id = 1; id <= MaxClients; id++)
	{
		if (g_lsgBurnTill[id] > 0.0)
		{
			if (!is_user_alive(id) || now >= g_lsgBurnTill[id] || pev(id, pev_waterlevel) >= 2)
				lsg_burn_off(id)
			else if (now >= g_lsgBurnNext[id])
			{
				g_lsgBurnNext[id] = now + 1.0
				new by = g_lsgBurnBy[id]
				if (by && !is_user_connected(by))
					by = 0
				if (g_lsgBurnDps[id] > 0.0)
					ExecuteHamB(Ham_TakeDamage, id, by ? by : 0, by ? by : 0, g_lsgBurnDps[id], DMG_BURN)
			}
		}
		if (g_lsgDrunkTill[id] > 0.0)
		{
			if (!is_user_alive(id) || now >= g_lsgDrunkTill[id])
				lsg_drunk_off(id)
			else
			{
				new Float:s = g_lsgDrunk[id], Float:pa[3]
				pa[0] = floatsin(now * 2.3, radian) * s * 2.0
				pa[1] = floatcos(now * 1.7, radian) * s * 2.0
				pa[2] = floatsin(now * 1.1, radian) * s
				set_pev(id, pev_punchangle, pa)
			}
		}
	}
	if (++g_lsgFunN >= 10)
	{
		g_lsgFunN = 0
		lsg_trail_check()
	}
}

// the hull a player has now fits at o (nothing solid there)
stock bool:lsg_fits(id, const Float:o[3])
{
	engfunc(EngFunc_TraceHull, o, o, IGNORE_MONSTERS, (pev(id, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN, id, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid)
}

stock lsg_swap(a, b)
{
	if (!ls_alive(a) || !ls_alive(b) || a == b || g_lsgBuried[a] || g_lsgBuried[b])
		return 0
	new Float:oa[3], Float:ob[3], Float:aa[3], Float:ab[3], Float:va[3], Float:vb[3]
	pev(a, pev_origin, oa)
	pev(b, pev_origin, ob)
	if (!lsg_fits(a, ob) || !lsg_fits(b, oa))
		return 0
	pev(a, pev_v_angle, aa)
	pev(b, pev_v_angle, ab)
	pev(a, pev_velocity, va)
	pev(b, pev_velocity, vb)
	engfunc(EngFunc_SetOrigin, a, ob)
	engfunc(EngFunc_SetOrigin, b, oa)
	set_pev(a, pev_angles, ab)
	set_pev(a, pev_v_angle, ab)
	set_pev(a, pev_fixangle, 1)
	set_pev(b, pev_angles, aa)
	set_pev(b, pev_v_angle, aa)
	set_pev(b, pev_fixangle, 1)
	set_pev(a, pev_velocity, vb)
	set_pev(b, pev_velocity, va)
	return 1
}

// chest deep: 30 units down, and no walking (they can still turn and shoot)
stock lsg_bury(id)
{
	if (!ls_alive(id) || g_lsgBuried[id])
		return 0
	new Float:o[3]
	pev(id, pev_origin, o)
	o[2] -= 30.0
	engfunc(EngFunc_SetOrigin, id, o)
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	set_pev(id, pev_movetype, MOVETYPE_NONE)
	g_lsgBuried[id] = true
	return 1
}

stock lsg_unbury(id)
{
	if (!lsg_ok(id) || !g_lsgBuried[id])
		return 0
	g_lsgBuried[id] = false
	if (!is_user_alive(id))
		return 1
	set_pev(id, pev_movetype, MOVETYPE_WALK)
	new Float:o[3]
	pev(id, pev_origin, o)
	o[2] += 30.0
	// (onto the floor, never into a wall: a little higher until they fit)
	for (new i = 0; i < 8 && !lsg_fits(id, o); i++)
		o[2] += 8.0
	engfunc(EngFunc_SetOrigin, id, o)
	return 1
}

// view: 0 first person, 1 third person, 2 the other one; reset: back to first person at death/respawn
stock lsg_camera(id, view, reset)
{
	if (!lsg_ok(id))
		return 0
	new third = view == 2 ? (g_lsgCam[id] ? 0 : 1) : view
	set_view(id, third ? CAMERA_3RDPERSON : CAMERA_NONE)
	g_lsgCam[id] = third
	g_lsgCamKeep[id] = reset ? 0 : 1
	return 1
}

stock lsg_cam_reapply(id)
{
	if (lsg_ok(id) && g_lsgCam[id])
		set_view(id, CAMERA_3RDPERSON)
}

stock lsg_fun_died(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_view_back(id)
	lsg_burn_off(id)
	lsg_drunk_off(id)
	g_lsgBuried[id] = false
	if (g_lsgCam[id] && !g_lsgCamKeep[id])
	{
		set_view(id, CAMERA_NONE)
		g_lsgCam[id] = 0
	}
}

stock lsg_fun_spawned(id)
{
	if (id < 1 || id > MaxClients || !is_user_alive(id))
		return
	g_lsgBuried[id] = false
	for (new k = 0; k < 32; k++)
		if (g_lsgTrail[k] == id)
			lsg_trail_send(k)
	if (g_lsgCam[id])
	{
		// (kept: the game may put the view back as they spawn, so it's set again a moment later)
		if (g_lsgCamKeep[id])
			set_task(0.2, "lsg_cam_again", LSG_TASK - 480 + id)
		else
		{
			set_view(id, CAMERA_NONE)
			g_lsgCam[id] = 0
		}
	}
}

stock lsg_fun_left(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_view_back(id)
	lsg_burn_off(id)
	g_lsgDrunkTill[id] = 0.0
	g_lsgBuried[id] = false
	g_lsgCam[id] = 0
	g_lsgCamKeep[id] = 0
	remove_task(LSG_TASK - 480 + id)
	for (new k = 0; k < 32; k++)
		if (g_lsgTrail[k] == id)
			g_lsgTrail[k] = 0
}

// effect: 0 sparks, 1 smoke, 2 blood, 3 firework, 4 implosion, 5 tar, 6 teleport, 7 lava, 8 a colored flash
stock lsg_burst(kind, const Float:o[3], count, r, g, b)
{
	// (smoke and blood need their sprites: not there when the map loaded, no burst)
	if ((kind == 1 && !g_lsgSprSmoke) || (kind == 2 && (!g_lsgSprBlood || !g_lsgSprSpray)))
		return
	for (new n = 0; n < count; n++)
	{
		message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
		switch (kind)
		{
			case 1:
			{
				write_byte(TE_SMOKE)
				lsg_coords(o)
				write_short(g_lsgSprSmoke)
				write_byte(20)
				write_byte(12)
			}
			case 2:
			{
				write_byte(TE_BLOODSPRITE)
				lsg_coords(o)
				write_short(g_lsgSprSpray)
				write_short(g_lsgSprBlood)
				write_byte(247)
				write_byte(12)
			}
			case 3:
			{
				write_byte(TE_EXPLOSION2)
				lsg_coords(o)
				write_byte(random_num(0, 200))
				write_byte(16)
			}
			case 4:
			{
				write_byte(TE_IMPLOSION)
				lsg_coords(o)
				write_byte(128)
				write_byte(20)
				write_byte(5)
			}
			case 5:
			{
				write_byte(TE_TAREXPLOSION)
				lsg_coords(o)
			}
			case 6:
			{
				write_byte(TE_TELEPORT)
				lsg_coords(o)
			}
			case 7:
			{
				write_byte(TE_LAVASPLASH)
				lsg_coords(o)
			}
			case 8:
			{
				write_byte(TE_DLIGHT)
				lsg_coords(o)
				write_byte(30)
				write_byte(r)
				write_byte(g)
				write_byte(b)
				write_byte(8)
				write_byte(30)
			}
			default:
			{
				write_byte(TE_SPARKS)
				lsg_coords(o)
			}
		}
		message_end()
		// (a firework's light, in its color)
		if (kind == 3)
		{
			message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
			write_byte(TE_DLIGHT)
			lsg_coords(o)
			write_byte(25)
			write_byte(r)
			write_byte(g)
			write_byte(b)
			write_byte(6)
			write_byte(20)
			message_end()
		}
	}
}

stock lsg_coords(const Float:o[3])
{
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
}

// View From: a camera entity each shot; players see through it (set_view) until it ends
stock g_lsgViewCam[33], Float:g_lsgViewTill[33], bool:g_lsgViewFreeze[33], g_lsgCamFollow[2048], Float:g_lsgCamOff[2048], Float:g_lsgCamStop[2048]

stock lsg_view_from(who, one, from, const Float:a[3], const Float:aa[3], hasA, const Float:b[3], const Float:ba[3], hasB, Float:off, Float:move, Float:secs, freeze)
{
	if (from <= 0 && !hasA)
		return 0
	if (from > 0 && !pev_valid(from))
		return 0
	if (!lsg_ent_room("View From"))
		return 0
	new cam = create_entity("info_target")
	if (cam <= 0)
		return 0
	set_pev(cam, pev_classname, "ls_studio_cam")
	engfunc(EngFunc_SetModel, cam, "models/w_c4.mdl")
	set_pev(cam, pev_rendermode, kRenderTransTexture)
	set_pev(cam, pev_renderamt, 0.0)
	set_pev(cam, pev_solid, SOLID_NOT)
	set_pev(cam, pev_movetype, MOVETYPE_NOCLIP)
	new Float:o[3], Float:ang[3]
	if (from > 0)
	{
		pev(from, pev_origin, o)
		o[2] += off
		pev(from, from <= MaxClients ? pev_v_angle : pev_angles, ang)
		g_lsgCamFollow[cam] = from
		g_lsgCamOff[cam] = off
	}
	else
	{
		o = a
		ang = aa
		g_lsgCamFollow[cam] = 0
	}
	engfunc(EngFunc_SetOrigin, cam, o)
	set_pev(cam, pev_angles, ang)
	set_pev(cam, pev_v_angle, ang)
	g_lsgCamStop[cam] = 0.0
	// a glide from A to B: the engine moves it (velocity, angular velocity); stopped when it's there
	if (from <= 0 && hasB && move > 0.0)
	{
		new Float:vel[3], Float:av[3]
		for (new k = 0; k < 3; k++)
		{
			vel[k] = (b[k] - a[k]) / move
			av[k] = (ba[k] - aa[k]) / move
		}
		set_pev(cam, pev_velocity, vel)
		set_pev(cam, pev_avelocity, av)
		g_lsgCamStop[cam] = get_gametime() + move
	}
	new n = 0
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		if ((who == 0 && p != one) || (who == 1 && get_user_team(p) != 1) || (who == 2 && get_user_team(p) != 2))
			continue
		lsg_view_back(p)
		g_lsgViewCam[p] = cam
		g_lsgViewTill[p] = secs > 0.0 ? get_gametime() + secs : 0.0
		g_lsgViewFreeze[p] = freeze != 0
		if (freeze)
			set_pev(p, pev_flags, pev(p, pev_flags) | FL_FROZEN)
		engfunc(EngFunc_SetView, p, cam)
		n++
	}
	if (!n)
		remove_entity(cam)
	return n
}

// one player's view back: first or third person as Set Camera left it
stock lsg_view_back(p)
{
	new cam = g_lsgViewCam[p]
	if (!cam)
		return
	g_lsgViewCam[p] = 0
	g_lsgViewTill[p] = 0.0
	if (is_user_connected(p))
	{
		if (g_lsgViewFreeze[p])
			set_pev(p, pev_flags, pev(p, pev_flags) & ~FL_FROZEN)
		engfunc(EngFunc_SetView, p, p)
		if (g_lsgCam[p])
			set_view(p, CAMERA_3RDPERSON)
	}
	g_lsgViewFreeze[p] = false
	// (the camera goes when nobody looks through it any more)
	for (new q = 1; q <= MaxClients; q++)
		if (g_lsgViewCam[q] == cam)
			return
	if (pev_valid(cam))
		remove_entity(cam)
}

stock lsg_view_return(p)
{
	if (p)
		lsg_view_back(p)
	else
		for (new q = 1; q <= MaxClients; q++)
			lsg_view_back(q)
}

stock lsg_view_tick(Float:now)
{
	for (new p = 1; p <= MaxClients; p++)
	{
		new cam = g_lsgViewCam[p]
		if (!cam)
			continue
		if (!pev_valid(cam) || (g_lsgViewTill[p] > 0.0 && now >= g_lsgViewTill[p]))
		{
			lsg_view_back(p)
			continue
		}
		new f = g_lsgCamFollow[cam]
		if (f > 0)
		{
			if (!pev_valid(f) || (f <= MaxClients && !is_user_connected(f)))
			{
				lsg_view_back(p)
				continue
			}
			new Float:o[3], Float:ang[3]
			pev(f, pev_origin, o)
			o[2] += g_lsgCamOff[cam]
			pev(f, f <= MaxClients ? pev_v_angle : pev_angles, ang)
			engfunc(EngFunc_SetOrigin, cam, o)
			set_pev(cam, pev_angles, ang)
			set_pev(cam, pev_v_angle, ang)
		}
		else if (g_lsgCamStop[cam] > 0.0 && now >= g_lsgCamStop[cam])
		{
			set_pev(cam, pev_velocity, Float:{0.0, 0.0, 0.0})
			set_pev(cam, pev_avelocity, Float:{0.0, 0.0, 0.0})
			g_lsgCamStop[cam] = 0.0
		}
	}
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
// the two bots sent to each downed player
stock g_lsgRevBots[33][2]

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
	// (a use goal on the spot, not on the player: the brain keeps a use goal on a player only while
	// they're alive and down gm_components' way, and this one is dead; on the spot it stays by them)
	for (new i = 0; i < 2; i++)
	{
		g_lsgRevBots[victim][i] = near[i]
		if (near[i])
			server_cmd("ls_mode goal %d use %.0f %.0f %.0f 0 75 %d 64", near[i], g_lsgDownOrg[victim][0], g_lsgDownOrg[victim][1], g_lsgDownOrg[victim][2], left)
	}
}

stock lsg_clear_down(p)
{
	if (g_lsgDown[p] && get_cvar_pointer("ls_mode_ver"))
		for (new i = 0; i < 2; i++)
			if (g_lsgRevBots[p][i] && is_user_connected(g_lsgRevBots[p][i]))
				server_cmd("ls_mode goal_clear %d use", g_lsgRevBots[p][i])
	g_lsgRevBots[p][0] = 0
	g_lsgRevBots[p][1] = 0
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
	// written within 5 s (a timer), at a new round, when the player leaves and at map end: not
	// the whole file on every call (PerfAudit #6: about 1 ms a write)
	g_lsgSavedDirty = true
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
    /// the nodes every_node has no room for (a graph holds 400): the mode-building pieces
    pub const EVERY_NODE_2: &str = include_str!("testdata/every_node_2.graph.json");
    pub const EVERY_NODE_3: &str = include_str!("testdata/every_node_3.graph.json");
    /// the HUD & View engine nodes (Y): their own graph, so others' every_node_3 edits don't meet them
    pub const EVERY_NODE_4: &str = include_str!("testdata/every_node_4.graph.json");
    pub const EVERY_NODE_FX: &str = include_str!("testdata/every_node_fx.graph.json");
    /// collision, overlap and hits (W2): their own graph too
    pub const EVERY_NODE_5: &str = include_str!("testdata/every_node_5.graph.json");
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

    /// A model or sound the graph names is loaded only if it's there (a missing one stops the map
    /// loading); what uses it checks it was loaded.
    #[test]
    fn named_files_are_loaded_only_if_there() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "pre", "name": "Pre"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "e", "type": "event.player_spawned", "pos": {"x": 0, "y": 0}, "params": {}},
                {"id": "p", "type": "ent.spawn_prop", "pos": {"x": 300, "y": 0}, "params": {"model": "models/not_here.mdl"}},
                {"id": "v", "type": "vec.make", "pos": {"x": 300, "y": 200}, "params": {}},
                {"id": "s", "type": "fx.sound_at", "pos": {"x": 600, "y": 0}, "params": {"sound": "custom/not_here.wav"}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                {"id": "w2", "from": {"node": "e", "pin": "player"}, "to": {"node": "p", "pin": "at"}},
                {"id": "w3", "from": {"node": "p", "pin": "then"}, "to": {"node": "s", "pin": "exec"}},
                {"id": "w4", "from": {"node": "v", "pin": "out"}, "to": {"node": "s", "pin": "pos"}}
            ]
        }).to_string()).unwrap();
        let src = generate(&g, false).source;
        assert!(src.contains("lsg_pre_model(\"models/not_here.mdl\")") && src.contains("lsg_pre_sound(\"custom/not_here.wav\")"), "the guarded loads");
        let pre = &src[src.find("public plugin_precache()").unwrap()..];
        let pre = &pre[..pre.find("\n}").unwrap()];
        assert!(!pre.contains("\tprecache_model(\"models/not_here") && !pre.contains("\tprecache_sound("), "{pre}");
        assert!(src.contains("if (!lsg_have(model))") && src.contains("if (!lsg_have(snd))"), "the users check");
    }

    /// Set Ammo / Give Ammo read the gun from the active item (X2: get_user_weapon says 0 while a gun
    /// is being drawn, and "Invalid weapon id 0" came 229 times in One in the Chamber's Test run).
    #[test]
    fn ammo_reads_the_active_item() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "ammo", "name": "Ammo"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "e", "type": "event.player_spawned", "pos": {"x": 0, "y": 0}, "params": {}},
                {"id": "a", "type": "player.set_ammo", "pos": {"x": 300, "y": 0}, "params": {}},
                {"id": "b", "type": "player.give_ammo", "pos": {"x": 600, "y": 0}, "params": {}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "a", "pin": "exec"}},
                {"id": "w2", "from": {"node": "e", "pin": "player"}, "to": {"node": "a", "pin": "player"}},
                {"id": "w3", "from": {"node": "a", "pin": "then"}, "to": {"node": "b", "pin": "exec"}},
                {"id": "w4", "from": {"node": "e", "pin": "player"}, "to": {"node": "b", "pin": "player"}}
            ]
        }).to_string()).unwrap();
        let src = generate(&g, false).source;
        for f in ["stock lsg_set_ammo(", "stock lsg_give_ammo("] {
            let at = src.find(f).unwrap_or_else(|| panic!("{f}"));
            let body = &src[at..at + src[at..].find("\n}").unwrap()];
            assert!(body.contains("m_pActiveItem") && body.contains("cs_get_weapon_id") && !body.contains("get_user_weapon("), "{body}");
        }
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
        let special = ["func.define", "fn.entry", "macro.entry", "event.custom", "creature.on_idle", "event.chat_command", "event.player_about_hurt", "event.player_revived", "event.down_timeout", "event.time_up", "event.on_tick"];
        for d in registry().nodes.iter().filter(|d| d.event && !special.contains(&d.kind.as_str()) && !collide::is_event(&d.kind) && !coverage::is_event(&d.kind) && !p5::is_event(&d.kind)) {
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

    /// NPCs (T1fix): a line, a speaker and replies from the graph (typed, or wired from a
    /// variable) reach players only as text (ls_npc_say / ls_npc_choose), never a server command.
    #[test]
    fn npc_text_stays_data() {
        let evil = "x\"); server_cmd(\"quit\"); /* ^\"|";
        let g = serde_json::json!({
            "schema": "loner-strike/graph", "version": 3, "mode": {"id": "npc_evil", "name": "Evil"},
            "variables": [{"id": "v1", "name": "Line", "type": "text", "scope": "mode", "default": evil}],
            "nodes": [
                {"id": "t", "type": "npc.on_talk", "params": {"name": evil}},
                {"id": "s", "type": "npc.say", "params": {"speaker": evil, "text": evil}},
                {"id": "c", "type": "npc.choose", "params": {"speaker": evil, "text": evil, "o1": evil, "o2": evil, "count": 2, "secs": 5}},
                {"id": "gv", "type": "var.get", "params": {"var": "v1"}},
                {"id": "n", "type": "npc.spawn", "params": {"model": "../../evil\".mdl", "name": evil}},
                {"id": "i", "type": "npc.spawn_use", "params": {"model": evil, "prompt": evil}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "t", "pin": "then"}, "to": {"node": "s", "pin": "exec"}},
                {"id": "w2", "from": {"node": "t", "pin": "player"}, "to": {"node": "s", "pin": "player"}},
                {"id": "w3", "from": {"node": "s", "pin": "then"}, "to": {"node": "c", "pin": "exec"}},
                {"id": "w4", "from": {"node": "t", "pin": "player"}, "to": {"node": "c", "pin": "player"}},
                {"id": "w5", "from": {"node": "gv", "pin": "out"}, "to": {"node": "c", "pin": "o3"}},
                {"id": "w6", "from": {"node": "c", "pin": "r1"}, "to": {"node": "n", "pin": "exec"}},
                {"id": "w7", "from": {"node": "n", "pin": "then"}, "to": {"node": "i", "pin": "exec"}},
                {"id": "w8", "from": {"node": "t", "pin": "player"}, "to": {"node": "n", "pin": "at_ent"}},
                {"id": "w9", "from": {"node": "t", "pin": "player"}, "to": {"node": "i", "pin": "at_ent"}}
            ]
        });
        let g = super::super::graph::from_value(g).unwrap();
        for debug in [false, true] {
            let src = generate(&g, debug).source;
            assert!(!src.contains("server_cmd(\"quit"), "{src}");
            assert_eq!(src.matches("*/").count(), 1, "only the header comment closes");
            for l in src.lines().filter(|l| !l.trim_start().starts_with("//")) {
                assert_eq!(l.replace("^\"", "").replace("'\"'", "").matches('"').count() % 2, 0, "{l}");
            }
            // a model outside models/ never loads; the replies' | can't split one into two
            assert!(!src.contains("evil") || !src.contains("../"), "no path out of models/");
            assert!(src.contains("ls_npc_precache(\"models/w_c4.mdl\")"));
            assert!(!src.lines().any(|l| l.contains("lsg_npc_rep(") && l.contains('|')), "no | inside a reply literal");
            assert!(src.contains("public ls_on_npc_choice(id, npc, choice)") && src.contains("public ls_on_npc_talk(id, npc)"));
        }
    }

    /// A conversation driven from a data table: the line and replies are Get Cell wires, a reply
    /// wired back into the same Say and Choose (the next row) isn't a loop the check refuses, and
    /// Picked (1-6) comes back with the reply.
    #[test]
    fn npc_dialogue_from_a_table_in_a_loop() {
        let g = serde_json::json!({
            "schema": "loner-strike/graph", "version": 3, "mode": {"id": "npc_table", "name": "Table talk"},
            "dataTables": [{"id": "dt1", "name": "Lines", "key": "id", "columns": [{"id": "id", "name": "Id", "type": "number"}, {"id": "line", "name": "Line", "type": "text"}, {"id": "yes", "name": "Yes", "type": "text"}],
                        "rows": [[0, "Hello there.", "Go on"], [1, "Still here?", "Bye"]]}],
            "variables": [{"id": "row", "name": "Row", "type": "number", "scope": "player", "default": 0}, {"id": "last", "name": "Last pick", "type": "number", "scope": "player", "default": 0}],
            "nodes": [
                {"id": "t", "type": "npc.on_talk"},
                {"id": "c", "type": "npc.choose", "params": {"count": 2, "o2": "Stop", "secs": 30}},
                {"id": "gr", "type": "var.get", "params": {"var": "row"}},
                {"id": "line", "type": "table.get_cell", "params": {"table": "dt1", "column": "line"}},
                {"id": "yes", "type": "table.get_cell", "params": {"table": "dt1", "column": "yes"}},
                {"id": "add", "type": "math.op", "params": {"op": "add", "b": 1}},
                {"id": "next", "type": "var.set", "params": {"var": "row"}},
                {"id": "keep", "type": "var.set", "params": {"var": "last"}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "t", "pin": "then"}, "to": {"node": "c", "pin": "exec"}},
                {"id": "w2", "from": {"node": "t", "pin": "player"}, "to": {"node": "c", "pin": "player"}},
                {"id": "w3", "from": {"node": "t", "pin": "npc"}, "to": {"node": "c", "pin": "npc"}},
                {"id": "w4", "from": {"node": "t", "pin": "player"}, "to": {"node": "gr", "pin": "player"}},
                {"id": "w5", "from": {"node": "gr", "pin": "out"}, "to": {"node": "line", "pin": "row"}},
                {"id": "w6", "from": {"node": "gr", "pin": "out"}, "to": {"node": "yes", "pin": "row"}},
                {"id": "w7", "from": {"node": "line", "pin": "out"}, "to": {"node": "c", "pin": "text"}},
                {"id": "w8", "from": {"node": "yes", "pin": "out"}, "to": {"node": "c", "pin": "o1"}},
                {"id": "w9", "from": {"node": "c", "pin": "r1"}, "to": {"node": "keep", "pin": "exec"}},
                {"id": "w10", "from": {"node": "c", "pin": "picked"}, "to": {"node": "keep", "pin": "value"}},
                {"id": "w11", "from": {"node": "t", "pin": "player"}, "to": {"node": "keep", "pin": "player"}},
                {"id": "w12", "from": {"node": "keep", "pin": "then"}, "to": {"node": "next", "pin": "exec"}},
                {"id": "w13", "from": {"node": "gr", "pin": "out"}, "to": {"node": "add", "pin": "a"}},
                {"id": "w14", "from": {"node": "add", "pin": "out"}, "to": {"node": "next", "pin": "value"}},
                {"id": "w15", "from": {"node": "t", "pin": "player"}, "to": {"node": "next", "pin": "player"}},
                {"id": "w16", "from": {"node": "next", "pin": "then"}, "to": {"node": "c", "pin": "exec"}}
            ]
        });
        let g = super::super::graph::from_value(g).unwrap();
        let p = validate(&g);
        assert!(!has_errors(&p), "{p:#?}");
        let src = generate(&g, false).source;
        assert!(src.contains("ls_npc_choose(") && src.contains("stock lsg_ask"), "{src}");
        assert!(src.contains("g_lsgAskK[id][choice]") && src.contains("= _:float(r)"), "Picked is the reply's number");
        // the replies come from the table at run time
        assert!(src.contains("lsg_npc_rep(lsgR, charsmax(lsgR), ctx[0], lsgN, lsgC, 1, lsg_dt0_"), "{src}");
        assert!(src.contains("lsg_npc_line(lsgL, charsmax(lsgL), \"\", lsg_dt0_"), "{src}");
        // and inside a Function, a wait is refused as for Menu
        let mut inf = g.clone();
        inf.functions.push(serde_json::from_value(serde_json::json!({"id": "f1", "name": "Ask", "inputs": [], "outputs": []})).unwrap());
        inf.nodes.push(serde_json::from_value(serde_json::json!({"id": "fe", "type": "fn.entry", "graph": "f1"})).unwrap());
        inf.nodes.push(serde_json::from_value(serde_json::json!({"id": "c2", "type": "npc.choose", "graph": "f1"})).unwrap());
        assert!(validate(&inf).iter().any(|p| p.node.as_deref() == Some("c2") && p.message.contains("a Function runs straight through")));
    }

    /// Bots: Offer on an interactable turns its bot use on (gm_components keeps the offer).
    #[test]
    fn bots_offer_an_interactable() {
        let g = serde_json::json!({
            "schema": "loner-strike/graph", "version": 3, "mode": {"id": "npc_bots", "name": "Bots use"},
            "variables": [],
            "nodes": [{"id": "m", "type": "event.map_started"}, {"id": "i", "type": "npc.spawn_use", "params": {"at": "spot1", "bots": true}}, {"id": "o", "type": "bots.offer", "params": {"kind": "use", "cost": 0}}],
            "edges": [
                {"id": "w1", "from": {"node": "m", "pin": "then"}, "to": {"node": "i", "pin": "exec"}},
                {"id": "w2", "from": {"node": "i", "pin": "then"}, "to": {"node": "o", "pin": "exec"}},
                {"id": "w3", "from": {"node": "i", "pin": "out"}, "to": {"node": "o", "pin": "at"}}
            ]
        });
        let g = super::super::graph::from_value(g).unwrap();
        assert!(!has_errors(&validate(&g)), "{:#?}", validate(&g));
        let src = generate(&g, false).source;
        assert!(src.contains("ls_interactable_offer(g_lsgOut[") && src.contains("bots=1"), "{src}");
        assert!(!src.contains("offer %s trap"), "not the brain's price offer");
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

    /// A kick's reason never reaches the console as written: quotes are refused by the checks,
    /// and the plugin keeps only letters, digits, spaces and plain punctuation (no ; or control
    /// characters) inside the quotes; Change Map takes map-name letters only.
    #[test]
    fn kick_and_map_change_cant_inject_commands() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "inj", "name": "Inj"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "e", "type": "event.player_spawned", "pos": {"x": 0, "y": 0}, "params": {}},
                {"id": "k", "type": "player.kick", "pos": {"x": 300, "y": 0}, "params": {"reason": "x\";rcon_password hacked;\""}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "k", "pin": "exec"}},
                {"id": "w2", "from": {"node": "e", "pin": "player"}, "to": {"node": "k", "pin": "player"}}
            ]
        }).to_string()).unwrap();
        // the quote is refused by the checks
        assert!(validate(&g).iter().any(|p| p.level == "error" && p.node.as_deref() == Some("k")));
        // and even generated anyway: the literal has no quote, and the stock cleans what's left
        let src = generate(&g, false).source;
        let line = src.lines().find(|l| l.contains("lsg_kick(ctx")).unwrap();
        assert!(!line.contains("\\\"") && line.matches('"').count() == 2, "{line}");
        assert!(src.contains("lsg_clean(reason, r, charsmax(r))"));
        assert!(src.contains("stock bool:lsg_safe_cmd(const cmd[], amx)") && src.contains("if (!isalnum(c[i]) && c[i] != ' '"), "the command check runs on the final string");
        // the plugin's lists are the app's
        for w in super::super::graph::cmd_rules::AMX_WORDS.iter().chain(&super::super::graph::cmd_rules::DENY_WORDS).chain(&super::super::graph::cmd_rules::SERVER_WORDS) {
            assert!(src.contains(&format!("\"{w}\"")), "{w}");
        }
        assert!(src.contains("if (!isalnum(map[i]) && map[i] != '_' && map[i] != '-' && map[i] != '.')"));
    }

    /// A Custom Event the map fires: its Player is who walked into the trigger. Ham_Use's first
    /// value is the activator (ham_const.inc names it the caller); the generated code takes
    /// whichever of the two is a player (QA4-B: it read only the second, so always nobody).
    #[test]
    fn map_fired_custom_events_know_who() {
        let g = super::super::graph::from_value(serde_json::json!({
            "schema": "loner-strike/graph", "version": 3, "mode": {"id": "xev", "name": "Map event"},
            "customEvents": [{"id": "ce1", "name": "Gate", "inputs": [{"id": "who", "name": "Player", "type": "player"}], "map": true}],
            "nodes": [{"id": "ev", "type": "event.custom", "params": {"event": "ce1"}}, {"id": "pr", "type": "debug.print", "params": {"text": "who", "chat": false}}],
            "edges": [{"id": "w1", "from": {"node": "ev", "pin": "then"}, "to": {"node": "pr", "pin": "exec"}}, {"id": "w2", "from": {"node": "ev", "pin": "who"}, "to": {"node": "pr", "pin": "value"}}]
        }))
        .unwrap();
        let src = generate(&g, false).source;
        assert!(src.contains("public lsg_ev_use(ent, caller, activator)"));
        let who = src.lines().find(|l| l.trim_start().starts_with("new who =")).unwrap();
        let c = who.find("caller >= 1").unwrap();
        let a = who.find("activator >= 1").unwrap();
        assert!(c < a, "the first Ham_Use value (the activator) first: {who}");
        assert!(src.contains("lsg_ce0(who)"), "the Player input gets who");
    }

    /// A timer that comes due while a breakpoint has the world paused keeps its schedule after
    /// Resume (what it still had to wait), instead of firing at the next quarter-second check.
    #[test]
    fn timers_keep_their_schedule_across_a_pause() {
        let g = parse(EVERY_NODE).unwrap();
        let dbg = generate(&g, true).source;
        let rel = generate(&g, false).source;
        let repeat = g.nodes.iter().find(|n| n.kind == "timer.repeat").map(|n| n.id.clone()).unwrap();
        let live: Vec<&str> = dbg.lines().filter(|l| l.starts_with("public lsg_t") && l.contains("(const data[], task)")).collect();
        assert!(!live.is_empty(), "timer callbacks");
        for cb in live {
            let name = cb.trim_start_matches("public ").split('(').next().unwrap();
            let i: usize = name.trim_start_matches("lsg_t").parse().unwrap();
            let body = &dbg[dbg.find(cb).unwrap()..];
            let first = body.lines().nth(2).unwrap_or("");
            assert_eq!(first.trim(), format!("if (lsd_timer_wait({i}, \"{name}\", data))"), "{repeat}: {cb}");
        }
        assert!(dbg.contains("g_lsdTOwe[n] = get_gametime() - g_lsdPausedAt") && dbg.contains("g_lsdResumedAt = get_gametime()"));
        assert!(!rel.contains("lsd_timer_wait"));
    }

    /// One graph with every node in the registry: it checks clean, every flow node becomes a
    /// function, and both builds are snapshotted (and compiled with amxxpc in mod.rs).
    #[test]
    fn every_node_snapshot() {
        let g = parse(EVERY_NODE).unwrap();
        // (these graphs nest loops on purpose: the cost advice is left out)
        let p = super::super::graph::validate_core(&g);
        assert!(p.is_empty(), "{:#?}", p);
        let g2 = parse(EVERY_NODE_2).unwrap();
        let p2 = super::super::graph::validate_core(&g2);
        assert!(p2.is_empty(), "every_node_2: {:#?}", p2);
        let g3 = parse(EVERY_NODE_3).unwrap();
        let p3 = super::super::graph::validate_core(&g3);
        assert!(p3.is_empty(), "every_node_3: {:#?}", p3);
        let g4 = parse(EVERY_NODE_4).unwrap();
        let p4 = super::super::graph::validate_core(&g4);
        assert!(p4.is_empty(), "every_node_4: {:#?}", p4);
        let gfx = parse(EVERY_NODE_FX).unwrap();
        let pfx = super::super::graph::validate_core(&gfx);
        assert!(pfx.is_empty(), "every_node_fx: {:#?}", pfx);
        let g5 = parse(EVERY_NODE_5).unwrap();
        let p5 = super::super::graph::validate_core(&g5);
        assert!(p5.is_empty(), "every_node_5: {:#?}", p5);
        let used: BTreeSet<&str> = g.nodes.iter().chain(g2.nodes.iter()).chain(g3.nodes.iter()).chain(g4.nodes.iter()).chain(g5.nodes.iter()).chain(gfx.nodes.iter()).map(|n| n.kind.as_str()).collect();
        // W1: the utility nodes are in every_node_w1.graph.json (w1_nodes.rs tests it)
        let mut used = used;
        // T1fix: the projectile nodes are in proj_tests.rs (their own graph, generated and compiled there)
        for k in super::super::proj_tests::KINDS {
            used.insert(k);
        }
        for k in w1::PURE.iter().chain(w1::FLOW) {
            used.insert(k);
        }
        for k in p5::kinds() {
            used.insert(k);
        }
        // EC: the engine coverage nodes are in every_node_ec.graph.json (gen/coverage/tests.rs tests it)
        let used: BTreeSet<&str> = used.into_iter().chain(coverage::PURE.iter().copied()).chain(coverage::EVENTS.iter().copied()).chain(coverage::ACTIONS.iter().copied()).collect();
        for d in &registry().nodes {
            assert!(used.contains(d.kind.as_str()), "every_node(_2/_3/_4).graph.json lacks {}", d.kind);
        }
        for (name, gg) in [("every_node_2", &g2), ("every_node_3", &g3), ("every_node_4", &g4), ("every_node_5", &g5), ("every_node_fx", &gfx)] {
            let r = generate(gg, false);
            for n in gg.nodes.iter().filter(|n| { let d = registry().get(&n.kind).unwrap(); !d.pure && !d.reroute }) {
                assert!(r.spans.iter().any(|s| s.node == n.id), "{name}: {} isn't in the plugin", n.id);
            }
            snapshot(&format!("{name}.sma"), &r.source);
            snapshot(&format!("{name}.debug.sma"), &generate(gg, true).source);
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
            ("markers.sma", include_str!("testdata/markers.graph.json")),
        ] {
            let g = parse(t).unwrap();
            assert!(validate(&g).is_empty(), "{name}: {:#?}", validate(&g));
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

    /// A flow runs as one call chain, every node's frame on the stack at once: the plugin gets
    /// room for all of them (node coverage run: 120 Prints in a row died with "stack error" on
    /// AMX Mod X's default 4096 cells, and without debug as garbage in replace and set_task).
    #[test]
    fn stack_has_room_for_a_long_chain() {
        let n = 120;
        let mut nodes = vec![serde_json::json!({"id": "e", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}})];
        let mut edges = vec![];
        for i in 0..n {
            nodes.push(serde_json::json!({"id": format!("p{i}"), "type": "debug.print", "pos": {"x": 300 * (i + 1), "y": 0}, "params": {"text": "x", "chat": false}}));
            let from = if i == 0 { "e".to_string() } else { format!("p{}", i - 1) };
            edges.push(serde_json::json!({"id": format!("w{i}"), "from": {"node": from, "pin": "then"}, "to": {"node": format!("p{i}"), "pin": "exec"}}));
        }
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "long", "name": "Long"}, "variables": [], "comments": [],
            "nodes": nodes, "edges": edges
        }).to_string()).unwrap();
        assert!(!has_errors(&validate(&g)));
        let src = generate(&g, true).source;
        let cells: usize = src.lines().find_map(|l| l.strip_prefix("#pragma dynamic ")).expect("a stack size").trim().parse().unwrap();
        // (a debug Print's frame: its value, two text buffers and the debugger's: under 450 cells)
        assert!(cells >= n * 450, "{cells} cells for {n} Prints");
        assert_eq!(stack_cells(0), 16_384);
        assert_eq!(stack_cells(10_000), 262_144);
    }

    /// A number wired into Print String (and chat, HUD, notices) keeps its decimals (node coverage
    /// run: a gravity of 0.5 printed as 1), in a plain graph too, without the version 3 helpers.
    #[test]
    fn printed_numbers_keep_decimals() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "num", "name": "Num"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "e", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}},
                {"id": "n", "type": "math.number", "pos": {"x": 0, "y": 200}, "params": {"value": 0.5}},
                {"id": "p", "type": "debug.print", "pos": {"x": 300, "y": 0}, "params": {"text": "g", "chat": false}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                {"id": "w2", "from": {"node": "n", "pin": "out"}, "to": {"node": "p", "pin": "value"}}
            ]
        }).to_string()).unwrap();
        let src = generate(&g, false).source;
        assert!(src.contains("lsg_ntext(") && src.contains("stock lsg_ntext(Float:n)") && !src.contains("floatround(0.5)"), "{src}");
    }

    /// In a Playtest build every flow node that reports its start reports how it went too (node
    /// coverage run: Run Server/AMX Command, Spawn Prop and Call Plugin Function said nothing when
    /// they worked, the Switches never said which way they went).
    #[test]
    fn every_flow_node_reports_a_result() {
        let mut missing = vec![];
        for (name, text) in [
            ("every_node", include_str!("testdata/every_node.graph.json")),
            ("every_node_2", include_str!("testdata/every_node_2.graph.json")),
            ("every_node_3", include_str!("testdata/every_node_3.graph.json")),
            ("all_nodes", include_str!("testdata/all_nodes.graph.json")),
        ] {
            let g = parse(text).unwrap();
            let src = generate(&g, true).source;
            for block in src.split("\n// ").skip(1) {
                let body = &block[..block.find("\n}\n").unwrap_or(block.len())];
                if body.contains("lsd_enter(") && !body.contains("lsd_exit(") && !body.contains("lsd_note(") {
                    missing.push(format!("{name}: {}", block.lines().next().unwrap_or("")));
                }
            }
        }
        assert!(missing.is_empty(), "{missing:#?}");
    }

    /// Buy Perk's Bought is kept for what reads it (Y's components sweep: always "no", the
    /// result was thrown away).
    #[test]
    fn buy_perk_keeps_bought() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "pk", "name": "Pk"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "e", "type": "event.player_spawned", "pos": {"x": 0, "y": 0}, "params": {}},
                {"id": "b", "type": "up.perk_buy", "pos": {"x": 300, "y": 0}, "params": {}},
                {"id": "p", "type": "debug.print", "pos": {"x": 600, "y": 0}, "params": {"text": "bought", "chat": false}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "b", "pin": "exec"}},
                {"id": "w2", "from": {"node": "e", "pin": "player"}, "to": {"node": "b", "pin": "player"}},
                {"id": "w3", "from": {"node": "b", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                {"id": "w4", "from": {"node": "b", "pin": "bought"}, "to": {"node": "p", "pin": "value"}}
            ]
        }).to_string()).unwrap();
        assert!(!has_errors(&validate(&g)));
        let src = generate(&g, false).source;
        assert!(src.contains("g_lsgO[0] = ls_perk_buy("), "stored");
        assert!(src.contains("g_lsgYesNo[(g_lsgO[0]) ? 1 : 0]"), "read back by the Print");
    }

    /// A component pin wired with another kind of component (a barricade into Door) is checked and
    /// said, not left to gm_components' native error, which ended the whole flow silently (Y's
    /// components sweep).
    #[test]
    fn component_pins_check_their_kind() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "ck", "name": "Ck"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "e", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}},
                {"id": "f", "type": "flow.pick_entity", "pos": {"x": 300, "y": 0}, "params": {"by": "class", "name": "ls_barricade"}},
                {"id": "d", "type": "comp.door", "pos": {"x": 600, "y": 0}, "params": {}},
                {"id": "s", "type": "shop.open", "pos": {"x": 900, "y": 0}, "params": {}},
                {"id": "h", "type": "comp.barricade_hp", "pos": {"x": 600, "y": 200}, "params": {}},
                {"id": "p", "type": "debug.print", "pos": {"x": 1200, "y": 0}, "params": {"text": "hp", "chat": false}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "f", "pin": "exec"}},
                {"id": "w2", "from": {"node": "f", "pin": "then"}, "to": {"node": "d", "pin": "exec"}},
                {"id": "w3", "from": {"node": "f", "pin": "entity"}, "to": {"node": "d", "pin": "door"}},
                {"id": "w4", "from": {"node": "d", "pin": "then"}, "to": {"node": "s", "pin": "exec"}},
                {"id": "w5", "from": {"node": "f", "pin": "entity"}, "to": {"node": "s", "pin": "shop"}},
                {"id": "w6", "from": {"node": "s", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                {"id": "w7", "from": {"node": "f", "pin": "entity"}, "to": {"node": "h", "pin": "barricade"}},
                {"id": "w8", "from": {"node": "h", "pin": "out"}, "to": {"node": "p", "pin": "value"}}
            ]
        }).to_string()).unwrap();
        assert!(!has_errors(&validate(&g)), "{:?}", validate(&g));
        let src = generate(&g, true).source;
        assert!(src.contains("stock bool:lsg_kind_ok(ent, const kind[])"));
        assert!(src.contains("!lsg_kind_ok(") && src.contains("\"ls_timed_door\")") && src.contains("\"ls_shop\")"), "the actions check");
        assert!(src.contains("Not done: that isn't a Timed door (or gm_components isn't running).") && src.contains("that isn't a Shop"));
        assert!(src.contains("lsg_kind_ok(") && src.contains("\"ls_barricade\") ? ls_barricade_hp("), "the value checks");
        // (the native only runs past the check)
        let door = &src[src.find("ls_door_open(").unwrap().saturating_sub(400)..src.find("ls_door_open(").unwrap()];
        assert!(door.contains("\"ls_timed_door\""), "{door}");
    }

    /// Wave Spawned One's entity is the spawner (gm_components asks before the enemy is made):
    /// it says so (Y's components sweep: it was labelled Spawned and read as the enemy).
    #[test]
    fn wave_spawn_says_spawner() {
        let nodes: Value = serde_json::from_str(include_str!("../../../../ui/src/views/tools/studio/nodes.json")).unwrap();
        let n = nodes["nodes"].as_array().unwrap().iter().find(|n| n["type"] == "comp.on_wave_spawn").unwrap();
        let pin = n["outputs"].as_array().unwrap().iter().find(|p| p["id"] == "spawned").unwrap();
        assert_eq!(pin["label"], "Spawner");
        assert!(n["help"].as_str().unwrap().contains("not the enemy"));
    }

    /// Chat Command runs with arguments too: AMX Mod X matches a "say /give" registration only
    /// against the bare word, so "/give 5" never came (node coverage run: "/shop" ran 6 times,
    /// "/shop 5" none). It's on all of say, and the handler checks the first word.
    #[test]
    fn chat_commands_take_arguments() {
        let g = parse(&serde_json::json!({
            "schema": super::super::graph::SCHEMA, "version": super::super::graph::VERSION, "mode": {"id": "cc", "name": "Cc"}, "variables": [], "comments": [],
            "nodes": [
                {"id": "c", "type": "event.chat_command", "pos": {"x": 0, "y": 0}, "params": {"command": "give"}},
                {"id": "p", "type": "debug.print", "pos": {"x": 300, "y": 0}, "params": {"text": "args", "chat": false}}
            ],
            "edges": [
                {"id": "w1", "from": {"node": "c", "pin": "then"}, "to": {"node": "p", "pin": "exec"}},
                {"id": "w2", "from": {"node": "c", "pin": "args"}, "to": {"node": "p", "pin": "value"}}
            ]
        }).to_string()).unwrap();
        let src = generate(&g, false).source;
        assert!(!src.contains("say /give") && !src.contains("say !give"), "no bare-word registrations");
        assert!(src.contains("register_clcmd(\"say\", \"lsg_chat") && src.contains("register_clcmd(\"say_team\", \"lsg_chat"));
        let h = &src[src.find("public lsg_chat").unwrap()..];
        let h = &h[..h.find("\n}").unwrap()];
        assert!(h.contains("!equali(w[1], \"give\")") && h.contains("return PLUGIN_CONTINUE\n\tctx[0] = id"), "{h}");
        // (the shared text slot only once it's this command)
        assert!(h.find("copy(g_lsgTxt[0]").unwrap() > h.find("equali").unwrap());
    }

    /// A vector as text: whole units for a place, two decimals for a short one (node coverage run:
    /// Normalize printed "-0 -0 0" for a direction of length 1).
    #[test]
    fn short_vectors_as_text_keep_decimals() {
        let h = include_str!("v3_helpers.pawn");
        let f = &h[h.find("stock lsg_vtext(v)").unwrap()..];
        let f = &f[..f.find("\n}").unwrap()];
        assert!(f.contains("floatabs(x) < 10.0") && f.contains("\"%.2f %.2f %.2f\"") && f.contains("\"%.0f %.0f %.0f\""), "{f}");
    }
}
