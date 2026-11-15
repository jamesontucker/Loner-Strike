//! The player's servers, more of them for agents: crash tracking (crashes.rs).

use super::catalog::{get, obj, server_prop, Done, Kind, Spec};
use super::Ctx;
use serde_json::{json, Value};

pub fn specs() -> Vec<Spec> {
    vec![Spec { name: "server_crashes", title: "Server crashes", kind: Kind::Read,
        schema: || obj(json!({"server": server_prop(), "fresh": {"type": "boolean", "description": "Read the server's files again now (else what was read in the last two minutes)"}, "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Most crashes listed (default 20)"}}), &["server"]),
        description: "One of the player's servers' crashes, newest first: time, map, mode, kind (crash, hang, abort), the fault and module, the stack, the last marks and log lines, and atStop (faults at a stop or from a debugger, not counted). Also how many in the last 24 hours, warnings for a known-bad module (the official Ham Sandwich, which crashes on map changes, or what the server's module check says), and whether crashes are posted to its Discord channel." }]
}

pub async fn run(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "server_crashes" => async {
            let id = super::catalog::id_arg(a, "server")?;
            let fresh = if a["fresh"].as_bool() == Some(true) { "?fresh=1" } else { "" };
            let mut v = get(ctx, &format!("/api/servers/{id}/crashes{fresh}")).await?;
            let n = super::catalog::limit(a, 20, 100);
            if let Some(list) = v.get_mut("crashes").and_then(Value::as_array_mut) {
                let total = list.len();
                list.truncate(n);
                v["listed"] = json!(list.len());
                v["more"] = json!(total.saturating_sub(n));
            }
            Ok(v)
        }
        .await,
        _ => return None,
    })
}
