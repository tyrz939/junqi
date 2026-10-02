//! What stands in a front garden (ART-PLAN M7): its boundary, a piece a cell (pickets, a low
//! wall, privet, railings) and its gate, two cells wide under the door; and the things a recipe
//! puts in it: a rose arch, hollyhocks, a water butt, bean canes, a bird bath, a sundial, a rusted
//! bike. Each is a flora sprite in the chunk painter's bank (`flora::Bank`), placed by
//! `terrain::standing` and drawn from the atlas sorted among the units: drawing only, nothing a
//! foot meets.
//!
//! A boundary piece is drawn to tile: its pattern repeats every 16 px and it is not outlined at
//! its ends, so a run of them is one fence.

use jane_core::grid::Rect;

use crate::canvas::{Canvas, Z, height_of_rows, normal};
use crate::flora::Sprite;
use crate::palette::{Ix, Ramp, Tone};
use crate::terrain::houses::Boundary;

/// A garden piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Piece {
    /// A run of white pickets on two rails, a cell long.
    Picket,
    /// Pickets gone grey, one missing, one leaning: a neglected garden's.
    PicketOld,
    /// A low brick wall under a stone coping.
    LowWall,
    /// Clipped privet.
    Privet,
    /// Iron railings on a stone plinth.
    Railings,
    /// The gates, two cells wide: posts and an open way through. The pickets' gate.
    PicketGate,
    /// A low wall's: brick piers with ball caps.
    LowWallGate,
    /// A privet's: the hedge cut back round a timber gate.
    PrivetGate,
    /// Railings': iron posts and an iron gate.
    RailingsGate,
    /// Roses over an arch at the gate (a cottage garden's).
    RoseArch,
    /// Hollyhocks against the wall.
    Hollyhocks,
    /// A water butt.
    WaterButt,
    /// A wigwam of bean canes, runner beans up it.
    Canes,
    /// A stone bird bath.
    BirdBath,
    /// A sundial on its pedestal.
    Sundial,
    /// A bicycle gone to rust in the grass.
    Bike,
}

impl Piece {
    /// Every piece, in bank order.
    pub const ALL: [Piece; 16] = [
        Piece::Picket,
        Piece::PicketOld,
        Piece::LowWall,
        Piece::Privet,
        Piece::Railings,
        Piece::PicketGate,
        Piece::LowWallGate,
        Piece::PrivetGate,
        Piece::RailingsGate,
        Piece::RoseArch,
        Piece::Hollyhocks,
        Piece::WaterButt,
        Piece::Canes,
        Piece::BirdBath,
        Piece::Sundial,
        Piece::Bike,
    ];

    /// Its name in the bank.
    pub fn name(self) -> &'static str {
        match self {
            Piece::Picket => "garden_picket",
            Piece::PicketOld => "garden_picket_old",
            Piece::LowWall => "garden_low_wall",
            Piece::Privet => "garden_privet",
            Piece::Railings => "garden_railings",
            Piece::PicketGate => "garden_picket_gate",
            Piece::LowWallGate => "garden_low_wall_gate",
            Piece::PrivetGate => "garden_privet_gate",
            Piece::RailingsGate => "garden_railings_gate",
            Piece::RoseArch => "garden_rose_arch",
            Piece::Hollyhocks => "garden_hollyhocks",
            Piece::WaterButt => "garden_water_butt",
            Piece::Canes => "garden_canes",
            Piece::BirdBath => "garden_bird_bath",
            Piece::Sundial => "garden_sundial",
            Piece::Bike => "garden_bike",
        }
    }

    /// A boundary's run, or its gate (two cells wide: placed a cell on, at the gate's west cell).
    pub fn of(b: Boundary, gate: bool, old: bool) -> Piece {
        match (b, gate) {
            (Boundary::Picket, false) if old => Piece::PicketOld,
            (Boundary::Picket, false) => Piece::Picket,
            (Boundary::LowWall, false) => Piece::LowWall,
            (Boundary::Privet, false) => Piece::Privet,
            (Boundary::Railings, false) => Piece::Railings,
            (Boundary::Picket, true) => Piece::PicketGate,
            (Boundary::LowWall, true) => Piece::LowWallGate,
            (Boundary::Privet, true) => Piece::PrivetGate,
            (Boundary::Railings, true) => Piece::RailingsGate,
        }
    }

    /// A run of boundary or a gate: low and long, its shadow thrown from a thin line.
    pub fn is_boundary(self) -> bool {
        matches!(
            self,
            Piece::Picket
                | Piece::PicketOld
                | Piece::LowWall
                | Piece::Privet
                | Piece::Railings
                | Piece::PicketGate
                | Piece::LowWallGate
                | Piece::PrivetGate
                | Piece::RailingsGate
                | Piece::RoseArch
        )
    }

    /// Two cells wide, its foot on the line between them.
    pub fn is_wide(self) -> bool {
        matches!(
            self,
            Piece::PicketGate | Piece::LowWallGate | Piece::PrivetGate | Piece::RailingsGate | Piece::RoseArch
        )
    }
}

/// The piece drawn, seeded. A gate stands over a worn stone threshold across its way through.
pub fn sprite(piece: Piece, seed: u32) -> Sprite {
    let mut s = drawn(piece, seed);
    if piece.is_wide() {
        let ay = s.ay;
        for x in 9..23 {
            let t = if x == 9 {
                Tone::Light
            } else if x == 22 {
                Tone::Mid
            } else {
                Tone::Lift
            };
            s.canvas.put(x, ay - 1, Ramp::Stone.at(t), normal(0, -60), 1);
            s.canvas.put(x, ay, Ramp::Stone.at(Tone::Shade), normal(0, 60), 1);
        }
    }
    s
}

fn drawn(piece: Piece, seed: u32) -> Sprite {
    match piece {
        Piece::Picket | Piece::PicketOld => {
            let (mut c, ax, ay) = canvas(16, 18);
            pickets(&mut c, 0, 16, ay, piece == Piece::PicketOld, seed);
            done(c, ax, ay, false)
        }
        Piece::LowWall => {
            let (mut c, ax, ay) = canvas(16, 16);
            low_wall(&mut c, 0, 16, ay);
            done(c, ax, ay, false)
        }
        Piece::Privet => {
            let (mut c, ax, ay) = canvas(16, 18);
            privet(&mut c, 0, 16, ay, seed);
            done(c, ax, ay, false)
        }
        Piece::Railings => {
            let (mut c, ax, ay) = canvas(16, 20);
            railings(&mut c, 0, 16, ay);
            done(c, ax, ay, false)
        }
        Piece::PicketGate => {
            let (mut c, ax, ay) = canvas(32, 20);
            pickets(&mut c, 0, 8, ay, false, seed);
            pickets(&mut c, 24, 32, ay, false, seed);
            for x in [8, 22] {
                gate_post(&mut c, x, ay, 13, Ramp::Limewash);
            }
            // The gate swung open into the garden, seen edge on behind its post.
            open_leaf(&mut c, 11, ay, Ramp::Limewash);
            done(c, ax, ay, false)
        }
        Piece::LowWallGate => {
            let (mut c, ax, ay) = canvas(32, 20);
            low_wall(&mut c, 0, 7, ay);
            low_wall(&mut c, 25, 32, ay);
            for x in [6, 22] {
                pier(&mut c, x, ay);
            }
            open_leaf(&mut c, 11, ay, Ramp::WoodOak);
            done(c, ax, ay, false)
        }
        Piece::PrivetGate => {
            let (mut c, ax, ay) = canvas(32, 20);
            privet(&mut c, 0, 9, ay, seed);
            privet(&mut c, 23, 32, ay, seed ^ 7);
            for x in [9, 21] {
                gate_post(&mut c, x, ay, 12, Ramp::WoodOak);
            }
            open_leaf(&mut c, 12, ay, Ramp::WoodOak);
            done(c, ax, ay, false)
        }
        Piece::RailingsGate => {
            let (mut c, ax, ay) = canvas(32, 22);
            railings(&mut c, 0, 8, ay);
            railings(&mut c, 24, 32, ay);
            for x in [8, 22] {
                gate_post(&mut c, x, ay, 15, Ramp::Iron);
            }
            open_leaf(&mut c, 11, ay, Ramp::Iron);
            done(c, ax, ay, false)
        }
        Piece::RoseArch => rose_arch(seed),
        Piece::Hollyhocks => hollyhocks(seed),
        Piece::WaterButt => water_butt(),
        Piece::Canes => canes(seed),
        Piece::BirdBath => bird_bath(),
        Piece::Sundial => sundial(),
        Piece::Bike => bike(seed),
    }
}

/// A canvas `w x h` and its foot: the middle, two px over the bottom.
fn canvas(w: i32, h: i32) -> (Canvas, i32, i32) {
    (Canvas::new(w, h), w / 2, h - 2)
}

/// Heights stood up from the foot, then (a thing, not a run) outlined.
fn done(mut c: Canvas, ax: i32, ay: i32, outline: bool) -> Sprite {
    if outline {
        c.outline();
    }
    let (w, h) = (c.w(), c.h());
    c.heights_by(Rect::new(0, 0, w, h), |_, y| height_of_rows((ay + 1 - y).max(1)));
    Sprite { canvas: c, ax, ay }
}

/// A px with a south-facing normal.
fn px(c: &mut Canvas, x: i32, y: i32, ix: Ix) {
    if (0..c.w()).contains(&x) && (0..c.h()).contains(&y) {
        c.put(x, y, ix, normal(0, 60), 1);
    }
}

/// White pickets over `x0..x1`, 2 px wide every 4, pointed, on two rails; grey and gapped when
/// `old`.
fn pickets(c: &mut Canvas, x0: i32, x1: i32, ay: i32, old: bool, seed: u32) {
    let paint = if old { Ramp::Deadwood } else { Ramp::Limewash };
    for ry in [ay - 7, ay - 3] {
        for x in x0..x1 {
            px(c, x, ry, paint.at(Tone::Mid));
            px(c, x, ry + 1, paint.at(Tone::Deep));
        }
    }
    let mut x = x0 + 1;
    let mut k = 0;
    while x + 1 < x1 {
        let h = crate::hash::h32(seed, k, 0x9c4e);
        if old && h % 5 == 0 {
            x += 4;
            k += 1;
            continue;
        }
        let lean = i32::from(old && h % 5 == 1);
        let top = ay - 10 + i32::from(old && h % 3 == 0);
        for y in top..=ay {
            let dx = i32::from(lean == 1 && y < ay - 5);
            let (l, r) = if y == top {
                (paint.at(Tone::High), paint.at(Tone::Shade))
            } else if y >= ay - 1 {
                (paint.at(Tone::Mid), paint.at(Tone::Shade))
            } else {
                (paint.at(Tone::Light), paint.at(Tone::Base))
            };
            px(c, x + dx, y, l);
            if y > top {
                px(c, x + 1 + dx, y, r);
            }
        }
        // The point's shadow side.
        px(c, x + 1, top, paint.at(Tone::Deep));
        x += 4;
        k += 1;
    }
}

/// A low wall over `x0..x1`: brick courses 3 px under a stone coping, lit along its top.
fn low_wall(c: &mut Canvas, x0: i32, x1: i32, ay: i32) {
    let top = ay - 9;
    for y in top..=ay {
        for x in x0..x1 {
            let ix = match y - top {
                0 => Ramp::Stone.at(Tone::High),
                1 => Ramp::Stone.at(Tone::Lift),
                2 => Ramp::Stone.at(Tone::Shade),
                d => {
                    let course = (d - 3) / 3;
                    let yy = (d - 3) % 3;
                    let xs = x + (course & 1) * 4;
                    if yy == 2 {
                        Ramp::Brick.at(Tone::Shade)
                    } else if xs.rem_euclid(8) == 0 {
                        Ramp::Brick.at(Tone::Mid)
                    } else if yy == 0 {
                        Ramp::Brick.at(Tone::Lift)
                    } else {
                        Ramp::Brick.at(Tone::Base)
                    }
                }
            };
            px(c, x, y, ix);
        }
    }
}

/// A brick pier with a stone cap and a ball on it.
fn pier(c: &mut Canvas, x: i32, ay: i32) {
    for y in ay - 12..=ay {
        for dx in 0..5 {
            let t = match dx {
                0 => Tone::Light,
                4 => Tone::Shade,
                _ if (ay - y) % 3 == 0 => Tone::Mid,
                _ => Tone::Base,
            };
            px(c, x + dx, y, Ramp::Brick.at(t));
        }
    }
    for dx in -1..6 {
        px(c, x + dx, ay - 13, Ramp::Stone.at(Tone::High));
        px(c, x + dx, ay - 12, Ramp::Stone.at(Tone::Mid));
    }
    c.disc_lit(x + 2, ay - 15, 1, Ramp::Stone, Z::flat(1));
}

/// Clipped privet over `x0..x1`: a flat lit top and a shaded face, in leaf clumps, its ends
/// rounded where a gate cuts it.
fn privet(c: &mut Canvas, x0: i32, x1: i32, ay: i32, seed: u32) {
    let top = ay - 11;
    for y in top..=ay {
        for x in x0..x1 {
            // Round the ends a run is cut at (not at a cell's edge: a run goes on).
            let end = (x0 > 0 && x - x0 < 2 || x1 < c.w() && x1 - 1 - x < 2) && y < top + 2;
            if end {
                continue;
            }
            let h = crate::hash::h32((x.rem_euclid(16) / 2) as u32, (y / 2) as u32, 0x9a1 ^ (seed % 2));
            let face = y >= top + 4;
            let t = match (face, h % 5) {
                (false, 0) => Tone::High,
                (false, 1 | 2) => Tone::Light,
                (false, _) => Tone::Lift,
                (true, 0) => Tone::Base,
                (true, 1) => Tone::Shade,
                (true, _) if y >= ay - 1 => Tone::Deep,
                (true, _) => Tone::Mid,
            };
            px(c, x, y, Ramp::Hedge.at(t));
        }
    }
    for x in x0..x1 {
        if !(x0 > 0 && x - x0 < 2 || x1 < c.w() && x1 - 1 - x < 2) {
            px(c, x, top, Ramp::Hedge.at(Tone::Lift));
        }
    }
}

/// Iron railings over `x0..x1`: bars every 3 px with spear heads, two rails, on a stone plinth.
fn railings(c: &mut Canvas, x0: i32, x1: i32, ay: i32) {
    for x in x0..x1 {
        px(c, x, ay - 2, Ramp::Stone.at(Tone::Light));
        px(c, x, ay - 1, Ramp::Stone.at(Tone::Base));
        px(c, x, ay, Ramp::Stone.at(Tone::Mid));
        for ry in [ay - 4, ay - 11] {
            px(c, x, ry, Ramp::Iron.at(Tone::Shade));
        }
    }
    let mut x = x0 + 1;
    while x < x1 {
        for y in ay - 13..ay - 2 {
            px(c, x, y, Ramp::Iron.at(if y == ay - 13 { Tone::Lift } else { Tone::Deep }));
        }
        px(c, x, ay - 14, Ramp::Iron.at(Tone::Light));
        x += 3;
    }
}

/// A gate post `tall` px, lit on its left, a cap on it.
fn gate_post(c: &mut Canvas, x: i32, ay: i32, tall: i32, r: Ramp) {
    for y in ay - tall..=ay {
        px(c, x, y, r.at(Tone::Light));
        px(c, x + 1, y, r.at(Tone::Base));
        px(c, x + 2, y, r.at(Tone::Shade));
    }
    for dx in 0..3 {
        px(c, x + dx, ay - tall - 1, r.at(if dx == 0 { Tone::High } else { Tone::Mid }));
    }
}

/// A gate standing open into the garden: its leaf seen edge on, a narrow board behind the post
/// it hangs from, a rail across it.
fn open_leaf(c: &mut Canvas, x: i32, ay: i32, r: Ramp) {
    for y in ay - 11..ay - 1 {
        px(c, x, y - 2, r.at(Tone::Lift));
        px(c, x + 1, y - 2, r.at(Tone::Shade));
    }
    px(c, x + 2, ay - 9, r.at(Tone::Mid));
}

/// Leaf in clumps along a path of points, each 3 x 3 and lit on its upper left, a bloom on one
/// clump in `every`.
fn foliage(c: &mut Canvas, pts: &[(i32, i32)], leaf: Ramp, bloom: Ramp, every: u32, seed: u32) {
    const CLUMP: [(i32, i32, Tone); 8] = [
        (0, -1, Tone::Light),
        (-1, -1, Tone::Lift),
        (1, -1, Tone::Lift),
        (-1, 0, Tone::Base),
        (0, 0, Tone::Lift),
        (1, 0, Tone::Mid),
        (0, 1, Tone::Mid),
        (1, 1, Tone::Shade),
    ];
    for &(x, y) in pts {
        for (dx, dy, t) in CLUMP {
            px(c, x + dx, y + dy, leaf.at(t));
        }
    }
    for (i, &(x, y)) in pts.iter().enumerate() {
        if crate::hash::h32(seed, i as u32, 0xf011) % every == 0 {
            px(c, x - 1, y - 1, bloom.at(Tone::Light));
            px(c, x, y - 1, bloom.at(Tone::Lift));
            px(c, x - 1, y, bloom.at(Tone::Base));
            px(c, x, y, bloom.at(Tone::Mid));
        }
    }
}

/// Roses over a timber arch at the gate: two posts and the arch between them, leaf all over
/// them and a bloom in clumps.
fn rose_arch(seed: u32) -> Sprite {
    let (mut c, ax, ay) = canvas(32, 38);
    let bloom = [Ramp::ClothRed, Ramp::Bloom, Ramp::ClothCream][(seed % 3) as usize];
    for x in [6, 23] {
        for y in ay - 26..=ay {
            px(&mut c, x, y, Ramp::WoodOak.at(Tone::Light));
            px(&mut c, x + 1, y, Ramp::WoodOak.at(Tone::Base));
            px(&mut c, x + 2, y, Ramp::WoodOak.at(Tone::Shade));
        }
    }
    let arch = |x: i32| ay - 27 - (81 - (x - 16) * (x - 16)).max(0) / 10;
    for x in 6..=25 {
        px(&mut c, x, arch(x), Ramp::WoodOak.at(Tone::Light));
        px(&mut c, x, arch(x) + 1, Ramp::WoodOak.at(Tone::Base));
        px(&mut c, x, arch(x) + 2, Ramp::WoodOak.at(Tone::Shade));
    }
    let mut pts = Vec::new();
    for y in (ay - 25..ay - 1).step_by(2) {
        let h = crate::hash::h32(seed, y as u32, 1);
        let j = (h % 3) as i32 - 1;
        if h % 5 != 0 {
            pts.push((6 + j, y));
        }
        if (h >> 8) % 5 != 0 {
            pts.push((25 + j, y + 1));
        }
    }
    for x in (5..=27).step_by(2) {
        pts.push((x, arch(x) + (crate::hash::h32(seed, x as u32, 2) % 3) as i32 - 1));
    }
    foliage(&mut c, &pts, Ramp::Shrub, bloom, 3, seed);
    done(c, ax, ay, true)
}

/// Hollyhocks: two or three spikes with big leaves at their foot and blooms up them.
fn hollyhocks(seed: u32) -> Sprite {
    let (mut c, ax, ay) = canvas(16, 34);
    let blooms = [Ramp::Bloom, Ramp::ClothRose, Ramp::ClothCream, Ramp::ClothRed];
    for (i, x) in [4, 8, 12].into_iter().enumerate() {
        let h = crate::hash::h32(seed, i as u32, 0x0110);
        if i == 2 && h % 2 == 0 {
            continue;
        }
        let tall = 20 + (h >> 4) as i32 % 8;
        for y in ay - tall..=ay {
            px(&mut c, x, y, Ramp::Leaf.at(Tone::Lift));
            px(&mut c, x + 1, y, Ramp::Leaf.at(Tone::Base));
        }
        // Blooms up the spike, open cups either side, buds at its tip.
        let bloom = blooms[(h >> 8) as usize % blooms.len()];
        let mut y = ay - tall + 3;
        let mut side = 1;
        for b in 0..3 {
            px(&mut c, x + (b & 1), ay - tall + b, bloom.at(Tone::Lift));
        }
        while y < ay - 7 {
            let bx = if side > 0 { x + 1 } else { x - 2 };
            px(&mut c, bx, y, bloom.at(Tone::Light));
            px(&mut c, bx + 1, y, bloom.at(Tone::Lift));
            px(&mut c, bx, y + 1, bloom.at(Tone::Base));
            px(&mut c, bx + 1, y + 1, bloom.at(Tone::Mid));
            y += 3;
            side = -side;
        }
    }
    let leaves: Vec<(i32, i32)> = [(3, ay - 2), (6, ay - 4), (9, ay - 2), (12, ay - 3), (5, ay), (11, ay)].into();
    foliage(&mut c, &leaves, Ramp::Shrub, Ramp::Shrub, 99, seed);
    done(c, ax, ay, true)
}

/// A water butt: a staved barrel with iron hoops and a lid, a brass tap at its foot.
fn water_butt() -> Sprite {
    let (mut c, ax, ay) = canvas(16, 22);
    c.polygon_lit(
        &[(3, ay - 14), (12, ay - 14), (13, ay - 7), (12, ay), (3, ay), (2, ay - 7)],
        Ramp::WoodOak,
        90,
        Z::flat(1),
    );
    for y in [ay - 12, ay - 3] {
        c.hline(2, 13, y, Ramp::Iron.at(Tone::Mid), 1);
    }
    c.ellipse_lit(Rect::new(2, ay - 17, 12, 5), Ramp::Slate, Z::flat(1));
    c.dot(7, ay - 2, Ramp::Brass.at(Tone::Light), 1);
    c.dot(7, ay - 1, Ramp::Brass.at(Tone::Shade), 1);
    done(c, ax, ay, true)
}

/// Bean canes in a wigwam tied at the top, runner beans climbing them, a scarlet flower or two.
fn canes(seed: u32) -> Sprite {
    let (mut c, ax, ay) = canvas(16, 32);
    let tip = (8, ay - 27);
    for x in [2, 8, 14] {
        c.line((x, ay), tip, Ramp::WoodPale.at(Tone::Lift), 1, 1);
    }
    let mut pts = Vec::new();
    for (k, x) in [2, 8, 14].into_iter().enumerate() {
        for i in 1..6 {
            let y = ay - i * 3 - (k as i32 & 1);
            let xx = x + (tip.0 - x) * (ay - y) / (ay - tip.1);
            pts.push((xx, y));
        }
    }
    foliage(&mut c, &pts, Ramp::Crop, Ramp::ClothRed, 6, seed);
    done(c, ax, ay, true)
}

/// A stone bird bath: a bowl on a pedestal, a little water in it catching the sky.
fn bird_bath() -> Sprite {
    let (mut c, ax, ay) = canvas(16, 20);
    c.polygon_lit(&[(5, ay - 1), (10, ay - 1), (11, ay), (4, ay)], Ramp::Stone, 60, Z::flat(1));
    c.polygon_lit(&[(6, ay - 8), (9, ay - 8), (9, ay - 1), (6, ay - 1)], Ramp::Stone, 90, Z::flat(1));
    c.ellipse_lit(Rect::new(1, ay - 13, 14, 6), Ramp::Stone, Z::flat(1));
    for x in 4..12 {
        px(&mut c, x, ay - 11, Ramp::Water.at(if x < 7 { Tone::High } else { Tone::Light }));
        px(&mut c, x, ay - 10, Ramp::Water.at(Tone::Base));
    }
    done(c, ax, ay, true)
}

/// A sundial: a brass dial on a stone pedestal, its gnomon throwing the hour.
fn sundial() -> Sprite {
    let (mut c, ax, ay) = canvas(12, 18);
    c.polygon_lit(&[(3, ay - 1), (8, ay - 1), (9, ay), (2, ay)], Ramp::Stone, 60, Z::flat(1));
    c.polygon_lit(&[(4, ay - 9), (7, ay - 9), (7, ay - 1), (4, ay - 1)], Ramp::Stone, 90, Z::flat(1));
    c.ellipse_lit(Rect::new(1, ay - 12, 10, 4), Ramp::Brass, Z::flat(1));
    c.line((4, ay - 11), (7, ay - 13), Ramp::Brass.at(Tone::Shade), 1, 1);
    c.dot(4, ay - 11, Ramp::Brass.at(Tone::High), 1);
    done(c, ax, ay, true)
}

/// A bicycle left lying in the long grass, rusted: two wheels, its frame, grass up through it.
fn bike(seed: u32) -> Sprite {
    let (mut c, ax, ay) = canvas(26, 18);
    let rust = Ramp::Copper;
    for cx in [6, 19] {
        for a in 0..32 {
            let ang = jane_core::angle::Angle((a * 2048) as u16);
            let (s, co) = (jane_core::angle::sin_q15(ang).0, jane_core::angle::cos_q15(ang).0);
            let (x, y) = (cx + ((co * 5) >> 15), ay - 5 + ((s * 5) >> 15));
            px(&mut c, x, y, Ramp::Iron.at(if s < 0 { Tone::Lift } else { Tone::Shade }));
        }
        px(&mut c, cx, ay - 5, rust.at(Tone::Base));
    }
    for (a, b) in [
        ((6, ay - 5), (12, ay - 5)),
        ((12, ay - 5), (16, ay - 11)),
        ((16, ay - 11), (9, ay - 11)),
        ((9, ay - 11), (6, ay - 5)),
        ((16, ay - 11), (19, ay - 5)),
        ((9, ay - 11), (12, ay - 5)),
    ] {
        c.line(a, b, rust.at(Tone::Base), 1, 1);
    }
    c.hline(8, 11, ay - 13, Ramp::Leather.at(Tone::Base), 1);
    c.line((16, ay - 11), (17, ay - 14), rust.at(Tone::Mid), 1, 1);
    c.hline(15, 19, ay - 14, Ramp::Iron.at(Tone::Shade), 1);
    // Grass up through it.
    for i in 0..7 {
        let h = crate::hash::h32(seed, i, 0xb1e);
        let x = 2 + (h % 22) as i32;
        let tall = 3 + (h >> 8) as i32 % 4;
        for y in ay - tall..=ay {
            px(&mut c, x, y, Ramp::TurfDry.at(if y == ay - tall { Tone::Light } else { Tone::Base }));
        }
    }
    done(c, ax, ay, true)
}
