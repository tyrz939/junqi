//! The ecotone (ART.md §3.1, "the grid never shows"): where two regions meet, the ground shifts
//! from one's ramps to the other's over some two dozen cells, not at the region map's 16-cell
//! staircase. Each region's share of the ground is the region map blurred by a tent two dozen
//! cells wide, sampled round a centre pushed about by a slow noise, so the band meanders and never
//! runs straight; the shares are worked at a lattice every [`LATTICE`] px and eased between. In
//! the band a px takes the second region's ramps by its share pushed by a clustered noise, so the
//! two grounds interleave in drifts and tongues of a few px to a few cells, each px of it either
//! side's colour or a quarter-step between them, and the drifts thin out toward either edge.
//!
//! Only the ground's ramps blend (`region::is_ground`); a wall or a roof keeps its cell's region
//! whole, so a house is one material. Pure in world px and the seed: two chunks agree on a seam.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::hash::hash2;

use super::field::{Field, smooth};
use super::{CELL, CHUNK_PX, TileSource};

/// Px between the shares' lattice points.
pub(crate) const LATTICE: i32 = 64;
/// Lattice points per chunk side (the chunk's edges both included).
const PTS: i32 = CHUNK_PX / LATTICE + 1;
/// Cells between the blur's samples, and samples each side of its centre: a tent of
/// `2 * STEP * (TAPS + 1)` cells, the band's width.
const STEP: i32 = 5;
const TAPS: i32 = 3;
/// The meander: the blur's centre pushed up to this many cells each way, by a noise with a
/// lattice every `2^MEANDER_SHIFT` px; then a finer push that rounds the staircase's corners.
const MEANDER: i32 = 11;
const MEANDER_SHIFT: u32 = 9;
const WIGGLE: i32 = 4;
const WIGGLE_SHIFT: u32 = 7;

mod salt {
    pub const MEANDER_X: u32 = 0x4543_4f58;
    pub const MEANDER_Y: u32 = 0x4543_4f59;
    pub const WIGGLE_X: u32 = 0x4543_5758;
    pub const WIGGLE_Y: u32 = 0x4543_5759;
    pub const DRIFT: u32 = 0x4543_4452;
    pub const GRAIN: u32 = 0x4543_4752;
    pub const GROUND_DRIFT: u32 = 0x4547_4452;
}

/// Smooth value noise at one world px, 0..=255: [`Field::at`]'s numbers without its box.
fn noise(x: i32, y: i32, shift: u32, salt: u32) -> i32 {
    let (gx, gy) = (x >> shift, y >> shift);
    let mask = (1 << shift) - 1;
    let (tx, ty) = (smooth(((x & mask) * 256) >> shift), smooth(((y & mask) * 256) >> shift));
    let v = |i: i32, j: i32| (hash2(gx + i, gy + j, salt) >> 24) as i32;
    let top = v(0, 0) * (256 - tx) + v(1, 0) * tx;
    let bottom = v(0, 1) * (256 - tx) + v(1, 1) * tx;
    (top * (256 - ty) + bottom * ty) >> 16
}

/// One chunk's region shares and the drift noise over it.
#[derive(Clone, Debug)]
pub(crate) struct Ecotone {
    /// Per lattice point, row-major `PTS x PTS`: each region's share, summing to 256.
    shares: Vec<[i32; 3]>,
    /// The chunk's one region when every share is whole, else `None`.
    pub whole: Option<u8>,
    drift: Field,
    grain: Field,
    /// The drift noise over the chunk, per px.
    nv: Vec<i32>,
}

impl Default for Ecotone {
    fn default() -> Self {
        Ecotone {
            shares: vec![[0; 3]; (PTS * PTS) as usize],
            whole: Some(0),
            drift: Field::default(),
            grain: Field::default(),
            nv: vec![0; (CHUNK_PX * CHUNK_PX) as usize],
        }
    }
}

impl Ecotone {
    /// Work the shares over the chunk whose top-left world cell is `(x0, y0)`.
    pub fn fill(&mut self, src: &impl TileSource, x0: i32, y0: i32, seed: u32) {
        let (px0, py0) = (x0 * CELL, y0 * CELL);
        let mut first = None;
        let mut mixed = false;
        for j in 0..PTS {
            for i in 0..PTS {
                let (wx, wy) = (px0 + i * LATTICE, py0 + j * LATTICE);
                let mx = (noise(wx, wy, MEANDER_SHIFT, seed ^ salt::MEANDER_X) - 128) * MEANDER * CELL / 128
                    + (noise(wx, wy, WIGGLE_SHIFT, seed ^ salt::WIGGLE_X) - 128) * WIGGLE * CELL / 128;
                let my = (noise(wx, wy, MEANDER_SHIFT, seed ^ salt::MEANDER_Y) - 128) * MEANDER * CELL / 128
                    + (noise(wx, wy, WIGGLE_SHIFT, seed ^ salt::WIGGLE_Y) - 128) * WIGGLE * CELL / 128;
                let (cx, cy) = ((wx + mx).div_euclid(CELL), (wy + my).div_euclid(CELL));
                let mut s = [0i32; 3];
                for b in -TAPS..=TAPS {
                    let wb = TAPS + 1 - b.abs();
                    for a in -TAPS..=TAPS {
                        let r = usize::from(src.region(cx + a * STEP, cy + b * STEP).min(2));
                        s[r] += wb * (TAPS + 1 - a.abs());
                    }
                }
                let top = s.iter().position(|&v| v == 256);
                match (first, top) {
                    (None, Some(r)) => first = Some(r),
                    (Some(f), Some(r)) if f == r => {}
                    _ => mixed = true,
                }
                self.shares[(j * PTS + i) as usize] = s;
            }
        }
        self.whole = if mixed { None } else { first.map(|r| r as u8) };
        if self.whole.is_some() {
            return;
        }
        // Drifts a cell or two across with a grain of a few px on their edges.
        self.drift.fill(px0, py0, CHUNK_PX, CHUNK_PX, 5, seed ^ salt::DRIFT);
        self.grain.fill(px0, py0, CHUNK_PX, CHUNK_PX, 3, seed ^ salt::GRAIN);
        self.nv.fill(0);
        self.drift.add_box(px0, py0, CHUNK_PX, CHUNK_PX, 6, &mut self.nv);
        self.grain.add_box(px0, py0, CHUNK_PX, CHUNK_PX, 2, &mut self.nv);
    }

    /// At chunk-local px `(x, y)` of a chunk that is not whole: the two regions with the most
    /// ground there, and how much of the second a px shows, 0..=256 in quarter steps.
    #[inline]
    pub fn mix(&self, x: i32, y: i32) -> (u8, u8, i32) {
        let (i, j) = (x / LATTICE, y / LATTICE);
        let tx = smooth(((x % LATTICE) * 256) / LATTICE);
        let ty = smooth(((y % LATTICE) * 256) / LATTICE);
        let k = (j * PTS + i) as usize;
        let n = PTS as usize;
        let (a, b, c, d) = (self.shares[k], self.shares[k + 1], self.shares[k + n], self.shares[k + n + 1]);
        let mut s = [0i32; 3];
        for r in 0..3 {
            let top = a[r] * (256 - tx) + b[r] * tx;
            let bottom = c[r] * (256 - tx) + d[r] * tx;
            s[r] = top * (256 - ty) + bottom * ty;
        }
        // The first and second by share (ties to the lower region).
        let mut first = 0;
        for r in 1..3 {
            if s[r] > s[first] {
                first = r;
            }
        }
        let mut second = usize::from(first == 0);
        for r in 0..3 {
            if r != first && s[r] > s[second] {
                second = r;
            }
        }
        let sum = s[first] + s[second];
        if s[second] == 0 || sum == 0 {
            return (first as u8, first as u8, 0);
        }
        // The second's share of the two, 0..=128, pushed by the drifts: hardly at the band's
        // edges, a whole region either way at its middle.
        let t = ((i64::from(s[second]) * 256) / i64::from(sum)) as i32;
        let bell = t * (256 - t) / 48;
        let noise = self.nv[(y * CHUNK_PX + x) as usize] / 8 - 128;
        let d = (t + noise * bell / 256).clamp(0, 256);
        (first as u8, second as u8, (d + 32) & !63)
    }
}

/// Cells of the ground blend's lattice each way: the chunk's and two round it, so every px of the
/// surface map (a cell round the chunk) has its four cell centres.
const GC: i32 = super::CHUNK_CELLS + 4;
/// The ground blend's reach, cells: a tent over `(2 * GROUND_TAPS + 1)^2` cells, which the
/// painter's [`super::REACH`] covers from the lattice's outer cells.
const GROUND_TAPS: i32 = 2;
const _: () = assert!(GROUND_TAPS + 2 <= super::REACH);
/// The tent's whole weight.
const TENT: i32 = (GROUND_TAPS + 1) * (GROUND_TAPS + 1) * (GROUND_TAPS + 1) * (GROUND_TAPS + 1);
/// A ground with less than this share of the cells round a px (of 256, eased between the cells'
/// centres so the test never cuts along a cell) is narrow there (a path, a strip of moss) and
/// neither drifts nor is drifted into.
const NARROW: i32 = 112;
/// Where a px's share of the other ground, pushed by the drift, turns it over (of 256).
const FLIP: i32 = 144;
/// How far the drift pushes a share: the larger, the shorter the tongues.
const REACH_DIV: i32 = 128;
/// The surface map's side, px, and where the chunk's px 0 is in it.
const MAP: i32 = super::MM;

/// Where two wild grounds meet (grass and earth, earth and moss): per cell round the chunk, the
/// two wild grounds most of its neighbourhood is and how much of each, and the drift noise; so a
/// px near the edge draws as the other ground in drifts that thin out over a couple of cells each
/// side, and a straight edge the land laid reads as a ragged margin, not a ruled line.
#[derive(Clone, Debug)]
pub(crate) struct GroundMix {
    /// Per cell `-2..=17` each way, row-major `GC x GC`: the two grounds with the most of its
    /// neighbourhood (a surface id, or `super::NONE`) and their shares, 0..=256.
    cells: Vec<[(u8, i32); 2]>,
    /// Per cell, the same lattice: laid by hand, kept whole.
    laid: Vec<bool>,
    /// The chunk's px the last [`GroundMix::apply`] turned over: chunk-local `(x, y)`, the
    /// ground it was and the ground it took.
    pub flips: Vec<(i16, i16, u8, u8)>,
    /// The drifts' noise, a lattice a cell apart, over the surface map; its top-left world px.
    drift: Field,
    mx: i32,
    my: i32,
    /// No cell of the chunk has two grounds near it.
    pub calm: bool,
}

impl Default for GroundMix {
    fn default() -> Self {
        GroundMix {
            cells: vec![[(super::NONE, 0); 2]; (GC * GC) as usize],
            laid: vec![false; (GC * GC) as usize],
            flips: Vec::new(),
            drift: Field::default(),
            mx: 0,
            my: 0,
            calm: true,
        }
    }
}

impl GroundMix {
    /// Work the shares from `surf(x, y)` (the surface of chunk-local cell `(x, y)`, up to
    /// `REACH` outside) and `blends(g)` (whether ground `g` drifts into its neighbours), over the
    /// chunk whose top-left world px is `(px0, py0)`.
    ///
    /// `laid(x, y)`: the cell's ground was laid by hand (a lane, a yard, a grown-over path): it
    /// neither drifts nor lends its ground to a drift, so a way keeps its edge and its fill.
    pub fn fill(
        &mut self,
        surf: impl Fn(i32, i32) -> u8,
        blends: impl Fn(u8) -> bool,
        laid: impl Fn(i32, i32) -> bool,
        px0: i32,
        py0: i32,
        seed: u32,
    ) {
        self.calm = true;
        for cj in 0..GC {
            for ci in 0..GC {
                let (x, y) = (ci - 2, cj - 2);
                self.laid[(cj * GC + ci) as usize] = laid(x, y);
                // At most a few grounds: tally them in a small list.
                let mut tally = [(super::NONE, 0i32); 4];
                for b in -GROUND_TAPS..=GROUND_TAPS {
                    let wb = GROUND_TAPS + 1 - b.abs();
                    for a in -GROUND_TAPS..=GROUND_TAPS {
                        let o = surf(x + a, y + b);
                        if !blends(o) || laid(x + a, y + b) {
                            continue;
                        }
                        let w = wb * (GROUND_TAPS + 1 - a.abs());
                        if let Some(t) = tally.iter_mut().find(|t| t.0 == o || t.0 == super::NONE) {
                            t.0 = o;
                            t.1 += w;
                        }
                    }
                }
                tally.sort_by_key(|t| -t.1);
                let two = [(tally[0].0, tally[0].1 * 256 / TENT), (tally[1].0, tally[1].1 * 256 / TENT)];
                self.calm &= two[1].1 == 0;
                self.cells[(cj * GC + ci) as usize] = two;
            }
        }
        if self.calm {
            return;
        }
        (self.mx, self.my) = (px0 - CELL, py0 - CELL);
        self.drift.fill(self.mx, self.my, MAP, MAP, 4, seed ^ salt::GROUND_DRIFT);
    }

    /// Lay the drifts over the surface map `mm` (`MAP x MAP` px, the chunk's px 0 at `CELL`):
    /// each px of a ground `blend` names takes the other ground where the drift says, and `hit`
    /// (`GC x GC`, cells `-2..=17`) is set for every cell a px of which changed.
    pub fn apply(&mut self, mm: &mut [u8], blend: &[bool; 256], hit: &mut [bool]) {
        hit.fill(false);
        self.flips.clear();
        if self.calm {
            return;
        }
        // A block of px between four cells' centres at a time: its corners are fixed, so each
        // ground's share is a bilinear of four numbers.
        for cy in 0..GC - 1 {
            for cx in 0..GC - 1 {
                let k = |a: i32, b: i32| self.cells[(b * GC + a) as usize];
                let corners = [k(cx, cy), k(cx + 1, cy), k(cx, cy + 1), k(cx + 1, cy + 1)];
                if corners.iter().all(|c| c[1].1 == 0 && c[0].0 == corners[0][0].0) {
                    continue;
                }
                // The grounds any corner names, with their four shares.
                let mut grounds = [(super::NONE, [0i32; 4]); 8];
                let mut n = 0;
                for (q, c) in corners.iter().enumerate() {
                    for e in c {
                        if e.0 == super::NONE || e.1 == 0 {
                            continue;
                        }
                        let at = match grounds[..n].iter().position(|g| g.0 == e.0) {
                            Some(at) => at,
                            None => {
                                grounds[n].0 = e.0;
                                n += 1;
                                n - 1
                            }
                        };
                        grounds[at].1[q] = e.1;
                    }
                }
                if n < 2 {
                    continue;
                }
                // Chunk-local px of the block: from the centre of cell `cx - 2` to that of `cx - 1`.
                let (x0, y0) = ((cx - 2) * CELL + CELL / 2, (cy - 2) * CELL + CELL / 2);
                for fy in 0..CELL {
                    let y = y0 + fy;
                    if !(-CELL..MAP - CELL).contains(&y) {
                        continue;
                    }
                    for fx in 0..CELL {
                        let x = x0 + fx;
                        if !(-CELL..MAP - CELL).contains(&x) {
                            continue;
                        }
                        let i = ((y + CELL) * MAP + x + CELL) as usize;
                        let g = mm[i];
                        if !blend[usize::from(g)]
                            || self.laid[((y.div_euclid(CELL) + 2) * GC + x.div_euclid(CELL) + 2) as usize]
                        {
                            continue;
                        }
                        let bil = |s: [i32; 4]| {
                            ((s[0] * (CELL - fx) + s[1] * fx) * (CELL - fy) + (s[2] * (CELL - fx) + s[3] * fx) * fy)
                                / (CELL * CELL)
                        };
                        let (mut own, mut o, mut t) = (0, g, 0);
                        for &(h, s) in &grounds[..n] {
                            let v = bil(s);
                            if h == g {
                                own = v;
                            } else if v > t {
                                (o, t) = (h, v);
                            }
                        }
                        // A narrow ground (a path, a strip of moss) is mostly its neighbours round
                        // it: it keeps its shape, and a broad one drifts by the lesser share, so
                        // only its margin frays.
                        if own < NARROW || t == 0 {
                            continue;
                        }
                        let t = t.min(own);
                        // Rare out at the reach's edge, in tongues and drifts nearer the line.
                        let bell = t * (256 - t) / 64;
                        if t + 127 * bell / REACH_DIV < FLIP {
                            continue;
                        }
                        let noise = self.drift.at(self.mx + x + CELL, self.my + y + CELL) - 128;
                        if t + noise * bell / REACH_DIV >= FLIP {
                            mm[i] = o;
                            if (0..CHUNK_PX).contains(&x) && (0..CHUNK_PX).contains(&y) {
                                self.flips.push((x as i16, y as i16, g, o));
                            }
                            hit[((y.div_euclid(CELL) + 2) * GC + x.div_euclid(CELL) + 2) as usize] = true;
                        }
                    }
                }
            }
        }
    }
}

/// Cells of [`GroundMix::apply`]'s `hit` each way.
pub(crate) const HIT_SIDE: i32 = GC;

#[cfg(test)]
mod tests {
    use super::*;

    /// West of x = 100 cells the Lowfields, east of it the Works.
    struct Split;
    impl TileSource for Split {
        fn size(&self) -> (i32, i32) {
            (200, 200)
        }
        fn tile(&self, _x: i32, _y: i32) -> jane_core::Tile {
            jane_core::Tile::Grass
        }
        fn region(&self, x: i32, _y: i32) -> u8 {
            if x < 100 { 0 } else { 2 }
        }
    }

    #[test]
    fn the_band_is_wide_and_whole_far_from_it() {
        let mut e = Ecotone::default();
        e.fill(&Split, 0, 0, 7);
        assert_eq!(e.whole, Some(0), "40 cells west of the line is all Lowfields");
        e.fill(&Split, 160, 0, 7);
        assert_eq!(e.whole, Some(2));
        // Across the line, row by row: some of each within a few cells of it, and the Works' share
        // rising from west to east over more than a dozen cells.
        let mut works = Vec::new();
        for cx in [80, 96] {
            e.fill(&Split, cx, 48, 7);
            assert!(e.whole.is_none());
            for x in 0..CHUNK_PX {
                let n = (0..CHUNK_PX)
                    .filter(|&y| {
                        let (a, b, t) = e.mix(x, y);
                        a == 2 || b == 2 && t >= 128
                    })
                    .count();
                works.push(n);
            }
        }
        eprintln!("{:?}", works.chunks(16).map(|c| c.iter().sum::<usize>() / 16).collect::<Vec<_>>());
        let head = works[..64].iter().sum::<usize>();
        let tail = works[works.len() - 64..].iter().sum::<usize>();
        assert!(head < tail, "{head} {tail}");
        assert!(works.iter().filter(|&&n| n > 0 && n < CHUNK_PX as usize).count() > 12 * CELL as usize);
    }

    #[test]
    fn two_chunks_agree_on_their_seam() {
        let (mut a, mut b) = (Ecotone::default(), Ecotone::default());
        a.fill(&Split, 80, 48, 3);
        b.fill(&Split, 96, 48, 3);
        // The lattice's last column of `a` is the first of `b`.
        for y in 0..CHUNK_PX {
            let (ra, sa, _) = a.mix(CHUNK_PX - 1, y);
            let (rb, sb, _) = b.mix(0, y);
            assert!(ra == rb || sa == sb || ra == sb, "{y}");
        }
        for j in 0..PTS {
            assert_eq!(a.shares[(j * PTS + PTS - 1) as usize], b.shares[(j * PTS) as usize]);
        }
    }
}
