//! `jane sweep` and the L4 table (VERIFICATION.md §3.2): every model on every seed, traced;
//! their experience metrics, the L5 audits, and a report the owner can read.

use std::fmt::Write as _;

use jane_bot::experience::{Experience, FIRST_HOUR, MINUTE, first_hour_value, in_band};

/// A unit row's id, or `?`.
pub fn unit_name(d: Option<u16>) -> &'static str {
    d.map_or("?", |d| jane_data::catalog().combat.unit(jane_core::UnitDefId(d)).id)
}

pub fn quest_name(q: u16) -> &'static str {
    jane_data::catalog().story.quest(jane_core::QuestId(q)).id
}

pub const REGIONS: [&str; 3] = ["Lowfields", "Waters", "Works"];

/// The L4 table for one experience, as text.
pub fn l4_table(x: &Experience) -> String {
    let m = Experience::min;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "L4 {} seed {} seat {}: {} min played, {} quests done, {} deaths ({}.{} an hour)",
        x.model,
        x.seed,
        x.seat,
        m(x.frames),
        x.quests.iter().filter(|q| q.done.is_some()).count(),
        x.deaths.len(),
        x.deaths_per_hour_tenths() / 10,
        x.deaths_per_hour_tenths() % 10
    );
    let _ = writeln!(
        s,
        "  no objective {} min, looking {} min (first hour {} + {}); walked {} cells, backtracked {} ({}%); walk:play {}.{:02}; night unlit, first night {} min",
        m(x.idle_by_hour.iter().sum()),
        m(x.search_by_hour.iter().sum()),
        m(x.idle_in(60)),
        m(x.searching_in(60)),
        x.walked_cells,
        x.backtrack_cells,
        x.backtrack_cells * 100 / x.walked_cells.max(1),
        x.walk_to_play() / 100,
        x.walk_to_play() % 100,
        m(x.night_unlit_first * 60)
    );
    let long: Vec<_> = x.stretches_over(60).collect();
    let _ = writeln!(
        s,
        "  empty walks (60 s or more, nothing new in view): {}; over 2 min {}; longest {} s; bare (nothing in view) {}",
        long.len(),
        x.stretches_over(120).count(),
        long.iter().map(|s| s.secs).max().unwrap_or(0),
        long.iter().filter(|s| s.bare).count()
    );
    for b in FIRST_HOUR.iter().filter(|b| b.models.contains(&x.model.as_str())) {
        let v = first_hour_value(x, b);
        let _ = writeln!(
            s,
            "  §4.1 {:<48} {:>8} min  band {}..{}  {}",
            b.claim,
            v.map_or("never".into(), m),
            b.lo.map_or(String::new(), m),
            b.hi.map_or(String::new(), m),
            if in_band(b, v) { "ok" } else { "OUT" }
        );
    }
    let _ = MINUTE;
    s
}
