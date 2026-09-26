//! Omens (PLAN.md §5, WORLD.md §7.3): claims the county makes that are true on some seeds.
//!
//! **Rolled once, at New Game, from the world stream.** Before anything else draws from
//! `GameState.rng` (ARCHITECTURE.md §4.4: the stream's order is fixed, and the ten-minute marks
//! come after), one `below(1000)` per row of `data/omens.json`, in row order, whatever the row
//! says; a row is true when its roll is under its `chance`. A lethal row rolled true is false when
//! an earlier lethal row of its region is already true (never two stacked). A true row sets its
//! flag (`omen:<id>`) to 1, and the triggers, clock rows and lists that read the flag make the
//! county do what the claim says. A false row sets nothing.
//!
//! Nothing says which are true: no event, no toast, no journal entry. The flags are the world's,
//! saved and hashed like any other.

use jane_core::Sfc32;
use jane_data::OmenDef;

use crate::state::{FlagKey, GameState};

/// Which of `rows` come true, by index, drawing one roll a row from `rng` in row order.
pub fn roll(rows: &[OmenDef], rng: &mut Sfc32) -> Vec<usize> {
    let mut lethal_true: Vec<jane_data::Region> = Vec::new();
    let mut out = Vec::new();
    for (i, o) in rows.iter().enumerate() {
        let r = rng.below(1000);
        if r >= u32::from(o.chance) {
            continue;
        }
        if o.lethal {
            if lethal_true.contains(&o.region) {
                continue;
            }
            lethal_true.push(o.region);
        }
        out.push(i);
    }
    out
}

/// Roll every omen of the catalog into `state`'s flags, from its world stream.
pub fn roll_omens(state: &mut GameState) {
    let rows = jane_data::catalog().story.omens;
    for i in roll(rows, &mut state.rng) {
        state.flags.insert(FlagKey::Named(crate::sym::of_name(rows[i].flag)), 1);
    }
}

/// Is this omen true in this world? For tools, tests and the dossier (VERIFICATION.md §3), never
/// for a line the game shows.
pub fn is_true(state: &GameState, id: &str) -> bool {
    let cat = jane_data::catalog();
    cat.story
        .omens
        .iter()
        .find(|o| o.id == id)
        .is_some_and(|o| state.flags.get(&FlagKey::Named(crate::sym::of_name(o.flag))).is_some_and(|&v| v != 0))
}

#[cfg(test)]
mod tests {
    use jane_data::Region;

    use super::*;

    /// About a third of seeds each, independently (PLAN.md §5), from the stream's first draws.
    #[test]
    fn each_omen_is_true_on_about_a_third_of_seeds() {
        let rows = jane_data::catalog().story.omens;
        assert!(rows.len() >= 3, "the omens the text already claims");
        let mut hits = vec![0u32; rows.len()];
        let n = 3000;
        for seed in 0..n {
            for i in roll(rows, &mut Sfc32::seeded(seed, 1)) {
                hits[i] += 1;
            }
        }
        for (o, h) in rows.iter().zip(&hits) {
            let want = u32::from(o.chance) * n / 1000;
            assert!(h.abs_diff(want) < n / 30, "{}: {h} of {n}, wanted about {want}", o.id);
        }
    }

    /// One roll a row, true or not, so a row added later moves nothing before it; never two
    /// lethal omens of a region at once.
    #[test]
    fn a_roll_a_row_and_never_two_lethal_ones_stacked() {
        let base = jane_data::catalog().story.omens[0];
        let row = |lethal: bool, region: Region| OmenDef { chance: 1000, lethal, region, ..base };
        let rows = [
            row(true, Region::Lowfields),
            row(true, Region::Lowfields),
            row(true, Region::Waters),
            row(false, Region::Lowfields),
        ];
        let mut rng = Sfc32::seeded(9, 1);
        assert_eq!(roll(&rows, &mut rng), [0, 2, 3]);
        let mut fresh = Sfc32::seeded(9, 1);
        for _ in 0..rows.len() {
            fresh.next_u32();
        }
        assert_eq!(rng, fresh, "one draw a row");
    }
}
