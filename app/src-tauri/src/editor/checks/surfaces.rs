//! Face checks: z-fighting, micro-gaps, off-grid brushes, faces nobody can see, texture seams
//! and stretched textures. One pass over neighboring brush pairs finds the first three kinds
//! of pair problems; the rest look at one brush at a time.

use super::scan::{at, overlap_area, pairs, Contents, Scan};
use super::views::Field;
use super::{issue, FixData, Issue, Level, MoreFix, Options};
use crate::editor::doc::TexAlign;
use crate::editor::geom::{clip_polygon_back, polygon_area, polygon_center, Polygon};
use crate::editor::math::{Bounds, Plane, Vec3};
use std::collections::HashSet;

/// How many of one kind are listed one by one (the rest are counted in one line).
pub const LIST_MAX: usize = 100;

/// Faces nobody can see: (brush index, face), and whether the compiler keeps them.
pub struct Hidden {
    pub faces: Vec<(usize, usize)>,
    pub set: HashSet<(usize, usize)>,
    /// Faces the compiler doesn't remove by itself (so NULL saves something).
    pub kept: usize,
    pub against_void: usize,
    /// Lighting data those kept faces would have used, in bytes.
    pub lightmap_bytes: usize,
}

const COVER_EPS: f64 = 0.05;

/// The parts of the fragments outside the brush (a point on the face counts as inside when
/// the point just in front of the face is inside: a face pressed against a solid is hidden).
fn subtract(frags: Vec<Polygon>, planes: &[Option<Plane>], n: Vec3) -> Vec<Polygon> {
    let mut out = Vec::new();
    for frag in frags {
        let mut rest = frag;
        for q in planes.iter().flatten() {
            let q2 = Plane::new(q.normal, q.dist - COVER_EPS * q.normal.dot(n));
            let front = clip_polygon_back(&rest, &q2.flipped());
            if polygon_area(&front) > 0.01 {
                out.push(front);
            }
            rest = clip_polygon_back(&rest, &q2);
            if polygon_area(&rest) <= 0.01 {
                rest.clear();
                break;
            }
        }
    }
    out
}

/// Lightmap bytes a face uses: one sample per 16 texels each way, 3 bytes each.
pub fn lightmap_bytes(poly: &[Vec3], a: &TexAlign) -> usize {
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in poly {
        let (u, v) = a.uv(*p);
        u0 = u0.min(u);
        u1 = u1.max(u);
        v0 = v0.min(v);
        v1 = v1.max(v);
    }
    if u0 > u1 {
        return 0;
    }
    let w = ((u1 / 16.0).ceil() - (u0 / 16.0).floor() + 1.0).clamp(1.0, 1e5);
    let h = ((v1 / 16.0).ceil() - (v0 / 16.0).floor() + 1.0).clamp(1.0, 1e5);
    (w * h * 3.0) as usize
}

pub fn hidden_faces(scan: &Scan, field: Option<&Field>) -> Hidden {
    let mut h = Hidden { faces: Vec::new(), set: HashSet::new(), kept: 0, against_void: 0, lightmap_bytes: 0 };
    for (i, b) in scan.brushes.iter().enumerate() {
        if b.hidden || !b.drawn || !matches!(b.contents, Contents::Solid | Contents::Sky) {
            continue;
        }
        // moving entities show their hidden faces when they move
        if !b.world && !matches!(b.class, "func_wall" | "func_illusionary") {
            continue;
        }
        for (f, poly) in b.polys.iter().enumerate() {
            if !b.face_drawn(f) || super::scan::is_sky(&b.b.faces[f].texture) {
                continue;
            }
            let Some(pl) = b.planes[f] else { continue };
            let n = pl.normal;
            let mut hidden = false;
            let mut kept = true;
            if b.seals() && field.is_some_and(|fl| fl.face_on_void(poly, n)) {
                hidden = true;
                kept = false;
                h.against_void += 1;
            } else {
                let mut bb = Bounds::empty();
                poly.iter().for_each(|p| bb.add(*p));
                let near: Vec<usize> = scan.near(bb.min, bb.max, 0.1).into_iter().filter(|k| *k != i && scan.is_opaque(*k) && !scan.brushes[*k].hidden).collect();
                let mut frags = vec![poly.clone()];
                // structural world brushes first: what they cover on another world brush, the
                // compiler removes itself (it doesn't clip by func_detail or entities)
                for pass_world in [true, false] {
                    for k in near.iter().filter(|k| scan.brushes[**k].seals() == pass_world) {
                        frags = subtract(frags, &scan.brushes[*k].planes, n);
                        if frags.is_empty() {
                            break;
                        }
                    }
                    if frags.is_empty() {
                        hidden = true;
                        kept = !(pass_world && b.seals());
                        break;
                    }
                }
            }
            if hidden {
                h.faces.push((i, f));
                h.set.insert((i, f));
                if kept {
                    h.kept += 1;
                    h.lightmap_bytes += lightmap_bytes(poly, &b.b.faces[f].align);
                }
            }
        }
    }
    h
}

/// The hidden-faces line for the panel.
pub fn hidden_issue(scan: &Scan, h: &Hidden) -> Option<Issue> {
    let (i, f) = *h.faces.first()?;
    let b = &scan.brushes[i];
    let n = h.faces.len();
    let saving = if h.kept > 0 {
        format!(" Making them NULL saves about {} KB of lighting and {} face{} the compiler would keep.", (h.lightmap_bytes + 1023) / 1024, h.kept, if h.kept == 1 { "" } else { "s" })
    } else {
        " The compiler drops these itself, so NULL only tidies the map.".to_string()
    };
    let void = if h.against_void > 0 { format!(" ({} face the void outside the map)", h.against_void) } else { String::new() };
    Some(Issue {
        brush: Some(b.id.0),
        face: Some(f),
        at: Some(at(polygon_center(&b.polys[f]))),
        hint: Some("NULL isn't drawn or lit, so faces against walls, floors or the void cost nothing.".into()),
        ..issue(Level::Info, "hidden-faces", format!("{n} face{} nobody can see{void} aren't NULL.{saving}", if n == 1 { "" } else { "s" }))
    })
}

fn same_dir(a: Vec3, b: Vec3) -> bool {
    a.normalized().dot(b.normalized()) > 0.9999
}

/// Distance from 0 modulo `m`, either way.
fn off_by(x: f64, m: f64) -> f64 {
    let r = x.rem_euclid(m);
    r.min(m - r)
}

/// A shared stretch of edge between two touching polygons (its midpoint), at least 1 unit long.
fn shared_edge(a: &[Vec3], b: &[Vec3]) -> Option<Vec3> {
    for i in 0..a.len() {
        let (a0, a1) = (a[i], a[(i + 1) % a.len()]);
        let d = a1 - a0;
        let len = d.len();
        if len < 1.0 {
            continue;
        }
        let dir = d / len;
        for j in 0..b.len() {
            let (b0, b1) = (b[j], b[(j + 1) % b.len()]);
            let off = |p: Vec3| {
                let r = p - a0;
                (r - dir * r.dot(dir)).len()
            };
            if off(b0) > 0.01 || off(b1) > 0.01 {
                continue;
            }
            let (t0, t1) = ((b0 - a0).dot(dir), (b1 - a0).dot(dir));
            let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(len));
            if hi - lo >= 1.0 {
                return Some(a0 + dir * ((lo + hi) / 2.0));
            }
        }
    }
    None
}

/// Z-fighting, micro-gaps and texture seams between neighboring brushes.
pub fn pair_checks(scan: &Scan, hidden: &Hidden, out: &mut Vec<Issue>) {
    let mut zf: Vec<Issue> = Vec::new();
    let mut gaps: Vec<Issue> = Vec::new();
    let mut seams: Vec<Issue> = Vec::new();
    let (mut zf_n, mut gap_n, mut seam_n) = (0usize, 0usize, 0usize);
    let names = |i: usize| -> String {
        let b = &scan.brushes[i];
        if b.owner().is_none() {
            "a world brush".into()
        } else {
            format!("a {}", b.class)
        }
    };
    for (i, j) in pairs(scan, 1.01) {
        let (a, b) = (&scan.brushes[i], &scan.brushes[j]);
        if a.hidden || b.hidden {
            continue;
        }
        let mut zf_done = false;
        let mut gap_done = false;
        for (fa, pa) in a.planes.iter().enumerate() {
            let Some(pa) = pa else { continue };
            if a.polys[fa].is_empty() {
                continue;
            }
            for (fb, pb) in b.planes.iter().enumerate() {
                let Some(pb) = pb else { continue };
                if b.polys[fb].is_empty() {
                    continue;
                }
                let dot = pa.normal.dot(pb.normal);
                if dot > 0.9999 && (pa.dist - pb.dist).abs() < 0.1 {
                    let drawn = a.face_drawn(fa) && b.face_drawn(fb) && !hidden.set.contains(&(i, fa)) && !hidden.set.contains(&(j, fb));
                    if !drawn {
                        continue;
                    }
                    if a.owner() != b.owner() {
                        if zf_done {
                            continue;
                        }
                        let (area, poly) = overlap_area(&a.polys[fa], &b.polys[fb], pb.normal);
                        // a 4 x 4 patch or more, really on the same plane (not just near the origin)
                        if area < 16.0 || poly.iter().any(|p| pb.distance(*p).abs() > 0.1) {
                            continue;
                        }
                        zf_done = true;
                        zf_n += 1;
                        if zf.len() >= LIST_MAX {
                            continue;
                        }
                        // the entity's face moves (the second one's when both are entities)
                        let (e, fe, other) = if b.owner().is_some() { (j, fb, i) } else { (i, fa, j) };
                        let eb = &scan.brushes[e];
                        let mut more = Vec::new();
                        // NULL the smaller face when the other covers all of it
                        let (sa, sb) = (polygon_area(&a.polys[fa]), polygon_area(&b.polys[fb]));
                        let (small, sf) = if sa <= sb { (i, fa) } else { (j, fb) };
                        if area >= sa.min(sb) - 0.5 {
                            more.push(MoreFix { fix: "null-face", brush: Some(scan.brushes[small].id.0), entity: None, data: Some(FixData { face: Some(sf), ..Default::default() }) });
                        }
                        let ent = eb.entity.and_then(|id| scan.doc.entities.get(&id));
                        if eb.class == "func_wall" && ent.is_some_and(|x| x.get("targetname").is_none() && x.get("rendermode").is_none_or(|m| m.trim() == "0")) {
                            more.push(MoreFix { fix: "to-world", brush: None, entity: eb.entity.map(|x| x.0), data: None });
                        }
                        zf.push(Issue {
                            brush: Some(eb.id.0),
                            entity: eb.entity.map(|x| x.0),
                            face: Some(fe),
                            at: Some(at(polygon_center(&poly))),
                            hint: Some("Move one of them 1 unit, NULL the face that should be hidden, or make the entity part of the world so the compiler merges them.".into()),
                            fix_data: Some(FixData { face: Some(fe), dist: Some(1.0), ..Default::default() }),
                            more_fixes: more,
                            ..issue(Level::Warning, "z-fighting", format!("Z-fighting: {} and {} have faces on the same spot ({} sq units); they flicker in the game.", names(e), names(other), area.round()))
                        });
                    } else if (pa.dist - pb.dist).abs() < 0.01 && a.b.faces[fa].texture.eq_ignore_ascii_case(&b.b.faces[fb].texture) {
                        let (fa_al, fb_al) = (&a.b.faces[fa].align, &b.b.faces[fb].align);
                        if !same_dir(fa_al.u_axis, fb_al.u_axis) || !same_dir(fa_al.v_axis, fb_al.v_axis) {
                            continue;
                        }
                        let Some(p) = shared_edge(&a.polys[fa], &b.polys[fb]) else { continue };
                        if overlap_area(&a.polys[fa], &b.polys[fb], pb.normal).0 > 0.5 {
                            continue;
                        }
                        let tex = &a.b.faces[fa].texture;
                        let scale = (fa_al.scale_u - fb_al.scale_u).abs() > 1e-3 || (fa_al.scale_v - fb_al.scale_v).abs() > 1e-3;
                        // unknown sizes: 16 (every GoldSrc texture is a multiple of 16), so no false alarms
                        let (w, hh) = scan.doc.material(tex).map(|m| (m.width.max(1) as f64, m.height.max(1) as f64)).unwrap_or((16.0, 16.0));
                        let ((ua, va), (ub, vb)) = (fa_al.uv(p), fb_al.uv(p));
                        let shift = off_by(ua - ub, w) > 0.5 || off_by(va - vb, hh) > 0.5;
                        if !scale && !shift {
                            continue;
                        }
                        seam_n += 1;
                        if seams.len() >= LIST_MAX {
                            continue;
                        }
                        seams.push(Issue {
                            brush: Some(b.id.0),
                            face: Some(fb),
                            texture: Some(tex.clone()),
                            at: Some(at(p)),
                            hint: Some("Align with neighbor copies the neighbor's shift and scale, so the texture runs on across the edge.".into()),
                            fix_data: Some(FixData { face: Some(fb), other_brush: Some(a.id.0), other_face: Some(fa), ..Default::default() }),
                            ..issue(Level::Info, "texture-seam", format!("Texture seam: {tex} doesn't line up across the edge ({}).", if scale { "different scale" } else { "different shift" }))
                        });
                    }
                } else if dot < -0.9999 && !gap_done {
                    // B's face in front of A's, a hair away
                    // (structural brushes only: func_detail doesn't seal, and props near walls are fine)
                    let gap = -pb.dist - pa.dist;
                    if !(0.0..1.5).contains(&gap) || !a.seals() || !b.seals() {
                        continue;
                    }
                    let (area, poly) = overlap_area(&a.polys[fa], &b.polys[fb], pb.normal);
                    if area <= 1.0 {
                        continue;
                    }
                    // the real distance over the shared part (slightly tilted planes differ from their dist)
                    let apart: Vec<f64> = poly.iter().map(|p| pb.distance(*p)).collect();
                    if apart.iter().any(|g| !(0.01..1.0).contains(g)) {
                        continue;
                    }
                    let gap = apart.iter().sum::<f64>() / apart.len() as f64;
                    gap_done = true;
                    gap_n += 1;
                    if gaps.len() >= LIST_MAX {
                        continue;
                    }
                    // move the face that's off the whole-unit grid onto the other one
                    let whole = |d: f64| (d - d.round()).abs() < 0.01;
                    let (mv, fm) = if whole(pa.dist) && !whole(pb.dist) { (j, fb) } else { (i, fa) };
                    gaps.push(Issue {
                        brush: Some(scan.brushes[mv].id.0),
                        face: Some(fm),
                        at: Some(at(polygon_center(&poly))),
                        hint: Some("Close the gap moves the face onto its neighbor. Gaps under a unit let light bleed and can leak.".into()),
                        fix_data: Some(FixData { face: Some(fm), dist: Some(gap), ..Default::default() }),
                        ..issue(Level::Warning, "micro-gap", format!("Micro-gap: two brushes are {:.2} units apart; they look touching but aren't.", gap))
                    });
                }
            }
        }
    }
    for (mut list, n, code, what) in [(zf, zf_n, "z-fighting", "z-fighting spots"), (gaps, gap_n, "micro-gap", "micro-gaps"), (seams, seam_n, "texture-seam", "texture seams")] {
        out.append(&mut list);
        if n > LIST_MAX {
            out.push(Issue { fix: None, ..issue(Level::Info, code, format!("{} more {what} (fix the ones above first).", n - LIST_MAX)) });
        }
    }
}

/// Axis-aligned brushes with sides off whole units.
pub fn off_grid(scan: &Scan, out: &mut Vec<Issue>) {
    let mut n = 0;
    for b in &scan.brushes {
        let axial = b.planes.iter().flatten().all(|p| [p.normal.x, p.normal.y, p.normal.z].iter().filter(|c| c.abs() > 1.0 - 1e-9).count() == 1);
        if !axial {
            continue;
        }
        let Some(p) = b.planes.iter().flatten().find(|p| (p.dist - p.dist.round()).abs() > 0.01) else { continue };
        n += 1;
        if n > LIST_MAX {
            continue;
        }
        out.push(Issue {
            brush: Some(b.id.0),
            at: Some(at(b.bounds.center())),
            hint: Some("Snap rounds the corners to whole units. Brushes off the grid make tiny cracks and odd lighting seams.".into()),
            ..issue(Level::Info, "off-grid", format!("A brush side sits at {:.3}, between whole units.", p.dist.abs()))
        });
    }
    if n > LIST_MAX {
        out.push(Issue { fix: None, ..issue(Level::Info, "off-grid", format!("{} more brushes off the whole-unit grid.", n - LIST_MAX)) });
    }
}

/// Textures scaled below `min_scale` or above `max_scale`.
pub fn stretched(scan: &Scan, opts: &Options, out: &mut Vec<Issue>) {
    let mut n = 0;
    for b in &scan.brushes {
        if b.hidden {
            continue;
        }
        for (f, face) in b.b.faces.iter().enumerate() {
            if !b.face_drawn(f) {
                continue;
            }
            let (su, sv) = (face.align.scale_u.abs(), face.align.scale_v.abs());
            let bad = |s: f64| s < opts.min_scale || s > opts.max_scale;
            if !bad(su) && !bad(sv) {
                continue;
            }
            n += 1;
            if n > LIST_MAX {
                continue;
            }
            let small = su.min(sv) < opts.min_scale;
            out.push(Issue {
                brush: Some(b.id.0),
                face: Some(f),
                texture: Some(face.texture.clone()),
                at: Some(at(polygon_center(&b.polys[f]))),
                hint: Some(if small {
                    "Tiny scales repeat the texture into noise and make the face's lighting huge (\"bad surface extents\")."
                } else {
                    "Big scales blur the texture and coarsen its lighting."
                }
                .into()),
                fix_data: Some(FixData { face: Some(f), ..Default::default() }),
                ..issue(Level::Warning, "stretched-texture", format!("{} is scaled {}×{} (outside {}–{}).", face.texture, fmt(face.align.scale_u), fmt(face.align.scale_v), fmt(opts.min_scale), fmt(opts.max_scale)))
            });
        }
    }
    if n > LIST_MAX {
        out.push(Issue { fix: None, ..issue(Level::Warning, "stretched-texture", format!("{} more stretched faces.", n - LIST_MAX)) });
    }
}

pub fn fmt(x: f64) -> String {
    let s = format!("{:.3}", x);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
