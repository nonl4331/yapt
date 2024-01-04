pub use crate::prelude::*;

#[derive(Debug)]
pub struct Ggx {
    a: f32,
    a_sq: f32,
}

impl Ggx {
    pub fn new(a: f32) -> Self {
        // don't allow a=0 due to floating point
        // large values of a also have slight
        // floating point issues such as a = 100
        let a = a.max(0.001); 
        Self { a, a_sq: a.powi(2) }
    }
    pub fn scatter(&self, sect: &Intersection, ray: &mut Ray, rng: &mut impl MinRng) -> bool {
        // by convention points away from surface hence the -ray.dir (section 2, definition)
        *ray = Ray::new(sect.pos, self.sample(sect.nor, -ray.dir, rng));
        false
    }

    pub fn sample(&self, normal: Vec3, wo: Vec3, rng: &mut impl MinRng) -> Vec3 {
        let coord = crate::coord::Coordinate::new_from_z(normal);
        let local_wo = coord.to_coord(wo);
        let wm = self.sample_vndf_local(local_wo, rng);
        let local_wi = local_wo.reflected(wm);
        coord.create_inverse().to_coord(local_wi).normalised()
    }

    // local space (hemisphere on z=0 plane see section 2, definition)
    pub fn sample_vndf_local(&self, in_w: Vec3, rng: &mut impl MinRng) -> Vec3 {
        // map episoid to unit hemisphere (section 2, importance sampling 1)
        let in_w = Vec3::new(self.a * in_w.x, self.a * in_w.y, in_w.z).normalised();

        // intersect unit hemisphere based on new in_w and record point (section 2, important
        // sampling 2)
        let p_hemi = Self::sample_vndf_hemisphere(in_w, rng);

        // transform intersection point back (section 2, importance sampling 3)
        let p_elipsoid = Vec3::new(p_hemi.x * self.a, p_hemi.y * self.a, p_hemi.z).normalised();
        // ^^ why is this * not /

        p_elipsoid
    }

    // (section 3, listing 3)
    fn sample_vndf_hemisphere(in_w_hemi: Vec3, rng: &mut impl MinRng) -> Vec3 {
        let phi = TAU * rng.gen();
        // can replace (1.0 - x) with x?
        let z = (1.0 - rng.gen()) * (1.0 + in_w_hemi.z) - in_w_hemi.z;
        let sin_theta = (1.0 - z.powi(2)).clamp(0.0, 1.0).sqrt();
        let c = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), z);
        c + in_w_hemi
    }

    // Dwm local (VNDF)
    // wo is camera ray
    pub fn pdf_wm_vndf_local(&self, wo: Vec3, wm: Vec3) -> f32 {
        if wm.z < 0.0 {
            return 0.0;
        }
        self.g1_local(wo) * wo.dot(wm).max(0.0) * self.wm_dist_local(wm) / wo.z
    }

    pub fn pdf(&self, wo: Vec3, nor: Vec3, wi: Vec3) -> f32 {
        // by convention points away from surface (section 2, definition)
        let coord = crate::coord::Coordinate::new_from_z(nor);
        let local_wo = coord.to_coord(wo);
        let local_wi = coord.to_coord(wi);
        let local_wm = (local_wo + local_wi).normalised();
        // Heitz2018GGX (17)
        self.pdf_wm_vndf_local(local_wo, local_wm) / (4.0 * local_wo.dot(local_wm))
    }

    // Dwm local
    fn wm_dist_local(&self, wm: Vec3) -> f32 {
        let a_sq = self.a_sq;
        let denom = self.a * (wm.x.powi(2) / a_sq + wm.y.powi(2) / a_sq + wm.z.powi(2));

        FRAC_1_PI / denom.powi(2)
    }

    fn g1_local(&self, w: Vec3) -> f32 {
        let lambda = self.a_sq * (w.x.powi(2) + w.y.powi(2)) / w.z.powi(2);
        let lambda = 0.5 * ((1.0 + lambda).sqrt() - 1.0);
        1.0 / (1.0 + lambda)
    }
}
