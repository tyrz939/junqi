use super::macro_check::{Issue, check, span_fault};
use super::macro_plan::{
    ARCH_LOCK, ATTEMPTS, D4, GateKind, MH, MW, MacroPlan, Reg, Role, SHORE, Wet, build, idx, plan, room_map,
};
use jane_core::blueprint::SpanKind;

const TRIES_PER_SEED: u32 = 20;

#[test]
fn every_seed_has_a_clean_plan() {
    let mut bad = Vec::new();
    let mut tries = 0u32;
    for seed in 1..=256u32 {
        let (p, issues) = plan(seed);
        tries += u32::from(p.attempt) + 1;
        if !issues.is_empty() {
            bad.push((seed, issues.iter().map(|i: &Issue| format!("{}: {}", i.code, i.text)).collect::<Vec<_>>()));
        }
    }
    assert!(bad.is_empty(), "seeds failing all {ATTEMPTS} attempts: {bad:?}");
    assert!(tries < 256 * TRIES_PER_SEED, "too many re-rolls: {tries} tries for 256 seeds");
}

#[test]
fn plan_is_deterministic_and_a_pit_is_caught() {
    let (a, _) = plan(7);
    let b = build(7, a.attempt);
    assert_eq!(a.district_of, b.district_of);
    assert_eq!(a.gates.len(), b.gates.len());
    assert_eq!(a.wet, b.wet);
    assert_eq!(a.river_in, b.river_in);
    assert_eq!(a.river_out, b.river_out);
    assert_eq!(a.fall, b.fall);
    assert_eq!(a.crossings.len(), b.crossings.len());
    for (x, y) in a.crossings.iter().zip(&b.crossings) {
        assert_eq!((x.at, x.kind), (y.at, y.kind));
    }
    // Every way out of a ledge's landing district cut: a pit the check must name.
    let mut p = a;
    let k = p.gates.iter().position(|g| g.kind == GateKind::Ledge).expect("a ledge");
    let land = p.gates[k].db;
    p.gates.retain(|g| g.kind == GateKind::Ledge || (g.da != land && g.db != land));
    let codes: Vec<_> = check(&p).iter().map(|i| i.code).collect();
    assert!(codes.contains(&"pit") || codes.contains(&"unreached"), "{codes:?}");
}

fn clean(seed: u32) -> MacroPlan {
    let (p, issues) = plan(seed);
    assert!(issues.is_empty(), "seed {seed}: {:?}", issues.iter().map(|i| i.code).collect::<Vec<_>>());
    p
}

#[test]
fn lake_is_one_in_five() {
    let (mut lakes, mut late) = (0, 0);
    for seed in 1..=64u32 {
        let p = clean(seed);
        lakes += i32::from(p.arch == "lake");
        late += i32::from(p.attempt >= ARCH_LOCK);
    }
    println!("lake archetype in {lakes} of 64 seeds; {late} on the free-draw fallback");
    assert!((9..=19).contains(&lakes), "{lakes} lake seeds of 64");
    assert!(late <= 2, "{late} seeds fell back to the free archetype draw");
}

fn cheb(a: (u8, u8), b: (u8, u8)) -> i32 {
    (i32::from(a.0) - i32::from(b.0)).abs().max((i32::from(a.1) - i32::from(b.1)).abs())
}

#[test]
fn water_is_sensible() {
    let cnt = |p: &MacroPlan, w: Wet| p.wet.iter().filter(|&&q| q == w).count() as i32;
    for seed in 1..=64u32 {
        let p = clean(seed);
        let ld = &p.districts[Role::Lake.id() as usize];
        let lake = cnt(&p, Wet::Lake);
        let share = lake * 1000 / ld.area as i32;
        assert!((80..=150).contains(&share) && (32..=160).contains(&lake), "seed {seed}: lake {lake} {share} permille");
        assert!((300..=1200).contains(&(ld.area as i32)), "seed {seed}: Lake district {}", ld.area);
        let room = room_map(&p, Role::Lake.id());
        assert!((0..p.wet.len()).all(|c| p.wet[c] != Wet::Lake || room[c] >= SHORE), "seed {seed}: lake room");
        assert_eq!(ld.level, 0);
        let (ri, ro) = (&p.river_in, &p.river_out);
        assert!(!ri.is_empty() && !ro.is_empty(), "seed {seed}: river");
        let bank = |q: (u8, u8)| {
            let (x, y) = (i32::from(q.0), i32::from(q.1));
            p.reg_at(x, y) == Reg::Waters
                && D4.iter().any(|&(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..MW).contains(&nx) && (0..MH).contains(&ny) && p.reg_at(nx, ny) == Reg::Lowfields
                })
        };
        let next_lake = |q: (u8, u8)| {
            D4.iter().any(|&(dx, dy)| {
                let (nx, ny) = (i32::from(q.0) + dx, i32::from(q.1) + dy);
                (0..MW).contains(&nx) && (0..MH).contains(&ny) && p.wet[idx(nx, ny)] == Wet::Lake
            })
        };
        assert!(bank(ri[0]) && next_lake(ri[ri.len() - 1]), "seed {seed}: inflow ends");
        let last = ro[ro.len() - 1];
        let on_edge = last.0 == 0 || last.1 == 0 || i32::from(last.0) == MW - 1 || i32::from(last.1) == MH - 1;
        assert!(next_lake(ro[0]) && on_edge, "seed {seed}: outflow ends");
        for q in ri.iter().chain(ro) {
            let (x, y) = (i32::from(q.0), i32::from(q.1));
            assert!(p.reg_at(x, y) != Reg::Works && p.level_at(x, y) <= 1, "seed {seed}: river on the upland");
        }
        let bodies = lake + cnt(&p, Wet::Mere) + cnt(&p, Wet::Pond);
        assert!((40..=260).contains(&bodies), "seed {seed}: {bodies} body cells");
        for s in &p.sites {
            assert_eq!(p.wet[idx(i32::from(s.pos.0), i32::from(s.pos.1))], Wet::Dry, "seed {seed}: {}", s.name);
        }
        for r in &p.routes {
            assert!(
                r.pts.iter().all(|q| !p.wet[idx(i32::from(q.0), i32::from(q.1))].body()),
                "seed {seed}: road on water"
            );
        }
    }
}

#[test]
fn spans_only_where_needed() {
    for seed in 1..=64u32 {
        let p = clean(seed);
        for c in &p.crossings {
            assert_eq!(span_fault(&p, c), None, "seed {seed}: {:?} at {:?}", c.kind, c.at);
            if let Some(gi) = c.gate {
                let g = &p.gates[gi as usize];
                let want = match g.name {
                    "The Castle Viaduct" => SpanKind::Viaduct,
                    "The Pipe Walkway" => SpanKind::Walkway,
                    "The Footbridge" => SpanKind::Footbridge,
                    "The Rail Arch" => SpanKind::Rail,
                    n => panic!("seed {seed}: crossing on {n}"),
                };
                assert_eq!(c.kind, want, "seed {seed}: {}", g.name);
            }
            if c.route.is_some() {
                assert_eq!(c.kind, SpanKind::Bridge);
            }
        }
        for r in &p.routes {
            for &q in &r.pts {
                if p.wet[idx(i32::from(q.0), i32::from(q.1))] == Wet::River {
                    assert!(
                        p.crossings.iter().any(|c| cheb(c.at, q) <= 1),
                        "seed {seed}: a road wades the river at {q:?}"
                    );
                }
            }
        }
        let bridges = p.crossings.iter().filter(|c| c.route.is_some()).count();
        assert!(bridges <= 8, "seed {seed}: {bridges} road bridges");
    }
}

#[test]
fn lake_sits_low() {
    let (mut p, _) = plan(7);
    p.districts[Role::Lake.id() as usize].level = 1;
    let codes: Vec<_> = check(&p).iter().map(|i| i.code).collect();
    assert!(codes.contains(&"lake_low"), "{codes:?}");
}

#[test]
#[ignore = "prints the re-roll statistics"]
fn stats() {
    use std::collections::BTreeMap;
    let mut hist: BTreeMap<(&str, &str), u32> = BTreeMap::new();
    let mut draws: BTreeMap<&str, u32> = BTreeMap::new();
    let mut rolls: BTreeMap<&str, u32> = BTreeMap::new();
    let mut clean: BTreeMap<&str, u32> = BTreeMap::new();
    let (mut lake_pm, mut lake_area, mut water, mut bridges, mut n) = (0i64, 0i64, 0i64, 0i64, 0i64);
    for seed in 1..=24u32 {
        for a in 0..80 {
            let (_, _, _, arch) = super::macro_plan::screen_info(seed, a);
            *draws.entry(arch).or_insert(0) += 1;
            if !super::macro_plan::region_screen(seed, a) {
                continue;
            }
            let p = build(seed, a);
            let issues = check(&p);
            *rolls.entry(p.arch).or_insert(0) += 1;
            if issues.is_empty() {
                *clean.entry(p.arch).or_insert(0) += 1;
                let cnt = |w: Wet| p.wet.iter().filter(|&&q| q == w).count() as i64;
                let area = i64::from(p.districts[Role::Lake.id() as usize].area);
                lake_pm += cnt(Wet::Lake) * 1000 / area;
                lake_area += area;
                water += cnt(Wet::Lake) + cnt(Wet::Mere) + cnt(Wet::Pond);
                bridges += p.crossings.iter().filter(|c| c.route.is_some()).count() as i64;
                n += 1;
            }
            for i in issues {
                *hist.entry((p.arch, i.code)).or_insert(0) += 1;
            }
        }
    }
    println!("draws {draws:?}\nscreened {rolls:?}\nclean {clean:?}");
    let n = n.max(1);
    println!(
        "clean rolls {n}: mean lake {} permille, mean Lake district {} cells, mean water {} cells, bridges/seed {}.{}",
        lake_pm / n,
        lake_area / n,
        water / n,
        bridges / n,
        bridges * 10 / n % 10
    );
    for (k, v) in &hist {
        println!("H {k:?} {v}");
    }
}

#[test]
#[ignore = "prints the seed census: archetypes, fallbacks, failures and the water"]
fn census() {
    for (lo, hi) in [(1u32, 64u32), (1, 256)] {
        let (mut lakes, mut late, mut bad, mut n) = (0, 0, 0, 0i64);
        let (mut pm, mut lk, mut mr, mut pd, mut br, mut tries) = (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
        for seed in lo..=hi {
            let (p, issues) = plan(seed);
            lakes += i32::from(p.arch == "lake");
            late += i32::from(p.attempt >= ARCH_LOCK);
            bad += i32::from(!issues.is_empty());
            let cnt = |w: Wet| p.wet.iter().filter(|&&q| q == w).count() as i64;
            let area = i64::from(p.districts[Role::Lake.id() as usize].area);
            pm += cnt(Wet::Lake) * 1000 / area;
            lk += cnt(Wet::Lake);
            mr += cnt(Wet::Mere);
            pd += cnt(Wet::Pond);
            br += p.crossings.iter().filter(|c| c.route.is_some()).count() as i64;
            tries += i64::from(p.attempt) + 1;
            n += 1;
        }
        println!(
            "CENSUS {lo}..{hi}: lake archetype {lakes} ({} permille), fallbacks {late}, unclean {bad}; mean lake {} cells {} permille of its district, mere {}, ponds {}, water {} cells of 9216; road bridges/seed {}.{}; tries/seed {}",
            i64::from(lakes) * 1000 / n,
            lk / n,
            pm / n,
            mr / n,
            pd / n,
            (lk + mr + pd) / n,
            br / n,
            br * 10 / n % 10,
            tries / n
        );
    }
}
