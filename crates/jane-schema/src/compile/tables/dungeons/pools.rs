//! Every `.room` file, parsed and linted, by pool (`pools.ts`). A pool is a name templates
//! share (`mine.plate`); a mission names only the pool.

use std::collections::BTreeMap;

use super::room::{self, Room, Shape};
use crate::compile::diag::Diagnostics;
use crate::compile::source::Source;

/// Where a line error is: `rooms/mine/plate.a.room:12`.
pub fn at_line(file: &str, line: usize) -> String {
    if line == 0 { file.to_owned() } else { format!("{file}:{line}") }
}

#[derive(Debug, Default)]
pub struct Library {
    /// Parsed templates in file path order; a file that does not parse is left out (its error
    /// is reported), and so is a second template with an id already taken.
    pub rooms: Vec<Room>,
    /// Every allowed transform of each room, index-aligned with `rooms`.
    pub shapes: Vec<Vec<Shape>>,
    /// Pool id to room indices, in file path order.
    pub pools: BTreeMap<String, Vec<usize>>,
}

impl Library {
    /// Read, parse and lint every `rooms/*/*.room`. `diag` gets the errors; pass a throwaway
    /// one to read quietly.
    pub fn read(src: &Source, diag: &mut Diagnostics) -> Library {
        let mut lib = Library::default();
        for (file, text) in src.texts_in("rooms", "room") {
            let depth = file.matches('/').count();
            if depth != 2 {
                diag.error(file, "a room lives at rooms/<dungeon>/<name>.room");
                continue;
            }
            let r = match room::parse(text, file) {
                Ok(r) => r,
                Err((line, msg)) => {
                    diag.error(at_line(file, line), msg);
                    continue;
                }
            };
            for (k, line) in &r.head_lines {
                if !room::HEADER_KEYS.contains(&k.as_str()) {
                    diag.error(at_line(file, *line), format!("header \"{k}\" means nothing to the parser"));
                }
            }
            for (line, msg) in room::lint(&r) {
                diag.error(at_line(file, line), msg);
            }
            if let Some(prev) = lib.rooms.iter().find(|p| p.id == r.id) {
                diag.error(
                    at_line(file, r.head_line("id")),
                    format!("room template \"{}\" is defined twice (first in {})", r.id, prev.file),
                );
                continue;
            }
            lib.pools.entry(r.pool.clone()).or_default().push(lib.rooms.len());
            lib.shapes.push(room::shapes_of(&r));
            lib.rooms.push(r);
        }
        lib
    }

    /// The templates of a pool, in file path order.
    pub fn pool(&self, pool: &str) -> impl Iterator<Item = &Room> {
        self.pools.get(pool).into_iter().flatten().map(|&i| &self.rooms[i])
    }

    pub fn by_id(&self, id: &str) -> Option<usize> {
        self.rooms.iter().position(|r| r.id == id)
    }
}

/// A socket, mark or rect of the template called `local`.
pub fn has_local(t: &Room, local: &str) -> bool {
    t.sockets.iter().any(|s| s.id == local)
        || t.marks.iter().any(|m| m.id == local)
        || t.rects.iter().any(|r| r.id == local)
}

/// Pools are interfaces (DUNGEONS.md §2.4): variants differ in their grid and never in their
/// bays, their doors' required members, needs, grants or blocks (templates.test.ts).
pub fn check_interfaces(lib: &Library, diag: &mut Diagnostics) {
    for members in lib.pools.values() {
        let face = |t: &Room| {
            let mut grants = t.grants.clone();
            grants.sort();
            let required: Vec<&str> = t.doors.iter().filter(|d| d.required).map(|d| d.id.as_str()).collect();
            format!("{:?} {:?} {:?} {:?} {:?} {:?}", t.bays, required, t.needs_verbs, t.needs_items, grants, t.blocks)
        };
        let first = &lib.rooms[members[0]];
        for &i in &members[1..] {
            let t = &lib.rooms[i];
            if face(t) != face(first) {
                diag.error(
                    at_line(&t.file, t.head_line("id")),
                    format!(
                        "{} differs from {} in its interface (bays, required doors, needs, grants, blocks): variants of a pool differ only in their grid",
                        t.id, first.id
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grok #8: a header the parser does not know (a typo) is an error, not a warning.
    #[test]
    fn an_unknown_room_header_is_an_error() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/rooms/burial/alcove.a.room");
        let text = std::fs::read_to_string(path).expect("a room");
        let typo = text.replacen(
            "pool    ",
            "rotat   90
pool    ",
            1,
        );
        for (t, bad) in [(&text, false), (&typo, true)] {
            let mut diag = Diagnostics::default();
            Library::read(&Source::from_texts(&[("rooms/burial/alcove.a.room", t)]), &mut diag);
            let said = diag.errors.iter().any(|d| d.msg.contains("\"rotat\" means nothing"));
            assert_eq!(said, bad, "{diag}");
        }
    }
}
