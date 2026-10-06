//! The terminal (PRESENTATION.md §3.2; ENGINE.md §12): backquote opens it over the top third; a
//! ring of 400 lines, a history of 100, tab completion over the console's rows and their
//! arguments' names. Every line entered leaves as `AppIntent::Console`; the app runs it, and any
//! change it makes to the world is a `Command::Dev` like any other command.

use std::collections::VecDeque;

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};

use crate::input::sc;
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, Ink, Ui, line_h, text_w};
use crate::ui::style::{self, argb};

/// Lines kept.
pub const RING: usize = 400;
/// Lines of history.
pub const HISTORY: usize = 100;

/// The console's rows (ENGINE.md §12), with what each takes, for `help` and completion.
pub const ROWS: [(&str, &str); 28] = [
    ("help", "this list"),
    ("give", "<item> [qty]"),
    ("god", "[on|off]"),
    ("tp", "<zone> [mark]"),
    ("time", "<hour>"),
    ("hp", "<points>"),
    ("mp", "<points>"),
    ("learn", "<spell>"),
    ("quest", "<quest>"),
    ("flag", "<name> <value>"),
    ("kill", "what she can see within 25 m"),
    ("spawn", "<unit>"),
    ("save", "[slot 1-3]"),
    ("load", "[slot 1-3]"),
    ("seed", "the county's seed"),
    ("hash", "the state hash"),
    ("pos", "where she stands"),
    ("inst", "units and props in her zone"),
    ("speed", "<0.25|1|4> or hold|step"),
    ("replay", "(not here: jane replay)"),
    ("ver", "the build and its content"),
    ("title", "back to the title"),
    ("party", "who is sitting down"),
    ("open", "let others sit down"),
    ("close", "stop letting them"),
    ("join", "[token] an idle body sits down (open first)"),
    ("leave", "<seat> she gets up"),
    ("clear", "empty the screen"),
];

/// A line's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// What was typed.
    Input,
    Out,
    Good,
    Error,
}

/// The terminal's state.
#[derive(Clone, Debug, Default)]
pub struct Console {
    pub open: bool,
    pub input: String,
    lines: VecDeque<(String, LineKind)>,
    history: Vec<String>,
    /// Where in the history up and down have walked to.
    hist_at: Option<usize>,
    /// Lines scrolled back.
    scroll: usize,
    /// The tick it opened, for the slide.
    opened: u32,
}

impl Console {
    pub fn toggle(&mut self, tick: u32) {
        self.open = !self.open;
        if self.open {
            self.opened = tick;
            if self.lines.is_empty() {
                self.say("Type help for the list. Tab completes.", LineKind::Out);
            }
        }
    }

    pub fn say(&mut self, s: &str, kind: LineKind) {
        for l in s.split('\n') {
            // Long lines wrap at the terminal's width (96 columns of the Fine face at 16:9).
            let mut first = true;
            for part in crate::ui::core::wrap_lines(l, 94) {
                if self.lines.len() >= RING {
                    self.lines.pop_front();
                }
                let text = if first { part.to_owned() } else { format!("  {part}") };
                first = false;
                self.lines.push_back((text, kind));
            }
        }
        self.scroll = 0;
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn lines(&self) -> impl Iterator<Item = &(String, LineKind)> {
        self.lines.iter()
    }

    /// Completes the word under the caret: a row's name first, then its argument's names.
    pub fn complete(&mut self) {
        let words: Vec<&str> = self.input.split(' ').collect();
        let (head, last) = (&words[..words.len() - 1], words[words.len() - 1]);
        let pool: Vec<&str> = match head {
            [] => ROWS.iter().map(|r| r.0).collect(),
            [cmd] => names_for(cmd),
            _ => Vec::new(),
        };
        let hits: Vec<&str> = pool.iter().copied().filter(|n| n.starts_with(last)).collect();
        let Some(first) = hits.first() else { return };
        // The longest prefix every hit shares.
        let mut common = first.len();
        for h in &hits[1..] {
            common = common.min(first.bytes().zip(h.bytes()).take_while(|(a, b)| a == b).count());
        }
        let done = hits.len() == 1;
        let mut out = String::with_capacity(self.input.len() + 16);
        for w in head {
            out.push_str(w);
            out.push(' ');
        }
        out.push_str(&first[..common]);
        if done {
            out.push(' ');
        } else if common == last.len() {
            let shown: Vec<&str> = hits.iter().take(12).copied().collect();
            let more = if hits.len() > 12 { format!(" and {} more", hits.len() - 12) } else { String::new() };
            self.say(&format!("{}{more}", shown.join("  ")), LineKind::Out);
        }
        self.input = out;
    }
}

/// The names a row's first argument takes.
fn names_for(cmd: &str) -> Vec<&'static str> {
    let cat = jane_data::catalog();
    match cmd {
        "give" => cat.combat.items.iter().map(|i| i.id).collect(),
        "learn" => cat.combat.spells.iter().map(|s| s.id).collect(),
        "spawn" => cat.combat.units.iter().map(|u| u.id).collect(),
        "quest" => cat.story.quests.iter().map(|q| q.id).collect(),
        "tp" => jane_core::ZoneId::ALL.iter().map(|z| z.name()).collect(),
        "god" => vec!["on", "off"],
        "speed" => vec!["0.25", "1", "4", "hold", "step"],
        _ => Vec::new(),
    }
}

/// Draws the terminal over the top third and takes the keyboard while it is open.
pub fn draw(ui: &mut Ui, c: &mut Console) {
    if !c.open {
        return;
    }
    let (cw, ch) = ui.canvas;
    let full = ch / 3 + 20;
    // It slides down over six ticks.
    let t = ui.tick.wrapping_sub(c.opened).min(6) as i32;
    let h = full * (t + 1) / 7;
    let r = Rect::new(0, 0, cw, h);
    ui.claim(r);
    ui.fill(r, argb(Ramp::UiSlot.at(Tone::Deep), 238));
    ui.fill(Rect::new(0, h - 2, cw, 1), argb(style::gold_deep(), 200));
    ui.fill(Rect::new(0, h - 1, cw, 1), argb(style::INK, 255));
    let lh = line_h(Face::Fine);
    let input_y = h - lh - 6;
    // The input line, with its caret.
    ui.fill(Rect::new(0, input_y - 3, cw, lh + 4), argb(Ramp::UiSlot.at(Tone::Shade), 255));
    ui.text(6, input_y, ">", Ink::fine(style::gold()));
    let end = ui.text(18, input_y, &c.input, Ink::fine(style::text_bright()));
    if (ui.tick / 20) % 2 == 0 {
        ui.fill(Rect::new(end + 1, input_y + 1, 6, lh - 3), argb(style::gold(), 220));
    }
    // The lines, newest at the bottom, scrolled back by the wheel or page keys.
    let rows = ((input_y - 6) / lh).max(0) as usize;
    let n = c.lines.len();
    let last = n.saturating_sub(c.scroll);
    let first = last.saturating_sub(rows);
    for (k, (s, kind)) in c.lines.iter().skip(first).take(last - first).enumerate() {
        let ink = match kind {
            LineKind::Input => style::gold(),
            LineKind::Out => style::text(),
            LineKind::Good => style::good(),
            LineKind::Error => style::bad(),
        };
        ui.text(6, 4 + k as i32 * lh, s, Ink::fine(ink));
    }
    if c.scroll > 0 {
        let s = format!("{} lines below", c.scroll);
        ui.text(cw - text_w(Face::Fine, &s) - 8, 4, &s, Ink::fine(style::warn()));
    }

    // The keys.
    ui.typing = true;
    for ch_ in ui.input.typed.chars() {
        if ch_ != '`' && (ch_ == ' ' || ch_.is_ascii_graphic()) && c.input.len() < 120 {
            c.input.push(ch_);
        }
    }
    let k = ui.input.keys;
    if k.has(sc::BACKSPACE) {
        c.input.pop();
    }
    if k.has(sc::TAB) {
        c.complete();
    }
    if k.has(sc::UP) && !c.history.is_empty() {
        let at = c.hist_at.map_or(c.history.len() - 1, |i| i.saturating_sub(1));
        c.hist_at = Some(at);
        c.input.clone_from(&c.history[at]);
    }
    if k.has(sc::DOWN)
        && let Some(i) = c.hist_at
    {
        if i + 1 < c.history.len() {
            c.hist_at = Some(i + 1);
            c.input.clone_from(&c.history[i + 1]);
        } else {
            c.hist_at = None;
            c.input.clear();
        }
    }
    let page = rows.max(1);
    if k.has(sc::PAGEUP) || ui.input.wheel > 0 {
        c.scroll = (c.scroll + page / 2).min(n.saturating_sub(rows));
    }
    if k.has(sc::PAGEDOWN) || ui.input.wheel < 0 {
        c.scroll = c.scroll.saturating_sub(page / 2);
    }
    if k.has(sc::RETURN) || k.has(sc::KP_ENTER) {
        let line = std::mem::take(&mut c.input);
        let line = line.trim().to_owned();
        if !line.is_empty() {
            c.say(&format!("> {line}"), LineKind::Input);
            if c.history.last() != Some(&line) {
                if c.history.len() >= HISTORY {
                    c.history.remove(0);
                }
                c.history.push(line.clone());
            }
            c.hist_at = None;
            ui.intent(AppIntent::Console(line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_completes_rows_then_their_names() {
        let mut c = Console { input: "he".into(), ..Console::default() };
        c.complete();
        assert_eq!(c.input, "help ");
        let mut c = Console { input: "gi".into(), ..Console::default() };
        c.complete();
        assert_eq!(c.input, "give ");
        let item = jane_data::catalog().combat.items[0].id;
        c.input = format!("give {}", &item[..item.len() - 1]);
        c.complete();
        assert!(c.input.starts_with(&format!("give {}", &item[..item.len() - 1])), "{}", c.input);
        // Two rows share "sp" (spawn, speed): the common part is kept and the choices are listed.
        let mut c = Console { input: "sp".into(), ..Console::default() };
        c.complete();
        assert_eq!(c.input, "sp");
        let listed = &c.lines.back().expect("the choices listed").0;
        assert!(listed.contains("spawn") && listed.contains("speed"), "{listed}");
    }

    #[test]
    fn the_ring_keeps_four_hundred_lines() {
        let mut c = Console::default();
        for i in 0..450 {
            c.say(&format!("line {i}"), LineKind::Out);
        }
        assert_eq!(c.lines().count(), RING);
        assert_eq!(c.lines().next().map(|l| l.0.as_str()), Some("line 50"));
    }
}
