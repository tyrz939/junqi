//! The chunk stamper (PORT.md §6.f): an authored place (`data/chunks/*.chunk`, a
//! [`ChunkDef`]) laid into the county at its site through the [`Kit`]. Replaces the builders of
//! `jane/src/world/chunks.ts`, which were code; a chunk is data now, and stamping one is the same
//! for every chunk, in the order the format's author set down:
//!
//! 1. **tiles**: every cell of the box, from the grid (the box is cleared by being drawn whole);
//! 2. **claims**: the cells nothing placed later by name may take;
//! 3. **props**: keyed ones by their content name, the rest named by where they stand, each with
//!    the fields its template sets and, for a door, where it leads;
//! 4. **units**: the same, with their patrol routes;
//! 5. **marks**, then the grid's **rects**, then the **`around`** rects (the box grown, clipped to
//!    the county);
//! 6. **fills**: a tuning row's props dropped into the fill's cells on the fill's own dice.
//!
//! What comes back is the stamped [`Chunk`]: its box, its gates and its slots in county cells.
//! The county reads nothing else from a chunk.

use alloc::vec::Vec;
use jane_core::action::TextRef;
use jane_core::blueprint::{Door, PropSpawn, Waypoint};
use jane_core::num::Tick;
use jane_core::tile::EAVES_ROWS;
use jane_core::{Key, NameId, Rect, Sfc32, Tile};
use jane_data::{ChunkDef, ChunkFill, PropTemplate};

use crate::kit::Kit;
use crate::steps::Step;

/// A set place as stamped: what the county's later stages read of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Its site's row (index into `County::sites`).
    pub site: u8,
    pub def: &'static ChunkDef,
    /// The box, in county cells.
    pub bounds: Rect,
    /// Each gate's county cell, one step outside the box, in the def's order.
    pub gates: Vec<(i32, i32)>,
    /// Each slot's county cell, in the def's order.
    pub slots: Vec<(NameId, (i32, i32))>,
}

impl Chunk {
    /// The site's id: `"station"`, `"town"`.
    pub fn id(&self) -> &'static str {
        self.def.id
    }

    /// A slot's county cell by name.
    pub fn slot(&self, name: NameId) -> Option<(i32, i32)> {
        self.slots.iter().find(|s| s.0 == name).map(|s| s.1)
    }

    /// A slot's county cell by its name's string (`<site>_door`).
    pub fn slot_named(&self, name: &str) -> Option<(i32, i32)> {
        let cat = jane_data::catalog();
        self.slots.iter().find(|s| cat.name(s.0) == name).map(|s| s.1)
    }
}

/// The fields a template sets, over a row that has only its key, def and cell.
pub fn apply_template(p: &mut PropSpawn, t: &PropTemplate) {
    p.locked = t.locked;
    p.key_tag = t.key_tag.map(Key::Name);
    p.hidden = t.hidden;
    p.on = t.on;
    p.loot = t.loot.to_vec();
    p.use_list = t.use_list;
    p.release = t.release;
    p.needs = t.needs.to_vec();
    p.talk = t.talk;
    p.label = t.label.map(TextRef::Text);
    p.night_lock = t.night_lock.map(jane_data::NightLockDef::lock);
}

/// The tile `def` stamps at its cell `(x, y)`: its grid's, but a roof's back [`EAVES_ROWS`] rows
/// are [`Tile::Eaves`], for the ground under them lies behind the house (its roof stands on its
/// walls three rows and a half south of its back edge): she walks there, drawn behind the roof.
pub fn stamped_tile(def: &ChunkDef, x: u16, y: u16) -> Tile {
    let roof = |y: i32| y >= 0 && def.tile(x, y as u16) == Tile::HouseRoof;
    let t = def.tile(x, y);
    if t == Tile::HouseRoof && (1..=EAVES_ROWS).any(|d| !roof(i32::from(y) - d)) { Tile::Eaves } else { t }
}

/// Stamp `def` for site row `site`, whose origin cell is `(ox, oy)`. Its fills throw
/// [`Step::CountyChunk`] at the kit's attempt, one stream per fill: `(site, fill)`.
pub fn stamp(k: &mut Kit, def: &'static ChunkDef, site: u8, ox: i32, oy: i32) -> Chunk {
    let margin = i32::from(jane_data::catalog().chunks.tuning.box_margin);
    let (bx, by) = def.origin(ox, oy, k.w(), k.h(), margin);
    let at = |c: jane_core::Cell| (bx + i32::from(c.x), by + i32::from(c.y));
    let bounds = Rect::new(bx, by, i32::from(def.w), i32::from(def.h));

    for y in 0..def.h {
        for x in 0..def.w {
            k.set(bx + i32::from(x), by + i32::from(y), stamped_tile(def, x, y));
        }
    }
    for r in def.claims {
        k.claim(Rect::new(bx + r.x, by + r.y, r.w, r.h));
    }
    for p in def.props {
        let (x, y) = at(p.at);
        let row = k.prop(p.key.map(Key::Name), p.prop.def, x, y);
        apply_template(row, &p.prop);
        row.to = p.to.map(|t| Door { zone: t.zone, mark: Key::Name(t.mark) });
    }
    for u in def.units {
        let (x, y) = at(u.at);
        let patrol = u
            .route
            .iter()
            .map(|w| {
                let (wx, wy) = at(w.at);
                Waypoint { cell: crate::kit::cell(wx, wy), dwell: w.dwell.unwrap_or(Tick::ZERO) }
            })
            .collect();
        k.unit(u.key.map(Key::Name), u.def, x, y, patrol).facing = u.facing;
    }
    for m in def.marks {
        let (x, y) = at(m.at);
        k.mark(Key::Name(m.name), x, y, m.facing);
    }
    for r in def.rects {
        k.rect(Key::Name(r.name), Rect::new(bx + r.rect.x, by + r.rect.y, r.rect.w, r.rect.h));
    }
    for a in def.around {
        let (n, e, s, w) = (i32::from(a.n), i32::from(a.e), i32::from(a.s), i32::from(a.w));
        let grown = Rect::new(bx - w, by - n, bounds.w + w + e, bounds.h + n + s);
        if let Some(r) = grown.intersect(Rect::new(0, 0, k.w(), k.h())) {
            k.rect(Key::Name(a.name), r);
        }
    }
    for (i, f) in def.fills.iter().enumerate() {
        let mut rng = k.dice(Step::CountyChunk, i32::from(site), i as i32);
        fill(k, &mut rng, f, bx, by);
    }

    Chunk {
        site,
        def,
        bounds,
        gates: def.gates.iter().map(|&g| def.gate_cell(g)).map(|(x, y)| (bx + x, by + y)).collect(),
        slots: def.slots.iter().map(|s| (s.name, at(s.at))).collect(),
    }
}

/// A fill: up to `count` anonymous props of the row's def, each tried at `tries` random cells of
/// the region; one lands where its whole footprint lies in the region and it, grown by `margin`,
/// is open unclaimed ground.
fn fill(k: &mut Kit, rng: &mut Sfc32, f: &ChunkFill, bx: i32, by: i32) {
    let row = jane_data::catalog().chunks.fill(f);
    let size = jane_data::catalog().story.prop(row.def);
    let (w, h) = (i32::from(size.w), i32::from(size.h));
    let cells: i32 = f.cells.iter().map(|r| r.w).sum();
    if cells == 0 {
        return;
    }
    let in_region = |x: i32, y: i32| f.cells.iter().any(|r| r.contains(x, y));
    let whole = |x: i32, y: i32| (y..y + h).all(|j| (x..x + w).all(|i| in_region(i, j)));
    for _ in 0..row.count {
        for _ in 0..row.tries {
            // The n-th cell of the region, counting its runs in reading order.
            let mut n = rng.range(0, cells - 1);
            let Some(run) = f.cells.iter().find(|r| {
                let here = n < r.w;
                if !here {
                    n -= r.w;
                }
                here
            }) else {
                break;
            };
            let (x, y) = (run.x + n, run.y);
            if whole(x, y) && k.fits(bx + x, by + y, w, h, i32::from(row.margin)) {
                k.prop(None, row.def, bx + x, by + y);
                break;
            }
        }
    }
}
