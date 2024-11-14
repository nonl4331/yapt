use bvh::{aabb::Aabb, Bvh};
use rand::{thread_rng, Rng};
use utility::Vec3;

fn main() {
    divan::main();
}

fn random_primitives() -> Vec<Aabb> {
    let mut rng = thread_rng();
    (0..100_000)
        .map(|_| {
            let lower_bound = Vec3::new(
                rng.gen_range(-10.0..10.0),
                rng.gen_range(-10.0..10.0),
                rng.gen_range(-10.0..10.0),
            );
            let extent = Vec3::new(
                rng.gen_range(0.0..1.0),
                rng.gen_range(0.0..1.0),
                rng.gen_range(0.0..1.0),
            );
            let upper_bound = lower_bound + extent;
            Aabb::new(lower_bound, upper_bound)
        })
        .collect()
}

#[divan::bench]
fn creation(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| random_primitives())
        .bench_refs(|v| Bvh::new(v));
}
