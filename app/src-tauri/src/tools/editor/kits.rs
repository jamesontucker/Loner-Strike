//! Mode map kits in the map editor (agent P5; the format is X's, docs/notes/X.md "Mode map kit
//! format", version 1). A kit says what a map needs for one game mode: markers (`ls_marker`
//! entities with `kind` = the marker id and `mode` = the mode id), the `ls_*` components it uses
//! and the creatures it spawns. The map says which modes it's made for in the worldspawn key
//! `ls_modes` (mode ids separated by spaces; saved in the project and the .bsp, read by
//! Playtest and Publish).
//!   GET /api/tools/editor/kits   {folder, kits, bad, madeFor, needs}
//! The live checks list, for each mode the map is made for, what it still needs (warnings with
//! a `place` the page offers to place), and markers' keys of the wrong type.

use super::{blocking, Ed, NO_DOC};
use crate::editor::checks::{issue, Issue, Level};
use crate::editor::doc::{Entity, MapDocument};
use crate::server::Shared;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "loner-strike/mode-kit";
pub const VERSION: u32 = 1;
/// The worldspawn key that says which modes the map is made for.
pub const MODES_KEY: &str = "ls_modes";
const MAX_KITS: usize = 64;
const MAX_FILE: u64 = 256 * 1024;
const RESERVED: [&str; 14] = ["classname", "origin", "angles", "model", "targetname", "target", "mode", "kind", "order", "path", "size", "radius", "spawnflags", "angle"];
const ICONS: [&str; 10] = ["flag", "skull", "star", "crate", "door", "person", "arrow", "circle", "bolt", "heart"];
const SHAPES: [&str; 5] = ["diamond", "box", "sphere", "cylinder", "line"];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Required {
    #[serde(default)]
    pub min: u32,
    #[serde(default)]
    pub max: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct KitKey {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub default: Value,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub help: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Draw {
    #[serde(default)]
    pub colour: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub shape: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Marker {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// point, area or path
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub required: Required,
    #[serde(default)]
    pub keys: Vec<KitKey>,
    #[serde(default)]
    pub draw: Draw,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub classname: String,
    #[serde(default)]
    pub min: u32,
    #[serde(default)]
    pub max: Option<u32>,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModeInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Kit {
    pub schema: String,
    pub version: u32,
    pub mode: ModeInfo,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub creatures: Vec<String>,
    #[serde(default)]
    pub map: Value,
    #[serde(default)]
    pub notes: String,
}

fn name_ok(s: &str, max: usize, first_letter: bool) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') && (!first_letter || s.as_bytes()[0].is_ascii_lowercase())
}

fn plain(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.chars().any(|c| c.is_control())
}

/// The kit, checked against the format; Err says what's wrong in plain words.
pub fn parse(text: &str) -> Result<Kit, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("It isn't valid JSON ({e})."))?;
    if v["schema"] != SCHEMA {
        return Err("It isn't a mode map kit.".into());
    }
    if v["version"].as_u64().is_none_or(|n| n > VERSION as u64 || n == 0) {
        return Err(format!("It's for a newer version of the kit format ({}); update the app.", v["version"]));
    }
    let k: Kit = serde_json::from_value(v).map_err(|e| format!("It doesn't follow the kit format ({e})."))?;
    if !name_ok(&k.mode.id, 24, true) || k.mode.id.len() < 2 {
        return Err("Its mode id isn't 2–24 of a-z, 0-9 and _ starting with a letter.".into());
    }
    if !plain(&k.mode.name, 40) || k.mode.name.trim().is_empty() || !plain(&k.mode.description, 60) || !plain(&k.notes, 600) {
        return Err("Its mode name, description or notes are too long.".into());
    }
    if k.markers.len() > 32 || k.components.len() > 32 || k.creatures.len() > 64 {
        return Err("It has too many markers, components or creatures.".into());
    }
    let mut seen = Vec::new();
    for m in &k.markers {
        let what = format!("Marker “{}”", m.id);
        if !name_ok(&m.id, 24, false) || seen.contains(&m.id) {
            return Err(format!("{what}: its id isn't 1–24 of a-z, 0-9 and _, or is used twice."));
        }
        seen.push(m.id.clone());
        if !plain(&m.label, 30) || m.label.trim().is_empty() || !plain(&m.description, 200) {
            return Err(format!("{what}: its label or description is too long."));
        }
        if !["point", "area", "path"].contains(&m.kind.as_str()) {
            return Err(format!("{what}: type must be point, area or path."));
        }
        let r = &m.required;
        if r.min > 64 || r.max.is_some_and(|x| x == 0 || x > 64 || x < r.min) {
            return Err(format!("{what}: required min/max are out of range."));
        }
        if m.keys.len() > 8 {
            return Err(format!("{what}: more than 8 keys."));
        }
        for key in &m.keys {
            if !name_ok(&key.name, 24, false) || RESERVED.contains(&key.name.as_str()) {
                return Err(format!("{what}: the key “{}” isn't allowed.", key.name));
            }
            if !["number", "text", "bool", "choice"].contains(&key.kind.as_str()) || !plain(&key.help, 120) {
                return Err(format!("{what}: the key “{}” has an unknown type or a long help.", key.name));
            }
            if key.kind == "choice" && (key.choices.is_empty() || key.choices.len() > 16 || key.choices.iter().any(|(v, l)| !name_ok(v, 24, false) || !plain(l, 40))) {
                return Err(format!("{what}: the key “{}” needs 1–16 choices with plain values.", key.name));
            }
        }
        let d = &m.draw;
        if (!d.colour.is_empty() && !(d.colour.len() == 7 && d.colour.starts_with('#') && d.colour[1..].bytes().all(|b| b.is_ascii_hexdigit())))
            || (!d.icon.is_empty() && !ICONS.contains(&d.icon.as_str()))
            || (!d.shape.is_empty() && !SHAPES.contains(&d.shape.as_str()))
            || d.size.is_some_and(|s| s.iter().any(|x| !(1.0..=4096.0).contains(x)))
            || d.radius.is_some_and(|x| !(1.0..=4096.0).contains(&x))
        {
            return Err(format!("{what}: its drawing (colour, icon, shape, size) isn't valid."));
        }
    }
    for c in &k.components {
        if !c.classname.starts_with("ls_") || !name_ok(&c.classname, 32, true) || c.min > 64 || c.max.is_some_and(|x| x == 0 || x > 64 || x < c.min) || !plain(&c.note, 120) {
            return Err(format!("Component “{}”: not an ls_ classname, or min/max out of range.", c.classname));
        }
    }
    if k.creatures.iter().any(|c| !(1..=32).contains(&c.len()) || !c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')) {
        return Err("A creature name isn't plain.".into());
    }
    Ok(k)
}

pub fn kits_dir() -> PathBuf {
    crate::sys::app_dir().join("kits")
}

/// Every installed kit (the newest file wins for a mode id), and the files that couldn't be used.
pub fn load(dir: &Path) -> (Vec<Kit>, Vec<Value>) {
    let mut kits: BTreeMap<String, Kit> = BTreeMap::new();
    let mut bad = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return (Vec::new(), bad) };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".kit.json")).collect();
    files.sort();
    for p in files.into_iter().take(MAX_KITS * 2) {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let md = std::fs::symlink_metadata(&p);
        let r = match md {
            Ok(m) if m.is_file() && m.len() <= MAX_FILE => std::fs::read_to_string(&p).map_err(|_| "It couldn't be read.".to_string()).and_then(|t| parse(&t)),
            Ok(m) if m.is_file() => Err("It's too big for a kit.".into()),
            _ => Err("It isn't a plain file.".into()),
        };
        match r {
            Ok(k) if kits.len() < MAX_KITS || kits.contains_key(&k.mode.id) => {
                kits.insert(k.mode.id.clone(), k);
            }
            Ok(_) => bad.push(json!({"file": name, "error": "Too many kits."})),
            Err(e) => bad.push(json!({"file": name, "error": e})),
        }
    }
    (kits.into_values().collect(), bad)
}

/// The modes the map says it's made for (worldspawn `ls_modes`).
pub fn made_for(doc: &MapDocument) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for m in doc.world.get(MODES_KEY).unwrap_or("").split([' ', ',', ';']).map(str::trim).filter(|m| name_ok(m, 24, true)) {
        if !v.iter().any(|x| x == m) {
            v.push(m.to_string());
        }
    }
    v
}

/// A marker entity counts for `mode` when its `mode` is that mode, `any` or empty.
fn for_mode(e: &Entity, mode: &str) -> bool {
    matches!(e.get("mode").map(str::trim), None | Some("") | Some("any")) || e.get("mode").is_some_and(|m| m.trim() == mode)
}

fn count_text(n: usize, min: u32, max: Option<u32>) -> String {
    let want = match max {
        Some(x) if x == min => format!("{min}"),
        Some(x) if min == 0 => format!("at most {x}"),
        Some(x) => format!("{min}–{x}"),
        None => format!("at least {min}"),
    };
    format!("{want} (has {n})")
}

/// What one mode still needs, as live check issues.
pub fn check_kit(doc: &MapDocument, kit: &Kit) -> Vec<Issue> {
    let mode = &kit.mode.id;
    let name = &kit.mode.name;
    let mut out = Vec::new();
    let markers: Vec<(u64, &Entity)> = doc.entities.iter().filter(|(_, e)| e.classname() == "ls_marker" && for_mode(e, mode)).map(|(id, e)| (id.0, e)).collect();
    for m in &kit.markers {
        let spots: Vec<&(u64, &Entity)> = markers.iter().filter(|(_, e)| e.get("kind").map(str::trim) == Some(m.id.as_str())).collect();
        let n = if m.kind == "path" {
            let mut paths: BTreeMap<String, Vec<(u64, String)>> = BTreeMap::new();
            for (id, e) in &spots {
                paths.entry(e.get("path").unwrap_or("1").trim().to_string()).or_default().push((*id, e.get("order").unwrap_or("").trim().to_string()));
            }
            for (p, list) in &paths {
                if list.len() < 2 {
                    out.push(Issue { entity: Some(list[0].0), place: Some(format!("{mode}:{}", m.id)), ..issue(Level::Warning, "kit-path", format!("{name}: path {p} of “{}” has one spot; a path needs 2 or more.", m.label)) });
                }
                let mut orders: Vec<&String> = list.iter().map(|x| &x.1).collect();
                orders.sort();
                if let Some(w) = orders.windows(2).find(|w| w[0] == w[1]) {
                    let id = list.iter().find(|x| &x.1 == w[0]).map(|x| x.0);
                    out.push(Issue { entity: id, ..issue(Level::Warning, "kit-path", format!("{name}: path {p} of “{}” has two spots with order {}.", m.label, if w[0].is_empty() { "(none)" } else { w[0] })) });
                }
            }
            paths.len()
        } else {
            spots.len()
        };
        let r = &m.required;
        if (n as u32) < r.min {
            let what = if m.kind == "path" { "paths" } else { "markers" };
            out.push(Issue { place: Some(format!("{mode}:{}", m.id)), ..issue(Level::Warning, "kit-needs", format!("{name} needs {} “{}” {what}: {}.", count_text(n, r.min, r.max), m.label, m.description.trim_end_matches('.'))) });
        } else if r.max.is_some_and(|x| n as u32 > x) {
            out.push(Issue { entity: spots.last().map(|s| s.0), ..issue(Level::Warning, "kit-too-many", format!("{name} uses {} “{}” at most; the map has {n}.", r.max.unwrap_or(0), m.label)) });
        }
        // the keys, of the right type
        for (id, e) in &spots {
            for k in &m.keys {
                let Some(v) = e.get(&k.name).map(str::trim).filter(|v| !v.is_empty()) else { continue };
                let wrong = match k.kind.as_str() {
                    "number" => v.parse::<f64>().map_or(true, |x| k.min.is_some_and(|m| x < m) || k.max.is_some_and(|m| x > m)),
                    "bool" => v != "0" && v != "1",
                    "choice" => !k.choices.iter().any(|(c, _)| c == v),
                    _ => v.chars().count() > 60,
                };
                if wrong {
                    let want = match k.kind.as_str() {
                        "number" => match (k.min, k.max) {
                            (Some(a), Some(b)) => format!("a number from {a} to {b}"),
                            _ => "a number".into(),
                        },
                        "bool" => "0 or 1".into(),
                        "choice" => format!("one of {}", k.choices.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(", ")),
                        _ => "up to 60 letters".into(),
                    };
                    out.push(Issue { entity: Some(*id), ..issue(Level::Warning, "kit-key", format!("{name}: “{}” key {} is “{v}”; it should be {want}.", m.label, k.name)) });
                }
            }
        }
    }
    for c in &kit.components {
        let n = doc.entities.values().filter(|e| e.classname() == c.classname || (e.classname() == "ls_marker" && for_mode(e, mode) && e.get("kind").map(str::trim) == Some(c.classname.as_str()))).count();
        if (n as u32) < c.min {
            let note = if c.note.is_empty() { String::new() } else { format!(": {}", c.note.trim_end_matches('.')) };
            out.push(Issue { place: Some(format!("{mode}:{}", c.classname)), ..issue(Level::Warning, "kit-needs", format!("{name} needs {} {}{note}.", count_text(n, c.min, c.max), c.classname)) });
        } else if c.max.is_some_and(|x| n as u32 > x) {
            out.push(issue(Level::Warning, "kit-too-many", format!("{name} uses {} {} at most; the map has {n}.", c.max.unwrap_or(0), c.classname)));
        }
    }
    out
}

/// The live checks for every mode the map is made for.
pub fn issues(doc: &MapDocument, kits: &[Kit]) -> Vec<Issue> {
    let mut out = Vec::new();
    for m in made_for(doc) {
        match kits.iter().find(|k| k.mode.id == m) {
            Some(k) => out.extend(check_kit(doc, k)),
            None => out.push(issue(Level::Info, "kit-unknown", format!("The map is made for “{m}”, but that mode isn't installed here, so what it needs can't be checked."))),
        }
    }
    out
}

/// The installed kits, read again only when a kit file changes (the live checks ask often).
pub fn cached() -> Vec<Kit> {
    use std::sync::Mutex;
    type Sig = Vec<(PathBuf, Option<std::time::SystemTime>, u64)>;
    static C: Mutex<Option<(PathBuf, Sig, Vec<Kit>)>> = Mutex::new(None);
    let dir = kits_dir();
    let sig: Sig = std::fs::read_dir(&dir)
        .map(|rd| {
            let mut v: Sig = rd.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".kit.json")).map(|e| {
                let md = e.metadata().ok();
                (e.path(), md.as_ref().and_then(|m| m.modified().ok()), md.map(|m| m.len()).unwrap_or(0))
            }).collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    let mut g = C.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((d, s, k)) = g.as_ref() {
        if *d == dir && *s == sig {
            return k.clone();
        }
    }
    let (k, _) = load(&dir);
    *g = Some((dir, sig, k.clone()));
    k
}

async fn api_kits(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let dir = kits_dir();
        let (kits, bad) = load(&dir);
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let needs: Vec<Issue> = issues(&s.doc, &kits);
        Ok(json!({"folder": dir.to_string_lossy(), "kits": kits, "bad": bad, "madeFor": made_for(&s.doc), "key": MODES_KEY, "needs": needs}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/editor/kits", get(api_kits))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const LAST_STAND: &str = r##"{
      "schema": "loner-strike/mode-kit", "version": 1,
      "mode": { "id": "last_stand", "name": "Last Stand", "description": "Hold the base against waves" },
      "source": "hand",
      "markers": [
        { "id": "creature_spawn", "label": "Creature spawn", "description": "Where creatures come in.", "type": "point",
          "required": { "min": 2, "max": 32 },
          "keys": [ { "name": "wave", "type": "number", "default": 1, "min": 1, "max": 99 },
                    { "name": "side", "type": "choice", "default": "north", "choices": [["north", "North"], ["south", "South"]] } ],
          "draw": { "colour": "#c0392b", "icon": "skull", "shape": "diamond" } },
        { "id": "lst_base", "label": "Base", "type": "area", "required": { "min": 1, "max": 1 }, "draw": { "colour": "#2980b9", "shape": "box", "size": [512, 512, 128] } },
        { "id": "patrol", "label": "Patrol", "type": "path", "required": { "min": 0, "max": null } }
      ],
      "components": [ { "classname": "ls_wave_spawner", "min": 1, "max": 1, "note": "One wave spawner" } ],
      "creatures": [ "zombie", "brute" ]
    }"##;

    fn marker(d: &mut MapDocument, kind: &str, mode: &str, extra: &[(&str, &str)]) {
        let mut e = Entity::new("ls_marker");
        e.keys.push(("kind".into(), kind.into()));
        if !mode.is_empty() {
            e.keys.push(("mode".into(), mode.into()));
        }
        e.keys.push(("origin".into(), "0 0 0".into()));
        for (k, v) in extra {
            e.keys.push((k.to_string(), v.to_string()));
        }
        d.insert_entity(e);
    }

    #[test]
    fn kits_are_checked_against_the_format() {
        let k = parse(LAST_STAND).unwrap();
        assert_eq!(k.markers.len(), 3);
        for (bad, why) in [
            (LAST_STAND.replace("\"version\": 1", "\"version\": 2"), "newer"),
            (LAST_STAND.replace("loner-strike/mode-kit", "other"), "isn't a mode map kit"),
            (LAST_STAND.replace("\"last_stand\"", "\"Last Stand\""), "mode id"),
            (LAST_STAND.replace("\"name\": \"wave\"", "\"name\": \"origin\""), "isn't allowed"),
            (LAST_STAND.replace("\"type\": \"area\"", "\"type\": \"blob\""), "point, area or path"),
            (LAST_STAND.replace("#c0392b", "red"), "drawing"),
            (LAST_STAND.replace("ls_wave_spawner", "func_door"), "ls_ classname"),
            (LAST_STAND.replace("\"creature_spawn\", \"label\"", "\"lst_base\", \"label\""), "used twice"),
        ] {
            let e = parse(&bad).unwrap_err();
            assert!(e.contains(why), "{why}: {e}");
        }
    }

    #[test]
    fn what_a_mode_still_needs() {
        let kit = parse(LAST_STAND).unwrap();
        let mut d = MapDocument::new();
        d.world.keys.push((MODES_KEY.into(), "last_stand royale".into()));
        assert_eq!(made_for(&d), vec!["last_stand", "royale"]);
        let iss = issues(&d, std::slice::from_ref(&kit));
        let codes: Vec<(&str, Option<&str>)> = iss.iter().map(|i| (i.code, i.place.as_deref())).collect();
        assert_eq!(
            codes,
            vec![("kit-needs", Some("last_stand:creature_spawn")), ("kit-needs", Some("last_stand:lst_base")), ("kit-needs", Some("last_stand:ls_wave_spawner")), ("kit-unknown", None)],
            "{iss:#?}"
        );
        assert!(iss[0].message.contains("2–32 (has 0)"), "{}", iss[0].message);

        // two spawns (one for any mode, one for another mode: doesn't count), a base, the spawner as a marker
        marker(&mut d, "creature_spawn", "last_stand", &[("wave", "3"), ("side", "south")]);
        marker(&mut d, "creature_spawn", "any", &[("wave", "0"), ("side", "west")]);
        marker(&mut d, "creature_spawn", "royale", &[]);
        marker(&mut d, "lst_base", "", &[("size", "512 512 128")]);
        marker(&mut d, "lst_base", "last_stand", &[]);
        marker(&mut d, "ls_wave_spawner", "last_stand", &[]);
        // a path with one spot, and one with a repeated order
        marker(&mut d, "patrol", "", &[("path", "1"), ("order", "1")]);
        marker(&mut d, "patrol", "", &[("path", "2"), ("order", "1")]);
        marker(&mut d, "patrol", "", &[("path", "2"), ("order", "1")]);
        let iss = check_kit(&d, &kit);
        let codes: Vec<&str> = iss.iter().map(|i| i.code).collect();
        assert_eq!(codes, vec!["kit-key", "kit-key", "kit-too-many", "kit-path", "kit-path"], "{iss:#?}");
        assert!(iss[0].message.contains("a number from 1 to 99"), "{}", iss[0].message);
        assert!(iss[1].message.contains("one of north, south"), "{}", iss[1].message);
    }
}
