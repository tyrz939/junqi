//! The county's authored places (`data/chunks/*.chunk`, PORT.md §6.f): the station, Julie's yard,
//! the town, the mine mouth, the graveyard and the rest, as grids the county stamps where the
//! skeleton put their sites. The format is `compile/tables/chunks/parse.rs`'s doc comment and
//! `data/chunks/README.md`.
//!
//! Everything here is in **box cells**: `(0, 0)` is the top-left cell of the chunk's box, `x`
//! runs east and `y` south, and nothing but a gate lies outside `0..w`, `0..h`. A stamper adds
//! the box's county origin ([`ChunkDef::origin`]) to every cell. The county reads only the gates
//! and slots; the rest is what the chunk draws and names (ARCHITECTURE.md §5.3 providers).

use jane_core::Tile;
use jane_core::action::Facing;
use jane_core::grid::{Cell, Rect};
use jane_core::ids::{NameId, PropDefId, UnitDefId, ZoneId};
use jane_core::num::Tick;

use crate::model::{NameKind, PropTemplate};
use crate::{model, model_enum};

model_enum! {
    /// A side of a chunk's box.
    pub enum ChunkSide { N, E, S, W }
}

model! {
    /// A side of the box held at a fixed county coordinate, whatever the site says: the station's
    /// west side stands on the line at x 4. The other axis follows the anchor and is clamped.
    pub struct ChunkPin {
        pub side: ChunkSide,
        /// County cells: the column of a west or east side, the row of a north or south one.
        pub at: u16,
    }
}

model! {
    /// A walkable cell just outside the box that the chunk's own paths lead in from. The county
    /// joins each arriving road to the nearest gate, going round the box, never through it.
    pub struct ChunkGate {
        pub side: ChunkSide,
        /// Box cells along the side: the column of a north or south gate, the row of an east or west one.
        pub along: u16,
    }
}

model! {
    /// An exact cell the chunk offers to content placed by name (`at.slot`): a door's hole in a wall.
    pub struct ChunkSlot {
        pub name: NameId,
        pub at: Cell,
    }
}

model! {
    /// A rect the chunk exports that reaches past its box: the box grown by these many cells on
    /// each side (clipped to the county by the stamper). The only way a chunk names ground outside
    /// itself; its grid never does.
    pub struct ChunkAround {
        pub name: NameId,
        pub n: u16,
        pub e: u16,
        pub s: u16,
        pub w: u16,
    }
}

model! {
    /// Where a door prop leads: a mark in another zone.
    pub struct ChunkDoorTo {
        pub zone: ZoneId,
        pub mark: NameId,
    }
}

model! {
    /// A prop the grid draws: its footprint is its def's `w` x `h` from `at`.
    pub struct ChunkProp {
        /// `None`: an anonymous prop, named by the stamper.
        pub key: Option<NameId>,
        pub at: Cell,
        pub prop: PropTemplate,
        pub to: Option<ChunkDoorTo>,
    }
}

model! {
    /// A patrol waypoint, and how long to stand there.
    pub struct ChunkWaypoint {
        pub at: Cell,
        /// Ticks to stand at the point before walking on; `None` never stops.
        pub dwell: Option<Tick>,
    }
}

model! {
    /// A person or an animal the grid stands up.
    pub struct ChunkUnit {
        /// `None`: an anonymous unit, named by the stamper.
        pub key: Option<NameId>,
        pub def: UnitDefId,
        pub at: Cell,
        pub facing: Option<Facing>,
        /// Empty: it stays where it stands.
        pub route: &'static [ChunkWaypoint],
    }
}

model! {
    pub struct ChunkMark {
        pub name: NameId,
        pub at: Cell,
        pub facing: Option<Facing>,
    }
}

model! {
    pub struct ChunkRect {
        pub name: NameId,
        pub rect: Rect,
    }
}

model! {
    /// A procedural part of a chunk: the cells a tuning row's props may land in, seed by seed.
    pub struct ChunkFill {
        /// Index into `ChunkTuning::fills`.
        pub fill: u8,
        /// The region as row runs (`h` 1), in reading order. A prop's whole footprint lies in it.
        pub cells: &'static [Rect],
    }
}

model! {
    /// One authored place (`data/chunks/<id>.chunk`).
    pub struct ChunkDef {
        /// The site it stands at: `"station"`, `"town"`.
        pub id: &'static str,
        /// Index into `County::sites`.
        pub site: u8,
        /// Box size in cells.
        pub w: u16,
        pub h: u16,
        /// The box cell that stands on the site's origin cell (`anchor centre` is `(w / 2, h / 2)`).
        pub anchor: Cell,
        pub pin: Option<ChunkPin>,
        /// The side the chunk's front looks out of: where she comes in. Some gate is on it (lint).
        pub face: ChunkSide,
        pub gates: &'static [ChunkGate],
        pub slots: &'static [ChunkSlot],
        pub around: &'static [ChunkAround],
        /// The tiles `cells` indexes, in the order the grid first shows them.
        pub palette: &'static [Tile],
        /// Row-major, `w * h` indices into `palette`: the ground. Read it through [`ChunkDef::tile`].
        pub cells: &'static [u8],
        /// Cells nothing placed later by name may take (a garden, the station path, a house's eaves),
        /// as row runs (`h` 1) in reading order. Props, units and marks claim their own cells as well.
        pub claims: &'static [Rect],
        /// In reading order of their top-left cell.
        pub props: &'static [ChunkProp],
        /// In reading order.
        pub units: &'static [ChunkUnit],
        /// In reading order.
        pub marks: &'static [ChunkMark],
        /// In reading order of their top-left cell.
        pub rects: &'static [ChunkRect],
        /// In legend order.
        pub fills: &'static [ChunkFill],
    }
}

impl ChunkDef {
    /// The ground at a box cell.
    pub fn tile(&self, x: u16, y: u16) -> Tile {
        self.palette[usize::from(self.cells[usize::from(y) * usize::from(self.w) + usize::from(x)])]
    }

    /// A gate's cell in box coordinates: one step outside the box.
    pub fn gate_cell(&self, g: ChunkGate) -> (i32, i32) {
        let (w, h, a) = (i32::from(self.w), i32::from(self.h), i32::from(g.along));
        match g.side {
            ChunkSide::N => (a, -1),
            ChunkSide::S => (a, h),
            ChunkSide::W => (-1, a),
            ChunkSide::E => (w, a),
        }
    }

    /// The box cell a gate leads in to: the gate's inward neighbour.
    pub fn gate_inner(&self, g: ChunkGate) -> Cell {
        let (x, y) = self.gate_cell(g);
        let x = x.clamp(0, i32::from(self.w) - 1);
        let y = y.clamp(0, i32::from(self.h) - 1);
        Cell::new(x as u16, y as u16)
    }

    /// The county cell of the box's top-left for a site origin `(ox, oy)` on a `map_w` x `map_h`
    /// county, keeping `margin` cells of room round it (`ChunkTuning::box_margin`) on every axis the
    /// pin does not hold (`boxAt` in the TypeScript).
    pub fn origin(&self, ox: i32, oy: i32, map_w: i32, map_h: i32, margin: i32) -> (i32, i32) {
        let clamp =
            |o: i32, a: u16, size: u16, map: i32| (o - i32::from(a)).min(map - i32::from(size) - margin).max(margin);
        let mut x = clamp(ox, self.anchor.x, self.w, map_w);
        let mut y = clamp(oy, self.anchor.y, self.h, map_h);
        if let Some(p) = self.pin {
            let at = i32::from(p.at);
            match p.side {
                ChunkSide::W => x = at,
                ChunkSide::E => x = at - i32::from(self.w) + 1,
                ChunkSide::N => y = at,
                ChunkSide::S => y = at - i32::from(self.h) + 1,
            }
        }
        (x, y)
    }

    /// Every name the chunk exports, with its kind: keyed props and units, marks, rects (the grid's
    /// and the `around` ones). Slots are not names in a blueprint; they are cells placements read.
    pub fn exports(&self) -> impl Iterator<Item = (NameKind, NameId)> + '_ {
        let props = self.props.iter().filter_map(|p| p.key.map(|k| (NameKind::Prop, k)));
        let units = self.units.iter().filter_map(|u| u.key.map(|k| (NameKind::Unit, k)));
        let marks = self.marks.iter().map(|m| (NameKind::Mark, m.name));
        let rects = self.rects.iter().map(|r| (NameKind::Rect, r.name));
        let around = self.around.iter().map(|r| (NameKind::Rect, r.name));
        props.chain(units).chain(marks).chain(rects).chain(around)
    }
}

model! {
    /// A fill's row of `data/tuning/chunks.json`: what it places and how hard it looks.
    pub struct ChunkFillDef {
        /// `"graveyard_coffins"`.
        pub id: &'static str,
        /// Anonymous props of this def.
        pub def: PropDefId,
        /// At most this many.
        pub count: u8,
        /// Random cells of the region tried for each before giving up on it.
        pub tries: u8,
        /// Open, unclaimed ground kept round each footprint.
        pub margin: u8,
    }
}

model! {
    /// `data/tuning/chunks.json`.
    pub struct ChunkTuning {
        /// Cells kept between a box and the county's edge, for the ring road round it.
        pub box_margin: u16,
        /// In file order; `ChunkFill::fill` indexes it.
        pub fills: &'static [ChunkFillDef],
    }
}

model! {
    /// The authored places and their tuning.
    pub struct Chunks {
        /// In file order (sorted by id).
        pub defs: &'static [ChunkDef],
        pub tuning: ChunkTuning,
    }
}

impl Chunks {
    /// No chunks: a source with no `chunks/` and no tuning for them (a test fixture).
    pub const EMPTY: Chunks = Chunks { defs: &[], tuning: ChunkTuning { box_margin: 0, fills: &[] } };

    /// A chunk by its site's id, by a scan: for tools and tests.
    pub fn get(&self, id: &str) -> Option<&'static ChunkDef> {
        self.defs.iter().find(|c| c.id == id)
    }

    /// The chunk drawn at a site (index into `County::sites`), if any.
    pub fn at_site(&self, site: u8) -> Option<&'static ChunkDef> {
        self.defs.iter().find(|c| c.site == site)
    }

    pub fn fill(&self, f: &ChunkFill) -> &'static ChunkFillDef {
        &self.tuning.fills[usize::from(f.fill)]
    }

    /// Every name any chunk exports, in the county.
    pub fn exports(&self) -> impl Iterator<Item = (NameKind, NameId)> + '_ {
        self.defs.iter().flat_map(ChunkDef::exports)
    }
}
