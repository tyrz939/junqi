//! SDL events into a `DeviceState` (PRESENTATION.md §4). The only file that knows what an SDL key,
//! button or pad is; what leaves it is plain numbers in `jane_present::input`'s shapes.
//!
//! Pads: every game controller SDL knows is opened, at boot and as it is plugged in (hot-plug),
//! and a log line names it. Their buttons and axes come from the controller API (the standard
//! mapping, never a raw joystick), from the events and from a poll each frame, and every open pad
//! is folded into one: whichever pad she picks up works. A joystick SDL has no mapping for is
//! named in the log with its GUID, so a line for it can go in `gamecontrollerdb.txt`.

use std::path::Path;

use jane_present::input::{DeviceState, MouseButton, Pad};
use sdl2::GameControllerSubsystem;
use sdl2::controller::{Axis, GameController};
use sdl2::event::{Event, WindowEvent};
use sdl2::mouse::MouseButton as SdlButton;

/// The file of extra pad mappings (SDL's `gamecontrollerdb.txt` format) read at boot, from beside
/// the saves and beside the exe.
pub const MAPPINGS_FILE: &str = "gamecontrollerdb.txt";

/// What an event meant to the loop, beyond the device state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happened {
    Nothing,
    Quit,
    /// The window's size changed: the canvas and the texture follow.
    Resized,
}

/// One open pad: its handle (`None` for a test's stand-in), what its events said, and the buttons
/// that went down since the last sample, so a tap shorter than a frame is seen.
struct OpenPad {
    id: u32,
    handle: Option<GameController>,
    pad: Pad,
    tapped: u32,
}

/// The devices: the state the mapper reads, and the pads in use.
pub struct Devices {
    pub state: DeviceState,
    sub: Option<GameControllerSubsystem>,
    pads: Vec<OpenPad>,
    /// How the canvas lies on the window, for window px to canvas px.
    fit: jane_present::input::Fit,
}

impl std::fmt::Debug for Devices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Devices").field("state", &self.state).field("pads", &self.pads.len()).finish_non_exhaustive()
    }
}

/// Hints SDL reads when its pad subsystem starts; set before `sdl2::init`. An environment
/// variable of the same name still wins (SDL's own rule), so a player can turn one back.
pub fn pad_hints() {
    // A pad held while the window is behind another (a guide open, a second monitor) still plays.
    sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    // Windows: Xbox pads through XInput, the oldest and surest path for a 360 pad; SDL 2.26's raw
    // input driver can take the pad and then hear nothing from it.
    #[cfg(windows)]
    sdl2::hint::set("SDL_JOYSTICK_RAWINPUT", "0");
}

impl Devices {
    /// `pads` is `None` when the game controller subsystem would not start: no pad, no harm.
    /// `mappings` are extra mapping files to read first (each only if it is there).
    pub fn new(pads: Option<GameControllerSubsystem>, fit: jane_present::input::Fit, mappings: &[&Path]) -> Devices {
        let mut d = Devices { state: DeviceState::default(), sub: pads, pads: Vec::new(), fit };
        if let Some(sub) = &d.sub {
            for p in mappings.iter().filter(|p| p.is_file()) {
                match sub.load_mappings(p) {
                    Ok(n) => println!("jane-app: {n} pad mapping(s) from {}", p.display()),
                    Err(e) => eprintln!("jane-app: pad mappings {}: {e}", p.display()),
                }
            }
            let n = sub.num_joysticks().unwrap_or(0);
            if n == 0 {
                println!("jane-app: no pad yet (plug one in at any time)");
            }
            for i in 0..n {
                d.open(i);
            }
        }
        d
    }

    /// The canvas moved on the window (a resize, or the `fill` row turned).
    pub fn set_fit(&mut self, fit: jane_present::input::Fit) {
        self.fit = fit;
    }

    /// Opens the controller at device index `index` if SDL maps it and it is not open yet.
    fn open(&mut self, index: u32) {
        let Some(sub) = &self.sub else { return };
        if !sub.is_game_controller(index) {
            let name = sub.name_for_index(index).unwrap_or_default();
            eprintln!(
                "jane-app: a joystick SDL has no pad mapping for: {name:?}; a line for it in {MAPPINGS_FILE} beside \
                 the saves makes it a pad"
            );
            return;
        }
        match sub.open(index) {
            Ok(c) => {
                let id = c.instance_id();
                if self.pads.iter().any(|p| p.id == id) {
                    return;
                }
                println!("jane-app: pad {}: {}", self.pads.len() + 1, c.name());
                self.pads.push(OpenPad { id, handle: Some(c), pad: Pad::default(), tapped: 0 });
            }
            Err(e) => eprintln!("jane-app: pad at {index} would not open: {e}"),
        }
    }

    fn cursor(&mut self, x: i32, y: i32) {
        self.state.mouse.pos = Some(self.fit.to_canvas(x, y));
    }

    fn pad_mut(&mut self, id: u32) -> Option<&mut OpenPad> {
        self.pads.iter_mut().find(|p| p.id == id)
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
            // Hot-plug: `which` is a device index here, an instance id everywhere after.
            Event::ControllerDeviceAdded { which, .. } => self.open(which),
            Event::JoyDeviceAdded { which, .. } => {
                // A controller comes as both; only an unmapped joystick needs saying here.
                if self.sub.as_ref().is_some_and(|s| !s.is_game_controller(which)) {
                    self.open(which);
                }
            }
            Event::ControllerDeviceRemoved { which, .. } => {
                if let Some(i) = self.pads.iter().position(|p| p.id == which) {
                    let p = self.pads.remove(i);
                    let name = p.handle.as_ref().map_or_else(String::new, GameController::name);
                    println!("jane-app: pad gone: {name}");
                }
            }
            Event::ControllerAxisMotion { which, axis, value, .. } => {
                if let Some(p) = self.pad_mut(which) {
                    p.pad.axes[axis as usize % 6] = value;
                }
            }
            Event::ControllerButtonDown { which, button, .. } => {
                if let Some(p) = self.pad_mut(which) {
                    let bit = 1 << (button as i32 & 31);
                    p.pad.held |= bit;
                    p.tapped |= bit;
                }
            }
            Event::ControllerButtonUp { which, button, .. } => {
                if let Some(p) = self.pad_mut(which) {
                    p.pad.held &= !(1 << (button as i32 & 31));
                }
            }
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::FocusLost => self.state.release_all(),
                // The pointer left the window: no reticle, no arrow, until it comes back.
                WindowEvent::Leave => self.state.mouse.pos = None,
                WindowEvent::SizeChanged(..) | WindowEvent::Resized(..) => return Happened::Resized,
                _ => {}
            },
            _ => {}
        }
        Happened::Nothing
    }

    /// Read the pads into the state; call once a frame after the events, before sampling. Every
    /// open pad folds into one: buttons held on any, each axis from the pad pushing it furthest.
    pub fn poll_pad(&mut self) {
        let mut merged: Option<Pad> = None;
        for p in &mut self.pads {
            if let Some(c) = &p.handle {
                p.pad.axes =
                    [Axis::LeftX, Axis::LeftY, Axis::RightX, Axis::RightY, Axis::TriggerLeft, Axis::TriggerRight]
                        .map(|a| c.axis(a));
                p.pad.held = PAD_BUTTONS.iter().filter(|&&b| c.button(b)).fold(0, |m, &b| m | 1 << (b as i32 & 31));
            }
            let m = merged.get_or_insert_with(Pad::default);
            m.held |= p.pad.held | p.tapped;
            for (a, v) in m.axes.iter_mut().zip(p.pad.axes) {
                if v.unsigned_abs() > a.unsigned_abs() {
                    *a = v;
                }
            }
            p.tapped = 0;
        }
        self.state.pad = merged;
    }

    /// A pad with no SDL behind it, as a test's events address it.
    #[cfg(test)]
    fn stand_in(&mut self, id: u32) {
        self.pads.push(OpenPad { id, handle: None, pad: Pad::default(), tapped: 0 });
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
    use jane_present::input::{Context, Edge, GameAction, Input, Mode, UiAction, pad, sc};
    use sdl2::controller::{Axis, Button};
    use sdl2::event::Event;

    use super::{Devices, PAD_BUTTONS};

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
        // Axes in the order `Pad::axes` keeps them.
        for (i, a) in [Axis::LeftX, Axis::LeftY, Axis::RightX, Axis::RightY, Axis::TriggerLeft, Axis::TriggerRight]
            .iter()
            .enumerate()
        {
            assert_eq!(*a as usize, i, "{a:?}");
        }
    }

    fn devices() -> Devices {
        Devices::new(None, jane_present::input::fit(1280, 720, jane_present::input::Scaling::Whole), &[])
    }

    fn down(which: u32, button: Button) -> Event {
        Event::ControllerButtonDown { timestamp: 0, which, button }
    }

    fn up(which: u32, button: Button) -> Event {
        Event::ControllerButtonUp { timestamp: 0, which, button }
    }

    fn axis(which: u32, axis: Axis, value: i16) -> Event {
        Event::ControllerAxisMotion { timestamp: 0, which, axis, value }
    }

    /// Synthetic SDL controller events, through the devices and the mapper, are her moves and
    /// presses: the path a real pad's events take.
    #[test]
    fn pad_events_reach_the_mapper() {
        let mut d = devices();
        d.poll_pad();
        assert_eq!(d.state.pad, None, "no pad, no pad state");
        d.stand_in(7);
        let mut input = Input::new();
        let play = Context { mode: Mode::Play, feet: Some((320.0, 180.0)) };
        // A tap shorter than a frame: down and up before the poll still presses A (bar 1).
        d.event(&down(7, Button::A));
        d.event(&up(7, Button::A));
        // Another pad's events (not open) are nobody's.
        d.event(&down(99, Button::Y));
        d.poll_pad();
        let _ = input.sample(&d.state, &play);
        d.state.end_sample();
        let edges: Vec<Edge> = input.drain().collect();
        assert_eq!(edges, vec![Edge::Game(GameAction::Bar(0))]);
        // The left stick walks; the right stick aims.
        d.event(&axis(7, Axis::LeftX, 32767));
        d.event(&axis(7, Axis::RightY, -32767));
        d.poll_pad();
        let f = input.sample(&d.state, &play);
        d.state.end_sample();
        assert_eq!(f.mv_mag, 127);
        assert!(f.aim.is_some() && input.pad_active() && !input.aiming_with_mouse());
        // B in a screen goes back; the stick steps through a menu.
        d.event(&axis(7, Axis::LeftX, 0));
        d.event(&axis(7, Axis::RightY, 0));
        d.event(&down(7, Button::B));
        d.poll_pad();
        let _ = input.sample(&d.state, &Context { mode: Mode::Ui, feet: None });
        d.state.end_sample();
        assert!(input.drain().any(|e| e == Edge::Ui(UiAction::Cancel)));
        // Unplugged: gone, and nothing held is left behind.
        d.event(&Event::ControllerDeviceRemoved { timestamp: 0, which: 7 });
        d.poll_pad();
        assert_eq!(d.state.pad, None);
    }

    /// Two pads fold into one: either one plays.
    #[test]
    fn every_open_pad_plays() {
        let mut d = devices();
        d.stand_in(1);
        d.stand_in(2);
        d.event(&down(2, Button::X));
        d.event(&axis(1, Axis::LeftY, -20_000));
        d.event(&axis(2, Axis::LeftY, 9_000));
        d.poll_pad();
        let p = d.state.pad.unwrap();
        assert!(p.is_held(pad::X));
        assert_eq!(p.axes[1], -20_000, "the pad pushed furthest");
    }

    /// SDL's own table maps an XInput pad (every Xbox 360 and later pad on Windows) to the
    /// standard layout our bindings name: A is button 0, the triggers are axes.
    #[test]
    fn sdl_maps_an_xinput_pad() {
        let Ok(sdl) = sdl2::init() else { return };
        let Ok(sub) = sdl.game_controller() else { return };
        // An Xbox 360 pad as SDL's XInput driver names it (Microsoft's vendor and the 360's
        // product, 'x' in byte 14), and any other pad on XInput.
        for g in ["030000005e0400008e02000000007800", "03000000ffff0000ffff000000007800"] {
            let guid = sdl2::joystick::Guid::from_string(g).unwrap();
            let m = sub.mapping_for_guid(guid).expect("an XInput mapping");
            for part in [",a:b0,", ",b:b1,", "lefttrigger:a2", "righttrigger:a5", "leftx:a0", "start:b7"] {
                assert!(m.contains(part), "{part} in {m}");
            }
        }
        // A line the player adds is taken.
        let line = "03000000ffff00001234000000000000,Jane Test Pad,a:b0,b:b1,x:b2,y:b3,leftx:a0,lefty:a1,";
        assert!(sub.add_mapping(line).is_ok());
    }
}
