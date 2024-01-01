use rand::Rng;
use rand_distr::StandardNormal;

#[derive(Debug)]
pub struct Sample {
    // this is current "RNG" value that the sample holds
    value: f32,
    // this is the "RNG" value for the backup value in
    // case the sample gets rejected
    backup_value: f32,
    // these are the indicies for iteration of which
    // the current "RNG" value was modified as well
    // as the iteration when the backup value was written
    modified_idx: usize,
    backup_idx: usize,
}

// the sample should ALWAYS be overwritten
// before it is read
impl Default for Sample {
    fn default() -> Self {
        Self {
            value: 0.0,
            backup_value: 0.0,
            modified_idx: 0,
            backup_idx: 0,
        }
    }
}

impl Sample {
    // backup the current value this should
    // happen before mutations are applied
    pub fn backup(&mut self) {
        self.backup_value = self.value;
        self.backup_idx = self.modified_idx;
    }
    // overwrite the current value and index from backup
    // this is called when a state vector is rejected which
    // will restore all mutations that happened in that iteration
    pub fn restore(&mut self) {
        self.value = self.backup_value;
        self.modified_idx = self.backup_idx;
    }
}

pub struct PssState<R: Rng> {
    // double check seedable is a trait <- TODO
    // the mutation iteration that we are currently on
    // this only counts sucessful mutations
    iteration: usize,
    // the iteration of which the last large mutation was accepted
    last_large_idx: usize,
    // the state vector itself
    pub state: Vec<Sample>,
    // inner (seeded) PRNG used to generate numbers
    rng: R,
    // boolean to decide if the current mutation is a large one
    is_large_mutation: bool,
    // current index of the next sample within
    // the state vector of the current iteration
    state_idx: usize,
}

impl<R: Rng> PssState<R> {
    const LARGE_PROB: f32 = 0.1;
    const SMALL_STDEV: f32 = 0.3;

    pub fn new(rng: R) -> Self {
        Self {
            iteration: 0,
            last_large_idx: 0,
            state: Vec::new(),
            rng,
            // 0th iteration must be true to
            // prevent uninitialised variables in ensure_ready()
            is_large_mutation: true,
            state_idx: 0,
        }
    }
    // 1:9 ratio of large:small mutation
    pub fn start_iteration(&mut self) {
        self.iteration += 1;
        self.is_large_mutation = self.rng.gen::<f32>() < Self::LARGE_PROB;
        self.state_idx = 0;
    }
    pub fn accept(&mut self) {
        if self.is_large_mutation {
            self.last_large_idx = self.iteration;
        }
    }
    pub fn reject(&mut self) {
        self.iteration -= 1;
        for sample in &mut self.state {
            sample.restore();
        }
    }
    pub fn ensure_ready(&mut self) {
        if self.state_idx >= self.state.len() {
            assert_eq!(self.state_idx, self.state.len());
            self.state.push(Default::default());
        }

        let sample = &mut self.state[self.state_idx];

        // apply large mutation
        if sample.modified_idx < self.last_large_idx {
            sample.value = self.rng.gen();
        }

        // backup and apply small mutations
        sample.backup();
        if self.is_large_mutation {
            sample.value = self.rng.gen();
        } else {
            let small_mutations = self.iteration - self.last_large_idx;
            let eff_std = Self::SMALL_STDEV * (small_mutations as f32).sqrt();
            let nor_sample: f32 = self.rng.sample(StandardNormal);

            sample.value += nor_sample * eff_std;
            sample.value -= sample.value.floor();
        }
        sample.modified_idx = self.iteration;
    }
    pub fn gen_unif(&mut self) -> f32 {
        self.ensure_ready();
        let val = self.state[self.state_idx].value;
        self.state_idx += 1;
        val
    }
}

use std::ops::Range;

pub trait MinRng {
    fn gen(&mut self) -> f32;
    fn gen_range(&mut self, range: Range<f32>) -> f32;
}

impl<R: Rng> MinRng for PssState<R> {
    fn gen(&mut self) -> f32 {
        self.gen_unif()
    }
    fn gen_range(&mut self, range: Range<f32>) -> f32 {
        (range.end - range.start) * self.gen_unif() + range.start
    }
}

impl<R: Rng> MinRng for R {
    fn gen(&mut self) -> f32 {
        self.gen::<f32>()
    }
    fn gen_range(&mut self, range: Range<f32>) -> f32 {
        self.gen_range(range)
    }
}
