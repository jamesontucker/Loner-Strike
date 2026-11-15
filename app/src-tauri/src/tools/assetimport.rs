//! The asset pickers' Import (Creature Studio's model and sound pickers, the Studio's sprite,
//! model and sound inputs, the editor's pickers for game-level things): a model, sprite or sound
//! from this computer, or from one of your map projects, copied into the game's own custom
//! folders so creatures and modes can use it at once (no Playtest needed).
//!   POST /api/tools/assets/import   {kind, name, data (base64), files?: [{name, data}], rename?}
//!                                   {kind, pick: true, rename?}  (the system file picker)
//!   GET  /api/tools/assets/project-files?kind=model|sprite|sound
//!   POST /api/tools/assets/adopt    {kind, project, path, rename?}
//! Answers: {ok, path, files, credit?} | {cancelled} | {taken: {name, suggestion}} | {error}.
//! The checks are the Content Browser's Import (editor/importexport.rs: the real type by its
//! first bytes, size, a safe name, a model's T and sequence files following it), shared, not
//! copied. Files go to models/custom/, sprites/custom/ and sound/custom/ in the game folder (not
//! the game's own places, so the app, the doctor and sharing treat them as custom); never over a
//! game file or any file already there in any folder the game reads.

use super::editor::importexport::{cap, companion_of, free_name, import_into, place_for, real_type, stock, with_companions, Incoming};
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// At most this much in one import (a model with its T and sequence files).
const MAX_TOTAL: usize = 96 << 20;
/// At most this many files a kind listed from your map projects.
const MAX_LISTED: usize = 500;

/// The extension and custom folder for a picker's kind.
fn kind_spec(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "model" => Some(("mdl", "models/custom/")),
        "sprite" => Some(("spr", "sprites/custom/")),
        "sound" => Some(("wav", "sound/custom/")),
        _ => None,
    }
}

fn kind_words(kind: &str) -> &'static str {
    match kind {
        "model" => "a model (.mdl)",
        "sprite" => "a sprite (.spr)",
        _ => "a sound (.wav)",
    }
}

/// Where a file of this kind goes in the game folder (`models/custom/x.mdl`...), or why it can't:
/// the Content Browser's name rules, then the kind's own folder.
pub fn game_place(kind: &str, name: &str) -> Result<(String, &'static str), String> {
    let (want_ext, dir) = kind_spec(kind).ok_or("That isn't a kind of file the pickers take.")?;
    let (rel, ext) = place_for(name)?;
    if ext != want_ext {
        return Err(format!("This picker takes {}.", kind_words(kind)));
    }
    let file = rel.rsplit('/').next().unwrap_or(&rel);
    Ok((format!("{dir}{file}"), ext))
}

/// Whether a game path is taken in any folder the game reads (its own, its downloads, CS for
/// CZ, Half-Life's), in any letter case.
pub fn taken_in_game(hl: &Path, game: &str, rel: &str) -> bool {
    let f = crate::mapcopy::Finder::default();
    super::doctor::game_path(hl, game).iter().any(|d| f.find(d, rel).is_some() || d.join(rel).exists())
}

/// A file name with a new stem, for the main file and, for a model, its companions
/// (`old.mdl` → `new.mdl`, `oldT.mdl` → `newT.mdl`, `old01.mdl` → `new01.mdl`).
fn renamed(name: &str, main_stem: &str, new_stem: &str) -> String {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    if stem.eq_ignore_ascii_case(main_stem) {
        return format!("{new_stem}.{ext}");
    }
    if stem.len() > main_stem.len() && stem[..main_stem.len()].eq_ignore_ascii_case(main_stem) {
        return format!("{new_stem}{}.{ext}", &stem[main_stem.len()..]);
    }
    name.to_string()
}

/// The pickers' Import into the game: the first file is the one picked, the rest a model's T
/// and sequence files (kept only when they really are its companions). Nothing is written when
/// any of them is taken: the answer offers a free name for the whole set instead.
pub fn import_to_game(hl: &Path, game: &str, kind: &str, mut files: Vec<(String, Vec<u8>)>, rename: Option<&str>) -> Result<Value, String> {
    if files.is_empty() {
        return Err("Nothing to import.".into());
    }
    let main_name = files[0].0.rsplit(['/', '\\']).next().unwrap_or("").to_string();
    let (_, ext) = game_place(kind, &main_name)?;
    let main_stem = main_name[..main_name.len() - ext.len() - 1].to_string();
    // companions: only a model's own (xT.mdl, x01.mdl)
    let base_low = main_stem.to_ascii_lowercase();
    files = files
        .into_iter()
        .enumerate()
        .filter(|(i, (n, _))| {
            let n = n.rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase();
            *i == 0 || (ext == "mdl" && n.strip_suffix(".mdl").is_some_and(|s| companion_of(s, &base_low)))
        })
        .map(|(_, f)| f)
        .collect();
    let mut stem = main_stem.clone();
    if let Some(r) = rename.map(str::trim).filter(|r| !r.is_empty()) {
        let r = r.rsplit(['/', '\\']).next().unwrap_or("");
        let r = r.strip_suffix(&format!(".{ext}")).or_else(|| r.strip_suffix(&format!(".{}", ext.to_ascii_uppercase()))).unwrap_or(r);
        stem = r.to_string();
    }
    let names: Vec<String> = files.iter().map(|(n, _)| renamed(n.rsplit(['/', '\\']).next().unwrap_or(""), &main_stem, &stem)).collect();
    // every file checked before anything is written
    let mut targets = Vec::new();
    for (n, (_, bytes)) in names.iter().zip(&files) {
        let (rel, e) = game_place(kind, n)?;
        if bytes.is_empty() || bytes.len() > cap(e) {
            return Err(format!("{n} is empty or bigger than {} MB.", cap(e) >> 20));
        }
        if !real_type(bytes, e) {
            return Err(format!("{n} isn't really a .{e} file (its first bytes say otherwise)."));
        }
        if stock(&rel, e, game) || stock(&format!("{}/{n}", rel.split('/').next().unwrap_or("")), e, game) {
            return Err(format!("The game has its own file called {n}. Pick another name."));
        }
        targets.push(rel);
    }
    let exists = |r: &str| taken_in_game(hl, game, r);
    if targets.iter().any(|t| exists(t)) {
        // a free name for the set: the main file's _2, _3... with its companions free too
        let main_rel = &targets[0];
        let mut suggestion = String::new();
        for i in 2..1000 {
            let s = format!("{stem}_{i}");
            if names.iter().all(|n| game_place(kind, &renamed(n, &stem, &s)).is_ok_and(|(r, _)| !exists(&r))) {
                suggestion = format!("{s}.{ext}");
                break;
            }
        }
        if suggestion.is_empty() {
            suggestion = free_name(Path::new(""), main_rel, &exists).rsplit('/').next().unwrap_or("").to_string();
        }
        return Ok(json!({"taken": {"name": format!("{stem}.{ext}"), "suggestion": suggestion}}));
    }
    let game_dir = hl.join(game);
    let incoming: Vec<Incoming> = names.iter().zip(files).map(|(n, (_, bytes))| Incoming { name: n.clone(), bytes, rename: false }).collect();
    let place = |n: &str| game_place(kind, n);
    let r = import_into(&game_dir, game, incoming, &place, &exists);
    if let Some(why) = r["refused"][0]["why"].as_str() {
        return Err(format!("{}: {why}.", r["refused"][0]["name"].as_str().unwrap_or("That file")));
    }
    if r["taken"].as_array().is_some_and(|t| !t.is_empty()) {
        return Ok(json!({"taken": {"name": format!("{stem}.{ext}"), "suggestion": r["taken"][0]["suggest"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("")}}));
    }
    let written: Vec<String> = r["imported"].as_array().into_iter().flatten().filter_map(|x| x["path"].as_str().map(String::from)).collect();
    let path = targets[0].clone();
    // (sounds are named without sound/ in creatures and modes)
    let use_as = if kind == "sound" { path.trim_start_matches("sound/").to_string() } else { path.clone() };
    let mut out = json!({"ok": true, "path": path, "use": use_as, "files": written, "message": format!("Added {} to your game.", path.rsplit('/').next().unwrap_or(&path))});
    if let Some(c) = credit_of(hl, game, &path) {
        out["credit"] = c;
    }
    Ok(out)
}

/// Whose work a file in the game is, when that's known (a GameBanana download, a Community
/// item, a note the player made): the credits check's own answer.
fn credit_of(hl: &Path, game: &str, rel: &str) -> Option<Value> {
    let p = super::doctor::game_path(hl, game).iter().find_map(|d| crate::mapcopy::Finder::default().find(d, rel))?;
    let ctx = super::credits::Ctx::load(&crate::sys::app_dir());
    let sha = super::credits::sha256_file(&p);
    let a = super::credits::resolve(rel, &sha, false, &ctx);
    if a.source.is_empty() || a.source == "unknown" {
        return None;
    }
    serde_json::to_value(&a).ok()
}

/// The models, sprites or sounds in your recent map projects' folders (models/, sprites/,
/// sound/ beside each project file), newest project first.
pub fn project_files(projects: &[PathBuf], kind: &str) -> Vec<Value> {
    let Some((ext, _)) = kind_spec(kind) else { return vec![] };
    let top = match kind {
        "model" => "models",
        "sprite" => "sprites",
        _ => "sound",
    };
    let mut out = Vec::new();
    for proj in projects {
        let Some(folder) = proj.parent() else { continue };
        let root = folder.join(top);
        if !root.is_dir() {
            continue;
        }
        let label = proj.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        for f in crate::archive::walk(&root).into_iter().take(4000) {
            if out.len() >= MAX_LISTED {
                return out;
            }
            let Ok(meta) = std::fs::symlink_metadata(&f) else { continue };
            let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if !meta.is_file() || !name.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
                continue;
            }
            // a model's T and sequence files come with it, not as picks of their own
            if ext == "mdl" {
                let stem = name.to_ascii_lowercase().trim_end_matches(".mdl").to_string();
                let is_companion = std::fs::read_dir(f.parent().unwrap_or(&root)).into_iter().flatten().flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                    n.strip_suffix(".mdl").is_some_and(|b| b != stem && companion_of(&stem, b))
                });
                if is_companion {
                    continue;
                }
            }
            let Ok(rel) = f.strip_prefix(folder) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/");
            out.push(json!({"project": proj.to_string_lossy(), "projectName": label, "path": rel, "name": name, "size": meta.len()}));
        }
    }
    out
}

/// A project file (and a model's companions) into the game, the same checked way. The project
/// must be one of `projects` (the editor's recent list) and the file inside its folder.
pub fn adopt(hl: &Path, game: &str, projects: &[PathBuf], kind: &str, project: &str, rel: &str, rename: Option<&str>) -> Result<Value, String> {
    let proj = projects.iter().find(|p| p.to_string_lossy() == project).ok_or("That project isn't in your recent map projects any more.")?;
    let folder = proj.parent().ok_or("That project's folder can't be read.")?;
    let rel = rel.replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|p| p == ".." || p.is_empty() || p.contains(':')) {
        return Err("That file isn't in the project's folder.".into());
    }
    let file = folder.join(&rel);
    let (Ok(real), Ok(real_folder)) = (file.canonicalize(), folder.canonicalize()) else { return Err("That file isn't there any more.".into()) };
    if !real.starts_with(&real_folder) || !std::fs::symlink_metadata(&file).is_ok_and(|m| m.is_file()) {
        return Err("That file isn't in the project's folder.".into());
    }
    let mut files = Vec::new();
    for f in with_companions(&file) {
        let n = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let len = std::fs::metadata(&f).map(|m| m.len() as usize).unwrap_or(usize::MAX);
        if len > MAX_TOTAL {
            return Err(format!("{n} is too big to copy."));
        }
        files.push((n, std::fs::read(&f).map_err(|e| format!("Couldn't read the file ({e})."))?));
    }
    import_to_game(hl, game, kind, files, rename)
}

// ---- routes ----------------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => answer(r),
        Err(_) => answer(Err("Something went wrong. Try again.".into())),
    }
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

/// The game the pickers put files in: the selected one, on this computer, never the real game
/// in a test build.
fn target(st: &Shared) -> Result<(PathBuf, String), String> {
    let g = st.local_game().ok_or("Set up the game on this computer first.")?;
    if g.remote.is_some() {
        return Err("This server is on another computer: add files to it there.".into());
    }
    super::studio::guard_real_game(&g.dir)?;
    Ok((g.hl.clone(), g.name.clone()))
}

#[derive(Deserialize)]
struct InFile {
    name: String,
    data: String,
}

#[derive(Deserialize)]
struct ImportReq {
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    data: String,
    #[serde(default)]
    files: Vec<InFile>,
    #[serde(default)]
    pick: bool,
    /// a new file name (after a "taken" answer), or true for the offered one
    #[serde(default)]
    rename: Option<Value>,
    /// a sound's conversion (WAV, MP3, Ogg or FLAC in; tools/soundconv.rs)
    #[serde(default)]
    sound: super::soundconv::SoundOpts,
}

/// `rename` as a name: a string, or (true) the offered free name.
fn rename_of(r: &Option<Value>, offered: Option<&str>) -> Option<String> {
    match r {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Bool(true)) => offered.map(String::from),
        _ => None,
    }
}

fn decode(name: &str, data: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|_| format!("{name} couldn't be read."))
}

/// An import that takes the offered name when asked to (rename: true). A sound is converted
/// first (an MP3, Ogg or FLAC, or a WAV the engine doesn't play well, becomes a WAV), and the
/// answer says what changed.
fn import_with(hl: &Path, game: &str, kind: &str, files: Vec<(String, Vec<u8>)>, rename: &Option<Value>, sound: super::soundconv::SoundOpts) -> Result<Value, String> {
    let (files, converted) = convert_sound(kind, files, sound)?;
    let mut r = import_plain(hl, game, kind, files, rename)?;
    if let (Some(c), true) = (converted, r["ok"] == json!(true)) {
        let msg = r["message"].as_str().unwrap_or("").trim_end_matches('.').to_string();
        r["message"] = json!(format!("{msg}: {c}."));
        r["converted"] = json!(c);
    }
    Ok(r)
}

/// A picker's sound converted for the game (its name becomes `<stem>.wav`); anything else as it is.
pub fn convert_sound(kind: &str, mut files: Vec<(String, Vec<u8>)>, sound: super::soundconv::SoundOpts) -> Result<(Vec<(String, Vec<u8>)>, Option<String>), String> {
    if kind != "sound" || files.is_empty() {
        return Ok((files, None));
    }
    let (name, bytes) = files.remove(0);
    let p = super::soundconv::prepare(&name, bytes, sound)?;
    files.insert(0, (p.name, p.bytes));
    Ok((files, p.summary))
}

fn import_plain(hl: &Path, game: &str, kind: &str, files: Vec<(String, Vec<u8>)>, rename: &Option<Value>) -> Result<Value, String> {
    if matches!(rename, Some(Value::Bool(true))) {
        let first = import_to_game(hl, game, kind, files.clone(), None)?;
        if let Some(s) = first["taken"]["suggestion"].as_str() {
            return import_to_game(hl, game, kind, files, Some(s));
        }
        return Ok(first);
    }
    import_to_game(hl, game, kind, files, rename_of(rename, None).as_deref())
}

async fn api_import(State(st): State<Shared>, h: HeaderMap, Json(req): Json<ImportReq>) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (hl, game) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    let Some((ext, _)) = kind_spec(&req.kind) else { return answer(Err("That isn't a kind of file the pickers take.".into())) };
    if req.pick {
        let Some(app) = st.app.get().cloned() else { return answer(Err("The file picker isn't available.".into())) };
        return blocking(move || {
            let title = format!("Add {} to your game", kind_words(&req.kind));
            let exts: &[&str] = if req.kind == "sound" { &["wav", "mp3", "ogg", "flac"] } else { &[ext] };
            let Some(picked) = app.dialog().file().set_title(&title).add_filter(kind_words(&req.kind), exts).blocking_pick_file() else {
                return Ok(json!({"cancelled": true}));
            };
            let p = picked.into_path().map_err(|e| e.to_string())?;
            let mut files = Vec::new();
            let mut total = 0usize;
            for f in with_companions(&p) {
                let n = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                total += std::fs::metadata(&f).map(|m| m.len() as usize).unwrap_or(usize::MAX);
                if total > MAX_TOTAL {
                    return Err(format!("That's more than {} MB.", MAX_TOTAL >> 20));
                }
                files.push((n, std::fs::read(&f).map_err(|e| format!("Couldn't read the file ({e})."))?));
            }
            import_with(&hl, &game, &req.kind, files, &req.rename, req.sound)
        })
        .await;
    }
    blocking(move || {
        let mut files = vec![(req.name.clone(), decode(&req.name, &req.data)?)];
        if req.files.len() > 16 {
            return Err("Too many files at once.".into());
        }
        for f in &req.files {
            files.push((f.name.clone(), decode(&f.name, &f.data)?));
        }
        if files.iter().map(|f| f.1.len()).sum::<usize>() > MAX_TOTAL {
            return Err(format!("That's more than {} MB.", MAX_TOTAL >> 20));
        }
        import_with(&hl, &game, &req.kind, files, &req.rename, req.sound)
    })
    .await
}

#[derive(Deserialize)]
struct KindQ {
    kind: String,
}

async fn api_project_files(Query(q): Query<KindQ>) -> Response {
    if kind_spec(&q.kind).is_none() {
        return answer(Err("That isn't a kind of file the pickers take.".into()));
    }
    blocking(move || Ok(json!({"files": project_files(&super::editor::recent_projects(), &q.kind)}))).await
}

#[derive(Deserialize)]
struct AdoptReq {
    kind: String,
    project: String,
    path: String,
    #[serde(default)]
    rename: Option<Value>,
}

async fn api_adopt(State(st): State<Shared>, h: HeaderMap, Json(req): Json<AdoptReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (hl, game) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let projects = super::editor::recent_projects();
        if matches!(req.rename, Some(Value::Bool(true))) {
            let first = adopt(&hl, &game, &projects, &req.kind, &req.project, &req.path, None)?;
            if let Some(s) = first["taken"]["suggestion"].as_str() {
                return adopt(&hl, &game, &projects, &req.kind, &req.project, &req.path, Some(s));
            }
            return Ok(first);
        }
        adopt(&hl, &game, &projects, &req.kind, &req.project, &req.path, rename_of(&req.rename, None).as_deref())
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/assets/import", post(api_import).layer(axum::extract::DefaultBodyLimit::max(MAX_TOTAL / 3 * 4 + (1 << 20))))
        .route("/api/tools/assets/project-files", get(api_project_files))
        .route("/api/tools/assets/adopt", post(api_adopt))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MDL: &[u8] = b"IDST a model";
    const WAV: &[u8] = b"RIFF\0\0\0\0WAVEfmt ";

    fn game(tag: &str) -> PathBuf {
        let hl = std::env::temp_dir().join(format!("ls-assetimport-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&hl);
        std::fs::create_dir_all(hl.join("cstrike/models")).unwrap();
        std::fs::create_dir_all(hl.join("valve/models")).unwrap();
        hl
    }

    /// An MP3 picked for a sound becomes a WAV in sound/custom/, and the answer says what changed;
    /// a WAV taken again under the offered name is converted the same way.
    #[test]
    fn a_picked_mp3_becomes_a_wav_and_says_so() {
        let hl = game("snd");
        let mp3 = include_bytes!("testdata/sound/tone.mp3").to_vec();
        let r = import_with(&hl, "cstrike", "sound", vec![("Tone.mp3".into(), mp3.clone())], &None, Default::default()).unwrap();
        assert_eq!((r["path"].as_str(), r["use"].as_str()), (Some("sound/custom/Tone.wav"), Some("custom/Tone.wav")), "{r}");
        assert_eq!(r["converted"], "converted from 44.1 kHz stereo MP3 to 22 kHz mono WAV");
        assert!(r["message"].as_str().unwrap().contains("converted from 44.1 kHz stereo MP3"), "{r}");
        let w = super::super::soundconv::parse_wav(&std::fs::read(hl.join("cstrike/sound/custom/Tone.wav")).unwrap()).unwrap();
        assert_eq!((w.format, w.channels, w.rate, w.bits), (1, 1, 22050, 16));
        // taken: nothing written, the offered name works with the same conversion
        let again = import_with(&hl, "cstrike", "sound", vec![("Tone.mp3".into(), mp3.clone())], &None, Default::default()).unwrap();
        assert_eq!(again["taken"]["name"], "Tone.wav");
        let opts = super::super::soundconv::SoundOpts { rate: 11025, bits: 8, ..Default::default() };
        let r = import_with(&hl, "cstrike", "sound", vec![("Tone.mp3".into(), mp3)], &Some(Value::Bool(true)), opts).unwrap();
        assert_eq!(r["path"], "sound/custom/Tone_2.wav", "{r}");
        assert_eq!(r["converted"], "converted from 44.1 kHz stereo MP3 to 11 kHz mono WAV, 8-bit");
        // not a sound at all
        assert!(import_with(&hl, "cstrike", "sound", vec![("x.ogg".into(), b"text".to_vec())], &None, Default::default()).unwrap_err().contains("couldn't be read as Ogg"));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn imports_into_the_games_custom_folders_with_the_content_browsers_checks() {
        let hl = game("imp");
        // a model with its T and sequence files: all into models/custom/, the T file following
        let r = import_to_game(&hl, "cstrike", "model", vec![("Crawler.mdl".into(), MDL.to_vec()), ("CrawlerT.mdl".into(), b"IDST tex".to_vec()), ("Crawler01.mdl".into(), b"IDSQ seq".to_vec()), ("other.mdl".into(), MDL.to_vec())], None).unwrap();
        assert_eq!(r["path"], "models/custom/Crawler.mdl", "{r}");
        assert_eq!(r["files"].as_array().unwrap().len(), 3, "a file that isn't its companion is left out: {r}");
        assert!(hl.join("cstrike/models/custom/CrawlerT.mdl").is_file() && hl.join("cstrike/models/custom/Crawler01.mdl").is_file());
        // a sound: used without sound/
        let r = import_to_game(&hl, "cstrike", "sound", vec![("growl.wav".into(), WAV.to_vec())], None).unwrap();
        assert_eq!((r["path"].as_str(), r["use"].as_str()), (Some("sound/custom/growl.wav"), Some("custom/growl.wav")));
        // the checks: the real type, the kind, a safe name, the game's own names
        assert!(import_to_game(&hl, "cstrike", "model", vec![("fake.mdl".into(), b"PK\x03\x04".to_vec())], None).unwrap_err().contains("isn't really a .mdl"));
        assert!(import_to_game(&hl, "cstrike", "sprite", vec![("x.mdl".into(), MDL.to_vec())], None).unwrap_err().contains("takes a sprite"));
        assert!(import_to_game(&hl, "cstrike", "model", vec![("con.mdl".into(), MDL.to_vec())], None).is_err());
        assert!(import_to_game(&hl, "cstrike", "model", vec![("../../evil.mdl".into(), MDL.to_vec())], None).is_ok_and(|v| v["path"] == "models/custom/evil.mdl"), "only the file's own name counts");
        assert!(import_to_game(&hl, "cstrike", "model", vec![("a b.mdl".into(), MDL.to_vec())], None).is_err());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn a_taken_name_writes_nothing_and_a_new_name_takes_the_companions_along() {
        let hl = game("taken");
        std::fs::create_dir_all(hl.join("valve/models/custom")).unwrap();
        // in another folder the game reads, in other letter case: still taken
        std::fs::write(hl.join("valve/models/custom/CRAWLER.mdl"), b"IDST theirs").unwrap();
        let set = || vec![("crawler.mdl".to_string(), MDL.to_vec()), ("crawlerT.mdl".to_string(), b"IDST tex".to_vec())];
        let r = import_to_game(&hl, "cstrike", "model", set(), None).unwrap();
        assert_eq!(r["taken"]["name"], "crawler.mdl");
        assert_eq!(r["taken"]["suggestion"], "crawler_2.mdl");
        assert!(!hl.join("cstrike/models/custom").exists(), "nothing written, not even the T file");
        assert_eq!(std::fs::read(hl.join("valve/models/custom/CRAWLER.mdl")).unwrap(), b"IDST theirs");
        // a name of the player's own: the T file follows
        let r = import_to_game(&hl, "cstrike", "model", set(), Some("crawler_big")).unwrap();
        assert_eq!(r["path"], "models/custom/crawler_big.mdl");
        assert_eq!(std::fs::read(hl.join("cstrike/models/custom/crawler_bigT.mdl")).unwrap(), b"IDST tex");
        // the offered name (rename: true)
        let r = import_with(&hl, "cstrike", "model", set(), &Some(Value::Bool(true)), Default::default()).unwrap();
        assert_eq!(r["path"], "models/custom/crawler_2.mdl", "{r}");
        // never over its own earlier import either
        let again = import_to_game(&hl, "cstrike", "model", set(), Some("crawler_big")).unwrap();
        assert!(again["taken"].is_object());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn map_project_files_are_listed_and_copied_in_the_same_checked_way() {
        let hl = game("proj");
        let proj_dir = hl.join("projects/arena");
        std::fs::create_dir_all(proj_dir.join("models/props")).unwrap();
        std::fs::create_dir_all(proj_dir.join("sound/amb")).unwrap();
        let proj = proj_dir.join("arena.lsmap");
        std::fs::write(&proj, b"{}").unwrap();
        std::fs::write(proj_dir.join("models/props/barrel.mdl"), MDL).unwrap();
        std::fs::write(proj_dir.join("models/props/barrelT.mdl"), b"IDST tex").unwrap();
        std::fs::write(proj_dir.join("sound/amb/wind.wav"), WAV).unwrap();
        let projects = vec![proj.clone()];
        let models = project_files(&projects, "model");
        assert_eq!(models.len(), 1, "the T file isn't a pick of its own: {models:?}");
        assert_eq!((models[0]["path"].as_str(), models[0]["projectName"].as_str()), (Some("models/props/barrel.mdl"), Some("arena")));
        assert_eq!(project_files(&projects, "sound")[0]["name"], "wind.wav");
        let r = adopt(&hl, "cstrike", &projects, "model", &proj.to_string_lossy(), "models/props/barrel.mdl", None).unwrap();
        assert_eq!(r["path"], "models/custom/barrel.mdl", "{r}");
        assert!(hl.join("cstrike/models/custom/barrelT.mdl").is_file(), "its T file came along");
        // only recent projects, only files inside their folder
        assert!(adopt(&hl, "cstrike", &projects, "model", "/etc/passwd", "x.mdl", None).is_err());
        assert!(adopt(&hl, "cstrike", &projects, "model", &proj.to_string_lossy(), "../../cstrike/models/custom/barrel.mdl", None).is_err());
        assert!(adopt(&hl, "cstrike", &projects, "model", &proj.to_string_lossy(), "/etc/hosts", None).is_err());
        // copied twice: taken, nothing over it
        assert!(adopt(&hl, "cstrike", &projects, "model", &proj.to_string_lossy(), "models/props/barrel.mdl", None).unwrap()["taken"].is_object());
        let _ = std::fs::remove_dir_all(&hl);
    }

    /// A file the pickers put in the game travels and counts like any custom file: a shared
    /// creature packs it under its maker's folder, a mode's node packs it too, and the map
    /// doctor and the .res list it when a map names it.
    #[test]
    fn imported_files_are_packed_and_counted_like_custom_files() {
        let hl = game("pack");
        import_to_game(&hl, "cstrike", "model", vec![("crawler.mdl".into(), MDL.to_vec())], None).unwrap();
        import_to_game(&hl, "cstrike", "sprite", vec![("goal.spr".into(), b"IDSP sprite".to_vec())], None).unwrap();
        let ini = hl.join("cstrike").join(super::super::creatures::FILE_REL);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "[crawler]\nbase = runner\nmodel = models/custom/crawler.mdl\n").unwrap();
        // a shared creature: its model under the maker's folder
        let pack = super::super::creaturepack::pack_types(&hl, "cstrike", &["crawler".into()], "gb7").unwrap();
        let names: Vec<String> = pack.entries(1 << 20).unwrap().into_iter().map(|(n, _)| n).collect();
        assert!(names.contains(&"models/gb7/custom/crawler.mdl".to_string()), "{names:?}");
        assert!(pack.text.contains("model = models/gb7/custom/crawler.mdl"), "{}", pack.text);
        // a mode: an Objective Marker's sprite goes with it
        let g = json!({"nodes": [{"type": "fx.marker", "params": {"sprite": "sprites/custom/goal.spr"}}, {"type": "prop.spawn", "params": {"model": "models/custom/crawler.mdl"}}]});
        let files = super::super::creaturepack::npc_files_in_graph_json(&g);
        assert!(super::super::creaturepack::any_custom(&hl, "cstrike", &files));
        let pack = super::super::creaturepack::Pack::default().with_npc(&hl, "cstrike", &files, "gb7").unwrap();
        let names: Vec<String> = pack.entries(1 << 20).unwrap().into_iter().map(|(n, _)| n).collect();
        assert!(names.contains(&"sprites/gb7/custom/goal.spr".to_string()) && names.contains(&"models/gb7/custom/crawler.mdl".to_string()), "{names:?}");
        let mut gv = g.clone();
        pack.rewrite_graph(&mut gv);
        assert_eq!(gv["nodes"][0]["params"]["sprite"], "sprites/gb7/custom/goal.spr");
        // a map naming it: the doctor finds it here (not the game's own) and the .res lists it
        let ents = "{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"cycler\"\n\"model\" \"models/custom/crawler.mdl\"\n}\n";
        let checks = super::super::doctor::file_checks(&crate::mapcopy::Finder::default(), &hl, "cstrike", "qa_map", ents);
        let c = checks.iter().find(|c| c.rel == "models/custom/crawler.mdl").expect("the doctor checks it");
        assert!(matches!(c.found, super::super::doctor::Found::Here { stock: false, .. }), "{:?}", c.found);
        assert!(super::super::doctor::res_wanted(&checks).contains(&"models/custom/crawler.mdl".to_string()));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn the_games_own_names_and_places_are_never_used() {
        let hl = game("stock");
        // a stock model name goes to models/custom/ (never the game's own place) and is refused
        // there as the game's own name
        let r = import_to_game(&hl, "cstrike", "model", vec![("v_ak47.mdl".into(), MDL.to_vec())], None);
        assert!(r.unwrap_err().contains("own file called v_ak47.mdl"));
        assert!(!hl.join("cstrike/models/v_ak47.mdl").exists() && !hl.join("cstrike/models/custom/v_ak47.mdl").exists());
        let _ = std::fs::remove_dir_all(&hl);
    }
}
