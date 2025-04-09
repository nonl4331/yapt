use crate::prelude::*;

// uses GGX
#[derive(Debug)]
pub struct RoughConductor {
    pub roughness: usize,
    pub f0: usize,
}

impl RoughConductor {
    #[must_use]
    pub fn new(roughness: usize, f0: usize) -> Mat {
        Mat::Metallic(Self { roughness, f0 })
    }
    pub fn new_raw(roughness: usize, f0: usize) -> Self {
        Self { roughness, f0 }
    }
    #[must_use]
    pub fn scatter(
        &self,
        sect: &Intersection,
        ray: &mut Ray,
        rng: &mut impl MinRng,
    ) -> ScatterStatus {
        // by convention points away from surface hence the -ray.dir (section 2, definition)
        *ray = Ray::new(sect.pos, self.sample(sect, -ray.dir, rng));
        ScatterStatus::NORMAL
    }
    #[must_use]
    pub fn sample(&self, sect: &Intersection, mut wo: Vec3, rng: &mut impl MinRng) -> Vec3 {
        let a = self.get_a(sect);

        let coord = crate::coord::Coordinate::new_from_z(sect.nor);
        wo = coord.global_to_local(wo);
        let wm = self.sample_vndf_local(a, wo, rng);
        let wi = wo.reflected(wm);
        coord.local_to_global(wi).normalised()
    }
    #[must_use]
    pub fn eval(&self, wo: Vec3, wi: Vec3, sect: &Intersection) -> Vec3 {
        let a = self.get_a(sect);
        let a_sq = a.powi(2);
        let wm = (wo + wi).normalised();

        // f * g2 / g1 (Heitz2018GGX 19)
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        let f0 = texs[self.f0].uv_value(sect.uv);
        let f = super::fresnel_conductor(f0, wm.dot(wo));

        let g2 = self.g2_local(a_sq, wo, wi, wm);
        let g1 = self.g1_local(a_sq, wo, wm);
        if g1 == 0.0 {
            return Vec3::ZERO;
        }
        f * g2 / g1
    }
    #[must_use]
    pub fn bxdf_cos(&self, wo: Vec3, wi: Vec3, sect: &Intersection) -> Vec3 {
        let a_sq = self.get_a(sect).powi(2);

        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        let f0 = texs[self.f0].uv_value(sect.uv);

        let wm = (wo + wi).normalised();
        let f = super::fresnel_conductor(f0, wm.dot(wo));

        f * self.ndf_local(a_sq, wm) * self.g2_local(a_sq, wo, wi, wm) / (4.0 * wo.z)
    }
    // local space (hemisphere on z=0 plane see section 2, definition)
    #[must_use]
    pub fn sample_vndf_local(&self, a: f32, in_w: Vec3, rng: &mut impl MinRng) -> Vec3 {
        // map episoid to unit hemisphere (section 2, importance sampling 1)
        let in_w = Vec3::new(a * in_w.x, a * in_w.y, in_w.z).normalised();

        // intersect unit hemisphere based on new in_w and record point (section 2, important
        // sampling 2)
        let p_hemi = Self::sample_vndf_hemisphere(in_w, rng);

        // transform intersection point back (section 2, importance sampling 3)
        Vec3::new(p_hemi.x * a, p_hemi.y * a, p_hemi.z).normalised()
        // see pbrt v4 9.6.4 for why  * not /
    }
    // (section 3, listing 3)
    #[must_use]
    fn sample_vndf_hemisphere(in_w_hemi: Vec3, rng: &mut impl MinRng) -> Vec3 {
        let phi = TAU * rng.gen();
        // can replace (1.0 - x) with x?
        let z = (1.0 - rng.gen()) * (1.0 + in_w_hemi.z) - in_w_hemi.z;
        let sin_theta = (1.0 - z.powi(2)).clamp(0.0, 1.0).sqrt();
        let c = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), z);
        c + in_w_hemi
    }
    // by convention points away from surface (section 2, definition)
    #[must_use]
    pub fn pdf(&self, wo: Vec3, wi: Vec3, sect: &Intersection) -> f32 {
        let a = self.get_a(sect);

        let mut wm = (wo + wi).normalised();
        if wm.z < 0.0 {
            wm = -wm;
        }
        // Heitz2018GGX (17)
        self.vndf_local(a.powi(2), wm, wo) / (4.0 * wo.dot(wm))
    }
    // visible normal distribution function
    // this is a valid PDF
    // wo is camera ray
    #[must_use]
    pub fn vndf_local(&self, a_sq: f32, wm: Vec3, wo: Vec3) -> f32 {
        if wm.z < 0.0 {
            return 0.0;
        }
        self.g1_local(a_sq, wo, wm) * wo.dot(wm).max(0.0) * self.ndf_local(a_sq, wm) / wo.z.abs()
        // see pbrt v4
    }
    // normal distribution function
    #[must_use]
    pub fn ndf_local(&self, a_sq: f32, wm: Vec3) -> f32 {
        if wm.z <= 0.0 {
            return 0.0;
        }
        let tmp = wm.z.powi(2) * (a_sq - 1.0) + 1.0;
        a_sq * FRAC_1_PI / tmp.powi(2)
    }
    #[must_use]
    fn lambda(&self, a_sq: f32, w: Vec3) -> f32 {
        // Heitz2018 (2)
        // fairly certain that w.x^2 + w.y^2 / w.z^2 = tan^2
        let lambda = a_sq * (w.x.powi(2) + w.y.powi(2)) / w.z.powi(2);
        // approx 1/100 billion change out < 0.0 due to floating point
        let out = 0.5 * ((1.0 + lambda).sqrt() - 1.0).max(0.0);
        out
    }
    #[must_use]
    pub fn g1_local(&self, a_sq: f32, w: Vec3, wm: Vec3) -> f32 {
        if w.dot(wm) * wm.z <= 0.0 {
            return 0.0;
        }
        let lambda = self.lambda(a_sq, w);
        1.0 / (1.0 + lambda)
    }
    // Height correlated G2 (Heitz2014Microfacet 99)
    #[must_use]
    fn g2_local(&self, a_sq: f32, wa: Vec3, wb: Vec3, wm: Vec3) -> f32 {
        let mut out = 1.0 / (1.0 + self.lambda(a_sq, wa) + self.lambda(a_sq, wb));
        if wa.dot(wm) * wa.z <= 0.0 || wb.dot(wm) * wb.z <= 0.0 {
            out = 0.0;
        }
        out
    }
    #[must_use]
    fn get_a(&self, sect: &Intersection) -> f32 {
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        texs[self.roughness].uv_value(sect.uv)[1].max(0.0001)
    }
}
