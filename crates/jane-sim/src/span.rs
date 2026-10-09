//! Spans in the sim (MAP.md §2.5, §3; R2): bridges and walkways over walkable ground, so two
//! surfaces lie on one cell. Every rule here is asked only where [`ZoneGrid::has_spans`] says
//! there are spans, so a zone without them (every zone built so far) steps, saves and hashes as
//! it did.
//!
//! - **The layer bit** is `Unit::on_span`: set as a body steps from one of a whole span's ends
//!   onto its deck along its axis ([`deck_for_move`]), cleared as it steps off an end. Off a span
//!   it is always clear.
//! - **On the deck** a body is collided against the deck: its cells are open, the cells beside it
//!   are parapets, the ground beyond its ends is the ground's ([`deck_meets`]). Under it, the
//!   ground's own tiles rule (and the rect's end rows are solid there: the abutments). Hops never
//!   start on a deck.
//! - **Its level** is the deck's ([`level_on`]): what sight, shots, blows and the AI read.
//! - **Occupancy** keys the deck apart (`ZoneGrid::occupy_on`): a body on the viaduct and one on
//!   the towpath under it never block each other.

use jane_core::Vec2;
use jane_core::blueprint::{Span, SpanIx};

use crate::grid::ZoneGrid;
use crate::state::Unit;

/// The level a body at `p` stands at: its deck's on a span, else its ground's.
#[inline]
pub fn level_on(g: &ZoneGrid, p: Vec2, deck: Option<SpanIx>) -> u8 {
    match deck {
        Some(i) if g.has_spans() => g.spans()[usize::from(i)].deck_level,
        _ => crate::height::level_of(g, p),
    }
}

/// The level a unit stands at ([`level_on`]).
#[inline]
pub fn unit_level(g: &ZoneGrid, u: &Unit) -> u8 {
    level_on(g, u.pos, u.on_span)
}

/// The level of a point (a bolt, a pool) over a deck if `deck` and there is one under it, else
/// on the ground.
#[inline]
pub fn point_level(g: &ZoneGrid, p: Vec2, deck: bool) -> u8 {
    let (x, y) = p.cell();
    match g.deck_at(x, y).filter(|_| deck) {
        Some(i) => g.spans()[usize::from(i)].deck_level,
        None => crate::height::level_of(g, p),
    }
}

/// Is a bolt or a pool at `p` (over a deck if `deck`) at the level of body `u` (MAP.md §3.3)?
/// Always, in a zone without levels or spans.
#[inline]
pub fn same_layer(g: &ZoneGrid, p: Vec2, deck: bool, u: &Unit) -> bool {
    if !g.has_spans() {
        return crate::height::same_level(g, p, u.pos);
    }
    point_level(g, p, deck) == unit_level(g, u)
}

/// The deck a move of this body is collided against: the one it is on, or the one it would step
/// onto from an end it stands in (at the deck's level, the span whole). `None`: the ground.
pub fn deck_for_move(g: &ZoneGrid, u: &Unit) -> Option<SpanIx> {
    if let Some(i) = u.on_span {
        return Some(i);
    }
    let (x, y) = u.pos.cell();
    g.deck_end_at(x, y).map(|(i, _)| i)
}

/// What feet on (or stepping onto) span `s`'s deck meet in cell `(x, y)`: `Some(false)` the deck,
/// open; `Some(true)` a parapet; `None` the ground's own rule (beyond its ends).
#[inline]
pub fn deck_meets(s: &Span, x: i32, y: i32) -> Option<bool> {
    if s.rect.contains(x, y) {
        Some(false)
    } else if s.beside(x, y) {
        Some(true)
    } else {
        None
    }
}

/// The deck a body collided against `deck` stands on once at `p`: that deck while its feet are
/// over it, none once they are off it.
#[inline]
pub fn deck_after(g: &ZoneGrid, deck: Option<SpanIx>, p: Vec2) -> Option<SpanIx> {
    deck.filter(|&i| {
        let (x, y) = p.cell();
        g.spans()[usize::from(i)].rect.contains(x, y)
    })
}

/// Is the walker at cell `from` (on deck `on`) stepping onto the deck over `to`, so that is the
/// layer `to` is held on? On the deck it stays on it while `to` is under it; from an end it steps
/// onto it along the axis.
pub fn steps_on_deck(g: &ZoneGrid, on: Option<SpanIx>, from: (i32, i32), to: (i32, i32)) -> bool {
    if !g.has_spans() {
        return false;
    }
    if let Some(i) = on {
        return g.spans()[usize::from(i)].rect.contains(to.0, to.1);
    }
    g.deck_end_at(from.0, from.1).is_some_and(|(i, _)| g.spans()[usize::from(i)].rect.contains(to.0, to.1))
}

/// May a blow at `a` (on deck `da`) reach a body at `b` (on deck `db`) across layers (MAP.md
/// §3.3: melee never crosses a layer)? Yes when neither is on a deck, or both are on the same
/// one; one on a deck reaches one off it only where the other is not under it (at its end).
pub fn blow_crosses_layers(g: &ZoneGrid, a: Vec2, da: Option<SpanIx>, b: Vec2, db: Option<SpanIx>) -> bool {
    match (da, db) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        (Some(d), None) => !under(g, d, b),
        (None, Some(d)) => !under(g, d, a),
    }
}

/// May a blow struck at `a` (on deck `da`) reach body `b` (MAP.md §3.3)? Never across layers
/// ([`blow_crosses_layers`]); with a deck at either end, only at one level; on the ground,
/// `height::melee_reaches` (true on flat ground).
pub fn blow_reaches(g: &ZoneGrid, a: Vec2, da: Option<SpanIx>, b: &Unit) -> bool {
    if !g.has_spans() || da.is_none() && b.on_span.is_none() {
        return crate::height::melee_reaches(g, a, b.pos);
    }
    blow_crosses_layers(g, a, da, b.pos, b.on_span) && level_on(g, a, da) == unit_level(g, b)
}

/// Is a body on the ground at `p` under span `d`'s deck?
fn under(g: &ZoneGrid, d: SpanIx, p: Vec2) -> bool {
    let (x, y) = p.cell();
    g.spans()[usize::from(d)].rect.contains(x, y)
}

/// The zone's spans set as `changed` says (`ZoneState::spans_changed` over the blueprint's own).
pub fn apply_changed(g: &mut ZoneGrid, built: &[Span], changed: u64) {
    for (i, s) in built.iter().enumerate() {
        g.set_span_broken(i as SpanIx, s.broken != (changed >> i & 1 != 0));
    }
}

/// Break span `i` of the zone, or mend it: its state and its runtime's grid together.
pub fn set_span_broken(
    zone: &mut crate::state::ZoneState,
    rt: &mut crate::runtime::ZoneRuntime,
    i: SpanIx,
    broken: bool,
) {
    let Some(built) = rt.grid.spans().get(usize::from(i)).map(|s| s.broken) else { return };
    let bit = 1u64 << i;
    zone.spans_changed = if broken == built { zone.spans_changed & !bit } else { zone.spans_changed | bit };
    rt.grid.set_span_broken(i, broken);
}

/// `viaduct`: a hand-built ground with every case of R2's spans (MAP.md §9, dev only, never a
/// zone of the game), 64 x 48 cells, for the tests:
///
/// - the north terrace at level 1 (rows 1 to 17), its south face rows 18 and 19 (cliff);
/// - a valley at level 0 (rows 20 to 27, [`TOWPATH`]) running east-west, its south rim row 28;
/// - the south terrace at level 1 (rows 29 to 46);
/// - [`VIADUCT`]: a span along y over the valley, 4 wide (x 30 to 33, rows 18 to 28), its deck at
///   level 1, its ends rows 17 and 29; the towpath runs under it;
/// - [`FOOTBRIDGE`]: a broken span 2 wide at x 46 and 47, the same rows;
/// - a stair down the north face ([`STAIR`], x 10 to 13) and one up the south rim ([`RIM_STAIR`],
///   x 52 to 55): the long way round between towpath and terraces;
/// - walls round the edge, `start` at (20, 10) facing south.
pub mod viaduct {
    use alloc::vec;

    use jane_core::blueprint::{Mark, Span};
    use jane_core::{Blueprint, Cell, Key, Plane, Rect, Tile, ZoneId};

    pub const W: u32 = 64;
    pub const H: u32 = 48;
    pub const FACE: Rect = Rect { x: 1, y: 18, w: 62, h: 2 };
    pub const TOWPATH: Rect = Rect { x: 1, y: 20, w: 62, h: 8 };
    pub const RIM: Rect = Rect { x: 1, y: 28, w: 62, h: 1 };
    pub const VIADUCT: Rect = Rect { x: 30, y: 18, w: 4, h: 11 };
    pub const FOOTBRIDGE: Rect = Rect { x: 46, y: 18, w: 2, h: 11 };
    pub const STAIR: Rect = Rect { x: 10, y: 18, w: 4, h: 2 };
    pub const RIM_STAIR: Rect = Rect { x: 52, y: 28, w: 4, h: 1 };

    /// The viaduct in `zone`'s blueprint.
    pub fn blueprint(zone: ZoneId) -> Blueprint {
        let cat = jane_data::catalog();
        let mut bp = Blueprint::new(zone, W, H, Tile::Grass);
        let (w, h) = (W as i32, H as i32);
        for r in [Rect::new(0, 0, w, 1), Rect::new(0, h - 1, w, 1), Rect::new(0, 0, 1, h), Rect::new(w - 1, 0, 1, h)] {
            bp.tiles.fill_rect(r, Tile::Wall);
        }
        let mut lv = vec![1u8; (W * H) as usize];
        for (x, y) in TOWPATH.cells() {
            lv[(y * w + x) as usize] = 0;
        }
        bp.tiles.fill_rect(FACE, Tile::Cliff);
        bp.tiles.fill_rect(RIM, Tile::Cliff);
        bp.tiles.fill_rect(TOWPATH, Tile::Road);
        bp.tiles.fill_rect(STAIR, Tile::Stair);
        // The stair's lower step is at the towpath's level; the rim's one step is the terrace's.
        for x in STAIR.x..STAIR.right() {
            lv[((STAIR.y + 1) * w + x) as usize] = 0;
        }
        bp.tiles.fill_rect(RIM_STAIR, Tile::Stair);
        bp.level = Some(Plane::pack(W, H, &lv));
        bp.spans = vec![
            Span { rect: VIADUCT, along_x: false, deck_level: 1, broken: false },
            Span { rect: FOOTBRIDGE, along_x: false, deck_level: 1, broken: true },
        ];
        bp.marks.insert(
            Key::Name(cat.story.start.mark),
            Mark { cell: Cell::new(20, 10), facing: Some(jane_core::action::Facing::South) },
        );
        bp
    }
}

/// Is span `s` well formed on `g` (MAP.md §2.5): its rect's end rows solid to the ground under it
/// (the abutments, so nothing walks up onto the deck from beneath), its ends open ground at its
/// deck's level, 2 to 4 wide and at most 40 long?
pub fn well_formed(g: &ZoneGrid, s: &Span) -> bool {
    let r = s.rect;
    let (a, b) = if s.along_x {
        (jane_core::Rect::new(r.x, r.y, 1, r.h), jane_core::Rect::new(r.right() - 1, r.y, 1, r.h))
    } else {
        (jane_core::Rect::new(r.x, r.y, r.w, 1), jane_core::Rect::new(r.x, r.bottom() - 1, r.w, 1))
    };
    let solid = |q: jane_core::Rect| q.cells().all(|(x, y)| g.solid(x, y));
    let ends = s.ends().iter().all(|e| e.cells().all(|(x, y)| !g.solid(x, y) && g.level_at(x, y) == s.deck_level));
    solid(a) && solid(b) && ends && (2..=4).contains(&if s.along_x { r.h } else { r.w }) && s.length() <= 40
}
