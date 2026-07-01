//! 
//! Audio output:
//! 
//!     Acts as a mixer amplifier and an output.
//!     Treats [0, 15] range as a [0, 30] range so that when channel amplifiers is off, the middle is a neat whole number 15 and not 7,5.
//!     So all of the channels together get up to 120 + amplifiers can multiply them by [1, 8], getting the value to 960 max.
//! 


use blip_buf::BlipBuf;
use smart_default::SmartDefault;
use rtrb::{Producer, RingBuffer};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use tinyaudio::{self, OutputDevice, OutputDeviceParameters};

use crate::{emu::{memory::mmu::Mmu, rendering::ppu::FRAME_CYCLES, sound::apu::{Apu, AudioSamples, SoundPanning}}, interface::keys::StorageKey};


// "Magic" numbers:
    /// Delay before the audio retries to initialize on web
    #[cfg(target_arch = "wasm32")]
    const WEB_UNHEALTHY_DELAY: f32 = 5.0;
    /// Delay before the audio output is declared unhealthy
    const UNHEALTHY_DELAY: f32 = 1.0;
    /// Min volume value
    pub const MIN_VOLUME: f32 = 0.001;
    /// Delay in frames to eliminate the startup pop
    const DEFAULT_POP_DELAY: usize = 10; // 5 still pops, so I guess 10 it is
// Real math:
    /// CPU clock value for blip
    const CPU_CLOCK: f64 = 4_194_304.0;
    /// Output sample rate
    const SAMPLE_RATE: f64 = 192_000.0;
    /// How many samples do we produce per frame
    const SAMPLE_AMOUNT: f64 = FRAME_CYCLES as f64 / (CPU_CLOCK / SAMPLE_RATE);
    /// How many samples it waits for before outputting sound
    const CHANNEL_SAMPLE_COUNT: usize = (SAMPLE_AMOUNT * 3.0) as usize; // 3 frame sound output delay aka ~50ms


pub struct EnabledChannels { ch1: bool, ch2: bool, ch3: bool, ch4: bool }
impl Default for EnabledChannels {
    fn default() -> Self { Self { ch1: true, ch2: true, ch3: true, ch4: true } }
}
impl EnabledChannels {
    pub fn dac(apu: &Apu) -> Self {
        Self {
            ch1: apu.ch1.dac_enabled, ch2: apu.ch2.dac_enabled,
            ch3: apu.ch3.dac_enabled, ch4: apu.ch4.dac_enabled
        }
    }
    pub fn left(sp: &SoundPanning) -> Self {
        Self { ch1: sp.ch1.left, ch2: sp.ch2.left, ch3: sp.ch3.left, ch4: sp.ch4.left }
    }
    pub fn right(sp: &SoundPanning) -> Self {
        Self { ch1: sp.ch1.right, ch2: sp.ch2.right, ch3: sp.ch3.right, ch4: sp.ch4.right }
    }

    /// Logical AND
    pub fn and(&self, b: &Self) -> Self {
        Self { 
            ch1: self.ch1 & b.ch1, ch2: self.ch2 & b.ch2,
            ch3: self.ch3 & b.ch3, ch4: self.ch4 & b.ch4
        }
    }

    /// Mix channels together
    /// It's done by basically adding all of the channels together and multiplying them by volume
    pub fn mix(&self, ch: &[u8; 4], volume: u8) -> i32 {
        // 15 is our baseline middle no sound
        // [0, 15] * 2 * 4 gets us [0, 120], which is nicely under u8 limit, so we can cast it all only in the end
        (
            if self.ch1 { ch[0] * 2 } else { 15 } +
            if self.ch2 { ch[1] * 2 } else { 15 } +
            if self.ch3 { ch[2] * 2 } else { 15 } +
            if self.ch4 { ch[3] * 2 } else { 15 }
        ) as i32 * volume as i32
    }
}
impl<'a> IntoIterator for &'a mut EnabledChannels {
    type Item = &'a mut bool;
    type IntoIter = std::array::IntoIter<Self::Item, 4>;
    fn into_iter(self) -> Self::IntoIter {
        [
            &mut self.ch1, &mut self.ch2,
            &mut self.ch3, &mut self.ch4,
        ].into_iter()
    }
}

/// Heartbeat is used to monitor audio output state
/// Basically, if it isn't being updated in a while, the audio output might be dead
#[derive(Default)]
struct Heartbeat {
    audio_heartbeat: Arc<AtomicUsize>,
    last_seen_heartbeat: usize,
    dead_time: f32,
}
impl Heartbeat {
    /// Changes output device on Desktop and acts as safety net on Web
    pub fn update(&mut self, audio_device: &Option<OutputDevice>, _initialized: bool, dt: f32) -> bool {
        let mut result = false;

        #[cfg(not(target_arch = "wasm32"))]
        let should_update = audio_device.is_some();
        #[cfg(target_arch = "wasm32")]
        let should_update = audio_device.is_some() || !_initialized;
        
        if should_update {
            let current_heartbeat = self.audio_heartbeat.load(Ordering::Relaxed);
            
            if current_heartbeat == self.last_seen_heartbeat { // Potentially unhealthy
                self.dead_time += dt;

                #[cfg(not(target_arch = "wasm32"))]
                let delay = UNHEALTHY_DELAY;
                #[cfg(target_arch = "wasm32")]
                let delay = if !_initialized { WEB_UNHEALTHY_DELAY } else { UNHEALTHY_DELAY };
                
                // Wait for a few seconds to accumulate to declare it unhealthy
                if self.dead_time > delay {
                    self.dead_time = 0.0;
                    result = true;
                }
            } else { // Healthy
                self.last_seen_heartbeat = current_heartbeat;
                self.dead_time = 0.0;
            }
        }

        result
    }
}

struct BlipHandler {
    /// Cool blip audio buffer
    blip: BlipBuf,
    /// Last mixed value
    last_mix: i32,
    /// End frame audio buffer
    pub sample_recorder: Vec<i16>,
}
impl BlipHandler {
    pub fn new() -> Self {
        let mut blip = BlipBuf::new((SAMPLE_AMOUNT * 1.2) as u32); // * 1.2 to have some extra room
        blip.set_rates(CPU_CLOCK, SAMPLE_RATE);
        Self { blip, last_mix: 0, sample_recorder: vec![] }
    }

    pub fn reset(&mut self) {
        self.blip.clear();
        self.last_mix = 0;
    }

    /// 4 channels * 15 max amp * 8 max vol = 960
    /// [0, 960] scales nicely to [0, 32_640] with * 34
    pub fn append(&mut self, mix: i32, clock_time: u32) {
        if mix != self.last_mix {
            let delta = (mix - self.last_mix) * 34;
            self.blip.add_delta(clock_time, delta);
            self.last_mix = mix;
        }
    }

    pub fn on_end_frame(&mut self, t_cycles: u32) -> usize {
        self.blip.end_frame(t_cycles);
        let samples_count = self.blip.samples_avail() as usize;
        
        if samples_count > 0 {
            if self.sample_recorder.len() != samples_count { self.sample_recorder.resize(samples_count, 0); } // Resize
            self.blip.read_samples(&mut self.sample_recorder, false);
        }

        samples_count
    }
}

struct SampleRateController {
    actual_rate: f64,
    gain: f64,
    interpolation_value: f64,
}
impl Default for SampleRateController {
    fn default() -> Self {
        Self { actual_rate: SAMPLE_RATE, gain: 0.00005, interpolation_value: 0.01 }
    }
}
impl SampleRateController {
    pub fn update(&mut self, producer: &Producer<f32>, blip_l: &mut BlipBuf, blip_r: &mut BlipBuf) {
        let buffered_samples = (CHANNEL_SAMPLE_COUNT as i16 * 2) - producer.slots() as i16; // * 2 since two channels

        let queue_ms = (buffered_samples as f64 / SAMPLE_RATE) * 1000.0;
        let error_ms = queue_ms - 50.0;

        let target_rate  = SAMPLE_RATE * (1.0 + error_ms * self.gain);
        self.actual_rate += (target_rate - self.actual_rate) * self.interpolation_value; // Simple interpolation

        blip_l.set_rates(CPU_CLOCK, self.actual_rate);
        blip_r.set_rates(CPU_CLOCK, self.actual_rate);
    }
}

/// It works by simply recording the last mixed left and right samples after
/// a certain amount of frames and then setting them as a sound output baseline
#[derive(SmartDefault)]
struct PopHandler {
    /// Delay in frames to eliminate the startup pop
    #[default(DEFAULT_POP_DELAY)]
    pop_delay: usize,
    /// Last recorder left mix
    pub last_mix_l: i32,
   /// Last recorder right mix
    pub last_mix_r: i32,
}
impl PopHandler {
    fn on_end_frame(&mut self) { self.pop_delay = self.pop_delay.saturating_sub(1); }
    fn reset(&mut self) { self.pop_delay = DEFAULT_POP_DELAY; }
    fn ready(&self) -> bool { self.pop_delay == 0 }
}

pub struct Audio {
    // State
        /// Volume [0.001, 1.0]
        pub volume: f32,
        /// Muted flag
        pub muted: bool,
    // Intermediary output
        /// Left blip handler
        blip_l: BlipHandler,
        /// Right blip handler
        blip_r: BlipHandler,
        /// Synchronizes frames with audio
        rate_controller: SampleRateController,
        /// Eliminates a small pop on startup
        pop_handler: PopHandler,
        /// T-cycle counter
        t_cycles: u32,
    // Tinyaudio output
        /// Audio output heartbeat
        heartbeat: Heartbeat,
        /// Audio ring array producer
        producer: Producer<f32>,
        /// Current audio output device
        audio_device: Option<OutputDevice>,
    // FFmpeg output buffer
        #[cfg(not(target_arch = "wasm32"))]
        pub combined_samples: Vec<i16>,
    // Web-specific flag
        /// Was the audio output initialized?
        #[cfg(target_arch = "wasm32")]
        web_initialized: bool,
}
impl Audio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load volume from storage
        let volume = cc.storage
            .and_then(|storage| eframe::get_value(storage, &StorageKey::Volume.to_string()))
            .unwrap_or(0.3);

        // Load muted flag from storage
        let muted = cc.storage
            .and_then(|storage| eframe::get_value(storage, &StorageKey::Muted.to_string()))
            .unwrap_or(false);

        let heartbeat = Heartbeat::default();

        #[cfg(not(target_arch = "wasm32"))]
        let (producer, audio_device) = Self::init_audio(heartbeat.audio_heartbeat.clone());

        #[cfg(target_arch = "wasm32")]
        let (producer, _) = RingBuffer::<f32>::new(CHANNEL_SAMPLE_COUNT*2); // * 2 since two channels
        #[cfg(target_arch = "wasm32")]
        let audio_device = None;

        Self {
            volume, muted,
            blip_l: BlipHandler::new(),
            blip_r: BlipHandler::new(),
            pop_handler: PopHandler::default(),
            rate_controller: SampleRateController::default(),
            t_cycles: 0,
            heartbeat, producer, audio_device,
            #[cfg(not(target_arch = "wasm32"))]
            combined_samples: vec![],
            #[cfg(target_arch = "wasm32")]
            web_initialized: false
        }
    }

    pub fn reset(&mut self) {
        self.blip_l.reset();
        self.blip_r.reset();
        self.pop_handler.reset();
        self.t_cycles = 0;
    }

    fn init_audio(audio_heartbeat: Arc<AtomicUsize>) -> (Producer<f32>, Option<OutputDevice>) {
        let (producer, mut consumer) = RingBuffer::<f32>::new(CHANNEL_SAMPLE_COUNT*2); // * 2 since two channels

        let params = OutputDeviceParameters {
            channels_count: 2,
            sample_rate: SAMPLE_RATE as usize,
            channel_sample_count: CHANNEL_SAMPLE_COUNT,
        };

        let device = tinyaudio::run_output_device(params, move |data| {
            audio_heartbeat.fetch_add(1, Ordering::Relaxed);
            for frame in data.chunks_mut(params.channels_count) {
                frame[0] = consumer.pop().unwrap_or(0.0);
                frame[1] = consumer.pop().unwrap_or(0.0);
            }
        }).expect("Failed to open audio device");

        (producer, Some(device))
    }

    pub fn update(&mut self, dt: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        let should_restart = self.heartbeat.update(&self.audio_device, true, dt);
        #[cfg(target_arch = "wasm32")]
        let should_restart = self.heartbeat.update(&self.audio_device, self.web_initialized, dt);

        if should_restart { self.restart_audio(); }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn web_check_audio(&mut self, ctx: &eframe::egui::Context) {
        if !self.web_initialized && ctx.input(|i| i.pointer.any_click()) {
            self.web_initialized = true;
            self.restart_audio();
        }
    }

    pub fn restart_audio(&mut self) {
        if let Some(ad) = self.audio_device.as_mut() { ad.close(); }
        self.audio_device = None;

        let (producer, audio_device) = 
            Self::init_audio(self.heartbeat.audio_heartbeat.clone());

        self.producer = producer;
        self.audio_device = audio_device;
    }

    fn mix(mmu: &Mmu, channels: &EnabledChannels, samples: &AudioSamples, volume: u8) -> i32 {
        if !mmu.apu.audio_enabled { 0 } else { channels.mix(&samples[3], volume) }
    }

    pub fn mix_append(&mut self, mmu: &Mmu, audio_channels: &EnabledChannels, samples: AudioSamples) {
        self.t_cycles += 4;  // Update it here and not in the end just because it might conflict with pop handler

        let dac = EnabledChannels::dac(&mmu.apu);
        let channels_l = audio_channels.and(&dac).and(&EnabledChannels::left(&mmu.apu.panning));
        let channels_r = audio_channels.and(&dac).and(&EnabledChannels::right(&mmu.apu.panning));

        let mix_l = Self::mix(mmu, &channels_l, &samples, mmu.apu.volume.left());
        let mix_r = Self::mix(mmu, &channels_r, &samples, mmu.apu.volume.right());

        if self.pop_handler.ready() {
            self.blip_l.append(mix_l, self.t_cycles);
            self.blip_r.append(mix_r, self.t_cycles);
        } else {
            self.pop_handler.last_mix_l = mix_l;
            self.pop_handler.last_mix_r = mix_r;
        }
    }
    pub fn on_end_frame(&mut self) {
        // Pretty straightforward
        if !self.pop_handler.ready() {
            self.pop_handler.on_end_frame();

            if self.pop_handler.ready() {
                self.blip_l.last_mix = self.pop_handler.last_mix_l;
                self.blip_r.last_mix = self.pop_handler.last_mix_r;
                self.blip_l.blip.add_delta(0, self.pop_handler.last_mix_l);
                self.blip_r.blip.add_delta(0, self.pop_handler.last_mix_r);
                self.t_cycles = 0; // Don't forget to reset the counter
            }

            return;
        }

        let count = self.blip_l.on_end_frame(self.t_cycles);
        self.blip_r.on_end_frame(self.t_cycles);    
        
        // TODO: probably real bad for FFMpeg overtime
        // Update the rate dynamically to seamlessly sync audio and video
        self.rate_controller.update(&self.producer, &mut self.blip_l.blip, &mut self.blip_r.blip);

        let volume =  if self.muted || self.volume == MIN_VOLUME { 0.0 } else { self.volume };

        // Resize
        #[cfg(not(target_arch = "wasm32"))]
        if self.combined_samples.len() != count * 2 {
            self.combined_samples.resize(count * 2, 0);
        }
        
        // Audio output loop
        for i in 0..count {
            let l = ((self.blip_l.sample_recorder[i] as f32) / 32768.0) * volume;
            let r = ((self.blip_r.sample_recorder[i] as f32) / 32768.0) * volume;

            #[cfg(not(target_arch = "wasm32"))]
            {
                self.combined_samples[i*2] = self.blip_l.sample_recorder[i];
                self.combined_samples[i*2 + 1] = self.blip_r.sample_recorder[i];
            }

            if self.audio_device.is_some() { 
                let _ = self.producer.push(l);
                let _ = self.producer.push(r);
            }
        }

        self.t_cycles = 0; // Don't forget to reset the counter
    }
}