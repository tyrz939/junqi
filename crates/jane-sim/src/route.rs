//! The way to a quest step's place, read off the county as built (the owner's playtest of 2
//! October 2026: "quest directions can feel confusing, giving clearer directions would help on
//! top of names"). From the most use of what she knows ([`Roads::start`]: where she is if she is
//! by a road, else the nearest named place she has been to or seen on a post, else where she was
//! asked, else Castle) along the roads to the road nearest the place: which way the road leaves
//! the start, each fingerpost the way turns at and what the post says that way, and where it
//! leaves the road. Every word is something the county has, in the order the walk meets it
//! (QUEST-TREE.md §2, "the words are true"; `tests/route.rs` holds it on seeds 1 to 8, from Castle,
//! every named place and points on the roads).
//!
//! Presentation only: the Log and the tracker read it; nothing in the sim does. A [`Roads`] is
//! read once per county, a [`Route`] walked (A* over the county) once per start and step.

use jane_core::action::{Action, ListRef, TextRef};
use jane_core::blueprint::StoryPlace;
use jane_core::{Blueprint, Key, QuestId, Rect, Tile, ZoneId};
use jane_data::ReqTarget;
use jane_world::county::country::roads::{compass, distance_words};

use crate::blueprints::Blueprints;

/// How far ahead along the way its heading is read, cells: a wiggle at a post is not a turn.
const AHEAD: usize = 40;
/// A fingerpost this near the way (cells, each way) is one the walk passes.
const POST_NEAR: i32 = 8;
/// A place this near the road (cells) is by it: no last leg off the road.
const BY_ROAD: i32 = 6;

/// What a step costs, tenths of a metre: on a road, and off it (the way keeps to the roads, and
/// crosses a field only where a road would go three times as far round).
const ON_ROAD: (u32, u32) = (10, 14);
const OFF_ROAD: (u32, u32) = (30, 42);
const START: u8 = u8::MAX;
/// [`START`] as a walk packs it (four bits), and the cost bits beside it.
const START_PACKED: u8 = 15;
const COST_MASK: u32 = (1 << 28) - 1;
/// The walk's queue, by cost to come plus the least the rest could cost: a step adds at most the
/// dearest step and the most one step can take off the rest, so a ring this long never wraps.
const RING: usize = 64;

/// The eight steps, the four straight ones first.
const STEPS: [(i32, i32); 8] = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];

/// A road this near her (cells, each way) is one she is on or by: the way starts where she is.
pub const NEAR_ROAD: i32 = 6;

/// Ground the way runs on: the roads' metal, their bridges, Castle's cobbles.
pub fn road(t: Tile) -> bool {
    matches!(t, Tile::Road | Tile::Boardwalk | Tile::Cobble)
}

/// Where the way starts from: the most use to her of what she knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// Where she is, on a road or by one.
    Here((i32, i32)),
    /// A named place: its name as the way says it ("Castle", "the Gold Mine"), its ground, and
    /// the cell the way starts from there (Castle's square).
    Place { name: String, ground: Rect, at: (i32, i32) },
    /// Where she was when she was asked.
    Given((i32, i32)),
}

impl Start {
    /// The cell the way starts from.
    pub fn at(&self) -> (i32, i32) {
        match self {
            Start::Here(at) | Start::Given(at) | Start::Place { at, .. } => *at,
        }
    }

    /// A named place's ground: the way says where it leaves it.
    pub fn ground(&self) -> Option<Rect> {
        match self {
            Start::Place { ground, .. } => Some(*ground),
            _ => None,
        }
    }

    /// As the way says it, after "from".
    pub fn said(&self) -> &str {
        match self {
            Start::Here(_) => "here",
            Start::Place { name, .. } => name,
            Start::Given(_) => "where you were asked",
        }
    }
}

/// One thing the way meets, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Leg {
    /// Out from the start (out of a named place's ground) at `at`, heading `dir`: by a road
    /// (`road`, one is a few steps on), else across the ground to one.
    Out { at: (i32, i32), dir: &'static str, road: bool },
    /// At the fingerpost standing at `post`, by the road at `at`, `metres` on from the last leg:
    /// go `dir`. `sign`: a place the post names that way, in its own words.
    Post { post: (i32, i32), at: (i32, i32), dir: &'static str, sign: Option<String>, metres: i32 },
    /// Where the way leaves the road for good, at `at`, `metres` on: the place is `dir` of it,
    /// `off` metres on foot (0: by the road).
    Off { at: (i32, i32), dir: &'static str, off: i32, metres: i32 },
    /// In Castle: the place is `dir` of the square's mark at `from`, `off` metres. Alone when the
    /// way starts in Castle; last, `metres` on from the leg before, when the way comes into it.
    InTown { from: (i32, i32), dir: &'static str, off: i32, metres: i32 },
}

/// The way from where it starts to a place: its legs, and the cells it walks (the start first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub from: Start,
    pub to: Rect,
    pub legs: Vec<Leg>,
    pub path: Vec<(i32, i32)>,
}

/// A fingerpost of the county: where it stands and its words.
#[derive(Clone, Debug)]
struct Post {
    at: (i32, i32),
    words: String,
}

/// A named place of the county (a site): its ground, the start the way takes from it, and the
/// fingerposts that name it.
#[derive(Clone, Debug)]
pub struct Site {
    pub id: &'static str,
    pub start: Start,
    pub posts: Vec<(i32, i32)>,
}

/// The county as the way reads it: where her feet go, the roads, the fingerposts, the named
/// places, Castle and its square.
#[derive(Debug)]
pub struct Roads {
    w: i32,
    h: i32,
    town: Rect,
    square: (i32, i32),
    /// By `y * w + x`, a bit each: ground her feet can stand on.
    open: Vec<u64>,
    /// By `y * w + x`, a bit each: a road's cell.
    on_road: Vec<u64>,
    posts: Vec<Post>,
    sites: Vec<Site>,
}

/// Is `k` the name `s`: a content name, or one the generator made.
fn is_named(bp: &Blueprint, k: Key, s: &str) -> bool {
    match k {
        Key::Name(n) => jane_data::catalog().name_id(s) == Some(n),
        Key::Local(i) => bp.local_names.get(i as usize).is_some_and(|l| l == s),
    }
}

/// The words a blueprint's prop reads out (`Action::Read`), if any.
fn read_words(bp: &Blueprint, list: Option<ListRef>) -> Option<String> {
    let words = bp.list(list?)?.iter().find_map(|a| match a {
        Action::Read(t) => Some(*t),
        _ => None,
    })?;
    Some(match words {
        TextRef::Text(t) => jane_data::catalog().text(t).to_owned(),
        TextRef::Local(_) => bp.text(words)?.to_owned(),
    })
}

fn bit(bits: &[u64], i: usize) -> bool {
    bits.get(i / 64).is_some_and(|b| b & (1 << (i % 64)) != 0)
}

impl Roads {
    /// The county's (`None` for a blueprint with no Castle).
    pub fn new(county: &Blueprint) -> Option<Roads> {
        let cat = jane_data::catalog();
        let rect_of = |name: &str| county.rects.iter().find(|(k, _)| is_named(county, **k, name)).map(|(_, r)| *r);
        let town = rect_of("site_town")?;
        let square = county
            .marks
            .iter()
            .find(|(k, _)| is_named(county, **k, "town_square"))
            .map_or(town.centre(), |(_, m)| (i32::from(m.cell.x), i32::from(m.cell.y)));
        let (w, h) = (county.tiles.w() as i32, county.tiles.h() as i32);
        let n = (w * h) as usize;
        // What stands on the road and stops her: a solid prop's cells (a gate while it is locked).
        let mut open = vec![0u64; n / 64 + 1];
        let mut on_road = vec![0u64; n / 64 + 1];
        for (i, t) in county.tiles.as_slice().iter().enumerate() {
            if t.flags() & jane_core::tile::F_SOLID == 0 {
                open[i / 64] |= 1 << (i % 64);
            }
            if road(*t) {
                on_road[i / 64] |= 1 << (i % 64);
            }
        }
        for p in &county.props {
            let d = cat.story.prop(p.def);
            if d.solid && !p.hidden && (!d.gate || p.locked) {
                for (x, y) in d.solid_rect(i32::from(p.cell.x), i32::from(p.cell.y)).cells() {
                    if county.tiles.inside(x, y) {
                        let i = (y * w + x) as usize;
                        open[i / 64] &= !(1 << (i % 64));
                    }
                }
            }
        }
        let fingerpost = cat.story.prop_id("fingerpost")?;
        let all_posts: Vec<Post> = county
            .props
            .iter()
            .filter(|p| p.def == fingerpost && !p.hidden)
            .filter_map(|p| {
                let words = read_words(county, p.use_list)?;
                Some(Post { at: (i32::from(p.cell.x), i32::from(p.cell.y)), words })
            })
            .collect();
        let mut roads = Roads { w, h, town, square, open, on_road, posts: Vec::new(), sites: Vec::new() };
        if !roads.open(square.0, square.1) {
            return None;
        }
        for s in cat.county.sites {
            let Some(ground) = rect_of(&format!("site_{}", s.id)) else { continue };
            let name = cat.text(s.name);
            let at = if s.id == "town" {
                square
            } else {
                // The road through or by its ground nearest its middle, else open ground there.
                let c = ground.centre();
                let near = |pred: &dyn Fn(i32, i32) -> bool| {
                    ground
                        .grow(3)
                        .cells()
                        .filter(|&(x, y)| pred(x, y))
                        .min_by_key(|&(x, y)| ((x - c.0).pow(2) + (y - c.1).pow(2), y, x))
                };
                let Some(at) =
                    near(&|x, y| roads.open(x, y) && roads.is_road((x, y))).or_else(|| near(&|x, y| roads.open(x, y)))
                else {
                    continue;
                };
                at
            };
            let upper = name.to_uppercase();
            let posts = all_posts.iter().filter(|p| p.words.contains(&upper)).map(|p| p.at).collect();
            let said = match name.strip_prefix("The ") {
                Some(rest) => format!("the {rest}"),
                None => name.to_owned(),
            };
            roads.sites.push(Site { id: s.id, start: Start::Place { name: said, ground, at }, posts });
        }
        // A fork's post: its words name a way each ("EAST: ..."). A post to one place stands
        // where there is no choice to make.
        roads.posts = all_posts.into_iter().filter(|p| p.words.contains(": ")).collect();
        Some(roads)
    }

    /// Can her feet stand on `(x, y)`?
    pub fn open(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && bit(&self.open, (y * self.w + x) as usize)
    }

    /// Is `(x, y)` a road's cell?
    pub fn is_road(&self, (x, y): (i32, i32)) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && bit(&self.on_road, (y * self.w + x) as usize)
    }

    /// Is a road within [`NEAR_ROAD`] of `(x, y)`?
    pub fn near_road(&self, (x, y): (i32, i32)) -> bool {
        Rect::new(x - NEAR_ROAD, y - NEAR_ROAD, 2 * NEAR_ROAD + 1, 2 * NEAR_ROAD + 1).cells().any(|c| self.is_road(c))
    }

    /// Castle's ground.
    pub fn town(&self) -> Rect {
        self.town
    }

    /// The county's named places.
    pub fn sites(&self) -> &[Site] {
        &self.sites
    }

    /// The way from Castle's square.
    pub fn castle(&self) -> Start {
        self.sites.iter().find(|s| s.id == "town").map_or_else(
            || Start::Place { name: "Castle".to_owned(), ground: self.town, at: self.square },
            |s| s.start.clone(),
        )
    }

    /// Where the way to `to` is best started from: where she is (`here`), if she is on a road or
    /// by one; else the nearest named place (to her, else to where she last was in the county,
    /// `last`) that `known` says she has been to or seen posted, not the place itself; else where
    /// she was asked (`given`); else Castle.
    pub fn start(
        &self,
        here: Option<(i32, i32)>,
        last: Option<(i32, i32)>,
        known: &dyn Fn(&Site) -> bool,
        given: Option<(i32, i32)>,
        to: Rect,
    ) -> Start {
        if let Some(at) = here
            && self.open(at.0, at.1)
            && self.near_road(at)
        {
            return Start::Here(at);
        }
        let near = here.or(last).or(given);
        let place = self
            .sites
            .iter()
            .filter(|s| known(s) && !s.start.ground().is_some_and(|g| g.grow(BY_ROAD).overlaps(to)))
            .min_by_key(|s| near.map_or(0, |(x, y)| dist2(nearest_in(s.start.ground().unwrap_or(to), (x, y)), (x, y))));
        match (place, given) {
            (Some(s), _) => s.start.clone(),
            (None, Some(at)) if self.open(at.0, at.1) => Start::Given(at),
            _ => self.castle(),
        }
    }

    /// The cheapest walk from `from` to `to` (or beside it: a door, a building), keeping to the
    /// roads: A* over the county, the least the rest could cost being the straight way by road.
    fn walk(&self, from: (i32, i32), to: Rect) -> Option<Vec<(i32, i32)>> {
        let goal = (0..4).map(|g| to.grow(g)).find(|r| r.cells().any(|(x, y)| self.open(x, y)))?;
        if !self.open(from.0, from.1) {
            return None;
        }
        let w = self.w;
        let ix = |x: i32, y: i32| (y * w + x) as usize;
        let rest = |x: i32, y: i32| {
            let (nx, ny) = nearest_in(goal, (x, y));
            let (dx, dy) = ((nx - x).unsigned_abs(), (ny - y).unsigned_abs());
            let (lo, hi) = (dx.min(dy), dx.max(dy));
            lo * ON_ROAD.1 + (hi - lo) * ON_ROAD.0
        };
        // Per cell, packed in one u32 (PLAY-PLAN.md §7: 16 MB a walk, not 20): the cost to come + 1
        // (0: not reached) in the low 28 bits, the step it was reached by in the top 4.
        let n = (self.w * self.h) as usize;
        let mut seen = vec![0u32; n];
        let pack = |c: u32, k: u8| c.min(COST_MASK) | u32::from(k.min(15)) << 28;
        let cost = |seen: &[u32], i: usize| seen[i] & COST_MASK;
        let mut ring: Vec<Vec<(i32, i32, u32)>> = vec![Vec::new(); RING];
        seen[ix(from.0, from.1)] = pack(1, START);
        let mut now = rest(from.0, from.1);
        ring[now as usize % RING].push((from.0, from.1, 0));
        let mut left = 1usize;
        let end = loop {
            if left == 0 {
                return None;
            }
            let Some((x, y, c)) = ring[now as usize % RING].pop() else {
                now += 1;
                continue;
            };
            left -= 1;
            if cost(&seen, ix(x, y)) != c + 1 {
                continue;
            }
            if goal.contains(x, y) {
                break (x, y);
            }
            for (k, (dx, dy)) in STEPS.into_iter().enumerate() {
                let (nx, ny) = (x + dx, y + dy);
                if !self.open(nx, ny) {
                    continue;
                }
                let diag = dx != 0 && dy != 0;
                if diag && !(self.open(x + dx, y) && self.open(x, y + dy)) {
                    continue;
                }
                let s = match (self.is_road((nx, ny)), diag) {
                    (true, false) => ON_ROAD.0,
                    (true, true) => ON_ROAD.1,
                    (false, false) => OFF_ROAD.0,
                    (false, true) => OFF_ROAD.1,
                };
                let nc = c + s;
                let i = ix(nx, ny);
                let had = cost(&seen, i);
                if had == 0 || nc + 1 < had {
                    seen[i] = pack(nc + 1, k as u8);
                    ring[(nc + rest(nx, ny)) as usize % RING].push((nx, ny, nc));
                    left += 1;
                }
            }
        };
        let mut path = vec![end];
        let mut at = end;
        let step = |at: (i32, i32)| (seen[ix(at.0, at.1)] >> 28) as u8;
        while step(at) != START_PACKED {
            let (dx, dy) = STEPS[usize::from(step(at))];
            at = (at.0 - dx, at.1 - dy);
            path.push(at);
        }
        path.reverse();
        Some(path)
    }

    /// The way from Castle to `to` (a place's ground in the county).
    pub fn route(&self, to: Rect) -> Option<Route> {
        self.route_from(&self.castle(), to)
    }

    /// The way from `from` to `to`.
    pub fn route_from(&self, from: &Start, to: Rect) -> Option<Route> {
        let (cx, cy) = to.centre();
        let start = from.at();
        let to_town = self.town.contains(cx, cy);
        let in_town = |dist: i32| {
            let (dx, dy) = (cx - self.square.0, cy - self.square.1);
            Leg::InTown { from: self.square, dir: wind(dx, dy), off: isqrt(dx, dy), metres: dist }
        };
        if to_town && self.town.contains(start.0, start.1) {
            return Some(Route { from: from.clone(), to, legs: vec![in_town(0)], path: Vec::new() });
        }
        let path = self.walk(start, to)?;
        // Out of the start's ground; into Castle's, when the place is there.
        let out = from.ground().and_then(|g| path.iter().position(|&(x, y)| !g.contains(x, y))).unwrap_or(0);
        let enter = if to_town {
            (out..path.len()).find(|&i| self.town.contains(path[i].0, path[i].1)).unwrap_or(path.len() - 1)
        } else {
            path.len() - 1
        };
        let heading = |i: usize| {
            let j = (i + AHEAD).min(path.len() - 1);
            wind(path[j].0 - path[i].0, path[j].1 - path[i].1)
        };
        // Where it leaves the road for good: the last cell on a road, from the start's edge on.
        let leave = (out..=enter).rev().find(|&i| self.is_road(path[i])).unwrap_or(out);
        let leave = if to_town { enter } else { leave };
        let by_road = path[out..=(out + AHEAD).min(path.len() - 1)].iter().any(|&c| self.is_road(c));
        let mut legs = vec![Leg::Out { at: path[out], dir: heading(out), road: by_road }];
        let mut dir = heading(out);
        let mut last = out;
        // The posts the way passes on the road, in the order it passes them.
        let mut passed: Vec<(usize, &Post)> = self
            .posts
            .iter()
            .filter_map(|p| {
                let (i, _) = path[..=leave]
                    .iter()
                    .enumerate()
                    .skip(out + 1)
                    .filter(|(_, c)| (c.0 - p.at.0).abs() <= POST_NEAR && (c.1 - p.at.1).abs() <= POST_NEAR)
                    .min_by_key(|(i, c)| ((c.0 - p.at.0).pow(2) + (c.1 - p.at.1).pow(2), *i))?;
                Some((i, p))
            })
            .collect();
        passed.sort_by_key(|&(i, p)| (i, p.at));
        for (i, p) in passed {
            let now = heading(i);
            // A post where the way goes on as it was is passed by.
            if now == dir || i >= leave {
                continue;
            }
            legs.push(Leg::Post {
                post: p.at,
                at: path[i],
                dir: now,
                sign: sign(&p.words, now),
                metres: metres_along(&path, last, i),
            });
            dir = now;
            last = i;
        }
        if to_town {
            legs.push(in_town(metres_along(&path, last, enter)));
            return Some(Route { from: from.clone(), to, legs, path });
        }
        if !self.is_road(path[leave]) {
            // No road from the start's edge: on foot from there.
            let off = metres_along(&path, out, path.len() - 1);
            let at = path[out];
            legs.push(Leg::Off { at, dir: wind(cx - at.0, cy - at.1), off, metres: 0 });
            return Some(Route { from: from.clone(), to, legs, path });
        }
        let off = metres_along(&path, leave, path.len() - 1);
        let off = if off <= BY_ROAD { 0 } else { off };
        let at = path[leave];
        legs.push(Leg::Off { at, dir: wind(cx - at.0, cy - at.1), off, metres: metres_along(&path, last, leave) });
        Some(Route { from: from.clone(), to, legs, path })
    }

    /// The same way, walked as far as its cell `i`, said from there: "here" is where she stands
    /// now, not where the way was set. The road out is read again from `i`, the turns and the
    /// leaving still ahead keep their places, and the first of them counts its metres from `i`.
    /// No new walk: the cells are the way's own. `None` when `i` is past the way's first turn or
    /// its end (the way is worked out again there), or the way did not start from where she was.
    pub fn walked(&self, r: &Route, i: usize) -> Option<Route> {
        if !matches!(r.from, Start::Here(_)) || i == 0 || i + 1 >= r.path.len() {
            return None;
        }
        let path = &r.path;
        let ix = |at: (i32, i32)| path.iter().position(|&c| c == at);
        let (cx, cy) = r.to.centre();
        let here = path[i];
        let heading = {
            let j = (i + AHEAD).min(path.len() - 1);
            wind(path[j].0 - here.0, path[j].1 - here.1)
        };
        let road = path[i..=(i + AHEAD).min(path.len() - 1)].iter().any(|&c| self.is_road(c));
        let mut legs = vec![Leg::Out { at: here, dir: heading, road }];
        let mut first = true;
        for l in r.legs.iter().skip(1) {
            let leg = match *l {
                Leg::Post { post, at, dir, ref sign, .. } => {
                    let k = ix(at)?;
                    if k <= i {
                        return None;
                    }
                    let metres = if first { metres_along(path, i, k) } else { leg_metres(l) };
                    Leg::Post { post, at, dir, sign: sign.clone(), metres }
                }
                Leg::Off { metres: 0, .. } => {
                    let off = metres_along(path, i, path.len() - 1);
                    let off = if road && off <= BY_ROAD { 0 } else { off };
                    Leg::Off { at: here, dir: wind(cx - here.0, cy - here.1), off, metres: 0 }
                }
                Leg::Off { at, dir, off, metres } => {
                    let k = ix(at)?;
                    if k <= i {
                        return None;
                    }
                    Leg::Off { at, dir, off, metres: if first { metres_along(path, i, k) } else { metres } }
                }
                Leg::InTown { from, dir, off, metres } => {
                    let walked = if first { metres_along(path, 0, i) } else { 0 };
                    Leg::InTown { from, dir, off, metres: (metres - walked).max(0) }
                }
                Leg::Out { .. } => continue,
            };
            first = false;
            legs.push(leg);
        }
        Some(Route { from: Start::Here(here), to: r.to, legs, path: path[i..].to_vec() })
    }
}

/// A turn's own metres, from the leg before it.
fn leg_metres(l: &Leg) -> i32 {
    match *l {
        Leg::Post { metres, .. } | Leg::Off { metres, .. } | Leg::InTown { metres, .. } => metres,
        Leg::Out { .. } => 0,
    }
}

fn dist2(a: (i32, i32), b: (i32, i32)) -> i64 {
    i64::from(a.0 - b.0).pow(2) + i64::from(a.1 - b.1).pow(2)
}

/// The wind of `(dx, dy)` in prose ("north-east").
fn wind(dx: i32, dy: i32) -> &'static str {
    match compass(dx, dy) {
        "NORTH" => "north",
        "SOUTH" => "south",
        "EAST" => "east",
        "WEST" => "west",
        "NORTH-EAST" => "north-east",
        "NORTH-WEST" => "north-west",
        "SOUTH-EAST" => "south-east",
        _ => "south-west",
    }
}

/// What a post says of the way `dir`: its first name that way, as the post writes it. A fork's
/// post reads "EAST: GOLD MINE, 300 m; MUSEUM, 1.2 km. NORTH: ..."; a place's "SCHOOL, NORTH, 400 m.".
pub fn sign(words: &str, dir: &str) -> Option<String> {
    let up = dir.to_uppercase();
    for part in words.split(". ") {
        let part = part.trim_end_matches('.');
        if let Some(rest) = part.strip_prefix(&format!("{up}: ")) {
            return rest.split(", ").next().map(str::to_owned);
        }
        let bits: Vec<&str> = part.split(", ").collect();
        if bits.len() == 3 && bits[1] == up {
            return Some(bits[0].to_owned());
        }
    }
    None
}

/// Metres walked along `path` from cell `a` to cell `b` (a diagonal step is 1.4).
fn metres_along(path: &[(i32, i32)], a: usize, b: usize) -> i32 {
    let tenths: i32 =
        (a + 1..=b).map(|i| if path[i].0 != path[i - 1].0 && path[i].1 != path[i - 1].1 { 14 } else { 10 }).sum();
    tenths / 10
}

fn isqrt(dx: i32, dy: i32) -> i32 {
    jane_core::num::isqrt((i64::from(dx).pow(2) + i64::from(dy).pow(2)) as u64) as i32
}

fn nearest_in(b: Rect, (x, y): (i32, i32)) -> (i32, i32) {
    (x.clamp(b.x, b.right() - 1), y.clamp(b.y, b.bottom() - 1))
}

/// Metres as a sign says them (a county cell is a metre).
pub fn metres(m: i32) -> String {
    distance_words(i64::from(m.max(1)) * 10)
}

impl Route {
    /// From where, as the way says it: "the edge of Castle" when it is a place's ground it walks
    /// out of on foot.
    fn start_edge(&self) -> String {
        match self.from {
            Start::Place { .. } => format!("the edge of {}", self.from.said()),
            _ => self.from.said().to_owned(),
        }
    }

    /// The whole way, as the Log says it.
    pub fn words(&self) -> String {
        let from = self.from.said();
        if let [Leg::Out { .. }, Leg::Off { dir, off, metres: 0, .. }] = self.legs[..] {
            return capitalise(&format!("From {}, it is {dir}, about {} on foot.", self.start_edge(), metres(off)));
        }
        let mut out = Vec::new();
        for l in &self.legs {
            out.push(match l {
                Leg::InTown { dir, off, metres: m, .. } if *m <= BY_ROAD => {
                    format!("In Castle: {} of the square, about {}.", dir, metres(*off))
                }
                Leg::InTown { dir, off, metres: m, .. } => {
                    format!("After about {}, in Castle: {} of the square, about {}.", metres(*m), dir, metres(*off))
                }
                Leg::Out { dir, road: true, .. } => format!("From {from}, take the road {dir}."),
                Leg::Out { dir, road: false, .. }
                    if self.legs.len() == 2 && matches!(self.legs[1], Leg::InTown { .. }) =>
                {
                    format!("From {from}, go {dir} on foot.")
                }
                Leg::Out { dir, road: false, .. } => format!("From {from}, go {dir} on foot to the road."),
                Leg::Post { dir, sign, metres: m, .. } => match sign {
                    Some(s) => format!("After about {}, at the fingerpost, go {dir}, the way it says {s}.", metres(*m)),
                    None => format!("After about {}, at the fingerpost, go {dir}.", metres(*m)),
                },
                Leg::Off { dir, off, metres: 0, .. } => format!("It is {dir}, about {} on foot.", metres(*off)),
                Leg::Off { dir, off: 0, metres: m, .. } => {
                    format!("After about {}, it is by the road, to the {dir}.", metres(*m))
                }
                Leg::Off { dir, off, metres: m, .. } => {
                    format!("After about {}, leave the road: it is {dir}, about {} on foot.", metres(*m), metres(*off))
                }
            });
        }
        capitalise(&out.join(" "))
    }

    /// The way in one short line, for the tracker: out from the start and the first turn.
    pub fn short(&self) -> String {
        if let [Leg::Out { .. }, Leg::Off { dir, metres: 0, .. }] = self.legs[..] {
            return capitalise(&format!("{dir} from {}, on foot", self.start_edge()));
        }
        let mut bits = Vec::new();
        for l in self.legs.iter().take(2) {
            bits.push(match l {
                Leg::InTown { dir, metres: 0, .. } => format!("in Castle, {dir} of the square"),
                Leg::InTown { .. } => "into Castle".to_owned(),
                Leg::Out { dir, .. } => format!("{dir} from {}", self.from.said()),
                Leg::Post { dir, .. } => format!("{dir} at the post"),
                Leg::Off { .. } => "off the road".to_owned(),
            });
        }
        capitalise(&bits.join(", then "))
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// Which way and how far a place is from her, for the tracker: "north-east of you, about 400 m".
pub fn bearing(from: (i32, i32), to: Rect) -> String {
    let (nx, ny) = nearest_in(to, from);
    let d = isqrt(nx - from.0, ny - from.1);
    if d <= BY_ROAD {
        return "Here, about you".to_owned();
    }
    let (cx, cy) = to.centre();
    capitalise(&format!("{} of you, about {}", wind(cx - from.0, cy - from.1), metres(d)))
}

// --- where a step is ---------------------------------------------------------------------------

/// Where a quest step's thing is: the zone it is in, and the ground in the county to make for
/// (the place itself, else the door down to its zone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub zone: ZoneId,
    pub rect: Rect,
}

/// The dungeons and rooms by the word a step's text calls them by, where the text says the thing
/// is in one (a name used for a direction, "near the mine", "the mine road", is not).
fn zone_in_text(text: &str) -> Option<ZoneId> {
    let t = text.to_lowercase();
    let named = |w: &str| {
        t.match_indices(w).any(|(i, _)| {
            let (before, after) = (&t[..i], &t[i + w.len()..]);
            !(before.ends_with("from the ")
                || before.ends_with("near the ")
                || before.ends_with("past the ")
                || before.ends_with("past the ruined ")
                || after.starts_with(" road"))
        })
    };
    [
        ("cellar", ZoneId::Cellar),
        ("kitchen", ZoneId::House),
        ("gold mine", ZoneId::Mine),
        ("burial", ZoneId::Burial),
        ("hoar stone", ZoneId::Burial),
        ("factory", ZoneId::Factory),
        ("butterfly forest", ZoneId::Forest),
        ("library", ZoneId::Library),
        ("museum", ZoneId::Museum),
        ("pipes", ZoneId::Pipes),
        ("school", ZoneId::School),
        ("castle arms", ZoneId::Arms),
    ]
    .into_iter()
    .find(|(w, _)| named(w))
    .map(|(_, z)| z)
}

/// The county's way into zone `z`: its door there, else the door of the zone its door is in.
fn door_to(bps: &Blueprints, z: ZoneId, depth: u8) -> Option<Rect> {
    let cat = jane_data::catalog();
    let foot = |p: &jane_core::blueprint::PropSpawn| {
        let d = cat.story.prop(p.def);
        Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
    };
    if let Some(p) = bps.get(ZoneId::County).props.iter().find(|p| p.to.is_some_and(|t| t.zone == z)) {
        return Some(foot(p));
    }
    if depth > 3 {
        return None;
    }
    ZoneId::ALL.iter().filter(|&&y| y != ZoneId::County && y != z).find_map(|&y| {
        bps.get(y).props.iter().any(|p| p.to.is_some_and(|t| t.zone == z)).then(|| door_to(bps, y, depth + 1)).flatten()
    })
}

/// Does list `r` of blueprint `bp` (or the catalog's) do `pred`, looking into its branches?
fn list_does(bp: &Blueprint, r: ListRef, pred: &impl Fn(&Action) -> bool, depth: u8) -> bool {
    let list = match r {
        ListRef::Catalog(_) => jane_data::catalog().list(r),
        ListRef::Blueprint(_) => bp.list(r).unwrap_or(&[]),
    };
    depth < 6
        && list.iter().any(|a| {
            pred(a)
                || match *a {
                    Action::If { then, els, .. } => {
                        list_does(bp, then, pred, depth + 1) || els.is_some_and(|e| list_does(bp, e, pred, depth + 1))
                    }
                    _ => false,
                }
        })
}

/// The zone and ground of a `Location` step's name: a rect of that name, a trigger or a thing
/// that marks it.
fn location(bps: &Blueprints, name: jane_core::NameId) -> Option<(ZoneId, Rect)> {
    let cat = jane_data::catalog();
    let does = |a: &Action| matches!(a, Action::Location(Key::Name(n)) if *n == name);
    for z in ZoneId::ALL {
        let bp = bps.get(z);
        if let Some(r) = bp.rects.get(&Key::Name(name)) {
            return Some((z, *r));
        }
        for t in bp.triggers.values() {
            if list_does(bp, t.actions, &does, 0)
                && let Some(r) = bp.rects.get(&t.rect)
            {
                return Some((z, *r));
            }
        }
        for t in cat.story.triggers.iter().filter(|t| t.zone == z) {
            if list_does(bp, t.trigger.actions, &does, 0)
                && let Some(r) = bp.rects.get(&t.trigger.rect)
            {
                return Some((z, *r));
            }
        }
        for p in &bp.props {
            let talks = p.talk.is_some_and(|tree| {
                cat.story.dialogue(tree).nodes.iter().any(|n| {
                    n.actions.is_some_and(|l| list_does(bp, l, &does, 0))
                        || n.options.iter().any(|o| o.actions.is_some_and(|l| list_does(bp, l, &does, 0)))
                })
            });
            if talks || p.use_list.is_some_and(|l| list_does(bp, l, &does, 0)) {
                let d = cat.story.prop(p.def);
                return Some((z, Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))));
            }
        }
    }
    None
}

/// Where step `i` of quest `q` is, on these blueprints: `None` when the words name no place the
/// county can show the way to (a potion made at the bench, a thing any rat drops).
pub fn place_of_step(bps: &Blueprints, q: QuestId, i: usize) -> Option<Place> {
    let cat = jane_data::catalog();
    let d = cat.story.quest(q);
    let r = d.requirements.get(i)?;
    let text = cat.text(r.text);
    let county = bps.get(ZoneId::County);
    let found = |zone: ZoneId, rect: Rect| {
        if zone == ZoneId::County {
            Some(Place { zone, rect })
        } else {
            door_to(bps, zone, 0).map(|rect| Place { zone, rect })
        }
    };
    // A story's place, by the name the words give it.
    if let Some(open) = text.find("{place:") {
        let key = text[open + 7..].split('}').next()?;
        let story = cat.county.stories.iter().find(|s| s.key == key)?;
        if let Some(StoryPlace::Placed { bounds, .. }) = county.stories.get(&story.id) {
            return Some(Place { zone: ZoneId::County, rect: *bounds });
        }
        return None;
    }
    let named = zone_in_text(text);
    match r.target {
        ReqTarget::Location(name) => {
            location(bps, name).and_then(|(z, r)| found(z, r)).or_else(|| found(named?, Rect::new(0, 0, 0, 0)))
        }
        ReqTarget::Kill(def) => {
            if let Some(z) = named {
                return found(z, Rect::new(0, 0, 0, 0));
            }
            ZoneId::ALL.iter().find_map(|&z| {
                let u = bps.get(z).units.iter().find(|u| u.def == def)?;
                found(z, Rect::new(i32::from(u.cell.x), i32::from(u.cell.y), 1, 1))
            })
        }
        ReqTarget::Acquire(item) => {
            if let Some(z) = named {
                return found(z, Rect::new(0, 0, 0, 0));
            }
            ZoneId::ALL.iter().find_map(|&z| {
                let p = bps.get(z).props.iter().find(|p| p.loot.iter().any(|s| s.item == item))?;
                let pd = cat.story.prop(p.def);
                found(z, Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(pd.w), i32::from(pd.h)))
            })
        }
    }
}
