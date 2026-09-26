//! The one verb runner (ARCHITECTURE.md §5, `sim/actions.ts`): dialogue options, prop use,
//! triggers, quest rewards, item use, unit death, clock rows and the console all speak it. The
//! match is exhaustive, so a new verb is a compile error here.
//!
//! Every verb but the combat unit's (`Aggro`, `Strike`, `Status`, `Heal`, no-ops until it lands)
//! is handled here or in the module it names. Player-scoped verbs (`Give`, `Take`, `Rest`,
//! `Travel`, `Talk`, `Read`) do nothing with `actor == None`. A name that does not resolve is a
//! no-op plus `EventKind::Missing`.

use jane_core::action::{FlagOp, FlagTest};
use jane_core::{Action, Cond, Condition, CondsRef, ListRef, TextRef, Vec2};

use crate::clear::{clear_footprint, fill_rect};
use crate::ctx::Ctx;
use crate::event::{EventKind, PropChange, ToastKind};
use crate::ids::PropIx;
use crate::journal;
use crate::state::{FactKey, FlagKey, Source, Speaker, TravelRequest};
use crate::tuning::SPAWN_RADIUS;
use crate::units::new_unit;
use crate::{dialogue, inventory, orders, quests, verbs};

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

/// A condition list, from the catalog or this zone's blueprint.
pub fn conds<'b>(cx: &Ctx<'b>, r: CondsRef) -> &'b [Cond] {
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

/// A content fact in this zone's names.
pub fn fact_key(cx: &Ctx<'_>, f: jane_core::action::FactKey) -> FactKey {
    use jane_core::action::FactKey as C;
    match f {
        C::Place(k) => FactKey::Place(cx.sym(k)),
        C::Person(k) => FactKey::Person(cx.sym(k)),
        C::Thing(t) => FactKey::Thing(t),
        C::Claim(t) => FactKey::Claim(t),
        C::Route(a, b) => FactKey::Route(cx.sym(a), cx.sym(b)),
        C::Danger(k) => FactKey::Danger(cx.sym(k)),
        C::Rumour(s) => FactKey::Rumour(s),
    }
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
        Condition::QuestActive(q) => quests::active(cx.world, q).is_some(),
        Condition::QuestReady(q) => quests::ready(cx.world, q),
        Condition::QuestDone(q) => quests::done(cx.world, q),
        // The world's: what any of them has learned, all of them know.
        Condition::HasSpell(s) => cx.world.growth.spells.contains(&s),
        // Hers, not the party's: what she is holding. False with no actor.
        Condition::HasItem(stack) => {
            let Some(p) = cx.actor.and_then(|s| cx.world.player(s)) else { return false };
            crate::bag::bag_count(&p.bag[..], stack.item) >= u32::from(stack.qty)
        }
        // A unit here by that name is asked; one that is not here, the world's flag.
        Condition::Dead(k) => {
            let s = cx.sym(k);
            match cx.rt.unit_names.get(&s).and_then(|&id| cx.zone.unit(id)) {
                Some(u) => !u.alive,
                None => flag(cx, FlagKey::Dead(s)) != 0,
            }
        }
        Condition::Knows(f) => journal::knows(cx.world, fact_key(cx, f)),
        Condition::Heard(t) => journal::heard(cx.world, t),
        // The living-world unit (§4.6.e): false outside a conversation until it lands.
        Condition::SpeakerKnows(_) => false,
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

/// The unit a verb lands on: the subject if it is a unit here, else the actor's body.
fn subject_unit(cx: &Ctx<'_>, subject: Subject) -> Option<crate::ids::UnitId> {
    match subject {
        Subject::Unit(id) if cx.zone.unit(id).is_some() => Some(id),
        _ => cx.actor_unit(),
    }
}

pub fn run_action(cx: &mut Ctx<'_>, a: &Action, subject: Subject) {
    match *a {
        Action::Quest(q) => {
            quests::give(cx, q);
        }
        Action::HandIn(q) => {
            quests::hand_in(cx, q);
        }
        Action::Flag { key, op } => {
            let k = flag_key(cx, key);
            let v = match op {
                FlagOp::Set(v) => v,
                FlagOp::Add(d) => flag(cx, k) + d,
            };
            set_flag(cx, k, v);
        }
        Action::Rest { until } => verbs::rest(cx, until),
        Action::Grow { stat, amount, id } => {
            let id = cx.sym(id);
            verbs::grow(cx, stat, amount, id);
        }
        Action::Give(s) => inventory::give(cx, s.item, s.qty),
        Action::Take(s) => {
            if let (Some(seat), Some(_)) = (cx.actor, cx.actor_unit()) {
                inventory::remove(cx, seat, s.item, s.qty);
            }
        }
        Action::Learn(spell) => {
            if cx.actor.is_some() {
                verbs::teach(cx.world, cx.events, spell);
            }
        }
        Action::Toast(t) => cx.emit(EventKind::Toast(ToastKind::Text(t))),
        Action::Read(t) => {
            // A read with nobody to read it is a toast.
            if cx.actor.is_none() {
                cx.emit(EventKind::Toast(ToastKind::Text(t)));
                return;
            }
            dialogue::read(cx, t);
            if let TextRef::Text(id) = t {
                journal::learn(cx, FactKey::Claim(id), Source::Read);
            }
        }
        Action::Show(k) | Action::Hide(k) => {
            let Some(ix) = prop(cx, k) else { return };
            let hide = matches!(a, Action::Hide(_));
            let p = &mut cx.zone.props[ix as usize];
            p.hidden = hide;
            let id = p.id;
            cx.rt.touch_prop(cx.zone, ix);
            // A shown prop stands aside whoever it landed on.
            if !hide {
                clear_footprint(cx, ix);
            }
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
                // A gate that drops never drops on anyone: whoever is under it is stood aside.
                if lock {
                    clear_footprint(cx, ix);
                }
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
            let k = cx.sym(k);
            quests::on_location(cx, k);
            journal::learn(cx, FactKey::Place(k), Source::Visited);
        }
        Action::Fill { rect, tile } => {
            let s = cx.sym(rect);
            match cx.rt.rects.get(&s).copied() {
                // A solid tile waits for anyone standing in the rect to step out of it.
                Some(r) => fill_rect(cx, r, tile),
                None => cx.emit(EventKind::Missing(s)),
            }
        }
        Action::Travel { zone, mark } => {
            let mark = cx.sym(mark);
            request_travel(cx, TravelRequest { zone, mark, at: None });
        }
        Action::Talk(tree) => {
            if cx.actor.is_some() {
                let speaker = match subject {
                    Subject::Unit(id) => Speaker::Unit(id),
                    Subject::Prop(id) => Speaker::Prop(id),
                    Subject::None => Speaker::None,
                };
                dialogue::start(cx, tree, speaker);
            }
        }
        Action::Throw(item) => {
            let who = subject_unit(cx, subject);
            verbs::throw(cx, who, item);
        }
        Action::Shake(n) => cx.emit(EventKind::Shake(n)),
        Action::Camera { mode, rect } => {
            let rect = rect.map(|r| cx.sym(r));
            cx.emit(EventKind::Camera { mode, rect });
        }
        Action::If { when, then, els } => {
            // Asked of whoever is acting, like any condition: `HasItem` is hers, flags are the world's.
            let met = conditions_met(cx, conds(cx, when));
            if met {
                run_actions(cx, then, subject);
            } else if let Some(e) = els {
                run_actions(cx, e, subject);
            }
        }
        Action::Send { unit, to, then } => {
            let (u, m) = (cx.sym(unit), cx.sym(to));
            let (Some(&id), Some(mark)) = (cx.rt.unit_names.get(&u), cx.rt.mark(m)) else { return };
            orders::send(cx, id, Vec2::centre(i32::from(mark.cell.x), i32::from(mark.cell.y)), then);
        }
        Action::Reveal(names) => {
            let keys: &[jane_core::Key] = match names {
                jane_core::NamesRef::Catalog(_) => cx.cat.names_of(names),
                jane_core::NamesRef::Blueprint(_) => {
                    let bp = cx.bp;
                    bp.names_of(names).unwrap_or(&[])
                }
            };
            for &k in keys {
                let s = cx.sym(k);
                if let Some(r) = cx.rt.rects.get(&s).copied() {
                    verbs::reveal_rect(cx, r);
                }
            }
        }
        // The combat unit's verbs, no-ops until it lands.
        Action::Aggro(_) | Action::Strike { .. } | Action::Status(_) | Action::Heal(_) => {}
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
