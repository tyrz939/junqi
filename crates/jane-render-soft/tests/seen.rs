//! Seen through what stands in front (PRESENTATION.md §1.6): whatever is drawn over her, the
//! part of her it covers shows through on the canvas's checker, one rule for every occluder. She
//! stands behind each kind in turn: a house's roof and a wall in the terrain, and a building, a
//! tall prop and a tree's crown standing in front of her in the y-sorted list, and after the
//! standing things she is drawn again (`Tint::Seen`).

use jane_present::frame::CHUNK_PX;
use jane_present::{
    AtlasPages, Backend, CLUT_LEN, ChunkCmd, ChunkId, ChunkLayers, Depth, Flags, Foot, Frame, Page, Pass, Span,
    SpriteCmd, Src, Tier, Tint,
};
use jane_render_soft::Soft;

const W: u16 = 128;
const H: u16 = 96;
const GRASS: u32 = 0xff70_8050;
/// Her colour (index 2) and the occluders' (index 3).
const HER: u32 = 0xffc0_4040;
const THING: u32 = 0xff30_3070;
/// Her box on the canvas: 12 x 24, feet on row 60.
const X0: i32 = 58;
const Y0: i32 = 36;
const SW: u16 = 12;
const SH: u16 = 24;

/// What stands in front of her.
#[derive(Clone, Copy, Debug)]
enum Occluder {
    /// A house's roof in the ground layers, 60 px up over rows 30..56, standing on ground south of her.
    Roof,
    /// A wall's face in the ground layers, rising from row 64 to 24 px.
    Wall,
    /// A building prop 64 x 40 whose foot is south of hers.
    Building,
    /// A post 4 px wide and 40 tall.
    TallProp,
    /// A tree's crown: a disc with holes.
    Crown,
}

/// The atlas: page 0 is her, page 1 the occluder's sprite (64 x 48, drawn by `shape`).
fn atlas(shape: impl Fn(i32, i32) -> bool) -> AtlasPages {
    let (sw, sh) = (usize::from(SW), usize::from(SH));
    let her = Page { w: SW, h: SH, albedo: vec![2; sw * sh].into(), ..Page::default() };
    let (ow, oh) = (64usize, 48usize);
    let mut thing = Page { w: ow as u16, h: oh as u16, albedo: vec![0; ow * oh].into(), ..Page::default() };
    for y in 0..oh {
        for x in 0..ow {
            if shape(x as i32, y as i32) {
                std::sync::Arc::make_mut(&mut thing.albedo)[y * ow + x] = 3;
            }
        }
    }
    let mut clut = vec![0xff00_0000; CLUT_LEN];
    clut[2] = HER;
    clut[3] = THING;
    AtlasPages { clut, pages: vec![her, thing], mist: Vec::new() }
}

fn frame(o: Occluder) -> (AtlasPages, Frame) {
    let mut layers = ChunkLayers::new(Tier::T0);
    for y in 0..CHUNK_PX {
        for x in 0..CHUNK_PX {
            let i = (y * CHUNK_PX + x) as usize;
            layers.albedo[i] = GRASS;
            layers.height[i] = match o {
                Occluder::Roof if (30..56).contains(&y) && (40..90).contains(&x) => 60,
                Occluder::Wall if (40..64).contains(&y) && (40..90).contains(&x) => (64 - y) as u8 + 4,
                _ => 1,
            };
            if layers.height[i] > 1 {
                layers.albedo[i] = THING;
            }
        }
    }
    let (atlas, thing) = match o {
        Occluder::Building => (atlas(|_, y| y >= 8), Some((32, 24, 64))),
        Occluder::TallProp => (atlas(|x, y| (30..34).contains(&x) && y >= 8), Some((32, 24, 64))),
        Occluder::Crown => {
            (atlas(|x, y| (x - 32).pow(2) + (y - 20).pow(2) < 18 * 18 && (x * 7 + y * 3) % 5 != 0), Some((32, 20, 64)))
        }
        Occluder::Roof | Occluder::Wall => (atlas(|_, _| false), None),
    };
    let mut f = Frame::new(Tier::T0);
    f.canvas = (W, H);
    f.clear = GRASS;
    f.layers = vec![layers];
    f.chunks.push(ChunkCmd { id: ChunkId { cx: 0, cy: 0 }, generation: 1, x: 0, y: 0, slot: 0 });
    f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: 1 } });
    let her = SpriteCmd {
        page: 0,
        src: Src { x: 0, y: 0, w: SW, h: SH },
        x: X0 as i16,
        y: Y0 as i16,
        flags: Flags::default(),
        height_px: 30,
        foot: matches!(o, Occluder::Roof | Occluder::Wall).then_some(Foot::at((Y0 + i32::from(SH)) as i16, true)),
    };
    f.sprites.push(her);
    if let Some((x, y, _)) = thing {
        f.sprites.push(SpriteCmd {
            page: 1,
            src: Src { x: 0, y: 0, w: 64, h: 48 },
            x: x as i16,
            y: y as i16,
            flags: Flags::default(),
            height_px: 40,
            foot: None,
        });
    }
    let n = f.sprites.len() as u32;
    f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: n } });
    f.sprites.push(SpriteCmd {
        flags: Flags { mirror: false, tint: Tint::Seen, bend: jane_present::Bend::NONE },
        foot: None,
        ..her
    });
    f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: n, len: 1 } });
    (atlas, f)
}

#[test]
fn behind_every_kind_of_occluder_she_shows_through_on_the_checker() {
    for o in [Occluder::Roof, Occluder::Wall, Occluder::Building, Occluder::TallProp, Occluder::Crown] {
        let (atlas, f) = frame(o);
        let mut soft = Soft::new();
        soft.upload_atlas(&atlas);
        soft.draw(&f);
        let (px, w, _) = soft.pixels();
        let px = px.to_vec();
        let at = |x: i32, y: i32| px[(y * i32::from(w) + x) as usize];
        // Drawn without her seen copy: what covers her.
        let mut bare = f;
        bare.passes.pop();
        bare.sprites[0].foot = bare.sprites[0].foot.map(|f| Foot { see: false, ..f });
        soft.draw(&bare);
        let (px2, _, _) = soft.pixels();
        let covered: Vec<(i32, i32)> = (Y0..Y0 + i32::from(SH))
            .flat_map(|y| (X0..X0 + i32::from(SW)).map(move |x| (x, y)))
            .filter(|&(x, y)| px2[(y * i32::from(w) + x) as usize] != HER)
            .collect();
        assert!(covered.len() >= 12, "{o:?}: something covers her ({} px)", covered.len());
        let (mut on, mut off) = (0, 0);
        for &(x, y) in &covered {
            if Tint::seen_at(x, y) {
                assert_eq!(at(x, y), HER, "{o:?}: ({x}, {y}) of her shows through");
                on += 1;
            } else {
                assert_ne!(at(x, y), HER, "{o:?}: ({x}, {y}) stays covered");
                off += 1;
            }
        }
        assert!(on > 0 && off > 0, "{o:?}: {on} shown, {off} covered");
        // Where nothing covers her, she is as she was.
        for y in Y0..Y0 + i32::from(SH) {
            for x in X0..X0 + i32::from(SW) {
                if !covered.contains(&(x, y)) {
                    assert_eq!(at(x, y), HER, "{o:?}: ({x}, {y}) uncovered");
                }
            }
        }
    }
}
