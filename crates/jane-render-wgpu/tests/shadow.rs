//! T2's shadow rules (PRESENTATION.md §1.7), drawn on the GPU from a made-up frame: a lone
//! upright sprite in open sun, a lantern beside the one holding it, a wall's block in a low
//! sun. Skipped where the machine has no wgpu adapter.

use jane_core::Angle;
use jane_present::frame::CHUNK_PX;
use jane_present::{
    AtlasPages, Backend, CLUT_LEN, Caster, ChunkCmd, ChunkId, ChunkLayers, Depth, Directional, Flags, Frame, Light,
    LightKind, Page, Pass, Post, Span, SpriteCmd, Src, Tier, height_of_rows,
};
use jane_render_wgpu::Wgpu;

const W: u16 = 256;
const H: u16 = 160;
/// The sprite: 16 x 40, a body 8 wide from row 4 to its foot on row 36.
const SW: u16 = 16;
const SH: u16 = 40;
const AY: i32 = 36;
/// Where it stands on the canvas.
const FOOT: (i32, i32) = (124, 96);
const GRASS: u32 = 0xff70_8050;

fn backend() -> Option<Wgpu> {
    match Wgpu::headless() {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("no wgpu adapter: skipped ({e})");
            None
        }
    }
}

/// The atlas: one page holding the sprite, its heights by `height(row)`.
fn atlas(height: impl Fn(i32) -> u8) -> AtlasPages {
    let n = usize::from(SW) * usize::from(SH);
    let mut page = Page {
        w: SW,
        h: SH,
        albedo: vec![0; n],
        normal: vec![[128, 200]; n],
        emissive: vec![0; n],
        height: vec![0; n],
    };
    for y in 4..=AY {
        for x in 4..12 {
            let i = y as usize * usize::from(SW) + x;
            page.albedo[i] = 2;
            page.height[i] = height(y);
        }
    }
    let mut clut = vec![0xff00_0000; CLUT_LEN];
    clut[2] = 0xffc0_8060;
    AtlasPages { clut, pages: vec![page], mist: Vec::new() }
}

/// Flat grass, with `raise(x, y)` over it where it returns a height.
fn ground(raise: impl Fn(i32, i32) -> Option<u8>) -> ChunkLayers {
    let mut l = ChunkLayers::new(Tier::T2);
    for y in 0..CHUNK_PX {
        for x in 0..CHUNK_PX {
            let i = (y * CHUNK_PX + x) as usize;
            l.albedo[i] = GRASS;
            l.height[i] = 1;
            if let Some(h) = raise(x, y) {
                l.height[i] = h;
                // A face looks south; what lies on top of it, up.
                l.normal[i] = if h < 20 || y >= 80 { [128, 220] } else { [128, 128] };
            }
        }
    }
    l
}

/// The sun at five: low in the west, a little south, so shadows run east and a little north.
fn five() -> Directional {
    Directional {
        azimuth: Angle(32768 - 2730),
        elevation: Angle::from_degrees(16),
        colour: [255, 210, 160],
        spread: 400,
    }
}

/// A frame of `layers`, with the sprite standing on `FOOT` if `sprite`, lit by `sun` and `points`.
fn frame(layers: ChunkLayers, sprite: bool, sun: Option<Directional>, points: &[Light]) -> Frame {
    let mut f = Frame::new(Tier::T2);
    f.canvas = (W, H);
    f.clear = GRASS;
    f.layers = vec![layers];
    f.chunks.push(ChunkCmd { id: ChunkId { cx: 0, cy: 0 }, generation: 1, x: 0, y: 0, slot: 0 });
    f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: 1 } });
    if sprite {
        f.sprites.push(SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: SW, h: SH },
            x: (FOOT.0 - 8) as i16,
            y: (FOOT.1 - AY) as i16,
            flags: Flags::default(),
            height_px: 45,
        });
        f.casters.push(Caster { sprite: 0, foot: (FOOT.0 as i16, FOOT.1 as i16), height: 45, depth: 5 });
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 1 } });
    }
    f.lights.extend_from_slice(points);
    f.passes.push(Pass::Lights {
        ambient: [200; 3],
        fill: [70, 78, 120],
        sun,
        points: Span { start: 0, len: points.len() as u32 },
        casters: Span { start: 0, len: f.casters.len() as u32 },
    });
    f.passes.push(Pass::Post(Post::NONE));
    f
}

fn draw(b: &mut Wgpu, atlas: &AtlasPages, f: &Frame) -> Vec<u32> {
    b.upload_atlas(atlas);
    b.draw(f);
    let mut px = Vec::new();
    let (w, h) = b.read_back(&mut px);
    assert_eq!((w, h), (W, H));
    px
}

fn luma(c: u32) -> i32 {
    let (r, g, b) = ((c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff);
    (r * 3 + g * 6 + b) as i32 / 10
}

fn at(px: &[u32], x: i32, y: i32) -> u32 {
    px[y as usize * usize::from(W) + x as usize]
}

/// The sprite's own px on the canvas.
fn body() -> impl Iterator<Item = (i32, i32)> {
    let (x0, y0) = (FOOT.0 - 8, FOOT.1 - AY);
    (4..=AY).flat_map(move |y| (4..12).map(move |x| (x0 + x, y0 + y)))
}

#[test]
fn a_sprite_never_darkens_itself_in_open_sun_and_its_shadow_grows_from_its_feet() {
    let Some(mut b) = backend() else { return };
    let f = frame(ground(|_, _| None), true, Some(five()), &[]);
    // Standing: every px its true height over the foot row.
    let standing = draw(&mut b, &atlas(|y| height_of_rows(AY - y).max(1) as u8), &f);
    // The same body lying down, 5 px high all over: nothing of it rises over its own px.
    let lying = draw(&mut b, &atlas(|_| 5), &f);
    for (x, y) in body() {
        let (s, l) = (at(&standing, x, y), at(&lying, x, y));
        let most = (0..3).map(|k| ((s >> (k * 8)) & 0xff).abs_diff((l >> (k * 8)) & 0xff)).max().unwrap_or(0);
        assert!(most <= 2, "({x}, {y}): standing {s:08x}, lying {l:08x}: the sprite shadows itself");
    }
    // The shadow is thrown east from the feet: the ground two px past the body's right edge on
    // the foot row is in it, and the grass well west of her is not.
    let lit = luma(at(&standing, 40, FOOT.1));
    let root = luma(at(&standing, FOOT.0 + 6, FOOT.1));
    assert!(root * 10 < lit * 8, "the ground at her feet is not in her shadow: {root} of {lit}");
    // And it is her silhouette's: far out along the shadow, still dark.
    let far = luma(at(&standing, FOOT.0 + 60, FOOT.1 - 16));
    assert!(far * 10 < lit * 9, "no long shadow at five: {far} of {lit}");
}

#[test]
fn a_light_never_shadows_the_one_who_holds_it() {
    let Some(mut b) = backend() else { return };
    let atlas = atlas(|y| height_of_rows(AY - y).max(1) as u8);
    // Her lantern, at her west side, her hip high.
    let lantern = |holder| Light {
        pos: (FOOT.0 - 9, FOOT.1 + 3),
        height: 20,
        colour: [255, 190, 116],
        radius: 90,
        size: 5,
        casts: true,
        kind: LightKind::Point,
        holder,
    };
    let held = draw(&mut b, &atlas, &frame(ground(|_, _| None), true, None, &[lantern(Some(0))]));
    let nobody = draw(&mut b, &atlas, &frame(ground(|_, _| None), false, None, &[lantern(None)]));
    let dropped = draw(&mut b, &atlas, &frame(ground(|_, _| None), true, None, &[lantern(None)]));
    // Behind her from the lantern (east of her body, on and above her foot row), the ground is
    // lit as if she were not there; with no holder named, her body throws its shadow there.
    let mut darker = 0;
    for (x, y) in [(FOOT.0 + 12, FOOT.1), (FOOT.0 + 20, FOOT.1 - 2), (FOOT.0 + 30, FOOT.1)] {
        let (h, n, d) = (luma(at(&held, x, y)), luma(at(&nobody, x, y)), luma(at(&dropped, x, y)));
        assert!((h - n).abs() <= 2, "({x}, {y}): held {h}, nobody there {n}: her lantern shadows her");
        darker += i32::from(d + 4 < n);
    }
    assert!(darker >= 2, "the check proves nothing: an unheld lantern casts no shadow of her either");
}

#[test]
fn a_walls_shadow_is_straight_edged_and_its_face_takes_no_stairs() {
    let Some(mut b) = backend() else { return };
    // A block 96 px wide: its top from row 40 to 79 at a one-cell face's height, its face from
    // row 80 to 95 rising from its foot on row 95.
    let block = ground(|x, y| {
        if !(80..176).contains(&x) {
            return None;
        }
        match y {
            40..=79 => Some(height_of_rows(16) as u8),
            80..=95 => Some(height_of_rows(96 - y).max(1) as u8),
            _ => None,
        }
    });
    let px = draw(&mut b, &atlas(|_| 1), &frame(block, false, Some(five()), &[]));
    let lit = luma(at(&px, 30, 120));
    // The face, lit by the low sun: each row one tone from end to end (a few px in from its
    // corners), no stepped shadow across it.
    for y in 81..95 {
        let row: Vec<i32> = (84..172).map(|x| luma(at(&px, x, y))).collect();
        let (lo, hi) = (row.iter().min().copied().unwrap_or(0), row.iter().max().copied().unwrap_or(0));
        assert!(hi - lo <= 3, "face row {y}: {lo}..{hi}, a shadow steps across it");
    }
    // East of it the block's shadow on the grass: its lower edge rises steadily to the east, a
    // row every few px, never a stair of more than two rows between neighbouring columns.
    let edge = |x: i32| (60..H as i32).rev().find(|&y| luma(at(&px, x, y)) * 10 < lit * 8);
    let edges: Vec<i32> = (180..250).filter_map(edge).collect();
    assert!(edges.len() > 60, "the block throws no shadow east: {edges:?}");
    for w in edges.windows(2) {
        assert!(w[1] <= w[0] && w[0] - w[1] <= 2, "the shadow's edge steps: {edges:?}");
    }
    assert!(edges[0] - edges[edges.len() - 1] >= 8, "the shadow's edge does not rise east: {edges:?}");
}
