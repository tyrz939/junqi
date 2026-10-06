//! The Frame's casters are the single source (PRESENTATION.md §1.7, decided 2026-09-27): what
//! throws a shadow is decided once, by the presenter, and every tier draws a shadow for every
//! caster the Frame lists. A lone house (the terrain's blocks), a lone post and a lone person at
//! 17:00, drawn on T0, T1 and T2: each tier darkens the ground on each one's shadow side, over an
//! area within a tolerance of T2's. One test: SDL lives once in a process. The GPU tiers are
//! skipped where the machine has no context for them.
#![cfg(feature = "gpu")]

use jane_present::frame::CHUNK_PX;
use jane_present::shadow::shade_at;
use jane_present::terrain::blocks;
use jane_present::{
    AtlasPages, Backend, Block, CLUT_LEN, Caster, ChunkCmd, ChunkId, ChunkLayers, Depth, Frame, Page, Pass, Span,
    SpriteCmd, Src, Tier, height_of_rows,
};
use jane_render_gl2::{Api, Gl2, Rows};
use jane_render_soft::Soft;
use jane_render_wgpu::Wgpu;

const W: u16 = 768;
const H: u16 = 256;
const GRASS: u32 = 0xff6c_8a4e;

/// The house: a roof 40 rows deep at 60 px over a face 48 rows tall (a storey), columns 16..96
/// of the first chunk; its face's foot on row 148.
const HOUSE: (i32, i32) = (16, 96);
const FOOT: i32 = 148;

/// The sprites' page: a post 2 px wide and 24 rows tall in columns 0..2, a person 8 wide and 40
/// rows tall in columns 4..12, each over a contact-shadow row (63), each px its true height over
/// the row it stands on.
fn atlas() -> AtlasPages {
    let (w, h) = (16usize, 64usize);
    let n = w * h;
    let mut page = Page {
        w: w as u16,
        h: h as u16,
        albedo: vec![0; n].into(),
        normal: vec![[128, 200]; n],
        emissive: vec![0; n],
        height: vec![0; n],
        glow: Vec::new(),
    };
    let mut put = |x0: usize, x1: usize, top: usize| {
        for y in top..63 {
            for x in x0..x1 {
                std::sync::Arc::make_mut(&mut page.albedo)[y * w + x] = 2;
                page.height[y * w + x] = height_of_rows(63 - y as i32).max(1) as u8;
            }
        }
        for x in x0..x1 {
            std::sync::Arc::make_mut(&mut page.albedo)[63 * w + x] = 1;
        }
    };
    put(0, 2, 39);
    put(4, 12, 23);
    let mut clut = vec![0xff00_0000; CLUT_LEN];
    clut[2] = 0xff90_6850;
    AtlasPages { clut, pages: vec![page], mist: Vec::new() }
}

/// Three chunks of grass, the house in the first.
fn chunks() -> Vec<ChunkLayers> {
    (0..3)
        .map(|k| {
            let mut l = ChunkLayers::new(Tier::T2);
            for y in 0..CHUNK_PX {
                for x in 0..CHUNK_PX {
                    let i = (y * CHUNK_PX + x) as usize;
                    l.albedo[i] = GRASS;
                    l.height[i] = 1;
                    if k == 0 && (HOUSE.0..HOUSE.1).contains(&x) {
                        if (FOOT - 88..FOOT - 48).contains(&y) {
                            l.albedo[i] = 0xff9a_4a3a;
                            l.height[i] = 60;
                        } else if (FOOT - 48..FOOT).contains(&y) {
                            l.albedo[i] = 0xffd8_c8a8;
                            l.height[i] = height_of_rows(FOOT - y).max(1) as u8;
                            l.normal[i] = [128, 220];
                        }
                    }
                }
            }
            l
        })
        .collect()
}

/// The frame at `tier`: the three chunks, the post and the person standing east of the house, and
/// the sun at 17:00, its shadows on (`shadows`) or not (the same sun with no strength).
fn frame(tier: Tier, shadows: bool) -> Frame {
    let mut f = Frame::new(tier);
    f.canvas = (W, H);
    f.clear = GRASS;
    f.layers = chunks();
    let (mut field, mut runs, mut out) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..3u16 {
        f.chunks.push(ChunkCmd {
            id: ChunkId { cx: k, cy: 0 },
            generation: 1,
            x: i32::from(k) * CHUNK_PX,
            y: 0,
            slot: k,
        });
        blocks(&f.layers[usize::from(k)].height, &[], &mut field, &mut runs, &mut out);
        let dx = k as i16 * CHUNK_PX as i16;
        f.blocks.extend(out.iter().map(|b| Block { x0: b.x0 + dx, x1: b.x1 + dx, ..*b }));
    }
    f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: 3 } });
    // The post at x 340 and the person at x 520, both on row 180.
    for (i, (x, src, top, depth)) in
        [(340i16, Src { x: 0, y: 0, w: 2, h: 64 }, 30u8, 2u8), (520, Src { x: 4, y: 0, w: 8, h: 64 }, 50, 5)]
            .into_iter()
            .enumerate()
    {
        let foot = (x, 180i16);
        f.sprites.push(SpriteCmd {
            page: 0,
            src,
            x: x - src.w as i16 / 2,
            y: foot.1 - 63,
            flags: jane_present::Flags::default(),
            height_px: top,
            foot: None,
        });
        f.casters.push(Caster { sprite: i as u32, foot, height: top, depth, ..Caster::default() });
    }
    let sky =
        jane_present::light::sky(17 * jane_core::num::TICKS_PER_HOUR, 0, false, 1000, jane_data::Region::Lowfields);
    let mut sun = sky.sun.expect("the sun is up at five");
    if !shadows {
        sun.strength = 0;
    }
    let (casters, blocks) = (Span { start: 0, len: 2 }, Span { start: 0, len: f.blocks.len() as u32 });
    if tier < Tier::T2 && sun.casts() {
        f.passes.push(Pass::Silhouettes { sun, shade: shade_at(sky.shade, sun.strength), casters, blocks });
    }
    f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 2 } });
    f.passes.push(Pass::Lights {
        ambient: sky.ambient,
        fill: sky.fill,
        sun: Some(sun),
        points: Span::default(),
        casters,
        blocks,
    });
    f
}

fn luma(c: u32) -> i32 {
    let (r, g, b) = ((c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff);
    (r * 3 + g * 6 + b) as i32
}

/// The px of `rect` `(x0, y0, x1, y1)` the shadows darken by a tenth or more.
fn dark(with: &[u32], without: &[u32], (x0, y0, x1, y1): (i32, i32, i32, i32)) -> usize {
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = y as usize * usize::from(W) + x as usize;
            n += usize::from(luma(with[i]) * 10 < luma(without[i]) * 9);
        }
    }
    n
}

fn draw(b: &mut dyn Backend, atlas: &AtlasPages, tier: Tier) -> (Vec<u32>, Vec<u32>) {
    b.upload_atlas(atlas);
    let mut out = [Vec::new(), Vec::new()];
    for (k, shadows) in [true, false].into_iter().enumerate() {
        b.draw(&frame(tier, shadows));
        let (w, h) = b.read_back(&mut out[k]);
        assert_eq!((w, h), (W, H));
    }
    let [a, c] = out;
    (a, c)
}

#[test]
fn every_tier_draws_a_shadow_for_every_caster_the_frame_lists() {
    let atlas = atlas();
    // East of each: the house's shadow on the grass past its east wall, the post's and the
    // person's past their bodies (at five the shadows run east, a little north).
    let regions =
        [("house", (HOUSE.1 + 2, 0, 330, 200)), ("post", (343, 100, 500, 200)), ("person", (526, 100, 760, 200))];
    let mut tiers: Vec<(Tier, Vec<usize>)> = Vec::new();
    let mut soft = Soft::new();
    let (w, wo) = draw(&mut soft, &atlas, Tier::T0);
    tiers.push((Tier::T0, regions.iter().map(|r| dark(&w, &wo, r.1)).collect()));
    match Gl2::headless(Api::Desktop) {
        Ok(mut gl) => {
            gl.set_rows(Rows::T1);
            let (w, wo) = draw(&mut gl, &atlas, Tier::T1);
            tiers.push((Tier::T1, regions.iter().map(|r| dark(&w, &wo, r.1)).collect()));
        }
        Err(e) => eprintln!("no GL context: T1 skipped ({e})"),
    }
    match Wgpu::headless() {
        Ok(mut wg) => {
            let (w, wo) = draw(&mut wg, &atlas, Tier::T2);
            tiers.push((Tier::T2, regions.iter().map(|r| dark(&w, &wo, r.1)).collect()));
        }
        Err(e) => eprintln!("no wgpu adapter: T2 skipped ({e})"),
    }
    for (t, n) in &tiers {
        eprintln!("{t:?}: {:?}", regions.iter().map(|r| r.0).zip(n).collect::<Vec<_>>());
        for (r, &k) in regions.iter().zip(n) {
            assert!(k > 40, "{t:?} draws no shadow for the {}: {k} px", r.0);
        }
    }
    // Each tier's shadow is about T2's: its area within a half and twice of it.
    if let Some((_, t2)) = tiers.iter().find(|t| t.0 == Tier::T2) {
        for (t, n) in &tiers {
            for ((r, &k), &k2) in regions.iter().zip(n).zip(t2) {
                assert!(k * 2 >= k2 && k <= k2 * 2, "{t:?}'s shadow of the {} is {k} px, T2's {k2}", r.0);
            }
        }
    }
}
