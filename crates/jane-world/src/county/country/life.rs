//! The living (`wildlife`, `wanderers`, `gaps` in `country.ts`): the ground's own creatures macro
//! cell by macro cell, thicker with the threat and kept back from the roads (the road is the safe
//! way because the country is kept off it, not because the country is empty); now and then
//! something walking a field edge beside a road; and last, anything on any screen still empty.

use jane_core::num::Permille;
use jane_core::view::{VIEW_H_CELLS, VIEW_W_CELLS};
use jane_core::{Grid, Rect, Sfc32, Tile, UnitDefId};
use jane_data::Region;

use super::defs::defs;
use super::{
    County, FIRST_CLEAR, ROAD_CLEAR, WORKS_BACK, clear, dist, folk, ground, hostile, near_chunk, put, road_clear, room,
    waypoint,
};
use crate::county::centre;
use crate::skeleton::{Biome, MACRO, SKEL_H, SKEL_W};
use crate::steps::Step;

/// A screen: what the camera shows at once, in cells (PORT.md §6.h).
pub const SCREEN_W: i32 = VIEW_W_CELLS as i32;
pub const SCREEN_H: i32 = VIEW_H_CELLS as i32;

/// The chance a macro cell away from the road has something in it, by threat, in permille. Threats
/// 2 and 3 went up by a tenth when the county came in to 2 km square; threat 1 is held so the
/// Works stay 1.6 times as thick as the Lowfields. *Tuning.*
pub const WILD: [i16; 7] = [0, 18, 29, 44, 90, 110, 130];
/// On ground of threat 3 or more, the chance of a pair, per threat, in permille. *Tuning.*
const COMPANY: i16 = 50;
/// Wanderers: the first this far along a road, then every 340 to 540 points; each walks 20 points
/// either side of its spot, 12 cells off the road. *Tuning.*
const WANDER_FROM: usize = 60;
const WANDER_EVERY: usize = 340;
const WANDER_JITTER: i32 = 200;
const WANDER_HALF: usize = 20;
const WANDER_OFF: i32 = 12;
/// A wanderer's dwell at each end of its beat, in ticks. *Tuning.*
const WANDER_DWELL: i32 = 240;
/// The night shift on the roads: the first this far along a road that is not the first walk, then
/// every 200 to 340 points; each walks 16 points either side of its spot, 12 cells off the line
/// (the field edge beside the road, as a wanderer walks). *Tuning.*
const NIGHT_FROM: usize = 40;
const NIGHT_EVERY: usize = 200;
const NIGHT_JITTER: i32 = 140;
const NIGHT_HALF: usize = 16;
const NIGHT_OFF: i32 = 12;
const NIGHT_DWELL: i32 = 180;
/// In the Works a wanderer's and the night shift's beat is this much further off the road (their
/// eye is longer than the field edge is wide), and nothing that cannot walk is put to walk one.
/// *Tuning.*
const WORKS_OFF: i32 = WORKS_BACK as i32;
/// The night shift on the rough ground: the chance a macro cell of each threat has one, permille.
/// Nothing under threat 2: the Lowfields' gentle ground is left to the road edges. Raised by
/// about half (2026-10-06: the owner's nights "considerably more threatening"). *Tuning.*
pub const NIGHT_WILD: [i16; 7] = [0, 0, 12, 38, 50, 62, 75];
/// Out of the Works, the chance one of the night shift is a black dog and not the bones, permille.
/// *Tuning.*
const NIGHT_HOUND: i16 = 350;
/// Cells past a warm light's lit reach (the sim's two thirds of its radius) that the night shift
/// is never put: it rises out of the dark, never in a lamp's or a fire's light.
const NIGHT_LIGHT_MARGIN: i32 = 3;

/// One kind of creature, and the biomes it keeps to (none: any).
type Wild = (fn(&super::defs::Units) -> UnitDefId, &'static [Biome]);

/// What a region's wild ground holds. *Tuning.*
fn table(region: Region) -> &'static [Wild] {
    use Biome::{Field, Foothill, Garden, Hedge, Hill, Marsh, Reed, Slag, WetWood, Wood, Yard};
    match region {
        Region::Lowfields => &[
            (|u| u.rat, &[]),
            (|u| u.skeleton, &[]),
            (|u| u.crow, &[Field, Hedge]),
            (|u| u.crow, &[Field, Hedge]),
            (|u| u.bat, &[]),
            (|u| u.pumpkin, &[Field, Hedge]),
            (|u| u.spider, &[Wood]),
            (|u| u.skeleton, &[Foothill, Wood]),
        ],
        Region::Waters => &[
            (|u| u.flower, &[Garden, Marsh]),
            (|u| u.statue, &[Garden]),
            (|u| u.spider, &[WetWood]),
            (|u| u.rat, &[Reed, Marsh]),
            (|u| u.crow, &[Reed, Garden]),
            (|u| u.bat, &[]),
            (|u| u.skeleton, &[]),
        ],
        Region::Works => &[
            (|u| u.soldier, &[]),
            (|u| u.skeleton_guard, &[]),
            (|u| u.skeleton_clerk, &[Yard]),
            (|u| u.wall_spider, &[Hill, Slag]),
            (|u| u.cactus, &[Slag]),
            (|u| u.crow, &[]),
        ],
    }
}

/// The bones about Julie's yard (the owner, 2026-09-30; WORLD.md §4 *Julie's yard*): this many
/// outside the fence, besides the two the chunk stands in the yard. *Tuning.*
const YARD_OUT: usize = 5;
/// They stand this far out from the yard's box at least, and within this of its middle (100 m of
/// the yard), in cells. *Tuning.*
const YARD_OFF: i32 = 6;
const YARD_REACH: i64 = 90;
/// Clear of the station road (the first walk to Julie's gate) by this, of any road's metal by this,
/// and of each other by this, in cells. *Tuning.*
const YARD_WALK_CLEAR: i64 = 16;
const YARD_ROAD_CLEAR: u8 = 8;
const YARD_APART: i64 = 8;
/// Throws at a spot before the yard makes do with fewer. *Tuning.*
const YARD_TRIES: u32 = 400;

/// A few of the yard's short-sighted bones (`yard_bones`, aggro 5 m) on open ground a little
/// outside Julie's fence, so the dog's quest always has one to hand and none comes running at her
/// on the step: clear of the station road she walks in on, off every road's metal, out of every
/// set place's box, never in water. Their phase is the row's (0: content, not the ground's).
pub fn yard_bones(c: &mut County<'_>) {
    let sk = c.sk;
    let Some(b) = c.chunks.iter().find(|ch| ch.site == sk.named.julie_house).map(|ch| ch.bounds) else { return };
    let (mx, my) = (b.x + b.w / 2, b.y + b.h / 2);
    let walk = sk.named.first_walk()[0];
    let station: Vec<(i32, i32)> = sk
        .roads
        .iter()
        .enumerate()
        .filter(|(_, r)| (r.from, r.to) == walk)
        .flat_map(|(n, _)| c.lines[n].iter().copied())
        .collect();
    let far = |(x, y): (i32, i32), pts: &[(i32, i32)], r: i64| {
        pts.iter().all(|&(px, py)| {
            let (dx, dy) = (i64::from(px - x), i64::from(py - y));
            dx * dx + dy * dy >= r * r
        })
    };
    let def = defs().u.yard_bones;
    let mut rng = c.k.dice(Step::CountyYard, 0, 0);
    let mut stood: Vec<(i32, i32)> = Vec::new();
    let reach = YARD_REACH as i32;
    for _ in 0..YARD_TRIES {
        if stood.len() >= YARD_OUT {
            break;
        }
        let (x, y) = (mx + rng.range(-reach, reach), my + rng.range(-reach, reach));
        let (dx, dy) = (i64::from(x - mx), i64::from(y - my));
        let ok = dx * dx + dy * dy <= YARD_REACH * YARD_REACH
            && !b.grow(YARD_OFF).contains(x, y)
            && !near_chunk(c, x, y, 2)
            && !c.k.solid(x, y)
            && !c.k.is_claimed(x, y)
            && c.k.get(x, y) != Tile::Water
            && !ground(sk, x, y).wet
            && dist(&c.country.d_road, x, y) >= YARD_ROAD_CLEAR
            && far((x, y), &station, YARD_WALK_CLEAR)
            && far((x, y), &stood, YARD_APART);
        if ok {
            c.k.unit(None, def, x, y, Vec::new());
            stood.push((x, y));
        }
    }
}

/// The creatures a macro cell of this region and biome may hold.
fn wild_here(region: Region, biome: Biome) -> Vec<UnitDefId> {
    let u = &defs().u;
    table(region).iter().filter(|(_, b)| b.is_empty() || b.contains(&biome)).map(|(f, _)| f(u)).collect()
}

/// The ground's own creatures, macro cell by macro cell, each on the cell's own dice. Nothing in a
/// haven, on water, within [`road_clear`] of a road (further in the Works) or [`FIRST_CLEAR`] of
/// the first walk.
pub fn wildlife(c: &mut County<'_>) {
    let sk = c.sk;
    for my in 1..SKEL_H - 1 {
        for mx in 1..SKEL_W - 1 {
            let threat = sk.threat.read(mx, my, 0);
            if threat == 0 || sk.terrain.water.read(mx, my, crate::skeleton::Water::Dry) != crate::skeleton::Water::Dry
            {
                continue;
            }
            let (cx, cy) = (centre(mx), centre(my));
            if dist(&c.country.d_road, cx, cy) < road_clear(sk.region_at(mx, my))
                || dist(&c.country.d_first, cx, cy) < FIRST_CLEAR
            {
                continue;
            }
            let mut rng = c.k.dice(Step::CountyWild, mx, my);
            wild_cell(c, &mut rng, mx, my, threat);
        }
    }
}

/// One macro cell's creature, or pair.
fn wild_cell(c: &mut County<'_>, rng: &mut Sfc32, mx: i32, my: i32, threat: u8) {
    if !rng.chance(Permille(WILD[usize::from(threat.min(6))])) {
        return;
    }
    let (region, biome) = (c.sk.region_at(mx, my), c.sk.terrain.biome.read(mx, my, Biome::Field));
    let here = wild_here(region, biome);
    if here.is_empty() {
        return;
    }
    // Company, where the ground is bad: a pair is a reason to go another way.
    let company = if threat >= 3 && rng.chance(Permille(i16::from(threat) * COMPANY)) { 2 } else { 1 };
    let def = *rng.pick(&here).expect("not empty");
    for _ in 0..company {
        let Some((x, y)) = c.k.spot(rng, Rect::new(mx * MACRO, my * MACRO, MACRO, MACRO), 1, 1, 1, 8) else { break };
        if dist(&c.country.d_road, x, y) < road_clear(region) {
            break;
        }
        hostile(c, def, x, y, Vec::new());
    }
}

/// Now and then something walks the field edge beside a road, up and back: the one thing on a road
/// that might come at her by day. Never on the first walk, never at a town's gate. Each road on its
/// own dice.
pub fn wanderers(c: &mut County<'_>) {
    for n in 0..c.sk.roads.len() {
        if c.country.first_lines[n] {
            continue;
        }
        let line = c.lines[n].clone();
        let mut rng = c.k.dice(Step::CountyWander, super::places::road_key(c.sk, n), 0);
        let mut i = WANDER_FROM;
        while i + WANDER_FROM < line.len() {
            wander(c, &mut rng, &line, i);
            i += WANDER_EVERY + rng.irandom(WANDER_JITTER) as usize;
        }
    }
}

/// One wanderer at point `i` of a road, if the ground there has something to walk it.
fn wander(c: &mut County<'_>, rng: &mut Sfc32, line: &[(i32, i32)], i: usize) {
    let (x, y) = line[i];
    if near_chunk(c, x, y, 40) || dist(&c.country.d_first, x, y) < FIRST_CLEAR + 20 {
        return;
    }
    let g = ground(c.sk, x, y);
    let works = g.region == Region::Works;
    let mut pool = wild_here(g.region, g.biome);
    if works {
        let cat = jane_data::catalog();
        pool.retain(|&u| cat.combat.unit(u).walk.0 > 0);
    }
    let Some(&def) = rng.pick(&pool) else { return };
    let side = if rng.chance(Permille(500)) { 1 } else { -1 };
    let off = if works { WANDER_OFF + WORKS_OFF } else { WANDER_OFF };
    let nrm = super::Normal::of(line, i);
    let a = line[i.saturating_sub(WANDER_HALF)];
    let b = line[(i + WANDER_HALF).min(line.len() - 1)];
    let p0 = nrm.cells(a, off * side);
    let p1 = nrm.cells(b, off * side);
    if c.k.solid(p0.0, p0.1) || c.k.solid(p1.0, p1.1) {
        return;
    }
    let beat = vec![waypoint(p0.0, p0.1, WANDER_DWELL), waypoint(p1.0, p1.1, WANDER_DWELL)];
    hostile(c, def, p0.0, p0.1, beat);
}

/// After the bell (WORLD.md §4.3): the night shift. On a road that is not the first walk, every
/// [`NIGHT_EVERY`] points or so, one stands at the edge of the road and walks it a little either
/// way; on the rough ground (threat 3 and over) a macro cell has one at [`NIGHT_WILD`] permille.
/// They are `nightOnly` rows (presence keeps them out of the world from six to nine) and they shun
/// warm light, so a lamp is the edge of where they go. They are the ground's own, placed as the
/// day's are: never in a haven, never near the first walk, never at a set place's gate, at the
/// threat of the ground they stand on. Each road and each macro cell throws its own dice.
pub fn night_shift(c: &mut County<'_>) {
    let lights = warm_lights(c);
    for n in 0..c.sk.roads.len() {
        if c.country.first_lines[n] {
            continue;
        }
        let line = c.lines[n].clone();
        let mut rng = c.k.dice(Step::CountyNight, super::places::road_key(c.sk, n), -1);
        let mut i = NIGHT_FROM + rng.irandom(NIGHT_JITTER) as usize;
        while i + NIGHT_FROM < line.len() {
            night_edge(c, &mut rng, &line, i, &lights);
            i += NIGHT_EVERY + rng.irandom(NIGHT_JITTER) as usize;
        }
    }
    let sk = c.sk;
    for my in 1..SKEL_H - 1 {
        for mx in 1..SKEL_W - 1 {
            let threat = sk.threat.read(mx, my, 0);
            let chance = NIGHT_WILD[usize::from(threat.min(6))];
            if chance == 0 || sk.terrain.water.read(mx, my, crate::skeleton::Water::Dry) != crate::skeleton::Water::Dry
            {
                continue;
            }
            let mut rng = c.k.dice(Step::CountyNight, -1, my * SKEL_W + mx);
            if !rng.chance(Permille(chance)) {
                continue;
            }
            let Some((x, y)) = c.k.spot(&mut rng, Rect::new(mx * MACRO, my * MACRO, MACRO, MACRO), 1, 1, 1, 8) else {
                continue;
            };
            if near_chunk(c, x, y, 40) || in_warm_light(&lights, x, y) {
                continue;
            }
            let def = night_def(c, &mut rng, x, y);
            hostile(c, def, x, y, Vec::new());
        }
    }
}

/// The night's row for the ground under `(x, y)`: the Works' own in the Works, else the bones or,
/// [`NIGHT_HOUND`] in a thousand, a black dog.
fn night_def(c: &County<'_>, rng: &mut Sfc32, x: i32, y: i32) -> UnitDefId {
    let u = &defs().u;
    if ground(c.sk, x, y).region == Region::Works {
        u.night_soldier
    } else if rng.chance(Permille(NIGHT_HOUND)) {
        u.night_hound
    } else {
        u.night_skeleton
    }
}

/// The warm lights already standing (lamps, fires, lit windows), lit or not yet: `(x, y, r²)` in
/// cells, `r` the sim's lit reach (two thirds of the radius) and [`NIGHT_LIGHT_MARGIN`] more.
fn warm_lights(c: &County<'_>) -> Vec<(i32, i32, i32)> {
    let cat = jane_data::catalog();
    c.k.blueprint()
        .props
        .iter()
        .filter_map(|p| {
            let d = cat.story.prop(p.def);
            let l = d.light.as_ref().filter(|l| !l.cold)?;
            let r = l.radius.0 * 2 / 3 / jane_core::num::CELL_FX + NIGHT_LIGHT_MARGIN;
            Some((i32::from(p.cell.x) + i32::from(d.w) / 2, i32::from(p.cell.y) + i32::from(d.h) / 2, r * r))
        })
        .collect()
}

/// Is the cell inside one of [`warm_lights`]?
fn in_warm_light(lights: &[(i32, i32, i32)], x: i32, y: i32) -> bool {
    lights.iter().any(|&(lx, ly, r2)| (lx - x) * (lx - x) + (ly - y) * (ly - y) <= r2)
}

/// One of the night shift at point `i` of a road: at the road's edge, walking a stretch of it.
fn night_edge(c: &mut County<'_>, rng: &mut Sfc32, line: &[(i32, i32)], i: usize, lights: &[(i32, i32, i32)]) {
    let (x, y) = line[i];
    if near_chunk(c, x, y, 40) || dist(&c.country.d_first, x, y) < FIRST_CLEAR + 20 {
        return;
    }
    let side = if rng.chance(Permille(500)) { 1 } else { -1 };
    let off = if ground(c.sk, x, y).region == Region::Works { NIGHT_OFF + WORKS_OFF } else { NIGHT_OFF };
    let nrm = super::Normal::of(line, i);
    let a = line[i.saturating_sub(NIGHT_HALF)];
    let b = line[(i + NIGHT_HALF).min(line.len() - 1)];
    let p0 = nrm.cells(a, off * side);
    let p1 = nrm.cells(b, off * side);
    if c.k.solid(p0.0, p0.1) || c.k.solid(p1.0, p1.1) {
        return;
    }
    // Its beat stays out of the light at both ends (a shade walking into a lamp only turns back).
    if in_warm_light(lights, p0.0, p0.1) || in_warm_light(lights, p1.0, p1.1) {
        return;
    }
    let def = night_def(c, rng, p0.0, p0.1);
    let beat = vec![waypoint(p0.0, p0.1, NIGHT_DWELL), waypoint(p1.0, p1.1, NIGHT_DWELL)];
    hostile(c, def, p0.0, p0.1, beat);
}

/// The last pass: any screen of open land that still has nothing on it (no unit, no prop but a
/// herb, a rock or a lamp) gets something small of its own, on the screen's own dice. By a road,
/// flowers or a fallen log; out in the fields, a thing that lives there.
pub fn gaps(c: &mut County<'_>) {
    let (gw, gh) = (c.k.w() / SCREEN_W, c.k.h() / SCREEN_H);
    let mut has = Grid::new(gw as u32, gh as u32, false);
    let d = defs();
    let east = jane_data::catalog().county.furnishing.east_road;
    let east_lamps = east.map_or([d.p.lamp_post; 2], |e| [e.lamp, e.bridge_lamp]);
    let minor = [d.p.herb, d.p.rock, d.p.lamp_post, d.p.lamp_run, east_lamps[0], east_lamps[1]];
    let bp = c.k.blueprint();
    let cells =
        bp.props.iter().filter(|p| !minor.contains(&p.def)).map(|p| p.cell).chain(bp.units.iter().map(|u| u.cell));
    for cell in cells {
        has.set(i32::from(cell.x) / SCREEN_W, i32::from(cell.y) / SCREEN_H, true);
    }
    for sy in 0..gh {
        for sx in 0..gw {
            if has.read(sx, sy, true) {
                continue;
            }
            let mut rng = c.k.dice(Step::CountyGap, sx, sy);
            gap(c, &mut rng, sx, sy);
        }
    }
}

/// One empty screen's something small.
fn gap(c: &mut County<'_>, rng: &mut Sfc32, sx: i32, sy: i32) {
    let d = defs();
    let r = Rect::new(sx * SCREEN_W + 4, sy * SCREEN_H + 3, SCREEN_W - 8, SCREEN_H - 6);
    let mut spot = c.k.spot(rng, r, 3, 3, 1, 30);
    if spot.is_none() {
        // A screen of thick wood or scrub: open a small clearing in it, off anything claimed.
        for _ in 0..40 {
            let x = rng.range(r.x, r.x + r.w - 4);
            let y = rng.range(r.y, r.y + r.h - 4);
            if room(c, x - 1, y - 1, 5, 5) {
                clear(c, Rect::new(x - 1, y - 1, 5, 5), Tile::GrassTall);
                spot = Some((x, y));
                break;
            }
        }
    }
    let Some((x, y)) = spot else {
        // Not even room for a clearing (a screen of crag and road): flowers on any open cell. The
        // TypeScript left such a screen empty.
        if let Some((x, y)) = c.k.spot(rng, r, 1, 1, 0, 60) {
            if !near_chunk(c, x, y, 2) {
                put(c, d.p.flowers, x, y);
            }
        }
        return;
    };
    let (x, y) = (x + 1, y + 1);
    if near_chunk(c, x, y, 2) {
        return;
    }
    let g = ground(c.sk, x, y);
    // Gentle ground: beside a road, in a haven, or within sight of the first walk.
    let gentle =
        dist(&c.country.d_road, x, y) < ROAD_CLEAR || g.threat == 0 || dist(&c.country.d_first, x, y) < FIRST_CLEAR + 8;
    let roll = rng.irandom(3);
    if g.region == Region::Works {
        let def = [d.p.sleepers, d.p.gravestone, d.p.bones, d.p.boulder][roll as usize];
        put(c, def, x, y);
    } else if gentle {
        match roll {
            0 => {
                put(c, d.p.log, x, y);
            }
            1 => {
                put(c, d.p.boulder, x, y);
            }
            2 => {
                put(c, d.p.stump, x, y);
                put(c, d.p.flowers, x + 1, y + 1);
            }
            _ => {
                for _ in 0..4 {
                    let fx = x - 2 + rng.irandom(4);
                    let fy = y - 1 + rng.irandom(2);
                    put(c, d.p.flowers, fx, fy);
                }
            }
        }
    } else {
        match roll {
            0 => {
                for n in 0..2 {
                    folk(c, rng, d.u.rabbit, x + n * 2, y, 4);
                }
            }
            1 => {
                let def = if g.region == Region::Lowfields { d.u.crow } else { d.u.rat };
                if !hostile(c, def, x, y, Vec::new()) {
                    put(c, d.p.boulder, x, y);
                }
            }
            2 => {
                put(c, d.p.boulder, x, y);
            }
            _ => {
                put(c, d.p.stump, x, y);
                put(c, d.p.log, x + 2, y + 1);
            }
        }
    }
}
