//! C5: the verb is taught before it is demanded, and first used in safety. "Taught before
//! demanded" falls out of the gated solve (a lock that wants a spell she has not learned stays
//! shut). "In safety" does not: the room that grants a spell she did not bring must hold the
//! thing that teaches it, something to try it on with the materials that wants, and no enemy
//! that is not what guards the teacher.

use jane_core::action::{Action, Condition};
use jane_core::blueprint::PropSpawn;
use jane_core::ids::SpellId;
use jane_data::{Faction, MissionGrant};

use super::{Check, Ctx, Fault, answers_of};
use crate::solve::rows::{conds, each_action};

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let (bp, cat, m) = (c.bp, c.cat, c.m);
    let mut out = Vec::new();
    for room in &c.info.rooms {
        let node = &m.nodes[room.node];
        for grant in node.grants {
            let MissionGrant::Verb(verb) = *grant else { continue };
            if m.given_verbs.contains(&verb) {
                continue;
            }
            let spell = cat.combat.spells.get(verb.index()).map_or("?", |s| s.id);
            let answers = answers_of(verb);
            let here: Vec<&PropSpawn> =
                bp.props.iter().filter(|p| room.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y))).collect();
            let teacher = here.iter().find(|p| teaches(c, p, verb));
            let first_use = here.iter().find(|p| cat.story.prop(p.def).answers == answers && answers.is_some());
            if teacher.is_none() {
                out.push(Fault::new(Check::C5, format!("nothing in {} teaches {spell}", node.id)));
            }
            let Some(first_use) = first_use else {
                out.push(Fault::new(Check::C5, format!("{} teaches {spell} and has nothing to try it on", node.id)));
                continue;
            };
            for need in &first_use.needs {
                let have: i32 = here
                    .iter()
                    .flat_map(|p| p.loot.iter())
                    .filter(|s| s.item == need.item)
                    .map(|s| i32::from(s.qty))
                    .sum();
                if have < i32::from(need.qty) {
                    let item = cat.combat.items.get(need.item.index()).map_or("?", |d| d.id);
                    out.push(Fault::new(
                        Check::C5,
                        format!("{} wants {} {item} and {} holds {have}", c.name(first_use.key), need.qty, node.id),
                    ));
                }
            }
            // The teacher's guard: the units whose death its `<key>_free` trigger waits on.
            let mut guards = Vec::new();
            if let Some(t) = teacher {
                let free = format!("{}_free", c.name(t.key));
                if let Some((_, trig)) = bp.triggers.iter().find(|(k, _)| c.name(**k) == free) {
                    for cond in trig.when.and_then(|w| conds(bp, cat, w)).unwrap_or(&[]) {
                        if let Condition::Dead(u) = cond.c {
                            guards.push(u);
                        }
                    }
                }
            }
            for u in &bp.units {
                if !room.rect.contains(i32::from(u.cell.x), i32::from(u.cell.y)) {
                    continue;
                }
                if cat.combat.units.get(u.def.index()).is_some_and(|d| d.faction == Faction::Friendly) {
                    continue;
                }
                if !guards.contains(&u.key) {
                    out.push(Fault::new(
                        Check::C5,
                        format!("{spell} can be learned in {} while {} is still up", node.id, c.name(u.key)),
                    ));
                }
            }
        }
    }
    out
}

/// Does working this prop, or talking to it, teach the spell?
pub fn teaches(c: &Ctx<'_>, p: &PropSpawn, verb: SpellId) -> bool {
    let (bp, cat) = (c.bp, c.cat);
    let mut found = false;
    let mut look = |r| {
        each_action(bp, cat, r, &mut |a| {
            if *a == Action::Learn(verb) {
                found = true;
            }
        });
    };
    if let Some(u) = p.use_list {
        look(u);
    }
    if let Some(tree) = p.talk.and_then(|t| cat.story.dialogue.get(t.index())) {
        for n in tree.nodes {
            if let Some(a) = n.actions {
                look(a);
            }
            for o in n.options {
                if let Some(a) = o.actions {
                    look(a);
                }
            }
        }
    }
    found
}
