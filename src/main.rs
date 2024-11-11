#![feature(get_mut_unchecked)]
#[allow(static_mut_refs)]
pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 1024;
const SAMPLES: u64 = 10000;
const FILENAME: &str = "out.exr";

pub mod camera;
pub mod coord;
pub mod distributions;
pub mod envmap;
pub mod gui;
pub mod integrator;
pub mod loader;
pub mod material;
pub mod pssmlt;
pub mod scene;
pub mod triangle;
pub mod work_handler;

pub mod prelude {
    pub use crate::{
        camera::Cam, envmap::*, integrator::*, loader, material::*, pssmlt::MinRng, scene::Scene,
        triangle::Tri, work_handler::*, IntegratorType, Intersection, RenderSettings, Splat, BVH,
        CAM, ENVMAP, HEIGHT, MATERIALS, MATERIAL_NAMES, NORMALS, SAMPLABLE, TRIANGLES, VERTICES,
        WIDTH,
    };
    pub use bvh::Bvh;
    pub use derive_new::new;
    pub use std::{
        collections::HashMap,
        f32::consts::*,
        fmt,
        ptr::{addr_of, addr_of_mut},
        sync::Arc,
    };
    pub use utility::{Ray, Vec2, Vec3};
}
use prelude::*;

use clap::Parser;
use once_cell::unsync::Lazy;

const CHUNK_SIZE: usize = 4096;

const BOOTSTRAP_CHAINS: usize = 100_000;
const CHAINS: usize = 100;

pub static mut VERTICES: Vec<Vec3> = vec![];
pub static mut NORMALS: Vec<Vec3> = vec![];
pub static mut MATERIALS: Vec<Mat> = vec![];
pub static mut TRIANGLES: Vec<Tri> = vec![];
pub static mut SAMPLABLE: Vec<usize> = vec![];
pub static mut BVH: Bvh = Bvh { nodes: vec![] };
pub static mut MATERIAL_NAMES: Lazy<HashMap<String, usize>> = Lazy::new(HashMap::new);
pub static mut ENVMAP: EnvMap = EnvMap::DEFAULT;
pub static mut CAM: Cam = crate::camera::PLACEHOLDER;

const MAGIC_VALUE_ONE: f32 = 543543521.0;
const MAGIC_VALUE_ONE_VEC: Vec3 = Vec3::new(MAGIC_VALUE_ONE, MAGIC_VALUE_ONE, MAGIC_VALUE_ONE);
const MAGIC_VALUE_TWO: f32 = 5435421.5;
const MAGIC_VALUE_TWO_VEC: Vec3 = Vec3::new(MAGIC_VALUE_TWO, MAGIC_VALUE_TWO, MAGIC_VALUE_TWO);

#[derive(clap::ValueEnum, Copy, Clone, Default)]
pub enum IntegratorType {
    Naive,
    #[default]
    NEE,
}

pub struct Splat {
    uv: [f32; 2],
    rgb: Vec3,
}

impl Splat {
    pub fn new(uv: [f32; 2], rgb: Vec3) -> Self {
        Self { uv, rgb }
    }
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

fn main() {
    create_logger();

    let args = RenderSettings::parse();

    eframe::run_native(
        "yapt",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            // framebuffer for displying render to the screen
            let fb_handle = cc.egui_ctx.load_texture(
                "fb",
                egui::ImageData::Color(std::sync::Arc::new(egui::ColorImage::new(
                    [args.width as usize, args.height as usize],
                    egui::Color32::BLACK,
                ))),
                egui::TextureOptions::default(),
            );

            let app = App::new(fb_handle, args.clone(), cc.egui_ctx.clone());

            Ok(Box::new(app))
        }),
    )
    .unwrap();
}

#[derive(Parser, Clone)]
#[command(about, long_about = None)]
pub struct RenderSettings {
    #[arg(short, default_value_t = false)]
    pub bvh_heatmap: bool,
    #[arg(short, long, default_value_t = crate::WIDTH)]
    pub width: u32,
    #[arg(short, long, default_value_t = crate::HEIGHT)]
    pub height: u32,
    #[arg(short = 'n', long, default_value_t = crate::SAMPLES)]
    pub samples: u64,
    #[arg(short='o', long, default_value_t = String::from(crate::FILENAME))]
    pub filename: String,
    #[arg(short, long, default_value_t = IntegratorType::default())]
    pub integrator: IntegratorType,
    #[arg(short, long, default_value_t = Scene::default())]
    pub scene: Scene,
    #[arg(short, default_value_t = false)]
    pub pssmlt: bool,
    #[arg(short, long)]
    pub environment_map: Option<String>,
    #[arg(long, default_value_t = 0.0)]
    pub u_low: f32,
    #[arg(long, default_value_t = 1.0)]
    pub u_high: f32,
    #[arg(long, default_value_t = 0.0)]
    pub v_low: f32,
    #[arg(long, default_value_t = 1.0)]
    pub v_high: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            bvh_heatmap: false,
            width: crate::WIDTH,
            height: crate::HEIGHT,
            samples: crate::SAMPLES,
            filename: String::from(crate::FILENAME),
            integrator: IntegratorType::default(),
            scene: Scene::default(),
            pssmlt: false,
            environment_map: None,
            u_low: 0.0,
            u_high: 1.0,
            v_low: 0.0,
            v_high: 1.0,
        }
    }
}

pub struct App {
    pub render_settings: RenderSettings,
    // egui state
    pub fb_tex_handle: egui::TextureHandle,
    pub context: egui::Context,
    // communication
    pub update_recv: std::sync::mpsc::Receiver<Update>,
    pub work_req: std::sync::mpsc::Sender<ComputeChange>,
    // state
    pub canvas: Vec<Vec3>,
    pub splats_done: u64,
    pub work_rays: u64,
    // work statistics
    pub work_start: std::time::Instant,
    pub last_update: std::time::Instant,
    pub updated: bool,
    pub workload_id: u8,
    // gui state
    pub display_settings: bool,
}

impl App {
    pub fn new(
        fb_tex_handle: egui::TextureHandle,
        render_settings: RenderSettings,
        context: egui::Context,
    ) -> Self {
        let (update_recv, work_req) = work_handler::create_work_handler();
        let mut a = Self {
            fb_tex_handle,
            render_settings,
            context,
            update_recv,
            work_req,
            canvas: Vec::new(),
            splats_done: 0,
            work_start: std::time::Instant::now(),
            last_update: std::time::Instant::now(),
            workload_id: 0,
            work_rays: 0,
            updated: false,
            display_settings: false,
        };
        a.init();
        if a.render_settings.samples != 0 {
            a.work_req
                .send(ComputeChange::WorkSamples(
                    a.render_settings.samples,
                    a.workload_id,
                ))
                .unwrap();
            a.work_start = std::time::Instant::now();
        }
        a
    }
    fn init(&mut self) {
        let rs = &mut self.render_settings;
        assert!(rs.u_low >= 0.0);
        assert!(rs.u_high >= rs.u_low && rs.u_low <= 1.0);
        assert!(rs.v_low >= 0.0);
        assert!(rs.v_high >= rs.v_low && rs.v_low <= 1.0);

        self.canvas = vec![Vec3::ZERO; rs.width as usize * rs.height as usize];

        if let Some(ref path) = rs.environment_map {
            if let Ok(image) = TextureData::from_path(path) {
                unsafe { crate::ENVMAP = EnvMap::Image(image) };
                log::info!("Loaded envmap");
            } else {
                log::warn!("Could not import envmap {path}.");
            }
        }

        unsafe {
            CAM = crate::scene::setup_scene(&rs);
            BVH = Bvh::new(&mut *addr_of_mut!(TRIANGLES));

            // calculate samplable objects after BVH rearranges TRIANGLES
            for (i, tri) in TRIANGLES.iter().enumerate() {
                if let Mat::Light(_) = MATERIALS[tri.mat] {
                    SAMPLABLE.push(i);
                }
            }
        }

        let state = State::new(
            rs.width as usize,
            rs.height as usize,
            self.context.clone(),
            rs.integrator,
            0,
        );

        self.work_req
            .send(ComputeChange::UpdateState(state))
            .unwrap();
    }
    // reset canvas and state and prepare for a new workload
    pub fn next_workload(&mut self) {
        let state = State::new(
            self.render_settings.width as usize,
            self.render_settings.height as usize,
            self.context.clone(),
            self.render_settings.integrator,
            0,
        );
        self.work_req
            .send(ComputeChange::UpdateState(state))
            .unwrap();
        self.workload_id = self.workload_id.wrapping_add(1);
        self.canvas = vec![
            Vec3::ZERO;
            self.render_settings.width as usize
                * self.render_settings.height as usize
        ];
        self.work_rays = 0;
        self.splats_done = 0;
        self.updated = true;
        self.last_update = std::time::Instant::now();
        self.work_start = std::time::Instant::now();
    }
}

// REC.2020 -> XYZ.Y (not entirely sure if this is correct)
fn scalar_contribution(rgb: Vec3) -> f32 {
    (0.144616903586208 * rgb.x + 0.677998071518871 * rgb.y + 0.0280726930490874 * rgb.z).max(0.0001)
    // max is to avoid NAN
}

pub fn heatmap(t: f32) -> Vec3 {
    const C0: Vec3 = Vec3::new(-0.020390, 0.009557, 0.018508);
    const C1: Vec3 = Vec3::new(3.108226, -0.106297, -1.105891);
    const C2: Vec3 = Vec3::new(-14.539061, -2.943057, 14.548595);
    const C3: Vec3 = Vec3::new(71.394557, 22.644423, -71.418400);
    const C4: Vec3 = Vec3::new(-152.022488, -31.024563, 152.048692);
    const C5: Vec3 = Vec3::new(139.593599, 12.411251, -139.604042);
    const C6: Vec3 = Vec3::new(-46.532952, -0.000874, 46.532928);
    C0 + (C1 + (C2 + (C3 + (C4 + (C5 + C6 * t) * t) * t) * t) * t) * t
}

pub fn create_logger() {
    // ensure default log level when
    // RUST_LOG isn't set is info
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
}
