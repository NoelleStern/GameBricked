//! 
//! Renders final image to a framebuffer and also a texture
//! 


use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::{emu::{emulator::Emulator, rendering::{palette::RawPalette, ppu::{RawShade, SCREEN_HEIGHT, SCREEN_WIDTH}}}, interface::ui::Ui};


/// Hallo, it's a me!
const DEFAULT_IMAGE: &[u8; SCREEN_WIDTH*SCREEN_HEIGHT] = include_bytes!("../../assets/graphics/Noelle.bin");


/// Represents possible LCD texture scale
#[derive(Serialize, Deserialize)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum ScreenIntegerScale {
    /// Scale to max available
    #[default]
    None = 0,
    /// 1x scale
    X1 = 1,
    /// 2x scale
    X2 = 2,
    /// 3x scale
    X3 = 3,
    /// 4x scale
    X4 = 4,
    /// 5x scale
    X5 = 5,
    /// 6x scale
    X6 = 6,
    /// 7x scale
    X7 = 7,
    /// 8x scale
    X8 = 8,
    /// Scale to max available integer
    Max
}
impl From<usize> for ScreenIntegerScale {
    fn from(val: usize) -> Self {
        match val {
            1 => ScreenIntegerScale::X1,    2 => ScreenIntegerScale::X2,
            3 => ScreenIntegerScale::X3,    4 => ScreenIntegerScale::X4,
            5 => ScreenIntegerScale::X5,    6 => ScreenIntegerScale::X6,
            7 => ScreenIntegerScale::X7,    8 => ScreenIntegerScale::X8,
            0 => ScreenIntegerScale::None,  _ => ScreenIntegerScale::Max
        }
    }
}

pub struct ShadeBuffer {
    pub buffer: Vec<RawShade>
}
impl Default for ShadeBuffer {
    fn default() -> Self {
        Self { buffer: vec![RawShade::Zero; SCREEN_WIDTH*SCREEN_HEIGHT] }
    }
}
impl ShadeBuffer {
    pub fn set_shade<T: Into<usize>>(&mut self, x: T, y: T, shade: RawShade) {
        let index = (y.into() * SCREEN_WIDTH) + x.into();
        self.buffer[index] = shade;
    }
    pub fn get_shade(palette_byte: u8, color_index: RawShade) -> RawShade {
        ((palette_byte >> ((color_index as u8) * 2)) & 0x03).into()
    }
}

pub struct Framebuffer {
    /// A flat [R,G,B...] vector
    pub buffer: Vec<u8>
}
impl Default for Framebuffer {
    fn default() -> Self {
        Self { buffer: vec![0u8; SCREEN_WIDTH*SCREEN_HEIGHT*3] } // * 3 for R, G and B
    }
}
impl Framebuffer {
    #[allow(dead_code)]
    pub fn set_rgb<T: Into<usize>>(&mut self, x: T, y: T, shade: RawShade, palette: &'static RawPalette) {
        let index = (y.into() * SCREEN_WIDTH) + x.into();
        self.set_rgb_by_index(index, shade, palette)
    }
    pub fn set_rgb_by_index(&mut self, index: usize, shade: RawShade, palette: &'static RawPalette) {
        let i = index * 3;
        let rgb = palette.match_shade(shade);
        self.buffer[i] =   rgb[0];
        self.buffer[i+1] = rgb[1];
        self.buffer[i+2] = rgb[2];
    }
}

/// | 7 -------------- | 6 ------------- | 5 ----------- | 4 --------------- | 3 --------- | 2 ------ | 1 -------- | 0 --------------------------- |
/// | LCD & PPU enable | Window tile map | Window enable | BG & Window tiles | BG tile map | OBJ size |	OBJ enable | BG & Window enable / priority |
#[derive(Default, Debug)]
#[allow(clippy::tabs_in_doc_comments)]
pub struct LCDControl {
    /// LCD & PPU enabled
    /// 0: off / 1: on
    pub enabled: bool,
    /// Window tile map area
    /// 0: 0x9800–0x9BFF / 1: 0x9C00–0x9FFF
    pub window_tile_map: bool,
    /// Window enabled
    /// 0: off / 1: on
    pub window_enabled: bool,
    /// BG & Window tile data area
    /// 0: 0x8800–0x97FF / 1: 0x8000–0x8FFF
    pub tile_data_area: bool,
    /// BG tile map area
    /// 0: 0x9800–0x9BFF / 1: 0x9C00–0x9FFF
    pub bg_tile_map: bool,
    /// Sprite size
    /// 0: 8×8 / 1: 8×16
    pub sprite_size: bool,
    /// Sprite enabled
    /// 0: off / 1: on
    pub sprite_enabled: bool,
    /// BG & Window enabled / priority (Different meaning in CGB Mode)
    /// 0: off / 1: on
    pub priority: bool
}
impl LCDControl {
    pub fn get_sprite_height(&self) -> usize {
        if !self.sprite_size { 8 } else { 16 }
    }
}
impl From<LCDControl> for u8 {
    fn from(value: LCDControl) -> u8 {
        (value.enabled as u8)           << 7 |
        (value.window_tile_map as u8)   << 6 |
        (value.window_enabled as u8)    << 5 |
        (value.tile_data_area as u8)    << 4 |
        (value.bg_tile_map as u8)       << 3 |
        (value.sprite_size as u8)       << 2 |
        (value.sprite_enabled as u8)    << 1 |
        (value.priority as u8)
    }
}
impl From<u8> for LCDControl {
    fn from(byte: u8) -> Self {
        let enabled         = ((byte >> 7) & 1) != 0;
        let window_tile_map = ((byte >> 6) & 1) != 0;
        let window_enabled  = ((byte >> 5) & 1) != 0;
        let tile_data_area  = ((byte >> 4) & 1) != 0;
        let bg_tile_map     = ((byte >> 3) & 1) != 0;
        let sprite_size     = ((byte >> 2) & 1) != 0;
        let sprite_enabled  = ((byte >> 1) & 1) != 0;
        let priority        = ( byte       & 1) != 0;

        LCDControl {
            enabled, window_tile_map, window_enabled,
            tile_data_area, bg_tile_map, sprite_size,
            sprite_enabled, priority
        }
    }
}

/// | 7 | 6 ------------ | 5 --------------- | 4 --------------- | 3 --------------- | 2 ------- | 10 ----- |
/// | - | LYC int select | Mode 2 int select | Mode 1 int select | Mode 0 int select | LYC == LY | PPU mode |
#[derive(Default)]
pub struct STATStatus {
    pub lyc_flag: bool,
    pub mode2_flag: bool,
    pub mode1_flag: bool,
    pub mode0_flag: bool,
    pub equals: bool,
    pub ppu_mode: u8,
}
impl STATStatus {
    pub fn new(value: u8, equals: bool, ppu_mode: u8) -> Self {
        let v: STATStatus = value.into();
        Self {
            lyc_flag: v.lyc_flag,
            mode2_flag: v.mode2_flag,
            mode1_flag: v.mode1_flag,
            mode0_flag: v.mode0_flag,
            equals, ppu_mode
        }
    }
}
impl From<STATStatus> for u8 {
    fn from(value: STATStatus) -> u8 {
        (value.lyc_flag as u8)      << 6 |
        (value.mode2_flag as u8)    << 5 |
        (value.mode1_flag as u8)    << 4 |
        (value.mode0_flag as u8)    << 3 |
        (value.equals as u8)        << 2 |
        value.ppu_mode & 0b11
    }
}
impl From<u8> for STATStatus {
    fn from(byte: u8) -> Self {
        let lyc_flag    = ((byte >> 6) & 1) != 0;
        let mode2_flag  = ((byte >> 5) & 1) != 0;
        let mode1_flag  = ((byte >> 4) & 1) != 0;
        let mode0_flag  = ((byte >> 3) & 1) != 0;
        let equals      = ((byte >> 2) & 1) != 0;
        let ppu_mode = byte & 0b11;

        STATStatus {
            lyc_flag, mode2_flag, mode1_flag, mode0_flag,
            equals, ppu_mode
        }
    }
}

#[derive(Clone, Copy)]
pub struct Window {
    pub wy: u8,
    pub wx: i16,
}
impl Window {
    pub fn new(wy: u8, wx: i16) -> Self {
        Self { wy, wx }
    }
}

impl Emulator {
    pub fn buffer_idle_frame(&mut self) {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let index = (y * SCREEN_WIDTH) + x;
                let color_index: RawShade = DEFAULT_IMAGE[index].into();
                let shade = ShadeBuffer::get_shade(0b11100100, color_index);
                self.ui.shade_buffer.set_shade(x, y, shade);
            }
        }

        self.ui.write_shade_to_frame(); // Write to a framebuffer
        self.ui.write_buffer_to_texture(); // Write to a screen texture
    }
}

impl Ui {
    pub fn write_shade_to_frame(&mut self) {
        let palette = self.get_raw_palette();
        for (i, s) in self.shade_buffer.buffer.iter().enumerate() {
            self.framebuffer.set_rgb_by_index(i, *s, palette);
        }
    }
    pub fn write_buffer_to_texture(&mut self) {
        let size = [SCREEN_WIDTH, SCREEN_HEIGHT];
        let color_image = egui::ColorImage::from_rgb(size, &self.framebuffer.buffer);
        self.screen_texture.set(color_image, egui::TextureOptions::NEAREST);
    }
}