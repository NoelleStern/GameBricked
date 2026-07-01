//! 
//! Command line argument parser
//! 


use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};


/// CLI parser
#[derive(Parser, Clone, Debug)]
#[command(version, about, long_about = None, name = "game-bricked")]
pub struct Cli {
    // /// Log level (off/error/warn/info/debug)
    // #[arg(short = 'l', long, default_value = "off")]
    // pub log_level: LevelFilter,
    
    // /// Log filename
    // #[arg(short = 'f', long, default_value = "game-bricked.log")]
    // pub log_file: String,

    /// Application mode
    #[command(subcommand)]
    pub mode: Option<Command>,
}

/// Subcommands
#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Core emulator
    Main(MainArgs),
    /// Additional utils
    Utils(UtilArgs),
}

#[derive(Args, Default, Clone, Debug)]
pub struct MainArgs {
    /// Path to the ROM file
    #[arg(short='r', long)]
    pub rom: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
pub struct UtilArgs {
    /// Util
    #[command(subcommand)]
    pub util: UtilCommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum UtilCommand {
   Converter(ConverterArgs)
}

#[derive(Args, Clone, Debug)]
pub struct ConverterArgs {
    /// Path(s) to the file(s) to convert
    #[arg(short='f', long, num_args = 1.., value_terminator(";"))]
    pub files: Vec<PathBuf>,
}
