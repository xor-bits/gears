use winit::{dpi::PhysicalPosition, event::WindowEvent};

use crate::frame::Frame;

pub enum HideMode {
    None,

    // Cursor gets moved to the middle of screen always when it is moved.
    // CursorMove::moved gets called
    GrabCursor,

    // Hide cursor whenever the window is hovered.
    Hovered,
}

pub struct CursorController {
    focused: bool,
    hide_mode: HideMode,
    next_ignored: bool,
    next_ignored_value: Option<PhysicalPosition<f64>>,
}

impl CursorController {
    pub fn new() -> Self {
        Self {
            focused: false,
            hide_mode: HideMode::None,
            next_ignored: true,
            next_ignored_value: None,
        }
    }

    pub fn with_hide_mode(mut self, hide_mode: HideMode) -> Self {
        self.hide_mode = hide_mode;
        self
    }

    pub fn event(&mut self, event: &WindowEvent, frame: &Frame) -> Option<(f64, f64)> {
        match event {
            WindowEvent::Focused(f) => self.focused = *f,
            _ => {}
        }

        match (event, &self.hide_mode) {
            (WindowEvent::Focused(f), HideMode::GrabCursor) => {
                let window = frame.window();
                window.set_cursor_visible(!f);

                if *f {
                    let size = window.inner_size();
                    let middle = PhysicalPosition {
                        x: size.width / 2,
                        y: size.height / 2,
                    };

                    window.set_cursor_position(middle).unwrap();
                }
                None
            }
            (WindowEvent::Focused(f), HideMode::Hovered) => {
                let window = frame.window();
                window.set_cursor_visible(!f);
                None
            }
            (WindowEvent::CursorMoved { position, .. }, HideMode::GrabCursor) => {
                if !self.focused {
                    return None;
                }

                let window = frame.window();

                let size = window.inner_size();
                let middle = PhysicalPosition {
                    x: size.width / 2,
                    y: size.height / 2,
                };

                let centered_position = PhysicalPosition::new(
                    (position.x - middle.x as f64) * 0.001,
                    (position.y - middle.y as f64) * 0.001,
                );

                if self.next_ignored {
                    self.next_ignored_value = Some(centered_position);
                    self.next_ignored = false;
                    return None;
                }
                if let Some(value) = &self.next_ignored_value {
                    if value == &centered_position {
                        return None;
                    } else {
                        self.next_ignored_value = None;
                    }
                }

                if centered_position.x != 0.0 || centered_position.y != 0.0 {
                    window.set_cursor_position(middle).unwrap();
                    Some((centered_position.x, centered_position.y))
                } else {
                    None
                }
            }
            (_, _) => None,
        }
    }

    pub fn hide_cursor(&mut self, hide_mode: HideMode) {
        self.hide_mode = hide_mode;
    }

    pub fn show_cursor(&mut self) {
        self.hide_cursor(HideMode::None);
    }
}
