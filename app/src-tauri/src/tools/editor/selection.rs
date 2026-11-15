//! What's selected in the map editor, for AI agents (agent MCP): the page reports its selection
//! (brushes, entities, picked faces, the tool and view) whenever it changes, so an agent asked
//! to "change this" knows what "this" is; an agent can set it too, to show the person what it
//! changed or found (the page picks that up and selects it).
//!   GET  /api/tools/editor/selection   the selection, each item in a few words, bounds, center
//!                                      (`?after=<seq>`: waits up to 25 s for a newer one)
//!   POST /api/tools/editor/selection   {brushes, entities, faces: [[brush, face]], tool?, view?, by?}
//! Selection isn't part of the document: no undo step, nothing saved.

use super::{blocking, forbidden, Ed, NO_DOC};
use crate::editor::doc::{BrushId, EntityId};
use crate::editor::geom::brush_bounds;
use crate::editor::math::Bounds;
use crate::server::Shared;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::extract::Query;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Most ids kept (a select-all on a big map is still fine).
const MAX: usize = 100_000;

#[derive(Clone, Default)]
struct Sel {
    doc: u64,
    brushes: Vec<u64>,
    entities: Vec<u64>,
    faces: Vec<(u64, usize)>,
    tool: String,
    view: String,
    /// "page" (the person) or "agent"
    by: String,
    /// goes up with every change (the page follows agent selections by it)
    seq: u64,
}

/// One selection per editor (tests run their own editors side by side).
fn store() -> &'static Mutex<HashMap<usize, Sel>> {
    static S: OnceLock<Mutex<HashMap<usize, Sel>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn key(ed: &Ed) -> usize {
    Arc::as_ptr(ed) as usize
}

#[derive(Deserialize)]
struct Req {
    #[serde(default)]
    brushes: Vec<u64>,
    #[serde(default)]
    entities: Vec<u64>,
    /// [[brush, face]] or [{brush, face}]
    #[serde(default)]
    faces: Vec<Value>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    view: Option<String>,
    #[serde(default)]
    by: Option<String>,
}

fn face_of(v: &Value) -> Option<(u64, usize)> {
    match v {
        Value::Array(a) if a.len() == 2 => Some((a[0].as_u64()?, a[1].as_u64()? as usize)),
        Value::Object(_) => Some((v["brush"].as_u64()?, v["face"].as_u64()? as usize)),
        _ => None,
    }
}

fn short(s: Option<String>) -> Option<String> {
    s.map(|t| t.chars().filter(|c| c.is_ascii_alphanumeric() || "-_ ".contains(*c)).take(32).collect())
}

fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The selection as agents read it: what's still there, each in a few words, its bounds.
fn answer(s: &super::Session, sel: &Sel) -> Value {
    let d = &s.doc;
    let stale = sel.doc != 0 && sel.doc != s.id;
    let mut bounds = Bounds::empty();
    let mut items = vec![];
    let (mut brushes, mut entities, mut faces) = (vec![], vec![], vec![]);
    if !stale {
        for id in &sel.brushes {
            let Some(b) = d.brushes.get(&BrushId(*id)) else { continue };
            let bb = brush_bounds(b);
            if !bb.is_empty() {
                bounds = bounds.union(&bb);
            }
            let size = if bb.is_empty() { json!(null) } else { json!([round(bb.max.x - bb.min.x), round(bb.max.y - bb.min.y), round(bb.max.z - bb.min.z)]) };
            let mut textures: Vec<&str> = vec![];
            for f in &b.faces {
                if !textures.contains(&f.texture.as_str()) && textures.len() < 4 {
                    textures.push(&f.texture);
                }
            }
            let owner = b.entity.and_then(|e| d.entities.get(&e)).map(|e| e.classname().to_string()).unwrap_or_else(|| "world".into());
            items.push(json!({"kind": "brush", "id": id, "of": owner, "entity": b.entity.map(|e| e.0), "size": size, "textures": textures}));
            brushes.push(*id);
        }
        for id in &sel.entities {
            let Some(e) = d.entities.get(&EntityId(*id)) else { continue };
            let origin = e.origin();
            if let Some(o) = origin {
                bounds.add(o);
            }
            let mut it = json!({"kind": "entity", "id": id, "classname": e.classname(), "origin": origin.map(|o| json!([round(o.x), round(o.y), round(o.z)]))});
            if let Some(n) = e.get("targetname").filter(|n| !n.is_empty()) {
                it["targetname"] = json!(n);
            }
            items.push(it);
            entities.push(*id);
        }
        for (b, f) in &sel.faces {
            let Some(face) = d.brushes.get(&BrushId(*b)).and_then(|br| br.faces.get(*f)) else { continue };
            items.push(json!({"kind": "face", "brush": b, "face": f, "texture": face.texture}));
            faces.push(json!([b, f]));
        }
    }
    let total = items.len();
    items.truncate(200);
    let (b, c) = if bounds.is_empty() {
        (Value::Null, Value::Null)
    } else {
        let c = bounds.center();
        (json!({"min": [round(bounds.min.x), round(bounds.min.y), round(bounds.min.z)], "max": [round(bounds.max.x), round(bounds.max.y), round(bounds.max.z)]}), json!([round(c.x), round(c.y), round(c.z)]))
    };
    json!({
        "docId": s.id, "brushes": brushes, "entities": entities, "faces": faces,
        "tool": sel.tool, "view": sel.view, "by": if sel.by.is_empty() { "page" } else { sel.by.as_str() }, "seq": sel.seq,
        "count": total, "items": items, "bounds": b, "center": c,
        "empty": total == 0,
        "note": if stale { "The selection was made on another map; nothing of this one is selected." } else if total == 0 { "Nothing is selected in the editor." } else { "" },
    })
}

#[derive(Deserialize)]
struct AfterQ {
    after: Option<u64>,
}

/// Longest a `?after=` look waits for a newer selection.
/// (Parity A, QA4 area A: 55 s, under WebKit's 60 s request timeout; with the state's long poll
/// the idle editor asks about twice a minute)
const WAIT: std::time::Duration = std::time::Duration::from_secs(55);

fn seq_now(ed: &Ed) -> u64 {
    store().lock().unwrap_or_else(|e| e.into_inner()).get(&key(ed)).map_or(0, |s| s.seq)
}

/// `?after=<seq>` waits (up to 25 s) for a selection newer than that, so the page hears about
/// an agent's selection without asking every second.
async fn api_get(Extension(ed): Extension<Ed>, q: Option<Query<AfterQ>>) -> Response {
    if let Some(after) = q.and_then(|q| q.0.after) {
        let until = std::time::Instant::now() + WAIT;
        while seq_now(&ed) <= after && std::time::Instant::now() < until {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let sel = store().lock().unwrap_or_else(|e| e.into_inner()).get(&key(&ed)).cloned().unwrap_or_default();
        Ok(answer(s, &sel))
    })
    .await
}

async fn api_set(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<Req>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        if req.brushes.len() + req.entities.len() + req.faces.len() > MAX {
            return Err("That's too much to select at once.".into());
        }
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let mut all = store().lock().unwrap_or_else(|e| e.into_inner());
        let old = all.get(&key(&ed)).cloned().unwrap_or_default();
        let mut faces: Vec<(u64, usize)> = req.faces.iter().filter_map(face_of).collect();
        faces.dedup();
        let by = match req.by.as_deref() {
            Some("agent") => "agent",
            _ => "page",
        };
        let sel = Sel {
            doc: s.id,
            brushes: req.brushes,
            entities: req.entities,
            faces,
            // (the page says its tool and view; an agent's selection keeps them)
            tool: short(req.tool).unwrap_or(old.tool),
            view: short(req.view).unwrap_or(old.view),
            by: by.into(),
            seq: old.seq + 1,
        };
        let v = answer(s, &sel);
        all.insert(key(&ed), sel);
        Ok(v)
    })
    .await
}

/// Mounted by tools/editor.rs (`routes_with`), sharing its session.
pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/editor/selection", get(api_get).post(api_set))
}
