//! 
//! Application top bar:
//! 
//!     Contains all of the bells, whistles, buttons and toggles.
//! 


use eframe::egui::{self, Align, Button, Layout, MenuBar, Panel, RichText, Slider, ViewportCommand};

use crate::{emu::{audio::MIN_VOLUME, emulator::Emulator, rendering::palette::PALETTES}, interface::{helper::Helper, renderer::ScreenIntegerScale, theme::{self, ui_theme::THEMES}}};


impl Emulator {
    pub fn show_topbar(&mut self, ui: &mut egui::Ui) {
        let minimal_menu_frame = egui::Frame::menu(ui.style())
            .corner_radius(0.0).inner_margin(0.0).stroke(egui::Stroke::NONE);
        
        Panel::top("app_top_bar").frame(minimal_menu_frame).show_inside(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)).show(ui, |ui| {
                MenuBar::new().ui(ui, |ui| {
                    ui.spacing_mut().item_spacing.x =   2.0;
                    ui.spacing_mut().button_padding.x = 10.0;
                    ui.spacing_mut().button_padding.y = 1.0;
                    ui.visuals_mut().widgets.noninteractive.bg_stroke
                        = egui::Stroke::new(1.0, self.ui.get_theme().surface1); // Changes the separator color

                    // File tab
                    Helper::custom_dropdown(ui, "File", false, |ui| {

                        Helper::text_separator_centered(ui, "ROM");

                        // Open ROM
                        if ui.button("Open ROM").clicked() {
                            let file_tx = self.ui.file_tx.clone();
               
                            let task = async move {
                                let file = rfd::AsyncFileDialog::new()
                                    .add_filter("ROM files", &["gb", "bin"])
                                    .pick_file()
                                    .await;

                                if let Some(file_handle) = file {
                                    let name = file_handle.file_name();
                                    let bytes = file_handle.read().await;

                                    #[cfg(not(target_arch = "wasm32"))]
                                    let path  = file_handle.path().to_path_buf();
                                    #[cfg(target_arch = "wasm32")]
                                    let path = std::path::PathBuf::new();
                                    
                                    // Send data back to the main UI thread
                                    let _ = file_tx.send((name, path, bytes));
                                }
                            };

                            #[cfg(not(target_arch = "wasm32"))]
                            std::thread::spawn(move || pollster::block_on(task) );

                            #[cfg(target_arch = "wasm32")]
                            wasm_bindgen_futures::spawn_local(task);
                            
                            ui.close();
                        }
                        // Recent ROMs
                        #[cfg(not(target_arch = "wasm32"))]
                        ui.menu_button("Open Recent", |ui| {
                            Helper::set_popup_style(ui);

                            if self.ui.recent_roms.list.is_empty() {
                                ui.add_enabled(false, Button::new("No Recent Items"));
                            } else {
                                let paths_to_render = self.ui.recent_roms.list.clone();

                                for path in paths_to_render {
                                    let display_name = crate::interface::ui::Ui::get_file_display_name(&path);
                                    if ui.button(&display_name).on_hover_text(path.to_string_lossy()).clicked() {
                                        crate::interface::ui::try_load_rom_from_path(self, ui.ctx(), display_name, path.clone());
                                        self.ui.recent_roms.add(path);
                                        ui.close();
                                    }
                                }

                                ui.separator();

                                if ui.add(Helper::shortcut_button("Clear Recent", "🗑")).clicked() {
                                    self.ui.recent_roms.list.clear();
                                }
                            }
                        });

                        ui.add_enabled_ui(self.get_rom().is_some(), |ui| {
                            if ui.add(Helper::shortcut_button("Close ROM", "❌")).clicked() {
                                self.reset(None, true);
                            }
                        });

                        Helper::text_separator_centered(ui, "Boot ROM");

                        ui.add_enabled_ui(self.ui.boot_rom.is_some(), |ui| {
                            // Skip boot ROM button
                            if ui.add(Helper::shortcut_button("Skip Boot", "⏩").selected(self.ui.skip_boot && self.ui.boot_rom.is_some())).clicked() { 
                                self.ui.skip_boot = !self.ui.skip_boot;
                            }
                            // Clear boot ROM button
                            if ui.add(Helper::shortcut_button("Clear Boot ROM", "🗑")).clicked() {
                                self.ui.boot_rom = None;
                            }
                        });

                        Helper::text_separator_centered(ui, "Exit");

                        // Exit
                        ui.scope(|ui| {
                            let widgets = &mut ui.visuals_mut().widgets;
                            widgets.inactive.weak_bg_fill = self.ui.get_theme().maroon.linear_multiply(0.2);
                            widgets.hovered.weak_bg_fill = self.ui.get_theme().maroon.linear_multiply(0.4);
                            widgets.active.weak_bg_fill = self.ui.get_theme().maroon.linear_multiply(0.3);

                            if ui.button(RichText::new("Exit").color(self.ui.get_theme().maroon)).clicked() {
                                ui.send_viewport_cmd(ViewportCommand::Close);
                            }
                        });
                    });

                    // Emulation tab
                    Helper::custom_dropdown(ui, "Emulation", true, |ui| {

                        Helper::text_separator_centered(ui, "Controls");

                        if Helper::shortcut_selectable_button(ui, self.ui.pause, "Pause", "Ctrl + P", "⏸").clicked() {
                            self.toggle_pause();
                        }
                        
                        ui.add_enabled_ui(self.get_rom().is_some(), |ui| {
                            if ui.add(Helper::shortcut_button("Reset System", "↺  Ctrl + R")).clicked() {
                                self.reset(None, false);
                            }
                        });
                    });

                    // Audio and video tab
                    Helper::custom_dropdown(ui, "Audio/Video", true, |ui| {

                        Helper::text_separator_centered(ui, "Visuals");

                        // Fullscreen toggle
                        #[cfg(not(target_arch = "wasm32"))]
                        let fullscreen_button = Helper::shortcut_selectable_button(ui, self.ui.fullscreen, "Fullscreen", "F11", "❌");
                        #[cfg(target_arch = "wasm32")]
                        let fullscreen_button = Helper::selectable_button(ui, self.ui.fullscreen, "Fullscreen", "❌");

                        if fullscreen_button.clicked() {
                            self.ui.fullscreen = !self.ui.fullscreen;
                        }

                        // Theme selector
                        ui.menu_button("Themes", |ui| {
                            Helper::set_popup_style(ui);

                            for (id, item) in THEMES.iter().enumerate() {
                                if Helper::selectable_button(ui, self.ui.theme_id == id, item.name, "✅").clicked() {
                                    self.ui.theme_id = id;
                                    theme::catppuccin::set_theme(ui, *self.ui.get_theme());
                                }
                            }
                        });

                        // Palette selector
                        ui.menu_button("Palettes", |ui| {
                            Helper::set_popup_style(ui);

                            for id in 0..PALETTES.len() {
                                let name = self.ui.get_palette(id).name;
                                if Helper::selectable_button(ui, self.ui.palette_id == id, name, "✅").clicked() {
                                    self.ui.palette_id = id;
                                }
                            }
                        });

                        // Shader selector
                        ui.menu_button("Shaders", |ui| {
                            Helper::set_popup_style(ui);

                            if Helper::selectable_button(ui, self.ui.shader_state.shader_id == 0, "None", "✅").clicked() {
                                self.ui.shader_state.shader_id = 0;
                            }

                            for id in 1..=self.ui.shader_state.get_entry_len() {
                                let entry = &self.ui.shader_state.get_entry(id);
                                if Helper::selectable_button(ui, self.ui.shader_state.shader_id == id, &entry.name, "✅").clicked() {
                                    self.ui.shader_state.shader_id = id;
                                }
                            }
                        });

                        Helper::text_separator_centered(ui, "Scaling");

                        // Scaling selector
                        ui.add_enabled_ui(self.ui.available_max_scale as usize > 1, |ui| {
                            ui.menu_button("Scaling Factor", |ui| {
                                Helper::set_popup_style(ui);

                                if self.ui.available_max_scale as usize > 1 {
                                    
                                    let mut size = self.ui.available_max_scale as usize;
                                    if self.ui.scale != ScreenIntegerScale::Max { size = size.max(self.ui.scale as usize); }
                                    
                                    for s in (ScreenIntegerScale::X1 as usize)..=size {
                                        if Helper::selectable_button(ui, self.ui.scale as usize == s, format!("{}x scale", s), "✅").clicked() {
                                           self.ui.scale = s.into();
                                        }
                                    }

                                    ui.separator();

                                    if Helper::selectable_button(ui, self.ui.scale == ScreenIntegerScale::Max, "Max Available", "✅").clicked() {
                                        self.ui.scale = ScreenIntegerScale::Max;
                                    }

                                } else {
                                    ui.add_enabled(false, Button::new("No Available Items"));
                                }
                            });
                        });
                        
                        // Integer scaling
                        if Helper::selectable_button(ui, self.ui.scale != ScreenIntegerScale::None, "Integer Scaling", "❌").clicked() {
                            if self.ui.scale == ScreenIntegerScale::None { self.ui.scale = ScreenIntegerScale::Max; } else { self.ui.scale = ScreenIntegerScale::None; }
                        }

                        Helper::text_separator_centered(ui, "Audio");

                        // Audio channel selector
                        ui.menu_button("Audio Channels", |ui| {
                            Helper::set_popup_style(ui);

                            for (i, ch) in &mut self.ui.audio_channels.into_iter().enumerate() {
                                if Helper::selectable_button(ui, *ch, format!("Channel {}", i), "✅").clicked() {
                                    *ch = !*ch;
                                }
                            }
                        });
                    });

                    // Tools tab
                    Helper::custom_dropdown(ui, "Tools", true, |ui| {
                        Helper::text_separator_centered(ui, "Views");

                        self.ui.header_view.view_params.window_toggle(ui);
                        self.ui.tile_view.view_params.window_toggle(ui);
                        self.ui.debug_view.view_params.window_toggle(ui);
                    });

                    ui.separator();

                    // FPS counter
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(7.0);
                        
                        let fps_text = format!("FPS: {:.0}", self.ui.frame_counter.fps);
                        if self.ui.frame_counter.fps < 58.0 { ui.colored_label(self.ui.get_theme().yellow, fps_text); }
                        else { ui.colored_label(self.ui.get_theme().green, fps_text); }

                        ui.add_space(7.0);

                        if ui.add(Slider::new(&mut self.audio.volume, MIN_VOLUME..=1.0).logarithmic(true).show_value(false)).changed() {
                            self.audio.muted = self.audio.volume == MIN_VOLUME;
                        };

                        ui.add_space(3.0);

                        ui.horizontal(|ui| {
                            let icon = if self.audio.muted || self.audio.volume == MIN_VOLUME { "🔇" }
                                else if self.audio.volume < 0.05 { "🔈" }
                                else if self.audio.volume < 0.3 { "🔉" }
                                else { "🔊" };

                            let selected = self.audio.muted || self.audio.volume == MIN_VOLUME;
                            let text = if selected { RichText::new(icon).color(self.ui.get_theme().maroon) } else { RichText::new(icon) };
                            let volume_btn = egui::Button::new(text).frame(true).selected(selected);

                             ui.scope(|ui| {
                                ui.visuals_mut().selection.bg_fill = self.ui.get_theme().maroon.linear_multiply(0.3);

                                if ui.add(volume_btn).clicked()
                                    && self.audio.volume != MIN_VOLUME { 
                                        self.audio.muted = !self.audio.muted; 
                                    }
                            });
                        });

                        ui.separator();
                    });
                });
            });
        });
    }
}