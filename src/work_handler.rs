use rand_pcg::Pcg64Mcg;

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    usize,
};

use crate::{IntegratorType, Naive, Splat, NEEMIS};

const MIN_WORKGROUP_SIZE: u64 = 4096;
const PARK_TIME: std::time::Duration = std::time::Duration::from_millis(20);

// ------------------------------
// Thread Communication
// ------------------------------
pub enum Update {
    // splats, workload_id, rays shot
    Calculation(Vec<Splat>, u8, u64),
    PssmltBootstrapDone,
    WorkQueueCleared,
    NoState,
}

pub enum ComputeChange {
    Shutdown,
    // samples, workload_id
    WorkSamples(u64, u8),
    WorkMutations(u64),
    UpdateState(State),
}

pub struct State {
    width: usize,
    height: usize,
    ctx: egui::Context,
    integrator: IntegratorType,
    base_rng_seed: u64,
}

impl State {
    pub fn new(
        width: usize,
        height: usize,
        ctx: egui::Context,
        integrator: IntegratorType,
        base_rng_seed: u64,
    ) -> Self {
        State {
            width,
            height,
            ctx,
            integrator,
            base_rng_seed,
        }
    }
}

#[derive(Clone)]
pub enum WorkLoad {
    Pixels(std::ops::Range<u64>),
    Mutations(u64),
}

struct WorkQueue {
    queue: VecDeque<(WorkLoad, Arc<State>, u64, u8)>,
    read: AtomicUsize,
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self {
            queue: VecDeque::default(),
            read: AtomicUsize::new(usize::MAX),
        }
    }
}

enum FetchState {
    Work(WorkLoad, Arc<State>, u64, u8),
    Busy,
    Empty,
}

impl WorkQueue {
    const QUEUE_EMPTY: usize = usize::MAX;
    const QUEUE_BUSY: usize = usize::MAX - 1;
    pub unsafe fn add_work(
        queue: &mut Arc<Self>,
        mut new_work: VecDeque<(WorkLoad, Arc<State>, u64, u8)>,
    ) {
        let s = unsafe { Arc::<WorkQueue>::get_mut_unchecked(queue) };
        let old_index = s.read.swap(Self::QUEUE_BUSY, Ordering::SeqCst);
        s.queue.append(&mut new_work);

        // trim old data
        if old_index < s.queue.len() {
            s.queue = s.queue.split_off(old_index);
        }
        if s.queue.is_empty() {
            s.read.store(Self::QUEUE_EMPTY, Ordering::SeqCst);
        } else {
            s.read.store(0, Ordering::SeqCst);
        }
    }
    pub unsafe fn clear(queue: &mut Arc<Self>) {
        let s = unsafe { Arc::<WorkQueue>::get_mut_unchecked(queue) };
        s.read.store(Self::QUEUE_EMPTY, Ordering::SeqCst);
        s.queue.clear();
    }
    pub fn get_work(queue: &Arc<Self>) -> FetchState {
        // note this will either:
        // leave both Self::QUEUE_EMPTY & Self::QUEUE_BUSY unmodified
        // OR
        // set the current index to Self::QUEUE_BUSY
        let current_index = queue.read.fetch_max(Self::QUEUE_BUSY, Ordering::SeqCst);

        if current_index == Self::QUEUE_BUSY {
            return FetchState::Busy;
        } else if current_index == Self::QUEUE_EMPTY {
            return FetchState::Empty;
        }
        let val = queue.queue[current_index].clone();
        if current_index + 1 != queue.queue.len() {
            queue.read.store(current_index + 1, Ordering::SeqCst);
        } else {
            queue.read.store(Self::QUEUE_EMPTY, Ordering::SeqCst);
        }
        FetchState::Work(val.0, val.1, val.2, val.3)
    }
}
// ------------------------------
// Creating the work handler
// ------------------------------
pub fn create_work_handler() -> (Receiver<Update>, Sender<ComputeChange>) {
    let (gui_thread_requester, compute_thread_request_handler) = channel::<ComputeChange>();
    let (update_sender, gui_thread_receiver) = channel::<Update>();

    std::thread::spawn(move || {
        let mut state: Option<Arc<State>> = None;

        let mut work_queue = Arc::new(WorkQueue::default());
        // for seeding RNG, changes with each work batch
        let mut work_id = 0;

        // ------------------------------
        // Spawn compute threads
        // ------------------------------
        let num_threads = num_cpus::get();
        log::trace!("Spawned {num_threads} compute threads.");
        for i in 0..num_threads {
            spawn_compute_thread(i as u64, work_queue.clone(), update_sender.clone());
        }
        // ------------------------------
        // Change Handling loop
        // ------------------------------
        while let Ok(change) = compute_thread_request_handler.recv() {
            match change {
                ComputeChange::Shutdown => {
                    unsafe { WorkQueue::clear(&mut work_queue) };
                }
                ComputeChange::WorkSamples(samples, workload_id) => {
                    // notify GUI that required state was not provided
                    let Some(ref state) = state else {
                        update_sender.send(Update::NoState).unwrap();
                        continue;
                    };

                    let workgroup_size =
                        MIN_WORKGROUP_SIZE.max(state.width as u64 * state.height as u64 / 256);

                    let mut pixels_start = 0;
                    let end = samples * state.width as u64 * state.height as u64;
                    let mut deque = VecDeque::new();
                    while pixels_start < end {
                        let pixels_end = (pixels_start + workgroup_size).min(end);
                        deque.push_back((
                            WorkLoad::Pixels(pixels_start..pixels_end),
                            state.clone(),
                            work_id,
                            workload_id,
                        ));

                        pixels_start = pixels_end;
                        work_id += 1;
                    }
                    unsafe { WorkQueue::add_work(&mut work_queue, deque) };
                }
                ComputeChange::WorkMutations(_) => todo!(),
                ComputeChange::UpdateState(new_state) => {
                    // clear out work queue before modifying state
                    unsafe { WorkQueue::clear(&mut work_queue) };
                    match state.as_mut() {
                        None => state = Some(Arc::new(new_state)),
                        Some(ref mut old_state) => {
                            // work is ignore from threads that are currently running
                            // i.e. threads that currently hold an Arc<State>
                            unsafe {
                                *Arc::get_mut_unchecked(old_state) = new_state;
                            }
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
    work_stealer: Arc<WorkQueue>,
    update_sender: Sender<Update>,
) {
    std::thread::spawn(move || {
        loop {
            // ------------------------------
            // Get new work or park
            // ------------------------------
            let (work, state, work_id, workload_id) = match WorkQueue::get_work(&work_stealer) {
                FetchState::Work(work, state, work_id, workload_id) => {
                    log::trace!(
                        "Thread {thread_id} got work {work_id} as part of workload {workload_id}.",
                    );
                    (work, state, work_id, workload_id)
                }
                FetchState::Empty => {
                    std::thread::sleep(PARK_TIME);
                    continue;
                }
                FetchState::Busy => continue,
            };

            let rng = Pcg64Mcg::new((state.base_rng_seed + work_id) as u128);

            let work_result = match work {
                WorkLoad::Pixels(pixels) => work_pixels(pixels, rng, state.as_ref(), workload_id),
                WorkLoad::Mutations(_) => todo!(),
            };

            log::trace!(
                "Thread {thread_id} finished work {work_id} as part of workload {workload_id}."
            );
            update_sender.send(work_result).unwrap();
            state.ctx.request_repaint();
        }
    });
}

fn work_pixels(
    pixels: std::ops::Range<u64>,
    mut rng: Pcg64Mcg,
    state: &State,
    workload_id: u8,
) -> Update {
    let mut rays = 0;
    let mut splats = Vec::with_capacity((pixels.end - pixels.start) as usize);

    let frame_pixels = (state.width * state.height) as u64;
    for pixel_i in pixels {
        let pixel_i = pixel_i % frame_pixels;
        let (uv, ray) = unsafe { crate::CAM.get_ray(pixel_i, &mut rng) };
        let (col, ray_count) = match state.integrator {
            IntegratorType::Naive => Naive::rgb(ray, &mut rng),
            IntegratorType::NEE => NEEMIS::rgb(ray, &mut rng, unsafe {
                &*std::ptr::addr_of!(crate::SAMPLABLE)
            }),
        };
        splats.push(Splat::new(uv, col));
        rays += ray_count;
    }
    Update::Calculation(splats, workload_id, rays)
}

fn work_mutations(_mutations: u64, _rng: Pcg64Mcg, _state: &State, _workload_id: u64) -> Update {
    todo!()
}
