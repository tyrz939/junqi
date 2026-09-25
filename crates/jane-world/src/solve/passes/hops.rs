//! Hops: a `to` that names this zone is a one-way edge to its mark (the factory's vent, the
//! mine's way into the arena). The mark becomes a place the next flood starts from, in the layer
//! she was in when she took it.

use super::Changed;
use crate::solve::model::Solve;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    for i in 0..s.bp.props.len() {
        if s.props[i].withheld {
            continue;
        }
        let Some(door) = s.bp.props[i].to.filter(|d| d.zone == s.bp.zone) else { continue };
        let Some(at) = s.at(i) else { continue };
        if s.held_shut(i, at) || s.hops.contains(&(door.mark, at)) {
            continue;
        }
        s.hops.push((door.mark, at));
        s.mark_fired(i);
        s.opened = true;
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;
    use jane_core::blueprint::Door;

    use super::*;
    use crate::solve::sketch::Sketch;

    #[test]
    fn a_door_into_this_zone_is_a_one_way_edge_to_its_mark() {
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#..#..#", "#######"]);
        k.mark("start", 1, 1);
        let far = k.mark("far", 5, 1);
        let vent = k.prop("vent", "lever", 2, 1);
        k.bp.props[vent].to = Some(Door { zone: ZoneId::Arms, mark: far });
        // A door to another zone is travel, not an edge.
        let out = k.prop("out", "lever", 1, 2);
        k.bp.props[out].to = Some(Door { zone: ZoneId::County, mark: far });
        k.with(|s| {
            s.flood_all();
            assert!(!s.layers.seen_any(5, 1));
            assert!(run(s));
            assert_eq!(s.hops, vec![(far, 0)]);
            assert!(!run(s), "taken once per layer");
            s.flood_all();
            assert!(s.layers.seen_any(5, 1) && s.layers.seen_any(4, 1));
        });
    }

    #[test]
    fn a_locked_door_is_no_edge() {
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#..#..#", "#######"]);
        k.mark("start", 1, 1);
        let far = k.mark("far", 5, 1);
        let vent = k.prop("vent", "lever", 2, 1);
        k.bp.props[vent].to = Some(Door { zone: ZoneId::Arms, mark: far });
        k.bp.props[vent].locked = true;
        k.with(|s| {
            s.flood_all();
            assert!(!run(s));
        });
    }
}
