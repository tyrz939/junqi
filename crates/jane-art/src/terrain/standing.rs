//! Pass 5: what stands on the ground. Low things (long grass, flower beds, stepping stones,
//! crops) are painted into the ground; tall things (trees, shrubs, stones, fences, low walls) go
//! into the strips, one per cell row, with the contact shade under each baked into the ground.
//! Then the wall runs' caster segments.

use jane_core::Tile;
use jane_core::grid::Rect;
use jane_data::{TileGroup, TilePattern as P};

use super::{
    CELL, CHUNK_CELLS, CHUNK_PX, CasterSeg, Chunk, NONE, Painter, Placed, STRIP_BELOW, STRIP_H, STRIP_MARGIN, Standing,
    Style, fast, pack, salt,
};
use crate::canvas::{Canvas, FLAT, Z, normal};
use crate::flora::{Bank, Sprite};
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone, letter};

/// Which bank sprite a cell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Large(usize, usize),
    Medium(usize, usize),
    Pine(usize),
    Dead(usize),
    Bush(usize, usize),
    Berry(usize),
    Rocks(usize),
    Boulder(usize),
}

impl Pick {
    /// The sprite's index in `Bank::all()`'s order.
    fn index(self) -> u16 {
        let i = match self {
            Pick::Large(p, i) => p * 4 + i,
            Pick::Medium(p, i) => 12 + p * 3 + i,
            Pick::Pine(i) => 21 + i,
            Pick::Dead(i) => 24 + i,
            Pick::Bush(p, i) => 26 + p * 4 + i,
            Pick::Berry(i) => 34 + i,
            Pick::Rocks(i) => 36 + i,
            Pick::Boulder(i) => 40 + i,
        };
        i as u16
    }
}

fn sprite(b: &Bank, pick: Pick) -> &Sprite {
    match pick {
        Pick::Large(p, i) => &b.large[p][i],
        Pick::Medium(p, i) => &b.medium[p][i],
        Pick::Pine(i) => &b.pines[i],
        Pick::Dead(i) => &b.dead[i],
        Pick::Bush(p, i) => &b.bushes[p][i],
        Pick::Berry(i) => &b.berry[i],
        Pick::Rocks(i) => &b.rocks[i],
        Pick::Boulder(i) => &b.boulders[i],
    }
}

/// A standing thing: the sprite and how far its foot sits east of the cell's centre and below
/// the cell's bottom, px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Thing {
    pick: Pick,
    ox: i32,
    oy: i32,
    canopy: bool,
}

/// The cell's own style (paint applied).
fn own(p: &Painter, cx: i32, cy: i32) -> Style {
    *p.style_k(Painter::at(cx, cy))
}

fn raw(p: &Painter, cx: i32, cy: i32) -> Tile {
    p.s.raw[Painter::at(cx, cy)]
}

/// Area noise: which wood a cell is in, by blocks of `2^shift` cells.
fn area(wx: i32, wy: i32, shift: u32, seed: u32) -> u32 {
    h32((wx >> shift) as u32, (wy >> shift) as u32, seed ^ salt::STAND ^ 0xa4ea) & 255
}

/// What stands on chunk-local cell `(cx, cy)`, if anything is drawn there. A wood is not a tree
/// per cell: crowns go down on a staggered three-cell lattice, the wood's south edge gets a row
/// of its own so the trunks show, and a lone tree always gets one. Cells with no crown of their
/// own are the shaded floor under their neighbours'.
fn thing(p: &Painter, cx: i32, cy: i32, seed: u32) -> Option<Thing> {
    let st = own(p, cx, cy);
    let (wx, wy) = (cx + p.x0c, cy + p.y0c);
    let h = h32(wx as u32, wy as u32, seed ^ salt::STAND);
    let g = p.s.surf[Painter::at(cx, cy)];
    let ground = if g == NONE { None } else { Some(p.styles.id(g).row.pattern) };
    let t = raw(p, cx, cy);
    match st.row.pattern {
        P::DeadTree => Some(Thing { pick: Pick::Dead((h & 1) as usize), ox: 0, oy: 0, canopy: false }),
        P::Tree | P::Pine => {
            let is_tree = |dx: i32, dy: i32| raw(p, cx + dx, cy + dy) == Tile::Tree;
            let (n, s, w, e) = (is_tree(0, -1), is_tree(0, 1), is_tree(-1, 0), is_tree(1, 0));
            let lone = !n && !s && !w && !e;
            let band = wy.div_euclid(3);
            let (mut anchor, mut ox, mut large) = (false, 0, true);
            if lone {
                anchor = true;
                large = h & 3 != 0;
            } else if !s {
                anchor = wx & 1 == 0 || !w;
                large = h & 1 == 0;
                ox = if h & 2 == 0 {
                    0
                } else if h & 4 == 0 {
                    -2
                } else {
                    2
                };
            } else if wy.rem_euclid(3) == 2 && wx.rem_euclid(3) == 0 {
                anchor = true;
                if band & 1 == 1 && e && is_tree(2, 0) {
                    ox = 24;
                }
                ox += ((h >> 3) & 3) as i32 * 2 - 2;
            }
            if !anchor {
                return None;
            }
            let pick = if st.row.pattern == P::Pine
                || (ground == Some(P::Earth) && h & 3 != 0)
                || (ground == Some(P::Cracked) && h & 1 == 0)
            {
                Pick::Pine((h % 3) as usize)
            } else if ground == Some(P::Marsh) {
                if h & 15 == 3 {
                    Pick::Dead((h & 1) as usize)
                } else if large {
                    Pick::Large(2, (h & 3) as usize)
                } else {
                    Pick::Medium(2, (h % 3) as usize)
                }
            } else {
                let a = area(wx, wy, 5, seed);
                let pal = if a < 60 {
                    1
                } else if a > 230 || h & 15 == 5 {
                    2 - usize::from(h & 15 == 5)
                } else {
                    0
                };
                if large { Pick::Large(pal, ((h >> 1) & 3) as usize) } else { Pick::Medium(pal, (h % 3) as usize) }
            };
            let canopy = !matches!(pick, Pick::Dead(_));
            Some(Thing { pick, ox, oy: 0, canopy })
        }
        P::Bush => {
            let pick = if h & 31 == 7 {
                Pick::Berry((h & 1) as usize)
            } else {
                Pick::Bush(usize::from(area(wx, wy, 4, seed) % 5 == 0), ((h >> 2) & 3) as usize)
            };
            Some(Thing { pick, ox: 0, oy: 2, canopy: false })
        }
        P::Rubble => Some(Thing { pick: Pick::Rocks((h & 3) as usize), ox: 0, oy: 2, canopy: false }),
        P::Cliff if t == Tile::Cliff && p.s.surf[Painter::at(cx, cy)] != NONE => {
            Some(Thing { pick: Pick::Boulder((h % 6) as usize), ox: 0, oy: 2, canopy: false })
        }
        _ => None,
    }
}

/// Low things painted into the ground, the shade under a wood's crowns, and the contact shade
/// under every standing thing (from two cells beyond the chunk, so none is cut at a seam).
pub(super) fn ground(p: &mut Painter, x0: i32, y0: i32, seed: u32) {
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let st = own(p, cx, cy);
            let g = p.s.surf[Painter::at(cx, cy)];
            let (wx, wy) = (x0 + cx, y0 + cy);
            let h = h32(wx as u32, wy as u32, seed ^ salt::STAND ^ 0x10);
            let (px, py) = (cx * CELL, cy * CELL);
            let z = p.s.ly.z(px + 8, py + 8);
            match st.row.pattern {
                P::Flowers => flower_bed(p, px, py, wx, wy, &st, h, z, seed),
                P::Stepping => stepping(p, px, py, &st, h, z),
                P::Crops => crops(p, px, py, wx, wy, g, &st, h, z),
                _ => {}
            }
        }
    }
    // Long grass, from the cells round the chunk too (its blades reach into the next cell): clumps
    // scattered anywhere in the cell, drawn top to bottom so the nearer overlap the further, so a
    // stand of it reads as one meadow and never as a row of tufts to a cell.
    for cy in -1..=CHUNK_CELLS {
        for cx in -1..=CHUNK_CELLS {
            let st = own(p, cx, cy);
            if st.row.pattern != P::Tuft {
                continue;
            }
            let g = p.s.surf[Painter::at(cx, cy)];
            let wet = g != NONE && matches!(p.styles.id(g).row.pattern, P::Marsh | P::Cracked);
            let r = if wet { st.accent.unwrap_or(Ramp::Reed) } else { st.ramp };
            let (wx, wy) = (x0 + cx, y0 + cy);
            let h = h32(wx as u32, wy as u32, seed ^ salt::STAND ^ 0x11);
            let n = 4 + below(h, 3);
            let mut spots = [(0i32, 0i32, 0u32); 6];
            for k in 0..n {
                let hk = h32(h, k, 1);
                spots[k as usize] =
                    (cy * CELL + 3 + below(hk.rotate_right(8), 14) as i32, cx * CELL + below(hk, 16) as i32, hk);
            }
            spots[..n as usize].sort_by_key(|&(y, x, hk)| (y, x, hk));
            for &(y, x, hk) in &spots[..n as usize] {
                let z = p.s.ly.z(x, y);
                tall_tuft(p, x, y, r, 2 + (hk >> 16) as i32 % 3, hk >> 20, z);
            }
        }
    }
    // The shade of a crown on the ground round its trunk, then the contact shade under every
    // standing thing and along every fence and low wall, from cells beyond the chunk whose shade
    // reaches in.
    let world = (x0 * CELL, y0 * CELL);
    for cy in -1..=CHUNK_CELLS {
        for cx in -2..=CHUNK_CELLS {
            let st = own(p, cx, cy);
            if matches!(st.row.pattern, P::Fence | P::StoneWall) {
                let (fx, fy) = (cx * CELL + 8, cy * CELL + CELL - 2);
                contact(p, fx, fy, 10, 3, world, seed);
                continue;
            }
            let Some(t) = thing(p, cx, cy, seed) else { continue };
            let s = sprite(&p.bank, t.pick);
            let wide = s.canvas.w() * 3 / 8;
            let (fx, fy) = (cx * CELL + 8 + t.ox, cy * CELL + CELL - 2 + t.oy / 2);
            if t.canopy {
                contact(p, fx + 3, fy - 2, s.canvas.w() * 9 / 16, 10, world, seed);
            }
            let (rx, ry) = (wide.max(6), (wide / 3).clamp(3, 6));
            contact(p, fx + 2, fy, rx, ry, world, seed);
        }
    }
}

/// Darken an ellipse of ground, soft and cool: two tones in its core, one toward its rim, the
/// rim's edge broken in 2 px clusters by world px, never a checker (ART.md §3.1).
fn contact(p: &mut Painter, cx: i32, cy: i32, rx: i32, ry: i32, (wx0, wy0): (i32, i32), seed: u32) {
    for y in cy - ry..=cy + ry {
        for x in cx - rx..=cx + rx {
            let (dx, dy) = (x - cx, y - cy);
            let d = dx * dx * 256 / (rx * rx).max(1) + dy * dy * 256 / (ry * ry).max(1);
            if d > 256 {
                continue;
            }
            let cluster = fast(((wx0 + x) >> 1) as u32, ((wy0 + y) >> 1) as u32, seed ^ salt::STAND) & 255;
            let s = if d < 80 {
                -2
            } else if d < 190 || (cluster as i32) > (d - 190) * 4 {
                -1
            } else {
                0
            };
            if s != 0 && p.s.ly.z(x, y) < 12 {
                p.s.ly.step(x, y, s);
            }
        }
    }
}

/// Long grass: a clump of blades, none the same length, none centred, lit at the tips.
fn tall_tuft(p: &mut Painter, x: i32, y: i32, r: Ramp, n: i32, h: u32, z: i32) {
    for b in 0..n {
        let hb = h32(h, b as u32, 2);
        let spread = b * 2 - n + 1;
        let len = 3 + (n - spread.abs()) + (hb % 4) as i32;
        let lean = spread.signum() * (1 + (hb >> 2) as i32 % 2) + i32::from(spread == 0) * ((hb >> 4) as i32 % 3 - 1);
        for i in 0..len {
            let (bx, by) = (x + spread / 2 + lean * i * i / (len * len).max(1) * 2, y - i);
            let tone = if i == len - 1 {
                Tone::Light
            } else if i == 0 {
                Tone::Deep
            } else if i < len / 3 {
                Tone::Shade
            } else if i < len * 2 / 3 {
                Tone::Mid
            } else {
                Tone::Base
            };
            // Low in the height layer and near upright: a clump is too fine to throw a shadow of
            // its own or turn from the light, which at a low sun smeared it into dark scribbles.
            p.s.ly.put(bx, by, r.at(tone), normal(lean * 30, -10), z + 1 + i / 6);
        }
    }
    for dx in -2..=2 {
        p.s.ly.step(x + dx, y + 1, -1);
    }
}

/// A flower bed: a low leafy mat, and two or three blooms of one colour to a bed so it reads as
/// planted.
#[allow(clippy::too_many_arguments)]
fn flower_bed(p: &mut Painter, px: i32, py: i32, wx: i32, wy: i32, st: &Style, h: u32, z: i32, seed: u32) {
    let leaf = st.accent.unwrap_or(Ramp::Shrub);
    let colours = ['w', 'y', 'r', 'p', 'o'];
    let bed = area(wx, wy, 2, seed) % 6;
    let bloom = |t: Tone| if bed == 5 { st.ramp.at(t) } else { letter(colours[bed as usize]).unwrap_or(Ix::INK) };
    for y in 3..CELL - 1 {
        for x in 1..CELL - 1 {
            let hh = fast((px + x) as u32, (py + y) as u32, h);
            if hh & 3 != 0 {
                let t = if (x + y) & 3 == 0 {
                    Tone::Light
                } else if hh & 4 == 0 {
                    Tone::Base
                } else {
                    Tone::Mid
                };
                p.s.ly.put(px + x, py + y, leaf.at(t), normal(0, -20), z + 1);
            }
        }
    }
    for k in 0..4 {
        if k == 3 && h & 1 == 0 {
            break;
        }
        let hk = h32(h, k, 3);
        let (fx, fy) = (px + 3 + below(hk, 10) as i32, py + 4 + below(hk.rotate_right(8), 8) as i32);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            p.s.ly.put(fx + dx, fy + dy, bloom(Tone::Light), normal(dx * 50, dy * 50), z + 3);
        }
        p.s.ly.put(fx, fy, letter('y').unwrap_or(Ix::INK), FLAT, z + 4);
        p.s.ly.step(fx + 1, fy + 2, -1);
    }
}

/// A stepping stone: a flat slab lit from the top-left, its shadow under it.
fn stepping(p: &mut Painter, px: i32, py: i32, st: &Style, h: u32, z: i32) {
    let (ox, oy) = ((h & 1) as i32 - ((h >> 1) & 1) as i32, ((h >> 2) & 1) as i32);
    let r = Rect::new(px + 2 + ox, py + 2 + oy, 12, 10);
    let rise = i32::from(st.row.rise).max(2);
    for y in r.y..=r.bottom() {
        for x in r.x..r.right() {
            if y == r.bottom() {
                p.s.ly.step(x + 1, y, -2);
                continue;
            }
            let corner = (x == r.x || x == r.right() - 1) && (y == r.y || y == r.bottom() - 1);
            if corner {
                continue;
            }
            let (t, n) = if y == r.y || x == r.x {
                (Tone::Light, normal(if x == r.x { -60 } else { 0 }, if y == r.y { -60 } else { 0 }))
            } else if y == r.bottom() - 1 || x == r.right() - 1 {
                (Tone::Mid, normal(if x == r.right() - 1 { 60 } else { 0 }, if y == r.bottom() - 1 { 60 } else { 0 }))
            } else {
                (Tone::Base, FLAT)
            };
            p.s.ly.put(x, y, st.ramp.at(t), n, z + rise);
        }
    }
}

/// Crops: along each furrow a line of leaf, planted in rows.
#[allow(clippy::too_many_arguments)]
fn crops(p: &mut Painter, px: i32, py: i32, wx: i32, wy: i32, g: u8, st: &Style, h: u32, z: i32) {
    let _ = (wx, wy);
    for row in 0..2 {
        let ry = py + 2 + row * 8;
        for k in 0..3 {
            let x = px + 1 + k * 5 + ((h >> (k + row * 3)) & 1) as i32;
            for (dx, dy, t) in [
                (0, 3, Tone::Shade),
                (1, 3, Tone::Shade),
                (2, 3, Tone::Shade),
                (0, 2, Tone::Mid),
                (1, 2, Tone::Base),
                (2, 2, Tone::Mid),
                (0, 1, Tone::Base),
                (1, 1, Tone::Light),
                (2, 1, Tone::Base),
                (1, 0, Tone::Lift),
            ] {
                if p.surf_px(x + dx, ry + dy) == g {
                    p.s.ly.put(x + dx, ry + dy, st.ramp.at(t), normal((dx - 1) * 40, (dy - 2) * 30), z + 4 - dy);
                }
            }
            p.s.ly.step(x + 1, ry + 4, -1);
        }
    }
}

/// A post-and-rail fence in `c` (48 x STRIP_H) whose cell has its bottom-left at `(16, 84)`:
/// rails along an east-west run, a rail seen from above down a north-south one, a post in the
/// middle.
fn fence(c: &mut Canvas, r: Ramp, (w, e, n, s): (bool, bool, bool, bool)) {
    let (left, b) = (CELL, STRIP_H - STRIP_BELOW);
    let mid = left + 8;
    if (w || e) || !(n || s) {
        let x0 = if w { left } else { mid - 2 };
        let x1 = if e { left + CELL } else { mid + 2 };
        for ry in [b - 18, b - 10] {
            c.rect_lit(Rect::new(x0, ry, x1 - x0, 3), r, (b - ry) as u8);
        }
    } else {
        let top = if n { b - 32 } else { b - 20 };
        c.rect_lit(Rect::new(mid - 2, top, 4, b - 8 - top), r, 14);
    }
    c.rect_bevel(Rect::new(mid - 3, b - 24, 6, 24), r, 1, Z::new(1, 24));
    c.dot(mid - 2, b - 24, r.at(Tone::High), 24);
    // Every px its true height over the foot (the one projection, PRESENTATION.md §1.7): the
    // post and the rails across the view stand up on the fence's line, where each had stood its
    // relief's one height all the way down (a post 24 px from top to foot lay in the height field
    // as a slab 20 rows deep and threw a chunky block; the rails, lost behind it, none). A rail
    // running away from the viewer is level, seen from above, past the post.
    c.upright(b - 1);
    if !(w || e) && (n || s) {
        let top = if n { b - 32 } else { b - 20 };
        c.level(Rect::new(mid - 2, top, 4, (b - 24) - top), 14);
    }
}

/// A low dry-stone wall in `c`: a capped top that runs on into its neighbours and a face of
/// coursed stone where it drops south.
fn stone_wall(c: &mut Canvas, r: Ramp, (w, e, n, s): (bool, bool, bool, bool), h: u32) {
    let (left, b) = (CELL, STRIP_H - STRIP_BELOW);
    let top = if n { b - CELL - 8 } else { b - 22 };
    let face = if s { b + 4 } else { b - 8 };
    let x0 = if w { left } else { left + 1 };
    let x1 = if e { left + CELL } else { left + CELL - 1 };
    c.rect_lit(Rect::new(x0, top, x1 - x0, face - top), r, 16);
    // Capstones in courses of 5 px.
    for y in (top + 2..face - 1).step_by(5) {
        let off = ((y + (h & 3) as i32) & 3) - 1;
        c.hline(x0, x1 - 1, y, r.at(Tone::Shade), 16);
        c.vline(x0 + 5 + off, (y - 4).max(top + 2), y, r.at(Tone::Shade), 16);
        c.vline(x0 + 11 - off, y + 1, y + 4, r.at(Tone::Shade), 16);
    }
    if !s {
        c.rect_lit(Rect::new(x0, face, x1 - x0, b - face), r, 8);
        c.gradient(Rect::new(x0, face, x1 - x0, b - face), r, crate::canvas::Dir::Down, Tone::Mid, Tone::Shade, true);
        for (k, x) in [x0 + 3 + (h & 1) as i32, x0 + 9, x0 + 13].iter().enumerate() {
            c.vline(*x, face + 1 + (k as i32 & 1) * 2, face + 3 + (k as i32 & 1) * 2, r.at(Tone::Deep), 6);
        }
        c.hline(x0, x1 - 1, face + 4, r.at(Tone::Shade), 4);
    }
}

/// What a strip pixel is, for the renderer's canopy ghost, dappled light and god rays: nothing,
/// a solid standing thing (a trunk, a stone, a fence), or canopy (a crown's leaves).
pub mod mask {
    /// Nothing is drawn.
    pub const CLEAR: u8 = 0;
    /// A trunk, a shrub, a stone, a fence, a low wall.
    pub const SOLID: u8 = 1;
    /// A tree's crown.
    pub const CANOPY: u8 = 2;
}

/// The rows' strips: every standing thing of a row stamped into one strip canvas, cropped to
/// what is drawn and resolved, with its mask.
pub(super) fn strips(p: &mut Painter, x0: i32, y0: i32, seed: u32, out: &mut Chunk) {
    let sw = p.s.row.w();
    let placed = p.standing == Standing::Placed;
    // Placed, a fence in the row above the chunk reaches 4 px into it and one in the two rows
    // below it stands up into it: those rows are laid too, for what falls inside.
    let rows = if placed { -1..CHUNK_CELLS + 2 } else { 0..CHUNK_CELLS };
    for row in rows {
        let inner = (0..CHUNK_CELLS).contains(&row);
        let mut any = false;
        let mut canopy = false;
        let mut built = false;
        // The fences and walls of the cells either side of the chunk are laid first, only so the
        // outline sees a run go on past the seam; they are cleared again after it.
        for cx in [-1, CHUNK_CELLS].into_iter().chain(0..CHUNK_CELLS) {
            let st = own(p, cx, row);
            let foot = STRIP_H - STRIP_BELOW;
            let margin = !(0..CHUNK_CELLS).contains(&cx);
            if margin && !matches!(st.row.pattern, P::Fence | P::StoneWall) {
                continue;
            }
            if matches!(st.row.pattern, P::Fence | P::StoneWall) {
                let same = |dx: i32, dy: i32| raw(p, cx + dx, row + dy) == st.tile;
                let nb = (same(-1, 0), same(1, 0), same(0, -1), same(0, 1));
                let h = h32((x0 + cx) as u32, (y0 + row) as u32, seed ^ salt::STAND);
                p.s.thing.clear();
                if st.row.pattern == P::Fence {
                    fence(&mut p.s.thing, st.ramp, nb);
                } else {
                    stone_wall(&mut p.s.thing, st.ramp, nb, h);
                }
                if !any {
                    reset_row(p);
                    any = true;
                }
                let (sx, sy) = (STRIP_MARGIN + cx * CELL - CELL, 0);
                built = true;
                let thing = std::mem::replace(&mut p.s.thing, Canvas::new(0, 0));
                p.s.row.stamp(&thing, sx, sy);
                grow_bb(&mut p.s.row_bb, sx, sy, thing.w(), thing.h());
                mark(&mut p.s.rowmask, sw, &thing, sx, sy, None);
                if margin {
                    retag(&mut p.s.rowmask, sw, &thing, sx, sy);
                }
                p.s.thing = thing;
                continue;
            }
            if !inner {
                continue;
            }
            let Some(t) = thing(p, cx, row, seed) else { continue };
            out.placed.push(Placed {
                sprite: t.pick.index(),
                x: (cx * CELL + 8 + t.ox) as i16,
                y: ((row + 1) * CELL - 2 + t.oy) as i16,
                row: row as u8,
            });
            if placed {
                continue;
            }
            if !any {
                reset_row(p);
                any = true;
            }
            canopy |= t.canopy;
            let s = sprite(&p.bank, t.pick);
            let (sx, sy) = (STRIP_MARGIN + cx * CELL + 8 + t.ox - s.ax, foot - 2 + t.oy - s.ay);
            p.s.row.stamp(&s.canvas, sx, sy);
            let (cw, ch) = (s.canvas.w(), s.canvas.h());
            grow_bb(&mut p.s.row_bb, sx, sy, cw, ch);
            let trunk = if t.canopy { Some(p.styles.tile(Tile::Tree).accent.unwrap_or(Ramp::Bark)) } else { None };
            mark(&mut p.s.rowmask, sw, &s.canvas, sx, sy, trunk);
        }
        if any {
            if built {
                // Fences and low walls are outlined once the row is laid, so a run of them is one
                // piece and no join shows where one cell meets the next.
                p.s.row.outline();
                let bb = p.s.row_bb;
                for y in bb.y.max(0)..bb.bottom().min(STRIP_H) {
                    for x in bb.x.max(0)..bb.right().min(sw) {
                        let i = (y * sw + x) as usize;
                        if p.s.rowmask[i] == BORROWED {
                            p.s.row.clear_px(x, y);
                            p.s.rowmask[i] = mask::CLEAR;
                        }
                    }
                }
            }
            if placed {
                bake(p, row);
            } else {
                crop(p, row, canopy, out);
            }
        }
    }
}

/// Paint the strip canvas's drawn px into the ground layers, where they fall in the chunk: a
/// fence or a low wall laid flat with the walls, its heights kept for the light.
fn bake(p: &mut Painter, row: i32) {
    let bb = p.s.row_bb;
    let top = (row + 1) * CELL + STRIP_BELOW - STRIP_H;
    for y in bb.y.max(0)..bb.bottom().min(STRIP_H) {
        let ly = top + y;
        if !(0..CHUNK_PX).contains(&ly) {
            continue;
        }
        for x in bb.x.max(STRIP_MARGIN)..bb.right().min(STRIP_MARGIN + CHUNK_PX) {
            let ix = p.s.row.get(x, y);
            if ix.is_opaque() {
                let (n, z) = (p.s.row.normal_at(x, y), p.s.row.height_at(x, y));
                p.s.ly.put(x - STRIP_MARGIN, ly, ix, n, i32::from(z));
            }
        }
    }
}

/// A mask value for a strip's scratch only: a px laid by a cell beyond the chunk, for the outline.
const BORROWED: u8 = 255;

/// Mark a stamped thing's drawn pixels as borrowed from beyond the chunk.
fn retag(m: &mut [u8], sw: i32, c: &Canvas, sx: i32, sy: i32) {
    for y in 0..c.h() {
        for x in 0..c.w() {
            let (tx, ty) = (sx + x, sy + y);
            if c.get(x, y).is_opaque() && tx >= 0 && ty >= 0 && tx < sw && ty < STRIP_H {
                m[(ty * sw + tx) as usize] = BORROWED;
            }
        }
    }
}

/// Mark the mask under a stamped thing: its drawn pixels solid, or, for a tree (`trunk` names its
/// bark), canopy everywhere but the bark.
fn mark(m: &mut [u8], sw: i32, c: &Canvas, sx: i32, sy: i32, trunk: Option<Ramp>) {
    for y in 0..c.h() {
        for x in 0..c.w() {
            let ix = c.get(x, y);
            if !ix.is_opaque() {
                continue;
            }
            let (tx, ty) = (sx + x, sy + y);
            if tx < 0 || ty < 0 || tx >= sw || ty >= STRIP_H {
                continue;
            }
            let bark = trunk.is_some_and(|b| Ramp::of(ix).is_some_and(|(r, _)| r == b));
            m[(ty * sw + tx) as usize] = if trunk.is_some() && !bark { mask::CANOPY } else { mask::SOLID };
        }
    }
}

/// Clear the strip canvas and its mask where the last row drew, and start a new drawn box.
fn reset_row(p: &mut Painter) {
    let bb = p.s.row_bb;
    let sw = p.s.row.w();
    p.s.row.clear_rect(bb);
    for y in bb.y.max(0)..bb.bottom().min(STRIP_H) {
        let (a, b) = (bb.x.max(0), bb.right().min(sw).max(bb.x.max(0)));
        p.s.rowmask[(y * sw + a) as usize..(y * sw + b) as usize].fill(mask::CLEAR);
    }
    p.s.row_bb = Rect::new(0, 0, 0, 0);
}

/// Grow the drawn box by a stamp at `(x, y)` of `w x h`.
fn grow_bb(bb: &mut Rect, x: i32, y: i32, w: i32, h: i32) {
    if bb.w == 0 {
        *bb = Rect::new(x, y, w, h);
        return;
    }
    let (x0, y0) = (bb.x.min(x), bb.y.min(y));
    let (x1, y1) = (bb.right().max(x + w), bb.bottom().max(y + h));
    *bb = Rect::new(x0, y0, x1 - x0, y1 - y0);
}

/// Crop the strip canvas to what is drawn and write it out, resolved.
fn crop(p: &Painter, row: i32, canopy: bool, out: &mut Chunk) {
    let c = &p.s.row;
    let bb = p.s.row_bb;
    let (mut x0, mut y0, mut x1, mut y1) = (c.w(), c.h(), -1, -1);
    for y in bb.y.max(0)..bb.bottom().min(c.h()) {
        for x in bb.x.max(0)..bb.right().min(c.w()) {
            if c.get(x, y).is_opaque() {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < 0 {
        return;
    }
    let s = out.next_strip();
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    s.row = row as u8;
    s.x = (x0 - STRIP_MARGIN) as i16;
    s.y = ((row + 1) * CELL + STRIP_BELOW - STRIP_H + y0) as i16;
    s.w = w as u16;
    s.h = h as u16;
    s.canopy = canopy;
    s.albedo.clear();
    s.normal.clear();
    s.height.clear();
    s.mask.clear();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let ix = c.get(x, y);
            if ix.is_opaque() {
                s.albedo.push(pack(ix));
                s.normal.push(c.normal_at(x, y));
                s.height.push(c.height_at(x, y).max(1));
                s.mask.push(p.s.rowmask[(y * c.w() + x) as usize]);
            } else {
                s.albedo.push(0);
                s.normal.push(FLAT);
                s.height.push(0);
                s.mask.push(mask::CLEAR);
            }
        }
    }
}

/// The casters (PRESENTATION.md §1.7): every edge between a cell that casts and one that does
/// not, merged along a row or a column while the height holds (walls, roofs, hedges, cliffs, low
/// stone walls); a fence's rails as a segment along its run; and a square round the trunk of
/// every tree that shows one, at the crown's height.
pub(super) fn casters(p: &Painter, x0: i32, y0: i32, out: &mut Chunk) {
    out.casters.clear();
    let casts = |cx: i32, cy: i32| -> u8 {
        let st = own(p, cx, cy);
        let raised_wall =
            st.raised() && matches!(st.row.group, TileGroup::Wall | TileGroup::Roof) && st.tile != Tile::Void;
        let low_wall = st.row.pattern == P::StoneWall;
        if raised_wall || low_wall { st.row.rise } else { 0 }
    };
    let px = |c: i32| c * CELL;
    for a in 0..CHUNK_CELLS {
        for (d, edge) in [(-1, 0), (1, CELL)] {
            for across in [true, false] {
                let mut run: Option<(i32, u8)> = None;
                for b in 0..=CHUNK_CELLS {
                    let (cx, cy, nx, ny) = if across { (b, a, b, a + d) } else { (a, b, a + d, b) };
                    let cur = (b < CHUNK_CELLS).then(|| casts(cx, cy)).filter(|&h| h > 0 && casts(nx, ny) == 0);
                    if let Some((start, rh)) = run {
                        if cur != Some(rh) {
                            let seg = if across {
                                let y = px(y0 + a) + edge;
                                CasterSeg { a: (px(x0 + start), y), b: (px(x0 + b), y), height: rh }
                            } else {
                                let x = px(x0 + a) + edge;
                                CasterSeg { a: (x, px(y0 + start)), b: (x, px(y0 + b)), height: rh }
                            };
                            out.casters.push(seg);
                            run = None;
                        }
                    }
                    if run.is_none() {
                        run = cur.map(|h| (b, h));
                    }
                }
            }
        }
    }
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let st = own(p, cx, cy);
            let (wx, wy) = (px(x0 + cx), px(y0 + cy));
            if st.row.pattern == P::Fence {
                let same = |dx: i32, dy: i32| raw(p, cx + dx, cy + dy) == st.tile;
                let (w, e, n, s) = (same(-1, 0), same(1, 0), same(0, -1), same(0, 1));
                let (mx, my) = (wx + 8, wy + CELL - 4);
                if w || e || !(n || s) {
                    let (a, b) = ((if w { wx } else { mx - 2 }, my), (if e { wx + CELL } else { mx + 2 }, my));
                    out.casters.push(CasterSeg { a, b, height: st.row.rise });
                }
                if n || s {
                    let (a, b) = ((mx, if n { wy } else { my - 2 }), (mx, if s { wy + CELL } else { my }));
                    out.casters.push(CasterSeg { a, b, height: st.row.rise });
                }
                continue;
            }
            let Some(t) = thing(p, cx, cy, p.seed) else { continue };
            if !matches!(t.pick, Pick::Large(..) | Pick::Medium(..) | Pick::Pine(_) | Pick::Dead(_)) {
                continue;
            }
            let (ax, ay) = (wx + 4 + t.ox, wy + 8);
            let (bx, by) = (ax + 8, ay + 6);
            let hgt = st.row.rise;
            for (a, b) in [((ax, ay), (bx, ay)), ((bx, ay), (bx, by)), ((bx, by), (ax, by)), ((ax, by), (ax, ay))] {
                out.casters.push(CasterSeg { a, b, height: hgt });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flora::Ramps;

    #[test]
    fn a_placement_names_the_sprite_it_stamps() {
        let b = Bank::new(Ramps::default());
        let all = b.all();
        let mut picks = Vec::new();
        for p in 0..3 {
            picks.extend((0..4).map(|i| Pick::Large(p, i)));
            picks.extend((0..3).map(|i| Pick::Medium(p, i)));
        }
        picks.extend((0..3).map(Pick::Pine));
        picks.extend((0..2).map(Pick::Dead));
        for p in 0..2 {
            picks.extend((0..4).map(|i| Pick::Bush(p, i)));
        }
        picks.extend((0..2).map(Pick::Berry));
        picks.extend((0..4).map(Pick::Rocks));
        picks.extend((0..6).map(Pick::Boulder));
        for pick in picks {
            assert!(std::ptr::eq(sprite(&b, pick), all[usize::from(pick.index())].1), "{pick:?}");
        }
    }
}
