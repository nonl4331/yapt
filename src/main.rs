pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 1024;
const SAMPLES: u64 = 10000;
const FILENAME: &str = "out.exr";

pub mod camera;
pub mod coord;
pub mod distributions;
pub mod envmap;
pub mod film;
pub mod integrator;
pub mod loader;
pub mod material;
pub mod pssmlt;
pub mod startup;
pub mod triangle;

pub mod prelude {
    pub use crate::envmap::*;
    pub use crate::film::*;
    pub use crate::material::Mat;
    pub use crate::pssmlt::MinRng;
    pub use crate::triangle::Tri;
    pub use crate::Intersection;
    pub use crate::{
        ENVMAP, HEIGHT, MATERIALS, MATERIAL_NAMES, NORMALS, SAMPLABLE, TRIANGLES, VERTICES, WIDTH,
    };
    pub use bvh::Bvh;
    pub use derive_new::new;
    pub use std::f32::consts::*;
    pub use utility::{Ray, Vec2, Vec3};
}

use crate::{camera::Cam, material::*, startup::Args, triangle::Tri};
use once_cell::unsync::Lazy;
use prelude::*;
use std::{collections::HashMap, fmt};

pub static mut VERTICES: Vec<Vec3> = vec![];
pub static mut NORMALS: Vec<Vec3> = vec![];
pub static mut MATERIALS: Vec<Mat> = vec![];
pub static mut TRIANGLES: Vec<Tri> = vec![];
pub static mut SAMPLABLE: Vec<usize> = vec![];
pub static mut BVH: Bvh = Bvh { nodes: vec![] };
pub static mut MATERIAL_NAMES: Lazy<HashMap<String, usize>> = Lazy::new(HashMap::new);
pub static mut ENVMAP: EnvMap = EnvMap::DEFAULT;

const MAGIC_VALUE_ONE: f32 = 543543521.0;
const MAGIC_VALUE_ONE_VEC: Vec3 = Vec3::new(MAGIC_VALUE_ONE, MAGIC_VALUE_ONE, MAGIC_VALUE_ONE);
const MAGIC_VALUE_TWO: f32 = 5435421.5;
const MAGIC_VALUE_TWO_VEC: Vec3 = Vec3::new(MAGIC_VALUE_TWO, MAGIC_VALUE_TWO, MAGIC_VALUE_TWO);

#[derive(clap::ValueEnum, Copy, Clone)]
pub enum IntegratorType {
    Naive,
    NEE,
}

impl fmt::Display for IntegratorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Naive => "naive",
            Self::NEE => "nee",
        };
        write!(f, "{s}")
    }
}

#[derive(clap::ValueEnum, Copy, Clone)]
pub enum Scene {
    One,
    Car,
    Sphere,
}

impl fmt::Display for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::One => "one",
            Self::Car => "car",
            Self::Sphere => "sphere",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, new)]
pub struct Intersection {
    pub t: f32,
    pub pos: Vec3,
    pub nor: Vec3,
    pub out: bool,
    pub mat: usize,
    pub id: usize,
}

impl Intersection {
    pub const NONE: Self = Self {
        t: -1.0,
        pos: Vec3::ZERO,
        nor: Vec3::ZERO,
        out: false,
        mat: 0,
        id: 0,
    };

    pub fn is_none(&self) -> bool {
        self.t == -1.0
    }

    pub fn min(&mut self, other: Self) {
        if self.is_none() || (other.t < self.t && other.t > 0.0) {
            *self = other;
        }
    }
}

unsafe fn setup_scene(args: &Args) -> Cam {
    match args.scene {
        Scene::One => scene_one(args),
        Scene::Car => scene_car(args),
        Scene::Sphere => scene_sphere(args),
    }
}

unsafe fn scene_one(args: &Args) -> Cam {
    loader::add_material("floor", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("ball1", Mat::Matte(Matte::new(Vec3::new(0.5, 0.8, 0.8))));
    loader::add_material("ball2", Mat::Matte(Matte::new(Vec3::new(0.8, 0.0, 0.0))));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE * 3.0)));

    let model_map = loader::create_model_map(vec![
        ("floor", "floor"),
        ("ball1", "ball1"),
        ("light", "light"),
        ("ball2", "ball2"),
    ]);

    loader::load_obj("res/one.obj", 1.0, Vec3::ZERO, model_map);

    Cam::new(
        Vec3::new(0.0, -1.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::Z,
        70.0,
        1.0,
        args.width,
        args.height,
    )
}

unsafe fn scene_car(args: &Args) -> Cam {
    let test_mat = Mat::Glossy(Ggx::new(0.1, Vec3::X * 0.8));
    loader::add_material("default", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("floor", Mat::Matte(Matte::new(Vec3::ONE * 0.8)));
    loader::add_material("test", test_mat);
    //loader::add_material("test", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE * 3.0)));

    let model_map = loader::create_model_map(vec![
        ("default", "default"),
        ("floor", "floor"),
        ("light", "light"),
        ("BodyMat", "test"),
    ]);

    loader::load_obj("res/sports_car.obj", 1.0, Vec3::ZERO, model_map);

    Cam::new(
        Vec3::new(2.0, -4.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::Z,
        70.0,
        1.0,
        args.width,
        args.height,
    )
}

unsafe fn scene_sphere(args: &Args) -> Cam {
    let test_mat = Mat::Glossy(Ggx::new(0.001, Vec3::ONE));
    loader::add_material("default", test_mat);

    let model_map = loader::create_model_map(vec![("default", "default")]);

    loader::load_obj("res/sphere.obj", 1.0, Vec3::ZERO, model_map);

    Cam::new(
        Vec3::new(0.0, -3.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        args.width,
        args.height,
    )
}

unsafe fn scene_furnace_test(args: &Args) -> Cam {
    loader::add_material("Inner", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE)));

    let model_map = loader::create_model_map(vec![("Inner", "Inner"), ("Outer", "light")]);

    loader::load_obj("res/furnace_test.obj", 1.0, Vec3::ZERO, model_map);

    Cam::new(
        Vec3::new(-4.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        args.width,
        args.height,
    )
}

fn main() {
    startup::run();
}
