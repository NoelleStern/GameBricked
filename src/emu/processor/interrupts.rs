//! 
//! GameBoy hardware interrupts
//! 


use crate::emu::{processor::cpu::Cpu, memory::mmu::Mmu};


/// Interrupt Flag address
pub const IF_ADDRESS: usize = 0xFF0F;
// Note: IE_ADDRESS already exists under MMU


/// Interrupt bits
#[derive(Debug, Clone, Copy)]
pub enum InterruptBit {
    /// Bit 4
    Joypad = 4,
    /// Bit 3
    Serial = 3,
    /// Bit 2
    Timer = 2,
    /// Bit 1
    Stat = 1,
    /// Bit 0
    VBlank = 0,
}
impl InterruptBit {
    /// Returns a respective memory address to jump to
    pub fn to_address(self) -> u16 {
        match self {
            InterruptBit::Joypad => 0x60,
            InterruptBit::Serial => 0x58,
            InterruptBit::Timer  => 0x50,
            InterruptBit::Stat   => 0x48,
            InterruptBit::VBlank => 0x40,
        }
    }
}


#[derive(Debug, Clone)]
pub struct Interrupts {
    pub joypad: bool,
    pub serial: bool,
    pub timer:  bool,
    pub stat:   bool,
    pub vblank: bool,
}
impl Interrupts {
    pub fn test(cpu: &mut Cpu, mmu: &mut Mmu) -> Option<InterruptBit> {
        let mut result: Option<InterruptBit> = None;

        let ieb: u8 = mmu.get_ie();
        let ifb: u8 = mmu.get_if();
        let pending: bool = (ieb & ifb) != 0;

        if cpu.halt && pending {
           cpu.halt = false; // Unhalt!
        }

        if cpu.ime && pending { // If at least some interrupt is enabled and allowed
            let iie: Interrupts = ieb.into();
            let iif: Interrupts = ifb.into();

            result = if iie.vblank && iif.vblank { Some(InterruptBit::VBlank) }
            else if     iie.stat   && iif.stat   { Some(InterruptBit::Stat  ) }
            else if     iie.timer  && iif.timer  { Some(InterruptBit::Timer ) }
            else if     iie.serial && iif.serial { Some(InterruptBit::Serial) }
            else if     iie.joypad && iif.joypad { Some(InterruptBit::Joypad) }
            else                                 { None                       };
        }
        
        result
    }
    pub fn unset(mmu: &mut Mmu, bit: InterruptBit) {
        let ifb = mmu.get_if();
        let value = ifb & !(1 << bit as u8);
        mmu.set_if(value);
    }
}
impl From<Interrupts> for u8 {
    fn from(ie: Interrupts) -> u8 {
        (ie.joypad as u8)   << InterruptBit::Joypad as u8 |
        (ie.serial as u8)   << InterruptBit::Serial as u8 |
        (ie.timer as u8)    << InterruptBit::Timer  as u8 |
        (ie.stat as u8)     << InterruptBit::Stat   as u8 |
        (ie.vblank as u8)   << InterruptBit::VBlank as u8
    }
}
impl From<u8> for Interrupts {
    fn from(byte: u8) -> Self {
        let joypad  = ((byte >> InterruptBit::Joypad as u8) & 1) != 0;
        let serial  = ((byte >> InterruptBit::Serial as u8) & 1) != 0;
        let timer   = ((byte >> InterruptBit::Timer  as u8) & 1) != 0;
        let stat    = ((byte >> InterruptBit::Stat   as u8) & 1) != 0;
        let vblank  = ((byte >> InterruptBit::VBlank as u8) & 1) != 0;

        Interrupts { joypad, serial, timer, stat, vblank }
    }
}