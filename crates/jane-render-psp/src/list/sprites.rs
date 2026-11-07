//! A sprite's quads and the ground over its feet: a presenter sprite as its trimmed rect on its
//! PSP page (bent rows as strips, PORT.md §13.12), the terrain patch laid back over what stands
//! behind it, and the chunks' relief and water as rects of cells (PLAN-PSP-SPEED.md W1).

use alloc::vec::Vec;

use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, SpriteCmd, Tint};

use super::{Lister, Mode, PATCH_WORDS, PATCHES_MOST, Patch, Quad, Tex, abgr, fx, rgba};

/// The largest patch laid: a sprite past it is drawn whole over the terrain.
const PATCH_MOST: i32 = 256;
// A patch is read from the relief cells alone (`relief_cells`, over `shadow::GROUND`): terrain
// that hides a foot stands over `Foot::RELIEF`, higher still.
const _: () = assert!((jane_present::shadow::GROUND as u8) < jane_present::Foot::RELIEF);

/// Most rects a chunk's relief is laid in; past it, one span a row of cells (still only rows
/// with relief, from their first raised cell to their last).
const RELIEF_RECTS_MOST: usize = 24;

/// A rect of a chunk's cells, `(x0, y0, x1, y1)`.
pub(super) type CellRect = (u8, u8, u8, u8);

/// A row of cells' bits laid as spans of cells, `(x0, x1)`, each handed on.
type Spans<'a> = dyn Fn(u32, &mut dyn FnMut(u8, u8)) + 'a;

/// The cells of a chunk's height layer (`CHUNK_PX` square, a byte a px) with any px over
/// `shadow::GROUND` (what `Mode::RaisedMark`'s CLUT marks), as rects of cells `(x0, y0, x1, y1)`:
/// each row's runs, a run joined to the one above it when they span the same cells.
pub(super) fn relief_cells(height: &[u8], out: &mut Vec<CellRect>) {
    // Four px a word: a px over 4 is one of 8 or more (a high bit) or 5 to 7 (bit 2 and bit 0
    // or 1), tested a byte at a time inside the word with no carry between them.
    const _: () = assert!(jane_present::shadow::GROUND == 4);
    let raised = |w: u32| w & 0xf8f8_f8f8 != 0 || (w >> 2) & (w | w >> 1) & 0x0101_0101 != 0;
    let (n, cells, cell) =
        (CHUNK_PX as usize, jane_present::frame::CHUNK_CELLS as usize, jane_present::frame::CELL as usize);
    out.clear();
    if height.len() < n * n {
        out.push((0, 0, cells as u8, cells as u8));
        return;
    }
    let mut rows = [0u32; 16];
    for (cy, row) in rows.iter_mut().enumerate().take(cells) {
        for y in cy * cell..(cy + 1) * cell {
            let line = &height[y * n..(y + 1) * n];
            for (cx, px) in line.chunks_exact(cell).enumerate() {
                if *row >> cx & 1 == 0
                    && px.chunks_exact(4).any(|b| raised(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
                {
                    *row |= 1 << cx;
                }
            }
        }
    }
    lay_cells(&rows, out);
}

/// The cells of a `T8` chunk whose CLUT marks any px of them as water (`frame::T8_WATER`, what
/// the water's `Mode::Mark` keeps), as rects of cells as [`relief_cells`] lays them: the water
/// passes' tight bounds (PLAN-PSP-SPEED.md W1).
pub(super) fn water_cells(l: &jane_present::ChunkLayers, out: &mut Vec<CellRect>) {
    let (n, cells, cell) =
        (CHUNK_PX as usize, jane_present::frame::CHUNK_CELLS as usize, jane_present::frame::CELL as usize);
    out.clear();
    if !l.is_t8() || l.albedo.len() * 4 < n * n || l.clut.len() < 256 {
        out.push((0, 0, cells as u8, cells as u8));
        return;
    }
    // The CLUT's water entries, a bit each.
    let mut wet = [0u64; 4];
    for (i, &c) in l.clut[..256].iter().enumerate() {
        if (c >> 24) as u8 == jane_present::frame::T8_WATER {
            wet[i / 64] |= 1 << (i % 64);
        }
    }
    let mut rows = [0u32; 16];
    if wet != [0; 4] {
        let is = |ix: u32| wet[(ix >> 6) as usize] >> (ix & 63) & 1 != 0;
        // Four px a word, a cell's row four words.
        let (words, per) = (n / 4, cell / 4);
        for (cy, row) in rows.iter_mut().enumerate().take(cells) {
            for y in cy * cell..(cy + 1) * cell {
                let line = &l.albedo[y * words..(y + 1) * words];
                for (cx, px) in line.chunks_exact(per).enumerate() {
                    if *row >> cx & 1 == 0
                        && px.iter().any(|&w| is(w & 0xff) || is(w >> 8 & 0xff) || is(w >> 16 & 0xff) || is(w >> 24))
                    {
                        *row |= 1 << cx;
                    }
                }
            }
        }
    }
    lay_cells(&rows, out);
}

/// A chunk's rows of cells (a bit a cell) laid as rects of cells `(x0, y0, x1, y1)`: each row's
/// runs, a run joined to the one above it when they span the same cells; past
/// [`RELIEF_RECTS_MOST`], one span a row.
fn lay_cells(rows: &[u32; 16], out: &mut Vec<CellRect>) {
    let cells = jane_present::frame::CHUNK_CELLS as usize;
    let runs = |m: u32, f: &mut dyn FnMut(u8, u8)| {
        let mut x = 0;
        while x < cells {
            if m >> x & 1 == 0 {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < cells && m >> x & 1 != 0 {
                x += 1;
            }
            f(x0 as u8, x as u8);
        }
    };
    let lay = |spans: &Spans<'_>, out: &mut Vec<CellRect>| {
        out.clear();
        // The rects still open: those ending at the row above.
        for (cy, &m) in rows.iter().enumerate().take(cells) {
            let y = cy as u8;
            spans(m, &mut |x0, x1| match out.iter_mut().find(|r| r.3 == y && r.0 == x0 && r.2 == x1) {
                Some(r) => r.3 = y + 1,
                None => out.push((x0, y, x1, y + 1)),
            });
        }
    };
    lay(&runs, out);
    if out.len() > RELIEF_RECTS_MOST {
        let span = |m: u32, f: &mut dyn FnMut(u8, u8)| {
            if m != 0 {
                f(m.trailing_zeros() as u8, (32 - m.leading_zeros()) as u8);
            }
        };
        lay(&span, out);
    }
}

impl Lister {
    /// A sprite's quads: its trimmed rect on its PSP page, by strips where it bends.
    pub(super) fn sprite(&mut self, s: &SpriteCmd, lights: &[jane_present::Light], index: u32) {
        let Some(i) = self.find(s.page, s.src.x, s.src.y) else {
            self.misses += 1;
            return;
        };
        let Some(t) = self.targets[i] else {
            self.misses += 1;
            return;
        };
        let (rx, ry) = self.rects[i];
        // The command's rect in the presenter sprite's own px, and the trimmed rect in them.
        let (lx0, ly0) = (i32::from(s.src.x) - i32::from(rx), i32::from(s.src.y) - i32::from(ry));
        let (sw, sh) = (i32::from(s.src.w), i32::from(s.src.h));
        let (tx, ty) = (i32::from(t.tx), i32::from(t.ty));
        let ix0 = lx0.max(tx);
        let ix1 = (lx0 + sw).min(tx + i32::from(t.w));
        let iy0 = ly0.max(ty);
        let iy1 = (ly0 + sh).min(ty + i32::from(t.h));
        if ix0 >= ix1 || iy0 >= iy1 {
            return;
        }
        let (mode, colour) = match s.flags.tint {
            Tint::None => (Mode::Alpha, 0xffff_ffff),
            Tint::Ghost(a) => (Mode::Alpha, u32::from(a) << 24 | 0x00ff_ffff),
            Tint::Flash(a) => (Mode::Add, rgba([a, a, a], 255)),
            // One texel in two on the canvas's checker, here half of each (PORT.md §13.12).
            Tint::Seen => (Mode::Alpha, 0x80ff_ffff),
        };
        let base =
            Quad { tex: Tex::Page(t.page), mode, colour, x0: 0, y0: 0, x1: 0, y1: 0, u0: 0, v0: 0, u1: 0, v1: 0 };
        let first = self.quads.len();
        let mirror = s.flags.mirror;
        // Canvas x of the drawn columns' left edge before any bend.
        let x = i32::from(s.x) + if mirror { lx0 + sw - ix1 } else { ix0 - lx0 };
        let u = i32::from(t.u) + ix0 - tx;
        let bend = s.flags.bend;
        // Rows `iy0..iy1` in runs of one shift (a row's index is from the command's top).
        let mut r0 = iy0;
        while r0 < iy1 {
            let shift = bend.shift(r0 - ly0);
            let mut r1 = r0 + 1;
            while r1 < iy1 && bend.shift(r1 - ly0) == shift {
                r1 += 1;
            }
            let y = i32::from(s.y) + r0 - ly0;
            let v = i32::from(t.v) + r0 - ty;
            self.push(base, x + shift, y, ix1 - ix0, r1 - r0, u, v, mirror);
            r0 = r1;
        }
        // What glows on it, laid over the light later: the same quads through the glow CLUT.
        if self.effects & fx::GLOW != 0
            && s.flags.tint == Tint::None
            && self.has_glow.get(usize::from(t.page)).copied().unwrap_or(false)
        {
            for k in first..self.quads.len() {
                let q = self.quads[k];
                self.glows.push(Quad { tex: Tex::Glow(t.page), mode: Mode::AddGlow, colour: 0xffff_ffff, ..q });
            }
        }
        // Its relief: the same quads again over its normal page, through the CLUT of the lamp
        // that lights it most if one does, else the sun's.
        if s.flags.tint == Tint::None && self.has_normals.get(usize::from(t.page)).copied().unwrap_or(false) {
            let clut = self.lamp_relief(lights, s, index).or(self.relief.map(|_| u16::MAX));
            if let Some(c) = clut {
                for k in first..self.quads.len() {
                    let q = self.quads[k];
                    self.quads.push(Quad {
                        tex: Tex::Normal(t.page, c),
                        mode: Mode::Multiply2,
                        colour: 0xffff_ffff,
                        ..q
                    });
                }
            }
        }
    }

    /// Each placed chunk's relief cells (`relief_rects`) read for its generation, once a frame:
    /// where the sun's pass marks raised terrain, and the only cells a patch can come from (L4).
    pub(super) fn relief_fresh(&mut self, frame: &Frame) {
        if self.relief_read == Some(self.placed.len()) {
            return;
        }
        self.relief_read = Some(self.placed.len());
        for k in 0..self.placed.len() {
            let slot = self.placed[k].2;
            let Some(l) = frame.layers.get(usize::from(slot)).filter(|l| l.has_height()) else { continue };
            let generation = self.chunks.iter().find(|c| c.0 == slot).map(|c| c.1);
            let s = usize::from(slot);
            if self.relief_rects.len() <= s {
                self.relief_rects.resize_with(s + 1, || None);
            }
            if generation.is_none() || self.relief_rects[s].as_ref().is_none_or(|r| Some(r.0) != generation) {
                let mut rects = self.relief_rects[s].take().map(|r| r.1).unwrap_or_default();
                relief_cells(&l.height, &mut rects);
                self.relief_rects[s] = Some((generation.unwrap_or(u32::MAX), rects));
            }
        }
    }

    /// Lays back over sprite `s` the terrain in front of its feet (`Foot::hides`), as a patch
    /// of the chunks' px over its rect: what `soft` leaves out of the sprite, drawn over it. Her
    /// `Tint::Seen` pass, drawn later, shows through it.
    pub(super) fn patch(&mut self, frame: &Frame, s: &SpriteCmd, f: jane_present::Foot) {
        let reach = s.flags.bend.reach();
        let (x0, y0) = ((i32::from(s.x) - reach).max(0), i32::from(s.y).max(0));
        let x1 = (i32::from(s.x) + i32::from(s.src.w) + reach).min(self.w);
        let y1 = (i32::from(s.y) + i32::from(s.src.h)).min(self.h);
        let (w, h) = (x1 - x0, y1 - y0);
        if w <= 0 || h <= 0 || w > PATCH_MOST || h > PATCH_MOST {
            return;
        }
        let (tw, th) = ((w as u32).next_power_of_two().max(4), (h as u32).next_power_of_two());
        // Its rows in the arena, after the last patch's: each a multiple of 4 words (a texture's
        // buffer width), the patch 16-byte aligned. A patch past the count or the arena is not
        // laid (its sprite's feet show through what hides them), but only counted as left out
        // once something does hide them: found by the same scan, written or not.
        let stride = (w as usize).next_multiple_of(4);
        let at = self.patch_px.len().next_multiple_of(4);
        let words = stride * h as usize;
        let room = self.patches.len() < PATCHES_MOST && at + words <= PATCH_WORDS && self.arena(at + words);
        if room {
            self.patch_px.resize(at + words, 0);
        }
        let mut any = false;
        // Only the chunks' cells with relief (L4): a px no higher than `shadow::GROUND` hides
        // nothing (`Foot::hides` asks for more than `Foot::RELIEF`), and most cells are flat.
        self.relief_fresh(frame);
        'scan: for &(cx, cy, slot) in &self.placed {
            let Some(l) = frame.layers.get(usize::from(slot)) else { continue };
            if !l.has_height() {
                continue;
            }
            let Some(Some((_, rects))) = self.relief_rects.get(usize::from(slot)) else { continue };
            let cell = |v: u8| i32::from(v) * jane_present::frame::CELL;
            for &(rx0, ry0, rx1, ry1) in rects {
                let (ax0, ax1) = (x0.max(cx + cell(rx0)), x1.min(cx + cell(rx1)));
                let (ay0, ay1) = (y0.max(cy + cell(ry0)), y1.min(cy + cell(ry1)));
                for y in ay0..ay1 {
                    let row = ((y - cy) * CHUNK_PX) as usize;
                    for x in ax0..ax1 {
                        let k = row + (x - cx) as usize;
                        if jane_present::Foot::hides(l.height[k], y, i32::from(f.y)) {
                            any = true;
                            if !room {
                                break 'scan;
                            }
                            let c = if l.is_t8() { l.t8_abgr(k) } else { abgr(l.albedo[k]) };
                            self.patch_px[at + (y - y0) as usize * stride + (x - x0) as usize] = c | 0xff00_0000;
                        }
                    }
                }
            }
        }
        if !any || !room {
            if room {
                // Nothing hid its feet: the room it was given goes back.
                self.patch_px.truncate(at.min(self.patch_px.len()));
            } else if any {
                self.patches_skipped += 1;
            }
            return;
        }
        let i = self.patches.len() as u16;
        self.patches.push(Patch {
            at: at as u32,
            w: w as u16,
            h: h as u16,
            stride: stride as u16,
            tw: tw as u16,
            th: th as u16,
        });
        self.quads.push(Quad {
            tex: Tex::Patch(i),
            mode: Mode::Alpha,
            colour: 0xffff_ffff,
            x0: x0 as i16,
            y0: y0 as i16,
            x1: x1 as i16,
            y1: y1 as i16,
            u0: 0,
            v0: 0,
            u1: w as u16,
            v1: h as u16,
        });
    }

    /// Pushes `w x h` px at canvas `(x, y)` from texels `(u, v)` (mirrored: `u..u + w` laid right
    /// to left), clipped to the canvas.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push(&mut self, mut q: Quad, x: i32, y: i32, w: i32, h: i32, u: i32, v: i32, mirror: bool) {
        let (cx0, cy0) = (x.max(0), y.max(0));
        let (cx1, cy1) = ((x + w).min(self.w), (y + h).min(self.h));
        if cx0 >= cx1 || cy0 >= cy1 {
            return;
        }
        let v0 = v + (cy0 - y);
        let v1 = v0 + (cy1 - cy0);
        let (u0, u1) = if mirror {
            // Canvas column x + k shows texel u + w - 1 - k.
            let first = u + w - (cx0 - x);
            (first, first - (cx1 - cx0))
        } else {
            let first = u + (cx0 - x);
            (first, first + (cx1 - cx0))
        };
        q.x0 = cx0 as i16;
        q.y0 = cy0 as i16;
        q.x1 = cx1 as i16;
        q.y1 = cy1 as i16;
        q.u0 = u0 as u16;
        q.u1 = u1 as u16;
        q.v0 = v0 as u16;
        q.v1 = v1 as u16;
        self.quads.push(q);
    }
}
