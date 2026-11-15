//! Small vector and plane math for the brush editor. Everything is f64 in GoldSrc units
//! (z up), the same numbers a .map file holds.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Index, Mul, Neg, Sub, SubAssign};

/// Distance below which a point counts as lying on a plane.
pub const PLANE_EPS: f64 = 1e-4;
/// Distance below which two points count as the same vertex.
pub const POINT_EPS: f64 = 1e-3;
/// Half-size of the "infinite" polygon a face starts from before it is clipped. Far beyond the
/// GoldSrc world (+-4096) so even out-of-bounds brushes still build.
pub const BIG: f64 = 131_072.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const fn v3(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);
    pub const X: Vec3 = v3(1.0, 0.0, 0.0);
    pub const Y: Vec3 = v3(0.0, 1.0, 0.0);
    pub const Z: Vec3 = v3(0.0, 0.0, 1.0);

    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    /// Unit vector, or zero when the vector is (nearly) zero.
    pub fn normalized(self) -> Vec3 {
        let l = self.len();
        if l < 1e-12 {
            Vec3::ZERO
        } else {
            self / l
        }
    }
    pub fn abs(self) -> Vec3 {
        v3(self.x.abs(), self.y.abs(), self.z.abs())
    }
    pub fn min(self, o: Vec3) -> Vec3 {
        v3(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    pub fn max(self, o: Vec3) -> Vec3 {
        v3(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    pub fn mul_each(self, o: Vec3) -> Vec3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn approx_eq(self, o: Vec3, eps: f64) -> bool {
        (self - o).abs().max_elem() <= eps
    }
    pub fn max_elem(self) -> f64 {
        self.x.max(self.y).max(self.z)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    /// Round each component to the nearest multiple of `grid` (no-op for grid <= 0).
    pub fn snapped(self, grid: f64) -> Vec3 {
        if grid <= 0.0 {
            return self;
        }
        let s = |a: f64| (a / grid).round() * grid;
        v3(s(self.x), s(self.y), s(self.z))
    }
    /// Index (0, 1, 2) of the largest absolute component; ties go z, then x, then y, like the
    /// Quake/GoldSrc texture base-axis table.
    pub fn major_axis(self) -> usize {
        let a = self.abs();
        if a.z >= a.x && a.z >= a.y {
            2
        } else if a.x >= a.y {
            0
        } else {
            1
        }
    }
    /// Any unit vector perpendicular to this (unit) vector.
    pub fn any_perpendicular(self) -> Vec3 {
        let helper = if self.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
        helper.cross(self).normalized()
    }
}

impl Index<usize> for Vec3 {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => &self.z,
        }
    }
}
impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f64) -> Vec3 {
        v3(self.x / s, self.y / s, self.z / s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        v3(-self.x, -self.y, -self.z)
    }
}

/// A plane `normal . p = dist`. For brush faces the normal points out of the brush, so the
/// brush is the set of points with `normal . p <= dist` for every face.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub normal: Vec3,
    pub dist: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
    On,
}

impl Plane {
    pub fn new(normal: Vec3, dist: f64) -> Plane {
        Plane { normal, dist }
    }
    /// Plane through a point with the given (unit) normal.
    pub fn from_point_normal(p: Vec3, normal: Vec3) -> Plane {
        Plane { normal, dist: normal.dot(p) }
    }
    /// Plane from three points the way a .map file stores them: the normal is
    /// `(p2 - p0) x (p1 - p0)`, i.e. the points run clockwise seen from outside.
    /// None when the points are (nearly) in a line.
    pub fn from_map_points(p: &[Vec3; 3]) -> Option<Plane> {
        let n = (p[2] - p[0]).cross(p[1] - p[0]);
        let l = n.len();
        if !(l > 1e-9) || !l.is_finite() {
            return None;
        }
        let normal = n / l;
        Some(Plane { normal, dist: normal.dot(p[0]) })
    }
    pub fn distance(&self, p: Vec3) -> f64 {
        self.normal.dot(p) - self.dist
    }
    pub fn side(&self, p: Vec3, eps: f64) -> Side {
        let d = self.distance(p);
        if d > eps {
            Side::Front
        } else if d < -eps {
            Side::Back
        } else {
            Side::On
        }
    }
    pub fn flipped(&self) -> Plane {
        Plane { normal: -self.normal, dist: -self.dist }
    }
    pub fn project(&self, p: Vec3) -> Vec3 {
        p - self.normal * self.distance(p)
    }
    /// Same plane (and same facing) within tolerances.
    pub fn approx_eq(&self, o: &Plane) -> bool {
        self.normal.approx_eq(o.normal, 1e-6) && (self.dist - o.dist).abs() <= POINT_EPS
    }
}

/// A 3x3 matrix stored as rows; used for rotate/scale and the general affine transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub r: [Vec3; 3],
}

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3 { r: [Vec3::X, Vec3::Y, Vec3::Z] };

    pub fn scale(s: Vec3) -> Mat3 {
        Mat3 { r: [v3(s.x, 0.0, 0.0), v3(0.0, s.y, 0.0), v3(0.0, 0.0, s.z)] }
    }
    /// Rotation by `degrees` about a unit `axis` (right-handed).
    pub fn rotation(axis: Vec3, degrees: f64) -> Mat3 {
        let a = axis.normalized();
        let (s, c) = degrees.to_radians().sin_cos();
        // Snap the common right angles exactly, so rotating a box by 90 keeps integer points.
        let (s, c) = (clean(s), clean(c));
        let t = 1.0 - c;
        Mat3 {
            r: [
                v3(t * a.x * a.x + c, t * a.x * a.y - s * a.z, t * a.x * a.z + s * a.y),
                v3(t * a.x * a.y + s * a.z, t * a.y * a.y + c, t * a.y * a.z - s * a.x),
                v3(t * a.x * a.z - s * a.y, t * a.y * a.z + s * a.x, t * a.z * a.z + c),
            ],
        }
    }
    pub fn mul_vec(&self, v: Vec3) -> Vec3 {
        v3(self.r[0].dot(v), self.r[1].dot(v), self.r[2].dot(v))
    }
    pub fn transpose(&self) -> Mat3 {
        let r = &self.r;
        Mat3 { r: [v3(r[0].x, r[1].x, r[2].x), v3(r[0].y, r[1].y, r[2].y), v3(r[0].z, r[1].z, r[2].z)] }
    }
    pub fn det(&self) -> f64 {
        self.r[0].dot(self.r[1].cross(self.r[2]))
    }
    /// `self * o` (apply `o` first, then `self`).
    pub fn mul(&self, o: &Mat3) -> Mat3 {
        let c = o.transpose();
        Mat3 { r: [0, 1, 2].map(|i| v3(self.r[i].dot(c.r[0]), self.r[i].dot(c.r[1]), self.r[i].dot(c.r[2]))) }
    }
    pub fn inverse(&self) -> Option<Mat3> {
        let d = self.det();
        if d.abs() < 1e-12 || !d.is_finite() {
            return None;
        }
        let [a, b, c] = self.r;
        // Columns of the inverse are the cross products of the rows, divided by det.
        let cols = [b.cross(c) / d, c.cross(a) / d, a.cross(b) / d];
        Some(Mat3 { r: cols }.transpose())
    }
}

fn clean(x: f64) -> f64 {
    for t in [-1.0, 0.0, 1.0] {
        if (x - t).abs() < 1e-12 {
            return t;
        }
    }
    x
}

/// An affine transform `p -> m * p + t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub m: Mat3,
    pub t: Vec3,
}

impl Affine {
    pub fn translate(d: Vec3) -> Affine {
        Affine { m: Mat3::IDENTITY, t: d }
    }
    /// Apply `m` around a pivot point.
    pub fn around(m: Mat3, pivot: Vec3) -> Affine {
        Affine { m, t: pivot - m.mul_vec(pivot) }
    }
    pub fn apply(&self, p: Vec3) -> Vec3 {
        self.m.mul_vec(p) + self.t
    }
    pub const IDENTITY: Affine = Affine { m: Mat3::IDENTITY, t: Vec3::ZERO };
    /// `self` first, then `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        Affine { m: next.m.mul(&self.m), t: next.m.mul_vec(self.t) + next.t }
    }
}

/// Axis-aligned bounds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    pub fn empty() -> Bounds {
        Bounds { min: v3(f64::MAX, f64::MAX, f64::MAX), max: v3(f64::MIN, f64::MIN, f64::MIN) }
    }
    pub fn add(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn union(&self, o: &Bounds) -> Bounds {
        Bounds { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
    /// True when the boxes overlap with positive volume (touching faces don't count).
    pub fn overlaps(&self, o: &Bounds) -> bool {
        self.min.x < o.max.x - PLANE_EPS
            && o.min.x < self.max.x - PLANE_EPS
            && self.min.y < o.max.y - PLANE_EPS
            && o.min.y < self.max.y - PLANE_EPS
            && self.min.z < o.max.z - PLANE_EPS
            && o.min.z < self.max.z - PLANE_EPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_points_give_outward_normal() {
        // Top face of a 0..64 box as Hammer writes it (clockwise seen from above).
        let p = [v3(0.0, 0.0, 64.0), v3(0.0, 64.0, 64.0), v3(64.0, 64.0, 64.0)];
        let pl = Plane::from_map_points(&p).unwrap();
        assert!(pl.normal.approx_eq(Vec3::Z, 1e-12));
        assert!((pl.dist - 64.0).abs() < 1e-12);
        assert!(Plane::from_map_points(&[Vec3::ZERO, Vec3::X, Vec3::X * 2.0]).is_none());
    }

    #[test]
    fn rotation_and_inverse() {
        let r = Mat3::rotation(Vec3::Z, 90.0);
        assert_eq!(r.mul_vec(Vec3::X), Vec3::Y);
        let inv = r.inverse().unwrap();
        assert!(inv.mul_vec(Vec3::Y).approx_eq(Vec3::X, 1e-12));
        let s = Mat3::scale(v3(2.0, 3.0, 4.0)).inverse().unwrap();
        assert!(s.mul_vec(v3(2.0, 3.0, 4.0)).approx_eq(v3(1.0, 1.0, 1.0), 1e-12));
        let a = Affine::around(r, v3(10.0, 0.0, 0.0));
        assert!(a.apply(v3(11.0, 0.0, 0.0)).approx_eq(v3(10.0, 1.0, 0.0), 1e-12));
    }

    #[test]
    fn snapping_and_axes() {
        assert_eq!(v3(7.0, -9.0, 15.9).snapped(8.0), v3(8.0, -8.0, 16.0));
        assert_eq!(v3(1.0, 1.0, 1.0).major_axis(), 2);
        assert_eq!(v3(-3.0, 1.0, 1.0).major_axis(), 0);
        let p = v3(0.3, 0.4, 0.866).normalized().any_perpendicular();
        assert!(p.dot(v3(0.3, 0.4, 0.866).normalized()).abs() < 1e-12);
    }
}
