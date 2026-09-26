//! Typed events for presentation (ARCHITECTURE.md §11). No English: ids only; `jane-present`
//! owns the strings. Routing is the TS rule: personal kinds carry `to`, zone-local kinds carry
//! `in_zone`, party-wide kinds carry neither; [`events_for`] filters.
//!
//! Only the kinds this unit emits are here; each later unit adds its own (the enum is the list
//! §11 gives, grown as the systems land).

use jane_core::{Rect, Sym, TextRef, ZoneId};

use crate::ids::{PropId, Seat, UnitId};
use crate::state::PlayerState;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropChange {
    Show,
    Hide,
    Lock,
    Unlock,
    Switch,
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
}

impl EventKind {
    /// Kinds that belong to one person, not to everyone who can see the spot.
    pub const fn personal(self) -> bool {
        matches!(self, EventKind::Toast(_) | EventKind::Zone { .. })
    }
}

/// What one seat should see and hear: her own, her zone's, and the party's.
pub fn events_for<'a>(events: &'a [Event], me: &'a PlayerState) -> impl Iterator<Item = &'a Event> + 'a {
    events.iter().filter(move |e| e.to.is_none_or(|s| s == me.seat) && e.in_zone.is_none_or(|z| z == me.zone))
}
