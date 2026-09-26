//! Contact sheets and the PNG encoder (ART.md §5): stored deflate, crc32, adler32. No image
//! crate, so a sheet is the same bytes on every target.
//!
//! The sheets are for looking, never asserted; the goldens hash the canvases instead. Every
//! sheet here is a pure function of its canvas and the font.

use jane_core::angle::Angle;
use jane_core::grid::Rect;

use crate::canvas::{Canvas, Dir, decode};
use crate::chrome::{self, ButtonState};
use crate::font::{self, Face, Font, Style};
use crate::light::{self, Ground, Sun};
use crate::palette::{self, Ix, Ramp, Tone};

/// An RGBA image, 8 bits a channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in px.
    pub w: u32,
    /// Height in px.
    pub h: u32,
    /// Row-major RGBA, 4 bytes a pixel.
    pub px: Vec<u8>,
}

impl Image {
    /// A `w x h` image filled with `fill`.
    pub fn new(w: u32, h: u32, fill: [u8; 4]) -> Self {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&fill);
        }
        Self { w, h, px }
    }

    /// Set pixel `(x, y)`; outside the image does nothing.
    pub fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        if x < self.w && y < self.h {
            let i = ((y * self.w + x) * 4) as usize;
            self.px[i..i + 4].copy_from_slice(&c);
        }
    }

    /// The pixel at `(x, y)`; transparent black outside.
    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        if x < self.w && y < self.h {
            let i = ((y * self.w + x) * 4) as usize;
            [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
        } else {
            [0; 4]
        }
    }

    /// A `s x s` block per source pixel.
    pub fn block(&mut self, x: u32, y: u32, s: u32, c: [u8; 4]) {
        for dy in 0..s {
            for dx in 0..s {
                self.set(x * s + dx, y * s + dy, c);
            }
        }
    }

    /// Fill the `w x h` px rect at `(x, y)` with opaque `rgb`.
    pub fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, [rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }

    /// Lay `rgb` over pixel `(x, y)` at coverage `a` (0..=255).
    pub fn blend(&mut self, x: u32, y: u32, rgb: [u8; 3], a: u8) {
        let d = self.get(x, y);
        let a = u32::from(a);
        let mix = |s: u8, d: u8| ((u32::from(s) * a + u32::from(d) * (255 - a) + 127) / 255) as u8;
        self.set(x, y, [mix(rgb[0], d[0]), mix(rgb[1], d[1]), mix(rgb[2], d[2]), 255]);
    }

    /// Encode as PNG.
    pub fn png(&self) -> Vec<u8> {
        png(self.w, self.h, &self.px)
    }
}

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xffff_ffffu32;
    for bytes in chunks {
        for &b in *bytes {
            c = table[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xffff_ffff
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[&kind, data]).to_be_bytes());
}

/// Encode RGBA pixels as a PNG, uncompressed (stored deflate blocks).
pub fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len(), (w * h * 4) as usize, "png: pixel count");
    let mut raw = Vec::with_capacity(rgba.len() + h as usize);
    for row in rgba.chunks((w * 4) as usize) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        let n = b.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, deflate, adaptive filters, no interlace
    chunk(&mut out, *b"IHDR", &ihdr);
    chunk(&mut out, *b"IDAT", &z);
    chunk(&mut out, *b"IEND", &[]);
    out
}

/// Sheet background.
const BG: [u8; 3] = [22, 20, 30];
/// The checker behind clear pixels, per canvas px.
const CHECK: [[u8; 3]; 2] = [[52, 50, 62], [64, 62, 74]];
/// Label ink.
const TEXT: [u8; 3] = [216, 208, 192];
/// Space between panels, px.
const PAD: u32 = 8;

fn checker(x: i32, y: i32) -> [u8; 3] {
    CHECK[((x ^ y) & 1) as usize]
}

fn rgba(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

fn split(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

fn darken(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| (u32::from(v) * 179 / 256) as u8)
}

fn lerp3(a: [u8; 3], b: [u8; 3], t: u32, of: u32) -> [u8; 3] {
    let of = of.max(1);
    let t = t.min(of);
    [0, 1, 2].map(|k| ((u32::from(a[k]) * (of - t) + u32::from(b[k]) * t) / of) as u8)
}

/// Fill canvas px `(cx, cy)` of a panel at `(x0, y0)` shown `s` image px a canvas px.
fn put_px(img: &mut Image, x0: u32, y0: u32, cx: i32, cy: i32, s: u32, rgb: [u8; 3]) {
    img.fill(x0 + cx as u32 * s, y0 + cy as u32 * s, s, s, rgb);
}

/// Write `text` in `face` at `(x, y)` in image px, in `rgb`.
pub fn label(img: &mut Image, font: &Font, x: u32, y: u32, text: &str, face: Face, rgb: [u8; 3]) {
    let run = crate::demo::text_run(font, text, Style::plain(face, Ix::INK));
    for cy in 0..run.h() {
        for cx in 0..run.w() {
            if run.get(cx, cy).is_opaque() {
                put_px(img, x, y, cx, cy, 1, rgb);
            }
        }
    }
}

/// Lay a canvas's albedo over the image at `(x, y)`, `s` image px a canvas px: clear shows what
/// is there, AO darkens it to 70 %, chrome's translucent fills blend by [`palette::alpha`].
pub fn put_albedo(img: &mut Image, c: &Canvas, x: u32, y: u32, s: u32) {
    for cy in 0..c.h() {
        for cx in 0..c.w() {
            let ix = c.get(cx, cy);
            for d in 0..s * s {
                let (px, py) = (x + cx as u32 * s + d % s, y + cy as u32 * s + d / s);
                match ix {
                    Ix::CLEAR | Ix::AO => {
                        let cover = palette::ao_cover(c, cx, cy);
                        if cover > 0 {
                            let p = img.get(px, py);
                            img.set(px, py, rgba(palette::ao([p[0], p[1], p[2]], cover)));
                        }
                    }
                    _ => img.blend(px, py, palette::rgb(ix), palette::alpha(ix)),
                }
            }
        }
    }
}

/// `jane sheet layers`: albedo, normal (R east, G south, B up), emissive (over the sprite at a
/// sixth) and height (dark to light over `0..=max`) side by side at 4x, each on a checker where
/// nothing is drawn.
pub fn layers(c: &Canvas, name: &str, font: &Font) -> Image {
    const S: u32 = 4;
    let (pw, ch) = ((c.w() as u32 * S).max(96), c.h() as u32 * S);
    let top = PAD + 18 + 14;
    let mut img = Image::new(PAD + 4 * (pw + PAD), top + ch + PAD, rgba(BG));
    let maxh = c.heights().iter().copied().max().unwrap_or(0);
    label(&mut img, font, PAD, PAD, &format!("layers: {name}, {} x {} px", c.w(), c.h()), Face::Small, TEXT);
    let names = ["albedo".to_string(), "normal".into(), "emissive".into(), format!("height 0-{maxh} px")];
    for (k, n) in names.iter().enumerate() {
        label(&mut img, font, PAD + k as u32 * (pw + PAD), PAD + 20, n, Face::Fine, TEXT);
    }
    let (dark, light) = (split(palette::SHADOW_TINT), split(palette::LIGHT_TINT));
    for cy in 0..c.h() {
        for cx in 0..c.w() {
            let ix = c.get(cx, cy);
            let bg = checker(cx, cy);
            let drawn = ix.is_opaque();
            let albedo = match ix {
                Ix::CLEAR => bg,
                Ix::AO => darken(bg),
                _ => palette::rgb(ix),
            };
            let normal = if drawn {
                let [nx, ny, nz] = decode(c.normal_at(cx, cy));
                [(128 + nx) as u8, (128 + ny) as u8, (128 + nz) as u8]
            } else {
                bg
            };
            let e = c.emissive_at(cx, cy);
            let emissive = if e != Ix::CLEAR {
                palette::rgb(e)
            } else if drawn {
                palette::rgb(ix).map(|v| v / 6)
            } else {
                [8, 8, 12]
            };
            let height = if drawn { lerp3(dark, light, u32::from(c.height_at(cx, cy)), u32::from(maxh)) } else { bg };
            for (k, rgb) in [albedo, normal, emissive, height].into_iter().enumerate() {
                put_px(&mut img, PAD + k as u32 * (pw + PAD), top, cx, cy, S, rgb);
            }
        }
    }
    img
}

/// `jane sheet light`: the sprite on open ground lit from the eight compass points at 30° and
/// from overhead through [`light::light`], with the shadows its height casts; a 3 x 3 grid at
/// 3x with overhead in the middle. `night` drops the ambient so emissive parts show.
pub fn lit(c: &Canvas, name: &str, font: &Font, night: bool) -> Image {
    lit_with(c, name, font, night, None)
}

/// [`lit`] for an upright sprite standing on row `ay` (a person): its shadow is its silhouette
/// cast from the feet ([`light::light_upright`]), as a renderer draws it.
pub fn lit_upright(c: &Canvas, name: &str, font: &Font, night: bool, ay: i32) -> Image {
    lit_with(c, name, font, night, Some(ay))
}

fn lit_with(c: &Canvas, name: &str, font: &Font, night: bool, upright: Option<i32>) -> Image {
    const S: u32 = 3;
    // Grid position and name of each sun: N, NE, E, SE, S, SW, W, NW, overhead.
    const AT: [(u32, u32, &str); 9] = [
        (1, 0, "N"),
        (2, 0, "NE"),
        (2, 1, "E"),
        (2, 2, "SE"),
        (1, 2, "S"),
        (0, 2, "SW"),
        (0, 1, "W"),
        (0, 0, "NW"),
        (1, 1, "overhead"),
    ];
    let maxh = i32::from(c.heights().iter().copied().max().unwrap_or(0));
    // A shadow at 30° runs 1.73 times the height; leave it room.
    let pad = (maxh * 2).max(12);
    let mut stage = Canvas::new(c.w() + 2 * pad, c.h() + 2 * pad);
    stage.stamp(c, pad, pad);
    let (ambient, colour) = if night { ([22, 24, 42], [256, 232, 196]) } else { ([64, 64, 84], [236, 228, 208]) };
    let suns: [Sun; 9] = light::compass(Angle::from_degrees(30), colour);
    let ground = Ground { ix: Ramp::Grass.at(Tone::Mid) };
    let (pw, ph) = (stage.w() as u32 * S, stage.h() as u32 * S);
    let top = PAD + 18;
    let mut img = Image::new(PAD + 3 * (pw + PAD), top + 3 * (ph + 14 + PAD), rgba(BG));
    let when = if night { "night" } else { "dusk" };
    let title = format!("light: {name}, eight points at 30 deg and overhead, {when}");
    label(&mut img, font, PAD, PAD, &title, Face::Small, TEXT);
    for (sun, (gx, gy, name)) in suns.iter().zip(AT) {
        let px = match upright {
            Some(ay) => light::light_upright(&stage, ay + pad, sun, ambient, ground),
            None => light::light(&stage, sun, ambient, ground),
        };
        let (x0, y0) = (PAD + gx * (pw + PAD), top + gy * (ph + 14 + PAD));
        for (i, rgb) in px.iter().enumerate() {
            put_px(&mut img, x0, y0, i as i32 % stage.w(), i as i32 / stage.w(), S, *rgb);
        }
        label(&mut img, font, x0, y0 + ph + 2, name, Face::Fine, TEXT);
    }
    img
}

/// `jane sheet palette`: the TS base colours, then every ramp in luminance order with its name.
pub fn palette_sheet(font: &Font) -> Image {
    const SW: u32 = 14;
    const ROW: u32 = 18;
    const NAME_W: u32 = 104;
    const COL: u32 = NAME_W + 8 * (SW + 2) + 16;
    let rows = Ramp::ALL.len().div_ceil(2) as u32;
    let y0 = PAD + 22;
    let mut img = Image::new(PAD + 2 * COL, y0 + 2 * ROW + rows * ROW + PAD, rgba(BG));
    label(&mut img, font, PAD, PAD, &format!("palette: {} entries", palette::LEN), Face::Small, TEXT);
    label(&mut img, font, PAD, y0 + 1, "base", Face::Fine, TEXT);
    for i in 2..palette::RAMP_BASE {
        img.fill(PAD + 48 + u32::from(i - 2) * (SW + 2), y0, SW, SW, palette::rgb(Ix(i)));
    }
    for (k, r) in Ramp::ALL.iter().enumerate() {
        let (cx, cy) = (PAD + (k as u32 % 2) * COL, y0 + 2 * ROW + (k as u32 / 2) * ROW);
        label(&mut img, font, cx, cy + 1, r.name(), Face::Fine, TEXT);
        for (t, tone) in Tone::ALL.iter().enumerate() {
            img.fill(cx + NAME_W + t as u32 * (SW + 2), cy, SW, SW, palette::rgb(r.at(*tone)));
        }
    }
    img
}

/// `jane sheet font`: every glyph in all four faces, then sample runs with shadow, bold,
/// outline and wrapping; composed at 1x and shown at 2x.
pub fn font_sheet(font: &Font) -> Image {
    const W: i32 = 760;
    let ink = Ramp::UiInk.at(Tone::Light);
    let gold = Ramp::UiGold.at(Tone::High);
    let glyphs: Vec<char> = font::chars().collect();
    let rows = |f: Face| glyphs.len().div_ceil((W / f.cell().0) as usize) as i32;
    let body: i32 = Face::ALL.iter().map(|&f| 14 + rows(f) * f.cell().1 + 6).sum();
    let mut c = Canvas::flat(W + 16, 8 + body + 12 + 3 * 18 + 27 + 45 + 5 * 18 + 16);
    let mut y = 8;
    for f in Face::ALL {
        let (cw, chh) = f.cell();
        let what = format!("{f:?}: lattice {}x, pen {} px, cell {cw} x {chh}", f.scale(), f.pen());
        font.draw(&mut c, 8, y, &what, Style::plain(Face::Fine, Ramp::UiGold.at(Tone::Light)));
        y += 14;
        for (k, row) in glyphs.chunks((W / cw) as usize).enumerate() {
            let s: String = row.iter().collect();
            font.draw(&mut c, 8, y + k as i32 * chh, &s, Style::plain(f, ink));
        }
        y += rows(f) * chh + 6;
    }
    y += 12;
    let small = Style::plain(Face::Small, ink);
    font.draw(&mut c, 8, y, "Needs wood x2 · Rats 3 of 5 · £4 · 18:30", Style { shadow: true, ..small });
    font.draw(&mut c, 8, y + 18, "Bold: the left-luggage trunk is a railway trunk.", Style { bold: true, ..small });
    font.draw(&mut c, 8, y + 36, "Outlined: Lost Property", Style { outline: true, ..Style::plain(Face::Small, gold) });
    let head = Style { outline: true, shadow: true, ..Style::plain(Face::Head, gold) };
    font.draw(&mut c, 8, y + 54, "The Lowfields", head);
    font.draw(&mut c, 8, y + 81, "JANE", Style { face: Face::Title, ..head });
    let text =
        "A lamp that stops the train, a bell on an ankle, a tortoise going north, and what lives under the hatch.";
    for (k, line) in Font::wrap(Face::Small, text, 360).iter().enumerate() {
        font.draw(&mut c, 8, y + 132 + k as i32 * 18, line, small);
    }
    let mut img = Image::new(c.w() as u32 * 2, c.h() as u32 * 2, rgba(palette::rgb(Ramp::UiSlot.at(Tone::Base))));
    put_albedo(&mut img, &c, 0, 0, 2);
    img
}

/// `jane sheet chrome`: panels, buttons in each state, bars and slots over a sky so the
/// translucent fills show; composed at 1x and shown at 2x.
pub fn chrome_sheet(font: &Font) -> Image {
    const S: u32 = 2;
    let (w, h) = (400, 200);
    let mut back = Canvas::flat(w, h);
    back.fill_rect(Rect::new(0, 0, w, h), Ix::SEAM, 0);
    back.gradient(Rect::new(0, 0, w, 120), Ramp::Sky, Dir::Down, Tone::Mid, Tone::High, true);
    back.gradient(Rect::new(0, 120, w, 80), Ramp::Grass, Dir::Down, Tone::Base, Tone::Shade, true);
    let mut img = Image::new(w as u32 * S, h as u32 * S, rgba(BG));
    put_albedo(&mut img, &back, 0, 0, S);
    let mut at = |c: &Canvas, x: u32, y: u32| put_albedo(&mut img, c, x * S, y * S, S);
    let mut p = chrome::panel(150, 90);
    font.draw(
        &mut p,
        8,
        8,
        "Satchel",
        Style { shadow: true, ..Style::plain(Face::Small, Ramp::UiGold.at(Tone::Light)) },
    );
    let note = "A panel is 85 % fill with a cut-corner outline and a 2 px bevel.";
    for (k, line) in Font::wrap(Face::Fine, note, 134).iter().enumerate() {
        font.draw(&mut p, 8, 30 + k as i32 * 12, line, Style::plain(Face::Fine, Ramp::UiInk.at(Tone::Light)));
    }
    at(&p, 8, 8);
    at(&chrome::panel(40, 24), 164, 8);
    let states = [ButtonState::Idle, ButtonState::Hover, ButtonState::Pressed, ButtonState::Disabled];
    for (k, st) in states.iter().enumerate() {
        let (x, y) = (214 + (k as u32 % 2) * 70, 8 + (k as u32 / 2) * 26);
        at(&chrome::button(64, 20, *st, &format!("{st:?}"), font, Face::Fine), x, y);
    }
    at(&chrome::button(134, 26, ButtonState::Idle, "Continue", font, Face::Small), 214, 64);
    let bars = [
        (1000, 1000, Ramp::ClothRed),
        (620, 780, Ramp::ClothRed),
        (400, 400, Ramp::ClothBlue),
        (150, 330, Ramp::UiGold),
    ];
    for (k, (f, l, r)) in bars.iter().enumerate() {
        at(&chrome::bar(120, 8, *f, *l, *r), 8, 110 + k as u32 * 12);
    }
    for (k, left) in [0, 250, 500, 750].iter().enumerate() {
        let (x, y) = (140 + k as u32 * 40, 110);
        at(&chrome::slot(36), x, y);
        at(&chrome::sweep(36, *left), x, y);
        let x = 140 + k as u32 * 24;
        at(&chrome::slot(20), x, 152);
        at(&chrome::sweep(20, *left), x, 152);
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_the_references() {
        assert_eq!(crc32(&[b"123456789"]), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn a_png_has_its_structure() {
        let mut img = Image::new(3, 2, [0, 0, 0, 255]);
        img.set(1, 1, [255, 0, 0, 255]);
        let p = img.png();
        assert_eq!(&p[..8], &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
        assert_eq!(&p[12..16], b"IHDR");
        assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
        assert_eq!(p, img.png(), "same pixels, same bytes");
    }

    #[test]
    fn big_images_span_several_stored_blocks() {
        let img = Image::new(300, 100, [1, 2, 3, 4]);
        let p = img.png();
        assert!(p.len() > 300 * 100 * 4);
    }
}
