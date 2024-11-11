use crate::prelude::*;

pub enum EnvMap {
    Solid(Vec3),
    Image(TextureData),
}

impl EnvMap {
    pub const DEFAULT: Self = EnvMap::Solid(Vec3::ZERO);

    #[must_use]
    pub fn sample(&self, uv: Vec2) -> Vec3 {
        match self {
            Self::Solid(v) => *v,
            Self::Image(v) => v.sample(uv),
        }
    }
    #[must_use]
    pub fn sample_dir(&self, dir: Vec3) -> Vec3 {
        let theta = dir.z.acos() / PI;
        let phi = (dir.y.atan2(dir.x) + PI) / TAU;
        self.sample(Vec2::new(theta, phi))
    }
}

pub struct TextureData {
    dim: [usize; 2],
    pub data: Vec<Vec3>,
}

impl TextureData {
    pub fn from_path(filepath: &str) -> Result<Self, Box<dyn std::error::Error>> {
        use exr::prelude::*;
        let image = read_first_rgba_layer_from_file(
            filepath,
            |resolution, _| {
                let default_pixel = [0.0, 0.0, 0.0, 0.0];
                let empty_line = vec![default_pixel; resolution.width()];
                let empty_image = vec![empty_line; resolution.height()];
                empty_image
            },
            |pixel_vector, position, (r, g, b, _): (f32, f32, f32, f32)| {
                pixel_vector[position.y()][position.x()] = [r, g, b, 0.0]
            },
        )?;

        let resolution = image.layer_data.size;

        let dim = [resolution.width() as usize, resolution.height() as usize];

        // Convert the pixels to Vec<Vector3<f32>> using an iterator
        let data: Vec<Vec3> = image
            .layer_data
            .channel_data
            .pixels
            .into_iter()
            .flatten()
            .map(|v| Vec3::new(v[0], v[1], v[2]))
            .collect();

        Ok(Self { dim, data })
    }
    #[must_use]
    pub fn sample(&self, uv: Vec2) -> Vec3 {
        // since it's (theta, phi)
        let x = uv.y.clamp(0.0, 1.0) * (self.dim[0] - 1) as f32;
        let y = uv.x.clamp(0.0, 1.0) * (self.dim[1] - 1) as f32;
        let index = x as usize + y as usize * self.dim[0];

        self.data[index]
    }
}
