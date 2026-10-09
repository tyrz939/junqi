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

pub mod atmosphere;
pub mod chunks;
pub mod combat;
pub mod county;
pub mod dungeon_themes;
pub mod dungeons;
pub mod living;
pub mod looks;
pub mod story;
pub mod tile_looks;

pub use atmosphere::*;
pub use chunks::*;
pub use combat::*;
pub use county::*;
pub use dungeon_themes::*;
pub use dungeons::*;
pub use living::*;
pub use looks::*;
pub use story::*;
pub use tile_looks::*;

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
        /// Night variants of texts, by day text then stage (NIGHT.md §7.1): [`Catalog::night_text`].
        /// Left out of the content hash while there are none, so a catalog without variants hashes
        /// as it did before they existed (`compile::content_hash`).
        pub night_texts: &'static [NightText],
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

model! {
    /// A text's night variant (NIGHT.md §7.1): read in place of `day` while the night's latched
    /// stage is at least `min` (1 to 4). Its own `TextId`, so reading it is its own claim.
    pub struct NightText {
        pub day: jane_core::TextId,
        pub min: u8,
        pub text: jane_core::TextId,
    }
}

/// The text read for `day` at night stage `stage` (0 by day): the deepest variant whose `min`
/// the stage reaches, else the day text. `table` is sorted by day text, then stage.
pub fn night_text_in(table: &[NightText], day: jane_core::TextId, stage: u8) -> jane_core::TextId {
    if stage == 0 {
        return day;
    }
    let from = table.partition_point(|n| n.day < day);
    table[from..].iter().take_while(|n| n.day == day).filter(|n| n.min <= stage).last().map_or(day, |n| n.text)
}

impl Catalog {
    /// The text read for `day` at night stage `stage` ([`night_text_in`]).
    pub fn night_text(&self, day: jane_core::TextId, stage: u8) -> jane_core::TextId {
        night_text_in(self.night_texts, day, stage)
    }

    /// The day text a night variant varies, if `t` is one.
    pub fn day_text_of(&self, t: jane_core::TextId) -> Option<jane_core::TextId> {
        self.night_texts.iter().find(|n| n.text == t).map(|n| n.day)
    }

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
