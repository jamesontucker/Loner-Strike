//! Leak tracing (agent P5, Hammer parity): the leak line of the last compile that leaked (or a
//! pointfile the mapper loads from another compile), where its hole is, and the brushes
//! around the hole, for the page's "Go to leak" and for agents (MCP editor_leak).
//! - The compile (compile.rs) hands its `.lin`/`.pts` line here when the map leaks and clears it
//!   when a compile finishes without one.
//! - The hole: SDHLT's line runs from the entity that "sees" the void, portal by portal, out to
//!   the outside. The gap is where it leaves the world's brushes for good: the last point still
//!   inside the box round all world brushes. The brushes nearest that point are where the gap
//!   is (brush entities don't seal a map, so they are named apart).
//!
//!   GET  /api/tools/editor/leak          the line, the hole and the brushes round it
//!   POST /api/tools/editor/leak/load     a .lin or .pts picked in the file dialog
//!   POST /api/tools/editor/leak/clear

use super::{blocking, dialog, forbidden, Ed, Pick, NO_DOC};
use crate::editor::doc::MapDocument;
use crate::editor::geom::brush_bounds;
use crate::server::Shared;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Router};
use serde_json::{json, Value};
use std::sync::Mutex;

/// The leak line shown now.
#[derive(Clone, Debug, PartialEq)]
pub struct Leak {
    pub points: Vec<[f64; 3]>,
    /// where it came from, for people (the pointfile's name)
    pub pointfile: String,
    /// "compile" or "file"
    pub from: &'static str,
}

static LEAK: Mutex<Option<Leak>> = Mutex::new(None);
/// Tests that use the one last-leak slot take turns.
#[cfg(test)]
pub static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The compile found a leak (or none: `None`).
pub fn set(leak: Option<Leak>) {
    *LEAK.lock().unwrap_or_else(|e| e.into_inner()) = leak;
}

pub fn current() -> Option<Leak> {
    LEAK.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The biggest pointfile read (SDHLT's .pts has a point every couple of units).
const MAX_POINTFILE: u64 = 16 << 20;
/// How many brushes are named round the hole.
const NEAREST: usize = 4;

fn dist_to_box(p: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> f64 {
    (0..3).map(|i| (lo[i] - p[i]).max(0.0).max(p[i] - hi[i])).map(|d| d * d).sum::<f64>().sqrt()
}

/// The hole and what's round it, for a line in a document.
pub fn analyze(doc: &MapDocument, points: &[[f64; 3]]) -> Value {
    if points.len() < 2 {
        return json!({"hole": null, "nearest": [], "start": null});
    }
    // the box round all world brushes
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let world: Vec<_> = doc.brushes.iter().filter(|(_, b)| b.entity.is_none()).map(|(id, b)| (*id, brush_bounds(b))).collect();
    for (_, bb) in &world {
        let (a, b) = ([bb.min.x, bb.min.y, bb.min.z], [bb.max.x, bb.max.y, bb.max.z]);
        for i in 0..3 {
            lo[i] = lo[i].min(a[i]);
            hi[i] = hi[i].max(b[i]);
        }
    }
    let inside = |p: &[f64; 3]| (0..3).all(|i| p[i] > lo[i] + 0.5 && p[i] < hi[i] - 0.5);
    let index = if world.is_empty() { points.len() - 1 } else { points.iter().rposition(inside).unwrap_or(points.len() - 1) };
    let hole = points[index];
    // the brushes round the hole: world first (they seal), then brush entities (they don't)
    let mut near: Vec<(f64, u64, Option<u64>)> = doc
        .brushes
        .iter()
        .map(|(id, b)| {
            let bb = brush_bounds(b);
            (dist_to_box(hole, [bb.min.x, bb.min.y, bb.min.z], [bb.max.x, bb.max.y, bb.max.z]), id.0, b.entity.map(|e| e.0))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let nearest: Vec<Value> = near
        .iter()
        .filter(|(_, _, e)| e.is_none())
        .take(NEAREST)
        .map(|(d, id, _)| json!({"brush": id, "distance": (d * 10.0).round() / 10.0}))
        .collect();
    let entities_near: Vec<Value> = near
        .iter()
        .filter_map(|(d, id, e)| e.map(|e| (d, id, e)))
        .filter(|(d, _, _)| **d < 64.0)
        .take(NEAREST)
        .map(|(d, id, e)| {
            let class = doc.entities.get(&crate::editor::doc::EntityId(e)).map(|x| x.classname().to_string()).unwrap_or_default();
            json!({"brush": id, "entity": e, "classname": class, "distance": (d * 10.0).round() / 10.0})
        })
        .collect();
    // the entity the line starts at (SDHLT starts it at one that can see the void)
    let first = points[0];
    let start = doc
        .entities
        .iter()
        .filter_map(|(id, e)| e.origin().map(|o| (id.0, e.classname().to_string(), ((o.x - first[0]).powi(2) + (o.y - first[1]).powi(2) + (o.z - first[2]).powi(2)).sqrt())))
        .filter(|x| x.2 < 64.0)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(id, class, _)| json!({"entity": id, "classname": class}));
    let length: f64 = points.windows(2).map(|w| (0..3).map(|i| (w[1][i] - w[0][i]).powi(2)).sum::<f64>().sqrt()).sum();
    json!({
        "hole": {"at": hole, "index": index},
        "nearest": nearest,
        "brushEntitiesNear": entities_near,
        "start": start,
        "length": length.round(),
        "advice": if entities_near.is_empty() {
            "Close the gap next to the hole with a world brush (or move the brush that should seal it); then compile again."
        } else {
            "Close the gap next to the hole with a world brush: brush entities (func_wall, doors, func_detail…) don't seal a map."
        },
    })
}

fn report(doc: Option<&MapDocument>) -> Value {
    match current() {
        None => json!({"leaked": false}),
        Some(l) => {
            let mut v = json!({"leaked": true, "pointfile": l.pointfile, "from": l.from, "points": l.points});
            if let Some(d) = doc {
                let a = analyze(d, &l.points);
                for k in ["hole", "nearest", "brushEntitiesNear", "start", "length", "advice"] {
                    v[k] = a[k].clone();
                }
            }
            v
        }
    }
}

async fn api_leak(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || {
        let g = ed.lock();
        Ok(report(g.as_ref().map(|s| &s.doc)))
    })
    .await
}

async fn api_load(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, body: Option<axum::Json<Value>>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    // (test builds only: the browser tests name the file instead of the dialog)
    #[cfg(test)]
    let given = body.and_then(|b| b.get("path").and_then(Value::as_str).map(std::path::PathBuf::from));
    #[cfg(not(test))]
    let given: Option<std::path::PathBuf> = {
        let _ = body;
        None
    };
    blocking(move || {
        if ed.lock().is_none() {
            return Err(NO_DOC.into());
        }
        let picked = match given {
            Some(p) => Some(p),
            None => dialog(&st, Pick::Pointfile, "")?,
        };
        let Some(path) = picked else { return Ok(json!({"cancelled": true})) };
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
        if ext != "lin" && ext != "pts" {
            return Err("Pick a leak line: the compiler's .lin or .pts file, next to the map it compiled.".into());
        }
        let meta = std::fs::metadata(&path).map_err(|e| format!("That file can't be read ({e})."))?;
        if !meta.is_file() || meta.len() > MAX_POINTFILE {
            return Err("That pointfile is too big to be a leak line.".into());
        }
        let text = String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| format!("That file can't be read ({e})."))?).to_string();
        let points = if ext == "lin" { crate::tools::compile::parse_lin(&text) } else { crate::tools::compile::parse_pts(&text) };
        if points.len() < 2 {
            return Err("There's no leak line in that file.".into());
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        set(Some(Leak { points, pointfile: name, from: "file" }));
        let g = ed.lock();
        Ok(report(g.as_ref().map(|s| &s.doc)))
    })
    .await
}

async fn api_clear(h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        set(None);
        Ok(json!({"leaked": false}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/leak", get(api_leak))
        .route("/api/tools/editor/leak/load", post(api_load))
        .route("/api/tools/editor/leak/clear", post(api_clear))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::{Entity, EntityId};
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;

    #[test]
    fn the_hole_is_where_the_line_leaves_the_world() {
        // a room 0..512 with its east wall (x 496..512) short of the ceiling: a gap at the top
        let mut d = MapDocument::default();
        let walls = [
            make_box(v3(0.0, 0.0, 0.0), v3(512.0, 512.0, 16.0), "W"),     // floor
            make_box(v3(0.0, 0.0, 240.0), v3(512.0, 512.0, 256.0), "W"),  // ceiling
            make_box(v3(0.0, 0.0, 0.0), v3(16.0, 512.0, 256.0), "W"),     // west
            make_box(v3(496.0, 0.0, 0.0), v3(512.0, 512.0, 200.0), "W"),  // east, short: the gap
            make_box(v3(0.0, 0.0, 0.0), v3(512.0, 16.0, 256.0), "W"),     // south
            make_box(v3(0.0, 496.0, 0.0), v3(512.0, 512.0, 256.0), "W"),  // north
        ];
        for (i, b) in walls.into_iter().enumerate() {
            d.brushes.insert(crate::editor::doc::BrushId(i as u64 + 1), b);
        }
        let mut e = Entity::new("info_player_start");
        e.set("origin", "256 256 64");
        d.entities.insert(EntityId(7), e);
        // the line: from the spawn, up to the gap, out through it, into the void
        let line = [[256.0, 256.0, 64.0], [400.0, 256.0, 210.0], [504.0, 256.0, 220.0], [600.0, 256.0, 220.0]];
        let a = analyze(&d, &line);
        assert_eq!(a["hole"]["index"], 2, "{a}");
        assert_eq!(a["hole"]["at"], json!([504.0, 256.0, 220.0]));
        // the east wall (4) and the ceiling (2) are nearest
        let near: Vec<u64> = a["nearest"].as_array().unwrap().iter().map(|x| x["brush"].as_u64().unwrap()).collect();
        assert_eq!(&near[..2], &[2, 4], "{a}");
        assert_eq!(a["start"]["classname"], "info_player_start");
        assert_eq!(a["length"], json!(((144f64.powi(2) + 146f64.powi(2)).sqrt() + (104f64.powi(2) + 100.0).sqrt() + 96.0).round()));
    }

    #[test]
    fn the_last_leak_is_kept_and_cleared() {
        let _turn = TEST_LOCK.blocking_lock();
        set(Some(Leak { points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], pointfile: "x.lin".into(), from: "compile" }));
        let v = report(None);
        assert_eq!((v["leaked"].clone(), v["pointfile"].clone()), (json!(true), json!("x.lin")));
        set(None);
        assert_eq!(report(None)["leaked"], false);
    }
}
