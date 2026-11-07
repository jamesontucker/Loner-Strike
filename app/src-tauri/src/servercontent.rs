//! Weapon skins and player models for a dedicated server, from Get more.
//!
//! A player's game never downloads a file it already has, so a skin that replaces
//! models/v_ak47.mdl on a server would reach nobody. Here a server's skins and player
//! models go in under their own names (models/ls/<mod>/v_ak47.mdl,
//! models/player/ls<mod>/ls<mod>.mdl), and gm_content.amxx loads them and swaps them in
//! (per weapon; for everyone, one team, or admins), so players download them when they
//! join. The list lives in the server's data folder:
//!     content.json   the app's record (what, where, for whom, on or off)
//!     content.ini    what the plugin reads, written from it
//! gm_content.amxx also counts what each map loads against the engine's 512-a-kind
//! precache limit (over it the server stops) and writes content_status.json.

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const DATA: &str = "addons/amxmodx/data/gamemodes";
/// The engine's precache limit for models, sounds and other files, each
pub const PRECACHE_LIMIT: u32 = 512;
/// Who sees an item
pub const WHO: &[&str] = &["all", "ct", "t", "admin"];

static LOCK: Mutex<()> = Mutex::new(());

/// The weapon a view (v_), player (p_) or world (w_) model is for, from its file name:
/// ("weapon_ak47", 'v') for v_ak47.mdl. None for anything else (shields, hands...).
pub fn weapon_of(file_name: &str) -> Option<(&'static str, char)> {
    const WEAPONS: &[(&str, &str)] = &[
        ("ak47", "weapon_ak47"), ("m4a1", "weapon_m4a1"), ("awp", "weapon_awp"), ("deagle", "weapon_deagle"),
        ("usp", "weapon_usp"), ("glock18", "weapon_glock18"), ("p228", "weapon_p228"), ("elite", "weapon_elite"),
        ("fiveseven", "weapon_fiveseven"), ("m3", "weapon_m3"), ("xm1014", "weapon_xm1014"), ("mac10", "weapon_mac10"),
        ("tmp", "weapon_tmp"), ("mp5", "weapon_mp5navy"), ("ump45", "weapon_ump45"), ("p90", "weapon_p90"),
        ("aug", "weapon_aug"), ("sg552", "weapon_sg552"), ("galil", "weapon_galil"), ("famas", "weapon_famas"),
        ("scout", "weapon_scout"), ("sg550", "weapon_sg550"), ("g3sg1", "weapon_g3sg1"), ("m249", "weapon_m249"),
        ("knife", "weapon_knife"), ("hegrenade", "weapon_hegrenade"), ("flashbang", "weapon_flashbang"),
        ("smokegrenade", "weapon_smokegrenade"), ("c4", "weapon_c4"), ("backpack", "weapon_c4"),
    ];
    let lower = file_name.to_lowercase();
    let stem = lower.strip_suffix(".mdl")?;
    let (kind, rest) = stem.split_once('_')?;
    let kind = match kind {
        "v" => 'v',
        "p" => 'p',
        "w" => 'w',
        _ => return None,
    };
    WEAPONS.iter().find(|(n, _)| *n == rest).map(|(_, w)| (*w, kind))
}

/// What a download brings for a server: (files to copy: source -> path in the game, the
/// item's weapons or player model). Weapon skins keep their file names in their own
/// folder; a player model is renamed ls<mod>, its texture file (…T.mdl) with it.
pub fn plan(section: &str, mod_id: i64, files: &[PathBuf]) -> Result<(Vec<(PathBuf, String)>, Value), String> {
    let name_of = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tag = format!("ls{}", mod_id.unsigned_abs());
    let mut out = Vec::new();
    match section {
        "skins" => {
            let mut weapons = serde_json::Map::new();
            for f in files {
                let n = name_of(f);
                let Some((weapon, kind)) = weapon_of(&n) else { continue };
                let dest = format!("models/ls/{tag}/{}", n.to_lowercase());
                let entry = weapons.entry(weapon).or_insert_with(|| json!({}));
                if entry.get(kind.to_string()).is_none() {
                    entry[kind.to_string()] = json!(dest);
                    out.push((f.clone(), dest));
                }
            }
            if out.is_empty() {
                return Err("That download has no weapon models (v_, p_ or w_ files) to use on a server.".into());
            }
            Ok((out, json!({"weapons": weapons})))
        }
        "players" => {
            // the model: an .mdl that isn't a texture file (…T.mdl beside one with the same name)
            let stems: Vec<String> = files.iter().map(|f| name_of(f).to_lowercase()).filter_map(|n| n.strip_suffix(".mdl").map(String::from)).collect();
            let main = files.iter().find(|f| {
                let n = name_of(f).to_lowercase();
                n.strip_suffix(".mdl").is_some_and(|s| !(s.ends_with('t') && stems.iter().any(|o| *o == s[..s.len() - 1])))
            });
            let Some(main) = main else { return Err("That download has no player model (.mdl) in it.".into()) };
            let stem = name_of(main).to_lowercase().trim_end_matches(".mdl").to_string();
            out.push((main.clone(), format!("models/player/{tag}/{tag}.mdl")));
            if let Some(t) = files.iter().find(|f| name_of(f).to_lowercase() == format!("{stem}t.mdl")) {
                out.push((t.clone(), format!("models/player/{tag}/{tag}T.mdl")));
            }
            Ok((out, json!({"player": tag})))
        }
        _ => Err("Only weapon skins and player models can be used on a server.".into()),
    }
}

fn list_path(game_dir: &Path) -> PathBuf {
    game_dir.join(DATA).join("content.json")
}

/// The server's items: [{mod, section, name, author, url, date, who, on, files, weapons | player}]
pub fn load(game_dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(list_path(game_dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// content.ini for gm_content.amxx: one line per weapon model set or player model.
///     weapon <weapon_x> <who> <v model or -> <p model or -> <w model or ->
///     player <who> <model name>
pub fn ini_text(items: &[Value]) -> String {
    let mut lines = vec!["; Written by Loner-Strike: this server's skins and player models (Get more). Change them there.".to_string()];
    for it in items.iter().filter(|i| i["on"].as_bool().unwrap_or(true)) {
        let who = it["who"].as_str().filter(|w| WHO.contains(w)).unwrap_or("all");
        if let Some(weapons) = it["weapons"].as_object() {
            for (weapon, m) in weapons {
                let part = |k: &str| m[k].as_str().unwrap_or("-").to_string();
                lines.push(format!("weapon {weapon} {who} {} {} {}", part("v"), part("p"), part("w")));
            }
        }
        if let Some(p) = it["player"].as_str() {
            lines.push(format!("player {who} {p}"));
        }
    }
    lines.push(String::new());
    lines.join("\r\n")
}

/// Saves the list and the plugin's file, and tells the sync a server's copy changed.
pub fn save(game_dir: &Path, items: &[Value]) -> Result<(), String> {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = game_dir.join(DATA);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&list_path(game_dir), serde_json::to_string_pretty(items).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't save the list ({e})."))?;
    crate::sys::write_atomic(&dir.join("content.ini"), ini_text(items).as_bytes()).map_err(|e| format!("Couldn't save content.ini ({e})."))?;
    crate::servers::mark_changed_dir(game_dir);
    Ok(())
}

/// Copies an item's files in and adds it to the list (switched on, for everyone).
pub fn add(game_dir: &Path, section: &str, mod_id: i64, info: &Value, files: &[PathBuf]) -> Result<Vec<String>, String> {
    let (copies, what) = plan(section, mod_id, files)?;
    let mut items = load(game_dir);
    if items.iter().any(|i| i["mod"].as_i64() == Some(mod_id) && i["section"] == section) {
        return Err("That's already on this server. Remove it first to add another version.".into());
    }
    let mut placed = Vec::new();
    for (src, rel) in &copies {
        let dest = game_dir.join(rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::copy(src, &dest).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
        placed.push(rel.clone());
    }
    let mut item = json!({
        "mod": mod_id, "section": section, "name": info["name"], "author": info["author"], "url": info["url"],
        "date": crate::sys::now() as i64, "who": "all", "on": true, "files": placed,
    });
    for (k, v) in what.as_object().into_iter().flatten() {
        item[k] = v.clone();
    }
    items.push(item);
    save(game_dir, &items)?;
    Ok(placed)
}

/// Takes an item off the server: its files and its line.
pub fn remove(game_dir: &Path, section: &str, mod_id: i64) -> Result<Vec<String>, String> {
    let mut items = load(game_dir);
    let at = items.iter().position(|i| i["mod"].as_i64() == Some(mod_id) && i["section"] == section).ok_or("That isn't on this server.")?;
    let item = items.remove(at);
    let files: Vec<String> = item["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
    for rel in &files {
        // only what this list put there: under models/ls/ or models/player/ls…
        if (rel.starts_with("models/ls/") || rel.starts_with("models/player/ls")) && !rel.contains("..") {
            let _ = std::fs::remove_file(game_dir.join(rel));
            if let Some(d) = game_dir.join(rel).parent() {
                let _ = std::fs::remove_dir(d);
            }
        }
    }
    save(game_dir, &items)?;
    Ok(files)
}

/// What the last map loaded, as gm_content.amxx counted it: {map, models, sounds, generic}.
fn slots(g: &Game) -> Value {
    let rel = format!("{DATA}/content_status.json");
    let bytes = match &g.remote {
        Some(_) => crate::remote::game_file(g, &rel).ok().flatten(),
        None => std::fs::read(g.dir.join(&rel)).ok(),
    };
    bytes.and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null)
}

/// How many precache slots an item takes (each model file one; a player model's texture
/// file isn't loaded on its own).
pub fn slots_needed(item: &Value) -> u32 {
    let weapons = item["weapons"].as_object().map(|w| w.values().map(|m| m.as_object().map(|o| o.len()).unwrap_or(0)).sum::<usize>()).unwrap_or(0);
    (weapons + usize::from(item["player"].is_string())) as u32
}

async fn api_list(State(st): State<Shared>) -> Json<Value> {
    let Some(g) = st.game() else { return Json(json!({"items": [], "slots": null})) };
    let v = tokio::task::spawn_blocking(move || {
        let items = load(&g.dir);
        let ours: u32 = items.iter().filter(|i| i["on"].as_bool().unwrap_or(true)).map(slots_needed).sum();
        json!({"items": items, "slots": slots(&g), "limit": PRECACHE_LIMIT, "ours": ours, "server": g.dedicated})
    })
    .await
    .unwrap_or_else(|_| json!({"items": []}));
    Json(v)
}

#[derive(Deserialize)]
struct SetReq {
    #[serde(rename = "mod")]
    mod_id: i64,
    section: String,
    #[serde(default)]
    who: Option<String>,
    #[serde(default)]
    on: Option<bool>,
}

/// Who sees an item, and whether it's on.
async fn api_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<SetReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(g) = st.game() else { return (StatusCode::OK, Json(json!({"error": "Couldn't find the server."}))) };
    if req.who.as_deref().is_some_and(|w| !WHO.contains(&w)) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown choice."})));
    }
    let r = tokio::task::spawn_blocking(move || {
        let mut items = load(&g.dir);
        let it = items.iter_mut().find(|i| i["mod"].as_i64() == Some(req.mod_id) && i["section"] == req.section.as_str()).ok_or("That isn't on this server.")?;
        if let Some(w) = req.who {
            it["who"] = json!(w);
        }
        if let Some(on) = req.on {
            it["on"] = json!(on);
        }
        save(&g.dir, &items)?;
        Ok::<_, String>(json!({"ok": true}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into()));
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/servercontent", get(api_list)).route("/api/servercontent/set", post(api_set))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapons_by_file_name() {
        assert_eq!(weapon_of("v_ak47.mdl"), Some(("weapon_ak47", 'v')));
        assert_eq!(weapon_of("P_MP5.MDL"), Some(("weapon_mp5navy", 'p')));
        assert_eq!(weapon_of("w_backpack.mdl"), Some(("weapon_c4", 'w')));
        for no in ["v_shield_ak47.mdl", "v_ak47.wav", "ak47.mdl", "x_ak47.mdl", "v_ak48.mdl"] {
            assert!(weapon_of(no).is_none(), "{no}");
        }
    }

    #[test]
    fn skins_and_player_models_get_their_own_names() {
        let f = |p: &str| PathBuf::from(p);
        let (copies, what) = plan("skins", 123, &[f("/x/models/v_ak47.mdl"), f("/x/models/p_ak47.mdl"), f("/x/readme.txt"), f("/x/sound/weapons/ak47-1.wav")]).unwrap();
        assert_eq!(copies.iter().map(|c| c.1.as_str()).collect::<Vec<_>>(), ["models/ls/ls123/v_ak47.mdl", "models/ls/ls123/p_ak47.mdl"]);
        assert_eq!(what["weapons"]["weapon_ak47"]["v"], "models/ls/ls123/v_ak47.mdl");
        assert!(plan("skins", 1, &[f("/x/readme.txt")]).is_err());
        let (copies, what) = plan("players", 77, &[f("/x/models/player/leet2/leet2.mdl"), f("/x/models/player/leet2/leet2T.mdl")]).unwrap();
        assert_eq!(copies.iter().map(|c| c.1.as_str()).collect::<Vec<_>>(), ["models/player/ls77/ls77.mdl", "models/player/ls77/ls77T.mdl"]);
        assert_eq!(what["player"], "ls77");
        assert!(plan("sounds", 1, &[]).is_err());
    }

    #[test]
    fn the_plugins_file() {
        let items = vec![
            json!({"mod": 1, "section": "skins", "who": "ct", "on": true, "weapons": {"weapon_ak47": {"v": "models/ls/ls1/v_ak47.mdl"}}}),
            json!({"mod": 2, "section": "players", "who": "admin", "on": true, "player": "ls2"}),
            json!({"mod": 3, "section": "players", "who": "all", "on": false, "player": "ls3"}),
            json!({"mod": 4, "section": "players", "who": "evil;quit", "on": true, "player": "ls4"}),
        ];
        let t = ini_text(&items);
        assert!(t.contains("\r\nweapon weapon_ak47 ct models/ls/ls1/v_ak47.mdl - -\r\n"));
        assert!(t.contains("\r\nplayer admin ls2\r\n"));
        assert!(!t.contains("ls3"), "off: left out");
        assert!(t.contains("\r\nplayer all ls4\r\n"), "an unknown choice falls back to everyone");
        assert_eq!(slots_needed(&items[0]), 1);
        assert_eq!(slots_needed(&items[1]), 1);
    }

    #[test]
    fn add_and_remove() {
        let dir = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "content-").unwrap();
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("v_awp.mdl"), b"IDST").unwrap();
        let game = dir.join("cstrike");
        let placed = add(&game, "skins", 9, &json!({"name": "Gold AWP"}), &[src.join("v_awp.mdl")]).unwrap();
        assert_eq!(placed, ["models/ls/ls9/v_awp.mdl"]);
        assert!(game.join("models/ls/ls9/v_awp.mdl").is_file());
        assert!(std::fs::read_to_string(game.join(DATA).join("content.ini")).unwrap().contains("weapon weapon_awp all models/ls/ls9/v_awp.mdl - -"));
        assert!(add(&game, "skins", 9, &json!({}), &[src.join("v_awp.mdl")]).is_err(), "once");
        remove(&game, "skins", 9).unwrap();
        assert!(!game.join("models/ls/ls9/v_awp.mdl").exists());
        assert!(load(&game).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
