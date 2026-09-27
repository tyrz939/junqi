//! The console's part of a story test (like [`crate::crawl::setup`] for a dungeon): a new game
//! put where a player who had played the spine as far as an act would stand, so the acts after
//! it can be played and looked at without playing the ones before. It writes the state
//! directly (quests done, the dog's offers made), which no tape records: a run started here is
//! for a person looking, never a fixture.
//!
//! What a player carries at each act is what the story gave her by then (`STORY.md` §4): each
//! dungeon's key to the next place, each verb, the growth the places before it offer
//! (the dungeons' jars and pages, not the county's errands: the story bot does few of those),
//! and what she packs at the bench before an act (`story.rs` provisions): six apples, a
//! life-steal, a Manashield, no Stone Skin. It is the story's kit, leaner than a dungeon test's
//! ([`crate::crawl::setup`]), so a dungeon that finishes from here finishes in the story.

use jane_core::ZoneId;
use jane_sim::input::DevOp;
use jane_sim::state::FlagKey;
use jane_sim::{Command, Sim};

use crate::sense::{item, spell};

/// One act of the spine: the quest the dog gives for it, the quests done before it, the offers
/// made, what she carries into it (keys and the like), and the verbs she knows.
struct Act {
    name: &'static str,
    /// The dungeon whose growth she has found everything before.
    zone: ZoneId,
    done: &'static [&'static str],
    offered: &'static [&'static str],
    items: &'static [(&'static str, u16)],
    verbs: &'static [&'static str],
}

const FIRST: [&str; 5] = ["the_letter", "defeat_skeleton", "see_the_kitchen", "stock_the_bench", "rats_below"];

const ACTS: [Act; 7] = [
    Act { name: "mine", zone: ZoneId::Mine, done: &[], offered: &["offered_rats"], items: &[], verbs: &["icebolt"] },
    Act {
        name: "museum",
        zone: ZoneId::Museum,
        done: &["the_mine"],
        offered: &["offered_rats", "offered_mine"],
        items: &[("key_museum", 1)],
        verbs: &["icebolt", "repair"],
    },
    Act {
        name: "forest",
        zone: ZoneId::Library,
        done: &["the_mine", "the_museum"],
        offered: &["offered_rats", "offered_mine", "offered_museum"],
        items: &[("key_forest", 1)],
        verbs: &["icebolt", "repair", "explosion"],
    },
    Act {
        name: "factory",
        zone: ZoneId::Pipes,
        done: &["the_mine", "the_museum", "the_forest"],
        offered: &["offered_rats", "offered_mine", "offered_museum", "offered_forest"],
        items: &[("key_works", 1)],
        verbs: &["icebolt", "repair", "explosion", "grow"],
    },
    Act {
        name: "burial",
        zone: ZoneId::Burial,
        done: &["the_mine", "the_museum", "the_forest", "the_factory"],
        offered: &["offered_rats", "offered_mine", "offered_museum", "offered_forest", "offered_factory"],
        items: &[("key_stone", 1)],
        verbs: &["icebolt", "repair", "explosion", "grow", "spark"],
    },
    Act {
        name: "school",
        zone: ZoneId::School,
        done: &["the_mine", "the_museum", "the_forest", "the_factory", "the_burial"],
        offered: &[
            "offered_rats",
            "offered_mine",
            "offered_museum",
            "offered_forest",
            "offered_factory",
            "offered_burial",
        ],
        items: &[("the_ball", 1)],
        verbs: &["icebolt", "repair", "explosion", "grow", "spark", "fireball"],
    },
    Act {
        name: "choice",
        zone: ZoneId::School,
        done: &["the_mine", "the_museum", "the_forest", "the_factory", "the_burial", "the_school"],
        offered: &[
            "offered_rats",
            "offered_mine",
            "offered_museum",
            "offered_forest",
            "offered_factory",
            "offered_burial",
            "offered_school",
        ],
        items: &[("the_ball", 1), ("key_stair", 1)],
        verbs: &["icebolt", "repair", "explosion", "grow", "spark", "fireball"],
    },
];

/// The acts [`start_at`] knows, in the story's order.
pub fn acts() -> impl Iterator<Item = &'static str> {
    ACTS.iter().map(|a| a.name)
}

/// Put a new game at the start of `act` (the dog about to offer it, on Julie's step, at ten in
/// the morning of the first day): the state written now, and the console commands to send
/// before playing (one a frame, [`crate::Bot::setup`]). `act+` is the act played and not yet
/// taken back: its quest in the log with every step done, and what she found in it in her bag
/// (the next act's keys and verb), for the walk back to the dog.
pub fn start_at(sim: &mut Sim, act: &str) -> Result<Vec<Command>, String> {
    let cat = jane_data::catalog();
    let unknown = || format!("no act {act}: one of {} (or one with + after it)", acts().collect::<Vec<_>>().join(", "));
    let (name, played) = act.strip_suffix('+').map_or((act, false), |n| (n, true));
    let i = ACTS.iter().position(|a| a.name == name).ok_or_else(unknown)?;
    let a = if played { ACTS.get(i + 1).ok_or_else(unknown)? } else { &ACTS[i] };
    let open = if played { Some(format!("the_{name}")) } else { None };
    for q in FIRST.iter().chain(a.done).filter(|q| open.as_deref() != Some(**q)) {
        let id = cat.story.quest_id(q).ok_or_else(|| format!("no quest {q}"))?;
        let st = sim.state_mut();
        st.quests.active.retain(|p| p.quest != id);
        if !st.quests.done.contains(&id) {
            st.quests.done.push(id);
        }
    }
    for f in a.offered {
        let n = cat.name_id(f).ok_or_else(|| format!("no name {f}"))?;
        sim.state_mut().flags.insert(FlagKey::Named(jane_sim::sym::of_name(n)), 1);
    }
    if let Some(q) = open {
        let id = cat.story.quest_id(&q).ok_or_else(|| format!("no quest {q}"))?;
        let reqs = cat.story.quest(id).requirements;
        // A place step reads the world's flag for the place (`quests.rs`); the rest count.
        for r in reqs {
            if let jane_data::ReqTarget::Location(n) = r.target {
                sim.state_mut().flags.insert(FlagKey::Been(jane_sim::sym::of_name(n)), 1);
            }
        }
        let counts = reqs.iter().map(|r| r.qty).collect();
        sim.state_mut().quests.active.push(jane_sim::state::QuestProgress { quest: id, counts });
    }
    let mut out = vec![Command::Dev(DevOp::Time { hour: 10 })];
    let (strength, spirit) = dungeon_growth_before(sim.blueprints(), a.zone);
    for (stat, amount) in [(jane_core::action::Stat::Strength, strength), (jane_core::action::Stat::Spirit, spirit)] {
        if amount > 0 {
            out.push(Command::Dev(DevOp::Grow { stat, amount: amount.min(i32::from(i16::MAX)) as i16 }));
        }
    }
    for v in a.verbs {
        out.push(Command::Dev(DevOp::Learn(spell(v))));
    }
    // Every act's keys up to this one: the story's keys are bound, and she keeps them.
    let upto = ACTS.iter().position(|x| std::ptr::eq(x, a)).unwrap_or(0);
    let mut items: Vec<(&str, u16)> = Vec::new();
    for x in &ACTS[..=upto] {
        for &(i, q) in x.items {
            if !items.iter().any(|&(j, _)| j == i) {
                items.push((i, q));
            }
        }
    }
    // Under the House (`rats_below`, written done above) is handed in holding the three pieces of
    // meat ("It wants you to keep them"), and pays two Savage Snakeroot and two water: the dog's
    // bait for the Burial's small snakes, brewed at the bench before Under the Stone.
    let packed = [
        ("key_auntie_house", 1),
        ("apple", 6),
        ("potion_lifesteal", 1),
        ("potion_manashield", 1),
        ("rat_meat", 3),
        ("savage_snakeroot", 2),
        ("small_water", 2),
    ];
    for &(i, qty) in items.iter().chain(&packed) {
        out.push(Command::Dev(DevOp::Give { item: item(i), qty }));
    }
    for (i, qty) in crate::crawl::materials_before(sim.blueprints(), a.zone) {
        out.push(Command::Dev(DevOp::Give { item: i, qty }));
    }
    // The School is come to up the stair behind Goldskin, which a console cannot open without
    // playing the Burial: the act starts where the stair comes up, with The Bell at Nine given
    // and Julie's Other Key in her bag, as the dog would have given them.
    if name == "school" && !played {
        sim.state_mut().flags.insert(
            FlagKey::Named(jane_sim::sym::of_name(cat.name_id("offered_school").ok_or("no name offered_school")?)),
            1,
        );
        out.push(Command::Dev(DevOp::Give { item: item("key_stair"), qty: 1 }));
        let q = cat.story.quest_id("the_school").ok_or("no quest the_school")?;
        out.push(Command::Dev(DevOp::Quest(q)));
        if let Some(n) = cat.name_id("entry") {
            out.push(Command::Dev(DevOp::Tp { zone: ZoneId::School, mark: jane_sim::sym::of_name(n) }));
            return Ok(out);
        }
    }
    let step = cat.name_id("dogs_step").ok_or("no mark dogs_step")?;
    out.push(Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: jane_sim::sym::of_name(step) }));
    Ok(out)
}

/// The growth the dungeons before `z` (in [`crate::crawl::ORDER`]) hold, and nothing else.
fn dungeon_growth_before(bps: &jane_sim::Blueprints, z: ZoneId) -> crate::crawl::Growth {
    let at = crate::crawl::ORDER.iter().position(|&o| o == z).unwrap_or(0);
    crate::crawl::ORDER[..at].iter().fold((0, 0), |(s, p), &d| {
        let (ds, dp) = crate::crawl::growth_in(bps.get(d));
        (s + ds, p + dp)
    })
}
