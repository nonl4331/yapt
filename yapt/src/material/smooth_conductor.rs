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
        self.f(wo.dot(sect.nor), sect.uv)
    }
    // fresnel
    // due to RGB rendering use shlick's approximation
    // https://diglib.eg.org/server/api/core/bitstreams/726dc384-d7dd-4c0e-8806-eadec0ff3886/content
    #[must_use]
    pub fn f(&self, cos_theta: f32, uv: Vec2) -> Vec3 {
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        let ior = texs[self.f0].uv_value(uv);
        ior + (1.0 - ior) * (1.0 - cos_theta).powi(5)
    }
}
