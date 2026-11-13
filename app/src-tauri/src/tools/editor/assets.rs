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

/// The folders an asset can be in, and the file types each holds.
const PLACES: [(&str, &[&str]); 4] = [("models/", &["mdl"]), ("sprites/", &["spr"]), ("sound/", &["wav", "mp3"]), ("gfx/env/", &["tga", "bmp"])];

/// A clean path inside the game folder, or None: only models/, sprites/, sound/ and gfx/env/
/// with their own file types; plain names (letters, digits, _ - . space), no empty, "." or ".."
/// parts, nothing absolute (/, \, drive letters). A backslash counts as a folder separator,
/// as the game reads it.
pub fn clean_rel(v: &str) -> Option<String> {
    let v = v.replace('\\', "/");
    if v.is_empty() || v.len() > 120 || v.starts_with('/') || v.contains(':') {
        return None;
    }
    let parts: Vec<&str> = v.split('/').collect();
    if parts.len() > 8 || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.starts_with(' ') || p.ends_with(' ') || p.ends_with('.')) {
        return None;
    }
    if !v.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-. ".contains(&b) || b == b'/') {
        return None;
    }
    let low = v.to_ascii_lowercase();
    let ext = low.rsplit_once('.').map(|x| x.1).unwrap_or("");
    PLACES.iter().any(|(dir, exts)| low.starts_with(dir) && exts.contains(&ext) && low.len() > dir.len()).then_some(v)
}

/// An entity key's value as a game path: models and sprites as they are, sounds under sound/.
fn clean(v: &str) -> Option<String> {
    let v = v.trim().trim_start_matches(['*', '#']).replace('\\', "/");
    if v.starts_with('!') {
        return None;
    }
    let low = v.to_ascii_lowercase();
    let v = if (low.ends_with(".wav") || low.ends_with(".mp3")) && !low.starts_with("sound/") { format!("sound/{v}") } else { v };
    clean_rel(&v)
}

/// The biggest file of each type that's taken (bigger: not a real asset).
fn size_cap(ext: &str) -> u64 {
    match ext {
        "mdl" | "wav" | "mp3" => 32 << 20,
        _ => 8 << 20,
    }
}

/// Whether the file's first bytes are what its type says (MDL IDST/IDSQ, SPR IDSP, WAV
/// RIFF…WAVE, MP3 ID3 or a frame sync, BMP BM, TGA a plain image header).
pub fn looks_right(path: &Path, ext: &str) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut h = [0u8; 18];
    let n = f.read(&mut h).unwrap_or(0);
    let h = &h[..n];
    match ext {
        "mdl" => h.starts_with(b"IDST") || h.starts_with(b"IDSQ"),
        "spr" => h.starts_with(b"IDSP"),
        "wav" => n >= 12 && &h[..4] == b"RIFF" && &h[8..12] == b"WAVE",
        "mp3" => h.starts_with(b"ID3") || (n >= 2 && h[0] == 0xFF && h[1] & 0xE0 == 0xE0),
        "bmp" => h.starts_with(b"BM"),
        // id length, colour map type 0/1, image type 1 2 3 9 10 11, then 8/16/24/32 bits
        "tga" => n == 18 && h[1] <= 1 && matches!(h[2], 1 | 2 | 3 | 9 | 10 | 11) && matches!(h[16], 8 | 15 | 16 | 24 | 32),
        _ => false,
    }
}

/// A found file that's safe to use: a real file (not a link) whose real path stays under
/// `root`, of a sane size, and of the type its name says. Returns its real path.
pub fn vet(root: &Path, found: &Path, rel: &str) -> Option<PathBuf> {
    let md = std::fs::symlink_metadata(found).ok()?;
    if !md.file_type().is_file() {
        return None;
    }
    let real = std::fs::canonicalize(found).ok()?;
    let root = std::fs::canonicalize(root).ok()?;
    if !real.starts_with(&root) {
        return None;
    }
    let ext = rel.rsplit_once('.').map(|x| x.1.to_ascii_lowercase()).unwrap_or_default();
    (md.len() > 0 && md.len() <= size_cap(&ext) && looks_right(&real, &ext)).then_some(real)
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
            let kind = if low.starts_with("sprites/") {
                "sprite"
            } else if low.starts_with("models/") {
                "model"
            } else if low.starts_with("sound/") {
                "sound"
            } else {
                continue;
            };
            add(rel, kind, id.0, class);
        }
    }
    out
}

/// Where to look: the game (when it's set up) and the map's own folders.
pub struct Places {
    pub hl: Option<PathBuf>,
    /// "cstrike" or "czero"
    pub game: String,
    /// the project's folder, and the folder the map was imported from in this run and its
    /// parent (never a path read from a project file)
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
    let project = s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf);
    let import = s.import_dir.clone();
    let mut push = |x: PathBuf| {
        if x.is_dir() && !v.contains(&x) {
            v.push(x);
        }
    };
    if let Some(d) = project {
        push(d);
    }
    if let Some(d) = import {
        push(d.clone());
        // (a map in maps/ next to its sprites/, models/ and sound/)
        if let Some(p) = d.parent() {
            push(p.to_path_buf());
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
                    if let Some(p) = f.find(d, &rel).and_then(|p| vet(d, &p, &rel)) {
                        status = Status::Beside(p);
                        break 'look;
                    }
                }
                if let Some(hl) = &places.hl {
                    if let Found::Elsewhere { path, folder } = doctor::find_file(&f, hl, &path, &rel) {
                        if let Some(p) = vet(&hl.join(&folder), &path, &rel) {
                            status = Status::Elsewhere(p);
                            break 'look;
                        }
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
        out.push(Issue { level, code, message, brush: None, entity, texture: None, fix: None, place: None });
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
/// Copies `src` to `<base>/<rel>` unless something is there already. Ok(true) when it
/// copied. `rel` must be a clean asset path, `src` a real file of its type, and the folder it
/// lands in must really be inside `base` (no linked folders leading elsewhere).
fn copy_into(base: &Path, rel: &str, src: &Path) -> Result<bool, String> {
    let rel = clean_rel(rel).ok_or("That file's name isn't a model, sprite, sound or sky path.")?;
    let ext = rel.rsplit_once('.').map(|x| x.1.to_ascii_lowercase()).unwrap_or_default();
    let md = std::fs::symlink_metadata(src).map_err(|_| format!("{rel} isn't there any more."))?;
    if !md.file_type().is_file() || md.len() > size_cap(&ext) || !looks_right(src, &ext) {
        return Err(format!("{rel} isn't a real {ext} file, so it wasn't copied."));
    }
    let dest = base.join(&rel);
    if std::fs::symlink_metadata(&dest).is_ok() {
        return Ok(false);
    }
    let parent = dest.parent().ok_or("No folder to copy to.")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("Couldn't make {} ({e}).", parent.display()))?;
    let (real_parent, real_base) = (std::fs::canonicalize(parent).map_err(|e| e.to_string())?, std::fs::canonicalize(base).map_err(|e| e.to_string())?);
    if !real_parent.starts_with(&real_base) {
        return Err(format!("{} leads outside {}, so {rel} wasn't copied.", parent.display(), base.display()));
    }
    std::fs::copy(src, &dest).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
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
            if copy_into(&to, &rel, src)? {
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
            Status::Game { stock: false } => alternatives(&a.named.rel).iter().find_map(|r| path.iter().find_map(|d| f.find(d, r).and_then(|p| vet(d, &p, r)))),
            _ => None,
        };
        let Some(src) = src else { continue };
        if src.starts_with(project_dir) {
            continue;
        }
        let rel = src_rel(&src, &a.named.rel);
        if copy_into(project_dir, &rel, &src)? {
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

    const MDL: &[u8] = b"IDST\x0a\0\0\0model";
    const SPR: &[u8] = b"IDSP\x02\0\0\0sprite";
    const WAV: &[u8] = b"RIFF\x10\0\0\0WAVEfmt data";
    const TGA: &[u8] = &[0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 24, 0, 1, 2, 3];

    #[test]
    fn only_clean_asset_paths() {
        for ok in ["sprites/de_kobbl/blin.spr", "models/barney.mdl", "sound/ambience/wind 1.wav", "gfx/env/desertup.tga", "sprites\\x\\y.spr"] {
            assert!(clean_rel(ok).is_some(), "{ok}");
        }
        for bad in [
            "../x.spr", "sprites/../../x.spr", "sprites/./x.spr", "/etc/x.spr", "C:/x/y.mdl", "c:x.mdl", "sprites\\..\\..\\x.spr", "\\\\server\\share\\x.spr",
            "sprites//x.spr", "models/x.txt", "maps/x.spr", ".ssh/id_rsa", "models/x.mdl.", "sound/x.mdl", "sprites/x\0.spr", "sprites/\u{f1}.spr", "",
        ] {
            assert!(clean_rel(bad).is_none(), "{bad}");
        }
        // entity values: sounds get sound/, anything else must be a clean asset path
        let d = doc_with(&[("env_sprite", &[("model", "../../.ssh/id_rsa.spr")]), ("cycler", &[("model", "C:/x/y.mdl")]), ("ambient_generic", &[("message", "../../../etc/passwd.wav")]), ("env_glow", &[("model", "/Users/x/.ssh/id_rsa")])]);
        assert!(named_files(&d).iter().all(|n| n.kind == "sky"), "{:?}", named_files(&d));
    }

    #[cfg(unix)]
    #[test]
    fn links_wrong_types_and_linked_folders_are_refused() {
        use std::os::unix::fs::symlink;
        let root = tmp("sec");
        let secret = root.join("home/.ssh");
        std::fs::create_dir_all(&secret).unwrap();
        std::fs::write(secret.join("id_rsa"), b"-----BEGIN OPENSSH PRIVATE KEY-----").unwrap();
        // a sprite-looking file outside the map's folder, reached by a link
        std::fs::write(secret.join("real.spr"), SPR).unwrap();
        let src = root.join("map");
        std::fs::create_dir_all(src.join("sprites")).unwrap();
        symlink(secret.join("real.spr"), src.join("sprites/link.spr")).unwrap();
        symlink(&secret, src.join("sprites/dir")).unwrap();
        // a file named like a sprite that isn't one, and one too big
        std::fs::copy(secret.join("id_rsa"), src.join("sprites/fake.spr")).unwrap();
        std::fs::write(src.join("sprites/big.spr"), [b"IDSP".as_slice(), &vec![0u8; (8 << 20) + 1]].concat()).unwrap();
        std::fs::write(src.join("sprites/good.spr"), SPR).unwrap();
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike")).unwrap();
        let places = Places { hl: Some(hl.clone()), game: "cstrike".into(), beside: vec![src.clone()] };
        let mut d = doc_with(&[
            ("env_sprite", &[("model", "sprites/link.spr")]),
            ("env_sprite", &[("model", "sprites/dir/real.spr")]),
            ("env_sprite", &[("model", "sprites/fake.spr")]),
            ("env_sprite", &[("model", "sprites/big.spr")]),
            ("env_sprite", &[("model", "sprites/good.spr")]),
        ]);
        d.world.keys.clear();
        let a = locate(named_files(&d), &places);
        let st = |rel: &str| a.iter().find(|x| x.named.rel == rel).unwrap().status.clone();
        for bad in ["sprites/link.spr", "sprites/dir/real.spr", "sprites/fake.spr", "sprites/big.spr"] {
            assert_eq!(st(bad), Status::Missing, "{bad}");
        }
        assert!(matches!(st("sprites/good.spr"), Status::Beside(_)));
        // the download folder's sprites/ is a link to somewhere else: nothing is copied there
        std::fs::create_dir_all(hl.join("cstrike_downloads")).unwrap();
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        symlink(&elsewhere, hl.join("cstrike_downloads/sprites")).unwrap();
        assert!(copy_to_game(&a, &places).is_err());
        assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none());
        // copies only go under the folder they're for, and only real files of their type
        let proj = root.join("proj");
        assert!(copy_into(&proj, "../outside.spr", &src.join("sprites/good.spr")).is_err());
        assert!(copy_into(&proj, "sprites/x.spr", &src.join("sprites/fake.spr")).is_err());
        assert!(copy_into(&proj, "sprites/x.spr", &src.join("sprites/link.spr")).is_err());
        assert!(copy_into(&proj, "sprites/x.spr", &src.join("sprites/good.spr")).unwrap());
        assert!(!root.join("outside.spr").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A project file can't name a folder to search: only this run's own import does.
    #[test]
    fn a_project_file_cannot_add_search_folders() {
        let json = r#"{"version":1,"world":{"wads":[],"keys":[]},"brushes":{},"entities":{},"next_id":1,"editor":{"source":"/Users/someone/.ssh"}}"#;
        if let Ok(d) = serde_json::from_str::<MapDocument>(json) {
            assert_eq!(serde_json::to_value(&d.editor).unwrap().get("source"), None);
        }
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
        for (rel, what) in [("cstrike/models/barney.mdl", MDL), ("valve/sound/ambience/wind1.wav", WAV)] {
            std::fs::create_dir_all(hl.join(rel).parent().unwrap()).unwrap();
            std::fs::write(hl.join(rel), what).unwrap();
        }
        for side in SKY_SIDES {
            std::fs::create_dir_all(hl.join("valve/gfx/env")).unwrap();
            std::fs::write(hl.join(format!("valve/gfx/env/desert{side}.tga")), TGA).unwrap();
        }
        // the imported map's folder: maps/ with sprites/ beside it
        let src = root.join("de_kobbl");
        std::fs::create_dir_all(src.join("maps")).unwrap();
        std::fs::create_dir_all(src.join("sprites/de_kobbl")).unwrap();
        std::fs::write(src.join("sprites/de_kobbl/blin.spr"), SPR).unwrap();
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
        assert_eq!(ed.lock().as_ref().unwrap().import_dir.as_deref(), Some(root.join("de_kobbl/maps").as_path()));

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
        std::fs::write(root.join("de_kobbl/sprites/de_kobbl/blin.spr"), SPR).unwrap();
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
