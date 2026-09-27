//! The frame tables (ART.md §4): for each frame of a cycle, a body bob, the arms' and legs'
//! swing, a foot lift, the upper body's lean, and a `folds` phase. Every part reads the pose, so
//! a scarf end, a pack and a held thing move with the body; what hangs loose (hair ends, a
//! hem, a scarf's tail) reads the previous frame's bob and lean, so it lags a frame behind.

use crate::sprite::FrameId;

/// Which way a frame faces. West is `Side` mirrored at draw time, south-west `DownRight` and
/// north-west `UpRight`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// Toward the viewer (south).
    Down,
    /// Away from the viewer (north).
    Up,
    /// East.
    Side,
    /// Toward the viewer and to the right (south-east): three quarters on, the face turned
    /// toward the right, the near side of her the screen's left.
    DownRight,
    /// Away and to the right (north-east): three quarters from behind, a sliver of cheek past
    /// the hair on the right, the near side of her the screen's right.
    UpRight,
}

impl Facing {
    /// Toward the viewer, straight on or three quarters: the face shows.
    pub const fn front(self) -> bool {
        matches!(self, Facing::Down | Facing::DownRight)
    }

    /// Away from the viewer, straight or three quarters: the back shows.
    pub const fn back(self) -> bool {
        matches!(self, Facing::Up | Facing::UpRight)
    }

    /// A diagonal.
    pub const fn diagonal(self) -> bool {
        matches!(self, Facing::DownRight | Facing::UpRight)
    }
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
    /// Hands raised, px up the screen: a blow wound up, a spell gathered.
    pub raise: [i32; 2],
    /// The eyes shut: a blow taken.
    pub shut: bool,
}

const fn pose(bob: i32, arm: [i32; 2], leg: [i32; 2], lift: [i32; 2], lean: i32) -> Pose {
    Pose {
        bob,
        arm,
        leg,
        lift,
        lean,
        lag: (0, 0),
        breathe: false,
        phase: 0,
        spread: [0, 0],
        splay: [0, 0],
        raise: [0, 0],
        shut: false,
    }
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
    pose(0, [-4, 4], [4, -4], [0, 0], 1),
    pose(1, [-3, 3], [3, -3], [0, 1], 1),
    pose(0, [1, -1], [0, 0], [0, 2], 0),
    pose(0, [4, -4], [-4, 4], [0, 0], 1),
    pose(1, [3, -3], [-3, 3], [1, 0], 1),
]);

/// The walk seen from the front: the forward foot a pixel lower, the back one lifted a pixel,
/// the hand on the forward foot's side swung back (up), the weight coming down after contact.
pub const WALK_DOWN: [Pose; 6] = cycle([
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(0, [-2, 2], [1, 0], [0, 0], 0),
    pose(1, [-1, 1], [1, 0], [0, 2], 0),
    pose(0, [0, 0], [0, 0], [0, 1], 0),
    pose(0, [2, -2], [0, 1], [0, 0], 0),
    pose(1, [1, -1], [0, 1], [2, 0], 0),
]);

/// The walk seen from behind: the forward foot goes up the screen.
pub const WALK_UP: [Pose; 6] = cycle([
    pose(0, [0, 0], [0, 0], [0, 0], 0),
    pose(0, [2, -2], [-1, 0], [0, 0], 0),
    pose(1, [1, -1], [-1, 0], [0, 2], 0),
    pose(0, [0, 0], [0, 0], [0, 1], 0),
    pose(0, [-2, 2], [0, -1], [0, 0], 0),
    pose(1, [-1, 1], [0, -1], [2, 0], 0),
]);

/// The walk on a diagonal: the side's table, its swings read along the diagonal (a stride of
/// three px is two across and two down the screen), the near limbs the index-0 ones as on the
/// side.
pub const WALK_DIAG: [Pose; 6] = WALK_SIDE;

/// The breathe (ART.md §4): the standing frame with the chest, shoulders and head a pixel up
/// and the folds half a turn on; the hem stays where it hung.
pub const BREATHE: Pose = Pose { breathe: true, phase: 32768, ..pose(0, [0, 0], [0, 0], [0, 0], 0) };

/// The frames every person promises, with their facing and pose: five facings of six walk
/// frames and a breathe (the west three are mirrors at draw time). The dead frames are posed
/// from the front (ART.md §4.1).
pub const LIVING: [(FrameId, Facing, Pose); 35] = [
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
    (FrameId::DownRight, Facing::DownRight, WALK_DIAG[0]),
    (FrameId::DownRight1, Facing::DownRight, WALK_DIAG[1]),
    (FrameId::DownRight2, Facing::DownRight, WALK_DIAG[2]),
    (FrameId::DownRight3, Facing::DownRight, WALK_DIAG[3]),
    (FrameId::DownRight4, Facing::DownRight, WALK_DIAG[4]),
    (FrameId::DownRight5, Facing::DownRight, WALK_DIAG[5]),
    (FrameId::DownRightB, Facing::DownRight, BREATHE),
    (FrameId::UpRight, Facing::UpRight, WALK_DIAG[0]),
    (FrameId::UpRight1, Facing::UpRight, WALK_DIAG[1]),
    (FrameId::UpRight2, Facing::UpRight, WALK_DIAG[2]),
    (FrameId::UpRight3, Facing::UpRight, WALK_DIAG[3]),
    (FrameId::UpRight4, Facing::UpRight, WALK_DIAG[4]),
    (FrameId::UpRight5, Facing::UpRight, WALK_DIAG[5]),
    (FrameId::UpRightB, Facing::UpRight, BREATHE),
];

/// A pose from the standing frame with only these moved.
const fn act(bob: i32, arm: [i32; 2], raise: [i32; 2], spread: [i32; 2], leg: [i32; 2], lean: i32, shut: bool) -> Pose {
    Pose { spread, raise, shut, ..pose(bob, arm, leg, [0, 0], lean) }
}

/// The attack (ART.md §4): wind-up (lean back, the near hand raised), strike (lean in, the arm
/// at full reach, the weight onto the front foot), recover. From the side (east); facing the
/// viewer the near hand comes down across the body; from behind, the same seen from the back.
pub const ATTACK_SIDE: [Pose; 3] = [
    act(0, [-3, 1], [6, 0], [0, 0], [0, 0], -2, false),
    act(1, [8, -2], [5, 0], [0, 0], [2, -2], 3, false),
    act(0, [3, 0], [1, 0], [0, 0], [1, -1], 1, false),
];
/// The attack on a diagonal: the side's, its reach two px shorter (a blow along the diagonal
/// shows two thirds of itself across the screen, and a held thing at full reach must stay in
/// the frame).
pub const ATTACK_DIAG: [Pose; 3] = [
    act(0, [-3, 1], [6, 0], [0, 0], [0, 0], -2, false),
    act(1, [6, -2], [5, 0], [0, 0], [2, -2], 3, false),
    act(0, [2, 0], [1, 0], [0, 0], [1, -1], 1, false),
];
/// The attack facing the viewer.
pub const ATTACK_DOWN: [Pose; 3] = [
    act(0, [0, 0], [8, 0], [2, 0], [0, 0], 0, false),
    act(1, [3, -1], [2, 0], [-3, 0], [1, 0], 0, false),
    act(0, [1, 0], [1, 0], [0, 0], [0, 0], 0, false),
];
/// The attack from behind.
pub const ATTACK_UP: [Pose; 3] = [
    act(0, [0, 0], [0, 8], [0, 2], [0, 0], 0, false),
    act(1, [-1, 3], [0, 3], [0, -3], [0, -1], 0, false),
    act(0, [0, 1], [0, 1], [0, 0], [0, 0], 0, false),
];
/// The cast (ART.md §4): hands together, hands out (the school's glow between them: the
/// presenter's fx), hands down.
pub const CAST_SIDE: [Pose; 3] = [
    act(0, [3, 3], [5, 5], [0, 0], [0, 0], 0, false),
    act(0, [6, 5], [6, 7], [0, 0], [1, -1], 1, false),
    act(0, [1, 1], [1, 1], [0, 0], [0, 0], 0, false),
];
/// The cast facing the viewer.
pub const CAST_DOWN: [Pose; 3] = [
    act(0, [0, 0], [5, 5], [-2, -2], [0, 0], 0, false),
    act(0, [0, 0], [7, 7], [1, 1], [0, 0], 0, false),
    act(0, [0, 0], [1, 1], [0, 0], [0, 0], 0, false),
];
/// The cast from behind.
pub const CAST_UP: [Pose; 3] = CAST_DOWN;
/// Hurt (ART.md §4): leant back, the head down a px, the eyes shut.
pub const HURT_SIDE: Pose = act(1, [-2, -3], [1, 1], [0, 0], [-1, 1], -2, true);
/// Hurt facing the viewer or away.
pub const HURT_DOWN: Pose = act(1, [0, 0], [2, 2], [1, 1], [0, 0], 0, true);

/// The fight frames a person promises when it attacks, casts, or can be hurt, with their facing
/// and pose.
pub fn fight(attacks: bool, casts: bool) -> Vec<(FrameId, Facing, Pose)> {
    use FrameId as F;
    let mut v = Vec::new();
    if attacks {
        for (ids, facing, poses) in [
            ([F::Atk1, F::Atk2, F::Atk3], Facing::Side, ATTACK_SIDE),
            ([F::AtkDown1, F::AtkDown2, F::AtkDown3], Facing::Down, ATTACK_DOWN),
            ([F::AtkUp1, F::AtkUp2, F::AtkUp3], Facing::Up, ATTACK_UP),
            ([F::AtkDownRight1, F::AtkDownRight2, F::AtkDownRight3], Facing::DownRight, ATTACK_DIAG),
            ([F::AtkUpRight1, F::AtkUpRight2, F::AtkUpRight3], Facing::UpRight, ATTACK_DIAG),
        ] {
            v.extend(ids.into_iter().zip(poses).map(|(f, p)| (f, facing, p)));
        }
    }
    if casts {
        for (ids, facing, poses) in [
            ([F::Cast1, F::Cast2, F::Cast3], Facing::Side, CAST_SIDE),
            ([F::CastDown1, F::CastDown2, F::CastDown3], Facing::Down, CAST_DOWN),
            ([F::CastUp1, F::CastUp2, F::CastUp3], Facing::Up, CAST_UP),
            ([F::CastDownRight1, F::CastDownRight2, F::CastDownRight3], Facing::DownRight, CAST_SIDE),
            ([F::CastUpRight1, F::CastUpRight2, F::CastUpRight3], Facing::UpRight, CAST_SIDE),
        ] {
            v.extend(ids.into_iter().zip(poses).map(|(f, p)| (f, facing, p)));
        }
    }
    if attacks || casts {
        v.extend([
            (F::Hurt, Facing::Side, HURT_SIDE),
            (F::HurtDown, Facing::Down, HURT_DOWN),
            (F::HurtUp, Facing::Up, HURT_DOWN),
            (F::HurtDownRight, Facing::DownRight, HURT_SIDE),
            (F::HurtUpRight, Facing::UpRight, HURT_SIDE),
        ]);
    }
    v
}
