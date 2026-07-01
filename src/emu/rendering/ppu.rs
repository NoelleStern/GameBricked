//!
//! PPU - Pixel Processing Unit (GPUs weren't a thing yet).
//! 
//!     GameBoy has a whopping 8KB of VRAM to work with!
//!     It's split into 3 segments: 6KB of tile memory, 1KB of BG tile map 0 and 1KB of BG tile map 1.
//!     The original GameBoy had an available palette of 4 colors that were basically different shades of green.
//!     Each tile is encoded in 16 bytes: 8×8 pixels, 2 bits per pixel.
//! 
//!     https://mgba-emu.github.io/gbdoc
//!     https://github.com/Ashiepaws/GBEDG/blob/master/ppu/index.md
//!     https://blog.tigris.fr/2019/09/15/writing-an-emulator-the-first-pixel
//!


use smart_default::SmartDefault;
use serde::{Deserialize, Serialize};
use std::{cmp::Reverse, collections::VecDeque, mem};

use crate::{emu::{memory::mmu::{Mmu, OAM_SIZE, VRAM_BEGIN}, processor::interrupts::InterruptBit, rendering::oam::OamEntry}, interface::renderer::{STATStatus, ShadeBuffer, Window}};


// Palette stuff:
    /// Background palette memory address
    pub const BG_PALETTE_ADDRESS: usize = 0xFF47;
    /// Sprite palette 0 memory address
    pub const SPRITE_PALETTE_0_ADDRESS: usize = 0xFF48;
    /// Sprite palette 1 memory address
    pub const SPRITE_PALETTE_1_ADDRESS: usize = 0xFF49;

// Screen cycle stuff:
    /// LCD screen is 160px wide
    pub const SCREEN_WIDTH: usize = 160;
    // LCD screen is 144px high
    pub const SCREEN_HEIGHT: usize = 144;
    /// It takes 456 T-cycles per horizontal line
    const LINE_CYCLES: usize = 456;
    /// 144 lines => 65664 T-cycles
    const VDRAW_CYCLES: usize = SCREEN_HEIGHT * LINE_CYCLES;
    /// 10 lines => 4560 T-cycles
    const VBLANK_CYCLES: usize = 10 * LINE_CYCLES;
    /// Two of the above combined => 70224 T-cycles per frame
    pub const FRAME_CYCLES: usize = VDRAW_CYCLES + VBLANK_CYCLES;

// Screen memory stuff:
    /// BG tile map 0 memory address (Occupies a KB: 32x32, bigger than the screen)
    const BG_TILE_MAP_0: usize = 0x9800;
    /// BG tile map 1 memory address (Occupies a KB: 32x32, bigger than the screen)
    const BG_TILE_MAP_1: usize = 0x9C00;
    /// Tile memory size - 6KB (Last 2KB bytes are split between the two BG tile maps)
    const TILE_MEMORY_SIZE: usize = BG_TILE_MAP_0 - VRAM_BEGIN;
    /// Available amount of tiles is 384 (each occupies 16KB)
    pub const TILE_CAPACITY: usize = TILE_MEMORY_SIZE / 16;
    /// Available amount of OAM entries is 40 (each OAM entry is 4 bytes total)
    pub const OAM_CAPACITY: usize = OAM_SIZE / 4;


/// PPU modes
/// They happen it this order, even though the mode numbers kinda suggest otherwise
#[repr(u8)]
#[derive(Default, Debug, Copy, Clone)]
pub enum ModePPU {
    /// Mode 2
    #[default]
    OAMScan = 2,
    /// Mode 3
    DrawingPixels = 3,
    /// Mode 0
    HBlank = 0,
    /// Mode 1
    VBlank = 1,
}

/// Raw pixel value
#[repr(u8)]
#[derive(Deserialize, Serialize)]
#[derive(Default, Debug, PartialEq, Copy, Clone)]
pub enum RawShade { // They're all shades of green, I know
    #[default]
    /// "White" by default
    Zero = 0,
    /// "Light gray" by default
    One = 1,
    /// "Dark gray" by default
    Two = 2,
    /// "Black" by default
    Three = 3,
}
impl From<u8> for RawShade {
    fn from(byte: u8) -> RawShade {
        match byte {
            0 => RawShade::Zero,
            1 => RawShade::One,
            2 => RawShade::Two,
            _ => RawShade::Three,
        }
    }
}

/// FIFO pixel metadata
#[derive(Default, Debug, Clone, Copy)]
pub struct FIFOEntry {
    /// Palette color
    pixel: RawShade,
    /// Palette id
    palette: TilePalette,
    /// BG to sprite priority
    bg_priority: bool,
    /// Pixel source
    source: FIFOSource,
    /// Should be discarded?
    pub discard_flag: bool,
}
impl FIFOEntry {
    pub fn new(pixel: RawShade, palette: TilePalette, bg_priority: bool, source: FIFOSource, discard_flag: bool) -> Self {
        Self { pixel, palette, bg_priority, source, discard_flag }
    }
}

/// Which palette does a pixel use
#[derive(Default, Debug, Clone, Copy)]
pub enum TilePalette {
    #[default]
    /// Background palette 
    Bg,
    /// Sprite palette
    Sprite(bool)
}
impl TilePalette {
    pub fn get_palette(&self, mmu: &Mmu) -> u8 {
        match self {
            TilePalette::Bg => mmu.get_bg_palette(),
            TilePalette::Sprite(value) => {
                if !*value { mmu.get_sprite_palette0() } else { mmu.get_sprite_palette1() }
            }
        }
    }
}

/// Where did pixel enter FIFO from
/// This info is crucial for pixel mixing
#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub enum FIFOSource {
    #[default]
    /// Pixel came from background layer
    Bg = 0,
    /// Pixel came from window layer
    Window = 1,
    /// Pixel came from sprite layer
    Sprite = 2,
}

#[derive(SmartDefault)]
pub struct Fifo {
    /// FIFO queue
    #[default(VecDeque::with_capacity(16))]
    queue: VecDeque<FIFOEntry>,
    /// OBJ can suspend fetching
    suspend: bool,
}
impl Fifo {
    /// https://github.com/Ashiepaws/GBEDG/blob/master/ppu/index.md#pixel-mixing
    /// 
    /// If, during the shifting process, both the Background and the Sprite FIFO contain at least one pixel, they are both shifted out and compared as follows:
    ///    1. If the color number of the Sprite Pixel is 0, the Background Pixel is pushed to the LCD.
    ///    2. If the BG-to-OBJ-Priority bit is 1 and the color number of the Background Pixel is anything other than 0, the Background Pixel is pushed to the LCD.
    ///    3. If none of the above conditions apply, the Sprite Pixel is pushed to the LCD.
    pub fn mix(&mut self, entries: Vec<FIFOEntry>) {
        for (i, e) in entries.iter().enumerate() {
            let current_entry = self.queue[i];

            if current_entry.source != FIFOSource::Sprite {
                if e.pixel == RawShade::Zero { continue; } // 1
                if e.bg_priority && current_entry.pixel != RawShade::Zero { continue; } // 2
            } else { continue; } // Lower x sprite has priority
            
            self.queue[i] = *e; // 3
        }
    }
    pub fn push8(&mut self, entries: [FIFOEntry; 8]) {
        for e in entries.iter() { self.queue.push_back(*e) }
    }
    pub fn try_pop(&mut self) -> Option<FIFOEntry> {
        if self.pop_ready() { self.queue.pop_front() } else { None }
    }
    pub fn push_ready(&self) -> bool    { self.queue.len() <= 8 }
    pub fn pop_ready(&self) -> bool     { self.queue.len() > 8  }
    pub fn clear(&mut self)             { self.queue.clear();   }
    pub fn len(&mut self) -> usize      { self.queue.len()      }
}

#[derive(Default)]
pub enum FetcherState {
    #[default]
    GetTileID,
    GetTileDataHigh,
    GetTileDataLow,
    PushToFIFO,
}

/// Pixel fetcher uses a simple 4-step state-machine
#[derive(Default)]
pub struct PixelFetcher {
    // Implementation:
        /// Current state
        state: FetcherState,
        /// Current pixel source
        current_source: FetcherSource,
        /// Window this line
        window: Option<Window>,
        /// Current sprite
        sprite: Option<OamEntry>,
        /// Internal current horizontal bg tile counter
        bg_index: u8,
        /// Internal current horizontal window tile counter
        window_index: u8,
        /// Current window y
        /// Resets only on VBlank
        global_window_y: usize,
        /// Background and window FIFO
        fifo: Fifo,
        /// Pixel discard count according to SCX
        discard_counter: u8,
        /// Gets set every second tick.
        /// Paces the fetcher.
        tick_flag: bool,
    // Intermediary data:
        /// Current tile ID calculated by GetTileID
        tile_id: u8,
        /// Byte retrieved by GetTileDataHigh
        high: u8,
        /// Byte retrieved by GetTileDataLow
        low: u8,
        /// Current LY value taken straight from PPU
        ly: u8,
    // Init:
        /// Increases startup by 2 T-cycles resulting in 174 T-cycle baseline
        init_flag: bool,
        /// Down counter that counts 2 PushToFIFO successfully executing
        initial_counter: u8,
}
impl PixelFetcher {
    pub fn start(&mut self, mmu: &Mmu, ly: u8) {
        self.ly = ly;
        self.bg_index = 0;
        self.window_index = 0;
        self.init_flag = false;
        self.initial_counter = 2;
        self.discard_counter = mmu.get_scx() % 8;

        self.window = None;
        self.sprite = None;

        self.current_source = FetcherSource::Bg; // Set source
        self.state = FetcherState::GetTileID; // Reset state
        self.fifo.clear(); // Clear
    }
    pub fn on_scanline_end(&mut self) {
        // If window was triggered this line - update the global y counter
        if self.window.is_some() { self.global_window_y += 1; }
    }
    pub fn on_vblank(&mut self) {
        self.global_window_y = 0; // Only resets here
    }
    pub fn trigger_window(&mut self, window: Window) {
        self.window = Some(window);
        self.current_source = FetcherSource::Window; // Set source
        self.state = FetcherState::GetTileID; // Reset state
        self.fifo.clear(); // Clear
    }
    pub fn trigger_sprite(&mut self, entry: OamEntry) {
        self.sprite = Some(entry);
        self.state = FetcherState::GetTileID; // Reset state
        self.fifo.suspend = true; // Suspend FIFO from popping
        // DO NOT CLEAR, we mix instead
    }

    /// Returns current horizontal tile number
    fn get_bg_tile_x(&self, mmu: &Mmu) -> usize { self.bg_index.wrapping_add(mmu.get_scx() / 8) as usize }
    /// Returns current vertical pixel value
    fn get_bg_pixel_y(&self, mmu: &Mmu) -> usize { self.ly.wrapping_add(mmu.get_scy()) as usize } // Because we can map up to 32*8 => 256 pixels (aka a full u8)
    fn get_base_offset(&mut self, mmu: &Mmu) -> usize {
        // A whole tile takes 16 bytes, hence * 16
        let lcd_control = mmu.get_lcdc();
        if lcd_control.tile_data_area { VRAM_BEGIN + (self.tile_id as usize * 16) } // "Absolute"
        else { 0x9000u16.wrapping_add_signed((self.tile_id as i8 as i16) * 16) as usize } // "Relative"
    } 
    fn get_bg_data(&mut self, mmu: &Mmu, add_one: bool) -> u8 {
        let base_offset = self.get_base_offset(mmu);
        let tile_line = self.get_bg_pixel_y(mmu) % 8; // [0, 7]
        let address = base_offset + (tile_line * 2) + add_one as usize; // A single line takes 2 bytes, hence * 2
        mmu.raw_read8(address)
    }
    fn get_window_data(&mut self, mmu: &Mmu, add_one: bool) -> u8 {
        let base_offset = self.get_base_offset(mmu);
        let tile_line: usize = self.global_window_y % 8; // [0, 7]
        let address: usize = base_offset + (tile_line * 2) + add_one as usize; // A single line takes 2 bytes, hence * 2
        mmu.raw_read8(address)
    }
    fn get_sprite_data(&mut self, entry: OamEntry, mmu: &Mmu, add_one: bool) -> u8 {
        let lcd_control = mmu.get_lcdc();
        let sprite_height = lcd_control.get_sprite_height();

        let mut tile_id = self.tile_id as usize;
        if lcd_control.sprite_size { tile_id &= !1; } // Bit 0 of tile index for 8x16 objects should be ignored

        let mut y: usize = (self.ly as i16 - entry.y) as usize;         // It's guaranteed to be non-negative
        if entry.attr.flip_y { y = (sprite_height-1) - y; }             // Apply y-flip
        let tile_line: usize = y % sprite_height;                       // [0, 7] or [0, 15]
        let offset: usize = (tile_id * 16) + (tile_line * 2);           // A whole tile takes 16 bytes and a line takes 2 bytes 
        let address: usize = VRAM_BEGIN + offset + add_one as usize;    // Let's calculate the final address

        mmu.raw_read8(address)
    }

    pub fn tick4(&mut self, mmu: &Mmu) {
        if !self.tick_flag { self.tick_flag = true; return; } else { self.tick_flag = false; } // Fetcher runs at every second T-cycle
        if !self.init_flag && self.initial_counter == 0 { self.init_flag = true; } // Initialization succeeded
        
        match self.state {
            // Read the tile's ID from the background map
            FetcherState::GetTileID => {
                if let Some(entry) = self.sprite { self.tile_id = entry.tile_id } // Use pre-determined sprite tile-id instead
                else {
                    match self.current_source {
                        FetcherSource::Bg => {
                            // When LCDC.3 is enabled and the X coordinate of the current scanline is not inside the window then tile map 0x9C00 is used
                            let tile_map_address = if mmu.get_lcdc().bg_tile_map { BG_TILE_MAP_1 } else { BG_TILE_MAP_0 };

                            // Divide by 8 to map pixel pos [0, 255] to tile id [0, 31]
                            // Mod by 32 to wrap horizontally and clamp to the same [0, 31]
                            let y_index = self.get_bg_pixel_y(mmu) / 8;
                            let x_index = self.get_bg_tile_x(mmu) % 32;
                            let address = tile_map_address + (y_index * 32) + x_index;
                            self.tile_id = mmu.raw_read8(address);
                        },
                        FetcherSource::Window => {
                            // When LCDC.6 is enabled and the X coordinate of the current scanline is inside the window then tile map 0x9C00 is used
                            let tile_map_address = if mmu.get_lcdc().window_tile_map { BG_TILE_MAP_1 } else { BG_TILE_MAP_0 };

                            // Window disregards both SCX and SCY
                            let y_index = (self.global_window_y) / 8;
                            let x_index = self.window_index as usize;
                            let address = tile_map_address + (y_index * 32) + x_index;
                            self.tile_id = mmu.raw_read8(address);
                        }
                    }
                }
               
                self.state = FetcherState::GetTileDataHigh; // Advance state
            },
            // Read the first byte of pixel data
            FetcherState::GetTileDataHigh => {
                self.high = if let Some(entry) = self.sprite {
                    self.get_sprite_data(entry, mmu, false)
                } else {
                    match self.current_source {
                        FetcherSource::Bg => self.get_bg_data(mmu, false),
                        FetcherSource::Window => self.get_window_data(mmu, false),
                    }
                };
                
                self.state = FetcherState::GetTileDataLow; // Advance state
            },
            // Read the second byte of pixel data
            FetcherState::GetTileDataLow => {
                self.low = if let Some(entry) = self.sprite {
                    self.get_sprite_data(entry, mmu, true)
                } else {
                    match self.current_source {
                        FetcherSource::Bg => self.get_bg_data(mmu, true),
                        FetcherSource::Window => self.get_window_data(mmu, true),
                    }
                };

                self.state = FetcherState::PushToFIFO; // Advance state
                self.push(); // Try push (can take 6 T-cycles)
            },
            // Store pixel data in the FIFO
            FetcherState::PushToFIFO => { self.push(); } // But usually takes 8 T-cycles
        }
    }

    fn push(&mut self) {
        // It always pushes 8 pixels here all together and so it has to wait for enough space to free up.
        // For example visible BG is always 160px wide which is divisible by 8.
        // Window is always 32x32 tiles big which are also divisible by 8.
        // And sprites are always only 8 pixels wide.
        // Sometimes it'll have to discard some pixels here and there though.

        // So yeah. Right here it might wait for FIFO to clear up
        // and so some say Fetcher has a separate "sleep" state

        if let Some(entry) = self.sprite {
            // Read the pixels
            let mut entries = self.get_entries(
                FIFOSource::Sprite,
                TilePalette::Sprite(entry.attr.palette_id),
                entry.attr.priority
            );

            // Flip x if sprite demands it
            if entry.attr.flip_x { entries.reverse(); }

            // Mix
            let mut vec = mem::take(&mut entries).to_vec();
            if entry.x < 0 { vec.truncate(8 - entry.x.unsigned_abs() as usize); } // Shrink if not all pixels are visible
            self.fifo.mix(vec);

            // Pre-advance
            self.fifo.suspend = false;
            self.sprite = None; // Clear the sprite
        } else {
            match self.current_source {
                FetcherSource::Bg => {
                    if !self.fifo.push_ready() { return; } // Capacity check

                    // Read the pixels
                    let mut entries = self.get_entries(
                        FIFOSource::Bg,
                        TilePalette::Bg,
                        false
                    );

                    // Set discard flags
                    if self.bg_index == 0 && self.discard_counter > 0 {
                        for e in entries.iter_mut().take(self.discard_counter as usize) {
                            e.discard_flag = true;
                        }
                    }

                    self.fifo.push8(entries); // Push
                    self.bg_index += 1; // Pre-advance
                },
                FetcherSource::Window => {
                    if !self.fifo.push_ready() { return; } // Capacity check

                    // Read the pixels
                    let mut entries = self.get_entries(
                        FIFOSource::Window,
                        TilePalette::Bg,
                        false
                    );

                    // Set discard flags
                    if  let Some(window) = self.window
                        && self.window_index == 0 && window.wx < 0 {
                            for e in entries.iter_mut().take(window.wx.unsigned_abs() as usize) {
                                e.discard_flag = true;
                            }
                        }

                    self.fifo.push8(entries); // Push
                    self.window_index += 1; // Pre-advance
                }
            }
        }

        // Advance
        self.initial_counter = self.initial_counter.saturating_sub(1);
        self.state = FetcherState::GetTileID // Restart the fetcher loop
    }

    fn get_entries(&self, source: FIFOSource, palette: TilePalette, priority: bool) -> [FIFOEntry; 8] {
        bytes_to_pixels(self.high, self.low).map(|pixel| {
            FIFOEntry::new(pixel, palette, priority, source, false)
        })
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum FetcherSource {
    #[default]
    Bg,
    Window,
}

pub type Tile = [[RawShade; 8]; 8];
pub fn empty_tile() -> Tile { [[RawShade::Zero; 8]; 8] }

#[derive(Default)]
pub struct Ppu {
    /// Is the frame ready?
    pub finished: bool,
    /// Current state of the state machine
    mode: ModePPU,
    /// Stores up to 10 sprite entries in the current scanline
    raw_oam_buffer: Vec<OamEntry>,
    /// Pixel fetcher
    fetcher: PixelFetcher,
    /// T-cycle counter for the current mode
    t_cycles: usize,
    /// Number of the line currently being drawn
    ly: u8,
    /// Number of pixels already put out in the current scanline
    lx: u8,
    /// Up-to-date stat value
    stat: STATStatus,
    /// Amount of T-cycles current DrawingPixels mode has taken
    drawing_length: usize,
}
impl Ppu {
    pub fn tick4(&mut self, mmu: &mut Mmu, shade_buffer: &mut ShadeBuffer) {
        for _t in 0..4u8 {
            self.t_cycles += 1; // Increment
            self.stat = mmu.get_stat_status(); // Update stat

            match self.mode {
                // At the start of each scanline, the PPU scans OAM for sprites it has to render on the current scanline.
                // It takes 2 T-cycles per OAM entry and goes trough all 40 of them, resulting in 80 T-cycles total.
                ModePPU::OAMScan => {
                    if self.t_cycles.is_multiple_of(2) {
                        let lcd_control = mmu.get_lcdc();

                        let oam_id = self.t_cycles / 2; // Remap [2, 80] to [1, 40]
                        let entry = mmu.oam_set[oam_id-1]; // Remap [1, 40] to [0, 39]

                        // https://github.com/Ashiepaws/GBEDG/blob/master/ppu/index.md#sprite-fetching
                        //
                        // Note that I already correct OAM entry x and y values to x-8 and y-16 beforehand.
                        // A sprite is only added to the buffer if all of the following conditions apply:
                        //
                        //     1. Sprite X-Position must be greater than 0
                        //     2. LY + 16 must be greater than or equal to Sprite Y-Position
                        //     3. LY + 16 must be less than Sprite Y-Position + Sprite Height (8 in Normal Mode, 16 in Tall-Sprite-Mode)
                        //     4. The amount of sprites already stored in the OAM Buffer must be less than 10
                        
                        let sprite_height: i16 = lcd_control.get_sprite_height() as i16;
                        if  (entry.x > -8)                                  // 1
                            && (self.ly as i16 >= entry.y)                  // 2
                            && ((self.ly as i16) < entry.y + sprite_height) // 3
                            && (self.raw_oam_buffer.len() < 10) {           // 4
                                // Should naturally go from lowest oam id to highest
                                self.raw_oam_buffer.push(entry);
                            }
                    }

                    // Advance
                    if self.t_cycles == 80 { // Takes 80 T-cycles flat
                        self.t_cycles = 0;

                        self.lx = 0;
                        self.fetcher.start(mmu, self.ly);
                        self.raw_oam_buffer.reverse(); // To have correct same x output
                        self.raw_oam_buffer.sort_by_key(|e| Reverse(e.x)); // Sort by x, higher to lower (should be stable)
                        self.update_mode(mmu, ModePPU::DrawingPixels); // Advance state
                    }
                }
                // Draws pixels to the screen
                ModePPU::DrawingPixels => {
                    // Usually approximated to 172 T-cycles since:
                    //
                    //     It takes 12 T-cycles as startup overhead + 160 T-cycles for the actual productive work.
                    //     You can see it happen here: https://youtu.be/HyzD8pNlpwI?t=3120 (3 M-cycle clocks corresponds to our 12 T-cycles)
                    //
                    //     It generally takes 8 clocks per 8 pixels and that's why the second part matches the screen width exactly.
                    //     Other then that, window, sprites and SCX introduce additional cycles and apparently accumulate up to 289 T-cycles according to Pan Docs.
                    
                    let lcd_control = mmu.get_lcdc();
                    let window: Window = mmu.get_window();
                    
                    // Trigger window
                    if  lcd_control.window_enabled
                        && self.fetcher.current_source == FetcherSource::Bg // Window isn't set yet
                        && self.ly >= window.wy && self.lx as i16 >= window.wx && (self.lx as i16) < window.wx + 160 {
                            self.fetcher.trigger_window(window);
                        }

                    // Trigger sprites
                    if  lcd_control.sprite_enabled
                        && self.fetcher.fifo.len() >= 8 // Has to have at least 8 pixels
                        && self.fetcher.sprite.is_none() // Another sprite isn't processing
                        && let Some(last) = self.raw_oam_buffer.last() // Last since reversed (easier to manage this way)
                        && self.lx as i16 == last.x.clamp(0, 159) { // Count any negative number as 0 (+ an upper limit for a good measure)
                            let entry = self.raw_oam_buffer.pop().unwrap();
                            self.fetcher.trigger_sprite(entry);
                        }
                    
                    // Pop a pixel from the FIFO and push it to the screen, if any
                    if !self.fetcher.fifo.suspend && let Some(entry) = self.fetcher.fifo.try_pop() {
                        if entry.discard_flag { /* Discard if set to discard */ } 
                        else {
                            // Draw to framebuffer
                            let shade = ShadeBuffer::get_shade(entry.palette.get_palette(mmu), entry.pixel);
                            shade_buffer.set_shade(self.lx, self.ly, shade);
                            self.lx += 1;
                        }
                    }

                    self.fetcher.tick4(mmu); // Fetch

                    if self.lx == SCREEN_WIDTH as u8 { // Once we drew a full line
                        self.fetcher.on_scanline_end();
                        self.drawing_length = self.t_cycles;
                        self.t_cycles = 0; // Not before the previous line
                        self.update_mode(mmu, ModePPU::HBlank); // Advance state
                    }
                },
                ModePPU::HBlank => {
                    // OAMScan, DrawingPixels and HBlank take exactly 456 T-cycles together.
                    // OAMScan timing is a 80 T-cycles flat. DrawingPixels has a variable timing and HBlank fills the rest.
                    // Usually approximated to 204 T-cycles according to all of the above.

                    if self.t_cycles == ((LINE_CYCLES - 80) - self.drawing_length) {
                        self.t_cycles = 0;

                        self.inc_ly(mmu);

                        // Advance state
                        if self.ly == SCREEN_HEIGHT as u8 { self.update_mode(mmu, ModePPU::VBlank); }
                        else { self.update_mode(mmu, ModePPU::OAMScan); }
                    }
                },
                ModePPU::VBlank => {
                    if self.t_cycles == LINE_CYCLES { // Takes 456 T-cycles flat
                        self.t_cycles = 0;

                        self.inc_ly(mmu);

                        if self.ly == SCREEN_HEIGHT as u8 + 10 { // Takes 4560 T-cycles flat
                            self.res_ly(mmu);
                            self.finished = true;
                            self.update_mode(mmu, ModePPU::OAMScan); // Restart
                        }
                    }
                }
            }
        }
    }

    fn update_mode(&mut self, mmu: &mut Mmu, mode: ModePPU) {
        self.mode = mode;
        mmu.ppu_mode = mode as u8;

        let interrupt = match mode {
            ModePPU::HBlank => self.stat.mode0_flag,
            ModePPU::OAMScan => {
                self.raw_oam_buffer = vec![]; // Reset the buffer
                self.stat.mode2_flag
            }
            ModePPU::VBlank => {
                self.fetcher.on_vblank();
                mmu.set_interrupt_flag(InterruptBit::VBlank); // Trigger VBlank interrupt
                self.stat.mode1_flag
            },
            _ => false
        };

        if interrupt { mmu.set_interrupt_flag(InterruptBit::Stat); }
    }
    
    fn inc_ly(&mut self, mmu: &mut Mmu) {
        self.ly += 1;
        self.update_ly(mmu);
    }
    fn res_ly(&mut self, mmu: &mut Mmu) {
        self.ly = 0;
        self.update_ly(mmu);
    }
    /// Update LY in memory
    fn update_ly(&self, mmu: &mut Mmu) {
        mmu.set_ly(self.ly);
        if self.stat.lyc_flag && self.ly == mmu.get_lyc() { mmu.set_interrupt_flag(InterruptBit::Stat); }
    }
}

// [0x8000, 0x9FFF]
impl Mmu {
    // https://rylev.github.io/DMG-01/public/book/graphics/tile_ram.html
    pub fn write_vram(&mut self, address: usize, value: u8) {
        self.memory[address] = value;

        // The following code just makes it easier to manage the tile set,
        // by effectively doubling it in a more easy to work with format

        // If our index is greater than 0x1800, we're not writing to the tile set storage
        // so we can just return.
        let index = address - VRAM_BEGIN;
        if index >= 0x1800 { return }

        // Tiles rows are encoded in two bytes with the first byte always
        // on an even address. Bitwise ANDing the address with 0xFFE
        // gives us the address of the first byte.
        // For example: `12 & 0xFFFE == 12` and `13 & 0xFFFE == 12`
        let normalized_index = index & 0xFFFE;

        // First we need to get the two bytes that encode the tile row.
        let high = self.memory[VRAM_BEGIN+normalized_index];
        let low = self.memory[VRAM_BEGIN+normalized_index + 1];

        // A tiles is 8 rows tall. Since each row is encoded with two bytes a tile
        // is therefore 16 bytes in total.
        let tile_index = index / 16;
        // Every two bytes is a new row
        let row_index = (index % 16) / 2;

        let pixels = bytes_to_pixels(high, low);
        self.tile_set[tile_index][row_index].copy_from_slice(&pixels); // Thanks, clippy
    }
}

// https://rylev.github.io/DMG-01/public/book/graphics/tile_ram.html
fn bytes_to_pixels(high: u8, low: u8) -> [RawShade; 8] {
    let mut result = [RawShade::Zero; 8];

    // Now we're going to loop 8 times to get the 8 pixels that make up a given row.
    for (i, item) in result.iter_mut().enumerate() {
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
        let mask = 1 << (7 - i);
        let lsb = high & mask;
        let msb = low & mask;

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
            (true,  true)  => RawShade::Three,
            (false, true)  => RawShade::Two,
            (true,  false) => RawShade::One,
            (false, false) => RawShade::Zero,
        };

        *item = value;
    }

    result
}