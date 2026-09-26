//! Input mapping (PRESENTATION.md §4): device state in, one [`InputFrame`] and a queue of
//! [`Edge`]s out. Nothing downstream knows which device it was (ENGINE.md §10).
//!
//! The app turns SDL events into a [`DeviceState`] (keys by SDL scancode number, the mouse
//! already in canvas px, a pad in the standard mapping) and never hands this module an SDL
//! type; a test fills the same struct by hand. Floats are allowed here and never leave: move and
//! aim are quantised to `(Angle, magnitude)` before they become an `InputFrame`, and the raw aim
//! goes out unbent (the sim does the assist, ARCHITECTURE.md §5.4).
//!
//! Bindings are the const [`BINDINGS`] table for now. `data/bindings.json`, compiled in with the
//! rest of the content, and the per-user overrides in `config.json` replace it at P7; the table
//! has the shape the data will have (`Binding { action, keys: [Scancode; 2], mouse, pad }`).

use jane_core::angle::iatan2;
use jane_core::{Angle, Fx, Rect, Vec2};
use jane_sim::UnitId;
use jane_sim::input::{AssistProfile, InputFrame};
use jane_sim::view::View;

use crate::frame::CANVAS_H;

/// SDL scancode numbers (USB HID usage ids), the ones the table names. No SDL type: the app
/// passes `scancode as u16`.
pub mod sc {
    pub const A: u16 = 4;
    pub const D: u16 = 7;
    pub const E: u16 = 8;
    pub const F: u16 = 9;
    pub const I: u16 = 12;
    pub const J: u16 = 13;
    pub const K: u16 = 14;
    pub const M: u16 = 16;
    pub const S: u16 = 22;
    pub const W: u16 = 26;
    /// `1`; `2` to `9` follow, then `0`.
    pub const N1: u16 = 30;
    pub const RETURN: u16 = 40;
    pub const ESCAPE: u16 = 41;
    pub const TAB: u16 = 43;
    pub const SPACE: u16 = 44;
    /// The key left of `1` on a US board.
    pub const GRAVE: u16 = 53;
    /// `F1`; `F2` to `F12` follow.
    pub const F1: u16 = 58;
    pub const F2: u16 = 59;
    pub const F3: u16 = 60;
    pub const F5: u16 = 62;
    pub const F9: u16 = 66;
    pub const F12: u16 = 69;
    pub const RIGHT: u16 = 79;
    pub const LEFT: u16 = 80;
    pub const DOWN: u16 = 81;
    pub const UP: u16 = 82;
    pub const KP_ENTER: u16 = 88;
    pub const LSHIFT: u16 = 225;
    pub const RSHIFT: u16 = 229;
}

/// Pad buttons in the standard mapping's order (`SDL_GameControllerButton`), as bit numbers of
/// [`Pad::held`].
pub mod pad {
    pub const A: u8 = 0;
    pub const B: u8 = 1;
    pub const X: u8 = 2;
    pub const Y: u8 = 3;
    /// "View" on an Xbox pad.
    pub const BACK: u8 = 4;
    pub const GUIDE: u8 = 5;
    /// "Menu" on an Xbox pad.
    pub const START: u8 = 6;
    pub const LSTICK: u8 = 7;
    pub const RSTICK: u8 = 8;
    pub const LB: u8 = 9;
    pub const RB: u8 = 10;
    pub const DPAD_UP: u8 = 11;
    pub const DPAD_DOWN: u8 = 12;
    pub const DPAD_LEFT: u8 = 13;
    pub const DPAD_RIGHT: u8 = 14;
}

/// How many scancodes a [`KeySet`] holds.
pub const KEYS: usize = 512;

/// A set of held (or just pressed) scancodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeySet([u64; KEYS / 64]);

impl KeySet {
    pub const EMPTY: KeySet = KeySet([0; KEYS / 64]);

    /// Out-of-range codes are ignored.
    pub fn set(&mut self, code: u16, on: bool) {
        let c = usize::from(code);
        if c >= KEYS {
            return;
        }
        let bit = 1u64 << (c % 64);
        if on {
            self.0[c / 64] |= bit;
        } else {
            self.0[c / 64] &= !bit;
        }
    }

    pub fn has(&self, code: u16) -> bool {
        let c = usize::from(code);
        c < KEYS && self.0[c / 64] & (1u64 << (c % 64)) != 0
    }

    pub fn clear(&mut self) {
        *self = KeySet::EMPTY;
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    /// The codes in the set, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        (0..KEYS as u16).filter(|&c| self.has(c))
    }
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    const fn bit(self) -> u8 {
        match self {
            MouseButton::Left => 1,
            MouseButton::Middle => 2,
            MouseButton::Right => 4,
        }
    }
}

/// The mouse, in canvas px (the app divides window px by the window's scale, [`to_canvas`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mouse {
    /// The cursor in canvas px; `None` while it is outside the window.
    pub pos: Option<(f32, f32)>,
    /// Buttons held, by [`MouseButton`] bit.
    pub held: u8,
    /// Buttons pressed since the last sample (a click shorter than a frame still counts).
    pub pressed: u8,
    /// Wheel notches since the last sample, up positive.
    pub wheel: i32,
    /// The cursor moved (or a button went down) since the last sample: the mouse is aiming.
    pub moved: bool,
}

impl Mouse {
    pub fn is_held(&self, b: MouseButton) -> bool {
        self.held & b.bit() != 0
    }

    pub fn was_pressed(&self, b: MouseButton) -> bool {
        self.pressed & b.bit() != 0
    }
}

/// A pad in the standard mapping. Axes in `SDL_GameControllerAxis` order: left x, left y, right
/// x, right y (-32768..=32767, y down), left trigger, right trigger (0..=32767).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    pub axes: [i16; 6],
    /// Buttons held, bit [`pad`]`::*`.
    pub held: u32,
}

impl Pad {
    pub const fn is_held(&self, b: u8) -> bool {
        self.held & (1 << b) != 0
    }
}

/// Everything the devices say this frame. The app fills it from events; a test by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceState {
    /// Keys held.
    pub keys: KeySet,
    /// Keys that went down since the last sample (not OS repeats).
    pub pressed: KeySet,
    pub mouse: Mouse,
    pub pad: Option<Pad>,
}

impl DeviceState {
    /// A key went down or up.
    pub fn key(&mut self, code: u16, down: bool) {
        if down && !self.keys.has(code) {
            self.pressed.set(code, true);
        }
        self.keys.set(code, down);
    }

    /// A mouse button went down or up.
    pub fn button(&mut self, b: MouseButton, down: bool) {
        if down {
            self.mouse.held |= b.bit();
            self.mouse.pressed |= b.bit();
            self.mouse.moved = true;
        } else {
            self.mouse.held &= !b.bit();
        }
    }

    /// Focus went elsewhere: nothing is held any more (the key-ups will go to another window).
    pub fn release_all(&mut self) {
        self.keys.clear();
        self.mouse.held = 0;
    }

    /// After a sample: the presses and the wheel have been read.
    pub fn end_sample(&mut self) {
        self.pressed.clear();
        self.mouse.pressed = 0;
        self.mouse.wheel = 0;
        self.mouse.moved = false;
    }
}

/// What a binding does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Sprint,
    /// Use, talk; held: push and pull.
    Use,
    /// A bar slot, `0..8`.
    Bar(u8),
    Bags,
    Book,
    Quests,
    Map,
    /// Pause, back.
    Pause,
    QuickSave,
    QuickLoad,
    Console,
    /// The debug overlay.
    Debug,
    /// The path grid.
    Grid,
    /// The canvas to a PNG.
    Shot,
}

/// A pad input a binding can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadInput {
    Button(u8),
    LeftTrigger,
    RightTrigger,
}

/// One row of the table: up to two keys (0 is none), a mouse button, a pad input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub action: Action,
    pub keys: [u16; 2],
    pub mouse: Option<MouseButton>,
    pub pad: Option<PadInput>,
}

const fn row(action: Action, keys: [u16; 2], mouse: Option<MouseButton>, pad: Option<PadInput>) -> Binding {
    Binding { action, keys, mouse, pad }
}

#[allow(clippy::unnecessary_wraps)] // the table's `pad` column is an Option; this fills it
const fn btn(b: u8) -> Option<PadInput> {
    Some(PadInput::Button(b))
}

/// The bindings (README's Controls table; the pad column is the 2020 layout). Stands in for
/// `data/bindings.json` until P7 compiles that in.
pub const BINDINGS: &[Binding] = &[
    row(Action::Up, [sc::W, sc::UP], None, btn(pad::DPAD_UP)),
    row(Action::Down, [sc::S, sc::DOWN], None, btn(pad::DPAD_DOWN)),
    row(Action::Left, [sc::A, sc::LEFT], None, btn(pad::DPAD_LEFT)),
    row(Action::Right, [sc::D, sc::RIGHT], None, btn(pad::DPAD_RIGHT)),
    row(Action::Sprint, [sc::LSHIFT, sc::RSHIFT], None, Some(PadInput::RightTrigger)),
    row(Action::Use, [sc::E, sc::F], None, btn(pad::B)),
    row(Action::Bar(0), [sc::N1, sc::SPACE], Some(MouseButton::Left), btn(pad::A)),
    row(Action::Bar(1), [sc::N1 + 1, 0], None, btn(pad::X)),
    row(Action::Bar(2), [sc::N1 + 2, 0], None, btn(pad::Y)),
    row(Action::Bar(3), [sc::N1 + 3, 0], None, btn(pad::LB)),
    row(Action::Bar(4), [sc::N1 + 4, 0], None, btn(pad::RB)),
    row(Action::Bar(5), [sc::N1 + 5, 0], None, None),
    row(Action::Bar(6), [sc::N1 + 6, 0], None, None),
    row(Action::Bar(7), [sc::N1 + 7, 0], None, None),
    row(Action::Bags, [sc::TAB, sc::I], None, btn(pad::BACK)),
    row(Action::Book, [sc::K, 0], None, None),
    row(Action::Quests, [sc::J, 0], None, None),
    row(Action::Map, [sc::M, 0], None, None),
    row(Action::Pause, [sc::ESCAPE, 0], None, btn(pad::START)),
    row(Action::QuickSave, [sc::F5, 0], None, None),
    row(Action::QuickLoad, [sc::F9, 0], None, None),
    row(Action::Console, [sc::GRAVE, 0], None, None),
    row(Action::Debug, [sc::F2, 0], None, None),
    row(Action::Grid, [sc::F3, 0], None, None),
    row(Action::Shot, [sc::F12, 0], None, None),
];

/// What the UI and the app hear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    Pause,
    /// Back out of whatever is open (Esc or B while a screen is up).
    Cancel,
    Confirm,
    Up,
    Down,
    Left,
    Right,
    TabLeft,
    TabRight,
    Bags,
    Book,
    Quests,
    Map,
    Console,
    Debug,
    Grid,
    Shot,
    QuickSave,
    QuickLoad,
}

/// What becomes a sim `Command` (the app stamps it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameAction {
    /// `Command::Use`.
    Use,
    /// `Command::Bar { slot }`, `0..8`.
    Bar(u8),
}

/// A press, queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Ui(UiAction),
    Game(GameAction),
}

/// Who has the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// She does: move, use, the bar.
    #[default]
    Play,
    /// A screen does (pause, bags, a menu): the keys navigate and she stands still.
    Ui,
    /// A text field does (the console): nothing is held; only Esc and the console key get out.
    Text,
}

/// What the mapper needs from the scene this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Context {
    pub mode: Mode,
    /// Her feet in canvas px, if she is drawn ([`world_to_canvas`]).
    pub feet: Option<(f32, f32)>,
}

/// Aim comes from her chest, this far above her feet, in canvas px.
pub const CHEST_PX: f32 = 12.0;
/// Right button held walks toward the cursor at full tilt from this many canvas px out.
pub const WALK_FULL_PX: f32 = 64.0;
/// The cursor must be this far from her chest, in canvas px, to aim.
pub const AIM_MIN_PX: f32 = 2.0;
/// Left stick, radial (2020's value).
pub const STICK_DEADZONE: f32 = 0.2;
/// Right stick aims past this.
pub const AIM_DEADZONE: f32 = 0.35;
/// A trigger counts as held past this.
pub const TRIGGER_HELD: i16 = 16_384;

/// The device state mapper. Keeps what edges need from the last sample (the pad's buttons) and
/// which device aimed last.
#[derive(Clone, Debug, Default)]
pub struct Input {
    edges: Vec<Edge>,
    pad_was: u32,
    rt_was: bool,
    lt_was: bool,
    aim_pad: bool,
    pad_last: bool,
    /// The Controls row's assist profile; `None`: `Pad` while the pad aims, `Off` for a mouse.
    pub assist: Option<AssistProfile>,
}

impl Input {
    pub fn new() -> Input {
        Input::default()
    }

    /// The held frame for this sample, with this sample's presses queued as edges. The app
    /// calls it once a frame and then [`DeviceState::end_sample`].
    pub fn sample(&mut self, dev: &DeviceState, ctx: &Context) -> InputFrame {
        self.key_edges(dev, ctx.mode);
        self.mouse_edges(dev, ctx.mode);
        self.pad_edges(dev.pad.as_ref(), ctx.mode);
        if dev.mouse.moved {
            self.aim_pad = false;
        }
        if ctx.mode != Mode::Play {
            return InputFrame { assist: self.profile(), ..InputFrame::IDLE };
        }
        let held = |a: Action| is_held(dev, a);
        let (mut mx, mut my) = (0.0f32, 0.0f32);
        if held(Action::Left) {
            mx -= 1.0;
        }
        if held(Action::Right) {
            mx += 1.0;
        }
        if held(Action::Up) {
            my -= 1.0;
        }
        if held(Action::Down) {
            my += 1.0;
        }
        if mx == 0.0
            && my == 0.0
            && let Some(p) = &dev.pad
        {
            let (lx, ly) = (axis(p.axes[0]), axis(p.axes[1]));
            if lx.hypot(ly) > STICK_DEADZONE {
                (mx, my) = (lx, ly);
            }
        }
        // The aim: the right stick when it is out, else the cursor from her chest.
        let chest_to_cursor = match (ctx.feet, dev.mouse.pos) {
            (Some((fx, fy)), Some((cx, cy))) => Some((cx - fx, cy - (fy - CHEST_PX))),
            _ => None,
        };
        let mut aim = None;
        if let Some(p) = &dev.pad {
            let (rx, ry) = (axis(p.axes[2]), axis(p.axes[3]));
            if rx.hypot(ry) > AIM_DEADZONE {
                self.aim_pad = true;
                aim = Some(angle_of(rx, ry));
            }
        }
        if !self.aim_pad
            && let Some((dx, dy)) = chest_to_cursor
        {
            let len = dx.hypot(dy);
            if len > AIM_MIN_PX {
                aim = Some(angle_of(dx, dy));
                // 2020's virtual stick: right button held walks toward the cursor.
                if mx == 0.0 && my == 0.0 && dev.mouse.is_held(MouseButton::Right) {
                    let s = (len / WALK_FULL_PX).min(1.0);
                    (mx, my) = (dx / len * s, dy / len * s);
                }
            }
        }
        let (mv_dir, mv_mag) = quantise(mx, my);
        InputFrame {
            mv_dir,
            mv_mag,
            aim,
            sprint: held(Action::Sprint),
            use_held: held(Action::Use),
            assist: self.profile(),
        }
    }

    /// The edges queued since the last drain, in the order they were pressed within a sample.
    pub fn drain(&mut self) -> std::vec::Drain<'_, Edge> {
        self.edges.drain(..)
    }

    /// The mouse aims (draw the reticle, hide the OS cursor); else the pad does.
    pub fn aiming_with_mouse(&self) -> bool {
        !self.aim_pad
    }

    /// The last thing touched was the pad: hints show pad glyphs.
    pub fn pad_active(&self) -> bool {
        self.pad_last
    }

    fn profile(&self) -> AssistProfile {
        self.assist.unwrap_or(if self.aim_pad { AssistProfile::Pad } else { AssistProfile::Off })
    }

    fn key_edges(&mut self, dev: &DeviceState, mode: Mode) {
        if dev.pressed.is_empty() {
            return;
        }
        self.pad_last = false;
        for code in dev.pressed.iter() {
            if mode != Mode::Text && (code == sc::RETURN || code == sc::KP_ENTER) {
                if mode == Mode::Ui {
                    self.edges.push(Edge::Ui(UiAction::Confirm));
                }
                continue;
            }
            for b in BINDINGS.iter().filter(|b| code != 0 && b.keys.contains(&code)) {
                if let Some(e) = edge_for(b.action, mode, false) {
                    self.edges.push(e);
                }
            }
        }
    }

    fn mouse_edges(&mut self, dev: &DeviceState, mode: Mode) {
        for b in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
            if !dev.mouse.was_pressed(b) {
                continue;
            }
            self.pad_last = false;
            // A screen reads the mouse itself (widgets, drag and drop); only play hears a click.
            if mode != Mode::Play {
                continue;
            }
            for row in BINDINGS.iter().filter(|r| r.mouse == Some(b)) {
                if let Some(e) = edge_for(row.action, mode, false) {
                    self.edges.push(e);
                }
            }
        }
    }

    fn pad_edges(&mut self, p: Option<&Pad>, mode: Mode) {
        let Some(p) = p else {
            self.pad_was = 0;
            self.rt_was = false;
            self.lt_was = false;
            return;
        };
        let rt = p.axes[5] > TRIGGER_HELD;
        let lt = p.axes[4] > TRIGGER_HELD;
        let rose = p.held & !self.pad_was;
        let rt_rose = rt && !self.rt_was;
        let lt_rose = lt && !self.lt_was;
        (self.pad_was, self.rt_was, self.lt_was) = (p.held, rt, lt);
        let moved = axis(p.axes[0]).hypot(axis(p.axes[1])) > STICK_DEADZONE
            || axis(p.axes[2]).hypot(axis(p.axes[3])) > AIM_DEADZONE;
        if rose != 0 || rt_rose || lt_rose || moved {
            self.pad_last = true;
        }
        for row in BINDINGS {
            let fired = match row.pad {
                Some(PadInput::Button(b)) => rose & (1 << b) != 0,
                Some(PadInput::RightTrigger) => rt_rose,
                Some(PadInput::LeftTrigger) => lt_rose,
                None => false,
            };
            if fired && let Some(e) = edge_for(row.action, mode, true) {
                self.edges.push(e);
            }
        }
    }
}

/// Whether a binding's action is held on any device.
fn is_held(dev: &DeviceState, a: Action) -> bool {
    BINDINGS.iter().filter(|b| b.action == a).any(|b| {
        b.keys.iter().any(|&k| k != 0 && dev.keys.has(k))
            || b.mouse.is_some_and(|m| dev.mouse.is_held(m))
            || match (b.pad, &dev.pad) {
                (Some(PadInput::Button(n)), Some(p)) => p.is_held(n),
                (Some(PadInput::RightTrigger), Some(p)) => p.axes[5] > TRIGGER_HELD,
                (Some(PadInput::LeftTrigger), Some(p)) => p.axes[4] > TRIGGER_HELD,
                _ => false,
            }
    })
}

/// What pressing `a` means in `mode`. On a pad in a screen the face and shoulder buttons
/// navigate (A confirm, B back, LB and RB the tabs).
fn edge_for(a: Action, mode: Mode, from_pad: bool) -> Option<Edge> {
    // Heard in play and in a screen alike.
    let toggle = match a {
        Action::Console => Some(UiAction::Console),
        Action::Debug => Some(UiAction::Debug),
        Action::Grid => Some(UiAction::Grid),
        Action::Shot => Some(UiAction::Shot),
        Action::QuickSave => Some(UiAction::QuickSave),
        Action::QuickLoad => Some(UiAction::QuickLoad),
        Action::Bags => Some(UiAction::Bags),
        Action::Book => Some(UiAction::Book),
        Action::Quests => Some(UiAction::Quests),
        Action::Map => Some(UiAction::Map),
        _ => None,
    };
    let ui = match mode {
        Mode::Text => match a {
            Action::Pause => Some(UiAction::Cancel),
            Action::Console => Some(UiAction::Console),
            _ => None,
        },
        _ if toggle.is_some() => toggle,
        Mode::Play => {
            return match a {
                Action::Pause => Some(Edge::Ui(UiAction::Pause)),
                Action::Use => Some(Edge::Game(GameAction::Use)),
                Action::Bar(n) => Some(Edge::Game(GameAction::Bar(n))),
                _ => None,
            };
        }
        Mode::Ui => match a {
            Action::Up => Some(UiAction::Up),
            Action::Down => Some(UiAction::Down),
            Action::Left => Some(UiAction::Left),
            Action::Right => Some(UiAction::Right),
            // B on a pad is back, as Esc is.
            Action::Pause => Some(UiAction::Cancel),
            Action::Use if from_pad => Some(UiAction::Cancel),
            Action::Use | Action::Bar(0) => Some(UiAction::Confirm),
            Action::Bar(3) if from_pad => Some(UiAction::TabLeft),
            Action::Bar(4) if from_pad => Some(UiAction::TabRight),
            _ => None,
        },
    };
    ui.map(Edge::Ui)
}

/// A stick axis as -1..=1.
fn axis(v: i16) -> f32 {
    (f32::from(v) / 32767.0).clamp(-1.0, 1.0)
}

/// The bearing of `(dx, dy)` (y down), quantised through the sim's own table.
fn angle_of(dx: f32, dy: f32) -> Angle {
    let len = dx.hypot(dy);
    if len == 0.0 {
        return Angle::EAST;
    }
    let k = 16_384.0 / len;
    iatan2((dy * k).round() as i32, (dx * k).round() as i32)
}

/// A move vector (length 1 is full tilt) as the sim reads it: `(Angle, 0..=127)`. Standing still
/// is `(EAST, 0)`, as `InputFrame::IDLE` has it.
pub fn quantise(mx: f32, my: f32) -> (Angle, u8) {
    let len = mx.hypot(my);
    let mag = (len.min(1.0) * 127.0).round() as u8;
    if mag == 0 {
        return (Angle::EAST, 0);
    }
    (angle_of(mx, my), mag)
}

/// The canvas for a window of `win` px (PRESENTATION.md, the window): always `CANVAS_H` tall,
/// `ceil(win_w / s)` wide with `s = win_h / CANVAS_H`, clamped to a sane width. A zero-height
/// (minimised) window keeps 16:9.
pub fn canvas_size(win_w: u32, win_h: u32) -> (u16, u16) {
    const MAX_W: u64 = 4096;
    let h = u64::from(CANVAS_H);
    if win_h == 0 || win_w == 0 {
        return (crate::frame::CANVAS_W, CANVAS_H);
    }
    let w = (u64::from(win_w) * h).div_ceil(u64::from(win_h)).clamp(1, MAX_W);
    (w as u16, CANVAS_H)
}

/// A point in window px as canvas px, through the window's scale `s = win_h / CANVAS_H`.
pub fn to_canvas(win_x: i32, win_y: i32, win_h: u32) -> (f32, f32) {
    let s = f64::from(win_h.max(1)) / f64::from(CANVAS_H);
    ((f64::from(win_x) / s) as f32, (f64::from(win_y) / s) as f32)
}

/// Fx per canvas px: `FX_ONE` per sim px, and the render scale is 2 (PRESENTATION.md, the
/// canvas).
const FX_PER_CANVAS: f64 = (jane_core::num::FX_ONE / 2) as f64;

/// A sim position in canvas px, for a camera whose top-left is `camera` canvas px.
pub fn world_to_canvas(p: Vec2, camera: (i32, i32)) -> (f32, f32) {
    let f = |v: Fx, cam: i32| (f64::from(v.0) / FX_PER_CANVAS - f64::from(cam)) as f32;
    (f(p.x, camera.0), f(p.y, camera.1))
}

/// A canvas point as a sim position, for a camera whose top-left is `camera` canvas px.
pub fn canvas_to_world(c: (f32, f32), camera: (i32, i32)) -> Vec2 {
    let f = |v: f32, cam: i32| Fx(((f64::from(v) + f64::from(cam)) * FX_PER_CANVAS).round() as i32);
    Vec2 { x: f(c.0, camera.0), y: f(c.1, camera.1) }
}

/// What a bar press with a cursor names as its `on` (`Command::Bar`): the unit under the
/// cursor, nearest first, else her own body ("over nobody"). A unit is under the cursor when
/// the point is inside a person-sized box standing on its feet (16 x 20 sim px).
///
/// TODO(P6 scene): hit the drawn sprite's rect from the `Frame` once units draw at their real
/// sizes; the box is a person's.
pub fn pick(view: &View<'_>, at: Vec2) -> UnitId {
    const HALF_W: i32 = 8 * jane_core::num::FX_ONE;
    const TALL: i32 = 20 * jane_core::num::FX_ONE;
    const BELOW: i32 = 2 * jane_core::num::FX_ONE;
    let (cx, cy) = at.cell();
    let near = Rect::new(cx - 2, cy - 1, 5, 5);
    view.units_in(near)
        .map(|u| u.unit)
        .filter(|u| u.alive)
        .filter(|u| {
            let dx = at.x.0 - u.pos.x.0;
            let up = u.pos.y.0 - at.y.0;
            dx.abs() <= HALF_W && (-BELOW..=TALL).contains(&up)
        })
        .min_by_key(|u| {
            let dx = i64::from(at.x.0 - u.pos.x.0);
            let dy = i64::from(at.y.0 - (u.pos.y.0 - TALL / 2));
            (dx * dx + dy * dy, u.id)
        })
        .map_or(view.body().id, |u| u.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(dev: &mut DeviceState, code: u16) {
        dev.key(code, true);
    }

    fn play_at(feet: (f32, f32)) -> Context {
        Context { mode: Mode::Play, feet: Some(feet) }
    }

    fn sample(input: &mut Input, dev: &mut DeviceState, ctx: &Context) -> (InputFrame, Vec<Edge>) {
        let f = input.sample(dev, ctx);
        dev.end_sample();
        (f, input.drain().collect())
    }

    #[test]
    fn keys_hold_and_release() {
        let mut k = KeySet::default();
        k.set(sc::W, true);
        k.set(511, true);
        k.set(600, true);
        assert!(k.has(sc::W) && k.has(511) && !k.has(600) && !k.has(sc::S));
        assert_eq!(k.iter().collect::<Vec<_>>(), vec![sc::W, 511]);
        k.set(sc::W, false);
        assert!(!k.has(sc::W));
    }

    #[test]
    fn wasd_and_arrows_walk_in_eight_directions_at_full_tilt() {
        let mut input = Input::new();
        let cases: [(&[u16], Angle); 6] = [
            (&[sc::D], Angle::EAST),
            (&[sc::S], Angle::SOUTH),
            (&[sc::LEFT], Angle::WEST),
            (&[sc::UP], Angle::NORTH),
            (&[sc::W, sc::D], Angle(57344)),
            (&[sc::DOWN, sc::A], Angle(16384 + 8192)),
        ];
        for (keys, want) in cases {
            let mut dev = DeviceState::default();
            for &k in keys {
                press(&mut dev, k);
            }
            let (f, _) = sample(&mut input, &mut dev, &Context::default());
            assert_eq!(f.mv_mag, 127, "{keys:?}");
            assert!(f.mv_dir.gap(want) <= 8, "{keys:?}: {:?} want {want:?}", f.mv_dir);
        }
        // Opposites cancel: she stands.
        let mut dev = DeviceState::default();
        press(&mut dev, sc::A);
        press(&mut dev, sc::D);
        let (f, _) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!((f.mv_dir, f.mv_mag), (Angle::EAST, 0));
    }

    #[test]
    fn shift_sprints_and_e_uses_held_and_pressed() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        press(&mut dev, sc::LSHIFT);
        press(&mut dev, sc::E);
        let (f, edges) = sample(&mut input, &mut dev, &Context::default());
        assert!(f.sprint && f.use_held);
        assert_eq!(edges, vec![Edge::Game(GameAction::Use)]);
        // Still held next frame: held, but no second press.
        let (f, edges) = sample(&mut input, &mut dev, &Context::default());
        assert!(f.use_held);
        assert!(edges.is_empty());
        dev.key(sc::E, false);
        let (f, _) = sample(&mut input, &mut dev, &Context::default());
        assert!(!f.use_held && f.sprint);
    }

    #[test]
    fn number_keys_space_and_left_click_press_bar_slots() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        for n in 0..8 {
            press(&mut dev, sc::N1 + n);
        }
        let (_, edges) = sample(&mut input, &mut dev, &Context::default());
        let want: Vec<Edge> = (0..8).map(|n| Edge::Game(GameAction::Bar(n))).collect();
        assert_eq!(edges, want);
        let mut dev = DeviceState::default();
        press(&mut dev, sc::SPACE);
        dev.button(MouseButton::Left, true);
        let (_, edges) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!(edges, vec![Edge::Game(GameAction::Bar(0)), Edge::Game(GameAction::Bar(0))]);
    }

    #[test]
    fn a_tap_shorter_than_a_frame_still_presses() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        dev.key(sc::E, true);
        dev.key(sc::E, false);
        let (f, edges) = sample(&mut input, &mut dev, &Context::default());
        assert!(!f.use_held);
        assert_eq!(edges, vec![Edge::Game(GameAction::Use)]);
    }

    #[test]
    fn the_toggles() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        for k in [sc::ESCAPE, sc::TAB, sc::M, sc::GRAVE, sc::F2, sc::F3, sc::F12] {
            press(&mut dev, k);
        }
        let (_, edges) = sample(&mut input, &mut dev, &Context::default());
        // Scancode order: Esc 41, Tab 43, ` 53, F2 59, F3 60, F12 69, M 16 first.
        assert_eq!(
            edges,
            [
                UiAction::Map,
                UiAction::Pause,
                UiAction::Bags,
                UiAction::Console,
                UiAction::Debug,
                UiAction::Grid,
                UiAction::Shot
            ]
            .map(Edge::Ui)
        );
    }

    #[test]
    fn a_screen_takes_the_keys_and_she_stands() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        let ui = Context { mode: Mode::Ui, feet: Some((100.0, 100.0)) };
        for k in [sc::ESCAPE, sc::W, sc::RETURN, sc::E, sc::N1 + 2] {
            press(&mut dev, k);
        }
        dev.button(MouseButton::Left, true);
        let (f, edges) = sample(&mut input, &mut dev, &ui);
        assert_eq!((f.mv_mag, f.use_held, f.aim), (0, false, None));
        // Scancode order: E, W, 3 (no bar in a screen), Return, Esc; the click is the screen's.
        assert_eq!(edges, [UiAction::Confirm, UiAction::Up, UiAction::Confirm, UiAction::Cancel].map(Edge::Ui));
        // Typing: only Esc and the console key are heard.
        let text = Context { mode: Mode::Text, feet: None };
        let mut dev = DeviceState::default();
        for k in [sc::E, sc::GRAVE, sc::M, sc::ESCAPE, sc::RETURN] {
            press(&mut dev, k);
        }
        let (_, edges) = sample(&mut input, &mut dev, &text);
        assert_eq!(edges, [UiAction::Cancel, UiAction::Console].map(Edge::Ui));
    }

    #[test]
    fn the_mouse_aims_from_her_chest() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        dev.mouse.pos = Some((300.0, 188.0));
        dev.mouse.moved = true;
        // Feet at (200, 200): the chest is (200, 188), the cursor due east of it.
        let (f, _) = sample(&mut input, &mut dev, &play_at((200.0, 200.0)));
        assert_eq!(f.aim, Some(Angle::EAST));
        assert_eq!(f.mv_mag, 0, "no button, no walk");
        assert_eq!(f.assist, AssistProfile::Off);
        dev.mouse.pos = Some((200.0, 88.0));
        let (f, _) = sample(&mut input, &mut dev, &play_at((200.0, 200.0)));
        assert_eq!(f.aim, Some(Angle::NORTH));
        // On her chest: no aim; not drawn: no aim.
        dev.mouse.pos = Some((201.0, 188.0));
        assert_eq!(sample(&mut input, &mut dev, &play_at((200.0, 200.0))).0.aim, None);
        dev.mouse.pos = Some((300.0, 188.0));
        assert_eq!(sample(&mut input, &mut dev, &Context::default()).0.aim, None);
    }

    #[test]
    fn the_right_button_walks_toward_the_cursor_by_distance_over_64() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        dev.button(MouseButton::Right, true);
        let feet = (200.0, 212.0); // chest (200, 200)
        for (dist, mag) in [(16.0, 32), (32.0, 64), (64.0, 127), (300.0, 127)] {
            dev.mouse.pos = Some((200.0, 200.0 + dist));
            let (f, edges) = sample(&mut input, &mut dev, &play_at(feet));
            assert_eq!(f.mv_mag, mag, "{dist}");
            assert_eq!(f.mv_dir, Angle::SOUTH);
            assert!(edges.is_empty(), "the right button is no bar slot");
        }
        // Keys win over the virtual stick.
        press(&mut dev, sc::A);
        let (f, _) = sample(&mut input, &mut dev, &play_at(feet));
        assert_eq!((f.mv_dir, f.mv_mag), (Angle::WEST, 127));
        assert_eq!(f.aim, Some(Angle::SOUTH));
    }

    #[test]
    fn the_pad_moves_aims_and_presses_in_the_2020_layout() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        let mut p = Pad::default();
        p.axes[0] = 32767; // left stick hard right
        p.axes[3] = -32767; // right stick up
        p.axes[5] = 32767; // RT
        p.held = 1 << pad::A | 1 << pad::B | 1 << pad::LB;
        dev.pad = Some(p);
        let (f, edges) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!((f.mv_dir, f.mv_mag), (Angle::EAST, 127));
        assert_eq!(f.aim, Some(Angle::NORTH));
        assert!(f.sprint && f.use_held);
        assert_eq!(f.assist, AssistProfile::Pad);
        assert!(input.pad_active() && !input.aiming_with_mouse());
        assert_eq!(
            edges,
            vec![Edge::Game(GameAction::Use), Edge::Game(GameAction::Bar(0)), Edge::Game(GameAction::Bar(3))]
        );
        // Held, not pressed again; a small stick is inside the dead zone.
        p.axes[0] = 3000;
        dev.pad = Some(p);
        let (f, edges) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!(f.mv_mag, 0);
        assert!(edges.is_empty());
        // The mouse moving takes the aim back.
        p.axes[3] = 0;
        dev.pad = Some(p);
        dev.mouse.pos = Some((10.0, 0.0));
        dev.mouse.moved = true;
        let (f, _) = sample(&mut input, &mut dev, &play_at((0.0, 12.0)));
        assert_eq!(f.aim, Some(Angle::EAST));
        assert_eq!(f.assist, AssistProfile::Off);
    }

    #[test]
    fn a_pad_in_a_screen_navigates() {
        let mut input = Input::new();
        let mut dev = DeviceState { pad: Some(Pad::default()), ..DeviceState::default() };
        let _ = sample(&mut input, &mut dev, &Context::default());
        dev.pad = Some(Pad {
            held: 1 << pad::A | 1 << pad::B | 1 << pad::RB | 1 << pad::DPAD_DOWN | 1 << pad::START,
            ..Pad::default()
        });
        let (_, edges) = sample(&mut input, &mut dev, &Context { mode: Mode::Ui, feet: None });
        assert_eq!(
            edges,
            [UiAction::Down, UiAction::Cancel, UiAction::Confirm, UiAction::TabRight, UiAction::Cancel].map(Edge::Ui)
        );
    }

    #[test]
    fn quantise_is_the_sims_angle_and_a_7_bit_magnitude() {
        assert_eq!(quantise(0.0, 0.0), (Angle::EAST, 0));
        assert_eq!(quantise(0.001, 0.0), (Angle::EAST, 0));
        assert_eq!(quantise(0.5, 0.0), (Angle::EAST, 64));
        assert_eq!(quantise(-3.0, 0.0), (Angle::WEST, 127));
        let (d, m) = quantise(0.0, 1.0);
        assert_eq!((d, m), (Angle::SOUTH, 127));
    }

    #[test]
    fn the_canvas_is_432_tall_and_as_wide_as_the_window_shows() {
        assert_eq!(canvas_size(1536, 864), (768, 432));
        assert_eq!(canvas_size(768, 432), (768, 432));
        assert_eq!(canvas_size(1920, 1080), (768, 432));
        assert_eq!(canvas_size(3440, 1440), (1032, 432));
        assert_eq!(canvas_size(1537, 864), (769, 432), "ceil: the last column is part-shown");
        assert_eq!(canvas_size(800, 0), (768, 432));
        assert_eq!(to_canvas(1536, 864, 864), (768.0, 432.0));
        assert_eq!(to_canvas(250, 100, 1080), (100.0, 40.0));
    }

    #[test]
    fn world_and_canvas_round_trip() {
        // One sim px is two canvas px; a sim px is 256 Fx.
        let p = Vec2 { x: Fx(256 * 100), y: Fx(256 * 50) };
        assert_eq!(world_to_canvas(p, (0, 0)), (200.0, 100.0));
        assert_eq!(world_to_canvas(p, (150, 40)), (50.0, 60.0));
        assert_eq!(canvas_to_world((50.0, 60.0), (150, 40)), p);
    }

    #[test]
    fn the_table_binds_every_action_and_no_key_twice() {
        let mut seen = KeySet::default();
        for b in BINDINGS {
            for &k in b.keys.iter().filter(|&&k| k != 0) {
                assert!(!seen.has(k), "{k} bound twice");
                seen.set(k, true);
            }
        }
        for n in 0..8 {
            assert!(BINDINGS.iter().any(|b| b.action == Action::Bar(n)), "bar slot {n}");
        }
    }

    #[test]
    fn a_cursor_over_nobody_names_her_and_over_her_names_her() {
        let sim = jane_sim::Sim::new_game(1, "Jane");
        let v = sim.view(jane_sim::Seat(0)).expect("seat 0 sits down at New Game");
        let me = v.body();
        let far = Vec2 { x: Fx(me.pos.x.0 + 256 * 400), y: me.pos.y };
        assert_eq!(pick(&v, far), me.id);
        let chest = Vec2 { x: me.pos.x, y: Fx(me.pos.y.0 - 256 * 6) };
        assert_eq!(pick(&v, chest), me.id);
    }
}
