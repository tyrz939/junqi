//! Named patches that are more than a threat number (`AREA_DRESS`, `jane/src/world/areas.ts`).
//! Most patches of `data/areas.json` are only that: the ground is whatever the biome made it and
//! the danger is in who lives there. A few are places a quest stands on, so they are drawn: the
//! allotments are plots, the Top Field is planted, Quarry Steps has a face.
//!
//! Drawn straight after the land and before the roads and chunks, so a road that crosses one
//! simply crosses it. Each leaves the marks and rects its quests use, and may offer a slot: an
//! exact cell for a placement row (the adit's rock belongs against the face). No dice: each is
//! geometry round its patch's centre.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::action::Facing;
use jane_core::{Key, NameId, Rect, Tile};

use super::{County, centre};
use crate::kit::Kit;

/// A dresser: the kit, the patch's centre cell and its radius in cells. Returns the slots it
/// offers, by name.
type Dress = fn(&mut Kit, i32, i32, i32) -> Vec<(Key, (i32, i32))>;

/// The patches that are drawn, by their `data/areas.json` id.
const DRESS: &[(&str, Dress)] = &[("allotments", allotments), ("top_field", top_field), ("quarry_steps", quarry_steps)];

/// A name as the blueprint keys it: the catalog's when content uses it, the kit's own otherwise.
pub fn named(k: &mut Kit, s: &str) -> Key {
    jane_data::catalog().name_id(s).map_or_else(|| k.local(s), Key::Name)
}

/// Every patch the skeleton placed that has a dresser, in the skeleton's order. Their slots go
/// to `County::area_slots`.
pub fn dress_areas(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    for a in &c.sk.areas {
        let id = cat.name(a.def.id);
        let Some((_, dress)) = DRESS.iter().find(|(n, _)| *n == id) else { continue };
        for (key, at) in dress(&mut c.k, centre(a.mx), centre(a.my), i32::from(a.def.radius)) {
            if let Key::Name(n) = key {
                c.area_slots.push((n, at));
            }
        }
    }
}

/// A slot a dressed patch offers, by name.
pub fn area_slot(c: &County<'_>, name: NameId) -> Option<(i32, i32)> {
    c.area_slots.iter().find(|s| s.0 == name).map(|s| s.1)
}

/// Rows of fenced plots either side of a cinder path. Plot nine is the one nobody lets.
fn allotments(k: &mut Kit, cx: i32, cy: i32, _radius: i32) -> Vec<(Key, (i32, i32))> {
    const W: i32 = 58;
    const H: i32 = 40;
    let (x0, y0) = (cx - W / 2, cy - H / 2);
    k.fill(Rect::new(x0, y0, W, H), Tile::Grass);
    let path_y = y0 + H / 2 - 1;
    k.fill(Rect::new(x0 - 4, path_y, W + 8, 3), Tile::Dirt);
    let mut n = 1;
    for top in [true, false] {
        for col in 0..5 {
            let px = x0 + 2 + col * 11;
            let py = if top { y0 + 2 } else { path_y + 5 };
            let ph = if top { path_y - 2 - py } else { y0 + H - 2 - py };
            k.fill(Rect::new(px, py, 9, ph), Tile::Garden);
            // A fence on three sides; the side on the path is open, and plots are walkable, so
            // nothing is ever shut in.
            for i in 0..9 {
                k.set(px + i, if top { py - 1 } else { py + ph }, Tile::Fence);
            }
            for j in (0..ph).step_by(2) {
                k.set(px - 1, py + j, Tile::Fence);
            }
            if n == 9 {
                let plot = named(k, "plot_nine");
                k.rect(plot, Rect::new(px, py, 9, ph));
                k.mark(plot, px + 4, py + ph / 2, Some(Facing::South));
                let stake = named(k, "plot_nine_stake");
                k.mark(stake, px + 4, if top { py + ph + 1 } else { py - 2 }, Some(Facing::South));
            }
            n += 1;
        }
    }
    let shed = named(k, "allotment_shed");
    k.mark(shed, x0 + W - 4, path_y + 1, Some(Facing::East));
    Vec::new()
}

/// A planted field: drills of turned earth with grass between, in a disc a little over half the
/// patch across. What grows in it is not turnips.
fn top_field(k: &mut Kit, cx: i32, cy: i32, radius: i32) -> Vec<(Key, (i32, i32))> {
    let r = radius * 11 / 20;
    for y in cy - r..=cy + r {
        for x in cx - r..=cx + r {
            if (x - cx) * (x - cx) + (y - cy) * (y - cy) > r * r {
                continue;
            }
            if matches!(k.get(x, y), Tile::Water | Tile::Sand) {
                continue;
            }
            k.set(x, y, if (y - cy + r) % 4 < 2 { Tile::Garden } else { Tile::Grass });
        }
    }
    let m = named(k, "top_field");
    k.mark(m, cx, cy, Some(Facing::South));
    Vec::new()
}

/// A worked face with a ledge in front of it, and an adit somebody has put a rock across.
fn quarry_steps(k: &mut Kit, cx: i32, cy: i32, _radius: i32) -> Vec<(Key, (i32, i32))> {
    k.fill(Rect::new(cx - 16, cy - 6, 33, 16), Tile::Dirt);
    k.fill(Rect::new(cx - 14, cy - 12, 29, 7), Tile::Cliff);
    // The steps: two lower ledges either side, so the face reads as cut, not fallen.
    k.fill(Rect::new(cx - 22, cy - 8, 7, 4), Tile::Cliff);
    k.fill(Rect::new(cx + 16, cy - 8, 7, 4), Tile::Cliff);
    // Its rect is data (`placements/lowfields.json` `quarry_top_ledge`), centred on this mark.
    let top = named(k, "quarry_top");
    k.mark(top, cx, cy, Some(Facing::North));
    let adit = named(k, "quarry_adit");
    k.mark(adit, cx, cy - 2, Some(Facing::North));
    // The rock across the adit stands flush under the face: three cells wide, two deep.
    vec![(adit, (cx - 1, cy - 5))]
}
