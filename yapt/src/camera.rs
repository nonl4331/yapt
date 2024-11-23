use crate::prelude::*;

pub const PLACEHOLDER: Cam = Cam {
    lower_left: Vec3 {
        x: -0.5773503,
        y: 1.0,
        z: -0.5773503,
    },
    up: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.1547006,
    },
    right: Vec3 {
        x: 1.1547006,
        y: 0.0,
        z: 0.0,
    },
    origin: Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    width: 1024,
    height: 1024,
};

#[derive(Debug)]
pub struct Cam {
    pub lower_left: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub origin: Vec3,
    width: u32,
    height: u32,
}

impl Cam {
    // todo: figure out how to properly go blender XYZ euler -> quat
    /*#[must_use]
    pub fn new_rot(
        origin: Vec3,
        mut rotation: Vec3,
        hfov: f32,
        render_settings: &RenderSettings,
        degrees: bool,
    ) -> Self {
        if degrees {
            rotation *= std::f32::consts::PI / 180.0;
        }

        let (roll, pitch, yaw) = (rotation[1] * 0.5, rotation[0] * 0.5, -rotation[2] * 0.5);

        let (sr, cr) = roll.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();

        // w, x, y, z
        let q = Quaternion::new(
            cr * cp * cy + sr * sp * sy,
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
        );

        Self::new_quat(origin, q, hfov, render_settings)
    }*/

    // see https://math.stackexchange.com/questions/40164/how-do-you-rotate-a-vector-by-a-unit-quaternion
    // and https://en.wikipedia.org/wiki/Quaternions_and_spatial_rotation
    #[must_use]
    pub fn new_quat(
        origin: Vec3,
        q: Quaternion,
        hfov: f32,
        render_settings: &RenderSettings,
    ) -> Self {
        let qp = q.conj();

        // match blender default where default is up = Y, forward = -Z (still in space with Z up
        // and Y forward)
        let up: Quaternion = Vec3::new(0.0, 1.0, 0.0).into();
        let up = q.hamilton(up).hamilton(qp).xyz();

        let forward: Quaternion = Vec3::new(0.0, 0.0, -1.0).into();
        let forward = q.hamilton(forward).hamilton(qp).xyz();

        let aspect_ratio =
            u32::from(render_settings.width) as f32 / u32::from(render_settings.height) as f32;
        let right_mag = 2.0 * (0.5 * hfov.to_radians()).tan();
        let up_mag = right_mag / aspect_ratio;

        let right = forward.cross(up).normalised() * right_mag;
        let up = right.cross(forward).normalised() * up_mag;

        let lower_left = origin - 0.5 * right - 0.5 * up + forward;
        let lower_left = lower_left + render_settings.u_low * right + render_settings.v_low * up;
        let right = right * (render_settings.u_high - render_settings.u_low);
        let up = up * (render_settings.v_high - render_settings.v_low);

        Self {
            lower_left,
            up,
            right,
            origin,
            width: render_settings.width.into(),
            height: render_settings.height.into(),
        }
    }
    #[must_use]
    pub fn new(
        origin: Vec3,
        look_at: Vec3,
        mut up: Vec3,
        hfov: f32,
        focus_dist: f32,
        render_settings: &RenderSettings,
    ) -> Self {
        let forward = (look_at - origin).normalised();
        up.normalise();
        let aspect_ratio =
            u32::from(render_settings.width) as f32 / u32::from(render_settings.height) as f32;

        let right_mag = focus_dist * 2.0 * (0.5 * hfov.to_radians()).tan();
        let up_mag = right_mag / aspect_ratio;

        let right = forward.cross(up).normalised() * right_mag;
        let up = right.cross(forward).normalised() * up_mag;

        let lower_left = origin - 0.5 * right - 0.5 * up + forward * focus_dist;
        let lower_left = lower_left + render_settings.u_low * right + render_settings.v_low * up;
        let right = right * (render_settings.u_high - render_settings.u_low);
        let up = up * (render_settings.v_high - render_settings.v_low);

        Self {
            lower_left,
            up,
            right,
            origin,
            width: render_settings.width.into(),
            height: render_settings.height.into(),
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
