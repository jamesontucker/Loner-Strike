//! The files a map's entities name (agent P5): models, sprites, sounds and the sky. Like the
//! texture files, each must be on the computer or the game stops loading the map
//! ("Mod_LoadModel: sprites/x.spr not found"). Each one is looked for where the game looks (the
//! game's folder, its _downloads folder, cstrike and valve), then beside the project and the
//! file it was imported from (and that folder's parent: a map's folder often sits next to its
//! models/ sprites/ sound/), then in the other games' folders.
//!   GET  /api/tools/editor/assets          every named file and where it is
//!   POST /api/tools/editor/assets/copy     {to: "game"|"project"}: copies the found ones there
//! Playtest (tools/compile.rs) refuses while one is missing, copies the found ones into the
//! game's _downloads folder and writes the map's .res.

use super::{blocking, forbidden, global, Ed, Session, NO_DOC};
use crate::editor::checks::{Issue, Level};
use crate::editor::doc::MapDocument;
use crate::mapcopy::Finder;
use crate::server::Shared;
use crate::tools::doctor::{self, Found};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const SKY_SIDES: [&str; 6] = ["up", "dn", "lf", "rt", "ft", "bk"];
/// (a map with more is broken or hostile; the rest aren't looked at)
const MAX_FILES: usize = 2000;

/// One file a map names (relative to the game folder, forward slashes).
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub rel: String,
    /// model, sprite, sound, sky
    pub kind: &'static str,
    /// the entities that name it (0: the world)
    pub users: Vec<u64>,
    /// the class of the first one (for messages)
    pub class: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// where the game looks (`stock`: every game has it)
    Game { stock: bool },
    /// beside the project or the imported file, not in the game yet
    Beside(PathBuf),
    /// in another game's folder (Condition Zero's for a CS 1.6 map, ...)
    Elsewhere(PathBuf),
    Missing,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub named: Named,
    pub status: Status,
}

/// A value that's a path inside the game folder (no drive letters, no climbing out).
fn clean(v: &str) -> Option<String> {
    let v = v.trim().trim_start_matches(['*', '#']).replace('\\', "/");
    let v = v.trim_start_matches('/').to_string();
    if v.is_empty() || v.len() > 120 || v.contains("..") || v.contains(':') || v.starts_with('!') {
        return None;
    }
    v.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-./ +~()[]{}".contains(&b)).then_some(v)
}

/// The files the map's entities name, with who names them.
pub fn named_files(doc: &MapDocument) -> Vec<Named> {
    let mut out: Vec<Named> = Vec::new();
    let mut add = |rel: String, kind: &'static str, user: u64, class: &str| {
        if out.len() >= MAX_FILES {
            return;
        }
        match out.iter_mut().find(|n| n.rel.eq_ignore_ascii_case(&rel)) {
            Some(n) => {
                if !n.users.contains(&user) {
                    n.users.push(user);
                }
            }
            None => out.push(Named { rel, kind, users: vec![user], class: class.to_string() }),
        }
    };
    if let Some(sky) = doc.world.get("skyname").map(str::trim).filter(|s| !s.is_empty() && s.len() <= 40 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))) {
        for side in SKY_SIDES {
            add(format!("gfx/env/{sky}{side}.tga"), "sky", 0, "worldspawn");
        }
    }
    for (id, e) in &doc.entities {
        let class = e.classname();
        for (k, v) in &e.keys {
            if k == "classname" || k == "targetname" || k == "target" {
                continue;
            }
            let Some(rel) = clean(v) else { continue };
            let low = rel.to_ascii_lowercase();
            if low.ends_with(".mdl") || low.ends_with(".spr") {
                add(rel, if low.ends_with(".spr") { "sprite" } else { "model" }, id.0, class);
            } else if low.ends_with(".wav") || low.ends_with(".mp3") {
                let rel = if low.starts_with("sound/") { rel } else { format!("sound/{rel}") };
                add(rel, "sound", id.0, class);
            }
        }
    }
    out
}

/// Where to look: the game (when it's set up) and the map's own folders.
pub struct Places {
    pub hl: Option<PathBuf>,
    /// "cstrike" or "czero"
    pub game: String,
    /// the project's folder and the imported file's, each then its parent
    pub beside: Vec<PathBuf>,
}

impl Places {
    pub(super) fn of(st: &Shared, s: &Session) -> Places {
        let g = st.local_game();
        Places {
            hl: g.as_ref().map(|g| g.hl.clone()),
            game: g.as_ref().map(|g| if g.is_czero() { "czero" } else { "cstrike" }).unwrap_or("cstrike").to_string(),
            beside: beside_dirs(s),
        }
    }
}

fn beside_dirs(s: &Session) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    let roots = [s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf), s.folder.clone(), s.doc.editor.source.as_ref().map(PathBuf::from)];
    for d in roots.into_iter().flatten() {
        for x in [Some(d.clone()), d.parent().map(Path::to_path_buf)].into_iter().flatten() {
            if x.is_dir() && !v.contains(&x) {
                v.push(x);
            }
        }
    }
    v
}

/// The sky's sides can be .tga or .bmp.
fn alternatives(rel: &str) -> Vec<String> {
    let mut v = vec![rel.to_string()];
    if rel.starts_with("gfx/env/") && rel.ends_with(".tga") {
        v.push(format!("{}.bmp", &rel[..rel.len() - 4]));
    }
    v
}

pub fn locate(named: Vec<Named>, places: &Places) -> Vec<Asset> {
    let f = Finder::default();
    let path = places.hl.as_ref().map(|hl| doctor::game_path(hl, &places.game)).unwrap_or_default();
    named
        .into_iter()
        .map(|n| {
            let mut status = Status::Missing;
            'look: for rel in alternatives(&n.rel) {
                if let Some(hl) = &places.hl {
                    if let Found::Here { stock, .. } = doctor::find_file(&f, hl, &path, &rel) {
                        status = Status::Game { stock };
                        break 'look;
                    }
                }
                for d in &places.beside {
                    if let Some(p) = f.find(d, &rel) {
                        status = Status::Beside(p);
                        break 'look;
                    }
                }
                if let Some(hl) = &places.hl {
                    if let Found::Elsewhere { path, .. } = doctor::find_file(&f, hl, &path, &rel) {
                        status = Status::Elsewhere(path);
                        break 'look;
                    }
                }
            }
            Asset { named: n, status }
        })
        .collect()
}

fn who(n: &Named) -> String {
    let more = n.users.len().saturating_sub(1);
    let first = if n.users.first() == Some(&0) { "the map's sky".to_string() } else { format!("{} #{}", n.class, n.users[0]) };
    if more > 0 {
        format!("{first} and {more} more")
    } else {
        first
    }
}

/// Live checks: a missing file stops the game loading the map.
pub fn issues(assets: &[Asset], game_set_up: bool) -> Vec<Issue> {
    let mut out = Vec::new();
    for a in assets {
        let entity = a.named.users.first().copied().filter(|u| *u != 0);
        let (level, code, message) = match &a.status {
            Status::Missing if game_set_up => (
                Level::Error,
                "missing-file",
                format!("{} isn't on this computer ({}). The game stops loading a map at a missing {}: put the file beside the project (in its {}/ folder), or change the entity.", a.named.rel, who(&a.named), a.named.kind, a.named.rel.split('/').next().unwrap_or("")),
            ),
            Status::Beside(_) | Status::Elsewhere(_) => (Level::Info, "file-not-in-game", format!("{} ({}) isn't in the game yet; Playtest copies it there.", a.named.rel, who(&a.named))),
            _ => continue,
        };
        // (the sky: one line for its sides)
        if a.named.kind == "sky" && out.iter().any(|i: &Issue| i.code == code && i.message.contains("the map's sky")) {
            continue;
        }
        out.push(Issue { level, code, message, brush: None, entity, texture: None, fix: None });
    }
    out
}

fn status_json(s: &Status) -> Value {
    match s {
        Status::Game { stock } => json!({"status": "game", "stock": stock}),
        Status::Beside(p) => json!({"status": "beside", "from": p.to_string_lossy()}),
        Status::Elsewhere(p) => json!({"status": "elsewhere", "from": p.to_string_lossy()}),
        Status::Missing => json!({"status": "missing"}),
    }
}

pub fn assets_json(assets: &[Asset], places: &Places, project_dir: Option<&Path>) -> Value {
    let list: Vec<Value> = assets
        .iter()
        .map(|a| {
            let mut v = status_json(&a.status);
            v["path"] = json!(a.named.rel);
            v["kind"] = json!(a.named.kind);
            v["entities"] = json!(a.named.users.iter().filter(|u| **u != 0).collect::<Vec<_>>());
            v["who"] = json!(who(&a.named));
            v["inProject"] = json!(project_dir.is_some_and(|d| Finder::default().find(d, &a.named.rel).is_some()));
            v
        })
        .collect();
    let missing = assets.iter().filter(|a| a.status == Status::Missing).count();
    let to_game = assets.iter().filter(|a| matches!(a.status, Status::Beside(_) | Status::Elsewhere(_))).count();
    json!({"assets": list, "missing": missing, "toGame": to_game, "game": places.hl.is_some(), "saved": project_dir.is_some()})
}

/// Copies `src` to `dest` unless something is there already. Ok(true) when it copied.
fn copy_new(src: &Path, dest: &Path) -> Result<bool, String> {
    if dest.exists() {
        return Ok(false);
    }
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("Couldn't make {} ({e}).", p.display()))?;
    }
    std::fs::copy(src, dest).map_err(|e| format!("Couldn't copy {} ({e}).", src.display()))?;
    Ok(true)
}

/// The game's own download folder (what a server sends players, and where the game looks).
fn downloads(hl: &Path, game: &str) -> PathBuf {
    hl.join(format!("{game}_downloads"))
}

/// Copies the found files that aren't in the game into its _downloads folder.
pub fn copy_to_game(assets: &[Asset], places: &Places) -> Result<Vec<String>, String> {
    let hl = places.hl.as_ref().ok_or("The game isn't set up, so there's no game folder to copy to.")?;
    let to = downloads(hl, &places.game);
    let mut done = Vec::new();
    for a in assets {
        if let Status::Beside(src) | Status::Elsewhere(src) = &a.status {
            let rel = src_rel(src, &a.named.rel);
            if copy_new(src, &to.join(&rel))? {
                done.push(rel);
            }
        }
    }
    Ok(done)
}

/// The path to copy to: the named path, with the found file's own extension (a .bmp sky side).
fn src_rel(src: &Path, rel: &str) -> String {
    match (src.extension(), rel.rsplit_once('.')) {
        (Some(e), Some((stem, _))) => format!("{stem}.{}", e.to_string_lossy().to_ascii_lowercase()),
        _ => rel.to_string(),
    }
}

/// Copies every found file that isn't one of the game's own next to the project (so the
/// project folder holds all the map needs, to share or move).
pub fn copy_to_project(assets: &[Asset], places: &Places, project_dir: &Path) -> Result<Vec<String>, String> {
    let f = Finder::default();
    let path = places.hl.as_ref().map(|hl| doctor::game_path(hl, &places.game)).unwrap_or_default();
    let mut done = Vec::new();
    for a in assets {
        let src = match &a.status {
            Status::Beside(p) | Status::Elsewhere(p) => Some(p.clone()),
            Status::Game { stock: false } => alternatives(&a.named.rel).iter().find_map(|r| path.iter().find_map(|d| f.find(d, r))),
            _ => None,
        };
        let Some(src) = src else { continue };
        if src.starts_with(project_dir) {
            continue;
        }
        let rel = src_rel(&src, &a.named.rel);
        if copy_new(&src, &project_dir.join(&rel))? {
            done.push(rel);
        }
    }
    Ok(done)
}

// ---- Playtest -------------------------------------------------------------------------

/// What Playtest does with the map's files: none missing (else a plain message), the found
/// ones copied into the game.
pub struct ForPlaytest {
    assets: Vec<Asset>,
    places: Places,
}

pub fn for_playtest(st: &Shared) -> Result<ForPlaytest, String> {
    for_playtest_in(st, &global())
}

fn for_playtest_in(st: &Shared, ed: &Ed) -> Result<ForPlaytest, String> {
    let lock = ed.lock();
    let s = lock.as_ref().ok_or(NO_DOC)?;
    check_playtest(&s.doc, Places::of(st, s))
}

fn check_playtest(doc: &MapDocument, places: Places) -> Result<ForPlaytest, String> {
    if places.hl.is_none() {
        // (no game: the compile says so; nothing can be called missing)
        return Ok(ForPlaytest { assets: Vec::new(), places });
    }
    let assets = locate(named_files(doc), &places);
    let missing: Vec<&Asset> = assets.iter().filter(|a| a.status == Status::Missing).collect();
    if !missing.is_empty() {
        let mut sky = false;
        let list: Vec<String> = missing
            .iter()
            .filter(|a| a.named.kind != "sky" || !std::mem::replace(&mut sky, true))
            .take(6)
            .map(|a| if a.named.kind == "sky" { "the sky's pictures (gfx/env/…)".to_string() } else { format!("{} ({})", a.named.rel, who(&a.named)) })
            .collect();
        let more = missing.len().saturating_sub(list.len());
        return Err(format!(
            "Playtest stopped: the game would stop loading this map, because it uses {} that {} on this computer: {}{}. Put {} beside the project (in its models/, sprites/ or sound/ folder) or change those entities; the Project panel lists them.",
            if missing.len() == 1 { "a file" } else { "files" },
            if missing.len() == 1 { "isn't" } else { "aren't" },
            list.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() },
            if missing.len() == 1 { "it" } else { "them" },
        ));
    }
    Ok(ForPlaytest { assets, places })
}

impl ForPlaytest {
    /// After the map is in the game: its files copied into the game's _downloads folder, and
    /// its .res written when it needs files players may not have. Returns notes for the log.
    pub fn install(&self, map: &str) -> Vec<String> {
        let mut notes = Vec::new();
        match copy_to_game(&self.assets, &self.places) {
            Ok(done) if !done.is_empty() => notes.push(format!("Copied the map's files into the game: {}.", done.join(", "))),
            Ok(_) => {}
            Err(e) => notes.push(e),
        }
        let custom = self.assets.iter().any(|a| !matches!(a.status, Status::Game { stock: true }));
        if let (true, Some(hl)) = (custom, &self.places.hl) {
            let m = crate::tools::mapinfo::MapFile { name: map.to_string(), bsp: hl.join(&self.places.game).join("maps").join(format!("{map}.bsp")), game: self.places.game.clone(), hl: hl.clone(), czero: self.places.game == "czero" };
            match doctor::write_res(&m) {
                Ok(v) => notes.push(v["message"].as_str().unwrap_or("").to_string()),
                Err(e) => notes.push(e),
            }
        }
        notes
    }
}

// ---- routes ---------------------------------------------------------------------------

fn current(st: &Shared, ed: &Ed) -> Result<(Vec<Asset>, Places, Option<PathBuf>), String> {
    let g = ed.lock();
    let s = g.as_ref().ok_or(NO_DOC)?;
    let places = Places::of(st, s);
    let dir = s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf);
    Ok((locate(named_files(&s.doc), &places), places, dir))
}

async fn api_assets(State(st): State<Shared>, Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let (assets, places, dir) = current(&st, &ed)?;
        Ok(assets_json(&assets, &places, dir.as_deref()))
    })
    .await
}

#[derive(Deserialize)]
struct CopyReq {
    to: String,
}

async fn api_copy(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<CopyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let (assets, places, dir) = current(&st, &ed)?;
        let done = match req.to.as_str() {
            "game" => copy_to_game(&assets, &places)?,
            "project" => copy_to_project(&assets, &places, dir.as_deref().ok_or("Save the map first: the files go in the project's folder.")?)?,
            _ => return Err("Copy them where?".into()),
        };
        let (assets, places, dir) = current(&st, &ed)?;
        let mut v = assets_json(&assets, &places, dir.as_deref());
        v["copied"] = json!(done);
        v["message"] = json!(match (done.len(), req.to.as_str()) {
            (0, _) => "Nothing to copy: they're there already.".to_string(),
            (n, "game") => format!("Copied {n} file{} into the game's download folder.", if n == 1 { "" } else { "s" }),
            (n, _) => format!("Copied {n} file{} next to the project.", if n == 1 { "" } else { "s" }),
        });
        Ok(v)
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/editor/assets", get(api_assets)).route("/api/tools/editor/assets/copy", post(api_copy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::Entity;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-assets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

/// A map file with only entities and an empty texture list (version 30, 15 lumps).
    fn tiny_bsp(ents: &str) -> Vec<u8> {
        let mut lumps = [(0u32, 0u32); 15];
        lumps[0] = (124, ents.len() as u32);
        let tex_at = 124 + ents.len() as u32;
        lumps[2] = (tex_at, 4);
        for (i, l) in lumps.iter_mut().enumerate() {
            if i != 0 && i != 2 {
                *l = (tex_at + 4, 0);
            }
        }
        let mut b = 30u32.to_le_bytes().to_vec();
        for (o, l) in lumps {
            b.extend_from_slice(&o.to_le_bytes());
            b.extend_from_slice(&l.to_le_bytes());
        }
        b.extend_from_slice(ents.as_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b
    }

    fn doc_with(keys: &[(&str, &[(&str, &str)])]) -> MapDocument {
        let mut d = MapDocument::new();
        d.world.keys.push(("skyname".into(), "desert".into()));
        for (class, kv) in keys {
            let mut e = Entity::new(class);
            for (k, v) in kv.iter() {
                e.keys.push((k.to_string(), v.to_string()));
            }
            d.insert_entity(e);
        }
        d
    }

    #[test]
    fn named_found_missing_and_copied() {
        let root = tmp("a");
        let hl = root.join("Half-Life");
        // the game: one model and the sky in cstrike, a sound only in valve
        for (rel, what) in [("cstrike/models/barney.mdl", "x"), ("valve/sound/ambience/wind1.wav", "x")] {
            std::fs::create_dir_all(hl.join(rel).parent().unwrap()).unwrap();
            std::fs::write(hl.join(rel), what).unwrap();
        }
        for side in SKY_SIDES {
            std::fs::create_dir_all(hl.join("valve/gfx/env")).unwrap();
            std::fs::write(hl.join(format!("valve/gfx/env/desert{side}.tga")), "x").unwrap();
        }
        // the imported map's folder: maps/ with sprites/ beside it
        let src = root.join("de_kobbl");
        std::fs::create_dir_all(src.join("maps")).unwrap();
        std::fs::create_dir_all(src.join("sprites/de_kobbl")).unwrap();
        std::fs::write(src.join("sprites/de_kobbl/blin.spr"), "spr").unwrap();
        let d = doc_with(&[
            ("env_sprite", &[("model", "sprites/de_kobbl/blin.spr")]),
            ("env_glow", &[("model", "sprites\\de_kobbl\\BLIN.spr")]),
            ("cycler", &[("model", "models/barney.mdl")]),
            ("ambient_generic", &[("message", "ambience/wind1.wav")]),
            ("func_door", &[("noise1", "doors/custom_door.wav"), ("model", "*3")]),
            ("trigger_once", &[("message", "!HG_ALERT")]),
        ]);
        let named = named_files(&d);
        let rels: Vec<&str> = named.iter().map(|n| n.rel.as_str()).collect();
        assert_eq!(rels.len(), 6 + 4, "{rels:?}"); // 6 sky sides, the sprite once, model, 2 sounds
        assert!(rels.contains(&"sound/doors/custom_door.wav"));
        assert_eq!(named.iter().find(|n| n.kind == "sprite").unwrap().users.len(), 2);

        let places = Places { hl: Some(hl.clone()), game: "cstrike".into(), beside: vec![src.join("maps"), src.clone()] };
        let a = locate(named, &places);
        let st = |rel: &str| a.iter().find(|x| x.named.rel.eq_ignore_ascii_case(rel)).unwrap().status.clone();
        assert!(matches!(st("sprites/de_kobbl/blin.spr"), Status::Beside(_)));
        assert!(matches!(st("models/barney.mdl"), Status::Game { stock: false }));
        assert!(matches!(st("sound/ambience/wind1.wav"), Status::Game { stock: true }));
        assert_eq!(st("sound/doors/custom_door.wav"), Status::Missing);
        let iss = issues(&a, true);
        let miss: Vec<&Issue> = iss.iter().filter(|i| i.code == "missing-file").collect();
        assert_eq!(miss.len(), 1);
        assert!(miss[0].message.contains("func_door"), "{}", miss[0].message);
        assert_eq!(miss[0].entity, Some(5));
        assert_eq!(iss.iter().filter(|i| i.code == "file-not-in-game").count(), 1);

        // copying into the game's download folder; never over a file that's there
        let done = copy_to_game(&a, &places).unwrap();
        assert_eq!(done, vec!["sprites/de_kobbl/blin.spr".to_string()]);
        assert!(hl.join("cstrike_downloads/sprites/de_kobbl/blin.spr").is_file());
        assert!(copy_to_game(&a, &places).unwrap().is_empty());
        let again = locate(named_files(&d), &places);
        assert!(again.iter().all(|x| !matches!(x.status, Status::Beside(_))));
        // next to a project: the custom ones (sprite from the game now, the model), not valve's
        let proj = root.join("project");
        std::fs::create_dir_all(&proj).unwrap();
        let mut done = copy_to_project(&again, &places, &proj).unwrap();
        done.sort();
        assert_eq!(done, vec!["models/barney.mdl".to_string(), "sprites/de_kobbl/blin.spr".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An imported map whose env_sprite names a sprite that isn't here (de_kobbl's blin.spr):
    /// the live checks and the Project panel say so, and Playtest refuses until it's beside the map.
    #[tokio::test]
    async fn playtest_refuses_a_missing_sprite() {
        use super::super::{routes_with, Editor};
        use crate::server::AppState;
        use std::sync::{Arc, Mutex, OnceLock, RwLock};
        use tower::ServiceExt;
        let root = tmp("pt");
        let st: Shared = Arc::new(AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let ed: Ed = Arc::new(Editor::with_dir(root.join("editor")));
        std::fs::create_dir_all(root.join("de_kobbl/maps")).unwrap();
        let file = root.join("de_kobbl/maps/de_kobbl.jmf");
        std::fs::write(&file, "x").unwrap();
        let mut d = doc_with(&[("env_sprite", &[("model", "sprites/de_kobbl/blin.spr"), ("origin", "0 0 64")])]);
        d.world.keys.clear();
        ed.install_from(d, "de_kobbl".into(), None, Some(&file), &[]);
        assert_eq!(ed.lock().as_ref().unwrap().doc.editor.source.as_deref(), Some(root.join("de_kobbl/maps").to_string_lossy().as_ref()));

        assert!(for_playtest_in(&st, &ed).is_ok(), "no game set up: the compile says so instead");
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike")).unwrap();
        let places = || {
            let g = ed.lock();
            Places { hl: Some(hl.clone()), game: "cstrike".into(), beside: beside_dirs(g.as_ref().unwrap()) }
        };
        let doc = || ed.lock().as_ref().unwrap().doc.clone();
        let err = check_playtest(&doc(), places()).err().expect("refused");
        assert!(err.contains("sprites/de_kobbl/blin.spr (env_sprite #1)"), "{err}");
        let get = |uri: &'static str| {
            let (st, ed) = (st.clone(), ed.clone());
            async move {
                let req = axum::http::Request::builder().uri(uri).header("host", "127.0.0.1:8027").body(axum::body::Body::empty()).unwrap();
                let res = routes_with(ed).with_state(st).oneshot(req).await.unwrap();
                serde_json::from_slice::<Value>(&axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap()
            }
        };
        let checks = get("/api/tools/editor/checks").await;
        let hit = checks["issues"].as_array().unwrap().iter().find(|i| i["code"] == "missing-file").cloned();
        // (without a game set up nothing is called missing in the live checks: it may be in the game)
        assert!(hit.is_none(), "{checks}");
        let a = get("/api/tools/editor/assets").await;
        assert_eq!(a["missing"], 1, "{a}");
        assert_eq!(a["assets"][0]["status"], "missing");

        // the sprite beside the map's folder: found, and Playtest goes ahead
        std::fs::create_dir_all(root.join("de_kobbl/sprites/de_kobbl")).unwrap();
        std::fs::write(root.join("de_kobbl/sprites/de_kobbl/blin.spr"), "spr").unwrap();
        let ok = check_playtest(&doc(), places()).expect("goes ahead");
        // after the map is in the game: the sprite in cstrike_downloads, and a .res naming it
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::write(hl.join("cstrike/maps/de_kobbl.bsp"), tiny_bsp("{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"env_sprite\"\n\"model\" \"sprites/de_kobbl/blin.spr\"\n}\n")).unwrap();
        let notes = ok.install("de_kobbl");
        assert!(hl.join("cstrike_downloads/sprites/de_kobbl/blin.spr").is_file(), "{notes:?}");
        let res = std::fs::read_to_string(hl.join("cstrike/maps/de_kobbl.res")).unwrap_or_default();
        assert!(res.contains("sprites/de_kobbl/blin.spr"), "{notes:?} {res}");
        let a = get("/api/tools/editor/assets").await;
        assert_eq!(a["assets"][0]["status"], "beside", "{a}");
        assert_eq!(a["toGame"], 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn paths_that_climb_out_are_ignored() {
        let d = doc_with(&[("env_sprite", &[("model", "../../etc/x.spr")]), ("cycler", &[("model", "C:/x/y.mdl")])]);
        assert!(named_files(&d).iter().all(|n| n.kind == "sky"));
    }
}
