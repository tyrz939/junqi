//! Daytime threat for the terraced county (PLAN-map-r4 section 7): the plain county's layers
//! (`skeleton::build_threat`: a region's floor, the named patches, the dungeons' rings, the road's
//! discount, the havens) laid over districts instead of a river's distance, and the dungeons' rings
//! measured by walking the plan's gates rather than by the crow, so a ring does not leak through a
//! face into the district above it.

use jane_core::grid::Grid;

use super::macro_plan::{CELLS, MH, MW, MacroPlan, Role, idx};
use crate::skeleton::{DUNGEON_RING, MACRO, PlacedArea, PlacedSite, ROAD, Terrain, inside_in, metres_sq, mouth_phase};

/// A district's floor (0..=6): its region's, and more where the ground is a pocket or the School's
/// (the Hollow's one ramp, the lake's far shore, the crown).
pub const fn floor(role: Role) -> u8 {
    match role {
        Role::StationFields | Role::SallowBottom | Role::TopField | Role::CastleTerrace | Role::Foothills => 1,
        Role::MuseumTerrace | Role::LibraryTerrace => 2,
        Role::ButterflyHollow | Role::Lake => 3,
        Role::Lip | Role::Cutting | Role::Plateau | Role::Slagmere => 4,
        Role::Crown => 5,
    }
}

/// Daytime threat, 0..=6, on the macro grid: [`floor`] by district, the patches stamped over it,
/// the rings round the dungeons' mouths (one more, once, never above the phase of the dungeon's own
/// rooms), a road one less (never below one), and the havens round the sites that keep one at zero.
pub fn by_district(
    plan: &MacroPlan,
    t: &Terrain,
    sites: &[PlacedSite],
    areas: &[PlacedArea],
    road: &Grid<u8>,
) -> Grid<u8> {
    let (mw, mh) = (t.region.w() as i32, t.region.h() as i32);
    debug_assert_eq!((mw, mh), (MW, MH));
    let mut threat = Grid::new(mw as u32, mh as u32, 0u8);
    for y in 0..mh {
        for x in 0..mw {
            let d = &plan.districts[usize::from(plan.district_of[idx(x, y)])];
            threat.set(x, y, floor(d.role));
        }
    }
    let stamp = |threat: &mut Grid<u8>, mx: i32, my: i32, radius: i32, f: &dyn Fn(u8) -> u8| {
        let reach = (radius + MACRO - 1) / MACRO;
        let r2 = i64::from(radius) * i64::from(radius);
        for y in my - reach..=my + reach {
            for x in mx - reach..=mx + reach {
                if inside_in(mw, mh, x, y) && metres_sq(x, y, mx, my) <= r2 {
                    threat.set(x, y, f(threat.read(x, y, 0)));
                }
            }
        }
    };
    for a in areas {
        stamp(&mut threat, a.mx, a.my, i32::from(a.def.radius), &|_| a.def.threat);
    }
    // The rings: by the walk, over the plan's gates with the ledges taken as they hop.
    let (walk, _, _) = plan.graphs(true);
    let mut cap = Grid::new(mw as u32, mh as u32, 0u8);
    for s in sites.iter().filter(|s| s.def.dungeon) {
        let phase = mouth_phase(s.def.id).unwrap_or(6);
        let (dist, _) = plan.dijkstra(&walk, idx(s.mx, s.my), None);
        for (c, &d) in dist.iter().enumerate().take(CELLS) {
            if d <= DUNGEON_RING {
                let (x, y) = ((c as i32) % MW, (c as i32) / MW);
                let old = cap.read(x, y, 0);
                cap.set(x, y, if old == 0 { phase } else { old.min(phase) });
            }
        }
    }
    for (v, &c) in threat.as_mut_slice().iter_mut().zip(cap.as_slice()) {
        if c != 0 {
            *v = (*v + 1).min(c);
        }
    }
    // A road is the safer way, always: one less, never below one.
    for (v, &r) in threat.as_mut_slice().iter_mut().zip(road.as_slice()) {
        if r & ROAD != 0 {
            *v = v.saturating_sub(1).max(1);
        }
    }
    for s in sites {
        if let Some(hub) = s.def.hub {
            stamp(&mut threat, s.mx, s.my, i32::from(hub), &|_| 0);
        }
    }
    threat
}
