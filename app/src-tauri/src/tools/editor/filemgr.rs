//! The Content Browser's file management for your own files (Parity A): the project folder's
//! subfolders, New folder, move and rename (the map's references follow, in one undo step with
//! the files), delete (into the editor's deleted files folder, undoable), and Copy to Mine for
//! the game's, downloaded and Community files.
//!   GET  /api/tools/editor/content/folders
//!   POST /api/tools/editor/content/folder       {kind, path}
//!   POST /api/tools/editor/content/move         {from, to}
//!   POST /api/tools/editor/content/delete       {path, confirm?}
//!   POST /api/tools/editor/content/copy-to-mine {folder, path}
//! Paths are the project folder's own (`models/props/crate.mdl`, `mine.wad`), checked like
//! Import's (safe names, the right type root, never the game's own names, never over a file).

use super::importexport::{companion_of, place_for};
use super::{blocking, Ed};
use crate::editor::doc::{Entity, EntityId, MapDocument, WorldSettings};
use crate::editor::history::Tx;
use crate::server::Shared;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const ROOTS: [(&str, &str); 3] = [("model", "models"), ("sprite", "sprites"), ("sound", "sound")];

/// The type root a project path is under ("models", "sprites", "sound"), or "" for a .wad beside
/// the project.
fn root_of(rel: &str) -> Option<&'static str> {
    let low = rel.to_ascii_lowercase();
    if low.ends_with(".wad") {
        return (!rel.contains('/')).then_some("");
    }
    ROOTS.iter().map(|r| r.1).find(|r| low.starts_with(&format!("{r}/")))
}

/// A checked project path: a clean asset path (or a bare .wad name), safe parts, under its type.
pub fn checked_rel(rel: &str) -> Result<String, String> {
    let rel = rel.trim().replace('\\', "/");
    let bad = || format!("{rel} isn't a name the game or Windows can use (letters, digits, _ - . and folders of those)");
    let root = root_of(&rel).ok_or_else(|| format!("{rel} has to stay in models/, sprites/ or sound/ (a .wad beside the project)"))?;
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() > 6 || parts.iter().any(|p| p.is_empty() || p.starts_with('.') || p.len() > 64 || crate::setup::record::windows_unsafe(p) || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))) {
        return Err(bad());
    }
    let file = parts[parts.len() - 1];
    let (placed, _) = place_for(file).map_err(|_| bad())?;
    if root.is_empty() {
        return Ok(placed);
    }
    if super::assets::clean_rel(&rel).is_none() || !placed.starts_with(&format!("{root}/")) {
        return Err(bad());
    }
    Ok(rel)
}

/// A model's companions beside it (`xT.mdl`, `x01.mdl`...), as project paths.
fn companions(project: &Path, rel: &str) -> Vec<String> {
    let Some(stem) = rel.to_ascii_lowercase().strip_suffix(".mdl").map(str::to_string) else { return Vec::new() };
    let (dir, base) = stem.rsplit_once('/').unwrap_or(("", &stem));
    let mut out = Vec::new();
    for e in std::fs::read_dir(project.join(dir)).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if let Some(s) = n.to_ascii_lowercase().strip_suffix(".mdl") {
            if s != base && companion_of(s, base) && e.path().is_file() {
                out.push(if dir.is_empty() { n } else { format!("{dir}/{n}") });
            }
        }
    }
    out.sort();
    out
}

/// What a key's value names, as a project path (sounds under sound/), like the map's checks read it.
fn named(v: &str) -> Option<String> {
    let t = v.trim().trim_start_matches(['*', '#']).replace('\\', "/");
    if t.starts_with('!') {
        return None;
    }
    let low = t.to_ascii_lowercase();
    let t = if (low.ends_with(".wav") || low.ends_with(".mp3")) && !low.starts_with("sound/") { format!("sound/{t}") } else { t };
    super::assets::clean_rel(&t)
}

/// The value for a new path, written the way the old one was (a sound without sound/, a leading
/// * or # kept).
fn rewritten(old_value: &str, new_rel: &str) -> String {
    let lead: String = old_value.trim().chars().take_while(|c| *c == '*' || *c == '#').collect();
    let body = old_value.trim().trim_start_matches(['*', '#']).replace('\\', "/");
    let new = if new_rel.to_ascii_lowercase().starts_with("sound/") && !body.to_ascii_lowercase().starts_with("sound/") { &new_rel["sound/".len()..] } else { new_rel };
    format!("{lead}{new}")
}

/// The entities and texture list that name `old`, rewritten to `new`; and how many name it.
pub fn rewrite_refs(doc: &MapDocument, old: &str, new: &str) -> (Vec<(EntityId, Entity)>, Option<WorldSettings>, usize) {
    let mut ents = Vec::new();
    let mut n = 0;
    for (id, e) in &doc.entities {
        let mut changed = false;
        let mut e2 = e.clone();
        for (k, v) in e2.keys.iter_mut() {
            if k == "classname" || k == "targetname" || k == "target" {
                continue;
            }
            if named(v).is_some_and(|r| r.eq_ignore_ascii_case(old)) {
                *v = rewritten(v, new);
                changed = true;
            }
        }
        if changed {
            n += 1;
            ents.push((*id, e2));
        }
    }
    let mut world = None;
    if old.to_ascii_lowercase().ends_with(".wad") {
        let hits = |w: &str| w.replace('\\', "/").rsplit('/').next().is_some_and(|f| f.eq_ignore_ascii_case(old));
        if doc.world.wads.iter().any(|w| hits(w)) {
            let mut w = doc.world.clone();
            w.wads = w.wads.iter().map(|x| if hits(x) { new.to_string() } else { x.clone() }).collect();
            n += 1;
            world = Some(w);
        }
    }
    (ents, world, n)
}

/// Moves (or renames) a file and its companions inside `project`, and rewrites what the map names,
/// as one undo step on `doc`. The paths moved and the entities changed.
pub fn move_in(project: &Path, doc: &mut MapDocument, history: &mut crate::editor::history::History, from: &str, to: &str, game: &str) -> Result<Value, String> {
    let from = checked_rel(from)?;
    let to = checked_rel(to)?;
    if root_of(&from) != root_of(&to) {
        return Err("A file stays with its kind: models in models/, sprites in sprites/, sounds in sound/.".into());
    }
    if from.rsplit_once('.').map(|x| x.1.to_ascii_lowercase()) != to.rsplit_once('.').map(|x| x.1.to_ascii_lowercase()) {
        return Err("Keep the file's type (its ending): .mdl, .spr, .wav or .wad.".into());
    }
    if from.eq_ignore_ascii_case(&to) {
        return Err("That's where it is already.".into());
    }
    let is_stock = |r: &str| if r.ends_with(".wad") { crate::tools::doctor::stock_wad_names(game).iter().any(|w| w.eq_ignore_ascii_case(r)) } else { crate::tools::doctor::is_stock(r, game) };
    if is_stock(&to) {
        return Err(format!("The game has its own {to}: pick another name."));
    }
    let src = project.join(&from);
    if !src.is_file() {
        return Err(format!("{from} isn't in the project's folder."));
    }
    // the model's T and sequence files follow it: xT.mdl → yT.mdl, x01.mdl → y01.mdl
    let mut pairs = vec![(from.clone(), to.clone())];
    if from.to_ascii_lowercase().ends_with(".mdl") {
        let old_stem = &from[..from.len() - 4];
        let new_stem = &to[..to.len() - 4];
        for c in companions(project, &from) {
            let tail = &c[old_stem.len()..];
            pairs.push((c.clone(), format!("{new_stem}{tail}")));
        }
    }
    for (_, b) in &pairs {
        if project.join(b).exists() || crate::mapcopy::Finder::default().find(project, b).is_some() {
            return Err(format!("There's a {b} already. Pick another name."));
        }
    }
    let label = if from.rsplit_once('/').map(|x| x.0) == to.rsplit_once('/').map(|x| x.0) { format!("Rename {from}") } else { format!("Move {from}") };
    let mut tx = Tx::new(doc, &label);
    tx.move_files(pairs.iter().map(|(a, b)| (project.join(a), project.join(b))).collect())?;
    let (ents, world, n) = rewrite_refs(tx.doc(), &from, &to);
    for (id, e) in ents {
        tx.set_entity(id, e);
    }
    if let Some(w) = world {
        tx.set_world(w);
    }
    tx.commit(history);
    Ok(json!({"ok": true, "moved": pairs.iter().map(|(a, b)| json!({"from": a, "to": b})).collect::<Vec<_>>(), "references": n, "path": to}))
}

/// Deletes a file (and a model's companions): moved into `bin`, one undo step (Undo brings it back).
pub fn delete_in(project: &Path, bin: &Path, doc: &mut MapDocument, history: &mut crate::editor::history::History, rel: &str) -> Result<Value, String> {
    let rel = checked_rel(rel)?;
    if !project.join(&rel).is_file() {
        return Err(format!("{rel} isn't in the project's folder."));
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut files = vec![rel.clone()];
    files.extend(companions(project, &rel));
    let pairs: Vec<(PathBuf, PathBuf)> = files.iter().map(|f| (project.join(f), bin.join(format!("{secs}-{}", f.replace('/', "_"))))).collect();
    let mut tx = Tx::new(doc, &format!("Delete {rel}"));
    tx.move_files(pairs)?;
    tx.commit(history);
    Ok(json!({"ok": true, "deleted": files, "keptIn": bin.to_string_lossy()}))
}

/// How many entities (and the texture list) name a project path.
pub fn uses(doc: &MapDocument, rel: &str) -> usize {
    rewrite_refs(doc, rel, rel).2
}

/// The project folder's subfolders under each type root (models/props, ...).
pub fn folders(project: &Path) -> Value {
    let mut out = serde_json::Map::new();
    for (kind, root) in ROOTS {
        let mut v = Vec::new();
        let mut stack = vec![(project.join(root), root.to_string(), 0)];
        while let Some((d, rel, depth)) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) && !e.path().is_symlink() {
                    let n = e.file_name().to_string_lossy().to_string();
                    let r = format!("{rel}/{n}");
                    if depth < 4 && super::assets::clean_rel(&format!("{r}/x.{}", if kind == "model" { "mdl" } else if kind == "sprite" { "spr" } else { "wav" })).is_some() {
                        v.push(r.clone());
                        stack.push((e.path(), r, depth + 1));
                    }
                }
            }
        }
        v.sort();
        out.insert(kind.into(), json!(v));
    }
    Value::Object(out)
}

/// Makes a folder under a type root (`models/props`), safe names only.
pub fn new_folder(project: &Path, kind: &str, path: &str) -> Result<String, String> {
    let root = ROOTS.iter().find(|r| r.0 == kind).map(|r| r.1).ok_or("Which kind: model, sprite or sound?")?;
    let path = path.trim().trim_matches('/').replace('\\', "/");
    let rel = if path.to_ascii_lowercase().starts_with(&format!("{root}/")) { path } else { format!("{root}/{path}") };
    let ext = match kind {
        "model" => "mdl",
        "sprite" => "spr",
        _ => "wav",
    };
    checked_rel(&format!("{rel}/x.{ext}")).map_err(|_| format!("{rel} isn't a folder name the game or Windows can use (letters, digits, _ - .)"))?;
    let dir = project.join(&rel);
    if dir.exists() {
        return Err(format!("{rel} is there already."));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make {rel} ({e})."))?;
    Ok(rel)
}

// ---- routes ------------------------------------------------------------------------------

fn forbidden() -> Response {
    (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

fn game_of(st: &Shared) -> String {
    st.local_game().map(|g| if g.is_czero() { "czero" } else { "cstrike" }.to_string()).unwrap_or_else(|| "cstrike".into())
}

/// Other projects saved in the same folder (they may name these files too; only the open one changes).
fn others(project_file: &Path) -> usize {
    let Some(dir) = project_file.parent() else { return 0 };
    std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path() != project_file && e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("lsmap"))).count()
}

const NO_PROJECT: &str = "Save the project first: your files live in its folder.";

async fn api_folders(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let dir = ed.lock().as_ref().and_then(|s| s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf));
        Ok(match dir {
            Some(d) => json!({"folders": folders(&d), "project": true}),
            None => json!({"folders": {}, "project": false}),
        })
    })
    .await
}

#[derive(Deserialize)]
struct FolderReq {
    kind: String,
    path: String,
}

async fn api_folder(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<FolderReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = ed.lock().as_ref().and_then(|s| s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf)).ok_or(NO_PROJECT)?;
        let rel = new_folder(&dir, &req.kind, &req.path)?;
        Ok(json!({"ok": true, "path": rel, "folders": folders(&dir)}))
    })
    .await
}

#[derive(Deserialize)]
struct MoveReq {
    from: String,
    to: String,
}

async fn api_move(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<MoveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let game = game_of(&st);
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(super::NO_DOC)?;
        let file = s.project.clone().ok_or(NO_PROJECT)?;
        let dir = file.parent().ok_or(NO_PROJECT)?.to_path_buf();
        let mut v = move_in(&dir, &mut s.doc, &mut s.history, &req.from, &req.to, &game)?;
        let n = others(&file);
        if n > 0 {
            v["others"] = json!(format!("{n} other project{} in this folder may use it too; only the open one was updated.", if n == 1 { "" } else { "s" }));
        }
        v["state"] = s.state();
        Ok(v)
    })
    .await
}

#[derive(Deserialize)]
struct DeleteReq {
    path: String,
    #[serde(default)]
    confirm: bool,
}

async fn api_delete(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<DeleteReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let bin = ed.dir.join("deleted");
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(super::NO_DOC)?;
        let dir = s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf).ok_or(NO_PROJECT)?;
        let rel = checked_rel(&req.path)?;
        let n = uses(&s.doc, &rel);
        if n > 0 && !req.confirm {
            return Ok(json!({"uses": n, "ask": format!("The map still uses {rel} ({n} place{}). Delete it anyway?", if n == 1 { "" } else { "s" })}));
        }
        let mut v = delete_in(&dir, &bin, &mut s.doc, &mut s.history, &rel)?;
        v["state"] = s.state();
        Ok(v)
    })
    .await
}

#[derive(Deserialize)]
struct CopyReq {
    folder: String,
    path: String,
}

async fn api_copy_to_mine(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<CopyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let g = st.local_game().ok_or("Set up the game on this computer first.")?;
        let game = if g.is_czero() { "czero" } else { "cstrike" };
        let dir = ed.lock().as_ref().and_then(|s| s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf)).ok_or(NO_PROJECT)?;
        if req.folder == "project" {
            return Err("That one is yours already.".into());
        }
        let src = super::browser::resolve_file(&g.hl, game, Some(&dir), &req.folder, &req.path).ok_or("That file isn't there any more.")?;
        Ok(copy_to_mine(&src, &req.path, &dir, game))
    })
    .await
}

/// Copies a game, downloaded or Community file (and a model's companions) into the project at
/// the same path, through Import's checks; a name the game itself uses gets `_mine` (yours would
/// replace everyone's when shared), a taken one a free name. Its bytes are unchanged, so its
/// credits still trace back to its maker when you publish.
pub fn copy_to_mine(src: &Path, rel: &str, project: &Path, game: &str) -> Value {
    let mut files = vec![src.to_path_buf()];
    files.extend(super::importexport::with_companions(src).into_iter().skip(1));
    let stock_name = crate::tools::doctor::is_stock(rel, game);
    let sub = rel.rsplit_once('/').map(|x| x.0).unwrap_or("");
    // (the main file's stem; its companions keep their tail: x.mdl → x_mine.mdl, xT.mdl → x_mineT.mdl)
    let base = src.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let incoming: Vec<super::importexport::Incoming> = files
        .iter()
        .filter_map(|f| {
            let name = f.file_name()?.to_string_lossy().to_string();
            let name = if stock_name && name.len() >= base.len() && name[..base.len()].eq_ignore_ascii_case(&base) { format!("{base}_mine{}", &name[base.len()..]) } else { name };
            Some(super::importexport::Incoming { name, bytes: std::fs::read(f).ok()?, rename: true })
        })
        .collect();
    // (into the same subfolder it came from: models/props/x.mdl stays under models/props/)
    let place = |name: &str| {
        place_for(name).map(|(rel, ext)| match rel.split_once('/') {
            Some((root, file)) if ext != "wad" && sub.to_ascii_lowercase().starts_with(&format!("{root}/")) && super::assets::clean_rel(&format!("{sub}/{file}")).is_some() => (format!("{sub}/{file}"), ext),
            _ => (rel, ext),
        })
    };
    let exists = |r: &str| crate::mapcopy::Finder::default().find(project, r).is_some() || project.join(r).exists();
    let mut v = super::importexport::import_into(project, game, incoming, &place, &exists);
    v["source"] = json!(rel);
    v
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/content/folders", get(api_folders))
        .route("/api/tools/editor/content/folder", post(api_folder))
        .route("/api/tools/editor/content/move", post(api_move))
        .route("/api/tools/editor/content/delete", post(api_delete))
        .route("/api/tools/editor/content/copy-to-mine", post(api_copy_to_mine))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::history::History;

    fn ent(keys: &[(&str, &str)]) -> Entity {
        Entity { keys: keys.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Default::default() }
    }

    #[test]
    fn moves_and_renames_with_references_in_one_undo_step() {
        let root = std::env::temp_dir().join(format!("ls-filemgr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (p, b) in [("models/crate.mdl", "IDST c"), ("models/crateT.mdl", "IDST t"), ("sprites/glow.spr", "IDSP g"), ("sound/hum.wav", "RIFF....WAVE"), ("mine.wad", "WAD3")] {
            std::fs::create_dir_all(root.join(p).parent().unwrap()).unwrap();
            std::fs::write(root.join(p), b).unwrap();
        }
        let mut doc = MapDocument::default();
        doc.world.wads = vec!["mine.wad".into(), "halflife.wad".into()];
        let a = doc.new_entity_id();
        doc.entities.insert(a, ent(&[("classname", "cycler_sprite"), ("model", "models/crate.mdl")]));
        let b = doc.new_entity_id();
        doc.entities.insert(b, ent(&[("classname", "env_sprite"), ("model", "sprites/glow.spr")]));
        let c = doc.new_entity_id();
        doc.entities.insert(c, ent(&[("classname", "ambient_generic"), ("message", "hum.wav")]));
        let d = doc.new_entity_id();
        doc.entities.insert(d, ent(&[("classname", "ls_npc"), ("model", "MODELS\\Crate.mdl"), ("target", "models/crate.mdl")]));
        let mut h = History::new(100);
        let key = |doc: &MapDocument, id: EntityId, k: &str| doc.entities[&id].keys.iter().find(|x| x.0 == k).unwrap().1.clone();

        // move the model into a folder, renamed: its T file follows, both entities point at it
        let v = move_in(&root, &mut doc, &mut h, "models/crate.mdl", "models/props/box.mdl", "cstrike").unwrap();
        assert_eq!(v["references"], 2);
        assert!(root.join("models/props/box.mdl").is_file() && root.join("models/props/boxT.mdl").is_file());
        assert!(!root.join("models/crate.mdl").exists() && !root.join("models/crateT.mdl").exists());
        assert_eq!(key(&doc, a, "model"), "models/props/box.mdl");
        assert_eq!(key(&doc, d, "model"), "models/props/box.mdl");
        assert_eq!(key(&doc, d, "target"), "models/crate.mdl", "a target name isn't a file");
        // the sprite, the sound (written as it was: no sound/), the WAD in the texture list
        move_in(&root, &mut doc, &mut h, "sprites/glow.spr", "sprites/fx/glow2.spr", "cstrike").unwrap();
        assert_eq!(key(&doc, b, "model"), "sprites/fx/glow2.spr");
        move_in(&root, &mut doc, &mut h, "sound/hum.wav", "sound/amb/hum_low.wav", "cstrike").unwrap();
        assert_eq!(key(&doc, c, "message"), "amb/hum_low.wav");
        move_in(&root, &mut doc, &mut h, "mine.wad", "mine_v2.wad", "cstrike").unwrap();
        assert_eq!(doc.world.wads, ["mine_v2.wad", "halflife.wad"]);
        assert!(root.join("mine_v2.wad").is_file());

        // refused: a clash, an unsafe name, the game's own name, another kind's folder
        std::fs::write(root.join("sprites/taken.spr"), b"IDSP").unwrap();
        assert!(move_in(&root, &mut doc, &mut h, "sprites/fx/glow2.spr", "sprites/taken.spr", "cstrike").unwrap_err().contains("already"));
        assert!(move_in(&root, &mut doc, &mut h, "sprites/fx/glow2.spr", "sprites/con.spr", "cstrike").is_err());
        assert!(move_in(&root, &mut doc, &mut h, "sprites/fx/glow2.spr", "sprites/../x.spr", "cstrike").is_err());
        assert!(move_in(&root, &mut doc, &mut h, "models/props/box.mdl", "models/v_ak47.mdl", "cstrike").unwrap_err().contains("game has its own"));
        assert!(move_in(&root, &mut doc, &mut h, "models/props/box.mdl", "sprites/box.mdl", "cstrike").is_err());

        // undo, step by step: files and references back together
        h.undo(&mut doc); // the WAD
        assert_eq!(doc.world.wads, ["mine.wad", "halflife.wad"]);
        assert!(root.join("mine.wad").is_file() && !root.join("mine_v2.wad").exists());
        h.undo(&mut doc);
        h.undo(&mut doc);
        h.undo(&mut doc); // the model
        assert_eq!(key(&doc, a, "model"), "models/crate.mdl");
        assert_eq!(key(&doc, c, "message"), "hum.wav");
        assert!(root.join("models/crate.mdl").is_file() && root.join("models/crateT.mdl").is_file() && root.join("sound/hum.wav").is_file());
        assert!(!root.join("models/props/box.mdl").exists());
        // and redo
        h.redo(&mut doc);
        assert!(root.join("models/props/boxT.mdl").is_file());
        assert_eq!(key(&doc, a, "model"), "models/props/box.mdl");

        // delete: into the bin, the uses counted first; undo brings it back
        let bin = root.join("bin");
        assert_eq!(uses(&doc, "models/props/box.mdl"), 2);
        delete_in(&root, &bin, &mut doc, &mut h, "models/props/box.mdl").unwrap();
        assert!(!root.join("models/props/box.mdl").exists() && !root.join("models/props/boxT.mdl").exists());
        h.undo(&mut doc);
        assert!(root.join("models/props/box.mdl").is_file() && root.join("models/props/boxT.mdl").is_file());

        // folders
        assert_eq!(new_folder(&root, "model", "props/big").unwrap(), "models/props/big");
        assert!(new_folder(&root, "model", "../x").is_err() && new_folder(&root, "model", "con").is_err());
        assert!(folders(&root)["model"].as_array().unwrap().iter().any(|f| f == "models/props/big"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_to_mine_keeps_the_file_and_never_takes_the_games_name() {
        let root = std::env::temp_dir().join(format!("ls-copymine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let game = root.join("game");
        std::fs::create_dir_all(game.join("models/props")).unwrap();
        std::fs::write(game.join("models/props/barrel.mdl"), b"IDST barrel").unwrap();
        std::fs::write(game.join("models/props/barrelT.mdl"), b"IDST tex").unwrap();
        std::fs::write(game.join("models/v_ak47.mdl"), b"IDST ak").unwrap();
        let proj = root.join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        let v = copy_to_mine(&game.join("models/props/barrel.mdl"), "models/props/barrel.mdl", &proj, "cstrike");
        assert!(proj.join("models/props/barrel.mdl").is_file() && proj.join("models/props/barrelT.mdl").is_file(), "{v}");
        assert_eq!(std::fs::read(proj.join("models/props/barrel.mdl")).unwrap(), b"IDST barrel", "unchanged: its credits still trace");
        // again: a free name, the T file following
        copy_to_mine(&game.join("models/props/barrel.mdl"), "models/props/barrel.mdl", &proj, "cstrike");
        assert!(proj.join("models/props/barrel_2.mdl").is_file() && proj.join("models/props/barrel_2T.mdl").is_file());
        // the game's own name: yours gets _mine
        copy_to_mine(&game.join("models/v_ak47.mdl"), "models/v_ak47.mdl", &proj, "cstrike");
        assert!(proj.join("models/v_ak47_mine.mdl").is_file() && !proj.join("models/v_ak47.mdl").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
