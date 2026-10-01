//! No one-shots near the start (PLAY-PLAN.md 0.3): within 400 m of the Halt (where New Game
//! stands her) and of Julie's gate, no foe's single cast or volley can take 35% or more of New
//! Game health. The overnight audit found a cactus volley of ten needles at up to 52% each, and
//! the Explorer died 44 times at one cell beside the first walk on seed 3.
//!
//! "Can" is the worst a player could read off the row: every blow of the cast at the top of its
//! roll (every bolt of a volley landing: a cactus is met at arm's length) and all of any poison it
//! leaves; a ground pool counts its first pulse. Crits (one blow in twenty, doubled) are left out,
//! as `fight::max_hit` leaves them out; the table printed shows the worst with one crit beside it. The foes are every one the
//! county's blueprint stands there, at the threat of the ground they stand on (`PHASE_SCALE`).
//! What the living world sends out at night is not here: it walks in from the dark, it is not
//! stood on the field beside the path.

use jane_core::action::{School, Stat};
use jane_core::{Key, ZoneId};
use jane_sim::tuning::{HP_PER_STRENGTH, PHASE_SCALE};

/// Metres (one a cell) about the Halt and Julie's gate.
const NEAR: i64 = 400;
/// The share of New Game health no one cast near the start may take, per cent.
const CAP_PCT: i64 = 35;

/// The worst one cast of `spell` can do, in milli-points, from a caster of `strength` and
/// `spirit`: (every blow at its top, the same with one of them a crit).
fn worst(spell: jane_core::SpellId, strength: i64, spirit: i64) -> (i64, i64) {
    let cat = jane_data::catalog();
    let s = cat.combat.spell(spell);
    let Some(p) = s.power else { return (0, 0) };
    let stat = match p.stat {
        Stat::Strength => strength,
        Stat::Spirit => spirit,
    };
    // `combat::roll_power` at its top: the fixed part, the random part's whole points, the flat.
    let top = stat * 1_000_000 / i64::from(p.div.max(1))
        + stat * 1000 / i64::from(p.var_div.max(1)) * 1000
        + i64::from(p.flat.0);
    let top = (top + 500) / 1000 * 1000;
    let blows = i64::from(s.count.max(1));
    let dot = s.effect.map_or(0, |e| {
        let e = cat.combat.effect(e);
        match e.pulse {
            Some(pl) if e.harmful && pl.school != School::Heal => {
                i64::from(pl.amount.0) * i64::from(e.duration.0 / pl.every.0.max(1))
            }
            _ => 0,
        }
    });
    let all = top * blows + dot * blows;
    (all, all + top)
}

#[test]
fn nothing_near_the_halt_takes_a_third_of_new_game_health_at_once() {
    let cat = jane_data::catalog();
    let jane = cat.combat.unit(cat.combat.unit_id("jane").expect("her row"));
    let max_hp = i64::from(jane.strength) * i64::from(HP_PER_STRENGTH) * 1000;
    let yard = cat.combat.unit_id("yard_bones").expect("the yard's row");
    // The seeds the gen fixture holds (a county is a third of a second in release).
    let seeds = 1..=16u32;
    let mut bad = Vec::new();
    let mut seen = std::collections::BTreeMap::<&str, (i64, i64, u8, &str)>::new();
    for seed in seeds {
        let bp = jane_sim::blueprints::build_one(ZoneId::County, seed).expect("the county builds");
        let halt = bp.marks.get(&Key::Name(cat.story.start.mark)).expect("the start mark").cell;
        // Julie's gate: the middle of her yard's bones (two in the yard, five by the fence).
        let bones: Vec<_> = bp.units.iter().filter(|u| u.def == yard).map(|u| u.cell).collect();
        assert!(!bones.is_empty(), "seed {seed}: the yard's bones");
        let n = bones.len() as i32;
        let gate = (
            bones.iter().map(|c| i32::from(c.x)).sum::<i32>() / n,
            bones.iter().map(|c| i32::from(c.y)).sum::<i32>() / n,
        );
        let points = [("the Halt", (i32::from(halt.x), i32::from(halt.y))), ("Julie's gate", gate)];
        for u in &bp.units {
            let def = cat.combat.unit(u.def);
            if def.faction == jane_data::Faction::Friendly || def.boss {
                continue;
            }
            let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
            let Some(&(from, _)) = points.iter().find(|(_, (px, py))| {
                let (dx, dy) = (i64::from(x - px), i64::from(y - py));
                dx * dx + dy * dy <= NEAR * NEAR
            }) else {
                continue;
            };
            let m =
                i64::from(if u.phase > 1 { PHASE_SCALE[usize::from(u.phase).min(PHASE_SCALE.len() - 1)] } else { 1 });
            let (str_, spi) = (i64::from(def.strength) * m, i64::from(def.spirit) * m);
            let books = std::iter::once(def.book).chain(def.phases.iter().map(|p| p.book));
            for &s in books.flat_map(|b| b.iter()) {
                let (w, crit) = worst(s, str_, spi);
                let id = cat.combat.spell(s).id;
                let e = seen.entry(def.id).or_insert((0, 0, 0, id));
                if w > e.0 {
                    *e = (w, crit, u.phase, id);
                }
                if w * 100 >= CAP_PCT * max_hp {
                    bad.push(format!(
                        "seed {seed}: {} (phase {}) at ({x}, {y}) near {from}: {} up to {} of {} ({}%)",
                        def.id,
                        u.phase,
                        id,
                        w / 1000,
                        max_hp / 1000,
                        w * 100 / max_hp
                    ));
                }
            }
        }
    }
    for (who, (w, crit, phase, s)) in &seen {
        println!(
            "{who:<20} phase {phase} {s:<14} worst cast {:>3} ({:>2}% of New Game health; {:>2}% with a crit)",
            w / 1000,
            w * 100 / max_hp,
            crit * 100 / max_hp
        );
    }
    assert!(bad.is_empty(), "{} casts near the start over {CAP_PCT}%:\n{}", bad.len(), bad.join("\n"));
}
