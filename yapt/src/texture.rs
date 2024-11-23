use crate::prelude::*;

pub enum Texture {
    Image(Image),
    Solid(Vec3),
}

pub struct Image {
    backing: Vec<f32>,
    width: usize,
    height: usize,
}
