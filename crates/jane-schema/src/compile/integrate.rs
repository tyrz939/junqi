//! Checks that need every group at once (ARCHITECTURE.md §5.3, PORT.md §5.3): every name a list,
//! condition, trigger or quest uses is provided by some declared source, and every flag read is
//! written somewhere.
//!
//! **Scope today.** Names are checked county-wide, not per zone, and a miss is an *error* (Grok #8:
//! a typo used to pass `jane check`). The county's set places are `.chunk` files (PORT.md §6.f) and
//! provide what they draw, and so do the placements; a dressed area's code draws its marks, and
//! what a quest or trigger names there is placed by a row (`quarry_top`'s rect).

use std::collections::BTreeSet;

use jane_core::action::{Action, Condition, FlagKey};
use jane_core::ids::{Key, NameId};

use super::diag::Diagnostics;
use crate::model::{Catalog, ReqTarget, ScheduleSlot, ScheduleWhen};

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
    // The authored places' keyed props and units, marks and rects (PORT.md §6.f).
    for (_, n) in c.chunks.exports() {
        out.insert(n);
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
    // A schedule's marks and the props a unit is inside (ARCHITECTURE.md §4.6.a).
    for u in c.combat.units {
        for r in u.schedule {
            match r.slot {
                ScheduleSlot::Mark(n) => {
                    used.insert((Use::Mark, n));
                }
                ScheduleSlot::Inside(n) => {
                    used.insert((Use::Prop, n));
                }
                ScheduleSlot::Patrol | ScheduleSlot::Absent => {}
            }
            if let Some(ScheduleWhen::Flag(n)) = r.when {
                flags_read.insert(FlagKey::Named(Key::Name(n)));
            }
        }
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
    // An omen sets its flag at New Game, on the seeds it is true; one nobody reads is a claim the
    // county never makes good.
    for o in c.story.omens {
        let key = FlagKey::Named(Key::Name(o.flag));
        flags_written.insert(key);
        diag.need(
            flags_read.contains(&key),
            format!("omens.{}", o.id),
            "nothing reads the flag: the claim would never come true",
        );
    }

    living(c, &provided, &flags_written, diag);

    let is_socket = |n: NameId| c.name(n).starts_with('@');
    let mut missing: Vec<String> = used
        .iter()
        .filter(|(_, n)| !provided.contains(n) && !is_socket(*n))
        .map(|(u, n)| format!("{} \"{}\"", u.word(), c.name(*n)))
        .collect();
    missing.dedup();
    if !missing.is_empty() {
        diag.error(
            "providers",
            format!(
                "{} name(s) used with no declared provider (a chunk, a placement or a table must draw it): {}",
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
    // PORT.md §12: a warning until P1's triage, an error since (every read has a write).
    if !unset.is_empty() {
        diag.error("flags", format!("{} flag(s) read but never set: {}", unset.len(), unset.join(", ")));
    }
}

/// The living world's rows against everything else (ARCHITECTURE.md §6): a consequence's flag is
/// set by some list, its named death and a story's listeners are provided names. Errors: these
/// rows are new, and nothing in them is waiting on code to become data.
fn living(c: &Catalog, provided: &BTreeSet<NameId>, flags_written: &BTreeSet<FlagKey>, diag: &mut Diagnostics) {
    for r in c.living.consequences {
        let at = format!("consequences.{}.on", r.id);
        match r.on {
            Condition::Flag { key, .. } => {
                diag.need(flags_written.contains(&key), &at, "the flag is never set: the consequence could never fire");
            }
            Condition::Dead(k) => {
                let named = name(k).is_some_and(|n| provided.contains(&n));
                diag.need(named, &at, "the dead name is nothing any zone provides");
            }
            _ => {}
        }
        for sp in r.spreads {
            for &n in sp.to {
                let at = format!("consequences.{}.spreads", r.id);
                diag.need(provided.contains(&n), &at, format!("\"{}\" is nobody any zone provides", c.name(n)));
            }
        }
    }
    // A line that asks whether someone has heard the news asks of a row that spreads it.
    for conds in c.conds {
        for cond in *conds {
            if let Condition::SpeakerHeard(id) = cond.c {
                let r = &c.living.consequences[id.index()];
                diag.need(
                    !r.spreads.is_empty(),
                    format!("consequences.{}", r.id),
                    "a line asks whether someone has heard of it, and it spreads to nobody",
                );
            }
        }
    }
    for s in c.county.stories {
        let Some(sp) = s.spreads else { continue };
        let at = format!("stories.{}.spreads", s.key);
        diag.need(!s.quests.is_empty(), &at, "a story spreads from its first quest handed in; this one has none");
        for &n in sp.to {
            diag.need(provided.contains(&n), &at, format!("\"{}\" is nobody any zone provides", c.name(n)));
        }
    }
}
