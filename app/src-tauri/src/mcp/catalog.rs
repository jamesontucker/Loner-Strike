//! What agents can do: MCP tools, resources and prompts over Loner-Strike's own Tools.
//!
//! Every tool calls the app's existing routes in-process (the same router the app window
//! uses, through its local guard), so the rules, checks and wording are the tools' own: no
//! logic is copied here. This file only picks routes, checks arguments, trims big answers
//! down to what an agent can read, and applies the connector's rules (changes off unless
//! allowed, the person asked before anything that can't be undone).

use super::{Ctx, Outcome};
use axum::body::Body;
use axum::http::{header, Method, Request};
use serde_json::{json, Map, Value};
use tower::ServiceExt;

/// What a tool may do: look, or change something.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Read,
    /// needs "Allow changes"; undoable or adds only
    Change,
    /// needs "Allow changes" and can overwrite files or stop people playing (asks first)
    Risky,
}

pub struct Spec {
    pub name: &'static str,
    pub title: &'static str,
    pub kind: Kind,
    pub description: &'static str,
    pub schema: fn() -> Value,
}

// ---- schemas -------------------------------------------------------------------------

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}
fn none() -> Value {
    obj(json!({}), &[])
}
fn map_prop() -> Value {
    json!({"type": "string", "description": "Map name without .bsp, e.g. de_dust2"})
}
fn map_only() -> Value {
    obj(json!({"map": map_prop()}), &["map"])
}
fn vec3(what: &str) -> Value {
    json!({"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": what})
}
fn range2(what: &str) -> Value {
    json!({"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2, "description": what})
}
fn server_prop() -> Value {
    json!({"type": "string", "description": "A server id from servers_list, or \"game\" for the game the player hosts from Play"})
}

/// How the editor says nothing is open (tools/editor.rs NO_DOC): the one doc error that's
/// safe to go ahead on.
const EDITOR_EMPTY: &str = "No map is open in the editor";

const EDITOR_OPS: &[&str] = &[
    "add_box", "add_cylinder", "delete", "duplicate", "transform", "move_face", "clip", "hollow", "carve", "merge", "snap",
    "set_texture", "align", "set_key", "add_entity", "tie", "to_world", "set_world", "group", "ungroup", "group_flags",
];

pub fn specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        // maps
        Spec { name: "maps_list", title: "List maps", kind: Read, schema: none,
            description: "The game's maps with what the tools know about each: file size, nav (bot navigation) and its version/places, bot map knowledge (.lsv), missing WADs, recordings." },
        Spec { name: "map_info", title: "Map info", kind: Read, schema: map_only,
            description: "One map's facts: bounds, models, textures and where each comes from (or missing), entity counts by class, spawns, sky." },
        Spec { name: "map_entities", title: "Map entities", kind: Read,
            schema: || obj(json!({"map": map_prop(), "classname": {"type": "string", "description": "Only this class (prefix match, e.g. info_player or func_)"}, "limit": {"type": "integer", "minimum": 1, "maximum": 500}}), &["map"]),
            description: "A compiled map's entities: index, classname, origin, angles, model and keys." },
        Spec { name: "map_nav_summary", title: "Nav summary", kind: Read, schema: map_only,
            description: "The map's bot navigation (.nav): version, whether it fits the map, area count, places with area counts, ladders, one-way and broken links, attribute counts." },
        Spec { name: "map_nav_areas", title: "Nav areas", kind: Read,
            schema: || obj(json!({"map": map_prop(), "ids": {"type": "array", "items": {"type": "integer"}}, "place": {"type": "string"}, "near": vec3("Areas whose middle is within radius of this point"), "radius": {"type": "number"}, "limit": {"type": "integer", "minimum": 1, "maximum": 300}}), &["map"]),
            description: "Nav areas by id, place name or distance from a point: corners (nw/se), heights, links per direction (N E S W), place, attributes." },
        Spec { name: "map_knowledge", title: "Bot map knowledge", kind: Read, schema: map_only,
            description: "The bot brain's learned knowledge for the map (.lsv): stage (0-4), whether it fits the current nav, counts." },
        Spec { name: "map_probe", title: "Find the floor", kind: Read,
            schema: || obj(json!({"map": map_prop(), "x": {"type": "number"}, "y": {"type": "number"}, "z": {"type": "number"}}), &["map", "x", "y", "z"]),
            description: "Traces straight down from a point in a compiled map: floor height and what's there. Handy before placing nav areas or entities." },
        Spec { name: "map_doctor", title: "Map doctor", kind: Read, schema: map_only,
            description: "Runs the map doctor and returns its report: missing textures/models/sounds/sky, the .res download list, nav and knowledge, and fixes it can make." },
        Spec { name: "map_doctor_write_res", title: "Write the .res list", kind: Risky, schema: map_only,
            description: "Writes or updates the map's .res download list (custom files only; a .bak is kept). Asks the person first if a .res is already there." },
        Spec { name: "map_doctor_copy_files", title: "Copy missing files", kind: Change,
            schema: || obj(json!({"map": map_prop(), "files": {"type": "array", "items": {"type": "string"}, "description": "Files the doctor found only in another game folder (as it lists them)"}}), &["map", "files"]),
            description: "Copies files the doctor found only in another game folder into the map's game folder (never replaces a file)." },
        Spec { name: "health_options", title: "Health check options", kind: Read, schema: map_only,
            description: "Whether a map health check can run for a map (and why not), the modes, servers to copy and the preset mode." },
        Spec { name: "health_start", title: "Start a health check", kind: Change,
            schema: || obj(json!({"map": map_prop(), "mode": {"type": "string"}, "minutes": {"type": "integer", "minimum": 1, "maximum": 15}, "bots": {"type": "integer", "minimum": 2, "maximum": 20}, "server": {"type": "string"}}), &["map"]),
            description: "Starts a map health check: bots play the map on a hidden server for a few minutes, then a report says where they fall, get stuck or die to the map. Poll health_progress." },
        Spec { name: "health_progress", title: "Health check progress", kind: Read, schema: none,
            description: "The running health check: phase, what it's doing, time left." },
        Spec { name: "health_cancel", title: "Stop the health check", kind: Change, schema: none, description: "Stops the running health check." },
        Spec { name: "health_reports", title: "Health check reports", kind: Read, schema: none, description: "Earlier health check reports (newest first)." },
        Spec { name: "health_report", title: "Health check report", kind: Read,
            schema: || obj(json!({"id": {"type": "string"}}), &["id"]),
            description: "One health check report: verdict, findings in plain words, falls, stuck spots, deaths to the map, console errors." },
        // the map editor
        Spec { name: "editor_state", title: "Editor: the open map", kind: Read,
            schema: || obj(json!({"limit": {"type": "integer", "minimum": 1, "maximum": 500, "description": "How many brushes/entities to list (default 100)"}}), &[]),
            description: "The map open in the map editor: name, saved/dirty, undo/redo, world keys and WADs, bounds, brushes (id, min, max) and entities (id, classname, keys)." },
        Spec { name: "editor_list_entities", title: "Editor: entities", kind: Read,
            schema: || obj(json!({"classname": {"type": "string", "description": "Prefix match"}}), &[]),
            description: "Entities in the open editor map with their keys and brushes." },
        Spec { name: "editor_new", title: "Editor: new map", kind: Risky, schema: none,
            description: "Starts a new map in the editor: a 1024x1024 open-sky room with 2 CT and 2 T spawns and a sun. Asks the person first if the open map has unsaved changes." },
        Spec { name: "editor_open", title: "Editor: open a project", kind: Risky, schema: none,
            description: "Shows the open-file dialog on the person's screen for a .lsmap project; they pick the file. Asks first if the open map has unsaved changes." },
        Spec { name: "editor_import_map", title: "Editor: import a .map", kind: Risky, schema: none,
            description: "Shows the open-file dialog for a .map file (Valve 220 or standard); the person picks it. Asks first if the open map has unsaved changes." },
        Spec { name: "editor_add_box", title: "Editor: add a box", kind: Change,
            schema: || obj(json!({"min": vec3("Lowest corner [x, y, z]"), "max": vec3("Highest corner [x, y, z]"), "texture": {"type": "string"}}), &["min", "max"]),
            description: "Adds a box brush between two corners (GoldSrc units, Z up). Undoable." },
        Spec { name: "editor_add_cylinder", title: "Editor: add a cylinder", kind: Change,
            schema: || obj(json!({"base": vec3("Middle of the bottom [x, y, z]"), "radius": {"type": "number"}, "height": {"type": "number"}, "sides": {"type": "integer", "minimum": 3, "maximum": 64}, "texture": {"type": "string"}}), &["base", "radius", "height"]),
            description: "Adds an upright cylinder brush. Undoable." },
        Spec { name: "editor_add_entity", title: "Editor: add an entity", kind: Change,
            schema: || obj(json!({"classname": {"type": "string", "description": "e.g. info_player_start, info_player_deathmatch, light, weapon_ak47"}, "origin": vec3("Where [x, y, z]"), "keys": {"type": "object", "additionalProperties": {"type": "string"}}}), &["classname", "origin"]),
            description: "Adds a point entity (spawns, lights, weapons...). Undoable." },
        Spec { name: "editor_set_key", title: "Editor: set an entity key", kind: Change,
            schema: || obj(json!({"entity": {"type": "integer"}, "key": {"type": "string"}, "value": {"type": "string", "description": "Empty removes the key"}}), &["entity", "key", "value"]),
            description: "Sets (or with an empty value removes) a key on an entity. Undoable." },
        Spec { name: "editor_command", title: "Editor: other edits", kind: Change,
            schema: || obj(json!({"op": {"type": "string", "enum": EDITOR_OPS}, "args": {"type": "object", "description": "The op's fields, e.g. delete {brushes:[1], entities:[]}, transform {brushes, entities, translate:[x,y,z]}, hollow {brushes, thickness}, set_texture {faces:[[brush, null]], texture}, set_world {wads, keys}"}}), &["op"]),
            description: "Any map editor command (the same ones the editor page sends): delete, duplicate, transform, move_face, clip, hollow, carve, merge, snap, set_texture, align, tie, to_world, set_world, group... All undoable." },
        Spec { name: "editor_undo", title: "Editor: undo", kind: Change, schema: none, description: "Undoes the last editor change." },
        Spec { name: "editor_redo", title: "Editor: redo", kind: Change, schema: none, description: "Redoes the last undone editor change." },
        Spec { name: "editor_checks", title: "Editor: check the map", kind: Read, schema: none,
            description: "Live checks on the open map: leaks, missing spawns, bad brushes, missing textures, limits." },
        Spec { name: "editor_textures", title: "Editor: textures", kind: Read,
            schema: || obj(json!({"search": {"type": "string"}, "limit": {"type": "integer", "minimum": 1, "maximum": 500}}), &[]),
            description: "Textures in the open map's WADs (name, size), optionally filtered." },
        Spec { name: "editor_save", title: "Editor: save", kind: Risky,
            schema: || obj(json!({"as": {"type": "boolean", "description": "Pick a new file (the person chooses it in a dialog)"}}), &[]),
            description: "Saves the editor project. The first time (or with as) the person picks the file in a dialog; saving over the project it came from asks them first." },
        Spec { name: "editor_export_map", title: "Editor: export .map", kind: Change, schema: none,
            description: "Exports the open map as a Valve 220 .map for the compilers; the person picks where in a save dialog." },
        // the nav fixer
        Spec { name: "nav_open", title: "Nav fixer: open", kind: Read, schema: map_only,
            description: "Opens (or returns) the nav fixer session for a map: area count, unsaved changes, undo/redo, problems with suggested fixes, knowledge status." },
        Spec { name: "nav_problems", title: "Nav fixer: problems", kind: Read, schema: map_only,
            description: "Problems in the map's nav (unreachable areas, traps, broken links, ladders with no end, cramped areas) with area ids, a spot and a one-click fix edit when there is one." },
        Spec { name: "nav_add_area", title: "Nav fixer: add an area", kind: Change,
            schema: || obj(json!({"map": map_prop(), "x": range2("[x1, x2]"), "y": range2("[y1, y2]"), "z": {"type": "number", "description": "Height (dropped to the floor unless snap is false)"}, "place": {"type": "string"}, "attr": {"type": "integer", "description": "crouch 1, jump 2, precise 4, no jump 8"}, "snap": {"type": "boolean"}}), &["map", "x", "y", "z"]),
            description: "Adds a flat nav area over the box x by y at height z. Undoable until saved." },
        Spec { name: "nav_link", title: "Nav fixer: link areas", kind: Change,
            schema: || obj(json!({"map": map_prop(), "a": {"type": "integer"}, "b": {"type": "integer"}, "oneWay": {"type": "boolean", "description": "a drop from a to b that can't be climbed back"}}), &["map", "a", "b"]),
            description: "Links area a to b (both ways unless oneWay). Undoable until saved." },
        Spec { name: "nav_unlink", title: "Nav fixer: unlink areas", kind: Change,
            schema: || obj(json!({"map": map_prop(), "a": {"type": "integer"}, "b": {"type": "integer"}, "both": {"type": "boolean"}}), &["map", "a", "b"]),
            description: "Removes the link a to b (and b to a unless both is false). Undoable until saved." },
        Spec { name: "nav_edit", title: "Nav fixer: other edits", kind: Change,
            schema: || obj(json!({"map": map_prop(), "edit": {"type": "object", "description": "{op: delete, ids} | {op: reshape, id, x, y, z?} | {op: attr, ids, set, clear} | {op: place, ids, name} | a problem's fix edit"}}), &["map", "edit"]),
            description: "Any nav fixer edit, including a problem's suggested fix. Undoable until saved." },
        Spec { name: "nav_undo", title: "Nav fixer: undo", kind: Change, schema: map_only, description: "Undoes the last nav edit." },
        Spec { name: "nav_redo", title: "Nav fixer: redo", kind: Change, schema: map_only, description: "Redoes the last undone nav edit." },
        Spec { name: "nav_save", title: "Nav fixer: save", kind: Risky, schema: map_only,
            description: "Writes the edited nav over the map's .nav (a .bak of the original is kept the first time). Asks the person first." },
        // replay
        Spec { name: "replay_list", title: "Recordings", kind: Read, schema: none,
            description: "Bot recordings (health checks, the sim): id, map, when, length, bots, kills." },
        Spec { name: "replay_summary", title: "Recording summary", kind: Read,
            schema: || obj(json!({"id": {"type": "string"}}), &["id"]),
            description: "One recording in numbers: players, rounds, kills (with headshots, weapons, deaths to the world), falls with where they started and the drop, and stuck spots." },
        // servers
        Spec { name: "servers_list", title: "Servers", kind: Read, schema: none,
            description: "The player's own dedicated servers on this computer (id, name, state, map, port) and the game they host from Play." },
        Spec { name: "server_status", title: "Server status", kind: Read, schema: || obj(json!({"server": server_prop()}), &["server"]),
            description: "A running server's map, mode and players (people and bots)." },
        Spec { name: "server_start", title: "Start a server", kind: Change,
            schema: || obj(json!({"server": {"type": "string"}, "mode": {"type": "string", "description": "Game mode id to switch to once it's up"}}), &["server"]),
            description: "Starts one of the player's dedicated servers on this computer." },
        Spec { name: "server_stop", title: "Stop a server", kind: Risky, schema: || obj(json!({"server": {"type": "string"}}), &["server"]),
            description: "Stops one of the player's dedicated servers. Asks the person first if anyone is playing on it." },
        Spec { name: "server_change_map", title: "Change map or mode", kind: Risky,
            schema: || obj(json!({"server": server_prop(), "map": map_prop(), "mode": {"type": "string", "description": "Game mode id (leave out to keep the mode)"}}), &["server"]),
            description: "Changes a running server's (or the hosted game's) map and/or mode. Asks the person first if anyone is playing." },
        Spec { name: "server_set_bots", title: "Set the bot count", kind: Change,
            schema: || obj(json!({"server": server_prop(), "count": {"type": "integer", "minimum": 0, "maximum": 31}}), &["server", "count"]),
            description: "Sets how many bots a running server (or the hosted game) has." },
    ]
}

pub fn has_tool(name: &str) -> bool {
    specs().iter().any(|s| s.name == name)
}

pub fn tool_list() -> Vec<Value> {
    specs()
        .iter()
        .map(|s| {
            let note = match s.kind {
                Kind::Read => "",
                Kind::Change => " (Needs \"Allow changes\".)",
                Kind::Risky => " (Needs \"Allow changes\"; may ask the person at the computer.)",
            };
            json!({
                "name": s.name,
                "title": s.title,
                "description": format!("{}{note}", s.description),
                "inputSchema": (s.schema)(),
                "annotations": {
                    "title": s.title,
                    "readOnlyHint": s.kind == Kind::Read,
                    "destructiveHint": s.kind == Kind::Risky,
                    "openWorldHint": false,
                },
            })
        })
        .collect()
}

// ---- calling the app ------------------------------------------------------------------

/// Why a tool didn't do it.
#[derive(Debug)]
pub enum Fail {
    /// the connector's rules or the person said no
    Refused(String),
    /// the tool itself (its own words) or bad arguments
    Error(String),
}

type Done = Result<Value, Fail>;

fn err(m: impl Into<String>) -> Fail {
    Fail::Error(m.into())
}

/// One request to the app's own routes, as the app window makes it.
pub async fn call(ctx: &Ctx, method: Method, path: &str, target: Option<&str>, body: Option<Value>) -> Done {
    let post = method == Method::POST;
    let mut b = Request::builder().method(method).uri(path).header(header::HOST, &ctx.host);
    if post {
        b = b.header("x-panel", "1").header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(t) = target {
        b = b.header("x-target", t);
    }
    let req = b
        .body(match body {
            Some(v) => Body::from(v.to_string()),
            None if post => Body::from("{}"),
            None => Body::empty(),
        })
        .map_err(|_| err("Couldn't make that request."))?;
    let res = ctx.app.clone().oneshot(req).await.map_err(|_| err("The app didn't answer."))?;
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 512 << 20).await.map_err(|_| err("The app's answer was too big."))?;
    let v = if bytes.starts_with(b"LSMB") { lsmb_header(&bytes) } else { serde_json::from_slice::<Value>(&bytes).ok() };
    match v {
        Some(v) => match v.get("error").and_then(Value::as_str) {
            Some(e) => Err(err(e)),
            None => Ok(v),
        },
        None if status.is_success() => Ok(json!({"ok": true})),
        None => Err(err(format!("The app answered {status}: {}", String::from_utf8_lossy(&bytes[..bytes.len().min(200)])))),
    }
}

async fn get(ctx: &Ctx, path: &str) -> Done {
    call(ctx, Method::GET, path, None, None).await
}
async fn post(ctx: &Ctx, path: &str, body: Value) -> Done {
    call(ctx, Method::POST, path, None, Some(body)).await
}

/// The JSON header of an LSMB bundle ("LSMB", u32 version, u32 JSON length, JSON...).
pub fn lsmb_header(b: &[u8]) -> Option<Value> {
    if b.len() < 12 || &b[0..4] != b"LSMB" {
        return None;
    }
    let n = u32::from_le_bytes(b[8..12].try_into().ok()?) as usize;
    let mut v: Value = serde_json::from_slice(b.get(12..12usize.checked_add(n)?)?).ok()?;
    if let Some(o) = v.as_object_mut() {
        o.remove("sections");
    }
    Some(v)
}

/// The person at the computer says yes, or the tool stops.
async fn ask(ctx: &Ctx, text: String) -> Result<(), Fail> {
    let confirm = ctx.confirm.clone();
    let yes = tokio::task::spawn_blocking(move || confirm(text)).await.unwrap_or(false);
    if yes {
        Ok(())
    } else {
        Err(Fail::Refused("The person at the computer didn't allow it.".into()))
    }
}

// ---- arguments ------------------------------------------------------------------------

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn map_arg(a: &Value) -> Result<String, Fail> {
    let m = s(a, "map").ok_or_else(|| err("Say which map (map: \"de_dust2\")."))?;
    let m = m.strip_suffix(".bsp").unwrap_or(m);
    if !crate::maps::safe_arg(m) {
        return Err(err("That isn't a map name."));
    }
    Ok(m.to_string())
}

/// An id going into a route's path: letters, digits, - and _ only.
fn id_arg(a: &Value, k: &str) -> Result<String, Fail> {
    let v = s(a, k).ok_or_else(|| err(format!("Missing {k}.")))?;
    if v.is_empty() || v.len() > 64 || !v.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
        return Err(err(format!("That isn't a {k}.")));
    }
    Ok(v.to_string())
}

fn limit(a: &Value, default: usize, max: usize) -> usize {
    a.get("limit").and_then(Value::as_u64).map(|n| (n as usize).clamp(1, max)).unwrap_or(default)
}

fn vec3_arg(a: &Value, k: &str) -> Result<Value, Fail> {
    match a.get(k).and_then(Value::as_array) {
        Some(v) if v.len() == 3 && v.iter().all(|x| x.as_f64().is_some_and(f64::is_finite)) => Ok(Value::Array(v.clone())),
        _ => Err(err(format!("{k} must be [x, y, z]."))),
    }
}

/// Copies the listed fields that are there.
fn pick(a: &Value, keys: &[&str]) -> Map<String, Value> {
    keys.iter().filter_map(|k| a.get(*k).map(|v| (k.to_string(), v.clone()))).collect()
}

// ---- trimming -------------------------------------------------------------------------

/// Long lists cut to `n` items, with `<key>Omitted` saying how many went.
fn cut(v: &mut Value, n: usize) {
    match v {
        Value::Object(o) => {
            let mut omitted = vec![];
            for (k, x) in o.iter_mut() {
                if let Value::Array(a) = x {
                    if a.len() > n {
                        omitted.push((format!("{k}Omitted"), a.len() - n));
                        a.truncate(n);
                    }
                }
                cut(x, n);
            }
            for (k, c) in omitted {
                o.insert(k, json!(c));
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| cut(x, n)),
        _ => {}
    }
}

/// Small enough for an agent to read (about 48 KB of JSON at most).
pub fn trimmed(mut v: Value) -> Value {
    const MAX: usize = 48 * 1024;
    if let Value::Array(a) = &v {
        if v.to_string().len() > MAX {
            v = json!({"items": a.clone()});
        }
    }
    for n in [200, 60, 20, 5, 1] {
        if v.to_string().len() <= MAX {
            break;
        }
        cut(&mut v, n);
    }
    v
}

// ---- the tools ------------------------------------------------------------------------

/// Runs a tool under the connector's rules and logs what happened.
pub async fn call_tool(ctx: &Ctx, name: &str, args: Value) -> Value {
    let kind = specs().iter().find(|s| s.name == name).map(|s| s.kind).unwrap_or(Kind::Read);
    let detail = super::short(&args);
    let r = if kind != Kind::Read && !ctx.changes {
        Err(Fail::Refused("Changes are off. The person can turn on \"Allow changes\" under AI agents in Loner-Strike's Settings; until then only reading works.".into()))
    } else {
        run(ctx, name, &args).await
    };
    let (outcome, text, is_error) = match r {
        Ok(v) => (Outcome::Done, trimmed(v).to_string(), false),
        Err(Fail::Refused(m)) => (Outcome::Refused, m, true),
        Err(Fail::Error(m)) => (Outcome::Failed, m, true),
    };
    let note = if is_error { text.chars().take(160).collect() } else { String::new() };
    ctx.log.add(name, &detail, outcome, &note);
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

async fn run(ctx: &Ctx, name: &str, a: &Value) -> Done {
    match name {
        "maps_list" => get(ctx, "/api/tools/maps").await,
        "map_info" => get(ctx, &format!("/api/tools/map/{}/info", map_arg(a)?)).await,
        "map_entities" => {
            let v = get(ctx, &format!("/api/tools/map/{}/entities", map_arg(a)?)).await?;
            let class = s(a, "classname").unwrap_or("");
            let all = v.as_array().cloned().unwrap_or_default();
            let total = all.len();
            let picked: Vec<Value> = all.into_iter().filter(|e| e["classname"].as_str().unwrap_or("").starts_with(class)).collect();
            let matched = picked.len();
            Ok(json!({"total": total, "matched": matched, "entities": picked.into_iter().take(limit(a, 200, 500)).collect::<Vec<_>>()}))
        }
        "map_nav_summary" => {
            let v = get(ctx, &format!("/api/tools/map/{}/nav", map_arg(a)?)).await?;
            Ok(nav_summary(&v))
        }
        "map_nav_areas" => {
            let v = get(ctx, &format!("/api/tools/map/{}/nav", map_arg(a)?)).await?;
            nav_areas(&v, a)
        }
        "map_knowledge" => get(ctx, &format!("/api/tools/map/{}/know", map_arg(a)?)).await,
        "map_probe" => {
            let m = map_arg(a)?;
            let n = |k: &str| a.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| err(format!("{k} must be a number.")));
            get(ctx, &format!("/api/tools/map/{m}/probe?x={}&y={}&z={}&dir=down", n("x")?, n("y")?, n("z")?)).await
        }
        "map_doctor" => get(ctx, &format!("/api/tools/doctor/{}", map_arg(a)?)).await,
        "map_doctor_write_res" => {
            let m = map_arg(a)?;
            let report = get(ctx, &format!("/api/tools/doctor/{m}")).await?;
            if report["res"]["exists"] == true {
                ask(ctx, format!("An AI agent wants to update {m}.res, the list of files players download with the map. A .bak copy is kept. Allow it?")).await?;
            }
            post(ctx, &format!("/api/tools/doctor/{m}/res"), json!({})).await
        }
        "map_doctor_copy_files" => {
            let m = map_arg(a)?;
            let files = a.get("files").filter(|f| f.as_array().is_some_and(|l| l.iter().all(Value::is_string))).ok_or_else(|| err("files must be a list of file names."))?;
            post(ctx, &format!("/api/tools/doctor/{m}/copy"), json!({"files": files})).await
        }
        "health_options" => get(ctx, &format!("/api/tools/health/options?map={}", map_arg(a)?)).await,
        "health_start" => {
            let m = map_arg(a)?;
            let mut b = pick(a, &["mode", "minutes", "bots", "server"]);
            b.insert("map".into(), json!(m));
            post(ctx, "/api/tools/health/start", Value::Object(b)).await
        }
        "health_progress" => get(ctx, "/api/tools/health/progress").await,
        "health_cancel" => post(ctx, "/api/tools/health/cancel", json!({})).await,
        "health_reports" => get(ctx, "/api/tools/health/reports").await,
        "health_report" => get(ctx, &format!("/api/tools/health/report/{}", id_arg(a, "id")?)).await,

        "editor_state" => {
            let mut v = get(ctx, "/api/tools/editor/doc").await?;
            let n = limit(a, 100, 500);
            for k in ["brushes", "entities"] {
                if let Some(list) = v.get_mut(k).and_then(Value::as_array_mut) {
                    let total = list.len();
                    list.truncate(n);
                    v[format!("{k}Total")] = json!(total);
                }
            }
            Ok(v)
        }
        "editor_list_entities" => {
            let v = get(ctx, "/api/tools/editor/doc").await?;
            let class = s(a, "classname").unwrap_or("");
            let list: Vec<Value> = v["entities"].as_array().into_iter().flatten().filter(|e| e["classname"].as_str().unwrap_or("").starts_with(class)).cloned().collect();
            Ok(json!({"entities": list}))
        }
        "editor_new" | "editor_open" | "editor_import_map" => {
            // (unsaved changes, or no way to tell: ask; only nothing open at all goes ahead)
            match get(ctx, "/api/tools/editor/doc").await {
                Ok(doc) if doc["state"]["dirty"] == false => {}
                Ok(doc) => {
                    let what = doc["state"]["name"].as_str().unwrap_or("the open map").to_string();
                    ask(ctx, format!("An AI agent wants to close \"{what}\" in the map editor. It has changes that aren't saved, and they'd be lost. Allow it?")).await?;
                }
                Err(Fail::Error(e)) if e.starts_with(EDITOR_EMPTY) => {}
                Err(_) => ask(ctx, "An AI agent wants to close the map open in the map editor. Loner-Strike couldn't tell whether it has unsaved changes, which would be lost. Allow it?".into()).await?,
            }
            let path = match name {
                "editor_new" => "/api/tools/editor/new",
                "editor_open" => "/api/tools/editor/open",
                _ => "/api/tools/editor/import-map",
            };
            post(ctx, path, json!({})).await
        }
        "editor_add_box" => {
            let mut b = json!({"op": "add_box", "min": vec3_arg(a, "min")?, "max": vec3_arg(a, "max")?});
            if let Some(t) = s(a, "texture") {
                b["texture"] = json!(t);
            }
            editor_cmd(ctx, b).await
        }
        "editor_add_cylinder" => {
            let mut b = json!({"op": "add_cylinder", "base": vec3_arg(a, "base")?});
            b.as_object_mut().unwrap().extend(pick(a, &["radius", "height", "sides", "texture"]));
            if b.get("sides").is_none() {
                b["sides"] = json!(16);
            }
            editor_cmd(ctx, b).await
        }
        "editor_add_entity" => {
            let class = s(a, "classname").filter(|c| !c.is_empty()).ok_or_else(|| err("Say which classname."))?;
            let mut b = json!({"op": "add_entity", "classname": class, "origin": vec3_arg(a, "origin")?});
            if let Some(k) = a.get("keys").filter(|k| k.is_object()) {
                // (the editor takes keys as [[key, value]] pairs)
                let pairs: Vec<Value> = k.as_object().unwrap().iter().map(|(k, v)| json!([k, v.as_str().map(String::from).unwrap_or_else(|| v.to_string())])).collect();
                b["keys"] = json!(pairs);
            }
            editor_cmd(ctx, b).await
        }
        "editor_set_key" => {
            let mut b = json!({"op": "set_key"});
            b.as_object_mut().unwrap().extend(pick(a, &["entity", "key", "value"]));
            editor_cmd(ctx, b).await
        }
        "editor_command" => {
            let op = s(a, "op").filter(|o| EDITOR_OPS.contains(o)).ok_or_else(|| err(format!("op must be one of: {}.", EDITOR_OPS.join(", "))))?;
            let mut b = a.get("args").and_then(Value::as_object).cloned().unwrap_or_default();
            b.insert("op".into(), json!(op));
            editor_cmd(ctx, Value::Object(b)).await
        }
        "editor_undo" => post(ctx, "/api/tools/editor/undo", json!({})).await,
        "editor_redo" => post(ctx, "/api/tools/editor/redo", json!({})).await,
        "editor_checks" => get(ctx, "/api/tools/editor/checks").await,
        "editor_textures" => {
            let v = get(ctx, "/api/tools/editor/textures").await?;
            let q = s(a, "search").unwrap_or("").to_ascii_lowercase();
            let n = limit(a, 200, 500);
            let wads: Vec<Value> = v["wads"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|w| {
                    let tex: Vec<Value> = w["textures"].as_array().into_iter().flatten().filter(|t| t["name"].as_str().unwrap_or("").to_ascii_lowercase().contains(&q)).take(n).cloned().collect();
                    json!({"name": w["name"], "found": w["found"], "error": w["error"], "textures": tex})
                })
                .collect();
            Ok(json!({"wads": wads, "noGame": v["noGame"]}))
        }
        "editor_save" => {
            let save_as = a.get("as").and_then(Value::as_bool).unwrap_or(false);
            let doc = get(ctx, "/api/tools/editor/doc").await?;
            if doc["state"]["saved"] == true && !save_as {
                let what = doc["state"]["name"].as_str().unwrap_or("the project").to_string();
                ask(ctx, format!("An AI agent wants to save the map editor's changes over \"{what}\". Allow it?")).await?;
            }
            post(ctx, "/api/tools/editor/save", json!({"as": save_as})).await
        }
        "editor_export_map" => post(ctx, "/api/tools/editor/export-map", json!({})).await,

        "nav_open" | "nav_problems" => {
            let v = get(ctx, &format!("/api/tools/navfix/{}", map_arg(a)?)).await?;
            Ok(if name == "nav_open" { nav_session(&v) } else { json!({"problems": v["problems"]}) })
        }
        "nav_add_area" => {
            let mut e = pick(a, &["x", "y", "z", "place", "attr", "snap"]);
            e.insert("op".into(), json!("add"));
            nav_edit(ctx, a, Value::Object(e)).await
        }
        "nav_link" => {
            let mut e = pick(a, &["a", "b", "oneWay"]);
            e.insert("op".into(), json!("link"));
            nav_edit(ctx, a, Value::Object(e)).await
        }
        "nav_unlink" => {
            let mut e = pick(a, &["a", "b", "both"]);
            e.insert("op".into(), json!("unlink"));
            nav_edit(ctx, a, Value::Object(e)).await
        }
        "nav_edit" => {
            let e = a.get("edit").filter(|e| e["op"].is_string()).cloned().ok_or_else(|| err("edit must be an object with an op."))?;
            nav_edit(ctx, a, e).await
        }
        "nav_undo" | "nav_redo" => {
            let m = map_arg(a)?;
            let v = post(ctx, &format!("/api/tools/navfix/{m}/{}", if name == "nav_undo" { "undo" } else { "redo" }), json!({})).await?;
            Ok(nav_session(&v))
        }
        "nav_save" => {
            let m = map_arg(a)?;
            ask(ctx, format!("An AI agent wants to save its changes to {m}.nav, the bots' map of {m}. The original is kept as {m}.nav.bak the first time. Allow it?")).await?;
            let v = post(ctx, &format!("/api/tools/navfix/{m}/save"), json!({})).await?;
            Ok(nav_session(&v))
        }

        "replay_list" => get(ctx, "/api/tools/recordings").await,
        "replay_summary" => {
            let v = get(ctx, &format!("/api/tools/recording/{}", id_arg(a, "id")?)).await?;
            Ok(replay_summary(&v))
        }

        "servers_list" => {
            let v = get(ctx, "/api/servers").await?;
            let servers: Vec<Value> = v["servers"].as_array().into_iter().flatten().filter(|e| e["host"].is_null() && e["shared"] != true && e["id"].is_string()).map(server_view).collect();
            let game = call(ctx, Method::GET, "/api/state", None, None).await.ok().map(|s| state_view(&s));
            Ok(json!({"servers": servers, "game": game}))
        }
        "server_status" => {
            let t = target_arg(ctx, a).await?;
            let v = call(ctx, Method::GET, "/api/state", t.as_deref(), None).await?;
            Ok(state_view(&v))
        }
        "server_start" => {
            let id = own_server(ctx, a).await?;
            let mut b = json!({});
            if let Some(m) = s(a, "mode") {
                b["mode"] = json!(m);
            }
            post(ctx, &format!("/api/servers/{id}/start"), b).await
        }
        "server_stop" => {
            let id = own_server(ctx, a).await?;
            match people_on(ctx, Some(&id)).await {
                Some(0) => {}
                Some(n) => ask(ctx, format!("An AI agent wants to stop the server \"{id}\". {} on it right now. Allow it?", plural(n))).await?,
                None => ask(ctx, format!("An AI agent wants to stop the server \"{id}\". Loner-Strike couldn't tell who's on it. Allow it?")).await?,
            }
            post(ctx, &format!("/api/servers/{id}/stop"), json!({})).await
        }
        "server_change_map" => {
            let t = target_arg(ctx, a).await?;
            let map = match s(a, "map") {
                Some(_) => Some(map_arg(a)?),
                None => None,
            };
            let mode = s(a, "mode").map(String::from);
            if let Some(m) = &mode {
                if !crate::maps::safe_arg(m) {
                    return Err(err("That isn't a mode id."));
                }
            }
            let (cmd, args) = match (&mode, &map) {
                (Some(mode), Some(map)) => ("gm_mode", vec![mode.clone(), map.clone()]),
                (Some(mode), None) => ("gm_mode", vec![mode.clone()]),
                (None, Some(map)) => ("gm_map", vec![map.clone()]),
                (None, None) => return Err(err("Say a map, a mode or both.")),
            };
            let people = people_on(ctx, t.as_deref()).await;
            if people != Some(0) {
                let where_ = t.as_deref().map(|id| format!("the server \"{id}\"")).unwrap_or_else(|| "your game".into());
                let to = [mode.as_deref().map(|m| format!("mode {m}")), map.as_deref().map(|m| format!("map {m}"))].into_iter().flatten().collect::<Vec<_>>().join(", ");
                let who = people.map(|n| format!("{} playing.", plural(n))).unwrap_or_else(|| "Loner-Strike couldn't tell who's playing.".into());
                ask(ctx, format!("An AI agent wants to change {where_} to {to}. {who} Allow it?")).await?;
            }
            call(ctx, Method::POST, "/api/cmd", t.as_deref(), Some(json!({"cmd": cmd, "args": args}))).await
        }
        "server_set_bots" => {
            let t = target_arg(ctx, a).await?;
            let n = a.get("count").and_then(Value::as_u64).filter(|n| *n <= 31).ok_or_else(|| err("count must be 0 to 31."))?;
            call(ctx, Method::POST, "/api/cmd", t.as_deref(), Some(json!({"cmd": "gm_bots", "args": ["count", n.to_string()]}))).await
        }
        _ => Err(err(format!("Unknown tool: {name}"))),
    }
}

fn plural(n: usize) -> String {
    if n == 1 {
        "1 person is".into()
    } else {
        format!("{n} people are")
    }
}

async fn editor_cmd(ctx: &Ctx, body: Value) -> Done {
    post(ctx, "/api/tools/editor/cmd", body).await
}

async fn nav_edit(ctx: &Ctx, a: &Value, edit: Value) -> Done {
    let m = map_arg(a)?;
    let v = post(ctx, &format!("/api/tools/navfix/{m}/edit"), edit).await?;
    Ok(nav_session(&v))
}

/// The nav fixer's whole-state answer, without the whole nav.
fn nav_session(v: &Value) -> Value {
    let mut out = Map::new();
    for (k, x) in v.as_object().into_iter().flatten() {
        if k != "nav" && k != "reach" {
            out.insert(k.clone(), x.clone());
        }
    }
    let areas = v["nav"]["areas"].as_array().map(Vec::len).unwrap_or(0);
    out.insert("areas".into(), json!(areas));
    if let Some(r) = v["reach"].as_array() {
        let count = |n: i64| r.iter().filter(|x| x.as_i64() == Some(n)).count();
        out.insert("unreachable".into(), json!(count(1)));
        out.insert("traps".into(), json!(count(2)));
    }
    Value::Object(out)
}

fn place_name(nav: &Value, p: &Value) -> Value {
    match p.as_u64() {
        Some(0) | None => Value::Null,
        Some(i) => nav["places"].get(i as usize - 1).cloned().unwrap_or(Value::Null),
    }
}

fn nav_summary(v: &Value) -> Value {
    let areas = v["areas"].as_array().cloned().unwrap_or_default();
    let mut places: std::collections::BTreeMap<String, usize> = Default::default();
    let mut unnamed = 0;
    let (mut crouch, mut jump, mut hiding) = (0, 0, 0);
    for a in &areas {
        match place_name(v, &a["place"]).as_str() {
            Some(p) => *places.entry(p.to_string()).or_default() += 1,
            None => unnamed += 1,
        }
        let attr = a["attr"].as_u64().unwrap_or(0);
        crouch += (attr & 1 != 0) as usize;
        jump += (attr & 2 != 0) as usize;
        hiding += a["hiding"].as_array().map(Vec::len).unwrap_or(0);
    }
    let ladders = v["ladders"].as_array().cloned().unwrap_or_default();
    json!({
        "version": v["version"], "matchesMap": v["matchesMap"], "areas": areas.len(),
        "places": places, "unnamedAreas": unnamed, "crouchAreas": crouch, "jumpAreas": jump, "hidingSpots": hiding,
        "ladders": ladders.len(), "danglingLadders": ladders.iter().filter(|l| l["dangling"] == true).count(),
        "oneWayLinks": v["oneWay"], "danglingLinks": v["danglingLinks"],
    })
}

fn nav_areas(v: &Value, a: &Value) -> Done {
    let ids: Option<Vec<u64>> = a.get("ids").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).collect());
    let place = s(a, "place");
    let near = match a.get("near") {
        Some(_) => Some(vec3_arg(a, "near")?),
        None => None,
    };
    let radius = a.get("radius").and_then(Value::as_f64).unwrap_or(512.0);
    let n = limit(a, 100, 300);
    let mut out = vec![];
    for x in v["areas"].as_array().into_iter().flatten() {
        if ids.as_ref().is_some_and(|ids| !x["id"].as_u64().is_some_and(|i| ids.contains(&i))) {
            continue;
        }
        let pname = place_name(v, &x["place"]);
        if place.is_some_and(|p| pname.as_str() != Some(p)) {
            continue;
        }
        if let Some(p) = &near {
            let f = |v: &Value, i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(0.0);
            let (cx, cy) = ((f(&x["nw"], 0) + f(&x["se"], 0)) / 2.0, (f(&x["nw"], 1) + f(&x["se"], 1)) / 2.0);
            let cz = (f(&x["nw"], 2) + f(&x["se"], 2)) / 2.0;
            let d = ((cx - f(p, 0)).powi(2) + (cy - f(p, 1)).powi(2) + (cz - f(p, 2)).powi(2)).sqrt();
            if d > radius {
                continue;
            }
        }
        out.push(json!({"id": x["id"], "nw": x["nw"], "se": x["se"], "neZ": x["neZ"], "swZ": x["swZ"], "attr": x["attr"],
            "place": pname, "links": {"n": x["conns"][0], "e": x["conns"][1], "s": x["conns"][2], "w": x["conns"][3]},
            "hidingSpots": x["hiding"].as_array().map(Vec::len)}));
    }
    let matched = out.len();
    out.truncate(n);
    Ok(json!({"matched": matched, "areas": out}))
}

fn replay_summary(v: &Value) -> Value {
    let players = v["players"].as_array().cloned().unwrap_or_default();
    let name = |id: &Value| players.iter().find(|p| p["id"] == *id).and_then(|p| p["name"].as_str()).unwrap_or("?").to_string();
    let kills = v["kills"].as_array().cloned().unwrap_or_default();
    let world = kills.iter().filter(|k| k["killer"] == k["victim"] || k["cause"] != "weapon").count();
    let headshots = kills.iter().filter(|k| k["headshot"] == true).count();
    let mut weapons: std::collections::BTreeMap<String, usize> = Default::default();
    for k in &kills {
        if let Some(w) = k["weapon"].as_str().filter(|w| !w.is_empty()) {
            *weapons.entry(w.to_string()).or_default() += 1;
        }
    }
    let falls: Vec<Value> = kills
        .iter()
        .filter(|k| !k["fall"].is_null())
        .map(|k| json!({"t": k["t"], "who": name(&k["victim"]), "from": k["fall"]["from"], "drop": k["fall"]["drop"], "speed": k["fall"]["speed"], "at": k["at"]}))
        .collect();
    let stuck: Vec<Value> = v["stuck"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|st| json!({"who": name(&st["id"]), "from": st["from"], "seconds": st["to"].as_f64().zip(st["from"].as_f64()).map(|(b, a)| ((b - a) * 10.0).round() / 10.0), "at": st["at"]}))
        .collect();
    json!({
        "id": v["id"], "map": v["map"], "file": v["file"], "hasTrace": v["hasTrace"],
        "start": v["start"], "end": v["end"], "seconds": v["seconds"],
        "players": players.iter().map(|p| json!({"id": p["id"], "name": p["name"], "bot": p["bot"], "team": p["team"]})).collect::<Vec<_>>(),
        "rounds": v["rounds"].as_array().map(Vec::len),
        "kills": {"total": kills.len(), "headshots": headshots, "toTheWorld": world, "byWeapon": weapons},
        "falls": falls, "stuck": stuck,
    })
}

/// A server as the list shows it, without its password or folder.
fn server_view(e: &Value) -> Value {
    json!({"id": e["id"], "name": e["name"], "game": e["game"], "state": e["state"], "map": e["map"], "port": e["port"],
           "maxplayers": e["maxplayers"], "ready": e["ready"], "uptime": e["uptime"], "error": e["error"]})
}

/// A game's state in brief: map, mode and who's on.
fn state_view(v: &Value) -> Value {
    if v["online"] != true {
        return json!({"online": false, "reason": v["reason"]});
    }
    let players = v["players"].as_array().cloned().unwrap_or_default();
    let bots = players.iter().filter(|p| p["bot"].as_i64() == Some(1)).count();
    let mode = v["modes"].as_array().into_iter().flatten().find(|m| m["active"] == true || m["current"] == true).cloned();
    json!({"online": true, "map": v["map"], "mode": mode.as_ref().map(|m| m["id"].clone()).or_else(|| v.get("mode").cloned()),
           "people": players.len() - bots, "bots": bots,
           "players": players.iter().map(|p| json!({"name": p["name"], "bot": p["bot"].as_i64() == Some(1), "team": p["team"], "frags": p["frags"]})).collect::<Vec<_>>()})
}

/// People (not bots) on a game right now; None when that can't be told (the game didn't
/// answer, or answered without a player list), which callers treat as "maybe someone".
async fn people_on(ctx: &Ctx, target: Option<&str>) -> Option<usize> {
    match call(ctx, Method::GET, "/api/state", target, None).await {
        Ok(v) if v["online"] == true => v["players"].as_array().map(|l| l.iter().filter(|p| p["bot"].as_i64() != Some(1)).count()),
        _ => None,
    }
}

/// A server id that is one of the player's own on this computer.
async fn own_server(ctx: &Ctx, a: &Value) -> Result<String, Fail> {
    let id = id_arg(a, "server")?;
    let v = get(ctx, "/api/servers").await?;
    let ok = v["servers"].as_array().into_iter().flatten().any(|e| e["id"] == id.as_str() && e["host"].is_null() && e["shared"] != true);
    if ok {
        Ok(id)
    } else {
        Err(err("That isn't one of your servers on this computer (servers_list has them)."))
    }
}

/// "game" (the hosted game: no target) or one of the player's own servers.
async fn target_arg(ctx: &Ctx, a: &Value) -> Result<Option<String>, Fail> {
    match s(a, "server") {
        Some("game") | Some("local") => Ok(None),
        _ => own_server(ctx, a).await.map(Some),
    }
}

// ---- resources ------------------------------------------------------------------------

pub enum ResourceError {
    Unknown,
    Failed(String),
}

pub fn resource_list() -> Vec<Value> {
    vec![
        json!({"uri": "loner://maps", "name": "maps", "title": "Maps", "description": "The game's maps with nav, knowledge and missing files.", "mimeType": "application/json"}),
        json!({"uri": "loner://editor/doc", "name": "editor-doc", "title": "Map editor: open map", "description": "The map open in the map editor.", "mimeType": "application/json"}),
        json!({"uri": "loner://health/reports", "name": "health-reports", "title": "Health check reports", "description": "Earlier map health checks.", "mimeType": "application/json"}),
    ]
}

pub fn resource_templates() -> Vec<Value> {
    vec![
        json!({"uriTemplate": "loner://map/{map}/info", "name": "map-info", "title": "Map info", "description": "One map's facts.", "mimeType": "application/json"}),
        json!({"uriTemplate": "loner://map/{map}/doctor", "name": "map-doctor", "title": "Map doctor report", "description": "The map doctor's report for a map.", "mimeType": "application/json"}),
        json!({"uriTemplate": "loner://map/{map}/nav", "name": "map-nav", "title": "Nav summary", "description": "The map's bot navigation in brief.", "mimeType": "application/json"}),
        json!({"uriTemplate": "loner://health/{id}", "name": "health-report", "title": "Health check report", "description": "One health check report.", "mimeType": "application/json"}),
    ]
}

/// Reads a resource as text (read-only, logged like a tool).
pub async fn read_resource(ctx: &Ctx, uri: &str) -> Result<String, ResourceError> {
    let parts: Vec<&str> = uri.strip_prefix("loner://").ok_or(ResourceError::Unknown)?.split('/').collect();
    let (tool, args) = match parts.as_slice() {
        ["maps"] => ("maps_list", json!({})),
        ["editor", "doc"] => ("editor_state", json!({})),
        ["health", "reports"] => ("health_reports", json!({})),
        ["health", id] => ("health_report", json!({"id": id})),
        ["map", m, "info"] => ("map_info", json!({"map": m})),
        ["map", m, "doctor"] => ("map_doctor", json!({"map": m})),
        ["map", m, "nav"] => ("map_nav_summary", json!({"map": m})),
        _ => return Err(ResourceError::Unknown),
    };
    let r = run(ctx, tool, &args).await;
    let (outcome, note) = match &r {
        Ok(_) => (Outcome::Done, String::new()),
        Err(Fail::Refused(m)) | Err(Fail::Error(m)) => (Outcome::Failed, m.chars().take(160).collect()),
    };
    ctx.log.add("read", uri, outcome, &note);
    match r {
        Ok(v) => Ok(trimmed(v).to_string()),
        Err(Fail::Refused(m)) | Err(Fail::Error(m)) => Err(ResourceError::Failed(m)),
    }
}

// ---- prompts --------------------------------------------------------------------------

pub fn prompt_list() -> Vec<Value> {
    vec![
        json!({"name": "make_small_arena", "title": "Make a small arena",
               "description": "Builds a small enclosed arena in the map editor with spawns for both teams, checks it and exports a .map.",
               "arguments": [{"name": "size", "description": "Floor size in units (default 1024)", "required": false},
                             {"name": "theme", "description": "A look to aim for, e.g. desert, office", "required": false}]}),
        json!({"name": "check_map_for_bots", "title": "Check this map for bots",
               "description": "Looks at a map's nav, knowledge and files, lists what stops bots playing it well, and suggests fixes.",
               "arguments": [{"name": "map", "description": "Map name, e.g. de_dust2", "required": true}]}),
    ]
}

pub fn get_prompt(name: &str, args: &Map<String, Value>) -> Option<Value> {
    let arg = |k: &str| args.get(k).and_then(Value::as_str).filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string());
    let (description, text) = match name {
        "make_small_arena" => {
            let size = arg("size").and_then(|s| s.parse::<u32>().ok()).filter(|s| (256..=4096).contains(s)).unwrap_or(1024);
            let theme = arg("theme").map(|t| format!(" Aim for a {t} look when you pick textures (editor_textures lists them).")).unwrap_or_default();
            ("Make a small arena", format!(
                "Using Loner-Strike's map editor tools, make a small arena about {size} by {size} units.{theme}\n\
                 1. editor_new for a fresh map (it already has a sky room, 2 CT and 2 T spawns and a sun), or editor_state to see what's open.\n\
                 2. Add a floor, four walls about 256 units high and a few boxes for cover with editor_add_box (GoldSrc units, Z up; keep brushes on a 16-unit grid).\n\
                 3. Put the spawns at opposite ends with editor_command transform or editor_add_entity (info_player_start for CT, info_player_deathmatch for T), at least 64 units apart and 1 unit above the floor.\n\
                 4. Run editor_checks and fix every problem it lists.\n\
                 5. Tell me what you made, then editor_export_map so I can pick where to save the .map."
            ))
        }
        "check_map_for_bots" => {
            let map = arg("map")?;
            if !crate::maps::safe_arg(&map) {
                return None;
            }
            ("Check a map for bots", format!(
                "Check the map {map} for bots with Loner-Strike's tools.\n\
                 1. map_doctor {map}: missing files, the .res list, whether it has a nav and if the nav fits the map.\n\
                 2. map_nav_summary and nav_problems {map}: unreachable areas, traps, broken links, ladders with no end, unnamed places.\n\
                 3. map_knowledge {map}: has the bot brain learned the map, and does its knowledge fit the nav.\n\
                 4. health_reports: if there's a health check report for {map}, read it with health_report (falls, stuck spots, deaths to the map).\n\
                 Then give me a short list, worst first, of what stops bots playing {map} well and what to do about each. \
                 Don't change anything yet; ask me before using any tool that changes files."
            ))
        }
        _ => return None,
    };
    Some(json!({"description": description, "messages": [{"role": "user", "content": {"type": "text", "text": text}}]}))
}
