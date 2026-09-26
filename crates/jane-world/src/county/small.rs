//! The skeleton's small places, dressed (`smallPlace` in `county.ts`), and the signposts given
//! words. A small place is a shape to walk past and a named mark three cells south of its middle:
//! `poi_<n>` for one the seed rolled, the anchor's own id for one the story needs. A rolled place
//! is furnished here; a needed one gets its ground here and its things from the placement rows,
//! where they can carry a key, a dialogue tree and loot, and a clearing round its mark and a rect
//! of its name. Each small place is dressed on dice of its own, named for where it stands.
//!
//! The order in the county (`county.ts`): `claim_pois` decides at `place_chunks` which small
//! place each `poi` placement row gets and re-kinds it; [`small_places`] dresses them (and the
//! signposts, and the needed places' rects); `place_pois` puts the rows down and claims each small
//! place's ground; then `place_areas`, then the country's places.
//!
//! Where this parts from the TypeScript: a prop's footprint is the catalog's (the TypeScript
//! passed 2 x 2 for barrels, crates and carts, whatever their row said); nothing here paints
//! inside a set place's box, and an outskirt grows nothing solid on claimed ground; a needed
//! place's way out breaks crag (and, for one standing in water, decks the water) down to the
//! nearest road, and its mark moves to the nearest open cell when its own is wet or solid. The
//! TypeScript left those three to the county's re-roll.

use jane_core::action::{Action, Facing};
use jane_core::num::{Permille, isqrt};
use jane_core::tile::F_SOLID;
use jane_core::{Key, PropDefId, Rect, Sfc32, Tile};

use super::country::defs::defs;
use super::country::roads::{compass, distance_words};
use super::country::{downhill, ellipse_within, in_box};
use super::placements::PoiSpot;
use super::{County, centre};
use crate::steps::Step;

/// A small place's mark: the anchor's id, or `poi_<n>` for the `n`th of the list.
pub fn poi_key(c: &mut County<'_>, p: &PoiSpot, n: usize) -> Key {
    match p.anchor {
        Some(a) => Key::Name(jane_data::catalog().county.anchor(a).id),
        None => c.k.local(&format!("poi_{n}")),
    }
}

/// Every small place dressed, the signposts given words, and each needed place's rect.
pub fn small_places(c: &mut County<'_>) {
    let pois = c.pois.clone();
    for (n, p) in pois.iter().enumerate() {
        let mut rng = c.k.dice(Step::CountySmall, p.x, p.y);
        let key = poi_key(c, p, n);
        small_place(c, &mut rng, p, key);
    }
    signposts(c);
    // A place the story needs is known by its name: a mark (made above) and a rect of the same name.
    for (n, p) in pois.iter().enumerate() {
        if p.anchor.is_none() {
            continue;
        }
        let key = poi_key(c, p, n);
        if c.k.blueprint().marks.contains_key(&key) {
            c.k.rect(key, Rect::new(p.x - 6, p.y - 4, 13, 11));
        }
    }
}

/// One small place, dressed as its kind.
fn small_place(c: &mut County<'_>, rng: &mut Sfc32, p: &PoiSpot, name: Key) {
    let cat = jane_data::catalog();
    let (x, y) = (p.x, p.y);
    let rolled = p.anchor.is_none();
    let mut kind = p.kind.map_or("none", |k| cat.name(k));
    // A rolled place that landed on a road or in water is dropped. A place the story needs keeps
    // its mark (on ground made open for it) and goes undressed, because a path runs through it.
    if c.k.is_claimed(x, y) || c.k.get(x, y) == Tile::Water {
        if rolled {
            return;
        }
        kind = "none";
    }
    let d = defs();
    let mut s = Small { c: &mut *c, rng, x, y, rolled };
    match kind {
        // A bare spot the story needs: a mark and nothing else.
        "none" => {}
        "well" => {
            // Cobble worn by buckets, a trough, and a low wall on the windward side that somebody
            // built and nobody finished.
            s.pad(9, 9, Tile::Cobble);
            s.c.k.set(x, y, Tile::Water);
            s.lay(x - 4, y - 1, 1, 4, Tile::Wall);
            s.put(d.p.well_head, 0, -1);
            s.put(d.p.barrel, 2, 2);
            s.outskirt(6, Tile::Bush, 14);
        }
        "shrine" | "statue" => {
            s.pad(9, 7, Tile::Cobble);
            s.put(d.p.pillar, 0, 0);
            s.put(d.p.pillar, -3, 1);
            s.put(d.p.pillar, 3, 1);
            s.lay(x - 1, y + 2, 3, 1, Tile::Rubble);
            s.outskirt(6, Tile::Bush, 12);
        }
        "scarecrow" => {
            // Not a post in a field: a field, with the post in it. The furrows are what you see first.
            s.pad(13, 9, Tile::Dirt);
            for r in (-3..=3).step_by(2) {
                s.lay(x - 6, y + r, 13, 1, Tile::Garden);
            }
            s.put(d.p.scarecrow, 0, 0);
            s.outskirt(8, Tile::GrassTall, 18);
        }
        "signpost" => {
            // A fingerpost, the passing place worn into the verge beside it, and a bench of sorts.
            s.pad(9, 7, Tile::Dirt);
            // Two courses, not one: a single cell of setts is seen edge on and reads as rungs.
            s.lay(x - 3, y + 2, 7, 2, Tile::Cobble);
            s.put(d.p.signpost, 0, 0);
            s.put(d.p.crate_, -3, 1);
            s.outskirt(6, Tile::Bush, 12);
        }
        "signal" => {
            s.pad(7, 7, Tile::Cobble);
            s.lay(x - 3, y + 3, 7, 1, Tile::Rail);
            s.put(d.p.pillar, 0, 0);
            s.put(d.p.signpost, 2, 1);
            s.outskirt(6, Tile::Bush, 10);
        }
        "stones" => {
            s.pad(13, 13, Tile::Dirt);
            for (ox, oy) in [(-4, -4), (4, -4), (-5, 1), (5, 1), (0, 5), (-2, -5), (3, 4)] {
                s.put(d.p.pillar, ox, oy);
            }
            s.outskirt(9, Tile::GrassTall, 22);
        }
        "cottage" | "hut" => {
            // A roofless house still has its garden wall, its yard, and the black ring of the fire.
            s.pad(16, 13, Tile::Dirt);
            s.lay(x + 1, y - 1, 3, 2, Tile::Rubble);
            s.lay(x - 4, y - 4, 8, 1, Tile::HouseWall);
            s.lay(x - 4, y - 4, 1, 5, Tile::HouseWall);
            s.lay(x + 3, y - 4, 1, 3, Tile::HouseWall);
            if rolled {
                s.lay(x - 7, y + 4, 14, 1, Tile::Fence);
                s.lay(x - 7, y - 2, 1, 7, Tile::Fence);
                // The gate: a garden wall with no way through is a pen.
                s.lay(x - 1, y + 4, 3, 1, Tile::Dirt);
            }
            s.lay(x - 6, y + 1, 5, 2, Tile::Garden);
            s.put(d.p.campfire_cold, 5, 1);
            s.put(d.p.crate_, -2, 3);
            s.outskirt(9, Tile::Bush, 16);
        }
        "greenhouse" => {
            s.pad(15, 11, Tile::Garden);
            s.lay(x - 5, y - 4, 11, 1, Tile::Wall);
            s.lay(x - 5, y - 4, 1, 6, Tile::Wall);
            s.lay(x + 5, y - 4, 1, 4, Tile::Glass);
            for r in (-2..=3).step_by(2) {
                s.lay(x - 6, y + r, 12, 1, Tile::Dirt);
            }
            s.put(d.p.barrel, -4, 4);
            s.outskirt(8, Tile::Bush, 14);
        }
        "pump" | "pipe_end" => {
            s.pad(11, 9, Tile::Dirt);
            s.c.k.set(x + 2, y - 2, Tile::Rubble);
            s.lay(x - 1, y - 2, 3, 2, Tile::Wall);
            s.lay(x - 5, y + 2, 11, 2, Tile::Cobble);
            s.put(d.p.barrel, 4, -1);
            s.put(d.p.crate_, -4, -1);
            s.outskirt(7, Tile::Rubble, 12);
        }
        "jetty" | "boat" => {
            s.pad(5, 13, Tile::Cobble);
            s.lay(x - 1, y - 6, 3, 4, Tile::FloorWood);
            s.put(d.p.crate_, 1, 2);
            s.put(d.p.barrel, -2, 4);
            s.outskirt(7, Tile::GrassTall, 14);
        }
        "wagon" => {
            s.lay(x - 8, y, 17, 1, Tile::Track);
            if rolled {
                s.lay(x - 8, y - 1, 17, 1, Tile::Rail);
            }
            s.pad(9, 5, Tile::Dirt);
            s.put(d.p.minecart, -1, -2);
            s.put(d.p.crate_, 4, 2);
            s.outskirt(7, Tile::Bush, 12);
        }
        "cart" | "camp" => {
            // Somebody stopped here for a night. The ring of stones is cold and the cart never went on.
            s.pad(11, 9, Tile::Dirt);
            s.put(d.p.road_cart, 3, -2);
            s.put(d.p.campfire_cold, 0, 0);
            s.put(d.p.crate_, -3, -1);
            s.put(d.p.barrel, -1, 3);
            s.outskirt(7, Tile::Bush, 14);
        }
        "hollow_tree" => {
            // Thicket first, then the one tree you can get inside.
            s.outskirt(8, Tile::Tree, 26);
            // An anchor keeps the old small tree: something is hung in it, and a thicket seven deep
            // leaves nowhere to hang it.
            if rolled {
                s.lay(x - 3, y - 3, 7, 6, Tile::Tree);
            } else {
                s.lay(x - 2, y - 2, 5, 4, Tile::Tree);
            }
            s.lay(x - 1, y - 1, 3, 3, Tile::Dirt);
            s.lay(x - 1, y + 1, 3, 3, Tile::Dirt);
        }
        _ => {
            s.pad(7, 7, Tile::Dirt);
            s.outskirt(6, Tile::Bush, 10);
        }
    }
    // Whatever was drawn, the place can be stood at, and walked out of: a three-wide way is opened
    // south from the mark past the last of the dressing, so the shape can never be a trap.
    for oy in 2..=14 {
        for ox in -1..=1 {
            let (cx, cy) = (x + ox, y + oy);
            if c.k.get(cx, cy) == Tile::Water || c.k.is_claimed(cx, cy) {
                continue;
            }
            if c.k.solid(cx, cy) {
                c.k.set(cx, cy, Tile::Dirt);
            }
        }
    }
    // A place the story needs also gets room: the text says "at the well", not "somewhere in the
    // thicket near the well".
    if !rolled {
        clear_round(c, x, y + 3);
        way_out(c, x, y + 3);
    }
    let (mx, my) = if rolled { (x, y + 3) } else { stand(c, x, y + 3) };
    c.k.mark(name, mx, my, Some(Facing::South));
}

/// A needed place's way out: down the distance field to the nearest road or footpath, a crag in
/// the way is broken to dirt three cells wide, and where the place itself stands in water (a
/// marsh the land stage drew as pools), the water on the way is decked in planks. A lamp post the
/// story needs stood in a pocket of crag on a seed in thirty, and a cottage in the Sallow's pools
/// on another; the TypeScript re-rolled those counties. Broken here, before the rows go down, the
/// place's things find open ground about it; the cut at the end of the build
/// (`finish::cut_through`) breaks crag too, for whatever is still shut in, but never decks water.
fn way_out(c: &mut County<'_>, x: i32, y: i32) {
    let planks = c.k.get(x, y) == Tile::Water;
    for (lx, ly) in downhill(&c.country.d_road, x, y) {
        for (cx, cy) in Rect::new(lx - 1, ly - 1, 3, 3).cells() {
            if in_box(c, cx, cy) {
                continue;
            }
            match c.k.get(cx, cy) {
                Tile::Cliff => c.k.set(cx, cy, Tile::Dirt),
                Tile::Water if planks => c.k.set(cx, cy, Tile::Boardwalk),
                _ => {}
            }
        }
    }
}

/// Where a needed place's mark stands: three cells south of its middle, or, where that is water or
/// solid (a cottage on a river bank), the nearest open cell to it, ring by ring out to four. The
/// TypeScript set the mark in the river and re-rolled the county.
fn stand(c: &County<'_>, x: i32, y: i32) -> (i32, i32) {
    let open = |x: i32, y: i32| !c.k.solid(x, y) && !in_box(c, x, y);
    for r in 0..=4i32 {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox.abs().max(oy.abs()) == r && open(x + ox, y + oy) {
                    return (x + ox, y + oy);
                }
            }
        }
    }
    (x, y)
}

/// Open ground round a needed place's mark (`clearing` in `county.ts`): trees, bushes, outcrops
/// and rubble give way; water, buildings and a set place's box do not.
fn clear_round(c: &mut County<'_>, x: i32, y: i32) {
    for oy in -5..=5 {
        for ox in -7..=7 {
            let (cx, cy) = (x + ox, y + oy);
            if matches!(c.k.get(cx, cy), Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Cliff | Tile::Rubble)
                && !in_box(c, cx, cy)
            {
                c.k.set(cx, cy, Tile::Grass);
            }
        }
    }
}

/// The painters one small place is drawn with.
struct Small<'s, 'a> {
    c: &'s mut County<'a>,
    rng: &'s mut Sfc32,
    x: i32,
    y: i32,
    rolled: bool,
}

impl Small<'_, '_> {
    /// The ground a small place stands on: not a square (a square of cobble in a field reads as a
    /// tile), a blob `w x h` with a ragged edge and a scuffed apron of dirt round it.
    fn pad(&mut self, w: i32, h: i32, t: Tile) {
        let (x, y) = (self.x, self.y);
        let (jn, inn) = ((h + 1) / 2 + 2, (w + 1) / 2 + 2);
        for j in -jn..=jn {
            for i in -inn..=inn {
                let worn = self.c.k.get(x + i, y + j);
                if matches!(worn, Tile::Water | Tile::Road | Tile::Boardwalk) || in_box(self.c, x + i, y + j) {
                    continue;
                }
                // The middle is the surface, the rim is broken, and outside it the grass is worn.
                if ellipse_within(i, j, w, h, 70) || (ellipse_within(i, j, w, h, 100) && self.rng.chance(Permille(700)))
                {
                    self.c.k.set(x + i, y + j, t);
                } else if ellipse_within(i, j, w, h, 190) && self.rng.chance(Permille(450)) && worn != t {
                    self.c.k.set(x + i, y + j, Tile::Dirt);
                }
            }
        }
    }

    /// Structure: walls, fences, furrows. Never over a road, water or claimed ground.
    fn lay(&mut self, lx: i32, ly: i32, w: i32, h: i32, t: Tile) {
        for j in ly..ly + h {
            for i in lx..lx + w {
                let was = self.c.k.get(i, j);
                if matches!(was, Tile::Road | Tile::Boardwalk | Tile::Water) || self.c.k.is_claimed(i, j) {
                    continue;
                }
                self.c.k.set(i, j, t);
            }
        }
    }

    /// A prop at an offset from the middle, on a rolled place only (a needed place's things are the
    /// placement rows').
    fn put(&mut self, def: PropDefId, ox: i32, oy: i32) {
        let row = jane_data::catalog().story.prop(def);
        let (px, py) = (self.x + ox, self.y + oy);
        if self.rolled && self.c.k.fits(px, py, i32::from(row.w), i32::from(row.h), 0) {
            self.c.k.prop(None, def, px, py);
        }
    }

    /// The outskirt of a rolled place: the ring of stuff that gathered round the thing because
    /// people kept coming back. A solid thing only ever goes down with clear ground on all four
    /// sides: a hedge cannot grow itself, and a wall drawn by accident can shut a place off.
    fn outskirt(&mut self, r: i32, t: Tile, n: u32) {
        if !self.rolled {
            return;
        }
        let blocks = t.flags() & F_SOLID != 0;
        for _ in 0..n {
            let ox = self.rng.irandom(2 * r + 1) - r;
            let oy = self.rng.irandom(2 * r + 1) - r;
            if ox.abs() + oy.abs() < r - 1 {
                continue;
            }
            let (cx, cy) = (self.x + ox, self.y + oy);
            let was = self.c.k.get(cx, cy);
            if (was != Tile::Grass && was != Tile::Dirt) || in_box(self.c, cx, cy) {
                continue;
            }
            let k = &self.c.k;
            // Nor on claimed ground: a thing a placement row set down stands there, or a road's
            // margin. (The TypeScript grew a bush under a stake in the allotments.)
            if blocks
                && (k.is_claimed(cx, cy)
                    || k.solid(cx - 1, cy)
                    || k.solid(cx + 1, cy)
                    || k.solid(cx, cy - 1)
                    || k.solid(cx, cy + 1))
            {
                continue;
            }
            self.c.k.set(cx, cy, t);
        }
    }
}

/// A signpost says something. The seed's own roadside posts were drawn with nothing on them, and a
/// sign you cannot read is worse than no sign: it names the two nearest places (never somebody's
/// house, Julie's no more than anybody else's), which way, and how far as the crow flies.
fn signposts(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let post = defs().p.signpost;
    let sk = c.sk;
    let blank: Vec<(usize, i32, i32)> =
        c.k.blueprint()
            .props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.def == post && p.talk.is_none() && p.use_list.is_none())
            .map(|(i, p)| (i, i32::from(p.cell.x), i32::from(p.cell.y)))
            .collect();
    for (i, px, py) in blank {
        let mut near: Vec<(i64, u8, i32, i32)> = sk
            .sites
            .iter()
            .filter(|s| s.row != sk.named.julie_house)
            .map(|s| {
                let (dx, dy) = (centre(s.mx) - px, centre(s.my) - py);
                (i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy), s.row, dx, dy)
            })
            .collect();
        near.sort_by_key(|&(d2, row, _, _)| (d2, row));
        if near.is_empty() {
            continue;
        }
        let words: Vec<String> = near
            .iter()
            .take(2)
            .map(|&(d2, row, dx, dy)| {
                let tenths = i64::from(isqrt((d2 * 100) as u64));
                let name = cat.text(sk.site(row).def.name).to_uppercase();
                format!("{name}, {}, {}", compass(dx, dy), distance_words(tenths))
            })
            .collect();
        let text = c.k.text(&format!("{}.", words.join(". ")));
        let read = c.k.list(vec![Action::Read(text)]);
        let label = c.k.text("A signpost");
        let p = &mut c.k.props_mut()[i];
        p.label.get_or_insert(label);
        p.use_list = Some(read);
    }
}
