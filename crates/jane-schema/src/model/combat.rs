//! Spells, effects, units, items, recipes.

use jane_core::action::ListRef;
use jane_core::ids::{ItemId, NameId, SpriteId, TextId};
use jane_core::num::Tick;

use crate::model;

model! {
    /// An item row (`data/items.json` and `data/items/*.json`).
    pub struct ItemDef {
        /// The content id (`"julies_letter"`): for tools, saves' debug views and errors.
        pub id: &'static str,
        pub name: TextId,
        pub description: TextId,
        pub icon: SpriteId,
        pub max_stack: u16,
        pub usable: bool,
        pub cooldown: Tick,
        /// Runs on the user; consumes one unless `keep`.
        pub use_list: Option<ListRef>,
        pub keep: bool,
        /// Key tag: unlocks any prop whose `keyTag` matches. One use path for every key.
        pub opens: Option<NameId>,
        /// Cannot be destroyed or dropped.
        pub bound: bool,
        /// Derived: what the story cannot go on without (anything that `opens`, anything a quest
        /// asks the party to acquire). Never ages out on the ground; handed on when its holder leaves.
        pub story: bool,
    }
}

model! {
    /// A recipe. `inputs` are sorted by id: a recipe is keyed by its sorted inputs, never by a
    /// display name (renaming Pansy killed a 2020 recipe).
    pub struct RecipeDef {
        pub inputs: &'static [ItemId],
        pub output: ItemId,
        pub qty: u16,
    }
}

model! {
    pub struct Combat {
        /// Indexed by `ItemId`.
        pub items: &'static [ItemDef],
        /// Indexed by `RecipeId`, in file order.
        pub recipes: &'static [RecipeDef],
    }
}

impl Combat {
    pub fn item(&self, id: ItemId) -> &'static ItemDef {
        &self.items[id.index()]
    }

    /// The recipe whose sorted inputs are exactly `inputs` (sorted by the caller).
    pub fn recipe_for(&self, inputs: &[ItemId]) -> Option<&'static RecipeDef> {
        self.recipes.iter().find(|r| r.inputs == inputs)
    }
}
