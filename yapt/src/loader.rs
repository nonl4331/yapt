use std::collections::HashMap;

use gltf::Node;

use crate::prelude::*;

pub unsafe fn add_material<A: Into<String>>(names: Vec<A>, material: Mat) {
    let mut lock = MATERIAL_NAMES.lock().unwrap();
    let mat_names = lock.get_mut_or_init(HashMap::new);
    let mats = unsafe { MATERIALS.get().as_mut_unchecked() };
    let index = mats.len();
    mats.push(material);
    for name in names.into_iter() {
        mat_names.insert(name.into(), index);
    }
}
pub unsafe fn add_texture<T: Into<String>>(name: T, texture: Texture) {
    let mut lock = TEXTURE_NAMES.lock().unwrap();
    let tex_names = lock.get_mut_or_init(HashMap::new);
    let texs = unsafe { TEXTURES.get().as_mut_unchecked() };
    let index = texs.len();
    texs.push(texture);
    tex_names.insert(name.into(), index);
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
    unimplemented!();
}

pub unsafe fn load_gltf(path: &str, scale: f32, offset: Vec3) -> Vec<Cam> {
    let mats = unsafe { MATERIALS.get().as_mut_unchecked() };
    let texs = unsafe { TEXTURES.get().as_mut_unchecked() };
    let tris = unsafe { TRIANGLES.get().as_mut_unchecked() };
    let verts = unsafe { VERTICES.get().as_mut_unchecked() };
    let norms = unsafe { NORMALS.get().as_mut_unchecked() };
    let uvs = unsafe { UVS.get().as_mut_unchecked() };
    let mut lock = MATERIAL_NAMES.lock().unwrap();
    let mat_names = lock.get_mut_or_init(HashMap::new);
    let mut lock_tex = TEXTURE_NAMES.lock().unwrap();
    let tex_names = lock_tex.get_mut_or_init(HashMap::new);

    let cams = Vec::new();
    let (doc, bufs, _) = match gltf::import(path) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to load gltf @ {path}\n{e}");
            std::process::exit(0);
        }
    };

    let Some(scene) = doc.default_scene() else {
        log::error!("No default scene in gltf @ {path}");
        std::process::exit(0);
    };

    struct NodeCollection<'a> {
        nodes: Vec<Node<'a>>,
        translation: Vec3,
        rotation: Quaternion,
        scale: Vec3,
    }

    impl<'a> NodeCollection<'a> {
        pub fn new(
            nodes: Vec<Node<'a>>,
            translation: Vec3,
            rotation: Quaternion,
            scale: Vec3,
        ) -> Self {
            Self {
                nodes,
                translation,
                rotation,
                scale,
            }
        }
    }

    let mut node_queue_two = vec![NodeCollection::new(
        scene.nodes().collect(),
        Vec3::ZERO,
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vec3::ONE,
    )];

    while let Some(NodeCollection {
        mut nodes,
        translation,
        rotation,
        scale,
    }) = node_queue_two.pop()
    {
        while let Some(node) = nodes.pop() {
            let (local_translation, local_rotation, local_scale) = node.transform().decomposed();

            let local_translation: Vec3 = local_translation.into();
            let local_translation = local_translation + translation;

            let local_rotation = rotation.hamilton(Quaternion::new(
                local_rotation[3],
                local_rotation[0],
                local_rotation[1],
                local_rotation[2],
            ));

            let local_scale = scale.hadamard(local_scale.into());

            // load camera if it exists
            if let Some(cam) = node.camera() {
                //let cam = Cam::new_quat(translation, rotation, 70.0, ());
            }

            // load mesh if it exists
            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let mat = primitive.material();

                    // 0 reserved for default
                    let fallback_name: String = mat.index().map(|v| v + 1).unwrap_or(0).to_string();
                    let mat_name = mat.name().map(|s| s.to_owned()).unwrap_or(fallback_name);

                    let idx = if !mat_names.contains_key(&mat_name) {
                        let idx = mats.len();
                        mats.push(
                            mat_to_mat(&bufs, &mat, mat_name.clone(), texs, tex_names).unwrap(),
                        );
                        mat_names.insert(mat_name, idx);
                        idx
                    } else {
                        *mat_names.get(&mat_name).unwrap()
                    };

                    let reader = primitive.reader(|buffer| Some(&bufs[buffer.index()]));

                    match primitive.mode() {
                        gltf::mesh::Mode::Triangles => {
                            let vert_offset = verts.len();
                            let norm_offset = norms.len();
                            let uv_offset = uvs.len();
                            fn normalize_uv(uv: Vec2) -> Vec2 {
                                Vec2::new(uv.x - uv.x.floor(), uv.y - uv.y.floor())
                            }

                            let apply_transform = |v: Vec3| -> Vec3 {
                                let v = v.hadamard(local_scale);
                                local_rotation
                                    .hamilton(v.into())
                                    .hamilton(local_rotation.conj())
                                    .xyz()
                                    + local_translation
                            };

                            let new_verticies: Vec<Vec3> = reader
                                .read_positions()
                                .unwrap()
                                .map(|v| v.into())
                                .map(apply_transform)
                                .collect();

                            let new_normals: Vec<Vec3> = reader
                                .read_normals()
                                .unwrap()
                                .map(|v| v.into())
                                .map(apply_transform)
                                .collect();

                            let new_uvs: Vec<Vec2> = reader
                                .read_tex_coords(0)
                                .unwrap()
                                .into_f32()
                                .map(|v| normalize_uv(v.into()))
                                .collect();

                            verts.extend(new_verticies);
                            norms.extend(new_normals);
                            uvs.extend(new_uvs);

                            let new_tris: Vec<_> = reader
                                .read_indices()
                                .unwrap()
                                .into_u32()
                                .map(|v| v as usize)
                                .collect::<Vec<_>>()
                                .chunks_exact(3)
                                .map(|chunk| {
                                    let a = chunk[0];
                                    let b = chunk[1];
                                    let c = chunk[2];
                                    Tri::new(
                                        [a + vert_offset, b + vert_offset, c + vert_offset],
                                        [a + norm_offset, b + norm_offset, c + norm_offset],
                                        [a + uv_offset, b + uv_offset, c + uv_offset],
                                        idx,
                                    )
                                })
                                .collect();

                            tris.extend(new_tris);
                        }
                        gltf::mesh::Mode::TriangleFan => todo!(),
                        gltf::mesh::Mode::TriangleStrip => todo!(),
                        mode => {
                            log::error!("Unsupported primitive type: {mode:?}");
                            std::process::exit(0);
                        }
                    }
                }
            }

            let child_nodes: Vec<_> = node.children().collect();
            node_queue_two.push(NodeCollection::new(
                child_nodes,
                local_translation,
                local_rotation,
                local_scale,
            ));
        }
    }

    log::info!("Loaded: {} triangles", tris.len());
    log::info!("Loaded: {} materials", mats.len());
    log::info!("Loaded: {} textures", texs.len());
    log::info!("Loaded: {} verts", verts.len());
    log::info!("Loaded: {} norms", norms.len());

    cams
}

fn mat_to_mat(
    bufs: &[gltf::buffer::Data],
    gltf_mat: &gltf::Material,
    mat_name: String,
    texs: &mut Vec<Texture>,
    tex_names: &mut HashMap<String, usize>,
) -> Option<Mat> {
    let roughness = gltf_mat.pbr_metallic_roughness();
    match roughness.base_color_texture() {
        Some(info) => {
            let tex = info.texture();
            let source = tex.source().source();
            let gltf::image::Source::View { view, .. } = source else {
                panic!()
            };
            let buff = &bufs[view.buffer().index()];
            let tex_name = tex.name().map(|v| v.to_owned()).unwrap_or(mat_name);

            let idx = if !tex_names.contains_key(&tex_name) {
                let start = view.offset();
                let end = start + view.length();
                let tex_data = &buff[start..end];
                let image = image::load_from_memory(tex_data).unwrap();
                let image = image.to_rgb32f();
                let dim = image.dimensions();
                let image = image.into_vec();
                let tex = Texture::Image(Image::from_rgbf32(dim.0 as usize, dim.1 as usize, image));
                let idx = texs.len();
                texs.push(tex);
                tex_names.insert(tex_name, idx);
                idx
            } else {
                *tex_names.get(&tex_name).unwrap()
            };
            return Some(Mat::Glossy(Ggx::new(roughness.roughness_factor(), idx)));
        }
        None => {
            let base_col = roughness.base_color_factor();
            let tex_name = mat_name;
            let idx = if !tex_names.contains_key(&tex_name) {
                let tex = Texture::Solid(Vec3::new(base_col[0], base_col[1], base_col[2]));
                let idx = texs.len();
                texs.push(tex);
                tex_names.insert(tex_name, idx);
                idx
            } else {
                *tex_names.get(&tex_name).unwrap()
            };

            return Some(Mat::Glossy(Ggx::new(roughness.roughness_factor(), idx)));
        }
    }
}
