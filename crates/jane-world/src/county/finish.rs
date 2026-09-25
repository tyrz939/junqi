//! The county's last stages (`scatter`, `cutThrough`, `dropUnreachable` in `county.ts`): herbs and
//! rocks on what open ground is left, a way cut to any named place the wood closed round, and the
//! small places and creatures nobody can reach dropped.

use jane_core::action::Stack;
use jane_core::grid::DIRS4;
use jane_core::search::{Conn, Reach, flood};
use jane_core::{Key, Tile};

use super::County;
use crate::kit::Kit;
use crate::skeleton::Region;
use crate::steps::Step;

/// Herbs thrown for, placed or not.
const HERBS: u32 = 700;
/// Rocks thrown for, placed or not.
const ROCKS: u32 = 260;
/// Scatter keeps this far in from the county's edge.
const SCATTER_EDGE: i32 = 10;
const FIELD_HERBS: &[&str] = &["pansy", "nasturtium", "honeylace_lily", "hemshade_root"];
const WATER_HERBS: &[&str] = &["white_water_cap", "white_water_rose", "honeylace_lily", "night_lich_moss"];
const WORKS_HERBS: &[&str] = &["night_lich_moss", "savage_snakeroot", "hemshade_root"];
/// Cells a way cut out to the reached country may search: a few dozen cells of thicket, never a
/// search of half the county.
const CUT_BUDGET: u32 = 400_000;

/// Open, unclaimed ground that is neither water nor road.
fn open_ground(k: &Kit, x: i32, y: i32) -> bool {
    !k.is_claimed(x, y) && !k.solid(x, y) && !matches!(k.get(x, y), Tile::Water | Tile::Road)
}

/// Herbs, by region, and rocks, each on open unclaimed ground. The same throws for every herb,
/// placed or not ([`Step::CountyHerbs`]: x, y, which herb), so what stands in the way of one throw
/// decides that throw and never moves the next; the rocks likewise ([`Step::CountyRocks`]).
pub fn scatter(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let herb = cat.story.prop_id("herb").expect("a herb row");
    let rock = cat.story.prop_id("rock").expect("a rock row");
    let items = |names: &[&str]| -> Vec<jane_core::ItemId> {
        names.iter().map(|n| cat.combat.item_id(n).unwrap_or_else(|| panic!("no item {n}"))).collect()
    };
    let (field, water, works) = (items(FIELD_HERBS), items(WATER_HERBS), items(WORKS_HERBS));
    let (x1, y1) = (c.k.w() - SCATTER_EDGE - 1, c.k.h() - SCATTER_EDGE - 1);
    let mut rng = c.k.dice(Step::CountyHerbs, 0, 0);
    for _ in 0..HERBS {
        let x = rng.range(SCATTER_EDGE, x1);
        let y = rng.range(SCATTER_EDGE, y1);
        let pick = rng.next_u32();
        if !open_ground(&c.k, x, y) {
            continue;
        }
        let herbs = match c.sk.region_at(x >> 4, y >> 4) {
            Region::Lowfields => &field,
            Region::Waters => &water,
            Region::Works => &works,
        };
        let item = herbs[((u64::from(pick) * herbs.len() as u64) >> 32) as usize];
        c.k.prop(None, herb, x, y).loot = vec![Stack { item, qty: 1 }];
    }
    let mut rng = c.k.dice(Step::CountyRocks, 0, 0);
    for _ in 0..ROCKS {
        let x = rng.range(SCATTER_EDGE, x1);
        let y = rng.range(SCATTER_EDGE, y1);
        if open_ground(&c.k, x, y) {
            c.k.prop(None, rock, x, y);
        }
    }
}

/// Growth an axe clears, and rubble a barrow shifts; never water, a wall or a fence.
fn cuttable(t: Tile) -> bool {
    matches!(t, Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Hedge | Tile::Rubble)
}

/// Whether a mark is a rolled small place's (`poi_<n>`): those are dropped when cut off, not cut to.
fn rolled_poi(k: &Kit, key: Key) -> bool {
    k.local_name(key).is_some_and(|n| n.starts_with("poi_"))
}

/// The `start` mark's cell, if the county has one.
fn start(k: &Kit) -> Option<(i32, i32)> {
    let s = jane_data::catalog().name_id("start")?;
    k.blueprint().marks.get(&Key::Name(s)).map(|m| (i32::from(m.cell.x), i32::from(m.cell.y)))
}

/// Every cell a walker reaches from the platform, four ways over ground that does not stop feet,
/// by cell index (`y * w + x`); `None` for a county with no `start`.
fn from_start(k: &Kit) -> Option<Vec<bool>> {
    let s = start(k)?;
    let mut reach = Reach::new();
    flood(k.w() as u32, k.h() as u32, &[s], Conn::Four, u32::MAX, |x, y| !k.solid(x, y), &mut reach);
    let mut seen = vec![false; (k.w() * k.h()) as usize];
    for c in reach.order() {
        seen[c.0 as usize] = true;
    }
    Some(seen)
}

/// A named place (a story's ruin, a cottage the quests send her to) that the wood has closed round
/// is not lost: somebody cut a way to it. Flood from the platform; for each named mark the flood
/// never reached, in the order the marks were made, find the shortest way out to ground it did
/// reach, through trees, scrub and hedge and rubble but never water, a wall or a fence, and clear
/// that way to a trodden path. Then the same for every thing content named (a placement row's rat,
/// its parcel), units then props, in the order they were placed: the TypeScript cut only to marks,
/// and a rat spread into a field its hedges had closed was dropped as out of reach and the county
/// thrown away. No dice. What the flood reached at the end is left in `County::reached`.
pub fn cut_through(c: &mut County<'_>) {
    let k = &mut c.k;
    let (w, h) = (k.w(), k.h());
    let Some(mut seen) = from_start(k) else { return };
    let ix = |x: i32, y: i32| (y * w + x) as usize;
    let bp = k.blueprint();
    let at = |cell: jane_core::Cell| (i32::from(cell.x), i32::from(cell.y));
    let named = |key: Key| matches!(key, Key::Name(_));
    let targets: Vec<(i32, i32)> = bp
        .marks
        .iter()
        .filter(|(key, _)| !rolled_poi(k, **key))
        .map(|(_, m)| at(m.cell))
        .chain(bp.units.iter().filter(|u| named(u.key)).map(|u| at(u.cell)))
        .chain(bp.props.iter().filter(|p| named(p.key) && !p.hidden).map(|p| at(p.cell)))
        .collect();
    let mut out = Reach::new();
    for (mx, my) in targets {
        if !k.inside(mx, my) || seen[ix(mx, my)] || k.solid(mx, my) {
            continue;
        }
        // Breadth first out from it, over open ground and anything an axe can clear, to the
        // first cell of the reached country.
        let passable = |x: i32, y: i32| !k.solid(x, y) || cuttable(k.get(x, y));
        flood(w as u32, h as u32, &[(mx, my)], Conn::Four, CUT_BUDGET, passable, &mut out);
        let Some(found) = out
            .order()
            .iter()
            .map(|c| ((c.0 % w as u32) as i32, (c.0 / w as u32) as i32))
            .find(|&(x, y)| seen[ix(x, y)])
        else {
            continue;
        };
        // Back along the flood to it, a step nearer each time, clearing as it goes.
        let (mut x, mut y) = found;
        let mut d = out.dist(x, y);
        while d > 0 {
            if k.solid(x, y) {
                k.set(x, y, Tile::Dirt);
            }
            let Some(&(dx, dy)) = DIRS4.iter().find(|&&(dx, dy)| out.dist(x + dx, y + dy) == d - 1) else { break };
            (x, y, d) = (x + dx, y + dy, d - 1);
        }
        // Its ground is the reached country's now.
        flood(
            w as u32,
            h as u32,
            &[(mx, my)],
            Conn::Four,
            u32::MAX,
            |x, y| !k.solid(x, y) && !seen[ix(x, y)],
            &mut out,
        );
        for c in out.order() {
            seen[c.0 as usize] = true;
        }
    }
    c.reached = Some(seen);
}

/// A small place on an island of cliff or in a ring of water is not a place. Flood from the
/// platform, the way the solver will, and forget any rolled small place's mark (`poi_<n>`) the flood
/// never reached, and any creature (a unit with a phase) walled in where nobody can come. Story marks
/// are left for the solver to judge: if one of those is cut off the county is wrong and is re-rolled,
/// not quietly trimmed. Reads the flood `cut_through` left, if the ground has not changed since.
pub fn drop_unreachable(c: &mut County<'_>) {
    let Some(seen) = c.reached.take().or_else(|| from_start(&c.k)) else { return };
    let w = c.k.w();
    let reached = |cell: jane_core::Cell| seen[(i32::from(cell.y) * w + i32::from(cell.x)) as usize];
    let poi: Vec<Key> = c.k.blueprint().marks.keys().copied().filter(|&key| rolled_poi(&c.k, key)).collect();
    c.k.retain_marks(|key, m| !poi.contains(key) || reached(m.cell));
    c.k.retain_units(|u| u.phase == 0 || reached(u.cell));
}
