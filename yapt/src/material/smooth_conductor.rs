pub use crate::prelude::*;

#[derive(Debug)]
pub struct SmoothConductor {
    pub f0: usize,
}

impl SmoothConductor {
    pub fn new(f0: usize) -> Mat {
        Mat::Reflective(Self { f0 })
    }
    // see https://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray) -> ScatterStatus {
        let wo = -ray.dir;

        let wi = wo.reflected(sect.nor);
        let origin = sect.pos + 0.00001 * sect.nor;
        *ray = Ray::new(origin, wi);
        ScatterStatus::DIRAC_DELTA
    }
    pub fn eval(&self, wo: Vec3, _: Vec3, sect: &Intersection) -> Vec3 {
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        let f0 = texs[self.f0].uv_value(sect.uv);
        super::fresnel_conductor(f0, sect.nor.dot(wo))
    }
}
