//! `solve::resolve`: a re-solve with one more thing taken away goes on from the base solve's
//! trail, and must report exactly what a solve from the start reports, down to the floods run
//! and the pass that first reached each cell. Asked of every generated dungeon, for what C1 and
//! C6 take away (key tags, spells, flags, controls, gates kept shut) and for a spread of props.

use jane_core::ZoneId;
use jane_core::action::FlagKey;
use jane_core::ids::Key;
use jane_data::MissionEdgeKind;
use jane_world::solve::{Grant, KeyTag, Options, resolve, solve, solve_kept};

#[test]
fn a_resolve_reports_what_a_solve_from_the_start_does() {
    let cat = jane_data::catalog();
    let mut asked = 0;
    for z in ZoneId::ALL {
        let Some(m) = cat.dungeons.mission_of(z) else { continue };
        for seed in 1..=2 {
            let b = jane_world::dungeon::build(z, seed);
            let (bp, info) = (&b.blueprint, &b.info);
            let rules = jane_world::dungeon::checks::rules_of(m);
            let base = Options { trace: true, ..Options::default() };
            let (report, trail) = solve_kept(bp, &rules, &base);
            assert_eq!(report, solve(bp, &rules, &base), "{} {seed}: keeping a trail changed the solve", z.name());
            let mut grants: Vec<Grant> = Vec::new();
            for e in m.edges {
                match e.kind {
                    MissionEdgeKind::Key { tag, .. } => grants.push(Grant::Key(KeyTag::Tag(Key::Name(tag)))),
                    MissionEdgeKind::Verb { verb, .. } => grants.push(Grant::Verb(verb)),
                    MissionEdgeKind::Oneway { flag, .. } => grants.push(Grant::Flag(FlagKey::Named(Key::Name(flag)))),
                    _ => {}
                }
            }
            grants.extend(info.controls.iter().map(|c| Grant::Prop(c.prop)));
            grants.extend(info.lockins.iter().map(|l| Grant::Shut(l.gate)));
            grants.extend(info.locks.iter().map(|l| Grant::Shut(l.prop)));
            grants.extend(bp.props.iter().step_by(9).flat_map(|p| [Grant::Prop(p.key), Grant::Shut(p.key)]));
            grants.extend(rules.given_keys.iter().map(|&k| Grant::Key(KeyTag::Tag(k))));
            for (i, g) in grants.iter().enumerate() {
                let mut opts = base.without(*g);
                // Two at once now and then, as C1 takes every control of a state away.
                if let Some(h) = grants.get(i + 3).filter(|_| i % 4 == 0) {
                    opts = opts.without(*h);
                }
                let want = solve(bp, &rules, &opts);
                assert!(resolve(bp, &rules, &base, &report, &trail, &opts) == want, "{} {seed}: {g:?}", z.name());
                asked += 1;
            }
        }
    }
    assert!(asked > 100, "only {asked} re-solves asked");
}
