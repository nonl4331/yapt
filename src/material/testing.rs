#[cfg(test)]
mod tests {
    use rand::thread_rng;

    const THETA_BINS: usize = 180;
    const PHI_BINS: usize = 2 * THETA_BINS;
    const BINS: usize = PHI_BINS * THETA_BINS;
    const PDF_EPS: f64 = 1e-3;
    const SAMPLES: usize = 10_000_000;

    use super::super::*;

    #[test]
    pub fn lambertian() {
        crate::startup::create_logger();

        let mut rng = thread_rng();
        let wo = -generate_wo(&mut rng, true);

        let mat = Mat::Matte(Matte::new(Vec3::ZERO));

        test_material("lambertian", mat, wo, &mut rng);
    }

    #[test]
    pub fn ggx() {
        crate::startup::create_logger();

        let mut rng = thread_rng();
        let wo = -generate_wo(&mut rng, true);
        let a = rng.gen();

        let name = "ggx";
        let mat = Mat::Glossy(Ggx::new(a, Vec3::ONE));

        log_info("ggx", format!("alpha: {a}"));

        test_material(name, mat, wo, &mut rng);
    }
    fn log_info(mat: &str, info: String) {
        log::info!("{mat}: {info}");
    }

    fn test_material(name: &str, m: Mat, wo: Vec3, rng: &mut impl MinRng) {
        let sect = &Intersection::new(1.0, Vec3::ZERO, Vec3::Z, true, 0, 0);

        let sample = || -> Vec3 {
            let mut ray = Ray::new(Vec3::ZERO, wo);
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
        crate::startup::create_logger();

        let mut rng = thread_rng();
        let wo = -generate_wo(&mut rng, true);
        let a = rng.gen();

        let name = "ggx_vndf";
        let mat = Ggx::new(a, Vec3::ONE);

        log_info("ggx_vndf", format!("alpha: {a}"));

        let sample = || -> Vec3 { mat.sample_vndf_local(-wo, &mut rng) };
        let pdf = |wo: Vec3, wm: Vec3| -> f32 { mat.pdf_wm_vndf_local(-wo, wm) };

        log_info(name, format!("wo: {wo}"));

        sample_image(sample, SAMPLES, name);

        let sum = integrate_pdf(pdf, wo, name);

        log_info(name, format!("sum: {sum}"));

        assert!((sum - 1.0).abs() < PDF_EPS);
    }

    // uniform hemisphere/sphere sampling
    // pointing away from surface
    fn generate_wo(rng: &mut impl MinRng, hemi: bool) -> Vec3 {
        let cos_theta: f32 = if hemi {
            rng.gen()
        } else {
            rng.gen_range(-1.0..1.0)
        };
        let sin_theta = (1.0 - cos_theta.powi(2)).sqrt();

        let phi = TAU * rng.gen();

        Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta)
    }

    fn vector_to_idx(v: Vec3) -> usize {
        let theta = (v.z.acos() / PI) * THETA_BINS as f32;
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
