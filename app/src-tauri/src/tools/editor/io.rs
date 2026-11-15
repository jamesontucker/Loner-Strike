//! The entity I/O view's data (agent P5): the graph of what triggers what (editor/io.rs) with
//! each entity's name, class and where it is, for the page's I/O panel and arrows and for agents
//! (MCP editor_io).
//!   GET /api/tools/editor/io            every link, every broken one, the entities in them
//!   GET /api/tools/editor/io?entity=N   the same, plus N's chain (what it sets off down the
//!                                       line, and what leads to it)

use super::{blocking, Ed, NO_DOC};
use crate::editor::doc::{EntityId, MapDocument};
use crate::editor::geom::brush_bounds;
use crate::editor::io::{graph, Graph};
use crate::server::Shared;
use axum::extract::Query;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Where an entity is: its origin, or the middle of its brushes.
fn place(doc: &MapDocument, id: EntityId) -> Option<[f64; 3]> {
    let e = doc.entities.get(&id)?;
    if let Some(o) = e.origin() {
        return Some([o.x, o.y, o.z]);
    }
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for b in doc.brushes.values().filter(|b| b.entity == Some(id)) {
        let bb = brush_bounds(b);
        for (i, (a, c)) in [(bb.min.x, bb.max.x), (bb.min.y, bb.max.y), (bb.min.z, bb.max.z)].into_iter().enumerate() {
            lo[i] = lo[i].min(a);
            hi[i] = hi[i].max(c);
        }
    }
    lo[0].is_finite().then(|| [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0])
}

/// Most links sent (a big map has a few thousand at most).
const MAX_LINKS: usize = 20_000;

pub fn report(doc: &MapDocument, g: &Graph, entity: Option<u64>) -> Value {
    let mut ids: BTreeSet<EntityId> = BTreeSet::new();
    for l in &g.links {
        ids.insert(l.from);
        ids.extend(l.to.iter().copied());
    }
    for v in g.names.values() {
        ids.extend(v.iter().copied());
    }
    let incoming: BTreeSet<EntityId> = g.links.iter().flat_map(|l| l.to.iter().copied()).collect();
    let nodes: Vec<Value> = ids
        .iter()
        .filter_map(|id| {
            let e = doc.entities.get(id)?;
            let name = e.get("targetname").filter(|n| !n.is_empty());
            Some(json!({
                "entity": id.0, "classname": e.classname(), "targetname": name, "at": place(doc, *id),
                // a name nothing fires: fine for things that work by being touched or used, else idle
                "unused": name.is_some() && !incoming.contains(id),
            }))
        })
        .collect();
    let link = |l: &crate::editor::io::Link| json!({"from": l.from.0, "to": l.to.iter().map(|x| x.0).collect::<Vec<_>>(), "key": l.key, "name": l.name, "kind": l.kind.word(), "delay": l.delay});
    let links: Vec<Value> = g.links.iter().take(MAX_LINKS).map(link).collect();
    let broken: Vec<Value> = g.broken().map(link).collect();
    let mut v = json!({
        "nodes": nodes, "links": links, "broken": broken,
        "counts": {"links": g.links.len(), "broken": broken.len(), "named": g.names.len()},
        // a name on several entities fires them all (fine, but worth knowing)
        "shared": g.names.iter().filter(|(_, v)| v.len() > 1).map(|(n, v)| json!({"name": n, "entities": v.iter().map(|x| x.0).collect::<Vec<_>>()})).collect::<Vec<_>>(),
    });
    if let Some(e) = entity {
        let (down, up) = g.chain(EntityId(e));
        v["chain"] = json!({"entity": e, "down": down.iter().map(|x| x.0).collect::<Vec<_>>(), "up": up.iter().map(|x| x.0).collect::<Vec<_>>()});
    }
    v
}

#[derive(Deserialize)]
struct Q {
    entity: Option<u64>,
}

async fn api_io(Extension(ed): Extension<Ed>, Query(q): Query<Q>) -> Response {
    blocking(move || {
        let g = ed.lock();
        let s = g.as_ref().ok_or(NO_DOC)?;
        let graph = graph(&s.doc);
        let mut v = report(&s.doc, &graph, q.entity);
        v["revision"] = json!(s.doc.revision);
        Ok(v)
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/editor/io", get(api_io))
}
