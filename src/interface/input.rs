//!
//! Input processor:
//! 
//!     Handles keyboard and controller input using 'winit' and 'gilrs' respectfully.
//!     The only noteworthy thing I do here is dispatch repeat on hold for controllers.
//! 


use eframe::egui::{self, Key};
use gilrs::{Axis, Button, Event, EventType, GamepadId, Gilrs};

use crate::emu::controls::joypad::SoftwareJoypad;


/// Stick deadzone
/// It's pretty high since it mirrors the D-pad
const DEADZONE: f32 = 0.65;
/// Delay in seconds before button becomes considered held
const HOLD_DELAY: f32 = 0.25;
/// Time interval in seconds at which button input gets repeated
const REPEAT_RATE: f32 = 0.05;


#[derive(Clone, Copy)]
struct RepeatSoftwareJoypad {
    a:      ButtonRepeat,   b:      ButtonRepeat,
    select: ButtonRepeat,   start:  ButtonRepeat,
    up:     ButtonRepeat,   down:   ButtonRepeat,
    left:   ButtonRepeat,   right:  ButtonRepeat,
}
impl From<RepeatSoftwareJoypad> for SoftwareJoypad {
    fn from(val: RepeatSoftwareJoypad) -> Self {
        SoftwareJoypad {
            a:      val.a.pressed,      b:     val.b.pressed,
            select: val.select.pressed, start: val.start.pressed,
            up:     val.up.pressed,     down:  val.down.pressed,
            left:   val.left.pressed,   right: val.right.pressed,
        }
    }
}
impl Default for RepeatSoftwareJoypad {
    fn default() -> Self {
        Self { 
            a:      ButtonRepeat::new(Button::South),
            b:      ButtonRepeat::new(Button::East),
            select: ButtonRepeat::new(Button::Select),
            start:  ButtonRepeat::new(Button::Start),
            up:     ButtonRepeat::new(Button::DPadUp),
            down:   ButtonRepeat::new(Button::DPadDown),
            left:   ButtonRepeat::new(Button::DPadLeft),
            right:  ButtonRepeat::new(Button::DPadRight)
        }
    }
}
impl<'a> IntoIterator for &'a mut RepeatSoftwareJoypad {
    type Item = &'a mut ButtonRepeat;
    type IntoIter = std::array::IntoIter<Self::Item, 8>;
    fn into_iter(self) -> Self::IntoIter {
        [
            &mut self.a, &mut self.b, &mut self.select, &mut self.start,
            &mut self.up, &mut self.down, &mut self.left, &mut self.right,
        ].into_iter()
    }
}

#[derive(Default, Clone, Copy)]
struct ButtonRepeat {
    button: Button,
    pressed: bool,
    timer: f32,
}
impl ButtonRepeat {
    pub fn new(button: Button) -> Self {
        Self { button, ..Default::default() }
    }

    // Repeat logic
    fn update_button(&mut self, pressed: bool, dt: f32) -> bool {
        if !pressed { // If NOT pressed
            self.pressed = false;
            self.timer = 0.0;
            false
        } else { // If pressed
            if !self.pressed {
                self.pressed = true;
                self.timer = -HOLD_DELAY;
                true
            } else {
                self.timer += dt;

                if self.timer < REPEAT_RATE { false }
                else {
                    self.timer -= REPEAT_RATE;
                    true
                }
            }
        }
    }
}

pub struct Input {
    gilrs: Gilrs,
    joypad: RepeatSoftwareJoypad,
    active_gamepad: Option<GamepadId>,
}
impl Input {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().expect("Failed to initialize Gilrs");
        Self { gilrs, joypad: RepeatSoftwareJoypad::default(), active_gamepad: None }
    }

    fn max_abs(a: f32, b: f32) -> f32   { if a.abs() > b.abs() { a } else { b }    }
    fn apply_deadzone(v: f32) -> f32    { if v.abs() < DEADZONE { 0.0 } else { v } }
    fn poll_gamepad(&mut self, dt: f32) -> SoftwareJoypad {
        // Detect current controller
        while let Some(Event { id, event, .. }) = self.gilrs.next_event() {
            match event {
                EventType::ButtonPressed(_, _)
                | EventType::ButtonRepeated(_, _)
                | EventType::AxisChanged(_, _, _) => self.active_gamepad = Some(id),
                _ => ()
            }
        }

        // Process input
        if let Some(id) = self.active_gamepad {
            let gamepad = self.gilrs.gamepad(id);
            
            // Check the buttons
            for b in &mut self.joypad {
                b.update_button(gamepad.is_pressed(b.button), dt);
            }

            // Check the sticks if D-pad isn't pressed
            if  !self.joypad.up.pressed   &
                !self.joypad.down.pressed &
                !self.joypad.left.pressed &
                !self.joypad.right.pressed
            {
                let x: f32 = Self::apply_deadzone(
                    Self::max_abs(
                        gamepad.value(Axis::LeftStickX),
                        gamepad.value(Axis::RightStickX)
                    )
                );
                let y =  Self::apply_deadzone(
                    Self::max_abs(
                        gamepad.value(Axis::LeftStickY),
                        gamepad.value(Axis::RightStickY)
                    )
                );

                if (x != 0.0) || (y != 0.0) {
                    if x == 0.0     { self.joypad.left.pressed = false; self.joypad.right.pressed = false; }
                    else if x < 0.0 { self.joypad.left.pressed = true;                                     }
                    else            { self.joypad.right.pressed = true;                                    }

                    if y == 0.0     { self.joypad.up.pressed = false; self.joypad.down.pressed = false; }
                    else if y > 0.0 { self.joypad.up.pressed = true;                                    }
                    else            { self.joypad.down.pressed = true;                                  }
                }
            }
        }

        self.joypad.into()
    }

    fn poll_keyboard(ctx: &egui::Context) -> SoftwareJoypad {
        let mut buttons = SoftwareJoypad::default();
        ctx.input(|input| {
            if input.key_down(Key::X)           { buttons.a        = true; }
            if input.key_down(Key::Z)           { buttons.b        = true; }
            if input.key_down(Key::Enter)       { buttons.start    = true; }
            if input.key_down(Key::Space)       { buttons.select   = true; }
            if input.key_down(Key::ArrowUp)     { buttons.up       = true; }
            if input.key_down(Key::ArrowDown)   { buttons.down     = true; }
            if input.key_down(Key::ArrowLeft)   { buttons.left     = true; }
            if input.key_down(Key::ArrowRight)  { buttons.right    = true; }
        });
        buttons
    }

    pub fn process(&mut self, ctx: &egui::Context, dt: f32) -> SoftwareJoypad {
        let a = Self::poll_keyboard(ctx);
        let b = self.poll_gamepad(dt);
        SoftwareJoypad::combine(&a, &b)
    }
}