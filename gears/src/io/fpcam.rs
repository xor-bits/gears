use super::input_state::{InputAxis, InputState};
use crate::{frame::Frame, game_loop::Event};
use glam::Vec2;
use winit::event::{DeviceEvent, Event as WinitEvent, WindowEvent};

//

/// First person camera controller
#[derive(Debug, Clone, Copy, Default)]
pub struct FPCam {
    focused: bool,
    dir: Vec2,
    vel: Vec2,
}

//

impl FPCam {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dir(dir: Vec2) -> Self {
        Self {
            dir,
            ..Default::default()
        }
    }

    pub fn dir(&self, delta: f32) -> Vec2 {
        Self::clamp2(self.dir + self.vel * delta)
    }

    pub fn update(&mut self, input: &InputState, delta: f32) {
        self.vel = delta * Vec2::new(-3.0, 3.0) * input.get_axis(InputAxis::Look, 0);
        self.dir += self.vel;
        self.clamp();
    }

    pub fn event(&mut self, event: &Event, frame: &Frame) {
        match event {
            Event::WinitEvent(WinitEvent::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (x, y) },
                ..
            }) if self.focused => {
                self.dir -= Vec2::new(*x as f32 * 0.001, *y as f32 * 0.001);
                self.clamp();
            }
            Event::WinitEvent(WinitEvent::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            }) => {
                self.focused = *focused;
                let _ = frame.window().set_cursor_grab(self.focused);
                frame.window().set_cursor_visible(!self.focused);
            }
            _ => {}
        }
    }

    //

    fn clamp(&mut self) {
        self.dir = Self::clamp2(self.dir);
    }

    fn clamp2(mut dir: Vec2) -> Vec2 {
        dir.y = dir.y.clamp(
            -std::f32::consts::PI / 2.0 + 0.01,
            std::f32::consts::PI / 2.0 - 0.01,
        );
        dir
    }
}
