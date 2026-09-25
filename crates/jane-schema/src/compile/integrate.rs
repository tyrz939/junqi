//! Checks that need every group at once (ARCHITECTURE.md §5.3, PORT.md §5.3): every name a list,
//! condition, trigger or quest uses is provided by some declared source, and every flag read is
//! written somewhere.
//!
//! **Scope today.** Names are checked county-wide, not per zone, and a miss is a *warning*: the
//! county's authored places are still code (`chunks.ts`, `interiors.ts`, `areas.ts`) until they
//! become `.chunk` files (PORT.md §6.f, P2 stage 6), and the names they draw have no declared
//! provider yet. When the chunks land, a miss becomes an error and the check goes per zone.

use std::collections::BTreeSet;

use jane_core::action::{Action, Condition, FlagKey};
use jane_core::ids::{Key, NameId};

use super::diag::Diagnostics;
use crate::model::{Catalog, ReqTarget};

/// What a name is used as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Use {
    Prop,
    Unit,
    Mark,
    Rect,
}

impl Use {
    fn word(self) -> &'static str {
        match self {
            Use::Prop => "prop",
            Use::Unit => "unit",
            Use::Mark => "mark",
            Use::Rect => "rect",
        }
    }
}

/// Every name some source provides, of any kind.
fn provided(c: &Catalog) -> BTreeSet<NameId> {
    let mut out = BTreeSet::new();
    for p in c.county.provides() {
        out.insert(p.name);
    }
    for m in c.dungeons.missions {
        for n in m.provides {
            out.insert(n.name);
        }
    }
    for i in c.combat.items {
        if let Some(t) = i.opens {
            out.insert(t);
        }
    }
    out
}

fn name(k: Key) -> Option<NameId> {
    match k {
        Key::Name(n) => Some(n),
        Key::Local(_) => None,
    }
}

/// Run the cross-group checks over a compiled catalog.
pub fn check(c: &Catalog, diag: &mut Diagnostics) {
    let mut provided = provided(c);
    // A spawn verb provides its unit key.
    for list in c.lists {
        for a in *list {
            if let Action::Spawn { key: Key::Name(n), .. } = *a {
                provided.insert(n);
            }
        }
    }
    let mut used: BTreeSet<(Use, NameId)> = BTreeSet::new();
    let mut flags_read: BTreeSet<FlagKey> = BTreeSet::new();
    let mut flags_written: BTreeSet<FlagKey> = BTreeSet::new();
    let use_key = |u: Use, k: Key, used: &mut BTreeSet<(Use, NameId)>| {
        if let Some(n) = name(k) {
            used.insert((u, n));
        }
    };

    for list in c.lists {
        for a in *list {
            match *a {
                Action::Lock(k) | Action::Unlock(k) | Action::Show(k) | Action::Hide(k) => {
                    use_key(Use::Prop, k, &mut used);
                }
                Action::Switch { prop, .. } => use_key(Use::Prop, prop, &mut used),
                Action::Spawn { key, at, .. } => {
                    // A spawn provides its unit key; its mark must exist.
                    use_key(Use::Mark, at, &mut used);
                    flags_written.insert(FlagKey::Dead(key));
                }
                Action::Despawn(k) | Action::Aggro(k) => use_key(Use::Unit, k, &mut used),
                Action::Send { unit, to, .. } => {
                    use_key(Use::Unit, unit, &mut used);
                    use_key(Use::Mark, to, &mut used);
                }
                Action::Travel { mark, .. } => use_key(Use::Mark, mark, &mut used),
                Action::Fill { rect, .. } | Action::Strike { rect, .. } => use_key(Use::Rect, rect, &mut used),
                Action::Camera { rect: Some(r), .. } => use_key(Use::Rect, r, &mut used),
                Action::Reveal(r) => {
                    for &k in c.names_of(r) {
                        use_key(Use::Rect, k, &mut used);
                    }
                }
                Action::Flag { key, .. } => {
                    flags_written.insert(key);
                }
                Action::Location(k) => {
                    flags_written.insert(FlagKey::Been(k));
                }
                _ => {}
            }
        }
    }
    for conds in c.conds {
        for cond in *conds {
            match cond.c {
                Condition::Flag { key, .. } => {
                    flags_read.insert(key);
                }
                Condition::Dead(k) => use_key(Use::Unit, k, &mut used),
                _ => {}
            }
        }
    }
    for t in c.story.triggers {
        use_key(Use::Rect, t.trigger.rect, &mut used);
    }
    // A quest's location step is counted by the flag its `location` verb writes.
    for q in c.story.quests {
        for r in q.requirements {
            if let ReqTarget::Location(n) = r.target {
                flags_read.insert(FlagKey::Been(Key::Name(n)));
            }
        }
    }
    // Every unit that can die writes its dead: flag; mission state flags are written by their levers.
    for &n in &provided {
        flags_written.insert(FlagKey::Dead(Key::Name(n)));
    }
    for m in c.dungeons.missions {
        for n in m.provides {
            if n.what == crate::model::MissionNameWhat::Flag {
                flags_written.insert(FlagKey::Named(Key::Name(n.name)));
            }
        }
        for s in m.states {
            flags_written.insert(FlagKey::Named(Key::Name(s.flag)));
        }
    }
    for z in c.county.zones {
        for s in z.states {
            flags_written.insert(s.flag);
        }
    }

    let is_socket = |n: NameId| c.name(n).starts_with('@');
    let mut missing: Vec<String> = used
        .iter()
        .filter(|(_, n)| !provided.contains(n) && !is_socket(*n))
        .map(|(u, n)| format!("{} \"{}\"", u.word(), c.name(*n)))
        .collect();
    missing.dedup();
    if !missing.is_empty() {
        diag.warn(
            "providers",
            format!(
                "{} name(s) used with no declared provider (authored places are still code until .chunk files land): {}",
                missing.len(),
                missing.join(", ")
            ),
        );
    }

    let flag_name = |k: FlagKey| match k {
        FlagKey::Named(k) => name(k).map(|n| c.name(n).to_owned()),
        FlagKey::Been(k) => name(k).map(|n| format!("been:{}", c.name(n))),
        FlagKey::Dead(k) => name(k).map(|n| format!("dead:{}", c.name(n))),
    };
    let unset: Vec<String> = flags_read.difference(&flags_written).filter_map(|&k| flag_name(k)).collect();
    if !unset.is_empty() {
        diag.warn("flags", format!("{} flag(s) read but never set: {}", unset.len(), unset.join(", ")));
    }
}
