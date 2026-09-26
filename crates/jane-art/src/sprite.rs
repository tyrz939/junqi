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
        }
    }

    /// Whether this is a dead frame.
    pub const fn is_dead(self) -> bool {
        matches!(self, FrameId::Dead | FrameId::Dead2)
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
