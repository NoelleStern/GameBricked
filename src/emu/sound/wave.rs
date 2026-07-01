//! 
//! Wave sound channel e.g. Ch3:
//! 
//!     Wave RAM is 16 bytes long; each byte holds two samples, each 4 bits.
//! 


use smart_default::SmartDefault;

use crate::emu::sound::apu::{AudioChannel, LengthCounter, PhaseTimer};


#[derive(SmartDefault)]
pub struct WaveChannel {
    // Registers:
        /// NR30 byte value
        pub nr30: u8,
        /// NR31 byte value (write-only)
        pub nr31: u8,
        /// NR32 byte value
        pub nr32: u8,
        /// NR33 byte value (write-only)
        pub nr33: u8,
        /// NR34 byte value
        pub nr34: u8,
    // Channel state:
        /// DAC state
        pub dac_enabled: bool,
        /// Channel state
        pub channel_enabled: bool,
    // Implementation:
        /// Main frequency timer
        #[default(PhaseTimer::new(2, 32))]
        phase_timer: PhaseTimer,
        /// Disables the channel after a certain duration
        #[default(LengthCounter::new(256))]
        pub length_counter: LengthCounter,
        /// 16 bytes of 4-bit samples
        pub wave_ram: [u8; 16],          
        /// Output volume level      
        output_level: u8,
        /// Holds last buffered sample
        pub sample_buffer: u8,
}
impl WaveChannel {
    fn get_output_level(value: u8) -> u8 { (value >> 5) & 0b11 }
    
    // R/W Wave RAM [0xFF30, 0xFF3F]
    pub fn read(&self, address: usize) -> u8 {
        // TODO:
        // Quirk:
        // If the wave channel is enabled, accessing any byte from 0xFF30-0xFF3F is
        // equivalent to accessing the current byte selected by the waveform position.
        // Further, on the DMG accesses will only work in this manner if made within a couple of clocks
        // of the wave channel accessing wave RAM; if made at any other time, reads return 0xFF and writes have no effect.

        if self.channel_enabled { 0xFF }
        else { self.wave_ram[address - 0xFF30] }
    }
    pub fn write(&mut self, address: usize, value: u8) {
        if self.channel_enabled { return; }
        self.wave_ram[address - 0xFF30] = value
    }

    // Writes
    /// | 7 -------- | 6543210 |
    /// | DAC on/off | ------- |
    pub fn write0(&mut self, value: u8) {
        self.nr30 = value;
        self.dac_enabled = Self::get_dac(value);
        if !self.dac_enabled { self.channel_enabled = false; } // Disabled DAC disables the channel
    }
    /// | 76543210 ----------- |
    /// | Initial length timer |
    pub fn write1(&mut self, value: u8) {
        self.nr31 = value;
        self.length_counter.write1(value);
    }
    /// | 7 | 65 --------- | 43210 |
    /// | - | Output level | ----- |
    pub fn write2(&mut self, value: u8) {
        self.nr32 = value;
        self.output_level = Self::get_output_level(value);
    }
    /// This register stores the low 8 bits of the channel's 11-bit "period value"
    pub fn write3(&mut self, value: u8) {
        self.nr33 = value;
        self.phase_timer.write3(value);
    }
    /// | 7 ----- | 6 ----------- | 543 | 210 -- |
    /// | Trigger | Length enable | --- | Period |
    pub fn write4(&mut self, value: u8, sequencer_phase: u8) {
        self.nr34 = value;
        self.phase_timer.write4(value);
        self.write4_common(value, sequencer_phase);
    }
}
impl AudioChannel for WaveChannel {
    fn get_dac(value: u8) -> bool { (value >> 7) != 0 } // Because it's different for Ch3
    fn length_counter(&mut self) -> &mut LengthCounter { &mut self.length_counter }
    fn channel_enabled(&mut self) -> &mut bool { &mut self.channel_enabled }

    /// 1. Ch3 is enabled                                               (Channel is enabled)
    /// 2. If the length timer expired it is reset                      (If length counter is zero, it is set to max)
    /// 3. The period divider is set to the contents of NR33 and NR34   (Frequency timer is reloaded with period)
    /// 4. Volume is set to contents of NR32 initial volume             (Channel volume is reloaded from NRx2)
    /// 5. Wave RAM index is reset, but its not refilled                (Wave channel's position is set to 0 but sample buffer is NOT refilled)
    /// 6. Note that if the channel's DAC is off, after the above actions occur the channel will be immediately disabled again
    fn trigger(&mut self) {
        self.channel_enabled = true;                            // 1
        self.length_counter.trigger(&mut self.channel_enabled); // 2
        self.phase_timer.trigger();                             // 3
        self.output_level = Self::get_output_level(self.nr32); // 4
        self.phase_timer.phase = 0;                             // 5
        if !self.dac_enabled { self.channel_enabled = false; }  // 6
    }

    /// The wave channel's frequency timer period is set to (2048-frequency) * 2
    /// When the timer generates a clock, the position counter is advanced one sample
    /// in the wave table, looping back to the beginning when it goes past the end,
    /// then a sample is read into the sample buffer from this NEW position.
    fn waveform_tick(&mut self) {
        if !self.channel_enabled { return; }
        
        if self.phase_timer.tick() {
            // Sample buffer refills only here
            let byte_index = (self.phase_timer.phase / 2) as usize;
            let raw_byte = self.wave_ram[byte_index];
            self.sample_buffer = if self.phase_timer.phase.is_multiple_of(2) { (raw_byte >> 4) & 0x0F } else { raw_byte & 0x0F};
        }
    }

    fn sample(&self) -> u8 {
        if !self.channel_enabled { return 0; } // Return zero if the channel is off

        // https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Wave_Channel
        match self.output_level {
            1 => self.sample_buffer,        // 100% Volume
            2 => self.sample_buffer >> 1,   // 50% Volume
            3 => self.sample_buffer >> 2,   // 25% Volume
            _ => self.sample_buffer >> 4    // Silent
        }
    }
}