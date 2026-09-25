//! PORT.md §6.m stages 3 and 4: the skeleton's sites, roads, rail, patches, lamps, small places,
//! anchors and threat. Carries `jane/test/skeleton.test.ts` whole, plus stage 3's floors: every
//! site row holds by road, one to three bridges, every road site reachable, and the attempt budget
//! (mean at most 3, p99 at most 13). `SEEDS` seeds (64 by default), the TypeScript's seed list.

mod common;

use std::sync::OnceLock;
// A test may read the clock to hold a speed floor; worldgen never does.
#[allow(clippy::disallowed_types)]
use std::time::Instant;

use jane_data::{DistBy, catalog};
use jane_world::skeleton::roads::{FAR, road_distances};
use jane_world::skeleton::{
    MAX_ATTEMPTS, ROAD, ROAD_LIT, Region, SKEL_H, SKEL_W, Skeleton, SkeletonError, SkeletonRows, Water, build_skeleton,
    failures, metres_sq, region_name, skeleton, threat_at,
};

fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| i.wrapping_mul(2654435761)).collect()
}

fn all() -> &'static [Skeleton] {
    static ALL: OnceLock<Vec<Skeleton>> = OnceLock::new();
    ALL.get_or_init(|| seed_list().into_iter().map(|s| skeleton(s).expect("the catalog's rows build")).collect())
}

fn named(s: &Skeleton, id: &str) -> (i32, i32) {
    let x = s.sites.iter().find(|x| x.def.id == id).unwrap_or_else(|| panic!("no {id}"));
    (x.mx, x.my)
}

/// `n`-th percentile of a sorted list, nearest rank.
fn percentile(sorted: &[u32], n: usize) -> u32 {
    sorted[(sorted.len() * n).div_ceil(100).saturating_sub(1)]
}

#[test]
fn satisfies_every_row_of_sites_on_every_seed_within_the_attempt_budget() {
    let all = all();
    let why: Vec<String> =
        all.iter().filter(|s| !s.ok).take(5).map(|s| format!("{}: {}", s.seed, failures(s))).collect();
    assert!(why.is_empty(), "{why:#?}");
    assert!(all.iter().all(|s| s.attempt < MAX_ATTEMPTS));
    // PORT.md §6.j: mean attempts at most 3, p99 at most 13.
    let mut tries: Vec<u32> = all.iter().map(|s| s.stats.attempts).collect();
    tries.sort();
    let total: u32 = tries.iter().sum();
    let n = tries.len() as u32;
    let km: Vec<u32> = all.iter().map(|s| s.roads.iter().map(|r| r.metres).sum::<u32>()).collect();
    let areas: usize = all.iter().map(|s| s.areas.len()).sum();
    let mut failed = jane_world::skeleton::Failed::default();
    for s in all {
        let f = s.stats.failed;
        failed.site += f.site;
        failed.off_road += f.off_road;
        failed.road_checks += f.road_checks;
        failed.area += f.area;
        failed.anchor += f.anchor;
        failed.checks += f.checks;
    }
    eprintln!(
        "SOAK {n} seeds: attempts mean {}.{:02} p99 {} worst {}; road m mean {} min {} max {}; patches mean {}/{}; thrown away {failed:?}",
        total / n,
        total * 100 / n % 100,
        percentile(&tries, 99),
        tries[tries.len() - 1],
        km.iter().sum::<u32>() / n,
        km.iter().min().unwrap(),
        km.iter().max().unwrap(),
        areas / n as usize,
        catalog().county.areas.len(),
    );
    assert!(total <= 3 * n, "mean attempts {total} / {n}");
    assert!(percentile(&tries, 99) <= 13, "p99 attempts {}", percentile(&tries, 99));
}

#[test]
fn is_the_same_county_for_the_same_seed_and_a_different_one_for_a_different_seed() {
    let all = all();
    let i = 7.min(all.len() - 1);
    let again = skeleton(all[i].seed).unwrap();
    let spots = |s: &Skeleton| s.sites.iter().map(|x| (x.row, x.mx, x.my)).collect::<Vec<_>>();
    assert_eq!(spots(&again), spots(&all[i]));
    assert_eq!(again.road, all[i].road);
    assert_eq!(again.threat, all[i].threat);
    assert_eq!(again.pois, all[i].pois);
    assert_eq!(again.anchors, all[i].anchors);
    assert_eq!(again.rail, all[i].rail);
    assert_eq!(again.stats, all[i].stats);
    let mut layouts: Vec<Vec<(u8, i32, i32)>> = all.iter().map(spots).collect();
    layouts.sort();
    layouts.dedup();
    assert_eq!(layouts.len(), all.len());
}

#[test]
fn places_every_story_site_in_its_region() {
    let rows = catalog().county.sites;
    for s in all() {
        let ids: Vec<&str> = s.sites.iter().map(|x| x.def.id).collect();
        let want: Vec<&str> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, want);
        for x in &s.sites {
            assert_eq!(s.region_at(x.mx, x.my), x.def.region, "seed {}: {}", s.seed, x.def.id);
        }
    }
}

/// Sept 24: the county went from 3.6 x 2 km to 2 km square. The far shore is still the far end of
/// the county, a five-minute walk rather than ten.
#[test]
fn is_five_minutes_across_by_road_the_far_shore_is_at_least_2200_m_from_the_platform() {
    for s in all() {
        let c = s.checks.iter().find(|c| c.rule.starts_with("lake_statue 2200")).expect("the far-shore rule");
        assert!(c.ok, "{} {}", s.seed, c.detail);
    }
}

#[test]
fn puts_the_school_on_high_ground_north_of_the_town_where_it_can_loom() {
    for s in all() {
        let school = named(s, "school");
        let town = named(s, "town");
        assert!(school.1 < town.1, "seed {}", s.seed);
        let h = |(x, y): (i32, i32)| i32::from(s.terrain.height.read(x, y, 0));
        assert!(h(school) > h(town) + 40, "seed {}: school {} town {}", s.seed, h(school), h(town));
    }
}

#[test]
fn danger_is_a_map_not_a_gradient_every_region_has_patches() {
    for s in all() {
        for r in [Region::Lowfields, Region::Waters, Region::Works] {
            let here = s.areas.iter().filter(|a| s.region_at(a.mx, a.my) == r).count();
            assert!(here >= 3, "seed {} {}: {here} patches", s.seed, region_name(r));
        }
        // A haven is a haven, and the first walk is lit.
        let town = named(s, "town");
        assert_eq!(s.threat.read(town.0, town.1, 9), 0);
        let station = jane_world::skeleton::RoadEnd::Site(u16::from(s.named.station));
        let first = s.roads.iter().find(|r| r.from == station).expect("a road from the station");
        for &(x, y) in &first.cells {
            assert!(s.road.read(x, y, 0) & ROAD_LIT != 0, "seed {}: dark at ({x},{y})", s.seed);
        }
    }
}

#[test]
fn night_raises_everything_outside_lamplight_more_in_the_works_and_never_a_haven() {
    let s = &all()[3.min(all().len() - 1)];
    let town = named(s, "town");
    assert_eq!(threat_at(s, town.0, town.1, true), 0);
    let (mut lit, mut dark) = (None, None);
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let r = s.road.read(x, y, 0);
            if r & ROAD == 0 || s.threat.read(x, y, 0) == 0 {
                continue;
            }
            if r & ROAD_LIT != 0 && lit.is_none() {
                lit = Some((x, y));
            }
            if r & ROAD_LIT == 0 && dark.is_none() {
                dark = Some((x, y));
            }
        }
    }
    let (lx, ly) = lit.expect("a lit road");
    let (dx, dy) = dark.expect("a dark road");
    assert_eq!(threat_at(s, lx, ly, true), s.threat.read(lx, ly, 0));
    assert!(threat_at(s, dx, dy, true) > s.threat.read(dx, dy, 0));
    assert_eq!(threat_at(s, dx, dy, false), s.threat.read(dx, dy, 0));
    // In the Works the night adds two off the road.
    let works = (0..SKEL_H)
        .flat_map(|y| (0..SKEL_W).map(move |x| (x, y)))
        .find(|&(x, y)| s.region_at(x, y) == Region::Works && s.road.read(x, y, 0) == 0 && s.threat.read(x, y, 0) <= 4)
        .expect("open ground in the Works");
    assert_eq!(threat_at(s, works.0, works.1, true), s.threat.read(works.0, works.1, 0) + 2);
}

#[test]
fn the_railway_runs_through_the_halt_and_off_the_map_at_both_ends_over_the_river_clear_of_every_place() {
    let all = all();
    for s in all {
        let rail = &s.rail;
        let station = named(s, "station");
        assert!(rail.contains(&station), "{}: through the halt", s.seed);
        let (a, b) = (rail[0], rail[rail.len() - 1]);
        assert_eq!(a.1, SKEL_H - 1, "{}: in from the south edge", s.seed);
        assert!(b.0 == SKEL_W - 1 || b.1 == 0, "{}: out at the east or north edge", s.seed);
        // One piece: every cell touches the next.
        for w in rail.windows(2) {
            assert_eq!((w[0].0 - w[1].0).abs().max((w[0].1 - w[1].1).abs()), 1, "{}: a gap", s.seed);
        }
        for x in s.sites.iter().filter(|x| x.def.id != "station") {
            // Up the west fence the line is in column 0, which no chunk reaches.
            for &(cx, cy) in rail.iter().filter(|c| c.0 != 0) {
                assert!(metres_sq(cx, cy, x.mx, x.my) > 90 * 90, "{}: rail by {}", s.seed, x.def.id);
            }
        }
        for p in &s.pois {
            assert!(!rail.contains(&(p.mx, p.my)), "{}: a small place on the line", s.seed);
        }
        for &(x, y) in rail {
            assert_ne!(s.terrain.water.read(x, y, Water::Dry), Water::Lake, "{}: the rail crosses the lake", s.seed);
        }
    }
    // Most seeds carry it east across the Works and over the river.
    let east: Vec<&Skeleton> = all.iter().filter(|s| s.rail[s.rail.len() - 1].0 == SKEL_W - 1).collect();
    assert!(east.len() * 10 > all.len() * 9, "{} of {} run east", east.len(), all.len());
    for s in east {
        assert!(s.rail.iter().any(|&(x, y)| s.terrain.water.read(x, y, Water::Dry) == Water::River), "{}", s.seed);
    }
}

#[test]
#[allow(clippy::disallowed_types)]
fn crosses_the_river_in_few_places_and_is_quick_enough_to_run_on_a_title_screen() {
    for s in all() {
        let b = s.bridges();
        assert!((1..=3).contains(&b), "seed {}: {b} bridges", s.seed);
    }
    let t0 = Instant::now();
    let mut attempts = 0;
    for i in 0..10 {
        attempts += skeleton(9000 + i).unwrap().stats.attempts;
    }
    let ms = t0.elapsed().as_millis() / 10;
    eprintln!("speed: {ms} ms a skeleton, {attempts} attempts over 10 seeds");
    // Generous: debug builds and a busy machine. Release is far quicker (the report records it).
    assert!(ms < 900, "{ms} ms a skeleton");
}

/// Stage 3: every road rule holds when measured again from scratch along the built network, and
/// every site on the network is reachable from the platform.
#[test]
fn every_site_row_holds_by_road_and_every_road_site_is_reachable() {
    for s in all() {
        let station = named(s, "station");
        let from_station = road_distances(&s.road, station.0, station.1);
        for x in &s.sites {
            if x.def.on_road {
                assert_ne!(from_station.read(x.mx, x.my, FAR), FAR, "seed {}: {} off the network", s.seed, x.def.id);
            }
            for rule in x.def.dist {
                let o = s.site(rule.to);
                let tenths = match rule.by {
                    DistBy::Road => road_distances(&s.road, o.mx, o.my).read(x.mx, x.my, FAR),
                    DistBy::Line => jane_core::num::isqrt(metres_sq(x.mx, x.my, o.mx, o.my) as u64 * 100),
                };
                assert_ne!(tenths, FAR, "seed {}: {} to {}", s.seed, x.def.id, o.def.id);
                let m = tenths / 10;
                assert!(
                    rule.min.is_none_or(|lo| m >= u32::from(lo)),
                    "seed {}: {} {m} m from {}",
                    s.seed,
                    x.def.id,
                    o.def.id
                );
                assert!(
                    rule.max.is_none_or(|hi| tenths <= u32::from(hi) * 10),
                    "seed {}: {} {m} m from {}",
                    s.seed,
                    x.def.id,
                    o.def.id
                );
            }
        }
        // Sites off the network are off it: not seen from a road.
        for x in s.sites.iter().filter(|x| x.def.at.off_road.is_some()) {
            let off = i64::from(x.def.at.off_road.unwrap());
            for y in 0..SKEL_H {
                for xx in 0..SKEL_W {
                    if s.road.read(xx, y, 0) & ROAD != 0 {
                        assert!(
                            metres_sq(xx, y, x.mx, x.my) >= (off - 23).max(0).pow(2),
                            "seed {}: road by {}",
                            s.seed,
                            x.def.id
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_required_patch_and_every_anchor_is_placed() {
    let c = &catalog().county;
    for s in all() {
        for (i, row) in c.areas.iter().enumerate() {
            if row.required {
                assert!(s.area(i as u8).is_some(), "seed {}: no {}", s.seed, catalog().name(row.id));
            }
        }
        assert_eq!(s.anchors.len(), c.anchors.len());
        for (i, a) in s.anchors.iter().enumerate() {
            assert_eq!(usize::from(a.row), i);
            assert!(s.pois.iter().any(|p| p.anchor == Some(a.row) && (p.mx, p.my) == (a.mx, a.my)));
        }
    }
}

#[test]
fn rows_are_a_parameter_and_a_missing_named_site_is_an_error() {
    let c = &catalog().county;
    let rows = SkeletonRows::from(c);
    let no_school: Vec<_> = c.sites.iter().copied().filter(|s| s.id != "school").collect();
    let err = build_skeleton(1, &SkeletonRows { sites: &no_school, ..rows }, 0).unwrap_err();
    assert_eq!(err, SkeletonError::MissingSite("school"));
    // A site that hangs off a later one is refused before a cell is laid, not a panic halfway through.
    let mut later = c.sites.to_vec();
    later[1].road_from = Some(5);
    let err = build_skeleton(1, &SkeletonRows { sites: &later, ..rows }, 0).unwrap_err();
    assert!(matches!(err, SkeletonError::BadRow(_)), "{err}");
    // Fewer small places to roll: the same seed still builds, and the kinds it rolls are the rows given.
    let wells: Vec<_> = c.pois.iter().copied().filter(|p| catalog().name(p.kind) != "well").collect();
    let s = build_skeleton(1, &SkeletonRows { pois: &wells, ..rows }, 0).unwrap();
    let well = catalog().name_id("well");
    assert!(s.pois.iter().filter(|p| p.anchor.is_none()).all(|p| p.kind != well));
    // `from` asks for the next attempt on.
    let first = build_skeleton(1, &rows, 0).unwrap();
    let next = build_skeleton(1, &rows, first.attempt + 1).unwrap();
    assert!(next.attempt > first.attempt);
}
