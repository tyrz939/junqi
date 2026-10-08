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
}
