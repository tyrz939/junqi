//! Fires that are made, a fire's rest over time, and growth banked by a rest (PLAY-PLAN.md §2.2,
//! Phase 2). All of it only under `GameState::fires_made` (`tuning::FIRES_MADE` at New Game);
//! without it every fire is a rest as it was and nothing here runs.
//!
//! **Kept and made.** A prop whose def is `rest` (a hub's fire, a hearth, a dungeon's stove, a
//! bed, a fire a quest lit) is a rest always: the county keeps it. A `made` one (a fire pit, a
//! camp's fire, an old grate) is a rest only while it burns (`Prop::on`).
//!
//! **Making one.** USE held for [`FIRE_HOLD_TICKS`] on a cold pit with [`FIRE_LAY`] deadwood and a
//! light lays the wood and lights it ([`hold`], [`make`]); no dice. The light, in the order she
//! tries them: Fire (the `fireball` spell's mana), a match, a fire stone; an old grate (`fire_only`)
//! takes only Fire. Rain with nothing over the pit refuses every light, and a match in rain even
//! under a tent ("Too wet. It won't take in this."): nothing is spent. A tap says what is wanted.
//! Lit, it burns [`FIRE_BURNS`]; held again with deadwood it is fed ([`FIRE_MORE`] a stick, never
//! past [`FIRE_MOST`] from now). A camp's own fire burns with no clock until the rain gets it.
//!
//! **Going out.** At every ten-minute mark ([`Sim::burn_down`], step 2, which a bed's night runs
//! too, so a slept night burns as a sat-up one does), in every zone ever visited: a made fire whose
//! `burns_until` has come is ash (`on` false, `used` true); one whose region's rain ramp stands at
//! its `douse` with no tent or shed within [`FIRE_SHELTER_CELLS`] is out the same way.
//!
//! **Rest.** At any lit fire she saves and the fire's place becomes the waking point at once
//! (`verbs::rest`). The place, not the flame: ash still wakes her. Seated ([`state::Seated`]) she
//! mends `max / REST_TICKS` a tick, the remainder carried, until whole ([`step_seated`], after the
//! flush). Moving, acting (a cooldown mark moved), a blow, or anything hostile that has her as its
//! target or stands within its aggro reach of her with sight gets her up. A bed mends at once.
//!
//! **Unbanked growth.** A jar or a page found since the party's last rest is the finder's until
//! someone rests ([`found`], `verbs::grow`). If she dies first it comes off and the jar lies where
//! she fell ([`unbank_on_death`]); if she dies again before taking it back it goes home to where
//! it stood. Nothing is ever lost.
//!
//! [`state::Seated`]: crate::state::Seated

use jane_core::action::Stat;
use jane_core::{Cell, ItemId, Milli, SpellId, Sym, Tick, Vec2, ZoneId};
use jane_data::{Controller, Faction, PropDef};

use crate::ctx::{Ctx, WorldOp};
use crate::event::{Event, EventKind, FireWant, PropChange, SfxKind, ToastKind};
use crate::ids::{PropIx, Seat, UnitId};
use crate::interact::{FocusRef, Here, Verb, focus_of, spawn_of};
use crate::inventory;
use crate::sim::Sim;
use crate::state::{GameState, Prop, Seated, Unbanked, ZoneState};
use crate::tuning::{FIRE_BURNS, FIRE_HOLD_TICKS, FIRE_LAY, FIRE_MORE, FIRE_MOST, FIRE_SHELTER_CELLS, TICKS_PER_HOUR};

fn item(id: &str) -> Option<ItemId> {
    jane_data::catalog().combat.item_id(id)
}

/// Deadwood, matches, planks, fire stones: the items a fire asks about.
pub fn deadwood() -> Option<ItemId> {
    item("deadwood")
}
pub fn matches() -> Option<ItemId> {
    item("match")
}
fn planks() -> Option<ItemId> {
    item("wood")
}
fn fire_stone() -> Option<ItemId> {
    item("fire_stone")
}
/// The Fire spell.
pub fn fire_spell() -> Option<SpellId> {
    jane_data::catalog().combat.spell_id("fireball")
}

/// Is this prop a rest right now? A bed or a kept fire always; under `fires`, a made fire while it
/// burns. Hidden, nothing is.
pub fn rests(fires: bool, def: &PropDef, p: &Prop) -> bool {
    !p.hidden && (def.rest || (fires && def.made && p.on))
}

/// A made fire's pit she may make up or feed: a made def, shown, and not a quest's (no talk, no use
/// list: Pell's brazier is the parish's to light).
pub fn is_pit(world: &GameState, bp: &jane_core::Blueprint, def: &PropDef, p: &Prop) -> bool {
    world.fires_made
        && def.made
        && !p.hidden
        && spawn_of(bp, p).is_none_or(|s| s.talk.is_none() && s.use_list.is_none())
}

/// Deadwood to gather here now: a wood def, shown, not yet gathered (it comes back on the food
/// clock), and not something a story reads.
pub fn is_wood(world: &GameState, bp: &jane_core::Blueprint, def: &PropDef, p: &Prop) -> bool {
    world.fires_made && def.wood > 0 && !p.hidden && !p.used && spawn_of(bp, p).is_none_or(|s| s.talk.is_none())
}

fn count(world: &GameState, seat: Option<Seat>, it: Option<ItemId>) -> u32 {
    let (Some(p), Some(it)) = (seat.and_then(|s| world.player(s)), it) else { return 0 };
    crate::bag::bag_count(&p.bag[..], it)
}

/// The prompt over a pit: cold, make it; lit, rest (or feed it, holding deadwood, while it has a
/// clock with room on it).
pub fn verb(world: &GameState, seat: Option<Seat>, p: &Prop) -> Verb {
    if !p.on {
        return Verb::MakeFire;
    }
    let room = p.burns_until.is_some_and(|t| t.0 < world.tick.0.saturating_add(FIRE_MOST));
    if room && count(world, seat, deadwood()) > 0 { Verb::AddWood } else { Verb::Rest }
}

/// A tent or a shed within [`FIRE_SHELTER_CELLS`] of the pit (footprint to footprint).
pub fn sheltered(zone: &ZoneState, p: &Prop) -> bool {
    let cat = jane_data::catalog();
    let d = cat.story.prop(p.def);
    let (x0, y0) = (i32::from(p.cell.x), i32::from(p.cell.y));
    let (x1, y1) = (x0 + i32::from(d.w) - 1, y0 + i32::from(d.h) - 1);
    zone.props.iter().any(|q| {
        let qd = cat.story.prop(q.def);
        if !qd.shelters || q.hidden {
            return false;
        }
        let (qx0, qy0) = (i32::from(q.cell.x), i32::from(q.cell.y));
        let (qx1, qy1) = (qx0 + i32::from(qd.w) - 1, qy0 + i32::from(qd.h) - 1);
        let gap = |a0: i32, a1: i32, b0: i32, b1: i32| (b0 - a1).max(a0 - b1).max(0);
        gap(x0, x1, qx0, qx1).max(gap(y0, y1, qy0, qy1)) <= FIRE_SHELTER_CELLS
    })
}

/// Is the rain on this pit: its region's ramp at its `douse`?
fn wet(zone: &ZoneState, region: jane_data::Region, def: &PropDef) -> bool {
    def.douse.is_some_and(|d| zone.wetness[crate::living::region_ix(region)] >= d)
}

/// How she would light it, or why she cannot. Nothing is spent here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    Fire(SpellId),
    Match(ItemId),
    Stone(ItemId),
}

/// What she would make a fire at `p` with: two deadwood and the first light that takes, or what is
/// wanted. `wet` and `under` are the rain on the pit and a roof by it.
pub fn plan(
    world: &GameState,
    seat: Seat,
    mp: Milli,
    def: &PropDef,
    wet: bool,
    under: bool,
) -> Result<Light, FireWant> {
    let s = Some(seat);
    if wet && !under {
        return Err(FireWant::Wet);
    }
    if count(world, s, deadwood()) < u32::from(FIRE_LAY) {
        return Err(if count(world, s, planks()) > 0 { FireWant::Planks } else { FireWant::Wood });
    }
    let cat = jane_data::catalog();
    if let Some(f) = fire_spell().filter(|f| world.growth.spells.contains(f) && mp >= cat.combat.spell(*f).mp) {
        return Ok(Light::Fire(f));
    }
    if def.fire_only {
        return Err(FireWant::Fire);
    }
    if let Some(m) = matches().filter(|&m| count(world, s, Some(m)) > 0) {
        if !wet {
            return Ok(Light::Match(m));
        }
    }
    if let Some(st) = fire_stone().filter(|&st| count(world, s, Some(st)) > 0) {
        return Ok(Light::Stone(st));
    }
    // A match in the rain, under a roof: it is the wet that refuses it, and it says so.
    if wet && count(world, s, matches()) > 0 { Err(FireWant::Wet) } else { Err(FireWant::Light) }
}

/// The plan for the prop at `ix`, for `seat` whose body is `body`, here.
fn plan_at(cx: &Ctx<'_>, seat: Seat, body: UnitId, ix: PropIx) -> Result<Light, FireWant> {
    let p = &cx.zone.props[ix as usize];
    let def = cx.cat.story.prop(p.def);
    let region = cx.rt.region_at(i32::from(p.cell.x), i32::from(p.cell.y));
    let mp = cx.zone.unit(body).map_or(Milli::ZERO, |u| u.mp);
    plan(cx.world, seat, mp, def, wet(cx.zone, region, def), sheltered(cx.zone, p))
}

/// USE tapped on a pit: lit, she rests; cold, she is told what it wants (nothing if she has it:
/// the prompt says hold).
pub fn tap(cx: &mut Ctx<'_>, seat: Seat, body: UnitId, ix: PropIx) {
    if cx.zone.props[ix as usize].on {
        crate::verbs::rest(cx, None);
        return;
    }
    if let Err(w) = plan_at(cx, seat, body, ix) {
        cx.emit(EventKind::Toast(ToastKind::FireWants(w)));
    }
}

/// USE held this tick (`interact::hold_use`): on a pit she can make or feed, the hold counts up
/// and at [`FIRE_HOLD_TICKS`] it is done. `Some(true)`: she is busy with a fire (braced, she
/// does not walk). `None`: not a fire; pushing has the hold.
pub fn hold(cx: &mut Ctx<'_>, body: UnitId) -> Option<bool> {
    if !cx.world.fires_made {
        return None;
    }
    let seat = cx.actor?;
    let ix = cx.zone.unit_ix(body)?;
    let f = {
        let h = Here { world: cx.world, zone: cx.zone, rt: cx.rt, bp: cx.bp };
        focus_of(&h, &cx.zone.units[ix], &mut cx.scratch.units, &mut cx.scratch.props_b)
    }?;
    let (FocusRef::Prop(pid), Verb::MakeFire | Verb::AddWood) = (f.target, f.verb) else { return None };
    let pix = cx.zone.prop_ix(pid)?;
    let feeding = f.verb == Verb::AddWood;
    if !feeding && plan_at(cx, seat, body, pix).is_err() {
        return None;
    }
    let u = &mut cx.zone.units[ix];
    u.hold = u.hold.saturating_add(1);
    if u.hold < FIRE_HOLD_TICKS {
        return Some(true);
    }
    u.hold = 0;
    if feeding {
        feed(cx, seat, pix);
    } else {
        make(cx, seat, body, pix);
    }
    Some(true)
}

/// Lay the wood and light it: the hold has come to a second.
pub fn make(cx: &mut Ctx<'_>, seat: Seat, body: UnitId, ix: PropIx) {
    let light = match plan_at(cx, seat, body, ix) {
        Ok(l) => l,
        Err(w) => {
            cx.emit(EventKind::Toast(ToastKind::FireWants(w)));
            return;
        }
    };
    match light {
        Light::Fire(f) => {
            let cost = cx.cat.combat.spell(f).mp;
            if let Some(u) = cx.zone.unit_mut(body) {
                u.mp = Milli(u.mp.0 - cost.0);
            }
        }
        Light::Match(it) | Light::Stone(it) => {
            inventory::remove(cx, seat, it, 1);
        }
    }
    if let Some(w) = deadwood() {
        inventory::remove(cx, seat, w, FIRE_LAY);
    }
    let now = cx.world.tick;
    let p = &mut cx.zone.props[ix as usize];
    p.on = true;
    p.used = false;
    p.burns_until = Some(now.after(Tick(FIRE_BURNS)));
    let (pid, cell) = (p.id, p.cell);
    let at = cx.zone.unit(body).map_or(Vec2::ZERO, |u| u.pos);
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Switch });
    cx.emit(EventKind::Sfx { kind: SfxKind::Kindle, at });
    let zone = cx.zone.id;
    cx.emit_all(EventKind::Toast(ToastKind::FireLit { by: seat, zone, at: cell }));
}

/// One more stick: two hours more, never past twelve from now.
pub fn feed(cx: &mut Ctx<'_>, seat: Seat, ix: PropIx) {
    let Some(w) = deadwood() else { return };
    let now = cx.world.tick;
    let Some(t) = cx.zone.props[ix as usize].burns_until else { return };
    if inventory::remove(cx, seat, w, 1) == 0 {
        return;
    }
    let until = Tick(t.0.max(now.0).saturating_add(FIRE_MORE).min(now.0.saturating_add(FIRE_MOST)));
    let p = &mut cx.zone.props[ix as usize];
    p.burns_until = Some(until);
    let pid = p.id;
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Use });
    let hours = ((until.0 - now.0) / TICKS_PER_HOUR).min(255) as u8;
    cx.emit(EventKind::Toast(ToastKind::FireFed { hours }));
}

/// USE on a stump, a woodpile, a log: deadwood into her bag, and the food clock started.
pub fn gather(cx: &mut Ctx<'_>, seat: Seat, ix: PropIx) {
    let Some(w) = deadwood() else { return };
    let p = &cx.zone.props[ix as usize];
    let n = u16::from(cx.cat.story.prop(p.def).wood);
    let pid = p.id;
    let left = inventory::add(cx, seat, w, n);
    if left == n {
        cx.emit(EventKind::Toast(ToastKind::InventoryFull));
        return;
    }
    cx.zone.props[ix as usize].used = true;
    crate::regrow::emptied(cx, ix);
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Open });
}

impl Sim {
    /// Step 2, on every ten-minute mark: made fires that have burnt down, or that the rain has
    /// got, are ash, in every zone ever visited (see the module doc).
    pub(crate) fn burn_down(&mut self) {
        if !self.state.fires_made {
            return;
        }
        let now = self.state.tick;
        let cat = jane_data::catalog();
        for z in ZoneId::ALL {
            let bp = self.bps.get(z);
            let Some(zs) = self.state.zones[z.index()].as_deref_mut() else { continue };
            for ix in 0..zs.props.len() {
                let p = &zs.props[ix];
                let def = cat.story.prop(p.def);
                if !def.made || !p.on {
                    continue;
                }
                let spent = p.burns_until.is_some_and(|t| now >= t);
                let region = crate::living::region_at(bp, i32::from(p.cell.x), i32::from(p.cell.y));
                let rained = wet(zs, region, def) && !sheltered(zs, p);
                if !(spent || rained) {
                    continue;
                }
                let p = &mut zs.props[ix];
                p.on = false;
                p.used = true;
                p.burns_until = None;
                let prop = p.id;
                self.events.push(Event {
                    to: None,
                    in_zone: Some(z),
                    kind: EventKind::Prop { prop, change: PropChange::Switch },
                });
            }
        }
    }
}

// --- seated --------------------------------------------------------------------------------

/// Sit her down at a fire (`verbs::rest`): she mends from here.
pub fn sit(u: &mut crate::state::Unit) {
    u.seated = Some(Box::new(Seated {
        at: u.pos,
        hp_was: u.hp,
        gcd_was: u.gcd_until,
        stop_was: u.stop_until,
        hp_acc: 0,
        mp_acc: 0,
    }));
}

/// After the flush: every seat seated in this zone mends a tick's worth, or gets up.
pub fn step_seated(cx: &mut Ctx<'_>) {
    if !cx.world.fires_made {
        return;
    }
    let z = cx.zone.id;
    for seat in 0..cx.world.players.len() {
        let p = &cx.world.players[seat];
        if !p.connected || p.zone != z {
            continue;
        }
        let body = p.unit;
        let Some(u) = cx.zone.unit(body) else { continue };
        let Some(s) = u.seated.as_deref() else { continue };
        let up = !u.alive
            || u.pos != s.at
            || u.gcd_until != s.gcd_was
            || u.stop_until != s.stop_was
            || u.hp < s.hp_was
            || u.carrying.is_some();
        if up || threatened(cx, body) {
            if let Some(u) = cx.zone.unit_mut(body) {
                u.seated = None;
            }
            continue;
        }
        if let Some(u) = cx.zone.unit_mut(body) {
            crate::life::mend_seated(u);
        }
    }
}

/// Is anything hostile after her, or near enough to see her and come? Its target is her, or she
/// stands inside its aggro reach of her (`ai::aggro_reach`, the dark counted) in its sight.
pub fn threatened(cx: &mut Ctx<'_>, body: UnitId) -> bool {
    let Some(her) = cx.zone.unit(body) else { return false };
    let (pos, her_sum) = (her.pos, u32::from(her.strength) + u32::from(her.spirit));
    let mut near = std::mem::take(&mut cx.scratch.near);
    let reach = i64::from(crate::tuning::AGGRO_MAX_FX) * 2;
    crate::combat::query_near(cx.rt, pos, reach, &mut near);
    let mut found = false;
    for &oid in &near {
        let Some(o) = cx.zone.unit(oid) else { continue };
        if oid == body || !o.alive || o.hidden || o.faction == Faction::Friendly || o.controller == Controller::Npc {
            continue;
        }
        if o.target == Some(body) {
            found = true;
            break;
        }
        let def = crate::units::def_of(o);
        let dark = crate::ai::night_reach(cx, oid, def);
        let Some(o) = cx.zone.unit(oid) else { continue };
        let Some(h) = cx.zone.unit(body) else { continue };
        let r = crate::ai::aggro_reach(def, o.strength, Some(her_sum), dark);
        if r > 0 && crate::combat::metres_between(o, h) <= r && crate::los::line_of_sight(&cx.rt.grid, o.pos, h.pos) {
            found = true;
            break;
        }
    }
    cx.scratch.near = near;
    found
}

// --- unbanked growth -----------------------------------------------------------------------

/// `verbs::grow` has just given growth for `id`: if `id` is a prop here (a jar, a page) it is its
/// finder's, unbanked, until someone rests.
pub fn found(cx: &mut Ctx<'_>, stat: Stat, amount: i16, id: Sym) {
    if !cx.world.fires_made {
        return;
    }
    let (Some(seat), Some(ix)) = (cx.actor, crate::zone::prop_by_key(cx.rt, id)) else { return };
    let prop = cx.zone.props[ix as usize].id;
    let zone = cx.zone.id;
    let g = &mut cx.world.growth;
    g.unbanked.retain(|e| e.id != id);
    g.unbanked.push(Unbanked { id, stat, amount, seat, zone, prop, lying: false, home: Cell::new(0, 0) });
}

/// Any rest banks it all.
pub fn bank(world: &mut GameState) {
    world.growth.unbanked.clear();
}

/// Her body is about to stand up again (`life::revive_player`), and lay at `dead_at`: what she found
/// since the last rest comes off and lies there; what already lay somewhere goes home.
pub fn unbank_on_death(cx: &mut Ctx<'_>, seat: Seat, dead_at: Vec2) {
    if !cx.world.fires_made || cx.world.growth.unbanked.iter().all(|e| e.seat != seat) {
        return;
    }
    let list = std::mem::take(&mut cx.world.growth.unbanked);
    let mut keep = Vec::with_capacity(list.len());
    let (mut lies, mut home) = (false, false);
    for mut e in list {
        if e.seat != seat {
            keep.push(e);
            continue;
        }
        if e.lying {
            send_home(cx, &e);
            home = true;
            continue;
        }
        // The growth comes off: the world's, and every body's (`WorldOp::Grow`, negative).
        let g = &mut cx.world.growth;
        g.found.retain(|&f| f != e.id);
        let v = match e.stat {
            Stat::Strength => &mut g.strength,
            Stat::Spirit => &mut g.spirit,
        };
        *v = v.saturating_add_signed(-e.amount);
        cx.wops.ops.push(WorldOp::Grow { stat: e.stat, amount: -e.amount });
        if e.zone != cx.zone.id {
            // It never left its shelf: shown there again, to be found again.
            send_home(cx, &e);
            home = true;
            continue;
        }
        let Some(ix) = cx.zone.prop_ix(e.prop) else { continue };
        e.home = cx.zone.props[ix as usize].cell;
        let (x, y) = dead_at.cell();
        let cell = cx.rt.grid.nearest_free(x, y, 4, None).unwrap_or((x, y));
        cx.rt.move_prop(cx.zone, ix, Cell::new(cell.0.max(0) as u16, cell.1.max(0) as u16));
        reset(&mut cx.zone.props[ix as usize]);
        cx.rt.touch_prop(cx.zone, ix);
        cx.emit_all(EventKind::Prop { prop: e.prop, change: PropChange::Show });
        e.lying = true;
        lies = true;
        keep.push(e);
    }
    cx.world.growth.unbanked = keep;
    let mut ring = Vec::new();
    crate::ring::wake_props(cx.zone, cx.rt, &mut ring);
    if lies {
        cx.emit(EventKind::Toast(ToastKind::FoundLies));
    }
    if home {
        cx.emit(EventKind::Toast(ToastKind::FoundHome));
    }
}

/// Unopened and shown again.
fn reset(p: &mut Prop) {
    p.hidden = false;
    p.used = false;
    p.on = false;
}

/// A jar back where it was found, shown and unopened: in this zone through its runtime, elsewhere
/// in the zone's state (its runtime is made again on arrival).
fn send_home(cx: &mut Ctx<'_>, e: &Unbanked) {
    if e.zone == cx.zone.id {
        let Some(ix) = cx.zone.prop_ix(e.prop) else { return };
        if e.lying {
            cx.rt.move_prop(cx.zone, ix, e.home);
        }
        reset(&mut cx.zone.props[ix as usize]);
        cx.rt.touch_prop(cx.zone, ix);
        return;
    }
    let Some(zs) = cx.world.zone_mut(e.zone) else { return };
    let Some(p) = zs.props.iter_mut().find(|p| p.id == e.prop) else { return };
    if e.lying {
        p.cell = e.home;
    }
    reset(p);
}
