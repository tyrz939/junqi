//! Height read off the zone (MAP.md §2, §6.1): which cells of a chunk are a plateau's face, its
//! rims, a join through a face (a stair's, a ladder's, a ramp's step) or a crag on flat ground,
//! and the level each cell's heights stand on. The painters (`hard::height`) draw them; the chunk's
//! heights are made absolute by it ([`LEVEL_PX`] a level, `Painter::finish`), so every tier casts
//! a plateau as the slab it is and a face from its foot.
//!
//! A cell's **base** is the level its heights count from: its own for ground, a rim and a north
//! flight; its foot's for a face and a join through one, which climb from the foot to the top.
//! Nothing here runs in a zone without levels: every zone built so far paints as it did.

use jane_core::Tile;
use jane_core::tile::F_SOLID;

use super::{GM, M, Painter, TileSource};

/// True px a level stands over the one under it (MAP.md §2.1: drawn `rows_up(40) = 32` rows, two
/// cells of face).
pub const LEVEL_PX: i32 = 40;

/// What a cell is to the height painters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Ground, or anything height does not touch.
    #[default]
    Plain,
    /// Part of a plateau's south face: `k` cells over its foot, of `n`.
    Face,
    /// A plateau's edge seen edge-on or from behind: lower ground beside it (`side`).
    Rim,
    /// A step through a face (a stair, a ladder, a ramp's road): `k` cells over its foot, of `n`.
    Join,
    /// A stair down a north rim: its flight falls away north inside the cell.
    NorthFlight,
    /// A cliff with nothing lower round it: a crag on flat ground, the old painter's.
    Crag,
}

/// Lower ground to the west.
pub(crate) const WEST: u8 = 1;
/// Lower ground to the east.
pub(crate) const EAST: u8 = 2;
/// Lower ground to the north.
pub(crate) const NORTH: u8 = 4;

/// A cell as the height painters read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HCell {
    pub kind: Kind,
    /// Cells over the foot (a face's or a join's), 0 the lowest.
    pub k: u8,
    /// Cells in the face's or the join's run: two a level.
    pub n: u8,
    /// [`WEST`], [`EAST`], [`NORTH`]: which sides have lower ground (a rim's, a face's ends).
    pub side: u8,
    /// The level its heights count from.
    pub base: u8,
    /// Its own level.
    pub level: u8,
}

/// A cliff-like cell: a face, a rim, a ledge or a waterfall.
#[inline]
pub(crate) const fn cliffy(t: Tile) -> bool {
    matches!(t, Tile::Cliff | Tile::LedgeN | Tile::LedgeE | Tile::LedgeS | Tile::LedgeW | Tile::Waterfall)
}

/// Feet may stand on it.
#[inline]
fn walkable(t: Tile) -> bool {
    t != Tile::Void && t.flags() & F_SOLID == 0
}

/// What cell `(x, y)` of `src` is ([`HCell`]).
pub(crate) fn classify(src: &impl TileSource, x: i32, y: i32) -> HCell {
    let t = src.tile(x, y);
    let l = src.level(x, y);
    let lv = |dx: i32, dy: i32| src.level(x + dx, y + dy);
    let tl = |dx: i32, dy: i32| src.tile(x + dx, y + dy);
    let plain = HCell { base: l, level: l, ..HCell::default() };
    if t == Tile::Void {
        return plain;
    }
    if cliffy(t) {
        // Down the column over cliff of its own level to the foot.
        let mut j = 1;
        while j <= 4 && cliffy(tl(0, j)) && lv(0, j) == l {
            j += 1;
        }
        let foot = (tl(0, j) != Tile::Void).then(|| lv(0, j)).filter(|&f| f < l);
        let side = (u8::from(lv(-1, 0) < l && !cliffy(tl(-1, 0))) * WEST)
            | (u8::from(lv(1, 0) < l && !cliffy(tl(1, 0))) * EAST)
            | (u8::from(lv(0, -1) < l) * NORTH);
        if let Some(f) = foot {
            let n = 2 * (l - f);
            if (j - 1) < i32::from(n) {
                return HCell { kind: Kind::Face, k: (j - 1) as u8, n, side, base: f, level: l };
            }
        }
        if side != 0 {
            return HCell { kind: Kind::Rim, side, ..plain };
        }
        return HCell { kind: Kind::Crag, ..plain };
    }
    if !walkable(t) {
        return plain;
    }
    // A join's upper row: the ground under it is walkable and lower.
    let (ls, ln) = (lv(0, 1), lv(0, -1));
    if walkable(tl(0, 1)) && ls < l {
        let n = 2 * (l - ls);
        return HCell { kind: Kind::Join, k: n - 1, n, side: 0, base: ls, level: l };
    }
    // Its lower row: the ground over it is walkable and higher; or a flight down a north rim.
    if walkable(tl(0, -1)) && ln > l {
        return HCell { kind: Kind::Join, k: 0, n: 2 * (ln - l), side: 0, base: l, level: l };
    }
    if t == Tile::Stair && walkable(tl(0, -1)) && ln < l {
        return HCell { kind: Kind::NorthFlight, ..plain };
    }
    plain
}

/// Fills the painter's height scratch for the chunk whose first cell is `(x0, y0)`: every cell of
/// the scratch, read from `src` (a face's run is looked down four cells, inside `REACH`).
pub(crate) fn gather(p: &mut Painter, src: &impl TileSource, x0: i32, y0: i32) {
    p.levels = src.has_levels();
    p.s.any_deck = false;
    if !p.levels && !src.has_decks() {
        // A flat zone after one with height: nothing of it is left in the scratch.
        if p.s.hk_set {
            p.s.hk.fill(HCell::default());
            p.s.deck.fill(0);
            p.s.hk_set = false;
        }
        return;
    }
    p.s.hk_set = true;
    for j in 0..GM {
        for i in 0..GM {
            let (x, y) = (x0 + i - M, y0 + j - M);
            let k = (j * GM + i) as usize;
            p.s.hk[k] = if p.levels { classify(src, x, y) } else { HCell::default() };
            let d = src.deck(x, y);
            p.s.deck[k] = d;
            p.s.any_deck |= d != 0;
        }
    }
}

/// Lower ground is under a deck here (`TileSource::deck`).
pub const DECK_UNDER: u8 = 1;
/// A deck lies just east of this cell.
pub const DECK_E: u8 = 2;
/// A deck lies just west of it.
pub const DECK_W: u8 = 4;
/// A deck lies just south of it.
pub const DECK_S: u8 = 8;
/// A deck lies just north of it.
pub const DECK_N: u8 = 16;
/// Under a deck, its level stands in the top bits ([`deck_level`]).
pub const DECK_LEVEL_SHIFT: u8 = 5;

/// The level of the deck over a cell whose `TileSource::deck` is `d` (it has [`DECK_UNDER`]).
pub const fn deck_level(d: u8) -> u8 {
    d >> DECK_LEVEL_SHIFT
}
