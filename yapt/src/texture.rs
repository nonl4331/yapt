use crate::prelude::*;

#[derive(Debug)]
pub enum Texture {
    Image(Image),
    Solid(Vec3),
}

#[derive(Debug)]
pub struct Image {
    backing: Vec<Vec3>,
    width: usize,
    height: usize,
}

impl Image {
    pub fn from_rgbf32(width: usize, height: usize, data: Vec<f32>) -> Self {
        assert!(width * height * 3 == data.len());
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
                img.backing[x + img.width * y]
            }
            Self::Solid(v) => *v,
        }
    }
}
