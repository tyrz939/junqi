//! Names for the places the county generates (`jane/src/world/names.ts`): the hamlets, farms,
//! inns, cottages and woodcutters' clearings the country builds by the hundred, and the camps and
//! ruins a story sends her to. Every one of them gets a board with its name on it (the county's
//! stories stage), so a quest can say "the hen house at Hollins Farm" and she can walk the road
//! until she reads HOLLINS FARM.
//!
//! A name is a pure function of the seed and the story, never of the built county: text that says
//! `{place:<story>}` can be shown anywhere (the quest log in a dungeon, a save loaded underground)
//! without building anything, which is why [`story_name`] lives here and presentation calls it
//! (ARCHITECTURE.md §1). The county gives the story's name to whichever place the story claims,
//! and the places nobody claimed take the rest of the list in build order.
//!
//! The seed's order for a kind's list is a Fisher-Yates shuffle on [`Step::CountyNamePool`], keyed
//! by the kind, at attempt 0 whatever the county's attempt: a re-rolled county keeps its names.

use jane_core::{StoryId, ZoneId};
use jane_data::{PlaceKind, StoryDef};

use crate::steps::{Step, dice};

/// Past the end of a kind's list, the list again with a word in front, a word a lap.
const PAST: [&str; 6] = ["Upper", "Lower", "Little", "Great", "Far", "Old"];

/// Every name any seed could give a place of this kind, before the seed shuffles them (the
/// woodcutters' clearings already roots by ends). For the tests.
pub fn all_names(kind: PlaceKind) -> Vec<&'static str> {
    let cat = jane_data::catalog();
    cat.county.pool(kind).names.iter().map(|&t| cat.text(t)).collect()
}

/// The seed's order for a kind's names.
pub fn pool(seed: u32, kind: PlaceKind) -> Vec<&'static str> {
    let mut p = all_names(kind);
    dice(seed, ZoneId::County, Step::CountyNamePool, 0, kind as i32, 0).shuffle(&mut p);
    p
}

/// The `n`th name of a kind in a seed's order ([`pool`]). Past the end of the list, the list again
/// with a word in front ("Upper Holt End"; "The Upper Halfway House"). A kind with one name has it
/// everywhere: every inn on the road is the Halfway House (the innkeepers' own line in
/// `data/dialogue/country.json`: "it was halfway when it was painted").
pub fn nth_of(pool: &[&str], n: usize) -> String {
    if pool.is_empty() {
        return String::new();
    }
    if n < pool.len() || pool.len() == 1 {
        return pool[n.min(pool.len() - 1)].to_owned();
    }
    let lap = n / pool.len() - 1;
    let base = pool[n % pool.len()];
    let word = PAST[lap % PAST.len()];
    match base.strip_prefix("The ") {
        Some(rest) => format!("The {word} {rest}"),
        None => format!("{word} {base}"),
    }
}

/// The `n`th name of a kind on a seed.
pub fn nth_name(seed: u32, kind: PlaceKind, n: usize) -> String {
    nth_of(&pool(seed, kind), n)
}

/// How many stories take their names from a kind's list before the unclaimed places start on it:
/// the kind's stories with no name of their own.
pub fn stories_of_kind(kind: PlaceKind) -> usize {
    jane_data::catalog().county.stories.iter().filter(|s| s.kind == kind && s.name.is_none()).count()
}

/// Where in its kind's list a story without a name of its own draws: its place among those.
fn draws(s: &StoryDef) -> usize {
    let stories = jane_data::catalog().county.stories;
    stories.iter().filter(|x| x.kind == s.kind && x.name.is_none()).position(|x| x.id == s.id).unwrap_or(0)
}

/// The name a story's place has on this seed, whether or not the seed found it a place. A tale's
/// is its own, the same on every seed.
pub fn story_name(seed: u32, id: StoryId) -> String {
    let cat = jane_data::catalog();
    let s = cat.county.story(id);
    match s.name {
        Some(t) => cat.text(t).to_owned(),
        None => nth_name(seed, s.kind, draws(s)),
    }
}

/// The seed's names, every kind's list shuffled once, for a build that names many places.
#[derive(Clone, Debug)]
pub struct Names {
    pools: Vec<Vec<&'static str>>,
}

impl Names {
    pub fn new(seed: u32) -> Self {
        Self { pools: PlaceKind::ALL.iter().map(|&k| pool(seed, k)).collect() }
    }

    /// As [`nth_name`].
    pub fn nth(&self, kind: PlaceKind, n: usize) -> String {
        nth_of(&self.pools[kind as usize], n)
    }

    /// As [`story_name`].
    pub fn story(&self, id: StoryId) -> String {
        let cat = jane_data::catalog();
        let s = cat.county.story(id);
        match s.name {
            Some(t) => cat.text(t).to_owned(),
            None => self.nth(s.kind, draws(s)),
        }
    }
}

/// What the board at a named place says: the name in capitals in one of the lines a parish or a
/// farmer would paint (`data/names.json` `boards`), the `n`th round the kind's list.
pub fn board_text(kind: PlaceKind, name: &str, n: u32) -> String {
    let cat = jane_data::catalog();
    let lines = cat.county.pool(kind).boards;
    let line = if lines.is_empty() { "{NAME}." } else { cat.text(lines[n as usize % lines.len()]) };
    line.replace("{NAME}", &name.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_the_end_the_list_comes_round_with_a_word_in_front() {
        let p = ["Holt End", "The Crown", "Pennyfold"];
        assert_eq!(nth_of(&p, 0), "Holt End");
        assert_eq!(nth_of(&p, 3), "Upper Holt End");
        assert_eq!(nth_of(&p, 4), "The Upper Crown");
        assert_eq!(nth_of(&p, 8), "Lower Pennyfold");
        assert_eq!(nth_of(&["The Halfway House"], 7), "The Halfway House");
        assert_eq!(nth_of(&[], 2), "");
    }

    #[test]
    fn a_seed_shuffles_each_kind_its_own_way_and_keeps_every_name() {
        for kind in PlaceKind::ALL {
            let mut a = pool(7, kind);
            let mut all = all_names(kind);
            a.sort();
            all.sort();
            assert_eq!(a, all, "{}", kind.name());
        }
        assert_ne!(pool(7, PlaceKind::Hamlet), pool(8, PlaceKind::Hamlet));
        assert_eq!(pool(7, PlaceKind::Hamlet), pool(7, PlaceKind::Hamlet));
    }

    #[test]
    fn a_story_name_is_the_same_by_either_road() {
        let cat = jane_data::catalog();
        let names = Names::new(2026);
        for s in cat.county.stories {
            assert_eq!(story_name(2026, s.id), names.story(s.id), "{}", s.key);
            assert!(!names.story(s.id).is_empty(), "{}", s.key);
        }
    }

    #[test]
    fn a_board_says_the_name_in_capitals() {
        let t = board_text(PlaceKind::Farmstead, "Hollins Farm", 3);
        assert!(t.contains("HOLLINS FARM"), "{t}");
    }
}
