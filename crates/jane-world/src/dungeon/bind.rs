//! `@socket` placeholders bound per node (generate.ts `resolve`). A mission's lists are compiled
//! once, with `@chest:reward` interned as a name of its own; a node's `at_names` says what each
//! placeholder means in it, and `@self` is the holding's key (or a verb edge's prop). A list is
//! copied into the blueprint with its names swapped, nested lists and conditions included.

use jane_core::action::{Action, Cond, Condition, FactKey, FlagKey};
use jane_core::{Blueprint, CondsRef, Key, ListRef, NameId, NamesRef};
use jane_data::{MissionNode, catalog};

/// What the placeholders of one list mean.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub at_names: &'static [(NameId, NameId)],
    /// The name `@self` stands for, if anything.
    pub at_self: Option<(NameId, Key)>,
}

impl Binding {
    pub fn new(node: &'static MissionNode, at_self: Option<NameId>, self_key: Option<Key>) -> Self {
        Self { at_names: node.at_names, at_self: at_self.zip(self_key) }
    }

    pub fn key(&self, k: Key) -> Key {
        let Key::Name(n) = k else { return k };
        if let Some((s, to)) = self.at_self {
            if s == n {
                return to;
            }
        }
        self.at_names.iter().find(|(p, _)| *p == n).map_or(k, |&(_, to)| Key::Name(to))
    }

    fn flag(&self, f: FlagKey) -> FlagKey {
        match f {
            FlagKey::Named(k) => FlagKey::Named(self.key(k)),
            FlagKey::Been(k) => FlagKey::Been(self.key(k)),
            FlagKey::Dead(k) => FlagKey::Dead(self.key(k)),
        }
    }

    fn fact(&self, f: FactKey) -> FactKey {
        match f {
            FactKey::Place(k) => FactKey::Place(self.key(k)),
            FactKey::Person(k) => FactKey::Person(self.key(k)),
            FactKey::Route(a, b) => FactKey::Route(self.key(a), self.key(b)),
            FactKey::Danger(k) => FactKey::Danger(self.key(k)),
            other => other,
        }
    }

    pub fn cond(&self, c: Cond) -> Cond {
        let inner = match c.c {
            Condition::Flag { key, test } => Condition::Flag { key: self.flag(key), test },
            Condition::Dead(k) => Condition::Dead(self.key(k)),
            Condition::Knows(f) => Condition::Knows(self.fact(f)),
            Condition::Within { unit, rect } => Condition::Within { unit: self.key(unit), rect: self.key(rect) },
            other => other,
        };
        Cond { not: c.not, c: inner }
    }

    /// The actions of a list, bound, for splicing into another list.
    pub fn actions(&self, bp: &mut Blueprint, r: ListRef) -> Vec<Action> {
        let src: Vec<Action> = match r {
            ListRef::Catalog(_) => catalog().list(r).to_vec(),
            ListRef::Blueprint(_) => bp.list(r).map(<[Action]>::to_vec).unwrap_or_default(),
        };
        src.into_iter().map(|a| self.action(bp, a)).collect()
    }

    /// A list copied into the blueprint, bound.
    pub fn list(&self, bp: &mut Blueprint, r: ListRef) -> ListRef {
        let v = self.actions(bp, r);
        bp.push_list(v)
    }

    pub fn conds(&self, bp: &mut Blueprint, r: CondsRef) -> CondsRef {
        let src: Vec<Cond> = match r {
            CondsRef::Catalog(_) => catalog().conds_of(r).to_vec(),
            CondsRef::Blueprint(_) => bp.conds_of(r).map(<[Cond]>::to_vec).unwrap_or_default(),
        };
        let v = src.into_iter().map(|c| self.cond(c)).collect();
        bp.push_conds(v)
    }

    fn names(&self, bp: &mut Blueprint, r: NamesRef) -> NamesRef {
        let src: Vec<Key> = match r {
            NamesRef::Catalog(_) => catalog().names_of(r).to_vec(),
            NamesRef::Blueprint(_) => bp.names_of(r).map(<[Key]>::to_vec).unwrap_or_default(),
        };
        let v = src.into_iter().map(|k| self.key(k)).collect();
        bp.push_names(v)
    }

    pub fn action(&self, bp: &mut Blueprint, a: Action) -> Action {
        let k = |x: Key| self.key(x);
        match a {
            Action::Flag { key, op } => Action::Flag { key: self.flag(key), op },
            Action::Grow { stat, amount, id } => Action::Grow { stat, amount, id: k(id) },
            Action::Lock(x) => Action::Lock(k(x)),
            Action::Unlock(x) => Action::Unlock(k(x)),
            Action::Show(x) => Action::Show(k(x)),
            Action::Hide(x) => Action::Hide(k(x)),
            Action::Switch { prop, on } => Action::Switch { prop: k(prop), on },
            Action::Spawn { key, def, at } => Action::Spawn { key: k(key), def, at: k(at) },
            Action::Despawn(x) => Action::Despawn(k(x)),
            Action::Aggro(x) => Action::Aggro(k(x)),
            Action::Location(x) => Action::Location(k(x)),
            Action::Fill { rect, tile } => Action::Fill { rect: k(rect), tile },
            Action::Strike { rect, amount, school, effect, hits_friends } => {
                Action::Strike { rect: k(rect), amount, school, effect, hits_friends }
            }
            Action::Travel { zone, mark } => Action::Travel { zone, mark: k(mark) },
            Action::Camera { mode, rect } => Action::Camera { mode, rect: rect.map(k) },
            Action::If { when, then, els } => {
                let when = self.conds(bp, when);
                let then = self.list(bp, then);
                let els = els.map(|e| self.list(bp, e));
                Action::If { when, then, els }
            }
            Action::Send { unit, to, then } => {
                let then = then.map(|t| self.list(bp, t));
                Action::Send { unit: k(unit), to: k(to), then }
            }
            Action::Reveal(names) => Action::Reveal(self.names(bp, names)),
            other => other,
        }
    }
}
