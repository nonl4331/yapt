use crate::prelude::*;
use bvh::aabb::{Aabb, Aabound};

#[derive(Debug, new, PartialEq)]
pub struct Tri {
    pos: [usize; 3],
    nor: [usize; 3],
    uv: [usize; 3],
    pub mat: usize,
}

impl Aabound for Tri {
    fn aabb(&self) -> Aabb {
        let verts = unsafe { VERTICES.get().as_ref_unchecked() };
        let a = verts[self.pos[0]];
        let b = verts[self.pos[1]];
        let c = verts[self.pos[2]];

        let min_x = a.x.min(b.x).min(c.x);
        let min_y = a.y.min(b.y).min(c.y);
        let min_z = a.z.min(b.z).min(c.z);

        let max_x = a.x.max(b.x).max(c.x);
        let max_y = a.y.max(b.y).max(c.y);
        let max_z = a.z.max(b.z).max(c.z);

        let mut min = Vec3::new(min_x, min_y, min_z);
        let mut max = Vec3::new(max_x, max_y, max_z);
        let diff = max - min;
        if diff.x == 0.0 {
            max.x += 1e-5;
            min.x -= 1e-5;
        }
        if diff.y == 0.0 {
            max.y += 1e-5;
            min.y -= 1e-5;
        }
        if diff.z == 0.0 {
            max.z += 1e-5;
            min.z -= 1e-5;
        }

        max += 1e-5 * diff;
        min -= 1e-5 * diff;

        Aabb::new(min, max)
    }
}

impl Tri {
    // see whoop 2013 https://jcgt.org/published/0002/01/05/paper.pdf
    #[must_use]
    pub fn intersect(&self, ray: &Ray, rng: &mut impl MinRng) -> Intersection {
        let verts = unsafe { VERTICES.get().as_ref_unchecked() };
        let norms = unsafe { NORMALS.get().as_ref_unchecked() };
        let mats = unsafe { MATERIALS.get().as_ref_unchecked() };
        let uvs = unsafe { UVS.get().as_ref_unchecked() };

        let v0 = verts[self.pos[0]];
        let v1 = verts[self.pos[1]];
        let v2 = verts[self.pos[2]];
        let n0 = norms[self.nor[0]];
        let n1 = norms[self.nor[1]];
        let n2 = norms[self.nor[2]];
        let uv0 = uvs[self.uv[0]];
        let uv1 = uvs[self.uv[1]];
        let uv2 = uvs[self.uv[2]];
        let ro: Vec3 = Vec3::new(ray.origin.x, ray.origin.y, ray.origin.z);

        let mut p0t: Vec3 = v0 - ro;
        let mut p1t: Vec3 = v1 - ro;
        let mut p2t: Vec3 = v2 - ro;

        let (x, y, z) = (ray.dir.x.abs(), ray.dir.y.abs(), ray.dir.z.abs());

        let max_axis = if x > y && x > z {
            0
        } else if y > z {
            1
        } else {
            2
        };

        let mut swaped_raydir = ray.dir;

        if max_axis == 0 {
            p0t = p0t.zyx();
            p1t = p1t.zyx();
            p2t = p2t.zyx();
            swaped_raydir = swaped_raydir.zyx();
        } else if max_axis == 1 {
            p0t = p0t.xzy();
            p1t = p1t.xzy();
            p2t = p2t.xzy();
            swaped_raydir = swaped_raydir.xzy();
        }

        let sz = 1.0 / swaped_raydir.z;
        let sx = -swaped_raydir.x * sz;
        let sy = -swaped_raydir.y * sz;

        p0t.x += sx * p0t.z;
        p0t.y += sy * p0t.z;
        p1t.x += sx * p1t.z;
        p1t.y += sy * p1t.z;
        p2t.x += sx * p2t.z;
        p2t.y += sy * p2t.z;

        let mut e0 = p1t.x * p2t.y - p1t.y * p2t.x;
        let mut e1 = p2t.x * p0t.y - p2t.y * p0t.x;
        let mut e2 = p0t.x * p1t.y - p0t.y * p1t.x;
        if e0 == 0.0 || e1 == 0.0 || e2 == 0.0 {
            e0 = (p1t.x as f64 * p2t.y as f64 - p1t.y as f64 * p2t.x as f64) as f32;
            e1 = (p2t.x as f64 * p0t.y as f64 - p2t.y as f64 * p0t.x as f64) as f32;
            e2 = (p0t.x as f64 * p1t.y as f64 - p0t.y as f64 * p1t.x as f64) as f32;
        }

        if (e0 < 0.0 || e1 < 0.0 || e2 < 0.0) && (e0 > 0.0 || e1 > 0.0 || e2 > 0.0) {
            return Intersection::NONE;
        }

        let det = e0 + e1 + e2;
        if det == 0.0 {
            return Intersection::NONE;
        }

        p0t *= sz;
        p1t *= sz;
        p2t *= sz;

        let t_scaled = e0 * p0t.z + e1 * p1t.z + e2 * p2t.z;
        if (det < 0.0 && t_scaled >= 0.0) || (det > 0.0 && t_scaled <= 0.0) {
            return Intersection::NONE;
        }

        let inv_det = 1.0 / det;

        let b0 = e0 * inv_det;
        let b1 = e1 * inv_det;
        let b2 = e2 * inv_det;

        let uv = b0 * uv0 + b1 * uv1 + b2 * uv2;

        if !mats[self.mat].uv_intersect(uv, rng) {
            return Intersection::NONE;
        }

        let t = inv_det * t_scaled;

        //let mut gnormal = (v2 - v0).cross(v1 - v0).normalised();

        let mut normal = b0 * n0 + b1 * n1 + b2 * n2;
        /*if gnormal.dot(normal) < 0.0 {
            gnormal = -gnormal;
        }
        normal = gnormal;*/

        let out = normal.dot(ray.dir) < 0.0;
        if !out {
            normal = -normal;
        }

        let mut point = b0 * v0 + b1 * v1 + b2 * v2;

        point += normal * 0.000001;

        Intersection::new(t, uv, point, normal, out, self.mat, 0)
    }
    #[must_use]
    pub fn sample_ray(&self, sect: &Intersection, rng: &mut impl MinRng) -> (Ray, Vec3) {
        let verts = unsafe { VERTICES.get().as_ref_unchecked() };
        let norms = unsafe { NORMALS.get().as_ref_unchecked() };
        let mats = unsafe { MATERIALS.get().as_ref_unchecked() };
        let v0 = verts[self.pos[0]];
        let v1 = verts[self.pos[1]];
        let v2 = verts[self.pos[2]];
        let n0 = norms[self.nor[0]];
        let n1 = norms[self.nor[1]];
        let n2 = norms[self.nor[2]];

        let uv = rng.gen().sqrt();
        let uv = (1.0 - uv, uv * rng.gen());

        let mut point = uv.0 * v0 + uv.1 * v1 + (1.0 - uv.0 - uv.1) * v2;
        let nor = uv.0 * n0 + uv.1 * n1 + (1.0 - uv.0 - uv.1) * n2;
        point += nor * 0.000001;

        let dir = point - sect.pos;

        let ray = Ray::new(sect.pos, dir);

        let le = mats[self.mat].le(point, dir);

        (ray, le)
    }
    #[must_use]
    pub fn pdf(&self, sect: &Intersection, ray: &Ray) -> f32 {
        let verts = unsafe { VERTICES.get().as_ref_unchecked() };
        let v0 = verts[self.pos[0]];
        let v1 = verts[self.pos[1]];
        let v2 = verts[self.pos[2]];
        let area = 0.5 * (v1 - v0).cross(v2 - v0).mag();
        (sect.pos - ray.origin).mag_sq() / (sect.nor.dot(ray.dir).abs() * area)
    }
}
