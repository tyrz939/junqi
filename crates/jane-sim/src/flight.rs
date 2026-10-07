//! Step 8: what is in the air and on the ground (`combat.ts stepProjectiles, stepGrounds`).
//!
//! **Bolts fly straight**, unless a seat cast one at her target (PLAY-PLAN §2.1). 2020 aimed once
//! and never homed, which is what makes a slow poison bolt dodgeable; a seat's bolt cast at a
//! unit turns toward it at most `SEEK_TURN` a tick (so a moving foe is hittable and a wall
//! still is a wall), and one cast at a prop ends at its middle. Each tick a bolt moves by its
//! velocity; it dies on the first cell along
//! the move that blocks a shot (walls, sight-blocking and solid props), on the first enemy body
//! it ends inside (lowest id), or when its range runs out. Where it dies it splashes (the blow
//! over `div`, whole points, on every other enemy near), touches the props that answer its school
//! (the interact unit's `school_touch`), and says so. Bolts step in the order they were cast.
//!
//! **Pools pulse.** A ground spell pulses every `pulse` ticks from the tick after it is cast, on
//! every present unit of another side inside its radius (by faction, not by enmity: a beast's
//! web catches the undead too), rolling the caster's power per victim, until it runs out.

use alloc::vec::Vec;

use jane_core::angle::{along, bearing};
use jane_core::num::{CELL_FX, dist_sq, within};
use jane_core::tile::BLOCK_SHOT;
use jane_core::{Fx, Milli};
use jane_data::Controller;

use crate::combat::{Hit, body_dist_sq, is_enemy, max_bounds, query_near, roll_power, round_points};
use crate::ctx::Ctx;
use crate::event::EventKind;
use crate::los::first_blocked_cell;
use crate::state::Seek;
use crate::tuning::{SCHOOL_TOUCH_FX, SEEK_TURN};

/// Step 8, bolts.
pub fn step_projectiles(cx: &mut Ctx<'_>) {
    let zi = cx.zone.id.index();
    let mut near = core::mem::take(&mut cx.scratch.near);
    let mut i = 0;
    while i < cx.zone.projectiles.len() {
        let spell = cx.cat.combat.spell(cx.zone.projectiles[i].spell);
        let speed = spell.speed.map_or(512, |s| s.0);
        // A bolt cast at a unit turns toward it, at most `SEEK_TURN` a tick, while it lives.
        if let Seek::Unit(t) = cx.zone.projectiles[i].seek {
            let at = cx.zone.unit(t).filter(|u| u.alive && !u.hidden).map(|u| u.pos);
            let p = &mut cx.zone.projectiles[i];
            match at {
                Some(at) if at != p.pos => {
                    let turn = p.heading.diff(bearing(p.pos, at)).clamp(-SEEK_TURN, SEEK_TURN);
                    p.heading = p.heading.wrapping_add(turn);
                    p.vel = along(p.heading, Fx(speed));
                }
                Some(_) => {}
                None => p.seek = Seek::None,
            }
        }
        let p = &mut cx.zone.projectiles[i];
        let mut to = p.pos + p.vel;
        // A bolt cast at a prop ends at its middle.
        let mut ended = false;
        if let Seek::Point(at) = p.seek {
            if dist_sq(p.pos, at) <= i64::from(speed) * i64::from(speed) {
                to = at;
                ended = true;
            }
        }
        let mut dead = ended || first_blocked_cell(&cx.rt.grid, p.pos, to, BLOCK_SHOT).is_some();
        p.pos = to;
        p.left = Fx(p.left.0 - speed);
        let p = *p;
        let mut victim = None;
        if !dead {
            query_near(cx.rt, p.pos, i64::from(max_bounds()), &mut near);
            for &id in &near {
                let Some(u) = cx.zone.unit(id) else { continue };
                if Some(id) == p.from
                    || !u.alive
                    || u.hidden
                    || u.controller == Controller::Npc
                    || !is_enemy(p.faction, u.faction)
                    || victim.is_some_and(|v| v < id)
                {
                    continue;
                }
                let b = i64::from(crate::units::def_of(u).bounds.0);
                if body_dist_sq(u, p.pos) > b * b {
                    continue;
                }
                victim = Some(id);
            }
        }
        if let Some(v) = victim {
            cx.scratch.hits[zi].push(Hit {
                to: v,
                amount: p.hit,
                school: spell.school,
                from: p.from,
                crit: p.crit,
                status: spell.effect,
            });
            dead = true;
        }
        if !dead && p.left.0 > 0 {
            i += 1;
            continue;
        }
        // An Explosion of hers pushes what it caught out from the burst (`feel::burst`).
        let blast = spell.school == jane_core::action::School::Blast;
        let mut caught: Vec<crate::ids::UnitId> = Vec::new();
        if blast {
            caught.extend(victim);
        }
        if let Some(splash) = spell.splash.filter(|_| p.hit.0 > 0) {
            let amount = round_points(i64::from(p.hit.0) / i64::from(splash.div.max(1)));
            query_near(cx.rt, p.pos, i64::from(splash.radius.0) + i64::from(max_bounds()), &mut near);
            for &id in &near {
                let Some(u) = cx.zone.unit(id) else { continue };
                if Some(id) == victim
                    || Some(id) == p.from
                    || !u.alive
                    || u.hidden
                    || u.controller == Controller::Npc
                    || !is_enemy(p.faction, u.faction)
                {
                    continue;
                }
                let r = i64::from(splash.radius.0);
                if body_dist_sq(u, p.pos) > r * r || amount.0 <= 0 {
                    continue;
                }
                cx.scratch.hits[zi].push(Hit {
                    to: id,
                    amount,
                    school: spell.school,
                    from: p.from,
                    crit: false,
                    status: None,
                });
                if blast {
                    caught.push(id);
                }
            }
        }
        if blast {
            crate::feel::burst(cx, spell, p.pos, p.from, &caught);
        }
        cx.zone.projectiles.remove(i);
        let touch = spell.touch.unwrap_or(SCHOOL_TOUCH_FX);
        // Stopped on the prop it was cast at (its own cells stop a shot): it touches the middle.
        let reach = i64::from(touch.0) + i64::from(CELL_FX);
        let at = match p.seek {
            Seek::Point(at) if dist_sq(p.pos, at) <= reach * reach => at,
            _ => p.pos,
        };
        crate::hooks::school_touch(cx, spell.school, at, p.from, touch);
        cx.emit(EventKind::Impact { spell: p.spell, school: spell.school, at: p.pos });
    }
    cx.scratch.near = near;
}

/// Step 8, pools.
pub fn step_grounds(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    let zi = cx.zone.id.index();
    let mut near = core::mem::take(&mut cx.scratch.near);
    let mut i = 0;
    while i < cx.zone.grounds.len() {
        let g = cx.zone.grounds[i];
        let spell = cx.cat.combat.spell(g.spell);
        if g.next_pulse <= now {
            let every = spell.ground.map_or(30, |p| p.pulse.0.max(1));
            cx.zone.grounds[i].next_pulse = now.after(jane_core::Tick(every));
            query_near(cx.rt, g.pos, i64::from(g.radius.0), &mut near);
            for &id in &near {
                let Some(u) = cx.zone.unit(id) else { continue };
                if !u.alive || u.hidden || u.faction == g.faction || u.controller == Controller::Npc {
                    continue;
                }
                if !within(g.pos, u.pos, g.radius) {
                    continue;
                }
                let amount = match (spell.power, g.from.and_then(|f| cx.zone.unit_ix(f))) {
                    (Some(p), Some(cix)) => {
                        let zone = &mut *cx.zone;
                        round_points(roll_power(&mut zone.rng, &zone.units[cix], &p))
                    }
                    _ => Milli::ZERO,
                };
                if amount.0 > 0 || spell.effect.is_some() {
                    cx.scratch.hits[zi].push(Hit {
                        to: id,
                        amount,
                        school: spell.school,
                        from: g.from,
                        crit: false,
                        status: spell.effect,
                    });
                }
            }
        }
        if g.until <= now {
            cx.zone.grounds.remove(i);
        } else {
            i += 1;
        }
    }
    cx.scratch.near = near;
}
