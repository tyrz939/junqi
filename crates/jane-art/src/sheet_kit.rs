//! The prop kit's contact sheet (ART.md §5): `jane sheet props`, every prop look in every
//! frame it promises, at 1x and 3x on the ground it stands on, its glowing frames also shown
//! at night so the emissive reads. Pure; never asserted (the goldens hash the canvases).

use crate::canvas::Canvas;
use crate::font::{Face, Font};
use crate::looks::Rendered;
use crate::palette::{self, Ix, Ramp, Tone};
use crate::sheet::{Image, label};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

const BG: [u8; 3] = [22, 20, 30];
const TEXT: [u8; 3] = [216, 208, 192];
const DIM: [u8; 3] = [150, 144, 132];
const PAD: u32 = 8;

/// A frame at `s`x on the day-lit grass, or at night (the ground and the prop dimmed to a
/// blue dusk, what emits laid over unlit).
fn draw(img: &mut Image, c: &Canvas, x: u32, y: u32, s: u32, night: bool) {
    let ground = palette::rgb(Ramp::Grass.at(Tone::Shade));
    let dim = |p: [u8; 3]| if night { [p[0] / 4, p[1] / 4 + 4, p[2] / 3 + 10] } else { p };
    for cy in 0..c.h() {
        for cx in 0..c.w() {
            let ix = c.get(cx, cy);
            let e = c.emissive_at(cx, cy);
            let rgb = if e != Ix::CLEAR && night {
                palette::rgb(e)
            } else {
                match ix {
                    Ix::CLEAR => dim(ground),
                    Ix::AO => dim(palette::ao(ground, palette::ao_cover(c, cx, cy))),
                    _ => dim(palette::rgb(ix)),
                }
            };
            img.fill(x + cx as u32 * s, y + cy as u32 * s, s, s, rgb);
        }
    }
}

/// `jane sheet icons`: every icon on a dark slot at 32 and its chip at 16, at 1x, then both at
/// 3x, named.
pub fn icons(sets: &[Rendered], font: &Font) -> Image {
    const S: u32 = 3;
    let cols = 6u32;
    let (cell_w, cell_h) = (32 + 16 + 32 * S + 16 * S + 40, 32 * S + 26);
    let names: Vec<&Rendered> = sets.iter().filter(|r| r.variant == 0).collect();
    let rows = (names.len() as u32).div_ceil(cols);
    let mut img = Image::new(PAD + cols * cell_w + PAD, PAD + rows * cell_h + PAD, [BG[0], BG[1], BG[2], 255]);
    let slot = palette::rgb(Ramp::UiSlot.at(Tone::Base));
    for (k, r) in names.iter().enumerate() {
        let (x0, y0) = (PAD + (k as u32 % cols) * cell_w, PAD + (k as u32 / cols) * cell_h);
        label(&mut img, font, x0, y0, r.name, Face::Fine, TEXT);
        let chip = sets.iter().find(|c| c.sprite == r.sprite && c.variant == 1);
        let mut x = x0;
        for (set, s) in [(Some(*r), 1), (chip, 1), (Some(*r), S), (chip, S)] {
            let Some(set) = set else { continue };
            let c = &set.set.frames[0].1;
            let side = c.w() as u32 * s;
            img.fill(x, y0 + 14, side, side, slot);
            for cy in 0..c.h() {
                for cx in 0..c.w() {
                    let ix = c.get(cx, cy);
                    if ix.is_opaque() {
                        img.fill(x + cx as u32 * s, y0 + 14 + cy as u32 * s, s, s, palette::rgb(ix));
                    }
                }
            }
            x += side + 6;
        }
    }
    img
}

/// `jane sheet props`: each prop's frames at 1x, then at 3x, then its lit frame at night.
pub fn props(sets: &[Rendered], font: &Font) -> Image {
    const S: u32 = 3;
    let cols = 4u32;
    let cell_w =
        sets.iter().map(|r| r.set.w as u32 * (S + 1) * r.set.frames.len().min(3) as u32).max().unwrap_or(64) + 40;
    let cell_w = cell_w.min(560);
    let cell_h = sets.iter().map(|r| r.set.h as u32 * S).max().unwrap_or(48) + 30;
    let rows = (sets.len() as u32).div_ceil(cols);
    let mut img = Image::new(PAD + cols * cell_w + PAD, PAD + rows * cell_h + PAD, [BG[0], BG[1], BG[2], 255]);
    for (k, r) in sets.iter().enumerate() {
        let (x0, y0) = (PAD + (k as u32 % cols) * cell_w, PAD + (k as u32 / cols) * cell_h);
        label(&mut img, font, x0, y0, &r.key(), Face::Fine, TEXT);
        let (w, h) = (r.set.w as u32, r.set.h as u32);
        let mut x = x0;
        for (_, c) in &r.set.frames {
            draw(&mut img, c, x, y0 + 14, 1, false);
            x += w + 2;
        }
        x += 6;
        for (f, c) in r.set.frames.iter().take(3) {
            if x + w * S > x0 + cell_w - 4 {
                break;
            }
            let glows = c.emissive().iter().any(|&e| e != Ix::CLEAR);
            draw(&mut img, c, x, y0 + 14, S, glows);
            label(&mut img, font, x, y0 + 16 + h * S, f.name(), Face::Fine, DIM);
            x += w * S + 4;
        }
    }
    img
}

/// The fire looks `jane sheet fires` lays out, each with its frames in the order she meets them
/// (cold, laid, lit, ash); the older fires after, to be judged beside.
const FIRES: [&str; 5] = ["campfire_cold", "old_grate", "campfire", "cold_hearth", "brazier_cold"];

/// `jane sheet fires` (2026-10-02, the fire she makes): each fire's frames at 1x by day and by
/// night, then at 4x by day and by night, labelled; the match flare's frames at 4x over the
/// night ground; the deadwood and the matches (and the firewood they must not be) at 32 and 16,
/// at 1x and 3x.
pub fn fires(font: &Font) -> Result<Image, String> {
    use crate::sprite::FrameId;
    const S: u32 = 4;
    let order = [FrameId::Base, FrameId::Open2, FrameId::On, FrameId::Open];
    let mut looks = Vec::new();
    for name in FIRES {
        looks.extend(crate::looks::render(name)?.into_iter().filter(|r| r.variant == 0 && r.seat == 0));
    }
    let cell_h = |r: &Rendered| 16 + r.set.h as u32 * S * 2 + 30;
    let width = 1240;
    let flare_h = 16 + 48 * S + 16;
    let icons_h = 16 + 32 * 3 + 20;
    let height = PAD + looks.iter().map(cell_h).sum::<u32>() + flare_h + icons_h + PAD;
    let mut img = Image::new(width, height, [BG[0], BG[1], BG[2], 255]);
    let mut y = PAD;
    for r in &looks {
        let (w, h) = (r.set.w as u32, r.set.h as u32);
        label(&mut img, font, PAD, y, r.name, Face::Fine, TEXT);
        let mut frames: Vec<(FrameId, &Canvas)> =
            order.iter().filter_map(|f| r.set.frames.iter().find(|(g, _)| g == f).map(|(_, c)| (*f, c))).collect();
        frames.extend(r.set.frames.iter().filter(|(f, _)| !order.contains(f)).map(|(f, c)| (*f, c)));
        for (row, night) in [(0u32, false), (1, true)] {
            let y0 = y + 14 + row * (h * S + 14);
            let mut x = PAD;
            for (_, c) in &frames {
                draw(&mut img, c, x, y0, 1, night);
                x += w + 2;
            }
            x += 10;
            for (f, c) in &frames {
                draw(&mut img, c, x, y0, S, night);
                let name = match (*f, frames.len()) {
                    (FrameId::Base, 4) => "cold",
                    (FrameId::Open2, 4) => "laid",
                    (FrameId::On, 4) => "lit",
                    (FrameId::Open, 4) => "ash",
                    _ => f.name(),
                };
                label(&mut img, font, x, y0 + h * S + 1, name, Face::Fine, DIM);
                x += w * S + 6;
            }
        }
        y += cell_h(r);
    }
    // The match flare, a frame every few ticks, struck at the cell's middle a hand high.
    label(&mut img, font, PAD, y, "match_flare", Face::Fine, TEXT);
    let r = crate::fx::match_flare();
    let mut rng = crate::fx::Lcg(5);
    let mut pool = Vec::new();
    crate::fx::emit(&r, (0, 0), jane_core::Angle::NORTH, &mut rng, &mut |s| pool.push(s));
    let ground = palette::rgb(Ramp::Grass.at(Tone::Shade));
    let night = [ground[0] / 4, ground[1] / 4 + 4, ground[2] / 3 + 10];
    let (fw, fh) = (40i32, 48i32);
    let mut x = PAD;
    for tick in 0..=u32::from(crate::fx::MATCH_TICKS) {
        if [0, 2, 4, 7, 11, 16, 22, 29].contains(&tick) {
            let mut px = vec![0u32; (fw * fh) as usize];
            crate::fx::rasterise(&pool, &mut px, fw, fh, (fw / 2, fh - 8));
            img.fill(x, y + 14, fw as u32 * S, fh as u32 * S, night);
            for (i, p) in px.iter().enumerate() {
                if *p != 0 {
                    let rgb = [(p >> 16) as u8, (p >> 8) as u8, *p as u8];
                    let (qx, qy) = (i as u32 % fw as u32, i as u32 / fw as u32);
                    img.fill(x + qx * S, y + 14 + qy * S, S, S, rgb);
                }
            }
            label(&mut img, font, x, y + 15 + fh as u32 * S, &format!("t{tick}"), Face::Fine, DIM);
            x += fw as u32 * S + 6;
        }
        pool.retain_mut(crate::fx::Spark::step);
    }
    y += flare_h;
    // The icons, beside the firewood.
    let mut x = PAD;
    let slot = palette::rgb(Ramp::UiSlot.at(Tone::Base));
    for name in ["item_deadwood", "item_match", "item_wood"] {
        let sets = crate::looks::render(name)?;
        label(&mut img, font, x, y, name, Face::Fine, TEXT);
        for (v, s) in [(0u8, 1u32), (1, 1), (0, 3), (1, 3)] {
            let Some(set) = sets.iter().find(|r| r.variant == v) else { continue };
            let c = &set.set.frames[0].1;
            let side = c.w() as u32 * s;
            img.fill(x, y + 14, side, side, slot);
            for cy in 0..c.h() {
                for cx in 0..c.w() {
                    let ix = c.get(cx, cy);
                    if ix.is_opaque() {
                        img.fill(x + cx as u32 * s, y + 14 + cy as u32 * s, s, s, palette::rgb(ix));
                    }
                }
            }
            x += side + 6;
        }
        x += 20;
    }
    Ok(img)
}
