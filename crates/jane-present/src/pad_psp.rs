//! The PSP's pad onto the standard mapping the bindings' pad column names (PORT.md §13.13). It
//! lives here, not in the spike, so the host's tests drive it (the spike builds only for the
//! PSP); `spikes/psp-game`'s shell calls it once a frame.

use crate::input::{Mode, Pad, pad};

/// The PSP's buttons this frame (`sceCtrl`'s bits, rust-psp's `CtrlButtons`), and the stick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PspPad {
    pub buttons: u32,
    /// The stick, 0..=255 a side, 128 the middle.
    pub lx: u8,
    pub ly: u8,
}

impl PspPad {
    /// Buttons `b` held, the stick in the middle.
    pub const fn buttons(b: u32) -> PspPad {
        PspPad { buttons: b, lx: 128, ly: 128 }
    }
}

/// `sceCtrl`'s button bits.
pub mod psp {
    pub const SELECT: u32 = 0x1;
    pub const START: u32 = 0x8;
    pub const UP: u32 = 0x10;
    pub const RIGHT: u32 = 0x20;
    pub const DOWN: u32 = 0x40;
    pub const LEFT: u32 = 0x80;
    pub const L: u32 = 0x100;
    pub const R: u32 = 0x200;
    pub const TRIANGLE: u32 = 0x1000;
    pub const CIRCLE: u32 = 0x2000;
    pub const CROSS: u32 = 0x4000;
    pub const SQUARE: u32 = 0x8000;
    /// The script's names for them.
    pub const NAMES: [(&str, u32); 12] = [
        ("select", SELECT),
        ("start", START),
        ("up", UP),
        ("right", RIGHT),
        ("down", DOWN),
        ("left", LEFT),
        ("l", L),
        ("r", R),
        ("triangle", TRIANGLE),
        ("circle", CIRCLE),
        ("cross", CROSS),
        ("square", SQUARE),
    ];
}

/// The PSP pad as the standard mapping the bindings' pad column names (PORT.md §13.13). Face
/// buttons by place (cross A, circle B, square X, triangle Y), SELECT the View button, START
/// Menu; in play L and R are the triggers (hop, sprint) and the d-pad the shoulders and the
/// stick clicks (target back and next, bar 4 and 5); in a screen L and R are the shoulders (the
/// tabs) and the d-pad the d-pad (a step). In a screen A confirms and B backs out
/// (`input::edge_for`), so cross goes on through a conversation and circle leaves it.
pub fn route(p: PspPad, mode: Mode) -> Pad {
    let on = |b: u32| p.buttons & b != 0;
    let mut held = 0u32;
    let mut set = |b: u8, v: bool| {
        if v {
            held |= 1 << b;
        }
    };
    set(pad::A, on(psp::CROSS));
    set(pad::B, on(psp::CIRCLE));
    set(pad::X, on(psp::SQUARE));
    set(pad::Y, on(psp::TRIANGLE));
    set(pad::BACK, on(psp::SELECT));
    set(pad::START, on(psp::START));
    let mut axes = [0i16; 6];
    let stick = |v: u8| ((i32::from(v) - 128) * 256).clamp(-32_767, 32_767) as i16;
    (axes[0], axes[1]) = (stick(p.lx), stick(p.ly));
    if mode == Mode::Play {
        axes[4] = if on(psp::L) { 32_767 } else { 0 };
        axes[5] = if on(psp::R) { 32_767 } else { 0 };
        set(pad::LB, on(psp::LEFT));
        set(pad::RB, on(psp::RIGHT));
        set(pad::LSTICK, on(psp::UP));
        set(pad::RSTICK, on(psp::DOWN));
    } else {
        set(pad::LB, on(psp::L));
        set(pad::RB, on(psp::R));
        set(pad::DPAD_UP, on(psp::UP));
        set(pad::DPAD_DOWN, on(psp::DOWN));
        set(pad::DPAD_LEFT, on(psp::LEFT));
        set(pad::DPAD_RIGHT, on(psp::RIGHT));
    }
    Pad { axes, held }
}

/// What walks her (the console's Controls page; a worn stick drifts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WalkWith {
    /// The stick walks; the d-pad targets (left, right) and plays bar 4 and 5 (up, down).
    #[default]
    Stick,
    /// The d-pad walks, 8 ways at full speed, and the stick is not read at all (in play or in a
    /// screen); L with the d-pad targets and plays bar 4 and 5, and a tap of L hops.
    Dpad,
    /// Both walk; the d-pad's other work is L with it, as `Dpad`.
    Both,
}

impl WalkWith {
    pub const ALL: [WalkWith; 3] = [WalkWith::Stick, WalkWith::Dpad, WalkWith::Both];

    pub fn label(self) -> &'static str {
        match self {
            WalkWith::Stick => "Stick",
            WalkWith::Dpad => "D-pad",
            WalkWith::Both => "Both",
        }
    }

    /// The settings file's word for it, and back.
    pub fn from_key(k: &str) -> Option<WalkWith> {
        WalkWith::ALL.into_iter().find(|v| v.key() == k)
    }

    pub fn key(self) -> &'static str {
        match self {
            WalkWith::Stick => "stick",
            WalkWith::Dpad => "dpad",
            WalkWith::Both => "both",
        }
    }

    /// The d-pad walks (its targets and bar 4 and 5 are then L with it).
    pub fn dpad_walks(self) -> bool {
        self != WalkWith::Stick
    }

    /// The stick is read.
    pub fn stick(self) -> bool {
        self != WalkWith::Dpad
    }
}

/// How far the stick must lean before it is read (a worn stick rests off its middle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeadZone {
    /// 12% of its throw.
    Low,
    /// 20%, the PC's (`input::STICK_DEADZONE`): as the PSP always read it.
    #[default]
    Medium,
    /// 35%.
    High,
}

impl DeadZone {
    pub const ALL: [DeadZone; 3] = [DeadZone::Low, DeadZone::Medium, DeadZone::High];

    pub fn label(self) -> &'static str {
        match self {
            DeadZone::Low => "Low",
            DeadZone::Medium => "Medium",
            DeadZone::High => "High",
        }
    }

    /// The settings file's word for it, and back.
    pub fn from_key(k: &str) -> Option<DeadZone> {
        DeadZone::ALL.into_iter().find(|v| v.key() == k)
    }

    pub fn key(self) -> &'static str {
        match self {
            DeadZone::Low => "low",
            DeadZone::Medium => "medium",
            DeadZone::High => "high",
        }
    }

    /// The radius, of 32 767.
    pub fn radius(self) -> i32 {
        match self {
            DeadZone::Low => 32_767 * 12 / 100,
            DeadZone::Medium => DZ_INPUT,
            DeadZone::High => 32_767 * 35 / 100,
        }
    }
}

/// The input mapper's own radial dead zone (`input::STICK_DEADZONE`, a fifth of the throw).
const DZ_INPUT: i32 = 32_767 / 5;

/// The pad's settings (the console's Controls page; `settings.txt` on the stick).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadSettings {
    pub walk: WalkWith,
    pub dead: DeadZone,
}

impl PadSettings {
    /// The hints' pad: the PSP's, its d-pad's play moved under L when the d-pad walks.
    pub fn style(self) -> crate::input::PadStyle {
        if self.walk.dpad_walks() { crate::input::PadStyle::PspDpad } else { crate::input::PadStyle::Psp }
    }
}

/// The console's settings file (`ms0:/PSP/SAVEDATA/JANE00001/settings.txt`): `key value` lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub pad: PadSettings,
    pub volumes: crate::audio::Volumes,
}

impl Settings {
    pub fn write(&self) -> alloc::string::String {
        let v = self.volumes;
        alloc::format!(
            "walk {}\ndead {}\nmaster {}\nmusic {}\nsfx {}\n",
            self.pad.walk.key(),
            self.pad.dead.key(),
            v.master,
            v.music,
            v.sfx
        )
    }

    /// Reads what it knows; anything missing or unread keeps its default.
    pub fn read(s: &str) -> Settings {
        let mut out = Settings::default();
        for line in s.lines() {
            let (k, v) = line.trim().split_once(' ').unwrap_or((line, ""));
            let pct = |d: u8| v.parse::<u8>().map_or(d, |n| n.min(100));
            match k {
                "walk" => out.pad.walk = WalkWith::from_key(v).unwrap_or_default(),
                "dead" => out.pad.dead = DeadZone::from_key(v).unwrap_or_default(),
                "master" => out.volumes.master = pct(out.volumes.master),
                "music" => out.volumes.music = pct(out.volumes.music),
                "sfx" => out.volumes.sfx = pct(out.volumes.sfx),
                _ => {}
            }
        }
        out
    }
}

/// Frames L may be held and still be a tap (a hop), about 200 ms.
pub const L_TAP_FRAMES: u32 = 12;

const DPAD: u32 = psp::UP | psp::DOWN | psp::LEFT | psp::RIGHT;

/// The PSP pad onto the standard mapping with the player's [`PadSettings`], once a frame. With
/// the stick alone it is [`route`] (the stick through the dead zone chosen). When the d-pad walks,
/// in play: the d-pad is the d-pad (walk, 8 ways, full speed); a d-pad direction pressed while L
/// is held is instead what the d-pad does with the stick walking (left, right the foe before and
/// the next; up, down bar 4 and 5), and a tap of L (let go within [`L_TAP_FRAMES`], nothing else
/// pressed meanwhile) hops on its release. In `Dpad` the stick is never read. A screen is
/// [`route`]'s, the stick left out in `Dpad`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PspRouter {
    pub settings: PadSettings,
    was: u32,
    /// Frames L has been held, while it is.
    l_frames: u32,
    /// Something else was pressed while L was held: not a tap.
    l_used: bool,
    /// The d-pad directions pressed while L was held (L's, until each is let go).
    l_dirs: u32,
}

impl PspRouter {
    pub fn new(settings: PadSettings) -> PspRouter {
        PspRouter { settings, ..PspRouter::default() }
    }

    pub fn route(&mut self, p: PspPad, mode: Mode) -> Pad {
        let s = self.settings;
        let rose = p.buttons & !self.was;
        self.was = p.buttons;
        let mut q = p;
        // The stick: left out, or through the dead zone chosen.
        (q.lx, q.ly) = if s.walk.stick() { dead_zone(p.lx, p.ly, s.dead) } else { (128, 128) };
        let l = p.buttons & psp::L != 0;
        let mut hop = false;
        if l {
            if rose & !psp::L != 0 {
                self.l_used = true;
            }
            self.l_frames = self.l_frames.saturating_add(1);
        } else {
            hop = self.l_frames > 0 && self.l_frames <= L_TAP_FRAMES && !self.l_used;
            (self.l_frames, self.l_used) = (0, false);
        }
        if l {
            self.l_dirs |= rose & DPAD;
        }
        self.l_dirs &= p.buttons;
        if mode != Mode::Play || !s.walk.dpad_walks() {
            self.l_dirs = 0;
            return route(q, mode);
        }
        // Play with the d-pad walking: the d-pad and L's taps by hand; the rest as `route`.
        q.buttons &= !(DPAD | psp::L);
        let mut pad_ = route(q, mode);
        let mut set = |b: u8, v: bool| {
            if v {
                pad_.held |= 1 << b;
            }
        };
        let walk = p.buttons & !self.l_dirs;
        let mine = self.l_dirs;
        set(pad::DPAD_UP, walk & psp::UP != 0);
        set(pad::DPAD_DOWN, walk & psp::DOWN != 0);
        set(pad::DPAD_LEFT, walk & psp::LEFT != 0);
        set(pad::DPAD_RIGHT, walk & psp::RIGHT != 0);
        set(pad::LB, mine & psp::LEFT != 0);
        set(pad::RB, mine & psp::RIGHT != 0);
        set(pad::LSTICK, mine & psp::UP != 0);
        set(pad::RSTICK, mine & psp::DOWN != 0);
        // The hop: the left trigger for the one frame L is let go after a tap.
        pad_.axes[4] = if hop { 32_767 } else { 0 };
        pad_
    }
}

/// The PSP's stick reads 0 to 255, so its rim is 127 steps of 256 out.
const RIM: i32 = 127 * 256;

/// The stick through a radial dead zone: inside it, the middle; past it, rescaled so the edge of
/// the zone lands just past the input mapper's own fifth and the rim stays the rim. `Medium` is
/// the mapper's own: the stick as read.
pub fn dead_zone(lx: u8, ly: u8, dz: DeadZone) -> (u8, u8) {
    if dz == DeadZone::Medium {
        return (lx, ly);
    }
    let (x, y) = ((i32::from(lx) - 128) * 256, (i32::from(ly) - 128) * 256);
    let mag = jane_core::num::isqrt((i64::from(x) * i64::from(x) + i64::from(y) * i64::from(y)) as u64) as i32;
    let r = dz.radius();
    if mag <= r {
        return (128, 128);
    }
    let lo = DZ_INPUT + 256;
    let out = lo + (mag.min(RIM) - r) * (RIM - lo) / (RIM - r);
    let k = |v: i32| (128 + v * out / mag / 256).clamp(0, 255) as u8;
    (k(x), k(y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Action, Bindings, Context, DeviceState, Edge, GameAction, Input, PadStyle, UiAction};
    use crate::ui::core::UiInput;
    use crate::ui::dialogue::{self, DialogueBox, GO_PAD};
    use crate::ui::hud::HudCtx;
    use crate::ui::{Ui, UiArt, UiOut};
    use crate::view::DialogueView;
    use jane_sim::input::Command;

    /// One frame of the PSP pad through the mapper, as the shell samples it: its edges.
    fn frame(input: &mut Input, dev: &mut DeviceState, p: PspPad, mode: Mode) -> Vec<Edge> {
        dev.pad = Some(route(p, mode));
        input.sample(dev, &Context { mode, feet: None });
        dev.end_sample();
        input.drain().collect()
    }

    fn ui_actions(edges: &[Edge]) -> Vec<UiAction> {
        edges.iter().filter_map(|e| if let Edge::Ui(a) = e { Some(*a) } else { None }).collect()
    }

    fn line(choosing: bool) -> DialogueView {
        DialogueView {
            speaker: "Julie".into(),
            text: "It was not there yesterday.".into(),
            options: if choosing { vec!["I'll take it".into(), "Not now".into()] } else { vec![] },
            choosing,
            more: !choosing,
            key: (1, 2, 0, 3),
        }
    }

    /// The box drawn on a PSP's canvas with the PSP's hints, fed this frame's actions.
    fn talk(ui: &mut Ui, bx: &mut DialogueBox, d: &DialogueView, actions: Vec<UiAction>, tick: u32) -> Vec<UiOut> {
        let b = Bindings::default();
        ui.begin(UiInput { actions, pad: true, ..UiInput::default() }, tick, (480, 272));
        dialogue::draw(ui, bx, d, HudCtx { bindings: &b, pad: true, window_open: false, style: PadStyle::Psp });
        ui.out.clone()
    }

    /// A tap of `b` in a conversation (a screen), then its release: the UI's actions.
    fn tap(input: &mut Input, dev: &mut DeviceState, b: u32) -> Vec<UiAction> {
        let got = ui_actions(&frame(input, dev, PspPad::buttons(b), Mode::Ui));
        frame(input, dev, PspPad::buttons(0), Mode::Ui);
        got
    }

    #[test]
    fn the_hints_name_the_buttons_that_do_it() {
        let b = Bindings::default();
        let (go, leave) = (b.pad(GO_PAD).unwrap(), b.pad(Action::Use).unwrap());
        assert_eq!((PadStyle::Psp.name(go), PadStyle::Psp.name(leave)), ("×", "○"));
        assert_eq!((PadStyle::Xbox.name(go), PadStyle::Xbox.name(leave)), ("A", "B"));
        // And the mapper hears them so: × goes on, ○ backs out.
        let (mut input, mut dev) = (Input::new(), DeviceState::default());
        assert_eq!(tap(&mut input, &mut dev, psp::CROSS), [UiAction::Confirm]);
        assert_eq!(tap(&mut input, &mut dev, psp::CIRCLE), [UiAction::Cancel]);
    }

    #[test]
    fn cross_reveals_then_goes_on_and_circle_leaves() {
        let (mut input, mut dev) = (Input::new(), DeviceState::default());
        let mut ui = Ui::new(UiArt::build(1).0);
        ui.pad_style = PadStyle::Psp;
        let mut bx = DialogueBox::default();
        let d = line(false);
        // ○ in play starts the talk (Use), and held on into the conversation it is not a press.
        let e = frame(&mut input, &mut dev, PspPad::buttons(psp::CIRCLE), Mode::Play);
        assert_eq!(e, [Edge::Game(GameAction::Use)]);
        let held = frame(&mut input, &mut dev, PspPad::buttons(psp::CIRCLE), Mode::Ui);
        assert!(talk(&mut ui, &mut bx, &d, ui_actions(&held), 100).is_empty(), "the talk's ○ closes nothing");
        frame(&mut input, &mut dev, PspPad::buttons(0), Mode::Ui);
        // × while it types shows the rest and sends nothing.
        let a = tap(&mut input, &mut dev, psp::CROSS);
        assert!(talk(&mut ui, &mut bx, &d, a, 102).is_empty());
        assert!(bx.done(&d, 102));
        // × again goes on.
        let a = tap(&mut input, &mut dev, psp::CROSS);
        assert_eq!(talk(&mut ui, &mut bx, &d, a, 103), [UiOut::Command(Command::Advance)]);
        // ○ leaves.
        let a = tap(&mut input, &mut dev, psp::CIRCLE);
        assert_eq!(talk(&mut ui, &mut bx, &d, a, 104), [UiOut::Command(Command::CloseDialogue)]);
    }

    #[test]
    fn the_dpad_or_the_stick_lights_an_option_and_cross_chooses_it() {
        for by_stick in [false, true] {
            let (mut input, mut dev) = (Input::new(), DeviceState::default());
            let mut ui = Ui::new(UiArt::build(1).0);
            let mut bx = DialogueBox::default();
            let d = line(true);
            talk(&mut ui, &mut bx, &d, vec![], 1);
            let a = if by_stick {
                let a = ui_actions(&frame(&mut input, &mut dev, PspPad { buttons: 0, lx: 128, ly: 255 }, Mode::Ui));
                frame(&mut input, &mut dev, PspPad::buttons(0), Mode::Ui);
                a
            } else {
                tap(&mut input, &mut dev, psp::DOWN)
            };
            assert_eq!(a, [UiAction::Down]);
            talk(&mut ui, &mut bx, &d, a, 50);
            assert_eq!(bx.focus, 1);
            let a = tap(&mut input, &mut dev, psp::UP);
            talk(&mut ui, &mut bx, &d, a, 51);
            assert_eq!(bx.focus, 0);
            let a = tap(&mut input, &mut dev, psp::CROSS);
            assert_eq!(talk(&mut ui, &mut bx, &d, a, 52), [UiOut::Command(Command::Choose { option: 0 })]);
        }
    }

    /// One frame through a router with `walk` and `dead`: what she holds and the edges.
    struct Rig {
        r: PspRouter,
        input: Input,
        dev: DeviceState,
    }

    impl Rig {
        fn new(walk: WalkWith, dead: DeadZone) -> Rig {
            Rig { r: PspRouter::new(PadSettings { walk, dead }), input: Input::new(), dev: DeviceState::default() }
        }
        fn frame(&mut self, p: PspPad, mode: Mode) -> (jane_sim::input::InputFrame, Vec<Edge>) {
            self.dev.pad = Some(self.r.route(p, mode));
            let f = self.input.sample(&self.dev, &Context { mode, feet: None });
            self.dev.end_sample();
            (f, self.input.drain().collect())
        }
        fn play(&mut self, b: u32) -> (jane_sim::input::InputFrame, Vec<Edge>) {
            self.frame(PspPad::buttons(b), Mode::Play)
        }
    }

    /// The stick leaned `pct` of its throw toward 0 degrees (east).
    fn lean(pct: i32) -> PspPad {
        PspPad { buttons: 0, lx: (128 + 127 * pct / 100) as u8, ly: 128 }
    }

    #[test]
    fn the_stick_alone_at_medium_is_the_old_route() {
        let mut r = PspRouter::default();
        for mode in [Mode::Play, Mode::Ui] {
            for b in [0, psp::L, psp::LEFT | psp::UP, psp::CROSS | psp::R, psp::DOWN | psp::RIGHT | psp::SELECT] {
                for (lx, ly) in [(128, 128), (200, 90), (0, 255), (150, 128)] {
                    let p = PspPad { buttons: b, lx, ly };
                    assert_eq!(r.route(p, mode), route(p, mode));
                }
            }
        }
    }

    #[test]
    fn the_dpad_walks_eight_ways_at_full_speed_and_the_stick_is_not_read() {
        let mut g = Rig::new(WalkWith::Dpad, DeadZone::Medium);
        let (f, e) = g.play(psp::UP | psp::LEFT);
        assert_eq!(f.mv_mag, 127, "full speed");
        assert!(e.is_empty(), "walking is not a press: {e:?}");
        let (f, _) = g.play(psp::RIGHT);
        assert_eq!((f.mv_mag, f.mv_dir), (127, jane_core::Angle::from_degrees(0)));
        // The stick leaned all the way: nothing, in play or in a screen.
        let (f, _) = g.frame(PspPad { buttons: 0, lx: 255, ly: 0 }, Mode::Play);
        assert_eq!(f.mv_mag, 0);
        let (_, e) = g.frame(PspPad { buttons: 0, lx: 128, ly: 255 }, Mode::Ui);
        assert!(e.is_empty(), "the stick steps nothing in a screen: {e:?}");
        // The d-pad still steps a screen.
        g.frame(PspPad::buttons(0), Mode::Ui);
        let (_, e) = g.frame(PspPad::buttons(psp::DOWN), Mode::Ui);
        assert_eq!(e, [Edge::Ui(UiAction::Down)]);
        // Both: the stick and the d-pad walk.
        let mut g = Rig::new(WalkWith::Both, DeadZone::Medium);
        assert!(g.frame(lean(100), Mode::Play).0.mv_mag >= 126, "the stick's rim (127 of 128 steps)");
        g.play(0);
        assert_eq!(g.play(psp::DOWN).0.mv_mag, 127);
    }

    #[test]
    fn l_with_the_dpad_targets_and_plays_bar_4_and_5_and_a_tap_of_l_hops() {
        for walk in [WalkWith::Dpad, WalkWith::Both] {
            let mut g = Rig::new(walk, DeadZone::Medium);
            // L held, then right: the next foe; up: bar 4; down: bar 5; left: the foe before.
            assert!(g.play(psp::L).1.is_empty());
            let (f, e) = g.play(psp::L | psp::RIGHT);
            assert_eq!(e, [Edge::Game(GameAction::Tab { back: false })]);
            assert_eq!(f.mv_mag, 0, "L's d-pad does not walk");
            assert_eq!(g.play(psp::L | psp::UP).1, [Edge::Game(GameAction::Bar(3))]);
            assert_eq!(g.play(psp::L | psp::DOWN).1, [Edge::Game(GameAction::Bar(4))]);
            assert_eq!(g.play(psp::L | psp::LEFT).1, [Edge::Game(GameAction::Tab { back: true })]);
            // Let go of L after using it: no hop.
            assert!(g.play(0).1.is_empty());
            // A tap of L hops, on its release.
            for _ in 0..L_TAP_FRAMES {
                assert!(g.play(psp::L).1.is_empty());
            }
            assert_eq!(g.play(0).1, [Edge::Game(GameAction::Hop)]);
            // Held longer: no hop.
            for _ in 0..=L_TAP_FRAMES {
                g.play(psp::L);
            }
            assert!(g.play(0).1.is_empty());
            // Walking by the d-pad and tapping L: a hop, and the walk goes on.
            g.play(psp::UP);
            let (f, e) = g.play(psp::UP | psp::L);
            assert!(e.is_empty() && f.mv_mag == 127, "a direction held before L still walks");
            assert_eq!(g.play(psp::UP).1, [Edge::Game(GameAction::Hop)]);
        }
        // The stick alone: the d-pad targets as it did, L hops on its press.
        let mut g = Rig::new(WalkWith::Stick, DeadZone::Medium);
        assert_eq!(g.play(psp::RIGHT).1, [Edge::Game(GameAction::Tab { back: false })]);
        assert_eq!(g.play(psp::L).1, [Edge::Game(GameAction::Hop)]);
    }

    #[test]
    fn the_dead_zone_keeps_a_worn_sticks_rest_still() {
        // A stick resting at 25% of its throw: Medium (and the old route) walk on it, High does not.
        for (dead, pct, walks) in [
            (DeadZone::Medium, 25, true),
            (DeadZone::High, 25, false),
            (DeadZone::High, 45, true),
            (DeadZone::Low, 15, true),
            (DeadZone::Medium, 15, false),
            (DeadZone::Low, 10, false),
        ] {
            let mut g = Rig::new(WalkWith::Stick, dead);
            let (f, _) = g.frame(lean(pct), Mode::Play);
            assert_eq!(f.mv_mag > 0, walks, "{dead:?} at {pct}%");
        }
        // Pushed all the way it is all the way.
        assert_eq!(dead_zone(255, 128, DeadZone::High), (255, 128));
        assert_eq!(dead_zone(0, 128, DeadZone::Low), (1, 128));
    }

    #[test]
    fn the_settings_file_reads_back_and_shrugs_off_junk() {
        let s = Settings {
            pad: PadSettings { walk: WalkWith::Dpad, dead: DeadZone::High },
            volumes: crate::audio::Volumes { master: 40, music: 0, sfx: 100 },
        };
        assert_eq!(Settings::read(&s.write()), s);
        assert_eq!(Settings::read(""), Settings::default());
        assert_eq!(Settings::read("walk sideways\nmaster 250\nwho knows\n").volumes.master, 100);
        assert_eq!(PadSettings::default().walk, WalkWith::Stick, "the stick walks unless the player says");
        assert_eq!(s.pad.style(), PadStyle::PspDpad);
        assert_eq!(PadStyle::PspDpad.name(Bindings::default().pad(Action::Bar(3)).unwrap()), "L+↑");
    }
}
