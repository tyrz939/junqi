//! The verbs and conditions (ARCHITECTURE.md §5.1). Exhaustive enums, so a new verb is a
//! compile error in the sim's `run_action`, the solver's `each_action`, the schema and the emitter.
//!
//! An `Action` is small and `Copy`: anything of variable length (a nested list, a condition
//! list, a list of names) is a reference into the catalog or the blueprint, never a copy.

use crate::ids::{
    ConsequenceId, DialogueId, EffectId, ItemId, Key, PropDefId, QuestId, SpellId, StoryId, TextId, UnitDefId, ZoneId,
};
use crate::num::{Milli, Permille};
use crate::tile::Tile;

/// Where a list lives: the compiled catalog, or the blueprint of the zone the list runs in.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ListRef {
    Catalog(u16),
    Blueprint(u16),
}

/// A condition list, stored like an action list.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CondsRef {
    Catalog(u16),
    Blueprint(u16),
}

/// A list of names (`Reveal`), stored like an action list.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NamesRef {
    Catalog(u16),
    Blueprint(u16),
}

/// A string: a content text, or one the generator wrote (`Blueprint::texts`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextRef {
    Text(TextId),
    Local(u16),
}

/// Damage schools. `Blast` is Explosion's and `Shock` is Electric's; each is also what a prop may answer to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Stat {
    Strength,
    Spirit,
}

/// 0 east, 1 south, 2 west, 3 north.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FlagKey {
    Named(Key),
    Been(Key),
    Dead(Key),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FlagOp {
    Set(i32),
    Add(i32),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Heal {
    Flat(Milli),
    Pct(Permille),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CameraMode {
    Follow,
    Lock,
}

/// A door not answered at some hours: what it says instead, and the hours it is shut, `from`
/// up to `to` on the clock, wrapping midnight (content leaves them out for the bell's night, 21
/// to 6). `keyed`: whoever holds a key that fits the door's `keyTag` is answered all the same
/// (Julie's door, once the house is hers).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NightLock {
    pub says: TextRef,
    pub from: u8,
    pub to: u8,
    pub keyed: bool,
}

impl NightLock {
    /// The hours the bell keeps: shut from nine to six.
    pub const BELL: (u8, u8) = (21, 6);

    /// Is the door shut at `hour` (0..=23)? `from == to` is never.
    pub const fn shut_at(&self, hour: u8) -> bool {
        hour_within(hour, self.from, self.to)
    }
}

/// Is `hour` (0..=23) in the hours `from` up to `to`, wrapping midnight? `from == to` is never.
/// A door's hours and `Condition::Hours` are this one rule.
pub const fn hour_within(hour: u8, from: u8, to: u8) -> bool {
    if from < to {
        hour >= from && hour < to
    } else if from > to {
        hour >= from || hour < to
    } else {
        false
    }
}

/// A stack of items.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// A bell rung `strikes` times (STORY.md §8): the School's bell at nine and at six, its
    /// early Tuesday, the Timekeeper's rope, or (`church`) the church at six for evensong.
    /// `from` is where it hangs, a unit, prop or mark of the zone the list runs in; `None`, the
    /// list's subject. Presentation: it says so (an event) and changes nothing.
    Ring {
        strikes: u8,
        from: Option<Key>,
        church: bool,
    },
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
    /// Set a thing down at the subject's feet: a prop of row `prop`, made where it stands, that
    /// holds one `item` and gives it back when picked up (the light stone).
    Place {
        prop: PropDefId,
        item: ItemId,
    },
    /// A door is not answered at these hours from now on, whatever its row said.
    NightLock {
        prop: Key,
        lock: NightLock,
    },
    /// A door is answered at every hour from now on, whatever its row said.
    NightUnlock(Key),
    /// Every prop of row `def` in the zone switched (`on`, or each flipped): a run of lamps
    /// the county builder stands as a row of its own (the east road's, `lamp_east`).
    SwitchAll {
        def: PropDefId,
        on: Option<bool>,
    },
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
                | Action::NightLock { .. }
                | Action::NightUnlock(_)
                | Action::Lock(_)
                | Action::Unlock(_)
                | Action::Switch { .. }
                | Action::SwitchAll { .. }
                | Action::Spawn { .. }
                | Action::Despawn(_)
                | Action::Fill { .. }
                | Action::Send { .. }
                | Action::Flag { .. }
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FlagTest {
    Eq(i32),
    Min(i32),
    NonZero,
}

/// What a thing is, for a fact about it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Thing {
    Item(ItemId),
    Prop(PropDefId),
}

/// A fact the journal can hold (ARCHITECTURE.md §3.7).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    Flag {
        key: FlagKey,
        test: FlagTest,
    },
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
    /// Whoever she is talking to has heard of what the county did (a consequence row's `spreads`)
    /// by now: the town's news, as `SpeakerKnows` is a story's.
    SpeakerHeard(ConsequenceId),
    /// The clock's hour is `from` up to `to`, wrapping midnight ([`hour_within`], a door's hours'
    /// rule): the Museum's bench knows when the doors are shut.
    Hours {
        from: u8,
        to: u8,
    },
    /// The day of the week, 0 Sunday to 6 Saturday: New Game is a Sunday (`GameState::weekday`).
    Weekday(u8),
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
        assert!(Action::Lock(Key::Local(0)).is_world_verb() && Action::Unlock(Key::Local(0)).is_world_verb());
        assert!(!Action::Learn(SpellId(0)).is_world_verb());
    }
}
