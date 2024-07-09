use crate::{prelude::*, startup::Args};
use indicatif::{ProgressBar, ProgressStyle};
use minifb::*;
use rayon::prelude::*;

use std::{
    sync::{mpsc, Arc, Mutex},
    time::Instant,
};

pub enum ToFilm {
    Results(IntegratorResults),
    DisplayImage,
    FinishRender,
}

#[derive(new)]
pub struct IntegratorResults {
    rays_shot: u64,
    splats: Vec<Splat>,
}

#[derive(new)]
pub struct Splat {
    uv: [f32; 2],
    rgb: Vec3,
}

pub struct Film {
    ready_to_use: Arc<Mutex<Vec<Vec<Splat>>>>,
    canvas: Vec<Vec3>,
    receiver: mpsc::Receiver<ToFilm>,
    width: usize,
    height: usize,
    stats: FilmStats,
    window: Option<(Window, Vec<u32>)>,
}

#[derive(Debug)]
pub struct FilmStats {
    rays_shot: u64,
    splats_done: u64,
    splats_per_sample: u64,
    bar: ProgressBar,
    start: Instant,
    last_sample: Instant,
    sample_splats: u64,
    sample_rays: u64,
    samples_done: u64,
}

impl FilmStats {
    pub fn new(args: &Args) -> Self {
        let bar = ProgressBar::new(args.samples).with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .unwrap(),
        );
        Self {
            rays_shot: 0,
            splats_done: 0,
            splats_per_sample: args.width as u64 * args.height as u64,
            bar,
            start: Instant::now(),
            last_sample: Instant::now(),
            sample_splats: 0,
            sample_rays: 0,
            samples_done: 0,
        }
    }
    pub fn add_batch(&mut self, rays: u64, splats: usize) {
        self.rays_shot += rays;
        self.sample_rays += rays;
        self.splats_done += splats as u64;
        self.sample_splats += splats as u64;
        if self.sample_splats >= self.splats_per_sample {
            let dur = self.last_sample.elapsed();
            let secs = dur.as_secs_f64();
            let mray_per_second = 1e-6 * self.sample_rays as f64 / secs;
            let msplat_per_second = 1e-6 * self.sample_splats as f64 / secs;
            self.samples_done += 1;
            self.last_sample = Instant::now();
            self.sample_rays = 0;
            self.sample_splats = 0;
            self.bar.set_position(self.samples_done);
            self.bar.set_message(format!(
                "{:.2} MRay/s | {:.2} MSplat/s | ({})",
                mray_per_second,
                msplat_per_second,
                dur.as_millis()
            ));
        }
    }
    pub fn finish(self) {
        self.bar.finish_and_clear();
        let dur = self.start.elapsed();
        let secs = dur.as_secs_f64();
        println!(
            "Time Taken: {}s @ {:.1} ms/sample",
            dur.as_secs(),
            1e3 * secs / self.samples_done as f64
        );
        println!(
            "Rays shot: {} @ {:.2} MRay/s",
            self.rays_shot,
            1e-6 * self.rays_shot as f64 / secs
        );
        println!(
            "Splats done: {} @ {:.2} MSplat/s",
            self.splats_done,
            1e-6 * self.splats_done as f64 / secs
        );
        println!(
            "Average ray depth: {:.2}",
            self.rays_shot as f64 / self.splats_done as f64
        );
    }
}

impl Film {
    pub fn new(args: &Args) -> (std::thread::JoinHandle<Vec<Vec3>>, FilmChild) {
        let (send_child, recv_child) = std::sync::mpsc::channel();

        let width = args.width as usize;
        let height = args.height as usize;
        let gui = args.gui;
        let stats = FilmStats::new(args);

        let thread = std::thread::spawn(move || {
            let (send, recv) = std::sync::mpsc::channel();

            let window = if gui {
                Some({
                    let mut w = Window::new("path tracer", width, height, WindowOptions::default())
                        .unwrap();
                    w.set_target_fps(60);
                    (w, vec![0u32; width * height])
                })
            } else {
                None
            };

            let film = Self {
                ready_to_use: Default::default(),
                canvas: vec![Vec3::ZERO; width * height],
                receiver: recv,
                width,
                height,
                stats,
                window,
            };
            let child = film.child(send);

            send_child.send(child).unwrap();

            film.run()
        });

        (thread, recv_child.recv().unwrap())
    }
    pub fn run(mut self) -> Vec<Vec3> {
        while let Ok(to_film) = self.receiver.recv() {
            let (rays, mut splats) = match to_film {
                ToFilm::Results(IntegratorResults { rays_shot, splats }) => (rays_shot, splats),
                ToFilm::DisplayImage => {
                    self.display_blocking();
                    continue;
                }
                ToFilm::FinishRender => {
                    self.stats.finish();
                    break;
                }
            };

            if splats.is_empty() {
                self.stats.finish();
                break;
            }

            self.add_splats(&splats);
            self.stats.add_batch(rays, splats.len());
            splats.clear();
            self.ready_to_use.lock().unwrap().push(splats);
        }

        self.canvas
    }
    pub fn add_splats(&mut self, splats: &[Splat]) {
        for splat in splats {
            let idx = self.uv_to_idx(splat.uv);
            self.canvas[idx] += splat.rgb;
        }
    }

    pub fn child(&self, sender: mpsc::Sender<ToFilm>) -> FilmChild {
        FilmChild {
            ready_to_use: self.ready_to_use.clone(),
            sender,
        }
    }

    fn uv_to_idx(&self, uv: [f32; 2]) -> usize {
        assert!(uv[0] <= 1.0 && uv[1] <= 1.0);

        let x = (uv[0] * self.width as f32) as usize;
        let y = (uv[1] * self.height as f32) as usize;

        (y * self.width + x).min(self.width * self.height - 1)
    }
    fn display_blocking(&mut self) {
        if let Some((window, buf)) = self.window.as_mut() {
            let mult = ((self.width * self.height) as f64 / self.stats.splats_done as f64) as f32;
            buf.par_iter_mut()
                .zip(self.canvas.par_iter())
                .for_each(|(v, rgb)| {
                    // scale based on samples
                    let rgb = *rgb * mult;

                    // gamma correction
                    let r = rgb.x.powf(1.0 / 2.2);
                    let g = rgb.y.powf(1.0 / 2.2);
                    let b = rgb.z.powf(1.0 / 2.2);

                    // convert to u32
                    let r = ((r * 255.0) as u8) as u32;
                    let g = ((g * 255.0) as u8) as u32;
                    let b = ((b * 255.0) as u8) as u32;

                    *v = r << 16 | g << 8 | b;
                });

            window
                .update_with_buffer(&buf, self.width, self.height)
                .unwrap();
        }
    }
}

#[derive(Clone)]
pub struct FilmChild {
    ready_to_use: Arc<Mutex<Vec<Vec<Splat>>>>,
    sender: mpsc::Sender<ToFilm>,
}

impl FilmChild {
    pub fn add_results(&self, results: IntegratorResults) {
        self.sender.send(ToFilm::Results(results)).unwrap();
    }
    pub fn get_vec(&self) -> Vec<Splat> {
        match self.ready_to_use.lock().unwrap().pop() {
            Some(v) => v,
            _ => Vec::new(),
        }
    }
    pub fn finish_render(&self) {
        self.sender.send(ToFilm::FinishRender).unwrap();
    }
    pub fn display_blocking(&self) {
        self.sender.send(ToFilm::DisplayImage).unwrap();
    }
}
