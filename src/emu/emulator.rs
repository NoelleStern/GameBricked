//! 
//! Main emulator file:
//! 
//!     Ties the motherboard, UI and I/O together.
//!


use eframe::egui;
use color_eyre::eyre;

use crate::emu::audio::Audio;
use crate::emu::memory::header::Header;
use crate::interface::ui::Ui;
use crate::interface::rom::Rom;
use crate::interface::input::Input;
use crate::emu::motherboard::MotherBoard; 


pub struct Emulator {
    /// User Interface
    pub ui: Ui,
    /// Emulated motherboard
    pub mb: MotherBoard,
    /// User input handler
    pub input: Input,
    /// Audio output handler
    pub audio: Audio,
    /// FFmpeg video recorder
    #[cfg(not(target_arch = "wasm32"))]
    pub recorder: crate::emu::ffmpeg::Recorder,
}
impl Emulator {
    pub fn new(cc: &eframe::CreationContext<'_>, screen_texture: egui::TextureHandle) -> eyre::Result<Self> {
        let ui: Ui = Ui::new(cc, screen_texture);                       // UI
        let mb: MotherBoard = MotherBoard::init(ui.boot_rom.clone());   // Motherboard
        let input: Input = Input::new();                                // Initialize input
        let audio: Audio = Audio::new(cc);                              // Initialize the audio

        Ok(
            Self {
                ui, mb, input, audio,
                #[cfg(not(target_arch = "wasm32"))]
                recorder: crate::emu::ffmpeg::Recorder::default()
            }
        )
    }

    /// Reset the emulator state
    pub fn reset(&mut self, rom: Option<Rom>, reset_rom: bool) {
        self.mb.reset(self.ui.boot_rom.clone(), rom, reset_rom); // Reset the MB
        self.audio.reset(); // Always reset audio too
        if self.ui.boot_rom.is_none() || self.ui.skip_boot { self.skip_boot(); }
    }

    // ROM stuff
    pub fn get_rom(&self) -> &Option<Rom> { &self.mb.mmu.cart.rom }
    pub fn load_rom(&mut self, rom: Rom, header: Header) {
        self.ui.rom_header = Some(header); // Set header
        self.reset(Some(rom), false);
    }

    /// Toggle the emulator execution
    pub fn toggle_pause(&mut self) {
        self.ui.pause = !self.ui.pause;
        self.ui.frame_counter.reset();
    } 

    // Boot skip
    pub fn skip_boot(&mut self) {
        // Taken straight from the GameBoy Doctor:
        self.mb.mmu.raw_write8(0xFF50, 1);
        self.mb.cpu.registers.a = 0x01;
        self.mb.cpu.registers.a = 0x01;
        self.mb.cpu.registers.f = 0xB0.into();
        self.mb.cpu.registers.b = 0x00;
        self.mb.cpu.registers.c = 0x13;
        self.mb.cpu.registers.d = 0x00;
        self.mb.cpu.registers.e = 0xD8;
        self.mb.cpu.registers.h = 0x01;
        self.mb.cpu.registers.l = 0x4D;
        self.mb.cpu.sp = 0xFFFE;
        self.mb.cpu.pc = 0x0100;
        // More advanced stuff
        self.mb.mmu.raw_write8(0xFF40, 0x91); // Set LCD
        self.mb.mmu.raw_write8(0xFF47, 0xFC); // Set background palette
    }

    /// Is the game currently running?
    pub fn is_running(&self) -> bool {
        !self.ui.pause && self.get_rom().is_some()
    }

    // Recording stuff
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write_recording(&mut self) {
        if let Some(recorder) = self.recorder.active.as_mut() {
            recorder.write_video_buffer(&self.ui.framebuffer.buffer);
            recorder.write_audio_samples(&self.audio.combined_samples);
        }
    }
}