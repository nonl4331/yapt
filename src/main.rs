pub const WIDTH: usize = 1080;
pub const HEIGHT: usize = 1080;
const SAMPLES: u64 = 1000;

pub mod camera;
pub mod integrator;
pub mod loader;
pub mod material;
pub mod triangle;

pub mod prelude {
    pub use crate::material::Mat;
    pub use crate::triangle::Tri;
    pub use crate::Intersection;
    pub use crate::{SAMPLABLE, HEIGHT, MATERIALS, MATERIAL_NAMES, NORMALS, TRIANGLES, VERTICES, WIDTH};
    pub use bvh::Bvh;
    pub use derive_new::new;
    pub use utility::{Ray, Vec3};
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
    loader::add_material("matte", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE * 3.0)));

    let model_map = loader::create_model_map(vec![
        ("top_light", "light"),
        ("bottom_light", "light"),
        ("centre", "matte"),
    ]);

    loader::load_obj("res/test.obj", 1.0, Vec3::ZERO, model_map);
}

fn main() {
    create_logger();

    unsafe { scene_init() };

    let mut buffer = vec![Vec3::ZERO; WIDTH * HEIGHT];

    let cam = Cam::new(
        Vec3::new(0.0, -2.6, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::Z,
        90.0,
        10.0,
    );
    let bvh = unsafe { Bvh::new(&mut TRIANGLES) };

    let bar = ProgressBar::new(SAMPLES).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .unwrap(),
    );

    for sample in 1..=SAMPLES {
        let start = std::time::Instant::now();

        let sample_ray_count = buffer
            .par_iter_mut()
            .enumerate()
            .map(|(i, pixel)| {
                let mut rng = thread_rng();
                let ray = cam.get_ray(i, &mut rng);

                let (col, rays) = NEEMIS::rgb(ray, &bvh, &mut rng, unsafe {&SAMPLABLE});
                //let (col, rays) = Naive::rgb(ray, &bvh, &mut rng);

                *pixel += (col - *pixel) / sample as f32;

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

    let img = image::Rgb32FImage::from_vec(
        WIDTH as u32,
        HEIGHT as u32,
        buffer
            .iter()
            .flat_map(|v| [v.x, v.y, v.z])
            .collect::<Vec<f32>>(),
    )
    .unwrap();

    img.save("out.exr").unwrap();
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
