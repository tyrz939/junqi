//! The ways into the dungeons (`data/doors.json`, the doors loop of `county.ts`). Every row
//! applies: all thirteen zones exist (PORT.md §6.l). A door is either set into its landmark's face,
//! at the chunk's `<site>_door` slot, or stood on open ground beside a site (a grate in the town, a
//! manhole cover on the road: the pipes have no building of their own), found by
//! [`crate::kit::Kit::spot`] on the door's own dice with room for somebody to stand in front of it.
//! A row with a `mark` leaves one below the door, where the dungeon's own way out arrives; the
//! ground in front of every door is claimed, so nothing of the country's is built across it.

use alloc::format;
use jane_core::action::{Facing, TextRef};
use jane_core::blueprint::Door;
use jane_core::{Key, Rect};
use jane_data::DoorAt;

use super::County;
use crate::steps::Step;

/// Cells round a site's box a door beside it may stand in.
const NEAR: i32 = 14;
/// Rows of open ground kept below a door that stands on its own.
const FRONT: i32 = 3;
/// Cells a door that stands on its own is tried at.
const TRIES: u32 = 200;
/// Rows of ground claimed in front of every door.
const CLAIM: i32 = 4;

/// Every door row, in row order. A `Near` door throws [`Step::CountyDoor`] for its row.
pub fn set_doors(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    for (row, d) in cat.county.doors.iter().enumerate() {
        let site = match d.at {
            DoorAt::Chunk(s) | DoorAt::Near(s) => s,
        };
        let Some(ch) = c.chunks.iter().find(|ch| ch.site == site) else { continue };
        let size = cat.story.prop(d.def);
        let (w, h) = (i32::from(size.w), i32::from(size.h));
        let at = match d.at {
            DoorAt::Chunk(_) => ch.slot_named(&format!("{}_door", ch.id())),
            DoorAt::Near(_) => {
                let mut rng = c.k.dice(Step::CountyDoor, row as i32, 0);
                c.k.spot(&mut rng, ch.bounds.grow(NEAR), w, h + FRONT, 1, TRIES)
            }
        };
        let Some((x, y)) = at else { continue };
        let p = c.k.prop(Some(Key::Name(d.key)), d.def, x, y);
        p.locked = d.key_tag.is_some() && !d.keyed;
        p.key_tag = d.key_tag.map(Key::Name);
        p.to = d.to.map(|m| Door { zone: d.zone, mark: Key::Name(m) });
        p.label = Some(TextRef::Text(d.label));
        p.night_lock = d.night_lock.map(|l| jane_core::NightLock { keyed: d.keyed, ..l.lock() });
        if let Some(m) = d.mark {
            c.k.mark(Key::Name(m), x, y + h, Some(Facing::South));
        }
        c.k.claim(Rect::new(x - 1, y + h, w + 2, CLAIM));
    }
}
