use crate::prelude::*;
use derive_new::new;
use json::object::Object;
use json::JsonValue;
use std::collections::HashMap;
use std::io::Read;
use std::num::NonZeroU32;
use std::path::Path;
use std::process::exit;

type Quat = Quaternion;

impl TryFrom<&[JsonValue]> for Quat {
    type Error = &'static str;
    fn try_from(value: &[JsonValue]) -> Result<Self, Self::Error> {
        if let [JsonValue::Number(w), JsonValue::Number(x), JsonValue::Number(y), JsonValue::Number(z)] =
            value[..]
        {
            Ok(Quat::new(w.into(), x.into(), y.into(), z.into()))
        } else {
            Err("Failed to parse into Quat")
        }
    }
}
impl TryFrom<&JsonValue> for Quat {
    type Error = &'static str;
    fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
        if let JsonValue::Array(arr) = value {
            Ok(arr[..].try_into()?)
        } else {
            Err("Failed to parse into Quat")
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum TexIdentifier {
    #[default]
    Default,
    Name(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum MatType {
    #[default]
    Default,
    Metallic,
    Glossy,
    Diffuse,
    Glass,
    Light,
    Invisible,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum TexOverride {
    #[default]
    Default,
    Path(std::path::PathBuf),
    Data(String),
    Rgb(Vec3),
}

#[derive(Debug, Clone, Default, PartialEq, new)]
pub struct MatOverride {
    pub mtype: MatType,
    // matte
    pub albedo: TexIdentifier,
    // light
    pub irradiance: Option<Vec3>, // possibly TexIdentifier in future
    // metallic
    pub roughness: TexIdentifier,
    pub ior_tex: TexIdentifier,
    // refractive
    pub ior: Option<f64>, // possibly TexIdentifier in future
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum MatIdentifier {
    #[default]
    Default,
    Invisible,
    Name(String),
}

#[derive(Debug, Clone, Default, PartialEq, new)]
pub struct CamOverride {
    pos: Option<Vec3>,
    rot: Option<Rot>,
    hfov: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, new)]
pub struct MeshOverride {
    pub material: MatIdentifier,
    pub offset: Vec3,
    pub rot: Rot,
    pub scale: f64,
}

impl Default for MeshOverride {
    fn default() -> Self {
        Self {
            material: MatIdentifier::default(),
            offset: Vec3::default(),
            rot: Rot::default(),
            scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Rot {
    #[default]
    Identity,
    Quat(Quat),
    Euler(Vec3),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    pub cam: HashMap<String, CamOverride>,
    pub mat: HashMap<String, MatOverride>,
    pub mesh: HashMap<String, MeshOverride>,
    pub tex: HashMap<String, TexOverride>,
}

pub fn load_overrides_file(render_settings: &mut InputParameters) -> Overrides {
    let mut overrides = Overrides::default();
    let mut string = String::new();
    let source = render_settings.overrides.clone();
    std::fs::File::open(&source)
        .unwrap()
        .read_to_string(&mut string)
        .unwrap();
    load_overrides(&mut overrides, render_settings, &string);

    let relative_to_scene = |filepath: &mut String| {
        // use relative path to scene file if not absolute
        if !filepath.is_empty() && !Path::new(filepath).has_root() {
            let mut relative_to_scene = Path::new(&source).parent().unwrap().to_owned();
            relative_to_scene.push(&filepath);
            *filepath = relative_to_scene
                .canonicalize()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap();
        }
    };

    relative_to_scene(&mut render_settings.scene_filepath);
    relative_to_scene(&mut render_settings.env_map);

    overrides
}

// assuming flat layout
fn load_overrides(overrides: &mut Overrides, render_settings: &mut InputParameters, source: &str) {
    let json = json::parse(source).unwrap();

    // top level object should always be an object
    let JsonValue::Object(obj) = json else {
        log::error!("Invalid top level object: {}", json);
        exit(0);
    };

    parse_render_settings(render_settings, &obj);

    // parse top level objects (tex.name1, cam.0, mesh.name1, ect)
    for (name, obj) in obj.iter().filter_map(|(name, val)| {
        if let JsonValue::Object(obj) = val {
            Some((name, obj))
        } else {
            None
        }
    }) {
        parse_cam_override(&mut overrides.cam, name, obj);
        parse_mesh_override(&mut overrides.mesh, name, obj);
        parse_mat_override(&mut overrides.mat, name, obj);
        parse_tex_override(&mut overrides.tex, name, obj);
    }
}

fn parse_render_settings(render_settings: &mut InputParameters, obj: &Object) {
    if render_settings.integrator.is_none() {
        let int = obj["integrator"]
            .as_str()
            .map(|v| v.to_lowercase().trim().to_owned());
        match int.as_ref().map(|v| &v[..]) {
            Some("nee") => render_settings.integrator = Some(IntegratorType::NEE),
            Some("naive") => render_settings.integrator = Some(IntegratorType::Naive),
            Some(v) => {
                log::error!("unknown integrator{v}");
                exit(0);
            }
            None => {}
        }
    }

    if let Some(path) = obj["scene"].as_str() {
        if render_settings.scene_filepath.is_empty() {
            render_settings.scene_filepath = path.to_owned();
        }
    }
    if let Some(path) = obj["output_filename"].as_str() {
        if render_settings.output_filename.is_empty() {
            render_settings.output_filename = path.to_owned();
        }
    }
    if let Some(hash) = obj["scene_hash"].as_str() {
        render_settings.scene_hash = hash.to_owned();
    }
    if let Some(hash) = obj["env_hash"].as_str() {
        render_settings.env_hash = hash.to_owned();
    }
    if render_settings.camera.is_empty() {
        if let Some(name) = obj["camera"].as_str() {
            render_settings.camera = name.to_owned();
        } else if let Some(n) = obj["camera"].as_usize() {
            render_settings.camera = format!("{n}");
        }
    }
    if render_settings.width.is_none() {
        if let Some(num) = obj["width"].as_u32() {
            let Some(w) = NonZeroU32::new(num) else {
                log::error!("width cannot be 0!");
                exit(0);
            };
            render_settings.width = Some(w);
        }
    }
    if render_settings.height.is_none() {
        if let Some(num) = obj["height"].as_u32() {
            let Some(h) = NonZeroU32::new(num) else {
                log::error!("height cannot be 0!");
                exit(0);
            };
            render_settings.height = Some(h);
        }
    }

    if let Some(num) = obj["samples"].as_u64() {
        if render_settings.samples.is_none() {
            render_settings.samples = Some(num);
        }
    }

    if let Some(heatmap) = obj["heatmap"].as_bool() {
        if render_settings.bvh_heatmap.is_none() {
            render_settings.bvh_heatmap = Some(heatmap);
        }
    }
    if let Some(headless) = obj["headless"].as_bool() {
        if render_settings.headless.is_none() {
            render_settings.headless = Some(headless);
        }
    }

    if let Some(pssmlt) = obj["pssmlt"].as_bool() {
        if render_settings.pssmlt.is_none() {
            render_settings.pssmlt = Some(pssmlt);
        }
    }

    if let Some(b) = obj["disable_shading_normals"].as_bool() {
        if render_settings.disable_shading_normals.is_none() {
            render_settings.disable_shading_normals = Some(b);
        }
    }

    if let Some(env) = obj["env_map"].as_str() {
        if render_settings.env_map.is_empty() {
            render_settings.env_map = env.to_owned();
        }
    }

    if render_settings.u_low.is_none() {
        if let Some(ulow) = obj["u_low"].as_f32() {
            if !(0.0..=1.0).contains(&ulow) {
                log::error!("u_low must be between 0 and 1!");
                exit(0);
            }
            render_settings.u_low = Some(ulow);
        }
    }
    if render_settings.u_high.is_none() {
        if let Some(uhigh) = obj["u_high"].as_f32() {
            if !(0.0..=1.0).contains(&uhigh) {
                log::error!("u_high must be between 0 and 1!");
                exit(0);
            }
            render_settings.u_high = Some(uhigh);
        }
    }
    if render_settings.v_low.is_none() {
        if let Some(vlow) = obj["v_low"].as_f32() {
            if !(0.0..=1.0).contains(&vlow) {
                log::error!("v_low mvst be between 0 and 1!");
                exit(0);
            }
            render_settings.v_low = Some(vlow);
        }
    }
    if render_settings.v_high.is_none() {
        if let Some(vhigh) = obj["v_high"].as_f32() {
            if !(0.0..=1.0).contains(&vhigh) {
                log::error!("v_high mvst be between 0 and 1!");
                exit(0);
            }
            render_settings.v_high = Some(vhigh);
        }
    }
    if let Some(threads) = obj["threads"].as_usize() {
        if render_settings.num_threads.is_none() {
            render_settings.num_threads = Some(threads);
        }
    }
}

fn parse_mat_override(mat_overrides: &mut HashMap<String, MatOverride>, name: &str, obj: &Object) {
    let Some(name) = name.strip_prefix("mat.") else {
        return;
    };
    let mut o = MatOverride::default();

    if let Some(mtype) = obj["type"].as_str() {
        o.mtype = match &mtype.to_lowercase().trim()[..] {
            "default" => MatType::Default,
            "lambertian" | "diffuse" => MatType::Diffuse,
            "ggx" | "metallic" => MatType::Metallic,
            "glossy" => MatType::Glossy,
            "glass" | "refractive" => MatType::Glass,
            "light" | "emissive" => MatType::Light,
            "invisible" => MatType::Invisible,
            _ => {
                log::error!("Unknown material type: {}", mtype);
                exit(0);
            }
        };
    }

    if let Some(ior) = obj["ior"].as_f64() {
        o.ior = Some(ior);
    } else if let Some(ior_tex) = obj["ior"].as_str() {
        o.ior_tex = TexIdentifier::Name(ior_tex.to_owned());
    }

    if let Ok(irradiance) = (&obj["irradiance"]).try_into() {
        o.irradiance = Some(irradiance);
    } else if let Some(irradiance) = obj["irradiance"].as_f32() {
        o.irradiance = Some(Vec3::splat(irradiance));
    }

    if let Some(tex) = obj["albedo"].as_str() {
        o.albedo = TexIdentifier::Name(tex.to_owned());
    }

    if let Some(tex) = obj["roughness"].as_str() {
        o.roughness = TexIdentifier::Name(tex.to_owned());
    }

    mat_overrides.insert(name.to_owned(), o);
}

fn parse_mesh_override(
    mesh_overrides: &mut HashMap<String, MeshOverride>,
    name: &str,
    obj: &Object,
) {
    let Some(name) = name.strip_prefix("mesh.") else {
        return;
    };
    let mut o = MeshOverride::default();

    // load material before visiblity check
    if let Some(mat) = obj["material"].as_str() {
        o.material = MatIdentifier::Name(mat.to_owned());
    }

    if let JsonValue::Boolean(false) = obj["visible"] {
        o.material = MatIdentifier::Invisible;
    }

    if let Ok(rot) = (&obj["rot"]).try_into() {
        o.rot = Rot::Quat(rot);
    } else if let Ok(rot) = (&obj["rot"]).try_into() {
        o.rot = Rot::Euler(rot);
    }

    if let Some(scale) = obj["scale"].as_f64() {
        o.scale = scale;
    }

    if let Ok(offset) = (&obj["offset"]).try_into() {
        o.offset = offset;
    }

    mesh_overrides.insert(name.to_owned(), o);
}

fn parse_cam_override(cam_overrides: &mut HashMap<String, CamOverride>, name: &str, obj: &Object) {
    let Some(name) = name.strip_prefix("cam.") else {
        return;
    };
    let mut o = CamOverride::default();

    if let Ok(pos) = (&obj["pos"]).try_into() {
        o.pos = Some(pos);
    }

    if let Ok(rot) = (&obj["rot"]).try_into() {
        o.rot = Some(Rot::Quat(rot));
    } else if let Ok(rot) = (&obj["rot"]).try_into() {
        o.rot = Some(Rot::Euler(rot));
    }

    if let Some(hfov) = obj["hfov"].as_f64() {
        o.hfov = Some(hfov);
    }

    cam_overrides.insert(name.to_owned(), o);
}

fn parse_tex_override(tex_overrides: &mut HashMap<String, TexOverride>, name: &str, obj: &Object) {
    let Some(name) = name.strip_prefix("tex.") else {
        return;
    };
    let mut o = TexOverride::default();

    // order is important as priority is: data > path > rgb
    if let Ok(rgb) = (&obj["rgb"]).try_into() {
        o = TexOverride::Rgb(rgb);
    }

    if let Some(path) = obj["path"].as_str() {
        o = TexOverride::Path(path.to_owned().into());
    }

    if let Some(data) = obj["data"].as_str() {
        o = TexOverride::Data(data.to_owned());
    }

    tex_overrides.insert(name.to_owned(), o);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn load_overrides(source: &str) -> (Overrides, InputParameters) {
        let mut overrides = Overrides::default();
        let mut render_settings = InputParameters::default();
        super::load_overrides(&mut overrides, &mut render_settings, source);
        (overrides, render_settings)
    }

    #[test]
    fn mesh_override_invisible() {
        const TEST: &str = r#"{"mesh.example": {"visible": false, "material": "example_mat"}}"#;
        let mut mesh = HashMap::new();
        mesh.insert(
            String::from("example"),
            MeshOverride::new(MatIdentifier::Invisible, Vec3::ZERO, Rot::Identity, 1.0),
        );
        let expected = Overrides {
            mesh,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }

    #[test]
    fn mesh_override_offset_rot_scale_material() {
        const TEST: &str = r#"{"mesh.example$$$": {"material": "example_mat👍", "offset": [3.2, -2.3, 4.1], "rot": [0.0, 3.2, 4.2], "scale": 2.1}}"#;
        let mut mesh = HashMap::new();
        mesh.insert(
            String::from("example$$$"),
            MeshOverride::new(
                MatIdentifier::Name(String::from("example_mat👍")),
                Vec3::new(3.2, -2.3, 4.1),
                Rot::Euler(Vec3::new(0.0, 3.2, 4.2)),
                2.1,
            ),
        );
        let expected = Overrides {
            mesh,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }

    #[test]
    fn mesh_default_material_quat_rot() {
        // empty strings are allowed for naming
        const TEST: &str = r#"{"mesh.": {"rot": [0.386, 0.403, 0.600, 0.574]}}"#;
        let mut mesh = HashMap::new();
        mesh.insert(
            String::from(""),
            MeshOverride::new(
                MatIdentifier::Default,
                Vec3::ZERO,
                Rot::Quat(Quat::new(0.386, 0.403, 0.600, 0.574)),
                1.0,
            ),
        );
        let expected = Overrides {
            mesh,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }

    #[test]
    fn tex_rgb() {
        const TEST: &str = r#"{"tex.example": {"rgb": [1.0, 0.0, 0.0]}}"#;
        let mut tex = HashMap::new();
        tex.insert(String::from("example"), TexOverride::Rgb(Vec3::X));
        let expected = Overrides {
            tex,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn tex_path() {
        // priority is data > path > rgb
        const TEST: &str =
            r#"{"tex.example": {"rgb": [1.0, 0.0, 0.0], "path": "example_path/image.png"}}"#;
        let mut tex = HashMap::new();
        tex.insert(
            String::from("example"),
            TexOverride::Path(String::from("example_path/image.png").into()),
        );
        let expected = Overrides {
            tex,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn tex_data() {
        // priority is data > path > rgb
        const TEST: &str = r#"{"tex.example": {"rgb": [1.0, 0.0, 0.0], "data": "raklsjdjksakldjsaklhfashfasfasljka", "path": "example_path/image.png"}}"#;
        let mut tex = HashMap::new();
        tex.insert(
            String::from("example"),
            TexOverride::Data(String::from("raklsjdjksakldjsaklhfashfasfasljka")),
        );
        let expected = Overrides {
            tex,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn mat_invisible() {
        const TEST: &str = r#"{"mat.example": {"type": "invisible"}}"#;
        let mut mat = HashMap::new();
        mat.insert(
            String::from("example"),
            MatOverride::new(
                MatType::Invisible,
                TexIdentifier::Default,
                None,
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        let expected = Overrides {
            mat,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn mat_diffuse() {
        const TEST: &str = r#"{"mat.example": {"type": "lambertian", "albedo": "bob"}, "mat.example2": {"type": "diffuse", "albedo": "barry"}}"#;
        let mut mat = HashMap::new();
        mat.insert(
            String::from("example"),
            MatOverride::new(
                MatType::Diffuse,
                TexIdentifier::Name(String::from("bob")),
                None,
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        mat.insert(
            String::from("example2"),
            MatOverride::new(
                MatType::Diffuse,
                TexIdentifier::Name(String::from("barry")),
                None,
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        let expected = Overrides {
            mat,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn mat_metallic() {
        // note: metallic currently requires a texture for ior
        const TEST: &str = r#"{"mat.example": {"type": "metallic", "ior": "bob"}, "mat.example2": {"type": "ggx", "roughness": "barry"}}"#;
        let mut mat = HashMap::new();
        mat.insert(
            String::from("example"),
            MatOverride::new(
                MatType::Metallic,
                TexIdentifier::Default,
                None,
                TexIdentifier::Default,
                TexIdentifier::Name(String::from("bob")),
                None,
            ),
        );
        mat.insert(
            String::from("example2"),
            MatOverride::new(
                MatType::Metallic,
                TexIdentifier::Default,
                None,
                TexIdentifier::Name(String::from("barry")),
                TexIdentifier::Default,
                None,
            ),
        );
        let expected = Overrides {
            mat,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn mat_glass() {
        // note: glass currently requires a float for ior
        const TEST: &str = r#"{"mat.example": {"type": "glass", "ior": 3.2}, "mat.example2": {"type": "refractive"}}"#;
        let mut mat = HashMap::new();
        mat.insert(
            String::from("example"),
            MatOverride::new(
                MatType::Glass,
                TexIdentifier::Default,
                None,
                TexIdentifier::Default,
                TexIdentifier::Default,
                Some(3.2),
            ),
        );
        mat.insert(
            String::from("example2"),
            MatOverride::new(
                MatType::Glass,
                TexIdentifier::Default,
                None,
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        let expected = Overrides {
            mat,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn mat_light() {
        // note: glass currently requires a float for ior
        const TEST: &str = r#"{"mat.example": {"type": "light", "irradiance": 15}, "mat.example2": {"type": "emissive", "irradiance": [5.0, 0.0, 1.0]}}"#;
        let mut mat = HashMap::new();
        mat.insert(
            String::from("example"),
            MatOverride::new(
                MatType::Light,
                TexIdentifier::Default,
                Some(Vec3::splat(15.0)),
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        mat.insert(
            String::from("example2"),
            MatOverride::new(
                MatType::Light,
                TexIdentifier::Default,
                Some(Vec3::new(5.0, 0.0, 1.0)),
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        let expected = Overrides {
            mat,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn mat_glossy() {
        const TEST: &str =
            r#"{"mat.example": {"type": "glossy", "ior": "some_tex", "albedo": "some_tex2"}}"#;
        let mut mat = HashMap::new();
        mat.insert(
            String::from("example"),
            MatOverride::new(
                MatType::Glossy,
                TexIdentifier::Name(String::from("some_tex2")),
                None,
                TexIdentifier::Default,
                TexIdentifier::Name(String::from("some_tex")),
                None,
            ),
        );
        let expected = Overrides {
            mat,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn cam() {
        // note: glass currently requires a float for ior
        const TEST: &str = r#"{"cam.example": {"hfov": 70, "pos": [3.2, 1.4, 0.0], "rot": [0.386, 0.403, 0.6, 0.574]}, "cam.0": {"rot": [0.0, 1.0, 0.0]}}"#;
        let mut cam = HashMap::new();
        cam.insert(
            String::from("0"),
            CamOverride::new(None, Some(Rot::Euler(Vec3::Y)), None),
        );
        cam.insert(
            String::from("example"),
            CamOverride::new(
                Some(Vec3::new(3.2, 1.4, 0.0)),
                Some(Rot::Quat(Quat::new(0.386, 0.403, 0.6, 0.574))),
                Some(70.0),
            ),
        );
        let expected = Overrides {
            cam,
            ..Default::default()
        };
        assert_eq!(load_overrides(TEST).0, expected);
    }
    #[test]
    fn render_settings() {
        const TEST: &str = r#"{"scene": "waaaaa.glb", "integrator": "nee", "output_filename": "test.png", "width": 1024, "height": 1024, "samples": 100, "headless": true, "camera": 1, "disable_shading_normals": true, "scene_hash": "abcd", "env_hash": "efgh", "u_low": 0.1, "u_high": 0.5, "v_low": 0.2, "v_high": 0.6, "threads": 16, "heatmap": true, "pssmlt": true, "env_map": "env.exr"}"#;
        let render_settings = unsafe {
            InputParameters {
                scene_filepath: String::from("waaaaa.glb"),
                integrator: Some(IntegratorType::NEE),
                output_filename: String::from("test.png"),
                env_map: String::from("env.exr"),
                width: Some(NonZeroU32::new_unchecked(1024)),
                height: Some(NonZeroU32::new_unchecked(1024)),
                samples: Some(100),
                headless: Some(true),
                camera: String::from("1"),
                disable_shading_normals: Some(true),
                pssmlt: Some(true),
                bvh_heatmap: Some(true),
                scene_hash: String::from("abcd"),
                env_hash: String::from("efgh"),
                u_low: Some(0.1),
                u_high: Some(0.5),
                v_low: Some(0.2),
                v_high: Some(0.6),
                num_threads: Some(16),
                help: None,
                overrides: String::new(),
            }
        };
        assert_eq!(load_overrides(TEST).1, render_settings);
    }
    #[test]
    fn full_load() {
        let render_settings = unsafe {
            InputParameters {
                scene_filepath: String::from("test.glb"),
                integrator: Some(IntegratorType::NEE),
                output_filename: String::from("test.png"),
                env_map: String::from("env.exr"),
                width: Some(NonZeroU32::new_unchecked(1024)),
                height: Some(NonZeroU32::new_unchecked(1024)),
                samples: Some(100),
                headless: Some(true),
                camera: String::from("1"),
                disable_shading_normals: Some(true),
                pssmlt: Some(true),
                bvh_heatmap: Some(true),
                scene_hash: String::from(
                    "ceb56db724d9ba50f0ce9b1081ddb22570348b2dc90749622a1ec8b38f6b0963",
                ),
                env_hash: String::from(
                    "9ef9eb7f373ac6523e64cd003fea41320451958b25ff6cfdc894227240e006f1",
                ),
                u_low: Some(0.1),
                u_high: Some(0.5),
                v_low: Some(0.2),
                v_high: Some(0.6),
                num_threads: Some(32),
                help: None,
                overrides: String::new(),
            }
        };
        let mut mesh = HashMap::new();
        mesh.insert(
            String::from("robot"),
            MeshOverride {
                material: MatIdentifier::Invisible,
                ..MeshOverride::default()
            },
        );
        mesh.insert(
            String::from("alien"),
            MeshOverride {
                material: MatIdentifier::Name(String::from("exists1")),
                offset: Vec3::new(34.0, 1.2, -3.2),
                rot: Rot::Quat(Quat::new(0.386, 0.403, 0.600, 0.574)),
                ..MeshOverride::default()
            },
        );
        mesh.insert(
            String::from("dog"),
            MeshOverride {
                rot: Rot::Euler(Vec3::new(0.386, 0.403, 0.650)),
                scale: 2.0,
                ..MeshOverride::default()
            },
        );
        let mut mat = HashMap::new();
        mat.insert(
            String::from("exists1"),
            MatOverride::new(
                MatType::Diffuse,
                TexIdentifier::Name(String::from("exists1.base_color")),
                None,
                TexIdentifier::Default,
                TexIdentifier::Default,
                None,
            ),
        );
        mat.insert(
            String::from("exists2"),
            MatOverride {
                mtype: MatType::Glass,
                ior: Some(1.5),
                roughness: TexIdentifier::Name(String::from("exists1.roughness")),
                ..MatOverride::default()
            },
        );
        mat.insert(
            String::from("exists3"),
            MatOverride {
                mtype: MatType::Light,
                irradiance: Some(Vec3::splat(15.0)),
                roughness: TexIdentifier::Name(String::from("custom2")),
                ..MatOverride::default()
            },
        );
        mat.insert(
            String::from("exists4"),
            MatOverride {
                mtype: MatType::Invisible,
                ..MatOverride::default()
            },
        );
        let mut tex = HashMap::new();
        tex.insert(String::from("custom1"), TexOverride::Rgb(Vec3::ONE));
        tex.insert(
            String::from("custom2"),
            TexOverride::Path(PathBuf::from("relative_path/image.png")),
        );
        tex.insert(
            String::from("custom3"),
            TexOverride::Data(String::from("BINARY_DATA")),
        );
        let mut cam = HashMap::new();
        cam.insert(
            String::from("0"),
            CamOverride {
                hfov: Some(70.0),
                pos: Some(Vec3::ZERO),
                rot: Some(Rot::Quat(Quat::new(0.386, 0.403, 0.6, 0.574))),
                ..CamOverride::default()
            },
        );
        cam.insert(
            String::from("1"),
            CamOverride {
                rot: Some(Rot::Euler(Vec3::ZERO)),
                ..CamOverride::default()
            },
        );
        let expected = Overrides {
            mesh,
            mat,
            tex,
            cam,
            ..Default::default()
        };
        assert_eq!(load_overrides(BIG_TEST), (expected, render_settings));
    }
    const BIG_TEST: &str = r#"
{
    "scene": "test.glb",
    "integrator": "nee",
    "output_filename": "test.png",
    "env_map": "env.exr",
    "u_low": 0.1,
    "u_high": 0.5,
    "v_low": 0.2,
    "v_high": 0.6,
    "threads": 32,
    "headless": true,
    "camera": 1,
    "pssmlt": true,
    "heatmap": true,
    "disable_shading_normals": true,
    "width": 1024,
    "height": 1024,
    "samples": 100,
    "scene_hash": "ceb56db724d9ba50f0ce9b1081ddb22570348b2dc90749622a1ec8b38f6b0963",
    "env_hash": "9ef9eb7f373ac6523e64cd003fea41320451958b25ff6cfdc894227240e006f1",

    "mesh.robot": {
        "visible": false
    },
    "mesh.alien": {
        "material": "exists1",
        "offset": [34.0, 1.2, -3.2],
        "rot": [0.386, 0.403, 0.600, 0.574]
    },
    "mesh.dog": {
        "rot": [0.386, 0.403, 0.650],
        "scale": 2.0
    },

    "tex.custom1": {
        "rgb": [1.0, 1.0, 1.0]
    },
    "tex.custom2": {
        "path": "relative_path/image.png"
    },
    "tex.custom3": {
        "data": "BINARY_DATA"
    },
    
    
    "mat.exists1": {
        "type": "lambertian",
        "albedo": "exists1.base_color"
    },
    "mat.exists2": {
        "type": "glass",
        "ior": 1.5,
        "roughness": "exists1.roughness"
    },
    "mat.exists3": {
        "type": "light",
        "irradiance": 15,
        "roughness": "custom2"
    },
    "mat.exists4": {
        "type": "invisible"
    },


    "cam.0": {
        "hfov": 70,
        "pos": [
            0,
            0,
            0
        ],
        "rot": [
            0.386,
            0.403,
            0.6,
            0.574
        ]
    },
    "cam.1": {
        "rot": [
            0,
            0,
            0
        ]
    }
}
"#;
}
