//! Comfort limits (how many markers, trails, projectiles... a mode keeps at once): defaults, not
//! hard caps. Each is a mode setting in the "Limits" group (a number variable `lim_<id>` the Studio
//! adds with the node: the creator sets its start value, hosts change it in mode settings), and
//! the Problems list warns when one is raised a lot. The only enforced ceilings are the engine's
//! crash points: free entity slots (gen.rs `lsg_ent_room` keeps a reserve), the precache table
//! and the reliable channel; a mode that reaches one skips that spawn and says so.
//!
//! Node agents: add a line to LIMITS for a node family's limit, read it in codegen with
//! `Gen::limit_expr("<id>")`, and size any array by `max`. The UI side is limits.ts (same ids).

use super::graph::{Graph, Problem, Variable};
use serde_json::{json, Value};

pub struct Limit {
    pub id: &'static str,
    /// the setting's label ("Objective markers at once")
    pub label: &'static str,
    pub default: u32,
    /// raised above this, Problems warns (never blocks)
    pub lag_above: u32,
    /// what the plugin can hold (its arrays): a value above is clamped
    pub max: u32,
    /// the nodes that use it (the Studio adds the setting with them)
    pub kinds: &'static [&'static str],
}

pub const LIMITS: &[Limit] = &[
    Limit { id: "markers", label: "Objective markers at once", default: 32, lag_above: 64, max: 256, kinds: &["fx.marker"] },
    Limit { id: "trails", label: "Trails at once", default: 32, lag_above: 64, max: 256, kinds: &["fun.trail"] },
    // EC (gen/coverage.rs)
    Limit { id: "ec_attach", label: "Attached things at once", default: 64, lag_above: 128, max: 256, kinds: &["ent.attach"] },
    Limit { id: "ec_anim_loops", label: "Looping animations at once", default: 64, lag_above: 128, max: 256, kinds: &["ent.play_anim"] },
    Limit { id: "ec_npc_moves", label: "NPC moves at once", default: 32, lag_above: 48, max: 64, kinds: &["npc.move_to", "npc.follow_path"] },
    Limit { id: "ec_map_hooks", label: "Map entity kinds heard", default: 96, lag_above: 160, max: 256, kinds: &["event.map_fired"] },
    // P5: Player Command's uses a second per player, Top Players' places
    Limit { id: "command_rate", label: "Player Command uses a second", default: 5, lag_above: 20, max: 1000, kinds: &["in.command"] },
    Limit { id: "top_players", label: "Top Players places", default: 10, lag_above: 50, max: 100, kinds: &["save.top"] },
    // W2 (collide.rs): trigger volumes, who each holds at once, bots' avoided areas (the bot brain keeps 32)
    Limit { id: "trigger_volumes", label: "Trigger volumes at once", default: 32, lag_above: 64, max: 256, kinds: &["vol.spawn"] },
    Limit { id: "volume_members", label: "Things inside one trigger volume", default: 32, lag_above: 48, max: 64, kinds: &["vol.spawn"] },
    Limit { id: "avoid_areas", label: "Areas bots avoid at once", default: 16, lag_above: 24, max: 32, kinds: &["bots.avoid"] },
    Limit { id: "projectiles", label: "Projectiles at once", default: 64, lag_above: 128, max: 512, kinds: &["proj.spawn"] },
    // (effects, HUD and chat sent in one server frame: past ~160 the reliable channel can overflow
    // and kick players; gen.rs lsg_msg_room)
    Limit { id: "messages", label: "Effect messages a frame", default: 48, lag_above: 96, max: 160, kinds: &["fun.burst", "fun.trail", "msg.killfeed", "msg.status_icon", "proj.spawn", "msg.big_hud", "msg.color_chat", "msg.radio"] },
];

/// A mode's own files (models, sounds, sprites) loaded with the map: the game holds 512 models and
/// 512 sounds for the map, every plugin and the game itself together, so a mode keeps to its share.
/// Problems warns past PRECACHE_WARN and refuses past PRECACHE_MAX; the plugin refuses past it too.
pub const PRECACHE_WARN: usize = 64;
pub const PRECACHE_MAX: usize = 160;

/// The files a mode loads with the map (its nodes' model, sound and sprite paths).
pub fn precache_count(g: &Graph) -> usize {
    crate::tools::requirements::content_of(g).len()
}

pub fn get(id: &str) -> Option<&'static Limit> {
    LIMITS.iter().find(|l| l.id == id)
}

/// The mode-setting variable that holds a limit (its id is `lim_<id>`).
pub fn var_id(id: &str) -> String {
    format!("lim_{id}")
}

pub fn var<'a>(g: &'a Graph, id: &str) -> Option<&'a Variable> {
    let want = var_id(id);
    g.variables.iter().find(|v| v.id == want && v.ty == "number" && v.scope == "mode")
}

/// The limit's variable as the Studio adds it (a mode setting in the Limits group).
pub fn var_json(l: &Limit) -> Value {
    json!({"id": var_id(l.id), "name": l.label.chars().take(30).collect::<String>(), "type": "number", "scope": "mode", "default": l.default,
        "setting": {"label": l.label, "help": format!("A comfort limit (default {}). Higher may lag servers with many players.", l.default), "min": 1, "max": l.max, "step": 1, "group": "Limits"}})
}

/// A limit raised a lot: a warning on the mode (never an error, never blocks Compile).
pub fn problems(g: &Graph) -> Vec<Problem> {
    let mut p = Vec::new();
    let files = precache_count(g);
    if files > PRECACHE_MAX {
        p.push(Problem { level: "error", node: None, pin: None, edge: None, message: format!("This mode loads {files} files of its own (models, sounds, sprites); the most is {PRECACHE_MAX}: the game holds 512 models and 512 sounds for everything on the server.") });
    } else if files > PRECACHE_WARN {
        p.push(Problem { level: "warning", node: None, pin: None, edge: None, message: format!("This mode loads {files} files of its own (models, sounds, sprites): on a map with many of its own, the game's 512 could run out.") });
    }
    for l in LIMITS {
        if !g.nodes.iter().any(|n| l.kinds.contains(&n.kind.as_str())) {
            continue;
        }
        let Some(v) = var(g, l.id) else { continue };
        let n = v.default.as_f64().unwrap_or(l.default as f64);
        if n > l.lag_above as f64 {
            p.push(Problem { level: "warning", node: None, pin: None, edge: None, message: format!("{} is set to {} (the default is {}): that may lag servers with many players.", l.label, n.round(), l.default) });
        }
    }
    p
}
