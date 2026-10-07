//! Ground at a story's place, for the rows set down there (`openAt`, `placeCells`, `nearOnFoot`,
//! `spotFor` in `jane/src/world/placements.ts`). A place a story claimed was built and claimed
//! whole, so the kit's own "open ground" says no everywhere in it: these look at the ground and at
//! the things standing on it instead. None of them throws dice, so a tale's dressing moves nothing
//! else in the county.
//!
//! The stories stage (PORT.md §6.m stage 8) calls these from its rows at a place and from its
//! check that a tale fits a place before taking it (`taleRoom`), with the place's box and the cell
//! of its board. They are here, beside the placements, because they are the placements' own rules.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::hash::Fnv;
use jane_core::search::{Conn, Reach, flood};
use jane_core::{NameId, PropDefId, Rect, Tile};
use jane_data::PlacementDef;

use crate::kit::Kit;

/// How far round a tale's place its things may be walked to from its front, in cells beyond its
/// footprint.
pub const ON_FOOT: i32 = 40;
/// Cells round a footprint [`open_at`] looks at to see whether a solid thing would shut a way.
const SEAL: i32 = 6;
/// Beyond `within`, how far [`open_at`] looks for things already standing.
const LOOK: i32 = 16;

/// Growth and rubble a tale may clear from under what it sets down. Water, walls and fences stay.
pub fn soft_ground(t: Tile) -> bool {
    matches!(t, Tile::Bush | Tile::GrassTall | Tile::Tree | Tile::DeadTree | Tile::Rubble)
}

/// A prop standing somewhere, as the ground rules see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stood {
    pub def: PropDefId,
    pub x: i32,
    pub y: i32,
    pub hidden: bool,
}

impl Stood {
    fn size(self) -> (i32, i32) {
        let row = jane_data::catalog().story.prop(self.def);
        (i32::from(row.w), i32::from(row.h))
    }

    /// Its footprint, if it stops her feet and is not hidden.
    fn blocks(self) -> Option<Rect> {
        let row = jane_data::catalog().story.prop(self.def);
        (row.solid && !self.hidden).then(|| Rect::new(self.x, self.y, i32::from(row.w), i32::from(row.h)))
    }
}

/// What stands on the ground near a place: the county's own things, or those plus stand-ins for a
/// tale's things tried there for a moment and taken away again (`taleRoom`'s pool).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    pub props: Vec<Stood>,
    pub units: Vec<(i32, i32)>,
    pub marks: Vec<(i32, i32)>,
}

impl Standing {
    /// Every prop, unit and mark of the kit whose cell lies in `window`.
    pub fn near(k: &Kit, window: Rect) -> Standing {
        let bp = k.blueprint();
        let at = |c: jane_core::Cell| (i32::from(c.x), i32::from(c.y));
        let inside = |(x, y): (i32, i32)| window.contains(x, y);
        Standing {
            props: bp
                .props
                .iter()
                .map(|p| Stood { def: p.def, x: i32::from(p.cell.x), y: i32::from(p.cell.y), hidden: p.hidden })
                .filter(|s| inside((s.x, s.y)))
                .collect(),
            units: bp.units.iter().map(|u| at(u.cell)).filter(|&c| inside(c)).collect(),
            marks: bp.marks.values().map(|m| at(m.cell)).filter(|&c| inside(c)).collect(),
        }
    }

    /// Only what lies in `window`.
    fn within(&self, window: Rect) -> Standing {
        let inside = |&(x, y): &(i32, i32)| window.contains(x, y);
        Standing {
            props: self.props.iter().copied().filter(|s| window.contains(s.x, s.y)).collect(),
            units: self.units.iter().copied().filter(inside).collect(),
            marks: self.marks.iter().copied().filter(inside).collect(),
        }
    }
}

/// Ground a short walk from a story's place ([`near_on_foot`]).
#[derive(Clone, Debug)]
pub struct OnFoot {
    window: Rect,
    reach: Reach,
}

impl OnFoot {
    pub fn has(&self, x: i32, y: i32) -> bool {
        self.window.contains(x, y) && self.reach.reached(x - self.window.x, y - self.window.y)
    }
}

/// Ground a short walk from a story's place: from the cell in front of its board (or the middle of
/// its south edge), over open ground and round anything solid standing on it, inside a window
/// [`ON_FOOT`] cells beyond its footprint, and only ground she can reach at all (`ground`). A tale's
/// things go down only here: "the stone north of the house" is a stone she can walk to from the
/// house, not one across a hedge whose gap is half a mile off.
pub fn near_on_foot(k: &Kit, bounds: Rect, board: Option<(i32, i32)>, ground: impl Fn(i32, i32) -> bool) -> OnFoot {
    let (x0, y0) = ((bounds.x - ON_FOOT).max(0), (bounds.y - ON_FOOT).max(0));
    let x1 = (bounds.x + bounds.w + ON_FOOT).min(k.w());
    let y1 = (bounds.y + bounds.h + ON_FOOT).min(k.h());
    let window = Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0));
    let (ww, wh) = (window.w, window.h);
    let mut open = vec![false; (ww * wh) as usize];
    for j in 0..wh {
        for i in 0..ww {
            open[(j * ww + i) as usize] = ground(x0 + i, y0 + j) && !k.solid(x0 + i, y0 + j);
        }
    }
    for s in &Standing::near(k, Rect::new(x0 - LOOK, y0 - LOOK, ww + 2 * LOOK, wh + 2 * LOOK)).props {
        if let Some(r) = s.blocks() {
            shut(&mut open, window, r);
        }
    }
    let from = board.map_or((bounds.x + (bounds.w >> 1), bounds.y + bounds.h), |(x, y)| (x, y + 1));
    // From the front, or from the nearest open cell to it if the front itself is taken.
    let is_open = |x: i32, y: i32| window.contains(x, y) && open[((y - y0) * ww + (x - x0)) as usize];
    let start = (0..=3).find_map(|r| {
        (-r..=r).find_map(|oy| (-r..=r).map(|ox| (from.0 + ox, from.1 + oy)).find(|&(x, y)| is_open(x, y)))
    });
    let mut reach = Reach::new();
    let starts: Vec<(i32, i32)> = start.map(|(x, y)| (x - x0, y - y0)).into_iter().collect();
    flood(ww as u32, wh as u32, &starts, Conn::Four, u32::MAX, |i, j| open[(j * ww + i) as usize], &mut reach);
    OnFoot { window, reach }
}

/// Mark the cells of `r` inside `window` shut.
fn shut(open: &mut [bool], window: Rect, r: Rect) {
    set_in(open, window, r, false);
}

/// Set the cells of `r` inside `window` to `v`, in a grid of the window's cells.
fn set_in(cells: &mut [bool], window: Rect, r: Rect, v: bool) {
    let Some(r) = r.intersect(window) else { return };
    for j in r.y..r.y + r.h {
        for i in r.x..r.x + r.w {
            cells[((j - window.y) * window.w + (i - window.x)) as usize] = v;
        }
    }
}

/// The nearest `w x h` footprint to `(x, y)`, in square rings out to `within`, whose ground is
/// walkable or only growth ([`soft_ground`]; never water or a road), on which nothing already
/// stands, nor hard against it (two things touching are one thing to the eye), not on a mark nor
/// beside one (that is where somebody stands), which `reach` (if given) touches, and, if `blocks`,
/// which would not shut a way through ([`seals`]). Unless `dry`, the growth under it is cleared to
/// dirt. `pool`: what stands there instead of the kit's own things (a tale tried for fit).
#[allow(clippy::too_many_arguments)]
pub fn open_at(
    k: &mut Kit,
    reach: Option<&OnFoot>,
    (x, y): (i32, i32),
    (w, h): (i32, i32),
    within: i32,
    blocks: bool,
    dry: bool,
    pool: Option<&Standing>,
) -> Option<(i32, i32)> {
    let r = within + LOOK;
    let window = Rect::new(x - r, y - r, 2 * r + 1, 2 * r + 1);
    let near = pool.map_or_else(|| Standing::near(k, window), |p| p.within(window));
    let free = |k: &Kit, cx: i32, cy: i32| {
        for j in cy..cy + h {
            for i in cx..cx + w {
                if !k.inside(i, j) {
                    return false;
                }
                let t = k.get(i, j);
                if (k.solid(i, j) && !soft_ground(t)) || matches!(t, Tile::Water | Tile::Road) {
                    return false;
                }
            }
        }
        for q in &near.props {
            let (sw, sh) = q.size();
            let hit = if q.hidden {
                q.x < cx + w && q.x + sw > cx && q.y < cy + h && q.y + sh > cy
            } else {
                q.x <= cx + w && q.x + sw >= cx && q.y <= cy + h && q.y + sh >= cy
            };
            if hit {
                return false;
            }
        }
        if near.units.iter().any(|&(ux, uy)| Rect::new(cx, cy, w, h).contains(ux, uy)) {
            return false;
        }
        if near.marks.iter().any(|&(mx, my)| mx >= cx - 1 && mx <= cx + w && my >= cy - 1 && my <= cy + h) {
            return false;
        }
        if blocks && seals(k, &near, (cx, cy), (w, h)) {
            return false;
        }
        let Some(reach) = reach else { return true };
        // Somewhere she can walk to from the place touches it: the ground under it, or beside it.
        (cy - 1..=cy + h).any(|j| {
            (cx - 1..=cx + w).any(|i| {
                let corner = (i == cx - 1 || i == cx + w) && (j == cy - 1 || j == cy + h);
                !corner && reach.has(i, j)
            })
        })
    };
    for r in 0..=within {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox.abs().max(oy.abs()) != r || !free(k, x + ox, y + oy) {
                    continue;
                }
                if !dry {
                    for j in y + oy..y + oy + h {
                        for i in x + ox..x + ox + w {
                            if soft_ground(k.get(i, j)) {
                                k.set(i, j, Tile::Dirt);
                            }
                        }
                    }
                }
                return Some((x + ox, y + oy));
            }
        }
    }
    None
}

/// Would a solid thing on this footprint shut a way through? The open ground round it, in a window
/// [`SEAL`] cells wider each side, must still join up without it: the open cells beside it (not its
/// corners) and the marks nearby, all one piece of ground. A chest set in the one gap to a board, a
/// person stood in a doorway, a wall built across a path are all turned down.
fn seals(k: &Kit, near: &Standing, (cx, cy): (i32, i32), (w, h): (i32, i32)) -> bool {
    let window = Rect::new(cx - SEAL, cy - SEAL, w + 2 * SEAL, h + 2 * SEAL);
    let (ww, wh) = (window.w, window.h);
    let foot = Rect::new(cx, cy, w, h);
    let mut open = vec![false; (ww * wh) as usize];
    for j in 0..wh {
        for i in 0..ww {
            let (gx, gy) = (window.x + i, window.y + j);
            open[(j * ww + i) as usize] = !foot.contains(gx, gy) && k.inside(gx, gy) && !k.solid(gx, gy);
        }
    }
    for s in &near.props {
        if let Some(r) = s.blocks() {
            shut(&mut open, window, r);
        }
    }
    for &(ux, uy) in &near.units {
        shut(&mut open, window, Rect::new(ux, uy, 1, 1));
    }
    let ix = |x: i32, y: i32| ((y - window.y) * ww + (x - window.x)) as usize;
    let mut must = Vec::new();
    for j in cy - 1..=cy + h {
        for i in cx - 1..=cx + w {
            let edge = (i == cx - 1 || i == cx + w) != (j == cy - 1 || j == cy + h);
            if edge && open[ix(i, j)] {
                must.push((i, j));
            }
        }
    }
    for &(mx, my) in &near.marks {
        if window.contains(mx, my) && open[ix(mx, my)] {
            must.push((mx, my));
        }
    }
    if must.len() < 2 {
        return false;
    }
    let mut reach = Reach::new();
    let from = [(must[0].0 - window.x, must[0].1 - window.y)];
    flood(ww as u32, wh as u32, &from, Conn::Four, u32::MAX, |i, j| open[(j * ww + i) as usize], &mut reach);
    must.iter().any(|&(x, y)| !reach.reached(x - window.x, y - window.y))
}

/// Open cells about a slot at a story's place, nearest the slot first, for a row with several of a
/// thing (the stones in a garden): floor nothing stands on and nobody stands on, with open floor on
/// both sides along one axis (so a pushed thing can go one way and she can stand on the other),
/// never two side by side. Ties in distance go by a hash of the row's key and the cell. At most `n`.
pub fn place_cells(k: &Kit, from: (i32, i32), within: i32, n: usize, key: NameId) -> Vec<(i32, i32)> {
    let span = within * 2 + 3;
    let window = Rect::new(from.0 - within - 1, from.1 - within - 1, span, span);
    let mut busy = vec![false; (span * span) as usize];
    let near = Standing::near(k, window.grow(LOOK));
    for s in &near.props {
        let (sw, sh) = s.size();
        set_in(&mut busy, window, Rect::new(s.x, s.y, sw, sh), true);
    }
    for &(ux, uy) in &near.units {
        set_in(&mut busy, window, Rect::new(ux, uy, 1, 1), true);
    }
    let open = |x: i32, y: i32| {
        window.contains(x, y) && !busy[((y - window.y) * span + (x - window.x)) as usize] && !k.solid(x, y)
    };
    let name = jane_data::catalog().name(key);
    let mut cells = Vec::new();
    for y in from.1 - within..=from.1 + within {
        for x in from.0 - within..=from.0 + within {
            if open(x, y) && ((open(x - 1, y) && open(x + 1, y)) || (open(x, y - 1) && open(x, y + 1))) {
                let d = (x - from.0).abs().max((y - from.1).abs());
                cells.push((d, Fnv::new().str(name).i32(x).i32(y).mix(), y, x));
            }
        }
    }
    cells.sort();
    let mut out: Vec<(i32, i32)> = Vec::new();
    for (_, _, y, x) in cells {
        if out.iter().any(|&(ox, oy)| (ox - x).abs() <= 1 && (oy - y).abs() <= 1) {
            continue;
        }
        out.push((x, y));
        if out.len() == n {
            break;
        }
    }
    out
}

/// Where a tale's row set by position (`dx`, `dy` from the place's top-left) goes: the cell for its
/// thing, or `None` if there is no room. With `room`, open ground for the whole of a small scene
/// (the box, the stone and somewhere to stand to push it) with this row's thing at `(ox, oy)` in it.
/// Only what stops her feet (a person, a solid thing, a scene) has to leave the ground round it
/// joined up. `dry`: only look, clear nothing.
pub fn spot_for(
    k: &mut Kit,
    reach: Option<&OnFoot>,
    bounds: Rect,
    row: &PlacementDef,
    dry: bool,
    pool: Option<&Standing>,
) -> Option<(i32, i32)> {
    let cat = jane_data::catalog();
    let size = row.prop.map_or((1, 1), |t| {
        let p = cat.story.prop(t.def);
        (i32::from(p.w), i32::from(p.h))
    });
    let [rw, rh, ox, oy] = row.room.map_or([size.0, size.1, 0, 0], |r| r.map(i32::from));
    let blocks = row.unit.is_some() || row.room.is_some() || row.prop.is_some_and(|t| cat.story.prop(t.def).solid);
    let at = (bounds.x + row.dx.map_or(0, i32::from), bounds.y + row.dy.map_or(0, i32::from));
    let within = row.within.map_or(3, i32::from);
    open_at(k, reach, at, (rw, rh), within, blocks, dry, pool).map(|(x, y)| (x + ox, y + oy))
}

#[cfg(test)]
mod tests {
    use jane_core::{Key, ZoneId};

    use super::*;

    fn kit() -> Kit {
        Kit::new(ZoneId::County, 40, 30, 7, 0, Tile::Grass, true)
    }

    #[test]
    fn open_at_steps_round_what_stands_and_clears_growth() {
        let cat = jane_data::catalog();
        let crate_ = cat.story.prop_id("crate").expect("a crate row");
        let mut k = kit();
        k.prop(None, crate_, 10, 10);
        k.set(14, 10, Tile::Bush);
        let at = open_at(&mut k, None, (10, 10), (1, 1), 6, false, false, None).expect("room");
        let (cw, ch) = (i32::from(cat.story.prop(crate_).w), i32::from(cat.story.prop(crate_).h));
        assert!(at.0 > 10 + cw || at.1 > 10 + ch || at.0 < 9 || at.1 < 9, "{at:?} touches the crate");
        k.set(20, 20, Tile::Bush);
        assert_eq!(open_at(&mut k, None, (20, 20), (1, 1), 0, false, false, None), Some((20, 20)));
        assert_eq!(k.get(20, 20), Tile::Dirt, "growth under it is cleared");
        k.set(25, 25, Tile::Water);
        assert_eq!(open_at(&mut k, None, (25, 25), (1, 1), 0, false, true, None), None);
    }

    #[test]
    fn a_solid_thing_never_shuts_the_only_gap() {
        let mut k = kit();
        // A wall across the kit with one gap at x 20.
        for x in 0..40 {
            if x != 20 {
                k.set(x, 15, Tile::HouseWall);
            }
        }
        let got = open_at(&mut k, None, (20, 15), (1, 1), 2, true, true, None).expect("room beside the gap");
        assert_ne!(got, (20, 15), "not in the gap");
        assert_eq!(open_at(&mut k, None, (20, 15), (1, 1), 0, false, true, None), Some((20, 15)));
    }

    #[test]
    fn place_cells_are_apart_and_nearest_first() {
        let k = kit();
        let cells = place_cells(&k, (20, 15), 3, 4, jane_data::catalog().name_id("start").expect("start"));
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], (20, 15));
        for (i, a) in cells.iter().enumerate() {
            for b in &cells[i + 1..] {
                assert!((a.0 - b.0).abs() > 1 || (a.1 - b.1).abs() > 1, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn on_foot_stops_at_a_wall() {
        let mut k = kit();
        for y in 0..30 {
            k.set(20, y, Tile::HouseWall);
        }
        k.mark(Key::Local(0), 1, 1, None);
        let foot = near_on_foot(&k, Rect::new(5, 5, 6, 6), None, |_, _| true);
        assert!(foot.has(8, 11));
        assert!(foot.has(19, 20));
        assert!(!foot.has(21, 20));
    }
}
