//! C1: every critical node is reached, in the mission's order, and every lock holds without its
//! grant. (C2, no key behind its own lock, falls out of the forward solve: a key behind its own
//! door leaves the far room unreached, which is the first half of this check.)
//!
//! "In order": the flood that first reaches a critical room is never earlier than the one that
//! reached a node the mission puts before it. Nodes that share a place in the order may be
//! reached either way round.
//!
//! "Holds": solve again with the one thing that opens the lock taken away (the key tag, the
//! spell, the flag; for a state gate, every control of its state) and the far room must stay
//! unreached (DUNGEONS.md §2.6, [`crate::solve::ablate`]).

use jane_core::action::FlagKey;
use jane_core::ids::Key;
use jane_data::MissionEdgeKind;

use super::{Check, Ctx, Fault};
use crate::solve::ablate::Grant;
use crate::solve::model::KeyTag;

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let mut out = Vec::new();
    let m = c.m;
    let mut critical: Vec<_> = c.info.rooms.iter().filter(|r| m.nodes[r.node].critical).collect();
    critical.sort_by_key(|r| m.nodes[r.node].order);
    let (mut last, mut last_order, mut floor) = (0u16, None, 0u16);
    for r in critical {
        let n = &m.nodes[r.node];
        let p = c.pass_of(r.rect);
        if p.is_none() {
            out.push(Fault::new(Check::C1, format!("{} is never reached", n.id)));
        }
        if last_order != Some(n.order) {
            floor = last;
            last_order = Some(n.order);
        }
        if let Some(p) = p {
            if p < floor {
                out.push(Fault::new(
                    Check::C1,
                    format!(
                        "{} (order {}) is reached before something that comes earlier in the mission",
                        n.id, n.order
                    ),
                ));
            }
            last = last.max(p);
        }
    }

    for lock in &c.info.locks {
        let edge = &m.edges[lock.edge];
        if edge.shortcut {
            continue;
        }
        let (Some(a), Some(b)) = (c.room_of(usize::from(edge.from)), c.room_of(usize::from(edge.to))) else {
            continue;
        };
        // The far side is the one the lock keeps her out of: whichever room the flood reached
        // later, unless one end was never reached at all (behind a verb this dungeon does not
        // give her), which is then the far side however its pass compares.
        let (pa, pb) = (c.pass_of(a.rect), c.pass_of(b.rect));
        let far = match (pa, pb) {
            (_, None) => b,
            (None, _) => a,
            (Some(pa), Some(pb)) => {
                if pb >= pa {
                    b
                } else {
                    a
                }
            }
        };
        let mut opts = c.opts.clone();
        let what = match lock.kind {
            MissionEdgeKind::Key { tag, .. } => {
                opts = opts.without(Grant::Key(KeyTag::Tag(Key::Name(tag))));
                format!("the key \"{}\"", c.cat.name(tag))
            }
            MissionEdgeKind::Verb { verb, .. } => {
                opts = opts.without(Grant::Verb(verb));
                format!("the spell {}", c.cat.combat.spells.get(verb.index()).map_or("?", |s| s.id))
            }
            MissionEdgeKind::Oneway { flag, .. } => {
                opts = opts.without(Grant::Flag(FlagKey::Named(Key::Name(flag))));
                format!("the flag \"{}\"", c.cat.name(flag))
            }
            // A state gate is held by its controls: take every one of them away. One that stands
            // open as the building starts is not a lock on the way in, and proves nothing by this.
            MissionEdgeKind::State { var, is, .. } if m.states[usize::from(var)].initial != is => {
                let controls: Vec<Key> = c.info.controls.iter().filter(|k| k.state == var).map(|k| k.prop).collect();
                let names: Vec<&str> = controls.iter().map(|&k| c.name(k)).collect();
                for k in controls {
                    opts = opts.without(Grant::Prop(k));
                }
                format!("the controls of \"{}\" ({})", m.states[usize::from(var)].id, names.join(", "))
            }
            _ => continue,
        };
        opts.trace = true;
        if c.resolve(&opts).info.reached_rect(far.rect) {
            out.push(Fault::new(
                Check::C1,
                format!(
                    "{} can be reached without {what}: the lock on {} -> {} does not hold",
                    m.nodes[far.node].id,
                    m.nodes[usize::from(edge.from)].id,
                    m.nodes[usize::from(edge.to)].id
                ),
            ));
        }
    }
    out
}
