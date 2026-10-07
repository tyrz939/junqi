//! The ways as walked: every footpath, lane and link lane kept clear from where it leaves the road to
//! where it meets its place, and every door with room to stand in front of it.
//!
//! What is trodden is recorded as it is laid (`County::trodden`, `County::ways`) and claimed before
//! the country fills up, so nothing placed after stands on it. What was there first and what a
//! later stage paints without asking are put right in the `ways` stage ([`clear_ways`]): growth
//! across a way gives way, a fence it crosses gets a gate, and a crate, a rock or a stump on it is
//! moved to the nearest open ground beside it rather than lost. [`audit`] measures all of it; the
//! county's tests hold it to zero (`tests/county_ways.rs`).

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::blueprint::PropSpawn;
use jane_core::tile::F_SOLID;
use jane_core::{Key, PropDefId, Rect, Tile};
use jane_data::PropDef;

use super::County;
use crate::kit::Kit;

/// What a way was laid as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WayKind {
    /// A place's lane down to the road (`country::lane`).
    Lane,
    /// A line joined round a set place's box to its gate (`links`).
    Link,
    /// A footpath between sites (`roads::lay_paths`).
    Footpath,
    /// A story's path from the road to a place off it (`stories`).
    Story,
}

/// A way as laid: its centre line from its place's end (a lane's door or yard, a link's gate, a
/// story's door or edge) to the road, and the door's approach cell it is meant to begin at, if it leads to a door.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Way {
    pub kind: WayKind,
    pub line: Vec<(i32, i32)>,
    pub door: Option<(i32, i32)>,
}

/// The buildings drawn with a front door (`data/looks/buildings.json`): a cottage, the farmhouse,
/// the barn, a shed, the inn, a reed hut. The door stands in the middle of the front wall, give or
/// take half a cell (`jane-art`'s `door_offset`).
const DOORED: [&str; 11] = [
    "cottage_thatch",
    "cottage_timber",
    "cottage_slate",
    "cottage_tile",
    "cottage_empty",
    "tale_cottage_boarded",
    "farmhouse",
    "barn",
    "shed",
    "inn",
    "reed_hut",
];

/// A door prop set in a wall: its approach is the cell in front of it.
const DOORS: [&str; 2] = ["door", "door_talk"];

/// The cells in front of a building's door or a door prop at `(x, y)`, front row first: two
/// across the middle of the front wall and the two in front of those; `None` for anything
/// without a door.
pub fn door_approach(def: &PropDef, x: i32, y: i32) -> Option<[(i32, i32); 4]> {
    let (w, h) = (i32::from(def.w), i32::from(def.h));
    if DOORED.contains(&def.id) {
        let (a, b) = (x + w / 2 - 1, x + w / 2);
        return Some([(a, y + h), (b, y + h), (a, y + h + 1), (b, y + h + 1)]);
    }
    if DOORS.contains(&def.id) {
        let a = x + (w - 1) / 2;
        return Some([(a, y + h), (a, y + h), (a, y + h + 1), (a, y + h + 1)]);
    }
    None
}

/// Where a building at `(x, y)` wants its lane to begin: the left of its two approach cells.
pub fn door_step(def: PropDefId, x: i32, y: i32) -> (i32, i32) {
    let d = jane_data::catalog().story.prop(def);
    (x + i32::from(d.w) / 2 - 1, y + i32::from(d.h))
}

/// Whether a prop of `def` stands in her way where it stands: solid, and not a gate (a way in).
pub fn stops_feet(def: &PropDef, p: &PropSpawn) -> bool {
    def.solid && !def.gate && !p.hidden
}

/// The cells a solid prop's feet stand in (`PropDef::solid_parts`, sixteenths of a cell from its
/// footprint's top-left): a trunk's one cell, a crate's front row, a house's whole footprint.
pub fn feet_cells(def: &PropDef, x: i32, y: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for r in def.solid_parts() {
        if r.w <= 0 || r.h <= 0 {
            continue;
        }
        let (cx0, cy0) = (r.x.div_euclid(16), r.y.div_euclid(16));
        let (cx1, cy1) = ((r.x + r.w - 1).div_euclid(16), (r.y + r.h - 1).div_euclid(16));
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                if !out.contains(&(x + cx, y + cy)) {
                    out.push((x + cx, y + cy));
                }
            }
        }
    }
    out
}

/// Mark `(x, y)` trodden if it is inside the county.
pub fn tread(trodden: &mut [bool], k: &Kit, x: i32, y: i32) {
    if k.inside(x, y) {
        trodden[(y * k.w() + x) as usize] = true;
    }
}

/// Whether `(x, y)` is trodden (outside counts as not).
pub fn trodden_at(c: &County<'_>, x: i32, y: i32) -> bool {
    c.k.inside(x, y) && c.trodden[(y * c.k.w() + x) as usize]
}

/// Claim every trodden cell: nothing placed after this stands on a way.
pub fn claim_trodden(c: &mut County<'_>) {
    let w = c.k.w();
    for i in 0..c.trodden.len() {
        if c.trodden[i] {
            let i = i as i32;
            c.k.claim(Rect::new(i % w, i / w, 1, 1));
        }
    }
}

/// Fences, walls and hedges: a way through one wants a gate.
const fn barrier(t: Tile) -> bool {
    matches!(t, Tile::Fence | Tile::StoneWall | Tile::Hedge)
}

/// Growth a way clears: it was trodden before anything grew there again.
const fn growth(t: Tile) -> bool {
    matches!(t, Tile::Tree | Tile::DeadTree | Tile::Bush)
}

/// Where a story's path, laid to its place's edge before the story's rows went down, now ends
/// against a building's blind side: its last few cells are taken up again and a walk laid from the
/// building's door round to what is left of it.
fn rejoin_blind_ends(c: &mut County<'_>) {
    const TRIM: usize = 4;
    let cat = jane_data::catalog();
    for wi in 0..c.ways.len() {
        if c.ways[wi].kind != WayKind::Story || c.ways[wi].door.is_some() || c.ways[wi].line.len() <= TRIM + 1 {
            continue;
        }
        let (p0, p1) = (c.ways[wi].line[0], c.ways[wi].line[1]);
        let q = (p0.0 + (p0.0 - p1.0).signum(), p0.1 + (p0.1 - p1.1).signum());
        let Some(step) = c.k.blueprint().props.iter().find_map(|p| {
            let d = cat.story.prop(p.def);
            let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
            (DOORED.contains(&d.id) && stops_feet(d, p) && feet_cells(d, x, y).contains(&q))
                .then(|| door_step(p.def, x, y))
        }) else {
            continue;
        };
        let stub: Vec<(i32, i32)> = c.ways[wi].line[..TRIM]
            .iter()
            .flat_map(|&(x, y)| [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)])
            .collect();
        for &(x, y) in &stub {
            if c.k.inside(x, y) {
                c.trodden[(y * c.k.w() + x) as usize] = false;
            }
        }
        let to = c.ways[wi].line[TRIM];
        let walk =
            super::country::tread_way(c, step, 2, Some(step), super::country::Reach::Way, Some(to), WayKind::Lane);
        for (x, y) in stub {
            let on_walk = walk.iter().any(|&(wx, wy)| (x - wx == 0 || x - wx == 1) && (y - wy == 0 || y - wy == 1));
            if !on_walk && c.k.get(x, y) == Tile::Dirt && !trodden_at(c, x, y) {
                c.k.set(x, y, Tile::Grass);
            }
        }
        c.ways[wi].line.drain(..TRIM);
    }
}

/// How far a prop in the way is moved looking for open ground.
const MOVE_REACH: i32 = 4;

/// What is read where it stands: a board by its place, a post at its fork. Never moved.
const SIGNS: [&str; 6] = ["name_board", "sign", "signpost", "fingerpost", "milestone", "notice"];

/// The `ways` stage: every trodden cell and every door's approach cleared. Growth on them gives
/// way to trodden dirt; a fence across a way is opened for a gate; a small thing standing on one
/// (a crate, a rock, a stump, a hay cart) is moved to the nearest open ground off it.
pub fn clear_ways(c: &mut County<'_>) {
    rejoin_blind_ends(c);
    let cat = jane_data::catalog();
    let (w, h) = (c.k.w(), c.k.h());
    // The door steps join the trodden ground for the sweep (not for the record). A set place's
    // box is drawn by hand and left as it is.
    let boxes: Vec<Rect> = c.chunks.iter().map(|ch| ch.bounds).collect();
    let in_box = |x: i32, y: i32| boxes.iter().any(|b| b.contains(x, y));
    let mut keep = c.trodden.clone();
    for p in &c.k.blueprint().props {
        let d = cat.story.prop(p.def);
        if let Some(cells) = door_approach(d, i32::from(p.cell.x), i32::from(p.cell.y)) {
            for (x, y) in cells {
                if c.k.inside(x, y) && !in_box(x, y) {
                    keep[(y * w + x) as usize] = true;
                }
            }
        }
    }
    let kept = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && keep[(y * w + x) as usize];
    for y in 0..h {
        for x in 0..w {
            if !kept(x, y) {
                continue;
            }
            let t = c.k.get(x, y);
            if growth(t) || (barrier(t) && c.trodden[(y * w + x) as usize]) {
                c.k.set(x, y, Tile::Dirt);
            }
        }
    }
    // A fence a way's centre line crosses: a gate, the way's width.
    for wi in 0..c.ways.len() {
        for pi in 0..c.ways[wi].line.len() {
            let (x, y) = c.ways[wi].line[pi];
            if barrier(c.k.get(x, y)) && !in_box(x, y) {
                for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0), (0, -1)] {
                    if barrier(c.k.get(x + ox, y + oy)) && !in_box(x + ox, y + oy) {
                        c.k.set(x + ox, y + oy, Tile::Dirt);
                    }
                }
            }
        }
    }
    // Every cell a solid prop's feet stand in, by prop.
    let mut feet = vec![u32::MAX; (w * h) as usize];
    let props = &c.k.blueprint().props;
    let unders: Vec<Key> = props.iter().filter_map(|p| p.under).collect();
    let mut blocking = Vec::new();
    for (i, p) in props.iter().enumerate() {
        let d = cat.story.prop(p.def);
        if !stops_feet(d, p) {
            continue;
        }
        let cells = feet_cells(d, i32::from(p.cell.x), i32::from(p.cell.y));
        let mut on_way = false;
        for &(x, y) in &cells {
            if c.k.inside(x, y) {
                feet[(y * w + x) as usize] = i as u32;
                on_way |= kept(x, y);
            }
        }
        if on_way {
            blocking.push(i);
        }
    }
    let mut gone = Vec::new();
    for i in blocking {
        let p = c.k.blueprint().props[i].clone();
        let d = cat.story.prop(p.def);
        // Not a building, a door, a board or a sign, nor anything hidden under another.
        let movable = door_approach(d, 0, 0).is_none()
            && !SIGNS.contains(&d.id)
            && p.to.is_none()
            && p.under.is_none()
            && !unders.contains(&p.key);
        if !movable {
            continue;
        }
        let (px, py) = (i32::from(p.cell.x), i32::from(p.cell.y));
        let (fw, fh) = (i32::from(d.w), i32::from(d.h));
        let open = |x: i32, y: i32, feet: &[u32]| {
            (y..y + fh).all(|j| {
                (x..x + fw).all(|ii| {
                    c.k.inside(ii, j)
                        && !kept(ii, j)
                        && c.k.get(ii, j).flags() & F_SOLID == 0
                        && !matches!(c.k.get(ii, j), Tile::Water | Tile::Road | Tile::Boardwalk)
                        && (feet[(j * w + ii) as usize] == u32::MAX || feet[(j * w + ii) as usize] == i as u32)
                })
            })
        };
        let mut to = None;
        'rings: for r in 1..=MOVE_REACH {
            for j in -r..=r {
                for ii in -r..=r {
                    if ii.abs().max(j.abs()) != r {
                        continue;
                    }
                    if open(px + ii, py + j, &feet) {
                        to = Some((px + ii, py + j));
                        break 'rings;
                    }
                }
            }
        }
        for (x, y) in feet_cells(d, px, py) {
            if c.k.inside(x, y) && feet[(y * w + x) as usize] == i as u32 {
                feet[(y * w + x) as usize] = u32::MAX;
            }
        }
        match to {
            Some((nx, ny)) => {
                for (x, y) in feet_cells(d, nx, ny) {
                    if c.k.inside(x, y) {
                        feet[(y * w + x) as usize] = i as u32;
                    }
                }
                c.k.claim(Rect::new(nx, ny, fw, fh));
                c.k.props_mut()[i].cell = crate::kit::cell(nx, ny);
            }
            // Nowhere to put it: only a nameless thing (a rock, a stump) is let go.
            None if matches!(p.key, Key::Local(_)) && p.loot.is_empty() && p.talk.is_none() => gone.push(p.key),
            None => {
                for (x, y) in feet_cells(d, px, py) {
                    if c.k.inside(x, y) {
                        feet[(y * w + x) as usize] = i as u32;
                    }
                }
            }
        }
    }
    if !gone.is_empty() {
        c.k.retain_props(|p| !gone.contains(&p.key));
    }
}

/// What [`audit`] found: counts, and a line saying where for each.
#[derive(Clone, Debug, Default)]
pub struct Audit {
    /// Trodden cells a solid tile stands on (growth, a fence, a wall).
    pub tile_blocked: u32,
    /// Solid props whose feet stand on a trodden cell.
    pub prop_blocked: u32,
    /// Ways whose centre line runs through a fence, a wall or a hedge with no gate.
    pub fence_crossed: u32,
    /// Ways that end at a fence, a wall, a hedge or the blind side of a building.
    pub blind_end: u32,
    /// Lanes laid for a door that do not begin at its approach.
    pub short_of_door: u32,
    /// Doors whose approach is blocked by growth, a fence or a prop.
    pub door_blocked: u32,
    pub notes: Vec<String>,
}

impl Audit {
    pub fn total(&self) -> u32 {
        self.tile_blocked
            + self.prop_blocked
            + self.fence_crossed
            + self.blind_end
            + self.short_of_door
            + self.door_blocked
    }
}

/// The ways measured as the county stands: see [`Audit`]. Set places' boxes are drawn by hand and
/// left out.
pub fn audit(c: &County<'_>) -> Audit {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let (w, h) = (c.k.w(), c.k.h());
    let in_box = |x: i32, y: i32| c.chunks.iter().any(|ch| ch.bounds.contains(x, y));
    let mut a = Audit::default();
    let solid_tile = |x: i32, y: i32| c.k.get(x, y).flags() & F_SOLID != 0 && c.k.get(x, y) != Tile::Water;
    // Solid props by cell, and every door's approach.
    let mut feet = vec![false; (w * h) as usize];
    let mut building = vec![false; (w * h) as usize];
    let mut door_cells = vec![false; (w * h) as usize];
    let mut doors = Vec::new();
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        if let Some(app) = door_approach(d, x, y) {
            doors.push((d.id, x, y, app));
            for &(ax, ay) in &app[..2] {
                if c.k.inside(ax, ay - 1) {
                    door_cells[((ay - 1) * w + ax) as usize] = true;
                }
            }
        }
        if !stops_feet(d, p) {
            continue;
        }
        let cells = feet_cells(d, x, y);
        let mut on_way = None;
        for &(fx, fy) in &cells {
            if !c.k.inside(fx, fy) {
                continue;
            }
            feet[(fy * w + fx) as usize] = true;
            if DOORED.contains(&d.id) {
                building[(fy * w + fx) as usize] = true;
            }
            if trodden_at(c, fx, fy) && !in_box(fx, fy) {
                on_way = Some((fx, fy));
            }
        }
        if let Some((fx, fy)) = on_way {
            a.prop_blocked += 1;
            let by = c
                .ways
                .iter()
                .find(|w| {
                    w.line.iter().any(|&(lx, ly)| (fx - lx == 0 || fx - lx == 1) && (fy - ly == 0 || fy - ly == 1))
                })
                .map(|w| (w.kind, w.line.first().copied()));
            a.notes.push(format!("a {} at ({x}, {y}) stands on a way at ({fx}, {fy}), {by:?}", d.id));
        }
    }
    for y in 0..h {
        for x in 0..w {
            if c.trodden[(y * w + x) as usize] && solid_tile(x, y) && !in_box(x, y) {
                a.tile_blocked += 1;
                if a.tile_blocked <= 20 {
                    a.notes.push(format!("{:?} on a way at ({x}, {y})", c.k.get(x, y)));
                }
            }
        }
    }
    let is_feet = |x: i32, y: i32| c.k.inside(x, y) && feet[(y * w + x) as usize];
    for way in &c.ways {
        if let Some(&(x, y)) = way.line.iter().find(|&&(x, y)| barrier(c.k.get(x, y)) && !in_box(x, y)) {
            a.fence_crossed += 1;
            a.notes.push(format!("a {:?} crosses {:?} at ({x}, {y})", way.kind, c.k.get(x, y)));
        }
        // Its place's end, and the cell a step on past it.
        let (p0, p1) = match way.line.len() {
            0 | 1 => continue,
            _ => (way.line[0], way.line[1]),
        };
        let (sx, sy) = ((p0.0 - p1.0).signum(), (p0.1 - p1.1).signum());
        let q = (p0.0 + sx, p0.1 + sy);
        let door = c.k.inside(q.0, q.1) && door_cells[(q.1 * w + q.0) as usize];
        let blind = barrier(c.k.get(q.0, q.1))
            || matches!(c.k.get(q.0, q.1), Tile::Wall | Tile::HouseWall)
            || (c.k.inside(q.0, q.1) && building[(q.1 * w + q.0) as usize]);
        // A way that begins on its door's step meets its door, whichever way it then leaves.
        let at_door = way.door.is_some_and(|d| (p0.0 - d.0).abs() + (p0.1 - d.1).abs() <= 1);
        if blind && !door && !at_door && !in_box(q.0, q.1) {
            a.blind_end += 1;
            let what = bp
                .props
                .iter()
                .find(|p| {
                    let d = cat.story.prop(p.def);
                    stops_feet(d, p) && feet_cells(d, i32::from(p.cell.x), i32::from(p.cell.y)).contains(&q)
                })
                .map_or("nothing", |p| cat.story.prop(p.def).id);
            a.notes.push(format!(
                "a {:?} ends at ({}, {}) against {:?} and {what} (from {:?})",
                way.kind,
                p0.0,
                p0.1,
                c.k.get(q.0, q.1),
                way.line.last()
            ));
        }
        if let Some(d) = way.door {
            if (p0.0 - d.0).abs() + (p0.1 - d.1).abs() > 1 {
                a.short_of_door += 1;
                a.notes.push(format!("a lane for the door at {d:?} begins at {p0:?}"));
            }
        }
    }
    for (id, x, y, app) in doors {
        if in_box(x, y) {
            continue;
        }
        let bad = app[..2].iter().chain(&app[2..]).find(|&&(ax, ay)| solid_tile(ax, ay) || is_feet(ax, ay));
        if let Some(&(ax, ay)) = bad {
            a.door_blocked += 1;
            a.notes
                .push(format!("the {id} at ({x}, {y}): its door is blocked at ({ax}, {ay}) by {:?}", c.k.get(ax, ay)));
        }
    }
    a
}
