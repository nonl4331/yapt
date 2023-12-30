use crate::{prelude::*, startup::Args};

use std::sync::{mpsc, Arc, Mutex};

#[derive(new)]
pub struct Splat {
    uv: [f32; 2],
    rgb: Vec3,
}

pub struct Film {
    ready_to_use: Arc<Mutex<Vec<Vec<Splat>>>>,
    canvas: Vec<Vec3>,
    receiver: mpsc::Receiver<Vec<Splat>>,
    width: usize,
    height: usize,
}

impl Film {
    pub fn new(receiver: mpsc::Receiver<Vec<Splat>>, args: &Args) -> Self {
        let width = args.width as usize;
        let height = args.height as usize;
        Self {
            ready_to_use: Default::default(),
            canvas: vec![Vec3::ZERO; width * height],
            receiver,
            width,
            height,
        }
    }
    pub fn run(mut self) -> Vec<Vec3> {
        while let Ok(mut splats) = self.receiver.recv() {
            if splats.is_empty() {
                break;
            }

            self.add_splats(&splats);
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

    pub fn child(&self, sender: mpsc::Sender<Vec<Splat>>) -> FilmChild {
        FilmChild {
            ready_to_use: self.ready_to_use.clone(),
            sender,
        }
    }

    fn uv_to_idx(&self, uv: [f32; 2]) -> usize {
        if uv[0] >= 1.0 && uv[1] >= 1.0 {
            println!("{uv:?}");
        }
        let x = (uv[0] * self.width as f32) as usize;
        let y = (uv[1] * self.height as f32) as usize;

        (y * self.width + x).min(self.width * self.height - 1)
    }
}

#[derive(Clone)]
pub struct FilmChild {
    ready_to_use: Arc<Mutex<Vec<Vec<Splat>>>>,
    sender: mpsc::Sender<Vec<Splat>>,
}

impl FilmChild {
    pub fn add_splats(&self, splats: Vec<Splat>) {
        self.sender.send(splats).unwrap();
    }
    pub fn get_vec(&self) -> Vec<Splat> {
        match self.ready_to_use.lock().unwrap().pop() {
            Some(v) => v,
            _ => Vec::new(),
        }
    }
}
