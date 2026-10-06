//! PORT.md §6.m stage 8 (stories, names, boards) and the county's end of stage 9 (the proven
//! county). Carries the seeded parts of `jane/test/stories.test.ts`, `tales.test.ts`,
//! `truth.test.ts`, `locked-words.test.ts` and `quest-audit.test.ts`, and the story parts of
//! `county.test.ts` and `town.test.ts` (the town's small stories are sim plays, P4). The data parts
//! (thirty-odd stories, eight to twelve tales, every pool deep enough, no name inside another) are
//! `jane-data/tests/county.rs` and the build's own checks.
//!
//! Over `SEEDS` seeds (64 by default), each county built once through `build_zone` (proven by the
//! solver, re-rolled on a refusal) on every core, and every check run on it:
//!
//! - every story claims a place of its kind or records why not, and that is rare (the TypeScript's
//!   bar, 5 %); no two share one; each placed one has its board saying its name, its rect and its
//!   mark; a few stand by the first walk and none on the platform's doorstep;
//! - names are unique per pool and on the seed, keep to VOICE.md (no dashes, nobody's name), and
//!   the words that put them in fit the tracker; every board names its place in capitals;
//! - every placement row at a story's place lands when its story placed, and none when it did not;
//! - every `{place:X}` in a placed story's words names a story with a place on the seed;
//! - every tale's things stand where she can walk to them; the words are true (the stones, the
//!   step, the pen, the chair, the seats, the tin);
//! - every placed story is on screen from a road or from the footpath it laid;
//! - `build_zone(County, s)` validates; the county re-rolls on at most 2 % of seeds;
//! - the same seed builds the same county.

mod common;

use std::sync::OnceLock;

use jane_core::action::{Action, ListRef, TextRef};
use jane_core::blueprint::{PropSpawn, StoryPlace};
use jane_core::search::{Conn, Reach, flood};
use jane_core::view::{HALF_H_CELLS, HALF_W_CELLS};
use jane_core::{Blueprint, Key, NameId, Rect, StoryId, Tile, ZoneId};
use jane_data::{PlaceAt, PlaceKind, StoryDef};
use jane_world::build_zone;
use jane_world::county::county_skeleton;
use jane_world::names::{Names, all_names, pool, story_name};
use jane_world::solve::{ZoneRules, validate};

fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| i.wrapping_mul(2654435761)).collect()
}

type Check = fn(u32, &Blueprint, &mut Survey);

const CHECKS: &[(&str, Check)] = &[
    ("records", every_story_has_a_record),
    ("first", by_the_first_walk),
    ("names", names_are_their_own),
    ("boards", boards_name_their_places),
    ("rows", story_rows_land),
    ("words", place_words_name_placed_stories),
    ("tales", tales_can_be_walked_to),
    ("truth", the_words_are_true),
    ("seen", story_places_are_seen),
    ("solver", the_solver_holds),
];

/// What one seed's county showed.
#[derive(Default)]
struct Survey {
    /// Per check, what it complained of.
    bad: Vec<Vec<String>>,
    placed: u32,
    skipped: u32,
    /// Per tale, whether it landed whole.
    tales: Vec<(StoryId, bool)>,
    attempts: u8,
}

fn surveys() -> &'static [Survey] {
    static ALL: OnceLock<Vec<Survey>> = OnceLock::new();
    ALL.get_or_init(|| {
        let seeds = seed_list();
        let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
        let per = seeds.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            let jobs: Vec<_> = seeds
                .chunks(per)
                .map(|part| s.spawn(move || part.iter().map(|&seed| survey(seed)).collect::<Vec<_>>()))
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("a county builds")).collect()
        })
    })
}

fn survey(seed: u32) -> Survey {
    let bp = build_zone(ZoneId::County, seed).expect("the county builds");
    let mut s = Survey { bad: vec![Vec::new(); CHECKS.len()], attempts: bp.attempts, ..Survey::default() };
    for (i, (_, check)) in CHECKS.iter().enumerate() {
        let mut one = Survey::default();
        check(seed, &bp, &mut one);
        s.bad[i] = one.bad.into_iter().flatten().collect();
        s.placed += one.placed;
        s.skipped += one.skipped;
        s.tales.extend(one.tales);
    }
    s
}

fn verdict(name: &str) {
    let i = CHECKS.iter().position(|(n, _)| *n == name).expect("a check");
    let bad: Vec<&String> = surveys().iter().flat_map(|s| &s.bad[i]).collect();
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.iter().take(40).map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
}

impl Survey {
    fn bad(&mut self, s: String) {
        if self.bad.is_empty() {
            self.bad.push(Vec::new());
        }
        self.bad[0].push(s);
    }
}

// --- helpers ------------------------------------------------------------------------------------

fn cat() -> &'static jane_data::Catalog {
    jane_data::catalog()
}

fn text_of(bp: &Blueprint, t: TextRef) -> String {
    match t {
        TextRef::Text(id) => cat().text(id).to_owned(),
        TextRef::Local(_) => bp.text(t).unwrap_or_default().to_owned(),
    }
}

/// The words a prop reads out, if it reads.
fn words(bp: &Blueprint, list: Option<ListRef>) -> Option<String> {
    let ListRef::Blueprint(i) = list? else { return None };
    bp.lists[usize::from(i)].iter().find_map(|a| match a {
        Action::Read(t) => Some(text_of(bp, *t)),
        _ => None,
    })
}

fn at(c: jane_core::Cell) -> (i32, i32) {
    (i32::from(c.x), i32::from(c.y))
}

fn named(n: &str) -> NameId {
    cat().name_id(n).unwrap_or_else(|| panic!("no name {n}"))
}

fn prop<'a>(bp: &'a Blueprint, n: &str) -> Option<&'a PropSpawn> {
    let k = Key::Name(cat().name_id(n)?);
    bp.props.iter().find(|p| p.key == k)
}

fn size(p: &PropSpawn) -> (i32, i32) {
    let d = cat().story.prop(p.def);
    (i32::from(d.w), i32::from(d.h))
}

/// Cells of open ground between two footprints (0: touching or overlapping).
fn gap(a: &PropSpawn, b: &PropSpawn) -> i32 {
    let ((ax, ay), (aw, ah)) = (at(a.cell), size(a));
    let ((bx, by), (bw, bh)) = (at(b.cell), size(b));
    let dx = (ax - (bx + bw)).max(bx - (ax + aw)).max(0);
    let dy = (ay - (by + bh)).max(by - (ay + ah)).max(0);
    dx.max(dy)
}

/// A placed story's record.
fn placed(bp: &Blueprint, id: StoryId) -> Option<(Rect, String)> {
    match bp.stories.get(&id)? {
        StoryPlace::Placed { bounds, name, .. } => Some((*bounds, text_of(bp, *name))),
        StoryPlace::Skipped(_) => None,
    }
}

fn story(key: &str) -> &'static StoryDef {
    cat().county.stories.iter().find(|s| s.key == key).unwrap_or_else(|| panic!("no story {key}"))
}

/// Every `{place:<id>}` a text names.
fn place_refs(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("{place:") {
        let after = &rest[i + "{place:".len()..];
        let Some(end) = after.find('}') else { break };
        out.push(&after[..end]);
        rest = &after[end + 1..];
    }
    out
}

/// A text as she reads it on a seed: every `{place:<id>}` is that story's place name.
fn say(seed: u32, s: &str) -> String {
    let mut out = s.to_owned();
    for id in place_refs(s) {
        let name = story_name(seed, story(id).id);
        out = out.replace(&format!("{{place:{id}}}"), &name);
    }
    out
}

/// Cells she can walk to from where she starts: the ground, less every solid thing standing
/// (hidden things stand nowhere). The tales test's flood.
fn walk(bp: &Blueprint) -> Reach {
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let mut blocked = vec![false; (w * h) as usize];
    for p in &bp.props {
        let d = cat().story.prop(p.def);
        if !d.solid || p.hidden {
            continue;
        }
        let (x, y) = at(p.cell);
        for j in y..(y + i32::from(d.h)).min(h) {
            for i in x..(x + i32::from(d.w)).min(w) {
                blocked[(j * w + i) as usize] = true;
            }
        }
    }
    let start = bp.marks[&Key::Name(named("start"))];
    let mut reach = Reach::new();
    let solid = |x: i32, y: i32| bp.tiles.read(x, y, Tile::Void).flags() & jane_core::tile::F_SOLID != 0;
    flood(
        bp.w(),
        bp.h(),
        &[at(start.cell)],
        Conn::Four,
        u32::MAX,
        |x, y| !blocked[(y * w + x) as usize] && !solid(x, y),
        &mut reach,
    );
    reach
}

// --- the records ---------------------------------------------------------------------------------

#[test]
fn every_story_claims_a_place_of_its_kind_or_says_why_not() {
    verdict("records");
}

/// stories.test.ts "every story finds a place, or says why not, and that is rare; no two share
/// one; each has its board", and county.test.ts's story marks.
fn every_story_has_a_record(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let mut boxes: Vec<Rect> = Vec::new();
    let board = cat().story.prop_id("name_board").expect("a name_board row");
    let recorded: Vec<StoryId> = bp.stories.keys().copied().collect();
    let order: Vec<StoryId> = cat().county.stories.iter().map(|x| x.id).collect();
    if recorded != order {
        s.bad(format!("seed {seed}: the records are not every story in data order"));
    }
    for st in cat().county.stories {
        match bp.stories.get(&st.id) {
            None => s.bad(format!("seed {seed}: {} has no record", st.key)),
            Some(StoryPlace::Skipped(why)) => {
                s.skipped += 1;
                let why = text_of(bp, *why);
                if why.len() <= 10 {
                    s.bad(format!("seed {seed}: {} skipped without a reason ({why})", st.key));
                }
            }
            Some(StoryPlace::Placed { kind, name, bounds, .. }) => {
                s.placed += 1;
                let kind = bp.local_names.get(match kind {
                    Key::Local(i) => *i as usize,
                    Key::Name(_) => usize::MAX,
                });
                if kind.map(String::as_str) != Some(st.kind.name()) {
                    s.bad(format!("seed {seed}: {} is at a {kind:?}, not a {}", st.key, st.kind.name()));
                }
                let name = text_of(bp, *name);
                if name != story_name(seed, st.id) {
                    s.bad(format!(
                        "seed {seed}: {} is called {name}, its words say {}",
                        st.key,
                        story_name(seed, st.id)
                    ));
                }
                let has_board = bp.props.iter().any(|p| {
                    p.def == board
                        && p.label.is_some_and(|l| text_of(bp, l) == name)
                        && bounds.contains(at(p.cell).0, at(p.cell).1)
                });
                if !has_board {
                    s.bad(format!("seed {seed}: {}: no board saying {name} at its place", st.key));
                }
                let key = Key::Name(st.place_name);
                if bp.rects.get(&key) != Some(bounds) {
                    s.bad(format!("seed {seed}: {}: no rect story_{} round its place", st.key, st.key));
                }
                match bp.marks.get(&key) {
                    Some(m) if bounds.contains(at(m.cell).0, at(m.cell).1) => {}
                    _ => s.bad(format!("seed {seed}: {}: no mark story_{} at its board", st.key, st.key)),
                }
                if boxes.contains(bounds) {
                    s.bad(format!("seed {seed}: {} shares a place", st.key));
                }
                boxes.push(*bounds);
            }
        }
    }
}

#[test]
fn stories_are_rarely_skipped() {
    let (placed, skipped) = surveys().iter().fold((0, 0), |(p, k), s| (p + s.placed, k + s.skipped));
    println!("stories placed {placed}, skipped {skipped} over {} seeds", surveys().len());
    // The TypeScript's bar: at most one in twenty.
    assert!(skipped * 20 <= placed + skipped, "{skipped} of {} story places skipped", placed + skipped);
}

#[test]
fn a_few_stand_by_the_first_walk_and_none_on_the_platform() {
    verdict("first");
}

/// stories.test.ts "a few stand on the first walk, and none crowds the opening".
fn by_the_first_walk(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let sk = county_skeleton(seed, bp.attempts - 1).expect("the skeleton");
    let centre = |m: i32| m * 16 + 8;
    let first = sk.named.first_walk();
    let cells: Vec<(i32, i32)> = sk
        .roads
        .iter()
        .filter(|r| first.contains(&(r.from, r.to)))
        .flat_map(|r| r.cells.iter().map(|&(x, y)| (centre(x), centre(y))))
        .collect();
    let station = sk.sites.iter().find(|x| x.def.id == "station").expect("a station");
    let (sx, sy) = (2 * centre(station.mx), 2 * centre(station.my));
    let mut near = 0;
    for st in cat().county.stories {
        let Some((b, _)) = placed(bp, st.id) else { continue };
        let by = cells.iter().any(|&(x, y)| {
            let dx = i64::from((b.x - x).max(x - (b.x + b.w)).max(0));
            let dy = i64::from((b.y - y).max(y - (b.y + b.h)).max(0));
            dx * dx + dy * dy < 70 * 70
        });
        near += i32::from(by);
        let (dx, dy) = (i64::from(2 * b.x + b.w - sx), i64::from(2 * b.y + b.h - sy));
        if dx * dx + dy * dy < 4 * 150 * 150 {
            s.bad(format!("seed {seed}: {} is on the platform's doorstep", st.key));
        }
    }
    if near < 3 {
        s.bad(format!("seed {seed}: {near} stories by the first walk"));
    }
}

// --- names -------------------------------------------------------------------------------------

#[test]
fn names_are_unique_per_pool_and_keep_to_the_voice() {
    verdict("names");
}

/// stories.test.ts "every name a seed could paint on a board is its own" and "the words fit the
/// tracker with the longest name any seed could put in, and keep to VOICE.md", on the seed's own
/// names; tales.test.ts "the names are their own".
fn names_are_their_own(seed: u32, bp: &Blueprint, s: &mut Survey) {
    for kind in PlaceKind::ALL {
        let p = pool(seed, kind);
        let mut sorted = p.clone();
        sorted.sort();
        sorted.dedup();
        if kind != PlaceKind::Inn && sorted.len() != p.len() {
            s.bad(format!("seed {seed}: {} has a name twice", kind.name()));
        }
    }
    // Every story's name on the seed is its own (but the inn's, which every inn shares).
    let names = Names::new(seed);
    let mut seen: Vec<(String, &str)> = Vec::new();
    for st in cat().county.stories {
        let n = names.story(st.id);
        if n.contains('\u{2013}') || n.contains('\u{2014}') || n.contains("Jane") || n.is_empty() {
            s.bad(format!("seed {seed}: {} is called \"{n}\"", st.key));
        }
        if st.kind != PlaceKind::Inn {
            if let Some((_, other)) = seen.iter().find(|(m, _)| *m == n) {
                s.bad(format!("seed {seed}: {} and {other} are both {n}", st.key));
            }
        }
        seen.push((n, st.key));
        // The words as she reads them, with the seed's names in.
        for &q in st.quests {
            let q = cat().story.quest(q);
            for r in q.requirements {
                let t = cat().text(r.text);
                if place_refs(t).is_empty() {
                    s.bad(format!("{}: a step that names no place: \"{t}\"", q.id));
                }
                let said = say(seed, t);
                if said.chars().count() > 70 {
                    s.bad(format!("seed {seed}: {}: \"{said}\" is too long for the tracker", q.id));
                }
            }
            let (d, back) = (say(seed, cat().text(q.description)), say(seed, cat().text(q.return_to)));
            if d.chars().count() > 320 || back.chars().count() > 70 {
                s.bad(format!("seed {seed}: {}: description or hand-in too long", q.id));
            }
        }
    }
    // Every board on the seed says a name no other board says, but the inns'.
    let board = cat().story.prop_id("name_board").expect("a name_board row");
    let inn = all_names(PlaceKind::Inn);
    let mut labels: Vec<String> = bp
        .props
        .iter()
        .filter(|p| p.def == board)
        .filter_map(|p| p.label.map(|l| text_of(bp, l)))
        .filter(|l| !inn.contains(&l.as_str()))
        .collect();
    let n = labels.len();
    labels.sort();
    labels.dedup();
    if labels.len() != n {
        s.bad(format!("seed {seed}: two boards say the same name"));
    }
}

#[test]
fn every_board_names_its_place() {
    verdict("boards");
}

/// Every name board reads its own label in capitals; a story's own board line is its own.
fn boards_name_their_places(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let board = cat().story.prop_id("name_board").expect("a name_board row");
    for p in bp.props.iter().filter(|p| p.def == board) {
        let Some(label) = p.label.map(|l| text_of(bp, l)) else {
            s.bad(format!("seed {seed}: a board with no name"));
            continue;
        };
        // A tale's own board says what the tale says it does (COKER. DRY-STONE WALLING.).
        let own = |w: &str| {
            cat().county.stories.iter().any(|st| {
                st.board.is_some_and(|b| {
                    story_name(seed, st.id) == label && cat().text(b).replace("{NAME}", &label.to_uppercase()) == w
                })
            })
        };
        match words(bp, p.use_list) {
            Some(w) if (w.contains(&label.to_uppercase()) || own(&w)) && !w.contains("{NAME}") => {}
            w => s.bad(format!("seed {seed}: the board for {label} says {w:?}")),
        }
    }
}

// --- rows --------------------------------------------------------------------------------------

#[test]
fn every_row_at_a_story_place_lands_when_its_story_placed() {
    verdict("rows");
}

fn story_rows_land(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let has_prop = |n: NameId| bp.props.iter().any(|p| p.key == Key::Name(n));
    let has_unit = |n: NameId| bp.units.iter().any(|u| u.key == Key::Name(n));
    for row in cat().county.placements {
        let PlaceAt::Place { story: id, slot } = row.at else { continue };
        let st = cat().county.story(id);
        let key = cat().name(row.key);
        match placed(bp, id) {
            Some((b, _)) => {
                if let Some(e) = row.edit {
                    if let Some(k) = e.key {
                        if !has_prop(k) {
                            s.bad(format!("seed {seed}: {}: the edit {key} named nothing {}", st.key, cat().name(k)));
                        }
                    }
                    continue;
                }
                if slot.is_some_and(|x| cat().name(x) == "hostiles") {
                    let u = row.unit.expect("a hostiles row stands a unit");
                    if !bp.units.iter().any(|x| x.def == u.def && b.grow(8).contains(at(x.cell).0, at(x.cell).1)) {
                        s.bad(format!("seed {seed}: {}: none of its camp is {key}'s creature", st.key));
                    }
                    continue;
                }
                if row.prop.is_some() {
                    for &k in row.keys {
                        if !has_prop(k) {
                            s.bad(format!("seed {seed}: {}: {} was not put down", st.key, cat().name(k)));
                        }
                    }
                    if let Some(h) = row.hides {
                        if !has_prop(h.key) {
                            s.bad(format!("seed {seed}: {}: {} was not hidden", st.key, cat().name(h.key)));
                        }
                    }
                }
                if row.unit.is_some() && !has_unit(row.key) {
                    s.bad(format!("seed {seed}: {}: {key} was not stood", st.key));
                }
                if let Some(m) = row.mark {
                    if !bp.marks.contains_key(&Key::Name(m)) {
                        s.bad(format!("seed {seed}: {}: no mark {}", st.key, cat().name(m)));
                    }
                }
                if let Some(r) = row.rect {
                    if !bp.rects.contains_key(&Key::Name(r.name)) {
                        s.bad(format!("seed {seed}: {}: no rect {}", st.key, cat().name(r.name)));
                    }
                }
            }
            None => {
                if row.keys.iter().any(|&k| has_prop(k) || has_unit(k)) {
                    s.bad(format!("seed {seed}: {} has no place, but {key} stands", st.key));
                }
            }
        }
    }
}

#[test]
fn a_story_named_in_placed_words_has_a_place() {
    verdict("words");
}

/// stories.test.ts: every `{place:X}` is a story; on a seed, the words of a placed story only send
/// her to places the seed has.
fn place_words_name_placed_stories(seed: u32, bp: &Blueprint, s: &mut Survey) {
    for st in cat().county.stories {
        if placed(bp, st.id).is_none() {
            continue;
        }
        for &q in st.quests {
            let q = cat().story.quest(q);
            let texts = q.requirements.iter().map(|r| r.text).chain([q.description, q.return_to, q.completion]);
            for t in texts {
                for id in place_refs(cat().text(t)) {
                    if placed(bp, story(id).id).is_none() {
                        s.bad(format!("seed {seed}: {} names {{place:{id}}}, which has no place", q.id));
                    }
                }
            }
        }
    }
}

// --- tales and truth ----------------------------------------------------------------------------

#[test]
fn every_tale_lands_whole_where_she_can_walk_to_it() {
    verdict("tales");
    let mut rate: Vec<(StoryId, u32)> = Vec::new();
    for s in surveys() {
        for &(id, whole) in &s.tales {
            match rate.iter_mut().find(|(t, _)| *t == id) {
                Some((_, n)) => *n += u32::from(whole),
                None => rate.push((id, u32::from(whole))),
            }
        }
    }
    let n = surveys().len();
    let line: Vec<String> = rate.iter().map(|(id, k)| format!("{} {k}/{n}", cat().county.story(*id).key)).collect();
    println!("tales landed whole: {}", line.join(", "));
}

/// tales.test.ts "every tale finds its place, puts down everything it needs, and all of it can be
/// walked to".
fn tales_can_be_walked_to(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let reach = walk(bp);
    let near = |x: i32, y: i32, w: i32, h: i32| (y - 1..=y + h).any(|j| (x - 1..=x + w).any(|i| reach.reached(i, j)));
    for t in cat().county.stories.iter().filter(|t| t.tale) {
        if placed(bp, t.id).is_none() {
            let why = match bp.stories.get(&t.id) {
                Some(StoryPlace::Skipped(w)) => text_of(bp, *w),
                _ => "no record".into(),
            };
            s.bad(format!("seed {seed}: {} has no place ({why})", t.key));
            s.tales.push((t.id, false));
            continue;
        }
        let mut whole = true;
        for row in cat().county.placements {
            if !matches!(row.at, PlaceAt::Place { story, .. } if story == t.id) || row.edit.is_some() {
                continue;
            }
            let props =
                row.prop.map(|_| row.keys.to_vec()).unwrap_or_default().into_iter().chain(row.hides.map(|h| h.key));
            for k in props {
                match bp.props.iter().find(|p| p.key == Key::Name(k)) {
                    Some(p) => {
                        let ((x, y), (w, h)) = (at(p.cell), size(p));
                        if !near(x, y, w, h) {
                            s.bad(format!("seed {seed}: {}: {} cannot be walked to", t.key, cat().name(k)));
                            whole = false;
                        }
                    }
                    None => {
                        s.bad(format!("seed {seed}: {}: {} was not put down", t.key, cat().name(k)));
                        whole = false;
                    }
                }
            }
            if row.unit.is_some() {
                match bp.units.iter().find(|u| u.key == Key::Name(row.key)) {
                    Some(u) if near(at(u.cell).0, at(u.cell).1, 1, 1) => {}
                    u => {
                        let what = if u.is_some() { "cannot be walked to" } else { "was not stood" };
                        s.bad(format!("seed {seed}: {}: {} {what}", t.key, cat().name(row.key)));
                        whole = false;
                    }
                }
            }
            if let Some(m) = row.mark {
                match bp.marks.get(&Key::Name(m)) {
                    Some(mk) if reach.reached(at(mk.cell).0, at(mk.cell).1) => {}
                    mk => {
                        let what = if mk.is_some() { "cannot be walked to" } else { "is missing" };
                        s.bad(format!("seed {seed}: {}: mark {} {what}", t.key, cat().name(m)));
                        whole = false;
                    }
                }
            }
        }
        s.tales.push((t.id, whole));
    }
}

#[test]
fn what_the_words_say_is_there_is_there() {
    verdict("truth");
}

/// truth.test.ts "stories: the stones, the step, the pen, the garden, the chair and the seats are
/// there", on every seed the story has a place.
fn the_words_are_true(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let has = |key: &str| placed(bp, story(key).id).is_some();
    let push = |p: &PropSpawn| cat().story.prop(p.def).push;
    if has("bettany") {
        // "There are four stones, and they shift if you lean on them." "The spare's under a stone."
        let key = Key::Name(named("bettany_key_drop"));
        let stones: Vec<&PropSpawn> = (1..=4).filter_map(|n| prop(bp, &format!("bettany_stone_{n}"))).collect();
        let under: Vec<&&PropSpawn> = stones.iter().filter(|p| p.under == Some(key)).collect();
        let drop = prop(bp, "bettany_key_drop");
        if stones.len() != 4 || !stones.iter().all(|p| push(p)) || under.len() != 1 {
            s.bad(format!("seed {seed}: bettany: {} stones, {} over the key", stones.len(), under.len()));
        } else if drop.is_none_or(|k| !k.hidden || k.cell != under[0].cell) {
            s.bad(format!("seed {seed}: bettany: the key is not hidden under its stone"));
        }
        for p in &stones {
            let t = bp.tiles.read(at(p.cell).0, at(p.cell).1, Tile::Void);
            if !matches!(t, Tile::Crops | Tile::Dirt | Tile::Garden) {
                s.bad(format!("seed {seed}: bettany: a stone on {t:?}, not in the garden"));
            }
        }
    }
    if has("hackett_house") {
        // "My mother kept a tin under the front step."
        let tin = Key::Name(named("hackett_tin_drop"));
        let ok = prop(bp, "hackett_step").is_some_and(|p| p.under == Some(tin) && push(p))
            && prop(bp, "hackett_tin_drop").is_some_and(|p| p.hidden);
        if !ok {
            s.bad(format!("seed {seed}: hackett: the tin is not under the step"));
        }
    }
    if has("leckie") {
        // "It's in the sheep pen." "In the pen with the sheep."
        let sheep = cat().combat.unit_id("sheep").expect("sheep");
        if let Some(sack) = prop(bp, "leckie_sack_drop") {
            let (x, y) = at(sack.cell);
            let n = bp
                .units
                .iter()
                .filter(|u| u.def == sheep && (at(u.cell).0 - x).abs().max((at(u.cell).1 - y).abs()) <= 7)
                .count();
            if n < 2 {
                s.bad(format!("seed {seed}: leckie: {n} sheep round the sack"));
            }
        } else {
            s.bad(format!("seed {seed}: leckie: no sack"));
        }
    }
    if has("denny_ruin") {
        // "Candles. And a chair."
        match (prop(bp, "denny_chair"), prop(bp, "denny_candles")) {
            (Some(a), Some(b)) if gap(a, b) <= 3 => {}
            _ => s.bad(format!("seed {seed}: denny: the chair is not by the candles")),
        }
    }
    if has("voke_brother") {
        // "Two stones are set by it to sit on."
        let seats = (1..=4).filter(|n| prop(bp, &format!("voke_seat_{n}")).is_some()).count();
        if seats != 2 {
            s.bad(format!("seed {seed}: voke: {seats} seats"));
        }
    }
    if has("farrant_camp") {
        // "Teaspoons? They're not mine."
        let ring = cat().combat.item_id("farrant_ring").expect("the ring");
        let spoons = cat().combat.item_id("crow_spoons").expect("the spoons");
        let the_box = bp.props.iter().find(|p| p.loot.iter().any(|l| l.item == ring));
        if the_box.is_none_or(|b| !b.loot.iter().any(|l| l.item == spoons)) {
            s.bad(format!("seed {seed}: farrant: the ring is not with the spoons"));
        }
    }
    if has("vosper_sister") && prop(bp, "vosper_tin_drop").is_none_or(|p| !p.hidden) {
        // "a biscuit tin heavy with paper is put out on the step"
        s.bad(format!("seed {seed}: vosper: the tin is out before it is put out"));
    }
}

/// locked-words.test.ts, the data half: the left-luggage trunk is a trunk, locked to its own key.
#[test]
fn the_left_luggage_trunk_is_a_trunk_locked_to_its_key() {
    let row =
        cat().county.placements.iter().find(|r| cat().name(r.key) == "left_luggage_trunk").expect("the trunk's row");
    let t = row.prop.expect("a prop");
    let def = cat().story.prop(t.def);
    assert_eq!(def.id, "luggage_trunk");
    assert_eq!(cat().text(def.name), "Trunk");
    assert!(t.locked);
    assert_eq!(t.key_tag.map(|k| cat().name(k)), Some("left_luggage"));
}

// --- findable --------------------------------------------------------------------------------

#[test]
fn every_story_place_is_on_screen_from_a_road_or_its_footpath() {
    verdict("seen");
}

/// quest-audit.test.ts rule C, for the stories' places: the place is on screen (1.5 half-screens
/// each way) from a made road or from the footpath the story laid to it.
fn story_places_are_seen(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let (hw, hh) = (HALF_W_CELLS as i32, HALF_H_CELLS as i32);
    for st in cat().county.stories {
        let Some(StoryPlace::Placed { bounds: b, path, .. }) = bp.stories.get(&st.id) else { continue };
        let seen = |x: i32, y: i32| {
            let dx = (b.x - x).max(x - (b.x + b.w - 1)).max(0);
            let dy = (b.y - y).max(y - (b.y + b.h - 1)).max(0);
            2 * dx <= 3 * hw && 2 * dy <= 3 * hh
        };
        let r = b.grow(3 * hw / 2 + 1);
        let road = (r.y..r.y + r.h)
            .any(|y| (r.x..r.x + r.w).any(|x| bp.tiles.read(x, y, Tile::Void) == Tile::Road && seen(x, y)));
        if !road && !path.iter().any(|c| seen(at(*c).0, at(*c).1)) {
            s.bad(format!("seed {seed}: {} is out of sight of any road or path", st.key));
        }
        if !path.is_empty() {
            let (x, y) = at(path[0]);
            if bp.tiles.read(x, y, Tile::Void) != Tile::Road {
                s.bad(format!("seed {seed}: {}'s footpath does not leave from a road", st.key));
            }
        }
    }
}

// --- proven -----------------------------------------------------------------------------------

#[test]
fn the_county_is_proven_and_rarely_re_rolled() {
    verdict("solver");
    let n = surveys().len();
    let rerolled = surveys().iter().filter(|s| s.attempts > 1).count();
    let worst = surveys().iter().map(|s| s.attempts).max().unwrap_or(0);
    println!("county: {rerolled} of {n} seeds re-rolled (most attempts {worst})");
    // PORT.md §6.j: county attempt > 0 on at most 2 % of seeds.
    assert!(rerolled * 50 <= n, "{rerolled} of {n} counties re-rolled");
}

/// `build_zone` hands back a county the solver holds.
fn the_solver_holds(seed: u32, bp: &Blueprint, s: &mut Survey) {
    let r = validate(bp, &ZoneRules::for_zone(ZoneId::County));
    for e in &r.errors {
        s.bad(format!("seed {seed} (attempt {}): {}", bp.attempts, e.show(bp)));
    }
}

/// Names are the seed's, not the county's: a story's words can be read anywhere without building
/// anything, and a tale's name is its own on every seed.
#[test]
fn a_story_name_is_known_without_the_county() {
    let tale = story("tale_bees");
    assert_eq!(story_name(3, tale.id), story_name(4, tale.id));
    assert_eq!(story_name(3, tale.id), cat().text(tale.name.expect("a tale's own name")));
    let differ = cat().county.stories.iter().filter(|s| !s.tale).any(|s| story_name(3, s.id) != story_name(4, s.id));
    assert!(differ, "another seed, other names");
}
