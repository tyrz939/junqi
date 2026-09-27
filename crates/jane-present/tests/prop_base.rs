//! A prop that blocks less than its footprint (`PropDef::base`, the owner's first playtest: "can't
//! get close to things from above them") never blocks less than it is drawn standing on. Every px
//! of its look stands on the ground `rows_up(height)` rows below it (ART.md §1.1); the rows of
//! ground those reach, counted from the footprint's front edge, fit inside its `base` rows.

use jane_art::canvas::rows_up;
use jane_art::looks::{self, Family};
use jane_art::palette::Ix;

/// Canvas px a sim cell is drawn at.
const CELL: i32 = 16;

#[test]
fn a_prop_blocks_at_least_the_ground_it_is_drawn_on() {
    let cat = jane_data::catalog();
    let mut looks = looks::family(Family::Prop).expect("the props render");
    looks.extend(looks::family(Family::Building).expect("the buildings render"));
    let mut checked = 0;
    let mut short = Vec::new();
    for d in cat.story.props.iter().filter(|d| d.base < d.h) {
        for r in looks.iter().filter(|r| r.sprite == d.sprite) {
            for (_, c) in &r.set.frames {
                // The canvas's last row is the footprint's front edge.
                let front = c.h();
                let mut deepest = 0;
                for y in 0..c.h() {
                    for x in 0..c.w() {
                        if c.get(x, y) != Ix::CLEAR {
                            deepest = deepest.max(front - (y + rows_up(i32::from(c.height_at(x, y)))));
                        }
                    }
                }
                if deepest > i32::from(d.base) * CELL {
                    short.push(format!(
                        "{}: drawn on {deepest} px of ground, blocks {}",
                        d.id,
                        i32::from(d.base) * CELL
                    ));
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 40, "the upright props were looked at ({checked})");
    assert!(short.is_empty(), "blocks less than it stands on: {short:#?}");
}
