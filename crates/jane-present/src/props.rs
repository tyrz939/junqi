//! The prop kit in the atlas (ART.md §2.3, §5, PRESENTATION.md §1.11): every prop look, all its
//! frames packed at boot in all four layers, and the pick of a frame for a prop this tick:
//! `open` for a chest looted or a lid lifted, `on` for a lamp alight, a lever thrown or a plate
//! pressed, else one of its bases by the prop's id. A prop whose sprite has no look yet keeps
//! its stand-in.

use jane_art::looks::{self, Family};
use jane_art::palette::Ix;
use jane_art::sprite::FrameId;
use jane_core::ids::SpriteId;

use crate::atlas::{Atlas, RefId};

/// One prop look's frames in the atlas.
#[derive(Clone, Debug)]
struct Set {
    sprite: SpriteId,
    bases: Vec<RefId>,
    on: Option<RefId>,
    open: Option<RefId>,
    /// How high its lit glass glows above its foot, px: where its light shines from.
    glass: Option<u8>,
}

/// Every prop look, packed.
#[derive(Clone, Debug, Default)]
pub struct Props {
    sets: Vec<Set>,
}

/// A prop's state as the frame pick reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct State {
    /// Lit, thrown or pressed.
    pub on: bool,
    /// Its lid up, its loot taken.
    pub open: bool,
}

impl Props {
    /// Renders every prop look and packs its frames, each anchored on its foot (the row under
    /// its footprint's front edge), as the stand-ins are. A look that fails to render is left
    /// out, and its props keep their stand-in.
    pub fn build(atlas: &mut Atlas) -> Props {
        let mut sets = Vec::new();
        for r in looks::family(Family::Prop).unwrap_or_default() {
            let h = r.set.h;
            let mut set = Set { sprite: r.sprite, bases: Vec::new(), on: None, open: None, glass: None };
            for (f, c) in &r.set.frames {
                let id = atlas.add_canvas(c, (0, h as i16), h.clamp(1, 255) as u8, |_, _, t| t);
                match f {
                    FrameId::On => {
                        set.on = Some(id);
                        set.glass = glow_height(c);
                    }
                    FrameId::Open => set.open = Some(id),
                    _ => set.bases.push(id),
                }
            }
            if !set.bases.is_empty() {
                sets.push(set);
            }
        }
        Props { sets }
    }

    fn find(&self, sprite: SpriteId) -> Option<&Set> {
        self.sets.iter().find(|s| s.sprite == sprite)
    }

    /// Whether sprite `s` is drawn from the kit.
    pub fn has(&self, s: SpriteId) -> bool {
        self.find(s).is_some()
    }

    /// The frame prop `id` drawn as `sprite` shows in `state`: open over on over a base picked
    /// by its id (ART.md §1, the TS build's rule).
    pub fn look(&self, sprite: SpriteId, id: u32, state: State) -> Option<RefId> {
        let s = self.find(sprite)?;
        let base = s.bases[(jane_art::hash::h32(id, 0, 0x5641_5259) % s.bases.len() as u32) as usize];
        Some(match (state.open, state.on) {
            (true, _) if s.open.is_some() => s.open.unwrap_or(base),
            (_, true) if s.on.is_some() => s.on.unwrap_or(base),
            _ => base,
        })
    }

    /// How high sprite `s`'s lit glass glows above its foot, px.
    pub fn glass(&self, s: SpriteId) -> Option<u8> {
        self.find(s).and_then(|x| x.glass)
    }
}

/// The glow's middle row, as a height above the canvas's foot (its last row + 1).
fn glow_height(c: &jane_art::Canvas) -> Option<u8> {
    let rows: Vec<i32> = (0..c.h()).filter(|&y| (0..c.w()).any(|x| c.emissive_at(x, y) != Ix::CLEAR)).collect();
    if rows.is_empty() {
        return None;
    }
    let mid = rows.iter().sum::<i32>() / rows.len() as i32;
    Some((c.h() - mid).clamp(1, 255) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lamp_lights_a_chest_opens_and_bases_vary_by_id() {
        let mut atlas = Atlas::with_layers(true);
        let p = Props::build(&mut atlas);
        let sprite = |n: &str| jane_art::looks::find(n).unwrap().0;
        let lamp = sprite("lamp_post");
        let (out, lit) = (p.look(lamp, 1, State::default()).unwrap(), p.look(lamp, 1, State { on: true, open: false }).unwrap());
        assert_ne!(out, lit);
        let g = p.glass(lamp).expect("a street lamp's glass glows");
        assert!(g > 30, "high on its post: {g}");
        let chest = sprite("chest");
        assert_ne!(p.look(chest, 3, State::default()), p.look(chest, 3, State { on: false, open: true }));
        let crate_ = sprite("crate");
        let bases: std::collections::BTreeSet<RefId> = (0..32).filter_map(|i| p.look(crate_, i, State::default())).collect();
        assert_eq!(bases.len(), 2, "two crates, picked by id");
        let r = atlas.get(out);
        assert_eq!((r.src.w, r.ay as i32), (16, i32::from(r.src.h)), "a prop stands on its foot row");
    }
}
