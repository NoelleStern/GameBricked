pub mod audio;
pub mod sound;
pub mod memory;
pub mod emulator;
pub mod controls;
pub mod processor;
pub mod rendering;
pub mod motherboard;

#[cfg(not(target_arch = "wasm32"))]
pub mod ffmpeg;