//! Extrude (TrenchBroom's Shift-drag on a face): a new brush pulled straight out of one face of
//! a brush, the face's outline swept along its normal. The source brush is left as it is. The
//! new brush is convex by construction (a prism over a convex polygon); its textures continue
//! the source's: the far end and the sides take the face of the source they line up with
//! (a box's walls carry on round the new piece), anything else the face's own texture.

use super::doc::Brush;
use super::geom::{brush_polygons, convex_hull_planes};
use super::math::Vec3;
use super::ops::{brush_from_planes, sources_of, style_from, tidy, OpResult};

/// Thinnest extrusion (units): less than this and there's no brush to make.
pub const MIN_EXTRUDE: f64 = 0.5;

/// The brush made by pulling face `face` of `b` out by `distance` (positive, along the face's
/// outward normal). Errors when the face doesn't exist or the distance is too small.
pub fn extrude_face(b: &Brush, face: usize, distance: f64) -> OpResult<Brush> {
    if !distance.is_finite() {
        return Err("That distance isn't a number.".into());
    }
    if distance < MIN_EXTRUDE {
        return Err("Drag outwards from the face to pull out a new brush.".into());
    }
    let f = b.faces.get(face).ok_or("That face doesn't exist.")?;
    let plane = f.plane().ok_or("That face is broken.")?;
    let poly = brush_polygons(b).into_iter().nth(face).unwrap_or_default();
    if poly.len() < 3 {
        return Err("That face doesn't touch the brush.".into());
    }
    let d = plane.normal * distance;
    let mut pts: Vec<Vec3> = poly.clone();
    pts.extend(poly.iter().map(|p| tidy(*p + d)));
    let planes = convex_hull_planes(&pts);
    if planes.len() < 4 {
        return Err("That face is too small to pull out.".into());
    }
    let src = sources_of([b]);
    let own = (f.texture.clone(), f.align.clone());
    let mut out = brush_from_planes(&planes, |pl| {
        // the far end and the base: the face's own texture (its world-aligned mapping carries on)
        if pl.normal.dot(plane.normal).abs() > 0.999 {
            return own.clone();
        }
        style_from(&src, pl)
    })?;
    out.entity = b.entity;
    out.group = b.group;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::{brush_bounds, brush_vertices, brush_volume, is_valid};
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;
    use crate::editor::shapes::make_spike;

    fn face_facing(b: &Brush, n: Vec3) -> usize {
        b.faces.iter().position(|f| f.plane().map(|p| p.normal.approx_eq(n, 1e-9)).unwrap_or(false)).unwrap()
    }

    fn convex(b: &Brush) -> bool {
        let planes: Vec<_> = b.faces.iter().filter_map(|f| f.plane()).collect();
        brush_vertices(b).iter().all(|v| planes.iter().all(|p| p.distance(*v) <= 1e-3))
    }

    #[test]
    fn a_box_face_pulls_out_a_box() {
        let mut b = make_box(Vec3::ZERO, v3(64.0, 32.0, 128.0), "WALL");
        let east = face_facing(&b, v3(1.0, 0.0, 0.0));
        b.faces[east].texture = "EAST".into();
        let top = face_facing(&b, v3(0.0, 0.0, 1.0));
        b.faces[top].texture = "TOP".into();
        let e = extrude_face(&b, east, 48.0).unwrap();
        assert!(is_valid(&e) && convex(&e));
        assert_eq!(e.faces.len(), 6);
        let bb = brush_bounds(&e);
        assert_eq!((bb.min, bb.max), (v3(64.0, 0.0, 0.0), v3(112.0, 32.0, 128.0)));
        assert!((brush_volume(&e) - 48.0 * 32.0 * 128.0).abs() < 1e-6);
        // the ends take the pulled face's texture, the top carries on from the source's top
        let tex = |n: Vec3| e.faces[face_facing(&e, n)].texture.clone();
        assert_eq!(tex(v3(1.0, 0.0, 0.0)), "EAST");
        assert_eq!(tex(v3(-1.0, 0.0, 0.0)), "EAST");
        assert_eq!(tex(v3(0.0, 0.0, 1.0)), "TOP");
        assert_eq!(tex(v3(0.0, 1.0, 0.0)), "WALL");
        // the source is untouched (a new brush, not a push)
        assert_eq!(brush_bounds(&b).max, v3(64.0, 32.0, 128.0));
    }

    #[test]
    fn a_slanted_face_pulls_out_along_its_normal() {
        let s = make_spike(v3(-64.0, -64.0, 0.0), v3(64.0, 64.0, 64.0), 4, 1.0, "ROCK").unwrap();
        for i in 0..s.faces.len() {
            let n = s.faces[i].plane().unwrap().normal;
            let e = extrude_face(&s, i, 16.0).unwrap();
            assert!(is_valid(&e) && convex(&e), "face {i}");
            // a prism over the face: the face's area × 16
            let poly = brush_polygons(&s)[i].clone();
            let area = crate::editor::geom::polygon_area(&poly);
            assert!((brush_volume(&e) - area * 16.0).abs() < 1e-3, "face {i}");
            // every new corner is 16 out from the face's plane or on it
            let pl = s.faces[i].plane().unwrap();
            for v in brush_vertices(&e) {
                let d = pl.distance(v);
                assert!(d.abs() < 1e-3 || (d - 16.0).abs() < 1e-3, "face {i}: {d}");
            }
            let _ = n;
        }
    }

    #[test]
    fn too_small_or_inwards_is_refused() {
        let b = make_box(Vec3::ZERO, v3(64.0, 64.0, 64.0), "WALL");
        assert!(extrude_face(&b, 0, 0.0).is_err());
        assert!(extrude_face(&b, 0, -16.0).is_err());
        assert!(extrude_face(&b, 0, f64::NAN).is_err());
        assert!(extrude_face(&b, 99, 16.0).is_err());
    }
}
