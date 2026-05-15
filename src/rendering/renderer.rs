use color_eyre::eyre;
use minifb::{Key, KeyRepeat, Window, WindowOptions};

use crate::{emulator::Emulator, rendering::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH}};


const MAIN_WINDOW_TITLE: &str   = "Game Bricked";   // Hallo
const TILE_VIEWER_WIDTH: usize  = 32 * 8;           // Tile viewer debug window width
const TILE_VIEWER_HEIGHT: usize = 12 * 8;           // Tile viewer debug window height


pub trait EmulatorWindow {
    fn new(emu: &Emulator) -> eyre::Result<Self> where Self: Sized;
    fn step(&mut self, emu: &mut Emulator) -> eyre::Result<()>;
    fn get_window(&mut self) -> &mut Window;
}
pub struct TileWindow {
    pub window: Window,
    pub buffer: Vec<u32>,
}
impl EmulatorWindow for TileWindow {
    fn new(_emu: &Emulator) -> eyre::Result<Self> {
        Ok(TileWindow {
            window: new_window(
                "Tile set preview",
                TILE_VIEWER_WIDTH, TILE_VIEWER_HEIGHT
            )?,
            buffer: vec![0u32; TILE_VIEWER_WIDTH*TILE_VIEWER_HEIGHT]
        })
    }

    fn step(&mut self, emu: &mut Emulator) -> eyre::Result<()> {
        for y in 0..TILE_VIEWER_HEIGHT {
            for x in 0..TILE_VIEWER_WIDTH {
                let buff_i = y * TILE_VIEWER_WIDTH + x;
                let tile_i = (y/8 * TILE_VIEWER_WIDTH/8) + x/8;
                let color = emu.cpu.mmu.ppu.tile_set[tile_i][y%8][x%8];
                self.buffer[buff_i] = emu.get_palette().match_pixel(color);
            }
        }
        self.window.update_with_buffer(&self.buffer, TILE_VIEWER_WIDTH, TILE_VIEWER_HEIGHT)?;
        Ok(())
    }
    
    fn get_window(&mut self) -> &mut Window {
        &mut self.window
    }
}
pub struct MainWindow {
    pub window: Window,
    pub buffer: Vec<u32>,
}
impl MainWindow {
    pub fn get_title(emu: &Emulator) -> String {
        format!("{}: {}", MAIN_WINDOW_TITLE, emu.header.title)
    }
}
impl EmulatorWindow for MainWindow {
    fn new(emu: &Emulator) -> eyre::Result<Self> {
        Ok(MainWindow {
            window: new_window(
                &Self::get_title(emu),
                SCREEN_WIDTH, SCREEN_HEIGHT,
            )?,
            buffer: vec![0u32; SCREEN_WIDTH*SCREEN_HEIGHT]
        })
    }

    fn step(&mut self, emu: &mut Emulator) -> eyre::Result<()> {
        // Handle input
        for key in self.window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::P => { emu.next_palette() },
                Key::R => {
                    let result = emu.toggle_ffmpeg()?;
                    let title = Self::get_title(emu);
                    if result { self.window.set_title( &format!("{} - RECORDING", title) ); }
                    else { self.window.set_title( &title ); }
                },
                _ => (),
            }
        }

        // Emulate a frame
        emu.cpu.process_frame();
        emu.cpu.get_frame(&mut self.buffer, emu.get_palette());

        // Write new frame
        emu.write_ffmpeg(&self.buffer)?;
        self.window.update_with_buffer(&self.buffer, SCREEN_WIDTH, SCREEN_HEIGHT)?;
        
        Ok(())
    }

    fn get_window(&mut self) -> &mut Window {
        &mut self.window
    }
}


fn new_window(name: &str, width: usize, height: usize) -> eyre::Result<Window> {
    let mut window = Window::new(
        name, width, height,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        }
    )?;
    window.set_target_fps(60);
    Ok(window)
}