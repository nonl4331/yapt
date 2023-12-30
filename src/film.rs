use crate::{prelude::*, startup::Args};
use indicatif::{ProgressBar, ProgressStyle};

use std::{
    sync::{mpsc, Arc, Mutex},
    time::Instant,
};

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
    receiver: mpsc::Receiver<IntegratorResults>,
    width: usize,
    height: usize,
    stats: FilmStats,
}

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
    pub fn new(receiver: mpsc::Receiver<IntegratorResults>, args: &Args) -> Self {
        let width = args.width as usize;
        let height = args.height as usize;

        Self {
            ready_to_use: Default::default(),
            canvas: vec![Vec3::ZERO; width * height],
            receiver,
            width,
            height,
            stats: FilmStats::new(args),
        }
    }
    pub fn run(mut self) -> Vec<Vec3> {
        while let Ok(results) = self.receiver.recv() {
            let IntegratorResults {
                rays_shot: rays,
                mut splats,
            } = results;

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

    pub fn child(&self, sender: mpsc::Sender<IntegratorResults>) -> FilmChild {
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
}

#[derive(Clone)]
pub struct FilmChild {
    ready_to_use: Arc<Mutex<Vec<Vec<Splat>>>>,
    sender: mpsc::Sender<IntegratorResults>,
}

impl FilmChild {
    pub fn add_results(&self, results: IntegratorResults) {
        self.sender.send(results).unwrap();
    }
    pub fn get_vec(&self) -> Vec<Splat> {
        match self.ready_to_use.lock().unwrap().pop() {
            Some(v) => v,
            _ => Vec::new(),
        }
    }
    pub fn finish_render(&self) {
        self.sender
            .send(IntegratorResults::new(0, Vec::new()))
            .unwrap();
    }
}
