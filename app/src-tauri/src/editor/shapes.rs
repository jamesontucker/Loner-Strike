//! More brush shapes for the editor's shape tool: wedge, spike (pyramid/cone), sphere and
//! arch (Hammer's primitives). Box and cylinder live in `ops`. Every shape is checked like any
//! other brush; points are rounded to the grid when that keeps the shape exact.

use super::doc::Brush;
use super::geom::convex_hull_planes;
use super::math::{v3, Plane, Vec3};
use super::ops::{brush_from_planes, tidy, OpResult};
use super::texture::world_aligned;

/// Most sides a round shape may have (the arch's pieces count separately).
pub const MAX_SIDES: usize = 64;

/// A brush around a set of points (their convex hull), world-aligned `texture` on every face.
pub fn brush_from_points(points: &[Vec3], texture: &str) -> OpResult<Brush> {
    let planes = convex_hull_planes(points);
    if planes.len() < 4 {
        return Err("That shape is flat: drag out a bigger one.".into());
    }
    brush_from_planes(&planes, |p| (texture.to_string(), world_aligned(p.normal)))
}

fn snapped_xy(p: Vec3, grid: f64) -> Vec3 {
    if grid > 0.0 {
        let s = p.snapped(grid);
        v3(s.x, s.y, p.z)
    } else {
        p
    }
}

/// A ring of `sides` points round `centre` (counter-clockwise from above, starting on +x).
fn ring(centre: Vec3, rx: f64, ry: f64, sides: usize, grid: f64) -> Vec<Vec3> {
    (0..sides)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / sides as f64;
            snapped_xy(centre + v3(a.cos() * rx, a.sin() * ry, 0.0), grid)
        })
        .collect()
}

fn sides_ok(sides: usize) -> OpResult<()> {
    if (3..=MAX_SIDES).contains(&sides) {
        Ok(())
    } else {
        Err(format!("Pick between 3 and {MAX_SIDES} sides."))
    }
}

fn size_ok(min: Vec3, max: Vec3) -> OpResult<()> {
    let s = max - min;
    if s.x < 1.0 || s.y < 1.0 || s.z < 1.0 {
        return Err("The shape is too thin: drag out a bigger one.".into());
    }
    Ok(())
}

/// A wedge (ramp) filling the box `a`..`b`: full floor, the top edge along the +y side, so the
/// slope faces -y. Turn it with the rotate buttons for other directions.
pub fn make_wedge(a: Vec3, b: Vec3, texture: &str) -> OpResult<Brush> {
    let (min, max) = (a.min(b), a.max(b));
    size_ok(min, max)?;
    let pts = [
        v3(min.x, min.y, min.z),
        v3(max.x, min.y, min.z),
        v3(max.x, max.y, min.z),
        v3(min.x, max.y, min.z),
        v3(min.x, max.y, max.z),
        v3(max.x, max.y, max.z),
    ];
    brush_from_points(&pts, texture)
}

/// A spike (pyramid or cone): a `sides`-sided base filling the box's floor, the point at the
/// middle of its top.
pub fn make_spike(a: Vec3, b: Vec3, sides: usize, grid: f64, texture: &str) -> OpResult<Brush> {
    sides_ok(sides)?;
    let (min, max) = (a.min(b), a.max(b));
    size_ok(min, max)?;
    let c = (min + max) * 0.5;
    let base = v3(c.x, c.y, min.z);
    let apex = tidy(v3(c.x, c.y, max.z));
    let build = |g: f64| -> OpResult<Brush> {
        let mut pts = ring(base, (max.x - min.x) / 2.0, (max.y - min.y) / 2.0, sides, g);
        pts.push(apex);
        let b = brush_from_points(&pts, texture)?;
        if b.faces.len() != sides + 1 {
            return Err("Rounding bent the spike.".into());
        }
        Ok(b)
    };
    if grid > 0.0 {
        if let Ok(b) = build(grid) {
            return Ok(b);
        }
    }
    build(0.0)
}

/// A sphere (or a squashed one) filling the box: `sides` facets round the middle and half as
/// many bands from pole to pole. 8-16 sides look round enough; every facet is one brush face.
pub fn make_sphere(a: Vec3, b: Vec3, sides: usize, texture: &str) -> OpResult<Brush> {
    if !(4..=32).contains(&sides) {
        return Err("A sphere needs between 4 and 32 sides.".into());
    }
    let (min, max) = (a.min(b), a.max(b));
    size_ok(min, max)?;
    let c = (min + max) * 0.5;
    let r = (max - min) * 0.5;
    let bands = (sides / 2).max(2);
    // the points: north pole, the rings, south pole
    let at = |band: usize, i: usize| -> Vec3 {
        let lat = std::f64::consts::PI * band as f64 / bands as f64; // 0 at the top
        let lon = std::f64::consts::TAU * i as f64 / sides as f64;
        tidy(c + v3(lat.sin() * lon.cos() * r.x, lat.sin() * lon.sin() * r.y, lat.cos() * r.z))
    };
    let mut planes = Vec::new();
    for band in 0..bands {
        for i in 0..sides {
            let j = (i + 1) % sides;
            // a facet's corners (two of them meet at a pole on the end bands)
            let (p0, p1, p2, p3) = (at(band, i), at(band, j), at(band + 1, j), at(band + 1, i));
            let tri = if band == 0 { [p0, p3, p2] } else { [p0, p1, p3] };
            let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            if n.len() < 1e-9 {
                continue;
            }
            let mut pl = Plane::from_point_normal(tri[0], n.normalized());
            if pl.distance(c) > 0.0 {
                pl = pl.flipped();
            }
            planes.push(pl);
        }
    }
    brush_from_planes(&planes, |p| (texture.to_string(), world_aligned(p.normal)))
}

/// Settings for an arch: a curved wall seen from above, made of `sides` pieces.
#[derive(Clone, Copy, Debug)]
pub struct Arch {
    /// Middle of the circle the arch follows, at its floor.
    pub centre: Vec3,
    pub radius_x: f64,
    pub radius_y: f64,
    /// Wall thickness, inwards from the outer edge.
    pub wall: f64,
    pub height: f64,
    /// How far round it goes (degrees, up to 360) and where it starts (0 = +x).
    pub arc: f64,
    pub start: f64,
    pub sides: usize,
}

/// An arch: `sides` brushes, each a four-sided block between two angles.
pub fn make_arch(a: &Arch, grid: f64, texture: &str) -> OpResult<Vec<Brush>> {
    sides_ok(a.sides)?;
    if !(a.radius_x >= 2.0 && a.radius_y >= 2.0 && a.height >= 1.0) {
        return Err("The arch is too small.".into());
    }
    if !(a.wall >= 1.0) || a.wall >= a.radius_x.min(a.radius_y) {
        return Err("The wall must be thinner than the arch is wide.".into());
    }
    if !(a.arc > 0.0 && a.arc <= 360.0) {
        return Err("The arch must go between 1 and 360 degrees round.".into());
    }
    let point = |deg: f64, inner: bool, z: f64, g: f64| {
        let t = deg.to_radians();
        let (rx, ry) = if inner { (a.radius_x - a.wall, a.radius_y - a.wall) } else { (a.radius_x, a.radius_y) };
        let p = snapped_xy(a.centre + v3(t.cos() * rx, t.sin() * ry, 0.0), g);
        v3(p.x, p.y, z)
    };
    let build = |g: f64| -> OpResult<Vec<Brush>> {
        let step = a.arc / a.sides as f64;
        let (z0, z1) = (a.centre.z, a.centre.z + a.height);
        (0..a.sides)
            .map(|k| {
                let (d0, d1) = (a.start + step * k as f64, a.start + step * (k + 1) as f64);
                let mut pts = Vec::new();
                for z in [z0, z1] {
                    for (d, inner) in [(d0, false), (d1, false), (d1, true), (d0, true)] {
                        pts.push(point(d, inner, z, g));
                    }
                }
                let b = brush_from_points(&pts, texture)?;
                if b.faces.len() != 6 {
                    return Err("Rounding bent a piece of the arch.".into());
                }
                Ok(b)
            })
            .collect()
    };
    if grid > 0.0 {
        if let Ok(v) = build(grid) {
            return Ok(v);
        }
    }
    build(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::{brush_bounds, brush_volume, is_valid};

    #[test]
    fn wedge_is_half_a_box() {
        let w = make_wedge(v3(0.0, 0.0, 0.0), v3(64.0, 128.0, 32.0), "RAMP").unwrap();
        assert!(is_valid(&w));
        assert_eq!(w.faces.len(), 5);
        assert!((brush_volume(&w) - 64.0 * 128.0 * 32.0 / 2.0).abs() < 1e-6);
        assert!(make_wedge(Vec3::ZERO, v3(64.0, 64.0, 0.0), "RAMP").is_err());
    }

    #[test]
    fn spike_is_a_pyramid() {
        let s = make_spike(v3(-32.0, -32.0, 0.0), v3(32.0, 32.0, 96.0), 4, 1.0, "ROCK").unwrap();
        assert!(is_valid(&s));
        assert_eq!(s.faces.len(), 5);
        let bb = brush_bounds(&s);
        assert!((bb.max.z - 96.0).abs() < 1e-9);
        // a 4-sided spike in a 64 box: a square of diagonal 64 at the base
        assert!((brush_volume(&s) - (32.0 * 32.0 * 2.0) * 96.0 / 3.0).abs() < 1e-3);
        let cone = make_spike(Vec3::ZERO, v3(64.0, 64.0, 64.0), 16, 1.0, "ROCK").unwrap();
        assert_eq!(cone.faces.len(), 17);
        assert!(make_spike(Vec3::ZERO, v3(64.0, 64.0, 64.0), 2, 1.0, "ROCK").is_err());
    }

    #[test]
    fn sphere_is_round_enough() {
        let s = make_sphere(v3(-64.0, -64.0, -64.0), v3(64.0, 64.0, 64.0), 12, "ROCK").unwrap();
        assert!(is_valid(&s));
        assert_eq!(s.faces.len(), 12 * 6);
        let ideal = 4.0 / 3.0 * std::f64::consts::PI * 64.0f64.powi(3);
        let v = brush_volume(&s);
        assert!(v < ideal && v > ideal * 0.8, "{v} vs {ideal}");
        let bb = brush_bounds(&s);
        assert!((bb.max.z - 64.0).abs() < 1e-6 && (bb.min.z + 64.0).abs() < 1e-6);
        assert!(make_sphere(Vec3::ZERO, v3(64.0, 64.0, 64.0), 3, "X").is_err());
        assert!(make_sphere(Vec3::ZERO, v3(64.0, 64.0, 64.0), 33, "X").is_err());
    }

    #[test]
    fn arch_pieces() {
        let a = Arch { centre: Vec3::ZERO, radius_x: 128.0, radius_y: 128.0, wall: 16.0, height: 64.0, arc: 180.0, start: 0.0, sides: 8 };
        let pieces = make_arch(&a, 1.0, "BRICK").unwrap();
        assert_eq!(pieces.len(), 8);
        assert!(pieces.iter().all(is_valid));
        let total: f64 = pieces.iter().map(brush_volume).sum();
        // half a ring 128..112 wide, 64 high; the pieces are chords so a little less
        let ring = std::f64::consts::PI * (128.0f64.powi(2) - 112.0f64.powi(2)) / 2.0 * 64.0;
        assert!(total < ring * 1.02 && total > ring * 0.9, "{total} vs {ring}");
        assert!(make_arch(&Arch { wall: 200.0, ..a }, 1.0, "BRICK").is_err());
        assert!(make_arch(&Arch { arc: 0.0, ..a }, 1.0, "BRICK").is_err());
        let full = make_arch(&Arch { arc: 360.0, sides: 12, ..a }, 0.0, "BRICK").unwrap();
        assert_eq!(full.len(), 12);
    }
}
