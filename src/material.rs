use std::f32::consts::FRAC_1_PI;

use rand::Rng;

use crate::prelude::*;

#[derive(Debug, new)]
pub enum Mat {
    Matte(Matte),
    Light(Light),
}

impl Mat {
    pub fn eval(&self, _sect: &Intersection, _wo: Vec3, _wi: Vec3) -> Vec3 {
        match self {
            // cos pdf and weakening factor cancel out
            Self::Matte(m) => m.albedo,
            Self::Light(_) => unreachable!(),
        }
    }
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray, rng: &mut impl Rng) -> bool {
        match self {
            Self::Matte(_) => Matte::scatter(ray, sect, rng),
            Self::Light(_) => true,
        }
    }
    pub fn le(&self, _pos: Vec3, _wo: Vec3) -> Vec3 {
        match self {
            Self::Matte(_) => Vec3::ZERO,
            Self::Light(l) => l.irradiance,
        }
    }
    // scattering pdf
    pub fn spdf(&self, sect: &Intersection, wi: Vec3) -> f32 {
        match self {
            Self::Matte(_) => wi.dot(sect.nor).max(0.0) * FRAC_1_PI,
            Self::Light(_) => unreachable!(),
        }
    }
    pub fn bxdf_cos(&self, sect: &Intersection, _wo: Vec3, wi: Vec3) -> Vec3 {
        match self {
            Self::Matte(m) => m.albedo * wi.dot(sect.nor).max(0.0) * FRAC_1_PI,
            Self::Light(_) => unreachable!(),
        }
    }
}

#[derive(Debug, new)]
pub struct Matte {
    pub albedo: Vec3,
}

impl Matte {
    pub fn scatter(ray: &mut Ray, sect: &Intersection, rng: &mut impl Rng) -> bool {
        let dir = random_in_unit_sphere(rng) + sect.nor;
        *ray = Ray::new(sect.pos, dir.normalised());
        false
    }
}

fn random_vec3(rng: &mut impl Rng) -> Vec3 {
    Vec3::new(
        rng.gen::<f32>() - 0.5,
        rng.gen::<f32>() - 0.5,
        rng.gen::<f32>() - 0.5,
    )
}

fn random_in_unit_sphere(rng: &mut impl Rng) -> Vec3 {
    loop {
        let p = random_vec3(rng);
        if p.mag_sq() < 1.0 {
            return p;
        }
    }
}

#[derive(Debug, new)]
pub struct Light {
    irradiance: Vec3,
}
