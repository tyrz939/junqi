//! Where combat reaches the world verbs. The combat unit calls these by name and owns none of
//! what they do: a world spell landing on a prop, a bolt's school touching a prop, a unit dying.
//!
//! | Hook | Called by | Does |
//! | --- | --- | --- |
//! | [`on_world_spell`] | a `World` spell cast (`combat.ts tryCast`, kind `world`) | finds the prop (or takes the one given), pays its `needs`, switches it on, runs its list |
//! | [`on_school_touch`] | a bolt ending ([`crate::interact::school_touch`] finds the props) | switches one answering prop on, runs its list |
//! | [`on_kill`] | the flush, when a unit dies | kill counts for active quests; the journal's Person Dead |

use jane_core::action::School;
use jane_core::{Sym, UnitDefId};
use jane_data::WorldSpell;

use crate::actions::{Subject, run_actions};
use crate::ctx::Ctx;
use crate::event::{EventKind, PropChange, ToastKind};
use crate::ids::{PropIx, Seat, UnitId};
use crate::interact::{spawn_of, world_spell_on, world_spell_target};
use crate::journal;
use crate::quests;
use crate::state::{FactKey, Source};

/// A world spell (Repair, Grow) cast by `caster`. With `prop`, on that prop if it answers;
/// without, on the nearest prop within 2 m that answers (Grow passes over anything in the dark).
/// Returns whether the cast was valid: an invalid one costs nothing (2020), and says why.
pub fn on_world_spell(cx: &mut Ctx<'_>, caster: UnitId, verb: WorldSpell, prop: Option<PropIx>) -> bool {
    let (target, shaded) = match prop {
        Some(ix) => (Some(ix), false),
        None => world_spell_target(cx, caster, verb),
    };
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

/// A bolt of `school` touched prop `ix` (it answers the school and is not on): it switches on,
/// is used, and runs its list with `from` as the subject.
pub fn on_school_touch(cx: &mut Ctx<'_>, _school: School, ix: PropIx, from: Option<UnitId>) {
    let p = &mut cx.zone.props[ix as usize];
    if p.hidden || p.on {
        return;
    }
    p.on = true;
    p.used = true;
    let pid = p.id;
    let bp = cx.bp;
    if let Some(list) = spawn_of(bp, &cx.zone.props[ix as usize]).and_then(|s| s.use_list) {
        run_actions(cx, list, from.map_or(Subject::None, Subject::Unit));
    }
    cx.emit(EventKind::Prop { prop: pid, change: PropChange::Switch });
}

/// A unit of `victim_def` (keyed `victim_key`) died, by `killer_seat`'s hand if a seat's. Every
/// active quest that counts it counts it, whoever made the kill (a friend's kill counts), and a
/// named victim is known dead.
pub fn on_kill(cx: &mut Ctx<'_>, _killer_seat: Option<Seat>, victim_def: UnitDefId, victim_key: Option<Sym>) {
    quests::on_kill(cx, victim_def);
    if let Some(k) = victim_key {
        journal::learn(cx, FactKey::Person(k), Source::Dead);
    }
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
        assert!(!in_ctx(&mut s, |cx, me| on_world_spell(cx, me, WorldSpell::Repair, None)));
        assert!(toasts(&mut s).contains(&ToastKind::Needs { item: item("rock"), qty: 2 }));
        crate::bag::bag_add(&mut s.state.players[0].bag[..], item("rock"), 3);
        assert!(in_ctx(&mut s, |cx, me| on_world_spell(cx, me, WorldSpell::Repair, None)));
        assert_eq!(crate::bag::bag_count(&s.state.players[0].bag[..], item("rock")), 1);
        let p = &s.state.zone(ZoneId::County).unwrap().props[0];
        assert!(p.used && p.on);
        assert_eq!(flag(&s, "mended"), 1);
        // Used: there is nothing left to repair.
        assert!(!in_ctx(&mut s, |cx, me| on_world_spell(cx, me, WorldSpell::Repair, None)));
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
        assert!(!in_ctx(&mut s, |cx, me| on_world_spell(cx, me, WorldSpell::Grow, None)));
        assert!(toasts(&mut s).contains(&ToastKind::NothingGrowsWithoutLight));
        assert_eq!(flag(&s, "grown"), 0);
        // A torch lit beside it.
        s.state.zone_mut(ZoneId::County).unwrap().props[1].hidden = false;
        assert!(in_ctx(&mut s, |cx, me| on_world_spell(cx, me, WorldSpell::Grow, None)));
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
        in_ctx(&mut s, |cx, me| crate::interact::school_touch(cx, School::Fire, at, near, Some(me)));
        assert_eq!(flag(&s, "lit"), 0, "fire does not light a frost torch");
        in_ctx(&mut s, |cx, me| crate::interact::school_touch(cx, School::Frost, at, near, Some(me)));
        in_ctx(&mut s, |cx, me| crate::interact::school_touch(cx, School::Frost, at, near, Some(me)));
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
        in_ctx(&mut s, |cx, _| on_kill(cx, Some(Seat(0)), skeleton, None));
        in_ctx(&mut s, |cx, _| {
            crate::quests::give(cx, q);
        });
        assert!(!crate::quests::ready(&s.state, q));
        s.drain_events();
        let key = s.state.syms.find("dog").unwrap();
        in_ctx(&mut s, |cx, _| on_kill(cx, None, skeleton, Some(key)));
        assert!(crate::quests::ready(&s.state, q), "a friend's kill (or the world's) counts");
        let ev = s.drain_events().to_vec();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Toast(ToastKind::KillProgress { quest: q, req: 0, n: 1, of: 1 }))
        );
        assert!(ev.iter().any(|e| e.kind == EventKind::Quest { quest: q, change: crate::event::QuestChange::Ready }));
        assert_eq!(crate::journal::known(&s.state, FactKey::Person(key)).map(|k| k.how), Some(Source::Dead));
        // Counted to its quantity and no further.
        in_ctx(&mut s, |cx, _| on_kill(cx, None, skeleton, None));
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
        in_ctx(&mut s, |cx, _| crate::triggers::reset_on_death(cx, Seat(0)));
        let z = s.state.zone(ZoneId::County).unwrap();
        assert!(!z.props[0].locked && !z.triggers.fired.get(0), "undone and re-armed");
    }
}
