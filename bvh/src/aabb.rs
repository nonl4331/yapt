use utility::*;

pub trait Aabound {
    fn aabb(&self) -> Aabb;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        if min.x > max.x || min.y > max.y || min.z > max.z {
            panic!("Maximum value in AABB must be greater or equal to the minimum!");
        }
        Self { min, max }
    }

    pub fn does_int(&self, ray: &Ray) -> bool {
        let mut t1 = (self.min.x - ray.origin.x) * ray.inv_dir.x;
        let mut t2 = (self.max.x - ray.origin.x) * ray.inv_dir.x;

        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t2 *= 1.0 + 2.0 * gamma(3);

        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        let mut t1 = (self.min.y - ray.origin.y) * ray.inv_dir.y;
        let mut t2 = (self.max.y - ray.origin.y) * ray.inv_dir.y;

        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t2 *= 1.0 + 2.0 * gamma(3);

        let tmin = tmin.max(t1.min(t2));
        let tmax = tmax.min(t1.max(t2));

        let mut t1 = (self.min.z - ray.origin.z) * ray.inv_dir.z;
        let mut t2 = (self.max.z - ray.origin.z) * ray.inv_dir.z;

        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t2 *= 1.0 + 2.0 * gamma(3);

        let tmin = tmin.max(t1.min(t2));
        let tmax = tmax.min(t1.max(t2));

        tmax > tmin.max(0.0)
    }

    pub fn centroid(&self) -> Vec3 {
        0.5 * (self.max + self.min)
    }

    pub fn merge(a: Self, b: Self) -> Self {
        Aabb::new(min_vec3(&a.min, &b.min), max_vec3(&a.max, &b.max))
    }

    pub fn extend_contains(aabb: &mut Option<Self>, point: Vec3) {
        match aabb {
            Some(inner) => {
                inner.min = min_vec3(&inner.min, &point);
                inner.max = max_vec3(&inner.max, &point);
            }
            None => *aabb = Some(Aabb::new(point, point)),
        }
    }

    pub fn extent(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn surface_area(&self) -> f32 {
        let extent = self.extent();
        2.0 * (extent.x * extent.y + extent.x * extent.z + extent.y * extent.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::Rng;
    use rand_seeder::Seeder;

    #[test]
    fn merge_aabb() {
        let a = Aabb::new(Vec3::new(-3.0, 5.0, 0.0), Vec3::new(-2.9, 5.1, 0.0));
        let b = Aabb::new(Vec3::new(1.0, 4.0, 1.2), Vec3::new(1.1, 4.1, 1.3));
        assert_eq!(
            Aabb::merge(a, b),
            Aabb::new(Vec3::new(-3.0, 4.0, 0.0), Vec3::new(1.1, 5.1, 1.3))
        );
    }

    #[test]
    #[ignore]
    fn aabb_int() {
        let mut rng: SmallRng = Seeder::from("stripy zebra").make_rng();

        let aabb = Aabb::new(Vec3::new(-3.0, 4.0, 0.0), Vec3::new(1.1, 5.1, 1.3));
        let extent = aabb.extent();

        let mut gen_ray = || -> Ray {
            let mut origin = Vec3::zeros();
            while origin == Vec3::zeros() {
                origin = Vec3::new(
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                )
                .normalize()
                    * 15.0;
            }

            let point_in_aabb = aabb.min
                + Vec3::new(
                    rng.gen_range(0.0..1.0) * extent.x,
                    rng.gen_range(0.0..1.0) * extent.y,
                    rng.gen_range(0.0..1.0) * extent.z,
                );

            let dir = (point_in_aabb - origin).normalize();
            Ray::new(origin, dir)
        };

        for _ in 0..100_000_000 {
            let ray = gen_ray();
            assert!(aabb.does_int(&ray));
        }
    }
}
