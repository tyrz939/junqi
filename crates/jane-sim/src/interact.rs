//! USE and the world verbs behind it (`sim/interact.ts`). 2020's B button was "Use / Talk / Push /
//! Pull" and so is this: one entry point; capability rows on the prop decide what happens.
//!
//! ```text
//! tap USE    put down | pick up a drop | talk | unlock / enter | loot | carry | read | use
//! hold USE   against a `push` prop: push it one cell (or pull it, walking away)
//! Repair     a world spell: the nearest prop that answers `repair`, paying its `needs`
//! Grow       the same for `grow`, and only where a light comes down
//! a bolt     switches on props that answer its school (2020's frost-lit blue torches)
//! ```
//!
//! **Focus** ([`focus_of`]) is what USE would act on now: pure, asked by the command and by
//! `View::focus` for the prompt. Reach is measured from the feet: 12 px to a prop's footprint,
//! 20 px to someone to talk to, 14 px to a drop. Something behind her scores 8 px further, and a
//! thing that only pushes 6 px further, so the note by the crate is read first. Distances are
//! `Fx` with one `isqrt` per candidate inside reach (ARCHITECTURE.md §2); ties go to the first
//! in id order.

use jane_core::action::{Facing, School};
use jane_core::blueprint::PropSpawn;
use jane_core::num::{CELL_FX, dist_sq, isqrt};
use jane_core::{Blueprint, Cell, Fx, ItemId, NightLock, Rect, TextId, TextRef, Vec2};
use jane_data::{Answers, Faction, PropDef, WorldSpell};

use crate::actions::{Subject, request_travel, run_actions};
use crate::clear::box_touches_feet;
use crate::ctx::Ctx;
use crate::dialogue;
use crate::event::{EventKind, PropChange, SfxKind, ToastKind};
use crate::ids::{DropId, PropId, PropIx, Seat, UnitId};
use crate::inventory;
use crate::light::{grows_at, prop_centre};
use crate::runtime::ZoneRuntime;
use crate::state::{GameState, LootState, NightState, Prop, Speaker, TravelRequest, Unit, ZoneState};
use crate::tuning::{
    BODY_HALF_FX, FOCUS_BEHIND_FX, FOCUS_PUSH_ONLY_FX, PICKUP_REACH_FX, PUSH_ENERGY, PUSH_HOLD_TICKS, TALK_REACH_FX,
    USE_REACH_FX, WORLD_SPELL_REACH_FX,
};
use crate::under::uncover;
use crate::units::{move_unit, place_unit, spend_energy};

/// What USE would act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusRef {
    Drop(DropId),
    Unit(UnitId),
    Prop(PropId),
}

/// The word on the prompt; presentation owns the English.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Enter,
    /// A door not answered after dark, at night.
    TryTheDoor,
    Unlock,
    Open,
    PickUp,
    Craft,
    Read,
    Use,
    HoldToPush,
    Take(ItemId),
    Talk,
    PutDown,
    /// The prop row's own prompt ("Gather", "Pick up").
    Custom(TextId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub target: FocusRef,
    pub verb: Verb,
    /// It also moves: the prompt says "(hold to push)".
    pub pushes: bool,
}

/// What focus and the verbs read of a zone, borrowed.
#[derive(Clone, Copy, Debug)]
pub struct Here<'a> {
    pub world: &'a GameState,
    pub zone: &'a ZoneState,
    pub rt: &'a ZoneRuntime,
    pub bp: &'a Blueprint,
}

/// A placed prop's spawn row: where it leads, what it holds, runs and says.
pub fn spawn_of<'b>(bp: &'b Blueprint, p: &Prop) -> Option<&'b PropSpawn> {
    p.spawn.and_then(|s| bp.props.get(usize::from(s)))
}

/// What a prop still holds.
pub fn loot_of<'b>(bp: &'b Blueprint, p: &'b Prop) -> &'b [jane_core::Stack] {
    match &p.loot {
        LootState::AsSpawned => spawn_of(bp, p).map_or(&[], |s| s.loot.as_slice()),
        LootState::Left(v) => v,
    }
}

pub(crate) fn has_loot(bp: &Blueprint, p: &Prop) -> bool {
    !p.used && !loot_of(bp, p).is_empty()
}

/// Its `use` list runs when used (not once-and-used, and not a thing only a spell or a bolt
/// switches on).
fn usable(def: &PropDef, s: Option<&PropSpawn>, p: &Prop) -> bool {
    s.is_some_and(|s| s.use_list.is_some()) && !(def.once && p.used) && def.answers.is_none()
}

pub fn footprint(def: &PropDef, p: &Prop) -> Rect {
    Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(def.w), i32::from(def.h))
}

/// From the feet to the prop's rectangle (not its middle: big props are easy to reach).
pub fn prop_distance_sq(def: &PropDef, p: &Prop, at: Vec2) -> i64 {
    let x0 = i32::from(p.cell.x) * CELL_FX;
    let y0 = i32::from(p.cell.y) * CELL_FX;
    let (x1, y1) = (x0 + i32::from(def.w) * CELL_FX, y0 + i32::from(def.h) * CELL_FX);
    let d = |v: i32, lo: i32, hi: i32| {
        i64::from(if v < lo {
            lo - v
        } else if v > hi {
            v - hi
        } else {
            0
        })
    };
    let (dx, dy) = (d(at.x.0, x0, x1), d(at.y.0, y0, y1));
    dx * dx + dy * dy
}

const fn sq(r: i32) -> i64 {
    r as i64 * r as i64
}

fn is_enemy(a: &Unit, b: &Unit) -> bool {
    a.faction != b.faction && (a.faction == Faction::Friendly || b.faction == Faction::Friendly)
}

/// Behind her: the dot of (to - from) with her facing is negative.
fn behind(u: &Unit, to: Vec2) -> bool {
    let (fx, fy) = u.facing.delta();
    i64::from(to.x.0 - u.pos.x.0) * i64::from(fx) + i64::from(to.y.0 - u.pos.y.0) * i64::from(fy) < 0
}

/// The hours a door keeps now: a verb's, else its row's.
pub fn night_lock_of(bp: &Blueprint, p: &Prop) -> Option<NightLock> {
    match p.night {
        NightState::AsSpawned => spawn_of(bp, p).and_then(|s| s.night_lock),
        NightState::Locked(l) => Some(l),
        NightState::Open => None,
    }
}

/// Does this door refuse `seat` now? What it says if so: it is inside its hours, and it is not
/// a `keyed` lock she holds a key for (one that fits the door's `keyTag`).
pub fn night_shut(
    world: &GameState,
    bp: &Blueprint,
    locals: &[jane_core::Sym],
    p: &Prop,
    seat: Option<Seat>,
) -> Option<TextRef> {
    let lock = night_lock_of(bp, p)?;
    if !lock.shut_at(world.hour() as u8) {
        return None;
    }
    let tag = spawn_of(bp, p).and_then(|s| s.key_tag);
    let holds = |seat: Seat| {
        let (Some(tag), Some(pl)) = (tag, world.player(seat)) else { return false };
        let cat = jane_data::catalog();
        let tag = crate::sym::of_key(tag, locals);
        pl.bag.iter().flatten().any(|s| cat.combat.item(s.item).opens.is_some_and(|o| crate::sym::of_name(o) == tag))
    };
    if lock.keyed && seat.is_some_and(holds) {
        return None;
    }
    Some(lock.says)
}

/// The seat whose body `u` is, if any.
fn seat_of_body(world: &GameState, u: &Unit) -> Option<Seat> {
    world.players.iter().find(|p| p.unit == u.id).map(|p| p.seat)
}

fn interactable(h: &Here<'_>, def: &PropDef, p: &Prop) -> bool {
    if p.hidden {
        return false;
    }
    let s = spawn_of(h.bp, p);
    p.locked
        || s.is_some_and(|s| s.to.is_some())
        || has_loot(h.bp, p)
        || s.is_some_and(|s| s.talk.is_some())
        || def.carry
        || def.bench
        || def.store
        || usable(def, s, p)
        // A stone, a barrel, a bale: nothing to open, but it moves. The prompt is how she learns that.
        || def.push
}

fn first_verb(h: &Here<'_>, def: &PropDef, p: &Prop, seat: Option<Seat>) -> Option<Verb> {
    let s = spawn_of(h.bp, p);
    let custom = def.prompt.map(Verb::Custom);
    if p.locked {
        return Some(Verb::Unlock);
    }
    if s.is_some_and(|s| s.to.is_some()) {
        let shut = night_shut(h.world, h.bp, &h.rt.locals, p, seat).is_some();
        return Some(if shut { Verb::TryTheDoor } else { custom.unwrap_or(Verb::Enter) });
    }
    // The row knows best what it is: a herb is gathered. A chest has no word of its own.
    if has_loot(h.bp, p) {
        return Some(custom.unwrap_or(Verb::Open));
    }
    if def.carry {
        return Some(Verb::PickUp);
    }
    if def.bench {
        return Some(Verb::Craft);
    }
    if def.store {
        return Some(custom.unwrap_or(Verb::Open));
    }
    if s.is_some_and(|s| s.talk.is_some()) {
        return Some(custom.unwrap_or(Verb::Read));
    }
    if usable(def, s, p) {
        return Some(custom.unwrap_or(Verb::Use));
    }
    custom
}

/// What USE would act on right now, for body `u`. `units` and `props` are reused buffers.
pub fn focus_of(h: &Here<'_>, u: &Unit, units: &mut Vec<UnitId>, props: &mut Vec<PropIx>) -> Option<Focus> {
    let cat = jane_data::catalog();
    if let Some(c) = u.carrying {
        return Some(Focus { target: FocusRef::Prop(c), verb: Verb::PutDown, pushes: false });
    }
    // The nearest drop in reach; of two as near, the later.
    let mut drop = None;
    let mut best = sq(PICKUP_REACH_FX);
    for d in &h.zone.drops {
        let dd = dist_sq(u.pos, d.pos);
        if dd <= best {
            best = dd;
            drop = Some(d);
        }
    }
    if let Some(d) = drop {
        return Some(Focus { target: FocusRef::Drop(d.id), verb: Verb::Take(d.item), pushes: false });
    }
    let mut best: Option<Focus> = None;
    let mut score = i64::MAX;
    // Someone to talk to, in id order.
    let r = Fx(TALK_REACH_FX).cell() + 1;
    let (cx, cy) = u.pos.cell();
    h.rt.unit_blocks.query(cx - r, cy - r, cx + r, cy + r, units);
    units.sort();
    for &id in units.iter() {
        let Some(o) = h.zone.unit(id) else { continue };
        if o.id == u.id || !o.alive || !o.awake || o.hidden || is_enemy(u, o) {
            continue;
        }
        if cat.combat.unit(o.def).talk.is_none() {
            continue;
        }
        let dd = dist_sq(u.pos, o.pos);
        if dd > sq(TALK_REACH_FX) {
            continue;
        }
        let s = i64::from(isqrt(dd as u64)) + if behind(u, o.pos) { i64::from(FOCUS_BEHIND_FX) } else { 0 };
        if s < score {
            score = s;
            best = Some(Focus { target: FocusRef::Unit(o.id), verb: Verb::Talk, pushes: false });
        }
    }
    // Props in reach, in id order.
    let (x0, y0) = (Fx(u.pos.x.0 - USE_REACH_FX).cell() - 1, Fx(u.pos.y.0 - USE_REACH_FX).cell() - 1);
    let (x1, y1) = (Fx(u.pos.x.0 + USE_REACH_FX).cell() + 1, Fx(u.pos.y.0 + USE_REACH_FX).cell() + 1);
    h.rt.props.query(x0, y0, x1, y1, props);
    for &ix in props.iter() {
        let p = &h.zone.props[ix as usize];
        let def = cat.story.prop(p.def);
        if !h.rt.awake_props.get(ix as usize).copied().unwrap_or(false) || !interactable(h, def, p) {
            continue;
        }
        let dd = prop_distance_sq(def, p, u.pos);
        if dd > sq(USE_REACH_FX) {
            continue;
        }
        let verb = first_verb(h, def, p, seat_of_body(h.world, u));
        let mut s = i64::from(isqrt(dd as u64));
        if behind(u, prop_centre(def, p)) {
            s += i64::from(FOCUS_BEHIND_FX);
        }
        if def.push && verb.is_none() {
            s += i64::from(FOCUS_PUSH_ONLY_FX);
        }
        if s < score {
            score = s;
            let verb = match verb {
                Some(v) => v,
                None if def.push => Verb::HoldToPush,
                None => Verb::Use,
            };
            best = Some(Focus { target: FocusRef::Prop(p.id), verb, pushes: def.push && verb != Verb::HoldToPush });
        }
    }
    best
}

/// The actor's body index, alive.
fn body_ix(cx: &Ctx<'_>) -> Option<usize> {
    let id = cx.actor_unit()?;
    cx.zone.unit_ix(id).filter(|&i| cx.zone.units[i].alive)
}

/// The `Use` command, for the actor (in dialogue it is `Advance`, and the caller says so).
pub fn use_(cx: &mut Ctx<'_>) {
    let (Some(seat), Some(ix)) = (cx.actor, body_ix(cx)) else { return };
    let body = cx.zone.units[ix].id;
    if cx.zone.units[ix].carrying.is_some() {
        put_down(cx, body);
        return;
    }
    let f = {
        let h = Here { world: cx.world, zone: cx.zone, rt: cx.rt, bp: cx.bp };
        focus_of(&h, &cx.zone.units[ix], &mut cx.scratch.units, &mut cx.scratch.props_b)
    };
    let Some(f) = f else { return };
    match f.target {
        FocusRef::Drop(d) => {
            inventory::pick_up(cx, seat, d);
        }
        FocusRef::Unit(id) => {
            let Some(tree) = cx.zone.unit(id).and_then(|o| cx.cat.combat.unit(o.def).talk) else { return };
            dialogue::start(cx, tree, Speaker::Unit(id));
        }
        FocusRef::Prop(id) => {
            if let Some(pix) = cx.zone.prop_ix(id) {
                use_prop(cx, seat, body, pix);
            }
        }
    }
}

/// The first key in her bag whose tag opens this lock.
fn find_key(cx: &Ctx<'_>, seat: Seat, tag: jane_core::Sym) -> Option<ItemId> {
    let p = cx.world.player(seat)?;
    p.bag
        .iter()
        .flatten()
        .map(|s| s.item)
        .find(|&i| cx.cat.combat.item(i).opens.is_some_and(|o| crate::sym::of_name(o) == tag))
}

fn use_prop(cx: &mut Ctx<'_>, seat: Seat, body: UnitId, ix: PropIx) {
    let cat = cx.cat;
    let bp: &Blueprint = cx.bp;
    let p = &cx.zone.props[ix as usize];
    let (pid, def, locked, used, loot) = (p.id, cat.story.prop(p.def), p.locked, p.used, has_loot(bp, p));
    let spawn = spawn_of(bp, p);
    let at = cx.zone.unit(body).map_or(Vec2::ZERO, |u| u.pos);
    if locked {
        let key = spawn.and_then(|s| s.key_tag).and_then(|t| find_key(cx, seat, cx.sym(t)));
        let Some(key) = key else {
            cx.emit(EventKind::Toast(ToastKind::Locked { prop: pid }));
            cx.emit(EventKind::Sfx { kind: SfxKind::Locked, at });
            return;
        };
        // One use path for every key. 2020 consumed the key; so do we, except bound ones.
        if !cat.combat.item(key).bound {
            inventory::remove(cx, seat, key, 1);
        }
        let p = &mut cx.zone.props[ix as usize];
        p.locked = false;
        if def.gate {
            p.solid = false;
            cx.rt.touch_prop(cx.zone, ix);
        }
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Unlock });
        cx.emit(EventKind::Toast(ToastKind::UnlockedWith(key)));
        return;
    }
    if let Some(to) = spawn.and_then(|s| s.to) {
        // Some doors are not answered at some hours. The key turns; the door does not (unless
        // the lock is one she holds the key to).
        if let Some(says) = night_shut(cx.world, bp, &cx.rt.locals, &cx.zone.props[ix as usize], Some(seat)) {
            cx.emit(EventKind::Toast(ToastKind::NightLock(says)));
            cx.emit(EventKind::Sfx { kind: SfxKind::Locked, at });
            return;
        }
        let mark = cx.sym(to.mark);
        if to.zone == cx.zone.id {
            hop(cx, body, mark);
        } else {
            request_travel(cx, TravelRequest { zone: to.zone, mark, at: None });
        }
        return;
    }
    if loot {
        open_loot(cx, seat, body, ix);
        return;
    }
    if def.carry {
        let u = cx.zone.unit_mut(body).expect("the actor's body");
        if u.energy_locked || u.energy < PUSH_ENERGY {
            cx.emit(EventKind::Toast(ToastKind::TooTired));
            return;
        }
        u.carrying = Some(pid);
        cx.zone.props[ix as usize].solid = false;
        cx.rt.touch_prop(cx.zone, ix);
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Use });
        return;
    }
    if def.store {
        // A cupboard: the window opens on it, for her; what is in it is the party's (`store.rs`).
        crate::inventory::emit_to(cx, seat, EventKind::Store { prop: pid });
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Open });
        return;
    }
    let (talk, use_list) = (spawn.and_then(|s| s.talk), spawn.and_then(|s| s.use_list));
    if let Some(tree) = talk {
        // Something read may also do something: the wall notice is the map (`Reveal`).
        let opened = dialogue::start(cx, tree, Speaker::Prop(pid));
        if opened && def.answers.is_none() && !(def.once && used) {
            if let Some(list) = use_list {
                cx.zone.props[ix as usize].used = true;
                run_actions(cx, list, Subject::Unit(body));
            }
        }
        return;
    }
    if let Some(list) = use_list.filter(|_| !(def.once && used)) {
        let p = &mut cx.zone.props[ix as usize];
        p.used = true;
        p.on = !p.on;
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Switch });
        run_actions(cx, list, Subject::Unit(body));
        return;
    }
    if def.bench {
        // The prompt says Craft: her bag opens with the craft row (`EventKind::Bench`).
        crate::inventory::emit_to(cx, seat, EventKind::Bench { prop: pid });
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Use });
    } else if def.push {
        cx.emit(EventKind::Toast(ToastKind::ItShifts));
    }
}

/// A chest, a bush, a bowl: what fits goes into her bag; what does not stays in it.
fn open_loot(cx: &mut Ctx<'_>, seat: Seat, body: UnitId, ix: PropIx) {
    let cat = cx.cat;
    let p = &cx.zone.props[ix as usize];
    let (pid, def) = (p.id, cat.story.prop(p.def));
    let use_list = spawn_of(cx.bp, p).and_then(|s| s.use_list);
    let mut stacks = [jane_core::Stack { item: ItemId(0), qty: 0 }; 8];
    let mut n = 0;
    for s in loot_of(cx.bp, p).iter().take(stacks.len()) {
        stacks[n] = *s;
        n += 1;
    }
    let mut left: Vec<jane_core::Stack> = Vec::new();
    for s in &stacks[..n] {
        let rest = inventory::add(cx, seat, s.item, s.qty);
        if rest > 0 {
            left.push(jane_core::Stack { item: s.item, qty: rest });
        }
    }
    let p = &mut cx.zone.props[ix as usize];
    if left.is_empty() {
        p.loot = LootState::Left(left);
        p.used = true;
        if def.hide_when_used {
            p.hidden = true;
            cx.rt.touch_prop(cx.zone, ix);
        }
        crate::regrow::emptied(cx, ix);
        if let Some(list) = use_list {
            run_actions(cx, list, Subject::Unit(body));
        }
    } else {
        // The chest keeps what did not fit.
        p.loot = LootState::Left(left);
        cx.emit(EventKind::Toast(ToastKind::InventoryFull));
    }
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Open });
}

/// A `to` into the zone she is in: over a dropped gate, through a vent. She is moved to the mark
/// and nothing reloads. One way by nature; refused with full arms, like any other door.
fn hop(cx: &mut Ctx<'_>, body: UnitId, mark: jane_core::Sym) {
    let Some(m) = cx.rt.mark(mark) else { return };
    let Some(ix) = cx.zone.unit_ix(body) else { return };
    if cx.zone.units[ix].carrying.is_some() {
        cx.emit(EventKind::Toast(ToastKind::PutDownFirst));
        return;
    }
    let (mx, my) = (i32::from(m.cell.x), i32::from(m.cell.y));
    let own = Some(cx.zone.units[ix].pos.cell());
    let (fx, fy) = cx.rt.grid.nearest_free(mx, my, crate::tuning::ARRIVAL_RADIUS, own).unwrap_or((mx, my));
    let u = &mut cx.zone.units[ix];
    place_unit(cx.rt, u, Vec2::centre(fx, fy));
    if let Some(f) = m.facing {
        u.facing = f;
    }
    u.hold = 0;
    let at = u.pos;
    cx.emit(EventKind::Sfx { kind: SfxKind::Push, at });
}

/// Props of `pred` within `reach` of `at` (to the footprint): any?
fn any_prop_near(
    h: &Here<'_>,
    at: Vec2,
    reach: i32,
    props: &mut Vec<PropIx>,
    pred: impl Fn(&PropDef, &Prop) -> bool,
) -> bool {
    let cat = jane_data::catalog();
    let (x0, y0) = (Fx(at.x.0 - reach).cell() - 1, Fx(at.y.0 - reach).cell() - 1);
    let (x1, y1) = (Fx(at.x.0 + reach).cell() + 1, Fx(at.y.0 + reach).cell() + 1);
    h.rt.props.query(x0, y0, x1, y1, props);
    props.iter().any(|&ix| {
        let p = &h.zone.props[ix as usize];
        let def = cat.story.prop(p.def);
        pred(def, p) && prop_distance_sq(def, p, at) <= sq(reach)
    })
}

/// A bed or a fire within reach of `at`: the game can only be saved there.
pub fn near_rest(h: &Here<'_>, at: Vec2, props: &mut Vec<PropIx>) -> bool {
    any_prop_near(h, at, 2 * USE_REACH_FX, props, |d, p| d.rest && !p.hidden)
}

/// A bench within reach: the bag window shows the craft row only then.
pub fn near_bench(h: &Here<'_>, at: Vec2, props: &mut Vec<PropIx>) -> bool {
    any_prop_near(h, at, 2 * USE_REACH_FX, props, |d, _| d.bench)
}

/// A bed or a fire near `at` in a zone whose runtime is not to hand: by a scan of its props (the
/// rare question `Rest { until }` asks of a friend in another zone).
pub fn near_rest_scan(zone: &ZoneState, at: Vec2) -> bool {
    let cat = jane_data::catalog();
    zone.props.iter().any(|p| {
        let def = cat.story.prop(p.def);
        def.rest && !p.hidden && prop_distance_sq(def, p, at) <= sq(2 * USE_REACH_FX)
    })
}

// --- carry ---------------------------------------------------------------------------------

/// Could prop `ix` stand at `to`? A sill takes feet and refuses freight: a pushed barrel stops at
/// the mouth of its room; a carried thing goes down wherever there is room. The prop is lifted
/// off the grid for the look and put back.
fn footprint_free(cx: &mut Ctx<'_>, ix: PropIx, to: (i32, i32), pushed: bool) -> bool {
    let p = &cx.zone.props[ix as usize];
    let def = cx.cat.story.prop(p.def);
    let there = Rect::new(to.0, to.1, i32::from(def.w), i32::from(def.h));
    if pushed && there.cells().any(|(x, y)| cx.rt.grid.no_push(x, y)) {
        return false;
    }
    let here = footprint(def, p);
    cx.rt.flush_prop_flags(cx.zone, &mut cx.scratch.props);
    let was = cx.zone.props[ix as usize].solid;
    cx.zone.props[ix as usize].solid = false;
    cx.rt.restamp_cells(cx.zone, here, &mut cx.scratch.props);
    // Nothing solid there, and no body where its feet would stand: one in the notch behind it may
    // stay (she pushes a crate south from there), one its feet would land on may not.
    let ok = there.cells().all(|(x, y)| cx.rt.grid.inside(x, y) && !cx.rt.grid.solid(x, y))
        && !cx.zone.units.iter().any(|u| u.alive && !u.hidden && box_touches_feet(u, def, to));
    cx.zone.props[ix as usize].solid = was;
    cx.rt.restamp_cells(cx.zone, here, &mut cx.scratch.props);
    ok
}

/// Put down what she carries, in the cells in front of her feet (never her own cell).
pub fn put_down(cx: &mut Ctx<'_>, body: UnitId) {
    let Some(u) = cx.zone.unit(body) else { return };
    let Some(pid) = u.carrying else { return };
    let Some(ix) = cx.zone.prop_ix(pid) else {
        cx.zone.unit_mut(body).expect("the body").carrying = None;
        return;
    };
    let def = cx.cat.story.prop(cx.zone.props[ix as usize].def);
    let (fx, fy) = u.facing.delta();
    let (w, h) = (i32::from(def.w), i32::from(def.h));
    // The cell block directly ahead: past the leading edge of her body box along the facing (not
    // her feet's cell: with her feet in its far half, the box reaches into the next cell, and a
    // rock put down there held her fast), centred across it.
    let ahead = |f: i32, at: Fx, size: i32| match f.cmp(&0) {
        std::cmp::Ordering::Greater => Fx(at.0 + BODY_HALF_FX - 1).cell() + 1,
        std::cmp::Ordering::Less => Fx(at.0 - BODY_HALF_FX).cell() - size,
        std::cmp::Ordering::Equal => at.cell() - size.div_euclid(2),
    };
    let (x, y) = (ahead(fx, u.pos.x, w), ahead(fy, u.pos.y, h));
    if !footprint_free(cx, ix, (x, y), false) {
        cx.emit(EventKind::Toast(ToastKind::NoRoom));
        return;
    }
    cx.rt.move_prop(cx.zone, ix, Cell::new(x.max(0) as u16, y.max(0) as u16));
    cx.zone.props[ix as usize].solid = def.solid;
    cx.zone.unit_mut(body).expect("the body").carrying = None;
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Push });
    uncover(cx, ix);
}

// --- push and pull -------------------------------------------------------------------------

/// The solid pushable prop in the cell just ahead of feet at `pos` facing `facing`.
fn pushable_ahead(cx: &mut Ctx<'_>, pos: Vec2, facing: Facing) -> Option<PropIx> {
    let (fx, fy) = facing.delta();
    let reach = CELL_FX - 256;
    let (x, y) = (Fx(pos.x.0 + fx * reach).cell(), Fx(pos.y.0 + fy * reach).cell());
    cx.rt.props.query(x, y, x, y, &mut cx.scratch.props_b);
    cx.scratch.props_b.iter().copied().find(|&ix| {
        let p = &cx.zone.props[ix as usize];
        let def = cx.cat.story.prop(p.def);
        !p.hidden
            && p.solid
            && def.push
            && cx.rt.awake_props.get(ix as usize).copied().unwrap_or(false)
            && footprint(def, p).contains(x, y)
    })
}

/// The movement along one axis of the stick, as the TS read it: `sign(round(component))`.
pub fn stick_axes(dir: jane_core::Angle, mag: u8) -> (i32, i32) {
    let axis = |q: i32| {
        // |q / 32768 * mag / 127| >= 0.5, and exactly -0.5 rounds to 0 as `Math.round` does.
        let v = i64::from(q) * i64::from(mag) * 2;
        let half = i64::from(jane_core::num::Q15_ONE) * 127;
        if v >= half {
            1
        } else if v < -half {
            -1
        } else {
            0
        }
    };
    (axis(jane_core::angle::cos_q15(dir).0), axis(jane_core::angle::sin_q15(dir).0))
}

/// Every tick USE is held (step 6). `(mx, my)` is the stick as whole steps, so holding USE and
/// walking away reads as a pull. Returns whether she is braced against something (the caller
/// does not move her).
pub fn hold_use(cx: &mut Ctx<'_>, body: UnitId, mx: i32, my: i32) -> bool {
    let Some(ix) = cx.zone.unit_ix(body) else { return false };
    let u = &cx.zone.units[ix];
    if u.carrying.is_some() || !u.alive {
        cx.zone.units[ix].hold = 0;
        return false;
    }
    let (pos, facing) = (u.pos, u.facing);
    let Some(pix) = pushable_ahead(cx, pos, facing) else {
        cx.zone.units[ix].hold = 0;
        return false;
    };
    let (fx, fy) = facing.delta();
    let along = mx * fx + my * fy;
    if along == 0 {
        cx.zone.units[ix].hold = 0;
        return true;
    }
    let uu = &mut cx.zone.units[ix];
    if uu.energy_locked || uu.energy < PUSH_ENERGY {
        let first = uu.hold == 0;
        uu.hold = 1;
        if first {
            cx.emit(EventKind::Toast(ToastKind::TooTired));
        }
        return true;
    }
    uu.hold += 1;
    if uu.hold < PUSH_HOLD_TICKS {
        return true;
    }
    uu.hold = 0;
    let dir = along.signum();
    let step = |d: i32| (Fx(d * fx * CELL_FX), Fx(d * fy * CELL_FX));
    if dir < 0 {
        // Pull: she steps back a cell first, then the prop follows into the gap.
        let from = uu.pos;
        let (dx, dy) = step(-1);
        move_unit(cx.rt, uu, dx, dy);
        let moved = (uu.pos.x.0 - from.x.0).abs() + (uu.pos.y.0 - from.y.0).abs();
        if moved < CELL_FX - 128 {
            let back = (Fx(from.x.0 - uu.pos.x.0), Fx(from.y.0 - uu.pos.y.0));
            move_unit(cx.rt, uu, back.0, back.1);
            return true;
        }
    }
    let p = &cx.zone.props[pix as usize];
    let to = (i32::from(p.cell.x) + fx * dir, i32::from(p.cell.y) + fy * dir);
    if to.0 < 0 || to.1 < 0 || !footprint_free(cx, pix, to, true) {
        if dir < 0 {
            let (dx, dy) = step(1);
            move_unit(cx.rt, &mut cx.zone.units[ix], dx, dy);
        }
        return true;
    }
    cx.rt.move_prop(cx.zone, pix, Cell::new(to.0 as u16, to.1 as u16));
    let uu = &mut cx.zone.units[ix];
    spend_energy(uu, PUSH_ENERGY);
    if dir > 0 {
        let (dx, dy) = step(1);
        move_unit(cx.rt, uu, dx, dy);
    }
    let (pid, at) = (cx.zone.props[pix as usize].id, cx.zone.units[ix].pos);
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Push });
    cx.emit(EventKind::Sfx { kind: SfxKind::Push, at });
    uncover(cx, pix);
    true
}

// --- world verbs ---------------------------------------------------------------------------

fn answers_verb(a: Option<Answers>, verb: WorldSpell) -> bool {
    matches!((a, verb), (Some(Answers::Repair), WorldSpell::Repair) | (Some(Answers::Grow), WorldSpell::Grow))
}

/// The nearest prop within 2 m that answers `verb`, not hidden and not yet used; `Grow` passes
/// over anything standing in the dark. Returns it, and whether one was passed over for the dark.
pub fn world_spell_target(cx: &mut Ctx<'_>, caster: UnitId, verb: WorldSpell) -> (Option<PropIx>, bool) {
    let Some(at) = cx.zone.unit(caster).map(|u| u.pos) else { return (None, false) };
    let cat = cx.cat;
    let reach = WORLD_SPELL_REACH_FX;
    let (x0, y0) = (Fx(at.x.0 - reach).cell() - 1, Fx(at.y.0 - reach).cell() - 1);
    let (x1, y1) = (Fx(at.x.0 + reach).cell() + 1, Fx(at.y.0 + reach).cell() + 1);
    cx.rt.props.query(x0, y0, x1, y1, &mut cx.scratch.props_b);
    let mut best = None;
    let mut best_d = sq(reach);
    let mut shaded = false;
    for &ix in &cx.scratch.props_b {
        let p = &cx.zone.props[ix as usize];
        let def = cat.story.prop(p.def);
        if p.hidden || p.used || !answers_verb(def.answers, verb) {
            continue;
        }
        let d = prop_distance_sq(def, p, at);
        if d > best_d {
            continue;
        }
        if verb == WorldSpell::Grow && !grows_at(cx.zone, cx.rt, cx.world.clock, prop_centre(def, p)) {
            shaded = true;
            continue;
        }
        best_d = d;
        best = Some(ix);
    }
    (best, shaded)
}

/// Repair or Grow landing on prop `ix` for the caster: pays its `needs` from the caster's bag
/// (her seat's), switches it on, runs its list. False (and the cast costs nothing) if the bag
/// is short.
pub fn world_spell_on(cx: &mut Ctx<'_>, caster: UnitId, ix: PropIx) -> bool {
    let seat = cx.party.seat_of(caster);
    let bp: &Blueprint = cx.bp;
    let needs: &[jane_core::Stack] = spawn_of(bp, &cx.zone.props[ix as usize]).map_or(&[], |s| s.needs.as_slice());
    for n in needs {
        let have = seat.map_or(0, |s| inventory::count(cx, s, n.item));
        if have < u32::from(n.qty) {
            cx.emit(EventKind::Toast(ToastKind::Needs { item: n.item, qty: n.qty }));
            return false;
        }
    }
    if let Some(s) = seat {
        for n in needs {
            inventory::remove(cx, s, n.item, n.qty);
        }
    }
    let p = &mut cx.zone.props[ix as usize];
    p.used = true;
    p.on = true;
    let pid = p.id;
    if let Some(list) = spawn_of(bp, &cx.zone.props[ix as usize]).and_then(|s| s.use_list) {
        run_actions(cx, list, Subject::Unit(caster));
    }
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Use });
    true
}

/// A bolt of `school` ended at `at`: every prop that answers the school, within `touch` of its
/// middle, not hidden and not already on, switches on and runs its list (subject `from`), in id
/// order over the props there when it landed.
pub fn school_touch(cx: &mut Ctx<'_>, school: School, at: Vec2, touch: Fx, from: Option<UnitId>) {
    let cat = cx.cat;
    let (x0, y0) = (Fx(at.x.0 - touch.0).cell() - 1, Fx(at.y.0 - touch.0).cell() - 1);
    let (x1, y1) = (Fx(at.x.0 + touch.0).cell() + 1, Fx(at.y.0 + touch.0).cell() + 1);
    // A prop's list may use the buffers: ask again after each one, from past the last.
    let mut after: Option<PropIx> = None;
    loop {
        cx.rt.props.query(x0, y0, x1, y1, &mut cx.scratch.props_b);
        let next = cx.scratch.props_b.iter().copied().find(|&ix| {
            let p = &cx.zone.props[ix as usize];
            let def = cat.story.prop(p.def);
            after.is_none_or(|a| ix > a)
                && !p.hidden
                && !p.on
                && def.answers.and_then(Answers::school) == Some(school)
                && dist_sq(at, prop_centre(def, p)) <= sq(touch.0)
        });
        let Some(ix) = next else { return };
        after = Some(ix);
        school_switch(cx, ix, from);
    }
}

/// A bolt's school touched prop `ix` (it answers the school and is not on): it switches on, is
/// used, and runs its list with `from` as the subject.
fn school_switch(cx: &mut Ctx<'_>, ix: PropIx, from: Option<UnitId>) {
    let p = &mut cx.zone.props[ix as usize];
    if p.hidden || p.on {
        return;
    }
    p.on = true;
    p.used = true;
    let pid = p.id;
    let bp: &Blueprint = cx.bp;
    if let Some(list) = spawn_of(bp, &cx.zone.props[ix as usize]).and_then(|s| s.use_list) {
        run_actions(cx, list, from.map_or(Subject::None, Subject::Unit));
    }
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Switch });
}
