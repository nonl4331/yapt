use clap::Parser;
use fern::colors::{Color, ColoredLevelConfig};
use indicatif::{ProgressBar, ProgressStyle};

use crate::prelude::*;
use crate::{camera::Cam, integrator::*, material::*};
use crate::{HEIGHT, SAMPLES, WIDTH};
use rand::thread_rng;
use rayon::prelude::*;

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
pub struct Args {
    #[arg(short, default_value_t = false)]
    bvh_heatmap: bool,
    #[arg(short, default_value_t = crate::WIDTH)]
    width: usize,
    #[arg(short, default_value_t = crate::HEIGHT)]
    height: usize,
    #[arg(short, default_value_t = crate::SAMPLES)]
    samples: u64,
}

pub fn run() {
    let args = Args::parse();

    create_logger();

    unsafe { crate::scene_init() };

    let cam = crate::get_camera();

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
        generate_heatmap(cam, bvh);
    } else {
        render_image(cam, bvh);
    }
}

fn generate_heatmap(cam: Cam, bvh: Bvh) {
    let buf: Vec<_> = (0..(WIDTH * HEIGHT)).into_par_iter().map(|i| {
        let mut rng = thread_rng();
        let (_, ray) = cam.get_ray(i, &mut rng);
        bvh.traverse_steps(&ray)
    }).collect();

    let max = *buf.iter().max().unwrap() as f32;

    let buf: Vec<_> = buf.into_iter().map(|v| heatmap(v as f32 / max)).collect();
    save_image(buf, 1.0, "heatmap.exr");
}

fn render_image(cam: Cam, bvh: Bvh) {
    let (send, recv) = std::sync::mpsc::channel();
    let film = Film::new(recv);
    let child = film.child(send);

    let bar = ProgressBar::new(SAMPLES).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .unwrap(),
    );

    let film_thread = std::thread::spawn(move || film.run());

    for sample in 1..=SAMPLES {
        let start = std::time::Instant::now();

        let sample_ray_count = (0..(WIDTH * HEIGHT))
            .collect::<Vec<_>>()
            .par_chunks(1024)
            .enumerate()
            .map(|(i, c)| {
                let c = c.len();
                let offset = 1024 * i;
                let mut splats = child.clone().get_vec();
                let mut rng = thread_rng();
                let mut rays = 0;
                for idx in offset..(offset + c) {
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

    save_image(buffer, m, crate::FILENAME);
}

fn save_image(buffer: Vec<Vec3>, m: f32, filename: &'static str) {
    let img = image::Rgb32FImage::from_vec(
        WIDTH as u32,
        HEIGHT as u32,
        buffer
            .iter()
            .flat_map(|v| [v.x * m, v.y * m, v.z * m])
            .collect::<Vec<f32>>(),
    )
    .unwrap();

    img.save(filename).unwrap();
}

fn heatmap(t: f32) -> Vec3 {
    const C0 : Vec3 = Vec3::new(-0.020390,0.009557,0.018508);
    const C1 : Vec3 = Vec3::new(3.108226,-0.106297,-1.105891);
    const C2 : Vec3 = Vec3::new(-14.539061,-2.943057,14.548595);
    const C3 : Vec3 = Vec3::new(71.394557,22.644423,-71.418400);
    const C4 : Vec3 = Vec3::new(-152.022488,-31.024563,152.048692);
    const C5 : Vec3 = Vec3::new(139.593599,12.411251,-139.604042);
    const C6 : Vec3 = Vec3::new(-46.532952,-0.000874,46.532928);
    C0+(C1+(C2+(C3+(C4+(C5+C6*t)*t)*t)*t)*t)*t
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
