//! Seats and commands (ARCHITECTURE.md §3.4, §4.5; `sim.ts join, leave, command`).

use jane_core::action::Facing;
use jane_core::{Vec2, ZoneId};

use crate::bag::bag_add;
use crate::ctx::{PartySnap, forget_unit};
use crate::event::{Event, EventKind, ToastKind};
use crate::ids::{ClientToken, Seat};
use crate::input::{Command, DevOp, InputFrame, StampedCommand};
use crate::ring::{Watchers, step_ring};
use crate::sim::{Sim, stamp_seats_fog};
use crate::state::{FlagKey, PlayerState, QuestProgress, Stats, TravelRequest, Unit};
use crate::tuning::{ARRIVAL_RADIUS, BAR_SLOTS, MAX_PLAYERS, TICKS_PER_HOUR};
use crate::units::{max_hp, max_mp, new_unit};
use crate::{dialogue, interact, inventory, quests, store};

impl Sim {
    /// Apply one command (step 0). A cast reads its seat's frame: the raw aim and the assist
    /// profile of this frame, so the aim is in the record.
    pub(crate) fn command(&mut self, c: &StampedCommand, frames: &[InputFrame; MAX_PLAYERS]) {
        let seat = match (c.seat, c.cmd) {
            (None, Command::Join { who }) => {
                self.join(who);
                return;
            }
            (None, Command::Table { delay }) => {
                self.state.table_delay = delay.min(crate::tuning::TABLE_DELAY_MAX);
                return;
            }
            (Some(s), _) => s,
            (None, _) => return,
        };
        if !self.state.player(seat).is_some_and(|p| p.connected) {
            return;
        }
        let p = &mut self.state.players[seat.index()];
        match c.cmd {
            // The host's decision alone. Closing sends nobody home; it stops anyone new sitting down.
            Command::Open(on) => {
                if seat == Seat::HOST {
                    self.state.open = on;
                }
            }
            Command::Leave => self.leave(seat),
            Command::Bind { slot, to } => {
                if usize::from(slot) < BAR_SLOTS {
                    p.bar[usize::from(slot)] = Some(to.into());
                }
            }
            Command::Unbind { slot } => {
                if usize::from(slot) < BAR_SLOTS {
                    p.bar[usize::from(slot)] = None;
                }
            }
            Command::BarSwap { a, b } => {
                if usize::from(a) < BAR_SLOTS && usize::from(b) < BAR_SLOTS {
                    p.bar.swap(usize::from(a), usize::from(b));
                }
            }
            Command::Dev(op) => self.dev(seat, op),
            Command::Hop => {
                let frame = frames[seat.index()];
                self.in_seat_ctx(seat, |cx| crate::feel::hop(cx, seat, frame));
            }
            Command::Bar { slot, on } => {
                let frame = frames[seat.index()];
                self.in_seat_ctx(seat, |cx| crate::combat::bar_command(cx, seat, slot, on, frame));
            }
            Command::Cast { spell, on } => {
                let frame = frames[seat.index()];
                self.in_seat_ctx(seat, |cx| crate::combat::player_cast(cx, seat, spell, on, frame));
            }
            Command::Goto(g) => self.in_seat_ctx(seat, |cx| crate::walk::goto(cx, seat, g)),
            Command::Halt => self.in_seat_ctx(seat, |cx| crate::cast::halt(cx, seat)),
            // Her verbs, in her zone's context with her as the actor.
            Command::Use
            | Command::Item(_)
            | Command::BagMove { .. }
            | Command::BagDestroy { .. }
            | Command::CraftPut { .. }
            | Command::CraftClear { .. }
            | Command::CraftClearAll
            | Command::CraftTake
            | Command::StorePut { .. }
            | Command::StoreTake { .. }
            | Command::StoreMove { .. }
            | Command::StorePutAll { .. }
            | Command::Advance
            | Command::Choose { .. }
            | Command::CloseDialogue
            | Command::Abandon(_) => self.seat_command(seat, c.cmd),
            // A seated join is nobody's, and so is a seated table.
            Command::Join { .. } | Command::Table { .. } => {}
        }
    }

    /// A command that acts in the world, run in her zone's context with her as the actor
    /// (`sim.ts run`). Nothing happens without her body there.
    fn seat_command(&mut self, seat: Seat, cmd: Command) {
        let zone = self.state.players[seat.index()].zone;
        let snap = PartySnap::of(&self.state);
        self.with_ctx(zone, Some(seat), &snap, false, |cx| {
            if cx.actor_unit().is_none() {
                return;
            }
            match cmd {
                Command::Use => {
                    if cx.world.players[seat.index()].dialogue.is_some() {
                        dialogue::advance(cx);
                    } else {
                        interact::use_(cx);
                    }
                }
                Command::Item(item) => {
                    inventory::use_item(cx, item);
                }
                Command::BagMove { from, to } => inventory::move_slot(cx, seat, from, to),
                Command::BagDestroy { slot } => {
                    inventory::destroy(cx, seat, slot);
                }
                Command::CraftPut { bag, slot } => inventory::craft_put(cx, seat, bag, slot),
                Command::CraftClear { slot } => inventory::craft_clear(cx, seat, slot),
                Command::CraftClearAll => inventory::craft_clear_all(cx, seat),
                Command::CraftTake => {
                    inventory::craft_take(cx, seat);
                }
                Command::StorePut { prop, bag, to } => store::put(cx, seat, prop, bag, to),
                Command::StoreTake { prop, slot, to } => store::take(cx, seat, prop, slot, to),
                Command::StoreMove { prop, from, to } => store::arrange(cx, prop, from, to),
                Command::StorePutAll { prop } => store::put_all(cx, seat, prop),
                Command::Advance => dialogue::advance(cx),
                Command::Choose { option } => dialogue::choose(cx, option),
                Command::CloseDialogue => dialogue::close(cx),
                Command::Dev(DevOp::Give { item, qty }) => {
                    inventory::add(cx, seat, item, qty);
                }
                Command::Dev(DevOp::Quest(q)) => {
                    quests::give(cx, q);
                }
                Command::Abandon(q) => {
                    quests::abandon(cx, q);
                }
                _ => {}
            }
        });
    }

    fn dev(&mut self, seat: Seat, op: DevOp) {
        let p = &mut self.state.players[seat.index()];
        match op {
            DevOp::God(on) => p.god = on,
            // Performed with every other zone change, at the end of the step.
            DevOp::Tp { zone, mark } => p.travel = Some(TravelRequest { zone, mark, at: None }),
            DevOp::Time { hour } => self.state.clock = u32::from(hour % 24) * TICKS_PER_HOUR,
            DevOp::Flag { flag, value } => {
                self.state.flags.insert(FlagKey::Named(flag), value);
            }
            DevOp::Hp(v) => self.in_seat_ctx(seat, |cx| {
                let now = cx.world.tick;
                if let Some(u) = cx.actor_unit().and_then(|b| cx.zone.unit_mut(b)) {
                    crate::life::dev_hp(u, v, now);
                }
            }),
            DevOp::Mp(v) => self.in_seat_ctx(seat, |cx| {
                let now = cx.world.tick;
                if let Some(u) = cx.actor_unit().and_then(|b| cx.zone.unit_mut(b)) {
                    crate::life::dev_mp(u, v, now);
                }
            }),
            DevOp::Learn(spell) => self.in_seat_ctx(seat, |cx| crate::combat::learn_verb(cx, spell)),
            DevOp::Grow { stat, amount } => self.in_seat_ctx(seat, |cx| crate::verbs::dev_grow(cx, stat, amount)),
            DevOp::Kill => self.in_seat_ctx(seat, |cx| {
                if let Some(b) = cx.actor_unit() {
                    crate::combat::dev_kill(cx, b);
                }
            }),
            DevOp::Spawn(def) => self.in_seat_ctx(seat, |cx| {
                if let Some(b) = cx.actor_unit() {
                    crate::combat::dev_spawn(cx, b, def);
                }
            }),
            DevOp::Give { .. } | DevOp::Quest(_) => self.seat_command(seat, Command::Dev(op)),
        }
    }

    /// Run `f` in the seat's zone, acting as her.
    fn in_seat_ctx(&mut self, seat: Seat, f: impl FnOnce(&mut crate::ctx::Ctx<'_>)) {
        let zone = self.state.players[seat.index()].zone;
        let snap = PartySnap::of(&self.state);
        self.with_ctx(zone, Some(seat), &snap, false, f);
    }

    /// Where anyone who sits down appears: the party's last bed or fire, else where the story
    /// starts in the county.
    fn arrival(&mut self) -> (ZoneId, Vec2, Facing) {
        if let Some(r) = self.state.rest {
            return (r.zone, r.pos, Facing::South);
        }
        let cat = jane_data::catalog();
        let bp = self.bps.get(ZoneId::County);
        let mark =
            bp.marks.get(&jane_core::Key::Name(cat.story.start.mark)).or_else(|| bp.marks.values().next()).copied();
        match mark {
            Some(m) => {
                (ZoneId::County, Vec2::centre(i32::from(m.cell.x), i32::from(m.cell.y)), m.facing.unwrap_or_default())
            }
            None => (ZoneId::County, Vec2::centre(bp.w() as i32 / 2, bp.h() as i32 / 2), Facing::South),
        }
    }

    /// The seat `Join { who }` would give now, and whether it is her own body back; `None` when
    /// it would be refused (the world closed, or all four taken). One rule for [`join`] and for
    /// [`seat_for`](Self::seat_for).
    fn seat_choice(&self, who: ClientToken) -> Option<(Seat, bool)> {
        let s = &self.state;
        if !s.players.is_empty() && !s.open {
            return None;
        }
        match s.players.iter().position(|p| !p.connected && p.who == who && p.parked.is_some()) {
            Some(i) => Some((Seat(i as u8), true)),
            None if s.players.len() < MAX_PLAYERS => Some((Seat(s.players.len() as u8), false)),
            None => None,
        }
    }

    /// The seat a `Join { who }` applied first in the next step would sit her in, or `None` if it
    /// would be refused. The lockstep host tells a joiner her seat with the snapshot it sends
    /// before the step that seats her (ARCHITECTURE.md §7).
    pub fn seat_for(&self, who: ClientToken) -> Option<Seat> {
        self.seat_choice(who).map(|(s, _)| s)
    }

    /// Sit down (§4.5). A `who` that has played here before gets her own body and bags back;
    /// anyone else gets the start kit. Refused when the world is closed or all four are taken.
    pub(crate) fn join(&mut self, who: ClientToken) -> Option<Seat> {
        let (chosen, returning) = self.seat_choice(who)?;
        let back = returning.then_some(chosen.index());
        let (zone, at, facing) = self.arrival();
        self.ensure_zone(zone);
        self.ensure_runtime(zone);
        let rt = self.rts[zone.index()].as_deref().expect("runtime");
        let (cx, cy) = at.cell();
        let (fx, fy) = rt.grid.nearest_free(cx, cy, ARRIVAL_RADIUS, None).unwrap_or((cx, cy));
        let pos = Vec2::centre(fx, fy);

        let (seat, body, fresh) = if let Some(i) = back {
            let p = &mut self.state.players[i];
            let mut body = *p.parked.take().expect("a parked body");
            p.connected = true;
            p.respawn_at = None;
            body.pos = pos;
            body.path = None;
            if !body.alive {
                body.alive = true;
                body.hp = max_hp(&body);
            }
            (Seat(i as u8), body, false)
        } else {
            let seat = Seat(self.state.players.len() as u8);
            let id = self.state.next.unit();
            let body = new_unit(id, None, self.jane, pos, facing, self.state.tick);
            self.state.players.push(PlayerState {
                seat,
                who,
                unit: id,
                zone,
                last_mark: self.start_sym,
                respawn_at: None,
                bag: Box::new([None; crate::tuning::BAG_SLOTS]),
                bar: [None; BAR_SLOTS],
                craft: [None; crate::tuning::CRAFT_INPUTS],
                dialogue: None,
                god: false,
                stats: Stats::default(),
                travel: None,
                connected: true,
                parked: None,
                assist: None,
                fight: crate::state::Fight::default(),
            });
            (seat, body, true)
        };
        let p = &mut self.state.players[seat.index()];
        p.zone = zone;
        p.last_mark = self.start_sym;
        if fresh {
            self.start_kit(seat);
        }
        let growth = (self.state.growth.strength, self.state.growth.spirit);
        let snap = PartySnap::of(&self.state);
        self.with_ctx(zone, Some(seat), &snap, false, |cx| {
            let mut body = body;
            // A body never sleeps while its seat is connected.
            body.awake = true;
            if fresh {
                // Catch up: whatever the table grew before she came. (Spells are the world's:
                // her book is derived.)
                body.strength += growth.0;
                body.spirit += growth.1;
                body.hp = max_hp(&body);
                body.mp = max_mp(&body);
            }
            let ix = cx.zone.insert_unit(body);
            cx.rt.add_unit(&cx.zone.units[ix]);
            let w = Watchers::of(cx.world, zone, cx.zone);
            step_ring(cx.zone, cx.rt, &w, true, &mut cx.scratch.props);
            stamp_seats_fog(cx);
            cx.emit(EventKind::Zone { zone, first: seat == Seat::HOST && fresh });
            let place = cx.world.syms.intern(zone.name());
            crate::journal::learn(cx, crate::state::FactKey::Place(place), crate::state::Source::Seen);
        });
        self.announce_party();
        Some(seat)
    }

    /// The start kit (`data/start.json`) into a new seat's bag and bar, and the start quests.
    fn start_kit(&mut self, seat: Seat) {
        let cat = jane_data::catalog();
        let start = &cat.story.start;
        let p = &mut self.state.players[seat.index()];
        for s in start.items {
            // The quests unit counts `acquire`; the bag is enough here.
            let _ = bag_add(&mut p.bag[..], s.item, s.qty);
        }
        for (i, slot) in start.bar.iter().enumerate() {
            if slot.is_some() {
                p.bar[i] = *slot;
            }
        }
        // The quests unit gives quests properly (events, counts); the log starts with these.
        for &q in start.quests {
            let quests = &mut self.state.quests;
            if !quests.done.contains(&q) && !quests.active.iter().any(|a| a.quest == q) {
                let n = cat.story.quest(q).requirements.len();
                quests.active.push(QuestProgress { quest: q, counts: vec![0; n] });
            }
        }
    }

    /// Get up (§4.5). Her body waits, parked, with what she owned; what the story cannot go on
    /// without is handed to the lowest seat still here, overflow at her feet.
    pub(crate) fn leave(&mut self, seat: Seat) {
        let (zone, unit) = {
            let p = &self.state.players[seat.index()];
            (p.zone, p.unit)
        };
        let snap = PartySnap::of(&self.state);
        let body: Option<Unit> = self.with_ctx(zone, Some(seat), &snap, false, |cx| {
            // She puts down what she carries, in front of her (`use`); with no room there it
            // goes back where it was lifted from, solid as its row says.
            if cx.zone.unit(unit)?.carrying.is_some() {
                interact::put_down(cx, unit);
            }
            let ix = cx.zone.unit_ix(unit)?;
            if let Some(pid) = cx.zone.units[ix].carrying.take() {
                if let Some(pix) = cx.zone.prop_ix(pid) {
                    let p = &mut cx.zone.props[pix as usize];
                    p.solid = cx.cat.story.prop(p.def).solid;
                    cx.rt.touch_prop(cx.zone, pix);
                }
            }
            forget_unit(cx.zone, cx.rt, cx.party, unit);
            let body = cx.zone.remove_unit(unit)?;
            cx.rt.remove_unit(&body);
            Some(body)
        });
        let p = &mut self.state.players[seat.index()];
        p.connected = false;
        p.dialogue = None;
        p.travel = None;
        p.parked = body.map(Box::new);
        self.hand_on(seat);
        self.announce_party();
    }

    /// Story items (`ItemDef.story`) from a leaver's bag into the lowest connected seat's; what
    /// does not fit lands at her feet. With nobody left, the leaver keeps them.
    fn hand_on(&mut self, from: Seat) {
        let cat = jane_data::catalog();
        let Some(heir) = self.state.connected().next().map(|p| p.seat) else { return };
        let mut any = false;
        for i in 0..crate::tuning::BAG_SLOTS {
            let Some(stack) = self.state.players[from.index()].bag[i] else { continue };
            if !cat.combat.item(stack.item).story {
                continue;
            }
            self.state.players[from.index()].bag[i] = None;
            any = true;
            let left = bag_add(&mut self.state.players[heir.index()].bag[..], stack.item, stack.qty);
            if left > 0 {
                let (hz, hu) = {
                    let h = &self.state.players[heir.index()];
                    (h.zone, h.unit)
                };
                let id = self.state.next.drop();
                let tick = self.state.tick;
                if let Some(z) = self.state.zone_mut(hz) {
                    if let Some(pos) = z.unit(hu).map(|u| u.pos) {
                        z.drops.push(crate::state::Drop { id, item: stack.item, qty: left, pos, born: tick });
                    }
                }
            }
        }
        if any {
            self.events.push(Event {
                to: Some(heir),
                in_zone: None,
                kind: EventKind::Toast(ToastKind::LeftWhatMattered),
            });
        }
    }

    /// The penalty follows the head count, so say so whenever it changes.
    fn announce_party(&mut self) {
        let n = self.state.party_size();
        if n <= 1 && self.state.players.len() <= 1 {
            return;
        }
        self.events.push(Event { to: None, in_zone: None, kind: EventKind::Party { connected: n } });
    }
}
