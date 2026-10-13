//! The committed county (`assets/world/county.bp`) is its source (`data/world/county/`) baked: a source
//! edited and not baked again fails here. `cargo jane bake-world` bakes it.

use std::path::Path;

use jane_core::ZoneId;
use jane_sim::baked::{self, COUNTY};

fn source(f: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/world/county").join(f);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_baked_county_is_its_source_baked() {
    let file = baked::bake_county(&source("plan.txt"), &source("places.txt"), &source("routes.txt"), &mut |_| {})
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(file == COUNTY, "assets/world/county.bp is stale: its source has changed. Run `cargo jane bake-world`.");
}

#[test]
fn the_baked_county_reads_back_as_built_and_packed() {
    let pc = baked::load(ZoneId::County, false).expect("the baked county reads");
    assert_eq!((pc.zone, pc.w(), pc.h()), (ZoneId::County, 1536, 1536));
    let console = baked::load(ZoneId::County, true).expect("the baked county reads packed");
    let mut packed = pc.clone();
    packed.pack();
    assert!(console == packed, "the console form is the PC form packed");
    assert_eq!(baked::decode(COUNTY, ZoneId::School, false).err(), Some(baked::BakedError::Header), "another zone's");
    // Any byte changed is refused, never played.
    let mut bad = COUNTY.to_vec();
    let at = bad.len() / 2;
    bad[at] ^= 0x40;
    assert!(baked::decode(&bad, ZoneId::County, false).is_err(), "a damaged file is refused");
}
