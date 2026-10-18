use std::collections::HashSet;

use foundation::Vec2;
use winit::event::{ElementState, KeyEvent, MouseButton};

use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Default)]
pub struct Input {
    keys_held: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    mouse_held: HashSet<MouseButton>,
    raw_motion: Vec2,
    focused: bool,
}

impl Input {
    pub fn key_held(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_held.contains(&button)
    }

    pub fn raw_motion(&self) -> Vec2 {
        self.raw_motion
    }

    pub(crate) fn on_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.keys_held.clear();
            self.mouse_held.clear();
        }
    }

    pub(crate) fn on_keyboard(&mut self, event: KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        match event.state {
            ElementState::Pressed => {
                if self.keys_held.insert(code) {
                    self.keys_pressed.insert(code);
                }
            }
            ElementState::Released => {
                self.keys_held.remove(&code);
            }
        }
    }

    pub(crate) fn on_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.mouse_held.insert(button);
            }
            ElementState::Released => {
                self.mouse_held.remove(&button);
            }
        }
    }

    pub(crate) fn on_mouse_motion(&mut self, delta: Vec2) {
        if self.focused {
            self.raw_motion += delta;
        }
    }

    pub(crate) fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.raw_motion = Vec2::ZERO;
    }
}
