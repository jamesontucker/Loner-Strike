//! Which game modes players can vote for (gm_pubserver's mode vote: rock the vote with
//! mode voting on, and the vote at map end), per game or server.
//!
//! The list is <data>/votemodes.ini, one mode id per line, written here. Without it the
//! plugin reads configs/gamemodes/votemodes.ini (shipped, rewritten on updates), and
//! without that its built-in list, which is DEFAULT below. Modes that aren't installed,
//! and modes with no map that suits them, are left out of the vote by the plugin.
//! A server on another machine gets the file with its other settings (the data folder
//! goes up after every change made here).

use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// The plugin's built-in list, in order (zp5 shows in the vote as "Zombie Plague")
pub const DEFAULT: &[&str] = &[
    "classic", "csdm", "csdm_ffa", "gungame", "gungame_team", "deathrun", "basebuilder", "soccerjam", "zp5", "scoutz", "war3", "hns",
    "paintball", "shero",
];
const FILE: &str = "votemodes.ini";
const SHIPPED: &str = "addons/amxmodx/configs/gamemodes/votemodes.ini";

/// Mode ids in a list: the first word of each line; ; # and // start comments. Lines
/// like "preset:<id>" are kept free for later (presets in the vote), not modes.
pub fn parse(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some(id) = line.split_whitespace().next() else { continue };
        if id.contains(':') || !valid_id(id) {
            continue;
        }
        let id = id.to_lowercase();
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 31 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// The list the plugin uses: the saved one, else the shipped file, else the built-in one
/// (whichever names something first, as the plugin reads them). (ids, saved here)
pub fn current(data: &Path, game_dir: &Path) -> (Vec<String>, bool) {
    let saved = parse(&crate::sys::read_latin1(&data.join(FILE)).unwrap_or_default());
    if !saved.is_empty() {
        return (saved, true);
    }
    let shipped = parse(&crate::sys::read_latin1(&game_dir.join(SHIPPED)).unwrap_or_default());
    if !shipped.is_empty() {
        return (shipped, false);
    }
    (DEFAULT.iter().map(|s| s.to_string()).collect(), false)
}

/// The file's text for a list of ids.
pub fn render(ids: &[String]) -> String {
    let mut text = String::from("; Game modes players can vote for. Saved by Loner-Strike (Rules page); one mode id per line.\r\n");
    for id in ids {
        text.push_str(id);
        text.push_str("\r\n");
    }
    text
}

/// Checks a new list against the installed modes (keeps their order, drops repeats).
pub fn check(ids: &[String], installed: &[(String, String)]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let id = id.trim().to_lowercase();
        if !valid_id(&id) || !installed.iter().any(|(m, _)| *m == id) {
            return Err("That game mode isn't installed.".into());
        }
        if !out.contains(&id) {
            out.push(id);
        }
    }
    if out.len() < 2 && installed.len() >= 2 {
        return Err("Pick at least two modes for the vote.".into());
    }
    Ok(out)
}

fn installed(game_dir: &Path) -> Vec<(String, String)> {
    crate::extras::parse_modes(&crate::sys::read_latin1(&game_dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default())
}

fn listing(data: &Path, game_dir: &Path) -> Value {
    let modes = installed(game_dir);
    let (ids, saved) = current(data, game_dir);
    let chosen: Vec<&String> = ids.iter().filter(|id| modes.iter().any(|(m, _)| m == *id)).collect();
    json!({
        "modes": modes.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "chosen": chosen,
        "custom": saved,
    })
}

#[derive(Deserialize)]
struct Save {
    #[serde(default)]
    modes: Vec<String>,
    /// back to the shipped list
    #[serde(default)]
    reset: bool,
}

type Reply = (StatusCode, Json<Value>);

fn bad(msg: impl Into<String>) -> Reply {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg.into()})))
}

async fn api_get(State(st): State<Shared>) -> Reply {
    let Some(g) = st.game() else { return bad("No game found.") };
    let v = tokio::task::spawn_blocking(move || listing(&g.data, &g.dir)).await.unwrap_or_else(|_| json!({"error": "Couldn't read the list."}));
    (StatusCode::OK, Json(v))
}

async fn api_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<Save>) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(g) = st.game() else { return bad("No game found.") };
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let path = g.data.join(FILE);
        if req.reset {
            match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("Couldn't reset the list ({e}).")),
                _ => {}
            }
        } else {
            let ids = check(&req.modes, &installed(&g.dir))?;
            std::fs::create_dir_all(&g.data).map_err(|e| e.to_string())?;
            crate::sys::write_latin1(&path, &render(&ids)).map_err(|e| format!("Couldn't save the list ({e})."))?;
        }
        Ok(listing(&g.data, &g.dir))
    })
    .await
    .unwrap_or_else(|_| Err("Couldn't save the list.".into()));
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => bad(e),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/modevote", get(api_get).post(api_set))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ls-modevote-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lists_skip_comments_repeats_and_preset_lines() {
        let ids = parse("; comment\r\n# also\r\n// and this\r\nclassic\r\n  csdm  extra words\r\nCSDM\r\npreset:fun\r\nbad/id\r\n\r\nzp5\r\n");
        assert_eq!(ids, vec!["classic", "csdm", "zp5"]);
    }

    #[test]
    fn saved_list_wins_then_shipped_then_built_in() {
        let game = tmp("game");
        let data = game.join("addons/amxmodx/data/gamemodes");
        std::fs::create_dir_all(&data).unwrap();
        assert_eq!(current(&data, &game), (DEFAULT.iter().map(|s| s.to_string()).collect(), false));
        let shipped = game.join(SHIPPED);
        std::fs::create_dir_all(shipped.parent().unwrap()).unwrap();
        std::fs::write(&shipped, "; only comments\n").unwrap();
        assert_eq!(current(&data, &game).0.len(), DEFAULT.len());
        std::fs::write(&shipped, "classic\nknife\n").unwrap();
        assert_eq!(current(&data, &game), (vec!["classic".to_string(), "knife".to_string()], false));
        std::fs::write(data.join(FILE), render(&["gungame".to_string(), "csdm".to_string()])).unwrap();
        assert_eq!(current(&data, &game), (vec!["gungame".to_string(), "csdm".to_string()], true));
        let _ = std::fs::remove_dir_all(&game);
    }

    #[test]
    fn new_lists_need_installed_modes() {
        let installed: Vec<(String, String)> = [("classic", "Classic"), ("csdm", "Deathmatch"), ("zp5", "Zombie Plague Advanced")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let ok = check(&["csdm".into(), "Classic".into(), "csdm".into()], &installed).unwrap();
        assert_eq!(ok, vec!["csdm", "classic"]);
        assert!(check(&["classic".into(), "kreedz".into()], &installed).is_err());
        assert!(check(&["classic".into()], &installed).is_err());
        assert!(check(&["../x".into(), "classic".into()], &installed).is_err());
    }

    #[test]
    fn default_matches_the_shipped_file() {
        let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/configs/gamemodes/votemodes.ini");
        let text = std::fs::read_to_string(shipped).unwrap();
        assert_eq!(parse(&text), DEFAULT.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }
}
