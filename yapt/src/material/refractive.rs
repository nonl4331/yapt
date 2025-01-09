pub use crate::prelude::*;

// perfect refractive top layer + lambertian bottom layer
#[derive(Debug)]
pub struct Refractive {
    pub ior: f32,
}

impl Refractive {
    pub fn new(ior: f32) -> Self {
        Self { ior }
    }
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray, rng: &mut impl MinRng) -> bool {
        let eta = if sect.out { 1.0 / self.ior } else { self.ior };
        let wo = -ray.dir;

        let cos = wo.dot(sect.nor);
        let sin = (1.0 - cos.powi(2)).sqrt();
        let is_tir = eta * sin > 1.0;
        let r0 = ((1.0 - eta) / (1.0 + eta)).powi(2);

        let (origin, wi);

        let r = r0 + (1.0 - r0) * (1.0 - cos).powi(5);

        if is_tir || r > rng.gen() {
            // reflect
            wi = wo.reflected(sect.nor);
            origin = sect.pos + 0.00001 * sect.nor;
        } else {
            // refract
            let perp = eta * (ray.dir + cos * sect.nor);
            let para = -(1.0 - perp.mag_sq()).abs().sqrt() * sect.nor;
            wi = perp + para;

            origin = sect.pos - 0.00001 * sect.nor;
        }
        *ray = Ray::new(origin, wi);
        false
    }
}
