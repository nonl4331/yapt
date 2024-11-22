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
        Scene::SponzaIvy => scene_sponza_ivy(render_settings),
    }
}
unsafe fn scene_one(render_settings: &RenderSettings) -> Cam {
    loader::add_material("floor", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("ball1", Mat::Matte(Matte::new(Vec3::new(0.5, 0.8, 0.8))));
    loader::add_material("ball2", Mat::Matte(Matte::new(Vec3::new(0.8, 0.0, 0.0))));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE * 3.0)));

    let model_map = loader::create_model_map(vec![
        ("floor", "floor"),
        ("ball1", "ball1"),
        ("light", "light"),
        ("ball2", "ball2"),
    ]);

    loader::load_obj("res/one.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(0.0, -1.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::Z,
        70.0,
        1.0,
        render_settings,
    )
}

unsafe fn scene_car(_render_settings: &RenderSettings) -> Cam {
    unimplemented!();
}

unsafe fn scene_sphere(render_settings: &RenderSettings) -> Cam {
    let test_mat = Mat::Glossy(Ggx::new(0.001, Vec3::ONE));
    loader::add_material("default", test_mat);

    let model_map = loader::create_model_map(vec![("default", "default")]);

    loader::load_obj("res/sphere.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(0.0, -3.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        render_settings,
    )
}

unsafe fn scene_sphere_left_right(render_settings: &RenderSettings) -> Cam {
    let test_mat = Mat::Glossy(Ggx::new(0.001, Vec3::ONE));
    loader::add_material("test mat", test_mat);
    loader::add_material("diffuse", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material(
        "orange light",
        Mat::Light(Light::new(Vec3::new(1.0, 0.65, 0.0))),
    );
    loader::add_material(
        "blue light",
        Mat::Light(Light::new(Vec3::new(0.68, 0.85, 0.9))),
    );

    let model_map = loader::create_model_map(vec![
        ("floor", "diffuse"),
        ("sphere", "test mat"),
        ("right_light", "orange light"),
        ("left_light", "blue light"),
    ]);

    loader::load_obj("res/sphere_left_right.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(0.0, -3.0, 2.0),
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::Z,
        70.0,
        1.0,
        render_settings,
    )
}

unsafe fn scene_furnace_test(render_settings: &RenderSettings) -> Cam {
    loader::add_material("Inner", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE)));

    let model_map = loader::create_model_map(vec![("Inner", "Inner"), ("Outer", "light")]);

    loader::load_obj("res/furnace_test.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(-4.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        render_settings,
    )
}

unsafe fn scene_room(render_settings: &RenderSettings) -> Cam {
    loader::add_material("rest", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material("empty", Mat::Invisible);
    loader::add_material("light", Mat::Light(Light::new(Vec3::ONE * 1.0)));

    let model_map = loader::create_model_map(vec![
        ("grey_and_white_room:lambert2SG_light", "empty"),
        ("lambert3SG", "empty"),
        ("grey_and_white_room:Glass", "empty"),
        ("grey_and_white_room:lambert2SG_light", "empty"),
        ("light", "light"),
    ]);

    loader::load_obj("res/room.obj", 1.0, Vec3::ZERO, &model_map);

    Cam::new(
        Vec3::new(3.0, -3.0, 1.8),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::Z,
        70.0,
        1.0,
        render_settings,
    )
}

unsafe fn scene_sponza_ivy(render_settings: &RenderSettings) -> Cam {
    loader::add_material("rest", Mat::Matte(Matte::new(Vec3::ONE * 0.5)));
    loader::add_material(
        "green",
        Mat::Glossy(Ggx::new(0.95, Vec3::new(0.0, 1.0, 0.0))),
    );
    let model_map = loader::create_model_map(vec![("default", "rest"), ("IvyLeaf", "green")]);
    loader::load_obj("res/sponza_ivy.obj", 1.0, Vec3::ZERO, &model_map);
    Cam::new(
        Vec3::new(5.0, 0.92, 9.7475),
        Vec3::new(3.0, -0.3, 9.7475),
        Vec3::Z,
        70.0,
        1.0,
        render_settings,
    )
}
