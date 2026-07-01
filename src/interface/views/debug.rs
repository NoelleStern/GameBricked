//! 
//! Emulator debug window
//! 


use smart_default::SmartDefault;
use eframe::egui::{self, Color32, RichText};

use crate::{emu::{motherboard::MotherBoard, processor::registers::FlagsRegister}, interface::views::view::ViewParams};


#[derive(SmartDefault)]
pub struct DebugView {
    #[cfg(not(target_arch = "wasm32"))]
    #[default(ViewParams::new("Debug View".to_string(), "F12".to_string()))]
    pub view_params: ViewParams,

    #[cfg(target_arch = "wasm32")]
    #[default(ViewParams::new("Debug View".to_string(), "".to_string()))]
    pub view_params: ViewParams,
}
impl DebugView {
    pub fn window(&mut self, ui: &mut egui::Ui, mb: &MotherBoard) {
        self.view_params.show_window(ui, |ui| {
            Self::show(ui, mb);
        });
    }

    fn show(ui: &mut egui::Ui, mb: &MotherBoard) {
        let r = &mb.cpu.registers;
        Self::register_label(ui, "AF", r.a, r.f.into());
        Self::register_label(ui, "BC", r.b, r.c);
        Self::register_label(ui, "DE", r.d, r.e);
        Self::register_label(ui, "HL", r.h, r.l);
        Self::flags_label(ui, r.f);
    }

    fn register_label(ui: &mut eframe::egui::Ui, label: &str, r1: u8, r2: u8) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new(
                format!("{} ", label)
            ).monospace());
            ui.label(RichText::new(
                format!("{:02X} {:02X}", r1, r2)
            ).monospace().color(Color32::RED));
        });
    }

    fn flags_label(ui: &mut eframe::egui::Ui, f: FlagsRegister) {
        ui.label(RichText::new(
            format!(
                "{} {} {} {}",
                if f.zero       { "Z" } else { "_" },
                if f.subtract   { "N" } else { "_" },
                if f.half_carry { "H" } else { "_" },
                if f.carry      { "C" } else { "_" }
            )
        ).color(Color32::CYAN).monospace());
    }   
}