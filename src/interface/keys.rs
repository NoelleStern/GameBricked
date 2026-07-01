//! 
//! Storage keys used for saving and loading persistent values:
//! 
//!     I convert them from enum to string using fmt::Display.
//! 


/// Storage keys
#[derive(Debug)]
pub enum StorageKey {
    Scale,
    Muted,
    Volume,
    ThemeId,
    BootRom,
    SkipBoot,
    ShaderId,
    PaletteId,
    #[cfg(not(target_arch = "wasm32"))]
    RecentRoms,
}
impl std::fmt::Display for StorageKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}