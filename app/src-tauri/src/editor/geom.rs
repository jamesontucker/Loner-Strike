//! Brush geometry: plane sets to face polygons, validity, bounds, volume and convex hulls.

use super::doc::{Brush, Face};
use super::math::{v3, Bounds, Plane, Side, Vec3, BIG, PLANE_EPS, POINT_EPS};

/// A convex polygon, vertices counter-clockwise seen from the front (the normal's side), which
/// is also three.js's front-face winding.
pub type Polygon = Vec<Vec3>;

/// A huge square on the plane, counter-clockwise seen from the front.
pub fn base_polygon(plane: &Plane) -> Polygon {
    let n = plane.normal;
    let u = n.any_perpendicular();
    let v = n.cross(u);
    let c = n * plane.dist;
    vec![c - u * BIG - v * BIG, c + u * BIG - v * BIG, c + u * BIG + v * BIG, c - u * BIG + v * BIG]
}

/// Keep the part of a convex polygon behind (or on) the plane.
pub fn clip_polygon_back(poly: &[Vec3], plane: &Plane) -> Polygon {
    if poly.is_empty() {
        return Vec::new();
    }
    let d: Vec<f64> = poly.iter().map(|p| plane.distance(*p)).collect();
    if d.iter().all(|x| *x <= PLANE_EPS) {
        return poly.to_vec();
    }
    if d.iter().all(|x| *x >= -PLANE_EPS) {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let (a, b, da, db) = (poly[i], poly[j], d[i], d[j]);
        if da <= PLANE_EPS {
            out.push(a);
        }
        if (da > PLANE_EPS && db < -PLANE_EPS) || (da < -PLANE_EPS && db > PLANE_EPS) {
            let t = da / (da - db);
            out.push(a + (b - a) * t);
        }
    }
    dedupe_ring(out)
}

fn dedupe_ring(mut pts: Polygon) -> Polygon {
    pts.dedup_by(|a, b| a.approx_eq(*b, POINT_EPS));
    while pts.len() > 1 && pts[0].approx_eq(*pts.last().unwrap(), POINT_EPS) {
        pts.pop();
    }
    pts
}

/// Area of a planar polygon.
pub fn polygon_area(poly: &[Vec3]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut s = Vec3::ZERO;
    for i in 1..poly.len() - 1 {
        s += (poly[i] - poly[0]).cross(poly[i + 1] - poly[0]);
    }
    s.len() * 0.5
}

pub fn polygon_center(poly: &[Vec3]) -> Vec3 {
    if poly.is_empty() {
        return Vec3::ZERO;
    }
    poly.iter().fold(Vec3::ZERO, |a, p| a + *p) / poly.len() as f64
}

/// Polygon for each plane of a plane set (empty when that plane doesn't touch the solid).
pub fn planes_polygons(planes: &[Plane]) -> Vec<Polygon> {
    planes
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut poly = base_polygon(p);
            for (j, q) in planes.iter().enumerate() {
                if i == j || poly.is_empty() {
                    continue;
                }
                // An identical plane would clip the face away entirely; skip exact duplicates
                // here and report them in `validate`.
                if q.approx_eq(p) {
                    if j < i {
                        poly.clear();
                    }
                    continue;
                }
                poly = clip_polygon_back(&poly, q);
            }
            if polygon_area(&poly) < 1e-6 {
                poly.clear();
            }
            poly
        })
        .collect()
}

/// Planes of a brush; None if any face's points are in a line.
pub fn brush_planes(b: &Brush) -> Option<Vec<Plane>> {
    b.faces.iter().map(Face::plane).collect()
}

/// Face polygons of a brush, one per face (empty for a face that doesn't touch the solid or
/// whose points are in a line).
pub fn brush_polygons(b: &Brush) -> Vec<Polygon> {
    let planes: Vec<Option<Plane>> = b.faces.iter().map(Face::plane).collect();
    let valid: Vec<Plane> = planes.iter().flatten().copied().collect();
    let polys = planes_polygons(&valid);
    let mut it = polys.into_iter();
    planes.iter().map(|p| if p.is_some() { it.next().unwrap_or_default() } else { Vec::new() }).collect()
}

/// Unique vertices of a brush.
pub fn brush_vertices(b: &Brush) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::new();
    for poly in brush_polygons(b) {
        for p in poly {
            if !out.iter().any(|q| q.approx_eq(p, POINT_EPS)) {
                out.push(p);
            }
        }
    }
    out
}

pub fn brush_bounds(b: &Brush) -> Bounds {
    let mut bb = Bounds::empty();
    for poly in brush_polygons(b) {
        for p in poly {
            bb.add(p);
        }
    }
    bb
}

/// Volume of the closed solid made by the polygons (0 when open or empty).
pub fn polygons_volume(polys: &[Polygon]) -> f64 {
    let all: Vec<Vec3> = polys.iter().flatten().copied().collect();
    if all.is_empty() {
        return 0.0;
    }
    let c = polygon_center(&all);
    let mut vol = 0.0;
    for poly in polys {
        for i in 1..poly.len().saturating_sub(1) {
            vol += (poly[0] - c).dot((poly[i] - c).cross(poly[i + 1] - c)) / 6.0;
        }
    }
    vol.max(0.0)
}

pub fn brush_volume(b: &Brush) -> f64 {
    polygons_volume(&brush_polygons(b))
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrushProblem {
    TooFewFaces,
    /// The face's three points are in a line, so it has no plane.
    BadPoints(usize),
    /// Two faces share a plane.
    DuplicatePlane(usize, usize),
    /// The face doesn't touch the solid (another face hides it), or is a sliver.
    FaceNotOnSolid(usize),
    /// The planes don't enclose a finite volume.
    Open,
    /// Nothing is left: the planes enclose no space at all.
    Empty,
    NotFinite,
}

impl BrushProblem {
    pub fn message(&self) -> String {
        match self {
            BrushProblem::TooFewFaces => "A brush needs at least 4 faces.".into(),
            BrushProblem::BadPoints(i) => format!("Face {} has its three points in a line.", i + 1),
            BrushProblem::DuplicatePlane(a, b) => format!("Faces {} and {} lie on the same plane.", a + 1, b + 1),
            BrushProblem::FaceNotOnSolid(i) => format!("Face {} doesn't touch the brush (it's hidden by other faces).", i + 1),
            BrushProblem::Open => "The faces don't close the brush.".into(),
            BrushProblem::Empty => "The faces leave no space inside.".into(),
            BrushProblem::NotFinite => "The brush has coordinates that aren't numbers.".into(),
        }
    }
}

/// Everything wrong with a brush (empty = valid: convex by construction, closed, every face a
/// real polygon).
pub fn validate(b: &Brush) -> Vec<BrushProblem> {
    let mut out = Vec::new();
    if b.faces.iter().any(|f| f.points.iter().any(|p| !p.is_finite())) {
        return vec![BrushProblem::NotFinite];
    }
    if b.faces.len() < 4 {
        out.push(BrushProblem::TooFewFaces);
    }
    let planes: Vec<Option<Plane>> = b.faces.iter().map(Face::plane).collect();
    for (i, p) in planes.iter().enumerate() {
        if p.is_none() {
            out.push(BrushProblem::BadPoints(i));
        }
    }
    for i in 0..planes.len() {
        for j in i + 1..planes.len() {
            if let (Some(a), Some(c)) = (planes[i], planes[j]) {
                if a.approx_eq(&c) {
                    out.push(BrushProblem::DuplicatePlane(i, j));
                }
            }
        }
    }
    let polys = brush_polygons(b);
    if polys.iter().all(|p| p.is_empty()) {
        out.push(BrushProblem::Empty);
        return out;
    }
    let limit = BIG * 0.5;
    if polys.iter().flatten().any(|p| p.abs().max_elem() > limit) {
        out.push(BrushProblem::Open);
    }
    for (i, poly) in polys.iter().enumerate() {
        if planes[i].is_some() && poly.is_empty() && !out.iter().any(|p| matches!(p, BrushProblem::DuplicatePlane(_, j) if *j == i)) {
            out.push(BrushProblem::FaceNotOnSolid(i));
        }
    }
    out
}

pub fn is_valid(b: &Brush) -> bool {
    validate(b).is_empty()
}

/// Remove faces that don't touch the solid (left over after push/pull or clipping).
/// Returns how many were removed.
pub fn drop_unused_faces(b: &mut Brush) -> usize {
    let polys = brush_polygons(b);
    let before = b.faces.len();
    let mut i = 0;
    b.faces.retain(|_| {
        let keep = !polys[i].is_empty();
        i += 1;
        keep
    });
    before - b.faces.len()
}

/// Three well-spread points of a polygon, in .map order (clockwise from outside).
pub fn map_points_for_polygon(poly: &[Vec3]) -> Option<[Vec3; 3]> {
    if poly.len() < 3 {
        return None;
    }
    let a = poly[0];
    // Farthest vertex from a, then the one giving the biggest triangle.
    let (bi, _) = poly.iter().enumerate().skip(1).map(|(i, p)| (i, (*p - a).len())).fold((1, -1.0), |m, x| if x.1 > m.1 { x } else { m });
    let b = poly[bi];
    let mut best = (0usize, -1.0);
    for (i, p) in poly.iter().enumerate() {
        let area = (b - a).cross(*p - a).len();
        if i != 0 && i != bi && area > best.1 {
            best = (i, area);
        }
    }
    if best.1 <= 1e-9 {
        return None;
    }
    let c = poly[best.0];
    // a, b, c follow the polygon's ccw order only if bi < best.0; .map wants the reverse.
    if bi < best.0 {
        Some([a, c, b])
    } else {
        Some([a, b, c])
    }
}

/// Three points on a plane, in .map order, for a face we only know as a plane (axis-aligned
/// planes at whole-unit distances get whole-unit points).
pub fn map_points_for_plane(plane: &Plane) -> [Vec3; 3] {
    let poly = base_polygon(&Plane { normal: plane.normal, dist: plane.dist });
    let c = polygon_center(&poly);
    let scale = 64.0 / BIG;
    let small: Vec<Vec3> = poly.iter().map(|p| c + (*p - c) * scale).collect();
    map_points_for_polygon(&small).unwrap_or([c, c + Vec3::X, c + Vec3::Y])
}

/// The planes of the convex hull of a point set (outward normals). Fine for brush-sized sets
/// (a few hundred points).
pub fn convex_hull_planes(points: &[Vec3]) -> Vec<Plane> {
    let mut pts: Vec<Vec3> = Vec::new();
    for p in points {
        if !pts.iter().any(|q| q.approx_eq(*p, POINT_EPS)) {
            pts.push(*p);
        }
    }
    let n = pts.len();
    let mut planes: Vec<Plane> = Vec::new();
    if n < 4 {
        return planes;
    }
    let tol = POINT_EPS;
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let nrm = (pts[j] - pts[i]).cross(pts[k] - pts[i]);
                if nrm.len() < 1e-6 {
                    continue;
                }
                let nrm = nrm.normalized();
                let pl = Plane::from_point_normal(pts[i], nrm);
                if planes.iter().any(|q| q.approx_eq(&pl) || q.approx_eq(&pl.flipped())) {
                    continue;
                }
                let (mut front, mut back) = (false, false);
                for p in &pts {
                    match pl.side(*p, tol) {
                        Side::Front => front = true,
                        Side::Back => back = true,
                        Side::On => {}
                    }
                    if front && back {
                        break;
                    }
                }
                if front && back {
                    continue;
                }
                if !front && back {
                    planes.push(pl);
                } else if front && !back {
                    planes.push(pl.flipped());
                }
            }
        }
    }
    planes
}

/// Axis-aligned box planes (outward).
pub fn box_planes(min: Vec3, max: Vec3) -> Vec<Plane> {
    vec![
        Plane::new(v3(0.0, 0.0, 1.0), max.z),
        Plane::new(v3(0.0, 0.0, -1.0), -min.z),
        Plane::new(v3(1.0, 0.0, 0.0), max.x),
        Plane::new(v3(-1.0, 0.0, 0.0), -min.x),
        Plane::new(v3(0.0, 1.0, 0.0), max.y),
        Plane::new(v3(0.0, -1.0, 0.0), -min.y),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ops::make_box;

    #[test]
    fn box_polygons_and_measures() {
        let b = make_box(v3(0.0, 0.0, 0.0), v3(64.0, 32.0, 16.0), "AAATRIGGER");
        assert!(is_valid(&b), "{:?}", validate(&b));
        let polys = brush_polygons(&b);
        assert_eq!(polys.len(), 6);
        for (poly, f) in polys.iter().zip(&b.faces) {
            assert_eq!(poly.len(), 4);
            // Counter-clockwise from outside: the polygon's own normal matches the plane's.
            let n = (poly[1] - poly[0]).cross(poly[2] - poly[0]).normalized();
            assert!(n.approx_eq(f.plane().unwrap().normal, 1e-9));
        }
        assert!((brush_volume(&b) - 64.0 * 32.0 * 16.0).abs() < 1e-6);
        let bb = brush_bounds(&b);
        assert!(bb.min.approx_eq(Vec3::ZERO, 1e-9) && bb.max.approx_eq(v3(64.0, 32.0, 16.0), 1e-9));
        assert_eq!(brush_vertices(&b).len(), 8);
    }

    #[test]
    fn detects_problems() {
        let mut b = make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "X");
        let mut open = b.clone();
        open.faces.remove(0);
        assert!(validate(&open).contains(&BrushProblem::Open));
        let mut dup = b.clone();
        dup.faces.push(b.faces[0].clone());
        assert!(validate(&dup).iter().any(|p| matches!(p, BrushProblem::DuplicatePlane(0, 6))));
        let mut hidden = b.clone();
        // A plane well outside the box never touches it.
        let mut f = b.faces[0].clone();
        for p in &mut f.points {
            p.z += 100.0;
        }
        hidden.faces.push(f);
        assert!(validate(&hidden).contains(&BrushProblem::FaceNotOnSolid(6)));
        assert_eq!(drop_unused_faces(&mut hidden), 1);
        assert!(is_valid(&hidden));
        b.faces[2].points = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
        assert!(validate(&b).contains(&BrushProblem::BadPoints(2)));
        let mut empty = make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "X");
        let mut f = empty.faces[0].clone(); // top, normal +z at z=64
        for p in &mut f.points {
            p.z -= 200.0; // keep only z <= -136, which the bottom face excludes
        }
        empty.faces[0] = f;
        assert!(validate(&empty).contains(&BrushProblem::Empty));
    }

    #[test]
    fn hull_of_cube_points() {
        let mut pts = Vec::new();
        for x in [0.0, 1.0] {
            for y in [0.0, 1.0] {
                for z in [0.0, 1.0] {
                    pts.push(v3(x, y, z) * 32.0);
                }
            }
        }
        pts.push(v3(16.0, 16.0, 16.0)); // inside, ignored
        let planes = convex_hull_planes(&pts);
        assert_eq!(planes.len(), 6);
        let polys = planes_polygons(&planes);
        assert!((polygons_volume(&polys) - 32768.0).abs() < 1e-6);
    }

    #[test]
    fn map_points_round_trip_plane() {
        for pl in box_planes(v3(-8.0, -8.0, -8.0), v3(8.0, 8.0, 8.0)) {
            let pts = map_points_for_plane(&pl);
            assert!(Plane::from_map_points(&pts).unwrap().approx_eq(&pl));
        }
        let tilted = Plane::from_point_normal(v3(3.0, 4.0, 5.0), v3(1.0, 2.0, 3.0).normalized());
        assert!(Plane::from_map_points(&map_points_for_plane(&tilted)).unwrap().approx_eq(&tilted));
    }
}
