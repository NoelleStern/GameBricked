use crate:: rendering::ppu::PPU;


///
/// Memory and such :3
///
///     GameBoy has a 16KB of combined memory shared evenly between RAM and VRAM.
///     https://rylev.github.io/DMG-01/public/book/memory_map.html
///


const BOOT_ROM_BEGIN: usize         = 0x0000; // R
const BOOT_ROM_END: usize           = 0x00FF; // R
const GAME_ROM_BANK_0_BEGIN: usize  = 0x0000; // R
const GAME_ROM_BANK_0_END: usize    = 0x3FFF; // R
const GAME_ROM_BANK_N_BEGIN: usize  = 0x4000; // R
const GAME_ROM_BANK_N_END: usize    = 0x7FFF; // R
pub const VRAM_BEGIN: usize         = 0x8000; // R/W VRAM
pub const VRAM_END: usize           = 0x9FFF; // R/W VRAM
const CART_RAM_BEGIN: usize         = 0xA000; // R/W Cart RAM
const CART_RAM_END: usize           = 0xBFFF; // R/W Cart RAM
const WORKING_RAM_BEGIN: usize      = 0xC000; // R/W
const WORKING_RAM_END: usize        = 0xDFFF; // R/W
const ECHO_RAM_BEGIN: usize         = 0xE000; // PROHIBITED
const ECHO_RAM_END: usize           = 0xFDFF; // PROHIBITED
const OAM_BEGIN: usize              = 0xFE00; // R/W
const OAM_END: usize                = 0xFE9F; // R/W
const UNUSED_BEGIN: usize           = 0xFEA0; // R/W, but always returns 0, can't really do much with it
const UNUSED_END: usize             = 0xFEFF; // R/W, but always returns 0, can't really do much with it
const IO_BEGIN: usize               = 0xFF00; // R/W
const IO_END: usize                 = 0xFF7F; // R/W
const HIGH_RAM_BEGIN: usize         = 0xFF80; // R/W HRAM
const HIGH_RAM_END: usize           = 0xFFFE; // R/W HRAM
const IER_ADDRESS: usize            = 0xFFFF; // R/W Interrupt Enable Register

// Sizes
const VRAM_SIZE: usize              = VRAM_END - VRAM_BEGIN + 1; // VRAM size - 8KB in total (6KB + 1KB + 1KB)
const WORKING_RAM_SIZE: usize       = WORKING_RAM_END-WORKING_RAM_BEGIN+1; // 8KB RAM
const CART_RAM_SIZE: usize          = CART_RAM_END-CART_RAM_BEGIN+1; // 8KB CART RAM
const HIGH_RAM_SIZE: usize          = HIGH_RAM_END-HIGH_RAM_BEGIN+1; // 127B
const OAM_SIZE: usize               = OAM_END-OAM_BEGIN+1;


pub type Memory = [u8; 0xFFFF+1];
fn empty_memory() -> Memory { [0u8; 0xFFFF+1] }

pub struct CartMemory {
    pub boot_rom: Vec<u8>,  // Boot ROM
    pub rom: Vec<u8>,       // Cart ROM
    pub boot_flag: bool,    // Indicates if boot rom was executed
    pub bank: u8,           // Bank indicator
}
impl Default for CartMemory {
    fn default() -> Self {
        Self { boot_rom: vec![], rom: vec![], boot_flag: false, bank: 0 }
    }
}
impl CartMemory {
    pub fn set_all(&self, memory: &mut Memory) {
        self.set_boot_rom(memory);
        self.set_bank_0(memory  );
        self.set_bank_n(memory  );
    }
    pub fn set_boot_rom(&self, memory: &mut Memory) {
        if self.boot_flag {
            memory[..=BOOT_ROM_END].copy_from_slice(
                &self.rom[..=BOOT_ROM_END]
            );
        } else {
            memory[..=BOOT_ROM_END].copy_from_slice(
                &self.boot_rom[..=BOOT_ROM_END]
            );
        }
    }
    pub fn set_bank_0(&self, memory: &mut Memory) { // Excluding boot rom part
        memory[BOOT_ROM_END+1..=GAME_ROM_BANK_0_END].copy_from_slice(
            &self.rom[BOOT_ROM_END+1..=GAME_ROM_BANK_0_END]
        );
    }
    pub fn set_bank_n(&self, memory: &mut Memory) {
        // TODO: implement memory banking
        memory[GAME_ROM_BANK_N_BEGIN..=GAME_ROM_BANK_N_END].copy_from_slice(
            &self.rom[GAME_ROM_BANK_N_BEGIN..=GAME_ROM_BANK_N_END]
        );
    }
}

pub struct MMU {
    pub cart: CartMemory,   // Cartridge memory manager
    pub memory: Memory,     // All of the accessible memory
    pub ppu: PPU,           // Pixel Processing Unit (feels weird to put it here, but it updates with memory writes)
}
impl Default for MMU {
    fn default() -> Self {
        Self { cart: CartMemory::default(), memory: empty_memory(), ppu: PPU::default() }
    }
}
impl MMU {
    pub fn init(&mut self) {
        self.cart.set_all(&mut self.memory); // Init cartridge memory
    }

    pub fn boot_rom_unload(&mut self) {
        let flag = self.read8(0xFF50) == 0x1; // Flag is located in the actual memory
        if self.cart.boot_flag != flag {
            self.cart.boot_flag = flag;
            self.cart.set_boot_rom(&mut self.memory);
        }
    }

    // Rom stuff
    pub fn load_boot_rom(&mut self, rom: Vec<u8>) {
        self.cart.boot_rom = rom;
    }
    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.cart.rom = rom;
    }

    // Read
    pub fn read8(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }
    pub fn read16_rev(&self, address: u16) -> u16 {
        let lsb = self.read8(address) as u16;
        let msb = self.read8(address+1) as u16;
        (msb << 8) | lsb
    }

    // Write
    pub fn write8(&mut self, address: u16, value: u8) {
        let address = address as usize;
        match address {
            VRAM_BEGIN..=VRAM_END               => self.ppu.write(&mut self.memory, address, value), // VRAM
            CART_RAM_BEGIN..=CART_RAM_END       => self.memory[address] = value, // Cart RAM
            WORKING_RAM_BEGIN..=WORKING_RAM_END => self.memory[address] = value, // Working RAM
            OAM_BEGIN..=OAM_END                 => self.memory[address] = value, // OAM
            UNUSED_BEGIN..=UNUSED_END           => { /* Do nothing */ },
            IO_BEGIN..=IO_END                   => self.memory[address] = value, // I/O
            HIGH_RAM_BEGIN..=HIGH_RAM_END       => self.memory[address] = value, // HRAM
            IER_ADDRESS                         => self.memory[address] = value, // Interrupt Enabled Register
            _ => unreachable!("address: {:#06X}", address) // You shouldn't write anywhere else
        }
    }
}