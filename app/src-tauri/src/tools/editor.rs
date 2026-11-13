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
use crate::editor::doc::{Brush, BrushId, Entity, EntityId, GroupId, MapDocument, Material, WorldSettings};
use crate::editor::geom::brush_bounds;
use crate::editor::history::{self, History, Keep, Tx};
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
}

struct Session {
    /// Changes whenever another document is opened, so the page knows to start over.
    id: u64,
    doc: MapDocument,
    history: History,
    name: String,
    /// Where Save writes (a project file). None until saved or opened from a project.
    project: Option<PathBuf>,
    /// The revision each brush last changed at, and brushes removed since.
    stamps: HashMap<BrushId, u64>,
    removed: Vec<(BrushId, u64)>,
    /// Asks older than this get the whole map (removed notes were dropped).
    horizon: u64,
    prev: BTreeMap<BrushId, Brush>,
    drag: Option<Drag>,
    wads: Option<(Vec<String>, Vec<PathBuf>, Arc<wad::WadSet>)>,
}

impl Session {
    fn new(id: u64, doc: MapDocument, name: String, project: Option<PathBuf>) -> Session {
        let rev = doc.revision;
        let stamps = doc.brushes.keys().map(|b| (*b, rev)).collect();
        let prev = doc.brushes.clone();
        Session { id, doc, history: History::default(), name, project, stamps, removed: Vec::new(), horizon: 0, prev, drag: None, wads: None }
    }

    /// Notes which brushes changed since the last look (cheap: brushes compare by value).
    fn track(&mut self) {
        let rev = self.doc.revision;
        for (id, b) in &self.doc.brushes {
            if self.prev.get(id) != Some(b) {
                self.stamps.insert(*id, rev);
            }
        }
        for id in self.prev.keys() {
            if !self.doc.brushes.contains_key(id) {
                self.stamps.remove(id);
                self.removed.push((*id, rev));
            }
        }
        if self.removed.len() > MAX_REMOVED {
            let cut = self.removed.len() - MAX_REMOVED / 2;
            self.horizon = self.removed[cut - 1].1;
            self.removed.drain(..cut);
        }
        self.prev = self.doc.brushes.clone();
    }

    /// The document's WADs, opened (kept until the WAD list or the game changes).
    fn wads(&mut self, dirs: &[PathBuf]) -> Option<Arc<wad::WadSet>> {
        if dirs.is_empty() {
            return None;
        }
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
    next_id: AtomicU64,
    /// Texture pictures and lists, by WAD stamp.
    pngs: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    lists: Mutex<HashMap<String, Arc<Value>>>,
}

impl Editor {
    fn new() -> Editor {
        Editor { session: Mutex::new(None), next_id: AtomicU64::new(1), pngs: Mutex::default(), lists: Mutex::default() }
    }
    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn install(&self, doc: MapDocument, name: String, project: Option<PathBuf>, dirs: &[PathBuf]) -> Value {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut s = Session::new(id, doc, name, project);
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
    ED.get_or_init(|| Arc::new(Editor::new())).clone()
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

fn doc_json(s: &Session) -> Value {
    let d = &s.doc;
    let mut owned: HashMap<EntityId, Vec<u64>> = HashMap::new();
    let mut bounds = Bounds::empty();
    let brushes: Vec<Value> = d
        .brushes
        .iter()
        .map(|(id, b)| {
            if let Some(e) = b.entity {
                owned.entry(e).or_default().push(id.0);
            }
            let bb = brush_bounds(b);
            if !bb.is_empty() {
                bounds = bounds.union(&bb);
            }
            json!({"id": id.0, "entity": b.entity.map(|e| e.0), "group": b.group.map(|g| g.0), "min": vec_json(bb.min), "max": vec_json(bb.max)})
        })
        .collect();
    let entities: Vec<Value> = d
        .entities
        .iter()
        .map(|(id, e)| {
            if let Some(o) = e.origin() {
                bounds.add(o);
            }
            json!({"id": id.0, "classname": e.classname(), "keys": e.keys, "group": e.group.map(|g| g.0), "brushes": owned.get(id).cloned().unwrap_or_default()})
        })
        .collect();
    let groups: Vec<Value> = d.groups.iter().map(|(id, g)| json!({"id": id.0, "name": g.name, "parent": g.parent.map(|p| p.0), "hidden": g.hidden, "locked": g.locked})).collect();
    if bounds.is_empty() {
        bounds = Bounds { min: v3(-512.0, -512.0, 0.0), max: v3(512.0, 512.0, 256.0) };
    }
    let mut v = s.state();
    v["world"] = json!({"wads": d.world.wads, "keys": d.world.keys});
    v["brushes"] = Value::Array(brushes);
    v["entities"] = Value::Array(entities);
    v["groups"] = Value::Array(groups);
    v["bounds"] = json!([vec_json(bounds.min), vec_json(bounds.max)]);
    v["stats"] = serde_json::to_value(checks::stats(d)).unwrap_or(Value::Null);
    v
}

/// The brush meshes changed since `since` (all of them when `since` is missing, too old, or for
/// another document), as an LSMB bundle: header `brushes[{id, entity, faces[{face, texture,
/// normal, start, count}]}]` (start/count in vertices: one polygon per face, drawn as a fan)
/// and `removed[]`; sections `pos` (f32 x3, GoldSrc units) and `uv` (f32 x2, divided by the
/// texture size).
fn meshes_bundle(s: &Session, doc_id: Option<u64>, since: Option<u64>) -> Vec<u8> {
    let full = since.is_none();
    let since = since.unwrap_or(0);
    let full = full || doc_id != Some(s.id) || since < s.horizon || since > s.doc.revision;
    let mut pos: Vec<f32> = Vec::new();
    let mut uv: Vec<f32> = Vec::new();
    let mut list = Vec::new();
    for (id, b) in &s.doc.brushes {
        if !full && s.stamps.get(id).is_none_or(|r| *r <= since) {
            continue;
        }
        let faces: Vec<Value> = ops::brush_mesh(b, |t| s.doc.texture_size(t))
            .into_iter()
            .map(|f| {
                let start = pos.len() / 3;
                for (p, (u, v)) in f.vertices.iter().zip(&f.uvs) {
                    pos.extend([p.x as f32, p.y as f32, p.z as f32]);
                    uv.extend([*u as f32, *v as f32]);
                }
                json!({"face": f.face, "texture": f.texture, "normal": vec_json(f.normal), "start": start, "count": f.vertices.len()})
            })
            .collect();
        list.push(json!({"id": id.0, "entity": b.entity.map(|e| e.0), "group": b.group.map(|g| g.0), "faces": faces}));
    }
    let removed: Vec<u64> = if full { Vec::new() } else { s.removed.iter().filter(|(_, r)| *r > since).map(|(id, _)| id.0).collect() };
    let header = json!({"format": "LSMB", "kind": "editor", "docId": s.id, "revision": s.doc.revision, "full": full, "removed": removed, "brushes": list});
    goldsrc::mesh::write_bundle(header, &[Section::f32("pos", 3, &pos), Section::f32("uv", 2, &uv)])
}

fn checks_json(s: &mut Session, dirs: &[PathBuf]) -> Value {
    let known: Option<HashSet<String>> = s.wads(dirs).map(|set| {
        let mut k = HashSet::new();
        for w in set.wads.iter().filter_map(|(_, w)| w.as_ref().ok()) {
            k.extend(w.entries.iter().filter(|(n, _)| w.has_texture(n)).map(|(n, _)| n.clone()));
        }
        k
    });
    let issues = checks::run(&s.doc, known.as_ref());
    json!({"docId": s.id, "revision": s.doc.revision, "issues": issues, "stats": checks::stats(&s.doc), "texturesChecked": known.is_some()})
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
    },
    SetKey { entity: u64, key: String, value: String },
    AddEntity { classname: String, origin: P3, #[serde(default)] keys: Vec<(String, String)> },
    Tie { brushes: Vec<u64>, classname: String },
    ToWorld { entity: u64 },
    SetWorld { wads: Option<Vec<String>>, keys: Option<Vec<(String, String)>> },
    Group { name: String, #[serde(default)] brushes: Vec<u64>, #[serde(default)] entities: Vec<u64> },
    Ungroup { group: u64 },
    GroupFlags { group: u64, hidden: Option<bool>, locked: Option<bool> },
}

fn yes() -> bool {
    true
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
        Cmd::MoveFace { drag: Some(k), .. } => Some(k.clone()),
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
        Cmd::Transform { brushes, entities, translate, rotate, scale, pivot, lock } => {
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
            history::cmd_transform(d, h, &bs, &es, &t, lock, label, None)?;
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
                        s.drag = Some(Drag { key: k.clone(), brushes: [(id, b.clone())].into_iter().collect() });
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
        Cmd::Align { faces, action, u, v, degrees, drag } => {
            if faces.is_empty() {
                return Err("Pick a face first.".into());
            }
            let f: Vec<(BrushId, usize)> = faces.iter().map(|(b, i)| (BrushId(*b), *i)).collect();
            let (u, v) = (num(u, -65536.0, 65536.0, "value")?, num(v, -65536.0, 65536.0, "value")?);
            let deg = num(degrees, -3600.0, 3600.0, "angle")?;
            let anchor = |poly: &[Vec3]| crate::editor::geom::polygon_center(poly);
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
                    history::cmd_align(d, h, &f, drag, |a, p, (w, ht), _| texture::fit(a, p, w, ht, ru, rv))?
                }
                "center" => history::cmd_align(d, h, &f, drag, |a, p, (w, ht), _| texture::center(a, p, w, ht))?,
                "world" => history::cmd_align(d, h, &f, drag, |a, _, _, n| texture::align_to_world(a, n))?,
                "face" => history::cmd_align(d, h, &f, drag, |a, _, _, n| texture::align_to_face(a, n))?,
                _ => return Err(NOT_UNDERSTOOD.into()),
            }
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
    }
    Ok(json!({"created": {"brushes": created.0, "entities": created.1}, "note": note}))
}

/// A command against the open document, with what the page needs to catch up.
fn apply(s: &mut Session, cmd: Cmd, dirs: &[PathBuf]) -> Value {
    let before = (s.doc.revision, s.doc.entities.clone(), s.doc.groups.clone(), s.doc.world.clone());
    match run_cmd(s, cmd) {
        Ok(mut v) => {
            s.learn_textures(dirs);
            s.track();
            let doc_changed = s.doc.entities != before.1 || s.doc.groups != before.2 || s.doc.world != before.3;
            v["changed"] = json!(s.doc.revision != before.0);
            v["docChanged"] = json!(doc_changed);
            v["state"] = s.state();
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
    // old .map files are often Windows-1252: keep the bytes as Latin-1 rather than refuse
    Ok(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => e.into_bytes().iter().map(|b| *b as char).collect(),
    })
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "Untitled".into())
}

/// A project file (`.lsmap`, the editor's own JSON) or a `.map`, by its extension.
pub fn load_file(path: &Path) -> Result<(MapDocument, bool), String> {
    let text = read_text(path)?;
    let is_map = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("map"));
    let doc = if is_map { mapfile::read_map(&text)? } else { MapDocument::from_json(&text)? };
    Ok((doc, !is_map))
}

/// Writes next to the target and renames over it, so a failed write never leaves half a file.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("lsmap-tmp");
    std::fs::write(&tmp, text).map_err(|_| "Couldn't write the file there. Pick another folder.".to_string())?;
    std::fs::rename(&tmp, path).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        "Couldn't save over that file. Pick another name.".to_string()
    })
}

fn save_project(s: &mut Session, path: &Path) -> Result<(), String> {
    write_atomic(path, &s.doc.to_json())?;
    s.project = Some(path.to_path_buf());
    s.name = stem(path);
    s.history.mark_saved();
    Ok(())
}

fn export_map(s: &Session, path: &Path) -> Result<usize, String> {
    let text = mapfile::write_map(&s.doc, &mapfile::WriteOptions::default())?;
    write_atomic(path, &text)?;
    Ok(text.len())
}

enum Pick {
    Open,
    Import,
    SaveProject,
    ExportMap,
}

/// The system file dialog; None when cancelled.
fn dialog(st: &Shared, kind: Pick, name: &str) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt;
    let app = st.app.get().cloned().ok_or(INTERNAL)?;
    let d = app.dialog().file();
    let picked = match kind {
        Pick::Open => d.set_title("Open a map project").add_filter("Loner-Strike map project", &[PROJECT_EXT]).blocking_pick_file(),
        Pick::Import => d.set_title("Import a .map file").add_filter("GoldSrc map source", &["map"]).blocking_pick_file(),
        Pick::SaveProject => d.set_title("Save the map project").set_file_name(format!("{name}.{PROJECT_EXT}")).add_filter("Loner-Strike map project", &[PROJECT_EXT]).blocking_save_file(),
        Pick::ExportMap => d.set_title("Export a .map file").set_file_name(format!("{name}.map")).add_filter("GoldSrc map source", &["map"]).blocking_save_file(),
    };
    match picked {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|_| "That file can't be used.".to_string()),
    }
}

// ---- routes ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Since {
    doc: Option<u64>,
    since: Option<u64>,
}

async fn api_doc(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        Ok(doc_json(s))
    })
    .await
}

async fn api_meshes(Extension(ed): Extension<Ed>, Query(q): Query<Since>) -> Response {
    let r = tokio::task::spawn_blocking(move || {
        let g = ed.lock();
        g.as_ref().map(|s| meshes_bundle(s, q.doc, q.since))
    })
    .await;
    match r {
        Ok(Some(b)) => ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")], b).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": NO_DOC}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": INTERNAL}))).into_response(),
    }
}

async fn api_checks(State(st): State<Shared>, Extension(ed): Extension<Ed>) -> Response {
    let dirs = wad_dirs(&st);
    blocking(move || {
        let mut g = ed.lock();
        let s = g.as_mut().ok_or(NO_DOC)?;
        Ok(checks_json(s, &dirs))
    })
    .await
}

async fn api_cmd(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let cmd: Cmd = match serde_json::from_value(body) {
        Ok(c) => c,
        Err(_) => return bad(NOT_UNDERSTOOD),
    };
    let dirs = wad_dirs(&st);
    blocking(move || {
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
        let label = if redo { s.history.redo(&mut s.doc) } else { s.history.undo(&mut s.doc) };
        s.track();
        Ok(json!({"label": label, "changed": label.is_some(), "docChanged": label.is_some(), "state": s.state()}))
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
        Ok(ed.install(doc, stem(&path), project.then_some(path), &dirs))
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
        save_project(s, &path)?;
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
        let bytes = export_map(s, &path)?;
        Ok(json!({"exported": true, "file": path.file_name().map(|n| n.to_string_lossy().to_string()), "bytes": bytes}))
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

fn routes_with(ed: Ed) -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/doc", get(api_doc))
        .route("/api/tools/editor/meshes", get(api_meshes))
        .route("/api/tools/editor/checks", get(api_checks))
        .route("/api/tools/editor/textures", get(api_textures))
        .route("/api/tools/editor/wads", get(api_wads))
        .route("/api/tools/editor/tex/:file", get(api_tex))
        .route("/api/tools/editor/cmd", post(api_cmd))
        .route("/api/tools/editor/undo", post(api_undo))
        .route("/api/tools/editor/redo", post(api_redo))
        .route("/api/tools/editor/new", post(api_new))
        .route("/api/tools/editor/open", post(api_open))
        .route("/api/tools/editor/import-map", post(api_import))
        .route("/api/tools/editor/save", post(api_save))
        .route("/api/tools/editor/export-map", post(api_export))
        .layer(Extension(ed))
}

/// The editor's routes (merged into the tools' local routes).
pub fn routes() -> Router<Shared> {
    routes_with(global())
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

    struct T {
        st: Shared,
        ed: Ed,
    }

    impl T {
        fn new() -> T {
            T { st: state(), ed: Arc::new(Editor::new()) }
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
        let ed = Editor::new();
        ed.install(starter_map(), "Untitled".into(), None, &[]);
        let mut g = ed.lock();
        let s = g.as_mut().unwrap();
        run_cmd(s, serde_json::from_value(json!({"op": "add_box", "min": [0, 0, 0], "max": [32, 32, 32], "texture": "CRATE01"})).unwrap()).unwrap();
        assert!(s.history.is_dirty());

        let proj = dir.join("room.lsmap");
        save_project(s, &proj).unwrap();
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
}
