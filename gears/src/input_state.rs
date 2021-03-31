use std::collections::HashMap;

use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
    window::Window,
};

pub struct InputState {
    keymap: HashMap<VirtualKeyCode, bool>,
    window_focused: bool,
    window_size: LogicalSize<u32>,
    window_psize: PhysicalSize<u32>,
}

impl InputState {
    pub fn new(
        window_focused: bool,
        window_size: LogicalSize<u32>,
        window_psize: PhysicalSize<u32>,
    ) -> Self {
        Self {
            keymap: HashMap::new(),
            window_focused,
            window_size,
            window_psize,
        }
    }

    pub fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub fn window_size(&self) -> LogicalSize<u32> {
        self.window_size
    }

    pub fn window_physical_size(&self) -> PhysicalSize<u32> {
        self.window_psize
    }

    pub fn key_held(&self, key: VirtualKeyCode) -> bool {
        if let Some(value) = self.keymap.get(&key) {
            *value
        } else {
            false
        }
    }

    pub fn update_key(&mut self, input: &KeyboardInput) {
        input.virtual_keycode.map(|keycode| {
            self.keymap.insert(
                keycode,
                match input.state {
                    ElementState::Pressed => true,
                    _ => false,
                },
            );
        });
    }

    pub fn update(&mut self, event: &WindowEvent, window: &Window) {
        match event {
            WindowEvent::KeyboardInput { input, .. } => self.update_key(input),
            WindowEvent::Focused(f) => self.window_focused = *f,
            WindowEvent::Resized(size) => {
                self.window_size = size.clone().to_logical(window.scale_factor())
            }
            _ => (),
        }
    }
}
