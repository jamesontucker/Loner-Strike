//! The prefab library's routes (`/api/tools/editor/prefabs*`): the starter prefabs that come
//! with the app and the player's own in `<app dir>/editor/prefabs/` (`<id>.map`, `<id>.json`,
//! `<id>.png`), saving a selection, renaming, deleting, a package to share (zip) and
//! installing one (from a file, or from the community browser's Try it: `install_dir`).
//! Placing goes through the editor's `cmd` (`place_prefab`) so it's one undo step.

use super::{blocking, forbidden, Ed, NO_DOC};
use crate::editor::doc::MapDocument;
use crate::editor::history;
use crate::editor::prefab::{self, PrefabMeta};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_MINE: usize = 2000;
const BUILTIN: &str = "builtin:";
const MINE: &str = "mine:";
const THUMB_W: u32 = 192;
const THUMB_H: u32 = 144;

pub fn lib_dir(editor_dir: &Path) -> PathBuf {
    editor_dir.join("prefabs")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn card(id: &str, m: &PrefabMeta, builtin: bool) -> Value {
    json!({
        "id": id, "name": m.name, "tags": m.tags, "author": m.author, "description": m.description, "builtin": builtin,
        "brushes": m.brushes, "entities": m.entities, "size": m.size, "textures": m.textures, "wads": m.wads, "created": m.created,
        "thumb": format!("/api/tools/editor/prefab/{}/thumb.png", id.replace(':', "~")),
    })
}

/// The player's own prefabs: (id without the "mine:" prefix, details), newest first.
pub fn mine(dir: &Path) -> Vec<(String, PrefabMeta)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()).map(String::from) else { continue };
        if p.extension().and_then(|x| x.to_str()) != Some("json") || !prefab::valid_id(&stem) || !dir.join(format!("{stem}.map")).is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(m) = serde_json::from_str::<PrefabMeta>(&text) else { continue };
        out.push((stem, m));
        if out.len() >= MAX_MINE {
            break;
        }
    }
    out.sort_by(|a, b| b.1.created.cmp(&a.1.created).then(a.1.name.cmp(&b.1.name)));
    out
}

pub fn list(dir: &Path) -> Value {
    let mut items: Vec<Value> = mine(dir).iter().map(|(id, m)| card(&format!("{MINE}{id}"), m, false)).collect();
    items.extend(prefab::builtins().iter().map(|(id, m, _)| card(&format!("{BUILTIN}{id}"), m, true)));
    json!({"prefabs": items, "folder": dir.to_string_lossy()})
}

/// A prefab's pieces and details by id ("builtin:stairs", "mine:my_room").
pub fn load(dir: &Path, id: &str) -> Result<(MapDocument, PrefabMeta), String> {
    if let Some(b) = id.strip_prefix(BUILTIN) {
        return prefab::builtins().into_iter().find(|(i, _, _)| *i == b).map(|(_, m, f)| (f, m)).ok_or_else(|| "That prefab isn't there any more.".to_string());
    }
    let own = id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("That prefab isn't there any more.")?;
    let map = std::fs::read_to_string(dir.join(format!("{own}.map"))).map_err(|_| "That prefab isn't there any more.".to_string())?;
    let meta = std::fs::read_to_string(dir.join(format!("{own}.json"))).ok();
    prefab::from_files(&map, meta.as_deref(), own)
}

/// A free id for a name (`crate`, `crate_2`, ...).
fn free_id(dir: &Path, name: &str) -> String {
    let base = prefab::slug(name);
    let taken = |id: &str| dir.join(format!("{id}.map")).exists() || dir.join(format!("{id}.json")).exists();
    if !taken(&base) {
        return base;
    }
    (2..10_000).map(|n| format!("{base}_{n}")).find(|id| !taken(id)).unwrap_or_else(|| format!("{base}_{}", now_secs()))
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't save the prefab ({e}).")
    })
}

/// Saves pieces as one of the player's prefabs; its id ("mine:...").
pub fn save(dir: &Path, frag: &MapDocument, meta: &PrefabMeta, thumb: Option<&[u8]>) -> Result<String, String> {
    if mine(dir).len() >= MAX_MINE {
        return Err("Your prefab library is full (2000). Delete some first.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make the prefab folder ({e})."))?;
    let id = free_id(dir, &meta.name);
    let (map, js) = prefab::to_files(frag, meta)?;
    if let Some(png) = thumb {
        prefab::check_png(png)?;
        write_file(&dir.join(format!("{id}.png")), png)?;
    }
    write_file(&dir.join(format!("{id}.map")), map.as_bytes())?;
    write_file(&dir.join(format!("{id}.json")), js.as_bytes())?;
    Ok(format!("{MINE}{id}"))
}

/// What a package (an unpacked folder) brought: prefab names and WAD files beside them.
#[derive(Debug, Default)]
pub struct Installed {
    pub prefabs: Vec<String>,
    pub ids: Vec<String>,
    pub wads: Vec<PathBuf>,
    pub skipped: Vec<String>,
}

/// Installs every prefab in an unpacked package or download: each `.map` (with the `.json`
/// and `.png` of the same name beside it, when there are) becomes one of the player's
/// prefabs. WAD files are listed for the caller to put where the game looks.
pub fn install_dir(lib: &Path, src: &Path, author_hint: &str) -> Result<Installed, String> {
    let mut got = Installed::default();
    let files = crate::archive::walk(src);
    for p in &files {
        let low = p.to_string_lossy().to_lowercase();
        if low.ends_with(".wad") {
            got.wads.push(p.clone());
            continue;
        }
        if !low.ends_with(".map") {
            continue;
        }
        if got.prefabs.len() >= 200 {
            break;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("prefab").to_string();
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX);
        if size > prefab::MAX_PREFAB_MAP as u64 {
            got.skipped.push(format!("{stem}: too big"));
            continue;
        }
        let Ok(bytes) = std::fs::read(p) else { continue };
        let text: String = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => e.into_bytes().iter().map(|b| *b as char).collect(),
        };
        let js = crate::caps::read_text(&p.with_extension("json"), "prefab info", 1 << 20).ok(); // [Caps]
        match prefab::from_files(&text, js.as_deref(), &stem) {
            Ok((frag, mut meta)) => {
                if meta.author.is_empty() {
                    meta.author = prefab::clean_text(author_hint, 48);
                }
                if meta.created == 0 {
                    meta.created = now_secs();
                }
                let png = crate::caps::read_file(&p.with_extension("png"), "picture", 1 << 20).ok().filter(|b| prefab::check_png(b).is_ok()); // [Caps]
                let id = save(lib, &frag, &meta, png.as_deref())?;
                got.prefabs.push(meta.name.clone());
                got.ids.push(id);
            }
            Err(e) => got.skipped.push(format!("{stem}: {e}")),
        }
    }
    if got.prefabs.is_empty() {
        let why = got.skipped.first().cloned().map(|s| format!(" ({s})")).unwrap_or_default();
        return Err(format!("There's no prefab (.map) in it the editor can use{why}."));
    }
    Ok(got)
}

/// Copies a package's WADs into the game folder, never over a file that's there.
pub fn install_wads(wads: &[PathBuf], game_dir: &Path) -> Vec<String> {
    let mut added = Vec::new();
    for w in wads {
        let Some(name) = w.file_name().and_then(|n| n.to_str()) else { continue };
        if !crate::content::safe_part(name) {
            continue;
        }
        let dest = game_dir.join(name);
        if dest.exists() {
            continue;
        }
        // [Caps] size checked before reading; only the header is needed
        let ok = std::fs::metadata(w).is_ok_and(|m| m.len() <= crate::goldsrc::MAX_WAD) && std::fs::File::open(w).and_then(|f| { use std::io::Read; let mut h = [0u8; 4]; f.take(4).read_exact(&mut h).map(|_| h) }).is_ok_and(|h| &h == b"WAD3");
        if ok && std::fs::copy(w, &dest).is_ok() {
            added.push(name.to_string());
        }
    }
    added
}

/// A package to share: `prefabs/<id>.map|json|png`, README.txt and loner-strike.json.
pub fn package(dir: &Path, id: &str, out: &Path) -> Result<(), String> {
    let (frag, meta) = load(dir, id)?;
    let slug = id.split(':').nth(1).unwrap_or("prefab");
    let (map, js) = prefab::to_files(&frag, &meta)?;
    let png = match id.strip_prefix(MINE).map(|x| dir.join(format!("{x}.png"))).and_then(|p| std::fs::read(p).ok()) {
        Some(p) => p,
        None => prefab::thumbnail(&frag, THUMB_W, THUMB_H),
    };
    let readme = format!(
        "{}\r\n\r\nA map editor prefab made with Loner-Strike ({} brushes, {} entities, {} x {} x {} units).\r\n\
         Textures: {}\r\nWADs: {}\r\n\r\nLoner-Strike: Tools > Map editor > Prefabs > Add from a file, or Try it in Tools > Community.\r\n\
         Other editors: prefabs/{slug}.map is a plain Valve 220 .map (copy and paste it into J.A.C.K. or TrenchBroom).\r\n",
        meta.name, meta.brushes, meta.entities, meta.size[0], meta.size[1], meta.size[2], meta.textures.join(", "), meta.wads.join(", ")
    );
    let manifest = json!({"schema": 1, "app": env!("CARGO_PKG_VERSION"), "item": {"kind": "prefab", "name": meta.name, "author": meta.author, "summary": meta.description, "tags": meta.tags,
        "prefab": {"files": [format!("prefabs/{slug}.map")], "brushes": meta.brushes, "entities": meta.entities, "size": meta.size, "wads": meta.wads}}});
    let part = out.with_extension("part");
    let write = || -> Result<(), String> {
        let f = std::fs::File::create(&part).map_err(|e| format!("Couldn't write the package ({e})."))?;
        let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in [
            (format!("prefabs/{slug}.map"), map.into_bytes()),
            (format!("prefabs/{slug}.json"), js.into_bytes()),
            (format!("prefabs/{slug}.png"), png),
            ("README.txt".into(), readme.into_bytes()),
            ("loner-strike.json".into(), serde_json::to_vec_pretty(&manifest).unwrap_or_default()),
        ] {
            z.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(&body).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, out).map_err(|e| format!("Couldn't save the package ({e})."))
}

/// Installs a file the player picked: a package (zip/rar/7z) or a single .map.
fn install_file(lib: &Path, work: &Path, file: &Path, game_dir: Option<&Path>) -> Result<Value, String> {
    let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = crate::sys::make_temp_dir(work, "prefab-").map_err(|e| e.to_string())?;
    let res = (|| {
        let src = tmp.join("x");
        std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
        if crate::archive::is_archive(&name) {
            let copy = tmp.join(format!("in{}", crate::archive::ext_of(&name)));
            std::fs::copy(file, &copy).map_err(|e| format!("Couldn't read the file ({e})."))?;
            crate::archive::extract(&copy, &src, 512 << 20)?;
        } else if name.to_lowercase().ends_with(".map") {
            std::fs::copy(file, src.join(&name)).map_err(|e| format!("Couldn't read the file ({e})."))?;
            for side in ["json", "png"] {
                let _ = std::fs::copy(file.with_extension(side), src.join(Path::new(&name).with_extension(side)));
            }
        } else {
            return Err("Pick a prefab package (.zip) or a .map file.".into());
        }
        let got = install_dir(lib, &src, "")?;
        let wads = game_dir.map(|g| install_wads(&got.wads, g)).unwrap_or_default();
        Ok(json!({"ok": true, "prefabs": got.prefabs, "ids": got.ids, "wads": wads, "skipped": got.skipped}))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

// ---- routes ------------------------------------------------------------------------------

async fn api_list(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || Ok(list(&lib_dir(&ed.dir)))).await
}

async fn api_thumb(Extension(ed): Extension<Ed>, UrlPath(id): UrlPath<String>) -> Response {
    let id = id.replacen('~', ":", 1);
    let r = tokio::task::spawn_blocking(move || {
        let dir = lib_dir(&ed.dir);
        if let Some(own) = id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)) {
            if let Ok(b) = std::fs::read(dir.join(format!("{own}.png"))) {
                return Some(b);
            }
        }
        load(&dir, &id).ok().map(|(f, _)| prefab::thumbnail(&f, THUMB_W, THUMB_H))
    })
    .await
    .ok()
    .flatten();
    match r {
        Some(b) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-cache")], b).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[derive(Deserialize)]
struct SaveReq {
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    brushes: Vec<u64>,
    #[serde(default)]
    entities: Vec<u64>,
    /// a PNG of the view (base64, a data: URL is fine)
    thumb: Option<String>,
}

async fn api_save(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<SaveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let name = prefab::clean_text(&req.name, 64);
        if name.is_empty() {
            return Err("Give the prefab a name.".into());
        }
        if req.brushes.is_empty() && req.entities.is_empty() {
            return Err("Pick what goes in the prefab first.".into());
        }
        let thumb = match req.thumb.as_deref().map(|t| t.rsplit(',').next().unwrap_or(t).trim()) {
            Some(t) if !t.is_empty() => {
                if t.len() > 1_500_000 {
                    return Err("The thumbnail is too big.".into());
                }
                Some(base64::engine::general_purpose::STANDARD.decode(t).map_err(|_| "The thumbnail couldn't be read.".to_string())?)
            }
            _ => None,
        };
        let mut frag = {
            let g = ed.lock();
            let s = g.as_ref().ok_or(NO_DOC)?;
            let f = history::fragment(&s.doc, &super::bids(&req.brushes), &super::eids(&req.entities));
            let mut f = f;
            f.world.wads = s.doc.world.wads.clone();
            f
        };
        if frag.brushes.is_empty() && frag.entities.is_empty() {
            return Err("Those pieces aren't in the map any more.".into());
        }
        prefab::centre(&mut frag)?;
        let mut meta = prefab::describe(&frag, &name, &req.tags, &req.author, now_secs());
        // only the WADs the prefab's textures come from, when that's known
        if frag.materials.values().all(|m| m.wad.is_some()) && !frag.materials.is_empty() {
            let used: Vec<String> = frag.materials.values().filter_map(|m| m.wad.as_deref()).map(|w| w.rsplit(['\\', '/']).next().unwrap_or(w).to_ascii_lowercase()).collect();
            meta.wads.retain(|w| used.contains(w));
        }
        meta.description = prefab::clean_text(&req.description, 400);
        let id = save(&lib_dir(&ed.dir), &frag, &meta, thumb.as_deref())?;
        Ok(json!({"ok": true, "id": id, "name": meta.name}))
    })
    .await
}

#[derive(Deserialize)]
struct EditReq {
    id: String,
    name: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
}

async fn api_edit(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<EditReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = lib_dir(&ed.dir);
        let own = req.id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("Only your own prefabs can be changed. Save a copy first.")?.to_string();
        let path = dir.join(format!("{own}.json"));
        let mut m: PrefabMeta = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or("That prefab isn't there any more.")?;
        if let Some(n) = req.name {
            let n = prefab::clean_text(&n, 64);
            if n.is_empty() {
                return Err("Give the prefab a name.".into());
            }
            m.name = n;
        }
        if let Some(t) = req.tags {
            m.tags = prefab::clean_tags(&t);
        }
        if let Some(d) = req.description {
            m.description = prefab::clean_text(&d, 400);
        }
        write_file(&path, serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?.as_bytes())?;
        Ok(json!({"ok": true}))
    })
    .await
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

async fn api_delete(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<IdReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = lib_dir(&ed.dir);
        let own = req.id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("The starter prefabs can't be deleted.")?;
        let mut any = false;
        for ext in ["map", "json", "png"] {
            any |= std::fs::remove_file(dir.join(format!("{own}.{ext}"))).is_ok();
        }
        if !any {
            return Err("That prefab isn't there any more.".into());
        }
        Ok(json!({"ok": true}))
    })
    .await
}

async fn api_export(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<IdReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = lib_dir(&ed.dir);
        let (_, meta) = load(&dir, &req.id)?;
        let slug = req.id.split(':').nth(1).unwrap_or("prefab").to_string();
        let Some(out) = super::dialog(&st, super::Pick::ExportPrefab, &format!("{slug}-prefab"))? else { return Ok(json!({"cancelled": true})) };
        package(&dir, &req.id, &out)?;
        Ok(json!({"ok": true, "file": out.file_name().map(|n| n.to_string_lossy().to_string()), "name": meta.name}))
    })
    .await
}

async fn api_import(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game().map(|g| g.dir.clone());
    blocking(move || {
        let Some(file) = super::dialog(&st, super::Pick::ImportPrefab, "")? else { return Ok(json!({"cancelled": true})) };
        install_file(&lib_dir(&ed.dir), &ed.dir.join("work"), &file, game.as_deref())
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/prefabs", get(api_list))
        .route("/api/tools/editor/prefab/:id/thumb.png", get(api_thumb))
        .route("/api/tools/editor/prefabs/save", post(api_save))
        .route("/api/tools/editor/prefabs/edit", post(api_edit))
        .route("/api/tools/editor/prefabs/delete", post(api_delete))
        .route("/api/tools/editor/prefabs/export", post(api_export))
        .route("/api/tools/editor/prefabs/import", post(api_import))
}

/// For the community browser's Try it (tools/community.rs): installs the prefabs in an
/// unpacked download into the player's library, and its WADs into the game folder.
pub fn install_download(src: &Path, author: &str, game_dir: Option<&Path>) -> Result<Value, String> {
    let lib = lib_dir(&crate::sys::app_dir().join("editor"));
    let got = install_dir(&lib, src, author)?;
    let wads = game_dir.map(|g| install_wads(&got.wads, g)).unwrap_or_default();
    Ok(json!({"prefabs": got.prefabs, "ids": got.ids, "wads": wads, "skipped": got.skipped}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-prefabs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn library_save_list_load_delete_and_packages() {
        let root = tmp("lib");
        let lib = lib_dir(&root);
        let (frag, meta) = load(&lib, "builtin:crate_stack").unwrap();
        let png = prefab::thumbnail(&frag, 64, 48);
        let a = save(&lib, &frag, &meta, Some(&png)).unwrap();
        let b = save(&lib, &frag, &meta, None).unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("mine:crate_stack", "mine:crate_stack_2"));
        assert!(save(&lib, &frag, &meta, Some(b"nope")).is_err());
        let l = list(&lib);
        let items = l["prefabs"].as_array().unwrap();
        assert_eq!(items.iter().filter(|x| x["builtin"] == json!(false)).count(), 2);
        assert_eq!(items.iter().filter(|x| x["builtin"] == json!(true)).count(), 8);
        let (back, m) = load(&lib, &a).unwrap();
        assert_eq!(back.brushes.len(), frag.brushes.len());
        assert_eq!(m.name, "Crate stack");
        assert!(load(&lib, "mine:../../etc").is_err() && load(&lib, "builtin:nope").is_err() && load(&lib, "x").is_err());

        // a package round trip: zip -> unpack -> install into another library
        let zip = root.join("out.zip");
        package(&lib, &a, &zip).unwrap();
        let un = root.join("un");
        std::fs::create_dir_all(&un).unwrap();
        crate::archive::extract(&zip, &un, 64 << 20).unwrap();
        assert!(un.join("prefabs/crate_stack.map").is_file() && un.join("loner-strike.json").is_file());
        std::fs::write(un.join("prefabs/mytex.wad"), b"WAD3\0\0\0\0\0\0\0\0").unwrap();
        let lib2 = root.join("lib2");
        let got = install_dir(&lib2, &un, "someone").unwrap();
        assert_eq!(got.prefabs, vec!["Crate stack"]);
        assert_eq!(got.wads.len(), 1);
        assert!(lib2.join("crate_stack.png").is_file());
        let game = root.join("cstrike");
        std::fs::create_dir_all(&game).unwrap();
        assert_eq!(install_wads(&got.wads, &game), vec!["mytex.wad"]);
        // never over a file that's there
        assert!(install_wads(&got.wads, &game).is_empty());

        // a plain .map picked from the computer
        let raw = root.join("fence.map");
        std::fs::write(&raw, crate::editor::mapfile::write_map(&frag, &Default::default()).unwrap()).unwrap();
        let v = install_file(&lib2, &root.join("work"), &raw, None).unwrap();
        assert_eq!(v["prefabs"], json!(["fence"]));
        assert!(install_file(&lib2, &root.join("work"), &root.join("out.zip.nope"), None).is_err());
        // nothing usable
        let empty = root.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("readme.txt"), "hi").unwrap();
        assert!(install_dir(&lib2, &empty, "").is_err());

        // delete: only your own
        let gone = mine(&lib).len();
        std::fs::remove_file(lib.join("crate_stack_2.map")).unwrap();
        assert_eq!(mine(&lib).len(), gone - 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
