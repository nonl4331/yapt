use crate::prelude::*;

const MAX_DEPTH: u64 = 50;
const RUSSIAN_ROULETTE_THRESHOLD: u64 = 3;

pub struct Naive {}

impl Naive {
    pub fn rgb(mut ray: Ray, bvh: &Bvh, rng: &mut impl MinRng) -> (Vec3, u64) {
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
                if rng.gen() > p {
                    break;
                }
                tp /= p;
            }
        }
        if rgb.contains_nan() {
            println!("a");
            return (Vec3::ZERO, 0);
        }
        (rgb, depth)
    }
}

pub struct NEEMIS {}

impl NEEMIS {
    pub fn rgb(mut ray: Ray, bvh: &Bvh, rng: &mut impl MinRng, samplable: &[usize]) -> (Vec3, u64) {
        if samplable.is_empty() {
            return Naive::rgb(ray, bvh, rng);
        }
        let inverse_samplable = 1.0 / samplable.len() as f32;

        let mut tp = Vec3::ONE;

        let mut ray_count = 1;

        // ----
        // find first intersection (MIS + NEE doesn't apply to camera rays)
        // ----
        let mut sect = get_intersection(&ray, bvh);

        if sect.is_none() {
            return (Vec3::ZERO, ray_count);
        }

        let mut mat = unsafe { &MATERIALS[sect.mat] };

        let mut rgb = mat.le(sect.pos, ray.dir);

        if let Mat::Light(_) = mat {
            return (rgb, 1);
        }

        let mut wo = ray.dir;

        for depth in 1..MAX_DEPTH {
            // ----
            // Light sampling
            // ----
            // pick light
            let light_idx = rng.gen_range(0.0..(samplable.len() as f32)) as usize;
            //let light_idx = *samplable.choose(rng).unwrap();
            let light = unsafe { &TRIANGLES[light_idx] };

            // sample ray
            let (light_ray, light_le) = light.sample_ray(&sect, rng);

            // check for obstructions
            ray_count += 1;
            let light_sect = intersect_idx(&light_ray, bvh, light_idx);
            let mut light_pdf = 0.0;
            if !light_sect.is_none() {
                light_pdf = light.pdf(&light_sect, &light_ray) * inverse_samplable;
            }

            // add light contribution if path is reachable by bsdf
            let light_bsdf_pdf = mat.spdf(&sect, light_ray.dir);
            if light_bsdf_pdf != 0.0 && light_pdf != 0.0 {
                rgb += tp
                    * power_heuristic(light_pdf, light_bsdf_pdf)
                    * mat.bxdf_cos(&sect, wo, light_ray.dir)
                    * light_le
                    / light_pdf;
            }

            // ----
            // BSDF sampling
            // ----
            if mat.scatter(&sect, &mut ray, rng) {
                unreachable!()
            }

            ray_count += 1;
            let new_sect = get_intersection(&ray, bvh);
            if new_sect.is_none() {
                return (rgb, ray_count);
            }

            let new_mat = unsafe { &MATERIALS[new_sect.mat] };
            let bsdf_pdf = mat.spdf(&sect, ray.dir);

            tp *= mat.eval(&sect, wo, ray.dir);

            // hit samplable calculate weight
            if samplable.contains(&new_sect.id) {
                let bsdf_light_pdf =
                    unsafe { TRIANGLES[new_sect.id].pdf(&new_sect, &ray) } * inverse_samplable;

                tp *= power_heuristic(bsdf_pdf, bsdf_light_pdf);
            }

            let le = new_mat.le(new_sect.pos, ray.dir);
            if le != Vec3::ZERO {
                rgb += tp * le;
            }

            if let Mat::Light(_) = new_mat {
                break;
            }

            sect = new_sect;
            mat = new_mat;
            wo = ray.dir;

            // ----
            // Russian Roulette early exit
            // ----
            if depth > RUSSIAN_ROULETTE_THRESHOLD {
                let p = tp.component_max();
                if rng.gen() > p {
                    break;
                }
                tp /= p;
            }
        }

        if rgb.contains_nan() {
            return (Vec3::ZERO, 0);
        }

        (rgb, ray_count)
    }
}

fn get_intersection(ray: &Ray, bvh: &Bvh) -> Intersection {
    let mut sect = Intersection::NONE;
    for range in bvh.traverse(ray) {
        for i in range {
            let mut tri_sect = unsafe { TRIANGLES[i].intersect(ray) };
            tri_sect.id = i;
            sect.min(tri_sect);
        }
    }
    sect
}

pub fn intersect_idx(ray: &Ray, bvh: &Bvh, idx: usize) -> Intersection {
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
