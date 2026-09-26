//! Proof (DUNGEONS.md §2.6, `checks.ts`, PORT.md §6.m stage 15). The lock-and-key solver says a
//! blueprint CAN be finished; these checks say it is the dungeon that was designed: in the
//! authored order, with locks that really hold, a rest room where it should be, a shortcut, a
//! tease, and a way in to every room that seals. A candidate that fails any of them is re-rolled,
//! and the check that failed is named ([`Fault::check`]) so a bad pool or a bad mission shows.
//!
//! | | |
//! | --- | --- |
//! | C1 | every critical node reached, in order, and every lock holds without its grant (`c01_locks`) |
//! | C2 | no key behind its own lock: falls out of C1's forward solve |
//! | C3 | no order of spending plain keys strands her, on the room graph (`c03_plain_keys`) |
//! | C4 | materials cannot be starved (`c04_sinks`) |
//! | C5 | the verb is taught before it is demanded, and first used in safety (`c05_teacher`) |
//! | C6 | every lock-in can be followed into (`c06_lockin`) |
//! | C7 | a rest room off the hub, with no enemies, at the right depth (`c07_rest`) |
//! | C8 | the first completion is the right length (`c08_crit_len`) |
//! | C9 | a loop, and a shortcut that comes out near the rest room (`c09_cycle`) |
//! | C10 | the verb's first lock and the boss gate are seen before they can be opened (`c10_seen`) |
//! | C11 | plates can be held (`c11_plates`) |
//! | C12 | nothing solid appears on a person (`c12_trigger_solid`) |
//!
//! The files follow the TypeScript's numbering (C7 is the rest room, C8 the walk). They run in
//! [`ORDER`], which is the TypeScript's: C8 walks before C7 reads where along the walk the rest
//! room fell.
//!
//! A building with states (a breaker, a valve) is solved with the stateful flood: every solve
//! here passes the mission's states, and a `state` edge's lock is proven like any other, by
//! taking its controls away and finding the far room unreached.

use std::collections::BTreeMap;
use std::fmt;

use jane_core::Blueprint;
use jane_core::action::{Action, FlagKey, ListRef, School};
use jane_core::grid::Rect;
use jane_core::ids::{ItemId, Key, NameId, SpellId};
use jane_data::{Answers, Catalog, MissionDef, MissionNode, SpellKind, WorldSpell, catalog};

use super::generate::{BuildInfo, Built, RoomInfo};
use crate::solve::model::{Options, ZoneRules};
use crate::solve::report::{Report, name_of};
use crate::solve::rows::each_action;
use crate::solve::run::solve;

pub mod c01_locks;
pub mod c03_plain_keys;
pub mod c04_sinks;
pub mod c05_teacher;
pub mod c06_lockin;
pub mod c07_rest;
pub mod c08_crit_len;
pub mod c09_cycle;
pub mod c10_seen;
pub mod c11_plates;
pub mod c12_trigger_solid;

pub use c08_crit_len::{Walk, distances, first_completion};

/// Which proof a fault came from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Check {
    /// The candidate never became a dungeon: no layout, a bad row (`BuildInfo::errors`).
    Build,
    /// The lock-and-key solver refused it (ARCHITECTURE.md §5.3 point 2).
    Solver,
    C1,
    C3,
    C4,
    C5,
    C6,
    C7,
    C8,
    C9,
    C10,
    C11,
    C12,
}

/// The order the checks run in: the TypeScript's `checkDungeon`.
pub const ORDER: [Check; 11] = [
    Check::C1,
    Check::C3,
    Check::C4,
    Check::C5,
    Check::C6,
    Check::C8,
    Check::C7,
    Check::C9,
    Check::C10,
    Check::C11,
    Check::C12,
];

impl Check {
    pub const fn name(self) -> &'static str {
        match self {
            Check::Build => "build",
            Check::Solver => "solver",
            Check::C1 => "C1",
            Check::C3 => "C3",
            Check::C4 => "C4",
            Check::C5 => "C5",
            Check::C6 => "C6",
            Check::C7 => "C7",
            Check::C8 => "C8",
            Check::C9 => "C9",
            Check::C10 => "C10",
            Check::C11 => "C11",
            Check::C12 => "C12",
        }
    }
}

/// Why a candidate is not the dungeon that was designed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fault {
    pub check: Check,
    /// In English, with the blueprint's names.
    pub text: String,
}

impl Fault {
    pub fn new(check: Check, text: impl Into<String>) -> Self {
        Self { check, text: text.into() }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.check {
            Check::Build => f.write_str(&self.text),
            c => write!(f, "{}: {}", c.name(), self.text),
        }
    }
}

/// The solver's rules for a mission: the zone's, with the mission's keys, spells and states
/// (read from `info.mission`, so a test can judge a blueprint against a mission that asks for
/// something else).
pub fn rules_of(m: &MissionDef) -> ZoneRules {
    let mut r = ZoneRules::for_zone(m.zone);
    r.given_keys = m.given_keys.iter().map(|&n| Key::Name(n)).collect();
    r.given_verbs = Some(m.given_verbs.to_vec());
    r.states = m.states.iter().map(|s| FlagKey::Named(Key::Name(s.flag))).collect();
    r
}

/// What every check reads: the blueprint, how it was made, and the traced solve of it.
#[derive(Debug)]
pub struct Ctx<'a> {
    pub bp: &'a Blueprint,
    pub info: &'a BuildInfo,
    pub m: &'static MissionDef,
    pub cat: &'static Catalog,
    pub rules: ZoneRules,
    /// Traced, with the mission's states: what every re-solve starts from.
    pub opts: Options,
    /// The traced solve of the whole blueprint.
    pub base: Report,
    /// The first completion (C8), which C7 reads too.
    pub walk: Walk,
}

impl Ctx<'_> {
    pub fn node(&self, room: &RoomInfo) -> &'static MissionNode {
        &self.m.nodes[room.node]
    }

    pub fn room_of(&self, node: usize) -> Option<&RoomInfo> {
        self.info.room_of(node)
    }

    pub fn name(&self, k: Key) -> &str {
        name_of(self.bp, k)
    }

    /// The first flood that reached any cell of `r`, in the base solve (`passOf`).
    pub fn pass_of(&self, r: Rect) -> Option<u16> {
        self.base.info.first_in(r)
    }

    pub fn prop(&self, k: Key) -> Option<&jane_core::blueprint::PropSpawn> {
        self.bp.props.iter().find(|p| p.key == k)
    }

    /// Solve again, traced, with these options.
    pub fn resolve(&self, opts: &Options) -> Report {
        solve(self.bp, &self.rules, opts)
    }
}

/// Checks C1 to C12 on a candidate, after the solver. Empty: the blueprint is the dungeon that
/// was designed. A refusal by the solver stops here, as the TypeScript's did.
pub fn check_dungeon(built: &Built) -> Vec<Fault> {
    check(&built.blueprint, &built.info)
}

/// [`check_dungeon`] on a blueprint and the record of how it was made, separately: a test breaks
/// one and keeps the other.
pub fn check(bp: &Blueprint, info: &BuildInfo) -> Vec<Fault> {
    if !info.errors.is_empty() || info.layout.is_none() {
        let mut out: Vec<Fault> = info.errors.iter().map(|e| Fault::new(Check::Build, e.clone())).collect();
        if out.is_empty() {
            out.push(Fault::new(Check::Build, "no layout"));
        }
        return out;
    }
    let m = info.mission;
    let rules = rules_of(m);
    let opts = Options { trace: true, ..Options::default() };
    let base = solve(bp, &rules, &opts);
    if !base.ok() {
        return base.lines(bp).into_iter().map(|e| Fault::new(Check::Solver, e)).collect();
    }
    let walk = first_completion(bp, info);
    let c = Ctx { bp, info, m, cat: catalog(), rules, opts, base, walk };
    let mut out = Vec::new();
    for check in ORDER {
        out.extend(run(check, &c));
    }
    out
}

/// One check alone, on a context whose base solve held.
pub fn run(check: Check, c: &Ctx<'_>) -> Vec<Fault> {
    match check {
        Check::C1 => c01_locks::check(c),
        Check::C3 => c03_plain_keys::check(c),
        Check::C4 => c04_sinks::check(c),
        Check::C5 => c05_teacher::check(c),
        Check::C6 => c06_lockin::check(c),
        Check::C7 => c07_rest::check(c),
        Check::C8 => c08_crit_len::check(c),
        Check::C9 => c09_cycle::check(c),
        Check::C10 => c10_seen::check(c),
        Check::C11 => c11_plates::check(c),
        Check::C12 => c12_trigger_solid::check(c),
        Check::Build | Check::Solver => Vec::new(),
    }
}

// --- what a node gives, read off its holdings ----------------------------------------------

/// What finishing a node hands her (`gainsOf`): key tags, other items, spells, flags, and the
/// states she gets a hand on (indices into `MissionDef::states`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gains {
    pub keys: BTreeMap<NameId, i32>,
    pub items: BTreeMap<ItemId, i32>,
    pub verbs: Vec<SpellId>,
    pub flags: Vec<FlagKey>,
    pub states: Vec<u8>,
}

impl Gains {
    fn item(&mut self, cat: &Catalog, item: ItemId, qty: u16) {
        let qty = i32::from(qty);
        match cat.combat.items.get(item.index()).and_then(|d| d.opens) {
            Some(tag) => *self.keys.entry(tag).or_insert(0) += qty,
            None => *self.items.entry(item).or_insert(0) += qty,
        }
    }

    fn actions(&mut self, bp: &Blueprint, cat: &'static Catalog, list: Option<ListRef>) {
        let Some(r) = list else { return };
        each_action(bp, cat, r, &mut |a| match *a {
            Action::Give(s) => self.item(cat, s.item, s.qty),
            Action::Learn(sp) => self.verbs.push(sp),
            Action::Flag { key, .. } => self.flags.push(key),
            _ => {}
        });
    }
}

/// What a node gives, read off its holdings: guaranteed drops and death lists of its units,
/// loot, `use` lists, the lists of a control's states, and every line of a dialogue it holds.
pub fn gains_of(bp: &Blueprint, node: &MissionNode) -> Gains {
    let cat = catalog();
    let mut g = Gains::default();
    for h in node.holds {
        if let Some(u) = h.unit {
            let Some(def) = cat.combat.units.get(u.index()) else { continue };
            for l in def.loot.iter().filter(|l| l.chance.0 >= 1000) {
                g.item(cat, l.item, l.qty);
            }
            g.actions(bp, cat, def.on_death);
            continue;
        }
        for s in h.loot.unwrap_or(&[]) {
            g.item(cat, s.item, s.qty);
        }
        g.actions(bp, cat, h.use_list);
        if let Some(state) = h.controls {
            g.states.push(state);
            for &l in &h.becomes {
                g.actions(bp, cat, l);
            }
        }
        if let Some(tree) = h.talk.and_then(|t| cat.story.dialogue.get(t.index())) {
            for n in tree.nodes {
                g.actions(bp, cat, n.actions);
                for o in n.options {
                    g.actions(bp, cat, o.actions);
                }
            }
        }
    }
    g
}

/// What a spell switches on: a world verb its own, anything else its school.
pub fn answers_of(spell: SpellId) -> Option<Answers> {
    let d = catalog().combat.spells.get(spell.index())?;
    if d.kind == SpellKind::World {
        return match d.world? {
            WorldSpell::Repair => Some(Answers::Repair),
            WorldSpell::Grow => Some(Answers::Grow),
        };
    }
    match d.school {
        School::Heal => None,
        School::Physical => Some(Answers::Physical),
        School::Frost => Some(Answers::Frost),
        School::Fire => Some(Answers::Fire),
        School::Nature => Some(Answers::Nature),
        School::Blast => Some(Answers::Blast),
        School::Shock => Some(Answers::Shock),
    }
}
