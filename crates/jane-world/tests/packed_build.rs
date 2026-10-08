//! The console form of the build (PORT.md §13.3, phase 3): every zone built packed, the county
//! packed before its solve, is the zone built as on PC and then packed, field for field.

use jane_core::ZoneId;

#[test]
fn a_zone_built_packed_is_the_zone_built_then_packed() {
    for seed in [1, 2] {
        for z in ZoneId::ALL {
            let mut pc = jane_world::build_zone_with(z, seed, &mut |_| {}).expect("builds");
            pc.pack();
            let console = jane_world::build_zone_packed_with(z, seed, &mut |_| {}).expect("builds packed");
            assert!(console == pc, "seed {seed}: {} differs built packed", z.name());
        }
    }
}
