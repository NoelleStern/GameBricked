//!
//! Behold the DMG-CPU aka Sharp SM83 - a spiritual relative of fan favorite Zilog Z80 and Intel 8080.
//! 
//!     Yep, the Sinclair ZX Spectrum Zilog Z80.
//! 
//!     https://rgbds.gbdev.io/docs/v1.0.1/gbz80.7
//!     https://meganesu.github.io/generate-gb-opcodes
//!     http://www.codeslinger.co.uk/pages/projects/gameboy/files/GB.pdf
//! 
//!     Passes all of the Blargg cpu_instrs and instr_timing tests
//!


use crate::emu::{processor::{instruction_info::InstInfo, interrupts::{InterruptBit, Interrupts}, registers::{CPUReg, DoubleCPUReg, Registers}}, memory::mmu::Mmu};


/// Sub instruction type
enum SubInst {
    Rlc, Rrc,
    Rl, Rr,
    Sla, Sra,
    Swap, Srl, 
    Bit(u8), Res(u8), Set(u8),
}

/// Interrupt Enable Flag values
#[derive(Default)]
pub enum EIFlag {
    Unset = 0,
    Set = 1,
    #[default]
    JustSet = 2,
}

/// A 16-bit operation buffer object
/// (stores intermediary 2 bytes of data)
#[derive(Default, Clone, Copy)]
pub struct OpBuffer {
    /// Upper byte
    upper: u8,
    /// Lower byte
    lower: u8,
}
impl From<OpBuffer> for u16 {
    fn from(buff: OpBuffer) -> u16 {
        ((buff.upper as u16) << 8) | (buff.lower as u16)
    }
}
impl From<u16> for OpBuffer {
    fn from(word: u16) -> Self {
        OpBuffer { upper: (word >> 8) as u8, lower: word as u8 }
    }
}

#[derive(Default)]
pub struct Cpu {
    // Registers
        /// 8-bit Registers
        pub registers: Registers,
        /// Stack Pointer - points to where the top of the stack is
        pub sp: u16,
        /// Program Counter - points to the next instruction in the memory
        pub pc: u16,
    // Flags
        /// Interrupt Master Enable
        pub ime: bool,
        /// STOP instruction flag
        pub stop: bool,
        /// HALT instruction flag
        pub halt: bool,
        /// HALT bug indicator
        pub halt_bug: bool,
        /// EI state, since EI doesn't immediately take effect
        pub ei_flag: EIFlag,
    // Execution state
        /// Current CMD execution step
        pub current_step: u8,
        /// Current CMD opcode
        pub current_cmd: u8,
        /// Current SUB CMD opcode
        pub current_sub_cmd: u8,
        /// Current interrupt
        pub interrupt: Option<InterruptBit>,
        /// Operation buffer
        pub op_buffer: OpBuffer,
}
impl Cpu {
    pub fn debug_info(&self, mmu: &Mmu) -> String {
        format!(
            "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} SP:{:04X} PC:{:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
            // "A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})",
            self.registers.a, u8::from(self.registers.f),
            self.registers.b, self.registers.c,
            self.registers.d, self.registers.e,
            self.registers.h, self.registers.l,
            self.sp, self.pc, 
            mmu.cpu_read8(self.pc), mmu.cpu_read8(self.pc+1),
            mmu.cpu_read8(self.pc+2), mmu.cpu_read8(self.pc+3),
        )
    }

    // Fetch
    fn fetch_bci(&self, mmu: &Mmu)  -> u8 { mmu.cpu_read8(self.registers.get_bc()) } // Get u8 value at BC address
    fn fetch_dei(&self, mmu: &Mmu)  -> u8 { mmu.cpu_read8(self.registers.get_de()) } // Get u8 value at DE address
    fn fetch_hli(&self, mmu: &Mmu)  -> u8 { mmu.cpu_read8(self.registers.get_hl()) } // Get u8 value at HL address
    fn fetch8(&mut self, mmu: &Mmu) -> u8 {
        // Get u8 value at next PC address
        let result = mmu.cpu_read8(self.pc);
        self.pc = self.pc.wrapping_add(1);
        result
    }
  
    // Overflow
    fn carry_borrow_check(a: u8, b: u8, cf: bool) -> bool { (b as u16 + cf as u16) > a  as u16 }    // Carry borrow (with optional C)
    fn carry_overflow_check(a: u8, b: u8, cf: Option<bool>) -> bool {                               // Carry overflow (with optional C)
        (a as u16 + b as u16 + cf.unwrap_or(false) as u16) > 0xFF
    }
    fn hc_borrow_check(a: u8, b: u8, cf: Option<bool>) -> bool {                                    // Half carry borrow from bit 4 check
        ((b & 0x0F) + cf.unwrap_or(false) as u8) > (a & 0x0F)
    }
    fn hc_overflow_check(a: u8, b: u8, cf: Option<bool>) -> bool {                                  // Half carry overflow from bit 3 check
       ((a & 0x0F) + (b & 0x0F) + cf.unwrap_or(false) as u8) > 0x0F
    }        
    fn hc_overflow_check16(a: u16, b: u16) -> bool { ((a & 0x0FFF) + (b & 0x0FFF)) > 0x0FFF }       // Half carry u16 overflow check

    // Inc / Dec
    pub fn inc_sp(&mut self) { self.sp = self.sp.wrapping_add(1); }
    pub fn dec_sp(&mut self) { self.sp = self.sp.wrapping_sub(1); }

    // CPUReg stuff
    fn get_reg_mut(&mut self, reg: &CPUReg) -> &mut u8 {
        match reg {
            CPUReg::A => &mut self.registers.a,
            CPUReg::B => &mut self.registers.b,
            CPUReg::C => &mut self.registers.c,
            CPUReg::D => &mut self.registers.d,
            CPUReg::E => &mut self.registers.e,
            CPUReg::H => &mut self.registers.h,
            CPUReg::L => &mut self.registers.l,
        }
    }
    fn get_reg_val(&self, reg: &CPUReg) -> u8 {
        match reg {
            CPUReg::A => self.registers.a,
            CPUReg::B => self.registers.b,
            CPUReg::C => self.registers.c,
            CPUReg::D => self.registers.d,
            CPUReg::E => self.registers.e,
            CPUReg::H => self.registers.h,
            CPUReg::L => self.registers.l,
        }
    }

    // Opcode instructions
    // https://rgbds.gbdev.io/docs/v1.0.1/gbz80.7
    fn adc(&mut self, value: u8, step: u8) -> bool {                            // ADd + Carry
        match step {
            1 => {
                let result = self.registers.a.wrapping_add(value).wrapping_add(self.registers.f.carry as u8);

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = Self::hc_overflow_check(self.registers.a, value, Some(self.registers.f.carry));
                self.registers.f.carry = Self::carry_overflow_check(self.registers.a, value, Some(self.registers.f.carry));

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn add(&mut self, value: u8, step: u8) -> bool {                            // ADD
        match step {
            1 => {
                let (result, overflow_flag) = self.registers.a.overflowing_add(value);

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = Self::hc_overflow_check(self.registers.a, value, None);
                self.registers.f.carry = overflow_flag;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn add16(&mut self, r: DoubleCPUReg, step: u8) -> bool { //                 // ADD
        match step {
            1 => {
                let value = self.sample_add16(
                    self.registers.get_hl(),
                    r.get(self)
                );
                self.registers.set_hl(value);
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn add_sp_e8(&mut self, mmu: &Mmu, step: u8) -> bool {                      // ADD e8 to SP
        match step {
            1 => {
                self.op_buffer = (self.fetch8(mmu) as i8 as i16 as u16).into();
            },
            2 => {
                self.registers.f.zero = false;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = Self::hc_overflow_check(self.sp as u8, self.op_buffer.lower, None);
                self.registers.f.carry = Self::carry_overflow_check(self.sp as u8, self.op_buffer.lower, None);
            },
            3 => {
                self.sp = self.sp.wrapping_add(self.op_buffer.into());
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn and(&mut self, value: u8, step: u8) -> bool {                            // bitwise AND
        match step {
            1 => {
                let result = self.registers.a & value;

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = true;
                self.registers.f.carry = false;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn bit(&mut self, value: u8, n: u8, step: u8) -> bool {                     // test a BIT in a register, set the zero flag if the bit isn't set
        match step {
            1 => {
                self.sample_bit(value, n);
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn call(&mut self, mmu: &mut Mmu, step: u8) -> bool {                       // CALL
        self.call_c(mmu, true, step)
    }
    fn call_c(&mut self, mmu: &mut Mmu, flag: bool, step: u8) -> bool {         // CALL Conditional (custom)
        match step {
            1 => {
                self.op_buffer.lower = self.fetch8(mmu);
            },
            2 => {
                self.op_buffer.upper = self.fetch8(mmu);
                return !flag;
            },
            3 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, (self.pc >> 8) as u8); // Upper
            },
            4 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, self.pc as u8); // Lower
            },
            5 => {
                self.pc = self.op_buffer.into();
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn ccf(&mut self, step: u8) -> bool {                                       // Complement Carry Flag (aka invert)
        match step {
            1 => {
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = !self.registers.f.carry;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn cp(&mut self, value: u8, step: u8) -> bool {                             // ComPare, not CoPy!
        match step {
            1 => {
                let result = self.registers.a == value;
        
                self.registers.f.zero = result;
                self.registers.f.subtract = true;
                self.registers.f.half_carry = Self::hc_borrow_check(self.registers.a, value, None);
                self.registers.f.carry = value > self.registers.a;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn cpl(&mut self, step: u8) -> bool {                                       // ComPLement accumulator (aka bitwise NOT)
        match step {
            1 => {
                self.registers.f.subtract = true;
                self.registers.f.half_carry = true;

                self.registers.a = !self.registers.a;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn daa(&mut self, step: u8) -> bool {                                       // Decimal Adjust Accumulator
        match step {
            1 => {
               // https://rgbds.gbdev.io/docs/v1.0.1/gbz80.7#DAA
                let mut adjust: u8 = 0; // Initialize the adjustment to 0

                let result = if self.registers.f.subtract { // After subtraction
                    if self.registers.f.half_carry { adjust += 0x06; } // If half_carry is set, add 0x06 to the adjustment
                    if self.registers.f.carry { adjust += 0x60; } // If carry is set, add 0x60 to the adjustment
                    self.registers.a.wrapping_sub(adjust) // Subtract the adjustment from A
                } else { // After addition
                    // If half-carry is set or A & 0x0F > 9, add 0x06 to the adjustment
                    if self.registers.f.half_carry || (self.registers.a & 0x0F) > 9 {
                        adjust += 0x06;
                    }

                    // If carry flag is set or A > 0x99, add $0x60 to the adjustment and set the carry flag
                    if self.registers.f.carry || self.registers.a > 0x99 {
                        adjust += 0x60;
                        self.registers.f.carry = true;
                    }

                    self.registers.a.wrapping_add(adjust) // Add the adjustment to A
                };

                self.registers.f.zero = result == 0;
                self.registers.f.half_carry = false;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn dec(&mut self, r: CPUReg, step: u8) -> bool {                            // DECrement by 1
        match step {
            1 => {
                let result = self.sample_dec(self.get_reg_val(&r));
                let r = self.get_reg_mut(&r);
                *r = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn dec16(&mut self, r: DoubleCPUReg, step: u8) -> bool {                    // DECrement by 1
        match step {
            1 => {
                r.dec(self);
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn di(&mut self, step: u8) -> bool {                                        // Disable Interrupts
        match step {
            1 => {
                self.ime = false;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn ei(&mut self, step: u8) -> bool {                                        // Enable Interrupts
        match step {
            1 => {
                self.ei_flag = EIFlag::JustSet;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn halt(&mut self, step: u8) -> bool {                                      // HALT (enter CPU low-power consumption mode)
        match step {
            1 => {
                if self.ime { self.halt_bug = true; }
                else { self.halt = true; }
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn inc(&mut self, r: CPUReg, step: u8) -> bool {                            // INCrement by 1
        match step {
            1 => {
                let result = self.sample_inc(self.get_reg_val(&r));
                let r = self.get_reg_mut(&r);
                *r = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn inc16(&mut self, r: DoubleCPUReg, step: u8) -> bool {                    // INCrement by 1
        match step {
            1 => {
                r.inc(self);
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn jp(&mut self, mmu: &Mmu, step: u8) -> bool {                             // JumP to an address 
        self.jp_c(mmu, true, step)
    }
    fn jp_hl(&mut self, step: u8) -> bool {                                     // JumP HL, only takes 1 M-cycle
        match step {
            1 => {
                let address = self.registers.get_hl();
                self.pc = address;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn jp_c(&mut self, mmu: &Mmu, flag: bool, step: u8) -> bool {               // JP Conditional (custom)
        match step {
            1 => {
                self.op_buffer.lower = self.fetch8(mmu);
            },
            2 => {
                self.op_buffer.upper= self.fetch8(mmu);
                return !flag;
            },
            3 => {
                self.pc = self.op_buffer.into();
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn jr(&mut self, mmu: &Mmu, step: u8) -> bool {                             // Jump Relative
        self.jr_c(mmu, true, step)
    }
    fn jr_c(&mut self, mmu: &Mmu, flag: bool, step: u8) -> bool {               // JR Conditional (custom)
        match step {
            1 => {
                self.op_buffer.lower = self.fetch8(mmu);
                !flag
            },
            2 => {
                self.pc = self.sample_jr(self.op_buffer.lower);
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn ld(r: &mut u8, v: u8, step: u8) -> bool {                                // LoaD
        match step {
            1 => {
                *r = v;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn ld16(&mut self, mmu: &Mmu, r1: CPUReg, r2: CPUReg, step: u8) -> bool {   // LoaD 16
        match step {
            1 => {
                let value = self.fetch8(mmu);
                let r = self.get_reg_mut(&r2);
                *r = value;
            },
            2 => {
                let value = self.fetch8(mmu);
                let r = self.get_reg_mut(&r1);
                *r = value;
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn ld_a_hl(&mut self, mmu: &mut Mmu, r: DoubleCPUReg, step: u8) -> bool {   // LD A (HL+/-)
        match step {
            1 => {
                self.registers.a = self.fetch_hli(mmu);
                match r {
                    DoubleCPUReg::HL(inc) => {
                        if inc { self.registers.inc_hl(); }
                        else { self.registers.dec_hl(); }
                    },
                    _ => unreachable!()
                }
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn ld_hl(&mut self, mmu: &mut Mmu, value: u8, step: u8) -> bool {           // LD (HL) X
        match step {
            1 => {
                mmu.cpu_write8(self.registers.get_hl(), value);
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn ld_hl_a(&mut self, mmu: &mut Mmu, r: DoubleCPUReg, step: u8) -> bool {   // LD (HL+/-) A
        match step {
            1 => {
                let address = self.registers.get_hl();
                mmu.cpu_write8(address, self.registers.a);
                match r {
                    DoubleCPUReg::HL(inc) => {
                        if inc { self.registers.inc_hl(); }
                        else { self.registers.dec_hl(); }
                    },
                    _ => unreachable!()
                }
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn ld_hl_sp_e8(&mut self, mmu: &Mmu, step: u8) -> bool {                    // LD HL,SP+e8
        match step {
            1 => {
                self.op_buffer = (self.fetch8(mmu) as i8 as i16 as u16).into();

                self.registers.f.zero = false;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = Self::hc_overflow_check(self.sp as u8, self.op_buffer.lower, None);
                self.registers.f.carry = Self::carry_overflow_check(self.sp as u8, self.op_buffer.lower, None);
            },
            2 => {
                let value = self.sp.wrapping_add(self.op_buffer.into());
                self.registers.set_hl(value);
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn or(&mut self, value: u8, step: u8) -> bool {                             // bitwise OR
        match step {
            1 => {
                let result = self.registers.a | value;

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = false;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn pop_af(&mut self, mmu: &Mmu, step: u8) -> bool {                         // POP AF
        match step {
            1 => {
                let value= mmu.cpu_read8(self.sp);
                self.inc_sp();

                self.registers.f.zero       = ((value >> 7) & 1) != 0;
                self.registers.f.subtract   = ((value >> 6) & 1) != 0;
                self.registers.f.half_carry = ((value >> 5) & 1) != 0;
                self.registers.f.carry      = ((value >> 4) & 1) != 0;
            },
            2 => {
                self.registers.a = mmu.cpu_read8(self.sp);
                self.inc_sp();
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn pop(&mut self, mmu: &Mmu, r1: CPUReg, r2: CPUReg, step: u8) -> bool {    // POP
        match step {
            1 => {
                let result = mmu.cpu_read8(self.sp);
                let r = self.get_reg_mut(&r2);
                *r = result;
                self.inc_sp();
            },
            2 => {
                let value =  mmu.cpu_read8(self.sp);
                let r = self.get_reg_mut(&r1);
                *r = value;
                self.inc_sp();
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn push(&mut self, mmu: &mut Mmu, r1: u8, r2: u8, step: u8) -> bool {       // PUSH
        match step {
            1 => { /* Internal delay */ },
            2 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, r1);
            },
            3 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, r2);
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn res(&mut self, r: CPUReg, n: u8, step: u8) -> bool {                     // RESet a bit
        self.sub_inst(SubInst::Res(n), r, step)
    }
    fn ret(&mut self, mmu: &Mmu, step: u8) -> bool {                            // RETurn from subroutine
        match step {
            1 => {
                self.op_buffer.lower = mmu.cpu_read8(self.sp);
                self.inc_sp();
            },
            2 => {
                self.op_buffer.upper = mmu.cpu_read8(self.sp);
                self.inc_sp();
            },
            3 => {
                self.pc = self.op_buffer.into();
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn ret_c(&mut self, mmu: &Mmu, flag: bool, step: u8) -> bool {              // RETurn Conditional (custom)
        match step {
            1 => { !flag },
            2..=4 => { self.ret(mmu, step-1) },
            _ => unreachable!("{}", step)
        }
    }
    fn reti(&mut self, mmu: &Mmu, step: u8) -> bool {                           // RETurn from subroutine and enable Interrupts
        let result = self.ret(mmu, step);
        if result { self.ime = true; }
        result
    }
    fn rl(&mut self, r: CPUReg, step: u8) -> bool {                             // Rotate Left through the carry flag
        self.sub_inst(SubInst::Rl, r, step)
    }
    fn rla(&mut self, step: u8) -> bool {                                       // Rotate Left A through the carry flag
        match step {
            1 => {
                let result = (self.registers.a << 1) | (self.registers.f.carry as u8);

                self.registers.f.zero = false;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = (self.registers.a & 0x80) != 0;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn rlc(&mut self, r: CPUReg, step: u8) -> bool {                            // Rotate Left Circular
        self.sub_inst(SubInst::Rlc, r, step)
    }
    fn rlca(&mut self, step: u8) -> bool {                                      // Rotate Left Circular
        match step {
            1 => {
                let c = (self.registers.a & 0x80) != 0;
                let result: u8 = (self.registers.a << 1) | (c as u8);

                self.registers.f.zero = false;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = c;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn rr(&mut self, r: CPUReg, step: u8) -> bool {                             // Rotate Right
        self.sub_inst(SubInst::Rr, r, step)
    }
    fn rra(&mut self, step: u8) -> bool {                                       // Rotate Right A
        match step {
            1 => {
                let result = (self.registers.a >> 1) | ((self.registers.f.carry as u8) << 7);

                self.registers.f.zero = false;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = (self.registers.a & 0x01) != 0;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn rrc(&mut self, r: CPUReg, step: u8) -> bool {                            // Rotate Right Circular
        self.sub_inst(SubInst::Rrc, r, step)
    }
    fn rrca(&mut self, step: u8) -> bool {                                      // Rotate Right Circular A
        match step {
            1 => {
                let c = (self.registers.a & 0x01) != 0;
                let result = (self.registers.a >> 1) | ((c as u8) << 7);

                self.registers.f.zero = false;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = c;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn rst(&mut self, mmu: &mut Mmu, address: u16, step: u8) -> bool {          // ReStarT (this is a shorter and faster equivalent to CALL)
        match step {
            1 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, (self.pc >> 8) as u8); // Upper
            },
            2 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, self.pc as u8); // Lower
            },
            3 => {
                self.pc = address;
                return true;
            },
            _ => unreachable!("{}", step)
        }
        false
    }
    fn sbc(&mut self, value: u8, step: u8) -> bool {                            // SuBtract with C
        match step {
            1 => {
                let c = self.registers.f.carry;
                let result = self.registers.a.wrapping_sub(value).wrapping_sub(c as u8);

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = true;
                self.registers.f.half_carry = Self::hc_borrow_check(self.registers.a, value, Some(c));
                self.registers.f.carry = Self::carry_borrow_check(self.registers.a, value, c);


                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn scf(&mut self, step: u8) -> bool {                                       // Set Carry Flag
        match step {
            1 => {
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = true;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn set(&mut self, r: CPUReg, n: u8, step: u8) -> bool {                     // SET a bit
        self.sub_inst(SubInst::Set(n), r, step)
    }
    fn sla(&mut self, r: CPUReg, step: u8) -> bool {                            // Shift Left Arithmetically
        self.sub_inst(SubInst::Sla, r, step)
    }
    fn sra(&mut self, r: CPUReg, step: u8) -> bool {                            // Shift Right Arithmetically
        self.sub_inst(SubInst::Sra, r, step)
    }
    fn srl(&mut self, r: CPUReg, step: u8) -> bool {                            // Shift Right Logically
        self.sub_inst(SubInst::Srl, r, step)
    }
    fn stop(&mut self, step: u8) -> bool {                                      // STOP (enter CPU very low power mode)
        match step {
            1 => {
                self.stop = true;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn sub(&mut self, value: u8, step: u8) -> bool {                            // SUBtract
        match step {
            1 => {
                let (result, overflow_flag) = self.registers.a.overflowing_sub(value);

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = true;
                self.registers.f.half_carry = Self::hc_borrow_check(self.registers.a, value, None);
                self.registers.f.carry = overflow_flag;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn swap(&mut self, r: CPUReg, step: u8) -> bool {                           // SWAP
        self.sub_inst(SubInst::Swap, r, step)
    }
    fn xor(&mut self, value: u8, step: u8) -> bool {                            // bitwise XOR
        match step {
            1 => {
                let result = self.registers.a ^ value;

                self.registers.f.zero = result == 0;
                self.registers.f.subtract = false;
                self.registers.f.half_carry = false;
                self.registers.f.carry = false;

                self.registers.a = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    
    // Sample instructions
    fn sample_add16(&mut self, v1: u16, v2: u16) -> u16 {                       // ADD u16 (sample)
        let (result, overflow_flag) = v1.overflowing_add(v2);

        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check16(v1, v2);
        self.registers.f.carry = overflow_flag;

        result
    }
    fn sample_bit(&mut self, value: u8, n: u8) {                                // BIT (sample)
        let bit: bool = ((value >> n) & 1) != 0;

        self.registers.f.zero = !bit;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = true;
    }
    fn sample_dec(&mut self, value: u8) -> u8 {                                 // DECrement by 1 (sample)
        let result = value.wrapping_sub(1);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = Self::hc_borrow_check(value, 1, None);

        result
    }
    fn sample_inc(&mut self, value: u8) -> u8 {                                 // INCrement by 1 (sample)
        let result = value.wrapping_add(1);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check(value, 1, None);

        result
    }
    fn sample_jr(&mut self, value: u8) -> u16 {                                 // Jump Relative (sample)
        self.pc.wrapping_add_signed(value as i8 as i16)
    }
    fn sample_res(&mut self, value: u8, n: u8) -> u8 {                          // RESet a bit (sample)
        value & !(1 << n)
    }
    fn sample_rlc(&mut self, value: u8) -> u8 {                                 // Rotate Left Circular (sample)
        let c = (value & 0x80) != 0;
        let result = (value << 1) | (c as u8);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = c;

        result
    }
    fn sample_rl(&mut self, value: u8) -> u8 {                                  // Rotate Left through the carry flag
        let result = (value << 1) | (self.registers.f.carry as u8);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x80) != 0;

        result
    }
    fn sample_rr(&mut self, value: u8) -> u8 {                                  // Rotate Right (sample)
        let result = (value >> 1) | ((self.registers.f.carry as u8) << 7);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x01) != 0;

        result
    }
    fn sample_rrc(&mut self, value: u8) -> u8 {                                 // Rotate Right Circular (sample)
        let c = (value & 0x01) != 0;
        let result = (value >> 1) | ((c as u8) << 7);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = c;

        result
    }
    fn sample_set(&mut self, value: u8, n: u8) -> u8 {                          // SET a bit (sample)
        value | (1 << n)
    }
    fn sample_sla(&mut self, value: u8) -> u8 {                                 // Shift Left Arithmetically (sample)
        let result = value << 1;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x80) != 0;

        result
    }
    fn sample_sra(&mut self, value: u8) -> u8 {                                 // Shift Right Arithmetically (sample)
        let result = (value >> 1) | (value & 0x80);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x01) != 0;

        result
    }
    fn sample_srl(&mut self, value: u8) -> u8 {                                 // Shift Right Logically (sample)
        let result = value >> 1;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 1) != 0;

        result
    }
    fn sample_swap(&mut self, value: u8) -> u8 {                                // SWAP (sample)
        let result = value.rotate_right(4); // Clippy said so

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;

        result
    }

    // Sub instructions
    fn sub_inst(&mut self, sub: SubInst, r: CPUReg, step: u8) -> bool {         // Most sub instructions
        match step {
            1 => {
                let value = self.get_reg_val(&r);

                let result = match sub {
                    SubInst::Rlc        => self.sample_rlc( value),
                    SubInst::Rrc        => self.sample_rrc( value),
                    SubInst::Rl         => self.sample_rl(  value),
                    SubInst::Rr         => self.sample_rr(  value),
                    SubInst::Sla        => self.sample_sla( value),
                    SubInst::Sra        => self.sample_sra( value),
                    SubInst::Swap       => self.sample_swap(value),
                    SubInst::Srl        => self.sample_srl( value),
                    SubInst::Res(n) => self.sample_res( value, n),
                    SubInst::Set(n) => self.sample_set( value, n),
                    _ => unreachable!("{}", step)
                };
                
                let r = self.get_reg_mut(&r);
                *r = result;
                true
            },
            _ => unreachable!("{}", step)
        }
    }
    fn sub_inst_hl(&mut self, mmu: &mut Mmu, sub: SubInst, step: u8) -> bool {  // HL sub instructions
        match sub {
            SubInst::Bit(n) => {
                match step {
                    1 => { 
                        let value = self.fetch_hli(mmu);
                        self.sample_bit(value, n);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
            },
            _ => {
                match step {
                    1 => { self.op_buffer.lower = self.fetch_hli(mmu); },
                    2 => {                    
                        self.op_buffer.lower = match sub {
                            SubInst::Rlc        => self.sample_rlc( self.op_buffer.lower),
                            SubInst::Rrc        => self.sample_rrc( self.op_buffer.lower),
                            SubInst::Rl         => self.sample_rl(  self.op_buffer.lower),
                            SubInst::Rr         => self.sample_rr(  self.op_buffer.lower),
                            SubInst::Sla        => self.sample_sla( self.op_buffer.lower),
                            SubInst::Sra        => self.sample_sra( self.op_buffer.lower),
                            SubInst::Swap       => self.sample_swap(self.op_buffer.lower),
                            SubInst::Srl        => self.sample_srl( self.op_buffer.lower),
                            SubInst::Res(n) => self.sample_res(self.op_buffer.lower, n),
                            SubInst::Set(n) => self.sample_set(self.op_buffer.lower, n),
                            _ => unreachable!()
                        };

                        let address = self.registers.get_hl();
                        mmu.cpu_write8(address, self.op_buffer.lower);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
            }
        }
        false
    }

    pub fn tick4(&mut self, mmu: &mut Mmu) {
        // This is important. We don't want to deal with interrupts mid-cmd
        if self.current_step == 0 && self.interrupt.is_none() {
            self.interrupt = Interrupts::test(self, mmu);
        }

        // Do the processing
        let has_finished = match self.interrupt {
            Some(bit) => { self.process_interrupt(mmu, bit) },
            None => self.process_cmd(mmu),
        };

        // Advance state
        if !has_finished {
            self.current_step += 1;
        }
        else {
            self.manage_ei_flag();
            self.current_step = 0;
        }
    }

    fn manage_ei_flag(&mut self) {
        match self.ei_flag {
            EIFlag::Set => { self.ime = true; self.ei_flag = EIFlag::Unset; },
            EIFlag::JustSet => { self.ei_flag = EIFlag::Set; },
            _ => ()
        }
    }

    fn process_interrupt(&mut self, mmu: &mut Mmu, bit: InterruptBit) -> bool {
        // 4 M-cycles cycles total (I do not count fetching the new opcode)
        match self.current_step {
            0 => {
                self.ime = false;
                Interrupts::unset(mmu, bit);
            },
            1 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, (self.pc >> 8) as u8); // Upper
            },
            2 => {
                self.dec_sp();
                mmu.cpu_write8(self.sp, self.pc as u8); // Lower
            },
            3 => {
                self.pc = bit.to_address();
                self.interrupt = None;
                return true;
            },
            _ => unreachable!("{}", self.current_step)
        }
        false
    }

    fn process_cmd(&mut self, mmu: &mut Mmu) -> bool {
        let mut has_finished = false;
        if self.current_step != 0 {
            // If any other step - keep executing
            has_finished = self.match_cmd(mmu, self.current_cmd, self.current_step)
        } else {
            // If first step - fetch
            self.current_cmd = self.fetch8(mmu); // Fetch an instruction
            let info = InstInfo::decode(self.current_cmd);

            // If the command only takes 1 M-cycle - execute immediately
            if info.cl.full == 1 { 
                has_finished = self.match_cmd(mmu, self.current_cmd, 1);
            }
        }
        has_finished
    }

    /// Main huge CPU switch case
    fn match_cmd(&mut self, mmu: &mut Mmu, cmd: u8, step: u8) -> bool {
        let has_finished: bool = match cmd {
            // Ox0X
            0x00 => { /* NOP, does nothing */ true },
            0x01 => self.ld16(mmu, CPUReg::B, CPUReg::C, step), // LD BC, d16
            0x02 => {
                // LD (BC), A [2 M-cycles total]
                // Store contents of A in the memory location specified by BC
                match step {
                    1 => {
                        let address = self.registers.get_bc();
                        mmu.cpu_write8(address, self.registers.a);
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0x03 => self.inc16(DoubleCPUReg::BC, step), // INC BC
            0x04 => self.inc(CPUReg::B, step), // INC B
            0x05 => self.dec(CPUReg::B, step), // DEC B
            0x06 => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.b, v, step) }, // LD B, d8
            0x07 => self.rlca(step), // RLCA
            0x08 => {
                // LD (a16), SP [5 M-cycles total]
                // Store SP at (a16)
                match step {
                    1 => { self.op_buffer.lower = self.fetch8(mmu); },
                    2 => { self.op_buffer.upper = self.fetch8(mmu); },
                    3 => {
                        let lower = self.sp as u8;
                        let address: u16 = self.op_buffer.into();
                        mmu.cpu_write8(address, lower);
                    },
                    4 => {
                        let upper = (self.sp >> 8) as u8;
                        let address: u16 = self.op_buffer.into();
                        mmu.cpu_write8(address+1, upper);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },    
            0x09 => self.add16(DoubleCPUReg::BC, step), // ADD HL, BC
            0x0A => {
                // LD A, (BC) [2 M-cycles total]
                // Load 8-bit contents of memory specified by BC into A
                match step {
                    1 => {
                        self.registers.a = self.fetch_bci(mmu);
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0x0B => self.dec16(DoubleCPUReg::BC, step), // INC BC
            0x0C => self.inc(CPUReg::C, step), // INC C
            0x0D => self.dec(CPUReg::C, step), // DEC C
            0x0E => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.c, v, step) }, // LD C, d8
            0x0F => self.rrca(step), // RRCA
            // Ox1X
            0x10 => self.stop(step), // STOP
            0x11 => self.ld16(mmu, CPUReg::D, CPUReg::E, step), // LD DE, d16
            0x12 => {
                // LD (DE), A [2 M-cycles total]
                // Store contents of A in the memory location specified by DE
                match step {
                    1 => {
                        let address = self.registers.get_de();
                        mmu.cpu_write8(address, self.registers.a);
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0x13 => self.inc16(DoubleCPUReg::DE, step), // INC DE
            0x14 => self.inc(CPUReg::D, step), // INC D
            0x15 => self.dec(CPUReg::D, step), // DEC D
            0x16 => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.d, v, step) }, // LD D, d8
            0x17 => self.rla(step), // RLA
            0x18 => self.jr(mmu, step), // JR s8
            0x19 => self.add16(DoubleCPUReg::DE, step), // ADD HL, DE
            0x1A => {
                // LD A, (DE) [2 M-cycles total]
                // Load 8-bit contents of memory specified by DE into A
                match step {
                    1 => {
                        self.registers.a = self.fetch_dei(mmu);
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0x1B => self.dec16(DoubleCPUReg::DE, step), // INC DE
            0x1C => self.inc(CPUReg::E, step), // INC E
            0x1D => self.dec(CPUReg::E, step), // DEC E
            0x1E => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.e, v, step) }, // LD E, d8
            0x1F => self.rra(step), // RRA
            // Ox2X
            0x20 => self.jr_c(mmu, !self.registers.f.zero, step), // JR NZ, s8
            0x21 => self.ld16(mmu, CPUReg::H, CPUReg::L, step), // LD HL, d16
            0x22 => self.ld_hl_a(mmu, DoubleCPUReg::HL(true), step), // LD (HL+), A
            0x23 => self.inc16(DoubleCPUReg::HL(false), step), // INC HL
            0x24 => self.inc(CPUReg::H, step), // INC H
            0x25 => self.dec(CPUReg::H, step), // DEC H
            0x26 => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.h, v, step) }, // LD H, d8
            0x27 => self.daa(step), // DAA
            0x28 => self.jr_c(mmu, self.registers.f.zero, step), // JR Z, s8
            0x29 => self.add16(DoubleCPUReg::HL(false), step), // ADD HL, HL
            0x2A => self.ld_a_hl(mmu, DoubleCPUReg::HL(true), step), // LD A, (HL+)
            0x2B => self.dec16(DoubleCPUReg::HL(false), step), // INC HL
            0x2C => self.inc(CPUReg::L, step), // INC L
            0x2D => self.dec(CPUReg::L, step), // DEC L
            0x2E => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.l, v, step) }, // LD L, d8
            0x2F => self.cpl(step), // CPL
            // Ox3X
            0x30 => self.jr_c(mmu, !self.registers.f.carry, step), // JR NC, s8
            0x31 => {
                // LD SP, d16 [3 M-cycles total]
                // Load 2 bytes of data into SP
                match step {
                    1 => {
                        self.sp = (self.sp & 0xFF00) | (self.fetch8(mmu) as u16);
                    },
                    2 => {
                        self.sp = ((self.fetch8(mmu) as u16) << 8) |  (self.sp & 0x00FF);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0x32 => self.ld_hl_a(mmu, DoubleCPUReg::HL(false), step), // LD (HL-), A
            0x33 => self.inc16(DoubleCPUReg::SP, step), // INC SP
            0x34 => {
                // INC (HL) [3 M-cycles total]
                // Increment contents of memory specified by HL by 1
                match step {
                    1 => { self.op_buffer.lower = self.fetch_hli(mmu); },
                    2 => {
                        let value = self.sample_inc(self.op_buffer.lower);
                        let address = self.registers.get_hl();
                        mmu.cpu_write8(address, value);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0x35 => {
                // DEC (HL) [3 M-cycles total]
                // Decrement contents of memory specified by HL by 1
                match step {
                    1 => { self.op_buffer.lower = self.fetch_hli(mmu); },
                    2 => {
                        let value = self.sample_dec(self.op_buffer.lower);
                        let address = self.registers.get_hl();
                        mmu.cpu_write8(address, value);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0x36 => {
                // LD (HL), d8 [3 M-cycles total]
                // Store d8 in the memory location specified by HL
                match step {
                    1 => { self.op_buffer.lower = self.fetch8(mmu); },
                    2 => {
                        let address = self.registers.get_hl();
                        mmu.cpu_write8(address, self.op_buffer.lower);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0x37 => self.scf(step), // SCF
            0x38 => self.jr_c(mmu, self.registers.f.carry, step), // JR C, s8
            0x39 => self.add16(DoubleCPUReg::SP, step), // ADD HL, SP
            0x3A => self.ld_a_hl(mmu, DoubleCPUReg::HL(false), step), // LD A, (HL-)
            0x3B => self.dec16(DoubleCPUReg::SP, step), // INC SP
            0x3C => self.inc(CPUReg::A, step), // INC A
            0x3D => self.dec(CPUReg::A, step), // DEC A
            0x3E => { let v = self.fetch8(mmu); Self::ld(&mut self.registers.a, v, step) }, // LD A, d8
            0x3F => self.ccf(step), // CCF 
            // Ox4X - Loads
            0x40 => { /* Basically does nothing */ true },                       // Load B into B
            0x41 => Self::ld(&mut self.registers.b, self.registers.c, step), // Load C into B
            0x42 => Self::ld(&mut self.registers.b, self.registers.d, step), // Load D into B
            0x43 => Self::ld(&mut self.registers.b, self.registers.e, step), // Load E into B
            0x44 => Self::ld(&mut self.registers.b, self.registers.h, step), // Load H into B
            0x45 => Self::ld(&mut self.registers.b, self.registers.l, step), // Load L into B
            0x46 => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.b, v, step) }, // Load (HL) into B
            0x47 => Self::ld(&mut self.registers.b, self.registers.a, step), // Load A into B
            0x48 => Self::ld(&mut self.registers.c, self.registers.b, step), // Load B into C
            0x49 => { /* Basically does nothing */ true },                       // Load C into C
            0x4A => Self::ld(&mut self.registers.c, self.registers.d, step), // Load D into C
            0x4B => Self::ld(&mut self.registers.c, self.registers.e, step), // Load E into C
            0x4C => Self::ld(&mut self.registers.c, self.registers.h, step), // Load H into C
            0x4D => Self::ld(&mut self.registers.c, self.registers.l, step), // Load L into C
            0x4E => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.c, v, step) }, // Load (HL) into C
            0x4F => Self::ld(&mut self.registers.c, self.registers.a, step), // Load A into C
            // Ox5X - Loads
            0x50 => Self::ld(&mut self.registers.d, self.registers.b, step), // Load B into D
            0x51 => Self::ld(&mut self.registers.d, self.registers.c, step), // Load C into D
            0x52 => { /* Basically does nothing */ true },                       // Load D into D
            0x53 => Self::ld(&mut self.registers.d, self.registers.e, step), // Load E into D
            0x54 => Self::ld(&mut self.registers.d, self.registers.h, step), // Load H into D
            0x55 => Self::ld(&mut self.registers.d, self.registers.l, step), // Load L into D
            0x56 => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.d, v, step) }, // Load (HL) into D
            0x57 => Self::ld(&mut self.registers.d, self.registers.a, step), // Load A into D
            0x58 => Self::ld(&mut self.registers.e, self.registers.b, step), // Load B into E
            0x59 => Self::ld(&mut self.registers.e, self.registers.c, step), // Load C into E
            0x5A => Self::ld(&mut self.registers.e, self.registers.d, step), // Load D into E
            0x5B => { /* Basically does nothing */ true },                       // Load E into E
            0x5C => Self::ld(&mut self.registers.e, self.registers.h, step), // Load H into E
            0x5D => Self::ld(&mut self.registers.e, self.registers.l, step), // Load L into E
            0x5E => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.e, v, step) }, // Load (HL) into E
            0x5F => Self::ld(&mut self.registers.e, self.registers.a, step), // Load A into E
            // Ox6X - Loads
            0x60 => Self::ld(&mut self.registers.h, self.registers.b, step), // Load B into H
            0x61 => Self::ld(&mut self.registers.h, self.registers.c, step), // Load C into H
            0x62 => Self::ld(&mut self.registers.h, self.registers.d, step), // Load D into H
            0x63 => Self::ld(&mut self.registers.h, self.registers.e, step), // Load E into H
            0x64 => { /* Basically does nothing */ true },                       // Load H into H
            0x65 => Self::ld(&mut self.registers.h, self.registers.l, step), // Load L into H
            0x66 => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.h, v, step) }, // Load (HL) into H
            0x67 => Self::ld(&mut self.registers.h, self.registers.a, step), // Load A into L
            0x68 => Self::ld(&mut self.registers.l, self.registers.b, step), // Load B into L
            0x69 => Self::ld(&mut self.registers.l, self.registers.c, step), // Load C into L
            0x6A => Self::ld(&mut self.registers.l, self.registers.d, step), // Load D into L
            0x6B => Self::ld(&mut self.registers.l, self.registers.e, step), // Load E into L
            0x6C => Self::ld(&mut self.registers.l, self.registers.h, step), // Load H into L
            0x6D => { /* Basically does nothing */ true },                       // Load L into L
            0x6E => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.l, v, step) }, // Load (HL) into L
            0x6F => Self::ld(&mut self.registers.l, self.registers.a, step), // Load A into L
            // Ox7X - Loads
            0x70 => self.ld_hl(mmu, self.registers.b, step), // Store B in the memory location specified by HL
            0x71 => self.ld_hl(mmu, self.registers.c, step), // Store C in the memory location specified by HL
            0x72 => self.ld_hl(mmu, self.registers.d, step), // Store D in the memory location specified by HL
            0x73 => self.ld_hl(mmu, self.registers.e, step), // Store E in the memory location specified by HL
            0x74 => self.ld_hl(mmu, self.registers.h, step), // Store H in the memory location specified by HL
            0x75 => self.ld_hl(mmu, self.registers.l, step), // Store L in the memory location specified by HL
            0x76 => self.halt(step),
            0x77 => self.ld_hl(mmu, self.registers.a, step), // Store A at the memory location specified by HL
            0x78 => Self::ld(&mut self.registers.a, self.registers.b, step), // Load B into A
            0x79 => Self::ld(&mut self.registers.a, self.registers.c, step), // Load C into A
            0x7A => Self::ld(&mut self.registers.a, self.registers.d, step), // Load D into A
            0x7B => Self::ld(&mut self.registers.a, self.registers.e, step), // Load E into A 
            0x7C => Self::ld(&mut self.registers.a, self.registers.h, step), // Load H into A
            0x7D => Self::ld(&mut self.registers.a, self.registers.l, step), // Load L into A
            0x7E => { let v = self.fetch_hli(mmu); Self::ld(&mut self.registers.a, v, step) }, // Load (HL) into A
            0x7F => { /* Basically does nothing */ true },                       // Load A into A
            // Ox8X - Maths
            0x80 => self.add(self.registers.b, step), // ADD B
            0x81 => self.add(self.registers.c, step), // ADD C
            0x82 => self.add(self.registers.d, step), // ADD D
            0x83 => self.add(self.registers.e, step), // ADD E
            0x84 => self.add(self.registers.h, step), // ADD H
            0x85 => self.add(self.registers.l, step), // ADD L
            0x86 => self.add(self.fetch_hli(mmu), step), // ADD (HL)
            0x87 => self.add(self.registers.a, step), // ADD A
            0x88 => self.adc(self.registers.b, step), // ADC B
            0x89 => self.adc(self.registers.c, step), // ADC C
            0x8A => self.adc(self.registers.d, step), // ADC D
            0x8B => self.adc(self.registers.e, step), // ADC E
            0x8C => self.adc(self.registers.h, step), // ADC H
            0x8D => self.adc(self.registers.l, step), // ADC L
            0x8E => self.adc(self.fetch_hli(mmu), step), // ADC (HL)
            0x8F => self.adc(self.registers.a, step), // ADC A
            // Ox9X - Maths
            0x90 => self.sub(self.registers.b, step), // SUB B
            0x91 => self.sub(self.registers.c, step), // SUB C
            0x92 => self.sub(self.registers.d, step), // SUB D
            0x93 => self.sub(self.registers.e, step), // SUB E
            0x94 => self.sub(self.registers.h, step), // SUB H
            0x95 => self.sub(self.registers.l, step), // SUB L
            0x96 => self.sub(self.fetch_hli(mmu), step), // SUB (HL)
            0x97 => self.sub(self.registers.a, step), // SUB A
            0x98 => self.sbc(self.registers.b, step), // SBC B
            0x99 => self.sbc(self.registers.c, step), // SBC C
            0x9A => self.sbc(self.registers.d, step), // SBC D
            0x9B => self.sbc(self.registers.e, step), // SBC E
            0x9C => self.sbc(self.registers.h, step), // SBC H
            0x9D => self.sbc(self.registers.l, step), // SBC L
            0x9E => self.sbc(self.fetch_hli(mmu), step), // SBC (HL)
            0x9F => self.sbc(self.registers.a, step), // SBC A
            // OxAX - Maths
            0xA0 => self.and(self.registers.b, step), // AND B
            0xA1 => self.and(self.registers.c, step), // AND C
            0xA2 => self.and(self.registers.d, step), // AND D
            0xA3 => self.and(self.registers.e, step), // AND E
            0xA4 => self.and(self.registers.h, step), // AND H
            0xA5 => self.and(self.registers.l, step), // AND L
            0xA6 => self.and(self.fetch_hli(mmu), step), // AND (HL)
            0xA7 => self.and(self.registers.a, step), // AND A
            0xA8 => self.xor(self.registers.b, step), // XOR B
            0xA9 => self.xor(self.registers.c, step), // XOR C
            0xAA => self.xor(self.registers.d, step), // XOR D
            0xAB => self.xor(self.registers.e, step), // XOR E
            0xAC => self.xor(self.registers.h, step), // XOR H
            0xAD => self.xor(self.registers.l, step), // XOR L
            0xAE => self.xor(self.fetch_hli(mmu), step), // XOR (HL)
            0xAF => self.xor(self.registers.a, step), // XOR A
            // OxBX - Maths
            0xB0 => self.or(self.registers.b, step),  // OR B
            0xB1 => self.or(self.registers.c, step),  // OR C
            0xB2 => self.or(self.registers.d, step),  // OR D
            0xB3 => self.or(self.registers.e, step),  // OR E
            0xB4 => self.or(self.registers.h, step),  // OR H
            0xB5 => self.or(self.registers.l, step),  // OR L
            0xB6 => self.or(self.fetch_hli(mmu), step),  // OR (HL)
            0xB7 => self.or(self.registers.a, step),  // OR A
            0xB8 => self.cp(self.registers.b, step),  // CP B
            0xB9 => self.cp(self.registers.c, step),  // CP C
            0xBA => self.cp(self.registers.d, step),  // CP D
            0xBB => self.cp(self.registers.e, step),  // CP E
            0xBC => self.cp(self.registers.h, step),  // CP H
            0xBD => self.cp(self.registers.l, step),  // CP L
            0xBE => self.cp(self.fetch_hli(mmu), step),  // CP (HL)
            0xBF => self.cp(self.registers.a, step),  // CP A
            // OxCX
            0xC0 => self.ret_c(mmu, !self.registers.f.zero, step), // RET NZ
            0xC1 => self.pop(mmu, CPUReg::B, CPUReg::C, step), // POP BC
            0xC2 => self.jp_c(mmu, !self.registers.f.zero, step), // JP NZ, a16
            0xC3 => self.jp(mmu, step), // JP a16
            0xC4 => self.call_c(mmu, !self.registers.f.zero, step), // CALL NZ, a16
            0xC5 => self.push(mmu, self.registers.b, self.registers.c, step), // PUSH BC
            0xC6 => { let value = self.fetch8(mmu); self.add(value, step) }, // ADD d8
            0xC7 => self.rst(mmu, 0x00, step), // Restart 0 -> 0x00
            0xC8 => self.ret_c(mmu, self.registers.f.zero, step), // RET Z
            0xC9 => self.ret(mmu, step), // RET
            0xCA => self.jp_c(mmu, self.registers.f.zero, step), // JP Z, a16
            0xCB => {
                // Subcommand prefix
                let mut result = false;
                match step {
                    1 => {
                        self.current_sub_cmd = self.fetch8(mmu); // Fetch an instruction
                        let info = InstInfo::decode_sub(self.current_sub_cmd);

                        // Execute right away if it's only 2 M-cycles long
                        if info.cl.full == 2 {
                            result = self.match_sub_cmd(mmu, self.current_sub_cmd, step); 
                        }
                    }
                    2..=3 => {
                        result = self.match_sub_cmd(mmu, self.current_sub_cmd, step-1)
                    }
                    _ => unreachable!("{}", step)
                }
                result
            },
            0xCC => self.call_c(mmu, self.registers.f.zero, step), // CALL Z, a16
            0xCD => self.call(mmu, step), // CALL a16
            0xCE => { let value = self.fetch8(mmu); self.adc(value, step) }, // ADC A, d8
            0xCF => self.rst(mmu, 0x08, step), // Restart 1 -> 0x08
            // 0xDX
            0xD0 => self.ret_c(mmu, !self.registers.f.carry, step),
            0xD1 => self.pop(mmu, CPUReg::D, CPUReg::E, step), // POP DE
            0xD2 => self.jp_c(mmu, !self.registers.f.carry, step), // JP NC, a16
            0xD3 => { /* NONEXISTENT */ true },
            0xD4 => self.call_c(mmu, !self.registers.f.carry, step), // CALL NC, a16
            0xD5 => self.push(mmu, self.registers.d, self.registers.e, step), // PUSH DE
            0xD6 => { let value = self.fetch8(mmu); self.sub(value, step) }, // SUB d8
            0xD7 => self.rst(mmu, 0x10, step), // Restart 2 -> 0x10
            0xD8 => self.ret_c(mmu, self.registers.f.carry, step), // RET C
            0xD9 => self.reti(mmu, step), // RETI
            0xDA => self.jp_c(mmu, self.registers.f.carry, step), // JP C, a16
            0xDB => { /* NONEXISTENT */ true },
            0xDC => self.call_c(mmu, self.registers.f.carry, step), // CALL C, a16
            0xDD => { /* NONEXISTENT */ true },
            0xDE => { let value = self.fetch8(mmu); self.sbc(value, step) }, // SBC
            0xDF => self.rst(mmu, 0x18, step), // Restart 3 -> 0x18
            // 0xEX
            0xE0 => {
                // LD (a8), A [3 M-cycles total]
                match step {
                    1 => { self.op_buffer.lower = self.fetch8(mmu); },
                    2 => {
                        let address = 0xFF00 + self.op_buffer.lower as u16;
                        mmu.cpu_write8(address, self.registers.a);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0xE1 => self.pop(mmu, CPUReg::H, CPUReg::L, step), // POP HL
            0xE2 => {
                // LD (C), A [2 M-cycles total]
                // Store A at the address a16
                match step {
                    1 => {
                        let address = 0xFF00 + self.registers.c as u16;
                        mmu.cpu_write8(address, self.registers.a);
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0xE3 => { /* NONEXISTENT */ true },
            0xE4 => { /* NONEXISTENT */ true },
            0xE5 => self.push(mmu, self.registers.h, self.registers.l, step), // PUSH HL
            0xE6 => { let value = self.fetch8(mmu); self.and(value, step) }, // AND d8
            0xE7 => self.rst(mmu, 0x20, step), // Restart 4 -> 0x20
            0xE8 => self.add_sp_e8(mmu, step), // ADD SP, s8
            0xE9 => self.jp_hl(step), // Jump HL
            0xEA => {
                // LD (a16), A [4 M-cycles total]
                match step {
                    1 => { self.op_buffer.lower = self.fetch8(mmu); },
                    2 => { self.op_buffer.upper = self.fetch8(mmu); },
                    3 => {
                        mmu.cpu_write8(self.op_buffer.into(), self.registers.a);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0xEB => { /* NONEXISTENT */ true },
            0xEC => { /* NONEXISTENT */ true },
            0xED => { /* NONEXISTENT */ true },
            0xEE => { let value = self.fetch8(mmu); self.xor(value, step) }, // XOR d8
            0xEF => self.rst(mmu, 0x28, step), // Restart 5 -> 0x28
            // 0xFX
            0xF0 => {
                // LD A, (a8) [3 M-cycles total]
                match step {
                    1 => { self.op_buffer.lower = self.fetch8(mmu); },
                    2 => {
                        let address = 0xFF00 + self.op_buffer.lower as u16;
                        self.registers.a = mmu.cpu_read8(address);
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0xF1 => self.pop_af(mmu, step), // POP AF
            0xF2 => {
                // LD A, (C) [2 M-cycles total]
                match step {
                    1 => {
                        let address = 0xFF00 + self.registers.c as u16;
                        self.registers.a = mmu.cpu_read8(address);
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0xF3 => self.di(step), // Disable Interrupts
            0xF4 => { /* NONEXISTENT */ true },
            0xF5 => self.push(mmu, self.registers.a, self.registers.f.into(), step), // PUSH AF
            0xF6 => { let value = self.fetch8(mmu); self.or(value, step) }, // OR d8
            0xF7 => self.rst(mmu, 0x30, step), // Restart 6 -> 0x30
            0xF8 => self.ld_hl_sp_e8(mmu, step), // LD HL, SP+s8
            0xF9 => {
                // LD SP, HL [2 M-cycles total]
                // Load HL into SP
                match step {
                    1 => {
                        self.sp = self.registers.get_hl();
                        true
                    },
                    _ => unreachable!("{}", step)
                }
            },
            0xFA => {
                // LD A, (a16) [4 M-cycles total]
                // Load into A contents of address a16
                match step {
                    1 => { self.op_buffer.lower = self.fetch8(mmu); },
                    2 => { self.op_buffer.upper = self.fetch8(mmu); },
                    3 => {
                        self.registers.a = mmu.cpu_read8(self.op_buffer.into());
                        return true;
                    },
                    _ => unreachable!("{}", step)
                }
                false
            },
            0xFB => self.ei(step), // Enable Interrupts
            0xFC => { /* NONEXISTENT */ true },
            0xFD => { /* NONEXISTENT */ true },
            0xFE => { let value = self.fetch8(mmu); self.cp(value, step) }, // CP d8
            0xFF => self.rst(mmu, 0x38, step), // Restart 7 -> 0x38
        };
        
        has_finished
    }
    /// Additional huge CPU switch case
    fn match_sub_cmd(&mut self, mmu: &mut Mmu, cmd: u8, step: u8) -> bool { 
        match cmd {
            // 0x0X
            0x00 => self.rlc(CPUReg::B, step),
            0x01 => self.rlc(CPUReg::C, step),
            0x02 => self.rlc(CPUReg::D, step),
            0x03 => self.rlc(CPUReg::E, step),
            0x04 => self.rlc(CPUReg::H, step),
            0x05 => self.rlc(CPUReg::L, step),
            0x06 => self.sub_inst_hl(mmu, SubInst::Rlc, step),
            0x07 => self.rlc(CPUReg::A, step),
            0x08 => self.rrc(CPUReg::B, step),
            0x09 => self.rrc(CPUReg::C, step),
            0x0A => self.rrc(CPUReg::D, step),
            0x0B => self.rrc(CPUReg::E, step),
            0x0C => self.rrc(CPUReg::H, step),
            0x0D => self.rrc(CPUReg::L, step),
            0x0E => self.sub_inst_hl(mmu, SubInst::Rrc, step),
            0x0F => self.rrc(CPUReg::A, step),
            // 0x1X
            0x10 => self.rl(CPUReg::B, step),
            0x11 => self.rl(CPUReg::C, step),
            0x12 => self.rl(CPUReg::D, step),
            0x13 => self.rl(CPUReg::E, step),
            0x14 => self.rl(CPUReg::H, step),
            0x15 => self.rl(CPUReg::L, step),
            0x16 => self.sub_inst_hl(mmu, SubInst::Rl, step),
            0x17 => self.rl(CPUReg::A, step),
            0x18 => self.rr(CPUReg::B, step),
            0x19 => self.rr(CPUReg::C, step),
            0x1A => self.rr(CPUReg::D, step),
            0x1B => self.rr(CPUReg::E, step),
            0x1C => self.rr(CPUReg::H, step),
            0x1D => self.rr(CPUReg::L, step),
            0x1E => self.sub_inst_hl(mmu, SubInst::Rr, step),
            0x1F => self.rr(CPUReg::A, step),
            // 0x2X
            0x20 => self.sla(CPUReg::B, step),
            0x21 => self.sla(CPUReg::C, step),
            0x22 => self.sla(CPUReg::D, step),
            0x23 => self.sla(CPUReg::E, step),
            0x24 => self.sla(CPUReg::H, step),
            0x25 => self.sla(CPUReg::L, step),
            0x26 => self.sub_inst_hl(mmu, SubInst::Sla, step),
            0x27 => self.sla(CPUReg::A, step),
            0x28 => self.sra(CPUReg::B, step),
            0x29 => self.sra(CPUReg::C, step),
            0x2A => self.sra(CPUReg::D, step),
            0x2B => self.sra(CPUReg::E, step),
            0x2C => self.sra(CPUReg::H, step),
            0x2D => self.sra(CPUReg::L, step),
            0x2E => self.sub_inst_hl(mmu, SubInst::Sra, step),
            0x2F => self.sra(CPUReg::A, step),
            // 0x3X
            0x30 => self.swap(CPUReg::B, step),
            0x31 => self.swap(CPUReg::C, step),
            0x32 => self.swap(CPUReg::D, step),
            0x33 => self.swap(CPUReg::E, step),
            0x34 => self.swap(CPUReg::H, step),
            0x35 => self.swap(CPUReg::L, step),
            0x36 => self.sub_inst_hl(mmu, SubInst::Swap, step),
            0x37 => self.swap(CPUReg::A, step),
            0x38 => self.srl(CPUReg::B, step),
            0x39 => self.srl(CPUReg::C, step),
            0x3A => self.srl(CPUReg::D, step),
            0x3B => self.srl(CPUReg::E, step),
            0x3C => self.srl(CPUReg::H, step),
            0x3D => self.srl(CPUReg::L, step),
            0x3E => self.sub_inst_hl(mmu, SubInst::Srl, step),
            0x3F => self.srl(CPUReg::A, step),
            // 0x4X
            0x40 => self.bit(self.registers.b, 0, step),
            0x41 => self.bit(self.registers.c, 0, step),
            0x42 => self.bit(self.registers.d, 0, step),
            0x43 => self.bit(self.registers.e, 0, step),
            0x44 => self.bit(self.registers.h, 0, step),
            0x45 => self.bit(self.registers.l, 0, step),
            0x46 => self.sub_inst_hl(mmu, SubInst::Bit(0), step),
            0x47 => self.bit(self.registers.a, 0, step),
            0x48 => self.bit(self.registers.b, 1, step),
            0x49 => self.bit(self.registers.c, 1, step),
            0x4A => self.bit(self.registers.d, 1, step),
            0x4B => self.bit(self.registers.e, 1, step),
            0x4C => self.bit(self.registers.h, 1, step),
            0x4D => self.bit(self.registers.l, 1, step),
            0x4E => self.sub_inst_hl(mmu, SubInst::Bit(1), step),
            0x4F => self.bit(self.registers.a, 1, step),
            // 0x5X
            0x50 => self.bit(self.registers.b, 2, step),
            0x51 => self.bit(self.registers.c, 2, step),
            0x52 => self.bit(self.registers.d, 2, step),
            0x53 => self.bit(self.registers.e, 2, step),
            0x54 => self.bit(self.registers.h, 2, step),
            0x55 => self.bit(self.registers.l, 2, step),
            0x56 => self.sub_inst_hl(mmu, SubInst::Bit(2), step),
            0x57 => self.bit(self.registers.a, 2, step),
            0x58 => self.bit(self.registers.b, 3, step),
            0x59 => self.bit(self.registers.c, 3, step),
            0x5A => self.bit(self.registers.d, 3, step),
            0x5B => self.bit(self.registers.e, 3, step),
            0x5C => self.bit(self.registers.h, 3, step),
            0x5D => self.bit(self.registers.l, 3, step),
            0x5E => self.sub_inst_hl(mmu, SubInst::Bit(3), step),
            0x5F => self.bit(self.registers.a, 3, step),
            // 0x6X
            0x60 => self.bit(self.registers.b, 4, step),
            0x61 => self.bit(self.registers.c, 4, step),
            0x62 => self.bit(self.registers.d, 4, step),
            0x63 => self.bit(self.registers.e, 4, step),
            0x64 => self.bit(self.registers.h, 4, step),
            0x65 => self.bit(self.registers.l, 4, step),
            0x66 => self.sub_inst_hl(mmu, SubInst::Bit(4), step),
            0x67 => self.bit(self.registers.a, 4, step),
            0x68 => self.bit(self.registers.b, 5, step),
            0x69 => self.bit(self.registers.c, 5, step),
            0x6A => self.bit(self.registers.d, 5, step),
            0x6B => self.bit(self.registers.e, 5, step),
            0x6C => self.bit(self.registers.h, 5, step),
            0x6D => self.bit(self.registers.l, 5, step),
            0x6E => self.sub_inst_hl(mmu, SubInst::Bit(5), step),
            0x6F => self.bit(self.registers.a, 5, step),
            // 0x7X
            0x70 => self.bit(self.registers.b, 6, step),
            0x71 => self.bit(self.registers.c, 6, step),
            0x72 => self.bit(self.registers.d, 6, step),
            0x73 => self.bit(self.registers.e, 6, step),
            0x74 => self.bit(self.registers.h, 6, step),
            0x75 => self.bit(self.registers.l, 6, step),
            0x76 => self.sub_inst_hl(mmu, SubInst::Bit(6), step),
            0x77 => self.bit(self.registers.a, 6, step),
            0x78 => self.bit(self.registers.b, 7, step),
            0x79 => self.bit(self.registers.c, 7, step),
            0x7A => self.bit(self.registers.d, 7, step),
            0x7B => self.bit(self.registers.e, 7, step),
            0x7C => self.bit(self.registers.h, 7, step),
            0x7D => self.bit(self.registers.l, 7, step),
            0x7E => self.sub_inst_hl(mmu, SubInst::Bit(7), step),
            0x7F => self.bit(self.registers.a, 7, step),
            // 0x8X
            0x80 => self.res(CPUReg::B, 0, step),
            0x81 => self.res(CPUReg::C, 0, step),
            0x82 => self.res(CPUReg::D, 0, step),
            0x83 => self.res(CPUReg::E, 0, step),
            0x84 => self.res(CPUReg::H, 0, step),
            0x85 => self.res(CPUReg::L, 0, step),
            0x86 => self.sub_inst_hl(mmu, SubInst::Res(0), step),
            0x87 => self.res(CPUReg::A, 0, step),
            0x88 => self.res(CPUReg::B, 1, step),
            0x89 => self.res(CPUReg::C, 1, step),
            0x8A => self.res(CPUReg::D, 1, step),
            0x8B => self.res(CPUReg::E, 1, step),
            0x8C => self.res(CPUReg::H, 1, step),
            0x8D => self.res(CPUReg::L, 1, step),
            0x8E => self.sub_inst_hl(mmu, SubInst::Res(1), step),
            0x8F => self.res(CPUReg::A, 1, step),
            // 0x9X
            0x90 => self.res(CPUReg::B, 2, step),
            0x91 => self.res(CPUReg::C, 2, step),
            0x92 => self.res(CPUReg::D, 2, step),
            0x93 => self.res(CPUReg::E, 2, step),
            0x94 => self.res(CPUReg::H, 2, step),
            0x95 => self.res(CPUReg::L, 2, step),
            0x96 => self.sub_inst_hl(mmu, SubInst::Res(2), step),
            0x97 => self.res(CPUReg::A, 2, step),
            0x98 => self.res(CPUReg::B, 3, step),
            0x99 => self.res(CPUReg::C, 3, step),
            0x9A => self.res(CPUReg::D, 3, step),
            0x9B => self.res(CPUReg::E, 3, step),
            0x9C => self.res(CPUReg::H, 3, step),
            0x9D => self.res(CPUReg::L, 3, step),
            0x9E => self.sub_inst_hl(mmu, SubInst::Res(3), step),
            0x9F => self.res(CPUReg::A, 3, step),
            // 0xAX
            0xA0 => self.res(CPUReg::B, 4, step),
            0xA1 => self.res(CPUReg::C, 4, step),
            0xA2 => self.res(CPUReg::D, 4, step),
            0xA3 => self.res(CPUReg::E, 4, step),
            0xA4 => self.res(CPUReg::H, 4, step),
            0xA5 => self.res(CPUReg::L, 4, step),
            0xA6 => self.sub_inst_hl(mmu, SubInst::Res(4), step),
            0xA7 => self.res(CPUReg::A, 4, step),
            0xA8 => self.res(CPUReg::B, 5, step),
            0xA9 => self.res(CPUReg::C, 5, step),
            0xAA => self.res(CPUReg::D, 5, step),
            0xAB => self.res(CPUReg::E, 5, step),
            0xAC => self.res(CPUReg::H, 5, step),
            0xAD => self.res(CPUReg::L, 5, step),
            0xAE => self.sub_inst_hl(mmu, SubInst::Res(5), step),
            0xAF => self.res(CPUReg::A, 5, step),
            // 0xBX
            0xB0 => self.res(CPUReg::B, 6, step),
            0xB1 => self.res(CPUReg::C, 6, step),
            0xB2 => self.res(CPUReg::D, 6, step),
            0xB3 => self.res(CPUReg::E, 6, step),
            0xB4 => self.res(CPUReg::H, 6, step),
            0xB5 => self.res(CPUReg::L, 6, step),
            0xB6 => self.sub_inst_hl(mmu, SubInst::Res(6), step),
            0xB7 => self.res(CPUReg::A, 6, step),
            0xB8 => self.res(CPUReg::B, 7, step),
            0xB9 => self.res(CPUReg::C, 7, step),
            0xBA => self.res(CPUReg::D, 7, step),
            0xBB => self.res(CPUReg::E, 7, step),
            0xBC => self.res(CPUReg::H, 7, step),
            0xBD => self.res(CPUReg::L, 7, step),
            0xBE => self.sub_inst_hl(mmu, SubInst::Res(7), step),
            0xBF => self.res(CPUReg::A, 7, step),
            // 0xCX
            0xC0 => self.set(CPUReg::B, 0, step),
            0xC1 => self.set(CPUReg::C, 0, step),
            0xC2 => self.set(CPUReg::D, 0, step),
            0xC3 => self.set(CPUReg::E, 0, step),
            0xC4 => self.set(CPUReg::H, 0, step),
            0xC5 => self.set(CPUReg::L, 0, step),
            0xC6 => self.sub_inst_hl(mmu, SubInst::Set(0), step),
            0xC7 => self.set(CPUReg::A, 0, step),
            0xC8 => self.set(CPUReg::B, 1, step),
            0xC9 => self.set(CPUReg::C, 1, step),
            0xCA => self.set(CPUReg::D, 1, step),
            0xCB => self.set(CPUReg::E, 1, step),
            0xCC => self.set(CPUReg::H, 1, step),
            0xCD => self.set(CPUReg::L, 1, step),
            0xCE => self.sub_inst_hl(mmu, SubInst::Set(1), step),
            0xCF => self.set(CPUReg::A, 1, step),
            // 0xDX
            0xD0 => self.set(CPUReg::B, 2, step),
            0xD1 => self.set(CPUReg::C, 2, step),
            0xD2 => self.set(CPUReg::D, 2, step),
            0xD3 => self.set(CPUReg::E, 2, step),
            0xD4 => self.set(CPUReg::H, 2, step),
            0xD5 => self.set(CPUReg::L, 2, step),
            0xD6 => self.sub_inst_hl(mmu, SubInst::Set(2), step),
            0xD7 => self.set(CPUReg::A, 2, step),
            0xD8 => self.set(CPUReg::B, 3, step),
            0xD9 => self.set(CPUReg::C, 3, step),
            0xDA => self.set(CPUReg::D, 3, step),
            0xDB => self.set(CPUReg::E, 3, step),
            0xDC => self.set(CPUReg::H, 3, step),
            0xDD => self.set(CPUReg::L, 3, step),
            0xDE => self.sub_inst_hl(mmu, SubInst::Set(3), step),
            0xDF => self.set(CPUReg::A, 3, step),
            // 0xEX
            0xE0 => self.set(CPUReg::B, 4, step),
            0xE1 => self.set(CPUReg::C, 4, step),
            0xE2 => self.set(CPUReg::D, 4, step),
            0xE3 => self.set(CPUReg::E, 4, step),
            0xE4 => self.set(CPUReg::H, 4, step),
            0xE5 => self.set(CPUReg::L, 4, step),
            0xE6 => self.sub_inst_hl(mmu, SubInst::Set(4), step),
            0xE7 => self.set(CPUReg::A, 4, step),
            0xE8 => self.set(CPUReg::B, 5, step),
            0xE9 => self.set(CPUReg::C, 5, step),
            0xEA => self.set(CPUReg::D, 5, step),
            0xEB => self.set(CPUReg::E, 5, step),
            0xEC => self.set(CPUReg::H, 5, step),
            0xED => self.set(CPUReg::L, 5, step),
            0xEE => self.sub_inst_hl(mmu, SubInst::Set(5), step),
            0xEF => self.set(CPUReg::A, 5, step),
            // 0xFX
            0xF0 => self.set(CPUReg::B, 6, step),
            0xF1 => self.set(CPUReg::C, 6, step),
            0xF2 => self.set(CPUReg::D, 6, step),
            0xF3 => self.set(CPUReg::E, 6, step),
            0xF4 => self.set(CPUReg::H, 6, step),
            0xF5 => self.set(CPUReg::L, 6, step),
            0xF6 => self.sub_inst_hl(mmu, SubInst::Set(6), step),
            0xF7 => self.set(CPUReg::A, 6, step),
            0xF8 => self.set(CPUReg::B, 7, step),
            0xF9 => self.set(CPUReg::C, 7, step),
            0xFA => self.set(CPUReg::D, 7, step),
            0xFB => self.set(CPUReg::E, 7, step),
            0xFC => self.set(CPUReg::H, 7, step),
            0xFD => self.set(CPUReg::L, 7, step),
            0xFE => self.sub_inst_hl(mmu, SubInst::Set(7), step),
            0xFF => self.set(CPUReg::A, 7, step),
        }
    }
}