use crate::prelude::*;
use rand::Rng;

const ASPECT_RATIO: f32 = WIDTH as f32 / HEIGHT as f32;

pub struct Cam {
    pub lower_left: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub origin: Vec3,
}

impl Cam {
    pub fn new(origin: Vec3, look_at: Vec3, mut up: Vec3, hfov: f32, focus_dist: f32) -> Self {
        let forward = (look_at - origin).normalised();
        up.normalise();

        let right_mag = focus_dist * 2.0 * (0.5 * hfov.to_radians()).tan();
        let up_mag = right_mag / ASPECT_RATIO;

        let right = forward.cross(up).normalised() * right_mag;
        let up = right.cross(forward).normalised() * up_mag;

        let lower_left = origin - 0.5 * right - 0.5 * up + forward * focus_dist;

        Self {
            origin,
            lower_left,
            right,
            up,
        }
    }

    pub fn get_ray(&self, i: usize, rng: &mut impl Rng) -> ([f32; 2], Ray) {
        let (u, v) = (i % WIDTH, i / WIDTH);
        let (u, v) = (
            (u as f32 + rng.gen::<f32>()) / WIDTH as f32,
            (v as f32 + rng.gen::<f32>()) / HEIGHT as f32,
        );

        ([u, v], Ray::new(
            self.origin,
            self.lower_left + self.right * u + self.up * (1.0 - v) - self.origin,
        ))
    }
}
