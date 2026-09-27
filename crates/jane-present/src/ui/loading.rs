//! The loading screen (PRESENTATION.md §3.2): while New Game builds the county on the app's
//! thread, the county's skeleton forms on a card: the land, then the river and the lake, then
//! the roads and the railway, then the sites, lamps and patches, then one mark per zone as the
//! build finishes; a stage name in the Small face beneath. Input is ignored until it is done.
//!
//! The card is painted from the skeleton (`jane_world::skeleton`) the loader sends first, at 2x
//! (a macro cell is 2 x 2 px), each layer revealed by a sweep as its stage comes up.

use jane_art::font::Face;
use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_world::skeleton::{Biome, ROAD, ROAD_BRIDGE, ROAD_LIT, SKEL_H, SKEL_W, Skeleton, Water};

use crate::frame::Src;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, PanelStyle, Ui, text_w};
use crate::ui::style::{self, argb};

/// The UI image slot the card is painted into.
pub const CARD: u16 = 1;
/// Px per macro cell on the card.
const SCALE: i32 = 2;
/// Ticks each stage takes to sweep in.
pub const STAGE_TICKS: u32 = 20;
/// The stages, in order.
pub const STAGES: [&str; 5] =
    ["The land", "The river and the lake", "Roads and the railway", "Sites, lamps and patches", "The zones"];

/// The layers of the card, painted once from the skeleton.
#[derive(Clone, Debug, Default)]
pub struct Card {
    pub w: i32,
    pub h: i32,
    /// Per stage, the colour it lays on each px (0: nothing).
    layers: [Vec<u32>; 4],
    /// Where each zone's mark goes, card px, in `ZoneId` order (the county's is its crown).
    zones: Vec<(i32, i32)>,
}

impl Card {
    pub fn from_skeleton(s: &Skeleton) -> Card {
        let (w, h) = (SKEL_W * SCALE, SKEL_H * SCALE);
        let n = (w * h) as usize;
        let mut c = Card { w, h, layers: [vec![0; n], vec![0; n], vec![0; n], vec![0; n]], zones: Vec::new() };
        let t = &s.terrain;
        let ink = |ix: Ix| {
            let [r, g, b] = palette::rgb(ix);
            0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
        };
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                let ramp = match t.biome.read(x, y, Biome::Field) {
                    Biome::Field => Ramp::Grass,
                    Biome::Hedge | Biome::Garden => Ramp::Leaf,
                    Biome::Wood | Biome::WetWood => Ramp::ClothGreen,
                    Biome::Foothill | Biome::Hill => Ramp::Reed,
                    Biome::Reed | Biome::Marsh => Ramp::ClothMoss,
                    Biome::Slag | Biome::Yard => Ramp::Slate,
                    Biome::Town => Ramp::Plaster,
                };
                // Height as tone: low ground in shade, the crown lit.
                let ht = i32::from(t.height.read(x, y, 0));
                let lit = (x + y) & 1 == 0 && ht % 32 > 24;
                let tone = Tone::ALL[(1 + ht * 4 / 256 + i32::from(lit)).clamp(1, 5) as usize];
                let water = t.water.read(x, y, Water::Dry);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = ((y * SCALE + dy) * w + x * SCALE + dx) as usize;
                    c.layers[0][i] = ink(ramp.at(tone));
                    if water != Water::Dry {
                        let deep = if water == Water::Lake { Tone::Shade } else { Tone::Mid };
                        let sparkle = (x * 7 + y * 3 + dx) % 11 == 0;
                        c.layers[1][i] = ink(Ramp::Water.at(if sparkle { Tone::Light } else { deep }));
                    }
                }
            }
        }
        let mut dot = |layer: usize, x: i32, y: i32, v: u32| {
            if x >= 0 && y >= 0 && x < w && y < h {
                c.layers[layer][(y * w + x) as usize] = v;
            }
        };
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                let bits = s.road.read(x, y, 0);
                if bits & ROAD == 0 {
                    continue;
                }
                let col = if bits & ROAD_BRIDGE != 0 {
                    ink(Ramp::WoodDark.at(Tone::Base))
                } else if bits & ROAD_LIT != 0 {
                    ink(Ramp::GlassLit.at(Tone::Light))
                } else {
                    ink(Ramp::WoodPale.at(Tone::Mid))
                };
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    dot(2, x * SCALE + dx, y * SCALE + dy, col);
                }
            }
        }
        for (k, &(x, y)) in s.rail.iter().enumerate() {
            let col = if k % 2 == 0 { ink(Ramp::Bone.at(Tone::Light)) } else { ink(Ix::INK) };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                dot(2, x * SCALE + dx, y * SCALE + dy, col);
            }
        }
        for p in &s.pois {
            let col =
                ink(if p.anchor.is_some() { Ramp::GlassLit.at(Tone::High) } else { Ramp::Plaster.at(Tone::High) });
            dot(3, p.mx * SCALE, p.my * SCALE, col);
            dot(3, p.mx * SCALE + 1, p.my * SCALE, col);
        }
        for a in &s.areas {
            let r = i32::from(a.def.radius) * SCALE / jane_world::skeleton::MACRO;
            let col = ink(Ramp::ClothRed.at(if a.def.threat >= 4 { Tone::High } else { Tone::Mid }));
            for k in 0..(r * 6).max(8) {
                // A dotted ring: every other step of a circle.
                if k % 2 == 1 {
                    continue;
                }
                let ang = jane_core::Angle((k * 65536 / (r * 6).max(8)) as u16);
                let (cx, cy) = (a.mx * SCALE + 1, a.my * SCALE + 1);
                let x = cx + jane_core::angle::cos_q15(ang).0 * r / 32768;
                let y = cy + jane_core::angle::sin_q15(ang).0 * r / 32768;
                dot(3, x, y, col);
            }
        }
        for site in &s.sites {
            let col = ink(if site.def.dungeon { Ramp::ClothRed.at(Tone::High) } else { Ramp::UiGold.at(Tone::Light) });
            let (x, y) = (site.mx * SCALE, site.my * SCALE);
            for (dx, dy) in [
                (0, -2),
                (-1, -1),
                (0, -1),
                (1, -1),
                (-2, 0),
                (-1, 0),
                (0, 0),
                (1, 0),
                (2, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
                (0, 2),
            ] {
                dot(3, x + dx, y + dy, col);
            }
        }
        // One mark a zone: the county at the crown, the rest at the sites that lead to them.
        let (cx, cy) = t.crown;
        c.zones.push((cx * SCALE, cy * SCALE));
        for site in s.sites.iter().take(12) {
            c.zones.push((site.mx * SCALE, site.my * SCALE));
        }
        c
    }
}

/// The loading screen's state.
#[derive(Clone, Debug, Default)]
pub struct LoadingState {
    pub card: Option<Card>,
    /// The presenter tick the card arrived.
    pub since: u32,
    /// The build has finished.
    pub built: bool,
    /// The seed, shown under the card.
    pub seed: u32,
    /// "New Game", "Continue", "Load".
    pub verb: &'static str,
}

impl LoadingState {
    /// Every stage has swept in and the build is done: play can start.
    pub fn done(&self, tick: u32) -> bool {
        self.built && (self.card.is_none() || tick.wrapping_sub(self.since) >= STAGE_TICKS * STAGES.len() as u32 + 20)
    }
}

pub fn draw(ui: &mut Ui, st: &mut LoadingState) {
    let (cw, ch) = ui.canvas;
    ui.fill(Rect::new(0, 0, cw, ch), argb(Ramp::UiSlot.at(Tone::Deep), 255));
    let t = ui.tick.wrapping_sub(st.since);
    let (w, h) = (SKEL_W * SCALE, SKEL_H * SCALE);
    let r = Rect::new((cw - w) / 2 - 8, 36, w + 16, h + 16);
    ui.panel(r, PanelStyle::Window);
    let (x0, y0) = (i32::from(r.x) + 8, i32::from(r.y) + 8);
    let stage = (t / STAGE_TICKS).min(STAGES.len() as u32 - 1) as usize;
    if let Some(card) = &st.card {
        // Compose the stages so far; the current one sweeps down from the top.
        let img = ui.image_mut(CARD, w as u16, h as u16);
        let sweep_row = |k: usize| -> i32 {
            let s = t.saturating_sub(k as u32 * STAGE_TICKS);
            (s as i32 * h / STAGE_TICKS as i32).min(h)
        };
        let rows: Vec<i32> = (0..4).map(sweep_row).collect();
        let bg = argb(Ramp::UiSlot.at(Tone::Shade), 255);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let mut p = bg;
                for (k, layer) in card.layers.iter().enumerate() {
                    if y < rows[k] && layer[i] != 0 {
                        p = layer[i];
                    }
                }
                img.argb[i] = p;
            }
        }
        ui.image_changed(CARD);
        ui.image(CARD, Src { x: 0, y: 0, w: w as u16, h: h as u16 }, Rect::new(x0, y0, w, h), 255);
        // The leading edge of the sweep, a lamp-coloured line.
        if stage < 4 {
            let y = rows[stage];
            if y < h {
                ui.fill(Rect::new(x0, y0 + y, w, 1), argb(style::gold(), 200));
            }
        }
        // The zones' marks, one by one.
        if stage == 4 {
            let n = (t.saturating_sub(4 * STAGE_TICKS) * card.zones.len() as u32 / STAGE_TICKS) as usize;
            for (i, &(zx, zy)) in card.zones.iter().enumerate().take(n.min(card.zones.len())) {
                let pulse = if i + 1 == n { 3 } else { 2 };
                ui.fill(Rect::new(x0 + zx - pulse, y0 + zy - pulse, pulse * 2 + 1, pulse * 2 + 1), argb(Ix::INK, 200));
                ui.fill(Rect::new(x0 + zx - 1, y0 + zy - 1, 3, 3), argb(style::gold(), 255));
            }
        }
    }
    // The stage's name, and the dots of the stages beneath it.
    let name = STAGES[stage];
    let y = i32::from(r.y) + i32::from(r.h) + 12;
    ui.text((cw - text_w(Face::Small, name)) / 2, y, name, Ink::small(style::text_bright()).shadow());
    for k in 0..STAGES.len() {
        let x = cw / 2 - 40 + k as i32 * 20;
        let on = k <= stage;
        ui.fill(Rect::new(x, y + 26, 8, 2), argb(if on { style::gold() } else { style::dim() }, 255));
    }
    let mut foot = String::with_capacity(32);
    foot.push_str(st.verb);
    foot.push_str(" · seed ");
    foot.push_str(&st.seed.to_string());
    ui.text((cw - text_w(Face::Fine, &foot)) / 2, ch - 20, &foot, Ink::fine(style::dim()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_paints_every_layer_of_a_real_skeleton() {
        let s = jane_world::skeleton::skeleton(7).expect("seed 7 builds");
        let c = Card::from_skeleton(&s);
        assert_eq!((c.w, c.h), (SKEL_W * SCALE, SKEL_H * SCALE));
        assert!(c.layers[0].iter().all(|&p| p != 0), "land everywhere");
        for k in 1..4 {
            assert!(c.layers[k].iter().any(|&p| p != 0), "layer {k} draws");
        }
        assert_eq!(c.zones.len(), 13);
        let st = LoadingState { card: Some(c), since: 0, built: true, seed: 7, verb: "New Game" };
        assert!(!st.done(10));
        assert!(st.done(10_000));
    }
}
