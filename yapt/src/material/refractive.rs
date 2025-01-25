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
    // see https://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
    pub fn scatter(
        &self,
        sect: &Intersection,
        ray: &mut Ray,
        rng: &mut impl MinRng,
    ) -> ScatterStatus {
        let wo = -ray.dir;

        let mut reflect = || {
            let wi = wo.reflected(sect.nor);
            let origin = sect.pos + 0.00001 * sect.nor;
            *ray = Ray::new(origin, wi);
            ScatterStatus::DIRAC_DELTA
        };

        let mut eta1 = 1.0;
        let mut eta2 = self.ior;

        if !sect.out {
            std::mem::swap(&mut eta1, &mut eta2);
        }
        let eta = eta1 / eta2;

        let cosi = wo.dot(sect.nor);

        let sint_sq = eta.powi(2) * (1.0 - cosi.powi(2));
        let is_tir = sint_sq >= 1.0;
        if is_tir {
            return reflect();
        }

        let cost = (1.0 - sint_sq).sqrt();

        let rs = ((eta1 * cosi - eta2 * cost) / (eta1 * cosi + eta2 * cost)).powi(2);
        let rp = ((eta1 * cost - eta2 * cosi) / (eta1 * cost + eta2 * cosi)).powi(2);
        let r = 0.5 * (rs + rp);

        if r > rng.gen() {
            return reflect();
        }

        // refract
        let perp = eta * (ray.dir + cosi * sect.nor);
        let para = -(1.0 - perp.mag_sq()).abs().sqrt() * sect.nor;
        let wi = perp + para;
        let origin = sect.pos - 0.00001 * sect.nor;
        *ray = Ray::new(origin, wi);

        ScatterStatus::DIRAC_DELTA
    }
}
