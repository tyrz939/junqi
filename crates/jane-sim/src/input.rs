//! Input and commands (ARCHITECTURE.md §3.4). Held input is one [`InputFrame`] per seat per
//! frame; discrete actions are [`Command`]s stamped with a seat and a sequence number, applied
//! at the start of the step in `(seat, seq)` order.
//!
//! The client turns sticks and cursors into `(Angle, magnitude)` with whatever arithmetic it
//! likes and quantises; the sim never normalises. The frame carries the raw aim and the assist
//! profile, never an assisted angle.

use jane_core::{Angle, ItemId, QuestId, SpellId, Sym, ZoneId};
use jane_data::BarSlot;
use serde::{Deserialize, Serialize};

use crate::ids::{ClientToken, PropId, Seat, UnitId};
use crate::tuning::MAX_PLAYERS;

/// Which aim assist the seat plays with (§5.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistProfile {
    #[default]
    Off,
    Pad,
    Mouse,
}

/// What a seat has chosen (PLAY-PLAN §2.1): a unit (a foe, a friend) or a prop (a blue torch, a
/// cracked wall, a socket, a bud). The client holds it and carries it in every frame; the sim
/// validates it each tick (`target.rs`): something gone, hidden or too far is no target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TargetRef {
    Unit(UnitId),
    Prop(PropId),
}

/// Held input for one seat for one frame. Twelve bytes on the wire ([`InputFrame::to_bytes`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFrame {
    pub mv_dir: Angle,
    /// 0..=127; 0 is standing still.
    pub mv_mag: u8,
    /// Raw aim from the cursor or right stick; `None`: along her facing.
    pub aim: Option<Angle>,
    pub sprint: bool,
    pub use_held: bool,
    pub assist: AssistProfile,
    /// Her hard target as the client holds it; `None`: free aim.
    pub target: Option<TargetRef>,
    /// The free-aim key held (or the right stick pushed): casts go along `aim` even with a target.
    pub free: bool,
}

impl InputFrame {
    pub const IDLE: InputFrame = InputFrame {
        mv_dir: Angle::EAST,
        mv_mag: 0,
        aim: None,
        sprint: false,
        use_held: false,
        assist: AssistProfile::Off,
        target: None,
        free: false,
    };

    /// Walking along `dir` at full tilt.
    pub const fn walk(dir: Angle) -> InputFrame {
        InputFrame { mv_dir: dir, mv_mag: 127, ..InputFrame::IDLE }
    }

    /// `[dir lo, dir hi, mag, aim lo, aim hi, flags, id 0..=3, 0, 0]`; flags: bit 0 sprint, 1 use,
    /// 2 aim present, 3..=4 assist, 5 free aim, 6..=7 the target's kind (0 none, 1 unit, 2 prop).
    pub fn to_bytes(self) -> [u8; 12] {
        let aim = self.aim.unwrap_or_default().0;
        let (kind, id) = match self.target {
            None => (0u8, 0u32),
            Some(TargetRef::Unit(u)) => (1, u.get()),
            Some(TargetRef::Prop(p)) => (2, p.get()),
        };
        let flags = u8::from(self.sprint)
            | u8::from(self.use_held) << 1
            | u8::from(self.aim.is_some()) << 2
            | (self.assist as u8) << 3
            | u8::from(self.free) << 5
            | kind << 6;
        let [d0, d1] = self.mv_dir.0.to_le_bytes();
        let [a0, a1] = aim.to_le_bytes();
        let [i0, i1, i2, i3] = id.to_le_bytes();
        [d0, d1, self.mv_mag.min(127), a0, a1, flags, i0, i1, i2, i3, 0, 0]
    }

    pub fn from_bytes(b: [u8; 12]) -> InputFrame {
        let flags = b[5];
        let id = u32::from_le_bytes([b[6], b[7], b[8], b[9]]);
        InputFrame {
            free: flags & 32 != 0,
            target: match flags >> 6 {
                1 => UnitId::new(id).map(TargetRef::Unit),
                2 => PropId::new(id).map(TargetRef::Prop),
                _ => None,
            },
            mv_dir: Angle(u16::from_le_bytes([b[0], b[1]])),
            mv_mag: b[2].min(127),
            aim: (flags & 4 != 0).then_some(Angle(u16::from_le_bytes([b[3], b[4]]))),
            sprint: flags & 1 != 0,
            use_held: flags & 2 != 0,
            assist: match (flags >> 3) & 3 {
                1 => AssistProfile::Pad,
                2 => AssistProfile::Mouse,
                _ => AssistProfile::Off,
            },
        }
    }
}

/// A console mutation (`app/terminal.ts`): ordinary commands, recorded like any other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DevOp {
    Give {
        item: ItemId,
        qty: u16,
    },
    God(bool),
    /// Travel now to a zone's mark (at the end of this step).
    Tp {
        zone: ZoneId,
        mark: Sym,
    },
    /// Set the clock to an hour of the day.
    Time {
        hour: u8,
    },
    Hp(i32),
    Mp(i32),
    Learn(SpellId),
    Quest(QuestId),
    Flag {
        flag: Sym,
        value: i32,
    },
    Kill,
    Spawn(jane_core::UnitDefId),
    /// Growth as a finding would give it (the party's), with nothing found: a test kit carries
    /// what the content offers before a place (the bot's dungeon crawl).
    Grow {
        stat: jane_core::action::Stat,
        amount: i16,
    },
    /// Made fires, a fire's rest over time and unbanked finds (`GameState::fires_made`) on or off.
    Fires(bool),
}

/// A discrete action, addressed to a seat.
///
/// Handled: `Join`, `Leave`, `Open`, `Bind`, `Unbind`, `BarSwap`, `CloseDialogue`,
/// `Dev(God | Tp | Time | Flag | Grow)` (seats); `Bar`, `Cast`, `Dev(Hp | Mp | Learn | Kill | Spawn)`
/// (combat). The rest are no-ops until their owners land (interact: `Use`; inventory: `Item`,
/// `Bag*`, `Craft*`, `Dev(Give)`, a bar slot holding an item; store: `Store*`; dialogue: `Advance`, `Choose`;
/// quests: `Dev(Quest)`, `Abandon`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Use,
    /// A bar slot pressed. `on` as for `Cast`.
    Bar {
        slot: u8,
        on: Option<UnitId>,
    },
    /// Cast along this frame's aim (assisted by its profile, ARCHITECTURE.md §5.4). `on` is
    /// what a friendly spell reads: with a cursor, the unit under it when it was pressed, and
    /// the caster's own body for "over nobody"; `None` only without a cursor (a pad), when a
    /// friendly spell lands on the friend nearest the aim line.
    Cast {
        spell: SpellId,
        on: Option<UnitId>,
    },
    Item(ItemId),
    BagMove {
        from: u8,
        to: u8,
    },
    BagDestroy {
        slot: u8,
    },
    CraftPut {
        bag: u8,
        slot: u8,
    },
    CraftClear {
        slot: u8,
    },
    CraftClearAll,
    CraftTake,
    /// Bag slot `bag` into cupboard `prop` (`store.rs`): onto its slot `to`, or wherever it
    /// fits (`None`: a quick move, shift-click).
    StorePut {
        prop: PropId,
        bag: u8,
        to: Option<u8>,
    },
    /// Cupboard `prop`'s slot `slot` into her bag: onto bag slot `to`, or wherever it fits.
    StoreTake {
        prop: PropId,
        slot: u8,
        to: Option<u8>,
    },
    /// One of cupboard `prop`'s slots onto another.
    StoreMove {
        prop: PropId,
        from: u8,
        to: u8,
    },
    /// Everything in her bag that fits, into cupboard `prop`.
    StorePutAll {
        prop: PropId,
    },
    Bind {
        slot: u8,
        to: BarSlotWire,
    },
    Unbind {
        slot: u8,
    },
    BarSwap {
        a: u8,
        b: u8,
    },
    Advance,
    Choose {
        option: u8,
    },
    CloseDialogue,
    /// Someone sits down (stamped with seat `None`); the seat is assigned in order.
    Join {
        who: ClientToken,
    },
    /// Someone gets up. Her body and what she owned wait for her; what the story needs is handed on.
    Leave,
    /// Seat 0 only: let others sit down, or stop letting them.
    Open(bool),
    Dev(DevOp),
    /// Set a side quest aside (`quests::abandon`): any seat may, for the whole party, as any
    /// seat may take one; the main line refuses. Last, so older tapes keep their variant numbers.
    Abandon(QuestId),
    /// Click to move (PLAY-PLAN §2.1): walk there on a path through what she has seen and do
    /// what the place asks (`walk.rs`). Any held move cancels it.
    Goto(Goto),
    /// Esc: stop swinging, walking and casting (an unfinished cast costs nothing).
    Halt,
    /// Her hop (`feel::hop`): along this frame's stick, else her facing.
    Hop,
    /// The table's input delay D in frames (stamped with seat `None`, from the host): every
    /// foe's wind-up is that much longer (`GameState::table_delay`).
    Table {
        delay: u8,
    },
}

/// Where a right-click sends her.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Goto {
    /// Open ground: walk there.
    Ground(jane_core::Vec2),
    /// A foe: into reach, then swing (a caster already in range of it only targets it). A
    /// friend or a person: over to her, then talk.
    Unit(UnitId),
    /// A door, a fire, a chest, anything used: over to it, then use it.
    Prop(PropId),
}

/// A bar slot as a command carries it (the catalog's `BarSlot` has no serde).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarSlotWire {
    Spell(SpellId),
    Item(ItemId),
}

impl From<BarSlotWire> for BarSlot {
    fn from(w: BarSlotWire) -> BarSlot {
        match w {
            BarSlotWire::Spell(s) => BarSlot::Spell(s),
            BarSlotWire::Item(i) => BarSlot::Item(i),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StampedCommand {
    /// `None` = a join.
    pub seat: Option<Seat>,
    pub seq: u16,
    pub cmd: Command,
}

/// One frame of input for the whole table. `commands` are sorted `(seat, seq)`: joins first.
#[derive(Clone, Copy, Debug)]
pub struct StepInput<'a> {
    pub frames: [InputFrame; MAX_PLAYERS],
    pub commands: &'a [StampedCommand],
}

impl StepInput<'_> {
    pub const IDLE: StepInput<'static> = StepInput { frames: [InputFrame::IDLE; MAX_PLAYERS], commands: &[] };

    /// Seat 0 holding `frame`, nobody else doing anything.
    pub fn solo(frame: InputFrame) -> StepInput<'static> {
        let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
        frames[0] = frame;
        StepInput { frames, commands: &[] }
    }
}

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stepped {
    /// Time advanced (the world was not frozen).
    pub ran: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_twelve_bytes_and_round_trips() {
        let f = InputFrame {
            mv_dir: Angle(40_000),
            mv_mag: 99,
            aim: Some(Angle(123)),
            sprint: true,
            use_held: false,
            assist: AssistProfile::Mouse,
            target: PropId::new(70_000).map(TargetRef::Prop),
            free: true,
        };
        assert_eq!(InputFrame::from_bytes(f.to_bytes()), f);
        assert_eq!(InputFrame::from_bytes(InputFrame::IDLE.to_bytes()), InputFrame::IDLE);
        let u = InputFrame { target: UnitId::new(3).map(TargetRef::Unit), ..InputFrame::IDLE };
        assert_eq!(InputFrame::from_bytes(u.to_bytes()), u);
        let wire = postcard::to_allocvec(&f).unwrap();
        assert!(wire.len() <= 16, "{}", wire.len());
    }
}
