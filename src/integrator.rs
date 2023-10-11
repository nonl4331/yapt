use crate::prelude::*;
use rand::seq::SliceRandom;
use rand::Rng;

const MAX_DEPTH: u64 = 50;
const RUSSIAN_ROULETTE_THRESHOLD: u64 = 3;

pub struct Naive {}

impl Naive {
    pub fn rgb(mut ray: Ray, bvh: &Bvh, rng: &mut impl Rng) -> (Vec3, u64) {
        let (mut tp, mut rgb) = (Vec3::ONE, Vec3::ZERO);

        let mut depth = 0;

        while depth < MAX_DEPTH {
            depth += 1;

            let sect = get_intersection(&ray, bvh);

            if sect.is_none() {
                return (Vec3::ZERO, depth);
            }

            let mat = unsafe { &MATERIALS[sect.mat] };

            let wo = ray.dir;

            rgb += mat.le(sect.pos, ray.dir) * tp;

            if mat.scatter(&sect, &mut ray, rng) {
                break;
            }

            tp *= mat.eval(&sect, wo, ray.dir);

            if depth > RUSSIAN_ROULETTE_THRESHOLD {
                let p = tp.component_max();
                if rng.gen::<f32>() > p {
                    break;
                }
                tp /= p;
            }
        }
        if rgb.contains_nan() {
            return (Vec3::ZERO, 0);
        }

        (rgb, depth)
    }
}

pub struct NEEMIS {}

impl NEEMIS {
    pub fn rgb(mut ray: Ray, bvh: &Bvh, rng: &mut impl Rng, samplable: &[usize]) -> (Vec3, u64) {
        if samplable.is_empty() {
            return Naive::rgb(ray, bvh, rng);
        }

        let (mut tp, mut rgb) = (Vec3::ONE, Vec3::ZERO);

        let mut depth = 0;

        while depth < MAX_DEPTH {
            depth += 1;

            // ----
            // get intersection
            // ----
            let sect = get_intersection(&ray, bvh);

            if sect.is_none() {
                return (Vec3::ZERO, depth);
            }

            let mat = unsafe { &MATERIALS[sect.mat] };
    
            // if hit light do not continue
            if let Mat::Light(_) = mat {
                return (tp * mat.le(sect.pos, ray.dir), depth);
            }

            let wo = ray.dir;

            // ----
            // Light sampling
            // ----
            // pick light
            let light_idx = *samplable.choose(rng).unwrap();
            let light = unsafe { &TRIANGLES[light_idx] };

            // sample ray
            let (light_ray, mut light_pdf, light_le) = light.sample_ray(&sect, rng);


            // check for obstructions
            let light_sect = intersect_idx(&light_ray, bvh, light_idx);
            if light_sect.is_none() {
                light_pdf = 0.0;
            }

            // add light contribution
            let light_bxdf_pdf = mat.spdf(&sect, light_ray.dir);
            rgb += tp * mat.bxdf_cos(&sect, ray.dir, wo) * power_heuristic(light_pdf, light_bxdf_pdf) * light_le / light_pdf;


            // ----
            // BXDF scattering
            // ----
            if mat.scatter(&sect, &mut ray, rng) {
                break;
            }

            // ----
            // BXDF sampling
            // ----
            let bxdf_pdf = mat.spdf(&sect, ray.dir);
            let bxdf_le = mat.le(sect.pos, ray.dir);

            // accumulate bxdf contribution
            tp *= mat.eval(&sect, wo, ray.dir);

            // check if hits samplable light
            if samplable.contains(&sect.id) {
                let bxdf_light_pdf = unsafe { TRIANGLES[sect.id].pdf(&sect, &ray) } / samplable.len() as f32;
                rgb += tp * power_heuristic(bxdf_pdf, bxdf_light_pdf) * bxdf_le;

                // exit since hit light
                break;
            }

            // non samplable emissive surface
            rgb += tp * bxdf_le;

            // ----
            // Russian Roulette early exit
            // ----
            if depth > RUSSIAN_ROULETTE_THRESHOLD {
                let p = tp.component_max();
                if rng.gen::<f32>() > p {
                    break;
                }
                tp /= p;
            }
        }
        if rgb.contains_nan() {
            return (Vec3::ZERO, 0);
        }

        (rgb, depth)
    }
}

fn get_intersection(ray: &Ray, bvh: &Bvh) -> Intersection {
    let mut sect = Intersection::NONE;
    for range in bvh.traverse(ray) {
        for i in range {
            let mut tri_sect = unsafe {TRIANGLES[i].intersect(ray)};
            tri_sect.id = i;
            sect.min(tri_sect);
        }
    }
    sect
}

fn intersect_idx(ray: &Ray, bvh: &Bvh, idx: usize) -> Intersection {
    let sect = unsafe { TRIANGLES[idx].intersect(ray) };
    if sect.is_none() {
        return sect;
    }

    for range in bvh.traverse(ray) {
        for i in range {
            if i == idx {
                continue;
            }
            let t = unsafe { TRIANGLES[i].intersect(ray).t };
            if t > 0.0 && t < sect.t {
                return Intersection::NONE;
            }
        }
    }
    sect
}

#[inline]
pub fn power_heuristic(pdf_a: f32, pdf_b: f32) -> f32 {
    let a_sq = pdf_a.powi(2);
    a_sq / (a_sq + pdf_b.powi(2))
}
