//! The verbs and conditions (ARCHITECTURE.md §5.1). Exhaustive enums, so a new verb is a
//! compile error in the sim's `run_action`, the solver's `each_action`, the schema and the emitter.
//!
//! An `Action` is small and `Copy`: anything of variable length (a nested list, a condition
//! list, a list of names) is a reference into the catalog or the blueprint, never a copy.

use crate::ids::{DialogueId, EffectId, ItemId, Key, PropDefId, QuestId, SpellId, StoryId, TextId, UnitDefId, ZoneId};
use crate::num::{Milli, Permille};
use crate::tile::Tile;

/// Where a list lives: the compiled catalog, or the blueprint of the zone the list runs in.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ListRef {
    Catalog(u16),
    Blueprint(u16),
}

/// A condition list, stored like an action list.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CondsRef {
    Catalog(u16),
    Blueprint(u16),
}

/// A list of names (`Reveal`), stored like an action list.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NamesRef {
    Catalog(u16),
    Blueprint(u16),
}

/// A string: a content text, or one the generator wrote (`Blueprint::texts`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TextRef {
    Text(TextId),
    Local(u16),
}

/// Damage schools. `Blast` is Explosion's and `Shock` is Electric's; each is also what a prop may answer to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum School {
    Heal,
    Physical,
    Frost,
    Fire,
    Nature,
    Blast,
    Shock,
}

impl School {
    pub const ALL: [School; 7] =
        [School::Heal, School::Physical, School::Frost, School::Fire, School::Nature, School::Blast, School::Shock];
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Stat {
    Strength,
    Spirit,
}

/// 0 east, 1 south, 2 west, 3 north.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[repr(u8)]
pub enum Facing {
    East,
    #[default]
    South,
    West,
    North,
}

impl Facing {
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Facing::East => (1, 0),
            Facing::South => (0, 1),
            Facing::West => (-1, 0),
            Facing::North => (0, -1),
        }
    }
}

/// A flag's key. The TypeScript's `been:` and `dead:` string prefixes are variants.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FlagKey {
    Named(Key),
    Been(Key),
    Dead(Key),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FlagOp {
    Set(i32),
    Add(i32),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Heal {
    Flat(Milli),
    Pct(Permille),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CameraMode {
    Follow,
    Lock,
}

/// A stack of items.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Stack {
    pub item: ItemId,
    pub qty: u16,
}

/// A verb.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Quest(QuestId),
    HandIn(QuestId),
    Flag {
        key: FlagKey,
        op: FlagOp,
    },
    /// Sleep; `until` is an hour of the day.
    Rest {
        until: Option<u8>,
    },
    /// Growth by finding: `id` names the jar or the page, so taking it twice does nothing.
    Grow {
        stat: Stat,
        amount: i16,
        id: Key,
    },
    Give(Stack),
    Take(Stack),
    Learn(SpellId),
    Toast(TextRef),
    /// Words on a thing, shown in the reading box.
    Read(TextRef),
    Lock(Key),
    Unlock(Key),
    Show(Key),
    Hide(Key),
    Switch {
        prop: Key,
        on: Option<bool>,
    },
    Spawn {
        key: Key,
        def: UnitDefId,
        at: Key,
    },
    Despawn(Key),
    Aggro(Key),
    Location(Key),
    Fill {
        rect: Key,
        tile: Tile,
    },
    /// Everything hostile in a rect is hit once.
    Strike {
        rect: Key,
        amount: Milli,
        school: School,
        effect: Option<EffectId>,
        hits_friends: bool,
    },
    Status(EffectId),
    Heal(Heal),
    Travel {
        zone: ZoneId,
        mark: Key,
    },
    Talk(DialogueId),
    Throw(ItemId),
    Shake(u8),
    Camera {
        mode: CameraMode,
        rect: Option<Key>,
    },
    If {
        when: CondsRef,
        then: ListRef,
        els: Option<ListRef>,
    },
    /// Walk a unit to a mark, then run `then` with that unit as the subject.
    Send {
        unit: Key,
        to: Key,
        then: Option<ListRef>,
    },
    /// Set the fog's seen-bits over these rects of the actor's zone.
    Reveal(NamesRef),
}

impl Action {
    /// Verbs that do nothing without a player to do them to (ARCHITECTURE.md §5.2).
    pub const fn needs_actor(&self) -> bool {
        matches!(
            self,
            Action::Give(_)
                | Action::Take(_)
                | Action::Rest { .. }
                | Action::Travel { .. }
                | Action::Talk(_)
                | Action::Read(_)
        )
    }

    /// World verbs: the only ones a consequence row may use (ARCHITECTURE.md §4.6.d).
    pub const fn is_world_verb(&self) -> bool {
        matches!(
            self,
            Action::Show(_)
                | Action::Hide(_)
                | Action::Switch { .. }
                | Action::Spawn { .. }
                | Action::Despawn(_)
                | Action::Fill { .. }
                | Action::Send { .. }
                | Action::Flag { .. }
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FlagTest {
    Eq(i32),
    Min(i32),
    NonZero,
}

/// What a thing is, for a fact about it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Thing {
    Item(ItemId),
    Prop(PropDefId),
}

/// A fact the journal can hold (ARCHITECTURE.md §3.7).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FactKey {
    Place(Key),
    Person(Key),
    Thing(Thing),
    Claim(TextId),
    Route(Key, Key),
    Danger(Key),
    Rumour(StoryId),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Condition {
    Flag { key: FlagKey, test: FlagTest },
    Night,
    QuestActive(QuestId),
    QuestReady(QuestId),
    QuestDone(QuestId),
    HasItem(Stack),
    HasSpell(SpellId),
    Dead(Key),
    Knows(FactKey),
    Heard(TextId),
    SpeakerKnows(StoryId),
}

/// A condition, or its negation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Cond {
    pub not: bool,
    pub c: Condition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_stay_small() {
        // A verb is copied into no instance; still, a list of them should fit a cache line or two.
        assert!(std::mem::size_of::<Action>() <= 24, "{}", std::mem::size_of::<Action>());
        assert!(std::mem::size_of::<Cond>() <= 24, "{}", std::mem::size_of::<Cond>());
    }

    #[test]
    fn verb_classes() {
        assert!(Action::Travel { zone: ZoneId::Mine, mark: Key::Local(0) }.needs_actor());
        assert!(!Action::Show(Key::Local(0)).needs_actor());
        assert!(Action::Show(Key::Local(0)).is_world_verb());
        assert!(!Action::Learn(SpellId(0)).is_world_verb());
    }
}
