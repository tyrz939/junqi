//! The people's contact sheets (ART.md §5): `jane sheet unit <id>`, `jane sheet units` and
//! `jane sheet person --grid`, on the day-lit ground they stand on. Pure functions of the sets
//! and the font; never asserted (the goldens hash the canvases).

use jane_data::{Build, Coat, Hair, PersonLook};

use crate::canvas::Canvas;
use crate::font::{Face, Font};
use crate::looks::Rendered;
use crate::palette::{self, Ramp, Tone};
use crate::person;
use crate::sheet::{Image, label, put_albedo};
use crate::sprite::FrameId;

const BG: [u8; 3] = [22, 20, 30];
const TEXT: [u8; 3] = [216, 208, 192];
const DIM: [u8; 3] = [150, 144, 132];
const PAD: u32 = 8;

/// The ground a unit is shown on: the Lowfields' grass, day-lit.
fn ground() -> [u8; 3] {
    palette::rgb(Ramp::Grass.at(Tone::Shade))
}

fn draw(img: &mut Image, c: &Canvas, x: u32, y: u32, s: u32) {
    img.fill(x, y, c.w() as u32 * s, c.h() as u32 * s, ground());
    put_albedo(img, c, x, y, s);
}

/// The rows of `jane sheet unit`: the down, up and side cycles (six walk frames and the
/// breathe), the dead frames, and a creature's idle pair, hurt and attack.
const ROWS: [&[FrameId]; 5] = [
    &[FrameId::Down, FrameId::Down1, FrameId::Down2, FrameId::Down3, FrameId::Down4, FrameId::Down5, FrameId::DownB],
    &[FrameId::Up, FrameId::Up1, FrameId::Up2, FrameId::Up3, FrameId::Up4, FrameId::Up5, FrameId::UpB],
    &[FrameId::Side, FrameId::Side1, FrameId::Side2, FrameId::Side3, FrameId::Side4, FrameId::Side5, FrameId::SideB],
    &[FrameId::Dead, FrameId::Dead2],
    &[FrameId::Idle, FrameId::Idle2, FrameId::Hurt, FrameId::Atk1, FrameId::Atk2, FrameId::Atk3],
];

/// `jane sheet unit <id>`: every set the sprite renders to (each variant, each seat), each as a
/// 1x strip of every frame with the west frames mirrored beside it, then the cycles at 4x.
/// `scale` is the grid's (4 on the owner's sheets).
pub fn unit(sets: &[Rendered], font: &Font, scale: u32) -> Image {
    let s = scale.max(1);
    let (fw, fh) = sets.first().map_or((person::W as u32, person::H as u32), |r| (r.set.w as u32, r.set.h as u32));
    let rows: Vec<&[FrameId]> =
        ROWS.iter().copied().filter(|ids| sets.iter().any(|r| ids.iter().any(|&f| r.set.frame(f).is_some()))).collect();
    let cols = 7u32;
    let cell = (fw * s + PAD, fh * s + 14 + PAD);
    let strip_h = fh + 20;
    let block_h = 20 + strip_h + rows.len() as u32 * cell.1;
    let w = PAD + cols * cell.0 + PAD;
    let strip_w = PAD + (30 + 4) * (fw + 2) + PAD;
    let mut img = Image::new(w.max(strip_w), PAD + sets.len() as u32 * block_h + PAD, [BG[0], BG[1], BG[2], 255]);
    for (k, r) in sets.iter().enumerate() {
        let y0 = PAD + k as u32 * block_h;
        label(&mut img, font, PAD, y0, &format!("{}: {} frames", r.key(), r.set.frames.len()), Face::Small, TEXT);
        // The 1x strip, west mirrored at the end.
        let mut x = PAD;
        for (_, c) in &r.set.frames {
            draw(&mut img, c, x, y0 + 20, 1);
            x += fw + 2;
        }
        x += 8;
        for id in [FrameId::Side, FrameId::Side1, FrameId::Side2, FrameId::Side3] {
            if let Some(c) = r.set.frame(id) {
                let mut m = c.clone();
                m.mirror_x();
                draw(&mut img, &m, x, y0 + 20, 1);
                x += fw + 2;
            }
        }
        for (row, ids) in rows.iter().enumerate() {
            for (col, id) in ids.iter().enumerate() {
                let Some(c) = r.set.frame(*id) else { continue };
                let (fx, fy) = (PAD + col as u32 * cell.0, y0 + 20 + strip_h + row as u32 * cell.1);
                draw(&mut img, c, fx, fy, s);
                label(&mut img, font, fx, fy + fh * s + 2, id.name(), Face::Fine, DIM);
            }
        }
    }
    img
}

/// `jane sheet units`: every set of every look, the standing frames and the dead frame at 1x
/// and 2x, named.
pub fn units(sets: &[Rendered], font: &Font) -> Image {
    let fw = sets.iter().map(|r| r.set.w as u32).max().unwrap_or(person::W as u32);
    let fh = sets.iter().map(|r| r.set.h as u32).max().unwrap_or(person::H as u32);
    let ids = [FrameId::Down, FrameId::Up, FrameId::Side, FrameId::Idle, FrameId::Dead];
    let cell_w = ids.len() as u32 * (fw * 2 + 2) + fw * 5 + 24;
    let cell_h = fh * 2 + 30;
    let cols = 3u32;
    let rows = (sets.len() as u32).div_ceil(cols);
    let mut img = Image::new(PAD + cols * cell_w + PAD, PAD + rows * cell_h + PAD, [BG[0], BG[1], BG[2], 255]);
    for (k, r) in sets.iter().enumerate() {
        let (x0, y0) = (PAD + (k as u32 % cols) * cell_w, PAD + (k as u32 / cols) * cell_h);
        label(&mut img, font, x0, y0, &r.key(), Face::Fine, TEXT);
        let mut x = x0;
        let (w, h) = (r.set.w as u32, r.set.h as u32);
        for id in ids {
            if let Some(c) = r.set.frame(id) {
                draw(&mut img, c, x, y0 + 14 + fh - h, 1);
                x += w + 2;
            }
        }
        x += 8;
        for id in ids {
            if let Some(c) = r.set.frame(id) {
                draw(&mut img, c, x, y0 + 14 + 2 * (fh - h), 2);
                x += w * 2 + 2;
            }
        }
    }
    img
}

/// `jane sheet person --grid`: every build by every hair, in every coat, standing, at 2x; the
/// look's other axes from `base`.
pub fn grid(base: &PersonLook, font: &Font) -> Result<Image, String> {
    const S: u32 = 2;
    let builds = [Build::Slim, Build::Broad, Build::Stout, Build::Child];
    let hairs =
        [Hair::Short, Hair::Cropped, Hair::Long, Hair::Bun, Hair::Pigtails, Hair::Bald, Hair::Curlers, Hair::Wet];
    let coats = [
        Coat::Coat,
        Coat::Dress,
        Coat::Gown,
        Coat::Apron,
        Coat::Smock,
        Coat::Jacket,
        Coat::Nightdress,
        Coat::Overcoat,
        Coat::Canvas,
        Coat::Cardigan,
    ];
    let ramps = ["cloth_plum", "cloth_navy", "cloth_rose", "cloth_tweed", "cloth_sky"];
    let (fw, fh) = (person::W as u32 * S, person::H as u32 * S);
    let label_w = 120;
    let mut img = Image::new(
        PAD + label_w + coats.len() as u32 * (fw + 2) + PAD,
        PAD + 16 + (builds.len() * hairs.len()) as u32 * (fh + 2) + PAD,
        [BG[0], BG[1], BG[2], 255],
    );
    for (k, coat) in coats.iter().enumerate() {
        label(&mut img, font, PAD + label_w + k as u32 * (fw + 2), PAD, &format!("{coat:?}"), Face::Fine, TEXT);
    }
    let mut row = 0u32;
    for b in builds {
        for h in hairs {
            let y = PAD + 16 + row * (fh + 2);
            label(&mut img, font, PAD, y + fh / 2 - 6, &format!("{b:?} {h:?}"), Face::Fine, TEXT);
            for (k, coat) in coats.iter().enumerate() {
                let mut look = *base;
                look.build = b;
                look.head.hair = h;
                look.body.coat = *coat;
                look.body.coat_ramp = ramps[(row as usize + k) % ramps.len()];
                let set = person::render(&look, person::seed("grid") ^ row)?;
                if let Some(c) = set.frame(FrameId::Down) {
                    draw(&mut img, c, PAD + label_w + k as u32 * (fw + 2), y, S);
                }
            }
            row += 1;
        }
    }
    Ok(img)
}

/// `jane sheet close <id> [frame ...]`: a few frames of one set side by side at `scale`, with
/// the 1x frames under them: the art director's loupe.
pub fn closeup(r: &Rendered, ids: &[FrameId], font: &Font, scale: u32) -> Image {
    let s = scale.max(1);
    let (fw, fh) = (r.set.w as u32, r.set.h as u32);
    let n = ids.len().max(1) as u32;
    let mut img =
        Image::new(PAD + n * (fw * s + PAD), PAD + 20 + fh * s + PAD + fh + 2 * PAD, [BG[0], BG[1], BG[2], 255]);
    label(&mut img, font, PAD, PAD, &r.key(), Face::Small, TEXT);
    for (k, id) in ids.iter().enumerate() {
        let Some(c) = r.set.frame(*id) else { continue };
        let x = PAD + k as u32 * (fw * s + PAD);
        draw(&mut img, c, x, PAD + 20, s);
        draw(&mut img, c, x, PAD + 20 + fh * s + PAD, 1);
        label(&mut img, font, x + fw + 6, PAD + 20 + fh * s + PAD, id.name(), Face::Fine, DIM);
    }
    img
}

/// `jane sheet silhouettes <id> ...`: each look's standing frames filled black on a pale
/// ground, at 3x, then the frames themselves under them: do they differ in outline alone?
pub fn silhouettes(sets: &[Rendered], font: &Font) -> Image {
    const S: u32 = 3;
    let (fw, fh) = (person::W as u32 * S, person::H as u32 * S);
    let ids = [FrameId::Down, FrameId::Side];
    let cell = (ids.len() as u32 * (fw + 2) + PAD, fh * 2 + 30);
    let mut img = Image::new(PAD + sets.len() as u32 * cell.0, PAD + cell.1, [BG[0], BG[1], BG[2], 255]);
    for (k, r) in sets.iter().enumerate() {
        let x0 = PAD + k as u32 * cell.0;
        label(&mut img, font, x0, PAD, r.name, Face::Fine, TEXT);
        for (j, id) in ids.iter().enumerate() {
            let Some(c) = r.set.frame(*id) else { continue };
            let x = x0 + j as u32 * (fw + 2);
            img.fill(x, PAD + 14, fw, fh, [214, 208, 196]);
            for cy in 0..c.h() {
                for cx in 0..c.w() {
                    if c.get(cx, cy).is_opaque() {
                        img.fill(x + cx as u32 * S, PAD + 14 + cy as u32 * S, S, S, [24, 20, 30]);
                    }
                }
            }
            draw(&mut img, c, x, PAD + 16 + fh, S);
        }
    }
    img
}
