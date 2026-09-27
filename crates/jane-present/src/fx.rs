//! Effects (PRESENTATION.md §2): the particle pool, event-driven. A cast, an impact, a death, a
//! status or a swing resolves at event time into parts of its `jane_art::fx` recipe; a bolt the
//! view says is in flight sheds its trail every tick and carries its light; the weather's rain
//! falls and splashes, and ripples where it lands on water. Pools are fixed by the tier's
//! `max_particles` (§1.3), a third of it the weather's, and overflow drops the oldest.
//!
//! Every part steps by tick and is drawn at `pos + vel * alpha` (§1.11). The dice are an `Lcg`
//! reseeded per zone from `h32(seed, zone)`, so the same fight looks the same twice and nothing
//! here touches the sim's.

use jane_art::fx::{self as art, Lcg, Recipe, Role, Shape, Spark};
use jane_core::Angle;
use jane_core::action::Facing;
use jane_core::tile::F_WATER;
use jane_sim::event::{Event, EventKind};
use jane_sim::view::View;

use crate::atmos::Atmosphere;
use crate::frame::{CELL, Depth, FX_TO_CANVAS, Frame, Light, LightKind, PartShape, Particle, Pass, Rgb, Span, Tier};
use crate::light::Sky;

/// A light an effect throws, in zone canvas px, fading over its ticks.
#[derive(Clone, Copy, Debug)]
struct Glow {
    x: i32,
    y: i32,
    z: u8,
    colour: Rgb,
    radius: u16,
    ticks: u8,
    left: u8,
}

/// A status worn: whose, how it looks, how often it sheds.
#[derive(Clone, Copy, Debug)]
struct Worn {
    unit: u32,
    effect: u16,
    look: art::Status,
    every: u8,
}

/// A bolt in flight as last seen: its head, last tick's and this tick's, zone canvas px Q4.
#[derive(Clone, Copy, Debug)]
struct Head {
    id: u32,
    prev: (i32, i32),
    cur: (i32, i32),
    bolt: art::Bolt,
}

/// The pool and its dice.
#[derive(Debug)]
pub struct Fx {
    tier: Tier,
    cap: usize,
    parts: Vec<Spark>,
    rain: Vec<Spark>,
    rng: Lcg,
    glows: Vec<Glow>,
    worn: Vec<Worn>,
    heads: Vec<Head>,
    heads_next: Vec<Head>,
    grounds: Vec<u32>,
    grounds_next: Vec<u32>,
    tick: u32,
}

/// A frame of Q4 px.
const Q: i32 = 16;
/// Rain falls from this high, px, and this fast, Q4 px a tick.
const RAIN_TOP: (i32, i32) = (110, 190);
const RAIN_FALL: (i32, i32) = (150, 210);
/// Canvas px round the view the rain falls in, so none is missing at an edge.
const RAIN_MARGIN: i32 = 32;

impl Fx {
    /// A pool for `tier`, sized by its `max_particles`.
    pub fn new(tier: Tier, max_particles: u16) -> Fx {
        let cap = usize::from(max_particles);
        Fx {
            tier,
            cap,
            parts: Vec::with_capacity(cap - cap / 3),
            rain: Vec::with_capacity(cap / 3),
            rng: Lcg(1),
            glows: Vec::with_capacity(64),
            worn: Vec::with_capacity(64),
            heads: Vec::with_capacity(64),
            heads_next: Vec::with_capacity(64),
            grounds: Vec::with_capacity(32),
            grounds_next: Vec::with_capacity(32),
            tick: 0,
        }
    }

    /// Parts alive: the effects' and the weather's.
    /// The pool's size from the `max_particles` row, as it is now; what is over it goes, the
    /// oldest first.
    pub fn set_cap(&mut self, max_particles: u16) {
        self.cap = usize::from(max_particles);
        let (fx, weather) = (self.cap - self.cap / 3, self.cap / 3);
        if self.parts.len() > fx {
            self.parts.drain(..self.parts.len() - fx);
        }
        self.rain.truncate(weather);
    }

    pub fn count(&self) -> (usize, usize) {
        (self.parts.len(), self.rain.len())
    }

    /// A new zone: nothing carries over, and the dice start again from the zone.
    pub fn zone(&mut self, view: &View<'_>) {
        self.parts.clear();
        self.rain.clear();
        self.glows.clear();
        self.worn.clear();
        self.heads.clear();
        self.grounds.clear();
        self.rng = Lcg(jane_art::hash::h32(view.seed(), view.zone() as u32, 0x6678_6678));
    }

    fn push(&mut self, s: Spark) {
        let fx_cap = self.cap - self.cap / 3;
        if self.parts.len() >= fx_cap {
            // Overflow drops the oldest.
            self.parts.remove(0);
        }
        self.parts.push(s);
    }

    fn emit(&mut self, r: &Recipe, at: (i32, i32), dir: Angle) {
        let mut rng = self.rng;
        let mut out = Vec::new();
        art::emit(r, at, dir, &mut rng, &mut |s| out.push(s));
        self.rng = rng;
        for s in out {
            self.push(s);
        }
        if let Some(l) = r.light {
            let colour = l.role.of(art::hue(r.tint));
            self.glows.push(Glow { x: at.0, y: at.1, z: l.z, colour, radius: l.radius, ticks: l.ticks, left: l.ticks });
            if self.glows.len() > 48 {
                self.glows.remove(0);
            }
        }
    }

    /// This tick's events, as they happen (§2: a cast or a death resolves now, not a frame late).
    pub fn on_events(&mut self, view: &View<'_>, events: &[Event]) {
        let cat = jane_data::catalog();
        let px = |v: jane_core::Vec2| (v.x.0 >> FX_TO_CANVAS, v.y.0 >> FX_TO_CANVAS);
        for e in jane_sim::event::events_for(events, view.me()) {
            match e.kind {
                EventKind::Cast { unit, spell, at } => {
                    let def = cat.combat.spell(spell);
                    let Some(f) = art::spell(def.id) else { continue };
                    let facing = view.unit(unit).map_or(Facing::South, |u| u.facing);
                    let (dx, dy) = step(facing);
                    let (x, y) = px(at);
                    self.emit(&art::cast(f.cast), (x + dx * 10, y + dy * 4), angle(facing));
                }
                EventKind::Swing { at, facing, .. } => {
                    let (dx, dy) = step(facing);
                    let (x, y) = px(at);
                    self.emit(&art::swing(), (x + dx * 12, y + dy * 8), angle(facing));
                }
                EventKind::Impact { spell, at, .. } => {
                    let def = cat.combat.spell(spell);
                    if let Some(f) = art::spell(def.id) {
                        let dir = Angle(self.rng.below(65536) as u16);
                        self.emit(&art::impact(f.impact), px(at), dir);
                    }
                }
                EventKind::Death { at, .. } => self.emit(&art::death(), px(at), Angle::NORTH),
                EventKind::Status { unit, effect, on } => {
                    let def = cat.combat.effect(effect);
                    let Some(look) = art::status(def.id) else { continue };
                    let (r, every) = art::wear(look);
                    if on && every == 255 {
                        // Instant: one flicker as it lands.
                        if let Some(u) = view.unit(unit) {
                            self.emit(&r, px(u.pos), Angle::NORTH);
                        }
                    } else if on {
                        if !self.worn.iter().any(|w| w.unit == unit.get() && w.effect == effect.0) && self.worn.len() < 64 {
                            self.worn.push(Worn { unit: unit.get(), effect: effect.0, look, every });
                        }
                    } else {
                        self.worn.retain(|w| !(w.unit == unit.get() && w.effect == effect.0));
                    }
                }
                _ => {}
            }
        }
    }

    /// One tick: every part steps, the bolts shed their trails, the statuses theirs, the pools on
    /// the ground theirs, and the rain falls. `view_px` is the view's top-left and size in zone
    /// canvas px.
    pub fn tick(&mut self, view: &View<'_>, atmos: &Atmosphere, tick: u32, view_px: (i32, i32, i32, i32)) {
        self.tick = tick;
        let cat = jane_data::catalog();
        self.parts.retain_mut(Spark::step);
        for g in &mut self.glows {
            g.left = g.left.saturating_sub(1);
        }
        self.glows.retain(|g| g.left > 0);
        // Statuses shed on their wearers.
        for i in 0..self.worn.len() {
            let w = self.worn[i];
            if tick % u32::from(w.every.max(1)) != w.unit % u32::from(w.every.max(1)) {
                continue;
            }
            let Some(u) = jane_sim::ids::UnitId::new(w.unit).and_then(|id| view.unit(id)) else { continue };
            if !u.alive {
                continue;
            }
            let (r, _) = art::wear(w.look);
            let at = (u.pos.x.0 >> FX_TO_CANVAS, u.pos.y.0 >> FX_TO_CANVAS);
            self.emit(&r, at, Angle::NORTH);
        }
        // Bolts: their trails, and their heads for the frame.
        self.heads_next.clear();
        for p in view.projectiles() {
            let def = cat.combat.spell(p.spell);
            let Some(bolt) = art::spell(def.id).and_then(|f| f.bolt) else { continue };
            let cur = (p.pos.x.0 >> (FX_TO_CANVAS - 4), p.pos.y.0 >> (FX_TO_CANVAS - 4));
            let prev = self.heads.iter().find(|h| h.id == p.id.get()).map_or(cur, |h| h.cur);
            self.heads_next.push(Head { id: p.id.get(), prev, cur, bolt });
            let back = Angle(p.heading.0.wrapping_add(32768));
            // Shed along the tick's whole path, not at its end: a trail, not a string of beads.
            for k in 0..3 {
                let t = (k * 2 + 1) * 256 / 6;
                let at = (prev.0 + (cur.0 - prev.0) * t / 256, prev.1 + (cur.1 - prev.1) * t / 256);
                let jitter = (self.rng.range(-1, 1), self.rng.range(-1, 1));
                if k == 0 || self.rng.below(2) == 0 {
                    self.emit(&art::trail(bolt), (at.0 / Q + jitter.0, at.1 / Q + jitter.1), back);
                }
            }
        }
        std::mem::swap(&mut self.heads, &mut self.heads_next);
        // Pools on the ground: a burst when one appears.
        self.grounds_next.clear();
        for g in view.grounds() {
            let id = g.id.get();
            self.grounds_next.push(id);
            if !self.grounds.contains(&id) {
                let def = cat.combat.spell(g.spell);
                if let Some(look) = art::spell(def.id).and_then(|f| f.ground) {
                    self.emit(&art::ground(look), (g.pos.x.0 >> FX_TO_CANVAS, g.pos.y.0 >> FX_TO_CANVAS), Angle::NORTH);
                }
            }
        }
        std::mem::swap(&mut self.grounds, &mut self.grounds_next);
        self.weather(view, atmos, view_px);
    }

    /// The rain: drops fall onto ground points in and round the view, leaning with the wind, and
    /// each ends in a splash, or a ripple on water.
    fn weather(&mut self, view: &View<'_>, atmos: &Atmosphere, (vx, vy, vw, vh): (i32, i32, i32, i32)) {
        let cap = self.cap / 3;
        let look = jane_art::weather::rain(atmos.region());
        // Below T2 a drop catches a little of the sky's light of its own, or a night's rain is
        // lost in the dark: T2 lights each drop by the lamps it passes.
        let rain_glow = if self.tier >= Tier::T2 { 0 } else { 90 };
        // Land what falls: a drop at its last tick splashes where it stands.
        let mut i = 0;
        while i < self.rain.len() {
            let s = &mut self.rain[i];
            let was_drop = matches!(s.shape, Shape::Streak);
            if s.step() {
                i += 1;
                continue;
            }
            let (x, y) = (s.x / Q, s.y / Q);
            self.rain.swap_remove(i);
            if was_drop && self.rain.len() < cap {
                let water = view.flags(x.div_euclid(CELL), y.div_euclid(CELL)) & F_WATER != 0;
                let (shape, life, colour) =
                    if water { (Shape::Ring(1, 7), 22, look.ripple) } else { (Shape::Ring(0, 2), 5, look.splash) };
                self.rain.push(Spark {
                    x: x * Q,
                    y: y * Q,
                    z: 0,
                    vx: 0,
                    vy: 0,
                    vz: 0,
                    age: 0,
                    life,
                    shape,
                    colour,
                    late: colour,
                    glow: rain_glow / 2,
                    grav: 0,
                    drag: 255,
                    ground: true,
                });
            }
        }
        let level = atmos.rain();
        if level == 0 || !atmos.outdoors() {
            return;
        }
        // How many a tick: a full storm fills the weather's third of the pool.
        let area = (vw + 2 * RAIN_MARGIN) * (vh + RAIN_TOP.1 + RAIN_MARGIN);
        let want = (level as i32 / 256) * area / (768 * 620) * cap as i32 / 256 / 9;
        let n = want.clamp(0, 400);
        let wind = atmos.wind();
        for _ in 0..n {
            if self.rain.len() >= cap {
                break;
            }
            let top = self.rng.range(RAIN_TOP.0, RAIN_TOP.1);
            let fall = self.rng.range(RAIN_FALL.0, RAIN_FALL.1);
            let life = (top * Q / fall).clamp(2, 250) as u8;
            let gx = vx - RAIN_MARGIN + self.rng.range(0, vw + 2 * RAIN_MARGIN);
            let gy = vy + self.rng.range(0, vh + top);
            // It lands where it falls to: start it upwind of its ground point.
            let lean = wind * 3 / 2 + self.rng.range(-3, 3);
            let head = if self.rng.below(3) == 0 { look.head } else { look.tail };
            self.rain.push(Spark {
                x: gx * Q - lean * i32::from(life),
                y: gy * Q,
                z: top * Q,
                vx: lean,
                vy: 0,
                vz: -fall,
                age: 0,
                life,
                shape: Shape::Streak,
                colour: head,
                late: head,
                glow: rain_glow,
                grav: 0,
                drag: 255,
                ground: false,
            });
        }
    }

    /// The effects' lights into the frame's list (`cam` the view's top-left, canvas px): every
    /// glow still burning, and every bolt's head. A bolt lights the wall it passes (§2).
    pub fn lights(&self, f: &mut Frame, cam: (i32, i32), alpha: u8) {
        let a = i32::from(alpha);
        for g in &self.glows {
            let k = u32::from(g.left) * 256 / u32::from(g.ticks.max(1));
            f.lights.push(Light {
                pos: (g.x - cam.0, g.y - cam.1),
                height: g.z.max(6),
                colour: g.colour.map(|c| (u32::from(c) * k / 256) as u8),
                radius: g.radius,
                size: 8,
                casts: false,
                clear: 0,
                kind: LightKind::Point,
            });
        }
        for h in &self.heads {
            let (_, _, light) = art::head(h.bolt);
            let Some(l) = light else { continue };
            let (x, y) = lerp(h.prev, h.cur, a);
            let hue = art::hue(art::head(h.bolt).0);
            f.lights.push(Light {
                pos: (x - cam.0, y - cam.1),
                height: l.z,
                colour: l.role.of(hue),
                radius: l.radius,
                size: 4,
                casts: true,
                clear: 6,
                kind: LightKind::Point,
            });
        }
    }

    /// Below T2, before the light pass: every part that does not glow, so the lightmap lights it
    /// as it lights the ground (rain shows in a lamp's pool and is dark beyond it). T2 lights
    /// its parts itself, after the light pass, and draws nothing here.
    pub fn draw_under_light(&self, f: &mut Frame, cam: (i32, i32), alpha: u8) {
        if self.tier >= Tier::T2 {
            return;
        }
        let p0 = f.parts.len();
        for s in self.rain.iter().chain(&self.parts).filter(|s| s.ground && s.glow == 0) {
            self.put(f, s, cam, alpha, None);
        }
        if f.parts.len() > p0 {
            f.passes.push(Pass::Particles { layer: Depth::Ground, parts: Span::since(p0, f.parts.len()) });
        }
        let a0 = f.parts.len();
        for s in self.parts.iter().filter(|s| !s.ground && s.glow == 0) {
            self.put(f, s, cam, alpha, None);
        }
        if f.parts.len() > a0 {
            f.passes.push(Pass::Particles { layer: Depth::Canopy, parts: Span::since(a0, f.parts.len()) });
        }
        let r0 = f.parts.len();
        for s in self.rain.iter().filter(|s| !s.ground) {
            self.put(f, s, cam, alpha, None);
        }
        if f.parts.len() > r0 {
            f.passes.push(Pass::Particles { layer: Depth::Weather, parts: Span::since(r0, f.parts.len()) });
        }
    }

    /// The parts on the ground (`Particles { Ground }`): splashes, ripples, marks. Below T2 only
    /// those that glow (the rest went under the light).
    pub fn draw_ground(&self, f: &mut Frame, cam: (i32, i32), alpha: u8, sky: &Sky) {
        let p0 = f.parts.len();
        let t2 = self.tier >= Tier::T2;
        for s in self.rain.iter().chain(&self.parts).filter(|s| s.ground && (t2 || s.glow > 0)) {
            self.put(f, s, cam, alpha, Some(sky));
        }
        if f.parts.len() > p0 {
            f.passes.push(Pass::Particles { layer: Depth::Ground, parts: Span::since(p0, f.parts.len()) });
        }
    }

    /// The parts in the air (`Particles { Canopy }`, then the rain as `Particles { Weather }`).
    /// Below T2 only those that glow, and the bolts' heads.
    pub fn draw_air(&self, f: &mut Frame, cam: (i32, i32), alpha: u8, sky: &Sky) {
        let p0 = f.parts.len();
        let t2 = self.tier >= Tier::T2;
        for s in self.parts.iter().filter(|s| !s.ground && (t2 || s.glow > 0)) {
            self.put(f, s, cam, alpha, Some(sky));
        }
        // Each bolt's head: a soft glow and a bright core.
        let a = i32::from(alpha);
        for h in &self.heads {
            let (tint, r, _) = art::head(h.bolt);
            let hue = art::hue(tint);
            let (x, y) = lerp(h.prev, h.cur, a);
            let (sx, sy) = (x - cam.0, y - cam.1 - 16);
            // The head's own streak back along its flight: it moves.
            let (tx, ty) = ((h.prev.0 - h.cur.0) * 3 / (2 * Q), (h.prev.1 - h.cur.1) * 3 / (2 * Q));
            f.parts.push(Particle {
                x: sx as i16,
                y: sy as i16,
                shape: PartShape::Streak { dx: tx.clamp(-40, 40) as i8, dy: ty.clamp(-40, 40) as i8 },
                colour: self.lit(Role::Mid.of(hue), 255, Some(sky)),
                alpha: 220,
                glow: 255,
                height: 16,
            });
            for (shape, colour, alpha) in [
                (PartShape::Glow { r }, Role::Mid.of(hue), 200),
                (PartShape::Dot { size: 2 }, Role::Core.of(hue), 255),
            ] {
                let off = i32::from(matches!(shape, PartShape::Dot { .. }));
                f.parts.push(Particle {
                    x: (sx - off) as i16,
                    y: (sy - off) as i16,
                    shape,
                    colour: self.lit(colour, 255, Some(sky)),
                    alpha,
                    glow: 255,
                    height: 16,
                });
            }
        }
        if f.parts.len() > p0 {
            f.passes.push(Pass::Particles { layer: Depth::Canopy, parts: Span::since(p0, f.parts.len()) });
        }
        if t2 {
            let r0 = f.parts.len();
            for s in self.rain.iter().filter(|s| !s.ground) {
                self.put(f, s, cam, alpha, Some(sky));
            }
            if f.parts.len() > r0 {
                f.passes.push(Pass::Particles { layer: Depth::Weather, parts: Span::since(r0, f.parts.len()) });
            }
        }
    }

    /// A colour as this tier draws it: T2 lights parts itself, and a part drawn under the light
    /// is lit by it; one drawn over the light below T2 has its unlit share multiplied by the
    /// frame's flat light here.
    fn lit(&self, c: Rgb, glow: u8, sky: Option<&Sky>) -> Rgb {
        let Some(sky) = sky.filter(|_| self.tier < Tier::T2) else { return c };
        let g = u32::from(glow);
        [0, 1, 2].map(|k| {
            let unlit = (u32::from(c[k]) * (u32::from(sky.ambient[k]) + 16)) >> 8;
            ((unlit * (255 - g) + u32::from(c[k]) * g) / 255).min(255) as u8
        })
    }

    /// One part into the frame, where it stands at `alpha`.
    fn put(&self, f: &mut Frame, s: &Spark, cam: (i32, i32), alpha: u8, sky: Option<&Sky>) {
        let a = i32::from(alpha);
        let (x, y, z) = if s.ground {
            (s.x, s.y, s.z)
        } else {
            (s.x + s.vx * a / 256, s.y + s.vy * a / 256, (s.z + s.vz * a / 256).max(0))
        };
        let (sx, sy) = (x / Q - cam.0, (y - z) / Q - cam.1);
        let (cw, ch) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        if sx < -64 || sy < -64 || sx > cw + 64 || sy > ch + 64 {
            return;
        }
        let life = i32::from(s.life.max(1));
        let shape = match s.shape {
            Shape::Dot(n) => PartShape::Dot { size: n },
            Shape::Streak => {
                // The stroke trails back along its motion: a drop's tail up and upwind.
                // Some drops are caught longer than others (its start, hashed): rain is not a comb.
                let k = 3 + (((s.y >> 4).wrapping_mul(7) + i32::from(s.life)) & 3);
                let (dx, dy) = (-s.vx * k / (3 * Q), (s.vz - s.vy) * k / (3 * Q));
                PartShape::Streak { dx: dx.clamp(-40, 40) as i8, dy: dy.clamp(-40, 40) as i8 }
            }
            Shape::Ring(r0, r1) => {
                let r = i32::from(r0) + (i32::from(r1) - i32::from(r0)) * i32::from(s.age) / life;
                PartShape::Ring { r: r.clamp(0, 255) as u8 }
            }
            Shape::Glow(r) => {
                let r = i32::from(r) * (life - i32::from(s.age)) / life;
                PartShape::Glow { r: r.clamp(1, 255) as u8 }
            }
        };
        let alpha = match s.shape {
            Shape::Streak if !s.ground && s.glow == 0 => s.alpha_now() / 4 * 3,
            _ => s.alpha_now(),
        };
        f.parts.push(Particle {
            x: sx as i16,
            y: sy as i16,
            shape,
            colour: self.lit(s.colour_now(), s.glow, sky),
            alpha,
            glow: s.glow,
            height: (z / Q).clamp(0, 255) as u8,
        });
    }
}

/// A facing as a unit step across the ground.
fn step(f: Facing) -> (i32, i32) {
    match f {
        Facing::East => (1, 0),
        Facing::West => (-1, 0),
        Facing::North => (0, -1),
        Facing::South => (0, 1),
    }
}

fn angle(f: Facing) -> Angle {
    match f {
        Facing::East => Angle::EAST,
        Facing::West => Angle::WEST,
        Facing::North => Angle::NORTH,
        Facing::South => Angle::SOUTH,
    }
}

/// Q4 `a` toward `b` at `alpha` of 256, as whole px.
fn lerp(a: (i32, i32), b: (i32, i32), alpha: i32) -> (i32, i32) {
    ((a.0 + (b.0 - a.0) * alpha / 256) / Q, (a.1 + (b.1 - a.1) * alpha / 256) / Q)
}
