//! The dialogue box (PRESENTATION.md §3.2): the speaker's name, the line revealed at three
//! characters a tick, at most two options, a "more" mark while a line continues, and hints for
//! the device in hand. It sends what `jane-bot` sends: `Advance` for the next line, `Choose` for
//! an option, `CloseDialogue` to walk away (Esc, or the pad's B). A press while the line is still typing shows the
//! rest of it first.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_sim::input::Command;

use crate::input::{Action, UiAction, sc};
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, PanelStyle, Ui, advance, line_h, text_w, wrap_lines};
use crate::ui::hud::{HudCtx, key_cap, key_cap_w};
use crate::ui::style::{self, argb};
use crate::view::DialogueView;

/// Characters revealed a tick (§3.2).
pub const REVEAL_PER_TICK: u32 = 3;
/// The action whose pad button goes on in a screen (A; the PSP's ×): `input::edge_for` hears bar
/// 1's pad button as Confirm there and Use's (B; the PSP's ○) as back.
pub const GO_PAD: Action = Action::Bar(0);
/// Ticks before a line that did not change may be pressed again.
const RESEND: u32 = 15;

/// The box's own state between frames: which line it is typing and since when.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DialogueBox {
    key: Option<(u32, usize, u16, u32)>,
    since: u32,
    /// Everything shown at once (a press while typing).
    all: bool,
    /// The option the keys have lit.
    pub focus: u8,
    /// The line the last command went for, and when: one command a line, so a double press is
    /// not two lines (again after `RESEND` ticks, for a choice that comes back to its own line).
    sent: Option<((u32, usize, u16, u32), u32)>,
}

impl DialogueBox {
    /// Characters of `d` shown at `tick`.
    pub fn shown(&self, d: &DialogueView, tick: u32) -> usize {
        if self.all || self.key != Some(d.key) {
            return if self.all { usize::MAX } else { 0 };
        }
        (tick.wrapping_sub(self.since).saturating_mul(REVEAL_PER_TICK)) as usize
    }

    /// Whether the whole line is out.
    pub fn done(&self, d: &DialogueView, tick: u32) -> bool {
        self.shown(d, tick) >= d.text.chars().count()
    }

    /// Follows the view: a new line starts typing.
    pub fn follow(&mut self, d: &DialogueView, tick: u32) {
        if self.key != Some(d.key) {
            self.key = Some(d.key);
            self.since = tick;
            self.all = false;
            self.focus = 0;
        }
    }

    pub fn reset(&mut self) {
        *self = DialogueBox::default();
    }
}

/// Draws the box for `d` and answers the keys and the pointer while it is on top.
pub fn draw(ui: &mut Ui, bx: &mut DialogueBox, d: &DialogueView, cx: HudCtx<'_>) {
    let tick = ui.tick;
    bx.follow(d, tick);
    let (cw, ch) = ui.canvas;
    let w = (cw - 48).min(620);
    let x = (cw - w) / 2;
    let text_cols = ((w - 48) / advance(Face::Small)) as usize;
    let text_lines = wrap_lines(&d.text, text_cols).count().max(2) as i32;
    let opt_h = if d.choosing { d.options.len() as i32 * 22 + 6 } else { 0 };
    let h = 24 + text_lines * line_h(Face::Small) + opt_h + 22;
    let y = ch - h - 70;
    let r = Rect::new(x, y, w, h);
    ui.panel(r, PanelStyle::Window);
    // Walking away, as Esc and the pad's B do.
    if ui.close_box(crate::ui::core::wid("dlg-close", 0), Rect::new(x + w - 24, y + 6, 16, 16)) && ui.interactive {
        ui.command(Command::CloseDialogue);
        return;
    }

    // The speaker on a plate over the frame's top edge.
    if !d.speaker.is_empty() {
        let sw = text_w(Face::Small, &d.speaker) + 24;
        let pr = Rect::new(x + 18, y - 11, sw, 22);
        ui.panel(pr, PanelStyle::Tip);
        ui.fill(Rect::new(x + 20, y - 9, sw - 4, 1), argb(style::gold(), 120));
        ui.text(x + 30, y - 9, &d.speaker, Ink::small(style::gold()).shadow());
    }

    // The line, typing.
    let shown = bx.shown(d, tick);
    let tr = Rect::new(x + 18, y + 16, w - 48, text_lines * line_h(Face::Small));
    ui.wrapped_reveal(tr, &d.text, Ink::small(style::text_bright()).shadow(), shown);
    let done = bx.done(d, tick);

    // The options, once the line is out.
    let mut chosen = None;
    if d.choosing && done {
        let oy = y + 18 + text_lines * line_h(Face::Small) + 4;
        for (i, o) in d.options.iter().enumerate() {
            let row = Rect::new(x + 14, oy + i as i32 * 22, w - 28, 20);
            let over = ui.hover(row);
            if over && ui.input.pointer.is_some() {
                bx.focus = i as u8;
            }
            let lit = bx.focus == i as u8;
            if lit {
                ui.fill(row, argb(Ramp::UiPanel.at(Tone::Light), 70));
                ui.fill(Rect::new(i32::from(row.x), i32::from(row.y), 2, 20), argb(style::gold(), 230));
            }
            // The number to press, as a small cap.
            let n = if i == 0 { "1" } else { "2" };
            let cap = Rect::new(i32::from(row.x) + 8, i32::from(row.y) + 2, 14, 16);
            ui.fill(cap, argb(if lit { style::gold() } else { Ramp::UiInk.at(Tone::Mid) }, 255));
            ui.text(i32::from(cap.x) + 3, i32::from(cap.y) + 2, n, Ink::fine(style::INK));
            let ink = if lit { style::text_bright() } else { style::text() };
            ui.text(i32::from(row.x) + 30, i32::from(row.y) + 1, o, Ink::small(ink).shadow());
            if over && ui.input.released {
                chosen = Some(i as u8);
            }
            ui.claim(row);
        }
    }

    // The mark that says a press goes on: a bobbing arrow while more follows, a square at the end.
    if done && !d.choosing {
        let bob = ((tick / 8) % 4) as i32;
        let bob = if bob == 3 { 1 } else { bob };
        let (mx, my) = (x + w - 22, y + h - 20 + bob);
        let glyph = if d.more { "▼" } else { "■" };
        ui.text(mx, my, glyph, Ink::fine(style::gold()).shadow());
    }

    // Hints for the device in hand, bottom right, quiet.
    let hint = if d.choosing && done {
        "Choose"
    } else if !done {
        "Skip"
    } else if d.more {
        "Next"
    } else {
        "Close"
    };
    let hy = y + h - 22;
    if cx.pad {
        // A pad goes on with its confirm button and backs out with Use's (`input::edge_for`: in a
        // screen A, the PSP's ×, confirms; B, the PSP's ○, is back), so the hints name those, not
        // Use's button as the keys' hint does.
        let (go, leave) = (GO_PAD, Action::Use);
        let hx = x + w - 36 - text_w(Face::Fine, hint) - key_cap_w(cx, go);
        key_cap(ui, hx, hy, cx, go);
        ui.text(hx + key_cap_w(cx, go) + 4, hy + 4, hint, Ink::fine(style::quiet()).shadow());
        let mut lx = hx;
        if d.choosing && done {
            let s = "↑↓";
            lx -= text_w(Face::Fine, s) + 6;
            ui.text(lx, hy + 4, s, Ink::fine(style::quiet()).shadow());
        }
        let l = "Leave";
        lx -= 14 + key_cap_w(cx, leave) + 4 + text_w(Face::Fine, l);
        key_cap(ui, lx, hy, cx, leave);
        ui.text(lx + key_cap_w(cx, leave) + 4, hy + 4, l, Ink::fine(style::quiet()).shadow());
    } else if d.choosing && done {
        let s = if cx.pad { "↑↓ and" } else { "1, 2 or" };
        let hx = x + w - 30 - text_w(Face::Fine, s) - key_cap_w(cx, Action::Use) - 8;
        ui.text(hx, hy + 4, s, Ink::fine(style::quiet()).shadow());
        key_cap(ui, hx + text_w(Face::Fine, s) + 6, hy, cx, Action::Use);
    } else {
        let hx = x + w - 36 - text_w(Face::Fine, hint) - key_cap_w(cx, Action::Use);
        key_cap(ui, hx, hy, cx, Action::Use);
        ui.text(hx + key_cap_w(cx, Action::Use) + 4, hy + 4, hint, Ink::fine(style::quiet()).shadow());
    }

    if !ui.interactive {
        return;
    }
    // Keys: Use, Confirm, Space and Enter go on; 1 and 2 pick; up and down move the light; back
    // (Esc, the pad's B) walks away.
    let n = d.options.len() as u8;
    let mut go = false;
    for a in ui.input.actions.clone() {
        match a {
            UiAction::Confirm => go = true,
            UiAction::Cancel => {
                ui.command(Command::CloseDialogue);
                return;
            }
            UiAction::Up if n > 0 => bx.focus = (bx.focus + n - 1) % n,
            UiAction::Down if n > 0 => bx.focus = (bx.focus + 1) % n,
            _ => {}
        }
    }
    if d.choosing && done {
        for (k, i) in [(sc::N1, 0u8), (sc::N1 + 1, 1)] {
            if ui.input.keys.has(k) && i < n {
                chosen = Some(i);
            }
        }
    }
    // A click on the box itself goes on too.
    let clicked_box = ui.input.released && ui.input.pointer.is_some_and(|p| r.contains(p)) && chosen.is_none();
    if clicked_box && !(d.choosing && done) {
        go = true;
    }
    if go && !done {
        bx.all = true;
        return;
    }
    if bx.sent.is_some_and(|(k, t)| k == d.key && tick.wrapping_sub(t) < RESEND) {
        return;
    }
    if let Some(o) = chosen {
        bx.sent = Some((d.key, tick));
        ui.command(Command::Choose { option: o });
    } else if go {
        bx.sent = Some((d.key, tick));
        if d.choosing {
            ui.command(Command::Choose { option: bx.focus.min(n.saturating_sub(1)) });
        } else {
            ui.command(Command::Advance);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Bindings;
    use crate::ui::core::UiInput;
    use crate::ui::{UiArt, UiOut};

    fn line(text: &str, choosing: bool) -> DialogueView {
        DialogueView {
            speaker: "The dog".into(),
            text: text.into(),
            options: if choosing { vec!["Yes".into(), "No".into()] } else { vec![] },
            choosing,
            more: !choosing,
            key: (1, 2, 0, 3),
        }
    }

    fn frame(ui: &mut Ui, bx: &mut DialogueBox, d: &DialogueView, input: UiInput, tick: u32) -> Vec<UiOut> {
        let b = Bindings::default();
        ui.begin(input, tick, (768, 432));
        draw(ui, bx, d, HudCtx { bindings: &b, pad: false, window_open: false, style: crate::input::PadStyle::Xbox });
        ui.out.clone()
    }

    fn confirm() -> UiInput {
        UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }
    }

    #[test]
    fn the_line_types_at_three_a_tick_and_a_press_shows_the_rest_first() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut bx = DialogueBox::default();
        let d = line("It was not there yesterday.", false);
        frame(&mut ui, &mut bx, &d, UiInput::default(), 100);
        assert_eq!(bx.shown(&d, 100), 0);
        assert_eq!(bx.shown(&d, 104), 12);
        // A press mid-line shows it all and sends nothing.
        assert!(frame(&mut ui, &mut bx, &d, confirm(), 104).is_empty());
        assert!(bx.done(&d, 104));
        // The next press advances, once.
        assert_eq!(frame(&mut ui, &mut bx, &d, confirm(), 105), vec![UiOut::Command(Command::Advance)]);
        assert!(frame(&mut ui, &mut bx, &d, confirm(), 106).is_empty(), "one command a line");
        // A new line types again from nothing.
        let next = DialogueView { key: (1, 2, 1, 3), ..d };
        frame(&mut ui, &mut bx, &next, UiInput::default(), 200);
        assert_eq!(bx.shown(&next, 200), 0);
    }

    #[test]
    fn a_choice_is_chosen_by_number_by_light_or_by_click() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut bx = DialogueBox::default();
        let d = line("Well?", true);
        frame(&mut ui, &mut bx, &d, UiInput::default(), 1);
        let mut keys = crate::input::KeySet::EMPTY;
        keys.set(sc::N1 + 1, true);
        let got = frame(&mut ui, &mut bx, &d, UiInput { keys, ..UiInput::default() }, 50);
        assert_eq!(got, vec![UiOut::Command(Command::Choose { option: 1 })]);
        let mut bx = DialogueBox::default();
        frame(&mut ui, &mut bx, &d, UiInput::default(), 1);
        frame(&mut ui, &mut bx, &d, UiInput { actions: vec![UiAction::Down], ..UiInput::default() }, 50);
        assert_eq!(bx.focus, 1);
        let got = frame(&mut ui, &mut bx, &d, confirm(), 51);
        assert_eq!(got, vec![UiOut::Command(Command::Choose { option: 1 })]);
    }
}
