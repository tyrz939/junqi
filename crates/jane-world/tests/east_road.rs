//! The east road (`tuning/country.json` `eastRoad`; STORY.md §6, WORLD.md §7.3), on every seed the
//! county tests build: its lamps are their own row and stand switched on, its river bridge is the
//! rect the bridge omen counts on, with a lamp of its own row at each end, and the lighting notice
//! and the toll board that make the two claims stand by the road.

use jane_core::{Key, ZoneId};
use jane_world::build_zone;

#[test]
fn the_east_road_has_its_lamps_its_bridge_and_its_notices_on_every_seed() {
    let cat = jane_data::catalog();
    let e = cat.county.furnishing.east_road.expect("tuning/country.json has an east road");
    for seed in 1..=16 {
        let bp = build_zone(ZoneId::County, seed).expect("the county builds");
        let lamps: Vec<_> = bp.props.iter().filter(|p| p.def == e.lamp).collect();
        assert!(lamps.len() >= 2, "seed {seed}: {} east road lamps", lamps.len());
        assert!(lamps.iter().all(|p| p.on), "seed {seed}: stood switched on");
        let bridge_lamps: Vec<_> = bp.props.iter().filter(|p| p.def == e.bridge_lamp).collect();
        assert!(
            !bridge_lamps.is_empty() && bridge_lamps.len() <= 2,
            "seed {seed}: {} bridge lamps",
            bridge_lamps.len()
        );
        assert!(bridge_lamps.iter().all(|p| p.on));
        let rect = bp.rects.get(&Key::Name(e.bridge)).unwrap_or_else(|| panic!("seed {seed}: the east bridge"));
        assert!(rect.w >= 7 && rect.h >= 7, "seed {seed}: {rect:?}");
        for tree in [e.notice, e.toll_board] {
            assert!(bp.props.iter().any(|p| p.talk == Some(tree)), "seed {seed}: {}", cat.story.dialogue(tree).id);
        }
    }
}
