use super::macro_check::{Issue, check};
use super::macro_plan::{ATTEMPTS, GateKind, build, plan};

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
    assert!(tries < 256 * 16, "too many re-rolls: {tries} tries for 256 seeds");
}

#[test]
fn plan_is_deterministic_and_a_pit_is_caught() {
    let (a, _) = plan(7);
    let b = build(7, a.attempt);
    assert_eq!(a.district_of, b.district_of);
    assert_eq!(a.gates.len(), b.gates.len());
    // Every way out of a ledge's landing district cut: a pit the check must name.
    let mut p = a;
    let k = p.gates.iter().position(|g| g.kind == GateKind::Ledge).expect("a ledge");
    let land = p.gates[k].db;
    p.gates.retain(|g| g.kind == GateKind::Ledge || (g.da != land && g.db != land));
    let codes: Vec<_> = check(&p).iter().map(|i| i.code).collect();
    assert!(codes.contains(&"pit") || codes.contains(&"unreached"), "{codes:?}");
}

#[test]
#[ignore = "prints the re-roll statistics"]
fn stats() {
    use std::collections::BTreeMap;
    let mut hist: BTreeMap<(&str, &str), u32> = BTreeMap::new();
    let (mut rolls, mut clean): (BTreeMap<&str, u32>, BTreeMap<&str, u32>) = (BTreeMap::new(), BTreeMap::new());
    for seed in 1..=24u32 {
        for a in 0..80 {
            if !super::macro_plan::region_screen(seed, a) {
                continue;
            }
            let p = build(seed, a);
            let issues = check(&p);
            *rolls.entry(p.arch).or_insert(0) += 1;
            if issues.is_empty() {
                *clean.entry(p.arch).or_insert(0) += 1;
            }
            for i in issues {
                *hist.entry((p.arch, i.code)).or_insert(0) += 1;
            }
        }
    }
    println!("rolls {rolls:?}\nclean {clean:?}");
    for (k, v) in &hist {
        println!("H {k:?} {v}");
    }
}
