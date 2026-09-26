//! The step-1 artefacts (ART.md §8): shapes drawn straight from the primitives while there are
//! no looks yet, so the sheets, the goldens and the light pass have something real to show.
//! Each is a function of nothing; later steps replace them with looks.

use std::fmt::Write as _;

use jane_core::grid::Rect;

use crate::canvas::{Canvas, Dir, StrokeKind, Z};
use crate::chrome::{self, ButtonState};
use crate::font::{Face, Font, Style};
use crate::hash::{h32, salt};
use crate::palette::{Ramp, Tone};

/// The demo sprites, by name: what `jane sheet layers <name>` and `jane sheet light <name>` take.
pub const NAMES: [&str; 5] = ["sphere", "ball", "panel", "lamp", "still"];

/// The demo sprite called `name`.
pub fn sprite(name: &str) -> Option<Canvas> {
    Some(match name {
        "sphere" => sphere(),
        "ball" => ball(),
        "panel" => bevel_panel(),
        "lamp" => lamp(),
        "still" => still_life(),
        _ => return None,
    })
}

/// A 32 px soft lit sphere of stone on its contact shadow, in a 48 px (3 x 3 cell) box: the
/// first artefact.
pub fn sphere() -> Canvas {
    let mut c = Canvas::cells(3, 3);
    c.soft_ellipse(Rect::new(8, 6, 32, 32), Ramp::Stone, Z::new(1, 16));
    c.ao_contact(Rect::new(9, 32, 30, 10), 3);
    c.outline();
    c
}

/// A hard-banded lit ball of plum cloth with a brass stud on top, in a 32 px box.
pub fn ball() -> Canvas {
    let mut c = Canvas::cells(2, 2);
    c.ellipse_lit(Rect::new(4, 4, 24, 22), Ramp::ClothPlum, Z::new(1, 12));
    c.disc_lit(12, 10, 2, Ramp::Brass, Z::flat(14));
    c.ao_contact(Rect::new(4, 22, 24, 8), 2);
    c.outline();
    c
}

/// A bevelled oak panel with a rounded iron plate, four brass studs and plank lines: 48 x 32.
pub fn bevel_panel() -> Canvas {
    let mut c = Canvas::new(48, 32);
    c.rect_bevel(Rect::new(0, 0, 48, 32), Ramp::WoodOak, 3, Z::new(2, 8));
    for y in [10, 21] {
        c.hline(3, 44, y, Ramp::WoodOak.at(Tone::Shade), 7);
    }
    c.rect_round(Rect::new(14, 9, 20, 14), Ramp::Iron, 2, 4, Z::new(9, 11));
    for (x, y) in [(5, 5), (42, 5), (5, 26), (42, 26)] {
        c.disc_lit(x, y, 1, Ramp::Brass, Z::flat(10));
    }
    c.outline();
    c
}

/// A lamp on a post: an iron foot and post, a cap, and a lit glass globe that emits. 32 x 48.
pub fn lamp() -> Canvas {
    let mut c = Canvas::cells(2, 3);
    c.ao_contact(Rect::new(6, 40, 20, 7), 2);
    c.rect_bevel(Rect::new(10, 39, 12, 6), Ramp::Iron, 1, Z::new(2, 5));
    c.rect_lit(Rect::new(14, 20, 4, 20), Ramp::Iron, 20);
    c.set_emitting(true);
    c.soft_ellipse(Rect::new(8, 7, 16, 15), Ramp::GlassLit, Z::new(22, 30));
    c.set_emitting(false);
    c.rect_round(Rect::new(9, 3, 14, 5), Ramp::Iron, 1, 2, Z::new(28, 32));
    c.disc_lit(16, 2, 1, Ramp::Brass, Z::flat(33));
    c.outline();
    c
}

/// A still life of every step-1 primitive on 64 x 48: a slate roof (`polyline_fill`) over a
/// plaster wall with a dithered banner (`gradient`), an oak crate (`rect_bevel`), a stone ball
/// (`soft_ellipse`), a turf tuft (`strokes`), a brace (`line`), the roof's ridge (`polyline`)
/// and contact shadows.
pub fn still_life() -> Canvas {
    let mut c = Canvas::cells(4, 3);
    // The wall and its roof.
    c.rect_lit(Rect::new(4, 14, 30, 22), Ramp::Plaster, 18);
    c.polyline_fill(&[(1, 15), (19, 2), (37, 15)], Ramp::Slate.at(Tone::Base), 24);
    c.gradient(Rect::new(4, 3, 30, 12), Ramp::Slate, Dir::DownRight, Tone::Light, Tone::Deep, true);
    c.polyline(&[(1, 15), (19, 2), (37, 15)], Ramp::Slate.at(Tone::High), 1, 24);
    c.gradient(Rect::new(12, 20, 14, 9), Ramp::ClothRed, Dir::Down, Tone::Light, Tone::Shade, true);
    // The crate and the ball.
    c.ao_contact(Rect::new(34, 38, 26, 8), 2);
    c.rect_bevel(Rect::new(36, 24, 22, 18), Ramp::WoodOak, 2, Z::new(3, 12));
    c.line((38, 26), (55, 39), Ramp::WoodOak.at(Tone::Shade), 1, 12);
    c.ao_contact(Rect::new(18, 42, 18, 5), 2);
    c.soft_ellipse(Rect::new(20, 32, 14, 13), Ramp::Stone, Z::new(1, 7));
    // A turf tuft and a rope.
    c.soft_ellipse(Rect::new(2, 36, 16, 10), Ramp::Grass, Z::new(1, 4));
    c.strokes(Rect::new(2, 34, 16, 12), Ramp::Grass, StrokeKind::Grass, 20, h32(1, 0, salt::DEMO));
    c.outline();
    c
}

/// A run of text on a clear flat canvas sized to fit it.
pub fn text_run(font: &Font, text: &str, style: Style) -> Canvas {
    let (w, h) = (Font::measure(style.face, text) + 2, style.face.cell().1 + 2);
    let mut c = Canvas::flat(w, h);
    font.draw(&mut c, 0, 0, text, style);
    c
}

/// The golden set (ART.md §1): every demo sprite, glyph runs in each working face, and the
/// chrome pieces. `tests/golden.txt` holds the FNV hash of each; `jane sheet --bless` rewrites
/// it from this list.
pub fn goldens(font: &Font) -> Vec<(String, Canvas)> {
    let ink = Ramp::UiInk.at(Tone::Light);
    let mut out: Vec<(String, Canvas)> = NAMES.iter().filter_map(|&n| Some((n.to_string(), sprite(n)?))).collect();
    let runs = [
        ("font-fine", "The quick brown fox jumps over the lazy dog. 0123456789", Style::plain(Face::Fine, ink)),
        ("font-fine-marks", "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ £←↑→↓•°·■▼◆●✓✗∞", Style::plain(Face::Fine, ink)),
        ("font-small", "Needs wood x2 · Rats 3 of 5", Style { shadow: true, ..Style::plain(Face::Small, ink) }),
        ("font-small-bold", "Lost Property", Style { bold: true, ..Style::plain(Face::Small, ink) }),
        ("font-head", "Jane", Style { outline: true, ..Style::plain(Face::Head, Ramp::UiGold.at(Tone::Light)) }),
    ];
    for (name, text, style) in runs {
        out.push((name.to_string(), text_run(font, text, style)));
    }
    out.push(("chrome-panel".into(), chrome::panel(96, 48)));
    out.push(("chrome-button".into(), chrome::button(64, 20, ButtonState::Pressed, "Take", font, Face::Fine)));
    out.push(("chrome-bar".into(), chrome::bar(80, 8, 620, 780, Ramp::ClothRed)));
    out.push(("chrome-slot".into(), chrome::slot(36)));
    out.push(("chrome-sweep".into(), chrome::sweep(36, 300)));
    out
}

/// The text of `tests/golden.txt` for the current goldens.
pub fn golden_file(font: &Font) -> String {
    let mut s = String::from(
        "# jane-art goldens: FNV-1a over size and all four layers (ART.md §1).\n\
         # Regenerate with `jane sheet --bless` or `JANE_BLESS=1 cargo test -p jane-art --test golden`.\n",
    );
    for (name, c) in goldens(font) {
        let _ = writeln!(s, "{name} {:08x}", c.hash());
    }
    // Every look, variant and seat: the hash of all its frames (ART.md §8 step 2 on).
    match crate::looks::all() {
        Ok(sets) => {
            for r in sets {
                let kind = match crate::looks::find(r.name).map(|(_, l)| l) {
                    Some(jane_data::Look::Prop(_) | jane_data::Look::Building(_)) => "prop",
                    Some(jane_data::Look::Icon(_)) => "icon",
                    _ => "unit",
                };
                let _ = writeln!(s, "{kind}:{} {:08x}", r.key(), r.set.hash());
            }
        }
        Err(e) => {
            let _ = writeln!(s, "looks-error {e}");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Ix;

    #[test]
    fn every_demo_keeps_the_layer_contract() {
        for (name, c) in goldens(&Font::build()) {
            c.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn the_lamp_emits_and_nothing_else_does() {
        let lamp = lamp();
        assert!(lamp.emissive().iter().any(|&e| e != Ix::CLEAR));
        for n in ["sphere", "ball", "panel", "still"] {
            assert!(sprite(n).unwrap().emissive().iter().all(|&e| e == Ix::CLEAR), "{n}");
        }
    }

    #[test]
    fn rendering_twice_gives_the_same_bytes() {
        let font = Font::build();
        let (a, b) = (goldens(&font), goldens(&font));
        for ((n, x), (_, y)) in a.iter().zip(&b) {
            assert_eq!(x, y, "{n}");
        }
    }
}
