//! 
//! Header view window
//! 


use smart_default::SmartDefault;
use eframe::egui::{self, RichText};

use crate::{emu::memory::header::Header, interface::views::view::ViewParams};


#[derive(SmartDefault)]
pub struct HeaderView {
    #[default(ViewParams::new("Header View".to_string(), "".to_string()))]
    pub view_params: ViewParams
}
impl HeaderView {
    pub fn window(&mut self, ui: &mut egui::Ui, header: &Option<Header>) {
        self.view_params.show_window(ui, |ui| {
            Self::show(ui, header);
        });
    }

    fn show(ui: &mut egui::Ui, header: &Option<Header>) {
        if let Some(h) = &header {
            ui.vertical(|ui| {
                let mut logo_line_counter = 0;
                for line in h.printable_info().lines() {
                    if logo_line_counter > 0 { ui.spacing_mut().item_spacing.y = -3.0; } else { ui.spacing_mut().item_spacing.y = 0.0; }
                    if !line.is_empty() { ui.label(RichText::new(line).monospace()); }
                    if line.starts_with("Logo:") { logo_line_counter = 8; }
                    logo_line_counter -= 1;
                }
            });                    
        } else {
            ui.label(
                RichText::new(
                    "No ROM loaded"
                ).monospace()
            );
        }
    }
    
}