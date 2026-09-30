//! T2's shadow rules (PRESENTATION.md §1.7), drawn on the GPU from a made-up frame: a lone
//! upright sprite in open sun, a lantern beside the one holding it, a wall's block in a low
//! sun. Skipped where the machine has no wgpu adapter.

use jane_core::Angle;
use jane_present::frame::CHUNK_PX;
use jane_present::shadow::{self, shear};
use jane_present::{
    AtlasPages, Backend, Block, CLUT_LEN, Caster, ChunkCmd, ChunkId, ChunkLayers, Depth, Directional, Flags, Foot,
    Frame, Light, LightKind, Page, Pass, Post, Span, SpriteCmd, Src, Tier, height_of_rows,
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
        glow: Vec::new(),
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
        strength: 255,
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
            foot: None,
        });
        f.casters.push(Caster {
            sprite: 0,
            foot: (FOOT.0 as i16, FOOT.1 as i16),
            height: 45,
            depth: 5,
            ..Caster::default()
        });
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 1 } });
    }
    f.lights.extend_from_slice(points);
    f.passes.push(Pass::Lights {
        // A night's flat light with its night fill: a lamp's pool shows whole (`light::pool`).
        ambient: [96, 106, 140],
        fill: [70, 78, 120],
        sun,
        points: Span { start: 0, len: points.len() as u32 },
        casters: Span { start: 0, len: f.casters.len() as u32 },
        blocks: Span::default(),
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

/// Behind the terrain (PRESENTATION.md §1.6): a roof 40 px up over the rows her body is drawn
/// on, standing on ground south of her feet, hides her there; seen through it, one px in two of
/// her shows, colour alone (the roof's field and light are as they were); below the roof's
/// front she is drawn whole.
#[test]
fn what_stands_behind_the_terrain_is_hidden_where_it_stands_in_front() {
    let Some(mut b) = backend() else { return };
    let atlas = atlas(|y| height_of_rows(AY - y).max(1) as u8);
    let roof = |_: i32, y: i32| (70..90).contains(&y).then_some(40);
    let with = |foot: Option<Foot>| {
        let mut f = frame(ground(roof), true, Some(five()), &[]);
        f.sprites[0].foot = foot;
        f.casters.clear();
        if let Some(Pass::Lights { casters, .. }) = f.passes.iter_mut().find(|q| matches!(q, Pass::Lights { .. })) {
            *casters = Span::default();
        }
        f
    };
    let mut bare = frame(ground(roof), false, Some(five()), &[]);
    bare.passes.retain(|q| !matches!(q, Pass::Sprites { .. }));
    let bare = draw(&mut b, &atlas, &bare);
    let whole = draw(&mut b, &atlas, &with(None));
    let hid = draw(&mut b, &atlas, &with(Some(Foot { y: FOOT.1 as i16, see: false })));
    let seen = draw(&mut b, &atlas, &with(Some(Foot { y: FOOT.1 as i16, see: true })));
    let (mut under, mut shown, mut below) = (0, 0, 0);
    for (x, y) in body() {
        if (70..90).contains(&y) {
            under += 1;
            assert_ne!(at(&whole, x, y), at(&bare, x, y), "({x}, {y}): she is drawn over the roof when not behind it");
            assert_eq!(at(&hid, x, y), at(&bare, x, y), "({x}, {y}): the roof hides her");
            shown += usize::from(at(&seen, x, y) != at(&bare, x, y));
        } else if y >= 90 {
            below += 1;
            assert_eq!(at(&hid, x, y), at(&whole, x, y), "({x}, {y}): below the roof she is drawn");
        }
    }
    assert!(under > 100 && below > 10);
    assert!(shown * 3 > under && shown * 3 < under * 2, "seen through: {shown} of {under} px show");
}

/// Seen through what stands in front (PRESENTATION.md §1.6): a prop standing in front of her
/// covers her lower half, and her copy drawn after the standing things (`Tint::Seen`) shows one px
/// in two of what it covers, on the canvas's checker; the rest stays covered.
#[test]
fn a_prop_in_front_of_her_is_seen_through_on_the_checker() {
    let Some(mut b) = backend() else { return };
    let mut atlas = atlas(|y| height_of_rows(AY - y).max(1) as u8);
    // The page doubled: her on the left, a block of another colour on the right.
    let p = &mut atlas.pages[0];
    let (w, h) = (usize::from(SW), usize::from(SH));
    let mut albedo = vec![0; 2 * w * h];
    for y in 0..h {
        albedo[y * 2 * w..y * 2 * w + w].copy_from_slice(&p.albedo[y * w..y * w + w]);
        for x in 0..w {
            albedo[y * 2 * w + w + x] = if y >= 20 { 3 } else { 0 };
        }
    }
    p.height =
        (0..h).flat_map(|y| p.height[y * w..y * w + w].iter().chain(&p.height[y * w..y * w + w]).copied()).collect();
    p.normal =
        (0..h).flat_map(|y| p.normal[y * w..y * w + w].iter().chain(&p.normal[y * w..y * w + w]).copied()).collect();
    p.emissive = vec![0; 2 * w * h];
    p.albedo = albedo;
    p.w = 2 * SW;
    atlas.clut[3] = 0xff20_3090;
    let mut f = frame(ground(|_, _| None), true, None, &[]);
    let her = f.sprites[0];
    // The block stands 8 px in front of her.
    f.sprites.push(SpriteCmd { src: Src { x: SW, ..her.src }, y: her.y + 8, ..her });
    f.passes.retain(|q| !matches!(q, Pass::Sprites { .. }));
    let pos = f.passes.iter().position(|q| matches!(q, Pass::Lights { .. })).expect("a light pass");
    f.passes.insert(pos, Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 2 } });
    let bare = draw(&mut b, &atlas, &f);
    f.sprites.push(SpriteCmd { flags: Flags { mirror: false, tint: jane_present::Tint::Seen }, ..her });
    f.passes.insert(pos + 1, Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 2, len: 1 } });
    let seen = draw(&mut b, &atlas, &f);
    let (mut on, mut off) = (0, 0);
    for (x, y) in body() {
        let covered = y - (FOOT.1 - AY) >= 28;
        if !covered {
            assert_eq!(at(&seen, x, y), at(&bare, x, y), "({x}, {y}): uncovered, as she was");
        } else if jane_present::Tint::seen_at(x, y) {
            on += usize::from(at(&seen, x, y) != at(&bare, x, y));
        } else {
            off += 1;
            assert_eq!(at(&seen, x, y), at(&bare, x, y), "({x}, {y}): stays covered");
        }
    }
    assert!(on > 20 && off > 20, "seen through: {on} px show, {off} stay covered");
}

#[test]
fn a_light_never_shadows_the_one_who_holds_it() {
    let Some(mut b) = backend() else { return };
    let atlas = atlas(|y| height_of_rows(AY - y).max(1) as u8);
    // Her lantern, at her west side, her hip high, over the rows her body stands on (her depth
    // lies behind her foot row).
    let lantern = |holder| Light {
        pos: (FOOT.0 - 9, FOOT.1 - 2),
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

/// A sprite `w` x `h` whose px `(x, y)` is drawn where `shape` says, standing its row's true
/// height over its foot row `ay` (upright, as `Canvas::upright` stands a sprite).
fn upright_atlas(w: u16, h: u16, ay: i32, shape: impl Fn(i32, i32) -> bool) -> AtlasPages {
    let n = usize::from(w) * usize::from(h);
    let mut page = Page {
        w,
        h,
        albedo: vec![0; n],
        normal: vec![[128, 200]; n],
        emissive: vec![0; n],
        height: vec![0; n],
        glow: Vec::new(),
    };
    for y in 0..i32::from(h) {
        for x in 0..i32::from(w) {
            if shape(x, y) {
                let i = y as usize * usize::from(w) + x as usize;
                page.albedo[i] = 2;
                page.height[i] = height_of_rows(ay - y).max(1) as u8;
            }
        }
    }
    let mut clut = vec![0xff00_0000; CLUT_LEN];
    clut[2] = 0xffc0_8060;
    AtlasPages { clut, pages: vec![page], mist: Vec::new() }
}

/// A frame of flat grass and the one sprite of `atlas`, its foot row `ay` on canvas `foot`, its
/// middle column over the foot, lit by `sun` alone.
fn stand(atlas: &AtlasPages, ay: i32, foot: (i32, i32), depth: u8, sun: Directional) -> Frame {
    let p = &atlas.pages[0];
    let top = p.height.iter().copied().max().unwrap_or(1);
    let mut f = frame(ground(|_, _| None), false, None, &[]);
    f.sprites.push(SpriteCmd {
        page: 0,
        src: Src { x: 0, y: 0, w: p.w, h: p.h },
        x: (foot.0 - i32::from(p.w) / 2) as i16,
        y: (foot.1 - ay) as i16,
        flags: Flags::default(),
        height_px: top,
        foot: None,
    });
    f.casters.push(Caster { sprite: 0, foot: (foot.0 as i16, foot.1 as i16), height: top, depth, ..Caster::default() });
    f.passes.clear();
    f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: 1 } });
    f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 1 } });
    f.passes.push(Pass::Lights {
        ambient: [200; 3],
        fill: [70, 78, 120],
        sun: Some(sun),
        points: Span { start: 0, len: 0 },
        casters: Span { start: 0, len: 1 },
        blocks: Span::default(),
    });
    f.passes.push(Pass::Post(Post::NONE));
    f
}

/// A sun in the north at 40 degrees: shadows run south, down the screen, in front of what
/// casts them, so the whole of one is seen.
/// A post-and-rail fence across the view as the painter draws it, six cells from x 80, its foot
/// on row 95: each cell's post 6 px wide (rows 72 to 95, 28 px at its top), the rails across
/// rows 77 to 79 (21 to 18 px) and 85 to 87 (11 to 8), every px of it marked a fence's and
/// standing at least 5 px up; and its posts and rails as the frame's blocks.
fn fence() -> (ChunkLayers, Vec<Block>) {
    let post = |x: i32| (80..176).contains(&x) && (x - 80) % 16 >= 5 && (x - 80) % 16 < 11;
    let rail = |x: i32, y: i32| (80..176).contains(&x) && matches!(y, 77..=79 | 85..=87);
    let mut l =
        ground(|x, y| (post(x) && (72..=95).contains(&y) || rail(x, y)).then(|| height_of_rows(95 - y).max(5) as u8));
    for y in 0..CHUNK_PX {
        for x in 0..CHUNK_PX {
            if post(x) && (72..=95).contains(&y) || rail(x, y) {
                let k = (y * CHUNK_PX + x) as usize;
                l.fence[k / 64] |= 1 << (k % 64);
            }
        }
    }
    let part = |x0: i16, x1: i16, lo: u8, height: u8| Block { x0, y0: 94, x1, y1: 96, height, lo, fence: true };
    let mut blocks: Vec<Block> = (0..6).map(|i| part(85 + 16 * i, 91 + 16 * i, 0, 28)).collect();
    blocks.extend([part(80, 176, 7, 11), part(80, 176, 17, 21)]);
    (l, blocks)
}

/// `f` with `blocks` as its terrain's.
fn with_blocks(mut f: Frame, blocks: &[Block]) -> Frame {
    f.blocks.extend_from_slice(blocks);
    for p in &mut f.passes {
        if let Pass::Lights { blocks: b, .. } = p {
            *b = Span { start: 0, len: blocks.len() as u32 };
        }
    }
    f
}

#[test]
fn a_fence_lays_the_bands_t0_and_t1_lay_on_the_ground_alone_and_a_wall_spills_nothing() {
    // A fence's sun shadow (PRESENTATION.md §1.7) is its posts' lines and its two rails' along
    // the true sun, the bands `shadow::block_bands` gives T0 and T1; it never darkens the fence.
    // A sun in the north throws it down the screen, where it is seen whole.
    let Some(mut b) = backend() else { return };
    let (layers, blocks) = fence();
    let px = draw(&mut b, &atlas(|_| 1), &with_blocks(frame(layers, false, Some(north_sun()), &[]), &blocks));
    let lit = luma(at(&px, 30, 140));
    let dark = |x: i32, y: i32| luma(at(&px, x, y)) * 10 < lit * 8;
    let k = shear(&north_sun()).unwrap();
    let mut bands = Vec::new();
    for bl in &blocks {
        shadow::block_bands(bl, k, |band| bands.push(band));
    }
    let covered =
        |x: i32, y: i32, r: i32| bands.iter().any(|b| b.x0 - r <= x && x < b.x1 + r && b.y0 - r <= y && y < b.y1 + r);
    let (mut agree, mut n) = (0, 0);
    for y in (97..H as i32).step_by(2) {
        for x in (0..W as i32).step_by(2) {
            if covered(x, y, 0) && !covered(x, y, -3) {
                continue;
            }
            // Well inside a band: dark; three px from any: lit.
            if !covered(x, y, 3)
                || covered(x, y, -3)
                || bands.iter().any(|b| b.x0 + 3 <= x && x < b.x1 - 3 && b.y0 <= y && y < b.y1)
            {
                n += 1;
                agree += i32::from(dark(x, y) == covered(x, y, 0));
            }
        }
    }
    assert!(n > 500 && agree * 100 >= n * 97, "T2 lays {agree} of {n} px as the bands do");
    // A post's line starts at its foot; down a column between two posts, the rails' lines with
    // lit grass under the lower one and between them (cot 40 degrees: 1.19 rows a px).
    assert!(dark(104, 97) && dark(104, 110), "no post's line from its foot");
    let col: Vec<bool> = (96..130).map(|y| dark(113, y)).collect();
    let lines = col.windows(2).filter(|w| !w[0] && w[1]).count() + usize::from(col[0]);
    assert!(lines >= 2 && !dark(113, 99), "no floating rails in the fence's shadow: {col:?}");
    // The fence (the grass's albedo, facing the low sun at five, which throws its shadow east
    // along it) is never in its own shadow.
    let (layers, _) = fence();
    let px = draw(&mut b, &atlas(|_| 1), &with_blocks(frame(layers, false, Some(five()), &[]), &blocks));
    let lit = luma(at(&px, 30, 140));
    for x in (82..172).step_by(8) {
        for y in [78, 86] {
            assert!(luma(at(&px, x, y)) * 10 >= lit * 8, "a rail shadowed at ({x}, {y}): {}", luma(at(&px, x, y)));
        }
    }
    // A wall's block a cell deep and 13 px spills nothing south, and keeps its true shadow.
    let wall = ground(|x, y| {
        if !(80..176).contains(&x) {
            return None;
        }
        match y {
            40..=79 => Some(height_of_rows(16) as u8),
            80..=95 => Some(height_of_rows(96 - y).max(1) as u8),
            _ => None,
        }
    });
    // (A chunk is held by its generation: a fresh backend, or the fence would be drawn again.)
    let Some(mut b) = backend() else { return };
    let px = draw(&mut b, &atlas(|_| 1), &frame(wall, false, Some(five()), &[]));
    let lit = luma(at(&px, 30, 120));
    assert!(luma(at(&px, 128, 104)) * 10 >= lit * 8, "the wall spills south");
    assert!(luma(at(&px, 190, 85)) * 10 < lit * 8, "the wall lost its true shadow");
}

fn north_sun() -> Directional {
    Directional {
        azimuth: Angle::NORTH,
        elevation: Angle::from_degrees(40),
        colour: [255, 230, 200],
        spread: 300,
        strength: 255,
    }
}

/// How many px of canvas row `y` are in shadow: darker than four fifths of the open grass.
fn dark_across(px: &[u32], y: i32) -> usize {
    let lit = luma(at(px, 4, y));
    (0..W as i32).filter(|&x| luma(at(px, x, y)) * 10 < lit * 8).count()
}

#[test]
fn a_trees_shadow_is_its_trunks_at_the_root_and_its_crowns_further_out() {
    // A crown 32 wide from row 4 to row 30, on a trunk 4 wide from row 31 to its foot on row 56:
    // the crown floats 33 px to 65 px up.
    const AY: i32 = 56;
    let Some(mut b) = backend() else { return };
    let tree = upright_atlas(40, 60, AY, |x, y| {
        let crown = (4..=30).contains(&y) && (4..36).contains(&x);
        let trunk = (31..=AY).contains(&y) && (18..22).contains(&x);
        crown || trunk
    });
    let foot = (128, 62);
    let px = draw(&mut b, &tree, &stand(&tree, AY, foot, 6, north_sun()));
    // cot 40 degrees is 1.19: the trunk's shadow lies on the 39 rows south of its foot and the
    // crown's from there to 77 rows. Near the root, a trunk and its penumbra; no wider.
    for dy in 6..30 {
        let n = dark_across(&px, foot.1 + dy);
        assert!(n <= 4 + 2 + 4, "{dy} rows south of the root: {n} px dark, the crown's shadow stands on the root");
        assert!(n >= 3, "{dy} rows south of the root: {n} px dark, the trunk throws nothing");
    }
    // Further out, the crown's width.
    for dy in 50..70 {
        let n = dark_across(&px, foot.1 + dy);
        assert!(n >= 28, "{dy} rows south of the root: {n} px dark, no crown's shadow");
    }
}

#[test]
fn a_lanterns_shadow_hangs_apart_from_its_posts() {
    // A post 2 wide from its foot on row 56 up to row 8, an arm out east along rows 8 and 9, and
    // a lantern 8 wide hanging from it, rows 10 to 22 (43 px to 58 px up), 8 px clear of the post.
    const AY: i32 = 56;
    let Some(mut b) = backend() else { return };
    let lamp = upright_atlas(40, 60, AY, |x, y| {
        let post = (8..=AY).contains(&y) && (10..12).contains(&x);
        let arm = (8..=9).contains(&y) && (10..28).contains(&x);
        let lantern = (10..=22).contains(&y) && (20..28).contains(&x);
        post || arm || lantern
    });
    let foot = (128, 62);
    let px = draw(&mut b, &lamp, &stand(&lamp, AY, foot, 4, north_sun()));
    let x0 = foot.0 - 20;
    let lit = luma(at(&px, 4, foot.1 + 20));
    let dark = |x: i32, y: i32| luma(at(&px, x, y)) * 10 < lit * 8;
    // Near the root, only the post's thin shadow: the ground under where the lantern hangs is
    // lit.
    for dy in 6..30 {
        let y = foot.1 + dy;
        assert!(!(x0 + 21..x0 + 27).any(|x| dark(x, y)), "{dy} rows south: the lantern's shadow stands on the root");
        assert!((x0 + 9..x0 + 13).any(|x| dark(x, y)), "{dy} rows south: the post throws nothing");
    }
    // Further out, the lantern's own shadow, the ground between it and the post's lit.
    let y = foot.1 + 58;
    assert!((x0 + 21..x0 + 27).all(|x| dark(x, y)), "no shadow of the lantern where it lands");
    assert!((x0 + 14..x0 + 18).any(|x| !dark(x, y)), "the lantern's shadow runs into the post's");
}

/// A post `w` px wide and 48 rows tall (60 px), standing on `(128, 40)`.
fn post(w: i32) -> (AtlasPages, i32, (i32, i32)) {
    const AY: i32 = 50;
    (
        upright_atlas(24, 52, AY, |x, y| (2..=AY).contains(&y) && (12 - w / 2..12 - w / 2 + w).contains(&x)),
        AY,
        (128, 40),
    )
}

#[test]
fn a_thin_post_throws_a_thin_shadow() {
    let Some(mut b) = backend() else { return };
    let (atlas, ay, foot) = post(2);
    let px = draw(&mut b, &atlas, &stand(&atlas, ay, foot, 4, north_sun()));
    // Down the screen from its root: its 2 px and a px of penumbra each side at most.
    for dy in 4..40 {
        let n = dark_across(&px, foot.1 + dy);
        assert!((1..=4).contains(&n), "{dy} rows south of the root: {n} px dark");
    }
}

/// A post 12 px wide's shadow 40 rows south of it under a sun 40 degrees up in the north, with the
/// spread and strength a clear sky gives a sun `deg` up (the shape stays the sun's at 40, so only
/// the softness and the darkness change) and `cloud` over it: `(px of penumbra, the umbra's luma
/// of the open grass's, per mille)`.
fn softness(b: &mut Wgpu, deg: i32, cloud: u32) -> (usize, i32) {
    let s = jane_core::angle::sin_q15(Angle::from_degrees(deg)).0;
    let mut sun = Directional {
        spread: jane_present::light::spread(s),
        strength: jane_present::light::strength(s),
        ..north_sun()
    };
    jane_present::light::diffuse(&mut sun, cloud);
    let (atlas, ay, foot) = post(12);
    let px = draw(b, &atlas, &stand(&atlas, ay, foot, 4, sun));
    // Its umbra 30 rows down, and the rows of penumbra at its tip (70 rows down, where the ray
    // clears its top) and across it 50 rows down.
    let lit = luma(at(&px, 4, foot.1 + 30));
    let umbra = luma(at(&px, foot.0, foot.1 + 30));
    let drop = (lit - umbra).max(1);
    let soft = |v: i32| v < lit - drop / 8 && v > umbra + drop / 8;
    let tip = (foot.1 + 40..i32::from(H)).filter(|&y| soft(luma(at(&px, foot.0, y)))).count();
    let side = (foot.0 - 16..foot.0 + 16).filter(|&x| soft(luma(at(&px, x, foot.1 + 50)))).count();
    (tip + side, umbra * 1000 / lit.max(1))
}

#[test]
fn a_noon_shadow_is_crisper_and_darker_than_five_oclocks_and_cloud_fades_it() {
    let Some(mut b) = backend() else { return };
    let (noon_edge, noon) = softness(&mut b, 46, 0);
    let (five_edge, five) = softness(&mut b, 16, 0);
    assert!(noon_edge < five_edge, "the penumbra at noon {noon_edge} px, at five {five_edge}");
    assert!(noon < five, "the umbra at noon {noon} of the lit grass, at five {five}: no darker");
    // Mist, then rain: fainter each.
    let (_, mist) = softness(&mut b, 46, 65535 * 3 / 8);
    let (_, rain) = softness(&mut b, 46, 65535);
    assert!(noon < mist && mist < rain, "noon {noon}, in mist {mist}, in rain {rain}");
}

#[test]
fn a_lamp_throws_a_fences_posts_and_floating_rails_far_off_and_right_against_it() {
    // The owner's second playtest (2026-09-29): a lamp or her lantern saw an E-W fence as a wall,
    // and right against it saw through it. The fence's parts stand in the field with their bars
    // (`scatter.wgsl`'s `fences`), as T0's and T1's lamps throw them (`shadow::block_slabs`).
    // Each frame is drawn with the fence's blocks and without (its px alone stand in no field).
    let (layers, blocks) = fence();
    let lamp = |pos: (i32, i32), height: u8| Light {
        pos,
        height,
        colour: [255, 220, 170],
        radius: 140,
        size: 3,
        casts: true,
        kind: LightKind::Point,
        holder: None,
    };
    let shot = |l: Light, with: bool| {
        let mut b = backend()?;
        let f = frame(layers.clone(), false, None, &[l]);
        let f = if with { with_blocks(f, &blocks) } else { f };
        Some(draw(&mut b, &atlas(|_| 1), &f))
    };
    // A lamp 40 px up, 35 px south of the fence, between two posts (x 107..117).
    let far = lamp((112, 130), 40);
    let (Some(with), Some(open)) = (shot(far, true), shot(far, false)) else { return };
    let dark = |x: i32, y: i32| luma(at(&with, x, y)) * 10 < luma(at(&open, x, y)) * 8;
    // Up the column between the posts: lit under the lower rail, its line, lit between the two
    // rails, the upper one's line (t = d 40 / (40 - z) from the lamp: rows 79 to 88 and 56 to 69, the
    // penumbra round them).
    let col: Vec<bool> = (40..95).map(|y| dark(112, y)).collect();
    assert!(!dark(112, 92), "the ground under the lower rail is in its shadow: the fence is a wall: {col:?}");
    assert!(dark(112, 83), "the lower rail throws nothing: {col:?}");
    assert!(!dark(112, 78), "no gap between the rails' lines: {col:?}");
    assert!(dark(112, 62), "the upper rail throws nothing: {col:?}");
    // Behind a post (x 101..107, 0 to 28 px), along the ray from the lamp through it, dark far
    // up the screen.
    assert!(dark(96, 60) && dark(92, 45), "a post throws nothing");
    // Her lantern, 18 px up, three rows south of the fence's foot, right against it.
    let near = lamp((112, 98), 18);
    let (Some(with), Some(open)) = (shot(near, true), shot(near, false)) else { return };
    let dark = |x: i32, y: i32| luma(at(&with, x, y)) * 10 < luma(at(&open, x, y)) * 8;
    let col: Vec<bool> = (40..95).map(|y| dark(112, y)).collect();
    // Its posts either side (x 101..107 and 117..123) throw their shadows out nearly level with
    // the fence: on the ground between the next posts, a row or two up from its foot.
    assert!(dark(96, 92) && dark(128, 92), "the light passes through the posts right against them");
    assert!(dark(112, 91), "the light passes under the lower rail right against it: {col:?}");
    // Between the lower rail's line and the upper rail's, lit: the upper rail's underside (17 px)
    // is a px over the lantern, so its soft shadow (the lantern is 3 px across) takes most of
    // the ground past a few rows; T0 and T1, lighting from a point, lay it from 63 px out.
    assert!(!dark(112, 84), "the fence is a wall to a light right against it: {col:?}");
}
