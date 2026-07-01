//! 
//! A sad-sad one function file
//! 


use color_eyre::eyre;
use std::{fs::File, io::Read, path::PathBuf};


pub fn read_file(path: &PathBuf) -> eyre::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}