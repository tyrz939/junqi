//! The frame tables (ART.md §4): for each frame of a cycle, a body bob, the arms' and legs'
//! swing, a foot lift and a `folds` phase. Every part reads the pose, so a scarf end, a pack and
//! a held thing move with the body.

use crate::sprite::FrameId;

/// Which way a frame faces. West is `Side` mirrored at draw time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// Toward the viewer (south).
    Down,
    /// Away from the viewer (north).
    Up,
    /// East.
    Side,
}

/// One frame's pose. Index 0 of a pair is the near limb (side) or the screen-left one (down,
/// up); index 1 the far or screen-right one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pose {
    /// The upper body's drop in px, down positive: a stride sits a pixel lower than a pass.
    pub bob: i32,
    /// The hands' swing: along the facing in px on `side` (forward positive), down the screen
    /// on `down` and `up` (a hand swung toward the viewer hangs lower).
    pub arm: [i32; 2],
    /// The feet's swing, as the hands'.
    pub leg: [i32; 2],
    /// A foot lifted off the ground, px.
    pub lift: [i32; 2],
    /// The breathe: shoulders and head a pixel up, the folds half a turn on.
    pub breathe: bool,
    /// The `folds` phase, a quarter turn a frame.
    pub phase: u16,
}

const fn pose(bob: i32, arm: [i32; 2], leg: [i32; 2], lift: [i32; 2], phase: u16) -> Pose {
    Pose { bob, arm, leg, lift, breathe: false, phase }
}

/// The walk seen from the side, standing frame first: stand (a pass, both feet down), stride
/// (near foot forward, sitting a pixel lower), pass (the far foot lifted), stride (far foot
/// forward). The arms swing against the legs.
pub const WALK_SIDE: [Pose; 4] = [
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(1, [-3, 3], [3, -3], [0, 0], 16384),
    pose(0, [0, 0], [0, 0], [0, 2], 32768),
    pose(1, [3, -3], [-3, 3], [0, 0], 49152),
];

/// The walk seen from the front: the forward foot a pixel lower, the back one lifted a pixel,
/// the hand on the forward foot's side swung back (up).
pub const WALK_DOWN: [Pose; 4] = [
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(1, [-1, 1], [1, 0], [0, 1], 16384),
    pose(0, [0, 0], [0, 0], [0, 1], 32768),
    pose(1, [1, -1], [0, 1], [1, 0], 49152),
];

/// The walk seen from behind: the forward foot goes up the screen.
pub const WALK_UP: [Pose; 4] = [
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(1, [1, -1], [-1, 0], [0, 1], 16384),
    pose(0, [0, 0], [0, 0], [0, 1], 32768),
    pose(1, [-1, 1], [0, -1], [1, 0], 49152),
];

/// The breathe (ART.md §4): the standing frame with the chest, shoulders and head a pixel up
/// and the folds half a turn on.
pub const BREATHE: Pose = Pose { breathe: true, phase: 32768, ..pose(0, [0, 0], [0, 0], [0, 0], 0) };

/// The frames every person promises in step 2, with their facing and pose: three facings of
/// four walk frames and a breathe. The dead frames are derived from `Side` (ART.md §4.1).
pub const LIVING: [(FrameId, Facing, Pose); 15] = [
    (FrameId::Down, Facing::Down, WALK_DOWN[0]),
    (FrameId::Down1, Facing::Down, WALK_DOWN[1]),
    (FrameId::Down2, Facing::Down, WALK_DOWN[2]),
    (FrameId::Down3, Facing::Down, WALK_DOWN[3]),
    (FrameId::DownB, Facing::Down, BREATHE),
    (FrameId::Up, Facing::Up, WALK_UP[0]),
    (FrameId::Up1, Facing::Up, WALK_UP[1]),
    (FrameId::Up2, Facing::Up, WALK_UP[2]),
    (FrameId::Up3, Facing::Up, WALK_UP[3]),
    (FrameId::UpB, Facing::Up, BREATHE),
    (FrameId::Side, Facing::Side, WALK_SIDE[0]),
    (FrameId::Side1, Facing::Side, WALK_SIDE[1]),
    (FrameId::Side2, Facing::Side, WALK_SIDE[2]),
    (FrameId::Side3, Facing::Side, WALK_SIDE[3]),
    (FrameId::SideB, Facing::Side, BREATHE),
];
