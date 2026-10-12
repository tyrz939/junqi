//! The terraced county's proofs (PLAN-map-r4 section 6.2): what the solver and the pit proofs say
//! of the county as built, run on every target so PC and PSP reach the same county. They are an
//! offline check on a map: the one the game ships is picked by hand and must pass them.
//!
//! - **P-1** the solver: every contract name, every mark, every quest's anchor and every gate opens,
//!   with the ledges and spans crossed (`solve::validate`).
//! - **P-2** no pits, by the tiles: whatever she can walk to she can walk back from, as the spans
//!   are built and mended.
//! - **P-3** no pits, with the props that stand in the way.
//! - **P-4** night: with the two barriers shut every story place is still reached and there is still
//!   no pit.
//! - **P-5** the routes' walking times: the first walk, the Castle, the mine, the Museum and the
//!   School, against [`BANDS`]. Reported, not refused: a hand-picked map is judged on its times by the
//!   designer, who is told them.
//! - **P-6** a kept fire with no ledge landing near it (reported, not refused, in this round).
//! - **P-7** nothing on the land's joints: no prop or unit on a face, rim, stair, ladder, ledge, fall,
//!   landing or deck, no prop across two levels, no chunk's box across a face, no joint of the carve
//!   laid over by a stage.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::num::octile10;
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Key, Rect, Tile};

use super::carve::Carved;
use super::one_level;
use crate::bits::Bits;
use crate::solve::portals::{Portals, reach_words};
use crate::solve::{ZoneRules, validate};

/// A route's walking pace, cells in fifteen seconds (3.75 a second, 60 px/s at 16 px a cell).
const CELLS_PER_15S: i32 = 56;

/// A chain of routes by their ends in the plan, and the fewest and most seconds MAP.md allows it.
pub type Band = (&'static [(&'static str, &'static str)], i32, i32);

/// The routes P-5 times, by their ends in the plan (a chain is the routes end to end), with the seconds
/// of walking MAP.md asks of each: the first walk, Julie's to the Castle, the mine road, the Museum road
/// and the long way to the School. Measured on the carve's own road waypoints (the macro cells' centres
/// with each ramp's foot and top and each deck's ends put in order), which the roads follow, at the pace
/// above. The plan's own check (`macro_check`) times the same routes at 5 cells a second, and holds the
/// Halt to the mine mouth to 300 to 420 s of that: at this pace the routes run about a third longer.
pub const BANDS: [Band; 5] = [
    (&[("station", "julie_house")], 60, 150),
    (&[("julie_house", "town")], 90, 240),
    (&[("town", "gold_mine")], 300, 420),
    (&[("town", "museum")], 0, 300),
    (
        &[
            ("station", "julie_house"),
            ("julie_house", "town"),
            ("town", "graveyard"),
            ("graveyard", "factory"),
            ("factory", "school"),
        ],
        0,
        900,
    ),
];

/// What a proof found: the routes' times, and the report-only counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Proof {
    /// P-5: seconds on each of [`BANDS`]' routes (-1 where the plan has no such route).
    pub seconds: [i32; 5],
    /// P-6: kept fires, and how many have no ledge landing within [`FIRE_LEDGE`] cells.
    pub fires: u32,
    pub fires_far: u32,
}

impl Proof {
    /// Whether route `k` of [`BANDS`] takes the time MAP.md asks of it.
    pub fn in_band(&self, k: usize) -> bool {
        (BANDS[k].1..=BANDS[k].2).contains(&self.seconds[k])
    }
}

/// Why a county was refused: the proof's name and the words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub proof: &'static str,
    pub text: String,
}

fn refuse(proof: &'static str, text: String) -> Refusal {
    Refusal { proof, text }
}

/// A ledge landing this near a kept fire serves it (about eight seconds' walk).
pub const FIRE_LEDGE: i32 = 60;

/// The walking time along a chain of routes, in seconds, or `None` if the plan lacks one.
pub fn route_seconds(carved: &Carved, chain: &[(&str, &str)]) -> Option<i32> {
    let mut len10: i64 = 0;
    for &(from, to) in chain {
        let ri = carved.plan.routes.iter().position(|r| r.from == from && r.to == to)?;
        let way = carved.ways.get(ri)?;
        for w in way.pts.windows(2) {
            len10 += i64::from(octile10(w[1].0 - w[0].0, w[1].1 - w[0].1));
        }
    }
    // cells = len10 / 10; seconds = cells * 15 / 56.
    Some((len10 * 15 / (10 * i64::from(CELLS_PER_15S))) as i32)
}

/// P-5: each of [`BANDS`]' routes timed (-1: the plan has no such route).
pub fn times(carved: &Carved) -> [i32; 5] {
    let mut out = [-1; 5];
    for (s, &(chain, ..)) in out.iter_mut().zip(&BANDS) {
        *s = route_seconds(carved, chain).unwrap_or(-1);
    }
    out
}

/// The tiles that stop feet, a bit a cell (`y * w + x`).
fn solid_words(bp: &Blueprint) -> Vec<u64> {
    let (w, h) = (bp.w() as usize, bp.h() as usize);
    let mut solid_id = [false; 256];
    for &t in Tile::ALL {
        solid_id[usize::from(t.id())] = t.flags() & F_SOLID != 0;
    }
    let mut words = vec![0u64; (w * h).div_ceil(64)];
    let mut ids = vec![0u8; w];
    for y in 0..h {
        bp.tile_row(0, y as u32, &mut ids);
        let base = y * w;
        for (x, &id) in ids.iter().enumerate() {
            if solid_id[usize::from(id)] {
                let i = base + x;
                words[i >> 6] |= 1 << (i & 63);
            }
        }
    }
    words
}

/// The cells under props that stand in the way of feet, a bit a cell: what the solver counts as
/// shutting them out (`PropDef::shuts_cells` and a gate, not what is pushed or carried), the whole
/// footprint, unless hidden.
pub(super) fn blocked_words(bp: &Blueprint) -> Vec<u64> {
    let cat = jane_data::catalog();
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let mut words = vec![0u64; (w as usize * h as usize).div_ceil(64)];
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        let shuts = d.gate || (d.solid && !d.push && !d.carry);
        if p.hidden || !shuts {
            continue;
        }
        let r = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
        for (x, y) in r.cells() {
            if x >= 0 && y >= 0 && x < w && y < h {
                let i = y as usize * w as usize + x as usize;
                words[i >> 6] |= 1 << (i & 63);
            }
        }
    }
    words
}

/// Where she starts.
fn start_cell(bp: &Blueprint) -> Option<(i32, i32)> {
    let id = jane_data::catalog().name_id("start")?;
    bp.marks.get(&Key::Name(id)).map(|m| (i32::from(m.cell.x), i32::from(m.cell.y)))
}

/// The cells reached from `start` over `open` (a bit a cell), forward and back across the carve's
/// ledges and spans (as built, or all mended with `broken` 0).
fn pit(bp: &Blueprint, portals: &Portals, open: &[u64], start: (i32, i32), broken: u64) -> (Bits, Option<(i32, i32)>) {
    let (w, h) = (bp.w(), bp.h());
    let fwd = reach_words(w, h, open, portals, &[start], broken, false);
    let back = reach_words(w, h, open, portals, &[start], broken, true);
    let first = (0..fwd.len().div_ceil(64)).find_map(|k| {
        let m = fwd.word(k) & !back.word(k);
        (m != 0).then(|| {
            let i = (k << 6) + m.trailing_zeros() as usize;
            ((i % w as usize) as i32, (i / w as usize) as i32)
        })
    });
    (fwd, first)
}

/// The land's joints: a face, a ledge, a stair, a ladder, a fall.
fn joint(t: Tile) -> bool {
    matches!(
        t,
        Tile::Cliff
            | Tile::LedgeN
            | Tile::LedgeE
            | Tile::LedgeS
            | Tile::LedgeW
            | Tile::Stair
            | Tile::Ladder
            | Tile::Waterfall
    )
}

/// P-7: nothing set on the land's joints. Every offence found, the first `limit` of them, in words.
pub fn joint_offences(bp: &Blueprint, carved: &Carved, limit: usize) -> Vec<String> {
    let cat = jane_data::catalog();
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    // The cells of landings, joins and decks, a bit a cell.
    let mut held = Bits::new(w as usize * h as usize, false);
    let mut hold = |r: Rect| {
        for y in r.y.max(0)..r.bottom().min(h) {
            let row = y as usize * w as usize;
            held.fill(row + r.x.max(0) as usize..row + r.right().min(w) as usize, true);
        }
    };
    for j in &carved.joins {
        hold(j.rect);
        j.landings.iter().for_each(|&l| hold(l));
    }
    carved.spans.iter().for_each(|s| hold(s.rect));
    // And every cell the carve laid as a face, a rim, a stair, a ladder, a ledge or a fall (a chunk's
    // own crag, a mine's rock face, is its own), which no stage has laid over.
    let mut out = Vec::new();
    let mut row: Vec<Option<Tile>> = vec![None; w as usize];
    let mut ids = vec![0u8; w as usize];
    for y in 0..h {
        row.fill(None);
        carved.tile_row(y, &mut row);
        bp.tile_row(0, y as u32, &mut ids);
        for (x, t) in row.iter().enumerate() {
            let Some(t) = t.filter(|&t| joint(t)) else { continue };
            held.set(y as usize * w as usize + x, true);
            if ids[x] != t.id() && out.len() < limit {
                out.push(format!("the carve's {t:?} at ({x}, {y}) was laid over with {:?}", Tile::from_id(ids[x])));
            }
        }
    }
    if out.len() >= limit {
        return out;
    }
    let on_joint =
        |x: i32, y: i32| -> bool { x < 0 || y < 0 || x >= w || y >= h || held[y as usize * w as usize + x as usize] };
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        let (x0, y0) = (i32::from(p.cell.x), i32::from(p.cell.y));
        let level = bp.level_at(x0, y0);
        let name = d.id;
        let cells = (0..i32::from(d.h)).flat_map(|j| (0..i32::from(d.w)).map(move |i| (x0 + i, y0 + j)));
        let bad = cells.into_iter().find_map(|(x, y)| {
            if on_joint(x, y) {
                Some(format!("a {name} at ({x0}, {y0}) stands on the land's joint at ({x}, {y}): {:?}", bp.tile(x, y)))
            } else if bp.level_at(x, y) != level {
                Some(format!("a {name} at ({x0}, {y0}) stands across two levels"))
            } else {
                None
            }
        });
        out.extend(bad);
        if out.len() >= limit {
            return out;
        }
    }
    for u in &bp.units {
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        if on_joint(x, y) {
            out.push(format!("a unit at ({x}, {y}) stands on the land's joint: {:?}", bp.tile(x, y)));
            if out.len() >= limit {
                return out;
            }
        }
    }
    if let Some(level) = &bp.level {
        for pad in &carved.pads {
            if !one_level(level, pad.boxr) {
                out.push(format!("the {} chunk's box crosses a face", pad.site));
            }
        }
    }
    out
}

fn joints(bp: &Blueprint, carved: &Carved) -> Result<(), Refusal> {
    match joint_offences(bp, carved, 1).into_iter().next() {
        Some(why) => Err(refuse("P-7", why)),
        None => Ok(()),
    }
}

/// The kept fires and how many have no ledge landing within [`FIRE_LEDGE`] cells (P-6).
fn fires(bp: &Blueprint, carved: &Carved) -> (u32, u32) {
    let cat = jane_data::catalog();
    let landings: Vec<(i32, i32)> = carved.portals().ledges.iter().map(|l| l.landing).collect();
    let near = FIRE_LEDGE * FIRE_LEDGE;
    let (mut n, mut far) = (0, 0);
    for p in bp.props.iter().filter(|p| {
        let d = cat.story.prop(p.def);
        d.rest && !d.made
    }) {
        n += 1;
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        if !landings.iter().any(|&(lx, ly)| (lx - x) * (lx - x) + (ly - y) * (ly - y) <= near) {
            far += 1;
        }
    }
    (n, far)
}

/// P-1: the solver's word on the county.
pub fn solved(bp: &Blueprint, rules: &ZoneRules) -> Result<(), Refusal> {
    let report = validate(bp, rules);
    if report.ok() {
        return Ok(());
    }
    let first: Vec<String> = report.errors.iter().take(3).map(|e| format!("{}", e.show(bp))).collect();
    Err(refuse("P-1", format!("{} errors, the first: {}", report.errors.len(), first.join("; "))))
}

/// Every proof, the cheap ones first: P-7, the pits (P-2, P-3), the night (P-4), and then the solver
/// (P-1), which `report` hears as `"solve"`. P-5 and P-6 are counted, not refused.
pub fn prove(bp: &Blueprint, carved: &Carved, rules: &ZoneRules, report: crate::Report<'_>) -> Result<Proof, Refusal> {
    report("prove");
    joints(bp, carved)?;
    let Some(start) = start_cell(bp) else { return Err(refuse("P-2", String::from("the county has no start"))) };
    let (w, h) = (bp.w(), bp.h());
    let solid = solid_words(bp);
    let open: Vec<u64> = solid.iter().map(|s| !s).collect();
    let broken = crate::solve::portals::built_broken(bp);
    let portals = carved.portals();
    for (label, bits) in [("as built", broken), ("mended", 0)] {
        if label == "mended" && broken == 0 {
            continue;
        }
        if let (_, Some(at)) = pit(bp, &portals, &open, start, bits) {
            return Err(refuse("P-2", format!("a cell at {at:?} can be reached but not left ({label})")));
        }
    }
    // With the props that stand in the way.
    let blocked = blocked_words(bp);
    let open_props: Vec<u64> = open.iter().zip(&blocked).map(|(o, b)| o & !b).collect();
    drop(open);
    let (day, at) = pit(bp, &portals, &open_props, start, broken);
    if let Some(at) = at {
        return Err(refuse("P-3", format!("a cell at {at:?} can be reached but not left, with the props")));
    }
    // Night: the two barriers shut.
    let mut night = open_props;
    for r in super::stage::night_barriers(carved) {
        for (x, y) in r.cells() {
            if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                let i = y as usize * w as usize + x as usize;
                night[i >> 6] &= !(1 << (i & 63));
            }
        }
    }
    let (reached, at) = pit(bp, &portals, &night, start, broken);
    if let Some(at) = at {
        return Err(refuse("P-4", format!("a cell at {at:?} can be reached but not left with the barriers shut")));
    }
    for (key, m) in &bp.marks {
        let (x, y) = (i32::from(m.cell.x), i32::from(m.cell.y));
        let i = y as usize * w as usize + x as usize;
        // The story's places: the named marks that were reached by day must be reached by night.
        if matches!(key, Key::Name(_)) && day[i] && !reached[i] {
            return Err(refuse("P-4", format!("the mark at ({x}, {y}) is cut off with the barriers shut")));
        }
    }
    let seconds = times(carved);
    let (fires, fires_far) = fires(bp, carved);
    report("solve");
    solved(bp, rules)?;
    Ok(Proof { seconds, fires, fires_far })
}
