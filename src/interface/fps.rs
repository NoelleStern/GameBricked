//! 
//! Emulator FPS counter:
//! 
//!     Simply counts rendered frames and updates the value every second.
//! 


use instant::Instant;
use smart_default::SmartDefault;


#[derive(SmartDefault)]
pub struct FPSCounter {
    /// FPS value
    pub fps: f32,
    /// Frame counter
    counter: u32,
    #[default(Instant::now())]
    /// A second-long timer
    timer: Instant,
}
impl FPSCounter {
    pub fn increment(&mut self) { self.counter += 1; }
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn update(&mut self) {
        let elapsed = self.timer.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            self.fps = self.counter as f32 / elapsed;
            self.counter = 0;
            self.timer = Instant::now();
        }
    }
}