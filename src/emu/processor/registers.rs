//!
//! GameBoy Registers:
//!
//!     All of the cool 8-bit DMG CPU registers.
//! 
//!     https://rylev.github.io/DMG-01/public/book/cpu/registers.html
//!


use crate::emu::processor::cpu::Cpu;

// Bit  Name    Explanation
// -----------------------------------
// 7	Z	    Zero flag
// 6	N	    Subtraction flag (BCD)
// 5	H	    Half Carry flag (BCD)
// 4	C	    Carry flag
const ZERO_FLAG_BYTE_POSITION: u8       = 7; // Bit 7
const SUBTRACT_FLAG_BYTE_POSITION: u8   = 6; // Bit 6
const HALF_CARRY_FLAG_BYTE_POSITION: u8 = 5; // Bit 5
const CARRY_FLAG_BYTE_POSITION: u8      = 4; // Bit 4
                                             // The rest is 0s


pub enum CPUReg { A, B, C, D, E, H, L }
pub enum DoubleCPUReg { BC, DE, HL(bool), SP }
impl DoubleCPUReg {
    pub fn get(&self, cpu: &Cpu) -> u16 {
        match self {
            DoubleCPUReg::BC    => cpu.registers.get_bc(),
            DoubleCPUReg::DE    => cpu.registers.get_de(),
            DoubleCPUReg::HL(_) => cpu.registers.get_hl(),
            DoubleCPUReg::SP    => cpu.sp,
        }
    }
    pub fn inc(&self, cpu: &mut Cpu) {
        match self {
            DoubleCPUReg::BC    => cpu.registers.inc_bc(),
            DoubleCPUReg::DE    => cpu.registers.inc_de(),
            DoubleCPUReg::HL(_) => cpu.registers.inc_hl(),
            DoubleCPUReg::SP    => cpu.inc_sp(),
        }
    }
    pub fn dec(&self, cpu: &mut Cpu) {
        match self {
            DoubleCPUReg::BC    => cpu.registers.dec_bc(),
            DoubleCPUReg::DE    => cpu.registers.dec_de(),
            DoubleCPUReg::HL(_) => cpu.registers.dec_hl(),
            DoubleCPUReg::SP    => cpu.dec_sp(),
        }
    }
}

/// 8-bit registers
#[derive(Default)]
pub struct Registers {
    // AF
        /// Accumulator 8-bit register
        pub a: u8,
        /// Flags 8-bit register
        pub f: FlagsRegister,
    // BC
        /// B 8-bit register
        pub b: u8,
        /// C 8-bit register
        pub c: u8,
    // DE
        /// D 8-bit register
        pub d: u8,
        /// E 8-bit register
        pub e: u8,
    // HL (can be used to point to addresses in memory)
        /// H 8-bit register
        pub h: u8,
        /// L 8-bit register
        pub l: u8,
}
impl Registers {
    /// Get / Set 16-bit combined registers AF, BC, DE and HL
    fn get_combined(r1: u8, r2: u8) -> u16 { (r1 as u16) << 8 | r2 as u16 }
    pub fn set_combined<T: From<u8>>(r1: &mut u8, r2: &mut T, value: u16) { // F flag + deref add some complexity here
        *r1 = ((value & 0xFF00) >> 8) as u8;
        *r2 = ((value & 0xFF) as u8).into();
    }

    // Get u16
    pub fn get_af(&self) -> u16 { Self::get_combined(self.a, self.f.into()) }
    pub fn get_bc(&self) -> u16 { Self::get_combined(self.b, self.c)        }
    pub fn get_de(&self) -> u16 { Self::get_combined(self.d, self.e)        }
    pub fn get_hl(&self) -> u16 { Self::get_combined(self.h, self.l)        }

    // Set u16
    pub fn set_af(&mut self, value: u16) { Self::set_combined(&mut self.a, &mut self.f, value); }
    pub fn set_bc(&mut self, value: u16) { Self::set_combined(&mut self.b, &mut self.c, value); }
    pub fn set_de(&mut self, value: u16) { Self::set_combined(&mut self.d, &mut self.e, value); }
    pub fn set_hl(&mut self, value: u16) { Self::set_combined(&mut self.h, &mut self.l, value); }

    // Increment
    pub fn inc_bc(&mut self) { self.set_bc(self.get_bc().wrapping_add(1)); }
    pub fn inc_de(&mut self) { self.set_de(self.get_de().wrapping_add(1)); }
    pub fn inc_hl(&mut self) { self.set_hl(self.get_hl().wrapping_add(1)); }

    // Decrement
    pub fn dec_bc(&mut self) { self.set_bc(self.get_bc().wrapping_sub(1)); }
    pub fn dec_de(&mut self) { self.set_de(self.get_de().wrapping_sub(1)); }
    pub fn dec_hl(&mut self) { self.set_hl(self.get_hl().wrapping_sub(1)); }
}

#[derive(Default, Clone, Copy)]
pub struct FlagsRegister {
    // Zero aka Z flag - was the result zero?
    pub zero:       bool,
    /// Subtract aka N flag - was it a subtraction?
    /// Set by many, used only by DAA instruction
    pub subtract:   bool,
    /// Half carry aka H flag - did a 4-bit/12-bit overflow occur?
    /// Set by many, used only by DAA instruction
    pub half_carry: bool,
    /// Carry aka C - Did a 8-bit/16-bit overflow occur?
    /// For C and H first overflow is for 8-bit operations and second is for 16-bit ones
    pub carry:      bool,
}
impl FlagsRegister {
    pub fn new(z: bool, n: bool, h: bool, c: bool) -> Self {
        Self { zero: z, subtract: n, half_carry: h, carry: c }
    }
}
impl From<FlagsRegister> for u8 {
    fn from(flag: FlagsRegister) -> u8 {
        (flag.zero as u8)       << ZERO_FLAG_BYTE_POSITION       |
        (flag.subtract as u8)   << SUBTRACT_FLAG_BYTE_POSITION   |
        (flag.half_carry as u8) << HALF_CARRY_FLAG_BYTE_POSITION |
        (flag.carry as u8)      << CARRY_FLAG_BYTE_POSITION
    }
}
impl From<u8> for FlagsRegister {
    fn from(byte: u8) -> Self {
        let zero       = ((byte >> ZERO_FLAG_BYTE_POSITION      ) & 1) != 0;
        let subtract   = ((byte >> SUBTRACT_FLAG_BYTE_POSITION  ) & 1) != 0;
        let half_carry = ((byte >> HALF_CARRY_FLAG_BYTE_POSITION) & 1) != 0;
        let carry      = ((byte >> CARRY_FLAG_BYTE_POSITION     ) & 1) != 0;

        FlagsRegister { zero, subtract, half_carry, carry }
    }
}