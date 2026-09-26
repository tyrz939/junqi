//! SDL events into a `DeviceState` (PRESENTATION.md §4). The only file that knows what an SDL key,
//! button or pad is; what leaves it is plain numbers in `jane_present::input`'s shapes.

use jane_present::input::{DeviceState, MouseButton, Pad};
use sdl2::GameControllerSubsystem;
use sdl2::controller::{Axis, GameController};
use sdl2::event::{Event, WindowEvent};
use sdl2::mouse::MouseButton as SdlButton;

/// What an event meant to the loop, beyond the device state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happened {
    Nothing,
    Quit,
    /// The window's size changed: the canvas and the texture follow.
    Resized,
}

/// The devices: the state the mapper reads, and the one pad in use.
pub struct Devices {
    pub state: DeviceState,
    pads: Option<GameControllerSubsystem>,
    pad: Option<GameController>,
    /// Pad buttons that went down since the last sample, so a tap shorter than a frame is seen.
    pad_tapped: u32,
    /// The window's height in px, for window px to canvas px.
    win_h: u32,
}

impl std::fmt::Debug for Devices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Devices").field("state", &self.state).field("pad", &self.pad.is_some()).finish_non_exhaustive()
    }
}

impl Devices {
    /// `pads` is `None` when the game controller subsystem would not start: no pad, no harm.
    pub fn new(pads: Option<GameControllerSubsystem>, win_h: u32) -> Devices {
        let mut d = Devices { state: DeviceState::default(), pads, pad: None, pad_tapped: 0, win_h };
        d.open_first_pad();
        d
    }

    pub fn set_window_height(&mut self, h: u32) {
        self.win_h = h;
    }

    fn open_first_pad(&mut self) {
        let Some(sub) = &self.pads else { return };
        let n = sub.num_joysticks().unwrap_or(0);
        self.pad = (0..n).filter(|&i| sub.is_game_controller(i)).find_map(|i| sub.open(i).ok());
    }

    fn cursor(&mut self, x: i32, y: i32) {
        self.state.mouse.pos = Some(jane_present::input::to_canvas(x, y, self.win_h));
    }

    /// Fold one event in.
    pub fn event(&mut self, e: &Event) -> Happened {
        match *e {
            Event::Quit { .. } => return Happened::Quit,
            Event::KeyDown { scancode: Some(sc), repeat: false, .. } => self.state.key(sc as i32 as u16, true),
            Event::KeyUp { scancode: Some(sc), .. } => self.state.key(sc as i32 as u16, false),
            Event::MouseMotion { x, y, .. } => {
                self.cursor(x, y);
                self.state.mouse.moved = true;
            }
            Event::MouseButtonDown { mouse_btn, x, y, .. } | Event::MouseButtonUp { mouse_btn, x, y, .. } => {
                let down = matches!(e, Event::MouseButtonDown { .. });
                self.cursor(x, y);
                if let Some(b) = button(mouse_btn) {
                    self.state.button(b, down);
                }
            }
            Event::MouseWheel { y, .. } => self.state.mouse.wheel += y,
            Event::ControllerDeviceAdded { .. } if self.pad.is_none() => self.open_first_pad(),
            Event::ControllerDeviceRemoved { which, .. }
                if self.pad.as_ref().is_some_and(|p| p.instance_id() == which) =>
            {
                self.pad = None;
                self.open_first_pad();
            }
            Event::ControllerButtonDown { button, .. } => self.pad_tapped |= 1 << (button as i32 & 31),
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::FocusLost => self.state.release_all(),
                WindowEvent::Leave => self.state.mouse.pos = None,
                WindowEvent::SizeChanged(..) | WindowEvent::Resized(..) => return Happened::Resized,
                _ => {}
            },
            _ => {}
        }
        Happened::Nothing
    }

    /// Read the pad into the state; call once a frame after the events, before sampling.
    pub fn poll_pad(&mut self) {
        self.state.pad = self.pad.as_ref().map(|c| {
            let axes = [Axis::LeftX, Axis::LeftY, Axis::RightX, Axis::RightY, Axis::TriggerLeft, Axis::TriggerRight]
                .map(|a| c.axis(a));
            let mut held = self.pad_tapped;
            for b in PAD_BUTTONS {
                if c.button(b) {
                    held |= 1 << (b as i32 & 31);
                }
            }
            Pad { axes, held }
        });
        self.pad_tapped = 0;
    }
}

/// The standard mapping's buttons in `SDL_GameControllerButton` order, which is
/// `jane_present::input::pad`'s bit order.
const PAD_BUTTONS: [sdl2::controller::Button; 15] = {
    use sdl2::controller::Button as B;
    [
        B::A,
        B::B,
        B::X,
        B::Y,
        B::Back,
        B::Guide,
        B::Start,
        B::LeftStick,
        B::RightStick,
        B::LeftShoulder,
        B::RightShoulder,
        B::DPadUp,
        B::DPadDown,
        B::DPadLeft,
        B::DPadRight,
    ]
};

fn button(b: SdlButton) -> Option<MouseButton> {
    match b {
        SdlButton::Left => Some(MouseButton::Left),
        SdlButton::Middle => Some(MouseButton::Middle),
        SdlButton::Right => Some(MouseButton::Right),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use jane_present::input::{pad, sc};

    use super::PAD_BUTTONS;

    #[test]
    fn sdl_numbers_are_the_mappers_numbers() {
        use sdl2::keyboard::Scancode;
        for (s, n) in [
            (Scancode::W, sc::W),
            (Scancode::Num1, sc::N1),
            (Scancode::Escape, sc::ESCAPE),
            (Scancode::Grave, sc::GRAVE),
            (Scancode::F12, sc::F12),
            (Scancode::Up, sc::UP),
            (Scancode::LShift, sc::LSHIFT),
            (Scancode::RShift, sc::RSHIFT),
        ] {
            assert_eq!(s as i32 as u16, n, "{s:?}");
        }
        for (i, b) in PAD_BUTTONS.iter().enumerate() {
            assert_eq!(*b as i32, i as i32, "{b:?}");
        }
        assert_eq!(sdl2::controller::Button::DPadRight as i32, i32::from(pad::DPAD_RIGHT));
        assert_eq!(sdl2::controller::Button::Start as i32, i32::from(pad::START));
    }
}
