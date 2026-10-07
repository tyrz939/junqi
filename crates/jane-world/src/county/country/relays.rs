//! Dead lamp runs, and the relay box at the head of each (`relayRuns` in `county.ts`). The
//! skeleton decides which stretches of road still work; this finds the longest stretches that do
//! not and furnishes them: a box she will walk past many times and can do nothing with, and the
//! lamps it feeds, standing dark at the usual spacing. Sparking a box switches its run on and sets
//! `lamps_<n>`, a flag the threat field can read later: a lit road keeps its discount after dark.
//! The Factory's reward, out here where it counts. Every zone exists (PORT.md §6.l), so every
//! county has them. No dice.

use alloc::format;
use alloc::vec::Vec;
use jane_core::action::{Action, FlagKey, FlagOp};
use jane_core::{Key, Tile};

use super::County;
use super::defs::defs;

/// Dead lamps, cells apart along a run. *Tuning.*
const SPACING: usize = 26;
/// A dark stretch shorter than four lamps is a gap, not a run. *Tuning.*
const LEAST: usize = SPACING * 4;
/// Boxes a county gets at most. *Tuning.*
const RUNS: usize = 3;
/// Fewer lamps than this and the box is taken back out. *Tuning.*
const FEWEST: usize = 3;
/// A run's head is its first point this far outside every set place's box: the chunks' claimed
/// ground and a little. *Tuning.*
const CHUNK_CLEAR: i32 = 8;
/// Where beside a road a box or a dead lamp may stand: north or south of the point four to six
/// cells out (the TypeScript's), then east or west, for a road that runs north and south.
const BESIDE: [(i32, i32); 12] =
    [(0, -4), (0, 4), (0, -5), (0, 5), (0, -6), (0, 6), (-4, 0), (4, 0), (-5, 0), (5, 0), (-6, 0), (6, 0)];

/// A dark stretch of a road: points `from..=to` of line `line`.
#[derive(Clone, Copy, Debug)]
struct Run {
    line: usize,
    from: usize,
    to: usize,
}

/// The longest dark stretches get a relay box and a run of dead lamps.
pub fn relay_runs(c: &mut County<'_>) {
    // Every unlit stretch of every road, longest first; ties on where it starts.
    let mut runs = Vec::new();
    for (n, (line, on)) in c.lines.iter().zip(&c.lit).enumerate() {
        if on.len() != line.len() {
            continue;
        }
        let mut from = None;
        for i in 0..=line.len() {
            let dark = on.get(i).is_some_and(|&lit| !lit);
            match (dark, from) {
                (true, None) => from = Some(i),
                (false, Some(f)) => {
                    if i - f >= LEAST {
                        runs.push(Run { line: n, from: f, to: i - 1 });
                    }
                    from = None;
                }
                _ => {}
            }
        }
    }
    runs.sort_by_key(|r| (core::cmp::Reverse(r.to - r.from), r.line, r.from));

    // The box goes down first, tried at several points along the head of the run: the lit end of a
    // dark stretch is usually the edge of a town or a yard, where the ground is spoken for. Only
    // once a box stands does its run get its lamps.
    let d = defs();
    let mut placed = 0;
    for run in runs {
        if placed >= RUNS {
            break;
        }
        let line = c.lines[run.line].clone();
        let box_key = c.k.local(&format!("relay_{placed}"));
        // The head of the run is where it leaves the set places' ground: a road that is dark from
        // end to end starts in the middle of a place, where nothing of the country's goes.
        let Some(head) = (run.from..=run.to).find(|&i| {
            let (x, y) = line[i];
            !c.chunks.iter().any(|ch| ch.bounds.grow(CHUNK_CLEAR).contains(x, y))
        }) else {
            continue;
        };
        let mut at = None;
        let last = (run.to.saturating_sub(SPACING)).min(head + SPACING);
        let mut i = head;
        while i <= last && at.is_none() {
            if beside(c, &line, i, d.p.relay_box, box_key) {
                at = Some(i);
            }
            i += 4;
        }
        let Some(at) = at else { continue };
        let mut lamps = Vec::new();
        let mut i = at + SPACING;
        while i + 2 < run.to {
            let key = c.k.local(&format!("lamp_run_{placed}_{}", lamps.len()));
            if beside(c, &line, i, d.p.lamp_run, key) {
                lamps.push(key);
            }
            i += SPACING;
        }
        if lamps.len() < FEWEST {
            // Not a run, just a gap. Take the box back out rather than leave a switch for two lamps.
            c.k.retain_props(|p| p.key != box_key && !lamps.contains(&p.key));
            continue;
        }
        let mut acts: Vec<Action> = lamps.iter().map(|&k| Action::Switch { prop: k, on: Some(true) }).collect();
        let flag = c.k.local(&format!("lamps_{placed}"));
        acts.push(Action::Flag { key: FlagKey::Named(flag), op: FlagOp::Set(1) });
        let toast = c.k.text("It takes, and the next one takes, and it goes away down the road ahead of you.");
        acts.push(Action::Toast(toast));
        let list = c.k.list(acts);
        let label = c.k.text("A relay box");
        if let Some(p) = c.k.props_mut().iter_mut().find(|p| p.key == box_key) {
            p.use_list = Some(list);
            p.label = Some(label);
        }
        placed += 1;
    }
}

/// A prop keyed `key` beside point `i` of `line` ([`BESIDE`]), on open, dry, unclaimed ground off
/// the metal.
fn beside(c: &mut County<'_>, line: &[(i32, i32)], i: usize, def: jane_core::PropDefId, key: Key) -> bool {
    let Some(&(x, y)) = line.get(i) else { return false };
    for (dx, dy) in BESIDE {
        let (px, py) = (x + dx, y + dy);
        if c.k.solid(px, py) || c.k.is_claimed(px, py) {
            continue;
        }
        if matches!(c.k.get(px, py), Tile::Water | Tile::Road | Tile::Boardwalk) {
            continue;
        }
        c.k.prop(Some(key), def, px, py);
        return true;
    }
    false
}
