//! Truth and cohesion over the built world (VERIFICATION.md §2 L5), per seed: every quest step's
//! words held against where its thing stands, and every omen's claim against the county.
//!
//! This is an audit, not a player: it reads the blueprints (as the console's setup does), never
//! a View. For each step (and each quest's "Back to ..."):
//!
//! - **where it is**: every instance of what completes it (a thing to hold on the ground or in a
//!   chest, a trigger's rect or a thing that marks the place, units to put down, whoever takes it
//!   back), in the county or in the dungeon it stands in;
//! - **what the words name**: each phrase after a preposition ("at the well", "on the station
//!   road", "in the allotments") is looked for among what the county built: a thing's name or
//!   label, a story place's name, a patch's name, a door's zone. A way ("... road", "... lane",
//!   "... track") wants a road or path under the thing; a landmark wants to stand within the
//!   preposition's reach of it ([`reach`]: "by the well" means within [`BESIDE`] cells);
//! - **the verdict**: `ok`; `far` (the landmark is built, but not near the thing); `unbuilt`
//!   (nothing in the county is called that: the 2026 web build's bug, an anchor a quest names
//!   that was never built); `off-road` (the thing is more than a half-screen from any road or
//!   path, and so is the nearest landmark the words name: `QUEST-TREE.md` C1); `nothing` (no
//!   instance of the thing on this seed).
//!
//! Omens (STORY.md §6, PLAN.md §5): which came true on the seed, where each claim is posted or
//! said, and the lethal rule (never two of a region).

use std::collections::BTreeMap;

use jane_core::action::Action;
use jane_core::blueprint::{Blueprint, StoryPlace};
use jane_core::{ListRef, QuestId, TextRef, Tile, ZoneId};
use jane_data::ReqTarget;
use jane_sim::Blueprints;

/// Cells "by", "at", "beside" may be from what they name (QUEST-TREE.md: the truth test's 3,
/// and a prop's own size and the placement's spread).
pub const BESIDE: i32 = 10;
/// Cells "in", "on", "inside" may be (a patch, a garden, a platform).
pub const WITHIN: i32 = 40;
/// Cells any other preposition may be ("near", "past", "from", "behind"): two half-screens,
/// QUEST-TREE.md A3's `POSTED_NEAR`.
pub const NEAR: i32 = 48;
/// Cells from the thing a way it is "on" may be.
pub const ON_WAY: i32 = 12;
/// A half-screen: the thing is "seen from a road" within this (QUEST-TREE.md C1 `ON_SCREEN`).
pub const HALF_SCREEN: i32 = jane_core::view::HALF_W_CELLS as i32;

const PREPS: [&str; 26] = [
    "by", "at", "beside", "against", "under", "facing", "round", "near", "past", "from", "behind", "off", "up",
    "along", "below", "across", "to", "of", "into", "in", "inside", "on", "through", "over", "down", "outside",
];

const ARTICLES: [&str; 9] = ["the", "a", "an", "her", "his", "its", "their", "your", "whoever"];

const WAYS: [&str; 8] = ["road", "lane", "street", "track", "path", "footpath", "steps", "prints"];

/// How far a preposition lets a landmark be from the thing.
pub fn reach(prep: &str) -> i32 {
    match prep {
        "by" | "at" | "beside" | "against" | "under" | "facing" | "round" => BESIDE,
        "in" | "inside" | "on" | "into" | "over" | "outside" => WITHIN,
        "from" | "past" | "up" | "along" | "across" | "through" | "down" => ROUTE,
        _ => NEAR,
    }
}

/// Cells a place a route is walked "from", "past" or "up" may be: a way leads from it, it is
/// not beside it.
pub const ROUTE: i32 = 150;

/// What a named place (a story's, a patch, a site) adds to a preposition's reach: it has an
/// edge, and "by Sundial Cottage" is by its garden wall, not its door.
pub const PLACE_EDGE: i32 = 16;

/// A phrase the words name a place by, and what the county holds of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phrase {
    pub prep: String,
    pub words: Vec<String>,
    /// A way (a road, a lane, a track), not a landmark.
    pub way: bool,
    /// The words of a place's own name in it (written with a capital), if any.
    pub proper: Vec<String>,
    /// The nearest thing so named to the step's thing: cells, and what it is.
    pub nearest: Option<(i32, String)>,
    /// Anything in the county so named at all.
    pub built: bool,
    pub ok: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Ok,
    /// In a dungeon or a house, named by the words (or not).
    Elsewhere {
        zone: &'static str,
        named: bool,
    },
    /// A landmark named is built, but not within reach of the thing.
    Far(String),
    /// Nothing in the county is called what the words name.
    Unbuilt(String),
    /// More than a half-screen from any road or path, and so is the named landmark.
    OffRoad(i32),
    /// No instance on this seed.
    Nothing,
}

impl Verdict {
    pub fn word(&self) -> String {
        match self {
            Verdict::Ok => "ok".into(),
            Verdict::Elsewhere { zone, named: true } => format!("in {zone}"),
            Verdict::Elsewhere { zone, named: false } => format!("in {zone}, UNNAMED"),
            Verdict::Far(w) => format!("FAR: {w}"),
            Verdict::Unbuilt(w) => format!("UNBUILT: {w}"),
            Verdict::OffRoad(d) => format!("OFF-ROAD: {d} cells"),
            Verdict::Nothing => "NOTHING".into(),
        }
    }

    pub fn bad(&self) -> bool {
        !matches!(self, Verdict::Ok | Verdict::Elsewhere { named: true, .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepAudit {
    pub quest: &'static str,
    /// 255: the hand-in.
    pub step: u8,
    pub text: String,
    pub kind: &'static str,
    /// Where the instance judged stands (the one nearest what the words name).
    pub zone: Option<ZoneId>,
    pub at: Option<(i32, i32)>,
    pub instances: u32,
    /// Cells from the instance to the nearest road or path (the county).
    pub to_road: Option<i32>,
    pub phrases: Vec<Phrase>,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmenAudit {
    pub id: &'static str,
    pub region: &'static str,
    pub lethal: bool,
    pub true_here: bool,
    /// Where its claim is posted or said: (zone, cell, what says it).
    pub posted: Vec<(ZoneId, (i32, i32), String)>,
}

/// A thing the county built with a name.
struct Named {
    /// Called this by name (a label, a story place, a patch, a rect), not only its kind.
    proper: bool,
    /// A place with ground (a story's, a patch, a rect), not a thing.
    place: bool,
    rect: jane_core::Rect,
    words: Vec<String>,
    what: String,
}

fn text_of(bp: &Blueprint, r: TextRef) -> String {
    match r {
        TextRef::Text(t) => jane_data::catalog().text(t).to_owned(),
        TextRef::Local(_) => bp.text(r).unwrap_or("").to_owned(),
    }
}

fn lists<'a>(bp: &'a Blueprint) -> impl Fn(ListRef) -> &'a [Action] {
    move |r| match r {
        ListRef::Catalog(_) => jane_data::catalog().list(r),
        ListRef::Blueprint(_) => bp.list(r).unwrap_or(&[]),
    }
}

/// Does a prop spawn's use list or conversation do `pred`?
fn spawn_does(bp: &Blueprint, p: &jane_core::blueprint::PropSpawn, pred: &impl Fn(&Action) -> bool) -> bool {
    let mut hit = false;
    let l = lists(bp);
    if let Some(u) = p.use_list {
        crate::sense::visit(&l, u, &mut |a| hit |= pred(a));
    }
    if let Some(t) = p.talk {
        crate::sense::visit_tree(&l, t, &mut |a| hit |= pred(a));
    }
    hit
}

fn prop_rect(p: &jane_core::blueprint::PropSpawn) -> jane_core::Rect {
    let d = jane_data::catalog().story.prop(p.def);
    jane_core::Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
}

impl Named {
    /// What it is, and how far from `at` its edge is.
    fn to_rect_what(&self, at: (i32, i32)) -> String {
        format!("{} ({} cells to its edge)", self.what, to_rect(at, self.rect))
    }
}

/// Cells from a point to a rect (0 inside), Chebyshev.
fn to_rect(c: (i32, i32), r: jane_core::Rect) -> i32 {
    let dx = (r.x - c.0).max(c.0 - (r.right() - 1)).max(0);
    let dy = (r.y - c.1).max(c.1 - (r.bottom() - 1)).max(0);
    dx.max(dy)
}

fn way_tile(t: Tile) -> bool {
    matches!(t, Tile::Road | Tile::Cobble | Tile::Dirt | Tile::GrownPath | Tile::Stepping)
}

/// Cells from `c` to the nearest road or path, searched out to `max`.
fn to_way(bp: &Blueprint, c: (i32, i32), max: i32) -> Option<i32> {
    to_tile(bp, c, max, way_tile)
}

/// Cells from `c` to the nearest tile that is `what`, searched out to `max`.
fn to_tile(bp: &Blueprint, c: (i32, i32), max: i32, what: impl Fn(Tile) -> bool) -> Option<i32> {
    for k in 0..=max {
        for d in -k..=k {
            for (x, y) in [(c.0 + d, c.1 - k), (c.0 + d, c.1 + k), (c.0 - k, c.1 + d), (c.0 + k, c.1 + d)] {
                if bp.tiles.get(x, y).is_some_and(|&t| what(t)) {
                    return Some(k);
                }
            }
        }
    }
    None
}

/// Words for trees standing together.
const TREES: [&str; 5] = ["wood", "woods", "copse", "trees", "spinney"];

/// Words of time, not place ("from six till the bell").
const TIMES: [&str; 14] = [
    "six", "four", "noon", "till", "bell", "nine", "clock", "morning", "evening", "dark", "daylight", "dusk", "sunday",
    "tuesday",
];

/// The phrases of a text: after each preposition, the words up to the next preposition or mark,
/// and those of them written with a capital (a place's own name: "Castle", "Julie's").
pub fn phrases(text: &str) -> Vec<(String, Vec<String>, Vec<String>)> {
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        // "No. 12" keeps its full stop.
        let number_stop = ch == '.' && (cur == "No" || cur == "no");
        if ch.is_alphanumeric() || ch == '\'' || number_stop {
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            if matches!(ch, ',' | '.' | ';' | ':') {
                toks.push(",".into());
            }
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].to_lowercase();
        if PREPS.contains(&t.as_str()) {
            let mut words = Vec::new();
            let mut proper: Vec<String> = Vec::new();
            let mut j = i + 1;
            while j < toks.len() {
                let w = toks[j].to_lowercase();
                if w == "," || PREPS.contains(&w.as_str()) {
                    break;
                }
                if !ARTICLES.contains(&w.as_str()) {
                    let ws = if WAYS.contains(&w.as_str()) { vec![w.clone()] } else { crate::lost::words(&w) };
                    if toks[j].chars().next().is_some_and(char::is_uppercase) {
                        proper.extend(ws.iter().cloned());
                    }
                    words.extend(ws);
                }
                j += 1;
            }
            words.retain(|w| !TIMES.contains(&w.as_str()));
            proper.retain(|w| !TIMES.contains(&w.as_str()));
            if !words.is_empty() {
                out.push((t, words, proper));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Every named thing in a zone: props (a name or a label), story places, patches, doors.
fn named(bp: &Blueprint, seed: u32) -> Vec<Named> {
    let cat = jane_data::catalog();
    let mut out = Vec::new();
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        let mut s = cat.text(d.name).to_owned();
        let mut proper = false;
        if let Some(l) = p.label {
            s.push(' ');
            s.push_str(&text_of(bp, l));
            proper = true;
        }
        if let Some(door) = p.to {
            s.push(' ');
            s.push_str(door.zone.name());
            proper = true;
        }
        if let Some(t) = p.talk {
            let words = posted(t);
            if !crate::lost::words(&words).is_empty() {
                s.push(' ');
                s.push_str(&words);
                proper = true;
            }
        }
        let words = crate::lost::words(&s);
        if !words.is_empty() {
            out.push(Named { proper, place: false, rect: prop_rect(p), words, what: s });
        }
    }
    for (&id, place) in &bp.stories {
        if let StoryPlace::Placed { bounds, .. } = place {
            let name = jane_world::names::story_name(seed, id);
            out.push(Named { proper: true, place: true, rect: *bounds, words: crate::lost::words(&name), what: name });
        }
    }
    for a in &bp.areas {
        let name = match a.name {
            jane_core::Key::Name(n) => cat.name(n).to_owned(),
            jane_core::Key::Local(i) => bp.local_names.get(i as usize).cloned().unwrap_or_default(),
        };
        out.push(named_key(&name, a.rect));
    }
    // A site's ground, by the site's name (the station, the farm, the town: their buildings
    // stand there). Not the other rects nor the marks: they are the layout's own names, which
    // nobody sees; an anchor a quest names is built only when something stands at it (the web
    // build's bug).
    for (k, r) in &bp.rects {
        let name = match *k {
            jane_core::Key::Name(n) => cat.name(n).to_owned(),
            jane_core::Key::Local(i) => bp.local_names.get(i as usize).cloned().unwrap_or_default(),
        };
        if name.starts_with("site_") {
            out.push(named_key(&name, *r));
        }
    }
    out
}

/// What a thing that talks posts: its speaker's name ("Lost Property"), and every run of
/// capitals in its lines, as a sign is lettered ("CASTLE HALT", "PUBLIC FOOTPATH No. 3. THE LONG
/// HEDGE").
fn posted(tree: jane_core::DialogueId) -> String {
    let cat = jane_data::catalog();
    let t = cat.story.dialogue(tree);
    let mut out = cat.text(t.speaker).to_owned();
    for n in t.nodes {
        for l in n.lines {
            let mut run: Vec<&str> = Vec::new();
            for w in cat.text(l.text).split_whitespace().chain(std::iter::once("")) {
                let letters: String = w.chars().filter(|c| c.is_alphabetic()).collect();
                if letters.len() >= 2 && letters.chars().all(char::is_uppercase) {
                    run.push(w);
                } else {
                    if !run.is_empty() {
                        out.push(' ');
                        out.push_str(&run.join(" "));
                    }
                    run.clear();
                }
            }
        }
    }
    out
}

/// A patch by its key ("site_town", "allotments"): its words, and a site's own name when the
/// key is a site's ("Castle" for the town).
fn named_key(key: &str, rect: jane_core::Rect) -> Named {
    let cat = jane_data::catalog();
    let mut what = key.replace('_', " ");
    let site = key.strip_prefix("site_").unwrap_or(key);
    if let Some(sd) = cat.county.sites.iter().find(|d| d.id == site || key.starts_with(&format!("{}_", d.id))) {
        what.push_str(" / ");
        what.push_str(cat.text(sd.name));
    }
    Named { proper: true, place: rect.w > 1 || rect.h > 1, rect, words: crate::lost::words(&what), what }
}

/// Does a named thing answer a phrase? A place's own name is answered by a thing called that
/// (every capital word of it: "Castle" in "Castle square"); a kind ("the well") by anything of
/// the kind or so labelled (the phrase's last word, or two of its words).
fn answers(n: &Named, p: &Phrase) -> bool {
    let has =
        |w: &String| n.words.iter().any(|h| h.starts_with(w.as_str()) || w.starts_with(h.as_str()) && h.len() >= 4);
    if !p.proper.is_empty() {
        return n.proper && p.proper.iter().all(has);
    }
    let head = p.words.last().is_some_and(has);
    head || p.words.iter().filter(|w| has(w)).count() >= 2
}

/// The words every kind of thing the county can build is called by (a prop row's name): a
/// phrase naming a kind outside them ("the yard", "the rim") names nothing a check can find, and
/// is not held to one.
fn kind_words() -> &'static Vec<String> {
    static W: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    W.get_or_init(|| {
        let cat = jane_data::catalog();
        let mut v: Vec<String> = cat.story.props.iter().flat_map(|d| crate::lost::words(cat.text(d.name))).collect();
        v.sort();
        v.dedup();
        v
    })
}

/// Where each instance of a step's thing stands: (zone, rect).
fn instances(bps: &Blueprints, q: QuestId, step: u8) -> Vec<(ZoneId, jane_core::Rect)> {
    let cat = jane_data::catalog();
    let def = cat.story.quest(q);
    let mut out = Vec::new();
    for &z in &ZoneId::ALL {
        let bp = bps.get(z);
        let rect_of = |k: jane_core::Key| bp.rects.get(&k).copied();
        match (step, def.requirements.get(usize::from(step)).map(|r| r.target)) {
            (255, _) => {
                let does = |a: &Action| matches!(a, Action::HandIn(x) if *x == q);
                push_places(bp, &does, &mut out);
                for u in &bp.units {
                    let d = cat.combat.unit(u.def);
                    if d.talk.is_some_and(|t| {
                        let mut hit = false;
                        crate::sense::visit_tree(&lists(bp), t, &mut |a| hit |= does(a));
                        hit
                    }) {
                        out.push((z, jane_core::Rect::new(i32::from(u.cell.x), i32::from(u.cell.y), 1, 1)));
                        // Someone who keeps hours is also where the hours put them.
                        for row in d.schedule {
                            if let jane_data::ScheduleSlot::Mark(n) = row.slot {
                                if let Some(m) = bp.marks.get(&jane_core::Key::Name(n)) {
                                    out.push((z, jane_core::Rect::new(i32::from(m.cell.x), i32::from(m.cell.y), 1, 1)));
                                }
                            }
                        }
                    }
                }
            }
            (_, Some(ReqTarget::Location(name))) => {
                let does = |a: &Action| matches!(a, Action::Location(jane_core::Key::Name(n)) if *n == name);
                push_places(bp, &does, &mut out);
                push_units(bp, &does, &mut out);
                let _ = rect_of;
            }
            (_, Some(ReqTarget::Kill(d))) => {
                for u in bp.units.iter().filter(|u| u.def == d) {
                    out.push((z, jane_core::Rect::new(i32::from(u.cell.x), i32::from(u.cell.y), 1, 1)));
                }
            }
            (_, Some(ReqTarget::Acquire(item))) => {
                for p in bp.props.iter().filter(|p| p.loot.iter().any(|l| l.item == item)) {
                    out.push((z, prop_rect(p)));
                }
                for u in bp.units.iter().filter(|u| cat.combat.unit(u.def).loot.iter().any(|l| l.item == item)) {
                    out.push((z, jane_core::Rect::new(i32::from(u.cell.x), i32::from(u.cell.y), 1, 1)));
                }
                let gives = |a: &Action| matches!(a, Action::Give(st) if st.item == item);
                push_units(bp, &gives, &mut out);
                for p in &bp.props {
                    if spawn_does(bp, p, &gives) {
                        out.push((z, prop_rect(p)));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Units of a zone whose conversation or death does `pred`.
fn push_units(bp: &Blueprint, pred: &impl Fn(&Action) -> bool, out: &mut Vec<(ZoneId, jane_core::Rect)>) {
    let cat = jane_data::catalog();
    let l = lists(bp);
    for u in &bp.units {
        let d = cat.combat.unit(u.def);
        let mut hit = false;
        if let Some(t) = d.talk {
            crate::sense::visit_tree(&l, t, &mut |a| hit |= pred(a));
        }
        if let Some(dl) = d.on_death {
            crate::sense::visit(&l, dl, &mut |a| hit |= pred(a));
        }
        if hit {
            out.push((bp.zone, jane_core::Rect::new(i32::from(u.cell.x), i32::from(u.cell.y), 1, 1)));
        }
    }
}

/// Trigger rects and props of a zone that do `pred`.
fn push_places(bp: &Blueprint, pred: &impl Fn(&Action) -> bool, out: &mut Vec<(ZoneId, jane_core::Rect)>) {
    let cat = jane_data::catalog();
    let l = lists(bp);
    let mut rects: Vec<jane_core::Key> = Vec::new();
    for (_, t) in cat.story.triggers_in(bp.zone) {
        let mut hit = false;
        crate::sense::visit(&l, t.trigger.actions, &mut |a| hit |= pred(a));
        if hit {
            rects.push(t.trigger.rect);
        }
    }
    for t in bp.triggers.values() {
        let mut hit = false;
        crate::sense::visit(&l, t.actions, &mut |a| hit |= pred(a));
        if hit {
            rects.push(t.rect);
        }
    }
    for k in rects {
        if let Some(&r) = bp.rects.get(&k) {
            out.push((bp.zone, r));
        }
    }
    for p in &bp.props {
        if spawn_does(bp, p, pred) {
            out.push((bp.zone, prop_rect(p)));
        }
    }
}

/// Every quest step, and every hand-in, on a seed.
pub fn steps(bps: &Blueprints) -> Vec<StepAudit> {
    let cat = jane_data::catalog();
    let seed = bps.seed();
    let county = bps.get(ZoneId::County);
    let names = named(county, seed);
    let mut out = Vec::new();
    for (qi, def) in cat.story.quests.iter().enumerate() {
        let q = QuestId(qi as u16);
        let n = def.requirements.len();
        for step in (0..n as u8).chain(std::iter::once(255)) {
            let (text, kind, sought) = if step == 255 {
                (cat.text(def.return_to).to_owned(), "hand-in", Vec::new())
            } else {
                let r = &def.requirements[usize::from(step)];
                let sought = match r.target {
                    ReqTarget::Acquire(i) => crate::lost::words(cat.text(cat.combat.item(i).name)),
                    ReqTarget::Kill(d) => crate::lost::words(cat.text(cat.combat.unit(d).name)),
                    ReqTarget::Location(_) => Vec::new(),
                };
                let k = match r.target {
                    ReqTarget::Acquire(_) => "acquire",
                    ReqTarget::Kill(_) => "kill",
                    ReqTarget::Location(_) => "location",
                };
                (cat.text(r.text).to_owned(), k, sought)
            };
            let text = expand(&text, seed);
            let mut ph: Vec<Phrase> = phrases(&text)
                .into_iter()
                .map(|(prep, words, proper)| {
                    let words: Vec<String> = words.into_iter().filter(|w| !sought.contains(w)).collect();
                    let proper: Vec<String> = proper.into_iter().filter(|w| words.contains(w)).collect();
                    let way = words.last().is_some_and(|w| WAYS.contains(&w.as_str()));
                    Phrase { prep, words, way, proper, nearest: None, built: false, ok: false }
                })
                .filter(|p| !p.words.is_empty())
                .collect();
            // A thing to fetch "to the door ..." names where it goes, not where it is: from the
            // "to" on, the words are the delivery's.
            if kind == "acquire" {
                if let Some(i) = ph.iter().position(|p| p.prep == "to") {
                    ph.truncate(i);
                }
            }
            let mut inst = instances(bps, q, step);
            // The zone the words name (the cellar's rats, not the county's) is where it is judged.
            if let Some(z) = crate::story::zone_in_text(&text) {
                if inst.iter().any(|(iz, _)| *iz == z) {
                    inst.retain(|(iz, _)| *iz == z);
                }
            }
            let in_county: Vec<jane_core::Rect> =
                inst.iter().filter(|(z, _)| *z == ZoneId::County).map(|&(_, r)| r).collect();
            // Made at a bench: nowhere to find, and so nowhere to be wrong about.
            let made = step != 255
                && matches!(def.requirements[usize::from(step)].target,
                    ReqTarget::Acquire(i) if cat.combat.recipes.iter().any(|r| r.output == i));
            let mut a = StepAudit {
                quest: def.id,
                step,
                text: text.clone(),
                kind,
                zone: None,
                at: None,
                instances: inst.len() as u32,
                to_road: None,
                phrases: Vec::new(),
                verdict: Verdict::Nothing,
            };
            if made {
                a.verdict = Verdict::Ok;
                a.kind = "made";
                a.phrases = ph;
                out.push(a);
                continue;
            }
            if in_county.is_empty() {
                if let Some(&(z, r)) = inst.first() {
                    a.zone = Some(z);
                    a.at = Some(r.centre());
                    let named_zone = crate::story::zone_in_text(&text).is_some()
                        || crate::story::zone_in_text(&expand(cat.text(def.description), seed)).is_some()
                        || matches!(z, ZoneId::House | ZoneId::Arms | ZoneId::Church);
                    a.verdict = Verdict::Elsewhere { zone: z.name(), named: named_zone };
                }
                a.phrases = ph;
                out.push(a);
                continue;
            }
            // The instance judged: the one nearest what the landmark phrases name.
            let score = |r: jane_core::Rect| -> i32 {
                ph.iter()
                    .filter(|p| !p.way)
                    .map(|p| {
                        names
                            .iter()
                            .filter(|n| answers(n, p))
                            .map(|n| to_rect(r.centre(), n.rect))
                            .min()
                            .unwrap_or(1000)
                    })
                    .sum()
            };
            let best = *in_county.iter().min_by_key(|r| (score(**r), r.y, r.x)).expect("an instance");
            let at = best.centre();
            a.zone = Some(ZoneId::County);
            a.at = Some(at);
            a.to_road = to_way(county, at, 80);
            for p in &mut ph {
                // A wood is its trees, not a thing called "wood".
                if p.proper.is_empty() && p.words.last().is_some_and(|w| TREES.contains(&w.as_str())) {
                    let d = to_tile(county, at, WITHIN, |t| t == Tile::Tree);
                    p.built = true;
                    p.ok = d.is_some();
                    p.nearest = d.map(|d| (d, "trees".to_owned()));
                    continue;
                }
                if p.way {
                    let d = to_way(county, at, ON_WAY);
                    p.built = true;
                    p.ok = d.is_some();
                    p.nearest = d.map(|d| (d, "a road or path".to_owned()));
                    continue;
                }
                // A name is answered by something called that; a kind ("the well") by any of it.
                // Held against its reach, a named place's edge allowed for.
                let near = names
                    .iter()
                    .filter(|n| answers(n, p))
                    .map(|n| (to_rect(at, n.rect) - if n.place { PLACE_EDGE } else { 0 }, n.to_rect_what(at)))
                    .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
                    .map(|(d, w)| (d.max(0), w));
                p.built = near.is_some();
                p.ok = near.as_ref().is_some_and(|(d, _)| *d <= reach(&p.prep));
                p.nearest = near;
            }
            // A kind the words name ("the garden", "the yard", "the walls") is a part of the named
            // place beside it ("at Catkin Cottage"): it holds when that place holds.
            let a_name_holds = ph.iter().any(|p| !p.proper.is_empty() && !p.way && p.ok);
            let mut verdict = Verdict::Ok;
            for p in &ph {
                let a_kind = p.proper.is_empty();
                let checkable =
                    !a_kind || p.built || p.words.last().is_some_and(|w| kind_words().iter().any(|k| k == w));
                if p.way || p.ok || !checkable || (a_kind && a_name_holds) {
                    continue;
                }
                let words = p.words.join(" ");
                let v = match &p.nearest {
                    None => Verdict::Unbuilt(format!("{} {words}", p.prep)),
                    Some((d, what)) => Verdict::Far(format!("{} {words}: nearest {what} at {d} cells", p.prep)),
                };
                if verdict == Verdict::Ok || v > verdict {
                    verdict = v;
                }
            }
            // Seen from a road: the thing, or a landmark the words name that stands by it.
            let landmark_on_road = ph.iter().filter(|p| !p.way && p.ok).any(|p| {
                names
                    .iter()
                    .filter(|n| answers(n, p) && to_rect(at, n.rect) <= reach(&p.prep))
                    .any(|n| to_way(county, n.rect.centre(), HALF_SCREEN).is_some())
            });
            let road = a.to_road.unwrap_or(999);
            if verdict == Verdict::Ok && road > HALF_SCREEN && !landmark_on_road {
                verdict = Verdict::OffRoad(road);
            }
            a.phrases = ph;
            a.verdict = verdict;
            out.push(a);
        }
    }
    out
}

/// `{place:<story>}` as the seed names it.
pub fn expand(s: &str, seed: u32) -> String {
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
        match key.strip_prefix("place:") {
            Some(story) => out.push_str(&crate::lost::story_name(seed, story)),
            None if key == "name" => out.push_str("Jane"),
            None => out.push_str(key),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Every omen on a seed: true or not (the world's flags after New Game), and where its claim is
/// posted or said.
pub fn omens(bps: &Blueprints) -> Vec<OmenAudit> {
    let cat = jane_data::catalog();
    let sim = jane_sim::Sim::new_game_with(bps.clone(), "Jane");
    let mut out = Vec::new();
    for o in cat.story.omens {
        let claim = key_sentence(cat.text(o.claim));
        let mut posted = Vec::new();
        for &z in &ZoneId::ALL {
            let bp = bps.get(z);
            for p in &bp.props {
                let mut says = p.label.is_some_and(|l| plain(&text_of(bp, l)).contains(&claim));
                if let Some(t) = p.talk {
                    says |= cat
                        .story
                        .dialogue(t)
                        .nodes
                        .iter()
                        .any(|n| n.lines.iter().any(|l| plain(cat.text(l.text)).contains(&claim)));
                }
                if says {
                    let d = cat.story.prop(p.def);
                    posted.push((z, (i32::from(p.cell.x), i32::from(p.cell.y)), cat.text(d.name).to_owned()));
                }
            }
            for u in &bp.units {
                if let Some(t) = cat.combat.unit(u.def).talk {
                    if cat
                        .story
                        .dialogue(t)
                        .nodes
                        .iter()
                        .any(|n| n.lines.iter().any(|l| plain(cat.text(l.text)).contains(&claim)))
                    {
                        posted.push((
                            z,
                            (i32::from(u.cell.x), i32::from(u.cell.y)),
                            cat.combat.unit(u.def).id.to_owned(),
                        ));
                    }
                }
            }
        }
        out.push(OmenAudit {
            id: o.id,
            region: match o.region {
                jane_data::Region::Lowfields => "Lowfields",
                jane_data::Region::Waters => "Waters",
                jane_data::Region::Works => "Works",
            },
            lethal: o.lethal,
            true_here: jane_sim::omens::is_true(sim.state(), o.id),
            posted,
        });
    }
    out
}

/// Lower case letters and single spaces.
fn plain(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A claim's last sentence, plain: the uncanny part a sign or a person must say ("does not
/// always stop"), however the row round it is worded.
fn key_sentence(claim: &str) -> String {
    let last = claim.trim_end_matches('.').rsplit(". ").next().unwrap_or(claim);
    let last = last.rsplit(": ").next().unwrap_or(last);
    plain(last)
}

/// The omen rules on one seed's audit: no two lethal of a region true together; every claim
/// posted or said somewhere. Each broken rule, in words.
pub fn omen_problems(os: &[OmenAudit]) -> Vec<String> {
    let mut out = Vec::new();
    let mut lethal: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for o in os {
        if o.lethal && o.true_here {
            lethal.entry(o.region).or_default().push(o.id);
        }
        if o.posted.is_empty() {
            out.push(format!("omen {}: its claim is posted and said nowhere on this seed", o.id));
        }
    }
    for (r, ids) in lethal {
        if ids.len() > 1 {
            out.push(format!("two lethal omens true in the {r}: {ids:?}"));
        }
    }
    out
}
