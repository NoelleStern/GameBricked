use minifb::Key;
use std::fs::File;
use color_eyre::eyre;
use simplelog::{CombinedLogger, Config, WriteLogger};

use crate::{emulator::Emulator, rendering::renderer::{EmulatorWindow, MainWindow, TileWindow}};


mod cpu;
mod memory;
mod emulator;
mod rendering;


fn main() -> eyre::Result<()> {
    // Init runtime stuff
    color_eyre::install()?;
    init_logger()?;

    // Init the emulator
    let mut emu = Emulator::new(
        "roms/dmg_boot.bin",
        "roms/Tetris.gb"
    )?;

    // Print header info
    emu.print_header();

    // Init windows
    let tile_window = TileWindow::new(&emu)?;
    let main_window = MainWindow::new(&emu)?;
    let mut windows: Vec<Box<dyn EmulatorWindow>> = vec![
        Box::new(main_window),
        Box::new(tile_window)
    ];

    // Process windows
    while windows[0].get_window().is_open() && !windows[0].get_window().is_key_down(Key::Escape) {
        for w in windows.iter_mut() {
            if w.get_window().is_open() {
                w.step(&mut emu)?;
            }
        };
    }

    // Stop FFmpeg if it's running
    emu.stop_ffmpeg()?;

    Ok(())
}

fn init_logger() -> eyre::Result<()> {
    CombinedLogger::init(vec![WriteLogger::new(
        simplelog::LevelFilter::Info,
        Config::default(),
        File::create("log.txt")?,
    )])?;
    Ok(())
}
