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
    /*pub fn from_u8(width: usize, height: usize, data: &[u8]) -> Self {
        assert!(width * height * 4 == data.len());
        let backing: Vec<_> = data
            .array_windows::<4>()
            .map(|[r, g, b, ..]| Vec3::new(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0))
            .collect();
        Self {
            width,
            height,
            backing,
        }
    }*/
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
                let x = ((img.width - 1) as f32 * uv.x) as usize;
                let y = ((img.height - 1) as f32 * uv.y) as usize;
                img.backing[x + img.width * y]
            }
            Self::Solid(v) => *v,
        }
    }
}
