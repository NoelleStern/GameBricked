//! 
//! FFmpeg video and audio recorder:
//! 
//!     The approach I've decided on is the following:
//!         1) Record video and audio as separate .mp4 and .m4a files using two FFmpeg pipes in parallel
//!         2) Combine them into a singular .mp4 file in a separate thread using another FFmpeg pipe
//! 


use chrono::Local;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};


#[derive(Default)]
pub struct Recorder {
    pub active: Option<ActiveRecorder>,
    pub active_jobs: Vec<JoinHandle<()>>,
}
impl Recorder {
    pub fn toggle(&mut self) -> bool {
        if !self.stop() {
            self.active = Some(ActiveRecorder::new());
            return true;
        }
        false
    }
    pub fn stop(&mut self) -> bool {
        if let Some(a) = self.active.as_mut() {
            let handle = a.stop();
            self.active_jobs.push(handle);
            self.active = None;
            return true;
        }
        false
    }
    pub fn exit(&mut self) {
        self.stop();
        self.join_all();
    }
    // Run occasionally somewhere
    pub fn clear_finished_jobs(&mut self) {
        self.active_jobs.retain(|handle| !handle.is_finished());
    }
    // Call on exit
    pub fn join_all(&mut self) {
        let jobs = std::mem::take(&mut self.active_jobs);
        for handle in jobs {
            let _ = handle.join();
        }
    }
}

pub struct ActiveRecorder {
    name: String,
    ffmpeg_video: FFmpeg,
    ffmpeg_audio: FFmpeg,
}
impl ActiveRecorder {
    pub fn new() -> Self {
        let name = Local::now().format("%Y-%m-%d_%H-%M-%S_%3f").to_string();
        Self {
            ffmpeg_video: FFmpeg::new(FFmpegMode::Video, &Self::get_video_name(&name)),
            ffmpeg_audio: FFmpeg::new(FFmpegMode::Audio, &Self::get_audio_name(&name)),
            name,
        }
    }
    fn get_video_name(name: &str) -> String { format!("{}.mp4", name) }
    fn get_audio_name(name: &str) -> String { format!("{}.m4a", name) }
    pub fn write_video_buffer(&mut self, buffer: &[u8]) {
        self.ffmpeg_video.write_buffer(buffer);
    }
    pub fn write_audio_samples(&mut self, combined_samples: &[i16]) {
        let byte_slice: &[u8] = bytemuck::cast_slice(combined_samples);
        self.ffmpeg_audio.write_buffer(byte_slice);
    }
    fn stop(&mut self) -> JoinHandle<()> {
        self.ffmpeg_video.stop();
        self.ffmpeg_audio.stop();

        let mut final_mix = Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel", "0", // Silent
                "-i", &Self::get_video_name(&self.name), // Input video
                "-i", &Self::get_audio_name(&self.name), // Input audio
                // Encode and map
                "-c:v", "copy",
                "-c:a", "copy",
                "-map", "0:v:0",
                "-map", "1:a:0",
                // Output
                format!("{}.combined.mp4", self.name).as_str()
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn().expect("Failed to execute final stitching merge pass.");

        let name = self.name.clone();
        thread::spawn(move || {
            match final_mix.wait() {
                Ok(status) if status.success() => {
                    let _ = std::fs::remove_file(format!("{}.mp4", name));
                    let _ = std::fs::remove_file(format!("{}.m4a", name));
                },
                _ => ()
            }
        })
    }
}


#[derive(PartialEq)]
pub enum FFmpegMode { Video, Audio }
pub struct FFmpeg {
    pub process: std::process::Child,
    pub stdin: Option<std::process::ChildStdin>,
}
impl FFmpeg {
    pub fn new(mode: FFmpegMode, name: &str) -> Self {
        let mut process =
            if mode == FFmpegMode::Video { Self::new_video_process(name) }
            else { Self::new_audio_process(name) };

        Self { stdin: process.stdin.take(), process }
    }
    
    // New FFmpeg video process
    fn new_video_process(name: &str) -> std::process::Child {
        Command::new("ffmpeg")
            .args([
                "-y",
                // Silent
                "-loglevel", "0",
                // Raw video input
                "-f", "rawvideo",
                "-pix_fmt", "rgb24",
                "-video_size", "160x144",
                "-framerate", "60",
                "-i", "-",
                // Upscale
                "-vf", format!(
                    "scale=iw*{}:ih*{}:flags=neighbor", 2, 2
                ).as_str(),
                // Encode
                "-c:v", "libx264",
                "-preset", "fast",
                "-crf", "18",
                // Compatibility
                "-pix_fmt", "yuv420p",
                // Output
                name
            ])
            .stdin(Stdio::piped()).stdout(Stdio::null()).spawn()
            .expect("Failed to launch FFmpeg video recording.")
    }

    // New FFmpeg audio process
    fn new_audio_process(name: &str) -> std::process::Child {
        Command::new("ffmpeg")
            .args([
                "-y",
                // Silent
                "-loglevel", "0",
                // Raw audio input
                "-f", "s16le",  // Input format
                "-ar", "192k",  // Sample rate
                "-ac", "2",     // Amount of channels
                "-i", "-",
                // Encode
                "-c:a", "aac",
                // Output
                name
            ])
            .stdin(Stdio::piped()).stdout(Stdio::null()).spawn()
            .expect("Failed to launch FFmpeg audio recording.")
    }

    // Write framebuffer to FFmpeg
    pub fn write_buffer(&mut self, buffer: &[u8]) {
        if self.stdin.is_some() 
            && let Some(stdin) = self.stdin.as_mut() {
                let _ = stdin.write_all(buffer); // Write buffer
            }
    }

    // Stop FFmpeg gracefully
    pub fn stop(&mut self) {
        drop(self.stdin.take()); // Close the pipe
        let _ = self.process.wait(); // Wait for FFmpeg to finish
    }
}