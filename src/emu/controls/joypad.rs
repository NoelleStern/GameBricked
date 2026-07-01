//!
//! 0xFF00: Joypad buttons [R/W]:
//! 
//!     +-----------+-----------+-------+
//!     | Low/High  | Bit5      | Bit4  |
//!     +-----------+-----------+-------+
//!     | Bit3      | START     | DOWN  |
//!     | Bit2      | SELECT    | UP    |
//!     | Bit1      | B         | LEFT  |
//!     | Bit0      | A         | RIGHT |
//!     +-----------+-----------+-------+
//! 


use crate::emu::memory::mmu::Mmu;


// | 76 | 5 ------------ | 4 ---------- | 3 -------- | 2 ------- | 1 ---- | 0 ----- |
// | -- | Select buttons | Select D-pad | Start/Down | Select/Up | B/Left | A/Right |
pub const JOYPAD_ADDRESS: usize             = 0xFF00;
const MAIN_BUTTONS_FLAG_BYTE_POSITION: u8   = 5; // Bit 5
const DPAD_FLAG_BYTE_POSITION: u8           = 4; // Bit 4
const START_DOWN_FLAG_BYTE_POSITION: u8     = 3; // Bit 3
const SELECT_UP_FLAG_BYTE_POSITION: u8      = 2; // Bit 2
const B_LEFT_FLAG_BYTE_POSITION: u8         = 1; // Bit 1
const A_RIGHT_FLAG_BYTE_POSITION: u8        = 0; // Bit 0


#[derive(Default, Clone)]
pub struct SoftwareJoypad {
    pub a:      bool, pub b:        bool,
    pub select: bool, pub start:    bool,
    pub up:     bool, pub down:     bool,
    pub left:   bool, pub right:    bool,
}
impl SoftwareJoypad {
    pub fn get_main(&self) -> u8 {
        (!self.start as u8 )    << START_DOWN_FLAG_BYTE_POSITION |
        (!self.select as u8)    << SELECT_UP_FLAG_BYTE_POSITION  |
        (!self.b as u8     )    << B_LEFT_FLAG_BYTE_POSITION     |
        (!self.a as u8     )    << A_RIGHT_FLAG_BYTE_POSITION
    }
    pub fn get_dpad(&self) -> u8  {
        (!self.down as u8 )     << START_DOWN_FLAG_BYTE_POSITION |
        (!self.up as u8   )     << SELECT_UP_FLAG_BYTE_POSITION  |
        (!self.left as u8 )     << B_LEFT_FLAG_BYTE_POSITION     |
        (!self.right as u8)     << A_RIGHT_FLAG_BYTE_POSITION
    }
    pub fn combine(a: &Self, b: &Self) -> Self {
        Self {
            a:      a.a      | b.a,
            b:      a.b      | b.b,
            select: a.select | b.select,
            start:  a.start  | b.start,
            up:     a.up     | b.up,
            down:   a.down   | b.down,
            left:   a.left   | b.left,
            right:  a.right  | b.right
        }
    }
}
impl<'a> IntoIterator for &'a SoftwareJoypad {
    type Item = &'a bool;
    type IntoIter = std::array::IntoIter<Self::Item, 8>;
    fn into_iter(self) -> Self::IntoIter {
        [
            &self.a, &self.b, &self.select, &self.start,
            &self.up, &self.down, &self.left, &self.right,
        ].into_iter()
    }
}

#[derive(Default, Clone)]
pub struct HardwareJoypad {
    pub main: bool,
    pub dpad: bool,
}
impl HardwareJoypad {
    pub fn get_state(&self, joypad: &SoftwareJoypad) -> u8 {
        let upper: u8 = self.clone().into();
        if      self.main   { upper | joypad.get_main() }
        else if self.dpad   { upper | joypad.get_dpad() }
        else                { upper | 0xF               } // If neither buttons nor D-pad is selected, then the lower nibble reads 0xF
    }
}
impl From<HardwareJoypad> for u8 {
    fn from(joypad: HardwareJoypad) -> u8 {
        (!joypad.main as u8) << MAIN_BUTTONS_FLAG_BYTE_POSITION |
        (!joypad.dpad as u8) << DPAD_FLAG_BYTE_POSITION
    }
}
impl From<u8> for HardwareJoypad {
    fn from(byte: u8) -> Self {
        let main = ((byte >> MAIN_BUTTONS_FLAG_BYTE_POSITION) & 1) != 1;
        let dpad = ((byte >> DPAD_FLAG_BYTE_POSITION        ) & 1) != 1;
        HardwareJoypad { main, dpad }
    }
}

// 0xFF00
impl Mmu {
    pub fn read_joypad(&self) -> u8 {
        let joypad: HardwareJoypad = self.memory[JOYPAD_ADDRESS].into();
        joypad.get_state(&self.joypad)
    }
}