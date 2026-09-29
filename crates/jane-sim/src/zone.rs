//! A zone's state, made from its blueprint on the first visit and kept for ever
//! (`sim/zones.ts createZoneState`).

use std::collections::BTreeMap;

use jane_core::blueprint::{PropSpawn, UnitSpawn};
use jane_core::hash::fnv1a;
use jane_core::{Blueprint, CellIx, Sfc32, Sym, Tick, Tile, Vec2};

use crate::ids::{PropId, PropIx, UnitId};
use crate::runtime::{FogGeom, ZoneRuntime, intern_locals, zone_triggers};
use crate::state::{Bits, GameState, LootState, Prop, SpawnBase, TriggerBits, Unit, ZoneState};
use crate::sym::of_key;
use crate::tuning::PHASE_SCALE;
use crate::units::{max_hp, max_mp, new_unit, patrol_of};

/// A zone's own dice: `seeded(seed ^ fnv1a(zone), 1)` (ARCHITECTURE.md §4.4).
pub fn zone_rng(seed: u32, bp: &Blueprint) -> Sfc32 {
    Sfc32::seeded(seed ^ fnv1a(bp.zone.name().as_bytes()), 1)
}

/// A zone with nothing in it: the blueprint's size, its fog and its dice.
pub fn empty_zone_state(bp: &Blueprint, seed: u32) -> ZoneState {
    ZoneState {
        id: bp.zone,
        rng: zone_rng(seed, bp),
        units: Vec::new(),
        props: Vec::new(),
        drops: Vec::new(),
        projectiles: Vec::new(),
        grounds: Vec::new(),
        triggers: TriggerBits::default(),
        tile_deltas: BTreeMap::new(),
        fog: vec![0; FogGeom::of(bp).words() as usize].into_boxed_slice(),
        pending_fill: Vec::new(),
        sleeping_due: Vec::new(),
        wetness: [0; crate::state::REGIONS],
        pressure: vec![0; bp.areas.len()],
        ring_key: None,
        spawned: SpawnBase::default(),
    }
}

/// The zone as the blueprint builds it: its locals interned, every unit and prop numbered from
/// the world's counters in blueprint order, one trigger bit per row of the merged table.
pub fn create_zone_state(state: &mut GameState, bp: &Blueprint) -> ZoneState {
    let locals = intern_locals(&mut state.syms, bp);
    let mut z = empty_zone_state(bp, state.seed);
    z.spawned = SpawnBase { unit: state.next.unit, prop: state.next.prop, tick: state.tick };
    for s in &bp.units {
        let id = state.next.unit();
        z.units.push(spawn_unit(s, id, of_key(s.key, &locals), state.tick));
    }
    for (i, s) in bp.props.iter().enumerate() {
        z.props.push(spawn_prop(s, i as u16, state.next.prop(), of_key(s.key, &locals)));
    }
    let n = zone_triggers(bp, &locals).len() as u32;
    z.triggers = TriggerBits { fired: Bits::new(n), inside: Bits::new(n) };
    z
}

/// A blueprint unit row as it is first made, with its id and key. The save rebuilds an
/// untouched spawn with this, so it is the only place a spawned unit is made.
pub fn spawn_unit(s: &UnitSpawn, id: UnitId, key: Sym, now: Tick) -> Unit {
    let pos = Vec2::centre(i32::from(s.cell.x), i32::from(s.cell.y));
    let mut u = new_unit(id, Some(key), s.def, pos, s.facing.unwrap_or_default(), now);
    if s.phase > 1 {
        // Strength is health and melee; spirit is mana and spell power. Both follow the phase.
        let m = u16::from(PHASE_SCALE[usize::from(s.phase).min(PHASE_SCALE.len() - 1)]);
        u.strength *= m;
        u.spirit *= m;
        u.hp = max_hp(&u);
        u.mp = max_mp(&u);
    }
    u.patrol = patrol_of(&s.patrol);
    u
}

/// Blueprint prop row `i` as it is first made, with its id and key (as [`spawn_unit`]).
pub fn spawn_prop(s: &PropSpawn, i: u16, id: PropId, key: Sym) -> Prop {
    let def = jane_data::catalog().story.prop(s.def);
    Prop {
        id,
        key,
        def: s.def,
        spawn: Some(i),
        cell: s.cell,
        // A gate is a door that leads nowhere: shut while locked, open otherwise.
        solid: if def.gate { s.locked } else { def.solid },
        hidden: s.hidden,
        locked: s.locked,
        used: false,
        on: s.on,
        loot: LootState::AsSpawned,
        under_done: false,
        regrow: None,
        night: crate::state::NightState::AsSpawned,
    }
}

/// Change a tile in a live zone, keeping the deltas compact: a tile set back to the
/// blueprint's is no longer a delta.
pub fn set_tile(zone: &mut ZoneState, rt: &mut ZoneRuntime, bp: &Blueprint, x: i32, y: i32, t: Tile) {
    if !rt.grid.inside(x, y) {
        return;
    }
    rt.grid.set_tile(x, y, t);
    let i = CellIx(y as u32 * bp.w() + x as u32);
    if *bp.tiles.at(i) == t {
        zone.tile_deltas.remove(&i);
    } else {
        zone.tile_deltas.insert(i, t);
    }
}

/// A prop by key in a live zone.
pub fn prop_by_key(rt: &ZoneRuntime, key: jane_core::Sym) -> Option<PropIx> {
    rt.names.get(&key).copied()
}

/// Full hp and mp for a unit's current stats.
pub fn heal_full(u: &mut crate::state::Unit) {
    u.hp = max_hp(u);
    u.mp = max_mp(u);
}
