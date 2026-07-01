//!
//! Object Attribute Memory
//! 


use crate::emu::{memory::mmu::{Mmu, OAM_BEGIN}, rendering::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH}};


pub const PRIORITY_FLAG_BYTE_POSITION: u8   = 7; // Bit 7
pub const FLIP_Y_FLAG_BYTE_POSITION: u8     = 6; // Bit 6
pub const FLIP_X_FLAG_BYTE_POSITION: u8     = 5; // Bit 5
pub const PALETTE_FLAG_BYTE_POSITION: u8    = 4; // Bit 4
                                                 // Bits 0-3 are CGB only


/// | 7 ------ | 6 ---- | 5 ---- | 4 --------- | 3 -- | 210 ------- |
/// | Priority | Y flip | X flip | DMG palette | Bank | CGB palette |
#[derive(Debug, Default, Clone, Copy)]
pub struct Attributes {
    /// Draw priority flag
    /// 0: drawn on top of everything / 1: BG and Window color indices 1–3 are drawn over this obj
    pub priority: bool,
    /// Vertical mirror flag
    /// 0: normal / 1: entire obj should be mirrored vertically
    pub flip_y: bool,
    /// Horizontal mirror flag
    /// 0: normal / 1: entire obj should be mirrored horizontally
    pub flip_x: bool,
    /// Palette ID flag
    /// 0: use sprite palette 0 / 1: use sprite palette 1
    pub palette_id: bool,
}
impl From<Attributes> for u8 {
    fn from(attr: Attributes) -> u8 {
        (attr.priority as u8)   << PRIORITY_FLAG_BYTE_POSITION |
        (attr.flip_y as u8)     << FLIP_Y_FLAG_BYTE_POSITION   |
        (attr.flip_x as u8)     << FLIP_X_FLAG_BYTE_POSITION   |
        (attr.palette_id as u8) << PALETTE_FLAG_BYTE_POSITION
    }
}
impl From<u8> for Attributes {
    fn from(byte: u8) -> Self {
        let priority    = ((byte >> PRIORITY_FLAG_BYTE_POSITION) & 1) != 0;
        let flip_y      = ((byte >> FLIP_Y_FLAG_BYTE_POSITION  ) & 1) != 0;
        let flip_x      = ((byte >> FLIP_X_FLAG_BYTE_POSITION  ) & 1) != 0;
        let palette_id  = ((byte >> PALETTE_FLAG_BYTE_POSITION ) & 1) != 0;

        Attributes { priority, flip_y, flip_x, palette_id }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OamEntry {
    /// Byte 0 — Y Position
    pub y: i16,
    /// Byte 1 — X Position
    pub x: i16,
    /// Byte 2 — Tile Index
    pub tile_id: u8,
    /// Byte 3 - Attributes
    pub attr: Attributes,
}
impl Default for OamEntry {
    fn default() -> Self {
        Self { 
            attr: Attributes::default(),
            tile_id: 0, x: -8, y: -16, // Here I adjust the coordinate values
        }
    }
}
impl OamEntry {
    pub fn update(&mut self, byte_number: usize, value: u8) {
        match byte_number {
            0 => self.y = value as i16 - 16,    // Here I adjust x coordinate value
            1 => self.x = value as i16 - 8,     // Here I adjust y coordinate value
            2 => self.tile_id = value,
            3 => self.attr = value.into(),
            _ => unreachable!()
        }
    }
    pub fn should_show(&self) -> bool {
        (self.x != -8) | (self.y != -16) |
        (self.x <= SCREEN_WIDTH as i16)  |
        (self.y <= SCREEN_HEIGHT as i16)
    }
    pub fn get_palette(&self, mmu: &Mmu) -> u8 {
        if !self.attr.palette_id { mmu.get_sprite_palette0() } else { mmu.get_sprite_palette1() }
    }
}

// [0xFE00, 0xFE9F]
impl Mmu {
    pub fn write_oam(&mut self, address: usize, value: u8) {
        self.memory[address] = value;

        let index = address - OAM_BEGIN;
        let oam_id = index / 4;
        let byte_number = index % 4;

        let oam = &mut self.oam_set[oam_id];
        oam.update(byte_number, value);
    }
}