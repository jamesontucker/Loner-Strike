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
}
