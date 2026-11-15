//! The Content Browser's Import and Export (Parity A): your own models, sprites, sounds and
//! texture files copied into the open project's folder (models/, sprites/, sound/, a .wad beside
//! the project), where they show under Mine and go with Publish and Remix; and a file (with a
//! model's texture and sequence files) copied out to a folder you pick, or shown in Finder or
//! Explorer.
//!   POST /api/tools/editor/content/import  {files: [{name, data (base64), rename?}]}
//!   POST /api/tools/editor/content/reveal  {folder, path}
//!   POST /api/tools/editor/content/export  {folder, path}
//! Every file is checked by its real type (its first bytes), its size, a safe name (the strict
//! asset path rules, no names Windows can't hold), never over a file that's there (a free name is
//! offered instead), and never under a name the game's own files use.

use super::assets::clean_rel;
use super::{blocking, Ed};
use crate::mapcopy::Finder;
use crate::server::Shared;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// At most this many files in one import, and this much in all.
const MAX_FILES: usize = 40;
const MAX_TOTAL: usize = 160 << 20;

/// The size a file of a kind may be.
pub fn cap(ext: &str) -> usize {
    match ext {
        "mdl" | "wav" => 32 << 20,
        "wad" => 64 << 20,
        _ => 8 << 20,
    }
}

/// Whether the bytes are what the name says (MDL IDST/IDSQ, SPR IDSP, WAV RIFF…WAVE, WAD WAD3
/// with its file list inside the file).
pub fn real_type(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "mdl" => bytes.starts_with(b"IDST") || bytes.starts_with(b"IDSQ"),
        "spr" => bytes.starts_with(b"IDSP"),
        "wav" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "wad" => {
            if bytes.len() < 12 || &bytes[..4] != b"WAD3" {
                return false;
            }
            let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
            let at = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
            n <= 8192 && at.checked_add(n * 32).is_some_and(|end| end <= bytes.len())
        }
        _ => false,
    }
}

/// Where a file goes in the project folder (`models/x.mdl`...), or why it can't.
pub fn place_for(name: &str) -> Result<(String, &'static str), String> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    let low = name.to_ascii_lowercase();
    let ext = low.rsplit_once('.').map(|x| x.1).unwrap_or("");
    let ext: &'static str = match ext {
        "mdl" => "mdl",
        "spr" => "spr",
        "wav" => "wav",
        "wad" => "wad",
        _ => return Err("only models (.mdl), sprites (.spr), sounds (.wav) and texture files (.wad) can be imported".into()),
    };
    let bad = || "its name has characters the game or Windows can't use (letters, digits, _ - . only)".to_string();
    let part_ok = !name.is_empty() && !name.starts_with('.') && name.len() <= 64 && !crate::setup::record::windows_unsafe(name) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b));
    if !part_ok {
        return Err(bad());
    }
    let rel = match ext {
        "mdl" => format!("models/{name}"),
        "spr" => format!("sprites/{name}"),
        "wav" => format!("sound/{name}"),
        _ => name.to_string(),
    };
    if ext != "wad" && clean_rel(&rel).is_none() {
        return Err(bad());
    }
    Ok((rel, ext))
}

/// The game's own file by that name (a project file would hide or replace it when shared).
pub fn stock(rel: &str, ext: &str, game: &str) -> bool {
    if ext == "wad" {
        crate::tools::doctor::stock_wad_names(game).iter().any(|w| w.eq_ignore_ascii_case(rel))
    } else {
        crate::tools::doctor::is_stock(rel, game)
    }
}

/// A model's companions: `xT.mdl` (its textures) and `x01.mdl`... (sequence groups), by name.
pub fn companion_of(stem_low: &str, base_low: &str) -> bool {
    if stem_low == format!("{base_low}t") {
        return true;
    }
    let b = stem_low.as_bytes();
    stem_low.len() == base_low.len() + 2 && stem_low.starts_with(base_low) && b[b.len() - 2..].iter().all(|c| c.is_ascii_digit())
}

pub struct Incoming {
    pub name: String,
    pub bytes: Vec<u8>,
    /// take a free name if that one's taken
    pub rename: bool,
}

/// Copies the files into `project` (each whole, never over a file). The summary: imported
/// (with the path), renamed (from → to), taken (a name already used: a free one offered),
/// refused (and why).
pub fn import(project: &Path, game: &str, files: Vec<Incoming>) -> Value {
    let exists = |r: &str| Finder::default().find(project, r).is_some() || project.join(r).exists();
    import_into(project, game, files, &place_for, &exists)
}

/// The same checks and copying into any folder (shared with the asset pickers' Import, which
/// puts files in the game's custom folders, tools/assetimport.rs): `place` maps a file name to
/// its path under `root` (or why not), `exists` says whether a path is taken anywhere it'd be
/// seen (the project, or every folder the game reads).
pub fn import_into(root: &Path, game: &str, files: Vec<Incoming>, place: &dyn Fn(&str) -> Result<(String, &'static str), String>, exists: &dyn Fn(&str) -> bool) -> Value {
    let project = root;
    let mut imported = Vec::new();
    let mut renamed = Vec::new();
    let mut taken = Vec::new();
    let mut refused = Vec::new();
    // a model and its companions are renamed together (the model names its T file)
    let models: Vec<String> = files.iter().filter_map(|f| f.name.to_ascii_lowercase().strip_suffix(".mdl").map(str::to_string)).collect();
    let base_of = |stem: &str| models.iter().find(|m| m.as_str() != stem && companion_of(stem, m)).cloned();
    let mut new_stems: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut order: Vec<&Incoming> = files.iter().collect();
    // (the models first, so their companions follow a rename)
    order.sort_by_key(|f| base_of(&f.name.to_ascii_lowercase().trim_end_matches(".mdl").to_string()).is_some());
    for f in order {
        let (rel, ext) = match place(&f.name) {
            Ok(x) => x,
            Err(why) => {
                refused.push(json!({"name": f.name, "why": why}));
                continue;
            }
        };
        if f.bytes.is_empty() || f.bytes.len() > cap(ext) {
            refused.push(json!({"name": f.name, "why": format!("it's empty or bigger than {} MB", cap(ext) >> 20)}));
            continue;
        }
        if !real_type(&f.bytes, ext) {
            refused.push(json!({"name": f.name, "why": format!("it isn't really a .{ext} file (its first bytes say otherwise)")}));
            continue;
        }
        if stock(&rel, ext, game) {
            refused.push(json!({"name": f.name, "why": "the game has its own file by that name: rename yours first".to_string()}));
            continue;
        }
        let file = rel.rsplit('/').next().unwrap_or(&rel).to_string();
        let dir_rel = rel[..rel.len() - file.len()].to_string();
        let stem_low = file.to_ascii_lowercase().trim_end_matches(&format!(".{ext}")).to_string();
        // a companion whose model was renamed follows it
        let mut want = rel.clone();
        if let Some(base) = base_of(&stem_low) {
            if let Some(nb) = new_stems.get(&base) {
                let tail = &file[base.len()..];
                want = format!("{dir_rel}{nb}{tail}");
            }
        }
        if exists(&want) {
            if !f.rename {
                taken.push(json!({"name": f.name, "path": want, "suggest": free_name(project, &want, exists)}));
                continue;
            }
            want = free_name(project, &want, exists);
        }
        let dest = project.join(&want);
        let r = (|| {
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d)?;
            }
            crate::sys::write_atomic(&dest, &f.bytes)
        })();
        match r {
            Err(e) => refused.push(json!({"name": f.name, "why": format!("it couldn't be saved ({e})")})),
            Ok(()) => {
                if want != rel {
                    renamed.push(json!({"from": rel, "to": want}));
                    if ext == "mdl" {
                        let new_file = want.rsplit('/').next().unwrap_or(&want).to_ascii_lowercase();
                        new_stems.insert(stem_low.clone(), new_file.trim_end_matches(".mdl").to_string());
                    }
                }
                imported.push(json!({"name": f.name, "path": want}));
            }
        }
    }
    let n = imported.len();
    let mut msg = vec![format!("Imported {n} file{}.", if n == 1 { "" } else { "s" })];
    if !renamed.is_empty() {
        msg.push(format!("{} under a new name.", renamed.len()));
    }
    if !taken.is_empty() {
        msg.push(format!("{} already there.", taken.len()));
    }
    if !refused.is_empty() {
        msg.push(format!("{} refused.", refused.len()));
    }
    json!({"imported": imported, "renamed": renamed, "taken": taken, "refused": refused, "message": msg.join(" ")})
}

/// `name_2.ext`, `name_3.ext`... the first that isn't there.
pub fn free_name(project: &Path, rel: &str, exists: &dyn Fn(&str) -> bool) -> String {
    let _ = project;
    let (stem, ext) = rel.rsplit_once('.').unwrap_or((rel, ""));
    for i in 2..1000 {
        let r = format!("{stem}_{i}.{ext}");
        if !exists(&r) {
            return r;
        }
    }
    format!("{stem}_new.{ext}")
}

/// A file and, for a model, its companions beside it (`xT.mdl`, `x01.mdl`...): their paths.
pub fn with_companions(file: &Path) -> Vec<PathBuf> {
    let mut out = vec![file.to_path_buf()];
    let low = file.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if let (Some(base), Some(dir)) = (low.strip_suffix(".mdl"), file.parent()) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if let Some(stem) = n.strip_suffix(".mdl") {
                if stem != base && companion_of(stem, base) && e.path().is_file() {
                    out.push(e.path());
                }
            }
        }
        out[1..].sort();
    }
    out
}

/// Copies a file (and a model's companions) into `dest`, never over a file there. Their names.
pub fn export(file: &Path, dest: &Path) -> Result<Vec<String>, String> {
    let files = with_companions(file);
    for f in &files {
        let name = f.file_name().ok_or("That file can't be copied.")?;
        if dest.join(name).exists() {
            return Err(format!("{} is in that folder already. Pick another folder.", name.to_string_lossy()));
        }
    }
    let mut out = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap();
        std::fs::copy(f, dest.join(name)).map_err(|e| format!("{} couldn't be copied ({e}).", name.to_string_lossy()))?;
        out.push(name.to_string_lossy().to_string());
    }
    Ok(out)
}

#[derive(Deserialize)]
struct InFile {
    name: String,
    data: String,
    #[serde(default)]
    rename: bool,
}

#[derive(Deserialize)]
struct ImportReq {
    files: Vec<InFile>,
}

fn project_and_game(st: &Shared, ed: &Ed) -> Result<(Option<PathBuf>, String), String> {
    let g = st.local_game().ok_or("Set up the game on this computer first.")?;
    let game = if g.is_czero() { "czero" } else { "cstrike" }.to_string();
    let project = ed.lock().as_ref().and_then(|s| s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf));
    Ok((project, game))
}

async fn api_import(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<ImportReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    blocking(move || {
        let (project, game) = project_and_game(&st, &ed)?;
        let Some(project) = project else {
            return Ok(json!({"needProject": true, "error": "Save the project first: imported files go in its folder. Or put them in the game with Tools › Add your own."}));
        };
        if req.files.len() > MAX_FILES {
            return Err(format!("Import at most {MAX_FILES} files at once."));
        }
        let mut total = 0usize;
        let mut files = Vec::new();
        for f in req.files {
            let bytes = base64::engine::general_purpose::STANDARD.decode(f.data.as_bytes()).map_err(|_| format!("{} couldn't be read.", f.name))?;
            total += bytes.len();
            if total > MAX_TOTAL {
                return Err(format!("That's more than {} MB in one go. Import fewer files at once.", MAX_TOTAL >> 20));
            }
            files.push(Incoming { name: f.name, bytes, rename: f.rename });
        }
        Ok(import(&project, &game, files))
    })
    .await
}

#[derive(Deserialize)]
struct FileReq {
    folder: String,
    path: String,
}

/// The file a request names, in the game's folders or the project's (the browser's own checks).
fn resolve(st: &Shared, ed: &Ed, folder: &str, rel: &str) -> Result<PathBuf, String> {
    let g = st.local_game().ok_or("Set up the game on this computer first.")?;
    let game = if g.is_czero() { "czero" } else { "cstrike" };
    let (project, _) = project_and_game(st, ed)?;
    super::browser::resolve_file(&g.hl, game, project.as_deref(), folder, rel).ok_or_else(|| "That file isn't there any more.".to_string())
}

async fn api_reveal(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    blocking(move || {
        let p = resolve(&st, &ed, &req.folder, &req.path)?;
        super::workspace::reveal_path(&p)?;
        Ok(json!({"ok": true}))
    })
    .await
}

async fn api_export(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<FileReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    blocking(move || {
        let p = resolve(&st, &ed, &req.folder, &req.path)?;
        let Some(dest) = super::pick_folder(&st, "Export to which folder?")? else { return Ok(json!({"cancelled": true})) };
        let names = export(&p, &dest)?;
        Ok(json!({"ok": true, "files": names, "folder": dest.to_string_lossy()}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/content/import", post(api_import).layer(axum::extract::DefaultBodyLimit::max(MAX_TOTAL / 3 * 4 + (1 << 20))))
        .route("/api/tools/editor/content/reveal", post(api_reveal))
        .route("/api/tools/editor/content/export", post(api_export))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wad() -> Vec<u8> {
        let mut b = b"WAD3".to_vec();
        b.extend(0u32.to_le_bytes());
        b.extend(12u32.to_le_bytes());
        b
    }

    #[test]
    fn checks_type_name_size_and_stock() {
        assert!(real_type(b"IDST....", "mdl") && real_type(b"IDSQ", "mdl") && !real_type(b"IDSP", "mdl"));
        assert!(real_type(b"IDSP", "spr") && !real_type(b"PK\x03\x04", "spr"));
        assert!(real_type(b"RIFF\0\0\0\0WAVEfmt ", "wav") && !real_type(b"ID3", "wav"));
        assert!(real_type(&wad(), "wad"));
        let mut lying = wad();
        lying[4] = 5; // five textures listed, none there
        assert!(!real_type(&lying, "wad"));
        assert_eq!(place_for("Barrel.mdl").unwrap(), ("models/Barrel.mdl".into(), "mdl"));
        assert_eq!(place_for("C:\\stuff\\glow.spr").unwrap().0, "sprites/glow.spr");
        assert_eq!(place_for("alarm.wav").unwrap().0, "sound/alarm.wav");
        assert_eq!(place_for("mine.wad").unwrap().0, "mine.wad");
        assert!(place_for("con.mdl").is_err(), "a name Windows can't hold");
        assert!(place_for("café.mdl").is_err() && place_for("a b.mdl").is_err() && place_for("x.exe").is_err() && place_for("..mdl").is_err());
    }

    #[test]
    fn imports_into_the_project_never_over_a_file_with_companions_together() {
        let root = std::env::temp_dir().join(format!("ls-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("models")).unwrap();
        std::fs::write(root.join("models/crate.mdl"), b"IDST old").unwrap();
        let f = |name: &str, bytes: &[u8], rename: bool| Incoming { name: name.into(), bytes: bytes.to_vec(), rename };
        let r = import(&root, "cstrike", vec![
            f("glow.spr", b"IDSP new", false),
            f("alarm.wav", b"RIFF\0\0\0\0WAVEfmt ", false),
            f("mine.wad", &wad(), false),
            f("fake.mdl", b"PK\x03\x04", false),
            f("v_ak47.mdl", b"IDST mine", false),
            f("crate.mdl", b"IDST new", false),
            f("crateT.mdl", b"IDST tex", false),
            f("crate01.mdl", b"IDSQ seq", false),
        ]);
        assert!(root.join("sprites/glow.spr").is_file() && root.join("sound/alarm.wav").is_file() && root.join("mine.wad").is_file());
        let why = |n: &str| r["refused"].as_array().unwrap().iter().find(|x| x["name"] == n).map(|x| x["why"].as_str().unwrap().to_string());
        assert!(why("fake.mdl").unwrap().contains("isn't really a .mdl"));
        assert!(why("v_ak47.mdl").unwrap().contains("the game has its own file"));
        // crate.mdl is taken: nothing written over, a free name offered
        assert_eq!(std::fs::read(root.join("models/crate.mdl")).unwrap(), b"IDST old");
        assert_eq!(r["taken"][0]["suggest"], "models/crate_2.mdl");
        // again, taking the new name: its T and sequence files follow it
        let r = import(&root, "cstrike", vec![f("crate.mdl", b"IDST new", true), f("crateT.mdl", b"IDST tex", true), f("crate01.mdl", b"IDSQ seq", true)]);
        assert_eq!(r["refused"].as_array().unwrap().len(), 0, "{r}");
        assert_eq!(std::fs::read(root.join("models/crate_2.mdl")).unwrap(), b"IDST new");
        assert_eq!(std::fs::read(root.join("models/crate_2T.mdl")).unwrap(), b"IDST tex");
        assert_eq!(std::fs::read(root.join("models/crate_201.mdl")).unwrap(), b"IDSQ seq");
        assert_eq!(std::fs::read(root.join("models/crate.mdl")).unwrap(), b"IDST old");
        // export: the model with its companions, never over a file
        let out = root.join("out");
        std::fs::create_dir_all(&out).unwrap();
        assert_eq!(export(&root.join("models/crate_2.mdl"), &out).unwrap(), ["crate_2.mdl", "crate_201.mdl", "crate_2T.mdl"]);
        assert!(export(&root.join("models/crate_2.mdl"), &out).unwrap_err().contains("already"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
