use crate::rendering::ppu::TilePixel;


pub struct Palette {
    white:      u32,
    light_gray: u32,
    dark_gray:  u32,
    black:      u32,
}
impl Palette {
    pub const fn new(colors: [u32; 4]) -> Self {
        Self {
            white:      colors[0],
            light_gray: colors[1],
            dark_gray:  colors[2],
            black:      colors[3],
        }
    }
    
    pub fn match_pixel(&self, value: TilePixel) -> u32 {
        match value {
            TilePixel::Zero => self.white,
            TilePixel::One =>  self.light_gray,
            TilePixel::Two =>  self.dark_gray,
            _ =>               self.black,
        }
    }

    pub fn match_pixel_utf8(value: TilePixel) -> &'static str {
        match value {
            TilePixel::Zero => "░",
            TilePixel::One  => "▒",
            TilePixel::Two  => "▓",
            _ =>               "█",
        }
    }
}