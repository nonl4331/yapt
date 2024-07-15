use std::f32::consts::{FRAC_1_PI, TAU};

use crate::coord::Coordinate;
use crate::prelude::*;

mod ggx;
mod testing;

pub use ggx::Ggx;

#[derive(Debug, new)]
pub enum Mat {
    Matte(Matte),
    Light(Light),
    Glossy(Ggx),
}

impl Mat {
    #[must_use]
    pub fn eval(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> Vec3 {
        wo = -wo;
        if self.requires_local_space() {
            (wo, wi) = Self::to_local_space(sect, wo, wi);
        }

        match self {
            // cos pdf and weakening factor cancel out
            Self::Matte(m) => m.albedo,
            Self::Light(_) => unreachable!(),
            Self::Glossy(m) => m.eval(wo, wi),
        }
    }
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray, rng: &mut impl MinRng) -> bool {
        match self {
            Self::Matte(_) => Matte::scatter(ray, sect, rng),
            Self::Light(_) => true,
            Self::Glossy(m) => m.scatter(sect, ray, rng),
        }
    }
    #[must_use]
    pub fn le(&self, _pos: Vec3, _wo: Vec3) -> Vec3 {
        match self {
            Self::Matte(_) | Self::Glossy(_) => Vec3::ZERO,
            Self::Light(l) => l.irradiance,
        }
    }
    // scattering pdf
    #[must_use]
    pub fn spdf(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> f32 {
        // wo should be pointing away from the surface for BRDFs
        wo = -wo;
        if self.requires_local_space() {
            (wo, wi) = Self::to_local_space(sect, wo, wi);
        }
        match self {
            Self::Matte(_) => Matte::pdf(wi, sect.nor),
            Self::Light(_) => 0.0,
            Self::Glossy(m) => m.pdf(wo, wi),
        }
    }
    #[must_use]
    pub fn bxdf_cos(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> Vec3 {
        wo = -wo;
        if self.requires_local_space() {
            (wo, wi) = Self::to_local_space(sect, wo, wi);
        }
        match self {
            Self::Matte(m) => m.albedo * wi.dot(sect.nor).max(0.0) * FRAC_1_PI,
            Self::Light(_) => unreachable!(),
            Self::Glossy(m) => m.bxdf_cos(wo, wi),
        }
    }
    fn requires_local_space(&self) -> bool {
        match self {
            Self::Matte(_) | Self::Light(_) => false,
            Self::Glossy(_) => true,
        }
    }
    fn to_local_space(sect: &Intersection, wo: Vec3, wi: Vec3) -> (Vec3, Vec3) {
        let coord = crate::coord::Coordinate::new_from_z(sect.nor);
        (coord.global_to_local(wo), coord.global_to_local(wi))
    }
}

#[derive(Debug, new)]
pub struct Matte {
    pub albedo: Vec3,
}

impl Matte {
    pub fn scatter(ray: &mut Ray, sect: &Intersection, rng: &mut impl MinRng) -> bool {
        let dir = Self::sample(sect.nor, rng);
        *ray = Ray::new(sect.pos, dir.normalised());
        false
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
}

#[derive(Debug, new)]
pub struct Light {
    irradiance: Vec3,
}
