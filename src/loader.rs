use std::collections::HashMap;

use crate::prelude::*;

pub unsafe fn add_material<T: Into<String>>(name: T, material: Mat) {
    let index = MATERIALS.len();
    MATERIALS.push(material);
    MATERIAL_NAMES.insert(name.into(), index);
}

pub fn create_model_map<T: Into<String>>(map: Vec<(T, T)>) -> HashMap<String, String> {
    let mut hashmap = HashMap::new();
    for (key, value) in map {
        let (key, value) = (key.into(), value.into());
        if !unsafe { MATERIAL_NAMES.contains_key(&value) } {
            log::error!("material {value} does not exist!");
            std::process::exit(0);
        }
        hashmap.insert(key, value);
    }
    hashmap
}

pub unsafe fn load_obj(path: &str, scale: f32, offset: Vec3, model_map: HashMap<String, String>) {
    let (models, _) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            triangulate: true,
            ignore_points: true,
            ignore_lines: true,
            ..Default::default()
        },
    )
    .unwrap();

    let mut total = 0;

    for m in models.iter() {
        // load material
        let mat_idx = model_map
            .get(&m.name)
            .map(|mat_name| MATERIAL_NAMES.get(mat_name).copied().unwrap_or(0))
            .unwrap_or(0);

        if mat_idx >= MATERIALS.len() {
            log::error!("material index {mat_idx} does not exist!");
            std::process::exit(0);
        }

        let mesh = &m.mesh;

        let vo = VERTICES.len();
        let no = NORMALS.len();

        // load vertices
        for j in 0..mesh.positions.len() / 3 {
            let i = j * 3;
            VERTICES.push(
                Vec3::new(
                    mesh.positions[i] * scale,
                    mesh.positions[i + 1] * scale,
                    mesh.positions[i + 2] * scale,
                ) + offset,
            )
        }

        // load normals
        for j in 0..mesh.normals.len() / 3 {
            let i = j * 3;
            NORMALS.push(Vec3::new(
                mesh.normals[i],
                mesh.normals[i + 1],
                mesh.normals[i + 2],
            ))
        }

        // create triangles
        let ilen = mesh.indices.len();
        let nilen = mesh.normal_indices.len();
        assert_eq!(ilen % 3, 0);
        assert_eq!(ilen, nilen);

        for j in 0..ilen / 3 {
            let i = j * 3;
            TRIANGLES.push(Tri::new(
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
