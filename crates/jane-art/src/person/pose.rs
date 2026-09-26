//! The frame tables (ART.md §4): for each frame of a cycle, a body bob, the arms' and legs'
//! swing, a foot lift, the upper body's lean, and a `folds` phase. Every part reads the pose, so
//! a scarf end, a pack and a held thing move with the body; what hangs loose (hair ends, a
//! hem, a scarf's tail) reads the previous frame's bob and lean, so it lags a frame behind.

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
    /// The upper body's drop in px, down positive: the weight coming down onto a foot.
    pub bob: i32,
    /// The hands' swing: along the facing in px on `side` (forward positive), down the screen
    /// on `down` and `up` (a hand swung toward the viewer hangs lower).
    pub arm: [i32; 2],
    /// The feet's swing, as the hands'.
    pub leg: [i32; 2],
    /// A foot lifted off the ground, px.
    pub lift: [i32; 2],
    /// The head and shoulders carried forward of the hips, px (`side`): a walker leans in.
    pub lean: i32,
    /// The previous frame's bob and lean: what hangs loose follows these.
    pub lag: (i32, i32),
    /// The breathe: shoulders and head a pixel up, the folds half a turn on.
    pub breathe: bool,
    /// The `folds` phase.
    pub phase: u16,
    /// Hands flung out from the body, px outward (and half that up): a body fallen (`down`).
    pub spread: [i32; 2],
    /// Feet splayed out from the body, px outward (`down`).
    pub splay: [i32; 2],
}

const fn pose(bob: i32, arm: [i32; 2], leg: [i32; 2], lift: [i32; 2], lean: i32) -> Pose {
    Pose { bob, arm, leg, lift, lean, lag: (0, 0), breathe: false, phase: 0, spread: [0, 0], splay: [0, 0] }
}

/// The fallen, before they are laid down (ART.md §4.1): seen from above on her back, one arm
/// flung out, the other bent at her side, a knee drawn up and the other leg out.
pub const FALLEN: [Pose; 2] = [
    Pose { spread: [4, 1], splay: [1, 2], lift: [2, 0], ..pose(0, [0, 0], [0, 0], [0, 0], 0) },
    Pose { spread: [1, 4], splay: [2, 0], lift: [0, 3], ..pose(0, [0, 0], [0, 0], [0, 0], 0) },
];

/// A cycle with each frame's lag taken from the frame before it (the last before the first),
/// and the folds' phase a sixth of a turn a frame.
const fn cycle(mut t: [Pose; 6]) -> [Pose; 6] {
    let mut i = 0;
    let (mut bob, mut lean) = (t[5].bob, t[5].lean);
    while i < 6 {
        let (b, l) = (t[i].bob, t[i].lean);
        t[i].lag = (bob, lean);
        t[i].phase = (i as u32 * 65536 / 6) as u16;
        bob = b;
        lean = l;
        i += 1;
    }
    t
}

/// The walk seen from the side, six frames, standing frame first: stand (a pass, both feet
/// down), contact (near foot out ahead, far behind, leaning in), down (the weight comes onto
/// it, a pixel lower, the far heel up), pass (feet together, the far foot lifted), contact and
/// down on the other foot. The arms swing against the legs.
pub const WALK_SIDE: [Pose; 6] = cycle([
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(0, [-3, 3], [3, -3], [0, 0], 1),
    pose(1, [-2, 2], [2, -2], [0, 1], 1),
    pose(0, [0, 0], [0, 0], [0, 2], 0),
    pose(0, [3, -3], [-3, 3], [0, 0], 1),
    pose(1, [2, -2], [-2, 2], [1, 0], 1),
]);

/// The walk seen from the front: the forward foot a pixel lower, the back one lifted a pixel,
/// the hand on the forward foot's side swung back (up), the weight coming down after contact.
pub const WALK_DOWN: [Pose; 6] = cycle([
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(0, [-1, 1], [1, 0], [0, 0], 0),
    pose(1, [-1, 1], [1, 0], [0, 1], 0),
    pose(0, [0, 0], [0, 0], [0, 1], 0),
    pose(0, [1, -1], [0, 1], [0, 0], 0),
    pose(1, [1, -1], [0, 1], [1, 0], 0),
]);

/// The walk seen from behind: the forward foot goes up the screen.
pub const WALK_UP: [Pose; 6] = cycle([
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(0, [1, -1], [-1, 0], [0, 0], 0),
    pose(1, [1, -1], [-1, 0], [0, 1], 0),
    pose(0, [0, 0], [0, 0], [0, 1], 0),
    pose(0, [-1, 1], [0, -1], [0, 0], 0),
    pose(1, [-1, 1], [0, -1], [1, 0], 0),
]);

/// The breathe (ART.md §4): the standing frame with the chest, shoulders and head a pixel up
/// and the folds half a turn on; the hem stays where it hung.
pub const BREATHE: Pose = Pose { breathe: true, phase: 32768, ..pose(0, [0, 0], [0, 0], [0, 0], 0) };

/// The frames every person promises, with their facing and pose: three facings of six walk
/// frames and a breathe. The dead frames are derived from `Side` (ART.md §4.1).
pub const LIVING: [(FrameId, Facing, Pose); 21] = [
    (FrameId::Down, Facing::Down, WALK_DOWN[0]),
    (FrameId::Down1, Facing::Down, WALK_DOWN[1]),
    (FrameId::Down2, Facing::Down, WALK_DOWN[2]),
    (FrameId::Down3, Facing::Down, WALK_DOWN[3]),
    (FrameId::Down4, Facing::Down, WALK_DOWN[4]),
    (FrameId::Down5, Facing::Down, WALK_DOWN[5]),
    (FrameId::DownB, Facing::Down, BREATHE),
    (FrameId::Up, Facing::Up, WALK_UP[0]),
    (FrameId::Up1, Facing::Up, WALK_UP[1]),
    (FrameId::Up2, Facing::Up, WALK_UP[2]),
    (FrameId::Up3, Facing::Up, WALK_UP[3]),
    (FrameId::Up4, Facing::Up, WALK_UP[4]),
    (FrameId::Up5, Facing::Up, WALK_UP[5]),
    (FrameId::UpB, Facing::Up, BREATHE),
    (FrameId::Side, Facing::Side, WALK_SIDE[0]),
    (FrameId::Side1, Facing::Side, WALK_SIDE[1]),
    (FrameId::Side2, Facing::Side, WALK_SIDE[2]),
    (FrameId::Side3, Facing::Side, WALK_SIDE[3]),
    (FrameId::Side4, Facing::Side, WALK_SIDE[4]),
    (FrameId::Side5, Facing::Side, WALK_SIDE[5]),
    (FrameId::SideB, Facing::Side, BREATHE),
];
