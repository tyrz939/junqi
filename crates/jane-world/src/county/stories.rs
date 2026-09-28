//! Stories at generated places (`jane/src/world/stories.ts`). The open country's hamlets, farms,
//! cottages, inns, woodcutters' clearings, camps and ruins are rolled per seed (`country`); the
//! stories are fixed data. A story (`data/stories/*.json`) asks for "one farm in the Lowfields,
//! quiet ground, in sight of a road", and this gives it exactly one, the same one every time the
//! seed is built, never one another story has. Its rows in `data/placements` then say
//! `at: { place: <story>, slot: "yard" }` and land in that farm's yard ([`at_place`]); its words
//! say `{place:<story>}` and read "Hollins Farm" ([`crate::names::story_name`]), which is what the
//! board at the farm gate says ([`name_boards`]).
//!
//! A place off the road (a camp, a ruin, a clearing in the wood) is found by a footpath: when a
//! story claims one that cannot be seen from a road, a trodden path is laid from the nearest road
//! to it, with a fingerpost where it leaves the road ([`lay_path`]).
//!
//! A story that finds no place on a seed is skipped, with the reason written on the blueprint
//! (`Blueprint::stories`), and nothing of it is placed: no giver, so no quest that cannot be done.
//!
//! Where this parts from the TypeScript: distances compare squared, in whole cells (a place's
//! centre doubled, so an odd width is exact); "seen from a road" is `4 dx <= 5 HALF_W` and
//! `4 dy <= 5 HALF_H` (1.25 half-screens) with `jane_core::view`'s half-screen; the seed's choice
//! among the places that fit ranks by `mix32(fnv1a(step, seed, story, place))` with the place's
//! number to break a tie; the walkable ground and the ground a short walk from a tale's place are
//! `jane_core::search::fill` and `flood`.

use jane_core::action::{Action, Facing};
use jane_core::blueprint::StoryPlace;
use jane_core::hash::Fnv;
use jane_core::num::isqrt;
use jane_core::search::{Fill, fill};
use jane_core::tile::F_SOLID;
use jane_core::view::{HALF_H_CELLS, HALF_W_CELLS};
use jane_core::{Blueprint, Key, NameId, Rect, StoryId, Tile, UnitDefId};
use jane_data::{PlaceAt, PlacementDef, StoryDef};

use super::County;
use super::centre;
use super::country::roads::{compass, distance_words};
use super::country::{FIRST_CLEAR, Kind, Place, dist, in_box};
use super::placements::{apply_edit, hide_under, pick_top, put_prop};
use super::tale_ground::{OnFoot, Standing, Stood, near_on_foot, place_cells, spot_for};
use crate::kit::{Kit, js_round};
use crate::names::{Names, board_text};
use crate::steps::Step;

/// The opening is the letter's: nothing of a story's this close to the platform or Julie's gate
/// (cells, centre to centre). *Tuning.*
const KEEP_OFF: [(&str, i64); 2] = [("station", 170), ("julie_house", 100)];
/// Two stories' places stand at least this far apart, centre to centre, unless one is the other's
/// `near`: one every now and then. *Tuning.*
const SPREAD: i64 = 60;
/// Every story place is seen from a road: some cell of its footprint within this many quarters of
/// a half-screen of a made road, each way (1.25 half-screens; the quest audit's "on screen from the
/// road" is 1.5, so this keeps a margin). *Tuning.*
const SEEN_QUARTERS: i32 = 5;
/// The longest footpath a story lays, in cells from the road to the place's edge. *Tuning.*
const PATH_MAX: i32 = 160;
/// Road cells are binned this many cells square, so "the nearest road" is a look in a few bins.
const BIN: i32 = 32;
/// How far up a chain of `near` stories its root is looked for.
const CHAIN: usize = 8;

/// Kinds that stand off the road by nature, and are reached by a footpath laid for the story.
const fn pathed(k: Kind) -> bool {
    matches!(k, Kind::Camp | Kind::Ruin | Kind::Woodcutter)
}

/// Ground a footpath may be trodden over: grass, growth and scrub. Never water, a fence, a wall or
/// a field in crops.
const fn treadable(t: Tile) -> bool {
    matches!(
        t,
        Tile::Grass
            | Tile::GrassTall
            | Tile::Bush
            | Tile::Tree
            | Tile::DeadTree
            | Tile::Moss
            | Tile::DryBed
            | Tile::Sand
            | Tile::Rubble
    )
}

/// Within 1.25 half-screens each way: on screen from there.
const fn in_sight(dx: i32, dy: i32) -> bool {
    4 * dx <= SEEN_QUARTERS * HALF_W_CELLS as i32 && 4 * dy <= SEEN_QUARTERS * HALF_H_CELLS as i32
}

/// What the stories claimed: each story's place (an index into `County::places`), why each other
/// was skipped, the footpaths laid to places off the road, in claim order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Claims {
    pub claims: Vec<(StoryId, usize)>,
    pub skipped: Vec<(StoryId, String)>,
    pub paths: Vec<(StoryId, Vec<(i32, i32)>)>,
}

impl Claims {
    pub fn place_of(&self, id: StoryId) -> Option<usize> {
        self.claims.iter().find(|(s, _)| *s == id).map(|&(_, p)| p)
    }
}

/// How far a place is from the roads, both ways the story needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Reach {
    /// Some road cell is within [`in_sight`] of the footprint's edge.
    seen: bool,
    /// The nearest road cell to the footprint's edge, and its squared distance, within
    /// [`PATH_MAX`] each way.
    road: Option<(i32, i32)>,
    cells2: i64,
}

/// Road cells, binned [`BIN`] square, by bin, in cell order.
struct RoadBins {
    cols: i32,
    rows: i32,
    bins: Vec<Vec<(i32, i32)>>,
}

fn road_bins(k: &Kit) -> RoadBins {
    let (cols, rows) = ((k.w() + BIN - 1) / BIN, (k.h() + BIN - 1) / BIN);
    let mut bins = vec![Vec::new(); (cols * rows) as usize];
    for y in 0..k.h() {
        for x in 0..k.w() {
            if k.get(x, y) == Tile::Road {
                bins[((y / BIN) * cols + x / BIN) as usize].push((x, y));
            }
        }
    }
    RoadBins { cols, rows, bins }
}

fn reach(rb: &RoadBins, b: Rect) -> Reach {
    let r = PATH_MAX;
    let mut out = Reach { seen: false, road: None, cells2: i64::MAX };
    let (bx0, bx1) = ((b.x - r).div_euclid(BIN).max(0), (b.x + b.w + r).div_euclid(BIN).min(rb.cols - 1));
    let (by0, by1) = ((b.y - r).div_euclid(BIN).max(0), (b.y + b.h + r).div_euclid(BIN).min(rb.rows - 1));
    for by in by0..=by1 {
        for bx in bx0..=bx1 {
            for &(x, y) in &rb.bins[(by * rb.cols + bx) as usize] {
                let dx = (b.x - x).max(x - (b.x + b.w - 1)).max(0);
                let dy = (b.y - y).max(y - (b.y + b.h - 1)).max(0);
                out.seen |= in_sight(dx, dy);
                let d2 = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
                if d2 < out.cells2 {
                    out.cells2 = d2;
                    out.road = Some((x, y));
                }
            }
        }
    }
    let max = i64::from(PATH_MAX);
    if out.cells2 > max * max {
        out.road = None;
    }
    out
}

/// Cells under a solid thing (`y * w + x`), whatever else it does: a crate in a doorway shuts a
/// ruin as well as a wall does (hidden things stand nowhere).
pub fn stopping(bp: &Blueprint) -> Vec<bool> {
    let cat = jane_data::catalog();
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let mut blocked = vec![false; (w * h) as usize];
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        if !d.solid || p.hidden {
            continue;
        }
        let r = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
        if let Some(r) = r.intersect(Rect::new(0, 0, w, h)) {
            for (x, y) in r.cells() {
                blocked[(y * w + x) as usize] = true;
            }
        }
    }
    blocked
}

/// Cells reachable on foot from the start mark (`y * w + x`): the ground, less whatever solid
/// thing already stands on it ([`stopping`]). Every cell when the county has no start.
pub fn walkable(k: &Kit, blocked: &[bool]) -> Vec<bool> {
    let (w, h) = (k.w(), k.h());
    let n = (w * h) as usize;
    let Some(start) = jane_data::catalog().name_id("start").and_then(|s| k.blueprint().marks.get(&Key::Name(s))) else {
        return vec![true; n];
    };
    let mut reach = Fill::new();
    let s = (i32::from(start.cell.x), i32::from(start.cell.y));
    let tiles = k.blueprint().tiles.as_slice();
    fill(w as u32, h as u32, &[s], |i| !blocked[i] && tiles[i].flags() & F_SOLID == 0, &mut reach);
    let mut seen = vec![false; n];
    for r in reach.runs() {
        seen[r.cells(w as u32)].fill(true);
    }
    seen
}

/// The slots a story's rows use at its place, in row order: what a place must offer before the
/// story can have it.
fn slots_wanted(id: StoryId) -> Vec<&'static str> {
    let cat = jane_data::catalog();
    let mut out: Vec<&'static str> = Vec::new();
    for r in cat.county.placements {
        if let PlaceAt::Place { story, slot: Some(s) } = r.at {
            let s = cat.name(s);
            if story == id && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// "folk" is one of its people, "folk_2" the second.
fn folk_slot(slot: &str) -> Option<usize> {
    match slot.strip_prefix("folk") {
        Some("") => Some(1),
        Some(rest) => rest.strip_prefix('_').filter(|d| d.len() == 1).and_then(|d| d.parse().ok()),
        None => None,
    }
}

/// Does the place offer this slot: a kept cell, a named prop, enough of its people, what bites.
fn offers(p: &Place, slot: &str) -> bool {
    if let Some(n) = folk_slot(slot) {
        return p.folk.len() >= n;
    }
    if slot == "hostiles" {
        return !p.hostiles.is_empty();
    }
    p.slot(slot).is_some() || p.thing(slot).is_some()
}

/// Squared distance between two doubled points, in doubled cells.
fn d2(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(a.0 - b.0), i64::from(a.1 - b.1));
    dx * dx + dy * dy
}

/// The story `near` names, and how far down the chain `s` is from its root.
fn root_of(s: &'static StoryDef) -> (&'static StoryDef, usize) {
    let cat = jane_data::catalog();
    let (mut r, mut depth) = (s, 0);
    while let Some(n) = r.near {
        if depth >= CHAIN {
            break;
        }
        r = cat.county.story(n.story);
        depth += 1;
    }
    (r, depth)
}

/// The order stories choose in: the scarcest kind first (a seed has four farms and forty
/// clearings, so the farms are spoken for before a cottage story can crowd one out), each chain
/// together, its first place before the places that follow it. A chain is as scarce as the
/// scarcest place in it. Ties keep the order of the data.
fn claim_order(places: &[Place]) -> Vec<&'static StoryDef> {
    let stories = jane_data::catalog().county.stories;
    let count = |k| places.iter().filter(|p| p.kind.story_kind() == Some(k)).count();
    let index = |id: StoryId| stories.iter().position(|s| s.id == id).unwrap_or(usize::MAX);
    let mut scarce: Vec<(StoryId, usize)> = Vec::new();
    for s in stories {
        let (root, _) = root_of(s);
        let n = count(s.kind);
        match scarce.iter_mut().find(|(r, _)| *r == root.id) {
            Some((_, m)) => *m = (*m).min(n),
            None => scarce.push((root.id, n)),
        }
    }
    let mut out: Vec<&'static StoryDef> = stories.iter().collect();
    out.sort_by_key(|s| {
        let (root, depth) = root_of(s);
        let n = scarce.iter().find(|(r, _)| *r == root.id).map_or(0, |&(_, n)| n);
        (n, index(root.id), depth, index(s.id))
    });
    out
}

/// Why places were turned down, counted, in the order the reasons first came up.
#[derive(Default)]
struct Reasons(Vec<(String, u32)>);

impl Reasons {
    fn add(&mut self, why: String) {
        match self.0.iter_mut().find(|(w, _)| *w == why) {
            Some((_, n)) => *n += 1,
            None => self.0.push((why, 1)),
        }
    }

    fn add_n(&mut self, why: String, n: u32) {
        match self.0.iter_mut().find(|(w, _)| *w == why) {
            Some((_, m)) => *m = n,
            None => self.0.push((why, n)),
        }
    }

    fn say(&self) -> String {
        self.0.iter().map(|(w, n)| format!("{n} {w}")).collect::<Vec<_>>().join(", ")
    }
}

/// What the claim keeps while it chooses: each place's reach, worked out once; the road bins; the
/// walkable ground, once a tale asks; each place's ground a short walk from it, once a tale asks.
struct Claimer {
    seed: u32,
    reach: Vec<Option<Reach>>,
    bins: Option<RoadBins>,
    blocked: Option<Vec<bool>>,
    ground: Option<Vec<bool>>,
    on_foot: Vec<(usize, OnFoot)>,
    /// Unit keys to defs, sorted by key.
    defs: Vec<(Key, UnitDefId)>,
    /// Each site's centre, doubled, by its id.
    sites: Vec<(&'static str, (i32, i32))>,
    claims: Claims,
}

impl Claimer {
    fn new(c: &County<'_>) -> Self {
        let mut defs: Vec<(Key, UnitDefId)> = c.k.blueprint().units.iter().map(|u| (u.key, u.def)).collect();
        defs.sort_by_key(|&(k, d)| (k, d));
        Self {
            seed: c.sk.seed,
            reach: vec![None; c.places.len()],
            bins: None,
            blocked: None,
            ground: None,
            on_foot: Vec::new(),
            defs,
            sites: c.sk.sites.iter().map(|s| (s.def.id, (2 * centre(s.mx), 2 * centre(s.my)))).collect(),
            claims: Claims::default(),
        }
    }

    fn def_of(&self, key: Key) -> Option<UnitDefId> {
        let i = self.defs.partition_point(|&(k, _)| k < key);
        self.defs.get(i).filter(|&&(k, _)| k == key).map(|&(_, d)| d)
    }

    fn reach_of(&mut self, k: &Kit, i: usize, b: Rect) -> Reach {
        if let Some(r) = self.reach[i] {
            return r;
        }
        let r = reach(self.bins.get_or_insert_with(|| road_bins(k)), b);
        self.reach[i] = Some(r);
        r
    }

    fn blocked(&mut self, k: &Kit) -> &[bool] {
        self.blocked.get_or_insert_with(|| stopping(k.blueprint()))
    }

    fn ground(&mut self, k: &Kit) -> &[bool] {
        if self.ground.is_none() {
            let g = walkable(k, self.blocked(k));
            self.ground = Some(g);
        }
        self.ground.as_deref().unwrap_or_default()
    }

    /// Ground a short walk from place `i`, worked out the first time a tale asks and kept: the
    /// rows set down there later walk the same ground the tale was fitted on.
    fn on_foot(&mut self, k: &Kit, i: usize, p: &Place) -> &OnFoot {
        if let Some(n) = self.on_foot.iter().position(|(j, _)| *j == i) {
            return &self.on_foot[n].1;
        }
        let w = k.w();
        self.ground(k);
        let g = self.ground.as_deref().unwrap_or_default();
        let foot =
            near_on_foot(k, p.bounds, p.slot("board"), |x, y| x >= 0 && x < w && y >= 0 && g[(y * w + x) as usize]);
        self.on_foot.push((i, foot));
        &self.on_foot.last().expect("just pushed").1
    }

    fn follows(a: &StoryDef, b: &StoryDef) -> bool {
        a.near.is_some_and(|n| n.story == b.id)
    }

    /// The places story `s` could have now, following place `near` if it follows another, never
    /// place `also`; `out` hears why each other of its kind and region was turned down.
    fn fitting(
        &mut self,
        c: &County<'_>,
        s: &'static StoryDef,
        near: Option<usize>,
        also: Option<usize>,
        out: &mut Option<&mut Reasons>,
    ) -> Vec<usize> {
        let cat = jane_data::catalog();
        let mut say = |why: String| {
            if let Some(o) = out.as_mut() {
                o.add(why);
            }
        };
        let (lo, hi) = s.threat;
        let mut want = slots_wanted(s.id);
        want.push("board");
        let mut fits = Vec::new();
        for (i, p) in c.places.iter().enumerate() {
            if p.kind.story_kind() != Some(s.kind) || p.region != s.region {
                continue;
            }
            if self.claims.claims.iter().any(|&(_, j)| j == i) || also == Some(i) {
                say("another story's".into());
                continue;
            }
            if p.threat < lo || p.threat > hi {
                say("threat".into());
                continue;
            }
            if s.first.is_some_and(|(a, b)| u16::from(p.first) < a || u16::from(p.first) > b) {
                say("not by the first walk".into());
                continue;
            }
            if let Some(missing) = want.iter().find(|w| !offers(p, w)) {
                say(format!("no {missing}"));
                continue;
            }
            if !s.hostile.is_empty() {
                let n = p.hostiles.iter().filter(|&&k| self.def_of(k).is_some_and(|d| s.hostile.contains(&d))).count();
                if n < usize::from(s.count.max(1)) {
                    let kinds: Vec<&str> = s.hostile.iter().map(|&d| cat.combat.unit(d).id).collect();
                    say(format!("not {} {}", s.count.max(1), kinds.join("|")));
                    continue;
                }
            }
            let at = p.centre2();
            let opening =
                self.sites.iter().any(|&(id, sc)| KEEP_OFF.iter().any(|&(k, r)| k == id && d2(sc, at) < 4 * r * r));
            if opening {
                say("too near the opening".into());
                continue;
            }
            if let (Some(n), Some(near)) = (near, s.near) {
                let max = i64::from(near.max);
                if d2(c.places[n].centre2(), at) > 4 * max * max {
                    say(format!("not within {} of {}", near.max, cat.county.story(near.story).key));
                    continue;
                }
            }
            // Spread: not on top of another story, unless one follows the other.
            let crowded = self.claims.claims.iter().any(|&(id, j)| {
                let other = cat.county.story(id);
                !Self::follows(s, other)
                    && !Self::follows(other, s)
                    && d2(c.places[j].centre2(), at) < 4 * SPREAD * SPREAD
            });
            if crowded {
                say("crowds another story".into());
                continue;
            }
            let r = self.reach_of(&c.k, i, p.bounds);
            if !(r.seen || (pathed(p.kind) && r.road.is_some())) {
                say("out of sight of a road".into());
                continue;
            }
            // Her way to the board: the cell in front of it is open, and come at from somewhere
            // other than where the board will stand. The TypeScript asked it of a tale alone, and a
            // woodyard's woodpiles and stump boxed in its board's front and failed the county.
            if let Some((bx, by)) = p.slot("board") {
                let (k, w) = (&c.k, c.k.w());
                let blocked = self.blocked(k);
                let open = |x: i32, y: i32| k.inside(x, y) && !k.solid(x, y) && !blocked[(y * w + x) as usize];
                let reach = [(bx - 1, by + 1), (bx + 1, by + 1), (bx, by + 2)];
                if !open(bx, by + 1) || !reach.iter().any(|&(x, y)| open(x, y)) {
                    say("board shut in".into());
                    continue;
                }
            }
            // A tale is played at its place, not only visited: every cell the place kept open must
            // be ground she can walk to. The board is read from the cell in front of it, and that
            // cell must be come at from somewhere other than where the board will stand; the other
            // kept cells are stood on.
            if s.tale {
                let w = c.k.w();
                let g = self.ground(&c.k);
                let at =
                    |x: i32, y: i32| x >= 0 && y >= 0 && x < w && g.get((y * w + x) as usize).copied().unwrap_or(false);
                let front = |(x, y): (i32, i32)| {
                    at(x, y + 1) && [(x - 1, y + 1), (x + 1, y + 1), (x, y + 2)].iter().any(|&(i, j)| at(i, j))
                };
                let cut = p
                    .slots
                    .iter()
                    .any(|&(name, (x, y))| if name == "board" { !front((x, y)) } else { !at(x, y) && !at(x, y + 1) });
                if cut {
                    say("cut off".into());
                    continue;
                }
            }
            fits.push(i);
        }
        fits
    }
}

/// Give each story its place ([`claim_order`]). Among the places that fit, the seed chooses by a
/// hash, so the stories scatter over the region instead of piling up at whatever was built first.
/// A chain's first place is only worth having if the places it sends her on to can be found near
/// it; a tale's must have room for everything it sets down there, a short walk from its front.
fn claim_places(c: &mut County<'_>, names: &Names) -> Claimer {
    let cat = jane_data::catalog();
    let mut cl = Claimer::new(c);
    for s in claim_order(&c.places) {
        let near = match s.near {
            Some(n) => match cl.claims.place_of(n.story) {
                Some(p) => Some(p),
                None => {
                    let why = format!("the story it follows ({}) has no place", cat.county.story(n.story).key);
                    cl.claims.skipped.push((s.id, why));
                    continue;
                }
            },
            None => None,
        };
        let mut no = Reasons::default();
        let mut fits = cl.fitting(c, s, near, None, &mut Some(&mut no));
        let followers: Vec<&'static StoryDef> =
            cat.county.stories.iter().filter(|f| f.near.is_some_and(|n| n.story == s.id)).collect();
        if !followers.is_empty() && !fits.is_empty() {
            let had = fits.len() as u32;
            fits.retain(|&p| followers.iter().all(|f| !cl.fitting(c, f, Some(p), Some(p), &mut None).is_empty()));
            if fits.is_empty() {
                let who: Vec<&str> = followers.iter().map(|f| f.key).collect();
                no.add_n(format!("nothing near for {}", who.join(" and ")), had);
            }
        }
        let skip = |no: &Reasons| {
            let why = if no.0.is_empty() { format!("none in the {}", region_name(s.region)) } else { no.say() };
            format!("no {} fits: {why}", s.kind.name())
        };
        if fits.is_empty() {
            cl.claims.skipped.push((s.id, skip(&no)));
            continue;
        }
        let seed = cl.seed;
        let score = |i: usize| Fnv::new().u16(Step::CountyStory.into()).u32(seed).str(s.key).i32(c.places[i].n).mix();
        fits.sort_by_key(|&i| (score(i), c.places[i].n));
        // A tale also needs everything it sets down about the place to find room there. That is a
        // trial placing of all its things, so it is asked of the seed's choices in turn, best
        // first, and not of every place that fits.
        if s.tale {
            let mut ok = None;
            for (n, &i) in fits.iter().enumerate() {
                let p = c.places[i].clone();
                cl.on_foot(&c.k, i, &p);
                let foot = &cl.on_foot.iter().find(|(j, _)| *j == i).expect("just worked out").1;
                match tale_room(&mut c.k, foot, &p, s.id) {
                    Some(miss) => no.add(format!("no room for {}", cat.name(miss))),
                    None => {
                        ok = Some(n);
                        break;
                    }
                }
            }
            let Some(n) = ok else {
                cl.claims.skipped.push((s.id, format!("no {} fits: {}", s.kind.name(), no.say())));
                continue;
            };
            fits.drain(..n);
        }
        let i = fits[0];
        cl.claims.claims.push((s.id, i));
        let r = cl.reach_of(&c.k, i, c.places[i].bounds);
        if let (false, Some(road)) = (r.seen, r.road) {
            let p = c.places[i].clone();
            let line = lay_path(c, &p, road, &names.story(s.id));
            cl.claims.paths.push((s.id, line));
        }
    }
    cl
}

fn region_name(r: jane_data::Region) -> &'static str {
    match r {
        jane_data::Region::Lowfields => "lowfields",
        jane_data::Region::Waters => "waters",
        jane_data::Region::Works => "works",
    }
}

/// A trodden path, two cells wide, from the road straight to the nearest edge of the place, and a
/// fingerpost where it leaves the road saying where it goes. Returns the path's centre line. A set
/// place's box keeps its own ground where the path crosses it (the TypeScript trod a path across
/// the farm's grass).
fn lay_path(c: &mut County<'_>, p: &Place, road: (i32, i32), name: &str) -> Vec<(i32, i32)> {
    let b = p.bounds;
    let (tx, ty) = (road.0.clamp(b.x, b.x + b.w - 1), road.1.clamp(b.y, b.y + b.h - 1));
    let n = (tx - road.0).abs().max((ty - road.1).abs());
    let steps = i64::from(n.max(1));
    let mut line = Vec::new();
    for s in 0..=n {
        let x = road.0 + js_round(i64::from(tx - road.0) * i64::from(s), steps) as i32;
        let y = road.1 + js_round(i64::from(ty - road.1) * i64::from(s), steps) as i32;
        line.push((x, y));
        for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            if treadable(c.k.get(x + i, y + j)) && !in_box(c, x + i, y + j) {
                c.k.set(x + i, y + j, Tile::Dirt);
            }
        }
    }
    // The fingerpost: a few cells along the path from the road, to one side of it, on open ground.
    let at = line[(line.len() - 1).min(5)];
    let metres = (js_round(i64::from(n) * 11, 500) * 50).max(50);
    let cat = jane_data::catalog();
    let post = cat.story.prop_id("fingerpost").expect("a fingerpost row");
    for (ox, oy) in [(2, 0), (-3, 0), (2, 1), (-3, 1), (0, 2), (0, -2), (3, 2), (-4, 2)] {
        let (x, y) = (at.0 + ox, at.1 + oy);
        if !c.k.fits(x, y, 2, 1, 0) || c.k.solid(x, y + 1) {
            continue;
        }
        let key = c.k.local(&format!("story_post_{}", p.n));
        let label = c.k.text(&format!("A fingerpost to {name}"));
        let words = c.k.text(&format!("FOOTPATH. {}, {metres} m.", name.to_uppercase()));
        let read = c.k.list(vec![Action::Read(words)]);
        let q = c.k.prop(Some(key), post, x, y);
        q.label = Some(label);
        q.use_list = Some(read);
        break;
    }
    line
}

/// Before a tale takes a place: would every one of its things set down by position find room
/// there, in order, each a short walk from the front? The things are stood in for a moment and
/// taken away again, and no ground is cleared. Returns the key of the first that would not, or
/// `None` if all of them would. A tale that cannot be set out whole at a place does not take it.
fn tale_room(k: &mut Kit, foot: &OnFoot, p: &Place, story: StoryId) -> Option<NameId> {
    let cat = jane_data::catalog();
    let r = super::tale_ground::ON_FOOT + 32;
    let b = p.bounds;
    let mut pool = Standing::near(k, Rect::new(b.x - r, b.y - r, b.w + 2 * r, b.h + 2 * r));
    // The board goes up before the tale's things are set down, and the place's mark in front of
    // it: both are there when the rows look for room, so they are here too.
    if let Some((bx, by)) = p.slot("board") {
        let def = cat.story.prop_id("name_board").expect("a name_board row");
        pool.props.push(Stood { def, x: bx, y: by, hidden: false });
        pool.marks.push((bx, by + 1));
    }
    // The stand-ins, by the row key that put each down: where it stands.
    let mut stood: Vec<(NameId, (i32, i32))> = Vec::new();
    for row in cat.county.placements {
        if !matches!(row.at, PlaceAt::Place { story: s, .. } if s == story) {
            continue;
        }
        if row.on.is_none() && row.dx.is_none() && row.dy.is_none() {
            continue;
        }
        let cell = match row.on {
            // Exactly where the words put it, on a thing stood in above: it needs no room.
            Some(on) => {
                let Some(&(_, (ux, uy))) = stood.iter().find(|(k, _)| *k == on) else { continue };
                (ux + row.dx.map_or(0, i32::from), uy + row.dy.map_or(0, i32::from))
            }
            None => match spot_for(k, Some(foot), b, row, true, Some(&pool)) {
                Some(c) => c,
                None => return Some(row.key),
            },
        };
        let (pw, ph) = row.prop.map_or((1, 1), |t| {
            let d = cat.story.prop(t.def);
            (i32::from(d.w), i32::from(d.h))
        });
        // Stand it in, so the next row finds this one there.
        if let Some(t) = row.prop {
            pool.props.push(Stood { def: t.def, x: cell.0, y: cell.1, hidden: t.hidden });
            stood.push((row.key, cell));
        }
        if row.unit.is_some() {
            pool.units.push((if row.prop.is_some() { cell.0 + pw } else { cell.0 }, cell.1));
        }
        if row.mark.is_some() {
            let below = if row.prop.is_some() { ph } else { i32::from(row.unit.is_some()) };
            pool.marks.push((cell.0, cell.1 + below));
        }
    }
    None
}

/// The boards. Every hamlet, farm, inn, cottage and clearing gets its name at the edge that faces
/// the road; a camp or a ruin only when a story has claimed it. A claimed place's name is its
/// story's (the text already says it); the rest take the seed's list in build order after the
/// stories' share.
fn name_boards(c: &mut County<'_>, names: &Names, claims: &Claims) {
    let cat = jane_data::catalog();
    let board = cat.story.prop_id("name_board").expect("a name_board row");
    let mut next: Vec<(jane_data::PlaceKind, usize)> = Vec::new();
    for i in 0..c.places.len() {
        let p = &c.places[i];
        let Some(kind) = p.kind.story_kind() else { continue };
        let story = claims.claims.iter().find(|&&(_, j)| j == i).map(|&(s, _)| s);
        if story.is_none() && !kind.always_named() {
            continue;
        }
        let Some((bx, by)) = p.slot("board") else { continue };
        let name = match story {
            Some(s) => names.story(s),
            None => {
                let n = match next.iter_mut().find(|(k, _)| *k == kind) {
                    Some((_, n)) => n,
                    None => {
                        next.push((kind, crate::names::stories_of_kind(kind)));
                        &mut next.last_mut().expect("just pushed").1
                    }
                };
                let name = names.nth(kind, *n);
                *n += 1;
                name
            }
        };
        if name.is_empty() {
            continue;
        }
        let own = story.and_then(|s| cat.county.story(s).board);
        let text = match own {
            Some(t) => cat.text(t).replace("{NAME}", &name.to_uppercase()),
            None => board_text(kind, &name, Fnv::new().u32(c.sk.seed).i32(p.n).mix()),
        };
        let key = c.k.local(&format!("board_{}", p.n));
        let label = c.k.text(&name);
        let words = c.k.text(&text);
        let read = c.k.list(vec![Action::Read(words)]);
        let q = c.k.prop(Some(key), board, bx, by);
        q.label = Some(label);
        q.use_list = Some(read);
    }
}

/// The stories stage: each story claims a place ([`claim_places`]), the boards go up with the
/// places' names on them ([`name_boards`]), each story place gets its `story_<id>` rect (its
/// footprint) and mark (on the ground in front of its board, where a person reading it stands),
/// `Blueprint::stories` records where each landed or why not, and each story's rows go into the
/// place it claimed (`Stage::Places`).
pub fn stories(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let names = Names::new(c.sk.seed);
    let cl = claim_places(c, &names);
    name_boards(c, &names, &cl.claims);
    for &(id, i) in &cl.claims.claims {
        let (b, board) = (c.places[i].bounds, c.places[i].slot("board"));
        let name = cat.county.story(id).place_name;
        c.k.rect(Key::Name(name), b);
        if let Some((x, y)) = board {
            c.k.mark(Key::Name(name), x, y + 1, None);
        }
    }
    for s in cat.county.stories {
        let record = match cl.claims.place_of(s.id) {
            Some(i) => {
                let p = &c.places[i];
                let (bounds, kind) = (p.bounds, p.kind);
                let path = cl
                    .claims
                    .paths
                    .iter()
                    .find(|(id, _)| *id == s.id)
                    .map_or_else(Vec::new, |(_, l)| l.iter().map(|&(x, y)| crate::kit::cell(x, y)).collect());
                let kind = c.k.local(kind.name());
                let name = c.k.text(&names.story(s.id));
                StoryPlace::Placed { kind, name, bounds, path }
            }
            None => {
                let why = cl.claims.skipped.iter().find(|(id, _)| *id == s.id).map_or("not tried", |(_, w)| w.as_str());
                StoryPlace::Skipped(c.k.text(why))
            }
        };
        c.k.story(s.id, record);
    }
    c.story_claims = cl.claims;
    c.ground = cl.ground;
    c.on_foot = cl.on_foot;
    super::placements::apply_placements(c, super::placements::Stage::Places, cat.county.placements);
    posts(c, &names);
}

/// A fingerpost to a place stands this far from it at least, in cells from its edge: passed on the
/// way there, not only at it. *Tuning.*
const POST_OFF: i32 = 72;
/// How far along the roads from the nearest point a post to a place is looked for, cells walked.
/// *Tuning.*
const POST_WALK: i32 = 480;
/// Two posts to one place stand at least this far apart: one each way along the road. *Tuning.*
const POST_APART: i32 = 60;
/// The set places the quests send her to by name that have no board of their own at the road:
/// the dungeons' doors and the library.
const POSTED_SITES: [&str; 7] = ["gold_mine", "museum", "library", "butterfly_forest", "factory", "burial", "school"];

/// Squared cells from a rect's edge.
fn edge_d2(b: Rect, (x, y): (i32, i32)) -> i64 {
    let dx = i64::from((b.x - x).max(x - (b.x + b.w - 1)).max(0));
    let dy = i64::from((b.y - y).max(y - (b.y + b.h - 1)).max(0));
    dx * dx + dy * dy
}

/// The roadside fingerposts (the sweep of 28 September 2026: a place's name written only at the
/// place is not a way to it). Every story's place and every set place a quest sends her to by name
/// gets one on each way along the road nearest it, [`POST_OFF`] or more from it, saying which way
/// it lies and how far; a place by the road (not one a footpath was laid to, whose post is at the
/// path's end) gets one where the road comes nearest as well. Each post's label names the place.
fn posts(c: &mut County<'_>, names: &Names) {
    let cat = jane_data::catalog();
    let rb = road_bins(&c.k);
    let mut to: Vec<(Rect, String, bool)> = Vec::new();
    for &(id, i) in &c.story_claims.claims {
        let pathed = c.story_claims.paths.iter().any(|(s, _)| *s == id);
        to.push((c.places[i].bounds, names.story(id), !pathed));
    }
    for id in POSTED_SITES {
        if let Some(ch) = c.chunks.iter().find(|ch| ch.id() == id) {
            let name = cat.text(c.sk.site(ch.site).def.name);
            let name = name.strip_prefix("The ").map_or_else(|| name.to_owned(), |n| format!("the {n}"));
            to.push((ch.bounds, name, false));
        }
    }
    for (b, name, near) in to {
        let Some(start) = reach(&rb, b).road else { continue };
        let mut at: Vec<(i32, i32)> = Vec::new();
        // The first walk (the halt, Julie's, the town) is the letter's: no post of this kind on it.
        let first = |c: &County<'_>, (x, y): (i32, i32)| dist(&c.country.d_first, x, y) < FIRST_CLEAR;
        if near && !first(c, start) {
            at.push(start);
        }
        // Along the roads from the nearest point, nearest first: the first cell far enough off
        // each way.
        let mut seen = std::collections::BTreeSet::from([start]);
        let mut queue = std::collections::VecDeque::from([(start, 0)]);
        let mut far: Vec<(i32, i32)> = Vec::new();
        while let Some(((x, y), d)) = queue.pop_front() {
            let off = i64::from(POST_OFF);
            let apart = i64::from(POST_APART);
            if edge_d2(b, (x, y)) >= off * off
                && far.iter().all(|&f| d2(f, (x, y)) >= apart * apart)
                && !first(c, (x, y))
            {
                far.push((x, y));
                if far.len() == 2 {
                    break;
                }
            }
            if d >= POST_WALK {
                continue;
            }
            for (ox, oy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let n = (x + ox, y + oy);
                if c.k.get(n.0, n.1) == Tile::Road && seen.insert(n) {
                    queue.push_back((n, d + 1));
                }
            }
        }
        at.extend(far);
        for road in at {
            post_beside(c, road, b, &name);
        }
    }
}

/// A fingerpost to `name` (whose ground is `b`) on open ground beside the road cell `road`, with
/// room in front of it to stand and read.
fn post_beside(c: &mut County<'_>, road: (i32, i32), b: Rect, name: &str) {
    let def = jane_data::catalog().story.prop_id("fingerpost").expect("a fingerpost row");
    let open = |c: &County<'_>, x: i32, y: i32| {
        !matches!(c.k.get(x, y), Tile::Road | Tile::Boardwalk | Tile::Water)
            && !c.k.solid(x, y)
            && !c.k.is_claimed(x, y)
    };
    let mut spot = None;
    'rings: for r in 3..=7 {
        for (x, y) in [
            (road.0 + r, road.1),
            (road.0 - r - 1, road.1),
            (road.0, road.1 + r),
            (road.0, road.1 - r),
            (road.0 + r, road.1 + r),
            (road.0 - r - 1, road.1 - r),
        ] {
            let fits = open(c, x, y) && open(c, x + 1, y) && !c.k.solid(x, y + 1) && !c.k.solid(x + 1, y + 1);
            if fits && !super::country::near_chunk(c, x, y, 2) {
                spot = Some((x, y));
                break 'rings;
            }
        }
    }
    let Some((x, y)) = spot else { return };
    let (tx, ty) = (b.x + b.w / 2, b.y + b.h / 2);
    let (dx, dy) = (tx - x, ty - y);
    let tenths = i64::from(isqrt((d2((x, y), (tx, ty)) * 100) as u64));
    let words = format!("{}, {}, {}.", name.to_uppercase(), compass(dx, dy), distance_words(tenths));
    let label = c.k.text(&format!("A fingerpost to {name}"));
    let words = c.k.text(&words);
    let read = c.k.list(vec![Action::Read(words)]);
    let key = c.k.local(&format!("place_post_{x}_{y}"));
    let p = c.k.prop(Some(key), def, x, y);
    p.label = Some(label);
    p.use_list = Some(read);
}

/// A row at a story's place. The place was built and claimed whole, so nothing here searches for
/// open ground: a kept cell is exactly where the thing goes, a named prop is edited in place, and
/// a resident is replaced where she stands by the story's own person, who walks her round. A
/// tale's row set by position finds its ground a short walk from the place's front. A story the
/// seed found no place for places nothing.
pub fn at_place(c: &mut County<'_>, row: &PlacementDef) {
    let cat = jane_data::catalog();
    let PlaceAt::Place { story, slot } = row.at else { return };
    let Some(i) = c.story_claims.place_of(story) else { return };
    let p = c.places[i].clone();
    let slot = slot.map_or("", |s| cat.name(s));
    let size = |def| {
        let d = cat.story.prop(def);
        (i32::from(d.w), i32::from(d.h))
    };
    if let Some(e) = &row.edit {
        if let Some(q) = p.thing(slot).and_then(|k| super::placements::prop_index(&c.k, k)) {
            apply_edit(&mut c.k.props_mut()[q], e);
        }
        return;
    }
    if slot == "hostiles" {
        // A camp that is a story's: what stands round its fire is the story's own creature (one
        // def per camp, so a kill only counts here, and nothing comes back once it is cleared).
        if let Some(u) = row.unit {
            for spawn in c.k.units_mut().iter_mut().filter(|s| p.hostiles.contains(&s.key)) {
                spawn.def = u.def;
            }
        }
        return;
    }
    let mut cell = p.slot(slot);
    let mut patrol = Vec::new();
    if let Some(on) = row.on {
        let Some(q) = super::placements::prop_index(&c.k, Key::Name(on)) else { return };
        let at = c.k.blueprint().props[q].cell;
        cell = Some((i32::from(at.x) + row.dx.map_or(0, i32::from), i32::from(at.y) + row.dy.map_or(0, i32::from)));
    } else if row.dx.is_some() || row.dy.is_some() {
        let foot = match c.ground.as_ref() {
            Some(g) => match c.on_foot.iter().position(|(j, _)| *j == i) {
                Some(n) => Some(n),
                None => {
                    let w = c.k.w();
                    let f = near_on_foot(&c.k, p.bounds, p.slot("board"), |x, y| {
                        x >= 0 && x < w && y >= 0 && g.get((y * w + x) as usize).copied().unwrap_or(false)
                    });
                    c.on_foot.push((i, f));
                    Some(c.on_foot.len() - 1)
                }
            },
            None => None,
        };
        let foot = foot.map(|n| c.on_foot[n].1.clone());
        let Some(spot) = spot_for(&mut c.k, foot.as_ref(), p.bounds, row, false, None) else { return };
        cell = Some(spot);
    }
    if let Some(n) = folk_slot(slot) {
        let Some(&key) = p.folk.get(n - 1) else { return };
        let Some(was) = c.k.blueprint().units.iter().find(|u| u.key == key).cloned() else { return };
        c.k.retain_units(|u| u.key != key);
        cell = Some((i32::from(was.cell.x), i32::from(was.cell.y)));
        patrol = was.patrol;
    }
    let (cx, cy) = match cell {
        Some(c) => c,
        None => {
            // Beside one of the place's props: the open cell below it.
            let Some(q) = p.thing(slot).and_then(|k| super::placements::prop_index(&c.k, k)) else { return };
            let q = &c.k.blueprint().props[q];
            (i32::from(q.cell.x), i32::from(q.cell.y) + size(q.def).1)
        }
    };
    // A tale's row set by position has its cell already: `within` there was how far to look.
    let by_position = row.dx.is_some() || row.dy.is_some() || row.on.is_some();
    if let Some(t) = &row.prop {
        if !by_position && (row.keys.len() > 1 || row.within.is_some()) {
            // Several of a thing about the slot (the stones in a garden), or one beside what
            // already stands on it (the chair by the candles): each on open floor of its own.
            let within = row.within.map_or(3, i32::from);
            let cells = place_cells(&c.k, (cx, cy), within, row.keys.len(), row.key);
            let tops: Vec<usize> =
                cells.iter().zip(row.keys).map(|(&(x, y), &key)| put_prop(&mut c.k, key, t, x, y)).collect();
            if !tops.is_empty() {
                let top = pick_top(&c.k, row, &tops);
                hide_under(&mut c.k, row, top);
            }
        } else {
            let top = put_prop(&mut c.k, row.key, t, cx, cy);
            hide_under(&mut c.k, row, top);
        }
    }
    let (pw, ph) = row.prop.map_or((1, 1), |t| size(t.def));
    if let Some(u) = row.unit {
        let ux = if row.prop.is_some() { cx + pw } else { cx };
        let patrol = if row.prop.is_some() { Vec::new() } else { patrol };
        let spawn = c.k.unit(Some(Key::Name(row.key)), u.def, ux, cy, patrol);
        spawn.facing = u.facing;
        if let Some(phase) = u.phase.filter(|&ph| ph > 1) {
            spawn.phase = phase;
        }
    }
    // Below a thing, below a person; a mark on its own is where it was put.
    if let Some(m) = row.mark {
        let below = if row.prop.is_some() { ph } else { i32::from(row.unit.is_some()) };
        c.k.mark(Key::Name(m), cx, cy + below, Some(Facing::South));
    }
    if let Some(r) = row.rect {
        let (rw, rh) = (i32::from(r.w), i32::from(r.h));
        c.k.rect(Key::Name(r.name), Rect::new(cx - (rw >> 1), cy - (rh >> 1), rw, rh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folk_slots_count_the_people() {
        assert_eq!(folk_slot("folk"), Some(1));
        assert_eq!(folk_slot("folk_2"), Some(2));
        assert_eq!(folk_slot("folks"), None);
        assert_eq!(folk_slot("yard"), None);
    }

    #[test]
    fn in_sight_is_a_screen_and_a_quarter() {
        assert!(in_sight(30, 16));
        assert!(!in_sight(31, 0));
        assert!(!in_sight(0, 17));
    }

    #[test]
    fn every_chain_claims_root_first() {
        let order = claim_order(&[]);
        for (n, s) in order.iter().enumerate() {
            if let Some(near) = s.near {
                assert!(order[..n].iter().any(|r| r.id == near.story), "{} before its root", s.key);
            }
        }
    }
}
