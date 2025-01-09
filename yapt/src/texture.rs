use gltf::material::AlphaMode;

use crate::prelude::*;

#[derive(Debug)]
pub enum Texture {
    Image(Image),
    Solid(Vec3),
}

#[derive(Debug)]
pub struct Image {
    pub backing: Vec<[f32; 4]>,
    width: usize,
    height: usize,
}

impl Image {
    pub fn from_rgbaf32(
        width: usize,
        height: usize,
        mut data: Vec<f32>,
        alpha_mode: AlphaMode,
        alpha_cuttoff: f32,
    ) -> Self {
        assert!(width * height * 4 == data.len());
        for e in data.iter_mut().skip(3).step_by(4) {
            *e = match alpha_mode {
                AlphaMode::Opaque => 1.0,
                AlphaMode::Mask => {
                    if *e > alpha_cuttoff {
                        1.0
                    } else {
                        0.0
                    }
                }
                AlphaMode::Blend => *e,
            };
        }
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
            Self::Solid(_v) => true,
        }
    }
}
