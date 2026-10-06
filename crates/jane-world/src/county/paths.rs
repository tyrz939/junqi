//! A footpath's two ends: a mark each, a little way out from the set place it leaves (where the
//! stile is), open ground round it, and a fingerpost beside it saying where the path goes and how
//! far. Runs after the chunks and the railway, so an end is found outside every chunk's box.

use jane_core::action::{Action, Facing};
use jane_core::{Key, Tile};
use jane_data::PlaceAt;

use super::County;
use crate::kit::js_round;

/// Points along the path from the first outside every chunk to where the stile stands: half a
/// screen, so the stile is out of sight of the gate it leaves (24 at 48 x 27, PORT.md §6.h).
const STILE_IN: usize = jane_core::view::HALF_W_CELLS as usize;
/// How far outside a chunk's box an end must be.
const CHUNK_MARGIN: i32 = 8;

/// Metres as a fingerpost says them: to the nearest 50, and past a kilometre in tenths of one.
pub fn distance_words(metres: i64) -> String {
    if metres < 1000 { format!("{metres} m") } else { format!("{} km", Tenths(js_round(metres, 100))) }
}

/// A number of tenths that prints as `12.3`.
struct Tenths(i64);

impl std::fmt::Display for Tenths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.0 / 10, self.0 % 10)
    }
}

/// Open ground round a named spot: trees, bushes, outcrops and rubble give way; water and
/// buildings do not.
pub fn clearing(c: &mut County<'_>, x: i32, y: i32) {
    for oy in -5..=5 {
        for ox in -7..=7 {
            let t = c.k.get(x + ox, y + oy);
            if matches!(t, Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Cliff | Tile::Rubble) {
                c.k.set(x + ox, y + oy, Tile::Grass);
            }
        }
    }
}

/// Marks and fingerposts at every footpath's two ends. Every end's mark first, then the posts: a
/// path that leaves a place and comes back to it (the nurse's prints, out from the car and back)
/// has its two ends a few cells apart, and a post set down beside the first once stood on the
/// second's mark. A mark claims the ring round it, so no post is set there now.
pub fn path_ends(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let fingerpost = cat.story.prop_id("fingerpost").expect("a fingerpost row");
    let label = c.k.text("A fingerpost");
    let footpaths = c.footpaths.clone();
    let mut posts = Vec::new();
    for fp in footpaths {
        let row = &cat.county.paths[fp.row];
        let line = &c.lines[fp.line];
        let outside = |&(x, y): &(i32, i32)| c.chunks.iter().all(|ch| !ch.bounds.grow(CHUNK_MARGIN).contains(x, y));
        let (Some(a), Some(b)) = (line.iter().position(outside), line.iter().rposition(outside)) else { continue };
        if b <= a {
            continue;
        }
        let pa = line[b.min(a + STILE_IN)];
        let pb = line[a.max(b.saturating_sub(STILE_IN))];
        // Tenths of the path's points, a point a metre and a tenth of path, to the nearest 50 m.
        let metres = js_round((b - a) as i64 * 11, 500) * 50;
        for (name, (x, y), toward) in [(row.marks[0], pa, row.to), (row.marks[1], pb, row.from)] {
            clearing(c, x, y);
            c.k.mark(Key::Name(name), x, y, Some(Facing::South));
            // A path end the quests already furnish (their own post or sign, saying more) gets no
            // second post from here: two fingerposts and a sign at one stile is clutter.
            if !cat.county.placements.iter().any(|p| p.at == PlaceAt::Mark(name)) {
                posts.push(((x, y), toward, metres));
            }
        }
    }
    for ((x, y), toward, metres) in posts {
        let Some(there) = c.sk.sites.get(usize::from(toward)) else { continue };
        let words = format!("FOOTPATH. {}, {}.", cat.text(there.def.name).to_uppercase(), distance_words(metres));
        // Beside the stile, not on it.
        for (ox, oy) in [(2, -1), (-3, -1), (2, 1), (-3, 1)] {
            if !c.k.fits(x + ox, y + oy, 2, 1, 0) {
                continue;
            }
            let words = c.k.text(&words);
            let read = c.k.list(vec![Action::Read(words)]);
            let p = c.k.prop(None, fingerpost, x + ox, y + oy);
            p.label = Some(label);
            p.use_list = Some(read);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_read_as_a_fingerpost_says_them() {
        assert_eq!(distance_words(350), "350 m");
        assert_eq!(distance_words(1000), "1.0 km");
        assert_eq!(distance_words(1250), "1.3 km");
        assert_eq!(distance_words(2400), "2.4 km");
    }
}
