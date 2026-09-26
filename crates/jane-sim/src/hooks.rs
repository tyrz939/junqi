//! Where combat calls into systems other units own (PORT.md §8: two agents never own one file,
//! so the seams are here, small and named). The signature is the contract; each body hands the
//! call to the module that owns what it does. Nothing else in combat reaches into those systems.
//!
//! | Hook | Owner | Called from |
//! | --- | --- | --- |
//! | [`world_verb`] | interact (`interact.ts worldVerb`) | a `World` spell's cast |
//! | [`school_touch`] | interact (`interact.ts schoolTouch`) | a bolt's end (step 8) |
//! | [`use_item`] | inventory (`inventory.ts useItem`) | `Command::Bar` on an item slot |
//! | [`quest_kill`] | quests (`quests.ts onUnitKilled`) | a kill by one of the party (step 10) |
//! | [`unit_died`] | journal, living world (§3.7, §4.6.c) | every creature's death (step 10) |
//! | [`respawn_allowed`] | living world (§4.6.c ecology) | a corpse due to stand up (step 12) |
//! | [`put_down_dead`] | interact (`sim.ts revivePlayer`, `moveProp`) | a seat waking (step 6) |
//! | [`reset_lock_ins`] | triggers (`sim.ts revivePlayer`, trigger `reset`) | a seat waking (step 6) |

use jane_core::action::School;
use jane_core::{Cell, Fx, ItemId, UnitDefId, Vec2};
use jane_data::WorldSpell;

use crate::ctx::Ctx;
use crate::event::{EventKind, ToastKind};
use crate::ids::{Seat, UnitId};
use crate::interact::{world_spell_on, world_spell_target};
use crate::state::{FactKey, Source};
use crate::tuning::SPAWN_RADIUS;
use crate::{interact, inventory, journal, quests, triggers};

/// A `World` spell (Repair, Grow) does its verb to the prop in front of `caster`: the nearest
/// unused one within 2 m that answers it (Grow passes over anything standing in the dark), paying
/// what the prop `needs` from her bag, then marking it used and on and running its `use` list.
/// Returns whether it found something to do; `false` makes the cast fail and cost nothing, and
/// says why ("Nothing here to repair", "Nothing grows without light", "Needs 2x Rock").
pub fn world_verb(cx: &mut Ctx<'_>, caster: UnitId, verb: WorldSpell) -> bool {
    let (target, shaded) = world_spell_target(cx, caster, verb);
    let Some(ix) = target else {
        let why = match verb {
            WorldSpell::Repair => ToastKind::NothingToRepair,
            WorldSpell::Grow if shaded => ToastKind::NothingGrowsWithoutLight,
            WorldSpell::Grow => ToastKind::NothingGrows,
        };
        cx.emit(EventKind::Toast(why));
        return false;
    };
    world_spell_on(cx, caster, ix)
}

/// A bolt of `school` ended at `at`: every unhidden prop not yet on that answers the school and
/// whose middle is within `touch` is switched on and used, and its `use` list runs for `from`.
pub fn school_touch(cx: &mut Ctx<'_>, school: School, at: Vec2, from: Option<UnitId>, touch: Fx) {
    interact::school_touch(cx, school, at, touch, from);
}

/// A bar slot holding an item was pressed: the item is used as `Command::Item` uses it.
pub fn use_item(cx: &mut Ctx<'_>, seat: Seat, item: ItemId) {
    let before = cx.actor.replace(seat);
    inventory::use_item(cx, item);
    cx.actor = before;
}

/// One of the party (`seat`) killed a unit of `def`: kill requirements of the quests in the log
/// count, the progress is said to everyone.
pub fn quest_kill(cx: &mut Ctx<'_>, _seat: Seat, def: UnitDefId) {
    quests::on_kill(cx, def);
}

/// A creature (not a seat's body) died, killed by `slayer` if one of the party: a named one is
/// known dead (§3.7). The journal's `Danger AttackedIn` and the ecology's pressure wait on areas
/// in the blueprint and the living-world unit.
pub fn unit_died(cx: &mut Ctx<'_>, unit: UnitId, _slayer: Option<Seat>) {
    if let Some(k) = cx.zone.unit(unit).and_then(|u| u.key) {
        journal::learn(cx, FactKey::Person(k), Source::Dead);
    }
}

/// May this corpse stand up now? The ecology says no while its def is at its area's `cap` or the
/// pressure is over the row's `hold` line, and pushes it back onto `sleeping_due` at the next
/// hour itself.
///
/// Placeholder (the living-world unit's): always.
pub fn respawn_allowed(cx: &mut Ctx<'_>, unit: UnitId) -> bool {
    let _ = (cx, unit);
    true
}

/// A seat wakes: what her body carried when she fell stays where she fell, on the nearest free
/// cell by her body (else where it was lifted from), solid as its row says. `revive_player`
/// clears `carrying` after.
pub fn put_down_dead(cx: &mut Ctx<'_>, _seat: Seat, body: UnitId) {
    let Some(u) = cx.zone.unit(body) else { return };
    let Some(pid) = u.carrying else { return };
    let (x, y) = u.pos.cell();
    let Some(pix) = cx.zone.prop_ix(pid) else { return };
    if let Some((fx, fy)) = cx.rt.grid.nearest_free(x, y, SPAWN_RADIUS, None) {
        cx.rt.move_prop(cx.zone, pix, Cell::new(fx as u16, fy as u16));
    }
    let p = &mut cx.zone.props[pix as usize];
    p.solid = cx.cat.story.prop(p.def).solid;
    cx.rt.touch_prop(cx.zone, pix);
}

/// A seat wakes: every fired trigger with a `reset` in her zone undoes itself and re-arms, unless
/// a friend still alive stands in its rect (so a death never leaves a gate shut in her face, and
/// with company a lock-in holds while someone is still inside).
pub fn reset_lock_ins(cx: &mut Ctx<'_>, seat: Seat, _body: UnitId) {
    triggers::reset_on_death(cx, seat);
}

/// The hooks from the combat unit's side, with a context made the way the step makes one
/// (`verbs2.test.ts` E7/E8 "Grow, end to end", `dungeon-verbs.test.ts` "Repair", `quests.ts
/// onUnitKilled`, the trigger `reset` of `sim.ts revivePlayer`).
#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use jane_core::action::FlagOp;
    use jane_core::blueprint::{Mark, PropSpawn, Trigger, TriggerMode};
    use jane_core::{Action, Blueprint, Cell, FlagKey as ContentFlag, Key, Rect, Stack, Tile, ZoneId};

    use super::*;
    use crate::blueprints::Blueprints;
    use crate::ctx::PartySnap;
    use crate::sim::Sim;
    use crate::state::FlagKey;

    fn real() -> Blueprints {
        static B: OnceLock<Blueprints> = OnceLock::new();
        B.get_or_init(|| Blueprints::build(7).expect("seed 7 builds")).clone()
    }

    fn spawn(key: Key, row: &str, x: u16, y: u16) -> PropSpawn {
        PropSpawn {
            key,
            def: jane_data::catalog().story.prop_id(row).unwrap(),
            cell: Cell::new(x, y),
            locked: false,
            key_tag: None,
            hidden: false,
            on: false,
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
        }
    }

    /// A floor for the county, `start` at (6, 16); `f` furnishes it.
    fn room(f: impl FnOnce(&mut Blueprint)) -> Sim {
        let cat = jane_data::catalog();
        let mut bp = Blueprint::new(ZoneId::County, 48, 32, Tile::Floor);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(6, 16), facing: None });
        f(&mut bp);
        let real = real();
        let bp = Arc::new(bp);
        let zones =
            std::array::from_fn(|i| if i == 0 { Arc::clone(&bp) } else { Arc::clone(real.get(ZoneId::ALL[i])) });
        Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane")
    }

    fn in_ctx<R>(s: &mut Sim, f: impl FnOnce(&mut Ctx<'_>, UnitId) -> R) -> R {
        let body = s.state.players[0].unit;
        let snap = PartySnap::of(&s.state);
        s.with_ctx(ZoneId::County, Some(Seat(0)), &snap, false, |cx| f(cx, body))
    }

    fn flag(s: &Sim, name: &str) -> i32 {
        let k = FlagKey::Named(s.state.syms.find(name).unwrap());
        s.state.flags.get(&k).copied().unwrap_or(0)
    }

    fn toasts(s: &mut Sim) -> Vec<ToastKind> {
        s.drain_events()
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Toast(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    fn item(id: &str) -> jane_core::ItemId {
        jane_data::catalog().combat.item_id(id).unwrap()
    }

    #[test]
    fn repair_pays_its_needs_and_costs_nothing_when_it_cannot() {
        let mut s = room(|bp| {
            let mended = bp.local("mended");
            let list = bp.push_list(vec![Action::Flag { key: ContentFlag::Named(mended), op: FlagOp::Set(1) }]);
            let k = bp.local("steps");
            let mut p = spawn(k, "broken_steps", 8, 15);
            p.needs = vec![Stack { item: item("rock"), qty: 2 }];
            p.use_list = Some(list);
            bp.props.push(p);
        });
        s.drain_events();
        assert!(!in_ctx(&mut s, |cx, me| world_verb(cx, me, WorldSpell::Repair)));
        assert!(toasts(&mut s).contains(&ToastKind::Needs { item: item("rock"), qty: 2 }));
        crate::bag::bag_add(&mut s.state.players[0].bag[..], item("rock"), 3);
        assert!(in_ctx(&mut s, |cx, me| world_verb(cx, me, WorldSpell::Repair)));
        assert_eq!(crate::bag::bag_count(&s.state.players[0].bag[..], item("rock")), 1);
        let p = &s.state.zone(ZoneId::County).unwrap().props[0];
        assert!(p.used && p.on);
        assert_eq!(flag(&s, "mended"), 1);
        // Used: there is nothing left to repair.
        assert!(!in_ctx(&mut s, |cx, me| world_verb(cx, me, WorldSpell::Repair)));
        assert!(toasts(&mut s).contains(&ToastKind::NothingToRepair));
    }

    #[test]
    fn nothing_grows_in_the_dark_and_in_the_light_the_buds_list_runs() {
        let mut s = room(|bp| {
            let grown = bp.local("grown");
            let list = bp.push_list(vec![Action::Flag { key: ContentFlag::Named(grown), op: FlagOp::Set(1) }]);
            let k = bp.local("bud");
            let mut p = spawn(k, "bud", 8, 16);
            p.use_list = Some(list);
            bp.props.push(p);
            let t = bp.local("torch");
            let mut torch = spawn(t, "torch", 9, 18);
            torch.hidden = true;
            bp.props.push(torch);
        });
        s.drain_events();
        assert!(!in_ctx(&mut s, |cx, me| world_verb(cx, me, WorldSpell::Grow)));
        assert!(toasts(&mut s).contains(&ToastKind::NothingGrowsWithoutLight));
        assert_eq!(flag(&s, "grown"), 0);
        // A torch lit beside it.
        s.state.zone_mut(ZoneId::County).unwrap().props[1].hidden = false;
        assert!(in_ctx(&mut s, |cx, me| world_verb(cx, me, WorldSpell::Grow)));
        assert_eq!(flag(&s, "grown"), 1);
        assert!(s.state.zone(ZoneId::County).unwrap().props[0].on, "it is on, and its own light shows");
    }

    #[test]
    fn a_frost_bolt_lights_a_cold_torch_once() {
        let mut s = room(|bp| {
            let lit = bp.local("lit");
            let list = bp.push_list(vec![Action::Flag { key: ContentFlag::Named(lit), op: FlagOp::Add(1) }]);
            let k = bp.local("cold");
            let mut p = spawn(k, "torch_blue", 20, 20);
            p.use_list = Some(list);
            bp.props.push(p);
        });
        let at = jane_core::Vec2::centre(20, 20);
        let near = jane_core::Fx::from_px(14);
        in_ctx(&mut s, |cx, me| school_touch(cx, School::Fire, at, Some(me), near));
        assert_eq!(flag(&s, "lit"), 0, "fire does not light a frost torch");
        in_ctx(&mut s, |cx, me| school_touch(cx, School::Frost, at, Some(me), near));
        in_ctx(&mut s, |cx, me| school_touch(cx, School::Frost, at, Some(me), near));
        assert_eq!(flag(&s, "lit"), 1, "once on, a second bolt does nothing");
        let p = &s.state.zone(ZoneId::County).unwrap().props[0];
        assert!(p.on && p.used);
    }

    #[test]
    fn a_kill_counts_for_quests_in_the_log_and_a_named_victim_is_known_dead() {
        let cat = jane_data::catalog();
        let mut s = room(|_| {});
        let skeleton = cat.combat.unit_id("skeleton").unwrap();
        let q = cat.story.quest_id("defeat_skeleton").unwrap();
        // Not in the log: nothing counts.
        in_ctx(&mut s, |cx, _| quest_kill(cx, Seat(0), skeleton));
        in_ctx(&mut s, |cx, _| {
            crate::quests::give(cx, q);
        });
        assert!(!crate::quests::ready(&s.state, q));
        s.drain_events();
        in_ctx(&mut s, |cx, _| quest_kill(cx, Seat(0), skeleton));
        assert!(crate::quests::ready(&s.state, q));
        let ev = s.drain_events().to_vec();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Toast(ToastKind::KillProgress { quest: q, req: 0, n: 1, of: 1 }))
        );
        assert!(ev.iter().any(|e| e.kind == EventKind::Quest { quest: q, change: crate::event::QuestChange::Ready }));
        // Counted to its quantity and no further.
        in_ctx(&mut s, |cx, _| quest_kill(cx, Seat(0), skeleton));
        assert_eq!(crate::quests::active(&s.state, q).unwrap().counts[0], 1);
    }

    #[test]
    fn a_death_undoes_a_lock_in_and_re_arms_it_unless_a_friend_is_still_inside() {
        let mut s = room(|bp| {
            let gate = bp.local("gate");
            let mut g = spawn(gate, "gate_h", 30, 10);
            g.locked = false;
            bp.props.push(g);
            let rect = bp.local("arena");
            bp.rects.insert(rect, Rect::new(20, 12, 10, 10));
            let actions = bp.push_list(vec![Action::Lock(gate)]);
            let reset = bp.push_list(vec![Action::Unlock(gate)]);
            bp.triggers.insert(
                rect,
                Trigger { rect, mode: TriggerMode::Enter, once: true, when: None, actions, reset: Some(reset) },
            );
        });
        let body = s.state.players[0].unit;
        s.state.zone_mut(ZoneId::County).unwrap().unit_mut(body).unwrap().pos = jane_core::Vec2::centre(25, 15);
        s.rebuild_runtimes();
        s.step(&crate::input::StepInput::IDLE);
        assert!(s.state.zone(ZoneId::County).unwrap().props[0].locked, "the lock-in fired");
        in_ctx(&mut s, |cx, me| reset_lock_ins(cx, Seat(0), me));
        let z = s.state.zone(ZoneId::County).unwrap();
        assert!(!z.props[0].locked && !z.triggers.fired.get(0), "undone and re-armed");
    }
}
