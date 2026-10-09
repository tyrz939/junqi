//! The night's stage (NIGHT.md §3): how deep the county's second world is tonight, latched at
//! the turn and kept until the dawn turn.
//!
//! - **Latched at the bell** ([`step`], from the clock's step 2): at the bell's first stroke, or
//!   at 21:00 where no bell rings (after `bell_stopped` the night turns in silence, NIGHT.md
//!   §2.2), the stage is computed from the world's flags ([`stage_from_flags`]) and written with
//!   the tick of the turn. On a Tuesday under `omen:early_bell` the bell rings at 20:50 and the
//!   turn comes with it: the stage is latched then, for the look and the words, while the sim's
//!   own night (`GameState::is_night`, doors, spawns) keeps 21:00 as built.
//! - **No change mid-night.** A boss killed at 23:00 deepens tomorrow's night, not tonight's.
//! - **Cleared at the dawn turn** (06:00): stage 0, `turned_at` the dawn's tick, so the
//!   presentation can run the reverse turn from it.
//!
//! The night is the party's, the world's state, never a seat's (§2.5); it is saved and hashed
//! with the rest (`save::Form`). What is derived from it is never saved: which lamps are wrong
//! (`light::warm`), the intensity by place ([`NightMap`]), the turn's progress
//! (`tick - turned_at`).

use jane_core::{Blueprint, Key, Rect, Tick, ZoneId};
use serde::{Deserialize, Serialize};

use crate::state::{FlagKey, GameState};
use crate::tuning::{NIGHT_END_HOUR, NIGHT_START_HOUR, TICKS_PER_DAY, TICKS_PER_HOUR};

/// The night as latched (NIGHT.md §3.2): one field, saved and hashed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Night {
    /// 0 by day (and at night after an ending that keeps no night world); else 1 to 4, N1 to N4.
    pub stage: u8,
    /// The tick of the last turn: the night's (its bell's first stroke, or 21:00), or by day
    /// the dawn's (06:00). `Tick(0)` at New Game.
    pub turned_at: Tick,
}

/// The deepest stage, N4: the Ball is up.
pub const STAGE_MAX: u8 = 4;

/// The early bell's mark, 20:50 (`data/clock/omens.json`).
pub const EARLY_BELL: u32 = 20 * TICKS_PER_HOUR + 50 * TICKS_PER_HOUR / 60;
const NIGHT_FROM: u32 = NIGHT_START_HOUR * TICKS_PER_HOUR;
const DAWN: u32 = NIGHT_END_HOUR * TICKS_PER_HOUR;

fn flag(s: &GameState, name: &str) -> i32 {
    s.syms.find(name).and_then(|k| s.flags.get(&FlagKey::Named(k)).copied()).unwrap_or(0)
}

/// Has the spine step `name` happened: its consequence fired (`data/consequences.json`: the
/// mine's at Iron Knuckles' fall, the Works' at the Foreman's, the Burial's at Goldskin's), or a
/// world flag of the name set (`mine_quiet` sets one too)?
fn spine(s: &GameState, name: &str) -> bool {
    let cat = jane_data::catalog();
    flag(s, name) != 0 || cat.living.consequence_id(name).is_some_and(|c| s.consequences_done.get(u32::from(c.0)))
}

/// The stage the world's flags give now (NIGHT.md §3.1, §3.2): N1 from New Game, one deeper for
/// each of `mine_quiet`, `works_dark` and `burial_quiet` (each a consequence the spine fires);
/// N4 once `bell_stopped`; then the ending's rule (§3.1, `the_end`): the shield held (1) keeps
/// no night world, the hill (2, or `night_gone`) no night at all, and the train (3) leaves it at
/// N4.
pub fn stage_from_flags(s: &GameState) -> u8 {
    if flag(s, "night_gone") != 0 {
        return 0;
    }
    match flag(s, "the_end") {
        1 | 2 => return 0,
        3 => return STAGE_MAX,
        _ => {}
    }
    if flag(s, "bell_stopped") != 0 {
        return STAGE_MAX;
    }
    let on = |n: &str| u8::from(spine(s, n));
    1 + on("mine_quiet") + on("works_dark") + on("burial_quiet")
}

/// Does the early bell ring tonight, at 20:50? The omen's clock row's own conditions
/// (`data/clock/omens.json`): `omen:early_bell`, a Tuesday, and a bell that still rings.
pub fn early_bell(s: &GameState) -> bool {
    flag(s, "omen:early_bell") != 0 && s.weekday() == 2 && flag(s, "bell_stopped") == 0
}

/// Keep the night with the clock (step 2, after the clock moves and before its rows run, so a
/// row at the bell reads tonight's stage). Latches at the turn, clears at the dawn turn, and
/// otherwise leaves the night alone. A clock set by hand (the console's `time`) or a save from
/// before the night was kept lands in the night as it is, its turn already past: `turned_at` is
/// the tick the clock last stood at the turn.
pub fn step(s: &mut GameState) {
    let (clock, now) = (s.clock, s.tick.0);
    // The tick the clock last stood at `mark`.
    let since = |mark: u32| Tick(now.saturating_sub((clock + TICKS_PER_DAY - mark) % TICKS_PER_DAY));
    let night = clock >= NIGHT_FROM || clock < DAWN;
    let early = !night && clock >= EARLY_BELL;
    if night || (early && early_bell(s)) {
        if s.night.stage == 0 {
            let stage = stage_from_flags(s);
            if stage > 0 {
                let mark = if night { NIGHT_FROM } else { EARLY_BELL };
                s.night = Night { stage, turned_at: since(mark) };
            }
        }
    } else if s.night.stage != 0 && !early {
        s.night = Night { stage: 0, turned_at: since(DAWN) };
    }
}

/// The night's intensity by place (NIGHT.md §3.2), for the look and the sound: the stage moved
/// by where a cell is, so the gradient across the map reads as the shield's reach. A pure
/// function of the blueprint and the stage, so every seat and every tier draws the same.
///
/// - Julie's house and yard (the `site_julie_house` ground): always 0; so are her house and
///   cellar, and every dungeon ("a dungeon's dark is its own", §2.4).
/// - A hub inside its fence (the ground of Castle, the canteen, the Reedcutters' fire, the
///   Halt; and the Arms and the church, which are Castle's): the stage less one.
/// - The Lowfields and the Waters: the stage. The Works: the stage plus one.
///
/// All held to 0 to 4. The first walk (the station road and on to Castle) is 0 by §3.2, but the
/// blueprint does not keep its cells yet: it reads as the ground under it until the shield's
/// WORLDGEN lands (NIGHT.md §9 R4).
#[derive(Clone, Debug)]
pub struct NightMap<'a> {
    bp: &'a Blueprint,
    julie: Option<Rect>,
    hubs: [Option<Rect>; 4],
}

/// The hubs' grounds, by their site rows (`data/sites.json`).
const HUBS: [&str; 4] = ["site_town", "site_canteen", "site_reed_camp", "site_station"];

impl<'a> NightMap<'a> {
    pub fn of(bp: &'a Blueprint) -> Self {
        let julie = rect_named(bp, "site_julie_house");
        let hubs = HUBS.map(|n| rect_named(bp, n));
        Self { bp, julie, hubs }
    }

    /// The intensity at cell `(x, y)` of the blueprint's zone, at night stage `stage`.
    pub fn intensity(&self, stage: u8, x: i32, y: i32) -> u8 {
        if stage == 0 {
            return 0;
        }
        let hub = stage - 1;
        match self.bp.zone {
            ZoneId::County => {}
            ZoneId::Arms | ZoneId::Church => return hub,
            _ => return 0,
        }
        if self.julie.is_some_and(|r| r.contains(x, y)) {
            return 0;
        }
        if self.hubs.iter().flatten().any(|r| r.contains(x, y)) {
            return hub;
        }
        // Region order is `jane_data::Region`'s: Lowfields, Waters, Works.
        match self.bp.regions.region_at(x, y) {
            Some(2) => (stage + 1).min(STAGE_MAX),
            _ => stage.min(STAGE_MAX),
        }
    }
}

/// A rect of the blueprint by name: a generated name (`site_<id>`) or a content one.
fn rect_named(bp: &Blueprint, name: &str) -> Option<Rect> {
    let cat = jane_data::catalog();
    bp.rects.iter().find_map(|(k, r)| {
        let is = match *k {
            Key::Local(i) => bp.local_names.get(i as usize).is_some_and(|n| n == name),
            Key::Name(n) => cat.name(n) == name,
        };
        is.then_some(*r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marks_are_ten_minute_marks() {
        // A bed's night works the clock a mark at a time (`living::Sim::sleep_to`): the turn and
        // the dawn turn must fall on marks, or a night slept would latch where a night idled did not.
        let every = crate::tuning::ECOLOGY_EVERY;
        assert_eq!(EARLY_BELL % every, 0);
        assert_eq!(NIGHT_FROM % every, 0);
        assert_eq!(DAWN % every, 0);
    }
}
