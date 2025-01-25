use rand_distr::num_traits::Float;

use crate::prelude::*;

pub unsafe fn setup_scene(render_settings: &RenderSettings) -> Cam {
    match render_settings.scene.to_lowercase().trim() {
        "one" => scene_one(render_settings),
        "car" => scene_car(render_settings),
        "sphere" => scene_sphere(render_settings),
        "sphere_left_right" | "sphere-left-right" | "sphereleftright" | "slr" => {
            scene_sphere_left_right(render_settings)
        }
        "furnace_test" | "furnace-test" | "furnacetest" | "ft" => {
            scene_furnace_test(render_settings)
        }
        "room" | "r" => scene_room(render_settings),
        "sponza" | "s" => scene_sponza(render_settings),
        "sponza_ivy" | "sponza-ivy" | "sponzaivy" | "si" => scene_sponza_ivy(render_settings),
        "mitsuba_knob" | "mk" | "mitsuba-knob" => scene_mitsuba_knob(render_settings),
        _ => scene_custom(
            &render_settings.scene,
            render_settings.camera_idx,
            render_settings,
        ),
    }
}

unsafe fn scene_one(render_settings: &RenderSettings) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    loader::load_gltf("res/one.glb", 1.0, Vec3::ZERO, render_settings);
    Cam::new_rot(
        Vec3::new(4.9323, -2.1785, 2.6852),
        Vec3::new(63.527, 0.000007, 66.17),
        39.6,
        render_settings,
        true,
    )
}

unsafe fn scene_car(_render_settings: &RenderSettings) -> Cam {
    unimplemented!();
}

unsafe fn scene_sphere_left_right(_: &RenderSettings) -> Cam {
    todo!()
}

unsafe fn scene_furnace_test(_: &RenderSettings) -> Cam {
    todo!()
}

unsafe fn scene_sponza(render_settings: &RenderSettings) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    loader::add_material(
        vec!["light", "Material.001"],
        Mat::Light(Light::new(Vec3::ONE * 15.0)),
    );
    let cams = loader::load_gltf("res/sponza.glb", 1.0, Vec3::ZERO, render_settings);
    cams.into_iter().nth(0).unwrap_or_else(|| {
        Cam::new_quat(
            Vec3::new(5.280, 0.0, 0.962),
            Quaternion::new(0.386, 0.403, 0.600, 0.574),
            69.42.to_radians(),
            render_settings,
        )
    })
}

unsafe fn scene_sponza_ivy(render_settings: &RenderSettings) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    let cams = loader::load_gltf("res/sponza_ivy.glb", 1.0, Vec3::ZERO, render_settings);
    cams.into_iter().nth(0).unwrap_or_else(|| {
        Cam::new_rot(
            Vec3::new(6.8876, -0.082649, 10.742),
            Vec3::new(98.27, 0.0, 96.0),
            70.0,
            render_settings,
            true,
        )
    })
}

unsafe fn scene_room(render_settings: &RenderSettings) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    loader::add_material(
        vec!["Emitter-mid-window", "Emitter-Rear"],
        Mat::Light(Light::new(Vec3::ONE * 5.0)),
    );
    let cams = loader::load_gltf("res/room.glb", 1.0, Vec3::ZERO, render_settings);
    cams.into_iter().nth(0).unwrap()
}

unsafe fn scene_mitsuba_knob(render_settings: &RenderSettings) -> Cam {
    let _tex = loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    let _tex = loader::add_texture("backdrop_base_roughness", Texture::Solid(Vec3::ONE));

    loader::add_material(vec!["case"], Mat::Refractive(Refractive::new(1.33)));

    let cams = loader::load_gltf("res/mitsuba_knob.glb", 1.0, Vec3::ZERO, render_settings);
    cams.into_iter().nth(0).unwrap()
}

unsafe fn scene_sphere(render_settings: &RenderSettings) -> Cam {
    loader::add_material(vec!["sphere"], Mat::Refractive(Refractive::new(1.33)));
    let cams = loader::load_gltf("res/sphere.glb", 1.0, Vec3::ZERO, render_settings);
    cams.into_iter().nth(0).unwrap()
}

unsafe fn scene_custom(
    filepath: &str,
    mut camera_idx: usize,
    render_settings: &RenderSettings,
) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    let cams = loader::load_gltf(filepath, 1.0, Vec3::ZERO, render_settings);
    if !cams.is_empty() && camera_idx >= cams.len() {
        log::warn!("Camera index {camera_idx} is out of range falling back to camera 0!");
        camera_idx = 0;
    }
    cams.into_iter().nth(camera_idx).unwrap_or_else(|| {
        log::warn!("{filepath} does not contain any cameras, using fallback!");
        Cam::new_rot(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            70.0,
            render_settings,
            true,
        )
    })
}
