use std::collections::HashMap;

use gltf::{material::AlphaMode, Node};

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
pub unsafe fn add_texture<T: Into<String>>(name: T, texture: Texture) -> usize {
    let mut lock = TEXTURE_NAMES.lock().unwrap();
    let tex_names = lock.get_mut_or_init(HashMap::new);
    let texs = unsafe { TEXTURES.get().as_mut_unchecked() };
    let index = texs.len();
    texs.push(texture);
    tex_names.insert(name.into(), index);
    index
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

pub unsafe fn load_gltf(
    path: &str,
    scale: f32,
    offset: Vec3,
    render_settings: &RenderSettings,
) -> Vec<Cam> {
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

    let mut cams = Vec::new();
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

    let mut node_queue = vec![NodeCollection::new(
        scene.nodes().collect(),
        offset,
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vec3::splat(scale),
    )];

    while let Some(NodeCollection {
        mut nodes,
        translation,
        rotation,
        scale,
    }) = node_queue.pop()
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
                if let gltf::camera::Projection::Perspective(perp) = cam.projection() {
                    let hfov = (perp.yfov()
                        * (render_settings.width.get() as f32
                            / render_settings.height.get() as f32))
                        .to_degrees();
                    log::info!(
                        "Loaded cam {} @ {} with fov {}deg & quat {:?}",
                        cams.len(),
                        local_translation,
                        hfov,
                        local_rotation,
                    );
                    cams.push(Cam::new_quat(
                        local_translation,
                        local_rotation,
                        hfov,
                        render_settings,
                    ));
                }
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

                            let new_uvs: Vec<Vec2> = if let Some(coords) =
                                reader.read_tex_coords(0).map(|v| v.into_f32())
                            {
                                coords.map(|v| v.into()).collect()
                            } else {
                                vec![Vec2::ZERO; new_verticies.len()]
                            };

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
            node_queue.push(NodeCollection::new(
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

fn get_tex_idx(
    bufs: &[gltf::buffer::Data],
    tex_names: &mut HashMap<String, usize>,
    texs: &mut Vec<Texture>,
    info: Option<gltf::texture::Info>,
    fallback: [f32; 4],
    tex_name: String,
    alpha_mode: AlphaMode,
    alpha_cuttof: f32,
) -> usize {
    if let Some(idx) = tex_names.get(&tex_name) {
        return *idx;
    }

    let idx = texs.len();

    let tex = if let Some(info) = info {
        let tex = info.texture();
        let source = tex.source().source();
        let gltf::image::Source::View { view, .. } = source else {
            panic!()
        };
        let buff = &bufs[view.buffer().index()];

        let start = view.offset();
        let end = start + view.length();
        let tex_data = &buff[start..end];
        let image = image::load_from_memory(tex_data).unwrap();
        let image = image.to_rgba32f();
        let dim = image.dimensions();
        let image = image.into_vec();
        Texture::Image(Image::from_rgbaf32(
            dim.0 as usize,
            dim.1 as usize,
            image,
            alpha_mode,
            alpha_cuttof,
        ))
    } else {
        Texture::Solid(Vec3::new(fallback[0], fallback[1], fallback[2]))
    };

    texs.push(tex);
    tex_names.insert(tex_name, idx);
    idx
}

fn mat_to_mat(
    bufs: &[gltf::buffer::Data],
    gltf_mat: &gltf::Material,
    mat_name: String,
    texs: &mut Vec<Texture>,
    tex_names: &mut HashMap<String, usize>,
) -> Option<Mat> {
    let roughness = gltf_mat.pbr_metallic_roughness();

    // get base col
    let base_col_idx = get_tex_idx(
        bufs,
        tex_names,
        texs,
        roughness.base_color_texture(),
        roughness.base_color_factor(),
        format!("{mat_name}_base_colour"),
        gltf_mat.alpha_mode(),
        gltf_mat.alpha_cutoff().unwrap_or(0.5),
    );

    // get roughnes
    let metallic_roughness_idx = get_tex_idx(
        bufs,
        tex_names,
        texs,
        roughness.metallic_roughness_texture(),
        [
            0.0,
            roughness.roughness_factor(),
            roughness.metallic_factor(),
            1.0,
        ],
        format!("{mat_name}_base_roughness"),
        gltf_mat.alpha_mode(),
        gltf_mat.alpha_cutoff().unwrap_or(0.5),
    );

    Some(Mat::Glossy(Ggx::new(metallic_roughness_idx, base_col_idx)))
}
