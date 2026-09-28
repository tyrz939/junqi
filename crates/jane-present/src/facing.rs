//! Eight ways a unit faces on screen (ART.md §4, PRESENTATION.md §1.11): the sim's four and
//! the diagonals between them. The sim keeps four (its rules turn on a dominant axis); the
//! presenter reads the diagonal off how the unit moved, so a walk down and to the right shows
//! her walking down and to the right. Standing, a diagonal is kept while it agrees with the
//! sim's facing, so she stops as she walked; turned to face something, she turns.

use jane_core::action::Facing;

/// A facing in eight sectors, east first and clockwise (screen y runs down, south).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[allow(missing_docs)]
pub enum Face8 {
    East,
    SouthEast,
    #[default]
    South,
    SouthWest,
    West,
    NorthWest,
    North,
    NorthEast,
}

/// Each sector's unit direction, scaled by 1000 (`y` down the screen).
const DIRS: [(i64, i64); 8] =
    [(1000, 0), (707, 707), (0, 1000), (-707, 707), (-1000, 0), (-707, -707), (0, -1000), (707, -707)];

/// What the sector a unit already faces is worth over its neighbours, per mille: a walk has to
/// swing about five degrees past a sector's edge to turn her, so a path that wanders along the
/// edge never flickers between two frames.
const STAY: i64 = 1100;

impl Face8 {
    /// Every sector, in order.
    pub const ALL: [Face8; 8] = [
        Face8::East,
        Face8::SouthEast,
        Face8::South,
        Face8::SouthWest,
        Face8::West,
        Face8::NorthWest,
        Face8::North,
        Face8::NorthEast,
    ];

    /// The sector of a sim facing.
    pub const fn of(f: Facing) -> Face8 {
        match f {
            Facing::East => Face8::East,
            Facing::South => Face8::South,
            Facing::West => Face8::West,
            Facing::North => Face8::North,
        }
    }

    const fn index(self) -> usize {
        self as usize
    }

    /// Whether this sector is one of the three facing west, which are drawn as the east ones
    /// mirrored.
    pub const fn west(self) -> bool {
        matches!(self, Face8::West | Face8::SouthWest | Face8::NorthWest)
    }

    /// Whether this is a diagonal.
    pub const fn diagonal(self) -> bool {
        self.index() % 2 == 1
    }

    /// The sim facing a frame table without diagonals shows for this sector: a diagonal is shown
    /// from the side.
    pub const fn cardinal(self) -> Facing {
        match self {
            Face8::East | Face8::SouthEast | Face8::NorthEast => Facing::East,
            Face8::West | Face8::SouthWest | Face8::NorthWest => Facing::West,
            Face8::South => Facing::South,
            Face8::North => Facing::North,
        }
    }

    /// Whether this sector is within 45 degrees of the sim facing `f`: `f` itself or a diagonal
    /// either side of it.
    pub const fn agrees(self, f: Facing) -> bool {
        let d = (self.index() + 8 - Face8::of(f).index()) % 8;
        d <= 1 || d == 7
    }

    /// Standing: the sector she was in if it still agrees with the sim's facing (she stops as
    /// she walked), else the sim's facing.
    pub const fn settle(self, sim: Facing) -> Face8 {
        if self.agrees(sim) { self } else { Face8::of(sim) }
    }

    /// Moving by `(dx, dy)` (any units, y down) while the sim faces `sim`: the sector nearest
    /// the motion among the three that agree with the sim, the one she is in held a little
    /// over its neighbours. A motion that does not agree with the sim at all (knocked back,
    /// pushed) leaves her facing as she stood.
    pub fn moving(self, dx: i64, dy: i64, sim: Facing) -> Face8 {
        if dx == 0 && dy == 0 {
            return self.settle(sim);
        }
        let dot = |f: Face8| {
            let (x, y) = DIRS[f.index()];
            x * dx + y * dy
        };
        let best = Face8::ALL.into_iter().max_by_key(|&f| dot(f)).unwrap_or(self);
        if !best.agrees(sim) {
            // At right angles to the sim's facing she is sliding along a wall she walks into
            // on the diagonal (the sim keeps the axis she pressed first): she faces between
            // the two. Anything further round (knocked back) leaves her as she stood.
            let s = Face8::of(sim).index();
            return match (best.index() + 8 - s) % 8 {
                2 => Face8::ALL[(s + 1) % 8],
                6 => Face8::ALL[(s + 7) % 8],
                _ => self.settle(sim),
            };
        }
        let worth = |f: Face8| if f == self { dot(f) * STAY / 1000 } else { dot(f) };
        Face8::ALL.into_iter().filter(|f| f.agrees(sim)).max_by_key(|&f| worth(f)).unwrap_or(best)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_on_the_diagonal_faces_the_diagonal_and_a_stop_keeps_it() {
        let f = Face8::South.moving(10, 10, Facing::South);
        assert_eq!(f, Face8::SouthEast);
        assert_eq!(f.settle(Facing::South), Face8::SouthEast, "she stops as she walked");
        assert_eq!(f.settle(Facing::East), Face8::SouthEast);
        assert_eq!(f.settle(Facing::North), Face8::North, "turned to face something, she turns");
        assert_eq!(Face8::East.moving(-10, -9, Facing::West), Face8::NorthWest);
        assert_eq!(Face8::East.moving(100, 3, Facing::East), Face8::East);
    }

    #[test]
    fn a_path_along_a_sector_edge_does_not_flicker() {
        // About 24 degrees below east: past the edge at 22.5, but not far enough to turn her.
        let (dx, dy) = (1000, 445);
        assert_eq!(Face8::East.moving(dx, dy, Facing::East), Face8::East);
        assert_eq!(Face8::SouthEast.moving(dx, dy, Facing::East), Face8::SouthEast);
        // Well past it she turns.
        assert_eq!(Face8::East.moving(1000, 700, Facing::East), Face8::SouthEast);
    }

    #[test]
    fn sliding_along_a_wall_on_the_diagonal_faces_the_diagonal() {
        // She presses up and to the left into a wall on her left; the sim faces west, she
        // slides north.
        assert_eq!(Face8::SouthWest.moving(0, -30, Facing::West), Face8::NorthWest);
        assert_eq!(Face8::East.moving(0, 30, Facing::East), Face8::SouthEast);
    }

    #[test]
    fn knocked_back_she_keeps_her_facing() {
        assert_eq!(Face8::East.moving(-50, 0, Facing::East), Face8::East);
        assert_eq!(Face8::SouthEast.moving(-50, -50, Facing::South), Face8::SouthEast);
    }

    #[test]
    fn west_sectors_mirror_and_diagonals_fall_back_to_the_side() {
        assert!(Face8::SouthWest.west() && Face8::NorthWest.west() && !Face8::North.west());
        assert_eq!(Face8::NorthEast.cardinal(), Facing::East);
        assert_eq!(Face8::SouthWest.cardinal(), Facing::West);
        assert!(Face8::NorthEast.agrees(Facing::North) && Face8::NorthEast.agrees(Facing::East));
        assert!(!Face8::NorthEast.agrees(Facing::South));
    }
}
