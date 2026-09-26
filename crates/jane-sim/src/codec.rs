//! Serde for the few catalog enums the state holds. `jane-schema`'s model builds without serde
//! (the game links only the model), so these ride as their row index, a `u8`, through
//! `#[serde(with = ...)]`. A content change that reorders them changes `content_hash`, and a
//! save across that is refused anyway (ARCHITECTURE.md §3.5).

use jane_data::{BarSlot, Controller, Faction};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod faction {
    use super::*;

    pub fn serialize<S: Serializer>(f: &Faction, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(*f as u8)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Faction, D::Error> {
        Ok(match u8::deserialize(d)? {
            0 => Faction::Undead,
            1 => Faction::Beast,
            2 => Faction::Bandit,
            3 => Faction::Friendly,
            n => return Err(D::Error::custom(format_args!("no faction {n}"))),
        })
    }
}

pub mod controller {
    use super::*;

    pub fn serialize<S: Serializer>(c: &Controller, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(*c as u8)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Controller, D::Error> {
        Ok(match u8::deserialize(d)? {
            0 => Controller::Player,
            1 => Controller::Ai,
            2 => Controller::Npc,
            3 => Controller::Snake,
            n => return Err(D::Error::custom(format_args!("no controller {n}"))),
        })
    }
}

/// The action bar: each slot `None`, `(0, spell)` or `(1, item)`.
pub mod bar {
    use jane_core::{ItemId, SpellId};

    use super::*;
    use crate::tuning::BAR_SLOTS;

    type Wire = [Option<(u8, u16)>; BAR_SLOTS];

    pub fn serialize<S: Serializer>(bar: &[Option<BarSlot>; BAR_SLOTS], s: S) -> Result<S::Ok, S::Error> {
        let w: Wire = bar.map(|b| {
            b.map(|b| match b {
                BarSlot::Spell(x) => (0, x.0),
                BarSlot::Item(x) => (1, x.0),
            })
        });
        w.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[Option<BarSlot>; BAR_SLOTS], D::Error> {
        let w = Wire::deserialize(d)?;
        let mut out = [None; BAR_SLOTS];
        for (o, x) in out.iter_mut().zip(w) {
            *o = match x {
                None => None,
                Some((0, v)) => Some(BarSlot::Spell(SpellId(v))),
                Some((1, v)) => Some(BarSlot::Item(ItemId(v))),
                Some((t, _)) => return Err(D::Error::custom(format_args!("no bar slot kind {t}"))),
            };
        }
        Ok(out)
    }
}
