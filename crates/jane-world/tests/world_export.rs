//! Phase A scaffolding: export the pinned county's source, and prove the source builds the same county.

use jane_world::county::terraced::CHOSEN;
use jane_world::county::terraced::build::{build, build_pinned, export_pinned};
use jane_world::county::terraced::source::{parse, write_places, write_plan, write_routes};

fn dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/world/county")
}

#[test]
#[ignore = "writes data/world/county from the pinned map's searches"]
fn export_the_pinned_county() {
    let (plan, places, routes) = export_pinned(CHOSEN).expect("the pinned map");
    std::fs::create_dir_all(dir()).unwrap();
    std::fs::write(dir().join("plan.txt"), plan).unwrap();
    std::fs::write(dir().join("places.txt"), places).unwrap();
    std::fs::write(dir().join("routes.txt"), routes).unwrap();
}

#[test]
#[ignore = "builds the county twice"]
fn the_source_builds_the_pinned_county() {
    let read = |f: &str| std::fs::read_to_string(dir().join(f)).unwrap().replace("\r\n", "\n");
    let (plan, places, routes) = (read("plan.txt"), read("places.txt"), read("routes.txt"));
    let src = parse(&plan, &places, &routes).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(write_plan(&src.plan, src.salt), plan, "plan.txt round trip");
    assert_eq!(write_places(&src.places), places, "places.txt round trip");
    assert_eq!(write_routes(&src.routes, &src.plan), routes, "routes.txt round trip");
    let t = std::time::Instant::now();
    let a = build_pinned(CHOSEN, &mut |_| {}, false).expect("the pinned map").bp;
    let ta = t.elapsed();
    let b = build(&src, &mut |_| {}, false).unwrap_or_else(|e| panic!("{e:?}")).bp;
    println!("pinned {ta:?}, source {:?}", t.elapsed() - ta);
    assert_eq!(jane_world::hash::hash(&a), jane_world::hash::hash(&b), "hash");
    assert!(a == b, "blueprints differ");
}
