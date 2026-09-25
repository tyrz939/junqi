//! The passes: each one looks at what the last flood reached and does what can be done there,
//! and says whether it did anything. `run.rs` sweeps them in order until none does.
//!
//! What they share is here: asking a condition, running a list as the solver runs it (every
//! verb that changes what she holds or where she can walk, nothing else), and killing a unit.

pub mod gates;
pub mod hops;
pub mod ifs;
pub mod kills;
pub mod loot;
pub mod mechanisms;
pub mod repairs;
pub mod states;
pub mod triggers;

use jane_core::action::{Action, Cond, Condition, FlagOp, FlagTest};
use jane_core::ids::{DialogueId, Key, UnitDefId};
use jane_data::{Answers, Faction, SpellKind, WorldSpell};

use super::model::{Deferred, KeyTag, Solve, Waiting};
use super::rows::{self, MAX_DEPTH};

/// Did the pass do anything?
pub type Changed = bool;

/// A pass: look at what is reached, do what can be done, say whether anything was.
pub type Pass = for<'a, 'b> fn(&'b mut Solve<'a>) -> Changed;

pub fn test(t: FlagTest, v: i32) -> bool {
    match t {
        FlagTest::Eq(x) => v == x,
        FlagTest::Min(x) => v >= x,
        FlagTest::NonZero => v != 0,
    }
}

impl Solve<'_> {
    /// Does this condition hold in layer `s`? `None` when it is not about a state at all.
    pub(crate) fn state_holds(&self, c: &Cond, s: usize) -> Option<bool> {
        let Condition::Flag { key, test: t } = c.c else { return None };
        let bit = self.states.bit(key)?;
        let v = ((s >> bit) & 1) as i32;
        Some(test(t, v) != c.not)
    }

    /// Can this condition hold, some time? The clock and the quest log are outside the zone:
    /// assume the story gets there.
    pub(crate) fn holds(&self, c: &Cond) -> bool {
        // "Not yet" can only have been true earlier, so it never stops anything from happening.
        if c.not {
            return true;
        }
        match c.c {
            Condition::Flag { key, test: t } => test(t, self.flags.get(&key).copied().unwrap_or(0)),
            Condition::Dead(k) => self.dead.contains(&k),
            Condition::HasSpell(s) => self.verbs.as_ref().is_none_or(|v| v.contains(&s)),
            Condition::HasItem(st) => self.have(self.item_tag(st.item)) >= i32::from(st.qty.max(1)),
            _ => true,
        }
    }

    pub(crate) fn holds_in(&self, c: &Cond, s: usize) -> bool {
        self.state_holds(c, s).unwrap_or_else(|| self.holds(c))
    }

    /// Run a list as it would run in layer `s`. `again`: whatever owns the list can run it again
    /// later (a lever, a `while` row), so a `then` that cannot hold yet is worth keeping.
    /// `control`: the list is a control's, and what it does to props is what its state IS.
    pub(crate) fn apply_actions(&mut self, list: &[Action], s: usize, again: bool, control: bool) {
        self.apply_at(list, s, again, control, 0);
    }

    fn apply_at(&mut self, list: &[Action], s: usize, again: bool, control: bool, depth: u8) {
        if depth > MAX_DEPTH {
            return;
        }
        for a in list {
            match *a {
                Action::If { when, then, els } => {
                    let conds = self.conds(when);
                    if conds.iter().all(|c| self.holds_in(c, s)) {
                        self.apply_at(self.list(then), s, again, control, depth + 1);
                    } else {
                        if let Some(e) = els {
                            self.apply_at(self.list(e), s, again, control, depth + 1);
                        }
                        // Only what is not about a state can come true later in the same state.
                        if again && conds.iter().all(|c| self.state_holds(c, s).unwrap_or(true)) {
                            self.deferred.push(Deferred { when, then, layer: s, control });
                        }
                    }
                }
                // The solver assumes fights are won; it assumes walks are finished too.
                Action::Send { then, .. } => {
                    if let Some(t) = then {
                        self.apply_at(self.list(t), s, again, control, depth + 1);
                    }
                }
                _ => self.apply_action(a, control),
            }
        }
    }

    pub(crate) fn apply_action(&mut self, a: &Action, control: bool) {
        match *a {
            // What a control does to props is what its state IS (`states::prepare`), not
            // something that happens once. `lock` from anything else never shuts what is open.
            Action::Unlock(k) | Action::Show(k) | Action::Hide(k) if !control => {
                let Some(&i) = self.prop_ix.get(&k) else { return };
                let p = &mut self.props[i];
                match *a {
                    Action::Unlock(_) => p.open = true,
                    Action::Hide(_) => (p.hidden, p.removed) = (true, true),
                    _ => (p.hidden, p.removed) = (false, false),
                }
                if matches!(a, Action::Unlock(_)) {
                    self.mark_fired(i);
                }
                if self.blocks_feet(i) {
                    self.opened = true;
                }
            }
            Action::Give(st) => self.add_key(self.item_tag(st.item), i32::from(st.qty)),
            Action::Flag { key, op } => {
                if self.opts.withhold.flags.contains(&key) || self.states.bit(key).is_some() {
                    return;
                }
                let v = self.flags.entry(key).or_insert(0);
                match op {
                    FlagOp::Set(x) => *v = x,
                    FlagOp::Add(x) => *v += x,
                }
            }
            Action::Learn(sp) => {
                if !self.opts.withhold.verbs.contains(&sp) {
                    if let Some(v) = self.verbs.as_mut() {
                        v.insert(sp);
                    }
                }
            }
            Action::Spawn { key, def, at } => {
                let hostile = self.cat.combat.units.get(def.index()).is_some_and(|d| d.faction != Faction::Friendly);
                if hostile && !self.dead.contains(&key) && !self.waiting.iter().any(|w| w.unit == key) {
                    self.waiting.push(Waiting { unit: key, def, at });
                }
            }
            _ => {}
        }
    }

    /// She won the fight: what it guaranteed to drop is hers, and its death rows run.
    pub(crate) fn killed(&mut self, key: Key, def: UnitDefId) {
        self.dead.insert(key);
        let Some(d) = self.cat.combat.units.get(def.index()) else { return };
        for l in d.loot {
            if l.chance.0 >= 1000 {
                self.add_key(self.item_tag(l.item), i32::from(l.qty));
            }
        }
        if let Some(l) = d.on_death {
            self.apply_actions(self.list(l), 0, false, false);
        }
        for ph in d.phases {
            if let Some(l) = ph.on_enter {
                self.apply_actions(self.list(l), 0, false, false);
            }
        }
    }

    /// Every `learn` in a dialogue tree. The solver does not read; it assumes she does.
    pub(crate) fn read_tree(&mut self, tree: DialogueId) {
        let Some(t) = self.cat.story.dialogue.get(tree.index()) else { return };
        let mut learnt = Vec::new();
        for node in t.nodes {
            let lists = node.actions.into_iter().chain(node.options.iter().filter_map(|o| o.actions));
            for l in lists {
                rows::each_action(self.bp, self.cat, l, &mut |a| {
                    if matches!(a, Action::Learn(_)) {
                        learnt.push(*a);
                    }
                });
            }
        }
        for a in learnt {
            self.apply_action(&a, false);
        }
    }

    /// Does she know a spell that would switch this on?
    pub(crate) fn can_answer(&self, answers: Answers) -> bool {
        let Some(verbs) = &self.verbs else { return true };
        verbs.iter().any(|sp| {
            let Some(d) = self.cat.combat.spells.get(sp.index()) else { return false };
            match d.kind {
                SpellKind::World => matches!(
                    (d.world, answers),
                    (Some(WorldSpell::Repair), Answers::Repair) | (Some(WorldSpell::Grow), Answers::Grow)
                ),
                SpellKind::Bolt => answers.school() == Some(d.school),
                _ => false,
            }
        })
    }

    /// Does she hold what mending it would use up?
    pub(crate) fn has_needs(&self, i: usize) -> bool {
        self.bp.props[i].needs.iter().all(|n| self.have(KeyTag::Item(n.item)) >= i32::from(n.qty))
    }

    /// Can this prop be worked in layer `s`, by someone standing in it?
    pub(crate) fn can_work(&self, i: usize, s: usize) -> bool {
        if self.props[i].withheld || self.hidden_in(i, s) || self.locked_in(i, s) {
            return false;
        }
        self.def(i).answers.is_none_or(|a| self.can_answer(a) && self.has_needs(i))
    }

    /// Work a prop's `use` in every layer it can be worked in: a control in every layer she
    /// reaches it in, anything else once. Mechanisms and repairs both come through here.
    pub(crate) fn work(&mut self, i: usize, at: usize) -> Changed {
        let def = self.def(i);
        let control = self.is_control(i);
        let mut changed = false;
        for s in at..self.layers.count {
            if s != at
                && (!control
                    || !self.layers.reached[s]
                    || self.hidden_in(i, s)
                    || self.locked_in(i, s)
                    || !self.touches_in(i, s))
            {
                continue;
            }
            let bit = 1u8 << if control { s } else { 0 };
            if self.props[i].fired & bit != 0 {
                continue;
            }
            if let Some(a) = def.answers {
                if !self.can_answer(a) || !self.has_needs(i) {
                    continue;
                }
                // It is mended once, in whichever state she first mends it.
                if !self.props[i].paid {
                    self.props[i].paid = true;
                    for n in &self.bp.props[i].needs {
                        *self.keys.entry(KeyTag::Item(n.item)).or_insert(0) -= i32::from(n.qty);
                    }
                }
            }
            self.props[i].fired |= bit;
            self.mark_fired(i);
            self.apply_actions(self.use_of(i), s, !def.once && def.answers.is_none(), control);
            changed = true;
        }
        changed
    }
}
