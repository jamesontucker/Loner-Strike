//! The game mode components in the editor (`/api/tools/editor/components*`): the library for
//! the drag-in panel (agent W1's `editor/components.json`; the forms come from
//! `lonerstrike.fgd` through `/fgd`), and map packs: the map's `ls_*` entities written as a
//! component file (`classname x y z key=value ...`, docs/COMPONENTS.md) so a mode can be tried
//! on any map, stock ones too, without compiling; and read back in to edit.
//!
//! Packs go to the player's own folder, `addons/amxmodx/configs/components/<map>[.<mode>].ini`
//! (loaded at map start like the shipped `configs/gamemodes/components/` ones, and never
//! replaced by an update); a file that's there is kept as `.bak` first.

use super::{blocking, forbidden, Ed, NO_DOC};
use crate::editor::doc::{Entity, MapDocument};
use crate::editor::history::{History, Tx};
use crate::editor::mapfile::fmt_num;
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const LIBRARY: &str = include_str!("../../editor/components.json");
const MAX_PACK: u64 = 4 * 1024 * 1024;

/// The library, parsed once.
pub fn library() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| serde_json::from_str(LIBRARY).unwrap_or_else(|_| json!({"groups": [], "components": []})))
}

/// One value for a pack line: plain, or in quotes when it has spaces (quotes inside dropped).
fn pack_value(v: &str) -> String {
    let v: String = v.chars().filter(|c| *c != '"' && !c.is_control()).collect();
    if v.is_empty() || v.contains(char::is_whitespace) {
        format!("\"{v}\"")
    } else {
        v
    }
}

fn pack_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 64 && k.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// The map's point components as a pack: (text, how many, what was left out and why).
pub fn write_pack(doc: &MapDocument, map: &str) -> (String, usize, Vec<String>) {
    let owned: std::collections::HashSet<_> = doc.brushes.values().filter_map(|b| b.entity).collect();
    let mut out = format!("; Components for {map}, written by the Loner-Strike map editor.\n; One per line: classname x y z key=value ... (docs/COMPONENTS.md)\n");
    let mut n = 0;
    let mut skipped = Vec::new();
    for (id, e) in &doc.entities {
        let class = e.classname();
        if !class.starts_with("ls_") {
            continue;
        }
        if owned.contains(id) {
            skipped.push(format!("{class} is made of brushes, so it can only go in the compiled map"));
            continue;
        }
        let Some(o) = e.origin() else {
            skipped.push(format!("{class} has no position"));
            continue;
        };
        let mut line = format!("{class} {} {} {}", fmt_num(o.x.round()), fmt_num(o.y.round()), fmt_num(o.z.round()));
        for (k, v) in &e.keys {
            if k == "classname" || k == "origin" || !pack_key(k) {
                continue;
            }
            line.push_str(&format!(" {k}={}", pack_value(v)));
        }
        out.push_str(&line);
        out.push('\n');
        n += 1;
    }
    (out, n, skipped)
}

/// Splits a pack line into words, keeping "quoted values" whole.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A pack's components as entities (bad lines are listed, not fatal).
pub fn read_pack(text: &str) -> (Vec<Entity>, Vec<String>) {
    let mut ents = Vec::new();
    let mut bad = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let w = words(line);
        let pos: Option<Vec<f64>> = w.get(1..4).map(|p| p.iter().filter_map(|x| x.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 32768.0)).collect());
        let class = w.first().cloned().unwrap_or_default();
        match pos {
            Some(p) if p.len() == 3 && class.starts_with("ls_") && pack_key(&class) => {
                let mut e = Entity::new(&class);
                e.set("origin", &format!("{} {} {}", fmt_num(p[0]), fmt_num(p[1]), fmt_num(p[2])));
                for kv in &w[4..] {
                    if let Some((k, v)) = kv.split_once('=') {
                        if pack_key(k) && k != "classname" && k != "origin" && v.len() <= 1024 {
                            e.set(k, v);
                        }
                    }
                }
                ents.push(e);
            }
            _ => bad.push(format!("line {}: {}", i + 1, line.chars().take(60).collect::<String>())),
        }
        if ents.len() >= 2000 {
            break;
        }
    }
    (ents, bad)
}

/// Adds a pack's components to the map as one undo step.
pub fn cmd_add_pack(doc: &mut MapDocument, h: &mut History, ents: Vec<Entity>) -> usize {
    let n = ents.len();
    let mut tx = Tx::new(doc, "Load components");
    for e in ents {
        tx.add_entity(e);
    }
    tx.commit(h);
    n
}

fn valid_mode(m: &str) -> bool {
    !m.is_empty() && m.len() <= 32 && m.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// Where a pack lives in a game folder.
pub fn pack_path(game_dir: &Path, map: &str, mode: Option<&str>) -> Result<PathBuf, String> {
    if !crate::maps::safe_arg(map) || map.len() > 31 {
        return Err("That isn't a map name.".into());
    }
    let file = match mode.filter(|m| !m.is_empty()) {
        Some(m) if valid_mode(m) => format!("{map}.{m}.ini"),
        Some(_) => return Err("That isn't a mode id.".into()),
        None => format!("{map}.ini"),
    };
    Ok(game_dir.join("addons").join("amxmodx").join("configs").join("components").join(file))
}

// ---- routes ------------------------------------------------------------------------------

async fn api_library() -> Response {
    ([(header::CACHE_CONTROL, "max-age=3600")], Json(library().clone())).into_response()
}

#[derive(Deserialize)]
struct PackReq {
    map: String,
    #[serde(default)]
    mode: Option<String>,
}

async fn api_save_pack(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<PackReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game().map(|g| g.dir.clone());
    blocking(move || {
        let game = game.ok_or("Set up the game first: component files go in its folder.")?;
        let path = pack_path(&game, req.map.trim(), req.mode.as_deref().map(str::trim))?;
        let (text, n, skipped) = {
            let g = ed.lock();
            let s = g.as_ref().ok_or(NO_DOC)?;
            write_pack(&s.doc, req.map.trim())
        };
        if n == 0 {
            return Err("There are no components (ls_ entities) in the map to save. Drag some in from Components.".into());
        }
        let dir = path.parent().ok_or("That folder can't be used.")?;
        std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make the components folder ({e})."))?;
        let rel = path.strip_prefix(&game).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| path.to_string_lossy().to_string());
        // P5 (QA round 3): the same as the file there: nothing to save, no History entry, no .bak
        if std::fs::read(&path).is_ok_and(|old| old == text.as_bytes()) {
            return Ok(json!({"ok": true, "count": n, "file": rel, "skipped": skipped, "unchanged": true}));
        }
        if path.is_file() {
            let _ = std::fs::copy(&path, path.with_extension("ini.bak"));
        }
        let tmp = path.with_extension("ini.part");
        crate::safety::before("Map editor", "Saved the map's components", &path); // backups (P7, safety.rs)
        std::fs::write(&tmp, text.as_bytes()).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| format!("Couldn't write the component file ({e})."))?;
        Ok(json!({"ok": true, "count": n, "file": rel, "skipped": skipped}))
    })
    .await
}

async fn api_load_pack(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<PackReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game().map(|g| g.dir.clone());
    blocking(move || {
        let game = game.ok_or("Set up the game first.")?;
        let mine = pack_path(&game, req.map.trim(), req.mode.as_deref().map(str::trim))?;
        // the player's own file, else the shipped one
        let shipped = mine.parent().and_then(Path::parent).map(|c| c.join("gamemodes").join("components").join(mine.file_name().unwrap_or_default()));
        let path = [Some(mine.clone()), shipped].into_iter().flatten().find(|p| p.is_file()).ok_or("There's no component file for that map yet.")?;
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(u64::MAX) > MAX_PACK {
            return Err("That component file is too big.".into());
        }
        let text = String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| format!("Couldn't read it ({e})."))?).to_string();
        let (ents, bad) = read_pack(&text);
        if ents.is_empty() {
            return Err("That file has no components the editor can read.".into());
        }
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        let n = cmd_add_pack(&mut s.doc, &mut s.history, ents);
        s.track();
        let mut v = s.state();
        v["count"] = json!(n);
        v["bad"] = json!(bad);
        v["file"] = json!(path.strip_prefix(&game).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default());
        Ok(v)
    })
    .await
}

#[derive(Deserialize)]
struct PreviewQ {
    map: String,
}

/// What the pack for the open map would hold (for the panel).
async fn api_preview(Extension(ed): Extension<Ed>, Query(q): Query<PreviewQ>) -> Response {
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let (text, n, skipped) = write_pack(&s.doc, &q.map);
        Ok(json!({"text": text, "count": n, "skipped": skipped}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/components", get(api_library))
        .route("/api/tools/editor/components/pack", get(api_preview))
        .route("/api/tools/editor/components/save-pack", post(api_save_pack))
        .route("/api/tools/editor/components/load-pack", post(api_load_pack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_parses_and_matches_the_fgd() {
        let lib = library();
        let comps = lib["components"].as_array().unwrap();
        assert!(comps.len() >= 10);
        let fgd = crate::editor::fgd::parse(&crate::editor::fgd::shipped()).unwrap();
        for c in comps {
            let class = c["class"].as_str().unwrap();
            assert!(fgd.iter().any(|k| k.name == class), "{class} has no FGD entry");
        }
    }

    #[test]
    fn npcs_and_interactables_are_in_the_library_and_the_fgd() {
        let fgd = crate::editor::fgd::parse(&crate::editor::fgd::shipped()).unwrap();
        let class = |n: &str| fgd.iter().find(|k| k.name == n).unwrap_or_else(|| panic!("{n}"));
        let npc = class("ls_npc");
        assert_eq!(npc.kind, "point");
        for k in ["model", "name", "behavior", "targets", "unkillable", "team", "greet", "target", "targetname"] {
            assert!(npc.keys.iter().any(|x| x.name == k), "ls_npc {k}");
        }
        // what the editor's I/O panel reads: the target key sets things off
        assert!(npc.keys.iter().any(|x| x.name == "target" && x.kind == "target_destination"));
        assert_eq!(npc.keys.iter().find(|x| x.name == "unkillable").unwrap().default, "1");
        let b = npc.keys.iter().find(|x| x.name == "behavior").unwrap();
        assert_eq!(b.choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["stand", "wander", "patrol", "follow"]);
        for n in ["ls_interactable", "ls_interactable_brush"] {
            let c = class(n);
            for k in ["prompt", "range", "cooldown", "once", "hide", "team", "target", "bots"] {
                assert!(c.keys.iter().any(|x| x.name == k), "{n} {k}");
            }
        }
        assert_eq!(class("ls_interactable_brush").kind, "solid");
        // no key of theirs reads as a creature type (shared creatures look for creatures.ini in the help)
        for n in ["ls_npc", "ls_interactable", "ls_interactable_brush"] {
            assert!(class(n).keys.iter().all(|k| !k.help.contains("creatures.ini")), "{n}");
        }
        let lib = library();
        for n in ["ls_npc", "ls_interactable", "ls_interactable_brush"] {
            assert!(lib["components"].as_array().unwrap().iter().any(|c| c["class"] == n && c["group"] == "People"), "{n}");
        }
    }

    /// gm_components (the plugin, read as source): an NPC's stand-in model is loaded with every
    /// map, and the log names the model that was asked for, not the stand-in twice (Y's check).
    #[test]
    /// FX's sim: Spawn NPC and Spawn Interactable refused at spots right next to a bot. Only the
    /// world blocks a spot now (players, bots and creatures never: the hull trace ignores them),
    /// a blocked NPC goes to the nearest free spot within 128 by default, the keys can ask for
    /// near:N or fail, and how far it moved is ls_spawn_moved().
    fn npc_and_interactable_spawns_take_the_nearest_free_spot() {
        let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/scripting/gm_components.sma")).unwrap();
        let free = &src[src.find("stock bool:spawn_spot_free(").unwrap()..];
        let free = &free[..free.find("\n}\n").unwrap()];
        assert!(free.contains("IGNORE_MONSTERS, HULL_HUMAN") && !free.contains("DONT_IGNORE_MONSTERS"), "people never block a spot");
        let pol = &src[src.find("stock Float:spawn_policy(").unwrap()..];
        let pol = &pol[..pol.find("\n}\n").unwrap()];
        assert!(pol.contains("\"fail\"") && pol.contains("\"near\"") && pol.contains("\"here\""));
        for native in ["public n_npc_spawn(", "public n_interactable_spawn("] {
            let body = &src[src.find(native).unwrap()..];
            let body = &body[..body.find("\n}\n").unwrap()];
            assert!(body.contains("spawn_place(o, extra, ") && body.contains("comp_make(") && body.contains(", at, keys, SRC_RUNTIME)"), "{native} places through the check");
        }
        assert!(src.contains("register_native(\"ls_spawn_moved\", \"n_spawn_moved\")") && src.contains("register_srvcmd(\"ls_npc_spawn_test\""));
        let inc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/scripting/include/ls_components.inc")).unwrap();
        assert!(inc.contains("native Float:ls_spawn_moved();"));
    }

    #[test]
    fn npc_model_fallback_is_loaded_and_named_right() {
        let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/scripting/gm_components.sma")).unwrap();
        let block = &src[src.find("/* ==== NPCs and interactables (T1fix) ==== */").unwrap()..];
        let pre = &block[block.find("stock npc_precache()").unwrap()..];
        let pre = &pre[..pre.find("\n}\n").unwrap()];
        assert!(pre.contains("precache_model_once(NPC_FALLBACK)"), "the stand-in loads with the map");
        let init = &block[block.find("stock npc_comp_init(s)").unwrap()..];
        let init = &init[..init.find("\n}\n").unwrap()];
        assert!(!init.contains("copy(m, charsmax(m), NPC_FALLBACK)") && !init.contains("copy(m, charsmax(m), \"models/player/gign/gign.mdl\")"), "the asked-for name isn't overwritten before the log");
        assert!(init.contains("isn't loaded; showing %s instead.\", g_label[s][0] ? g_label[s] : \"(no name)\", m[0] ? m : \"(none)\", NPC_FALLBACK)"));
        // killable ones are solid (bullets hit them), unkillable ones walk-through; bots shoot one only by opt-in
        assert!(block.contains("pev(e, pev_solid) == SOLID_NOT && pev(e, pev_takedamage) != DAMAGE_NO"));
        assert!(block.contains("ki(s, \"bots_attack\", 0)") && block.contains("\"ls_npc_target\" : \"ls_npc\""));
    }

    #[test]
    fn packs_round_trip() {
        let mut d = MapDocument::new();
        let mut cp = Entity::new("ls_capture_point");
        cp.set("origin", "1152 2464 120.4");
        cp.set("name", "A");
        cp.set("radius", "200");
        cp.set("stages", "2400:30:30:2 1200:30:25:5");
        d.insert_entity(cp);
        let mut light = Entity::new("light");
        light.set("origin", "0 0 0");
        d.insert_entity(light);
        let bar = d.insert_entity(Entity::new("ls_barricade"));
        let mut b = crate::editor::ops::make_box(crate::editor::math::v3(0.0, 0.0, 0.0), crate::editor::math::v3(64.0, 16.0, 96.0), "CRATE01");
        b.entity = Some(bar);
        d.insert_brush(b);
        let (text, n, skipped) = write_pack(&d, "de_dust2");
        assert_eq!(n, 1);
        assert_eq!(skipped.len(), 1, "the barricade is brushes");
        assert!(text.contains("ls_capture_point 1152 2464 120 name=A radius=200 stages=\"2400:30:30:2 1200:30:25:5\""), "{text}");
        let (ents, bad) = read_pack(&format!("{text}\nnot a line\nls_bad 1 2\n"));
        assert_eq!(ents.len(), 1);
        assert_eq!(bad.len(), 2);
        assert_eq!(ents[0].get("stages"), Some("2400:30:30:2 1200:30:25:5"));
        assert_eq!(ents[0].get("origin"), Some("1152 2464 120"));
        let mut h = History::default();
        let mut d2 = MapDocument::new();
        assert_eq!(cmd_add_pack(&mut d2, &mut h, ents), 1);
        assert_eq!(h.undo(&mut d2).as_deref(), Some("Load components"));
        assert!(d2.entities.is_empty());
        // where packs go
        let g = Path::new("/g/cstrike");
        assert_eq!(pack_path(g, "de_dust2", None).unwrap(), g.join("addons/amxmodx/configs/components/de_dust2.ini"));
        assert_eq!(pack_path(g, "de_dust2", Some("br")).unwrap(), g.join("addons/amxmodx/configs/components/de_dust2.br.ini"));
        assert!(pack_path(g, "../x", None).is_err() && pack_path(g, "de_dust2", Some("a/b")).is_err());
    }
}
