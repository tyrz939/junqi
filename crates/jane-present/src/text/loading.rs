//! The loading screen's lines (PRESENTATION.md §3.2): one row per stage the build reports
//! (`jane_world::build_stages`), three ways of saying it, one picked by the seed. A stage with no
//! row still moves the lantern; it only says nothing.
//!
//! Written against `VOICE.md`: the county being got ready by the people who keep it, in plain
//! words: the surveyor, the carter, the lampman, the sweeper, the constable, the caretaker. Each
//! line is something a person does that she could later see done. They say what is being made
//! in general and never where: no place she has not met by name, nobody by their own name (a
//! trade, not a person), no direction, no number that is a clue, nothing a later act reveals. No
//! "Jane" (her name is not hers to give here) and no en or em dashes (the tests hold both).

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
            "A surveyor chains the hills and writes them down.",
            "The map is drawn. It is right, as of today.",
            "The School's hill goes on the map first.",
        ],
    ),
    (
        "land",
        [
            "The flood boards go up by the river.",
            "The low fields flood, as they do every spring.",
            "The farmer walks his grass and finds it long.",
        ],
    ),
    (
        "edge",
        [
            "Nobody cuts the woods at the county's edge.",
            "The woodcutter keeps to this side of the edge woods.",
            "A tree line goes all the way round, with no gap in it.",
        ],
    ),
    (
        "roads",
        [
            "The roadmen rake the lanes and fill the holes.",
            "Milestones go in, a mile apart or near it.",
            "The carter leaves his cart on the station road.",
        ],
    ),
    (
        "paths",
        [
            "Hedges are laid along the lanes and staked.",
            "The council paints the footpath numbers on posts.",
            "A stile goes in wherever a footpath meets a hedge.",
        ],
    ),
    (
        "chunks",
        [
            "The town is built and moved into, house by house.",
            "The sweeper does the square. Leaves now, salt later.",
            "The milkman leaves the bottles on the steps.",
        ],
    ),
    (
        "rail",
        [
            "The gang lays the line as far as the Halt.",
            "The signalman sets the signal to danger, then clear.",
            "The gang beds the sleepers and bolts the rails down.",
        ],
    ),
    (
        "doors",
        [
            "Keys are cut and hung on nails by the doors.",
            "Doors are hung, and most of them are locked.",
            "A door is painted and its number screwed back on.",
        ],
    ),
    (
        "road_furniture",
        [
            "The lampman fills the lamps along the roads.",
            "The council nails a notice to a post.",
            "The lampman trims the wicks. Lamps on at half six.",
        ],
    ),
    (
        "small_places",
        [
            "A well is dug and a bucket left on the chain.",
            "A bench is put where the walk is longest.",
            "The hens are counted in at dusk, and counted again.",
        ],
    ),
    (
        "country",
        [
            "The sheep are brought in before the lamps.",
            "Washing goes out on the lines, and comes in early.",
            "The farms light their kitchens one by one.",
        ],
    ),
    (
        "stories",
        [
            "A letter is written, and not posted.",
            "Neighbours talk over the fence, and stop when you pass.",
            "A spare key goes next door, in case.",
        ],
    ),
    (
        "perimeters",
        [
            "The farmer lays a hedge and leaves a gap for the cows.",
            "A wall goes round the field, one stone on two.",
            "The reedcutters cut a drain and bank the spoil.",
        ],
    ),
    (
        "scatter",
        [
            "Leaves come down on the verges.",
            "The plough turns up stones, and the farmer piles them.",
            "Apples fall in an orchard nobody picks.",
        ],
    ),
    (
        "wildlife",
        [
            "Crows settle in the tall trees.",
            "Rats get into the sheds again.",
            "Foxes and worse keep off the road until dark.",
        ],
    ),
    (
        "cut_through",
        [
            "Children wear a gap through a hedge.",
            "Somebody finds a short cut across a field.",
            "A gate gets left open, and the cows find it.",
        ],
    ),
    (
        "solve",
        [
            "The postman walks the whole round, to be sure of it.",
            "The constable tries every gate on his beat.",
            "The surveyor checks there is a way to everywhere.",
        ],
    ),
    (
        "house",
        [
            "A note is written at a kitchen table, and started again.",
            "A kettle is filled for two and left on the stove.",
            "A lamp is left on behind drawn curtains.",
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
            "A cage is wound down the shaft and left at the bottom.",
            "The pitmen set props and hang their lamps on them.",
            "The banksman chalks the tally board and counts again.",
        ],
    ),
    (
        "burial",
        [
            "The sexton cuts the grass between the graves.",
            "Fresh flowers go on a grave, as every week.",
            "The mason recuts a name that had worn faint.",
        ],
    ),
    (
        "arms",
        [
            "The landlady dries the glasses and hangs them up.",
            "The chairs go up on the tables in the public bar.",
            "The fire in the bar is laid, to be lit at six.",
        ],
    ),
    (
        "church",
        [
            "The vicar puts up the hymn numbers for evensong.",
            "A candle is lit in the side chapel.",
            "The pews are dusted, the front one twice.",
        ],
    ),
    (
        "factory",
        [
            "The stoker banks the boiler for the night.",
            "The time clerk racks the clocking cards, all but one.",
            "Soot comes down on the yard. Somebody sweeps it.",
        ],
    ),
    (
        "forest",
        [
            "Coloured wings settle in the trees.",
            "The paths are left unclipped, and grow over.",
            "A glasshouse is left with its vents shut.",
        ],
    ),
    (
        "library",
        [
            "Rain comes in through the library roof.",
            "The librarian stamps the books out, and the dates after.",
            "A reading room is locked with the lamps on.",
        ],
    ),
    (
        "museum",
        [
            "The attendant dusts the cases and locks them.",
            "Labels are typed for the cases, in capitals.",
            "A portrait is hung in a long room.",
        ],
    ),
    (
        "pipes",
        [
            "Water finds its way under the streets.",
            "The council bolts a grate over the drain.",
            "A ganger chalks the water level on the brick.",
        ],
    ),
    (
        "school",
        ["The caretaker hangs a bell in the tower.", "Chalk is put back in its box.", "Desks are set out in rows."],
    ),
    (
        CLOSING_NEW,
        [
            "The train slows for Castle Halt.",
            "One o'clock. The brakes go on.",
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

    /// The train she comes in on is the hour New Game starts (the owner's playtest, 2026-10-07:
    /// one o'clock, so the first day has light in it), and the timetable at the Halt says so.
    #[test]
    fn the_train_comes_in_at_the_hour_new_game_starts() {
        let hour = jane_sim::tuning::START_HOUR;
        let words = ["Twelve", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Eleven"];
        let said = format!("{} o'clock.", words[(hour % 12) as usize]);
        let (_, closing) = LINES.iter().find(|(k, _)| *k == CLOSING_NEW).expect("the closing lines");
        for l in closing.iter().filter(|l| l.contains("o'clock")) {
            assert!(l.starts_with(&said), "{l}: not {said}");
        }
        assert!(closing.iter().any(|l| l.starts_with(&said)), "a closing line names the hour");
        let cat = jane_data::catalog();
        let board = cat.story.dialogue(cat.story.dialogue_id("timetable").expect("the timetable"));
        let lines: Vec<&str> = board.nodes.iter().flat_map(|n| n.lines.iter().map(|l| cat.text(l.text))).collect();
        let arr = format!("arr. {hour}.00");
        assert!(lines.iter().any(|l| l.contains(&arr)), "{lines:?}: {arr}");
    }

    #[test]
    fn seeds_pick_different_lines() {
        let picks: alloc::collections::BTreeSet<&str> = (0..40).filter_map(|s| line("land", s)).collect();
        assert_eq!(picks.len(), 3);
        assert_eq!(line("areas", 7), None);
    }
}
