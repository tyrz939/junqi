//! What a dungeon asks beyond the crawl's general order (`crawl.rs`): each boss's pattern and each
//! dungeon's one idea, played the way the room tells a player to play it (DUNGEONS.md §3). A
//! tactic reads only what the crawl reads (the view, and the catalog's rows for what a prop or a
//! unit does); it is called from the crawl's choice and from the fight, and says nothing when it
//! has nothing to add.
//!
//! | Module | Dungeon |
//! | --- | --- |
//! | [`mine`] | the Gold Mine: the hoists over Iron Knuckles |
//! | [`museum`] | the Museum: the breaker, the exhibits, the Attendant |
//! | [`forest`] | the library and Butterfly Forest: buds in the light, the Emperor |
//! | [`works`] | the pipes and the Factory: fuses, sockets, the Foreman |
//! | [`burial`] | the Burial Chamber: the four corners, the flames, Goldskin |
//! | [`school`] | the School: the timetable, the lessons, the Timekeeper |

pub mod burial;
pub mod forest;
pub mod mine;
pub mod museum;
pub mod school;
pub mod works;
