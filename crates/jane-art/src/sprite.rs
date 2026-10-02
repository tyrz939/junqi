//! The generators' product (ART.md §1): a [`SpriteSet`] is one look rendered, every frame its
//! family and cycles promise, all four layers each, with the ramps each role was drawn in so a
//! swap can recolour by role.
//!
//! This is what the atlas builder packs and what `jane sheet` draws. Frames are canvases of one
//! size; the anchor is where the sprite stands (units: feet at `(w / 2, h - 4)`).

use crate::canvas::Canvas;
use crate::palette::{Ix, Ramp};

/// A frame of a sprite (ART.md §1). The TS build's names keep their meaning: `Down` is the
/// standing frame facing the viewer, `Base` a prop at rest, `Open` over `On` over `Base` the
/// renderer's pick. Walks are six frames with the standing frame first (`Down, Down1 ..
/// Down5`), and `DownB` is the breathe; `Side` faces east and west is the east frame mirrored
/// at draw time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum FrameId {
    Down,
    Down1,
    Down2,
    Down3,
    Down4,
    Down5,
    DownB,
    Up,
    Up1,
    Up2,
    Up3,
    Up4,
    Up5,
    UpB,
    Side,
    Side1,
    Side2,
    Side3,
    Side4,
    Side5,
    SideB,
    Atk1,
    Atk2,
    Atk3,
    Cast1,
    Cast2,
    Cast3,
    Hurt,
    Dead,
    Dead2,
    Base,
    Base2,
    Base3,
    On,
    Open,
    /// A creature's idle pose, facing the viewer (ART.md §2.2): the dog and the cat sit, the
    /// sheep grazes, a hen pecks, a rabbit sits up.
    Idle,
    /// The idle pose's other beat: a head tilted, a tail swept, a nose to the ground.
    Idle2,
    /// A person's attack, cast and hurt facing the viewer and away (ART.md §4); `Atk1 .. Atk3`,
    /// `Cast1 .. Cast3` and `Hurt` are the side's (east; west mirrors).
    AtkDown1,
    AtkDown2,
    AtkDown3,
    AtkUp1,
    AtkUp2,
    AtkUp3,
    CastDown1,
    CastDown2,
    CastDown3,
    CastUp1,
    CastUp2,
    CastUp3,
    HurtDown,
    HurtUp,
    /// The diagonals (ART.md §4): facing the viewer and to the right (south-east), the walk, the
    /// breathe; south-west is this mirrored at draw time, as west is `Side`'s.
    DownRight,
    DownRight1,
    DownRight2,
    DownRight3,
    DownRight4,
    DownRight5,
    DownRightB,
    /// Facing away and to the right (north-east); north-west mirrors it.
    UpRight,
    UpRight1,
    UpRight2,
    UpRight3,
    UpRight4,
    UpRight5,
    UpRightB,
    /// A person's attack, cast and hurt on the diagonals.
    AtkDownRight1,
    AtkDownRight2,
    AtkDownRight3,
    AtkUpRight1,
    AtkUpRight2,
    AtkUpRight3,
    CastDownRight1,
    CastDownRight2,
    CastDownRight3,
    CastUpRight1,
    CastUpRight2,
    CastUpRight3,
    HurtDownRight,
    HurtUpRight,
    /// `Open` for a look's second and third bases: an apple tree picked keeps its own crown.
    Open2,
    Open3,
    /// A person's blink (ART-PLAN Q4): the standing frame with the eyes a 1-px line, on the
    /// facings whose eyes show (the west ones mirror the side's and the diagonal's).
    DownBlink,
    SideBlink,
    DownRightBlink,
    /// The frame after a turn (ART-PLAN B3): the new facing standing, with the hair's ends and
    /// the hem still swung from the old, trailing behind and lifted a px. A turn to face the
    /// viewer or away passes through the diagonal on the side it came from.
    SideTurn,
    DownRightTurn,
    UpRightTurn,
    /// A landing (ART-PLAN B3): the standing frame squashed, a px shorter and a touch wider.
    DownLand,
    UpLand,
    SideLand,
    DownRightLand,
    UpRightLand,
    /// A townsperson's work (ART-PLAN Q4): a loop of up to four beats facing the viewer (a page
    /// turned, a bottle set down, the needles and the rocking chair), never mirrored ...
    Task1,
    Task2,
    Task3,
    Task4,
    /// ... or from the side (the broom swept), mirrored for the west like the side's walk.
    TaskSide1,
    TaskSide2,
    TaskSide3,
    TaskSide4,
}

impl FrameId {
    /// The name `jane sheet` labels it with.
    pub const fn name(self) -> &'static str {
        match self {
            FrameId::Down => "down",
            FrameId::Down1 => "down_1",
            FrameId::Down2 => "down_2",
            FrameId::Down3 => "down_3",
            FrameId::Down4 => "down_4",
            FrameId::Down5 => "down_5",
            FrameId::DownB => "down_b",
            FrameId::Up => "up",
            FrameId::Up1 => "up_1",
            FrameId::Up2 => "up_2",
            FrameId::Up3 => "up_3",
            FrameId::Up4 => "up_4",
            FrameId::Up5 => "up_5",
            FrameId::UpB => "up_b",
            FrameId::Side => "side",
            FrameId::Side1 => "side_1",
            FrameId::Side2 => "side_2",
            FrameId::Side3 => "side_3",
            FrameId::Side4 => "side_4",
            FrameId::Side5 => "side_5",
            FrameId::SideB => "side_b",
            FrameId::Atk1 => "atk_1",
            FrameId::Atk2 => "atk_2",
            FrameId::Atk3 => "atk_3",
            FrameId::Cast1 => "cast_1",
            FrameId::Cast2 => "cast_2",
            FrameId::Cast3 => "cast_3",
            FrameId::Hurt => "hurt",
            FrameId::Dead => "dead",
            FrameId::Dead2 => "dead_2",
            FrameId::Base => "base",
            FrameId::Base2 => "base_2",
            FrameId::Base3 => "base_3",
            FrameId::On => "on",
            FrameId::Open => "open",
            FrameId::Open2 => "open_2",
            FrameId::Open3 => "open_3",
            FrameId::Idle => "idle",
            FrameId::Idle2 => "idle_2",
            FrameId::AtkDown1 => "atk_down_1",
            FrameId::AtkDown2 => "atk_down_2",
            FrameId::AtkDown3 => "atk_down_3",
            FrameId::AtkUp1 => "atk_up_1",
            FrameId::AtkUp2 => "atk_up_2",
            FrameId::AtkUp3 => "atk_up_3",
            FrameId::CastDown1 => "cast_down_1",
            FrameId::CastDown2 => "cast_down_2",
            FrameId::CastDown3 => "cast_down_3",
            FrameId::CastUp1 => "cast_up_1",
            FrameId::CastUp2 => "cast_up_2",
            FrameId::CastUp3 => "cast_up_3",
            FrameId::HurtDown => "hurt_down",
            FrameId::HurtUp => "hurt_up",
            FrameId::DownRight => "down_right",
            FrameId::DownRight1 => "down_right_1",
            FrameId::DownRight2 => "down_right_2",
            FrameId::DownRight3 => "down_right_3",
            FrameId::DownRight4 => "down_right_4",
            FrameId::DownRight5 => "down_right_5",
            FrameId::DownRightB => "down_right_b",
            FrameId::UpRight => "up_right",
            FrameId::UpRight1 => "up_right_1",
            FrameId::UpRight2 => "up_right_2",
            FrameId::UpRight3 => "up_right_3",
            FrameId::UpRight4 => "up_right_4",
            FrameId::UpRight5 => "up_right_5",
            FrameId::UpRightB => "up_right_b",
            FrameId::AtkDownRight1 => "atk_down_right_1",
            FrameId::AtkDownRight2 => "atk_down_right_2",
            FrameId::AtkDownRight3 => "atk_down_right_3",
            FrameId::AtkUpRight1 => "atk_up_right_1",
            FrameId::AtkUpRight2 => "atk_up_right_2",
            FrameId::AtkUpRight3 => "atk_up_right_3",
            FrameId::CastDownRight1 => "cast_down_right_1",
            FrameId::CastDownRight2 => "cast_down_right_2",
            FrameId::CastDownRight3 => "cast_down_right_3",
            FrameId::CastUpRight1 => "cast_up_right_1",
            FrameId::CastUpRight2 => "cast_up_right_2",
            FrameId::CastUpRight3 => "cast_up_right_3",
            FrameId::HurtDownRight => "hurt_down_right",
            FrameId::HurtUpRight => "hurt_up_right",
            FrameId::DownBlink => "down_blink",
            FrameId::SideBlink => "side_blink",
            FrameId::DownRightBlink => "down_right_blink",
            FrameId::SideTurn => "side_turn",
            FrameId::DownRightTurn => "down_right_turn",
            FrameId::UpRightTurn => "up_right_turn",
            FrameId::DownLand => "down_land",
            FrameId::UpLand => "up_land",
            FrameId::SideLand => "side_land",
            FrameId::DownRightLand => "down_right_land",
            FrameId::UpRightLand => "up_right_land",
            FrameId::Task1 => "task_1",
            FrameId::Task2 => "task_2",
            FrameId::Task3 => "task_3",
            FrameId::Task4 => "task_4",
            FrameId::TaskSide1 => "task_side_1",
            FrameId::TaskSide2 => "task_side_2",
            FrameId::TaskSide3 => "task_side_3",
            FrameId::TaskSide4 => "task_side_4",
        }
    }

    /// Every frame, in order.
    pub const ALL: [FrameId; 100] = {
        use FrameId as F;
        [
            F::Down,
            F::Down1,
            F::Down2,
            F::Down3,
            F::Down4,
            F::Down5,
            F::DownB,
            F::Up,
            F::Up1,
            F::Up2,
            F::Up3,
            F::Up4,
            F::Up5,
            F::UpB,
            F::Side,
            F::Side1,
            F::Side2,
            F::Side3,
            F::Side4,
            F::Side5,
            F::SideB,
            F::Atk1,
            F::Atk2,
            F::Atk3,
            F::Cast1,
            F::Cast2,
            F::Cast3,
            F::Hurt,
            F::Dead,
            F::Dead2,
            F::Base,
            F::Base2,
            F::Base3,
            F::On,
            F::Open,
            F::Idle,
            F::Idle2,
            F::AtkDown1,
            F::AtkDown2,
            F::AtkDown3,
            F::AtkUp1,
            F::AtkUp2,
            F::AtkUp3,
            F::CastDown1,
            F::CastDown2,
            F::CastDown3,
            F::CastUp1,
            F::CastUp2,
            F::CastUp3,
            F::HurtDown,
            F::HurtUp,
            F::DownRight,
            F::DownRight1,
            F::DownRight2,
            F::DownRight3,
            F::DownRight4,
            F::DownRight5,
            F::DownRightB,
            F::UpRight,
            F::UpRight1,
            F::UpRight2,
            F::UpRight3,
            F::UpRight4,
            F::UpRight5,
            F::UpRightB,
            F::AtkDownRight1,
            F::AtkDownRight2,
            F::AtkDownRight3,
            F::AtkUpRight1,
            F::AtkUpRight2,
            F::AtkUpRight3,
            F::CastDownRight1,
            F::CastDownRight2,
            F::CastDownRight3,
            F::CastUpRight1,
            F::CastUpRight2,
            F::CastUpRight3,
            F::HurtDownRight,
            F::HurtUpRight,
            F::Open2,
            F::Open3,
            F::DownBlink,
            F::SideBlink,
            F::DownRightBlink,
            F::SideTurn,
            F::DownRightTurn,
            F::UpRightTurn,
            F::DownLand,
            F::UpLand,
            F::SideLand,
            F::DownRightLand,
            F::UpRightLand,
            F::Task1,
            F::Task2,
            F::Task3,
            F::Task4,
            F::TaskSide1,
            F::TaskSide2,
            F::TaskSide3,
            F::TaskSide4,
        ]
    };

    /// The frame `jane sheet` calls `name` (`"side_1"`).
    pub fn by_name(name: &str) -> Option<FrameId> {
        FrameId::ALL.iter().copied().find(|f| f.name() == name)
    }

    /// Whether this is a dead frame.
    pub const fn is_dead(self) -> bool {
        matches!(self, FrameId::Dead | FrameId::Dead2)
    }

    /// Whether this frame faces east and so is drawn mirrored for the west-facing ones (west,
    /// south-west, north-west): the side's and the diagonals' walks, breathes, blows, casts and
    /// hurts. Facing the viewer or away, an idle and the dead are never mirrored.
    pub const fn faces_east(self) -> bool {
        use FrameId as F;
        matches!(
            self,
            F::Side
                | F::Side1
                | F::Side2
                | F::Side3
                | F::Side4
                | F::Side5
                | F::SideB
                | F::Atk1
                | F::Atk2
                | F::Atk3
                | F::Cast1
                | F::Cast2
                | F::Cast3
                | F::Hurt
        ) || ((self as u8) >= (F::DownRight as u8) && (self as u8) <= (F::HurtUpRight as u8))
            || matches!(
                self,
                F::SideBlink
                    | F::DownRightBlink
                    | F::SideTurn
                    | F::DownRightTurn
                    | F::UpRightTurn
                    | F::SideLand
                    | F::DownRightLand
                    | F::UpRightLand
                    | F::TaskSide1
                    | F::TaskSide2
                    | F::TaskSide3
                    | F::TaskSide4
            )
    }
}

/// What a part of a sprite is, for swaps by role and the emissive contract (ART.md §1, §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Role {
    Hair,
    Skin,
    Coat,
    Front,
    Legs,
    Boots,
    Hat,
    Pack,
    Held,
    Eye,
    Glass,
    /// A creature's pelt or plumage.
    Fur,
    /// A creature's underside or second coat colour.
    Belly,
    /// A creature's markings.
    Mark,
    /// A prop's main material.
    Body,
    /// A prop's second material: bands, a frame, a lid.
    Trim,
    /// A flame or an ember: it emits when lit.
    Flame,
}

/// One look rendered: every frame, one size, one anchor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSet {
    /// Frame width, px.
    pub w: i32,
    /// Frame height, px.
    pub h: i32,
    /// Anchor x: where the sprite stands.
    pub ax: i32,
    /// Anchor y.
    pub ay: i32,
    /// Every frame, in [`FrameId`] order.
    pub frames: Vec<(FrameId, Canvas)>,
    /// The ramp each role was drawn in. A role's ramp is its own: no two roles share one, so a
    /// swap by role is a swap by ramp.
    pub roles: Vec<(Role, Ramp)>,
    /// The roles that may write the emissive layer.
    pub emits: Vec<Role>,
}

impl SpriteSet {
    /// The frame `id`, if the set has it.
    pub fn frame(&self, id: FrameId) -> Option<&Canvas> {
        self.frames.iter().find(|(f, _)| *f == id).map(|(_, c)| c)
    }

    /// The ramp `role` was drawn in.
    pub fn ramp_of(&self, role: Role) -> Option<Ramp> {
        self.roles.iter().find(|(r, _)| *r == role).map(|(_, r)| *r)
    }

    /// The set with `role` redrawn in `to` (ART.md §3, `Look::Swap`): the albedo of every frame
    /// has that role's tones replaced by `to`'s same tones (a dead frame's by their pallid
    /// twins); normals, emissive and height are shared.
    pub fn swap(&self, role: Role, to: Ramp) -> SpriteSet {
        let Some(from) = self.ramp_of(role) else { return self.clone() };
        let mut out = self.clone();
        for (id, c) in &mut out.frames {
            let dead = id.is_dead();
            c.remap(|ix| {
                let live = live_of(ix, from, dead);
                match Ramp::of(live) {
                    Some((r, t)) if r == from && live != ix => crate::palette::pallor(to.at(t)),
                    Some((r, t)) if r == from => to.at(t),
                    _ => ix,
                }
            });
        }
        for (r, ramp) in &mut out.roles {
            if *r == role {
                *ramp = to;
            }
        }
        out
    }

    /// FNV-1a over every frame's four layers, in order: the golden hash of the set.
    pub fn hash(&self) -> u32 {
        let mut f = jane_core::hash::Fnv::new().i32(self.w).i32(self.h).i32(self.ax).i32(self.ay);
        for (id, c) in &self.frames {
            f = f.u8(*id as u8).u32(c.hash());
        }
        f.finish()
    }
}

/// The live tone a pixel of a dead frame came from, when it is `from`'s pallid twin: dead frames
/// are pallid, so a swap looks through the pallor to the ramp underneath.
fn live_of(ix: Ix, from: Ramp, dead: bool) -> Ix {
    if !dead {
        return ix;
    }
    crate::palette::Tone::ALL.iter().map(|&t| from.at(t)).find(|&live| crate::palette::pallor(live) == ix).unwrap_or(ix)
}
