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

// ---- modes that load only the types they list -----------------------------------------------------

const GM: &str = "addons/amxmodx/configs/gamemodes";

fn read_latin1(p: &Path) -> String {
    std::fs::read(p).map(|b| b.iter().map(|c| *c as char).collect()).unwrap_or_default()
}

/// The server's modes (modes.ini): (id, shown name).
fn modes(server_game: &Path) -> Vec<(String, String)> {
    read_latin1(&server_game.join(GM).join("modes.ini"))
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() || t.starts_with(';') || t.starts_with("//") {
                return None;
            }
            let id = t.split_whitespace().next()?.trim_matches('"').to_ascii_lowercase();
            let name = t.split('"').nth(1).filter(|n| !n.trim().is_empty()).map(|n| n.trim().to_string()).unwrap_or_else(|| id.clone());
            (super::creatures::mode_ok(&id)).then_some((id, name))
        })
        .collect()
}

/// A mode cfg's `ls_creature_precache` list (None: it doesn't set one, or sets "all"/empty).
fn cfg_list(text: &str) -> Option<Vec<String>> {
    let v = text.lines().filter_map(|l| {
        let t = l.trim();
        let rest = t.strip_prefix("ls_creature_precache")?;
        (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.split("//").next().unwrap_or("").trim().trim_matches('"').trim().to_string())
    }).last()?;
    let words: Vec<String> = v.split_whitespace().map(|w| w.to_ascii_lowercase()).collect();
    (!words.is_empty() && !words.iter().any(|w| w == "all")).then_some(words)
}

/// The server's modes that load only the creature types they list, each with how ("waves": a
/// waves.ini section, Last Stand's built-in table when it has none; "precache": its cfg's
/// ls_creature_precache list) and what it lists. Modes that load every type aren't here.
pub fn limiting_modes(server_game: &Path) -> Vec<Value> {
    let waves = super::creatures::parse_waves(&read_latin1(&server_game.join(super::creatures::WAVES_REL)));
    let mut out = Vec::new();
    for (id, name) in modes(server_game) {
        let section = waves.iter().find(|w| w.mode == id).cloned();
        let set = section.or_else(|| (id == "laststand").then(super::creatures::laststand_default));
        if let Some(w) = set {
            out.push(json!({"id": id, "name": name, "how": "waves", "types": super::creatures::wave_types(&w, 1000)}));
            continue;
        }
        if let Some(list) = cfg_list(&read_latin1(&server_game.join(GM).join(format!("{id}.cfg")))) {
            out.push(json!({"id": id, "name": name, "how": "precache", "types": list}));
        }
    }
    out
}

/// Adds `types` to one mode's list on a server, the way that mode reads it: its waves.ini rows
/// (other types and other sections as they are; Last Stand with no section starts from its own
/// lineup with the built-in boss, pack and heavy rounds kept, own_rounds), each new type a
/// tenth of each row; or its cfg's ls_creature_precache list (the rest of the cfg as it is).
/// Returns the types added (none: nothing written).
pub fn add_to_mode(server_game: &Path, mode: &str, types: &[String]) -> Result<Vec<String>, String> {
    let m = limiting_modes(server_game).into_iter().find(|m| m["id"] == mode).ok_or("That mode on the server loads every type already.")?;
    // (in the mode's own order: a cfg list keeps its order)
    let have: Vec<String> = m["types"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)).collect();
    let add: Vec<String> = types.iter().map(|t| t.to_ascii_lowercase()).filter(|t| !have.contains(t)).collect::<BTreeSet<_>>().into_iter().collect();
    if add.is_empty() {
        return Ok(add);
    }
    let (path, new) = if m["how"] == "waves" {
        let path = server_game.join(super::creatures::WAVES_REL);
        let old = read_latin1(&path);
        let mut w = super::creatures::parse_waves(&old).into_iter().find(|w| w.mode == mode).unwrap_or_else(super::creatures::laststand_default);
        for r in w.rows.iter_mut() {
            let total: u32 = r.mix.iter().map(|(_, n)| *n).sum();
            for t in &add {
                r.mix.push((t.clone(), (total / 10).max(1)));
            }
        }
        (path, super::creatures::write_waves(&old, &w))
    } else {
        let path = server_game.join(GM).join(format!("{mode}.cfg"));
        let old = read_latin1(&path);
        let mut list: Vec<String> = have.clone();
        list.extend(add.iter().cloned());
        let line = format!("ls_creature_precache \"{}\"", list.join(" "));
        // the last line that sets it is the one the game keeps: that one is replaced
        let at = old.split_inclusive('\n').enumerate().filter(|(_, l)| l.trim().strip_prefix("ls_creature_precache").is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace))).map(|(i, _)| i).last();
        let new: String = old.split_inclusive('\n').enumerate().map(|(i, l)| if Some(i) == at { format!("{line}{}", if l.ends_with("\r\n") { "\r\n" } else if l.ends_with('\n') { "\n" } else { "" }) } else { l.to_string() }).collect();
        (path, new)
    };
    crate::safety::before("Creature Studio", &format!("Add your creatures to {}'s list on a server", m["name"].as_str().unwrap_or(mode)), &path);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save it ({e})."))?;
    }
    crate::sys::make_writable(&path);
    crate::sys::write_latin1_atomic(&path, &new).map_err(|e| format!("Couldn't save it ({e})."))?;
    Ok(add)
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
    json!({"id": e.id, "name": e.name, "remote": e.remote.is_some(), "game": e.game, "sameGame": same_game, "limits": limiting_modes(&game_dir), "types": status(&g.dir, &game_dir)})
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

#[derive(Deserialize)]
struct ModesReq {
    servers: Vec<String>,
    /// the modes to add to on each server; empty: every mode there that limits its types
    #[serde(default)]
    modes: Vec<String>,
    types: Vec<String>,
}

/// "Add to <Mode>'s creatures" / "Add to all of them": only the player's own types.
async fn api_modes(State(st): State<Shared>, h: HeaderMap, Json(r): Json<ModesReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let g = source(&st)?;
            let mine = own_types(&g.dir);
            if r.types.is_empty() || r.types.iter().any(|t| !mine.contains_key(&t.to_ascii_lowercase())) {
                return Err(Fail::new(StatusCode::OK, "Pick your own creature types."));
            }
            let all = targets(&g);
            let mut results: Vec<Value> = Vec::new();
            let mut added_any = 0;
            for id in &r.servers {
                let Some(e) = all.iter().find(|e| e.id == *id) else { continue };
                let game_dir = e.dir.join(&e.game);
                let wanted: Vec<String> = if r.modes.is_empty() { limiting_modes(&game_dir).iter().filter_map(|m| m["id"].as_str().map(String::from)).collect() } else { r.modes.clone() };
                let mut added: Vec<Value> = Vec::new();
                let mut error: Option<String> = None;
                for m in &wanted {
                    match add_to_mode(&game_dir, m, &r.types) {
                        Ok(a) if !a.is_empty() => added.push(json!({"mode": m, "types": a})),
                        Ok(_) => {}
                        Err(err) => error = Some(err),
                    }
                }
                if !added.is_empty() {
                    added_any += added.len();
                    crate::servers::mark_changed(&e.id);
                    if e.remote.is_some() {
                        if let Err(err) = crate::servers::sync(e, false) {
                            error = Some(format!("Saved in its copy here, but the upload failed ({err}). It goes up with the next upload."));
                        }
                    }
                }
                results.push(json!({"id": e.id, "name": e.name, "ok": error.is_none(), "error": error, "added": added, "limits": limiting_modes(&game_dir)}));
            }
            let message = if added_any == 0 { "They're in those lists already.".to_string() } else { "Added. The modes load them from the next map.".to_string() };
            Ok(json!({"ok": results.iter().any(|x| x["ok"] == true), "message": message, "results": results}))
        })
        .await,
    )
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/creatures/servers", get(api_servers))
        .route("/api/tools/creatures/send", post(api_send))
        .route("/api/tools/creatures/send-modes", post(api_modes))
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
    fn modes_that_list_their_types_are_found_and_take_the_type() {
        let root = std::env::temp_dir().join(format!("ls-creaturesend-modes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let server = game(&root, "srv", "");
        let gm = server.join(GM);
        std::fs::write(gm.join("modes.ini"), "; modes\nclassic \"Classic\" \"\" \"\" \"\"\nlaststand \"Last Stand\" \"\" \"\" \"\"\nzday \"Zombie Day\" \"\" \"\" \"\"\nopen \"Open Hunt\" \"\" \"\" \"\"\n").unwrap();
        // Zombie Day lists its types in its cfg; Open Hunt loads all; Classic sets nothing
        let zcfg = "// zday\nmp_roundtime 3\nls_creature_precache \"zombie runner\" // the list\nsv_gravity 800\n";
        std::fs::write(gm.join("zday.cfg"), zcfg).unwrap();
        std::fs::write(gm.join("open.cfg"), "ls_creature_precache all\n").unwrap();
        let other = "; my notes\n[othermode]\nboss =\nwave 1 = zombie 1\n";
        std::fs::write(server.join(super::super::creatures::WAVES_REL), other).unwrap();
        let lim = limiting_modes(&server);
        let names: Vec<&str> = lim.iter().map(|m| m["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["Last Stand", "Zombie Day"], "{lim:?}");
        assert!(!lim[0]["types"].as_array().unwrap().contains(&json!("qa_crawler")));
        // Last Stand with no section: its own lineup, own_rounds, other sections byte for byte
        assert_eq!(add_to_mode(&server, "laststand", &["qa_crawler".into()]).unwrap(), vec!["qa_crawler"]);
        let text = read_latin1(&server.join(super::super::creatures::WAVES_REL));
        assert!(text.starts_with(other), "{text}");
        let ls = super::super::creatures::parse_waves(&text).into_iter().find(|w| w.mode == "laststand").unwrap();
        assert!(ls.own_rounds);
        assert_eq!(ls.rows[0].mix, vec![("zombie".to_string(), 100), ("qa_crawler".into(), 10)]);
        // the cfg list: only its line changes
        assert_eq!(add_to_mode(&server, "zday", &["qa_crawler".into()]).unwrap(), vec!["qa_crawler"]);
        assert_eq!(read_latin1(&gm.join("zday.cfg")), "// zday\nmp_roundtime 3\nls_creature_precache \"zombie runner qa_crawler\"\nsv_gravity 800\n");
        // both list it now; again: nothing written
        assert!(limiting_modes(&server).iter().all(|m| m["types"].as_array().unwrap().contains(&json!("qa_crawler"))));
        assert!(add_to_mode(&server, "laststand", &["qa_crawler".into()]).unwrap().is_empty());
        assert!(add_to_mode(&server, "zday", &["qa_crawler".into()]).unwrap().is_empty());
        assert_eq!(read_latin1(&server.join(super::super::creatures::WAVES_REL)), text);
        // a mode that loads everything isn't one to add to
        assert!(add_to_mode(&server, "open", &["qa_crawler".into()]).is_err());
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
