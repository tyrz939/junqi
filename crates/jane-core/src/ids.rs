//! Ids (ARCHITECTURE.md §3.1). Content rows are `u16` newtypes fixed at compile time, in
//! sorted-string order per table; `jane-data` emits the tables they index.

use std::fmt;

macro_rules! row_ids {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub u16);

        impl $name {
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    )*};
}

row_ids! {
    SpellId,
    EffectId,
    ItemId,
    UnitDefId,
    PropDefId,
    QuestId,
    DialogueId,
    TriggerId,
    RecipeId,
    /// Every English string content names: a line, a toast, a label, a quest's text.
    TextId,
    StoryId,
    ConsequenceId,
    /// A look: the art of a sprite or an icon (ART.md §5).
    SpriteId,
    /// A dungeon mission (`data/dungeons/*.json`).
    DungeonId,
    PoolId,
    TemplateId,
    /// Every name content uses: contract keys, marks, rects, flags, `location`, `grow` ids,
    /// and every derived `zone_node_socket` name.
    NameId,
}

/// A name at runtime: interned by the sim, pre-seeded so `Sym(n) == NameId(n)` below the
/// content's name count. Generator-only names append; a save stores only the tail, as strings.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Sym(pub u32);

impl From<NameId> for Sym {
    fn from(n: NameId) -> Sym {
        Sym(u32::from(n.0))
    }
}

/// A name as a blueprint holds it: a content name, or one the generator made
/// (`Blueprint::local_names`). The sim interns locals into `Sym`s when the zone loads, so
/// worldgen never touches runtime state.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Key {
    Name(NameId),
    Local(u32),
}

/// The thirteen zones, in tick order: `data/zones.json`'s order, which `jane-schema` checks
/// against this list.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum ZoneId {
    County,
    House,
    Cellar,
    Mine,
    Burial,
    Arms,
    Church,
    Factory,
    Forest,
    Library,
    Museum,
    Pipes,
    School,
}

pub const ZONE_COUNT: usize = 13;

impl ZoneId {
    pub const ALL: [ZoneId; ZONE_COUNT] = [
        ZoneId::County,
        ZoneId::House,
        ZoneId::Cellar,
        ZoneId::Mine,
        ZoneId::Burial,
        ZoneId::Arms,
        ZoneId::Church,
        ZoneId::Factory,
        ZoneId::Forest,
        ZoneId::Library,
        ZoneId::Museum,
        ZoneId::Pipes,
        ZoneId::School,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The id content uses (`"county"`).
    pub const fn name(self) -> &'static str {
        match self {
            ZoneId::County => "county",
            ZoneId::House => "house",
            ZoneId::Cellar => "cellar",
            ZoneId::Mine => "mine",
            ZoneId::Burial => "burial",
            ZoneId::Arms => "arms",
            ZoneId::Church => "church",
            ZoneId::Factory => "factory",
            ZoneId::Forest => "forest",
            ZoneId::Library => "library",
            ZoneId::Museum => "museum",
            ZoneId::Pipes => "pipes",
            ZoneId::School => "school",
        }
    }

    pub fn from_name(s: &str) -> Option<ZoneId> {
        ZoneId::ALL.into_iter().find(|z| z.name() == s)
    }
}

impl From<ZoneId> for u16 {
    fn from(z: ZoneId) -> u16 {
        z as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_names_round_trip_in_order() {
        for (i, z) in ZoneId::ALL.into_iter().enumerate() {
            assert_eq!(z.index(), i);
            assert_eq!(ZoneId::from_name(z.name()), Some(z));
        }
        assert_eq!(ZoneId::from_name("icehouse"), None);
    }

    #[test]
    fn syms_extend_names() {
        assert_eq!(Sym::from(NameId(7)), Sym(7));
        assert_eq!(format!("{:?}", ItemId(3)), "ItemId(3)");
    }
}
