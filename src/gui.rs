use crate::prelude::*;
use crate::App;
use rayon::prelude::*;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let rs = &mut self.render_settings;
        // -----------------------------------------------
        // Handle updates from work handling threads and compute threads
        // Note that Update::Calculation does not present directly to the GUI
        // This is for performance reasons
        // -----------------------------------------------
        while let Ok(update) = self.update_recv.try_recv() {
            match update {
                Update::Calculation(splats, _thread_id, ray_count) => {
                    self.splats_done += splats.len() as u64;
                    if self.splats_done == rs.width as u64 * rs.height as u64 * rs.samples {
                        println!(
                            "Mrays: {:.2} - Rays shot: {} - elapsed: {:.1}",
                            (self.work_rays as f64 / self.work_start.elapsed().as_secs_f64())
                                / 1000000 as f64,
                            self.work_rays,
                            self.work_start.elapsed().as_secs_f64()
                        );
                    }
                    // add splats to image
                    for splat in splats {
                        let uv = splat.uv;
                        let idx = {
                            assert!(uv[0] <= 1.0 && uv[1] <= 1.0);

                            let x = (uv[0] * rs.width as f32) as usize;
                            let y = (uv[1] * rs.height as f32) as usize;

                            (y * rs.width as usize + x)
                                .min(rs.width as usize * rs.height as usize - 1)
                        };

                        self.canvas[idx] += splat.rgb;
                        self.updated = true;
                    }
                    self.work_rays += ray_count;
                }
                Update::WorkQueueCleared => log::info!("Work queue cleared!"),
                Update::PssmltBootstrapDone => log::info!("PSSMLT bootstrap done!"),
                Update::NoState => log::info!("No state found!"),
            }
        }

        // -----------------------------------------------
        // Present framebufferto GUI @ 2Hz if there has been an update
        // This is limited to 2Hz as there is a non trivial amount of overhead
        // -----------------------------------------------
        if self.updated && self.last_update.elapsed() > std::time::Duration::from_millis(500) {
            // update texture
            let mult = ((rs.width * rs.height) as f64 / self.splats_done as f64) as f32;
            let buf = self
                .canvas
                .par_iter()
                .map(|rgb| {
                    // scale based on samples
                    let rgb = *rgb * mult;

                    // gamma correction
                    let r = rgb.x.powf(1.0 / 2.2);
                    let g = rgb.y.powf(1.0 / 2.2);
                    let b = rgb.z.powf(1.0 / 2.2);

                    let r = (r * 255.0) as u8;
                    let g = (g * 255.0) as u8;
                    let b = (b * 255.0) as u8;

                    egui::Color32::from_rgb(r, g, b)
                })
                .collect();

            let raw_buf = egui::ColorImage {
                size: [rs.width as usize, rs.height as usize],
                pixels: buf,
            };
            self.fb_tex_handle
                .set(raw_buf, egui::TextureOptions::default());
            self.context.request_repaint();
            self.updated = false;
            self.last_update = std::time::Instant::now();
        }

        // -----------------------------------------------
        // Draw GUI
        // -----------------------------------------------
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.menu_button("File", |ui| ui.button("Export Camera"));
            if ui.button("Start Render").clicked() {
                rs.samples = 1000;
                self.work_req
                    .send(ComputeChange::WorkSamples(rs.samples))
                    .unwrap();
                self.work_rays = 0;
                self.work_start = std::time::Instant::now();
            }
            if ui.button("Show render settings").clicked() {
                self.display_settings = true;
            }
            ui.label(format!(
                "Mrays: {:.2} - Rays shot: {} - elapsed: {:.1}",
                (self.work_rays as f64 / self.work_start.elapsed().as_secs_f64()) / 1000000 as f64,
                self.work_rays,
                self.work_start.elapsed().as_secs_f64()
            ));
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let size = self.fb_tex_handle.size_vec2();
            let sized_tex = egui::load::SizedTexture::new(&self.fb_tex_handle, size);
            ui.add(egui::Image::new(sized_tex).shrink_to_fit().max_size(size));
        });
        egui::Window::new("Render Settings")
            .open(&mut self.display_settings)
            .show(ctx, |ui| {
                ui.label(format!("width: {}", rs.width));
                ui.label(format!("height: {}", rs.height));
            });
    }
}
