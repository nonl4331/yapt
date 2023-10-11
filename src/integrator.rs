use crate::prelude::*;
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

            rgb += mat.le(&sect, wo) * tp;

            if mat.scatter(&sect, &mut ray, rng) {
                break;
            }

            tp *= mat.eval(sect, wo, ray.dir);

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
        for tri in unsafe { &TRIANGLES[range] } {
            sect.min(tri.intersect(ray));
        }
    }
    sect
}
