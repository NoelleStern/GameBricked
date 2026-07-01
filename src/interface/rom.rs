//! 
//! ROM stuff
//! 


use std::path::PathBuf;
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::interface::keys::StorageKey;


// https://gbdev.gg8.se/files/roms/bootroms/?hotlink=readme.txt
    /// Most common DMG boot ROM SHA256
    const DMG_BOOT_SHA256: &str = "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7";
    /// Early DMG boot ROM SHA256
    const DMG0_BOOT_SHA256: &str = "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e";
    /// Verified DMG boot ROM SHA256 checksum array
    const ALLOWED_BOOT_SUMS: [&str; 2] = [DMG_BOOT_SHA256, DMG0_BOOT_SHA256];


#[derive(Debug, Clone)]
pub struct Rom {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}
impl Rom {
    pub fn new(path: PathBuf, bytes: Vec<u8>) -> Self {
        Self { path, bytes }
    }
}

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct BootRom { pub bytes: Vec<u8> }
impl BootRom {
    pub fn new(bytes: Vec<u8>) -> Self { Self { bytes } }
    pub fn checksum_check(rom_bytes: &[u8]) -> bool {
        let hash_string = hex::encode(Sha256::digest(rom_bytes));
        ALLOWED_BOOT_SUMS.contains(&hash_string.as_str())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
#[derive(Serialize, Deserialize)]
pub struct RecentRoms { pub list: Vec<PathBuf> }
#[cfg(not(target_arch = "wasm32"))]
impl RecentRoms {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.storage
            .and_then(|storage| eframe::get_value(
                storage, &StorageKey::RecentRoms.to_string()
            )).unwrap_or_default()
    }
    pub fn add(&mut self, path: PathBuf) {
        self.list.retain(|p| p != &path);
        self.list.insert(0, path);
        self.list.truncate(5); // Cap at 5
    }
    pub fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, &StorageKey::RecentRoms.to_string(), &self);
    }
}