use crate::prelude::*;

pub struct Cam {
    pub lower_left: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub origin: Vec3,
    width: u32,
    height: u32,
}

impl Cam {
    #[must_use]
    pub fn new(
        origin: Vec3,
        look_at: Vec3,
        mut up: Vec3,
        hfov: f32,
        focus_dist: f32,
        width: u32,
        height: u32,
    ) -> Self {
        let forward = (look_at - origin).normalised();
        up.normalise();
        let aspect_ratio = width as f32 / height as f32;

        let right_mag = focus_dist * 2.0 * (0.5 * hfov.to_radians()).tan();
        let up_mag = right_mag / aspect_ratio;

        let right = forward.cross(up).normalised() * right_mag;
        let up = right.cross(forward).normalised() * up_mag;

        let lower_left = origin - 0.5 * right - 0.5 * up + forward * focus_dist;

        Self {
            lower_left,
            up,
            right,
            origin,
            width,
            height,
        }
    }
    #[must_use]
    pub fn get_ray(&self, i: u64, rng: &mut impl MinRng) -> ([f32; 2], Ray) {
        let (u, v) = (i % self.width as u64, i / self.width as u64);
        let (u, v) = (
            (u as f32 + rng.gen()) / self.width as f32,
            (v as f32 + rng.gen()) / self.height as f32,
        );

        (
            [u, v],
            Ray::new(
                self.origin,
                self.lower_left + self.right * u + self.up * (1.0 - v) - self.origin,
            ),
        )
    }
    #[must_use]
    pub fn get_centre_ray(&self, i: u64) -> Ray {
        let (u, v) = (i % self.width as u64, i / self.width as u64);
        let (u, v) = (
            (u as f32 + 0.5) / self.width as f32,
            (v as f32 + 0.5) / self.height as f32,
        );
        Ray::new(
            self.origin,
            self.lower_left + self.right * u + self.up * (1.0 - v) - self.origin,
        )
    }
    #[must_use]
    pub fn get_random_ray(&self, rng: &mut impl MinRng) -> ([f32; 2], Ray) {
        let (u, v) = (rng.gen(), rng.gen());
        (
            [u, v],
            Ray::new(
                self.origin,
                self.lower_left + self.right * u + self.up * (1.0 - v) - self.origin,
            ),
        )
    }
}
