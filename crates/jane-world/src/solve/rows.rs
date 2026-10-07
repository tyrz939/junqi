//! Lists a blueprint wrote are rows like any other (ARCHITECTURE.md §5.3 point 2). The catalog's
//! own were checked at build; these are checked here, per build: every id they hold is a row that
//! exists, and every name they lean on is in this blueprint.
//!
//! [`each_action`] is the TypeScript's `eachAction`: anything that reads a list for what it COULD
//! do walks it this way, so a `learn` under an `if` is still found.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use jane_core::action::{Action, Cond, Condition, CondsRef, ListRef, NamesRef, School};
use jane_core::blueprint::Blueprint;
use jane_core::ids::Key;
use jane_core::num::Milli;
use jane_data::{Catalog, NameKind};

use super::report::RowFault;

/// How deep lists may nest. A blueprint list that names itself would otherwise never end.
pub const MAX_DEPTH: u8 = 16;

pub fn list<'a>(bp: &'a Blueprint, cat: &'static Catalog, r: ListRef) -> Option<&'a [Action]> {
    match r {
        ListRef::Blueprint(i) => bp.lists.get(usize::from(i)).map(Vec::as_slice),
        ListRef::Catalog(i) => cat.lists.get(usize::from(i)).copied(),
    }
}

pub fn conds<'a>(bp: &'a Blueprint, cat: &'static Catalog, r: CondsRef) -> Option<&'a [Cond]> {
    match r {
        CondsRef::Blueprint(i) => bp.conds.get(usize::from(i)).map(Vec::as_slice),
        CondsRef::Catalog(i) => cat.conds.get(usize::from(i)).copied(),
    }
}

pub fn names<'a>(bp: &'a Blueprint, cat: &'static Catalog, r: NamesRef) -> Option<&'a [Key]> {
    match r {
        NamesRef::Blueprint(i) => bp.name_lists.get(usize::from(i)).map(Vec::as_slice),
        NamesRef::Catalog(i) => cat.name_lists.get(usize::from(i)).copied(),
    }
}

/// Every action in a list and the lists inside it (`if` both ways, `send`'s `then`), in order.
pub fn each_action<'a>(bp: &'a Blueprint, cat: &'static Catalog, r: ListRef, f: &mut dyn FnMut(&'a Action)) {
    walk(bp, cat, r, 0, f);
}

fn walk<'a>(bp: &'a Blueprint, cat: &'static Catalog, r: ListRef, depth: u8, f: &mut dyn FnMut(&'a Action)) {
    if depth > MAX_DEPTH {
        return;
    }
    for a in list(bp, cat, r).unwrap_or(&[]) {
        f(a);
        match *a {
            Action::If { then, els, .. } => {
                walk(bp, cat, then, depth + 1, f);
                if let Some(e) = els {
                    walk(bp, cat, e, depth + 1, f);
                }
            }
            Action::Send { then: Some(t), .. } => walk(bp, cat, t, depth + 1, f),
            _ => {}
        }
    }
}

/// Every dangling id and malformed row in a list and its conditions (`actionRowErrors`).
pub fn row_faults(bp: &Blueprint, cat: &'static Catalog, r: ListRef, when: Option<CondsRef>) -> Vec<RowFault> {
    let mut out = Vec::new();
    check_list(bp, cat, r, 0, &mut out);
    if let Some(w) = when {
        check_conds(bp, cat, w, &mut out);
    }
    out.dedup();
    out
}

fn check_list(bp: &Blueprint, cat: &'static Catalog, r: ListRef, depth: u8, out: &mut Vec<RowFault>) {
    if depth > MAX_DEPTH {
        out.push(RowFault::TooDeep);
        return;
    }
    let Some(l) = list(bp, cat, r) else {
        out.push(RowFault::UnknownList(r));
        return;
    };
    let c = cat;
    for a in l {
        match *a {
            Action::Quest(q) | Action::HandIn(q) if q.index() >= c.story.quests.len() => {
                out.push(RowFault::UnknownQuest(q));
            }
            Action::Give(s) | Action::Take(s) if s.item.index() >= c.combat.items.len() => {
                out.push(RowFault::UnknownItem(s.item));
            }
            Action::Throw(i) if i.index() >= c.combat.items.len() => out.push(RowFault::UnknownItem(i)),
            Action::Place { item, .. } if item.index() >= c.combat.items.len() => out.push(RowFault::UnknownItem(item)),
            Action::Learn(s) if s.index() >= c.combat.spells.len() => out.push(RowFault::UnknownSpell(s)),
            Action::Status(e) if e.index() >= c.combat.effects.len() => out.push(RowFault::UnknownEffect(e)),
            Action::Spawn { def, .. } if def.index() >= c.combat.units.len() => out.push(RowFault::UnknownUnitDef(def)),
            Action::Talk(d) if d.index() >= c.story.dialogue.len() => out.push(RowFault::UnknownDialogue(d)),
            Action::Strike { amount, school, effect, .. } => {
                if school == School::Heal || amount <= Milli::ZERO {
                    out.push(RowFault::BadStrike);
                }
                if let Some(e) = effect.filter(|e| e.index() >= c.combat.effects.len()) {
                    out.push(RowFault::UnknownEffect(e));
                }
            }
            Action::Grow { amount, .. } if amount <= 0 => out.push(RowFault::BadGrow),
            Action::If { when, then, els } => {
                match conds(bp, c, when) {
                    Some([]) => out.push(RowFault::EmptyIf),
                    Some(_) => check_conds(bp, c, when, out),
                    None => out.push(RowFault::UnknownConds(when)),
                }
                check_list(bp, c, then, depth + 1, out);
                if let Some(e) = els {
                    check_list(bp, c, e, depth + 1, out);
                }
            }
            Action::Send { then: Some(t), .. } => check_list(bp, c, t, depth + 1, out),
            Action::Reveal(n) => match names(bp, c, n) {
                Some([]) => out.push(RowFault::EmptyReveal),
                Some(_) => {}
                None => out.push(RowFault::UnknownNames(n)),
            },
            _ => {}
        }
    }
}

fn check_conds(bp: &Blueprint, cat: &'static Catalog, r: CondsRef, out: &mut Vec<RowFault>) {
    let Some(cs) = conds(bp, cat, r) else {
        out.push(RowFault::UnknownConds(r));
        return;
    };
    for c in cs {
        match c.c {
            Condition::QuestActive(q) | Condition::QuestReady(q) | Condition::QuestDone(q)
                if q.index() >= cat.story.quests.len() =>
            {
                out.push(RowFault::UnknownQuest(q));
            }
            Condition::HasItem(s) if s.item.index() >= cat.combat.items.len() => {
                out.push(RowFault::UnknownItem(s.item));
            }
            Condition::HasSpell(s) if s.index() >= cat.combat.spells.len() => out.push(RowFault::UnknownSpell(s)),
            _ => {}
        }
    }
}

/// Every name a list leans on that this blueprint does not have: a prop it locks, unlocks, shows,
/// hides or switches; a mark it spawns at or sends to; a rect it fills, strikes or reveals.
pub fn missing_names(
    bp: &Blueprint,
    cat: &'static Catalog,
    r: ListRef,
    props: &BTreeMap<Key, usize>,
) -> Vec<(NameKind, Key)> {
    let mut out = Vec::new();
    let mut need = |kind: NameKind, k: Key, there: bool| {
        if !there && !out.contains(&(kind, k)) {
            out.push((kind, k));
        }
    };
    each_action(bp, cat, r, &mut |a| match *a {
        Action::Lock(p) | Action::Unlock(p) | Action::Show(p) | Action::Hide(p) | Action::Switch { prop: p, .. } => {
            need(NameKind::Prop, p, props.contains_key(&p));
        }
        Action::Spawn { at, .. } => need(NameKind::Mark, at, bp.marks.contains_key(&at)),
        Action::Send { to, .. } => need(NameKind::Mark, to, bp.marks.contains_key(&to)),
        Action::Fill { rect, .. } | Action::Strike { rect, .. } => {
            need(NameKind::Rect, rect, bp.rects.contains_key(&rect));
        }
        Action::Reveal(n) => {
            for &r in names(bp, cat, n).unwrap_or(&[]) {
                need(NameKind::Rect, r, bp.rects.contains_key(&r));
            }
        }
        _ => {}
    });
    out
}

#[cfg(test)]
mod tests {
    use jane_core::action::{Cond, Condition, FlagKey, FlagOp, FlagTest};
    use jane_core::ids::{EffectId, SpellId, ZoneId};

    use super::*;
    use crate::solve::sketch::Sketch;

    /// `verbs2.test.ts` "is checked at boot all the way down: a bad row inside a branch is still a bad row".
    #[test]
    fn a_bad_row_inside_a_branch_is_still_a_bad_row() {
        let mut k = Sketch::new(ZoneId::Arms, &["...."]);
        let cat = jane_data::catalog();
        let flag = k.flag("test_x");
        let good_then = k.list(vec![Action::Learn(k.spell("icebolt"))]);
        let when = k.conds(vec![Cond { not: false, c: Condition::Flag { key: flag, test: FlagTest::NonZero } }]);
        let good = k.list(vec![Action::If { when, then: good_then, els: None }]);
        assert_eq!(row_faults(&k.bp, cat, good, None), vec![]);

        let bad_spell = SpellId(u16::MAX);
        let bad_when = k.conds(vec![Cond { not: false, c: Condition::HasSpell(bad_spell) }]);
        let bad_then = k.list(vec![Action::Give(jane_core::Stack { item: jane_core::ItemId(u16::MAX), qty: 1 })]);
        let deep = k.list(vec![Action::Status(EffectId(u16::MAX))]);
        let unit = k.name("x");
        let to = k.name("y");
        let els = k.list(vec![Action::Send { unit, to, then: Some(deep) }]);
        let nested = k.list(vec![Action::If { when: bad_when, then: bad_then, els: Some(els) }]);
        let faults = row_faults(&k.bp, cat, nested, None);
        assert!(faults.contains(&RowFault::UnknownSpell(bad_spell)), "{faults:?}");
        assert!(faults.contains(&RowFault::UnknownItem(jane_core::ItemId(u16::MAX))), "{faults:?}");
        assert!(faults.contains(&RowFault::UnknownEffect(EffectId(u16::MAX))), "{faults:?}");

        let empty_when = k.conds(vec![]);
        let empty_then = k.list(vec![]);
        let empty = k.list(vec![Action::If { when: empty_when, then: empty_then, els: None }]);
        assert_eq!(row_faults(&k.bp, cat, empty, None), vec![RowFault::EmptyIf]);
    }

    #[test]
    fn a_list_that_names_itself_ends() {
        let mut k = Sketch::new(ZoneId::Arms, &["...."]);
        let cat = jane_data::catalog();
        let when = k.conds(vec![Cond { not: true, c: Condition::Night }]);
        // List 0 will be this one: an `if` whose `then` is itself.
        let me = ListRef::Blueprint(k.bp.lists.len() as u16);
        let l = k.list(vec![Action::If { when, then: me, els: None }]);
        assert_eq!(l, me);
        assert!(row_faults(&k.bp, cat, l, None).contains(&RowFault::TooDeep));
        let mut n = 0;
        each_action(&k.bp, cat, l, &mut |_| n += 1);
        assert_eq!(n, usize::from(MAX_DEPTH) + 1);
    }

    #[test]
    fn names_a_list_leans_on_must_be_in_the_blueprint() {
        let mut k = Sketch::new(ZoneId::Arms, &["...."]);
        let cat = jane_data::catalog();
        let here = k.mark("here", 0, 0);
        let gone = k.name("no_such_gate");
        let nowhere = k.name("nowhere");
        let flag = FlagKey::Named(k.name("f"));
        let l = k.list(vec![
            Action::Lock(gone),
            Action::Send { unit: gone, to: here, then: None },
            Action::Fill { rect: nowhere, tile: jane_core::Tile::Floor },
            Action::Flag { key: flag, op: FlagOp::Set(1) },
        ]);
        let missing = missing_names(&k.bp, cat, l, &BTreeMap::new());
        assert_eq!(missing, vec![(NameKind::Prop, gone), (NameKind::Rect, nowhere)]);
    }
}
