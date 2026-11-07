//! Lists shared between servers: admins and VIPs, bans, swear filter words, the welcome
//! page and the timed chat messages. Each list has a group of members ("local:<game>"
//! for the player's own CS 1.6 or Condition Zero, or server ids); an edit to the list on
//! one member is copied to the others. Plugins, mods and every other setting stay per
//! server.
//!
//! Only requests that edit a list pass it on (server.rs, with_target, by path). Each
//! member's contents are remembered as a hash, so a request that didn't change this
//! member's list (another setting on the same page) passes nothing on. Bans are merged,
//! not replaced: the ones added or lifted since the last share go to the others, and
//! bans made in the game on one server stay where they are.

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
    /// each member's list as last seen (a hash)
    #[serde(default)]
    hashes: BTreeMap<String, String>,
    /// bans: the shared set as last passed on ("id STEAM_..." / "ip ...")
    #[serde(default)]
    base: Vec<String>,
}

type Config = BTreeMap<String, Group>;

static LOCK: Mutex<()> = Mutex::new(());
static STATE: OnceLock<Shared> = OnceLock::new();

pub fn init(st: &Shared) {
    if STATE.set(st.clone()).is_ok() {
        watch_bans();
    }
}

/// Bans made in the game (amx_ban, the vote ban's permanent ones) are written to the
/// game's own ban files, not through the app: shared ban lists are looked at once a
/// minute so those reach the other servers too.
fn watch_bans() {
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let Some(group) = load().remove("bans").filter(|g| g.members.len() >= 2) else { continue };
        for m in &group.members {
            check(m, "bans", false);
        }
    });
}

fn file() -> std::path::PathBuf {
    crate::sys::app_dir().join("shared-lists.json")
}

fn load() -> Config {
    let mut c: Config = std::fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // groups saved before your own game was named by which game it is ("local")
    if let Some(me) = local_id() {
        for g in c.values_mut() {
            for m in g.members.iter_mut().filter(|m| *m == "local") {
                *m = me.clone();
            }
            if let Some(h) = g.hashes.remove("local") {
                g.hashes.insert(me.clone(), h);
            }
            g.members.dedup();
        }
    }
    c
}

fn save(c: &Config) -> Result<(), String> {
    crate::sys::write_atomic(&file(), serde_json::to_string_pretty(c).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

/// "local:cstrike" while CS 1.6 is the player's game, and so on (switching game changes it).
fn local_id() -> Option<String> {
    STATE.get()?.local_game().map(|g| format!("local:{}", g.name))
}

/// The game for a member, if it's there right now (a "local:" one only while it's open).
fn game(member: &str) -> Option<Arc<Game>> {
    if let Some(name) = member.strip_prefix("local:") {
        return STATE.get()?.local_game().filter(|g| g.name == name);
    }
    crate::servers::game_for(member, || None)
}

/// The list a change request edits, from its path.
pub fn list_for(path: &str) -> Option<&'static str> {
    Some(match path {
        "/api/admins/ban" | "/api/admins/unban" => "bans",
        "/api/admins/person" | "/api/admins/remove" | "/api/admins/settings" => "admins",
        "/api/community/settings" => "filter",
        "/api/welcome" => "welcome",
        "/api/extras/settings" => "messages",
        _ => return None,
    })
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

/// Copies a list from one member to the others (whole: when a member joins); servers on
/// other machines get the new files uploaded.
fn share(list: &str, from: &str, members: &[String]) -> Result<(), String> {
    let src = game(from).ok_or("That server isn't set up yet.")?;
    each_other(from, members, |g| copy(list, &src, g))
}

/// Runs `f` for every member but `from`, uploading to servers on other machines after.
fn each_other(from: &str, members: &[String], f: impl Fn(&Game) -> Result<(), String>) -> Result<(), String> {
    let mut failed = Vec::new();
    for m in members.iter().filter(|m| *m != from) {
        // the player's other game isn't open: it gets the list when it's next shared
        let Some(g) = game(m) else {
            if !m.starts_with("local:") {
                failed.push(format!("{} (isn't set up)", name_of(m)));
            }
            continue;
        };
        match f(&g) {
            Ok(()) if crate::servers::get(m).is_some_and(|e| e.remote.is_some()) => crate::servers::sync_settings(m),
            Ok(()) => {}
            Err(e) => failed.push(format!("{} ({e})", name_of(m))),
        }
    }
    if !failed.is_empty() {
        return Err(format!("Couldn't copy it to {}.", failed.join(", ")));
    }
    Ok(())
}

/// Every member's list as it is now.
fn remember(list: &str, group: &mut Group) {
    group.hashes = group.members.iter().filter_map(|m| game(m).map(|g| (m.clone(), print(list, &g)))).collect();
}

fn name_of(target: &str) -> String {
    if target.starts_with("local:") {
        return "your game".into();
    }
    crate::servers::get(target).map(|e| e.name).unwrap_or_else(|| target.to_string())
}

/// After a request that edits `list` on `target` ("local" or a server id): if this
/// member's list really changed, the change goes to the other members.
pub fn after_change(target: &str, list: &str) {
    let member = if target == "local" { local_id() } else { Some(target.to_string()) };
    if let Some(member) = member {
        check(&member, list, true);
    }
}

/// Passes `member`'s list on if it changed since it was last seen. `edited`: the app just
/// changed it; otherwise it's the minute check, which trusts only a settled ban file.
fn check(member: &str, list: &str, edited: bool) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let member = member.to_string();
    let mut c = load();
    let Some(group) = c.get_mut(list).filter(|g| g.members.len() >= 2 && g.members.contains(&member)) else { return };
    let Some(g) = game(&member) else { return };
    let now = print(list, &g);
    if group.hashes.get(&member) == Some(&now) {
        return;
    }
    let done = if list == "bans" {
        // what was added or lifted here since the last share, and only that
        // this member's bans couldn't be read: nothing is worked out from that
        let Ok(mine) = crate::admins::ban_set(&g) else { return };
        if !edited {
            // the game may be halfway through writing the file: the same answer twice, a
            // moment apart, or nothing is passed on this time
            std::thread::sleep(std::time::Duration::from_secs(2));
            if crate::admins::ban_set(&g).ok().as_ref() != Some(&mine) {
                return;
            }
            // every shared ban gone at once isn't something the game does by itself
            if mine.is_empty() && !group.base.is_empty() {
                return;
            }
        }
        let mine: Vec<String> = mine.into_iter().collect();
        let add: Vec<String> = mine.iter().filter(|b| !group.base.contains(b)).cloned().collect();
        let lift: Vec<String> = group.base.iter().filter(|b| !mine.contains(b)).cloned().collect();
        let r = each_other(&member, &group.members, |o| crate::admins::apply_bans(o, &add, &lift));
        // only once every member has them: a server that was off or out of reach gets the
        // same additions and lifts again at the next check (the others already have them,
        // and applying them twice changes nothing)
        if r.is_err() {
            return;
        }
        group.base = group.base.iter().filter(|b| !lift.contains(b)).cloned().chain(add).collect();
        r
    } else {
        share(list, &member, &group.members)
    };
    if done.is_ok() {
        remember(list, group);
    } else {
        group.hashes.insert(member, now);
    }
    let _ = save(&c);
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
    let mut targets: Vec<Value> = local_id().map(|id| json!({"id": id, "name": "Your game"})).into_iter().collect();
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
            if !m.starts_with("local:") && !crate::servers::get(&m).is_some_and(|e| e.ready) {
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
        if members.len() >= 2 {
            share(&req.list, &req.from, &members)?;
            let mut group = Group { members, ..Default::default() };
            remember(&req.list, &mut group);
            if req.list == "bans" {
                let from = game(&req.from).ok_or("That server isn't set up yet.")?;
                group.base = crate::admins::ban_set(&from)?.into_iter().collect();
            }
            c.insert(req.list.clone(), group);
        } else {
            c.remove(&req.list);
        }
        save(&c)?;
        Ok(json!({"ok": true}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/lists", route_get(api_get).post(api_set))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_list_edits_pass_on() {
        assert_eq!(list_for("/api/admins/ban"), Some("bans"));
        assert_eq!(list_for("/api/admins/person"), Some("admins"));
        assert_eq!(list_for("/api/community/settings"), Some("filter"));
        assert_eq!(list_for("/api/extras/settings"), Some("messages"));
        assert_eq!(list_for("/api/welcome"), Some("welcome"));
        assert_eq!(list_for("/api/maps/change"), None);
        assert_eq!(list_for("/api/extras/rotation"), None);
    }
}
