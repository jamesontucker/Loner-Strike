//! Lists shared between servers: admins and VIPs, bans, swear filter words, the welcome
//! page and the timed chat messages. Each list has a group of members ("local" for the
//! player's own game, or server ids); a change saved on one member is copied to the
//! others. Plugins, mods and every other setting stay per server.
//!
//! After every successful change request (server.rs, with_target) the list's contents
//! on that member are compared with what was last shared; when they differ, the others
//! get a copy. What was last shared is kept as a hash in shared-lists.json, so it holds
//! across restarts.

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get as route_get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

/// (id, what the page calls it)
pub const LISTS: &[(&str, &str)] = &[
    ("admins", "Admins and VIPs"),
    ("bans", "Bans"),
    ("filter", "Swear filter words"),
    ("welcome", "Welcome page"),
    ("messages", "Chat messages"),
];

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
struct Group {
    members: Vec<String>,
    /// what the members had when last shared
    #[serde(default)]
    hash: String,
}

type Config = BTreeMap<String, Group>;

static LOCK: Mutex<()> = Mutex::new(());
static STATE: OnceLock<Shared> = OnceLock::new();

pub fn init(st: &Shared) {
    let _ = STATE.set(st.clone());
}

fn file() -> std::path::PathBuf {
    crate::sys::app_dir().join("shared-lists.json")
}

fn load() -> Config {
    std::fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(c: &Config) -> Result<(), String> {
    crate::sys::write_atomic(&file(), serde_json::to_string_pretty(c).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

fn game(target: &str) -> Option<Arc<Game>> {
    if target == "local" {
        return STATE.get()?.local_game();
    }
    crate::servers::game_for(target, || None)
}

fn print(list: &str, g: &Game) -> String {
    let text = match list {
        "admins" => crate::admins::shared_admins(g),
        "bans" => crate::admins::shared_bans(g),
        "filter" => crate::community::shared_words(g),
        "welcome" => crate::welcome::shared(&g.data),
        "messages" => crate::extras::shared_ads(g),
        _ => String::new(),
    };
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(text.as_bytes()))
}

fn copy(list: &str, from: &Game, to: &Game) -> Result<(), String> {
    match list {
        "admins" => crate::admins::copy_admins(from, to),
        "bans" => crate::admins::copy_bans(from, to),
        "filter" => crate::community::copy_words(from, to),
        "welcome" => crate::welcome::copy(&from.data, &to.data),
        "messages" => crate::extras::copy_ads(from, to),
        _ => Ok(()),
    }
}

/// Copies a list from one member to the others; servers on other machines get the new
/// files uploaded.
fn share(list: &str, from: &str, members: &[String]) -> Result<String, String> {
    let src = game(from).ok_or("That server isn't set up yet.")?;
    let mut failed = Vec::new();
    for m in members.iter().filter(|m| *m != from) {
        let done = game(m).ok_or_else(|| "isn't set up".to_string()).and_then(|g| copy(list, &src, &g));
        match done {
            Ok(()) if m != "local" && crate::servers::get(m).is_some_and(|e| e.remote.is_some()) => crate::servers::sync_settings(m),
            Ok(()) => {}
            Err(e) => failed.push(format!("{} ({e})", name_of(m))),
        }
    }
    if !failed.is_empty() {
        return Err(format!("Couldn't copy it to {}.", failed.join(", ")));
    }
    Ok(print(list, &src))
}

fn name_of(target: &str) -> String {
    if target == "local" {
        return "your game".into();
    }
    crate::servers::get(target).map(|e| e.name).unwrap_or_else(|| target.to_string())
}

/// After a change on `target`: any shared list it changed goes to the other members.
pub fn after_change(target: &str) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = load();
    let Some(g) = game(target) else { return };
    let mut changed = false;
    for (list, group) in c.iter_mut() {
        if group.members.len() < 2 || !group.members.iter().any(|m| m == target) {
            continue;
        }
        let now = print(list, &g);
        if now == group.hash {
            continue;
        }
        if let Ok(h) = share(list, target, &group.members) {
            group.hash = h;
            changed = true;
        }
    }
    if changed {
        let _ = save(&c);
    }
}

/// A server that's gone leaves every group.
pub fn forget(target: &str) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = load();
    let before = c.values().map(|g| g.members.len()).sum::<usize>();
    for g in c.values_mut() {
        g.members.retain(|m| m != target);
    }
    if c.values().map(|g| g.members.len()).sum::<usize>() != before {
        let _ = save(&c);
    }
}

// ---- API -----------------------------------------------------------------------------

async fn api_get() -> Json<Value> {
    let c = load();
    let mut targets = vec![json!({"id": "local", "name": "Your game"})];
    targets.extend(crate::servers::load().into_iter().filter(|e| e.ready).map(|e| json!({"id": e.id, "name": e.name})));
    let lists: BTreeMap<&str, Vec<String>> = LISTS.iter().map(|(id, _)| (*id, c.get(*id).map(|g| g.members.clone()).unwrap_or_default())).collect();
    Json(json!({"lists": lists, "names": LISTS.iter().map(|(id, n)| (id.to_string(), n.to_string())).collect::<BTreeMap<_, _>>(), "targets": targets}))
}

#[derive(Deserialize)]
struct SetReq {
    list: String,
    /// the members sharing it (fewer than two: not shared)
    members: Vec<String>,
    /// whose list the others get
    from: String,
}

async fn api_set(headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, Json(req): Json<SetReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) || on.is_none() {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        if !LISTS.iter().any(|(id, _)| *id == req.list) {
            return Err("Unknown list.".into());
        }
        let mut members: Vec<String> = Vec::new();
        for m in req.members {
            if m != "local" && !crate::servers::get(&m).is_some_and(|e| e.ready) {
                return Err("One of those servers isn't there any more, or isn't set up yet.".into());
            }
            if !members.contains(&m) {
                members.push(m);
            }
        }
        if members.len() >= 2 && !members.contains(&req.from) {
            return Err("Pick which list to share.".into());
        }
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut c = load();
        let hash = if members.len() >= 2 { share(&req.list, &req.from, &members)? } else { String::new() };
        if members.len() >= 2 {
            c.insert(req.list.clone(), Group { members, hash });
        } else {
            c.remove(&req.list);
        }
        save(&c)?;
        Ok(json!({"ok": true}))
    })
    .await
    .unwrap_or_else(|_| Err("Something went wrong.".into()));
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/lists", route_get(api_get).post(api_set))
}
