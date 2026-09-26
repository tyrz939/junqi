//! The living world (ARCHITECTURE.md §4.6, §8; WORLD.md; `living.rs`): the world stream draws
//! the same whatever the party does, the sky holds until it is due, rain puts a fire out at its
//! line, hunting thins a patch and time refills it, a consequence fires once and lands where
//! nobody was, a rumour reaches its person when it says, and a schedule never moves anyone in
//! view. Then the determinism gates with all of it running over a game day.
//! Run under `--profile checked` too.

mod common;

use common::bot::{cmd, idle, place, prop, sym, unit};
use common::{SEED, Tape, bps, new_game};
use jane_core::action::{Facing, School};
use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Milli, Sfc32, Tick, Vec2, ZoneId};
use jane_data::{Region, ScheduleSlot, catalog};
use jane_sim::living::{ScheduleWhere, region_ix};
use jane_sim::state::{FactKey, FlagKey, Source, WeatherKind, WeatherState};
use jane_sim::{Command, DevOp, EventKind, Hit, Seat, Sim, StepInput, UnitId};

const HOUR: u32 = 7200;
const DAY: u32 = 24 * HOUR;

fn tp(sim: &mut Sim, zone: ZoneId) {
    let mark = jane_sim::sym::of_name(catalog().name_id("start").unwrap());
    cmd(sim, Command::Dev(DevOp::Tp { zone, mark }));
    assert_eq!(sim.state().players[0].zone, zone);
}

/// Draws the world stream makes when the hour turns: two per region, one per area of every zone.
fn draws_an_hour() -> u32 {
    let areas: usize = ZoneId::ALL.iter().map(|&z| bps().get(z).areas.len()).sum();
    assert!(areas >= 6, "the county's patches are on its blueprint ({areas})");
    6 + areas as u32
}

fn county_unit_ids(sim: &Sim, prefix: &str) -> Vec<UnitId> {
    let z = sim.state().zone(ZoneId::County).unwrap();
    let syms = &sim.state().syms;
    z.units.iter().filter(|u| u.key.is_some_and(|k| syms.name(k).starts_with(prefix))).map(|u| u.id).collect()
}

fn kill(sim: &mut Sim, z: ZoneId, id: UnitId) {
    sim.queue_hit(
        z,
        Hit { to: id, amount: Milli(1_000_000_000), school: School::Physical, from: None, crit: false, status: None },
    );
}

fn alive(sim: &Sim, id: UnitId) -> bool {
    sim.state().zone(ZoneId::County).unwrap().unit(id).is_some_and(|u| u.alive)
}

fn area_ix(sim: &Sim, name: &str) -> usize {
    let n = catalog().name_id(name).unwrap();
    sim.blueprint(ZoneId::County)
        .areas
        .iter()
        .position(|a| a.name == jane_core::Key::Name(n))
        .expect("the patch is placed")
}

/// §8 `zone_order_does_not_move_rolls`: two parties go round the county's doors in two orders;
/// the world stream, the skies and the county's patches come out the same, draw for draw.
#[test]
fn zone_order_does_not_move_rolls() {
    let mut a = new_game();
    let mut b = new_game();
    let order_a = [ZoneId::Mine, ZoneId::Museum, ZoneId::County, ZoneId::School];
    let order_b = [ZoneId::Museum, ZoneId::School, ZoneId::Mine, ZoneId::County];
    for (i, (za, zb)) in order_a.iter().zip(order_b).enumerate() {
        tp(&mut a, *za);
        tp(&mut b, zb);
        let to = (i as u32 + 1) * HOUR + 17;
        while a.state().tick.0 < to {
            a.step(&StepInput::IDLE);
        }
        while b.state().tick.0 < to {
            b.step(&StepInput::IDLE);
        }
    }
    assert_eq!(a.state().tick, b.state().tick);
    assert_eq!(a.state().rng, b.state().rng, "the world stream");
    assert_eq!(a.state().weather, b.state().weather);
    let pa = &a.state().zone(ZoneId::County).unwrap().pressure;
    assert_eq!(pa, &b.state().zone(ZoneId::County).unwrap().pressure);
    // And it did draw: exactly the hour's draws, every hour, visited or not.
    let mut fresh = Sfc32::seeded(SEED, 1);
    for _ in 0..4 * draws_an_hour() {
        fresh.next_u32();
    }
    assert_eq!(a.state().rng, fresh);
}

/// §4.6.b: the stream moves only when the hour turns, by the same count every hour; a sky
/// changes only on the hour and never before its `until`; the first walk is clear.
#[test]
fn weather_rolls_on_the_hour_and_holds_until_it_is_due() {
    let mut s = new_game();
    let per_hour = draws_an_hour();
    let mut fresh = Sfc32::seeded(SEED, 1);
    let clear = Tick(u32::from(catalog().living.tuning.clear_hours) * HOUR);
    let mut changes = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for _ in 0..DAY {
        let (rng0, sky0) = (s.state().rng, s.state().weather);
        s.step(&StepInput::IDLE);
        let st = s.state();
        if st.clock % HOUR == 0 {
            for _ in 0..per_hour {
                fresh.next_u32();
            }
            assert_eq!(st.rng, fresh, "hour {}", st.clock / HOUR);
            for (now, then) in st.weather.iter().zip(sky0) {
                if *now != then {
                    assert!(then.until <= st.tick, "a sky changed before it was due");
                    changes += 1;
                }
                assert!(now.until > st.tick);
                kinds.insert(now.kind);
            }
        } else {
            assert_eq!(st.rng, rng0, "a draw off the hour");
            assert_eq!(st.weather, sky0);
        }
        if st.tick < clear {
            assert!(st.weather.iter().all(|w| w.kind == WeatherKind::Clear), "the first walk is clear");
        }
    }
    assert!(changes >= 3, "a day of skies changed {changes} times");
    assert!(kinds.len() >= 2, "{kinds:?}");
    let v = s.view(Seat(0)).unwrap();
    assert_eq!(v.weather(), &s.state().weather[region_ix(Region::Lowfields)], "the county is under the Lowfields sky");
}

/// §4.6.b: rain wets the county a step at a time; a campfire's light is out exactly while the
/// ramp stands at its `douse` or over it, and back when it falls under. Indoors stays dry.
#[test]
fn wetness_douses_a_fire_exactly_at_the_threshold() {
    let mut s = new_game();
    tp(&mut s, ZoneId::House);
    tp(&mut s, ZoneId::County);
    let cat = catalog();
    let campfire = cat.story.prop_id("campfire").unwrap();
    let douse = cat.story.prop(campfire).douse.unwrap();
    let fire =
        s.state().zone(ZoneId::County).unwrap().props.iter().find(|p| p.def == campfire && !p.hidden).unwrap().clone();
    let t = cat.living.tuning.wetness;
    let rain = WeatherState { kind: WeatherKind::Rain, since: s.state().tick, until: Tick(u32::MAX) };
    s.state_mut().weather[region_ix(Region::Lowfields)] = rain;
    let (mut went_out, mut prev) = (false, 0u8);
    for _ in 0..(256 / u32::from(t.rise) + 2) * t.every.0 {
        s.step(&StepInput::IDLE);
        let w = s.state().zone(ZoneId::County).unwrap().wetness;
        assert!(w == prev || w == prev.saturating_add(t.rise), "{prev} to {w}");
        prev = w;
        let lit = s.view(Seat(0)).unwrap().light_showing(&fire).is_some();
        assert_eq!(lit, w < douse, "wetness {w}");
        went_out |= !lit;
        assert_eq!(s.state().zone(ZoneId::House).unwrap().wetness, 0, "a roof keeps the rain off");
    }
    assert!(went_out && prev == 255);
    // The rain stops; the ground dries; the fire is a light again under its line.
    s.state_mut().weather[region_ix(Region::Lowfields)] =
        WeatherState { kind: WeatherKind::Clear, since: s.state().tick, until: Tick(u32::MAX) };
    let mut back = false;
    for _ in 0..(256 / u32::from(t.fall) + 2) * t.every.0 {
        s.step(&StepInput::IDLE);
        let w = s.state().zone(ZoneId::County).unwrap().wetness;
        let lit = s.view(Seat(0)).unwrap().light_showing(&fire).is_some();
        assert_eq!(lit, w < douse, "wetness {w}");
        back |= lit;
    }
    assert!(back && s.state().zone(ZoneId::County).unwrap().wetness == 0);
    assert_eq!(s.view(Seat(0)).unwrap().wetness(), 0);
}

/// §4.6.c: one kill does not hold a patch back, so the rat stands up on its own clock; clear the
/// allotments and the rats wait past their clock, each hour taking pressure off, and stand up
/// again on an hour once the patch is under its line.
#[test]
fn hunting_thins_an_area_and_time_refills_it() {
    let cat = catalog();
    let rat = cat.combat.unit_id("rat").unwrap();
    let respawn = cat.combat.unit(rat).respawn;
    let eco = cat.living.ecology_of(cat.name_id("allotments").unwrap()).unwrap();
    let pop = eco.population(rat).unwrap();

    // One rat.
    let mut s = new_game();
    let a = area_ix(&s, "allotments");
    let rats = county_unit_ids(&s, "rat_allotment_");
    assert_eq!(rats.len(), 4);
    kill(&mut s, ZoneId::County, rats[0]);
    s.step(&StepInput::IDLE);
    assert!(!alive(&s, rats[0]));
    let died = s.state().tick;
    assert_eq!(s.state().zone(ZoneId::County).unwrap().pressure[a], pop.weight);
    while s.state().tick < died.after(respawn) {
        s.step(&StepInput::IDLE);
    }
    assert!(alive(&s, rats[0]), "one kill: back on its own clock");

    // All four.
    let mut s = new_game();
    for &r in &rats {
        kill(&mut s, ZoneId::County, r);
    }
    s.step(&StepInput::IDLE);
    let died = s.state().tick;
    assert_eq!(s.state().zone(ZoneId::County).unwrap().pressure[a], 4 * pop.weight);
    while s.state().tick <= died.after(respawn) {
        s.step(&StepInput::IDLE);
    }
    assert!(rats.iter().all(|&r| !alive(&s, r)), "the patch is held back past the rats' own clock");
    let due = &s.state().zone(ZoneId::County).unwrap().sleeping_due;
    for &r in &rats {
        let &(at, _) = due.iter().find(|e| e.1 == r).expect("still due");
        assert!(at > s.state().tick);
    }
    let mut last = s.state().zone(ZoneId::County).unwrap().pressure[a];
    let mut back_at = None;
    for _ in 0..48 * HOUR {
        s.step(&StepInput::IDLE);
        let st = s.state();
        let p = st.zone(ZoneId::County).unwrap().pressure[a];
        if st.clock % HOUR == 0 {
            let off = last - p;
            assert!(off >= eco.recover / 2 && off <= eco.recover / 2 + eco.recover || p == 0, "{last} to {p}");
        } else {
            assert_eq!(p, last, "pressure moves only on the hour, or with a kill");
        }
        last = p;
        let up = rats.iter().filter(|&&r| alive(&s, r)).count();
        if up > 0 && back_at.is_none() {
            assert_eq!(st.clock % HOUR, 0, "they stand up on the hour");
            assert!(p < pop.hold);
            back_at = Some(st.tick);
        }
        if up == rats.len() {
            break;
        }
    }
    let back_at = back_at.expect("the patch refills");
    assert!(rats.iter().all(|&r| alive(&s, r)), "all four back");
    assert!(back_at.0 - died.0 > respawn.0 + HOUR, "quieter than one kill");
}

/// §4.6.d: a consequence fires once, ever, writes the journal and says so to everyone; one whose
/// zone nobody is in waits, through a save and a load, and is there when she walks in.
#[test]
fn a_consequence_fires_once_survives_save_load_and_lands_later() {
    let cat = catalog();
    let id = cat.living.consequence_id("mine_quiet").unwrap();
    let row = &cat.living.consequences[id.index()];
    let mut s = new_game();
    tp(&mut s, ZoneId::Mine);
    let knuckles = FlagKey::Dead(sym(&s, "iron_knuckles"));
    s.state_mut().flags.insert(knuckles, 1);
    s.drain_events();
    idle(&mut s, 1);
    let fired = |s: &mut Sim| s.drain_events().iter().filter(|e| e.kind == EventKind::Consequence(id)).count();
    assert_eq!(fired(&mut s), 1);
    assert!(s.state().consequences_done.get(u32::from(id.0)));
    assert_eq!(s.state().consequences_owed, [(ZoneId::County, id)], "nobody is in the county");
    let j = &s.state().journal;
    assert_eq!(j.known.get(&FactKey::Consequence(id)).map(|k| k.how), Some(Source::Seen));
    let claim = FactKey::Claim(row.contradicts.unwrap());
    assert_eq!(j.known.get(&claim).map(|k| k.how), Some(Source::Contradicted));
    // Never again: the bit is the proof.
    s.state_mut().flags.remove(&knuckles);
    idle(&mut s, 3);
    s.state_mut().flags.insert(knuckles, 1);
    idle(&mut s, 3);
    assert_eq!(fired(&mut s), 0);
    assert_eq!(s.state().consequences_owed.len(), 1);

    let watcher = sym(&s, "mine_watcher");
    let in_county = |s: &Sim| s.state().zone(ZoneId::County).unwrap().units.iter().any(|u| u.key == Some(watcher));
    assert!(!in_county(&s));
    let mut t = Sim::from_save_with(&s.save(), bps()).expect("loads");
    assert_eq!(t.state(), s.state());
    tp(&mut t, ZoneId::County);
    assert!(in_county(&t), "the county has it the moment she walks in");
    assert!(t.state().consequences_owed.is_empty());
    assert_eq!(t.state().flags.get(&FlagKey::Named(sym(&t, "mine_quiet"))), Some(&1));
    let w = unit(&t, "mine_watcher");
    let m = t.runtime(ZoneId::County).unwrap().mark(sym(&t, "company_notice")).unwrap();
    let at = Vec2::centre(i32::from(m.cell.x), i32::from(m.cell.y));
    assert!(dist_sq(w.pos, at) <= i64::from(6 * CELL_FX).pow(2), "by the Company notice");
    idle(&mut t, 30);
    assert_eq!(fired(&mut t), 0);

    // A consequence of the zone she stands in lands the same tick.
    let mut s = new_game();
    let before = county_unit_ids(&s, "rat_allotment_").len();
    let q = cat.story.quest_id("rats_in_the_sheds").unwrap();
    s.state_mut().quests.done.push(q);
    idle(&mut s, 1);
    assert_eq!((before, county_unit_ids(&s, "rat_allotment_").len()), (4, 0), "rats at four, not eight");
    assert_eq!(county_unit_ids(&s, "rat_shed_").len(), 4);
    assert!(s.state().consequences_owed.is_empty());
}

/// §4.6.e: a story done is heard of by the person it spreads to after `after`, not before; the
/// line that asks `SpeakerKnows` plays only then, and telling it writes the rumour.
#[test]
fn a_rumour_reaches_its_person_after_its_delay() {
    let cat = catalog();
    let ames = cat.county.stories.iter().find(|s| s.key == "ames").unwrap();
    let tree = cat.story.dialogue(cat.story.dialogue_id("door_pound_3_heard").unwrap());
    let heard = tree.node_index("heard").unwrap();
    let mut s = new_game();
    let door = prop(&s, "door_pound_3");
    let def = cat.story.prop(door.def);
    place(
        &mut s,
        i32::from(door.cell.x) + i32::from(def.w) / 2,
        i32::from(door.cell.y) + i32::from(def.h),
        Facing::North,
    );
    idle(&mut s, 2);
    let node = |s: &mut Sim| {
        cmd(s, Command::Use);
        let d = s.view(Seat(0)).unwrap().dialogue().expect("the door answers");
        assert_eq!(d.tree, cat.story.dialogue_id("door_pound_3_heard"));
        let n = s.state().players[0].dialogue.unwrap().node;
        cmd(s, Command::CloseDialogue);
        n
    };
    assert_ne!(node(&mut s), heard);
    s.state_mut().quests.done.push(ames.quests[0]);
    idle(&mut s, 1);
    let key = (cat.name_id("door_pound_3").unwrap(), ames.id);
    let due = *s.state().rumours.get(&key).expect("the door will hear");
    assert_eq!(due, s.state().tick.after(ames.spreads.unwrap().after));
    s.state_mut().tick = Tick(due.0 - 1);
    assert_ne!(node(&mut s), heard, "not a tick early");
    assert!(!s.state().journal.known.contains_key(&FactKey::Rumour(ames.id)));
    s.state_mut().tick = due;
    assert_eq!(node(&mut s), heard);
    let k = s.state().journal.known.get(&FactKey::Rumour(ames.id)).copied();
    assert_eq!(k.map(|k| k.how), Some(Source::Heard), "the line tells it");
}

/// §4.6.a: Mr Cobb's hours. Unwatched he is simply where the hour says; watched, nothing
/// vanishes, appears or jumps: he goes in only once she looks away, comes out only when
/// neither end is watched, and walks from the yard to his seat.
#[test]
fn a_schedule_obeys_the_watcher_box() {
    let mut s = new_game();
    let cobb = unit(&s, "mr_cobb").id;
    let tilly = unit(&s, "tilly").id;
    let mark = |s: &Sim, n: &str| s.runtime(ZoneId::County).unwrap().mark(sym(s, n)).unwrap().cell;
    let state = |s: &Sim, u: UnitId| s.view(Seat(0)).unwrap().schedule_state(u).expect("scheduled");
    let far = mark(&s, "start");
    let front = catalog().name_id("arms_front").unwrap();
    let yard = catalog().name_id("arms_yard").unwrap();
    let to_hour = |s: &mut Sim, h: u32| {
        s.state_mut().clock = h * HOUR - 1;
        idle(s, 31);
    };
    let stand = |s: &mut Sim, c: jane_core::Cell| place(s, i32::from(c.x), i32::from(c.y), Facing::South);
    let near_unit = |s: &mut Sim, u: UnitId| {
        let (x, y) = s.state().zone(ZoneId::County).unwrap().unit(u).unwrap().pos.cell();
        place(s, x + 2, y, Facing::West);
    };

    // 17:00, nobody near: at his seat.
    idle(&mut s, 31);
    assert_eq!(state(&s, cobb).at, ScheduleWhere::Mark(front));

    // 21:00 with her beside him: he stays out; she walks off, he goes in.
    near_unit(&mut s, cobb);
    to_hour(&mut s, 21);
    let st = state(&s, cobb);
    assert!(matches!(st.slot, ScheduleSlot::Inside(_)) && matches!(st.at, ScheduleWhere::Walking(_)), "{st:?}");
    assert!(!unit(&s, "mr_cobb").hidden);
    stand(&mut s, far);
    idle(&mut s, 30);
    let door = prop(&s, "arms_door").id;
    assert_eq!(state(&s, cobb).at, ScheduleWhere::Inside(door), "behind the Arms door");
    // The dog's shorthand: a dayOnly row is `Absent` after the bell, and Tilly went in unseen.
    let t = state(&s, tilly);
    assert_eq!((t.slot, t.at), (ScheduleSlot::Absent, ScheduleWhere::Away));

    // 06:00, her standing where he would come out: he waits inside.
    let yard_cell = mark(&s, "arms_yard");
    stand(&mut s, yard_cell);
    to_hour(&mut s, 6);
    assert_eq!(state(&s, cobb).at, ScheduleWhere::Inside(door));
    stand(&mut s, far);
    idle(&mut s, 30);
    assert_eq!(state(&s, cobb).at, ScheduleWhere::Mark(yard), "out into the yard once nobody looks");

    // 09:00, watched: to his seat on foot, never a jump.
    near_unit(&mut s, cobb);
    s.state_mut().clock = 9 * HOUR - 1;
    let mut last = unit(&s, "mr_cobb").pos;
    let (mut sent, mut arrived) = (false, false);
    for _ in 0..60 * 60 {
        s.step(&StepInput::IDLE);
        let u = unit(&s, "mr_cobb");
        assert!(dist_sq(u.pos, last) <= i64::from(2 * 256).pow(2), "a step at a time");
        last = u.pos;
        sent |= u.order.is_some();
        if state(&s, cobb).at == ScheduleWhere::Mark(front) {
            arrived = true;
            break;
        }
    }
    assert!(sent, "sent, not moved");
    assert!(arrived, "he reached his seat");
}

// --- the determinism gates, with the living world running over a game day ---------------------

/// What the day's tape does to the world besides moving: a story and a quest done (a rumour, a
/// consequence), the allotments hunted, a named death away from the county. Applied at the same
/// frames to every sim, so a run and its replay see the same world.
fn living_edits(sim: &mut Sim, f: u32) {
    let cat = catalog();
    match f {
        10 => {
            let st = sim.state_mut();
            st.quests.done.push(cat.story.quest_id("ames_spectacles").unwrap());
            st.quests.done.push(cat.story.quest_id("rats_in_the_sheds").unwrap());
        }
        20_000 | 90_000 if sim.state().is_live(ZoneId::County) => {
            for r in county_unit_ids(sim, "rat_allotment_") {
                if alive(sim, r) {
                    kill(sim, ZoneId::County, r);
                }
            }
        }
        60_000 => {
            let k = FlagKey::Dead(sim.state().syms.find("iron_knuckles").unwrap());
            sim.state_mut().flags.insert(k, 1);
        }
        _ => {}
    }
}

fn day_tape(seed: u32, guests: bool) -> Tape {
    let mut t = Tape::new(seed);
    t.guests = guests;
    t
}

fn run_day(sim: &mut Sim, tape: &mut Tape, from: u32, to: u32) {
    for f in from..to {
        living_edits(sim, f);
        let input = tape.frame(f);
        sim.step(&input);
    }
}

/// `same_tape_same_hash` over a day of skies, rain, hunts, consequences and rumours.
#[test]
fn same_tape_same_hash_over_a_living_day() {
    let mut a = new_game();
    let mut b = new_game();
    let (mut ta, mut tb) = (day_tape(51, true), day_tape(51, true));
    let mut skies = std::collections::BTreeSet::new();
    for h in 0..24 {
        run_day(&mut a, &mut ta, h * HOUR, (h + 1) * HOUR);
        run_day(&mut b, &mut tb, h * HOUR, (h + 1) * HOUR);
        assert_eq!(a.hash(), b.hash(), "hour {h}");
        skies.extend(a.state().weather.iter().map(|w| w.kind));
    }
    assert_eq!(a.state(), b.state());
    // The day did live.
    assert!(skies.len() >= 2, "{skies:?}");
    assert!(!a.state().rumours.is_empty());
    assert!(a.state().consequences_done.get(0) && a.state().consequences_done.get(1));
}

/// `save_load_continue`: saved twice across the day, loaded, continued: the run that never
/// saved.
#[test]
fn save_load_continue_over_a_living_day() {
    let mut straight = new_game();
    let mut ts = day_tape(52, false);
    run_day(&mut straight, &mut ts, 0, DAY);
    for at in [31_003, 97_777] {
        let mut first = new_game();
        let mut t = day_tape(52, false);
        run_day(&mut first, &mut t, 0, at);
        let mut resumed = Sim::from_save_with(&first.save(), bps()).expect("the save loads");
        assert_eq!(resumed.hash(), first.hash(), "loading changes nothing (frame {at})");
        run_day(&mut resumed, &mut t, at, DAY);
        assert_eq!(resumed.hash(), straight.hash(), "saved at {at}");
        assert_eq!(resumed.state(), straight.state());
    }
}

/// `runtime_rebuild_is_invisible` with the living world running.
#[test]
fn runtime_rebuild_is_invisible_over_a_living_day() {
    let mut a = new_game();
    let mut b = new_game();
    let (mut ta, mut tb) = (day_tape(53, true), day_tape(53, true));
    for f in 0..DAY {
        run_day(&mut a, &mut ta, f, f + 1);
        if f % 9_973 == 13 {
            b.rebuild_runtimes();
        }
        run_day(&mut b, &mut tb, f, f + 1);
        if f % HOUR == 0 {
            assert_eq!(a.hash(), b.hash(), "frame {f}");
        }
    }
    assert_eq!(a.state(), b.state());
}
