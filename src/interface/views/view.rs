use eframe::egui::{self, Window};

use crate::interface::helper::Helper;


#[derive(Default, Clone)]
pub struct ViewParams {
    /// View title
    pub title: String,
    /// Open shortcut
    pub shortcut: String,
    /// Open flag
    pub is_open: bool,
    /// Standalone window flag
    pub immediate: bool,
    /// Standalone window initialized flag
    pub window_initialized: bool,
    /// Standalone window content size
    #[allow(dead_code)]
    pub rect_size: egui::Vec2,
}
impl ViewParams {
    pub fn new(title: String, shortcut: String) -> Self {
        Self { title, shortcut, ..Default::default() }
    }

    pub fn window_toggle(&mut self, ui: &mut egui::Ui) {
        let response = 
            if self.shortcut.is_empty() {  Helper::selectable_button(ui, self.is_open, &self.title, "❌") }
            else { Helper::shortcut_selectable_button(ui, self.is_open, &self.title, &self.shortcut, "❌") };

        if response.clicked() {
            self.immediate = ui.input(|i| i.modifiers.shift);
            self.is_open = !self.is_open;
            if !self.is_open { self.window_initialized = false; }
        }
    }

    pub fn show_window(&mut self, ui: &mut egui::Ui, add_contents: impl FnMut(&mut egui::Ui)) {
        let open_snapshot: bool = self.is_open;

        if self.is_open {

            #[cfg(not(target_arch = "wasm32"))]
            if self.immediate { self.immediate_window(ui, add_contents); } 
            else { self.window(ui, add_contents); }

            #[cfg(target_arch = "wasm32")]
            self.window(ui, add_contents);

        }

        if open_snapshot != self.is_open && !self.is_open { self.window_initialized = false; }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn immediate_window(&mut self, ui: &mut egui::Ui, mut add_contents: impl FnMut(&mut egui::Ui)) {
        ui.show_viewport_immediate(
            egui::ViewportId::from_hash_of(self.title.replace(" ", "_")),
            egui::ViewportBuilder::default()
                .with_title(&self.title)
                .with_resizable(false)
                .with_visible(false),
            |ui, _class| {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    let content_response = egui::Frame::NONE
                        .show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            add_contents(ui);
                        });

                    let content_size = content_response.response.rect.size();
                    let final_size = (content_size + egui::vec2(24.0, 24.0)).max(egui::Vec2 { x: 230.0, y: 20.0 });

                    if !self.window_initialized || self.rect_size != final_size {
                        ui.send_viewport_cmd(egui::ViewportCommand::InnerSize(final_size));
                        if !self.window_initialized { ui.send_viewport_cmd(egui::ViewportCommand::Visible(true)); }
                        self.window_initialized = true;
                    }
                });

                if ui.input(|i| i.viewport().close_requested()) { self.is_open = false; }
            }
        );
    }

    fn window(&mut self, ui: &mut egui::Ui, mut add_contents: impl FnMut(&mut egui::Ui)) {
        Window::new(&self.title)
            .open(&mut self.is_open)
            .auto_sized()
            .show(ui.ctx(), |ui| {
                add_contents(ui);
            });
    }
}

