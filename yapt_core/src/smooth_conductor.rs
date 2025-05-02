use super::*;

#[derive(Debug)]
pub struct SmoothConductor<T: TextureHandler> {
    pub f0: T,
}

impl<T: TextureHandler> SmoothConductor<T> {
    pub fn new(f0: T) -> Material<T> {
        Material::Reflective(Self { f0 })
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
        let f0 = self.f0.uv_value(sect.uv);
        super::fresnel_conductor(f0, sect.nor.dot(wo))
    }
}
