//! Input mapping (PRESENTATION.md §4): device state in, one [`InputFrame`] and a queue of
//! [`Edge`]s out. Nothing downstream knows which device it was (ENGINE.md §10).
//!
//! The app turns SDL events into a [`DeviceState`] (keys by SDL scancode number, the mouse
//! already in canvas px, a pad in the standard mapping) and never hands this module an SDL
//! type; a test fills the same struct by hand. Floats are allowed here and never leave: move and
//! aim are quantised to `(Angle, magnitude)` before they become an `InputFrame`, and the raw aim
//! goes out unbent (the sim does the assist, ARCHITECTURE.md §5.4).
//!
//! Bindings are data: `data/bindings.json`, compiled in by this crate's build script into
//! [`BINDINGS`] (an unknown name is a build error), with the player's overrides from
//! `config.json` laid over them into the [`Bindings`] in force.

use alloc::vec::Vec;
use jane_core::angle::iatan2;
use jane_core::{Angle, Fx, Rect, Vec2};
use jane_sim::UnitId;
use jane_sim::input::{AssistProfile, InputFrame, TargetRef};
use jane_sim::view::View;

#[cfg(feature = "std")]
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
    pub const BACKSPACE: u16 = 42;
    pub const DELETE: u16 = 76;
    pub const TAB: u16 = 43;
    pub const SPACE: u16 = 44;
    pub const N0: u16 = 39;
    /// The key left of `1` on a US board.
    pub const GRAVE: u16 = 53;
    /// `F1`; `F2` to `F12` follow.
    pub const F1: u16 = 58;
    pub const F2: u16 = 59;
    pub const F3: u16 = 60;
    pub const F4: u16 = 61;
    pub const F5: u16 = 62;
    pub const F6: u16 = 63;
    pub const F7: u16 = 64;
    pub const F8: u16 = 65;
    pub const F9: u16 = 66;
    pub const F12: u16 = 69;
    pub const HOME: u16 = 74;
    pub const PAGEUP: u16 = 75;
    pub const END: u16 = 77;
    pub const PAGEDOWN: u16 = 78;
    pub const RIGHT: u16 = 79;
    pub const LEFT: u16 = 80;
    pub const DOWN: u16 = 81;
    pub const UP: u16 = 82;
    pub const KP_ENTER: u16 = 88;
    pub const LCTRL: u16 = 224;
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
    pub pos: Option<(Px, Px)>,
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
    /// Her hop (`Command::Hop`): out of a blow's way.
    Hop,
    /// Use, talk; held: push and pull.
    Use,
    /// Left click (PLAY-PLAN §2.1): target the foe or prop under the cursor, without moving;
    /// over open ground, let the target go.
    Select,
    /// Right click: walk there, or to the foe and swing at it, or to the door, fire, chest or
    /// person and use it (click to move; the pad has none).
    Goto,
    /// Tab, RB: the next foe in front, nearest first (Shift-Tab goes back).
    Target,
    /// LB: the foe before.
    TargetBack,
    /// Held: casts go along the aim even with a target.
    FreeAim,
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
    /// While the world is held still (F6), one tick of it.
    Step,
    /// A quarter speed, or back.
    Slow,
    /// Four times speed, or back.
    Fast,
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

// `BINDINGS` (the rows of `data/bindings.json`), `ACTIONS`, `PADS` and `MICE` (every name the
// data and `config.json` may use), compiled by build.rs.
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// A scancode's name as a key cap shows it (`E`, `Space`, `F5`); `?` for one with no name.
pub fn key_name(code: u16) -> &'static str {
    crate::input_names::KEY_NAMES.iter().find(|k| k.1 == code).map_or("?", |k| k.0)
}

/// The scancode a cap name names (`key_name`'s inverse, any case; `Escape`, `Return`, `Up` and
/// the like are taken too).
pub fn key_code(name: &str) -> Option<u16> {
    let alias = match name.to_ascii_lowercase().as_str() {
        "escape" => Some(sc::ESCAPE),
        "return" => Some(sc::RETURN),
        "backspace" => Some(sc::BACKSPACE),
        "up" => Some(sc::UP),
        "down" => Some(sc::DOWN),
        "left" => Some(sc::LEFT),
        "right" => Some(sc::RIGHT),
        "grave" | "backquote" => Some(sc::GRAVE),
        "lshift" => Some(sc::LSHIFT),
        "lctrl" => Some(sc::LCTRL),
        _ => None,
    };
    alias.or_else(|| crate::input_names::KEY_NAMES.iter().find(|k| k.0.eq_ignore_ascii_case(name)).map(|k| k.1))
}

/// A pad input's name as a hint shows it.
pub fn pad_name(p: PadInput) -> &'static str {
    PADS.iter().find(|x| x.1 == p).map_or("?", |x| x.0)
}

/// The pad input a name names.
pub fn pad_input(name: &str) -> Option<PadInput> {
    PADS.iter().find(|x| x.0.eq_ignore_ascii_case(name)).map(|x| x.1)
}

/// A mouse button's name.
pub fn mouse_name(b: MouseButton) -> &'static str {
    MICE.iter().find(|x| x.1 == b).map_or("?", |x| x.0)
}

/// The mouse button a name names.
pub fn mouse_button(name: &str) -> Option<MouseButton> {
    MICE.iter().find(|x| x.0.eq_ignore_ascii_case(name)).map(|x| x.1)
}

/// An action's data name (`"use"`, `"bar3"`).
pub fn action_name(a: Action) -> &'static str {
    ACTIONS.iter().find(|x| x.1 == a).map_or("?", |x| x.0)
}

/// An action's label on the Controls screen.
pub fn action_label(a: Action) -> &'static str {
    ACTIONS.iter().find(|x| x.1 == a).map_or("?", |x| x.2)
}

/// The action a data name names.
pub fn action(name: &str) -> Option<Action> {
    ACTIONS.iter().find(|x| x.0 == name).map(|x| x.1)
}

/// The bindings in force (PRESENTATION.md §4): one row per action, the compiled table with the
/// player's overrides from `config.json` laid over it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    pub rows: Vec<Binding>,
}

impl Default for Bindings {
    fn default() -> Bindings {
        Bindings { rows: BINDINGS.to_vec() }
    }
}

impl Bindings {
    /// The row for `a`.
    pub fn row(&self, a: Action) -> Option<&Binding> {
        self.rows.iter().find(|b| b.action == a)
    }

    /// The first key bound to `a`, as its cap shows it.
    pub fn key(&self, a: Action) -> &'static str {
        self.row(a).and_then(|b| b.keys.iter().find(|&&k| k != 0)).map_or("?", |&k| key_name(k))
    }

    /// The pad input bound to `a`, if any.
    pub fn pad(&self, a: Action) -> Option<PadInput> {
        self.row(a).and_then(|b| b.pad)
    }

    /// Every other action bound to the same key, button or pad input as `(a, col)` (col 0 and 1
    /// the keys, 2 the mouse, 3 the pad): conflicts are shown, never refused.
    pub fn conflicts(&self, a: Action, col: u8) -> impl Iterator<Item = Action> + '_ {
        let me = self.row(a).copied();
        self.rows
            .iter()
            .filter(move |b| b.action != a)
            .filter(move |b| {
                let Some(m) = me else { return false };
                match col {
                    0 | 1 => {
                        let k = m.keys[usize::from(col)];
                        k != 0 && b.keys.contains(&k)
                    }
                    2 => m.mouse.is_some() && b.mouse == m.mouse,
                    _ => m.pad.is_some() && b.pad == m.pad,
                }
            })
            .map(|b| b.action)
    }
}

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
    /// At a cupboard: the lit thing across to the other side (X, or 2).
    Quick,
    /// At a cupboard: everything in the bag put away (Y, or 3).
    QuickAll,
    /// Delete or Backspace in a screen: the Log's Abandon (X on a pad is `Quick`).
    Remove,
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
    Step,
    Slow,
    Fast,
}

/// What becomes a sim `Command` (the app stamps it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameAction {
    /// `Command::Use`.
    Use,
    /// `Command::Bar { slot }`, `0..8`.
    Bar(u8),
    /// A left click in the world: target what is under the cursor (`target::pick_target`).
    Select,
    /// A right click in the world: `Command::Goto` (`target::goto_at`).
    Goto,
    /// Tab, RB (`back`: Shift-Tab, LB): cycle the foes in front.
    Tab { back: bool },
    /// `Command::Hop`.
    Hop,
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
    pub feet: Option<(Px, Px)>,
}

/// A canvas coordinate: `f32` on the PC, where the window's scale may be fractional (`Fit`), whole
/// px on the consoles, whose canvas is the screen (PORT.md §13.11).
#[cfg(feature = "std")]
pub type Px = f32;
/// A canvas coordinate: whole px on the consoles, whose canvas is the screen (PORT.md §13.11).
#[cfg(not(feature = "std"))]
pub type Px = i32;

/// A stick's or the keys' lean on one axis: -1 to 1 as `f32` on the PC, as it always was;
/// -32 767 to 32 767 on the consoles.
#[cfg(feature = "std")]
type Tilt = f32;
#[cfg(not(feature = "std"))]
type Tilt = i32;
#[cfg(feature = "std")]
const TILT_ZERO: Tilt = 0.0;
#[cfg(not(feature = "std"))]
const TILT_ZERO: Tilt = 0;
#[cfg(feature = "std")]
const TILT_ONE: Tilt = 1.0;
#[cfg(not(feature = "std"))]
const TILT_ONE: Tilt = 32_767;

/// Aim comes from her chest, this far above her feet, in canvas px.
#[cfg(feature = "std")]
pub const CHEST_PX: Px = 12.0;
/// Aim comes from her chest, this far above her feet, in canvas px.
#[cfg(not(feature = "std"))]
pub const CHEST_PX: Px = 12;
/// Right button held walks toward the cursor at full tilt from this many canvas px out.
#[cfg(feature = "std")]
pub const WALK_FULL_PX: Px = 64.0;
/// Right button held walks toward the cursor at full tilt from this many canvas px out.
#[cfg(not(feature = "std"))]
pub const WALK_FULL_PX: Px = 64;
/// The cursor must be this far from her chest, in canvas px, to aim.
#[cfg(feature = "std")]
pub const AIM_MIN_PX: Px = 2.0;
/// The cursor must be this far from her chest, in canvas px, to aim.
#[cfg(not(feature = "std"))]
pub const AIM_MIN_PX: Px = 2;
/// Left stick, radial (2020's value).
#[cfg(feature = "std")]
pub const STICK_DEADZONE: f32 = 0.2;
/// Right stick aims past this.
#[cfg(feature = "std")]
pub const AIM_DEADZONE: f32 = 0.35;
/// The left stick leans a step through a screen past this.
#[cfg(feature = "std")]
pub const STICK_NAV: f32 = 0.6;
/// Left stick, radial (2020's value), of [`TILT_ONE`].
#[cfg(not(feature = "std"))]
pub const STICK_DEADZONE: i32 = 32_767 / 5;
/// Right stick aims past this.
#[cfg(not(feature = "std"))]
pub const AIM_DEADZONE: i32 = 32_767 * 35 / 100;
/// The left stick leans a step through a screen past this.
#[cfg(not(feature = "std"))]
pub const STICK_NAV: i32 = 32_767 * 3 / 5;
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
    /// The way the left stick leans in a screen, so one lean is one step ([`STICK_NAV`]).
    stick_dir: Option<UiAction>,
    /// The Controls row's assist profile; `None`: `Pad` while the pad aims, `Off` for a mouse.
    pub assist: Option<AssistProfile>,
    /// The table in force: the compiled rows with the player's overrides over them.
    pub bindings: Bindings,
    /// Her hard target as this client holds it (PLAY-PLAN §2.1), carried in every frame;
    /// `target.rs` chooses it, settles it against the view and lets it go.
    pub target: Option<TargetRef>,
    /// The target last let go (Esc, a click on open ground): not taken back from the sim's soft
    /// target until the sim has let it go too (it hears the `Halt` a few frames later).
    pub dropped: Option<TargetRef>,
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
            // The mouse is the device in hand: the reticle and the arrow come back, hints show keys.
            self.aim_pad = false;
            self.pad_last = false;
        }
        if ctx.mode != Mode::Play {
            return InputFrame { assist: self.profile(), ..InputFrame::IDLE };
        }
        let rows = &self.bindings.rows;
        let held = |a: Action| is_held(rows, dev, a);
        let (mut mx, mut my) = (TILT_ZERO, TILT_ZERO);
        if held(Action::Left) {
            mx -= TILT_ONE;
        }
        if held(Action::Right) {
            mx += TILT_ONE;
        }
        if held(Action::Up) {
            my -= TILT_ONE;
        }
        if held(Action::Down) {
            my += TILT_ONE;
        }
        if mx == TILT_ZERO
            && my == TILT_ZERO
            && let Some(p) = &dev.pad
        {
            let (lx, ly) = (axis(p.axes[0]), axis(p.axes[1]));
            if longer(lx, ly, STICK_DEADZONE) {
                (mx, my) = (lx, ly);
            }
        }
        // The aim: the right stick when it is out, else the cursor from her chest.
        let chest_to_cursor = match (ctx.feet, dev.mouse.pos) {
            (Some((fx, fy)), Some((cx, cy))) => Some((cx - fx, cy - (fy - CHEST_PX))),
            _ => None,
        };
        let mut aim = None;
        // The right stick pushed aims freely (PLAY-PLAN §2.1), as the free-aim key does.
        let mut stick_out = false;
        if let Some(p) = &dev.pad {
            let (rx, ry) = (axis(p.axes[2]), axis(p.axes[3]));
            if longer(rx, ry, AIM_DEADZONE) {
                self.aim_pad = true;
                stick_out = true;
                aim = Some(angle_of(rx, ry));
            }
        }
        if !self.aim_pad
            && let Some((dx, dy)) = chest_to_cursor
            && longer(dx, dy, AIM_MIN_PX)
        {
            aim = Some(angle_of(dx, dy));
        }
        let (mv_dir, mv_mag) = quantise(mx, my);
        InputFrame {
            mv_dir,
            mv_mag,
            aim,
            sprint: held(Action::Sprint),
            use_held: held(Action::Use),
            assist: self.profile(),
            target: self.target,
            free: held(Action::FreeAim) || stick_out,
        }
    }

    /// The edges queued since the last drain, in the order they were pressed within a sample.
    pub fn drain(&mut self) -> alloc::vec::Drain<'_, Edge> {
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
            if mode == Mode::Ui && (code == sc::DELETE || code == sc::BACKSPACE) {
                self.edges.push(Edge::Ui(UiAction::Remove));
                continue;
            }
            let shift = dev.keys.has(sc::LSHIFT) || dev.keys.has(sc::RSHIFT);
            for b in self.bindings.rows.iter().filter(|b| code != 0 && b.keys.contains(&code)) {
                // Shift-Tab goes back through the foes.
                let a = if b.action == Action::Target && shift { Action::TargetBack } else { b.action };
                if let Some(e) = edge_for(a, mode, false) {
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
            for row in self.bindings.rows.iter().filter(|r| r.mouse == Some(b)) {
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
        let moved = longer(axis(p.axes[0]), axis(p.axes[1]), STICK_DEADZONE)
            || longer(axis(p.axes[2]), axis(p.axes[3]), AIM_DEADZONE);
        if rose != 0 || rt_rose || lt_rose || moved {
            self.pad_last = true;
        }
        for row in &self.bindings.rows {
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
        // The left stick steps through a screen as the D-pad does: one step a lean.
        let (lx, ly) = (axis(p.axes[0]), axis(p.axes[1]));
        let dir = if lx.abs().max(ly.abs()) < STICK_NAV {
            None
        } else if lx.abs() > ly.abs() {
            Some(if lx > TILT_ZERO { UiAction::Right } else { UiAction::Left })
        } else {
            Some(if ly > TILT_ZERO { UiAction::Down } else { UiAction::Up })
        };
        if mode == Mode::Ui
            && dir != self.stick_dir
            && let Some(a) = dir
        {
            self.edges.push(Edge::Ui(a));
        }
        self.stick_dir = dir;
    }
}

/// Where the reticle goes (§4): along `aim` (the assisted aim) from her chest, at the cursor's
/// distance from it. Every bearing lands on its own side, the four axes too. PC only: the
/// consoles have no cursor.
#[cfg(feature = "std")]
pub fn reticle_at(chest: (f32, f32), cursor: (f32, f32), aim: Angle) -> (i32, i32) {
    let d = f64::from((cursor.0 - chest.0).hypot(cursor.1 - chest.1));
    // Q15 reaches 32768 (1.0) on the axes: read it wide, never through an i16, which turns it
    // to -1.0 and threw the reticle to her other side.
    let k = f64::from(jane_core::angle::cos_q15(aim).0) / 32768.0;
    let s = f64::from(jane_core::angle::sin_q15(aim).0) / 32768.0;
    ((f64::from(chest.0) + k * d).round() as i32, (f64::from(chest.1) + s * d).round() as i32)
}

/// Whether a binding's action is held on any device.
fn is_held(rows: &[Binding], dev: &DeviceState, a: Action) -> bool {
    rows.iter().filter(|b| b.action == a).any(|b| {
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
        Action::Step => Some(UiAction::Step),
        Action::Slow => Some(UiAction::Slow),
        Action::Fast => Some(UiAction::Fast),
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
                Action::Select => Some(Edge::Game(GameAction::Select)),
                Action::Goto => Some(Edge::Game(GameAction::Goto)),
                Action::Target => Some(Edge::Game(GameAction::Tab { back: false })),
                Action::TargetBack => Some(Edge::Game(GameAction::Tab { back: true })),
                Action::Hop => Some(Edge::Game(GameAction::Hop)),
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
            // Space was bar 1 and goes on in a menu or a conversation still.
            Action::Use | Action::Bar(0) | Action::Hop => Some(UiAction::Confirm),
            Action::Bar(1) => Some(UiAction::Quick),
            Action::Bar(2) => Some(UiAction::QuickAll),
            Action::TargetBack if from_pad => Some(UiAction::TabLeft),
            Action::Target if from_pad => Some(UiAction::TabRight),
            _ => None,
        },
    };
    ui.map(Edge::Ui)
}

/// A stick axis as -1..=1.
#[cfg(feature = "std")]
fn axis(v: i16) -> Tilt {
    (f32::from(v) / 32767.0).clamp(-1.0, 1.0)
}

/// A stick axis as -32 767..=32 767.
#[cfg(not(feature = "std"))]
fn axis(v: i16) -> Tilt {
    i32::from(v).max(-32_767)
}

/// Whether `(dx, dy)` is longer than `min`.
#[cfg(feature = "std")]
fn longer(dx: f32, dy: f32, min: f32) -> bool {
    dx.hypot(dy) > min
}

#[cfg(not(feature = "std"))]
fn longer(dx: i32, dy: i32, min: i32) -> bool {
    let (dx, dy, min) = (i64::from(dx), i64::from(dy), i64::from(min));
    dx * dx + dy * dy > min * min
}

/// The bearing of `(dx, dy)` (y down), through the sim's own table.
#[cfg(not(feature = "std"))]
fn angle_of(dx: i32, dy: i32) -> Angle {
    iatan2(dy, dx)
}

/// A move vector (length [`TILT_ONE`] is full tilt) as the sim reads it: `(Angle, 0..=127)`.
#[cfg(not(feature = "std"))]
pub fn quantise(mx: i32, my: i32) -> (Angle, u8) {
    let (x, y) = (i64::from(mx), i64::from(my));
    let len = ((x * x + y * y).unsigned_abs().isqrt() as i64).min(i64::from(TILT_ONE));
    let mag = ((len * 127 * 2 + i64::from(TILT_ONE)) / (2 * i64::from(TILT_ONE))) as u8;
    if mag == 0 {
        return (Angle::EAST, 0);
    }
    (angle_of(mx, my), mag)
}

/// The bearing of `(dx, dy)` (y down), quantised through the sim's own table.
#[cfg(feature = "std")]
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
#[cfg(feature = "std")]
pub fn quantise(mx: f32, my: f32) -> (Angle, u8) {
    let len = mx.hypot(my);
    let mag = (len.min(1.0) * 127.0).round() as u8;
    if mag == 0 {
        return (Angle::EAST, 0);
    }
    (angle_of(mx, my), mag)
}

/// How the canvas is laid on the window (PRESENTATION.md, the window; `config.json` `scaling`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scaling {
    /// The largest whole multiple that fits, every canvas px the same square of window px, with
    /// thin bars of [`BARS`] round it. The default.
    #[default]
    Whole,
    /// The window's height filled at whatever scale that is, by sharp bilinear: whole-number
    /// nearest, then a smooth last fit, so no px is wider than its neighbour by a whole px.
    Fill,
}

/// The bars round a whole-number canvas: the theme's dark, the frame's own clear.
pub const BARS: u32 = 0xff10_1014;

/// The canvas on the window: its size, and where and how large it is shown. PC only: a console's
/// canvas is its screen.
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The canvas, px: always `CANVAS_H` tall, as wide as the window shows at the scale.
    pub canvas: (u16, u16),
    /// Window px per canvas px: whole for [`Scaling::Whole`].
    pub scale: f32,
    /// The canvas's top-left in the window, px (0, 0 when it fills).
    pub origin: (i32, i32),
    /// The canvas's size on the window, px (it may run a part px past the right edge).
    pub size: (u32, u32),
}

#[cfg(feature = "std")]
impl Fit {
    /// A point in window px as canvas px.
    pub fn to_canvas(&self, win_x: i32, win_y: i32) -> (f32, f32) {
        let s = f64::from(self.scale.max(1e-3));
        let f = |v: i32, o: i32| (f64::from(v - o) / s) as f32;
        (f(win_x, self.origin.0), f(win_y, self.origin.1))
    }

    /// Whether the scale is a whole number.
    pub fn whole(&self) -> bool {
        (self.scale - self.scale.round()).abs() < 1e-4
    }
}

/// The canvas for a window of `win` px (PRESENTATION.md, the window). [`Scaling::Whole`]: the
/// largest whole `k` at which `CANVAS_W x CANVAS_H` fits (never under 1), the canvas
/// `CANVAS_H` tall and as wide as the window shows at `k` (a wider window widens it), centred
/// with bars. [`Scaling::Fill`]: `s = win_h / CANVAS_H`, the canvas `ceil(win_w / s)` wide, no
/// bars. A zero-sized (minimised) window keeps the canvas 16:9.
#[cfg(feature = "std")]
pub fn fit(win_w: u32, win_h: u32, scaling: Scaling) -> Fit {
    const MAX_W: u64 = 4096;
    let (cw, ch) = (u64::from(crate::frame::CANVAS_W), u64::from(CANVAS_H));
    if win_h == 0 || win_w == 0 {
        return Fit { canvas: (cw as u16, ch as u16), scale: 1.0, origin: (0, 0), size: (cw as u32, ch as u32) };
    }
    let (ww, wh) = (u64::from(win_w), u64::from(win_h));
    match scaling {
        Scaling::Whole => {
            let k = (wh / ch).min(ww / cw).max(1);
            let w = (ww / k).clamp(1, MAX_W);
            let size = ((w * k) as u32, (ch * k) as u32);
            let origin = ((win_w as i32 - size.0 as i32) / 2, (win_h as i32 - size.1 as i32) / 2);
            Fit { canvas: (w as u16, ch as u16), scale: f32::from(k as u16), origin, size }
        }
        Scaling::Fill => {
            let w = (ww * ch).div_ceil(wh).clamp(1, MAX_W);
            let size = ((w * wh).div_ceil(ch) as u32, win_h);
            Fit {
                canvas: (w as u16, ch as u16),
                scale: (f64::from(win_h) / f64::from(CANVAS_H)) as f32,
                origin: (0, 0),
                size,
            }
        }
    }
}

/// Fx per canvas px: `FX_ONE` per sim px, and the render scale is 2 (PRESENTATION.md, the
/// canvas).
#[cfg(feature = "std")]
const FX_PER_CANVAS: f64 = (jane_core::num::FX_ONE / 2) as f64;

/// A sim position in canvas px, for a camera whose top-left is `camera` canvas px.
#[cfg(feature = "std")]
pub fn world_to_canvas(p: Vec2, camera: (i32, i32)) -> (Px, Px) {
    let f = |v: Fx, cam: i32| (f64::from(v.0) / FX_PER_CANVAS - f64::from(cam)) as f32;
    (f(p.x, camera.0), f(p.y, camera.1))
}

/// A sim position in whole canvas px (floored), for a camera whose top-left is `camera`.
#[cfg(not(feature = "std"))]
pub fn world_to_canvas(p: Vec2, camera: (i32, i32)) -> (Px, Px) {
    let f = |v: Fx, cam: i32| v.0.div_euclid(jane_core::num::FX_ONE / 2) - cam;
    (f(p.x, camera.0), f(p.y, camera.1))
}

/// A canvas point as a sim position, for a camera whose top-left is `camera` canvas px.
#[cfg(feature = "std")]
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
    fn number_keys_press_bar_slots_space_hops_and_left_click_targets() {
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
        // Space is the hop (PLAY-PLAN.md D2); left click chooses a target (§2.1), never a swing.
        assert_eq!(edges, vec![Edge::Game(GameAction::Hop), Edge::Game(GameAction::Select)]);
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
        for k in [sc::ESCAPE, sc::I, sc::M, sc::GRAVE, sc::F2, sc::F3, sc::F12] {
            press(&mut dev, k);
        }
        let (_, edges) = sample(&mut input, &mut dev, &Context::default());
        // Scancode order: I 12, M 16, Esc 41, ` 53, F2 59, F3 60, F12 69.
        assert_eq!(
            edges,
            [
                UiAction::Bags,
                UiAction::Map,
                UiAction::Pause,
                UiAction::Console,
                UiAction::Debug,
                UiAction::Grid,
                UiAction::Shot
            ]
            .map(Edge::Ui)
        );
    }

    #[test]
    fn tab_cycles_foes_and_shift_tab_goes_back() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        press(&mut dev, sc::TAB);
        let (_, edges) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!(edges, vec![Edge::Game(GameAction::Tab { back: false })]);
        dev.key(sc::TAB, false);
        press(&mut dev, sc::LSHIFT);
        press(&mut dev, sc::TAB);
        let (f, edges) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!(edges, vec![Edge::Game(GameAction::Tab { back: true })]);
        assert!(f.sprint, "Shift still sprints");
        // In a screen Tab is nothing of hers; the bags are I now.
        dev = DeviceState::default();
        press(&mut dev, sc::TAB);
        let (_, edges) = sample(&mut input, &mut dev, &Context { mode: Mode::Ui, feet: None });
        assert!(edges.is_empty(), "{edges:?}");
    }

    #[test]
    fn the_frame_carries_her_target_and_the_free_aim_key() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        let t = TargetRef::Unit(UnitId::new(7).unwrap());
        input.target = Some(t);
        let (f, _) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!((f.target, f.free), (Some(t), false));
        dev.key(sc::LCTRL, true);
        let (f, _) = sample(&mut input, &mut dev, &Context::default());
        assert_eq!((f.target, f.free), (Some(t), true), "held Ctrl aims freely, the target kept");
        // A screen sends an idle frame: no target either.
        let (f, _) = sample(&mut input, &mut dev, &Context { mode: Mode::Ui, feet: None });
        assert_eq!(f.target, None);
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
        // Scancode order: E, W, 3 (no bar in a screen: at a cupboard, put all away), Return, Esc;
        // the click is the screen's.
        assert_eq!(
            edges,
            [UiAction::Confirm, UiAction::Up, UiAction::QuickAll, UiAction::Confirm, UiAction::Cancel].map(Edge::Ui)
        );
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
    fn a_right_click_is_a_click_to_move_not_a_virtual_stick() {
        let mut input = Input::new();
        let mut dev = DeviceState::default();
        dev.button(MouseButton::Right, true);
        let feet = (200.0, 212.0); // chest (200, 200)
        dev.mouse.pos = Some((200.0, 264.0));
        let (f, edges) = sample(&mut input, &mut dev, &play_at(feet));
        assert_eq!(edges, vec![Edge::Game(GameAction::Goto)], "the app turns it into Command::Goto");
        assert_eq!(f.mv_mag, 0, "held, it no longer walks her toward the cursor");
        assert_eq!(f.aim, Some(Angle::SOUTH));
        // Held, it is pressed once.
        let (_, edges) = sample(&mut input, &mut dev, &play_at(feet));
        assert!(edges.is_empty());
        // A screen's click is the screen's.
        dev.button(MouseButton::Right, false);
        dev.button(MouseButton::Right, true);
        let (_, edges) = sample(&mut input, &mut dev, &Context { mode: Mode::Ui, feet: Some(feet) });
        assert!(edges.is_empty());
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
        assert!(f.free, "the right stick pushed aims freely");
        assert!(input.pad_active() && !input.aiming_with_mouse());
        // LB is the foe before now (bar 4 and 5 went to the stick clicks).
        assert_eq!(
            edges,
            vec![
                Edge::Game(GameAction::Use),
                Edge::Game(GameAction::Tab { back: true }),
                Edge::Game(GameAction::Bar(0))
            ]
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
            [UiAction::Down, UiAction::Cancel, UiAction::TabRight, UiAction::Confirm, UiAction::Cancel].map(Edge::Ui)
        );
    }

    #[test]
    fn the_left_stick_steps_through_a_screen_once_a_lean() {
        let mut input = Input::new();
        let ui = Context { mode: Mode::Ui, feet: None };
        let mut dev = DeviceState { pad: Some(Pad::default()), ..DeviceState::default() };
        let mut lean = |input: &mut Input, x: i16, y: i16| {
            dev.pad = Some(Pad { axes: [x, y, 0, 0, 0, 0], held: 0 });
            sample(input, &mut dev, &ui).1
        };
        assert_eq!(lean(&mut input, 0, 30_000), vec![Edge::Ui(UiAction::Down)]);
        assert!(lean(&mut input, 0, 31_000).is_empty(), "held, not again");
        assert!(lean(&mut input, 0, 2_000).is_empty());
        assert_eq!(lean(&mut input, -30_000, 4_000), vec![Edge::Ui(UiAction::Left)]);
        assert_eq!(lean(&mut input, 0, -30_000), vec![Edge::Ui(UiAction::Up)]);
        assert!(input.pad_active());
        // The mouse moving makes the mouse the device again: hints and the pointer follow it.
        dev.mouse.moved = true;
        let _ = sample(&mut input, &mut dev, &ui);
        assert!(!input.pad_active() && input.aiming_with_mouse());
    }

    #[test]
    fn the_reticle_sits_on_the_aims_own_side_at_every_bearing() {
        let chest = (320.0, 168.0);
        // The four axes, where Q15 reads exactly 1.0, and every bearing between.
        for (cursor, aim) in [
            ((400.0, 168.0), Angle::EAST),
            ((240.0, 168.0), Angle::WEST),
            ((320.0, 248.0), Angle::SOUTH),
            ((320.0, 88.0), Angle::NORTH),
        ] {
            let r = reticle_at(chest, cursor, aim);
            assert!(
                (r.0 - cursor.0 as i32).abs() <= 1 && (r.1 - cursor.1 as i32).abs() <= 1,
                "{aim:?}: {r:?} at the cursor {cursor:?}"
            );
        }
        for step in 0..256u16 {
            let a = Angle(step << 8);
            let dx = f64::from(jane_core::angle::cos_q15(a).0) * 80.0 / 32768.0;
            let dy = f64::from(jane_core::angle::sin_q15(a).0) * 80.0 / 32768.0;
            let cursor = (chest.0 + dx as f32, chest.1 + dy as f32);
            let r = reticle_at(chest, cursor, a);
            assert!((r.0 - cursor.0.round() as i32).abs() <= 1 && (r.1 - cursor.1.round() as i32).abs() <= 1, "{a:?}");
        }
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
    fn the_canvas_is_360_tall_and_a_whole_multiple_with_bars() {
        let f = |w, h| fit(w, h, Scaling::Whole);
        // 1080p is 3x exactly, 1440p 4x, 4K 6x, 720p 2x: no bars.
        for (w, h, k) in [(1920, 1080, 3.0), (2560, 1440, 4.0), (3840, 2160, 6.0), (1280, 720, 2.0)] {
            assert_eq!(f(w, h), Fit { canvas: (640, 360), scale: k, origin: (0, 0), size: (w, h) });
        }
        // 1366 x 768: 2x, a bar of 24 px above and below.
        assert_eq!(f(1366, 768), Fit { canvas: (683, 360), scale: 2.0, origin: (0, 24), size: (1366, 720) });
        // 21:9 widens the canvas at the same height; a part px is a bar.
        assert_eq!(f(3440, 1440), Fit { canvas: (860, 360), scale: 4.0, origin: (0, 0), size: (3440, 1440) });
        assert_eq!(f(2560, 1080).canvas, (853, 360));
        assert_eq!(f(2560, 1080).origin, (0, 0));
        // A tall window: the width decides, the rest is bars.
        assert_eq!(f(1300, 1000), Fit { canvas: (650, 360), scale: 2.0, origin: (0, 140), size: (1300, 720) });
        // Smaller than 1x: 1x, cropped about the middle.
        assert_eq!(f(600, 300).scale, 1.0);
        assert_eq!(f(800, 0).canvas, (640, 360));
        // The mouse through the bars: the canvas's corners.
        let g = f(1366, 768);
        assert_eq!(g.to_canvas(0, 24), (0.0, 0.0));
        assert_eq!(g.to_canvas(1366, 744), (683.0, 360.0));
        assert_eq!(g.to_canvas(683, 384), (341.5, 180.0));
    }

    #[test]
    fn filling_the_window_keeps_the_height_and_shows_what_is_wide() {
        let f = |w, h| fit(w, h, Scaling::Fill);
        assert_eq!(f(1280, 720).canvas, (640, 360));
        assert_eq!(f(1920, 1080).canvas, (640, 360));
        assert_eq!(f(1366, 768).canvas, (641, 360), "ceil: the last column is part-shown");
        assert_eq!(f(3440, 1440).canvas, (860, 360));
        let g = f(1600, 900);
        assert_eq!((g.scale, g.origin, g.size), (2.5, (0, 0), (1600, 900)));
        assert_eq!(g.to_canvas(1600, 900), (640.0, 360.0));
        assert_eq!(g.to_canvas(250, 100), (100.0, 40.0));
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
