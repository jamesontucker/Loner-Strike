//! One-click fixes for the deeper checks, applied inside the caller's transaction (so each
//! click, or each "Fix all", is one undo step). Things already changed or gone are skipped.

use super::FixData;
use crate::editor::doc::{BrushId, EntityId};
use crate::editor::geom::polygon_center;
use crate::editor::history::Tx;
use crate::editor::mapfile::fmt_num;
use crate::editor::ops::{self, OpResult};
use crate::editor::texture;

/// The codes this module handles.
pub fn handles(fix: &str) -> bool {
    matches!(fix, "nudge-face" | "null-face" | "move-entity" | "close-gap" | "snap-brush" | "null-hidden" | "align-neighbor" | "reset-scale")
}

/// Apply one fix; returns 1 when something changed.
pub fn apply(tx: &mut Tx, fix: &str, brush: Option<BrushId>, entity: Option<EntityId>, face: Option<usize>, data: Option<&FixData>) -> OpResult<usize> {
    let d = data.cloned().unwrap_or_default();
    let face = face.or(d.face);
    let get = |tx: &Tx, id: Option<BrushId>| id.and_then(|i| tx.doc().brushes.get(&i).cloned().map(|b| (i, b)));
    Ok(match fix {
        "nudge-face" | "close-gap" => {
            let (Some((id, b)), Some(f)) = (get(tx, brush), face) else { return Ok(0) };
            if f >= b.faces.len() {
                return Ok(0);
            }
            let dist = d.dist.unwrap_or(1.0).clamp(-64.0, 64.0);
            tx.set_brush(id, ops::move_face(&b, f, dist, true)?);
            1
        }
        "null-face" => {
            let (Some((id, mut b)), Some(f)) = (get(tx, brush), face) else { return Ok(0) };
            let Some(fc) = b.faces.get_mut(f) else { return Ok(0) };
            fc.texture = "NULL".into();
            tx.set_brush(id, b);
            1
        }
        "move-entity" => {
            let (Some(id), Some(to)) = (entity, d.to) else { return Ok(0) };
            let Some(mut e) = tx.doc().entities.get(&id).cloned() else { return Ok(0) };
            if !to.iter().all(|x| x.is_finite() && x.abs() <= 65536.0) {
                return Err("That spot is outside the map.".into());
            }
            e.set("origin", &format!("{} {} {}", fmt_num(to[0]), fmt_num(to[1]), fmt_num(to[2])));
            tx.set_entity(id, e);
            1
        }
        "snap-brush" => {
            let Some((id, b)) = get(tx, brush) else { return Ok(0) };
            tx.set_brush(id, ops::snap_vertices(&b, 1.0)?);
            1
        }
        "null-hidden" => {
            let list = super::hidden_face_list(tx.doc());
            let mut n = 0;
            let mut by_brush: std::collections::BTreeMap<BrushId, Vec<usize>> = std::collections::BTreeMap::new();
            for (id, f) in list {
                by_brush.entry(id).or_default().push(f);
            }
            for (id, faces) in by_brush {
                let Some(mut b) = tx.doc().brushes.get(&id).cloned() else { continue };
                for f in faces {
                    b.faces[f].texture = "NULL".into();
                    n += 1;
                }
                tx.set_brush(id, b);
            }
            n.min(1)
        }
        "align-neighbor" => {
            let (Some((id, mut b)), Some(f)) = (get(tx, brush), face) else { return Ok(0) };
            let (Some((_, other)), Some(of)) = (get(tx, d.other_brush.map(BrushId)), d.other_face) else { return Ok(0) };
            let (Some(src), true) = (other.faces.get(of), f < b.faces.len()) else { return Ok(0) };
            b.faces[f].align = src.align.clone();
            tx.set_brush(id, b);
            1
        }
        "reset-scale" => {
            let (Some((id, mut b)), Some(f)) = (get(tx, brush), face) else { return Ok(0) };
            let polys = crate::editor::geom::brush_polygons(&b);
            let Some(poly) = polys.get(f).filter(|p| !p.is_empty()) else { return Ok(0) };
            let a = &b.faces[f].align;
            let (su, sv) = (if a.scale_u < 0.0 { -1.0 } else { 1.0 }, if a.scale_v < 0.0 { -1.0 } else { 1.0 });
            b.faces[f].align = texture::set_scale(a, su, sv, polygon_center(poly));
            tx.set_brush(id, b);
            1
        }
        _ => return Err("That problem can't be fixed here.".into()),
    })
}
