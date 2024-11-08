use bvh::Bvh;
use rand_pcg::Pcg64Mcg;

use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc,
};

use crate::{Cam, IntegratorType, Naive, Splat, NEEMIS};

const MIN_WORKGROUP_SIZE: u64 = 4096;
const PARK_TIME: std::time::Duration = std::time::Duration::from_millis(20);

// ------------------------------
// Thread Communication
// ------------------------------
pub enum Update {
    // splats, thread_id, rays shot
    Calculation(Vec<Splat>, u64, u64),
    PssmltBootstrapDone,
    WorkQueueCleared,
    NoState,
}

pub enum ComputeChange {
    Shutdown,
    WorkSamples(u64),
    WorkMutations(u64),
    UpdateState(State),
}

pub struct State {
    cam: Cam,
    width: usize,
    height: usize,
    ctx: egui::Context,
    integrator: IntegratorType,
    base_rng_seed: u64,
}

impl State {
    pub fn new(
        cam: Cam,
        width: usize,
        height: usize,
        ctx: egui::Context,
        integrator: IntegratorType,
        base_rng_seed: u64,
    ) -> Self {
        State {
            cam,
            width,
            height,
            ctx,
            integrator,
            base_rng_seed,
        }
    }
}

pub enum Work {
    Pixels(std::ops::Range<u64>),
    Mutations(u64),
}

// ------------------------------
// Creating the work handler
// ------------------------------
pub fn create_work_handler() -> (Receiver<Update>, Sender<ComputeChange>) {
    let (gui_thread_requester, compute_thread_request_handler) = channel::<ComputeChange>();
    let (update_sender, gui_thread_receiver) = channel::<Update>();

    std::thread::spawn(move || {
        let mut state: Option<Arc<State>> = None;

        let work_queue = crossbeam_deque::Worker::<(Work, Arc<State>, u64)>::new_fifo();
        let mut work_id = 0;

        // ------------------------------
        // Spawn compute threads
        // ------------------------------
        let num_threads = num_cpus::get();
        for i in 0..num_threads {
            spawn_compute_thread(i as u64, work_queue.stealer(), update_sender.clone());
        }
        // ------------------------------
        // Change Handling loop
        // ------------------------------
        while let Ok(change) = compute_thread_request_handler.recv() {
            match change {
                ComputeChange::Shutdown => while let Some(_) = work_queue.pop() {},
                ComputeChange::WorkSamples(samples) => {
                    // notify GUI that required state was not provided
                    let Some(ref state) = state else {
                        update_sender.send(Update::NoState).unwrap();
                        continue;
                    };

                    let workgroup_size =
                        MIN_WORKGROUP_SIZE.max(state.width as u64 * state.height as u64 / 256);

                    let mut pixels_start = 0;
                    let end = samples * state.width as u64 * state.height as u64;
                    while pixels_start < end {
                        let pixels_end = (pixels_start + workgroup_size).min(end);
                        work_queue.push((
                            Work::Pixels(pixels_start..pixels_end),
                            state.clone(),
                            work_id,
                        ));

                        pixels_start = pixels_end;
                        work_id += 1;
                    }
                }
                ComputeChange::WorkMutations(_) => todo!(),
                ComputeChange::UpdateState(new_state) => {
                    // clear out work queue before modifying state
                    while let Some(_) = work_queue.pop() {}
                    match state.as_mut() {
                        None => state = Some(Arc::new(new_state)),
                        Some(ref mut old_state) => {
                            *Arc::get_mut(old_state).unwrap() = new_state;
                        }
                    }
                }
            }
        }
    });
    (gui_thread_receiver, gui_thread_requester)
}

// ------------------------------
// Creating a compute thread
// ------------------------------
fn spawn_compute_thread(
    thread_id: u64,
    work_stealer: crossbeam_deque::Stealer<(Work, Arc<State>, u64)>,
    update_sender: Sender<Update>,
) {
    std::thread::spawn(move || {
        loop {
            // ------------------------------
            // Get new work or park
            // ------------------------------
            let (work, state, work_id) = match work_stealer.steal() {
                crossbeam_deque::Steal::Empty => {
                    std::thread::park_timeout(PARK_TIME);
                    continue;
                }
                crossbeam_deque::Steal::Retry => continue,
                crossbeam_deque::Steal::Success(work) => {
                    log::trace!("Thread {thread_id} got work.");
                    work
                }
            };

            let rng = Pcg64Mcg::new((state.base_rng_seed + work_id) as u128);

            let work_result = match work {
                Work::Pixels(pixels) => work_pixels(pixels, rng, state.as_ref(), thread_id),
                Work::Mutations(_) => todo!(),
            };

            log::trace!("Thread {thread_id} finished work.");
            update_sender.send(work_result).unwrap();
            state.ctx.request_repaint();
        }
    });
}

fn work_pixels(
    pixels: std::ops::Range<u64>,
    mut rng: Pcg64Mcg,
    state: &State,
    thread_id: u64,
) -> Update {
    let mut rays = 0;
    let mut splats = Vec::with_capacity((pixels.end - pixels.start) as usize);
    let pn = pixels.end - pixels.start;

    let frame_pixels = (state.width * state.height) as u64;
    for pixel_i in pixels {
        let pixel_i = pixel_i % frame_pixels;
        let (uv, ray) = state.cam.get_ray(pixel_i, &mut rng);
        let (col, ray_count) = match state.integrator {
            IntegratorType::Naive => Naive::rgb(ray, unsafe { &crate::BVH }, &mut rng),
            IntegratorType::NEE => NEEMIS::rgb(ray, unsafe { &crate::BVH }, &mut rng, unsafe {
                &*std::ptr::addr_of!(crate::SAMPLABLE)
            }),
        };
        splats.push(Splat::new(uv, col));
        rays += ray_count;
    }
    Update::Calculation(splats, thread_id, rays)
}

fn work_mutations(mutations: u64, mut rng: Pcg64Mcg, state: &State, thread_id: u64) -> Update {
    /*let mut rays = 0;
    let mut splats = Vec::with_capacity(2 * mutations as usize);


    let mut rng = crate::pssmlt::PssState::new(rng);

    for _ in 0..mutations {

            let (uv, ray) = state.cam.get_random_ray(&mut rng);
            let (col_cir, ray_count) = match state.integrator {
                IntegratorType::Naive => Naive::rgb(ray, &state.bvh, &mut rng),
                IntegratorType::NEE => NEEMIS::rgb(ray, &state.bvh, &mut rng, unsafe {
                    &*std::ptr::addr_of!(crate::SAMPLABLE)
                }),

            };
            rays += ray_count;

    }*/

    todo!()
}
