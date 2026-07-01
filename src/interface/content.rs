//! 
//! Main emulator view:
//! 
//!     Calculates scale and renders either as is or using a shader.
//! 
//!     Scale values:
//!         ScreenScale::None               - occupies maximum available space
//!         ScreenScale::X1-ScreenScale::X8 - scales to the respective integer
//!         ScreenScale::Max                - scales to maximum available integer
//! 


use eframe::{egui::{self, Direction, Image, Layout}, egui_wgpu};

use crate::{emu::{emulator::Emulator, rendering::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH}}, interface::renderer::ScreenIntegerScale};


impl Emulator {
    pub fn show_content(&mut self, ui: &mut egui::Ui) {
        let minimal_content_frame = egui::Frame::menu(ui.style())
            .corner_radius(0.0).inner_margin(0.0).stroke(egui::Stroke::NONE);
        
        egui::CentralPanel::default().frame(minimal_content_frame).show_inside(ui, |ui| {

            let centering_layout = Layout::centered_and_justified(Direction::TopDown);
            ui.allocate_ui_with_layout(ui.available_size(), centering_layout, |ui| {

                // Set min size
                let s = if self.ui.scale == ScreenIntegerScale::Max { 1 } else { (self.ui.scale as usize).max(1) };
                ui.set_min_width((SCREEN_WIDTH*s) as f32); ui.set_min_height((SCREEN_HEIGHT*s) as f32);

                // Calculate max scale
                let available = ui.available_size();
                let tex_size = egui::vec2(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32);
                let scale_x = available.x / tex_size.x; let scale_y = available.y / tex_size.y;
                self.ui.available_max_scale = (scale_x.min(scale_y).floor().min(8.0) as usize).into();

                // Calculate actual scale
                let scale = if self.ui.scale != ScreenIntegerScale::None {
                        if self.ui.scale == ScreenIntegerScale::Max { scale_x.min(scale_y).floor() as f32}
                        else { self.ui.scale as u32 as f32 }
                    } else { scale_x.min(scale_y) };

                match self.ui.shader_state.shader_id {
                    0 => {

                        // Draw image texture
                        ui.add(Image::from_texture(&self.ui.screen_texture)
                            .shrink_to_fit().max_size(tex_size*scale)
                        );

                    },
                    _ => {

                        let size = contain(available, SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32).min(tex_size*scale);
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

                        // Update shader resources
                        self.ui.shader_state.update(
                            ui, rect,
                            &egui::ColorImage::from_rgb(
                                [SCREEN_WIDTH, SCREEN_HEIGHT],
                                &self.ui.framebuffer.buffer
                            )
                        );

                        // Render via shader
                        ui.painter().add(
                            egui_wgpu::Callback::new_paint_callback(
                                rect, self.ui.shader_state.get_shader_callback()
                            )
                        );

                    }
                }
            });

        });
    }
}

fn contain(available: egui::Vec2, aspect: f32) -> egui::Vec2 {
    let avail_aspect = available.x / available.y;
    if avail_aspect > aspect { 
        egui::vec2(available.y * aspect, available.y)
    } else { egui::vec2(available.x, available.x / aspect) }
}