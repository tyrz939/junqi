//! What a player without markers can know, and how she looks for the rest (VERIFICATION.md §2
//! L3, the **Lost** and the **Explorer**).
//!
//! The Reader resolves a quest step by the catalog's name for it over the whole zone. The Lost
//! may use a thing only once it has been **on screen** (the camera's 48 × 27 cells about her,
//! `jane_sim::trace::camera`): a prop, a person or a creature she has had in view, a door she has
//! seen, ground she has looked over. Until then she looks for it from the words alone:
//!
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

use jane_core::num::CELL_FX;
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
const STOP: [&str; 74] = [
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
