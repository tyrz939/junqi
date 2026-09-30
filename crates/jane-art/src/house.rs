//! The building painter (ART.md §2.4, §8 step 6): `house(look)` draws a building prop (a
//! cottage, a farmhouse, a barn, the inn, a shed, a reed hut, the steeple) as one sprite on its
//! footprint, seen in the 3/4 view: a front wall `storeys` high standing on the footprint's
//! front edge, and over it the roof's front slope running back to the ridge.
//!
//! **Who owns what** (decided 2026-09-27): a building that is a *prop* (the county's cottages,
//! the barn, the inn: rows with a footprint the sim collides) is drawn here, as a sprite that
//! the renderer y-sorts with the people, so she walks behind its roof and in front of its wall.
//! A building made of *tiles* (the town's terraces, Julie's house: `HouseWall` and `HouseRoof`
//! cells) is the terrain painter's (`terrain`, §2.6). The two share their materials by ramp and
//! their language (coursed slate and tile, thatch in clusters, a stone footing, lit windows
//! that emit), so a cottage prop and a tiled house stand in one street without a seam in style.
//!
//! Frames: `base` by day, windows dark and reflecting; `on` at night (a lit building, when the
//! view says its light shows), windows warm and emitting, each a little different by a hashed
//! offset. Heights are true: the wall stands up row by row from the foot, and the roof lands on
//! the house under it, rising from the eave to the ridge.

use jane_core::grid::Rect;
use jane_core::ids::SpriteId;
use jane_data::{HouseLook, HouseStyle, Roofing, Walling};

use crate::canvas::{CELL_PX, Canvas, FLAT, StrokeKind, Z, normal};
use crate::kit::parts::{self, blocks, planks, post};
use crate::palette::{Ix, Ramp, Tone};
use crate::sprite::{FrameId, Role, SpriteSet};

/// The ramps a building is drawn in.
#[derive(Clone, Copy, Debug)]
struct Stuff {
    look: HouseLook,
    roof: Ramp,
    wall: Ramp,
    door: Ramp,
    trim: Ramp,
    seed: u32,
}

fn roof_ramp(r: Roofing) -> Ramp {
    match r {
        Roofing::Thatch => Ramp::Thatch,
        Roofing::Slate => Ramp::Slate,
        Roofing::Tile => Ramp::RoofTile,
        Roofing::Tin => Ramp::Iron,
        Roofing::Reed => Ramp::HairFair,
    }
}

fn wall_ramp(w: Walling) -> Ramp {
    match w {
        Walling::Plaster | Walling::Timber => Ramp::Plaster,
        Walling::Stone => Ramp::Stone,
        Walling::Brick => Ramp::Brick,
        Walling::Board => Ramp::WoodOak,
        Walling::Reed => Ramp::Reed,
    }
}

/// Render every frame of building `look`, drawn as `sprite` (whose rows give the footprint).
pub fn render(look: &HouseLook, sprite: SpriteId, seed: u32) -> Result<SpriteSet, String> {
    let (fw, fh) = crate::kit::footprint(sprite)?;
    // The walls stand above the footprint's front edge and the roof covers the footprint over
    // them, so the canvas is the footprint, the walls' height and `rise` more for a chimney.
    let walls = if look.style == HouseStyle::Steeple { 0 } else { storey(look.style) * i32::from(look.storeys) };
    let (w, h) = (i32::from(fw) * CELL_PX, i32::from(fh) * CELL_PX + walls + i32::from(look.rise));
    let r = crate::person::ramp;
    let s = Stuff {
        look: *look,
        roof: roof_ramp(look.roof),
        wall: wall_ramp(look.wall),
        door: r(look.door)?,
        trim: r(look.trim)?,
        seed,
    };
    let mut frames = Vec::new();
    let ids: &[FrameId] = if look.lit { &[FrameId::Base, FrameId::On] } else { &[FrameId::Base] };
    for &id in ids {
        let mut c = Canvas::new(w, h);
        house(&mut c, &s, i32::from(fh) * CELL_PX, id == FrameId::On);
        frames.push((id, c));
    }
    let emits = if look.lit { vec![Role::Glass] } else { Vec::new() };
    Ok(SpriteSet {
        w,
        h,
        ax: 0,
        ay: h - i32::from(fh) * CELL_PX,
        frames,
        roles: vec![(Role::Body, s.wall), (Role::Trim, s.roof)],
        emits,
    })
}

/// How tall one storey's wall stands, px: a door and a window's height with the lintel over
/// them, beside a person 32 px tall.
fn storey(style: HouseStyle) -> i32 {
    match style {
        HouseStyle::Barn => 36,
        // A shed's boards stand a door's height, near hers: it reads as a hut of planks, not a
        // tent (the owner, 2026-10-01).
        HouseStyle::Shed => 26,
        HouseStyle::Hut => 22,
        _ => 28,
    }
}

fn house(c: &mut Canvas, s: &Stuff, depth: i32, lit: bool) {
    let (w, h) = (c.w(), c.h());
    let foot = h - 1;
    let look = &s.look;
    if look.style == HouseStyle::Steeple {
        steeple(c, s, lit);
        return finish(c, s, foot, depth, foot - 1, 0);
    }
    // The lean-to takes a quarter of the width at the east end.
    let lean = if look.lean_to { (w / 4).max(16) } else { 0 };
    let (x0, x1) = (2, w - 3 - lean);
    let wall_h = storey(look.style) * i32::from(look.storeys);
    let eave = foot - wall_h;
    // The roof covers the footprint over the walls: its front slope from the eave up to the
    // ridge (three fifths of it, facing the viewer and the sun), its back slope beyond, seen
    // foreshortened and in shade.
    let back = (eave - depth).max(2);
    // A shed's roof is one pent of tin falling from the back to the front, square at its ends:
    // no ridge, no hips, nothing down to the ground.
    let shed = look.style == HouseStyle::Shed;
    let slope = if shed { eave - back } else { (eave - back) * 3 / 5 };
    let ridge = eave - slope;
    c.ao_contact(Rect::new(x0 - 2, foot - 3, x1 - x0 + 5 + lean, 6), 1);
    wall(c, s, Rect::new(x0, eave + 1, x1 - x0 + 1, wall_h));
    openings(c, s, x0, x1, eave, foot, lit);
    back_slope(c, s, x0 - 2, x1 + 2, back, ridge);
    roof(c, s, x0 - 2, x1 + 2, ridge, eave + 2);
    // The roof's ends: slate is gabled (a barge board down each end), and a cottage or a
    // farmhouse turns a gable to the road over its door; thatch, reed, tile and tin are hipped
    // (the ends slope back; thatch soft at its corners).
    let thatched = matches!(look.roof, Roofing::Thatch | Roofing::Reed);
    if look.roof == Roofing::Slate || shed {
        barges(c, s, x0 - 2, x1 + 2, back, eave + 2);
        if matches!(look.style, HouseStyle::Cottage | HouseStyle::Farmhouse | HouseStyle::Inn) && !look.dormers {
            front_gable(c, s, (x0 + x1) / 2 + door_offset(s), eave, ridge, lit);
        }
    } else {
        hips(c, s, x0 - 2, x1 + 2, back, ridge, eave + 2, thatched);
    }
    if look.dormers && look.storeys < 3 {
        for dx in [x0 + (x1 - x0) / 4, x0 + 3 * (x1 - x0) / 4] {
            dormer(c, s, dx, ridge + slope / 2, lit);
        }
    }
    if !shed {
        chimney(c, s, x0 + (x1 - x0) * if s.seed & 1 == 0 { 1 } else { 4 } / 5, ridge);
    }
    if lean > 0 {
        let lx = x1 + 3;
        let lh = wall_h * 2 / 3;
        wall(c, s, Rect::new(lx, foot - lh + 1, lean - 3, lh));
        roof(c, s, lx - 1, w - 2, foot - lh - slope / 2, foot - lh + 2);
    }
    if look.porch {
        porch(c, s, (x0 + x1) / 2 + door_offset(s), foot);
    }
    if look.silhouette {
        c.remap(|ix| if ix.is_opaque() { Ix::SEAM } else { ix });
    }
    finish(c, s, foot, depth, eave, ridge);
}

/// The door's offset from the wall's middle, by the seed: no two cottages the same.
fn door_offset(s: &Stuff) -> i32 {
    [-8, 0, 6, -4][(s.seed >> 3 & 3) as usize]
}

/// A wall in its material over `r`: calm plaster with a footing, timber framing, coursed stone
/// or brick, or boards; the eave's shadow along its top.
fn wall(c: &mut Canvas, s: &Stuff, r: Rect) {
    let ramp = s.wall;
    match s.look.wall {
        Walling::Stone => blocks(c, r, ramp, 4, 8, false, s.seed, 3),
        Walling::Brick => blocks(c, r, ramp, 3, 6, false, s.seed, 3),
        Walling::Board | Walling::Reed => planks(c, r, ramp, (r.w / 4).max(2), false, false, s.seed, 3),
        Walling::Plaster | Walling::Timber => {
            parts::fill(c, r, ramp.at(Tone::Base), parts::south(), 3);
            // A few long soft stains of weather down the plaster, in clusters.
            for i in 0..(r.w / 16).max(1) {
                let hh = parts::hash(s.seed, i, 5);
                let x = r.x + 3 + (hh % (r.w - 6).max(1) as u32) as i32;
                for y in r.y + 4..r.y + 4 + 3 + (hh >> 8 & 3) as i32 {
                    c.tint(x, y, ramp, Tone::Mid);
                    c.tint(x + 1, y + 1, ramp, Tone::Mid);
                }
            }
            if s.look.wall == Walling::Timber {
                for x in (r.x..r.right()).step_by(14) {
                    c.fill_normal(Rect::new(x, r.y, 2, r.h), s.trim.at(Tone::Base), parts::south(), 4);
                    c.vline(x, r.y, r.bottom() - 1, s.trim.at(Tone::Light), 4);
                }
                c.fill_normal(Rect::new(r.x, r.y + r.h / 2, r.w, 2), s.trim.at(Tone::Base), parts::south(), 4);
                c.fill_normal(Rect::new(r.right() - 2, r.y, 2, r.h), s.trim.at(Tone::Mid), parts::south(), 4);
            }
            // A stone footing.
            blocks(c, Rect::new(r.x, r.bottom() - 4, r.w, 4), Ramp::Stone, 4, 7, false, s.seed ^ 9, 3);
        }
    }
    // The eave's shadow on the wall's top rows.
    c.shade(Rect::new(r.x - 4, r.y - 2, r.w + 8, 6), ramp, 1);
}

/// The door and the windows along the wall from `x0` to `x1`, a row of windows a storey.
fn openings(c: &mut Canvas, s: &Stuff, x0: i32, x1: i32, eave: i32, foot: i32, lit: bool) {
    let look = &s.look;
    let st = storey(look.style);
    let mid = (x0 + x1) / 2 + door_offset(s);
    if look.style == HouseStyle::Shed {
        return shed_front(c, s, (x0, x1), mid, foot, lit);
    }
    let barn = look.style == HouseStyle::Barn;
    // The door (a barn's great doors).
    let (dw, dh) = if barn { (22, 24) } else { (8, 14.min(st - 4)) };
    let door = Rect::new(mid - dw / 2, foot - dh + 1, dw, dh);
    c.fill_normal(Rect::new(door.x - 1, door.y - 1, door.w + 2, door.h + 1), s.trim.at(Tone::Shade), parts::south(), 4);
    planks(c, door, s.door, if barn { 6 } else { 2 }, false, false, s.seed ^ 3, 5);
    if barn {
        c.vline(mid, door.y, door.bottom() - 1, s.door.at(Tone::Deep), 6);
        c.line((door.x + 1, door.bottom() - 2), (mid - 2, door.y + 1), s.door.at(Tone::Light), 1, 6);
        c.line((mid + 1, door.y + 1), (door.right() - 2, door.bottom() - 2), s.door.at(Tone::Light), 1, 6);
    } else {
        c.dot(door.right() - 3, door.y + dh / 2, Ramp::Brass.at(Tone::Light), 6);
        c.fill_normal(Rect::new(door.x - 2, foot - 1, door.w + 4, 2), Ramp::Stone.at(Tone::Light), FLAT, 2);
    }
    if look.boarded {
        boards(c, s, door);
    }
    // Windows: a row a storey, spaced out and clear of the door.
    let (ww, wh) = if look.style == HouseStyle::Inn { (10, 9) } else { (8, 8) };
    for k in 0..i32::from(look.storeys) {
        let row_foot = foot - k * st;
        let wy = row_foot - st + 5;
        let mut x = x0 + 5;
        let mut n = 0;
        while x + ww < x1 - 3 {
            let clear = x + ww + 3 < door.x || x > door.right() + 3 || k > 0;
            if clear && !(barn && k == 0) {
                window(c, s, Rect::new(x, wy, ww, wh), lit && !look.boarded, n + k * 7);
            }
            x += ww + 12;
            n += 1;
        }
    }
    let _ = eave;
}

/// A shed's front: a ledged and braced plank door with a hasp and a padlock on it (it is always
/// locked: "Padlocked. The padlock is new and the shed is not."), and one small window.
fn shed_front(c: &mut Canvas, s: &Stuff, (x0, x1): (i32, i32), mid: i32, foot: i32, lit: bool) {
    let st = storey(s.look.style);
    let (dw, dh) = (10, st - 5);
    let door = Rect::new(mid - dw / 2, foot - dh + 1, dw, dh);
    c.fill_normal(Rect::new(door.x - 1, door.y - 1, door.w + 2, door.h + 1), s.trim.at(Tone::Deep), parts::south(), 4);
    planks(c, door, s.door, 3, false, false, s.seed ^ 3, 5);
    // The ledges across it and the brace between them, each lit along its top.
    for y in [door.y + 3, door.bottom() - 5] {
        c.fill_normal(Rect::new(door.x, y, door.w, 2), s.door.at(Tone::Base), parts::south(), 6);
        c.hline(door.x, door.right() - 1, y, s.door.at(Tone::Light), 6);
    }
    c.line((door.x + 1, door.bottom() - 6), (door.right() - 2, door.y + 5), s.door.at(Tone::Light), 1, 6);
    // The hasp over the door's edge onto the frame, and the padlock hung on its staple: brass,
    // a lit top and the shackle's iron loop.
    let hy = door.y + dh / 2 - 1;
    c.fill_normal(Rect::new(door.right() - 4, hy, 6, 2), Ramp::Iron.at(Tone::Base), parts::south(), 7);
    c.hline(door.right() - 4, door.right() + 1, hy, Ramp::Iron.at(Tone::Light), 7);
    let lock = Rect::new(door.right() - 1, hy + 3, 4, 4);
    c.dot(lock.x, hy + 2, Ramp::Iron.at(Tone::Light), 8);
    c.dot(lock.x + 3, hy + 2, Ramp::Iron.at(Tone::Mid), 8);
    c.fill_normal(lock, Ramp::Brass.at(Tone::Base), parts::south(), 8);
    c.hline(lock.x, lock.right() - 1, lock.y, Ramp::Brass.at(Tone::High), 8);
    c.dot(lock.x + 1, lock.y + 2, Ramp::Brass.at(Tone::Deep), 8);
    // A plank sill under the door.
    c.fill_normal(Rect::new(door.x - 1, foot - 1, door.w + 2, 2), s.trim.at(Tone::Base), FLAT, 2);
    // One small window, on whichever side has the room.
    let (ww, wh) = (7, 6);
    let wx =
        if door.x - x0 >= ww + 8 { x0 + (door.x - x0 - ww) / 2 } else { door.right() + (x1 - door.right() - ww) / 2 };
    window(c, s, Rect::new(wx, foot - st + 6, ww, wh), lit && !s.look.boarded, 0);
}

/// A window in its frame: four panes round a cross, a sill; lit, warm and emitting a little
/// unevenly; boarded, planks across it.
fn window(c: &mut Canvas, s: &Stuff, r: Rect, lit: bool, n: i32) {
    c.fill_normal(Rect::new(r.x - 1, r.y - 1, r.w + 2, r.h + 2), s.trim.at(Tone::Base), parts::south(), 5);
    let pane = Rect::new(r.x, r.y, r.w, r.h);
    if lit {
        // One window in five is dark even at night: a room nobody is in.
        let hh = parts::hash(s.seed, n, 11);
        if hh % 5 == 0 {
            parts::glass(c, pane, false, 6);
        } else {
            c.set_emitting(true);
            let t = [Tone::Light, Tone::High, Tone::Light, Tone::Lift][(hh >> 4 & 3) as usize];
            c.fill_normal(pane, Ramp::GlassLit.at(t), parts::south(), 6);
            c.hline(pane.x, pane.right() - 1, pane.bottom() - 1, Ramp::GlassLit.at(t.step(-1)), 6);
            c.set_emitting(false);
        }
    } else {
        parts::glass(c, pane, false, 6);
    }
    c.vline(r.x + r.w / 2, r.y, r.bottom() - 1, s.trim.at(Tone::Light), 7);
    c.hline(r.x, r.right() - 1, r.y + r.h / 2, s.trim.at(Tone::Light), 7);
    c.fill_normal(Rect::new(r.x - 2, r.bottom() + 1, r.w + 4, 1), s.trim.at(Tone::Light), FLAT, 7);
    if s.look.boarded {
        boards(c, s, r);
    }
}

/// Planks nailed across `r`.
fn boards(c: &mut Canvas, s: &Stuff, r: Rect) {
    for (i, y) in (r.y + 1..r.bottom() - 1).step_by(4).enumerate() {
        let b = Rect::new(r.x - 1, y, r.w + 2, 3);
        planks(c, b, Ramp::WoodPale, 1, true, false, s.seed ^ i as u32, 8);
        c.dot(r.x, y + 1, Ramp::Iron.at(Tone::Light), 9);
    }
}

/// The roof's front slope from the eave (row `eave`, `x0..=x1`) up to the ridge (row `ridge`), in
/// its material's courses, the ridge capped along its top.
fn roof(c: &mut Canvas, s: &Stuff, x0: i32, x1: i32, ridge: i32, eave: i32) {
    let r = Rect::new(x0, ridge, x1 - x0 + 1, eave - ridge + 1);
    let ramp = s.roof;
    let n = normal(0, 46);
    match s.look.roof {
        Roofing::Thatch | Roofing::Reed => {
            // Thatch: a deep soft mass with a rounded eave, the straw in downward strokes, a
            // patterned ridge.
            let mut m = Canvas::new(c.w(), c.h());
            m.fill_rect(Rect::new(r.x, r.y, r.w, r.h - 2), Ix::INK, 1);
            m.ellipse(Rect::new(r.x, r.bottom() - 6, r.w, 6), Ix::INK, 1);
            c.inflate(&m, ramp, 4, Z::new(8, 10));
            c.retone(
                ramp,
                [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::Light],
            );
            // The straw laid in courses: each a lit lip over a band of shade where the next
            // course overhangs it, and the reed's grain in short streaks of two or three px.
            for y in (r.y + 6..r.bottom() - 4).step_by(6) {
                for x in r.x..r.right() {
                    c.tint(x, y, ramp, Tone::Mid);
                    c.tint(x, y + 1, ramp, Tone::Lift);
                }
            }
            for i in 0..(r.w * r.h / 40).max(4) {
                let hh = parts::hash(s.seed, i, 17);
                let (x, y) = (r.x + (hh % r.w as u32) as i32, r.y + 2 + ((hh >> 8) % (r.h - 4).max(1) as u32) as i32);
                let t = if hh >> 20 & 1 == 0 { Tone::Mid } else { Tone::Light };
                for d in 0..2 + (hh >> 21 & 1) as i32 {
                    c.tint(x, y + d, ramp, t);
                }
            }
            // The ridge: a cap of bound straw with its zigzag of pegged liggers.
            for x in (r.x + 2..r.right() - 2).step_by(4) {
                c.tint(x, r.y + 3, ramp, Tone::Shade);
                c.tint(x + 1, r.y + 4, ramp, Tone::Shade);
                c.tint(x + 2, r.y + 3, ramp, Tone::Shade);
            }
            c.hline(r.x + 1, r.right() - 2, r.y + 1, ramp.at(Tone::Light), 11);
            let _ = StrokeKind::Hair;
        }
        _ => {
            parts::fill(c, r, ramp.at(Tone::Base), n, 8);
            let tin = s.look.roof == Roofing::Tin;
            for y in r.y..r.bottom() {
                let row = (y - r.y) % 3;
                let course = (y - r.y) / 3;
                for x in r.x..r.right() {
                    let off = if course % 2 == 1 { 3 } else { 0 };
                    let t = if tin {
                        [Tone::Light, Tone::Base, Tone::Mid][((x - r.x) % 3) as usize]
                    } else if row == 2 {
                        Tone::Shade
                    } else if (s.look.roof == Roofing::Slate && (x - r.x + off) % 6 == 5)
                        || (s.look.roof == Roofing::Tile && (x - r.x + off) % 4 == 3)
                    {
                        Tone::Mid
                    } else if row == 0 {
                        Tone::Lift
                    } else {
                        Tone::Base
                    };
                    c.put(x, y, ramp.at(t), n, 8);
                }
            }
            // A few slates or tiles lighter or darker than the rest: weather, in clusters.
            for i in 0..(r.w * r.h / 120).max(2) {
                let hh = parts::hash(s.seed, i, 13);
                let (x, y) =
                    (r.x + (hh % r.w as u32) as i32, r.y + 1 + ((hh >> 8) % (r.h - 2).max(1) as u32) as i32 / 3 * 3);
                let t = if hh >> 20 & 1 == 0 { Tone::Light } else { Tone::Mid };
                for dx in 0..3 {
                    c.tint(x + dx, y, ramp, t);
                    c.tint(x + dx, y + 1, ramp, t);
                }
            }
            // The ridge's capping.
            c.fill_normal(Rect::new(r.x, r.y - 1, r.w, 2), ramp.at(Tone::Light), FLAT, 9);
            c.hline(r.x, r.right() - 1, r.y - 1, ramp.at(Tone::High), 9);
        }
    }
    // The eave's edge: a dark line where the roof overhangs the wall.
    c.hline(r.x, r.right() - 1, r.bottom(), ramp.at(Tone::Deep), 8);
}

/// The roof's back slope from the ridge (row `ridge`) back to row `top`: the same courses,
/// facing away from the sun, a band darker.
fn back_slope(c: &mut Canvas, s: &Stuff, x0: i32, x1: i32, top: i32, ridge: i32) {
    if ridge <= top {
        return;
    }
    let r = Rect::new(x0, top, x1 - x0 + 1, ridge - top);
    let thatch = matches!(s.look.roof, Roofing::Thatch | Roofing::Reed);
    parts::fill(c, r, s.roof.at(if thatch { Tone::Mid } else { Tone::Shade }), normal(0, -70), 8);
    if thatch {
        for y in (r.y + 2..r.bottom()).step_by(5) {
            c.hline(r.x, r.right() - 1, y, s.roof.at(Tone::Shade), 8);
        }
    } else {
        // Slates, tiles or tin in the shade: the same courses as the front, their joints
        // staggered, each course's lower edge catching the sky a tone up, one slate in five
        // a tone of its own, so the back reads as laid and not as a flat band.
        let tin = s.look.roof == Roofing::Tin;
        let (pitch, len) = if s.look.roof == Roofing::Tile { (3, 4) } else { (3, 6) };
        for y in r.y..r.bottom() {
            let (row, course) = ((y - r.y) % pitch, (y - r.y) / pitch);
            for x in r.x..r.right() {
                let off = if course % 2 == 1 { len / 2 } else { 0 };
                let slate = parts::hash(s.seed, (x - r.x + off) / len, course + 40);
                let t = if tin {
                    [Tone::Mid, Tone::Shade, Tone::Shade][((x - r.x) % 3) as usize]
                } else if row == pitch - 1 || (x - r.x + off) % len == len - 1 {
                    Tone::Deep
                } else if row == 0 || slate % 5 == 0 {
                    Tone::Mid
                } else {
                    Tone::Shade
                };
                c.put(x, y, s.roof.at(t), normal(0, -70), 8);
            }
        }
    }
    if thatch {
        // The straw's grain in the shade, a few streaks.
        for i in 0..(r.w * r.h / 50).max(2) {
            let hh = parts::hash(s.seed, i, 19);
            let (x, y) = (r.x + (hh % r.w as u32) as i32, r.y + ((hh >> 8) % r.h.max(1) as u32) as i32);
            c.tint(x, y, s.roof, Tone::Base);
            c.tint(x, y + 1, s.roof, Tone::Base);
        }
    }
    c.hline(r.x, r.right() - 1, r.y, s.roof.at(Tone::Base), 8);
}

/// Hip the roof over `x0..=x1` (back eave row `back`, ridge `ridge`, front eave `eave`): each end
/// a sloping face, a triangle from the eave's corners to the ridge's end, the west one lit and
/// the east one in shade, its hip rafter a line; `soft` (thatch) rounds the top corners.
#[allow(clippy::too_many_arguments)]
fn hips(c: &mut Canvas, s: &Stuff, x0: i32, x1: i32, back: i32, ridge: i32, eave: i32, soft: bool) {
    let ramp = s.roof;
    let reach = ((x1 - x0) / 6).clamp(6, 20).min(ridge - back + (eave - ridge) / 2);
    let inset = |y: i32| -> i32 {
        let (d, span) = if y <= ridge { (y - back, ridge - back) } else { (eave - y, eave - ridge) };
        reach * d.clamp(0, span.max(1)) / span.max(1)
    };
    for y in back..=eave {
        let k = inset(y);
        for x in x0..=x1 {
            if !c.get(x, y).is_opaque() {
                continue;
            }
            let (west, east) = (x < x0 + k, x > x1 - k);
            if !(west || east) {
                continue;
            }
            let edge = x == x0 + k - 1 || x == x1 - k + 1;
            let t = match (west, edge, y > ridge) {
                (true, true, _) => Tone::Light,
                (true, false, true) => Tone::Lift,
                (true, false, false) => Tone::Base,
                (false, true, _) => Tone::Deep,
                (false, false, true) => Tone::Mid,
                (false, false, false) => Tone::Shade,
            };
            let n = normal(if west { -70 } else { 70 }, if y > ridge { 30 } else { -30 });
            c.put(x, y, ramp.at(t), n, 9);
        }
    }
    if soft {
        // Thatch rounds over its corners: a quarter circle off each top corner.
        let r = reach.min(8);
        for dy in 0..r {
            for dx in 0..r {
                let (u, v) = (r - dx, r - dy);
                if u * u + v * v > r * r {
                    c.clear_px(x0 + dx, back + dy);
                    c.clear_px(x1 - dx, back + dy);
                }
            }
        }
    }
}

/// Barge boards down a gabled roof's two ends, from the back eave to the front: a trim line
/// lit on the west end and in shade on the east, a px proud of the roof.
fn barges(c: &mut Canvas, s: &Stuff, x0: i32, x1: i32, back: i32, eave: i32) {
    for y in back..=eave {
        c.put(x0, y, s.trim.at(Tone::Light), normal(-60, 0), 10);
        c.put(x0 + 1, y, s.trim.at(Tone::Base), normal(-30, 0), 10);
        c.put(x1, y, s.trim.at(Tone::Shade), normal(60, 0), 10);
        c.put(x1 - 1, y, s.trim.at(Tone::Mid), normal(30, 0), 10);
    }
}

/// A gable turned to the road over the door at `cx`: its triangle of wall rising above the
/// eave with a small window, its two roof slopes along its top edges, lit on the west and in
/// shade on the east, a finial at its peak.
fn front_gable(c: &mut Canvas, s: &Stuff, cx: i32, eave: i32, ridge: i32, lit: bool) {
    let half = (eave - ridge + 2).clamp(9, 17);
    let peak = eave - half;
    let face = [(cx - half + 2, eave + 1), (cx + half - 3, eave + 1), (cx, peak + 2), (cx - 1, peak + 2)];
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(&face, Ix::INK, 1);
    for y in peak..=eave + 1 {
        for x in cx - half..cx + half {
            if m.get(x, y).is_opaque() {
                c.put(x, y, s.wall.at(Tone::Base), parts::south(), 11);
            }
        }
    }
    if s.look.wall == Walling::Timber {
        c.vline(cx - 1, peak + 3, eave, s.trim.at(Tone::Base), 11);
    }
    let win = Rect::new(cx - 3, eave - half / 2, 6, 6);
    window(c, s, win, lit, 1);
    // The two slopes: four px of roof along each upper edge, overhanging the eave by two, and a
    // barge board of trim under each.
    let (west, east) =
        ([Tone::High, Tone::Light, Tone::Lift, Tone::Base], [Tone::Base, Tone::Mid, Tone::Shade, Tone::Deep]);
    for i in 0..4 {
        c.line((cx - 1, peak - 1 + i), (cx - half - 2, eave + 1 + i), s.roof.at(west[i as usize]), 1, 12);
        c.line((cx, peak - 1 + i), (cx + half + 1, eave + 1 + i), s.roof.at(east[i as usize]), 1, 12);
    }
    c.line((cx - 1, peak + 3), (cx - half + 1, eave + 1), s.trim.at(Tone::Light), 1, 12);
    c.line((cx, peak + 3), (cx + half - 2, eave + 1), s.trim.at(Tone::Shade), 1, 12);
    c.fill_normal(Rect::new(cx - 1, peak - 2, 2, 2), s.trim.at(Tone::Light), FLAT, 13);
}

/// A dormer on the roof's slope at `(cx, y)`: a little gabled face with its window.
fn dormer(c: &mut Canvas, s: &Stuff, cx: i32, y: i32, lit: bool) {
    let face = Rect::new(cx - 5, y - 4, 10, 8);
    parts::fill(c, face, s.wall.at(Tone::Base), parts::south(), 12);
    window(c, s, Rect::new(cx - 3, y - 2, 6, 5), lit, cx);
    c.polyline_fill(&[(cx - 7, y - 4), (cx, y - 10), (cx + 1, y - 10), (cx + 8, y - 4)], s.roof.at(Tone::Base), 13);
    c.line((cx - 7, y - 4), (cx, y - 10), s.roof.at(Tone::Light), 1, 13);
}

/// A brick chimney standing up from the ridge at `x`, a stone cap and two pots.
fn chimney(c: &mut Canvas, s: &Stuff, x: i32, ridge: i32) {
    let top = (ridge - 10).max(0);
    blocks(c, Rect::new(x - 4, top + 3, 8, ridge - top + 3), Ramp::Brick, 3, 4, false, s.seed ^ 0x15, 12);
    c.rect_bevel(Rect::new(x - 5, top + 1, 10, 3), Ramp::Stone, 1, Z::new(13, 14));
    for px in [x - 3, x + 1] {
        c.fill_normal(Rect::new(px, top - 1, 2, 3), Ramp::Copper.at(Tone::Base), parts::south(), 14);
        c.dot(px, top - 1, Ix::SEAM, 14);
    }
}

/// A porch over the door at `cx`: two posts and a little pitched roof.
fn porch(c: &mut Canvas, s: &Stuff, cx: i32, foot: i32) {
    for px in [cx - 8, cx + 6] {
        post(c, px, foot - 17, foot - 1, 2, s.trim, 9);
    }
    c.polyline_fill(
        &[(cx - 10, foot - 16), (cx - 1, foot - 22), (cx, foot - 22), (cx + 9, foot - 16)],
        s.roof.at(Tone::Base),
        10,
    );
    c.line((cx - 10, foot - 16), (cx - 1, foot - 22), s.roof.at(Tone::Light), 1, 10);
    c.hline(cx - 10, cx + 9, foot - 16, s.roof.at(Tone::Deep), 10);
}

/// The steeple: a square stone tower with a louvred belfry and a slate spire and its vane.
fn steeple(c: &mut Canvas, s: &Stuff, lit: bool) {
    let (w, foot) = (c.w(), c.h() - 1);
    let (x, tw) = (4, w - 8);
    c.ao_contact(Rect::new(x - 1, foot - 3, tw + 2, 6), 1);
    let base = foot - (c.h() * 2 / 3);
    blocks(c, Rect::new(x, base, tw, foot - base + 1), s.wall, 4, 8, false, s.seed, 3);
    let bel = Rect::new(x + tw / 2 - 4, base + 4, 8, 9);
    c.fill_normal(bel, Ix::SEAM, parts::south(), 4);
    for y in (bel.y + 1..bel.bottom()).step_by(2) {
        c.hline(bel.x, bel.right() - 1, y, s.trim.at(Tone::Base), 5);
    }
    if lit {
        window(c, s, Rect::new(x + tw / 2 - 3, foot - 12, 6, 7), true, 1);
    }
    let top = 2;
    c.polygon_lit(
        &[(x + tw / 2 - 1, top + 3), (x + tw / 2, top + 3), (x + tw, base), (x - 1, base)],
        s.roof,
        90,
        Z::new(10, 16),
    );
    c.retone(s.roof, crate::kit::HARD);
    c.vline(x + tw / 2, top, top + 3, Ramp::Iron.at(Tone::Base), 17);
    c.hline(x + tw / 2 - 2, x + tw / 2 + 2, top + 1, Ramp::Iron.at(Tone::Base), 17);
}

/// The finishing pass: clusters, the outline, then true heights: the wall stands up from the
/// foot; the roof lands on the house, rising from the eave's height to the ridge's.
fn finish(c: &mut Canvas, s: &Stuff, foot: i32, depth: i32, eave: i32, ridge: i32) {
    let glow: Vec<(i32, i32, Ix)> = (0..c.h())
        .flat_map(|y| (0..c.w()).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let e = c.emissive_at(x, y);
            (e != Ix::CLEAR).then_some((x, y, e))
        })
        .collect();
    for r in [s.wall, s.roof, s.trim, s.door] {
        c.declutter(r);
    }
    c.despike();
    c.outline();
    c.relight(&glow);
    c.upright(foot);
    if ridge > 0 && eave > ridge {
        let wall_z = (foot - eave) * 5 / 4;
        let span = eave - ridge;
        c.heights_by(Rect::new(0, 0, c.w(), eave + 1), |_, y| {
            // From the wall's top at the eave to the ridge, which lands half the footprint back.
            let up = (eave - y).clamp(0, span + 12);
            wall_z + up * (depth / 2) / span.max(1)
        });
    }
}
