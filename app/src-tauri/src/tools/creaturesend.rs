//! "Send my creatures to…" (Creature Studio): the player's own creature types, with their
//! custom bases, models and sounds, go to servers of theirs as one marked block
//! (`own:creatures`) in the server's creatures.ini, through creaturesrv's carry (the server's own
//! sections stay as they are; a file the server has, different, stays the server's). Servers on
//! another computer get it in their local copy, then the usual upload.
//!
//! - `GET  /api/tools/creatures/servers`: the servers, and for each which of the player's types
//!   are there (up to date or changed), which are the server's own, which were sent but aren't
//!   on this computer any more.
//! - `POST /api/tools/creatures/send` {servers, types, remove}: adds `types` and takes `remove`
//!   off each server's block (the rest of the block stays, brought up to date).

use super::creaturepack as cp;
use super::creaturesrv;
use super::{blocking, json_answer, Fail};
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The block the player's own creatures go in on a server.
pub const ID: &str = "own:creatures";
const INI: &str = "addons/amxmodx/configs/gamemodes/creatures.ini";

fn read_ini(game_dir: &Path) -> String {
    std::fs::read(game_dir.join(INI)).map(|b| b.iter().map(|c| *c as char).collect()).unwrap_or_default()
}

/// Sections by lower-case name, several of one name joined (the game reads them in order).
fn by_name(text: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (n, body) in creaturesrv::sections(text) {
        out.entry(n).or_default().push_str(&body);
    }
    out
}

/// The player's own types on this computer (their part of creatures.ini, not installed blocks).
/// Built-in types the player changed stay on this computer: a server keeps its own built-ins.
pub fn own_types(local_game: &Path) -> BTreeMap<String, String> {
    let mut m = by_name(&cp::split_blocks(&read_ini(local_game)).0);
    m.retain(|n, _| !cp::is_builtin(n));
    m
}

/// What one server has of the player's types: [{name, state}], state "current" (as here),
/// "changed" (an older version), "theirs" (the server's own type of that name: never touched),
/// "missing" (not there), "gone" (sent before, not on this computer any more).
pub fn status(local_game: &Path, server_game: &Path) -> Vec<Value> {
    let mine = own_types(local_game);
    let text = read_ini(server_game);
    let (user, blocks) = cp::split_blocks(&text);
    let theirs = by_name(&user);
    let sent = blocks.iter().find(|b| b.id == ID).map(|b| by_name(&b.body)).unwrap_or_default();
    let mut out: Vec<Value> = Vec::new();
    for (n, body) in &mine {
        let state = if theirs.contains_key(n) {
            "theirs"
        } else {
            match sent.get(n) {
                Some(b) if b == body => "current",
                Some(_) => "changed",
                None => "missing",
            }
        };
        out.push(json!({"name": n, "state": state}));
    }
    for n in sent.keys().filter(|n| !mine.contains_key(*n)) {
        out.push(json!({"name": n, "state": "gone"}));
    }
    out
}

/// Puts `add` on a server and takes `remove` off, keeping (and bringing up to date) the rest of
/// what was sent before. Only the player's own types can be named; their custom bases come
/// along. Returns creaturesrv's answer plus "sent" (the types asked for, now there).
pub fn send(local_game: &Path, game: &str, server_game: &Path, add: &[String], remove: &[String]) -> Result<Value, String> {
    let mine = own_types(local_game);
    for n in add {
        if !mine.contains_key(&n.to_ascii_lowercase()) {
            return Err(format!("{n} isn't one of your creature types."));
        }
    }
    let (_, blocks) = cp::split_blocks(&read_ini(server_game));
    // what was sent before: the types the block holds that aren't only there as another one's
    // base (a base comes back by itself while a type that uses it stays)
    let sent = blocks.iter().find(|b| b.id == ID).map(|b| by_name(&b.body)).unwrap_or_default();
    let bases: BTreeSet<String> = sent.values().filter_map(|body| base_line(body)).collect();
    let before: BTreeSet<String> = sent.keys().filter(|n| !bases.contains(*n)).cloned().collect();
    let drop: BTreeSet<String> = remove.iter().map(|n| n.to_ascii_lowercase()).collect();
    // what stays: the types asked for before (still the player's own) and the new ones; the bases
    // come back by themselves from the types that use them
    let mut want: BTreeSet<String> = before.into_iter().filter(|n| mine.contains_key(n)).collect();
    want.extend(add.iter().map(|n| n.to_ascii_lowercase()));
    want.retain(|n| !drop.contains(n));
    let mut v = creaturesrv::carry_types(local_game, game, server_game, ID, &want, "your creatures")?;
    if let Value::Object(o) = &mut v {
        o.insert("sent".into(), json!(want.iter().filter(|n| add.iter().any(|a| a.eq_ignore_ascii_case(n))).collect::<Vec<_>>()));
        o.insert("removed".into(), json!(drop.iter().collect::<Vec<_>>()));
    }
    Ok(v)
}

/// A section's `base =` (the last one), lower case.
fn base_line(body: &str) -> Option<String> {
    body.lines().filter_map(|l| l.split_once('=')).filter(|(k, _)| k.trim().eq_ignore_ascii_case("base")).map(|(_, v)| v.trim().to_ascii_lowercase()).last()
}

// ---- routes -----------------------------------------------------------------------------------

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

/// This computer's game the Studio edits (where the player's types are).
fn source(st: &Shared) -> Result<std::sync::Arc<crate::game::Game>, Fail> {
    let g = st.local_game().ok_or_else(|| Fail::new(StatusCode::OK, "Set up the game on this computer first."))?;
    if g.remote.is_some() {
        return Err(Fail::new(StatusCode::OK, "Pick your own game or a server on this computer in Creature Studio first."));
    }
    Ok(g)
}

/// The servers creatures can go to: every server of the player's but the one being edited.
fn targets(g: &crate::game::Game) -> Vec<crate::servers::Entry> {
    crate::servers::load().into_iter().filter(|e| e.ready && !crate::steam::same_dir(&e.dir.join(&e.game), &g.dir)).collect()
}

fn server_json(g: &crate::game::Game, e: &crate::servers::Entry) -> Value {
    let game_dir = e.dir.join(&e.game);
    let same_game = e.game == g.name;
    json!({"id": e.id, "name": e.name, "remote": e.remote.is_some(), "game": e.game, "sameGame": same_game, "types": status(&g.dir, &game_dir)})
}

async fn api_servers(State(st): State<Shared>) -> Response {
    json_answer(
        blocking(move || {
            let g = source(&st)?;
            let list: Vec<Value> = targets(&g).iter().map(|e| server_json(&g, e)).collect();
            Ok(json!({"servers": list}))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct SendReq {
    servers: Vec<String>,
    #[serde(default)]
    types: Vec<String>,
    #[serde(default)]
    remove: Vec<String>,
}

async fn api_send(State(st): State<Shared>, h: HeaderMap, Json(r): Json<SendReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let g = source(&st)?;
            if r.servers.is_empty() || (r.types.is_empty() && r.remove.is_empty()) {
                return Err(Fail::new(StatusCode::OK, "Pick a server and a creature type."));
            }
            let all = targets(&g);
            let mut results: Vec<Value> = Vec::new();
            for id in &r.servers {
                let Some(e) = all.iter().find(|e| e.id == *id) else {
                    results.push(json!({"id": id, "ok": false, "error": "That server isn't in your list any more."}));
                    continue;
                };
                let game_dir = e.dir.join(&e.game);
                let mut one = match send(&g.dir, &g.name, &game_dir, &r.types, &r.remove) {
                    Ok(v) => v,
                    Err(err) => {
                        results.push(json!({"id": e.id, "name": e.name, "ok": false, "error": err}));
                        continue;
                    }
                };
                crate::servers::mark_changed(&e.id);
                // (a server on another computer: its local copy has it now; then the upload)
                let uploaded = if e.remote.is_some() {
                    Some(match crate::servers::sync(e, false) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(err),
                    })
                } else {
                    None
                };
                if let Value::Object(o) = &mut one {
                    o.insert("id".into(), json!(e.id));
                    o.insert("name".into(), json!(e.name));
                    o.insert("ok".into(), json!(!matches!(uploaded, Some(Err(_)))));
                    if let Some(Err(err)) = &uploaded {
                        o.insert("error".into(), json!(format!("Saved in its copy here, but the upload failed ({err}). It goes up with the next upload.")));
                    }
                    o.insert("types".into(), json!(status(&g.dir, &game_dir)));
                }
                results.push(one);
            }
            let good = results.iter().filter(|x| x["ok"] == true).count();
            // (only taking types off says so, not "Sent")
            let only_removing = r.types.is_empty();
            let (done, none) = if only_removing { ("Removed from", "Nothing was removed.") } else { ("Sent to", "Nothing was sent.") };
            let message = match (good, results.len()) {
                (0, _) => none.to_string(),
                (n, t) if n == t => format!("{done} {} server{}. They use it from their next map.", n, if n == 1 { "" } else { "s" }),
                (n, t) => format!("{done} {n} of {t} servers."),
            };
            Ok(json!({"ok": good > 0, "message": message, "results": results}))
        })
        .await,
    )
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/creatures/servers", get(api_servers)).route("/api/tools/creatures/send", post(api_send))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(root: &Path, name: &str, ini: &str) -> std::path::PathBuf {
        let g = root.join(name).join("cstrike");
        std::fs::create_dir_all(g.join("addons/amxmodx/configs/gamemodes")).unwrap();
        std::fs::write(g.join(INI), ini).unwrap();
        g
    }

    #[test]
    fn sends_updates_and_removes_the_players_own_types() {
        let root = std::env::temp_dir().join(format!("ls-creaturesend-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let local = game(&root, "mine", "; mine\n[zombie]\nhp = 150\n\n[tough]\nbase = brute\nhp = 900\n\n[crawler]\nbase = tough\nmodel = models/qa/crawler.mdl\nhp = 77\n\n[sprinter]\nbase = runner\nspeed = 400\n");
        std::fs::create_dir_all(local.join("models/qa")).unwrap();
        std::fs::write(local.join("models/qa/crawler.mdl"), b"IDST mine").unwrap();
        // the server: its own notes and type, and a model file of its own at the same path
        let server_ini = "; the server's own\n[sprinter]\nbase = zombie\nhp = 5\n";
        let server = game(&root, "srv", server_ini);
        // nothing sent yet
        let st = status(&local, &server);
        assert!(st.contains(&json!({"name": "crawler", "state": "missing"})) && st.contains(&json!({"name": "sprinter", "state": "theirs"})), "{st:?}");
        // send crawler: its custom base comes along, the model is copied
        let v = send(&local, "cstrike", &server, &["crawler".into()], &[]).unwrap();
        assert_eq!(v["types"], json!(["tough", "crawler"]), "{v}");
        let text = read_ini(&server);
        assert!(text.starts_with(server_ini), "the server's own part stays as it was: {text}");
        assert!(text.contains("; >>> from own:creatures") && text.contains("[crawler]") && text.contains("[tough]"));
        assert_eq!(std::fs::read(server.join("models/qa/crawler.mdl")).unwrap(), b"IDST mine");
        assert!(status(&local, &server).contains(&json!({"name": "crawler", "state": "current"})));
        // a changed built-in isn't one to send
        assert!(send(&local, "cstrike", &server, &["zombie".into()], &[]).is_err());
        // a type the server has of its own isn't sent over it
        let v = send(&local, "cstrike", &server, &["sprinter".into()], &[]).unwrap();
        assert!(v["skipped"].as_array().unwrap().contains(&json!("sprinter")));
        assert!(read_ini(&server).starts_with(server_ini));
        // changed here: the server's copy reads "changed"; Update brings it up to date
        std::fs::write(local.join(INI), read_ini(&local).replace("hp = 77", "hp = 88")).unwrap();
        assert!(status(&local, &server).contains(&json!({"name": "crawler", "state": "changed"})));
        send(&local, "cstrike", &server, &["crawler".into()], &[]).unwrap();
        assert!(read_ini(&server).contains("hp = 88") && !read_ini(&server).contains("hp = 77"));
        // a file a server already has, different, stays the server's (never written over)
        let other = game(&root, "srv2", "");
        std::fs::create_dir_all(other.join("models/qa")).unwrap();
        std::fs::write(other.join("models/qa/crawler.mdl"), b"IDST theirs").unwrap();
        let v = send(&local, "cstrike", &other, &["crawler".into()], &[]).unwrap();
        assert_eq!(v["kept"], json!(["models/qa/crawler.mdl"]), "{v}");
        assert_eq!(std::fs::read(other.join("models/qa/crawler.mdl")).unwrap(), b"IDST theirs");
        send(&local, "cstrike", &other, &[], &["crawler".into()]).unwrap();
        assert_eq!(std::fs::read(other.join("models/qa/crawler.mdl")).unwrap(), b"IDST theirs", "and taking the block off leaves it");
        // not one of the player's types: refused
        assert!(send(&local, "cstrike", &server, &["boss".into()], &[]).is_err());
        // Remove: the block goes, the server's own part is byte-identical
        send(&local, "cstrike", &server, &[], &["crawler".into()]).unwrap();
        assert_eq!(read_ini(&server), server_ini);
        assert!(status(&local, &server).contains(&json!({"name": "crawler", "state": "missing"})));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_type_removed_here_shows_as_gone_there() {
        let root = std::env::temp_dir().join(format!("ls-creaturesend-gone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let local = game(&root, "mine", "[a_one]\nbase = zombie\nhp = 1\n\n[b_two]\nbase = runner\n");
        let server = game(&root, "srv", "");
        send(&local, "cstrike", &server, &["a_one".into(), "b_two".into()], &[]).unwrap();
        std::fs::write(local.join(INI), "[b_two]\nbase = runner\n").unwrap();
        let st = status(&local, &server);
        assert!(st.contains(&json!({"name": "a_one", "state": "gone"})) && st.contains(&json!({"name": "b_two", "state": "current"})), "{st:?}");
        // the next send drops what isn't the player's any more
        send(&local, "cstrike", &server, &["b_two".into()], &[]).unwrap();
        assert!(!read_ini(&server).contains("[a_one]"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
