use crate::prelude::*;

#[derive(Debug)]
pub struct Glossy {
    ior: f32,
    albedo: usize,
    eta_sq: f32,
    ri_average: f32,
}

impl Glossy {
    pub fn new(ior: f32, albedo: usize) -> Self {
        let ni = ior;
        let ni2 = ni.powi(2);
        let ni4 = ni2.powi(2);
        let re_average = 0.5
            + ((ni - 1.0) * (3.0 * ni + 1.0)) / (6.0 * (ni + 1.0).powi(2))
            + (ni2 * (ni2 - 1.0).powi(2)) / (ni2 + 1.0).powi(3) * ((ni - 1.0) / (ni + 1.0)).ln()
            - (2.0 * ni2 * ni * (ni2 + 2.0 * ni - 1.0)) / ((ni2 + 1.0) * (ni4 - 1.0))
            + (8.0 * ni4 * (ni4 + 1.0)) / ((ni2 + 1.0) * (ni4 - 1.0).powi(2)) * ni.ln();
        let ri_average = 1.0 - (1.0 / ni2) * (1.0 - re_average);
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
        let r = self.fresnel_reflectance(sect, wo);
        let origin = sect.pos + 0.00001 * sect.nor;

        if rng.gen() > r {
            let cos_theta = rng.gen().sqrt();
            let sin_theta = (1.0 - cos_theta.powi(2)).sqrt();
            let phi = TAU * rng.gen();
            let local_wi = Vec3::new(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta);

            let wi = Coordinate::new_from_z(sect.nor).local_to_global(local_wi);
            *ray = Ray::new(origin, wi);
            ScatterStatus::NORMAL
        } else {
            let wi = wo.reflected(sect.nor);
            *ray = Ray::new(origin, wi);
            ScatterStatus::DIRAC_DELTA
        }
    }
    // should never be reached with dirac delta scatter
    pub fn bxdf_cos(&self, sect: &Intersection, wi: Vec3, wo: Vec3) -> Vec3 {
        let fi = self.fresnel_reflectance(sect, wo);
        let fo = self.fresnel_reflectance(sect, wi);

        let a = self.get_albedo(sect);

        self.eta_sq * (1.0 - fi) * a * FRAC_1_PI * (1.0 - fo) * sect.nor.dot(wi).max(0.0)
            / (1.0 - self.ri_average * a)
    }
    // should never be reached with dirac delta scatter
    pub fn pdf(&self, sect: &Intersection, wi: Vec3, wo: Vec3) -> f32 {
        let fi = self.fresnel_reflectance(sect, wo);

        (1.0 - fi) * wi.dot(sect.nor).max(0.0) * FRAC_1_PI
    }
    // the simplified case where you are evaluations BRDF * COS / PDF
    pub fn eval(&self, sect: &Intersection, wi: Vec3, _: Vec3, status: ScatterStatus) -> Vec3 {
        let a = self.get_albedo(sect);
        let fo = self.fresnel_reflectance(sect, wi);

        if status.contains(ScatterStatus::DIRAC_DELTA) {
            return Vec3::ONE;
        }

        self.eta_sq * a * (1.0 - fo) / (1.0 - self.ri_average)
    }
    #[must_use]
    pub fn get_albedo(&self, sect: &Intersection) -> Vec3 {
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        texs[self.albedo].uv_value(sect.uv)
    }
    fn fresnel_reflectance(&self, sect: &Intersection, w: Vec3) -> f32 {
        let cosi = w.dot(sect.nor);

        let sint_sq = self.eta_sq * (1.0 - cosi.powi(2));
        let is_tir = sint_sq >= 1.0;
        if is_tir {
            return 1.0;
        }

        let cost = (1.0 - sint_sq).sqrt();

        let eta1 = 1.0;
        let eta2 = self.ior;

        let rs = ((eta1 * cosi - eta2 * cost) / (eta1 * cosi + eta2 * cost)).powi(2);
        let rp = ((eta1 * cost - eta2 * cosi) / (eta1 * cost + eta2 * cosi)).powi(2);
        0.5 * (rs + rp)
    }
}
