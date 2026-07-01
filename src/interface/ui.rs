//! 
//! Main UI file:
//! 
//!     Contains the layout, UI update logic and the main logic loop.
//! 


#[cfg(not(target_arch = "wasm32"))]
use crate::{ filesystem, interface::rom::RecentRoms };

use eframe::egui::{self, Align2, Key};
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};
use std::{path::PathBuf, sync::mpsc::{self, Receiver, Sender}};

use crate::{
    emu::{audio::EnabledChannels, emulator::Emulator, memory::header::Header, rendering::{palette::{PALETTES, RawPalette, UIPalette}}}, interface::{
        fps::FPSCounter, keys::StorageKey, renderer::{Framebuffer, ScreenIntegerScale, ShadeBuffer}, rom::{BootRom, Rom}, theme::{self, ui_theme::THEMES}, views::{debug::DebugView, header::HeaderView, tile::TileView},
    }, shaders::pipeline::ShaderState
};


/// HZ in M-cycles
const CPU_HZ: f32 = 1_048_576.0;


pub struct Ui {
    // State
        /// Pause flag
        pub pause: bool,
        /// Boot rom file
        pub boot_rom: Option<BootRom>,
        /// Skip boot ROM
        pub skip_boot: bool,
    // Rendering
        /// Render palette id
        pub palette_id: usize,
        /// Frame buffer - a flat [R,G,B...] vector
        pub framebuffer: Framebuffer,
        /// Shade buffer - a framebuffer made of shades
        pub shade_buffer: ShadeBuffer,
        /// Main LCD screen texture
        pub screen_texture: egui::TextureHandle,
    // ROM
        /// ROM header
        pub rom_header: Option<Header>,
        /// Recent ROMs list
        #[cfg(not(target_arch = "wasm32"))]
        pub recent_roms: RecentRoms,
        /// File sender
        pub file_tx: Sender<(String, PathBuf, Vec<u8>)>,
        /// File receiver
        pub file_rx: Receiver<(String, PathBuf, Vec<u8>)>,
    // UI
        /// UI theme id
        pub theme_id: usize,
        /// Fullscreen flag
        pub fullscreen: bool,
        /// Header view
        pub header_view: HeaderView,
        /// Tile view
        pub tile_view: TileView,
        /// Debug view
        pub debug_view: DebugView,
        /// Toasts
        pub toasts: Toasts,
        /// Screen scale value
        pub scale: ScreenIntegerScale,
        /// Maximum currently available scale
        pub available_max_scale: ScreenIntegerScale,
        /// Enabled audio channels
        pub audio_channels: EnabledChannels,
        /// Emulator fame counter
        pub frame_counter: FPSCounter,
    // Shader pipeline
        /// Shader resources
        pub shader_state: ShaderState,
}
impl Ui {
    pub fn new(cc: &eframe::CreationContext, screen_texture: egui::TextureHandle) -> Self {
        // Boot ROM
        let boot_rom = cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::BootRom.to_string()
            )).unwrap_or(None);

        // Skip boot
        let skip_boot = cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::SkipBoot.to_string()
            )).unwrap_or(false);

        // Palette ID
        let palette_id = cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::PaletteId.to_string()
            )).unwrap_or(0).clamp(0, PALETTES.len()-1);

        // ROM stuff
        #[cfg(not(target_arch = "wasm32"))]
        let recent_roms = RecentRoms::new(cc);
        let (file_tx, file_rx) = mpsc::channel();

        // Theme
        let theme_id = cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::ThemeId.to_string()
            )).unwrap_or(0).clamp(0, THEMES.len()-1);
        theme::catppuccin::set_theme(&cc.egui_ctx, THEMES[theme_id].theme); // Set theme by ID

        // Toasts
        let toasts = Toasts::new()
            .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0))
            .direction(egui::Direction::BottomUp);

        // Screen scale
        let scale = cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::Scale.to_string()
            )).unwrap_or(ScreenIntegerScale::Max);

        // Setup rendering pipeline
        let rs = cc.wgpu_render_state.as_ref().expect("WGPU rendering backend is required");
        let mut shader_state = ShaderState::new(rs);

        shader_state.add_shader_entry(include_str!("../shaders/wgsl/MonoLCD.c.wgsl"),   "LCD");
        shader_state.add_shader_entry(include_str!("../shaders/wgsl/LED.c.wgsl"),       "LED");
        shader_state.add_shader_entry(include_str!("../shaders/wgsl/CRT.c.wgsl"),       "CRT");
        shader_state.add_shader_entry(include_str!("../shaders/wgsl/Scale2x.c.wgsl"),   "Scale2x");

        shader_state.shader_id = cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::ShaderId.to_string()
            )).unwrap_or(0).clamp(0, shader_state.get_entry_len());

        Self {
            pause: false, boot_rom, skip_boot,
            palette_id,
            framebuffer: Framebuffer::default(),
            shade_buffer: ShadeBuffer::default(),
            screen_texture,
            rom_header: None, file_tx, file_rx,
            #[cfg(not(target_arch = "wasm32"))]
            recent_roms,
            theme_id, fullscreen: false,
            header_view: HeaderView::default(),
            tile_view: TileView::new(cc),
            debug_view: DebugView::default(),
            toasts, scale, available_max_scale: ScreenIntegerScale::None,
            audio_channels: EnabledChannels::default(),
            frame_counter: FPSCounter::default(),
            shader_state,
        }
    }

    // Theme stuff
    pub fn get_theme(&self) -> &'static theme::catppuccin::Theme { &THEMES[self.theme_id].theme }

    // Palette stuff
    pub fn get_palette(&self, id: usize) -> &UIPalette<'_> { &PALETTES[id] }
    pub fn get_raw_palette(&self) -> &'static RawPalette { &PALETTES[self.palette_id].palette }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_file_display_name(path: &std::path::Path) -> String {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown File".to_string())
    }

    // ROM reads
    pub fn try_read_rom(&mut self, bytes: &[u8]) -> Option<Header> { Header::new(bytes).ok() }
    pub fn try_read_boot_rom(&mut self, bytes: Vec<u8>) -> Option<BootRom> {
        if BootRom::checksum_check(&bytes) { Some(BootRom::new(bytes)) } else { None }
    }
}


impl eframe::App for Emulator {
    // Main layout
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Main layout
        if !self.ui.fullscreen { self.show_topbar(ui); } // Top bar
        self.show_content(ui); // Main view
        
        // Windows
        self.ui.header_view.window(ui, &self.ui.rom_header); // Header Viewer
        self.ui.tile_view.window(ui, &self.mb.mmu, self.ui.get_raw_palette()); // Tile Viewer
        self.ui.debug_view.window(ui, &self.mb); // Debug View

        // Additional UI
        self.ui.toasts.show(ui);
    }

    // Exit stuff
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, &StorageKey::Scale.to_string(),      &self.ui.scale);
        eframe::set_value(storage, &StorageKey::Muted.to_string(),      &self.audio.muted);
        eframe::set_value(storage, &StorageKey::Volume.to_string(),     &self.audio.volume);
        eframe::set_value(storage, &StorageKey::ThemeId.to_string(),    &self.ui.theme_id);
        eframe::set_value(storage, &StorageKey::BootRom.to_string(),    &self.ui.boot_rom);
        eframe::set_value(storage, &StorageKey::SkipBoot.to_string(),   &self.ui.skip_boot);
        eframe::set_value(storage, &StorageKey::ShaderId.to_string(),   &self.ui.shader_state.shader_id);
        eframe::set_value(storage, &StorageKey::PaletteId.to_string(),  &self.ui.palette_id);

        #[cfg(not(target_arch = "wasm32"))]
        self.ui.recent_roms.save(storage);
    }
    fn on_exit(&mut self) {
        // Stop recordings gracefully
        #[cfg(not(target_arch = "wasm32"))]
        self.recorder.exit();
    }

    // UI logic
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt);

        // Web can't do audio until the page is clicked on
        #[cfg(target_arch = "wasm32")]
        self.audio.web_check_audio(ctx);

        // Check audio health
        self.audio.update(dt);

        // ROM input
        if let Ok((name, path, bytes)) = self.ui.file_rx.try_recv() {
            try_load_rom(self, ctx, name, path, bytes);
        }

        // Handle input
        {
            // Common shortcuts
            ctx.input(|i| {
                if i.modifiers.ctrl && i.key_pressed(Key::P) {
                    self.toggle_pause();
                }
                if i.modifiers.ctrl && i.key_pressed(Key::R) {
                    self.reset(None, false);
                }
            });

            // Web doesn't support F-row shortcuts nor recording
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut fullscreen_changed = false;
                let mut title_changed = false;

                ctx.input(|i| {
                    if i.key_pressed(Key::F11) {
                        self.ui.fullscreen = !self.ui.fullscreen;
                        fullscreen_changed = true;
                    }
                    if i.key_pressed(Key::F12) {
                        self.ui.debug_view.view_params.is_open = !self.ui.debug_view.view_params.is_open;
                    }
                });

                if self.get_rom().is_some() {
                    ctx.input(|i| {
                        if i.key_pressed(Key::R) {
                            let _ = self.recorder.toggle();
                            title_changed = true;
                        }
                    });
                }

                if title_changed { self.set_title(ctx); }
                if fullscreen_changed { ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.ui.fullscreen)); }
            }

            // Web input bindings
            #[cfg(target_arch = "wasm32")]
            {
                let mut fullscreen_changed = false;

                ctx.input(|input| {
                    if input.key_pressed(Key::Escape) {
                        if self.ui.fullscreen {
                            self.ui.fullscreen = false;
                            fullscreen_changed = true;
                        }
                    }
                });

                if fullscreen_changed { ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.ui.fullscreen)); }
            }
        }

        if !self.is_running() && self.get_rom().is_none() {
            self.buffer_idle_frame()
        }

        self.ui.frame_counter.update(); // Calculate FPS
        ctx.request_repaint(); // Request next frame
    }

    // Main loop logic
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt);

        // Main loop
        if self.is_running() {
            self.mb.set_joypad(self.input.process(ctx, dt)); // Handle input
            
            // Emulate M-cycles
            let mut m_cycles = (dt * CPU_HZ) as u32;
            while m_cycles > 0 {
                while (m_cycles > 0) && (!self.mb.ppu.finished) { 
                    let samples = self.mb.tick4(&mut self.ui.shade_buffer);
                    self.audio.mix_append(&self.mb.mmu, &self.ui.audio_channels, samples);
                    m_cycles -= 1;
                }
                
                // Do end frame stuff:
                if self.mb.ppu.finished {
                    self.audio.on_end_frame();          // Process audio
                    self.ui.write_shade_to_frame();     // Write shade buffer to framebuffer
                    self.ui.write_buffer_to_texture();  // Write framebuffer to screen texture
                    self.ui.frame_counter.increment();  // Increment frame counter
                    self.mb.ppu.finished = false;       // Reset finished flag
                    #[cfg(not(target_arch = "wasm32"))]
                    self.write_recording();             // Write to disc
                }
            }
        } else {
            self.ui.write_shade_to_frame();
            self.ui.write_buffer_to_texture();
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.recorder.clear_finished_jobs(); // Clear handles
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn try_load_rom_from_path(emu: &mut Emulator, ctx: &egui::Context, name: String, path: PathBuf) {
    let rom_file = filesystem::read_file(&path);
    if let Ok(bytes) = rom_file { try_load_rom(emu, ctx, name, path, bytes) }
    else { try_load_rom(emu, ctx, "".to_string(), path, vec![])}
}

pub fn try_load_rom(emu: &mut Emulator, ctx: &egui::Context, name: String, path: PathBuf, bytes: Vec<u8>) {
    let mut read_result = None;
    if !bytes.is_empty() { read_result = emu.ui.try_read_rom(&bytes); }
    
    match read_result {
        Some(header) => {
            
            // Show a Success toast
            emu.ui.toasts.add(Toast {
                text: format!("Loaded: {}", name).into(),
                kind: ToastKind::Success,
                options: ToastOptions::default()
                    .duration_in_seconds(5.0)
                    .show_progress(true),
                ..Default::default()
            });

            let rom = Rom::new(path, bytes);

            #[cfg(not(target_arch = "wasm32"))]
            emu.ui.recent_roms.add(rom.path.clone());   // Add to recent

            emu.load_rom(rom, header);                  // Actually load the ROM
            emu.set_title(ctx);                         // Change the title

        },
        None => {

            let mut read_result = None;
            if !bytes.is_empty() { read_result = emu.ui.try_read_boot_rom(bytes); }
        
            match read_result {
                Some(rom) => {
                    // Show a Success toast
                    emu.ui.toasts.add(Toast {
                        text: "Successfully added a boot ROM".into(),
                        kind: ToastKind::Success,
                        options: ToastOptions::default()
                            .duration_in_seconds(5.0)
                            .show_progress(true),
                        ..Default::default()
                    });

                    emu.ui.boot_rom = Some(rom);
                },
                None => {
                    // Show an Error toast
                    emu.ui.toasts.add(Toast {
                        text: format!("Failed to load: {}", name).into(),
                        kind: ToastKind::Error,
                        options: ToastOptions::default()
                            .duration_in_seconds(5.0)
                            .show_progress(true),
                        ..Default::default()
                    });
                },
            }

        },
    }
}