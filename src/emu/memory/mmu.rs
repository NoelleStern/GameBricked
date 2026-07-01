//!
//! Memory and such :3
//!
//!     GameBoy has a 16KB of combined memory shared evenly between RAM and VRAM.
//! 
//!     https://gbdev.io/pandocs/Memory_Map.html
//!     https://rylev.github.io/DMG-01/public/book/memory_map.html
//!


use smart_default::SmartDefault;

use crate::{emu::{controls::joypad::{JOYPAD_ADDRESS, SoftwareJoypad}, memory::dma::{DMA_ADDRESS, Dma}, processor::{interrupts::{IF_ADDRESS, InterruptBit}, timers::Timers}, rendering::{oam::OamEntry, ppu::{self, BG_PALETTE_ADDRESS, OAM_CAPACITY, SPRITE_PALETTE_0_ADDRESS, SPRITE_PALETTE_1_ADDRESS, TILE_CAPACITY, Tile}}, sound::apu::Apu}, interface::{renderer::{LCDControl, STATStatus, Window}, rom::Rom}};


// const CUSTOM_LOGO: [u8; 48] = [
//     0b00001111, 0b10001000, 0b00000000, 0b00000111, 0b00000000, 0b00001011,
//     0b00000000, 0b00001100, 0b00000000, 0b00001111, 0b00000111, 0b01000100,
//     0b00000000, 0b10001010, 0b00000100, 0b10100100, 0b00000000, 0b00001101,
//     0b00000000, 0b00000101, 0b00000000, 0b00001110, 0b00000000, 0b00000001,
//     0b10111001, 0b10011111, 0b00000111, 0b01000111, 0b10101010, 0b10101010,
//     0b10101010, 0b10101010, 0b10011111, 0b10001111, 0b01110100, 0b01000111,
//     0b00111010, 0b10101010, 0b00010101, 0b01010100, 0b00010001, 0b00011101,
//     0b01011001, 0b01010101, 0b00101110, 0b00001110, 0b00011111, 0b10011111,
// ];


// Memory regions:
    const BOOT_ROM_END: usize           = 0x00FF; // R   - ROM
    const GAME_ROM_BANK_0_END: usize    = 0x3FFF; // R   - ROM
    const GAME_ROM_BANK_N_BEGIN: usize  = 0x4000; // R   - ROM
    const GAME_ROM_BANK_N_END: usize    = 0x7FFF; // R   - ROM
    pub const VRAM_BEGIN: usize         = 0x8000; // R/W - VRAM
    pub const VRAM_END: usize           = 0x9FFF; // R/W - VRAM
    const CART_RAM_BEGIN: usize         = 0xA000; // R/W - Cart RAM
    const CART_RAM_END: usize           = 0xBFFF; // R/W - Cart RAM
    const WORKING_RAM_BEGIN: usize      = 0xC000; // R/W - WRAM
    const WORKING_RAM_END: usize        = 0xDFFF; // R/W - WRAM
    // const ECHO_RAM_BEGIN: usize      = 0xE000; // PROHIBITED
    // const ECHO_RAM_END: usize        = 0xFDFF; // PROHIBITED
    pub const OAM_BEGIN: usize          = 0xFE00; // R/W - OAM
    const OAM_END: usize                = 0xFE9F; // R/W - OAM
    const UNUSED_BEGIN: usize           = 0xFEA0; // R/W (always returns 0, can't really do much with it)
    const UNUSED_END: usize             = 0xFEFF; // R/W (always returns 0, can't really do much with it)
    const IO_BEGIN: usize               = 0xFF00; // R/W - IO
    const IO_END: usize                 = 0xFF7F; // R/W - IO
    const HIGH_RAM_BEGIN: usize         = 0xFF80; // R/W - HRAM
    const HIGH_RAM_END: usize           = 0xFFFE; // R/W - HRAM
    const IE_ADDRESS: usize             = 0xFFFF; // R/W - Interrupt Enable Register aka IE

// Region sizes:
    // const VRAM_SIZE: usize           = VRAM_END - VRAM_BEGIN + 1; // VRAM size - 8KB in total (6KB + 1KB + 1KB)
    // const WORKING_RAM_SIZE: usize    = WORKING_RAM_END-WORKING_RAM_BEGIN+1; // 8KB RAM
    // const CART_RAM_SIZE: usize       = CART_RAM_END-CART_RAM_BEGIN+1; // 8KB CART RAM
    // const HIGH_RAM_SIZE: usize       = HIGH_RAM_END-HIGH_RAM_BEGIN+1; // 127B
    pub const OAM_SIZE: usize           = OAM_END-OAM_BEGIN+1; // 160B


#[derive(SmartDefault)]
pub struct CartMemory {
    #[default(vec![0u8; BOOT_ROM_END+1])]
    /// Boot ROM
    pub boot_rom: Vec<u8>,
    /// Game ROM
    pub rom: Option<Rom>,
    /// Bank index
    pub bank_id: u8,
}
impl CartMemory {
    pub fn set_all(&self, memory: &mut Memory) {
        self.set_boot_rom(memory, false);
        self.set_bank_0(memory);
        self.set_bank_n(memory);
    }
    pub fn set_boot_rom(&self, memory: &mut Memory, boot_flag: bool) {
        if boot_flag {
            if let Some(rom) = &self.rom {
                memory[..=BOOT_ROM_END].clone_from_slice(
                    &rom.bytes[..=BOOT_ROM_END]
                );
            }
        } else {
            memory[..=BOOT_ROM_END].clone_from_slice(
                &self.boot_rom[..=BOOT_ROM_END]
            );
        }
    }
    pub fn set_bank_0(&self, memory: &mut Memory) { // Excluding the boot ROM part
        if let Some(rom) = &self.rom {
            memory[BOOT_ROM_END+1..=GAME_ROM_BANK_0_END].clone_from_slice(
                &rom.bytes[BOOT_ROM_END+1..=GAME_ROM_BANK_0_END]
            );
        }
    }
    pub fn set_bank_n(&self, memory: &mut Memory) {
        // TODO: implement memory banking
        if let Some(rom) = &self.rom {
            memory[GAME_ROM_BANK_N_BEGIN..=GAME_ROM_BANK_N_END].clone_from_slice(
            &rom.bytes[GAME_ROM_BANK_N_BEGIN..=GAME_ROM_BANK_N_END]
            );
        }
    }
}

type Memory = Vec<u8>;
#[derive(SmartDefault)]
pub struct Mmu {
    /// Cartridge memory manager
    pub cart: CartMemory,
    /// All of the accessible memory
    #[default(vec![0u8; 0xFFFF+1])]
    pub memory: Memory,
    /// Direct Memory Access, directly integrated
    pub dma: Dma,
    /// Audio Processing Unit, closely integrated
    pub apu: Apu,
    /// DIV, TIMA, TMA and TAC, closely integrated
    pub timers: Timers,
    /// Tile set ready for rendering
    #[default([ppu::empty_tile(); TILE_CAPACITY])]
    pub tile_set: [Tile; TILE_CAPACITY],
    /// OAM set ready for rendering
    #[default([OamEntry::default(); OAM_CAPACITY])]
    pub oam_set: [OamEntry; OAM_CAPACITY],
    /// Current input state
    pub joypad: SoftwareJoypad,
    /// Current PPU mode
    pub ppu_mode: u8,
}
impl Mmu {
    /// Initializes cartridge memory
    pub fn init(&mut self) { self.cart.set_all(&mut self.memory); }

    // ROM stuff
    pub fn load_boot_rom(&mut self, rom: Vec<u8>)   { self.cart.boot_rom = rom; }
    pub fn load_rom(&mut self, rom: Option<Rom>)    { self.cart.rom = rom;      }

    // LCD screen stuff
    pub fn get_lcdc(&self) -> LCDControl    { self.memory[0xFF40].into() }
    pub fn get_scy(&self) -> u8             { self.memory[0xFF42]        }
    pub fn get_scx(&self) -> u8             { self.memory[0xFF43]        }
    /// The Window is visible (if enabled) when WX and WY are in the range [0; 166] and [0; 143] respectively.
    /// Values WX=7, WY=0 place the Window at the top left of the screen, completely covering the background.
    pub fn get_window(&self) -> Window {
        let wy = self.memory[0xFF4A];
        let wx = self.memory[0xFF4B];
        Window::new(wy, wx as i16 - 7)
    }
    pub fn get_stat_status(&self) -> STATStatus {
        let value = self.memory[0xFF41];
        let ly = self.get_ly();
        let lyc = self.get_lyc();
        STATStatus::new(value, ly == lyc, self.ppu_mode)
    }

    // LY
    pub fn get_ly(&self) -> u8              { self.memory[0xFF44]          }
    pub fn set_ly(&mut self, value: u8)     { self.memory[0xFF44] = value; }
    pub fn get_lyc(&self) -> u8             { self.memory[0xFF45]          }
    pub fn set_lyc(&mut self, value: u8)    { self.memory[0xFF45] = value; }

    // Palettes
    pub fn get_bg_palette(&self) -> u8      { self.memory[BG_PALETTE_ADDRESS]       }
    pub fn get_sprite_palette0(&self) -> u8 { self.memory[SPRITE_PALETTE_0_ADDRESS] }
    pub fn get_sprite_palette1(&self) -> u8 { self.memory[SPRITE_PALETTE_1_ADDRESS] }

    // Interrupts
    pub fn get_ie(&self) -> u8                              { self.memory[IE_ADDRESS]                        }
    pub fn get_if(&self) -> u8                              { self.memory[IF_ADDRESS]                        }
    pub fn set_if(&mut self, value: u8)                     { self.memory[IF_ADDRESS] = value;               }
    pub fn set_interrupt_flag(&mut self, bit: InterruptBit) { self.set_if(self.get_if() | (1 << bit as u8)); }

    // Reads
    pub fn raw_read8(&self, address: usize) -> u8 {
        match address {
            JOYPAD_ADDRESS  => self.read_joypad(),          // Joypad
            0xFF04..=0xFF07 => self.read_timers(address),   // Timers
            0xFF10..=0xFF3F => self.read_apu(address),      // APU
            _ => self.memory[address]                       // Just read
        }
    }
    pub fn cpu_read8(&self, address: u16) -> u8 {
        let address = address as usize;
        if !self.dma.active { self.raw_read8(address) } // Normal read
        else {
            // If DMA takes place, only HRAM is available
            match address {
                HIGH_RAM_BEGIN..=HIGH_RAM_END => self.raw_read8(address),
                _ => 0xFF, // Give back 0xFF otherwise
            }
        }
    }

    // Writes
    pub fn raw_write8(&mut self, address: usize, value: u8) {
        match address {
            VRAM_BEGIN..=VRAM_END               => self.write_vram(address, value), // VRAM                 [0x8000, 0x9FFF]
            CART_RAM_BEGIN..=CART_RAM_END       => self.memory[address] = value,    // Cart RAM             [0xA000, 0xBFFF]
            WORKING_RAM_BEGIN..=WORKING_RAM_END => self.memory[address] = value,    // Working RAM          [0xC000, 0xDFFF]
            OAM_BEGIN..=OAM_END                 => self.write_oam(address, value),  // OAM                  [0xFE00, 0xFE9F]
            UNUSED_BEGIN..=UNUSED_END           => { /* Do nothing */ },            // Unused               [0xFEA0, 0xFEFF]
            IO_BEGIN..=IO_END                   => self.write_io(address, value),   // I/O                  [0xFF00, 0xFF7F]
            HIGH_RAM_BEGIN..=HIGH_RAM_END       => self.memory[address] = value,    // HRAM                 [0xFF80, 0xFFFE]
            IE_ADDRESS                          => self.memory[address] = value,    // Interrupt Enabled    0xFFFF
            0x2000                              => { /* TODO: MBC control */ },
            0x60B0|0x40B0|0x00B0|0x00FF|0x00A9|0x00A4|0x0064  => { /* Added all that just to play a Tetris ROM hack */ },
            0x0000                              => { /* TODO */ },
            _ => unreachable!("address: {:#06X}", address) // You shouldn't write anywhere else
        }
    }
    pub fn cpu_write8(&mut self, address: u16, value: u8) {
        let address = address as usize;
        if !self.dma.active { self.raw_write8(address, value); } // Normal write
        else { if let HIGH_RAM_BEGIN..=HIGH_RAM_END = address { self.raw_write8(address, value) } } // If DMA takes place, only HRAM is available
    }

    // [0xFF00, 0xFF7F]
    fn write_io(&mut self, address: usize, value: u8) {
        // 0xFF00 => Joypad input - just write
        // 0xFF0F => Interrupts - just write
        // 0xFF40..=0xFF4B => LCD Control, Status, Position, Scrolling, and Palettes - just write
        // Additionally [0xFF4C, 0xFF4F], [0xFF51, 0xFF56], [0xFF68, 0xFF6C] and 0xFF70 are all CGB stuff
        match address {
            0xFF01..=0xFF02 =>  { /* TODO */ },                     // Serial transfer
            0xFF04..=0xFF07 =>  self.write_timers(address, value),  // Timers
            0xFF10..=0xFF3F =>  self.write_apu(address, value),     // APU
            DMA_ADDRESS =>      self.write_dma(value),              // DMA
            0xFF50 =>           self.write_boot_rom_flag(value),    // Boot ROM mapping control
            _ =>                self.memory[address] = value        // Just write
        }
    }
    fn write_boot_rom_flag(&mut self, value: u8) {
        let new_flag = value == 0x1;
        let old_flag = self.memory[0xFF50] == 0x1;
        if new_flag != old_flag { self.cart.set_boot_rom(&mut self.memory, new_flag); }
    }
}