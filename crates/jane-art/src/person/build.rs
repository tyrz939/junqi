//! The per-build table (ART.md §2.1): eight numbers a build, which every other axis and every
//! frame table reads.

use jane_data::Build;

/// A build's proportions, in px on the 32 x 40 frame with the feet on row 36.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proportions {
    /// The top of the hair (the outline's row), with the skull two rows under it.
    pub head_y: i32,
    /// The head's width with its hair, outline included.
    pub head_w: i32,
    /// The coat across the shoulders, outline included, arms outside it.
    pub shoulder_w: i32,
    /// The coat at the waist.
    pub waist_w: i32,
    /// The coat at the hip, where the legs start.
    pub hip_w: i32,
    /// From the hip to the sole.
    pub leg_h: i32,
    /// The row the arms hang from.
    pub arm_y: i32,
    /// From the shoulder to the fingertips.
    pub arm_l: i32,
}

/// The four builds. Only `slim` is tuned (ART.md §8 step 2); `child` dresses the town's four
/// children, and `broad` and `stout` are first drafts that later steps tune.
pub const fn of(b: Build) -> Proportions {
    match b {
        Build::Slim => {
            Proportions { head_y: 4, head_w: 16, shoulder_w: 12, waist_w: 10, hip_w: 12, leg_h: 7, arm_y: 22, arm_l: 9 }
        }
        Build::Broad => {
            Proportions { head_y: 4, head_w: 16, shoulder_w: 16, waist_w: 14, hip_w: 14, leg_h: 7, arm_y: 22, arm_l: 9 }
        }
        Build::Stout => {
            Proportions { head_y: 5, head_w: 16, shoulder_w: 14, waist_w: 16, hip_w: 16, leg_h: 6, arm_y: 23, arm_l: 8 }
        }
        Build::Child => Proportions {
            head_y: 10,
            head_w: 16,
            shoulder_w: 10,
            waist_w: 10,
            hip_w: 10,
            leg_h: 5,
            arm_y: 26,
            arm_l: 6,
        },
    }
}

impl Proportions {
    /// The skull's height; a child's is a row shorter.
    pub const fn skull_h(&self) -> i32 {
        if self.leg_h <= 5 { 12 } else { 13 }
    }

    /// The skull's top row (the hair sits two rows above it).
    pub const fn skull_y(&self) -> i32 {
        self.head_y + 2
    }

    /// The chin: the skull's last row.
    pub const fn chin_y(&self) -> i32 {
        self.skull_y() + self.skull_h() - 1
    }

    /// The shoulders' top row: under the chin and a neck of two rows (one for a child).
    pub const fn shoulder_y(&self) -> i32 {
        self.chin_y() + if self.leg_h <= 5 { 2 } else { 3 }
    }

    /// The hip: where the legs leave the coat.
    pub const fn hip_y(&self) -> i32 {
        super::AY - self.leg_h
    }
}
