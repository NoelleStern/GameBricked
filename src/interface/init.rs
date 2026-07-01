//! 
//! Eframe init functions
//! 


use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

use crate::emu::rendering::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};


pub fn load_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto_sans".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "noto_sans_mono".to_owned(),
        FontData::from_static(include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf")).into(),
    );
    fonts.families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "noto_sans".to_owned());
    fonts.families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "noto_sans_mono".to_owned());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(24.0, egui::FontFamily::Proportional),
    );
    ctx.set_global_style(style);
}

pub fn create_default_texture(ctx: &egui::Context) -> egui::TextureHandle {
    ctx.load_texture(
        "dot_matrix",
        egui::ColorImage::new(
            [SCREEN_WIDTH, SCREEN_HEIGHT],
            vec![egui::Color32::BLACK; SCREEN_WIDTH * SCREEN_HEIGHT]
        ),
        egui::TextureOptions::NEAREST,
    )
}