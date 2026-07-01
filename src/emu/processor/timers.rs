//!
//! GameBoy Timers
//! 
//!     Passes all of the mooneye-gb acceptance timer tests
//! 


use crate::emu::{memory::mmu::Mmu, sound::apu::Apu};


const TAC_ENABLED_FLAG_BYTE_POSITION: u8  = 2; // Bit 2


#[derive(Default, Debug, Copy, Clone)]
enum TMCFrequency {
    /// 2^12 Hz or every 1024 T-cycles
    #[default]
    F0 = 0b00,
    /// 2^18 Hz or every 16 T-cycles
    F1 = 0b01,
    /// 2^16 Hz or every 64 T-cycles
    F2 = 0b10,
    /// 2^14 Hz or every 256 T-cycles
    F3 = 0b11,
}
impl TMCFrequency {
    fn get_bit_shift(&self)-> u8 {
        match self {
            TMCFrequency::F0 => 9, TMCFrequency::F1 => 3,
            TMCFrequency::F2 => 5, TMCFrequency::F3 => 7,
        }
    }
}
/// | 76543 | 21 --- | 0 ---------- |
/// | ----- | Enable | Clock select |
#[derive(Default, Clone, Copy)]
struct Tac {
    enabled: bool,
    frequency: TMCFrequency,
}
impl From<Tac> for u8 {
    fn from(tac: Tac) -> u8 {
        (tac.enabled as u8) << TAC_ENABLED_FLAG_BYTE_POSITION | tac.frequency as u8
    }
}
impl From<u8> for Tac {
    fn from(byte: u8) -> Self {
        let enabled  = ((byte >> TAC_ENABLED_FLAG_BYTE_POSITION) & 1) != 0;
        let frequency  = match byte & 0b11 {
            0 => TMCFrequency::F0, 1 => TMCFrequency::F1,
            2 => TMCFrequency::F2, _ => TMCFrequency::F3,
        };
        Tac { enabled, frequency }
    }
}

#[derive(Default)]
pub struct Tima {
    value: u8,
    state: TIMAState,
    /// TIMA update delay on reset
    reload_delay: u8,
}
impl Tima {
    fn try_increment(&mut self) {
        if self.state == TIMAState::Normal {
            if self.value == 255 { // If about to overflow
                self.value = 0;
                self.reload_delay = 4;
                self.state = TIMAState::OverflowDelay; // Activate the delay
            } else {
                self.value += 1;
            }
        }
    }
    fn tick(&mut self, tma: u8) -> bool {
        match self.state {
            TIMAState::Normal => false,
            TIMAState::OverflowDelay => {
                // I think == 0 would happen 1 T-cycle too late
                if self.reload_delay > 1 {
                    self.reload_delay -= 1;
                    false
                } else {
                    self.value = tma;
                    self.state = TIMAState::Reloading;
                    true
                }
            }
            TIMAState::Reloading => {
                self.state = TIMAState::Normal;
                false
            }
        }
    }
}

#[derive(Default, PartialEq)]
pub enum TIMAState {
    #[default]
    Normal,
    /// Reload phase 1: TIMA is 0x00
    OverflowDelay,
    /// Reload phase 2: TIMA reloaded with TMA, Interrupt fires
    Reloading,
}


#[derive(Default)]
pub struct Timers {
    /// Internal 16-bit divider
    div_counter: u16,
    /// Timer Counter - FF05
    tima: Tima,
    /// Timer Modulo - FF06
    tma: u8,
    /// Timer Control - FF07
    tac: Tac,
}
impl Timers {
    // It returns bool to trigger timer interrupt
    pub fn tick4(&mut self, apu: &mut Apu) -> bool {
        let mut result = false;
        for _t in 0..4u8 {
            if self.tima.tick(self.tma) { result = true };
            self.divider_tick(apu);
        }
        result
    }

    pub fn read_div(&self) -> u8 { (self.div_counter >> 8) as u8 }
    pub fn write_div(&mut self, apu: &mut Apu) { self.update_div(apu, 0); }
    fn divider_tick(&mut self, apu: &mut Apu) { self.update_div(apu, self.div_counter.wrapping_add(1)); }
    fn update_div(&mut self, apu: &mut Apu, value: u16) {
        let old_value = self.div_counter;
        self.div_counter = value; // Update the value

        let old_signal = self.timer_signal(old_value);
        self.check_tima_falling_edge(old_signal, value);
        self.check_div_apu_falling_edge(apu, old_value, value);
    }

    // Falling edge detection
    fn check_tima_falling_edge(&mut self, old_signal: bool, new_value: u16) {
        // It uses bit shift to determine the timing
        let new_signal = self.timer_signal(new_value);
        let should_increment =  old_signal && !new_signal; // TIMA increments on 1 becoming 0
        if should_increment { self.tima.try_increment(); }
    }
    fn check_div_apu_falling_edge(&mut self, apu: &mut Apu, old_value: u16, new_value: u16) {
        let old_div_apu_bit = (((old_value >> 8) >> 4) & 1) != 0;
        let new_div_apu_bit = (((new_value >> 8) >> 4) & 1) != 0;
        if old_div_apu_bit && !new_div_apu_bit { apu.sequencer_step(); } // Trigger the audio sequencer every 8192 T-cycles
    }
    fn timer_signal(&self, value: u16 ) -> bool {
        let timer_bit = self.tac.frequency.get_bit_shift();
        self.tac.enabled && ((value >> timer_bit) & 1 != 0)
    }

    pub fn read_tima(&self) -> u8 { self.tima.value }
    pub fn write_tima(&mut self, value: u8) {
        // Fails "tima_write_reloading.gb" without those
        if self.tima.state == TIMAState::Reloading { return; }
        if self.tima.state == TIMAState::OverflowDelay { self.tima.state = TIMAState::Normal; }

        self.tima.value = value;
    }

    pub fn read_tma(&self) -> u8 { self.tma }
    pub fn write_tma(&mut self, value: u8) {
        self.tma = value;
        // Fails "tma_write_reloading.gb" without this line
        if self.tima.state == TIMAState::Reloading { self.tima.value = value; }
    }
    pub fn read_tac(&self) -> u8 { u8::from(self.tac) | 0b11111000 }
    pub fn write_tac(&mut self, value: u8) {
        let old_signal = self.timer_signal(self.div_counter);
        self.tac = (value & 0b111).into();
        // DIV stays the same, but TAC might update the signal
        self.check_tima_falling_edge(old_signal, self.div_counter);
    }
}

// [0xFF04, 0xFF07]
impl Mmu {
    pub fn read_timers(&self, address: usize) -> u8 {
        match address {
            0xFF04 => self.timers.read_div(),   // DIV
            0xFF05 => self.timers.read_tima(),  // TIMA
            0xFF06 => self.timers.read_tma(),   // TMA
            0xFF07 => self.timers.read_tac(),   // TAC
            _ => unreachable!("address: {:#06X}", address)
        }
    }
    pub fn write_timers(&mut self, address: usize, value: u8) {
        match address {
            0xFF04 => self.timers.write_div(&mut self.apu), // DIV, value gets discarded
            0xFF05 => self.timers.write_tima(value),        // TIMA
            0xFF06 => self.timers.write_tma(value),         // TMA
            0xFF07 => self.timers.write_tac(value),         // TAC
            _ => unreachable!("address: {:#06X}", address)
        }
    }
}