use super::*;

#[derive(Debug)]
pub struct SmoothDielectric {
    pub ior: f32,
}

impl SmoothDielectric {
    pub fn new<T: TextureHandler>(ior: f32) -> Material<T> {
        Material::Refractive(Self { ior })
    }
    // see https://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
    pub fn scatter(
        &self,
        sect: &Intersection,
        ray: &mut Ray,
        rng: &mut impl MinRng,
    ) -> ScatterStatus {
        let wo = -ray.dir;

        let mut eta1 = 1.0;
        let mut eta2 = self.ior;

        if !sect.out {
            std::mem::swap(&mut eta1, &mut eta2);
        }
        let eta = eta1 / eta2;

        let cosi = wo.dot(sect.nor);

        let r = super::fresnel_dielectric(eta1, eta2, sect.nor, wo);

        // reflect
        if r >= rng.random() {
            let wi = wo.reflected(sect.nor);
            let origin = sect.pos + 0.00001 * sect.nor;
            *ray = Ray::new(origin, wi);
            return ScatterStatus::DIRAC_DELTA;
        }

        // refract
        let perp = eta * (cosi * sect.nor - wo);
        let para = -(1.0 - perp.mag_sq()).abs().sqrt() * sect.nor;
        let wi = perp + para;
        let origin = sect.pos - 0.00001 * sect.nor;
        *ray = Ray::new(origin, wi);

        ScatterStatus::DIRAC_DELTA
    }
}
