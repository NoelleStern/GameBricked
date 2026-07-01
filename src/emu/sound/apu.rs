//! 
//! GameBoy's Audio Processing Unit:
//!     
//!     A general rule of thumb for NR1x-NR4x is the following:
//!         NRx0 is some channel-specific feature (if present)
//!         NRx1 controls length timer
//!         NRx2 controls volume and envelope
//!         NRx3 controls period (maybe only partially)
//!         NRx4 has the channel's trigger and length timer enable bits, as well as any leftover bits of period
//! 
//!     Both of those sources are crucial:
//!     https://gbdev.io/pandocs/Audio_Registers.html
//!     https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
//! 


use std::mem;
use smart_default::SmartDefault;

use crate::emu::{memory::mmu::Mmu, sound::{noise::NoiseChannel, square::SquareChannel, wave::WaveChannel}};


pub trait AudioChannel {
    // Helpers:
        /// Only true for Ch1, Ch2 and Ch4, Ch3 overrides it
        fn get_dac(value: u8) -> bool { (value & 0b1111_1000) != 0 }
        /// Get trigger flag from the given byte
        fn get_trigger(value: u8) -> bool { (value >> 7) != 0 }
    // Getters:
        fn length_counter(&mut self) -> &mut LengthCounter;
        fn channel_enabled(&mut self) -> &mut bool;
    // Methods:
        /// (Re)trigger the channel
        fn trigger(&mut self);
        /// Tick the waveform at a CPU speed
        fn waveform_tick(&mut self);
        /// Sample the audio
        fn sample(&self) -> u8;
        /// Disabled channels should still clock length
        fn length_step(&mut self) {
            if self.length_counter().step() { *self.channel_enabled() = false; }
        }
    // Special helpers:
        fn write4_common(&mut self, value: u8, sequencer_phase: u8) {
            if self.length_counter().write4(value, sequencer_phase) { *self.channel_enabled() = false };
            if Self::get_trigger(value) { self.trigger() }
        }
}

/// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Timer
/// 
/// A timer generates an output clock every N input clocks, where N is the timer's period.
/// If a timer's rate is given as a frequency, its period is 4194304/frequency in Hz.
/// Each timer has an internal counter that is decremented on each input clock.
/// When the counter becomes zero, it is reloaded with the period and an output clock is generated.
#[derive(Default)]
pub struct AudioTimer { // Most commonly frequency timer, but doesn't have to be
    /// Timer counter
    counter: u16,
    /// Pace timing (update period)
    pub period: u16,
}
impl AudioTimer {
    pub fn new(value: u16) -> Self {
        Self { counter: value, period: value }
    }

    pub fn reload(&mut self) { self.counter = self.period; }
    pub fn manual_reload(&mut self, value: u16) { self.counter = value; }
    pub fn input_clock(&mut self) -> bool {             // The return bool represents an output clock
        self.counter = self.counter.saturating_sub(1);  // Decrement
        if self.counter == 0 { self.reload(); true }    // Reload
        else { false }
    }
}


/// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Volume_Envelope
/// 
/// A volume envelope has a volume counter and an internal timer clocked at 64 Hz by the frame sequencer.
/// When the timer generates a clock and the envelope period is not zero, a new volume
/// is calculated by adding or subtracting (as set by NRx2) one from the current volume. 
/// If this new volume within the 0 to 15 range, the volume is updated, otherwise it is left unchanged
/// and no further automatic increments/decrements are made to the volume until the channel is triggered again.
#[derive(Default)]
pub struct VolumeEnvelope {
    // Active counter values:
        /// Current volume value
        pub volume: u8,
        /// Direction - 0: Decreasing / 1: Increasing
        increasing: bool,
        /// Countdown timer
        timer: AudioTimer,
    // Configured values via registers:
        /// Configured initial volume by NRx2
        configured_volume: u8,
        /// Configured direction by NRx2
        configured_increasing: bool,
        /// Configured sweep period by NRx2
        configured_period: u8,
}
impl VolumeEnvelope {
    pub fn get_volume(value: u8) -> u8      {   value >> 4            }
    pub fn get_direction(value: u8) -> bool { ((value >> 3) & 1) != 0 }
    pub fn get_period(value: u8) -> u8      {   value       & 0b111   }

    /// | 7654 --------- | 3 ----- | 210 ------ |
    /// | Initial volume | Env dir | Sweep pace |
    /// 
    /// The volume envelope and sweep timers treat
    /// a period of 0 as 8, but we don't really care here!
    /// 
    /// According to Pan Docs:
    ///    Writes to this register while the channel is on require retriggering it afterwards.
    ///    If the write turns the channel off, retriggering is not necessary (it would do nothing).
    pub fn write2(&mut self, value: u8) {
        self.configured_volume = Self::get_volume(value);
        self.configured_increasing = Self::get_direction(value);
        self.configured_period = Self::get_period(value);
    }

    /// 1. Volume envelope timer is reloaded with period
    /// 2. Channel volume is reloaded from NRx2
    pub fn trigger(&mut self) {
        self.timer.period = self.configured_period as u16;  // 1
        self.timer.reload();                                // 1
        self.volume = self.configured_volume;               // 2
        self.increasing = self.configured_increasing;       // Pan Docs suggest so (see above)
    }

    /// A volume envelope has a volume counter and an internal timer clocked at 64 Hz by the frame sequencer.
    /// 
    /// 1. When the timer generates a clock and the envelope period is not zero:
    ///    2. A new volume is calculated by adding or subtracting (as set by NRx2) one from the current volume.
    ///    If this new volume within the 0 to 15 range, the volume is updated, otherwise it is left unchanged
    ///    and no further automatic increments/decrements are made to the volume until the channel is triggered again.
    pub fn step(&mut self) {
        if self.timer.input_clock() && self.timer.period != 0 { // 1
            if self.increasing { if self.volume < 15 { self.volume += 1; } }    // 2
            else { if self.volume > 0 { self.volume -= 1; } }                   // 2
        }
    }
}

/// A length counter disables a channel when it decrements to zero.
/// It contains an internal counter and enabled flag.
/// Writing a byte to NRx1 loads the counter with 64-data (256-data for wave channel).
/// The counter can be reloaded at any time.
#[derive(Default)]
pub struct LengthCounter {
    /// 0: Play indefinitely / 1: Take length into account
    enabled: bool,
    /// Internal counter, not really a timer
    counter: u16,
    /// Maximum sound length
    /// 64 for Ch1, Ch2 and Ch4, 256 for Ch3
    max_length: u16,
    /// Trigger quirk flag
    extra_clock: bool,
}
impl LengthCounter {
    pub fn new(max_length: u16) -> Self {
        Self { max_length, counter: max_length, ..Default::default() }
    }

    pub fn get_length(value: u8) -> u8      {   value & 0b11_1111     }
    pub fn get_enabled(value: u8) -> bool   { ((value >> 6) & 1) != 0 }
    pub fn get_trigger(value: u8) -> bool   { ( value >> 7)      != 0 }

    /// Triggering resets the counter to max value if it has expired
    /// If length counter is zero, it is set to 64 (256 for wave channel)
    pub fn trigger(&mut self, channel_enabled: &mut bool) {
        if self.counter == 0 { self.counter = self.max_length; }
        if self.extra_clock { 
            self.extra_clock = false; if self.step() { *channel_enabled = false; }
        }
    }

    // Writes
    /// (let's ignore wave duty)
    /// NR11, NR21 and NR41:            NR31:  
    /// | 76 | 543210 ------------- |   | 76543210 ----------- |
    /// | -- | Initial length timer |   | Initial length timer |
    pub fn write1(&mut self, value: u8) {
        let length = 
            if self.max_length == 256 { value }
            else { LengthCounter::get_length(value) };

        self.counter = self.max_length - length as u16;
    }
    /// | 7 ----- | 6 ----------- | 543210 |
    /// | Trigger | Length enable | ------ |
    pub fn write4(&mut self, value: u8, sequencer_phase: u8) -> bool {
        let old_enabled = self.enabled;
        self.enabled = LengthCounter::get_enabled(value);

        let mut result = false;
        if sequencer_phase.is_multiple_of(2) {
            // Length is being enabled right now
            if !old_enabled && self.enabled { result = self.step(); }
            // Channel is triggered while length is 0 aka unfreezing
            if Self::get_trigger(value) && self.enabled && self.counter == 0 { self.extra_clock = true; }
        }
        result
    }

    /// Each length counter is clocked at 256 Hz by the frame sequencer.
    /// 
    /// 1. When clocked while enabled by NRx4 and the counter is not zero:
    ///    2. It's decremented
    ///    3. If it becomes zero the channel is disabled
    pub fn step(&mut self) -> bool {
        if self.enabled && self.counter != 0 { // 1
            self.counter = self.counter.saturating_sub(1);  // 2
            if self.counter == 0 { return true; }           // 3
        }
        false
    }
}

#[derive(SmartDefault)]
pub struct PhaseTimer {
    /// Current pattern position
    /// [0,7] for Ch1 and Ch2, [0,31] for Ch3
    pub phase: u8,
    /// Timer counter
    pub timer: AudioTimer,
    /// Base frequency
    pub frequency: u16,
    /// Period multiplier
    /// 4 for Ch1 and Ch2, 2 for Ch3
    #[default = 4]
    pub multiplier: u8,
    /// Phase modulo
    /// 8 for Ch1 and Ch2, 32 for Ch3
    #[default = 8]
    pub modulo: u8,
}
impl PhaseTimer {
    pub fn new(multiplier: u8, modulo: u8) -> Self {
        Self { multiplier, modulo, timer: AudioTimer::new(2048*multiplier as u16), ..Default::default() }
    }
}
impl PhaseTimer {
    pub fn get_frequency(upper: u8, lower: u8) -> u16 {
        (((upper & 0b111) as u16) << 8) | lower as u16
    }

    /// Set lower frequency bits
    /// Doesn't affect the timer
    pub fn write3(&mut self, value: u8) {
        self.frequency = PhaseTimer::get_frequency((self.frequency >> 8) as u8, value);
    }
    /// Set upper frequency bits
    /// Doesn't affect the timer
    pub fn write4(&mut self, value: u8) {
        self.frequency = PhaseTimer::get_frequency(value, self.frequency as u8);
    }

    /// 1. Frequency timer is reloaded with period
    /// 2. Triggering resets wave channel's phase
    pub fn trigger(&mut self) {
        let square_flag = self.multiplier == 2;
        if square_flag { // Square channel
            // Quirk: when triggering a square channel, the low two bits of the frequency timer are NOT modified
            let value = (self.timer.counter & 0b11) | (self.timer.period & 0b1111_1100);
            self.timer.manual_reload(value) // 1
        } else { // Wave channel
            self.timer.reload();            // 1
        }
    }

    pub fn tick(&mut self) -> bool {
        if self.timer.input_clock() {
            self.timer.manual_reload((2048 - self.frequency) * self.multiplier as u16); // Reloads only here
            self.phase = (self.phase + 1) % self.modulo;
            true
        } else { false }
    }
}

#[derive(Default)]
pub struct AudioVolume {
    /// Left channel volume value
    left: u8,
    /// Right channel volume value
    right: u8,
}
impl AudioVolume {
    /// | 7 ------ | 654 ------- | 3 ------- | 210 -------- |
    /// | VIN left | Left volume | VIN right | Right volume |
    pub fn new(value: u8) -> Self {
        Self {
            left: (value >> 4) & 0b111, right: value & 0b111
        }
    }

    /// It's volume + 1 to map [0, 7] to [1, 8] since it
    /// can't actually mute the sound according to Pan Docs
    fn get_volume(value: u8) -> u8 { value + 1 }
    pub fn left(&self) -> u8 { Self::get_volume(self.left) }
    pub fn right(&self) -> u8 { Self::get_volume(self.right) }
}

#[derive(Default)]
pub struct AudioLR {
    /// Left channel enabled
    pub left: bool,
    /// Right channel enabled
    pub right: bool,
}

#[derive(Default)]
pub struct SoundPanning {
    pub ch1: AudioLR,
    pub ch2: AudioLR,
    pub ch3: AudioLR,
    pub ch4: AudioLR,
}
impl SoundPanning {
    /// | 7 --- | 6 --- | 5 --- | 4 --- | 3 --- | 2 --- | 1 --- | 0 --- |
    /// | Ch4 L | Ch3 L | Ch2 L | Ch1 L | Ch4 R | Ch3 R | Ch2 R | Ch1 R |
    pub fn new(value: u8) -> Self {
        Self {
            ch1: AudioLR { left: ((value >> 4) & 1) != 0, right:   value       & 1  != 0 },
            ch2: AudioLR { left: ((value >> 5) & 1) != 0, right: ((value >> 1) & 1) != 0 },
            ch3: AudioLR { left: ((value >> 6) & 1) != 0, right: ((value >> 2) & 1) != 0 },
            ch4: AudioLR { left: ((value >> 7) & 1) != 0, right: ((value >> 3) & 1) != 0 },
        }
    }
}

pub type AudioSamples = [[u8;4];4];
fn empty_samples() -> AudioSamples { [[0u8;4];4] }

#[derive(SmartDefault)]
pub struct Apu {
    // Global audio control registers:
        /// NR50 byte value
        /// Master volume & VIN panning register
        pub nr50: u8,
        /// NR51 byte value
        /// Sound panning register
        pub nr51: u8,
    // Sound channels:
        /// Audio channel 1 - Square 1
        #[default(SquareChannel::new(true))]
        pub ch1: SquareChannel,
        /// Audio channel 2 - Square 2
        pub ch2: SquareChannel,
        /// Audio channel 3 - Wave Channel
        pub ch3: WaveChannel,
        /// Audio channel 4 - Noise Channel
        pub ch4: NoiseChannel,
    // State:
        /// Audio state
        pub audio_enabled: bool,
        /// Master volume
        pub volume: AudioVolume,
        /// Sound panning
        pub panning: SoundPanning,
        /// Phase value [0,7]
        pub sequencer_phase: u8,
        /// So that I don't have to set phase to -1
        #[default(true)]
        pub sequencer_reset_flag: bool,
}
impl Apu {
    pub fn tick4(&mut self) -> AudioSamples {
        let mut result: AudioSamples = empty_samples();
        if self.audio_enabled {
            for item in &mut result {
                self.waveform_tick(); // Tick the channel waveforms
                *item = [ self.ch1.sample(), self.ch2.sample(), self.ch3.sample(), self.ch4.sample() ]; // Sample
            }
        }
        result
    }

    /// Frame sequencer - ticked externally by DIV_APU
    /// Power state does not affect the 512 Hz timer that feeds the frame sequencer
    /// 
    /// It steps like that:
    ///     Step        0   1   2   3   4   5   6   7
    ///                 -----------------------------
    ///     Len Ctr     x   -   x   -   x   -   x   -   => 256 Hz
    ///     Sweep       -   -   x   -   -   -   x   -   => 128 Hz
    ///     Vol Env     -   -   -   -   -   -   -   x   => 64  Hz
    pub fn sequencer_step(&mut self) {
        if self.sequencer_reset_flag { self.sequencer_phase = 0; self.sequencer_reset_flag = false; }
        else { self.sequencer_phase = (self.sequencer_phase+1) % 8; }

        match self.sequencer_phase {
            0 | 4 => { self.length_step()                         },
            2 | 6 => { self.length_step(); self.ch1.sweep_step(); },
                7 => { self.envelope_step()                       },
                _ => ( /* Steps 1, 3 and 5 do nothing */          )   
        }
    }

    // R/W
    /// | 7 ---------- | 654 | 3 ----- | 2 ---- | 1 ----- | 0 ----- |
    /// | Audio on/off | --- | Ch4 on? | H3 on? | Ch2 on? | Ch1 on? |
    pub fn read52(&self) -> u8 {
        (self.audio_enabled       as u8) << 7 |
        (self.ch4.channel_enabled as u8) << 3 |
        (self.ch3.channel_enabled as u8) << 2 |
        (self.ch2.channel_enabled as u8) << 1 |
        (self.ch1.channel_enabled as u8)
    }
    /// | 7 ------ | 654 ------- | 3 ------- | 210 -------- |
    /// | VIN left | Left volume | VIN right | Right volume |
    pub fn write50(&mut self, value: u8) {
        self.nr50 = value;
        self.volume = AudioVolume::new(value);
    }
    /// | 7 --- | 6 --- | 5 --- | 4 --- | 3 --- | 2 --- | 1 --- | 0 --- |
    /// | Ch4 L | Ch3 L | Ch2 L | Ch1 L | Ch4 R | Ch3 R | Ch2 R | Ch1 R |
    pub fn write51(&mut self, value: u8) {
        self.nr51 = value;
        self.panning = SoundPanning::new(value);
    }
    /// When powered off, all registers (NR10-NR51) are instantly
    /// written with zero and any writes to those registers are ignored while power remains off
    /// (except on the DMG, where length counters are unaffected by power and can still be written while off).
    /// 
    /// When powered on, the frame sequencer is reset so that the next step will be 0,
    /// the square duty units are reset to the first step of the waveform, and the wave channel's sample buffer is reset to 0. 
    pub fn write52(&mut self, value: u8) {
        let power_bit = (value >> 7) != 0;
        if self.audio_enabled && !power_bit {
            self.audio_enabled = false; // Power off

            let len1 = self.ch1.length_counter.counter;
            let len2 = self.ch2.length_counter.counter;
            let len3 = self.ch3.length_counter.counter;
            let len4 = self.ch4.length_counter.counter;

            let wave_ram = mem::take(&mut self.ch3.wave_ram);
            let sample_buffer = self.ch3.sample_buffer;

            self.ch1 = SquareChannel::new(true);
            self.ch2 = SquareChannel::default();
            self.ch3 = WaveChannel::default();
            self.ch4 = NoiseChannel::default();

            self.ch1.length_counter.counter = len1;
            self.ch2.length_counter.counter = len2;
            self.ch3.length_counter.counter = len3;
            self.ch4.length_counter.counter = len4;

            self.ch3.wave_ram = wave_ram;
            self.ch3.sample_buffer = sample_buffer;
            
            // APU
            self.write50(0);
            self.write51(0);
        } else if !self.audio_enabled && power_bit {
            // When powered on: 
            //      1. The frame sequencer is reset so that the next step will be 0
            //      2. The square duty units are reset to the first step of the waveform
            //      3. The wave channel's sample buffer is reset to 0

            self.audio_enabled = true;          // Power on
            self.sequencer_reset_flag = true;   // 1
            self.ch1.phase_timer.phase = 0;     // 2
            self.ch2.phase_timer.phase = 0;     // 2
            self.ch3.sample_buffer = 0;         // 3
        }
    }

    // Ticks and steps
    fn waveform_tick(&mut self) {
        self.ch1.waveform_tick();
        self.ch2.waveform_tick();
        self.ch3.waveform_tick();
        self.ch4.waveform_tick();
    }
    fn length_step(&mut self) {
        self.ch1.length_step();
        self.ch2.length_step();
        self.ch3.length_step();
        self.ch4.length_step();
    }
    fn envelope_step(&mut self) {
        self.ch1.envelope_step();
        self.ch2.envelope_step();
        self.ch4.envelope_step();
    }
}

// [0xFF10, 0xFF3F]
impl Mmu {
    /// https://gbdev.io/pandocs/Audio_Registers.html
    /// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Register_Reading
    ///
    /// When an NRxx register is read back, the last written value ORed with the following is returned: 
    ///
    ///     NRxx    NRx0    NRx1    NRx2    NRx3    NRx4
    ///     ---------------------------------------------
    ///     NR1x    0x80    0x3F    0x00    0xFF    0xBF
    ///     NR2x    0xFF    0x3F    0x00    0xFF    0xBF 
    ///     NR3x    0x7F    0xFF    0x9F    0xFF    0xBF 
    ///     NR4x    0xFF    0xFF    0x00    0x00    0xBF 
    ///     NR5x    0x00    0x00    0x70    ----    ----
    ///
    ///     $FF27-$FF2F always read back as $FF
    ///
    /// That is, the channel length counters, frequencies, and unused bits always read back as set to all 1s.
    pub fn read_apu(&self, address: usize) -> u8 {
        match address {
            // Ch1
            0xFF10 => self.apu.ch1.read0() | 0x80,  // NR10, Bit 7 is unused (always 1)
            0xFF11 => self.apu.ch1.nrx1 | 0x3F,     // NR11, Only bits 6 and 7 are readable
            0xFF12 => self.apu.ch1.nrx2,            // NR12, All bits readable
            0xFF13 => 0xFF,                         // NR13, Write-only
            0xFF14 => self.apu.ch1.nrx4 | 0xBF,     // NR14, Only bit 6 is readable
            // Ch2
            0xFF15 => 0xFF,                         // Unused (always 0xFF)
            0xFF16 => self.apu.ch2.nrx1 | 0x3F,     // NR21, Only bits 6 and 7 are readable
            0xFF17 => self.apu.ch2.nrx2,            // NR22, All bits readable
            0xFF18 => 0xFF,                         // NR23, Write-only
            0xFF19 => self.apu.ch2.nrx4 | 0xBF,     // NR24, Only bit 6 is readable
            // Ch3
            0xFF1A => self.apu.ch3.nr30 | 0x7F,     // NR30, Only bit 7 is readable
            0xFF1B => 0xFF,                         // NR31, Write-only
            0xFF1C => self.apu.ch3.nr32 | 0x9F,     // NR32, Only Bits 5 and 6 are readable
            0xFF1D => 0xFF,                         // NR33, Write-only
            0xFF1E => self.apu.ch3.nr34 | 0xBF,     // NR34, Only bit 6 is readable
            // Ch 4
            0xFF1F => 0xFF,                         // Unused (always 0xFF)
            0xFF20 => 0xFF,                         // NR41, Write-only
            0xFF21 => self.apu.ch4.nr42,            // NR42, All bits readable
            0xFF22 => self.apu.ch4.nr43,            // NR43, All bits readable
            0xFF23 => self.apu.ch4.nr44 | 0xBF,     // NR44, Only bit 6 is readable
            // Global audio control
            0xFF24 => self.apu.nr50,                // NR50, All bits readable
            0xFF25 => self.apu.nr51,                // NR51, All bits readable
            0xFF26 => self.apu.read52() | 0x70,     // NR52, Bit 4-6 are unused (always 1)
            // Other
            0xFF27..=0xFF2F => 0xFF,                // Unreadable (always 0xFF)
            0xFF30..=0xFF3F =>
                self.apu.ch3.read(address),         // Ch3 Wave RAM
            _ => 
                unreachable!("address: {:#06X}", address)
        }
    }

    pub fn write_apu(&mut self, address: usize, value: u8) {
        if !self.apu.audio_enabled {
            match address {
                // Length counters
                0xFF11 => self.apu.ch1.write1(value & 0x3F),    // NR11
                0xFF16 => self.apu.ch2.write1(value & 0x3F),    // NR21
                0xFF1B => self.apu.ch3.write1(value),           // NR31
                0xFF20 => self.apu.ch4.write1(value),           // NR41 (Powering off shouldn't affect NR41)
                // Other
                0xFF26 => self.apu.write52(value),              // NR51
                0xFF30..=0xFF3F =>
                    self.apu.ch3.write(address, value),         // Wave RAM
                _ => ()                                         // Ignore all the other writes completely
            }
        } else {
            match address {
                // Ch1
                0xFF10 => self.apu.ch1.write0(value),   // NR10
                0xFF11 => self.apu.ch1.write1(value),   // NR11
                0xFF12 => self.apu.ch1.write2(value),   // NR12
                0xFF13 => self.apu.ch1.write3(value),   // NR13
                0xFF14 => self.apu.ch1.write4(          // NR14
                    value, self.apu.sequencer_phase
                ),
                // Ch2
                0xFF16 => self.apu.ch2.write1(value),   // NR21
                0xFF17 => self.apu.ch2.write2(value),   // NR22
                0xFF18 => self.apu.ch2.write3(value),   // NR23
                0xFF19 => self.apu.ch2.write4(          // NR24
                    value, self.apu.sequencer_phase
                ),
                // Ch3
                0xFF1A => self.apu.ch3.write0(value),   // NR30
                0xFF1B => self.apu.ch3.write1(value),   // NR31
                0xFF1C => self.apu.ch3.write2(value),   // NR32
                0xFF1D => self.apu.ch3.write3(value),   // NR33
                0xFF1E => self.apu.ch3.write4(          // NR34
                    value, self.apu.sequencer_phase
                ),
                // Ch4
                0xFF20 => self.apu.ch4.write1(value),   // NR41
                0xFF21 => self.apu.ch4.write2(value),   // NR42
                0xFF22 => self.apu.ch4.write3(value),   // NR43
                0xFF23 => self.apu.ch4.write4(          // NR44
                    value, self.apu.sequencer_phase
                ),
                // Global audio control
                0xFF24 => self.apu.write50(value),      // NR50
                0xFF25 => self.apu.write51(value),      // NR51
                0xFF26 => self.apu.write52(value),      // NR52
                // Wave RAM
                0xFF30..=0xFF3F => 
                    self.apu.ch3.write(address, value), // Wave RAN
                _ => ()                                 // Ignore all the other writes completely
            }
        }
    }
}