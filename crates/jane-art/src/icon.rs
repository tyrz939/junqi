//! Icons (ART.md §2.5, §8 step 6): an item, a spell or a status as one object, drawn at 32 x 32
//! for the bag and drawn again at 16 x 16 for a chip, never downscaled. Every shape is laid out
//! on a 32-unit grid and placed at the size it is drawn at, so the chip is its own render with
//! its own clusters; details under 2 px are left out of it.
//!
//! An icon is lit from the top-left like everything else, lined selectively, and at most 24
//! colours. The slot behind it is the chrome's (`chrome::slot`), not the icon's. A potion, an
//! orb or a light stone glows (`glow`): its liquid or its heart emits, so the bar glows a little
//! at night.

use jane_core::grid::Rect;
use jane_data::{IconClass, IconLook, IconMark};

use crate::canvas::{Canvas, Z};
use crate::palette::{Ix, Ramp, Tone};
use crate::sprite::{FrameId, Role, SpriteSet};

/// The two sizes an icon is drawn at.
pub const SIZES: [i32; 2] = [32, 16];

/// A drawing on the 32-unit grid placed at size `s`.
struct G<'a> {
    c: &'a mut Canvas,
    s: i32,
}

impl G<'_> {
    fn v(&self, u: i32) -> i32 {
        u * self.s / 32
    }
    fn r(&self, x: i32, y: i32, w: i32, h: i32) -> Rect {
        let (x0, y0) = (self.v(x), self.v(y));
        Rect::new(x0, y0, (self.v(x + w) - x0).max(1), (self.v(y + h) - y0).max(1))
    }
    fn p(&self, x: i32, y: i32) -> (i32, i32) {
        (self.v(x), self.v(y))
    }
    fn big(&self) -> bool {
        self.s >= 32
    }
    /// A soft volume over the shape `f` draws into a mask.
    fn vol(&mut self, ramp: Ramp, rad: i32, f: impl Fn(&mut Canvas, &Self)) {
        let mut m = Canvas::new(self.c.w(), self.c.h());
        f(&mut m, self);
        let rad = (rad * self.s / 32).max(1);
        self.c.inflate(&m, ramp, rad, Z::new(1, 3));
    }
    fn ell(&mut self, ramp: Ramp, x: i32, y: i32, w: i32, h: i32) {
        let r = self.r(x, y, w, h);
        self.c.ellipse_lit(r, ramp, Z::new(1, 3));
    }
    fn line(&mut self, ix: Ix, a: (i32, i32), b: (i32, i32), w: i32) {
        let (a, b) = (self.p(a.0, a.1), self.p(b.0, b.1));
        // Two px at the least: a one-px diagonal is a row of spikes, and the clean-up takes it.
        let w = (w * self.s / 32).max(2);
        self.c.line(a, b, ix, w, 3);
    }
    fn poly(&mut self, ramp: Ramp, pts: &[(i32, i32)], rad: i32) {
        let pts: Vec<(i32, i32)> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.vol(ramp, rad, |m, _| m.polyline_fill(&pts, Ix::INK, 1));
    }
    fn dot(&mut self, ix: Ix, x: i32, y: i32) {
        let (x, y) = self.p(x, y);
        self.c.dot(x, y, ix, 4);
    }
    fn fill(&mut self, ix: Ix, x: i32, y: i32, w: i32, h: i32) {
        let r = self.r(x, y, w, h);
        self.c.fill_rect(r, ix, 3);
    }
}

/// Render icon `look` at both sizes: variant 0 is the 32 x 32, variant 1 the 16 x 16 chip.
pub fn render(look: &IconLook) -> Result<Vec<SpriteSet>, String> {
    let ramp = crate::person::ramp(look.ramp)?;
    let trim = look.trim.map_or(Ok(default_trim(look.class)), crate::person::ramp)?;
    let mut out = Vec::new();
    for s in SIZES {
        let mut c = Canvas::new(s, s);
        draw(&mut G { c: &mut c, s }, look, ramp, trim);
        finish(&mut c, ramp, trim);
        let emits = if look.glow { vec![Role::Glass] } else { Vec::new() };
        out.push(SpriteSet { w: s, h: s, ax: 0, ay: 0, frames: vec![(FrameId::Base, c)], roles: vec![(Role::Body, ramp)], emits });
    }
    Ok(out)
}

fn default_trim(class: IconClass) -> Ramp {
    match class {
        IconClass::Flask | IconClass::Vial => Ramp::Glass,
        IconClass::Key | IconClass::Ring | IconClass::Amulet | IconClass::Spectacles => Ramp::Brass,
        IconClass::Herb | IconClass::Bloom | IconClass::Fruit | IconClass::Grapes => Ramp::Leaf,
        IconClass::Spell | IconClass::Status => Ramp::Iron,
        IconClass::Tool | IconClass::Spanner | IconClass::Scissors | IconClass::Spoons | IconClass::Can => Ramp::WoodDark,
        _ => Ramp::Leather,
    }
}

const HARD: [Tone; 8] = [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::High, Tone::High];

fn finish(c: &mut Canvas, ramp: Ramp, trim: Ramp) {
    let glow: Vec<(i32, i32, Ix)> = (0..c.h())
        .flat_map(|y| (0..c.w()).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let e = c.emissive_at(x, y);
            (e != Ix::CLEAR).then_some((x, y, e))
        })
        .collect();
    for r in [ramp, trim] {
        c.retone(r, HARD);
        c.declutter(r);
    }
    c.despike();
    c.outline();
    c.relight(&glow);
    c.cap_heights(4);
}

fn draw(g: &mut G<'_>, look: &IconLook, ramp: Ramp, trim: Ramp) {
    let glint = Ramp::HairWhite.at(Tone::High);
    match look.class {
        IconClass::Flask | IconClass::Vial => {
            let vial = look.class == IconClass::Vial;
            // Glass first, then the liquid inside it, a cork, a streak of light.
            if vial {
                g.poly(trim, &[(12, 8), (20, 8), (20, 28), (16, 30), (12, 28)], 3);
            } else {
                g.ell(trim, 5, 11, 22, 19);
                g.poly(trim, &[(12, 4), (20, 4), (20, 14), (12, 14)], 2);
            }
            if ramp != Ramp::Glass {
                g.c.set_emitting(look.glow);
                if vial {
                    g.poly(ramp, &[(13, 16), (19, 16), (19, 27), (16, 29), (13, 27)], 3);
                } else {
                    let r = g.r(7, 17, 18, 12);
                    g.c.ellipse_lit(r, ramp, Z::new(2, 3));
                }
                g.c.set_emitting(false);
            }
            g.fill(Ramp::WoodPale.at(Tone::Base), 13, if vial { 5 } else { 1 }, 6, 4);
            g.line(glint, (9, 16), (9, 21), 1);
            if g.big() {
                g.dot(glint, 10, 14);
            }
        }
        IconClass::Key => {
            // A bow, a shank, a bit.
            g.vol(ramp, 3, |m, g| {
                m.ellipse(g.r(3, 3, 12, 12), Ix::INK, 1);
                m.polyline_fill(&[g.p(12, 11), g.p(15, 9), g.p(29, 23), g.p(26, 26)], Ix::INK, 1);
                m.polyline_fill(&[g.p(21, 21), g.p(24, 18), g.p(28, 22), g.p(25, 25)], Ix::INK, 1);
            });
            let hole = g.r(7, 7, 4, 4);
            g.c.ellipse(hole, Ix::CLEAR, 0);
            g.fill(ramp.at(Tone::Shade), 22, 25, 3, 3);
            g.fill(ramp.at(Tone::Shade), 18, 22, 3, 3);
        }
        IconClass::Bar => {
            // An ingot: a trapezoid, its top lit, its face in shade.
            g.poly(ramp, &[(8, 10), (24, 10), (29, 24), (3, 24)], 3);
            g.fill(ramp.at(Tone::High), 9, 11, 14, 2);
            g.fill(ramp.at(Tone::Mid), 4, 20, 25, 4);
        }
        IconClass::Orb => {
            let r = g.r(5, 5, 22, 22);
            g.c.set_emitting(look.glow);
            g.c.soft_ellipse(r, ramp, Z::new(1, 4));
            g.c.set_emitting(false);
            g.dot(glint, 11, 10);
            g.dot(glint, 12, 10);
        }
        IconClass::Stone | IconClass::Gem => {
            let gem = look.class == IconClass::Gem;
            g.c.set_emitting(look.glow);
            g.poly(ramp, &[(6, 12), (12, 5), (22, 6), (27, 14), (24, 25), (12, 27), (5, 21)], if gem { 2 } else { 5 });
            g.c.set_emitting(false);
            if gem {
                g.line(ramp.at(Tone::High), (12, 5), (15, 16), 1);
                g.line(ramp.at(Tone::Shade), (15, 16), (24, 25), 1);
                g.line(ramp.at(Tone::Light), (6, 12), (15, 16), 1);
            } else if g.big() {
                g.line(ramp.at(Tone::Shade), (14, 12), (18, 18), 1);
            }
        }
        IconClass::Herb | IconClass::Bloom | IconClass::Mushroom => {
            if look.class == IconClass::Mushroom {
                g.poly(trim, &[(13, 16), (19, 16), (20, 28), (12, 28)], 2);
                g.vol(ramp, 5, |m, g| m.ellipse(g.r(5, 5, 22, 15), Ix::INK, 1));
                for (x, y) in [(11, 9), (18, 8), (21, 13)] {
                    g.fill(Ramp::HairWhite.at(Tone::Light), x, y, 2, 2);
                }
            } else {
                // A stem and three leaves, and a flower at the top for a bloom.
                g.line(trim.at(Tone::Shade), (16, 29), (16, 10), 2);
                for (x, y, w) in [(6, 18, 10), (16, 14, 10), (8, 24, 9)] {
                    g.ell(trim, x, y, w, 6);
                }
                if look.class == IconClass::Bloom {
                    for (dx, dy) in [(-4, 0), (4, 0), (0, -4), (0, 4)] {
                        g.ell(ramp, 12 + dx, 5 + dy, 8, 8);
                    }
                    g.fill(Ramp::ClothMustard.at(Tone::Light), 15, 8, 3, 3);
                } else {
                    g.ell(ramp, 10, 3, 12, 10);
                }
            }
        }
        IconClass::Fruit => {
            g.ell(ramp, 5, 8, 22, 21);
            g.line(Ramp::Bark.at(Tone::Base), (16, 9), (17, 4), 2);
            g.ell(trim, 18, 3, 9, 5);
            g.dot(glint, 10, 13);
        }
        IconClass::Grapes => {
            for (x, y) in [(8, 8), (15, 8), (22, 8), (11, 14), (18, 14), (14, 20), (11, 25)] {
                g.ell(ramp, x - 3, y, 8, 8);
            }
            g.ell(trim, 14, 2, 10, 5);
        }
        IconClass::Egg => {
            g.vol(ramp, 7, |m, g| m.ellipse(g.r(8, 4, 16, 24), Ix::INK, 1));
            g.dot(glint, 13, 9);
        }
        IconClass::Loaf | IconClass::Crepe => {
            if look.class == IconClass::Crepe {
                g.poly(ramp, &[(4, 26), (28, 26), (16, 6)], 3);
                g.line(ramp.at(Tone::Shade), (10, 22), (22, 22), 1);
            } else {
                g.vol(ramp, 7, |m, g| m.ellipse(g.r(3, 8, 26, 20), Ix::INK, 1));
                for x in [10, 16, 22] {
                    g.line(ramp.at(Tone::Shade), (x - 2, 12), (x + 1, 17), 1);
                }
            }
        }
        IconClass::Meat => {
            g.line(Ramp::Bone.at(Tone::Light), (20, 20), (28, 28), 3);
            g.fill(Ramp::Bone.at(Tone::Light), 26, 26, 4, 4);
            g.vol(ramp, 5, |m, g| m.ellipse(g.r(3, 3, 20, 20), Ix::INK, 1));
        }
        IconClass::Potatoes => {
            for (x, y, w) in [(4, 12, 14), (15, 9, 13), (10, 18, 14)] {
                g.vol(ramp, 4, |m, g| m.ellipse(g.r(x, y, w, 10), Ix::INK, 1));
            }
        }
        IconClass::Pot | IconClass::Tin | IconClass::Can => {
            // A pot of honey (a lid and a band), a tin (a can with its rim), an oil can (a
            // spout).
            g.poly(ramp, &[(8, 10), (24, 10), (25, 28), (7, 28)], 4);
            g.fill(trim.at(Tone::Base), 7, 7, 18, 4);
            g.fill(trim.at(Tone::Light), 7, 7, 18, 1);
            if look.class == IconClass::Can {
                g.line(ramp.at(Tone::Base), (22, 12), (30, 4), 2);
            }
            if look.class == IconClass::Pot {
                g.fill(Ramp::ClothLinen.at(Tone::Light), 11, 16, 10, 6);
            }
        }
        IconClass::Glove | IconClass::Coat | IconClass::Scarf | IconClass::Hat | IconClass::Cap | IconClass::Fleece => {
            garment(g, look.class, ramp, trim);
        }
        IconClass::Letter | IconClass::Parcel | IconClass::Sack => {
            match look.class {
                IconClass::Letter => {
                    g.poly(ramp, &[(3, 8), (29, 8), (29, 25), (3, 25)], 2);
                    g.line(ramp.at(Tone::Shade), (3, 8), (16, 18), 1);
                    g.line(ramp.at(Tone::Shade), (29, 8), (16, 18), 1);
                    g.ell(Ramp::ClothRed, 13, 15, 6, 6);
                }
                IconClass::Parcel => {
                    g.poly(ramp, &[(5, 9), (27, 9), (27, 27), (5, 27)], 2);
                    g.fill(trim.at(Tone::Base), 15, 9, 2, 18);
                    g.fill(trim.at(Tone::Base), 5, 17, 22, 2);
                    g.ell(trim, 12, 4, 8, 6);
                }
                _ => {
                    g.vol(ramp, 6, |m, g| {
                        m.ellipse(g.r(5, 10, 22, 20), Ix::INK, 1);
                        m.fill_rect(g.r(12, 4, 8, 8), Ix::INK, 1);
                    });
                    g.fill(trim.at(Tone::Base), 11, 11, 10, 2);
                }
            }
        }
        IconClass::Tool | IconClass::Scissors | IconClass::Spanner | IconClass::Spoons => tool(g, look.class, ramp, trim),
        IconClass::Net => {
            for k in 0..5 {
                let o = 4 + k * 6;
                g.line(ramp.at(Tone::Base), (o, 4), (4, o), 1);
                g.line(ramp.at(Tone::Light), (o, 28), (28, o), 1);
            }
            g.line(trim.at(Tone::Base), (4, 4), (28, 28), 2);
        }
        IconClass::Plate => {
            g.ell(ramp, 3, 7, 26, 18);
            let r = g.r(8, 10, 16, 11);
            g.c.ellipse(r, ramp.at(Tone::Mid), 3);
        }
        IconClass::Ring | IconClass::Amulet => {
            let r = g.r(7, 9, 18, 18);
            g.c.ellipse_lit(r, ramp, Z::new(1, 3));
            let inner = g.r(11, 13, 10, 10);
            g.c.ellipse(inner, Ix::CLEAR, 0);
            if look.class == IconClass::Amulet {
                g.line(ramp.at(Tone::Shade), (10, 2), (16, 9), 1);
                g.line(ramp.at(Tone::Shade), (22, 2), (16, 9), 1);
            }
            g.ell(trim, 12, 5, 8, 7);
        }
        IconClass::Spectacles => {
            for x in [3, 17] {
                let r = g.r(x, 10, 12, 11);
                g.c.ellipse(r, ramp.at(Tone::Base), 3);
                let inner = g.r(x + 2, 12, 8, 7);
                g.c.ellipse(inner, Ramp::Glass.at(Tone::Light), 3);
            }
            g.line(ramp.at(Tone::Base), (14, 14), (18, 14), 1);
        }
        IconClass::Logs => {
            for (x, y) in [(4, 16), (16, 16), (10, 6)] {
                g.ell(ramp, x, y, 12, 12);
                let r = g.r(x + 3, y + 3, 6, 6);
                g.c.ellipse(r, trim.at(Tone::Light), 3);
            }
        }
        IconClass::Butterfly => {
            for (x, y, w, h) in [(3, 5, 13, 13), (16, 5, 13, 13), (5, 17, 11, 10), (16, 17, 11, 10)] {
                g.ell(ramp, x, y, w, h);
            }
            g.line(Ramp::ClothBlack.at(Tone::Base), (16, 6), (16, 27), 2);
        }
        IconClass::Tortoise => {
            g.vol(Ramp::Leaf, 3, |m, g| m.ellipse(g.r(22, 13, 8, 8), Ix::INK, 1));
            g.vol(ramp, 6, |m, g| m.ellipse(g.r(3, 8, 23, 18), Ix::INK, 1));
            for (x, y) in [(10, 13), (17, 13), (13, 19)] {
                g.fill(ramp.at(Tone::Shade), x, y, 5, 1);
            }
        }
        IconClass::Dust => {
            g.c.set_emitting(look.glow);
            g.vol(ramp, 4, |m, g| m.ellipse(g.r(4, 14, 24, 14), Ix::INK, 1));
            g.c.set_emitting(false);
            for (x, y) in [(9, 17), (17, 16), (21, 21)] {
                g.dot(ramp.at(Tone::Glint), x, y);
            }
        }
        IconClass::Spell | IconClass::Status => {
            let spell = look.class == IconClass::Spell;
            if spell {
                g.c.set_emitting(look.glow);
                let r = g.r(2, 2, 28, 28);
                g.c.soft_ellipse(r, ramp, Z::new(1, 3));
                g.c.set_emitting(false);
            } else {
                let r = g.r(2, 2, 28, 28);
                g.c.ellipse_lit(r, trim, Z::new(1, 3));
                let inner = g.r(5, 5, 22, 22);
                g.c.ellipse(inner, ramp.at(Tone::Deep), 3);
            }
            mark(g, look.mark, if spell { Ramp::HairWhite } else { ramp });
        }
    }
    if look.mark != IconMark::None && !matches!(look.class, IconClass::Spell | IconClass::Status) {
        mark(g, look.mark, trim);
    }
}

fn garment(g: &mut G<'_>, class: IconClass, ramp: Ramp, trim: Ramp) {
    match class {
        IconClass::Glove => {
            g.poly(ramp, &[(9, 28), (9, 13), (11, 5), (14, 5), (15, 12), (17, 4), (20, 4), (21, 12), (24, 8), (27, 10), (23, 22), (22, 28)], 3);
            g.fill(trim.at(Tone::Base), 9, 25, 13, 3);
        }
        IconClass::Coat => {
            g.poly(ramp, &[(10, 4), (22, 4), (29, 10), (27, 19), (24, 17), (24, 29), (8, 29), (8, 17), (5, 19), (3, 10)], 3);
            g.line(ramp.at(Tone::Shade), (16, 5), (16, 28), 1);
            for y in [12, 18, 24] {
                g.dot(Ramp::Brass.at(Tone::Light), 14, y);
            }
        }
        IconClass::Scarf => {
            g.poly(ramp, &[(4, 6), (28, 6), (28, 12), (20, 12), (20, 28), (14, 28), (14, 12), (4, 12)], 2);
            for y in [16, 22] {
                g.fill(trim.at(Tone::Base), 14, y, 6, 2);
            }
            g.fill(trim.at(Tone::Base), 10, 6, 2, 6);
        }
        IconClass::Hat => {
            g.ell(ramp, 2, 18, 28, 9);
            g.vol(ramp, 5, |m, g| m.ellipse(g.r(8, 5, 16, 18), Ix::INK, 1));
            g.fill(trim.at(Tone::Base), 8, 17, 16, 3);
        }
        IconClass::Cap => {
            g.vol(ramp, 6, |m, g| m.ellipse(g.r(4, 8, 22, 16), Ix::INK, 1));
            g.poly(ramp, &[(16, 18), (30, 20), (28, 24), (14, 23)], 2);
        }
        _ => {
            // A fleece: a cloud of wool.
            for (x, y, w) in [(3, 11, 12), (11, 6, 12), (18, 11, 11), (8, 16, 16)] {
                g.vol(ramp, 4, |m, g| m.ellipse(g.r(x, y, w, 12), Ix::INK, 1));
            }
        }
    }
}

fn tool(g: &mut G<'_>, class: IconClass, ramp: Ramp, trim: Ramp) {
    match class {
        IconClass::Scissors => {
            g.line(ramp.at(Tone::Light), (8, 26), (26, 5), 2);
            g.line(ramp.at(Tone::Base), (24, 26), (6, 5), 2);
            for x in [4, 20] {
                let r = g.r(x, 22, 8, 8);
                g.c.ellipse(r, trim.at(Tone::Base), 3);
                let inner = g.r(x + 2, 24, 4, 4);
                g.c.ellipse(inner, Ix::CLEAR, 0);
            }
        }
        IconClass::Spanner => {
            g.line(ramp.at(Tone::Base), (8, 24), (22, 10), 3);
            for (x, y) in [(3, 21), (19, 3)] {
                let r = g.r(x, y, 10, 10);
                g.c.ellipse_lit(r, ramp, Z::new(1, 3));
            }
            let (a, b) = (g.r(21, 3, 4, 5), g.r(4, 25, 4, 5));
            g.c.fill_rect(a, Ix::CLEAR, 0);
            g.c.fill_rect(b, Ix::CLEAR, 0);
        }
        IconClass::Spoons => {
            for (x, t) in [(10, Tone::Light), (20, Tone::Base)] {
                g.line(ramp.at(t), (x, 12), (x + 2, 29), 2);
                g.ell(ramp, x - 4, 2, 9, 12);
            }
        }
        _ => {
            // A billhook: a wooden handle, a curved blade hooked at its tip.
            g.line(trim.at(Tone::Base), (6, 28), (14, 18), 4);
            g.poly(ramp, &[(13, 19), (17, 15), (24, 5), (29, 4), (27, 9), (24, 8), (20, 18), (16, 22)], 2);
        }
    }
}

/// A mark in the middle of a disc or over an object.
fn mark(g: &mut G<'_>, m: IconMark, ramp: Ramp) {
    let (lit, base, dark) = (ramp.at(Tone::High), ramp.at(Tone::Light), ramp.at(Tone::Shade));
    match m {
        IconMark::None | IconMark::Cork => {}
        IconMark::Flame => {
            g.poly(ramp, &[(16, 6), (22, 16), (21, 23), (16, 26), (11, 23), (10, 16), (14, 12)], 3);
            g.fill(lit, 15, 18, 3, 6);
        }
        IconMark::Frost => {
            for (a, b) in [((16, 6), (16, 26)), ((7, 11), (25, 21)), ((7, 21), (25, 11))] {
                g.line(base, a, b, 2);
            }
        }
        IconMark::Leaf | IconMark::Thorn => {
            g.poly(ramp, &[(8, 24), (12, 12), (24, 7), (21, 20)], 2);
            g.line(dark, (9, 23), (22, 9), 1);
        }
        IconMark::Bolt => g.poly(ramp, &[(18, 4), (10, 17), (15, 17), (12, 28), (22, 13), (17, 13), (21, 4)], 1),
        IconMark::Burst | IconMark::Star => {
            for (a, b) in [((16, 5), (16, 27)), ((5, 16), (27, 16)), ((8, 8), (24, 24)), ((24, 8), (8, 24))] {
                g.line(base, a, b, 2);
            }
            g.ell(ramp, 12, 12, 8, 8);
        }
        IconMark::Fist | IconMark::Hammer => {
            g.line(dark, (10, 24), (20, 12), 3);
            g.poly(ramp, &[(15, 6), (25, 12), (22, 17), (12, 11)], 1);
        }
        IconMark::Web => {
            for (a, b) in [((16, 5), (16, 27)), ((5, 16), (27, 16)), ((8, 8), (24, 24)), ((24, 8), (8, 24))] {
                g.line(base, a, b, 1);
            }
            let r = g.r(10, 10, 12, 12);
            g.c.ellipse(r, base, 4);
            let r = g.r(11, 11, 10, 10);
            g.c.ellipse(r, dark, 4);
        }
        IconMark::Skull => {
            g.ell(Ramp::Bone, 9, 7, 14, 13);
            g.fill(Ramp::Bone.at(Tone::Base), 12, 19, 8, 5);
            g.fill(Ix::SEAM, 12, 12, 3, 3);
            g.fill(Ix::SEAM, 18, 12, 3, 3);
        }
        IconMark::Shield => g.poly(ramp, &[(8, 7), (24, 7), (23, 18), (16, 26), (9, 18)], 3),
        IconMark::Drop => {
            g.poly(ramp, &[(16, 5), (22, 17), (20, 24), (16, 26), (12, 24), (10, 17)], 3);
            g.dot(lit, 14, 17);
        }
        IconMark::Heart => {
            g.ell(ramp, 8, 9, 9, 9);
            g.ell(ramp, 15, 9, 9, 9);
            g.poly(ramp, &[(8, 14), (24, 14), (16, 25)], 1);
        }
    }
}
