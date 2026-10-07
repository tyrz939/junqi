//! C11: plates can be held. A plate whose release re-locks something has, in its room,
//! something she can push onto it: a pushable with a path of clear cells to the plate and, at
//! every step, somewhere to stand behind it (`pushPath`).

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::action::Action;
use jane_core::grid::Rect;
use jane_core::tile::{F_NOPUSH, F_SOLID};
use jane_core::{Grid, Tile};

use super::{Check, Ctx, Fault};

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let (bp, cat) = (c.bp, c.cat);
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let mut out = Vec::new();
    let foot = |p: &jane_core::blueprint::PropSpawn| {
        let d = cat.story.prop(p.def);
        Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
    };
    for plate in &bp.props {
        let pd = cat.story.prop(plate.def);
        let relocks =
            plate.release.and_then(|r| bp.list(r)).is_some_and(|l| l.iter().any(|a| matches!(a, Action::Lock(_))));
        if !pd.plate || !relocks {
            continue;
        }
        let (x, y) = (i32::from(plate.cell.x), i32::from(plate.cell.y));
        let Some(room) = c.info.rooms.iter().find(|r| r.rect.contains(x, y)) else { continue };
        let mut solid_at = Grid::new(bp.w(), bp.h(), false);
        for p in &bp.props {
            if cat.story.prop(p.def).solid && !p.hidden {
                solid_at.fill_rect(foot(p), true);
            }
        }
        let ok = bp.props.iter().any(|p| {
            if !cat.story.prop(p.def).push || !room.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y)) {
                return false;
            }
            let own = foot(p);
            push_path(w, h, own, foot(plate), |x, y| {
                let f = bp.tiles.read(x, y, Tile::Void).flags();
                f & (F_SOLID | F_NOPUSH) == 0 && (!solid_at.read(x, y, true) || own.contains(x, y))
            })
        });
        if !ok {
            out.push(Fault::new(Check::C11, format!("nothing can be pushed onto {}", c.name(plate.key))));
        }
    }
    out
}

/// Can a `from`-sized thing be pushed from `from` until it overlaps `to`, over cells `clear`
/// says are free, with somewhere clear to stand behind it at every step? (`room.ts` `pushPath`.)
pub fn push_path(gw: i32, gh: i32, from: Rect, to: Rect, clear: impl Fn(i32, i32) -> bool) -> bool {
    let (w, h) = (from.w, from.h);
    let fits = |x: i32, y: i32| {
        (0..h).all(|j| (0..w).all(|i| x + i >= 0 && y + j >= 0 && x + i < gw && y + j < gh && clear(x + i, y + j)))
    };
    let overlaps = |x: i32, y: i32| x < to.x + to.w && x + w > to.x && y < to.y + to.h && y + h > to.y;
    let mut seen = Grid::new(gw.max(1) as u32, gh.max(1) as u32, false);
    seen.set(from.x, from.y, true);
    let mut queue = vec![(from.x, from.y)];
    let mut n = 0;
    while n < queue.len() {
        let (x, y) = queue[n];
        n += 1;
        if overlaps(x, y) {
            return true;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if seen.read(nx, ny, true) || !fits(nx, ny) {
                continue;
            }
            // Somewhere to stand on the side she pushes from.
            let behind = if dx != 0 {
                (0..h).any(|j| clear(if dx > 0 { x - 1 } else { x + w }, y + j))
            } else {
                (0..w).any(|i| clear(x + i, if dy > 0 { y - 1 } else { y + h }))
            };
            if !behind {
                continue;
            }
            seen.set(nx, ny, true);
            queue.push((nx, ny));
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_push_needs_room_behind() {
        let open = |x: i32, y: i32| (0..6).contains(&x) && y == 0;
        assert!(push_path(6, 1, Rect::new(1, 0, 1, 1), Rect::new(4, 0, 1, 1), open));
        // Against the west wall there is nowhere to stand to push it east.
        assert!(!push_path(6, 1, Rect::new(0, 0, 1, 1), Rect::new(4, 0, 1, 1), open));
    }
}
