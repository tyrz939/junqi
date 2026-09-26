//! What the solver is told about a zone ([`ZoneRules`], [`Options`]) and what it holds while it
//! plays one ([`Solve`]): keys in hand, spells known, flags, the dead, and how every prop stands.
//!
//! Everything here is monotone but the state layers: keys, loot, kills, spells and flags only
//! ever grow, which is why a fixed point exists and the loop in `run.rs` reaches it.

use std::collections::{BTreeMap, BTreeSet};

use jane_core::action::{Action, Cond, CondsRef, FlagKey, ListRef};
use jane_core::blueprint::{Blueprint, Trigger};
use jane_core::ids::{ItemId, Key, NameId, SpellId, UnitDefId, ZoneId};
use jane_data::{Catalog, PropDef};

use super::ablate::Withhold;
use super::flood::Layers;
use super::report::TriggerName;
use super::rows;

/// A zone may have three reversible states at most, so the stateful flood has eight layers at most.
pub const MAX_STATES: usize = 3;
pub const MAX_LAYERS: usize = 1 << MAX_STATES;

/// Floods one solve may run before it stops looking: the TypeScript's bound, never met by a real zone.
pub const MAX_PASSES: u16 = 64;

/// What a key fits. An item with `opens` is its tag; any other item is itself (a `needs`
/// material, a `hasItem` condition), which is the TypeScript's `item:<id>`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum KeyTag {
    Tag(Key),
    Item(ItemId),
}

/// The names every seed of a zone must contain, as a blueprint holds them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contract {
    pub units: Vec<Key>,
    pub props: Vec<Key>,
    pub marks: Vec<Key>,
    pub rects: Vec<Key>,
}

impl Contract {
    fn add(&mut self, units: &[NameId], props: &[NameId], marks: &[NameId], rects: &[NameId]) {
        let push = |into: &mut Vec<Key>, from: &[NameId]| {
            for &n in from {
                if !into.contains(&Key::Name(n)) {
                    into.push(Key::Name(n));
                }
            }
        };
        push(&mut self.units, units);
        push(&mut self.props, props);
        push(&mut self.marks, marks);
        push(&mut self.rects, rects);
    }
}

/// What the solver is told about a zone: the contract, what the story hands over at the door,
/// and its reversible mechanisms. [`ZoneRules::for_zone`] reads them from the catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneRules {
    pub zone: ZoneId,
    pub contract: Contract,
    /// Key tags (an item's `opens`) the story hands over from outside the zone, 99 of each.
    pub given_keys: Vec<Key>,
    /// Spells known at the door. `None`: nothing is gated on knowing a spell (every hand-built zone).
    pub given_verbs: Option<Vec<SpellId>>,
    /// The flags of the zone's reversible mechanisms (a breaker, a valve): bit `n` of a layer is
    /// `states[n]`. Unset or 0 is how the zone starts. At most [`MAX_STATES`].
    pub states: Vec<FlagKey>,
    /// Where she comes in, in order of preference: the first of these the blueprint has as a mark
    /// (PORT.md §6.d: never "whichever mark was written first").
    pub entrances: Vec<Key>,
}

impl ZoneRules {
    /// The rules content gives a zone: `zones.json`'s row, plus for the county what the placement
    /// rows promise, and for a generated dungeon its mission's contract, keys, verbs and states.
    pub fn for_zone(zone: ZoneId) -> Self {
        let cat = jane_data::catalog();
        let def = cat.county.zone(zone);
        let mut contract = Contract::default();
        let c = &def.contract;
        contract.add(c.units, c.props, c.marks, c.rects);
        let mut rules = ZoneRules::open(zone);
        match def.mission.map(|m| cat.dungeons.mission(m)).or_else(|| cat.dungeons.mission_of(zone)) {
            Some(m) => {
                let mc = &m.contract;
                contract.add(mc.units, mc.props, mc.marks, mc.rects);
                rules.given_keys = m.given_keys.iter().map(|&n| Key::Name(n)).collect();
                rules.given_verbs = Some(m.given_verbs.to_vec());
                rules.states = m.states.iter().map(|s| FlagKey::Named(Key::Name(s.flag))).collect();
            }
            None => {
                if zone == ZoneId::County {
                    let p = &cat.county.promised;
                    contract.add(p.units, p.props, p.marks, p.rects);
                }
                rules.given_keys = def.given_keys.iter().map(|&n| Key::Name(n)).collect();
                rules.given_verbs = def.given_verbs.map(<[SpellId]>::to_vec);
                rules.states = def.states.iter().map(|s| s.flag).collect();
            }
        }
        rules.entrances = default_entrances(&contract);
        rules.contract = contract;
        rules
    }

    /// No contract, nothing given, nothing gated, no states: a blueprint judged on itself alone.
    pub fn open(zone: ZoneId) -> Self {
        let contract = Contract::default();
        ZoneRules {
            zone,
            entrances: default_entrances(&contract),
            contract,
            given_keys: Vec::new(),
            given_verbs: None,
            states: Vec::new(),
        }
    }
}

/// `start`, `entry`, `front` (the TypeScript's order), then the contract's marks in order: the
/// cellar comes in by `stair_a`, the first mark its contract names.
fn default_entrances(contract: &Contract) -> Vec<Key> {
    let cat = jane_data::catalog();
    let mut out: Vec<Key> = ["start", "entry", "front"].iter().filter_map(|n| cat.name_id(n)).map(Key::Name).collect();
    for &m in &contract.marks {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    out
}

/// How one solve is asked: the ablation knobs, where to start, and how much to record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Things she is never given, to prove that the lock they open holds without them.
    pub withhold: Withhold,
    /// Gates that stay shut whatever is unlocked: "could she get here with this gate down?"
    pub shut: Vec<Key>,
    /// Flood from this mark instead of the zone's entrance.
    pub entry: Option<Key>,
    /// The blueprint is a PIECE of a zone (one room, stamped alone by the template harness), so a
    /// list naming something in another room is expected, not a broken row; so is a door to a
    /// mark of another room; and the catalog's trigger rows for the zone are not the piece's.
    pub fragment: bool,
    /// Keep which flood first reached each cell ([`super::BuildInfo::first_seen`]): two bytes a
    /// cell, so the county does not ask for it.
    pub trace: bool,
}

/// What a control's list leaves a prop as in the state it leads to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Governed {
    pub locked: Option<bool>,
    pub hidden: Option<bool>,
}

/// How one prop stands, and what has been done with it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PropState {
    /// Opened for good: a key, a lever, a kill.
    pub open: bool,
    /// Hidden now (a control's state may say otherwise in its layer).
    pub hidden: bool,
    /// Hidden for good by something that is not a control (blown up, picked up).
    pub removed: bool,
    /// Kept shut by [`Options::shut`].
    pub shut: bool,
    /// Its lists are withheld ([`Withhold::props`]): it is never worked.
    pub withheld: bool,
    pub looted: bool,
    pub talked: bool,
    /// Its `needs` were spent (a repair is paid for once, whichever state it is mended in).
    pub paid: bool,
    /// Its `use` sets a state flag.
    pub control: bool,
    /// A bit per layer it was worked in; anything but a control only ever sets bit 0.
    pub fired: u8,
    /// The flood after which it was opened, looted, fired or read.
    pub fired_at: Option<u16>,
}

/// A `then` whose `if` could not hold yet, kept to be tried again.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Deferred {
    pub when: CondsRef,
    pub then: ListRef,
    pub layer: usize,
    pub control: bool,
}

/// A unit a trigger (or a breaker) would stand up, waiting for the flood to reach its mark.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Waiting {
    pub unit: Key,
    pub def: UnitDefId,
    pub at: Key,
}

/// A trigger of this zone: the catalog's rows for it, then the blueprint's own.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TriggerRow {
    pub name: TriggerName,
    pub t: Trigger,
    pub fired: bool,
}

/// The zone's reversible states, and what each layer looks like.
#[derive(Clone, Debug, Default)]
pub(crate) struct States {
    /// Bit `n` of a layer is `flags[n]`.
    pub flags: Vec<FlagKey>,
    /// Per layer: what the controls leave each prop as there (by prop index). `None`: no pull
    /// leads there. Layer 0 (how the zone starts) is always `Some`, with nothing overridden.
    pub governed: Vec<Option<Vec<Governed>>>,
    /// Props whose `use` sets a state flag, by index.
    pub controls: Vec<usize>,
}

impl States {
    pub fn bit(&self, key: FlagKey) -> Option<usize> {
        self.flags.iter().position(|&f| f == key)
    }

    pub fn layers(&self) -> usize {
        1 << self.flags.len()
    }
}

/// One solve in progress. Passes (`passes/*.rs`) read and grow it; `run.rs` drives them.
#[derive(Debug)]
pub(crate) struct Solve<'a> {
    pub bp: &'a Blueprint,
    pub cat: &'static Catalog,
    pub rules: &'a ZoneRules,
    pub opts: &'a Options,
    /// Prop index by key; a key written twice keeps its first prop.
    pub prop_ix: BTreeMap<Key, usize>,
    pub props: Vec<PropState>,
    pub triggers: Vec<TriggerRow>,
    pub keys: BTreeMap<KeyTag, i32>,
    /// `None`: nothing is gated on a spell.
    pub verbs: Option<BTreeSet<SpellId>>,
    pub flags: BTreeMap<FlagKey, i32>,
    pub dead: BTreeSet<Key>,
    pub waiting: Vec<Waiting>,
    pub deferred: Vec<Deferred>,
    pub states: States,
    pub layers: Layers,
    /// Marks a same-zone `to` leads to, and the layer she was in when she took it.
    pub hops: Vec<(Key, usize)>,
    pub entry: (i32, i32),
    /// Something that blocks feet changed: flood again.
    pub opened: bool,
    pub pass: u16,
    pub reached_cells: u32,
    /// Kept pass by pass when the solve is asked for one (`run::solve_kept`).
    pub trail: Option<Box<Trail>>,
}

/// What a solve went through, kept so it can be asked again without one grant and go on from the
/// last pass the grant made no difference to (`run::resolve`): the solve as each pass began, and
/// the pass (plus one) each thing was first had in.
#[derive(Debug, Default)]
pub struct Trail {
    pub(crate) starts: Vec<Snapshot>,
    pub(crate) prop_ix: BTreeMap<Key, usize>,
    pub(crate) states: States,
    pub(crate) keys: BTreeMap<KeyTag, u16>,
    pub(crate) verbs: BTreeMap<SpellId, u16>,
    pub(crate) flags: BTreeMap<FlagKey, u16>,
    /// Controls (by prop index) a flood first went on from into another layer.
    pub(crate) edges: BTreeMap<usize, u16>,
}

/// A [`Solve`] as a pass began: everything a pass can change. The blueprint, the rules and the
/// options are the caller's; `prop_ix` and `states` never change after `Solve::new` and are kept
/// once, in the [`Trail`].
#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    props: Vec<PropState>,
    triggers: Vec<TriggerRow>,
    keys: BTreeMap<KeyTag, i32>,
    verbs: Option<BTreeSet<SpellId>>,
    flags: BTreeMap<FlagKey, i32>,
    dead: BTreeSet<Key>,
    waiting: Vec<Waiting>,
    deferred: Vec<Deferred>,
    layers: Layers,
    hops: Vec<(Key, usize)>,
    entry: (i32, i32),
    reached_cells: u32,
}

impl<'a> Solve<'a> {
    pub fn list(&self, r: ListRef) -> &'a [Action] {
        rows::list(self.bp, self.cat, r).unwrap_or(&[])
    }

    pub fn conds(&self, r: CondsRef) -> &'a [Cond] {
        rows::conds(self.bp, self.cat, r).unwrap_or(&[])
    }

    pub fn def(&self, i: usize) -> &'static PropDef {
        self.cat.story.prop(self.bp.props[i].def)
    }

    /// The prop's `use` list, or nothing.
    pub fn use_of(&self, i: usize) -> &'a [Action] {
        self.bp.props[i].use_list.map_or(&[], |r| self.list(r))
    }

    /// The first flood after which the prop was done anything with.
    pub fn mark_fired(&mut self, i: usize) {
        let pass = self.pass;
        self.props[i].fired_at.get_or_insert(pass);
    }

    pub fn is_control(&self, i: usize) -> bool {
        self.props[i].control
    }

    pub fn hidden_in(&self, i: usize, s: usize) -> bool {
        let p = &self.props[i];
        if p.removed {
            return true;
        }
        match self.states.governed.get(s).and_then(Option::as_ref).and_then(|g| g[i].hidden) {
            Some(h) => h,
            None => p.hidden,
        }
    }

    pub fn locked_in(&self, i: usize, s: usize) -> bool {
        if self.props[i].open {
            return false;
        }
        match self.states.governed.get(s).and_then(Option::as_ref).and_then(|g| g[i].locked) {
            Some(l) => l,
            None => self.bp.props[i].locked,
        }
    }

    /// Does this prop ever stand in the way of feet? Only then is its opening worth a new flood.
    pub fn blocks_feet(&self, i: usize) -> bool {
        let def = self.def(i);
        def.gate || (def.solid && !def.push && !def.carry)
    }

    /// The prop's footprint grown by one cell, inclusive: `(x0, y0, x1, y1)`.
    pub fn ring(&self, i: usize) -> (i32, i32, i32, i32) {
        let p = &self.bp.props[i];
        let def = self.def(i);
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        (x - 1, y - 1, x + i32::from(def.w), y + i32::from(def.h))
    }

    /// Can she stand beside it in layer `s`?
    pub fn touches_in(&self, i: usize, s: usize) -> bool {
        self.layers.touches(s, self.ring(i))
    }

    /// The first layer she can stand beside it in, seeing it. Most zones have the one.
    pub fn at(&self, i: usize) -> Option<usize> {
        (0..self.layers.count).find(|&s| self.layers.reached[s] && !self.hidden_in(i, s) && self.touches_in(i, s))
    }

    /// What a prop's lock is to the passes that are not the gate pass: a locked prop that answers
    /// no verb does nothing else until it is opened. A lock is no answer to a spell: `schoolTouch`
    /// in the sim does not look at it, so a locked case really can be blown open.
    pub fn held_shut(&self, i: usize, at: usize) -> bool {
        self.locked_in(i, at) && self.def(i).answers.is_none()
    }

    pub fn item_tag(&self, item: ItemId) -> KeyTag {
        match self.cat.combat.items.get(item.index()).and_then(|d| d.opens) {
            Some(n) => KeyTag::Tag(Key::Name(n)),
            None => KeyTag::Item(item),
        }
    }

    pub fn add_key(&mut self, tag: KeyTag, qty: i32) {
        if !self.opts.withhold.keys.contains(&tag) {
            if let Some(t) = self.trail.as_mut() {
                t.keys.entry(tag).or_insert(self.pass + 1);
            }
            *self.keys.entry(tag).or_insert(0) += qty;
        }
    }

    /// The solve as it stands, for the trail.
    pub(crate) fn snapshot(&self) -> Snapshot {
        Snapshot {
            props: self.props.clone(),
            triggers: self.triggers.clone(),
            keys: self.keys.clone(),
            verbs: self.verbs.clone(),
            flags: self.flags.clone(),
            dead: self.dead.clone(),
            waiting: self.waiting.clone(),
            deferred: self.deferred.clone(),
            layers: self.layers.keep(),
            hops: self.hops.clone(),
            entry: self.entry,
            reached_cells: self.reached_cells,
        }
    }

    /// The solve kept in `trail` as pass `pass` began, asked with `opts`.
    pub(crate) fn restore(
        bp: &'a Blueprint,
        rules: &'a ZoneRules,
        opts: &'a Options,
        trail: &Trail,
        pass: u16,
    ) -> Self {
        let s = trail.starts[usize::from(pass)].clone();
        Solve {
            bp,
            cat: jane_data::catalog(),
            rules,
            opts,
            prop_ix: trail.prop_ix.clone(),
            props: s.props,
            triggers: s.triggers,
            keys: s.keys,
            verbs: s.verbs,
            flags: s.flags,
            dead: s.dead,
            waiting: s.waiting,
            deferred: s.deferred,
            states: trail.states.clone(),
            layers: s.layers,
            hops: s.hops,
            entry: s.entry,
            opened: false,
            pass,
            reached_cells: s.reached_cells,
            trail: None,
        }
    }

    pub fn have(&self, tag: KeyTag) -> i32 {
        self.keys.get(&tag).copied().unwrap_or(0)
    }
}
