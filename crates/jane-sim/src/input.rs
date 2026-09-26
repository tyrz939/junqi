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

use crate::ids::{ClientToken, Seat, UnitId};
use crate::tuning::MAX_PLAYERS;

/// Which aim assist the seat plays with (§5.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistProfile {
    #[default]
    Off,
    Pad,
    Mouse,
}

/// Held input for one seat for one frame. Seven bytes on the wire ([`InputFrame::to_bytes`]).
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
}

impl InputFrame {
    pub const IDLE: InputFrame = InputFrame {
        mv_dir: Angle::EAST,
        mv_mag: 0,
        aim: None,
        sprint: false,
        use_held: false,
        assist: AssistProfile::Off,
    };

    /// Walking along `dir` at full tilt.
    pub const fn walk(dir: Angle) -> InputFrame {
        InputFrame { mv_dir: dir, mv_mag: 127, ..InputFrame::IDLE }
    }

    /// `[dir lo, dir hi, mag, aim lo, aim hi, flags, 0]`; flags: bit 0 sprint, 1 use, 2 aim
    /// present, 3..=4 assist.
    pub fn to_bytes(self) -> [u8; 7] {
        let aim = self.aim.unwrap_or_default().0;
        let flags = u8::from(self.sprint)
            | u8::from(self.use_held) << 1
            | u8::from(self.aim.is_some()) << 2
            | (self.assist as u8) << 3;
        let [d0, d1] = self.mv_dir.0.to_le_bytes();
        let [a0, a1] = aim.to_le_bytes();
        [d0, d1, self.mv_mag.min(127), a0, a1, flags, 0]
    }

    pub fn from_bytes(b: [u8; 7]) -> InputFrame {
        let flags = b[5];
        InputFrame {
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
}

/// A discrete action, addressed to a seat.
///
/// Handled: `Join`, `Leave`, `Open`, `Bind`, `Unbind`, `BarSwap`, `CloseDialogue`,
/// `Dev(God | Tp | Time | Flag | Grow)` (seats); `Bar`, `Cast`, `Dev(Hp | Mp | Learn | Kill | Spawn)`
/// (combat). The rest are no-ops until their owners land (interact: `Use`; inventory: `Item`,
/// `Bag*`, `Craft*`, `Dev(Give)`, a bar slot holding an item; dialogue: `Advance`, `Choose`;
/// quests: `Dev(Quest)`).
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
    fn a_frame_is_seven_bytes_and_round_trips() {
        let f = InputFrame {
            mv_dir: Angle(40_000),
            mv_mag: 99,
            aim: Some(Angle(123)),
            sprint: true,
            use_held: false,
            assist: AssistProfile::Mouse,
        };
        assert_eq!(InputFrame::from_bytes(f.to_bytes()), f);
        assert_eq!(InputFrame::from_bytes(InputFrame::IDLE.to_bytes()), InputFrame::IDLE);
        let wire = postcard::to_allocvec(&f).unwrap();
        assert!(wire.len() <= 11, "{}", wire.len());
    }
}
