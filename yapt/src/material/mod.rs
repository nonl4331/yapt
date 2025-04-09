use std::f32::consts::{FRAC_1_PI, TAU};
use std::ops::{BitAnd, BitOr};

use crate::coord::Coordinate;
use crate::{prelude::*, TEXTURES};

mod rough_conductor;
mod rough_dielectric;
mod smooth_conductor;
mod smooth_dielectric;
mod smooth_dielectric_lambertian;
mod testing;

pub use rough_conductor::RoughConductor;
pub use rough_dielectric::RoughDielectric;
pub use smooth_conductor::SmoothConductor;
pub use smooth_dielectric::SmoothDielectric;
pub use smooth_dielectric_lambertian::SmoothDielectricLambertian;

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
    Matte(Lambertian),
    Light(Light),
    Metallic(RoughConductor),
    Glossy(SmoothDielectricLambertian),
    Refractive(SmoothDielectric),
    RoughRefractive(RoughDielectric),
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
            Self::RoughRefractive(m) => m.eval(wo, wi, sect),
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
            Self::Matte(_) => Lambertian::scatter(ray, sect, rng),
            Self::Light(_) => ScatterStatus::EXIT,
            Self::Invisible => unreachable!(),
            Self::Metallic(m) => m.scatter(sect, ray, rng),
            Self::Glossy(m) => m.scatter(sect, ray, rng),
            Self::Refractive(m) => m.scatter(sect, ray, rng),
            Self::RoughRefractive(m) => m.scatter(sect, ray, rng),
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
            | Self::RoughRefractive(_)
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
            Self::Matte(_) => Lambertian::pdf(wi, sect.nor),
            Self::Light(_) => 0.0,
            Self::Metallic(m) => m.pdf(wo, wi, sect),
            Self::RoughRefractive(m) => m.pdf(wo, wi, sect),
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
            Self::RoughRefractive(m) => m.bxdf_cos(wo, wi, sect),
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
            Self::Metallic(_) | Self::RoughRefractive(_) => true,
            Self::Invisible => unreachable!(),
        }
    }
    #[must_use]
    pub fn to_local_space(sect: &Intersection, wo: Vec3, wi: Vec3) -> (Vec3, Vec3) {
        let coord = crate::coord::Coordinate::new_from_z(sect.nor);
        (coord.global_to_local(wo), coord.global_to_local(wi))
    }
}

#[derive(Debug)]
pub struct Lambertian {
    pub albedo: usize,
}

impl Lambertian {
    pub fn new(albedo: usize) -> Mat {
        Mat::Matte(Self { albedo })
    }
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

#[derive(Debug)]
pub struct Light {
    irradiance: Vec3,
}

impl Light {
    pub fn new(irradiance: Vec3) -> Mat {
        Mat::Light(Self { irradiance })
    }
}

// fresnel dielectric
// eta1 = outer ior, eta2 = inner ior
#[must_use]
#[inline(always)]
pub fn fresnel_dielectric(eta1: f32, eta2: f32, nor: Vec3, wo: Vec3) -> f32 {
    let eta = eta1 / eta2;

    let cosi = wo.dot(nor);

    let sint_sq = eta.powi(2) * (1.0 - cosi.powi(2));
    let is_tir = sint_sq >= 1.0;
    if is_tir {
        return 1.0;
    }

    let cost = (1.0 - sint_sq).sqrt();

    let rs = ((eta1 * cosi - eta2 * cost) / (eta1 * cosi + eta2 * cost)).powi(2);
    let rp = ((eta1 * cost - eta2 * cosi) / (eta1 * cost + eta2 * cosi)).powi(2);

    0.5 * (rs + rp)
}

// fresnel conductor
// due to RGB rendering use shlick's approximation
// https://diglib.eg.org/server/api/core/bitstreams/726dc384-d7dd-4c0e-8806-eadec0ff3886/content
#[must_use]
#[inline(always)]
pub fn fresnel_conductor(f0: Vec3, cos: f32) -> Vec3 {
    f0 + (1.0 - f0) * (1.0 - cos).powi(5)
}
