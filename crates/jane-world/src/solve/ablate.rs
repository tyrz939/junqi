//! Ablation: take one thing away and solve again, to prove that the lock it opens really holds
//! (DUNGEONS.md §2.6, check C1: "solve again with that one grant withheld; the far node must be
//! unreachable"). What can be taken away: a key tag, a spell, a flag, a prop's lists; or a gate
//! kept shut whatever is unlocked.

use jane_core::action::FlagKey;
use jane_core::blueprint::Blueprint;
use jane_core::grid::Rect;
use jane_core::ids::{Key, SpellId};

use super::model::{KeyTag, Options, ZoneRules};
use super::report::Report;
use super::run::solve;

/// Things she is never given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Withhold {
    /// Key tags: no chest, drop or door-side gift ever adds one.
    pub keys: Vec<KeyTag>,
    /// Spells: never learned (and not known at the door).
    pub verbs: Vec<SpellId>,
    /// Flags: never set by any list (a state's flag is a control's; withhold the control).
    pub flags: Vec<FlagKey>,
    /// Props whose lists never run: never looted, read, pulled, mended or taken.
    pub props: Vec<Key>,
}

/// One thing a lock might be opened by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grant {
    Key(KeyTag),
    Verb(SpellId),
    Flag(FlagKey),
    /// A prop's lists: a control, a lever, a chest.
    Prop(Key),
    /// Not a grant but its gate: kept shut whatever is unlocked.
    Shut(Key),
}

impl Options {
    /// These options with one more thing taken away.
    pub fn without(&self, g: Grant) -> Options {
        let mut o = self.clone();
        match g {
            Grant::Key(k) => o.withhold.keys.push(k),
            Grant::Verb(v) => o.withhold.verbs.push(v),
            Grant::Flag(f) => o.withhold.flags.push(f),
            Grant::Prop(p) => o.withhold.props.push(p),
            Grant::Shut(p) => o.shut.push(p),
        }
        o
    }
}

/// Solve again with `grant` taken away, traced.
pub fn without(bp: &Blueprint, rules: &ZoneRules, opts: &Options, grant: Grant) -> Report {
    let mut o = opts.without(grant);
    o.trace = true;
    solve(bp, rules, &o)
}

/// Does the lock hold: with `grant` taken away, is no cell of `behind` ever reached? Ask it of a
/// blueprint that passed: one refused before the flood reaches nothing, so every lock "holds".
pub fn lock_holds(bp: &Blueprint, rules: &ZoneRules, opts: &Options, grant: Grant, behind: Rect) -> bool {
    !without(bp, rules, opts, grant).info.reached_rect(behind)
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;
    use jane_core::action::Stack;

    use super::*;
    use crate::solve::sketch::Sketch;

    #[test]
    fn a_key_in_reach_opens_its_gate_and_without_it_the_room_behind_is_not_reached() {
        let mut k = Sketch::new(ZoneId::Arms, &["########", "#......#", "#......#", "#......#", "########"]);
        k.mark("start", 1, 3);
        let vault = k.rect("vault", 6, 1, 1, 3);
        let vault = k.bp.rects[&vault];
        let g = k.prop("g", "gate_v", 5, 1);
        k.bp.props[g].locked = true;
        let generic = k.name("generic");
        k.bp.props[g].key_tag = Some(generic);
        let gate = k.bp.props[g].key;
        let c = k.prop("chest", "chest", 1, 1);
        k.bp.props[c].loot = vec![Stack { item: k.item("key_generic"), qty: 1 }];
        let chest = k.bp.props[c].key;
        let base = Options { trace: true, ..Options::default() };
        let whole = solve(&k.bp, &k.rules, &base);
        assert!(whole.ok(), "{:?}", whole.lines(&k.bp));
        assert!(whole.info.reached_rect(vault));
        let bp = &k.bp;
        assert!(lock_holds(bp, &k.rules, &base, Grant::Key(KeyTag::Tag(generic)), vault));
        assert!(lock_holds(bp, &k.rules, &base, Grant::Prop(chest), vault));
        assert!(lock_holds(bp, &k.rules, &base, Grant::Shut(gate), vault));
        assert!(!lock_holds(bp, &k.rules, &base, Grant::Verb(k.spell("repair")), vault), "no spell was needed");
    }
}
