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
use std::path::Path;

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

fn manifest_rel(mode: &str) -> String {
    format!("addons/amxmodx/configs/gamemodes/{mode}.settings.json")
}

fn values_rel(mode: &str) -> String {
    format!("addons/amxmodx/data/studio_settings/{mode}.cfg")
}

/// Where a game's files are read and written: this computer's folder, or a server's on its
/// machine (over the same link the app uses for every other server setting).
pub trait Files {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>, String>;
    fn write(&self, rel: &str, data: &[u8]) -> Result<(), String>;
}

impl Files for crate::game::Game {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>, String> {
        crate::remote::game_file(self, rel)
    }
    fn write(&self, rel: &str, data: &[u8]) -> Result<(), String> {
        crate::remote::put_game_file(self, rel, data)
    }
}

/// A plain folder (tests).
impl Files for Path {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(self.join(rel)) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn write(&self, rel: &str, data: &[u8]) -> Result<(), String> {
        let p = self.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        crate::sys::write_atomic(&p, data).map_err(|e| e.to_string())
    }
}

/// The manifest a mode has in a game (none: the mode has no settings).
pub fn read_manifest(f: &(impl Files + ?Sized), mode: &str) -> Result<Option<Value>, String> {
    let Some(b) = f.read(&manifest_rel(mode))? else { return Ok(None) };
    if b.len() > 256 * 1024 {
        return Ok(None);
    }
    let v: Value = serde_json::from_slice(&b).unwrap_or(Value::Null);
    Ok(v["settings"].is_array().then_some(v))
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

/// A host's value for a setting: the right type (a number; 0 or 1 for yes/no; a whole number
/// for a choice), else refused. Outside the author's range is allowed (hosts are trusted),
/// with a warning.
pub fn check_value(def: &Value, v: &Value) -> Result<(f64, Option<String>), String> {
    let label = def["label"].as_str().unwrap_or("That setting");
    let ty = def["type"].as_str().unwrap_or("number");
    let x = match v {
        Value::Bool(b) => if *b { 1.0 } else { 0.0 },
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        _ => return Err(format!("{label} takes a number.")),
    };
    if !x.is_finite() || x.abs() > 1e9 {
        return Err(format!("{label} takes a number."));
    }
    if ty == "bool" && x != 0.0 && x != 1.0 {
        return Err(format!("{label} is yes or no (1 or 0)."));
    }
    if ty == "choice" && x.fract() != 0.0 {
        return Err(format!("{label} takes one of its choices (a whole number)."));
    }
    let (lo, hi) = (def["min"].as_f64().unwrap_or(0.0), def["max"].as_f64().unwrap_or(0.0));
    let warn = (x < lo || x > hi).then(|| format!("{label}: {x} is outside what the mode's author suggests ({lo}-{hi}); the mode may misbehave."));
    Ok((x, warn))
}

/// The settings of a mode in a game, each with its value there, its default and whether it
/// differs from it.
pub fn get(f: &(impl Files + ?Sized), mode: &str) -> Result<Value, String> {
    let Some(m) = read_manifest(f, mode)? else { return Ok(json!({"mode": mode, "settings": []})) };
    let have = read_values(&crate::rcon::latin1(&f.read(&values_rel(mode))?.unwrap_or_default()));
    let list: Vec<Value> = m["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let mut d = d.clone();
            let cvar = d["cvar"].as_str().unwrap_or_default().to_string();
            let v = have.get(&cvar).cloned().filter(|v| check_value(&d, v).is_ok()).unwrap_or_else(|| d["default"].clone());
            d["changed"] = json!(v.as_f64() != d["default"].as_f64());
            if let Ok((_, Some(w))) = check_value(&d, &v) {
                d["warning"] = json!(w);
            }
            d["value"] = v;
            d
        })
        .collect();
    Ok(json!({"mode": mode, "name": m["name"], "menu": m["menu"], "settings": list}))
}

/// Writes a mode's values file: the given values (the rest at their defaults are left out, so
/// the mode's defaults apply to them).
fn write_values(f: &(impl Files + ?Sized), mode: &str, defs: &[Value], have: &Map<String, Value>) -> Result<(), String> {
    let mut text = String::from("// this server's values for the mode's settings (the app and the admin menu write it; read, never run)\n");
    for d in defs {
        let cvar = d["cvar"].as_str().unwrap_or_default();
        if let Some(v) = have.get(cvar).and_then(Value::as_f64) {
            text.push_str(&format!("{cvar} \"{v}\"\n"));
        }
    }
    f.write(&values_rel(mode), text.as_bytes()).map_err(|e| format!("Couldn't save the settings ({e})."))
}

/// Changes some of a mode's settings (the rest kept); every value's type checked first.
pub fn set(f: &(impl Files + ?Sized), mode: &str, values: &Map<String, Value>) -> Result<Value, String> {
    let m = read_manifest(f, mode)?.ok_or("That mode has no settings here (compile or install it first).")?;
    let defs = m["settings"].as_array().cloned().unwrap_or_default();
    let mut have = read_values(&crate::rcon::latin1(&f.read(&values_rel(mode))?.unwrap_or_default()));
    let mut warnings = Vec::new();
    for (k, v) in values {
        let def = defs.iter().find(|d| d["cvar"].as_str() == Some(k.as_str())).ok_or_else(|| format!("This mode has no setting called {}.", k.chars().take(40).collect::<String>()))?;
        let (x, w) = check_value(def, v)?;
        warnings.extend(w);
        have.insert(k.clone(), json!(x));
    }
    write_values(f, mode, &defs, &have)?;
    let mut out = get(f, mode)?;
    out["warnings"] = json!(warnings);
    Ok(out)
}

/// Back to the mode's defaults: these settings, or all of them (empty list).
pub fn reset(f: &(impl Files + ?Sized), mode: &str, which: &[String]) -> Result<Value, String> {
    let m = read_manifest(f, mode)?.ok_or("That mode has no settings here (compile or install it first).")?;
    let defs = m["settings"].as_array().cloned().unwrap_or_default();
    let mut have = read_values(&crate::rcon::latin1(&f.read(&values_rel(mode))?.unwrap_or_default()));
    if which.is_empty() {
        have.clear();
    } else {
        for k in which {
            if !defs.iter().any(|d| d["cvar"].as_str() == Some(k.as_str())) {
                return Err(format!("This mode has no setting called {}.", k.chars().take(40).collect::<String>()));
            }
            have.remove(k);
        }
    }
    write_values(f, mode, &defs, &have)?;
    get(f, mode)
}

#[derive(Deserialize)]
pub struct Where {
    pub mode: String,
}

/// The game the request is about (the player's own, or the server it names), for this.
fn target(st: &Shared) -> Result<std::sync::Arc<crate::game::Game>, String> {
    st.game().ok_or_else(|| "Set up the game on this computer first.".to_string())
}

pub async fn api_get(State(st): State<Shared>, Query(q): Query<Where>) -> Response {
    let g = target(&st);
    super::blocking(move || {
        if !graph::valid_mode_id(&q.mode) {
            return Err("That isn't a mode id.".into());
        }
        get(g?.as_ref(), &q.mode)
    })
    .await
}

#[derive(Deserialize)]
pub struct SetReq {
    pub mode: String,
    #[serde(default)]
    pub values: Map<String, Value>,
    /// back to the defaults: these cvars, or all with `reset_all`
    #[serde(default)]
    pub reset: Vec<String>,
    #[serde(default)]
    pub reset_all: bool,
}

pub async fn api_set(State(st): State<Shared>, h: HeaderMap, Json(req): Json<SetReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let g = target(&st);
    super::blocking(move || {
        let g = g?;
        if !graph::valid_mode_id(&req.mode) {
            return Err("That isn't a mode id.".into());
        }
        if req.values.len() > 64 || req.reset.len() > 64 {
            return Err("Too many settings at once.".into());
        }
        if g.remote.is_none() {
            super::guard_real_game(&g.dir)?;
        }
        if req.reset_all || !req.reset.is_empty() {
            return reset(g.as_ref(), &req.mode, if req.reset_all { &[] } else { &req.reset });
        }
        set(g.as_ref(), &req.mode, &req.values)
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
        let f: &Path = &d;
        // the defaults until a host changes one
        assert_eq!(get(f, "koth").unwrap()["settings"][0]["value"], 30.0);
        assert_eq!(get(f, "koth").unwrap()["settings"][0]["changed"], false);
        let ok: Map<String, Value> = serde_json::from_value(json!({"lsm_koth_hold": 60, "lsm_koth_ff": true, "lsm_koth_size": 2})).unwrap();
        let v = set(f, "koth", &ok).unwrap();
        assert_eq!(v["settings"][0]["value"], 60.0);
        assert_eq!(v["settings"][0]["changed"], true);
        assert_eq!(v["settings"][1]["value"], 1.0);
        assert!(v["warnings"].as_array().unwrap().is_empty());
        // hosts are trusted: outside the author's range is kept, with a warning
        let v = set(f, "koth", &serde_json::from_value(json!({"lsm_koth_hold": 500})).unwrap()).unwrap();
        assert_eq!(v["settings"][0]["value"], 500.0);
        assert!(v["warnings"][0].as_str().unwrap().contains("outside what the mode's author suggests (5-120)"), "{v}");
        assert!(v["settings"][0]["warning"].is_string());
        // the wrong type, half a choice, yes/no other than 0/1, an unknown name, text: refused
        for bad in [json!({"lsm_koth_size": 1.5}), json!({"lsm_koth_ff": 2}), json!({"sv_cheats": 1}), json!({"lsm_koth_hold": "1;quit"}), json!({"lsm_koth_hold": "60"})] {
            assert!(set(f, "koth", &serde_json::from_value(bad.clone()).unwrap()).is_err(), "{bad}");
        }
        let file = std::fs::read_to_string(d.join("addons/amxmodx/data/studio_settings/koth.cfg")).unwrap();
        assert!(file.contains("lsm_koth_hold \"500\"") && !file.contains("cheats") && !file.contains("quit"), "{file}");
        // reset one, then all: back to the defaults (left out of the file)
        let v = reset(f, "koth", &["lsm_koth_hold".to_string()]).unwrap();
        assert_eq!(v["settings"][0]["value"], 30.0);
        assert_eq!(v["settings"][1]["value"], 1.0);
        let v = reset(f, "koth", &[]).unwrap();
        assert!(v["settings"].as_array().unwrap().iter().all(|s| s["changed"] == false));
        let file = std::fs::read_to_string(d.join("addons/amxmodx/data/studio_settings/koth.cfg")).unwrap();
        assert!(!file.contains("lsm_koth"), "{file}");
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
