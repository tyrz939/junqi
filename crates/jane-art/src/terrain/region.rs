//! The region ramps (ART.md §2.7 `RegionStyle`, §8 step 8): the same tiles in each region's own
//! materials, so the Lowfields, the Waters and the Works read as three places. A region's row
//! swaps ramps tone for tone as the chunk's albedo is resolved, per cell, by the region under it
//! (`TileSource::region`): the painter's clusters and shading are kept, only their colour moves.
//!
//! The Lowfields are the palette as drawn: warm greens and plaster, tile and thatch. The Waters
//! turn their grass to a dank blue-green, their earth to mud and their roofs to slate and reed.
//! The Works' grass is slag-grey (`turf_slag`), their earth the same slag, their gravel cinder, their
//! walls brick and their roofs slate. The lamp, the sky and the mist of a region are the weather's
//! (`jane-present`'s sky and atmosphere), not rows here.

use crate::palette::{Ix, Ramp};

/// One region's ramps: what each of the ground's, the walls' and the roofs' ramps becomes.
#[derive(Clone, Copy, Debug)]
pub struct RegionStyle {
    /// The region's name (`jane_data::Region`, in its order).
    pub name: &'static str,
    /// The ground's swaps: grass, earth, gravel, hedges.
    pub ground: &'static [(Ramp, Ramp)],
    /// The walls' swaps.
    pub wall: &'static [(Ramp, Ramp)],
    /// The roofs' swaps.
    pub roof: &'static [(Ramp, Ramp)],
}

/// The three regions, in `jane_data::Region` order: 0 Lowfields, 1 Waters, 2 Works.
pub const REGIONS: [RegionStyle; 3] = [
    RegionStyle { name: "lowfields", ground: &[], wall: &[], roof: &[] },
    RegionStyle {
        name: "waters",
        ground: &[
            (Ramp::Turf, Ramp::LeafDeep),
            (Ramp::TurfDry, Ramp::Marsh),
            (Ramp::Earth, Ramp::Mud),
            (Ramp::Hedge, Ramp::Needle),
        ],
        wall: &[],
        roof: &[(Ramp::RoofTile, Ramp::Slate), (Ramp::Thatch, Ramp::Reed)],
    },
    RegionStyle {
        name: "works",
        ground: &[
            (Ramp::Turf, Ramp::TurfSlag),
            (Ramp::TurfDry, Ramp::TurfSlag),
            (Ramp::Earth, Ramp::TurfSlag),
            (Ramp::Gravel, Ramp::Ballast),
            (Ramp::Soil, Ramp::Mud),
        ],
        wall: &[(Ramp::Plaster, Ramp::Brick)],
        roof: &[(Ramp::RoofTile, Ramp::Slate), (Ramp::Thatch, Ramp::Slate)],
    },
];

/// `ix` as region `region` draws it (a region past the table draws as the Lowfields).
#[inline]
pub fn tint(region: u8, ix: Ix) -> Ix {
    let Some(style) = REGIONS.get(usize::from(region)).filter(|_| region != 0) else { return ix };
    let Some((ramp, tone)) = Ramp::of(ix) else { return ix };
    for &(from, to) in style.ground.iter().chain(style.wall).chain(style.roof) {
        if from == ramp {
            return to.at(tone);
        }
    }
    ix
}

/// Whether some region swaps `ramp` as ground: what the ecotone blends across a border (a wall's
/// or a roof's ramp keeps its cell's region whole).
#[inline]
pub fn is_ground(ramp: Ramp) -> bool {
    GROUND[ramp as usize]
}

/// [`is_ground`] by ramp, worked once.
const GROUND: [bool; Ramp::ALL.len()] = {
    let mut t = [false; Ramp::ALL.len()];
    let mut r = 0;
    while r < REGIONS.len() {
        let g = REGIONS[r].ground;
        let mut i = 0;
        while i < g.len() {
            t[g[i].0 as usize] = true;
            i += 1;
        }
        r += 1;
    }
    t
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Tone;

    #[test]
    fn each_region_keeps_the_tone_and_moves_the_colour() {
        let grass = Ramp::Turf.at(Tone::Lift);
        assert_eq!(tint(0, grass), grass);
        assert_eq!(tint(1, grass), Ramp::LeafDeep.at(Tone::Lift));
        assert_eq!(tint(2, grass), Ramp::TurfSlag.at(Tone::Lift));
        assert_eq!(tint(2, Ramp::Plaster.at(Tone::Base)), Ramp::Brick.at(Tone::Base));
        // What no region swaps is itself everywhere.
        assert_eq!(tint(2, Ramp::Water.at(Tone::Base)), Ramp::Water.at(Tone::Base));
        assert_eq!(tint(9, grass), grass);
    }
}
