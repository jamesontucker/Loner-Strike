//! Mode settings (Blueprint's Instance Editable): variables a mode marks as settings hosts can
//! change without touching the graph. Compiling writes what they are next to the mode
//! (`configs/gamemodes/<mode>.settings.json`); each server keeps its own values in
//! `data/studio_settings/<mode>.cfg`, which the mode's plugin reads (never runs) as a map starts
//! and at each new round, and its admin menu writes. The Play page, the server page and MCP read
//! and change them here, checked against the manifest (names, types, ranges, choices).

use super::graph::{self, Graph};
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// What a graph's mode settings are (for the manifest).
pub fn manifest(g: &Graph) -> Value {
    let list: Vec<Value> = graph::mode_settings(g)
        .iter()
        .map(|x| {
            let st = x.setting.as_ref().unwrap();
            let (lo, hi) = st.range(&x.ty);
            let d = match &x.default {
                Value::Bool(b) => if *b { 1.0 } else { 0.0 },
                Value::Number(n) => n.as_f64().unwrap_or(0.0),
                _ => 0.0,
            };
            json!({
                "cvar": graph::setting_cvar(&g.mode.id, &x.id),
                "var": x.id,
                "label": st.label,
                "help": st.help,
                "group": st.group,
                "type": if x.ty == "bool" { "bool" } else if st.choices.is_empty() { "number" } else { "choice" },
                "min": lo,
                "max": hi,
                "step": if st.step > 0.0 { st.step } else { 0.0 },
                "choices": st.choices,
                "default": d,
            })
        })
        .collect();
    json!({"mode": g.mode.id, "name": g.mode.name, "menu": graph::settings_menu_cmd(&g.mode.id), "settings": list})
}

fn manifest_path(game_dir: &Path, mode: &str) -> PathBuf {
    game_dir.join("addons/amxmodx/configs/gamemodes").join(format!("{mode}.settings.json"))
}

fn values_path(game_dir: &Path, mode: &str) -> PathBuf {
    game_dir.join("addons/amxmodx/data/studio_settings").join(format!("{mode}.cfg"))
}

/// The manifest a mode has in a game (none: the mode has no settings).
pub fn read_manifest(game_dir: &Path, mode: &str) -> Option<Value> {
    let t = std::fs::read_to_string(manifest_path(game_dir, mode)).ok()?;
    if t.len() > 256 * 1024 {
        return None;
    }
    let v: Value = serde_json::from_str(&t).ok()?;
    v["settings"].is_array().then_some(v)
}

/// `name "value"` lines: the values a server keeps (unknown names and non-numbers left out).
pub fn read_values(text: &str) -> Map<String, Value> {
    let mut out = Map::new();
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with("//") || l.starts_with(';') {
            continue;
        }
        let mut parts = l.splitn(2, char::is_whitespace);
        let (Some(k), Some(v)) = (parts.next(), parts.next()) else { continue };
        if let Ok(x) = v.trim().trim_matches('"').parse::<f64>() {
            if x.is_finite() {
                out.insert(k.to_ascii_lowercase(), json!(x));
            }
        }
    }
    out
}

/// A value checked against its setting: a number in range (whole for yes/no and choices).
pub fn check_value(def: &Value, v: &Value) -> Result<f64, String> {
    let label = def["label"].as_str().unwrap_or("That setting");
    let x = match v {
        Value::Bool(b) => if *b { 1.0 } else { 0.0 },
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    let (lo, hi) = (def["min"].as_f64().unwrap_or(0.0), def["max"].as_f64().unwrap_or(0.0));
    let whole = matches!(def["type"].as_str(), Some("bool") | Some("choice"));
    if !x.is_finite() || x < lo || x > hi || (whole && x.fract() != 0.0) {
        return Err(format!("{label} takes {}", if whole { format!("a whole number from {lo} to {hi}") } else { format!("a number from {lo} to {hi}") }));
    }
    Ok(x)
}

/// The settings of a mode in a game, each with its value on this server.
pub fn get(game_dir: &Path, mode: &str) -> Value {
    let Some(m) = read_manifest(game_dir, mode) else { return json!({"mode": mode, "settings": []}) };
    let have = read_values(&std::fs::read_to_string(values_path(game_dir, mode)).unwrap_or_default());
    let list: Vec<Value> = m["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let mut d = d.clone();
            let cvar = d["cvar"].as_str().unwrap_or_default().to_string();
            let v = have.get(&cvar).cloned().filter(|v| check_value(&d, v).is_ok()).unwrap_or_else(|| d["default"].clone());
            d["value"] = v;
            d
        })
        .collect();
    json!({"mode": mode, "name": m["name"], "menu": m["menu"], "settings": list})
}

/// Changes some of a mode's settings on a server (the rest kept); every value checked first.
pub fn set(game_dir: &Path, mode: &str, values: &Map<String, Value>) -> Result<Value, String> {
    let m = read_manifest(game_dir, mode).ok_or("That mode has no settings here (compile or install it first).")?;
    let defs = m["settings"].as_array().cloned().unwrap_or_default();
    let path = values_path(game_dir, mode);
    let mut have = read_values(&std::fs::read_to_string(&path).unwrap_or_default());
    for (k, v) in values {
        let def = defs.iter().find(|d| d["cvar"].as_str() == Some(k.as_str())).ok_or_else(|| format!("This mode has no setting called {}.", k.chars().take(40).collect::<String>()))?;
        have.insert(k.clone(), json!(check_value(def, v)?));
    }
    let mut text = String::from("// this server's values for the mode's settings (the app and the admin menu write it)\n");
    for d in &defs {
        let cvar = d["cvar"].as_str().unwrap_or_default();
        let v = have.get(cvar).and_then(Value::as_f64).unwrap_or_else(|| d["default"].as_f64().unwrap_or(0.0));
        text.push_str(&format!("{cvar} \"{v}\"\n"));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the settings ({e})."))?;
    }
    crate::sys::write_atomic(&path, text.as_bytes()).map_err(|e| format!("Couldn't save the settings ({e})."))?;
    Ok(get(game_dir, mode))
}

#[derive(Deserialize)]
pub struct Where {
    pub mode: String,
    /// one of this computer's servers (none: the game)
    #[serde(default)]
    pub server: Option<String>,
}

/// The game folder (cstrike/czero) of the game, or of one of this computer's servers.
fn game_dir(st: &Shared, server: Option<&str>) -> Result<PathBuf, String> {
    match server.filter(|s| !s.is_empty()) {
        Some(id) => {
            let e = crate::servers::load().into_iter().find(|e| e.id == id).ok_or("That server isn't on this computer.")?;
            if e.remote.is_some() {
                return Err("That server isn't on this computer: change its mode settings from its own app.".into());
            }
            Ok(e.dir.join(&e.game))
        }
        None => Ok(super::local_game(st)?.dir.clone()),
    }
}

pub async fn api_get(State(st): State<Shared>, Query(q): Query<Where>) -> Response {
    super::blocking(move || {
        if !graph::valid_mode_id(&q.mode) {
            return Err("That isn't a mode id.".into());
        }
        Ok(get(&game_dir(&st, q.server.as_deref())?, &q.mode))
    })
    .await
}

#[derive(Deserialize)]
pub struct SetReq {
    pub mode: String,
    #[serde(default)]
    pub server: Option<String>,
    pub values: Map<String, Value>,
}

pub async fn api_set(State(st): State<Shared>, h: HeaderMap, Json(req): Json<SetReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    super::blocking(move || {
        if !graph::valid_mode_id(&req.mode) {
            return Err("That isn't a mode id.".into());
        }
        if req.values.len() > 64 {
            return Err("Too many settings at once.".into());
        }
        let dir = game_dir(&st, req.server.as_deref())?;
        super::guard_real_game(&dir)?;
        set(&dir, &req.mode, &req.values)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked_against_the_manifest() {
        let g = graph::parse(&json!({
            "schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "koth", "name": "King"}, "comments": [], "nodes": [], "edges": [],
            "variables": [
                {"id": "hold", "name": "Hold time", "type": "number", "scope": "mode", "default": 30, "setting": {"label": "Seconds to win", "min": 5, "max": 120, "group": "Rules"}},
                {"id": "ff", "name": "Friendly", "type": "bool", "scope": "mode", "default": false, "setting": {"label": "Friendly fire"}},
                {"id": "size", "name": "Size", "type": "number", "scope": "mode", "default": 1, "setting": {"label": "Hill", "choices": ["Small", "Medium", "Large"]}},
                {"id": "secret", "name": "Not one", "type": "number", "scope": "player", "default": 0}
            ]
        }).to_string()).unwrap();
        assert!(!graph::has_errors(&graph::validate(&g)), "{:?}", graph::validate(&g));
        let m = manifest(&g);
        let s = m["settings"].as_array().unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s[0]["cvar"], "lsm_koth_hold");
        assert_eq!(s[2]["type"], "choice");
        assert_eq!(s[2]["max"], 2.0);
        let d = std::env::temp_dir().join(format!("ls-modeset-{}", std::process::id()));
        let gm = d.join("addons/amxmodx/configs/gamemodes");
        std::fs::create_dir_all(&gm).unwrap();
        std::fs::write(gm.join("koth.settings.json"), m.to_string()).unwrap();
        // the defaults until a host changes one
        assert_eq!(get(&d, "koth")["settings"][0]["value"], 30.0);
        let ok: Map<String, Value> = serde_json::from_value(json!({"lsm_koth_hold": 60, "lsm_koth_ff": true, "lsm_koth_size": 2})).unwrap();
        let v = set(&d, "koth", &ok).unwrap();
        assert_eq!(v["settings"][0]["value"], 60.0);
        assert_eq!(v["settings"][1]["value"], 1.0);
        // out of range, half a choice, an unknown name, a command in disguise: refused, nothing written
        for bad in [json!({"lsm_koth_hold": 500}), json!({"lsm_koth_size": 1.5}), json!({"sv_cheats": 1}), json!({"lsm_koth_hold": "1;quit"})] {
            assert!(set(&d, "koth", &serde_json::from_value(bad.clone()).unwrap()).is_err(), "{bad}");
        }
        let file = std::fs::read_to_string(d.join("addons/amxmodx/data/studio_settings/koth.cfg")).unwrap();
        assert!(file.contains("lsm_koth_hold \"60\"") && !file.contains("500") && !file.contains("cheats"), "{file}");
        // a setting on a player variable, or a range that's upside down: the checks say so
        let mut g2 = g.clone();
        g2.variables[3].setting = g.variables[0].setting.clone();
        assert!(graph::has_errors(&graph::validate(&g2)));
        let mut g3 = g.clone();
        g3.variables[0].setting.as_mut().unwrap().max = 1.0;
        assert!(graph::has_errors(&graph::validate(&g3)));
        let _ = std::fs::remove_dir_all(&d);
    }
}
