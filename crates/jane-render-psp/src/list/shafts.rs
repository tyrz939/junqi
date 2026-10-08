//! Light shafts as T1 draws them (`jane-render-gl2`'s `RAYS_FS`, PRESENTATION.md §1.9): over each
//! px in the sun's shade, the air up its column to 48 px, lit where the sun's ray past it reaches
//! the ground; shown against shade, faint in clear air and strong in mist, only while the sun is
//! low. On the GE: where the sun reaches is the frame's stencil (the silhouettes mark the shade),
//! read back as `T32` through [`sun_clut`] and summed at half size in the lightmap's target from
//! six points up each column (each a shifted quad), then added over the shade in the sun's light.

use jane_present::frame::WeatherKind;

use super::{Blend, Lister, Mode, Quad, Tex, Where, atmos_fx};
use crate::water::{TURN, isin};

/// The points up each column (T1 takes twelve, 4 px apart; here six, 8 px apart).
const STEPS: i32 = 6;
/// What the silhouettes mark the shade with.
const SHADE: u8 = 1;
/// The sun's spread past which the air holds no shafts (`jane_present::light::SHAFTS_SPREAD`
/// and the presenter's 200 over it).
const MOST_SPREAD: u16 = (5 * 65536 / 360) as u16 + 200;

/// The stencil as where the sun reaches: entry 1 (the shade) clear, every other a sixth of
/// T1's threshold-free share, opaque; `0xAABBGGRR`.
pub fn sun_clut() -> [u32; 256] {
    // Each point's share: T1's `(seen / 12 - 0.35) * 1.6` taken linear over six, about a tenth.
    let w = 26u32;
    core::array::from_fn(|i| if i == usize::from(SHADE) { 0 } else { 0xff00_0000 | w << 16 | w << 8 | w })
}

const fn quad(tex: Tex, mode: Mode, colour: u32, r: (i32, i32, i32, i32), uv: (i32, i32, i32, i32)) -> Quad {
    Quad {
        tex,
        mode,
        colour,
        x0: r.0 as i16,
        y0: r.1 as i16,
        x1: r.2 as i16,
        y1: r.3 as i16,
        u0: uv.0 as u16,
        v0: uv.1 as u16,
        u1: uv.2 as u16,
        v1: uv.3 as u16,
    }
}

impl Lister {
    /// The shafts, when the sun is under 30 degrees in clear air or mist (the presenter's rule
    /// for T1's `Rays` pass, which a console frame does not carry).
    pub(super) fn shafts(&mut self) {
        if self.atmos_off & atmos_fx::SHAFTS != 0 || self.cluts.len() < 4 {
            return;
        }
        let (Some(sun), Some(a)) = (self.sun, self.weather) else { return };
        let low = 30 * 65536 / 360;
        let el = i32::from(sun.elevation.0);
        if !matches!(a.kind, WeatherKind::Clear | WeatherKind::Mist)
            || sun.spread > MOST_SPREAD
            || el >= low
            || el <= 0
            || sun.colour.iter().all(|&c| c <= 40)
        {
            return;
        }
        let k = (low - el) * 255 / low;
        let strength = (k * (160 + i32::from(a.mist) * 95 / 255) / 255).min(255);
        // T2's shaft light: the sun at its gain by the strength, strongest in mist (gl2's
        // `0.06 + mist * 0.45`, at SUN_GAIN 1.6), as a display value added over the darks.
        let gain = strength * (15 + i32::from(a.mist) * 115 / 255) / 255 * 16 / 10 * 14 / 10;
        if gain < 2 {
            return;
        }
        let ray = sun.colour.map(|c| (i32::from(c) * gain / 255).min(255) as u32);
        // The column's step back along the ray, per px up: toward the sun's back across the
        // ground by the run (`1 / tan(elevation)`, at most 8), and up the screen.
        let (tx, ty) = (isin(i32::from(sun.azimuth.0) + TURN / 4), isin(i32::from(sun.azimuth.0)));
        let tan = (isin(el) * 4096 / isin(el + TURN / 4).max(1)).max(492);
        let run = 4096 * 4096 / tan;
        let (w, h) = (self.w, self.h);
        let (hw, hh) = (w / 2, h / 2);
        self.quads.push(quad(Tex::None, Mode::RtBegin, 0xff00_0000, (0, 0, hw, hh), (0, 0, 0, 0)));
        for s in 1..=STEPS {
            let up = s * 8;
            let ox = -(tx * up / 4096) * run / 4096;
            let oy = -up - (ty * up / 4096) * run / 4096;
            // Target px x reads the frame at `2x + ox`: only where that is on the canvas.
            let (x0, x1) = ((-ox).max(0).div_euclid(2), ((w - ox) / 2).min(hw));
            let (y0, y1) = ((-oy).max(0).div_euclid(2), ((h - oy) / 2).min(hh));
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            let uv = (2 * x0 + ox, 2 * y0 + oy, 2 * x1 + ox, 2 * y1 + oy);
            self.quads.push(quad(Tex::Frame(3, 3), Mode::AddGlow, 0xffff_ffff, (x0, y0, x1, y1), uv));
        }
        self.quads.push(quad(Tex::None, Mode::RtEnd, 0, (0, 0, 0, 0), (0, 0, 0, 0)));
        let colour = 0xff00_0000 | ray[2] << 16 | ray[1] << 8 | ray[0];
        self.quads.push(quad(
            Tex::LightRt,
            Mode::Masked(Blend::Glow, Where::Is(SHADE)),
            colour,
            (0, 0, w, h),
            (0, 0, hw, hh),
        ));
    }
}
