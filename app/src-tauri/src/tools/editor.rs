//! The map editor's routes (`/api/tools/editor/*`, the app window's listener only), on top
//! of agent E's core in `crate::editor`. One open document per app run, held here with its
//! undo history; the page asks for what changed by revision (TOOLS-PLAN §3.8).
//!
//! Files are never named by the page: open, import, save-as and export go through the
//! system's file dialog, and Save writes back to the file the document came from.
//! JSON routes answer `{error}` with 200 (400 when the request itself is malformed), like the
//! rest of the app; POSTs need the panel header. Nothing here panics on bad input: every
//! number is checked, and the work runs off the async threads (a panic there becomes the
//! internal-error message, and the session lock survives it).

use crate::editor::checks;
use crate::editor::extrude; // Parity A
use crate::editor::shear; // Parity A
use crate::editor::fgd;
use crate::editor::shapes;
use crate::editor::vertex;
use crate::editor::doc::{Brush, BrushId, Entity, EntityId, GroupId, MapDocument, Material, WorldSettings};
use crate::editor::geom::brush_bounds;
use crate::editor::history::{self, FixIt, History, Keep, Tx};
use crate::editor::mapfile;
use crate::editor::math::{v3, Affine, Bounds, Mat3, Plane, Vec3, BIG};
use crate::editor::ops;
use crate::editor::texture;
use crate::goldsrc::{self, mesh::Section, wad};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

pub mod components;
pub mod prefabs;
pub mod assets; // P5: the models, sprites, sounds and sky the map names
pub mod budget; // P5: how close the map is to the engine's limits
pub mod leak; // P5: the leak line, its hole and the brushes round it
pub mod io; // P5: the entity I/O view's data
pub mod browser; // P5: the Content Browser (models, sprites, sounds)
pub mod kits; // P5: mode map kits (what each mode the map is made for needs)
pub mod workspace; // P5: project files, undo history, find/replace, duplicate with new names
pub mod heavy; // Checks: heavy views from the last compile's VIS data
pub mod npcs; // NPC-editor: Checks for NPCs and interactables
pub mod selection; // MCP: what's selected, for agents
pub mod importexport; // Parity A: the Content Browser's Import and Export

const INTERNAL: &str = "Loner-Strike hit an internal error in the map editor. Try again, and copy the diagnostics from Settings if it keeps happening.";
const NO_DOC: &str = "No map is open in the editor. Start a new map or open one.";
const NOT_UNDERSTOOD: &str = "That edit wasn't understood.";
const MAX_FILE: u64 = 64 * 1024 * 1024;
/// Removed-brush notes kept for `meshes?since`; older asks get everything again.
const MAX_REMOVED: usize = 20_000;
const PROJECT_EXT: &str = "lsmap";

// ---- the session ----------------------------------------------------------------------

/// What a drag started from: the brushes as they were, so each step of a push/pull is
/// computed from the start (face numbers can't shift under it) and folds into one undo step.
struct Drag {
    key: String,
    brushes: BTreeMap<BrushId, Brush>,
    /// Parity A: the brush an extrude drag has made so far (the next step reshapes it)
    made: Option<BrushId>,
}

struct Session {
    /// Changes whenever another document is opened, so the page knows to start over.
    id: u64,
    doc: MapDocument,
    history: History,
    name: String,
    /// Where Save writes (a project file). None until saved or opened from a project.
    project: Option<PathBuf>,
    /// The folder the map came from (the project's, or an imported .map/.rmf/.jmf's): texture
    /// files there are found before the game's (P4, QA-1).
    folder: Option<PathBuf>,
    /// The revision each brush last changed at, and brushes removed since.
    stamps: HashMap<BrushId, u64>,
    removed: Vec<(BrushId, u64)>,
    /// Asks older than this get the whole map (removed notes were dropped).
    horizon: u64,
    /// P5 (perf): the revision each entity last changed at, and entities removed since (for
    /// the document changes each command answers with)
    estamps: HashMap<EntityId, u64>,
    eremoved: Vec<(EntityId, u64)>,
    /// P5 (perf): each brush's bounds, kept as brushes change (the document answer used to
    /// work them out for every brush every time)
    bounds: HashMap<BrushId, Bounds>,
    drag: Option<Drag>,
    wads: Option<(Vec<String>, Vec<PathBuf>, Arc<wad::WadSet>)>,
    /// P5: the folder the map was imported from in this run (assets.rs searches it)
    import_dir: Option<PathBuf>,
}

impl Session {
    fn new(id: u64, mut doc: MapDocument, name: String, project: Option<PathBuf>) -> Session {
        // P5: every document has a permanent uid (a new map, an import, an old project)
        if !doc.editor.uid.as_deref().is_some_and(crate::editor::doc::valid_uid) {
            doc.editor.uid = Some(crate::editor::doc::new_uid());
        }
        let rev = doc.revision;
        let stamps = doc.brushes.keys().map(|b| (*b, rev)).collect();
        let estamps = doc.entities.keys().map(|e| (*e, rev)).collect();
        let bounds = doc.brushes.iter().map(|(id, b)| (*id, brush_bounds(b))).collect();
        doc.take_touched();
        let folder = project.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        Session {
            id, doc, history: History::default(), name, project, folder, stamps, removed: Vec::new(), horizon: 0, estamps, eremoved: Vec::new(), bounds,
            drag: None, wads: None, import_dir: None,
        }
    }

    /// Notes which brushes and entities changed since the last look. P5 (perf): from the
    /// document's own change notes (history writes them as it edits), not by comparing and
    /// copying the whole map after every command.
    fn track(&mut self) {
        let rev = self.doc.revision;
        let t = self.doc.take_touched();
        for id in t.brushes {
            if let Some(b) = self.doc.brushes.get(&id) {
                self.stamps.insert(id, rev);
                self.bounds.insert(id, brush_bounds(b));
            } else if self.stamps.remove(&id).is_some() {
                self.bounds.remove(&id);
                self.removed.push((id, rev));
            }
        }
        for id in t.entities {
            if self.doc.entities.contains_key(&id) {
                self.estamps.insert(id, rev);
            } else if self.estamps.remove(&id).is_some() {
                self.eremoved.push((id, rev));
            }
        }
        if self.removed.len() > MAX_REMOVED {
            let cut = self.removed.len() - MAX_REMOVED / 2;
            self.horizon = self.horizon.max(self.removed[cut - 1].1);
            self.removed.drain(..cut);
        }
        if self.eremoved.len() > MAX_REMOVED {
            let cut = self.eremoved.len() - MAX_REMOVED / 2;
            self.horizon = self.horizon.max(self.eremoved[cut - 1].1);
            self.eremoved.drain(..cut);
        }
    }

    /// The document's WADs, opened (kept until the WAD list or the game changes).
    fn wads(&mut self, dirs: &[PathBuf]) -> Option<Arc<wad::WadSet>> {
        if dirs.is_empty() && self.folder.is_none() {
            return None;
        }
        // the map's own folder first, then the game's
        let all: Vec<PathBuf> = self.folder.iter().cloned().chain(dirs.iter().cloned()).collect();
        let dirs = &all[..];
        let names = self.doc.world.wad_names();
        if let Some((n, d, set)) = &self.wads {
            if *n == names && d == dirs {
                return Some(set.clone());
            }
        }
        let set = Arc::new(wad::WadSet::open(&crate::mapcopy::Finder::default(), dirs, &names));
        self.wads = Some((names, dirs.to_vec(), set.clone()));
        Some(set)
    }

    /// Learns the pixel size of textures the map uses (for alignment and drawing) from its
    /// WADs; brushes using a newly sized texture are redrawn.
    fn learn_textures(&mut self, dirs: &[PathBuf]) {
        let Some(set) = self.wads(dirs) else { return };
        let used: HashSet<String> = self.doc.brushes.values().flat_map(|b| b.faces.iter().map(|f| f.texture.to_ascii_lowercase())).collect();
        let mut learned = HashSet::new();
        for name in used {
            if self.doc.material(&name).is_some() {
                continue;
            }
            if let Some((w, h, from)) = texture_size(&set, &name) {
                self.doc.add_material(Material { name: name.clone(), width: w, height: h, wad: Some(from) });
                learned.insert(name);
            }
        }
        if learned.is_empty() {
            return;
        }
        self.doc.revision += 1;
        let rev = self.doc.revision;
        for (id, b) in &self.doc.brushes {
            if b.faces.iter().any(|f| learned.contains(&f.texture.to_ascii_lowercase())) {
                self.stamps.insert(*id, rev);
            }
        }
    }

    fn state(&self) -> Value {
        json!({
            "docId": self.id,
            "revision": self.doc.revision,
            "name": self.name,
            "saved": self.project.is_some(),
            "dirty": self.history.is_dirty(),
            "undo": self.history.undo_label(),
            "redo": self.history.redo_label(),
        })
    }
}

/// The open document (one per app run) and a few caches.
pub struct Editor {
    session: Mutex<Option<Session>>,
    /// Where autosaves, backups and the recent list live.
    dir: PathBuf,
    next_id: AtomicU64,
    /// Texture pictures and lists, by WAD stamp.
    pngs: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    lists: Mutex<HashMap<String, Arc<Value>>>,
    /// Checks: the last answer, by (document, revision, scale limits)
    checks_last: Mutex<Option<((u64, u64, u64, u64), Value)>>,
}

impl Editor {
    fn with_dir(dir: PathBuf) -> Editor {
        Editor { session: Mutex::new(None), dir, next_id: AtomicU64::new(1), pngs: Mutex::default(), lists: Mutex::default(), checks_last: Mutex::default() }
    }
    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn install(&self, doc: MapDocument, name: String, project: Option<PathBuf>, dirs: &[PathBuf]) -> Value {
        self.install_from(doc, name, project, None, dirs)
    }
    /// As `install`, remembering the file it came from (its folder is searched for WADs).
    fn install_from(&self, doc: MapDocument, name: String, project: Option<PathBuf>, source: Option<&Path>, dirs: &[PathBuf]) -> Value {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut s = Session::new(id, doc, name, project);
        if let Some(dir) = source.and_then(Path::parent) {
            s.folder = Some(dir.to_path_buf());
            // P5: where its models/sprites/sounds are looked for, also after Save (this run only:
            // never read from a project file, which could point anywhere)
            s.import_dir = Some(dir.to_path_buf());
        }
        s.learn_textures(dirs);
        s.track();
        let st = s.state();
        *self.lock() = Some(s);
        st
    }
}

type Ed = Arc<Editor>;

fn global() -> Ed {
    static ED: OnceLock<Ed> = OnceLock::new();
    ED.get_or_init(|| Arc::new(Editor::with_dir(crate::sys::app_dir().join("editor")))).clone()
}

/// The folders WADs are looked for in (none when the game isn't set up).
fn wad_dirs(st: &Shared) -> Vec<PathBuf> {
    st.local_game().map(|g| wad::search_dirs(&g.hl, g.is_czero())).unwrap_or_default()
}

/// A texture's size from the first WAD that has it (only the 40-byte header is read).
fn texture_size(set: &wad::WadSet, name: &str) -> Option<(u32, u32, String)> {
    let key = name.to_ascii_lowercase();
    for (wad_name, w) in &set.wads {
        let Ok(w) = w else { continue };
        if !w.has_texture(&key) {
            continue;
        }
        let e = w.entries.get(&key)?;
        if let Some((tw, th)) = miptex_size(&w.path, e.offset) {
            return Some((tw, th, wad_name.clone()));
        }
    }
    None
}

fn miptex_size(path: &Path, offset: u32) -> Option<(u32, u32)> {
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(offset as u64)).ok()?;
    let mut h = [0u8; 24];
    f.read_exact(&mut h).ok()?;
    let w = u32::from_le_bytes(h[16..20].try_into().ok()?);
    let hh = u32::from_le_bytes(h[20..24].try_into().ok()?);
    ((1..=4096).contains(&w) && (1..=4096).contains(&hh)).then_some((w, hh))
}

fn wads_stamp(set: &wad::WadSet) -> String {
    let mut parts = Vec::new();
    for (n, w) in &set.wads {
        match w {
            Ok(w) => {
                let m = std::fs::metadata(&w.path).ok();
                let t = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                parts.push(format!("{}:{}:{t}", w.path.display(), m.map_or(0, |m| m.len())));
            }
            Err(_) => parts.push(format!("{n}:missing")),
        }
    }
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    super::cache::short_hash(&refs)
}

// ---- answers --------------------------------------------------------------------------

fn ok(v: Value) -> Response {
    Json(v).into_response()
}
fn fail(msg: impl Into<String>) -> Response {
    Json(json!({"error": msg.into()})).into_response()
}
fn bad(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg.into()}))).into_response()
}
fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => ok(v),
        Ok(Err(e)) => fail(e),
        Err(_) => fail(INTERNAL),
    }
}

// ---- the starter map ------------------------------------------------------------------

/// "New map": a 1024 x 1024 open-sky room with two spawns a side and a sun.
pub fn starter_map() -> MapDocument {
    let mut d = MapDocument::new();
    d.world = WorldSettings { wads: vec!["halflife.wad".into(), "cstrike.wad".into()], keys: vec![("skyname".into(), "desert".into())] };
    let (lo, hi, top, t) = (-512.0, 512.0, 256.0, 16.0);
    let slabs = [
        (v3(lo - t, lo - t, -t), v3(hi + t, hi + t, 0.0), "CRETE3_FLR04"),
        (v3(lo - t, lo - t, top), v3(hi + t, hi + t, top + t), "SKY"),
        (v3(lo - t, lo, 0.0), v3(lo, hi, top), "OUT_WALL5"),
        (v3(hi, lo, 0.0), v3(hi + t, hi, top), "OUT_WALL5"),
        (v3(lo - t, lo - t, 0.0), v3(hi + t, lo, top), "OUT_WALL5"),
        (v3(lo - t, hi, 0.0), v3(hi + t, hi + t, top), "OUT_WALL5"),
    ];
    for (a, b, tex) in slabs {
        d.insert_brush(ops::make_box(a, b, tex));
    }
    let spawn = |class: &str, x: f64, y: f64, angle: &str| {
        let mut e = Entity::new(class);
        e.set("origin", &format!("{x} {y} 36"));
        e.set("angle", angle);
        e
    };
    for y in [-64.0, 64.0] {
        d.insert_entity(spawn("info_player_start", -384.0, y, "0"));
        d.insert_entity(spawn("info_player_deathmatch", 384.0, y, "180"));
    }
    let mut sun = Entity::new("light_environment");
    sun.set("origin", "0 0 192");
    sun.set("pitch", "-60");
    sun.set("angle", "45");
    sun.set("_light", "255 248 230 220");
    d.insert_entity(sun);
    d
}

// ---- reading the document -------------------------------------------------------------

fn vec_json(p: Vec3) -> Value {
    json!([p.x, p.y, p.z])
}

fn brush_json(s: &Session, id: BrushId, b: &Brush) -> Value {
    let bb = s.bounds.get(&id).cloned().unwrap_or_else(|| brush_bounds(b));
    json!({"id": id.0, "entity": b.entity.map(|e| e.0), "group": b.group.map(|g| g.0), "min": vec_json(bb.min), "max": vec_json(bb.max)})
}

fn entity_json(id: EntityId, e: &crate::editor::doc::Entity, owned: &HashMap<EntityId, Vec<u64>>) -> Value {
    json!({"id": id.0, "classname": e.classname(), "keys": e.keys, "group": e.group.map(|g| g.0), "brushes": owned.get(&id).cloned().unwrap_or_default()})
}

/// The parts of the document answer that are small and always sent whole: world, settings,
/// groups, the map's bounds and its counts.
fn doc_common(s: &Session, v: &mut Value) {
    let d = &s.doc;
    // P5 (perf): bounds from the kept per-brush bounds
    let mut bounds = Bounds::empty();
    for bb in s.bounds.values() {
        if !bb.is_empty() {
            bounds = bounds.union(bb);
        }
    }
    for e in d.entities.values() {
        if let Some(o) = e.origin() {
            bounds.add(o);
        }
    }
    if bounds.is_empty() {
        bounds = Bounds { min: v3(-512.0, -512.0, 0.0), max: v3(512.0, 512.0, 256.0) };
    }
    let groups: Vec<Value> = d.groups.iter().map(|(id, g)| json!({"id": id.0, "name": g.name, "parent": g.parent.map(|p| p.0), "hidden": g.hidden, "locked": g.locked})).collect();
    v["world"] = json!({"wads": d.world.wads, "keys": d.world.keys});
    v["editor"] = settings_json(d);
    v["groups"] = Value::Array(groups);
    v["bounds"] = json!([vec_json(bounds.min), vec_json(bounds.max)]);
    v["stats"] = serde_json::to_value(checks::stats(d)).unwrap_or(Value::Null);
}

fn doc_json(s: &Session) -> Value {
    let d = &s.doc;
    let mut owned: HashMap<EntityId, Vec<u64>> = HashMap::new();
    let brushes: Vec<Value> = d
        .brushes
        .iter()
        .map(|(id, b)| {
            if let Some(e) = b.entity {
                owned.entry(e).or_default().push(id.0);
            }
            brush_json(s, *id, b)
        })
        .collect();
    let entities: Vec<Value> = d.entities.iter().map(|(id, e)| entity_json(*id, e, &owned)).collect();
    let mut v = s.state();
    v["brushes"] = Value::Array(brushes);
    v["entities"] = Value::Array(entities);
    doc_common(s, &mut v);
    v
}

/// P5 (perf): what changed in the document since revision `since`: the changed and removed
/// brushes and entities (in the document answer's own shape) plus the small parts whole.
/// None when that's too far back (removed notes dropped) or ahead: the page asks for it all.
///   {docId, from, revision, brushes[], removedBrushes[], entities[], removedEntities[], groups,
///    world, editor, bounds, stats} (+ the state: name, dirty, undo, redo…)
fn changes_json(s: &Session, since: u64) -> Option<Value> {
    if since < s.horizon || since > s.doc.revision {
        return None;
    }
    let d = &s.doc;
    let brushes: Vec<Value> = d.brushes.iter().filter(|(id, _)| s.stamps.get(id).is_some_and(|r| *r > since)).map(|(id, b)| brush_json(s, *id, b)).collect();
    let changed_ents: Vec<EntityId> = s.estamps.iter().filter(|(_, r)| **r > since).map(|(e, _)| *e).collect();
    let mut owned: HashMap<EntityId, Vec<u64>> = HashMap::new();
    if !changed_ents.is_empty() {
        let want: HashSet<EntityId> = changed_ents.iter().copied().collect();
        for (id, b) in &d.brushes {
            if let Some(e) = b.entity.filter(|e| want.contains(e)) {
                owned.entry(e).or_default().push(id.0);
            }
        }
    }
    let mut entities: Vec<Value> = changed_ents.iter().filter_map(|id| d.entities.get(id).map(|e| (id, e))).map(|(id, e)| entity_json(*id, e, &owned)).collect();
    entities.sort_by_key(|e| e["id"].as_u64());
    let mut v = s.state();
    v["from"] = json!(since);
    v["brushes"] = Value::Array(brushes);
    v["removedBrushes"] = json!(s.removed.iter().filter(|(_, r)| *r > since).map(|(id, _)| id.0).collect::<Vec<_>>());
    v["entities"] = Value::Array(entities);
    v["removedEntities"] = json!(s.eremoved.iter().filter(|(_, r)| *r > since).map(|(id, _)| id.0).collect::<Vec<_>>());
    doc_common(s, &mut v);
    Some(v)
}

/// The brush meshes changed since `since` (all of them when `since` is missing, too old, or for
/// another document), as an LSMB bundle: header `brushes[{id, entity, faces[{face, texture,
/// normal, start, count}]}]` (start/count in vertices: one polygon per face, drawn as a fan)
/// and `removed[]`; each face's `align` is `[u axis x3, u offset, v axis x3, v offset,
/// rotation, u scale, v scale]` (Valve 220); sections `pos` (f32 x3, GoldSrc units) and `uv` (f32 x2, divided by the
/// texture size).
///
/// P5 (perf): what a meshes answer needs, copied under a brief lock (the changed brushes and
/// the texture sizes); the meshes are built off it, so edits and other asks don't wait for them.
struct MeshSnap {
    id: u64,
    revision: u64,
    full: bool,
    brushes: Vec<(BrushId, Brush)>,
    removed: Vec<u64>,
    materials: BTreeMap<String, Material>,
}

impl MeshSnap {
    fn take(s: &Session, doc_id: Option<u64>, since: Option<u64>) -> MeshSnap {
        let full = since.is_none();
        let since = since.unwrap_or(0);
        let full = full || doc_id != Some(s.id) || since < s.horizon || since > s.doc.revision;
        let brushes = s.doc.brushes.iter().filter(|(id, _)| full || s.stamps.get(id).is_some_and(|r| *r > since)).map(|(id, b)| (*id, b.clone())).collect();
        let removed = if full { Vec::new() } else { s.removed.iter().filter(|(_, r)| *r > since).map(|(id, _)| id.0).collect() };
        MeshSnap { id: s.id, revision: s.doc.revision, full, brushes, removed, materials: s.doc.materials.clone() }
    }

    fn build(self) -> Vec<u8> {
        let size = |t: &str| self.materials.get(&t.to_ascii_lowercase()).map(|m| (m.width.max(1) as f64, m.height.max(1) as f64)).unwrap_or((64.0, 64.0));
        let mut pos: Vec<f32> = Vec::new();
        let mut uv: Vec<f32> = Vec::new();
        let mut list = Vec::new();
        for (id, b) in &self.brushes {
            let faces: Vec<Value> = ops::brush_mesh(b, size)
                .into_iter()
                .map(|f| {
                    let a = &b.faces[f.face].align;
                    let start = pos.len() / 3;
                    for (p, (u, v)) in f.vertices.iter().zip(&f.uvs) {
                        pos.extend([p.x as f32, p.y as f32, p.z as f32]);
                        uv.extend([*u as f32, *v as f32]);
                    }
                    json!({"face": f.face, "texture": f.texture, "normal": vec_json(f.normal), "start": start, "count": f.vertices.len(),
                        "align": [a.u_axis.x, a.u_axis.y, a.u_axis.z, a.u_offset, a.v_axis.x, a.v_axis.y, a.v_axis.z, a.v_offset, a.rotation, a.scale_u, a.scale_v]})
                })
                .collect();
            list.push(json!({"id": id.0, "entity": b.entity.map(|e| e.0), "group": b.group.map(|g| g.0), "faces": faces}));
        }
        let header = json!({"format": "LSMB", "kind": "editor", "docId": self.id, "revision": self.revision, "full": self.full, "removed": self.removed, "brushes": list});
        goldsrc::mesh::write_bundle(header, &[Section::f32("pos", 3, &pos), Section::f32("uv", 2, &uv)])
    }
}

/// Checks: what the checks need, taken under a brief lock so they run off it (on br_valley they
/// take ~0.4 s; edits mustn't wait for them).
struct CheckSnap {
    id: u64,
    revision: u64,
    doc: MapDocument,
    name: String,
    wads: Option<Arc<wad::WadSet>>,
    places: assets::Places,
}

impl CheckSnap {
    fn take(s: &mut Session, st: &Shared, dirs: &[PathBuf]) -> CheckSnap {
        CheckSnap { id: s.id, revision: s.doc.revision, doc: s.doc.clone(), name: s.name.clone(), wads: s.wads(dirs), places: assets::Places::of(st, s) }
    }
}

/// Checks: every check on a snapshot, with `opts` for the deeper ones (map name, scale limits,
/// VIS data). Runs without the editor's lock.
fn checks_json(c: &CheckSnap, places: Option<&assets::Places>, opts: &checks::Options) -> Value {
    let known: Option<HashSet<String>> = c.wads.as_ref().map(|set| {
        let mut k = HashSet::new();
        for w in set.wads.iter().filter_map(|(_, w)| w.as_ref().ok()) {
            k.extend(w.entries.iter().filter(|(n, _)| w.has_texture(n)).map(|(n, _)| n.clone()));
        }
        k
    });
    let opts = checks::Options { map: if opts.map.is_empty() { c.name.clone() } else { opts.map.clone() }, ..opts.clone() };
    let mut issues = checks::run_with(&c.doc, known.as_ref(), &opts);
    // P5: models, sprites, sounds and the sky the entities name
    if let Some(p) = places {
        issues.extend(assets::issues(&assets::locate(assets::named_files(&c.doc), p), p.hl.is_some()));
        // and what each mode the map is made for still needs (mode map kits)
        issues.extend(kits::issues(&c.doc, &kits::cached()));
        // creature types this game doesn't have (shared creatures, CP-ui)
        issues.extend(super::creatures::editor_issues(&c.doc, super::creatures::game_ini(p.hl.as_deref(), &p.game).as_deref()));
    }
    // NPCs and interactables: a name, something to see (NPC-editor)
    issues.extend(npcs::issues(&c.doc));
    json!({"docId": c.id, "revision": c.revision, "issues": issues, "stats": checks::stats(&c.doc), "texturesChecked": known.is_some()})
}

// ---- commands -------------------------------------------------------------------------

type P3 = [f64; 3];

fn p3(p: P3) -> Result<Vec3, String> {
    let v = v3(p[0], p[1], p[2]);
    if v.is_finite() && v.abs().max_elem() <= BIG {
        Ok(v)
    } else {
        Err("That point is outside the map.".into())
    }
}

fn num(x: f64, lo: f64, hi: f64, what: &str) -> Result<f64, String> {
    if x.is_finite() && x >= lo && x <= hi {
        Ok(x)
    } else {
        Err(format!("The {what} is out of range."))
    }
}

fn texture_name(t: &str) -> Result<String, String> {
    let t = t.trim();
    if t.is_empty() || t.len() > 15 || !t.bytes().all(|c| c.is_ascii_graphic() && c != b'"') {
        return Err("That isn't a texture name.".into());
    }
    Ok(t.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum KeepSide {
    Back,
    Front,
    Both,
}

#[derive(Deserialize)]
struct Rotate {
    axis: P3,
    degrees: f64,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Cmd {
    AddBox { min: P3, max: P3, texture: String },
    AddCylinder { base: P3, radius: f64, height: f64, sides: usize, texture: String, #[serde(default)] grid: f64 },
    /// wedge, spike, cone, sphere, arch or torus, filling the box `min`..`max`
    AddShape {
        shape: String,
        min: P3,
        max: P3,
        #[serde(default = "twelve")] sides: usize,
        texture: String,
        #[serde(default)] grid: f64,
        #[serde(default = "half_turn")] arc: f64,
        #[serde(default)] start: f64,
        #[serde(default = "sixteen")] wall: f64,
        /// Parity A: arch/torus pieces climb this much each (spirals)
        #[serde(default)] add_height: f64,
        /// Parity A: a cone's top as a share of its base (0 = a point)
        #[serde(default)] top: f64,
        /// Parity A: a torus's tube sides
        #[serde(default = "eight")] tube_sides: usize,
    },
    MoveVertices { picks: Vec<(u64, P3)>, delta: P3, drag: Option<String> },
    /// Parity A: pull a new brush out of a face (Shift-drag); steps of one drag reshape it
    ExtrudeFace { brush: u64, face: usize, distance: f64, drag: Option<String> },
    /// Parity A: a new corner at `at` + `delta` (split an edge or a face); steps of one drag move it
    AddVertex { brush: u64, at: P3, delta: P3, drag: Option<String> },
    /// Parity A: these corners of one brush become one, at their middle
    MergeVertices { brush: u64, picks: Vec<P3> },
    /// Parity A: slide coordinate `along` by `factor` × (coordinate `axis` − the pivot's)
    Shear {
        #[serde(default)] brushes: Vec<u64>,
        #[serde(default)] entities: Vec<u64>,
        axis: usize,
        along: usize,
        factor: f64,
        pivot: P3,
        #[serde(default = "yes")] lock: bool,
        #[serde(default)] keep_scale: bool,
    },
    /// paste special: copies, each moved, turned (degrees about x, y, z) and stretched from the last
    Array {
        #[serde(default)] brushes: Vec<u64>,
        #[serde(default)] entities: Vec<u64>,
        copies: usize,
        #[serde(default)] offset: P3,
        #[serde(default)] rotate: P3,
        scale: Option<P3>,
        pivot: Option<P3>,
        #[serde(default = "yes")] lock: bool,
    },
    Paste { text: String, #[serde(default)] offset: P3 },
    Fix { fixes: Vec<FixReq>, with: Option<String> },
    Delete { #[serde(default)] brushes: Vec<u64>, #[serde(default)] entities: Vec<u64> },
    Duplicate { #[serde(default)] brushes: Vec<u64>, #[serde(default)] entities: Vec<u64>, offset: P3 },
    Transform {
        #[serde(default)] brushes: Vec<u64>,
        #[serde(default)] entities: Vec<u64>,
        translate: Option<P3>,
        rotate: Option<Rotate>,
        scale: Option<P3>,
        pivot: Option<P3>,
        #[serde(default = "yes")] lock: bool,
        /// P5: texture lock keeps the texture's scale (a stretch repeats it, Hammer's scale lock off)
        #[serde(default)] keep_scale: bool,
        /// P5: joins the step before it with the same key (a Shift-drag copy's move)
        #[serde(default)] merge: Option<String>,
    },
    MoveFace { brush: u64, face: usize, distance: f64, #[serde(default = "yes")] lock: bool, drag: Option<String> },
    Clip { brushes: Vec<u64>, points: [P3; 3], keep: KeepSide, texture: Option<String> },
    Hollow { brushes: Vec<u64>, thickness: f64 },
    Carve { cutter: u64, targets: Vec<u64> },
    Merge { brushes: Vec<u64> },
    Snap { brushes: Vec<u64>, grid: f64 },
    SetTexture { faces: Vec<(u64, Option<usize>)>, texture: String },
    Align {
        faces: Vec<(u64, usize)>,
        action: String,
        #[serde(default)] u: f64,
        #[serde(default)] v: f64,
        #[serde(default)] degrees: f64,
        drag: Option<String>,
        /// "set": the face sheet's numbers, each only when given
        ou: Option<f64>,
        ov: Option<f64>,
        su: Option<f64>,
        sv: Option<f64>,
        rot: Option<f64>,
        /// "justify": left, right, top, bottom, center, fit
        mode: Option<String>,
        /// treat the faces as one surface (justify, fit, centre, scale and turn round their middle)
        #[serde(default)] one: bool,
        /// P5 "view": the camera's right and up (GoldSrc)
        right: Option<P3>,
        up: Option<P3>,
    },
    /// P5: Hammer's Alt+right-click: the texture and the alignment of `from` onto `faces`
    WrapTexture { from: (u64, usize), faces: Vec<(u64, usize)>, texture: Option<String> },
    SetKey { entity: u64, key: String, value: String },
    AddEntity { classname: String, origin: P3, #[serde(default)] keys: Vec<(String, String)> },
    Tie { brushes: Vec<u64>, classname: String },
    ToWorld { entity: u64 },
    SetWorld { wads: Option<Vec<String>>, keys: Option<Vec<(String, String)>> },
    Group { name: String, #[serde(default)] brushes: Vec<u64>, #[serde(default)] entities: Vec<u64> },
    Ungroup { group: u64 },
    GroupFlags { group: u64, hidden: Option<bool>, locked: Option<bool> },
    /// a prefab from the library dropped at `point` on a surface facing `normal`
    PlacePrefab {
        prefab: String,
        point: P3,
        #[serde(default = "up")] normal: P3,
        #[serde(default)] grid: f64,
        /// degrees to turn it about z (on top of facing out of a wall)
        #[serde(default)] yaw: f64,
        /// on a wall, turn it so its front faces out (default on)
        #[serde(default = "yes")] face_out: bool,
        /// filled in by api_cmd from the library before the session is locked
        #[serde(skip)] loaded: Option<Box<(MapDocument, crate::editor::prefab::PrefabMeta)>>,
    },
}

fn up() -> P3 {
    [0.0, 0.0, 1.0]
}

fn yes() -> bool {
    true
}
fn twelve() -> usize {
    12
}
fn half_turn() -> f64 {
    180.0
}
fn eight() -> usize {
    8
}

fn sixteen() -> f64 {
    16.0
}

#[derive(Deserialize)]
struct FixReq {
    fix: String,
    brush: Option<u64>,
    entity: Option<u64>,
    texture: Option<String>,
    /// Checks: the face and details of the deeper checks' fixes
    #[serde(default)]
    face: Option<usize>,
    #[serde(default, rename = "fixData")]
    data: Option<checks::FixData>,
}

fn bids(v: &[u64]) -> Vec<BrushId> {
    v.iter().map(|i| BrushId(*i)).collect()
}
fn eids(v: &[u64]) -> Vec<EntityId> {
    v.iter().map(|i| EntityId(*i)).collect()
}

fn need_brushes(d: &MapDocument, ids: &[BrushId]) -> Result<(), String> {
    if ids.is_empty() {
        return Err("Pick a brush first.".into());
    }
    if ids.len() > 100_000 {
        return Err("That's too many brushes at once.".into());
    }
    if ids.iter().any(|i| !d.brushes.contains_key(i)) {
        return Err("That brush no longer exists.".into());
    }
    Ok(())
}

fn safe_key(k: &str, v: &str) -> Result<(), String> {
    if k.is_empty() || k.len() > 64 || v.len() > 2048 || k.contains(['"', '\n', '\r']) || v.contains(['"', '\n', '\r']) {
        return Err("Keys and values can't be empty, very long or contain quotes.".into());
    }
    Ok(())
}

/// The selection's middle (brush bounds and entity origins), the default pivot.
fn centre_of(d: &MapDocument, brushes: &[BrushId], entities: &[EntityId]) -> Vec3 {
    let mut b = Bounds::empty();
    for id in brushes {
        if let Some(br) = d.brushes.get(id) {
            let bb = brush_bounds(br);
            if !bb.is_empty() {
                b = b.union(&bb);
            }
        }
    }
    for id in entities {
        if let Some(o) = d.entities.get(id).and_then(Entity::origin) {
            b.add(o);
        }
    }
    if b.is_empty() {
        Vec3::ZERO
    } else {
        b.center()
    }
}

/// Runs one command: one undo step, or nothing at all when it fails.
fn run_cmd(s: &mut Session, cmd: Cmd) -> Result<Value, String> {
    // a drag continues only while the same drag sends steps
    let drag_key = match &cmd {
        Cmd::MoveFace { drag: Some(k), .. } | Cmd::MoveVertices { drag: Some(k), .. } | Cmd::ExtrudeFace { drag: Some(k), .. } | Cmd::AddVertex { drag: Some(k), .. } => Some(k.clone()),
        _ => None,
    };
    if s.drag.as_ref().map(|d| Some(&d.key) != drag_key.as_ref()).unwrap_or(false) {
        s.drag = None;
    }
    let d = &mut s.doc;
    let h = &mut s.history;
    let mut created: (Vec<u64>, Vec<u64>) = (Vec::new(), Vec::new());
    let mut note: Option<String> = None;
    match cmd {
        Cmd::AddBox { min, max, texture } => {
            let (a, b) = (p3(min)?, p3(max)?);
            let size = (b - a).abs();
            if size.x < 1.0 || size.y < 1.0 || size.z < 1.0 {
                return Err("The box is too thin: drag out a bigger one.".into());
            }
            let id = history::cmd_add_brush(d, h, ops::make_box(a, b, &texture_name(&texture)?), "Add box");
            created.0.push(id.0);
        }
        Cmd::AddCylinder { base, radius, height, sides, texture, grid } => {
            let c = ops::make_cylinder(p3(base)?, num(radius, 1.0, BIG, "radius")?, num(height, 1.0, BIG, "height")?, sides, num(grid, 0.0, 1024.0, "grid")?, &texture_name(&texture)?)?;
            created.0.push(history::cmd_add_brush(d, h, c, "Add cylinder").0);
        }
        Cmd::AddShape { shape, min, max, sides, texture, grid, arc, start, wall, add_height, top, tube_sides } => {
            let (a, b) = (p3(min)?, p3(max)?);
            let tex = texture_name(&texture)?;
            let grid = num(grid, 0.0, 1024.0, "grid")?;
            let (lo, hi) = (a.min(b), a.max(b));
            let made: Vec<Brush> = match shape.as_str() {
                "wedge" => vec![shapes::make_wedge(a, b, &tex)?],
                "spike" => vec![shapes::make_spike(a, b, sides, grid, &tex)?],
                "sphere" => vec![shapes::make_sphere(a, b, sides, &tex)?],
                "arch" => {
                    let c = (lo + hi) * 0.5;
                    let arch = shapes::Arch {
                        centre: v3(c.x, c.y, lo.z),
                        radius_x: (hi.x - lo.x) / 2.0,
                        radius_y: (hi.y - lo.y) / 2.0,
                        wall: num(wall, 1.0, BIG, "wall thickness")?,
                        height: hi.z - lo.z,
                        arc: num(arc, 1.0, 360.0, "arc")?,
                        start: num(start, -360.0, 360.0, "start angle")?,
                        sides,
                        add_height: num(add_height, -4096.0, 4096.0, "add height")?,
                    };
                    shapes::make_arch(&arch, grid, &tex)?
                }
                "cone" => vec![shapes::make_cone(a, b, sides, num(top, 0.0, 0.95, "top size")?, grid, &tex)?],
                "torus" => {
                    let c = (lo + hi) * 0.5;
                    let torus = shapes::Torus {
                        centre: v3(c.x, c.y, lo.z),
                        radius_x: (hi.x - lo.x) / 2.0,
                        radius_y: (hi.y - lo.y) / 2.0,
                        wall: num(wall, 1.0, BIG, "tube width")?,
                        height: hi.z - lo.z,
                        sides,
                        tube_sides,
                        arc: num(arc, 1.0, 360.0, "arc")?,
                        start: num(start, -360.0, 360.0, "start angle")?,
                        add_height: num(add_height, -4096.0, 4096.0, "add height")?,
                    };
                    shapes::make_torus(&torus, &tex)?
                }
                _ => return Err(NOT_UNDERSTOOD.into()),
            };
            let label = match shape.as_str() {
                "wedge" => "Add wedge",
                "spike" => "Add spike",
                "sphere" => "Add sphere",
                "cone" => "Add cone",
                "torus" => "Add torus",
                _ => "Add arch",
            };
            let mut tx = Tx::new(d, label);
            for b in made {
                created.0.push(tx.add_brush(b).0);
            }
            tx.commit(h);
        }
        Cmd::ExtrudeFace { brush, face, distance, drag } => {
            let id = BrushId(brush);
            let dist = num(distance, -BIG, BIG, "distance")?;
            // a drag pulls from the source as it was when it started
            if !matches!((&s.drag, &drag), (Some(dr), Some(k)) if &dr.key == k) {
                let b = d.brushes.get(&id).cloned().ok_or("That brush no longer exists.")?;
                s.drag = drag.as_ref().map(|k| Drag { key: k.clone(), brushes: [(id, b)].into_iter().collect(), made: None });
            }
            let base = s.drag.as_ref().and_then(|dr| dr.brushes.get(&id).cloned()).or_else(|| d.brushes.get(&id).cloned()).ok_or("That brush no longer exists.")?;
            let made = s.drag.as_ref().and_then(|dr| dr.made).filter(|m| d.brushes.contains_key(m));
            let mut tx = Tx::new(d, "Extrude");
            if let Some(k) = &drag {
                tx = tx.merge_key(k);
            }
            if dist < extrude::MIN_EXTRUDE {
                // back onto the face: no new brush (yet)
                let Some(m) = made else { return Err("Drag outwards from the face to pull out a new brush.".into()) };
                tx.remove_brush(m);
                if let Some(dr) = &mut s.drag {
                    dr.made = None;
                }
            } else {
                let nb = extrude::extrude_face(&base, face, dist)?;
                let nid = match made {
                    Some(m) => {
                        tx.set_brush(m, nb);
                        m
                    }
                    None => tx.add_brush(nb),
                };
                if let Some(dr) = &mut s.drag {
                    dr.made = Some(nid);
                }
                created.0.push(nid.0);
            }
            tx.commit(h);
        }
        Cmd::AddVertex { brush, at, delta, drag } => {
            let id = BrushId(brush);
            let (at, delta) = (p3(at)?, p3(delta)?);
            if !matches!((&s.drag, &drag), (Some(dr), Some(k)) if &dr.key == k) {
                let b = d.brushes.get(&id).cloned().ok_or("That brush no longer exists.")?;
                s.drag = drag.as_ref().map(|k| Drag { key: k.clone(), brushes: [(id, b)].into_iter().collect(), made: None });
            }
            let base = s.drag.as_ref().and_then(|dr| dr.brushes.get(&id).cloned()).or_else(|| d.brushes.get(&id).cloned()).ok_or("That brush no longer exists.")?;
            let nb = vertex::add_vertex(&base, at, delta)?;
            let mut tx = Tx::new(d, "Split");
            if let Some(k) = &drag {
                tx = tx.merge_key(k);
            }
            tx.set_brush(id, nb);
            tx.commit(h);
        }
        Cmd::Shear { brushes, entities, axis, along, factor, pivot, lock, keep_scale } => {
            let (bs, es) = (bids(&brushes), eids(&entities));
            if bs.is_empty() && es.is_empty() {
                return Err("Pick something to shear first.".into());
            }
            if !bs.is_empty() {
                need_brushes(d, &bs)?;
            }
            let t = shear::shear_affine(axis, along, factor, p3(pivot)?)?;
            history::cmd_transform_with(d, h, &bs, &es, &t, lock, keep_scale, "Shear", None)?;
        }
        Cmd::MergeVertices { brush, picks } => {
            let id = BrushId(brush);
            if picks.len() > 4096 {
                return Err("That's too many corners at once.".into());
            }
            let pts = picks.into_iter().map(p3).collect::<Result<Vec<_>, _>>()?;
            let b = d.brushes.get(&id).cloned().ok_or("That brush no longer exists.")?;
            let nb = vertex::merge_vertices(&b, &pts)?;
            let mut tx = Tx::new(d, "Merge corners");
            tx.set_brush(id, nb);
            tx.commit(h);
        }
        Cmd::MoveVertices { picks, delta, drag } => {
            if picks.is_empty() {
                return Err("Pick some corners first.".into());
            }
            if picks.len() > 4096 {
                return Err("That's too many corners at once.".into());
            }
            let delta = p3(delta)?;
            let mut by_brush: BTreeMap<BrushId, Vec<Vec3>> = BTreeMap::new();
            for (b, p) in &picks {
                by_brush.entry(BrushId(*b)).or_default().push(p3(*p)?);
            }
            // a drag moves from where it started, so corners are found on the brush as it was
            let same_drag = matches!((&s.drag, &drag), (Some(dr), Some(k)) if &dr.key == k);
            if !same_drag {
                s.drag = drag.as_ref().map(|k| Drag { key: k.clone(), brushes: BTreeMap::new(), made: None });
            }
            let mut base: BTreeMap<BrushId, Brush> = s.drag.as_ref().map(|dr| dr.brushes.clone()).unwrap_or_default();
            for id in by_brush.keys() {
                if !base.contains_key(id) {
                    base.insert(*id, d.brushes.get(id).cloned().ok_or("That brush no longer exists.")?);
                }
            }
            if let Some(dr) = &mut s.drag {
                dr.brushes = base.clone();
            }
            let mut tx = Tx::new(d, "Move corners");
            if let Some(k) = &drag {
                tx = tx.merge_key(k);
            }
            for (id, pts) in &by_brush {
                tx.set_brush(*id, vertex::move_vertices(&base[id], pts, delta)?);
            }
            tx.commit(h);
        }
        Cmd::Array { brushes, entities, copies, offset, rotate, scale, pivot, lock } => {
            let (bs, es) = (bids(&brushes), eids(&entities));
            if !bs.is_empty() {
                need_brushes(d, &bs)?;
            }
            let pivot = match pivot {
                Some(p) => p3(p)?,
                None => centre_of(d, &bs, &es),
            };
            let off = p3(offset)?;
            let r = p3(rotate)?;
            for x in [r.x, r.y, r.z] {
                num(x, -360.0, 360.0, "angle")?;
            }
            let mut m = Mat3::IDENTITY;
            if let Some(sc) = scale {
                let f = p3(sc)?;
                if [f.x, f.y, f.z].iter().any(|x| *x < 0.01 || *x > 100.0) {
                    return Err("That stretch is out of range.".into());
                }
                m = Mat3::scale(f);
            }
            for (axis, deg) in [(Vec3::X, r.x), (Vec3::Y, r.y), (Vec3::Z, r.z)] {
                if deg != 0.0 {
                    m = Mat3::rotation(axis, deg).mul(&m);
                }
            }
            let step = Affine::around(m, pivot).then(&Affine::translate(off));
            let out = history::cmd_array(d, h, &bs, &es, copies, &step, lock)?;
            created = (out.brushes.iter().map(|i| i.0).collect(), out.entities.iter().map(|i| i.0).collect());
        }
        Cmd::Paste { text, offset } => {
            if text.len() > 16 * 1024 * 1024 {
                return Err("That's too much to paste at once.".into());
            }
            let frag = mapfile::read_map(&text).map_err(|_| "The clipboard doesn't hold map pieces. Copy some brushes or entities first.".to_string())?;
            let out = history::cmd_paste(d, h, &frag, p3(offset)?)?;
            created = (out.brushes.iter().map(|i| i.0).collect(), out.entities.iter().map(|i| i.0).collect());
        }
        Cmd::Fix { fixes, with } => {
            if fixes.is_empty() || fixes.len() > 10_000 {
                return Err("Pick a problem to fix.".into());
            }
            let with = match with {
                Some(t) => Some(texture_name(&t)?),
                None => None,
            };
            let list: Vec<FixIt> = fixes.into_iter().map(|f| FixIt { fix: f.fix, brush: f.brush.map(BrushId), entity: f.entity.map(EntityId), texture: f.texture, face: f.face, data: f.data }).collect();
            let n = history::cmd_fix(d, h, &list, with.as_deref())?;
            note = Some(if n == 1 { "Fixed.".into() } else { format!("Fixed {n} problems.") });
        }
        Cmd::Delete { brushes, entities } => {
            if brushes.is_empty() && entities.is_empty() {
                return Err("Pick something to delete first.".into());
            }
            history::cmd_delete(d, h, &bids(&brushes), &eids(&entities));
        }
        Cmd::Duplicate { brushes, entities, offset } => {
            let off = p3(offset)?;
            let mut tx = Tx::new(d, "Duplicate");
            let mut new_entity: HashMap<EntityId, EntityId> = HashMap::new();
            for id in eids(&entities) {
                let e = tx.doc().entities.get(&id).cloned().ok_or("That entity no longer exists.")?;
                let mut e2 = e.clone();
                if let Some(o) = e.origin() {
                    let n = o + off;
                    e2.set("origin", &format!("{} {} {}", mapfile::fmt_num(n.x), mapfile::fmt_num(n.y), mapfile::fmt_num(n.z)));
                }
                let nid = tx.add_entity(e2);
                new_entity.insert(id, nid);
                created.1.push(nid.0);
            }
            for id in bids(&brushes) {
                let b = tx.doc().brushes.get(&id).cloned().ok_or("That brush no longer exists.")?;
                let mut nb = ops::translate(&b, off, true)?;
                nb.entity = b.entity.map(|e| new_entity.get(&e).copied().unwrap_or(e));
                created.0.push(tx.add_brush(nb).0);
            }
            if created.0.is_empty() && created.1.is_empty() {
                return Err("Pick something to duplicate first.".into());
            }
            tx.commit(h);
        }
        Cmd::Transform { brushes, entities, translate, rotate, scale, pivot, lock, keep_scale, merge } => {
            let (bs, es) = (bids(&brushes), eids(&entities));
            if bs.is_empty() && es.is_empty() {
                return Err("Pick something to move first.".into());
            }
            if !bs.is_empty() {
                need_brushes(d, &bs)?;
            }
            let pivot = match pivot {
                Some(p) => p3(p)?,
                None => centre_of(d, &bs, &es),
            };
            let (t, label) = if let Some(tr) = translate {
                (Affine::translate(p3(tr)?), "Move")
            } else if let Some(r) = rotate {
                let axis = p3(r.axis)?;
                if axis.len() < 1e-6 {
                    return Err(NOT_UNDERSTOOD.into());
                }
                (Affine::around(Mat3::rotation(axis, num(r.degrees, -3600.0, 3600.0, "angle")?), pivot), "Rotate")
            } else if let Some(sc) = scale {
                let f = p3(sc)?;
                if [f.x, f.y, f.z].iter().any(|x| x.abs() < 1e-3 || x.abs() > 1000.0) {
                    return Err("That scale would squash it flat.".into());
                }
                (Affine::around(Mat3::scale(f), pivot), if f.x < 0.0 || f.y < 0.0 || f.z < 0.0 { "Mirror" } else { "Scale" })
            } else {
                return Err(NOT_UNDERSTOOD.into());
            };
            history::cmd_transform_with(d, h, &bs, &es, &t, lock, keep_scale, label, merge.as_deref().filter(|k| !k.is_empty() && k.len() <= 64))?;
        }
        Cmd::MoveFace { brush, face, distance, lock, drag } => {
            let id = BrushId(brush);
            let dist = num(distance, -BIG, BIG, "distance")?;
            let base = match (&s.drag, &drag) {
                (Some(dr), Some(_)) => dr.brushes.get(&id).cloned(),
                _ => None,
            };
            let base = match base {
                Some(b) => b,
                None => {
                    let b = d.brushes.get(&id).cloned().ok_or("That brush no longer exists.")?;
                    if let Some(k) = &drag {
                        s.drag = Some(Drag { key: k.clone(), brushes: [(id, b.clone())].into_iter().collect(), made: None });
                    }
                    b
                }
            };
            let nb = ops::move_face(&base, face, dist, lock)?;
            let mut tx = Tx::new(d, "Move face");
            if let Some(k) = &drag {
                tx = tx.merge_key(k);
            }
            tx.set_brush(id, nb);
            tx.commit(h);
        }
        Cmd::Clip { brushes, points, keep, texture } => {
            let bs = bids(&brushes);
            need_brushes(d, &bs)?;
            let pts = [p3(points[0])?, p3(points[1])?, p3(points[2])?];
            let plane = Plane::from_map_points(&pts).ok_or("Those clip points are in a line: pick points further apart.")?;
            let tex = match texture {
                Some(t) => texture_name(&t)?,
                None => d.brushes.get(&bs[0]).and_then(|b| b.faces.first()).map(|f| f.texture.clone()).unwrap_or_else(|| "CLIP".into()),
            };
            let keep = match keep {
                KeepSide::Back => Keep::Back,
                KeepSide::Front => Keep::Front,
                KeepSide::Both => Keep::Both,
            };
            let ids = history::cmd_clip(d, h, &bs, &plane, keep, &tex)?;
            created.0 = ids.iter().filter(|i| !bs.contains(i)).map(|i| i.0).collect();
        }
        Cmd::Hollow { brushes, thickness } => {
            let bs = bids(&brushes);
            need_brushes(d, &bs)?;
            let th = num(thickness, 0.5, 1024.0, "wall thickness")?;
            // all the picked brushes in one undo step
            let mut tx = Tx::new(d, "Hollow");
            for id in &bs {
                let b = tx.doc().brushes.get(id).cloned().ok_or("That brush no longer exists.")?;
                let walls = ops::hollow(&b, th)?;
                tx.remove_brush(*id);
                for w in walls {
                    created.0.push(tx.add_brush(Brush { entity: b.entity, group: b.group, ..w }).0);
                }
            }
            tx.commit(h);
        }
        Cmd::Carve { cutter, targets } => {
            let ts = bids(&targets);
            need_brushes(d, &[BrushId(cutter)])?;
            let n = history::cmd_carve(d, h, BrushId(cutter), &ts)?;
            note = Some(if n == 0 { "Nothing to carve: the brush doesn't overlap any others.".into() } else { format!("Carved {n} brush{}.", if n == 1 { "" } else { "es" }) });
        }
        Cmd::Merge { brushes } => {
            let bs = bids(&brushes);
            need_brushes(d, &bs)?;
            if bs.len() < 2 {
                return Err("Pick at least two brushes to merge.".into());
            }
            let (_, exact) = history::cmd_merge(d, h, &bs)?;
            if !exact {
                note = Some("These didn't form one convex shape: the result fills the gaps.".into());
            }
        }
        Cmd::Snap { brushes, grid } => {
            let bs = bids(&brushes);
            need_brushes(d, &bs)?;
            history::cmd_snap(d, h, &bs, num(grid, 0.125, 1024.0, "grid")?)?;
        }
        Cmd::SetTexture { faces, texture } => {
            if faces.is_empty() {
                return Err("Pick a face first.".into());
            }
            let f: Vec<(BrushId, Option<usize>)> = faces.iter().map(|(b, f)| (BrushId(*b), *f)).collect();
            history::cmd_set_texture(d, h, &f, &texture_name(&texture)?)?;
        }
        Cmd::Align { faces, action, u, v, degrees, drag, ou, ov, su, sv, rot, mode, one, right, up } => {
            if faces.is_empty() {
                return Err("Pick a face first.".into());
            }
            if faces.len() > 20_000 {
                return Err("That's too many faces at once.".into());
            }
            let f: Vec<(BrushId, usize)> = faces.iter().map(|(b, i)| (BrushId(*b), *i)).collect();
            let (u, v) = (num(u, -65536.0, 65536.0, "value")?, num(v, -65536.0, 65536.0, "value")?);
            let deg = num(degrees, -3600.0, 3600.0, "angle")?;
            let opt = |x: Option<f64>, lo: f64, hi: f64| -> Result<Option<f64>, String> { x.map(|x| num(x, lo, hi, "value")).transpose() };
            let (ou, ov, rot) = (opt(ou, -65536.0, 65536.0)?, opt(ov, -65536.0, 65536.0)?, opt(rot, -3600.0, 3600.0)?);
            let (su, sv) = (opt(su, -1024.0, 1024.0)?, opt(sv, -1024.0, 1024.0)?);
            if su.is_some_and(|x| x.abs() < 0.01) || sv.is_some_and(|x| x.abs() < 0.01) {
                return Err("A texture scale can't be 0.".into());
            }
            // treated as one: every face works from all the faces' corners
            let all: Vec<Vec3> = if one {
                let mut pts = Vec::new();
                for (id, i) in &f {
                    let b = d.brushes.get(id).ok_or("That brush no longer exists.")?;
                    pts.extend(crate::editor::geom::brush_polygons(b).get(*i).cloned().unwrap_or_default());
                }
                pts
            } else {
                Vec::new()
            };
            let pts = |poly: &[Vec3]| -> Vec<Vec3> { if one { all.clone() } else { poly.to_vec() } };
            let anchor = |poly: &[Vec3]| crate::editor::geom::polygon_center(&pts(poly));
            let drag = drag.as_deref();
            match action.as_str() {
                "shift" => history::cmd_align(d, h, &f, drag, |a, _, _, _| texture::shift(a, u, v))?,
                "scale" => {
                    if u.abs() < 0.01 || v.abs() < 0.01 {
                        return Err("A texture scale can't be 0.".into());
                    }
                    history::cmd_align(d, h, &f, drag, |a, p, _, _| texture::set_scale(a, u, v, anchor(p)))?
                }
                "rotate" => history::cmd_align(d, h, &f, drag, |a, p, _, n| texture::rotate(a, n, deg, anchor(p)))?,
                "fit" => {
                    let (ru, rv) = (if u > 0.0 { u } else { 1.0 }, if v > 0.0 { v } else { 1.0 });
                    history::cmd_align(d, h, &f, drag, |a, p, (w, ht), _| texture::fit(a, &pts(p), w, ht, ru, rv))?
                }
                "center" => history::cmd_align(d, h, &f, drag, |a, p, (w, ht), _| texture::center(a, &pts(p), w, ht))?,
                "justify" => {
                    let how = mode.as_deref().and_then(texture::Justify::parse).ok_or(NOT_UNDERSTOOD)?;
                    history::cmd_align(d, h, &f, drag, |a, p, (w, ht), _| texture::justify(a, &pts(p), w, ht, how))?
                }
                "set" => history::cmd_align(d, h, &f, drag, |a, _, _, n| texture::set_values(a, n, ou, ov, su, sv, rot))?,
                "world" => history::cmd_align(d, h, &f, drag, |a, _, _, n| texture::align_to_world(a, n))?,
                "face" => history::cmd_align(d, h, &f, drag, |a, _, _, n| texture::align_to_face(a, n))?,
                "view" => {
                    let (r, u) = (p3(right.ok_or(NOT_UNDERSTOOD)?)?, p3(up.ok_or(NOT_UNDERSTOOD)?)?);
                    if r.len() < 1e-6 || u.len() < 1e-6 {
                        return Err(NOT_UNDERSTOOD.into());
                    }
                    history::cmd_align(d, h, &f, drag, |a, _, _, n| texture::align_to_view(a, n, r, u))?
                }
                _ => return Err(NOT_UNDERSTOOD.into()),
            }
        }
        Cmd::WrapTexture { from, faces, texture } => {
            if faces.is_empty() {
                return Err("Pick a face first.".into());
            }
            if faces.len() > 20_000 {
                return Err("That's too many faces at once.".into());
            }
            let tex = texture.as_deref().map(texture_name).transpose()?;
            let f: Vec<(BrushId, usize)> = faces.iter().map(|(b, i)| (BrushId(*b), *i)).collect();
            history::cmd_wrap(d, h, (BrushId(from.0), from.1), &f, tex.as_deref())?;
        }
        Cmd::SetKey { entity, key, value } => {
            if key.len() > 64 || value.len() > 2048 {
                return Err("That key or value is too long.".into());
            }
            history::cmd_set_key(d, h, EntityId(entity), key.trim(), &value)?;
        }
        Cmd::AddEntity { classname, origin, keys } => {
            safe_key("classname", &classname)?;
            if !classname.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
                return Err("That isn't an entity class name.".into());
            }
            let o = p3(origin)?;
            let mut e = Entity::new(&classname);
            e.set("origin", &format!("{} {} {}", mapfile::fmt_num(o.x), mapfile::fmt_num(o.y), mapfile::fmt_num(o.z)));
            for (k, v) in keys.iter().take(64) {
                safe_key(k, v)?;
                if k != "classname" && k != "origin" {
                    e.set(k, v);
                }
            }
            created.1.push(history::cmd_add_entity(d, h, e).0);
        }
        Cmd::Tie { brushes, classname } => {
            let bs = bids(&brushes);
            need_brushes(d, &bs)?;
            if classname.is_empty() || classname.len() > 64 || !classname.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
                return Err("That isn't an entity class name.".into());
            }
            created.1.push(history::cmd_tie_to_entity(d, h, &bs, &classname)?.0);
        }
        Cmd::ToWorld { entity } => {
            if !d.entities.contains_key(&EntityId(entity)) {
                return Err("That entity no longer exists.".into());
            }
            history::cmd_move_to_world(d, h, EntityId(entity));
        }
        Cmd::SetWorld { wads, keys } => {
            let mut w = d.world.clone();
            if let Some(list) = wads {
                if list.len() > 64 || list.iter().any(|x| x.is_empty() || x.len() > 260 || x.contains(['"', ';', '\n'])) {
                    return Err("That WAD list isn't valid.".into());
                }
                w.wads = list;
            }
            if let Some(keys) = keys {
                for (k, v) in &keys {
                    safe_key(k, v)?;
                }
                w.keys = keys.into_iter().filter(|(k, _)| !matches!(k.as_str(), "classname" | "mapversion" | "wad")).take(256).collect();
            }
            history::cmd_set_world(d, h, w);
        }
        Cmd::Group { name, brushes, entities } => {
            let name = if name.trim().is_empty() { "Group".to_string() } else { name.trim().chars().take(64).collect() };
            if brushes.is_empty() && entities.is_empty() {
                return Err("Pick something to group first.".into());
            }
            let g = history::cmd_group(d, h, &name, &bids(&brushes), &eids(&entities))?;
            note = Some(format!("{}", g.0));
        }
        Cmd::Ungroup { group } => {
            if !d.groups.contains_key(&GroupId(group)) {
                return Err("That group no longer exists.".into());
            }
            history::cmd_ungroup(d, h, GroupId(group));
        }
        Cmd::GroupFlags { group, hidden, locked } => {
            let mut g = d.groups.get(&GroupId(group)).cloned().ok_or("That group no longer exists.")?;
            if let Some(x) = hidden {
                g.hidden = x;
            }
            if let Some(x) = locked {
                g.locked = x;
            }
            let mut tx = Tx::new(d, if hidden.is_some() { "Hide group" } else { "Lock group" });
            tx.set_group(GroupId(group), g);
            tx.commit(h);
        }
        Cmd::PlacePrefab { point, normal, grid, yaw, face_out, loaded, .. } => {
            let (frag, meta) = *loaded.ok_or("That prefab isn't there any more.")?;
            let n = v3(normal[0], normal[1], normal[2]);
            let grid = num(grid, 0.0, 1024.0, "grid")?;
            let yaw = num(yaw, -3600.0, 3600.0, "turn")?;
            let (t, _) = crate::editor::prefab::place_transform(&crate::editor::prefab::bounds(&frag), p3(point)?, n, grid, yaw, face_out);
            p3([t.t.x, t.t.y, t.t.z])?;
            // P5: the group remembers the prefab's uid
            let (out, _) = history::cmd_place_prefab_from(d, h, &frag, &t, &meta.name, &meta.wads, Some(&meta.uid))?;
            created = (out.brushes.iter().map(|i| i.0).collect(), out.entities.iter().map(|i| i.0).collect());
            note = Some(format!("Placed {}.", meta.name));
        }
    }
    Ok(json!({"created": {"brushes": created.0, "entities": created.1}, "note": note}))
}

/// A command against the open document, with what the page needs to catch up.
fn apply(s: &mut Session, cmd: Cmd, dirs: &[PathBuf]) -> Value {
    // P5 (perf): no copies of the entities, groups and world per command: the change notes
    // say what changed (groups and world are small and compared as before)
    let before = (s.doc.revision, s.doc.groups.len(), s.doc.world.clone());
    match run_cmd(s, cmd) {
        Ok(mut v) => {
            s.learn_textures(dirs);
            let ents = !s.doc.touched.entities.is_empty();
            s.track();
            let doc_changed = ents || s.doc.groups.len() != before.1 || s.doc.world != before.2;
            v["changed"] = json!(s.doc.revision != before.0);
            v["docChanged"] = json!(doc_changed);
            v["state"] = s.state();
            // P5 (perf): what changed, so the page patches its copy instead of asking for all of it
            if s.doc.revision != before.0 {
                v["changes"] = changes_json(s, before.0).unwrap_or(Value::Null);
            }
            v
        }
        Err(e) => {
            // a failed command leaves nothing behind; the page still gets where things are
            s.track();
            json!({"error": e, "state": s.state()})
        }
    }
}

// ---- files ----------------------------------------------------------------------------

fn read_text(path: &Path) -> Result<String, String> {
    let len = std::fs::metadata(path).map_err(|_| "That file couldn't be opened.".to_string())?.len();
    if len > MAX_FILE {
        return Err("That file is too big to open.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "That file couldn't be read.".to_string())?;
    // old .map files are in a Windows code page (Western, or Cyrillic in Russian maps; ours
    // are written that way too): UTF-8 when it is, else the code page it most likely was in
    Ok(crate::editor::rmf::decode_text(&bytes))
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "Untitled".into())
}

/// A project file (`.lsmap`, the editor's own JSON) or a `.map`, by its extension.
pub fn load_file(path: &Path) -> Result<(MapDocument, bool), String> {
    let text = read_text(path)?;
    let is_map = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("map"));
    let mut doc = if is_map { mapfile::read_map(&text)? } else { MapDocument::from_json(&text)? };
    // P5: an old project (or one with a broken uid) gets its permanent uid now, written into
    // the file so it keeps it (the rest of the file is as it was read)
    if !is_map && !doc.editor.uid.as_deref().is_some_and(crate::editor::doc::valid_uid) {
        doc.editor.uid = Some(crate::editor::doc::new_uid());
        let _ = write_atomic(path, doc.to_json());
    }
    Ok((doc, !is_map))
}

/// Writes next to the target and renames over it, so a failed write never leaves half a file.
fn write_atomic(path: &Path, text: impl AsRef<[u8]>) -> Result<(), String> {
    let tmp = path.with_extension("lsmap-tmp");
    // backups (P7, safety.rs): only files inside a game folder; a project elsewhere keeps its own copies
    crate::safety::before("Map editor", &format!("Saved {}", path.file_name().unwrap_or_default().to_string_lossy()), path);
    std::fs::write(&tmp, text).map_err(|_| "Couldn't write the file there. Pick another folder.".to_string())?;
    std::fs::rename(&tmp, path).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        "Couldn't save over that file. Pick another name.".to_string()
    })
}

fn save_project(s: &mut Session, path: &Path, backups: Option<&Path>) -> Result<(), String> {
    if let (Some(dir), true) = (backups, path.exists()) {
        backup(dir, path, s.doc.editor.uid.as_deref());
    }
    write_atomic(path, &s.doc.to_json())?;
    s.project = Some(path.to_path_buf());
    // saved somewhere new: WADs beside it are found from now on
    if s.folder.as_deref() != path.parent() {
        s.folder = path.parent().map(Path::to_path_buf);
        s.wads = None;
    }
    s.name = stem(path);
    s.history.mark_saved();
    Ok(())
}

/// Writes the document as a .map in the game's code page (Western, or Cyrillic when its text
/// needs it); the size, and a note when some characters couldn't be kept.
fn export_map(s: &Session, path: &Path) -> Result<(usize, Option<String>), String> {
    // groups and TrenchBroom layers kept, "omit from export" layers left out (EditorCoverage TB-2, TB-3)
    let text = mapfile::write_map(&s.doc, &mapfile::WriteOptions::export())?;
    let (bytes, cp, lost) = mapfile::encode_for_game(&text);
    write_atomic(path, &bytes)?;
    Ok((bytes.len(), code_page_note(cp, &lost)))
}

/// What to say when entity text has characters the game can't show (written as ?).
pub fn code_page_note(cp: mapfile::CodePage, lost: &[char]) -> Option<String> {
    (!lost.is_empty()).then(|| {
        let shown: String = lost.iter().take(12).collect();
        format!("Some text in the map has characters the game can't show ({shown}); they were written as ? ({}).", cp.name())
    })
}

enum Pick {
    Open,
    Import,
    SaveProject,
    ExportMap,
    ExportPrefab,
    ImportPrefab,
    Fgd,
    /// P5: a leak line from another compile
    Pointfile,
}

/// Parity A: a folder from the system dialog (Export…); None when cancelled. Test builds answer
/// with LS_DEV_PICK_FOLDER.
pub fn pick_folder(st: &Shared, title: &str) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt;
    #[cfg(test)]
    {
        if let Ok(p) = std::env::var("LS_DEV_PICK_FOLDER") {
            return Ok(Some(PathBuf::from(p)));
        }
    }
    let app = st.app.get().cloned().ok_or(INTERNAL)?;
    match app.dialog().file().set_title(title).blocking_pick_folder() {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|_| "That folder can't be used.".to_string()),
    }
}

/// The system file dialog; None when cancelled.
fn dialog(st: &Shared, kind: Pick, name: &str) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt;
    // P5: dev servers (test builds only) answer the dialog with LS_DEV_PICK, to open big maps in a browser
    #[cfg(test)]
    {
        // (save dialogs can answer elsewhere, so a test can import a file and save a project)
        let save = matches!(kind, Pick::SaveProject | Pick::ExportMap);
        if let Some(p) = save.then(|| std::env::var("LS_DEV_PICK_SAVE").ok()).flatten().or_else(|| std::env::var("LS_DEV_PICK").ok()) {
            return Ok(Some(PathBuf::from(p)));
        }
    }
    let app = st.app.get().cloned().ok_or(INTERNAL)?;
    let d = app.dialog().file();
    let picked = match kind {
        Pick::Open => d.set_title("Open a map project").add_filter("Loner-Strike map project", &[PROJECT_EXT]).blocking_pick_file(),
        Pick::Import => d.set_title("Import a .map file (TrenchBroom, Hammer, J.A.C.K.)").add_filter("Map source (.map: TrenchBroom, Hammer, J.A.C.K.)", &["map"]).blocking_pick_file(),
        Pick::SaveProject => d.set_title("Save the map project").set_file_name(format!("{name}.{PROJECT_EXT}")).add_filter("Loner-Strike map project", &[PROJECT_EXT]).blocking_save_file(),
        Pick::ExportMap => d.set_title("Export a .map file").set_file_name(format!("{name}.map")).add_filter("Map source (.map: TrenchBroom, Hammer, J.A.C.K.)", &["map"]).blocking_save_file(),
        Pick::ExportPrefab => d.set_title("Save the prefab package").set_file_name(format!("{name}.zip")).add_filter("Prefab package", &["zip"]).blocking_save_file(),
        Pick::ImportPrefab => d.set_title("Add a prefab").add_filter("Prefab package or .map", &["zip", "rar", "7z", "map"]).blocking_pick_file(),
        Pick::Fgd => d.set_title("Choose an entity definition file").add_filter("Entity definitions (FGD)", &["fgd"]).blocking_pick_file(),
        Pick::Pointfile => d.set_title("Load a leak line").add_filter("Leak line (pointfile)", &["lin", "pts"]).blocking_pick_file(),
    };
    match picked {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|_| "That file can't be used.".to_string()),
    }
}


// ---- autosave, backups, recent files -----------------------------------------------------

const AUTOSAVE: &str = "autosave.lsmap";
const AUTOSAVE_META: &str = "autosave.json";
const RECENT: &str = "recent.json";
const MAX_RECENT: usize = 10;
const KEEP_BACKUPS: usize = 10;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Copies a project about to be saved over into the backups folder (the last few per
/// project). P5: named `<name>~<uid>~<time>.lsmap`, so a renamed or moved project keeps its
/// backups (old ones, `<name>-<time>.lsmap`, are still kept by name).
fn backup(dir: &Path, path: &Path, uid: Option<&str>) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let name = stem(path);
    let file = match uid {
        Some(u) => format!("{name}~{u}~{}.{PROJECT_EXT}", now_secs()),
        None => format!("{name}-{}.{PROJECT_EXT}", now_secs()),
    };
    let _ = std::fs::copy(path, dir.join(file));
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut mine: Vec<(u64, PathBuf)> = rd.flatten().map(|e| e.path()).filter_map(|p| backup_of(&p, &name, uid).map(|t| (t, p))).collect();
    mine.sort();
    let mut mine: Vec<PathBuf> = mine.into_iter().map(|x| x.1).collect();
    while mine.len() > KEEP_BACKUPS {
        let _ = std::fs::remove_file(mine.remove(0));
    }
}

/// P5: when `p` is a backup of the project (by its uid, or an old one by name): its time.
pub(crate) fn backup_of(p: &Path, name: &str, uid: Option<&str>) -> Option<u64> {
    let n = p.file_name()?.to_str()?.strip_suffix(&format!(".{PROJECT_EXT}"))?;
    if let Some(u) = uid {
        let parts: Vec<&str> = n.rsplitn(3, '~').collect();
        if parts.len() == 3 && parts[1] == u {
            return parts[0].parse().ok();
        }
    }
    let t = n.strip_prefix(&format!("{name}-"))?;
    t.bytes().all(|c| c.is_ascii_digit()).then(|| t.parse().ok()).flatten()
}

/// P5: a project file's uid, without reading the whole map into a document (the editor
/// settings' `"uid"`, the last one in the file).
pub(crate) fn file_uid(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    // the editor settings come last: the end of the file is enough
    let from = len.saturating_sub(64 * 1024);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = Vec::new();
    f.take(64 * 1024).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let i = text.rfind("\"uid\"")?;
    let rest = text[i + 5..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let u = rest.get(..36)?;
    crate::editor::doc::valid_uid(u).then(|| u.to_string())
}

/// P5: where a project with this uid is now: the recent list, then the folder it was in (and
/// that folder's neighbours, one level), for a project renamed or moved outside the app.
pub(crate) fn find_project(dir: &Path, uid: &str, was: Option<&Path>) -> Option<PathBuf> {
    let is = |p: &Path| p.is_file() && p.extension().is_some_and(|e| e == PROJECT_EXT) && file_uid(p).as_deref() == Some(uid);
    if let Some(r) = recent_list(dir).into_iter().find(|r| r.uid.as_deref() == Some(uid) && is(&r.path)) {
        return Some(r.path);
    }
    let folder = was?.parent()?;
    let mut dirs = vec![folder.to_path_buf()];
    if let Some(up) = folder.parent() {
        dirs.push(up.to_path_buf());
        if let Ok(rd) = std::fs::read_dir(up) {
            dirs.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p != folder).take(50));
        }
    }
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        if let Some(p) = rd.flatten().map(|e| e.path()).take(300).find(|p| is(p)) {
            return Some(p);
        }
    }
    None
}

#[derive(serde::Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Recent {
    /// P5: the project's uid (follows a renamed or moved project)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    uid: Option<String>,
    path: PathBuf,
    project: bool,
}

fn recent_list(dir: &Path) -> Vec<Recent> {
    std::fs::read_to_string(dir.join(RECENT)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Puts a file at the top of the recent list.
fn remember(dir: &Path, path: &Path, project: bool) {
    let mut list = recent_list(dir);
    // P5: one entry per project: a moved or renamed one replaces its old path
    let uid = if project { file_uid(path) } else { None };
    list.retain(|r| r.path != path && (uid.is_none() || r.uid != uid));
    list.insert(0, Recent { path: path.to_path_buf(), project, uid });
    list.truncate(MAX_RECENT);
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(t) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(dir.join(RECENT), t);
        }
    }
}

fn recent_json(dir: &Path) -> Value {
    let files: Vec<Value> = recent_list(dir)
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let folder = r.path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            // P5: a project that moved still counts as found (opening it finds it by its uid)
            let found = r.path.is_file() || r.uid.as_deref().is_some_and(|u| find_project(dir, u, Some(&r.path)).is_some());
            json!({"index": i, "name": r.path.file_name().map(|n| n.to_string_lossy().to_string()), "folder": folder, "project": r.project, "found": found})
        })
        .collect();
    json!({"files": files})
}

/// P5 (perf): an autosave copies the document under a brief lock and writes it without the
/// lock (on br_valley, writing took 80-150 ms that every edit waited for).
struct AutosaveSnap {
    doc: MapDocument,
    meta: Value,
}

impl AutosaveSnap {
    fn take(s: &Session) -> Option<AutosaveSnap> {
        if !s.history.is_dirty() {
            return None;
        }
        let meta = json!({"name": s.name, "project": s.project, "uid": s.doc.editor.uid, "time": now_secs(), "brushes": s.doc.brushes.len(), "entities": s.doc.entities.len()});
        Some(AutosaveSnap { doc: s.doc.clone(), meta })
    }

    fn write(self, dir: &Path) -> Result<(), String> {
        // one autosave at a time (two in a row mustn't interleave their files)
        static WRITING: Mutex<()> = Mutex::new(());
        let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
        std::fs::create_dir_all(dir).map_err(|_| "Couldn't make the autosave folder.".to_string())?;
        write_atomic(&dir.join(AUTOSAVE), &self.doc.to_json())?;
        std::fs::write(dir.join(AUTOSAVE_META), self.meta.to_string()).map_err(|_| "Couldn't write the autosave.".to_string())?;
        Ok(())
    }
}

fn recovery_json(dir: &Path) -> Value {
    if !dir.join(AUTOSAVE).is_file() {
        return json!({"found": false});
    }
    let meta: Value = std::fs::read_to_string(dir.join(AUTOSAVE_META)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    json!({"found": true, "name": meta["name"], "time": meta["time"], "brushes": meta["brushes"], "entities": meta["entities"], "project": meta["project"].is_string()})
}

// ---- routes ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Since {
    doc: Option<u64>,
    since: Option<u64>,
}

/// The whole document, or (P5, perf) with `?doc=<id>&since=<revision>` only what changed since
/// (`changes_json`, marked `"changes": true`; the whole document when that's too far back).
async fn api_doc(Extension(ed): Extension<Ed>, q: Option<Query<Since>>) -> Response {
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        if let Some(Query(Since { doc: Some(d), since: Some(r) })) = q {
            if d == s.id {
                if let Some(mut v) = changes_json(s, r) {
                    v["changes"] = json!(true);
                    return Ok(v);
                }
            }
        }
        Ok(doc_json(s))
    })
    .await
}

async fn api_meshes(Extension(ed): Extension<Ed>, Query(q): Query<Since>) -> Response {
    let r = tokio::task::spawn_blocking(move || {
        // P5 (perf): copy what's needed under the lock, build the meshes without it
        let snap = ed.lock().as_ref().map(|s| MeshSnap::take(s, q.doc, q.since));
        snap.map(MeshSnap::build)
    })
    .await;
    match r {
        Ok(Some(b)) => ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")], b).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": NO_DOC}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": INTERNAL}))).into_response(),
    }
}

/// Checks: the stretched-texture limits the page lets the mapper change.
#[derive(Deserialize, Default)]
struct ChecksQ {
    #[serde(rename = "minScale")]
    min_scale: Option<f64>,
    #[serde(rename = "maxScale")]
    max_scale: Option<f64>,
}

async fn api_checks(State(st): State<Shared>, Extension(ed): Extension<Ed>, q: Option<Query<ChecksQ>>) -> Response {
    let dirs = wad_dirs(&st);
    let q = q.map(|q| q.0).unwrap_or_default();
    blocking(move || {
        // Checks: a snapshot under a brief lock; the checks run without it
        let snap = {
            let mut g = ed.lock();
            let s = g.as_mut().ok_or(NO_DOC)?;
            CheckSnap::take(s, &st, &dirs)
        };
        let d = checks::Options::default();
        let lim = |x: Option<f64>, def: f64| x.filter(|v| v.is_finite() && *v > 0.0 && *v <= 1024.0).unwrap_or(def);
        let (min_scale, max_scale) = (lim(q.min_scale, d.min_scale), lim(q.max_scale, d.max_scale));
        // the same document and settings as last time: the same answer
        let key = (snap.id, snap.revision, min_scale.to_bits(), max_scale.to_bits());
        let last = &ed.checks_last;
        if let Some((k, v)) = last.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if *k == key {
                return Ok(v.clone());
            }
        }
        let opts = checks::Options { map: snap.name.clone(), min_scale, max_scale, vis: heavy::vis_for(&st, &snap.name) };
        let v = checks_json(&snap, Some(&snap.places), &opts);
        // the document changed while they ran: the answer is stale (the page asks again)
        let now = ed.lock().as_ref().map(|s| (s.id, s.doc.revision));
        if now != Some((snap.id, snap.revision)) {
            return Ok(json!({"stale": true, "docId": snap.id, "revision": snap.revision}));
        }
        *last.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, v.clone()));
        Ok(v)
    })
    .await
}

async fn api_cmd(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let mut cmd: Cmd = match serde_json::from_value(body) {
        Ok(c) => c,
        Err(_) => return bad(NOT_UNDERSTOOD),
    };
    let dirs = wad_dirs(&st);
    blocking(move || {
        if let Cmd::PlacePrefab { prefab, loaded, .. } = &mut cmd {
            *loaded = Some(Box::new(prefabs::load(&prefabs::lib_dir(&ed.dir), prefab)?));
        }
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        Ok(apply(s, cmd, &dirs))
    })
    .await
}

async fn undo_redo(ed: Ed, h: HeaderMap, redo: bool) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        s.drag = None;
        let from = s.doc.revision;
        let label = if redo { s.history.redo(&mut s.doc) } else { s.history.undo(&mut s.doc) };
        s.track();
        let changes = if label.is_some() { changes_json(s, from).unwrap_or(Value::Null) } else { Value::Null };
        Ok(json!({"label": label, "changed": label.is_some(), "docChanged": label.is_some(), "state": s.state(), "changes": changes}))
    })
    .await
}

async fn api_undo(Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    undo_redo(ed, h, false).await
}
async fn api_redo(Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    undo_redo(ed, h, true).await
}

async fn api_new(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let dirs = wad_dirs(&st);
    blocking(move || Ok(ed.install(starter_map(), "Untitled".into(), None, &dirs))).await
}

async fn api_open(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    open_with(st, ed, h, Pick::Open).await
}
async fn api_import(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    open_with(st, ed, h, Pick::Import).await
}

async fn open_with(st: Shared, ed: Ed, h: HeaderMap, kind: Pick) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let dirs = wad_dirs(&st);
    blocking(move || {
        let Some(path) = dialog(&st, kind, "")? else { return Ok(json!({"cancelled": true})) };
        let (doc, project) = load_file(&path)?;
        remember(&ed.dir, &path, project);
        let mut state = ed.install_from(doc, stem(&path), project.then_some(path.clone()), Some(&path), &dirs);
        // P5: an imported .map's missing models, sprites and sounds, said now
        if !project {
            let g = ed.lock();
            if let Some(n) = g.as_ref().and_then(|s| assets::note_for(&st, s)) {
                state["notes"] = json!([n]);
            }
        }
        Ok(state)
    })
    .await
}

#[derive(Deserialize, Default)]
struct SaveReq {
    #[serde(default, rename = "as")]
    save_as: bool,
}

async fn api_save(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, body: Option<Json<SaveReq>>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let req = body.map(|b| b.0).unwrap_or_default();
    blocking(move || {
        let (known, name) = {
            let g = ed.lock();
            let s = g.as_ref().ok_or(NO_DOC)?;
            (s.project.clone(), s.name.clone())
        };
        let path = match known.filter(|_| !req.save_as) {
            Some(p) => p,
            None => match dialog(&st, Pick::SaveProject, &name)? {
                Some(p) => p,
                None => return Ok(json!({"cancelled": true})),
            },
        };
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        // P5: Save as to another file makes a copy: it gets its own uid
        if req.save_as && s.project.as_ref().is_some_and(|p| p != &path) {
            s.doc.editor.uid = Some(crate::editor::doc::new_uid());
        }
        save_project(s, &path, Some(&crate::localdata::join(&ed.dir, "backups")))?;
        let _ = std::fs::remove_file(ed.dir.join(AUTOSAVE));
        let _ = std::fs::remove_file(ed.dir.join(AUTOSAVE_META));
        remember(&ed.dir, &path, true);
        Ok(json!({"saved": true, "file": path.file_name().map(|n| n.to_string_lossy().to_string()), "state": s.state()}))
    })
    .await
}

async fn api_export(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let name = ed.lock().as_ref().ok_or(NO_DOC)?.name.clone();
        let Some(path) = dialog(&st, Pick::ExportMap, &name)? else { return Ok(json!({"cancelled": true})) };
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let (bytes, note) = export_map(s, &path)?;
        Ok(json!({"exported": true, "file": path.file_name().map(|n| n.to_string_lossy().to_string()), "bytes": bytes, "note": note}))
    })
    .await
}

/// The textures in the document's WADs: `{wads[{name, found, error?, textures[{name, width,
/// height}]}]}`.
fn texture_list(set: &wad::WadSet) -> Value {
    let wads: Vec<Value> = set
        .wads
        .iter()
        .map(|(n, w)| match w {
            Err(e) => json!({"name": n, "found": false, "error": if e == "missing" { "Not on this computer." } else { e.as_str() }, "textures": []}),
            Ok(w) => {
                let mut list: Vec<(&String, &wad::Entry)> = w.entries.iter().filter(|(name, _)| w.has_texture(name)).collect();
                list.sort_by(|a, b| a.0.cmp(b.0));
                let tex: Vec<Value> = list.iter().take(8192).filter_map(|(name, e)| miptex_size(&w.path, e.offset).map(|(tw, th)| json!({"name": name, "width": tw, "height": th}))).collect();
                json!({"name": n, "found": true, "textures": tex})
            }
        })
        .collect();
    json!({"wads": wads})
}

async fn api_textures(State(st): State<Shared>, Extension(ed): Extension<Ed>) -> Response {
    let dirs = wad_dirs(&st);
    blocking(move || {
        let set = {
            let mut g = ed.lock();
            let s = g.as_mut().ok_or(NO_DOC)?;
            s.wads(&dirs)
        };
        let Some(set) = set else { return Ok(json!({"wads": [], "noGame": true})) };
        let stamp = wads_stamp(&set);
        if let Some(v) = ed.lists.lock().unwrap_or_else(|e| e.into_inner()).get(&stamp) {
            return Ok((**v).clone());
        }
        let v = texture_list(&set);
        let mut lists = ed.lists.lock().unwrap_or_else(|e| e.into_inner());
        if lists.len() > 8 {
            lists.clear();
        }
        lists.insert(stamp, Arc::new(v.clone()));
        Ok(v)
    })
    .await
}

/// WAD files in the game folders, for the map settings' "Add a WAD".
async fn api_wads(State(st): State<Shared>) -> Response {
    let dirs = wad_dirs(&st);
    blocking(move || {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for d in &dirs {
            let game = d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let Ok(rd) = std::fs::read_dir(d) else { continue };
            let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.to_ascii_lowercase().ends_with(".wad")).collect();
            names.sort_by_key(|n| n.to_ascii_lowercase());
            for n in names {
                let low = n.to_ascii_lowercase();
                if matches!(low.as_str(), "tempdecal.wad" | "pldecal.wad" | "decals.wad" | "fonts.wad" | "gfx.wad" | "cached.wad" | "spraypaint.wad") || !seen.insert(low) {
                    continue;
                }
                out.push(json!({"name": n, "game": game}));
            }
        }
        Ok(json!({"wads": out}))
    })
    .await
}

async fn api_tex(State(st): State<Shared>, Extension(ed): Extension<Ed>, UrlPath(file): UrlPath<String>, h: HeaderMap) -> Response {
    let dirs = wad_dirs(&st);
    let seen = h.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()).map(String::from);
    let name = file.strip_suffix(".png").unwrap_or(&file).to_string();
    if texture_name(&name).is_err() || name.contains(['/', '\\']) {
        return bad("That isn't a texture name.");
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Option<(String, Arc<Vec<u8>>)>, (StatusCode, String)> {
        let set = {
            let mut g = ed.lock();
            let s = g.as_mut().ok_or((StatusCode::NOT_FOUND, NO_DOC.to_string()))?;
            s.wads(&dirs)
        };
        let set = set.ok_or((StatusCode::CONFLICT, "Set up the game on this computer first.".to_string()))?;
        let etag = format!("\"{}-{}\"", wads_stamp(&set), super::cache::short_hash(&[&name.to_ascii_lowercase()]));
        if seen.as_deref() == Some(etag.as_str()) {
            return Ok(None);
        }
        let key = etag.clone();
        if let Some(b) = ed.pngs.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return Ok(Some((etag, b.clone())));
        }
        let rec = set.read(&name).map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?.ok_or((StatusCode::NOT_FOUND, "That texture isn't in the map's WADs.".to_string()))?;
        let t = wad::decode_miptex(&rec, &name).map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
        let png = Arc::new(goldsrc::png(t.width, t.height, &t.pixels, true).map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?);
        let mut cache = ed.pngs.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() > 2048 {
            cache.clear();
        }
        cache.insert(key, png.clone());
        Ok(Some((etag, png)))
    })
    .await;
    match r {
        Ok(Ok(Some((etag, b)))) => ([(header::CONTENT_TYPE, "image/png".to_string()), (header::CACHE_CONTROL, "no-cache".into()), (header::ETAG, etag)], (*b).clone()).into_response(),
        Ok(Ok(None)) => StatusCode::NOT_MODIFIED.into_response(),
        Ok(Err((status, msg))) => (status, Json(json!({"error": msg}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": INTERNAL}))).into_response(),
    }
}


/// The clipboard text for a selection: a small .map (paste it into this map or another, or
/// into J.A.C.K./TrenchBroom).
#[derive(Deserialize)]
struct CopyReq {
    #[serde(default)]
    brushes: Vec<u64>,
    #[serde(default)]
    entities: Vec<u64>,
}

async fn api_copy(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<CopyReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        if req.brushes.is_empty() && req.entities.is_empty() {
            return Err("Pick something to copy first.".into());
        }
        let frag = history::fragment(&s.doc, &bids(&req.brushes), &eids(&req.entities));
        let text = mapfile::write_map(&frag, &mapfile::WriteOptions::default())?;
        Ok(json!({"text": text, "brushes": frag.brushes.len(), "entities": frag.entities.len()}))
    })
    .await
}

/// The entity definitions for the inspector: ours (Counter-Strike plus the mode components),
/// then the map's own FGD file on top when it has one (its classes replace ours by name).
async fn api_fgd(Extension(ed): Extension<Ed>) -> Response {
    static PARSED: OnceLock<Result<Vec<fgd::Class>, String>> = OnceLock::new();
    let ours = match PARSED.get_or_init(|| fgd::parse(&fgd::shipped())) {
        Ok(c) => c.clone(),
        Err(e) => return fail(e.clone()),
    };
    let custom = ed.lock().as_ref().and_then(|s| s.doc.editor.fgd.clone());
    blocking(move || {
        let Some(path) = custom else {
            return Ok(json!({"source": "Loner-Strike entity definitions", "classes": ours}));
        };
        let file = Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let own = read_text(Path::new(&path)).and_then(|t| fgd::parse(&t));
        match own {
            Ok(mine) => {
                let names: HashSet<String> = mine.iter().map(|c| c.name.to_ascii_lowercase()).collect();
                let mut all: Vec<fgd::Class> = ours.into_iter().filter(|c| !names.contains(&c.name.to_ascii_lowercase())).collect();
                all.extend(mine);
                all.sort_by(|a, b| a.name.cmp(&b.name));
                Ok(json!({"source": format!("Loner-Strike entity definitions + {file}"), "custom": {"file": file, "path": path}, "classes": all}))
            }
            Err(e) => Ok(json!({"source": "Loner-Strike entity definitions", "custom": {"file": file, "path": path, "error": e}, "classes": ours})),
        }
    })
    .await
}

fn settings_json(d: &MapDocument) -> Value {
    let e = &d.editor;
    json!({
        "fgd": e.fgd.as_deref().map(|p| Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
        "visSets": e.vis_sets.iter().map(|v| json!({"name": v.name, "brushes": v.brushes.iter().map(|b| b.0).collect::<Vec<_>>(), "entities": v.entities.iter().map(|x| x.0).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "cordon": {"boxes": e.cordon.boxes, "on": e.cordon.on, "seal": if e.cordon.seal == "black" { "black" } else { "sky" }, "keepLights": e.cordon.keep_lights},
        "mapName": e.map_name,
    })
}

/// Where the document is (cheap): the page polls this to pick up changes made from outside
/// (AI agents over MCP, another window).
/// P5 (perf): `?wait=<revision>&doc=<id>`: answers when the document isn't that any more (an
/// agent's edit, another window, an undo), or after 25 s: the page's watch for outside changes
/// waits here instead of asking every 1.5 s.
#[derive(Deserialize)]
struct WaitQ {
    wait: Option<u64>,
    doc: Option<u64>,
}

// Parity A (QA4 area A, idle traffic): 55 s, under WebKit's 60 s request timeout; with the
// selection's long poll the idle editor asks about twice a minute (25 s made it five)
const STATE_WAIT: Duration = Duration::from_secs(55);

async fn api_state(Extension(ed): Extension<Ed>, q: Option<Query<WaitQ>>) -> Response {
    if let Some(Query(WaitQ { wait: Some(rev), doc })) = q {
        let until = std::time::Instant::now() + STATE_WAIT;
        loop {
            // (never queue behind a long command: a busy lock is "not yet")
            let now = match ed.session.try_lock() {
                Ok(g) => Some(g.as_ref().map(|s| (s.id, s.doc.revision, s.state()))),
                Err(std::sync::TryLockError::Poisoned(e)) => Some(e.into_inner().as_ref().map(|s| (s.id, s.doc.revision, s.state()))),
                Err(std::sync::TryLockError::WouldBlock) => None,
            };
            match now {
                Some(None) => return Json(json!({"error": NO_DOC})).into_response(),
                Some(Some((id, r, st))) if r != rev || doc.is_some_and(|d| d != id) || std::time::Instant::now() >= until => return Json(st).into_response(),
                _ => {}
            }
            if std::time::Instant::now() >= until {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        Ok(s.state())
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisReq {
    name: String,
    #[serde(default)]
    brushes: Vec<u64>,
    #[serde(default)]
    entities: Vec<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CordonReq {
    #[serde(default)]
    boxes: Vec<crate::editor::doc::CordonBox>,
    #[serde(default)]
    on: bool,
    #[serde(default)]
    seal: String,
    #[serde(default)]
    keep_lights: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsReq {
    vis_sets: Option<Vec<VisReq>>,
    cordon: Option<CordonReq>,
    /// Parity A: the name the map compiles to ("" goes back to the project's own)
    map_name: Option<String>,
}

/// The editor's settings kept in the project (visibility sets, the cordon). Not undoable;
/// the project counts as changed.
async fn api_settings(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<SettingsReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        let mut next = s.doc.editor.clone();
        if let Some(v) = req.vis_sets {
            if v.len() > 200 || v.iter().any(|x| x.brushes.len() + x.entities.len() > 200_000) {
                return Err("That's too many visibility sets.".into());
            }
            next.vis_sets = v
                .into_iter()
                .map(|x| crate::editor::doc::VisSet { name: crate::editor::prefab::clean_text(&x.name, 64), brushes: bids(&x.brushes), entities: eids(&x.entities) })
                .filter(|x| !x.name.is_empty())
                .collect();
        }
        if let Some(c) = req.cordon {
            if c.boxes.len() > 32 {
                return Err("Use at most 32 cordon boxes.".into());
            }
            for b in &c.boxes {
                let ok = (0..3).all(|i| b.min[i].is_finite() && b.max[i].is_finite() && b.min[i].abs() <= BIG && b.max[i].abs() <= BIG && b.max[i] - b.min[i] >= 16.0);
                if !ok {
                    return Err("A cordon box is too small or too far out.".into());
                }
            }
            next.cordon = crate::editor::doc::CordonSettings { boxes: c.boxes, on: c.on, seal: if c.seal == "black" { "black".into() } else { String::new() }, keep_lights: c.keep_lights };
        }
        if let Some(n) = req.map_name {
            let n = n.trim().to_string();
            if !n.is_empty() && !super::compile::valid_map_name(&n) {
                return Err(super::compile::map_name_problem(&n).unwrap_or("That isn't a map name.").into());
            }
            next.map_name = (!n.is_empty()).then_some(n);
        }
        if next != s.doc.editor {
            s.doc.editor = next;
            s.history.mark_unsaved();
            // (a new revision: other pages and the poll pick it up)
            s.doc.revision += 1;
        }
        let mut v = s.state();
        v["editor"] = settings_json(&s.doc);
        Ok(v)
    })
    .await
}

/// Choose the map's own FGD file (read on top of ours); kept in the project.
async fn api_fgd_pick(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if ed.lock().is_none() {
            return Err(NO_DOC.into());
        }
        let Some(path) = dialog(&st, Pick::Fgd, "")? else { return Ok(json!({"cancelled": true})) };
        // read it now so a broken file is said at once
        let n = fgd::parse(&read_text(&path)?).map_err(|e| format!("That FGD file couldn't be read: {e}"))?.len();
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        s.doc.editor.fgd = Some(path.to_string_lossy().to_string());
        s.history.mark_unsaved();
        s.doc.revision += 1;
        let mut v = s.state();
        v["editor"] = settings_json(&s.doc);
        v["classes"] = json!(n);
        Ok(v)
    })
    .await
}

async fn api_fgd_clear(Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        if s.doc.editor.fgd.take().is_some() {
            s.history.mark_unsaved();
            s.doc.revision += 1;
        }
        let mut v = s.state();
        v["editor"] = settings_json(&s.doc);
        Ok(v)
    })
    .await
}

/// A game path an entity key names ("ambience/wind1.wav", "models/can.mdl"): relative, plain parts.
fn game_rel(p: &str, ext: &[&str]) -> Option<String> {
    let p = p.trim().trim_start_matches(['!', '*']).replace('\\', "/");
    let low = p.to_ascii_lowercase();
    let ok = !p.is_empty() && p.len() <= 200 && !p.starts_with('/') && !p.contains(':') && p.split('/').all(|x| !x.is_empty() && x != "." && x != "..") && ext.iter().any(|e| low.ends_with(e));
    ok.then_some(p)
}

#[derive(Deserialize)]
struct SoundQ {
    path: String,
}

/// A sound an entity plays (ambient_generic's `message`, a button's sound), for the
/// inspector's play button: looked for under sound/ in the game's folders.
async fn api_sound(State(st): State<Shared>, h: HeaderMap, Query(q): Query<SoundQ>) -> Response {
    let nf = || (StatusCode::NOT_FOUND, Json(json!({"error": "That sound isn't in the game's folders."}))).into_response();
    let Some(rel) = game_rel(&q.path, &[".wav", ".mp3"]) else { return nf() };
    let rel = if rel.to_ascii_lowercase().starts_with("sound/") { rel } else { format!("sound/{rel}") };
    let dirs: Vec<PathBuf> = st.local_game().map(|g| ["cstrike", "cstrike_downloads", "czero", "valve"].iter().map(|f| g.hl.join(f)).collect()).unwrap_or_default();
    let found = tokio::task::spawn_blocking(move || {
        let finder = crate::mapcopy::Finder::default();
        dirs.iter().find_map(|d| finder.find(d, &rel)).and_then(|p| crate::caps::read_file(&p, "sound", 32 << 20).ok().map(|b| (p, b))) // [Caps] checked before reading
    })
    .await
    .ok()
    .flatten();
    let Some((p, data)) = found else { return nf() };
    let ctype = if p.to_string_lossy().to_ascii_lowercase().ends_with(".mp3") { "audio/mpeg" } else { "audio/wav" };
    let len = data.len();
    // WebKit plays audio only from servers that answer byte ranges
    let range = h.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|r| crate::sounds::parse_range(r, len));
    match range {
        Some((a, b)) => (
            StatusCode::PARTIAL_CONTENT,
            [(header::CONTENT_TYPE, ctype.to_string()), (header::ACCEPT_RANGES, "bytes".into()), (header::CONTENT_RANGE, format!("bytes {a}-{b}/{len}"))],
            data[a..=b].to_vec(),
        )
            .into_response(),
        None => ([(header::CONTENT_TYPE, ctype.to_string()), (header::ACCEPT_RANGES, "bytes".into())], data).into_response(),
    }
}

async fn api_recent(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || Ok(recent_json(&ed.dir))).await
}

#[derive(Deserialize)]
struct IndexReq {
    index: usize,
}

async fn api_open_recent(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<IndexReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let dirs = wad_dirs(&st);
    blocking(move || {
        let mut r = recent_list(&ed.dir).get(req.index).cloned().ok_or("That file isn't in the recent list any more.")?;
        // P5: a project renamed or moved outside the app: found by its uid
        if !r.path.is_file() || (r.uid.is_some() && file_uid(&r.path) != r.uid) {
            match r.uid.as_deref().and_then(|u| find_project(&ed.dir, u, Some(&r.path))) {
                Some(p) => r.path = p,
                None if !r.path.is_file() => return Err("That file isn't there any more. It may have been moved or deleted.".into()),
                None => {}
            }
        }
        let (doc, project) = load_file(&r.path)?;
        remember(&ed.dir, &r.path, project);
        Ok(ed.install_from(doc, stem(&r.path), project.then_some(r.path.clone()), Some(&r.path), &dirs))
    })
    .await
}

async fn api_autosave(Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let snap = {
            let g = ed.lock();
            AutosaveSnap::take(g.as_ref().ok_or(NO_DOC)?)
        };
        let saved = match snap {
            Some(a) => a.write(&ed.dir).map(|_| true)?,
            None => false,
        };
        Ok(json!({"saved": saved}))
    })
    .await
}

async fn api_recovery(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || Ok(recovery_json(&ed.dir))).await
}

/// Opens the autosave as the document (with unsaved changes; Save goes to its project, if
/// it had one).
async fn api_recover(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let dirs = wad_dirs(&st);
    blocking(move || {
        let path = ed.dir.join(AUTOSAVE);
        if !path.is_file() {
            return Err("There's nothing to recover.".into());
        }
        let doc = MapDocument::from_json(&read_text(&path)?)?;
        let meta: Value = std::fs::read_to_string(ed.dir.join(AUTOSAVE_META)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
        let name = meta["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("Recovered").to_string();
        let mut project = meta["project"].as_str().map(PathBuf::from).filter(|p| p.extension().is_some_and(|e| e == PROJECT_EXT));
        // P5: the project was renamed or moved since: find it by its uid
        if let (Some(p), Some(u)) = (&project, doc.editor.uid.as_deref()) {
            if !p.is_file() || file_uid(p).as_deref() != Some(u) {
                project = find_project(&ed.dir, u, Some(p));
            }
        }
        let mut st = ed.install(doc, name, project, &dirs);
        // it isn't saved anywhere yet: mark it changed so Save (and the next autosave) keep it
        let mut g = ed.lock();
        if let Some(s) = g.as_mut() {
            s.history.mark_unsaved();
            st = s.state();
        }
        Ok(st)
    })
    .await
}

async fn api_discard_recovery(Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let _ = std::fs::remove_file(ed.dir.join(AUTOSAVE));
        let _ = std::fs::remove_file(ed.dir.join(AUTOSAVE_META));
        Ok(json!({"discarded": true}))
    })
    .await
}

fn routes_with(ed: Ed) -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/doc", get(api_doc))
        .route("/api/tools/editor/meshes", get(api_meshes))
        .route("/api/tools/editor/checks", get(api_checks))
        .route("/api/tools/editor/textures", get(api_textures))
        .route("/api/tools/editor/wads", get(api_wads))
        .route("/api/tools/editor/tex/:file", get(api_tex))
        // P5: a paste can carry a big map's pieces (Paste takes up to 16 MB of .map text; JSON
        // escaping adds to it): over the app's default 2 MB request limit
        .route("/api/tools/editor/cmd", post(api_cmd).layer(axum::extract::DefaultBodyLimit::max(40 << 20)))
        .route("/api/tools/editor/undo", post(api_undo))
        .route("/api/tools/editor/redo", post(api_redo))
        .route("/api/tools/editor/new", post(api_new))
        .route("/api/tools/editor/open", post(api_open))
        .route("/api/tools/editor/import-map", post(api_import))
        .route("/api/tools/editor/save", post(api_save))
        .route("/api/tools/editor/export-map", post(api_export))
        .route("/api/tools/editor/copy", post(api_copy))
        .route("/api/tools/editor/fgd", get(api_fgd))
        .route("/api/tools/editor/recent", get(api_recent))
        .route("/api/tools/editor/open-recent", post(api_open_recent))
        .route("/api/tools/editor/autosave", post(api_autosave))
        .route("/api/tools/editor/recovery", get(api_recovery))
        .route("/api/tools/editor/recover", post(api_recover))
        .route("/api/tools/editor/discard-recovery", post(api_discard_recovery))
        .route("/api/tools/editor/state", get(api_state))
        .route("/api/tools/editor/settings", post(api_settings))
        .route("/api/tools/editor/fgd/pick", post(api_fgd_pick))
        .route("/api/tools/editor/fgd/clear", post(api_fgd_clear))
        .route("/api/tools/editor/sound", get(api_sound))
        .merge(prefabs::routes())
        .merge(components::routes())
        .merge(workspace::routes()) // P5
        .merge(assets::routes()) // P5
        .merge(kits::routes()) // P5
        .merge(browser::routes()) // P5
        .merge(importexport::routes()) // Parity A
        .merge(budget::routes()) // P5
        .merge(selection::routes()) // MCP
        .merge(leak::routes()) // P5
        .merge(io::routes()) // P5
        .layer(Extension(ed))
}

/// The editor's routes (merged into the tools' local routes).
pub fn routes() -> Router<Shared> {
    routes_with(global())
}

// ---- hook for tools/compile.rs (agent Q): the open document, copied for compiling ----
/// A copy of the open document and its name; None when no map is open.
pub fn open_document() -> Option<(MapDocument, String)> {
    global().lock().as_ref().map(|s| (s.doc.clone(), s.name.clone()))
}

/// P5 (share and remix): opens a project file in the editor as Open does, and puts it in Recent.
pub fn open_project_path(st: &Shared, path: &Path) -> Result<Value, String> {
    let ed = global();
    let dirs = wad_dirs(st);
    let (doc, project) = load_file(path)?;
    remember(&ed.dir, path, project);
    Ok(ed.install_from(doc, stem(path), project.then_some(path.to_path_buf()), Some(path), &dirs))
}

/// P5 (share and remix): where remixed community maps are kept (the editor's own folder).
pub fn remix_dir() -> PathBuf {
    global().dir.join("remixes")
}

/// P5 (share and remix): the projects in the recent list that are on this computer, newest
/// first (Publish offers them as the map's editable source).
pub fn recent_projects() -> Vec<PathBuf> {
    recent_list(&global().dir).into_iter().filter(|r| r.project && r.path.is_file()).map(|r| r.path).collect()
}

/// The folder the open map came from (its project's, or the imported file's), for finding
/// texture files kept beside it (P4, QA-1).
pub fn open_document_folder() -> Option<PathBuf> {
    global().lock().as_ref().and_then(|s| s.folder.clone())
}

/// Tests elsewhere (the MCP connector's end-to-end test): the editor's routes over their own
/// editor in `dir`, never the app's (recent files, autosave and prefabs stay in `dir`).
#[cfg(test)]
pub fn routes_in(dir: PathBuf) -> Router<Shared> {
    routes_with(Arc::new(Editor::with_dir(dir)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::AppState;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::RwLock;
    use tower::ServiceExt;

    fn state() -> Shared {
        Arc::new(AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        })
    }

    static NEXT_T: AtomicU64 = AtomicU64::new(0);

    struct T {
        st: Shared,
        ed: Ed,
    }

    impl T {
        fn new() -> T {
            let dir = std::env::temp_dir().join(format!("ls-editor-t-{}-{}", std::process::id(), NEXT_T.fetch_add(1, Ordering::Relaxed)));
            let _ = std::fs::remove_dir_all(&dir);
            T { st: state(), ed: Arc::new(Editor::with_dir(dir)) }
        }
        async fn raw(&self, method: &str, uri: &str, body: Option<Value>, panel: bool) -> (StatusCode, Vec<u8>) {
            let mut req = Request::builder().method(method).uri(uri).header("host", "127.0.0.1:8027");
            if panel {
                req = req.header("x-panel", "1");
            }
            let body = match body {
                Some(v) => {
                    req = req.header("content-type", "application/json");
                    Body::from(v.to_string())
                }
                None => Body::empty(),
            };
            let res = routes_with(self.ed.clone()).with_state(self.st.clone()).oneshot(req.body(body).unwrap()).await.unwrap();
            let status = res.status();
            (status, axum::body::to_bytes(res.into_body(), 64 << 20).await.unwrap().to_vec())
        }
        async fn get(&self, uri: &str) -> Value {
            let (s, b) = self.raw("GET", uri, None, false).await;
            assert_eq!(s, StatusCode::OK, "{uri}");
            serde_json::from_slice(&b).unwrap()
        }
        async fn post(&self, uri: &str, body: Value) -> Value {
            let (s, b) = self.raw("POST", uri, Some(body), true).await;
            assert_eq!(s, StatusCode::OK, "{uri}: {}", String::from_utf8_lossy(&b));
            serde_json::from_slice(&b).unwrap()
        }
        async fn cmd(&self, body: Value) -> Value {
            self.post("/api/tools/editor/cmd", body).await
        }
        async fn meshes(&self, doc: u64, since: Option<u64>) -> (Value, HashMap<String, Vec<u8>>) {
            let q = since.map(|s| format!("&since={s}")).unwrap_or_default();
            let (s, b) = self.raw("GET", &format!("/api/tools/editor/meshes?doc={doc}{q}"), None, false).await;
            assert_eq!(s, StatusCode::OK);
            goldsrc::mesh::read_bundle(&b)
        }
    }

    /// Checks: the checks run on a snapshot, without the editor's lock: an edit made while they
    /// run on a big map goes through at once, the run that saw the old document answers
    /// `stale`, and a fix from the last answer still applies by id after another edit.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn checks_run_off_the_lock() {
        let t = T::new();
        let mut d = starter_map();
        for i in 0..60 {
            for j in 0..60 {
                let (x, y) = (-3000.0 + i as f64 * 96.0, -3000.0 + j as f64 * 96.0);
                d.insert_brush(ops::make_box(v3(x, y, 300.0), v3(x + 64.0, y + 64.0, 300.0 + 16.0 + (i % 5) as f64 * 8.0), "CRATE"));
            }
        }
        let mut sp = Entity::new("info_player_deathmatch");
        sp.set("origin", "200 200 30"); // sunk 6 units into the starter floor
        let spawn = d.insert_entity(sp);
        t.ed.install(d, "de_offlock".into(), None, &[]);
        let t2 = T { st: t.st.clone(), ed: t.ed.clone() };
        let started = std::time::Instant::now();
        let run = tokio::spawn(async move {
            let v = t2.get("/api/tools/editor/checks").await;
            (v, started.elapsed())
        });
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let t0 = std::time::Instant::now();
        t.cmd(json!({"op": "add_box", "min": [0, 0, 400], "max": [64, 64, 464], "texture": "CRATE"})).await;
        let edit = t0.elapsed();
        let (v, checks) = run.await.unwrap();
        assert!(checks > std::time::Duration::from_millis(400), "the checks were too quick to show anything: {checks:?}");
        assert!(edit * 3 < checks, "the edit waited for the checks: edit {edit:?}, checks {checks:?}");
        assert_eq!(v["stale"], true, "{v}");
        // asked again: the current document, then the same answer from memory
        let c = t.get("/api/tools/editor/checks").await;
        assert!(c.get("stale").is_none());
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(c["revision"], doc["revision"]);
        let t1 = std::time::Instant::now();
        assert_eq!(t.get("/api/tools/editor/checks").await, c);
        assert!(t1.elapsed() < std::time::Duration::from_millis(400));
        // a fix from that answer, after another edit: still applied by id
        let stuck = c["issues"].as_array().unwrap().iter().find(|i| i["code"] == "stuck-in-solid" && i["entity"] == spawn.0).cloned().expect("stuck spawn");
        t.cmd(json!({"op": "add_box", "min": [512, 0, 400], "max": [576, 64, 464], "texture": "CRATE"})).await;
        let r = t.cmd(json!({"op": "fix", "fixes": [{"fix": stuck["fix"], "entity": spawn.0, "fixData": stuck["fixData"]}]})).await;
        assert!(r.get("error").is_none(), "{r}");
        let doc = t.get("/api/tools/editor/doc").await;
        let e = doc["entities"].as_array().unwrap().iter().find(|e| e["id"] == spawn.0).unwrap();
        assert!(e["keys"].as_array().unwrap().iter().any(|kv| kv[0] == "origin" && kv[1] == "200 200 36"), "{e}");
    }

    /// For trying the page without the app: serves only the editor routes on 127.0.0.1:8027
    /// (what `npm run dev` proxies to), with the game from LS_TEST_GAME when set. File dialogs
    /// need the app, so open/save/import/export answer an error here.
    /// `LS_TEST_GAME=... [LS_EDITOR_PORT=8127] cargo test serve_editor_for_ui -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn serve_editor_for_ui() {
        let t = T::new();
        if let Ok(hl) = std::env::var("LS_TEST_GAME") {
            let app = std::env::temp_dir().join("ls-editor-serve");
            *t.st.current.write().unwrap() = Some(Arc::new(crate::game::Game::with_app_dir(Path::new(&hl), "cstrike", &app)));
        }
        let app = routes_with(t.ed.clone()).with_state(t.st.clone());
        // LS_EDITOR_PORT picks another port when 8027 is taken (point a dev server's /api there)
        let port = std::env::var("LS_EDITOR_PORT").ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or(8027);
        let l = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        println!("editor routes on http://127.0.0.1:{port}");
        axum::serve(l, app).await.unwrap();
    }

    #[tokio::test]
    async fn prefabs_settings_and_state_routes() {
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        let st = t.get("/api/tools/editor/state").await;
        let rev0 = st["revision"].as_u64().unwrap();
        // the library lists the starter prefabs
        let l = t.get("/api/tools/editor/prefabs").await;
        assert!(l["prefabs"].as_array().unwrap().iter().any(|p| p["id"] == "builtin:stairs"));
        let (code, png) = t.raw("GET", "/api/tools/editor/prefab/builtin~stairs/thumb.png", None, false).await;
        assert_eq!(code, StatusCode::OK);
        assert!(png.starts_with(&[0x89, b'P']));
        // place one on the floor: one step, grouped, picked up by the state poll
        let r = t.cmd(json!({"op": "place_prefab", "prefab": "builtin:stairs", "point": [100, 37, 0], "normal": [0, 0, 1], "grid": 16})).await;
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["created"]["brushes"].as_array().unwrap().len(), 8);
        assert_eq!(r["note"], "Placed Stairs.");
        let st = t.get("/api/tools/editor/state").await;
        assert!(st["revision"].as_u64().unwrap() > rev0);
        assert_eq!(st["undo"], "Place prefab");
        let doc = t.get("/api/tools/editor/doc").await;
        assert!(doc["groups"].as_array().unwrap().iter().any(|g| g["name"] == "Stairs"));
        assert!(t.cmd(json!({"op": "place_prefab", "prefab": "mine:nope", "point": [0, 0, 0]})).await["error"].is_string());
        // save the placed pieces as my own prefab, then place that
        let ids: Vec<u64> = r["created"]["brushes"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        let saved = t.post("/api/tools/editor/prefabs/save", json!({"name": "My stairs", "tags": ["stairs"], "brushes": ids})).await;
        assert_eq!(saved["id"], "mine:my_stairs", "{saved}");
        assert!(t.post("/api/tools/editor/prefabs/save", json!({"name": " ", "brushes": ids})).await["error"].is_string());
        assert!(t.post("/api/tools/editor/prefabs/save", json!({"name": "x", "brushes": ids, "thumb": "data:image/png;base64,AAAA"})).await["error"].is_string());
        let r2 = t.cmd(json!({"op": "place_prefab", "prefab": "mine:my_stairs", "point": [-200, 0, 0]})).await;
        assert_eq!(r2["created"]["brushes"].as_array().unwrap().len(), 8);
        assert!(t.post("/api/tools/editor/prefabs/edit", json!({"id": "mine:my_stairs", "name": "Steps", "tags": ["a", "b"]})).await["ok"] == json!(true));
        assert!(t.post("/api/tools/editor/prefabs/edit", json!({"id": "builtin:stairs", "name": "x"})).await["error"].is_string());
        assert!(t.post("/api/tools/editor/prefabs/delete", json!({"id": "builtin:stairs"})).await["error"].is_string());
        assert_eq!(t.post("/api/tools/editor/prefabs/delete", json!({"id": "mine:my_stairs"})).await["ok"], true);
        // settings kept in the project: visibility sets and the cordon
        let v = t.post("/api/tools/editor/settings", json!({"visSets": [{"name": "Stairs", "brushes": ids}], "cordon": {"boxes": [{"min": [0, 0, 0], "max": [256, 256, 128]}], "on": true, "seal": "black", "keepLights": true}})).await;
        assert_eq!(v["dirty"], true);
        assert_eq!(v["editor"]["cordon"]["seal"], "black");
        assert!(t.post("/api/tools/editor/settings", json!({"cordon": {"boxes": [{"min": [0, 0, 0], "max": [4, 256, 128]}]}})).await["error"].is_string());
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(doc["editor"]["visSets"][0]["brushes"].as_array().unwrap().len(), 8);
        {
            let g = t.ed.lock();
            let d = &g.as_ref().unwrap().doc;
            let back = MapDocument::from_json(&d.to_json()).unwrap();
            assert_eq!(back.editor.cordon.boxes.len(), 1);
            assert!(back.editor.cordon.keep_lights);
        }
        // Parity A: the name the map compiles to, kept in the project (Save as <name>_edit)
        let v = t.post("/api/tools/editor/settings", json!({"mapName": "de_lonely_edit"})).await;
        assert_eq!(v["editor"]["mapName"], "de_lonely_edit");
        assert!(t.post("/api/tools/editor/settings", json!({"mapName": "con"})).await["error"].is_string());
        {
            let g = t.ed.lock();
            let s = g.as_ref().unwrap();
            let back = MapDocument::from_json(&s.doc.to_json()).unwrap();
            assert_eq!(super::super::compile::project_map_name(&back, "Lonely Remix"), "de_lonely_edit");
        }
        assert_eq!(t.post("/api/tools/editor/settings", json!({"mapName": ""})).await["editor"]["mapName"], Value::Null);
        // sounds: only plain game paths
        assert_eq!(game_rel("ambience/wind1.wav", &[".wav"]).as_deref(), Some("ambience/wind1.wav"));
        assert_eq!(game_rel("!ambience\\x.wav", &[".wav"]).as_deref(), Some("ambience/x.wav"));
        assert!(game_rel("../x.wav", &[".wav"]).is_none() && game_rel("/etc/x.wav", &[".wav"]).is_none() && game_rel("c:/x.wav", &[".wav"]).is_none() && game_rel("x.exe", &[".wav"]).is_none());
        let (code, _) = t.raw("GET", "/api/tools/editor/sound?path=..%2Fx.wav", None, false).await;
        assert_eq!(code, StatusCode::NOT_FOUND);
        // the FGD answers without a custom file
        assert!(t.get("/api/tools/editor/fgd").await["classes"].as_array().unwrap().len() > 50);
    }

    #[tokio::test]
    async fn new_map_doc_and_full_meshes() {
        let t = T::new();
        // nothing open yet: plain words, not a crash
        assert_eq!(t.get("/api/tools/editor/doc").await["error"], NO_DOC);
        let (s, _) = t.raw("GET", "/api/tools/editor/meshes", None, false).await;
        assert_eq!(s, StatusCode::NOT_FOUND);

        let st = t.post("/api/tools/editor/new", json!({})).await;
        assert_eq!(st["name"], "Untitled");
        assert_eq!(st["dirty"], false);
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(doc["brushes"].as_array().unwrap().len(), 6);
        assert_eq!(doc["entities"].as_array().unwrap().iter().filter(|e| e["classname"] == "info_player_start").count(), 2);
        assert_eq!(doc["entities"].as_array().unwrap().iter().filter(|e| e["classname"] == "info_player_deathmatch").count(), 2);

        let (h, sec) = t.meshes(doc["docId"].as_u64().unwrap(), None).await;
        assert_eq!(h["full"], true);
        assert_eq!(h["brushes"].as_array().unwrap().len(), 6);
        let faces: usize = h["brushes"].as_array().unwrap().iter().map(|b| b["faces"].as_array().unwrap().len()).sum();
        assert_eq!(faces, 36);
        // every box face is a quad: 4 vertices each, positions x3, uvs x2
        assert_eq!(sec["pos"].len(), 36 * 4 * 3 * 4);
        assert_eq!(sec["uv"].len(), 36 * 4 * 2 * 4);

        // the starter map passes its own checks (spawns are there)
        let c = t.get("/api/tools/editor/checks").await;
        assert!(!c["issues"].as_array().unwrap().iter().any(|i| i["code"].as_str().unwrap_or("").contains("spawn")), "{c}");
    }

    #[tokio::test]
    async fn commands_undo_redo_and_meshes_since() {
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        let doc = t.get("/api/tools/editor/doc").await;
        let id = doc["docId"].as_u64().unwrap();
        let r0 = doc["revision"].as_u64().unwrap();

        let r = t.cmd(json!({"op": "add_box", "min": [0, 0, 0], "max": [64, 64, 64], "texture": "CRATE01"})).await;
        let new_id = r["created"]["brushes"][0].as_u64().unwrap();
        assert_eq!(r["changed"], true);
        assert_eq!(r["state"]["undo"], "Add box");
        assert_eq!(r["state"]["dirty"], true);
        let r1 = r["state"]["revision"].as_u64().unwrap();
        assert!(r1 > r0);

        // only the new brush comes back
        let (h, _) = t.meshes(id, Some(r0)).await;
        assert_eq!(h["full"], false);
        let ids: Vec<u64> = h["brushes"].as_array().unwrap().iter().map(|b| b["id"].as_u64().unwrap()).collect();
        assert_eq!(ids, vec![new_id]);
        // up to date: nothing
        let (h, _) = t.meshes(id, Some(r1)).await;
        assert!(h["brushes"].as_array().unwrap().is_empty());

        // a push/pull drag in three steps is one undo step, computed from where it started
        for (i, dist) in [16.0, 32.0, 48.0].iter().enumerate() {
            let r = t.cmd(json!({"op": "move_face", "brush": new_id, "face": 0, "distance": dist, "drag": "d1"})).await;
            assert!(r.get("error").is_none() || r["error"].is_null(), "step {i}: {r}");
        }
        let doc2 = t.get("/api/tools/editor/doc").await;
        let b = doc2["brushes"].as_array().unwrap().iter().find(|b| b["id"] == new_id).unwrap().clone();
        let size: Vec<f64> = (0..3).map(|k| b["max"][k].as_f64().unwrap() - b["min"][k].as_f64().unwrap()).collect();
        assert!(size.iter().any(|s| (*s - 112.0).abs() < 1e-6), "{size:?}");
        assert_eq!(doc2["undo"], "Move face");

        let u = t.post("/api/tools/editor/undo", json!({})).await;
        assert_eq!(u["label"], "Move face");
        assert_eq!(u["state"]["undo"], "Add box");
        let doc3 = t.get("/api/tools/editor/doc").await;
        let b = doc3["brushes"].as_array().unwrap().iter().find(|b| b["id"] == new_id).unwrap().clone();
        assert_eq!(b["max"], json!([64.0, 64.0, 64.0]));

        // undo the box too: meshes since r1 report it removed
        let u = t.post("/api/tools/editor/undo", json!({})).await;
        assert_eq!(u["label"], "Add box");
        assert_eq!(u["state"]["dirty"], false, "{u}");
        let (h, _) = t.meshes(id, Some(r1)).await;
        assert_eq!(h["removed"], json!([new_id]));

        let rd = t.post("/api/tools/editor/redo", json!({})).await;
        assert_eq!(rd["label"], "Add box");
        assert_eq!(t.get("/api/tools/editor/doc").await["brushes"].as_array().unwrap().len(), 7);
        // nothing to redo past the end: a plain answer
        t.post("/api/tools/editor/redo", json!({})).await;
        let rd = t.post("/api/tools/editor/redo", json!({})).await;
        assert_eq!(rd["changed"], false);

        // another document: an old docId gets everything
        t.post("/api/tools/editor/new", json!({})).await;
        let (h, _) = t.meshes(id, Some(r1)).await;
        assert_eq!(h["full"], true);
    }

    #[tokio::test]
    async fn bad_commands_change_nothing() {
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        let before = t.get("/api/tools/editor/doc").await;

        // no panel header
        let (s, _) = t.raw("POST", "/api/tools/editor/cmd", Some(json!({"op": "undo"})), false).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        let (s, _) = t.raw("POST", "/api/tools/editor/new", None, false).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        // not a command
        let (s, _) = t.raw("POST", "/api/tools/editor/cmd", Some(json!({"op": "explode"})), true).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        let first = before["brushes"][0]["id"].as_u64().unwrap();
        for body in [
            json!({"op": "add_box", "min": [0, 0, 0], "max": [64, 64, 0], "texture": "CRATE01"}),
            json!({"op": "add_box", "min": [0, 0, 0], "max": [1e30, 64, 64], "texture": "CRATE01"}),
            json!({"op": "add_box", "min": [0, 0, 0], "max": [64, 64, 64], "texture": "a\"b"}),
            json!({"op": "add_cylinder", "base": [0, 0, 0], "radius": 32, "height": 64, "sides": 999, "texture": "X"}),
            json!({"op": "move_face", "brush": first, "face": 99, "distance": 8}),
            json!({"op": "move_face", "brush": first, "face": 0, "distance": -100000}),
            json!({"op": "move_face", "brush": 999999, "face": 0, "distance": 8}),
            json!({"op": "hollow", "brushes": [first], "thickness": 5000}),
            json!({"op": "merge", "brushes": [first]}),
            json!({"op": "merge", "brushes": []}),
            json!({"op": "clip", "brushes": [first], "points": [[0, 0, 0], [0, 0, 0], [0, 0, 0]], "keep": "back"}),
            json!({"op": "set_key", "entity": 999999, "key": "a", "value": "b"}),
            json!({"op": "transform", "brushes": [first], "scale": [0, 1, 1]}),
            json!({"op": "transform", "brushes": [], "entities": []}),
            json!({"op": "set_texture", "faces": [], "texture": "X"}),
            json!({"op": "align", "faces": [[first, 0]], "action": "bogus"}),
            json!({"op": "add_entity", "classname": "bad name!", "origin": [0, 0, 0]}),
        ] {
            let r = t.cmd(body.clone()).await;
            assert!(r["error"].is_string(), "{body} -> {r}");
        }
        let after = t.get("/api/tools/editor/doc").await;
        assert_eq!(before["brushes"], after["brushes"]);
        assert_eq!(before["entities"], after["entities"]);
        assert_eq!(after["undo"], Value::Null);
    }

    #[tokio::test]
    async fn modelling_commands() {
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        let r = t.cmd(json!({"op": "add_box", "min": [0, 0, 0], "max": [128, 128, 128], "texture": "CRATE01"})).await;
        let b = r["created"]["brushes"][0].as_u64().unwrap();

        let r = t.cmd(json!({"op": "hollow", "brushes": [b], "thickness": 8})).await;
        assert_eq!(r["created"]["brushes"].as_array().unwrap().len(), 6, "{r}");
        t.post("/api/tools/editor/undo", json!({})).await;

        let r = t.cmd(json!({"op": "clip", "brushes": [b], "points": [[64, 0, 0], [64, 0, 128], [64, 128, 0]], "keep": "both"})).await;
        assert_eq!(r["created"]["brushes"].as_array().unwrap().len(), 1, "{r}");
        t.post("/api/tools/editor/undo", json!({})).await;

        let r = t.cmd(json!({"op": "transform", "brushes": [b], "rotate": {"axis": [0, 0, 1], "degrees": 90}})).await;
        assert!(r["error"].is_null(), "{r}");
        let r = t.cmd(json!({"op": "transform", "brushes": [b], "translate": [16, 0, 0]})).await;
        assert_eq!(r["state"]["undo"], "Move");

        let r = t.cmd(json!({"op": "set_texture", "faces": [[b, null]], "texture": "OUT_WALL5"})).await;
        assert!(r["error"].is_null(), "{r}");
        let r = t.cmd(json!({"op": "align", "faces": [[b, 0]], "action": "shift", "u": 8, "v": 0, "drag": "t1"})).await;
        assert!(r["error"].is_null(), "{r}");
        t.cmd(json!({"op": "align", "faces": [[b, 0]], "action": "shift", "u": 8, "v": 0, "drag": "t1"})).await;
        let u = t.post("/api/tools/editor/undo", json!({})).await;
        assert_eq!(u["state"]["undo"], "Apply texture");

        let r = t.cmd(json!({"op": "add_entity", "classname": "light", "origin": [0, 0, 64], "keys": [["_light", "255 255 255 200"]]})).await;
        let e = r["created"]["entities"][0].as_u64().unwrap();
        assert_eq!(r["docChanged"], true);
        let r = t.cmd(json!({"op": "set_key", "entity": e, "key": "targetname", "value": "lamp"})).await;
        assert!(r["error"].is_null());
        let r = t.cmd(json!({"op": "tie", "brushes": [b], "classname": "func_wall"})).await;
        let fw = r["created"]["entities"][0].as_u64().unwrap();
        let doc = t.get("/api/tools/editor/doc").await;
        let ent = doc["entities"].as_array().unwrap().iter().find(|x| x["id"] == fw).unwrap().clone();
        assert_eq!(ent["brushes"], json!([b]));

        let r = t.cmd(json!({"op": "duplicate", "brushes": [b], "entities": [], "offset": [0, 0, 128]})).await;
        assert_eq!(r["created"]["brushes"].as_array().unwrap().len(), 1);
        let r = t.cmd(json!({"op": "group", "name": "Crates", "brushes": [b]})).await;
        assert!(r["error"].is_null(), "{r}");
        let r = t.cmd(json!({"op": "set_world", "keys": [["skyname", "night"]]})).await;
        assert!(r["error"].is_null());
        assert_eq!(t.get("/api/tools/editor/doc").await["world"]["keys"], json!([["skyname", "night"]]));
    }

    #[test]
    fn project_and_map_files_round_trip() {
        let dir = std::env::temp_dir().join(format!("ls-editor-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ed = Editor::with_dir(dir.join("ed"));
        ed.install(starter_map(), "Untitled".into(), None, &[]);
        let mut g = ed.lock();
        let s = g.as_mut().unwrap();
        run_cmd(s, serde_json::from_value(json!({"op": "add_box", "min": [0, 0, 0], "max": [32, 32, 32], "texture": "CRATE01"})).unwrap()).unwrap();
        assert!(s.history.is_dirty());

        let proj = dir.join("room.lsmap");
        save_project(s, &proj, Some(&dir.join("backups"))).unwrap();
        // saving again keeps the old file as a backup
        save_project(s, &proj, Some(&dir.join("backups"))).unwrap();
        assert_eq!(std::fs::read_dir(dir.join("backups")).unwrap().count(), 1);
        assert!(!s.history.is_dirty());
        assert_eq!(s.name, "room");
        let (back, is_project) = load_file(&proj).unwrap();
        assert!(is_project);
        assert_eq!(back.brushes.len(), 7);

        let map = dir.join("room.map");
        export_map(s, &map).unwrap();
        let (m, is_project) = load_file(&map).unwrap();
        assert!(!is_project);
        assert_eq!(m.brushes.len(), 7);
        assert_eq!(m.entities.len(), s.doc.entities.len());

        // damaged files: plain words
        std::fs::write(dir.join("bad.map"), "{ \"classname\" \"worldspawn\" ( 0 0").unwrap();
        assert!(load_file(&dir.join("bad.map")).is_err());
        std::fs::write(dir.join("bad.lsmap"), "not json").unwrap();
        assert!(load_file(&dir.join("bad.lsmap")).is_err());
        assert!(load_file(&dir.join("missing.lsmap")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// P5/QA: a WAD beside the map's project or imported file counts for live checks and the
    /// texture list too (not only the game's folders); entity text keeps its code page on export.
    #[tokio::test]
    async fn wads_beside_the_map_and_code_pages() {
        let root = std::env::temp_dir().join(format!("ls-editor-beside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::create_dir_all(hl.join("valve")).unwrap();
        std::fs::write(hl.join("valve/halflife.wad"), wad::build_wad(&[("crete3_flr04", crate::goldsrc::bsp::fixture::miptex("crete3_flr04", true))])).unwrap();
        let proj = root.join("maps/russian");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("mine.wad"), wad::build_wad(&[("out_wall5", crate::goldsrc::bsp::fixture::miptex("out_wall5", true))])).unwrap();
        let g = crate::game::Game::with_app_dir(&hl, "cstrike", &root.join("app"));
        let t = T::new();
        *t.st.current.write().unwrap() = Some(Arc::new(g));
        let mut doc = starter_map();
        doc.world.wads = vec!["\\half-life\\valve\\halflife.wad".into(), "C:\\mapper\\mine.wad".into()];
        let mut text = Entity::new("game_text");
        text.set("origin", "0 0 64");
        text.set("message", "Фаза - #1");
        doc.insert_entity(text);
        let dirs = wad_dirs(&t.st);
        t.ed.install_from(doc, "russian".into(), None, Some(&proj.join("russian.rmf")), &dirs);
        // the texture list and the checks both see mine.wad beside the file
        let list = t.get("/api/tools/editor/textures").await;
        let mine = list["wads"].as_array().unwrap().iter().find(|w| w["name"] == "mine.wad").unwrap().clone();
        assert_eq!(mine["found"], true, "{list}");
        let c = t.get("/api/tools/editor/checks").await;
        let missing: Vec<String> = c["issues"].as_array().unwrap().iter().filter(|i| i["code"] == "missing-texture").filter_map(|i| i["message"].as_str().map(String::from)).collect();
        // (sky is in neither test WAD)
        assert!(!missing.iter().any(|m| m.to_ascii_lowercase().contains("out_wall5") || m.to_ascii_lowercase().contains("crete3")), "{missing:?}");
        // export: Cyrillic text in cp1251, read back the same
        let out = root.join("russian.map");
        {
            let g = t.ed.lock();
            let (_, note) = export_map(g.as_ref().unwrap(), &out).unwrap();
            assert!(note.is_none());
        }
        let raw = std::fs::read(&out).unwrap();
        assert!(raw.windows(4).any(|w| w == [0xD4, 0xE0, 0xE7, 0xE0]), "cp1251 bytes in the file");
        assert!(std::str::from_utf8(&raw).is_err());
        let (back, _) = load_file(&out).unwrap();
        assert!(back.entities.values().any(|e| e.get("message") == Some("Фаза - #1")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn textures_from_the_games_wads() {
        // a Half-Life folder with a WAD that has one texture
        let root = std::env::temp_dir().join(format!("ls-editor-wad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::create_dir_all(hl.join("valve")).unwrap();
        std::fs::write(hl.join("valve/halflife.wad"), wad::build_wad(&[("crete3_flr04", crate::goldsrc::bsp::fixture::miptex("crete3_flr04", true))])).unwrap();
        let g = crate::game::Game::with_app_dir(&hl, "cstrike", &root.join("app"));
        let t = T::new();
        *t.st.current.write().unwrap() = Some(Arc::new(g));

        t.post("/api/tools/editor/new", json!({})).await;
        let list = t.get("/api/tools/editor/textures").await;
        let hlw = list["wads"].as_array().unwrap().iter().find(|w| w["name"] == "halflife.wad").unwrap().clone();
        assert_eq!(hlw["found"], true);
        assert_eq!(hlw["textures"][0]["name"], "crete3_flr04");
        let cs = list["wads"].as_array().unwrap().iter().find(|w| w["name"] == "cstrike.wad").unwrap().clone();
        assert_eq!(cs["found"], false);

        let (s, b) = t.raw("GET", "/api/tools/editor/tex/CRETE3_FLR04.png", None, false).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(&b[1..4], b"PNG");
        let (s, _) = t.raw("GET", "/api/tools/editor/tex/nope.png", None, false).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, _) = t.raw("GET", "/api/tools/editor/tex/a%22b.png", None, false).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        // the floor's texture is known (sized), the walls' isn't: checks say so
        let c = t.get("/api/tools/editor/checks").await;
        let missing: Vec<&str> = c["issues"].as_array().unwrap().iter().filter(|i| i["code"] == "missing-texture").filter_map(|i| i["message"].as_str()).collect();
        assert!(missing.iter().any(|m| m.to_ascii_lowercase().contains("out_wall5")), "{missing:?}");
        assert!(!missing.iter().any(|m| m.to_ascii_lowercase().contains("crete3_flr04")), "{missing:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn extrude_split_and_merge() {
        // Parity A: Shift-drag a face pulls out a new brush; a drag's steps are one undo step
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        let n0 = t.get("/api/tools/editor/doc").await["brushes"].as_array().unwrap().len();
        let r = t.cmd(json!({"op": "add_box", "min": [0, 0, 0], "max": [64, 64, 64], "texture": "CRATE01"})).await;
        let b = r["created"]["brushes"][0].as_u64().unwrap();
        let mut made = 0;
        for (i, dist) in [16.0, 40.0, 0.0, 32.0].iter().enumerate() {
            let r = t.cmd(json!({"op": "extrude_face", "brush": b, "face": 0, "distance": dist, "drag": "x1"})).await;
            assert!(r["error"].is_null(), "step {i}: {r}");
            if let Some(id) = r["created"]["brushes"][0].as_u64() {
                made = id;
            }
        }
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(doc["undo"], "Extrude");
        assert_eq!(doc["brushes"].as_array().unwrap().len(), n0 + 2);
        let nb = doc["brushes"].as_array().unwrap().iter().find(|x| x["id"] == made).unwrap().clone();
        let size: Vec<f64> = (0..3).map(|i| nb["max"][i].as_f64().unwrap() - nb["min"][i].as_f64().unwrap()).collect();
        assert!(size.contains(&32.0) && size.iter().filter(|x| **x == 64.0).count() == 2, "{size:?}");
        // one undo takes the new brush away, the source stays
        t.post("/api/tools/editor/undo", json!({})).await;
        assert_eq!(t.get("/api/tools/editor/doc").await["brushes"].as_array().unwrap().len(), n0 + 1);
        // inwards is refused, nothing changes
        assert!(t.cmd(json!({"op": "extrude_face", "brush": b, "face": 0, "distance": -8})).await["error"].is_string());

        // split the top face's middle upwards, in two drag steps: one "Split"
        for z in [8.0, 24.0] {
            let r = t.cmd(json!({"op": "add_vertex", "brush": b, "at": [32, 32, 64], "delta": [0, 0, z], "drag": "s1"})).await;
            assert!(r["error"].is_null(), "{r}");
        }
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(doc["undo"], "Split");
        let top = doc["brushes"].as_array().unwrap().iter().find(|x| x["id"] == b).unwrap()["max"][2].as_f64().unwrap();
        assert_eq!(top, 88.0);
        assert!(t.cmd(json!({"op": "add_vertex", "brush": b, "at": [32, 32, 0], "delta": [0, 0, 8]})).await["error"].is_string());
        // merge two bottom corners
        let r = t.cmd(json!({"op": "merge_vertices", "brush": b, "picks": [[0, 0, 0], [64, 0, 0]]})).await;
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(t.get("/api/tools/editor/doc").await["undo"], "Merge corners");
        assert!(t.cmd(json!({"op": "merge_vertices", "brush": b, "picks": [[0, 0, 0]]})).await["error"].is_string());

        // shear: a fresh box leans 64 east at the top (Front view), one "Shear" step
        let r = t.cmd(json!({"op": "add_box", "min": [256, 0, 0], "max": [288, 128, 128], "texture": "CRATE01"})).await;
        let w = r["created"]["brushes"][0].as_u64().unwrap();
        let r = t.cmd(json!({"op": "shear", "brushes": [w], "axis": 2, "along": 0, "factor": 0.5, "pivot": [256, 0, 0]})).await;
        assert!(r["error"].is_null(), "{r}");
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(doc["undo"], "Shear");
        let nb = doc["brushes"].as_array().unwrap().iter().find(|x| x["id"] == w).unwrap().clone();
        assert_eq!(nb["max"][0].as_f64(), Some(352.0));
        assert_eq!(nb["min"][0].as_f64(), Some(256.0));
        let e = t.cmd(json!({"op": "shear", "brushes": [w], "axis": 2, "along": 0, "factor": 20, "pivot": [256, 0, 0]})).await;
        assert!(e["error"].as_str().unwrap_or("").contains("steep"), "{e}");
        assert!(t.cmd(json!({"op": "shear", "brushes": [w], "axis": 1, "along": 1, "factor": 0.5, "pivot": [0, 0, 0]})).await["error"].is_string());
    }

    #[tokio::test]
    async fn shapes_vertices_arrays_and_paste() {
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        for (shape, n) in [("wedge", 1), ("spike", 1), ("sphere", 1), ("arch", 8)] {
            let r = t.cmd(json!({"op": "add_shape", "shape": shape, "min": [0, 0, 0], "max": [128, 128, 64], "sides": 8, "texture": "ROCK", "grid": 1})).await;
            assert_eq!(r["created"]["brushes"].as_array().map(|a| a.len()), Some(n), "{shape}: {r}");
        }
        assert!(t.cmd(json!({"op": "add_shape", "shape": "blob", "min": [0, 0, 0], "max": [8, 8, 8], "texture": "ROCK"})).await["error"].is_string());
        // Parity A: cone (cut off), torus (with its own tube sides and a climb), arch add height
        let r = t.cmd(json!({"op": "add_shape", "shape": "cone", "min": [0, 0, 0], "max": [128, 128, 64], "sides": 12, "top": 0.5, "texture": "ROCK", "grid": 1})).await;
        assert_eq!(r["created"]["brushes"].as_array().map(|a| a.len()), Some(1), "cone: {r}");
        let r = t.cmd(json!({"op": "add_shape", "shape": "torus", "min": [0, 0, 0], "max": [256, 256, 32], "sides": 12, "tube_sides": 6, "wall": 32, "arc": 360, "add_height": 8, "texture": "PIPE", "grid": 1})).await;
        assert_eq!(r["created"]["brushes"].as_array().map(|a| a.len()), Some(12), "torus: {r}");
        assert_eq!(t.get("/api/tools/editor/doc").await["undo"], "Add torus");
        let r = t.cmd(json!({"op": "add_shape", "shape": "arch", "min": [0, 0, 0], "max": [256, 256, 16], "sides": 8, "wall": 32, "arc": 360, "add_height": 16, "texture": "STEP", "grid": 1})).await;
        assert_eq!(r["created"]["brushes"].as_array().map(|a| a.len()), Some(8), "spiral: {r}");
        assert!(t.cmd(json!({"op": "add_shape", "shape": "torus", "min": [0, 0, 0], "max": [64, 64, 32], "wall": 200, "texture": "PIPE"})).await["error"].is_string());
        assert!(t.cmd(json!({"op": "add_shape", "shape": "cone", "min": [0, 0, 0], "max": [64, 64, 32], "top": 2, "texture": "PIPE"})).await["error"].is_string());

        // a vertex drag in two steps: one undo step, from where it started
        let r = t.cmd(json!({"op": "add_box", "min": [0, 0, 0], "max": [64, 64, 64], "texture": "CRATE01"})).await;
        let b = r["created"]["brushes"][0].as_u64().unwrap();
        for dz in [8, 16] {
            let r = t.cmd(json!({"op": "move_vertices", "picks": [[b, [64, 64, 64]]], "delta": [0, 0, dz], "drag": "v1"})).await;
            assert!(r["error"].is_null(), "{r}");
        }
        let doc = t.get("/api/tools/editor/doc").await;
        let bb = doc["brushes"].as_array().unwrap().iter().find(|x| x["id"] == b).unwrap().clone();
        assert_eq!(bb["max"][2], 80.0);
        assert_eq!(doc["undo"], "Move corners");
        t.post("/api/tools/editor/undo", json!({})).await;
        let doc = t.get("/api/tools/editor/doc").await;
        assert_eq!(doc["brushes"].as_array().unwrap().iter().find(|x| x["id"] == b).unwrap()["max"][2], 64.0);
        let r = t.cmd(json!({"op": "move_vertices", "picks": [[b, [64, 64, 64]]], "delta": [-48, -48, -48]})).await;
        assert!(r["error"].as_str().unwrap().contains("convex"), "{r}");

        // meshes carry each face's alignment
        let (h, _) = t.meshes(doc["docId"].as_u64().unwrap(), None).await;
        assert_eq!(h["brushes"][0]["faces"][0]["align"].as_array().unwrap().len(), 11);

        // paste special: 3 copies stepping 64 along x
        let r = t.cmd(json!({"op": "array", "brushes": [b], "copies": 3, "offset": [64, 0, 0]})).await;
        assert_eq!(r["created"]["brushes"].as_array().unwrap().len(), 3, "{r}");
        t.post("/api/tools/editor/undo", json!({})).await;

        // copy as .map text, paste it back moved up
        let c = t.post("/api/tools/editor/copy", json!({"brushes": [b]})).await;
        let text = c["text"].as_str().unwrap().to_string();
        assert!(text.contains("worldspawn") && text.contains("CRATE01"));
        let r = t.cmd(json!({"op": "paste", "text": text, "offset": [0, 0, 128]})).await;
        assert_eq!(r["created"]["brushes"].as_array().unwrap().len(), 1, "{r}");
        assert!(t.cmd(json!({"op": "paste", "text": "hello", "offset": [0, 0, 0]})).await["error"].is_string());

        // face sheet: justify and direct values, treated as one
        let r = t.cmd(json!({"op": "align", "faces": [[b, 0], [b, 1]], "action": "justify", "mode": "left", "one": true})).await;
        assert!(r["error"].is_null(), "{r}");
        let r = t.cmd(json!({"op": "align", "faces": [[b, 0]], "action": "set", "su": 0.5, "rot": 45})).await;
        assert!(r["error"].is_null(), "{r}");
        assert!(t.cmd(json!({"op": "align", "faces": [[b, 0]], "action": "set", "su": 0})).await["error"].is_string());

        // check fixes: the starter map has no problems with a fix... add a dangling target
        let r = t.cmd(json!({"op": "add_entity", "classname": "trigger_relay", "origin": [0, 0, 64], "keys": [["target", "ghost"]]})).await;
        let e = r["created"]["entities"][0].as_u64().unwrap();
        let c = t.get("/api/tools/editor/checks").await;
        let i = c["issues"].as_array().unwrap().iter().find(|i| i["code"] == "dangling-target").unwrap().clone();
        assert_eq!(i["fix"], "clear-target");
        let r = t.cmd(json!({"op": "fix", "fixes": [{"fix": "clear-target", "entity": e}]})).await;
        assert_eq!(r["note"], "Fixed.");

        // the entity definitions
        let f = t.get("/api/tools/editor/fgd").await;
        assert!(f["classes"].as_array().unwrap().iter().any(|c| c["name"] == "func_buyzone"));
    }

    /// P5 (perf): each command answers with what changed (the page patches its copy), /doc
    /// gives the same since a revision, and /state waits for a change.
    #[tokio::test]
    async fn commands_answer_with_their_changes() {
        let t = T::new();
        let st = t.post("/api/tools/editor/new", json!({})).await;
        let doc_id = st["docId"].as_u64().unwrap();
        let full = t.get("/api/tools/editor/doc").await;
        let n_brushes = full["brushes"].as_array().unwrap().len();
        let rev0 = full["revision"].as_u64().unwrap();
        let b = full["brushes"][0]["id"].as_u64().unwrap();
        let before = full["brushes"][0].clone();
        // a nudge: just that brush, its bounds moved; no entities
        let r = t.cmd(json!({"op": "transform", "brushes": [b], "entities": [], "translate": [16, 0, 0]})).await;
        let c = &r["changes"];
        assert_eq!(c["from"], rev0);
        assert_eq!(c["revision"], r["state"]["revision"]);
        assert_eq!(c["brushes"].as_array().unwrap().len(), 1, "{c}");
        assert_eq!(c["brushes"][0]["id"], b);
        assert_eq!(c["brushes"][0]["min"][0].as_f64().unwrap(), before["min"][0].as_f64().unwrap() + 16.0);
        assert_eq!(c["entities"], json!([]));
        assert!(c["stats"]["brushes"].as_u64().unwrap() as usize == n_brushes && c["bounds"].is_array() && c["groups"].is_array());
        // an entity: added, then a key changed, then deleted
        let e = t.cmd(json!({"op": "add_entity", "classname": "light", "origin": [0, 0, 64]})).await;
        let eid = e["created"]["entities"][0].as_u64().unwrap();
        assert_eq!(e["changes"]["entities"][0]["id"], eid);
        assert_eq!(e["changes"]["brushes"], json!([]));
        let k = t.cmd(json!({"op": "set_key", "entity": eid, "key": "_light", "value": "255 0 0 200"})).await;
        assert!(k["changes"]["entities"][0]["keys"].to_string().contains("255 0 0 200"));
        let d = t.cmd(json!({"op": "delete", "brushes": [b], "entities": [eid]})).await;
        assert_eq!(d["changes"]["removedBrushes"], json!([b]));
        assert_eq!(d["changes"]["removedEntities"], json!([eid]));
        // undo answers with them back
        let u = t.post("/api/tools/editor/undo", json!({})).await;
        assert_eq!(u["changes"]["brushes"][0]["id"], b);
        assert_eq!(u["changes"]["entities"][0]["id"], eid);
        // a brush tied to an entity: the entity's brush list comes with the change
        let tie = t.cmd(json!({"op": "tie", "brushes": [b], "classname": "func_wall"})).await;
        let w = tie["created"]["entities"][0].as_u64().unwrap();
        let ent = tie["changes"]["entities"].as_array().unwrap().iter().find(|x| x["id"] == w).cloned().unwrap();
        assert_eq!(ent["brushes"], json!([b]));
        // /doc since a revision: everything since then; a mismatched doc id: the whole document
        let since = t.get(&format!("/api/tools/editor/doc?doc={doc_id}&since={rev0}")).await;
        assert_eq!(since["changes"], true);
        assert!(since["entities"].as_array().unwrap().iter().any(|x| x["id"] == w));
        let whole = t.get(&format!("/api/tools/editor/doc?doc={}&since={rev0}", doc_id + 99)).await;
        assert!(whole["changes"].is_null() && whole["brushes"].as_array().unwrap().len() >= n_brushes);
        // /state?wait: at once when the revision moved on, else after a change (or 25 s)
        let now = t.get("/api/tools/editor/state").await["revision"].as_u64().unwrap();
        assert_eq!(t.get(&format!("/api/tools/editor/state?wait={}&doc={doc_id}", now - 1)).await["revision"], now);
        let ed = t.ed.clone();
        let waiter = tokio::spawn({
            let uri = format!("/api/tools/editor/state?wait={now}&doc={doc_id}");
            let t2 = T { st: t.st.clone(), ed };
            async move { t2.get(&uri).await }
        });
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(!waiter.is_finished(), "it waits while nothing changes");
        t.cmd(json!({"op": "transform", "brushes": [b], "entities": [w], "translate": [16, 0, 0]})).await;
        let got = tokio::time::timeout(std::time::Duration::from_secs(3), waiter).await.unwrap().unwrap();
        assert!(got["revision"].as_u64().unwrap() > now);
    }

    /// P5: a big map's pieces paste in one request (over axum's default 2 MB limit).
    #[tokio::test]
    async fn a_big_paste_goes_through() {
        let t = T::new();
        t.post("/api/tools/editor/new", json!({})).await;
        let mut d = MapDocument::new();
        let mut h = History::default();
        for i in 0..12_000 {
            let x = (i % 100) as f64 * 80.0;
            let y = (i / 100) as f64 * 80.0;
            history::cmd_add_brush(&mut d, &mut h, crate::editor::ops::make_box(v3(x, y, 0.0), v3(x + 64.0, y + 64.0, 64.0), "CRATE1"), "Box");
        }
        let text = mapfile::write_map(&d, &mapfile::WriteOptions::default()).unwrap();
        assert!(text.len() > 4 << 20, "{}", text.len());
        let r = t.cmd(json!({"op": "paste", "text": text, "offset": [0, 0, 0]})).await;
        assert!(r["error"].is_null(), "{r}");
        assert!(r["created"]["brushes"].as_array().unwrap().len() >= 12_000);
    }

    #[tokio::test]
    async fn autosave_recovery_and_recent() {
        let t = T::new();
        assert_eq!(t.get("/api/tools/editor/recovery").await["found"], false);
        assert_eq!(t.get("/api/tools/editor/recent").await["files"], json!([]));
        t.post("/api/tools/editor/new", json!({})).await;
        // nothing changed: nothing written
        assert_eq!(t.post("/api/tools/editor/autosave", json!({})).await["saved"], false);
        t.cmd(json!({"op": "add_box", "min": [0, 0, 0], "max": [64, 64, 64], "texture": "CRATE01"})).await;
        assert_eq!(t.post("/api/tools/editor/autosave", json!({})).await["saved"], true);
        let r = t.get("/api/tools/editor/recovery").await;
        assert_eq!(r["found"], true);
        assert_eq!(r["brushes"], 7);

        // the app "crashed": a fresh editor on the same folder recovers it
        let t2 = T { st: t.st.clone(), ed: Arc::new(Editor::with_dir(t.ed.dir.clone())) };
        let st = t2.post("/api/tools/editor/recover", json!({})).await;
        assert_eq!(st["dirty"], true, "{st}");
        assert_eq!(t2.get("/api/tools/editor/doc").await["brushes"].as_array().unwrap().len(), 7);
        t2.post("/api/tools/editor/discard-recovery", json!({})).await;
        assert_eq!(t2.get("/api/tools/editor/recovery").await["found"], false);
        assert!(t2.post("/api/tools/editor/recover", json!({})).await["error"].is_string());

        // recent files: newest first, no duplicates, missing ones say so
        let f1 = t.ed.dir.join("a.lsmap");
        std::fs::create_dir_all(&t.ed.dir).unwrap();
        std::fs::write(&f1, starter_map().to_json()).unwrap();
        remember(&t.ed.dir, &f1, true);
        remember(&t.ed.dir, &t.ed.dir.join("gone.map"), false);
        remember(&t.ed.dir, &f1, true);
        let r = t.get("/api/tools/editor/recent").await;
        let files = r["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["name"], "a.lsmap");
        assert_eq!(files[1]["found"], false);
        let st = t.post("/api/tools/editor/open-recent", json!({"index": 0})).await;
        assert_eq!(st["name"], "a");
        assert!(t.post("/api/tools/editor/open-recent", json!({"index": 1})).await["error"].is_string());
        assert!(t.post("/api/tools/editor/open-recent", json!({"index": 9})).await["error"].is_string());
        let _ = std::fs::remove_dir_all(&t.ed.dir);
    }
}
