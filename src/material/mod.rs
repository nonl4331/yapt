use std::f32::consts::{FRAC_1_PI, FRAC_PI_2, FRAC_PI_4, TAU};

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
    pub fn eval(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> Vec3 {
        wo = -wo;
        if self.requires_local_space() {
            (wo, wi) = self.to_local_space(sect, wo, wi);
        }

        match self {
            // cos pdf and weakening factor cancel out
            Self::Matte(m) => m.albedo,
            Self::Light(_) => unreachable!(),
            Self::Glossy(m) => m.eval(sect, wo, wi),
        }
    }
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray, rng: &mut impl MinRng) -> bool {
        match self {
            Self::Matte(_) => Matte::scatter(ray, sect, rng),
            Self::Light(_) => true,
            Self::Glossy(m) => m.scatter(sect, ray, rng),
        }
    }
    pub fn le(&self, _pos: Vec3, _wo: Vec3) -> Vec3 {
        match self {
            Self::Matte(_) | Self::Glossy(_) => Vec3::ZERO,
            Self::Light(l) => l.irradiance,
        }
    }
    // scattering pdf
    pub fn spdf(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> f32 {
        // wo should be pointing away from the surface for BRDFs
        wo = -wo;
        if self.requires_local_space() {
            (wo, wi) = self.to_local_space(sect, wo, wi);
        }
        match self {
            Self::Matte(_) => Matte::pdf(wi, sect.nor),
            Self::Light(_) => 0.0,
            Self::Glossy(m) => m.pdf(wo, wi),
        }
    }
    pub fn bxdf_cos(&self, sect: &Intersection, mut wo: Vec3, mut wi: Vec3) -> Vec3 {
        wo = -wo;
        if self.requires_local_space() {
            (wo, wi) = self.to_local_space(sect, wo, wi);
        }
        match self {
            Self::Matte(m) => m.albedo * wi.dot(sect.nor).max(0.0) * FRAC_1_PI,
            Self::Light(_) => unreachable!(),
            Self::Glossy(m) => m.bxdf_cos(wo, wi),
            _ => todo!(),
        }
    }
    fn requires_local_space(&self) -> bool {
        match self {
            Self::Matte(_) | Self::Light(_) => false,
            Self::Glossy(_) => true,
        }
    }
    fn to_local_space(&self, sect: &Intersection, wo: Vec3, wi: Vec3) -> (Vec3, Vec3) {
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
    fn sample_local(rng: &mut impl MinRng) -> Vec3 {
        let cos_theta = rng.gen().sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
        let phi = TAU * rng.gen();
        Vec3::new(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta)
    }
    pub fn sample(normal: Vec3, rng: &mut impl MinRng) -> Vec3 {
        Coordinate::new_from_z(normal).local_to_global(Self::sample_local(rng))
    }

    pub fn pdf(outgoing: Vec3, normal: Vec3) -> f32 {
        outgoing.dot(normal).max(0.0) * FRAC_1_PI
    }
}

fn random_vec3(rng: &mut impl MinRng) -> Vec3 {
    Vec3::new(rng.gen() - 0.5, rng.gen() - 0.5, rng.gen() - 0.5)
}

fn random_in_unit_sphere(rng: &mut impl MinRng) -> Vec3 {
    loop {
        let p = random_vec3(rng);
        if p.mag_sq() < 1.0 {
            return p;
        }
    }
}

fn concentric_disc_sampling(rng: &mut impl MinRng) -> Vec2 {
    let offset = Vec2::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0));
    if offset.x == 0.0 || offset.y == 0.0 {
        return Vec2::new(0.0, 0.0);
    }
    let (theta, r);
    if offset.x.abs() > offset.y.abs() {
        r = offset.x;
        theta = FRAC_PI_4 * offset.y / offset.x;
    } else {
        r = offset.y;
        theta = FRAC_PI_2 - FRAC_PI_4 * offset.x / offset.y;
    }
    r * Vec2::new(theta.cos(), theta.sin())
}

fn cosine_hemisphere_sampling(rng: &mut impl MinRng) -> Vec3 {
    let d = concentric_disc_sampling(rng);
    let z = (1.0 - d.mag_sq()).max(0.0).sqrt();
    Vec3::new(d.x, d.y, z)
}

#[derive(Debug, new)]
pub struct Light {
    irradiance: Vec3,
}
