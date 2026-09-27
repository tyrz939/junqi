//! The loading screen's lines (PRESENTATION.md §3.2): one row per stage the build reports
//! (`jane_world::build_stages`), three ways of saying it, one picked by the seed. A stage with no
//! row still moves the lantern; it only says nothing.
//!
//! Written against `VOICE.md`: plain, short, specific, a little sad. They say what is being made
//! in general and never where: no place she has not met by name, no person, no direction, no
//! number that is a clue, nothing a later act reveals. No "Jane" (her name is not hers to give
//! here) and no en or em dashes (the tests hold both).

use jane_core::hash::Fnv;

/// Said as the train slows, when a New Game's county is built.
pub const CLOSING_NEW: &str = "closing_new";
/// Said when a loaded county is built.
pub const CLOSING_LOAD: &str = "closing_load";

/// `(stage, [three lines])`: the stage is the name the build reports.
pub const LINES: &[(&str, [&str; 3])] = &[
    (
        "skeleton",
        [
            "The hills are set down, one behind another.",
            "The county settles on a shape, and keeps it.",
            "Low ground is made, and high ground over it.",
        ],
    ),
    (
        "land",
        [
            "The river finds its bed.",
            "Water settles in the low fields, and stays.",
            "Grass comes up where the plough has not been.",
        ],
    ),
    (
        "edge",
        [
            "The trees are grown thick at the edge.",
            "A tree line is drawn round it all, and closed.",
            "The woods at the edge grow too thick to walk.",
        ],
    ),
    (
        "roads",
        [
            "The lanes are laid, and walked once.",
            "A road is worn where people have always gone.",
            "Milestones are set a mile apart, near enough.",
        ],
    ),
    (
        "paths",
        [
            "Hedges are set along the lanes.",
            "Footpaths are trodden across the fields.",
            "A stile is set in every hedge that needs one.",
        ],
    ),
    (
        "chunks",
        [
            "Houses are put up, and lived in a long time.",
            "A square is laid with cobbles, then with leaves.",
            "Doorsteps are scrubbed, and the milk set out.",
        ],
    ),
    (
        "rail",
        [
            "The rails are laid as far as the Halt.",
            "Sleepers are laid, and the rails over them.",
            "A signal is set at danger, then at clear.",
        ],
    ),
    (
        "doors",
        [
            "Doors are hung, and locked.",
            "Keys are cut, and hung on nails.",
            "A door is painted, and the number put back on.",
        ],
    ),
    (
        "road_furniture",
        [
            "The lamps are hung along the roads.",
            "A notice is nailed to a post, and left to weather.",
            "The lamps are trimmed and filled for tonight.",
        ],
    ),
    (
        "small_places",
        [
            "A well is dug, and a bucket left by it.",
            "A bench is set where the walk is long.",
            "A hen house is built, and the hens counted.",
        ],
    ),
    (
        "country",
        [
            "The sheep are brought in before dark.",
            "Washing is hung out, and taken in early.",
            "Cottage windows are lit, one and then another.",
        ],
    ),
    (
        "stories",
        [
            "A letter is written, and not posted.",
            "Things are said over fences, and not said again.",
            "A name is chalked on a door, and rubbed off.",
        ],
    ),
    (
        "scatter",
        [
            "Leaves come down on the verges.",
            "Stones turn up in the ploughed ground.",
            "Apples fall in an orchard nobody picks.",
        ],
    ),
    (
        "wildlife",
        [
            "Crows settle in the tall trees.",
            "The rats find the sheds.",
            "Something moves off the road, and waits for dark.",
        ],
    ),
    (
        "cut_through",
        [
            "Gaps are worn through the hedges.",
            "A short cut is found across a field.",
            "A gate is left open, and stays open.",
        ],
    ),
    (
        "solve",
        [
            "Someone walks it end to end, to be sure.",
            "Every gate is tried once.",
            "The way is walked, and it can be walked.",
        ],
    ),
    (
        "house",
        [
            "A note is left on a kitchen table.",
            "A kettle is filled, and left on the stove.",
            "Curtains are drawn, and a lamp left on.",
        ],
    ),
    (
        "cellar",
        [
            "Someone lights a stove in a cellar.",
            "Jars are labelled in a careful hand.",
            "A cellar step is swept, and the rest are not.",
        ],
    ),
    (
        "mine",
        [
            "A cage is wound down a shaft, and left at the bottom.",
            "Pit props are set, and lamps hung on them.",
            "A tally board is chalked at the pithead.",
        ],
    ),
    (
        "burial",
        [
            "Old stone is set over older stone.",
            "Flowers are left on a grave, fresh ones.",
            "A stair is cut, and covered over.",
        ],
    ),
    (
        "arms",
        [
            "Glasses are dried and hung over a bar.",
            "Chairs are put up on the tables in a public house.",
            "A fire is laid in a public house, to be lit at six.",
        ],
    ),
    (
        "church",
        [
            "Hymn numbers are slotted into a board.",
            "A candle is lit in a side chapel.",
            "The pews are dusted, the front one twice.",
        ],
    ),
    (
        "factory",
        [
            "A boiler is banked, and left ticking.",
            "A works whistle is tested, once.",
            "Soot settles on a yard, is swept, and settles.",
        ],
    ),
    (
        "forest",
        ["Coloured wings settle in the trees.", "A path is let grow over.", "A glasshouse fogs, and clears, and fogs."],
    ),
    (
        "library",
        [
            "Pages are left out in the rain.",
            "Books are shelved by someone who did not read them.",
            "A reading room is locked with the lamps on.",
        ],
    ),
    (
        "museum",
        [
            "Glass cases are dusted, and locked.",
            "Labels are typed for things nobody visits.",
            "A portrait is hung in a long room.",
        ],
    ),
    (
        "pipes",
        [
            "Water finds its way under the streets.",
            "A grate is bolted over a drain.",
            "Something drips in the dark, and keeps count.",
        ],
    ),
    (
        "school",
        ["A bell is hung in a tower, and tested once.", "Chalk is put back in its box.", "Desks are set out in rows."],
    ),
    (
        CLOSING_NEW,
        [
            "The train slows for Castle Halt.",
            "Five o'clock. The brakes go on.",
            "The train comes out of the tunnel, and slows.",
        ],
    ),
    (
        CLOSING_LOAD,
        ["The county is as she left it.", "It is all where she left it.", "Everything is put back where it was."],
    ),
];

/// The line `stage` says on `seed`, or `None` for a stage that says nothing.
pub fn line(stage: &str, seed: u32) -> Option<&'static str> {
    let (_, lines) = LINES.iter().find(|(k, _)| *k == stage)?;
    let h = Fnv::new().bytes(stage.as_bytes()).u32(seed).u32(0x4c4f_4144).mix();
    Some(lines[(h % lines.len() as u32) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_a_stage_the_build_reports_and_reads_plainly() {
        let stages = jane_world::build_stages();
        for (k, lines) in LINES {
            assert!(stages.contains(k) || *k == CLOSING_NEW || *k == CLOSING_LOAD, "{k} is never reported");
            for l in lines {
                assert!(!l.contains("Jane") && !l.contains('\u{2013}') && !l.contains('\u{2014}'), "{l}");
                assert!(l.ends_with('.') && l.len() <= 56, "{l}: one short sentence that fits the card");
                assert!(!l.contains("...") && !l.contains('!'), "{l}");
            }
        }
        // Every zone past the county says something as it is built.
        for z in jane_core::ZoneId::ALL.iter().skip(1) {
            assert!(line(z.name(), 1).is_some(), "{}", z.name());
        }
    }

    #[test]
    fn seeds_pick_different_lines() {
        let picks: std::collections::BTreeSet<&str> = (0..40).filter_map(|s| line("land", s)).collect();
        assert_eq!(picks.len(), 3);
        assert_eq!(line("land", 7), line("land", 7));
        assert_eq!(line("areas", 7), None);
    }
}
