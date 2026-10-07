//! Icons (ART.md §2.5, §8 step 6): an item, a spell or a status as one object, drawn at 32 x 32
//! for the bag and drawn again at 16 x 16 for a chip, never downscaled. Every shape is laid out
//! on a 32-unit grid and placed at the size it is drawn at, so the chip is its own render with
//! its own clusters; details under 2 px are left out of it.
//!
//! Each part is a silhouette painted as a **material** ([`Mat`]): metal is hard-banded with a
//! dark core, a specular edge and a glint; glass is dark and clear with a rim of refracted
//! light and a hot streak; cloth is soft and never shines; food and leather are soft with a
//! little gloss; fruit and glaze are glossy; stone is faceted; paper is pale with a shadowed
//! edge. Light comes from the top-left across the whole object as well as across each form, a
//! tone of light bounces back inside the far edge, and deliberate details (teeth, seams, a wax
//! seal, a crust's scoring) are laid over the shading last. The selective outline finishes it.
//!
//! An icon is at most 24 colours. The slot behind it is the chrome's (`chrome::slot`), not the
//! icon's. A potion, an orb or a light stone glows (`glow`): its liquid or its heart emits, so
//! the bar glows a little at night.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::grid::Rect;
use jane_core::num::isqrt;
use jane_data::{IconClass, IconLook, IconMark};

use crate::canvas::{BAKE_LIGHT, Canvas, UNIT, Z, normal};
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone};
use crate::sprite::{FrameId, Role, SpriteSet};

/// The two sizes an icon is drawn at.
pub const SIZES: [i32; 2] = [32, 16];

/// The icons' own hash salt: flecks of tweed, spots on a crust, a crepe's browning.
const SALT: u32 = 0x4943_4f4e;

/// How light falls on a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mat {
    /// Polished metal: hard bands from a dark core to a specular edge and a glint.
    Metal,
    /// Fruit skin, glaze, a liquid: soft bands, a bright highlight.
    Gloss,
    /// Bread, leather, wood, an egg, wax: soft bands, a little lift, no glint.
    Soft,
    /// Cloth and wool: low contrast, never shines.
    Cloth,
    /// Stone and coal: firm bands.
    Stone,
    /// Paper: pale, its shade a mid.
    Paper,
}

impl Mat {
    /// The lambert term (1/127ths) at which each tone starts, darkest first; `NEVER` for a tone
    /// the material does not use.
    const fn bands(self) -> [i32; 8] {
        const NEVER: i32 = i32::MAX;
        const ANY: i32 = i32::MIN;
        match self {
            Mat::Metal => [ANY, 5, 40, 72, NEVER, 100, 112, 120],
            Mat::Gloss => [ANY, -12, 22, 52, 78, 96, 110, 122],
            Mat::Soft => [NEVER, ANY, 18, 50, 80, 102, 118, NEVER],
            Mat::Cloth => [NEVER, ANY, 24, 58, 86, 108, NEVER, NEVER],
            Mat::Stone => [NEVER, ANY, 14, 48, 82, 104, 118, NEVER],
            Mat::Paper => [NEVER, NEVER, ANY, 28, 70, 96, 116, NEVER],
        }
    }

    fn tone(self, lam: i32) -> Tone {
        let b = self.bands();
        let k = b.iter().rposition(|&t| lam >= t).unwrap_or(0);
        Tone::ALL[k]
    }

    /// Whether light bounces back inside its far edge.
    const fn bounce(self) -> bool {
        matches!(self, Mat::Metal | Mat::Gloss | Mat::Soft | Mat::Stone)
    }
}

/// How a part's normals are made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    /// A soft volume: the normal leans out toward the nearest edge within this many grid units.
    Dome(i32),
    /// An upright cylinder: the normal turns across each row.
    Upright,
    /// A lying cylinder: the normal turns down each column.
    Lying,
    /// A flat face tilted by `(nx, ny)` (1/127ths).
    Face(i32, i32),
}

/// A silhouette drawn on the 32-unit grid into a mask at size `s`.
struct M<'a> {
    m: &'a mut Canvas,
    s: i32,
}

impl M<'_> {
    fn v(&self, u: i32) -> i32 {
        u * self.s / 32
    }
    fn p(&self, (x, y): (i32, i32)) -> (i32, i32) {
        (self.v(x), self.v(y))
    }
    fn r(&self, x: i32, y: i32, w: i32, h: i32) -> Rect {
        let (x0, y0) = (self.v(x), self.v(y));
        Rect::new(x0, y0, (self.v(x + w) - x0).max(1), (self.v(y + h) - y0).max(1))
    }
    fn ell(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let r = self.r(x, y, w, h);
        self.m.ellipse(r, Ix::INK, 1);
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let r = self.r(x, y, w, h);
        self.m.fill_rect(r, Ix::INK, 1);
    }
    fn poly(&mut self, pts: &[(i32, i32)]) {
        let pts: Vec<(i32, i32)> = pts.iter().map(|&q| self.p(q)).collect();
        self.m.polyline_fill(&pts, Ix::INK, 1);
    }
    /// A stroke `w` grid units wide (two px at the least).
    fn line(&mut self, a: (i32, i32), b: (i32, i32), w: i32) {
        let (a, b) = (self.p(a), self.p(b));
        let w = (w * self.s / 32).max(2);
        self.m.line(a, b, Ix::INK, w, 1);
    }
    fn cut_ell(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let r = self.r(x, y, w, h);
        self.m.ellipse(r, Ix::CLEAR, 0);
    }
    fn cut_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let r = self.r(x, y, w, h);
        self.m.fill_rect(r, Ix::CLEAR, 0);
    }
    fn cut_poly(&mut self, pts: &[(i32, i32)]) {
        let pts: Vec<(i32, i32)> = pts.iter().map(|&q| self.p(q)).collect();
        self.m.polyline_fill(&pts, Ix::CLEAR, 0);
    }
}

/// A drawing on the 32-unit grid placed at size `s`.
struct G<'a> {
    c: &'a mut Canvas,
    s: i32,
    /// Parts drawn from here on emit (a potion's liquid, a stone's heart).
    glow: bool,
}

/// The chamfer distance of every drawn px of `m` to clear (tenths of a px; the canvas edge is
/// clear).
fn distance(m: &Canvas) -> Vec<i32> {
    let (w, h) = (m.w(), m.h());
    let mut d: Vec<i32> = (0..w * h).map(|i| if m.get(i % w, i / w).is_opaque() { i32::MAX / 2 } else { 0 }).collect();
    let get = |d: &[i32], x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { 0 } else { d[(y * w + x) as usize] };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if d[i] != 0 {
                let m = (get(&d, x - 1, y) + 10)
                    .min(get(&d, x, y - 1) + 10)
                    .min(get(&d, x - 1, y - 1) + 14)
                    .min(get(&d, x + 1, y - 1) + 14);
                d[i] = d[i].min(m);
            }
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = (y * w + x) as usize;
            if d[i] != 0 {
                let m = (get(&d, x + 1, y) + 10)
                    .min(get(&d, x, y + 1) + 10)
                    .min(get(&d, x + 1, y + 1) + 14)
                    .min(get(&d, x - 1, y + 1) + 14);
                d[i] = d[i].min(m);
            }
        }
    }
    d
}

fn lambert(n: [i32; 2]) -> i32 {
    let nz = isqrt((UNIT * UNIT - (n[0] * n[0] + n[1] * n[1]).min(UNIT * UNIT)) as u64) as i32;
    (n[0] * BAKE_LIGHT[0] + n[1] * BAKE_LIGHT[1] + nz * BAKE_LIGHT[2]) / UNIT
}

impl G<'_> {
    fn v(&self, u: i32) -> i32 {
        u * self.s / 32
    }
    fn big(&self) -> bool {
        self.s >= 32
    }

    /// A part: the silhouette `f` draws, painted in `ramp` as `mat` with normals by `form`,
    /// standing `z` px. `sep`: a seam in its own dark where it lies over what is already drawn.
    #[allow(clippy::too_many_arguments)]
    fn part(&mut self, ramp: Ramp, mat: Mat, form: Form, z: Z, sep: bool, f: impl FnOnce(&mut M<'_>)) {
        let s = self.s;
        let mut mask = Canvas::new(s, s);
        f(&mut M { m: &mut mask, s });
        let d = distance(&mask);
        let at = |x: i32, y: i32| if x < 0 || y < 0 || x >= s || y >= s { 0 } else { d[(y * s + x) as usize] };
        // The part's box: light falls across the whole of it from the top-left.
        let (mut x0, mut y0, mut x1, mut y1) = (s, s, -1, -1);
        for y in 0..s {
            for x in 0..s {
                if at(x, y) > 0 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        if x1 < 0 {
            return;
        }
        let span = (x1 - x0 + y1 - y0 + 2).max(1);
        let mut l = Canvas::new(s, s);
        l.set_emitting(self.glow);
        let row_span = |y: i32| {
            let a = (0..s).find(|&x| at(x, y) > 0).unwrap_or(0);
            let b = (0..s).rev().find(|&x| at(x, y) > 0).unwrap_or(0);
            (a, b)
        };
        let col_span = |x: i32| {
            let a = (0..s).find(|&y| at(x, y) > 0).unwrap_or(0);
            let b = (0..s).rev().find(|&y| at(x, y) > 0).unwrap_or(0);
            (a, b)
        };
        for y in 0..s {
            for x in 0..s {
                let dd = at(x, y);
                if dd == 0 {
                    continue;
                }
                let (gx, gy) = (at(x + 1, y) - at(x - 1, y), at(x, y + 1) - at(x, y - 1));
                let len = isqrt((gx * gx + gy * gy) as u64) as i32;
                let dome = |r: i32| {
                    let r10 = (r * s / 32).max(1) * 10;
                    let u = ((r10 - dd + 5) * UNIT / r10).clamp(0, UNIT);
                    if len == 0 || u == 0 { (0, 0) } else { (-gx * u * 7 / (8 * len), -gy * u * 7 / (8 * len)) }
                };
                let (nx, ny) = match form {
                    Form::Dome(r) => dome(r),
                    Form::Upright => {
                        let (a, b) = row_span(y);
                        let t = (2 * x - a - b) * UNIT / (b - a + 1).max(1);
                        (t * 7 / 8, dome(2).1 / 2)
                    }
                    Form::Lying => {
                        let (a, b) = col_span(x);
                        let t = (2 * y - a - b) * UNIT / (b - a + 1).max(1);
                        (dome(2).0 / 2, t * 7 / 8)
                    }
                    Form::Face(nx, ny) => (nx, ny),
                };
                let across = 24 - 48 * (x - x0 + y - y0) / span;
                let lam = lambert([nx, ny]) + across;
                let mut tone = mat.tone(lam);
                // Light bounced back inside the far edge.
                if mat.bounce() && (11..=24).contains(&dd) && gx + gy < -6 && tone <= Tone::Mid {
                    tone = tone.step(1);
                }
                let h = i32::from(z.lo) + (i32::from(z.hi) - i32::from(z.lo)) * dd.min(30) / 30;
                l.put(x, y, ramp.at(tone), normal(nx, ny), h.clamp(1, 255) as u8);
            }
        }
        l.declutter(ramp);
        if sep {
            for y in 0..s {
                for x in 0..s {
                    let Some((r, t)) = Ramp::of(l.get(x, y)) else { continue };
                    let under =
                        |dx: i32, dy: i32| !l.get(x + dx, y + dy).is_opaque() && self.c.get(x + dx, y + dy).is_opaque();
                    if under(1, 0) || under(0, 1) {
                        l.recolour(x, y, r.at(Tone::Deep));
                    } else if under(-1, 0) || under(0, -1) {
                        l.recolour(x, y, r.at(t.step(-2).max(Tone::Shade)));
                    }
                }
            }
        }
        self.c.stamp(&l, 0, 0);
    }

    fn dome(&mut self, ramp: Ramp, mat: Mat, r: i32, f: impl FnOnce(&mut M<'_>)) {
        self.part(ramp, mat, Form::Dome(r), Z::new(1, 3), false, f);
    }
    fn over(&mut self, ramp: Ramp, mat: Mat, r: i32, f: impl FnOnce(&mut M<'_>)) {
        self.part(ramp, mat, Form::Dome(r), Z::new(1, 3), true, f);
    }

    // --- Details, laid over the shading on the 32-unit grid.

    fn px(&mut self, ix: Ix, x: i32, y: i32) {
        let (x, y) = (self.v(x), self.v(y));
        self.c.dot(x, y, ix, 1);
    }
    /// A 2 x 2 cluster at 32, one px at 16.
    fn blob(&mut self, ix: Ix, x: i32, y: i32) {
        let (px, py) = (self.v(x), self.v(y));
        let n = if self.big() { 2 } else { 1 };
        self.c.fill_rect(Rect::new(px, py, n, n), ix, 1);
    }
    fn rect(&mut self, ix: Ix, x: i32, y: i32, w: i32, h: i32) {
        let (x0, y0) = (self.v(x), self.v(y));
        let r = Rect::new(x0, y0, (self.v(x + w) - x0).max(1), (self.v(y + h) - y0).max(1));
        self.c.fill_rect(r, ix, 1);
    }
    /// A line `w` px wide (not scaled: a crease is a px at either size).
    fn seg(&mut self, ix: Ix, a: (i32, i32), b: (i32, i32), w: i32) {
        let (a, b) = ((self.v(a.0), self.v(a.1)), (self.v(b.0), self.v(b.1)));
        self.c.line(a, b, ix, w, 1);
    }
    /// A line through several points, a px wide.
    fn path(&mut self, ix: Ix, pts: &[(i32, i32)]) {
        for w in pts.windows(2) {
            self.seg(ix, w[0], w[1], 1);
        }
    }
    /// Recolour `ramp`'s px on the segment to `tone` (a crease in what is there, nothing else).
    fn crease(&mut self, ramp: Ramp, tone: Tone, a: (i32, i32), b: (i32, i32)) {
        let (a, b) = ((self.v(a.0), self.v(a.1)), (self.v(b.0), self.v(b.1)));
        let c = &mut *self.c;
        crate::canvas::bresenham(a.0, a.1, b.0, b.1, |x, y| c.tint(x, y, ramp, tone));
    }
    /// Recolour `ramp`'s px at a grid point.
    fn tint(&mut self, ramp: Ramp, tone: Tone, x: i32, y: i32) {
        let (x, y) = (self.v(x), self.v(y));
        self.c.tint(x, y, ramp, tone);
    }
    /// Every px of `ramp` inside the grid rect to `tone`.
    fn tint_rect(&mut self, ramp: Ramp, tone: Tone, x: i32, y: i32, w: i32, h: i32) {
        let (x0, y0, x1, y1) = (self.v(x), self.v(y), self.v(x + w), self.v(y + h));
        for py in y0..y1.max(y0 + 1) {
            for px in x0..x1.max(x0 + 1) {
                self.c.tint(px, py, ramp, tone);
            }
        }
    }
    /// Every px of `ramp` whose position `keep` accepts (in px) steps `by` tones.
    fn step_where(&mut self, ramp: Ramp, by: i32, keep: impl Fn(i32, i32) -> bool) {
        for y in 0..self.s {
            for x in 0..self.s {
                if !keep(x, y) {
                    continue;
                }
                if let Some((r, t)) = Ramp::of(self.c.get(x, y)) {
                    if r == ramp {
                        self.c.recolour(x, y, ramp.at(t.step(by)));
                    }
                }
            }
        }
    }
    /// Clusters of `n` hashed px flecks of `tone` over what is drawn in `ramp`: tweed, a crust's
    /// flour, a potato's dirt. Two px each at 32.
    fn flecks(&mut self, ramp: Ramp, tone: Tone, n: u32, seed: u32, area: (i32, i32, i32, i32)) {
        let (ax, ay, aw, ah) = area;
        for k in 0..n {
            let hh = h32(seed, k, SALT);
            let x = ax + below(hh, aw.max(1) as u32) as i32;
            let y = ay + below(hh.rotate_left(13), ah.max(1) as u32) as i32;
            let (px, py) = (self.v(x), self.v(y));
            let horiz = hh & 0x8000 != 0;
            self.c.tint(px, py, ramp, tone);
            if self.big() {
                if horiz {
                    self.c.tint(px + 1, py, ramp, tone);
                } else {
                    self.c.tint(px, py + 1, ramp, tone);
                }
            }
        }
    }
}

/// A white for glints: the hottest px on glass, metal and gloss.
fn glint() -> Ix {
    Ramp::HairWhite.at(Tone::High)
}

/// Render icon `look` at both sizes: variant 0 is the 32 x 32, variant 1 the 16 x 16 chip.
pub fn render(look: &IconLook) -> Result<Vec<SpriteSet>, String> {
    let ramp = crate::person::ramp(look.ramp)?;
    let trim = look.trim.map_or(Ok(default_trim(look.class)), crate::person::ramp)?;
    let mut out = Vec::new();
    for s in SIZES {
        let mut c = Canvas::new(s, s);
        draw(&mut G { c: &mut c, s, glow: false }, look, ramp, trim);
        finish(&mut c);
        let emits = if look.glow { vec![Role::Glass] } else { Vec::new() };
        out.push(SpriteSet {
            w: s,
            h: s,
            ax: 0,
            ay: 0,
            frames: vec![(FrameId::Base, c)],
            roles: vec![(Role::Body, ramp)],
            emits,
        });
    }
    Ok(out)
}

fn default_trim(class: IconClass) -> Ramp {
    match class {
        IconClass::Flask | IconClass::Vial => Ramp::Glass,
        IconClass::Key | IconClass::Ring | IconClass::Amulet | IconClass::Spectacles => Ramp::Brass,
        IconClass::Herb | IconClass::Bloom | IconClass::Fruit | IconClass::Grapes | IconClass::Moss => Ramp::Leaf,
        IconClass::Spell | IconClass::Status => Ramp::Iron,
        IconClass::Tool | IconClass::Spanner | IconClass::Scissors | IconClass::Spoons | IconClass::Can => {
            Ramp::WoodDark
        }
        _ => Ramp::Leather,
    }
}

/// The clean-up and the line: spikes off, the selective outline, what glows kept lit, the icon
/// lying flat.
fn finish(c: &mut Canvas) {
    let glow: Vec<(i32, i32, Ix)> = (0..c.h())
        .flat_map(|y| (0..c.w()).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let e = c.emissive_at(x, y);
            (e != Ix::CLEAR).then_some((x, y, e))
        })
        .collect();
    c.despike();
    c.outline();
    c.relight(&glow);
    c.cap_heights(4);
}

fn draw(g: &mut G<'_>, look: &IconLook, ramp: Ramp, trim: Ramp) {
    use IconClass as C;
    match look.class {
        C::Flask => flask(g, look.glow, ramp, trim),
        C::Vial => vial(g, ramp, trim),
        C::Key => key(g, ramp),
        C::Bar => bar(g, ramp),
        C::Orb => orb(g, look.glow, ramp),
        C::Stone => stone(g, ramp),
        C::Gem => gem(g, look.glow, ramp),
        C::Mushroom => mushroom(g, ramp, trim),
        C::Bloom => bloom(g, ramp, trim),
        C::Herb => root(g, ramp, trim),
        C::Moss => moss(g, ramp, trim),
        C::Fruit => apple(g, ramp, trim),
        C::Grapes => grapes(g, ramp, trim),
        C::Egg => egg(g, ramp),
        C::Loaf => loaf(g, ramp),
        C::Crepe => crepe(g, ramp),
        C::Meat => meat(g, ramp),
        C::Potatoes => potatoes(g, ramp),
        C::Pot => honey(g, ramp, trim),
        C::Tin => tin(g, ramp),
        C::Can => oil_can(g, ramp, trim),
        C::Glove => glove(g, ramp, trim),
        C::Coat => coat(g, ramp),
        C::Scarf => scarf(g, ramp, trim),
        C::Linen => linen(g, ramp, trim),
        C::Hat => hat(g, ramp, trim),
        C::Cap => cap(g, ramp),
        C::Fleece => fleece(g, ramp),
        C::Letter => letter(g, ramp),
        C::Parcel => parcel(g, ramp, trim),
        C::Sack => sack(g, ramp, trim),
        C::Satchel => satchel(g, ramp, trim),
        C::Tool => billhook(g, ramp, trim),
        C::Scissors => scissors(g, ramp, trim),
        C::Spanner => spanner(g, ramp),
        C::Spoons => spoons(g, ramp),
        C::Net => net(g, ramp, trim),
        C::Plate => plate(g, ramp),
        C::Ring => ring(g, ramp, trim),
        C::Amulet => amulet(g, ramp, trim),
        C::Spectacles => spectacles(g, ramp),
        C::Logs => logs(g, ramp, trim),
        C::Sticks => sticks(g, ramp, trim),
        C::Matchbox => matchbox(g, ramp, trim),
        C::Butterfly => butterfly(g, ramp),
        C::Tortoise => tortoise(g, ramp),
        C::Dust => dust(g, look.glow, ramp),
        C::Spell => spell(g, look, ramp),
        C::Status => status(g, look, ramp, trim),
    }
    if look.mark != IconMark::None && !matches!(look.class, C::Spell | C::Status) {
        mark(g, look.mark, trim, Ink::Bright);
    }
}

// ---------------------------------------------------------------------------------------------
// Glass and what is in it

/// A round-bellied potion: dark clear glass, the liquid lit from within up to its meniscus, a
/// cork, the glass's rim of refracted light and its hot streak.
fn flask(g: &mut G<'_>, glows: bool, liquid: Ramp, glass: Ramp) {
    let big = g.big();
    // The glass: a belly and a neck with a lip.
    g.part(glass, Mat::Cloth, Form::Dome(4), Z::new(1, 3), false, |m| {
        m.ell(4, 11, 24, 20);
        m.rect(12, 5, 8, 9);
        m.rect(11, 4, 10, 3);
    });
    // Empty glass is the slot seen through it: dark.
    g.step_where(glass, -3, |_, _| true);
    // The liquid, up to a meniscus a little above the belly's middle.
    if liquid != Ramp::Glass {
        g.glow = glows;
        g.part(liquid, Mat::Gloss, Form::Dome(6), Z::new(2, 3), false, |m| {
            m.ell(6, 13, 20, 16);
            m.cut_rect(0, 0, 32, 17);
        });
        g.glow = false;
        // Light through the liquid: its heart, low and toward the far side, is its brightest.
        let (cx, cy, r) = if big { (18, 25, 4) } else { (9, 12, 2) };
        g.step_where(liquid, 1, |x, y| (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r);
        g.seg(liquid.at(Tone::High), (8, 17), (23, 17), 1);
        if big {
            g.seg(liquid.at(Tone::Light), (7, 18), (24, 18), 1);
        }
    }
    // The cork, and the lip's ring of light.
    g.part(Ramp::WoodPale, Mat::Soft, Form::Upright, Z::new(3, 4), true, |m| m.rect(12, 1, 8, 5));
    g.seg(Ramp::WoodPale.at(Tone::High), (13, 1), (18, 1), 1);
    g.seg(glass.at(Tone::Light), (12, 6), (19, 6), 1);
    // Refracted light round the far rim of the belly, the hot streak and a glint.
    if big {
        g.path(glass.at(Tone::Light), &[(25, 15), (26, 19), (26, 23), (24, 27), (21, 29)]);
        g.path(glint(), &[(8, 15), (7, 18), (7, 21)]);
        g.path(glass.at(Tone::High), &[(9, 14), (8, 16)]);
        g.px(glint(), 10, 13);
        g.seg(glass.at(Tone::Light), (13, 7), (13, 11), 1);
    } else {
        g.px(glass.at(Tone::Light), 24, 22);
        g.rect(glint(), 7, 15, 2, 4);
    }
}

/// A tall vial with a cork: water in it, or nothing.
fn vial(g: &mut G<'_>, liquid: Ramp, glass: Ramp) {
    g.part(glass, Mat::Cloth, Form::Upright, Z::new(1, 3), false, |m| {
        m.rect(11, 7, 10, 17);
        m.ell(11, 18, 10, 12);
        m.rect(10, 6, 12, 3);
    });
    g.step_where(glass, -3, |_, _| true);
    if liquid != Ramp::Glass {
        g.part(liquid, Mat::Gloss, Form::Upright, Z::new(2, 3), false, |m| {
            m.rect(13, 15, 6, 9);
            m.ell(13, 19, 6, 9);
        });
        g.seg(liquid.at(Tone::High), (13, 15), (18, 15), 1);
    }
    g.part(Ramp::WoodPale, Mat::Soft, Form::Upright, Z::new(3, 4), true, |m| m.rect(12, 1, 8, 6));
    g.seg(Ramp::WoodPale.at(Tone::High), (13, 1), (18, 1), 1);
    g.seg(glass.at(Tone::High), (11, 8), (20, 8), 1);
    if g.big() {
        g.seg(glint(), (12, 10), (12, 22), 1);
        g.px(glass.at(Tone::Light), 19, 26);
        g.seg(glass.at(Tone::Light), (20, 12), (20, 22), 1);
    } else {
        g.rect(glint(), 12, 10, 2, 8);
    }
}

// ---------------------------------------------------------------------------------------------
// Metal

/// A key lying on the diagonal: a round bow with its hole, a collar, the shank, and a bit of two
/// teeth with a notch between; polished metal, its edges catching the light.
fn key(g: &mut G<'_>, metal: Ramp) {
    let big = g.big();
    // Along the shank `t`, out from it toward the bottom-left `k`: the point `(t - k, t + k)`.
    let at = |t: i32, k: i32| (t - k, t + k);
    g.part(metal, Mat::Metal, Form::Dome(3), Z::new(1, 3), false, |m| {
        m.ell(0, 0, 16, 16);
        m.line((12, 12), (26, 26), 5);
        if big {
            m.poly(&[at(17, 0), at(25, 0), at(25, 6), at(17, 6)]);
        } else {
            m.poly(&[at(16, 0), at(25, 0), at(25, 7), at(16, 7)]);
        }
    });
    // The hole through the bow, and the notch that parts the teeth.
    {
        let s = g.s;
        let mut cut = Canvas::new(s, s);
        let mut mm = M { m: &mut cut, s };
        mm.ell(5, 5, 6, 6);
        if big {
            mm.poly(&[at(20, 3), at(22, 3), at(22, 9), at(20, 9)]);
        } else {
            mm.poly(&[at(19, 4), at(21, 4), at(21, 9), at(19, 9)]);
        }
        for y in 0..s {
            for x in 0..s {
                if cut.get(x, y).is_opaque() {
                    g.c.clear_px(x, y);
                }
            }
        }
    }
    // The collar: a lit ring round the shank where it leaves the bow.
    g.crease(metal, Tone::Glint, (12, 17), (17, 12));
    g.crease(metal, Tone::Shade, (13, 18), (18, 13));
    if big {
        // The shank's lit ridge, the bow's lit inner lip, glints.
        g.crease(metal, Tone::High, (18, 16), (25, 23));
        g.crease(metal, Tone::Glint, (19, 17), (21, 19));
        g.crease(metal, Tone::Shade, (5, 12), (11, 12));
        g.crease(metal, Tone::High, (4, 5), (4, 9));
        g.px(glint(), 3, 3);
        g.px(glint(), 4, 2);
        g.crease(metal, Tone::Deep, at(17, 1), at(24, 1));
    } else {
        g.crease(metal, Tone::High, (16, 16), (24, 24));
        g.px(glint(), 2, 2);
    }
}

/// An ingot in the 3/4 view: a narrow top with a sheen and a maker's stamp, the long face
/// flaring down toward the viewer, the end in shade.
fn bar(g: &mut G<'_>, metal: Ramp) {
    g.part(metal, Mat::Metal, Form::Face(0, 70), Z::new(1, 2), false, |m| {
        m.poly(&[(6, 15), (25, 15), (29, 24), (2, 24)]);
    });
    g.part(metal, Mat::Metal, Form::Face(90, 10), Z::new(1, 2), false, |m| {
        m.poly(&[(22, 9), (25, 15), (29, 24), (26, 14)]);
    });
    g.part(metal, Mat::Metal, Form::Face(-30, -50), Z::new(2, 3), false, |m| {
        m.poly(&[(9, 9), (22, 9), (25, 15), (6, 15)]);
    });
    g.step_where(metal, -1, |_, _| true);
    // The top's front edge bright, the face's foot dark; a sheen across both.
    g.seg(metal.at(Tone::Glint), (7, 15), (24, 15), 1);
    g.crease(metal, Tone::Shade, (5, 16), (25, 16));
    g.crease(metal, Tone::High, (12, 10), (10, 14));
    g.crease(metal, Tone::High, (13, 10), (11, 14));
    g.crease(metal, Tone::Light, (13, 17), (12, 23));
    g.crease(metal, Tone::Light, (14, 17), (13, 23));
    g.crease(metal, Tone::Deep, (27, 17), (28, 23));
    if g.big() {
        // The stamp: a sunk panel with its lower lip lit.
        g.rect(metal.at(Tone::Shade), 15, 11, 5, 1);
        g.rect(metal.at(Tone::Base), 15, 12, 5, 1);
        g.seg(metal.at(Tone::High), (15, 13), (19, 13), 1);
        g.px(glint(), 10, 10);
        g.px(metal.at(Tone::Mid), 3, 23);
    } else {
        g.px(glint(), 9, 10);
    }
}

/// A ball of polished gold: a mirror of the room, the bright sky over a sharp horizon and the
/// dark ground under it, a window in the sky, the floor's light bounced back along its foot.
fn orb(g: &mut G<'_>, glows: bool, ramp: Ramp) {
    g.glow = glows;
    g.dome(ramp, Mat::Metal, 14, |m| m.ell(5, 5, 22, 22));
    g.glow = false;
    let (h, big) = (g.v(17), g.big());
    g.step_where(ramp, -2, |_, y| y > h);
    g.step_where(ramp, 1, |_, y| y == h);
    // The floor's light bounced back along the foot of the ball.
    g.crease(ramp, Tone::Light, (10, 25), (20, 25));
    if big {
        g.rect(glint(), 10, 9, 3, 2);
        g.rect(ramp.at(Tone::Glint), 10, 11, 2, 1);
        g.px(ramp.at(Tone::Glint), 14, 8);
        g.crease(ramp, Tone::High, (21, 23), (23, 21));
    } else {
        g.px(glint(), 10, 9);
    }
}

// ---------------------------------------------------------------------------------------------
// Stone

/// A facet of a stone or a gem: the triangle `a, b, c` flat, lit by its own tilt.
fn facet(g: &mut G<'_>, ramp: Ramp, mat: Mat, pts: &[(i32, i32)], tilt: (i32, i32)) {
    g.part(ramp, mat, Form::Face(tilt.0, tilt.1), Z::new(2, 3), false, |m| m.poly(pts));
}

/// Tilt of a facet whose outer edge's middle is `(x, y)` about the centre `(cx, cy)`: it leans
/// out toward that edge.
fn lean(x: i32, y: i32, cx: i32, cy: i32, k: i32) -> (i32, i32) {
    let (dx, dy) = (x - cx, y - cy);
    let len = isqrt((dx * dx + dy * dy) as u64).max(1) as i32;
    (dx * k / len, dy * k / len)
}

/// A rock, a stone, a lump of coal: an irregular lump cut in facets, each lit by its own tilt,
/// a crack, and on coal a glassy glint on every facet's edge.
fn stone(g: &mut G<'_>, ramp: Ramp) {
    let coal = ramp == Ramp::ClothBlack;
    let slate = ramp == Ramp::Slate;
    let rim: &[(i32, i32)] = if slate {
        &[(4, 15), (10, 8), (21, 6), (28, 12), (27, 22), (17, 27), (7, 25)]
    } else if coal {
        &[(5, 13), (12, 5), (22, 7), (28, 15), (24, 25), (13, 28), (4, 22)]
    } else {
        &[(4, 17), (8, 9), (17, 5), (26, 9), (28, 20), (21, 27), (9, 27)]
    };
    let (cx, cy) = if slate { (15, 15) } else { (14, 14) };
    for k in 0..rim.len() {
        let (a, b) = (rim[k], rim[(k + 1) % rim.len()]);
        let t = lean((a.0 + b.0) / 2, (a.1 + b.1) / 2, cx, cy, 70);
        facet(g, ramp, Mat::Stone, &[(cx, cy), a, b], t);
    }
    // The top facet, flat and lit.
    let top: Vec<(i32, i32)> = rim.iter().map(|&(x, y)| ((x + 2 * cx) / 3, (y + 2 * cy) / 3)).collect();
    g.part(ramp, Mat::Stone, Form::Face(-30, -30), Z::new(3, 3), false, |m| m.poly(&top));
    // The facet edges: a lit ridge on the lit side.
    for (k, &(x, y)) in top.iter().enumerate() {
        let (ox, oy) = rim[k];
        let lit = ox + oy < cx + cy;
        g.crease(ramp, if lit { Tone::High } else { Tone::Shade }, (x, y), (ox, oy));
    }
    if coal {
        for &(x, y) in top.iter().take(4) {
            g.px(Ramp::HairWhite.at(Tone::Light), x, y);
        }
        g.px(glint(), top[1].0 + 1, top[1].1 + 1);
    } else if g.big() {
        // A crack with its lit lip, a pit.
        g.crease(ramp, Tone::Deep, (18, 11), (20, 16));
        g.crease(ramp, Tone::Deep, (20, 16), (19, 20));
        g.crease(ramp, Tone::Light, (19, 11), (21, 16));
        g.px(ramp.at(Tone::Shade), 11, 20);
        g.px(ramp.at(Tone::Light), 11, 21);
        if !slate {
            // Lichen where the weather sits.
            g.blob(Ramp::LeafOlive.at(Tone::Light), 8, 23);
            g.px(Ramp::LeafOlive.at(Tone::Base), 10, 24);
        }
    }
}

/// A cut gem: a table facet on top, the crown's facets round it, the pavilion dark below with
/// one bright facet of light thrown back from inside; a glowing heart when it glows.
fn gem(g: &mut G<'_>, glows: bool, ramp: Ramp) {
    g.glow = glows;
    // The pavilion: two big facets down to the point.
    facet(g, ramp, Mat::Gloss, &[(4, 12), (16, 29), (16, 13)], (-20, 60));
    facet(g, ramp, Mat::Gloss, &[(16, 13), (16, 29), (28, 12)], (70, 50));
    facet(g, ramp, Mat::Gloss, &[(10, 13), (16, 29), (22, 13)], (0, 40));
    // The crown: a band of facets round the table.
    facet(g, ramp, Mat::Gloss, &[(4, 12), (9, 6), (12, 9), (10, 13)], (-80, -40));
    facet(g, ramp, Mat::Gloss, &[(9, 6), (23, 6), (20, 9), (12, 9)], (-10, -90));
    facet(g, ramp, Mat::Gloss, &[(23, 6), (28, 12), (22, 13), (20, 9)], (80, -30));
    facet(g, ramp, Mat::Gloss, &[(10, 13), (12, 9), (20, 9), (22, 13)], (-30, -30));
    g.glow = false;
    // The girdle's line, the table's edge, the light thrown back up through the pavilion.
    g.crease(ramp, Tone::High, (5, 12), (27, 12));
    g.crease(ramp, Tone::Glint, (12, 9), (20, 9));
    g.crease(ramp, Tone::Light, (13, 15), (16, 25));
    g.crease(ramp, Tone::Deep, (21, 15), (17, 26));
    if g.big() {
        g.px(glint(), 10, 7);
        g.px(glint(), 11, 7);
        g.px(glint(), 14, 17);
        g.px(ramp.at(Tone::Glint), 14, 18);
    } else {
        g.px(glint(), 10, 7);
    }
}

// ---------------------------------------------------------------------------------------------
// Things that grow

/// A fly agaric: a red cap spotted white, its gills in shade under the rim, a pale stem with its
/// ring, on a tuft of moss.
fn mushroom(g: &mut G<'_>, cap: Ramp, stem: Ramp) {
    g.dome(Ramp::Leaf, Mat::Cloth, 3, |m| m.ell(8, 25, 16, 6));
    g.part(stem, Mat::Soft, Form::Upright, Z::new(1, 3), false, |m| {
        m.poly(&[(12, 15), (20, 15), (21, 28), (11, 28)]);
        m.ell(10, 25, 12, 5);
    });
    // The ring, hanging from the stem.
    g.part(stem, Mat::Soft, Form::Upright, Z::new(3, 4), true, |m| m.poly(&[(11, 19), (21, 19), (22, 22), (10, 22)]));
    // The gills: the cap's underside, in the shade under its rim.
    g.part(stem, Mat::Cloth, Form::Face(0, 40), Z::new(3, 3), false, |m| m.ell(5, 13, 22, 6));
    let (y0, y1) = (g.v(13), g.v(19));
    g.step_where(stem, -2, |_, y| y >= y0 && y < y1);
    g.part(cap, Mat::Gloss, Form::Dome(8), Z::new(3, 5), false, |m| {
        m.ell(3, 3, 26, 22);
        m.cut_rect(0, 15, 32, 17);
    });
    for (x, y) in [(10, 7), (18, 5), (22, 10), (7, 12), (14, 11)] {
        g.blob(Ramp::HairWhite.at(Tone::Light), x, y);
    }
    if g.big() {
        g.px(Ramp::HairWhite.at(Tone::High), 10, 7);
        g.px(Ramp::HairWhite.at(Tone::Base), 23, 11);
        g.px(Ramp::HairWhite.at(Tone::Base), 19, 6);
        // Gill lines.
        for x in [9, 13, 17, 21] {
            g.seg(stem.at(Tone::Shade), (x, 15), (x + i32::from(x > 16) - i32::from(x < 16), 17), 1);
        }
    }
}

/// A leaf on the grid: a pointed oval from `a` to `b`, `w` wide, with its midrib.
fn leaf(g: &mut G<'_>, ramp: Ramp, a: (i32, i32), b: (i32, i32), w: i32) {
    let (mx, my) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = isqrt((dx * dx + dy * dy) as u64).max(1) as i32;
    let (px, py) = (-dy * w / (2 * len), dx * w / (2 * len));
    g.part(ramp, Mat::Soft, Form::Dome(3), Z::new(1, 3), true, |m| {
        m.poly(&[a, (mx + px, my + py), b, (mx - px, my - py)]);
    });
    if g.big() {
        g.crease(ramp, Tone::Shade, a, (mx + (b.0 - mx) / 2, my + (b.1 - my) / 2));
    }
}

/// A flower on its stem with two leaves: a rose's whorl, a lily's six points, a pansy's face, or
/// five round petals about a yellow eye.
fn bloom(g: &mut G<'_>, petal: Ramp, green: Ramp) {
    g.seg(green.at(Tone::Shade), (16, 30), (15, 13), 2);
    g.seg(green.at(Tone::Light), (15, 30), (14, 16), 1);
    leaf(g, green, (15, 24), (5, 17), 7);
    leaf(g, green, (16, 21), (27, 16), 7);
    let big = g.big();
    match petal {
        Ramp::Bloom => {
            // A rose: a round head, its petals wrapped in a whorl, a sepal under it.
            g.dome(petal, Mat::Soft, 5, |m| m.ell(7, 2, 18, 15));
            g.path(petal.at(Tone::Shade), &[(11, 11), (13, 7), (18, 6), (21, 9), (19, 12), (15, 11), (15, 9), (17, 9)]);
            if big {
                g.path(petal.at(Tone::High), &[(10, 12), (12, 7), (17, 5)]);
                g.path(petal.at(Tone::Light), &[(16, 13), (20, 12), (22, 9)]);
            }
        }
        Ramp::HairWhite => {
            // A lily: six pointed petals, a trumpet, stamens.
            for (a, b) in [
                ((16, 9), (16, 1)),
                ((16, 9), (25, 5)),
                ((16, 9), (24, 15)),
                ((16, 9), (8, 15)),
                ((16, 9), (7, 5)),
                ((16, 9), (16, 17)),
            ] {
                leaf(g, petal, a, b, 6);
            }
            g.dome(petal, Mat::Soft, 3, |m| m.ell(12, 5, 9, 9));
            g.blob(Ramp::ClothOchre.at(Tone::Base), 15, 7);
            if big {
                g.px(Ramp::ClothOchre.at(Tone::Shade), 18, 9);
                g.px(Ramp::ClothOchre.at(Tone::Shade), 13, 10);
            }
        }
        _ => {
            // Five round petals: the back two, then the front three over them.
            for (x, y) in [(7, 2), (17, 2)] {
                g.dome(petal, Mat::Soft, 4, |m| m.ell(x, y, 10, 10));
            }
            for (x, y) in [(3, 8), (21, 8), (12, 11)] {
                g.over(petal, Mat::Soft, 4, |m| m.ell(x, y, 10, 9));
            }
            let pansy = petal == Ramp::ClothPlum;
            if pansy {
                // A pansy's dark face about its eye.
                g.dome(Ramp::ClothBlack, Mat::Cloth, 2, |m| m.ell(11, 7, 10, 8));
            } else if big {
                // A nasturtium's veins, running in from each petal.
                for (a, b) in [((9, 5), (14, 9)), ((22, 5), (18, 9)), ((6, 12), (13, 12)), ((26, 12), (19, 12))] {
                    g.crease(petal, Tone::Shade, a, b);
                }
            }
            g.dome(Ramp::ClothMustard, Mat::Soft, 2, |m| m.ell(13, 8, 6, 6));
            g.px(Ramp::ClothMustard.at(Tone::High), 14, 9);
        }
    }
}

/// A root: a fat tuber with its rootlets and a tuft of leaves; a snakeroot twisted like its name.
fn root(g: &mut G<'_>, ramp: Ramp, green: Ramp) {
    leaf(g, green, (14, 12), (6, 2), 6);
    leaf(g, green, (16, 12), (17, 1), 6);
    leaf(g, green, (17, 12), (27, 4), 6);
    let big = g.big();
    if ramp == Ramp::Reed {
        // Twisted: an S of root, its turns in shade.
        g.part(ramp, Mat::Soft, Form::Dome(3), Z::new(1, 3), true, |m| {
            m.line((15, 12), (21, 16), 6);
            m.line((21, 16), (11, 22), 6);
            m.line((11, 22), (19, 27), 5);
            m.line((19, 27), (25, 30), 3);
        });
        g.crease(ramp, Tone::Shade, (16, 16), (19, 17));
        g.crease(ramp, Tone::Shade, (14, 23), (17, 25));
        g.crease(ramp, Tone::High, (12, 20), (18, 18));
    } else {
        g.part(ramp, Mat::Soft, Form::Dome(5), Z::new(1, 3), true, |m| {
            m.ell(7, 11, 17, 13);
            m.poly(&[(9, 19), (22, 19), (19, 29), (15, 31)]);
        });
        // Rings round the tuber and rootlets off it.
        for (a, b) in [((9, 16), (22, 15)), ((10, 20), (21, 21)), ((13, 25), (19, 25))] {
            g.crease(ramp, Tone::Shade, a, b);
        }
        g.path(ramp.at(Tone::Shade), &[(8, 21), (4, 24), (3, 28)]);
        g.path(ramp.at(Tone::Shade), &[(23, 20), (27, 22), (28, 26)]);
        if big {
            g.crease(ramp, Tone::High, (10, 13), (15, 12));
        }
    }
    if big {
        g.px(Ramp::Earth.at(Tone::Shade), 12, 18);
        g.px(Ramp::Earth.at(Tone::Shade), 18, 23);
    }
}

/// A cushion of moss on a flat stone: one soft mound in clumps, tufts lit in clusters, a few
/// stalks with red caps standing out of it.
fn moss(g: &mut G<'_>, ramp: Ramp, tuft: Ramp) {
    g.part(Ramp::Stone, Mat::Stone, Form::Dome(4), Z::new(1, 2), false, |m| m.ell(2, 19, 28, 11));
    g.over(ramp, Mat::Cloth, 6, |m| {
        for &(x, y, w) in &[(4, 13, 10), (11, 9, 11), (19, 10, 10), (7, 16, 11), (16, 15, 12), (22, 15, 8)] {
            m.ell(x, y, w, w * 4 / 5);
        }
    });
    // Each clump's crown lit, the hollows between them in shade.
    for &(x, y, w) in &[(4, 13, 10), (11, 9, 11), (19, 10, 10), (16, 15, 12)] {
        g.tint_rect(ramp, Tone::Light, x + 2, y + 1, w / 3, 2);
    }
    for (a, b) in [((11, 13), (13, 18)), ((19, 13), (18, 18)), ((8, 20), (14, 22))] {
        g.crease(ramp, Tone::Shade, a, b);
    }
    if g.big() {
        g.flecks(ramp, Tone::High, 8, 7, (5, 10, 20, 6));
        g.flecks(ramp, Tone::Mid, 10, 8, (5, 14, 22, 8));
        for (x, y) in [(11, 2), (18, 1), (23, 5)] {
            g.seg(tuft.at(Tone::Shade), (x, y + 2), (x + 1, y + 9), 1);
            g.blob(Ramp::ClothRed.at(Tone::Base), x - 1, y);
            g.px(Ramp::ClothRed.at(Tone::Light), x - 1, y);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Food

/// An apple: two shoulders and the dimple of its stalk, a streak or two, the gloss of its skin
/// with a window in it, a leaf.
fn apple(g: &mut G<'_>, skin: Ramp, green: Ramp) {
    g.dome(skin, Mat::Gloss, 9, |m| {
        m.ell(3, 8, 15, 20);
        m.ell(14, 8, 15, 20);
        m.ell(6, 14, 20, 16);
        m.cut_ell(13, 5, 6, 6);
    });
    // The stalk's dimple in shade, streaks down the cheek.
    g.tint_rect(skin, Tone::Shade, 13, 9, 6, 2);
    if g.big() {
        for (a, b) in [((9, 13), (8, 21)), ((22, 13), (23, 20)), ((12, 16), (12, 24))] {
            g.crease(skin, Tone::Mid, a, b);
        }
        g.rect(glint(), 8, 12, 2, 3);
        g.px(Ramp::HairWhite.at(Tone::Light), 10, 11);
        g.px(skin.at(Tone::High), 23, 24);
        for (x, y) in [(18, 17), (21, 22), (14, 25)] {
            g.px(skin.at(Tone::Light), x, y);
        }
    } else {
        g.rect(glint(), 7, 12, 2, 2);
    }
    g.seg(Ramp::Bark.at(Tone::Base), (16, 10), (18, 3), 2);
    leaf(g, green, (18, 5), (28, 3), 6);
}

/// A bunch of grapes: a cluster of glossy berries each with its glint, the stalk and a vine
/// leaf.
fn grapes(g: &mut G<'_>, skin: Ramp, green: Ramp) {
    leaf(g, green, (16, 5), (28, 2), 8);
    g.seg(Ramp::Bark.at(Tone::Base), (15, 9), (16, 2), 2);
    let berries: &[(i32, i32)] = &[(6, 8), (13, 8), (20, 8), (9, 14), (16, 14), (23, 13), (12, 20), (19, 20), (15, 25)];
    for &(x, y) in berries {
        g.over(skin, Mat::Gloss, 3, |m| m.ell(x, y, 8, 8));
    }
    for &(x, y) in berries {
        // At 16 a glint on every berry is a rash: every third.
        if g.big() || (x + y) % 3 == 0 {
            g.px(Ramp::HairWhite.at(Tone::Light), x + 2, y + 2);
        }
    }
    // A tendril.
    if g.big() {
        g.path(green.at(Tone::Light), &[(17, 6), (21, 7), (22, 10)]);
    }
}

/// An egg, broader below: a warm shade, the light's bounce under it, one soft glint.
fn egg(g: &mut G<'_>, shell: Ramp) {
    g.dome(shell, Mat::Soft, 9, |m| {
        m.ell(8, 9, 16, 20);
        m.ell(9, 3, 14, 18);
    });
    if g.big() {
        g.rect(Ramp::HairWhite.at(Tone::Light), 12, 8, 2, 3);
        g.px(glint(), 12, 8);
        for (x, y) in [(18, 13), (15, 21), (20, 22)] {
            g.px(shell.at(Tone::Mid), x, y);
        }
    } else {
        g.px(glint(), 12, 9);
    }
}

/// A loaf: a crusted dome scored three times, each cut's lip lit, flour dusted on top, and its
/// end cut to show the crumb.
fn loaf(g: &mut G<'_>, crust: Ramp) {
    g.dome(crust, Mat::Soft, 8, |m| {
        m.ell(2, 8, 28, 19);
        m.rect(3, 18, 26, 8);
    });
    // The top browner than the sides: the crust's colour, not its shade.
    g.step_where(crust, -1, |_, _| true);
    let big = g.big();
    for x in [8, 14, 20] {
        g.crease(crust, Tone::Deep, (x, 11), (x + 4, 18));
        g.crease(crust, Tone::High, (x - 1, 12), (x + 3, 19));
        if big {
            g.crease(crust, Tone::Light, (x - 1, 11), (x + 3, 18));
        }
    }
    // The bottom crust, darker and flat.
    g.tint_rect(crust, Tone::Shade, 4, 24, 24, 2);
    if big {
        g.flecks(crust, Tone::High, 7, 3, (6, 9, 18, 4));
        // The end cut: the crumb, pale, with holes.
        g.part(Ramp::ClothCream, Mat::Soft, Form::Face(60, 0), Z::new(2, 3), true, |m| m.ell(23, 12, 7, 13));
        g.px(Ramp::ClothCream.at(Tone::Mid), 26, 16);
        g.px(Ramp::ClothCream.at(Tone::Mid), 25, 20);
        g.px(Ramp::ClothCream.at(Tone::Mid), 27, 21);
    }
}

/// A crepe folded in four: a fan with a rounded edge, browned in lace, a second layer showing
/// at the fold, a spoon of jam on it.
fn crepe(g: &mut G<'_>, ramp: Ramp) {
    g.dome(ramp, Mat::Soft, 4, |m| {
        m.poly(&[(4, 27), (4, 6), (12, 5), (20, 8), (25, 13), (28, 20), (28, 27)]);
    });
    g.over(ramp, Mat::Soft, 3, |m| {
        m.poly(&[(4, 27), (6, 10), (14, 9), (21, 12), (25, 18), (26, 27)]);
    });
    g.flecks(ramp, Tone::Shade, 16, 11, (7, 11, 16, 14));
    g.flecks(ramp, Tone::Mid, 12, 12, (6, 9, 18, 16));
    // The fold's lit edge, the jam and its gloss.
    g.crease(ramp, Tone::High, (6, 26), (6, 11));
    g.crease(ramp, Tone::High, (15, 10), (21, 13));
    g.dome(Ramp::ClothRed, Mat::Gloss, 3, |m| {
        m.ell(10, 16, 10, 6);
        m.ell(15, 19, 6, 7);
    });
    g.px(Ramp::HairWhite.at(Tone::Light), 12, 17);
}

/// A joint of meat: the meat with its grain and a rind of fat, a bone through it with its knuckle.
fn meat(g: &mut G<'_>, ramp: Ramp) {
    g.part(Ramp::Bone, Mat::Soft, Form::Lying, Z::new(1, 3), false, |m| {
        m.line((18, 18), (27, 26), 5);
        m.ell(24, 21, 6, 6);
        m.ell(21, 25, 6, 6);
    });
    g.over(ramp, Mat::Gloss, 7, |m| {
        m.ell(3, 3, 20, 20);
        m.ell(7, 8, 16, 15);
    });
    // The fat's rind along the lit shoulder.
    g.over(Ramp::ClothCream, Mat::Soft, 2, |m| {
        m.ell(3, 3, 20, 20);
        m.cut_ell(6, 6, 20, 20);
    });
    if g.big() {
        for (a, b) in [((9, 11), (13, 13)), ((12, 16), (17, 18)), ((15, 9), (19, 11)), ((8, 17), (11, 19))] {
            g.crease(ramp, Tone::Shade, a, b);
        }
        g.px(glint(), 11, 9);
        g.px(Ramp::HairWhite.at(Tone::Light), 12, 9);
        g.crease(Ramp::Bone, Tone::High, (20, 19), (25, 24));
    } else {
        g.px(glint(), 11, 10);
    }
}

/// Three potatoes, lumpy, each with its eyes and its dirt.
fn potatoes(g: &mut G<'_>, skin: Ramp) {
    for (k, &(x, y, w)) in [(15, 7, 14), (3, 12, 15), (10, 17, 15)].iter().enumerate() {
        g.over(skin, Mat::Soft, 4, |m| {
            m.ell(x, y, w, 10);
            m.ell(x + 2, y + 2, w - 3, 9);
        });
        if g.big() {
            let (ex, ey) = (x + w / 2, y + 4);
            g.px(skin.at(Tone::Shade), ex, ey);
            g.px(skin.at(Tone::Light), ex, ey + 1);
            g.px(skin.at(Tone::Shade), x + 3, y + 6);
            g.flecks(skin, Tone::Mid, 3, k as u32 * 5 + 1, (x + 2, y + 3, w - 4, 5));
            g.px(skin.at(Tone::High), x + 3, y + 2);
        }
    }
}

/// A honey pot: a glazed jar, honey run over its rim and down its side, a dipper in it.
fn honey(g: &mut G<'_>, honey: Ramp, jar: Ramp) {
    g.seg(Ramp::WoodPale.at(Tone::Light), (20, 9), (27, 1), 2);
    g.dome(jar, Mat::Gloss, 7, |m| {
        m.ell(4, 10, 24, 20);
        m.rect(8, 7, 16, 6);
    });
    g.part(jar, Mat::Gloss, Form::Lying, Z::new(2, 4), true, |m| m.ell(6, 4, 20, 6));
    // The honey inside the rim and over it.
    g.glow = false;
    g.part(honey, Mat::Gloss, Form::Dome(3), Z::new(3, 4), false, |m| {
        m.ell(8, 5, 16, 4);
        m.poly(&[(8, 7), (13, 7), (13, 13), (11, 15), (9, 13)]);
        m.poly(&[(18, 7), (22, 7), (21, 17), (19, 17)]);
        m.ell(18, 15, 4, 4);
    });
    g.px(Ramp::HairWhite.at(Tone::Light), 10, 9);
    if g.big() {
        g.px(Ramp::HairWhite.at(Tone::Light), 19, 13);
        g.rect(glint(), 8, 15, 2, 3);
        // The glaze's band.
        g.crease(jar, Tone::Shade, (5, 20), (27, 20));
        g.crease(jar, Tone::Light, (5, 21), (27, 21));
    }
}

/// A tin: a can in the 3/4 view, its lid a rolled rim with a ring in it, a paper label round it,
/// a vertical sheen.
fn tin(g: &mut G<'_>, metal: Ramp) {
    g.part(metal, Mat::Metal, Form::Upright, Z::new(1, 3), false, |m| {
        m.rect(6, 8, 20, 18);
        m.ell(6, 22, 20, 7);
    });
    g.part(Ramp::ClothRed, Mat::Cloth, Form::Upright, Z::new(2, 3), false, |m| {
        m.rect(6, 12, 20, 10);
        m.ell(6, 18, 20, 7);
        m.cut_ell(6, 8, 20, 7);
    });
    g.part(metal, Mat::Metal, Form::Face(-10, -60), Z::new(3, 4), true, |m| m.ell(6, 4, 20, 8));
    g.crease(metal, Tone::Shade, (9, 8), (22, 8));
    if g.big() {
        let r = Rect::new(g.v(9), g.v(5), g.v(14), g.v(5));
        g.c.ellipse(r, metal.at(Tone::Mid), 1);
        let r = Rect::new(g.v(10), g.v(6), g.v(12), g.v(3));
        g.c.ellipse(r, metal.at(Tone::Light), 1);
        g.seg(Ramp::ClothLinen.at(Tone::Light), (9, 16), (22, 16), 1);
        g.seg(Ramp::ClothLinen.at(Tone::Light), (9, 17), (22, 17), 1);
        g.seg(glint(), (10, 23), (10, 26), 1);
        g.px(glint(), 8, 5);
    } else {
        g.seg(Ramp::ClothLinen.at(Tone::Light), (8, 16), (23, 16), 1);
    }
}

/// An oil can: a squat cone of copper, a long spout, a handle in iron, a drop at the spout.
fn oil_can(g: &mut G<'_>, copper: Ramp, iron: Ramp) {
    g.part(iron, Mat::Metal, Form::Dome(2), Z::new(1, 2), false, |m| {
        m.ell(1, 12, 12, 14);
        m.cut_ell(4, 15, 6, 8);
    });
    g.part(copper, Mat::Metal, Form::Upright, Z::new(2, 3), true, |m| {
        m.poly(&[(6, 28), (26, 28), (22, 15), (10, 15)]);
        m.ell(10, 10, 12, 9);
    });
    g.part(copper, Mat::Metal, Form::Dome(2), Z::new(3, 4), true, |m| m.line((20, 16), (30, 4), 3));
    g.part(iron, Mat::Metal, Form::Upright, Z::new(3, 4), true, |m| m.rect(14, 6, 4, 4));
    g.crease(copper, Tone::Shade, (7, 25), (25, 25));
    if g.big() {
        g.crease(copper, Tone::Glint, (11, 16), (9, 26));
        g.crease(copper, Tone::High, (12, 16), (10, 26));
        g.px(Ramp::ClothMustard.at(Tone::Light), 30, 5);
        g.px(glint(), 13, 12);
    } else {
        g.crease(copper, Tone::High, (11, 16), (9, 26));
    }
}

// ---------------------------------------------------------------------------------------------
// Things to wear

/// A leather glove, back of the hand: four fingers and a thumb, the seams between them, three
/// lines of stitching, a cuff in its own cloth.
fn glove(g: &mut G<'_>, leather: Ramp, cuff: Ramp) {
    g.dome(leather, Mat::Soft, 4, |m| {
        m.poly(&[(9, 12), (24, 12), (23, 24), (10, 24)]);
        m.ell(8, 4, 5, 13);
        m.ell(12, 2, 5, 14);
        m.ell(16, 3, 5, 13);
        m.ell(20, 6, 5, 11);
        m.line((10, 17), (4, 12), 5);
    });
    // The fingers parted, the knuckles lit.
    for x in [12, 16, 20] {
        g.crease(leather, Tone::Deep, (x, 6), (x, 13));
    }
    g.crease(leather, Tone::Shade, (9, 17), (11, 20));
    if g.big() {
        for x in [10, 14, 18, 22] {
            g.tint(leather, Tone::High, x, 13);
        }
        for x in [13, 16, 19] {
            g.crease(leather, Tone::Mid, (x, 15), (x, 21));
        }
    }
    g.part(cuff, Mat::Cloth, Form::Upright, Z::new(2, 3), true, |m| m.poly(&[(9, 23), (24, 23), (25, 30), (8, 30)]));
    for x in [11, 14, 17, 20, 23] {
        g.crease(cuff, Tone::Shade, (x, 24), (x, 29));
    }
}

/// A coat laid out: its shoulders, the sleeves hanging, lapels open on a V, a row of buttons,
/// pocket flaps, and the cloth's folds.
fn coat(g: &mut G<'_>, cloth: Ramp) {
    // The sleeves behind, then the body.
    g.dome(cloth, Mat::Cloth, 3, |m| {
        m.poly(&[(9, 5), (4, 9), (1, 26), (6, 27), (9, 14)]);
        m.poly(&[(23, 5), (28, 9), (31, 26), (26, 27), (23, 14)]);
    });
    g.step_where(cloth, -1, |_, _| true);
    g.over(cloth, Mat::Cloth, 4, |m| {
        m.poly(&[(10, 4), (22, 4), (25, 8), (25, 30), (7, 30), (7, 8)]);
    });
    // The lapels, the V of the lining, the opening down the front.
    g.dome(Ramp::ClothBlack, Mat::Cloth, 2, |m| m.poly(&[(13, 4), (19, 4), (16, 12)]));
    g.over(cloth, Mat::Cloth, 2, |m| m.poly(&[(10, 4), (13, 4), (16, 13), (12, 12)]));
    g.over(cloth, Mat::Cloth, 2, |m| m.poly(&[(19, 4), (22, 4), (20, 12), (16, 13)]));
    g.crease(cloth, Tone::Deep, (16, 13), (16, 29));
    g.crease(cloth, Tone::Light, (15, 13), (15, 29));
    for y in [15, 20, 25] {
        g.blob(Ramp::Brass.at(Tone::Light), 17, y);
    }
    if g.big() {
        g.seg(cloth.at(Tone::Shade), (9, 21), (13, 21), 1);
        g.seg(cloth.at(Tone::Shade), (19, 21), (23, 21), 1);
        g.crease(cloth, Tone::Shade, (10, 10), (9, 18));
        g.crease(cloth, Tone::Lift, (11, 10), (10, 18));
        g.crease(cloth, Tone::Shade, (22, 26), (24, 29));
        g.px(Ramp::Brass.at(Tone::High), 17, 15);
    }
}

/// A knitted scarf as it is worn: the wrap round a neck that is not there, knotted at the front,
/// its two ends hanging from the knot with bands of the second colour and a fringe.
fn scarf(g: &mut G<'_>, wool: Ramp, stripe: Ramp) {
    // The ends: the far one, then the near one over it.
    g.dome(wool, Mat::Cloth, 3, |m| m.line((18, 16), (22, 27), 6));
    g.step_where(wool, -1, |_, _| true);
    g.over(wool, Mat::Cloth, 3, |m| m.line((14, 16), (10, 28), 7));
    // The wrap: a band round in a U, and the knot at its foot.
    g.over(wool, Mat::Cloth, 3, |m| {
        let path = [(4, 3), (7, 10), (16, 14), (25, 10), (28, 3)];
        for w in path.windows(2) {
            m.line(w[0], w[1], 7);
        }
        for &(x, y) in &path[1..4] {
            m.ell(x - 3, y - 3, 7, 7);
        }
    });
    g.over(wool, Mat::Cloth, 3, |m| m.ell(12, 10, 9, 8));
    // Bands across the ends near their foot.
    let s = g.s;
    let mut mask = Canvas::new(s, s);
    {
        let mut m = M { m: &mut mask, s };
        m.line((5, 21), (16, 23), 2);
        m.line((5, 24), (16, 26), 2);
        m.line((17, 20), (26, 19), 2);
        m.line((17, 23), (26, 22), 2);
    }
    g.c.dye(&mask, wool, stripe);
    // The fringes: tassels past each end.
    let big = g.big();
    for k in 0..4 {
        let x = 7 + 2 * k;
        g.seg(wool.at(Tone::Light), (x, 29), (x - 1, 31), 1);
        if k < 3 {
            let x2 = 19 + 2 * k;
            g.seg(wool.at(Tone::Shade), (x2, 28), (x2 + 1, 30), 1);
        }
    }
    // The wrap's rib and its lit top edge, the knot's folds.
    if big {
        g.crease(wool, Tone::Mid, (6, 7), (15, 11));
        g.crease(wool, Tone::Mid, (18, 11), (26, 7));
        g.crease(wool, Tone::Shade, (14, 13), (18, 16));
        g.crease(wool, Tone::High, (5, 2), (7, 7));
    }
}

/// Folded washing: two cloths folded and stacked, the top one striped, a wooden peg on them.
fn linen(g: &mut G<'_>, cloth: Ramp, stripe: Ramp) {
    g.dome(stripe, Mat::Cloth, 3, |m| m.poly(&[(4, 17), (28, 17), (29, 28), (3, 28)]));
    g.crease(stripe, Tone::Shade, (4, 22), (28, 22));
    g.over(cloth, Mat::Cloth, 3, |m| m.poly(&[(5, 7), (27, 7), (28, 19), (4, 19)]));
    // The fold's lip, the stripes of the top cloth, a crease.
    g.crease(cloth, Tone::High, (5, 8), (26, 8));
    for y in [11, 15] {
        g.seg(stripe.at(Tone::Base), (6, y), (26, y), 1);
    }
    g.crease(cloth, Tone::Shade, (16, 9), (16, 18));
    g.part(Ramp::WoodPale, Mat::Soft, Form::Upright, Z::new(3, 4), true, |m| {
        m.rect(19, 2, 5, 12);
    });
    g.crease(Ramp::WoodPale, Tone::Deep, (21, 4), (21, 12));
    g.seg(Ramp::Iron.at(Tone::Light), (19, 9), (23, 9), 1);
}

/// A bowler: a hard felt crown with its sheen, a band and bow, a curled brim.
fn hat(g: &mut G<'_>, felt: Ramp, band: Ramp) {
    g.part(felt, Mat::Soft, Form::Lying, Z::new(1, 2), false, |m| m.ell(1, 17, 30, 11));
    g.over(felt, Mat::Soft, 7, |m| {
        m.ell(7, 4, 18, 20);
        m.cut_rect(0, 21, 32, 11);
    });
    g.over(band, Mat::Cloth, 1, |m| m.poly(&[(7, 16), (25, 16), (25, 20), (7, 20)]));
    g.dome(band, Mat::Cloth, 2, |m| m.ell(19, 15, 6, 6));
    // The brim's curled edge lit, the felt's sheen.
    g.crease(felt, Tone::Light, (3, 22), (10, 26));
    g.crease(felt, Tone::High, (11, 7), (9, 13));
    if g.big() {
        g.crease(felt, Tone::High, (12, 6), (10, 12));
        g.crease(felt, Tone::Light, (22, 26), (29, 22));
        g.px(glint(), 12, 7);
    }
}

/// A flat cap in tweed: its crown sloping forward over a stiff peak with its shadow under it, a
/// button on top and the panels' seams, flecks of the weave.
fn cap(g: &mut G<'_>, tweed: Ramp) {
    g.part(tweed, Mat::Cloth, Form::Face(20, 90), Z::new(1, 2), false, |m| {
        m.poly(&[(13, 18), (26, 15), (31, 19), (29, 23), (15, 23)]);
    });
    g.step_where(tweed, -1, |_, _| true);
    g.over(tweed, Mat::Cloth, 7, |m| {
        m.poly(&[(3, 20), (4, 13), (9, 8), (18, 7), (25, 10), (28, 16), (24, 19), (12, 20)]);
    });
    // The band round the crown's foot, the peak's lit edge.
    g.crease(tweed, Tone::Shade, (4, 19), (23, 18));
    g.crease(tweed, Tone::Light, (27, 20), (16, 22));
    // The panels' seams from the button.
    g.crease(tweed, Tone::Shade, (15, 9), (9, 17));
    g.crease(tweed, Tone::Shade, (16, 9), (22, 15));
    if g.big() {
        g.flecks(tweed, Tone::Light, 9, 21, (5, 10, 20, 7));
        g.flecks(tweed, Tone::Mid, 7, 22, (6, 12, 18, 6));
        g.crease(tweed, Tone::High, (6, 13), (10, 9));
    }
    g.over(tweed, Mat::Cloth, 1, |m| m.ell(13, 5, 5, 4));
}

/// A fleece rolled and tied: a lumpy roll of wool, its curls in rows (each a crescent of shade
/// under a lit top), a band of twine round its middle.
fn fleece(g: &mut G<'_>, wool: Ramp) {
    g.part(wool, Mat::Soft, Form::Lying, Z::new(1, 3), false, |m| {
        m.ell(2, 10, 28, 18);
        for x in [3, 9, 15, 21] {
            m.ell(x, 7, 9, 8);
        }
    });
    let big = g.big();
    for (row, y) in [(0, 11), (1, 16), (2, 21)] {
        let mut x = 5 + (row % 2) * 3;
        while x < 26 {
            if big {
                g.path(wool.at(Tone::Mid), &[(x, y + 2), (x + 2, y + 3), (x + 4, y + 2)]);
                g.tint(wool, Tone::High, x + 1, y);
                g.tint(wool, Tone::High, x + 2, y);
            } else if (x / 6 + row) % 2 == 0 {
                g.px(wool.at(Tone::Shade), x + 2, y + 3);
            }
            x += 6;
        }
    }
    g.over(Ramp::Reed, Mat::Soft, 1, |m| m.poly(&[(14, 7), (17, 7), (18, 28), (15, 28)]));
    g.crease(Ramp::Reed, Tone::Shade, (15, 9), (16, 27));
}

// ---------------------------------------------------------------------------------------------
// Paper, parcels and bags

/// A letter: an envelope with its flap down and its shadow, a red wax seal pressed with a mark,
/// a corner curled.
fn letter(g: &mut G<'_>, paper: Ramp) {
    g.part(paper, Mat::Paper, Form::Face(-10, -20), Z::new(1, 2), false, |m| {
        m.poly(&[(2, 8), (30, 8), (30, 25), (2, 25)]);
    });
    // The back's folds, the flap over them with its shadow.
    g.crease(paper, Tone::Mid, (2, 25), (13, 16));
    g.crease(paper, Tone::Mid, (30, 25), (19, 16));
    g.part(paper, Mat::Paper, Form::Face(-20, -40), Z::new(2, 3), true, |m| m.poly(&[(2, 8), (30, 8), (16, 19)]));
    g.crease(paper, Tone::Shade, (4, 10), (16, 20));
    g.crease(paper, Tone::Shade, (28, 10), (16, 20));
    // The seal.
    g.dome(Ramp::ClothRed, Mat::Gloss, 3, |m| {
        m.ell(12, 14, 8, 8);
        m.ell(11, 16, 3, 3);
        m.ell(18, 17, 3, 3);
    });
    if g.big() {
        g.px(Ramp::ClothRed.at(Tone::Shade), 15, 17);
        g.px(Ramp::ClothRed.at(Tone::Shade), 16, 18);
        g.px(Ramp::ClothRed.at(Tone::High), 14, 15);
        // The corner curled up, its underside in shade.
        g.part(paper, Mat::Paper, Form::Face(-60, -60), Z::new(3, 3), true, |m| {
            m.poly(&[(25, 25), (30, 20), (30, 25)]);
        });
        g.crease(paper, Tone::High, (4, 9), (13, 15));
    }
}

/// A present: a box in the 3/4 view, a ribbon round it both ways, a bow on top.
fn parcel(g: &mut G<'_>, paper: Ramp, ribbon: Ramp) {
    g.part(paper, Mat::Cloth, Form::Face(0, 50), Z::new(1, 2), false, |m| {
        m.poly(&[(4, 13), (24, 13), (24, 29), (4, 29)]);
    });
    g.part(paper, Mat::Cloth, Form::Face(80, 20), Z::new(1, 2), false, |m| {
        m.poly(&[(24, 13), (29, 9), (29, 25), (24, 29)]);
    });
    g.part(paper, Mat::Cloth, Form::Face(-30, -60), Z::new(2, 3), true, |m| {
        m.poly(&[(4, 13), (9, 9), (29, 9), (24, 13)]);
    });
    // The ribbon: down the front and the side, across the top.
    g.rect(ribbon.at(Tone::Base), 12, 13, 3, 16);
    g.seg(ribbon.at(Tone::Light), (12, 13), (12, 28), 1);
    g.seg(ribbon.at(Tone::Shade), (26, 11), (26, 27), 1);
    g.seg(ribbon.at(Tone::Base), (27, 10), (27, 26), 1);
    g.seg(ribbon.at(Tone::Light), (13, 12), (18, 10), 1);
    g.seg(ribbon.at(Tone::Light), (7, 11), (26, 11), 1);
    // The bow: two loops and a knot.
    g.over(ribbon, Mat::Gloss, 2, |m| {
        m.ell(7, 3, 9, 7);
        m.ell(16, 3, 9, 7);
    });
    g.px(paper.at(Tone::Shade), 11, 6);
    g.px(paper.at(Tone::Shade), 20, 6);
    g.dome(ribbon, Mat::Gloss, 1, |m| m.ell(13, 6, 5, 5));
    if g.big() {
        g.seg(ribbon.at(Tone::Base), (13, 10), (10, 13), 1);
        g.seg(ribbon.at(Tone::Base), (17, 10), (19, 13), 1);
        g.seg(paper.at(Tone::High), (5, 13), (23, 13), 1);
    }
}

/// A sack of sacking: a lumpy body, its neck gathered and tied with cord, the mouth's ruffle,
/// creases down from the tie, the weave.
fn sack(g: &mut G<'_>, cloth: Ramp, cord: Ramp) {
    g.dome(cloth, Mat::Cloth, 7, |m| {
        m.ell(4, 11, 24, 20);
        m.poly(&[(12, 12), (20, 12), (26, 22), (6, 22)]);
    });
    g.over(cloth, Mat::Cloth, 3, |m| {
        m.poly(&[(13, 6), (19, 6), (18, 12), (14, 12)]);
        m.ell(9, 1, 14, 7);
    });
    g.crease(cloth, Tone::Shade, (11, 5), (21, 5));
    for (a, b) in [((14, 13), (9, 24)), ((18, 13), (22, 25)), ((16, 14), (16, 22))] {
        g.crease(cloth, Tone::Shade, a, b);
    }
    if g.big() {
        g.crease(cloth, Tone::Light, (13, 13), (8, 24));
        g.flecks(cloth, Tone::Mid, 14, 31, (6, 15, 20, 12));
        g.tint(cloth, Tone::Light, 10, 2);
        g.tint(cloth, Tone::Light, 13, 2);
        g.tint(cloth, Tone::Light, 17, 2);
    }
    g.over(cord, Mat::Soft, 1, |m| m.rect(12, 10, 9, 3));
    g.seg(cord.at(Tone::Base), (20, 12), (23, 17), 2);
}

/// A satchel: a canvas bag, its flap over the top with a stitched edge, a leather strap and
/// brass buckle, the shoulder strap arching over.
fn satchel(g: &mut G<'_>, cloth: Ramp, leather: Ramp) {
    g.part(leather, Mat::Soft, Form::Dome(2), Z::new(1, 2), false, |m| {
        m.line((5, 14), (8, 5), 3);
        m.line((8, 5), (16, 2), 3);
        m.line((16, 2), (24, 5), 3);
        m.line((24, 5), (27, 14), 3);
    });
    g.over(cloth, Mat::Cloth, 5, |m| {
        m.poly(&[(3, 13), (29, 13), (28, 29), (4, 29)]);
    });
    g.over(cloth, Mat::Cloth, 3, |m| {
        m.poly(&[(3, 12), (29, 12), (28, 21), (16, 24), (4, 21)]);
    });
    g.crease(cloth, Tone::High, (4, 13), (27, 13));
    if g.big() {
        for x in (5..27).step_by(3) {
            let y = if x < 16 { 21 + (x - 4) / 5 } else { 23 - (x - 16) / 5 };
            g.tint(cloth, Tone::Light, x, y);
        }
        g.crease(cloth, Tone::Shade, (8, 25), (9, 28));
        g.crease(cloth, Tone::Shade, (23, 25), (22, 28));
    }
    g.over(leather, Mat::Soft, 1, |m| m.rect(14, 17, 4, 10));
    g.over(Ramp::Brass, Mat::Metal, 1, |m| {
        m.rect(13, 19, 6, 5);
        m.cut_rect(15, 21, 2, 1);
    });
    g.px(glint(), 13, 19);
}

// ---------------------------------------------------------------------------------------------
// Tools

/// A billhook: an ash handle with a ferrule and rivets, a broad blade swelling from it and
/// hooked at its tip, its cutting edge bright along the inside of the hook.
fn billhook(g: &mut G<'_>, metal: Ramp, wood: Ramp) {
    g.part(wood, Mat::Soft, Form::Dome(3), Z::new(1, 3), false, |m| m.line((5, 29), (13, 19), 6));
    g.crease(wood, Tone::Shade, (6, 27), (12, 20));
    g.over(metal, Mat::Metal, 3, |m| {
        m.poly(&[
            (11, 20),
            (13, 14),
            (17, 8),
            (22, 4),
            (27, 2),
            (30, 4),
            (30, 9),
            (27, 8),
            (25, 10),
            (23, 16),
            (19, 22),
            (14, 24),
        ]);
    });
    g.part(Ramp::Iron, Mat::Metal, Form::Lying, Z::new(3, 4), true, |m| m.line((11, 23), (15, 19), 6));
    // The edge along the inside of the blade and hook, polished; the back in shade.
    g.crease(metal, Tone::Glint, (17, 21), (22, 15));
    g.crease(metal, Tone::High, (22, 15), (25, 9));
    g.crease(metal, Tone::Deep, (13, 13), (21, 5));
    if g.big() {
        g.crease(metal, Tone::High, (18, 21), (23, 15));
        g.crease(metal, Tone::Shade, (16, 12), (20, 8));
        g.px(Ramp::Iron.at(Tone::High), 8, 25);
        g.px(Ramp::Iron.at(Tone::High), 10, 23);
        g.px(glint(), 28, 3);
    }
}

/// Scissors: two blades crossed at a brass screw, their handles two rings.
fn scissors(g: &mut G<'_>, metal: Ramp, rings: Ramp) {
    let big = g.big();
    g.part(rings, Mat::Metal, Form::Dome(2), Z::new(1, 3), false, |m| {
        m.ell(2, 18, 13, 13);
        m.ell(17, 18, 13, 13);
        m.cut_ell(6, 22, 5, 5);
        m.cut_ell(21, 22, 5, 5);
        m.line((11, 21), (15, 16), 4);
        m.line((21, 21), (17, 16), 4);
    });
    if big {
        g.over(metal, Mat::Metal, 2, |m| m.poly(&[(13, 18), (17, 14), (26, 2), (28, 3), (20, 17)]));
        g.over(metal, Mat::Metal, 2, |m| m.poly(&[(19, 18), (15, 14), (6, 2), (4, 3), (12, 17)]));
    } else {
        // At 16 a blade is a two-px stroke, or the clean-up takes it.
        g.over(metal, Mat::Metal, 1, |m| m.line((14, 17), (26, 3), 4));
        g.over(metal, Mat::Metal, 1, |m| m.line((18, 17), (6, 3), 4));
    }
    g.crease(metal, Tone::Glint, (7, 4), (13, 12));
    g.crease(metal, Tone::High, (25, 4), (19, 12));
    g.dome(Ramp::Brass, Mat::Metal, 1, |m| m.ell(14, 13, 4, 4));
    if g.big() {
        g.px(glint(), 5, 21);
        g.px(glint(), 21, 21);
    }
}

/// A spanner: an open jaw at one end, a ring at the other, the shank between.
fn spanner(g: &mut G<'_>, metal: Ramp) {
    g.dome(metal, Mat::Metal, 3, |m| {
        m.line((8, 24), (22, 10), 6);
        m.ell(17, 1, 14, 14);
        m.ell(0, 17, 14, 14);
        m.cut_poly(&[(22, 5), (26, 1), (32, 7), (28, 11)]);
        m.cut_ell(22, 3, 6, 6);
        m.cut_ell(4, 21, 6, 6);
    });
    g.crease(metal, Tone::High, (9, 21), (19, 11));
    g.crease(metal, Tone::Shade, (11, 24), (22, 13));
    if g.big() {
        g.crease(metal, Tone::Glint, (11, 19), (15, 15));
        g.px(glint(), 19, 4);
        g.px(glint(), 2, 20);
    }
}

/// Two spoons crossed, their bowls hollow: the light falls inside a hollow on its far side.
fn spoons(g: &mut G<'_>, metal: Ramp) {
    for (bx, a, b) in [(3, (8, 11), (27, 29)), (19, (24, 11), (5, 29))] {
        g.over(metal, Mat::Metal, 2, |m| {
            m.line(a, b, 3);
            m.ell(bx, 1, 10, 13);
        });
        // The hollow of the bowl: dark under the lit rim, bright at the far lip.
        let (x0, x1) = (g.v(bx + 2), g.v(bx + 8));
        let (y0, y1) = (g.v(3), g.v(12));
        g.step_where(metal, -2, |x, y| x >= x0 && x < x1 && y >= y0 && y < y1 - 1);
        g.tint_rect(metal, Tone::High, bx + 3, 10, 4, 1);
        g.tint(metal, Tone::Glint, bx + 6, 11);
    }
}

/// A butterfly net: its cane, the wire hoop, the gauze bag hanging from it in a mesh.
fn net(g: &mut G<'_>, gauze: Ramp, cane: Ramp) {
    g.part(cane, Mat::Soft, Form::Dome(2), Z::new(1, 2), false, |m| m.line((2, 30), (15, 13), 3));
    g.part(gauze, Mat::Cloth, Form::Dome(4), Z::new(1, 2), false, |m| {
        m.poly(&[(12, 9), (28, 5), (29, 14), (26, 27), (21, 30), (17, 24)]);
    });
    // The mesh: the slot seen through the gauze in square holes.
    for y in 0..g.s {
        for x in 0..g.s {
            let (u, v) = (x * 32 / g.s, y * 32 / g.s);
            if u % 3 != 0 && v % 3 != 0 {
                g.c.tint(x, y, gauze, Tone::Shade);
            }
        }
    }
    g.part(Ramp::Iron, Mat::Metal, Form::Dome(1), Z::new(2, 3), false, |m| {
        m.ell(10, 2, 21, 13);
        m.cut_ell(13, 4, 15, 9);
    });
    g.px(glint(), 13, 5);
}

/// A dinner plate in the 3/4 view: a rim with a painted ring, the well, a glint on the glaze.
fn plate(g: &mut G<'_>, china: Ramp) {
    g.part(china, Mat::Gloss, Form::Lying, Z::new(1, 2), false, |m| m.ell(2, 7, 28, 19));
    g.part(china, Mat::Paper, Form::Face(20, 30), Z::new(1, 2), false, |m| m.ell(8, 11, 16, 11));
    let r = Rect::new(g.v(5), g.v(9), g.v(22), g.v(15));
    let ring = Ramp::ClothSky.at(Tone::Base);
    let mut l = Canvas::new(g.s, g.s);
    l.ellipse(r, ring, 1);
    let inner = Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2);
    l.ellipse(inner, Ix::CLEAR, 0);
    for y in 0..g.s {
        for x in 0..g.s {
            if l.get(x, y).is_opaque() && g.c.get(x, y).is_opaque() {
                g.c.dot(x, y, ring, 1);
            }
        }
    }
    g.crease(china, Tone::Shade, (10, 12), (21, 12));
    if g.big() {
        g.seg(glint(), (6, 10), (9, 9), 1);
        g.px(china.at(Tone::High), 22, 22);
    }
}

// ---------------------------------------------------------------------------------------------
// Jewellery

/// A ring in the 3/4 view: a band broader at the front, a claw setting on top with a cut stone.
fn ring(g: &mut G<'_>, metal: Ramp, stone: Ramp) {
    g.dome(metal, Mat::Metal, 3, |m| {
        m.ell(5, 11, 22, 18);
        m.cut_ell(9, 13, 14, 11);
    });
    g.crease(metal, Tone::Glint, (8, 16), (9, 22));
    g.crease(metal, Tone::High, (9, 14), (13, 12));
    g.part(metal, Mat::Metal, Form::Dome(2), Z::new(2, 3), true, |m| m.poly(&[(11, 12), (21, 12), (19, 7), (13, 7)]));
    g.part(stone, Mat::Gloss, Form::Dome(3), Z::new(3, 4), true, |m| m.ell(11, 2, 10, 8));
    g.px(glint(), 13, 4);
    if g.big() {
        g.px(Ramp::HairWhite.at(Tone::Light), 14, 4);
        g.px(stone.at(Tone::High), 18, 7);
        g.px(metal.at(Tone::Glint), 11, 8);
        g.px(metal.at(Tone::Glint), 20, 8);
    }
}

/// An amulet: a chain of links in a V, a bail, and an oval medallion with a stone in its heart.
fn amulet(g: &mut G<'_>, metal: Ramp, stone: Ramp) {
    for k in 0..6 {
        let t = k * 2;
        for (x, y) in [(3 + t, 1 + t), (28 - t, 1 + t)] {
            let tone = if k % 2 == 0 { Tone::Light } else { Tone::Shade };
            g.blob(metal.at(tone), x, y);
        }
    }
    g.dome(metal, Mat::Metal, 3, |m| m.ell(8, 11, 16, 19));
    g.dome(metal, Mat::Metal, 1, |m| {
        m.ell(14, 9, 4, 4);
    });
    g.crease(metal, Tone::Deep, (11, 15), (11, 25));
    g.crease(metal, Tone::Deep, (20, 15), (20, 25));
    g.part(stone, Mat::Gloss, Form::Dome(3), Z::new(3, 4), true, |m| m.ell(12, 15, 8, 10));
    g.px(glint(), 14, 17);
    if g.big() {
        g.px(Ramp::HairWhite.at(Tone::Light), 15, 17);
        g.px(stone.at(Tone::High), 18, 23);
        g.crease(metal, Tone::Glint, (10, 14), (13, 12));
    }
}

/// Spectacles: two round lenses in iron rims, the bridge between, the arms folded back, a
/// streak of light across each lens.
fn spectacles(g: &mut G<'_>, metal: Ramp) {
    g.dome(Ramp::Glass, Mat::Gloss, 5, |m| {
        m.ell(3, 10, 12, 12);
        m.ell(17, 10, 12, 12);
    });
    g.step_where(Ramp::Glass, -1, |_, _| true);
    g.dome(metal, Mat::Metal, 1, |m| {
        m.ell(2, 9, 14, 14);
        m.cut_ell(4, 11, 10, 10);
        m.ell(16, 9, 14, 14);
        m.cut_ell(18, 11, 10, 10);
        m.line((14, 13), (18, 13), 2);
        m.line((2, 13), (0, 8), 2);
        m.line((30, 13), (31, 8), 2);
    });
    for x in [5, 19] {
        g.seg(glint(), (x + 1, 17), (x + 5, 13), 1);
        g.seg(Ramp::Glass.at(Tone::High), (x + 3, 18), (x + 6, 15), 1);
    }
}

// ---------------------------------------------------------------------------------------------
// The rest

/// Firewood: three split logs stacked, their bark in grooves, their cut ends showing rings.
fn logs(g: &mut G<'_>, bark: Ramp, wood: Ramp) {
    for &(x, y) in &[(8, 16), (17, 16), (12, 6)] {
        g.part(bark, Mat::Soft, Form::Lying, Z::new(1, 3), true, |m| m.rect(x - 2, y, 14, 10));
        g.crease(bark, Tone::Shade, (x + 1, y + 3), (x + 10, y + 3));
        g.crease(bark, Tone::Shade, (x + 2, y + 7), (x + 11, y + 7));
        g.part(wood, Mat::Soft, Form::Face(-60, -20), Z::new(3, 4), true, |m| m.ell(x - 5, y, 9, 10));
        let r = Rect::new(g.v(x - 3), g.v(y + 2), g.v(5).max(2), g.v(6).max(2));
        if g.big() {
            g.c.ellipse(r, wood.at(Tone::Mid), 1);
            let r2 = Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2);
            g.c.ellipse(r2, wood.at(Tone::Light), 1);
            g.px(wood.at(Tone::Shade), x - 1, y + 4);
        } else {
            g.px(wood.at(Tone::Shade), x - 1, y + 4);
        }
    }
}

/// Deadwood: a bundle of four grey sticks picked up off the ground, weathered silver along
/// their tops, a twig off two of them, their snapped ends pale and splintered, tied round the
/// middle with a twist of grass. Thin and crooked where `logs` is split and stacked.
fn sticks(g: &mut G<'_>, bark: Ramp, wood: Ramp) {
    // Each stick crooked: two lengths at a bend.
    let all: [[(i32, i32); 3]; 3] =
        [[(2, 22), (14, 13), (28, 4)], [(3, 29), (17, 19), (30, 11)], [(9, 31), (20, 25), (31, 20)]];
    // At 16 two sticks, or the chip is a scribble.
    let n = if g.big() { 3 } else { 2 };
    for (i, p) in all.iter().enumerate().take(n) {
        let w = if i == 2 { 4 } else { 5 };
        g.part(bark, Mat::Stone, Form::Dome(2), Z::new(1, 3), true, |m| {
            m.line(p[0], p[1], w);
            m.line(p[1], p[2], w);
        });
        // Silvered along its top, where the weather had it.
        g.crease(bark, Tone::High, (p[0].0 + 2, p[0].1 - 2), (p[1].0 - 1, p[1].1 - 2));
        if g.big() {
            g.crease(bark, Tone::Deep, (p[1].0 + 2, p[1].1 + 1), (p[1].0 + 4, p[1].1));
        }
        // The snapped end: pale wood, a splinter standing off it.
        g.blob(wood.at(Tone::Light), p[2].0 - 2, p[2].1 - 1);
        if g.big() {
            g.px(wood.at(Tone::High), p[2].0, p[2].1 - 2);
        }
    }
    // Twigs forking off the top stick and the middle one.
    if g.big() {
        g.part(bark, Mat::Stone, Form::Dome(1), Z::new(2, 3), true, |m| {
            m.line((11, 14), (8, 7), 2);
            m.line((22, 18), (27, 22), 2);
        });
    }
    // Weathered: a step greyer and darker than fresh bark, silver only along the tops.
    g.step_where(bark, -1, |_, _| true);
    // The twist of grass round the middle, across the grain, in two turns.
    g.part(Ramp::Reed, Mat::Cloth, Form::Dome(1), Z::new(3, 4), true, |m| m.line((12, 10), (19, 27), 4));
    g.crease(Ramp::Reed, Tone::Shade, (12, 15), (18, 24));
    g.crease(Ramp::Reed, Tone::High, (12, 11), (17, 22));
}

/// A box of matches: the sleeve with its label, the striker down its front, the tray slid out
/// at one end with a row of red heads in it, and one match lying in front.
fn matchbox(g: &mut G<'_>, paper: Ramp, wood: Ramp) {
    let red = Ramp::ClothRed;
    // The tray out of the sleeve's right end: its floor, the matches in it, its front.
    g.part(wood, Mat::Soft, Form::Face(-20, -70), Z::new(1, 2), false, |m| {
        m.poly(&[(17, 15), (23, 9), (30, 9), (24, 15)]);
    });
    for i in 0..3 {
        let (x, y) = (23 + i * 2, 13 - i * 2);
        g.seg(wood.at(Tone::High), (x - 4, y), (x, y), 1);
        g.blob(red.at(Tone::Base), x, y - 1);
        g.px(red.at(Tone::Light), x, y - 1);
    }
    g.part(wood, Mat::Soft, Form::Face(0, 60), Z::new(1, 2), true, |m| {
        m.poly(&[(17, 15), (24, 15), (24, 19), (17, 19)]);
    });
    // The sleeve: its top with the label, its front with the striker.
    g.part(paper, Mat::Cloth, Form::Face(-30, -60), Z::new(2, 3), true, |m| {
        m.poly(&[(2, 15), (8, 9), (23, 9), (17, 15)]);
    });
    g.part(paper, Mat::Cloth, Form::Face(0, 50), Z::new(2, 3), false, |m| {
        m.poly(&[(2, 15), (17, 15), (17, 23), (2, 23)]);
    });
    g.part(paper, Mat::Cloth, Form::Face(80, 20), Z::new(2, 3), false, |m| {
        m.poly(&[(17, 15), (18, 14), (18, 22), (17, 23)]);
    });
    g.rect(Ramp::ClothBrown.at(Tone::Shade), 3, 18, 14, 3);
    g.flecks(Ramp::ClothBrown, Tone::Base, 6, 0x6d61_7463, (3, 18, 14, 3));
    // The label: a red lozenge with a flame in it.
    g.over(red, Mat::Cloth, 1, |m| m.poly(&[(6, 12), (11, 10), (19, 10), (14, 14), (6, 14)]));
    if g.big() {
        g.seg(paper.at(Tone::High), (3, 15), (16, 15), 1);
        g.px(Ramp::ClothOchre.at(Tone::High), 12, 11);
        g.px(Ramp::ClothOchre.at(Tone::Light), 12, 12);
    }
    // A match lying in front of it.
    g.part(wood, Mat::Soft, Form::Lying, Z::new(1, 2), true, |m| m.line((7, 29), (23, 25), 2));
    g.part(red, Mat::Gloss, Form::Dome(2), Z::new(2, 3), true, |m| m.ell(21, 22, 5, 5));
}

/// A butterfly, wings open: pointed forewings and round hindwings, dark margins with pale
/// spots, an eyespot, a furred body and clubbed feelers.
fn butterfly(g: &mut G<'_>, wing: Ramp) {
    for side in [-1, 1] {
        let x = |u: i32| 16 + side * u;
        g.part(wing, Mat::Cloth, Form::Dome(4), Z::new(1, 2), false, |m| {
            m.poly(&[(x(1), 14), (x(6), 4), (x(14), 2), (x(15), 7), (x(9), 15)]);
        });
        g.part(wing, Mat::Cloth, Form::Dome(3), Z::new(1, 2), true, |m| {
            m.poly(&[(x(1), 15), (x(10), 14), (x(12), 21), (x(8), 27), (x(2), 23)]);
        });
    }
    // The dark margins, the pale spots in them, the eyespots.
    for (a, b) in [((2, 5), (11, 2)), ((21, 2), (30, 5)), ((5, 22), (8, 26)), ((27, 22), (24, 26))] {
        g.crease(wing, Tone::Deep, a, b);
    }
    for side in [-1, 1] {
        let x = |u: i32| 16 + side * u;
        g.blob(Ramp::HairWhite.at(Tone::Light), x(12) - i32::from(side > 0), 4);
        g.blob(Ramp::ClothBlack.at(Tone::Base), x(8) - i32::from(side > 0), 19);
        g.px(Ramp::ClothMustard.at(Tone::Light), x(8), 19);
        g.crease(wing, Tone::Shade, (x(3), 13), (x(10), 6));
    }
    g.part(Ramp::ClothBlack, Mat::Soft, Form::Upright, Z::new(2, 3), true, |m| m.ell(14, 6, 4, 20));
    g.path(Ramp::ClothBlack.at(Tone::Base), &[(15, 6), (12, 2), (11, 1)]);
    g.path(Ramp::ClothBlack.at(Tone::Base), &[(16, 6), (19, 2), (20, 1)]);
}

/// A tortoise: a domed shell in scutes, each lit at its upper edge, the rim of marginals, its
/// head out with an eye, its feet.
fn tortoise(g: &mut G<'_>, shell: Ramp) {
    let skin = Ramp::ClothMoss;
    g.dome(skin, Mat::Soft, 2, |m| {
        m.ell(22, 12, 9, 7);
        m.ell(5, 21, 5, 6);
        m.ell(19, 21, 5, 6);
    });
    g.over(shell, Mat::Gloss, 7, |m| {
        m.ell(2, 6, 25, 18);
        m.cut_rect(0, 21, 32, 11);
    });
    g.over(shell, Mat::Soft, 1, |m| m.poly(&[(2, 17), (27, 17), (26, 21), (3, 21)]));
    for (a, b) in [((9, 8), (7, 17)), ((15, 7), (15, 17)), ((21, 8), (23, 17)), ((4, 12), (26, 12))] {
        g.crease(shell, Tone::Deep, a, b);
    }
    for x in [5, 9, 13, 17, 21, 25] {
        g.crease(shell, Tone::Shade, (x, 18), (x, 21));
    }
    if g.big() {
        for (a, b) in [((10, 9), (14, 9)), ((16, 9), (20, 9)), ((8, 13), (13, 13)), ((16, 13), (21, 13))] {
            g.crease(shell, Tone::High, a, b);
        }
        g.px(Ramp::ClothBlack.at(Tone::Base), 27, 14);
    }
}

/// Gold dust: a drawstring pouch, its neck gathered and tied, and in front of it a heap of the
/// dust that glitters, a few grains loose.
fn dust(g: &mut G<'_>, glows: bool, gold: Ramp) {
    let leather = Ramp::Leather;
    g.dome(leather, Mat::Soft, 6, |m| {
        m.ell(2, 9, 18, 18);
        m.poly(&[(8, 5), (14, 5), (16, 11), (6, 11)]);
    });
    g.over(leather, Mat::Soft, 2, |m| m.ell(5, 1, 12, 6));
    g.crease(leather, Tone::Deep, (7, 3), (15, 3));
    for (a, b) in [((8, 8), (6, 16)), ((13, 8), (15, 15))] {
        g.crease(leather, Tone::Shade, a, b);
    }
    g.over(Ramp::ClothRed, Mat::Cloth, 1, |m| m.rect(6, 7, 10, 2));
    g.seg(Ramp::ClothRed.at(Tone::Base), (15, 8), (18, 13), 1);
    g.glow = glows;
    g.over(gold, Mat::Gloss, 5, |m| {
        m.ell(10, 18, 20, 11);
        m.cut_rect(0, 27, 32, 5);
    });
    g.glow = false;
    let big = g.big();
    for (k, &(x, y)) in [(16, 20), (22, 21), (19, 24), (13, 24), (25, 24), (18, 22)].iter().enumerate() {
        if big || k % 2 == 0 {
            g.px(if k % 3 == 0 { glint() } else { gold.at(Tone::Glint) }, x, y);
        }
    }
    if big {
        g.flecks(gold, Tone::Shade, 5, 41, (12, 23, 16, 3));
        g.blob(gold.at(Tone::Light), 6, 27);
        g.px(gold.at(Tone::High), 29, 26);
    }
}

// ---------------------------------------------------------------------------------------------
// Spells and statuses

/// A spell: a disc of its school's light, deep at its rim and brightest up and to the left of
/// its heart, a lit bevel on the rim's upper-left, and the mark itself white-hot with the
/// school's glow round it.
fn spell(g: &mut G<'_>, look: &IconLook, ramp: Ramp) {
    let s = g.s;
    let mut l = Canvas::new(s, s);
    l.set_emitting(look.glow);
    // In doubled px from the middle, so an even disc has a half-px centre.
    let r = s - 1;
    let seed = h32(s as u32, ramp.at(Tone::Base).0 as u32, SALT);
    for y in 0..s {
        for x in 0..s {
            let (dx, dy) = (2 * x + 1 - s, 2 * y + 1 - s);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                continue;
            }
            let d = isqrt(d2 as u64) as i32 * 100 / r;
            // The light's heart sits up and to the left; the bands break in 2 px clusters.
            let (fx, fy) = (dx + r / 3, dy + r / 3);
            let f = isqrt((fx * fx + fy * fy) as u64) as i32 * 100 / r;
            let jitter = below(h32((x >> 1) as u32, (y >> 1) as u32, seed), 9) as i32 - 4;
            let rim = d > 84;
            let tone = if rim {
                if dx + dy < -r / 2 {
                    Tone::Light
                } else if dx + dy > r / 2 {
                    Tone::Deep
                } else {
                    Tone::Shade
                }
            } else {
                match f + jitter {
                    v if v > 105 => Tone::Mid,
                    v if v > 75 => Tone::Base,
                    v if v > 40 => Tone::Light,
                    _ => Tone::High,
                }
            };
            let n = normal(dx * UNIT * 3 / (4 * r.max(1)), dy * UNIT * 3 / (4 * r.max(1)));
            l.put(x, y, ramp.at(tone), n, if rim { 2 } else { 1 });
        }
    }
    g.c.stamp(&l, 0, 0);
    // A grey disc (iron, grey cloth) has no colour for a glow to show against: its mark is dark.
    if matches!(ramp, Ramp::ClothGrey | Ramp::Iron | Ramp::Stone | Ramp::Slate | Ramp::HairWhite) {
        mark(g, look.mark, ramp, Ink::Dark);
        if g.big() {
            g.px(glint(), 7, 6);
            g.px(glint(), 8, 5);
        }
        return;
    }
    mark(g, look.mark, Ramp::HairWhite, Ink::Pale);
    // The mark's glow: the disc's px beside it go to its brightest.
    let mut halo = Vec::new();
    for y in 0..s {
        for x in 0..s {
            let Some((q, _)) = Ramp::of(g.c.get(x, y)) else { continue };
            if q != ramp {
                continue;
            }
            let near = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| matches!(Ramp::of(g.c.get(x + dx, y + dy)), Some((w, _)) if w == Ramp::HairWhite));
            if near {
                halo.push((x, y));
            }
        }
    }
    for (x, y) in halo {
        g.c.recolour(x, y, ramp.at(Tone::Glint));
    }
    if g.big() {
        g.px(glint(), 7, 6);
        g.px(glint(), 8, 5);
    }
}

/// A status: an iron bezel round a dark face, its mark in the effect's colour, bright.
fn status(g: &mut G<'_>, look: &IconLook, ramp: Ramp, bezel: Ramp) {
    g.dome(bezel, Mat::Metal, 3, |m| m.ell(1, 1, 30, 30));
    g.part(ramp, Mat::Soft, Form::Dome(6), Z::new(1, 2), true, |m| m.ell(4, 4, 24, 24));
    g.step_where(ramp, -4, |_, _| true);
    mark(g, look.mark, ramp, Ink::Bright);
    if g.big() {
        g.px(glint(), 6, 7);
    }
}

/// How a mark is inked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ink {
    /// Pale (a spell's white-hot mark: the disc's glow is laid round it after).
    Pale,
    /// A step brighter than what it lies on (a status's dark face, an object).
    Bright,
    /// Three steps darker (a pale disc: grey, iron).
    Dark,
}

/// A mark in the middle of a disc or over an object, in `ramp`, inked as `ink`.
fn mark(g: &mut G<'_>, m: IconMark, ramp: Ramp, ink: Ink) {
    let halo = ink == Ink::Pale;
    let big = g.big();
    let (lit, base, dark) = (ramp.at(Tone::High), ramp.at(Tone::Light), ramp.at(Tone::Shade));
    let mat = if halo { Mat::Paper } else { Mat::Gloss };
    let lift = match ink {
        Ink::Pale => 0,
        Ink::Bright => 1,
        Ink::Dark => -3,
    };
    let sep = true;
    let body = |g: &mut G<'_>, r: i32, f: &dyn Fn(&mut M<'_>)| {
        let s = g.s;
        let before = g.c.clone();
        g.part(ramp, mat, Form::Dome(r), Z::new(3, 4), sep, |mm| f(mm));
        // What the mark drew, a step brighter than the face it lies on.
        for y in 0..s {
            for x in 0..s {
                if g.c.get(x, y) != before.get(x, y) {
                    if let Some((q, t)) = Ramp::of(g.c.get(x, y)) {
                        if q == ramp {
                            g.c.recolour(x, y, ramp.at(t.step(lift)));
                        }
                    }
                }
            }
        }
    };
    match m {
        IconMark::None | IconMark::Cork => {}
        IconMark::Flame => {
            body(g, 3, &|mm| {
                mm.poly(&[
                    (16, 4),
                    (20, 10),
                    (23, 16),
                    (22, 23),
                    (16, 27),
                    (10, 23),
                    (9, 16),
                    (12, 12),
                    (13, 15),
                    (15, 10),
                ]);
            });
            g.part(ramp, Mat::Paper, Form::Dome(2), Z::new(4, 4), false, |mm| {
                mm.poly(&[(16, 13), (19, 19), (18, 24), (14, 24), (13, 19)]);
            });
            g.tint_rect(ramp, Tone::High, 15, 18, 3, 5);
            if !halo {
                g.px(lit, 16, 22);
            }
        }
        IconMark::Frost => {
            body(g, 1, &|mm| {
                for (a, b) in [((16, 4), (16, 28)), ((6, 10), (26, 22)), ((6, 22), (26, 10))] {
                    mm.line(a, b, 3);
                }
            });
            if big {
                for (a, b) in [((13, 7), (16, 10)), ((19, 7), (16, 10)), ((13, 25), (16, 22)), ((19, 25), (16, 22))] {
                    g.seg(base, a, b, 1);
                }
                g.px(lit, 16, 16);
            }
        }
        IconMark::Leaf | IconMark::Thorn => {
            body(g, 3, &|mm| mm.poly(&[(7, 25), (9, 15), (15, 9), (25, 6), (23, 16), (17, 22)]));
            g.crease(ramp, Tone::Shade, (8, 24), (22, 9));
            if big {
                for (a, b) in [((12, 20), (11, 15)), ((15, 17), (20, 17)), ((17, 14), (16, 10))] {
                    g.crease(ramp, Tone::Shade, a, b);
                }
            }
            if m == IconMark::Thorn {
                for (x, y) in [(6, 20), (12, 26), (21, 5), (26, 11)] {
                    g.blob(dark, x, y);
                }
            }
        }
        IconMark::Bolt => {
            body(g, 1, &|mm| mm.poly(&[(19, 3), (9, 17), (15, 17), (11, 29), (23, 13), (17, 13), (22, 3)]));
            g.crease(ramp, Tone::High, (18, 5), (11, 16));
        }
        IconMark::Burst | IconMark::Star => {
            let star = m == IconMark::Star;
            body(g, 2, &|mm| {
                if star {
                    mm.poly(&[(16, 3), (19, 13), (29, 16), (19, 19), (16, 29), (13, 19), (3, 16), (13, 13)]);
                } else {
                    for (a, b) in [((16, 3), (16, 29)), ((3, 16), (29, 16)), ((7, 7), (25, 25)), ((25, 7), (7, 25))] {
                        mm.line(a, b, 3);
                    }
                    mm.ell(10, 10, 12, 12);
                }
            });
            g.dome(ramp, Mat::Paper, 2, |mm| mm.ell(13, 13, 6, 6));
            g.px(lit, 15, 15);
        }
        IconMark::Fist | IconMark::Hammer => {
            // A hammer: its haft, and the head across it.
            body(g, 1, &|mm| mm.line((9, 26), (18, 14), 4));
            body(g, 2, &|mm| mm.poly(&[(12, 8), (21, 4), (27, 13), (18, 17)]));
            g.crease(ramp, Tone::High, (13, 8), (21, 5));
            g.crease(ramp, Tone::Shade, (18, 16), (26, 13));
        }
        IconMark::Web => {
            let ink = match ink {
                Ink::Pale => ramp.at(Tone::Light),
                Ink::Bright => lit,
                Ink::Dark => ramp.at(Tone::Deep),
            };
            for (a, b) in [((16, 3), (16, 29)), ((3, 16), (29, 16)), ((7, 7), (25, 25)), ((25, 7), (7, 25))] {
                g.seg(ink, a, b, 1);
            }
            for r in [5, 11] {
                if !big && r == 5 {
                    continue;
                }
                let rr = Rect::new(g.v(16 - r), g.v(16 - r), g.v(2 * r), g.v(2 * r));
                let mut l = Canvas::new(g.s, g.s);
                l.ellipse(rr, ink, 1);
                let inner = Rect::new(rr.x + 1, rr.y + 1, rr.w - 2, rr.h - 2);
                l.ellipse(inner, Ix::CLEAR, 0);
                g.c.stamp(&l, 0, 0);
            }
            let _ = base;
        }
        IconMark::Skull => {
            g.part(Ramp::Bone, Mat::Soft, Form::Dome(4), Z::new(3, 4), sep, |mm| {
                mm.ell(8, 5, 16, 15);
                mm.rect(11, 17, 10, 7);
            });
            for x in [11, 17] {
                g.rect(Ix::SEAM, x, 12, 4, 4);
            }
            g.rect(Ix::SEAM, 15, 17, 2, 2);
            g.seg(Ramp::Bone.at(Tone::Shade), (13, 22), (19, 22), 1);
            if big {
                for x in [14, 16, 18] {
                    g.px(Ramp::Bone.at(Tone::Shade), x, 23);
                }
            }
        }
        IconMark::Shield => {
            body(g, 3, &|mm| mm.poly(&[(8, 6), (24, 6), (24, 15), (16, 27), (8, 15)]));
            g.crease(ramp, Tone::Shade, (16, 7), (16, 25));
            g.crease(ramp, Tone::Shade, (9, 13), (23, 13));
            g.crease(ramp, Tone::High, (9, 7), (15, 7));
        }
        IconMark::Drop => {
            body(g, 4, &|mm| {
                mm.poly(&[(16, 4), (22, 15), (10, 15)]);
                mm.ell(9, 12, 14, 15);
            });
            g.blob(lit, 12, 17);
        }
        IconMark::Heart => {
            body(g, 4, &|mm| {
                mm.ell(6, 7, 11, 11);
                mm.ell(15, 7, 11, 11);
                mm.poly(&[(7, 14), (25, 14), (16, 26)]);
            });
            g.blob(lit, 10, 10);
        }
    }
}
