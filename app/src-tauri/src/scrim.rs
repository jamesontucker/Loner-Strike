//! Match mode ("Match (5v5)", gm_scrim.amxx): scrims and pick-up games.
//!
//! - Settings (round limit, overtime, knife round, bots, players, pauses) live in
//!   <data>/scrim.json; <data>/scrim.cfg is what the plugin runs, on every map and on
//!   gm_scrimreload.
//! - Live state: gm_scrimstatus makes the plugin write <data>/scrim_status.json.
//! - Past matches: the plugin adds one JSON object per line to <data>/matches.json.
//!
//! Only fixed commands are sent to the game: the reload, the status and the
//! gm_scrim actions in ACTIONS.

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::rcon::latin1;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;

pub const MODE: &str = "match";
const RELOAD: &str = "gm_scrimreload";
const STATUS: &str = "gm_scrimstatus";
const STATUS_DONE: &str = "Scrim status written";
/// The app's buttons: each becomes "gm_scrim <action>".
const ACTIONS: &[&str] = &["start", "restart", "warmup", "pause", "unpause"];
const MAX_ROUNDS: &[u32] = &[24, 30];
const MAX_MATCHES: usize = 50;
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// 30 (MR15) or 24 (MR12)
    pub max_rounds: u32,
    /// MR3 overtime on a tie
    pub overtime: bool,
    /// a knife round picks the sides
    pub knife: bool,
    /// bots play, as the game's own bot settings say (false: people only)
    pub bots: bool,
    /// players needed (ready) to start, both teams together
    pub players: u32,
    /// tactical pauses per team per match
    pub pauses: u32,
    /// seconds before anyone can end a tactical pause
    pub pause_seconds: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_rounds: 30, overtime: true, knife: true, bots: true, players: 10, pauses: 2, pause_seconds: 60 }
    }
}

/// Settings brought into range.
pub fn tidy(mut s: Settings) -> Settings {
    if !MAX_ROUNDS.contains(&s.max_rounds) {
        s.max_rounds = 30;
    }
    s.players = s.players.clamp(2, 32);
    s.pauses = s.pauses.min(5);
    s.pause_seconds = s.pause_seconds.clamp(15, 300);
    s
}

/// scrim.cfg, run by gm_scrim.amxx.
pub fn cfg(s: &Settings) -> String {
    let b = |v: bool| if v { 1 } else { 0 };
    [
        "// Written by Loner-Strike's Match page. Change it there.".to_string(),
        format!("gm_scrim_maxrounds {}", s.max_rounds),
        format!("gm_scrim_overtime {}", b(s.overtime)),
        format!("gm_scrim_knife {}", b(s.knife)),
        format!("gm_scrim_bots {}", b(s.bots)),
        format!("gm_scrim_players {}", s.players),
        format!("gm_scrim_pauses {}", s.pauses),
        format!("gm_scrim_pausetime {}", s.pause_seconds),
        String::new(),
    ]
    .join("\r\n")
}

fn load(data: &Path) -> Settings {
    std::fs::read_to_string(data.join("scrim.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).map(tidy).unwrap_or_default()
}

fn save(data: &Path, s: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(data.join("scrim.json"), text).map_err(|e| format!("Couldn't save the settings ({e})."))?;
    std::fs::write(data.join("scrim.cfg"), cfg(s)).map_err(|e| format!("Couldn't write scrim.cfg ({e})."))
}

/// matches.json: one match per line, newest first. Lines that aren't a match are skipped.
pub fn parse_matches(text: &str) -> Vec<Value> {
    let mut out: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .filter(|v| v["map"].is_string() && v["teams"].as_array().is_some_and(|t| t.len() == 2))
        .collect();
    out.reverse();
    out.truncate(MAX_MATCHES);
    out
}

/// The game writes names as the players typed them: UTF-8 mostly, older clients Latin-1.
pub fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_else(|e| latin1(e.as_bytes()))
}

/// Past matches, from the machine for a server on another one (the plugin adds them there).
fn matches(g: &Game) -> Vec<Value> {
    let bytes = match &g.remote {
        Some(_) => crate::remote::game_file(g, &format!("{DATA}/matches.json")).ok().flatten(),
        None => std::fs::read(g.data.join("matches.json")).ok(),
    };
    parse_matches(&decode(bytes.unwrap_or_default()))
}

/// Where the plugin keeps its files, relative to the game folder.
const DATA: &str = "addons/amxmodx/data/gamemodes";

fn current_mode(g: &Game) -> String {
    crate::extras::current_mode(g)
}

/// The plugin's live state, when the game is running this mode and answers.
fn live(g: &Game, mode: &str) -> Value {
    if mode != MODE {
        return Value::Null;
    }
    // on another machine the file it writes comes back in the same SSH command
    let file = g.remote.as_ref().map(|(_, dir)| format!("{dir}/{DATA}/scrim_status.json"));
    let sent = match &file {
        Some(f) => g.rcon.send_then_cat(STATUS, f, STATUS_MARK),
        None => g.rcon.send(STATUS).map(|r| (r, None)),
    };
    match sent {
        Ok((reply, file)) if reply.contains(STATUS_DONE) => {
            let bytes = file.unwrap_or_else(|| std::fs::read(g.data.join("scrim_status.json")).unwrap_or_default());
            serde_json::from_str(decode(bytes).trim()).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

/// Splits the reply from the status file sent after it (a server on another machine).
const STATUS_MARK: &str = "\n@@LS-SCRIM-FILE@@\n";

fn mode_installed(g: &Game) -> bool {
    crate::extras::parse_modes(&crate::sys::read_latin1(&g.dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap_or_default())
        .iter()
        .any(|(id, _)| id == MODE)
}

fn listing(g: &Game) -> Value {
    let mode = current_mode(g);
    json!({
        "settings": load(&g.data),
        "status": live(g, &mode),
        "matches": matches(g),
        "current_mode": mode,
        "installed": mode_installed(g),
        "plugin": g.dir.join("addons/amxmodx/plugins/gm_scrim.amxx").exists(),
    })
}

// ---- API ------------------------------------------------------------------------------

type Reply = (StatusCode, Json<Value>);

fn bad(msg: impl Into<String>) -> Reply {
    (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": msg.into()})))
}

fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn answer(r: Option<Result<Value, String>>) -> Reply {
    match r {
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"ok": false, "error": "No game found."}))),
        Some(Ok(v)) => (StatusCode::OK, Json(v)),
        Some(Err(e)) => bad(e),
    }
}

async fn with_game<T: Send + 'static>(st: &Shared, f: impl FnOnce(&Game) -> T + Send + 'static) -> Option<T> {
    let g = st.game()?;
    tokio::task::spawn_blocking(move || f(&g)).await.ok()
}

async fn api_get(State(st): State<Shared>) -> Reply {
    answer(with_game(&st, |g| Ok(listing(g))).await)
}

async fn api_settings(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<Settings>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let s = tidy(req);
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap();
            save(&g.data, &s)?;
            let _ = g.rcon.send(RELOAD);
            Ok(listing(g))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct ActionReq {
    action: String,
}

/// The fixed command for an app button, if it is one.
pub fn action_command(action: &str) -> Option<String> {
    ACTIONS.contains(&action).then(|| format!("gm_scrim {action}"))
}

async fn api_action(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ActionReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(line) = action_command(req.action.trim()) else { return bad("Unknown action.") };
    answer(
        with_game(&st, move |g| {
            if current_mode(g) != MODE {
                return Err("Switch to Match mode first.".to_string());
            }
            g.rcon.send(&line).map_err(|_| "The game isn't running, or didn't answer.".to_string())?;
            Ok(listing(g))
        })
        .await,
    )
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/scrim", get(api_get))
        .route("/api/scrim/settings", post(api_settings))
        .route("/api/scrim/action", post(api_action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrim_settings_are_tidied() {
        let t = tidy(Settings { max_rounds: 17, players: 0, pauses: 99, pause_seconds: 1, ..Settings::default() });
        assert_eq!((t.max_rounds, t.players, t.pauses, t.pause_seconds), (30, 2, 5, 15));
        let t = tidy(Settings { max_rounds: 24, players: 64, pause_seconds: 9999, ..Settings::default() });
        assert_eq!((t.max_rounds, t.players, t.pause_seconds), (24, 32, 300));
        assert_eq!(tidy(Settings::default()), Settings::default());
        // missing fields take the defaults
        let s: Settings = serde_json::from_str(r#"{"max_rounds": 24}"#).unwrap();
        assert_eq!(s, Settings { max_rounds: 24, ..Settings::default() });
        assert!(Settings::default().bots, "bots play by default, as the game's own bot settings say");
    }

    #[test]
    fn scrim_cfg_file() {
        let s = Settings { max_rounds: 24, overtime: false, knife: true, bots: true, players: 4, pauses: 1, pause_seconds: 30 };
        let c = cfg(&s);
        for line in ["gm_scrim_maxrounds 24", "gm_scrim_overtime 0", "gm_scrim_knife 1", "gm_scrim_bots 1", "gm_scrim_players 4", "gm_scrim_pauses 1", "gm_scrim_pausetime 30"] {
            assert!(c.contains(&format!("{line}\r\n")), "{line}");
        }
        let dir = std::env::temp_dir().join(format!("ls-scrim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir), s);
        assert!(dir.join("scrim.cfg").is_file());
        std::fs::write(dir.join("scrim.json"), "not json").unwrap();
        assert_eq!(load(&dir), Settings::default(), "a damaged file gives the defaults");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scrim_actions_are_fixed() {
        assert_eq!(action_command("pause").as_deref(), Some("gm_scrim pause"));
        assert_eq!(action_command("start").as_deref(), Some("gm_scrim start"));
        for bad in ["", "quit", "start; quit", "pause\nquit", "START"] {
            assert!(action_command(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn scrim_match_history() {
        let line = |map: &str, a: u32, b: u32| {
            format!(
                r#"{{"date":"2026-11-06 21:04","ts":1,"map":"{map}","maxrounds":30,"rounds":{},"overtime":0,"winner":{},"teams":[{{"name":"Team amy","score":{a},"players":["amy","Bob \"B\""]}},{{"name":"Team 2","score":{b},"players":[]}}]}}"#,
                a + b,
                if a > b { 1 } else if b > a { 2 } else { 0 }
            )
        };
        let text = format!("{}\r\n\r\ngarbage\n{{\"map\":\"x\"}}\n{}\n", line("de_dust2", 16, 12), line("de_inferno", 15, 15));
        let m = parse_matches(&text);
        assert_eq!(m.len(), 2, "broken and partial lines are skipped");
        assert_eq!(m[0]["map"], "de_inferno", "newest first");
        assert_eq!(m[1]["teams"][0]["players"][1], "Bob \"B\"");
        assert_eq!(m[1]["winner"], 1);
        let many: String = (0..80).map(|i| line(&format!("de_{i}"), 16, 3) + "\n").collect();
        let m = parse_matches(&many);
        assert_eq!(m.len(), MAX_MATCHES);
        assert_eq!(m[0]["map"], "de_79");
        // names in UTF-8 or Latin-1 both read
        let utf = line("de_nuke", 16, 0).replace("amy", "am\u{e9}");
        assert_eq!(parse_matches(&decode(utf.clone().into_bytes()))[0]["teams"][0]["players"][0], "am\u{e9}");
        let lat: Vec<u8> = utf.chars().map(|c| c as u32 as u8).collect();
        assert_eq!(parse_matches(&decode(lat))[0]["teams"][0]["players"][0], "am\u{e9}");
    }
}
