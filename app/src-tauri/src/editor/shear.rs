//! Shear (Hammer's skew, TrenchBroom's shear handle): slide the selection along one axis by an
//! amount that grows with another axis, about a pivot. A box sheared in the Front view becomes a
//! slanted wall; brushes stay convex (a shear maps flat faces to flat faces and keeps volume),
//! but very steep shears are refused: their faces get too thin to compile cleanly.

use super::math::{Affine, Mat3, Vec3};
use super::ops::OpResult;

/// The steepest shear allowed: 8 units along for every unit across (about 83°).
pub const MAX_SLOPE: f64 = 8.0;

fn unit(i: usize) -> Vec3 {
    [Vec3::X, Vec3::Y, Vec3::Z][i]
}

/// The shear moving coordinate `along` by `factor` × (the point's `axis` coordinate − the
/// pivot's). Points on the pivot's plane stay put.
pub fn shear_affine(axis: usize, along: usize, factor: f64, pivot: Vec3) -> OpResult<Affine> {
    if axis > 2 || along > 2 || axis == along {
        return Err("Pick an edge of the selection to shear along.".into());
    }
    if !factor.is_finite() {
        return Err("That shear isn't a number.".into());
    }
    if factor.abs() > MAX_SLOPE {
        return Err(format!("That's too steep to shear (more than {MAX_SLOPE} units along for every unit across). Shear less, or rotate the brush instead."));
    }
    let mut r = Mat3::IDENTITY.r;
    r[along] = unit(along) + unit(axis) * factor;
    Ok(Affine::around(Mat3 { r }, pivot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::{brush_bounds, brush_vertices, brush_volume, is_valid};
    use crate::editor::math::v3;
    use crate::editor::ops::{make_box, transform};
    use crate::editor::shapes::make_spike;

    fn convex(b: &crate::editor::doc::Brush) -> bool {
        let planes: Vec<_> = b.faces.iter().filter_map(|f| f.plane()).collect();
        brush_vertices(b).iter().all(|v| planes.iter().all(|p| p.distance(*v) <= 1e-3))
    }

    #[test]
    fn a_box_leans_into_a_slanted_wall() {
        // Front view: the top edge dragged 64 east, the bottom (z = 0) stays
        let b = make_box(Vec3::ZERO, v3(32.0, 128.0, 128.0), "WALL");
        let t = shear_affine(2, 0, 64.0 / 128.0, Vec3::ZERO).unwrap();
        let s = transform(&b, &t, true).unwrap();
        assert!(is_valid(&s) && convex(&s));
        assert_eq!(s.faces.len(), 6);
        assert!((brush_volume(&s) - brush_volume(&b)).abs() < 1e-6, "a shear keeps the volume");
        let bb = brush_bounds(&s);
        assert_eq!((bb.min, bb.max), (v3(0.0, 0.0, 0.0), v3(96.0, 128.0, 128.0)));
        // the top moved, the floor didn't; every corner on whole units
        let vs = brush_vertices(&s);
        assert!(vs.contains(&v3(64.0, 0.0, 128.0)) && vs.contains(&v3(0.0, 0.0, 0.0)));
        assert!(vs.iter().all(|p| p.x.fract() == 0.0 && p.y.fract() == 0.0 && p.z.fract() == 0.0));
    }

    #[test]
    fn any_brush_stays_valid_and_steep_or_bad_shears_are_refused() {
        let spike = make_spike(v3(-64.0, -64.0, 0.0), v3(64.0, 64.0, 96.0), 6, 1.0, "ROCK").unwrap();
        for (axis, along) in [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)] {
            let t = shear_affine(axis, along, -0.75, v3(0.0, 0.0, 0.0)).unwrap();
            let s = transform(&spike, &t, true).unwrap();
            assert!(is_valid(&s) && convex(&s), "{axis} {along}");
            assert!((brush_volume(&s) - brush_volume(&spike)).abs() < 1e-3);
        }
        assert!(shear_affine(0, 0, 1.0, Vec3::ZERO).is_err());
        assert!(shear_affine(3, 0, 1.0, Vec3::ZERO).is_err());
        assert!(shear_affine(2, 0, f64::NAN, Vec3::ZERO).is_err());
        assert!(shear_affine(2, 0, 9.0, Vec3::ZERO).unwrap_err().contains("steep"));
    }
}
