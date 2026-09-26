//! Typed events for presentation (ARCHITECTURE.md §11). No English: ids only; `jane-present`
//! owns the strings. Routing is the TS rule: personal kinds carry `to`, zone-local kinds carry
//! `in_zone`, party-wide kinds carry neither; [`events_for`] filters.
//!
//! Only the kinds the landed units emit are here; each later unit adds its own (the enum is the
//! list §11 gives, grown as the systems land).

use jane_core::action::{CameraMode, Facing, School};
use jane_core::{EffectId, ItemId, Milli, QuestId, Rect, SpellId, Sym, TextRef, UnitDefId, Vec2, ZoneId};

use crate::ids::{PropId, Seat, UnitId};
use crate::state::{JournalKind, PlayerState, WeatherKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub to: Option<Seat>,
    pub in_zone: Option<ZoneId>,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Text(TextRef),
    /// Travel refused while carrying something ("I should put this down first").
    PutDownFirst,
    /// A seat left and handed on what the story needs.
    LeftWhatMattered,
    // --- the world verbs (interact, inventory, quests) ---------------------------------
    /// "Quest: {name}". Party-wide.
    QuestGiven(QuestId),
    /// "Done: {name}". Party-wide.
    QuestDone(QuestId),
    /// "{req text}: n/of". Party-wide.
    KillProgress {
        quest: QuestId,
        req: u8,
        n: u16,
        of: u16,
    },
    InventoryFull,
    TooTired,
    /// A world spell's prop wants these first.
    Needs {
        item: ItemId,
        qty: u16,
    },
    NothingToRepair,
    NothingGrowsWithoutLight,
    NothingGrows,
    /// Tried while locked: presentation reads the prop's label and its row's `locked_says`.
    Locked {
        prop: PropId,
    },
    UnlockedWith(ItemId),
    /// A door not answered after dark, and what it says instead.
    NightLock(TextRef),
    /// "It shifts a little. Hold to push it."
    ItShifts,
    /// "No room to put it down".
    NoRoom,
    /// "I should keep that" (a bound item will not be destroyed).
    ShouldKeep,
    /// "I'm not hurt" (a heal-only item at full health).
    NotHurt,
    /// "It fits a lock somewhere" (a key used from the bag).
    FitsALock,
    /// "The night will not pass until everyone is resting".
    NightWaits,
    /// Growth by finding: strength ("a little stronger") or spirit ("the words stay").
    Stronger,
    WordsStay,
    /// Something shown from under a pushed prop: `top` was lying on `found`.
    Under {
        top: PropId,
        found: PropId,
    },
    /// A cast failed for a reason worth saying ("Too far", "Not enough mana"). GCD and cooldown
    /// stay quiet, as 2020's `PlayerCastSpell` did.
    SpellError(SpellError),
    /// The table learned a spell.
    Learned(SpellId),
    /// She woke at the party's last bed or fire.
    WokeAtRest,
    /// She woke at the door she came in by (no rest point at all: a county without a fire).
    WokeAtDoor,
}

/// What happened to a quest (`Quest` events, party-wide).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestChange {
    Given,
    Progress,
    Ready,
    Done,
}

/// Why a cast failed (`combat.ts SpellError`). Failure is an enum, never a silent no-op.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpellError {
    /// Stunned, or a world spell that found nothing to do.
    CastUnsuccessful,
    YouAreDead,
    OnCooldown,
    OnGcd,
    TooFar,
    NoTarget,
    NotEnoughMp,
    NotEnoughEnergy,
    NotInLos,
    NotValidTarget,
}

impl SpellError {
    /// Worth a toast. GCD, cooldown and a plain failure stay quiet.
    pub const fn says(self) -> bool {
        !matches!(self, SpellError::CastUnsuccessful | SpellError::OnCooldown | SpellError::OnGcd)
    }
}

/// What a sound is of, where no other event carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxKind {
    /// A `Strike` landed on its rect.
    Strike,
    /// Tried a lock without its key; a door not answered after dark.
    Locked,
    /// Something heavy moved a cell; a hop through a vent.
    Push,
    PlateDown,
    PlateUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropChange {
    Show,
    Hide,
    Lock,
    Unlock,
    Switch,
    /// A chest opened, a thing gathered.
    Open,
    /// Lifted, a bench touched, a world spell landed.
    Use,
    /// Moved a cell (pushed, pulled, put down).
    Push,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Toast(ToastKind),
    /// She arrived in a zone; `first` the first time anyone did.
    Zone {
        zone: ZoneId,
        first: bool,
    },
    /// The head count changed (the party penalty follows it).
    Party {
        connected: u8,
    },
    Prop {
        prop: PropId,
        change: PropChange,
    },
    Tiles(Rect),
    Spawned {
        unit: UnitId,
    },
    Despawned {
        unit: UnitId,
    },
    /// A name an action list used was not found (dev builds read it; content checks make it rare).
    Missing(Sym),
    /// Hit points lost (whole points, after resists, the party table and any shield).
    Damage {
        unit: UnitId,
        from: Option<UnitId>,
        at: Vec2,
        amount: Milli,
        school: School,
        crit: bool,
        /// What a mana shield took instead.
        absorbed: Milli,
    },
    /// Hit points gained (only what was missing).
    Heal {
        unit: UnitId,
        from: Option<UnitId>,
        at: Vec2,
        amount: Milli,
    },
    Death {
        unit: UnitId,
        def: UnitDefId,
        at: Vec2,
    },
    /// A creature stood up again at home, or a seat woke.
    Respawn {
        unit: UnitId,
    },
    Cast {
        unit: UnitId,
        spell: SpellId,
        at: Vec2,
    },
    CastFailed {
        unit: UnitId,
        spell: SpellId,
        why: SpellError,
    },
    /// A spell landed: a melee blow, or a bolt at its end.
    Impact {
        spell: SpellId,
        school: School,
        at: Vec2,
    },
    /// A melee swing, whether or not it found anyone.
    Swing {
        unit: UnitId,
        at: Vec2,
        facing: Facing,
    },
    Status {
        unit: UnitId,
        effect: EffectId,
        on: bool,
    },
    /// The screen shakes for whoever it is sent to (1 to 4).
    Shake(u8),
    Sfx {
        kind: SfxKind,
        at: Vec2,
    },
    /// Her body fell (to her alone).
    PlayerDied,
    // --- the world verbs (interact, inventory, quests, dialogue, journal) ----------------
    Quest {
        quest: QuestId,
        change: QuestChange,
    },
    /// Her bag or craft row changed.
    Bag,
    /// Her conversation opened, moved on or closed.
    Dialogue,
    /// Into her bag.
    Loot {
        item: ItemId,
        qty: u16,
    },
    /// A spell is the world's now.
    Learn(SpellId),
    /// She rested: the app writes the save. Party-wide.
    Rest,
    Camera {
        mode: CameraMode,
        rect: Option<Sym>,
    },
    /// Something new is understood (a new fact, or a stronger source for one). Party-wide.
    Journal(JournalKind),
    // --- the living world (`living.rs`) ----------------------------------------------------
    /// The sky over her changed: its region's sky turned, or she walked under another region's.
    /// Personal (each seat is told of her own sky).
    Weather {
        region: jane_data::Region,
        kind: WeatherKind,
    },
    /// Something the county does for good has happened (§4.6.d). Party-wide.
    Consequence(jane_core::ConsequenceId),
}

impl EventKind {
    /// Kinds that belong to one person, not to everyone who can see the spot (`sim.ts PERSONAL`).
    pub const fn personal(self) -> bool {
        matches!(
            self,
            EventKind::Toast(_)
                | EventKind::Zone { .. }
                | EventKind::Bag
                | EventKind::Dialogue
                | EventKind::Loot { .. }
                | EventKind::Learn(_)
                | EventKind::Shake(_)
                | EventKind::Camera { .. }
                | EventKind::CastFailed { .. }
                | EventKind::PlayerDied
                | EventKind::Weather { .. }
        )
    }
}

/// What one seat should see and hear: her own, her zone's, and the party's.
pub fn events_for<'a>(events: &'a [Event], me: &'a PlayerState) -> impl Iterator<Item = &'a Event> + 'a {
    events.iter().filter(move |e| e.to.is_none_or(|s| s == me.seat) && e.in_zone.is_none_or(|z| z == me.zone))
}
