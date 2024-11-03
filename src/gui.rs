use crate::prelude::*;
use crate::App;
use rayon::prelude::*;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut updated = false;
        while let Ok(update) = self.update_recv.try_recv() {
            match update {
                Update::Calculation(splats, _thread_id, _ray_count) => {
                    self.splats_done += splats.len() as u64;
                    // add splats to image
                    for splat in splats {
                        let uv = splat.uv;
                        let idx = {
                            assert!(uv[0] <= 1.0 && uv[1] <= 1.0);

                            let x = (uv[0] * self.args.width as f32) as usize;
                            let y = (uv[1] * self.args.height as f32) as usize;

                            (y * self.args.width as usize + x)
                                .min(self.args.width as usize * self.args.height as usize - 1)
                        };

                        self.canvas[idx] += splat.rgb;
                        updated = true;
                    }
                }
                Update::WorkQueueCleared => log::info!("Work queue cleared!"),
                Update::PssmltBootstrapDone => log::info!("PSSMLT bootstrap done!"),
                Update::NoState => log::info!("No state found!"),
            }
        }

        if updated {
            // update texture
            let mult =
                ((self.args.width * self.args.height) as f64 / self.splats_done as f64) as f32;
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
                size: [self.args.width as usize, self.args.height as usize],
                pixels: buf,
            };
            self.fb_tex_handle
                .set(raw_buf, egui::TextureOptions::default());
            self.context.request_repaint();
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.menu_button("File", |ui| ui.button("Export Camera"));
            if ui.button("Add sample").clicked() {
                self.work_req.send(ComputeChange::WorkSamples(1)).unwrap();
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let size = self.fb_tex_handle.size_vec2();
            let sized_tex = egui::load::SizedTexture::new(&self.fb_tex_handle, size);
            ui.add(egui::Image::new(sized_tex).shrink_to_fit().max_size(size));
        });
    }
}
