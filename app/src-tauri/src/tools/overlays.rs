//! The overlays the map viewer lays over a map, and the Replay tool's recordings:
//!
//! - `GET /api/tools/map/:name/nav`    the bot nav: areas, connections, hiding spots, places,
//!   and the ladders the game builds from the map's `func_ladder` brushes (JSON)
//! - `GET /api/tools/map/:name/know`   the brain's map knowledge (.lsv) as an LSMB bundle
//! - `GET /api/tools/map/:name/probe?x&y&z&dir=down|up`  the floor (or ceiling) a standing
//!   player meets from a point (JSON)
//! - `GET /api/tools/recordings`, `POST /api/tools/recordings/open` (file dialog)
//! - `GET /api/tools/recording/:id`    a recording as an LSMB bundle (per-player columns,
//!   heat grids; the rest in the header)
//! - `GET /api/tools/recording/:id/trace`  the brain's trace for it (JSON)
//!
//! Recordings are named by an id the server hands out (a hash of the path); the page never
//! names a path.

#[path = "replay.rs"]
pub mod replay;

use super::{blocking, cache, find_map, json_answer, locate, mapinfo::MapFile, Done, Fail};
use crate::goldsrc::{self, hull::MapHulls, lsv, mesh, nav};
use crate::server::Shared;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

const NO_NAV: &str = "This map has no bot nav file yet. Play it with bots once and the game makes one.";
const NO_LSV: &str = "The bots haven't learned this map yet. Play it with bots and they work it out as they go.";
const NO_RECORDING: &str = "That recording isn't open any more. Pick it again from the list.";

// ---- small helpers ----------------------------------------------------------------------

fn bundle_answer(r: Done<Vec<u8>>) -> Response {
    match r {
        Ok(b) => ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-cache")], b).into_response(),
        Err(f) => (if f.status == StatusCode::OK { StatusCode::UNPROCESSABLE_ENTITY } else { f.status }, Json(json!({"error": f.msg}))).into_response(),
    }
}

fn read(path: &std::path::Path, what: &'static str, max: u64) -> Done<Vec<u8>> {
    Ok(goldsrc::read_capped(path, what, max)?)
}

/// The map's collision trees, kept for the last two map versions (probing asks often).
fn hulls(m: &MapFile) -> Done<Arc<MapHulls>> {
    static MEMO: OnceLock<Mutex<Vec<(PathBuf, cache::Stamp, Arc<MapHulls>)>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let stamp = cache::Stamp::of(&m.bsp).map_err(|_| Fail::no_map())?;
    if let Some(h) = memo.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.0 == m.bsp && e.1 == stamp) {
        return Ok(h.2.clone());
    }
    let h = Arc::new(MapHulls::parse(&read(&m.bsp, "map file", goldsrc::MAX_BSP)?)?);
    let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
    list.push((m.bsp.clone(), stamp, h.clone()));
    if list.len() > 2 {
        list.remove(0);
    }
    Ok(h)
}

fn load_nav(m: &MapFile) -> Done<(nav::Nav, PathBuf)> {
    let p = m.sibling("nav").ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, NO_NAV))?;
    let n = nav::Nav::parse(&read(&p, "navigation file", nav::MAX_NAV)?)?;
    Ok((n, p))
}

// ---- nav --------------------------------------------------------------------------------

/// The nav as the page draws it (TOOLS-PLAN.md 3.8), plus ladders and link checks.
pub fn nav_json(n: &nav::Nav, bsp_size: Option<u64>, ladders: &[nav::Ladder]) -> Value {
    let id_of = |i: Option<usize>| i.map(|i| n.areas[i].id);
    let areas: Vec<Value> = n
        .areas
        .iter()
        .map(|a| {
            json!({
                "id": a.id, "attr": a.attr, "nw": a.lo, "se": a.hi, "neZ": a.ne_z, "swZ": a.sw_z,
                "conns": a.conns, "place": a.place,
                "hiding": a.hiding.iter().map(|h| json!({"id": h.id, "pos": h.pos, "flags": h.flags})).collect::<Vec<_>>(),
                "approach": a.approach.len(), "encounters": a.encounters.len(),
            })
        })
        .collect();
    let ladders: Vec<Value> = ladders
        .iter()
        .map(|l| {
            json!({
                "entity": l.entity, "top": l.top, "bottom": l.bottom, "length": l.length, "dir": l.dir,
                "bottomArea": id_of(l.bottom_area), "topForward": id_of(l.top_forward), "topLeft": id_of(l.top_left),
                "topRight": id_of(l.top_right), "topBehind": id_of(l.top_behind), "dangling": l.dangling,
            })
        })
        .collect();
    json!({
        "version": n.version, "bspSize": n.bsp_size,
        "matchesMap": bsp_size.map(|s| n.version < 4 || s == n.bsp_size as u64),
        "places": n.places, "areas": areas, "ladders": ladders,
        "oneWay": n.one_way_links(), "danglingLinks": n.dangling_links(),
        "attrBits": {"crouch": nav::ATTR_CROUCH, "jump": nav::ATTR_JUMP, "precise": nav::ATTR_PRECISE, "noJump": nav::ATTR_NO_JUMP},
        "spotFlags": {"inCover": nav::SPOT_IN_COVER, "goodSniper": nav::SPOT_GOOD_SNIPER, "idealSniper": nav::SPOT_IDEAL_SNIPER},
        "dirs": ["north (-y)", "east (+x)", "south (+y)", "west (-x)"],
    })
}

fn nav_for(m: &MapFile) -> Done<Value> {
    let (n, _) = load_nav(m)?;
    let size = std::fs::metadata(&m.bsp).ok().map(|x| x.len());
    // (ladders need the map's trees; a map we can't trace still shows its nav)
    let (ladders, warning) = match hulls(m) {
        Ok(h) => (nav::build_ladders(&n, &h.ladders, h.as_ref()), None),
        Err(f) => (Vec::new(), Some(format!("Ladders aren't shown: {}", f.msg))),
    };
    let mut v = nav_json(&n, size, &ladders);
    v["warning"] = json!(warning);
    Ok(v)
}

async fn api_nav(State(st): State<Shared>, Path(name): Path<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || nav_for(&locate(&game, &name)?)).await,
    };
    json_answer(r)
}

// ---- knowledge --------------------------------------------------------------------------

/// The knowledge as an LSMB bundle: facts in the header, everything per area as sections
/// (count = areas; signed and scaled values as f32 in the units the page uses).
pub fn know_bundle(k: &lsv::Lsv, nav: Option<&nav::Nav>) -> Vec<u8> {
    use mesh::Section;
    let n = k.areas.len();
    let nav_key = nav.map(lsv::nav_key);
    let key_matches = nav.map(|nv| nv.areas.len() == n && nav_key == Some(k.head.key));
    let mut cover_low = Vec::with_capacity(n * 16);
    let mut cover_high = Vec::with_capacity(n * 16);
    let mut cover_mat = Vec::with_capacity(n * 16);
    let (mut ceiling, mut sees, mut high) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut nc, mut na, mut no) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    let mut corners = vec![0f32; n * lsv::CORNERS * 4];
    let mut angles = vec![0f32; n * lsv::ANGLES * 5];
    let mut objects = vec![0f32; n * lsv::OBJECTS * 6];
    let mut exp_stand = vec![0u16; n * lsv::ANGLES];
    let mut exp_crouch = vec![0u16; n * lsv::ANGLES];
    let mut pc_low = Vec::with_capacity(n * 9);
    let mut pc_high = Vec::with_capacity(n * 9);
    let deg = |b: u8| b as f32 * 360.0 / 256.0;
    for (i, a) in k.areas.iter().enumerate() {
        cover_low.extend(a.cover_low.iter().map(|&v| v as u16 * 8));
        cover_high.extend(a.cover_high.iter().map(|&v| v as u16 * 8));
        cover_mat.extend(a.mat);
        ceiling.push(a.ceiling as u16 * 4);
        sees.push(a.sees);
        high.push(a.high);
        nc.push(a.corners.len() as u8);
        na.push(a.angles.len() as u8);
        no.push(a.objects.len() as u8);
        for (j, c) in a.corners.iter().enumerate() {
            let at = (i * lsv::CORNERS + j) * 4;
            corners[at..at + 4].copy_from_slice(&[c.x as f32, c.y as f32, c.z as f32, deg(c.yaw)]);
        }
        for (j, g) in a.angles.iter().enumerate() {
            let at = (i * lsv::ANGLES + j) * 5;
            angles[at..at + 5].copy_from_slice(&[deg(g.yaw), g.pitch as f32, g.dist as f32, g.weight as f32, g.area as f32]);
            exp_stand[i * lsv::ANGLES + j] = a.exposed_stand.get(j).copied().unwrap_or(0);
            exp_crouch[i * lsv::ANGLES + j] = a.exposed_crouch.get(j).copied().unwrap_or(0);
        }
        for (j, o) in a.objects.iter().enumerate() {
            let at = (i * lsv::OBJECTS + j) * 6;
            objects[at..at + 6].copy_from_slice(&[o.kind as f32, o.x as f32, o.y as f32, o.z as f32, o.top as f32, o.mat as f32]);
        }
        pc_low.extend(a.point_cover_low);
        pc_high.extend(a.point_cover_high);
    }
    let sections = vec![
        Section::u8("sight", 1, &k.sight),
        Section::u16("pairA", 1, &k.pairs.iter().map(|p| p.a).collect::<Vec<_>>()),
        Section::u16("pairB", 1, &k.pairs.iter().map(|p| p.b).collect::<Vec<_>>()),
        Section::u8("pairQuality", 1, &k.pairs.iter().map(|p| p.quality).collect::<Vec<_>>()),
        Section::u8("pairFlags", 1, &k.pairs.iter().map(|p| p.flags).collect::<Vec<_>>()),
        Section::u16("bangA", 1, &k.bangs.iter().map(|p| p.a).collect::<Vec<_>>()),
        Section::u16("bangB", 1, &k.bangs.iter().map(|p| p.b).collect::<Vec<_>>()),
        Section::u8("bangThick", 1, &k.bangs.iter().map(|p| p.thick).collect::<Vec<_>>()),
        Section::u8("bangMat", 1, &k.bangs.iter().map(|p| p.mat).collect::<Vec<_>>()),
        Section::u16("coverLow", 16, &cover_low),
        Section::u16("coverHigh", 16, &cover_high),
        Section::u8("coverMat", 16, &cover_mat),
        Section::u16("ceiling", 1, &ceiling),
        Section::u16("sees", 1, &sees),
        Section::u8("high", 1, &high),
        Section::u8("cornerCount", 1, &nc),
        Section::f32("corners", lsv::CORNERS * 4, &corners),
        Section::u8("angleCount", 1, &na),
        Section::f32("angles", lsv::ANGLES * 5, &angles),
        Section::u8("objectCount", 1, &no),
        Section::f32("objects", lsv::OBJECTS * 6, &objects),
        Section::u16("exposedStand", lsv::ANGLES, &exp_stand),
        Section::u16("exposedCrouch", lsv::ANGLES, &exp_crouch),
        Section::u16("pointCoverLow", 9, &pc_low),
        Section::u16("pointCoverHigh", 9, &pc_high),
    ];
    let materials: serde_json::Map<String, Value> = lsv::MATERIALS.iter().map(|(c, name)| ((*c as u32).to_string(), json!(name))).collect();
    let warning = match (nav, key_matches) {
        (None, _) => Some("This map has no nav file, so the knowledge can't be placed on it."),
        (Some(_), Some(false)) => Some("Knowledge was built for a different nav. Play the map again with bots to rebuild it."),
        _ => None,
    };
    let header = json!({
        "format": "LSMB", "kind": "know", "version": k.head.version,
        "stage": k.head.stage, "stageName": k.stage_name(), "done": k.head.stage as usize == lsv::STAGES.len() - 1,
        "count": n, "navCount": nav.map(|nv| nv.areas.len()),
        "key": k.head.key, "navKey": nav_key, "keyMatches": key_matches, "warning": warning,
        "traces": k.head.traces, "spentSeconds": k.head.spent_ms / 1000.0,
        "pairs": k.pairs.len(), "bangs": k.bangs.len(),
        "materials": materials, "objectTypes": lsv::OBJECT_TYPES, "stages": lsv::STAGES,
        "units": {
            "coverLow/coverHigh": "open distance in units, 16 directions from east counter-clockwise",
            "corners": "x, y, z, yaw degrees", "angles": "yaw degrees, pitch degrees (+ up), distance, weight, area index",
            "objects": "type, x, y, z, top, material code", "sight": "count x count bits, row-major, bit i*count+j: i sees j",
            "exposed*": "per angle: bits 0-8 = the area's 3x3 points seen", "pointCover*": "per point: bit d = solid within 64 units that way",
        },
    });
    mesh::write_bundle(header, &sections)
}

fn know_for(m: &MapFile) -> Done<Vec<u8>> {
    let p = m.sibling("lsv").ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, NO_LSV))?;
    let k = lsv::Lsv::parse(&read(&p, "map knowledge file", lsv::MAX_LSV)?)?;
    let nav = load_nav(m).ok().map(|(n, _)| n);
    Ok(know_bundle(&k, nav.as_ref()))
}

async fn api_know(State(st): State<Shared>, Path(name): Path<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || know_for(&locate(&game, &name)?)).await,
    };
    bundle_answer(r)
}

// ---- probe ------------------------------------------------------------------------------

#[derive(Deserialize)]
struct ProbeQuery {
    x: f32,
    y: f32,
    z: f32,
    dir: Option<String>,
}

pub fn probe(h: &MapHulls, x: f32, y: f32, z: f32, up: bool) -> Value {
    let p = [x, y, z];
    let contents = h.contents(p);
    let found = if up { h.ceiling_above(p, 8192.0) } else { h.floor_below(p, 8192.0) };
    match found {
        Some((at, t)) => json!({
            "hit": true, "dir": if up { "up" } else { "down" }, "z": at, "normal": t.normal, "fraction": t.fraction,
            "contents": goldsrc::hull::contents_name(contents),
        }),
        None => {
            let start = h.trace(1, [x, y, z + goldsrc::hull::PLAYER_HALF_HEIGHT], [x, y, z + goldsrc::hull::PLAYER_HALF_HEIGHT]);
            json!({"hit": false, "dir": if up { "up" } else { "down" }, "startSolid": start.start_solid,
                   "contents": goldsrc::hull::contents_name(contents)})
        }
    }
}

async fn api_probe(State(st): State<Shared>, Path(name): Path<String>, q: Option<Query<ProbeQuery>>) -> Response {
    let Some(Query(q)) = q else { return json_answer(Err(Fail::new(StatusCode::BAD_REQUEST, "Give a point: x, y and z."))) };
    let ok = |v: f32| v.is_finite() && v.abs() <= 65536.0;
    if !(ok(q.x) && ok(q.y) && ok(q.z)) {
        return json_answer(Err(Fail::new(StatusCode::BAD_REQUEST, "That point is outside any map.")));
    }
    let up = q.dir.as_deref() == Some("up");
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || Ok(probe(hulls(&locate(&game, &name)?)?.as_ref(), q.x, q.y, q.z, up))).await,
    };
    json_answer(r)
}

// ---- recordings -------------------------------------------------------------------------

/// Recordings the page can ask for by id: id -> path.
fn registry() -> &'static Mutex<HashMap<String, PathBuf>> {
    static R: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn id_for(p: &std::path::Path) -> String {
    cache::short_hash(&[&p.to_string_lossy()])
}

fn register(p: PathBuf) -> String {
    let id = id_for(&p);
    let mut r = registry().lock().unwrap_or_else(|e| e.into_inner());
    if r.len() > 2000 {
        r.clear();
    }
    r.insert(id.clone(), p);
    id
}

fn path_of(id: &str) -> Done<PathBuf> {
    if id.len() != 16 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(Fail::new(StatusCode::BAD_REQUEST, NO_RECORDING));
    }
    registry().lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned().filter(|p| p.is_file()).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, NO_RECORDING))
}

/// Where recordings are kept: the app's own folder and gm_simrec's in the game.
fn recording_dirs(game: Option<&crate::game::Game>) -> Vec<PathBuf> {
    let mut v = vec![crate::sys::app_dir().join("tools").join("recordings")];
    if let Some(g) = game {
        for d in [g.dir.clone(), g.hl.join("cstrike")] {
            let p = d.join("addons").join("amxmodx").join("data").join("simrec");
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

fn is_recording_name(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".csv") && !l.starts_with("trace_") && l != "lstrace.csv"
}

/// The brain's trace for a recording: `trace_<prefix>.csv` next to `<prefix>_<map>_<n>.csv`
/// (the simulation's results), else `lstrace.csv` beside it or in the game folder above
/// `addons/amxmodx/data/simrec/`.
pub fn trace_for(rec: &std::path::Path) -> Option<PathBuf> {
    let dir = rec.parent()?;
    let stem = rec.file_stem()?.to_string_lossy().to_string();
    let mut best: Option<(usize, PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(prefix) = name.strip_prefix("trace_").and_then(|n| n.strip_suffix(".csv")) else { continue };
        if !prefix.is_empty() && stem.starts_with(&format!("{prefix}_")) && best.as_ref().map(|b| prefix.len() > b.0).unwrap_or(true) {
            best = Some((prefix.len(), e.path()));
        }
    }
    if let Some((_, p)) = best {
        return Some(p);
    }
    let beside = dir.join("lstrace.csv");
    if beside.is_file() {
        return Some(beside);
    }
    // <game>/addons/amxmodx/data/simrec/<file>
    let game = dir.parent()?.parent()?.parent()?.parent()?;
    let p = game.join("lstrace.csv");
    p.is_file().then_some(p)
}

/// The map a recording is of, from its file name: gm_simrec names files `<map>_<time>.csv`;
/// the simulation prefixes them (`<server>_<mode>_<map>_<stamp>_<map>_<time>.csv`), so the
/// longest tail that's a map in the game wins, else what's after a 6-digit stamp.
pub fn map_of(stem: &str, exists: impl Fn(&str) -> bool) -> String {
    let base = match stem.rsplit_once('_') {
        Some((a, b)) if !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()) => a,
        _ => stem,
    };
    let mut tail = base;
    loop {
        if exists(tail) {
            return tail.to_string();
        }
        match tail.split_once('_') {
            Some((_, rest)) if !rest.is_empty() => tail = rest,
            _ => break,
        }
    }
    let parts: Vec<&str> = base.split('_').collect();
    if let Some(i) = parts.iter().rposition(|p| p.len() == 6 && p.bytes().all(|c| c.is_ascii_digit())) {
        if i + 1 < parts.len() {
            return parts[i + 1..].join("_");
        }
    }
    base.to_string()
}

fn scan(p: &std::path::Path) -> Result<replay::Scan, String> {
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, (cache::Stamp, replay::Scan)>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let stamp = cache::Stamp::of(p).map_err(|e| e.to_string())?;
    if let Some((s, v)) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(p) {
        if *s == stamp {
            return Ok(v.clone());
        }
    }
    if stamp.size > replay::MAX_FILE {
        return Err(replay::too_big(stamp.size));
    }
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let v = replay::scan_recording(std::io::BufReader::new(f))?;
    memo.lock().unwrap_or_else(|e| e.into_inner()).insert(p.to_path_buf(), (stamp, v.clone()));
    Ok(v)
}

fn recording_meta(p: &std::path::Path, game: Option<&crate::game::Game>) -> Value {
    let id = register(p.to_path_buf());
    let file = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let stem = p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let map = map_of(&stem, |m| crate::maps::safe_arg(m) && game.map(|g| super::mapinfo::locate(g, m).is_some()).unwrap_or(false));
    let st = cache::Stamp::of(p).ok();
    let mut v = json!({"id": id, "file": file, "map": map, "when": st.map(|s| s.mtime), "size": st.map(|s| s.size), "hasTrace": trace_for(p).is_some()});
    match scan(p) {
        Ok(s) => {
            v["seconds"] = json!(match (s.start, s.end) {
                (Some(a), Some(b)) => b - a,
                _ => 0.0,
            });
            v["bots"] = json!(s.bots);
            v["players"] = json!(s.players);
            v["kills"] = json!(s.kills);
            v["rounds"] = json!(s.rounds);
        }
        Err(e) => v["error"] = json!(e),
    }
    v
}

fn list_recordings(game: Option<Arc<crate::game::Game>>) -> Value {
    let mut paths: Vec<PathBuf> = Vec::new();
    for d in recording_dirs(game.as_deref()) {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if is_recording_name(&name) && e.metadata().map(|m| m.is_file() && m.len() > 0).unwrap_or(false) {
                paths.push(e.path());
            }
        }
    }
    // and the ones opened by hand this session
    for p in registry().lock().unwrap_or_else(|e| e.into_inner()).values() {
        if p.is_file() && !paths.contains(p) {
            paths.push(p.clone());
        }
    }
    let mut list: Vec<Value> = paths.iter().map(|p| recording_meta(p, game.as_deref())).collect();
    list.sort_by(|a, b| b["when"].as_u64().cmp(&a["when"].as_u64()));
    json!({"recordings": list, "folders": recording_dirs(game.as_deref()).iter().map(|d| d.to_string_lossy().to_string()).collect::<Vec<_>>()})
}

async fn api_recordings(State(st): State<Shared>) -> Response {
    let game = st.local_game();
    json_answer(blocking(move || Ok(list_recordings(game))).await)
}

async fn api_open(State(st): State<Shared>, headers: HeaderMap) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let Some(app) = st.app.get().cloned() else { return json_answer(Err(Fail::new(StatusCode::CONFLICT, "The file picker isn't available."))) };
    let game = st.local_game();
    json_answer(
        blocking(move || {
            let Some(picked) = app.dialog().file().set_title("Open a recording").add_filter("Recordings", &["csv"]).blocking_pick_file() else {
                return Ok(json!({"cancelled": true}));
            };
            let p = picked.into_path().map_err(|e| Fail::new(StatusCode::OK, e.to_string()))?;
            let meta = recording_meta(&p, game.as_deref());
            Ok(json!({"id": meta["id"], "recording": meta}))
        })
        .await,
    )
}

/// Parsed recordings, the last two (the page asks for a recording, then its trace).
fn parsed(p: &std::path::Path) -> Done<Arc<replay::Recording>> {
    static MEMO: OnceLock<Mutex<Vec<(PathBuf, cache::Stamp, Arc<replay::Recording>)>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let stamp = cache::Stamp::of(p).map_err(|_| Fail::new(StatusCode::NOT_FOUND, NO_RECORDING))?;
    if let Some(e) = memo.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.0 == p && e.1 == stamp) {
        return Ok(e.2.clone());
    }
    if stamp.size > replay::MAX_FILE {
        return Err(Fail::new(StatusCode::PAYLOAD_TOO_LARGE, replay::too_big(stamp.size)));
    }
    let f = std::fs::File::open(p).map_err(|e| Fail::new(StatusCode::NOT_FOUND, format!("Couldn't read the recording: {e}")))?;
    let r = Arc::new(replay::parse_recording(std::io::BufReader::new(f)).map_err(|e| Fail::new(StatusCode::UNPROCESSABLE_ENTITY, e))?);
    let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
    list.push((p.to_path_buf(), stamp, r.clone()));
    if list.len() > 2 {
        list.remove(0);
    }
    Ok(r)
}

/// A recording as an LSMB bundle: the columns (all players one after another; each player's
/// `start`/`count` in the header) and the heat grids (`heat.w` x `heat.h`, row-major).
pub fn recording_bundle(r: &replay::Recording, mut header: Value) -> Vec<u8> {
    use mesh::Section;
    let c = &r.cols;
    let sections = vec![
        Section::f32("t", 1, &c.t),
        Section::f32("x", 1, &c.x),
        Section::f32("y", 1, &c.y),
        Section::f32("z", 1, &c.z),
        Section::f32("vx", 1, &c.vx),
        Section::f32("vy", 1, &c.vy),
        Section::f32("vz", 1, &c.vz),
        Section::f32("yaw", 1, &c.yaw),
        Section::f32("pitch", 1, &c.pitch),
        Section::u8("hp", 1, &c.hp),
        Section::u8("duck", 1, &c.duck),
        Section::u8("ground", 1, &c.ground),
        Section::u8("weapon", 1, &c.weapon),
        Section::u8("venemy", 1, &c.venemy),
        Section::u8("team", 1, &c.team),
        Section::u32("keys", 1, &c.keys),
        Section::f32("heatTime", 1, &r.heat.time),
        Section::u32("heatDeaths", 1, &r.heat.deaths),
        Section::f32("heatStuck", 1, &r.heat.stuck),
    ];
    if let (Some(h), Value::Object(s)) = (header.as_object_mut(), r.summary()) {
        h.extend(s);
        h.insert("format".into(), json!("LSMB"));
        h.insert("kind".into(), json!("replay"));
        h.insert("seconds".into(), json!(r.seconds()));
        h.insert("bots".into(), json!(r.bots()));
    }
    mesh::write_bundle(header, &sections)
}

async fn api_recording(State(st): State<Shared>, Path(id): Path<String>) -> Response {
    let game = st.local_game();
    let r = blocking(move || {
        let p = path_of(&id)?;
        let rec = parsed(&p)?;
        let meta = recording_meta(&p, game.as_deref());
        Ok(recording_bundle(&rec, json!({"id": id, "file": meta["file"], "map": meta["map"], "hasTrace": meta["hasTrace"]})))
    })
    .await;
    bundle_answer(r)
}

async fn api_trace(State(st): State<Shared>, Path(id): Path<String>) -> Response {
    let _ = st;
    let r = blocking(move || {
        let p = path_of(&id)?;
        let rec = parsed(&p)?;
        let tp = trace_for(&p).ok_or_else(|| Fail::new(StatusCode::OK, "There's no bot trace for this recording. Recordings made with gm_simrec come with one (lstrace.csv)."))?;
        let size = std::fs::metadata(&tp).map(|m| m.len()).unwrap_or(0);
        if size > replay::MAX_FILE {
            return Err(Fail::new(StatusCode::OK, replay::too_big(size).replace("recording", "trace")));
        }
        let f = std::fs::File::open(&tp).map_err(|e| Fail::new(StatusCode::OK, format!("Couldn't read the trace: {e}")))?;
        let mut v = replay::parse_trace(std::io::BufReader::new(f), &rec).map_err(|e| Fail::new(StatusCode::OK, e))?;
        v["file"] = json!(tp.file_name().map(|n| n.to_string_lossy().to_string()));
        Ok(v)
    })
    .await;
    json_answer(r)
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/map/:name/nav", get(api_nav))
        .route("/api/tools/map/:name/know", get(api_know))
        .route("/api/tools/map/:name/probe", get(api_probe))
        .route("/api/tools/recordings", get(api_recordings))
        .route("/api/tools/recordings/open", post(api_open))
        .route("/api/tools/recording/:id", get(api_recording))
        .route("/api/tools/recording/:id/trace", get(api_trace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_names_from_files() {
        let maps = ["cs_rats2", "de_dust2"];
        let has = |m: &str| maps.contains(&m);
        assert_eq!(map_of("cs_rats2_1794524249", has), "cs_rats2");
        assert_eq!(map_of("W1-1_classic_cs_rats2_150726_cs_rats2_1794524249", has), "cs_rats2");
        assert_eq!(map_of("W1-1_classic_de_nuke_150726_de_nuke_1794524249", |_| false), "de_nuke");
        assert_eq!(map_of("fy_pool_day_123", |_| false), "fy_pool_day");
    }

    #[test]
    fn traces_are_found() {
        let dir = std::env::temp_dir().join(format!("ls-overlays-test-{}", std::process::id()));
        let sim = dir.join("results");
        std::fs::create_dir_all(&sim).unwrap();
        let rec = sim.join("W1-1_classic_cs_rats2_150726_cs_rats2_1794524249.csv");
        std::fs::write(&rec, "R,1,start\n").unwrap();
        std::fs::write(sim.join("trace_W1-1_classic_cs_rats2_150726.csv"), "#map,cs_rats2\n").unwrap();
        std::fs::write(sim.join("trace_W2-2_classic_cs_rats2_150744.csv"), "").unwrap();
        assert_eq!(trace_for(&rec).unwrap().file_name().unwrap(), "trace_W1-1_classic_cs_rats2_150726.csv");
        // a game's: <game>/addons/amxmodx/data/simrec/<map>_<n>.csv and <game>/lstrace.csv
        let simrec = dir.join("cstrike/addons/amxmodx/data/simrec");
        std::fs::create_dir_all(&simrec).unwrap();
        let rec = simrec.join("de_dust2_1.csv");
        std::fs::write(&rec, "").unwrap();
        assert!(trace_for(&rec).is_none());
        std::fs::write(dir.join("cstrike/lstrace.csv"), "").unwrap();
        assert_eq!(trace_for(&rec).unwrap(), dir.join("cstrike/lstrace.csv"));
        assert!(is_recording_name("de_dust2_1.csv") && !is_recording_name("trace_x.csv") && !is_recording_name("LSTRACE.csv"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ids_are_checked() {
        assert_eq!(path_of("../../etc").unwrap_err().status, StatusCode::BAD_REQUEST);
        assert_eq!(path_of("0123456789abcdef").unwrap_err().status, StatusCode::NOT_FOUND);
        let f = std::env::temp_dir().join(format!("ls-overlays-id-{}.csv", std::process::id()));
        std::fs::write(&f, "x").unwrap();
        let id = register(f.clone());
        assert_eq!(path_of(&id).unwrap(), f);
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn bundles_decode() {
        let rec = replay::parse_recording(include_str!("testdata/rec_snippet.csv").as_bytes()).unwrap();
        let b = recording_bundle(&rec, json!({"id": "x"}));
        let (h, s) = mesh::read_bundle(&b);
        assert_eq!(h["kind"], "replay");
        assert_eq!(s["t"].len(), rec.cols.t.len() * 4);
        assert_eq!(s["heatTime"].len(), rec.heat.w * rec.heat.h * 4);
        assert!(h["players"].as_array().unwrap().iter().any(|p| p["id"] == 1));
        assert!(h["kills"].as_array().unwrap().iter().any(|k| k["fall"].is_object()));
    }

    #[test]
    fn nav_and_probe_on_the_test_room() {
        let h = MapHulls::parse(&crate::goldsrc::hull::tests::room_bsp()).unwrap();
        let v = probe(&h, 50.0, 50.0, 40.0, false);
        assert_eq!(v["hit"], true);
        assert!(v["z"].as_f64().unwrap().abs() < 0.1, "{v}");
        let v = probe(&h, 50.0, 50.0, -500.0, false);
        assert_eq!(v["hit"], false);
        assert_eq!(v["contents"], "solid");
        let n = nav::Nav { version: 5, bsp_size: 7, places: vec!["Mid".into()], areas: vec![] };
        let v = nav_json(&n, Some(8), &[]);
        assert_eq!(v["matchesMap"], false);
        assert_eq!(v["places"][0], "Mid");
    }

    #[tokio::test]
    async fn routes_refuse_bad_requests() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;
        let st: Shared = Arc::new(crate::server::AppState {
            current: std::sync::RwLock::new(None),
            hosts: std::sync::RwLock::new(vec![]),
            lan_hosts: std::sync::RwLock::new(vec![]),
            token: std::sync::RwLock::new(String::new()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let call = |method: &str, uri: &str| {
            let app = routes().with_state(st.clone());
            let req = Request::builder().method(method).uri(uri).body(Body::empty()).unwrap();
            async move {
                let res = app.oneshot(req).await.unwrap();
                let status = res.status();
                let body = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
                (status, String::from_utf8_lossy(&body).to_string())
            }
        };
        for uri in ["/api/tools/map/..%2Fx/nav", "/api/tools/map/a%20b/probe?x=1&y=2&z=3", "/api/tools/recording/zzzz/trace"] {
            let (s, b) = call("GET", uri).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "{uri} {b}");
            assert!(b.contains("error"), "{uri} {b}");
        }
        let (s, _) = call("GET", "/api/tools/map/de_dust2/probe?x=NaN&y=0&z=0").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, _) = call("GET", "/api/tools/map/de_dust2/probe").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // binary routes say what's wrong with a status the page can check
        let (s, b) = call("GET", "/api/tools/recording/0123456789abcdef").await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{b}");
        // no game set up: a plain message, not a crash
        let (_, b) = call("GET", "/api/tools/map/de_dust2/nav").await;
        assert!(b.contains("error"), "{b}");
        // the file picker only for the app's own page
        let (s, _) = call("POST", "/api/tools/recordings/open").await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }

    /// LS_TEST_SIM: the fixed cs_rats2 nav and knowledge make a nav answer and a bundle that
    /// says the key matches (and doesn't against the unfixed nav).
    #[test]
    fn real_know_bundle() {
        let Ok(sim) = std::env::var("LS_TEST_SIM") else { return };
        let sim = std::path::PathBuf::from(sim);
        let nav = nav::Nav::parse(&std::fs::read(sim.join("navpack/fixes/cs_rats2.nav")).unwrap()).unwrap();
        let k = lsv::Lsv::parse(&std::fs::read(sim.join("navpack/fixes/cs_rats2.lsv")).unwrap()).unwrap();
        let (h, s) = mesh::read_bundle(&know_bundle(&k, Some(&nav)));
        assert_eq!(h["keyMatches"], true);
        assert_eq!(h["warning"], Value::Null);
        assert_eq!(s["sight"].len(), (586 * 586 + 7) / 8);
        assert_eq!(s["angles"].len(), 586 * 8 * 5 * 4);
        let old = nav::Nav::parse(&std::fs::read(sim.join("navpack/out/cs_rats2.nav")).unwrap()).unwrap();
        let (h, _) = mesh::read_bundle(&know_bundle(&k, Some(&old)));
        assert_eq!(h["keyMatches"], false);
        let v = nav_json(&nav, None, &[]);
        assert_eq!(v["areas"].as_array().unwrap().len(), 586);
        assert!(!v["oneWay"].as_array().unwrap().is_empty());
    }
}
