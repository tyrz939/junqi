//! What one seat sees (ARCHITECTURE.md §11): her zone, read only. `jane-present` builds its
//! frame from this and nothing else. This is the first slice: the seat, the ground and who
//! stands on it, and the clock. The rest of §11 lands with the systems it reads.

use jane_core::{Rect, Tile, Vec2, ZoneId};

use crate::ids::{Seat, UnitId};
use crate::runtime::ZoneRuntime;
use crate::sim::Sim;
use crate::state::{GameState, PlayerState, Prop, Unit, ZoneState};

/// A unit as drawn.
#[derive(Clone, Copy, Debug)]
pub struct UnitView<'a> {
    pub unit: &'a Unit,
    /// Where it stood before this tick's moves.
    pub prev_pos: Vec2,
    pub moved: bool,
    /// Per-instance variation (ART.md §3 `vary`), from the id: derived, never saved.
    pub variant: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    seat: Seat,
    state: &'a GameState,
    zone: &'a ZoneState,
    rt: &'a ZoneRuntime,
    indoor: bool,
}

impl Sim {
    /// Seat `seat`'s view; `None` for a seat that is not connected (or not yet in a live zone).
    pub fn view(&self, seat: Seat) -> Option<View<'_>> {
        let p = self.state.player(seat).filter(|p| p.connected)?;
        Some(View {
            seat,
            state: &self.state,
            zone: self.state.zone(p.zone)?,
            rt: self.runtime(p.zone)?,
            indoor: self.blueprint(p.zone).indoor,
        })
    }
}

impl<'a> View<'a> {
    pub fn seat(&self) -> Seat {
        self.seat
    }

    pub fn me(&self) -> &'a PlayerState {
        &self.state.players[self.seat.index()]
    }

    pub fn body(&self) -> &'a Unit {
        self.zone.unit(self.me().unit).expect("a connected seat's body is in her zone")
    }

    pub fn heroine(&self) -> &'a str {
        &self.state.name
    }

    pub fn seed(&self) -> u32 {
        self.state.seed
    }

    /// `(ticks since midnight, day)`.
    pub fn clock(&self) -> (u32, u32) {
        (self.state.clock, self.state.day)
    }

    pub fn zone(&self) -> ZoneId {
        self.zone.id
    }

    /// The zone's size in cells.
    pub fn size(&self) -> (u32, u32) {
        (self.rt.grid.w(), self.rt.grid.h())
    }

    pub fn indoor(&self) -> bool {
        self.indoor
    }

    pub fn tile(&self, cx: i32, cy: i32) -> Tile {
        self.rt.grid.tile_at(cx, cy)
    }

    pub fn flags(&self, cx: i32, cy: i32) -> u8 {
        self.rt.grid.flags_at(cx, cy)
    }

    /// Awake, unhidden units standing in `r`, in id order.
    pub fn units_in(&self, r: Rect) -> impl Iterator<Item = UnitView<'a>> + 'a {
        let zone = self.zone;
        let rt = self.rt;
        rt.awake_units.iter().filter_map(move |&id| {
            let u = zone.unit(id)?;
            let (cx, cy) = u.pos.cell();
            if u.hidden || !r.contains(cx, cy) {
                return None;
            }
            let prev_pos = prev_pos(rt, id).unwrap_or(u.pos);
            Some(UnitView { unit: u, prev_pos, moved: prev_pos != u.pos, variant: (id.get() % 4) as u8 })
        })
    }

    /// Props whose footprint may touch `r` (a superset, by block), in id order, hidden ones left out.
    pub fn props_in(&self, r: Rect) -> impl Iterator<Item = &'a Prop> + 'a {
        let cat = jane_data::catalog();
        let zone = self.zone;
        let mut ixs = Vec::new();
        self.rt.props.query(r.x, r.y, r.right() - 1, r.bottom() - 1, &mut ixs);
        ixs.into_iter().map(move |i| &zone.props[i as usize]).filter(move |p| {
            let d = cat.story.prop(p.def);
            !p.hidden && Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h)).overlaps(r)
        })
    }
}

fn prev_pos(rt: &ZoneRuntime, id: UnitId) -> Option<Vec2> {
    rt.prev_pos.binary_search_by_key(&id, |&(u, _)| u).ok().map(|i| rt.prev_pos[i].1)
}
