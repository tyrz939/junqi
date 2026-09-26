//! Instance ids, seats and the counters that issue ids (ARCHITECTURE.md §3.1).
//!
//! Every instance kind has one monotonic counter in [`Counters`] (`GameState.next`); an id is
//! never reused, so it is stable across saves. Generational arenas were rejected: four billion
//! per kind is unreachable, and a counter that overflows panics under the `checked` profile.

use std::fmt;
use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

macro_rules! instance_ids {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub NonZeroU32);

        impl $name {
            /// The id as a number (for tests, tools and derived per-id values).
            pub const fn get(self) -> u32 {
                self.0.get()
            }

            /// `None` for 0.
            pub const fn new(n: u32) -> Option<$name> {
                match NonZeroU32::new(n) {
                    Some(v) => Some($name(v)),
                    None => None,
                }
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    )*};
}

instance_ids! {
    UnitId,
    PropId,
    DropId,
    ProjId,
    GroundId,
}

/// The last id issued per kind; the next is one more. Starts at zero: the first id is 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Counters {
    pub unit: u32,
    pub prop: u32,
    pub drop: u32,
    pub proj: u32,
    pub ground: u32,
}

fn bump(c: &mut u32) -> NonZeroU32 {
    *c += 1;
    NonZeroU32::new(*c).expect("an id counter starts at zero and only grows")
}

impl Counters {
    pub fn unit(&mut self) -> UnitId {
        UnitId(bump(&mut self.unit))
    }
    pub fn prop(&mut self) -> PropId {
        PropId(bump(&mut self.prop))
    }
    pub fn drop(&mut self) -> DropId {
        DropId(bump(&mut self.drop))
    }
    pub fn proj(&mut self) -> ProjId {
        ProjId(bump(&mut self.proj))
    }
    pub fn ground(&mut self) -> GroundId {
        GroundId(bump(&mut self.ground))
    }
}

/// A seat at the table, 0..=3. Stable for the life of the world; inputs and commands are
/// addressed to it. It is also her coat colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Seat(pub u8);

impl Seat {
    pub const HOST: Seat = Seat(0);

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Who sits in a seat: a token the client keeps, never shown. It is how a returning guest gets
/// her own body and bags back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientToken(pub u64);

impl ClientToken {
    /// The person whose machine holds the world.
    pub const HOST: ClientToken = ClientToken(0);
}

/// An index into `ZoneState.props`. Props are never removed and only appended, so the index
/// order is the id order.
pub type PropIx = u32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_issue_from_one_and_never_repeat() {
        let mut c = Counters::default();
        assert_eq!(c.unit().get(), 1);
        assert_eq!(c.unit().get(), 2);
        assert_eq!(c.prop().get(), 1);
        assert_eq!(UnitId::new(0), None);
        assert_eq!(format!("{:?}", UnitId::new(7).unwrap()), "UnitId(7)");
    }
}
