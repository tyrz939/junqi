//! The night's look (NIGHT.md §4): what the presenter lays over the county after the bell, from
//! the blueprint and the stage alone, never the sim.
//!
//! - **The albedo** ([`jane_art::palette::night_ix`], §4.2): a sprite reads its albedo through
//!   the night's CLUT for the intensity where she stands (`Frame::night`, [`clut`]); a PC's chunk
//!   is painted in the night's materials cell by cell (`TileSource::night`); a console folds the
//!   night into its CLUTs instead (no repaint).
//! - **The overlays** ([`jane_art::night`], §4.3): each socket the painter found in a chunk (a
//!   window, a wall face, a fence's rail, a road cell) takes a look when a hash of the seed and
//!   the socket falls under the intensity's density (§4.4): [`place`]. A PC lays them into the
//!   chunk's layers as it is painted ([`composite`]); a console draws them as sprites from its
//!   night page, nearest her first, under its preset's cap.
//! - **The lamps** (§4.5): a wrong lamp's glass, light and flicker ([`GLASS_COLD`],
//!   [`POOL_COLD`], [`cold_flicker`]).
//! - **The sky** (§4.6, the owner's request of 9 October): the stars thin at intensity 3 and are
//!   gone at 4 under the Works' soot ([`stars_share`]); the moon is kept, dimmer under the soot.
//!
//! Everything here is a pure function of the seed, the blueprint's cells and the stage, so every
//! seat and every tier lays the same night, and nothing of it is saved.

use alloc::vec::Vec;

use jane_art::Canvas;
use jane_art::hash::h32;
use jane_art::night::{self as kit, Look, Opening};
use jane_art::palette::{self, Ix};
use jane_art::terrain::Chunk;

use crate::frame::{CELL, CHUNK_CELLS, CHUNK_PX, Rgb, rows_up};

/// The renderers' CLUT at night intensity `i`: a sprite's albedo through it is the night's.
pub fn clut(i: u8) -> Vec<u32> {
    palette::night_clut(i)
}

/// The most intensity.
pub const MAX: u8 = palette::NIGHT_MAX;

/// A wrong lamp's glass at its brightest, and the pool it lays (NIGHT.md §4.5).
pub const GLASS_COLD: Rgb = [0xc8, 0xee, 0xd6];
/// A wrong lamp's light at its middle, before its slow pulse: what lays the cold pool.
pub const LIGHT_COLD: Rgb = [0x78, 0xcc, 0xaa];
/// A wrong lamp's pool on the ground: its light's colour.
pub const POOL_COLD: Rgb = [0x2e, 0x5c, 0x50];

/// A wrong lamp's flicker this tick, 0..=255 of full: a slow uneven pulse (a breath every few
/// seconds, never a flame's quick dip), from a place set by `id`.
pub fn cold_flicker(tick: u32, id: u32) -> u8 {
    // Two slow triangles out of step: about 3.1 s and 4.7 s.
    let tri = |t: u32, period: u32| {
        let p = t % period;
        let half = period / 2;
        if p < half { p * 256 / half } else { (period - p) * 256 / half }
    };
    let t = tick.wrapping_add(id.wrapping_mul(2_654_435_761) >> 20);
    let a = tri(t, 187);
    let b = tri(t.wrapping_add(53), 283);
    (190 + (a + b) * 65 / 512).min(255) as u8
}

/// The share of the stars a night at intensity `i` (where she stands) shows, of 256 (NIGHT.md
/// §4.6: 120, 120, 60, 0 of 120): the Works' soot thins them at 3 and hides them at 4.
pub const fn stars_share(i: u8) -> u32 {
    match i {
        0..=2 => 256,
        3 => 128,
        _ => 0,
    }
}

/// The moon's light at intensity `i`, of 256: dimmer under the soot at 4.
pub const fn moon_share(i: u8) -> u32 {
    if i >= 4 { 176 } else { 256 }
}

/// One overlay laid in a chunk: the look ([`jane_art::night::all`]'s index), the chunk-local px
/// its anchor is laid on, mirrored or not, and for what hangs (a coat on a rail, a strip) the
/// height of what it hangs from, px (0: it lies flat on what is under it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub look: u16,
    pub x: i16,
    pub y: i16,
    pub mirror: bool,
    pub hang: u8,
}

/// The night kit as packed: each look's canvas and anchor, in [`jane_art::night::all`]'s order.
#[derive(Debug)]
pub struct Kit {
    pub looks: Vec<Look>,
    pub canvases: Vec<Canvas>,
    pub anchors: Vec<(i16, i16)>,
}

impl Kit {
    /// Every look rendered once.
    pub fn build() -> Kit {
        let looks = kit::all();
        let canvases: Vec<Canvas> = looks.iter().map(|&l| kit::render(l)).collect();
        let anchors = looks
            .iter()
            .map(|&l| {
                let (x, y) = kit::anchor(l);
                (x as i16, y as i16)
            })
            .collect();
        Kit { looks, canvases, anchors }
    }

    /// The index of `look`.
    pub fn index(looks: &[Look], look: Look) -> u16 {
        looks.iter().position(|&l| l == look).unwrap_or(0) as u16
    }
}

/// The night kit in the atlas: each look's sprite, anchored on its socket's point, in
/// [`jane_art::night::all`]'s order (what a console draws its overlays from: one page).
#[derive(Clone, Debug, Default)]
pub struct NightArt {
    pub refs: Vec<crate::atlas::RefId>,
}

impl NightArt {
    /// Packs every look of `kit`, keyed for the bake's night page.
    pub fn build(atlas: &mut crate::atlas::Atlas, kit: &Kit) -> NightArt {
        let refs = kit
            .canvases
            .iter()
            .zip(&kit.anchors)
            .enumerate()
            .map(|(i, (c, &a))| {
                atlas.key_next(crate::atlas::Key { cat: crate::atlas::cat::NIGHT, sprite: i as u16, vs: 0, frame: 0 });
                let top = (0..c.h()).flat_map(|y| (0..c.w()).map(move |x| (x, y))).map(|(x, y)| c.height_at(x, y));
                atlas.add_canvas(c, a, top.max().unwrap_or(1).max(1), |_, _, t| t)
            })
            .collect();
        NightArt { refs }
    }
}

crate::tables::tab_struct!(NightArt { refs });

/// Per mille of the sockets that take each overlay at each intensity (NIGHT.md §4.4).
struct Density {
    board: u32,
    brick: u32,
    /// Of the lit windows, gone dark (a cracked dark pane over the glow).
    dark: u32,
    card: u32,
    rust: u32,
    peel: u32,
    pipe: u32,
    chalk: u32,
    grate: u32,
    stain: u32,
    coat: u32,
    strip: u32,
}

const fn density(i: u8) -> Density {
    let z = Density {
        board: 0,
        brick: 0,
        dark: 0,
        card: 0,
        rust: 0,
        peel: 0,
        pipe: 0,
        chalk: 0,
        grate: 0,
        stain: 0,
        coat: 0,
        strip: 0,
    };
    match i {
        0 => z,
        1 => Density { board: 83, rust: 100, peel: 30, chalk: 30, ..z },
        2 => Density {
            board: 200,
            brick: 50,
            dark: 250,
            card: 40,
            rust: 180,
            peel: 60,
            chalk: 36,
            stain: 14,
            coat: 520,
            strip: 160,
            ..z
        },
        3 => Density {
            board: 250,
            brick: 100,
            dark: 250,
            card: 50,
            rust: 260,
            peel: 90,
            pipe: 120,
            chalk: 42,
            grate: 30,
            stain: 20,
            coat: 640,
            strip: 240,
        },
        _ => Density {
            board: 333,
            brick: 125,
            dark: 200,
            card: 60,
            rust: 340,
            peel: 120,
            pipe: 240,
            chalk: 48,
            grate: 45,
            stain: 26,
            coat: 760,
            strip: 320,
        },
    }
}

/// Salts, one a kind of socket.
mod salt {
    pub const WINDOW: u32 = 0x4e57_494e;
    pub const FACE: u32 = 0x4e46_4143;
    pub const WAY: u32 = 0x4e57_4159;
    pub const RAIL: u32 = 0x4e52_4149;
    pub const FRONT: u32 = 0x4e46_524f;
}

/// What a painted chunk offers the night's overlays (NIGHT.md §4.3): its windows, its house
/// fronts, its road cells and its fences' rails, kept a slot so the night can be laid again at the
/// turn without painting the chunk again (a console's: its CLUTs carry the night's colours).
#[derive(Clone, Debug, Default)]
pub struct Sockets {
    pub openings: Vec<jane_art::terrain::Opening>,
    pub faces: Vec<jane_art::terrain::Face>,
    pub ways: [u16; CHUNK_CELLS as usize],
    pub rails: Vec<jane_art::terrain::FencePart>,
}

impl Sockets {
    /// Reserved once: a chunk's worth.
    pub fn new() -> Sockets {
        Sockets {
            openings: Vec::with_capacity(32),
            faces: Vec::with_capacity(96),
            ways: [0; CHUNK_CELLS as usize],
            rails: Vec::with_capacity(64),
        }
    }

    /// `chunk`'s, as painted.
    pub fn copy_from(&mut self, chunk: &Chunk) {
        self.openings.clear();
        self.openings.extend_from_slice(&chunk.openings);
        self.faces.clear();
        self.faces.extend_from_slice(&chunk.faces);
        self.ways = chunk.ways;
        self.rails.clear();
        self.rails.extend(chunk.fences.iter().filter(|p| p.lo > 0 && p.x1 - p.x0 >= 6));
    }
}

/// The board layouts and their mirrors a front walks through, so the next window along it (one to
/// four cells on, or the one over it) never takes the same boards (ART.md §3.1, "no stamp twice
/// in view").
const BOARD_WALK: [(u8, bool); 5] = [(0, false), (1, false), (2, false), (0, true), (1, true)];

/// The overlays of chunk `(cx, cy)` of its zone, by its sockets `s`, at night stage `stage`, into `out`:
/// `intensity(x, y)` is the night's at world cell `(x, y)` (`jane_sim::night::NightMap`). A pure
/// function of the seed, the chunk's sockets and the intensities: the same on every seat and tier.
pub fn place(
    s: &Sockets,
    (cx, cy): (i32, i32),
    seed: u32,
    stage: u8,
    intensity: &dyn Fn(i32, i32) -> u8,
    looks: &[Look],
    out: &mut Vec<Overlay>,
) {
    out.clear();
    if stage == 0 {
        return;
    }
    let (ox, oy) = (cx * CHUNK_PX, cy * CHUNK_PX);
    let cell_i = |x: i32, y: i32| intensity((ox + x).div_euclid(CELL), (oy + y).div_euclid(CELL));
    let ix = |l: Look| Kit::index(looks, l);
    let roll = |s: u32, x: i32, y: i32| h32(seed ^ s, (ox + x) as u32, (oy + y) as u32);
    // Windows: boards, bricks, a pane gone dark, a card.
    for o in &s.openings {
        let i = cell_i(i32::from(o.x), i32::from(o.y));
        let Some(opening) = Opening::of_size(i32::from(o.w), i32::from(o.h)) else { continue };
        if i == 0 {
            continue;
        }
        let d = density(i);
        let h = roll(salt::WINDOW, i32::from(o.x), i32::from(o.y));
        let r = h % 1000;
        let look = if r < d.board {
            // The front's walk: by the window's cell along the front, the upper floor two on.
            let wcx = (ox + i32::from(o.x)).div_euclid(CELL);
            let front = h32(seed ^ salt::FRONT, (oy + i32::from(o.y)).div_euclid(CELL) as u32, 0) % 5;
            let (layout, mirror) = BOARD_WALK[(wcx + 2 * i32::from(o.upper) + front as i32).rem_euclid(5) as usize];
            out.push(Overlay { look: ix(Look::Boards { layout, opening }), x: o.x, y: o.y, mirror, hang: 0 });
            continue;
        } else if r < d.board + d.brick {
            Look::Brick { rough: h >> 12 & 1 == 1, opening }
        } else if o.lit && r < d.board + d.brick + d.dark {
            Look::Crack { variant: (h >> 13 & 1) as u8, opening }
        } else if r < d.board + d.brick + d.dark + d.card {
            Look::Card { opening }
        } else {
            continue;
        };
        out.push(Overlay { look: ix(look), x: o.x, y: o.y, mirror: h >> 14 & 1 == 1, hang: 0 });
    }
    // Wall faces: rust from the sills and the gutters, plaster peeled, the Works' pipes.
    for f in &s.faces {
        let (fx, fy) = (i32::from(f.x), i32::from(f.y));
        let i = cell_i(fx, fy);
        if i == 0 {
            continue;
        }
        let d = density(i);
        let h = roll(salt::FACE, fx, fy);
        let sill = s
            .openings
            .iter()
            .find(|o| (fx..fx + CELL).contains(&i32::from(o.x)) && (fy..fy + CELL).contains(&i32::from(o.y)));
        // Rust runs only from what rusts: a sill's iron, a gutter under the eave.
        let at = match sill {
            Some(o) => Some((
                i32::from(o.x) + 1 + (h >> 16) as i32 % (i32::from(o.w) - 2).max(1),
                i32::from(o.y) + i32::from(o.h) + 2,
            )),
            None if f.eave => Some((fx + 3 + (h >> 16) as i32 % 10, fy + 4)),
            None => None,
        };
        if let Some((x, y)) = at.filter(|_| h % 1000 < d.rust) {
            let variant = (h >> 10 & 3) as u8;
            out.push(Overlay {
                look: ix(Look::Rust { variant }),
                x: x as i16,
                y: y as i16,
                mirror: h >> 9 & 1 == 1,
                hang: 0,
            });
        }
        let h2 = h.rotate_left(11);
        // The Works' ironwork stays out of the hubs (NIGHT.md §4.4: "everywhere outdoors but
        // the town").
        let hub = i + 1 == stage;
        if sill.is_none() && !f.eave && f.plaster && h2 % 1000 < d.peel {
            let variant = (h2 >> 12) as u8 % 3;
            let (x, y) = (fx + 1 + (h2 >> 16) as i32 % 6, fy + 3 + (h2 >> 20) as i32 % 5);
            out.push(Overlay {
                look: ix(Look::Peel { variant }),
                x: x as i16,
                y: y as i16,
                mirror: h2 >> 9 & 1 == 1,
                hang: 0,
            });
        } else if sill.is_none() && !hub && i >= 3 && h2.rotate_left(7) % 1000 < d.pipe {
            let variant = (h2 >> 14) as u8 % 3;
            out.push(Overlay {
                look: ix(Look::Pipe { variant }),
                x: f.x,
                y: f.y + 8,
                mirror: h2 >> 8 & 1 == 1,
                hang: 0,
            });
        }
    }
    // Road cells: chalk, grates, stains.
    for (y, &row) in s.ways.iter().enumerate() {
        if row == 0 {
            continue;
        }
        for x in 0..CHUNK_CELLS {
            if row >> x & 1 == 0 {
                continue;
            }
            let (px, py) = (x * CELL, y as i32 * CELL);
            let i = cell_i(px, py);
            if i == 0 {
                continue;
            }
            let d = density(i);
            let h = roll(salt::WAY, px, py);
            let r = h % 1000;
            let look = if r < d.chalk {
                if i >= 2 && (h >> 10).trailing_zeros() >= 2 {
                    Look::Nine
                } else {
                    Look::Tally { groups: [1, 1, 2, 4, 4][usize::from(i.min(4))] }
                }
            } else if r < d.chalk + d.grate && i + 1 != stage {
                Look::Grate { variant: (h >> 11 & 1) as u8 }
            } else if r < d.chalk + d.grate + d.stain {
                Look::Stain { variant: (h >> 12 & 1) as u8 }
            } else {
                continue;
            };
            let (dx, dy) = ((h >> 16) as i32 % 4, (h >> 20) as i32 % 6);
            out.push(Overlay {
                look: ix(look),
                x: (px + 1 + dx) as i16,
                y: (py + 3 + dy) as i16,
                mirror: false,
                hang: 0,
            });
        }
    }
    // Rails: coats in the hubs (the square's railings), rag strips out in the fields.
    for part in &s.rails {
        let mut x = i32::from(part.x0) + 3;
        while x < i32::from(part.x1) - 2 {
            let gy = i32::from(part.y1) - 1;
            let i = cell_i(x, gy);
            let d = density(i);
            let h = roll(salt::RAIL, x, gy);
            let hub = stage >= 2 && i + 1 == stage;
            let y = gy - rows_up(i32::from(part.hi));
            if hub && h % 1000 < d.coat {
                let look = Look::Coat { cut: (h >> 10 & 3) as u8, cloth: (h >> 12 & 3) as u8 };
                out.push(Overlay { look: ix(look), x: x as i16, y: y as i16, mirror: h >> 9 & 1 == 1, hang: part.hi });
            } else if !hub && i >= 2 && h % 1000 < d.strip {
                let look = Look::Strip { variant: (h >> 10) as u8 % 3 };
                out.push(Overlay { look: ix(look), x: x as i16, y: y as i16, mirror: h >> 9 & 1 == 1, hang: part.hi });
            }
            x += 11 + (h >> 20) as i32 % 3;
        }
    }
}

/// Lays `overlays` into a PC chunk's layers as it is painted (NIGHT.md §4.3): each look's opaque
/// px over what is there, in the kit's own colours (the night's already), its glow put out (a
/// board over a lit window); what lies flat keeps the surface's height and relief, what hangs
/// stands from its rail; a stain darkens what is under it, as the contact shade does.
pub fn composite(chunk: &mut Chunk, overlays: &[Overlay], kit: &Kit) {
    let l = &mut chunk.layers;
    let lit = l.normal.len() == l.albedo.len();
    for o in overlays {
        let Some(c) = kit.canvases.get(usize::from(o.look)) else { continue };
        let (ax, ay) = kit.anchors[usize::from(o.look)];
        let flat = kit::flat(kit.looks[usize::from(o.look)]);
        let (w, h) = (c.w(), c.h());
        for v in 0..h {
            let y = i32::from(o.y) - i32::from(ay) + v;
            if !(0..CHUNK_PX).contains(&y) {
                continue;
            }
            for u in 0..w {
                let su = if o.mirror { w - 1 - u } else { u };
                let ix = c.get(su, v);
                if ix == Ix::CLEAR {
                    continue;
                }
                let x = i32::from(o.x) - i32::from(ax) + u;
                if !(0..CHUNK_PX).contains(&x) {
                    continue;
                }
                let k = (y * CHUNK_PX + x) as usize;
                if ix == Ix::AO {
                    let a = l.albedo[k];
                    let under = [(a >> 16) as u8, (a >> 8) as u8, a as u8];
                    let [r, g, b] = palette::ao(under, palette::ao_cover(c, su, v));
                    l.albedo[k] = 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
                    continue;
                }
                let [r, g, b] = palette::rgb(ix);
                l.albedo[k] = 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
                l.emissive[k] = Ix::CLEAR;
                if !flat {
                    // Hung from its rail: the rail's height at the anchor's row, a px less every
                    // four fifths of a row down.
                    let down = (v - i32::from(ay)).max(0);
                    l.height[k] = (i32::from(o.hang) - jane_art::canvas::height_of_rows(down)).clamp(2, 255) as u8;
                    if lit {
                        let mut n = c.normal_at(su, v);
                        if o.mirror {
                            n[0] = 255 - n[0];
                        }
                        l.normal[k] = n;
                    }
                }
            }
        }
    }
}

/// The overlays a console draws this frame at most, by its preset (NIGHT.md §4.3): Full 96,
/// Balanced 48, Fast 24.
pub fn console_cap(g: crate::gfx_psp::Graphics) -> u16 {
    use crate::gfx_psp::Effect;
    if g.has(Effect::Shafts) && g.has(Effect::Relief) {
        96
    } else if g.has(Effect::Reflections) {
        48
    } else {
        24
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_lamp_breathes_slowly_and_never_gutters() {
        let v: Vec<u8> = (0..600).map(|t| cold_flicker(t, 7)).collect();
        assert!(v.iter().all(|&b| b >= 190));
        // Slow: no step of a tick moves it more than a few.
        assert!(v.windows(2).all(|w| w[0].abs_diff(w[1]) <= 3), "{v:?}");
        assert!(v.iter().max().unwrap() - v.iter().min().unwrap() > 30, "it pulses");
    }

    #[test]
    fn the_soot_thins_the_stars_at_three_and_hides_them_at_four() {
        assert_eq!([0, 1, 2, 3, 4].map(stars_share), [256, 256, 256, 128, 0]);
        assert!(moon_share(4) < 256 && moon_share(4) > 128, "the moon kept, dimmer");
    }

    #[test]
    fn the_densities_grow_with_the_night() {
        for i in 1..4u8 {
            let (a, b) = (density(i), density(i + 1));
            assert!(a.board <= b.board && a.brick <= b.brick && a.rust <= b.rust && a.chalk <= b.chalk);
        }
        let none = density(0);
        assert_eq!(none.board + none.rust + none.chalk + none.coat, 0);
    }

    #[test]
    fn the_board_walk_never_repeats_within_four_cells() {
        for a in 0..5 {
            for d in 1..5 {
                assert_ne!(BOARD_WALK[a], BOARD_WALK[(a + d) % 5]);
            }
        }
    }

    #[test]
    fn console_caps_step_down_by_preset() {
        use crate::gfx_psp::Preset;
        assert_eq!(Preset::ALL.map(|p| console_cap(p.graphics())), [96, 48, 24]);
    }
}
