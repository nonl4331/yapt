pub const WIDTH: usize = 1024;
pub const HEIGHT: usize = 1024;
const SAMPLES: u64 = 10000;
const FILENAME: &'static str = "out_naive.exr";

pub mod camera;
pub mod coord;
pub mod integrator;
pub mod loader;
pub mod material;
pub mod triangle;
pub mod film;

pub mod prelude {
    pub use crate::material::Mat;
    pub use crate::triangle::Tri;
    pub use crate::Intersection;
    pub use crate::{
        HEIGHT, MATERIALS, MATERIAL_NAMES, NORMALS, SAMPLABLE, TRIANGLES, VERTICES, WIDTH,
    };
    pub use bvh::Bvh;
    pub use derive_new::new;
    pub use utility::{Ray, Vec2, Vec3};
    pub use crate::film::*;
}

use crate::{camera::Cam, integrator::*, material::*, triangle::Tri};
use fern::colors::{Color, ColoredLevelConfig};
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::unsync::Lazy;
use prelude::*;
use rand::thread_rng;
use rayon::prelude::*;
use std::collections::HashMap;

pub static mut VERTICES: Vec<Vec3> = vec![];
pub static mut NORMALS: Vec<Vec3> = vec![];
pub static mut MATERIALS: Vec<Mat> = vec![];
pub static mut TRIANGLES: Vec<Tri> = vec![];
pub static mut SAMPLABLE: Vec<usize> = vec![];
pub static mut BVH: Bvh = Bvh { nodes: vec![] };
pub static mut MATERIAL_NAMES: Lazy<HashMap<String, usize>> = Lazy::new(|| HashMap::new());

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

unsafe fn scene_init() {
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

    loader::load_obj("res/test1.obj", 1.0, Vec3::ZERO, model_map);
}

fn main() {
    create_logger();

    unsafe { scene_init() };

	let (send, recv) = std::sync::mpsc::channel();
	let film = Film::new(recv);
	let child = film.child(send);

    // test3
    let cam = Cam::new(
        Vec3::new(0.0, -1.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::Z,
        70.0,
        1.0,
    );

    let bvh = unsafe { Bvh::new(&mut TRIANGLES) };

    // calculate samplable objects after BVH rearranges TRIANGLES
    unsafe {
        for (i, tri) in TRIANGLES.iter().enumerate() {
            if let Mat::Light(_) = MATERIALS[tri.mat] {
                SAMPLABLE.push(i);
            }
        }
    }

    let bar = ProgressBar::new(SAMPLES).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .unwrap(),
    );

	let film_thread = std::thread::spawn(move || film.run());

    for sample in 1..=SAMPLES {
        let start = std::time::Instant::now();

        let sample_ray_count = (0..(WIDTH*HEIGHT)).collect::<Vec<_>>()
            .par_chunks(1024).enumerate()
            .map(|(i, c)| {
                let c = c.len();
                let offset = 1024*i;
                let mut splats = child.clone().get_vec();
                let mut rng = thread_rng();
                let mut rays = 0;
                for idx in offset..(offset+c) {
                    let (uv, ray) = cam.get_ray(idx, &mut rng);
                    //let (col, rays) = NEEMIS::rgb(ray, &bvh, &mut rng, unsafe { &SAMPLABLE });
                    let (col, ray_count) = Naive::rgb(ray, &bvh, &mut rng);
                    splats.push(Splat::new(uv, col));
                    rays += ray_count;
                }
                child.clone().add_splats(splats);

                rays
            })
            .sum::<u64>();
        

        let dur = start.elapsed();

        bar.set_position(sample);
        bar.set_message(format!(
            "{:.2} MRay/s ({})",
            sample_ray_count as f64 * 0.000001 / dur.as_secs_f64(),
            dur.as_millis()
        ));
    }

    bar.finish_and_clear();

    child.add_splats(Vec::new());
	let buffer = film_thread.join().unwrap();
    let m = 1.0 / SAMPLES as f32;

    let img = image::Rgb32FImage::from_vec(
        WIDTH as u32,
        HEIGHT as u32,
        buffer
            .iter()
            .flat_map(|v| [v.x * m, v.y * m, v.z * m])
            .collect::<Vec<f32>>(),
    )
    .unwrap();

    img.save(FILENAME).unwrap();
}

fn create_logger() {
    let colors = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Cyan)
        .debug(Color::Magenta);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} {} [{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for("winit", log::LevelFilter::Warn)
        .chain(std::io::stderr())
        .apply()
        .unwrap();
}
