//! 
//! App window title functions
//! 


use eframe::egui;

use crate::emu::emulator::Emulator;


/// Hallo, my name is...
pub const MAIN_WINDOW_TITLE: &str = "Game Bricked";


/// Title functions
impl Emulator {
    pub fn get_title(&self) -> String {
        let mut result = MAIN_WINDOW_TITLE.to_string();

        if let Some(header) = &self.ui.rom_header {
            result = format!("{}: {}", result, header.title);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.recorder.active.is_some() {
            result = format!("{} {}", result, "<RECORDING>");
        }

        result
    }
    pub fn set_title(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(
            egui::ViewportCommand::Title(self.get_title())
        );
    }
}