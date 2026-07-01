//! 
//! GameBoy's Direct Memory Access
//! 


use crate::emu::memory::mmu::{Mmu, OAM_BEGIN};


/// DMA in-memory address
pub const DMA_ADDRESS: usize = 0xFF46;


#[derive(Default)]
pub struct Dma {
    pub active: bool,
    pub delay: bool,
    pub source_base: u16,
    pub current_offset: u8,
}
impl Dma {
    fn init(&mut self, value: u8) {
        self.active = true;
        self.delay = true;
        self.source_base = (value as u16) << 8;
        self.current_offset = 0;
    }
}

// 0xFF46
impl Mmu {
    pub fn write_dma(&mut self, value: u8) { self.dma.init(value); }
    pub fn dma_tick4(&mut self) {
        if !self.dma.active { return; }
        if self.dma.delay { self.dma.delay = false; return; }

        let address = self.dma.source_base as usize + self.dma.current_offset as usize;
        let value = self.raw_read8(address);
        self.raw_write8(OAM_BEGIN + self.dma.current_offset as usize, value);

        self.dma.current_offset += 1;
        if self.dma.current_offset >= 160 { self.dma.active = false; }
    }
}