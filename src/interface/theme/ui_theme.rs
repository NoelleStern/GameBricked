//! 
//! Just a small theme adapter to not alter the catppuccin code
//! 
//!     Catppuccin themes come directly from:
//!         https://github.com/catppuccin/egui
//! 


use crate::interface::theme::themes::{FRAPPE, LATTE, MACCHIATO, MOCHA, Theme};


pub const THEMES: [UITheme; 4] = [
    UITheme::new("Frappe",      FRAPPE   ),
    UITheme::new("Macchiato",   MACCHIATO),
    UITheme::new("Mocha",       MOCHA    ),
    UITheme::new("Latte",       LATTE    ),
]; 


pub struct UITheme<'a> {
    pub name: &'a str,
    pub theme: Theme,
}
impl<'a> UITheme<'a> {
    pub const fn new(name: &'a str, theme: Theme) -> Self {
        Self { name, theme }
    }
}