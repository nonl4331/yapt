use crate::prelude::*;

const MAX_DEPTH: u64 = 50;
const RUSSIAN_ROULETTE_THRESHOLD: u64 = 15;

pub struct Naive {}

impl Naive {
    #[must_use]
    pub fn rgb(mut ray: Ray, rng: &mut impl MinRng) -> (Vec3, u64) {
        let mats = unsafe { MATERIALS.get().as_ref_unchecked() };
        let envmap = unsafe { ENVMAP.get().as_ref_unchecked() };
        let (mut tp, mut rgb) = (Vec3::ONE, Vec3::ZERO);

        let mut depth = 0;

        while depth < MAX_DEPTH {
            depth += 1;

            let sect = get_intersection(&ray, rng);

            if sect.is_none() {
                rgb += tp * envmap.sample_dir(ray.dir);
                break;
            }

            let mat = &mats[sect.mat];

            let wo = -ray.dir;

            rgb += mat.le() * tp;

            let status = mat.scatter(&sect, &mut ray, rng);

            if status.contains(ScatterStatus::EXIT) {
                break;
            }

            // by convention both wo and wi point away from the surface
            tp *= mat.eval(&sect, wo, ray.dir, status);
            if tp.contains_nan() {
                return (Vec3::new(0.0, 1.0, 0.0), depth);
            }

            if depth > RUSSIAN_ROULETTE_THRESHOLD {
                let p = tp.component_max();
                if rng.random() > p {
                    break;
                }
                tp /= p;
            }
        }
        if rgb.contains_nan() {
            log::warn!("NAN encountered!");
            return (Vec3::X, 0);
        }
        (rgb, depth)
    }
}

pub struct NEEMIS {}

impl NEEMIS {
    #[must_use]
    pub fn rgb(mut ray: Ray, rng: &mut impl MinRng, samplable: &[usize]) -> (Vec3, u64) {
        let mats = unsafe { MATERIALS.get().as_ref_unchecked() };
        let envmap = unsafe { ENVMAP.get().as_ref_unchecked() };
        let tris = unsafe { TRIANGLES.get().as_ref_unchecked() };
        let samplables = unsafe { SAMPLABLE.get().as_ref_unchecked() };

        if samplable.is_empty() {
            return Naive::rgb(ray, rng);
        }
        let inverse_samplable = 1.0 / samplable.len() as f32;

        let mut tp = Vec3::ONE;

        let mut ray_count = 1;

        // ----
        // find first intersection (MIS + NEE doesn't apply to camera rays)
        // ----
        let mut sect = get_intersection(&ray, rng);

        if sect.is_none() {
            return (envmap.sample_dir(ray.dir), ray_count);
        }

        let mut mat = &mats[sect.mat];

        let mut rgb = mat.le();

        if let Mat::Light(_) = mat {
            return (rgb, 1);
        }

        let mut wo = -ray.dir;

        for depth in 1..MAX_DEPTH {
            // ----
            // Light sampling
            // ----
            // pick light
            let light_idx = rng.random_range(0.0..(samplable.len() as f32)) as usize;
            let light_idx = samplables[light_idx];
            let light = &tris[light_idx];

            // sample ray
            let (light_ray, light_le) = light.sample_ray(&sect, rng);

            // check for obstructions
            ray_count += 1;
            let light_sect = intersect_idx(&light_ray, light_idx, rng);
            if !light_sect.is_none()
                && !mat
                    .properties()
                    .contains(MaterialProperties::ONLY_DIRAC_DELTA)
            {
                let light_pdf = light.pdf(&light_sect, &light_ray) * inverse_samplable;

                // add light contribution if path is reachable by bsdf
                // by convention both wo and wi point away from the surface
                let light_bsdf_pdf = mat.spdf(&sect, wo, light_ray.dir);
                if light_bsdf_pdf != 0.0 && light_pdf != 0.0 {
                    rgb += tp
                        * power_heuristic(light_pdf, light_bsdf_pdf)
                        * mat.bxdf_cos(&sect, wo, light_ray.dir)
                        * light_le
                        / light_pdf;
                }
            }

            // ----
            // BSDF sampling
            // ----
            let status = mat.scatter(&sect, &mut ray, rng);

            if status.contains(ScatterStatus::EXIT) {
                unreachable!()
            }

            tp *= mat.eval(&sect, wo, ray.dir, status);

            ray_count += 1;
            let new_sect = get_intersection(&ray, rng);
            if new_sect.is_none() {
                rgb += tp * envmap.sample_dir(ray.dir);
                break;
            }

            let new_mat = &mats[new_sect.mat];

            // hit samplable calculate weight
            if samplable.contains(&new_sect.id) && !status.contains(ScatterStatus::DIRAC_DELTA) {
                // by convention both wo and wi point away from the surface
                let bsdf_pdf = mat.spdf(&sect, wo, ray.dir);
                let bsdf_light_pdf = tris[new_sect.id].pdf(&new_sect, &ray) * inverse_samplable;
                rgb += tp * power_heuristic(bsdf_pdf, bsdf_light_pdf) * new_mat.le();
            } else {
                rgb += tp * new_mat.le();
            }

            if let Mat::Light(_) = new_mat {
                break;
            }

            sect = new_sect;
            mat = new_mat;
            wo = -ray.dir;

            // ----
            // Russian Roulette early exit
            // ----
            if depth > RUSSIAN_ROULETTE_THRESHOLD {
                let p = tp.component_max();
                if rng.random() > p {
                    break;
                }
                tp /= p;
            }
        }

        if rgb.contains_nan() {
            log::warn!("NAN encountered!");
            return (Vec3::ZERO, 0);
        }

        (rgb, ray_count)
    }
}
#[must_use]
fn get_intersection(ray: &Ray, rng: &mut impl MinRng) -> Intersection {
    let tris = unsafe { TRIANGLES.get().as_ref_unchecked() };
    let bvh = unsafe { BVH.get().as_ref_unchecked() };
    let mut sect = Intersection::NONE;
    for range in bvh.traverse(ray) {
        for i in range {
            let mut tri_sect = tris[i].intersect(ray, rng);
            tri_sect.id = i;
            sect.min(tri_sect);
        }
    }
    sect
}
#[must_use]
pub fn intersect_idx(ray: &Ray, idx: usize, rng: &mut impl MinRng) -> Intersection {
    let tris = unsafe { TRIANGLES.get().as_ref_unchecked() };
    let bvh = unsafe { BVH.get().as_ref_unchecked() };
    let sect = tris[idx].intersect(ray, rng);
    if sect.is_none() {
        return sect;
    }

    for range in bvh.traverse(ray) {
        for i in range {
            if i == idx {
                continue;
            }
            let t = tris[i].intersect(ray, rng).t;
            if t > 0.0 && t < sect.t {
                return Intersection::NONE;
            }
        }
    }
    sect
}

#[inline]
#[must_use]
pub fn power_heuristic(pdf_a: f32, pdf_b: f32) -> f32 {
    let a_sq = pdf_a.powi(2);
    a_sq / (a_sq + pdf_b.powi(2))
}
