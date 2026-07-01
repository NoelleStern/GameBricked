//! 
//! Motherboard:
//! 
//!     Simply combines all of the emulated components together
//! 


use crate::{emu::{controls::joypad::SoftwareJoypad, memory::mmu::Mmu, processor::{cpu::Cpu, interrupts::InterruptBit}, rendering::ppu::Ppu, sound::apu::AudioSamples}, interface::{renderer::ShadeBuffer, rom::{BootRom, Rom}}};


#[derive(Default)]
pub struct MotherBoard {
    /// Central Processing Unit
    pub cpu: Cpu,
    /// Memory Management Unit
    /// Contains a bunch of other modules, namely APU and timers
    pub mmu: Mmu,
    /// Pixel Processing Unit
    pub ppu: Ppu,
    /// M-cycle counter
    pub m_cycles: usize,
}
impl MotherBoard {
    pub fn init(boot_rom: Option<BootRom>) -> Self {
        let mut mb = Self::default();
        if let Some(rom) = boot_rom { mb.mmu.load_boot_rom(rom.bytes); }
        mb
    }
    
    /// Reset the state
    pub fn reset(&mut self, boot_rom: Option<BootRom>, rom: Option<Rom>, reset_rom: bool) {
        let mut game_rom = None;

        if !reset_rom {
            game_rom = if rom.is_some() { rom } else { self.mmu.cart.rom.take() }; // Take rom
        }

        *self = Self::default(); // Full reset

        // Re-init
        self.mmu.load_rom(game_rom); // Load game ROM to memory
        if let Some(br) = boot_rom { self.mmu.load_boot_rom(br.bytes); } // Load boot ROM to memory
        self.mmu.init(); // Initialize the memory after loading roms
    }

    /// Sets input
    pub fn set_joypad(&mut self, joypad: SoftwareJoypad) {
        self.mmu.joypad = joypad;
    }

    /// A system-wide tick representing 1 M-cycle (4 T-cycles)
    pub fn tick4(&mut self, shade_buffer: &mut ShadeBuffer) -> AudioSamples {
        // Tick the peripherals 
        if self.mmu.timers.tick4(&mut self.mmu.apu) {       // Tick the timers, also ticks APU sequencer
            self.mmu.set_interrupt_flag(
                InterruptBit::Timer
            );
        }
        self.ppu.tick4(&mut self.mmu, shade_buffer);        // Tick the PPU
        self.mmu.dma_tick4();                               // Tick the DMA
        let samples: AudioSamples = self.mmu.apu.tick4();   // Tick the APU

        self.cpu.tick4(&mut self.mmu);                      // CPU the CPU

        self.m_cycles += 1;                                 // Increment the cycles
        samples
    }
}