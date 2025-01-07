#![feature(
    get_mut_unchecked,
    sync_unsafe_cell,
    ptr_as_ref_unchecked,
    once_cell_get_mut,
    array_windows,
    iter_map_windows
)]
pub const WIDTH: std::num::NonZeroU32 = unsafe { std::num::NonZeroU32::new_unchecked(1024) };
pub const HEIGHT: std::num::NonZeroU32 = unsafe { std::num::NonZeroU32::new_unchecked(1024) };
const SAMPLES: u64 = 1000;
pub const NO_TEXTURE: usize = usize::MAX;

pub mod camera;
pub mod coord;
pub mod distributions;
pub mod envmap;
#[cfg(feature = "gui")]
pub mod gui;
pub mod integrator;
pub mod loader;
pub mod material;
pub mod pssmlt;
pub mod scene;
pub mod texture;
pub mod triangle;
pub mod work_handler;

pub mod prelude {
    pub use crate::{
        camera::Cam, coord::*, envmap::*, integrator::*, loader, material::*, pssmlt::MinRng,
        texture::*, triangle::Tri, work_handler::*, IntegratorType, Intersection, RenderSettings,
        Splat, BVH, CAM, ENVMAP, HEIGHT, MATERIALS, MATERIAL_NAMES, NORMALS, SAMPLABLE, TEXTURES,
        TEXTURE_NAMES, TRIANGLES, UVS, VERTICES, WIDTH,
    };
    pub use bvh::Bvh;
    pub use derive_new::new;
    pub use std::{
        cell::SyncUnsafeCell,
        collections::HashMap,
        f32::consts::*,
        fmt,
        ptr::{addr_of, addr_of_mut},
        sync::Arc,
    };
    pub use utility::{Ray, Vec2, Vec3};
}
use std::sync::Mutex;

use prelude::*;

use clap::Parser;

const CHUNK_SIZE: usize = 4096;

const BOOTSTRAP_CHAINS: usize = 100_000;
const CHAINS: usize = 100;

pub static VERTICES: SyncUnsafeCell<Vec<Vec3>> = SyncUnsafeCell::new(vec![]);
pub static NORMALS: SyncUnsafeCell<Vec<Vec3>> = SyncUnsafeCell::new(vec![]);
pub static UVS: SyncUnsafeCell<Vec<Vec2>> = SyncUnsafeCell::new(vec![]);
pub static MATERIALS: SyncUnsafeCell<Vec<Mat>> = SyncUnsafeCell::new(vec![]);
pub static TEXTURES: SyncUnsafeCell<Vec<Texture>> = SyncUnsafeCell::new(vec![]);
pub static TRIANGLES: SyncUnsafeCell<Vec<Tri>> = SyncUnsafeCell::new(vec![]);
pub static SAMPLABLE: SyncUnsafeCell<Vec<usize>> = SyncUnsafeCell::new(vec![]);
pub static BVH: SyncUnsafeCell<Bvh> = SyncUnsafeCell::new(Bvh { nodes: vec![] });
pub static MATERIAL_NAMES: Mutex<std::cell::OnceCell<HashMap<String, usize>>> =
    Mutex::new(std::cell::OnceCell::new());
pub static TEXTURE_NAMES: Mutex<std::cell::OnceCell<HashMap<String, usize>>> =
    Mutex::new(std::cell::OnceCell::new());
pub static ENVMAP: SyncUnsafeCell<EnvMap> = SyncUnsafeCell::new(EnvMap::DEFAULT);
pub static CAM: SyncUnsafeCell<Cam> = SyncUnsafeCell::new(crate::camera::PLACEHOLDER);

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
    pub uv: Vec2,
    pub pos: Vec3,
    pub nor: Vec3,
    pub out: bool,
    pub mat: usize,
    pub id: usize,
}

impl Intersection {
    pub const NONE: Self = Self {
        t: -1.0,
        uv: Vec2::ZERO,
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

    // GUI mode
    #[cfg(feature = "gui")]
    if !args.headless {
        eframe::run_native(
            "yapt",
            eframe::NativeOptions::default(),
            Box::new(|cc| {
                // framebuffer for displying render to the screen
                let fb_handle = cc.egui_ctx.load_texture(
                    "fb",
                    egui::ImageData::Color(std::sync::Arc::new(egui::ColorImage::new(
                        [
                            u32::from(args.width) as usize,
                            u32::from(args.height) as usize,
                        ],
                        egui::Color32::BLACK,
                    ))),
                    egui::TextureOptions::default(),
                );

                let app = App::new(Some((cc.egui_ctx.clone(), fb_handle)), args.clone());

                Ok(Box::new(app))
            }),
        )
        .unwrap();
        return;
    }

    // headless mode
    let mut app = App::new(
        #[cfg(feature = "gui")]
        None,
        args.clone(),
    );
    let rs = &mut app.render_settings;
    while let Ok(update) = app.update_recv.recv() {
        match update {
            Update::Calculation(splats, workload_id, ray_count)
                if workload_id == app.workload_id =>
            {
                app.work_duration += app.work_start.elapsed();
                app.work_start = std::time::Instant::now();
                app.splats_done += splats.len() as u64;

                // add splats to image
                for splat in splats {
                    let uv = splat.uv;
                    let idx = {
                        assert!(uv[0] <= 1.0 && uv[1] <= 1.0);

                        let x = (uv[0] * u32::from(rs.width) as f32) as usize;
                        let y = (uv[1] * u32::from(rs.height) as f32) as usize;

                        (y * u32::from(rs.width) as usize + x)
                            .min(u32::from(rs.width) as usize * u32::from(rs.height) as usize - 1)
                    };

                    app.canvas[idx] += splat.rgb;
                    app.updated = true;
                }
                app.work_rays += ray_count;

                // update progress
                if app.updated && app.last_update.elapsed() > std::time::Duration::from_millis(250)
                {
                    log::info!(
                        "Mrays: {:.2} - Rays shot: {} - elapsed: {:.1}",
                        (app.work_rays as f64 / app.work_duration.as_secs_f64()) / 1000000 as f64,
                        app.work_rays,
                        app.work_duration.as_secs_f64(),
                    );
                    app.updated = false;
                    app.last_update = std::time::Instant::now();
                }

                // work queue cleared
                if app.splats_done
                    == u32::from(rs.width) as u64 * u32::from(rs.height) as u64 * rs.samples
                {
                    log::info!(
                            "Render finished: Mrays: {:.2} - Rays shot: {} - elapsed: {:.1} - samples: {}",
                            (app.work_rays as f64 / app.work_duration.as_secs_f64())
                                / 1000000 as f64,
                            app.work_rays,
                            app.work_duration.as_secs_f64(),
                            rs.samples
                        );
                    break;
                }
            }
            Update::Calculation(_, workload_id, _) => {
                log::trace!("Got splats from previous workload {workload_id}!")
            }
            Update::PssmltBootstrapDone => log::info!("PSSMLT bootstrap done!"),
            Update::NoState => log::info!("No state found!"),
        }
    }
}

#[derive(Parser, Clone)]
#[command(about, long_about = None, disable_help_flag = true)]
pub struct RenderSettings {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    pub help: Option<bool>,
    #[arg(short, default_value_t = false)]
    pub bvh_heatmap: bool,
    #[arg(short, long, default_value_t = crate::WIDTH)]
    pub width: std::num::NonZeroU32,
    #[arg(short, long, default_value_t = crate::HEIGHT)]
    pub height: std::num::NonZeroU32,
    #[arg(short = 'n', long, default_value_t = crate::SAMPLES)]
    pub samples: u64,
    #[arg(short='o', long, default_value_t = String::new())]
    pub filename: String,
    #[arg(short, long, default_value_t = IntegratorType::default())]
    pub integrator: IntegratorType,
    #[arg(short, long, default_value_t = String::new())]
    pub scene: String,
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
    #[arg(long)]
    pub num_threads: Option<std::num::NonZeroUsize>,
    #[cfg(feature = "gui")]
    #[arg(long)]
    pub headless: bool,
    #[arg(short, long, default_value_t = 0)]
    camera_idx: usize,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            help: None,
            bvh_heatmap: false,
            width: crate::WIDTH,
            height: crate::HEIGHT,
            samples: crate::SAMPLES,
            filename: String::new(),
            integrator: IntegratorType::default(),
            scene: String::default(),
            pssmlt: false,
            environment_map: None,
            u_low: 0.0,
            u_high: 1.0,
            v_low: 0.0,
            v_high: 1.0,
            num_threads: None,
            #[cfg(feature = "gui")]
            headless: false,
            camera_idx: 0,
        }
    }
}

pub struct App {
    pub render_settings: RenderSettings,
    // egui state
    #[cfg(feature = "gui")]
    pub egui_state: Option<(egui::Context, egui::TextureHandle)>,
    // communication
    pub update_recv: std::sync::mpsc::Receiver<Update>,
    pub work_req: std::sync::mpsc::Sender<ComputeChange>,
    // state
    pub canvas: Vec<Vec3>,
    pub splats_done: u64,
    pub work_rays: u64,
    // work statistics
    pub work_duration: std::time::Duration,
    pub work_start: std::time::Instant,
    pub last_update: std::time::Instant,
    pub updated: bool,
    pub workload_id: u8,
    // gui state
    #[cfg(feature = "gui")]
    pub display_settings: bool,
}

impl App {
    pub fn new(
        #[cfg(feature = "gui")] egui_state: Option<(egui::Context, egui::TextureHandle)>,
        render_settings: RenderSettings,
    ) -> Self {
        let (update_recv, work_req) =
            work_handler::create_work_handler(render_settings.num_threads);
        let mut a = Self {
            #[cfg(feature = "gui")]
            egui_state,
            render_settings,
            update_recv,
            work_req,
            canvas: Vec::new(),
            splats_done: 0,
            work_duration: std::time::Duration::ZERO,
            work_start: std::time::Instant::now(),
            last_update: std::time::Instant::now(),
            workload_id: 0,
            work_rays: 0,
            updated: false,
            #[cfg(feature = "gui")]
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

        self.canvas =
            vec![Vec3::ZERO; u32::from(rs.width) as usize * u32::from(rs.height) as usize];
        let (cam, bvh, tris, mats, samplables, envmap) = unsafe {
            (
                CAM.get().as_mut_unchecked(),
                BVH.get().as_mut_unchecked(),
                TRIANGLES.get().as_mut_unchecked(),
                MATERIALS.get().as_mut_unchecked(),
                SAMPLABLE.get().as_mut_unchecked(),
                ENVMAP.get().as_mut_unchecked(),
            )
        };

        if let Some(ref path) = rs.environment_map {
            if let Ok(image) = TextureData::from_path(path) {
                *envmap = EnvMap::Image(image);
                log::info!("Loaded envmap");
            } else {
                log::warn!("Could not import envmap {path}.");
            }
        }

        *cam = unsafe { crate::scene::setup_scene(&rs) };
        *bvh = Bvh::new(tris);

        // calculate samplable objects after BVH rearranges TRIANGLES
        for (i, tri) in tris.iter().enumerate() {
            if let Mat::Light(_) = mats[tri.mat] {
                samplables.push(i);
            }
        }

        let state = State::new(
            u32::from(rs.width) as usize,
            u32::from(rs.height) as usize,
            #[cfg(feature = "gui")]
            self.egui_state.as_ref().map(|v| v.0.clone()),
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
            u32::from(self.render_settings.width) as usize,
            u32::from(self.render_settings.height) as usize,
            #[cfg(feature = "gui")]
            self.egui_state.as_ref().map(|v| v.0.clone()),
            self.render_settings.integrator,
            0,
        );
        self.work_req
            .send(ComputeChange::UpdateState(state))
            .unwrap();
        self.workload_id = self.workload_id.wrapping_add(1);
        self.canvas = vec![
            Vec3::ZERO;
            u32::from(self.render_settings.width) as usize
                * u32::from(self.render_settings.height) as usize
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
