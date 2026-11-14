//! Small linear algebra and geometry helpers.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

pub const PI: f32 = std::f32::consts::PI;

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
#[inline]
pub fn lerpd(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
#[inline]
pub fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}
#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// Wraps an angle in degrees to -180..180.
pub fn wrap_degrees(mut a: f32) -> f32 {
    a %= 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

macro_rules! vec3_type {
    ($name:ident, $t:ty) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        pub struct $name {
            pub x: $t,
            pub y: $t,
            pub z: $t,
        }
        impl $name {
            pub const ZERO: $name = $name { x: 0.0, y: 0.0, z: 0.0 };
            #[inline]
            pub const fn new(x: $t, y: $t, z: $t) -> Self {
                Self { x, y, z }
            }
            #[inline]
            pub fn dot(self, o: Self) -> $t {
                self.x * o.x + self.y * o.y + self.z * o.z
            }
            #[inline]
            pub fn cross(self, o: Self) -> Self {
                Self::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
            }
            #[inline]
            pub fn length_sq(self) -> $t {
                self.dot(self)
            }
            #[inline]
            pub fn length(self) -> $t {
                self.length_sq().sqrt()
            }
            pub fn normalized(self) -> Self {
                let l = self.length();
                if l > 1e-9 {
                    self * (1.0 / l)
                } else {
                    Self::ZERO
                }
            }
            #[inline]
            pub fn lerp(self, o: Self, t: $t) -> Self {
                self + (o - self) * t
            }
            #[inline]
            pub fn horizontal_len(self) -> $t {
                (self.x * self.x + self.z * self.z).sqrt()
            }
        }
        impl Add for $name {
            type Output = Self;
            #[inline]
            fn add(self, o: Self) -> Self {
                Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
            }
        }
        impl Sub for $name {
            type Output = Self;
            #[inline]
            fn sub(self, o: Self) -> Self {
                Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
            }
        }
        impl Mul<$t> for $name {
            type Output = Self;
            #[inline]
            fn mul(self, s: $t) -> Self {
                Self::new(self.x * s, self.y * s, self.z * s)
            }
        }
        impl Neg for $name {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self {
                Self::new(-self.x, -self.y, -self.z)
            }
        }
        impl AddAssign for $name {
            #[inline]
            fn add_assign(&mut self, o: Self) {
                *self = *self + o;
            }
        }
        impl SubAssign for $name {
            #[inline]
            fn sub_assign(&mut self, o: Self) {
                *self = *self - o;
            }
        }
    };
}
vec3_type!(Vec3, f32);
vec3_type!(DVec3, f64);

impl DVec3 {
    #[inline]
    pub fn to_f32(self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }
    #[inline]
    pub fn floor(self) -> IVec3 {
        IVec3::new(self.x.floor() as i32, self.y.floor() as i32, self.z.floor() as i32)
    }
}
impl Vec3 {
    #[inline]
    pub fn to_f64(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl IVec3 {
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
    #[inline]
    pub fn offset(self, dir: usize) -> Self {
        let d = DIRS[dir];
        Self::new(self.x + d.0, self.y + d.1, self.z + d.2)
    }
    #[inline]
    pub fn add(self, x: i32, y: i32, z: i32) -> Self {
        Self::new(self.x + x, self.y + y, self.z + z)
    }
    #[inline]
    pub fn center(self) -> DVec3 {
        DVec3::new(self.x as f64 + 0.5, self.y as f64 + 0.5, self.z as f64 + 0.5)
    }
    #[inline]
    pub fn to_f64(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

/// Direction indices shared with the native core: 0 -Y, 1 +Y, 2 -Z (north), 3 +Z (south), 4 -X (west), 5 +X (east).
pub const DIRS: [(i32, i32, i32); 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];
pub const DOWN: usize = 0;
pub const UP: usize = 1;
pub const NORTH: usize = 2;
pub const SOUTH: usize = 3;
pub const WEST: usize = 4;
pub const EAST: usize = 5;
pub const OPPOSITE: [usize; 6] = [1, 0, 3, 2, 5, 4];
/// Horizontal directions in clockwise order seen from above.
pub const HORIZONTAL: [usize; 4] = [NORTH, EAST, SOUTH, WEST];
/// Rotates a direction clockwise around +Y.
pub const ROTATE_CW: [usize; 6] = [0, 1, EAST, WEST, NORTH, SOUTH];
pub const ROTATE_CCW: [usize; 6] = [0, 1, WEST, EAST, SOUTH, NORTH];

/// Column-major 4x4 matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);

    pub fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y_radians * 0.5).tan();
        let mut m = [0.0; 16];
        m[0] = f / aspect;
        m[5] = f;
        m[10] = (far + near) / (near - far);
        m[11] = -1.0;
        m[14] = 2.0 * far * near / (near - far);
        Mat4(m)
    }
    pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
        let mut m = [0.0; 16];
        m[0] = 2.0 / (right - left);
        m[5] = 2.0 / (top - bottom);
        m[10] = -2.0 / (far - near);
        m[12] = -(right + left) / (right - left);
        m[13] = -(top + bottom) / (top - bottom);
        m[14] = -(far + near) / (far - near);
        m[15] = 1.0;
        Mat4(m)
    }
    pub fn translation(x: f32, y: f32, z: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[12] = x;
        m.0[13] = y;
        m.0[14] = z;
        m
    }
    pub fn scale(x: f32, y: f32, z: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[0] = x;
        m.0[5] = y;
        m.0[10] = z;
        m
    }
    pub fn rotation_x(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut m = Mat4::IDENTITY;
        m.0[5] = c;
        m.0[6] = s;
        m.0[9] = -s;
        m.0[10] = c;
        m
    }
    pub fn rotation_y(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut m = Mat4::IDENTITY;
        m.0[0] = c;
        m.0[2] = -s;
        m.0[8] = s;
        m.0[10] = c;
        m
    }
    pub fn rotation_z(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut m = Mat4::IDENTITY;
        m.0[0] = c;
        m.0[1] = s;
        m.0[4] = -s;
        m.0[5] = c;
        m
    }
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let a = &self.0;
        let b = &o.0;
        let mut m = [0.0; 16];
        for c in 0..4 {
            for r in 0..4 {
                m[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
            }
        }
        Mat4(m)
    }
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let m = &self.0;
        Vec3::new(
            m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12],
            m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13],
            m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14],
        )
    }
    pub fn transform_dir(&self, p: Vec3) -> Vec3 {
        let m = &self.0;
        Vec3::new(m[0] * p.x + m[4] * p.y + m[8] * p.z, m[1] * p.x + m[5] * p.y + m[9] * p.z, m[2] * p.x + m[6] * p.y + m[10] * p.z)
    }
    /// View matrix for a camera at the origin looking along yaw/pitch (degrees, Minecraft convention:
    /// yaw 0 looks toward +Z, yaw 90 toward -X; positive pitch looks down).
    pub fn view_rotation(yaw_deg: f32, pitch_deg: f32) -> Mat4 {
        Mat4::rotation_x(pitch_deg.to_radians()).mul(&Mat4::rotation_y((yaw_deg + 180.0).to_radians()))
    }
}

/// Unit view direction for Minecraft-style yaw/pitch in degrees.
pub fn look_dir(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (ys, yc) = yaw_deg.to_radians().sin_cos();
    let (ps, pc) = pitch_deg.to_radians().sin_cos();
    Vec3::new(-ys * pc, -ps, yc * pc)
}

/// Frustum as six inward-facing planes (a, b, c, d) extracted from a view-projection matrix.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [[f32; 4]; 6],
}
impl Frustum {
    pub fn from_matrix(m: &Mat4) -> Frustum {
        let m = &m.0;
        let row = |i: usize| [m[i], m[4 + i], m[8 + i], m[12 + i]];
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let add = |a: [f32; 4], b: [f32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
        let sub = |a: [f32; 4], b: [f32; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
        Frustum { planes: [add(r3, r0), sub(r3, r0), add(r3, r1), sub(r3, r1), add(r3, r2), sub(r3, r2)] }
    }
    /// Conservative test: false only when the box is entirely outside one plane.
    pub fn intersects_box(&self, min: Vec3, max: Vec3) -> bool {
        for p in &self.planes {
            let x = if p[0] >= 0.0 { max.x } else { min.x };
            let y = if p[1] >= 0.0 { max.y } else { min.y };
            let z = if p[2] >= 0.0 { max.z } else { min.z };
            if p[0] * x + p[1] * y + p[2] * z + p[3] < 0.0 {
                return false;
            }
        }
        true
    }
}

/// Axis-aligned box in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}
impl Aabb {
    pub fn new(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Aabb {
        Aabb { min: DVec3::new(x0, y0, z0), max: DVec3::new(x1, y1, z1) }
    }
    /// Box of the given width (X and Z) and height standing on `feet`.
    pub fn from_feet(feet: DVec3, width: f64, height: f64) -> Aabb {
        let h = width * 0.5;
        Aabb::new(feet.x - h, feet.y, feet.z - h, feet.x + h, feet.y + height, feet.z + h)
    }
    pub fn offset(&self, d: DVec3) -> Aabb {
        Aabb { min: self.min + d, max: self.max + d }
    }
    pub fn expand(&self, x: f64, y: f64, z: f64) -> Aabb {
        Aabb::new(self.min.x - x, self.min.y - y, self.min.z - z, self.max.x + x, self.max.y + y, self.max.z + z)
    }
    /// Extends the box in the direction of a movement vector.
    pub fn stretch(&self, d: DVec3) -> Aabb {
        let mut b = *self;
        if d.x < 0.0 { b.min.x += d.x } else { b.max.x += d.x }
        if d.y < 0.0 { b.min.y += d.y } else { b.max.y += d.y }
        if d.z < 0.0 { b.min.z += d.z } else { b.max.z += d.z }
        b
    }
    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x && self.max.x > o.min.x && self.min.y < o.max.y && self.max.y > o.min.y && self.min.z < o.max.z && self.max.z > o.min.z
    }
    pub fn contains(&self, p: DVec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y && p.z >= self.min.z && p.z <= self.max.z
    }
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
    /// Clips a movement along one axis (0 x, 1 y, 2 z) so `mover` does not enter `self`.
    pub fn clip_axis(&self, mover: &Aabb, axis: usize, mut d: f64) -> f64 {
        let (a_min, a_max) = (self.min, self.max);
        let overlaps = |i: usize| {
            let (smin, smax, mmin, mmax) = match i {
                0 => (a_min.x, a_max.x, mover.min.x, mover.max.x),
                1 => (a_min.y, a_max.y, mover.min.y, mover.max.y),
                _ => (a_min.z, a_max.z, mover.min.z, mover.max.z),
            };
            mmax > smin && mmin < smax
        };
        for i in 0..3 {
            if i != axis && !overlaps(i) {
                return d;
            }
        }
        let (smin, smax, mmin, mmax) = match axis {
            0 => (a_min.x, a_max.x, mover.min.x, mover.max.x),
            1 => (a_min.y, a_max.y, mover.min.y, mover.max.y),
            _ => (a_min.z, a_max.z, mover.min.z, mover.max.z),
        };
        if d > 0.0 && mmax <= smin {
            let gap = smin - mmax;
            if gap < d {
                d = gap;
            }
        } else if d < 0.0 && mmin >= smax {
            let gap = smax - mmin;
            if gap > d {
                d = gap;
            }
        }
        d
    }
    /// Ray/box intersection; returns the entry distance and the face that was hit.
    pub fn raycast(&self, origin: DVec3, dir: DVec3, max_dist: f64) -> Option<(f64, usize)> {
        let mut t_min = 0.0f64;
        let mut t_max = max_dist;
        let mut face = UP;
        let o = [origin.x, origin.y, origin.z];
        let d = [dir.x, dir.y, dir.z];
        let lo = [self.min.x, self.min.y, self.min.z];
        let hi = [self.max.x, self.max.y, self.max.z];
        const NEG: [usize; 3] = [WEST, DOWN, NORTH];
        const POS: [usize; 3] = [EAST, UP, SOUTH];
        for i in 0..3 {
            if d[i].abs() < 1e-12 {
                if o[i] < lo[i] || o[i] > hi[i] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d[i];
            let (mut t0, mut t1) = ((lo[i] - o[i]) * inv, (hi[i] - o[i]) * inv);
            let mut f = NEG[i];
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
                f = POS[i];
            }
            if t0 > t_min {
                t_min = t0;
                face = f;
            }
            if t1 < t_max {
                t_max = t1;
            }
            if t_min > t_max {
                return None;
            }
        }
        Some((t_min, face))
    }
}

#[inline]
pub fn floor_div(a: i32, b: i32) -> i32 {
    a.div_euclid(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_multiply_identity() {
        let t = Mat4::translation(1.0, 2.0, 3.0);
        assert_eq!(t.mul(&Mat4::IDENTITY), t);
        let p = t.transform_point(Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(p, Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn look_direction_matches_view_matrix() {
        for &(yaw, pitch) in &[(0.0, 0.0), (90.0, 0.0), (45.0, 30.0), (-120.0, -60.0)] {
            let d = look_dir(yaw, pitch);
            let v = Mat4::view_rotation(yaw, pitch).transform_dir(d);
            assert!((v.x).abs() < 1e-5 && (v.y).abs() < 1e-5 && (v.z + 1.0).abs() < 1e-5, "{yaw} {pitch} -> {v:?}");
        }
        assert!((look_dir(0.0, 0.0).z - 1.0).abs() < 1e-6);
        assert!((look_dir(90.0, 0.0).x + 1.0).abs() < 1e-6);
    }

    #[test]
    fn clip_axis_stops_at_surface() {
        let floor = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let mover = Aabb::new(0.2, 1.5, 0.2, 0.8, 3.3, 0.8);
        assert!((floor.clip_axis(&mover, 1, -2.0) + 0.5).abs() < 1e-12);
        let beside = Aabb::new(2.0, 1.5, 0.2, 2.6, 3.3, 0.8);
        assert_eq!(floor.clip_axis(&beside, 1, -2.0), -2.0);
    }

    #[test]
    fn raycast_hits_expected_face() {
        let b = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let (t, face) = b.raycast(DVec3::new(0.5, 3.0, 0.5), DVec3::new(0.0, -1.0, 0.0), 10.0).unwrap();
        assert!((t - 2.0).abs() < 1e-9);
        assert_eq!(face, UP);
        assert!(b.raycast(DVec3::new(0.5, 3.0, 0.5), DVec3::new(0.0, 1.0, 0.0), 10.0).is_none());
    }

    #[test]
    fn frustum_culls_behind_camera() {
        let proj = Mat4::perspective(70f32.to_radians(), 1.5, 0.05, 100.0);
        let vp = proj.mul(&Mat4::view_rotation(0.0, 0.0));
        let f = Frustum::from_matrix(&vp);
        assert!(f.intersects_box(Vec3::new(-1.0, -1.0, 5.0), Vec3::new(1.0, 1.0, 6.0)));
        assert!(!f.intersects_box(Vec3::new(-1.0, -1.0, -6.0), Vec3::new(1.0, 1.0, -5.0)));
    }
}
