//! The draw list's tests: what `soft` draws, laid as the GE lays it.

use super::*;
use super::{lightmap::*, sprites::*, sun::*};
use crate::pack::Pack;
use jane_present::atlas::SpriteRef;
use jane_present::frame::{CHUNK_PX, Src};
use jane_present::{Frame, Pass, SpriteCmd};

fn sref(page: u8, x: u16, y: u16, w: u16, h: u16, ax: i16, ay: i16) -> SpriteRef {
    SpriteRef { page, src: Src { x, y, w, h }, ax, ay, height: 1, top: 1, base: 0, base_height: 0, burn: (-1, -1) }
}

/// Two refs on one shelf, one on the next; the first trimmed 2 px in from its left and 3 from
/// its top on the PSP, at (100, 50) of page 4.
fn lister() -> Lister {
    use crate::pack::{Rec, RefLink};
    let refs = [sref(0, 0, 0, 10, 12, 5, 12), sref(0, 10, 0, 8, 8, 4, 8), sref(0, 0, 12, 6, 6, 3, 6)];
    let rec = |page, u, v, w, h| Rec { cat: 2, frame: 0, sprite: 0, vs: 0, page, u, v, w, h, ax: 0, ay: 0 };
    let pack = Pack {
        recs: alloc::vec![rec(4, 100, 50, 6, 9), rec(5, 0, 0, 8, 8), rec(u16::MAX, 0, 0, 0, 0)],
        refs: alloc::vec![
            RefLink { rec: 0, ax: 3, ay: 9 },
            RefLink { rec: 1, ax: 4, ay: 8 },
            RefLink { rec: 2, ax: 0, ay: 0 }
        ],
        ..Pack::default()
    };
    Lister::new(&refs, &pack)
}

#[test]
fn the_atmosphere_is_drawn_as_soft_draws_it_and_each_pass_can_be_left_out() {
    use jane_present::Span;
    use jane_present::frame::{FogVolume, PartShape, Particle, SkyLook, StarCmd, WaterCmd};
    let mut f = Frame::new(jane_present::Tier::T0);
    f.canvas = (480, 272);
    let sky = SkyLook {
        zenith: [20, 30, 80],
        horizon: [200, 140, 100],
        glow: [255, 140, 60],
        glow_x: 100,
        glow_amount: 0,
        stars: 255,
        star_list: Span { start: 0, len: 1 },
        moon: None,
        zone: (0, 40, 480, 272),
        tick: 0,
    };
    f.stars.push(StarCmd { x: 10, up: 20, bright: 200 });
    f.passes.push(Pass::Sky(sky));
    f.water.push(WaterCmd { x: 100, y: 100, phase: 0 });
    f.passes.push(Pass::Water { cells: Span { start: 0, len: 1 } });
    let part = |shape| Particle { x: 200, y: 150, shape, colour: [200, 210, 230], alpha: 200, glow: 0, height: 0 };
    f.parts.extend([
        part(PartShape::Streak { dx: -3, dy: -12 }),
        part(PartShape::Ring { r: 5 }),
        part(PartShape::Dot { size: 2 }),
    ]);
    f.passes.push(Pass::Particles { layer: jane_present::Depth::Weather, parts: Span { start: 0, len: 3 } });
    f.fog.push(FogVolume { rect: (-64, -64, 544, 336), edge: 16, density: 120, colour: [200, 200, 210], top: 0 });
    f.passes.push(Pass::Fog { volumes: Span { start: 0, len: 1 }, drift: (3, 0) });
    let mut l = lister();
    let q = l.build(&f).to_vec();
    let count = |t: fn(&Tex) -> bool| q.iter().filter(|q| t(&q.tex)).count();
    // The sky's rows above the zone's top (40 px: five strips); the fog, its edges off the
    // canvas, one.
    assert_eq!(l.strips.iter().filter(|s| s.tex == StripTex::Flat).count(), 5);
    assert_eq!(l.strips.iter().filter(|s| s.tex == StripTex::Mist).count(), 1);
    // The star, the glint and the dot are flat quads; the streak fades, the ring does not.
    assert_eq!(count(|t| *t == Tex::None), 3);
    assert_eq!(count(|t| *t == Tex::Line(true)), 1);
    assert_eq!(count(|t| *t == Tex::Line(false)), 12);
    // The fog's CLUT is never clear (a haze under the wisps), at most 230 of 256.
    assert!(l.fog_clut.iter().all(|&c| c >> 24 > 0 && c >> 24 <= 230));
    l.atmos_off = atmos_fx::SKY | atmos_fx::FOG | atmos_fx::PARTICLES | atmos_fx::WATER;
    l.build(&f);
    assert!(l.quads.is_empty() && l.strips.is_empty());
}

#[test]
fn a_wide_read_back_is_sliced_where_px_and_texels_are_whole() {
    let base = Quad {
        tex: Tex::LightRt,
        mode: Mode::Multiply2Opaque,
        colour: 0,
        x0: -1,
        y0: -1,
        x1: 479,
        y1: 271,
        u0: 0,
        v0: 0,
        u1: 240,
        v1: 136,
    };
    let mut out = Vec::new();
    slices(&base, &mut out);
    assert_eq!(out.len(), 15);
    assert!(out.iter().all(|q| q.u1 - q.u0 <= SLICE_TEXELS as u16));
    assert!(out.windows(2).all(|w| w[0].x1 == w[1].x0 && w[0].u1 == w[1].u0));
    assert!(out.iter().all(|q| (q.x1 - q.x0) as i32 * 240 == (q.u1 - q.u0) as i32 * 480), "the same scale");
    assert_eq!((out[0].x0, out[14].x1, out[14].u1), (-1, 479, 240));
    // 1:1 (the grade), 2:1 down (the saturation's half size), and a narrow one left whole.
    let grade = Quad { tex: Tex::Frame(0, 4), x0: 0, x1: 480, u0: 0, u1: 480, ..base };
    slices(&grade, &mut out);
    assert_eq!(out.len(), 30);
    let half = Quad { tex: Tex::Frame(0, 0), x0: 0, x1: 240, u0: 0, u1: 480, ..base };
    slices(&half, &mut out);
    assert!(out.iter().all(|q| q.u1 - q.u0 == 16 && q.x1 - q.x0 == 8));
    let thin = Quad { x0: 0, x1: 32, u0: 0, u1: 16, ..base };
    slices(&thin, &mut out);
    assert_eq!(out, [thin]);
}

#[test]
fn a_rect_finds_the_ref_it_lies_in() {
    let l = lister();
    assert_eq!(l.find(0, 0, 0), Some(0));
    assert_eq!(l.find(0, 9, 11), Some(0));
    assert_eq!(l.find(0, 10, 0), Some(1));
    assert_eq!(l.find(0, 12, 5), Some(1));
    assert_eq!(l.find(0, 2, 13), Some(2));
    assert_eq!(l.find(1, 0, 0), None);
}

fn cmd(src: Src, x: i16, y: i16, mirror: bool) -> SpriteCmd {
    SpriteCmd {
        page: 0,
        src,
        x,
        y,
        flags: jane_present::Flags { mirror, ..jane_present::Flags::default() },
        height_px: 0,
        foot: None,
    }
}

#[test]
fn a_trimmed_sprite_lands_where_its_px_were() {
    let mut l = lister();
    l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, false), &[], 0);
    // Trim (2, 3): px (2, 3) of the presenter's rect is texel (100, 50).
    assert_eq!(
        l.quads,
        [Quad {
            tex: Tex::Page(4),
            mode: Mode::Alpha,
            colour: 0xffff_ffff,
            x0: 22,
            y0: 33,
            x1: 28,
            y1: 42,
            u0: 100,
            v0: 50,
            u1: 106,
            v1: 59
        }]
    );
    // Mirrored: column k of the 10 wide rect shows px 9 - k; px 2..8 land at 20 + 2..20 + 8.
    l.quads.clear();
    l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, true), &[], 0);
    let q = l.quads[0];
    assert_eq!((q.x0, q.x1, q.u0, q.u1), (22, 28, 106, 100));
    // Clipped at the canvas's left: the first 3 columns drawn go.
    l.quads.clear();
    l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, -5, 30, false), &[], 0);
    let q = l.quads[0];
    // Px 2..8 at -5 + 2.. -5 + 8 = -3..3: columns 0..3 show texels 103..106.
    assert_eq!((q.x0, q.x1, q.u0, q.u1), (0, 3, 103, 106));
    l.quads.clear();
    l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, -5, 30, true), &[], 0);
    let q = l.quads[0];
    // Mirrored px 2..8 at -5 + 2.. -5 + 8 = -3..3: columns 0..3 show texels 102, 101, 100.
    assert_eq!((q.x0, q.x1, q.u0, q.u1), (0, 3, 103, 100));
}

#[test]
fn a_block_swept_along_the_sun_is_a_hexagon() {
    // A 10 x 4 footprint, 16 px tall, the sun throwing a px of height a px west and up.
    let b = jane_present::Block { x0: 20, y0: 30, x1: 30, y1: 34, height: 16, lo: 0, fence: false };
    let (pts, n) = block_poly(&b, (-256, -256), (480, 272)).expect("it casts");
    assert_eq!(n, 6);
    let mut got: Vec<(i16, i16)> = pts[..6].to_vec();
    got.sort_unstable();
    assert_eq!(got, [(4, 14), (4, 18), (14, 14), (20, 34), (30, 30), (30, 34)]);
    // Too low to cast: nothing.
    assert!(block_poly(&jane_present::Block { height: 3, ..b }, (-256, -256), (480, 272)).is_none());
}

fn quad_at(k: u16) -> Quad {
    Quad { tex: Tex::None, mode: Mode::Alpha, colour: 0, x0: k as i16, y0: 0, x1: 1, y1: 1, u0: 0, v0: 0, u1: 0, v1: 0 }
}

/// The program once ended on a buffer's growth to 14,506 quads (464,192 bytes, one block of a
/// heap whose largest was 204 KB). A buffer grows by what the heap gives, to a most, and drops.
#[test]
fn a_quad_buffer_grows_to_its_most_and_leaves_out_the_rest() {
    let mut b = QuadBuf::with_capacity(4);
    for k in 0..QUADS_MOST as u16 + 5 {
        b.push(quad_at(k));
    }
    assert_eq!((b.len(), b.dropped, b.capacity()), (QUADS_MOST, 5, QUADS_MOST));
    assert_eq!(b[QUADS_MOST - 1], quad_at(QUADS_MOST as u16 - 1), "what is held is the first");
    // Appended: what fits goes, the rest is counted, the other is emptied.
    let mut a = QuadBuf::with_capacity(8);
    let mut other = QuadBuf::with_capacity(8);
    for k in 0..QUADS_MOST as u16 - 2 {
        a.push(quad_at(k));
    }
    for k in 0..5 {
        other.push(quad_at(k));
    }
    other.dropped = 7;
    a.append(&mut other);
    assert_eq!((a.len(), a.dropped, other.len(), other.dropped), (QUADS_MOST, 3 + 7, 0, 0));
    // It reads and compares as the slice it holds.
    let mut c = QuadBuf::default();
    c.push(quad_at(9));
    assert_eq!(c, [quad_at(9)]);
    assert_eq!((&c).into_iter().count(), 1);
    assert_eq!(c.take_dropped(), 0);
}

/// A frame's patches (PSP v0.4.1 plan, E): 500 sprites whose feet a wall hides, over three
/// frames. They lay in one arena that never passes its words, nor its count, and never moves.
#[test]
fn patches_share_one_arena_that_holds_to_its_cap_and_does_not_move() {
    use jane_present::{ChunkLayers, Foot, Tier};
    let mut l = lister();
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    let mut layer = ChunkLayers::new(Tier::T0);
    // The chunk's top 120 rows are walls 40 px tall: they stand in front of feet at row 2.
    for k in 0..120 * CHUNK_PX as usize {
        layer.height[k] = 40;
        layer.albedo[k] = 0xff20_4060;
    }
    f.layers.push(layer);
    let foot = Foot::at(2, false);
    let sprite = |k: i16| cmd(Src { x: 0, y: 0, w: 10, h: 12 }, (k * 3) % 240, 10 + (k % 8) * 13, false);
    let mut capacity = None;
    for round in 0..3 {
        l.build(&f);
        assert!(l.patches.is_empty() && l.patch_px.is_empty(), "a frame begins with none");
        l.placed.push((0, 0, 0));
        for k in 0..500 {
            l.patch(&f, &sprite(k), foot);
        }
        assert_eq!(l.patches.len(), PATCHES_MOST, "round {round}");
        assert_eq!(l.patches_skipped as usize, 500 - PATCHES_MOST);
        assert!(l.patch_px.len() <= PATCH_WORDS);
        // Rows a multiple of 4 words, each patch 16-byte aligned, in order, none overlapping.
        let mut end = 0;
        for p in &l.patches {
            assert_eq!((p.at % 4, p.stride % 4), (0, 0));
            assert!(p.at as usize >= end, "{p:?} over the one before");
            assert!(p.stride >= p.w && p.tw >= p.w && p.th >= p.h);
            end = p.at as usize + usize::from(p.stride) * usize::from(p.h);
        }
        assert_eq!(end, l.patch_px.len(), "the arena holds the patches and nothing else");
        // The quads name them in order, one each; the first holds the wall's colour and a
        // clear px past its rect's right edge in the padding.
        let named: Vec<u16> =
            l.quads.iter().filter_map(|q| if let Tex::Patch(i) = q.tex { Some(i) } else { None }).collect();
        assert_eq!(named, (0..PATCHES_MOST as u16).collect::<Vec<_>>());
        let p = l.patches[0];
        assert_eq!(l.patch_px[p.at as usize] >> 24, 0xff);
        assert_eq!(l.patch_px[p.at as usize + usize::from(p.stride) - 1] >> 24, 0, "the padding is clear");
        let cap = l.patch_px.capacity();
        assert_eq!(*capacity.get_or_insert(cap), cap, "the arena did not move in round {round}");
        assert!((PATCH_FIRST..=PATCH_WORDS).contains(&cap));
    }
    assert_eq!(l.patch_bytes(), (1, l.patch_px.capacity() * 4));
    assert_eq!(l.left_out().2 as usize, 500 - PATCHES_MOST);
}

#[test]
fn a_patch_over_nothing_takes_no_room_and_one_past_the_arena_is_left_out() {
    use jane_present::{ChunkLayers, Foot, Tier};
    let mut l = lister();
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    let mut layer = ChunkLayers::new(Tier::T0);
    // Walls only in the corner at rows 200..272, columns 0..256.
    for y in 200..CHUNK_PX as usize {
        for x in 0..CHUNK_PX as usize {
            layer.height[y * CHUNK_PX as usize + x] = 40;
        }
    }
    f.layers.push(layer);
    l.build(&f);
    l.placed.push((0, 0, 0));
    // Nothing hides its feet at the top of the chunk: no patch, no room taken, nothing counted.
    l.patch(&f, &cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 10, false), Foot::at(150, false));
    assert!((l.patches.len(), l.patch_px.len(), l.patches_skipped) == (0, 0, 0));
    // One a wall hides: laid.
    l.patch(&f, &cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 205, false), Foot::at(205, false));
    assert_eq!((l.patches.len(), l.patch_px.len()), (1, 12 * 12));
    // A sprite as wide as the chunk and half as tall cannot fit the arena: counted, not laid.
    let wide = cmd(Src { x: 0, y: 0, w: 256, h: 140 }, 0, 130, false);
    l.patch(&f, &wide, Foot::at(205, false));
    assert_eq!((l.patches.len(), l.patches_skipped), (1, 1));
    assert!(l.patch_px.len() <= PATCH_WORDS);
}

/// A wood of stand-in blocks with a lantern among them once marked 8,100 polygons in a frame
/// (the frame that ended the program held 14,506 quads).
#[test]
fn a_wood_of_blocks_lays_no_more_polygons_than_the_cap() {
    use jane_present::{Block, Directional, Span, Tier};
    let mut l = lister();
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    for k in 0..3000i16 {
        f.blocks.push(Block {
            x0: (k % 100) * 4,
            y0: 20 + (k / 100) * 8,
            x1: (k % 100) * 4 + 3,
            y1: 26 + (k / 100) * 8,
            height: 30,
            lo: 0,
            fence: false,
        });
    }
    let sun = Directional {
        azimuth: jane_core::Angle(9000),
        elevation: jane_core::Angle(4000),
        colour: [255, 240, 200],
        spread: 100,
        strength: 200,
    };
    f.passes.push(Pass::Silhouettes {
        sun,
        shade: [60, 70, 110],
        casters: Span { start: 0, len: 0 },
        blocks: Span { start: 0, len: 3000 },
    });
    let k = jane_present::shadow::shear(&sun).expect("a sun that throws");
    let castable = f.blocks.iter().filter(|b| block_poly(b, k, (480, 272)).is_some()).count();
    assert!(castable > POLYS_MOST, "{castable} blocks cast: enough to pass the cap");
    l.build(&f);
    let polys = l.quads.iter().filter(|q| matches!(q.tex, Tex::Poly(_))).count();
    assert_eq!((polys, l.polys.len()), (SUN_POLYS_MOST, SUN_POLYS_MOST));
    assert_eq!(l.polys_skipped as usize, castable - SUN_POLYS_MOST);
    // The lights' marks take what the cap leaves, and no more; the rest are counted.
    let square: Poly = ([(2, 2), (6, 2), (6, 6), (2, 6), (0, 0), (0, 0), (0, 0), (0, 0)], 4);
    for _ in 0..2000 {
        l.mark(square);
    }
    assert_eq!(l.polys.len(), POLYS_MOST);
    assert_eq!(l.lamp_marks.len(), POLYS_MOST - SUN_POLYS_MOST);
    assert_eq!(l.polys_skipped as usize, castable - SUN_POLYS_MOST + 2000 - (POLYS_MOST - SUN_POLYS_MOST));
    // The next frame begins with the room back.
    l.lamp_marks.clear();
    l.build(&f);
    assert_eq!((l.polys.len(), l.polys_skipped as usize), (SUN_POLYS_MOST, castable - SUN_POLYS_MOST));
}

/// Every texel of every page opaque: a caster's silhouette is its whole trimmed rect.
struct Solid;

impl PagePx for Solid {
    fn texel(&mut self, _: u16, _: u16, _: u16) -> u8 {
        2
    }
}

/// A wood's shadow bands, a dozen to a caster, were 5,079 of 5,423 quads at the densest place
/// walked: a wood twice as dense must not take the buffer from the sprites and the light laid
/// after them. The pass keeps its most, the strongest bands first.
#[test]
fn a_wood_of_casters_lays_no_more_shadow_quads_than_the_pass_may() {
    use jane_present::{Caster, Directional, Foot, Span, Tier};
    let mut l = lister();
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    let n = 6000u32;
    for k in 0..n {
        let (x, y) = (20 + (k * 37 % 420) as i16, 40 + (k * 53 % 220) as i16);
        f.sprites.push(SpriteCmd {
            foot: Some(Foot::at(y, false)),
            ..cmd(Src { x: 0, y: 0, w: 10, h: 12 }, x, y - 12, false)
        });
        f.casters.push(Caster { sprite: k, foot: (x + 5, y), height: 12, depth: 4, sink: 0, burn: (0, 0), base: 0 });
    }
    let sun = Directional {
        azimuth: jane_core::Angle(9000),
        elevation: jane_core::Angle(4000),
        colour: [255, 240, 200],
        spread: 100,
        strength: 200,
    };
    f.passes.push(Pass::Silhouettes {
        sun,
        shade: [60, 70, 110],
        casters: Span { start: 0, len: n },
        blocks: Span { start: 0, len: 0 },
    });
    l.build_with(&f, &mut Solid);
    let (a, b) = l
        .marks
        .first()
        .map(|m| (m.0 as usize, l.marks.get(1).map_or(l.quads.len(), |m| m.0 as usize)))
        .expect("a pass");
    assert_eq!(l.marks[0].1, P::SUN_SHADOWS);
    // Its clear is the frame's (nothing before it touched the stencil): counted, not laid.
    assert_eq!(l.stencil_from_clear, Some(a as u32));
    assert_eq!(b - a, SUN_QUADS_MOST - 1, "the pass holds its most");
    let skipped = l.left_out().0;
    assert!(skipped > 0, "{n} casters of a few bands each are more than {SUN_QUADS_MOST}");
    // Strongest first: the bands' tints never get darker down the pass, so what is left out
    // is the faintest.
    let tints: Vec<u32> = l.quads[a..b]
        .iter()
        .filter(|q| q.mode == Mode::ShadowBand && q.tex == Tex::None)
        .map(|q| q.colour & 0xff)
        .collect();
    assert!(tints.windows(2).all(|w| w[0] <= w[1]), "strongest (darkest) bands first");
    // The next frame starts afresh.
    l.build_with(&f, &mut Solid);
    assert_eq!(l.left_out().0, skipped);
}

#[test]
fn the_sun_pre_cull_drops_only_casters_with_no_band_on_the_canvas() {
    use jane_present::{Bend, Caster, Directional, Foot, Span, Tier};
    // Casters on and all round the canvas, short and tall, deep and thin, leaning, under
    // suns from every side, low and high: the pre-cull's bound is checked against each
    // caster's own bands (the assert in `silhouettes`, under test the cull only marks).
    for az in 0..8u16 {
        for el in [1500u16, 4000, 12000] {
            let mut l = lister();
            let mut f = Frame::new(Tier::T0);
            f.canvas = (480, 272);
            let n = 900u32;
            for k in 0..n {
                let (x, y) = (-260 + (k * 37 % 1000) as i16, -200 + (k * 53 % 680) as i16);
                let hgt = 6 + (k * 7 % 60) as u16;
                let bend = Bend { lean: (k % 7) as i8 - 3, from: 2, span: 4 };
                let mut sp = cmd(Src { x: 0, y: 0, w: 4 + (k % 12) as u16, h: hgt }, x, y - hgt as i16, k % 2 == 0);
                sp.flags.bend = bend;
                f.sprites.push(SpriteCmd { foot: Some(Foot::at(y, false)), ..sp });
                f.casters.push(Caster {
                    sprite: k,
                    foot: (x + 2, y),
                    height: hgt.min(255) as u8 + (k % 3) as u8 * 10,
                    depth: 1 + (k % 9) as u8,
                    sink: 0,
                    burn: (0, 0),
                    base: if k % 5 == 0 { 8 } else { 0 },
                });
            }
            let sun = Directional {
                azimuth: jane_core::Angle(az * 8192),
                elevation: jane_core::Angle(el),
                colour: [255, 240, 200],
                spread: 100,
                strength: 200,
            };
            f.passes.push(Pass::Silhouettes {
                sun,
                shade: [60, 70, 110],
                casters: Span { start: 0, len: n },
                blocks: Span { start: 0, len: 0 },
            });
            l.build_with(&f, &mut Solid);
            let k = jane_present::shadow::shear(&sun).expect("a sun that throws");
            let culled = f.casters.iter().filter(|c| sun_off_canvas(&f, c, k, (480, 272))).count();
            assert!(culled > 0, "some casters are wholly off the canvas");
        }
    }
}

/// A frame of T8 chunks (their height layers from `height(slot, px)`) placed at `at`, one
/// caster in view so the sun's pass is laid.
fn relief_frame(at: &[(i32, i32)], height: impl Fn(usize, usize) -> u8) -> Frame {
    use jane_present::{Caster, ChunkCmd, ChunkLayers, Directional, Foot, Span, Tier};
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    for (k, &(x, y)) in at.iter().enumerate() {
        let mut layers = ChunkLayers::new_t8();
        for (i, h) in layers.height.iter_mut().enumerate() {
            *h = height(k, i);
        }
        f.layers.push(layers);
        let id = jane_present::frame::ChunkId { cx: k as u16, cy: 0 };
        f.chunks.push(ChunkCmd { id, generation: 7 + k as u32, x, y, slot: k as u16 });
    }
    f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: at.len() as u32 } });
    f.sprites
        .push(SpriteCmd { foot: Some(Foot::at(150, false)), ..cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 200, 138, false) });
    f.casters.push(Caster { sprite: 0, foot: (205, 150), height: 12, depth: 4, sink: 0, burn: (0, 0), base: 0 });
    let sun = Directional {
        azimuth: jane_core::Angle(9000),
        elevation: jane_core::Angle(4000),
        colour: [255, 240, 200],
        spread: 100,
        strength: 200,
    };
    f.passes.push(Pass::Silhouettes {
        sun,
        shade: [60, 70, 110],
        casters: Span { start: 0, len: 1 },
        blocks: Span { start: 0, len: 0 },
    });
    f
}

/// The stencil the sun's raised marks leave (PLAN-PSP-SPEED.md Q1): each canvas px set where
/// a `RaisedMark` quad's texel (its slot's height, 1:1) is over the ground, as the GE lays
/// them; and as the whole-chunk quads laid them before.
fn raised_px(l: &Lister, f: &Frame) -> (Vec<bool>, Vec<bool>) {
    let (w, h) = (480usize, 272usize);
    let mut now = alloc::vec![false; w * h];
    for q in l.quads.iter().filter(|q| q.mode == Mode::RaisedMark) {
        let Tex::Height(slot) = q.tex else { panic!("a raised mark samples a height layer") };
        let (x0, y0, x1, y1) = (i32::from(q.x0), i32::from(q.y0), i32::from(q.x1), i32::from(q.y1));
        assert!(x0 >= 0 && y0 >= 0 && x1 <= w as i32 && y1 <= h as i32, "on the canvas: {q:?}");
        assert_eq!((i32::from(q.u1 - q.u0), i32::from(q.v1 - q.v0)), (x1 - x0, y1 - y0), "1:1 {q:?}");
        let hl = &f.layers[usize::from(slot)].height;
        for y in y0..y1 {
            for x in x0..x1 {
                let (u, v) = (i32::from(q.u0) + x - x0, i32::from(q.v0) + y - y0);
                if i32::from(hl[(v * CHUNK_PX + u) as usize]) > jane_present::shadow::GROUND {
                    now[y as usize * w + x as usize] = true;
                }
            }
        }
    }
    let mut was = alloc::vec![false; w * h];
    for c in &f.chunks {
        let hl = &f.layers[usize::from(c.slot)].height;
        for v in 0..CHUNK_PX {
            for u in 0..CHUNK_PX {
                let (x, y) = (c.x + u, c.y + v);
                if (0..w as i32).contains(&x)
                    && (0..h as i32).contains(&y)
                    && i32::from(hl[(v * CHUNK_PX + u) as usize]) > jane_present::shadow::GROUND
                {
                    was[y as usize * w + x as usize] = true;
                }
            }
        }
    }
    (now, was)
}

#[test]
fn the_raised_marks_cover_the_relief_alone_and_stencil_what_whole_chunks_did() {
    // A chunk hanging off each edge and corner, one inside: a house's roof in a few cells,
    // lone px at and round the ground's height (4 and 5 either side of it), a flat chunk.
    let at = [(-100, -60), (156, -60), (412, -60), (-100, 196), (156, 196), (412, 196), (60, 10)];
    let mut x = 0x2545_f491u32;
    let mut lone = alloc::vec::Vec::new();
    for _ in 0..16 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        lone.push(x % 65536);
    }
    let height = |k: usize, i: usize| -> u8 {
        let (px, py) = ((i % 256) as u32, (i / 256) as u32);
        if k == 6 {
            0
        } else if (k == 0 || k == 4) && (40..90).contains(&px) && (100..150).contains(&py) {
            40
        } else if lone.contains(&(i as u32 ^ k as u32)) {
            [4u8, 5, 6, 7, 8, 200][i % 6]
        } else {
            (i % 5) as u8
        }
    };
    let f = relief_frame(&at, height);
    let mut l = lister();
    l.build_with(&f, &mut Solid);
    let (now, was) = raised_px(&l, &f);
    assert!(was.iter().any(|&r| r), "some relief is on the canvas");
    assert!(now == was, "the clipped marks stencil every px the whole chunks did, and no other");
    let area: i32 = l
        .quads
        .iter()
        .filter(|q| q.mode == Mode::RaisedMark)
        .map(|q| i32::from(q.x1 - q.x0) * i32::from(q.y1 - q.y0))
        .sum();
    assert!(area < 480 * 272 / 2, "the marks rasterise a fraction of the canvas, not 7 chunks: {area}");
    // Nothing touched the stencil before the pass: the frame's clear stands for its own.
    let first = l.marks.iter().find(|m| m.1 == P::SUN_SHADOWS).map(|m| m.0);
    assert!(first.is_some());
    assert_eq!(l.stencil_from_clear, first);
    assert!(!l.quads.iter().any(|q| q.mode == Mode::StencilClear));
    assert!(l.sub_marks.iter().any(|m| m.1 == P::SUN_RAISED));
    assert!(l.sub_marks.iter().any(|m| m.1 == P::SUN_BANDS));
    // A second frame reads the kept rects (the same generations) to the same stencil.
    l.build_with(&f, &mut Solid);
    assert!(raised_px(&l, &f).0 == was);
}

/// A lake over three `T8` chunks (two hanging off the canvas): water px under a ragged
/// diagonal shore that spills past the water cells (`ChunkLayers::water`, the frame's
/// `water`), the land's cells wet, shining or matt; the sky; people standing on the shore,
/// their feet at the water; rain when `wet`.
fn lake_frame(wet: u8, tick: u32, camera: (i32, i32)) -> Frame {
    use jane_present::frame::{SkyLook, T8_SHINE, T8_WATER, T8_WET, WaterCmd};
    use jane_present::{Atmos, ChunkCmd, ChunkLayers, Depth, Span, Tier};
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    f.tick = tick;
    f.camera = camera;
    f.passes.push(Pass::Sky(SkyLook {
        zenith: [20, 30, 80],
        horizon: [200, 140, 100],
        glow: [255, 140, 60],
        glow_x: 100,
        glow_amount: 40,
        stars: 0,
        star_list: Span { start: 0, len: 0 },
        moon: None,
        zone: (0, 0, 480, 272),
        tick,
    }));
    f.passes.push(Pass::Weather(Atmos { rain: wet, wet, wind: 2, ..Atmos::default() }));
    let at = [(-37, -23), (219, -23), (-37, 233)];
    let mut x = 0x2545_f491u32;
    for (k, &(cx, cy)) in at.iter().enumerate() {
        let mut l = ChunkLayers::new_t8();
        l.clut[0] = 0xff20_6040;
        l.clut[1] = u32::from(T8_WATER) << 24 | 0x0080_4020;
        l.clut[2] = u32::from(T8_WET) << 24 | 0x0030_5050;
        l.clut[3] = u32::from(T8_SHINE) << 24 | 0x0060_6060;
        // The shore: px (u, v) wet where u + v + 2 v / 3 passes a line, give or take a few px.
        let line = [300, 260, 420][k];
        for i in 0..(CHUNK_PX * CHUNK_PX) as usize {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let (u, v) = ((i % 256) as i32, (i / 256) as i32);
            let water = u + v + 2 * v / 3 + (x % 23) as i32 > line + 11;
            let ix = if water { 1 } else { [0, 2, 3][(u / 16 + v / 16) as usize % 3] };
            l.t8_set(i, ix);
        }
        // The water cells: those wholly under the line (the shore's px spill past them).
        for j in 0..16 {
            for i in 0..16 {
                let (u, v) = (i * 16, j * 16);
                if u + v + 2 * v / 3 > line + 11 {
                    l.water.push((i as u8, j as u8, 0));
                    f.water.push(WaterCmd { x: (cx + u) as i16, y: (cy + v) as i16, phase: 0 });
                }
            }
        }
        f.layers.push(l);
        let id = jane_present::frame::ChunkId { cx: k as u16, cy: 0 };
        f.chunks.push(ChunkCmd { id, generation: 3 + k as u32, x: cx, y: cy, slot: k as u16 });
    }
    f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: at.len() as u32 } });
    // Standing at the shore, tall and short: their mirrors in bands, rippled.
    for (k, &(sx, sy)) in [(150, 120), (200, 140), (330, 60), (90, 200), (420, 30), (240, 230)].iter().enumerate() {
        let src = if k % 2 == 0 { Src { x: 0, y: 0, w: 10, h: 12 } } else { Src { x: 10, y: 0, w: 8, h: 8 } };
        f.sprites.push(cmd(src, sx, sy, k % 3 == 1));
    }
    f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 6 } });
    f
}

/// What the water's pass leaves (PLAN-PSP-SPEED.md W1, W3), as the GE lays it: each canvas
/// px's stencil and a hash of what was laid on it in turn (each draw's mode, colour and the
/// texel it read; a strip's segment), from a stencil of noise (whatever the passes before
/// left in the alpha).
fn water_px(l: &Lister, f: &Frame) -> (Vec<u8>, Vec<u64>) {
    let (w, h) = (480i32, 272i32);
    let mut st: Vec<u8> = (0..w * h).map(|i| (i as u32).wrapping_mul(0x9e37_79b1).rotate_left(7) as u8).collect();
    let mut px = alloc::vec![0u64; (w * h) as usize];
    let a = l.marks.iter().position(|m| m.1 == P::WATER_FX).expect("the water's pass");
    let (q0, q1) = (l.marks[a].0 as usize, l.marks.get(a + 1).map_or(l.quads.len(), |m| m.0 as usize));
    let mix = |h: u64, k: u64| (h ^ k).wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(29);
    let passes = |s: u8, at: Where| match at {
        Where::Is(v) => s == v,
        Where::Bits(v, m) => s & m == v & m,
        Where::Over(v) => v < s,
    };
    // Texel `k` of `n` across a quad from `t0` to `t1` (backward when `t0 > t1`), sampled
    // at the px's centre.
    let texel = |t0: u16, t1: u16, k: i32, n: i32| {
        i32::from(t0) + ((2 * k + 1) * (i32::from(t1) - i32::from(t0))).div_euclid(2 * n)
    };
    for q in &l.quads[q0..q1] {
        if let Tex::Strip(i) = q.tex {
            let Mode::Masked(_, at) = q.mode else { panic!("a water strip is masked") };
            let s = l.strips[usize::from(i)];
            let v = &l.verts[s.start as usize..(s.start + u32::from(s.len)) as usize];
            for seg in v.windows(4).step_by(2) {
                let key = seg.iter().fold(7u64, |h, v| {
                    mix(h, (v.x as u16 as u64) << 48 | (v.y as u16 as u64) << 32 | u64::from(v.colour))
                });
                for y in i32::from(seg[0].y).max(0)..i32::from(seg[1].y).min(h) {
                    for x in i32::from(seg[0].x).max(0)..i32::from(seg[2].x).min(w) {
                        let i = (y * w + x) as usize;
                        if passes(st[i], at) {
                            px[i] = mix(px[i], key);
                        }
                    }
                }
            }
            continue;
        }
        let (x0, y0, x1, y1) = (i32::from(q.x0), i32::from(q.y0), i32::from(q.x1), i32::from(q.y1));
        let what = alloc::format!("{:?}", (q.tex, q.mode)).bytes().fold(0u64, |h, b| mix(h, u64::from(b)));
        for y in y0.max(0)..y1.min(h) {
            for x in x0.max(0)..x1.min(w) {
                let i = (y * w + x) as usize;
                let (u, v) = (texel(q.u0, q.u1, x - x0, x1 - x0), texel(q.v0, q.v1, y - y0, y1 - y0));
                let alpha = match q.tex {
                    Tex::Chunk(slot) => (f.layers[usize::from(slot)].t8_abgr((v * CHUNK_PX + u) as usize) >> 24) as u8,
                    Tex::Noise => (u ^ v.rotate_left(3)) as u8 & 0x80,
                    _ => 0xff,
                };
                match q.mode {
                    Mode::StencilClear => st[i] = 0,
                    Mode::Mark { tag, value, keep, need } => {
                        let texel_in = if tag == 0 { alpha != 0 } else { alpha == tag };
                        if texel_in && need.is_none_or(|n| st[i] & n != value & n) {
                            st[i] = st[i] & keep | value & !keep;
                        }
                    }
                    Mode::Masked(_, at) => {
                        if passes(st[i], at) {
                            let key = (u as u16 as u64) << 48 | (v as u16 as u64) << 32 | u64::from(q.colour);
                            px[i] = mix(mix(px[i], key), what);
                        }
                    }
                    m => panic!("the water's pass lays no {m:?}"),
                }
            }
        }
    }
    (st, px)
}

#[test]
fn the_tight_water_lays_every_px_the_box_did_and_fewer_quads() {
    use jane_present::frame::T8_WATER;
    let (mut quads, mut area) = ([0usize; 2], [0i64; 2]);
    for (wet, tick, cam) in [(0, 0, (0, 0)), (0, 377, (5, 11)), (180, 91, (-9, 3)), (255, 1234, (13, -6))] {
        let f = lake_frame(wet, tick, cam);
        let mut lay = |loose: bool| {
            let mut l = lister();
            l.water_loose = loose;
            l.build(&f);
            let a = l.marks.iter().position(|m| m.1 == P::WATER_FX).expect("the water's pass");
            let (q0, q1) = (l.marks[a].0 as usize, l.marks.get(a + 1).map_or(l.quads.len(), |m| m.0 as usize));
            let k = usize::from(!loose);
            quads[k] += q1 - q0;
            // The fill of what reads the water's mark: its mark, its sky and its crests.
            area[k] += l.quads[q0..q1]
                .iter()
                .filter(|q| matches!(q.mode, Mode::Mark { tag: T8_WATER, .. } | Mode::Masked(_, Where::Is(3))))
                .map(|q| match q.tex {
                    // A strip's own rect, not its quad's (the canvas).
                    Tex::Strip(i) => {
                        let s = l.strips[usize::from(i)];
                        let v = &l.verts[s.start as usize..(s.start + u32::from(s.len)) as usize];
                        i64::from(v[v.len() - 1].x - v[0].x) * i64::from(v[1].y - v[0].y)
                    }
                    _ => i64::from(q.x1 - q.x0) * i64::from(q.y1 - q.y0),
                })
                .sum::<i64>();
            (water_px(&l, &f), l)
        };
        let ((was_st, was_px), _) = lay(true);
        let ((st, px), l) = lay(false);
        assert!(was_px.iter().any(|&h| h != 0), "the water lays something");
        assert!(st == was_st, "wet {wet} tick {tick}: the stencil as the box left it");
        let diff = px.iter().zip(&was_px).filter(|(a, b)| a != b).count();
        assert_eq!(diff, 0, "wet {wet} tick {tick}: every px laid as the box laid it");
        // A second frame reads the kept rects to the same.
        let mut l = l;
        l.build(&f);
        assert!(water_px(&l, &f).1 == was_px);
    }
    assert!(quads[1] < quads[0], "fewer quads: {quads:?}");
    // (A lake over most of the canvas: its box is the canvas, its cells about four fifths.)
    assert!(area[1] * 5 < area[0] * 4, "the water's mark, sky and crests fill less: {area:?}");
}

#[test]
fn a_mirror_joins_bands_shifted_alike() {
    let f = lake_frame(0, 377, (5, 11));
    let count = |loose: bool| {
        let mut l = lister();
        l.water_loose = loose;
        l.build(&f);
        l.quads
            .iter()
            .filter(|q| matches!(q.tex, Tex::Page(_)) && q.mode == Mode::Masked(Blend::Alpha, Where::Over(2)))
            .count()
    };
    let (was, now) = (count(true), count(false));
    assert!(now < was && now > 0, "{now} quads for {was} bands");
}

#[test]
fn relief_past_the_rects_most_is_one_span_a_row() {
    // A chequer of raised cells: 128 runs, laid as 16 spans.
    let mut hl = alloc::vec![0u8; (CHUNK_PX * CHUNK_PX) as usize];
    for (i, h) in hl.iter_mut().enumerate() {
        let (cx, cy) = (i % 256 / 16, i / 256 / 16);
        if (cx + cy) % 2 == 0 && i % 16 == 3 {
            *h = 9;
        }
    }
    let mut out = alloc::vec::Vec::new();
    relief_cells(&hl, &mut out);
    assert_eq!(out.len(), 16);
    for (k, r) in out.iter().enumerate() {
        let x0 = (k % 2) as u8;
        assert_eq!(*r, (x0, k as u8, x0 + 15, k as u8 + 1));
    }
    // Flat: nothing. Raised everywhere: one rect.
    relief_cells(&alloc::vec![4u8; hl.len()], &mut out);
    assert!(out.is_empty());
    relief_cells(&alloc::vec![5u8; hl.len()], &mut out);
    assert_eq!(out, [(0, 0, 16, 16)]);
}

#[test]
fn a_ref_with_nothing_on_the_psp_is_a_miss() {
    let mut l = lister();
    l.sprite(&cmd(Src { x: 0, y: 12, w: 6, h: 6 }, 0, 0, false), &[], 0);
    assert!(l.quads.is_empty());
    assert_eq!(l.misses, 1);
}

#[test]
fn a_bent_sprite_is_strips_shifted_by_the_bend() {
    let mut l = lister();
    let mut c = cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, false);
    c.flags.bend = jane_present::Bend { lean: 2, from: 8, span: 4 };
    l.sprite(&c, &[], 0);
    // Rows 3..12 drawn; every row's shift is the bend's, and they tile the rows once.
    let mut rows = 0;
    for q in &l.quads {
        for y in q.y0..q.y1 {
            let r = i32::from(y) - 30;
            assert_eq!(i32::from(q.x0), 22 + c.flags.bend.shift(r), "row {r}");
            rows += 1;
        }
    }
    assert_eq!(rows, 9);
    assert!(l.quads.len() > 1);
}

/// The night's turn on C2 (NIGHT.md §4.7): the band as a few strips over the lightmap or the
/// flat light, every row covered once, the top lit and the bottom held dark mid-band; no
/// band, one strip at all of the light.
#[test]
fn the_turns_band_is_a_few_strips_down_the_screen() {
    let strips = |b: &jane_present::Band, rows: i32, unit: i32| {
        let mut v = Vec::new();
        band_strips(*b, rows, unit, |y0, y1, k| v.push((y0, y1, k)));
        v
    };
    assert_eq!(strips(&jane_present::Band::NONE, 136, 2), vec![(0, 136, 256)]);
    let b = jane_present::Band { edge: 140, soft: 48, dark: 51, down: true, crest: 0 };
    for (rows, unit) in [(138, 2), (272, 1)] {
        let v = strips(&b, rows, unit);
        assert_eq!((v[0].0, v[v.len() - 1].1), (0, rows));
        assert!(v.windows(2).all(|w| w[0].1 == w[1].0 && w[1].2 <= w[0].2), "{v:?}");
        assert!(v[0].2 == 256 && v[v.len() - 1].2 == 48, "{v:?}");
        assert!(v.len() <= 10, "a few strips: {}", v.len());
    }
    assert_eq!(sub_rgb(0xff40_8020, 0xff10_9010), 0xff30_0010);
}

/// A street of sprites over two pages (Q6a): grouped by page, every pair of quads whose px
/// touch is drawn in the order it was, the quads are the same, and the batches fall.
#[test]
fn sprites_grouped_by_page_keep_every_pair_that_touches() {
    use jane_present::{Depth, Span};
    let mut f = Frame::new(jane_present::Tier::T0);
    f.canvas = (480, 272);
    for k in 0..80i16 {
        // Refs 0 (page 4) and 1 (page 5) by turns, down the screen by depth, some over others.
        let src = if k % 3 == 0 { Src { x: 10, y: 0, w: 8, h: 8 } } else { Src { x: 0, y: 0, w: 10, h: 12 } };
        f.sprites.push(cmd(src, (k * 37) % 460, k * 3, k % 5 == 0));
    }
    f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 80 } });
    let mut l = lister();
    l.order_passes = 0;
    let a = l.build(&f).to_vec();
    l.order_passes = ORDERED;
    let b = l.build(&f).to_vec();
    let mut sa = a.clone();
    let mut sb = b.clone();
    let key = |q: &Quad| (q.x0, q.y0, q.x1, q.y1, q.u0, q.v0);
    sa.sort_by_key(key);
    sb.sort_by_key(key);
    assert_eq!(sa, sb, "the same quads");
    let pos = |q: &Quad| b.iter().position(|p| p == q).expect("kept");
    for i in 0..a.len() {
        for j in i + 1..a.len() {
            let touch = match (order::reach(&a[i]), order::reach(&a[j])) {
                (Some(x), Some(y)) => x.0 < y.2 && y.0 < x.2 && x.1 < y.3 && y.1 < x.3,
                _ => true,
            };
            if touch {
                assert!(pos(&a[i]) < pos(&a[j]), "quads {i} and {j} swapped");
            }
        }
    }
    let batches = |q: &[Quad]| 1 + q.windows(2).filter(|w| !guard::same_batch(&w[0], &w[1])).count();
    assert!(batches(&b) * 2 < batches(&a), "{} -> {}", batches(&a), batches(&b));
}

/// A block's swept hull laid from its edges is the sorted hull, corner for corner, along every
/// sun, near the canvas and far past it (clamped alike).
#[test]
fn a_swept_blocks_hull_from_its_edges_is_the_sorted_hull() {
    let mut s = 0x9e37_79b9u32;
    let mut rnd = |n: u32| {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s % n
    };
    let mut checked = 0;
    for k in 0..200_000 {
        let x0 = rnd(1200) as i32 - 600;
        let y0 = rnd(1200) as i32 - 600;
        let (bw, bh) = (1 + rnd(if k % 7 == 0 { 3 } else { 90 }) as i32, 1 + rnd(60) as i32);
        let b = jane_present::Block {
            x0: x0 as i16,
            y0: y0 as i16,
            x1: (x0 + bw) as i16,
            y1: (y0 + bh) as i16,
            height: rnd(256) as u8,
            lo: rnd(256) as u8,
            fence: rnd(4) == 0,
        };
        // Suns along the axes and between them, short and long; a camera near and far.
        let pick = |r: u32, v: u32| match r % 4 {
            0 => 0,
            1 => v as i32 - 300,
            2 => -(v as i32 % 40),
            _ => v as i32 % 40,
        };
        let (r0, v0, r1, v1) = (rnd(4), rnd(600), rnd(4), rnd(600));
        let kk = (pick(r0, v0), pick(r1, v1));
        let o = if k % 3 == 0 { (rnd(30_000) as i32 - 15_000, rnd(30_000) as i32 - 15_000) } else { (0, 0) };
        let Some(pts) = block_sweep(&b, kk, o) else { continue };
        assert_eq!(sweep_hull(&pts), hull(pts), "{b:?} k {kk:?} o {o:?}");
        checked += 1;
    }
    assert!(checked > 100_000);
}

/// L4: a patch scans only the chunk's cells with relief, and lays the px the whole scan laid.
#[test]
fn a_patch_from_the_relief_cells_alone_is_the_whole_scans_patch() {
    use jane_present::{ChunkLayers, Foot, Tier};
    let mut f = Frame::new(Tier::T0);
    f.canvas = (480, 272);
    let mut layer = ChunkLayers::new(Tier::T0);
    let mut s = 0x2545_f491u32;
    for k in 0..(CHUNK_PX * CHUNK_PX) as usize {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        let (x, y) = (k % CHUNK_PX as usize, k / CHUNK_PX as usize);
        // A wall over a few cells, a kerb (under the relief) over others, flat the rest.
        layer.height[k] = if (x / 16 + y / 16) % 5 == 0 {
            (s % 60) as u8
        } else if (x / 16) % 3 == 0 {
            (s % 5) as u8
        } else {
            0
        };
        layer.albedo[k] = 0xff00_0000 | s >> 8;
    }
    f.layers.push(layer);
    let feet = [Foot::at(40, false), Foot::at(120, false), Foot::at(250, false)];
    let mut l = lister();
    l.build(&f);
    l.placed.push((-10, 7, 0));
    for k in 0..60i16 {
        let s = cmd(Src { x: 0, y: 0, w: 10, h: 12 }, (k * 41) % 470 - 5, (k * 29) % 260, false);
        l.patch(&f, &SpriteCmd { src: Src { x: 0, y: 0, w: 24, h: 40 }, ..s }, feet[k as usize % 3]);
    }
    // The whole scan, as it was: every px of each patch's rect in the chunk.
    let mut patches = 0;
    for q in &l.quads {
        let Tex::Patch(i) = q.tex else { continue };
        let p = l.patches[usize::from(i)];
        let k = (0..60i16).find(|&k| {
            let (x, y) = ((k * 41) % 470 - 5, (k * 29) % 260);
            (i32::from(x).max(0), i32::from(y).max(0)) == (i32::from(q.x0), i32::from(q.y0))
        });
        let foot = feet[k.expect("its sprite") as usize % 3];
        for y in 0..i32::from(p.h) {
            for x in 0..i32::from(p.w) {
                let (cx, cy) = (i32::from(q.x0) + x + 10, i32::from(q.y0) + y - 7);
                let mut want = 0;
                if (0..CHUNK_PX).contains(&cx) && (0..CHUNK_PX).contains(&cy) {
                    let k = (cy * CHUNK_PX + cx) as usize;
                    if Foot::hides(f.layers[0].height[k], i32::from(q.y0) + y, i32::from(foot.y)) {
                        want = abgr(f.layers[0].albedo[k]) | 0xff00_0000;
                    }
                }
                let got = l.patch_px[p.at as usize + y as usize * usize::from(p.stride) + x as usize];
                assert_eq!(got, want, "patch {i} px ({x}, {y})");
            }
        }
        patches += 1;
    }
    assert!(patches > 10, "{patches} patches");
}
