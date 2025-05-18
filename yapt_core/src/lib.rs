pub trait TextureHandler {
    fn uv_value(&self, uv: Vec2) -> Vec3;
    fn does_intersect(&self, _: Vec2, _: &mut impl MinRng) -> bool { true }
}
impl TextureHandler for Vec3 {
    fn uv_value(&self, _: Vec2) -> Vec3 { *self }
}

mod coord;
mod rough_conductor;
mod rough_dielectric;
mod smooth_conductor;
mod smooth_dielectric;
mod smooth_dielectric_lambertian;
mod testing;
mod mat;

pub use mat::*;
pub use coord::*;
pub use rough_conductor::RoughConductor;
pub use rough_dielectric::RoughDielectric;
pub use smooth_conductor::SmoothConductor;
pub use smooth_dielectric::SmoothDielectric;
pub use smooth_dielectric_lambertian::SmoothDielectricLambertian;

use std::{
    f32::consts::{FRAC_1_PI, TAU},
    ops::{BitAnd, BitOr},
    cmp::Ordering,
    ops::{Add, AddAssign, Div, DivAssign, Index, Mul, MulAssign, Neg, Sub, SubAssign},
};

pub fn sort_by_indices<T>(vec: &mut [T], mut indices: Vec<usize>) {
    for index in 0..vec.len() {
        if indices[index] != index {
            let mut current_index = index;
            loop {
                let target_index = indices[current_index];
                indices[current_index] = current_index;
                if indices[target_index] == target_index {
                    break;
                }
                vec.swap(current_index, target_index);
                current_index = target_index;
            }
        }
    }
}

pub fn max_axis(a: &Vec3) -> usize {
    if a.x > a.y && a.x > a.z {
        0
    } else if a.y > a.z {
        1
    } else {
        2
    }
}

pub fn gamma(n: u32) -> f32 {
    let nm = n as f32 * 0.5 * f32::EPSILON;
    nm / (1.0 - nm)
}

pub fn max_vec3(a: &Vec3, b: &Vec3) -> Vec3 {
    let max_x = a.x.max(b.x);
    let max_y = a.y.max(b.y);
    let max_z = a.z.max(b.z);

    Vec3::new(max_x, max_y, max_z)
}

pub fn min_vec3(a: &Vec3, b: &Vec3) -> Vec3 {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let min_z = a.z.min(b.z);

    Vec3::new(min_x, min_y, min_z)
}

#[allow(clippy::float_cmp)]
pub fn float_cmp(a: f32, b: f32) -> Ordering {
    if a < b {
        Ordering::Less
    } else if a == b {
        Ordering::Equal
    } else {
        Ordering::Greater
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
    pub inv_dir: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, mut dir: Vec3) -> Self {
        dir.normalise();
        Self {
            origin,
            dir,
            inv_dir: Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z),
        }
    }
}

macro_rules! expr {
    ($e:expr) => {
        $e
    };
}

macro_rules! impl_operator {
    ($name:ident, $function_name:ident, $operator:tt) => {
        // Vec2
        impl $name for Vec2 {
        	type Output = Self;
            #[inline]
        	fn $function_name(self, rhs: Self) -> Self {
        		Vec2::new(expr!(self.x $operator rhs.x), expr!(self.y $operator rhs.y))
        	}
        }
        // Vec3
        impl $name for Vec3 {
            type Output = Self;
            #[inline]
            fn $function_name(self, rhs: Self) -> Self {
                Vec3::new(expr!(self.x $operator rhs.x), expr!(self.y $operator rhs.y), expr!(self.z $operator rhs.z))
            }
        }
    };
}

macro_rules! impl_operator_assign {
    ($name:ident, $function_name:ident, $operator:tt) => {
        // Vec2
        impl $name for Vec2 {
            #[inline]
            fn $function_name(&mut self, rhs: Self) {
                expr!(self.x $operator rhs.x);
                expr!(self.y $operator rhs.y);
            }
        }
        // Vec3
        impl $name for Vec3 {
            #[inline]
            fn $function_name(&mut self, rhs: Self) {
                expr!(self.x $operator rhs.x);
                expr!(self.y $operator rhs.y);
                expr!(self.z $operator rhs.z);
            }
        }
    };
}

macro_rules! impl_operator_float {
    ($name:ident, $function_name:ident, $operator:tt) => {
        // Vec2
        impl $name<f32> for Vec2 {
            type Output = Self;
            #[inline]
            fn $function_name(self, rhs: f32) -> Self {
                Vec2::new(expr!(self.x $operator rhs), expr!(self.y $operator rhs))
            }
        }
        impl $name<Vec2> for f32 {
            type Output = Vec2;
            #[inline]
            fn $function_name(self, rhs: Vec2) -> Vec2 {
                Vec2::new(expr!(self $operator rhs.x), expr!(self $operator rhs.y))
            }
        }
        // Vec3
        impl $name<f32> for Vec3 {
            type Output = Self;
            #[inline]
            fn $function_name(self, rhs: f32) -> Self {
                Vec3::new(expr!(self.x $operator rhs), expr!(self.y $operator rhs), expr!(self.z $operator rhs))
            }
        }
        impl $name<Vec3> for f32 {
            type Output = Vec3;
            #[inline]
            fn $function_name(self, rhs: Vec3) -> Vec3 {
                Vec3::new(expr!(self $operator rhs.x), expr!(self $operator rhs.y), expr!(self $operator rhs.z))
            }
        }
    };
}

macro_rules! impl_operator_float_assign {
    ($name:ident, $function_name:ident, $operator:tt) => {
        // Vec2
        impl $name<f32> for Vec2 {
            fn $function_name(&mut self, rhs: f32) {
                expr!(self.x $operator rhs);
                expr!(self.y $operator rhs);
            }
        }
        // Vec3
        impl $name<f32> for Vec3 {
            fn $function_name(&mut self, rhs: f32) {
                expr!(self.x $operator rhs);
                expr!(self.y $operator rhs);
                expr!(self.z $operator rhs);
            }
        }
    };
}

impl Vec3 {
    pub const ZERO: Self = Self::zero();
    pub const ONE: Self = Self::one();
    pub const X: Self = Self::x();
    pub const Y: Self = Self::y();
    pub const Z: Self = Self::z();

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v)
    }
    #[inline]
    pub const fn one() -> Self {
        Vec3::new(1.0, 1.0, 1.0)
    }

    #[inline]
    pub const fn zero() -> Self {
        Vec3::new(0.0, 0.0, 0.0)
    }

    #[inline]
    pub const fn x() -> Self {
        Vec3::new(1.0, 0.0, 0.0)
    }

    #[inline]
    pub const fn y() -> Self {
        Vec3::new(0.0, 1.0, 0.0)
    }

    #[inline]
    pub const fn z() -> Self {
        Vec3::new(0.0, 0.0, 1.0)
    }

    #[inline]
    pub const fn zyx(&self) -> Self {
        Self::new(self.z, self.y, self.x)
    }

    #[inline]
    pub const fn xzy(&self) -> Self {
        Self::new(self.x, self.z, self.y)
    }

    #[inline]
    pub const fn yxz(&self) -> Self {
        Self::new(self.y, self.x, self.z)
    }

    #[inline]
    pub const fn from_spherical(
        sin_theta: f32,
        cos_theta: f32,
        sin_phi: f32,
        cos_phi: f32,
    ) -> Self {
        Vec3::new(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta)
    }

    #[inline]
    pub const fn dot(&self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub const fn cross(&self, other: Self) -> Self {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    #[inline]
    pub const fn mag_sq(&self) -> f32 {
        self.dot(*self)
    }

    #[inline]
    pub fn mag(&self) -> f32 {
        self.dot(*self).sqrt()
    }

    #[inline]
    pub fn normalise(&mut self) {
        *self /= self.mag();
    }

    #[inline]
    pub fn normalised(self) -> Self {
        self / self.mag()
    }
    #[inline]
    pub fn abs(self) -> Self {
        Vec3::new(self.x.abs(), self.y.abs(), self.z.abs())
    }
    // note: self is pointing away from surface
    #[inline]
    pub fn reflect(&mut self, normal: Self) {
        *self = self.reflected(normal)
    }

    #[inline]
    pub fn reflected(&self, normal: Self) -> Self {
        2.0 * self.dot(normal) * normal - *self
    }

    #[inline]
    pub fn component_min(self) -> f32 {
        self.x.min(self.y.min(self.z))
    }

    #[inline]
    pub fn component_max(self) -> f32 {
        self.x.max(self.y.max(self.z))
    }

    #[inline]
    pub fn min_by_component(self, other: Self) -> Self {
        Vec3::new(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }

    #[inline]
    pub fn max_by_component(self, other: Self) -> Self {
        Vec3::new(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }

    #[inline]
    pub fn contains_nan(&self) -> bool {
        self.x.is_nan() || self.y.is_nan() || self.z.is_nan()
    }
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() || self.y.is_finite() || self.z.is_finite()
    }
    #[inline]
    pub fn hadamard(&self, other: Self) -> Self {
        Self::new(self.x * other.x, self.y * other.y, self.z * other.z)
    }
}

impl Vec2 {
    pub const ZERO: Self = Self::zero();
    pub const ONE: Self = Self::one();
    pub const X: Self = Self::x();
    pub const Y: Self = Self::y();
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    #[inline]
    pub const fn one() -> Self {
        Vec2::new(1.0, 1.0)
    }

    #[inline]
    pub const fn zero() -> Self {
        Vec2::new(0.0, 0.0)
    }

    #[inline]
    pub const fn x() -> Self {
        Vec2::new(1.0, 0.0)
    }

    #[inline]
    pub const fn y() -> Self {
        Vec2::new(0.0, 1.0)
    }

    #[inline]
    pub const fn dot(&self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    #[inline]
    pub const fn mag_sq(&self) -> f32 {
        self.dot(*self)
    }
    #[inline]
    pub fn mag(&self) -> f32 {
        self.dot(*self).sqrt()
    }
    #[inline]
    pub fn normalise(&mut self) {
        *self /= self.mag();
    }

    #[inline]
    pub fn normalised(self) -> Self {
        self / self.mag()
    }
    #[inline]
    pub fn abs(self) -> Self {
        Vec2::new(self.x.abs(), self.y.abs())
    }

    #[inline]
    pub fn component_min(self) -> f32 {
        self.x.min(self.y)
    }

    #[inline]
    pub fn component_max(self) -> f32 {
        self.x.max(self.y)
    }

    #[inline]
    pub fn min_by_component(self, other: Self) -> Self {
        Vec2::new(self.x.min(other.x), self.y.min(other.y))
    }

    #[inline]
    pub fn max_by_component(self, other: Self) -> Self {
        Vec2::new(self.x.max(other.x), self.y.max(other.y))
    }

    #[inline]
    pub fn contains_nan(&self) -> bool {
        self.x.is_nan() || self.y.is_nan()
    }
    #[inline]
    pub fn hadamard(&self, other: Self) -> Self {
        Self::new(self.x * other.x, self.y * other.y)
    }
}

impl_operator!(Add, add, +);
impl_operator_assign!(AddAssign, add_assign, +=);
impl_operator_float!(Add, add, +);
impl_operator_float_assign!(AddAssign, add_assign, +=);

impl_operator!(Sub, sub, -);
impl_operator_assign!(SubAssign, sub_assign, -=);
impl_operator_float!(Sub, sub, -);
impl_operator_float_assign!(SubAssign, sub_assign, -=);

impl_operator!(Mul, mul, *);
impl_operator_assign!(MulAssign, mul_assign, *=);
impl_operator_float!(Mul, mul, *);
impl_operator_float_assign!(MulAssign, mul_assign, *=);

impl_operator!(Div, div, /);
impl_operator_assign!(DivAssign, div_assign, /=);
impl_operator_float!(Div, div, /);
impl_operator_float_assign!(DivAssign, div_assign, /=);

impl Neg for Vec3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl std::fmt::Display for Vec3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl From<[f32; 3]> for Vec3 {
    fn from(vec: [f32; 3]) -> Self {
        Vec3::new(vec[0], vec[1], vec[2])
    }
}

impl From<[f32; 2]> for Vec2 {
    fn from(vec: [f32; 2]) -> Self {
        Vec2::new(vec[0], vec[1])
    }
}

impl Index<usize> for Vec3 {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "json")]
mod j {
    use super::*;
    use json::JsonValue;
    impl TryFrom<&[JsonValue]> for Vec3 {
        type Error = &'static str;
        fn try_from(value: &[JsonValue]) -> Result<Self, Self::Error> {
            if let [JsonValue::Number(x), JsonValue::Number(y), JsonValue::Number(z)] = value[..] {
                Ok(Vec3::new(x.into(), y.into(), z.into()))
            } else {
                Err("Failed to parse into Vec3")
            }
        }
    }
    impl TryFrom<&JsonValue> for Vec3 {
        type Error = &'static str;
        fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
            if let JsonValue::Array(arr) = value {
                Ok(arr[..].try_into()?)
            } else {
                Err("Failed to parse into Vec3")
            }
        }
    }
    type Quat = Quaternion;
    impl TryFrom<&[JsonValue]> for Quat {
        type Error = &'static str;
        fn try_from(value: &[JsonValue]) -> Result<Self, Self::Error> {
            if let [JsonValue::Number(w), JsonValue::Number(x), JsonValue::Number(y), JsonValue::Number(z)] =
                value[..]
            {
                Ok(Quat::new(w.into(), x.into(), y.into(), z.into()))
            } else {
                Err("Failed to parse into Quat")
            }
        }
    }
    impl TryFrom<&JsonValue> for Quat {
        type Error = &'static str;
        fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
            if let JsonValue::Array(arr) = value {
                Ok(arr[..].try_into()?)
            } else {
                Err("Failed to parse into Quat")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScatterStatus(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialProperties(u8);

impl ScatterStatus {
    pub const NORMAL: Self = Self(0);
    pub const EXIT: Self = Self(1);
    pub const DIRAC_DELTA: Self = Self(1 << 1);
    pub const BTDF: Self = Self(1 << 2);
    pub fn contains(&self, other: Self) -> bool {
        (*self | other) == *self
    }
}

impl BitAnd for ScatterStatus {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}
impl BitOr for ScatterStatus {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl MaterialProperties {
    pub const NORMAL: Self = Self(0);
    pub const ONLY_DIRAC_DELTA: Self = Self(1);
    pub fn contains(&self, other: Self) -> bool {
        (*self | other) == *self
    }
}

impl BitAnd for MaterialProperties {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}
impl BitOr for MaterialProperties {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub trait MinRng {
    fn random(&mut self) -> f32;
    fn random_range(&mut self, range: std::ops::Range<f32>) -> f32;
}

#[cfg(feature = "rand")]
mod r {
    use rand::Rng;
    impl<R: Rng> crate::MinRng for R {
        fn random(&mut self) -> f32 {
            self.random::<f32>()
        }
        fn random_range(&mut self, range: std::ops::Range<f32>) -> f32 {
            self.random_range(range)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Intersection {
    pub t: f32,
    pub uv: Vec2,
    pub pos: Vec3,
    pub nor: Vec3,
    pub out: bool,
    pub mat: usize,
    pub id: usize,
}

impl Intersection {
    pub const NONE: Self = Self {
        t: -1.0,
        uv: Vec2::ZERO,
        pos: Vec3::ZERO,
        nor: Vec3::ZERO,
        out: false,
        mat: 0,
        id: 0,
    };

    pub const fn new(t: f32, uv: Vec2, pos: Vec3, nor: Vec3, out: bool, mat: usize, id: usize) -> Self {
        Self { t, uv, pos, nor, out, mat, id }
    }

    #[allow(clippy::float_cmp)]
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.t == -1.0
    }

    pub fn min(&mut self, other: Self) {
        if self.is_none() || (other.t < self.t && other.t > 0.0) {
            *self = other;
        }
    }
}
