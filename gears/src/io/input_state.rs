use crate::game_loop::Event;
use gilrs::{Axis, Button, Event as GilrsEvent, EventType, GamepadId, Gilrs};
use glam::Vec2;
use std::collections::{hash_map::Entry, HashMap};
use winit::event::{
    ElementState, Event as WinitEvent, KeyboardInput, ScanCode, VirtualKeyCode, WindowEvent,
};

//

#[derive(Debug)]
pub struct InputState {
    virtual_keymap: HashMap<VirtualKeyCode, bool>,
    scancode_keymap: [bool; 150],

    players: Vec<Option<GamepadId>>,
    gamepads: HashMap<GamepadId, Gamepad>,

    window_focused: bool,
    should_close: bool,
}

#[derive(Debug, Default)]
struct Gamepad {
    player: usize,
    buttons: HashMap<Button, f32>,
    axis: HashMap<Axis, f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Input {
    /// W in most QWERTY keyboards
    ///
    /// Left stick in most gamepads
    MoveUp,
    /// S in most QWERTY keyboards
    ///
    /// Left stick in most gamepads
    MoveDown,
    /// A in most QWERTY keyboards
    ///
    /// Left stick in most gamepads
    MoveLeft,
    /// D in most QWERTY keyboards
    ///
    /// Left stick in most gamepads
    MoveRight,

    /// Up arrow in most QWERTY keyboards
    ///
    /// Right stick in most gamepads
    LookUp,
    /// Down arrow in most QWERTY keyboards
    ///
    /// Right stick in most gamepads
    LookDown,
    /// Left arrow in most QWERTY keyboards
    ///
    /// Right stick in most gamepads
    LookLeft,
    /// Right arrow in most QWERTY keyboards
    ///
    /// Right stick in most gamepads
    LookRight,

    /// R in most QWERTY keyboards
    ///
    /// DPad in most gamepads
    RollUp,
    /// F in most QWERTY keyboards
    ///
    /// DPad in most gamepads
    RollDown,
    /// Q in most QWERTY keyboards
    ///
    /// DPad in most gamepads
    RollLeft,
    /// E in most QWERTY keyboards
    ///
    /// DPad in most gamepads
    RollRight,

    /// Space in most QWERTY keyboards
    ///
    /// South/X/A in most gamepads
    Jump,
    /// LShift in most QWERTY keyboards
    ///
    /// East/O/B in most gamepads
    Crouch,
    /// R in most QWERTY keyboards
    ///
    /// West/☐/X in most gamepads
    Reload,

    /// LShift in most QWERTY keyboards
    ///
    /// Right trigger/RT2 in most gamepads
    Accelerate,
    /// LControl in most QWERTY keyboards
    ///
    /// Left trigger/LT2 in most gamepads
    Decelerate,

    /// PageDown in most QWERTY keyboards
    ///
    /// Right shoulder/RT1 in most gamepads
    Next,
    /// PageUp in most QWERTY keyboards
    ///
    /// Left shoulder/LT1 in most gamepads
    Prev,

    /// Tab in most QWERTY keyboards
    Stats,
    /// Escape in most QWERTY keyboards
    Pause,
    /// Alt in most QWERTY keyboards
    Mode,

    /// Unrecognized key
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAxis {
    /// WASD in most QWERTY keyboards
    ///
    /// Left stick in most gamepads
    Move,

    /// Arrow keys in most QWERTY keyboards
    ///
    /// Right stick in most gamepads
    Look,

    /// QERF in most QWERTY keyboards
    ///
    /// DPad in most gamepads
    Roll,

    /// ?? in most QWERTY keyboards
    ///
    /// Right and right triggers in most gamepads
    ///
    /// Two dimensional: y is always 0.0
    Trigger,

    /// Shift and Space in most QWERTY keyboards
    ///
    /// South/X/A and East/O/B in most gamepads
    ///
    /// Two dimensional: y is always 0.0
    ZMove,
}

pub trait Triggered {
    fn triggered(self) -> bool;
}

impl Triggered for f32 {
    fn triggered(self) -> bool {
        self >= 0.5
    }
}

//

impl Default for InputState {
    fn default() -> Self {
        Self {
            virtual_keymap: Default::default(),
            scancode_keymap: [false; 150],

            players: Default::default(),
            gamepads: Default::default(),

            window_focused: Default::default(),
            should_close: Default::default(),
        }
    }
}

impl Input {
    pub fn from_name(name: &'static str) -> Input {
        match name {
            "move-up" => Input::MoveUp,
            "move-down" => Input::MoveDown,
            "move-left" => Input::MoveLeft,
            "move-right" => Input::MoveRight,
            _ => Input::Undefined,
        }
    }

    /// TODO: rebinding
    pub const fn into_scancode(self) -> ScanCode {
        match self {
            Input::MoveUp => 17,
            Input::MoveDown => 31,
            Input::MoveLeft => 30,
            Input::MoveRight => 32,

            Input::LookUp => 103,
            Input::LookDown => 108,
            Input::LookLeft => 105,
            Input::LookRight => 106,

            Input::RollUp => 19,
            Input::RollDown => 33,
            Input::RollLeft => 16,
            Input::RollRight => 18,

            Input::Jump => 57,
            Input::Crouch => 42,
            Input::Reload => 19,

            Input::Accelerate => 42,
            Input::Decelerate => 29,

            Input::Next => 109,
            Input::Prev => 104,

            Input::Stats => 15,
            Input::Pause => 1,
            Input::Mode => 56,

            Input::Undefined => !0,
        }
    }

    /// TODO: rebinding
    pub const fn into_button(self) -> Button {
        match self {
            Input::MoveUp => Button::LeftThumb,
            Input::MoveDown => Button::LeftThumb,
            Input::MoveLeft => Button::LeftThumb,
            Input::MoveRight => Button::LeftThumb,

            Input::LookUp => Button::RightThumb,
            Input::LookDown => Button::RightThumb,
            Input::LookLeft => Button::RightThumb,
            Input::LookRight => Button::RightThumb,

            Input::RollUp => Button::DPadUp,
            Input::RollDown => Button::DPadDown,
            Input::RollLeft => Button::DPadLeft,
            Input::RollRight => Button::DPadRight,

            Input::Jump => Button::South,
            Input::Crouch => Button::East,
            Input::Reload => Button::West,

            Input::Accelerate => Button::RightTrigger2,
            Input::Decelerate => Button::LeftTrigger2,

            Input::Next => Button::RightTrigger,
            Input::Prev => Button::LeftTrigger,

            Input::Pause => Button::Start,
            Input::Stats => Button::Select,
            Input::Mode => Button::Mode,

            Input::Undefined => Button::Unknown,
        }
    }

    /// TODO: rebinding
    pub const fn into_axis(self) -> Axis {
        match self {
            Input::MoveUp => Axis::LeftStickY,
            Input::MoveDown => Axis::LeftStickY,
            Input::MoveLeft => Axis::LeftStickX,
            Input::MoveRight => Axis::LeftStickX,

            Input::LookUp => Axis::RightStickY,
            Input::LookDown => Axis::RightStickY,
            Input::LookLeft => Axis::RightStickX,
            Input::LookRight => Axis::RightStickX,

            Input::RollUp => Axis::DPadY,
            Input::RollDown => Axis::DPadY,
            Input::RollLeft => Axis::DPadX,
            Input::RollRight => Axis::DPadX,

            Input::Jump => Axis::Unknown,
            Input::Crouch => Axis::Unknown,
            Input::Reload => Axis::Unknown,

            Input::Accelerate => Axis::RightZ,
            Input::Decelerate => Axis::LeftZ,

            Input::Next => Axis::Unknown,
            Input::Prev => Axis::Unknown,

            Input::Stats => Axis::Unknown,
            Input::Pause => Axis::Unknown,
            Input::Mode => Axis::Unknown,

            Input::Undefined => Axis::Unknown,
        }
    }

    /// TODO: rebinding
    pub const fn is_reverse(self) -> bool {
        match self {
            Input::MoveUp => false,
            Input::MoveDown => true,
            Input::MoveLeft => true,
            Input::MoveRight => false,

            Input::LookUp => false,
            Input::LookDown => true,
            Input::LookLeft => true,
            Input::LookRight => false,

            Input::RollUp => false,
            Input::RollDown => true,
            Input::RollLeft => true,
            Input::RollRight => false,

            Input::Jump => false,
            Input::Crouch => false,
            Input::Reload => false,

            Input::Accelerate => false,
            Input::Decelerate => true,

            Input::Next => false,
            Input::Prev => true,

            Input::Stats => false,
            Input::Pause => false,
            Input::Mode => false,

            Input::Undefined => false,
        }
    }
}

impl From<&'static str> for Input {
    fn from(name: &'static str) -> Self {
        Self::from_name(name)
    }
}

impl From<Input> for ScanCode {
    fn from(val: Input) -> Self {
        val.into_scancode()
    }
}

impl Gamepad {
    fn get_button_value(&self, button: Button) -> Option<f32> {
        match button {
            Button::Unknown => None,
            button => self.buttons.get(&button).cloned(),
        }
    }

    fn get_axis_value(&self, axis: Axis) -> Option<f32> {
        match axis {
            Axis::Unknown => None,
            axis => self.axis.get(&axis).cloned(),
        }
    }

    fn get_value(&self, input: Input) -> Option<f32> {
        self.get_button_value(input.into_button()).or_else(|| {
            self.get_axis_value(input.into_axis())
                .map(|f| if input.is_reverse() { -f } else { f })
        })
    }
}

//

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_input(&mut self, event: &Event, input: Input) -> Option<(f32, usize, ElementState)> {
        match event {
            Event::GilrsEvent(GilrsEvent {
                event: EventType::ButtonPressed(button, _),
                id,
                ..
            }) if *button != Button::Unknown && *button == input.into_button() => {
                let player = self.gamepad_entry(*id).player;
                Some((1.0, player, ElementState::Pressed))
            }
            Event::GilrsEvent(GilrsEvent {
                event: EventType::ButtonReleased(button, _),
                id,
                ..
            }) if *button != Button::Unknown && *button == input.into_button() => {
                let player = self.gamepad_entry(*id).player;
                Some((0.0, player, ElementState::Released))
            }
            Event::GilrsEvent(GilrsEvent {
                event: EventType::AxisChanged(axis, val, _),
                id,
                ..
            }) if *axis != Axis::Unknown && *axis == input.into_axis() => {
                let player = self.gamepad_entry(*id).player;
                Some((*val, player, ElementState::Pressed))
            }
            Event::WinitEvent(WinitEvent::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state, scancode, ..
                            },
                        ..
                    },
                ..
            }) if *scancode == input.into_scancode() => Some((
                match *state {
                    ElementState::Pressed => 1.0,
                    ElementState::Released => 0.0,
                },
                0,
                *state,
            )),
            _ => None,
        }
    }

    /* pub fn to_input_axis(&mut self, event: &Event, input: InputAxis) -> Option<(Vec2, usize)> {
        todo!()
    } */

    pub fn update_key(&mut self, input: &KeyboardInput) {
        /* log::debug!(
            "virtual key: {:?} scancode: {}",
            input.virtual_keycode,
            input.scancode
        ); */
        let state = matches!(input.state, ElementState::Pressed);
        if let Some(scancode) = self.scancode_keymap.get_mut(input.scancode as usize) {
            *scancode = state;
        }
        if let Some(keycode) = input.virtual_keycode {
            self.virtual_keymap.insert(keycode, state);
        }
    }

    pub fn update_joystrick(&mut self, event: &GilrsEvent) {
        match event.event {
            EventType::ButtonPressed(button, _) => {
                *self
                    .gamepad_entry(event.id)
                    .buttons
                    .entry(button)
                    .or_default() = 1.0;
            }
            EventType::ButtonRepeated(_, _) => {}
            EventType::ButtonReleased(button, _) => {
                *self
                    .gamepad_entry(event.id)
                    .buttons
                    .entry(button)
                    .or_default() = 0.0;
            }
            EventType::ButtonChanged(button, val, _) => {
                *self
                    .gamepad_entry(event.id)
                    .buttons
                    .entry(button)
                    .or_default() = val;
            }
            EventType::AxisChanged(axis, val, _) => {
                *self.gamepad_entry(event.id).axis.entry(axis).or_default() = val;
            }
            EventType::Connected => {}
            EventType::Disconnected => {}
            EventType::Dropped => {}
        };
    }

    pub fn event(&mut self, event: &Event) {
        match event {
            Event::GilrsEvent(event) => self.update_joystrick(event),
            Event::WinitEvent(WinitEvent::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            }) => self.update_key(input),
            // Event::WinitEvent(WinitEvent::DeviceEvent { event, .. }) => log::debug!("device event: {event:?}"),
            Event::WinitEvent(WinitEvent::WindowEvent {
                event: WindowEvent::Focused(f),
                ..
            }) => self.window_focused = *f,
            Event::WinitEvent(WinitEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }) => self.should_close = true,
            _ => (),
        }
    }

    //

    pub fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub fn gui_key_held(&self, key: VirtualKeyCode) -> bool {
        if let Some(value) = self.virtual_keymap.get(&key) {
            *value
        } else {
            false
        }
    }

    pub fn key_held<S>(&self, scancode: S) -> bool
    where
        S: Into<ScanCode>,
    {
        if let Some(value) = self.scancode_keymap.get(scancode.into() as usize) {
            *value
        } else {
            false
        }
    }

    /// player 0 is keyboard/mouse/controller/gamepad/joystick
    /// players 1.. are the other controllers/gamepads/joysticks
    pub fn get_input(&self, input: Input, player: usize) -> f32 {
        let mut val = 0.0;
        if let Some(gamepad) = self.get_gamepad(player) {
            val += gamepad.get_value(input).unwrap_or(0.0);
        }
        if player == 0 && self.key_held(input) {
            val += 1.0
        }
        val
    }

    /// player 0 is keyboard/mouse/controller/gamepad/joystick
    /// players 1.. are the other controllers/gamepads/joysticks
    fn get_input_vec(&self, x_input: AxisInputs, y_input: AxisInputs, player: usize) -> Vec2 {
        let mut neg_x = 0.0;
        let mut pos_x = 0.0;
        let mut neg_y = 0.0;
        let mut pos_y = 0.0;
        if let Some(gamepad) = self.get_gamepad(player) {
            if x_input.2 {
                pos_x += gamepad.get_value(x_input.1).unwrap_or(0.0);
            } else {
                neg_x -= gamepad.get_value(x_input.0).unwrap_or(0.0);
                pos_x += gamepad.get_value(x_input.1).unwrap_or(0.0);
            }
            if y_input.2 {
                pos_y += gamepad.get_value(y_input.1).unwrap_or(0.0);
            } else {
                neg_y -= gamepad.get_value(y_input.0).unwrap_or(0.0);
                pos_y += gamepad.get_value(y_input.1).unwrap_or(0.0);
            }
        }
        if player == 0 {
            neg_x -= Self::btof(self.key_held(x_input.0));
            pos_x += Self::btof(self.key_held(x_input.1));
            neg_y -= Self::btof(self.key_held(y_input.0));
            pos_y += Self::btof(self.key_held(y_input.1));
        }
        Vec2::new(neg_x + pos_x, neg_y + pos_y)
    }

    /// player 0 is keyboard/mouse
    /// players 1.. are controllers/gamepads/joysticks
    pub fn get_axis(&self, input: InputAxis, player: usize) -> Vec2 {
        match input {
            InputAxis::Move => self.get_input_vec(
                (Input::MoveLeft, Input::MoveRight, true),
                (Input::MoveDown, Input::MoveUp, true),
                player,
            ),
            InputAxis::Look => self.get_input_vec(
                (Input::LookLeft, Input::LookRight, true),
                (Input::LookDown, Input::LookUp, true),
                player,
            ),
            InputAxis::Roll => self.get_input_vec(
                (Input::RollLeft, Input::RollRight, true),
                (Input::RollDown, Input::RollUp, true),
                player,
            ),
            InputAxis::Trigger => self.get_input_vec(
                (Input::Decelerate, Input::Accelerate, false),
                (Input::Undefined, Input::Undefined, true),
                player,
            ),
            InputAxis::ZMove => self.get_input_vec(
                (Input::Crouch, Input::Jump, false),
                (Input::Undefined, Input::Undefined, true),
                player,
            ),
        }
    }

    pub fn should_close(&self) -> bool {
        self.should_close
    }

    //

    fn gamepad_entry(&mut self, id: GamepadId) -> &'_ mut Gamepad {
        match self.gamepads.entry(id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let player = self.players.len();
                self.players.push(Some(id));
                entry.insert(Gamepad {
                    player,
                    ..Default::default()
                })
            }
        }
    }

    fn get_gamepad(&self, player: usize) -> Option<&'_ Gamepad> {
        self.players
            .get(player)
            .and_then(|gamepad| gamepad.as_ref())
            .and_then(|gamepad| self.gamepads.get(gamepad))
    }

    fn btof(b: bool) -> f32 {
        if b {
            1.0
        } else {
            0.0
        }
    }

    // modified deadzone filter from gilrs
    pub(crate) fn deadzone(ev: Option<GilrsEvent>, gilrs: &mut Gilrs) -> Option<GilrsEvent> {
        match ev {
            Some(GilrsEvent {
                event: EventType::AxisChanged(axis, val, nec),
                id,
                time,
            }) => {
                let mut threshold = match gilrs.gamepad(id).deadzone(nec) {
                    Some(t) => t,
                    None => return ev,
                };
                if threshold == 0.0 {
                    threshold = 0.2
                }

                if val.abs() < threshold {
                    Some(GilrsEvent {
                        id,
                        time,
                        event: EventType::AxisChanged(axis, 0.0, nec),
                    })
                } else {
                    Some(GilrsEvent {
                        id,
                        time,
                        event: EventType::AxisChanged(axis, val, nec),
                    })
                }
            }
            Some(GilrsEvent {
                event: EventType::ButtonChanged(button, val, nec),
                id,
                time,
            }) => {
                let mut threshold = match gilrs.gamepad(id).deadzone(nec) {
                    Some(t) => t,
                    None => return ev,
                };
                if threshold == 0.0 {
                    threshold = 0.2
                }

                if val.abs() < threshold {
                    Some(GilrsEvent {
                        id,
                        time,
                        event: EventType::ButtonChanged(button, 0.0, nec),
                    })
                } else {
                    Some(GilrsEvent {
                        id,
                        time,
                        event: EventType::ButtonChanged(button, val, nec),
                    })
                }
            }
            _ => ev,
        }
    }
}

//

type AxisInputs = (Input, Input, bool);
