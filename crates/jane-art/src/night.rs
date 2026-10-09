//! The night kit (NIGHT.md §4.3): the overlays the presenter lays over geometry it already
//! knows after the bell, never the sim. Boards and bricks over windows, rust run down from sills,
//! plaster peeled, coats on the railings, things hung from lamp arms and branches, chalk on the
//! cobbles, the Company's paper over the council's, ironwork on the Works' roads, stains.
//!
//! ```text
//! night::ALL               every look, in the atlas's order (one 256 x 256 page on a console)
//! night::render(look)      its canvas, four layers, ending `ao_contact; outline` as the kit does
//! night::anchor(look)      the px of the canvas laid on the socket's point
//! ```
//!
//! **Sockets and anchors.** Every look is laid by its anchor on a point the presenter knows:
//!
//! | Looks | Canvas | Anchor (laid on) |
//! | --- | --- | --- |
//! | `Boards`, `Brick`, `Card`, `Crack` | the opening plus a px each side | `(1, 1)`: the opening's top-left |
//! | `Rust` | a run a few px wide | top middle: the underside of a sill, a gutter, an arm |
//! | `Peel` | a patch | top-left, on a plaster face |
//! | `Coat` | a coat on its hook | top middle: the rail or peg it hangs from |
//! | `Cage`, `HandBell`, `TagChain`, `Strip`, `Chain`, `DoorTag` | the thing on its cord | top middle: where it hangs from |
//! | `Tally`, `Nine`, `Grate`, `Stain` | flat on the ground | top-left of the mark |
//! | `Ring` | a ring of chalk round a post | its middle, on the post at hand height |
//! | `Notice` | a sheet pasted at its corners | top-left, on a board's face |
//! | `Pipe` | a run along a wall | top-left, on a face |
//! | `Barrier` | a trestle across a track, two cells wide | its foot: bottom-left |
//!
//! What lies flat on a surface (the window covers, rust, peel, chalk, paper, pipes, a grate, a
//! stain) takes the surface's own height and relief where it is laid; what hangs (a coat, a cage,
//! a bell, a strip) and the barrier carry heights of their own, counted from their anchor (the
//! anchor's row the highest, the rows under it lower: the presenter rebases them on the rail).
//!
//! **The halfway rules (NIGHT.md §1).** Everything here could have come out of the Works or the
//! School: iron, rust, soot, numbered cards, registers, notices, the bell. Nothing that hangs could
//! read as a person: a coat hangs limp from its collar on a hook, shoulders fallen, no head; no
//! loop of rope anywhere. Marks laid on a surface (rust, chalk, a crack in glass, plaster gone)
//! are paint, not things, and take no outline; a stain is the contact shade alone (ART.md §3.1).

use alloc::vec::Vec;

use jane_core::grid::Rect;

use crate::canvas::{Canvas, FLAT, Normal, Z, bresenham, normal};
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone};

/// A window's opening as the facade paints it (`terrain::hard::facade`): a ground floor
/// casement, a sash, an upper floor's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opening {
    /// 10 x 10.
    Casement,
    /// 8 x 11.
    Sash,
    /// 8 x 8.
    Upper,
}

impl Opening {
    /// Every opening.
    pub const ALL: [Opening; 3] = [Opening::Casement, Opening::Sash, Opening::Upper];

    /// Its size, px.
    pub const fn size(self) -> (i32, i32) {
        match self {
            Opening::Casement => (10, 10),
            Opening::Sash => (8, 11),
            Opening::Upper => (8, 8),
        }
    }

    /// The opening of size `(w, h)`, if it is one.
    pub fn of_size(w: i32, h: i32) -> Option<Opening> {
        Opening::ALL.into_iter().find(|o| o.size() == (w, h))
    }

    /// Whether the opening's px `(x, y)` (its own top-left at `(0, 0)`) is glass, not frame:
    /// the facade's casement (an upper window as a casement) or its sash's bars.
    fn pane(self, x: i32, y: i32) -> bool {
        let (w, h) = self.size();
        if x <= 0 || y <= 0 || x >= w - 1 || y >= h - 1 {
            return false;
        }
        let (mx, my) = (w / 2, h / 2);
        match self {
            Opening::Sash => {
                let (q1, q3) = (my / 2, my + (h - 1 - my) / 2 + 1);
                !(x == mx || y == my || y == q1 || y == q3)
            }
            _ => !(x == mx || (w >= 10 && x == mx - 1) || y == my),
        }
    }
}

/// One look of the night kit. Fields: `layout` and `variant` pick a shape, `opening` the window
/// it fits, `cloth` a coat's cloth ([`COAT_CLOTHS`]), `cut` its cut, `groups` the tally's groups of five.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Look {
    /// Planks nailed across a window: three layouts.
    Boards { layout: u8, opening: Opening },
    /// A window bricked up: coursed (`rough` false) or rough.
    Brick { rough: bool, opening: Opening },
    /// A numbered Company card stood in a window.
    Card { opening: Opening },
    /// A cracked pane: two.
    Crack { variant: u8, opening: Opening },
    /// Rust run down from a sill or a bolt: short, long, forked, from a bolt.
    Rust { variant: u8 },
    /// Plaster gone: to brick, to lath, to stone.
    Peel { variant: u8 },
    /// A coat hung on a rail or a peg: four cuts in four drab cloths (`cloth` 0..4).
    Coat { cut: u8, cloth: u8 },
    /// An empty birdcage on a hook.
    Cage,
    /// A hand bell on a cord.
    HandBell,
    /// A Company tag on a chain.
    TagChain,
    /// A strip of cloth tied on: three.
    Strip { variant: u8 },
    /// A chalk register on the ground, five strokes a group: one, two or four groups.
    Tally { groups: u8 },
    /// A time chalked on the ground: "9".
    Nine,
    /// Julie's ring of chalk round a lamp post (NIGHT.md §5.2).
    Ring,
    /// A Company notice pasted over a notice: two.
    Notice { variant: u8 },
    /// A pipe run along a wall: straight, an elbow, a valve.
    Pipe { variant: u8 },
    /// A grate set in a road: square, a drain's bars.
    Grate { variant: u8 },
    /// ROAD CLOSED BY ORDER OF THE COMPANY: a trestle barrier, two cells wide.
    Barrier,
    /// A stain, the contact shade only (ART.md §3.1): two.
    Stain { variant: u8 },
    /// A chain and padlock across barn doors, a Company tag on it.
    Chain,
    /// A Company tag on a door.
    DoorTag,
}

/// The cloths the coats come in.
pub const COAT_CLOTHS: [Ramp; 4] = [Ramp::ClothBlack, Ramp::ClothGrey, Ramp::ClothBrown, Ramp::ClothNavy];

/// Every look, in the atlas's order.
pub fn all() -> Vec<Look> {
    let mut v = Vec::new();
    for o in Opening::ALL {
        for layout in 0..3 {
            v.push(Look::Boards { layout, opening: o });
        }
        v.push(Look::Brick { rough: false, opening: o });
        v.push(Look::Brick { rough: true, opening: o });
        v.push(Look::Card { opening: o });
        for variant in 0..2 {
            v.push(Look::Crack { variant, opening: o });
        }
    }
    v.extend((0..4).map(|variant| Look::Rust { variant }));
    v.extend((0..3).map(|variant| Look::Peel { variant }));
    for cut in 0..4 {
        for cloth in 0..4 {
            v.push(Look::Coat { cut, cloth });
        }
    }
    v.extend([Look::Cage, Look::HandBell, Look::TagChain]);
    v.extend((0..3).map(|variant| Look::Strip { variant }));
    v.extend([1, 2, 4].map(|groups| Look::Tally { groups }));
    v.extend([Look::Nine, Look::Ring]);
    v.extend((0..2).map(|variant| Look::Notice { variant }));
    v.extend((0..3).map(|variant| Look::Pipe { variant }));
    v.extend((0..2).map(|variant| Look::Grate { variant }));
    v.push(Look::Barrier);
    v.extend((0..2).map(|variant| Look::Stain { variant }));
    v.extend([Look::Chain, Look::DoorTag]);
    v
}

/// Whether the look lies flat on what it is laid on (it takes the surface's height and relief).
pub const fn flat(look: Look) -> bool {
    !matches!(
        look,
        Look::Coat { .. }
            | Look::Cage
            | Look::HandBell
            | Look::TagChain
            | Look::Strip { .. }
            | Look::Barrier
            | Look::Chain
            | Look::Ring
    )
}

/// The px of `look`'s canvas laid on its socket's point (the table in the module's doc).
pub fn anchor(look: Look) -> (i32, i32) {
    let c = render(look);
    match look {
        Look::Boards { .. } | Look::Brick { .. } | Look::Card { .. } | Look::Crack { .. } => (1, 1),
        Look::Peel { .. }
        | Look::Tally { .. }
        | Look::Nine
        | Look::Grate { .. }
        | Look::Notice { .. }
        | Look::Pipe { .. }
        | Look::Stain { .. } => (0, 0),
        Look::Ring => (c.w() / 2, c.h() / 2),
        Look::Barrier => (0, c.h() - 1),
        _ => (c.w() / 2, 0),
    }
}

/// Whether `look` takes the selective outline: things do; marks laid on a surface (rust, chalk,
/// plaster gone, a crack in the glass, a stain) and fine wire (a cage's bars, a chain's links)
/// do not, or the line would eat them whole.
fn outlined(look: Look) -> bool {
    !matches!(
        look,
        Look::Rust { .. }
            | Look::Peel { .. }
            | Look::Tally { .. }
            | Look::Nine
            | Look::Ring
            | Look::Stain { .. }
            | Look::Crack { .. }
            | Look::Cage
            | Look::Chain
    )
}

/// `look`'s canvas: four layers, nothing emitting.
pub fn render(look: Look) -> Canvas {
    let mut c = match look {
        Look::Boards { layout, opening } => boards(layout, opening),
        Look::Brick { rough, opening } => brick(rough, opening),
        Look::Card { opening } => card(opening),
        Look::Crack { variant, opening } => crack(variant, opening),
        Look::Rust { variant } => rust(variant),
        Look::Peel { variant } => peel(variant),
        Look::Coat { cut, cloth } => coat(cut % 4, COAT_CLOTHS[usize::from(cloth % 4)]),
        Look::Cage => cage(),
        Look::HandBell => hand_bell(),
        Look::TagChain => tag_chain(),
        Look::Strip { variant } => strip(variant % 3),
        Look::Tally { groups } => tally(groups),
        Look::Nine => nine(),
        Look::Ring => ring(),
        Look::Notice { variant } => notice(variant % 2),
        Look::Pipe { variant } => pipe(variant % 3),
        Look::Grate { variant } => grate(variant % 2),
        Look::Barrier => barrier(),
        Look::Stain { variant } => stain(variant % 2),
        Look::Chain => chain(),
        Look::DoorTag => door_tag(),
    };
    if outlined(look) {
        c.outline();
    }
    if !flat(look) {
        // Hung things stand from their foot up; the presenter rebases them on the anchor's rail.
        c.upright(c.h() - 1);
    }
    c
}

// ---------------------------------------------------------------------------------------------
// Shared strokes.

/// A face standing on a wall, looking south and a little up.
fn face() -> Normal {
    normal(0, 84)
}

/// The night kit's hash of `(a, b)` under `seed`.
fn hs(seed: u32, a: i32, b: i32) -> u32 {
    h32(seed ^ 0x4e49_4748, a as u32, (b as u32).wrapping_mul(0x9e37_79b9) ^ 0x006b_6974)
}

/// One px of `ramp`'s `tone`.
fn px(c: &mut Canvas, x: i32, y: i32, ramp: Ramp, tone: Tone, n: Normal, z: u8) {
    c.put(x, y, ramp.at(tone), n, z);
}

/// The row a board's top edge is on at column `x`: from `y0` at `x0`, falling `slope.0 / slope.1`
/// px a column.
fn y_at(x0: i32, y0: i32, slope: (i32, i32), x: i32) -> i32 {
    y0 + ((x - x0) * slope.0).div_euclid(slope.1.max(1))
}

/// A board nailed across, columns `x0..=x1`, `t` rows thick, its top edge from `y0` falling by
/// `slope` (a clean 0, 1:1 or 1:n step, never a staircase of odd steps): the top two rows lit,
/// the foot in shade, a streak or two of grain, an end split or sawn short by the seed.
#[allow(clippy::too_many_arguments)]
fn board(c: &mut Canvas, x0: i32, x1: i32, y0: i32, slope: (i32, i32), t: i32, ramp: Ramp, seed: u32, z: u8) {
    c.begin();
    let n = face();
    let shift = [0, 1, 0, -1][(hs(seed, 0, 1) % 4) as usize];
    for x in x0..=x1 {
        let y = y_at(x0, y0, slope, x);
        for k in 0..t {
            let tone = if k == t - 1 {
                Tone::Shade
            } else if k < 2 {
                Tone::Light.step(shift.min(0))
            } else {
                Tone::Base.step(shift)
            };
            px(c, x, y + k, ramp, tone, n, z);
        }
    }
    // Grain: a streak or two along the lit rows, in the tone under them.
    let len = x1 - x0 + 1;
    for s in 0..(len / 5).max(1) {
        let h = hs(seed, s, 2);
        let gx = x0 + 1 + below(h, (len - 4).max(1) as u32) as i32;
        let k = 1 + below(h >> 12, (t - 2).max(1) as u32) as i32;
        for d in 0..2 + (h >> 8 & 1) as i32 {
            let x = gx + d;
            if x < x1 {
                c.tint(x, y_at(x0, y0, slope, x) + k, ramp, Tone::Base.step(shift - 1));
            }
        }
    }
    // A weathered end: a corner gone.
    let h = hs(seed, 9, 9);
    if h % 3 == 0 {
        c.clear_px(x0, y_at(x0, y0, slope, x0));
    } else if h % 3 == 1 {
        c.clear_px(x1, y_at(x0, y0, slope, x1) + t - 1);
    }
}

/// A board standing up, columns `x0..x0 + w`, rows `y0..=y1`: lit down its left, shaded down its
/// right, a streak of grain, its top sawn ragged by the seed.
#[allow(clippy::too_many_arguments)]
fn vboard(c: &mut Canvas, x0: i32, w: i32, y0: i32, y1: i32, ramp: Ramp, seed: u32, z: u8) {
    c.begin();
    let n = face();
    let shift = [0, 1, 0, -1][(hs(seed, 0, 3) % 4) as usize];
    for y in y0..=y1 {
        for x in x0..x0 + w {
            let tone = if x == x0 {
                Tone::Light
            } else if x == x0 + w - 1 {
                Tone::Shade
            } else {
                Tone::Base.step(shift)
            };
            px(c, x, y, ramp, tone, n, z);
        }
    }
    let h = hs(seed, 1, 3);
    let gx = x0 + 1 + below(h, (w - 2).max(1) as u32) as i32;
    let gy = y0 + 2 + below(h >> 8, (y1 - y0 - 4).max(1) as u32) as i32;
    for d in 0..3 {
        c.tint(gx, gy + d, ramp, Tone::Base.step(shift - 1));
    }
    if h >> 20 & 1 == 0 {
        c.clear_px(x0 + w - 1, y0);
    } else {
        c.clear_px(x0, y0);
    }
}

/// A nail driven at `(x, y)`: its head catching the light, rust bled from it down the wood.
fn nail(c: &mut Canvas, x: i32, y: i32, z: u8) {
    c.dot(x, y, Ramp::Iron.at(Tone::High), z);
    if c.get(x, y + 1).is_opaque() {
        c.dot(x, y + 1, Ramp::Copper.at(Tone::Shade), z);
    }
}

/// Illegible writing in `ix`: `rows` rows of short words across `w` px from `(x, y)`, two rows a
/// line. The words are in the dialogue, never on the sprite (ART.md §2.3).
#[allow(clippy::too_many_arguments)]
fn writing(c: &mut Canvas, x0: i32, y0: i32, w: i32, rows: i32, ix: Ix, seed: u32, z: u8) {
    for k in 0..rows {
        let y = y0 + 2 * k;
        let mut x = x0 + i32::from(k == 0 && w > 6);
        let end = x0 + w - 1 - (hs(seed, k, 3) % 3) as i32;
        while x < end {
            let word = 1 + (hs(seed, k, x) % 4) as i32;
            let to = (x + word - 1).min(end - 1);
            c.hline(x, to, y, ix, z);
            x = to + 2;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Window covers: the opening plus a px each side, the opening's top-left at (1, 1).

fn cover(o: Opening) -> Canvas {
    let (w, h) = o.size();
    Canvas::new(w + 2, h + 2)
}

/// Planks nailed across a window. `0`: boards across with gaps, set askew; `1`: two boards in a
/// cross over the frame; `2`: boards stood up close over the whole opening, a batten across.
fn boards(layout: u8, o: Opening) -> Canvas {
    let mut c = cover(o);
    let (w, h) = (c.w(), c.h());
    let seed = u32::from(layout) * 977 + (w * 31 + h) as u32;
    let wood = |i: i32| if hs(seed, i, 5) % 3 == 0 { Ramp::WoodDark } else { Ramp::Deadwood };
    match layout % 3 {
        0 => {
            let rows: &[(i32, (i32, i32))] = match o {
                Opening::Casement => &[(0, (1, 7)), (5, (-1, 7)), (9, (0, 1))],
                Opening::Sash => &[(1, (1, 6)), (6, (0, 1)), (10, (-1, 6))],
                Opening::Upper => &[(1, (1, 6)), (6, (-1, 6))],
            };
            for (i, &(y, slope)) in rows.iter().enumerate() {
                let i = i as i32;
                let short = hs(seed, i, 7) % 3;
                let (x0, x1) = (i32::from(short == 1), w - 1 - i32::from(short == 2));
                board(&mut c, x0, x1, y, slope, 3, wood(i), seed ^ i as u32, 2 + 2 * (i as u8 & 1));
                for x in [x0 + 1, x1 - 1] {
                    nail(&mut c, x, y_at(x0, y, slope, x) + 1, 5);
                }
            }
        }
        1 => {
            // A cross, the second board over the first, nailed at its four ends and where they
            // cross; and on a tall window a short board across its foot.
            let off = (h - w) / 2 - 1;
            if o == Opening::Sash {
                board(&mut c, 0, w - 1, h - 3, (0, 1), 3, wood(5), seed ^ 5, 2);
            }
            board(&mut c, 0, w - 1, off, (1, 1), 3, wood(0), seed, 4);
            board(&mut c, 0, w - 1, w - 1 + off - 2, (-1, 1), 3, wood(1), seed ^ 1, 6);
            for x in [1, w - 2] {
                nail(&mut c, x, y_at(0, off, (1, 1), x) + 1, 7);
                nail(&mut c, x, y_at(0, w - 1 + off - 2, (-1, 1), x) + 1, 7);
            }
            nail(&mut c, w / 2 - 1, y_at(0, w - 1 + off - 2, (-1, 1), w / 2 - 1) + 1, 7);
        }
        _ => {
            // Stood up close, edge to edge over the whole opening, a batten nailed across.
            let widths: &[i32] = if w >= 12 { &[4, 4, 4] } else { &[3, 4, 3] };
            let mut x = 0;
            for (i, &bw) in widths.iter().enumerate() {
                let i = i as i32;
                let y0 = (hs(seed, i, 11) % 2) as i32;
                let y1 = h - 1 - (hs(seed, i, 12) % 2) as i32;
                vboard(&mut c, x, bw, y0, y1, wood(i + 3), seed ^ (i as u32 * 7), 2);
                x += bw;
            }
            let by = h / 2 - 1;
            board(&mut c, 0, w - 1, by, (0, 1), 3, Ramp::WoodDark, seed ^ 0x63, 5);
            let mut x = 0;
            for &bw in widths {
                nail(&mut c, x + bw / 2, by + 1, 7);
                nail(&mut c, x + bw / 2, 2, 4);
                x += bw;
            }
        }
    }
    c
}

/// A window bricked up: the bricks newer and redder than the wall's, set in clean grey mortar;
/// coursed, or rough (odd lengths, old and new together, a stone, the mortar smeared).
fn brick(rough: bool, o: Opening) -> Canvas {
    let mut c = cover(o);
    let (ow, oh) = o.size();
    let n = face();
    let seed = (ow * 13 + oh) as u32 ^ if rough { 0x55 } else { 0 };
    c.begin();
    let mortar = |c: &mut Canvas, x: i32, y: i32, t: Tone| px(c, x + 1, y + 1, Ramp::Stone, t, n, 2);
    for course in 0..(oh + 2) / 3 {
        let y0 = course * 3;
        // The bricks of this course: their starts and lengths, staggered course to course.
        let mut x = if course % 2 == 1 { -2 } else { 0 };
        let mut b = 0;
        while x < ow {
            let h = hs(seed, course, b);
            let len = if rough { 3 + (h % 4) as i32 } else { 4 };
            let ramp = if rough && (h >> 8) % 8 == 0 {
                Ramp::Stone
            } else if rough && h >> 8 & 1 == 1 {
                Ramp::Brick
            } else {
                Ramp::RoofTileNew
            };
            let shade = [Tone::Base, Tone::Base, Tone::Lift, Tone::Mid][(h >> 12 & 3) as usize];
            for yy in y0..(y0 + 3).min(oh) {
                for xx in x.max(0)..(x + len + 1).min(ow) {
                    let row = yy - y0;
                    if row == 2 || xx == x + len {
                        // Rough work's joints run wide and grey, coursed work's thin and clean.
                        mortar(&mut c, xx, yy, if rough { Tone::Lift } else { Tone::Light });
                    } else {
                        let t = if row == 0 { shade.step(1) } else { shade };
                        px(&mut c, xx + 1, yy + 1, ramp, t, n, 2);
                    }
                }
            }
            x += len + 1;
            b += 1;
        }
    }
    // The lintel's shadow on the first course.
    for x in 0..ow {
        c.tint(x + 1, 1, Ramp::RoofTileNew, Tone::Mid);
        c.tint(x + 1, 1, Ramp::Brick, Tone::Mid);
    }
    c
}

/// Night glass in every pane of the opening: near black, a faint slant of the sky in the upper
/// panes. The frame is the facade's and is left alone.
fn night_glass(c: &mut Canvas, o: Opening, z: u8) {
    let (ow, oh) = o.size();
    c.begin();
    let n = normal(0, 110);
    for y in 0..oh {
        for x in 0..ow {
            if o.pane(x, y) {
                let streak = y < oh / 2 && (x - y).rem_euclid(5) == 2;
                px(c, x + 1, y + 1, Ramp::Glass, if streak { Tone::Shade } else { Tone::Deep }, n, z);
            }
        }
    }
}

/// A numbered card stood in a window against the glass, leaning: a ruled heading, a bold number.
fn card(o: Opening) -> Canvas {
    let mut c = cover(o);
    let (ow, oh) = o.size();
    let (cw, ch) = (if ow >= 10 { 6 } else { 5 }, if oh >= 10 { 7 } else { 6 });
    let x = 1 + (ow - cw) / 2;
    let y = 1 + oh - ch - 1;
    night_glass(&mut c, o, 1);
    // Its shadow on the glass behind it.
    c.fill_normal(Rect::new(x + 1, y + 1, cw, ch), Ramp::Glass.at(Tone::Deep), face(), 1);
    // The card, leaning a px to the right at its top.
    let pts = [(x + 1, y), (x + cw, y), (x + cw - 1, y + ch - 1), (x, y + ch - 1)];
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(&pts, Ix::INK, 1);
    c.begin();
    for yy in 0..c.h() {
        for xx in 0..c.w() {
            if m.get(xx, yy).is_opaque() {
                let t = if yy == y { Tone::High } else { Tone::Light };
                px(&mut c, xx, yy, Ramp::ClothCream, t, face(), 3);
            }
        }
    }
    // The heading, ruled; the number under it, bold.
    c.hline(x + 2, x + cw - 2, y + 1, Ix::SEAM, 3);
    let digit: [u8; 5] = match o {
        Opening::Casement => [0b101, 0b101, 0b111, 0b001, 0b001],
        Opening::Sash => [0b111, 0b001, 0b010, 0b010, 0b010],
        Opening::Upper => [0b111, 0b001, 0b111, 0b100, 0b111],
    };
    let rows = if ch >= 7 { 5 } else { 4 };
    let (dx, dy) = (x + (cw - 3) / 2, y + 2);
    for (r, bits) in digit.iter().enumerate().take(rows) {
        for b in 0..3 {
            if bits >> (2 - b) & 1 == 1 {
                c.dot(dx + b, dy + r as i32, Ix::INK, 4);
            }
        }
    }
    c
}

/// A cracked pane in night glass: `0` a star from where a stone struck, `1` a long crack and a
/// shard gone, the dark room showing. The crack catches the light; the glass beside it does not.
fn crack(variant: u8, o: Opening) -> Canvas {
    let mut c = cover(o);
    let (ow, oh) = o.size();
    night_glass(&mut c, o, 2);
    let n = normal(0, 110);
    let lit = |c: &mut Canvas, x: i32, y: i32, t: Tone| {
        if o.pane(x - 1, y - 1) {
            px(c, x, y, Ramp::Glass, t, n, 2);
        }
    };
    if variant % 2 == 0 {
        let (ix, iy) = (1 + ow / 4, 1 + oh / 4 + 1);
        // The struck pane crazed through, paler than the whole ones round it.
        let (mut x0, mut x1, mut y0, mut y1) = (ix - 1, ix - 1, iy - 1, iy - 1);
        while o.pane(x0 - 1, iy - 1) {
            x0 -= 1;
        }
        while o.pane(x1 + 1, iy - 1) {
            x1 += 1;
        }
        while o.pane(ix - 1, y0 - 1) {
            y0 -= 1;
        }
        while o.pane(ix - 1, y1 + 1) {
            y1 += 1;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                lit(&mut c, x + 1, y + 1, if x == x1 || y == y1 { Tone::Shade } else { Tone::Mid });
            }
        }
        let rays: [(i32, i32); 4] = [(6, -1), (5, 5), (-2, 4), (1, -3)];
        for (k, (dx, dy)) in rays.into_iter().enumerate() {
            let mut i = 0;
            bresenham(ix, iy, ix + dx, iy + dy, |x, y| {
                let t = if i < 2 {
                    Tone::High
                } else if (i + k) % 3 == 0 {
                    Tone::Base
                } else {
                    Tone::Light
                };
                lit(&mut c, x, y, t);
                i += 1;
            });
        }
        lit(&mut c, ix, iy, Tone::Glint);
    } else {
        // A long crack falling from the top right to the foot, jagged.
        let (mut x, mut y) = (ow, 2);
        let mut k = 0;
        while y < oh && x > 1 {
            let h = hs(0xc4ac, k, 1);
            let nx = x - 1 - (h % 2) as i32;
            let mut i = 0;
            bresenham(x, y, nx, y + 1, |a, b| {
                lit(&mut c, a, b, if i == 0 && k % 3 == 0 { Tone::High } else { Tone::Light });
                i += 1;
            });
            x = nx;
            y += 1;
            k += 1;
        }
        // The shard gone from a lower pane: the room's dark, its edges lit.
        let (sx, sy) = (2 + i32::from(ow >= 10), oh - 2);
        let shard = [(sx, sy - 1), (sx + 1, sy - 1), (sx, sy), (sx + 1, sy), (sx, sy + 1)];
        for (a, b) in shard {
            if o.pane(a - 1, b - 1) {
                c.dot(a, b, Ix::SEAM, 2);
            }
        }
        lit(&mut c, sx + 2, sy - 1, Tone::High);
        lit(&mut c, sx + 1, sy + 1, Tone::Light);
    }
    c
}

// ---------------------------------------------------------------------------------------------
// Marks on a wall: rust and plaster gone.

/// Rust run down a face: dark and heavy where it gathers under the sill, thinning, paling and
/// breaking up as it falls. `0` short, `1` long, `2` forked, `3` from a bolt.
fn rust(variant: u8) -> Canvas {
    let (w, h) = [(4, 7), (4, 13), (7, 12), (4, 11)][usize::from(variant % 4)];
    let mut c = Canvas::new(w, h);
    let seed = 0x7275 + u32::from(variant);
    let top = if variant == 3 { 2 } else { 0 };
    // Each streak: its column, its length, how wide it starts.
    let streaks: &[(i32, i32, i32)] = match variant % 4 {
        0 => &[(1, 6, 2), (3, 3, 1)],
        1 => &[(1, 13, 2), (0, 5, 1), (3, 8, 1)],
        2 => &[(2, 6, 3), (1, 12, 1), (5, 9, 1), (4, 5, 1)],
        _ => &[(1, 9, 2), (3, 4, 1)],
    };
    c.begin();
    for (s, &(sx, len, wide)) in streaks.iter().enumerate() {
        let mut x = sx;
        for r in 0..len {
            let y = top + r;
            if y >= h {
                break;
            }
            // Water wanders: a step aside once down the run.
            if r == len / 2 && hs(seed, s as i32, 1) % 2 == 0 && x + 1 < w {
                x += 1;
            }
            let tail = r * 3 >= len * 2;
            if tail && hs(seed, s as i32, r) % 3 == 0 {
                continue;
            }
            // Orange-brown, never the red of anything else: the run's head in its mid, paling
            // to the iron's own orange as it thins.
            let t = if r * 3 < len {
                Tone::Mid
            } else if !tail {
                Tone::Base
            } else {
                Tone::Lift
            };
            let width = if r * 5 < len * 2 { wide } else { 1 };
            for d in 0..width {
                px(&mut c, x + d - (width - 1) / 2, y, Ramp::Copper, if d > 0 { t.step(1) } else { t }, FLAT, 1);
            }
        }
    }
    if variant % 4 == 3 {
        // A bolt's square head, rust weeping from under it.
        let b = w / 2 - 1;
        c.rect_lit(Rect::new(b, 0, 2, 2), Ramp::Iron, 2);
        c.dot(b, 0, Ramp::Iron.at(Tone::High), 2);
    } else {
        // Gathered under the sill: the run's head, a broken row across.
        for x in 0..w {
            if hs(seed, x, 40) % 3 == 0 {
                px(&mut c, x, 0, Ramp::Copper, Tone::Base, FLAT, 1);
            }
        }
    }
    c
}

/// Plaster broken away: `0` to the brick under it, `1` to the lath, `2` to rubble stone. The hole's
/// top and left lie in the shadow of the plaster's lip; its broken foot and right edge catch the
/// light.
fn peel(variant: u8) -> Canvas {
    let (w, h) = [(13, 10), (11, 12), (14, 9)][usize::from(variant % 3)];
    let mut c = Canvas::new(w, h);
    let seed = 0x7065 + u32::from(variant) * 17;
    // The hole: a few lobes, its edge broken by the seed.
    let mut m = Canvas::new(w, h);
    let lobes: &[(i32, i32, i32, i32)] = match variant % 3 {
        0 => &[(1, 1, 9, 6), (4, 3, 8, 6), (2, 5, 5, 3)],
        1 => &[(2, 1, 7, 7), (1, 5, 6, 6), (4, 7, 5, 4)],
        _ => &[(1, 2, 7, 5), (5, 1, 8, 6), (9, 4, 4, 4)],
    };
    for &(x, y, lw, lh) in lobes {
        m.ellipse(Rect::new(x, y, lw, lh), Ix::INK, 1);
    }
    for y in 0..h {
        for x in 0..w {
            let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| !m.get(x + dx, y + dy).is_opaque());
            if edge && hs(seed, x, y) % 3 == 0 {
                m.clear_px(x, y);
            }
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                m.clear_px(x, y);
            }
        }
    }
    m.despike();
    let inside = |x: i32, y: i32| m.get(x, y).is_opaque();
    let n = face();
    c.begin();
    for y in 0..h {
        for x in 0..w {
            if !inside(x, y) {
                continue;
            }
            let (ramp, t) = match variant % 3 {
                0 => {
                    let course = y / 3;
                    let col = (x + if course % 2 == 1 { 2 } else { 0 }) % 5;
                    if y % 3 == 2 || col == 4 {
                        (Ramp::Stone, Tone::Mid)
                    } else {
                        let b = hs(seed, (x + 2 * (course % 2)) / 5, course);
                        let t = [Tone::Base, Tone::Mid, Tone::Base, Tone::Lift][(b % 4) as usize];
                        (Ramp::Brick, if y % 3 == 0 { t.step(1) } else { t })
                    }
                }
                1 => {
                    if y % 2 == 0 {
                        (Ramp::WoodPale, if (x + y / 2) % 7 == 0 { Tone::Mid } else { Tone::Base })
                    } else if (x + y * 2) % 4 == 0 {
                        // The plaster's key, squeezed through between the laths.
                        (Ramp::Limewash, Tone::Shade)
                    } else {
                        (Ramp::WoodDark, Tone::Deep)
                    }
                }
                _ => {
                    let course = y / 3;
                    let b = hs(seed, (x + course * 3) / 4, course);
                    let joint = y % 3 == 2 || (x + course * 3) % 4 == 3 && b % 2 == 0;
                    if joint {
                        (Ramp::Stone, Tone::Shade)
                    } else {
                        let t = [Tone::Base, Tone::Lift, Tone::Mid][(b % 3) as usize];
                        (Ramp::Stone, if y % 3 == 0 { t.step(1) } else { t })
                    }
                }
            };
            // The lip's shadow down the hole's top and left.
            let shadowed = !inside(x, y - 1) || !inside(x - 1, y) || !inside(x - 1, y - 1);
            let t = if shadowed { t.step(-2).min(Tone::Shade) } else { t };
            px(&mut c, x, y, ramp, t, n, 1);
        }
    }
    // The broken plaster: its edge along the hole's foot and right, lit; a thin dark lip on top.
    c.begin();
    for y in 0..h {
        for x in 0..w {
            if inside(x, y) {
                continue;
            }
            // The broken edge faces up along the hole's foot (lit, unbroken) and left along its
            // right side (lit, broken); above and to the left it faces away and is the wall's.
            if inside(x, y - 1) {
                px(&mut c, x, y, Ramp::Limewash, Tone::High, n, 2);
            } else if inside(x - 1, y) && hs(seed, x, y) % 2 == 0 {
                px(&mut c, x, y, Ramp::Limewash, Tone::Light, n, 2);
            }
        }
    }
    c
}

// ---------------------------------------------------------------------------------------------
// Hung things.

/// A coat hung limp from its collar on a hook: the shoulders fallen away from the hook, the
/// sleeves hanging at its sides, no head. Cuts: `0` a long overcoat, double-breasted; `1` a short
/// jacket; `2` a donkey jacket, its leather yoke across the shoulders; `3` a belted mackintosh.
fn coat(cut: u8, ramp: Ramp) -> Canvas {
    let cut = usize::from(cut);
    let len = [24, 15, 16, 21][cut];
    let flare = [1, 0, 0, 1][cut];
    let cuff = [16, 14, 14, 16][cut];
    let (w, cx) = (15, 7);
    let mut c = Canvas::new(w, len);
    let hem = len - 1;
    // The body: its collar's two points either side of the hook, the shoulders fallen away
    // from it (the right a px lower: it hangs a little askew), then straight to the hem.
    let body = [
        (cx - 2, 2),
        (cx + 2, 2),
        (cx + 4, 5),
        (cx + 5, 8),
        (cx + 5 + flare, hem),
        (cx - 5 - flare, hem),
        (cx - 5, 7),
        (cx - 4, 4),
    ];
    c.polygon_cloth(&body, ramp, 70, Z::new(2, 3));
    // The inside of the collar's back between its points, dark; the front's opening down the
    // middle.
    for (x, y) in [(cx - 1, 2), (cx, 2), (cx + 1, 2), (cx, 3)] {
        c.tint(x, y, ramp, Tone::Deep);
    }
    for y in 4..=hem {
        c.tint(cx, y, ramp, Tone::Shade);
    }
    // The hook it hangs from: an iron peg with a knob, its tip in the collar.
    c.fill_normal(Rect::new(cx, 0, 1, 3), Ramp::Iron.at(Tone::Light), face(), 8);
    c.dot(cx + 1, 0, Ramp::Iron.at(Tone::Base), 8);
    c.dot(cx - 1, 0, Ramp::Iron.at(Tone::High), 8);
    // Drag folds from the hook to the fallen shoulders.
    c.line((cx - 2, 4), (cx - 3, 7), ramp.at(Tone::Mid), 1, 3);
    c.line((cx + 2, 4), (cx + 3, 7), ramp.at(Tone::Shade), 1, 3);
    // Folds in the skirt.
    c.folds(Rect::new(cx - 6, hem - (len - 9) / 2, 13, (len - 9) / 2), ramp, 4, [0, 9000, 20000, 5000][cut]);
    match cut {
        0 => {
            // Lapels, wide; two rows of buttons.
            c.line((cx - 1, 3), (cx - 3, 8), ramp.at(Tone::Lift), 1, 3);
            for y in [9, 12, 15] {
                c.tint(cx - 2, y, ramp, Tone::Light);
                c.tint(cx + 2, y, ramp, Tone::Light);
            }
            for x in [cx - 4, cx + 2] {
                c.hline(x, x + 2, 16, ramp.at(Tone::Shade), 3);
            }
        }
        1 => {
            c.line((cx - 1, 3), (cx - 2, 6), ramp.at(Tone::Lift), 1, 3);
            for y in [7, 10] {
                c.tint(cx + 1, y, ramp, Tone::Light);
            }
            for x in [cx - 4, cx + 2] {
                c.hline(x, x + 2, 10, ramp.at(Tone::Lift), 3);
                c.hline(x, x + 2, 11, ramp.at(Tone::Shade), 3);
            }
        }
        2 => {
            for y in [8, 11] {
                c.tint(cx + 1, y, ramp, Tone::Light);
            }
            for x in [cx - 4, cx + 2] {
                c.hline(x, x + 2, 11, ramp.at(Tone::Shade), 3);
            }
        }
        _ => {
            // Raglan seams, the belt and its buckle.
            c.line((cx - 1, 3), (cx - 4, 7), ramp.at(Tone::Mid), 1, 3);
            c.line((cx + 1, 3), (cx + 4, 7), ramp.at(Tone::Shade), 1, 3);
            for y in [8, 13] {
                c.tint(cx - 2, y, ramp, Tone::Light);
            }
            c.fill_normal(Rect::new(cx - 5, 11, 11, 1), ramp.at(Tone::Shade), face(), 4);
            c.hline(cx - 5, cx + 5, 10, ramp.at(Tone::Lift), 4);
            c.dot(cx - 1, 11, Ramp::Iron.at(Tone::Light), 5);
        }
    }
    // The sleeves, hanging at the sides in front of the body.
    for side in [-1, 1] {
        let pts = if side < 0 {
            [(cx - 4, 5), (cx - 3, 6), (cx - 3, cuff), (cx - 6, cuff), (cx - 6, 8)]
        } else {
            [(cx + 4, 5), (cx + 3, 6), (cx + 3, cuff), (cx + 6, cuff), (cx + 6, 8)]
        };
        c.polygon_cloth(&pts, ramp, 60, Z::new(5, 6));
        let (lo, hi) = if side < 0 { (cx - 6, cx - 3) } else { (cx + 3, cx + 6) };
        for x in lo..=hi {
            c.tint(x, cuff - 1, ramp, Tone::Mid);
        }
        // The fold where the sleeve hangs against the body, in shadow down its inner side.
        let inner = if side < 0 { cx - 3 } else { cx + 3 };
        for y in 8..cuff - 1 {
            c.tint(inner, y, ramp, if side < 0 { Tone::Deep } else { Tone::Mid });
        }
    }
    if cut == 2 {
        // The donkey jacket's yoke: leather over both shoulders and down the sleeves' heads.
        for y in 2..=6 {
            for x in 0..w {
                if let Some((r, t)) = Ramp::of(c.get(x, y)) {
                    if r == ramp {
                        c.recolour(x, y, Ramp::Leather.at(t.step(-1)));
                    }
                }
            }
        }
    }
    // The hem hangs uneven.
    c.clear_px(cx + 5 + flare, hem);
    c.declutter(ramp);
    c.despike();
    c
}

/// An empty birdcage on a hook: a domed top, the bars, a tray, its door hung open.
fn cage() -> Canvas {
    let (w, h) = (10, 14);
    let mut c = Canvas::new(w, h);
    let brass = Ramp::Brass;
    let cx = 4;
    // The ring it hangs by.
    c.dot(cx, 0, brass.at(Tone::Light), 3);
    c.dot(cx - 1, 1, brass.at(Tone::Base), 3);
    c.dot(cx + 1, 1, brass.at(Tone::Shade), 3);
    c.dot(cx, 2, brass.at(Tone::Base), 3);
    // The dome.
    c.polyline(
        &[(cx - 4, 6), (cx - 3, 4), (cx - 1, 3), (cx + 1, 3), (cx + 3, 4), (cx + 4, 6)],
        brass.at(Tone::Base),
        1,
        3,
    );
    c.dot(cx - 2, 4, brass.at(Tone::High), 3);
    c.dot(cx - 1, 3, brass.at(Tone::Light), 3);
    c.dot(cx + 3, 4, brass.at(Tone::Shade), 3);
    // The bars: lit on the light's side, dark on the far one; the back bars dimmer between.
    for (x, t) in
        [(cx - 4, Tone::Light), (cx - 2, Tone::Base), (cx, Tone::Mid), (cx + 2, Tone::Shade), (cx + 4, Tone::Deep)]
    {
        c.vline(x, 6, 11, brass.at(t), 3);
    }
    for x in [cx - 3, cx - 1, cx + 1, cx + 3] {
        c.vline(x, 7, 10, brass.at(Tone::Deep), 2);
    }
    c.dot(cx - 4, 7, brass.at(Tone::High), 3);
    // The perch across, the hoop at its middle.
    c.hline(cx - 3, cx + 3, 8, Ramp::WoodDark.at(Tone::Base), 3);
    c.hline(cx - 4, cx + 4, 6, brass.at(Tone::Light), 3);
    // The tray.
    c.hline(cx - 4, cx + 4, 12, brass.at(Tone::Light), 3);
    c.hline(cx - 4, cx + 4, 13, brass.at(Tone::Shade), 3);
    c.dot(cx + 4, 12, brass.at(Tone::Base), 3);
    // The door, swung out on its hinge to the right, open.
    c.vline(cx + 5, 8, 11, brass.at(Tone::Base), 3);
    c.dot(w - 1, 9, brass.at(Tone::Shade), 3);
    c.dot(w - 1, 10, brass.at(Tone::Shade), 3);
    c
}

/// A hand bell hung by its handle on a cord: a turned wooden handle, the brass bell flaring to
/// its lip, the clapper under it.
fn hand_bell() -> Canvas {
    let (w, h) = (7, 14);
    let mut c = Canvas::new(w, h);
    let cx = 3;
    c.vline(cx, 0, 3, Ramp::ClothBrown.at(Tone::Base), 3);
    c.disc_lit(cx, 5, 1, Ramp::WoodDark, Z::flat(4));
    c.vline(cx, 6, 7, Ramp::WoodDark.at(Tone::Base), 4);
    c.polygon_lit(
        &[(cx - 1, 8), (cx + 1, 8), (cx + 2, 10), (cx + 3, 12), (cx - 3, 12), (cx - 2, 10)],
        Ramp::Brass,
        110,
        Z::new(3, 4),
    );
    c.hline(cx - 3, cx + 3, 12, Ramp::Brass.at(Tone::Light), 4);
    c.dot(cx - 1, 9, Ramp::Brass.at(Tone::Glint), 4);
    c.dot(cx, 13, Ramp::Iron.at(Tone::Base), 3);
    c
}

/// A Company tag on a chain: a few links, a brass plate with its number struck in.
fn tag_chain() -> Canvas {
    let (w, h) = (6, 11);
    let mut c = Canvas::new(w, h);
    let cx = 3;
    for y in 0..5 {
        let t = if y % 2 == 0 { Tone::Light } else { Tone::Base };
        c.dot(cx + i32::from(y % 2 == 1) - 1, y, Ramp::Iron.at(t), 3);
        c.dot(cx, y, Ramp::Iron.at(t.step(-1)), 3);
    }
    c.rect_bevel(Rect::new(0, 5, 6, 6), Ramp::Brass, 1, Z::new(3, 4));
    c.dot(cx - 1, 6, Ix::SEAM, 4);
    for (x, y) in [(2, 8), (3, 8), (2, 9), (4, 9)] {
        c.dot(x, y, Ramp::Brass.at(Tone::Shade), 4);
    }
    c
}

/// A strip of cloth tied on: a knot and one rag hanging from it, ragged.
fn strip(variant: u8) -> Canvas {
    // One rag a strip, never a pair of tails (a pair hangs like legs): `0` a long pale rag
    // twisting once, `1` a short wide grey one frayed at its end, `2` a thin brown ribbon.
    let v = usize::from(variant);
    let ramp = [Ramp::ClothCream, Ramp::ClothGrey, Ramp::ClothBrown][v];
    let (w, h) = (6, [12, 8, 11][v]);
    let mut c = Canvas::new(w, h);
    let seed = 0x7374 + u32::from(variant);
    // The knot round what it is tied to, and its short end sticking out.
    c.rect_bevel(Rect::new(1, 0, 3, 2), ramp, 1, Z::new(4, 5));
    c.dot(4, 1, ramp.at(Tone::Shade), 5);
    let wide = [3, 3, 2][v];
    c.begin();
    for y in 2..h {
        let r = y - 2;
        // The rag hangs from the knot's left, drifting right as it falls; a twist shows its
        // shaded back a row.
        let x0 = 1 + (r * [1, 1, 2][v]) / [5, 6, 5][v];
        let width = if r * 3 > (h - 2) * 2 { wide - 1 } else { wide };
        let twist = v == 0 && r == 5;
        for d in 0..width.max(1) {
            let t = if twist {
                Tone::Shade
            } else if d == 0 {
                Tone::Light
            } else if d == width - 1 {
                Tone::Mid
            } else {
                Tone::Base
            };
            px(&mut c, x0 + d, y, ramp, t, face(), 3);
        }
    }
    // A frayed end.
    let x_end = 1 + ((h - 3) * [1, 1, 2][v]) / [5, 6, 5][v];
    if hs(seed, 1, 1) % 2 == 0 || v == 1 {
        c.clear_px(x_end + 1, h - 1);
    }
    c
}

// ---------------------------------------------------------------------------------------------
// Chalk.

/// A chalk stroke from `a` to `b`: pressed hard in the middle, thinning to a dusty tail.
fn chalk(c: &mut Canvas, a: (i32, i32), b: (i32, i32), seed: u32) {
    let mut pts = Vec::new();
    bresenham(a.0, a.1, b.0, b.1, |x, y| pts.push((x, y)));
    let n = pts.len();
    c.begin();
    for (i, &(x, y)) in pts.iter().enumerate() {
        let t = if i + 1 == n && hs(seed, x, y) % 2 == 0 {
            Tone::Shade
        } else if i == 0 || i + 1 == n {
            Tone::Mid
        } else {
            Tone::Base
        };
        px(c, x, y, Ramp::Limewash, t, FLAT, 1);
    }
}

/// A chalk register: groups of five, four strokes and a bar across them, a little wobbly.
fn tally(groups: u8) -> Canvas {
    let g = i32::from(groups.max(1));
    let mut c = Canvas::new(9 * g, 8);
    for k in 0..g {
        let gx = 1 + 9 * k;
        for s in 0..4 {
            let h = hs(0x7461 + k as u32, s, 1);
            let x = gx + 2 * s;
            let top = 1 + (h % 2) as i32;
            let lean = ((h >> 4) % 3) as i32 - 1;
            let foot = x - i32::from((h >> 9) % 4 == 0 && lean <= 0);
            chalk(&mut c, (x + lean.max(0), top), (foot, 6), h);
        }
        let h = hs(0x6261, k, 2);
        let y0 = 4 + (h % 2) as i32;
        chalk(&mut c, (gx - 1, y0 + 1), (gx + 7, y0 - 3), h);
    }
    c
}

/// "9" in chalk on the road, the hour the bell rings, drawn in one hurried stroke.
fn nine() -> Canvas {
    let mut c = Canvas::new(6, 8);
    let rows = [".###..", "#...#.", "#...#.", ".####.", "....#.", "....#.", "...#..", ".##..."];
    c.begin();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                // Pressed hard where the stroke turned, the chalk thinning on the tail.
                let t = match (x, y) {
                    (4, 3) | (0, 1) => Tone::Light,
                    (1 | 2, 7) => Tone::Mid,
                    _ => Tone::Base,
                };
                px(&mut c, x as i32, y as i32, Ramp::Limewash, t, FLAT, 1);
            }
        }
    }
    c
}

/// Julie's ring of chalk round a lamp's post at the height of her hand, seen from above and in
/// front: the back of it dim and broken where the post stands in front of it, the front bright.
fn ring() -> Canvas {
    let mut c = Canvas::new(8, 3);
    c.begin();
    for (x, y, t) in [
        (1, 0, Tone::Mid),
        (2, 0, Tone::Base),
        (5, 0, Tone::Base),
        (6, 0, Tone::Mid),
        (0, 1, Tone::Base),
        (7, 1, Tone::Mid),
        (1, 2, Tone::Light),
        (2, 2, Tone::High),
        (3, 2, Tone::High),
        (4, 2, Tone::High),
        (5, 2, Tone::Light),
        (6, 2, Tone::Base),
    ] {
        px(&mut c, x, y, Ramp::Limewash, t, FLAT, 2);
    }
    c
}

// ---------------------------------------------------------------------------------------------
// Paper and iron.

/// A Company notice pasted at its corners over the council's: the old one yellowed and torn,
/// peeking out round it; the new one clean, a bold heading, its lines, the Company's stamp in
/// oxblood. `0` the old one shows below and to the left; `1` above and to the right, the new
/// one's corner come unstuck.
fn notice(variant: u8) -> Canvas {
    let (w, h) = (13, 15);
    let mut c = Canvas::new(w, h);
    let seed = 0x6e6f + u32::from(variant);
    let n = face();
    let cream = Ramp::ClothCream;
    let (old, new) = if variant == 0 {
        (Rect::new(0, 3, 11, 12), Rect::new(2, 0, 11, 13))
    } else {
        (Rect::new(2, 0, 11, 12), Rect::new(0, 2, 11, 13))
    };
    // The old notice: yellowed, its exposed edges torn.
    c.begin();
    for y in old.y..old.bottom() {
        for x in old.x..old.right() {
            let edge = x == old.x || y == old.bottom() - 1 || x == old.right() - 1 || y == old.y;
            if edge && hs(seed, x, y) % 3 == 0 {
                continue;
            }
            px(&mut c, x, y, cream, if y == old.y { Tone::Base } else { Tone::Mid }, n, 2);
        }
    }
    writing(&mut c, old.x + 1, old.y + 2, old.w - 2, 5, Ramp::ClothCream.at(Tone::Deep), seed, 2);
    // The new one over it.
    c.begin();
    for y in new.y..new.bottom() {
        for x in new.x..new.right() {
            let t = if y == new.y || x == new.x {
                Tone::High
            } else if (x == new.x + 1 || x == new.right() - 2) && (y == new.y + 1 || y == new.bottom() - 2) {
                // Where the paste soaked through at a corner.
                Tone::Lift
            } else {
                Tone::Light
            };
            px(&mut c, x, y, cream, t, n, 4);
        }
    }
    // Heading, a rule, the lines, the stamp.
    c.fill_rect(Rect::new(new.x + 2, new.y + 2, new.w - 4, 2), Ix::SEAM, 4);
    c.hline(new.x + 2, new.right() - 3, new.y + 5, cream.at(Tone::Shade), 4);
    writing(&mut c, new.x + 2, new.y + 7, new.w - 4, 3, Ix::SEAM, seed ^ 9, 4);
    let (sx, sy) = (new.right() - 5, new.bottom() - 5);
    for (x, y) in [(1, 0), (2, 0), (0, 1), (3, 1), (0, 2), (3, 2), (1, 3), (2, 3)] {
        c.dot(sx + x, sy + y, Ramp::Oxblood.at(Tone::Base), 4);
    }
    c.dot(sx + 1, sy + 1, Ramp::Oxblood.at(Tone::Light), 4);
    c.dot(sx + 2, sy + 2, Ramp::Oxblood.at(Tone::Shade), 4);
    if variant == 1 {
        // The top right corner come unstuck and folded over: its back in shade.
        let (x1, y0) = (new.right() - 1, new.y);
        for (x, y) in [(x1, y0), (x1 - 1, y0), (x1, y0 + 1)] {
            c.clear_px(x, y);
        }
        c.dot(x1 - 2, y0, cream.at(Tone::Shade), 5);
        c.dot(x1 - 1, y0 + 1, cream.at(Tone::Mid), 5);
        c.dot(x1 - 2, y0 + 1, cream.at(Tone::Base), 5);
    }
    c
}

/// A length of pipe along a wall, `x0..=x1` on rows `y..y + 3`: lit along its top, shaded under.
fn pipe_run(c: &mut Canvas, x0: i32, x1: i32, y: i32, z: u8) {
    c.begin();
    let n = [normal(0, -70), normal(0, 30), normal(0, 100)];
    for x in x0..=x1 {
        for (k, t) in [Tone::Light, Tone::Base, Tone::Shade].into_iter().enumerate() {
            px(c, x, y + k as i32, Ramp::Iron, t, n[k], z);
        }
    }
}

/// The same pipe running down, `y0..=y1` on columns `x..x + 3`.
fn pipe_down(c: &mut Canvas, x: i32, y0: i32, y1: i32, z: u8) {
    c.begin();
    let n = [normal(-70, 0), normal(0, 30), normal(80, 0)];
    for y in y0..=y1 {
        for (k, t) in [Tone::Light, Tone::Base, Tone::Shade].into_iter().enumerate() {
            px(c, x + k as i32, y, Ramp::Iron, t, n[k], z);
        }
    }
}

/// A bracket strapping a pipe to the wall at column `x` over rows `y0..=y1`, its bolt's head lit,
/// rust under it.
fn bracket(c: &mut Canvas, x: i32, y0: i32, y1: i32, z: u8) {
    c.fill_normal(Rect::new(x, y0, 2, y1 - y0 + 1), Ramp::Iron.at(Tone::Mid), face(), z);
    c.vline(x, y0, y1, Ramp::Iron.at(Tone::Lift), z);
    c.dot(x, y0, Ramp::Iron.at(Tone::High), z);
    if c.get(x + 1, y1 + 1).is_opaque() || y1 + 1 < c.h() {
        c.dot(x + 1, y1 + 1, Ramp::Copper.at(Tone::Shade), z);
    }
}

/// A pipe run: `0` straight, a flanged joint and two brackets; `1` an elbow turning down the
/// wall; `2` a valve, its wheel painted the Company's oxblood.
fn pipe(variant: u8) -> Canvas {
    match variant {
        0 => {
            let mut c = Canvas::new(18, 6);
            pipe_run(&mut c, 0, 17, 1, 2);
            c.rect_lit(Rect::new(8, 0, 2, 5), Ramp::Iron, 4);
            c.dot(9, 4, Ramp::Iron.at(Tone::Deep), 4);
            for x in [3, 14] {
                bracket(&mut c, x, 0, 4, 4);
            }
            // Rust weeping from the joint, and a scab of it along the pipe's belly.
            c.dot(9, 5, Ramp::Copper.at(Tone::Mid), 4);
            c.dot(8, 5, Ramp::Copper.at(Tone::Shade), 4);
            for x in [11, 12] {
                c.tint(x, 3, Ramp::Iron, Tone::Shade);
                c.recolour(x, 3, Ramp::Copper.at(Tone::Shade));
            }
            c
        }
        1 => {
            let mut c = Canvas::new(13, 15);
            pipe_run(&mut c, 0, 7, 1, 2);
            pipe_down(&mut c, 8, 4, 14, 2);
            // The elbow: the bend's outer curve lit, the inner in shade.
            c.begin();
            let n = normal(50, -50);
            for (x, y, t) in [
                (8, 1, Tone::Light),
                (9, 1, Tone::Light),
                (8, 2, Tone::Base),
                (9, 2, Tone::Base),
                (10, 2, Tone::Light),
                (8, 3, Tone::Shade),
                (9, 3, Tone::Base),
                (10, 3, Tone::Base),
                (8, 4, Tone::Light),
            ] {
                px(&mut c, x, y, Ramp::Iron, t, n, 2);
            }
            c.rect_lit(Rect::new(7, 9, 5, 2), Ramp::Iron, 4);
            bracket(&mut c, 3, 0, 4, 4);
            c.dot(11, 11, Ramp::Copper.at(Tone::Shade), 4);
            c.dot(10, 12, Ramp::Copper.at(Tone::Mid), 4);
            c
        }
        _ => {
            let mut c = Canvas::new(18, 10);
            pipe_run(&mut c, 0, 17, 5, 2);
            // The valve's body over the pipe, its spindle, the wheel on top seen from above.
            c.rect_bevel(Rect::new(6, 4, 6, 5), Ramp::Iron, 1, Z::new(3, 4));
            c.vline(9, 2, 3, Ramp::Iron.at(Tone::Light), 5);
            let wheel = Ramp::Oxblood;
            c.begin();
            for (x, y, t) in [
                (7, 0, Tone::Light),
                (8, 0, Tone::Light),
                (9, 0, Tone::High),
                (10, 0, Tone::Light),
                (11, 0, Tone::Base),
                (6, 1, Tone::Light),
                (12, 1, Tone::Shade),
                (7, 2, Tone::Base),
                (8, 2, Tone::Shade),
                (10, 2, Tone::Shade),
                (11, 2, Tone::Shade),
                (9, 1, Tone::Base),
            ] {
                px(&mut c, x, y, wheel, t, FLAT, 7);
            }
            for x in [3, 15] {
                bracket(&mut c, x, 4, 8, 4);
            }
            c.dot(8, 9, Ramp::Copper.at(Tone::Shade), 4);
            c
        }
    }
}

/// A grate set flush in the road: `0` a square of bars in a frame, `1` a drain's bars across the
/// gutter. The dark under it is the dark of the drain.
fn grate(variant: u8) -> Canvas {
    let (w, h) = if variant == 0 { (12, 12) } else { (14, 8) };
    let mut c = Canvas::new(w, h);
    c.begin();
    for y in 0..h {
        for x in 0..w {
            let frame = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            let ix = if frame {
                let t = if x == 0 || y == 0 { Tone::Light } else { Tone::Base };
                Ramp::Iron.at(t)
            } else if variant == 0 {
                match (x % 3 == 0, y % 3 == 0) {
                    (true, true) => Ramp::Iron.at(Tone::Lift),
                    (true, false) => Ramp::Iron.at(Tone::Base),
                    (false, true) => Ramp::Iron.at(Tone::Light),
                    _ => Ix::SEAM,
                }
            } else if x % 2 == 1 {
                Ramp::Iron.at(if y == 1 { Tone::Light } else { Tone::Base })
            } else if y == h / 2 {
                Ramp::Iron.at(Tone::Mid)
            } else {
                Ix::SEAM
            };
            c.put(x, y, ix, FLAT, 1);
        }
    }
    // Rust round the bars where the water stands.
    for (x, y) in [(4, h - 2), (7, h - 2), (w - 2, 3)] {
        c.recolour(x, y, Ramp::Copper.at(Tone::Shade));
    }
    c
}

/// ROAD CLOSED BY ORDER OF THE COMPANY: a striped plank on two trestles, a rail under it, the
/// order on a board wired over it. Two cells wide; it stands on its foot row.
fn barrier() -> Canvas {
    let (w, h) = (32, 22);
    let mut c = Canvas::new(w, h);
    let foot = h - 1;
    let top = 9;
    c.ao_contact(Rect::new(-1, foot - 2, w + 2, 4), 1);
    // The trestles: a front leg to the foot, a back leg splayed away behind, a brace.
    for x in [4, 25] {
        c.line((x + 2, top + 2), (x + 3, foot - 3), Ramp::WoodDark.at(Tone::Shade), 2, 2);
        c.line((x + 1, top + 2), (x, foot), Ramp::WoodDark.at(Tone::Base), 2, 4);
        c.vline(x, top + 4, foot - 1, Ramp::WoodDark.at(Tone::Light), 4);
    }
    // The lower rail.
    board(&mut c, 3, 28, top + 7, (0, 1), 2, Ramp::Deadwood, 0xba, 5);
    // The plank: striped on the slant, oxblood and white.
    c.begin();
    for y in top..top + 4 {
        for x in 0..w {
            let red = (x + y) / 4 % 2 == 0;
            let ramp = if red { Ramp::Oxblood } else { Ramp::Limewash };
            let t = match y - top {
                0 => Tone::Light,
                3 => Tone::Shade,
                _ => Tone::Base,
            };
            let t = if red && y - top == 0 { Tone::High } else { t };
            px(&mut c, x, y, ramp, t, face(), 6);
        }
    }
    // The order: a board wired over the plank's middle, its heading in oxblood.
    let b = Rect::new(10, 1, 12, 8);
    c.vline(12, b.bottom(), top, Ramp::Iron.at(Tone::Base), 7);
    c.vline(19, b.bottom(), top, Ramp::Iron.at(Tone::Base), 7);
    c.begin();
    for y in b.y..b.bottom() {
        for x in b.x..b.right() {
            let t = if y == b.y {
                Tone::High
            } else if x == b.right() - 1 || y == b.bottom() - 1 {
                Tone::Mid
            } else {
                Tone::Light
            };
            px(&mut c, x, y, Ramp::ClothCream, t, face(), 8);
        }
    }
    c.hline(b.x + 2, b.right() - 3, b.y + 2, Ramp::Oxblood.at(Tone::Shade), 8);
    writing(&mut c, b.x + 2, b.y + 4, b.w - 4, 2, Ix::SEAM, 0xb0, 8);
    c
}

/// A stain: the contact shade alone, darkening what is under it (ART.md §3.1). `0` a spread pool,
/// `1` a splash with a trail.
fn stain(variant: u8) -> Canvas {
    let (w, h) = if variant == 0 { (12, 7) } else { (10, 8) };
    let mut c = Canvas::new(w, h);
    let lobes: &[(i32, i32, i32, i32)] = if variant == 0 {
        &[(0, 1, 8, 5), (4, 0, 7, 5), (6, 3, 6, 4)]
    } else {
        &[(0, 3, 6, 5), (5, 0, 3, 4), (7, 2, 3, 2), (2, 1, 2, 2)]
    };
    let mut m = Canvas::new(w, h);
    for &(x, y, lw, lh) in lobes {
        m.ellipse(Rect::new(x, y, lw, lh), Ix::INK, 1);
    }
    c.begin();
    for y in 0..h {
        for x in 0..w {
            if m.get(x, y).is_opaque() {
                c.put(x, y, Ix::AO, FLAT, 0);
            }
        }
    }
    c
}

/// A chain across barn doors between two staples, sagging, a padlock on it and a Company tag
/// hung from the lock's shackle.
fn chain() -> Canvas {
    let (w, h) = (17, 13);
    let mut c = Canvas::new(w, h);
    let iron = Ramp::Iron;
    // The staples.
    for x in [0, w - 2] {
        c.rect_lit(Rect::new(x, 0, 2, 3), iron, 3);
    }
    // The links: alternate flat and edge-on, along a sag.
    let mid = w / 2;
    for x in 1..w - 1 {
        let d = x - mid;
        let y = 1 + (mid * mid - d * d) * 4 / (mid * mid);
        let t = if x % 2 == 0 { Tone::Light } else { Tone::Shade };
        c.dot(x, y, iron.at(t), 3);
        if x % 2 == 0 {
            c.dot(x, y + 1, iron.at(Tone::Deep), 3);
        }
    }
    // The padlock, outlined on its own and laid over the chain.
    let mut lock = Canvas::new(7, 8);
    lock.polyline(&[(1, 3), (1, 1), (2, 0), (4, 0), (5, 1), (5, 3)], iron.at(Tone::Light), 1, 3);
    lock.rect_bevel(Rect::new(0, 3, 7, 5), iron, 1, Z::new(3, 4));
    lock.dot(3, 5, Ix::SEAM, 5);
    lock.dot(1, 4, iron.at(Tone::High), 5);
    lock.outline();
    c.stamp(&lock, mid - 3, 4);
    // The tag on its ring from the shackle.
    let mut tag = Canvas::new(4, 4);
    tag.rect_bevel(Rect::new(0, 0, 4, 4), Ramp::Brass, 1, Z::new(4, 5));
    tag.dot(1, 1, Ix::SEAM, 5);
    tag.outline();
    c.stamp(&tag, mid + 3, 7);
    c.dot(mid + 3, 6, Ramp::Brass.at(Tone::Base), 5);
    c
}

/// A Company tag nailed to a door: a brass plate, its number struck in, a nail at each end.
fn door_tag() -> Canvas {
    let mut c = Canvas::new(8, 6);
    c.rect_bevel(Rect::new(0, 0, 8, 6), Ramp::Brass, 1, Z::new(2, 3));
    for (x, y) in [(2, 2), (3, 2), (4, 2), (2, 3), (4, 3), (5, 3)] {
        c.dot(x, y, Ramp::Brass.at(Tone::Shade), 3);
    }
    for x in [1, 6] {
        c.dot(x, 1, Ramp::Iron.at(Tone::High), 4);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    /// The palette indices `c` uses but clear, AO and the two inks.
    fn colours(c: &Canvas) -> BTreeSet<u16> {
        c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0).collect()
    }

    #[test]
    fn every_look_draws_and_holds_the_layer_contract() {
        for l in all() {
            let c = render(l);
            c.validate().unwrap_or_else(|e| panic!("{l:?}: {e}"));
            let drawn = match l {
                Look::Stain { .. } => c.albedo().contains(&Ix::AO),
                _ => c.albedo().iter().any(|a| a.is_opaque()),
            };
            assert!(drawn, "{l:?} draws nothing");
            assert!(c.emissive().iter().all(|&e| e == Ix::CLEAR), "{l:?} glows");
            let (ax, ay) = anchor(l);
            assert!(ax >= 0 && ay >= 0 && ax < c.w() && ay < c.h(), "{l:?}: anchor ({ax}, {ay}) off its canvas");
        }
    }

    #[test]
    fn window_covers_are_the_opening_and_a_px_round() {
        for l in all() {
            if let Look::Boards { opening, .. }
            | Look::Brick { opening, .. }
            | Look::Card { opening }
            | Look::Crack { opening, .. } = l
            {
                let c = render(l);
                let (w, h) = opening.size();
                assert_eq!((c.w(), c.h()), (w + 2, h + 2), "{l:?}");
                assert_eq!(Opening::of_size(w, h), Some(opening));
            }
        }
    }

    #[test]
    fn neighbouring_board_layouts_differ() {
        for o in Opening::ALL {
            let hashes: BTreeSet<u32> =
                (0..3).map(|layout| render(Look::Boards { layout, opening: o }).hash()).collect();
            assert_eq!(hashes.len(), 3, "{o:?}");
        }
    }

    #[test]
    fn the_kit_packs_into_one_page() {
        // Shelves, tallest first, a px of gutter round each.
        let mut sizes: Vec<(i32, i32)> = all().into_iter().map(render).map(|c| (c.w() + 1, c.h() + 1)).collect();
        sizes.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
        let (mut x, mut y, mut shelf) = (0, 0, 0);
        for (w, h) in sizes {
            if x + w > 256 {
                x = 0;
                y += shelf;
                shelf = 0;
            }
            x += w;
            shelf = shelf.max(h);
        }
        assert!(y + shelf <= 256, "the night kit needs {} rows of a 256 page", y + shelf);
    }

    #[test]
    fn colour_budgets() {
        let mut kit = BTreeSet::new();
        for l in all() {
            let used = colours(&render(l));
            assert!(used.len() <= 64, "{l:?} uses {} colours", used.len());
            kit.extend(used);
        }
        assert!(kit.len() < 200, "the night kit uses {} colours", kit.len());
    }

    #[test]
    fn stains_only_darken() {
        for v in 0..2 {
            let c = render(Look::Stain { variant: v });
            assert!(c.albedo().iter().all(|&a| a == Ix::CLEAR || a == Ix::AO), "stain {v} paints");
        }
    }

    #[test]
    fn deterministic() {
        for l in all() {
            assert_eq!(render(l), render(l), "{l:?}");
        }
    }

    #[test]
    fn hung_things_stand_down_from_their_anchor() {
        for l in all().into_iter().filter(|&l| !flat(l)) {
            let c = render(l);
            for y in 1..c.h() {
                for x in 0..c.w() {
                    if c.get(x, y).is_opaque() && c.get(x, y - 1).is_opaque() {
                        assert!(c.height_at(x, y) <= c.height_at(x, y - 1), "{l:?} at ({x}, {y})");
                    }
                }
            }
        }
    }

    #[test]
    fn outline_closed_and_selective() {
        for l in all().into_iter().filter(|&l| outlined(l)) {
            let c = render(l);
            for y in 0..c.h() {
                for x in 0..c.w() {
                    let ix = c.get(x, y);
                    let open = |dx: i32, dy: i32| !c.get(x + dx, y + dy).is_opaque();
                    let (away, lit) = (open(1, 0) || open(0, 1), open(-1, 0) || open(0, -1));
                    if !ix.is_opaque() || !(away || lit) || ix == Ix::INK {
                        continue;
                    }
                    let Some((_, t)) = Ramp::of(ix) else { panic!("{l:?}: ({x}, {y}) is an edge in {ix:?}") };
                    if away {
                        assert_eq!(t, Tone::Deep, "{l:?}: ({x}, {y}) faces away in {t:?}");
                    } else {
                        assert!(t <= Tone::Base, "{l:?}: ({x}, {y}) a lit edge in {t:?}");
                    }
                }
            }
        }
    }
}
