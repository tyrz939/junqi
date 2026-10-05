//! Placements: how content gets into the generated county without knowing where anything is
//! (`jane/src/world/placements.ts`). A quest needs "six pumpkins in the Top Field", "a notice board
//! on the town square", "the scarecrow nearest the farm, with something in its pocket". Every seed
//! puts the Top Field, the square and the farm somewhere else, so content says where by name
//! ([`PlacementDef`], `data/placements/*.json`) and this works out the cells.
//!
//! Everything a row names is in the county's contract (`County::promised`), so a seed that cannot
//! place it fails validation and is re-rolled like any other broken county. Nothing is skipped
//! quietly.
//!
//! Four stages, because the builder claims ground as it goes ([`Stage`]): inside a chunk before
//! the chunk's box is closed to scatter, at a small place after it has been dressed, in a patch
//! last, on whatever is still open; and at a story's place once the stories have claimed theirs.
//!
//! Every row throws its own dice, [`Step::CountyPlaceRow`] keyed by a hash of its key's name: adding
//! a row, or a stone to a garden, moves nothing else in the county.

use jane_core::action::{Facing, TextRef};
use jane_core::blueprint::PropSpawn;
use jane_core::hash::Fnv;
use jane_core::{Key, NameId, Rect, Sfc32};
use jane_data::{PlaceAt, PlacementDef, PropEdit};

use super::chunks::apply_template;
use super::{County, areas, centre};
use crate::kit::Kit;
use crate::skeleton::{SKEL_H, SKEL_W, Skeleton};
use crate::steps::Step;

/// A small place where it stands, and the kind it is dressed as: the skeleton's roll, or the kind
/// a `poi` row needed it to be ([`claim_pois`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiSpot {
    /// Its centre cell.
    pub x: i32,
    pub y: i32,
    /// `None`: an anchor's bare spot (a lamp post stands there).
    pub kind: Option<NameId>,
    /// A small place the story needs by name: its row in the anchor table.
    pub anchor: Option<u8>,
}

impl PoiSpot {
    /// Every small place of the skeleton, rolled ones then anchors', as the county dresses them.
    pub fn all(sk: &Skeleton) -> Vec<PoiSpot> {
        sk.pois.iter().map(|p| PoiSpot { x: centre(p.mx), y: centre(p.my), kind: p.kind, anchor: p.anchor }).collect()
    }
}

/// Before the small places are dressed: which small place each `poi` row gets, turning one into
/// the kind the row needs if the seed did not roll one. Returns `(row key, index into pois)` in row
/// order, and changes `pois[n].kind`. No dice.
///
/// The nearest to the row's site (the town if it names none) of: a place already claimed as this
/// kind (two quests about the same scarecrow share it); else a free place of the kind; else any
/// free place, re-dressed. An anchor is its story's ground and is never handed out. A seed with no
/// small place at all claims nothing, and the contract check fails the county.
pub fn claim_pois(sk: &Skeleton, pois: &mut [PoiSpot], rows: &[PlacementDef]) -> Vec<(NameId, usize)> {
    let town = jane_data::catalog().county.site_ix("town");
    let mut taken: Vec<(usize, NameId)> = Vec::new();
    let mut out = Vec::new();
    for row in rows {
        let PlaceAt::Poi { kind, near } = row.at else { continue };
        let Some(site) = near.or(town).and_then(|s| sk.sites.get(usize::from(s))) else { continue };
        let (nx, ny) = (i64::from(centre(site.mx)), i64::from(centre(site.my)));
        let mut best: Option<(i64, usize)> = None;
        for want_kind in [true, false] {
            for (n, p) in pois.iter().enumerate() {
                if p.anchor.is_some() {
                    continue;
                }
                if taken.iter().any(|&(i, k)| i == n && k != kind) {
                    continue;
                }
                if want_kind && p.kind != Some(kind) {
                    continue;
                }
                let (dx, dy) = (i64::from(p.x) - nx, i64::from(p.y) - ny);
                let d = dx * dx + dy * dy;
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, n));
                }
            }
            if best.is_some() {
                break;
            }
        }
        let Some((_, n)) = best else { continue };
        pois[n].kind = Some(kind);
        if !taken.iter().any(|&(i, _)| i == n) {
            taken.push((n, kind));
        }
        out.push((row.key, n));
    }
    out
}

/// Which rows go down now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// At a mark, a site, a slot: inside the chunks while their ground is open.
    Chunks,
    /// At a kind of small place, or at an anchor, once the small places are dressed.
    Pois,
    /// In a named patch, on whatever is still open.
    Areas,
    /// At a story's place, once the stories have claimed theirs (PORT.md §6.m stage 8).
    Places,
}

impl Stage {
    /// The stage a row goes down in. A row in front of a prop (`PlaceAt::Prop`) goes down in the
    /// first stage its prop is there by.
    pub fn of(at: PlaceAt) -> Stage {
        match at {
            PlaceAt::Place { .. } => Stage::Places,
            PlaceAt::Poi { .. } | PlaceAt::Anchor(_) => Stage::Pois,
            PlaceAt::Area(_) => Stage::Areas,
            PlaceAt::Mark(_) | PlaceAt::Site(_) | PlaceAt::Slot(_) | PlaceAt::Prop(_) => Stage::Chunks,
        }
    }
}

/// Put the rows of one stage on the map, in row order: the order decides who gets a contested spot.
pub fn apply_placements(c: &mut County<'_>, stage: Stage, rows: &[PlacementDef]) {
    for row in rows {
        apply_row(c, stage, row);
    }
}

/// A row's own dice: [`Step::CountyPlaceRow`], keyed by its key's name.
pub fn row_dice(k: &Kit, row: &PlacementDef) -> Sfc32 {
    let h = Fnv::new().str(jane_data::catalog().name(row.key)).finish();
    k.dice(Step::CountyPlaceRow, h as i32, 0)
}

/// The index of the prop keyed `key`, the first of that key.
pub fn prop_index(k: &Kit, key: Key) -> Option<usize> {
    k.blueprint().props.iter().position(|p| p.key == key)
}

/// A prop def's footprint, in cells.
fn size_of(def: jane_core::PropDefId) -> (i32, i32) {
    let row = jane_data::catalog().story.prop(def);
    (i32::from(row.w), i32::from(row.h))
}

fn apply_row(c: &mut County<'_>, stage: Stage, row: &PlacementDef) {
    if let PlaceAt::Prop(target) = row.at {
        // Beside a prop another row or a chunk placed: in the first stage the prop is there by, once.
        let first = Key::Name(row.keys[0]);
        let bp = c.k.blueprint();
        let placed = bp.props.iter().any(|p| p.key == first) || bp.units.iter().any(|u| u.key == first);
        if prop_index(&c.k, Key::Name(target)).is_none() || placed {
            return;
        }
    } else if Stage::of(row.at) != stage {
        return;
    }
    if let PlaceAt::Place { .. } = row.at {
        at_place(c, row);
        return;
    }
    if let Some(e) = &row.edit {
        if let Some(i) = prop_index(&c.k, Key::Name(row.key)) {
            apply_edit(&mut c.k.props_mut()[i], e);
        }
        return;
    }
    if let PlaceAt::Slot(slot) = row.at {
        // Exactly there. A door belongs in the wall, which no search for open ground would choose.
        let at = c.chunks.iter().find_map(|ch| ch.slot(slot)).or_else(|| areas::area_slot(c, slot));
        if let (Some((x, y)), Some(t)) = (at, &row.prop) {
            let top = put_prop(&mut c.k, row.key, t, x, y);
            hide_under(&mut c.k, row, top);
        }
        return;
    }
    let Some(anchor) = anchor_of(c, row) else { return };
    let mut rng = row_dice(&c.k, row);
    let (w, h) = row.prop.map_or((1, 1), |t| size_of(t.def));
    let mut tops = Vec::new();
    for (n, &key) in row.keys.iter().enumerate() {
        // Spread: a point of its own somewhere in the patch, then the nearest open ground to that.
        let mut from = anchor;
        if row.spread {
            // Inside the patch's circle, not its square: the corners are somebody else's ground (and
            // somebody else's threat). A small search radius, so it stays where it was thrown.
            let reach = anchor.within * 4 / 5;
            let (mut ox, mut oy) = (0, 0);
            for _ in 0..8 {
                ox = rng.range(-reach, reach);
                oy = rng.range(-reach, reach);
                if ox * ox + oy * oy <= reach * reach {
                    break;
                }
                (ox, oy) = (0, 0);
            }
            from = Anchor { x: anchor.x + ox, y: anchor.y + oy, within: 6 };
        }
        let spot = open_spot(&c.k, &mut rng, from, w, h)
            .or_else(|| if row.spread { open_spot(&c.k, &mut rng, anchor, w, h) } else { None });
        let Some((x, y)) = spot else { break };
        if let Some(t) = &row.prop {
            tops.push(put_prop(&mut c.k, key, t, x, y));
        }
        if let Some(u) = row.unit {
            // Beside the prop if there is one, on the spot if not.
            let ux = if row.prop.is_some() { x + w } else { x };
            let phase = u.phase.unwrap_or_else(|| threat_at(c.sk, ux, y));
            let spawn = c.k.unit(Some(Key::Name(key)), u.def, ux, y, Vec::new());
            spawn.facing = u.facing;
            if phase > 1 {
                spawn.phase = phase;
            }
        }
        if n == 0 {
            if let Some(r) = row.rect {
                // Centred on the named place, not on wherever the thing found room: the text says
                // "at the scarecrow".
                let (rw, rh) = (i32::from(r.w), i32::from(r.h));
                c.k.rect(Key::Name(r.name), Rect::new(anchor.x - rw / 2, anchor.y - rh / 2, rw, rh));
            }
            if let Some(m) = row.mark {
                c.k.mark(Key::Name(m), x, y + h, Some(Facing::South));
            }
        }
    }
    if let Some(b) = row.beside {
        for &t in &tops {
            move_beside(&mut c.k, t, Key::Name(b));
        }
    }
    if !tops.is_empty() {
        let top = pick_top(&c.k, row, &tops);
        hide_under(&mut c.k, row, top);
    }
}

/// A row at a story's place, once the stories stage has claimed the places
/// ([`super::stories::at_place`]).
fn at_place(c: &mut County<'_>, row: &PlacementDef) {
    super::stories::at_place(c, row);
}

/// Set down a row's prop keyed `key`, with the fields its template sets. Returns its index.
pub fn put_prop(k: &mut Kit, key: NameId, t: &jane_data::PropTemplate, x: i32, y: i32) -> usize {
    apply_template(k.prop(Some(Key::Name(key)), t.def, x, y), t);
    k.blueprint().props.len() - 1
}

/// The fields an `edit` row sets, over a prop that is already there.
pub fn apply_edit(p: &mut PropSpawn, e: &PropEdit) {
    if let Some(k) = e.key {
        p.key = Key::Name(k);
    }
    if let Some(d) = e.def {
        p.def = d;
    }
    if let Some(v) = e.locked {
        p.locked = v;
    }
    if let Some(t) = e.key_tag {
        p.key_tag = Some(Key::Name(t));
    }
    if let Some(v) = e.hidden {
        p.hidden = v;
    }
    if let Some(l) = e.loot {
        p.loot = l.to_vec();
    }
    if let Some(u) = e.use_list {
        p.use_list = Some(u);
    }
    if let Some(t) = e.talk {
        p.talk = Some(t);
    }
    if let Some(l) = e.label {
        p.label = Some(TextRef::Text(l));
    }
    if let Some(l) = e.night_lock {
        p.night_lock = Some(l.lock());
    }
}

/// The threat of the ground under a cell, for a placed creature's phase.
pub fn threat_at(sk: &Skeleton, x: i32, y: i32) -> u8 {
    sk.threat.read((x >> 4).min(SKEL_W - 1), (y >> 4).min(SKEL_H - 1), 0)
}

/// Put a placed thing on the nearest open cell touching a prop's footprint: not a wall, not water,
/// not under another prop or on a person. Tried in rings out from the footprint's edge, the front
/// first (below it, where she stands to use it), then the sides, then behind. No dice.
pub fn move_beside(k: &mut Kit, t: usize, key: Key) {
    let Some(q) = prop_index(k, key) else { return };
    let bp = k.blueprint();
    let (qp, tp) = (&bp.props[q], &bp.props[t]);
    let (qx, qy) = (i32::from(qp.cell.x), i32::from(qp.cell.y));
    let ((qw, qh), (tw, th)) = (size_of(qp.def), size_of(tp.def));
    let taken = |x: i32, y: i32| {
        bp.props.iter().enumerate().any(|(i, p)| {
            let (px, py) = (i32::from(p.cell.x), i32::from(p.cell.y));
            let (sw, sh) = size_of(p.def);
            i != t && x < px + sw && px < x + tw && y < py + sh && py < y + th
        }) || bp.units.iter().any(|u| Rect::new(x, y, tw, th).contains(i32::from(u.cell.x), i32::from(u.cell.y)))
    };
    let open = |x: i32, y: i32| (0..th).all(|j| (0..tw).all(|i| !k.solid(x + i, y + j))) && !taken(x, y);
    let mut found = None;
    'rings: for r in 1..=3 {
        let (left, right) = (qx - r - tw + 1, qx + qw + r - 1);
        let mut ring: Vec<(i32, i32)> = (left..=right).map(|x| (x, qy + qh + r - 1)).collect();
        let mut y = qy + qh + r - 2;
        while y > qy - r - th {
            ring.push((left, y));
            ring.push((right, y));
            y -= 1;
        }
        ring.extend((left..=right).map(|x| (x, qy - r - th + 1)));
        for (x, y) in ring {
            if open(x, y) {
                found = Some((x, y));
                break 'rings;
            }
        }
    }
    let Some((x, y)) = found else { return };
    k.props_mut()[t].cell = crate::kit::cell(x, y);
    k.claim(Rect::new(x, y, tw, th));
}

/// Which of a row's props the thing is under: a hash of where they stand, so the seed chooses and
/// no dice are drawn.
pub fn pick_top(k: &Kit, row: &PlacementDef, tops: &[usize]) -> usize {
    if tops.len() == 1 {
        return tops[0];
    }
    let mut h = Fnv::new().str(jane_data::catalog().name(row.key));
    for &t in tops {
        let c = k.blueprint().props[t].cell;
        h = h.u16(c.x).u16(c.y);
    }
    tops[h.mix() as usize % tops.len()]
}

/// The thing a row `hides`: a hidden prop on the top prop's cell, which the top prop names as
/// lying under it (shown when the top is pushed off it, while `when` holds).
pub fn hide_under(k: &mut Kit, row: &PlacementDef, top: usize) {
    let Some(h) = &row.hides else { return };
    let c = k.blueprint().props[top].cell;
    let p = k.prop(Some(Key::Name(h.key)), h.prop.def, i32::from(c.x), i32::from(c.y));
    apply_template(p, &h.prop);
    p.hidden = true;
    let top = &mut k.props_mut()[top];
    top.under = Some(Key::Name(h.key));
    if let Some(w) = h.when {
        top.under_when = Some(w);
    }
}

/// Where a row's things go: a cell, and how far from it they may land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub x: i32,
    pub y: i32,
    pub within: i32,
}

/// The named place a row asks for, in cells; `None` when this seed has no such place.
pub fn anchor_of(c: &County<'_>, row: &PlacementDef) -> Option<Anchor> {
    let within = |d: i32| row.within.map_or(d, i32::from);
    let bp = c.k.blueprint();
    match row.at {
        PlaceAt::Mark(m) => {
            let m = bp.marks.get(&Key::Name(m))?;
            Some(Anchor { x: i32::from(m.cell.x), y: i32::from(m.cell.y), within: within(8) })
        }
        PlaceAt::Site(s) => {
            let b = c.chunks.iter().find(|ch| ch.site == s)?.bounds;
            Some(Anchor { x: b.x + b.w / 2, y: b.y + b.h / 2, within: within(b.w.max(b.h) / 2 + 4) })
        }
        PlaceAt::Area(a) => {
            let a = c.sk.area(a)?;
            Some(Anchor { x: centre(a.mx), y: centre(a.my), within: within(i32::from(a.def.radius) * 4 / 5) })
        }
        PlaceAt::Prop(key) => {
            // In front of it: the cell below its footprint, the middle of its width. A door's step.
            let q = &bp.props[prop_index(&c.k, Key::Name(key))?];
            let (w, h) = size_of(q.def);
            Some(Anchor { x: i32::from(q.cell.x) + (w >> 1), y: i32::from(q.cell.y) + h, within: within(3) })
        }
        PlaceAt::Anchor(a) => {
            let p = c.pois.iter().find(|p| p.anchor == Some(a))?;
            Some(Anchor { x: p.x, y: p.y + 3, within: within(6) })
        }
        PlaceAt::Poi { .. } => {
            let &(_, n) = c.claimed.iter().find(|(k, _)| *k == row.key)?;
            let p = c.pois.get(n)?;
            Some(Anchor { x: p.x, y: p.y + 3, within: within(5) })
        }
        PlaceAt::Slot(_) | PlaceAt::Place { .. } => None,
    }
}

/// An open, unclaimed `w x h` footprint (with a cell of open ground round it) near the anchor,
/// tried outward in squares that grow by eight fifths (1, 2, 4, 7, 12, 20, 32 cells out) so a thing
/// lands as close as it can. If every throw misses, every footprint in the last square is looked
/// at, nearest ring first, before the row is given up: a row with room somewhere near its place
/// always lands (the TypeScript gave up, and a crowded doorstep in the town lost its milk bottles
/// on one seed in sixty). The look draws no dice, so a row that lands on a throw lands where it did.
pub fn open_spot(k: &Kit, rng: &mut Sfc32, a: Anchor, w: i32, h: i32) -> Option<(i32, i32)> {
    let mut r = 1;
    while r <= a.within {
        let square = Rect::new(a.x - r, a.y - r, r * 2 + 1, r * 2 + 1);
        if let Some(s) = k.spot(rng, square, w, h, 1, SPOT_TRIES) {
            return Some(s);
        }
        r = grow(r);
    }
    for r in 0..=a.within {
        for oy in -r..=r {
            for ox in -r..=r {
                let (x, y) = (a.x + ox, a.y + oy);
                let whole = x + w - 1 <= a.x + a.within && y + h - 1 <= a.y + a.within;
                if ox.abs().max(oy.abs()) == r && whole && k.fits(x, y, w, h, 1) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// The next square out of [`open_spot`]: eight fifths, rounded.
fn grow(r: i32) -> i32 {
    (r * 8 + 4) / 5
}

/// Throws per square of [`open_spot`].
const SPOT_TRIES: u32 = 24;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_spot_grows_as_the_typescript_did() {
        let mut seen = Vec::new();
        let mut r = 1;
        while r <= 40 {
            seen.push(r);
            r = grow(r);
        }
        assert_eq!(seen, vec![1, 2, 4, 7, 12, 20, 32]);
    }

    #[test]
    fn every_row_has_a_stage() {
        let cat = jane_data::catalog();
        for row in cat.county.placements {
            let s = Stage::of(row.at);
            assert_eq!(s == Stage::Places, row.at_place(), "{}", cat.name(row.key));
        }
    }
}
