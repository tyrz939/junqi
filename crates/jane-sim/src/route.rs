//! The way to a quest step's place, read off the county as built (the owner's playtest of 2
//! October 2026: "quest directions can feel confusing, giving clearer directions would help on
//! top of names"). From Castle along the roads to the road nearest the place: which way the road
//! leaves Castle, each fingerpost the way turns at and what the post says that way, and where it
//! leaves the road. Every word is something the county has, in the order the walk meets it
//! (QUEST-TREE.md §2, "the words are true"; `tests/route.rs` holds it on seeds 1 to 8).
//!
//! Presentation only: the Log and the tracker read it; nothing in the sim does. A [`Roads`] is
//! built once per county (a walk over its road cells from Castle), a [`Route`] once per step.

use jane_core::action::{Action, ListRef, TextRef};
use jane_core::blueprint::StoryPlace;
use jane_core::{Blueprint, Key, Lookup, QuestId, Rect, Tile, ZoneId};
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
const NONE: u8 = u8::MAX;
const START: u8 = u8::MAX - 1;

/// The eight steps, the four straight ones first.
const STEPS: [(i32, i32); 8] = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];

/// Ground the way runs on: the roads' metal, their bridges, Castle's cobbles.
pub fn road(t: Tile) -> bool {
    matches!(t, Tile::Road | Tile::Boardwalk | Tile::Cobble)
}

/// One thing the way meets, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Leg {
    /// Out of Castle at `at`, heading `dir`: by a road (`road`, one is a few steps on), else
    /// across the ground to one.
    Out { at: (i32, i32), dir: &'static str, road: bool },
    /// At the fingerpost standing at `post`, by the road at `at`, `metres` on from the last leg:
    /// go `dir`. `sign`: a place the post names that way, in its own words.
    Post { post: (i32, i32), at: (i32, i32), dir: &'static str, sign: Option<String>, metres: i32 },
    /// Where the way leaves the road for good, at `at`, `metres` on: the place is `dir` of it,
    /// `off` metres on foot (0: by the road).
    Off { at: (i32, i32), dir: &'static str, off: i32, metres: i32 },
    /// In Castle itself: the place is `dir` of the square's mark at `from`, `off` metres.
    InTown { from: (i32, i32), dir: &'static str, off: i32 },
}

/// The way from Castle to a place: its legs, and the cells it walks (Castle's square first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
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

/// The county walked from Castle's square, keeping to the roads: the step each cell was reached
/// by, the fingerposts, Castle's ground and its square.
#[derive(Debug)]
pub struct Roads {
    w: i32,
    town: Rect,
    square: (i32, i32),
    /// By `y * w + x`: the step ([`STEPS`]) a cell was reached by; [`START`] for the square,
    /// [`NONE`] for one never reached.
    from: Vec<u8>,
    /// By `y * w + x`: what reaching the cell cost, metres (saturating).
    cost: Vec<u16>,
    /// By `y * w + x`, a bit each: a road's cell.
    on_road: Vec<u64>,
    posts: Vec<Post>,
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

impl Roads {
    /// The county's (`None` for a blueprint with no Castle).
    pub fn new(county: &Blueprint) -> Option<Roads> {
        let cat = jane_data::catalog();
        let town = county.rects.iter().find(|(k, _)| is_named(county, **k, "site_town")).map(|(_, r)| *r)?;
        let square = county
            .marks
            .iter()
            .find(|(k, _)| is_named(county, **k, "town_square"))
            .map_or(town.centre(), |(_, m)| (i32::from(m.cell.x), i32::from(m.cell.y)));
        let (w, h) = (county.tiles.w() as i32, county.tiles.h() as i32);
        let ix = |x: i32, y: i32| (y * w + x) as u32;
        // What stands on the road and stops her: a solid prop's cells (a gate while it is locked).
        let mut blocked = Lookup::with_capacity(1 << 12);
        for p in &county.props {
            let d = cat.story.prop(p.def);
            if d.solid && !p.hidden && (!d.gate || p.locked) {
                for (x, y) in d.solid_rect(i32::from(p.cell.x), i32::from(p.cell.y)).cells() {
                    blocked.insert(ix(x, y), ());
                }
            }
        }
        let open = |x: i32, y: i32| {
            county.tiles.inside(x, y)
                && county.tiles.read(x, y, Tile::Void).flags() & jane_core::tile::F_SOLID == 0
                && !blocked.contains(&ix(x, y))
        };
        // The cheapest walk from the square (a bucket queue: costs are small integers).
        let mut from = vec![NONE; (w * h) as usize];
        let mut cost = vec![u32::MAX; (w * h) as usize];
        let mut buckets: Vec<Vec<(i32, i32)>> = vec![Vec::new(); OFF_ROAD.1 as usize + 1];
        if !open(square.0, square.1) {
            return None;
        }
        from[ix(square.0, square.1) as usize] = START;
        cost[ix(square.0, square.1) as usize] = 0;
        buckets[0].push(square);
        let (mut now, mut left) = (0u32, 1usize);
        while left > 0 {
            let b = now as usize % buckets.len();
            let Some((x, y)) = buckets[b].pop() else {
                now += 1;
                continue;
            };
            left -= 1;
            if cost[ix(x, y) as usize] != now {
                continue;
            }
            for (k, (dx, dy)) in STEPS.into_iter().enumerate() {
                let (nx, ny) = (x + dx, y + dy);
                if !open(nx, ny) {
                    continue;
                }
                let diag = dx != 0 && dy != 0;
                if diag && !(open(x + dx, y) && open(x, y + dy)) {
                    continue;
                }
                let on = road(county.tiles.read(nx, ny, Tile::Void));
                let step = match (on, diag) {
                    (true, false) => ON_ROAD.0,
                    (true, true) => ON_ROAD.1,
                    (false, false) => OFF_ROAD.0,
                    (false, true) => OFF_ROAD.1,
                };
                let c = now + step;
                let i = ix(nx, ny) as usize;
                if c < cost[i] {
                    cost[i] = c;
                    from[i] = k as u8;
                    let n = buckets.len();
                    buckets[c as usize % n].push((nx, ny));
                    left += 1;
                }
            }
        }
        let fingerpost = cat.story.prop_id("fingerpost")?;
        let posts = county
            .props
            .iter()
            .filter(|p| p.def == fingerpost && !p.hidden)
            .filter_map(|p| {
                // A fork's post: its words name a way each ("EAST: ..."). A post to one place
                // stands where there is no choice to make.
                let words = read_words(county, p.use_list).filter(|w| w.contains(": "))?;
                Some(Post { at: (i32::from(p.cell.x), i32::from(p.cell.y)), words })
            })
            .collect();
        let cost = cost.iter().map(|&c| (c / 10).min(u32::from(u16::MAX)) as u16).collect();
        let mut on_road = vec![0u64; (w * h) as usize / 64 + 1];
        for (i, t) in county.tiles.as_slice().iter().enumerate() {
            if road(*t) {
                on_road[i / 64] |= 1 << (i % 64);
            }
        }
        Some(Roads { w, town, square, from, cost, on_road, posts })
    }

    /// Did the walk from Castle reach `(x, y)`?
    pub fn walked(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.w && y >= 0 && self.from.get((y * self.w + x) as usize).is_some_and(|&f| f != NONE)
    }

    fn cost_at(&self, x: i32, y: i32) -> u16 {
        self.cost[(y * self.w + x) as usize]
    }

    /// Is `(x, y)` a road's cell?
    pub fn is_road(&self, (x, y): (i32, i32)) -> bool {
        let i = (y * self.w + x) as usize;
        self.on_road.get(i / 64).is_some_and(|b| b & (1 << (i % 64)) != 0)
    }

    /// Castle's ground.
    pub fn town(&self) -> Rect {
        self.town
    }

    /// The way from Castle to `to` (a place's ground in the county).
    pub fn route(&self, to: Rect) -> Option<Route> {
        let (cx, cy) = to.centre();
        if self.town.contains(cx, cy) {
            let (dx, dy) = (cx - self.square.0, cy - self.square.1);
            let off = isqrt(dx, dy);
            let dir = wind(dx, dy);
            return Some(Route { to, legs: vec![Leg::InTown { from: self.square, dir, off }], path: Vec::new() });
        }
        // The cell of the place's ground (or beside it: a door, a building) the walk reached
        // cheapest; of two as cheap, the first in row order.
        let end = (0..4).find_map(|g| {
            to.grow(g).cells().filter(|&(x, y)| self.walked(x, y)).min_by_key(|&(x, y)| (self.cost_at(x, y), y, x))
        })?;
        let mut path = vec![end];
        let mut at = end;
        loop {
            match *self.from.get((at.1 * self.w + at.0) as usize)? {
                START => break,
                NONE => return None,
                k => {
                    let (dx, dy) = STEPS[usize::from(k)];
                    at = (at.0 - dx, at.1 - dy);
                    path.push(at);
                }
            }
        }
        path.reverse();
        let out = path.iter().position(|&(x, y)| !self.town.contains(x, y)).unwrap_or(0);
        let heading = |i: usize| {
            let j = (i + AHEAD).min(path.len() - 1);
            wind(path[j].0 - path[i].0, path[j].1 - path[i].1)
        };
        // Where it leaves the road for good: the last cell on a road, from Castle's edge on.
        let leave = (out..path.len()).rev().find(|&i| self.is_road(path[i])).unwrap_or(out);
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
        if !self.is_road(path[leave]) {
            // No road from Castle's edge: on foot from there.
            let off = metres_along(&path, out, path.len() - 1);
            let at = path[out];
            legs.push(Leg::Off { at, dir: wind(to.centre().0 - at.0, to.centre().1 - at.1), off, metres: 0 });
            return Some(Route { to, legs, path });
        }
        let off = metres_along(&path, leave, path.len() - 1);
        let off = if off <= BY_ROAD { 0 } else { off };
        let (cx, cy) = to.centre();
        let at = path[leave];
        legs.push(Leg::Off { at, dir: wind(cx - at.0, cy - at.1), off, metres: metres_along(&path, last, leave) });
        Some(Route { to, legs, path })
    }
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
    /// The whole way, as the Log says it.
    pub fn words(&self) -> String {
        if let [Leg::Out { .. }, Leg::Off { dir, off, metres: 0, .. }] = self.legs[..] {
            return format!("From the edge of Castle it is {dir}, about {} on foot.", metres(off));
        }
        let mut out = Vec::new();
        for l in &self.legs {
            out.push(match l {
                Leg::InTown { dir, off, .. } => {
                    format!("In Castle: {} of the square, about {}.", dir, metres(*off))
                }
                Leg::Out { dir, road: true, .. } => format!("From Castle take the road {dir}."),
                Leg::Out { dir, road: false, .. } => format!("From Castle go {dir} on foot to the road."),
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

    /// The way in one short line, for the tracker: out of Castle and the first turn.
    pub fn short(&self) -> String {
        if let [Leg::Out { .. }, Leg::Off { dir, metres: 0, .. }] = self.legs[..] {
            return format!("{} from the edge of Castle, on foot", capitalise(dir));
        }
        let mut bits = Vec::new();
        for l in self.legs.iter().take(2) {
            bits.push(match l {
                Leg::InTown { dir, .. } => format!("in Castle, {dir} of the square"),
                Leg::Out { dir, .. } => format!("{dir} from Castle"),
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
