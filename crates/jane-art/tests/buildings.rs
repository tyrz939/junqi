//! ART.md §5's acceptance tests for the building painter (§8 step 6): every building prop has
//! a look and draws its frames; its box is its footprint wide; windows emit only lit; the
//! colour budget; heights stand the walls up and land the roof on the house.

use std::collections::BTreeSet;

use jane_art::looks::{self, Family};
use jane_art::palette::Ix;
use jane_art::sprite::FrameId;
use jane_data::Look;

const BUILDINGS: [&str; 12] = [
    "cottage_thatch",
    "cottage_timber",
    "cottage_slate",
    "cottage_tile",
    "cottage_empty",
    "tale_cottage_boarded",
    "farmhouse",
    "barn",
    "shed",
    "inn",
    "reed_hut",
    "town_steeple",
];

#[test]
fn every_building_prop_is_drawn_and_lit_windows_emit() {
    let all = looks::family(Family::Building).unwrap();
    for name in BUILDINGS {
        let r = all.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("{name} has no look"));
        let Some((_, Look::Building(b))) = looks::find(name) else { unreachable!() };
        let (fw, _) = jane_art::kit::footprint(r.sprite).unwrap();
        assert_eq!(r.set.w, i32::from(fw) * 16, "{name}: its footprint wide");
        for (f, c) in &r.set.frames {
            c.validate().unwrap_or_else(|e| panic!("{name} {f:?}: {e}"));
            let lit = c.emissive().iter().filter(|&&e| e != Ix::CLEAR).count();
            if *f == FrameId::On {
                assert!(lit > 20, "{name}: its windows glow at night");
            } else {
                assert_eq!(lit, 0, "{name} {f:?}: glows by day");
            }
            let used: BTreeSet<u16> =
                c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0).collect();
            assert!(used.len() <= 96, "{name}: {} colours, the budget is 96", used.len());
        }
        assert_eq!(r.set.frame(FrameId::On).is_some(), b.lit, "{name}");
    }
}

#[test]
fn the_wall_stands_and_the_roof_lands_on_the_house() {
    let r = looks::render("cottage_slate").unwrap().remove(0);
    let c = r.set.frame(FrameId::Base).unwrap();
    let foot = c.h() - 1;
    let x = c.w() / 3;
    // Up the wall the height climbs a row at a time; on the roof it is higher still.
    let wall = c.height_at(x, foot - 4);
    let eave = (0..c.h()).rev().find(|&y| c.height_at(x, y) > 30).unwrap();
    assert!(wall < c.height_at(x, eave), "the roof stands over the wall");
    let top = (0..c.h()).find(|&y| c.get(x, y).is_opaque()).unwrap();
    assert!(c.height_at(x, top + 2) >= c.height_at(x, eave), "the ridge is the highest");
}
