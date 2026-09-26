//! Step 14: zone changes, in seat order (`sim.ts performTravel`, `zones.ts placeArrival`).
//! A request is made by `Travel`, a door (`interact.rs`) or `Dev(Tp)` and performed
//! here, so nothing is halfway through a unit list when the list changes.

use jane_core::Vec2;

use crate::ctx::{Ctx, PartySnap, forget_unit};
use crate::event::EventKind;
use crate::ids::Seat;
use crate::journal;
use crate::ring::{Watchers, step_ring};
use crate::sim::{Sim, stamp_seats_fog};
use crate::state::{FactKey, Source, TravelRequest, Unit};
use crate::tuning::ARRIVAL_RADIUS;

impl Sim {
    pub(crate) fn perform_travel(&mut self, seat: Seat) {
        let (from, unit, req) = {
            let p = &mut self.state.players[seat.index()];
            let Some(req) = p.travel.take() else { return };
            (p.zone, p.unit, req)
        };
        let snap = PartySnap::of(&self.state);
        let body: Option<Unit> = self.with_ctx(from, Some(seat), &snap, false, |cx| {
            forget_unit(cx.zone, cx.rt, cx.party, unit);
            let body = cx.zone.remove_unit(unit)?;
            cx.rt.remove_unit(&body);
            // What is in flight does not outlive the room, once the last of the party has left.
            let others_stay = cx.world.connected().any(|q| q.seat != seat && q.zone == from);
            if !others_stay {
                cx.zone.projectiles.clear();
                cx.zone.grounds.clear();
            }
            Some(body)
        });
        let Some(body) = body else { return };
        let first = self.state.zone(req.zone).is_none();
        let came_by = {
            let p = &mut self.state.players[seat.index()];
            p.zone = req.zone;
            let came_by = p.last_mark;
            if req.at.is_none() {
                p.last_mark = req.mark;
            }
            came_by
        };
        let start = self.start_sym;
        self.with_ctx(req.zone, Some(seat), &snap, false, |cx| {
            let mut body = body;
            place_arrival(cx, &mut body, req, start);
            let ix = cx.zone.insert_unit(body);
            cx.rt.add_unit(&cx.zone.units[ix]);
            let w = Watchers::of(cx.world, req.zone, cx.zone);
            step_ring(cx.zone, cx.rt, &w, true, &mut cx.scratch.props);
            stamp_seats_fog(cx);
            // Presence with no watcher box: arriving at night, it was already gone.
            crate::presence::presence(cx, true);
            cx.emit(EventKind::Zone { zone: req.zone, first });
            // The journal: the place is seen, and the way from where she came in is walked.
            let place = cx.world.syms.intern(req.zone.name());
            journal::learn(cx, FactKey::Place(place), Source::Seen);
            if req.at.is_none() && came_by != req.mark {
                journal::learn(cx, FactKey::Route(came_by, req.mark), Source::Walked);
            }
        });
    }
}

/// Put an arriving body down: at an exact spot (waking at a bed or fire) or at the named mark
/// (else `start`, else the first mark, else the middle), on the nearest free cell.
fn place_arrival(cx: &Ctx<'_>, body: &mut Unit, req: TravelRequest, start: jane_core::Sym) {
    let (cell, facing) = match req.at {
        Some(at) => (at.cell(), None),
        None => {
            let m = cx.rt.mark(req.mark).or_else(|| cx.rt.mark(start)).or_else(|| cx.bp.marks.values().next().copied());
            match m {
                Some(m) => ((i32::from(m.cell.x), i32::from(m.cell.y)), m.facing),
                None => ((cx.bp.w() as i32 / 2, cx.bp.h() as i32 / 2), None),
            }
        }
    };
    let (fx, fy) = cx.rt.grid.nearest_free(cell.0, cell.1, ARRIVAL_RADIUS, None).unwrap_or(cell);
    body.pos = Vec2::centre(fx, fy);
    if let Some(f) = facing {
        body.facing = f;
    }
    body.path = None;
    body.target = None;
    body.hold = 0;
    body.awake = true;
}
