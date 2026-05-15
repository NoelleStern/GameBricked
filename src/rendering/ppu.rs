use crate::memory::mmu::{Memory, VRAM_BEGIN};


///
/// PPU - Pixel Processing Unit (GPUs weren't a thing yet).
/// 
///     GameBoy has a whopping 8KB of VRAM to work with!
///     It's split into 3 segments: 6KB of tile memory, 1KB of BG tile map 0 and 1KB of BG tile map 1.
///     The original GameBoy had an available palette of 4 colors that were basically different shades of green.
///     Each tile is encoded in 16 bytes: 8×8 pixels, 2 bits per pixel.
/// 
///     https://mgba-emu.github.io/gbdoc/
///     https://blog.tigris.fr/2019/09/15/writing-an-emulator-the-first-pixel/
/// 
///


// Screen cycle stuff
pub const SCREEN_WIDTH: usize   = 160;                          // 160px wide
pub const SCREEN_HEIGHT: usize  = 144;                          // 144px high
const LINE_CYCLES: usize        = 456;                          // It takes 456 t-cycles per vertical line
const VDRAW_CYCLES: usize       = SCREEN_HEIGHT * LINE_CYCLES;  // 144 lines => 65664 t-cycles
const VBLANK_CYCLES: usize      = 10 * LINE_CYCLES;             // 10 lines => 4560 t-cycles
pub const FRAME_CYCLES: usize   = VDRAW_CYCLES + VBLANK_CYCLES; // Two of the above combined => 70224

// Screen memory stuff
pub const BG_TILE_MAP_0: usize  = 0x9800;                       // BG tile map 0 address        (Occupies a KB -> 32x32, bigger than the screen)
pub const BG_TILE_MAP_1: usize  = 0x9C00;                       // BG tile map 1 address        (Occupies a KB -> 32x32, bigger than the screen)
const TILE_MEMORY_SIZE: usize   = BG_TILE_MAP_0 - VRAM_BEGIN;   // Tile memory size - 6KB       (Last 2KB bytes are split between the two BG tile maps)
pub const TILE_AMOUNT: usize    = TILE_MEMORY_SIZE / 16;        // Available amount of tiles    (each occupies 16KB)
const TILES_PER_SCREEN: usize   = u8::MAX as usize + 1;         // Since tile maps only work with single bytes we can't use more than 256 at once


// They happen it this order, even though
// the mode numbers kinda suggest otherwise
#[derive(Debug, Copy, Clone)]
pub enum ModePPU {
    OAMScan         = 2, // Mode 2
    DrawingPixels   = 3, // Mode 3
    HBlank          = 0, // Mode 0
    VBlank          = 1, // Mode 1
}


// They're all shades of green, i know
#[derive(Debug, Copy, Clone)]
pub enum TilePixel {
    Zero,   // "White" by default
    One,    // "Light gray" by default
    Two,    // "Dark gray" by default
    Three,  // "Black" by default
}
impl std::fmt::Display for TilePixel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", self) }
}

pub type Tile = [[TilePixel; 8]; 8];
fn empty_tile() -> Tile { [[TilePixel::Zero; 8]; 8] }

pub struct PPU {
    pub mode: ModePPU,                  // Current state of the state machine
    pub ly: u8,                         // Number of the line currently being displayed
    pub x: u8,                          // Number of pixels already output in the current scanline
    pub ticks: usize,                   // Clock ticks counter for the current line
    pub tile_set: [Tile; TILE_AMOUNT],
}
impl Default for PPU {
    fn default() -> Self {
        Self { mode: ModePPU::OAMScan, ly: 0, x: 0, ticks: 0, tile_set: [empty_tile(); TILE_AMOUNT] }
    }
}
impl PPU {
    pub fn tick(&mut self, ticks: usize) {
        self.ticks += ticks;

        match self.mode  {
            ModePPU::OAMScan => {
                if self.ticks >= 80 {
                    self.ticks -= 80;
                    self.mode = ModePPU::DrawingPixels;
                }
            }
            ModePPU::DrawingPixels => {
                if self.ticks >= 172 {
                    self.ticks -= 172;
                    self.mode = ModePPU::HBlank;
                }
            },
            ModePPU::HBlank => {
                if self.ticks >= 204 {
                    self.ticks -= 204;

                    self.ly += 1;

                    if self.ly == 144 {
                        self.mode = ModePPU::VBlank;
                    } else {
                        self.mode = ModePPU::OAMScan;
                    }
                }
            },
            ModePPU::VBlank => {
                if self.ticks >= 456 {
                    self.ticks -= 456;

                    self.ly += 1;

                    if self.ly > 153 {
                        self.ly = 0;
                        self.mode = ModePPU::OAMScan;
                    }
                }
            }
        }
    }

    // https://rylev.github.io/DMG-01/public/book/graphics/tile_ram.html
    pub fn write(&mut self, memory: &mut Memory, mut index: usize, value: u8) {
        memory[index] = value;

        index -= VRAM_BEGIN;

        // The following code just makes it easier to manage the tile set,
        // by effectively doubling it in a more easy to work with format

        // If our index is greater than 0x1800, we're not writing to the tile set storage
        // so we can just return.
        if index >= 0x1800 { return }

        // Tiles rows are encoded in two bytes with the first byte always
        // on an even address. Bitwise ANDing the address with 0xFFE
        // gives us the address of the first byte.
        // For example: `12 & 0xFFFE == 12` and `13 & 0xFFFE == 12`
        let normalized_index = index & 0xFFFE;

        // First we need to get the two bytes that encode the tile row.
        let byte1 = memory[VRAM_BEGIN+normalized_index];
        let byte2 = memory[VRAM_BEGIN+normalized_index + 1];

        // A tiles is 8 rows tall. Since each row is encoded with two bytes a tile
        // is therefore 16 bytes in total.
        let tile_index = index / 16;
        // Every two bytes is a new row
        let row_index = (index % 16) / 2;

        // Now we're going to loop 8 times to get the 8 pixels that make up a given row.
        for pixel_index in 0..8 {
            // To determine a pixel's value we must first find the corresponding bit that encodes
            // that pixels value:
            // 1111_1111
            // 0123 4567
            //
            // As you can see the bit that corresponds to the nth pixel is the bit in the nth
            // position *from the left*. Bits are normally indexed from the right.
            //
            // To find the first pixel (a.k.a pixel 0) we find the left most bit (a.k.a bit 7). For
            // the second pixel (a.k.a pixel 1) we first the second most left bit (a.k.a bit 6) and
            // so on.
            //
            // We then create a mask with a 1 at that position and 0s everywhere else.
            //
            // Bitwise ANDing this mask with our bytes will leave that particular bit with its
            // original value and every other bit with a 0.
            let mask = 1 << (7 - pixel_index);
            let lsb = byte1 & mask;
            let msb = byte2 & mask;

            // If the masked values are not 0 the masked bit must be 1. If they are 0, the masked
            // bit must be 0.
            //
            // Finally we can tell which of the four tile values the pixel is. For example, if the least
            // significant byte's bit is 1 and the most significant byte's bit is also 1, then we
            // have tile value `Three`.
            //
            // +------+------------+
            // | 0b11 | black      |
            // | 0b10 | light-gray |
            // | 0b01 | dark-gray  |
            // | 0b00 | white      |
            // +------+------------+
            let value = match (lsb != 0, msb != 0) {
                (true, true)   => TilePixel::Three,
                (false, true)  => TilePixel::Two,
                (true, false)  => TilePixel::One,
                (false, false) => TilePixel::Zero,
            };

            self.tile_set[tile_index][row_index][pixel_index] = value;
        }
    }
}
