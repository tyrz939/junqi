//! Effects' art (ART.md §2, `fx`): the school colours, once, and the emissive parts a cast
//! shows. `SCHOOL_COLOUR` lives here: every spell's glow, bolt and pool reads its ramp.
//!
//! ```text
//! fx::school_ramp(school) -> Ramp       the school's colour, a ramp of the master palette
//! fx::cast_glow(school)   -> Canvas     the light gathered between the hands, emitting
//! ```

use jane_core::action::School;
use jane_core::grid::Rect;

use crate::canvas::{Canvas, Z};
use crate::palette::{Ramp, Tone};

/// The ramp a school's light is drawn in.
pub const fn school_ramp(s: School) -> Ramp {
    match s {
        School::Heal => Ramp::Bloom,
        School::Physical => Ramp::HairWhite,
        School::Frost => Ramp::Sky,
        School::Fire => Ramp::Ember,
        School::Nature => Ramp::Leaf,
        School::Blast => Ramp::ClothOchre,
        School::Shock => Ramp::GlassLit,
    }
}

/// Every school, in order.
pub const SCHOOLS: [School; 7] =
    [School::Heal, School::Physical, School::Frost, School::Fire, School::Nature, School::Blast, School::Shock];

/// The light a cast gathers between the hands: a soft orb of the school's colour, its core
/// bright, four short rays, all emitting. 13 x 13, drawn centred on the hands.
pub fn cast_glow(s: School) -> Canvas {
    let ramp = school_ramp(s);
    let mut c = Canvas::new(13, 13);
    c.set_emitting(true);
    c.soft_ellipse(Rect::new(2, 2, 9, 9), ramp, Z::new(1, 2));
    c.retone(ramp, [Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::Light, Tone::High, Tone::Glint, Tone::Glint]);
    for (x, y) in [(6, 0), (6, 1), (6, 11), (6, 12), (0, 6), (1, 6), (11, 6), (12, 6)] {
        c.dot(x, y, ramp.at(Tone::Light), 2);
    }
    c.fill_rect(Rect::new(5, 5, 3, 3), ramp.at(Tone::Glint), 3);
    c.set_emitting(false);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Ix;

    #[test]
    fn every_school_glows_in_its_own_colour() {
        let mut seen = std::collections::BTreeSet::new();
        for s in SCHOOLS {
            let c = cast_glow(s);
            c.validate().unwrap();
            assert!(c.emissive().iter().filter(|&&e| e != Ix::CLEAR).count() > 40, "{s:?} glows");
            assert!(seen.insert(school_ramp(s)), "{s:?} shares a colour");
        }
    }
}
