//! More brush shapes for the editor's shape tool: wedge, spike (pyramid), cone, sphere, arch
//! and torus (Hammer's and J.A.C.K.'s primitives). Box and cylinder live in `ops`. Every shape is checked like any
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
    /// Each piece sits this much higher than the one before (Hammer's "add height": spiral
    /// stairs and ramps). 0 for a flat arch.
    pub add_height: f64,
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
    if !a.add_height.is_finite() || a.add_height.abs() > 4096.0 {
        return Err("Add height must be between -4096 and 4096.".into());
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
                let rise = a.add_height * k as f64;
                let mut pts = Vec::new();
                for z in [z0 + rise, z1 + rise] {
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

/// A cone filling the box: a `sides`-sided base on its floor; `top` (0..1) is the top's size
/// as a share of the base's (0 comes to a point, like a spike; more cuts it off flat).
pub fn make_cone(a: Vec3, b: Vec3, sides: usize, top: f64, grid: f64, texture: &str) -> OpResult<Brush> {
    if !(0.0..=0.95).contains(&top) {
        return Err("The cone's top must be between 0% and 95% of its base.".into());
    }
    if top < 1e-3 {
        return make_spike(a, b, sides, grid, texture);
    }
    sides_ok(sides)?;
    let (min, max) = (a.min(b), a.max(b));
    size_ok(min, max)?;
    let c = (min + max) * 0.5;
    let (rx, ry) = ((max.x - min.x) / 2.0, (max.y - min.y) / 2.0);
    let build = |g: f64| -> OpResult<Brush> {
        // the top ring is the base scaled toward the cone's point, so each side stays flat
        // (rounding the top ring on its own would bend them); only the base is on the grid
        let base = ring(v3(c.x, c.y, min.z), rx, ry, sides, g);
        let lift = max.z - min.z;
        let mut pts = base.clone();
        pts.extend(base.iter().map(|p| tidy(v3(c.x + (p.x - c.x) * top, c.y + (p.y - c.y) * top, p.z + lift))));
        let b = brush_from_points(&pts, texture)?;
        if b.faces.len() != sides + 2 {
            return Err("Rounding bent the cone.".into());
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

/// Settings for a torus (a ring, J.A.C.K.'s torus): a tube going round, made of `sides` pieces.
#[derive(Clone, Copy, Debug)]
pub struct Torus {
    /// Middle of the ring, at its floor.
    pub centre: Vec3,
    /// Outer radii (the tube's outside edge).
    pub radius_x: f64,
    pub radius_y: f64,
    /// The tube's width (across, seen from above) and height.
    pub wall: f64,
    pub height: f64,
    /// How many pieces go round, and how many sides the tube has.
    pub sides: usize,
    pub tube_sides: usize,
    pub arc: f64,
    pub start: f64,
    /// Each piece's far end rises this much (a spiral, like a slide or a helix ramp).
    pub add_height: f64,
}

/// A torus: `sides` brushes; each joins two of the tube's cross-sections, so neighbors share
/// their ends exactly (no gaps or overlaps).
pub fn make_torus(t: &Torus, texture: &str) -> OpResult<Vec<Brush>> {
    sides_ok(t.sides)?;
    if !(3..=32).contains(&t.tube_sides) {
        return Err("The tube needs between 3 and 32 sides.".into());
    }
    if !(t.radius_x >= 2.0 && t.radius_y >= 2.0 && t.height >= 1.0) {
        return Err("The ring is too small.".into());
    }
    if !(t.wall >= 1.0) || t.wall > t.radius_x.min(t.radius_y) {
        return Err("The tube must be thinner than the ring is wide.".into());
    }
    if !(t.arc > 0.0 && t.arc <= 360.0) {
        return Err("The ring must go between 1 and 360 degrees round.".into());
    }
    if !t.add_height.is_finite() || t.add_height.abs() > 4096.0 {
        return Err("Add height must be between -4096 and 4096.".into());
    }
    let (half_w, half_h) = (t.wall / 2.0, t.height / 2.0);
    let (mx, my) = (t.radius_x - half_w, t.radius_y - half_w);
    let step = t.arc / t.sides as f64;
    let full = (t.arc - 360.0).abs() < 1e-9 && t.add_height == 0.0;
    // the cross-section at the end of piece k (k = sides wraps to 0 on a closed ring)
    let section = |k: usize| -> Vec<Vec3> {
        let k = if full && k == t.sides { 0 } else { k };
        let a = (t.start + step * k as f64).to_radians();
        let z = t.centre.z + half_h + t.add_height * k as f64;
        (0..t.tube_sides)
            .map(|j| {
                let phi = std::f64::consts::TAU * j as f64 / t.tube_sides as f64;
                let off = phi.cos() * half_w;
                tidy(v3(t.centre.x + a.cos() * (mx + off), t.centre.y + a.sin() * (my + off), z + phi.sin() * half_h))
            })
            .collect()
    };
    (0..t.sides)
        .map(|k| {
            let mut pts = section(k);
            pts.extend(section(k + 1));
            let b = brush_from_points(&pts, texture)?;
            if b.faces.len() < t.tube_sides + 2 {
                return Err("A piece of the ring came out flat: make it bigger or use fewer pieces.".into());
            }
            Ok(b)
        })
        .collect()
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
        let a = Arch { centre: Vec3::ZERO, radius_x: 128.0, radius_y: 128.0, wall: 16.0, height: 64.0, arc: 180.0, start: 0.0, sides: 8, add_height: 0.0 };
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

    /// Every face's plane keeps the whole brush behind it (convex), and no corner sits off its faces.
    fn convex(b: &Brush) -> bool {
        let planes: Vec<_> = b.faces.iter().filter_map(|f| f.plane()).collect();
        let verts = crate::editor::geom::brush_vertices(b);
        planes.len() == b.faces.len() && verts.iter().all(|v| planes.iter().all(|p| p.distance(*v) <= 1e-3))
    }

    #[test]
    fn arch_add_height_climbs() {
        let a = Arch { centre: Vec3::ZERO, radius_x: 128.0, radius_y: 128.0, wall: 32.0, height: 16.0, arc: 360.0, start: 0.0, sides: 12, add_height: 16.0 };
        let pieces = make_arch(&a, 1.0, "STEP").unwrap();
        assert_eq!(pieces.len(), 12);
        for (k, p) in pieces.iter().enumerate() {
            assert!(is_valid(p) && convex(p));
            let bb = brush_bounds(p);
            assert!((bb.min.z - 16.0 * k as f64).abs() < 1e-9, "piece {k} at {}", bb.min.z);
            assert!((bb.max.z - bb.min.z - 16.0).abs() < 1e-9);
        }
        assert!(make_arch(&Arch { add_height: f64::NAN, ..a }, 1.0, "STEP").is_err());
    }

    #[test]
    fn cone_comes_to_a_point_or_is_cut_off() {
        let point = make_cone(v3(-64.0, -64.0, 0.0), v3(64.0, 64.0, 128.0), 16, 0.0, 1.0, "ROCK").unwrap();
        assert!(is_valid(&point) && convex(&point));
        assert_eq!(point.faces.len(), 17);
        let cut = make_cone(v3(-64.0, -64.0, 0.0), v3(64.0, 64.0, 128.0), 16, 0.5, 1.0, "ROCK").unwrap();
        assert!(is_valid(&cut) && convex(&cut));
        assert_eq!(cut.faces.len(), 18);
        // a frustum: between the cone to a point and the cylinder
        let v = brush_volume(&cut);
        assert!(v > brush_volume(&point) && v < std::f64::consts::PI * 64.0 * 64.0 * 128.0, "{v}");
        // the base's corners on the 1-unit grid
        for p in crate::editor::geom::brush_vertices(&cut).into_iter().filter(|p| p.z == 0.0) {
            assert!((p.x - p.x.round()).abs() < 1e-6 && (p.y - p.y.round()).abs() < 1e-6, "{p:?}");
        }
        assert!(make_cone(Vec3::ZERO, v3(64.0, 64.0, 64.0), 8, 1.0, 1.0, "X").is_err());
        assert!(make_cone(Vec3::ZERO, v3(64.0, 64.0, 64.0), 2, 0.5, 1.0, "X").is_err());
    }

    #[test]
    fn torus_pieces_are_convex_and_meet() {
        let t = Torus { centre: Vec3::ZERO, radius_x: 256.0, radius_y: 256.0, wall: 64.0, height: 64.0, sides: 16, tube_sides: 8, arc: 360.0, start: 0.0, add_height: 0.0 };
        let pieces = make_torus(&t, "PIPE").unwrap();
        assert_eq!(pieces.len(), 16);
        for p in &pieces {
            assert!(is_valid(p) && convex(p));
            assert_eq!(p.faces.len(), 8 + 2);
            assert!(p.faces.iter().all(|f| f.texture == "PIPE"));
        }
        // the volume of a ring of 8-sided tubes (chords make it a little less than a round one)
        let total: f64 = pieces.iter().map(brush_volume).sum();
        let tube = 8.0 / 2.0 * 32.0 * 32.0 * (std::f64::consts::TAU / 8.0).sin(); // the octagon's area
        let ring = tube * std::f64::consts::TAU * 224.0;
        assert!(total < ring && total > ring * 0.95, "{total} vs {ring}");
        // neighbors share their end exactly: same corners
        let ends = |b: &Brush| crate::editor::geom::brush_vertices(b);
        let shared = ends(&pieces[0]).iter().filter(|p| ends(&pieces[1]).iter().any(|q| q.approx_eq(**p, 1e-6))).count();
        assert_eq!(shared, 8);
        // and the last meets the first on a closed ring
        let wrap = ends(&pieces[15]).iter().filter(|p| ends(&pieces[0]).iter().any(|q| q.approx_eq(**p, 1e-6))).count();
        assert_eq!(wrap, 8);
        // a spiral: each piece climbs, still convex
        let helix = make_torus(&Torus { add_height: 8.0, ..t }, "PIPE").unwrap();
        for (k, p) in helix.iter().enumerate() {
            assert!(is_valid(p) && convex(p), "helix piece {k}");
        }
        assert!(brush_bounds(&helix[15]).max.z > brush_bounds(&helix[0]).max.z + 100.0);
        // an oval ring is still made of good brushes
        let oval = make_torus(&Torus { radius_y: 160.0, ..t }, "PIPE").unwrap();
        assert!(oval.iter().all(|p| is_valid(p) && convex(p)));
        assert!(make_torus(&Torus { wall: 300.0, ..t }, "PIPE").is_err());
        assert!(make_torus(&Torus { tube_sides: 2, ..t }, "PIPE").is_err());
        assert!(make_torus(&Torus { arc: 0.0, ..t }, "PIPE").is_err());
    }

    #[test]
    fn every_shape_is_valid_convex_and_on_the_grid_where_it_can_be() {
        let (a, b) = (v3(-96.0, -64.0, 0.0), v3(96.0, 64.0, 128.0));
        let mut all = vec![make_wedge(a, b, "X").unwrap(), make_spike(a, b, 6, 1.0, "X").unwrap(), make_sphere(a, b, 10, "X").unwrap(), make_cone(a, b, 12, 0.25, 1.0, "X").unwrap()];
        all.push(crate::editor::ops::make_cylinder(v3(0.0, 0.0, 0.0), 64.0, 128.0, 12, 1.0, "X").unwrap());
        let arch = Arch { centre: Vec3::ZERO, radius_x: 96.0, radius_y: 64.0, wall: 16.0, height: 64.0, arc: 270.0, start: 45.0, sides: 9, add_height: 8.0 };
        all.extend(make_arch(&arch, 1.0, "X").unwrap());
        for (i, s) in all.iter().enumerate() {
            assert!(is_valid(s), "shape {i}");
            assert!(convex(s), "shape {i} isn't convex");
            // the plane points are what the .map stores: whole units except for spheres' curves
            // and a cut-off cone's top (scaled from its base so the sides stay flat)
            if i != 2 && i != 3 {
                for f in &s.faces {
                    for p in f.points {
                        assert!((p.x * 1024.0 - (p.x * 1024.0).round()).abs() < 1e-6, "shape {i}: {p:?}");
                    }
                }
            }
        }
    }
}
