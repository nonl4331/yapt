use clap::Parser;

use crate::envmap::TextureData;
use crate::prelude::*;
use crate::{camera::Cam, integrator::*, material::*, IntegratorType, Scene};
use rand::thread_rng;
use rand_pcg::Pcg64Mcg;
use rayon::prelude::*;

#[derive(Parser)]
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
}

pub fn run() {
    let args = Args::parse();

    create_logger();

    if let Some(ref path) = args.environment_map {
        if let Ok(image) = TextureData::from_path(path) {
            unsafe { crate::ENVMAP = EnvMap::Image(image) };
            log::info!("Loaded envmap");
        } else {
            log::warn!("Could not import envmap {path}.");
        }
    }

    let cam = unsafe { crate::setup_scene(&args) };

    let bvh = unsafe { Bvh::new(&mut TRIANGLES) };

    // calculate samplable objects after BVH rearranges TRIANGLES
    unsafe {
        for (i, tri) in TRIANGLES.iter().enumerate() {
            if let Mat::Light(_) = MATERIALS[tri.mat] {
                SAMPLABLE.push(i);
            }
        }
    }

    if args.bvh_heatmap {
        generate_heatmap(cam, bvh, args);
    } else {
        if args.pssmlt {
            render_image_pssmlt(cam, bvh, args)
        } else {
            render_image(cam, bvh, args);
        }
    }
}

fn generate_heatmap(cam: Cam, bvh: Bvh, args: Args) {
    let buf: Vec<_> = (0..(args.width * args.height))
        .into_par_iter()
        .map(|i| {
            let ray = cam.get_centre_ray(i as u64);
            bvh.traverse_steps(&ray)
        })
        .collect();

    let max = *buf.iter().max().unwrap() as f32;

    let buf: Vec<_> = buf.into_iter().map(|v| heatmap(v as f32 / max)).collect();
    save_image(buf, 1.0, args);
}

fn render_image(cam: Cam, bvh: Bvh, args: Args) {
    let (film_thread, child) = Film::new(&args);

    const CHUNK_SIZE: usize = 4096;
    let pixels = args.width as usize * args.height as usize;

    for sample_i in 0..args.samples {
        (0..pixels)
            .into_par_iter()
            .chunks(CHUNK_SIZE)
            .enumerate()
            .for_each(|(i, c)| {
                let c = c.len();
                let offset = CHUNK_SIZE * i;
                let mut splats = child.clone().get_vec();
                let mut rng = Pcg64Mcg::new(sample_i as u128 * pixels as u128 + i as u128);
                let mut rays = 0;
                for idx in offset..(offset + c) {
                    let idx = idx % pixels;
                    let (uv, ray) = cam.get_ray(idx as u64, &mut rng);
                    let (col, ray_count) = match args.integrator {
                        IntegratorType::Naive => Naive::rgb(ray, &bvh, &mut rng),
                        IntegratorType::NEE => {
                            NEEMIS::rgb(ray, &bvh, &mut rng, unsafe { &SAMPLABLE })
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
    let m = 1.0 / args.samples as f32;

    save_image(buffer, m, args);
}

fn render_image_pssmlt(cam: Cam, bvh: Bvh, args: Args) {
    let mutations_per_pixel = args.samples;

    // bootstrap
    const BOOTSTRAP_CHAINS: usize = 100_000;
    let contributions: Vec<_> = (0..BOOTSTRAP_CHAINS)
        .into_par_iter()
        .map(|i| {
            let mut rng = Pcg64Mcg::new(i as u128);
            let (_, ray) = cam.get_random_ray(&mut rng);
            let (rgb, _) = match args.integrator {
                IntegratorType::Naive => Naive::rgb(ray, &bvh, &mut rng),
                IntegratorType::NEE => NEEMIS::rgb(ray, &bvh, &mut rng, unsafe { &SAMPLABLE }),
            };
            scalar_contribution(rgb)
        })
        .collect();

    log::info!("bootstrap completed");

    let distr = crate::distributions::Distribution1D::new(&contributions);

    let weight = distr.func_int as f64 / BOOTSTRAP_CHAINS as f64;

    let (film_thread, child) = Film::new(&args);

    let pixels = args.width as usize * args.height as usize;
    let total_mutations = pixels * mutations_per_pixel as usize;

    const CHAINS: usize = 100;
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
            let (mut col_cur, ray_count) = match args.integrator {
                IntegratorType::Naive => Naive::rgb(ray, &bvh, &mut pss),
                IntegratorType::NEE => NEEMIS::rgb(ray, &bvh, &mut pss, unsafe { &SAMPLABLE }),
            };
            let mut rays = ray_count;
            let mut l_cur = scalar_contribution(col_cur);

            // number of mutations to do for the current chain
            let c = c.len();

            (0..c)
                .into_iter()
                .collect::<Vec<_>>()
                .chunks(4096)
                .for_each(|sc| {
                    let mut splats = child.clone().get_vec();
                    for _ in 0..sc.len() {
                        pss.start_iteration();
                        let (uv_prop, ray) = cam.get_random_ray(&mut pss);
                        let (col_prop, ray_count) = match args.integrator {
                            IntegratorType::Naive => Naive::rgb(ray, &bvh, &mut pss),
                            IntegratorType::NEE => {
                                NEEMIS::rgb(ray, &bvh, &mut pss, unsafe { &SAMPLABLE })
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

    save_image(buffer, m as f32, args);
}

fn save_image(buffer: Vec<Vec3>, m: f32, args: Args) {
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

        img.save(args.filename).unwrap();
    } else {
        use exr::image::write::WritableImage;
        use exr::image::*;
        use exr::math;
        use exr::meta::header::*;
        let dim = math::Vec2(args.width as usize, args.height as usize);
        let pixel_val = |pos: math::Vec2<usize>| {
            let i = pos.x() + pos.y() * args.width as usize;
            let rgb = buffer[i] * m;
            (rgb.x, rgb.y, rgb.z)
        };

        let mut layer_attributes = LayerAttributes::default();
        layer_attributes.white_luminance = Some(1000.0);
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

        image.write().to_file(args.filename).unwrap();
    }
}

// REC.2020 -> XYZ.Y (not entirely sure if this is correct)
fn scalar_contribution(rgb: Vec3) -> f32 {
    (0.144616903586208 * rgb.x + 0.677998071518871 * rgb.y + 0.0280726930490874 * rgb.z).max(0.0001)
    // max is to avoid NAN
}

fn heatmap(t: f32) -> Vec3 {
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
