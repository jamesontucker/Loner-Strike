//! Brush operations: create (box, cylinder), move/rotate/scale, push/pull a face, clip, hollow,
//! carve, merge, and snapping. Each returns new brushes and never edits in place, so the
//! history layer can keep the before and after.

use super::doc::{Brush, Face, TexAlign};
use super::geom::{
    box_planes, brush_planes, brush_polygons, brush_vertices, brush_volume, convex_hull_planes, drop_unused_faces,
    map_points_for_plane, map_points_for_polygon, planes_polygons, validate, Polygon,
};
use super::math::{v3, Affine, Mat3, Plane, Vec3, PLANE_EPS};
use super::texture::{locked, unlocked, world_aligned};

pub type OpResult<T> = Result<T, String>;

/// Smallest volume (cubic units) a piece needs to count as a brush.
const MIN_VOLUME: f64 = 1e-3;

/// A face on `plane` with a world-aligned texture. Uses the polygon for its three points when
/// known (better precision near the brush).
pub fn face_on_plane(plane: &Plane, poly: Option<&[Vec3]>, texture: &str) -> Face {
    face_with_align(plane, poly, texture, world_aligned(plane.normal))
}

fn face_with_align(plane: &Plane, poly: Option<&[Vec3]>, texture: &str, align: TexAlign) -> Face {
    let mut points = poly.and_then(map_points_for_polygon).unwrap_or_else(|| map_points_for_plane(plane));
    for p in &mut points {
        *p = tidy(*p);
    }
    Face { points, texture: texture.to_string(), align }
}

/// Snap coordinates that are a rounding error away from a 1/1024 step (plane intersections
/// give 31.999999 for 32, moves 192.00000000000006 for 192), so files stay clean.
pub fn tidy(p: Vec3) -> Vec3 {
    let t = |x: f64| {
        let r = (x * 1024.0).round() / 1024.0;
        if (x - r).abs() < 1e-6 { r + 0.0 } else { x }
    };
    v3(t(p.x), t(p.y), t(p.z))
}

fn check(b: Brush) -> OpResult<Brush> {
    match validate(&b).first() {
        None => Ok(b),
        Some(p) => Err(p.message()),
    }
}

/// Build a brush from planes; `style` gives each new face its texture and alignment.
/// Planes that end up not touching the solid are dropped.
pub fn brush_from_planes(planes: &[Plane], mut style: impl FnMut(&Plane) -> (String, TexAlign)) -> OpResult<Brush> {
    let polys = planes_polygons(planes);
    let mut faces = Vec::new();
    for (pl, poly) in planes.iter().zip(&polys) {
        if poly.is_empty() {
            continue;
        }
        let (tex, align) = style(pl);
        faces.push(face_with_align(pl, Some(poly), &tex, align));
    }
    check(Brush { faces, entity: None, group: None })
}

/// An axis-aligned box between two corners.
pub fn make_box(a: Vec3, b: Vec3, texture: &str) -> Brush {
    let (min, max) = (a.min(b), a.max(b));
    let faces = box_planes(min, max).iter().map(|p| face_on_plane(p, None, texture)).collect();
    Brush { faces, entity: None, group: None }
}

/// A cylinder standing on `base` (centre of the bottom) along +z, with `sides` sides
/// (3..=64). Vertices are rounded to `grid` when that keeps the shape valid (Hammer-style
/// whole-unit points; pass 0 to keep them exact).
pub fn make_cylinder(base: Vec3, radius: f64, height: f64, sides: usize, grid: f64, texture: &str) -> OpResult<Brush> {
    if !(3..=64).contains(&sides) {
        return Err("A cylinder needs between 3 and 64 sides.".into());
    }
    if !(radius > 0.5) || !(height > 0.5) {
        return Err("The cylinder is too small.".into());
    }
    let ring = |snap: f64| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / sides as f64;
                let p = base + v3(a.cos() * radius, a.sin() * radius, 0.0);
                if snap > 0.0 {
                    let s = p.snapped(snap);
                    v3(s.x, s.y, base.z)
                } else {
                    p
                }
            })
            .collect()
    };
    let build = |pts: Vec<Vec3>| -> OpResult<Brush> {
        let up = v3(0.0, 0.0, height);
        let mut planes = vec![Plane::new(Vec3::Z, base.z + height), Plane::new(-Vec3::Z, -base.z)];
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if a.approx_eq(b, 1e-6) {
                return Err("Rounding merged two corners.".into());
            }
            // Counter-clockwise ring seen from above: outward normal is (b - a) x up.
            let n = (b - a).cross(up).normalized();
            planes.push(Plane::from_point_normal(a, n));
        }
        let b = brush_from_planes(&planes, |p| (texture.to_string(), world_aligned(p.normal)))?;
        if b.faces.len() != sides + 2 {
            return Err("Rounding bent the cylinder.".into());
        }
        Ok(b)
    };
    if grid > 0.0 {
        if let Ok(b) = build(ring(grid)) {
            return Ok(b);
        }
    }
    build(ring(0.0))
}

/// Move/rotate/scale/mirror a brush. With `lock` the textures stay stuck to the brush.
pub fn transform(b: &Brush, t: &Affine, lock: bool) -> OpResult<Brush> {
    if t.m.inverse().is_none() {
        return Err("That would flatten the brush.".into());
    }
    let mirror = t.m.det() < 0.0;
    let mut out = b.clone();
    for f in &mut out.faces {
        for p in &mut f.points {
            *p = t.apply(*p);
        }
        if mirror {
            f.points.swap(1, 2);
        }
        let Some(plane) = f.plane() else { return Err("A face collapsed.".into()) };
        f.align = if lock { locked(&f.align, t) } else { unlocked(&f.align, &plane) };
    }
    check(out)
}

pub fn translate(b: &Brush, d: Vec3, lock: bool) -> OpResult<Brush> {
    transform(b, &Affine::translate(d), lock)
}

pub fn rotate(b: &Brush, axis: Vec3, degrees: f64, pivot: Vec3, lock: bool) -> OpResult<Brush> {
    transform(b, &Affine::around(Mat3::rotation(axis, degrees), pivot), lock)
}

pub fn scale(b: &Brush, factors: Vec3, pivot: Vec3, lock: bool) -> OpResult<Brush> {
    transform(b, &Affine::around(Mat3::scale(factors), pivot), lock)
}

/// Push/pull: slide one face along its normal (positive = outwards). Faces that the move
/// hides are removed; the moved face itself must survive.
pub fn move_face(b: &Brush, face: usize, distance: f64, lock: bool) -> OpResult<Brush> {
    let f = b.faces.get(face).ok_or("That face doesn't exist.")?;
    let plane = f.plane().ok_or("That face is broken.")?;
    let d = plane.normal * distance;
    let mut out = b.clone();
    let mf = &mut out.faces[face];
    for p in &mut mf.points {
        *p = tidy(*p + d);
    }
    if lock {
        mf.align = locked(&mf.align, &Affine::translate(d));
    }
    let moved = out.faces[face].clone();
    let polys = brush_polygons(&out);
    if polys[face].is_empty() {
        return Err("The face can't go that far: the brush would turn inside out.".into());
    }
    drop_unused_faces(&mut out);
    if !out.faces.contains(&moved) {
        return Err("The face can't go that far.".into());
    }
    check(out)
}

/// Move a face so it passes through a point (dragging a face to where the mouse is).
pub fn move_face_to(b: &Brush, face: usize, through: Vec3, lock: bool) -> OpResult<Brush> {
    let plane = b.faces.get(face).and_then(Face::plane).ok_or("That face doesn't exist.")?;
    move_face(b, face, plane.distance(through), lock)
}

/// Split by a plane. Returns (the part behind the plane, the part in front); either may be
/// None when the plane misses the brush. New faces get `texture`, world aligned.
pub fn clip(b: &Brush, plane: &Plane, texture: &str) -> (Option<Brush>, Option<Brush>) {
    let piece = |pl: Plane| -> Option<Brush> {
        let mut nb = b.clone();
        nb.faces.push(face_on_plane(&pl, None, texture));
        let polys = brush_polygons(&nb);
        if polys.last().map_or(true, |p| p.is_empty()) {
            // The plane doesn't cut: the brush is entirely on one side.
            let v = brush_vertices(b);
            let inside = !v.is_empty() && v.iter().all(|p| pl.distance(*p) <= PLANE_EPS);
            return inside.then(|| b.clone());
        }
        // Re-pick the new face's points from its polygon for precision.
        let last = nb.faces.len() - 1;
        if let Some(pts) = map_points_for_polygon(&polys[last]) {
            nb.faces[last].points = pts;
        }
        drop_unused_faces(&mut nb);
        (brush_volume(&nb) > MIN_VOLUME && validate(&nb).is_empty()).then_some(nb)
    };
    (piece(*plane), piece(plane.flipped()))
}

/// The overlap of two brushes (None when they don't overlap). Faces come from `a` first.
pub fn intersect(a: &Brush, b: &Brush) -> Option<Brush> {
    let mut nb = a.clone();
    nb.faces.extend(b.faces.iter().cloned());
    let polys = brush_polygons(&nb);
    if polys.iter().all(|p| p.is_empty()) {
        return None;
    }
    drop_unused_faces(&mut nb);
    dedupe_faces(&mut nb);
    (brush_volume(&nb) > MIN_VOLUME && validate(&nb).is_empty()).then_some(nb)
}

fn dedupe_faces(b: &mut Brush) {
    let mut seen: Vec<Plane> = Vec::new();
    b.faces.retain(|f| match f.plane() {
        Some(p) if !seen.iter().any(|q| q.approx_eq(&p)) => {
            seen.push(p);
            true
        }
        _ => false,
    });
}

/// Carve: `a` minus `cutter`, as convex pieces. Faces cut by the cutter take the cutter's
/// texture. Returns `a` unchanged when they don't overlap, and nothing when `a` is inside.
pub fn subtract(a: &Brush, cutter: &Brush) -> Vec<Brush> {
    if intersect(a, cutter).is_none() {
        return vec![a.clone()];
    }
    let mut pieces = Vec::new();
    let mut rest = a.clone();
    for cf in &cutter.faces {
        let Some(pl) = cf.plane() else { continue };
        let (back, front) = split_keep_tex(&rest, &pl, cf);
        if let Some(f) = front {
            pieces.push(f);
        }
        match back {
            Some(bk) => rest = bk,
            None => return pieces,
        }
    }
    // What's left of `rest` is inside the cutter: drop it.
    pieces
}

fn split_keep_tex(b: &Brush, pl: &Plane, cutter_face: &Face) -> (Option<Brush>, Option<Brush>) {
    let (back, front) = clip(b, pl, &cutter_face.texture);
    // Faces added by clip are world aligned; give them the cutter face's alignment instead.
    let fix = |nb: Option<Brush>| {
        nb.map(|mut nb| {
            for f in &mut nb.faces {
                if let Some(p) = f.plane() {
                    if (p.approx_eq(pl) || p.approx_eq(&pl.flipped())) && !b.faces.iter().any(|o| o.plane().is_some_and(|q| q.approx_eq(&p))) {
                        f.align = cutter_face.align.clone();
                    }
                }
            }
            nb
        })
    };
    (fix(back), fix(front))
}

/// Hollow: replace the brush with walls `thickness` thick (positive = walls go inwards).
pub fn hollow(b: &Brush, thickness: f64) -> OpResult<Vec<Brush>> {
    if !(thickness > 0.0) {
        return Err("Wall thickness must be more than 0.".into());
    }
    let mut inner = b.clone();
    for f in &mut inner.faces {
        let Some(pl) = f.plane() else { return Err("The brush has a broken face.".into()) };
        let d = -pl.normal * thickness;
        for p in &mut f.points {
            *p += d;
        }
    }
    if brush_polygons(&inner).iter().any(|p| p.is_empty()) || brush_volume(&inner) <= MIN_VOLUME || !validate(&inner).is_empty() {
        return Err("The walls are too thick for this brush.".into());
    }
    let walls = subtract(b, &inner);
    if walls.is_empty() {
        return Err("Hollowing left nothing.".into());
    }
    Ok(walls)
}

/// Pick the source face that best matches a plane, for faces of a rebuilt brush.
fn style_from(sources: &[(Plane, Face)], pl: &Plane) -> (String, TexAlign) {
    let mut best: Option<(&Plane, &Face, f64)> = None;
    for (sp, f) in sources {
        let score = sp.normal.dot(pl.normal) - (sp.dist - pl.dist).abs() * 1e-6;
        if best.map_or(true, |b| score > b.2) {
            best = Some((sp, f, score));
        }
    }
    match best {
        Some((sp, f, _)) if sp.normal.dot(pl.normal) > 0.999 => (f.texture.clone(), f.align.clone()),
        Some((_, f, _)) => {
            let w = world_aligned(pl.normal);
            (f.texture.clone(), TexAlign { scale_u: f.align.scale_u, scale_v: f.align.scale_v, ..w })
        }
        None => ("AAATRIGGER".into(), world_aligned(pl.normal)),
    }
}

fn sources_of<'a>(brushes: impl IntoIterator<Item = &'a Brush>) -> Vec<(Plane, Face)> {
    brushes.into_iter().flat_map(|b| b.faces.iter().filter_map(|f| f.plane().map(|p| (p, f.clone())))).collect()
}

/// Merge into one convex brush (the convex hull of all of them). `exact` is false when the
/// hull fills space the brushes didn't (they weren't a convex shape together).
pub struct Merged {
    pub brush: Brush,
    pub exact: bool,
}

pub fn merge(brushes: &[Brush]) -> OpResult<Merged> {
    if brushes.len() < 2 {
        return Err("Pick at least two brushes to merge.".into());
    }
    let pts: Vec<Vec3> = brushes.iter().flat_map(brush_vertices).collect();
    let planes = convex_hull_planes(&pts);
    let src = sources_of(brushes);
    let mut brush = brush_from_planes(&planes, |pl| style_from(&src, pl))?;
    brush.entity = brushes[0].entity;
    brush.group = brushes[0].group;
    // Union volume: each brush minus the ones before it.
    let mut union = 0.0;
    for (i, b) in brushes.iter().enumerate() {
        let mut frags = vec![b.clone()];
        for prev in &brushes[..i] {
            frags = frags.iter().flat_map(|f| subtract(f, prev)).collect();
        }
        union += frags.iter().map(brush_volume).sum::<f64>();
    }
    let hull = brush_volume(&brush);
    let exact = (hull - union).abs() <= 1e-3 * hull.max(1.0);
    Ok(Merged { brush, exact })
}

/// Round each face's three plane points to the grid (what Hammer does on load). Faces keep
/// their texture; fails if that breaks the brush.
pub fn snap_plane_points(b: &Brush, grid: f64) -> OpResult<Brush> {
    let mut out = b.clone();
    for f in &mut out.faces {
        for p in &mut f.points {
            *p = p.snapped(grid);
        }
    }
    check(out).map_err(|e| format!("Snapping to the {grid} grid breaks this brush: {e}"))
}

/// Round every corner to the grid and rebuild the brush around them (vertex snapping).
pub fn snap_vertices(b: &Brush, grid: f64) -> OpResult<Brush> {
    let pts: Vec<Vec3> = brush_vertices(b).into_iter().map(|p| p.snapped(grid)).collect();
    let planes = convex_hull_planes(&pts);
    let src = sources_of([b]);
    let mut out = brush_from_planes(&planes, |pl| style_from(&src, pl))
        .map_err(|e| format!("Snapping to the {grid} grid flattens this brush: {e}"))?;
    out.entity = b.entity;
    out.group = b.group;
    Ok(out)
}

/// Polygons with texture coordinates, for drawing and picking. `size` gives a texture's
/// pixel size by name.
pub struct FaceMesh {
    pub face: usize,
    pub texture: String,
    pub normal: Vec3,
    pub vertices: Polygon,
    /// Normalised texture coordinates (0..1 per repeat), one per vertex.
    pub uvs: Vec<(f64, f64)>,
}

pub fn brush_mesh(b: &Brush, size: impl Fn(&str) -> (f64, f64)) -> Vec<FaceMesh> {
    let planes = brush_planes(b);
    brush_polygons(b)
        .into_iter()
        .enumerate()
        .filter(|(_, p)| !p.is_empty())
        .map(|(i, poly)| {
            let f = &b.faces[i];
            let (w, h) = size(&f.texture);
            let uvs = poly.iter().map(|p| {
                let (u, v) = f.align.uv(*p);
                (u / w, v / h)
            });
            FaceMesh {
                face: i,
                texture: f.texture.clone(),
                normal: planes.as_ref().map(|p| p[i].normal).unwrap_or_default(),
                uvs: uvs.collect(),
                vertices: poly,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::{brush_bounds, is_valid};

    fn cube() -> Brush {
        make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "CRATE")
    }

    #[test]
    fn cylinder() {
        let c = make_cylinder(v3(0.0, 0.0, 0.0), 32.0, 64.0, 8, 1.0, "PIPE").unwrap();
        assert_eq!(c.faces.len(), 10);
        assert!(is_valid(&c));
        assert!(c.faces.iter().all(|f| f.points.iter().all(|p| p.x.fract() == 0.0 && p.y.fract() == 0.0)));
        let exact = make_cylinder(v3(0.0, 0.0, 0.0), 32.0, 64.0, 24, 0.0, "PIPE").unwrap();
        let ideal = std::f64::consts::PI * 32.0 * 32.0 * 64.0;
        assert!((brush_volume(&exact) - ideal).abs() / ideal < 0.02);
        assert!(make_cylinder(Vec3::ZERO, 32.0, 64.0, 2, 1.0, "X").is_err());
    }

    #[test]
    fn transforms() {
        let b = cube();
        let m = translate(&b, v3(16.0, 0.0, -8.0), true).unwrap();
        let bb = brush_bounds(&m);
        assert!(bb.min.approx_eq(v3(16.0, 0.0, -8.0), 1e-9));
        let r = rotate(&b, Vec3::Z, 90.0, Vec3::ZERO, true).unwrap();
        assert!(brush_bounds(&r).min.approx_eq(v3(-64.0, 0.0, 0.0), 1e-9));
        let s = scale(&b, v3(2.0, 1.0, 0.5), Vec3::ZERO, false).unwrap();
        assert!(brush_bounds(&s).max.approx_eq(v3(128.0, 64.0, 32.0), 1e-9));
        let mirrored = scale(&b, v3(-1.0, 1.0, 1.0), Vec3::ZERO, true).unwrap();
        assert!(is_valid(&mirrored));
        assert!(brush_bounds(&mirrored).min.approx_eq(v3(-64.0, 0.0, 0.0), 1e-9));
        assert!(scale(&b, v3(0.0, 1.0, 1.0), Vec3::ZERO, true).is_err());
        // Texture lock while moving: the point that was at texture (0,0) still is.
        let p = v3(0.0, 0.0, 64.0);
        assert!(m.faces[0].align.uv(p + v3(16.0, 0.0, -8.0)).0.abs() < 1e-9);
    }

    #[test]
    fn push_pull() {
        let b = cube();
        let top = 0; // make_box puts +z first
        let up = move_face(&b, top, 32.0, false).unwrap();
        assert!((brush_bounds(&up).max.z - 96.0).abs() < 1e-9);
        let down = move_face(&b, top, -32.0, false).unwrap();
        assert!((brush_volume(&down) - 64.0 * 64.0 * 32.0).abs() < 1e-6);
        assert!(move_face(&b, top, -64.0, false).is_err());
        assert!(move_face(&b, top, -100.0, false).is_err());
        let to = move_face_to(&b, top, v3(5.0, 5.0, 80.0), false).unwrap();
        assert!((brush_bounds(&to).max.z - 80.0).abs() < 1e-9);
        // float noise from the move is snapped away (0.1 * 3 is 0.30000000000000004)
        let noisy = move_face(&b, top, 0.1 + 0.1 + 0.1 + 127.7, false).unwrap();
        assert!(noisy.faces[top].points.iter().all(|p| p.z == 192.0));
        assert_eq!(tidy(v3(0.5 + 1e-9, 31.999_999_9, 1.0 / 3.0)), v3(0.5, 32.0, 1.0 / 3.0));
        // A wedge: pushing the slope in far enough hides the top face, which is dropped.
        let (wedge, _) = clip(&b, &Plane::from_point_normal(v3(32.0, 0.0, 64.0), v3(1.0, 0.0, 1.0).normalized()), "X");
        let wedge = wedge.unwrap();
        assert_eq!(wedge.faces.len(), 7);
        let slope = wedge.faces.len() - 1;
        let pushed = move_face(&wedge, slope, -30.0, false).unwrap();
        assert_eq!(pushed.faces.len(), 5);
        assert!(is_valid(&pushed));
    }

    #[test]
    fn clipping() {
        let b = cube();
        let pl = Plane::new(Vec3::X, 16.0);
        let (back, front) = clip(&b, &pl, "CLIP");
        let (back, front) = (back.unwrap(), front.unwrap());
        assert!((brush_volume(&back) - 16.0 * 64.0 * 64.0).abs() < 1e-6);
        assert!((brush_volume(&front) - 48.0 * 64.0 * 64.0).abs() < 1e-6);
        assert!(back.faces.iter().any(|f| f.texture == "CLIP"));
        let (all, none) = clip(&b, &Plane::new(Vec3::X, 100.0), "CLIP");
        assert_eq!(all.unwrap(), b);
        assert!(none.is_none());
        // Diagonal clip leaves a 5-face wedge.
        let (w, _) = clip(&b, &Plane::from_point_normal(v3(32.0, 32.0, 0.0), v3(1.0, 1.0, 0.0).normalized()), "X");
        assert_eq!(w.unwrap().faces.len(), 5);
    }

    #[test]
    fn carve_and_hollow() {
        let a = cube();
        let cutter = make_box(v3(16.0, 16.0, -10.0), v3(48.0, 48.0, 100.0), "HOLE");
        let pieces = subtract(&a, &cutter);
        assert_eq!(pieces.len(), 4);
        let vol: f64 = pieces.iter().map(brush_volume).sum();
        assert!((vol - (64.0 * 64.0 * 64.0 - 32.0 * 32.0 * 64.0)).abs() < 1e-3);
        assert!(pieces.iter().all(is_valid));
        assert!(pieces.iter().any(|p| p.faces.iter().any(|f| f.texture == "HOLE")));
        let away = make_box(v3(100.0, 0.0, 0.0), v3(120.0, 10.0, 10.0), "X");
        assert_eq!(subtract(&a, &away), vec![a.clone()]);
        let around = make_box(v3(-1.0, -1.0, -1.0), v3(65.0, 65.0, 65.0), "X");
        assert!(subtract(&a, &around).is_empty());

        let walls = hollow(&a, 8.0).unwrap();
        assert_eq!(walls.len(), 6);
        let vol: f64 = walls.iter().map(brush_volume).sum();
        assert!((vol - (64f64.powi(3) - 48f64.powi(3))).abs() < 1e-3);
        assert!(hollow(&a, 32.0).is_err());
        assert!(hollow(&a, 0.0).is_err());
    }

    #[test]
    fn merging() {
        let a = make_box(v3(0.0, 0.0, 0.0), v3(32.0, 64.0, 64.0), "A");
        let b = make_box(v3(32.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "B");
        let m = merge(&[a.clone(), b]).unwrap();
        assert!(m.exact);
        assert_eq!(m.brush.faces.len(), 6);
        assert!((brush_volume(&m.brush) - 64f64.powi(3)).abs() < 1e-6);
        let c = make_box(v3(32.0, 32.0, 0.0), v3(64.0, 64.0, 64.0), "C");
        let l = merge(&[a.clone(), c]).unwrap();
        assert!(!l.exact);
        assert!(merge(&[a]).is_err());
        // Big point sets (two 32-sided cylinders) stay quick enough.
        let c1 = make_cylinder(v3(0.0, 0.0, 0.0), 64.0, 64.0, 32, 0.0, "P").unwrap();
        let c2 = make_cylinder(v3(0.0, 0.0, 64.0), 64.0, 64.0, 32, 0.0, "P").unwrap();
        let t = std::time::Instant::now();
        let m = merge(&[c1, c2]).unwrap();
        assert!(m.exact);
        assert_eq!(m.brush.faces.len(), 34);
        assert!(t.elapsed().as_secs_f64() < 10.0);
    }

    #[test]
    fn snapping() {
        let mut b = cube();
        for f in &mut b.faces {
            for p in &mut f.points {
                *p += v3(0.3, -0.2, 0.1);
            }
        }
        let s = snap_plane_points(&b, 1.0).unwrap();
        assert!(s.faces.iter().all(|f| f.points.iter().all(|p| *p == p.snapped(1.0))));
        let v = snap_vertices(&b, 16.0).unwrap();
        assert!(brush_vertices(&v).iter().all(|p| *p == p.snapped(16.0)));
        assert_eq!(v.faces[0].texture, "CRATE");
        let tiny = make_box(v3(0.0, 0.0, 0.0), v3(4.0, 4.0, 4.0), "X");
        assert!(snap_vertices(&tiny, 16.0).is_err());
    }

    #[test]
    fn mesh_uvs() {
        let b = cube();
        let mesh = brush_mesh(&b, |_| (64.0, 64.0));
        assert_eq!(mesh.len(), 6);
        assert!(mesh.iter().all(|m| m.uvs.len() == m.vertices.len()));
        let top = &mesh[0];
        let span = top.uvs.iter().map(|u| u.0).fold(f64::MIN, f64::max) - top.uvs.iter().map(|u| u.0).fold(f64::MAX, f64::min);
        assert!((span - 1.0).abs() < 1e-9);
    }
}
