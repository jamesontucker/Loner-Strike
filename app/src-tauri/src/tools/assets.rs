//! The texture and model browser (Tools > Textures & models): every WAD in the game
//! folders and the textures maps carry inside them, which maps use each texture, the game's
//! models in 3D, and adding your own textures and models.
//!
//! Files are named, never pathed: a game folder from a fixed list, a WAD's file name, a
//! model's path under `models/` (each part checked, resolved whatever its case, and kept
//! inside that folder). New files come from the system's file dialog only. Downloads from
//! GameBanana go through the app's existing Get more code (`/api/gb/*`, the same checks,
//! records, backups and size caps); textures are the `textures` section there.
//!
//! App window only (merged into tools::local_routes). JSON routes answer `{error}` with
//! 200 (400 for a bad name); binary routes answer `{error}` with an error status (404 not
//! there, 422 damaged). POSTs need the panel header.

use crate::content;
use crate::game::Game;
use crate::goldsrc::mdl::{self, Model};
use crate::goldsrc::mesh::{write_bundle, Section};
use crate::goldsrc::{self, bsp, entities, wad, wadwrite};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use image::RgbaImage;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::cache;

const NO_GAME: &str = "Set up the game on this computer first.";
const BAD_NAME: &str = "That isn't a file the browser can open.";
const NO_FILE: &str = "That file isn't in the game folder any more.";
const NO_PICTURE: &str = "Choose a picture first.";
const INTERNAL: &str = "Loner-Strike hit an internal error in the texture and model browser. Try again, and copy the diagnostics from Settings if it keeps happening.";

/// The game folders the browser looks in.
const FOLDERS: [&str; 5] = ["cstrike", "czero", "valve", "cstrike_downloads", "czero_downloads"];
/// WADs the game itself needs: never written to, never replaced.
const STOCK_WADS: [&str; 12] = ["halflife.wad", "cstrike.wad", "czero.wad", "decals.wad", "tempdecal.wad", "pldecal.wad", "gfx.wad", "fonts.wad", "cached.wad", "liquids.wad", "xeno.wad", "spraypaint.wad"];
/// Models listed at most (all folders), and how deep under models/ we look.
const MAX_MODELS: usize = 20_000;
const MAX_DEPTH: usize = 5;
/// Custom WAD backups kept per WAD.
const KEEP_BACKUPS: usize = 5;
/// The biggest custom WAD we rewrite.
const MAX_CUSTOM_WAD: u64 = 64 << 20;

// ---- answers --------------------------------------------------------------------------

#[derive(Debug)]
struct Fail {
    status: StatusCode,
    msg: String,
}

impl Fail {
    fn new(status: StatusCode, msg: impl Into<String>) -> Fail {
        Fail { status, msg: msg.into() }
    }
    fn bad() -> Fail {
        Fail::new(StatusCode::BAD_REQUEST, BAD_NAME)
    }
    fn gone() -> Fail {
        Fail::new(StatusCode::NOT_FOUND, NO_FILE)
    }
}

impl From<goldsrc::Error> for Fail {
    fn from(e: goldsrc::Error) -> Fail {
        let status = if matches!(e, goldsrc::Error::Io(_)) { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY };
        Fail::new(status, e.to_string())
    }
}

impl From<String> for Fail {
    fn from(msg: String) -> Fail {
        Fail::new(StatusCode::OK, msg)
    }
}

type Done<T> = Result<T, Fail>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Done<T> + Send + 'static) -> Done<T> {
    tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(Fail::new(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)))
}

fn json_answer(r: Done<Value>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(f) if f.status == StatusCode::BAD_REQUEST || f.status == StatusCode::FORBIDDEN => (f.status, Json(json!({"error": f.msg}))).into_response(),
        Err(f) => Json(json!({"error": f.msg})).into_response(),
    }
}

/// Bytes with their type and version tag (None: the page already has this version).
struct Bin {
    bytes: Vec<u8>,
    kind: &'static str,
    etag: String,
}

fn binary_answer(r: Done<Option<Bin>>, seen: Option<String>) -> Response {
    match r {
        Ok(Some(b)) => ([(header::CONTENT_TYPE, b.kind.to_string()), (header::CACHE_CONTROL, "no-cache".into()), (header::ETAG, b.etag)], b.bytes).into_response(),
        Ok(None) => (StatusCode::NOT_MODIFIED, [(header::CACHE_CONTROL, "no-cache".to_string()), (header::ETAG, seen.unwrap_or_default())]).into_response(),
        Err(f) => {
            let status = if f.status == StatusCode::OK { StatusCode::UNPROCESSABLE_ENTITY } else { f.status };
            (status, Json(json!({"error": f.msg}))).into_response()
        }
    }
}

fn if_none_match(h: &HeaderMap) -> Option<String> {
    h.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()).map(String::from)
}

fn local_game(st: &Shared) -> Done<Arc<Game>> {
    st.local_game().ok_or_else(|| Fail::new(StatusCode::CONFLICT, NO_GAME))
}

// ---- names and folders ----------------------------------------------------------------

/// The folders there are, the player's game first.
fn folders(hl: &Path, czero: bool) -> Vec<(&'static str, PathBuf)> {
    let order: [&str; 5] = if czero { ["czero", "cstrike", "valve", "czero_downloads", "cstrike_downloads"] } else { FOLDERS };
    order.iter().map(|f| (*f, hl.join(f))).filter(|(_, d)| d.is_dir()).collect()
}

fn folder(hl: &Path, name: &str) -> Done<PathBuf> {
    let f = FOLDERS.iter().find(|f| f.eq_ignore_ascii_case(name)).ok_or_else(Fail::bad)?;
    let d = hl.join(f);
    if d.is_dir() { Ok(d) } else { Err(Fail::gone()) }
}

/// A WAD's file name: one plain part ending in .wad.
fn wad_name(name: &str) -> Option<&str> {
    (content::safe_part(name) && name.to_lowercase().ends_with(".wad") && name.len() > 4).then_some(name)
}

/// A texture name from the URL ("{grate.png" or "{grate").
fn texture_name(file: &str) -> Option<String> {
    let n = file.strip_suffix(".png").or_else(|| file.strip_suffix(".PNG")).unwrap_or(file);
    (!n.is_empty() && n.len() <= 16 && n.bytes().all(|c| c.is_ascii_graphic() && c != b'/' && c != b'\\')).then(|| n.to_string())
}

/// A model's path under the folder ("models/player/gign/gign.mdl"): 2-6 plain parts, the
/// first `models`, the last a .mdl.
fn model_rel(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let parts: Vec<&str> = p.split('/').collect();
    let ok = (2..=MAX_DEPTH + 1).contains(&parts.len())
        && parts[0].eq_ignore_ascii_case("models")
        && parts.iter().all(|x| content::safe_part(x))
        && parts.last().is_some_and(|l| l.to_lowercase().ends_with(".mdl"));
    ok.then_some(p)
}

/// The file at `rel` under `base`, whatever its case, and really inside `base` (no links out).
fn inside(base: &Path, rel: &str) -> Done<PathBuf> {
    let p = crate::mapcopy::Finder::default().find(base, rel).ok_or_else(Fail::gone)?;
    let (Ok(cb), Ok(cp)) = (base.canonicalize(), p.canonicalize()) else { return Err(Fail::gone()) };
    if !cp.starts_with(&cb) || !cp.is_file() {
        return Err(Fail::bad());
    }
    Ok(p)
}

fn etag_of(parts: &[&str]) -> String {
    format!("\"{}\"", cache::short_hash(parts))
}

fn stamp_text(p: &Path) -> Done<String> {
    Ok(cache::Stamp::of(p).map_err(|_| Fail::gone())?.key())
}

fn png_bin(t: &wad::Rgba, etag: String) -> Done<Option<Bin>> {
    Ok(Some(Bin { bytes: goldsrc::png(t.width, t.height, &t.pixels, true)?, kind: "image/png", etag }))
}

// ---- custom WADs (made here) ------------------------------------------------------------

fn custom_record(app: &Path) -> PathBuf {
    app.join("tools").join("custom_wads.json")
}

/// The WADs made with the browser: lower-case "folder/name.wad".
fn custom_wads(app: &Path) -> Vec<String> {
    let v: Value = std::fs::read(custom_record(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    v["wads"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.to_lowercase())).collect()
}

fn add_custom(app: &Path, key: &str) -> std::io::Result<()> {
    let mut list = custom_wads(app);
    if !list.iter().any(|k| k == &key.to_lowercase()) {
        list.push(key.to_lowercase());
    }
    std::fs::create_dir_all(app.join("tools"))?;
    crate::sys::write_atomic(&custom_record(app), serde_json::to_string_pretty(&json!({"wads": list})).unwrap_or_default().as_bytes())
}

// ---- WADs: list, textures, pictures ------------------------------------------------------

/// A texture's name, size and kind read from its record's header.
fn wad_listing(path: &Path) -> Done<Vec<Value>> {
    use std::io::{Read, Seek, SeekFrom};
    static MEMO: OnceLock<Mutex<Vec<(PathBuf, String, Arc<Vec<Value>>)>>> = OnceLock::new();
    let stamp = stamp_text(path)?;
    let memo = MEMO.get_or_init(Default::default);
    if let Some(hit) = memo.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|m| m.0 == path && m.1 == stamp) {
        return Ok(hit.2.as_ref().clone());
    }
    let w = wad::Wad::open(path)?;
    let size = std::fs::metadata(path).map_err(|_| Fail::gone())?.len();
    let mut f = std::fs::File::open(path).map_err(|_| Fail::gone())?;
    let mut out = Vec::new();
    for (low, e) in &w.entries {
        if e.kind != 0x43 && e.kind != 0x40 {
            continue;
        }
        let mut head = [0u8; 24];
        let ok = e.compression == 0 && e.offset as u64 + 24 <= size && f.seek(SeekFrom::Start(e.offset as u64)).is_ok() && f.read_exact(&mut head).is_ok();
        let (name, width, height) = if ok {
            let r = goldsrc::Le::new(&head, "texture file");
            let n = goldsrc::cname(&head[..16]);
            (if n.eq_ignore_ascii_case(low) { n } else { low.clone() }, r.u32(16).unwrap_or(0), r.u32(20).unwrap_or(0))
        } else {
            (low.clone(), 0, 0)
        };
        out.push(json!({"name": name, "width": width, "height": height, "kind": if e.kind == 0x40 { "decal" } else { "texture" }}));
    }
    out.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_lowercase());
    let mut m = memo.lock().unwrap_or_else(|e| e.into_inner());
    m.push((path.to_path_buf(), stamp, Arc::new(out.clone())));
    if m.len() > 8 {
        m.remove(0);
    }
    Ok(out)
}

/// Every WAD in the game folders.
fn list_wads(game: &Game) -> Value {
    let custom = custom_wads(&game.app);
    let mut out = Vec::new();
    for (name, dir) in folders(&game.hl, game.is_czero()) {
        let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
            .filter(|(n, p)| wad_name(n).is_some() && p.is_file())
            .collect();
        files.sort_by_key(|(n, _)| n.to_lowercase());
        for (file, path) in files {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let key = format!("{name}/{file}").to_lowercase();
            let mut v = json!({"folder": name, "name": file, "size": size, "custom": custom.contains(&key), "stock": STOCK_WADS.contains(&file.to_lowercase().as_str())});
            match wad_listing(&path) {
                Ok(list) => v["textures"] = json!(list.len()),
                Err(e) => v["error"] = json!(e.msg),
            }
            out.push(v);
        }
    }
    json!({"wads": out, "game": if game.is_czero() { "czero" } else { "cstrike" }})
}

/// An opened WAD for a minute (a page of textures arrives as many requests).
fn open_wad(path: &Path) -> Done<Arc<wad::Wad>> {
    static MEMO: OnceLock<Mutex<Vec<(PathBuf, String, Arc<wad::Wad>)>>> = OnceLock::new();
    let stamp = stamp_text(path)?;
    let memo = MEMO.get_or_init(Default::default);
    if let Some(hit) = memo.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|m| m.0 == path && m.1 == stamp) {
        return Ok(hit.2.clone());
    }
    let w = Arc::new(wad::Wad::open(path)?);
    let mut m = memo.lock().unwrap_or_else(|e| e.into_inner());
    m.push((path.to_path_buf(), stamp, w.clone()));
    if m.len() > 6 {
        m.remove(0);
    }
    Ok(w)
}

fn wad_texture(path: &Path, name: &str) -> Done<wad::Rgba> {
    let w = open_wad(path)?;
    let rec = w.read_miptex(name)?.ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "That WAD doesn't have that texture."))?;
    Ok(wad::decode_miptex(&rec, name)?)
}

/// A WAD texture as PNG, kept in the Tools cache.
fn wad_texture_png(folder: &str, path: &Path, name: &str, seen: Option<&str>) -> Done<Option<Bin>> {
    let stamp = cache::Stamp::of(path).map_err(|_| Fail::gone())?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = cache::map_dir(&cache::root().join("assets"), &format!("wad-{folder}-{file_name}"), &stamp);
    let file = cache::safe_file(name, "png");
    let etag = cache::etag(&dir, &file);
    if seen == Some(etag.as_str()) && dir.join(&file).is_file() {
        return Ok(None);
    }
    let bytes = cache::get_or_build(&dir, &[file.as_str()], 0, || -> Done<Vec<Vec<u8>>> {
        let t = wad_texture(path, name)?;
        Ok(vec![goldsrc::png(t.width, t.height, &t.pixels, true)?])
    })?;
    Ok(Some(Bin { bytes, kind: "image/png", etag }))
}

// ---- which maps use what --------------------------------------------------------------

/// For every map: the textures it uses (embedded or not), its WADs and the models its
/// entities name. Built from the texture and entity lumps only, kept while the maps stay
/// the same.
struct Usage {
    maps: Vec<String>,
    textures: BTreeMap<String, Vec<u32>>,
    embedded: Vec<(u32, String, u32, u32)>,
    wads: BTreeMap<String, Vec<u32>>,
    models: BTreeMap<String, Vec<u32>>,
}

fn usage(game: &Game) -> Arc<Usage> {
    static MEMO: OnceLock<Mutex<Option<(String, Arc<Usage>)>>> = OnceLock::new();
    let finder = crate::mapcopy::Finder::default();
    let czero = game.is_czero();
    let mut files = Vec::new();
    for m in game.maps(false) {
        let Some(name) = m["name"].as_str().filter(|n| crate::maps::safe_arg(n)) else { continue };
        let Some(lm) = crate::mapcopy::find_local(&finder, &game.hl, name, if czero { "czero" } else { "cstrike" }) else { continue };
        let Ok(st) = cache::Stamp::of(&lm.bsp) else { continue };
        files.push((lm.name, lm.bsp, st));
    }
    let key = cache::short_hash(&files.iter().map(|(n, _, s)| format!("{n}:{}", s.key())).collect::<Vec<_>>().iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let memo = MEMO.get_or_init(Default::default);
    if let Some((k, u)) = memo.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *k == key {
            return u.clone();
        }
    }
    let mut u = Usage { maps: Vec::new(), textures: BTreeMap::new(), embedded: Vec::new(), wads: BTreeMap::new(), models: BTreeMap::new() };
    for (name, bsp_path, _) in files {
        let Ok(l) = bsp::read_lumps(&bsp_path, &[bsp::LUMP_TEXTURES, bsp::LUMP_ENTITIES]) else { continue };
        let i = u.maps.len() as u32;
        u.maps.push(name);
        let add = |m: &mut BTreeMap<String, Vec<u32>>, k: String| {
            let v = m.entry(k).or_default();
            if v.last() != Some(&i) {
                v.push(i);
            }
        };
        for t in bsp::textures(&l[0], false).unwrap_or_default().into_iter().filter(|t| !t.name.is_empty()) {
            if t.embedded.is_some() {
                u.embedded.push((i, t.name.clone(), t.width, t.height));
            }
            add(&mut u.textures, t.name.to_lowercase());
        }
        let ents = entities::parse(&crate::rcon::latin1(&l[1]));
        for w in entities::wad_list(&ents) {
            add(&mut u.wads, w.to_lowercase());
        }
        for e in &ents {
            if let Some(m) = e.get("model").map(|m| m.replace('\\', "/").to_lowercase()).filter(|m| m.ends_with(".mdl") && m.len() < 128) {
                add(&mut u.models, m);
            }
        }
    }
    let u = Arc::new(u);
    *memo.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, u.clone()));
    u
}

fn usage_json(u: &Usage) -> Value {
    json!({
        "maps": u.maps,
        "textures": u.textures,
        "embedded": u.embedded.iter().map(|(m, n, w, h)| json!([m, n, w, h])).collect::<Vec<_>>(),
        "wads": u.wads,
        "models": u.models,
    })
}

// ---- models: list, bundle, textures, animation -------------------------------------------

/// What kind of model a path is, for the list's groups.
fn model_kind(rel_low: &str) -> &'static str {
    let file = rel_low.rsplit('/').next().unwrap_or("");
    if rel_low.starts_with("models/player/") {
        "player"
    } else if file.starts_with("v_") {
        "view"
    } else if file.starts_with("p_") {
        "carried"
    } else if file.starts_with("w_") {
        "world"
    } else if file.contains("hostage") || file.starts_with("scientist") || rel_low.contains("/hostage") {
        "hostage"
    } else {
        "prop"
    }
}

/// A file that's part of another model: its textures (`fooT.mdl`) or animations (`foo01.mdl`).
fn companion(file_low: &str, names: &std::collections::HashSet<String>) -> bool {
    let Some(stem) = file_low.strip_suffix(".mdl") else { return false };
    if let Some(base) = stem.strip_suffix('t') {
        if names.contains(&format!("{base}.mdl")) {
            return true;
        }
    }
    if stem.len() > 2 && stem[stem.len() - 2..].bytes().all(|c| c.is_ascii_digit()) && stem.is_char_boundary(stem.len() - 2) {
        return names.contains(&format!("{}.mdl", &stem[..stem.len() - 2]));
    }
    false
}

fn list_models(game: &Game) -> Value {
    let imported = imported(&game.app);
    let mut out = Vec::new();
    'all: for (fname, dir) in folders(&game.hl, game.is_czero()) {
        let mut stack = vec![(dir.join("models"), vec!["models".to_string()])];
        while let Some((d, rel)) = stack.pop() {
            let entries: Vec<(String, PathBuf, bool)> = std::fs::read_dir(&d)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| (e.file_name().to_string_lossy().to_string(), e.path(), e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                .collect();
            let names: std::collections::HashSet<String> = entries.iter().filter(|e| !e.2).map(|e| e.0.to_lowercase()).collect();
            for (name, path, is_dir) in entries {
                if !content::safe_part(&name) {
                    continue;
                }
                if is_dir {
                    if rel.len() < MAX_DEPTH {
                        let mut r = rel.clone();
                        r.push(name);
                        stack.push((path, r));
                    }
                    continue;
                }
                let low = name.to_lowercase();
                if !low.ends_with(".mdl") || companion(&low, &names) {
                    continue;
                }
                let rel_path = format!("{}/{name}", rel.join("/"));
                let rel_low = rel_path.to_lowercase();
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                out.push(json!({
                    "folder": fname, "path": rel_path, "name": name[..name.len() - 4], "kind": model_kind(&rel_low), "size": size,
                    "imported": imported.contains(&format!("{fname}/{rel_low}")),
                }));
                if out.len() >= MAX_MODELS {
                    break 'all;
                }
            }
        }
    }
    out.sort_by(|a, b| {
        let k = |v: &Value| (v["kind"].as_str().unwrap_or("").to_string(), v["path"].as_str().unwrap_or("").to_lowercase());
        k(a).cmp(&k(b))
    });
    json!({"models": out})
}

/// A parsed model (with its companion textures), kept for a few requests.
struct Loaded {
    path: PathBuf,
    stamp: String,
    model: Model,
}

fn load_model(path: &Path) -> Done<Arc<Loaded>> {
    static MEMO: OnceLock<Mutex<Vec<Arc<Loaded>>>> = OnceLock::new();
    let stamp = stamp_text(path)?;
    let memo = MEMO.get_or_init(Default::default);
    if let Some(hit) = memo.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|m| m.path == path && m.stamp == stamp) {
        return Ok(hit.clone());
    }
    let mut model = Model::parse(goldsrc::read_capped(path, "model file", mdl::MAX_MDL)?)?;
    if model.textures.is_empty() {
        if let (Some(dir), Some(stem)) = (path.parent(), path.file_stem()) {
            let t = format!("{}t.mdl", stem.to_string_lossy());
            if let Some(tp) = crate::mapcopy::Finder::default().find(dir, &t) {
                if let Ok(tm) = goldsrc::read_capped(&tp, "model file", mdl::MAX_MDL).map_err(Fail::from).and_then(|b| Model::parse(b).map_err(Fail::from)) {
                    model.take_textures(tm);
                }
            }
        }
    }
    let l = Arc::new(Loaded { path: path.to_path_buf(), stamp, model });
    let mut m = memo.lock().unwrap_or_else(|e| e.into_inner());
    m.push(l.clone());
    if m.len() > 4 {
        m.remove(0);
    }
    Ok(l)
}

/// A sequence's frames (each bone: x, y, z, quaternion), reading a group file when the
/// sequence lives in one (`<name>01.mdl`).
fn sequence_frames(l: &Loaded, seq: usize) -> Done<(Vec<f32>, usize)> {
    let m = &l.model;
    let s = m.sequences.get(seq).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "That model has no such animation."))?;
    let frames = (s.frames.max(1) as usize).min(mdl::MAX_FRAMES);
    let group_data = if s.group > 0 {
        let stem = l.path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let dir = l.path.parent().ok_or_else(Fail::gone)?;
        let gp = crate::mapcopy::Finder::default().find(dir, &format!("{stem}{:02}.mdl", s.group));
        match gp.and_then(|p| goldsrc::read_capped(&p, "model file", mdl::MAX_MDL).ok()) {
            Some(b) if b.starts_with(b"IDSQ") => Some(b),
            _ => return Err(Fail::new(StatusCode::NOT_FOUND, "That animation is in a file that isn't here.")),
        }
    } else {
        None
    };
    let mut out = Vec::with_capacity(frames * m.bones.len() * 7);
    for f in 0..frames {
        let pose = match &group_data {
            Some(d) => m.pose_with(seq, f as u32, d)?,
            None => m.pose(seq, f as u32)?,
        };
        for b in pose {
            out.extend(b);
        }
    }
    Ok((out, frames))
}

/// The model as an LSMB bundle: every body part's every model, posed by its first
/// sequence's first frame (`positions`, `normals`), plus the bone-space data and bone of
/// each vertex for playing animations (`local`, `localNormals`, `bone`, `normalBone`).
/// `uv` is divided by the size of the texture skin 0 uses; draws are per mesh.
pub fn model_bundle(m: &Model) -> Vec<u8> {
    let pose = m.pose(0, 0).unwrap_or_else(|_| m.rest());
    let mats = mdl::bone_matrices(&m.bones, &pose);
    let ident: mdl::Mat = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
    let mat = |b: u8| mats.get(b as usize).unwrap_or(&ident);
    let (mut pos, mut nor, mut uv, mut local, mut lnor) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut bone, mut nbone, mut idx) = (Vec::<u16>::new(), Vec::<u16>::new(), Vec::<u32>::new());
    let mut draws = Vec::new();
    let mut parts = Vec::new();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (pi, part) in m.bodyparts.iter().enumerate() {
        let mut models = Vec::new();
        for (mi, sm) in part.models.iter().enumerate() {
            let mut tris = 0;
            for (ki, mesh) in sm.meshes.iter().enumerate() {
                let tex = m.texture_for(0, mesh.skin_ref).map(|t| &m.textures[t]);
                let (tw, th) = tex.map(|t| (t.width as f32, t.height as f32)).unwrap_or((1.0, 1.0));
                let start = idx.len();
                let mut seen: HashMap<(u16, u16, i16, i16), u32> = HashMap::new();
                for t in &mesh.tris {
                    for c in t {
                        let at = *seen.entry((c.vert, c.norm, c.s, c.t)).or_insert_with(|| {
                            let v = sm.verts[c.vert as usize];
                            let vb = sm.vert_bone.get(c.vert as usize).copied().unwrap_or(0);
                            let n = sm.norms.get(c.norm as usize).copied().unwrap_or([0.0, 0.0, 1.0]);
                            let nb = sm.norm_bone.get(c.norm as usize).copied().unwrap_or(vb);
                            let p = mdl::transform(mat(vb), v);
                            let mut q = mdl::rotate(mat(nb), n);
                            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
                            if len > 1e-6 {
                                q = q.map(|x| x / len);
                            }
                            for k in 0..3 {
                                lo[k] = lo[k].min(p[k]);
                                hi[k] = hi[k].max(p[k]);
                            }
                            pos.extend(p);
                            nor.extend(q);
                            local.extend(v);
                            lnor.extend(n);
                            uv.extend([c.s as f32 / tw, c.t as f32 / th]);
                            bone.push(vb as u16);
                            nbone.push(nb as u16);
                            (bone.len() - 1) as u32
                        });
                        idx.push(at);
                    }
                }
                tris += mesh.tris.len();
                draws.push(json!({"part": pi, "model": mi, "mesh": ki, "skinRef": mesh.skin_ref, "start": start, "count": idx.len() - start}));
            }
            models.push(json!({"name": sm.name, "triangles": tris}));
        }
        parts.push(json!({"name": part.name, "base": part.base, "models": models}));
    }
    if bone.is_empty() {
        lo = [0.0; 3];
        hi = [0.0; 3];
    }
    let header = json!({
        "kind": "model",
        "name": m.name,
        "eye": m.eye,
        "bounds": [lo, hi],
        "bones": m.bones.iter().map(|b| json!({"name": b.name, "parent": b.parent})).collect::<Vec<_>>(),
        "sequences": m.sequences.iter().map(|s| json!({"name": s.name, "fps": s.fps, "frames": s.frames, "loop": s.flags & mdl::SEQ_LOOPING != 0, "activity": s.activity, "group": s.group})).collect::<Vec<_>>(),
        "textures": m.textures.iter().map(|t| json!({"name": t.name, "width": t.width, "height": t.height, "flags": t.flags,
            "masked": t.flags & mdl::TEX_MASKED != 0, "additive": t.flags & mdl::TEX_ADDITIVE != 0, "chrome": t.flags & mdl::TEX_CHROME != 0, "fullbright": t.flags & mdl::TEX_FULLBRIGHT != 0})).collect::<Vec<_>>(),
        "skins": m.skins,
        "bodyparts": parts,
        "draws": draws,
        "triangles": idx.len() / 3,
    });
    write_bundle(
        header,
        &[
            Section::f32("positions", 3, &pos),
            Section::f32("normals", 3, &nor),
            Section::f32("uv", 2, &uv),
            Section::f32("local", 3, &local),
            Section::f32("localNormals", 3, &lnor),
            Section::u16("bone", 1, &bone),
            Section::u16("normalBone", 1, &nbone),
            Section::u32("indices", 1, &idx),
        ],
    )
}

#[derive(Deserialize)]
struct ModelQuery {
    folder: String,
    path: String,
    #[serde(default)]
    i: Option<usize>,
    #[serde(default)]
    seq: Option<usize>,
}

fn find_model(game: &Game, q: &ModelQuery) -> Done<(PathBuf, String)> {
    let base = folder(&game.hl, &q.folder)?;
    let rel = model_rel(&q.path).ok_or_else(Fail::bad)?;
    Ok((inside(&base, &rel)?, rel))
}

/// What a model route sends: 0 the bundle, 1 a texture, 2 a sequence's frames.
fn model_answer(game: &Game, q: &ModelQuery, want: u8, seen: Option<&str>) -> Done<Option<Bin>> {
    let (path, _) = find_model(game, q)?;
    let stamp = stamp_text(&path)?;
    let tag = etag_of(&[&path.to_string_lossy(), &stamp, &want.to_string(), &format!("{:?}{:?}", q.i, q.seq), &goldsrc::PARSER_VERSION.to_string(), "mdl1"]);
    if seen == Some(tag.as_str()) {
        return Ok(None);
    }
    let l = load_model(&path)?;
    match want {
        0 => Ok(Some(Bin { bytes: model_bundle(&l.model), kind: "application/octet-stream", etag: tag })),
        1 => {
            let t = l.model.textures.get(q.i.unwrap_or(0)).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "That model has no such texture."))?;
            png_bin(&mdl::texture_rgba(t), tag)
        }
        _ => {
            let seq = q.seq.unwrap_or(0);
            let (frames, n) = sequence_frames(&l, seq)?;
            let s = &l.model.sequences[seq];
            let header = json!({"kind": "animation", "sequence": seq, "name": s.name, "fps": s.fps, "frames": n, "bones": l.model.bones.len(), "loop": s.flags & mdl::SEQ_LOOPING != 0});
            Ok(Some(Bin { bytes: write_bundle(header, &[Section::f32("frames", 7, &frames)]), kind: "application/octet-stream", etag: tag }))
        }
    }
}

// ---- imported models (new files we put in) ------------------------------------------------

fn imported_record(app: &Path) -> PathBuf {
    app.join("tools").join("imported_models.json")
}

/// Models added here as new files: lower-case "folder/models/...".
fn imported(app: &Path) -> Vec<String> {
    let v: Value = std::fs::read(imported_record(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    v["files"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.to_lowercase())).collect()
}

fn save_imported(app: &Path, list: &[String]) -> std::io::Result<()> {
    std::fs::create_dir_all(app.join("tools"))?;
    crate::sys::write_atomic(&imported_record(app), serde_json::to_string_pretty(&json!({"files": list})).unwrap_or_default().as_bytes())
}

/// Where a model file of this name goes when it's new: weapons in models/, a player model
/// in models/player/<name>/, anything else in models/.
fn new_model_dest(file: &str, m: &Model) -> String {
    let low = file.to_lowercase();
    let player = !["v_", "p_", "w_"].iter().any(|p| low.starts_with(p))
        && m.bones.len() >= 20
        && m.sequences.iter().any(|s| s.name.eq_ignore_ascii_case("walk") || s.name.to_lowercase().starts_with("run"))
        && m.sequences.len() >= 20;
    if player {
        let stem = &file[..file.len() - 4];
        format!("models/player/{stem}/{file}")
    } else {
        format!("models/{file}")
    }
}

/// Copies a model picked on this computer into the game as a new file (never over one):
/// returns the path it went to. Models that replace a game file go through Get more instead.
fn import_new_model(game_dir: &Path, folder_name: &str, app: &Path, src: &Path, m: &Model) -> Result<String, String> {
    let file = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if !content::safe_part(&file) {
        return Err("That file's name has characters the game can't use. Rename it (letters, digits, - and _) and try again.".into());
    }
    let dest = new_model_dest(&file, m);
    let (target, rel, exists) = content::resolve(game_dir, &dest);
    if exists {
        return Err(format!("The game already has {rel}."));
    }
    let mut copies = vec![(src.to_path_buf(), target.clone(), rel.clone())];
    // its textures file, when they're apart (fooT.mdl next to foo.mdl)
    if m.textures.is_empty() {
        let tname = format!("{}t.mdl", &file[..file.len() - 4]);
        let found = src.parent().and_then(|d| crate::mapcopy::Finder::default().find(d, &tname));
        match found {
            Some(t) => {
                let tdest = format!("{}/{}", dest.rsplit_once('/').map(|x| x.0).unwrap_or("models"), t.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(tname));
                let (tt, trel, texists) = content::resolve(game_dir, &tdest);
                if !texists {
                    copies.push((t, tt, trel));
                }
            }
            None => return Err(format!("This model keeps its textures in {}T.mdl, which isn't next to it. Put both files in the same folder and try again.", &file[..file.len() - 4])),
        }
    }
    let mut done = imported(app);
    for (from, to, rel) in &copies {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Couldn't make the folder: {e}"))?;
        }
        crate::safety::before("Textures & models", &format!("Added {rel}"), to);
        std::fs::copy(from, to).map_err(|e| format!("Couldn't copy the model: {e}"))?;
        done.push(format!("{folder_name}/{}", rel.to_lowercase()));
    }
    save_imported(app, &done).map_err(|e| e.to_string())?;
    Ok(rel)
}

/// Removes a model this browser added (only those).
fn remove_imported(game: &Game, folder_name: &str, rel: &str) -> Result<(), String> {
    let mut list = imported(&game.app);
    let key = format!("{folder_name}/{}", rel.to_lowercase());
    if !list.contains(&key) {
        return Err("Only models added here can be removed here. Others are removed in Get more.".into());
    }
    let base = folder(&game.hl, folder_name).map_err(|f| f.msg)?;
    // the model and its textures file
    let stem = rel[..rel.len() - 4].to_lowercase();
    let mut gone = Vec::new();
    for k in list.iter().filter(|k| **k == key || **k == format!("{folder_name}/{stem}t.mdl")) {
        let r = &k[folder_name.len() + 1..];
        if let Ok(p) = inside(&base, r) {
            crate::safety::before("Textures & models", &format!("Removed {r}"), &p);
            std::fs::remove_file(&p).map_err(|e| format!("Couldn't remove it: {e}"))?;
            // an emptied player folder goes too
            if let Some(d) = p.parent().filter(|d| d.parent().is_some_and(|pp| pp.file_name().is_some_and(|n| n.eq_ignore_ascii_case("player")))) {
                let _ = std::fs::remove_dir(d);
            }
        }
        gone.push(k.clone());
    }
    list.retain(|k| !gone.contains(k));
    save_imported(&game.app, &list).map_err(|e| e.to_string())
}

// ---- your own texture into a custom WAD ------------------------------------------------

struct Picked {
    img: RgbaImage,
}

static PICKED: Mutex<Option<Picked>> = Mutex::new(None);

/// A texture name from a picture's file name: lower case, allowed characters, 15 at most.
fn suggest_name(file: &str) -> String {
    let stem = file.rsplit_once('.').map(|x| x.0).unwrap_or(file).to_lowercase();
    let n: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).take(15).collect();
    if n.is_empty() { "mytexture".into() } else { n }
}

/// A custom WAD's file name: letters, digits, - and _, ending .wad, not one of the game's.
fn custom_wad_name(name: &str) -> Option<String> {
    let n = name.trim();
    let n = if n.to_lowercase().ends_with(".wad") { n.to_string() } else { format!("{n}.wad") };
    let stem = &n[..n.len() - 4];
    let ok = !stem.is_empty() && stem.len() <= 40 && stem.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') && !STOCK_WADS.contains(&n.to_lowercase().as_str());
    ok.then_some(n)
}

#[derive(Deserialize)]
struct TextureChoice {
    name: String,
    #[serde(default = "default_side")]
    size: u32,
    #[serde(default)]
    wad: String,
}

fn default_side() -> u32 {
    256
}

fn make_from_picked(c: &TextureChoice) -> Result<wadwrite::MipTexture, String> {
    let guard = PICKED.lock().unwrap_or_else(|e| e.into_inner());
    let p = guard.as_ref().ok_or(NO_PICTURE)?;
    wadwrite::make_texture(&p.img, &c.name, c.size).map_err(|e| match e {
        goldsrc::Error::NotFormat { .. } => "Texture names are 1-15 letters, digits or _ - (a { first makes the blue parts see-through).".to_string(),
        e => e.to_string(),
    })
}

/// Writes the texture into a custom WAD in the game folder: a new WAD, or one made here
/// before (the old version is kept in the app's folder). Returns (WAD name, replaced a
/// texture of the same name).
fn save_texture(game_dir: &Path, folder_name: &str, app: &Path, wad: &str, t: &wadwrite::MipTexture) -> Result<(String, bool), String> {
    let name = custom_wad_name(wad).ok_or("WAD names are letters, digits, - and _ (and not one of the game's own).")?;
    let (path, real, exists) = content::resolve(game_dir, &name);
    let key = format!("{folder_name}/{real}").to_lowercase();
    let mut lumps = Vec::new();
    if exists {
        if !custom_wads(app).contains(&key) {
            return Err(format!("{real} wasn't made here, so it's left as it is. Pick another name for your WAD."));
        }
        let bytes = goldsrc::read_capped(&path, "texture file", MAX_CUSTOM_WAD).map_err(|e| e.to_string())?;
        lumps = wadwrite::read_lumps(&bytes).map_err(|e| e.to_string())?;
        // the old version, in case
        let dir = app.join("tools").join("wad-backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stem = real[..real.len() - 4].to_lowercase();
        std::fs::write(dir.join(format!("{folder_name}-{stem}-{}.wad", (crate::sys::now() * 1000.0) as u64)), &bytes).map_err(|e| e.to_string())?;
        let mut old: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{folder_name}-{stem}-"))))
            .collect();
        old.sort();
        while old.len() > KEEP_BACKUPS {
            let _ = std::fs::remove_file(old.remove(0));
        }
    }
    let replaced = lumps.iter().any(|l| l.name.eq_ignore_ascii_case(&t.name));
    wadwrite::put_lump(&mut lumps, wadwrite::miptex_lump(t)).map_err(|e| e.to_string())?;
    crate::safety::before("Textures & models", &format!("Saved {} in {real}", t.name), &path);
    crate::sys::write_atomic(&path, &wadwrite::write_lumps(&lumps)).map_err(|e| format!("Couldn't save the WAD: {e}"))?;
    add_custom(app, &key).map_err(|e| e.to_string())?;
    Ok((real, replaced))
}

// ---- handlers -------------------------------------------------------------------------

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "Not allowed."}))).into_response()
}

async fn api_wads(State(st): State<Shared>) -> Response {
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => blocking(move || Ok(list_wads(&g))).await,
    };
    json_answer(r)
}

async fn api_wad(State(st): State<Shared>, UrlPath((f, w)): UrlPath<(String, String)>) -> Response {
    let r = match local_game(&st) {
        Err(e) => Err(e),
        Ok(g) => {
            blocking(move || {
                let base = folder(&g.hl, &f)?;
                let name = wad_name(&w).ok_or_else(Fail::bad)?;
                let path = inside(&base, name)?;
                Ok(json!({"folder": f, "name": path.file_name().map(|n| n.to_string_lossy().to_string()), "textures": wad_listing(&path)?}))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_wad_texture(State(st): State<Shared>, UrlPath((f, w, t)): UrlPath<(String, String, String)>, h: HeaderMap) -> Response {
    let seen = if_none_match(&h);
    let r = match local_game(&st) {
        Err(e) => Err(e),
        Ok(g) => {
            let seen = seen.clone();
            blocking(move || {
                let base = folder(&g.hl, &f)?;
                let name = wad_name(&w).ok_or_else(Fail::bad)?;
                let tex = texture_name(&t).ok_or_else(Fail::bad)?;
                let path = inside(&base, name)?;
                wad_texture_png(&f.to_lowercase(), &path, &tex, seen.as_deref())
            })
            .await
        }
    };
    binary_answer(r, seen)
}

async fn api_usage(State(st): State<Shared>) -> Response {
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => blocking(move || Ok(usage_json(&usage(&g)))).await,
    };
    json_answer(r)
}

async fn api_models(State(st): State<Shared>) -> Response {
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => blocking(move || Ok(list_models(&g))).await,
    };
    json_answer(r)
}

async fn model_route(st: Shared, q: ModelQuery, h: HeaderMap, want: u8) -> Response {
    let seen = if_none_match(&h);
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => {
            let seen = seen.clone();
            blocking(move || model_answer(&g, &q, want, seen.as_deref())).await
        }
    };
    binary_answer(r, seen)
}

async fn api_model(State(st): State<Shared>, Query(q): Query<ModelQuery>, h: HeaderMap) -> Response {
    model_route(st, q, h, 0).await
}

async fn api_model_texture(State(st): State<Shared>, Query(q): Query<ModelQuery>, h: HeaderMap) -> Response {
    model_route(st, q, h, 1).await
}

async fn api_model_anim(State(st): State<Shared>, Query(q): Query<ModelQuery>, h: HeaderMap) -> Response {
    model_route(st, q, h, 2).await
}

/// Model facts the list doesn't have (sizes, counts), for the side panel.
async fn api_model_info(State(st): State<Shared>, Query(q): Query<ModelQuery>) -> Response {
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => {
            blocking(move || {
                let (path, rel) = find_model(&g, &q)?;
                let l = load_model(&path)?;
                let m = &l.model;
                let u = usage(&g);
                let low = rel.to_lowercase();
                let maps: Vec<&String> = u.models.get(&low).map(|v| v.iter().filter_map(|&i| u.maps.get(i as usize)).collect()).unwrap_or_default();
                Ok(json!({"name": m.name, "bones": m.bones.len(), "sequences": m.sequences.len(), "textures": m.textures.len(), "skins": m.skins.len().max(1),
                          "bodyparts": m.bodyparts.len(), "maps": maps}))
            })
            .await
        }
    };
    json_answer(r)
}

#[derive(Deserialize)]
struct ExportReq {
    kind: String,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    wad: String,
    #[serde(default)]
    map: String,
    #[serde(default)]
    tex: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    i: usize,
}

/// The pixels and a file name for an export.
fn export_pixels(g: &Game, r: &ExportReq) -> Done<(wad::Rgba, String)> {
    match r.kind.as_str() {
        "wad" => {
            let base = folder(&g.hl, &r.folder)?;
            let path = inside(&base, wad_name(&r.wad).ok_or_else(Fail::bad)?)?;
            let tex = texture_name(&r.tex).ok_or_else(Fail::bad)?;
            Ok((wad_texture(&path, &tex)?, tex))
        }
        "map" => {
            if !crate::maps::safe_arg(&r.map) {
                return Err(Fail::bad());
            }
            let tex = texture_name(&r.tex).ok_or_else(Fail::bad)?;
            let m = super::mapinfo::locate(g, &r.map).ok_or_else(Fail::gone)?;
            let lite = super::mapinfo::read_lite(&m.bsp, true)?;
            let own = lite.textures.iter().find(|t| t.name.eq_ignore_ascii_case(&tex)).and_then(|t| t.embedded.clone());
            let rec = match own {
                Some(rec) if rec.len() > 40 => rec,
                _ => {
                    let wads = m.wads(&crate::mapcopy::Finder::default(), &entities::wad_list(&lite.entities));
                    wads.read(&tex)?.ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "That texture's file isn't on this computer."))?
                }
            };
            Ok((wad::decode_miptex(&rec, &tex)?, tex))
        }
        "model" => {
            let q = ModelQuery { folder: r.folder.clone(), path: r.path.clone(), i: Some(r.i), seq: None };
            let (path, _) = find_model(g, &q)?;
            let l = load_model(&path)?;
            let t = l.model.textures.get(r.i).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "That model has no such texture."))?;
            let name = t.name.rsplit_once('.').map(|x| x.0.to_string()).unwrap_or_else(|| t.name.clone());
            Ok((mdl::texture_rgba(t), name))
        }
        _ => Err(Fail::bad()),
    }
}

/// A file name for the save dialog: the texture's name with odd characters swapped.
fn export_file_name(tex: &str) -> String {
    let n: String = tex.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    format!("{}.png", if n.is_empty() { "texture" } else { &n })
}

async fn api_export(State(st): State<Shared>, h: HeaderMap, Json(req): Json<ExportReq>) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match (local_game(&st), st.app.get().cloned()) {
        (Err(f), _) => Err(f),
        (_, None) => Err(Fail::new(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)),
        (Ok(g), Some(app)) => {
            blocking(move || {
                let (t, name) = export_pixels(&g, &req)?;
                let png = goldsrc::png(t.width, t.height, &t.pixels, true)?;
                let Some(dest) = app.dialog().file().set_title("Save the texture as a picture").set_file_name(export_file_name(&name)).add_filter("PNG picture", &["png"]).blocking_save_file() else {
                    return Ok(json!({"cancelled": true}));
                };
                let dest = dest.into_path().map_err(|e| e.to_string())?;
                std::fs::write(&dest, png).map_err(|e| format!("Couldn't save it: {e}"))?;
                Ok(json!({"ok": true, "message": format!("Saved {}.", dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())}))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_pick_picture(State(st): State<Shared>, h: HeaderMap) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return json_answer(Err(Fail::new(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL))) };
    json_answer(
        blocking(move || {
            let Some(path) = app.dialog().file().set_title("Choose a picture for your texture").add_filter("Pictures", &["png", "jpg", "jpeg", "bmp", "tga", "webp"]).blocking_pick_file() else {
                return Ok(json!({"cancelled": true}));
            };
            let path = path.into_path().map_err(|e| e.to_string())?;
            let img = super::spray::load_picture(&path)?;
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let v = json!({"name": name, "width": img.width(), "height": img.height(), "suggested": suggest_name(&name)});
            *PICKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Picked { img });
            Ok(v)
        })
        .await,
    )
}

async fn api_texture_preview(h: HeaderMap, Json(c): Json<TextureChoice>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let t = make_from_picked(&c)?;
            let rgba = wad::decode_miptex(&wadwrite::miptex_lump(&t).data, "")?;
            let png = goldsrc::png(rgba.width, rgba.height, &rgba.pixels, true)?;
            Ok(json!({"png": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)), "width": t.width, "height": t.height}))
        })
        .await,
    )
}

async fn api_texture_save(State(st): State<Shared>, h: HeaderMap, Json(c): Json<TextureChoice>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => {
            blocking(move || {
                let t = make_from_picked(&c)?;
                // custom WADs go in cstrike: Condition Zero looks there too, and so do the compilers' setups
                let folder_name = "cstrike";
                let dir = folder(&g.hl, folder_name)?;
                let (wad, replaced) = save_texture(&dir, folder_name, &g.app, &c.wad, &t)?;
                let what = if replaced { "replaced in" } else { "added to" };
                Ok(json!({"ok": true, "folder": folder_name, "wad": wad, "texture": t.name, "message": format!("{} {what} {wad}. Maps that list {wad} can use it.", t.name)}))
            })
            .await
        }
    };
    json_answer(r)
}

#[derive(Deserialize)]
struct AddReq {
    kind: String,
}

/// Your own .wad or .mdl: picked in the system's dialog, read with the parsers first, then
/// installed. WADs and models that replace a game file go through Get more's installer (so
/// they're recorded, backed up and removable there); a new model is copied in as a new file.
async fn api_add_file(State(st): State<Shared>, h: HeaderMap, Json(req): Json<AddReq>) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match (local_game(&st), st.app.get().cloned()) {
        (Err(f), _) => Err(f),
        (_, None) => Err(Fail::new(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)),
        (Ok(g), Some(app)) => {
            blocking(move || {
                let (title, filter, ext) = match req.kind.as_str() {
                    "wad" => ("Add a texture file (WAD)", "Texture files", "wad"),
                    "model" => ("Add a model", "Models", "mdl"),
                    _ => return Err(Fail::bad()),
                };
                let Some(path) = app.dialog().file().set_title(title).add_filter(filter, &[ext]).blocking_pick_file() else {
                    return Ok(json!({"cancelled": true}));
                };
                let path = path.into_path().map_err(|e| e.to_string())?;
                add_own_file(&g, &path, ext)
            })
            .await
        }
    };
    json_answer(r)
}

fn add_own_file(g: &Game, path: &Path, ext: &str) -> Done<Value> {
    let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if !file.to_lowercase().ends_with(&format!(".{ext}")) {
        return Err(format!("That isn't a .{ext} file.").into());
    }
    if ext == "wad" {
        let bytes = goldsrc::read_capped(path, "texture file", goldsrc::MAX_WAD)?;
        let lumps = wadwrite::read_lumps(&bytes)?;
        let textures = lumps.iter().filter(|l| l.kind == 0x43).count();
        if textures == 0 {
            return Err("That WAD has no textures in it.".to_string().into());
        }
        if STOCK_WADS.contains(&file.to_lowercase().as_str()) {
            return Err(format!("{file} is one of the game's own files, so it can't be replaced here.").into());
        }
        let job = g.store.start_local_install("textures", path).map_err(|e| e.message())?;
        return Ok(json!({"ok": true, "job": job, "message": format!("Adding {file} ({textures} textures).")}));
    }
    let model = Model::parse(goldsrc::read_capped(path, "model file", mdl::MAX_MDL)?)?;
    if model.bodyparts.is_empty() {
        return Err("That file only holds another model's textures. Add the main model (without the T) instead.".to_string().into());
    }
    let low = file.to_lowercase();
    let game_dirs = [g.dir.clone(), g.hl.join("cstrike")];
    let existing = content::Existing::scan(&game_dirs);
    let weapon = ["v_", "p_", "w_"].iter().any(|p| low.starts_with(p));
    // a file of this name in the game: replaced through Get more (backed up, removable there)
    let in_game = weapon || {
        let mut rels = Vec::new();
        for d in &game_dirs {
            for f in crate::archive::walk(&d.join("models")) {
                if f.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&file)) {
                    rels.push(f);
                }
            }
        }
        !rels.is_empty()
    };
    if in_game {
        let (plan, _) = content::plan("skins", &[vec![file.clone()]], &existing);
        let section = match plan.first() {
            Some((_, d)) if d.to_lowercase().starts_with("models/player/") => "players",
            Some(_) => "skins",
            None => return Err(format!("The game has more than one {file}, so the app can't tell which one to replace.").into()),
        };
        let job = g.store.start_local_install(section, path).map_err(|e| e.message())?;
        return Ok(json!({"ok": true, "job": job, "message": format!("Adding {file}. The game's own file is kept and comes back if you remove it in Get more.")}));
    }
    let folder_name = g.dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "cstrike".into());
    let rel = import_new_model(&g.dir, &folder_name, &g.app, path, &model)?;
    Ok(json!({"ok": true, "folder": folder_name, "path": rel, "message": format!("Added {rel}.")}))
}

#[derive(Deserialize)]
struct RemoveReq {
    folder: String,
    path: String,
}

async fn api_remove_model(State(st): State<Shared>, h: HeaderMap, Json(req): Json<RemoveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match local_game(&st) {
        Err(f) => Err(f),
        Ok(g) => {
            blocking(move || {
                let rel = model_rel(&req.path).ok_or_else(Fail::bad)?;
                let f = FOLDERS.iter().find(|f| f.eq_ignore_ascii_case(&req.folder)).ok_or_else(Fail::bad)?;
                remove_imported(&g, f, &rel)?;
                Ok(json!({"ok": true}))
            })
            .await
        }
    };
    json_answer(r)
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/assets/wads", get(api_wads))
        .route("/api/tools/assets/wad/:folder/:wad", get(api_wad))
        .route("/api/tools/assets/wad/:folder/:wad/tex/:file", get(api_wad_texture))
        .route("/api/tools/assets/usage", get(api_usage))
        .route("/api/tools/assets/models", get(api_models))
        .route("/api/tools/assets/model", get(api_model))
        .route("/api/tools/assets/model/info", get(api_model_info))
        .route("/api/tools/assets/model/tex", get(api_model_texture))
        .route("/api/tools/assets/model/anim", get(api_model_anim))
        .route("/api/tools/assets/export", post(api_export))
        .route("/api/tools/assets/texture/pick", post(api_pick_picture))
        .route("/api/tools/assets/texture/preview", post(api_texture_preview))
        .route("/api/tools/assets/texture/save", post(api_texture_save))
        .route("/api/tools/assets/add-file", post(api_add_file))
        .route("/api/tools/assets/model/remove", post(api_remove_model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::bsp::fixture;
    use crate::server::AppState;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::RwLock;
    use tower::ServiceExt;

    /// A Half-Life folder: the cube room map, a WAD, a weapon model, a player model with its
    /// textures apart, an animation group file and a texture-only companion.
    fn game(tag: &str) -> (PathBuf, Arc<Game>) {
        let root = std::env::temp_dir().join(format!("ls-assets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        for d in ["cstrike/maps", "cstrike/models/player/guy", "valve"] {
            std::fs::create_dir_all(hl.join(d)).unwrap();
        }
        std::fs::write(hl.join("cstrike/maps/Room.bsp"), fixture::build()).unwrap();
        std::fs::write(hl.join("valve/test.wad"), wad::build_wad(&[("{grate", fixture::miptex("{grate", true)), ("Brick", fixture::miptex("brick", true))])).unwrap();
        std::fs::write(hl.join("cstrike/models/v_test.mdl"), mdl::fixture::build(false)).unwrap();
        std::fs::write(hl.join("cstrike/models/player/guy/guy.mdl"), mdl::fixture::build(true)).unwrap();
        std::fs::write(hl.join("cstrike/models/player/guy/guyT.mdl"), mdl::fixture::textures_file()).unwrap();
        std::fs::write(hl.join("cstrike/models/v_test01.mdl"), b"IDSQ").unwrap();
        let g = Game::with_app_dir(&hl, "cstrike", &root.join("app"));
        (root, Arc::new(g))
    }

    fn state(g: Option<Arc<Game>>) -> Shared {
        Arc::new(AppState {
            current: RwLock::new(g),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec!["192.168.1.5:8028".into()]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        })
    }

    async fn call(st: &Shared, method: &str, uri: &str, etag: Option<&str>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut req = Request::builder().method(method).uri(uri).header("host", "127.0.0.1:8027");
        if let Some(e) = etag {
            req = req.header(header::IF_NONE_MATCH, e);
        }
        let body = if method == "POST" {
            req = req.header("content-type", "application/json");
            Body::from("{\"kind\":\"wad\"}")
        } else {
            Body::empty()
        };
        let res = routes().with_state(st.clone()).oneshot(req.body(body).unwrap()).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        (status, headers, axum::body::to_bytes(res.into_body(), 64 << 20).await.unwrap().to_vec())
    }

    fn json_of(b: &[u8]) -> Value {
        serde_json::from_slice(b).unwrap()
    }

    #[tokio::test]
    async fn wads_and_textures() {
        let (root, g) = game("wads");
        let st = state(Some(g));
        let (s, _, b) = call(&st, "GET", "/api/tools/assets/wads", None).await;
        let v = json_of(&b);
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["wads"][0]["name"], "test.wad", "{v}");
        assert_eq!(v["wads"][0]["folder"], "valve");
        assert_eq!(v["wads"][0]["textures"], 2);

        let (_, _, b) = call(&st, "GET", "/api/tools/assets/wad/valve/TEST.wad", None).await;
        let v = json_of(&b);
        assert_eq!(v["textures"], json!([{"name": "brick", "width": 16, "height": 16, "kind": "texture"}, {"name": "{grate", "width": 16, "height": 16, "kind": "texture"}]), "{v}");

        let (s, h, png) = call(&st, "GET", "/api/tools/assets/wad/valve/test.wad/tex/%7Bgrate.png", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(image::load_from_memory(&png).unwrap().width(), 16);
        let tag = h[header::ETAG].to_str().unwrap().to_string();
        assert_eq!(call(&st, "GET", "/api/tools/assets/wad/valve/test.wad/tex/%7Bgrate.png", Some(&tag)).await.0, StatusCode::NOT_MODIFIED);
        let (s, _, b) = call(&st, "GET", "/api/tools/assets/wad/valve/test.wad/tex/nothere.png", None).await;
        assert_eq!((s, json_of(&b)["error"].as_str().is_some()), (StatusCode::NOT_FOUND, true));

        // names that aren't allowed
        for uri in ["/api/tools/assets/wad/evil/test.wad", "/api/tools/assets/wad/valve/..%2Fx.wad", "/api/tools/assets/wad/valve/test.bsp", "/api/tools/assets/wad/valve/.wad"] {
            assert_eq!(call(&st, "GET", uri, None).await.0, StatusCode::BAD_REQUEST, "{uri}");
        }
        let (s, _, b) = call(&st, "GET", "/api/tools/assets/wad/cstrike/none.wad", None).await;
        assert_eq!((s, json_of(&b)["error"].as_str()), (StatusCode::OK, Some(NO_FILE)));

        // which maps use what
        let (_, _, b) = call(&st, "GET", "/api/tools/assets/usage", None).await;
        let v = json_of(&b);
        assert_eq!(v["maps"], json!(["Room"]), "{v}");
        assert_eq!(v["textures"]["{grate"], json!([0]));
        assert_eq!(v["embedded"][0], json!([0, "brick", 16, 16]));
        assert_eq!(v["wads"]["test.wad"], json!([0]));
        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(cache::root()).ok();
    }

    #[tokio::test]
    async fn models() {
        let (root, g) = game("models");
        let st = state(Some(g));
        let (_, _, b) = call(&st, "GET", "/api/tools/assets/models", None).await;
        let v = json_of(&b);
        let list = v["models"].as_array().unwrap();
        // the textures file and the animation group aren't models of their own
        assert_eq!(list.len(), 2, "{v}");
        assert_eq!(list[0]["path"], "models/player/guy/guy.mdl");
        assert_eq!(list[0]["kind"], "player");
        assert_eq!(list[1]["kind"], "view");

        let (s, h, bundle) = call(&st, "GET", "/api/tools/assets/model?folder=cstrike&path=models/player/guy/guy.mdl", None).await;
        assert_eq!(s, StatusCode::OK);
        let (head, sections) = crate::goldsrc::mesh::read_bundle(&bundle);
        assert_eq!(head["textures"][0]["name"], "skin.bmp", "companion textures were taken");
        assert_eq!(head["draws"][0]["count"], 6);
        assert_eq!(head["bodyparts"][0]["models"][1]["name"], "blank");
        assert_eq!(head["sequences"][0]["frames"], 3);
        assert_eq!(sections["indices"].len(), 6 * 4);
        assert_eq!(sections["bone"].len(), 4 * 2);
        // vertex 2 sits on the arm bone (turned 90 degrees at (5,0,10)): local (4,0,0) → (5,4,10)
        let pos: Vec<f32> = sections["positions"].chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        assert!(pos.chunks(3).any(|p| (p[0] - 5.0).abs() < 1e-4 && (p[1] - 4.0).abs() < 1e-4 && (p[2] - 10.0).abs() < 1e-4), "{pos:?}");
        let tag = h[header::ETAG].to_str().unwrap().to_string();
        assert_eq!(call(&st, "GET", "/api/tools/assets/model?folder=cstrike&path=models/player/guy/guy.mdl", Some(&tag)).await.0, StatusCode::NOT_MODIFIED);

        let (s, _, png) = call(&st, "GET", "/api/tools/assets/model/tex?folder=cstrike&path=models/v_test.mdl&i=0", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(image::load_from_memory(&png).unwrap().to_rgba8().get_pixel(0, 0).0[3], 0, "masked");
        assert_eq!(call(&st, "GET", "/api/tools/assets/model/tex?folder=cstrike&path=models/v_test.mdl&i=9", None).await.0, StatusCode::NOT_FOUND);

        let (s, _, anim) = call(&st, "GET", "/api/tools/assets/model/anim?folder=cstrike&path=models/v_test.mdl&seq=0", None).await;
        assert_eq!(s, StatusCode::OK);
        let (head, sections) = crate::goldsrc::mesh::read_bundle(&anim);
        assert_eq!((head["frames"].as_u64(), head["bones"].as_u64()), (Some(3), Some(2)));
        assert_eq!(sections["frames"].len(), 3 * 2 * 7 * 4);

        let (_, _, b) = call(&st, "GET", "/api/tools/assets/model/info?folder=cstrike&path=models/v_test.mdl", None).await;
        assert_eq!(json_of(&b)["bones"], 2);

        for uri in [
            "/api/tools/assets/model?folder=cstrike&path=models/../maps/Room.bsp",
            "/api/tools/assets/model?folder=cstrike&path=maps/x.mdl",
            "/api/tools/assets/model?folder=cstrike&path=models/x.bsp",
            "/api/tools/assets/model?folder=etc&path=models/v_test.mdl",
            "/api/tools/assets/model?folder=cstrike&path=models/%2Fetc/x.mdl",
        ] {
            assert_eq!(call(&st, "GET", uri, None).await.0, StatusCode::BAD_REQUEST, "{uri}");
        }
        assert_eq!(call(&st, "GET", "/api/tools/assets/model?folder=cstrike&path=models/nothere.mdl", None).await.0, StatusCode::NOT_FOUND);
        // a damaged model is an error with words
        std::fs::write(st.local_game().unwrap().hl.join("cstrike/models/v_bad.mdl"), &mdl::fixture::build(false)[..500]).unwrap();
        let (s, _, b) = call(&st, "GET", "/api/tools/assets/model?folder=cstrike&path=models/v_bad.mdl", None).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(json_of(&b)["error"].as_str().unwrap().contains("damaged"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn posts_need_the_panel_and_no_game() {
        let (root, g) = game("posts");
        let st = state(Some(g));
        for uri in ["/api/tools/assets/export", "/api/tools/assets/texture/pick", "/api/tools/assets/texture/save", "/api/tools/assets/add-file", "/api/tools/assets/model/remove"] {
            let s = call(&st, "POST", uri, None).await.0;
            assert!(s == StatusCode::FORBIDDEN || s == StatusCode::UNPROCESSABLE_ENTITY, "{uri}: {s}");
        }
        let (_, _, b) = call(&state(None), "GET", "/api/tools/assets/wads", None).await;
        assert_eq!(json_of(&b)["error"], NO_GAME);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn custom_wad_writes() {
        let (root, g) = game("custom");
        let dir = g.hl.join("cstrike");
        let img = RgbaImage::from_fn(64, 32, |x, y| image::Rgba([x as u8 * 4, y as u8 * 8, 99, 255]));
        let t = wadwrite::make_texture(&img, "mine", 256).unwrap();
        assert_eq!(save_texture(&dir, "cstrike", &g.app, "my_textures", &t).unwrap(), ("my_textures.wad".to_string(), false));
        let t2 = wadwrite::make_texture(&img, "{mine2", 32).unwrap();
        save_texture(&dir, "cstrike", &g.app, "My_Textures.wad", &t2).unwrap();
        assert_eq!(save_texture(&dir, "cstrike", &g.app, "my_textures", &t).unwrap().1, true, "same name replaces");
        let w = wad::Wad::open(&dir.join("my_textures.wad")).unwrap();
        assert!(w.has_texture("mine") && w.has_texture("{mine2"));
        assert_eq!(w.entries.len(), 2);
        assert!(custom_wads(&g.app).contains(&"cstrike/my_textures.wad".to_string()));
        assert_eq!(std::fs::read_dir(g.app.join("tools/wad-backups")).unwrap().count(), 2);
        // a WAD that wasn't made here, the game's own, odd names: refused
        std::fs::write(dir.join("theirs.wad"), b"WAD3\0\0\0\0\x0c\0\0\0").unwrap();
        assert!(save_texture(&dir, "cstrike", &g.app, "theirs", &t).unwrap_err().contains("wasn't made here"));
        for bad in ["cstrike", "decals.wad", "../x", "a b", "", "x.wad.wad"] {
            assert!(save_texture(&dir, "cstrike", &g.app, bad, &t).is_err(), "{bad}");
        }
        assert_eq!(suggest_name("My Picture (1).PNG"), "my_picture__1_");
        assert_eq!(suggest_name(".png"), "mytexture");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn own_models() {
        let (root, g) = game("own");
        let src = root.join("mine");
        std::fs::create_dir_all(&src).unwrap();
        // a new prop with its textures apart
        std::fs::write(src.join("crate.mdl"), mdl::fixture::build(true)).unwrap();
        std::fs::write(src.join("crateT.mdl"), mdl::fixture::textures_file()).unwrap();
        let m = Model::parse(std::fs::read(src.join("crate.mdl")).unwrap()).unwrap();
        let rel = import_new_model(&g.dir, "cstrike", &g.app, &src.join("crate.mdl"), &m).unwrap();
        assert_eq!(rel, "models/crate.mdl");
        assert!(g.dir.join("models/crateT.mdl").is_file());
        assert!(import_new_model(&g.dir, "cstrike", &g.app, &src.join("crate.mdl"), &m).unwrap_err().contains("already has"));
        let listed = list_models(&g);
        assert!(listed["models"].as_array().unwrap().iter().any(|v| v["path"] == "models/crate.mdl" && v["imported"] == true));
        // only what was added here can be removed here
        assert!(remove_imported(&g, "cstrike", "models/v_test.mdl").is_err());
        remove_imported(&g, "cstrike", "models/crate.mdl").unwrap();
        assert!(!g.dir.join("models/crate.mdl").exists() && !g.dir.join("models/crateT.mdl").exists());
        assert!(imported(&g.app).is_empty());
        // textures apart but not next to it
        std::fs::remove_file(src.join("crateT.mdl")).unwrap();
        assert!(import_new_model(&g.dir, "cstrike", &g.app, &src.join("crate.mdl"), &m).unwrap_err().contains("crateT.mdl"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn names() {
        assert_eq!(model_rel("models\\player\\a\\a.mdl").as_deref(), Some("models/player/a/a.mdl"));
        for bad in ["models/x.wad", "x.mdl", "models/../a.mdl", "models//a.mdl", "models/.a.mdl", "maps/a.mdl", "models/a/b/c/d/e/f/g.mdl"] {
            assert!(model_rel(bad).is_none(), "{bad}");
        }
        let names: std::collections::HashSet<String> = ["gign.mdl", "gignt.mdl", "hostage.mdl", "hostage01.mdl", "w_9mm.mdl"].iter().map(|s| s.to_string()).collect();
        assert!(companion("gignt.mdl", &names) && companion("hostage01.mdl", &names));
        assert!(!companion("gign.mdl", &names) && !companion("w_9mm.mdl", &names) && !companion("abc12.mdl", &names));
        assert_eq!(model_kind("models/player/gign/gign.mdl"), "player");
        assert_eq!(model_kind("models/p_ak47.mdl"), "carried");
        assert_eq!(model_kind("models/hostage02.mdl"), "hostage");
        assert_eq!(model_kind("models/barrel.mdl"), "prop");
        assert_eq!(custom_wad_name("abc").as_deref(), Some("abc.wad"));
        assert!(custom_wad_name("halflife").is_none());
    }

    /// Texture downloads and own WADs go in through Get more's installer as the `textures`
    /// section: loose WADs at the top of the game folder, never the game's own.
    #[test]
    fn textures_section() {
        assert!(content::section("textures").is_some());
        assert!(content::allowed("textures", "mytex.wad"));
        assert!(!content::allowed("textures", "halflife.wad") && !content::allowed("textures", "decals.wad"));
        assert!(!content::allowed("textures", "models/mytex.wad") && !content::allowed("textures", "x.mdl"));
        assert!(!content::allowed("skins", "mytex.wad"));
        let (plan, skipped) = content::plan("textures", &[vec!["pack".into(), "My_Tex.wad".into()], vec!["readme.txt".into()], vec!["cstrike.wad".into()]], &content::Existing::default());
        assert_eq!(plan, vec![(0, "My_Tex.wad".to_string())]);
        assert_eq!(skipped.len(), 2);
    }

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> lists every WAD and decodes every texture in
    /// it, and builds the bundle for every model. `cargo test --release real_assets -- --nocapture`.
    #[test]
    fn real_assets() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = PathBuf::from(hl);
        let g = Game::with_app_dir(&hl, "cstrike", &std::env::temp_dir().join(format!("ls-assets-real-{}", std::process::id())));
        let t0 = std::time::Instant::now();
        let wads = list_wads(&g);
        let (mut textures, mut bad) = (0, Vec::new());
        for w in wads["wads"].as_array().unwrap() {
            let path = hl.join(w["folder"].as_str().unwrap()).join(w["name"].as_str().unwrap());
            let Ok(list) = wad_listing(&path) else {
                bad.push(format!("{}: {}", path.display(), w["error"]));
                continue;
            };
            for t in list {
                match wad_texture(&path, t["name"].as_str().unwrap()) {
                    Ok(_) => textures += 1,
                    Err(e) => bad.push(format!("{}/{}: {}", w["name"], t["name"], e.msg)),
                }
            }
        }
        println!("{} WADs, {textures} textures decoded in {} ms; {} not: {:?}", wads["wads"].as_array().unwrap().len(), t0.elapsed().as_millis(), bad.len(), bad.iter().take(10).collect::<Vec<_>>());
        let t0 = std::time::Instant::now();
        let models = list_models(&g);
        let (mut ok, mut tris, mut failed) = (0, 0u64, Vec::new());
        for m in models["models"].as_array().unwrap() {
            let path = hl.join(m["folder"].as_str().unwrap()).join(m["path"].as_str().unwrap());
            match load_model(&path) {
                Ok(l) => {
                    let b = model_bundle(&l.model);
                    let (head, _) = crate::goldsrc::mesh::read_bundle(&b);
                    tris += head["triangles"].as_u64().unwrap();
                    for s in 0..l.model.sequences.len().min(3) {
                        if let Err(e) = sequence_frames(&l, s) {
                            failed.push(format!("{} seq {s}: {}", path.display(), e.msg));
                        }
                    }
                    ok += 1;
                }
                Err(e) => failed.push(format!("{}: {}", path.display(), e.msg)),
            }
        }
        println!("{} models listed, {ok} built ({tris} triangles) in {} ms; {} problems: {:?}", models["models"].as_array().unwrap().len(), t0.elapsed().as_millis(), failed.len(), failed.iter().take(10).collect::<Vec<_>>());
        let t0 = std::time::Instant::now();
        let u = usage(&g);
        println!("usage: {} maps, {} textures, {} models in {} ms", u.maps.len(), u.textures.len(), u.models.len(), t0.elapsed().as_millis());
    }
}
