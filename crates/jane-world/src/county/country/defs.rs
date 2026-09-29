//! Every prop, unit and dialogue the country furnishers name, looked up in the catalog once
//! (the scans `prop_id` and friends do are for tools, never a hot path). A name the catalog lacks
//! is a content error the county tests catch on their first seed.

use std::sync::OnceLock;

use jane_core::{DialogueId, PropDefId, UnitDefId};

macro_rules! table {
    (@id $f:ident) => {
        stringify!($f)
    };
    (@id $f:ident $id:literal) => {
        $id
    };
    ($(#[$m:meta])* $name:ident: $ty:ty = $look:ident, $what:literal { $($f:ident $(= $id:literal)?),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug)]
        pub struct $name {
            $(pub $f: $ty,)*
        }

        impl $name {
            fn load() -> Self {
                let cat = jane_data::catalog();
                Self {
                    $($f: {
                        let id = table!(@id $f $($id)?);
                        $look(cat, id).unwrap_or_else(|| panic!("no {} row {}", $what, id))
                    },)*
                }
            }
        }
    };
}

fn prop(cat: &jane_data::Catalog, id: &str) -> Option<PropDefId> {
    cat.story.prop_id(id)
}

fn unit(cat: &jane_data::Catalog, id: &str) -> Option<UnitDefId> {
    cat.combat.unit_id(id)
}

fn talk(cat: &jane_data::Catalog, id: &str) -> Option<DialogueId> {
    cat.story.dialogue_id(id)
}

table! {
    /// The props.
    Props: PropDefId = prop, "prop" {
        lamp_post, lamp_run, relay_box, fingerpost, milestone, signpost, sign, herb, rock,
        wayside_shrine, flowers, well, well_head, trough, log, cart_wreck, road_cart, crate_ = "crate", barrel,
        bones, chest, haystack, hay_cart, beehive, cottage_thatch, cottage_timber, cottage_slate,
        cottage_tile, cottage_empty, flowerbed, hen_coop, crop, washing_line, woodpile, farmhouse,
        barn, pump, scarecrow, inn, apple_tree, stump, shed, tent, campfire_cold, camp_fire,
        standing_stone, bedroll, sleepers, web, den, boulder, reed_hut, slag_heap, minecart,
        gravestone, pillar,
    }
}

table! {
    /// The people, the animals and what bites.
    Units: UnitDefId = unit, "unit" {
        rat, skeleton, yard_bones, crow, bat, pumpkin, spider, flower, statue, soldier, skeleton_guard,
        skeleton_clerk, wall_spider, cactus, ruffian, night_skeleton, night_soldier, hen, sheep, rabbit, folk_old, folk_woman,
        folk_man, folk_wife, folk_farmer, folk_keeper, folk_regular, folk_orchard, folk_woodcutter,
        folk_reedcutter,
    }
}

table! {
    /// The trees a door, a well or a den answers with.
    Talks: DialogueId = talk, "dialogue" {
        country_shrine, country_well, country_hive, country_coop, country_barn, country_pump,
        country_inn, country_shed, country_tent, country_den, country_door_empty, country_grave,
        country_door_1, country_door_2, country_door_3, country_door_4,
    }
}

/// The three tables. What a chest or a tree holds is data (`tuning/country.json`, the catalog's
/// `county.furnishing`), read where it is placed.
#[derive(Debug)]
pub struct Defs {
    pub p: Props,
    pub u: Units,
    pub t: Talks,
}

/// The tables, looked up on first use.
pub fn defs() -> &'static Defs {
    static DEFS: OnceLock<Defs> = OnceLock::new();
    DEFS.get_or_init(|| Defs { p: Props::load(), u: Units::load(), t: Talks::load() })
}

impl Defs {
    /// The four looks a cottage has, in the TypeScript's order.
    pub fn cottages(&self) -> [PropDefId; 4] {
        [self.p.cottage_thatch, self.p.cottage_timber, self.p.cottage_slate, self.p.cottage_tile]
    }

    /// The four front doors a cottage answers with.
    pub fn doors(&self) -> [DialogueId; 4] {
        [self.t.country_door_1, self.t.country_door_2, self.t.country_door_3, self.t.country_door_4]
    }

    /// Whoever lives in a cottage or about a hamlet's green.
    pub fn people(&self) -> [UnitDefId; 4] {
        [self.u.folk_old, self.u.folk_woman, self.u.folk_man, self.u.folk_wife]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_name_is_in_the_catalog() {
        let d = super::defs();
        assert_ne!(d.p.lamp_post, d.p.fingerpost);
    }
}
