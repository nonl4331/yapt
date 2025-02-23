use std::f32::consts::{FRAC_1_PI, TAU};
use std::ops::{BitAnd, BitOr};

use crate::coord::Coordinate;
use crate::{prelude::*, TEXTURES};

mod ggx;
mod glossy;
mod smooth_conductor;
mod smooth_dielectric;
mod testing;

pub use ggx::Ggx;
pub use glossy::Glossy;
pub use smooth_conductor::SmoothConductor;
pub use smooth_dielectric::SmoothDielectric;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScatterStatus(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialProperties(u8);

impl ScatterStatus {
    pub const NORMAL: Self = Self(0);
    pub const EXIT: Self = Self(1);
    pub const DIRAC_DELTA: Self = Self(1 << 1);
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

#[derive(Debug, new)]
pub enum Mat {
    Matte(Matte),
    Light(Light),
    Metallic(Ggx),
    Glossy(Glossy),
    Refractive(SmoothDielectric),
    Reflective(SmoothConductor),
    Invisible,
}

impl Mat {
    #[must_use]
    pub fn eval(
        &self,
        sect: &Intersection,
        mut wo: Vec3,
        mut wi: Vec3,
        status: ScatterStatus,
    ) -> Vec3 {
        let texs = unsafe { TEXTURES.get().as_ref_unchecked() };
        if self.requires_local_space() {
            (wo, wi) = Self::to_local_space(sect, wo, wi);
        }

        match self {
            // cos pdf and weakening factor cancel out
            Self::Matte(m) => texs[m.albedo].uv_value(sect.uv),
            Self::Glossy(m) => m.eval(sect, wi, wo, status),
            Self::Light(_) | Self::Invisible => unreachable!(),
            Self::Metallic(m) => m.eval(wo, wi, sect),
            Self::Refractive(_) => Vec3::ONE,
            Self::Reflective(m) => m.eval(wo, wi, sect),
        }
    }
    pub fn scatter(
        &self,
        sect: &Intersection,
        ray: &mut Ray,
        rng: &mut impl MinRng,
    ) -> ScatterStatus {
        match self {
            Self::Matte(_) => Matte::scatter(ray, sect, rng),
            Self::Light(_) => ScatterStatus::EXIT,
            Self::Invisible => unreachable!(),
            Self::Metallic(m) => m.scatter(sect, ray, rng),
            Self::Glossy(m) => m.scatter(sect, ray, rng),
            Self::Refractive(m) => m.scatter(sect, ray, rng),
            Self::Reflective(m) => m.scatter(sect, ray),
        }
    }
    pub const fn properties(&self) -> MaterialProperties {
        match self {
            Self::Refractive(_) | Self::Reflective(_) => MaterialProperties::ONLY_DIRAC_DELTA,
            _ => MaterialProperties::NORMAL,
        }
    }
    pub fn uv_intersect(&self, uv: Vec2, rng: &mut impl MinRng) -> bool {
        let texs = unsafe { TEXTURES.get().as_ref_unchecked() };

        match self {
            Self::Invisible => false,
            Self::Metallic(m) => texs[m.f0].does_intersect(uv, rng),
            Self::Reflective(m) => texs[m.f0].does_intersect(uv, rng),
            _ => true,
        }
    }
    #[must_use]
    pub fn le(&self) -> Vec3 {
        match self {
            Self::Matte(_)
            | Self::Metallic(_)
            | Self::Refractive(_)
            | Self::Reflective(_)
            | Self::Invisible
            | Self::Glossy(_) => Vec3::ZERO,
            Self::Light(l) => l.irradiance,
        }
    }
    // scattering pdf
    #[must_use]
    pub fn spdf(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> f32 {
        // wo should be pointing away from the surface for BRDFs
        if self.requires_local_space() {
            (wo, wi) = Self::to_local_space(sect, wo, wi);
        }
        match self {
            Self::Matte(_) => Matte::pdf(wi, sect.nor),
            Self::Light(_) => 0.0,
            Self::Metallic(m) => m.pdf(wo, wi, sect),
            Self::Glossy(m) => m.pdf(sect, wi, wo),
            Self::Invisible | Self::Refractive(_) | Self::Reflective(_) => unreachable!(),
        }
    }
    #[must_use]
    pub fn bxdf_cos(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> Vec3 {
        if self.requires_local_space() {
            (wo, wi) = Self::to_local_space(sect, wo, wi);
        }
        match self {
            Self::Matte(m) => m.bxdf_cos(sect, wo, wi),
            Self::Light(_) | Self::Invisible | Self::Refractive(_) | Self::Reflective(_) => {
                unreachable!()
            }
            Self::Metallic(m) => m.bxdf_cos(wo, wi, sect),
            Self::Glossy(m) => m.bxdf_cos(sect, wi, wo),
        }
    }
    #[must_use]
    fn requires_local_space(&self) -> bool {
        match self {
            Self::Matte(_)
            | Self::Light(_)
            | Self::Refractive(_)
            | Self::Glossy(_)
            | Self::Reflective(_) => false,
            Self::Metallic(_) => true,
            Self::Invisible => unreachable!(),
        }
    }
    #[must_use]
    fn to_local_space(sect: &Intersection, wo: Vec3, wi: Vec3) -> (Vec3, Vec3) {
        let coord = crate::coord::Coordinate::new_from_z(sect.nor);
        (coord.global_to_local(wo), coord.global_to_local(wi))
    }
}

#[derive(Debug, new)]
pub struct Matte {
    pub albedo: usize,
}

impl Matte {
    pub fn scatter(ray: &mut Ray, sect: &Intersection, rng: &mut impl MinRng) -> ScatterStatus {
        let dir = Self::sample(sect.nor, rng);
        *ray = Ray::new(sect.pos, dir.normalised());
        ScatterStatus::NORMAL
    }
    #[must_use]
    fn sample_local(rng: &mut impl MinRng) -> Vec3 {
        let cos_theta = rng.gen().sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
        let phi = TAU * rng.gen();
        Vec3::new(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta)
    }
    #[must_use]
    pub fn sample(normal: Vec3, rng: &mut impl MinRng) -> Vec3 {
        Coordinate::new_from_z(normal).local_to_global(Self::sample_local(rng))
    }
    #[must_use]
    pub fn pdf(outgoing: Vec3, normal: Vec3) -> f32 {
        outgoing.dot(normal).max(0.0) * FRAC_1_PI
    }
    #[must_use]
    pub fn bxdf_cos(&self, sect: &Intersection, _: Vec3, wi: Vec3) -> Vec3 {
        self.albedo(sect.uv) * wi.dot(sect.nor).max(0.0) * FRAC_1_PI
    }
    #[must_use]
    pub fn albedo(&self, uv: Vec2) -> Vec3 {
        let texs = unsafe { TEXTURES.get().as_ref_unchecked() };
        texs[self.albedo].uv_value(uv)
    }
}

#[derive(Debug, new)]
pub struct Light {
    irradiance: Vec3,
}
