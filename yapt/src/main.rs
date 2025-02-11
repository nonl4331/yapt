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
pub mod overrides;
pub mod pssmlt;
pub mod texture;
pub mod triangle;
pub mod work_handler;

pub mod prelude {
    pub use crate::{
        camera::Cam, coord::*, envmap::*, feature_enabled, integrator::*, loader, material::*,
        pssmlt::MinRng, texture::*, triangle::Tri, work_handler::*, IntegratorType, Intersection,
        Splat, BVH, CAM, DISABLE_SHADING_NORMALS, ENVMAP, HEIGHT, MATERIALS, MATERIAL_NAMES,
        NORMALS, SAMPLABLE, TEXTURES, TEXTURE_NAMES, TRIANGLES, UVS, VERTICES, WIDTH,
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
use std::{
    num::{NonZeroU32, NonZeroUsize},
    process::exit,
    sync::Mutex,
};

use overrides::Overrides;
use prelude::*;

use clap::Parser;

const _CHUNK_SIZE: usize = 4096;

const _BOOTSTRAP_CHAINS: usize = 100_000;
const _CHAINS: usize = 100;

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

pub static OPTIONS: SyncUnsafeCell<u64> = SyncUnsafeCell::new(0);
pub const DISABLE_SHADING_NORMALS: u64 = 1;

pub fn feature_enabled(option: u64) -> bool {
    unsafe { OPTIONS.get().as_ref_unchecked() & option == option }
}
pub fn enable_feature(option: u64) {
    unsafe {
        *OPTIONS.get().as_mut_unchecked() |= option;
    }
}
pub fn disable_feature(option: u64) {
    unsafe {
        *OPTIONS.get().as_mut_unchecked() &= !option;
    }
}

#[derive(clap::ValueEnum, Debug, Copy, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, new, Clone)]
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

    let mut args2 = InputParameters::parse();

    let overrides = dbg!(overrides::load_overrides_file(
        args2.scene.clone(),
        &mut args2
    ));
    let rs: MainRenderSettings = args2.into();

    // GUI mode
    #[cfg(feature = "gui")]
    if !rs.headless {
        eframe::run_native(
            "yapt",
            eframe::NativeOptions::default(),
            Box::new(|cc| {
                // framebuffer for displying render to the screen
                let fb_handle = cc.egui_ctx.load_texture(
                    "fb",
                    egui::ImageData::Color(std::sync::Arc::new(egui::ColorImage::new(
                        [rs.width as usize, rs.height as usize],
                        egui::Color32::BLACK,
                    ))),
                    egui::TextureOptions::default(),
                );

                let app = App::new(
                    Some((cc.egui_ctx.clone(), fb_handle)),
                    rs.clone(),
                    overrides,
                );

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
        rs.clone(),
        overrides,
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

#[derive(Parser, Clone, Debug, Default, PartialEq)]
#[command(about, long_about = None, disable_help_flag = true)]
pub struct InputParameters {
    #[arg(short)]
    bvh_heatmap: Option<bool>,
    #[arg(short, long)]
    width: Option<NonZeroU32>,
    #[arg(short, long)]
    height: Option<NonZeroU32>,
    #[arg(short = 'n', long)]
    samples: Option<u64>,
    #[arg(short, long, default_value_t=String::new())]
    glb_filepath: String, // empty == None
    #[arg(short, long, default_value_t=String::new())]
    output_filename: String,
    #[arg(short, long)]
    integrator: Option<IntegratorType>,
    #[arg(short, long, default_value_t=String::new())]
    environment_map: String,
    #[arg(long)]
    u_low: Option<f32>,
    #[arg(long)]
    u_high: Option<f32>,
    #[arg(long)]
    v_low: Option<f32>,
    #[arg(long)]
    v_high: Option<f32>,
    #[arg(long)]
    num_threads: Option<usize>,
    #[arg(short, long, default_value_t=String::new())]
    camera: String,
    #[arg(short, default_value_t=String::new())]
    file_hash: String,
    #[arg(short)]
    headless: Option<bool>,
    #[arg(short)]
    pssmlt: Option<bool>,
    #[arg(short)]
    disable_shading_normals: Option<bool>,
    #[arg(short, long, default_value_t=String::new())]
    scene: String,
    #[arg(long, action = clap::ArgAction::HelpLong)]
    pub help: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MainRenderSettings {
    bvh_heatmap: bool,
    width: u32,
    height: u32,
    samples: u64,
    glb_filepath: String,
    output_filename: String,
    integrator: IntegratorType,
    environment_map: String,
    u: Vec2,
    v: Vec2,
    num_threads: usize,
    camera: String,
    headless: bool,
    pssmlt: bool,
    disable_shading_normals: bool,
}

impl From<InputParameters> for MainRenderSettings {
    fn from(r: InputParameters) -> Self {
        // check glb_filepath exists
        if r.glb_filepath.is_empty() {
            log::error!("No GLB/GLTF filepath specified.");
            exit(0);
        }

        if let Err(e) = std::fs::File::open(&r.glb_filepath) {
            log::error!("Cannot open glb/gltf @ {}: {e}", r.glb_filepath);
            exit(0);
        }

        if r.output_filename.is_empty() {
            log::warn!("No output file specified, not saving when render finishes");
        }

        let width = r.width.map(|v| v.into()).unwrap_or_else(|| {
            log::info!("No width specified defaulting to 1024");
            1024
        });

        let height = r.height.map(|v| v.into()).unwrap_or_else(|| {
            log::info!("No height specified defaulting to 1024");
            1024
        });

        let samples = r
            .samples
            .map(|v| if v == 0 { u64::MAX } else { v })
            .unwrap_or(100);

        let bvh_heatmap = r.bvh_heatmap.unwrap_or(false);

        let integrator = r.integrator.unwrap_or(IntegratorType::NEE);

        let validate_bounds = |low: Option<f32>, high: Option<f32>, name| -> Vec2 {
            let low = low.unwrap_or(0.0);
            let high = high.unwrap_or(1.0);
            if low > high {
                log::error!("{name}.low > {name}.high");
                exit(0);
            }
            if low < 0.0 {
                log::error!("{name}.low < 0.0");
                exit(0);
            }
            if high > 1.0 {
                log::error!("{name}.high > 1.0");
                exit(0);
            }
            Vec2::new(low, high)
        };

        let u = validate_bounds(r.u_low, r.u_high, 'u');
        let v = validate_bounds(r.v_low, r.v_high, 'v');

        let mut num_threads = r.num_threads.unwrap_or(0);
        if num_threads == 0 {
            num_threads = num_cpus::get();
            log::info!("Using {num_threads} threads.");
        }

        let headless = r.headless.unwrap_or(false);

        let pssmlt = r.pssmlt.unwrap_or(false);

        let disable_shading_normals = r.disable_shading_normals.unwrap_or(false);

        Self {
            bvh_heatmap,
            width,
            height,
            samples,
            glb_filepath: r.glb_filepath,
            output_filename: r.output_filename,
            integrator,
            environment_map: r.environment_map,
            u,
            v,
            num_threads,
            camera: r.camera,
            headless,
            pssmlt,
            disable_shading_normals,
        }
    }
}

pub struct App {
    pub render_settings: MainRenderSettings,
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
        render_settings: MainRenderSettings,
        overrides: Overrides,
    ) -> Self {
        let (update_recv, work_req) = work_handler::create_work_handler(
            NonZeroUsize::new(render_settings.num_threads).unwrap(),
        );

        // apply options
        if render_settings.disable_shading_normals {
            enable_feature(DISABLE_SHADING_NORMALS);
        }

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

        a.init(overrides);
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
    fn init(&mut self, overrides: Overrides) {
        let rs = &mut self.render_settings;

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

        if !rs.environment_map.is_empty() {
            if let Ok(image) = TextureData::from_path(&rs.environment_map) {
                *envmap = EnvMap::Image(image);
                log::info!("Loaded envmap");
            } else {
                log::warn!("Could not import envmap {}.", rs.environment_map);
            }
        }

        // setup scene
        unsafe {
            let cams = loader::load_gltf(&rs.glb_filepath, rs, &overrides);
            // TODO: proper camera management
            *cam = cams[0].clone();
        }

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
fn _scalar_contribution(rgb: Vec3) -> f32 {
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
