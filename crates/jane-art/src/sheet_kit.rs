//! The prop kit's contact sheet (ART.md §5): `jane sheet props`, every prop look in every
//! frame it promises, at 1x and 3x on the ground it stands on, its glowing frames also shown
//! at night so the emissive reads. Pure; never asserted (the goldens hash the canvases).

use crate::canvas::Canvas;
use crate::font::{Face, Font};
use crate::looks::Rendered;
use crate::palette::{self, Ix, Ramp, Tone};
use crate::sheet::{Image, label};

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

/// `jane sheet props`: each prop's frames at 1x, then at 3x, then its lit frame at night.
pub fn props(sets: &[Rendered], font: &Font) -> Image {
    const S: u32 = 3;
    let cols = 4u32;
    let cell_w = sets.iter().map(|r| r.set.w as u32 * (S + 1) * r.set.frames.len().min(3) as u32).max().unwrap_or(64) + 40;
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
