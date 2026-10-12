//! The display list's room (PSP v0.4.1 plan, E). The GE glue writes a frame into a 256 KB list and
//! `sceGuGetMemory` checks nothing, so a frame that does not fit is memory written past the list
//! (the owner's crash: 589 to 629 KB of list in a wood, mostly polygons, past both 256 KB lists
//! and onto the GU's own statics).
//! This is what the glue asks before it writes: what a frame's quads cost in the list (a bound,
//! near the truth), and which of its optional passes it leaves out when they would not fit,
//! first to last ([`Shed`]). Pure and integer, so a PC test reads what the PSP would write.
//!
//! What it counts is what `ge.rs`'s draw loop writes: a batch is the run of quads with the same
//! texture ([`batch_key`]: every polygon is one) and mode; it takes its texture's commands when the
//! texture changes, its mode's when the mode does, then a jump over its vertices and the draw.

use alloc::vec::Vec;

use super::{Mode, Poly, Quad, Strip, StripTex, Tex, slices};
use crate::capture::pass as P;

/// Bytes of the commands that bind a texture, at most: 21 words (the page, its CLUT through the
/// grade, the filter and the wrap put back).
pub const BIND: usize = 84;
/// Bytes of the commands that set a mode, at most: 19 words (the stencil and the pixel mask put
/// back, the blend).
pub const MODE: usize = 76;
/// A draw: `sceGuGetMemory`'s jump over the vertices (8 bytes) and `sceGuDrawArray`'s four
/// commands (16).
pub const DRAW: usize = 24;
/// A move into the lightmap's target (its buffer, scissor, clear rect and draw) and back.
pub const RT_BEGIN: usize = 160;
pub const RT_END: usize = 64;
/// A flat vertex (a colour and a 16-bit position) and a textured one, bytes.
pub const FLAT: usize = 12;
pub const TEXTURED: usize = 16;
/// The most vertices one polygon draw takes (the draw's count is 16 bits; a multiple of 3).
pub const POLY_VERTS: usize = 3072;
/// The pass a quad before the first mark belongs to.
pub const NO_PASS: u8 = u8::MAX;

/// What names a batch's texture: the polygons all one (they differ in which polygon, not in what
/// they sample), so a run of them is one batch and one draw.
pub fn batch_key(t: Tex) -> Tex {
    match t {
        Tex::Poly(_) => Tex::Poly(0),
        t => t,
    }
}

/// Whether `b` joins the batch `a` begins: the same texture ([`batch_key`]) and mode.
pub fn same_batch(a: &Quad, b: &Quad) -> bool {
    a.mode == b.mode && batch_key(a.tex) == batch_key(b.tex)
}

/// A sun shadow's block polygon (the bulk of a wood's silhouettes).
pub fn is_block_shadow(q: &Quad) -> bool {
    matches!(q.tex, Tex::Poly(_)) && q.mode == Mode::ShadowBand
}

/// A polygon marked in the lightmap target's stencil: a light's shadow of a block or a caster.
pub fn is_lamp_mark(q: &Quad) -> bool {
    matches!(q.tex, Tex::Poly(_)) && q.mode == Mode::StencilMark
}

/// The pass quad `i` belongs to (`capture::pass`), and the quad its range ends at (`usize::MAX`
/// for the last): `marks` as the lister left them, `(first quad, pass)` in order.
pub fn pass_at(marks: &[(u32, u8)], i: usize) -> (u8, usize) {
    let k = marks.partition_point(|m| m.0 as usize <= i);
    let id = if k == 0 { NO_PASS } else { marks[k - 1].1 };
    (id, marks.get(k).map_or(usize::MAX, |m| m.0 as usize))
}

/// What a frame leaves out when its list would not fit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shed {
    /// Passes left out whole: bit `id` of `capture::pass`.
    pub passes: u32,
    /// The lights' shadow polygons left out of the lightmap (the lights still light).
    pub lamp_polys: bool,
    /// The sun's block polygons left out (its casters' bands stay).
    pub sun_polys: bool,
    /// Quarters of the sun's bands left out, the faintest (the pass lays them strongest first):
    /// 0 to 3. A frame's own run of them is in [`Shed::cut`] ([`rung`] resolves it).
    pub trim: u8,
    /// The quads `[from, to)` that `trim` leaves out: the last of the sun's bands.
    pub cut: (u32, u32),
    /// How far down the ladder this is (0: nothing).
    pub level: u8,
}

/// The ladder's rungs: [`Shed::level`] runs `1..=LEVELS`.
pub const LEVELS: u8 = 9;

impl Shed {
    pub const NONE: Shed = Shed { passes: 0, lamp_polys: false, sun_polys: false, trim: 0, cut: (0, 0), level: 0 };

    /// Whether anything is left out.
    pub fn any(&self) -> bool {
        self.passes != 0 || self.lamp_polys || self.sun_polys || self.trim > 0
    }

    /// The rungs, each adding to the one before, the least missed first. The polygons first, the
    /// bulk of a list that once overflowed: a light's shadows of blocks (a lantern among a wood of
    /// stand-in blocks marked 8,100 of them in one frame, 589 KB of list; by day they are not
    /// seen), then the sun's. Then the sun's bands by quarters, the faintest first (a wood's
    /// shadows are most of its quads: 6,956 of 7,322 at the densest frame found), and the rest of
    /// the pass with the shafts that read its stencil; what glows; the air (particles, the water's
    /// shimmer and its reflections); the fog. Terrain, sprites and the lightmap's pools are never
    /// left out here: a frame past them is cut at the glue's limit. The bands' `cut` is the
    /// frame's own ([`rung`]).
    pub fn level(n: u8) -> Shed {
        let bit = |p: u8| 1u32 << p;
        let mut s = Shed {
            passes: 0,
            lamp_polys: n >= 1,
            sun_polys: n >= 2,
            trim: n.saturating_sub(2).min(3),
            cut: (0, 0),
            level: n.min(LEVELS),
        };
        if n >= 6 {
            s.passes |= bit(P::SUN_SHADOWS) | bit(P::SHAFTS);
        }
        if n >= 7 {
            s.passes |= bit(P::GLOW);
        }
        if n >= 8 {
            s.passes |= bit(P::PARTICLES) | bit(P::SHIMMER) | bit(P::WATER_FX);
        }
        if n >= 9 {
            s.passes |= bit(P::FOG);
        }
        s
    }

    /// Whether quad `q` of pass `pass` is left out.
    pub fn skips(&self, pass: u8, q: &Quad) -> bool {
        (pass < 32 && self.passes >> pass & 1 != 0)
            || (self.sun_polys && pass == P::SUN_SHADOWS && is_block_shadow(q))
            || (self.lamp_polys && pass == P::LIGHTMAP && is_lamp_mark(q))
    }
}

/// The run of the sun's bands in `quads`: the flat shadow quads that end the sun's pass (its clear,
/// its raised marks and its polygons come before them), `[from, to)`; `None` for a frame without.
pub fn sun_bands(quads: &[Quad], marks: &[(u32, u8)]) -> Option<(usize, usize)> {
    let k = marks.iter().position(|m| m.1 == P::SUN_SHADOWS)?;
    let from = marks[k].0 as usize;
    let to = marks.get(k + 1).map_or(quads.len(), |m| m.0 as usize).min(quads.len());
    let band = |q: &Quad| q.tex == Tex::None && q.mode == Mode::ShadowBand;
    let first = (from..to).find(|&i| band(&quads[i]))?;
    (first..to).all(|i| band(&quads[i])).then_some((first, to))
}

/// Rung `n` of the ladder as this frame has it: [`Shed::level`] with the sun's bands' `cut` found.
/// A rung that trims a frame without bands takes nothing more than the one before.
pub fn rung(n: u8, quads: &[Quad], marks: &[(u32, u8)]) -> Shed {
    let mut s = Shed::level(n);
    if s.trim > 0
        && let Some((a, b)) = sun_bands(quads, marks)
    {
        let keep = (b - a) * usize::from(4 - s.trim) / 4;
        s.cut = ((a + keep) as u32, b as u32);
    }
    s
}

/// The first quad at or after `i` that `shed` leaves in (`quads.len()` when none is left).
pub fn first_kept(quads: &[Quad], marks: &[(u32, u8)], shed: &Shed, mut i: usize) -> usize {
    if !shed.any() {
        return i;
    }
    while i < quads.len() {
        // The bands' faintest quarters go to their end.
        if i >= shed.cut.0 as usize && i < shed.cut.1 as usize {
            i = shed.cut.1 as usize;
            continue;
        }
        let (id, end) = pass_at(marks, i);
        if !shed.skips(id, &quads[i]) {
            break;
        }
        // A pass left out whole goes to its end; a polygon, one quad at a time.
        i = if id < 32 && shed.passes >> id & 1 != 0 { end.min(quads.len()) } else { i + 1 };
    }
    i.min(quads.len())
}

/// Where a batch that begins at quad `i` must end at the latest when passes are timed or left
/// out: the pass's end, or where its bands' faintest quarters begin.
pub fn stop_at(marks: &[(u32, u8)], shed: &Shed, i: usize) -> usize {
    let end = pass_at(marks, i).1;
    if i < shed.cut.0 as usize && shed.cut.0 < shed.cut.1 { end.min(shed.cut.0 as usize) } else { end }
}

/// The end of the batch that begins at quad `i`: the quads after it that [`same_batch`], not
/// past `stop` (the next pass's first quad, when passes are timed or left out).
pub fn batch_end(quads: &[Quad], i: usize, stop: usize) -> usize {
    let mut j = i + 1;
    while j < quads.len() && j < stop && same_batch(&quads[i], &quads[j]) {
        j += 1;
    }
    j
}

/// The vertices polygon `q` takes as triangles (the fan `p0, pk, pk+1` of each of its corners
/// past the second); none for a polygon of less than three corners.
pub fn poly_verts(q: &Quad, polys: &[Poly]) -> usize {
    let Tex::Poly(pi) = q.tex else { return 0 };
    polys.get(usize::from(pi)).map_or(0, |&(pts, n)| {
        let n = usize::from(n).min(pts.len());
        if n >= 3 { 3 * (n - 2) } else { 0 }
    })
}

/// The bytes a batch `run` (quads of one [`batch_key`] and mode, not the lightmap target's moves)
/// writes past its binds and the first [`DRAW`]: its vertices, and the draws it takes more.
pub fn payload(run: &[Quad], polys: &[Poly], strips: &[Strip], scratch: &mut Vec<Quad>) -> usize {
    let Some(first) = run.first() else { return 0 };
    let n = run.len();
    match first.tex {
        Tex::None | Tex::Line(_) => n * 2 * FLAT,
        Tex::Poly(_) => {
            let verts: usize = run.iter().map(|q| poly_verts(q, polys)).sum();
            verts * FLAT + verts.div_ceil(POLY_VERTS).saturating_sub(1) * DRAW
        }
        Tex::Strip(k) => strips
            .get(usize::from(k))
            .map_or(0, |s| usize::from(s.len) * if s.tex == StripTex::Mist { TEXTURED } else { FLAT }),
        // Each read-back quad is cut into slices and has a draw of its own.
        Tex::Frame(..) | Tex::LightRt => {
            let mut bytes = 0;
            for q in run {
                slices(q, scratch);
                bytes += scratch.len() * 2 * TEXTURED;
            }
            bytes + (n - 1) * DRAW
        }
        _ => n * 2 * TEXTURED,
    }
}

/// The bytes of display list `quads` take, the kept ones only ([`Shed`]): a bound the glue's
/// real writes stay under (every bind and mode change charged its most), and within a fifth of
/// them on a frame with a few hundred batches (measured: 86 to 97 of a hundred). `timed` is
/// whether passes are timed: the glue then writes a signal at each mark ([`SIGNAL`] bytes) and
/// ends a batch there, as it does where a pass is left out.
pub fn cost(
    quads: &[Quad],
    marks: &[(u32, u8)],
    polys: &[Poly],
    strips: &[Strip],
    shed: &Shed,
    timed: bool,
    scratch: &mut Vec<Quad>,
) -> usize {
    let stops = timed || shed.any();
    let mut bytes = if timed { SIGNAL * marks.len() } else { 0 };
    let (mut bound, mut mode): (Option<Tex>, Option<Mode>) = (None, None);
    let mut i = first_kept(quads, marks, shed, 0);
    while i < quads.len() {
        let q = quads[i];
        if q.mode == Mode::RtBegin || q.mode == Mode::RtEnd {
            bytes += if q.mode == Mode::RtBegin { RT_BEGIN } else { RT_END };
            (bound, mode) = (None, None);
            i = first_kept(quads, marks, shed, i + 1);
            continue;
        }
        let stop = if stops { stop_at(marks, shed, i) } else { usize::MAX };
        let j = batch_end(quads, i, stop);
        if bound != Some(batch_key(q.tex)) {
            bytes += BIND;
            bound = Some(batch_key(q.tex));
        }
        if mode != Some(q.mode) {
            bytes += MODE;
            mode = Some(q.mode);
        }
        bytes += DRAW + payload(&quads[i..j], polys, strips, scratch);
        i = first_kept(quads, marks, shed, j);
    }
    bytes
}

/// A signal (the pass timing's): two commands.
pub const SIGNAL: usize = 8;

/// A frame's list as planned: what it leaves out, its bytes then, and its bytes whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub shed: Shed,
    pub bytes: usize,
    pub whole: usize,
}

impl Plan {
    /// Whether the frame did not fit whole.
    pub fn cut(&self) -> bool {
        self.whole > self.bytes || self.shed.any()
    }
}

/// The least [`Shed`] under which `quads` fit `budget` bytes of list, or the last rung if none
/// does (the glue then stops at its own limit).
pub fn plan(
    quads: &[Quad],
    marks: &[(u32, u8)],
    polys: &[Poly],
    strips: &[Strip],
    timed: bool,
    budget: usize,
    scratch: &mut Vec<Quad>,
) -> Plan {
    let whole = cost(quads, marks, polys, strips, &Shed::NONE, timed, scratch);
    let mut plan = Plan { shed: Shed::NONE, bytes: whole, whole };
    let mut level = 0;
    while plan.bytes > budget && level < LEVELS {
        level += 1;
        let shed = rung(level, quads, marks);
        plan = Plan { shed, bytes: cost(quads, marks, polys, strips, &shed, timed, scratch), whole };
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(tex: Tex, mode: Mode) -> Quad {
        Quad { tex, mode, colour: 0xffff_ffff, x0: 0, y0: 0, x1: 8, y1: 8, u0: 0, v0: 0, u1: 8, v1: 8 }
    }

    fn square() -> Poly {
        let mut p = [(0i16, 0i16); 8];
        p[..4].copy_from_slice(&[(0, 0), (8, 0), (8, 8), (0, 8)]);
        (p, 4)
    }

    /// A frame of the kinds the glue draws, as the lister lays them: terrain; the sun's shadows (a
    /// clear, `sun` block polygons, a few bands); sprites; the lightmap (a target, a clear, `lamp`
    /// polygons marked, a pool); glow; particles. Its polygon table, the sun's first.
    struct Fixture {
        quads: Vec<Quad>,
        marks: Vec<(u32, u8)>,
        polys: Vec<Poly>,
    }

    impl Fixture {
        fn new(sun: usize, lamp: usize) -> Fixture {
            Fixture::with_bands(sun, lamp, 2)
        }

        /// As [`Fixture::new`] with `bands` of the sun's flat shadow quads after its polygons.
        fn with_bands(sun: usize, lamp: usize, bands: usize) -> Fixture {
            let mut quads = Vec::new();
            let mut marks = Vec::new();
            marks.push((quads.len() as u32, P::TERRAIN));
            quads.push(q(Tex::Chunk(0), Mode::Opaque));
            quads.push(q(Tex::Chunk(1), Mode::Opaque));
            marks.push((quads.len() as u32, P::SUN_SHADOWS));
            quads.push(q(Tex::None, Mode::StencilClear));
            for i in 0..sun {
                quads.push(q(Tex::Poly(i as u16), Mode::ShadowBand));
            }
            for _ in 0..bands {
                quads.push(q(Tex::None, Mode::ShadowBand));
            }
            marks.push((quads.len() as u32, P::SPRITES));
            for _ in 0..40 {
                quads.push(q(Tex::Page(3), Mode::Alpha));
            }
            marks.push((quads.len() as u32, P::LIGHTMAP));
            quads.push(q(Tex::None, Mode::RtBegin));
            quads.push(q(Tex::None, Mode::StencilClear));
            for i in 0..lamp {
                quads.push(q(Tex::Poly((sun + i) as u16), Mode::StencilMark));
            }
            quads.push(q(Tex::Pool, Mode::PoolLit));
            quads.push(q(Tex::None, Mode::RtEnd));
            marks.push((quads.len() as u32, P::GLOW));
            quads.push(q(Tex::Glow(3), Mode::AddGlow));
            marks.push((quads.len() as u32, P::PARTICLES));
            quads.push(q(Tex::Line(true), Mode::Alpha));
            quads.push(q(Tex::Line(true), Mode::Alpha));
            Fixture { quads, marks, polys: vec![square(); sun + lamp] }
        }

        fn cost(&self, shed: Shed, timed: bool) -> usize {
            cost(&self.quads, &self.marks, &self.polys, &[], &shed, timed, &mut Vec::new())
        }

        fn plan(&self, budget: usize) -> Plan {
            plan(&self.quads, &self.marks, &self.polys, &[], false, budget, &mut Vec::new())
        }

        fn mark(&self, pass: u8) -> usize {
            self.marks.iter().find(|m| m.1 == pass).map(|m| m.0 as usize).expect("a mark")
        }

        /// Rung `n` of the ladder as this frame has it.
        fn rung(&self, n: u8) -> Shed {
            rung(n, &self.quads, &self.marks)
        }
    }

    #[test]
    fn every_polygon_is_one_batch_and_one_draw() {
        let (few, many) = (Fixture::new(1, 0), Fixture::new(300, 0));
        // 300 squares are 6 vertices each in one draw: each past the first costs its vertices alone.
        assert_eq!(
            many.cost(Shed::NONE, false) - few.cost(Shed::NONE, false),
            299 * 6 * FLAT,
            "no bind, mode or draw more for a polygon"
        );
        assert!(same_batch(&q(Tex::Poly(1), Mode::ShadowBand), &q(Tex::Poly(9), Mode::ShadowBand)));
        assert!(!same_batch(&q(Tex::Poly(1), Mode::ShadowBand), &q(Tex::Poly(1), Mode::StencilMark)));
        assert_eq!(batch_end(&many.quads, 3, usize::MAX), 3 + 300);
    }

    #[test]
    fn a_frame_that_fits_is_left_whole() {
        let f = Fixture::new(200, 100);
        let p = f.plan(usize::MAX);
        assert_eq!((p.shed, p.bytes), (Shed::NONE, p.whole));
        assert!(!p.cut());
        assert_eq!(p.whole, f.cost(Shed::NONE, false));
    }

    #[test]
    fn a_lanterns_8000_block_shadows_give_way_before_anything_the_player_sees() {
        // The frames that ended the program: a lantern among a wood of blocks marks four polygons
        // for each. The lights' polygons go first, whatever the sun's pass holds.
        let f = Fixture::new(300, 8_100);
        assert!(f.plan(usize::MAX).whole > 500_000, "half a megabyte of list for a 256 KB list");
        let p = f.plan(200 * 1024);
        assert_eq!(p.shed, f.rung(1));
        assert!(p.shed.lamp_polys && !p.shed.sun_polys && p.shed.passes == 0);
        assert!(p.bytes <= 200 * 1024 && p.cut());
    }

    #[test]
    fn a_wood_of_sun_polygons_gives_up_those_then_the_suns_pass_then_the_rest_in_order() {
        let f = Fixture::new(2000, 0);
        // 2000 squares are 144 KB of vertices: against 100 KB the sun's polygons go (the lights'
        // rung gives nothing, and is passed over).
        let p = f.plan(100 * 1024);
        assert_eq!(p.shed.level, 2);
        assert!(p.shed.sun_polys && p.shed.passes == 0 && p.shed.trim == 0, "{:?}", p.shed);
        assert!(p.bytes <= 100 * 1024 && p.cut());
        // Less than the sprites and the chunks need: the ladder runs to its end and stops there.
        let p = f.plan(64);
        assert_eq!(p.shed, f.rung(LEVELS));
        assert!(p.bytes > 64, "what is left is the glue's to cut");
        // Each rung costs less than the one before, never more.
        let costs: Vec<usize> = (0..=LEVELS).map(|n| f.cost(f.rung(n), false)).collect();
        assert!(costs.windows(2).all(|w| w[1] <= w[0]), "{costs:?}");
        // The rungs, in order: lights' polygons, the sun's, a quarter of its bands, a half, three
        // quarters; the pass whole and the shafts, glow, the air, the fog; each adds to the one before.
        let bit = |n: u8, p: u8| Shed::level(n).passes >> p & 1 == 1;
        assert!(Shed::level(1).lamp_polys && !Shed::level(1).sun_polys && Shed::level(1).passes == 0);
        assert!(Shed::level(2).sun_polys && Shed::level(2).trim == 0 && Shed::level(2).passes == 0);
        assert_eq!((3..=5).map(|n| Shed::level(n).trim).collect::<Vec<_>>(), [1, 2, 3]);
        assert!((3..=5).all(|n| Shed::level(n).passes == 0));
        assert!(bit(6, P::SUN_SHADOWS) && bit(6, P::SHAFTS) && !bit(6, P::GLOW));
        assert!(bit(7, P::GLOW) && !bit(7, P::PARTICLES));
        assert!(bit(8, P::PARTICLES) && bit(8, P::SHIMMER) && bit(8, P::WATER_FX) && !bit(8, P::FOG));
        assert!(bit(9, P::FOG));
        for n in 0..LEVELS {
            let (a, b) = (Shed::level(n), Shed::level(n + 1));
            assert_eq!(a.passes & b.passes, a.passes);
            assert!(!a.lamp_polys || b.lamp_polys);
            assert!(!a.sun_polys || b.sun_polys);
            assert!(a.trim <= b.trim);
            assert_eq!(b.level, n + 1);
        }
        // The terrain, the sprites and the lightmap are never left out.
        let last = Shed::level(LEVELS);
        for pass in [P::TERRAIN, P::SPRITES, P::LIGHTMAP, P::LIGHT_MUL, P::SKY, P::GRADE, P::UI] {
            assert_eq!(last.passes >> pass & 1, 0, "pass {pass}");
        }
    }

    /// A wood's shadows are most of its quads (6,956 of 7,322 at the densest frame found): a list
    /// that cannot quite hold them loses the faintest quarter, then a half, then three quarters
    /// of the bands before it loses the pass, and the quads it keeps are the strongest.
    #[test]
    fn a_wood_of_shadow_bands_loses_its_faintest_quarters_before_the_pass() {
        let f = Fixture::with_bands(40, 0, 4000);
        let (a, b) = sun_bands(&f.quads, &f.marks).expect("the bands");
        assert_eq!(b - a, 4000, "the run after the polygons, to the pass's end");
        let whole = f.cost(Shed::NONE, false);
        // 4,000 bands are 96 KB of vertices: a quarter is 24 KB.
        let rungs: Vec<Plan> = [10, 30, 55].iter().map(|kb| f.plan(whole - kb * 1024)).collect();
        assert_eq!(rungs.iter().map(|p| p.shed.level).collect::<Vec<_>>(), [3, 4, 5], "a quarter, a half, three");
        for (p, t) in rungs.iter().zip(1u8..=3) {
            assert_eq!(p.shed.trim, t);
            let cut = p.shed.cut;
            assert_eq!(
                cut,
                ((a + 4000 * usize::from(4 - t) / 4) as u32, b as u32),
                "the last {t} quarters of the bands"
            );
            assert!(p.bytes <= whole);
            // Skipped from the cut to its end; before it kept; a batch before it ends at it.
            assert_eq!(first_kept(&f.quads, &f.marks, &p.shed, cut.0 as usize), cut.1 as usize);
            assert_eq!(first_kept(&f.quads, &f.marks, &p.shed, cut.0 as usize - 1), cut.0 as usize - 1);
            assert_eq!(stop_at(&f.marks, &p.shed, a), cut.0 as usize);
            assert_eq!(
                stop_at(&f.marks, &p.shed, cut.1 as usize),
                f.marks.iter().find(|m| m.1 == P::LIGHTMAP).map_or(usize::MAX, |m| m.0 as usize)
            );
        }
        // A quarter taken off is exactly a quarter of the bands' vertices: the cut costs no batch.
        assert_eq!(f.cost(f.rung(2), false) - f.cost(f.rung(3), false), 1000 * 2 * FLAT);
        // Past three quarters the pass goes whole.
        let p = f.plan(whole - 90 * 1024);
        assert_eq!(p.shed.level, 6, "{:?}", p.shed);
        // A frame without bands passes the trims over, each rung as the one before.
        let none = Fixture::with_bands(40, 0, 0);
        assert_eq!(sun_bands(&none.quads, &none.marks), None);
        assert_eq!(none.rung(4).cut, (0, 0));
    }

    #[test]
    fn what_is_left_out_is_skipped_whole_and_the_batches_do_not_cross_it() {
        let f = Fixture::new(10, 6);
        let (quads, marks) = (&f.quads, &f.marks);
        // Through glow: the sun's pass is skipped to the first sprite, glow to the particles.
        let shed = Shed::level(7);
        assert_eq!(first_kept(quads, marks, &shed, 2), f.mark(P::SPRITES));
        assert_eq!(first_kept(quads, marks, &shed, f.mark(P::GLOW)), f.mark(P::PARTICLES));
        // The polygons only: the clears stay, the polygons go, the bands and the pools stay.
        let polys_only = Shed::level(2);
        assert_eq!(first_kept(quads, marks, &polys_only, 2), 2, "the sun's clear is kept");
        assert_eq!(first_kept(quads, marks, &polys_only, 3), 3 + 10, "ten of the sun's polygons skipped");
        let lamp0 = f.mark(P::LIGHTMAP) + 2;
        assert_eq!(first_kept(quads, marks, &polys_only, lamp0), lamp0 + 6, "six of the lights' skipped");
        assert_eq!(quads[lamp0 + 6].mode, Mode::PoolLit);
        // A lamp polygon is its pass's alone: the same mode outside the lightmap is kept.
        assert!(Shed::level(1).skips(P::LIGHTMAP, &q(Tex::Poly(0), Mode::StencilMark)));
        assert!(!Shed::level(1).skips(P::SPRITES, &q(Tex::Poly(0), Mode::StencilMark)));
        assert!(!Shed::level(1).skips(P::SUN_SHADOWS, &q(Tex::Poly(0), Mode::ShadowBand)));
        // A batch stops at the next pass when passes are left out.
        let a = q(Tex::Page(3), Mode::Alpha);
        let three = vec![a, a, a];
        assert_eq!(batch_end(&three, 0, 2), 2);
        assert_eq!(batch_end(&three, 0, usize::MAX), 3);
        // Left out, the frame costs less; timed, each mark is a signal more.
        assert!(f.cost(shed, false) < f.cost(Shed::NONE, false));
        assert!(f.cost(Shed::NONE, true) >= f.cost(Shed::NONE, false) + SIGNAL * marks.len());
    }

    /// Frames of a random shape, as the lister lays them (its marks, a pass replacing the mark at
    /// its own first quad): whatever the budget, a plan is the first rung that fits it, or the last.
    #[test]
    fn a_plan_is_the_first_rung_that_fits_the_budget_or_the_last_on_any_frame() {
        let mut state = 0x2545_f491u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let order = [
            P::TERRAIN,
            P::SUN_SHADOWS,
            P::SPRITES,
            P::LIGHTMAP,
            P::SHAFTS,
            P::GLOW,
            P::FOG,
            P::PARTICLES,
            P::SHIMMER,
            P::WATER_FX,
            P::SATURATION,
            P::GRADE,
        ];
        let (mut fits, mut spent) = (0, 0);
        for round in 0..400 {
            let (mut quads, mut marks) = (Vec::new(), Vec::<(u32, u8)>::new());
            let polys = vec![square(); 64];
            for _ in 0..3 + next() % 8 {
                let id = order[next() as usize % order.len()];
                let at = quads.len() as u32;
                match marks.last_mut() {
                    Some(m) if m.0 == at => m.1 = id,
                    _ => marks.push((at, id)),
                }
                for _ in 0..next() % 120 {
                    let poly = (next() % 64) as u16;
                    quads.push(match (id, next() % 3) {
                        (P::SUN_SHADOWS, 0) => q(Tex::Poly(poly), Mode::ShadowBand),
                        (P::SUN_SHADOWS, _) => q(Tex::None, Mode::ShadowBand),
                        (P::LIGHTMAP, 0) => q(Tex::Poly(poly), Mode::StencilMark),
                        (P::LIGHTMAP, 1) => q(Tex::Pool, Mode::PoolLit),
                        (P::LIGHTMAP, _) => q(Tex::None, Mode::StencilClear),
                        (P::SPRITES | P::TERRAIN, k) => q(Tex::Page(k as u16), Mode::Alpha),
                        (P::GLOW, _) => q(Tex::Glow(1), Mode::AddGlow),
                        (P::SATURATION | P::GRADE, _) => {
                            Quad { x0: 0, x1: 480, u1: 480, ..q(Tex::Frame(0, 3), Mode::Lut(1)) }
                        }
                        (_, k) => q(Tex::Line(k == 0), Mode::Alpha),
                    });
                }
            }
            // From a few hundred bytes to 128 KB, evenly by powers of two.
            let budget = (1usize << (8 + next() % 10)) + (next() as usize) % 256;
            let timed = round % 2 == 0;
            let mut scratch = Vec::new();
            let p = plan(&quads, &marks, &polys, &[], timed, budget, &mut scratch);
            let bytes = |shed: Shed| cost(&quads, &marks, &polys, &[], &shed, timed, &mut Vec::new());
            assert_eq!(p.whole, bytes(Shed::NONE), "round {round}");
            assert_eq!(p.bytes, bytes(p.shed), "round {round}: the plan's bytes are its rung's");
            if p.bytes <= budget {
                fits += 1;
                // The first rung that fits: none before it did.
                for n in 0..p.shed.level {
                    assert!(
                        bytes(rung(n, &quads, &marks)) > budget,
                        "round {round}: rung {n} fits, so {} is not the first",
                        p.shed.level
                    );
                }
            } else {
                spent += 1;
                assert_eq!(
                    p.shed,
                    rung(LEVELS, &quads, &marks),
                    "round {round}: over the budget on a rung short of the last"
                );
            }
        }
        assert!(fits > 50 && spent > 50, "the generator covers both: {fits} fit, {spent} spent the ladder");
    }

    #[test]
    fn a_wide_read_back_costs_its_slices() {
        let wide = Quad { x0: 0, x1: 480, u0: 0, u1: 480, ..q(Tex::Frame(0, 4), Mode::Lut(0)) };
        let mut s = Vec::new();
        // 30 slices of two 16-byte vertices, and the draw's own commands.
        let c = cost(&[wide], &[], &[], &[], &Shed::NONE, false, &mut s);
        assert_eq!(c, BIND + MODE + DRAW + 30 * 2 * TEXTURED);
        // The lightmap target's moves are charged whole and reset the binds.
        let rt = [q(Tex::None, Mode::RtBegin), q(Tex::None, Mode::RtEnd)];
        assert_eq!(cost(&rt, &[], &[], &[], &Shed::NONE, false, &mut s), RT_BEGIN + RT_END);
    }

    #[test]
    fn a_polygon_with_fewer_than_three_corners_draws_nothing() {
        let line = ([(0i16, 0i16); 8], 2u8);
        assert_eq!(poly_verts(&q(Tex::Poly(0), Mode::ShadowBand), &[line]), 0);
        assert_eq!(poly_verts(&q(Tex::Poly(0), Mode::ShadowBand), &[square()]), 6);
        let mut hexagon = square();
        hexagon.1 = 6;
        assert_eq!(poly_verts(&q(Tex::Poly(0), Mode::ShadowBand), &[hexagon]), 12);
        // A polygon not in the table is none.
        assert_eq!(poly_verts(&q(Tex::Poly(5), Mode::ShadowBand), &[square()]), 0);
    }

    #[test]
    fn a_great_many_polygons_take_a_draw_for_each_3072_vertices() {
        let quads: Vec<Quad> = (0..1000).map(|i| q(Tex::Poly(i), Mode::ShadowBand)).collect();
        let polys = vec![square(); 1000];
        let mut s = Vec::new();
        let p = payload(&quads, &polys, &[], &mut s);
        // 6000 vertices: two draws (3072 + 2928), so one more than the first's.
        assert_eq!(p, 6000 * FLAT + DRAW);
    }
}
