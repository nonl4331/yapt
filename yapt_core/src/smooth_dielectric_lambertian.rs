use super::*;

#[derive(Debug)]
pub struct SmoothDielectricLambertian<T: TextureHandler> {
    pub ior: f32,
    albedo: T,
    eta_sq: f32,
    ri_average: f32,
}

impl<T: TextureHandler> SmoothDielectricLambertian<T> {
    pub fn new(ior: f32, albedo: T) -> Material<T> {
        Material::Glossy(Self::new_raw(ior, albedo))
    }
    pub fn new_raw(ior: f32, albedo: T) -> Self {
        let ni = ior;
        let ni2 = ni.powi(2);
        let ni4 = ni2.powi(2);
        let re_average = 0.5
            + ((ni - 1.0) * (3.0 * ni + 1.0)) / (6.0 * (ni + 1.0).powi(2))
            + (ni2 * (ni2 - 1.0).powi(2)) / (ni2 + 1.0).powi(3) * ((ni - 1.0) / (ni + 1.0)).ln()
            - (2.0 * ni2 * ni * (ni2 + 2.0 * ni - 1.0)) / ((ni2 + 1.0) * (ni4 - 1.0))
            + (8.0 * ni4 * (ni4 + 1.0)) / ((ni2 + 1.0) * (ni4 - 1.0).powi(2)) * ni.ln();
        let mut ri_average = 1.0 - (1.0 / ni2) * (1.0 - re_average);
        // avoid NAN with ln(0) above
        if (ior - 1.0).abs() < 0.000001 {
            ri_average = 0.0;
        }
        Self {
            ior,
            albedo,
            eta_sq: (1.0 / ior).powi(2),
            ri_average,
        }
    }

    pub fn scatter(
        &self,
        sect: &Intersection,
        ray: &mut Ray,
        rng: &mut impl MinRng,
    ) -> ScatterStatus {
        // by convention both wi and wo are pointing away from the surface
        let wo = -ray.dir;
        let r = super::fresnel_dielectric(1.0, self.ior, sect.nor, wo);

        if rng.random() > r {
            let cos_theta = rng.random().sqrt();
            let sin_theta = (1.0 - cos_theta.powi(2)).sqrt();
            let phi = TAU * rng.random();
            let local_wi = Vec3::new(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta);

            let wi = Coordinate::new_from_z(sect.nor).local_to_global(local_wi);
            *ray = Ray::new(sect.pos, wi);
            ScatterStatus::NORMAL
        } else {
            let wi = wo.reflected(sect.nor);
            *ray = Ray::new(sect.pos, wi);
            ScatterStatus::DIRAC_DELTA
        }
    }
    // should never be reached with dirac delta scatter
    pub fn bxdf_cos(&self, sect: &Intersection, wi: Vec3, wo: Vec3) -> Vec3 {
        let fi = super::fresnel_dielectric(1.0, self.ior, sect.nor, wo);
        let fo = super::fresnel_dielectric(1.0, self.ior, sect.nor, wi);

        let a = self.get_albedo(sect);

        self.eta_sq * (1.0 - fi) * a * FRAC_1_PI * (1.0 - fo) * wi.dot(sect.nor).max(0.0)
            / (1.0 - self.ri_average * a)
    }
    // should never be reached with dirac delta scatter
    pub fn pdf(&self, sect: &Intersection, wi: Vec3, wo: Vec3) -> f32 {
        let fi = super::fresnel_dielectric(1.0, self.ior, sect.nor, wo);

        (1.0 - fi) * wi.dot(sect.nor).max(0.0) * FRAC_1_PI
    }
    // the simplified case where you are evaluations BRDF * COS / PDF
    pub fn eval(&self, sect: &Intersection, wi: Vec3, _: Vec3, status: ScatterStatus) -> Vec3 {
        let a = self.get_albedo(sect);
        let fo = super::fresnel_dielectric(1.0, self.ior, sect.nor, wi);

        if status.contains(ScatterStatus::DIRAC_DELTA) {
            return Vec3::ONE;
        }

        self.eta_sq * a * (1.0 - fo) / (1.0 - self.ri_average * a)
    }
    #[must_use]
    pub fn get_albedo(&self, sect: &Intersection) -> Vec3 {
        self.albedo.uv_value(sect.uv)
    }
}
