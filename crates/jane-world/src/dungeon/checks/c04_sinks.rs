//! C4: materials cannot be starved. Everything every sink (a prop's `needs`) can eat is no more
//! than the zone supplies: loot, what `use` lists give, and the certain drops of its units (a
//! thing that respawns is a supply without end; only what is certain the first time counts).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::vec::Vec;

use jane_core::action::Action;
use jane_core::ids::ItemId;

use super::{Check, Ctx, Fault};
use crate::solve::rows::each_action;

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let (bp, cat) = (c.bp, c.cat);
    let mut supply: BTreeMap<ItemId, i32> = BTreeMap::new();
    let mut sinks: BTreeMap<ItemId, i32> = BTreeMap::new();
    for p in &bp.props {
        for s in &p.loot {
            *supply.entry(s.item).or_insert(0) += i32::from(s.qty);
        }
        if let Some(u) = p.use_list {
            each_action(bp, cat, u, &mut |a| {
                if let Action::Give(s) = *a {
                    *supply.entry(s.item).or_insert(0) += i32::from(s.qty);
                }
            });
        }
        for n in &p.needs {
            *sinks.entry(n.item).or_insert(0) += i32::from(n.qty);
        }
    }
    for u in &bp.units {
        let Some(row) = cat.combat.units.get(u.def.index()) else { continue };
        for l in row.loot.iter().filter(|l| l.chance.0 >= 1000) {
            *supply.entry(l.item).or_insert(0) += i32::from(l.qty);
        }
    }
    // Item ids are in id-string order, which is the TypeScript's sort.
    let mut out = Vec::new();
    for (&item, &eats) in &sinks {
        let has = supply.get(&item).copied().unwrap_or(0);
        if eats > has {
            let id = cat.combat.items.get(item.index()).map_or("?", |d| d.id);
            out.push(Fault::new(Check::C4, format!("the zone can eat {eats} {id} and supplies {has}")));
        }
    }
    out
}
