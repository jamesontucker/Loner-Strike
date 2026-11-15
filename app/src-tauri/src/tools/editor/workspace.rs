//! The map editor's project viewer, undo history and "many things at once" commands (agent P5).
//! A child of tools/editor.rs so it shares the open session; its own routes:
//!   GET  /api/tools/editor/project            the project's files (map, compile output, WADs,
//!                                             sounds/models/sprites it uses, sky, prefabs)
//!   POST /api/tools/editor/project/action     {action: open|reveal|rename|delete, path, name?}
//!   GET  /api/tools/editor/history            undo and redo labels
//!   POST /api/tools/editor/history/jump       {steps}: >0 undo that many, <0 redo
//!   POST /api/tools/editor/cmd2               duplicate (new target names), find, replace,
//!                                             similar, touching, align, distribute
//! Paths from the page are only acted on when they're one of the listed files.

use super::{blocking, forbidden, load_file, remember, stem, wad_dirs, Ed, Session, NOT_UNDERSTOOD, NO_DOC};
use crate::editor::bulk::{self, Edge, FindSpec, Similar};
use crate::editor::doc::{BrushId, EntityId, GroupId, MapDocument};
use crate::editor::math::Vec3;
use crate::server::Shared;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
#[cfg(test)]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ---- many-things commands ------------------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Cmd2 {
    Duplicate {
        #[serde(default)]
        brushes: Vec<u64>,
        #[serde(default)]
        entities: Vec<u64>,
        #[serde(default)]
        offset: [f64; 3],
        #[serde(default = "yes")]
        rename: bool,
        /// Shift-drag copy: its undo step is "Copy" and the move after it (same key) joins it
        #[serde(default)]
        merge: Option<String>,
    },
    Find {
        spec: FindSpec,
    },
    Replace {
        spec: FindSpec,
    },
    Similar {
        #[serde(default)]
        brushes: Vec<u64>,
        #[serde(default)]
        entities: Vec<u64>,
        similar: Similar,
    },
    Touching {
        #[serde(default)]
        brushes: Vec<u64>,
        #[serde(default)]
        entities: Vec<u64>,
        #[serde(default)]
        inside: bool,
    },
    Align {
        #[serde(default)]
        brushes: Vec<u64>,
        #[serde(default)]
        entities: Vec<u64>,
        axis: usize,
        edge: Edge,
        #[serde(default = "yes")]
        lock: bool,
    },
    Distribute {
        #[serde(default)]
        brushes: Vec<u64>,
        #[serde(default)]
        entities: Vec<u64>,
        axis: usize,
        #[serde(default = "yes")]
        lock: bool,
    },
    RenameGroup {
        group: u64,
        name: String,
    },
    /// one key on many entities (empty value: remove it)
    SetKeys {
        entities: Vec<u64>,
        key: String,
        #[serde(default)]
        value: String,
    },
    /// into a group (null: out of every group); groups can go inside groups too
    MoveToGroup {
        group: Option<u64>,
        #[serde(default)]
        brushes: Vec<u64>,
        #[serde(default)]
        entities: Vec<u64>,
        #[serde(default)]
        groups: Vec<u64>,
    },
}

fn yes() -> bool {
    true
}

fn bids(v: &[u64]) -> Vec<BrushId> {
    v.iter().map(|x| BrushId(*x)).collect()
}
fn eids(v: &[u64]) -> Vec<EntityId> {
    v.iter().map(|x| EntityId(*x)).collect()
}
fn ids_json(b: &[BrushId], e: &[EntityId]) -> Value {
    json!({"brushes": b.iter().map(|x| x.0).collect::<Vec<_>>(), "entities": e.iter().map(|x| x.0).collect::<Vec<_>>()})
}

fn run2(s: &mut Session, cmd: Cmd2) -> Result<Value, String> {
    s.drag = None;
    let (d, h) = (&mut s.doc, &mut s.history);
    Ok(match cmd {
        Cmd2::Duplicate { brushes, entities, offset, rename, merge } => {
            let off = Vec3 { x: offset[0], y: offset[1], z: offset[2] };
            let merge = merge.filter(|k| !k.is_empty() && k.len() <= 64);
            let label = if merge.is_some() { "Copy" } else { "Duplicate" };
            let out = bulk::cmd_duplicate_as(d, h, &bids(&brushes), &eids(&entities), off, rename, label, merge.as_deref())?;
            json!({"created": ids_json(&out.brushes, &out.entities)})
        }
        Cmd2::Find { spec } => {
            let f = bulk::find(d, &spec);
            json!({"found": f})
        }
        Cmd2::Replace { spec } => {
            let n = bulk::cmd_replace(d, h, &spec)?;
            json!({"replaced": n, "note": if n == 0 { "Nothing matched.".to_string() } else { format!("Replaced {n} place{}.", if n == 1 { "" } else { "s" }) }})
        }
        Cmd2::Similar { brushes, entities, similar } => {
            let (b, e) = bulk::similar(d, &bids(&brushes), &eids(&entities), &similar);
            json!({"select": ids_json(&b, &e)})
        }
        Cmd2::Touching { brushes, entities, inside } => {
            let (b, e) = bulk::touching(d, &bids(&brushes), &eids(&entities), inside);
            json!({"select": ids_json(&b, &e)})
        }
        Cmd2::Align { brushes, entities, axis, edge, lock } => {
            let n = bulk::cmd_align_objects(d, h, &bids(&brushes), &eids(&entities), axis, edge, lock)?;
            json!({"moved": n})
        }
        Cmd2::Distribute { brushes, entities, axis, lock } => {
            let n = bulk::cmd_distribute(d, h, &bids(&brushes), &eids(&entities), axis, lock)?;
            json!({"moved": n})
        }
        Cmd2::SetKeys { entities, key, value } => {
            let n = bulk::cmd_set_key_many(d, h, &eids(&entities), &key, &value)?;
            json!({"moved": n})
        }
        Cmd2::RenameGroup { group, name } => {
            bulk::cmd_rename_group(d, h, GroupId(group), &name)?;
            json!({})
        }
        Cmd2::MoveToGroup { group, brushes, entities, groups } => {
            let n = bulk::cmd_move_to_group(d, h, group.map(GroupId), &bids(&brushes), &eids(&entities), &groups.iter().map(|g| GroupId(*g)).collect::<Vec<_>>())?;
            json!({"moved": n})
        }
    })
}

/// Like tools/editor.rs `apply`: run, then say what changed and where things are.
fn apply2(s: &mut Session, cmd: Cmd2, dirs: &[PathBuf]) -> Value {
    // P5 (perf): no whole copies per command; the change notes say what changed
    let before = (s.doc.revision, s.doc.groups.clone(), s.doc.world.clone());
    match run2(s, cmd) {
        Ok(mut v) => {
            s.learn_textures(dirs);
            let ents = !s.doc.touched.entities.is_empty();
            s.track();
            v["changed"] = json!(s.doc.revision != before.0);
            v["docChanged"] = json!(ents || s.doc.groups != before.1 || s.doc.world != before.2);
            v["state"] = s.state();
            if s.doc.revision != before.0 {
                v["changes"] = super::changes_json(s, before.0).unwrap_or(Value::Null);
            }
            v
        }
        Err(e) => {
            s.track();
            json!({"error": e, "state": s.state()})
        }
    }
}

async fn api_cmd2(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let Ok(cmd) = serde_json::from_value::<Cmd2>(body) else { return super::bad(NOT_UNDERSTOOD) };
    let dirs = wad_dirs(&st);
    blocking(move || {
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        Ok(apply2(s, cmd, &dirs))
    })
    .await
}

// ---- undo history ---------------------------------------------------------------------

async fn api_history(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let (undo, redo) = s.history.labels();
        Ok(json!({"undo": undo, "redo": redo, "dirty": s.history.is_dirty()}))
    })
    .await
}

#[derive(Deserialize)]
struct Jump {
    steps: i64,
}

async fn api_history_jump(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<Jump>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        s.drag = None;
        let from = s.doc.revision;
        let mut n = 0;
        for _ in 0..req.steps.unsigned_abs().min(1000) {
            let r = if req.steps > 0 { s.history.undo(&mut s.doc) } else { s.history.redo(&mut s.doc) };
            if r.is_none() {
                break;
            }
            n += 1;
        }
        s.track();
        let changes = if n > 0 { super::changes_json(s, from).unwrap_or(Value::Null) } else { Value::Null };
        Ok(json!({"changed": n > 0, "docChanged": n > 0, "steps": n, "state": s.state(), "changes": changes}))
    })
    .await
}

// ---- the project's files --------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// the open project file
    Project,
    /// in the project's folder: rename and delete allowed
    Mine,
    /// compile output / installed map files: delete allowed
    Output,
    /// shared game files (WADs, sounds...): look only
    Game,
}

#[derive(Clone, Debug)]
struct FileRow {
    group: &'static str,
    label: String,
    path: Option<PathBuf>,
    kind: Kind,
    /// say so when it isn't there
    need: bool,
    note: String,
}

fn row(group: &'static str, label: impl Into<String>, path: Option<PathBuf>, kind: Kind, need: bool, note: impl Into<String>) -> FileRow {
    FileRow { group, label: label.into(), path, kind, need, note: note.into() }
}

/// Finds `rel` (forward slashes) in the game folders, first match wins.
fn find_in(dirs: &[PathBuf], rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/");
    if rel.contains("..") || rel.starts_with('/') {
        return None;
    }
    dirs.iter().map(|d| d.join(&rel)).find(|p| p.is_file())
}

/// Sound, model and sprite files the map's entities name (relative to the game folder).
/// The editor's asset checks use assets.rs now; kept for its test.
#[cfg(test)]
pub fn referenced_files(doc: &MapDocument) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for e in doc.entities.values() {
        let class = e.classname();
        for (k, v) in &e.keys {
            let v = v.trim().trim_start_matches('*').replace('\\', "/");
            if v.is_empty() || v.starts_with('!') {
                continue;
            }
            let low = v.to_ascii_lowercase();
            let is_sound_key = k.starts_with("noise") || k == "message" && class == "ambient_generic" || k == "sound";
            if is_sound_key && (low.ends_with(".wav") || low.ends_with(".mp3")) {
                out.insert(format!("sound/{v}"));
            } else if (k == "model" || k == "sprite" || k == "texture") && (low.ends_with(".mdl") || low.ends_with(".spr")) {
                out.insert(v);
            }
        }
    }
    out
}

fn collect(st: &Shared, ed: &Ed) -> Result<(Vec<FileRow>, Value), String> {
    let g = ed.lock();
    let s = g.as_ref().ok_or(NO_DOC)?;
    let game = st.local_game();
    let dirs = wad_dirs(st);
    let map = crate::tools::compile::map_name(&s.name);
    let mut rows = Vec::new();

    // the project and what sits beside it
    if let Some(p) = &s.project {
        rows.push(row("Project", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), Some(p.clone()), Kind::Project, true, "This map's project file"));
        if let Some(dir) = p.parent() {
            let base = stem(p);
            for ext in ["map", "bsp", "log", "err", "pts", "lin", "prt", "rmf", "jmf", "wad", "res", "nav", "lsv", "txt", "bmp", "tga"] {
                let f = dir.join(format!("{base}.{ext}"));
                if f.is_file() {
                    rows.push(row("Project", format!("{base}.{ext}"), Some(f), Kind::Mine, false, ""));
                }
            }
            // custom WADs kept with the project
            if let Ok(rd) = std::fs::read_dir(dir) {
                let mut wads: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wad")) && p.file_stem().is_some_and(|n| n.to_string_lossy() != base)).collect();
                wads.sort();
                for w in wads.into_iter().take(50) {
                    rows.push(row("Project", w.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), Some(w), Kind::Mine, false, "A texture file in the project's folder"));
                }
            }
        }
    } else {
        rows.push(row("Project", format!("{} (not saved yet)", s.name), None, Kind::Project, false, "Save the map to make a project file"));
    }
    // backups made on save
    let backups = crate::localdata::join(&ed.dir, "backups");
    if let Ok(rd) = std::fs::read_dir(&backups) {
        // P5: this project's by its uid (also after a rename), and old ones by name; newest first
        let mut b: Vec<(u64, PathBuf)> = rd.flatten().map(|e| e.path()).filter_map(|p| super::backup_of(&p, &s.name, s.doc.editor.uid.as_deref()).map(|t| (t, p))).collect();
        b.sort();
        for (_, p) in b.into_iter().rev().take(10) {
            rows.push(row("Backups", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), Some(p), Kind::Output, false, "A copy kept when you saved over it"));
        }
    }

    // compile output (the compiler's work folder) and what's installed in the game
    let mut work = Vec::new();
    if let Some(gm) = &game {
        let hl = gm.hl.to_string_lossy().to_string();
        if let Some(i) = hl.find("/drive_c/") {
            work.push(PathBuf::from(&hl[..i + 8]).join("Loner-Strike").join("Compiler").join("work").join(&map));
        }
    }
    work.push(crate::localdata::dir("tools/compile").join("work").join(&map));
    for w in work.iter().filter(|w| w.is_dir()) {
        for ext in ["bsp", "log", "err", "pts", "lin", "prt", "map"] {
            let f = w.join(format!("{map}.{ext}"));
            if f.is_file() {
                let note = match ext {
                    "bsp" => "The compiled map",
                    "log" => "The compiler's log",
                    "err" => "The compiler's errors",
                    "pts" | "lin" => "The leak line (the map isn't sealed)",
                    "prt" => "Portals (for vis)",
                    _ => "The .map sent to the compiler",
                };
                rows.push(row("Compiled", format!("{map}.{ext}"), Some(f), Kind::Output, false, note));
            }
        }
    }
    if let Some(gm) = &game {
        let maps = gm.dir.join("maps");
        for (ext, note, need) in [("bsp", "Installed in the game (Playtest)", false), ("nav", "The bots' paths", false), ("lsv", "What bots know about the map", false), ("res", "Files players download with the map", false)] {
            let f = maps.join(format!("{map}.{ext}"));
            if f.is_file() || need {
                rows.push(row("In the game", format!("maps/{map}.{ext}"), Some(f), Kind::Output, need, note));
            }
        }
        for ext in ["txt", "bmp", "tga"] {
            let f = gm.dir.join("overviews").join(format!("{map}.{ext}"));
            if f.is_file() {
                rows.push(row("In the game", format!("overviews/{map}.{ext}"), Some(f), Kind::Output, false, "The spectator overview (radar)"));
            }
        }
    }

    // texture files the map names
    for w in s.doc.world.wad_names() {
        let found = dirs.iter().map(|d| d.join(&w)).find(|p| p.is_file()).or_else(|| s.project.as_ref().and_then(|p| p.parent()).map(|d| d.join(&w)).filter(|p| p.is_file()));
        let stock = ["halflife.wad", "cstrike.wad", "decals.wad", "liquids.wad", "xeno.wad", "zhlt.wad", "sdhlt.wad"].contains(&w.to_ascii_lowercase().as_str());
        rows.push(row("Texture files", w.clone(), found, Kind::Game, true, if stock { "Comes with the game" } else { "Packed into the map when it's compiled" }));
    }
    // sky
    if let Some(sky) = s.doc.world.get("skyname").filter(|x| !x.trim().is_empty()) {
        let sides = ["up", "dn", "lf", "rt", "ft", "bk"];
        let found: Vec<Option<PathBuf>> = sides.iter().map(|sd| find_in(&dirs, &format!("gfx/env/{sky}{sd}.tga")).or_else(|| find_in(&dirs, &format!("gfx/env/{sky}{sd}.bmp")))).collect();
        let missing = found.iter().filter(|f| f.is_none()).count();
        rows.push(row(
            "Sounds, models and sky",
            format!("Sky “{sky}”"),
            found.iter().flatten().next().cloned(),
            Kind::Game,
            true,
            if missing == 0 { "All six sides found".to_string() } else { format!("{missing} of 6 sides missing") },
        ));
    }
    // models, sprites and sounds: found where the game looks, beside the map, or missing
    let places = super::assets::Places::of(st, s);
    let named: Vec<_> = super::assets::named_files(&s.doc).into_iter().filter(|n| n.kind != "sky").take(500).collect();
    for a in super::assets::locate(named, &places) {
        use super::assets::Status;
        let (path, note) = match &a.status {
            Status::Game { stock: true } => (find_in(&dirs, &a.named.rel), "Comes with the game"),
            Status::Game { stock: false } => (find_in(&dirs, &a.named.rel), "In the game"),
            Status::Beside(p) => (Some(p.clone()), "Beside the map, not in the game yet: Playtest copies it there"),
            Status::Elsewhere(p) => (Some(p.clone()), "In another game's folder: Playtest copies it into this one"),
            Status::Missing => (None, "Not on this computer: the game can't load the map without it"),
        };
        rows.push(row("Sounds, models and sky", a.named.rel.clone(), path, Kind::Game, true, note));
    }
    let meta = json!({
        "name": s.name,
        "map": map,
        "project": s.project.as_ref().map(|p| p.to_string_lossy().to_string()),
        "folder": s.project.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string()),
        "game": game.is_some(),
        "assets": super::assets::assets_json(&super::assets::locate(super::assets::named_files(&s.doc), &places), &places, s.project.as_deref().and_then(Path::parent)), // P5
    });
    Ok((rows, meta))
}

fn rows_json(rows: &[FileRow]) -> Value {
    let list: Vec<Value> = rows
        .iter()
        .map(|r| {
            let md = r.path.as_ref().and_then(|p| std::fs::metadata(p).ok());
            let exists = md.as_ref().is_some_and(|m| m.is_file());
            let modified = md.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
            let ext = r.path.as_ref().and_then(|p| p.extension()).map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            json!({
                "group": r.group,
                "label": r.label,
                "path": r.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                "exists": exists,
                "missing": r.need && !exists,
                "size": md.as_ref().map(|m| m.len()),
                "modified": modified,
                "note": r.note,
                "openHere": exists && matches!(ext.as_str(), "lsmap" | "map" | "rmf" | "jmf") && r.kind != Kind::Project,
                "canRename": exists && matches!(r.kind, Kind::Project | Kind::Mine),
                "canDelete": exists && matches!(r.kind, Kind::Mine | Kind::Output),
            })
        })
        .collect();
    json!(list)
}

async fn api_project(State(st): State<Shared>, Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let (rows, meta) = collect(&st, &ed)?;
        Ok(json!({"files": rows_json(&rows), "project": meta}))
    })
    .await
}

#[derive(Deserialize)]
struct Action {
    action: String,
    path: String,
    #[serde(default)]
    name: String,
}

/// Shows a file in Finder (Explorer on Windows): for the Content Browser too.
pub fn reveal_path(p: &Path) -> Result<(), String> {
    reveal(p)
}

fn reveal(p: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(p).spawn().map(crate::sys::reap);
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn().map(crate::sys::reap);
    #[cfg(not(any(target_os = "macos", windows)))]
    let r = std::process::Command::new("xdg-open").arg(p.parent().unwrap_or(p)).spawn().map(crate::sys::reap);
    r.map_err(|_| "Couldn't show the file.".to_string())
}

fn open_with_system(p: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(p).spawn().map(crate::sys::reap);
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(p).spawn().map(crate::sys::reap);
    #[cfg(not(any(target_os = "macos", windows)))]
    let r = std::process::Command::new("xdg-open").arg(p).spawn().map(crate::sys::reap);
    r.map_err(|_| "Couldn't open the file.".to_string())
}

/// A new file name: same extension, no folders, nothing odd.
pub fn safe_rename(old: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    let ext = old.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    let base = name.strip_suffix(&format!(".{ext}")).unwrap_or(name);
    if base.is_empty() || base.len() > 64 || base.starts_with('.') || base.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
        return Err("Use a plain name: letters, digits, spaces, - and _ (no slashes).".into());
    }
    let dir = old.parent().ok_or("That file can't be renamed.")?;
    let new = if ext.is_empty() { dir.join(base) } else { dir.join(format!("{base}.{ext}")) };
    if new.exists() {
        return Err("There's already a file with that name.".into());
    }
    Ok(new)
}

async fn api_project_action(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<Action>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let dirs = wad_dirs(&st);
    let st2 = st.clone();
    blocking(move || {
        let (rows, _) = collect(&st2, &ed)?;
        let want = PathBuf::from(&req.path);
        let r = rows.iter().find(|r| r.path.as_ref() == Some(&want)).cloned().ok_or("That file isn't one of this map's files.")?;
        let path = r.path.clone().ok_or("That file isn't one of this map's files.")?;
        match req.action.as_str() {
            "reveal" => {
                if !path.exists() {
                    return Err("That file isn't there.".into());
                }
                reveal(&path)?;
                Ok(json!({"done": true}))
            }
            "open" => {
                if !path.is_file() {
                    return Err("That file isn't there.".into());
                }
                let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                if (ext == "rmf" || ext == "jmf") && r.kind != Kind::Project {
                    let im = import_other(&path, &dirs)?;
                    let mut state = ed.install_from(im.doc, stem(&path), None, Some(&path), &dirs);
                    state["notes"] = json!(im.notes);
                    return Ok(json!({"opened": true, "state": state}));
                }
                if (ext == "lsmap" || ext == "map") && r.kind != Kind::Project {
                    let (doc, project) = load_file(&path)?;
                    remember(&ed.dir, &path, project);
                    let state = ed.install_from(doc, stem(&path), project.then_some(path.clone()), Some(&path), &dirs);
                    return Ok(json!({"opened": true, "state": state}));
                }
                open_with_system(&path)?;
                Ok(json!({"done": true}))
            }
            "rename" => {
                if !matches!(r.kind, Kind::Project | Kind::Mine) {
                    return Err("Only files in the project's folder can be renamed here.".into());
                }
                let new = safe_rename(&path, &req.name)?;
                std::fs::rename(&path, &new).map_err(|_| "Couldn't rename it. Is it open somewhere else?".to_string())?;
                // renaming the open project: keep saving to the new name
                let mut g = ed.lock();
                let mut state = Value::Null;
                if let Some(s) = g.as_mut() {
                    if s.project.as_ref() == Some(&path) {
                        s.project = Some(new.clone());
                        s.name = stem(&new);
                        remember(&ed.dir, &new, true);
                        state = s.state();
                    }
                }
                Ok(json!({"renamed": new.file_name().map(|n| n.to_string_lossy().to_string()), "state": state}))
            }
            "delete" => {
                if !matches!(r.kind, Kind::Mine | Kind::Output) {
                    return Err("That file can't be deleted from here.".into());
                }
                // not gone for good: moved into the editor's "deleted" folder
                let bin = ed.dir.join("deleted");
                std::fs::create_dir_all(&bin).map_err(|_| "Couldn't delete it.".to_string())?;
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let dest = bin.join(format!("{secs}-{}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                if std::fs::rename(&path, &dest).is_err() {
                    std::fs::copy(&path, &dest).map_err(|_| "Couldn't delete it.".to_string())?;
                    std::fs::remove_file(&path).map_err(|_| "Couldn't delete it. Is it open somewhere else?".to_string())?;
                }
                Ok(json!({"deleted": true, "keptIn": bin.to_string_lossy()}))
            }
            _ => Err(NOT_UNDERSTOOD.into()),
        }
    })
    .await
}

// ---- Hammer .rmf / J.A.C.K. .jmf import --------------------------------------------------

/// A .rmf or .jmf file read into a document (by its first bytes), with its texture files
/// found (Hammer and J.A.C.K. keep the WAD list in their own settings, not in the map).
pub fn import_other(path: &Path, game_dirs: &[PathBuf]) -> Result<crate::editor::rmf::Imported, String> {
    let mut im = read_other(path)?;
    let mut dirs: Vec<PathBuf> = path.parent().map(|p| vec![p.to_path_buf()]).unwrap_or_default();
    dirs.extend(game_dirs.iter().cloned());
    let note = find_wads(&mut im.doc, &dirs);
    im.notes.push(note);
    Ok(im)
}

/// Picks texture files that hold the map's textures: the ones it names when they're here,
/// then the fewest more from `dirs` (the file's own folder first, then the game's) that cover
/// the rest. Returns a plain note of what was found and what's still missing.
pub fn find_wads(doc: &mut MapDocument, dirs: &[PathBuf]) -> String {
    use std::collections::{BTreeMap, HashSet};
    let used: HashSet<String> = doc.brushes.values().flat_map(|b| b.faces.iter().map(|f| f.texture.to_ascii_lowercase())).collect();
    // every WAD in those folders (first one with a name wins), with its texture names
    let mut wads: BTreeMap<String, (usize, PathBuf, HashSet<String>)> = BTreeMap::new();
    for (rank, d) in dirs.iter().enumerate() {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wad"))).collect();
        files.sort();
        for p in files.into_iter().take(400) {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if wads.contains_key(&name.to_ascii_lowercase()) {
                continue;
            }
            if let Ok(w) = crate::goldsrc::wad::Wad::open(&p) {
                let names: HashSet<String> = w.entries.keys().filter(|k| used.contains(*k)).cloned().collect();
                wads.insert(name.to_ascii_lowercase(), (rank, p.clone(), names));
            }
        }
    }
    let base = |w: &str| w.rsplit(['/', '\\']).next().unwrap_or(w).to_ascii_lowercase();
    let mut chosen: Vec<String> = Vec::new();
    let mut covered: HashSet<String> = HashSet::new();
    // the ones the map already names, when they're here
    for w in doc.world.wads.clone() {
        if let Some((_, p, names)) = wads.get(&base(&w)) {
            covered.extend(names.iter().cloned());
            chosen.push(p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(w));
        }
    }
    // then the fewest more (most new textures first; the file's folder before the game's)
    loop {
        let best = wads
            .iter()
            .filter(|(k, _)| !chosen.iter().any(|c| c.to_ascii_lowercase() == **k))
            .map(|(k, (rank, p, names))| (names.iter().filter(|n| !covered.contains(*n)).count(), std::cmp::Reverse(*rank), k.clone(), p.clone()))
            .filter(|x| x.0 > 0)
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let Some((_, _, k, p)) = best else { break };
        covered.extend(wads[&k].2.iter().cloned());
        chosen.push(p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(k));
    }
    let missing: Vec<&String> = used.iter().filter(|t| !covered.contains(*t)).collect();
    let named_missing: Vec<String> = doc.world.wads.iter().filter(|w| !wads.contains_key(&base(w))).cloned().collect();
    if !chosen.is_empty() {
        // the map's own list first (even the ones not here), then what was found
        let mut list: Vec<String> = doc.world.wads.clone();
        for c in &chosen {
            if !list.iter().any(|w| base(w) == c.to_ascii_lowercase()) {
                list.push(c.clone());
            }
        }
        doc.world.wads = list;
    }
    let mut note = if chosen.is_empty() { "No texture files with this map's textures were found beside it or in the game.".to_string() } else { format!("Texture files for this map: {}.", chosen.join(", ")) };
    if !named_missing.is_empty() {
        note.push_str(&format!(" Named by the map but not on this computer: {}.", named_missing.join(", ")));
    }
    if !missing.is_empty() {
        let mut m: Vec<&String> = missing.clone();
        m.sort();
        note.push_str(&format!(" {} texture{} {} in any texture file here (e.g. {}): add their WAD in Map properties.", m.len(), if m.len() == 1 { "" } else { "s" }, if m.len() == 1 { "isn't" } else { "aren't" }, m.iter().take(4).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
    }
    note
}

fn read_other(path: &Path) -> Result<crate::editor::rmf::Imported, String> {
    let len = std::fs::metadata(path).map_err(|_| "That file couldn't be opened.".to_string())?.len();
    if len > super::MAX_FILE {
        return Err("That file is too big to be a map (over 64 MB).".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "That file couldn't be read.".to_string())?;
    if bytes.starts_with(b"JHMF") {
        crate::editor::rmf::read_jmf(&bytes)
    } else if bytes.len() > 7 && &bytes[4..7] == b"RMF" {
        crate::editor::rmf::read_rmf(&bytes)
    } else {
        Err("That isn't a Hammer .rmf or J.A.C.K. .jmf file.".into())
    }
}

async fn api_import_other(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let dirs = wad_dirs(&st);
    blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        // dev servers (test builds only) answer the dialog with LS_DEV_PICK, like the other file routes
        #[cfg(test)]
        let dev = std::env::var("LS_DEV_PICK").ok().map(PathBuf::from);
        #[cfg(not(test))]
        let dev: Option<PathBuf> = None;
        let path = match dev {
            Some(p) => p,
            None => {
                let app = st.app.get().cloned().ok_or(super::INTERNAL)?;
                let Some(picked) = app.dialog().file().set_title("Import a Hammer or J.A.C.K. map").add_filter("Hammer / J.A.C.K. map", &["rmf", "jmf"]).blocking_pick_file() else {
                    return Ok(json!({"cancelled": true}));
                };
                picked.into_path().map_err(|_| "That file can't be used.".to_string())?
            }
        };
        let im = import_other(&path, &dirs)?;
        // not a project yet: Save makes one (the original file is never written to)
        let mut state = ed.install_from(im.doc, stem(&path), None, Some(&path), &dirs);
        let mut notes = im.notes;
        // the models, sprites and sounds its entities name that aren't on this computer
        if let Some(n) = ed.lock().as_ref().and_then(|s| super::assets::note_for(&st, s)) {
            notes.push(n);
        }
        state["notes"] = json!(notes);
        Ok(state)
    })
    .await
}

/// Mounted by tools/editor.rs (`routes_with`), sharing its session.
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/import-other", post(api_import_other))
        .route("/api/tools/editor/project", get(api_project))
        .route("/api/tools/editor/project/action", post(api_project_action))
        .route("/api/tools/editor/history", get(api_history))
        .route("/api/tools/editor/history/jump", post(api_history_jump))
        .route("/api/tools/editor/cmd2", post(api_cmd2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::Entity;

    /// P5: a project's permanent uid: given to an old project when it's opened (and written
    /// into it), read back cheaply, and it finds the project after a rename or a move; backups
    /// and the recent list follow it.
    #[test]
    fn projects_keep_a_uid_and_are_followed_by_it() {
        use super::super::{backup, backup_of, file_uid, find_project, recent_list, PROJECT_EXT};
        use crate::editor::doc::valid_uid;
        let root = std::env::temp_dir().join(format!("ls-p5-uid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let maps = root.join("maps");
        std::fs::create_dir_all(&maps).unwrap();
        let app = root.join("app");
        // an old project: no uid
        let mut d = MapDocument::new();
        d.world.keys.push(("message".into(), "uid test".into()));
        let old = maps.join(format!("arena.{PROJECT_EXT}"));
        std::fs::write(&old, d.to_json()).unwrap();
        assert_eq!(file_uid(&old), None);
        let (doc, project) = load_file(&old).unwrap();
        assert!(project);
        let uid = doc.editor.uid.clone().unwrap();
        assert!(valid_uid(&uid));
        assert_eq!(file_uid(&old).as_deref(), Some(uid.as_str()), "written into the file on open");
        assert_eq!(load_file(&old).unwrap().0.editor.uid.as_deref(), Some(uid.as_str()), "and kept");
        // a .map import gets none from the file (the session gives it one)
        // recents: one entry per project, following a rename
        remember(&app, &old, true);
        let renamed = maps.join(format!("arena_v2.{PROJECT_EXT}"));
        std::fs::rename(&old, &renamed).unwrap();
        assert_eq!(find_project(&app, &uid, Some(&old)).as_deref(), Some(renamed.as_path()));
        // moved into a sibling folder
        let other = root.join("elsewhere");
        std::fs::create_dir_all(&other).unwrap();
        let moved = other.join(format!("arena_final.{PROJECT_EXT}"));
        std::fs::rename(&renamed, &moved).unwrap();
        assert_eq!(find_project(&app, &uid, Some(&old)).as_deref(), Some(moved.as_path()));
        remember(&app, &moved, true);
        let list = recent_list(&app);
        assert_eq!(list.len(), 1, "the moved project replaced its old entry");
        assert_eq!(list[0].uid.as_deref(), Some(uid.as_str()));
        // another project with a broken uid isn't mistaken for it
        let mut d2 = MapDocument::new();
        d2.editor.uid = Some("nope".into());
        std::fs::write(maps.join(format!("other.{PROJECT_EXT}")), d2.to_json()).unwrap();
        assert_eq!(find_project(&app, &uid, Some(&old)).as_deref(), Some(moved.as_path()));
        // backups: named by uid, found by it after a rename; old ones by name still listed
        let bdir = app.join("backups");
        backup(&bdir, &moved, Some(&uid));
        std::fs::write(bdir.join(format!("arena_final-123.{PROJECT_EXT}")), "{}").unwrap();
        let names: Vec<String> = std::fs::read_dir(&bdir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        let by_uid = names.iter().find(|n| n.contains(&uid)).unwrap();
        assert!(backup_of(&bdir.join(by_uid), "renamed_again", Some(&uid)).is_some());
        assert!(backup_of(&bdir.join("arena_final-123.lsmap"), "arena_final", Some(&uid)) == Some(123));
        assert!(backup_of(&bdir.join(by_uid), "arena_final", Some("00000000-0000-4000-8000-000000000000")).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn referenced_files_from_entities() {
        let mut d = MapDocument::new();
        let mut e = Entity::new("ambient_generic");
        e.set("message", "ambience/wind1.wav");
        d.entities.insert(EntityId(1), e);
        let mut e = Entity::new("ambient_generic");
        e.set("message", "!HG_ALERT");
        d.entities.insert(EntityId(2), e);
        let mut e = Entity::new("env_sprite");
        e.set("model", "sprites/glow01.spr");
        d.entities.insert(EntityId(3), e);
        let mut e = Entity::new("func_door");
        e.set("noise1", "doors\\doormove1.wav");
        e.set("message", "not a sound");
        d.entities.insert(EntityId(4), e);
        let r: Vec<String> = referenced_files(&d).into_iter().collect();
        assert_eq!(r, vec!["sound/ambience/wind1.wav", "sound/doors/doormove1.wav", "sprites/glow01.spr"]);
    }

    #[test]
    fn renames_stay_in_the_folder() {
        let dir = std::env::temp_dir().join(format!("ls-p5-rename-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("room.lsmap");
        std::fs::write(&old, "{}").unwrap();
        assert_eq!(safe_rename(&old, "hall").unwrap(), dir.join("hall.lsmap"));
        assert_eq!(safe_rename(&old, "hall.lsmap").unwrap(), dir.join("hall.lsmap"));
        assert!(safe_rename(&old, "../evil").is_err());
        assert!(safe_rename(&old, "a/b").is_err());
        assert!(safe_rename(&old, "").is_err());
        assert!(safe_rename(&old, "room").is_err(), "already there");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_in_refuses_climbing_out() {
        assert!(find_in(&[std::env::temp_dir()], "../etc/passwd").is_none());
        assert!(find_in(&[std::env::temp_dir()], "/etc/passwd").is_none());
    }
}
