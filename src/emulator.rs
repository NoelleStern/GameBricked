use color_eyre::eyre;
use std::{fs::File, io::Read};

use crate::{cpu::cpu::CPU, rendering::ffmpeg::FFmpeg, memory::header::Header, rendering::palette::Palette};


pub const OG_PALETTE: Palette = Palette::new([0xCADC9F, 0x8BAC0F, 0x306230, 0x0F380F]);
pub const BW_PALETTE: Palette = Palette::new([0xFFFFFF, 0xAAAAAA, 0x555555, 0x000000]);
pub const PALETTES: [Palette; 2] = [OG_PALETTE, BW_PALETTE];


pub struct Emulator {
    pub cpu: CPU,
    pub header: Header,
    palette_id: usize,
    ffmpeg: Option<FFmpeg>,
}
impl Emulator {
    pub fn new(boot_rom_path: &str, rom_path: &str) -> eyre::Result<Self> {
        let mut cpu: CPU = CPU::default();
        cpu.mmu.load_boot_rom(Self::read_file(boot_rom_path)?);
        cpu.mmu.load_rom(Self::read_file(rom_path)?);
        cpu.mmu.init(); // Initialize memory after loading roms

        let header = Header::new(&cpu.mmu.cart.rom)?;

        Ok(Self { cpu, header, palette_id: 0, ffmpeg: None })
    }
  
    fn read_file(path: &str) -> eyre::Result<Vec<u8>> {
        let mut f = File::open(path)?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    pub fn print_header(&self) {
        println!("\r\n{}\r\n", self.header.printable_info());
    }

    // Palette stuff
    pub fn get_palette(&self) -> &'static Palette{
        &PALETTES[self.palette_id]
    }
    pub fn next_palette(&mut self) {
        self.palette_id += 1;
        self.palette_id %= PALETTES.len();
    }

    // FFmpeg stuff
    pub fn toggle_ffmpeg(&mut self) -> eyre::Result<bool> {
        let mut result = false;
        if !self.stop_ffmpeg()? {
            self.ffmpeg = Some(FFmpeg::new(Some(2))?);
            result = true;
        }
        Ok(result)
    }
    pub fn write_ffmpeg(&mut self, buffer: &Vec<u32>) -> eyre::Result<()> {
        if let Some(ffmpeg) = self.ffmpeg.as_mut() {
            ffmpeg.write_buffer(buffer)?
        }
        Ok(())
    }
    pub fn stop_ffmpeg(&mut self) -> eyre::Result<bool> {
        let mut result = false;
        if let Some(ffmpeg) = self.ffmpeg.as_mut() {
            ffmpeg.stop()?;
            self.ffmpeg = None;
            result = true;
        }
        Ok(result)
    }
}