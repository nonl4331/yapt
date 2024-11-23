use crate::Vec3;

pub struct Coordinate {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Coordinate {
    pub const NOP: Self = Coordinate {
        x: Vec3::X,
        y: Vec3::Y,
        z: Vec3::Z,
    };
    #[must_use]
    pub fn new_from_z(z: Vec3) -> Self {
        let x = if z.x.abs() > z.y.abs() {
            Vec3::new(-z.z, 0.0, z.x) / (z.x * z.x + z.z * z.z).sqrt()
        } else {
            Vec3::new(0.0, z.z, -z.y) / (z.y * z.y + z.z * z.z).sqrt()
        };
        Coordinate {
            x,
            y: x.cross(z),
            z,
        }
    }
    #[must_use]
    pub fn local_to_global(&self, vec: Vec3) -> Vec3 {
        Vec3::new(
            vec.x * self.x.x + vec.y * self.y.x + vec.z * self.z.x,
            vec.x * self.x.y + vec.y * self.y.y + vec.z * self.z.y,
            vec.x * self.x.z + vec.y * self.y.z + vec.z * self.z.z,
        )
    }
    #[must_use]
    pub fn global_to_local(&self, vec: Vec3) -> Vec3 {
        Vec3::new(
            vec.x * self.x.x + vec.y * self.x.y + vec.z * self.x.z,
            vec.x * self.y.x + vec.y * self.y.y + vec.z * self.y.z,
            vec.x * self.z.x + vec.y * self.z.y + vec.z * self.z.z,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Quaternion {
    w: f32,
    x: f32,
    y: f32,
    z: f32,
}

impl Quaternion {
    pub const fn new(w: f32, x: f32, y: f32, z: f32) -> Self {
        Self { w, x, y, z }
    }
    pub const fn hamilton(&self, other: Self) -> Self {
        Self::new(
            self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
            self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
        )
    }
    pub const fn xyz(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
    pub const fn conj(&self) -> Self {
        Self::new(self.w, -self.x, -self.y, -self.z)
    }
}

impl From<Vec3> for Quaternion {
    fn from(v: Vec3) -> Self {
        Self::new(0.0, v.x, v.y, v.z)
    }
}

#[cfg(test)]
mod tests {

    const ETA: f32 = 100.0 * f32::EPSILON;

    use super::*;
    use rand::thread_rng;
    use rand::Rng;

    fn random_unit_vector() -> Vec3 {
        let mut rng = thread_rng();

        Vec3::new(rng.gen(), rng.gen(), rng.gen()).normalised()
    }

    #[test]
    fn inverse() {
        let z = random_unit_vector();
        let coord = Coordinate::new_from_z(z);
        let v = random_unit_vector();
        assert!(
            (v - coord.global_to_local(coord.local_to_global(v))).mag_sq() < ETA
                && (v - coord.local_to_global(coord.global_to_local(v))).mag_sq() < ETA
        );
    }

    #[test]
    fn random_coordiante_system() {
        let rando_coord = random_unit_vector();
        let coord = Coordinate::new_from_z(rando_coord);

        assert!((coord.global_to_local(rando_coord) - Vec3::Z).mag_sq() < ETA);
    }

    #[test]
    fn nop() {
        let rando_vec = random_unit_vector();
        let coord = Coordinate::NOP;
        assert_eq!(coord.global_to_local(rando_vec), rando_vec);
        assert_eq!(coord.local_to_global(rando_vec), rando_vec);
    }
}
