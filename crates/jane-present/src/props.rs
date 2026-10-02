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
    /// Open, one for each base (an apple tree picked keeps its crown), else one for them all.
    opens: Vec<RefId>,
    /// A made fire's wood laid, unlit (`jane_art::kit::fire_pit`: its `Open2`).
    laid: Option<RefId>,
    /// How high its lit glass glows above its foot, px: where its light shines from.
    glass: Option<u8>,
    /// A top things stand on: rows from its foot to the top's front edge, and the top's rows
    /// (`jane_art::kit::surface`).
    surface: Option<(i32, i32)>,
    /// A house's own make of it (ART-PLAN Q2, `jane_art::kit::house_variants`): a front door's
    /// paint and style, a chimney's pots; and which of a house's looks picks among them.
    house: Vec<RefId>,
    pick: HousePick,
}

/// Which of a house's looks picks a prop's house variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HousePick {
    None,
    Door,
    Chimney,
}

/// Every prop look, packed.
#[derive(Clone, Debug, Default)]
pub struct Props {
    sets: Vec<Set>,
    /// The rugs a room lays (ART-PLAN M4): `set_rug`, `set_rug_blue`, `set_runner`; and what
    /// each is laid by (a table, a bed, a hearth's stove), with which.
    rugs: [Option<SpriteId>; 3],
    rug_by: Vec<(SpriteId, Rug)>,
    /// Each item icon at ground scale, by icon.
    loot: Vec<(SpriteId, RefId)>,
}

/// Where a room lays a rug by a thing (ART-PLAN M4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rug {
    /// Under a table, a cell proud of it each way.
    Table,
    /// A runner along a bed's foot.
    Bed,
    /// Before a hearth.
    Hearth,
}

/// A made fire's state (ART.md §2.3, `jane_art::kit::fire_pit`: the fire pit `campfire_cold`
/// and the `old_grate`), as [`Props::fire_look`] picks its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FireState {
    /// Never or long unlit: old char in the ring (frame `Base`).
    #[default]
    Cold,
    /// Deadwood laid crosswise in it, not yet lit: her hold while she makes it (`Open2`).
    Laid,
    /// Burning: flames and embers, emissive (`On`).
    Lit,
    /// Burnt out: grey ash, charred stubs, a last ember or two glowing dim (`Open`).
    Ash,
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
        let (mut rugs, mut rug_by) = ([None; 3], Vec::new());
        let all = looks::family(Family::Prop).unwrap_or_default().into_iter();
        for r in all.chain(looks::family(Family::Building).unwrap_or_default()) {
            let h = r.set.h;
            // A hanging stands on its footprint's back edge, the wall's face's foot, where its
            // heights stand it (`jane_art::kit::hung`): so it is set into the face it hangs on.
            let look = match looks::find(r.name) {
                Some((_, jane_data::Look::Prop(p))) => Some(p),
                _ => None,
            };
            let hung = look.is_some_and(jane_art::kit::hung);
            match r.name {
                "set_rug" => rugs[0] = Some(r.sprite),
                "set_rug_blue" => rugs[1] = Some(r.sprite),
                "set_runner" => rugs[2] = Some(r.sprite),
                "table" => rug_by.push((r.sprite, Rug::Table)),
                "bed" => rug_by.push((r.sprite, Rug::Bed)),
                "stove" => rug_by.push((r.sprite, Rug::Hearth)),
                _ => {}
            }
            let fire = look.is_some_and(jane_art::kit::fire_pit);
            let ay = if hung { r.set.ay } else { h };
            let fh = jane_art::kit::footprint(r.sprite).map_or(1, |(_, fh)| i32::from(fh));
            let surface = look.and_then(|l| jane_art::kit::surface(l, fh));
            let mut set = Set {
                sprite: r.sprite,
                bases: Vec::new(),
                on: None,
                opens: Vec::new(),
                laid: None,
                glass: None,
                surface,
                house: Vec::new(),
                pick: HousePick::None,
            };
            if let Some(l) = look {
                let canvases =
                    jane_art::kit::house_variants(l, r.sprite, jane_art::kit::seed(r.name)).unwrap_or_default();
                set.house = canvases
                    .iter()
                    .map(|c| atlas.add_canvas(c, (0, ay as i16), h.clamp(1, 255) as u8, |_, _, t| t))
                    .collect();
                set.pick = match l.shape {
                    _ if set.house.is_empty() => HousePick::None,
                    "door" => HousePick::Door,
                    _ => HousePick::Chimney,
                };
            }
            for (f, c) in &r.set.frames {
                let id = atlas.add_canvas(c, (0, ay as i16), h.clamp(1, 255) as u8, |_, _, t| t);
                match f {
                    FrameId::On => {
                        set.on = Some(id);
                        set.glass = glow_height(c);
                    }
                    FrameId::Open2 if fire => set.laid = Some(id),
                    FrameId::Open | FrameId::Open2 | FrameId::Open3 => set.opens.push(id),
                    _ => set.bases.push(id),
                }
            }
            if !set.bases.is_empty() {
                sets.push(set);
            }
        }
        // Every item's icon at ground scale (ART.md §2.3 `small_thing`: the icon at 16 x 16),
        // standing on its canvas's last row: what a `shows_loot` prop and a drop are drawn as.
        let cat = jane_data::catalog();
        let mut icons: Vec<SpriteId> = cat.combat.items.iter().map(|i| i.icon).collect();
        icons.sort_unstable();
        icons.dedup();
        let loot = icons
            .into_iter()
            .map(|id| {
                let name = cat.sprites.get(usize::from(id.0)).copied().unwrap_or("");
                let (_, small) = crate::ui::icons::both(name);
                (id, atlas.add_canvas(&small, (0, small.h() as i16), 1, |_, _, t| t))
            })
            .collect();
        Props { sets, rugs, rug_by, loot }
    }

    /// The rug a room lays by a thing drawn as `sprite`, if it lays one: where, and its look
    /// (picked by the thing's `id`: the red rug or the blue; a runner by a bed).
    pub fn rug(&self, sprite: SpriteId, id: u32) -> Option<(Rug, RefId)> {
        let &(_, rug) = self.rug_by.iter().find(|r| r.0 == sprite)?;
        let which = match rug {
            Rug::Bed => 2,
            _ => (jane_art::hash::h32(id, 1, 0x5275_6721) % 2) as usize,
        };
        let s = self.rugs[which]?;
        Some((rug, self.look(s, id, State::default())?))
    }

    /// Item icon `icon` as a thing lying on the ground.
    pub fn loot_look(&self, icon: SpriteId) -> Option<RefId> {
        self.loot.binary_search_by_key(&icon, |e| e.0).ok().map(|i| self.loot[i].1)
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
        let i = (jane_art::hash::h32(id, 0, 0x5641_5259) % s.bases.len() as u32) as usize;
        let base = s.bases[i];
        Some(match (state.open, state.on) {
            (true, _) if !s.opens.is_empty() => s.opens[i.min(s.opens.len() - 1)],
            (_, true) if s.on.is_some() => s.on.unwrap_or(base),
            _ => base,
        })
    }

    /// The frame a door or a chimney drawn as `sprite` shows on a house of look `l` (ART-PLAN
    /// Q2): its paint and make, its pots. None for a sprite with no house variants.
    pub fn house_look(&self, sprite: SpriteId, l: &jane_art::terrain::houses::Look) -> Option<RefId> {
        let s = self.find(sprite)?;
        let v = match s.pick {
            HousePick::None => return None,
            HousePick::Door => l.door_variant(),
            HousePick::Chimney => l.chimney_variant(),
        };
        s.house.get(v as usize).copied()
    }

    /// The frame a made fire `id` drawn as `sprite` shows in `state`: `Cold` its base, `Laid`
    /// its `Open2`, `Lit` its `On`, `Ash` its `Open` (`jane_art::kit::frame_ids`). A sprite
    /// without the frame shows its base (or, lit, its `On`): a plain campfire is cold or lit.
    pub fn fire_look(&self, sprite: SpriteId, id: u32, state: FireState) -> Option<RefId> {
        let s = self.find(sprite)?;
        let base = self.look(sprite, id, State::default())?;
        Some(match state {
            FireState::Cold => base,
            FireState::Laid => s.laid.unwrap_or(base),
            FireState::Lit => s.on.unwrap_or(base),
            FireState::Ash => s.opens.first().copied().unwrap_or(base),
        })
    }

    /// Whether sprite `s` is a top things stand on: `(front, depth)`, the rows from its foot to
    /// the top's front edge and the top's rows.
    pub fn surface(&self, s: SpriteId) -> Option<(i32, i32)> {
        self.find(s).and_then(|x| x.surface)
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
    use jane_art::palette::Tone;

    #[test]
    fn a_lamp_lights_a_chest_opens_and_bases_vary_by_id() {
        let mut atlas = Atlas::with_layers(true);
        let p = Props::build(&mut atlas);
        let sprite = |n: &str| jane_art::looks::find(n).unwrap().0;
        let lamp = sprite("lamp_post");
        let (out, lit) =
            (p.look(lamp, 1, State::default()).unwrap(), p.look(lamp, 1, State { on: true, open: false }).unwrap());
        assert_ne!(out, lit);
        let g = p.glass(lamp).expect("a street lamp's glass glows");
        assert!(g > 30, "high on its post: {g}");
        let chest = sprite("chest");
        assert_ne!(p.look(chest, 3, State::default()), p.look(chest, 3, State { on: false, open: true }));
        let crate_ = sprite("crate");
        let bases: std::collections::BTreeSet<RefId> =
            (0..32).filter_map(|i| p.look(crate_, i, State::default())).collect();
        assert_eq!(bases.len(), 2, "two crates, picked by id");
        let r = atlas.get(out);
        assert_eq!((r.src.w, r.ay as i32), (16, i32::from(r.src.h)), "a prop stands on its foot row");
    }

    /// Where each lifted px of a sprite stands on the ground in the 3/4 view (a px `h` up drawn
    /// at row `y` stands on row `y + rows_up(h)`, T2's field and every tier's shadow), in rows
    /// from the sprite's top.
    fn feet(atlas: &Atlas, id: RefId) -> Vec<i32> {
        let r = *atlas.get(id);
        let page = &atlas.pages.pages[usize::from(r.page)];
        let mut out = Vec::new();
        for y in 0..r.src.h {
            for x in 0..r.src.w {
                let i = usize::from(r.src.y + y) * usize::from(page.w) + usize::from(r.src.x + x);
                let h = i32::from(page.height[i]);
                if page.albedo[i] > 1 && h > crate::shadow::GROUND {
                    out.push(i32::from(y) + crate::rows_up(h));
                }
            }
        }
        out
    }

    /// An apple tree picked shows bare: its own crown (each base its own open frame), no
    /// apples in it; filled again (its loot as spawned), the apples are back (2026-10-01).
    #[test]
    fn a_picked_apple_tree_is_bare_until_it_bears_again() {
        let mut atlas = Atlas::with_layers(true);
        let p = Props::build(&mut atlas);
        let tree = jane_art::looks::find("apple_tree").unwrap().0;
        let apples = |atlas: &Atlas, id: RefId| {
            let r = *atlas.get(id);
            let page = &atlas.pages.pages[usize::from(r.page)];
            let red = [Tone::Base, Tone::Light].map(|t| jane_art::palette::Ramp::ClothRed.at(t).0);
            (0..r.src.h)
                .flat_map(|y| (0..r.src.w).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let i = usize::from(r.src.y + y) * usize::from(page.w) + usize::from(r.src.x + x);
                    red.contains(&page.albedo[i])
                })
                .count()
        };
        let mut crowns = std::collections::BTreeSet::new();
        for id in 0..16 {
            let full = p.look(tree, id, State::default()).unwrap();
            let bare = p.look(tree, id, State { on: false, open: true }).unwrap();
            assert_ne!(full, bare, "tree {id}: picked looks picked");
            assert!(apples(&atlas, full) > 0, "tree {id}: apples on it");
            assert_eq!(apples(&atlas, bare), 0, "tree {id}: none once picked");
            crowns.insert((full, bare));
        }
        assert_eq!(crowns.len(), 2, "each crown its own bare frame");
    }

    /// A fire pit and the old grate each show four frames, one a state; lit and ash glow, cold
    /// and laid do not; the old campfire is cold or lit (2026-10-02).
    #[test]
    fn a_made_fire_is_cold_laid_lit_or_ash() {
        let mut atlas = Atlas::with_layers(true);
        let p = Props::build(&mut atlas);
        let glows = |atlas: &Atlas, id: RefId| {
            let r = *atlas.get(id);
            let page = &atlas.pages.pages[usize::from(r.page)];
            (0..r.src.h)
                .flat_map(|y| (0..r.src.w).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let i = usize::from(r.src.y + y) * usize::from(page.w) + usize::from(r.src.x + x);
                    page.emissive[i] != 0
                })
                .count()
        };
        let all = [FireState::Cold, FireState::Laid, FireState::Lit, FireState::Ash];
        for name in ["campfire_cold", "old_grate"] {
            let s = jane_art::looks::find(name).unwrap().0;
            let f: Vec<RefId> = all.iter().map(|&st| p.fire_look(s, 9, st).unwrap()).collect();
            let distinct: std::collections::BTreeSet<RefId> = f.iter().copied().collect();
            assert_eq!(distinct.len(), 4, "{name}: four frames");
            assert_eq!(f[0], p.look(s, 9, State::default()).unwrap(), "{name}: cold is its base");
            assert_eq!(f[2], p.look(s, 9, State { on: true, open: false }).unwrap(), "{name}: lit is on");
            assert_eq!(f[3], p.look(s, 9, State { on: false, open: true }).unwrap(), "{name}: ash is open");
            let g: Vec<usize> = f.iter().map(|&id| glows(&atlas, id)).collect();
            assert!(g[0] == 0 && g[1] == 0, "{name}: cold and laid are dark: {g:?}");
            assert!(g[2] > 4 * g[3] && g[3] > 0, "{name}: lit blazes, ash glows a little: {g:?}");
            assert!(p.glass(s).is_some(), "{name}: its light shines from its flames");
        }
        let camp = jane_art::looks::find("campfire").unwrap().0;
        assert_eq!(p.fire_look(camp, 1, FireState::Laid), p.fire_look(camp, 1, FireState::Cold));
        assert_ne!(p.fire_look(camp, 1, FireState::Lit), p.fire_look(camp, 1, FireState::Cold));
    }

    #[test]
    fn a_hanging_stands_on_the_face_it_hangs_on_not_a_cell_in_front_of_it() {
        // Chains, tools and the rest of a dungeon's `scatter_wall` are placed on the floor cell
        // under a wall and drawn up its face. They stood on the cell's front edge, so every tier
        // saw a post 37 px tall a cell in front of the wall, and a torch hung over it (its light
        // on the floor at the face's foot, 28 px up) threw a long hard wedge from it across the
        // floor (the mine at 17:00). They stand on the face's foot now, as the face does.
        let mut atlas = Atlas::with_layers(true);
        let p = Props::build(&mut atlas);
        let sprite = |n: &str| jane_art::looks::find(n).unwrap().0;
        for name in
            ["scatter_chains", "scatter_tools", "scatter_cobweb", "scatter_frame", "scatter_poster", "scatter_moss"]
        {
            let id = p.look(sprite(name), 1, State::default()).unwrap();
            let r = *atlas.get(id);
            assert_eq!(i32::from(r.ay), i32::from(r.src.h) - 16, "{name} stands on its cell's back edge");
            let feet = feet(&atlas, id);
            assert!(!feet.is_empty(), "{name} rises up the face");
            assert!(feet.iter().all(|&f| f == i32::from(r.ay)), "{name}: every lifted px on the face's foot: {feet:?}");
            assert!(
                i32::from(r.top) <= crate::height_of_rows(i32::from(r.ay)),
                "{name} is no taller than the face behind it"
            );
        }
        // A thing standing on the floor still stands on its footprint's front edge (its lid lies
        // behind it, on the crate).
        let id = p.look(sprite("crate"), 1, State::default()).unwrap();
        let r = *atlas.get(id);
        assert_eq!(feet(&atlas, id).into_iter().max(), Some(i32::from(r.ay) - 1), "a crate stands on its front row");
    }
}
