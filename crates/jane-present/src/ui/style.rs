//! The UI's colour language (ART.md §7): every colour is a master-palette index, so the chrome
//! shifts hue the way the world does (cool shadows, warm lights). Fills are those indices as
//! `0xAARRGGBB` at a coverage.
//!
//! | Role | Ink |
//! | --- | --- |
//! | Text on a panel | `ui_ink` light, shadowed in `k` |
//! | Quiet text (hints, labels) | `ui_ink` mid |
//! | Headings, focus, selection | `ui_gold` |
//! | Good / warn / over budget | `grass` light, `cloth_mustard` light, `cloth_red` light |
//! | Panel | `ui_panel` deep to shade, 88 %, a `k` rim and a lit inner edge |

use jane_art::palette::{self, Ix, Ramp, Tone};

/// An index as `0xAARRGGBB` at coverage `a`.
pub fn argb(ix: Ix, a: u8) -> u32 {
    let [r, g, b] = palette::rgb(ix);
    u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

/// `(r, g, b)` at coverage `a`.
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

/// A colour's coverage scaled by `k` of 255 (a fade).
pub fn fade(c: u32, k: u8) -> u32 {
    let a = ((c >> 24) * (u32::from(k) + 1)) >> 8;
    (c & 0x00ff_ffff) | a << 24
}

pub const INK: Ix = Ix::INK;
pub const SEAM: Ix = Ix::SEAM;

pub fn text() -> Ix {
    Ramp::UiInk.at(Tone::Light)
}
pub fn text_bright() -> Ix {
    Ramp::UiInk.at(Tone::High)
}
pub fn quiet() -> Ix {
    Ramp::UiInk.at(Tone::Mid)
}
pub fn dim() -> Ix {
    Ramp::UiInk.at(Tone::Shade)
}
pub fn gold() -> Ix {
    Ramp::UiGold.at(Tone::Light)
}
pub fn gold_deep() -> Ix {
    Ramp::UiGold.at(Tone::Mid)
}
pub fn good() -> Ix {
    Ramp::Grass.at(Tone::Light)
}
pub fn warn() -> Ix {
    Ramp::ClothMustard.at(Tone::High)
}
pub fn bad() -> Ix {
    Ramp::ClothRed.at(Tone::High)
}
pub fn frost() -> Ix {
    Ramp::Sky.at(Tone::Light)
}

/// Panel fill, top to bottom.
pub fn panel_top() -> Ix {
    Ramp::UiPanel.at(Tone::Mid)
}
pub fn panel_bottom() -> Ix {
    Ramp::UiPanel.at(Tone::Shade)
}
/// The lit inner edge of a panel.
pub fn panel_lit() -> Ix {
    Ramp::UiPanel.at(Tone::Light)
}
/// A slot's well.
pub fn well() -> Ix {
    Ramp::UiSlot.at(Tone::Shade)
}
pub fn well_lit() -> Ix {
    Ramp::UiSlot.at(Tone::Lift)
}

/// How opaque a panel's body is.
pub const PANEL_A: u8 = 246;
