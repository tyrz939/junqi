//! Kills: the solver assumes fights are won. Every hostile unit standing on a reached cell dies,
//! drops what it is sure to drop and runs its death rows; so does every unit a trigger stood up,
//! once its mark is reached.

use jane_data::Faction;

use super::Changed;
use crate::solve::model::Solve;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    let bp = s.bp;
    for u in &bp.units {
        if s.dead.contains(&u.key) || !s.layers.seen_any(i32::from(u.cell.x), i32::from(u.cell.y)) {
            continue;
        }
        if s.cat.combat.units.get(u.def.index()).is_none_or(|d| d.faction == Faction::Friendly) {
            continue;
        }
        s.killed(u.key, u.def);
        changed = true;
    }
    let mut n = s.waiting.len();
    while n > 0 {
        n -= 1;
        let w = s.waiting[n];
        let Some(m) = s.bp.marks.get(&w.at) else { continue };
        if !s.layers.seen_any(i32::from(m.cell.x), i32::from(m.cell.y)) {
            continue;
        }
        s.waiting.remove(n);
        s.killed(w.unit, w.def);
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;

    use super::*;
    use crate::solve::model::Waiting;
    use crate::solve::sketch::Sketch;

    #[test]
    fn reached_hostiles_die_friends_do_not_and_a_waiting_unit_dies_on_its_mark() {
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#..#..#", "#######"]);
        k.mark("start", 1, 1);
        let rat = k.unit("rat", "rat", 2, 1);
        let dog = k.unit("dog", "dog", 1, 1);
        let far = k.unit("far", "skeleton", 5, 1);
        let there = k.mark("there", 2, 1);
        let lock_in = k.name("lock_in_1");
        let skeleton = k.cat.combat.unit_id("skeleton").unwrap();
        k.with(|s| {
            s.flood_all();
            s.waiting.push(Waiting { unit: lock_in, def: skeleton, at: there });
            assert!(run(s));
            let dead: Vec<_> = s.dead.iter().copied().collect();
            assert!(dead.contains(&s.bp.units[rat].key) && dead.contains(&lock_in));
            assert!(!dead.contains(&s.bp.units[dog].key) && !dead.contains(&s.bp.units[far].key));
            assert!(s.waiting.is_empty());
            assert!(!run(s));
        });
    }
}
