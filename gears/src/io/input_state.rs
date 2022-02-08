/* use crate::loops::frame::EventLoopTarget; */
use parking_lot::RwLock;
use std::collections::HashMap;
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

pub struct InputState {
    keymap: HashMap<VirtualKeyCode, bool>,
    window_focused: bool,
}

impl InputState {
    pub fn new() -> RwLock<Self> {
        RwLock::new(Self {
            keymap: HashMap::new(),
            window_focused: false,
        })
    }

    pub fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub fn key_held(&self, key: VirtualKeyCode) -> bool {
        if let Some(value) = self.keymap.get(&key) {
            *value
        } else {
            false
        }
    }

    pub fn update_key(&mut self, input: &KeyboardInput) {
        if let Some(keycode) = input.virtual_keycode { self.keymap.insert(
                keycode,
                match input.state {
                    ElementState::Pressed => true,
                    _ => false,
                },
            ); }
    }

    pub fn update(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { input, .. } => self.update_key(input),
            WindowEvent::Focused(f) => self.window_focused = *f,
            _ => (),
        }
    }
}

/* impl EventLoopTarget for InputState {
    fn event(&self, event: &WindowEvent) {
        self.update(event);
    }
} */
