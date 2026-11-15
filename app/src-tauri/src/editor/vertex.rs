//! Vertex editing (TrenchBroom's vertex tool, simply): move some corners of a brush and
//! rebuild it around the new corners. Brushes must stay convex, so a move that would dent
//! the brush inwards, or swallow another corner, is refused with a reason instead.

use super::doc::Brush;
use super::geom::{brush_vertices, convex_hull_planes};
use super::math::{Vec3, POINT_EPS};
use super::ops::{brush_from_planes, sources_of, style_from, tidy, OpResult};

/// How close a picked point must be to a corner to count as that corner.
const PICK_EPS: f64 = 0.01;

/// The brush's corner nearest `p`, when it's within `PICK_EPS`.
pub fn find_vertex(b: &Brush, p: Vec3) -> Option<Vec3> {
    brush_vertices(b).into_iter().filter(|v| (*v - p).len() <= PICK_EPS).min_by(|a, c| (*a - p).len().total_cmp(&(*c - p).len()))
}

/// Move the corners at `picks` by `delta` and rebuild the brush. Faces keep the texture of
/// the old face they line up with best. Errors (nothing changes) when a picked point isn't
/// a corner, when the brush would go flat, dent inwards or lose a corner.
pub fn move_vertices(b: &Brush, picks: &[Vec3], delta: Vec3) -> OpResult<Brush> {
    if picks.is_empty() {
        return Err("Pick some corners first.".into());
    }
    if !delta.is_finite() {
        return Err("That move isn't a number.".into());
    }
    let verts = brush_vertices(b);
    let mut moved = vec![false; verts.len()];
    for p in picks {
        let i = verts.iter().position(|v| (*v - *p).len() <= PICK_EPS).ok_or("That corner isn't on the brush any more.")?;
        moved[i] = true;
    }
    let pts: Vec<Vec3> = verts.iter().zip(&moved).map(|(v, m)| if *m { tidy(*v + delta) } else { *v }).collect();
    let planes = convex_hull_planes(&pts);
    if planes.len() < 4 {
        return Err("That would flatten the brush.".into());
    }
    let src = sources_of([b]);
    let mut out = brush_from_planes(&planes, |pl| style_from(&src, pl)).map_err(|_| "That would flatten the brush.".to_string())?;
    out.entity = b.entity;
    out.group = b.group;
    // every corner must still be a corner: anything inside the hull means the shape the
    // mapper asked for isn't convex
    let after = brush_vertices(&out);
    let kept = |p: &Vec3| after.iter().any(|q| (*q - *p).len() <= POINT_EPS * 10.0);
    for (p, m) in pts.iter().zip(&moved) {
        if !kept(p) {
            return Err(if *m {
                "That would dent the brush inwards. Brushes have to stay convex: move it less far, or clip the brush in two first.".into()
            } else {
                "That would swallow another corner of the brush. Move it less far, or move that corner too.".into()
            });
        }
    }
    Ok(out)
}

/// Rebuild `b` round `pts` (its corners after an edit), textures from the old faces. `must`:
/// points that have to come out as corners, with the reason to give when one doesn't.
fn rebuild(b: &Brush, pts: &[Vec3], must: &[(Vec3, &str)]) -> OpResult<Brush> {
    let planes = convex_hull_planes(pts);
    if planes.len() < 4 {
        return Err("That would flatten the brush.".into());
    }
    let src = sources_of([b]);
    let mut out = brush_from_planes(&planes, |pl| style_from(&src, pl)).map_err(|_| "That would flatten the brush.".to_string())?;
    out.entity = b.entity;
    out.group = b.group;
    let after = brush_vertices(&out);
    for (p, why) in must {
        if !after.iter().any(|q| (*q - *p).len() <= POINT_EPS * 10.0) {
            return Err((*why).to_string());
        }
    }
    Ok(out)
}

/// Split: a new corner at `at` + `delta` (TrenchBroom's edge and face split: drag the middle of
/// an edge or a face out). Every old corner must stay a corner and the new one must be one, so
/// the brush stays convex; a split pushed inwards is refused.
pub fn add_vertex(b: &Brush, at: Vec3, delta: Vec3) -> OpResult<Brush> {
    if !at.is_finite() || !delta.is_finite() {
        return Err("That move isn't a number.".into());
    }
    let verts = brush_vertices(b);
    let p = tidy(at + delta);
    if verts.iter().any(|v| (*v - p).len() <= PICK_EPS) {
        return Err("There's a corner there already.".into());
    }
    let mut pts = verts.clone();
    pts.push(p);
    let mut must: Vec<(Vec3, &str)> = vec![(p, "A new corner there would dent the brush inwards or lie flat on a face: drag it outwards. Brushes have to stay convex.")];
    must.extend(verts.iter().map(|v| (*v, "That would swallow another corner of the brush. Drag it less far.")));
    rebuild(b, &pts, &must)
}

/// Merge: the corners at `picks` become one corner at their middle (TrenchBroom: drop a corner
/// on another). The other corners must stay corners.
pub fn merge_vertices(b: &Brush, picks: &[Vec3]) -> OpResult<Brush> {
    if picks.len() < 2 {
        return Err("Pick two or more corners to merge.".into());
    }
    let verts = brush_vertices(b);
    let mut merged = vec![false; verts.len()];
    for p in picks {
        let i = verts.iter().position(|v| (*v - *p).len() <= PICK_EPS).ok_or("That corner isn't on the brush any more.")?;
        merged[i] = true;
    }
    let n = merged.iter().filter(|m| **m).count() as f64;
    let mid = tidy(verts.iter().zip(&merged).filter(|(_, m)| **m).fold(Vec3::ZERO, |a, (v, _)| a + *v) * (1.0 / n));
    let mut pts: Vec<Vec3> = verts.iter().zip(&merged).filter(|(_, m)| !**m).map(|(v, _)| *v).collect();
    pts.push(mid);
    let must: Vec<(Vec3, &str)> = pts.iter().map(|v| (*v, "Merging those would leave a corner inside the brush. Pick corners next to each other.")).collect();
    rebuild(b, &pts, &must)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::{brush_bounds, brush_volume, is_valid};
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;

    fn cube() -> Brush {
        make_box(Vec3::ZERO, v3(64.0, 64.0, 64.0), "CRATE")
    }

    #[test]
    fn move_a_top_edge_makes_a_ramp() {
        let b = cube();
        // both corners of the top edge on the -y side come down to the floor... nearly
        let picks = [v3(0.0, 0.0, 64.0), v3(64.0, 0.0, 64.0)];
        let r = move_vertices(&b, &picks, v3(0.0, 0.0, -48.0)).unwrap();
        assert!(is_valid(&r));
        assert_eq!(r.faces.len(), 6);
        let expect = 64.0 * 64.0 * 16.0 + 64.0 * 64.0 * 48.0 / 2.0;
        assert!((brush_volume(&r) - expect).abs() < 1e-6, "{}", brush_volume(&r));
        // textures come from the old faces
        assert!(r.faces.iter().all(|f| f.texture == "CRATE"));
    }

    #[test]
    fn one_corner_up_splits_the_top() {
        let r = move_vertices(&cube(), &[v3(64.0, 64.0, 64.0)], v3(0.0, 0.0, 32.0)).unwrap();
        assert!(is_valid(&r));
        // the top is no longer flat: two triangles
        assert_eq!(r.faces.len(), 7);
        assert!((brush_bounds(&r).max.z - 96.0).abs() < 1e-9);
    }

    #[test]
    fn whole_face_moves_like_push_pull() {
        let top = [v3(0.0, 0.0, 64.0), v3(64.0, 0.0, 64.0), v3(64.0, 64.0, 64.0), v3(0.0, 64.0, 64.0)];
        let r = move_vertices(&cube(), &top, v3(0.0, 0.0, 16.0)).unwrap();
        assert!((brush_volume(&r) - 64.0 * 64.0 * 80.0).abs() < 1e-6);
    }

    #[test]
    fn dents_and_bad_picks_are_refused() {
        let b = cube();
        // a corner pushed into the middle: not convex
        let e = move_vertices(&b, &[v3(64.0, 64.0, 64.0)], v3(-48.0, -48.0, -48.0)).unwrap_err();
        assert!(e.contains("dent"), "{e}");
        // not moving at all is fine
        assert!(move_vertices(&b, &[v3(64.0, 64.0, 64.0)], Vec3::ZERO).is_ok());
        // pulled down past the corner below it: (64 64 0) ends up between (0 0 64) and the moved one
        let e = move_vertices(&b, &[v3(64.0, 64.0, 64.0)], v3(64.0, 64.0, -128.0)).unwrap_err();
        assert!(e.contains("swallow"), "{e}");
        // not a corner
        assert!(move_vertices(&b, &[v3(32.0, 32.0, 64.0)], v3(0.0, 0.0, 8.0)).is_err());
        assert!(move_vertices(&b, &[], v3(0.0, 0.0, 8.0)).is_err());
        // all of one face onto the opposite face: flat
        let top = [v3(0.0, 0.0, 64.0), v3(64.0, 0.0, 64.0), v3(64.0, 64.0, 64.0), v3(0.0, 64.0, 64.0)];
        assert!(move_vertices(&b, &top, v3(0.0, 0.0, -64.0)).is_err());
        assert_eq!(find_vertex(&b, v3(64.0, 64.0, 64.001)), Some(v3(64.0, 64.0, 64.0)));
        assert_eq!(find_vertex(&b, v3(60.0, 64.0, 64.0)), None);
    }

    #[test]
    fn split_an_edge_outwards_makes_a_roof_ridge() {
        // the middle of the top edge along x at y=0, pulled out (-y) and up: a new corner
        let b = cube();
        let r = add_vertex(&b, v3(32.0, 0.0, 64.0), v3(0.0, -16.0, 16.0)).unwrap();
        assert!(is_valid(&r));
        assert!(brush_vertices(&r).contains(&v3(32.0, -16.0, 80.0)));
        assert_eq!(brush_vertices(&r).len(), 9);
        assert!(brush_volume(&r) > 64.0f64.powi(3));
        // every face still has the whole brush behind it
        let planes: Vec<_> = r.faces.iter().filter_map(|f| f.plane()).collect();
        assert!(brush_vertices(&r).iter().all(|v| planes.iter().all(|p| p.distance(*v) <= 1e-3)));
    }

    #[test]
    fn split_a_face_into_a_pyramid_cap() {
        let r = add_vertex(&cube(), v3(32.0, 32.0, 64.0), v3(0.0, 0.0, 32.0)).unwrap();
        assert!(is_valid(&r));
        // the flat top becomes four sloped triangles
        assert_eq!(r.faces.len(), 5 + 4);
        assert!((brush_volume(&r) - (64.0f64.powi(3) + 64.0 * 64.0 * 32.0 / 3.0)).abs() < 1e-6);
    }

    #[test]
    fn splits_that_dent_or_sit_flat_are_refused() {
        let b = cube();
        // inwards: inside the brush
        assert!(add_vertex(&b, v3(32.0, 32.0, 64.0), v3(0.0, 0.0, -8.0)).unwrap_err().contains("dent"));
        // not moved: flat on the face, not a corner
        assert!(add_vertex(&b, v3(32.0, 32.0, 64.0), Vec3::ZERO).is_err());
        // onto an existing corner
        assert!(add_vertex(&b, v3(32.0, 0.0, 64.0), v3(32.0, 0.0, 0.0)).unwrap_err().contains("already"));
        // so far out it swallows a corner
        // out past a corner along its diagonal: that corner ends up inside
        assert!(add_vertex(&b, v3(64.0, 32.0, 64.0), v3(64.0, 96.0, 64.0)).unwrap_err().contains("swallow"));
    }

    #[test]
    fn merging_corners_and_dropping_one_on_another() {
        let b = cube();
        // the two top corners on the -y side merge at their middle: a 5-corner... wedge-like brush
        let r = merge_vertices(&b, &[v3(0.0, 0.0, 64.0), v3(64.0, 0.0, 64.0)]).unwrap();
        assert!(is_valid(&r));
        assert_eq!(brush_vertices(&r).len(), 7);
        assert!(brush_vertices(&r).contains(&v3(32.0, 0.0, 64.0)));
        // a corner dragged onto its neighbor merges with it (the move tool does that too)
        let d = move_vertices(&b, &[v3(64.0, 0.0, 64.0)], v3(-64.0, 0.0, 0.0)).unwrap();
        assert!(is_valid(&d));
        assert_eq!(brush_vertices(&d).len(), 7);
        assert!(merge_vertices(&b, &[v3(0.0, 0.0, 64.0)]).is_err());
        assert!(merge_vertices(&b, &[v3(0.0, 0.0, 64.0), v3(1.0, 2.0, 3.0)]).is_err());
        // all of the top onto one point: a pyramid
        let top = [v3(0.0, 0.0, 64.0), v3(64.0, 0.0, 64.0), v3(64.0, 64.0, 64.0), v3(0.0, 64.0, 64.0)];
        let p = merge_vertices(&b, &top).unwrap();
        assert_eq!(p.faces.len(), 5);
    }
}
