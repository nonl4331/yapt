use clap::Parser;

use crate::prelude::*;
use rand::thread_rng;
use rand_pcg::Pcg64Mcg;
use rayon::prelude::*;

const CHUNK_SIZE: usize = 4096;

const BOOTSTRAP_CHAINS: usize = 100_000;
const CHAINS: usize = 100;

#[derive(Parser, Clone)]
#[command(about, long_about = None)]
pub struct Args {
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
    #[arg(short, long, default_value_t = IntegratorType::NEE)]
    pub integrator: IntegratorType,
    #[arg(short, long, default_value_t = Scene::One)]
    pub scene: Scene,
    #[arg(short, default_value_t = false)]
    pub pssmlt: bool,
    #[arg(short, long)]
    pub environment_map: Option<String>,
    #[arg(short, long, default_value_t = false)]
    pub gui: bool,
    #[arg(long, default_value_t = 0.0)]
    pub u_low: f32,
    #[arg(long, default_value_t = 1.0)]
    pub u_high: f32,
    #[arg(long, default_value_t = 0.0)]
    pub v_low: f32,
    #[arg(long, default_value_t = 1.0)]
    pub v_high: f32,
}

#[derive(Default)]
pub enum AppState {
    #[default]
    Init,
    Render(Cam, Bvh),
    Rendering(std::thread::JoinHandle<()>),
}

pub struct App {
    fb_tex_handle: egui::TextureHandle,
    args: Args,
    state: AppState,
}

impl App {
    pub fn new(fb_tex_handle: egui::TextureHandle, args: Args) -> Self {
        Self {
            fb_tex_handle,
            args,
            state: AppState::default(),
        }
    }
    pub fn init(&mut self) {
        let AppState::Init = self.state else { return };
        assert!(self.args.u_low >= 0.0);
        assert!(self.args.u_high >= self.args.u_low && self.args.u_low <= 1.0);
        assert!(self.args.v_low >= 0.0);
        assert!(self.args.v_high >= self.args.v_low && self.args.v_low <= 1.0);

        if let Some(ref path) = self.args.environment_map {
            if let Ok(image) = TextureData::from_path(path) {
                unsafe { crate::ENVMAP = EnvMap::Image(image) };
                log::info!("Loaded envmap");
            } else {
                log::warn!("Could not import envmap {path}.");
            }
        }

        let cam = unsafe { crate::setup_scene(&self.args) };

        let bvh = unsafe { Bvh::new(&mut *addr_of_mut!(TRIANGLES)) };

        // calculate samplable objects after BVH rearranges TRIANGLES
        unsafe {
            for (i, tri) in TRIANGLES.iter().enumerate() {
                if let Mat::Light(_) = MATERIALS[tri.mat] {
                    SAMPLABLE.push(i);
                }
            }
        }
        self.state = AppState::Render(cam, bvh);
    }
    pub fn poll_state(&mut self) {
        self.init();
        self.render();
    }
    pub fn render(&mut self) {
        let AppState::Render(..) = self.state else {
            return;
        };
        let mut new_self = Self {
            args: self.args.clone(),
            fb_tex_handle: self.fb_tex_handle.clone(),
            state: AppState::Init,
        };
        std::mem::swap(&mut new_self, self);

        let thread = std::thread::spawn(move || {
            if new_self.args.bvh_heatmap {
                new_self.generate_heatmap();
            } else if new_self.args.pssmlt {
                new_self.render_image_pssmlt();
            } else {
                new_self.render_image();
            }
        });

        self.state = AppState::Rendering(thread);
    }
    pub fn generate_heatmap(&self) {
        let AppState::Render(cam, bvh) = &self.state else {
            return;
        };

        let buf: Vec<_> = (0..(self.args.width * self.args.height))
            .into_par_iter()
            .map(|i| {
                let ray = cam.get_centre_ray(i as u64);
                bvh.traverse_steps(&ray)
            })
            .collect();

        let max = *buf.iter().max().unwrap() as f32;

        let buf: Vec<_> = buf.into_iter().map(|v| heatmap(v as f32 / max)).collect();
        save_image(&buf, 1.0, &self.args);
    }
    fn render_image(&self) {
        let AppState::Render(cam, bvh) = &self.state else {
            return;
        };
        let (film_thread, child) = Film::init(&self.args, self.fb_tex_handle.clone());

        let pixels = self.args.width as usize * self.args.height as usize;

        let random_offset: u64 = rand::Rng::gen(&mut thread_rng());

        for sample_i in 0..self.args.samples {
            (0..pixels)
                .into_par_iter()
                .chunks(CHUNK_SIZE)
                .enumerate()
                .for_each(|(i, c)| {
                    let c = c.len();
                    let offset = CHUNK_SIZE * i;
                    let mut splats = child.clone().get_vec();
                    let mut rng = Pcg64Mcg::new(
                        sample_i as u128 * pixels as u128 + i as u128 + random_offset as u128,
                    );
                    let mut rays = 0;
                    for idx in offset..(offset + c) {
                        let idx = idx % pixels;
                        let (uv, ray) = cam.get_ray(idx as u64, &mut rng);
                        let (col, ray_count) = match self.args.integrator {
                            IntegratorType::Naive => Naive::rgb(ray, bvh, &mut rng),
                            IntegratorType::NEE => {
                                NEEMIS::rgb(ray, bvh, &mut rng, unsafe { &*addr_of!(SAMPLABLE) })
                            }
                        };
                        splats.push(Splat::new(uv, col));
                        rays += ray_count;
                    }
                    let results = IntegratorResults::new(rays, splats);
                    child.clone().add_results(results);
                });
            child.display_blocking();
        }

        child.finish_render();
        let buffer = film_thread.join().unwrap();
        let m = 1.0 / self.args.samples as f32;

        save_image(&buffer, m, &self.args);
    }
    fn render_image_pssmlt(&self) {
        let AppState::Render(cam, bvh) = &self.state else {
            return;
        };
        let mutations_per_pixel = self.args.samples;

        // bootstrap
        let contributions: Vec<_> = (0..BOOTSTRAP_CHAINS)
            .into_par_iter()
            .map(|i| {
                let mut rng = Pcg64Mcg::new(i as u128);
                let (_, ray) = cam.get_random_ray(&mut rng);
                let (rgb, _) = match self.args.integrator {
                    IntegratorType::Naive => Naive::rgb(ray, bvh, &mut rng),
                    IntegratorType::NEE => {
                        NEEMIS::rgb(ray, bvh, &mut rng, unsafe { &*addr_of!(SAMPLABLE) })
                    }
                };
                scalar_contribution(rgb)
            })
            .collect();

        log::info!("bootstrap completed");

        let distr = crate::distributions::Distribution1D::new(&contributions);

        let weight = distr.func_int as f64 / BOOTSTRAP_CHAINS as f64;

        let (film_thread, child) = Film::init(&self.args, self.fb_tex_handle.clone());

        let pixels = self.args.width as usize * self.args.height as usize;
        let total_mutations = pixels * mutations_per_pixel as usize;

        let chunk_size: usize = (total_mutations as f64 / CHAINS as f64) as usize;

        (0..total_mutations)
            .into_par_iter()
            .chunks(chunk_size)
            .for_each(|c| {
                // rng to pick the stating state for this chain
                let mut aux_rng = thread_rng();
                let i = distr.sample(&mut aux_rng);
                let mut pss = crate::pssmlt::PssState::new(Pcg64Mcg::new(i as u128));
                let (mut uv_cur, ray) = cam.get_random_ray(&mut pss);
                let (mut col_cur, ray_count) = match self.args.integrator {
                    IntegratorType::Naive => Naive::rgb(ray, bvh, &mut pss),
                    IntegratorType::NEE => {
                        NEEMIS::rgb(ray, bvh, &mut pss, unsafe { &*addr_of!(SAMPLABLE) })
                    }
                };
                let mut rays = ray_count;
                let mut l_cur = scalar_contribution(col_cur);

                // number of mutations to do for the current chain
                let c = c.len();

                (0..c).collect::<Vec<_>>().chunks(4096).for_each(|sc| {
                    let mut splats = child.clone().get_vec();
                    for _ in 0..sc.len() {
                        pss.start_iteration();
                        let (uv_prop, ray) = cam.get_random_ray(&mut pss);
                        let (col_prop, ray_count) = match self.args.integrator {
                            IntegratorType::Naive => Naive::rgb(ray, bvh, &mut pss),
                            IntegratorType::NEE => {
                                NEEMIS::rgb(ray, bvh, &mut pss, unsafe { &*addr_of!(SAMPLABLE) })
                            }
                        };
                        let l_prop = scalar_contribution(col_prop);
                        rays += ray_count;

                        let accept = (l_prop / l_cur).min(1.0);
                        splats.push(Splat::new(uv_prop, col_prop * accept / l_prop));
                        splats.push(Splat::new(uv_cur, col_cur * (1.0 - accept) / l_cur));

                        if aux_rng.gen() < accept {
                            uv_cur = uv_prop;
                            l_cur = l_prop;
                            col_cur = col_prop;
                            pss.accept();
                        } else {
                            pss.reject();
                        }
                    }
                    let results = IntegratorResults::new(rays, splats);
                    rays = 0;
                    child.clone().add_results(results);
                });
            });

        child.finish_render();
        let buffer = film_thread.join().unwrap();
        let m = weight / mutations_per_pixel as f64;

        save_image(&buffer, m as f32, &self.args);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("yapt");
            let size = self.fb_tex_handle.size_vec2();
            let sized_tex = egui::load::SizedTexture::new(&self.fb_tex_handle, size);
            ui.add(egui::Image::new(sized_tex).fit_to_exact_size(size));
            self.poll_state();
        });
    }
}

pub fn run() {
    create_logger();

    let args = Args::parse();

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

            let app = App::new(fb_handle, args.clone());

            Ok(Box::new(app))
        }),
    )
    .unwrap();
}

fn save_image(buffer: &[Vec3], m: f32, args: &Args) {
    if args.bvh_heatmap {
        let img = image::Rgb32FImage::from_vec(
            args.width,
            args.height,
            buffer
                .iter()
                .flat_map(|v| [v.x * m, v.y * m, v.z * m])
                .collect::<Vec<f32>>(),
        )
        .unwrap();

        img.save(args.filename.clone()).unwrap();
    } else {
        use exr::image::write::WritableImage;
        use exr::image::{Encoding, Image, Layer, SpecificChannels};
        use exr::math;
        use exr::meta::header::LayerAttributes;
        let dim = math::Vec2(args.width as usize, args.height as usize);
        let pixel_val = |pos: math::Vec2<usize>| {
            let i = pos.x() + pos.y() * args.width as usize;
            let rgb = buffer[i] * m;
            (rgb.x, rgb.y, rgb.z)
        };

        let layer_attributes = LayerAttributes {
            white_luminance: Some(1000.0),
            ..Default::default()
        };
        let layer = Layer::new(
            dim,
            layer_attributes,
            Encoding::FAST_LOSSLESS,
            SpecificChannels::rgb(pixel_val),
        );
        let mut image = Image::from_layer(layer);
        image.attributes.chromaticities = Some(exr::meta::attribute::Chromaticities {
            red: math::Vec2(0.708, 0.292),
            green: math::Vec2(0.170, 0.797),
            blue: math::Vec2(0.131, 0.0046),
            white: math::Vec2(0.3127, 0.3290),
        });
        image.attributes.pixel_aspect = 1.0;

        image.write().to_file(args.filename.clone()).unwrap();
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
