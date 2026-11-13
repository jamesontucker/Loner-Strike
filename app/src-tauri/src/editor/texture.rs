//! Texture alignment helpers (Valve 220 axes): world/face alignment, fit, shift, scale, rotate,
//! and "texture lock" (keep the texture stuck to the brush when it moves).

use super::doc::{nonzero, TexAlign};
use super::math::{Affine, Mat3, Plane, Vec3};

/// Hammer's default scale.
pub const DEFAULT_SCALE: f64 = 1.0;

/// The Quake/GoldSrc base axes for a face normal: floors and ceilings project from above,
/// walls from the side. Returns (u, v).
pub fn base_axes(normal: Vec3) -> (Vec3, Vec3) {
    match normal.major_axis() {
        2 => (Vec3::X, -Vec3::Y),
        0 => (Vec3::Y, -Vec3::Z),
        _ => (Vec3::X, -Vec3::Z),
    }
}

/// World-aligned alignment ("align to world" in Hammer), scale 1, no offset.
pub fn world_aligned(normal: Vec3) -> TexAlign {
    let (u, v) = base_axes(normal);
    TexAlign { u_axis: u, u_offset: 0.0, v_axis: v, v_offset: 0.0, rotation: 0.0, scale_u: DEFAULT_SCALE, scale_v: DEFAULT_SCALE }
}

/// Face-aligned axes: both axes lie in the face, u as close to the world u as possible.
pub fn face_axes(normal: Vec3) -> (Vec3, Vec3) {
    let (bu, _) = base_axes(normal);
    let mut u = (bu - normal * bu.dot(normal)).normalized();
    if u == Vec3::ZERO {
        u = normal.any_perpendicular();
    }
    let v = u.cross(normal).normalized();
    (u, v)
}

/// Point the axes at the face ("align to face"), keeping scale; offsets and rotation reset.
pub fn align_to_face(a: &TexAlign, normal: Vec3) -> TexAlign {
    let (u, v) = face_axes(normal);
    TexAlign { u_axis: u, v_axis: v, u_offset: 0.0, v_offset: 0.0, rotation: 0.0, ..a.clone() }
}

/// Point the axes at the world ("align to world"), keeping scale.
pub fn align_to_world(a: &TexAlign, normal: Vec3) -> TexAlign {
    let w = world_aligned(normal);
    TexAlign { scale_u: a.scale_u, scale_v: a.scale_v, ..w }
}

/// Alignment from the old Quake "standard" .map format: offsets, rotation and scale applied to
/// the base axes exactly as the Quake/HLT compilers do.
pub fn from_standard(normal: Vec3, x_off: f64, y_off: f64, rotation: f64, sx: f64, sy: f64) -> TexAlign {
    let (mut u, mut v) = base_axes(normal);
    let (s, c) = rotation.to_radians().sin_cos();
    let (s, c) = (snap_unit(s), snap_unit(c));
    let idx = |a: Vec3| if a.x != 0.0 { 0 } else if a.y != 0.0 { 1 } else { 2 };
    let (sv, tv) = (idx(u), idx(v));
    for a in [&mut u, &mut v] {
        let (x, y) = (get(*a, sv), get(*a, tv));
        let (nx, ny) = (c * x - s * y, s * x + c * y);
        set(a, sv, nx);
        set(a, tv, ny);
    }
    TexAlign { u_axis: u, u_offset: x_off, v_axis: v, v_offset: y_off, rotation, scale_u: nonzero(sx), scale_v: nonzero(sy) }
}

fn snap_unit(x: f64) -> f64 {
    for t in [-1.0, 0.0, 1.0] {
        if (x - t).abs() < 1e-12 {
            return t;
        }
    }
    x
}
fn get(a: Vec3, i: usize) -> f64 {
    a[i]
}
fn set(a: &mut Vec3, i: usize, val: f64) {
    match i {
        0 => a.x = val,
        1 => a.y = val,
        _ => a.z = val,
    }
}

/// Move the texture by whole pixels.
pub fn shift(a: &TexAlign, du: f64, dv: f64) -> TexAlign {
    TexAlign { u_offset: a.u_offset + du, v_offset: a.v_offset + dv, ..a.clone() }
}

/// Change the scale, keeping the texture fixed at `anchor` (a point on the face; e.g. the
/// face centre) so scaling feels like zooming around where you clicked.
pub fn set_scale(a: &TexAlign, su: f64, sv: f64, anchor: Vec3) -> TexAlign {
    let (su, sv) = (nonzero(su), nonzero(sv));
    let (u0, v0) = a.uv(anchor);
    let mut out = TexAlign { scale_u: su, scale_v: sv, ..a.clone() };
    out.u_offset = u0 - anchor.dot(out.u_axis) / su;
    out.v_offset = v0 - anchor.dot(out.v_axis) / sv;
    out
}

/// Rotate the texture on the face by `degrees` about the face normal, keeping the texture
/// fixed at `anchor`.
pub fn rotate(a: &TexAlign, normal: Vec3, degrees: f64, anchor: Vec3) -> TexAlign {
    let r = Mat3::rotation(normal, degrees);
    let (u0, v0) = a.uv(anchor);
    let mut out = TexAlign {
        u_axis: r.mul_vec(a.u_axis),
        v_axis: r.mul_vec(a.v_axis),
        rotation: (a.rotation + degrees).rem_euclid(360.0),
        ..a.clone()
    };
    out.u_offset = u0 - anchor.dot(out.u_axis) / nonzero(out.scale_u);
    out.v_offset = v0 - anchor.dot(out.v_axis) / nonzero(out.scale_v);
    out
}

/// Scale and offset so the texture covers the polygon exactly `repeat_u` x `repeat_v` times
/// ("fit" in Hammer). Axes stay as they are.
pub fn fit(a: &TexAlign, poly: &[Vec3], tex_w: f64, tex_h: f64, repeat_u: f64, repeat_v: f64) -> TexAlign {
    if poly.is_empty() {
        return a.clone();
    }
    let range = |axis: Vec3| {
        poly.iter().map(|p| p.dot(axis)).fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
    };
    let (u0, u1) = range(a.u_axis);
    let (v0, v1) = range(a.v_axis);
    let mut out = a.clone();
    if u1 - u0 > 1e-9 {
        out.scale_u = (u1 - u0) / (tex_w * nonzero(repeat_u));
        out.u_offset = -u0 / out.scale_u;
    }
    if v1 - v0 > 1e-9 {
        out.scale_v = (v1 - v0) / (tex_h * nonzero(repeat_v));
        out.v_offset = -v0 / out.scale_v;
    }
    normalize_offsets(&out, tex_w, tex_h)
}

/// Centre the texture on the polygon without changing scale.
pub fn center(a: &TexAlign, poly: &[Vec3], tex_w: f64, tex_h: f64) -> TexAlign {
    if poly.is_empty() {
        return a.clone();
    }
    let mid = |axis: Vec3, s: f64| {
        let (lo, hi) = poly.iter().map(|p| p.dot(axis)).fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        (lo + hi) * 0.5 / nonzero(s)
    };
    let mut out = a.clone();
    out.u_offset = tex_w * 0.5 - mid(a.u_axis, a.scale_u);
    out.v_offset = tex_h * 0.5 - mid(a.v_axis, a.scale_v);
    normalize_offsets(&out, tex_w, tex_h)
}

/// Where to line a texture up on a face (Hammer's justify buttons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Left,
    Right,
    Top,
    Bottom,
    Center,
    Fit,
}

impl Justify {
    pub fn parse(s: &str) -> Option<Justify> {
        Some(match s {
            "left" => Justify::Left,
            "right" => Justify::Right,
            "top" => Justify::Top,
            "bottom" => Justify::Bottom,
            "center" | "centre" => Justify::Center,
            "fit" => Justify::Fit,
            _ => return None,
        })
    }
}

/// Line the texture up with an edge of the points (one face, or several faces treated as
/// one), keeping scale (Fit changes scale to one repeat).
pub fn justify(a: &TexAlign, points: &[Vec3], tex_w: f64, tex_h: f64, how: Justify) -> TexAlign {
    if points.is_empty() {
        return a.clone();
    }
    let range = |axis: Vec3, s: f64| {
        points.iter().map(|p| p.dot(axis) / nonzero(s)).fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
    };
    let (u0, u1) = range(a.u_axis, a.scale_u);
    let (v0, v1) = range(a.v_axis, a.scale_v);
    let mut out = a.clone();
    match how {
        Justify::Left => out.u_offset = -u0,
        Justify::Right => out.u_offset = tex_w - u1,
        Justify::Top => out.v_offset = -v0,
        Justify::Bottom => out.v_offset = tex_h - v1,
        Justify::Center => return center(a, points, tex_w, tex_h),
        Justify::Fit => return fit(a, points, tex_w, tex_h, 1.0, 1.0),
    }
    normalize_offsets(&out, tex_w, tex_h)
}

/// Set the face sheet's numbers directly: offsets, scales and rotation (each only when
/// given). A new rotation turns the axes about the face normal by the difference.
pub fn set_values(a: &TexAlign, normal: Vec3, u_off: Option<f64>, v_off: Option<f64>, su: Option<f64>, sv: Option<f64>, rot: Option<f64>) -> TexAlign {
    let mut out = a.clone();
    if let Some(r) = rot {
        let d = r - a.rotation;
        if d.abs() > 1e-9 {
            let m = Mat3::rotation(normal, d);
            out.u_axis = m.mul_vec(a.u_axis);
            out.v_axis = m.mul_vec(a.v_axis);
        }
        out.rotation = r.rem_euclid(360.0);
    }
    if let Some(x) = su {
        out.scale_u = nonzero(x);
    }
    if let Some(x) = sv {
        out.scale_v = nonzero(x);
    }
    if let Some(x) = u_off {
        out.u_offset = x;
    }
    if let Some(x) = v_off {
        out.v_offset = x;
    }
    out
}

/// Bring offsets into 0..size (same look; tidier numbers).
pub fn normalize_offsets(a: &TexAlign, tex_w: f64, tex_h: f64) -> TexAlign {
    let wrap = |o: f64, s: f64| if s > 0.0 { clean_zero(o.rem_euclid(s)) } else { o };
    TexAlign { u_offset: wrap(a.u_offset, tex_w), v_offset: wrap(a.v_offset, tex_h), ..a.clone() }
}

fn clean_zero(x: f64) -> f64 {
    let r = (x * 1e6).round() / 1e6;
    if r.abs() < 1e-9 {
        0.0
    } else {
        r
    }
}

/// Texture lock: the alignment that keeps every point's texture coordinates when the face is
/// moved by `t` (`p -> m p + t`). Works for move, rotate, scale and mirror.
pub fn locked(a: &TexAlign, t: &Affine) -> TexAlign {
    let Some(inv) = t.m.inverse() else { return a.clone() };
    let inv_t = inv.transpose();
    let map_axis = |axis: Vec3, scale: f64, offset: f64| {
        // u(p) = p . (axis / scale) + offset; the new gradient is M^-T (axis / scale).
        let g = inv_t.mul_vec(axis / nonzero(scale));
        let len = g.len();
        if len < 1e-12 {
            return (axis, scale, offset);
        }
        let new_offset = offset - t.t.dot(g);
        let sign = if scale < 0.0 { -1.0 } else { 1.0 };
        let new_scale = sign / len;
        (g * new_scale, new_scale, new_offset)
    };
    let (u, su, uo) = map_axis(a.u_axis, a.scale_u, a.u_offset);
    let (v, sv, vo) = map_axis(a.v_axis, a.scale_v, a.v_offset);
    TexAlign { u_axis: u, scale_u: su, u_offset: uo, v_axis: v, scale_v: sv, v_offset: vo, rotation: a.rotation }
}

/// Without texture lock the axes stay put; but if a rotation turned the face edge-on to them
/// the texture would smear, so fall back to world alignment (keeping scale).
pub fn unlocked(a: &TexAlign, plane: &Plane) -> TexAlign {
    let proj = a.u_axis.cross(a.v_axis).normalized().dot(plane.normal).abs();
    if proj < 0.1 {
        align_to_world(a, plane.normal)
    } else {
        a.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::math::v3;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn base_axes_match_quake_table() {
        assert_eq!(base_axes(Vec3::Z), (Vec3::X, -Vec3::Y));
        assert_eq!(base_axes(-Vec3::X), (Vec3::Y, -Vec3::Z));
        assert_eq!(base_axes(Vec3::Y), (Vec3::X, -Vec3::Z));
        // 45 degree slopes count as floors (z wins ties), like the compilers.
        assert_eq!(base_axes(v3(1.0, 0.0, 1.0).normalized()).0, Vec3::X);
    }

    #[test]
    fn standard_format_rotation() {
        let a = from_standard(Vec3::Z, 0.0, 0.0, 90.0, 1.0, 1.0);
        assert!(a.u_axis.approx_eq(Vec3::Y, 1e-12), "{:?}", a.u_axis);
        assert!(a.v_axis.approx_eq(Vec3::X, 1e-12), "{:?}", a.v_axis);
    }

    #[test]
    fn lock_keeps_texture_on_points() {
        let a = TexAlign { u_offset: 13.0, v_offset: -7.0, scale_u: 0.5, scale_v: 2.0, ..world_aligned(Vec3::Z) };
        let p = v3(10.0, 20.0, 64.0);
        for t in [
            Affine::translate(v3(5.0, -3.0, 2.0)),
            Affine::around(Mat3::rotation(Vec3::Z, 30.0), v3(1.0, 2.0, 0.0)),
            Affine::around(Mat3::scale(v3(2.0, 0.5, 1.0)), v3(4.0, 4.0, 0.0)),
            Affine::around(Mat3::scale(v3(-1.0, 1.0, 1.0)), Vec3::ZERO),
        ] {
            let b = locked(&a, &t);
            assert!(close(b.uv(t.apply(p)), a.uv(p)), "{t:?}");
        }
    }

    #[test]
    fn fit_center_scale_rotate() {
        let poly = vec![v3(0.0, 0.0, 0.0), v3(128.0, 0.0, 0.0), v3(128.0, 64.0, 0.0), v3(0.0, 64.0, 0.0)];
        let a = fit(&world_aligned(Vec3::Z), &poly, 64.0, 64.0, 1.0, 1.0);
        assert!((a.scale_u - 2.0).abs() < 1e-9 && (a.scale_v - 1.0).abs() < 1e-9);
        // Corners land on texture edges.
        let (u, _) = a.uv(v3(0.0, 0.0, 0.0));
        let (u2, _) = a.uv(v3(128.0, 0.0, 0.0));
        assert!((u2 - u - 64.0).abs() < 1e-9 && (u.rem_euclid(64.0)).abs() < 1e-9);
        let c = center(&world_aligned(Vec3::Z), &poly, 64.0, 64.0);
        let (cu, cv) = c.uv(v3(64.0, 32.0, 0.0));
        assert!((cu.rem_euclid(64.0) - 32.0).abs() < 1e-9 && (cv.rem_euclid(64.0) - 32.0).abs() < 1e-9);
        let anchor = v3(10.0, 10.0, 0.0);
        let s = set_scale(&a, 0.25, 0.25, anchor);
        assert!(close(s.uv(anchor), a.uv(anchor)));
        let r = rotate(&a, Vec3::Z, 45.0, anchor);
        assert!(close(r.uv(anchor), a.uv(anchor)));
        assert_eq!(r.rotation, 45.0);
        let f = align_to_face(&a, v3(0.0, 1.0, 1.0).normalized());
        assert!(f.u_axis.dot(v3(0.0, 1.0, 1.0)).abs() < 1e-12 && f.v_axis.dot(v3(0.0, 1.0, 1.0)).abs() < 1e-12);
    }

    #[test]
    fn justify_edges() {
        use crate::editor::math::v3;
        // a wall 64 wide, 32 high, facing -y (u along x, v along -z)
        let poly = [v3(16.0, 0.0, 0.0), v3(80.0, 0.0, 0.0), v3(80.0, 0.0, 32.0), v3(16.0, 0.0, 32.0)];
        let a = world_aligned(-Vec3::Y);
        let left = justify(&a, &poly, 128.0, 128.0, Justify::Left);
        let (u, _) = left.uv(v3(16.0, 0.0, 0.0));
        assert!(u.rem_euclid(128.0).abs() < 1e-9);
        let right = justify(&a, &poly, 128.0, 128.0, Justify::Right);
        assert!((right.uv(v3(80.0, 0.0, 0.0)).0.rem_euclid(128.0)).abs() < 1e-9);
        let top = justify(&a, &poly, 128.0, 128.0, Justify::Top);
        // v grows downwards: the top edge (z 32) starts the texture
        assert!(top.uv(v3(16.0, 0.0, 32.0)).1.rem_euclid(128.0).abs() < 1e-9);
        let bottom = justify(&a, &poly, 128.0, 128.0, Justify::Bottom);
        assert!(bottom.uv(v3(16.0, 0.0, 0.0)).1.rem_euclid(128.0).abs() < 1e-9);
        let fit1 = justify(&a, &poly, 128.0, 128.0, Justify::Fit);
        assert!((fit1.scale_u - 0.5).abs() < 1e-9 && (fit1.scale_v - 0.25).abs() < 1e-9);
        assert_eq!(Justify::parse("centre"), Some(Justify::Center));
        assert_eq!(Justify::parse("nope"), None);
    }

    #[test]
    fn set_values_directly() {
        let a = world_aligned(Vec3::Z);
        let b = set_values(&a, Vec3::Z, Some(8.0), None, Some(0.5), None, Some(90.0));
        assert_eq!(b.u_offset, 8.0);
        assert_eq!(b.v_offset, a.v_offset);
        assert_eq!(b.scale_u, 0.5);
        assert_eq!(b.scale_v, a.scale_v);
        assert_eq!(b.rotation, 90.0);
        assert!(b.u_axis.approx_eq(Vec3::Y, 1e-9), "{:?}", b.u_axis);
        // back to 0 turns the axes back
        let c = set_values(&b, Vec3::Z, None, None, None, None, Some(0.0));
        assert!(c.u_axis.approx_eq(a.u_axis, 1e-9));
    }
}
