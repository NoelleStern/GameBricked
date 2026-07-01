//! 
//! Noise sound channel e.g. Ch4
//! 


use smart_default::SmartDefault;

use crate::emu::sound::apu::{AudioChannel, AudioTimer, LengthCounter, VolumeEnvelope};


#[derive(SmartDefault)]
pub struct NoiseChannel {
    // Registers:
        /// NR41 byte value (write-only)
        pub nr41: u8,
        /// NR42 byte value
        pub nr42: u8,
        /// NR43 byte value (write-only)
        pub nr43: u8,
        /// NR44 byte value
        pub nr44: u8,
    // Channel state:
        /// DAC state
        pub dac_enabled: bool,
        /// Channel state
        pub channel_enabled: bool,
    // Implementation:
        /// Timer counter
        timer: AudioTimer,
        /// Disables the channel after a certain duration
        #[default(LengthCounter::new(64))]
        pub length_counter: LengthCounter, 
        /// Volume envelope object responsible for volume sweep
        envelope: VolumeEnvelope,
        /// LFSR width mode
        lfsr_width: bool,
        /// Clock shift value
        clock_shift: u8,
        /// LFSR value
        #[default(0x7FFF)]
        lfsr: u16,
}
impl NoiseChannel {
    fn get_clock_shift(value: u8) -> u8         { ( value >> 4) & 0b1111  }
    fn get_lfsr_width(value: u8) -> bool        { ((value >> 3) & 1) != 0 }
    fn get_clock_divider_code(value: u8) -> u8  {   value       & 0b111   }

    /// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Noise_Channel
    /// 
    /// According to Pan Docs:
    ///    Clock divider: 
    ///        See the frequency formula below. Note that divider = 0 is treated as divider = 0.5 instead.
    ///        (0 => 8 here does exactly that)  
    fn convert_divider(value: u8) -> u8 {
        match value { 0 => 8,  1 => 16, 2 => 32, 3 => 48, 4 => 64, 5 => 80, 6 => 96, _ => 112 }
    }

    // Writes
    /// | 76 | 543210 ------------- |
    /// | -- | Initial length timer |
    pub fn write1(&mut self, value: u8) {
        self.nr41 = value;
        self.length_counter.write1(value);
    }
    /// | 7654 --------- | 3 ----- | 210 ------ |
    /// | Initial volume | Env dir | Sweep pace |
    pub fn write2(&mut self, value: u8) {
        self.nr42 = value;
        self.envelope.write2(value);
        self.dac_enabled = Self::get_dac(value);
        if !self.dac_enabled { self.channel_enabled = false; } // Disabled DAC disables the channel
    }
    /// | 7654 ------ | 3 -------- | 210 --------- |
    /// | Clock shift | LFSR width | Clock divider |
    pub fn write3(&mut self, value: u8) {
        self.nr43 = value;
        self.clock_shift = Self::get_clock_shift(value);
        self.lfsr_width = Self::get_lfsr_width(value);
        let clock_divider = Self::convert_divider(
            Self::get_clock_divider_code(value)
        );

        self.timer.period = (clock_divider as u16) << self.clock_shift;
    }
    /// | 7 ----- | 6 ----------- | 543210 |
    /// | Trigger | Length enable | ------ |
    pub fn write4(&mut self, value: u8, sequencer_phase: u8) {
        self.nr44 = value;
        self.write4_common(value, sequencer_phase);
    }

    // Ticks and steps
    pub fn envelope_step(&mut self) {
        if !self.channel_enabled { return; }
        self.envelope.step();
    }
}
impl AudioChannel for NoiseChannel {
    fn length_counter(&mut self) -> &mut LengthCounter { &mut self.length_counter }
    fn channel_enabled(&mut self) -> &mut bool { &mut self.channel_enabled }

    /// 1. Ch4 is enabled                                   (Channel is enabled)
    /// 2. If the length timer expired it is reset          (If length counter is zero, it is set to max)
    /// 3. Envelope timer is reset                          (Volume envelope timer is reloaded with period)
    /// 4. Volume is set to contents of NR42 initial volume (Channel volume is reloaded from NRx2)
    /// 5. LFSR bits are reset                              (Noise channel's LFSR bits are all set to 1)
    /// 6. Note that if the channel's DAC is off, after the above actions occur the channel will be immediately disabled again
    fn trigger(&mut self) {
        self.channel_enabled = true;                            // 1
        self.length_counter.trigger(&mut self.channel_enabled); // 2
        self.envelope.trigger();                                // 3, 4
        self.lfsr = 0x7FFF;                                     // 5
        if !self.dac_enabled { self.channel_enabled = false; }  // 6
    }

    /// We don't care about hardcore hardware accuracy!
    /// So let's pretend 15 bits (0-14) is all LFSR has, alright?
    fn waveform_tick(&mut self) {
        if !self.channel_enabled { return; }

        if self.timer.input_clock() {
            let bit0 = self.lfsr & 1; // Get bit 0
            let bit1 = (self.lfsr >> 1) & 1; // Get bit 1
            let xor_bit = bit0 ^ bit1; // Calculate xor bit

            self.lfsr >>= 1; // Shift right and
            self.lfsr &= !(1 << 14); // Clear bit 14
            self.lfsr |= xor_bit << 14; // Update bit 14

            // Handle short mode
            if self.lfsr_width {
                self.lfsr &= !(1 << 6); // Clear bit 6
                self.lfsr |= xor_bit << 6; // Update  bit 6
            }
        }
    }

    fn sample(&self) -> u8 {
        if !self.channel_enabled { return 0; } // Return no sound if the channel is off

        // If the bit shifted out is a 0, the channel emits a 0 (if you ignore that, the channel hisses)
        // Except that shift being equal to 14 or 15 stops the channel from being clocked entirely
        if self.clock_shift == 0 || self.clock_shift >= 14 { return 0; }

        // The waveform output is bit 0 of the LFSR, INVERTED
        if (self.lfsr & 1) == 0 { self.envelope.volume } else { 0 }
    }
}