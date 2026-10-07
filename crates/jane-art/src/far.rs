//! The far landmarks (ART.md §2.8; PLAY-PLAN.md Phase 5, the world audit §3 on Lynch): a shape
//! of its own on the horizon for each region, beside the School, so a place she cannot see yet
//! has a bearing. The Works' chimney with its plume, St Anne's spire over the Lowfields, and the
//! lake's statue on its plinth in the Waters. Each is a silhouette in three distance bands, as
//! the School is, its sky-side edge catching the light; the foot is its bottom row.
//!
//! Integer throughout, like the rest of the crate; every piece a function of its arguments.

use alloc::vec::Vec;
use jane_core::grid::Rect;

use crate::canvas::Canvas;
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone};

/// Frames of the chimney's plume, played in turn.
pub const PLUME_FRAMES: u8 = 6;

mod salt {
    pub const PLUME: u32 = 0x504c_554d;
    pub const YEWS: u32 = 0x5945_5753;
}

/// px a unit in `band` (0 far, 1 middle, 2 near).
const fn unit(band: u8) -> i32 {
    2 + if band > 2 { 2 } else { band as i32 }
}

/// The 4 x 4 Bayer threshold at a px, 0..=15.
fn bayer(x: i32, y: i32) -> i32 {
    const B: [i32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    B[((y & 3) * 4 + (x & 3)) as usize]
}

/// A disc of `ix` at `(cx, cy)` radius `r`, only where the Bayer threshold is under `fill` of 16.
fn disc_dither(c: &mut Canvas, cx: i32, cy: i32, r: i32, ix: Ix, fill: i32, z: u8) {
    for y in -r..=r {
        for x in -r..=r {
            if x * x + y * y <= r * r + r / 2 && bayer(cx + x, cy + y) < fill {
                c.dot(cx + x, cy + y, ix, z);
            }
        }
    }
}

/// The Works' chimney (the Factory's stack) with its plume, frame `frame` of [`PLUME_FRAMES`]:
/// a tall tapering brick stack with a corbelled cap and two iron bands, a shorter stack beside
/// it, over the saw-tooth roofs of the shops. `night` lights the north-light glazing dimly and
/// puts the furnace's glow under the plume (the night shift is working).
pub fn chimney(band: u8, night: bool, frame: u8) -> Canvas {
    let u = unit(band);
    let mut c = chimney_at(u, night);
    let mouth = (14 * u, c.h() - 1 - 45 * u);
    plume_into(&mut c, u, night, frame, mouth, false);
    c
}

/// The world chimney's mouth: px across from its canvas's left edge, and up from its foot.
pub const WORLD_MOUTH: (i32, i32) = (28, 434);
/// The world plume's unit, px.
pub const PLUME_U: i32 = 8;
/// The world plume's mouth in its own canvas: `(x, rows over the bottom)`.
pub const PLUME_MOUTH: (i32, i32) = (2 * PLUME_U, 2 * PLUME_U);
/// Where each world landmark's foot is across its canvas, px: the chimney's, the spire's, the
/// statue's.
pub const WORLD_FEET: [i32; 3] = [WORLD_MOUTH.0, 36, 11 * STATUE_U];
/// The world statue's unit, px.
const STATUE_U: i32 = 12;

/// The px at `(x, y)` a tone darker if it is drawn in `ramp`: a mortar course, a slate course.
fn darken(c: &mut Canvas, ramp: Ramp, x: i32, y: i32) {
    if let Some((r, t)) = Ramp::of(c.get(x, y))
        && r == ramp
    {
        let t = match t {
            Tone::High | Tone::Light => Tone::Base,
            Tone::Lift | Tone::Base => Tone::Mid,
            Tone::Mid => Tone::Shade,
            _ => Tone::Deep,
        };
        c.recolour(x, y, ramp.at(t));
    }
}

/// The volume over `mask` in `ramp`, turned as a body is (`Canvas::inflate`) and its shading
/// cut to the material's few tones.
fn solid(c: &mut Canvas, mask: &Canvas, ramp: Ramp, radius: i32, z: crate::canvas::Z) {
    c.inflate(mask, ramp, radius, z);
    c.retone(ramp, [Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::High]);
}

/// The Factory's great stack as it stands in the county, a real thing among the Works' walls: a
/// brick column on a plinth, rounded by the light, tapering from 34 px to 24 over 380, its mortar
/// courses, two iron bands, a soot-black head under a stone cap, and by night the furnace in its
/// mouth. No plume (that is [`plume_world`], laid over the mouth, so it can rise high and drift by
/// frame without a frame of the whole stack). Its foot is the bottom row, `WORLD_FEET[0]` across.
pub fn chimney_world(night: bool) -> Canvas {
    let (w, h) = (56, 450);
    let f = h - 1;
    let cx = WORLD_MOUTH.0;
    let mut m = Canvas::new(w, h);
    // The plinth, then the shaft, then the cap's corbel.
    m.fill_rect(Rect::new(cx - 20, f - 40, 40, 41), Ix::INK, 1);
    for y in (f - 422)..(f - 40) {
        let k = (f - 40 - y) * 256 / 382;
        let half = (17 * 256 - 5 * k) / 256;
        m.hline(cx - half, cx + half - 1, y, Ix::INK, 1);
    }
    let mut c = Canvas::new(w, h);
    solid(&mut c, &m, Ramp::Brick, 7, crate::canvas::Z::new(2, 12));
    // Works brick: a century of the Works' smoke has darkened all of it a tone.
    for y in 0..h {
        for x in 0..w {
            darken(&mut c, Ramp::Brick, x, y);
        }
    }
    // Mortar: a darker course every fourth row, broken by the bond.
    for y in (f - 420..f - 2).step_by(4) {
        for x in 0..w {
            if m.get(x, y).is_opaque() && (x + (y / 4) * 3) % 7 != 0 {
                darken(&mut c, Ramp::Brick, x, y);
            }
        }
    }
    // The plinth's coping: a stone course over it, lit on its top.
    c.rect_lit(Rect::new(cx - 22, f - 44, 44, 5), Ramp::Stone, 10);
    // Two iron bands round the shaft.
    for by in [f - 150, f - 285] {
        let half = (0..w).filter(|&x| m.get(x, by).is_opaque()).count() as i32 / 2 + 1;
        for x in cx - half..cx + half {
            let t = if x < cx - half / 3 {
                Tone::Light
            } else if x < cx + half / 2 {
                Tone::Base
            } else {
                Tone::Shade
            };
            c.dot(x, by, Ramp::Iron.at(t), 12);
            c.dot(x, by + 1, Ramp::Iron.at(Tone::Shade), 12);
        }
    }
    // Soot: the head blackened, thinning down the shaft by the Bayer pattern.
    for y in (f - 422)..(f - 350) {
        let k = (y - (f - 422)) * 16 / 72;
        for x in 0..w {
            if m.get(x, y).is_opaque() && bayer(x, y) >= k {
                darken(&mut c, Ramp::Brick, x, y);
                darken(&mut c, Ramp::Brick, x, y);
            }
        }
    }
    // The cap: a stone corbel two courses deep, and the dark of the mouth on its top.
    c.rect_lit(Rect::new(cx - 15, f - 430, 30, 9), Ramp::Stone, 14);
    c.rect_lit(Rect::new(cx - 13, f - 434, 26, 5), Ramp::Stone, 14);
    c.fill_rect(Rect::new(cx - 9, f - 434, 18, 2), Ramp::WallDark.at(Tone::Deep), 14);
    if night {
        c.set_emitting(true);
        c.hline(cx - 8, cx + 7, f - 434, Ramp::Ember.at(Tone::Base), 14);
        c.hline(cx - 5, cx + 4, f - 433, Ramp::Ember.at(Tone::Light), 14);
        c.set_emitting(false);
    }
    c.outline();
    c.upright(f);
    c
}

/// The world plume, frame `frame`: high and long, its mouth at [`PLUME_MOUTH`].
pub fn plume_world(night: bool, frame: u8) -> Canvas {
    let u = PLUME_U;
    let mut c = Canvas::new(42 * u, 26 * u);
    let mouth = (PLUME_MOUTH.0, c.h() - 1 - PLUME_MOUTH.1);
    plume_into(&mut c, u, night, frame, mouth, true);
    c
}

/// St Anne's tower and broach spire as they stand on its roof in the county: a stone tower lit
/// on its west face, two string courses, the belfry's louvred openings and a window that is lit
/// at evensong (`lit`); the spire in slate courses, lit west and shaded east, a lucarne halfway,
/// broaches at its foot, and an iron cross and a brass cock over it. 72 x 520, its foot the
/// bottom row, `WORLD_FEET[1]` across.
pub fn spire_world(lit: bool) -> Canvas {
    let (w, h) = (72, 520);
    let f = h - 1;
    let mut c = Canvas::new(w, h);
    let (x0, x1, top) = (12, 60, f - 180);
    // The tower: a block lit from the west.
    for y in top..=f {
        for x in x0..x1 {
            let t = if x == x0 {
                Tone::Light
            } else if x >= x1 - 16 {
                if x == x1 - 1 { Tone::Deep } else { Tone::Shade }
            } else {
                Tone::Base
            };
            c.dot(x, y, Ramp::Stone.at(t), 12);
        }
    }
    // Its stones: a course every six rows, the joints staggered.
    for y in (top + 3..f).step_by(6) {
        for x in x0 + 1..x1 - 1 {
            if (x + (y / 6) * 5) % 11 != 0 {
                darken(&mut c, Ramp::Stone, x, y);
            } else {
                darken(&mut c, Ramp::Stone, x, y - 1);
                darken(&mut c, Ramp::Stone, x, y - 2);
            }
        }
    }
    for sy in [f - 64, f - 124] {
        c.rect_lit(Rect::new(x0 - 2, sy, x1 - x0 + 4, 4), Ramp::Stone, 12);
    }
    // The belfry: two tall openings, louvred.
    for bx in [x0 + 8, x1 - 18] {
        c.fill_rect(Rect::new(bx, f - 170, 10, 34), Ramp::WallDark.at(Tone::Deep), 12);
        c.polyline_fill(&[(bx, f - 170), (bx + 5, f - 176), (bx + 9, f - 170)], Ramp::WallDark.at(Tone::Deep), 12);
        for ly in (f - 166..f - 138).step_by(4) {
            c.hline(bx + 1, bx + 8, ly, Ramp::WoodDark.at(Tone::Shade), 12);
        }
    }
    // The west window, lit at evensong.
    let win = Rect::new(x0 + 18, f - 46, 10, 26);
    if lit {
        c.set_emitting(true);
        c.fill_rect(win, Ramp::GlassLit.at(Tone::Base), 12);
        c.vline(win.x + 5, win.y, win.y + win.h - 1, Ramp::GlassLit.at(Tone::Shade), 12);
        c.dot(win.x + 1, win.y + 1, Ramp::GlassLit.at(Tone::High), 12);
        c.set_emitting(false);
    } else {
        c.fill_rect(win, Ramp::WallDark.at(Tone::Deep), 12);
        c.vline(win.x + 5, win.y, win.y + win.h - 1, Ramp::Stone.at(Tone::Shade), 12);
    }
    // The spire: the tower's width to a point, the west face lit, the east in shade, courses.
    let apex = (36, f - 470);
    for y in apex.1..top {
        let k = (y - apex.1) * 256 / (top - apex.1);
        let half = (24 * k) / 256;
        for x in apex.0 - half..=apex.0 + half {
            let t = if x == apex.0 - half {
                Tone::Light
            } else if x < apex.0 - half / 3 {
                Tone::Base
            } else if x < apex.0 + half / 3 {
                Tone::Mid
            } else {
                Tone::Shade
            };
            c.dot(x, y, Ramp::Slate.at(t), 14);
        }
        if (y - apex.1) % 7 == 6 {
            for x in apex.0 - half + 1..apex.0 + half {
                darken(&mut c, Ramp::Slate, x, y);
            }
        }
    }
    // The broaches at its foot, and a lucarne halfway up.
    c.polyline_fill(&[(x0, top), (x0 + 10, top - 22), (x0 + 10, top)], Ramp::Slate.at(Tone::Base), 14);
    c.polyline_fill(&[(x1 - 1, top), (x1 - 11, top - 22), (x1 - 11, top)], Ramp::Slate.at(Tone::Shade), 14);
    let ly = f - 330;
    c.fill_rect(Rect::new(apex.0 - 4, ly, 8, 12), Ramp::WallDark.at(Tone::Deep), 14);
    c.polyline_fill(&[(apex.0 - 6, ly), (apex.0, ly - 7), (apex.0 + 5, ly)], Ramp::Slate.at(Tone::Light), 14);
    // The finial: an iron rod and cross, and the cock on it catching the light.
    c.vline(apex.0, f - 500, apex.1, Ramp::Iron.at(Tone::Shade), 14);
    c.hline(apex.0 - 5, apex.0 + 5, f - 490, Ramp::Iron.at(Tone::Base), 14);
    c.hline(apex.0 - 3, apex.0 + 5, f - 503, Ramp::Brass.at(Tone::Base), 14);
    c.polyline_fill(
        &[
            (apex.0 - 3, f - 503),
            (apex.0 - 1, f - 509),
            (apex.0 + 2, f - 506),
            (apex.0 + 5, f - 509),
            (apex.0 + 4, f - 503),
        ],
        Ramp::Brass.at(Tone::Light),
        14,
    );
    c.outline();
    c.upright(f);
    c
}

/// The lake's statue as she stands in the county on her plinth: the figure of [`statue`] at 12
/// px a unit, turned as a body is in weathered stone, the plinth in lit courses; `dawn` gilds her
/// eastern edge. Her foot is the bottom row, `WORLD_FEET[2]` across.
pub fn statue_world(dawn: bool) -> Canvas {
    let u = STATUE_U;
    let flat = statue_at(u, false);
    let (w, h) = (flat.w(), flat.h());
    let f = h - 1;
    let plinth = 11 * u;
    // Her figure: what the far statue drew over the plinth.
    let mut m = Canvas::new(w, h);
    for y in 0..f - plinth {
        for x in 0..w {
            if flat.get(x, y).is_opaque() {
                m.dot(x, y, Ix::INK, 1);
            }
        }
    }
    let mut c = Canvas::new(w, h);
    solid(&mut c, &m, Ramp::Slate, 6, crate::canvas::Z::new(4, 14));
    // The folds of her robe and the veil's edge, cut in shadow.
    let p = |ax: i32, ay: i32| (11 * u + ax * u / 10, f - ay * u / 10);
    for (a, b) in
        [(p(-6, 185), p(-12, 112)), (p(6, 180), p(10, 112)), (p(-15, 238), p(-9, 190)), (p(1, 205), p(-2, 120))]
    {
        crate::canvas::bresenham(a.0, a.1, b.0, b.1, |x, y| {
            darken(&mut c, Ramp::Slate, x, y);
            darken(&mut c, Ramp::Slate, x + 1, y);
        });
    }
    // The plinth: a footing, the die and a cornice, each a lit block.
    let mid = 11 * u;
    c.rect_lit(Rect::new(mid - 4 * u, f - 2 * u + 1, 8 * u, 2 * u), Ramp::Slate, 8);
    c.rect_lit(Rect::new(mid - 3 * u, f - 10 * u + 1, 6 * u, 8 * u), Ramp::Slate, 10);
    c.rect_lit(Rect::new(mid - 4 * u, f - 11 * u + 1, 8 * u, u), Ramp::Slate, 10);
    // The lake's mark on the die: the stone darker and greened up to where the water stands in
    // winter, the line itself a ragged px.
    let wet = f - 2 * u - u - u / 2;
    for y in wet..f - 2 * u {
        for x in mid - 3 * u + 1..mid + 3 * u - 1 {
            darken(&mut c, Ramp::Slate, x, y);
        }
    }
    for x in mid - 3 * u + 1..mid + 3 * u - 1 {
        if bayer(x, wet) < 10 {
            c.dot(x, wet - 1, Ramp::Marsh.at(Tone::Shade), 10);
        }
    }
    c.outline();
    if dawn {
        // The dawn along her eastern edge, row by row, inside the outline: the first sun on her
        // before it is on anything else, so it glows.
        for y in 0..f - plinth {
            if let Some(x) = (0..w).rev().find(|&x| m.get(x, y).is_opaque()) {
                c.dot(x - 2, y, Ramp::Brass.at(Tone::Base), 14);
                c.set_emitting(y % 3 != 0);
                c.dot(x - 1, y, Ramp::Brass.at(Tone::Light), 14);
                c.set_emitting(false);
            }
        }
    }
    c.upright(f);
    c
}

/// The shops and the stacks, `u` px a unit, on a canvas with room for a plume.
fn chimney_at(u: i32, night: bool) -> Canvas {
    let (w, h) = (52 * u, 56 * u);
    let mut c = Canvas::new(w, h);
    let base = h - 1;
    let body = Ramp::WorksWall.at(Tone::Deep);
    let rim = Ramp::Works.at(Tone::Shade);
    let dark = Ramp::WallDark.at(Tone::Deep);
    let y = |units: i32| base - units * u;
    // The shops: a wall and four saw teeth, each a steep glazed face to the north and a long
    // pitch down.
    c.fill_rect(Rect::new(0, y(6), 26 * u, 6 * u + 1), body, 8);
    for k in 0..4 {
        let x0 = k * 13 * u / 2;
        let x1 = x0 + 13 * u / 2;
        c.polyline_fill(&[(x0, y(6)), (x0, y(10)), (x1, y(6))], body, 8);
        c.line((x0, y(10)), (x1, y(6)), rim, 1, 8);
        // The glazing in the steep face: dark, or at night a dim warm strip.
        let glass = Rect::new(x0 + 1, y(10) + u, (u / 2).max(1), 3 * u - 1);
        if night && k != 2 {
            c.set_emitting(true);
            c.fill_rect(glass, Ramp::GlassLit.at(Tone::Shade), 8);
            c.set_emitting(false);
        } else {
            c.fill_rect(glass, dark, 8);
        }
    }
    // The second stack, shorter and plain.
    c.fill_rect(Rect::new(21 * u, y(24), 2 * u, 18 * u), body, 10);
    c.fill_rect(Rect::new(21 * u - u / 2, y(24), 3 * u, u), body, 10);
    c.vline(21 * u, y(24), y(6), rim, 10);
    // The great stack: 4 units at its foot, 3 at its head, tapering by the px.
    let (foot, head, top) = (y(6), y(42), 14 * u);
    for py in head..=foot {
        let k = (py - head) * 256 / (foot - head).max(1);
        let half = (3 * u * 256 + (4 * u - 3 * u) * k) / 512;
        c.hline(top - half, top + half - 1, py, body, 12);
        // The sky-side (left) edge catches the light.
        c.dot(top - half, py, rim, 12);
    }
    // Two iron bands and the corbelled cap.
    for band_y in [y(18), y(31)] {
        let half = 2 * u;
        c.hline(top - half - 1, top + half, band_y, rim, 12);
    }
    c.fill_rect(Rect::new(top - 2 * u, y(44), 4 * u, 2 * u), body, 12);
    c.hline(top - 2 * u, top + 2 * u - 1, y(44), rim, 12);
    c.hline(top - 2 * u - 1, top + 2 * u, y(43), body, 12);
    // At night the mouth glows with the furnace under it.
    if night {
        c.set_emitting(true);
        c.hline(top - u, top + u - 1, y(44) - 1, Ramp::Ember.at(Tone::Base), 12);
        c.set_emitting(false);
    }
    c
}

/// The plume out of a mouth at `mouth` (px in `c`), `u` px a unit, frame `frame`: puffs leaving
/// the mouth and drifting east on the wind, growing and thinning, each with a lit crown and a
/// shaded belly; the frame walks them along. `tall` lifts it twice as high and a little further,
/// the world's plume. Many overlapping puffs make one billowing body: every belly first, then
/// every middle tone, then every lit crown, so it reads as one rope of smoke lit along its top.
fn plume_into(c: &mut Canvas, u: i32, night: bool, frame: u8, mouth: (i32, i32), tall: bool) {
    let n = if tall { 30 } else { 24 };
    let len = 256;
    let (run, up) = if tall { (36, 10) } else { (32, 4) };
    let puffs: Vec<(i32, i32, i32, i32)> = (0..n)
        .map(|k| {
            let s = (k * len / n + i32::from(frame % PLUME_FRAMES) * len / (n * i32::from(PLUME_FRAMES))) % len;
            let hs = h32(k as u32, 0, salt::PLUME);
            // Along a curve that rises steeply at the mouth and lies over on the wind, swelling.
            let dx = s * run * u / len;
            let rise = (up * u * s / len) + (5 * u * (s.min(64))) / 64;
            let wob = (below(hs, 3) as i32 - 1) * u * s / (2 * len);
            let r = u * 3 / 4 + s * 5 * u / (2 * len) + below(hs >> 8, 2) as i32 * u / 3;
            // Thick at the mouth, gone to a haze past two thirds.
            let fill = (17 - s * 16 / len).clamp(2, 16);
            (mouth.0 + dx, mouth.1 - rise + wob, r.max(1), fill)
        })
        .collect();
    let (belly, mid, crown) = if night {
        (Ramp::Slate.at(Tone::Deep), Ramp::Slate.at(Tone::Shade), Ramp::Stone.at(Tone::Shade))
    } else {
        (Ramp::Stone.at(Tone::Shade), Ramp::Stone.at(Tone::Mid), Ramp::Stone.at(Tone::Light))
    };
    for &(px, py, r, fill) in &puffs {
        disc_dither(c, px, py, r, belly, fill, 6);
    }
    for &(px, py, r, fill) in &puffs {
        disc_dither(c, px - r / 5, py - r / 3, r * 3 / 4, mid, fill, 6);
    }
    for &(px, py, r, fill) in &puffs {
        disc_dither(c, px - r / 3, py - r / 2, r / 2, crown, fill, 6);
    }
    for (k, &(px, py, r, fill)) in puffs.iter().enumerate() {
        let s = (k as i32 * len / n + i32::from(frame % PLUME_FRAMES) * len / (n * i32::from(PLUME_FRAMES))) % len;
        if night && s < len / 3 {
            // The furnace's light on the belly of the nearest puffs.
            c.set_emitting(true);
            let ember = if s < len / 6 { Ramp::Ember.at(Tone::Shade) } else { Ramp::Ember.at(Tone::Deep) };
            for x in -r..=r {
                let yy = py + r - (x * x) / (2 * r).max(1);
                if bayer(px + x, yy) < fill {
                    c.dot(px + x, yy, ember, 6);
                }
            }
            c.set_emitting(false);
        }
    }
}

/// St Anne's over the Lowfields: the nave's long roof, a west tower with its belfry open, and the
/// broach spire on it to a weathercock, two churchyard yews at its foot. `lit` puts a candle in
/// the nave's west window (evensong).
pub fn spire(band: u8, lit: bool) -> Canvas {
    spire_at(unit(band), lit)
}

fn spire_at(u: i32, lit: bool) -> Canvas {
    let (w, h) = (34 * u, 50 * u);
    let mut c = Canvas::new(w, h);
    let base = h - 1;
    let body = Ramp::Slate.at(Tone::Deep);
    let rim = Ramp::Slate.at(Tone::Shade);
    let dark = Ramp::WallDark.at(Tone::Deep);
    let y = |units: i32| base - units * u;
    // The nave: walls and a steep roof, the chancel lower at its east end.
    c.fill_rect(Rect::new(10 * u, y(7), 16 * u, 7 * u + 1), body, 8);
    c.polyline_fill(&[(10 * u, y(7)), (18 * u, y(12)), (26 * u, y(7))], body, 8);
    c.line((10 * u, y(7)), (18 * u, y(12)), rim, 1, 8);
    c.fill_rect(Rect::new(26 * u, y(5), 5 * u, 5 * u + 1), body, 8);
    c.polyline_fill(&[(26 * u, y(5)), (28 * u + u / 2, y(8)), (31 * u, y(5))], body, 8);
    // The nave's windows: tall lancets, one lit at evensong.
    for (k, x) in [14, 18, 22].into_iter().enumerate() {
        let r = Rect::new(x * u, y(5), u.max(2) - 1, 3 * u);
        if lit && k == 0 {
            c.set_emitting(true);
            c.fill_rect(r, Ramp::GlassLit.at(Tone::Base), 8);
            c.dot(r.x, r.y, Ramp::GlassLit.at(Tone::High), 8);
            c.set_emitting(false);
        } else {
            c.fill_rect(r, dark, 8);
        }
    }
    // The tower: square, with a string course and the belfry's louvred openings.
    let (tx, tw) = (4 * u, 6 * u);
    c.fill_rect(Rect::new(tx, y(18), tw, 18 * u + 1), body, 12);
    c.hline(tx - 1, tx + tw, y(12), rim, 12);
    c.hline(tx - 1, tx + tw, y(18), rim, 12);
    c.vline(tx, y(18), y(0), rim, 12);
    for bx in [tx + u, tx + 4 * u] {
        c.fill_rect(Rect::new(bx, y(17), u, 3 * u), dark, 12);
        // The louvres: a px of the stone across each opening.
        for ly in (y(17)..y(14)).step_by(2) {
            c.dot(bx, ly, rim, 12);
        }
    }
    // The broach spire: from the tower's whole width to a point, its broaches at the corners.
    let apex = (tx + tw / 2, y(42));
    c.polyline_fill(&[(tx, y(18)), (apex.0, apex.1), (tx + tw - 1, y(18))], body, 14);
    c.polyline_fill(&[(tx - 1, y(18)), (tx + u, y(21)), (tx + u, y(18))], body, 14);
    c.polyline_fill(&[(tx + tw, y(18)), (tx + tw - u - 1, y(21)), (tx + tw - u - 1, y(18))], body, 14);
    c.line((tx, y(18)), apex, rim, 1, 14);
    // A lucarne halfway up, and the cross and cock on the finial.
    c.fill_rect(Rect::new(apex.0 - u / 2, y(28), u.max(2) - 1, u + 1), dark, 14);
    c.vline(apex.0, y(46), apex.1, body, 14);
    c.hline(apex.0 - u, apex.0 + u, y(45), body, 14);
    c.hline(apex.0 - 1, apex.0 + u / 2 + 1, y(47), body, 14);
    c.dot(apex.0 + u / 2 + 1, y(47) - 1, body, 14);
    // Two yews in the churchyard: dark domes, ragged at the top.
    for (k, (yx, yr)) in [(29, 4), (1, 3)].into_iter().enumerate() {
        let (cx, r) = (yx * u, yr * u);
        let cy = base - r;
        for py in (cy - r)..=base {
            for px in (cx - r)..=(cx + r) {
                let (dx, dy) = (px - cx, (py - cy).min(0));
                let ragged = below(h32(px as u32, k as u32, salt::YEWS), 3) as i32;
                if dx * dx + dy * dy * 3 / 2 <= r * r - ragged * u {
                    c.dot(px, py, Ramp::LeafDeep.at(Tone::Deep), 6);
                }
            }
        }
        c.dot(cx - r / 3, cy - r + 1, Ramp::LeafDeep.at(Tone::Shade), 6);
    }
    c
}

/// The statue in the lake on her plinth (WORLD.md §5.3): a robed woman, one hand raised, on a
/// stepped pedestal standing out of the water, reeds at its foot. `dawn` gilds her eastern edge
/// (right) as the haze is lit from under the sky: a shape against the haze before she is a
/// statue. The day and the night faces are the presenter's mirror, not this.
pub fn statue(band: u8, dawn: bool) -> Canvas {
    statue_at(unit(band), dawn)
}

fn statue_at(u: i32, dawn: bool) -> Canvas {
    let (w, h) = (22 * u, 40 * u);
    let mut c = Canvas::new(w, h);
    let base = h - 1;
    let body = Ramp::Stone.at(Tone::Deep);
    let rim = if dawn { Ramp::Brass.at(Tone::Light) } else { Ramp::Stone.at(Tone::Shade) };
    let y = |units: i32| base - units * u;
    let mid = 11 * u;
    // The water's line and the reeds either side of the plinth.
    c.hline(0, w - 1, base, Ramp::Water.at(Tone::Deep), 2);
    for k in 0..9 {
        let x = (k * 5 + 1) * u / 2 + below(h32(k as u32, 7, salt::YEWS), 2) as i32;
        if (x - mid).abs() < 4 * u {
            continue;
        }
        let tall = u + below(h32(k as u32, 9, salt::YEWS), 3) as i32 * u / 2;
        c.line((x, base), (x + (k % 3 - 1), base - tall), Ramp::Reed.at(Tone::Deep), 1, 3);
    }
    // The plinth: a footing, the die and a cornice.
    c.fill_rect(Rect::new(mid - 4 * u, y(2), 8 * u, 2 * u), body, 8);
    c.fill_rect(Rect::new(mid - 3 * u, y(10), 6 * u, 8 * u), body, 10);
    c.fill_rect(Rect::new(mid - 4 * u, y(11), 8 * u, u), body, 10);
    c.hline(mid - 4 * u, mid + 4 * u - 1, y(11), rim, 10);
    c.vline(mid + 3 * u - 1, y(10), y(2), rim, 10);
    // A point in tenths of a unit: across from her middle, and up from the water.
    let p = |ax: i32, ay: i32| (mid + ax * u / 10, base - ay * u / 10);
    // Her robe: sloped shoulders, drawn in at the waist, flaring to a hem that trails west.
    let robe = [
        p(-32, 110),
        p(-21, 150),
        p(-15, 190),
        p(-16, 220),
        p(-20, 243),
        p(-10, 255),
        p(-5, 259),
        p(5, 259),
        p(10, 255),
        p(20, 243),
        p(16, 220),
        p(15, 190),
        p(20, 150),
        p(27, 110),
    ];
    c.polyline_fill(&robe, body, 14);
    // The lit edge down her eastern side.
    for py in robe[9].1..=robe[13].1 {
        if let Some(x) = (0..w).rev().find(|&x| c.get(x, py) == body) {
            c.dot(x, py, rim, 14);
        }
    }
    // The folds, and her other arm held down along the robe.
    let fold = Ramp::Stone.at(Tone::Deep);
    let ink = |c: &mut Canvas, a: (i32, i32), b: (i32, i32)| c.line(a, b, fold, 1, 14);
    ink(&mut c, p(-6, 185), p(-12, 112));
    ink(&mut c, p(6, 180), p(10, 112));
    ink(&mut c, p(-15, 238), p(-9, 190));
    // The raised arm: up and out from the shoulder, the elbow bent, the hand open over her head.
    let (sh, elbow, hand) = (p(17, 240), p(31, 266), p(33, 312));
    let thick = (u * 4 / 5).max(1);
    c.line(sh, elbow, body, thick, 14);
    c.line(elbow, hand, body, thick, 14);
    c.line((elbow.0 + thick / 2 + 1, elbow.1), (hand.0 + thick / 2 + 1, hand.1 + u / 2), rim, 1, 14);
    // The hand: a palm and three fingers' worth of px.
    c.fill_rect(Rect::new(hand.0 - u / 2, hand.1 - u, u.max(2), u + 1), body, 14);
    c.vline(hand.0 - u / 2, hand.1 - u - (u / 2).max(1), hand.1 - u, body, 14);
    c.vline(hand.0 + u / 2, hand.1 - u - (u / 2).max(1), hand.1 - u, body, 14);
    // Her head on its neck, veiled, a little bowed toward the raised hand.
    let (hx, hy, hr) = (mid + u / 5, base - 278 * u / 10, (u * 6 / 5).max(2));
    for py in -hr..=hr {
        for px in -hr..=hr {
            if px * px + py * py <= hr * hr {
                c.dot(hx + px, hy + py, body, 14);
            }
        }
    }
    // Her neck, and the veil falling from the back of her head over her west shoulder.
    let neck = (u * 4 / 5).max(1);
    c.fill_rect(Rect::new(hx - neck / 2, hy, neck, p(0, 259).1 - hy + 1), body, 14);
    c.polyline_fill(
        &[
            (hx - hr / 2, hy - hr),
            (hx - hr - u / 4, hy),
            (hx - hr - u / 3, hy + hr + u / 2),
            p(-19, 243),
            p(-6, 257),
            (hx, hy + hr),
        ],
        body,
        14,
    );
    for py in -hr..=0 {
        let half = jane_core::num::isqrt((hr * hr - py * py) as u64) as i32;
        c.dot(hx + half, hy + py, rim, 14);
    }
    c
}

/// The Hoar Stone's canvas width, px.
pub const HOAR_W: i32 = 48;
/// Its height, px. Its foot is the bottom row, its sprite's anchor.
pub const HOAR_H: i32 = 136;

/// The silhouette's slab, foot at `HOAR_H - 1`: a tall weathered stone with a rounded shoulder,
/// leaning a little east, wider at its foot.
fn hoar_mask() -> Canvas {
    let f = HOAR_H - 1;
    let mut m = Canvas::new(HOAR_W, HOAR_H);
    m.polyline_fill(
        &[
            (6, f),
            (4, f - 28),
            (7, f - 60),
            (6, f - 84),
            (9, f - 108),
            (12, f - 121),
            (19, f - 124),
            (29, f - 133),
            (34, f - 131),
            (37, f - 122),
            (35, f - 112),
            (38, f - 100),
            (37, f - 76),
            (41, f - 44),
            (44, f - 12),
            (43, f),
        ],
        Ix::INK,
        1,
    );
    m
}

/// A 3 x 5 cut letter of "DO NOT": rows top to bottom, three bits each, left first.
fn cut_letter(ch: char) -> [u8; 5] {
    match ch {
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'N' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        _ => [0; 5],
    }
}

/// The Hoar Stone (`data/chunks/burial.chunk`: "The stair under the Hoar Stone"): a monolith of
/// grey stone two and a half cells across and over eight tall, rising from the stair's block so
/// it stands over the ground fog and over the edge of her screen (the world audit §3: the last
/// fifty metres). Lichen in clusters, moss at its foot, and DO NOT cut in it at her eye's height,
/// each letter's lower lip catching the light. Heights are true (`Canvas::upright`), so on T2 it
/// stands out of a ground fog's `top`.
pub fn hoar_stone() -> Canvas {
    let mask = hoar_mask();
    let f = HOAR_H - 1;
    let mut c = Canvas::new(HOAR_W, HOAR_H);
    c.inflate(&mask, Ramp::Stone, 9, crate::canvas::Z::new(2, 10));
    c.retone(
        Ramp::Stone,
        [Tone::Deep, Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Light, Tone::High],
    );
    // Its bedding: faint courses across it where the stone split, and two cracks, each a dark
    // seam with its lower lip in the light.
    for (k, yy) in [f - 30, f - 77, f - 101].into_iter().enumerate() {
        for x in 0..HOAR_W {
            let y = yy + (x + k as i32 * 5) / 9 % 2;
            if mask.get(x, y) != Ix::CLEAR && mask.get(x - 2, y) != Ix::CLEAR && mask.get(x + 2, y) != Ix::CLEAR {
                c.tint(x, y, Ramp::Stone, Tone::Shade);
            }
        }
    }
    for crack in [
        [(30, f - 128), (27, f - 116), (29, f - 104), (25, f - 92)],
        [(9, f - 44), (14, f - 38), (13, f - 26), (17, f - 18)],
    ] {
        for s in crack.windows(2) {
            crate::canvas::bresenham(s[0].0, s[0].1, s[1].0, s[1].1, |x, y| {
                if mask.get(x, y) != Ix::CLEAR {
                    c.dot(x, y, Ramp::Stone.at(Tone::Deep), 9);
                    if mask.get(x + 1, y + 1) != Ix::CLEAR {
                        c.dot(x + 1, y + 1, Ramp::Stone.at(Tone::Light), 9);
                    }
                }
            });
        }
    }
    // Weathering: vertical runnels where the rain has gone down it for centuries.
    for (k, x) in [12, 19, 27, 33].into_iter().enumerate() {
        let top = f - 120 + below(h32(k as u32, 1, salt::YEWS), 20) as i32;
        let len = 24 + below(h32(k as u32, 2, salt::YEWS), 40) as i32;
        for y in top..(top + len).min(f - 6) {
            let xx = x + ((y / 9 + k as i32) % 3 == 0) as i32;
            if mask.get(xx, y) != Ix::CLEAR && mask.get(xx + 2, y) != Ix::CLEAR && (y + k as i32) % 5 != 0 {
                c.tint(xx, y, Ramp::Stone, Tone::Shade);
            }
        }
    }
    // DO NOT, cut a little under halfway up: dark, with a lit lower lip.
    let (mut x, y0) = (11, f - 62);
    for ch in "DO NOT".chars() {
        let rows = cut_letter(ch);
        for (r, bits) in rows.iter().enumerate() {
            for b in 0..3 {
                if bits & (0b100 >> b) != 0 {
                    let (px, py) = (x + b, y0 + r as i32);
                    c.dot(px, py, Ramp::Stone.at(Tone::Deep), 9);
                    if !(r + 1 < 5 && rows[r + 1] & (0b100 >> b) != 0) {
                        c.dot(px, py + 1, Ramp::Stone.at(Tone::Light), 9);
                    }
                }
            }
        }
        x += if ch == ' ' { 3 } else { 4 };
    }
    // Underneath, smaller, scratched later (`data/dialogue.json`): a run of fine scratches.
    for k in 0..7 {
        let sx = 13 + k * 3;
        c.dot(sx, f - 52 + (k % 2), Ramp::Stone.at(Tone::Shade), 9);
        c.dot(sx + 1, f - 51, Ramp::Stone.at(Tone::Shade), 9);
    }
    // Lichen: the hoar of its name, pale crusts in clusters, a few of them mustard.
    for k in 0..16u32 {
        let h = h32(k, 3, salt::YEWS);
        let (cx, cy) = (10 + below(h, 24) as i32, f - 20 - below(h >> 8, 96) as i32);
        let ramp = if k % 3 == 0 { Ramp::ClothMustard } else { Ramp::Bone };
        for j in 0..(6 + below(h >> 16, 9)) {
            let hj = h32(k, j, salt::YEWS);
            let (dx, dy) = (below(hj, 7) as i32 - 3, below(hj >> 8, 5) as i32 - 2);
            if mask.get(cx + dx, cy + dy) != Ix::CLEAR {
                c.dot(cx + dx, cy + dy, ramp.at(if j % 3 == 0 { Tone::Light } else { Tone::Base }), 9);
            }
        }
    }
    // Moss and grass about its foot.
    for x in 4..HOAR_W - 4 {
        let n = 2 + below(h32(x as u32, 4, salt::YEWS), 6) as i32;
        for d in 0..n {
            let y = f - d;
            if mask.get(x, y) != Ix::CLEAR && bayer(x, y) < 16 - d * 2 {
                c.dot(x, y, Ramp::Leaf.at(if d == n - 1 { Tone::Base } else { Tone::Shade }), 3);
            }
        }
    }
    c.outline();
    c.upright(f);
    c
}

/// The Hoar Stone as a shape in the fog, for the tiers whose fog has no height (T0, T1): the
/// stone's mask in ink, its lower third thinning into the fog by the Bayer pattern so it rises
/// out of it. Drawn over the fog as a ghost at the fog's own density.
pub fn hoar_silhouette() -> Canvas {
    let mask = hoar_mask();
    let f = HOAR_H - 1;
    let mut c = Canvas::new(HOAR_W, HOAR_H);
    let fade = HOAR_H / 3;
    for y in 0..HOAR_H {
        let up = f - y;
        for x in 0..HOAR_W {
            if mask.get(x, y) == Ix::CLEAR {
                continue;
            }
            if up >= fade || bayer(x, y) < up * 16 / fade {
                c.dot(x, y, Ix::INK, 1);
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inked(c: &Canvas) -> usize {
        c.albedo().iter().filter(|&&i| i != Ix::CLEAR).count()
    }

    #[test]
    fn each_landmark_grows_with_its_band_and_stands_on_its_foot() {
        for b in 0..3u8 {
            for c in [chimney(b, false, 0), spire(b, false), statue(b, false)] {
                assert!(c.validate().is_ok());
                // Something stands on the bottom row: it stands on the horizon.
                let foot = (0..c.w()).filter(|&x| c.get(x, c.h() - 1) != Ix::CLEAR).count();
                assert!(foot > 0, "a landmark stands on its foot");
            }
        }
        assert!(inked(&chimney(2, false, 0)) > inked(&chimney(0, false, 0)));
        assert!(inked(&spire(2, false)) > inked(&spire(0, false)));
        assert!(inked(&statue(2, false)) > inked(&statue(0, false)));
    }

    #[test]
    fn the_plume_moves_and_the_night_shift_glows() {
        let a = chimney(1, false, 0);
        let b = chimney(1, false, 3);
        assert_ne!(a.hash(), b.hash(), "the plume drifts frame to frame");
        let night = chimney(1, true, 0);
        assert!(night.emissive().iter().any(|&e| e != Ix::CLEAR), "the furnace lights the plume");
        assert!(a.emissive().iter().all(|&e| e == Ix::CLEAR), "by day nothing glows");
        assert!(spire(1, true).emissive().iter().any(|&e| e != Ix::CLEAR));
        assert_ne!(statue(1, true).hash(), statue(1, false).hash(), "the dawn gilds her");
    }

    #[test]
    fn the_hoar_stone_stands_eight_cells_and_rises_out_of_the_fog() {
        let c = hoar_stone();
        assert!(c.validate().is_ok());
        let top = (0..c.h()).find(|&y| (0..c.w()).any(|x| c.get(x, y) != Ix::CLEAR)).unwrap();
        assert!(c.h() - top >= 8 * 16, "taller than eight cells");
        let s = hoar_silhouette();
        let row = |y: i32| (0..s.w()).filter(|&x| s.get(x, y) != Ix::CLEAR).count();
        assert!(row(s.h() - 2) < row(s.h() / 2), "its foot thins into the fog");
    }
}
