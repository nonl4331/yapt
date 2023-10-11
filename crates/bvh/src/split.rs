use crate::Aabb;
use crate::BoundingData;

const NUM_BUCKETS: usize = 12;
const MAX_IN_NODE: usize = 255;

#[macro_export]
macro_rules! partition {
    ($array:expr, $closure:expr) => {{
        let len = $array.len();
        let (mut left, mut right) = (0, len - 1);
        let mid_index: usize;

        loop {
            while left < len && $closure(&$array[left]) {
                left += 1;
            }

            while right > 0 && !($closure(&$array[right])) {
                right -= 1;
            }

            if left >= right {
                mid_index = left;
                break;
            }
            $array.swap(left, right);
        }
        mid_index
    }};
}

#[derive(Copy, Clone)]
pub struct Bucket {
    count: u32,
    bounds: Aabb,
}

impl Bucket {
    pub fn new(bounds: Aabb) -> Self {
        Self { count: 1, bounds }
    }
    pub fn add(&mut self, new: Aabb) {
        self.bounds = Aabb::merge(self.bounds, new);
    }
    pub fn merge(a: Self, b: Self) -> Self {
        Self {
            count: a.count + b.count,
            bounds: Aabb::merge(a.bounds, b.bounds),
        }
    }
}

// SAH
pub fn split(
    bounds: &Aabb,
    centre_bounds: &Aabb,
    axis: usize,
    node_data: &mut [BoundingData],
) -> usize {
    let len = node_data.len();

    if len <= 4 {
        return split_equal(axis, node_data);
    }

    let mut buckets: [Option<Bucket>; NUM_BUCKETS] = [None; NUM_BUCKETS];

    let max_val = centre_bounds.max[axis];
    let min_val = centre_bounds.min[axis];

    let centroid_extent = max_val - min_val;

    // buckets primitives based of centroids
    for prim in &mut *node_data {
        let idx = bucket_idx(axis, prim, min_val, centroid_extent);

        let a = &mut buckets[idx];
        match a {
            Some(bucket) => bucket.add(prim.bounds),
            None => {
                *a = Some(Bucket::new(prim.bounds));
            }
        }
    }

    // determine optimal split of buckets
    let mut costs = [0.0; NUM_BUCKETS - 1];
    for (i, cost) in costs.iter_mut().enumerate().take(NUM_BUCKETS - 1) {
        let (left_bucket, right_bucket) = buckets.split_at(i);

        let left = left_bucket.iter().cloned().flatten().reduce(Bucket::merge);

        let (left_sa, left_count) = if let Some(bucket) = left {
            (bucket.bounds.surface_area(), bucket.count)
        } else {
            (0.0, 0)
        };

        let right = right_bucket.iter().cloned().flatten().reduce(Bucket::merge);

        let (right_sa, right_count) = if let Some(bucket) = right {
            (bucket.bounds.surface_area(), bucket.count)
        } else {
            (0.0, 0)
        };

        *cost = 0.125 * (left_count as f32 * left_sa + right_count as f32 * right_sa)
            / bounds.surface_area();
    }

    let (mc_idx, min_cost) = costs
        .into_iter()
        .enumerate()
        .reduce(|a, b| if a.1 < b.1 { a } else { b })
        .unwrap();

    if len > MAX_IN_NODE || min_cost < len as f32 {
        let closure = |node_data: &BoundingData| -> bool {
            bucket_idx(axis, node_data, min_val, centroid_extent) <= mc_idx
        };
        return partition!(node_data, closure);
    }
    0
}

fn bucket_idx(axis: usize, node_data: &BoundingData, min: f32, extent: f32) -> usize {
    let absolute_value = node_data.centroid[axis];

    let mut b = (NUM_BUCKETS as f32 * (absolute_value - min) / extent) as usize;

    if b == NUM_BUCKETS {
        b -= 1;
    }

    b
}

fn split_equal(axis: usize, node_data: &mut [BoundingData]) -> usize {
    let len = node_data.len();
    node_data[0..len].sort_by(|a, b| utility::float_cmp(a.centroid[axis], b.centroid[axis]));
    len / 2
}
