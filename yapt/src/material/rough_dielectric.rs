pub use crate::prelude::*;

#[derive(Debug)]
pub struct RoughDielectric {
    pub roughness: usize,
    pub ior: f32,
}

impl RoughDielectric {
    pub fn new(roughness: usize, ior: f32) -> Mat {
        Mat::RoughRefractive(Self { roughness, ior })
    }
    // see https://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
    pub fn scatter(
        &self,
        sect: &Intersection,
        ray: &mut Ray,
        rng: &mut impl MinRng,
    ) -> ScatterStatus {
        let mut wo = -ray.dir;
        let a = self.get_a(sect);

        let coord = crate::coord::Coordinate::new_from_z(sect.nor);
        wo = coord.global_to_local(wo);
        let wm = self.sample_vndf_local(a, wo, rng);
        assert!(wm.z >= 0.0);
        assert!(wo.z >= 0.0);
        // this fails every so often
        //assert!(wo.dot(wm) > 0.0);

        let mut eta1 = 1.0;
        let mut eta2 = self.ior;

        if !sect.out {
            std::mem::swap(&mut eta1, &mut eta2);
        }

        let eta = eta1 / eta2;
        let cosi = wm.dot(wo);

        let f = super::fresnel_dielectric(1.0, self.ior, wm, wo);
        // reflect
        if f >= rng.gen() {
            let wi = wo.reflected(wm);
            *ray = Ray::new(
                sect.pos + sect.nor * 0.00001,
                coord.local_to_global(wi).normalised(),
            );
            return ScatterStatus::NORMAL;
        }

        // refract
        let perp = eta * (cosi * wm - wo);
        let para = -(1.0 - perp.mag_sq()).abs().sqrt() * wm;
        let wi = perp + para;
        assert!(wm.dot(wo) >= 0.0 && wo.dot(wi) < 0.0);
        *ray = Ray::new(
            sect.pos - sect.nor * 0.00001,
            coord.local_to_global(wi).normalised(),
        );

        ScatterStatus::NORMAL
    }
    #[must_use]
    pub fn eval(&self, wo: Vec3, wi: Vec3, sect: &Intersection) -> Vec3 {
        //assert!(wo.z > 0.0);
        let pdf = self.pdf(wo, wi, sect);
        let bxdf_cos = self.bxdf_cos(wo, wi, sect);
        if pdf == 0.0 {
            if bxdf_cos != Vec3::ZERO {
                unreachable!();
            }
            return Vec3::ZERO;
        }
        bxdf_cos / pdf
    }
    #[must_use]
    pub fn bxdf_cos(&self, wo: Vec3, wi: Vec3, sect: &Intersection) -> Vec3 {
        let a_sq = self.get_a(sect).powi(2);

        let mut eta1 = 1.0;
        let mut eta2 = self.ior;

        if !sect.out {
            std::mem::swap(&mut eta1, &mut eta2);
        }

        let refraction = wo.z * wi.z < 0.0;

        let wm = if refraction {
            let wm = (eta2 * wi + eta1 * wo).normalised();
            wm * wm.z.signum()
        } else {
            (wo + wi).normalised()
        };

        // backfacing microfacet
        if wm.dot(wi) * wi.z < 0.0 || wm.dot(wo) * wo.z < 0.0 {
            return Vec3::ZERO;
        }

        let f = super::fresnel_dielectric(eta1, eta2, wm, wo);

        let eta = eta1 / eta2;
        let denom = ((wm.dot(wi) + wm.dot(wo)) / eta).powi(2);

        if refraction {
            let v = (1.0 - f) * self.ndf_local(a_sq, wm) * self.g2_local(a_sq, wo, wi, wm) / denom
                * (wi.dot(wm) * wo.dot(wm) / wo.z).abs();
            return Vec3::splat(v);
        }

        let v = f * self.ndf_local(a_sq, wm) * self.g2_local(a_sq, wo, wi, wm) / (4.0 * wo.z);
        Vec3::splat(v)
    }
    #[must_use]
    pub fn pdf(&self, wo: Vec3, wi: Vec3, sect: &Intersection) -> f32 {
        let a = self.get_a(sect);

        let mut eta1 = 1.0;
        let mut eta2 = self.ior;

        if !sect.out {
            std::mem::swap(&mut eta1, &mut eta2);
        }
        let eta = eta1 / eta2;

        let w_ref = (wi + wo).normalised();
        let mut ret = 0.0;
        if w_ref.z > 0.0 && !(w_ref.dot(wi) * wi.z < 0.0 || w_ref.dot(wo) * wo.z < 0.0) {
            ret += super::fresnel_dielectric(eta1, eta2, w_ref, wo)
                * self.vndf_local(a.powi(2), w_ref, wo)
                / (4.0 * wo.dot(w_ref));
        }

        let w_ref = (eta2 * wi + eta1 * wo).normalised();
        if w_ref.z > 0.0 && !(w_ref.dot(wi) * wi.z < 0.0 || w_ref.dot(wo) * wo.z < 0.0) {
            let denom = ((w_ref.dot(wi) + w_ref.dot(wo)) / eta).powi(2);
            ret += (1.0 - super::fresnel_dielectric(eta1, eta2, w_ref, wo))
                * self.vndf_local(a.powi(2), w_ref, wo)
                * wo.dot(w_ref).abs()
                / denom;
        }

        ret

        /*let refraction = wo.z * wi.z < 0.0;

        if refraction {
            // will shit itself when eta1 == eta2
            let wm = (eta2 * wi + eta1 * wo).normalised();

            // backfacing microfacet
            if wm.dot(wi) * wi.z < 0.0 || wm.dot(wo) * wo.z < 0.0 {
                return 0.0;
            }

            let denom = ((wm.dot(wi) + wm.dot(wo)) / eta).powi(2);
            return self.vndf_local(a.powi(2), wm, wo) * wo.dot(wm).abs() / denom;
        }

        // reflection
        let wm = (wo + wi).normalised();
        // Heitz2018GGX (17)
        self.vndf_local(a.powi(2), wm, wo) / (4.0 * wo.dot(wm))*/
    }

    #[must_use]
    fn get_a(&self, sect: &Intersection) -> f32 {
        let texs = unsafe { crate::TEXTURES.get().as_ref_unchecked() };
        texs[self.roughness].uv_value(sect.uv)[1].max(0.0001)
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
}
