

///
/// This file contains more or less what I'd call instruction metadata.
/// 
///     All we need to know for debugging and executing any opcode is located here.
///     It contains every instruction's address, name, byte length and cycle length.
///     I took the data straight from https://meganesu.github.io/generate-gb-opcodes/.
///     
///     It should come in handy for decompilation.
///


// Cycle length might depend on if opcode conditions were met
#[derive(Clone, Copy)]
pub struct CycleLength {
    pub full: u8,   // If conditions were met
    pub light: u8,  // If conditions weren't met
}
impl CycleLength {
    const fn new(full: u8, light: u8) -> Self {
        Self { full, light }
    }
}

#[derive(Clone, Copy)]
pub struct InstInfo<'a> {
    pub d: &'a str,         // Disassembly
    pub bl: u8,             // Byte length
    pub cl: CycleLength,    // Cycle length
}
impl<'a> InstInfo<'a> {
    const fn new_s(d: &'a str, bl: u8, cl1: u8) -> Self {           // New single
        Self { d, bl, cl: CycleLength::new(cl1, cl1) }
    }
    const fn new_d(d: &'a str, bl: u8, cl1: u8, cl2: u8) -> Self {  // New double
        Self { d, bl, cl: CycleLength::new(cl1, cl2) }
    }

    pub fn disassemble<T: AsRef<[u8]>>(binary: &T) -> String {
        let mut result: String = String::new();
        let bin = binary.as_ref();

        let mut i: usize = 0;
        while i < bin.len() {
            let decode = Self::decode(bin[i]);
            let bl = (decode.bl - 1) as usize;
            if bl == 0 {
                result += format!("\"{}\" ", decode.d).as_str();
            } else {
                let bytes = bin[i+1..=i+bl].to_vec();
                let string = bytes.iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<String>>().join(" ");
                result += format!("\"{}\" {} ", decode.d, string).as_str();
            }
            i += decode.bl as usize;
        }

        result
    }

    pub fn decode(cmd: u8) -> InstInfo<'static> { MAIN_INST_INFO[cmd as usize] }
    pub fn decode_sub(cmd: u8) -> InstInfo<'static> { SUB_INST_INFO[cmd as usize] }
}
impl<'a> std::fmt::Display for InstInfo<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.cl.full != self.cl.light {
            write!(
                f,
                "bl: {} cl: {}/{} name: \"{}\"",
                self.bl, self.cl.full, self.cl.light, self.d
            )
        } else {
            write!(
                f,
                "bl: {} cl: {:<3} name: \"{}\"",
                self.bl, self.cl.full, self.d
            )
        }
    }
}

// Game Boy CPU instructions
// https://meganesu.github.io/generate-gb-opcodes/
pub const MAIN_INST_INFO: [InstInfo; 256] = [
	InstInfo::new_s("NOP", 1, 1),           // 0x00
	InstInfo::new_s("LD BC, d16", 3, 3),    // 0x01
    InstInfo::new_s("LD (BC), A", 1, 2),    // 0x02
    InstInfo::new_s("INC BC", 1, 2),        // 0x03
    InstInfo::new_s("INC B", 1, 1),         // 0x04
    InstInfo::new_s("DEC B", 1, 1),         // 0x05
    InstInfo::new_s("LD B, d8", 2, 2),      // 0x06
    InstInfo::new_s("RLCA", 1, 1),          // 0x07
    InstInfo::new_s("LD (a16), SP", 3, 5),  // 0x08
    InstInfo::new_s("ADD HL, BC", 1, 2),    // 0x09
    InstInfo::new_s("LD A, (BC)", 1, 2),    // 0x0A
    InstInfo::new_s("DEC BC", 1, 2),        // 0x0B
    InstInfo::new_s("INC C", 1, 1),         // 0x0C
    InstInfo::new_s("DEC C", 1, 1),         // 0x0D
    InstInfo::new_s("LD C, d8", 2, 2),      // 0x0E
    InstInfo::new_s("RRCA", 1, 1),          // 0x0F

    InstInfo::new_s("STOP", 2, 1),          // 0x10
    InstInfo::new_s("LD DE, d16", 3, 3),    // 0x11
    InstInfo::new_s("LD (DE), A", 1, 2),    // 0x12
    InstInfo::new_s("INC DE", 1, 2),        // 0x13
    InstInfo::new_s("INC D", 1, 1),         // 0x14
    InstInfo::new_s("DEC D", 1, 1),         // 0x15
    InstInfo::new_s("LD D, d8", 2, 2),      // 0x16
    InstInfo::new_s("RLA", 1, 1),           // 0x17
    InstInfo::new_s("JR s8", 2, 3),         // 0x18 
    InstInfo::new_s("ADD HL, DE", 1, 2),    // 0x19
    InstInfo::new_s("LD A, (DE)", 1, 2),    // 0x1A
    InstInfo::new_s("DEC DE", 1, 2),        // 0x1B
    InstInfo::new_s("INC E", 1, 1),         // 0x1C
    InstInfo::new_s("DEC E", 1, 1),         // 0x1D
    InstInfo::new_s("LD E, d8", 2, 2),      // 0x1E
    InstInfo::new_s("RRA", 1, 1),           // 0x1F

    InstInfo::new_d("JR NZ, s8", 2, 3, 2),      // 0x20
    InstInfo::new_s("LD HL, d16", 3, 3),    // 0x21
    InstInfo::new_s("LD (HL+), A", 1, 2),   // 0x22
    InstInfo::new_s("INC HL", 1, 2),        // 0x23
    InstInfo::new_s("INC H", 1, 1),         // 0x24
    InstInfo::new_s("DEC H", 1, 1),         // 0x25
    InstInfo::new_s("LD H, d8", 2, 2),      // 0x26
    InstInfo::new_s("DAA", 1, 1),           // 0x27
    InstInfo::new_d("JR Z, s8", 2, 3, 2),       // 0x28
    InstInfo::new_s("ADD HL, HL", 1, 2),    // 0x29
    InstInfo::new_s("LD A, (HL+)", 1, 2),   // 0x2A
    InstInfo::new_s("DEC HL", 1, 2),        // 0x2B
    InstInfo::new_s("INC L", 1, 1),         // 0x2C
    InstInfo::new_s("DEC L", 1, 1),         // 0x2D
    InstInfo::new_s("LD L, d8", 2, 2),      // 0x2E
    InstInfo::new_s("CPL", 1, 1),           // 0x2F

    InstInfo::new_d("JR NC, s8", 2, 3, 2),      // 0x30
    InstInfo::new_s("LD SP, d16", 3, 3),    // 0x31
    InstInfo::new_s("LD (HL-), A", 1, 2),   // 0x32
    InstInfo::new_s("INC SP", 1, 2),        // 0x33
    InstInfo::new_s("INC (HL)", 1, 3),      // 0x34
    InstInfo::new_s("DEC (HL)", 1, 3),      // 0x35
    InstInfo::new_s("LD (HL), d8", 2, 3),   // 0x36
    InstInfo::new_s("SCF", 1, 1),           // 0x37
    InstInfo::new_d("JR C, s8", 2, 3, 2),       // 0x38
    InstInfo::new_s("ADD HL, SP", 1, 2),    // 0x39
    InstInfo::new_s("LD A, (HL-)", 1, 2),   // 0x3A
    InstInfo::new_s("DEC SP", 1, 2),        // 0x3B
    InstInfo::new_s("INC A", 1, 1),         // 0x3C
    InstInfo::new_s("DEC A", 1, 1),         // 0x3D
    InstInfo::new_s("LD A, d8", 2, 2),      // 0x3E
    InstInfo::new_s("CCF", 1, 1),           // 0x3F

    InstInfo::new_s("LD B, B", 1, 1),       // 0x40
    InstInfo::new_s("LD B, C", 1, 1),       // 0x41
    InstInfo::new_s("LD B, D", 1, 1),       // 0x42
    InstInfo::new_s("LD B, E", 1, 1),       // 0x43
    InstInfo::new_s("LD B, H", 1, 1),       // 0x44
    InstInfo::new_s("LD B, L", 1, 1),       // 0x45
    InstInfo::new_s("LD B, (HL)", 1, 2),    // 0x46
    InstInfo::new_s("LD B, A", 1, 1),       // 0x47
    InstInfo::new_s("LD C, B", 1, 1),       // 0x48
    InstInfo::new_s("LD C, C", 1, 1),       // 0x49
    InstInfo::new_s("LD C, D", 1, 1),       // 0x4A
    InstInfo::new_s("LD C, E", 1, 1),       // 0x4B
    InstInfo::new_s("LD C, H", 1, 1),       // 0x4C
    InstInfo::new_s("LD C, L", 1, 1),       // 0x4D
    InstInfo::new_s("LD C, (HL)", 1, 2),    // 0x4E
    InstInfo::new_s("LD C, A", 1, 1),       // 0x4F

    InstInfo::new_s("LD D, B", 1, 1),       // 0x50
    InstInfo::new_s("LD D, C", 1, 1),       // 0x51
    InstInfo::new_s("LD D, D", 1, 1),       // 0x52
    InstInfo::new_s("LD D, E", 1, 1),       // 0x53
    InstInfo::new_s("LD D, H", 1, 1),       // 0x54
    InstInfo::new_s("LD D, L", 1, 1),       // 0x55
    InstInfo::new_s("LD D, (HL)", 1, 2),    // 0x56
    InstInfo::new_s("LD D, A", 1, 1),       // 0x57
    InstInfo::new_s("LD E, B", 1, 1),       // 0x58
    InstInfo::new_s("LD E, C", 1, 1),       // 0x59
    InstInfo::new_s("LD E, D", 1, 1),       // 0x5A
    InstInfo::new_s("LD E, E", 1, 1),       // 0x5B
    InstInfo::new_s("LD E, H", 1, 1),       // 0x5C
    InstInfo::new_s("LD E, L", 1, 1),       // 0x5D
    InstInfo::new_s("LD E, (HL)", 1, 2),    // 0x5E
    InstInfo::new_s("LD E, A", 1, 1),       // 0x5F

    InstInfo::new_s("LD H, B", 1, 1),       // 0x60
    InstInfo::new_s("LD H, C", 1, 1),       // 0x61
    InstInfo::new_s("LD H, D", 1, 1),       // 0x62
    InstInfo::new_s("LD H, E", 1, 1),       // 0x63
    InstInfo::new_s("LD H, H", 1, 1),       // 0x64
    InstInfo::new_s("LD H, L", 1, 1),       // 0x65
    InstInfo::new_s("LD H, (HL)", 1, 2),    // 0x66
    InstInfo::new_s("LD H, A", 1, 1),       // 0x67
    InstInfo::new_s("LD L, B", 1, 1),       // 0x68
    InstInfo::new_s("LD L, C", 1, 1),       // 0x69
    InstInfo::new_s("LD L, D", 1, 1),       // 0x6A
    InstInfo::new_s("LD L, E", 1, 1),       // 0x6B
    InstInfo::new_s("LD L, H", 1, 1),       // 0x6C
    InstInfo::new_s("LD L, L", 1, 1),       // 0x6D
    InstInfo::new_s("LD L, (HL)", 1, 2),    // 0x6E
    InstInfo::new_s("LD L, A", 1, 1),       // 0x6F

    InstInfo::new_s("LD (HL), B", 1, 2),    // 0x70
    InstInfo::new_s("LD (HL), C", 1, 2),    // 0x71
    InstInfo::new_s("LD (HL), D", 1, 2),    // 0x72
    InstInfo::new_s("LD (HL), E", 1, 2),    // 0x73
    InstInfo::new_s("LD (HL), H", 1, 2),    // 0x74
    InstInfo::new_s("LD (HL), L", 1, 1),    // 0x75
    InstInfo::new_s("HALT", 1, 1),          // 0x76
    InstInfo::new_s("LD (HL), A", 1, 2),    // 0x77
    InstInfo::new_s("LD A, B", 1, 1),       // 0x78
    InstInfo::new_s("LD A, C", 1, 1),       // 0x79
    InstInfo::new_s("LD A, D", 1, 1),       // 0x7A
    InstInfo::new_s("LD A, E", 1, 1),       // 0x7B
    InstInfo::new_s("LD A, H", 1, 1),       // 0x7C
    InstInfo::new_s("LD A, L", 1, 1),       // 0x7D
    InstInfo::new_s("LD A, (HL)", 1, 2),    // 0x7E
    InstInfo::new_s("LD A, A", 1, 1),       // 0x7F

    InstInfo::new_s("ADD A, B", 1, 1),      // 0x80
    InstInfo::new_s("ADD A, C", 1, 1),      // 0x81
    InstInfo::new_s("ADD A, D", 1, 1),      // 0x82
    InstInfo::new_s("ADD A, E", 1, 1),      // 0x83
    InstInfo::new_s("ADD A, H", 1, 1),      // 0x84
    InstInfo::new_s("ADD A, L", 1, 1),      // 0x85
    InstInfo::new_s("ADD A, (HL)", 1, 2),   // 0x86
    InstInfo::new_s("ADD A, A", 1, 1),      // 0x87
    InstInfo::new_s("ADC A, B", 1, 1),      // 0x88
    InstInfo::new_s("ADC A, C", 1, 1),      // 0x89
    InstInfo::new_s("ADC A, D", 1, 1),      // 0x8A
    InstInfo::new_s("ADC A, E", 1, 1),      // 0x8B
    InstInfo::new_s("ADC A, H", 1, 1),      // 0x8C
    InstInfo::new_s("ADC A, L", 1, 1),      // 0x8D
    InstInfo::new_s("ADC A, (HL)", 1, 2),   // 0x8E
    InstInfo::new_s("ADC A, A", 1, 1),      // 0x8F

    InstInfo::new_s("SUB B", 1, 1),         // 0x90
    InstInfo::new_s("SUB C", 1, 1),         // 0x91
    InstInfo::new_s("SUB D", 1, 1),         // 0x92
    InstInfo::new_s("SUB E", 1, 1),         // 0x93
    InstInfo::new_s("SUB H", 1, 1),         // 0x94
    InstInfo::new_s("SUB L", 1, 1),         // 0x95
    InstInfo::new_s("SUB (HL)", 1, 2),      // 0x96
    InstInfo::new_s("SUB A", 1, 1),         // 0x97
    InstInfo::new_s("SBC A, B", 1, 1),      // 0x98
    InstInfo::new_s("SBC A, C", 1, 1),      // 0x99
    InstInfo::new_s("SBC A, D", 1, 1),      // 0x9A
    InstInfo::new_s("SBC A, E", 1, 1),      // 0x9B
    InstInfo::new_s("SBC A, H", 1, 1),      // 0x9C
    InstInfo::new_s("SBC A, L", 1, 1),      // 0x9D
    InstInfo::new_s("SBC A, (HL)", 1, 2),   // 0x9E
    InstInfo::new_s("SBC A, A", 1, 1),      // 0x9F

    InstInfo::new_s("AND B", 1, 1),         // 0xA0
    InstInfo::new_s("AND C", 1, 1),         // 0xA1
    InstInfo::new_s("AND D", 1, 1),         // 0xA2
    InstInfo::new_s("AND E", 1, 1),         // 0xA3
    InstInfo::new_s("AND H", 1, 1),         // 0xA4
    InstInfo::new_s("AND L", 1, 1),         // 0xA5
    InstInfo::new_s("AND (HL)", 1, 2),      // 0xA6
    InstInfo::new_s("AND A", 1, 1),         // 0xA7
    InstInfo::new_s("XOR B", 1, 1),         // 0xA8
    InstInfo::new_s("XOR C", 1, 1),         // 0xA9
    InstInfo::new_s("XOR D", 1, 1),         // 0xAA
    InstInfo::new_s("XOR E", 1, 1),         // 0xAB
    InstInfo::new_s("XOR H", 1, 1),         // 0xAC
    InstInfo::new_s("XOR L", 1, 1),         // 0xAD
    InstInfo::new_s("XOR (HL)", 1, 2),      // 0xAE
    InstInfo::new_s("XOR A", 1, 1),         // 0xAF

    InstInfo::new_s("OR B", 1, 1),          // 0xB0
    InstInfo::new_s("OR C", 1, 1),          // 0xB1
    InstInfo::new_s("OR D", 1, 1),          // 0xB2
    InstInfo::new_s("OR E", 1, 1),          // 0xB3
    InstInfo::new_s("OR H", 1, 1),          // 0xB4
    InstInfo::new_s("OR L", 1, 1),          // 0xB5
    InstInfo::new_s("OR (HL)", 1, 2),       // 0xB6
    InstInfo::new_s("OR A", 1, 1),          // 0xB7
    InstInfo::new_s("CP B", 1, 1),          // 0xB8
    InstInfo::new_s("CP C", 1, 1),          // 0xB9
    InstInfo::new_s("CP D", 1, 1),          // 0xBA
    InstInfo::new_s("CP E", 1, 1),          // 0xBB
    InstInfo::new_s("CP H", 1, 1),          // 0xBC
    InstInfo::new_s("CP L", 1, 1),          // 0xBD
    InstInfo::new_s("CP (HL)", 1, 2),       // 0xBE
    InstInfo::new_s("CP A", 1, 1),          // 0xBF

    InstInfo::new_d("RET NZ", 1, 5, 2),         // 0xC0
    InstInfo::new_s("POP BC", 1, 3),        // 0xC1
    InstInfo::new_d("JP NZ, a16", 3, 4, 3),     // 0xC2
    InstInfo::new_s("JP a16", 3, 4),        // 0xC3
    InstInfo::new_d("CALL NZ, a16", 3, 6, 3),   // 0xC4
    InstInfo::new_s("PUSH BC", 1, 4),       // 0xC5
    InstInfo::new_s("ADD A, d8", 2, 2),     // 0xC6
    InstInfo::new_s("RST 0", 1, 4),         // 0xC7
    InstInfo::new_d("RET Z", 1, 5, 2),  // 0xC8
    InstInfo::new_s("RET", 1, 4),           // 0xC9
    InstInfo::new_d("JP Z, a16", 3, 4, 3),      // 0xCA
    InstInfo::new_s("NONE", 1, 0),          // 0xCB
    InstInfo::new_d("CALL Z, a16", 3, 6, 3),    // 0xCC
    InstInfo::new_s("CALL a16", 3, 6),      // 0xCD
    InstInfo::new_s("ADC A, d8", 2, 2),     // 0xCE
    InstInfo::new_s("RST 1", 1, 4),         // 0xCF

    InstInfo::new_d("RET NC", 1, 5, 2),         // 0xD0
    InstInfo::new_s("POP DE", 1, 3),        // 0xD1
    InstInfo::new_d("JP NC, a16", 3, 4, 3),     // 0xD2
    InstInfo::new_s("NONE", 1, 0),          // 0xD3
    InstInfo::new_d("CALL NC, a16", 3, 6, 3),   // 0xD4
    InstInfo::new_s("PUSH DE", 1, 4),       // 0xD5
    InstInfo::new_s("SUB d8", 2, 2),        // 0xD6
    InstInfo::new_s("RST 2", 1, 4),         // 0xD7
    InstInfo::new_d("RET C", 1, 5, 2),          // 0xD8
    InstInfo::new_s("RETI", 1, 4),          // 0xD9
    InstInfo::new_d("JP C, a16", 3, 4, 3),      // 0xDA
    InstInfo::new_s("NONE", 1, 0),          // 0xDB
    InstInfo::new_d("CALL C, a16", 3, 6, 3),    // 0xDC
    InstInfo::new_s("NONE", 1, 0),          // 0xDD
    InstInfo::new_s("SBC A, d8", 2, 2),     // 0xDE
    InstInfo::new_s("RST 3", 1, 4),         // 0xDF

    InstInfo::new_s("LD (a8), A", 2, 3),    // 0xE0
    InstInfo::new_s("POP HL", 1, 3),        // 0xE1
    InstInfo::new_s("LD (C), A", 1, 2),     // 0xE2
    InstInfo::new_s("NONE", 1, 0),          // 0xE3
    InstInfo::new_s("NONE", 1, 0),          // 0xE4
    InstInfo::new_s("PUSH HL", 1, 4),       // 0xE5
    InstInfo::new_s("AND d8", 2, 2),        // 0xE6
    InstInfo::new_s("RST 4", 1, 4),         // 0xE7
    InstInfo::new_s("ADD SP, s8", 2, 4),    // 0xE8
    InstInfo::new_s("JP HL", 1, 1),         // 0xE9
    InstInfo::new_s("LD (a16), A", 3, 4),   // 0xEA
    InstInfo::new_s("NONE", 1, 0),          // 0xEB
    InstInfo::new_s("NONE", 1, 0),          // 0xEC
    InstInfo::new_s("NONE", 1, 0),          // 0xED
    InstInfo::new_s("XOR d8", 2, 2),        // 0xEE
    InstInfo::new_s("RST 5", 1, 4),         // 0xEF

    InstInfo::new_s("LD A, (a8)", 2, 3),    // 0xF0
    InstInfo::new_s("POP AF", 1, 3),        // 0xF1
    InstInfo::new_s("LD A, (C)", 1, 2),     // 0xF2
    InstInfo::new_s("DI", 1, 1),            // 0xF3
    InstInfo::new_s("NONE", 1, 0),          // 0xF4
    InstInfo::new_s("PUSH AF", 1, 4),       // 0xF5
    InstInfo::new_s("OR d8", 2, 2),         // 0xF6
    InstInfo::new_s("RST 6", 1, 4),         // 0xF7
    InstInfo::new_s("LD HL, SP+s8", 2, 3),  // 0xF8
    InstInfo::new_s("LD SP, HL", 1, 2),     // 0xF9
    InstInfo::new_s("LD A, (a16)", 3, 4),   // 0xFA
    InstInfo::new_s("EI", 1, 1),            // 0xFB
    InstInfo::new_s("NONE", 1, 0),          // 0xFC
    InstInfo::new_s("NONE", 1, 0),          // 0xFD
    InstInfo::new_s("CP d8", 2, 2),         // 0xFE
    InstInfo::new_s("RST 7", 1, 4),         // 0xFF
];
// Game Boy CPU instructions for opcodes prefixed by "CB"
pub const SUB_INST_INFO: [InstInfo; 256] = [
    InstInfo::new_s("RLC B", 2, 2),         // 0x00
    InstInfo::new_s("RLC C", 2, 2),         // 0x01
    InstInfo::new_s("RLC D", 2, 2),         // 0x02
    InstInfo::new_s("RLC E", 2, 2),         // 0x03
    InstInfo::new_s("RLC H", 2, 2),         // 0x04
    InstInfo::new_s("RLC L", 2, 2),         // 0x05
    InstInfo::new_s("RLC (HL)", 2, 4),      // 0x06
    InstInfo::new_s("RLC A", 2, 2),         // 0x07
    InstInfo::new_s("RRC B", 2, 2),         // 0x08
    InstInfo::new_s("RRC C", 2, 2),         // 0x09
    InstInfo::new_s("RRC D", 2, 2),         // 0x0A
    InstInfo::new_s("RRC E", 2, 2),         // 0x0B
    InstInfo::new_s("RRC H", 2, 2),         // 0x0C
    InstInfo::new_s("RRC L", 2, 2),         // 0x0D
    InstInfo::new_s("RRC (HL)", 2, 4),      // 0x0E
    InstInfo::new_s("RRC A", 2, 2),         // 0x0F

    InstInfo::new_s("RL B", 2, 2),          // 0x10
    InstInfo::new_s("RL C", 2, 2),          // 0x11
    InstInfo::new_s("RL D", 2, 2),          // 0x12
    InstInfo::new_s("RL E", 2, 2),          // 0x13
    InstInfo::new_s("RL H", 2, 2),          // 0x14
    InstInfo::new_s("RL L", 2, 2),          // 0x15
    InstInfo::new_s("RL (HL)", 2, 4),       // 0x16
    InstInfo::new_s("RL A", 2, 2),          // 0x17
    InstInfo::new_s("RR B", 2, 2),          // 0x18
    InstInfo::new_s("RR C", 2, 2),          // 0x19
    InstInfo::new_s("RR D", 2, 2),          // 0x1A
    InstInfo::new_s("RR E", 2, 2),          // 0x1B
    InstInfo::new_s("RR H", 2, 2),          // 0x1C
    InstInfo::new_s("RR L", 2, 2),          // 0x1D
    InstInfo::new_s("RR (HL)", 2, 4),       // 0x1E
    InstInfo::new_s("RR A", 2, 2),          // 0x1F

    InstInfo::new_s("SLA B", 2, 2),         // 0x20
    InstInfo::new_s("SLA C", 2, 2),         // 0x21
    InstInfo::new_s("SLA D", 2, 2),         // 0x22
    InstInfo::new_s("SLA E", 2, 2),         // 0x23
    InstInfo::new_s("SLA H", 2, 2),         // 0x24
    InstInfo::new_s("SLA L", 2, 2),         // 0x25
    InstInfo::new_s("SLA (HL)", 2, 4),      // 0x26
    InstInfo::new_s("SLA A", 2, 2),         // 0x27
    InstInfo::new_s("SRA B", 2, 2),         // 0x28
    InstInfo::new_s("SRA C", 2, 2),         // 0x29
    InstInfo::new_s("SRA D", 2, 2),         // 0x2A
    InstInfo::new_s("SRA E", 2, 2),         // 0x2B
    InstInfo::new_s("SRA H", 2, 2),         // 0x2C
    InstInfo::new_s("SRA L", 2, 2),         // 0x2D
    InstInfo::new_s("SRA (HL)", 2, 4),      // 0x2E
    InstInfo::new_s("SRA A", 2, 2),         // 0x2F

    InstInfo::new_s("SWAP B", 2, 2),        // 0x30
    InstInfo::new_s("SWAP C", 2, 2),        // 0x31
    InstInfo::new_s("SWAP D", 2, 2),        // 0x32
    InstInfo::new_s("SWAP E", 2, 2),        // 0x33
    InstInfo::new_s("SWAP H", 2, 2),        // 0x34
    InstInfo::new_s("SWAP L", 2, 2),        // 0x35
    InstInfo::new_s("SWAP (HL)", 2, 4),     // 0x36
    InstInfo::new_s("SWAP A", 2, 2),        // 0x37
    InstInfo::new_s("SRL B", 2, 2),         // 0x38
    InstInfo::new_s("SRL C", 2, 2),         // 0x39
    InstInfo::new_s("SRL D", 2, 2),         // 0x3A
    InstInfo::new_s("SRL E", 2, 2),         // 0x3B
    InstInfo::new_s("SRL H", 2, 2),         // 0x3C
    InstInfo::new_s("SRL L", 2, 2),         // 0x3D
    InstInfo::new_s("SRL (HL)", 2, 4),      // 0x3E
    InstInfo::new_s("SRL A", 2, 2),         // 0x3F

    InstInfo::new_s("BIT 0, B", 2, 2),      // 0x40
    InstInfo::new_s("BIT 0, C", 2, 2),      // 0x41
    InstInfo::new_s("BIT 0, D", 2, 2),      // 0x42
    InstInfo::new_s("BIT 0, E", 2, 2),      // 0x43
    InstInfo::new_s("BIT 0, H", 2, 2),      // 0x44
    InstInfo::new_s("BIT 0, L", 2, 2),      // 0x45
    InstInfo::new_s("BIT 0, (HL)", 2, 3),   // 0x46
    InstInfo::new_s("BIT 0, A", 2, 2),      // 0x47
    InstInfo::new_s("BIT 1, B", 2, 2),      // 0x48
    InstInfo::new_s("BIT 1, C", 2, 2),      // 0x49
    InstInfo::new_s("BIT 1, D", 2, 2),      // 0x4A
    InstInfo::new_s("BIT 1, E", 2, 2),      // 0x4B
    InstInfo::new_s("BIT 1, H", 2, 2),      // 0x4C
    InstInfo::new_s("BIT 1, L", 2, 2),      // 0x4D
    InstInfo::new_s("BIT 1, (HL)", 2, 3),   // 0x4E
    InstInfo::new_s("BIT 1, A", 2, 2),      // 0x4F

    InstInfo::new_s("BIT 2, B", 2, 2),      // 0x50
    InstInfo::new_s("BIT 2, C", 2, 2),      // 0x51
    InstInfo::new_s("BIT 2, D", 2, 2),      // 0x52
    InstInfo::new_s("BIT 2, E", 2, 2),      // 0x53
    InstInfo::new_s("BIT 2, H", 2, 2),      // 0x54
    InstInfo::new_s("BIT 2, L", 2, 2),      // 0x55
    InstInfo::new_s("BIT 2, (HL)", 2, 3),   // 0x56
    InstInfo::new_s("BIT 2, A", 2, 2),      // 0x57
    InstInfo::new_s("BIT 3, B", 2, 2),      // 0x58
    InstInfo::new_s("BIT 3, C", 2, 2),      // 0x59
    InstInfo::new_s("BIT 3, D", 2, 2),      // 0x5A
    InstInfo::new_s("BIT 3, E", 2, 2),      // 0x5B
    InstInfo::new_s("BIT 3, H", 2, 2),      // 0x5C
    InstInfo::new_s("BIT 3, L", 2, 2),      // 0x5D
    InstInfo::new_s("BIT 3, (HL)", 2, 3),   // 0x5E
    InstInfo::new_s("BIT 3, A", 2, 2),      // 0x5F

    InstInfo::new_s("BIT 4, B", 2, 2),      // 0x60
    InstInfo::new_s("BIT 4, C", 2, 2),      // 0x61
    InstInfo::new_s("BIT 4, D", 2, 2),      // 0x62
    InstInfo::new_s("BIT 4, E", 2, 2),      // 0x63
    InstInfo::new_s("BIT 4, H", 2, 2),      // 0x64
    InstInfo::new_s("BIT 4, L", 2, 2),      // 0x65
    InstInfo::new_s("BIT 4, (HL)", 2, 3),   // 0x66
    InstInfo::new_s("BIT 4, A", 2, 2),      // 0x67
    InstInfo::new_s("BIT 5, B", 2, 2),      // 0x68
    InstInfo::new_s("BIT 5, C", 2, 2),      // 0x69
    InstInfo::new_s("BIT 5, D", 2, 2),      // 0x6A
    InstInfo::new_s("BIT 5, E", 2, 2),      // 0x6B
    InstInfo::new_s("BIT 5, H", 2, 2),      // 0x6C
    InstInfo::new_s("BIT 5, L", 2, 2),      // 0x6D
    InstInfo::new_s("BIT 5, (HL)", 2, 3),   // 0x6E
    InstInfo::new_s("BIT 5, A", 2, 2),      // 0x6F

    InstInfo::new_s("BIT 6, B", 2, 2),      // 0x70
    InstInfo::new_s("BIT 6, C", 2, 2),      // 0x71
    InstInfo::new_s("BIT 6, D", 2, 2),      // 0x72
    InstInfo::new_s("BIT 6, E", 2, 2),      // 0x73
    InstInfo::new_s("BIT 6, H", 2, 2),      // 0x74
    InstInfo::new_s("BIT 6, L", 2, 2),      // 0x75
    InstInfo::new_s("BIT 6, (HL)", 2, 3),   // 0x76
    InstInfo::new_s("BIT 6, A", 2, 2),      // 0x77
    InstInfo::new_s("BIT 7, B", 2, 2),      // 0x78
    InstInfo::new_s("BIT 7, C", 2, 2),      // 0x79
    InstInfo::new_s("BIT 7, D", 2, 2),      // 0x7A
    InstInfo::new_s("BIT 7, E", 2, 2),      // 0x7B
    InstInfo::new_s("BIT 7, H", 2, 2),      // 0x7C
    InstInfo::new_s("BIT 7, L", 2, 2),      // 0x7D
    InstInfo::new_s("BIT 7, (HL)", 2, 3),   // 0x7E
    InstInfo::new_s("BIT 7, A", 2, 2),      // 0x7F

    InstInfo::new_s("RES 0, B", 2, 2),      // 0x80
    InstInfo::new_s("RES 0, C", 2, 2),      // 0x81
    InstInfo::new_s("RES 0, D", 2, 2),      // 0x82
    InstInfo::new_s("RES 0, E", 2, 2),      // 0x83
    InstInfo::new_s("RES 0, H", 2, 2),      // 0x84
    InstInfo::new_s("RES 0, L", 2, 2),      // 0x85
    InstInfo::new_s("RES 0, (HL)", 2, 4),   // 0x86
    InstInfo::new_s("RES 0, A", 2, 2),      // 0x87
    InstInfo::new_s("RES 1, B", 2, 2),      // 0x88
    InstInfo::new_s("RES 1, C", 2, 2),      // 0x89
    InstInfo::new_s("RES 1, D", 2, 2),      // 0x8A
    InstInfo::new_s("RES 1, E", 2, 2),      // 0x8B
    InstInfo::new_s("RES 1, H", 2, 2),      // 0x8C
    InstInfo::new_s("RES 1, L", 2, 2),      // 0x8D
    InstInfo::new_s("RES 1, (HL)", 2, 4),   // 0x8E
    InstInfo::new_s("RES 1, A", 2, 2),      // 0x8F

    InstInfo::new_s("RES 2, B", 2, 2),      // 0x90
    InstInfo::new_s("RES 2, C", 2, 2),      // 0x91
    InstInfo::new_s("RES 2, D", 2, 2),      // 0x92
    InstInfo::new_s("RES 2, E", 2, 2),      // 0x93
    InstInfo::new_s("RES 2, H", 2, 2),      // 0x94
    InstInfo::new_s("RES 2, L", 2, 2),      // 0x95
    InstInfo::new_s("RES 2, (HL)", 2, 4),   // 0x96
    InstInfo::new_s("RES 2, A", 2, 2),      // 0x97
    InstInfo::new_s("RES 3, B", 2, 2),      // 0x98
    InstInfo::new_s("RES 3, C", 2, 2),      // 0x99
    InstInfo::new_s("RES 3, D", 2, 2),      // 0x9A
    InstInfo::new_s("RES 3, E", 2, 2),      // 0x9B
    InstInfo::new_s("RES 3, H", 2, 2),      // 0x9C
    InstInfo::new_s("RES 3, L", 2, 2),      // 0x9D
    InstInfo::new_s("RES 3, (HL)", 2, 4),   // 0x9E
    InstInfo::new_s("RES 3, A", 2, 2),      // 0x9F

    InstInfo::new_s("RES 4, B", 2, 2),      // 0xA0
    InstInfo::new_s("RES 4, C", 2, 2),      // 0xA1
    InstInfo::new_s("RES 4, D", 2, 2),      // 0xA2
    InstInfo::new_s("RES 4, E", 2, 2),      // 0xA3
    InstInfo::new_s("RES 4, H", 2, 2),      // 0xA4
    InstInfo::new_s("RES 4, L", 2, 2),      // 0xA5
    InstInfo::new_s("RES 4, (HL)", 2, 4),   // 0xA6
    InstInfo::new_s("RES 4, A", 2, 2),      // 0xA7
    InstInfo::new_s("RES 5, B", 2, 2),      // 0xA8
    InstInfo::new_s("RES 5, C", 2, 2),      // 0xA9
    InstInfo::new_s("RES 5, D", 2, 2),      // 0xAA
    InstInfo::new_s("RES 5, E", 2, 2),      // 0xAB
    InstInfo::new_s("RES 5, H", 2, 2),      // 0xAC
    InstInfo::new_s("RES 5, L", 2, 2),      // 0xAD
    InstInfo::new_s("RES 5, (HL)", 2, 4),   // 0xAE
    InstInfo::new_s("RES 5, A", 2, 2),      // 0xAF

    InstInfo::new_s("RES 6, B", 2, 2),      // 0xB0
    InstInfo::new_s("RES 6, C", 2, 2),      // 0xB1
    InstInfo::new_s("RES 6, D", 2, 2),      // 0xB2
    InstInfo::new_s("RES 6, E", 2, 2),      // 0xB3
    InstInfo::new_s("RES 6, H", 2, 2),      // 0xB4
    InstInfo::new_s("RES 6, L", 2, 2),      // 0xB5
    InstInfo::new_s("RES 6, (HL)", 2, 4),   // 0xB6
    InstInfo::new_s("RES 6, A", 2, 2),      // 0xB7
    InstInfo::new_s("RES 7, B", 2, 2),      // 0xB8
    InstInfo::new_s("RES 7, C", 2, 2),      // 0xB9
    InstInfo::new_s("RES 7, D", 2, 2),      // 0xBA
    InstInfo::new_s("RES 7, E", 2, 2),      // 0xBB
    InstInfo::new_s("RES 7, H", 2, 2),      // 0xBC
    InstInfo::new_s("RES 7, L", 2, 2),      // 0xBD
    InstInfo::new_s("RES 7, (HL)", 2, 4),   // 0xBE
    InstInfo::new_s("RES 7, A", 2, 2),      // 0xBF

    InstInfo::new_s("SET 0, B", 2, 2),      // 0xC0
    InstInfo::new_s("SET 0, C", 2, 2),      // 0xC1
    InstInfo::new_s("SET 0, D", 2, 2),      // 0xC2
    InstInfo::new_s("SET 0, E", 2, 2),      // 0xC3
    InstInfo::new_s("SET 0, H", 2, 2),      // 0xC4
    InstInfo::new_s("SET 0, L", 2, 2),      // 0xC5
    InstInfo::new_s("SET 0, (HL)", 2, 4),   // 0xC6
    InstInfo::new_s("SET 0, A", 2, 2),      // 0xC7
    InstInfo::new_s("SET 1, B", 2, 2),      // 0xC8
    InstInfo::new_s("SET 1, C", 2, 2),      // 0xC9
    InstInfo::new_s("SET 1, D", 2, 2),      // 0xCA
    InstInfo::new_s("SET 1, E", 2, 2),      // 0xCB
    InstInfo::new_s("SET 1, H", 2, 2),      // 0xCC
    InstInfo::new_s("SET 1, L", 2, 2),      // 0xCD
    InstInfo::new_s("SET 1, (HL)", 2, 4),   // 0xCE
    InstInfo::new_s("SET 1, A", 2, 2),      // 0xCF

    InstInfo::new_s("SET 2, B", 2, 2),      // 0xD0
    InstInfo::new_s("SET 2, C", 2, 2),      // 0xD1
    InstInfo::new_s("SET 2, D", 2, 2),      // 0xD2
    InstInfo::new_s("SET 2, E", 2, 2),      // 0xD3
    InstInfo::new_s("SET 2, H", 2, 2),      // 0xD4
    InstInfo::new_s("SET 2, L", 2, 2),      // 0xD5
    InstInfo::new_s("SET 2, (HL)", 2, 4),   // 0xD6
    InstInfo::new_s("SET 2, A", 2, 2),      // 0xD7
    InstInfo::new_s("SET 3, B", 2, 2),      // 0xD8
    InstInfo::new_s("SET 3, C", 2, 2),      // 0xD9
    InstInfo::new_s("SET 3, D", 2, 2),      // 0xDA
    InstInfo::new_s("SET 3, E", 2, 2),      // 0xDB
    InstInfo::new_s("SET 3, H", 2, 2),      // 0xDC
    InstInfo::new_s("SET 3, L", 2, 2),      // 0xDD
    InstInfo::new_s("SET 3, (HL)", 2, 4),   // 0xDE
    InstInfo::new_s("SET 3, A", 2, 2),      // 0xDF
    
    InstInfo::new_s("SET 4, B", 2, 2),      // 0xE0
    InstInfo::new_s("SET 4, C", 2, 2),      // 0xE1
    InstInfo::new_s("SET 4, D", 2, 2),      // 0xE2
    InstInfo::new_s("SET 4, E", 2, 2),      // 0xE3
    InstInfo::new_s("SET 4, H", 2, 2),      // 0xE4
    InstInfo::new_s("SET 4, L", 2, 2),      // 0xE5
    InstInfo::new_s("SET 4, (HL)", 2, 4),   // 0xE6
    InstInfo::new_s("SET 4, A", 2, 2),      // 0xE7
    InstInfo::new_s("SET 5, B", 2, 2),      // 0xE8
    InstInfo::new_s("SET 5, C", 2, 2),      // 0xE9
    InstInfo::new_s("SET 5, D", 2, 2),      // 0xEA
    InstInfo::new_s("SET 5, E", 2, 2),      // 0xEB
    InstInfo::new_s("SET 5, H", 2, 2),      // 0xEC
    InstInfo::new_s("SET 5, L", 2, 2),      // 0xED
    InstInfo::new_s("SET 5, (HL)", 2, 4),   // 0xEE
    InstInfo::new_s("SET 5, A", 2, 2),      // 0xEF
    
    InstInfo::new_s("SET 6, B", 2, 2),      // 0xF0
    InstInfo::new_s("SET 6, C", 2, 2),      // 0xF1
    InstInfo::new_s("SET 6, D", 2, 2),      // 0xF2
    InstInfo::new_s("SET 6, E", 2, 2),      // 0xF3
    InstInfo::new_s("SET 6, H", 2, 2),      // 0xF4
    InstInfo::new_s("SET 6, L", 2, 2),      // 0xF5
    InstInfo::new_s("SET 6, (HL)", 2, 4),   // 0xF6
    InstInfo::new_s("SET 6, A", 2, 2),      // 0xF7
    InstInfo::new_s("SET 7, B", 2, 2),      // 0xF8
    InstInfo::new_s("SET 7, C", 2, 2),      // 0xF9
    InstInfo::new_s("SET 7, D", 2, 2),      // 0xFA
    InstInfo::new_s("SET 7, E", 2, 2),      // 0xFB
    InstInfo::new_s("SET 7, H", 2, 2),      // 0xFC
    InstInfo::new_s("SET 7, L", 2, 2),      // 0xFD
    InstInfo::new_s("SET 7, (HL)", 2, 4),   // 0xFE
    InstInfo::new_s("SET 7, A", 2, 2),      // 0xFF
];