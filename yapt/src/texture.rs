use crate::prelude::*;

#[derive(Debug)]
pub enum Texture {
    Image(Image),
    Solid(Vec3),
}

#[derive(Debug)]
pub struct Image {
    backing: Vec<[f32; 4]>,
    width: usize,
    height: usize,
}

impl Image {
    pub fn from_rgbaf32(width: usize, height: usize, data: Vec<f32>) -> Self {
        assert!(width * height * 4 == data.len());
        Self {
            width,
            height,
            backing: unsafe { std::mem::transmute(data) },
        }
    }
}

impl Texture {
    pub fn uv_value(&self, uv: Vec2) -> Vec3 {
        match self {
            Self::Image(img) => {
                let u = uv.x.fract().abs();
                let v = uv.y.fract().abs();
                let x = ((img.width - 1) as f32 * u) as usize;
                let y = ((img.height - 1) as f32 * v) as usize;
                let [r, g, b, _a] = img.backing[x + img.width * y];
                Vec3::new(r, g, b)
            }
            Self::Solid(v) => *v,
        }
    }
    pub fn does_intersect(&self, uv: Vec2, rng: &mut impl MinRng) -> bool {
        match self {
            Self::Image(img) => {
                let u = uv.x.fract().abs();
                let v = uv.y.fract().abs();
                let x = ((img.width - 1) as f32 * u) as usize;
                let y = ((img.height - 1) as f32 * v) as usize;
                img.backing[x + img.width * y][3] >= rng.gen()
            }
            Self::Solid(_v) => false,
        }
    }
}
