use crate::App;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.menu_button("File", |ui| ui.button("Export Camera"));
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let size = self.fb_tex_handle.size_vec2();
            let sized_tex = egui::load::SizedTexture::new(&self.fb_tex_handle, size);
            ui.add(egui::Image::new(sized_tex).shrink_to_fit().max_size(size));
            self.poll_state();
        });
    }
}
