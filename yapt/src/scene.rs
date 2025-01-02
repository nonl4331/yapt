use rand_distr::num_traits::Float;

use crate::prelude::*;

#[derive(clap::ValueEnum, Copy, Clone, Default)]
pub enum Scene {
    #[default]
    One,
    Car,
    Sphere,
    SphereLeftRight,
    FurnaceTest,
    Room,
    Sponza,
    SponzaIvy,
}
impl fmt::Display for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::One => "one",
            Self::Car => "car",
            Self::Sphere => "sphere",
            Self::SphereLeftRight => "sphere_left_right",
            Self::FurnaceTest => "furnace_test",
            Self::Room => "room",
            Self::Sponza => "sponza",
            Self::SponzaIvy => "sponza_ivy",
        };
        write!(f, "{s}")
    }
}
pub unsafe fn setup_scene(render_settings: &RenderSettings) -> Cam {
    match render_settings.scene {
        Scene::One => scene_one(render_settings),
        Scene::Car => scene_car(render_settings),
        Scene::Sphere => scene_sphere(render_settings),
        Scene::SphereLeftRight => scene_sphere_left_right(render_settings),
        Scene::FurnaceTest => scene_furnace_test(render_settings),
        Scene::Room => scene_room(render_settings),
        Scene::Sponza => scene_sponza(render_settings),
        Scene::SponzaIvy => scene_sponza_ivy(render_settings),
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

unsafe fn scene_sphere(render_settings: &RenderSettings) -> Cam {
    todo!();
}

unsafe fn scene_sphere_left_right(render_settings: &RenderSettings) -> Cam {
    todo!()
}

unsafe fn scene_furnace_test(render_settings: &RenderSettings) -> Cam {
    todo!()
}

unsafe fn scene_room(render_settings: &RenderSettings) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    loader::add_material(
        vec!["light", "grey_and_white_room:lambert2SG_light"],
        Mat::Light(Light::new(Vec3::ONE * 5.0)),
    );
    loader::load_gltf("res/room.glb", 1.0, Vec3::ZERO, render_settings);
    Cam::new_rot(
        Vec3::new(1.9687, -4.5139, 1.7961),
        Vec3::new(79.927, 0.0, 4.1697),
        70.0,
        render_settings,
        true,
    )
}

unsafe fn scene_sponza(render_settings: &RenderSettings) -> Cam {
    loader::add_texture("__default", Texture::Solid(Vec3::splat(0.5)));
    loader::add_material(vec!["rest"], Mat::Matte(Matte::new(0)));
    loader::add_material(
        vec!["light", "Material.001"],
        Mat::Light(Light::new(Vec3::ONE * 5.0)),
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
