//! Loot and reading: a chest she can stand beside is emptied into her bag (keys become key tags,
//! everything else counts as itself) and its `use` runs once; a prop that talks teaches every
//! `learn` in its tree.

use super::Changed;
use crate::solve::model::Solve;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    let bp = s.bp;
    for (i, p) in bp.props.iter().enumerate() {
        if s.props[i].withheld {
            continue;
        }
        if p.talk.is_none() && p.loot.is_empty() {
            continue;
        }
        let Some(at) = s.at(i) else { continue };
        if s.held_shut(i, at) {
            continue;
        }
        if let Some(tree) = p.talk.filter(|_| !s.props[i].talked) {
            s.props[i].talked = true;
            s.mark_fired(i);
            s.read_tree(tree);
            changed = true;
        }
        if !p.loot.is_empty() && !s.props[i].looted {
            s.props[i].looted = true;
            s.mark_fired(i);
            for st in &p.loot {
                s.add_key(s.item_tag(st.item), i32::from(st.qty));
            }
            s.apply_actions(s.use_of(i), at, false, false);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::action::Stack;
    use jane_core::{Action, ZoneId};

    use super::*;
    use crate::solve::model::KeyTag;
    use crate::solve::sketch::Sketch;

    #[test]
    fn a_reached_chest_gives_its_keys_once_and_a_locked_one_nothing() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "#....#", "#....#", "######"]);
        k.mark("start", 4, 3);
        let key = k.item("key_generic");
        let wood = k.item("wood");
        let chest = k.prop("chest", "chest", 1, 1);
        k.bp.props[chest].loot = vec![Stack { item: key, qty: 1 }, Stack { item: wood, qty: 2 }];
        let flag = k.flag("test_opened");
        let l = k.list(vec![Action::Flag { key: flag, op: jane_core::action::FlagOp::Set(1) }]);
        k.bp.props[chest].use_list = Some(l);
        let shut = k.prop("shut", "chest", 3, 1);
        k.bp.props[shut].loot = vec![Stack { item: key, qty: 5 }];
        k.bp.props[shut].locked = true;
        let generic = k.name("generic");
        k.with(|s| {
            s.flood_all();
            assert!(run(s));
            assert_eq!(s.have(KeyTag::Tag(generic)), 1);
            assert_eq!(s.have(KeyTag::Item(wood)), 2);
            assert_eq!(s.flags.get(&flag), Some(&1));
            assert!(!run(s), "a chest is looted once");
            assert!(!s.props[shut].looted);
        });
    }

    #[test]
    fn a_withheld_key_is_never_had() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "#....#", "#....#", "######"]);
        k.mark("start", 4, 3);
        let key = k.item("key_generic");
        let chest = k.prop("chest", "chest", 1, 1);
        k.bp.props[chest].loot = vec![Stack { item: key, qty: 1 }];
        let generic = k.name("generic");
        k.opts.withhold.keys.push(KeyTag::Tag(generic));
        k.with(|s| {
            s.flood_all();
            assert!(run(s));
            assert_eq!(s.have(KeyTag::Tag(generic)), 0);
        });
    }
}
