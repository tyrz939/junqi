//! What a player without markers can know, and how she looks for the rest (VERIFICATION.md §2
//! L3, the **Lost** and the **Explorer**).
//!
//! The Reader resolves a quest step by the catalog's name for it over the whole zone. The Lost
//! may use a thing only once it has been **on screen** (the camera's 48 × 27 cells about her,
//! `jane_sim::trace::camera`): a prop, a person or a creature she has had in view, a door she has
//! seen, ground she has looked over. Until then she looks for it from the words alone:
//!
//! 0. **A fingerpost that names it**: every sign with words she passes by a road she reads
//!    (within [`READ_NEAR`]), and a fingerpost whose label shares words with the step (the
//!    landmark of 1) she walks to and reads. What it says, which way and how far, is a [`Lead`]:
//!    she walks the roads to where it says the place is, reading the posts at the junctions on
//!    the way (a newer one that names it turns her), until the thing is on screen. A lead walked
//!    to its end without finding it is spent.
//! 1. **A landmark the words name**: of the things she has had on screen, the one whose name or
//!    label shares the most words with the step's text (a well for "at the well on the station
//!    road"), not yet stood by; she walks to it and looks round.
//! 2. **Along the roads**: a road leaving the edge of the screen she has not walked to (she walks
//!    the roads reading signs until one names it).
//! 3. **Anywhere unseen**: the nearest ground the map has not charted ([`fog_frontier`]).
//!
//! [`Eyes`] is her own memory of the screen; every [`FORGET_EVERY`] frames the Lost drops it and
//! her objective, and rebuilds both from the quest log's words and what she sees again (the
//! journal is the sim's and is kept). The Explorer walks [`fog_frontier`] with quests incidental.

use std::collections::{BTreeMap, BTreeSet};

use jane_core::num::{CELL_FX, isqrt};
use jane_core::{QuestId, Vec2, ZoneId};
use jane_sim::View;
use jane_sim::ids::{PropId, UnitId};
use jane_sim::trace::camera;

use crate::nav::{dist, road, walkable};
use crate::sense::prop_centre;

/// Frames between the Lost's forgetting (VERIFICATION.md L3: twenty real minutes).
pub const FORGET_EVERY: u32 = 20 * 60 * 60;

/// Frames the Lost looks for one step from the words before she is told where it is (and the
/// step counts as not found from the text): twenty real minutes.
pub const SEARCH_BUDGET: u32 = 20 * 60 * 60;

/// Cells she must come within a landmark to count it looked round.
pub const CHECKED_NEAR: i32 = 6;

/// Cells round a place she died that a search keeps away from: the road that killed her is not
/// the one she is looking for.
pub const SHUNNED: i32 = 64;

/// Blocks of what she has looked at, and of the road frontier, cells.
pub const BLOCK: i32 = 8;

/// Cells from a sign's edge within which she reads it as she passes (a fingerpost stands three to
/// seven cells off the road's line).
pub const READ_NEAR: i32 = 10;

/// Cells from where a lead says the place is that count as there: walked to its end.
pub const LEAD_ARRIVED: i32 = 12;

/// Cells within which a post she has had on screen and not read is gone to and read.
pub const POST_NEAR: i32 = 32;

/// Rings about where a sign says a place is searched for a road to walk to.
const LEAD_ROAD: i32 = 40;

/// What a sign said of a place: its name's words, and the road cell nearest where it says the
/// place is (which way from the sign, and how far).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lead {
    /// The sign, and which of the places it names (a fork's post names several).
    pub post: PropId,
    pub nth: u8,
    /// The place's name as the sign writes it, and its words.
    pub name: String,
    pub words: Vec<String>,
    /// Where to walk: a road cell (else a walkable one) about where the sign says the place is.
    pub to: (i32, i32),
    /// The look it was read on (the newest reading is the nearest post).
    pub read: u32,
}

/// Her memory of the screen.
#[derive(Clone, Debug, Default)]
pub struct Eyes {
    /// Props with a verb or a name that have been on screen, and where.
    pub props: BTreeMap<(ZoneId, PropId), (i32, i32)>,
    /// Units that have been on screen, and where she last saw each.
    pub units: BTreeMap<(ZoneId, UnitId), (i32, i32)>,
    /// Zones whose door she has seen from outside.
    pub doors: BTreeSet<ZoneId>,
    /// Landmarks she has stood by.
    pub checked: BTreeSet<(ZoneId, PropId)>,
    /// Road cells at the screen's edge with ground past them never on screen, by the block past
    /// them (the county).
    /// With the look it was found on: she follows the road she is on (the newest end first),
    /// as a person walks a road to its end before turning back to the last fork.
    pub frontier: BTreeMap<(i32, i32), ((i32, i32), u32)>,
    /// Looks taken (the frontier's clock).
    pub looks: u32,
    /// Blocks the screen has covered, by zone.
    pub looked: BTreeSet<(ZoneId, i32, i32)>,
    /// Signs she has read (the county's).
    pub read: BTreeSet<PropId>,
    /// What they said of the places they name.
    pub leads: Vec<Lead>,
    /// Signs read that named a place (a newer lead may turn a walk on an older one).
    pub reads: u32,
}

pub fn block_of((x, y): (i32, i32)) -> (i32, i32) {
    (x.div_euclid(BLOCK), y.div_euclid(BLOCK))
}

impl Eyes {
    /// Take in the screen.
    pub fn look(&mut self, v: &View<'_>) {
        let cat = jane_data::catalog();
        let z = v.zone();
        let me = v.body().pos;
        let at = me.cell();
        let cam = camera(at);
        for p in v.props_in(cam) {
            let Some(s) = v.prop_spawn(p) else { continue };
            let d = cat.story.prop(p.def);
            if let Some(door) = s.to {
                self.doors.insert(door.zone);
            }
            let thing = s.label.is_some()
                || s.talk.is_some()
                || s.to.is_some()
                || !s.loot.is_empty()
                || s.use_list.is_some()
                || d.rest
                || d.bench;
            if !thing {
                continue;
            }
            let c = prop_centre(p).cell();
            self.props.insert((z, p.id), c);
            if crate::sense::to_prop(p, me) <= i64::from(CHECKED_NEAR * CELL_FX) {
                self.checked.insert((z, p.id));
            }
            if z == ZoneId::County
                && !self.read.contains(&p.id)
                && crate::sense::to_prop(p, me) <= i64::from(READ_NEAR * CELL_FX)
            {
                self.read_sign(v, p);
            }
        }
        for u in v.units_in(cam) {
            if u.unit.id != v.body().id {
                self.units.insert((z, u.unit.id), u.unit.pos.cell());
            }
        }
        self.looks += 1;
        // Blocks wholly on screen (a block half on it has ground past the edge not yet seen).
        let up = |a: i32| a.div_euclid(BLOCK) + i32::from(a.rem_euclid(BLOCK) != 0);
        let (bx0, by0) = (up(cam.x), up(cam.y));
        let (bx1, by1) = (cam.right().div_euclid(BLOCK) - 1, cam.bottom().div_euclid(BLOCK) - 1);
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                self.looked.insert((z, bx, by));
            }
        }
        if z != ZoneId::County {
            return;
        }
        // The roads that leave the screen toward ground never on screen.
        let mut edge = Vec::new();
        for x in cam.x..cam.right() {
            edge.push(((x, cam.y), (0, -1)));
            edge.push(((x, cam.bottom() - 1), (0, 1)));
        }
        for y in cam.y..cam.bottom() {
            edge.push(((cam.x, y), (-1, 0)));
            edge.push(((cam.right() - 1, y), (1, 0)));
        }
        for (c, (dx, dy)) in edge {
            if !walkable(v, c.0, c.1) || !road(v.tile(c.0, c.1)) {
                continue;
            }
            let past = block_of((c.0 + dx, c.1 + dy));
            if !self.looked.contains(&(z, past.0, past.1)) {
                self.frontier.entry(past).or_insert((c, self.looks));
            }
        }
        let looked = &self.looked;
        self.frontier.retain(|b, _| !looked.contains(&(z, b.0, b.1)));
    }

    /// Rebuilt from the journal alone (VERIFICATION.md L3, the Lost after a week away): every
    /// entry of this zone says where she stood when she learned it (a sign read, a book, a
    /// person met); what stands there with words or a verb is remembered again.
    pub fn recall(&mut self, v: &View<'_>) {
        let z = v.zone();
        let spots: Vec<(i32, i32)> =
            v.journal().filter(|e| e.zone == z).map(|e| (i32::from(e.at.x), i32::from(e.at.y))).collect();
        for (x, y) in spots {
            for p in v.props_in(jane_core::Rect::new(
                x - CHECKED_NEAR,
                y - CHECKED_NEAR,
                2 * CHECKED_NEAR + 1,
                2 * CHECKED_NEAR + 1,
            )) {
                let Some(s) = v.prop_spawn(p) else { continue };
                if s.label.is_some() || s.talk.is_some() || s.to.is_some() {
                    self.props.insert((z, p.id), prop_centre(p).cell());
                    if let Some(door) = s.to {
                        self.doors.insert(door.zone);
                    }
                }
            }
        }
    }

    /// Read a sign by the road: each place it names with a way and a distance becomes a lead.
    fn read_sign(&mut self, v: &View<'_>, p: &jane_sim::Prop) {
        let mut text = String::new();
        if let Some(l) = v.prop_spawn(p).and_then(|s| s.use_list) {
            crate::sense::visit(&|l| v.list(l), l, &mut |a| {
                if let jane_core::action::Action::Read(t) = *a {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(v.text(t));
                }
            });
        }
        self.read.insert(p.id);
        if text.is_empty() {
            return;
        }
        let at = prop_centre(p).cell();
        let (w0, h0) = v.size();
        let mut any = false;
        for (nth, w) in way_words(&text).into_iter().enumerate() {
            let heading = match w.heading {
                Some(h) => h,
                // A footpath's post: the path goes on from the road past the post.
                None if w.footpath => match from_road(v, at) {
                    Some(h) => h,
                    None => continue,
                },
                None => continue,
            };
            let (hx, hy) = (i64::from(heading.0), i64::from(heading.1));
            let len = i64::from(isqrt((hx * hx + hy * hy) as u64).max(1));
            let m = i64::from(w.metres);
            let est = (
                (i64::from(at.0) + hx * m / len).clamp(0, i64::from(w0) - 1) as i32,
                (i64::from(at.1) + hy * m / len).clamp(0, i64::from(h0) - 1) as i32,
            );
            let Some(to) =
                nearest_road(v, est, LEAD_ROAD).or_else(|| crate::nav::nearest_walkable(v, est.0, est.1, 20))
            else {
                continue;
            };
            let words = words(&w.name);
            if words.is_empty() {
                continue;
            }
            any = true;
            self.leads.push(Lead { post: p.id, nth: nth as u8, name: w.name, words, to, read: self.looks });
        }
        if any {
            self.reads += 1;
        }
    }

    /// The nearest fingerpost or signpost she has had on screen within [`POST_NEAR`] and not
    /// read (a junction's post is read before a road is chosen).
    pub fn unread_post(&self, v: &View<'_>) -> Option<(i32, i32)> {
        let cat = jane_data::catalog();
        let posts = [cat.story.prop_id("fingerpost"), cat.story.prop_id("signpost")];
        let at = v.body().pos.cell();
        let z = v.zone();
        self.props
            .iter()
            .filter(|&(&(pz, id), &c)| {
                pz == z
                    && !self.read.contains(&id)
                    && sq(c, at) <= i64::from(POST_NEAR * POST_NEAR)
                    && v.prop(id).is_some_and(|p| posts.contains(&Some(p.def)))
            })
            .min_by_key(|&(&(_, id), &c)| (sq(c, at), id))
            .map(|(_, &c)| c)
    }

    /// The lead whose whole name the step's words say (the longest name, then the newest read),
    /// not `spent`.
    pub fn lead(&self, want: &[String], spent: &impl Fn(&Lead) -> bool) -> Option<&Lead> {
        self.leads
            .iter()
            .filter(|l| names(&l.words, want) && !spent(l))
            .max_by_key(|l| (l.words.len(), l.read, std::cmp::Reverse((l.post, l.nth))))
    }

    /// Has the cell been on screen?
    pub fn looked_at(&self, z: ZoneId, cell: (i32, i32)) -> bool {
        let (bx, by) = block_of(cell);
        self.looked.contains(&(z, bx, by))
    }

    /// The newest road frontier cell (the nearest of those), not in `bad` nor where she died
    /// (`dangers`: a road that killed her is not the one she is looking for).
    pub fn road_frontier(
        &self,
        from: (i32, i32),
        bad: &BTreeSet<(i32, i32)>,
        dangers: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        self.frontier
            .values()
            .copied()
            .filter(|&(c, _)| !bad.contains(&block_of(c)) && !crate::nav::near_danger(dangers, c, SHUNNED))
            .min_by_key(|&(c, t)| (std::cmp::Reverse(t), sq(c, from), c.1, c.0))
            .map(|(c, _)| c)
    }
}

/// Does a place's name (its words) answer the step's words: every word of the name is among them
/// ("Hazel Spinney" is not "Poplar Spinney", nor "Lower Millbrook Farm" "Millbrook Farm")?
pub fn names(name: &[String], want: &[String]) -> bool {
    !name.is_empty() && score(want, name) == name.len()
}

/// One place a sign names: its name, which way (a heading) and how far (metres, which are
/// cells).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayWords {
    pub name: String,
    pub heading: Option<(i32, i32)>,
    pub metres: i32,
    /// On a footpath's post ("FOOTPATH. THE OLD MILL, 250 m."): the way is the path's.
    pub footpath: bool,
}

/// A wind as a sign writes it, as a heading in tenths.
fn wind(s: &str) -> Option<(i32, i32)> {
    Some(match s.trim() {
        "NORTH" => (0, -10),
        "SOUTH" => (0, 10),
        "EAST" => (10, 0),
        "WEST" => (-10, 0),
        "NORTH-EAST" => (7, -7),
        "NORTH-WEST" => (-7, -7),
        "SOUTH-EAST" => (7, 7),
        "SOUTH-WEST" => (-7, 7),
        _ => return None,
    })
}

/// A distance as a sign writes it ("850 m", "1.2 km"), in metres.
fn metres(s: &str) -> Option<i32> {
    let s = s.trim().trim_end_matches('.');
    let (num, per) = if let Some(n) = s.strip_suffix(" km") {
        (n, 1000)
    } else if let Some(n) = s.strip_suffix(" m") {
        (n, 1)
    } else {
        return None;
    };
    let (whole, tenth) = num.split_once('.').unwrap_or((num, "0"));
    let whole: i32 = whole.trim().parse().ok()?;
    let tenth: i32 = tenth.get(..1).unwrap_or("0").parse().ok()?;
    Some(whole * per + tenth * per / 10)
}

/// The places a sign's words name, with how far each is and, where it says, which way: a
/// roadside post ("THE GOLD MINE, NORTH-EAST, 850 m."), a fork's ("EAST: CASTLE, 1.2 km;
/// LOWFIELDS, 900 m. WEST: ..."), a footpath's ("FOOTPATH. THE OLD MILL, 250 m.").
pub fn way_words(text: &str) -> Vec<WayWords> {
    let mut out = Vec::new();
    let mut footpath = false;
    for sentence in text.split(". ") {
        let sentence = sentence.trim().trim_end_matches('.');
        if sentence == "FOOTPATH" {
            footpath = true;
            continue;
        }
        let (heading, body) = match sentence.split_once(": ") {
            Some((d, b)) if wind(d).is_some() => (wind(d), b),
            _ => (None, sentence),
        };
        for entry in body.split("; ") {
            let mut name: Vec<&str> = Vec::new();
            let (mut h, mut m) = (heading, None);
            for part in entry.split(", ") {
                if let Some(d) = wind(part) {
                    h = Some(d);
                } else if let Some(x) = metres(part) {
                    m = Some(x);
                } else {
                    name.push(part.trim());
                }
            }
            let Some(metres) = m else { continue };
            if name.is_empty() {
                continue;
            }
            out.push(WayWords { name: name.join(", "), heading: h, metres, footpath: footpath && h.is_none() });
        }
    }
    out
}

/// The way from the nearest road to a footpath's post, which the path goes on in: a heading.
fn from_road(v: &View<'_>, at: (i32, i32)) -> Option<(i32, i32)> {
    let r = nearest_road(v, at, 12)?;
    let (dx, dy) = (at.0 - r.0, at.1 - r.1);
    ((dx, dy) != (0, 0)).then_some((dx, dy))
}

/// The nearest road cell (a road or a street she can stand on) to `(x, y)` within `r` rings.
pub fn nearest_road(v: &View<'_>, (x, y): (i32, i32), r: i32) -> Option<(i32, i32)> {
    let ok = |cx: i32, cy: i32| walkable(v, cx, cy) && road(v.tile(cx, cy));
    if ok(x, y) {
        return Some((x, y));
    }
    for k in 1..=r {
        for d in -k..=k {
            for (cx, cy) in [(x + d, y - k), (x + d, y + k), (x - k, y + d), (x + k, y + d)] {
                if ok(cx, cy) {
                    return Some((cx, cy));
                }
            }
        }
    }
    None
}

fn sq(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(a.0 - b.0), i64::from(a.1 - b.1));
    dx * dx + dy * dy
}

/// The nearest ground the map has not charted: an unseen fog block beside a seen one whose
/// middle can be stood on, within `rings` blocks, not in `bad`. The walkable cell of the seen
/// block nearest the unseen one.
pub fn fog_frontier(
    v: &View<'_>,
    rings: i32,
    bad: &BTreeSet<(i32, i32)>,
    dangers: &[(i32, i32)],
) -> Option<(i32, i32)> {
    let n = v.fog_block() as i32;
    let (w, h) = v.size();
    let (bw, bh) = ((w as i32 + n - 1) / n, (h as i32 + n - 1) / n);
    let (x, y) = v.body().pos.cell();
    let (bx, by) = (x / n, y / n);
    let seen = |bx: i32, by: i32| bx >= 0 && by >= 0 && bx < bw && by < bh && v.seen(bx * n, by * n);
    for k in 1..=rings {
        let mut ring: Vec<(i32, i32)> = Vec::new();
        for d in -k..=k {
            ring.extend([(bx + d, by - k), (bx + d, by + k), (bx - k, by + d), (bx + k, by + d)]);
        }
        ring.sort_by_key(|&(cx, cy)| (sq((cx, cy), (bx, by)), cy, cx));
        ring.dedup();
        for (ux, uy) in ring {
            if ux < 0 || uy < 0 || ux >= bw || uy >= bh || seen(ux, uy) || bad.contains(&(ux, uy)) {
                continue;
            }
            for (sx, sy) in [(ux - 1, uy), (ux + 1, uy), (ux, uy - 1), (ux, uy + 1)] {
                if !seen(sx, sy) {
                    continue;
                }
                let mid = (sx * n + n / 2, sy * n + n / 2);
                if let Some(c) = crate::nav::nearest_walkable(v, mid.0, mid.1, n / 2) {
                    if !crate::nav::near_danger(dangers, c, SHUNNED) {
                        return Some(c);
                    }
                }
            }
        }
    }
    None
}

/// The fog block a cell is in (the Explorer's set-aside key).
pub fn fog_block_of(v: &View<'_>, (x, y): (i32, i32)) -> (i32, i32) {
    let n = v.fog_block() as i32;
    (x.div_euclid(n), y.div_euclid(n))
}

/// Words that carry no place.
pub const STOP: [&str; 74] = [
    "the", "a", "an", "at", "on", "in", "by", "of", "to", "from", "into", "up", "down", "near", "past", "behind",
    "under", "over", "off", "out", "and", "or", "with", "for", "after", "before", "her", "his", "its", "their", "your",
    "whoever", "any", "some", "made", "end", "edge", "side", "first", "next", "last", "same", "along", "across",
    "north", "south", "east", "west", "night", "day", "daylight", "bell", "nine", "clock", "road", "track", "path",
    "open", "stand", "knock", "tell", "find", "where", "there", "two", "three", "one", "round", "inside", "until",
    "you", "set", "it", "is",
];

/// The words of a text that could name a place or a thing: lower case, apostrophes and the rest
/// dropped, stop words gone.
pub fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
        let w = w.trim_matches('\'').to_lowercase();
        let w = w.strip_suffix("'s").map(str::to_owned).unwrap_or(w);
        if w.len() < 3 || STOP.contains(&w.as_str()) || w.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if !out.contains(&w) {
            out.push(w);
        }
    }
    out
}

/// `{place:<story>}` as this seed names it; `{name}` as her name.
pub fn expand(v: &View<'_>, s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let key = &after[..close];
        if key == "name" {
            out.push_str(v.heroine());
        } else if let Some(story) = key.strip_prefix("place:") {
            out.push_str(&story_name(v.seed(), story));
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// A story's place name on a seed (`jane_world::names::story_name`, by the story's key).
pub fn story_name(seed: u32, key: &str) -> String {
    let cat = jane_data::catalog();
    cat.county
        .stories
        .iter()
        .find(|d| d.key == key)
        .map_or_else(|| key.replace('_', " "), |d| jane_world::names::story_name(seed, d.id))
}

/// What the log says of a step (its text, then the quest's description), expanded; `None` for
/// the hand-in (the log's "Back to ...").
pub fn step_text(v: &View<'_>, q: QuestId, step: Option<usize>) -> String {
    let cat = jane_data::catalog();
    let def = cat.story.quest(q);
    match step {
        Some(i) => expand(v, cat.text(def.requirements[i].text)),
        None => expand(v, cat.text(def.return_to)),
    }
}

/// The words a step's text names a place by: the step's own words, less the thing sought's.
pub fn landmark_words(v: &View<'_>, q: QuestId, step: Option<usize>) -> Vec<String> {
    let cat = jane_data::catalog();
    let def = cat.story.quest(q);
    let mut sought: Vec<String> = Vec::new();
    if let Some(i) = step {
        if let jane_data::ReqTarget::Acquire(item) = def.requirements[i].target {
            sought = words(cat.text(cat.combat.item(item).name));
        }
    }
    words(&step_text(v, q, step)).into_iter().filter(|w| !sought.contains(w)).collect()
}

/// A prop's words: its label, its kind's name, and its key's parts.
pub fn prop_words(v: &View<'_>, p: &jane_sim::Prop) -> Vec<String> {
    let cat = jane_data::catalog();
    let mut s = cat.text(cat.story.prop(p.def).name).to_owned();
    if let Some(l) = v.prop_spawn(p).and_then(|s| s.label) {
        s.push(' ');
        s.push_str(v.text(l));
    }
    words(&s)
}

/// How many of `want` a prop's words share (a word counts when one starts the other: "carter"
/// and "carters", "scarecrow" and "scarecrows").
pub fn score(have: &[String], want: &[String]) -> usize {
    want.iter().filter(|w| have.iter().any(|h| h.starts_with(w.as_str()) || w.starts_with(h.as_str()))).count()
}

/// The landmark she has seen that the words best name, and has not stood by: its cell.
pub fn landmark(v: &View<'_>, eyes: &Eyes, want: &[String]) -> Option<(i32, i32)> {
    if want.is_empty() {
        return None;
    }
    let z = v.zone();
    let at = v.body().pos;
    let mut best: Option<(usize, i64, PropId, (i32, i32))> = None;
    for (&(pz, id), &c) in &eyes.props {
        if pz != z || eyes.checked.contains(&(z, id)) {
            continue;
        }
        let Some(p) = v.prop(id) else { continue };
        let s = score(&prop_words(v, p), want);
        if s == 0 {
            continue;
        }
        let d = dist(at, Vec2::centre(c.0, c.1));
        let better =
            best.is_none_or(|(bs, bd, bid, _)| (std::cmp::Reverse(s), d, id) < (std::cmp::Reverse(bs), bd, bid));
        if better {
            best = Some((s, d, id, c));
        }
    }
    best.map(|b| b.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_says_which_way_and_how_far() {
        let w = way_words("THE GOLD MINE, NORTH-EAST, 1.2 km.");
        assert_eq!(
            w,
            vec![WayWords { name: "THE GOLD MINE".into(), heading: Some((7, -7)), metres: 1200, footpath: false }]
        );
        let w = way_words("EAST: CASTLE, 1.2 km; LOWFIELDS, 900 m. WEST: THE HALT, 2.0 km.");
        let got: Vec<_> = w.iter().map(|w| (w.name.as_str(), w.heading, w.metres)).collect();
        assert_eq!(
            got,
            vec![
                ("CASTLE", Some((10, 0)), 1200),
                ("LOWFIELDS", Some((10, 0)), 900),
                ("THE HALT", Some((-10, 0)), 2000)
            ]
        );
        let w = way_words("FOOTPATH. CARTER'S FARM, 250 m.");
        assert_eq!(w, vec![WayWords { name: "CARTER'S FARM".into(), heading: None, metres: 250, footpath: true }]);
        assert!(way_words("CASTLE 3.4 km").is_empty());
    }

    #[test]
    fn a_name_answers_the_words_whole() {
        let want = words("Her apples, to the woodpile at Poplar Spinney");
        assert!(names(&words("POPLAR SPINNEY"), &want));
        assert!(!names(&words("HAZEL SPINNEY"), &want));
        assert!(!names(&words("LOWER POPLAR SPINNEY"), &want));
        assert!(names(&words("the Gold Mine"), &words("The Gold Mine, at the end of the mine road")));
    }
}
