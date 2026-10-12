//! The carve (PLAN-map-r4 sections 1 to 6): the macro plan turned into levels on 4-cell blocks, the
//! river and the lake, joins, spans, the railway and the roads' waypoints, proven alone on a
//! tiles-only sketch. A pure function of the plan and a salt: integer only, `no_std` plus `alloc`.

pub mod blocks;
pub mod check;
pub mod joins;
pub mod level;
pub(crate) mod noise;
pub mod rail;
pub mod spans;
pub mod tiles;
pub mod water;
pub mod ways;

use alloc::vec::Vec;
use jane_core::blueprint::{Blueprint, Mark, Span};
use jane_core::grid::Cell;
use jane_core::{Plane, Rect, Tile, ZoneId};

use self::blocks::{Blocks, Pad};
use self::joins::{Join, JoinKind};
use self::spans::SpanMeta;
use self::water::WaterCut;
use self::ways::RoadWay;
use crate::county::terraced::macro_plan::{MH, MW, MacroPlan, STEP_CELLS};
use crate::solve::portals::{Ledge, Portals};

/// The county in cells.
pub const W: i32 = MW * STEP_CELLS;
/// Cells to a block.
pub const B: i32 = 4;
/// Blocks across.
pub const BW: i32 = W / B;
/// Rows of a level's south face (MAP.md 2.1).
pub const FACE_ROWS: i32 = 2;
/// Salts tried on a plan before the next plan.
pub const CARVE_SALTS: u8 = 4;

const _: () = assert!(MW == MH && W % B == 0);

/// Why a carve (or one of its stages) refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CarveFail {
    Shape(&'static str, (i32, i32)),
    Join(u8),
    Ledge(u8),
    Span(u8),
    River,
    Rail,
    Reach(&'static str),
    Pit((i32, i32)),
    Gates(u8),
}

/// The carved county: everything about the ground before the first tile.
#[derive(Clone, Debug)]
pub struct Carved {
    pub plan: MacroPlan,
    pub salt: u8,
    pub blocks: Blocks,
    pub level: Plane,
    pub pads: Vec<Pad>,
    pub water: WaterCut,
    pub joins: Vec<Join>,
    /// The plan's gates that found no place (a stair, ramp or ledge with no ground for it).
    pub missing: Vec<u8>,
    pub spans: Vec<Span>,
    pub span_meta: Vec<SpanMeta>,
    /// Flat plank bridges (no layer): `Boardwalk` over a channel with both banks on level ground.
    pub planks: Vec<Rect>,
    /// The railway's macro cells, from one end through the Halt to the other.
    pub rail: Vec<(i32, i32)>,
    pub ways: Vec<RoadWay>,
    /// The gates the night's barriers close (indices into `plan.gates`).
    pub night_gates: Vec<u8>,
}

impl PartialEq for Carved {
    fn eq(&self, o: &Self) -> bool {
        (self.plan.seed, self.plan.attempt, self.salt) == (o.plan.seed, o.plan.attempt, o.salt)
            && self.blocks == o.blocks
            && self.level == o.level
            && self.pads == o.pads
            && self.water == o.water
            && self.joins == o.joins
            && self.missing == o.missing
            && self.spans == o.spans
            && self.span_meta == o.span_meta
            && self.planks == o.planks
            && self.rail == o.rail
            && self.ways == o.ways
            && self.night_gates == o.night_gates
    }
}

impl Eq for Carved {}

/// What no salt can mend, found cheaply before any ground is laid: the main line has no way out of the
/// Lowfields to the map's edge or a terminus ([`rail::macro_rail`]), or the first walk crosses a river
/// on level-0 ground (its crossing would be boards at the floor, a change of level).
pub fn viable(plan: &MacroPlan, pads: &[Pad]) -> Result<Vec<(i32, i32)>, CarveFail> {
    use crate::county::terraced::macro_plan::{Wet, idx};
    let station = pads.iter().position(|p| p.site == "station").unwrap_or(0);
    let rail = rail::macro_rail(plan, pads, station)?;
    for (from, to) in [("station", "julie_house"), ("julie_house", "town")] {
        let Some(route) = plan.routes.iter().find(|r| r.from == from && r.to == to) else { continue };
        let low_river = route.pts.iter().any(|q| {
            plan.wet[idx(i32::from(q.0), i32::from(q.1))] == Wet::River
                && plan.level_at(i32::from(q.0), i32::from(q.1)) == 0
        });
        if low_river {
            return Err(CarveFail::River);
        }
    }
    Ok(rail)
}

/// The ground before the joins: the pads, the tidy blocks with the valley and levees, the water.
#[cfg(test)]
pub(crate) fn ground(plan: &MacroPlan, salt: u8) -> Result<(Vec<Pad>, Blocks, WaterCut), CarveFail> {
    let pads = blocks::pads(plan);
    let (b, water) = ground_on(plan, salt, &pads)?;
    Ok((pads, b, water))
}

fn ground_on(plan: &MacroPlan, salt: u8, pads: &[Pad]) -> Result<(Blocks, WaterCut), CarveFail> {
    let mut b = blocks::lay_blocks(plan, salt, pads);
    let (channel, _split, spring) = water::river_line(plan, salt, &b, pads);
    water::valley(plan, &mut b, &channel);
    blocks::levees(plan, &mut b);
    blocks::tidy(&mut b, plan, salt)?;
    let mut water = WaterCut::empty();
    water::lay_channel(&channel, &mut water.bits);
    water.channel = channel;
    water.source = spring;
    water::bodies(plan, salt, &b, &mut water);
    water::falls(&b, &mut water);
    Ok((b, water))
}

/// The carve of `plan` on `salt`, before its checks.
pub fn carve(plan: MacroPlan, salt: u8) -> Result<Carved, CarveFail> {
    let pads = blocks::pads(&plan);
    let rail = viable(&plan, &pads)?;
    carve_on(plan, salt, pads, rail)
}

fn carve_on(plan: MacroPlan, salt: u8, pads: Vec<Pad>, rail: Vec<(i32, i32)>) -> Result<Carved, CarveFail> {
    let (mut b, mut water) = ground_on(&plan, salt, &pads)?;
    let (mut joins, missing) = joins::lay_all(&plan, salt, &mut b, &water);
    let (spans, span_meta, planks) = spans::lay_spans(&plan, salt, &mut b, &mut water, &mut joins)?;
    let ways = ways::road_ways(&plan, &joins, &span_meta, &spans, &planks);
    let night_gates = ways::night_gates(&plan, &joins, &spans, &span_meta)?;
    let fixes: Vec<(Rect, u8)> = joins.iter().filter_map(|j| j.lower.map(|r| (r, j.lo))).collect();
    let level = level::pack_levels(&b, &fixes);
    Ok(Carved {
        plan,
        salt,
        blocks: b,
        level,
        pads,
        water,
        joins,
        missing: missing.iter().map(|m| m.0).collect(),
        spans,
        span_meta,
        planks,
        rail,
        ways,
        night_gates,
    })
}

/// A carve that fails a check is refused, as the first check it fails.
fn prove(c: Carved) -> Result<Carved, CarveFail> {
    match check::check_with(&c, true).first() {
        None => Ok(c),
        Some(i) => Err(match i.code {
            "shape" => CarveFail::Shape("checked", (0, 0)),
            "spans" => CarveFail::Span(u8::MAX),
            "water" => CarveFail::River,
            "gates" => CarveFail::Gates(0),
            "ledges" => CarveFail::Ledge(u8::MAX),
            "pit" => CarveFail::Pit((0, 0)),
            _ => CarveFail::Reach(i.code),
        }),
    }
}

/// [`carve`], and its checks: a carve that fails one is refused.
pub fn carve_proven(plan: MacroPlan, salt: u8) -> Result<Carved, CarveFail> {
    carve(plan, salt).and_then(prove)
}

/// What a seed's carve cost: plans tried (each a re-roll of the macro plan) and salts tried.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tries {
    pub plans: u32,
    pub salts: u32,
}

/// The carve of `seed`, with what it took: the first plan whose carve the checks accept, on one of
/// [`CARVE_SALTS`] salts, or the last refusal.
pub fn carve_seed_stats(seed: u32) -> (Result<Carved, CarveFail>, Tries) {
    carve_from(seed, 0, &mut |_| {})
}

/// [`carve_seed_stats`] from plan attempt `from` on (the county's re-roll asks for the plan after the
/// one it refused), saying `"plan"` and `"carve"` to `report` as each starts.
pub fn carve_from(seed: u32, from: u8, report: crate::Report<'_>) -> (Result<Carved, CarveFail>, Tries) {
    use crate::county::terraced::macro_plan::{ATTEMPTS, plan_from};
    let mut tries = Tries::default();
    let mut last = CarveFail::Shape("no plan", (0, 0));
    let mut from = from;
    while from < ATTEMPTS {
        report("plan");
        let Some(plan) = plan_from(seed, from) else { break };
        from = plan.attempt + 1;
        tries.plans += 1;
        // A plan no salt can mend is passed over before any ground is laid.
        report("carve");
        let pads = blocks::pads(&plan);
        let rail = match viable(&plan, &pads) {
            Ok(rail) => rail,
            Err(e) => {
                last = e;
                continue;
            }
        };
        for salt in 0..CARVE_SALTS {
            tries.salts += 1;
            match carve_on(plan.clone(), salt, pads.clone(), rail.clone()).and_then(prove) {
                Ok(c) => return (Ok(c), tries),
                Err(e) => last = e,
            }
        }
    }
    (Err(last), tries)
}

/// The carve of `seed`: [`carve_seed_stats`] without the count.
pub fn carve_seed(seed: u32) -> Result<Carved, CarveFail> {
    carve_seed_stats(seed).0
}

impl Carved {
    /// The level of cell `(x, y)`.
    pub fn level_at(&self, x: i32, y: i32) -> u8 {
        self.level.get(x as u32, y as u32)
    }

    /// The pad of the site on row `row` of the site table: its chunk's box and the ground kept
    /// round it.
    pub fn pad_of(&self, row: u8) -> Option<&Pad> {
        self.pads.iter().find(|p| p.row == row)
    }

    /// What the carve lays on row `y`: faces, rims, water, the joins' tiles, planks, made abutments,
    /// the arch's track. `out` is `W` long and cleared by the caller; cells nothing is laid on stay `None`.
    pub fn tile_row(&self, y: i32, out: &mut [Option<Tile>]) {
        tiles::face_row(&self.blocks, y, out);
        // Water: a body or the channel; where it crosses a face it falls.
        for k in 0..(W / 64) as usize {
            let mut word = self.water.bits.word((y as usize) * (W as usize / 64) + k);
            while word != 0 {
                let x = k * 64 + word.trailing_zeros() as usize;
                out[x] = Some(if out[x] == Some(Tile::Cliff) { Tile::Waterfall } else { Tile::Water });
                word &= word - 1;
            }
        }
        for j in &self.joins {
            if y >= j.rect.y && y < j.rect.bottom() && !matches!(j.kind, JoinKind::Closed(_)) {
                out[j.rect.x as usize..j.rect.right() as usize].fill(Some(j.tile));
            }
        }
        for r in &self.planks {
            if y >= r.y && y < r.bottom() {
                out[r.x as usize..r.right() as usize].fill(Some(Tile::Boardwalk));
            }
        }
        for m in &self.span_meta {
            let laid = m
                .abutments
                .iter()
                .map(|r| (r, Tile::Cliff))
                .chain(m.track.iter().map(|r| (r, Tile::Track)))
                .chain(m.buffers.iter().map(|r| (r, Tile::Rail)));
            for (r, t) in laid {
                if y >= r.y && y < r.bottom() {
                    out[r.x as usize..r.right() as usize].fill(Some(t));
                }
            }
        }
    }

    /// The carve alone as a county blueprint: grass, the carve's own tiles over it, its levels and spans,
    /// and the `start` mark at the Halt's pad. What the solver (and the sheets) read to prove it before
    /// anything is built on it.
    pub fn sketch(&self) -> Blueprint {
        let mut bp = Blueprint::new(ZoneId::County, W as u32, W as u32, Tile::Grass);
        let mut row: Vec<Option<Tile>> = alloc::vec![None; W as usize];
        for y in 0..W {
            row.fill(None);
            self.tile_row(y, &mut row);
            for (x, t) in row.iter().enumerate() {
                if let Some(t) = *t {
                    bp.tiles.set(x as i32, y, t);
                }
            }
        }
        bp.level = Some(self.level.clone());
        bp.spans.clone_from(&self.spans);
        if let Some(p) = self.pads.iter().find(|p| p.site == "station") {
            let (x, y) = p.boxr.centre();
            let key = bp.local("start");
            bp.marks.insert(key, Mark { cell: Cell::new(x as u16, y as u16), facing: None });
        }
        bp
    }

    /// The ledges and spans the solver crosses besides ground.
    pub fn portals(&self) -> Portals {
        let mut ledges = Vec::new();
        for j in self.joins.iter().filter(|j| j.kind == JoinKind::Ledge) {
            let r = j.rect;
            match j.tile {
                Tile::LedgeS => {
                    for x in r.x..r.right() {
                        ledges.push(Ledge { top: (x, r.y - 1), landing: (x, r.bottom()) });
                    }
                }
                Tile::LedgeE => {
                    for y in r.y..r.bottom() {
                        ledges.push(Ledge { top: (r.x - 1, y), landing: (r.right(), y) });
                    }
                }
                Tile::LedgeW => {
                    for y in r.y..r.bottom() {
                        ledges.push(Ledge { top: (r.right(), y), landing: (r.x - 1, y) });
                    }
                }
                _ => {}
            }
        }
        Portals { ledges, spans: self.spans.clone() }
    }

    /// The cells the night's barriers hold: each closed gate's join rects, and for a span gate the two
    /// ends of the deck (a barrier stands on a span's end).
    pub fn night_rects(&self) -> Vec<Rect> {
        let mut out = Vec::new();
        for &g in &self.night_gates {
            out.extend(self.joins.iter().filter(|j| j.gate == Some(g)).map(|j| j.rect));
            for (s, m) in self.spans.iter().zip(&self.span_meta) {
                if self.plan.crossings[usize::from(m.crossing)].gate == Some(g) {
                    out.extend(s.ends());
                }
            }
        }
        out
    }

    /// Every cell the carve holds for good: each join, its landings, each span with its ends and its
    /// approaches (the road's straight legs), each made abutment and the track. Faces are solid tiles and
    /// need no claim.
    pub fn claims(&self) -> Vec<Rect> {
        let mut out: Vec<Rect> = Vec::new();
        for j in &self.joins {
            out.extend(j.claims());
        }
        for (s, m) in self.spans.iter().zip(&self.span_meta) {
            out.push(s.rect);
            out.extend(s.ends());
            let [e0, e1] = m.ends;
            let r = s.rect;
            if s.along_x {
                out.push(Rect::new(e0.0, r.y, r.x - e0.0, r.h));
                out.push(Rect::new(r.right(), r.y, e1.0 - r.right() + 1, r.h));
            } else {
                out.push(Rect::new(r.x, e0.1, r.w, r.y - e0.1));
                out.push(Rect::new(r.x, r.bottom(), r.w, e1.1 - r.bottom() + 1));
            }
            out.extend(m.abutments.iter().copied());
            out.extend(m.track.iter().copied());
            out.extend(m.buffers.iter().copied());
        }
        out.extend(self.planks.iter().copied());
        out.extend(self.water.falls.iter().copied());
        out.retain(|r| r.w > 0 && r.h > 0);
        out
    }
}

#[cfg(test)]
mod tests;
