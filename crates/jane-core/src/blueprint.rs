//! The Blueprint: what a zone builder returns and the sim builds a zone from (PORT.md §4:
//! `world` and `sim` share nothing but core types and this). A pure function of
//! `(zone, seed)`, never saved; the save holds the seed and what changed since.
//!
//! Builders address nothing by coordinate outside themselves: story, triggers, dialogue and
//! travel name keys, marks and rects. That contract lets geometry roll per seed while the
//! story stays fixed.
//!
//! **Rule:** a change here is a `core` branch first (PORT.md §11).

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::misc::IndexMap;

use crate::action::{Action, Cond, CondsRef, Facing, ListRef, NamesRef, NightLock, Stack, TextRef};
use crate::grid::{Cell, Grid, Rect};
use crate::ids::{DialogueId, Key, PropDefId, StoryId, UnitDefId, ZoneId};
use crate::num::{Permille, Tick};
use crate::plane::Plane;
use crate::rare::{Rare, Thin};
use crate::tile::{FLAT_LEVEL, Material, Tile};

/// Candidates a zone rolls for one seed before it gives up; a generated dungeon spends the
/// last on its hand-placed fallback, so it never does.
pub const ZONE_ATTEMPTS: u8 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mark {
    pub cell: Cell,
    pub facing: Option<Facing>,
}

/// A patrol waypoint; `dwell` is how long to stand there before walking on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Waypoint {
    pub cell: Cell,
    pub dwell: Tick,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnitSpawn {
    pub key: Key,
    pub def: UnitDefId,
    pub cell: Cell,
    pub facing: Option<Facing>,
    pub patrol: Vec<Waypoint>,
    /// The threat (1..=6) of the ground it stands on; 0 = the row as written.
    pub phase: u8,
}

/// Where a door leads.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Door {
    pub zone: ZoneId,
    pub mark: Key,
}

/// A prop row: what every row has inline, the rest ([`PropRare`]) out of line (PORT.md §13.3,
/// phase 3: one row in eight sets any of it, and inline it was 128 bytes a row). The rare fields
/// read and write as if they were the row's own (`row.loot`, `row.talk = ..`), through
/// [`Deref`](core::ops::Deref); a row that never sets one holds none of them.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropSpawn {
    pub key: Key,
    pub def: PropDefId,
    pub cell: Cell,
    pub locked: bool,
    pub hidden: bool,
    pub on: bool,
    /// The rare fields; reach them through the row (`row.loot`), not this.
    pub rare: Rare<PropRare>,
}

impl PropSpawn {
    /// A row with nothing set but what and where it is.
    pub const fn new(key: Key, def: PropDefId, cell: Cell) -> Self {
        Self { key, def, cell, locked: false, hidden: false, on: false, rare: Rare::empty() }
    }
}

impl core::ops::Deref for PropSpawn {
    type Target = PropRare;
    #[inline]
    fn deref(&self) -> &PropRare {
        self.rare.get()
    }
}

impl core::ops::DerefMut for PropSpawn {
    #[inline]
    fn deref_mut(&mut self) -> &mut PropRare {
        self.rare.get_mut()
    }
}

/// The fields of a prop row that few rows set ([`PropSpawn`]).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropRare {
    pub key_tag: Option<Key>,
    pub to: Option<Door>,
    pub loot: Vec<Stack>,
    pub use_list: Option<ListRef>,
    /// Pressure plates: runs when the last thing steps off.
    pub release: Option<ListRef>,
    /// Materials a world verb (Repair) consumes from the caster's bag.
    pub needs: Vec<Stack>,
    pub talk: Option<DialogueId>,
    pub label: Option<TextRef>,
    /// This door is not answered at some hours, and this is what it says instead.
    pub night_lock: Option<NightLock>,
    /// The hidden prop this one lies on, shown when this one is pushed off it...
    pub under: Option<Key>,
    /// ...and only while these hold.
    pub under_when: Option<CondsRef>,
}

impl Thin for PropRare {
    fn none() -> &'static Self {
        static NONE: PropRare = PropRare::NONE;
        &NONE
    }
}

impl PropRare {
    /// Nothing set.
    pub const NONE: PropRare = PropRare {
        key_tag: None,
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
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TriggerMode {
    /// Fires on walking in.
    Enter,
    /// Fires once inside and `when` holds.
    While,
}

/// A trigger: a rect, when it fires, what it does, and what undoes it when the party dies.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trigger {
    pub rect: Key,
    pub mode: TriggerMode,
    pub once: bool,
    pub when: Option<CondsRef>,
    pub actions: ListRef,
    /// Run when the player dies after this fired; the trigger then re-arms.
    pub reset: Option<ListRef>,
}

/// Where a story found its place on this seed, or why it found none.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StoryPlace {
    Placed { kind: Key, name: TextRef, bounds: Rect, path: Vec<Cell> },
    Skipped(TextRef),
}

/// A named patch of the skeleton as laid in a zone (the county's allotments, the Top Field): its
/// name and the square it covers. The ecology keeps a pressure per area, in this list's order
/// (ARCHITECTURE.md §4.6.c); a unit belongs to the first area whose rect holds its home cell.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Area {
    pub name: Key,
    pub rect: Rect,
}

/// A span's index in its blueprint's [`Blueprint::spans`] (at most [`SPANS_MAX`]).
pub type SpanIx = u8;

/// Spans a zone may hold: a bit each in the zone's broken set (a `u64`).
pub const SPANS_MAX: usize = 64;

/// A span (MAP.md §2.5): a bridge or walkway over walkable ground, so two surfaces lie on its
/// cells. The cells of `rect` keep their own tiles and levels (the towpath, the rails: the ground
/// under it); the **deck** is a second layer over them at `deck_level`, walked along `along_x`
/// (east-west) or along y only, its long sides parapets. Its **ends** are the rows of cells just
/// beyond the rect along its axis, across its width ([`Span::ends`]): ground at `deck_level`
/// from which feet step onto the deck. The rect's own end rows are solid to the ground under it
/// (abutments, piers: a well-formed span's builder makes them so), so nothing walks up onto the
/// deck from beneath. A `broken` span has no deck (the collapsed footbridge, a verb gate) until a
/// consequence mends it; that bit is the zone's state in play, this its first value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    pub rect: Rect,
    pub along_x: bool,
    pub deck_level: u8,
    pub broken: bool,
}

impl Span {
    /// Its length along its axis, in cells.
    pub const fn length(&self) -> i32 {
        if self.along_x { self.rect.w } else { self.rect.h }
    }

    /// The two ends: the row (or column) of cells just beyond each end of the rect, across its
    /// width, from the low end.
    pub const fn ends(&self) -> [Rect; 2] {
        let r = self.rect;
        if self.along_x {
            [Rect::new(r.x - 1, r.y, 1, r.h), Rect::new(r.x + r.w, r.y, 1, r.h)]
        } else {
            [Rect::new(r.x, r.y - 1, r.w, 1), Rect::new(r.x, r.y + r.h, r.w, 1)]
        }
    }

    /// Which end (0 low, 1 high) cell `(x, y)` is in, if either.
    pub fn end_of(&self, x: i32, y: i32) -> Option<usize> {
        self.ends().iter().position(|e| e.contains(x, y))
    }

    /// A cell beside the deck: across its axis just outside the rect, along its length. A
    /// parapet to anything on the deck.
    pub const fn beside(&self, x: i32, y: i32) -> bool {
        let r = self.rect;
        if self.along_x {
            x >= r.x && x < r.x + r.w && (y == r.y - 1 || y == r.y + r.h)
        } else {
            y >= r.y && y < r.y + r.h && (x == r.x - 1 || x == r.x + r.w)
        }
    }

    /// The cell one step from `(x, y)` along the axis toward end `e`.
    pub const fn toward(&self, (x, y): (i32, i32), e: usize) -> (i32, i32) {
        let d = if e == 0 { -1 } else { 1 };
        if self.along_x { (x + d, y) } else { (x, y + d) }
    }
}

/// A zone as built.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blueprint {
    pub zone: ZoneId,
    pub name: TextRef,
    pub tiles: Grid<Tile>,
    pub units: Vec<UnitSpawn>,
    pub props: Vec<PropSpawn>,
    pub marks: IndexMap<Key, Mark>,
    pub rects: IndexMap<Key, Rect>,
    /// Interiors ignore the day clock and keep a fine fog.
    pub indoor: bool,
    /// Ambient light for interiors.
    pub ambient: Permille,
    /// Candidates rolled; more than 1 means one failed validation and was re-rolled.
    pub attempts: u8,
    /// Trigger rows the builder wrote itself, by name; merged with the catalog's for this zone.
    pub triggers: IndexMap<Key, Trigger>,
    /// The county only: where each story landed.
    pub stories: IndexMap<StoryId, StoryPlace>,
    /// Render-only material over terrain, in paint order (PORT.md §6.i).
    pub paint: Vec<(Rect, Material)>,
    /// Action lists `ListRef::Blueprint` indexes.
    pub lists: Vec<Vec<Action>>,
    /// Condition lists `CondsRef::Blueprint` indexes.
    pub conds: Vec<Vec<Cond>>,
    /// Name lists `NamesRef::Blueprint` indexes.
    pub name_lists: Vec<Vec<Key>>,
    /// Strings the generator wrote: a dungeon's name, a story's place name.
    pub texts: Vec<String>,
    /// Names the generator made: `Key::Local(i)` is `local_names[i]`.
    pub local_names: crate::names::Names,
    /// The county only: the skeleton's patches as placed, in the skeleton's order (§4.6.c).
    pub areas: Vec<Area>,
    /// The county only: the region under each part of it, whose sky rains there (§4.6.b).
    /// Empty for every other zone, which is under its zone's sky.
    pub regions: RegionMap,
    /// A generated dungeon only: where nothing follows her (WoW's instance edge, PLAN.md §2.6
    /// *Leash*): its rest room's floor and the threshold of each way out of the zone. A foe whose
    /// quarry stands in one, or that would itself step in, lets her go and evades home.
    pub sanctuary: Vec<Rect>,
    /// The county only: the shield's reach (NIGHT.md §5.4, WORLDGEN): `(prop row, stage)` for
    /// each lamp that goes wrong (cold, unsafe) from that night stage on, by row. Every lamp not
    /// listed is kept. Empty until the shield's ranks are generated (NIGHT.md §9 R4); hashed only
    /// when it holds a rank, so the blueprint hashes are unchanged until then.
    pub shield: Vec<(u16, u8)>,
    /// The tiles and paint packed in chunks ([`Blueprint::pack`], PORT.md §13.3): when set, `tiles`
    /// is hollow (its size only) and `paint` empty, and the tiles read through
    /// [`Blueprint::tile`]. `None` as built.
    pub packed: Option<alloc::boxed::Box<Packed>>,
    /// The ground's level under each cell, 0 to 3 (MAP.md §2.2), packed by chunk: most chunks
    /// hold one level. `None`: flat, every cell at [`FLAT_LEVEL`], which is every zone built so
    /// far. Never written in play and never saved (the seed's, like the terrain); hashed only
    /// when set, so a blueprint without it hashes as it did before it existed.
    pub level: Option<Plane>,
    /// Bridges and walkways over walkable ground (MAP.md §2.5), at most [`SPANS_MAX`]. Empty in
    /// every zone built so far; hashed only when there are any.
    pub spans: Vec<Span>,
}

/// A blueprint's tiles and paint packed in chunks (PORT.md §13.3, [`crate::plane`]'s chunk API):
/// what a console holds in place of the byte-a-cell grids.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Packed {
    /// Tile ids.
    pub tiles: Plane,
    /// The paint as laid, a cell at a time: 0 for none, else the [`Material`]'s index plus 1 (the
    /// last rect over a cell wins, as in paint order).
    pub paint: Plane,
}

/// Paint packed as [`Blueprint::pack`] packs it: `rects` laid in order a band of rows at a time
/// (the last over a cell wins), then `last(x, y)` over every cell where it names a material (paint
/// laid after all the rects, a cell at a time). Never a whole grid.
pub fn pack_paint(w: u32, h: u32, rects: &[(Rect, Material)], last: impl Fn(i32, i32) -> Option<Material>) -> Plane {
    pack_paint_rows(w, h, rects, |y, lay| {
        for x in 0..w as i32 {
            if let Some(m) = last(x, y) {
                lay(x, m);
            }
        }
    })
}

/// [`pack_paint`] with the last paint asked a row at a time: `last(y, lay)` calls `lay(x, m)` for
/// each cell of row `y` it paints, in any order (a later call for a cell wins). For a caller that
/// can skip a row's bare stretches without asking each cell.
pub fn pack_paint_rows(
    w: u32,
    h: u32,
    rects: &[(Rect, Material)],
    mut last: impl FnMut(i32, &mut dyn FnMut(i32, Material)),
) -> Plane {
    Plane::pack_bands(w, h, |y0, rows, out| {
        let band = Rect::new(0, y0 as i32, w as i32, rows as i32);
        for &(r, m) in rects {
            let Some(r) = r.intersect(band) else { continue };
            for y in r.y..r.bottom() {
                let row = (y - band.y) as usize * w as usize;
                out[row + r.x as usize..row + r.right() as usize].fill(m as u8 + 1);
            }
        }
        for y in band.y..band.bottom() {
            let row = &mut out[(y - band.y) as usize * w as usize..][..w as usize];
            last(y, &mut |x, m| row[x as usize] = m as u8 + 1);
        }
    })
}

/// Which region each part of a zone lies in, on a coarse grid: the county's is the skeleton's
/// macro grid (125 x 125, 16 cells to a macro cell). A byte is a region's index in region order
/// (`jane_data::Region`: 0 Lowfields, 1 Waters, 2 Works); core does not know the names. Empty
/// for a zone under one sky.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegionMap {
    /// Cells to a map cell, each way.
    pub scale: u16,
    pub w: u16,
    pub h: u16,
    /// Row-major, `w * h` bytes.
    pub cells: Vec<u8>,
}

impl RegionMap {
    /// A map of `w x h` map cells of `scale` cells each, all region `fill`.
    pub fn new(scale: u16, w: u16, h: u16, fill: u8) -> Self {
        Self { scale, w, h, cells: vec![fill; usize::from(w) * usize::from(h)] }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Set the region of map cell `(mx, my)`; outside is a no-op.
    pub fn set(&mut self, mx: u16, my: u16, region: u8) {
        if mx < self.w && my < self.h {
            self.cells[usize::from(my) * usize::from(self.w) + usize::from(mx)] = region;
        }
    }

    /// The region under cell `(x, y)`; `None` off the map or on an empty one.
    pub fn region_at(&self, x: i32, y: i32) -> Option<u8> {
        if self.scale == 0 || x < 0 || y < 0 {
            return None;
        }
        let s = i32::from(self.scale);
        let (mx, my) = (x / s, y / s);
        if mx >= i32::from(self.w) || my >= i32::from(self.h) {
            return None;
        }
        self.cells.get(my as usize * usize::from(self.w) + mx as usize).copied()
    }
}

impl Blueprint {
    /// Every list held at exactly its length (PORT.md §13.3): a finished blueprint is read, never
    /// grown, so the spare capacity its builder left is given back. Nothing it holds changes.
    pub fn shrink_to_fit(&mut self) {
        self.tiles.shrink_to_fit();
        self.units.shrink_to_fit();
        for u in &mut self.units {
            u.patrol.shrink_to_fit();
        }
        self.props.shrink_to_fit();
        for p in &mut self.props {
            p.rare.settle();
            if let Some(r) = p.rare.boxed_mut() {
                r.loot.shrink_to_fit();
                r.needs.shrink_to_fit();
            }
        }
        self.marks.shrink_to_fit();
        self.rects.shrink_to_fit();
        self.triggers.shrink_to_fit();
        self.stories.shrink_to_fit();
        for s in self.stories.values_mut() {
            if let StoryPlace::Placed { path, .. } = s {
                path.shrink_to_fit();
            }
        }
        self.paint.shrink_to_fit();
        self.lists.shrink_to_fit();
        self.lists.iter_mut().for_each(Vec::shrink_to_fit);
        self.conds.shrink_to_fit();
        self.conds.iter_mut().for_each(Vec::shrink_to_fit);
        self.name_lists.shrink_to_fit();
        self.name_lists.iter_mut().for_each(Vec::shrink_to_fit);
        self.texts.shrink_to_fit();
        self.texts.iter_mut().for_each(String::shrink_to_fit);
        self.local_names.shrink_to_fit();
        self.areas.shrink_to_fit();
        self.regions.cells.shrink_to_fit();
        self.sanctuary.shrink_to_fit();
        self.shield.shrink_to_fit();
    }

    /// Pack the tiles and paint in chunks ([`Packed`]) and let the grids go: the tiles then read
    /// through [`tile`](Self::tile) and [`tile_ix`](Self::tile_ix), the same tiles. The paint's
    /// order is folded into one material a cell, which is all a drawing of it reads. Hash a
    /// blueprint before packing it (`jane_world::hash` refuses a packed one). A no-op on a packed one.
    pub fn pack(&mut self) {
        if self.packed.is_some() {
            return;
        }
        // The tiles let go before the paint is laid out, so packing costs a plane, not a grid.
        let (w, h) = (self.w(), self.h());
        let cells = self.tiles.as_slice();
        let tiles = Plane::pack_by(w, h, |i| cells[i].id());
        self.pack_with_tiles(tiles);
    }

    /// [`pack`](Self::pack) with the tiles already packed (a builder's canvas packs its own): the
    /// grid, if any, let go and the paint packed beside `tiles`.
    pub fn pack_with_tiles(&mut self, tiles: Plane) {
        let (w, h) = (self.w(), self.h());
        assert_eq!((tiles.w(), tiles.h()), (w, h), "the tiles' plane is the zone's size");
        self.tiles = Grid::hollow(w, h);
        // The paint laid a band of rows at a time, in paint order (the last rect over a cell
        // wins), never as a whole grid.
        let rects = core::mem::take(&mut self.paint);
        let paint = pack_paint(w, h, &rects, |_, _| None);
        drop(rects);
        self.packed = Some(alloc::boxed::Box::new(Packed { tiles, paint }));
    }

    /// Packed already: `tiles` and `paint` as [`pack`](Self::pack) would make them (a builder that
    /// packs as it finishes); the grid and the paint's rects let go.
    pub fn set_packed(&mut self, tiles: Plane, paint: Plane) {
        let (w, h) = (self.w(), self.h());
        assert!((tiles.w(), tiles.h(), paint.w(), paint.h()) == (w, h, w, h), "planes the zone's size");
        self.tiles = Grid::hollow(w, h);
        self.paint = Vec::new();
        self.packed = Some(alloc::boxed::Box::new(Packed { tiles, paint }));
    }

    /// The tile at `(x, y)`, `Tile::Void` outside: from the grid, or the packed plane.
    #[inline]
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        match &self.packed {
            None => self.tiles.read(x, y, Tile::Void),
            Some(p) => Tile::from_id(p.tiles.read(x, y, 0)).unwrap_or(Tile::Void),
        }
    }

    /// The tile ids of `out.len()` cells of row `y` from `x0`, all inside the zone: a row at a
    /// time, through the grid or the packed tiles.
    pub fn tile_row(&self, x0: u32, y: u32, out: &mut [u8]) {
        match &self.packed {
            None => {
                let at = y as usize * self.w() as usize + x0 as usize;
                let n = out.len();
                for (o, t) in out.iter_mut().zip(&self.tiles.as_slice()[at..at + n]) {
                    *o = t.id();
                }
            }
            Some(p) => p.tiles.row(x0, y, out),
        }
    }

    /// The tile at in-grid cell `(x, y)`, whose index is `i`.
    #[inline]
    pub fn tile_in(&self, x: u32, y: u32, i: usize) -> Tile {
        match &self.packed {
            None => self.tiles.as_slice()[i],
            Some(p) => Tile::from_id(p.tiles.get(x, y)).unwrap_or(Tile::Void),
        }
    }

    /// The tile at in-grid cell index `i`.
    #[inline]
    pub fn tile_ix(&self, i: crate::grid::CellIx) -> Tile {
        let w = self.w();
        self.tile_in(i.0 % w, i.0 / w, i.0 as usize)
    }

    /// An empty zone of `w x h` cells of `fill`.
    pub fn new(zone: ZoneId, w: u32, h: u32, fill: Tile) -> Self {
        Self {
            zone,
            name: TextRef::Local(0),
            tiles: Grid::new(w, h, fill),
            units: Vec::new(),
            props: Vec::new(),
            marks: IndexMap::default(),
            rects: IndexMap::default(),
            indoor: false,
            ambient: Permille::ONE,
            attempts: 1,
            triggers: IndexMap::default(),
            stories: IndexMap::default(),
            paint: Vec::new(),
            lists: Vec::new(),
            conds: Vec::new(),
            name_lists: Vec::new(),
            texts: vec![String::new()],
            local_names: crate::names::Names::new(),
            areas: Vec::new(),
            regions: RegionMap::default(),
            sanctuary: Vec::new(),
            shield: Vec::new(),
            packed: None,
            level: None,
            spans: Vec::new(),
        }
    }

    /// The level of the ground at `(x, y)`: [`FLAT_LEVEL`] in a zone without a level plane, and
    /// outside it.
    #[inline]
    pub fn level_at(&self, x: i32, y: i32) -> u8 {
        self.level.as_ref().map_or(FLAT_LEVEL, |p| p.read(x, y, FLAT_LEVEL))
    }

    pub fn w(&self) -> u32 {
        self.tiles.w()
    }

    pub fn h(&self) -> u32 {
        self.tiles.h()
    }

    /// Intern a generator-made name. The same string gives the same key.
    pub fn local(&mut self, name: &str) -> Key {
        if let Some(i) = self.local_names.position(name) {
            return Key::Local(i as u32);
        }
        self.local_names.push(name);
        Key::Local(self.local_names.len() as u32 - 1)
    }

    pub fn push_list(&mut self, list: Vec<Action>) -> ListRef {
        self.lists.push(list);
        ListRef::Blueprint(self.lists.len() as u16 - 1)
    }

    pub fn push_conds(&mut self, conds: Vec<Cond>) -> CondsRef {
        self.conds.push(conds);
        CondsRef::Blueprint(self.conds.len() as u16 - 1)
    }

    pub fn push_names(&mut self, names: Vec<Key>) -> NamesRef {
        self.name_lists.push(names);
        NamesRef::Blueprint(self.name_lists.len() as u16 - 1)
    }

    pub fn push_text(&mut self, text: String) -> TextRef {
        self.texts.push(text);
        TextRef::Local(self.texts.len() as u16 - 1)
    }

    pub fn list(&self, r: ListRef) -> Option<&[Action]> {
        match r {
            ListRef::Blueprint(i) => self.lists.get(usize::from(i)).map(Vec::as_slice),
            ListRef::Catalog(_) => None,
        }
    }

    pub fn conds_of(&self, r: CondsRef) -> Option<&[Cond]> {
        match r {
            CondsRef::Blueprint(i) => self.conds.get(usize::from(i)).map(Vec::as_slice),
            CondsRef::Catalog(_) => None,
        }
    }

    pub fn names_of(&self, r: NamesRef) -> Option<&[Key]> {
        match r {
            NamesRef::Blueprint(i) => self.name_lists.get(usize::from(i)).map(Vec::as_slice),
            NamesRef::Catalog(_) => None,
        }
    }

    /// The region under cell `(x, y)` by the zone's [`RegionMap`]; `None` for a zone with none.
    pub fn region_at(&self, x: i32, y: i32) -> Option<u8> {
        self.regions.region_at(x, y)
    }

    /// The index of the first area whose rect holds cell `(x, y)`.
    pub fn area_at(&self, x: i32, y: i32) -> Option<usize> {
        self.areas.iter().position(|a| a.rect.contains(x, y))
    }

    pub fn text(&self, r: TextRef) -> Option<&str> {
        match r {
            TextRef::Local(i) => self.texts.get(usize::from(i)).map(String::as_str),
            TextRef::Text(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locals_intern_and_lists_index() {
        let mut bp = Blueprint::new(ZoneId::Mine, 10, 8, Tile::CaveWall);
        let a = bp.local("gate_3");
        assert_eq!(bp.local("gate_3"), a);
        assert_ne!(bp.local("gate_4"), a);
        let l = bp.push_list(vec![Action::Show(a)]);
        assert_eq!(bp.list(l), Some(&[Action::Show(a)][..]));
        let t = bp.push_text("The Old Adit".into());
        assert_eq!(bp.text(t), Some("The Old Adit"));
        assert_eq!((bp.w(), bp.h()), (10, 8));
    }

    #[test]
    fn a_region_map_answers_by_its_coarse_cell() {
        let bp = Blueprint::new(ZoneId::County, 64, 64, Tile::Grass);
        assert_eq!(bp.region_at(3, 3), None, "no map, no answer: the zone's own sky");
        let mut m = RegionMap::new(16, 4, 4, 0);
        m.set(1, 0, 2);
        m.set(3, 3, 1);
        m.set(9, 9, 1);
        assert_eq!(m.region_at(15, 15), Some(0));
        assert_eq!(m.region_at(16, 0), Some(2));
        assert_eq!(m.region_at(31, 15), Some(2));
        assert_eq!(m.region_at(63, 63), Some(1));
        assert_eq!(m.region_at(64, 0), None);
        assert_eq!(m.region_at(-1, 0), None);
    }
}
