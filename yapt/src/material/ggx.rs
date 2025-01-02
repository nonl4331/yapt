pub use crate::prelude::*;

#[derive(Debug)]
pub struct Ggx {
    a: f32,
    a_sq: f32,
    pub ior: usize,
}

impl Ggx {
    #[must_use]
    pub fn new(a: f32, ior: usize) -> Self {
        // don't allow a=0 due to floating point
        // large values of a also have slight
        // floating point issues such as a = 100
        let a = a.max(0.0001);
        Self {
            a,
            a_sq: a.powi(2),
            ior,
        }
    }
    #[must_use]
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray, rng: &mut impl MinRng) -> bool {
        // by convention points away from surface hence the -ray.dir (section 2, definition)
        *ray = Ray::new(sect.pos, self.sample(sect.nor, -ray.dir, rng));
        false
    }
    #[must_use]
    pub fn sample(&self, normal: Vec3, mut wo: Vec3, rng: &mut impl MinRng) -> Vec3 {
        let coord = crate::coord::Coordinate::new_from_z(normal);
        wo = coord.global_to_local(wo);
        let wm = self.sample_vndf_local(wo, rng);
        let wi = wo.reflected(wm);
        coord.local_to_global(wi).normalised()
    }
    #[must_use]
    pub fn eval(&self, wo: Vec3, wi: Vec3, uv: Vec2) -> Vec3 {
        let wm = (wo + wi).normalised();

        // f * g2 / g1 (Heitz2018GGX 19)
        let g2 = self.g2_local(wo, wi, wm);
        let f = self.f(wm.dot(wo), uv);
        let g1 = self.g1_local(wo, wm);
        if g1 == 0.0 {
            return Vec3::ZERO;
        }
        f * g2 / g1
    }
    #[must_use]
    pub fn bxdf_cos(&self, wo: Vec3, wi: Vec3, uv: Vec2) -> Vec3 {
        let wm = (wo + wi).normalised();
        self.f(wm.dot(wo), uv) * self.ndf_local(wm) * self.g2_local(wo, wi, wm) / (4.0 * wo.z)
    }
    // local space (hemisphere on z=0 plane see section 2, definition)
    #[must_use]
    pub fn sample_vndf_local(&self, in_w: Vec3, rng: &mut impl MinRng) -> Vec3 {
        // map episoid to unit hemisphere (section 2, importance sampling 1)
        let in_w = Vec3::new(self.a * in_w.x, self.a * in_w.y, in_w.z).normalised();

        // intersect unit hemisphere based on new in_w and record point (section 2, important
        // sampling 2)
        let p_hemi = Self::sample_vndf_hemisphere(in_w, rng);

        // transform intersection point back (section 2, importance sampling 3)
        Vec3::new(p_hemi.x * self.a, p_hemi.y * self.a, p_hemi.z).normalised()
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
    pub fn pdf(&self, wo: Vec3, wi: Vec3) -> f32 {
        let mut wm = (wo + wi).normalised();
        if wm.z < 0.0 {
            wm = -wm;
        }
        // Heitz2018GGX (17)
        self.vndf_local(wm, wo) / (4.0 * wo.dot(wm))
    }
    // visible normal distribution function
    // this is a valid PDF
    // wo is camera ray
    #[must_use]
    pub fn vndf_local(&self, wm: Vec3, wo: Vec3) -> f32 {
        if wm.z < 0.0 {
            return 0.0;
        }
        self.g1_local(wo, wm) * wo.dot(wm).max(0.0) * self.ndf_local(wm) / wo.z.abs()
        // see pbrt v4
    }
    // normal distribution function
    #[must_use]
    pub fn ndf_local(&self, wm: Vec3) -> f32 {
        if wm.z <= 0.0 {
            return 0.0;
        }
        let tmp = wm.z.powi(2) * (self.a_sq - 1.0) + 1.0;
        self.a_sq * FRAC_1_PI / tmp.powi(2)
    }
    #[must_use]
    fn lambda(&self, w: Vec3) -> f32 {
        // Heitz2018 (2)
        // fairly certain that w.x^2 + w.y^2 / w.z^2 = tan^2
        let lambda = self.a_sq * (w.x.powi(2) + w.y.powi(2)) / w.z.powi(2);
        // approx 1/100 billion change out < 0.0 due to floating point
        let out = 0.5 * ((1.0 + lambda).sqrt() - 1.0).max(0.0);
        out
    }
    #[must_use]
    pub fn g1_local(&self, w: Vec3, wm: Vec3) -> f32 {
        if w.dot(wm) * wm.z <= 0.0 {
            return 0.0;
        }
        let lambda = self.lambda(w);
        1.0 / (1.0 + lambda)
    }
    // Height correlated G2 (Heitz2014Microfacet 99)
    #[must_use]
    fn g2_local(&self, wa: Vec3, wb: Vec3, wm: Vec3) -> f32 {
        let mut out = 1.0 / (1.0 + self.lambda(wa) + self.lambda(wb));
        if wa.dot(wm) * wa.z <= 0.0 || wb.dot(wm) * wb.z <= 0.0 {
            out = 0.0;
        }
        out
    }
    // fresnel
    #[must_use]
    fn f(&self, cos_theta: f32, uv: Vec2) -> Vec3 {
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        let ior = texs[self.ior].uv_value(uv);
        ior + (1.0 - ior) * (1.0 - cos_theta).powi(5)
    }
}
