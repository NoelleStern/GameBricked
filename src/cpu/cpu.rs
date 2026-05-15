use crate::{
    cpu::instruction_info::InstInfo, memory::mmu::MMU, rendering::palette::Palette,
    rendering::ppu::{BG_TILE_MAP_0, FRAME_CYCLES, SCREEN_HEIGHT, SCREEN_WIDTH},
    cpu::registers::Registers
};


///
/// Behold the DMG-CPU aka Sharp SM83 - a spiritual relative of fan favorite Zilog Z80 and Intel 8080.
/// 
///     Yep, the Sinclair ZX Spectrum Zilog Z80.
/// 
///     https://rgbds.gbdev.io/docs/v1.0.1/gbz80.7
///     https://meganesu.github.io/generate-gb-opcodes/
///     http://www.codeslinger.co.uk/pages/projects/gameboy/files/GB.pdf
///


#[derive(Default)]
pub struct CPU {
    pub registers: Registers,   // 8bit Registers
    pub pc: u16,                // Program Counter - points to the next instruction in the memory.  (Technically is a register)
    pub sp: u16,                // Stack Pointer - points to where the top of the stack is.         (Technically is a register)
    pub mmu: MMU,               // Memory Management Unit
    pub ime: bool,              // Interrupt Master Enable
    pub halt: bool,             // HALT
}
impl CPU {
    pub fn debug_info(&self) -> String {
        format!(
            // "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} SP:{:04X} PC:{:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
            "A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})",
            self.registers.a, u8::from(self.registers.f),
            self.registers.b, self.registers.c,
            self.registers.d, self.registers.e,
            self.registers.h, self.registers.l,
            self.sp, self.pc, 
            self.mmu.read8(self.pc), self.mmu.read8(self.pc+1),
            self.mmu.read8(self.pc+2), self.mmu.read8(self.pc+3),
        )
    }

    pub fn inc_sp(&mut self) { self.sp = self.sp.wrapping_add(1); }
    pub fn dec_sp(&mut self) { self.sp = self.sp.wrapping_sub(1); }

    fn fetch8(&self) -> u8      { self.mmu.read8(self.pc.wrapping_sub(1))       } // Get u8 value at next PC address
    fn fetch16(&self) -> u16    { self.mmu.read16_rev(self.pc.wrapping_sub(2))  } // Get u16 value at next PC address
    fn fetch_bca(&self) -> u8   { self.mmu.read8(self.registers.get_bc())       } // Get u8 value at BC address
    fn fetch_dea(&self) -> u8   { self.mmu.read8(self.registers.get_de())       } // Get u8 value at DE address
    fn fetch_hla(&self) -> u8   { self.mmu.read8(self.registers.get_hl())       } // Get u8 value at HL address
  
    fn carry_overflow_check(v1: u8, v2: u8, cf: bool) -> bool { (v1 as u16 + v2 as u16 + cf as u16) > 0xFF }    // Carry overflow (with optional C)
    fn carry_borrow_check(v1: u8, v2: u8, cf: bool) -> bool { ((v2 & 0x0F) + cf as u8) > (v1 & 0x0F) }          // Carry borrow (with optional C)
    fn hc_overflow_check(v1: u8, v2: u8, cf: Option<bool>) -> bool {                                            // Half carry overflow from bit 3 check
       ((v1 & 0x0F) + (v2 & 0x0F) + cf.unwrap_or(false) as u8) > 0x0F
    }
    fn hc_borrow_check(v1: u8, v2: u8, cf: Option<bool>) -> bool {                                              // Half carry borrow from bit 4 check
        ((v2 & 0x0F) + cf.unwrap_or(false) as u8) > (v1 & 0x0F)
    }        
    fn hc_overflow_check16(v1: u16, v2: u16) -> bool { (v1 + v2) > 0xFF }                                       // Half carry u16 overflow check

    // https://rgbds.gbdev.io/docs/v1.0.1/gbz80.7
    fn adc(&mut self, value: u8) -> u8 {                        // ADd + Carry
        let result = self.registers.a.wrapping_add(value).wrapping_add(self.registers.f.carry as u8);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check(self.registers.a, value, Some(self.registers.f.carry));
        self.registers.f.carry = Self::carry_overflow_check(self.registers.a, value, self.registers.f.carry);

        result
    }
    fn add(&mut self, value: u8) -> u8 {                        // ADD
        let (result, overflow_flag) = self.registers.a.overflowing_add(value);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check(self.registers.a, value, None);
        self.registers.f.carry = overflow_flag;

        result
    }
    fn add16(&mut self, v1: u16, v2: u16) -> u16 {              // ADD u16
        let (result, overflow_flag) = v1.overflowing_add(v2);

        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check16(v1, v2);
        self.registers.f.carry = overflow_flag;

        result
    }
    fn add_sp_e8(&mut self) -> u16 {                            // ADD e8 to SP
        let value = self.fetch8();
        
        let low = (self.sp & 0xFF) as u8;

        self.registers.f.zero = false;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check(low , value, None);
        self.registers.f.carry = Self::carry_overflow_check(low, value, false);

        self.jr(self.fetch8())
    }
    fn and(&mut self, value: u8) -> u8 {                        // bitwise AND
        let result = self.registers.a & value;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = true;
        self.registers.f.carry = false;

        result
    }
    fn bit(&mut self, value: u8, n: u8) {                       // test a BIT in a register, set the zero flag if the bit isn't set
        let bit: bool = ((value >> n) & 1) != 0;

        self.registers.f.zero = !bit;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = true;
    }
    fn call(&mut self, address: u16) {                          // CALL 
        self.dec_sp();
        self.mmu.write8(self.sp, (self.pc >> 8) as u8);  // Upper
        self.dec_sp();
        self.mmu.write8(self.sp, (self.pc & 0xFF) as u8); // Lower

        self.pc = address;
    }
    fn call_c(&mut self, flag: bool, address: u16) -> bool {    // CALL Conditional (custom)
        if flag { self.call(address); }
        flag 
    }
    fn ccf(&mut self) {                                         // Complement Carry Flag (aka invert)
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = !self.registers.f.carry;
    }
    fn cp(&mut self, value: u8) {                               // ComPare, not CoPy!
        let result = self.registers.a == value;
        
        self.registers.f.zero = result;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;
    }
    fn cpl(&mut self, value: u8) -> u8 {                        // ComPLement accumulator (aka bitwise NOT)
        let result: u8 = !value;

        self.registers.f.subtract = true;
        self.registers.f.half_carry = true;

        result
    }
    fn daa(&mut self) -> u8 {                                   // Decimal Adjust Accumulator
        // https://rgbds.gbdev.io/docs/v1.0.1/gbz80.7#DAA
        let mut adjust: u8 = 0; // Initialize the adjustment to 0

        if self.registers.f.subtract { // After subtraction
            // If half_carry is set, add 0x06 to the adjustment
            if self.registers.f.half_carry {
                adjust += 0x06;
            }

            // If carry is set, add 0x60 to the adjustment
            if self.registers.f.carry {
                adjust += 0x60;
            }

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
        }
    }
    fn dec(&mut self, value: u8) -> u8 {                        // DECrement by 1
        let result = value.wrapping_sub(1);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = Self::hc_borrow_check(value, 1, None);

        result
    }
    fn di(&mut self) {                                          // Disable Interrupts
        self.ime = true;
    }
    fn ei(&mut self) {                                          // Enable Interrupts
        self.ime = false;
    }
    fn halt(&mut self) {                                        // HALT (enter CPU low-power consumption mode)
        self.halt = true;
    }
    fn inc(&mut self, value: u8) -> u8 {                        // INCrement by 1
        let result = value.wrapping_add(1);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = Self::hc_overflow_check(value, 1, None);

        result
    }
    fn jp(&mut self, value: u16) {                              // JumP to an address 
        self.pc = value;
    }
    fn jpc(&mut self, flag: bool, value: u16) -> bool {         // JP Conditional (custom)
        if flag { self.jp(value); }
        flag
    }
    fn jr(&mut self, value: u8) -> u16 {                        // Jump Relative
        let val = value as i8;
        let abs = val.abs() as u16;

        if val < 0 { self.pc.wrapping_sub(abs) }
        else { self.pc.wrapping_add(abs) }
    }
    fn jrc(&mut self, flag: bool, value: u8) -> bool {          // JR Conditional (custom)
        if flag { self.pc = self.jr(value); }
        flag
    }
    // ld is out of here           d                             // LoaD
    fn ld16(&mut self, address: u16, value: u16) {              // LoaD u16
        let upper = (value >> 8) as u8;
        let lower = (value & 0x0F) as u8 ;
        self.mmu.write8(address, lower);
        self.mmu.write8(address+1, upper);
    }
    fn ldh(&mut self, value: u8) {                              // LoaD Hram
        // TODO
    }
    // nop is out of here                                       // No OPeration
    fn or(&mut self, value: u8) -> u8 {                         // bitwise OR
        let result = self.registers.a | value;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;
        
        result
    }
    fn pop_af(&mut self) -> u16 {                               // POP AF
        let result = self.pop();
        
        self.registers.f.zero           = ((result >> 7) & 1) != 0;
        self.registers.f.subtract       = ((result >> 6) & 1) != 0;
        self.registers.f.half_carry     = ((result >> 5) & 1) != 0;
        self.registers.f.carry          = ((result >> 4) & 1) != 0;
        
        result
    }
    fn pop(&mut self) -> u16 {                                  // POP
        let lsb = self.mmu.read8(self.sp) as u16;
        self.inc_sp();
        let msb = self.mmu.read8(self.sp) as u16;
        self.inc_sp();

        (msb << 8) | lsb
    }
    fn push(&mut self, value: u16) {                            // PUSH
        self.dec_sp();
        self.mmu.write8(self.sp, ((value & 0xFF00) >> 8) as u8);
        self.dec_sp();
        self.mmu.write8(self.sp, (value & 0xFF) as u8);
    }
    fn res(&mut self, value: u8, n: u8) -> u8{                  // RESet a bit
        value & !(1 << n)
    }
    fn ret(&mut self) {                                         // RETurn from subroutine
        let lower = self.mmu.read8(self.sp);
        self.inc_sp();
        let upper = self.mmu.read8(self.sp);
        self.inc_sp();

        self.pc = (upper as u16) << 8 | lower as u16;
    }
    fn ret_c(&mut self, flag: bool) -> bool {                   // RETurn Conditional (custom)
        if flag { self.ret(); }
        flag
    }
    fn reti(&mut self) {                                        // RETurn from subroutine and enable Interrupts
        // TODO
    }
    fn rl(&mut self, value: u8) -> u8 {                         // Rotate Left through the carry flag
        let result = (value << 1) | (self.registers.f.carry as u8);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x80) != 0;

        result
    }
    fn rla(&mut self) -> u8 {                                   // Rotate Left A through the carry flag
        let result = (self.registers.a << 1) | (self.registers.f.carry as u8);

        self.registers.f.zero = false;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (self.registers.a & 0x80) != 0;

        result
    }
    fn rlc(&mut self, value: u8) -> u8 {                        // Rotate Left Circular
        let c = (value & 0x80) != 0;
        let result = (value << 1) | (c as u8);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = c;

        result
    }
    fn rlca(&mut self) -> u8 {                                  // Rotate Left Circular
        let c = (self.registers.a & 0x80) != 0;
        let result = (self.registers.a << 1) | (c as u8);

        self.registers.f.zero = false;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = c;

        result
    }
    fn rr(&mut self, value: u8) -> u8 {                         // Rotate Right
        let result = (value >> 1) | ((self.registers.f.carry as u8) << 7);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x01) != 0;

        result
    }
    fn rra(&mut self) -> u8 {                                   // Rotate Right A
        let result = (self.registers.a >> 1) | ((self.registers.f.carry as u8) << 7);

        self.registers.f.zero = false;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (self.registers.a & 0x01) != 0;

        result
    }
    fn rrc(&mut self, value: u8) -> u8 {                        // Rotate Right Circular
        let c = (value & 0x01) != 0;
        let result = (value >> 1) | ((c as u8) << 7);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = c;

        result
    }
    fn rrca(&mut self) -> u8 {                                  // Rotate Right Circular A
        let c = (self.registers.a & 0x01) != 0;
        let result = (self.registers.a >> 1) | ((c as u8) << 7);

        self.registers.f.zero = false;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = c;

        result
    }
    fn rst(&mut self, address: u16) {                           // ReStarT (this is a shorter and faster equivalent to CALL)
        self.dec_sp();
        self.mmu.write8(self.sp, (self.pc >> 8) as u8);
        self.dec_sp();
        self.mmu.write8(self.sp, (self.pc & 0x0F) as u8);
        self.pc = address;
    }
    fn sbc(&mut self, value: u8) -> u8 {                        // SuBtract with C
        let c = self.registers.f.carry;
        let result = self.registers.a.wrapping_sub(value).wrapping_sub(c as u8);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = Self::hc_borrow_check(self.registers.a, value, Some(c));
        self.registers.f.carry = Self::carry_borrow_check(self.registers.a, value, c);

        result
    }
    fn scf(&mut self) {                                         // Set Carry Flag
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = true;
    }
    fn set(&mut self, value: u8, n: u8) -> u8 {                 // SET a bit
        value | (1 << n)
    }
    fn sla(&mut self, value: u8) -> u8 {                        // Shift Left Arithmetically
        let result = value << 1;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x80) != 0;

        result
    }
    fn sra(&mut self, value: u8) -> u8 {                        // Shift Right Arithmetically
        let result = (value >> 1) | (value & 0x80);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x01) != 0;

        result
    }
    fn srl(&mut self, value: u8) -> u8 {                        // Shift Right Logically 
        let result = value >> 1;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = (value & 0x01) != 0;

        result
    }
    fn stop(&mut self) {                                        // STOP (enter CPU very low power mode)
        // TODO
    }
    fn sub(&mut self, value: u8) -> u8 {                        // SUBtract
        let (result, overflow_flag) = self.registers.a.overflowing_sub(value);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = true;
        self.registers.f.half_carry = Self::hc_borrow_check(self.registers.a, value, None);
        self.registers.f.carry = overflow_flag;
        
        result
    }
    fn swap(&mut self, value: u8) -> u8 {                       // SWAP
        let result = (value << 4) | (value >> 4);

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;

        result
    }
    fn xor(&mut self, value: u8) -> u8 {                        // bitwise XOR
        let result = self.registers.a ^ value;

        self.registers.f.zero = result == 0;
        self.registers.f.subtract = false;
        self.registers.f.half_carry = false;
        self.registers.f.carry = false;

        result
    }

    pub fn process_frame(&mut self) {
        if self.pc < 0x100 {
            let mut frame_cycles: usize = 0; // Cycles this frame
            while frame_cycles < FRAME_CYCLES {
                let t_cycles: usize = self.exec_next() as usize * 4; // Convert m-cycles to t-cycles
                self.mmu.ppu.tick(t_cycles); // Tick the PPU
                self.mmu.write8(0xFF44, self.mmu.ppu.ly); // Update LY in memory
                frame_cycles += t_cycles as usize;
            }
        } else {
            println!("done")
        }
    }

    pub fn get_frame(&mut self, framebuffer: &mut Vec<u32>, palette: &Palette) {
        let scy = self.mmu.read8(0xFF42) as usize;
        let scx = self.mmu.read8(0xFF43) as usize;

        for y in 0..SCREEN_HEIGHT {
            let ry = y + scy; // Real y
            for x in 0..SCREEN_WIDTH {
                let rx = x + scx; // Real x
                let buff_i = y * SCREEN_WIDTH + x;

                let tile_y = (ry/8) % 32;
                let tile_x = (rx/8) % 32;
                let screen_tile_id = (tile_y * 32) + tile_x;
                let tile_address = BG_TILE_MAP_0 + screen_tile_id;
                let tile_id = self.mmu.read8(tile_address as u16) as usize;
                let color = self.mmu.ppu.tile_set[tile_id][ry%8][rx%8];

                framebuffer[buff_i] = palette.match_pixel(color);
            }
        }
    }

    pub fn exec_next(&mut self) -> u8 {
        let cmd = self.mmu.read8(self.pc); // Fetch an instruction
        self.exec(cmd) // Execute the instruction
    }

    pub fn exec(&mut self, cmd: u8) -> u8 {
        // Get info
        let info = InstInfo::decode(cmd);
        let mut sub_info: Option<InstInfo<'_>> = None;
        
        // Do preparations
        self.mmu.boot_rom_unload(); // Swap boot rom with the actual one
        self.pc += info.bl as u16; // Update the PC

        // Check for subcommand
        if cmd == 0xCD {
            sub_info = Some( 
                InstInfo::decode_sub(self.mmu.read8(self.pc))
            )
        }

        // Execute
        let result = self.match_cmd(cmd);

        // Return cycle length
        match sub_info {
            Some(i) => i.cl.light, // light is always the same as full
            None => if result { info.cl.full } else { info.cl.light },
        }
    }

    pub fn match_cmd(&mut self, cmd: u8) -> bool {
        let mut result: bool = false;

        match cmd {
            // Ox0X
            0x00 => { /* NOP, does nothing */ },
            0x01 => self.registers.set_bc(self.fetch16()), // Load 2 bytes of data into BC
            0x02 => {
                // Store contents of A in the memory location specified by BC
                let address = self.registers.get_bc();
                self.mmu.write8(address, self.registers.a);
            },
            0x03 => self.registers.inc_bc(),                        // Increment BC by 1
            0x04 => self.registers.b = self.inc(self.registers.b),  // Increment B by 1
            0x05 => self.registers.b = self.dec(self.registers.b),  // Decrement B by 1
            0x06 => self.registers.b = self.fetch8(),               // Load d8 into B
            0x07 => self.registers.a = self.rlca(),                 // Rotate contents of A to the left
            0x08 => self.ld16(self.pc, self.sp),     // Store SP at (a16)
            0x09 => {
                // Add BC to HL, and store the result in HL
                let value = self.add16(
                    self.registers.get_hl(),
                    self.registers.get_bc()
                );
                self.registers.set_hl(value);
            },
            0x0A => self.registers.a = self.fetch_bca(),            // Load 8-bit contents of memory specified by BC into A
            0x0B => self.registers.dec_bc(),                        // Decrement BC by 1
            0x0C => self.registers.c = self.inc(self.registers.c),  // Increment C by 1
            0x0D => self.registers.c = self.dec(self.registers.c),  // Decrement C by 1
            0x0E => self.registers.c = self.fetch8(),               // Load d8 into C
            0x0F => self.registers.a = self.rrca(),                 // Rotate the contents of register A to the right
            // Ox1X
            0x10 => {
                unimplemented!("Ox10");
            },
            0x11 => self.registers.set_de(self.fetch16()), // Load 2 bytes of data into DE
            0x12 => {
                // Store contents of A in the memory location specified by DE
                let address = self.registers.get_de();
                self.mmu.write8(address, self.registers.a);
            },
            0x13 => self.registers.inc_de(),                        // Increment DE by 1
            0x14 => self.registers.d = self.inc(self.registers.d),  // Increment D by 1
            0x15 => self.registers.d = self.dec(self.registers.d),  // Decrement D by 1
            0x16 => self.registers.d = self.fetch8(),               // Load d8 into D
            0x17 => self.registers.a = self.rla(),                  // RLA
            0x18 => self.pc = self.jr(self.fetch8()),               // Jump i8 steps from the current address stored in the PC
            0x19 => {
                // Add DE to HL, and store the result in HL
                let value = self.add16(
                    self.registers.get_hl(),
                    self.registers.get_de()
                );
                self.registers.set_hl(value);
            },
            0x1A => self.registers.a = self.fetch_dea(),            // Load 8-bit contents of memory specified by DE into A
            0x1B => self.registers.dec_de(),                        // Decrement DE by 1
            0x1C => self.registers.e = self.inc(self.registers.e),  // Increment E by 1
            0x1D => self.registers.e = self.dec(self.registers.e),  // Decrement E by 1
            0x1E => self.registers.e = self.fetch8(),               // Load d8 into E
            0x1F => self.registers.a = self.cpl(self.registers.a),  // CPL
            // Ox2X
            0x20 => result = self.jrc(!self.registers.f.zero, self.fetch8()), // If Z flag is 0, JRC i8
            0x21 => self.registers.set_hl(self.fetch16()), // Load 2 bytes of data into HL
            0x22 => {
                // Store contents of A into the memory
                // location specified by HL then increment HL
                let address = self.registers.get_hl();
                self.mmu.write8(address, self.registers.a);
                self.registers.inc_hl();
            },
            0x23 => self.registers.inc_hl(),                        // Increment HL by 1
            0x24 => self.registers.h = self.inc(self.registers.h),  // Increment H by 1
            0x25 => self.registers.h = self.dec(self.registers.h),  // Decrement H by 1
            0x26 => self.registers.h = self.fetch8(),               // Load d8 into H
            0x27 => self.registers.a = self.daa(),                  // DAA
            0x28 => result = self.jrc(self.registers.f.zero, self.fetch8()), // If Z flag is 1, JRC i8
            0x29 => {
                // Add HL to Hl, and store the result in HL
                let value = self.add16(
                    self.registers.get_hl(),
                    self.registers.get_hl()
                );
                self.registers.set_hl(value);
            },
            0x2A => {
                // Load the contents of memory
                // specified by HL into A then increment HL
                let address = self.registers.get_hl();
                self.registers.a = self.mmu.read8(address);
                self.registers.inc_hl();
            },
            0x2B => self.registers.dec_hl(),                        // Decrement HL by 1
            0x2C => self.registers.l = self.inc(self.registers.l),  // Increment L by 1
            0x2D => self.registers.l = self.dec(self.registers.l),  // Decrement L by 1
            0x2E => self.registers.l = self.fetch8(),               // Load d8 into L
            0x2F => self.registers.a = self.cpl(self.registers.a),  // CPL
            // Ox3X
            0x30 => result = self.jrc(!self.registers.f.carry, self.fetch8()), // If C flag is 0, JR i8
            0x31 => self.sp = self.fetch16(), // Load 2 bytes of data into SP
            0x32 => {
                // Store contents of A into the memory
                // location specified by HL then decrement HL
                let address = self.registers.get_hl();
                self.mmu.write8(address, self.registers.a);
                self.registers.dec_hl();
            },
            0x33 => self.inc_sp(), // Increment SP by 1
            0x34 => {
                // Decrement contents of memory specified by HL by 1
                let address = self.registers.get_hl();
                let value = self.dec(self.fetch_hla());
                self.mmu.write8(address, value);
            },
            0x35 => {
                // Increment contents of memory specified by HL by 1
                let address = self.registers.get_hl();
                let value = self.inc(self.fetch_hla());
                self.mmu.write8(address, value);
            },
            0x36 => {
                // Store d8 in the memory location specified by HL
                let address = self.registers.get_hl();
                let value = self.fetch8();
                self.mmu.write8(address, value);
            },
            0x37 => self.scf(), // SCF
            0x38 => result = self.jrc(self.registers.f.carry, self.fetch8()), // If C flag is 1, JR i8 
            0x39 => {
                // Add SP to Hl, and store the result in HL
                let value = self.add16(
                    self.registers.get_hl(),
                    self.sp
                );
                self.registers.set_hl(value);
            },
            0x3A => {
                // Load the contents of memory
                // specified by HL into A then increment HL
                let address = self.registers.get_hl();
                self.registers.a = self.mmu.read8(address);
                self.registers.dec_hl();
            },
            0x3B => self.dec_sp(),                                  // Decrement SP by 1
            0x3C => self.registers.a = self.inc(self.registers.a),  // Increment A by 1
            0x3D => self.registers.a = self.dec(self.registers.a),  // Decrement A by 1
            0x3E => self.registers.a = self.fetch8(),               // Load d8 into A
            0x3F => self.ccf(),                                     // CCF
            // Ox4X - Loads
            0x40 => { /* Basically does nothing */ },       // Load B into B
            0x41 => self.registers.b = self.registers.c,    // Load C into B
            0x42 => self.registers.b = self.registers.d,    // Load D into B
            0x43 => self.registers.b = self.registers.e,    // Load E into B
            0x44 => self.registers.b = self.registers.h,    // Load H into B
            0x45 => self.registers.b = self.registers.l,    // Load L into B
            0x46 => self.registers.b = self.fetch_hla(),    // Load (HL) into B
            0x47 => self.registers.b = self.registers.a,    // Load A into B
            0x48 => self.registers.c = self.registers.b,    // Load B into C
            0x49 => { /* Basically does nothing */ },       // Load C into C
            0x4A => self.registers.c = self.registers.d,    // Load D into C
            0x4B => self.registers.c = self.registers.e,    // Load E into C
            0x4C => self.registers.c = self.registers.h,    // Load H into C
            0x4D => self.registers.c = self.registers.l,    // Load L into C
            0x4E => self.registers.c = self.fetch_hla(),    // Load (HL) into C
            0x4F => self.registers.c = self.registers.a,    // Load A into C
            // Ox5X - Loads
            0x50 => self.registers.d = self.registers.b,    // Load B into D
            0x51 => self.registers.d = self.registers.c,    // Load C into D
            0x52 => { /* Basically does nothing */ },       // Load D into D
            0x53 => self.registers.d = self.registers.e,    // Load E into D
            0x54 => self.registers.d = self.registers.h,    // Load H into D
            0x55 => self.registers.d = self.registers.l,    // Load L into D
            0x56 => self.registers.d = self.fetch_hla(),    // Load (HL) into D
            0x57 => self.registers.d = self.registers.a,    // Load A into D
            0x58 => self.registers.e = self.registers.b,    // Load B into E
            0x59 => self.registers.e = self.registers.c,    // Load C into E
            0x5A => self.registers.e = self.registers.d,    // Load D into E
            0x5B => { /* Basically does nothing */ },       // Load E into E
            0x5C => self.registers.e = self.registers.h,    // Load H into E
            0x5D => self.registers.e = self.registers.l,    // Load L into E
            0x5E => self.registers.e = self.fetch_hla(),    // Load (HL) into E
            0x5F => self.registers.e = self.registers.a,    // Load A into E
            // Ox6X - Loads
            0x60 => self.registers.h = self.registers.b,    // Load B into H
            0x61 => self.registers.h = self.registers.c,    // Load C into H
            0x62 => self.registers.h = self.registers.d,    // Load D into H
            0x63 => self.registers.h = self.registers.e,    // Load E into H
            0x64 => { /* Basically does nothing */ },       // Load H into H
            0x65 => self.registers.h = self.registers.l,    // Load L into H
            0x66 => self.registers.h = self.fetch_hla(),    // Load (HL) into H
            0x67 => self.registers.h = self.registers.a,    // Load A into L
            0x68 => self.registers.l = self.registers.b,    // Load B into L
            0x69 => self.registers.l = self.registers.c,    // Load C into L
            0x6A => self.registers.l = self.registers.d,    // Load D into L
            0x6B => self.registers.l = self.registers.e,    // Load E into L 
            0x6C => self.registers.l = self.registers.h,    // Load H into L
            0x6D => { /* Basically does nothing */ },       // Load L into L
            0x6E => self.registers.l = self.fetch_hla(),    // Load (HL) into L
            0x6F => self.registers.l = self.registers.a,    // Load A into L
            // Ox7X - Loads
            0x70 => self.mmu.write8(self.registers.get_hl(), self.registers.b), // Store B in the memory location specified by HL
            0x71 => self.mmu.write8(self.registers.get_hl(), self.registers.c), // Store C in the memory location specified by HL
            0x72 => self.mmu.write8(self.registers.get_hl(), self.registers.d), // Store D in the memory location specified by HL
            0x73 => self.mmu.write8(self.registers.get_hl(), self.registers.e), // Store E in the memory location specified by HL
            0x74 => self.mmu.write8(self.registers.get_hl(), self.registers.h), // Store H in the memory location specified by HL
            0x75 => self.mmu.write8(self.registers.get_hl(), self.registers.l), // Store L in the memory location specified by HL
            0x76 => unimplemented!("0x76"), // TODO
            0x77 => self.mmu.write8(self.registers.get_hl(), self.registers.a), // Store A at the memory location specified by HL
            0x78 => self.registers.a = self.registers.b,    // Load B into A
            0x79 => self.registers.a = self.registers.c,    // Load C into A
            0x7A => self.registers.a = self.registers.d,    // Load D into A
            0x7B => self.registers.a = self.registers.e,    // Load E into A 
            0x7C => self.registers.a = self.registers.h,    // Load H into A
            0x7D => self.registers.a = self.registers.l,    // Load L into A
            0x7E => self.registers.a = self.fetch_hla(),    // Load (HL) into A
            0x7F => { /* Basically does nothing */ },       // Load A into A
            // Ox8X - Maths
            0x80 => self.registers.a = self.add(self.registers.b),  // ADD B
            0x81 => self.registers.a = self.add(self.registers.c),  // ADD C
            0x82 => self.registers.a = self.add(self.registers.d),  // ADD D
            0x83 => self.registers.a = self.add(self.registers.e),  // ADD E
            0x84 => self.registers.a = self.add(self.registers.h),  // ADD H
            0x85 => self.registers.a = self.add(self.registers.l),  // ADD L
            0x86 => self.registers.a = self.add(self.fetch_hla()),  // ADD (HL)
            0x87 => self.registers.a = self.add(self.registers.a),  // ADD A
            0x88 => self.registers.a = self.adc(self.registers.b),  // ADC B
            0x89 => self.registers.a = self.adc(self.registers.c),  // ADC C
            0x8A => self.registers.a = self.adc(self.registers.d),  // ADC D
            0x8B => self.registers.a = self.adc(self.registers.e),  // ADC E
            0x8C => self.registers.a = self.adc(self.registers.h),  // ADC H
            0x8D => self.registers.a = self.adc(self.registers.l),  // ADC L
            0x8E => self.registers.a = self.adc(self.fetch_hla()),  // ADC (HL)
            0x8F => self.registers.a = self.adc(self.registers.a),  // ADC A
            // Ox9X - Maths
            0x90 => self.registers.a = self.sub(self.registers.b),  // SUB B
            0x91 => self.registers.a = self.sub(self.registers.c),  // SUB C
            0x92 => self.registers.a = self.sub(self.registers.d),  // SUB D
            0x93 => self.registers.a = self.sub(self.registers.e),  // SUB E
            0x94 => self.registers.a = self.sub(self.registers.h),  // SUB H
            0x95 => self.registers.a = self.sub(self.registers.l),  // SUB L
            0x96 => self.registers.a = self.sub(self.fetch_hla()),  // SUB (HL)
            0x97 => self.registers.a = self.sub(self.registers.a),  // SUB A
            0x98 => self.registers.a = self.sbc(self.registers.b),  // SBC B
            0x99 => self.registers.a = self.sbc(self.registers.c),  // SBC C
            0x9A => self.registers.a = self.sbc(self.registers.d),  // SBC D
            0x9B => self.registers.a = self.sbc(self.registers.e),  // SBC E
            0x9C => self.registers.a = self.sbc(self.registers.h),  // SBC H
            0x9D => self.registers.a = self.sbc(self.registers.l),  // SBC L
            0x9E => self.registers.a = self.sbc(self.fetch_hla()),  // SBC (HL)
            0x9F => self.registers.a = self.sbc(self.registers.a),  // SBC A
            // OxAX - Maths
            0xA0 => self.registers.a = self.and(self.registers.b),  // AND B
            0xA1 => self.registers.a = self.and(self.registers.c),  // AND C
            0xA2 => self.registers.a = self.and(self.registers.d),  // AND D
            0xA3 => self.registers.a = self.and(self.registers.e),  // AND E
            0xA4 => self.registers.a = self.and(self.registers.h),  // AND H
            0xA5 => self.registers.a = self.and(self.registers.l),  // AND L
            0xA6 => self.registers.a = self.and(self.fetch_hla()),  // AND (HL)
            0xA7 => self.registers.a = self.and(self.registers.a),  // AND A
            0xA8 => self.registers.a = self.xor(self.registers.b),  // XOR B
            0xA9 => self.registers.a = self.xor(self.registers.c),  // XOR C
            0xAA => self.registers.a = self.xor(self.registers.d),  // XOR D
            0xAB => self.registers.a = self.xor(self.registers.e),  // XOR E
            0xAC => self.registers.a = self.xor(self.registers.h),  // XOR H
            0xAD => self.registers.a = self.xor(self.registers.l),  // XOR L
            0xAE => self.registers.a = self.xor(self.fetch_hla()),  // XOR (HL)
            0xAF => self.registers.a = self.xor(self.registers.a),  // XOR A
            // OxBX - Maths
            0xB0 => self.registers.a = self.or(self.registers.b ),  // OR B
            0xB1 => self.registers.a = self.or(self.registers.c ),  // OR C
            0xB2 => self.registers.a = self.or(self.registers.d ),  // OR D
            0xB3 => self.registers.a = self.or(self.registers.e ),  // OR E
            0xB4 => self.registers.a = self.or(self.registers.h ),  // OR H
            0xB5 => self.registers.a = self.or(self.registers.l ),  // OR L
            0xB6 => self.registers.a = self.or(self.fetch_hla() ),  // OR (HL)
            0xB7 => self.registers.a = self.or(self.registers.a ),  // OR A
            0xB8 => self.cp(self.registers.b),                      // CP B
            0xB9 => self.cp(self.registers.c),                      // CP C
            0xBA => self.cp(self.registers.d),                      // CP D
            0xBB => self.cp(self.registers.e),                      // CP E
            0xBC => self.cp(self.registers.h),                      // CP H
            0xBD => self.cp(self.registers.l),                      // CP L
            0xBE => self.cp(self.fetch_hla()),                      // CP (HL)
            0xBF => self.cp(self.registers.a),                      // CP A
            // OxCX
            0xC0 => result = self.ret_c(!self.registers.f.zero),
            0xC1 => { let value = self.pop(); self.registers.set_bc(value); }, // POP BC
            0xC2 => result = self.jpc(!self.registers.f.zero, self.fetch16()), // If Z flag is 0, jump to a16
            0xC3 => self.jp(self.fetch16()), // Jump to a16
            0xC4 => result = self.call_c(!self.registers.f.zero, self.fetch16()), // If Z flag is 0, call a16
            0xC5 => self.push(self.registers.get_bc()),             // PUSH BC
            0xC6 => self.registers.a = self.add(self.fetch8()),    // ADD d8
            0xC7 => self.rst(0x00),                         // Restart 0 -> 0x00
            0xC8 => result = self.ret_c(self.registers.f.zero),  // RET Z
            0xC9 => self.ret(),                                     // RET
            0xCA => result = self.jpc(self.registers.f.zero, self.fetch16()),
            0xCB => { // Subcommand prefix - execute that instead
                let address = self.pc;
                self.pc += (InstInfo::decode_sub(self.mmu.read8(self.pc)).bl-1) as u16; // Update the pc
                self.match_sub_cmd(self.mmu.read8(address));
            },
            0xCC => result = self.call_c(self.registers.f.zero, self.fetch16()), // If Z flag is 1, call a16
            0xCD => self.call(self.fetch16()), // Call a16
            0xCE => self.registers.a = self.adc(self.fetch8()),
            0xCF => self.rst(0x08),                         // Restart 1 -> 0x08
            0xD0 => result = self.ret_c(!self.registers.f.carry),
            0xD1 => { let value = self.pop(); self.registers.set_de(value); }, // POP DE
            0xD2 => result = self.jpc(!self.registers.f.carry, self.fetch16()), // If C flag is 0, jump to a16
            0xD3 => { /* NONEXISTENT */ },
            0xD4 => result = self.call_c(!self.registers.f.carry, self.fetch16()), // If C flag is 0, call a16
            0xD5 => self.push(self.registers.get_de()),             // PUSH DE
            0xD6 => self.registers.a = self.sub(self.fetch8()),
            0xD7 => self.rst(0x10),                         // Restart 2 -> 0x10
            0xD8 => result = self.ret_c(self.registers.f.carry),  // RET C
            0xD9 => unimplemented!("0xD9"),
            0xDA => result = self.jpc(self.registers.f.carry, self.fetch16()), // If C flag is 1, jump to a16
            0xDB => { /* NONEXISTENT */ },
            0xDC => result = self.call_c(self.registers.f.carry, self.fetch16()), // If C flag is 1, call a16
            0xDD => { /* NONEXISTENT */ },
            0xDE => self.registers.a = self.sbc(self.fetch8()), // SBC
            0xDF => self.rst(0x18), // Restart 3 -> 0x18
            0xE0 => {
                // Store the contents of A at the address in the
                // memory range 0xFF00-0xFFFF specified by a8
                let address = 0xFF00 + self.fetch8() as u16;
                self.mmu.write8(address, self.registers.a);
            },
            0xE1 => { let value = self.pop(); self.registers.set_hl(value); }, // POP HL
            0xE2 => {
                // Store the contents of A at the address in the
                // memory range 0xFF00-0xFFFF specified by register C
                let address = 0xFF00 + self.registers.c as u16;
                self.mmu.write8(address, self.registers.a);
            },
            0xE3 => { /* NONEXISTENT */ },
            0xE4 => { /* NONEXISTENT */ },
            0xE5 => self.push(self.registers.get_hl()),             // PUSH HL
            0xE6 => self.registers.a = self.and(self.fetch8()),    // AND d8
            0xE7 => self.rst(0x20),                     // Restart 4 -> 0x20
            0xE8 => self.pc = self.add_sp_e8(),
            0xE9 => self.jp(self.registers.get_hl()),               // Jump HL
            0xEA => {
                // Store the contents of A at the address a16
                let address = self.fetch16();
                self.mmu.write8(address, self.registers.a);
            },
            0xEB => { /* NONEXISTENT */ },
            0xEC => { /* NONEXISTENT */ },
            0xED => { /* NONEXISTENT */ },
            0xEE => self.registers.a = self.xor(self.fetch8()), // XOR d8
            0xEF => self.rst(0x28),                        // Restart 5 -> 0x28
            0xF0 => {
                let address = 0xFF00 + self.fetch8() as u16;
                self.registers.a = self.mmu.read8(address);
            },
            0xF1 => { let value = self.pop_af(); self.registers.set_af(value); }, // POP AF
            0xF2 => {
                let address = 0xFF00 + self.registers.c as u16;
                self.registers.a = self.mmu.read8(address);
            }
            0xF3 => { /*unimplemented!("0xF3")*/ },
            0xF4 => { /* NONEXISTENT */ },
            0xF5 => self.push(self.registers.get_af()),             // PUSH AF
            0xF6 => self.registers.a = self.or(self.fetch8()),     // OR d8
            0xF7 => self.rst(0x30),                        // Restart 6 -> 0x30
            0xF8 => { let value = self.jr(self.fetch8()); self.registers.set_hl(value); },
            0xF9 => self.sp = self.registers.get_hl(),
            0xFA => {
                let address = self.fetch16();
                self.registers.a = self.mmu.read8(address);
            },
            0xFB => unimplemented!("0xFB"),
            0xFC => { /* NONEXISTENT */ },
            0xFD => { /* NONEXISTENT */ },
            0xFE => self.cp(self.fetch8()), // CP d8
            0xFF => self.rst(0x38), // Restart 7 -> 0x38
        };

        result
    }
    pub fn match_sub_cmd(&mut self, cmd: u8) { 
        match cmd {
            // 0x0X
            0x00 => self.registers.b = self.rlc(self.registers.b),
            0x01 => self.registers.c = self.rlc(self.registers.c),
            0x02 => self.registers.d = self.rlc(self.registers.d),
            0x03 => self.registers.e = self.rlc(self.registers.e),
            0x04 => self.registers.h = self.rlc(self.registers.h),
            0x05 => self.registers.l = self.rlc(self.registers.l),
            0x06 => {
                let value = self.rlc(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x07 => self.registers.a = self.rlc(self.registers.a),
            0x08 => self.registers.b = self.rrc(self.registers.b),
            0x09 => self.registers.c = self.rrc(self.registers.c),
            0x0A => self.registers.d = self.rrc(self.registers.d),
            0x0B => self.registers.e = self.rrc(self.registers.e),
            0x0C => self.registers.h = self.rrc(self.registers.h),
            0x0D => self.registers.l = self.rrc(self.registers.l),
            0x0E => {
                let value = self.rrc(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x0F => self.registers.a = self.rrc(self.registers.a),
            // 0x1X
            0x10 => self.registers.b = self.rl(self.registers.b),
            0x11 => self.registers.c = self.rl(self.registers.c),
            0x12 => self.registers.d = self.rl(self.registers.d),
            0x13 => self.registers.e = self.rl(self.registers.e),
            0x14 => self.registers.h = self.rl(self.registers.h),
            0x15 => self.registers.l = self.rl(self.registers.l),
            0x16 => {
                let value = self.rl(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x17 => self.registers.a = self.rl(self.registers.a),
            0x18 => self.registers.b = self.rr(self.registers.b),
            0x19 => self.registers.c = self.rr(self.registers.c),
            0x1A => self.registers.d = self.rr(self.registers.d),
            0x1B => self.registers.e = self.rr(self.registers.e),
            0x1C => self.registers.h = self.rr(self.registers.h),
            0x1D => self.registers.l = self.rr(self.registers.l),
            0x1E => {
                let value = self.rr(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x1F => self.registers.a = self.rr(self.registers.a),
            // 0x2X
            0x20 => self.registers.b = self.sla(self.registers.b),
            0x21 => self.registers.c = self.sla(self.registers.c),
            0x22 => self.registers.d = self.sla(self.registers.d),
            0x23 => self.registers.e = self.sla(self.registers.e),
            0x24 => self.registers.h = self.sla(self.registers.h),
            0x25 => self.registers.l = self.sla(self.registers.l),
            0x26 => {
                let value = self.sla(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x27 => self.registers.a = self.sla(self.registers.a),
            0x28 => self.registers.b = self.sra(self.registers.b),
            0x29 => self.registers.c = self.sra(self.registers.c),
            0x2A => self.registers.d = self.sra(self.registers.d),
            0x2B => self.registers.e = self.sra(self.registers.e),
            0x2C => self.registers.h = self.sra(self.registers.h),
            0x2D => self.registers.l = self.sra(self.registers.l),
            0x2E => {
                let value = self.sra(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x2F => self.registers.a = self.sra(self.registers.a),
            // 0x3X
            0x30 => self.registers.b = self.swap(self.registers.b),
            0x31 => self.registers.c = self.swap(self.registers.c),
            0x32 => self.registers.d = self.swap(self.registers.d),
            0x33 => self.registers.e = self.swap(self.registers.e),
            0x34 => self.registers.h = self.swap(self.registers.h),
            0x35 => self.registers.l = self.swap(self.registers.l),
            0x36 => {
                let value = self.swap(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x37 => self.registers.a = self.swap(self.registers.a),
            0x38 => self.registers.b = self.srl(self.registers.b),
            0x39 => self.registers.c = self.srl(self.registers.c),
            0x3A => self.registers.d = self.srl(self.registers.d),
            0x3B => self.registers.e = self.srl(self.registers.e),
            0x3C => self.registers.h = self.srl(self.registers.h),
            0x3D => self.registers.l = self.srl(self.registers.l),
            0x3E => {
                let value = self.srl(self.fetch_hla());
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x3F => self.registers.a = self.srl(self.registers.a),
            // 0x4X
            0x40 => self.bit(self.registers.b, 0),
            0x41 => self.bit(self.registers.c, 0),
            0x42 => self.bit(self.registers.d, 0),
            0x43 => self.bit(self.registers.e, 0),
            0x44 => self.bit(self.registers.h, 0),
            0x45 => self.bit(self.registers.l, 0),
            0x46 => self.bit(self.fetch_hla(), 0),
            0x47 => self.bit(self.registers.a, 0),
            0x48 => self.bit(self.registers.b, 1),
            0x49 => self.bit(self.registers.c, 1),
            0x4A => self.bit(self.registers.d, 1),
            0x4B => self.bit(self.registers.e, 1),
            0x4C => self.bit(self.registers.h, 1),
            0x4D => self.bit(self.registers.l, 1),
            0x4E => self.bit(self.fetch_hla(), 1),
            0x4F => self.bit(self.registers.a, 1),
            // 0x5X
            0x50 => self.bit(self.registers.b, 2),
            0x51 => self.bit(self.registers.c, 2),
            0x52 => self.bit(self.registers.d, 2),
            0x53 => self.bit(self.registers.e, 2),
            0x54 => self.bit(self.registers.h, 2),
            0x55 => self.bit(self.registers.l, 2),
            0x56 => self.bit(self.fetch_hla(), 2),
            0x57 => self.bit(self.registers.a, 2),
            0x58 => self.bit(self.registers.b, 3),
            0x59 => self.bit(self.registers.c, 3),
            0x5A => self.bit(self.registers.d, 3),
            0x5B => self.bit(self.registers.e, 3),
            0x5C => self.bit(self.registers.h, 3),
            0x5D => self.bit(self.registers.l, 3),
            0x5E => self.bit(self.fetch_hla(), 3),
            0x5F => self.bit(self.registers.a, 3),
            // 0x6X
            0x60 => self.bit(self.registers.b, 4),
            0x61 => self.bit(self.registers.c, 4),
            0x62 => self.bit(self.registers.d, 4),
            0x63 => self.bit(self.registers.e, 4),
            0x64 => self.bit(self.registers.h, 4),
            0x65 => self.bit(self.registers.l, 4),
            0x66 => self.bit(self.fetch_hla(), 4),
            0x67 => self.bit(self.registers.a, 4),
            0x68 => self.bit(self.registers.b, 5),
            0x69 => self.bit(self.registers.c, 5),
            0x6A => self.bit(self.registers.d, 5),
            0x6B => self.bit(self.registers.e, 5),
            0x6C => self.bit(self.registers.h, 5),
            0x6D => self.bit(self.registers.l, 5),
            0x6E => self.bit(self.fetch_hla(), 5),
            0x6F => self.bit(self.registers.a, 5),
            // 0x7X
            0x70 => self.bit(self.registers.b, 6),
            0x71 => self.bit(self.registers.c, 6),
            0x72 => self.bit(self.registers.d, 6),
            0x73 => self.bit(self.registers.e, 6),
            0x74 => self.bit(self.registers.h, 6),
            0x75 => self.bit(self.registers.l, 6),
            0x76 => self.bit(self.fetch_hla(), 6),
            0x77 => self.bit(self.registers.a,6),
            0x78 => self.bit(self.registers.b, 7),
            0x79 => self.bit(self.registers.c, 7),
            0x7A => self.bit(self.registers.d, 7),
            0x7B => self.bit(self.registers.e, 7),
            0x7C => self.bit(self.registers.h, 7),
            0x7D => self.bit(self.registers.l, 7),
            0x7E => self.bit(self.fetch_hla(), 7),
            0x7F => self.bit(self.registers.a, 7),
            // 0x8X
            0x80 => self.registers.b = self.res(self.registers.b, 0),
            0x81 => self.registers.c = self.res(self.registers.c, 0),
            0x82 => self.registers.d = self.res(self.registers.d, 0),
            0x83 => self.registers.e = self.res(self.registers.e, 0),
            0x84 => self.registers.h = self.res(self.registers.h, 0),
            0x85 => self.registers.l = self.res(self.registers.l, 0),
            0x86 => {
                let value = self.res(self.fetch_hla(), 0);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x87 => self.registers.a = self.res(self.registers.a, 0),
            0x88 => self.registers.b = self.res(self.registers.b, 1),
            0x89 => self.registers.c = self.res(self.registers.c, 1),
            0x8A => self.registers.d = self.res(self.registers.d, 1),
            0x8B => self.registers.e = self.res(self.registers.e, 1),
            0x8C => self.registers.h = self.res(self.registers.h, 1),
            0x8D => self.registers.l = self.res(self.registers.l, 1),
            0x8E => {
                let value = self.res(self.fetch_hla(), 1);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x8F => self.registers.a = self.res(self.registers.a, 1),
            // 0x9X
            0x90 => self.registers.b = self.res(self.registers.b, 2),
            0x91 => self.registers.c = self.res(self.registers.c, 2),
            0x92 => self.registers.d = self.res(self.registers.d, 2),
            0x93 => self.registers.e = self.res(self.registers.e, 2),
            0x94 => self.registers.h = self.res(self.registers.h, 2),
            0x95 => self.registers.l = self.res(self.registers.l, 2),
            0x96 => {
                let value = self.res(self.fetch_hla(), 2);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x97 => self.registers.a = self.res(self.registers.a, 2),
            0x98 => self.registers.b = self.res(self.registers.b, 3),
            0x99 => self.registers.c = self.res(self.registers.c, 3),
            0x9A => self.registers.d = self.res(self.registers.d, 3),
            0x9B => self.registers.e = self.res(self.registers.e, 3),
            0x9C => self.registers.h = self.res(self.registers.h, 3),
            0x9D => self.registers.l = self.res(self.registers.l, 3),
            0x9E => {
                let value = self.res(self.fetch_hla(), 3);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0x9F => self.registers.a = self.res(self.registers.a, 3),
            // 0xAX
            0xA0 => self.registers.b = self.res(self.registers.b, 4),
            0xA1 => self.registers.c = self.res(self.registers.c, 4),
            0xA2 => self.registers.d = self.res(self.registers.d, 4),
            0xA3 => self.registers.e = self.res(self.registers.e, 4),
            0xA4 => self.registers.h = self.res(self.registers.h, 4),
            0xA5 => self.registers.l = self.res(self.registers.l, 4),
            0xA6 => {
                let value = self.res(self.fetch_hla(), 4);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xA7 => self.registers.a = self.res(self.registers.a, 4),
            0xA8 => self.registers.b = self.res(self.registers.b, 5),
            0xA9 => self.registers.c = self.res(self.registers.c, 5),
            0xAA => self.registers.d = self.res(self.registers.d, 5),
            0xAB => self.registers.e = self.res(self.registers.e, 5),
            0xAC => self.registers.h = self.res(self.registers.h, 5),
            0xAD => self.registers.l = self.res(self.registers.l, 5),
            0xAE => {
                let value = self.res(self.fetch_hla(), 5);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xAF => self.registers.a = self.res(self.registers.a, 5),
            // 0xBX
            0xB0 => self.registers.b = self.res(self.registers.b, 6),
            0xB1 => self.registers.c = self.res(self.registers.c, 6),
            0xB2 => self.registers.d = self.res(self.registers.d, 6),
            0xB3 => self.registers.e = self.res(self.registers.e, 6),
            0xB4 => self.registers.h = self.res(self.registers.h, 6),
            0xB5 => self.registers.l = self.res(self.registers.l, 6),
            0xB6 => {
                let value = self.res(self.fetch_hla(), 6);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xB7 => self.registers.a = self.res(self.registers.a, 6),
            0xB8 => self.registers.b = self.res(self.registers.b, 7),
            0xB9 => self.registers.c = self.res(self.registers.c, 7),
            0xBA => self.registers.d = self.res(self.registers.d, 7),
            0xBB => self.registers.e = self.res(self.registers.e, 7),
            0xBC => self.registers.h = self.res(self.registers.h, 7),
            0xBD => self.registers.l = self.res(self.registers.l, 7),
            0xBE => {
                let value = self.res(self.fetch_hla(), 7);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xBF => self.registers.a = self.res(self.registers.a, 7),
            // 0xCX
            0xC0 => self.registers.b = self.set(self.registers.b, 0),
            0xC1 => self.registers.c = self.set(self.registers.c, 0),
            0xC2 => self.registers.d = self.set(self.registers.d, 0),
            0xC3 => self.registers.e = self.set(self.registers.e, 0),
            0xC4 => self.registers.h = self.set(self.registers.h, 0),
            0xC5 => self.registers.l = self.set(self.registers.l, 0),
            0xC6 => {
                let value = self.set(self.fetch_hla(), 0);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xC7 => self.registers.a = self.set(self.registers.a, 0),
            0xC8 => self.registers.b = self.set(self.registers.b, 1),
            0xC9 => self.registers.c = self.set(self.registers.c, 1),
            0xCA => self.registers.d = self.set(self.registers.d, 1),
            0xCB => self.registers.e = self.set(self.registers.e, 1),
            0xCC => self.registers.h = self.set(self.registers.h, 1),
            0xCD => self.registers.l = self.set(self.registers.l, 1),
            0xCE => {
                let value = self.set(self.fetch_hla(), 1);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xCF => self.registers.a = self.set(self.registers.a, 1),
            // 0xDX
            0xD0 => self.registers.b = self.set(self.registers.b, 2),
            0xD1 => self.registers.c = self.set(self.registers.c, 2),
            0xD2 => self.registers.d = self.set(self.registers.d, 2),
            0xD3 => self.registers.e = self.set(self.registers.e, 2),
            0xD4 => self.registers.h = self.set(self.registers.h, 2),
            0xD5 => self.registers.l = self.set(self.registers.l, 2),
            0xD6 => {
                let value = self.set(self.fetch_hla(), 2);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xD7 => self.registers.a = self.set(self.registers.a, 2),
            0xD8 => self.registers.b = self.set(self.registers.b, 3),
            0xD9 => self.registers.c = self.set(self.registers.c, 3),
            0xDA => self.registers.d = self.set(self.registers.d, 3),
            0xDB => self.registers.e = self.set(self.registers.e, 3),
            0xDC => self.registers.h = self.set(self.registers.h, 3),
            0xDD => self.registers.l = self.set(self.registers.l, 3),
            0xDE => {
                let value = self.set(self.fetch_hla(), 3);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xDF => self.registers.a = self.set(self.registers.a, 3),
            // 0xEX
            0xE0 => self.registers.b = self.set(self.registers.b, 4),
            0xE1 => self.registers.c = self.set(self.registers.c, 4),
            0xE2 => self.registers.d = self.set(self.registers.d, 4),
            0xE3 => self.registers.e = self.set(self.registers.e, 4),
            0xE4 => self.registers.h = self.set(self.registers.h, 4),
            0xE5 => self.registers.l = self.set(self.registers.l, 4),
            0xE6 => {
                let value = self.set(self.fetch_hla(), 4);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xE7 => self.registers.a = self.set(self.registers.a, 4),
            0xE8 => self.registers.b = self.set(self.registers.b, 5),
            0xE9 => self.registers.c = self.set(self.registers.c, 5),
            0xEA => self.registers.d = self.set(self.registers.d, 5),
            0xEB => self.registers.e = self.set(self.registers.e, 5),
            0xEC => self.registers.h = self.set(self.registers.h, 5),
            0xED => self.registers.l = self.set(self.registers.l, 5),
            0xEE => {
                let value = self.set(self.fetch_hla(), 5);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xEF => self.registers.a = self.set(self.registers.a, 5),
            // 0xFX
            0xF0 => self.registers.b = self.set(self.registers.b, 6),
            0xF1 => self.registers.c = self.set(self.registers.c, 6),
            0xF2 => self.registers.d = self.set(self.registers.d, 6),
            0xF3 => self.registers.e = self.set(self.registers.e, 6),
            0xF4 => self.registers.h = self.set(self.registers.h, 6),
            0xF5 => self.registers.l = self.set(self.registers.l, 6),
            0xF6 => {
                let value = self.set(self.fetch_hla(), 6);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xF7 => self.registers.a = self.set(self.registers.a, 6),
            0xF8 => self.registers.b = self.set(self.registers.b, 7),
            0xF9 => self.registers.c = self.set(self.registers.c, 7),
            0xFA => self.registers.d = self.set(self.registers.d, 7),
            0xFB => self.registers.e = self.set(self.registers.e, 7),
            0xFC => self.registers.h = self.set(self.registers.h, 7),
            0xFD => self.registers.l = self.set(self.registers.l, 7),
            0xFE => {
                let value = self.set(self.fetch_hla(), 7);
                let address = self.registers.get_hl();
                self.mmu.write8(address, value);
            }
            0xFF => self.registers.a = self.set(self.registers.a, 7),
        }
    }
}