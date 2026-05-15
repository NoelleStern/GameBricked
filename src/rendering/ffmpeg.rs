use chrono::Local;
use std::io::Write;
use color_eyre::eyre;
use std::process::{Command, Stdio};


pub struct FFmpeg {
    pub process: std::process::Child,
    pub stdin: Option<std::process::ChildStdin>,
    buffer: Vec<u8>,
}
impl FFmpeg {
    pub fn new(scale: Option<usize>) -> eyre::Result<Self> {
        let mut process = Self::new_process(scale.unwrap_or(1))?;
        Ok(Self { stdin: process.stdin.take(), process, buffer: vec![] })
    }
    
    // Initialize new FFmpeg process
    fn new_process(scale: usize) -> eyre::Result<std::process::Child> {
        Ok(
            Command::new("ffmpeg")
                .args([
                    "-y",
                    // Silent
                    "-loglevel", "0",
                    // Raw input
                    "-f", "rawvideo",
                    "-pix_fmt", "rgb24",
                    "-video_size", "160x144",
                    "-framerate", "60",
                    "-i", "-",
                    // Upscale
                    "-vf", format!(
                        "scale=iw*{}:ih*{}:flags=neighbor", scale, scale
                    ).as_str(),
                    // Encode
                    "-c:v", "libx264",
                    "-preset", "fast",
                    "-crf", "18",
                    // Compatibility
                    "-pix_fmt", "yuv420p",
                    // Output
                    format!(
                        "{}.mp4", Local::now().format("%Y%m%d_%H%M%S_%3f").to_string() // An unholy string
                    ).as_str(),
                ])
                .stdin(Stdio::piped()).spawn()?
        )
    }

    // Convert framebuffer to FFmpeg-compatible format
    fn buffer_to_rgb(&mut self, buffer: &[u32]) {
        // To not allocate it every frame
        let size = buffer.len() * 3;
        if self.buffer.len() != size {
            self.buffer = Vec::with_capacity(size);
            self.buffer.resize(size, 0);
        }

        buffer.iter().enumerate().for_each(|(i, p)| {
            let index = i*3;
            self.buffer[index] =    ((p >> 16) & 0xFF) as u8;   // R
            self.buffer[index+1] =  ((p >> 8) & 0xFF) as u8;    // G
            self.buffer[index+2] =  (p & 0xFF) as u8;           // B
        });
    }

    // Write framebuffer to FFmpeg
    pub fn write_buffer(&mut self, buffer: &[u32]) -> eyre::Result<()> {
        if self.stdin.is_some() {
            self.buffer_to_rgb(buffer); // Update the buffer
            if let Some(stdin) = self.stdin.as_mut() {    
                stdin.write_all(&mut self.buffer)?; // Write buffer
            }
        }
        Ok(())
    }

    // Stop FFmpeg gracefully
    pub fn stop(&mut self) -> eyre::Result<()> {
        drop(self.stdin.take()); // Close the pipe
        self.process.wait()?; // Wait for FFmpeg to finish
        self.buffer = vec![]; // Clear the buffer
        Ok(())
    }
}