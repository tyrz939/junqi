//! The builder's kit: a tile canvas plus spawn lists that a zone builder paints with and hands
//! back as a [`Blueprint`]. Motifs are painters here, not rooms (WORLDGEN.md §2: "draw a fence
//! ring around a lot, do not place 400 fences one by one"). Carries `jane/src/world/kit.ts`, with
//! PORT.md's corrections:
//!
//! - **Dice.** No stream in hand and no string-named `within`: a stage asks for its own
//!   [`Kit::dice`] with its [`Step`] and the row or cell it throws for, and passes the `Sfc32` to
//!   whatever it paints with (PORT.md §6.a).
//! - **Names by place.** Anonymous things are named by where they stand (`county_rock_812_40`),
//!   never by how many came before them, so one more lamp by the rail renames no sheep. Such a
//!   name is a `Key::Local` interned into the blueprint; a content name is `Key::Name` and the
//!   caller looks its id up once.
//! - **One `Claims` grid** in place of the TypeScript's three footprint sources (PORT.md §6.e),
//!   and prop footprints from the catalog's prop rows rather than a table built at run time.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use jane_core::action::{Action, Facing, ListRef, TextRef};
use jane_core::blueprint::{Mark, PropSpawn, StoryPlace, UnitSpawn, Waypoint};
use jane_core::num::{Permille, div_floor};
use jane_core::tile::F_SOLID;
use jane_core::{
    Blueprint, Cell, Grid, Key, Lookup, Material, PropDefId, Rect, Sfc32, StoryId, Tile, UnitDefId, ZoneId,
};

use crate::steps::{Step, dice};

/// Cells a builder has spoken for: a prop's footprint, a mark's standing room, a set place's
/// ground, a road's margin. Scatter never lands on a claimed cell. Outside the grid counts as
/// claimed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claims {
    grid: Grid<bool>,
}

impl Claims {
    pub fn new(w: u32, h: u32) -> Self {
        Self { grid: Grid::new(w, h, false) }
    }

    /// Claim every cell of `r` that is inside the grid.
    pub fn claim(&mut self, r: Rect) {
        self.grid.fill_rect(r, true);
    }

    pub fn is_claimed(&self, x: i32, y: i32) -> bool {
        self.grid.read(x, y, true)
    }
}

/// A zone under construction. See the module docs.
#[derive(Debug)]
pub struct Kit {
    seed: u32,
    attempt: u8,
    bp: Blueprint,
    claims: Claims,
    /// Name anonymous things by where they stand (the county) rather than by a running count.
    keys_by_place: bool,
    anon: u32,
    /// `local_names` by string, so interning costs one lookup rather than a scan of every name.
    names: Lookup<String, Key>,
    texts: Lookup<String, TextRef>,
}

/// A prop row with nothing set but where it is and what it is.
pub fn prop_spawn(key: Key, def: PropDefId, cell: Cell) -> PropSpawn {
    PropSpawn {
        key,
        def,
        cell,
        locked: false,
        key_tag: None,
        hidden: false,
        on: false,
        to: None,
        loot: Vec::new(),
        use_list: None,
        release: None,
        needs: Vec::new(),
        talk: None,
        label: None,
        night_lock: None,
        under: None,
        under_when: None,
    }
}

/// A cell from builder coordinates. Builders only place inside their grid, which is at most
/// 2000 cells a side.
pub fn cell(x: i32, y: i32) -> Cell {
    debug_assert!((0..=i32::from(u16::MAX)).contains(&x) && (0..=i32::from(u16::MAX)).contains(&y), "({x}, {y})");
    Cell::new(x as u16, y as u16)
}

/// `Math.round(n / d)` for `d > 0`: half rounds up, toward positive infinity, as JavaScript does.
pub const fn js_round(n: i64, d: i64) -> i64 {
    div_floor(2 * n + d, 2 * d)
}

/// The centre line of a wiggly stroke through `points`: every sixth step it drifts by up to one
/// cell sideways, never more than `wobble` off its course. Each segment is walked in
/// `max(|dx|, |dy|)` steps, rounded as JavaScript rounds.
pub fn stroke_line(rng: &mut Sfc32, points: &[(i32, i32)], wobble: i32) -> Vec<(i32, i32)> {
    let mut line = Vec::new();
    for seg in points.windows(2) {
        let ((ax, ay), (bx, by)) = (seg[0], seg[1]);
        let steps = (bx - ax).abs().max((by - ay).abs());
        let horizontal = (bx - ax).abs() >= (by - ay).abs();
        let mut drift = 0;
        for s in 0..=steps {
            if wobble > 0 && s % 6 == 0 {
                drift = (drift + rng.range(-1, 1)).clamp(-wobble, wobble);
            }
            let along = |a: i32, b: i32| {
                if steps == 0 { a } else { a + js_round(i64::from(b - a) * i64::from(s), i64::from(steps)) as i32 }
            };
            let x = along(ax, bx) + if horizontal { 0 } else { drift };
            let y = along(ay, by) + if horizontal { drift } else { 0 };
            line.push((x, y));
        }
    }
    line
}

impl Kit {
    /// A `w x h` canvas of `fill` for `zone`, seed `seed`, attempt `attempt`. With `keys_by_place`,
    /// anonymous things are named by their cell (`county_rock_812_40`); without it, by a count.
    pub fn new(zone: ZoneId, w: u32, h: u32, seed: u32, attempt: u8, fill: Tile, keys_by_place: bool) -> Self {
        Self {
            seed,
            attempt,
            bp: Blueprint::new(zone, w, h, fill),
            claims: Claims::new(w, h),
            keys_by_place,
            anon: 0,
            names: Lookup::new(),
            texts: Lookup::new(),
        }
    }

    pub fn zone(&self) -> ZoneId {
        self.bp.zone
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn attempt(&self) -> u8 {
        self.attempt
    }

    pub fn w(&self) -> i32 {
        self.bp.w() as i32
    }

    pub fn h(&self) -> i32 {
        self.bp.h() as i32
    }

    /// What has been built so far.
    pub fn blueprint(&self) -> &Blueprint {
        &self.bp
    }

    // --- dice -------------------------------------------------------------------------------

    /// The dice of one step of this build at this kit's attempt, for row or cell `(a, b)`.
    pub fn dice(&self, step: Step, a: i32, b: i32) -> Sfc32 {
        dice(self.seed, self.bp.zone, step, self.attempt, a, b)
    }

    /// The dice of one step at another attempt: a county stage that refines skeleton data takes
    /// the skeleton's attempt (PORT.md §6.a).
    pub fn dice_at(&self, step: Step, attempt: u8, a: i32, b: i32) -> Sfc32 {
        dice(self.seed, self.bp.zone, step, attempt, a, b)
    }

    // --- names ------------------------------------------------------------------------------

    /// Intern a generator-made name: the same string gives the same `Key::Local`, as
    /// [`Blueprint::local`] does, at the cost of one lookup.
    pub fn local(&mut self, name: &str) -> Key {
        if let Some(&k) = self.names.get(&name.to_owned()) {
            return k;
        }
        self.bp.local_names.push(name.to_owned());
        let k = Key::Local(self.bp.local_names.len() as u32 - 1);
        self.names.insert(name.to_owned(), k);
        k
    }

    /// A generator-made name's string, if `key` is one.
    pub fn local_name(&self, key: Key) -> Option<&str> {
        match key {
            Key::Local(i) => self.bp.local_names.get(i as usize).map(String::as_str),
            Key::Name(_) => None,
        }
    }

    /// A name for something nobody named: by where it stands (and `_2`, `_3` for a second and
    /// third on the same cell), or by a count when the kit was not asked to key by place.
    fn anon_key(&mut self, def: &str, x: i32, y: i32) -> Key {
        let zone = self.bp.zone.name();
        if !self.keys_by_place {
            let n = self.anon;
            self.anon += 1;
            return self.local(&format!("{zone}_{def}_{n}"));
        }
        let base = format!("{zone}_{def}_{x}_{y}");
        let mut name = base.clone();
        let mut n = 2;
        while self.names.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        self.local(&name)
    }

    /// A string the generator wrote (a sign's words, a label), interned.
    pub fn text(&mut self, s: &str) -> TextRef {
        if let Some(&t) = self.texts.get(&s.to_owned()) {
            return t;
        }
        let t = self.bp.push_text(s.to_owned());
        self.texts.insert(s.to_owned(), t);
        t
    }

    /// An action list the generator wrote.
    pub fn list(&mut self, actions: Vec<Action>) -> ListRef {
        self.bp.push_list(actions)
    }

    /// A condition list the generator wrote.
    pub fn conds(&mut self, conds: Vec<jane_core::Cond>) -> jane_core::CondsRef {
        self.bp.push_conds(conds)
    }

    // --- tiles ------------------------------------------------------------------------------

    pub fn inside(&self, x: i32, y: i32) -> bool {
        self.bp.tiles.inside(x, y)
    }

    /// The tile at `(x, y)`; `Void` outside.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Tile {
        self.bp.tiles.read(x, y, Tile::Void)
    }

    /// Set a tile; outside is a no-op.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, t: Tile) {
        self.bp.tiles.set(x, y, t);
    }

    pub fn fill(&mut self, r: Rect, t: Tile) {
        self.bp.tiles.fill_rect(r, t);
    }

    pub fn tiles(&self) -> &Grid<Tile> {
        &self.bp.tiles
    }

    /// The canvas itself, for a painter that walks every cell.
    pub fn tiles_mut(&mut self) -> &mut Grid<Tile> {
        &mut self.bp.tiles
    }

    /// Whether the tile at `(x, y)` stops feet (outside does).
    #[inline]
    pub fn solid(&self, x: i32, y: i32) -> bool {
        self.get(x, y).flags() & F_SOLID != 0
    }

    /// Render-only material over the terrain of `r` (PORT.md §6.i).
    pub fn paint(&mut self, r: Rect, m: Material) {
        self.bp.paint.push((r, m));
    }

    /// Paint several rects of one material.
    pub fn paint_all(&mut self, rects: impl IntoIterator<Item = Rect>, m: Material) {
        self.bp.paint.extend(rects.into_iter().map(|r| (r, m)));
    }

    /// A thick wiggly stroke through `points`: a square brush `width` cells across along
    /// [`stroke_line`]'s centre line, which it returns (a joint appears twice). Used for roads,
    /// paths and rivers; see [`Kit::round_brush`] for a brush that does not swell on a diagonal.
    pub fn stroke(
        &mut self,
        rng: &mut Sfc32,
        points: &[(i32, i32)],
        width: i32,
        t: Tile,
        wobble: i32,
    ) -> Vec<(i32, i32)> {
        let line = stroke_line(rng, points, wobble);
        self.square_brush(&line, width, |_| Some(t));
        line
    }

    /// A square brush `width` cells across along a centre line. Every cell under it is offered to
    /// `paint` with its tile; `paint` returns the tile to set, or `None` to leave it.
    pub fn square_brush(&mut self, line: &[(i32, i32)], width: i32, mut paint: impl FnMut(Tile) -> Option<Tile>) {
        let half = width / 2;
        for &(x, y) in line {
            for cy in y - half..y - half + width {
                for cx in x - half..x - half + width {
                    if !self.inside(cx, cy) {
                        continue;
                    }
                    if let Some(t) = paint(self.get(cx, cy)) {
                        self.set(cx, cy, t);
                    }
                }
            }
        }
    }

    /// A round brush along a centre line. At each point every cell within `half` cells on both
    /// axes is offered to `paint` with its tile and its squared distance from the point; `paint`
    /// returns the tile to set, or `None` to leave it. The disc is the caller's: compare the
    /// squared distance, never a root.
    pub fn round_brush(&mut self, line: &[(i32, i32)], half: i32, mut paint: impl FnMut(Tile, i32) -> Option<Tile>) {
        for &(x, y) in line {
            for oy in -half..=half {
                for ox in -half..=half {
                    let (cx, cy) = (x + ox, y + oy);
                    if !self.inside(cx, cy) {
                        continue;
                    }
                    if let Some(t) = paint(self.get(cx, cy), ox * ox + oy * oy) {
                        self.set(cx, cy, t);
                    }
                }
            }
        }
    }

    // --- claims -----------------------------------------------------------------------------

    pub fn claim(&mut self, r: Rect) {
        self.claims.claim(r);
    }

    pub fn is_claimed(&self, x: i32, y: i32) -> bool {
        self.claims.is_claimed(x, y)
    }

    pub fn claims(&self) -> &Claims {
        &self.claims
    }

    /// True when a `w x h` footprint at `(x, y)`, grown by `margin`, is open, unclaimed ground.
    pub fn fits(&self, x: i32, y: i32, w: i32, h: i32, margin: i32) -> bool {
        (y - margin..y + h + margin)
            .all(|j| (x - margin..x + w + margin).all(|i| !self.solid(i, j) && !self.is_claimed(i, j)))
    }

    /// An open spot for a `w x h` footprint inside `r`, tried `tries` times with `rng`, or `None`.
    pub fn spot(&self, rng: &mut Sfc32, r: Rect, w: i32, h: i32, margin: i32, tries: u32) -> Option<(i32, i32)> {
        for _ in 0..tries {
            let x = rng.range(r.x, r.x.max(r.x + r.w - w));
            let y = rng.range(r.y, r.y.max(r.y + r.h - h));
            if self.fits(x, y, w, h, margin) {
                return Some((x, y));
            }
        }
        None
    }

    // --- spawns -----------------------------------------------------------------------------

    /// A named arrival point. Its cell and the ring round it are claimed. A second mark of the
    /// same name moves the first and keeps its place in the order.
    pub fn mark(&mut self, name: Key, x: i32, y: i32, facing: Option<Facing>) {
        self.bp.marks.insert(name, Mark { cell: cell(x, y), facing });
        self.claim(Rect::new(x - 1, y - 1, 3, 3));
    }

    /// A named area.
    pub fn rect(&mut self, name: Key, r: Rect) {
        self.bp.rects.insert(name, r);
    }

    /// Where a story landed on this seed, or why it found no place (the county only).
    pub fn story(&mut self, id: StoryId, place: StoryPlace) {
        self.bp.stories.insert(id, place);
    }

    /// A prop at `(x, y)`, keyed `key` or by where it stands. Its footprint (the catalog's `w x h`)
    /// is claimed. Returns the row to set the rest on.
    pub fn prop(&mut self, key: Option<Key>, def: PropDefId, x: i32, y: i32) -> &mut PropSpawn {
        let row = jane_data::catalog().story.prop(def);
        let key = key.unwrap_or_else(|| self.anon_key(row.id, x, y));
        self.claim(Rect::new(x, y, i32::from(row.w), i32::from(row.h)));
        self.bp.props.push(prop_spawn(key, def, cell(x, y)));
        self.bp.props.last_mut().expect("just pushed")
    }

    /// Keep only the props `keep` says to.
    pub fn retain_props(&mut self, keep: impl FnMut(&PropSpawn) -> bool) {
        self.bp.props.retain(keep);
    }

    /// The props placed so far, to change a field on one (an `edit` row, a thing moved beside
    /// another). A prop moved this way claims its new footprint through [`Kit::claim`].
    pub fn props_mut(&mut self) -> &mut [PropSpawn] {
        &mut self.bp.props
    }

    /// The units placed so far, to change a field on one.
    pub fn units_mut(&mut self) -> &mut [UnitSpawn] {
        &mut self.bp.units
    }

    /// Keep only the units `keep` says to.
    pub fn retain_units(&mut self, keep: impl FnMut(&UnitSpawn) -> bool) {
        self.bp.units.retain(keep);
    }

    /// Keep only the marks `keep` says to, in their order.
    pub fn retain_marks(&mut self, mut keep: impl FnMut(&Key, &Mark) -> bool) {
        self.bp.marks.retain(|k, m| keep(k, m));
    }

    /// A unit at `(x, y)`, keyed `key` or by where it stands. Its cell is claimed.
    pub fn unit(&mut self, key: Option<Key>, def: UnitDefId, x: i32, y: i32, patrol: Vec<Waypoint>) -> &mut UnitSpawn {
        let id = jane_data::catalog().combat.unit(def).id;
        let key = key.unwrap_or_else(|| self.anon_key(id, x, y));
        self.claim(Rect::new(x, y, 1, 1));
        self.bp.units.push(UnitSpawn { key, def, cell: cell(x, y), facing: None, patrol, phase: 0 });
        self.bp.units.last_mut().expect("just pushed")
    }

    /// The finished blueprint: `name` is what the zone is called, `attempts` is this kit's
    /// attempt plus one.
    pub fn done(mut self, name: &str, indoor: bool, ambient: Permille) -> Blueprint {
        self.bp.name = self.text(name);
        self.bp.indoor = indoor;
        self.bp.ambient = ambient;
        self.bp.attempts = self.attempt.saturating_add(1);
        settle_units(&mut self.bp);
        self.bp
    }
}

/// Every unit that would start in terrain or a solid prop's cells (a gate while it is locked; a
/// hidden prop too, since it may show), or boxed in with no open cell beside it, moved to the
/// nearest cell that is open, has an open cell beside it and holds no other unit: found by a
/// flood from where it stood through what props stand on, never through terrain, so it stays in
/// its room. One with nowhere to go is left. The owner's playtest: "a rat spawned inside the
/// chest in the basement so it couldn't run anywhere". Every builder's last step.
pub fn settle_units(bp: &mut Blueprint) {
    use alloc::collections::{BTreeSet, VecDeque};
    const REACH: usize = 4096;
    const SIDES: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let cat = jane_data::catalog();
    let mut props = BTreeSet::new();
    for p in &bp.props {
        let def = cat.story.prop(p.def);
        if def.solid && (!def.gate || p.locked) {
            props.extend(def.solid_rect(i32::from(p.cell.x), i32::from(p.cell.y)).cells());
        }
    }
    let tiles = &bp.tiles;
    let terrain = |x: i32, y: i32| !tiles.inside(x, y) || tiles.read(x, y, Tile::Void).flags() & F_SOLID != 0;
    let open = |x: i32, y: i32| !terrain(x, y) && !props.contains(&(x, y));
    let roomy = |x: i32, y: i32| open(x, y) && SIDES.iter().any(|(dx, dy)| open(x + dx, y + dy));
    let mut at: Vec<(i32, i32)> = bp.units.iter().map(|u| (i32::from(u.cell.x), i32::from(u.cell.y))).collect();
    for i in 0..at.len() {
        let start = at[i];
        if roomy(start.0, start.1) {
            continue;
        }
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        let mut to = None;
        while let Some((x, y)) = queue.pop_front() {
            if roomy(x, y) && !at.contains(&(x, y)) {
                to = Some((x, y));
                break;
            }
            if seen.len() > REACH {
                break;
            }
            for (dx, dy) in SIDES {
                let n = (x + dx, y + dy);
                if !terrain(n.0, n.1) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        if let Some((x, y)) = to {
            at[i] = (x, y);
            bp.units[i].cell = cell(x, y);
        }
    }
}

// --- motifs ---------------------------------------------------------------------------------

impl Kit {
    /// Props of `def` at even spacing along the inside of a room's north and south walls: every
    /// `every` cells from two in along the top row, and from two and a half-step in along the
    /// bottom row, skipping a cell that is claimed or solid. `r` is the room's floor.
    pub fn torch_run(&mut self, r: Rect, every: i32, def: PropDefId) {
        let step = usize::try_from(every.max(1)).unwrap_or(1);
        for (x0, y) in [(r.x + 2, r.y), (r.x + 2 + every / 2, r.y + r.h - 1)] {
            for x in (x0..r.x + r.w - 1).step_by(step) {
                if !self.is_claimed(x, y) && !self.solid(x, y) {
                    self.prop(None, def, x, y);
                }
            }
        }
    }

    /// Up to `count` props of `def` piled into `r`: each at an open spot for its footprint (the
    /// catalog's `w x h`) found with `rng`; one with no spot is left out.
    pub fn pile(&mut self, rng: &mut Sfc32, r: Rect, def: PropDefId, count: u32) {
        let row = jane_data::catalog().story.prop(def);
        let (w, h) = (i32::from(row.w), i32::from(row.h));
        for _ in 0..count {
            if let Some((x, y)) = self.spot(rng, r, w, h, 0, SPOT_TRIES) {
                self.prop(None, def, x, y);
            }
        }
    }
}

/// How many places [`Kit::spot`] tries before a painter gives a thing up (the TypeScript's default).
pub const SPOT_TRIES: u32 = 60;

#[cfg(test)]
mod tests {
    use super::*;

    fn kit() -> Kit {
        Kit::new(ZoneId::County, 40, 30, 7, 0, Tile::Grass, true)
    }

    #[test]
    fn js_round_rounds_half_up() {
        assert_eq!(js_round(5, 2), 3);
        assert_eq!(js_round(-5, 2), -2);
        assert_eq!(js_round(7, 3), 2);
        assert_eq!(js_round(-7, 3), -2);
        assert_eq!(js_round(0, 9), 0);
    }

    #[test]
    fn claims_and_fits() {
        let mut k = kit();
        assert!(k.fits(2, 2, 3, 3, 1));
        k.claim(Rect::new(4, 4, 1, 1));
        assert!(!k.fits(2, 2, 3, 3, 0));
        assert!(!k.fits(-1, 0, 2, 2, 0), "outside is claimed");
        k.set(20, 20, Tile::Tree);
        assert!(!k.fits(19, 19, 3, 3, 0));
        assert!(k.fits(16, 16, 3, 3, 0));
    }

    #[test]
    fn names_by_place_are_unique_and_interned() {
        let mut k = kit();
        let a = k.local("county_rock_3_4");
        assert_eq!(k.local("county_rock_3_4"), a);
        assert_eq!(k.anon_key("rock", 3, 4), k.local("county_rock_3_4_2"));
        assert_eq!(k.local_name(a), Some("county_rock_3_4"));
        let t = k.text("A sign");
        assert_eq!(k.text("A sign"), t);
        let mut counted = Kit::new(ZoneId::Mine, 4, 4, 1, 0, Tile::CaveFloor, false);
        let a = counted.anon_key("torch", 1, 1);
        assert_ne!(counted.anon_key("torch", 1, 1), a);
    }

    #[test]
    fn a_stroke_is_connected_and_a_round_brush_is_round() {
        let mut k = kit();
        let mut rng = k.dice(Step::CountyRoad, 0, 0);
        let line = k.stroke(&mut rng, &[(2, 2), (30, 20)], 1, Tile::Road, 1);
        for w in line.windows(2) {
            assert!((w[0].0 - w[1].0).abs() <= 2 && (w[0].1 - w[1].1).abs() <= 2, "{w:?}");
        }
        let mut k = kit();
        k.round_brush(&[(10, 10)], 3, |_, d| (d <= 4).then_some(Tile::Dirt));
        assert_eq!(k.get(12, 10), Tile::Dirt);
        assert_eq!(k.get(12, 12), Tile::Grass);
        assert_eq!(k.get(13, 10), Tile::Grass);
    }

    #[test]
    fn props_claim_their_footprint_and_done_counts_attempts() {
        let cat = jane_data::catalog();
        let sign = cat.story.prop_id("sign").expect("a sign row");
        let mut k = Kit::new(ZoneId::County, 40, 30, 7, 2, Tile::Grass, true);
        let key = k.prop(None, sign, 5, 6).key;
        assert_eq!(k.local_name(key), Some("county_sign_5_6"));
        let row = cat.story.prop(sign);
        assert!(k.is_claimed(5 + i32::from(row.w) - 1, 6 + i32::from(row.h) - 1));
        k.mark(Key::Local(0), 20, 20, None);
        assert!(k.is_claimed(21, 21));
        let bp = k.done("Castle", false, Permille::ONE);
        assert_eq!(bp.attempts, 3);
        assert_eq!(bp.text(bp.name), Some("Castle"));
        assert_eq!(bp.props.len(), 1);
    }
}

#[cfg(test)]
mod motif_tests {
    use super::*;

    fn room() -> (Kit, Rect) {
        let mut k = Kit::new(ZoneId::Cellar, 30, 20, 3, 0, Tile::Wall, false);
        let r = Rect::new(2, 2, 20, 10);
        k.fill(r, Tile::Floor);
        (k, r)
    }

    #[test]
    fn a_torch_run_lines_the_north_and_south_walls_and_skips_what_is_taken() {
        let torch = jane_data::catalog().story.prop_id("torch").expect("a torch row");
        let (mut k, r) = room();
        k.claim(Rect::new(13, 2, 1, 1));
        k.torch_run(r, 9, torch);
        let at: Vec<(u16, u16)> = k.blueprint().props.iter().map(|p| (p.cell.x, p.cell.y)).collect();
        // North from x 4 every 9 (13 is claimed); south from x 8 every 9, below x 21.
        assert_eq!(at, vec![(4, 2), (8, 11), (17, 11)]);
    }

    #[test]
    fn a_pile_lands_on_open_ground_without_overlap() {
        let barrel = jane_data::catalog().story.prop_id("barrel").expect("a barrel row");
        let (mut k, r) = room();
        let mut rng = k.dice(Step::IntCellar, 0, 0);
        k.pile(&mut rng, Rect::new(r.x, r.y, 8, 6), barrel, 3);
        let props = &k.blueprint().props;
        assert_eq!(props.len(), 3, "room for three");
        for (i, a) in props.iter().enumerate() {
            assert!(Rect::new(2, 2, 8, 6).contains(i32::from(a.cell.x), i32::from(a.cell.y)));
            for b in &props[i + 1..] {
                let (dx, dy) = (i32::from(a.cell.x) - i32::from(b.cell.x), i32::from(a.cell.y) - i32::from(b.cell.y));
                assert!(dx.abs() >= 2 || dy.abs() >= 2, "{a:?} {b:?}");
            }
        }
    }
}
