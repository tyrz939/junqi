//! The living world (ARCHITECTURE.md §4.6, §8; WORLD.md; `living.rs`): the world stream draws
//! the same whatever the party does, the sky holds until it is due and follows the region under
//! her feet, rain puts a fire out at its line by its own region's ramp, hunting thins a patch and
//! every ten minutes time refills it, a consequence fires once and lands where nobody was (a
//! lock among its edits), a rumour reaches its person when it says, a schedule never moves anyone
//! in view, and a bed's night is the night idled through, world for world. Then the determinism
//! gates with all of it running over a game day. Run under `--profile checked` too.

mod common;

use std::sync::Arc;

use common::bot::{cmd, holds, idle, place, prop, sym, unit, walk_to_prop, zone_of};
use common::{SEED, Tape, bps, new_game};
use jane_core::action::{Facing, School};
use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Action, Key, Milli, Sfc32, Tick, Vec2, ZoneId};
use jane_data::{Region, ScheduleSlot, catalog};
use jane_sim::event::ToastKind;
use jane_sim::interact::Verb;
use jane_sim::living::{ScheduleWhere, region_at, region_ix};
use jane_sim::save::hash_of;
use jane_sim::state::{FactKey, FlagKey, NightState, Source, WeatherKind, WeatherState};
use jane_sim::{Blueprints, Command, DevOp, EventKind, Hit, Seat, Sim, StepInput, UnitId};

const HOUR: u32 = 7200;
const DAY: u32 = 24 * HOUR;
/// The ecology's step: ten game minutes.
const MARK: u32 = HOUR / 6;

fn tp(sim: &mut Sim, zone: ZoneId) {
    let mark = jane_sim::sym::of_name(catalog().name_id("start").unwrap());
    cmd(sim, Command::Dev(DevOp::Tp { zone, mark }));
    assert_eq!(sim.state().players[0].zone, zone);
}

/// Areas of every blueprint: one ecology draw each, every ten minutes.
fn areas() -> u32 {
    let areas: usize = ZoneId::ALL.iter().map(|&z| bps().get(z).areas.len()).sum();
    assert!(areas >= 6, "the county's patches are on its blueprint ({areas})");
    areas as u32
}

/// Draws the world stream makes at a ten-minute mark: two per region on the hour, then one per
/// area of every zone.
fn draws_at(clock: u32) -> u32 {
    match clock {
        c if c % HOUR == 0 => 6 + areas(),
        c if c % MARK == 0 => areas(),
        _ => 0,
    }
}

/// Draws in a whole hour: the skies once, the areas six times.
fn draws_an_hour() -> u32 {
    6 + 6 * areas()
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

fn sky(kind: WeatherKind, now: Tick) -> WeatherState {
    WeatherState { kind, since: now, until: Tick(u32::MAX) }
}

/// A free cell of the county in `region`, from the region map (the skeleton's macro grid).
fn cell_in(sim: &Sim, region: Region) -> (i32, i32) {
    let bp = sim.blueprint(ZoneId::County);
    let grid = &sim.runtime(ZoneId::County).expect("she is in the county").grid;
    let m = &bp.regions;
    let s = i32::from(m.scale);
    for my in (0..i32::from(m.h)).rev() {
        for mx in 0..i32::from(m.w) {
            if m.region_at(mx * s, my * s) != Some(region_ix(region) as u8) {
                continue;
            }
            let (x, y) = (mx * s + s / 2, my * s + s / 2);
            if let Some((x, y)) = grid.nearest_free(x, y, 3, None) {
                if region_at(bp, x, y) == region {
                    return (x, y);
                }
            }
        }
    }
    panic!("no free cell in {region:?}");
}

fn weather_events(sim: &mut Sim) -> Vec<(Option<Seat>, Region, WeatherKind)> {
    sim.drain_events()
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Weather { region, kind } => Some((e.to, region, kind)),
            _ => None,
        })
        .collect()
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
    // And it did draw: the omens at New Game, then exactly each mark's draws, every mark, visited
    // or not.
    let mut fresh = Sfc32::seeded(SEED, 1);
    for _ in 0..catalog().story.omens.len() as u32 + 4 * draws_an_hour() {
        fresh.next_u32();
    }
    assert_eq!(a.state().rng, fresh);
}

/// §4.6.b, §4.6.c: the stream moves only on a ten-minute mark, the skies' draws only on the hour;
/// a sky changes only on the hour and never before its `until`; the first walk is clear.
#[test]
fn weather_rolls_on_the_hour_and_holds_until_it_is_due() {
    let mut s = new_game();
    let mut fresh = Sfc32::seeded(SEED, 1);
    // New Game's own draws: one an omen (omens.rs).
    for _ in 0..catalog().story.omens.len() {
        fresh.next_u32();
    }
    let clear = Tick(u32::from(catalog().living.tuning.clear_hours) * HOUR);
    let mut changes = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for _ in 0..DAY {
        let (rng0, sky0) = (s.state().rng, s.state().weather);
        s.step(&StepInput::IDLE);
        let st = s.state();
        for _ in 0..draws_at(st.clock) {
            fresh.next_u32();
        }
        assert_eq!(st.rng, fresh, "{}:{:02}", st.clock / HOUR, st.clock % HOUR / 120);
        if st.clock % MARK != 0 {
            assert_eq!(st.rng, rng0, "a draw off the mark");
        }
        if st.clock % HOUR == 0 {
            for (now, then) in st.weather.iter().zip(sky0) {
                if *now != then {
                    assert!(then.until <= st.tick, "a sky changed before it was due");
                    changes += 1;
                }
                assert!(now.until > st.tick);
                kinds.insert(now.kind);
            }
        } else {
            assert_eq!(st.weather, sky0, "a sky moves only on the hour");
        }
        if st.tick < clear {
            assert!(st.weather.iter().all(|w| w.kind == WeatherKind::Clear), "the first walk is clear");
        }
    }
    assert!(changes >= 3, "a day of skies changed {changes} times");
    assert!(kinds.len() >= 2, "{kinds:?}");
    let v = s.view(Seat(0)).unwrap();
    assert_eq!(v.region(), Region::Lowfields, "she starts in the town");
    assert_eq!(v.weather(), &s.state().weather[region_ix(Region::Lowfields)], "the sky over her is her region's");
}

/// Decision 1 (§4.6.b): the county is three skies, by the skeleton's region under each cell. The
/// sky over her is the one over her feet; walking into another region, or her own sky turning,
/// tells her (and only her); the ramp under her is her region's; any other zone is under its
/// zone's sky.
#[test]
fn the_sky_follows_the_region_under_her_feet() {
    let mut s = new_game();
    let bp = Arc::clone(s.blueprint(ZoneId::County));
    assert_eq!((bp.regions.scale, bp.regions.w, bp.regions.h), (16, 125, 125), "the skeleton's macro grid");
    for r in [Region::Lowfields, Region::Waters, Region::Works] {
        let n = bp.regions.cells.iter().map(|&b| u32::from(b == region_ix(r) as u8)).sum::<u32>();
        assert!(n > 1000, "{r:?} covers {n} macro cells");
    }
    let waters = cell_in(&s, Region::Waters);
    let works = cell_in(&s, Region::Works);
    let now = s.state().tick;
    let st = s.state_mut();
    st.weather[region_ix(Region::Lowfields)] = sky(WeatherKind::Clear, now);
    st.weather[region_ix(Region::Waters)] = sky(WeatherKind::Rain, now);
    st.weather[region_ix(Region::Works)] = sky(WeatherKind::Clear, now);
    idle(&mut s, 1);
    assert_eq!(weather_events(&mut s), [], "her own sky did not change");
    assert_eq!(s.view(Seat(0)).unwrap().weather().kind, WeatherKind::Clear);

    // Across the river: the Waters' rain is over her, and she is told.
    place(&mut s, waters.0, waters.1, Facing::South);
    idle(&mut s, 1);
    assert_eq!(weather_events(&mut s), [(Some(Seat(0)), Region::Waters, WeatherKind::Rain)]);
    let v = s.view(Seat(0)).unwrap();
    assert_eq!((v.region(), v.weather().kind), (Region::Waters, WeatherKind::Rain));
    // The Waters' ramp climbs under her; the Lowfields' and the Works' stay dry.
    idle(&mut s, 10 * 60);
    let w = s.state().zone(ZoneId::County).unwrap().wetness;
    assert!(w[region_ix(Region::Waters)] > 0 && w[region_ix(Region::Lowfields)] == 0, "{w:?}");
    assert_eq!(w[region_ix(Region::Works)], 0);
    assert_eq!(s.view(Seat(0)).unwrap().wetness(), w[region_ix(Region::Waters)]);

    // Into the Works: another region is another sky, clear or not.
    place(&mut s, works.0, works.1, Facing::South);
    idle(&mut s, 1);
    assert_eq!(weather_events(&mut s), [(Some(Seat(0)), Region::Works, WeatherKind::Clear)]);
    assert_eq!(s.view(Seat(0)).unwrap().wetness(), 0);
    // Standing still, her sky turning is said too.
    let now = s.state().tick;
    s.state_mut().weather[region_ix(Region::Works)] = sky(WeatherKind::Mist, now);
    idle(&mut s, 1);
    assert_eq!(weather_events(&mut s), [(Some(Seat(0)), Region::Works, WeatherKind::Mist)]);

    // Any other zone is its row's in weather.json: the Museum is under the Waters.
    tp(&mut s, ZoneId::Museum);
    let v = s.view(Seat(0)).unwrap();
    assert_eq!((v.region(), v.weather().kind), (Region::Waters, WeatherKind::Rain));
    assert!(s.blueprint(ZoneId::Museum).regions.is_empty());
    let museum = s.state().zone(ZoneId::Museum).unwrap().wetness;
    assert_eq!((museum[region_ix(Region::Lowfields)], museum[region_ix(Region::Works)]), (0, 0), "one ramp");
}

/// §4.6.b: rain wets the county a step at a time, region by region; a campfire's light is out
/// exactly while the ramp of its own region stands at its `douse` or over it, and back when it
/// falls under, whatever the other regions' ramps say. Indoors stays dry.
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
    let here = s.runtime(ZoneId::County).unwrap().region_at(i32::from(fire.cell.x), i32::from(fire.cell.y));
    let other = if here == Region::Waters { Region::Works } else { Region::Waters };
    let (r, o) = (region_ix(here), region_ix(other));
    let t = cat.living.tuning.wetness;
    let ramp = |s: &Sim| s.state().zone(ZoneId::County).unwrap().wetness;
    let now = s.state().tick;
    s.state_mut().weather[r] = sky(WeatherKind::Rain, now);
    s.state_mut().weather[o] = sky(WeatherKind::Clear, now);
    let (mut went_out, mut prev) = (false, 0u8);
    for _ in 0..(256 / u32::from(t.rise) + 2) * t.every.0 {
        s.step(&StepInput::IDLE);
        let w = ramp(&s)[r];
        assert!(w == prev || w == prev.saturating_add(t.rise), "{prev} to {w}");
        prev = w;
        let lit = s.view(Seat(0)).unwrap().light_showing(&fire).is_some();
        assert_eq!(lit, w < douse, "wetness {w}");
        went_out |= !lit;
        assert_eq!(ramp(&s)[o], 0, "another region's sky is clear");
        assert_eq!(s.state().zone(ZoneId::House).unwrap().wetness, [0; 3], "a roof keeps the rain off");
    }
    assert!(went_out && prev == 255);
    // The rain moves to the other region: the ground dries here and the fire is a light again
    // under its line, while the other region's ramp is soaked.
    let now = s.state().tick;
    s.state_mut().weather[r] = sky(WeatherKind::Clear, now);
    s.state_mut().weather[o] = sky(WeatherKind::Storm, now);
    let mut back = false;
    for _ in 0..(256 / u32::from(t.fall) + 2) * t.every.0 {
        s.step(&StepInput::IDLE);
        let w = ramp(&s)[r];
        let lit = s.view(Seat(0)).unwrap().light_showing(&fire).is_some();
        assert_eq!(lit, w < douse, "wetness {w}");
        back |= lit;
    }
    assert!(back && ramp(&s)[r] == 0);
    assert_eq!(ramp(&s)[o], 255);
    assert!(s.view(Seat(0)).unwrap().light_showing(&fire).is_some(), "the storm is over another region");
}

/// §4.6.c, decision 2: one kill does not hold a patch back, so the rat stands up at the next
/// ten-minute mark, when the ecology looks (QUESTS.md K8), and, if she is watching where it lies
/// or its home, at the first mark she is not; clear the allotments and the rats wait, every ten
/// game minutes taking pressure off and looking at them again, and stand up again on a mark once
/// the patch is under its line.
#[test]
fn hunting_thins_an_area_and_time_refills_it() {
    let cat = catalog();
    let rat = cat.combat.unit_id("rat").unwrap();
    let eco = cat.living.ecology_of(cat.name_id("allotments").unwrap()).unwrap();
    let pop = eco.population(rat).unwrap();
    let next_mark = |s: &Sim| s.state().tick.after(Tick(MARK - s.state().clock % MARK));

    // One rat: back at the next mark, not after its row's `respawn`.
    let mut s = new_game();
    let a = area_ix(&s, "allotments");
    let rats = county_unit_ids(&s, "rat_allotment_");
    assert_eq!(rats.len(), 4);
    kill(&mut s, ZoneId::County, rats[0]);
    s.step(&StepInput::IDLE);
    assert!(!alive(&s, rats[0]));
    assert_eq!(s.state().zone(ZoneId::County).unwrap().pressure[a], pop.weight);
    let mark = next_mark(&s);
    assert_eq!(s.state().zone(ZoneId::County).unwrap().sleeping_due, [(mark, rats[0])]);
    while s.state().tick < mark {
        assert!(!alive(&s, rats[0]));
        s.step(&StepInput::IDLE);
    }
    assert!(alive(&s, rats[0]), "one kill: back at the next ten-minute mark");

    // Watched where it lies: not at that mark, but the first one she is not looking.
    let mut s = new_game();
    let at = s.state().zone(ZoneId::County).unwrap().unit(rats[0]).unwrap().pos;
    let (x, y) = at.cell();
    place(&mut s, x + 2, y, Facing::West);
    kill(&mut s, ZoneId::County, rats[0]);
    s.step(&StepInput::IDLE);
    let mark = next_mark(&s);
    while s.state().tick <= mark {
        s.step(&StepInput::IDLE);
    }
    assert!(!alive(&s, rats[0]), "nothing stands up in view");
    let again = s.state().zone(ZoneId::County).unwrap().sleeping_due.iter().find(|e| e.1 == rats[0]).unwrap().0;
    assert_eq!(again, mark.after(Tick(MARK)), "looked at again at the next mark");
    let home = s.state().zone(ZoneId::County).unwrap().unit(rats[0]).unwrap().home;
    let far = (at.x.0.max(home.x.0) / CELL_FX) + 20;
    let rt = s.runtime(ZoneId::County).unwrap();
    let (fx, fy) = rt.grid.nearest_free(far, y, 12, None).expect("somewhere out of sight");
    place(&mut s, fx, fy, Facing::East);
    while s.state().tick < again {
        s.step(&StepInput::IDLE);
    }
    assert!(alive(&s, rats[0]), "back at the first mark she was not looking");

    // All four.
    let mut s = new_game();
    for &r in &rats {
        kill(&mut s, ZoneId::County, r);
    }
    s.step(&StepInput::IDLE);
    let died = s.state().tick;
    assert_eq!(s.state().zone(ZoneId::County).unwrap().pressure[a], 4 * pop.weight);
    let mark = next_mark(&s);
    while s.state().tick <= mark {
        s.step(&StepInput::IDLE);
    }
    assert!(rats.iter().all(|&r| !alive(&s, r)), "the patch is held back past the first mark");
    let now = s.state().tick;
    let due = &s.state().zone(ZoneId::County).unwrap().sleeping_due;
    for &r in &rats {
        let &(at, _) = due.iter().find(|e| e.1 == r).expect("still due");
        assert!(at > now && at.0 - now.0 <= MARK, "looked at again at the next mark, not the next hour");
    }
    let mut last = s.state().zone(ZoneId::County).unwrap().pressure[a];
    let mut back_at = None;
    for _ in 0..48 * HOUR {
        s.step(&StepInput::IDLE);
        let st = s.state();
        let p = st.zone(ZoneId::County).unwrap().pressure[a];
        if st.clock % MARK == 0 {
            let off = last - p;
            assert!(off >= eco.recover / 2 && off <= eco.recover / 2 + eco.recover || p == 0, "{last} to {p}");
        } else {
            assert_eq!(p, last, "pressure moves only on a mark, or with a kill");
        }
        last = p;
        let up = rats.iter().filter(|&&r| alive(&s, r)).count();
        if up > 0 && back_at.is_none() {
            assert_eq!(st.clock % MARK, 0, "they stand up on a mark");
            assert!(p < pop.hold);
            back_at = Some(st.tick);
        }
        if up == rats.len() {
            break;
        }
    }
    let back_at = back_at.expect("the patch refills");
    assert!(rats.iter().all(|&r| alive(&s, r)), "all four back");
    assert!(back_at.0 - died.0 > 6 * HOUR, "quieter for hours than one kill");
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
    // Tilly went in unseen, behind her own door: after the bell only the dog is `Absent`
    // (WORLD.md §2.2), and the rest of the town is somewhere a slot names.
    let t = state(&s, tilly);
    let home = prop(&s, "door_pound_1").id;
    let pound_1 = catalog().name_id("door_pound_1").unwrap();
    assert_eq!((t.slot, t.at), (ScheduleSlot::Inside(pound_1), ScheduleWhere::Inside(home)));

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

/// Decision 4 (§4.6.d), WORLD.md §6: Julie's Kitchen done while she is in the house puts a night
/// lock she holds on Julie's door. The county is nobody's, so it waits; she walks out and it is
/// there, through a save. At night the door answers only a key that fits it (hers is bound, never
/// used up); by day it answers anyone; and the door out of the house never shuts her in.
#[test]
fn a_consequence_night_locks_a_door_where_nobody_is_and_her_key_opens_it() {
    let cat = catalog();
    let id = cat.living.consequence_id("house_kept").unwrap();
    let key = cat.combat.item_id("key_auntie_house").unwrap();
    let mut s = new_game();
    let door = prop(&s, "house_door").id;
    let night = |s: &Sim| {
        let z = s.state().zone(ZoneId::County).unwrap();
        z.props[z.prop_ix(door).unwrap() as usize].night
    };
    let ix = s.state().zone(ZoneId::County).unwrap().prop_ix(door).unwrap() as usize;
    s.state_mut().zone_mut(ZoneId::County).unwrap().props[ix].locked = false;
    s.rebuild_runtimes();
    tp(&mut s, ZoneId::House);
    s.drain_events();
    s.state_mut().quests.done.push(cat.story.quest_id("see_the_kitchen").unwrap());
    idle(&mut s, 1);
    assert!(s.drain_events().iter().any(|e| e.kind == EventKind::Consequence(id)));
    assert_eq!(s.state().consequences_owed, [(ZoneId::County, id)], "nobody is in the county");
    assert_eq!(night(&s), NightState::AsSpawned, "not yet: the county is not live");
    let mut t = Sim::from_save_with(&s.save(), bps()).expect("loads");
    tp(&mut t, ZoneId::County);
    let NightState::Locked(lock) = night(&t) else { panic!("night-locked the moment she is out") };
    assert!(lock.keyed && (lock.from, lock.to) == (21, 6), "the bell's hours, and her key");
    assert!(t.state().consequences_owed.is_empty());
    let d = prop(&t, "house_door");
    let def = catalog().story.prop(d.def);
    let front = (i32::from(d.cell.x) + i32::from(def.w) / 2, i32::from(d.cell.y) + i32::from(def.h));
    let verb = |t: &Sim| t.view(Seat(0)).unwrap().focus().map(|f| f.verb);
    place(&mut t, front.0, front.1, Facing::North);
    // No key, after the bell: not answered. By day: answered.
    cmd(&mut t, Command::Dev(DevOp::Time { hour: 22 }));
    idle(&mut t, 2);
    assert_eq!(verb(&t), Some(Verb::TryTheDoor));
    t.drain_events();
    cmd(&mut t, Command::Use);
    assert!(t.drain_events().iter().any(|e| matches!(e.kind, EventKind::Toast(ToastKind::NightLock(_)))));
    idle(&mut t, 2);
    assert_eq!(zone_of(&t), ZoneId::County);
    cmd(&mut t, Command::Dev(DevOp::Time { hour: 10 }));
    assert_ne!(verb(&t), Some(Verb::TryTheDoor), "by day, anyone");
    // Her key, after the bell: in, and the key is still hers.
    cmd(&mut t, Command::Dev(DevOp::Time { hour: 22 }));
    cmd(&mut t, Command::Dev(DevOp::Give { item: key, qty: 1 }));
    assert_ne!(verb(&t), Some(Verb::TryTheDoor));
    cmd(&mut t, Command::Use);
    idle(&mut t, 2);
    assert_eq!(zone_of(&t), ZoneId::House);
    assert_eq!(holds(&t, "key_auntie_house"), 1, "a bound key is not used up");
    // Never shut in: the way out answers at night, key or none.
    let key_slot = t.state().players[0].bag.iter().position(|x| x.is_some_and(|x| x.item == key)).unwrap();
    t.state_mut().players[0].bag[key_slot] = None;
    assert!(walk_to_prop(&mut t, "front_door"));
    assert_ne!(verb(&t), Some(Verb::TryTheDoor));
    cmd(&mut t, Command::Use);
    idle(&mut t, 2);
    assert_eq!(zone_of(&t), ZoneId::County);
}

// --- a bed's night (decision 3) --------------------------------------------------------------

/// The seed's blueprints with a bed beside a free cell near the county's `start`, whose use
/// sleeps to six (`Rest { until: 6 }`); and that cell, where she stands facing it.
fn a_county_with_a_bed() -> (Blueprints, (i32, i32)) {
    let real = bps();
    let probe = new_game();
    let grid = &probe.runtime(ZoneId::County).unwrap().grid;
    let start = probe.blueprint(ZoneId::County).marks[&Key::Name(catalog().name_id("start").unwrap())].cell;
    let (sx, sy) = (i32::from(start.x), i32::from(start.y));
    let spot = (0..30)
        .flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (sx + dx, sy + dy))))
        .find(|&(x, y)| (0..4).all(|dx| (-2..=2).all(|dy| grid.free(x + dx, y + dy, None))))
        .expect("a free spot by the start");
    let mut county = (**real.get(ZoneId::County)).clone();
    let k = county.local("night_bed");
    let sleep = county.push_list(vec![Action::Rest { until: Some(6) }]);
    let mut bed = common::room::spawn(k, "bed", (spot.0 + 1) as u16, (spot.1 - 1) as u16);
    bed.use_list = Some(sleep);
    county.props.push(bed);
    let county = Arc::new(county);
    let zones = std::array::from_fn(|i| {
        if ZoneId::ALL[i] == ZoneId::County { Arc::clone(&county) } else { Arc::clone(real.get(ZoneId::ALL[i])) }
    });
    (Blueprints::from_parts(SEED, zones), spot)
}

/// Everything world-level (§4.6): the clock and the tick, the world stream, the skies, the
/// flags, the consequences and what is owed, the rumours; and per zone state, the ramps, the
/// patches' pressure, the corpses due and which units stand. Not in it: where anything is, who is
/// awake, what anyone is doing or has in mind, her own body, the fog, the journal's sightings,
/// the id counters, `frame`.
fn world_hash(s: &Sim) -> u64 {
    let st = s.state();
    let zones: Vec<_> = st
        .zones
        .iter()
        .flatten()
        .map(|z| {
            let standing: Vec<(UnitId, bool)> = z.units.iter().map(|u| (u.id, u.alive)).collect();
            (z.id as u8, z.wetness, z.pressure.clone(), z.sleeping_due.clone(), standing)
        })
        .collect();
    let cons: Vec<_> = (0..catalog().living.consequences.len() as u16)
        .map(|i| st.journal.known.get(&FactKey::Consequence(jane_core::ConsequenceId(i))).map(|k| k.since))
        .collect();
    hash_of(&(
        (st.tick, st.clock, st.day, st.rng, st.weather),
        (&st.flags, &st.consequences_done, &st.consequences_owed, &st.rumours),
        cons,
        zones,
    ))
}

/// The same night two ways, from 22:00: one sim uses the bed (the night passes in one step), the
/// other idles by it through the same eight hours, a step a tick. `kills` of the allotments' four
/// rats die at five in the afternoon, due at three in the morning. Iron Knuckles is dead the
/// tick the night starts, so the mine's consequence fires on the night's first tick and lands in
/// the live county.
fn the_same_night(kills: usize) -> (Sim, Sim) {
    let (bps, (x, y)) = a_county_with_a_bed();
    let cat = catalog();
    let mut sims = [Sim::new_game_with(bps.clone(), "Jane"), Sim::new_game_with(bps, "Jane")];
    for s in &mut sims {
        place(s, x, y, Facing::East);
        cmd(s, Command::Dev(DevOp::God(true)));
        for r in county_unit_ids(s, "rat_allotment_").into_iter().take(kills) {
            kill(s, ZoneId::County, r);
        }
        idle(s, 1);
        cmd(s, Command::Dev(DevOp::Time { hour: 22 }));
        let knuckles = FlagKey::Dead(sym(s, "iron_knuckles"));
        s.state_mut().flags.insert(knuckles, 1);
        s.state_mut().quests.done.push(cat.story.quest_id("ames_spectacles").unwrap());
        s.drain_events();
    }
    let [mut slept, mut idled] = sims;
    assert_eq!(world_hash(&slept), world_hash(&idled));
    let (t0, c0) = (slept.state().tick, slept.state().clock);
    assert_eq!(c0 / HOUR, 22);
    let night = DAY - c0 + 6 * HOUR;
    cmd(&mut slept, Command::Use);
    assert!(slept.drain_events().iter().any(|e| e.kind == EventKind::Rest), "she used the bed");
    // The night and then the step's own tick: the clock and the tick moved together.
    assert_eq!((slept.state().tick.0, slept.state().clock), (t0.0 + night + 1, 6 * HOUR + 1));
    for _ in 0..=night {
        idled.step(&StepInput::IDLE);
    }
    assert_eq!((idled.state().tick, idled.state().clock), (slept.state().tick, slept.state().clock));
    (slept, idled)
}

/// Decision 3: sleeping eight hours is, world for world, idling through them. The stream drew
/// the same draws (eight hours of skies, 48 marks of ecology), the skies and the ramps are the
/// same, the patches' pressure is the same, the consequence fired on the same tick, and the rats
/// stood up (or were held) on the same ticks under the same rules. What may differ is only what
/// moves: where anyone stands, who is awake, her body.
#[test]
fn a_night_slept_is_a_night_idled() {
    // Two kills: under the line by three o'clock, so both rats stand on their own tick, mid-gap.
    let (slept, idled) = the_same_night(2);
    assert_eq!(world_hash(&slept), world_hash(&idled));
    let rats = county_unit_ids(&slept, "rat_allotment_");
    assert!(rats.iter().take(2).all(|&r| alive(&slept, r)), "they stood up while she slept");
    let id = catalog().living.consequence_id("mine_quiet").unwrap();
    let fired = slept.state().journal.known.get(&FactKey::Consequence(id)).map(|k| k.since);
    // The night was eight hours less the tick the clock was set in; its first tick fired it.
    assert_eq!(fired, Some(Tick(slept.state().tick.0 - (8 * HOUR - 1))));
    assert!(
        slept.state().zone(ZoneId::County).unwrap().units.iter().any(|u| u.key == Some(sym(&slept, "mine_watcher")))
    );
    assert!(!slept.state().rumours.is_empty(), "the rumour started");

    // Four kills: held past their tick, looked at again every mark, still held at six.
    let (slept, idled) = the_same_night(4);
    assert_eq!(world_hash(&slept), world_hash(&idled));
    let z = slept.state().zone(ZoneId::County).unwrap();
    assert!(rats.iter().all(|&r| !alive(&slept, r)));
    for &(at, _) in z.sleeping_due.iter().filter(|e| rats.contains(&e.1)) {
        assert!(at > slept.state().tick && at.0 - slept.state().tick.0 <= MARK, "held to the next mark");
    }
    let p = z.pressure[area_ix(&slept, "allotments")];
    let pop = catalog().living.ecology_of(catalog().name_id("allotments").unwrap()).unwrap();
    assert!(p < 4 * pop.populations[0].weight && p >= pop.populations[0].hold, "{p}");

    // And the night is the same night through a save and a load, and a rebuild.
    let (bps, (x, y)) = a_county_with_a_bed();
    let mut a = Sim::new_game_with(bps.clone(), "Jane");
    place(&mut a, x, y, Facing::East);
    idle(&mut a, 5);
    let mut b = Sim::from_save_with(&a.save(), bps).expect("loads");
    b.rebuild_runtimes();
    cmd(&mut a, Command::Use);
    cmd(&mut b, Command::Use);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.state().clock, 6 * HOUR + 1);
}

/// A bed in the house is a conversation: alone, the world holds while she reads, and the night
/// still passes when she chooses to sleep (a frozen step). The tick moves with the clock, so a
/// timer set before she slept has run out when she wakes.
#[test]
fn the_house_bed_sleeps_to_six_in_a_frozen_step() {
    let mut s = new_game();
    tp(&mut s, ZoneId::House);
    assert!(walk_to_prop(&mut s, "julies_bed"));
    let (t0, c0) = (s.state().tick, s.state().clock);
    // A cooldown of an hour, set now: it is due long before morning.
    let body = s.state().players[0].unit;
    let until = t0.after(Tick(HOUR));
    s.state_mut().zone_mut(ZoneId::House).unwrap().unit_mut(body).unwrap().stop_until = until;
    cmd(&mut s, Command::Use);
    assert!(s.frozen(), "alone, reading");
    let t1 = s.state().tick;
    cmd(&mut s, Command::Choose { option: 0 });
    let gap = 24 * HOUR - c0 + 6 * HOUR;
    assert_eq!(s.state().clock, 6 * HOUR, "a frozen step: the night, and no tick after it");
    assert_eq!(s.state().tick, t1.after(Tick(gap)));
    assert!(s.state().tick > until, "the timer has run out");
    assert_eq!(s.state().day, 1);
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
    let fired = |id: &str| a.state().consequences_done.get(u32::from(catalog().living.consequence_id(id).unwrap().0));
    assert!(fired("allotments_thinned") && fired("mine_quiet"));
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

/// The owner (2026-10-01): the fire by the mine was drawn out while its words said it burned. A
/// fire's words are chosen by `speakerLit` and its picture by `View::light_showing` (the presenter
/// draws a prop's lit frame, its flame and its pool by it): the two are one rule, dry and in the
/// rain, for the fire by the mine mouth on three seeds.
#[test]
fn a_fires_words_say_what_it_is_drawn_as_in_the_rain_and_out_of_it() {
    use common::bot::talk_through;
    use jane_sim::state::Speaker;
    let cat = catalog();
    let douse = cat.story.prop(cat.story.prop_id("campfire").unwrap()).douse.unwrap();
    for seed in [SEED, 1, 2] {
        let b = if seed == SEED { bps() } else { Blueprints::build(seed).expect("the seed builds") };
        let mut s = Sim::new_game_with(b, "Jane");
        tp(&mut s, ZoneId::County);
        let fire = prop(&s, "mine_fire");
        let (fx, fy) = (i32::from(fire.cell.x), i32::from(fire.cell.y));
        let r = region_ix(s.runtime(ZoneId::County).unwrap().region_at(fx, fy));
        for wet in [false, true] {
            if wet {
                let now = s.state().tick;
                s.state_mut().weather[r] = sky(WeatherKind::Rain, now);
                s.state_mut().zone_mut(ZoneId::County).unwrap().wetness[r] = 255;
                s.step(&StepInput::IDLE);
            }
            // Beside it, facing it: west of it, else east.
            let sides = [(fx - 1, fy + 1, Facing::East), (fx + 2, fy + 1, Facing::West)];
            let talked = sides.into_iter().any(|(x, y, f)| {
                place(&mut s, x, y, f);
                cmd(&mut s, Command::Use);
                s.view(Seat(0)).unwrap().dialogue().is_some_and(|d| d.speaker == Speaker::Prop(fire.id))
            });
            assert!(talked, "seed {seed}: she could not talk to the fire at {fx},{fy}");
            let v = s.view(Seat(0)).unwrap();
            let lit = v.light_showing(&prop(&s, "mine_fire")).is_some();
            assert_eq!(lit, !wet || s.state().zone(ZoneId::County).unwrap().wetness[r] < douse, "seed {seed}");
            let node = v.dialogue().unwrap().node.expect("its words").id;
            assert_eq!(node == "fire", lit, "seed {seed}, wet {wet}: it says {node:?} and is drawn lit {lit}");
            talk_through(&mut s, &[]);
        }
    }
}
