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
    pub use crate::{
        camera::Cam, envmap::*, film::*, integrator::*, material::*, pssmlt::MinRng, startup::Args,
        triangle::Tri, IntegratorType, Intersection, Scene, ENVMAP, HEIGHT, MATERIALS,
        MATERIAL_NAMES, NORMALS, SAMPLABLE, TRIANGLES, VERTICES, WIDTH,
    };
    pub use bvh::Bvh;
    pub use derive_new::new;
    pub use std::{
        f32::consts::*,
        ptr::{addr_of, addr_of_mut},
    };
    pub use utility::{Ray, Vec2, Vec3};
}

use crate::{
    camera::Cam,
    material::{Ggx, Light, Mat, Matte},
    startup::Args,
    triangle::Tri,
};
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
    SphereLeftRight,
    FurnaceTest,
    Room,
}

impl fmt::Display for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::One => "one",
            Self::Car => "car",
            Self::Sphere => "sphere",
            Self::SphereLeftRight => "sphere_left_right",
            Self::FurnaceTest => "furnace_test",
            Self::Room => "room",
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

    #[allow(clippy::float_cmp)]
    #[must_use]
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
        Scene::SphereLeftRight => scene_sphere_left_right(args),
        Scene::FurnaceTest => scene_furnace_test(args),
        Scene::Room => scene_room(args),
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

    loader::load_obj("res/one.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(0.0, -1.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::Z,
        70.0,
        1.0,
        args,
    )
}

unsafe fn scene_car(_args: &Args) -> Cam {
    unimplemented!();
}

unsafe fn scene_sphere(args: &Args) -> Cam {
    let test_mat = Mat::Glossy(Ggx::new(0.001, Vec3::ONE));
    loader::add_material("default", test_mat);

    let model_map = loader::create_model_map(vec![("default", "default")]);

    loader::load_obj("res/sphere.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(0.0, -3.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        args,
    )
}

unsafe fn scene_sphere_left_right(args: &Args) -> Cam {
    let test_mat = Mat::Glossy(Ggx::new(0.001, Vec3::ONE));
    loader::add_material("test mat", test_mat);
    loader::add_material("diffuse", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material(
        "orange light",
        Mat::Light(Light::new(Vec3::new(1.0, 0.65, 0.0))),
    );
    loader::add_material(
        "blue light",
        Mat::Light(Light::new(Vec3::new(0.68, 0.85, 0.9))),
    );

    let model_map = loader::create_model_map(vec![
        ("floor", "diffuse"),
        ("sphere", "test mat"),
        ("right_light", "orange light"),
        ("left_light", "blue light"),
    ]);

    loader::load_obj("res/sphere_left_right.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(0.0, -3.0, 2.0),
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::Z,
        70.0,
        1.0,
        args,
    )
}

unsafe fn scene_furnace_test(args: &Args) -> Cam {
    loader::add_material("Inner", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE)));

    let model_map = loader::create_model_map(vec![("Inner", "Inner"), ("Outer", "light")]);

    loader::load_obj("res/furnace_test.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(-4.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        args,
    )
}

unsafe fn scene_room(args: &Args) -> Cam {
    loader::add_material("rest", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("empty", Mat::Invisible);
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE * 1.0)));

    let model_map = loader::create_model_map(vec![
        ("grey_and_white_room:lambert2SG_light", "empty"),
        ("lambert3SG", "empty"),
        ("grey_and_white_room:Glass", "empty"),
        ("grey_and_white_room:lambert2SG_light", "empty"),
        ("light", "light"),
    ]);

    loader::load_obj("res/room.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(3.0, -3.0, 1.8),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        args,
    )
}

fn main() {
    startup::run();
}
