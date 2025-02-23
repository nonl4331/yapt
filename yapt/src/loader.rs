use std::{collections::HashMap, process::exit};

use gltf::Node;

use crate::{
    overrides::{self, CamIdentifier, Overrides, TexIdentifier, TexOverride},
    prelude::*,
    RenderSettings, CAMERAS, CAMERA_MAP,
};

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

pub unsafe fn load_gltf(path: &str, render_settings: &RenderSettings, overrides: &Overrides) {
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
    let cams = unsafe { CAMERAS.get().as_mut_unchecked() };
    let mut lock_cams = CAMERA_MAP.lock().unwrap();
    let cam_map = lock_cams.get_mut_or_init(HashMap::new);

    let gltf_data = std::fs::read(path).unwrap_or_else(|e| {
        log::error!("Failed to open scene @ {path}\n{e}");
        std::process::exit(0);
    });
    let data = &gltf_data;
    if !render_settings.scene_hash.is_empty() {
        let hash = sha256::digest(data);
        if hash != render_settings.scene_hash {
            log::error!(
                "Expected sha256 scene hash \"{}\" does not equal sha256 scene hash of {path} \"{hash}\"",
                render_settings.scene_hash
            );
            exit(0);
        }
    }

    let (doc, bufs, _) = gltf::import_slice(data).unwrap_or_else(|e| {
        log::error!("Failed to load scene @ {path}\n{e}");
        std::process::exit(0);
    });

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
        Vec3::ZERO,
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vec3::ONE,
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
                        * (render_settings.width as f32 / render_settings.height as f32))
                        .to_degrees();
                    log::trace!(
                        "Loaded cam {} @ {} with fov {}deg & quat {:?}",
                        cams.len(),
                        local_translation,
                        hfov,
                        local_rotation,
                    );
                    let idx = cams.len();
                    cams.push(Cam::new_quat(
                        local_translation,
                        local_rotation,
                        hfov,
                        render_settings,
                    ));

                    if let Some(name) = cam.name() {
                        cam_map.insert(CamIdentifier::Name(name.to_owned()), idx);
                    }
                    cam_map.insert(CamIdentifier::Index(cam.index()), idx);
                }
            }

            // load mesh if it exists
            if let Some(mesh) = node.mesh() {
                let mesh_name = mesh.name().unwrap_or("");
                let m_override = overrides.mesh.get(mesh_name);

                let mat = m_override
                    .map(|o| o.material.clone())
                    .unwrap_or(overrides::MatIdentifier::Default);

                if let overrides::MatIdentifier::Invisible = mat {
                    continue;
                }

                let offset = m_override.map(|o| o.offset).unwrap_or(Vec3::ZERO);
                let _rot = m_override
                    .map(|o| o.rot)
                    .unwrap_or(overrides::Rot::Identity);
                let scale = m_override.map(|o| o.scale).unwrap_or(1.0);

                for primitive in mesh.primitives() {
                    let mat = primitive.material();

                    // "" reserved for default
                    let fallback_name: String = mat
                        .index()
                        .map(|v| v.to_string())
                        .unwrap_or_else(String::new);

                    let mut mat_name = mat.name().map(|s| s.to_owned()).unwrap_or(fallback_name);
                    if let Some(overrides::MatIdentifier::Name(name)) =
                        m_override.map(|o| o.material.clone())
                    {
                        mat_name = name;
                    }

                    // skip @ material level
                    if let Some(MatType::Invisible) = overrides.mat.get(&mat_name).map(|v| v.mtype)
                    {
                        continue;
                    }

                    let idx = if !mat_names.contains_key(&mat_name) {
                        let idx = mats.len();
                        mats.push(
                            mat_to_mat(&bufs, &mat, mat_name.clone(), tex_names, &overrides)
                                .unwrap(),
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
                                let v = v.hadamard(local_scale * Vec3::splat(scale as f32));
                                // figure out how to chain rotations
                                local_rotation
                                    .hamilton(v.into())
                                    .hamilton(local_rotation.conj())
                                    .xyz()
                                    + local_translation
                                    + offset
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TexType {
    Colour,
    RoughnessMetallic,
    Ior,
}

fn get_tex_idx(
    tex_name: String,
    tex_names: &mut HashMap<String, usize>,
    overrides: &Overrides,
    mat: &gltf::Material,
    tex_type: TexType,
    bufs: &[gltf::buffer::Data],
) -> usize {
    let metallic_roughness = mat.pbr_metallic_roughness();
    let texs = unsafe { TEXTURES.get().as_mut_unchecked() };
    // 1.
    if let Some(idx) = tex_names.get(&tex_name) {
        return *idx;
    }

    let idx = texs.len();
    tex_names.insert(tex_name.clone(), idx);

    // 2.
    if let Some(tex_override) = overrides.tex.get(&tex_name) {
        match tex_override {
            TexOverride::Default => {}
            TexOverride::Rgb(rgb) => {
                let tex = Texture::Solid(*rgb);
                texs.push(tex);
                return idx;
            }
            TexOverride::Path(_) => unimplemented!(),
            TexOverride::Data(_) => unimplemented!(),
        }
    }

    // 3. default is metallic (for now)
    let alpha_mode = mat.alpha_mode();
    let alpha_cuttof = mat.alpha_cutoff().unwrap_or(0.5);

    let get_tex = |tex_info2: Option<gltf::texture::Info<'_>>, fallback| {
        if let Some(tex_info) = tex_info2 {
            let tex = tex_info.texture();
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
            Texture::Solid(fallback)
        }
    };

    let tex = match tex_type {
        // use roughness metallic as IOR
        TexType::RoughnessMetallic | TexType::Ior => get_tex(
            metallic_roughness.metallic_roughness_texture(),
            Vec3::new(
                0.0,
                metallic_roughness.roughness_factor(),
                metallic_roughness.metallic_factor(),
            ),
        ),
        TexType::Colour => {
            let col = metallic_roughness.base_color_factor();
            get_tex(
                metallic_roughness.base_color_texture(),
                Vec3::new(col[0], col[1], col[2]),
            )
        }
    };
    texs.push(tex);
    return idx;
}

use overrides::MatType;
fn mat_to_mat(
    bufs: &[gltf::buffer::Data],
    gltf_mat: &gltf::Material,
    mat_name: String,
    tex_names: &mut HashMap<String, usize>,
    overrides: &Overrides,
) -> Option<Mat> {
    let mat_overrides = overrides.mat.get(&mat_name);

    let mat_type = mat_overrides
        .map(|o| o.mtype)
        .unwrap_or(overrides::MatType::Default);

    let mat = match mat_type {
        MatType::Default | MatType::Metallic => {
            let mut base_colour = format!("{mat_name}.base_colour");
            if let Some(TexIdentifier::Name(name)) = mat_overrides.map(|o| o.albedo.clone()) {
                log::info!("Found override for {base_colour}");
                base_colour = name;
            }

            let base_colour_tex = get_tex_idx(
                base_colour,
                tex_names,
                overrides,
                gltf_mat,
                TexType::Colour,
                bufs,
            );

            let mut metallic_roughness = format!("{mat_name}.metallic_roughness");
            if let Some(TexIdentifier::Name(name)) = mat_overrides.map(|o| o.roughness.clone()) {
                log::info!("Found override for {metallic_roughness}");
                metallic_roughness = name;
            }

            let metallic_roughness_tex = get_tex_idx(
                metallic_roughness,
                tex_names,
                overrides,
                gltf_mat,
                TexType::Ior,
                bufs,
            );
            Mat::Metallic(Ggx::new(metallic_roughness_tex, base_colour_tex))
        }
        MatType::Light => {
            let irradiance = mat_overrides
                .map(|o| o.irradiance)
                .flatten()
                .unwrap_or(Vec3::ONE);

            Mat::Light(Light::new(irradiance))
        }
        MatType::Diffuse => {
            let mut base_colour = format!("{mat_name}.base_colour");
            if let Some(TexIdentifier::Name(name)) = mat_overrides.map(|o| o.albedo.clone()) {
                log::info!("Found override for {base_colour}");
                base_colour = name;
            }

            let base_colour_tex = get_tex_idx(
                base_colour,
                tex_names,
                overrides,
                gltf_mat,
                TexType::Colour,
                bufs,
            );

            Mat::Matte(Matte::new(base_colour_tex))
        }
        MatType::Glass => {
            let ior = mat_overrides
                .map(|o| o.ior.map(|ior| ior as f32))
                .flatten()
                .unwrap_or(1.5);
            Mat::Refractive(SmoothDielectric::new(ior))
        }
        MatType::Reflective => {
            let mut base_colour = format!("{mat_name}.base_colour");
            if let Some(TexIdentifier::Name(name)) = mat_overrides.map(|o| o.albedo.clone()) {
                log::info!("Found override for {base_colour}");
                base_colour = name;
            }
            let base_colour_tex = get_tex_idx(
                base_colour,
                tex_names,
                overrides,
                gltf_mat,
                TexType::Colour,
                bufs,
            );
            Mat::Reflective(SmoothConductor::new(base_colour_tex))
        }
        MatType::Invisible => unreachable!(), // this should be checked before this function!
        MatType::Glossy => {
            let mut base_colour = format!("{mat_name}.base_colour");
            if let Some(TexIdentifier::Name(name)) = mat_overrides.map(|o| o.albedo.clone()) {
                log::info!("Found override for {base_colour}");
                base_colour = name;
            }

            let base_colour_tex = get_tex_idx(
                base_colour,
                tex_names,
                overrides,
                gltf_mat,
                TexType::Colour,
                bufs,
            );

            let ior = mat_overrides
                .map(|o| o.ior.map(|ior| ior as f32 + 1.0))
                .flatten()
                .unwrap_or(1.5);

            Mat::Glossy(Glossy::new(ior, base_colour_tex))
        }
    };

    Some(mat)
}
