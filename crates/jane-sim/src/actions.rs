//! The one verb runner (ARCHITECTURE.md §5, `sim/actions.ts`): dialogue options, prop use,
//! triggers, quest rewards, item use, unit death, clock rows and the console all speak it. The
//! match is exhaustive, so a new verb is a compile error here.
//!
//! This unit handles the world verbs the clock and travel need: `Flag`, `Toast`, `Show`, `Hide`,
//! `Lock`, `Unlock`, `Switch`, `Spawn`, `Despawn`, `Location`, `If`, `Travel`. Every other verb
//! is a documented no-op until its owner lands (each arm says whose). Player-scoped verbs do
//! nothing with `actor == None`. A name that does not resolve is a no-op plus
//! `EventKind::Missing`.

use jane_core::action::{FlagOp, FlagTest};
use jane_core::{Action, Cond, Condition, CondsRef, ListRef, Vec2};

use crate::ctx::Ctx;
use crate::event::{EventKind, PropChange, ToastKind};
use crate::ids::PropIx;
use crate::state::{FlagKey, TravelRequest};
use crate::tuning::SPAWN_RADIUS;
use crate::units::new_unit;

/// What a list lands on: the unit healed, the prop used. Never changes inside a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    Unit(crate::ids::UnitId),
    Prop(crate::ids::PropId),
    None,
}

/// Run a list. Nested lists (`If`) recurse; content is checked to be finite at build.
pub fn run_actions(cx: &mut Ctx<'_>, list: ListRef, subject: Subject) {
    let actions: &[Action] = match list {
        ListRef::Catalog(_) => cx.cat.list(list),
        ListRef::Blueprint(_) => {
            let bp = cx.bp;
            bp.list(list).unwrap_or(&[])
        }
    };
    for a in actions {
        run_action(cx, a, subject);
    }
}

fn conds<'b>(cx: &Ctx<'b>, r: CondsRef) -> &'b [Cond] {
    match r {
        CondsRef::Catalog(_) => cx.cat.conds_of(r),
        CondsRef::Blueprint(_) => cx.bp.conds_of(r).unwrap_or(&[]),
    }
}

/// The world's value of a flag (0 when unset).
pub fn flag(cx: &Ctx<'_>, k: FlagKey) -> i32 {
    cx.world.flags.get(&k).copied().unwrap_or(0)
}

/// A content flag key in this zone's names.
pub fn flag_key(cx: &Ctx<'_>, k: jane_core::FlagKey) -> FlagKey {
    match k {
        jane_core::FlagKey::Named(n) => FlagKey::Named(cx.sym(n)),
        jane_core::FlagKey::Been(n) => FlagKey::Been(cx.sym(n)),
        jane_core::FlagKey::Dead(n) => FlagKey::Dead(cx.sym(n)),
    }
}

pub fn set_flag(cx: &mut Ctx<'_>, k: FlagKey, v: i32) {
    cx.world.flags.insert(k, v);
}

/// Every condition holds (each possibly negated).
pub fn conditions_met(cx: &Ctx<'_>, list: &[Cond]) -> bool {
    list.iter().all(|c| condition(cx, c.c) != c.not)
}

fn condition(cx: &Ctx<'_>, c: Condition) -> bool {
    match c {
        Condition::Flag { key, test } => {
            let v = flag(cx, flag_key(cx, key));
            match test {
                FlagTest::Eq(n) => v == n,
                FlagTest::Min(n) => v >= n,
                FlagTest::NonZero => v != 0,
            }
        }
        Condition::Night => cx.world.is_night(),
        Condition::QuestActive(q) => cx.world.quests.active.iter().any(|p| p.quest == q),
        Condition::QuestDone(q) => cx.world.quests.done.contains(&q),
        Condition::HasSpell(s) => cx.world.growth.spells.contains(&s),
        Condition::HasItem(stack) => {
            // The actor's bag (bags are hers, and read from any zone).
            let Some(p) = cx.actor.and_then(|s| cx.world.player(s)) else { return false };
            let n: u32 = p.bag.iter().flatten().filter(|s| s.item == stack.item).map(|s| u32::from(s.qty)).sum();
            n >= u32::from(stack.qty)
        }
        Condition::Dead(k) => flag(cx, FlagKey::Dead(cx.sym(k))) != 0,
        // The quests unit: counts against requirements.
        Condition::QuestReady(_)
        // the journal and the living-world units (§3.7, §4.6.e):
        | Condition::Knows(_)
        | Condition::Heard(_)
        | Condition::SpeakerKnows(_) => false,
    }
}

fn prop(cx: &mut Ctx<'_>, k: jane_core::Key) -> Option<PropIx> {
    let s = cx.sym(k);
    let p = cx.rt.names.get(&s).copied();
    if p.is_none() {
        cx.emit(EventKind::Missing(s));
    }
    p
}

pub fn run_action(cx: &mut Ctx<'_>, a: &Action, subject: Subject) {
    match *a {
        Action::Flag { key, op } => {
            let k = flag_key(cx, key);
            let v = match op {
                FlagOp::Set(v) => v,
                FlagOp::Add(d) => flag(cx, k) + d,
            };
            set_flag(cx, k, v);
        }
        Action::Toast(t) => cx.emit(EventKind::Toast(ToastKind::Text(t))),
        Action::Show(k) | Action::Hide(k) => {
            let Some(ix) = prop(cx, k) else { return };
            let hide = matches!(a, Action::Hide(_));
            let p = &mut cx.zone.props[ix as usize];
            p.hidden = hide;
            let id = p.id;
            cx.rt.touch_prop(cx.zone, ix);
            // The interact unit: a shown prop stands aside whoever it landed on (`clearFootprint`).
            cx.emit(EventKind::Prop { prop: id, change: if hide { PropChange::Hide } else { PropChange::Show } });
        }
        Action::Lock(k) | Action::Unlock(k) => {
            let Some(ix) = prop(cx, k) else { return };
            let lock = matches!(a, Action::Lock(_));
            let p = &mut cx.zone.props[ix as usize];
            p.locked = lock;
            let id = p.id;
            if cx.cat.story.prop(p.def).gate {
                p.solid = lock;
                cx.rt.touch_prop(cx.zone, ix);
                // The interact unit: a gate that drops never drops on anyone (`clearFootprint`).
            }
            cx.emit(EventKind::Prop { prop: id, change: if lock { PropChange::Lock } else { PropChange::Unlock } });
        }
        Action::Switch { prop: k, on } => {
            let Some(ix) = prop(cx, k) else { return };
            let p = &mut cx.zone.props[ix as usize];
            p.on = on.unwrap_or(!p.on);
            let id = p.id;
            cx.emit(EventKind::Prop { prop: id, change: PropChange::Switch });
        }
        Action::Spawn { key, def, at } => {
            let key = cx.sym(key);
            if cx.rt.unit_names.contains(&key) || cx.ops.spawn.iter().any(|u| u.key == Some(key)) {
                return;
            }
            let at = cx.sym(at);
            let Some(mark) = cx.rt.mark(at) else {
                cx.emit(EventKind::Missing(at));
                return;
            };
            let (mx, my) = (i32::from(mark.cell.x), i32::from(mark.cell.y));
            let (fx, fy) = cx.rt.grid.nearest_free(mx, my, SPAWN_RADIUS, None).unwrap_or((mx, my));
            let id = cx.world.next.unit();
            let u = new_unit(id, Some(key), def, Vec2::centre(fx, fy), mark.facing.unwrap_or_default(), cx.world.tick);
            cx.ops.spawn.push(u);
        }
        Action::Despawn(k) => {
            let k = cx.sym(k);
            if let Some(&id) = cx.rt.unit_names.get(&k) {
                if cx.party.seat_of(id).is_none() && !cx.ops.despawn.contains(&id) {
                    cx.ops.despawn.push(id);
                }
            }
        }
        Action::Location(k) => {
            // The quests unit counts it and the journal unit records it; the flag is the world's.
            let k = FlagKey::Been(cx.sym(k));
            set_flag(cx, k, 1);
        }
        Action::If { when, then, els } => {
            let met = conditions_met(cx, conds(cx, when));
            if met {
                run_actions(cx, then, subject);
            } else if let Some(e) = els {
                run_actions(cx, e, subject);
            }
        }
        Action::Travel { zone, mark } => {
            let mark = cx.sym(mark);
            request_travel(cx, TravelRequest { zone, mark, at: None });
        }
        // Later units' verbs, no-ops until they land.
        // The quests unit:
        Action::Quest(_)
        | Action::HandIn(_)
        // the interact unit:
        | Action::Rest { .. }
        | Action::Grow { .. }
        | Action::Read(_)
        | Action::Fill { .. }
        | Action::Reveal(_)
        | Action::Camera { .. }
        | Action::Shake(_)
        | Action::Aggro(_)
        | Action::Send { .. }
        // the inventory unit:
        | Action::Give(_)
        | Action::Take(_)
        | Action::Throw(_)
        // the combat unit:
        | Action::Learn(_)
        | Action::Strike { .. }
        | Action::Status(_)
        | Action::Heal(_)
        // the dialogue unit:
        | Action::Talk(_) => {}
    }
}

/// Ask to change zone; performed in step 14. Refused while carrying something, and without an
/// actor.
pub fn request_travel(cx: &mut Ctx<'_>, req: TravelRequest) {
    let Some(seat) = cx.actor else { return };
    let Some(body) = cx.actor_unit() else { return };
    if cx.zone.unit(body).is_some_and(|u| u.carrying.is_some()) {
        cx.emit(EventKind::Toast(ToastKind::PutDownFirst));
        return;
    }
    if let Some(p) = cx.world.players.get_mut(seat.index()) {
        p.travel = Some(req);
    }
}

/// A door leads somewhere: travel through it. The interact unit's `Use` calls this.
pub fn door_travel(cx: &mut Ctx<'_>, ix: PropIx) {
    let p = &cx.zone.props[ix as usize];
    let Some(spawn) = p.spawn else { return };
    let Some(to) = cx.bp.props.get(usize::from(spawn)).and_then(|s| s.to) else { return };
    let mark = cx.sym(to.mark);
    request_travel(cx, TravelRequest { zone: to.zone, mark, at: None });
}
