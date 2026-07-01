//! 
//! Eframe UI helper functions
//! 


use eframe::egui::{self, Align, Align2, Button, CornerRadius, Frame, IntoAtoms, Layout, Popup, PopupCloseBehavior, RectAlign, Response, RichText, Sense, Stroke, TextStyle};


/// Popup exact width value
const POPUP_WIDTH: f32 = 180.0;


pub struct Helper;
impl Helper {
    pub fn shortcut_button<'a, T: IntoAtoms<'a>>(t: T, a: &str) -> Button<'a> {
        Button::new(RichText::new(a).weak()).left_text(t)
    }
    pub fn selectable_button<'a, T: IntoAtoms<'a>>(ui: &mut egui::Ui, c: bool, t: T, a: &str) -> Response {
        if !c { ui.add(Button::new(t)) } else { ui.add(Self::shortcut_button(t, a).selected(true)) }
    }
    pub fn shortcut_selectable_button<'a, T: IntoAtoms<'a>>(ui: &mut egui::Ui, c: bool, t: T, s: &str, a: &str) -> Response {
        let text = if c { &format!("{}  {}", a, s) } else { s };
        ui.add(Self::shortcut_button(t, text).selected(c))
    }

    pub fn custom_dropdown(ui: &mut egui::Ui, button_text: &str, separator: bool, add_contents: impl FnOnce(&mut egui::Ui)) {
        let custom_popup_frame = Frame::menu(ui.style())
            .corner_radius( CornerRadius { nw: 0, ne: 0, sw: 6, se: 6 } );

        let anchor_rect = if separator { Some(ui.separator().rect) } else { None }; // Add a separator if specified
        let response = ui.button(button_text); // Add the button right after
        let rect = if let Some(rect) = anchor_rect { rect } else { response.rect };

        let align = if anchor_rect.is_none() { RectAlign::BOTTOM_START } 
        else { RectAlign { parent: Align2::CENTER_BOTTOM, child: Align2::LEFT_TOP } };
        let pos = align.parent.pos_in_rect(&rect) + egui::vec2(0.0, 5.0);

        Popup::menu(&response)
            .frame(custom_popup_frame).anchor(pos)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                    Self::set_popup_style(ui);
                    add_contents(ui);
                });
            });
        
    }

    pub fn set_popup_style(ui: &mut egui::Ui) {
        ui.set_width(POPUP_WIDTH);
        ui.spacing_mut().button_padding.x = 6.0;
        ui.spacing_mut().button_padding.y = 1.0;
        ui.spacing_mut().item_spacing.y = 3.0;
    }

    pub fn text_separator_centered(ui: &mut egui::Ui, text: &str) {
        let font_id = TextStyle::Body.resolve(ui.style());
        let text_color = ui.visuals().weak_text_color();
        let line_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let margin = 8.0;
        
        // Measure text size
        let galley = ui.painter().layout_no_wrap(text.to_string(), font_id, text_color);
        let text_size = galley.size();
        
        // Allocate space
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), text_size.y), 
            Sense::hover()
        );

        // Draw text at center
        let text_x = rect.center().x - text_size.x / 2.0;
        let text_pos = egui::pos2(text_x, rect.min.y);
        ui.painter().galley(text_pos, galley, text_color);

        let y = rect.center().y;
        let stroke = Stroke::new(1.0, line_color);
        
        // Draw lines
        let text_right = text_x + text_size.x + margin;
        if text_x - margin > rect.left() { ui.painter().hline(rect.left()..=(text_x - margin), y, stroke); } // Draw the left line
        if text_right < rect.right() {  ui.painter().hline(text_right..=rect.right(), y, stroke); } // Draw the right line
    }
}