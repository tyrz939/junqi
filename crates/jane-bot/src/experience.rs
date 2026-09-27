//! Experience metrics from a trace (VERIFICATION.md §2 L4): what a session was like, in
//! numbers, for one seat. Every number here is computed from the `.jtr` alone (L4's rule), so
//! anyone with the trace can recompute it.
//!
//! | Metric | From |
//! | --- | --- |
//! | time to each goal | a quest's `Given`, `Ready` and `Done` events, in real minutes (frames) |
//! | time to the first sighting of each step's target | the step's quest given, to its `Sight` |
//! | time looking | the frames the model's objective was a search (the Lost) |
//! | time not knowing what to do | the samples with no objective, alive, not talking or fighting |
//! | backtracking | cells walked over ground walked in the previous ten minutes, not on the way to a hand-in |
//! | deaths by cause | `Died`: what last hurt her, the region, the hour of play |
//! | empty walks | runs of samples walking out of doors (not talking, not fighting) over ground not charted before, with nothing new coming on screen ([`EMPTY_SECS`] or more); *bare* when nothing at all was in view |
//! | night exposure | samples out of doors at night outside any light, the first night |
//! | walk to play | samples walking over samples fighting or talking |
//! | journal size | journal writes by kind |

use std::collections::BTreeMap;

use jane_core::ZoneId;
use jane_sim::trace::{Ev, Kind, Sample, Trace};

/// Frames a real minute.
pub const MINUTE: u32 = 60 * 60;

/// Seconds of walking new ground with nothing new in view that make an empty stretch: PLAN.md
/// §2.4's "something visible every 20 to 30 s" (VERIFICATION.md L4's nothing-to-see runs are 60 s
/// or more: [`Experience::stretches_over`] counts those).
pub const EMPTY_SECS: u32 = 30;

/// Walking seconds charting no new ground that end an empty stretch (she is on ground seen
/// before).
const STALE: u32 = 3;

/// Frames back that walked ground counts as walked again (L4: "the previous ten minutes").
const BACKTRACK_WINDOW: u32 = 10 * MINUTE;

/// Cells a side of the ground a backtrack is counted on.
const BACKTRACK_BLOCK: i32 = 8;

/// One quest's times, frames (`None`: not in the trace).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestTimes {
    pub quest: u16,
    pub given: Option<u32>,
    pub ready: Option<u32>,
    pub done: Option<u32>,
}

/// One step's (255: the hand-in) looking and first sight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepTimes {
    pub quest: u16,
    pub step: u8,
    /// Frame its quest was given (or the trace began, for one given before).
    pub given: u32,
    /// Frame its target was first on screen, where, and how far from her.
    pub sighted: Option<(u32, (i32, i32), u32)>,
    /// Frames the model's objective was to look for it.
    pub searched: u32,
    /// Frames its objective was to do it (walking to it, fighting for it).
    pub pursued: u32,
    /// The model gave up looking and was told.
    pub gave_up: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeathAt {
    pub frame: u32,
    pub tick: u32,
    pub by: Option<u16>,
    pub region: u8,
    pub zone: u8,
    pub cell: (i32, i32),
}

/// A run of walking with nothing new (or nothing at all) in view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stretch {
    pub frame: u32,
    pub tick: u32,
    pub secs: u32,
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub region: u8,
    /// Nothing at all in view the whole way (not only nothing new).
    pub bare: bool,
    /// The objective she was on.
    pub doing: String,
    /// 0: nothing new at all came on screen; 1: nothing new a person would remember the walk
    /// by (no landmark: see `trace::Sample::new_landmarks`).
    pub kind: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Experience {
    pub seed: u32,
    pub model: String,
    pub seat: u8,
    pub frames: u32,
    pub ticks: u32,
    pub quests: Vec<QuestTimes>,
    pub steps: Vec<StepTimes>,
    pub deaths: Vec<DeathAt>,
    pub stretches: Vec<Stretch>,
    /// Frames alive with no objective, by hour of play.
    pub idle_by_hour: Vec<u32>,
    /// Frames looking (a search objective), by hour of play.
    pub search_by_hour: Vec<u32>,
    pub walked_cells: u32,
    pub backtrack_cells: u32,
    pub walk_samples: u32,
    pub play_samples: u32,
    /// Samples out of doors at night outside any light, before the first morning.
    pub night_unlit_first: u32,
    /// Hit points lost before the letter was done (the first walk), and deaths then.
    pub hurt_first_walk: i32,
    pub deaths_first_walk: u32,
    /// Row of the first thing that hurt her, and the frame.
    pub first_hurt: Option<(u32, Option<u16>)>,
    pub first_rest: Option<(u32, u32)>,
    pub learned: Vec<(u32, u16)>,
    /// Journal writes by kind (`trace::JOURNAL_KINDS`).
    pub journal: [u32; 8],
    /// Things new to her, by hour of play.
    pub new_by_hour: Vec<u32>,
    /// Kills, loot and quest changes by hour of play (the pacing curve).
    pub events_by_hour: Vec<u32>,
}

fn hour_of(frame: u32) -> usize {
    (frame / (60 * MINUTE)) as usize
}

fn bump(v: &mut Vec<u32>, i: usize, n: u32) {
    if v.len() <= i {
        v.resize(i + 1, 0);
    }
    v[i] += n;
}

/// The experience of seat `seat` in `t`.
pub fn measure(t: &Trace, seat: u8) -> Experience {
    let mut x = Experience {
        seed: t.header.seed,
        model: t.header.model.clone(),
        seat,
        frames: t.footer.frames,
        ticks: t.footer.ticks,
        ..Experience::default()
    };
    let county = ZoneId::County.index() as u8;
    let cat = jane_data::catalog();
    let letter = cat.story.quest_id("the_letter").map(|q| q.0);
    let mut quests: BTreeMap<u16, QuestTimes> = BTreeMap::new();
    let mut steps: BTreeMap<(u16, u8), StepTimes> = BTreeMap::new();
    // The objective in hand and since when.
    let mut doing: (String, Option<u16>, Option<u8>, u32) = ("none".into(), None, None, 0);
    let mut last_sample: Option<Sample> = None;
    let mut runs: [Option<(Stretch, bool)>; 2] = [None, None];
    let mut ground: BTreeMap<(i32, i32), u32> = BTreeMap::new();
    let mut first_morning = false;
    let mut been_night = false;
    let mut stale = 0u32;
    let mut letter_done = false;
    let close = |doing: &(String, Option<u16>, Option<u8>, u32),
                 until: u32,
                 x: &mut Experience,
                 steps: &mut BTreeMap<(u16, u8), StepTimes>| {
        let (obj, q, i, since) = doing;
        let n = until.saturating_sub(*since);
        if n == 0 {
            return;
        }
        if let Some(q) = q {
            let step = i.unwrap_or(255);
            let e = steps.entry((*q, step)).or_insert_with(|| StepTimes { quest: *q, step, ..StepTimes::default() });
            if obj.starts_with("search") {
                e.searched += n;
                bump(&mut x.search_by_hour, hour_of(*since), n);
            } else {
                e.pursued += n;
            }
        }
    };
    for r in &t.records {
        if r.seat.is_some_and(|s| s != seat) {
            continue;
        }
        let f = r.frame;
        match &r.kind {
            Kind::Decision { objective, quest, step, .. } => {
                close(&doing, f, &mut x, &mut steps);
                doing = (objective.clone(), *quest, *step, f);
            }
            Kind::Note(line) => {
                if let Some(rest) = line.split("lost: gave up looking for ").nth(1) {
                    let name = rest.trim_end_matches(" (told where)");
                    if let Some((q, i)) = parse_step(name) {
                        steps
                            .entry((q, i))
                            .or_insert_with(|| StepTimes { quest: q, step: i, ..StepTimes::default() })
                            .gave_up = true;
                    }
                }
            }
            Kind::Sight { quest, step, cell, dist, .. } => {
                let e = steps.entry((*quest, *step)).or_insert_with(|| StepTimes {
                    quest: *quest,
                    step: *step,
                    ..StepTimes::default()
                });
                if e.sighted.is_none() {
                    e.sighted = Some((f, *cell, *dist));
                }
            }
            Kind::Journal { kind, .. } => x.journal[usize::from(*kind).min(7)] += 1,
            Kind::Event(ev) => match *ev {
                Ev::Quest { quest, change } => {
                    let e = quests.entry(quest).or_insert_with(|| QuestTimes { quest, ..QuestTimes::default() });
                    match change {
                        0 => {
                            e.given.get_or_insert(f);
                        }
                        2 => {
                            e.ready.get_or_insert(f);
                        }
                        3 => {
                            e.done.get_or_insert(f);
                            if Some(quest) == letter {
                                letter_done = true;
                            }
                        }
                        _ => {}
                    }
                    bump(&mut x.events_by_hour, hour_of(f), 1);
                }
                Ev::Hurt { amount, by } => {
                    if x.first_hurt.is_none() {
                        x.first_hurt = Some((f, by));
                    }
                    if !letter_done {
                        x.hurt_first_walk += amount;
                    }
                }
                Ev::Died { by, region } => {
                    let (zone, cell) = last_sample.map_or((county, (0, 0)), |s| (s.zone, s.cell));
                    x.deaths.push(DeathAt { frame: f, tick: r.tick, by, region, zone, cell });
                    if !letter_done {
                        x.deaths_first_walk += 1;
                    }
                }
                Ev::Rest => {
                    if x.first_rest.is_none() {
                        x.first_rest = Some((f, r.tick));
                    }
                }
                Ev::Learn { spell } => x.learned.push((f, spell)),
                Ev::Kill { .. } | Ev::Loot { .. } => bump(&mut x.events_by_hour, hour_of(f), 1),
                _ => {}
            },
            Kind::Sample(s) => {
                // Nothing to do: alive, not talking or fighting, and no objective.
                if doing.0 == "none" && s.alive && !s.talking && !s.fighting {
                    bump(&mut x.idle_by_hour, hour_of(f), jane_sim::trace::SAMPLE_EVERY);
                }
                bump(&mut x.new_by_hour, hour_of(f), u32::from(s.new_things));
                let walking = s.moving && !s.talking && !s.fighting && s.alive;
                if walking {
                    x.walk_samples += 1;
                } else if s.talking || s.fighting {
                    x.play_samples += 1;
                }
                if s.night {
                    been_night = true;
                } else if been_night {
                    first_morning = true;
                }
                if !first_morning && s.zone == county && s.night && !s.lit {
                    x.night_unlit_first += 1;
                }
                // Walked, and walked again.
                if let Some(p) = last_sample.filter(|p| p.zone == s.zone && s.moving) {
                    let d = (s.cell.0 - p.cell.0).abs().max((s.cell.1 - p.cell.1).abs()) as u32;
                    if d < 40 {
                        x.walked_cells += d;
                        let b = (
                            s.cell.0.div_euclid(BACKTRACK_BLOCK) + i32::from(s.zone) * 100_000,
                            s.cell.1.div_euclid(BACKTRACK_BLOCK),
                        );
                        let pb = (
                            p.cell.0.div_euclid(BACKTRACK_BLOCK) + i32::from(p.zone) * 100_000,
                            p.cell.1.div_euclid(BACKTRACK_BLOCK),
                        );
                        let handing_in = doing.0.starts_with("hand in");
                        if b != pb && !handing_in && ground.get(&b).is_some_and(|&at| at + BACKTRACK_WINDOW >= f) {
                            x.backtrack_cells += d;
                        }
                        ground.insert(b, f);
                    }
                }
                // Empty stretches: walking out of doors over ground the map had not charted, with
                // nothing new coming on screen (kind 0), or no new landmark (kind 1). Ground walked
                // before is not held to it (nothing is new there by definition): three walking
                // seconds charting nothing end a run, as a new thing or leaving the county does.
                // Standing (talking, fighting) neither ends nor lengthens one. Not for a pair: the
                // map is the party's, and one seat's charting is the other's new ground.
                if walking && s.zone == county {
                    stale = if s.new_ground > 0 { 0 } else { stale + 1 };
                }
                if t.header.seats == 1 {
                    let fresh = walking && s.zone == county && stale < STALE;
                    let over = s.zone != county || (walking && stale >= STALE);
                    for (k, news) in [(0u8, s.new_things), (1u8, s.new_landmarks)] {
                        let run = &mut runs[usize::from(k)];
                        if fresh && news == 0 {
                            match run {
                                Some((st, bare)) => {
                                    st.secs += 1;
                                    st.to = s.cell;
                                    *bare &= s.in_view.empty();
                                }
                                None => {
                                    *run = Some((
                                        Stretch {
                                            frame: f,
                                            tick: r.tick,
                                            secs: 1,
                                            from: s.cell,
                                            to: s.cell,
                                            region: s.region,
                                            bare: false,
                                            doing: doing.0.clone(),
                                            kind: k,
                                        },
                                        s.in_view.empty(),
                                    ));
                                }
                            }
                        } else if news > 0 || over {
                            if let Some((mut st, bare)) = run.take() {
                                if st.secs >= EMPTY_SECS {
                                    st.bare = bare;
                                    x.stretches.push(st);
                                }
                            }
                        }
                    }
                }
                last_sample = Some(*s);
            }
            _ => {}
        }
    }
    close(&doing, t.footer.frames, &mut x, &mut steps);
    for run in &mut runs {
        if let Some((mut st, bare)) = run.take() {
            if st.secs >= EMPTY_SECS {
                st.bare = bare;
                x.stretches.push(st);
            }
        }
    }
    // Each step's quest given.
    for s in steps.values_mut() {
        s.given = quests.get(&s.quest).and_then(|q| q.given).unwrap_or(0);
    }
    x.quests = quests.into_values().collect();
    x.steps = steps.into_values().collect();
    x
}

/// `the_mine step 2` or `the_mine hand-in` as (quest, step index or 255).
fn parse_step(s: &str) -> Option<(u16, u8)> {
    let cat = jane_data::catalog();
    let (id, rest) = s.split_once(' ')?;
    let q = cat.story.quest_id(id)?;
    if rest == "hand-in" {
        return Some((q.0, 255));
    }
    let n: u8 = rest.strip_prefix("step ")?.parse().ok()?;
    Some((q.0, n.checked_sub(1)?))
}

impl Experience {
    /// Real minutes, one decimal, from frames.
    pub fn min(frames: u32) -> String {
        let tenths = frames * 10 / MINUTE;
        format!("{}.{}", tenths / 10, tenths % 10)
    }

    /// The frame a quest was done, by id.
    pub fn done(&self, id: &str) -> Option<u32> {
        let q = jane_data::catalog().story.quest_id(id)?.0;
        self.quests.iter().find(|t| t.quest == q)?.done
    }

    /// The frame a spell was learned, by id.
    pub fn learned_at(&self, id: &str) -> Option<u32> {
        let s = jane_data::catalog().combat.spell_id(id)?.0;
        self.learned.iter().find(|&&(_, x)| x == s).map(|&(f, _)| f)
    }

    /// Deaths before `frame`.
    pub fn deaths_before(&self, frame: u32) -> usize {
        self.deaths.iter().filter(|d| d.frame < frame).count()
    }

    /// Frames with no objective, alive, in the first `minutes`.
    pub fn idle_in(&self, minutes: u32) -> u32 {
        let hours = (minutes / 60) as usize;
        self.idle_by_hour.iter().take(hours.max(1)).sum()
    }

    /// Frames looking, in the first `minutes`.
    pub fn searching_in(&self, minutes: u32) -> u32 {
        let hours = (minutes / 60) as usize;
        self.search_by_hour.iter().take(hours.max(1)).sum()
    }

    /// Walk to play, hundredths.
    pub fn walk_to_play(&self) -> u32 {
        self.walk_samples * 100 / self.play_samples.max(1)
    }

    /// Deaths per real hour of play, tenths.
    pub fn deaths_per_hour_tenths(&self) -> u32 {
        (self.deaths.len() as u64 * 10 * 60 * u64::from(MINUTE) / u64::from(self.frames.max(1))) as u32
    }

    /// Empty stretches (nothing new) of at least `secs`.
    pub fn stretches_over(&self, secs: u32) -> impl Iterator<Item = &Stretch> {
        self.stretches.iter().filter(move |s| s.kind == 0 && s.secs >= secs)
    }

    /// Walks over new ground with no new landmark of at least `secs`.
    pub fn plain_over(&self, secs: u32) -> impl Iterator<Item = &Stretch> {
        self.stretches.iter().filter(move |s| s.kind == 1 && s.secs >= secs)
    }
}

/// A band from VERIFICATION.md §4, as a test holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    /// The §4 row, in its own words.
    pub claim: &'static str,
    /// The models it holds for.
    pub models: &'static [&'static str],
    /// Frames: at least, at most (`None`: open).
    pub lo: Option<u32>,
    pub hi: Option<u32>,
}

/// VERIFICATION.md §4.1's rows that give numbers, as frames of play. Recorded first, enforced
/// second (§6): `tests/experience.rs` holds the ones marked green there.
pub const FIRST_HOUR: [Band; 8] = [
    Band {
        claim: "Julie's gate reached by the station road",
        models: &["reader"],
        lo: Some(2 * MINUTE),
        hi: Some(4 * MINUTE),
    },
    Band { claim: "Julie's gate reached", models: &["rusher"], lo: Some(3 * MINUTE / 2), hi: Some(3 * MINUTE) },
    Band {
        claim: "The house key from the dog, the kitchen entered",
        models: &["reader"],
        lo: None,
        hi: Some(8 * MINUTE),
    },
    Band { claim: "Icebolt learned in the kitchen", models: &["reader"], lo: None, hi: Some(12 * MINUTE) },
    Band { claim: "The cellar's rats and the meat", models: &["reader"], lo: None, hi: Some(20 * MINUTE) },
    Band { claim: "Time lost, first hour", models: &["reader"], lo: None, hi: Some(6 * MINUTE) },
    Band { claim: "Time lost, first hour", models: &["lost"], lo: None, hi: Some(12 * MINUTE) },
    Band { claim: "Night exposure, first night", models: &["reader"], lo: None, hi: Some(3 * MINUTE) },
];

/// What a §4.1 row measured on one experience: the frames, or `None` when it never happened.
pub fn first_hour_value(x: &Experience, b: &Band) -> Option<u32> {
    match b.claim {
        "Julie's gate reached by the station road" | "Julie's gate reached" => x.done("the_letter"),
        "The house key from the dog, the kitchen entered" => x.done("see_the_kitchen"),
        "Icebolt learned in the kitchen" => x.learned_at("icebolt"),
        "The cellar's rats and the meat" => x.done("rats_below"),
        "Time lost, first hour" => Some(x.idle_in(60) + x.searching_in(60)),
        "Night exposure, first night" => Some(x.night_unlit_first * 60),
        _ => None,
    }
}

/// Does the value sit in the band?
pub fn in_band(b: &Band, v: Option<u32>) -> bool {
    v.is_some_and(|v| b.lo.is_none_or(|lo| v >= lo) && b.hi.is_none_or(|hi| v <= hi))
}
