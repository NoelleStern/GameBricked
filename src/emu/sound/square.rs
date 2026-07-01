//! 
//! Square sound channels e.g. Ch1 and Ch2
//! 


use smart_default::SmartDefault;

use crate::emu::sound::apu::{AudioChannel, AudioTimer, LengthCounter, PhaseTimer, VolumeEnvelope};


/// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Square_Wave
const DUTY_TABLE: [[u8;8];4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];


/// Changes pitch overtime
#[derive(Default)]
pub struct FrequencySweep {
    // Registers:
        /// NR10 byte value
        pub nr10: u8,
    // Implementation:
        /// Timer counter
        timer: AudioTimer,
        /// State
        /// Enabled / Disabled
        enabled: bool,
        /// Shadow frequency to operate on
        shadow_frequency: u16,
        /// Frequency shift value
        shift: u8,
        /// Negate flag
        /// 0: Addition / 1: Subtraction
        negate: bool,
        /// Actual period value
        actual_period: u16,
    // Quirk:
        /// Can disable the channel under certain conditions
        negate_used: bool,
}
impl FrequencySweep {
    pub fn get_pace(value: u8) -> u8        { ( value >> 4) & 0b111   }
    pub fn get_negate(value: u8) -> bool    { ((value >> 3) & 1) != 0 }
    pub fn get_shift(value: u8) -> u8       {   value       & 0b111   }

    /// | 7 | 654  | 3 ------- | 210 ----------- |
    /// | - | Pace | Direction | Individual step |
    pub fn write0(&mut self, value: u8, channel_enabled: &mut bool) {
        let old_negate = self.negate;

        self.nr10 = value;

        let old_period = self.actual_period;
        self.actual_period = Self::get_pace(value) as u16;

        // However, if 0 is written to this field, then iterations are instantly
        // disabled, and it will be reloaded as soon as it’s set to something else
        self.timer.period = if self.actual_period == 0 { 8 } else { self.actual_period }; // The volume envelope and sweep timers treat a period of 0 as 8
        if old_period == 0 && self.actual_period != 0 { self.timer.reload() }

        self.negate = Self::get_negate(value);
        self.shift = Self::get_shift(value);
        
        // Quirk: 
        // Clearing the sweep negate mode bit in NR10 after at least one sweep calculation has been made 
        // using the negate mode since the last trigger causes the channel to be immediately disabled. 
        // This prevents you from having the sweep lower the frequency then raise the frequency without a trigger in-between.
        if old_negate && !self.negate && self.negate_used { *channel_enabled = false; }
    }

    /// 1. Ch1 period value is copied to the "shadow register"  (Square 1's frequency is copied to the shadow register)
    /// 2. The "sweep timer" is reset                           (The sweep timer is reloaded)
    /// 3. The "enabled flag" is set if either the sweep pace or individual step are non-zero, cleared otherwise
    ///    (The internal enabled flag is set if either the sweep period or shift are non-zero, cleared otherwise)
    /// 4. If the individual step is non-zero, frequency calculation and overflow check are performed immediately
    ///    (If the sweep shift is non-zero, frequency calculation and the overflow check are performed immediately)
    pub fn trigger(&mut self, frequency: u16, channel_enabled: &mut bool) {
        self.negate_used = false; // Do it before overflow check or quirk fails

        self.shadow_frequency = frequency;                              // 1
        self.timer.reload();                                            // 2
        self.enabled = (self.actual_period != 0) || (self.shift != 0);  // 3
        if self.shift != 0 { self.overflow_check(channel_enabled); }    // 4
    }

    /// The overflow check simply calculates the new frequency and if it is greater than 2047, or 0x7FF, Ch1 is disabled.
    fn overflow_check(&mut self, channel_enabled: &mut bool) -> (u16, bool) {
        let new_frequency = self.calculate_frequency(); // Calculate
        if new_frequency > 2047 { *channel_enabled = false; (new_frequency, true) } // Disable
        else { (new_frequency, false) } 
    }
    /// Frequency calculation consists of:
    ///     1. Taking the value in the frequency "shadow register"
    ///     2. Shifting it right by the individual step, optionally negating the value (depending on the direction)
    ///     3. Summing this with the frequency "shadow register" to produce a new frequency
    /// 
    /// What is done with this new frequency depends on the context.
    fn calculate_frequency(&mut self) -> u16 {
        let delta: u16 = self.shadow_frequency >> self.shift;   // 1
        if self.negate {                                        // 2
            self.negate_used = true;
            self.shadow_frequency.wrapping_sub(delta)           // 3
        } else { self.shadow_frequency + delta }                // 3
    }
    
    /// The sweep timer is clocked at 128 Hz by the frame sequencer
    ///
    /// 1. When it generates a clock:
    ///   - 2. And the sweep's internal enabled flag is set and the sweep period is not zero:
    ///      - 3. A new frequency is calculated and the overflow check is performed
    ///      - 4. If the new frequency is 2047 or less and the sweep shift is not zero:
    ///         - 5. This new frequency is written back to the shadow frequency and square 1's frequency in NR13 and NR14
    ///         - 6. Then frequency calculation and overflow check are run AGAIN immediately using this new value, but this second new frequency is not written back
    pub fn step(&mut self, channel_enabled: &mut bool) -> Option<u16> {
        if self.timer.input_clock() { // 1
            if self.enabled && self.actual_period != 0 { // 2
                let (new_frequency, overflow) =
                    self.overflow_check(channel_enabled);   // 3
                if !overflow && self.shift != 0 {           // 4
                    self.shadow_frequency = new_frequency;      // 5
                    self.overflow_check(channel_enabled);       // 6
                    return Some(new_frequency);                 // 5, a little sequence break
                }
            }
        }
        None
    }

}


#[derive(SmartDefault)]
pub struct SquareChannel {
    // Registers:
        /// NRx1 byte value
        pub nrx1: u8,
        /// NRx2 byte value
        pub nrx2: u8,
        /// NRx3 byte value (write-only)
        pub nrx3: u8,
        /// NRx4 byte value
        pub nrx4: u8,
    // Channel state:
        /// DAC state
        pub dac_enabled: bool,
        /// Enabled / Disabled
        pub channel_enabled: bool,
    // Implementation:
        /// Changes pitch over time, only available in Ch1 aka Square 1
        pub sweep: Option<FrequencySweep>,
        /// Disables the channel after a certain duration
        #[default(LengthCounter::new(64))]
        pub length_counter: LengthCounter,
        /// Volume envelope object responsible for volume sweep
        envelope: VolumeEnvelope,
        /// Main frequency timer
        pub phase_timer: PhaseTimer,
        /// Current duty pattern id
        duty_pattern_id: usize,
}
impl SquareChannel {
    pub fn new(sweep: bool) -> Self {
        let ps = if sweep { Some(FrequencySweep::default()) } else { None };
        Self { sweep: ps, ..Default::default() }
    }
    fn get_duty(value: u8) -> u8 { value >> 6 }

    pub fn read0(&self) -> u8 {  self.sweep.as_ref().unwrap().nr10 }

    // Writes
    /// | 7 | 654  | 3 ------- | 210 ----------- |
    /// | - | Pace | Direction | Individual step |
    pub fn write0(&mut self, value: u8) {
        self.sweep.as_mut().unwrap().write0(value, &mut self.channel_enabled) 
    }
    /// | 76 ------ | 543210 ------------- |
    /// | Wave duty | Initial length timer |
    pub fn write1(&mut self, value: u8) {
        self.nrx1 = value;
        self.duty_pattern_id = Self::get_duty(value) as usize; 
        self.length_counter.write1(value);
    }
    /// | 7654 --------- | 3 ----- | 210 ------ |
    /// | Initial volume | Env dir | Sweep pace |
    pub fn write2(&mut self, value: u8) {
        self.nrx2 = value;
        self.envelope.write2(value);
        self.dac_enabled = Self::get_dac(value);
        if !self.dac_enabled { self.channel_enabled = false; } // Disabled DAC disables the channel
    }
    /// This register stores the low 8 bits of the channel's 11-bit "period value"
    pub fn write3(&mut self, value: u8) {
        self.nrx3 = value;
        self.phase_timer.write3(value);
    }
    /// | 7 ----- | 6 ----------- | 543 | 210 -- |
    /// | Trigger | Length enable | --- | Period |
    pub fn write4(&mut self, value: u8, sequencer_phase: u8) {
        self.nrx4 = value;
        self.phase_timer.write4(value);
        self.write4_common(value, sequencer_phase);
    }

    // Ticks and steps
    pub fn envelope_step(&mut self) {
        if !self.channel_enabled { return; }
        self.envelope.step();
    }
    pub fn sweep_step(&mut self) {
        if !self.channel_enabled { return; }

        let new_frequency = 
            self.sweep.as_mut().unwrap().step(&mut self.channel_enabled);
        
        if let Some(frequency) = new_frequency {
            self.write3(frequency as u8);
            // Avoid self.write4() since it would cause re-trigger
            self.nrx4 = (self.nrx4 & 0b1111_1000) | (((frequency >> 8) & 0b111) as u8);
            self.phase_timer.write4(self.nrx4);
        }
    }
}
impl AudioChannel for SquareChannel {
    fn length_counter(&mut self) -> &mut LengthCounter { &mut self.length_counter }
    fn channel_enabled(&mut self) -> &mut bool { &mut self.channel_enabled }

    /// 1. Ch1 is enabled                                               (Channel is enabled)
    /// 2. If length timer expired it is reset                          (If length counter is zero, it is set to max)
    /// 3. The period divider is set to the contents of NR13 and NR14   (Frequency timer is reloaded with period)
    /// 4. Envelope timer is reset                                      (Volume envelope timer is reloaded with period)
    /// 5. Volume is set to contents of NR12 initial volume             (Channel volume is reloaded from NRx2)
    /// 6. Sweep does several things                                    (Square 1's sweep does several things)
    /// 7. Note that if the channel's DAC is off, after the above actions occur the channel will be immediately disabled again
    fn trigger(&mut self) {
        self.channel_enabled = true;                            // 1
        self.length_counter.trigger(&mut self.channel_enabled); // 2
        self.phase_timer.trigger();                             // 3
        self.envelope.trigger();                                // 4, 5

        if let Some(sweep) = self.sweep.as_mut() { 
            sweep.trigger(self.phase_timer.frequency, &mut self.channel_enabled); // 6
        }

        if !self.dac_enabled { self.channel_enabled = false; }  // 7
    }

    fn waveform_tick(&mut self) {
        if !self.channel_enabled { return; }
        self.phase_timer.tick();
    }

    fn sample(&self) -> u8 {
        if !self.channel_enabled { return 0; } // Return zero if the channel is off
        let bit = DUTY_TABLE[self.duty_pattern_id][self.phase_timer.phase as usize]; // Get current duty pattern value
        if bit == 1 { self.envelope.volume } else { 0 }
    }
}