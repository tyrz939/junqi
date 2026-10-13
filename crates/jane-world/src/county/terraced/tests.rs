use super::macro_check::{check, span_fault};
use super::macro_plan::{GateKind, Role, Wet, idx};
use super::source::{county, write_places, write_plan, write_routes};
use jane_core::blueprint::SpanKind;

/// The county's source reads, and written again is the same text (`\r\n` read as `\n`).
#[test]
fn the_source_reads_and_writes_back_the_same() {
    let src = county();
    let same = |a: &str, b: String| a.replace("\r\n", "\n") == b;
    assert!(same(super::source::PLAN, write_plan(&src.plan, src.salt)), "plan.txt");
    assert!(same(super::source::PLACES, write_places(&src.places)), "places.txt");
    assert!(same(super::source::ROUTES, write_routes(&src.routes, &src.plan)), "routes.txt");
}

/// The source's plan passes the macro check, and its water and spans are sensible: sites dry, no road on
/// standing water, every span where one is needed and of its gate's kind, no road wading the river.
#[test]
fn the_plan_is_clean() {
    let p = county().plan;
    let issues: Vec<_> = check(&p).iter().map(|i| format!("{}: {}", i.code, i.text)).collect();
    assert!(issues.is_empty(), "{issues:?}");
    for s in &p.sites {
        assert_eq!(p.wet[idx(i32::from(s.pos.0), i32::from(s.pos.1))], Wet::Dry, "{}", s.name);
    }
    for r in &p.routes {
        assert!(r.pts.iter().all(|q| !p.wet[idx(i32::from(q.0), i32::from(q.1))].body()), "road on water");
    }
    for c in &p.crossings {
        assert_eq!(span_fault(&p, c), None, "{:?} at {:?}", c.kind, c.at);
        if let Some(gi) = c.gate {
            let g = &p.gates[gi as usize];
            let want = match g.name {
                "The Castle Viaduct" => SpanKind::Viaduct,
                "The Pipe Walkway" => SpanKind::Walkway,
                "The Footbridge" => SpanKind::Footbridge,
                "The Rail Arch" => SpanKind::Rail,
                n => panic!("crossing on {n}"),
            };
            assert_eq!(c.kind, want, "{}", g.name);
        }
        if c.route.is_some() {
            assert_eq!(c.kind, SpanKind::Bridge);
        }
    }
}

/// The check names what an edit breaks: every way out of a ledge's landing district cut is a pit; the
/// Lake raised off the floor is caught.
#[test]
fn the_check_catches_a_pit_and_a_high_lake() {
    let mut p = county().plan;
    let k = p.gates.iter().position(|g| g.kind == GateKind::Ledge).expect("a ledge");
    let land = p.gates[k].db;
    p.gates.retain(|g| g.kind == GateKind::Ledge || (g.da != land && g.db != land));
    let codes: Vec<_> = check(&p).iter().map(|i| i.code).collect();
    assert!(codes.contains(&"pit") || codes.contains(&"unreached"), "{codes:?}");
    let mut p = county().plan;
    p.districts[Role::Lake.id() as usize].level = 1;
    let codes: Vec<_> = check(&p).iter().map(|i| i.code).collect();
    assert!(codes.contains(&"lake_low"), "{codes:?}");
}
