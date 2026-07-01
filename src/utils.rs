//!
//! Additional utilities
//! 


use std::path::PathBuf;

use color_eyre::eyre;
use image::ImageReader;
use imagequant::{self, RGBA};
use crate::emu::rendering::palette::RawPalette;


/// Converts images to arrays of palette indexes in range [0;3]
pub struct Converter;
impl Converter {
    pub fn convert(path: &PathBuf) -> eyre::Result<Vec<u8>> {
        // Read image
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = ImageReader::open(path)?
            .decode()?.to_rgba8();

        // Convert pixels to RGBA vector
        let pixels: Vec<RGBA> = img.pixels().map(|p|
            RGBA::new(p[0], p[1], p[2], p[3])
        ).collect();

        // Convert image
        let palette: RawPalette = Self::get_quantized_palette(pixels, img.width() as usize, img.height() as usize)?;
        let result: Vec<u8> = img.pixels().map(|p|
            palette.reverse_match_pixel([p[0], p[1], p[2]]) as u8
        ).collect();

        Ok(result)
    }

    fn get_quantized_palette(pixels: Vec<RGBA>, width: usize, height: usize) -> eyre::Result<RawPalette> {
        // Quantize
        let mut attr = imagequant::new(); attr.set_max_colors(4)?;
        let mut image = attr.new_image(pixels, width, height, 0.0)?;

        // Sort from lightest to darkest
        let mut result = attr.quantize(&mut image)?;
        let mut palette = result.palette_vec();
        palette.sort_by(|a, b| {
            Self::luminance(*b).partial_cmp(&Self::luminance(*a)).unwrap()
        });

        // Convert to Palette
        let result: Vec<[u8; 3]> = palette.iter().take(4).map(|c| [c.r, c.g, c.b]).collect();
        let r: [[u8; 3]; 4] = result.try_into().unwrap();
        Ok(RawPalette::new(r))
    }

    fn luminance(c: RGBA) -> f32 {
        0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
    }
}
