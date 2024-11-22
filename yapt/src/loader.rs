use std::collections::HashMap;

use crate::prelude::*;

pub unsafe fn add_material<T: Into<String>>(name: T, material: Mat) {
    let mut lock = MATERIAL_NAMES.lock().unwrap();
    let mat_names = lock.get_mut_or_init(HashMap::new);
    let mats = unsafe { MATERIALS.get().as_mut_unchecked() };
    let index = mats.len();
    mats.push(material);
    mat_names.insert(name.into(), index);
}

pub fn create_model_map<T: Into<String>>(map: Vec<(T, T)>) -> HashMap<String, String> {
    let mut lock = MATERIAL_NAMES.lock().unwrap();
    let mat_names = lock.get_mut_or_init(HashMap::new);
    let mut hashmap = HashMap::new();
    for (key, value) in map {
        let (key, value) = (key.into(), value.into());
        if !mat_names.contains_key(&value) {
            log::error!("material {value} does not exist!");
            std::process::exit(0);
        }
        hashmap.insert(key, value);
    }
    hashmap
}

pub unsafe fn load_obj(path: &str, scale: f32, offset: Vec3, model_map: &HashMap<String, String>) {
    let mats = unsafe { MATERIALS.get().as_mut_unchecked() };
    let tris = unsafe { TRIANGLES.get().as_mut_unchecked() };
    let verts = unsafe { VERTICES.get().as_mut_unchecked() };
    let norms = unsafe { NORMALS.get().as_mut_unchecked() };
    let mut lock = MATERIAL_NAMES.lock().unwrap();
    let mat_names = lock.get_mut_or_init(HashMap::new);
    let (models, model_mats) = match tobj::load_obj(
        path,
        &tobj::LoadOptions {
            triangulate: true,
            ignore_points: true,
            ignore_lines: true,
            ..Default::default()
        },
    ) {
        Ok(v) => v,
        Err(e) => {
            log::error!("{e}");
            std::process::exit(0);
        }
    };

    let mut total = 0;

    for m in &models {
        // fallback to primitive name if materials don't exist
        let mat_name = match model_mats {
            Ok(ref mats) => m
                .mesh
                .material_id
                .and_then(|i| mats.get(i).map(|mat| mat.name.clone())),
            _ => Some(m.name.clone()),
        };

        let mat_idx = match mat_name {
            Some(ref name) => model_map
                .get(name)
                .map_or(0, |mat_name| mat_names.get(mat_name).copied().unwrap_or(0)),
            None => 0,
        };

        if mat_idx >= mats.len() {
            log::error!("material index {mat_idx} does not exist!");
            std::process::exit(0);
        }

        let mesh = &m.mesh;

        let vo = verts.len();
        let no = norms.len();

        // load vertices
        for j in 0..mesh.positions.len() / 3 {
            let i = j * 3;
            verts.push(
                Vec3::new(
                    mesh.positions[i] * scale,
                    mesh.positions[i + 1] * scale,
                    mesh.positions[i + 2] * scale,
                ) + offset,
            );
        }

        // load normals
        for j in 0..mesh.normals.len() / 3 {
            let i = j * 3;
            norms.push(Vec3::new(
                mesh.normals[i],
                mesh.normals[i + 1],
                mesh.normals[i + 2],
            ));
        }

        // create triangles
        let ilen = mesh.indices.len();
        let nilen = mesh.normal_indices.len();
        assert_eq!(ilen % 3, 0);
        assert_eq!(ilen, nilen);

        for j in 0..ilen / 3 {
            let i = j * 3;
            tris.push(Tri::new(
                [
                    mesh.indices[i] as usize + vo,
                    mesh.indices[i + 1] as usize + vo,
                    mesh.indices[i + 2] as usize + vo,
                ],
                [
                    mesh.normal_indices[i] as usize + no,
                    mesh.normal_indices[i + 1] as usize + no,
                    mesh.normal_indices[i + 2] as usize + no,
                ],
                mat_idx,
            ));
        }

        total += mesh.indices.len() / 3;
    }

    log::info!("loaded {total} triangles");
}
