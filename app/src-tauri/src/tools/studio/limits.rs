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
    Limit { id: "projectiles", label: "Projectiles at once", default: 64, lag_above: 128, max: 512, kinds: &["proj.spawn"] },
];

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
