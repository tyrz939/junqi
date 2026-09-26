//! The compiled catalog: the types the game runs on. `jane-data` holds one as a static built
//! from `/data`; the `dev-data` feature builds the same value at startup through the same code
//! (ARCHITECTURE.md §6). Everything is `&'static`, `Copy` and integer.
//!
//! One module per table group, each owned by one unit of work (PORT.md §8): `combat`, `story`,
//! `county`, `dungeons`. A group adds its tables as fields of its own struct and never edits
//! another group's file.

use jane_core::action::{Action, Cond};
use jane_core::ids::Key;

use crate::model;

pub mod chunks;
pub mod combat;
pub mod county;
pub mod dungeons;
pub mod living;
pub mod story;

pub use chunks::*;
pub use combat::*;
pub use county::*;
pub use dungeons::*;
pub use living::*;
pub use story::*;

model! {
    /// Everything content says, compiled.
    pub struct Catalog {
        /// xxh3 of everything that changes behaviour (text excluded): the save and lockstep pin.
        pub content_hash: u64,
        /// `NameId` to its string: contract keys, marks, rects, flags, story names.
        pub names: &'static [&'static str],
        /// `TextId` to its English.
        pub texts: &'static [&'static str],
        /// `SpriteId` to the sprite or icon name a look is keyed by.
        pub sprites: &'static [&'static str],
        /// `ListRef::Catalog` targets.
        pub lists: &'static [&'static [Action]],
        /// `CondsRef::Catalog` targets.
        pub conds: &'static [&'static [Cond]],
        /// `NamesRef::Catalog` targets.
        pub name_lists: &'static [&'static [Key]],
        pub combat: Combat,
        pub story: Story,
        pub county: County,
        pub dungeons: Dungeons,
        /// The county's authored places (`data/chunks`), after the groups whose names they use.
        pub chunks: Chunks,
        /// The living world (ARCHITECTURE.md §4.6): weather, ecology, consequences, the sim's
        /// tuning. Compiled last, so the names and texts it adds come after everyone else's.
        pub living: Living,
    }
}

impl Catalog {
    pub fn list(&self, r: jane_core::ListRef) -> &'static [Action] {
        match r {
            jane_core::ListRef::Catalog(i) => self.lists[usize::from(i)],
            jane_core::ListRef::Blueprint(_) => &[],
        }
    }

    pub fn conds_of(&self, r: jane_core::CondsRef) -> &'static [Cond] {
        match r {
            jane_core::CondsRef::Catalog(i) => self.conds[usize::from(i)],
            jane_core::CondsRef::Blueprint(_) => &[],
        }
    }

    pub fn names_of(&self, r: jane_core::NamesRef) -> &'static [Key] {
        match r {
            jane_core::NamesRef::Catalog(i) => self.name_lists[usize::from(i)],
            jane_core::NamesRef::Blueprint(_) => &[],
        }
    }

    pub fn name(&self, n: jane_core::NameId) -> &'static str {
        self.names[n.index()]
    }

    pub fn text(&self, t: jane_core::TextId) -> &'static str {
        self.texts[t.index()]
    }

    /// The `NameId` of a content name, by a scan: for tools and tests, never the hot path.
    pub fn name_id(&self, s: &str) -> Option<jane_core::NameId> {
        self.names.iter().position(|n| *n == s).map(|i| jane_core::NameId(i as u16))
    }
}
