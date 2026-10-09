//! The night kit's contact sheets (NIGHT.md §4.3, ART.md §5): every look at 1x and 4x on what it
//! would really be laid on, and one wall at 1x (and blown up) with a few of them in use.
//!
//! `cargo run -p jane-art --example night_sheet --release -- OUTDIR`

use jane_art::canvas::Canvas;
use jane_art::font::{Face, Font};
use jane_art::night::{self, Look, Opening};
use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_art::sheet::{Image, label, put_albedo};

const BG: [u8; 3] = [22, 20, 30];
const TEXT: [u8; 3] = [216, 208, 192];
const PAD: u32 = 8;

/// What a look is laid on.
#[derive(Clone, Copy)]
enum Ground {
    Plaster,
    BrickFace,
    Window(Opening),
    Cobbles,
    Railing,
    LampArm,
    Post,
    Door,
}

fn rgb(r: Ramp, t: Tone) -> [u8; 3] {
    palette::rgb(r.at(t))
}

fn h(x: i32, y: i32) -> u32 {
    let mut v = (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b);
    v ^= v >> 15;
    v = v.wrapping_mul(0x2c1b_3c6d);
    v ^ (v >> 12)
}

/// The colour of `g` at canvas px `(x, y)` (the look's own px; the margin is negative or past it).
fn ground(g: Ground, x: i32, y: i32) -> [u8; 3] {
    match g {
        Ground::Plaster => {
            let t = if h(x / 3, y / 2) % 9 == 0 { Tone::Lift } else { Tone::Base };
            rgb(Ramp::PlasterOchre, t)
        }
        Ground::BrickFace => {
            let (row, course) = (y.rem_euclid(4), y.div_euclid(4));
            let col = (x + if course % 2 == 0 { 0 } else { 3 }).rem_euclid(6);
            if row == 3 || col == 5 {
                rgb(Ramp::Stone, Tone::Mid)
            } else {
                let t = [Tone::Base, Tone::Mid, Tone::Base, Tone::Shade][(h((x + 3) / 6, course) % 4) as usize];
                rgb(Ramp::Brick, if row == 0 { t.step(1) } else { t })
            }
        }
        Ground::Window(o) => {
            let (w, hh) = o.size();
            let (lx, ly) = (x - 1, y - 1);
            if (-1..=w).contains(&lx) && ly == -1 {
                rgb(Ramp::Stone, Tone::Mid)
            } else if (-1..=w).contains(&lx) && ly == hh {
                rgb(Ramp::Stone, Tone::High)
            } else if (0..w).contains(&lx) && (0..hh).contains(&ly) {
                if pane(o, lx, ly) {
                    rgb(Ramp::Glass, if ly < hh / 2 { Tone::Base } else { Tone::Shade })
                } else {
                    rgb(Ramp::RacingGreen, if lx == 0 || ly == 0 { Tone::Lift } else { Tone::Shade })
                }
            } else {
                ground(Ground::Plaster, x, y)
            }
        }
        Ground::Cobbles => {
            let (cy, ry) = (y.div_euclid(4), y.rem_euclid(4));
            let cx = (x + if cy % 2 == 0 { 0 } else { 2 }).rem_euclid(5);
            if ry == 3 || cx == 4 {
                rgb(Ramp::Setts, Tone::Shade)
            } else if ry == 0 || cx == 0 {
                rgb(Ramp::Setts, Tone::Lift)
            } else {
                rgb(Ramp::Setts, Tone::Base)
            }
        }
        Ground::Railing => {
            if y == 0 {
                rgb(Ramp::Iron, Tone::Light)
            } else if y == 1 {
                rgb(Ramp::Iron, Tone::Shade)
            } else if x.rem_euclid(5) == 2 {
                rgb(Ramp::Iron, Tone::Deep)
            } else {
                rgb(Ramp::Setts, Tone::Shade)
            }
        }
        Ground::LampArm => {
            if y == -1 {
                rgb(Ramp::Iron, Tone::Light)
            } else if y == -2 {
                rgb(Ramp::Iron, Tone::Base)
            } else {
                [38, 44, 64]
            }
        }
        Ground::Post => {
            if x == 3 {
                rgb(Ramp::Iron, Tone::Light)
            } else if x == 4 {
                rgb(Ramp::Iron, Tone::Shade)
            } else {
                rgb(Ramp::Setts, Tone::Shade)
            }
        }
        Ground::Door => {
            let t = if x.rem_euclid(4) == 3 { Tone::Shade } else { Tone::Base };
            rgb(Ramp::Oxblood, t)
        }
    }
}

fn pane(o: Opening, x: i32, y: i32) -> bool {
    let (w, h) = o.size();
    if x <= 0 || y <= 0 || x >= w - 1 || y >= h - 1 {
        return false;
    }
    let (mx, my) = (w / 2, h / 2);
    match o {
        Opening::Sash => {
            let (q1, q3) = (my / 2, my + (h - 1 - my) / 2 + 1);
            !(x == mx || y == my || y == q1 || y == q3)
        }
        _ => !(x == mx || (w >= 10 && x == mx - 1) || y == my),
    }
}

fn ground_of(l: Look) -> Ground {
    match l {
        Look::Boards { opening, .. }
        | Look::Brick { opening, .. }
        | Look::Card { opening }
        | Look::Crack { opening, .. } => Ground::Window(opening),
        Look::Rust { variant } if variant % 2 == 1 => Ground::BrickFace,
        Look::Rust { .. } | Look::Peel { .. } | Look::Notice { .. } => Ground::Plaster,
        Look::Pipe { .. } => Ground::BrickFace,
        Look::Coat { .. } => Ground::Railing,
        Look::Cage | Look::HandBell | Look::TagChain | Look::Strip { .. } => Ground::LampArm,
        Look::Ring => Ground::Post,
        Look::Chain | Look::DoorTag => Ground::Door,
        _ => Ground::Cobbles,
    }
}

/// `c` laid on `g` at `s`x with `m` px of ground round it, its top-left at image `(x, y)`.
fn tile(img: &mut Image, c: &Canvas, g: Ground, x: u32, y: u32, s: u32, m: i32) {
    for cy in -m..c.h() + m {
        for cx in -m..c.w() + m {
            let rgb = ground(g, cx, cy);
            img.fill(x + ((cx + m) as u32) * s, y + ((cy + m) as u32) * s, s, s, rgb);
        }
    }
    put_albedo(img, c, x + m as u32 * s, y + m as u32 * s, s);
}

/// A short name for a look's label.
fn short(l: Look) -> String {
    let o = |o: Opening| match o {
        Opening::Casement => "cas",
        Opening::Sash => "sash",
        Opening::Upper => "up",
    };
    match l {
        Look::Boards { layout, opening } => format!("boards {layout} {}", o(opening)),
        Look::Brick { rough, opening } => format!("brick {} {}", if rough { "rough" } else { "cours" }, o(opening)),
        Look::Card { opening } => format!("card {}", o(opening)),
        Look::Crack { variant, opening } => format!("crack {variant} {}", o(opening)),
        Look::Coat { cut, cloth } => format!("coat {cut}/{cloth}"),
        _ => format!("{l:?}")
            .to_lowercase()
            .replace("variant: ", "")
            .replace("groups: ", "")
            .replace(['{', '}', ' '], ""),
    }
}

/// Which page of the sheet a look is on.
fn page(l: Look) -> usize {
    match l {
        Look::Boards { .. } | Look::Brick { .. } | Look::Card { .. } | Look::Crack { .. } => 0,
        Look::Coat { .. } => 1,
        Look::Rust { .. }
        | Look::Peel { .. }
        | Look::Notice { .. }
        | Look::Pipe { .. }
        | Look::Chain
        | Look::DoorTag => 2,
        Look::Cage | Look::HandBell | Look::TagChain | Look::Strip { .. } | Look::Ring => 3,
        _ => 4,
    }
}

fn kit_sheet(font: &Font, pg: usize) -> Image {
    let looks: Vec<Look> = night::all().into_iter().filter(|&l| page(l) == pg).collect();
    let (wmax, m) = (720u32, 3i32);
    // Lay the cells out in rows.
    let mut place = Vec::new();
    let (mut x, mut y, mut row_h) = (PAD, PAD, 0u32);
    for &l in &looks {
        let c = night::render(l);
        let (w1, h1) = ((c.w() + 2 * m) as u32, (c.h() + 2 * m) as u32);
        let cw = (w1 + 4 + w1 * 4).max(60) + 10;
        let ch = 14 + h1 * 4 + 8;
        if x + cw > wmax {
            x = PAD;
            y += row_h;
            row_h = 0;
        }
        place.push((l, c, x, y));
        x += cw;
        row_h = row_h.max(ch);
    }
    let mut img = Image::new(wmax + PAD, y + row_h + PAD, [BG[0], BG[1], BG[2], 255]);
    for (l, c, x, y) in place {
        label(&mut img, font, x, y, &short(l), Face::Fine, TEXT);
        let g = ground_of(l);
        let w1 = (c.w() + 2 * m) as u32;
        tile(&mut img, &c, g, x, y + 14, 1, m);
        tile(&mut img, &c, g, x + w1 + 4, y + 14, 4, m);
    }
    img
}

/// A wall in use: three windows covered, rust under the sills, plaster gone, a notice, a pipe, a
/// railing of coats before it, chalk and a grate on the road, a stain by the door.
fn in_use() -> Canvas {
    let (w, h) = (176, 84);
    let mut c = Canvas::new(w, h);
    let lay = |c: &mut Canvas, l: Look, x: i32, y: i32| {
        let s = night::render(l);
        let (ax, ay) = night::anchor(l);
        c.stamp(&s, x - ax, y - ay);
    };
    // Covers on three windows at their openings' top-lefts; rust from their sills.
    lay(&mut c, Look::Boards { layout: 1, opening: Opening::Casement }, 12, 30);
    lay(&mut c, Look::Brick { rough: false, opening: Opening::Casement }, 44, 30);
    lay(&mut c, Look::Card { opening: Opening::Sash }, 78, 29);
    lay(&mut c, Look::Boards { layout: 0, opening: Opening::Upper }, 13, 6);
    lay(&mut c, Look::Crack { variant: 0, opening: Opening::Upper }, 45, 6);
    lay(&mut c, Look::Boards { layout: 2, opening: Opening::Upper }, 78, 6);
    lay(&mut c, Look::Rust { variant: 0 }, 17, 16);
    lay(&mut c, Look::Rust { variant: 1 }, 50, 41);
    lay(&mut c, Look::Rust { variant: 2 }, 82, 41);
    lay(&mut c, Look::Peel { variant: 0 }, 100, 10);
    lay(&mut c, Look::Notice { variant: 0 }, 112, 28);
    lay(&mut c, Look::Pipe { variant: 1 }, 128, 4);
    lay(&mut c, Look::DoorTag, 148, 36);
    // A railing's coats.
    lay(&mut c, Look::Coat { cut: 0, cloth: 0 }, 100, 50);
    lay(&mut c, Look::Coat { cut: 3, cloth: 2 }, 116, 50);
    lay(&mut c, Look::Coat { cut: 1, cloth: 3 }, 131, 50);
    // The road.
    lay(&mut c, Look::Tally { groups: 2 }, 6, 66);
    lay(&mut c, Look::Nine, 34, 70);
    lay(&mut c, Look::Grate { variant: 1 }, 50, 70);
    lay(&mut c, Look::Stain { variant: 0 }, 150, 64);
    lay(&mut c, Look::Barrier, 66, 83);
    c
}

/// The wall and road under [`in_use`]: plaster above a brick plinth, three windows on each floor,
/// the railing, the cobbles.
fn in_use_ground(x: i32, y: i32) -> [u8; 3] {
    for (ox, oy, o) in [
        (12, 30, Opening::Casement),
        (44, 30, Opening::Casement),
        (78, 29, Opening::Sash),
        (13, 6, Opening::Upper),
        (45, 6, Opening::Upper),
        (78, 6, Opening::Upper),
    ] {
        let (w, hh) = o.size();
        if (ox - 1..=ox + w).contains(&x) && (oy - 1..=oy + hh).contains(&y) {
            return ground(Ground::Window(o), x - ox + 1, y - oy + 1);
        }
    }
    if (140..=156).contains(&x) && (24..=47).contains(&y) {
        return ground(Ground::Door, x, y);
    }
    if y >= 58 {
        return ground(Ground::Cobbles, x, y);
    }
    if y == 50 && x >= 94 {
        return rgb(Ramp::Iron, Tone::Light);
    }
    if y >= 46 {
        return ground(Ground::BrickFace, x, y);
    }
    ground(Ground::Plaster, x, y)
}

fn use_sheet(font: &Font) -> Image {
    let c = in_use();
    let s = 4u32;
    let (w, h) = (c.w() as u32, c.h() as u32);
    let mut img = Image::new(PAD * 3 + w + w * s, PAD + 14 + h * s + PAD, [BG[0], BG[1], BG[2], 255]);
    label(&mut img, font, PAD, PAD, "in use, 1x and 4x", Face::Fine, TEXT);
    for (x0, sc) in [(PAD, 1), (PAD * 2 + w, s)] {
        for y in 0..c.h() {
            for x in 0..c.w() {
                img.fill(x0 + x as u32 * sc, PAD + 14 + y as u32 * sc, sc, sc, in_use_ground(x, y));
            }
        }
        put_albedo(&mut img, &c, x0, PAD + 14, sc);
    }
    let _ = Ix::CLEAR;
    img
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let font = Font::build();
    for (pg, name) in ["windows", "coats", "walls", "hung", "ground"].into_iter().enumerate() {
        let kit = kit_sheet(&font, pg);
        std::fs::write(format!("{out}/night_kit_{pg}_{name}.png"), kit.png()).expect("write kit sheet");
    }
    let used = use_sheet(&font);
    std::fs::write(format!("{out}/night_in_use.png"), used.png()).expect("write use sheet");
    let mut colours = std::collections::BTreeSet::new();
    let mut area = 0;
    for l in night::all() {
        let c = night::render(l);
        area += c.w() * c.h();
        colours.extend(c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0));
    }
    println!("{} looks, {area} px, {} colours", night::all().len(), colours.len());
}
