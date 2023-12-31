use clap::Parser;
use exr::meta::header::ImageAttributes;
use fern::colors::{Color, ColoredLevelConfig};

use crate::prelude::*;
use crate::{camera::Cam, integrator::*, material::*, IntegratorType, Scene};
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
}

pub fn run() {
    let args = Args::parse();

    create_logger();

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
        render_image(cam, bvh, args);
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
    let (send, recv) = std::sync::mpsc::channel();
    let film = Film::new(recv, &args);
    let child = film.child(send);

    let film_thread = std::thread::spawn(move || film.run());

    const CHUNK_SIZE: usize = 4096;
    let pixels = args.width as usize * args.height as usize;

    (0..(pixels * args.samples as usize))
        .into_par_iter()
        .chunks(CHUNK_SIZE)
        .enumerate()
        .for_each(|(i, c)| {
            let c = c.len();
            let offset = CHUNK_SIZE * i;
            let mut splats = child.clone().get_vec();
            let mut rng = Pcg64Mcg::new(i as u128);
            let mut rays = 0;
            for idx in offset..(offset + c) {
                let idx = idx % pixels;
                let (uv, ray) = cam.get_ray(idx as u64, &mut rng);
                let (col, ray_count) = match args.integrator {
                    IntegratorType::Naive => Naive::rgb(ray, &bvh, &mut rng),
                    IntegratorType::NEE => NEEMIS::rgb(ray, &bvh, &mut rng, unsafe { &SAMPLABLE }),
                };
                splats.push(Splat::new(uv, col));
                rays += ray_count;
            }
            let results = IntegratorResults::new(rays, splats);
            child.clone().add_results(results);
        });

    child.finish_render();
    let buffer = film_thread.join().unwrap();
    let m = 1.0 / args.samples as f32;

    save_image(buffer, m, args);
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
        use exr::math;
        use exr::image::write::WritableImage;
        use exr::image::*;
        use exr::meta::header::*;
        let dim = math::Vec2(args.width as usize, args.height as usize);
        let pixel_val = |pos: math::Vec2<usize>| {
            let i = pos.x() + pos.y() * args.width as usize;
            let rgb = buffer[i] * m;
            (rgb.x, rgb.y, rgb.z)
        };

        let mut layer_attributes = LayerAttributes::default();
        layer_attributes.white_luminance = Some(1000.0);
        let layer = Layer::new(dim, layer_attributes, Encoding::FAST_LOSSLESS, SpecificChannels::rgb(pixel_val));
        let mut image = Image::from_layer(layer);
        image.attributes.chromaticities = Some(exr::meta::attribute::Chromaticities { red: math::Vec2(0.708, 0.292), green: math::Vec2(0.170, 0.797), blue: math::Vec2(0.131, 0.0046), white: math::Vec2(0.3127, 0.3290)});
        image.attributes.pixel_aspect = 1.0;
        
        image.write().to_file(args.filename).unwrap();
    }
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
