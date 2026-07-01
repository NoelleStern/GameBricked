//! 
//! Tile view window:
//! 
//!     Nice and clean tile view window.
//!     I draw a grid on a separate texture to save on computing layouts.
//! 


use eframe::egui::{self, TextureHandle};

use crate::{emu::{memory::mmu::Mmu, rendering::{palette::RawPalette, ppu::TILE_CAPACITY}}, interface::{renderer::ShadeBuffer, views::view::ViewParams}};


/// Scale factor
const SCALE: usize = 2;
/// Tile amount per row
const TILE_VIEW_WIDTH: usize = 32;
/// Rile amount per column
const TILE_VIEW_HEIGHT: usize = 12;


pub struct TileView {
    pub view_params: ViewParams,
    pub params: TileViewParams,
}
pub struct TileViewParams {
    pub tile_buffer: Vec<u8>,
    pub texture_buffer: TextureHandle,
    pub grid_texture: TextureHandle,
    pub selected_tile: usize,
}
impl TileView {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let view_params = ViewParams::new("Tile View".to_string(), "".to_string());

        let capacity = (8*8*TILE_CAPACITY) * 3; // * 3 for R, G and B
        let tile_buffer = vec![0; capacity];

        let texture_buffer = cc.egui_ctx.load_texture(
            "tile map",
            egui::ColorImage::new(
                [TILE_VIEW_WIDTH*8, TILE_VIEW_HEIGHT*8],
                vec![egui::Color32::BLACK; TILE_VIEW_WIDTH*8 * TILE_VIEW_HEIGHT*8],
            ),
            egui::TextureOptions::NEAREST,
        );

        let grid_texture = cc.egui_ctx.load_texture(
            "tile map grid",
            egui::ColorImage::new(
                [TILE_VIEW_WIDTH*8*SCALE, TILE_VIEW_HEIGHT*8*SCALE],
                Self::create_grid(TILE_VIEW_WIDTH*8*SCALE, TILE_VIEW_HEIGHT*8*SCALE, 8*SCALE),
            ),
            egui::TextureOptions::NEAREST,
        );
       
        Self {
            view_params,
            params: TileViewParams {
                tile_buffer, texture_buffer,
                grid_texture, selected_tile: 0
            }
        }
    }

    pub fn window(&mut self, ui: &mut egui::Ui, mmu: &Mmu, draw_palette: &RawPalette) {
        if self.view_params.is_open {
            self.decode_tiles(mmu, draw_palette);

            self.params.texture_buffer.set(
                egui::ColorImage::from_rgb(
                    [TILE_VIEW_WIDTH*8, TILE_VIEW_HEIGHT*8],
                    &self.params.tile_buffer
                ), 
                egui::TextureOptions::NEAREST
            );
        }

        self.view_params.show_window(ui, |ui| {
            Self::show(ui, &mut self.params);
        });
    }

    fn show(ui: &mut egui::Ui, params: &mut TileViewParams) {
        ui.vertical(|ui| {

            ui.horizontal(|ui| {
                ui.add(
                    Self::get_tile(params.selected_tile, &params.texture_buffer)
                );

                ui.vertical(|ui| {
                    ui.label(format!("Tile ID: {}", params.selected_tile));
                    ui.label(format!("Address: {:#06X}", 0x8000 + params.selected_tile));
                });
            });


            let size = egui::vec2((TILE_VIEW_WIDTH*8*SCALE) as f32, (TILE_VIEW_HEIGHT*8*SCALE) as f32);
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

            ui.put(
                rect,
                egui::Image::new((
                    params.texture_buffer.id(),
                    egui::vec2((TILE_VIEW_WIDTH*8*SCALE) as f32, (TILE_VIEW_HEIGHT*8*SCALE) as f32),
                )),
            );
            ui.put(
                rect,
                egui::Image::new((
                    params.grid_texture.id(),
                    size,
                )),
            );

            if response.clicked() 
                && let Some(pos) = response.interact_pointer_pos() {
                    let local = pos - rect.min;
                    let tx = (local.x / (8*SCALE) as f32) as usize;
                    let ty = (local.y / (8*SCALE) as f32) as usize;
                    let tile_id = (ty * TILE_VIEW_WIDTH) + tx;
                    params.selected_tile = tile_id;
            }
        });
    }

    fn decode_tiles(&mut self, mmu: &Mmu, draw_palette: &RawPalette) {
        let bg_palette = mmu.get_bg_palette();

        for y in 0..TILE_VIEW_HEIGHT*8 {
            for x in 0..TILE_VIEW_WIDTH*8 {
                let tile_i = (y/8 * TILE_VIEW_WIDTH) + x/8;
                let color_index = mmu.tile_set[tile_i][y%8][x%8];
                let shade = ShadeBuffer::get_shade(bg_palette, color_index);
                let rgb = draw_palette.match_shade(shade);

                let buff_i = (y * TILE_VIEW_WIDTH*8 + x) * 3;
                self.params.tile_buffer[buff_i] =   rgb[0];
                self.params.tile_buffer[buff_i+1] = rgb[1];
                self.params.tile_buffer[buff_i+2] = rgb[2];
            }
        }
    }

    fn create_grid(width: usize, height: usize, tile_size: usize) -> Vec<egui::Color32> {
        let mut result = vec![egui::Color32::TRANSPARENT; width * height];
        for x in (tile_size..width).step_by(tile_size) {
            for y in 0..height {
                result[(y * width) + x] = egui::Color32::BLACK;
            }
        }
        for y in (tile_size..height).step_by(tile_size) {
            for x in 0..width {
                result[(y * width) + x] = egui::Color32::BLACK;
            }
        }
        result
    }

    fn get_tile(tile_id: usize, texture: &TextureHandle) -> egui::Image<'_> {
        let tx = tile_id%TILE_VIEW_WIDTH;
        let ty = tile_id/TILE_VIEW_WIDTH;
        
        let uv_min = egui::pos2(
            tx as f32 / TILE_VIEW_WIDTH as f32,
            ty as f32 / TILE_VIEW_HEIGHT as f32,
        );

        let uv_max = egui::pos2(
            (tx+1) as f32 / TILE_VIEW_WIDTH as f32,
            (ty+1) as f32 / TILE_VIEW_HEIGHT as f32,
        );

        egui::Image::new(
            (
                texture.id(),
                egui::vec2(64.0, 64.0),
            )
        ).uv(egui::Rect::from_min_max(uv_min, uv_max))
    }
}