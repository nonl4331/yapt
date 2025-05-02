#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU8;
    use std::sync::atomic::Ordering::SeqCst;
    pub static LOADED_DATA: AtomicU8 = AtomicU8::new(0);


    impl TextureHandler for Vec3 {
        fn uv_value(&self, _: Vec2) -> Vec3 { *self }
    }

    #[cfg(not(feature = "rand"))]
    mod r {
        use rand::Rng;
        impl<R: Rng> crate::MinRng for R {
            fn random(&mut self) -> f32 {
                self.random::<f32>()
            }
            fn random_range(&mut self, range: core::ops::Range<f32>) -> f32 {
                self.random_range(range)
            }
        }
    }

    fn init_test() {
        if LOADED_DATA.load(SeqCst) == 2 {
            return;
        }
        if LOADED_DATA.swap(1, SeqCst) == 1 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            return;
        }
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Info)
            .parse_default_env()
            .is_test(true)
            .init();
        LOADED_DATA.store(2, SeqCst);
    }

    const THETA_BINS: usize = 180;
    const PHI_BINS: usize = 2 * THETA_BINS;
    const BINS: usize = PHI_BINS * THETA_BINS;
    const PDF_EPS: f64 = 1e-3;
    const SAMPLES: usize = 10_000_000;

    use crate::*;

    #[test]
    pub fn lambertian() {
        init_test();
        let mut rng = rand::rng();
        let wo = generate_wo(&mut rng, true);

        let mat = Lambertian::new(Vec3::ZERO);

        test_material("lambertian", mat, wo, &mut rng);
    }

    // note this doesn't test the delta dirac part yet
    #[test]
    pub fn layered() {
        init_test();
        let mut rng = rand::rng();
        let wo = generate_wo(&mut rng, true);

        let mat = SmoothDielectricLambertian::new_raw(1.5, Vec3::ZERO);

        let sect = &Intersection::new(1.0, Vec2::ZERO, Vec3::ZERO, Vec3::Z, true, 0, 0);

        let sample = || -> Vec3 {
            loop {
                let mut ray = Ray::new(Vec3::ZERO, -wo);
                if !mat
                    .scatter(sect, &mut ray, &mut rng)
                    .contains(ScatterStatus::DIRAC_DELTA)
                {
                    return ray.dir;
                }
            }
        };
        let pdf = |wo: Vec3, wi: Vec3| -> f32 { mat.pdf(sect, wi, wo) };

        log_info("glossy", format!("wo: {wo}"));

        sample_image(sample, SAMPLES, "glossy");

        let sum = integrate_pdf(pdf, wo, "glossy");

        let non_dirac = 1.0 - fresnel_dielectric(1.0, mat.ior, sect.nor, wo);

        log_info("glossy", format!("sum: {sum} vs {non_dirac}"));

        assert!((sum - non_dirac as f64).abs() < PDF_EPS);
    }

    #[test]
    pub fn rough_dielectric() {
        init_test();
        let mut rng = rand::rng();
        let wo = generate_wo(&mut rng, true);
        let rand = Vec3::splat(MinRng::random(&mut rng).max(0.0001));
        let a = rand.y;

        let name = "rough dielectric";
        let mat = RoughDielectric::new(Vec3::Y * MinRng::random(&mut rng), 1.5);

        log_info("rough dielectric", format!("alpha: {a}"));

        test_material(name, mat, wo, &mut rng);
    }

    #[test]
    pub fn ggx() {
        init_test();
        let mut rng = rand::rng();
        let wo = generate_wo(&mut rng, true);
        let rand = Vec3::splat(MinRng::random(&mut rng).max(0.0001));
        let a = rand.y;

        let name = "ggx";
        let mat = RoughConductor::new(Vec3::Y * MinRng::random(&mut rng), Vec3::ONE);

        log_info("ggx", format!("alpha: {a}"));

        test_material(name, mat, wo, &mut rng);
    }

    fn log_info(mat: &str, info: String) {
        log::info!("{mat}: {info}");
    }

    fn test_material(name: &str, m: Material<Vec3>, wo: Vec3, rng: &mut impl MinRng) {
        let sect = &Intersection::new(1.0, Vec2::ZERO, Vec3::ZERO, Vec3::Z, true, 0, 0);

        let sample = || -> Vec3 {
            let mut ray = Ray::new(Vec3::ZERO, -wo);
            m.scatter(sect, &mut ray, rng);
            ray.dir
        };
        let pdf = |wo: Vec3, wi: Vec3| -> f32 { m.spdf(sect, wo, wi) };

        log_info(name, format!("wo: {wo}"));

        sample_image(sample, SAMPLES, name);

        let sum = integrate_pdf(pdf, wo, name);

        log_info(name, format!("sum: {sum}"));

        assert!((sum - 1.0).abs() < PDF_EPS);
    }


    #[test]
    fn vndf() {
        init_test();
        let mut rng = rand::rng();
        let rand = Vec3::splat(MinRng::random(&mut rng).max(0.0001));
        let wo = generate_wo(&mut rng, true);
        let a = rand.y;
        let a_sq = a.powi(2);

        let name = "ggx_vndf";
        let mat = RoughConductor::new_raw(rand, Vec3::ONE);

        log_info("ggx_vndf", format!("alpha: {a}"));

        let sample = || -> Vec3 { mat.sample_vndf_local(a, wo, &mut rng) };
        let pdf = |wo: Vec3, wm: Vec3| -> f32 { mat.vndf_local(a_sq, wm, wo) };

        log_info(name, format!("wo: {wo}"));

        sample_image(sample, SAMPLES, name);

        let sum = integrate_pdf(pdf, wo, name);

        log_info(name, format!("sum: {sum}"));
        assert!((sum - 1.0).abs() < PDF_EPS, "sum = {sum}");
    }

    #[test]
    fn vndf_transformed() {
        init_test();
        let mut rng = rand::rng();
        let wo = generate_wo(&mut rng, true);
        let rand = Vec3::splat(MinRng::random(&mut rng).max(0.0001));
        let a = rand.y;
        let a_sq = a.powi(2);

        let name = "ggx_vndf_transformed";
        let mat = RoughConductor::new_raw(rand, Vec3::ONE);

        log_info("ggx_vndf_transformed", format!("alpha: {a}"));

        let sample = || -> Vec3 {
            let wm = mat.sample_vndf_local(a, wo, &mut rng);
            wo.reflected(wm)
        };
        let pdf = |wo: Vec3, wi: Vec3| -> f32 {
            let wm = (wo + wi).normalised();
            mat.vndf_local(a_sq, wm, wo) / (4.0 * wo.dot(wm))
        };

        log_info(name, format!("wo: {wo}"));

        sample_image(sample, SAMPLES, name);

        let sum = integrate_pdf(pdf, wo, name);

        log_info(name, format!("sum: {sum}"));
        assert!((sum - 1.0).abs() < PDF_EPS, "sum = {sum}");
    }

    // int NDF * cos theta = 1
    // i.e. projected area = 1
    #[test]
    fn ndf_area() {
        init_test();
        let mut rng = rand::rng();
        let rand = Vec3::splat(MinRng::random(&mut rng).max(0.0001));
        let a = rand.y;
        let a_sq = a.powi(2);

        let name = "ggx_ndf_area";
        let mat = RoughConductor::new_raw(rand, Vec3::ONE);

        let pdf = |_: Vec3, wm: Vec3| -> f32 { mat.ndf_local(a_sq, wm) * wm.z };

        log_info("ggx_ndf_area", format!("alpha: {a}"));

        let sum = integrate_pdf(pdf, Vec3::ZERO, name);

        log_info(name, format!("sum: {sum}"));
        assert!((sum - 1.0).abs() < PDF_EPS, "sum = {sum}");
    }

    #[test]
    fn weak_white_furnace() {
        init_test();
        let mut rng = rand::rng();
        let rand = Vec3::splat(MinRng::random(&mut rng).max(0.0001));
        let a = rand.y;
        let a_sq = a.powi(2);

        let wo = generate_wo(&mut rng, true);

        let name = "weak_white_furnace";
        let mat = RoughConductor::new_raw(rand, Vec3::ONE);

        let pdf = |wo: Vec3, wi: Vec3| -> f32 {
            let wm = (wo + wi).normalised();
            mat.ndf_local(a_sq, wm) * mat.g1_local(a_sq, wo, wm) / (4.0 * wo.z.abs())
        };

        log_info("weak_white_furnace", format!("alpha: {a}"));

        let sum = integrate_pdf(pdf, wo, name);

        log_info(name, format!("sum: {sum}"));
        assert!((sum - 1.0).abs() < PDF_EPS, "sum = {sum}");
    }

    // uniform hemisphere/sphere sampling
    // pointing away from surface
    fn generate_wo(rng: &mut impl MinRng, hemi: bool) -> Vec3 {
        let cos_theta: f32 = if hemi {
            MinRng::random(rng)
        } else {
            rng.random_range(-1.0..1.0)
        };
        let sin_theta = (1.0 - cos_theta.powi(2)).sqrt();

        let phi = TAU * MinRng::random(rng);

        Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta)
    }

    fn vector_to_idx(v: Vec3) -> usize {
        let theta = (v.z.acos() / std::f32::consts::PI) * THETA_BINS as f32;
        let theta = (theta as usize).min(THETA_BINS - 1);

        let phi = v.y.atan2(v.x);
        let phi = (if phi < 0.0 { phi + TAU } else { phi } / TAU) * PHI_BINS as f32;
        let phi = (phi as usize).min(PHI_BINS - 1);

        theta * PHI_BINS + phi
    }

    fn sample_image<F: FnMut() -> Vec3>(mut sample_generator: F, samples: usize, name: &str) {
        let mut image = vec![0; BINS];
        let mut max_count = 0;
        for _ in 0..samples {
            let sampled_dir = sample_generator();
            let idx = vector_to_idx(sampled_dir);
            image[idx] += 1;
            max_count = max_count.max(image[idx]);
        }

        normalise_and_send(image, format!("{name}:sampled"), max_count as f64);
    }

    fn normalise_and_send<T: Into<f64>>(data: Vec<T>, name: String, max_val: f64) {
        let img: Vec<_> = data
            .into_iter()
            .map(|v| (v.into() * 256.0 / max_val) as u8)
            .collect();

        image::save_buffer(
            format!("{name}.png"),
            &img,
            PHI_BINS as u32,
            THETA_BINS as u32,
            image::ColorType::L8,
        )
        .unwrap()
    }

    fn integrate_pdf<F: Fn(Vec3, Vec3) -> f32>(pdf: F, wo: Vec3, name: &str) -> f64 {
        let mut image = vec![0.0; BINS];
        let mut max_val = 0.0f64;
        let func = |wi: Vec3| pdf(wo, wi) as f64;
        for idx in 0..BINS {
            let (phi_bin, theta_bin) = (idx % PHI_BINS, idx / PHI_BINS);

            use std::f64::consts;
            let phi = consts::TAU * phi_bin as f64 / PHI_BINS as f64;
            let phi_upper = consts::TAU * (phi_bin + 1) as f64 / PHI_BINS as f64;
            let theta = consts::PI * theta_bin as f64 / THETA_BINS as f64;
            let theta_upper = consts::PI * (theta_bin + 1) as f64 / THETA_BINS as f64;

            image[idx] = integrate_solid_angle(&func, (phi, phi_upper), (theta, theta_upper));
            max_val = max_val.max(image[idx]);
        }
        let sum = image.iter().sum();
        normalise_and_send(image, format!("{name}:pdf"), max_val);
        sum
    }

    fn integrate_solid_angle<F: Fn(Vec3) -> f64>(
        pdf: &F,
        phi_bounds: (f64, f64),
        theta_bounds: (f64, f64),
    ) -> f64 {
        let eval_spherical = |phi: f64, theta: f64| {
            let sin_theta = theta.sin();
            let w = Vec3::new(
                (sin_theta * phi.cos()) as f32,
                (sin_theta * phi.sin()) as f32,
                theta.cos() as f32,
            );
            pdf(w) * sin_theta
        };

        // 61 point Gauss-Kronrod
        use rgsl::integration::qk61;

        let outer_func = |phi: f64| {
            let inner_func = |theta: f64| eval_spherical(phi, theta) as f64;
            qk61(inner_func, theta_bounds.0, theta_bounds.1).0
        };
        qk61(outer_func, phi_bounds.0, phi_bounds.1).0
    }
}
