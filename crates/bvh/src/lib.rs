use crate::aabb::Aabb;
use crate::aabb::Aabound;
use crate::split::split;
use core::ops::Range;
use std::collections::VecDeque;
use utility::Ray;
use utility::Vec3;

pub mod aabb;
mod split;

use std::f32::EPSILON;

pub struct BoundingData {
    idx: usize,
    bounds: Aabb,
    centroid: Vec3,
}

impl BoundingData {
    pub fn from_prim<T: Aabound>(prim: &T, idx: usize) -> Self {
        let bounds = prim.aabb();
        Self {
            idx,
            bounds,
            centroid: bounds.centroid(),
        }
    }
}

#[derive(Debug)]
pub struct Bvh {
    pub nodes: Vec<Node>,
}

impl Bvh {
    pub fn new<T: Aabound>(primitives: &mut [T]) -> Self {
        let mut bvh = Self { nodes: Vec::new() };

        // generate bounding data & index data for primitives
        let mut prim_data = primitives
            .iter()
            .enumerate()
            .map(|(i, p)| BoundingData::from_prim(p, i))
            .collect::<Vec<_>>();

        let mut order = Vec::new();

        bvh.construct_node(0, &mut order, &mut prim_data);

        utility::sort_by_indices(primitives, order);

        bvh
    }

    pub fn construct_node(
        &mut self,
        idx: usize,
        prim_order: &mut Vec<usize>,
        prim_data: &mut [BoundingData],
    ) -> usize {
        let num_prim = prim_data.len();

        let bounds = prim_data
            .iter()
            .map(|v| v.bounds)
            .reduce(Aabb::merge)
            .unwrap();

        let node_idx = self.nodes.len();

        // create parent node without children
        self.nodes.push(Node::new(bounds, idx, num_prim));

        if num_prim == 1 {
            prim_order.push(prim_data[0].idx);
        } else {
            let centroid_bounds = prim_data
                .iter()
                .map(|v| Aabb::new(v.centroid, v.centroid))
                .reduce(Aabb::merge)
                .unwrap();

            // use the axis with the maximum extend to split with
            let max_axis = utility::max_axis(&centroid_bounds.extent());

            if centroid_bounds.max[max_axis] - centroid_bounds.min[max_axis] < 100.0 * EPSILON {
                // the maximum axis is small enough that it's not worth splitting
                for idx in prim_data.iter().map(|v| v.idx) {
                    prim_order.push(idx)
                }
            } else {
                let mid_idx = split(&bounds, &centroid_bounds, max_axis, prim_data);

                if mid_idx != 0 {
                    // split and recursively construct child nodes
                    let (left, right) = prim_data.split_at_mut(mid_idx);

                    self.nodes[node_idx].left = self.construct_node(idx, prim_order, left);
                    self.nodes[node_idx].right =
                        self.construct_node(idx + left.len(), prim_order, right);
                } else {
                    // SAH indicates that the split isn't worth it
                    for idx in prim_data.iter().map(|v| v.idx) {
                        prim_order.push(idx)
                    }
                }
            }
        }

        node_idx
    }

    pub fn traverse(&self, ray: &Ray) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();

        let mut node_stack = VecDeque::from([0]);
        while !node_stack.is_empty() {
            let idx = node_stack.pop_front().unwrap();

            let node = &self.nodes[idx];

            if !node.bounds.does_int(ray) {
                continue;
            }

            // both children are valid or neither are
            if node.left != 0 {
                node_stack.push_back(node.left);
                node_stack.push_back(node.right);
            } else {
                ranges.push(node.prim_idx..(node.prim_idx + node.num_prim))
            }
        }
        ranges
    }
    pub fn traverse_steps(&self, ray: &Ray) -> usize {
        let mut steps = 0;

        let mut node_stack = VecDeque::from([0]);
        while !node_stack.is_empty() {
            let idx = node_stack.pop_front().unwrap();

            let node = &self.nodes[idx];

            if !node.bounds.does_int(ray) {
                continue;
            }

            // both children are valid or neither are
            if node.left != 0 {
                steps += 1;
                node_stack.push_back(node.left);
                node_stack.push_back(node.right);
            }
        }
        steps
    }
}

#[derive(Debug)]
pub struct Node {
    bounds: Aabb,
    left: usize,
    right: usize,
    prim_idx: usize,
    num_prim: usize,
}

impl Node {
    pub fn new(bounds: Aabb, prim_idx: usize, num_prim: usize) -> Self {
        Self {
            bounds,
            left: 0,
            right: 0,
            prim_idx,
            num_prim,
        }
    }
}
