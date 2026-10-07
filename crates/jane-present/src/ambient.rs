//! The ambient life layer (ART-PLAN M1, B4): what lives in a scene besides its people. Silence is
//! the default and a bird an event: by day a sparrow or a pigeon pecks by a lived-in house or on
//! the square, mostly alone, and flies off when she comes within three cells; open country keeps
//! a lone crow on a fence now and then, turned to watch her (by night only the odd crow); ducks
//! paddle on open water; a cat sits on a wall; butterflies work the flower beds by day and moths
//! circle the lamps by night; smoke rises from the chimneys of the houses someone lives in, and
//! not from the empty ones; dust in the Works. The water has its
//! own: foam along the shore, glints on open water by day, lily pads at the margins, a fish rising
//! now and then, rings at her feet on the stepping stones.
//!
//! It reads the [`View`] (the tiles, the hour, the weather, where she is) and emits sprites into
//! the presenter's draw lists and particles into the frame. Every choice is a hash of a cell and
//! a period of ticks (`h32(cell, tick / period)`): two machines show the same birds. It never
//! touches the sim and gives nothing a footprint; its only memory is which flocks she has
//! flushed and the rings at her feet, both the presenter's.
//!
//! How many it draws is the tier's (ART-PLAN M1): about 12 actors on T0, 24 on T1 and 48 on T2,
//! the farthest from the middle of the view left out first; the birds' and crows' odds are the
//! same on every tier. By day out of doors at least one is always on screen, a bird kept only
//! where nothing else lives (ART-PLAN §7 rule 5, `something_lives_in_every_outdoor_frame_by_day`).

use alloc::vec::Vec;
use jane_art::creature::critter::{self, Critter, Pose as CPose};
use jane_art::hash::h32;
use jane_core::Angle;
use jane_core::Rect;
use jane_core::angle::{cos_q15, sin_q15};
use jane_core::tile::{F_SOLID, Tile};
use jane_data::Region;
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::atmos::Atmosphere;
use crate::creatures::{self, Creatures};
use crate::facing::Face8;
use crate::frame::{CELL, Depth, Frame, PartShape, Particle, Pass, Rgb, Span, Tier};

/// The most actors a tier draws (ART-PLAN M1).
pub const fn cap(tier: Tier) -> usize {
    match tier {
        Tier::T0 => 12,
        Tier::T1 => 24,
        Tier::T2 => 48,
    }
}

/// Of 256, how many candidate spots are taken, by tier: T0 is sparser, T2 busier.
const fn share(tier: Tier) -> u32 {
    match tier {
        Tier::T0 => 48,
        Tier::T1 => 72,
        Tier::T2 => 96,
    }
}

/// Of 256, how many candidate spots by a lived-in house or on the square a ground bird takes, and
/// of 512, how many perches a crow takes by day and by night: the same on every tier (the cap only
/// limits, so the tiers keep one mood). Silence is the default and a bird an event (2026-10-06:
/// "it feels like birdland").
const BIRDS: u32 = 16;
const CROWS_DAY: u32 = 12;
const CROWS_NIGHT: u32 = 3;
/// A ground bird keeps within this many px of a lived-in house's chimney (or to the square).
const HOME: i32 = 8 * CELL;

/// A block of cells holds at most one flock (ground birds), one perch, one patch of butterflies.
const BLOCK: i32 = 6;
/// Ticks a flock stays before it moves on (it flies off, and another may land).
const EPOCH: u32 = 2400;
/// Ticks a flock takes to land, or to leave.
const LAND: u32 = 40;
/// She flushes a bird within this many px (three cells).
const FLUSH: i32 = 3 * CELL;
/// Ticks a flushed bird is drawn flying off: a crouch and a hop, then a climbing arc away from
/// her with its wings beating, fading out over its last ticks (never a blink to elsewhere).
const FLEE: u32 = 72;
/// Ticks of the hop before the wings open.
const SPRING: u32 = 6;
/// Ticks the flight fades over at its end.
const FADE: u32 = 18;
/// The most ticks a bird of a flushed flock waits after the first goes (they go one by one).
const STAGGER: u32 = 6;
/// The ticks of a chimney's puff from the pot to gone.
const PUFF: u32 = 200;
/// Puffs over a chimney.
const PUFFS: u32 = 5;
/// The salt of everything here.
const SALT: u32 = 0x616d_6269;
/// Draw keys of ambient sprites: under the units', the drops' and the props' marks.
pub const AMBIENT_KEY: u32 = 0x0400_0000;

/// Clock ticks an hour (the sim's, `jane_core::num::TICKS_PER_HOUR`).
const HOUR: u32 = jane_core::num::TICKS_PER_HOUR;

/// A flock put up: its key, the tick, the flock as it was, and where she was.
type Flushed = (u32, u32, Flock, Option<(i32, i32)>);

/// One thing the layer draws this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actor {
    /// Its ground point, zone canvas px.
    pub x: i32,
    pub y: i32,
    /// Px it is drawn up off its ground point (a bird in the air, a puff over a pot).
    pub up: i32,
    pub look: RefId,
    pub mirror: bool,
    /// 255 drawn as it is; less, see-through (a puff fading, a bird flying off into the distance).
    pub alpha: u8,
    /// It sits on what the terrain raises at its ground point (a fence, a wall): drawn on its top.
    pub perch: bool,
    /// It lies flat on the ground layer (a lily pad), under everything that stands.
    pub flat: bool,
    /// Its draw key's low bits.
    pub key: u32,
}

/// A ring on the water at her feet, or a fish's rise: `(x, y, start tick, widest)`.
type Ring = (i32, i32, u32, u8);

/// What it shows of a kind of critter: its frames by pose.
#[derive(Clone, Debug)]
struct Set {
    critter: Critter,
    frames: Vec<(CPose, RefId)>,
}

/// The ambient layer: its looks in the atlas and its little memory.
#[derive(Clone, Debug)]
pub struct Ambient {
    tier: Tier,
    sets: Vec<Set>,
    /// Smoke's puffs by size, smallest first; a second size row for variety.
    puffs: Vec<RefId>,
    pads: Vec<RefId>,
    crow: Option<u16>,
    cats: Vec<u16>,
    /// Flocks she has flushed: `(flock key, tick, the flock as it was, where she was)`. A flushed
    /// flock flies from where it was and away from where she was then, whatever the blocks round
    /// the view make of its ground or wherever she walks meanwhile.
    flushed: Vec<Flushed>,
    /// The flock kept on screen by day when no other is (rule 5), and the tick it was chosen
    /// (it flies in over its first ticks, never popping up).
    forced: Option<u32>,
    forced_at: u32,
    rings: Vec<Ring>,
    /// This tick's actors, nearest the view's middle first.
    actors: Vec<Actor>,
    /// This tick's particles, in zone px, with their layer: `true` on the ground.
    parts: Vec<(Particle, bool)>,
    /// The chimneys of lived-in houses near the view: `(id, x, top y, foot y)`, zone px.
    chimneys: Vec<(u32, i32, i32, i32)>,
    /// Lamps lit near the view: `(id, x, glass y)`, zone px.
    lamps: Vec<(u32, i32, i32)>,
}

/// The presenter's facts the layer reads besides the view.
#[derive(Debug)]
pub struct Ctx<'a> {
    pub tick: u32,
    /// The view in zone canvas px: `(x, y, w, h)`.
    pub view_px: (i32, i32, i32, i32),
    /// Her feet, zone canvas px, and whether she is walking.
    pub her: Option<((i32, i32), bool)>,
    pub atmos: &'a Atmosphere,
    /// The cells where a person stands (a bird beside one is put up).
    pub people: &'a [(i32, i32)],
}

impl Ambient {
    /// Packs the critters, the puffs and the pads into `atlas`.
    pub fn build(tier: Tier, atlas: &mut Atlas, creatures: &Creatures) -> Ambient {
        let sets = Critter::ALL
            .iter()
            .map(|&c| {
                let s = critter::render(c);
                let anchor = (s.ax as i16, s.ay as i16);
                let height = s.ay.clamp(1, 255) as u8;
                let frames =
                    s.frames.iter().map(|(p, cv)| (*p, atlas.add_canvas(cv, anchor, height, |_, _, t| t))).collect();
                Set { critter: c, frames }
            })
            .collect();
        let puffs = (1..=6)
            .flat_map(|r| [critter::puff(r, r as u32 * 7), critter::puff(r, r as u32 * 13 + 1)])
            .map(|c| atlas.add_canvas(&c, ((c.w() / 2) as i16, (c.h() / 2) as i16), 1, |_, _, t| t))
            .collect();
        let pads = (0..6)
            .map(critter::lily_pad)
            .map(|c| atlas.add_canvas(&c, ((c.w() / 2) as i16, (c.h() / 2) as i16), 1, |_, _, t| t))
            .collect();
        let set_of = |name: &str| jane_art::looks::find(name).and_then(|(s, _)| creatures.set(s));
        Ambient {
            tier,
            sets,
            puffs,
            pads,
            crow: set_of("crow"),
            cats: ["town_cat_black", "town_cat_ginger"].iter().filter_map(|n| set_of(n)).collect(),
            flushed: Vec::with_capacity(16),
            forced: None,
            forced_at: 0,
            rings: Vec::with_capacity(16),
            actors: Vec::with_capacity(64),
            parts: Vec::with_capacity(512),
            chimneys: Vec::with_capacity(16),
            lamps: Vec::with_capacity(16),
        }
    }

    /// A new zone: nothing remembered.
    pub fn zone(&mut self) {
        self.flushed.clear();
        self.forced = None;
        self.rings.clear();
        self.actors.clear();
        self.parts.clear();
    }

    /// The chimneys and lamps the presenter found near the view this tick.
    pub fn set_sources(
        &mut self,
        chimneys: impl Iterator<Item = (u32, i32, i32, i32)>,
        lamps: impl Iterator<Item = (u32, i32, i32)>,
    ) {
        self.chimneys.clear();
        self.chimneys.extend(chimneys);
        self.lamps.clear();
        self.lamps.extend(lamps);
    }

    /// This tick's actors (nearest the view's middle first), drawn by the presenter.
    pub fn actors(&self) -> &[Actor] {
        &self.actors
    }

    /// How many living things it shows this tick (smoke counts: ART-PLAN §7 rule 5 names it).
    pub fn living(&self) -> usize {
        self.actors.iter().filter(|a| !a.flat).count()
    }

    fn frame(&self, c: Critter, p: CPose) -> Option<RefId> {
        let s = self.sets.iter().find(|s| s.critter == c)?;
        s.frames.iter().find(|(q, _)| *q == p).or(s.frames.first()).map(|(_, r)| *r)
    }

    /// Steps the layer: who is where this tick.
    pub fn tick(&mut self, view: &View<'_>, cx: &Ctx<'_>, creatures: &Creatures) {
        self.actors.clear();
        self.parts.clear();
        let t = cx.tick;
        self.flushed.retain(|&(_, at, _, _)| t.wrapping_sub(at) < EPOCH + FLEE + STAGGER);
        self.rings.retain(|r| t.wrapping_sub(r.2) < 60);
        let (clock, _) = view.clock();
        let hour = clock / HOUR % 24;
        let minute = clock % HOUR * 60 / HOUR;
        let atmos = cx.atmos;
        let weather = atmos.atmos().kind;
        let wet = matches!(weather, crate::frame::WeatherKind::Rain | crate::frame::WeatherKind::Storm);
        let outdoors = !view.indoor();
        let day = (7..18).contains(&hour) || hour == 6 && minute >= 30;
        let seed = view.seed() ^ SALT;
        let (vx, vy, vw, vh) = cx.view_px;
        let mid = (vx + vw / 2, vy + vh / 2);
        // The cells the view and a margin cover.
        let margin = 2 * CELL;
        let cells = Rect::new(
            (vx - margin).div_euclid(CELL),
            (vy - margin).div_euclid(CELL),
            (vw + 2 * margin) / CELL + 2,
            (vh + 2 * margin) / CELL + 2,
        );
        let tile = |x: i32, y: i32| view.tile(x, y);
        let in_view =
            |x: i32, y: i32, pad: i32| x >= vx - pad && y >= vy - pad && x < vx + vw + pad && y < vy + vh + pad;
        let her = cx.her.map(|h| h.0);
        let near_her = |x: i32, y: i32, d: i32| her.is_some_and(|(hx, hy)| (hx - x).abs() < d && (hy - y).abs() < d);
        let thr = share(self.tier);
        let mut candidates: Vec<Flock> = Vec::new();

        if outdoors {
            let (bx0, by0) = (cells.x.div_euclid(BLOCK), cells.y.div_euclid(BLOCK));
            let (bx1, by1) = ((cells.x + cells.w).div_euclid(BLOCK), (cells.y + cells.h).div_euclid(BLOCK));
            for by in by0..=by1 {
                for bx in bx0..=bx1 {
                    let hb = h32(bx as u32, by as u32, seed);
                    // Ground birds: a spot of open ground in the block.
                    if day {
                        let home = |x: i32, y: i32| {
                            self.chimneys
                                .iter()
                                .any(|&(_, hx, _, foot)| (hx - x).abs() < HOME && (foot - y).abs() < HOME)
                        };
                        if let Some(f) = ground_flock(view, bx, by, hb, t, &tile, &home) {
                            candidates.push(f);
                        }
                    }
                    // Ducks on open water, by day.
                    if (6..19).contains(&hour) {
                        self.ducks(bx, by, hb, t, &tile, thr);
                    }
                    // A crow on a fence (the odd one by night), or a cat on a wall.
                    let crows = if day { CROWS_DAY } else { CROWS_NIGHT };
                    self.perched(bx, by, hb, t, &tile, her, creatures, (thr, crows), day && !wet);
                    // Butterflies over a flower bed, in the warm of the day.
                    if (10..17).contains(&hour) && weather == crate::frame::WeatherKind::Clear {
                        self.butterflies(bx, by, hb, t, &tile, thr);
                    }
                }
            }
        }
        // The flocks that land this epoch, and the one kept for rule 5.
        let memory = self.flushed.clone();
        let flushed = |k: u32| memory.iter().find(|f| f.0 == k).map(|f| f.1);
        // Fewer land in the rain.
        let thr = if wet { BIRDS / 3 } else { BIRDS };
        if day && outdoors {
            // The one kept stays while it is in view and has ground under it.
            let drawn = |f: &Flock| f.bird(0, t, true, &tile).is_some();
            let keep = self.forced.filter(|k| {
                candidates.iter().any(|f| f.key == *k && flushed(*k).is_none() && in_view(f.x, f.y, 0) && drawn(f))
            });
            // Rule 5 keeps a bird only where nothing else lives: a duck, a crow, the cat, a
            // butterfly or a chimney's smoke in view is enough.
            let other = self.actors.iter().any(|a| !a.flat && in_view(a.x, a.y, 0))
                || self.chimneys.iter().any(|&(_, x, top, _)| in_view(x, top, 0));
            let shown =
                other || candidates.iter().any(|f| f.present(thr) && flushed(f.key).is_none() && in_view(f.x, f.y, 0));
            let was = self.forced;
            self.forced = keep.or_else(|| {
                (!shown)
                    .then(|| {
                        candidates
                            .iter()
                            .filter(|f| {
                                flushed(f.key).is_none()
                                    && in_view(f.x, f.y, -CELL)
                                    && !near_her(f.x, f.y, 5 * CELL)
                                    && drawn(f)
                            })
                            .min_by_key(|f| f.rank)
                            .map(|f| f.key)
                    })
                    .flatten()
            });
            if self.forced.is_some() && self.forced != was {
                self.forced_at = t;
            }
        } else {
            self.forced = None;
        }
        let mut newly = Vec::new();
        // Somebody walking onto a bird's ground puts it up too (it never steps aside in a blink).
        let trodden = |x: i32, y: i32| {
            let (cx_, cy_) = (x.div_euclid(CELL), y.div_euclid(CELL));
            cx.people.iter().any(|&(px, py)| (px - cx_).abs() <= 1 && (py - cy_).abs() <= 1)
        };
        for f in &candidates {
            let forced = self.forced == Some(f.key);
            if flushed(f.key).is_some() || !(forced || f.present(thr)) {
                continue;
            }
            let arrive = if forced { Some(t.wrapping_sub(self.forced_at)) } else { None };
            // A bird in the air flies from (or to) one ground point: where it stood the tick it
            // went (flushed, or leaving at the epoch's end), or where it will stand when it lands.
            let (_, ep_t) = f.epoch_t(t);
            let anchor = match arrive {
                Some(a) if a < LAND => t.wrapping_add(LAND - a),
                None if ep_t < LAND => t.wrapping_add(LAND - ep_t),
                None if ep_t >= EPOCH - LAND => t.wrapping_sub(ep_t - (EPOCH - LAND)),
                _ => t,
            };
            let mut up = false;
            for i in 0..if f.home { f.n } else { 1 } {
                let Some(b) = f.bird(i, anchor, forced, &tile) else { continue };
                up |= near_her(b.0, b.1, FLUSH) || trodden(b.0, b.1);
                self.bird(f, i, b, t, arrive, None, her);
            }
            if up {
                newly.push(*f);
            }
        }
        // The flocks put up: each drawn to the end of its flight from where it stood.
        for &(_, at, f, from) in &memory {
            if t.wrapping_sub(at) >= FLEE + STAGGER {
                continue;
            }
            for i in 0..if f.home { f.n } else { 1 } {
                let Some(b) = f.bird(i, at, false, &tile) else { continue };
                self.bird(&f, i, b, t, None, Some(at), from);
            }
        }
        for f in newly {
            if !self.flushed.iter().any(|g| g.0 == f.key) {
                self.flushed.push((f.key, t, f, her));
            }
        }
        // Smoke from the chimneys of lived-in houses (ART §2.8): five puffs on a rising path,
        // leaning with the wind, each fainter.
        let wind = atmos.wind();
        for i in 0..self.chimneys.len() {
            let (id, x, top, foot) = self.chimneys[i];
            let hc = h32(id, 0, seed ^ 0x736d);
            for k in 0..PUFFS {
                let age = t.wrapping_add(k * PUFF / PUFFS).wrapping_add(hc) % PUFF;
                let up = 3 + (age * 34 / PUFF) as i32;
                // It leans with the wind as it rises, and wanders a px or two.
                let lean = (wind * age as i32 / 48).clamp(-40, 40) + (wind.signum() * age as i32) / 40;
                let wander = sin_q15(Angle((age * 600).wrapping_add(hc) as u16)).0 * 2 / 32768;
                let size = (1 + age * 5 / PUFF) as usize;
                // Thick off the pot, thinning only over its last half.
                let alpha = if age < PUFF / 2 { 235 } else { 235 - ((age - PUFF / 2) * 470 / PUFF) as i32 };
                let look = self.puffs[(size.min(6) - 1) * 2 + (k as usize & 1)];
                self.actors.push(Actor {
                    x: x + lean + wander,
                    y: foot,
                    up: foot - top + up,
                    look,
                    mirror: (hc >> k) & 1 == 1,
                    alpha: alpha.clamp(0, 255) as u8,
                    perch: false,
                    flat: false,
                    key: 0x10_0000 | (id & 0xffff) << 3 | k,
                });
            }
        }
        // Moths round the lamps at night: three pale motes on loops round the glass.
        if outdoors && view.lamps_lit() && !wet {
            for &(id, x, gy) in &self.lamps {
                if !in_view(x, gy, CELL) {
                    continue;
                }
                for k in 0..3u32 {
                    let hm = h32(id, k, seed ^ 0x6d6f);
                    let a = Angle(t.wrapping_mul(500 + hm % 400).wrapping_add(hm) as u16);
                    let b = Angle(t.wrapping_mul(1300 + (hm >> 9) % 700).wrapping_add(hm >> 3) as u16);
                    let r = 5 + (hm >> 16) as i32 % 5;
                    let dx = cos_q15(a).0 * r / 32768 + sin_q15(b).0 * 2 / 32768;
                    let dy = sin_q15(a).0 * r / 65536 + cos_q15(b).0 * 2 / 32768;
                    let flap = (t / 2 + k) % 3 == 0;
                    self.parts.push((
                        Particle {
                            x: (x + dx) as i16,
                            y: (gy + dy - 2) as i16,
                            shape: PartShape::Dot { size: if flap { 2 } else { 1 } },
                            colour: [236, 222, 188],
                            alpha: 230,
                            glow: 150,
                            height: 30,
                        },
                        false,
                    ));
                }
            }
        }
        // The Works: dust hangs in the air and drifts.
        if view.region() == Region::Works || matches!(view.zone(), jane_core::ZoneId::Factory) {
            let n = match self.tier {
                Tier::T0 => 10,
                Tier::T1 => 20,
                Tier::T2 => 36,
            };
            for k in 0..n {
                let hd = h32(k, 0, seed ^ 0x6475);
                let life = 400 + hd % 400;
                let at = t.wrapping_add(hd >> 8);
                let age = at % life;
                let cyc = at / life;
                let hs = h32(k, cyc, seed ^ 0x6476);
                let x = vx + (hs % vw.max(1) as u32) as i32 + (wind * age as i32) / 64 + age as i32 / 40;
                let y = vy + ((hs >> 12) % vh.max(1) as u32) as i32 - age as i32 / 30;
                let fade = (age.min(life - age) * 4).min(140) as u8;
                self.parts.push((
                    Particle {
                        x: x as i16,
                        y: y as i16,
                        shape: PartShape::Dot { size: 1 },
                        colour: [214, 200, 170],
                        alpha: fade,
                        glow: 40,
                        height: 20,
                    },
                    false,
                ));
            }
        }
        self.water(view, cx, &cells, &tile, day, wet, hour);
        // The cap: the nearest the view's middle first, so what goes is at the edges.
        let n_cap = cap(self.tier);
        let dist = |a: &Actor| (a.x - mid.0).abs() + (a.y - a.up - mid.1).abs();
        let (flat, mut living): (Vec<Actor>, Vec<Actor>) = self.actors.drain(..).partition(|a| a.flat);
        // Smoke's five puffs are one actor, and the chimneys nearest the middle take up to half
        // the cap first (the mood of a lived-in street); then the rest, nearest first.
        living.sort_by_key(|a| (dist(a), a.key));
        let plume_of = |a: &Actor| (a.key & 0x10_0000 != 0).then_some(a.key >> 3);
        let mut plumes: Vec<u32> = Vec::new();
        for a in &living {
            if let Some(p) = plume_of(a).filter(|p| !plumes.contains(p)) {
                if plumes.len() < n_cap / 2 {
                    plumes.push(p);
                }
            }
        }
        let mut count = plumes.len();
        let mut kept = Vec::with_capacity(living.len());
        for a in living {
            match plume_of(&a) {
                Some(p) => {
                    if plumes.contains(&p) {
                        kept.push(a);
                    }
                }
                None if count < n_cap => {
                    count += 1;
                    kept.push(a);
                }
                None => {}
            }
        }
        self.actors = kept;
        self.actors.extend(flat);
    }

    /// A bird of flock `f`: on the ground, landing, leaving or flushed. `arrive` is, for a flock
    /// kept for rule 5, the ticks since it was chosen (it flies in over its first [`LAND`]).
    #[allow(clippy::too_many_arguments)]
    fn bird(
        &mut self,
        f: &Flock,
        i: u32,
        (x, y, mirror, pose): (i32, i32, bool, CPose),
        t: u32,
        arrive: Option<u32>,
        gone: Option<u32>,
        her: Option<(i32, i32)>,
    ) {
        let hb = h32(f.key, i, 0x6269);
        let (_, ep_t) = f.epoch_t(t);
        let key = 0x20_0000 | (f.key & 0x3fff) << 4 | i;
        let speed = if f.kind == Critter::Pigeon { 2 } else { 3 };
        let beat = if (t / 3 + i) % 2 == 0 { CPose::Fly1 } else { CPose::Fly2 };
        let on_ground = |s: &mut Self, look: CPose| {
            let look = s.frame(f.kind, look).unwrap_or(0);
            s.actors.push(Actor { x, y, up: 0, look, mirror, alpha: 255, perch: false, flat: false, key });
        };
        if let Some(at) = gone {
            // Flushed: the first goes at once, the rest a beat or two after, heads up till then.
            let wait = if i == 0 { 0 } else { 2 + hb % STAGGER };
            let since = t.wrapping_sub(at);
            if since < wait {
                on_ground(self, CPose::Look);
                return;
            }
            let s = since - wait;
            if s >= FLEE {
                return;
            }
            // Away from her along x, and up the screen unless she comes from above.
            let dir = her.map_or(if hb & 1 == 0 { 1 } else { -1 }, |(hx, _)| if x >= hx { 1 } else { -1 });
            let vy = her.map_or(-1, |(_, hy)| if y <= hy + CELL { -1 } else { 1 });
            let (dx, dy, up, hop) = flee(s, speed, vy);
            let look = self.frame(f.kind, if hop { CPose::Hop } else { beat }).unwrap_or(0);
            let alpha = if s + FADE > FLEE { ((FLEE - s) * 255 / FADE) as u8 } else { 255 };
            self.actors.push(Actor {
                x: x + dir * dx,
                y: y + dy,
                up,
                look,
                mirror: dir < 0,
                alpha,
                perch: false,
                flat: false,
                key,
            });
            return;
        }
        // Flying in at the epoch's start (or when first kept for rule 5), or off at its end.
        let side = if hb & 2 == 0 { 1 } else { -1 };
        let fly = match arrive {
            Some(a) if a < LAND => Some(-((LAND - a) as i32)),
            None if ep_t < LAND => Some(-((LAND - ep_t) as i32)),
            None if ep_t >= EPOCH - LAND => Some((ep_t - (EPOCH - LAND)) as i32),
            _ => None,
        };
        let Some(s) = fly else {
            on_ground(self, pose);
            return;
        };
        // In the air on the same arc as a flushed bird; landing is leaving backwards.
        let a = s.unsigned_abs();
        let (dx, dy, up, hop) = flee(a, speed, -1);
        let look = self.frame(f.kind, if hop { CPose::Hop } else { beat }).unwrap_or(0);
        // Going: fading as it goes; coming: out of nothing.
        let alpha = (255 - a as i32 * 6).clamp(0, 255) as u8;
        self.actors.push(Actor {
            x: x + side * dx,
            y: y + dy,
            up,
            look,
            mirror: (side < 0) ^ (s < 0),
            alpha,
            perch: false,
            flat: false,
            key,
        });
    }

    /// Ducks on open water in block `(bx, by)`: a pair paddling slow loops, now and then a
    /// dabble, tail up.
    fn ducks(&mut self, bx: i32, by: i32, hb: u32, t: u32, tile: &impl Fn(i32, i32) -> Tile, thr: u32) {
        if (hb >> 8) % 256 >= thr {
            return;
        }
        let (cx, cy) = (bx * BLOCK + (hb >> 16) as i32 % BLOCK, by * BLOCK + (hb >> 20) as i32 % BLOCK);
        let open = |x: i32, y: i32| (-1..=1).all(|dy| (-1..=1).all(|dx| tile(x + dx, y + dy) == Tile::Water));
        if !open(cx, cy) {
            return;
        }
        let n = 1 + (hb >> 24) % 2;
        for k in 0..n {
            let hk = h32(hb, k, 0x6475);
            let period = 3000 + hk % 2000;
            let a = Angle((t.wrapping_add(hk) % period * 65536 / period) as u16);
            let r = 12 + (hk >> 8) as i32 % 10;
            let (ox, oy) = (cos_q15(a).0 * r / 32768, sin_q15(a).0 * r / 65536);
            let (x, y) = (cx * CELL + 8 + ox + k as i32 * 14, cy * CELL + 10 + oy + k as i32 * 5);
            if tile(x.div_euclid(CELL), y.div_euclid(CELL)) != Tile::Water {
                continue;
            }
            // Heading: along the loop (anticlockwise), so east on its lower half.
            let mirror = sin_q15(a).0 < 0;
            let beat = t.wrapping_add(hk) / 24 % 2;
            let dabble = t.wrapping_add(hk) / 90 % 11 == 0;
            let pose = if dabble {
                CPose::Peck
            } else if beat == 0 {
                CPose::Stand
            } else {
                CPose::Look
            };
            let kind = if k == 0 { Critter::Drake } else { Critter::Duck };
            let look = self.frame(kind, pose).unwrap_or(0);
            self.actors.push(Actor {
                x,
                y,
                up: 0,
                look,
                mirror,
                alpha: 255,
                perch: false,
                flat: false,
                key: 0x30_0000 | ((hb & 0xfff) << 2) | k,
            });
        }
    }

    /// A crow on a fence or a dry-stone wall, turned to watch her when she is near; or, by day,
    /// a cat sitting on a wall.
    #[allow(clippy::too_many_arguments)]
    fn perched(
        &mut self,
        bx: i32,
        by: i32,
        hb: u32,
        t: u32,
        tile: &impl Fn(i32, i32) -> Tile,
        her: Option<(i32, i32)>,
        creatures: &Creatures,
        (thr, crows): (u32, u32),
        day: bool,
    ) {
        let roll = (hb >> 4) % 512;
        if roll >= thr.max(crows) {
            return;
        }
        // The first fence or wall cell on a hashed walk through the block.
        let start = (hb >> 12) as i32;
        let cell = (0..BLOCK * BLOCK).map(|k| (start + k * 7) % (BLOCK * BLOCK)).find_map(|k| {
            let (x, y) = (bx * BLOCK + k % BLOCK, by * BLOCK + k / BLOCK);
            matches!(tile(x, y), Tile::Fence | Tile::StoneWall).then_some((x, y))
        });
        let Some((cx, cy)) = cell else { return };
        let (x, y) = (cx * CELL + 8, cy * CELL + 13);
        let cat = roll < thr && day && tile(cx, cy) == Tile::StoneWall && hb % 3 == 0 && !self.cats.is_empty();
        if cat {
            let set = self.cats[(hb >> 9) as usize % self.cats.len()];
            let pose = creatures::Pose {
                facing: Face8::South,
                anim: 0,
                still: creatures::IDLE_AFTER + t.wrapping_add(hb) % 600,
                tick: t,
                dead: false,
                attack: None,
                id: hb & 0xffff,
            };
            let (look, mirror) = creatures.frame(set, pose);
            self.actors.push(Actor {
                x,
                y,
                up: 0,
                look,
                mirror,
                alpha: 255,
                perch: true,
                flat: false,
                key: 0x40_0000 | (hb & 0xffff),
            });
            return;
        }
        let Some(set) = self.crow.filter(|_| roll < crows) else { return };
        // A lone crow; a pair one time in eight.
        let n = 1 + u32::from((hb >> 28) % 8 == 0);
        for k in 0..n {
            let (x, y) = (x + k as i32 * 11 - 5 * (n as i32 - 1), y);
            if !matches!(tile(x.div_euclid(CELL), cy), Tile::Fence | Tile::StoneWall) {
                continue;
            }
            // Within ten cells she is watched: it turns to her and holds still. Else it looks
            // about: the idle pair, or a turn now and then.
            let watching = her.filter(|(hx, hy)| (hx - x).abs() < 10 * CELL && (hy - y).abs() < 8 * CELL);
            let facing = match watching {
                Some((hx, hy)) => face_to(hx - x, hy - y),
                None => Face8::ALL[(h32(hb, (t + k * 300) / 400, 0x6372) % 8) as usize],
            };
            let pose = creatures::Pose {
                facing,
                anim: 0,
                still: if watching.is_some() || (t / 200 + k) % 3 != 0 { 0 } else { creatures::IDLE_AFTER + t % 200 },
                tick: t,
                dead: false,
                attack: None,
                id: (hb ^ k) & 0xffff,
            };
            let (look, mirror) = creatures.frame(set, pose);
            self.actors.push(Actor {
                x,
                y,
                up: 0,
                look,
                mirror,
                alpha: 255,
                perch: true,
                flat: false,
                key: 0x50_0000 | ((hb & 0xffff) << 2) | k,
            });
        }
    }

    /// Butterflies over a flower bed: one or two, wandering loops a hand over the flowers.
    fn butterflies(&mut self, bx: i32, by: i32, hb: u32, t: u32, tile: &impl Fn(i32, i32) -> Tile, thr: u32) {
        if (hb >> 13) % 256 >= thr {
            return;
        }
        let start = (hb >> 3) as i32;
        let cell = (0..BLOCK * BLOCK).map(|k| (start + k * 5) % (BLOCK * BLOCK)).find_map(|k| {
            let (x, y) = (bx * BLOCK + k % BLOCK, by * BLOCK + k / BLOCK);
            matches!(tile(x, y), Tile::FlowerBed | Tile::Garden).then_some((x, y))
        });
        let Some((cx, cy)) = cell else { return };
        let n = 1 + (hb >> 27) % 2;
        for k in 0..n {
            let hk = h32(hb, k, 0x6275);
            let kind = [Critter::White, Critter::Admiral, Critter::Tortoiseshell][(hk % 3) as usize];
            let a = Angle(t.wrapping_mul(180 + hk % 120).wrapping_add(hk) as u16);
            let b = Angle(t.wrapping_mul(470 + (hk >> 8) % 200).wrapping_add(hk >> 4) as u16);
            let dx = cos_q15(a).0 * 20 / 32768 + sin_q15(b).0 * 6 / 32768;
            let dy = sin_q15(a).0 * 8 / 32768 + cos_q15(b).0 * 3 / 32768;
            let up = 10 + sin_q15(Angle(t.wrapping_mul(900).wrapping_add(hk) as u16)).0 * 4 / 32768;
            let beat = (t / 3 + k) % 4;
            let pose = match beat {
                0 => CPose::Fly1,
                1 | 3 => CPose::Fly2,
                _ => CPose::Glide,
            };
            let look = self.frame(kind, pose).unwrap_or(0);
            self.actors.push(Actor {
                x: cx * CELL + 8 + dx,
                y: cy * CELL + 10 + dy,
                up,
                look,
                mirror: cos_q15(a).0 < 0,
                alpha: 255,
                perch: false,
                flat: false,
                key: 0x60_0000 | ((hb & 0xffff) << 2) | k,
            });
        }
    }

    /// The water's own life (ART-PLAN B4): foam along the shore in three beats, glints on open
    /// water by day, lily pads at the margins, a fish rising now and then, rings at her feet on
    /// the stepping stones.
    #[allow(clippy::too_many_arguments)]
    fn water(
        &mut self,
        view: &View<'_>,
        cx: &Ctx<'_>,
        cells: &Rect,
        tile: &impl Fn(i32, i32) -> Tile,
        day: bool,
        wet: bool,
        hour: u32,
    ) {
        let t = cx.tick;
        let seed = view.seed() ^ SALT ^ 0x7761;
        let (vx, vy, vw, vh) = cx.view_px;
        let foam_every = if self.tier == Tier::T0 { 4 } else { 3 };
        let beat = t / 14;
        let is_water = |x: i32, y: i32| tile(x, y) == Tile::Water;
        for y in cells.y..cells.y + cells.h {
            for x in cells.x..cells.x + cells.w {
                if !is_water(x, y) {
                    continue;
                }
                let (px, py) = (x * CELL, y * CELL);
                if px + CELL < vx - 8 || py + CELL < vy - 8 || px > vx + vw + 8 || py > vy + vh + 8 {
                    continue;
                }
                let hc = h32(x as u32, y as u32, seed);
                let land = |dx: i32, dy: i32| {
                    let t = tile(x + dx, y + dy);
                    t != Tile::Water && t != Tile::Void
                };
                let edges = [(0, -1), (0, 1), (-1, 0), (1, 0)];
                let margin = edges.iter().any(|&(dx, dy)| land(dx, dy));
                // Foam: a lace of light dots a px or two off the bank, in three beats.
                for (e, &(dx, dy)) in edges.iter().enumerate() {
                    if !land(dx, dy) {
                        continue;
                    }
                    for k in 0..CELL {
                        let hk = h32(hc, k as u32 * 4 + e as u32, 0x666f);
                        if (hk.wrapping_add(beat)) % foam_every != 0 {
                            continue;
                        }
                        // Off the bank: under a bank to the north the water is in its shadow,
                        // so the foam lies further out.
                        let off = 1 + (hk >> 8) as i32 % 2 + if dy < 0 { 2 } else { 0 };
                        let (fx, fy) = match (dx, dy) {
                            (0, -1) => (px + k, py + off),
                            (0, _) => (px + k, py + CELL - 1 - off),
                            (-1, _) => (px + off, py + k),
                            _ => (px + CELL - 1 - off, py + k),
                        };
                        self.parts.push((
                            Particle {
                                x: fx as i16,
                                y: fy as i16,
                                shape: PartShape::Dot { size: 1 },
                                colour: [222, 236, 236],
                                alpha: 120 + (hk >> 12) as u8 % 90,
                                glow: 0,
                                height: 0,
                            },
                            true,
                        ));
                    }
                }
                if margin {
                    // Lily pads: on one margin cell in three, in the still water off the bank.
                    // Only off a natural bank: a jetty's edge keeps none.
                    let bank = edges.iter().any(|&(dx, dy)| {
                        land(dx, dy) && !matches!(tile(x + dx, y + dy), Tile::Boardwalk | Tile::Stepping)
                    });
                    if hc % 4 == 0 && bank {
                        let n = 1 + (hc >> 4) % 2;
                        for k in 0..n {
                            let hk = h32(hc, k, 0x6c69);
                            // Off the bank's lip: pushed away from whichever side is land.
                            let (mut ox, mut oy) = (2 + (hk % 12) as i32, 3 + ((hk >> 4) % 10) as i32);
                            for &(dx, dy) in &edges {
                                if land(dx, dy) {
                                    ox -= dx * 4;
                                    oy -= dy * 4;
                                }
                            }
                            let fx = px + ox.clamp(2, 14);
                            let fy = py + oy.clamp(3, 13);
                            let bob = i32::from((t / 50).wrapping_add(hk) % 7 == 0);
                            self.actors.push(Actor {
                                x: fx,
                                y: fy + bob,
                                up: 0,
                                look: self.pads[(hk >> 8) as usize % self.pads.len()],
                                mirror: hk >> 12 & 1 == 1,
                                alpha: 255,
                                perch: false,
                                flat: true,
                                key: 0x70_0000 | ((hc & 0xffff) << 2) | k,
                            });
                        }
                    }
                    continue;
                }
                // Glints of the sun on open water: a short white spark at a hashed px.
                if day && !wet && view.weather().kind == jane_sim::state::WeatherKind::Clear {
                    let every = if self.tier == Tier::T0 { 360 } else { 200 };
                    let at = t.wrapping_add(hc) % every;
                    if at < 10 {
                        let hk = h32(hc, t.wrapping_add(hc) / every, 0x676c);
                        let (gx, gy) = (px + (hk % 14) as i32 + 1, py + ((hk >> 4) % 14) as i32 + 1);
                        let a = (if at < 5 { at * 50 } else { (10 - at) * 50 }).min(255) as u8;
                        for (sx, sy, size, al) in [(0, 0, 1, a), (-1, 0, 1, a / 2), (1, 0, 1, a / 2)] {
                            self.parts.push((
                                Particle {
                                    x: (gx + sx) as i16,
                                    y: (gy + sy) as i16,
                                    shape: PartShape::Dot { size },
                                    colour: [255, 250, 230],
                                    alpha: al,
                                    glow: 200,
                                    height: 0,
                                },
                                true,
                            ));
                        }
                    }
                }
                // A fish rises: a ring now and then, on still water, dawn and dusk most.
                let rises = if (6..9).contains(&hour) || (16..20).contains(&hour) { 41 } else { 97 };
                if !wet && h32(hc, t / 60, 0x6669) % (rises * 6) == 0 && t % 60 == hc % 60 {
                    self.rings.push((px + 3 + (hc % 10) as i32, py + 4 + (hc >> 4) as i32 % 8, t, 9));
                }
            }
        }
        // Rings at her feet where she walks on the stepping stones, or on sand at the water.
        if let Some(((hx, hy), true)) = cx.her {
            let (cx_, cy_) = (hx.div_euclid(CELL), hy.div_euclid(CELL));
            let here = tile(cx_, cy_);
            let by_water = [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| is_water(cx_ + dx, cy_ + dy));
            if (here == Tile::Stepping || here == Tile::Sand && by_water || here == Tile::Boardwalk && by_water)
                && t % 18 == 0
            {
                self.rings.push((hx, hy, t, 7));
            }
        }
        for &(x, y, at, r) in &self.rings {
            let age = t.wrapping_sub(at) as i32;
            if age >= 60 {
                continue;
            }
            let rr = 1 + age * i32::from(r) / 60;
            let alpha = (200 - age * 3).clamp(0, 255) as u8;
            self.parts.push((
                Particle {
                    x: x as i16,
                    y: y as i16,
                    shape: PartShape::Ring { r: rr as u8 },
                    colour: [210, 230, 236],
                    alpha,
                    glow: 0,
                    height: 0,
                },
                true,
            ));
            if age < 4 {
                // The splash's drop at its middle.
                self.parts.push((
                    Particle {
                        x: x as i16,
                        y: (y - 2) as i16,
                        shape: PartShape::Dot { size: 1 },
                        colour: [236, 246, 246],
                        alpha: 220,
                        glow: 0,
                        height: 2,
                    },
                    false,
                ));
            }
        }
    }

    /// The layer's particles into the frame: below T2 those that do not glow go under the light
    /// (`under`), the rest over it, coloured for the tier as [`crate::fx`]'s are.
    pub fn draw_parts(&self, f: &mut Frame, cam: (i32, i32), under: bool, sky: Option<&crate::light::Sky>) {
        let t2 = f.tier >= Tier::T2;
        if under && t2 {
            return;
        }
        let (cw, ch) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        for ground in [true, false] {
            let p0 = f.parts.len();
            for (p, g) in &self.parts {
                if *g != ground || (under != (!t2 && p.glow == 0)) {
                    continue;
                }
                let (x, y) = (i32::from(p.x) - cam.0, i32::from(p.y) - cam.1);
                if x < -16 || y < -16 || x > cw + 16 || y > ch + 16 {
                    continue;
                }
                let colour = if under { p.colour } else { lit(f.tier, p.colour, p.glow, sky) };
                f.parts.push(Particle { x: x as i16, y: y as i16, colour, ..*p });
            }
            if f.parts.len() > p0 {
                let layer = if ground { Depth::Ground } else { Depth::Canopy };
                f.passes.push(Pass::Particles { layer, parts: Span::since(p0, f.parts.len()) });
            }
        }
    }
}

/// A colour as a tier draws it over the light (as `fx::Fx::lit`): T2 lights its parts itself;
/// below it the unlit share is multiplied by the frame's flat light.
fn lit(tier: Tier, c: Rgb, glow: u8, sky: Option<&crate::light::Sky>) -> Rgb {
    let Some(sky) = sky.filter(|_| tier < Tier::T2) else { return c };
    let g = u32::from(glow);
    [0, 1, 2].map(|k| {
        let unlit = (u32::from(c[k]) * (u32::from(sky.ambient[k]) + 16)) >> 8;
        ((unlit * (255 - g) + u32::from(c[k]) * g) / 255).min(255) as u8
    })
}

/// The sector a vector `(dx, dy)` points into (screen y down).
fn face_to(dx: i32, dy: i32) -> Face8 {
    let (ax, ay) = (dx.abs(), dy.abs());
    // Within about 22 degrees of an axis, that axis; else the diagonal.
    let diag = ax * 5 > ay * 2 && ay * 5 > ax * 2;
    match (diag, dx >= 0, dy >= 0, ax >= ay) {
        (true, true, true, _) => Face8::SouthEast,
        (true, false, true, _) => Face8::SouthWest,
        (true, true, false, _) => Face8::NorthEast,
        (true, false, false, _) => Face8::NorthWest,
        (false, true, _, true) => Face8::East,
        (false, false, _, true) => Face8::West,
        (false, _, true, false) => Face8::South,
        (false, _, false, false) => Face8::North,
    }
}

/// A flock of ground birds that may land in a block this epoch.
#[derive(Clone, Copy, Debug)]
struct Flock {
    key: u32,
    kind: Critter,
    /// Its spot, zone px.
    x: i32,
    y: i32,
    n: u32,
    /// Its rank among candidates: the lowest is kept for rule 5.
    rank: u32,
    /// Ticks into the epoch it starts at.
    off: u32,
    /// The salt of its block.
    hb: u32,
    /// Whether it is where people live (a lived-in house, the square): only there do birds land
    /// of their own accord; elsewhere one comes only as rule 5's, alone.
    home: bool,
}

impl Flock {
    /// Whether it lands this epoch, of 256 `thr`.
    fn present(&self, thr: u32) -> bool {
        self.home && h32(self.hb, self.epoch_t(0).0, 0x7072) % 256 < thr
    }

    /// The epoch it is in at tick `t` (the `t` passed to `present` is ignored) and the ticks into it.
    fn epoch_t(&self, t: u32) -> (u32, u32) {
        let at = t.wrapping_add(self.off);
        (at / EPOCH, at % EPOCH)
    }

    /// Bird `i` at tick `t`: its ground point, whether it faces west, and its pose; `None` if
    /// its spot is no ground for it.
    fn bird(&self, i: u32, t: u32, forced: bool, tile: &impl Fn(i32, i32) -> Tile) -> Option<(i32, i32, bool, CPose)> {
        let hb = h32(self.key, i, 0x6269);
        let (e, _) = self.epoch_t(t);
        let _ = forced;
        let he = h32(hb, e, 0x6570);
        let (bx, by) = (self.x + (he % 33) as i32 - 16, self.y + ((he >> 6) % 17) as i32 - 8);
        // Its business, a beat at a time: peck, look about, hop on a few px, turn.
        let seg_len = 40 + hb % 40;
        let at = t.wrapping_add(hb);
        let seg = at / seg_len;
        let into = at % seg_len;
        let hs = |s: u32| h32(hb, s, 0x7367);
        let jitter = |s: u32| {
            let h = hs(s);
            ((h % 13) as i32 - 6, ((h >> 4) % 7) as i32 - 3)
        };
        let (j0, j1) = (jitter(seg.wrapping_sub(1)), jitter(seg));
        let act = hs(seg) >> 8 & 7;
        let hop = act >= 6;
        let (jx, jy, lift) = if hop && into < 8 {
            let k = into as i32;
            (j0.0 + (j1.0 - j0.0) * k / 8, j0.1 + (j1.1 - j0.1) * k / 8, 2 - (k - 4).abs() / 2)
        } else if hop {
            (j1.0, j1.1, 0)
        } else {
            (j0.0, j0.1, 0)
        };
        let (x, y) = (bx + if hop { jx } else { j0.0 }, by + if hop { jy } else { j0.1 });
        let cell = tile(x.div_euclid(CELL), y.div_euclid(CELL));
        if !bird_ground(cell) {
            return None;
        }
        let mirror = hs(seg) >> 12 & 1 == 1;
        let pose = if hop && into < 8 {
            CPose::Hop
        } else {
            match act {
                0..=3 => {
                    if (into / 9) % 2 == 0 {
                        CPose::Peck
                    } else {
                        CPose::Stand
                    }
                }
                4 | 5 => CPose::Look,
                _ => CPose::Stand,
            }
        };
        let _ = lift;
        Some((x, y - lift, mirror, pose))
    }
}

/// A bird `s` ticks into a take-off, flying at about `speed` px a tick, drifting `vy` (-1 up the
/// screen, 1 down): how far it has gone along its heading, down the screen, up off the ground,
/// and whether it is still in its hop. A spring for [`SPRING`] ticks, then the wings open and it
/// climbs on an arc that steepens, so it is out of the view, or nearly, before it fades. Pure:
/// the take-off is one and the same on every machine.
fn flee(s: u32, speed: i32, vy: i32) -> (i32, i32, i32, bool) {
    let s = s as i32;
    let spring = SPRING as i32;
    if s < spring {
        // The hop: up 0, 2, 4, 5, 6, 6 px, a px or two along.
        let up = [0, 2, 4, 5, 6, 6][s as usize];
        return (s / 2, 0, up, true);
    }
    let k = s - spring;
    let dx = spring / 2 + k * speed + k * k / 30;
    let up = 6 + k * 3 / 2 + k * k / 28;
    (dx, vy * k / 3, up, false)
}

/// Ground a bird pecks over.
fn bird_ground(t: Tile) -> bool {
    t.flags() & F_SOLID == 0
        && matches!(
            t,
            Tile::Grass
                | Tile::Dirt
                | Tile::Road
                | Tile::Sand
                | Tile::GrownPath
                | Tile::Cobble
                | Tile::FlowerBed
                | Tile::Crops
                | Tile::Garden
        )
}

/// The flock that may land in block `(bx, by)`, if it has ground for one: its kind by the
/// ground and the region (pigeons on the setts and in the Works, a robin in a garden, sparrows).
#[allow(clippy::too_many_arguments)]
fn ground_flock(
    view: &View<'_>,
    bx: i32,
    by: i32,
    hb: u32,
    t: u32,
    tile: &impl Fn(i32, i32) -> Tile,
    home: &impl Fn(i32, i32) -> bool,
) -> Option<Flock> {
    let _ = t;
    // A spot: the first open ground on a hashed walk through the block, with ground round it.
    let start = hb as i32 & 0xff;
    let (cx, cy) = (0..BLOCK * BLOCK).map(|k| (start + k * 11) % (BLOCK * BLOCK)).find_map(|k| {
        let (x, y) = (bx * BLOCK + k % BLOCK, by * BLOCK + k / BLOCK);
        let open = bird_ground(tile(x, y)) && bird_ground(tile(x + 1, y)) && bird_ground(tile(x - 1, y));
        open.then_some((x, y))
    })?;
    let ground = tile(cx, cy);
    // Birds land only where people live: by a lived-in house, or on the square's setts.
    let home = ground == Tile::Cobble || home(cx * CELL + 8, cy * CELL + 8);
    let region = view.region_at(cx, cy);
    let kind = match (region, ground) {
        (Region::Works, _) => Critter::Pigeon,
        (_, Tile::Cobble | Tile::Road) => {
            if hb >> 20 & 1 == 0 {
                Critter::Pigeon
            } else {
                Critter::Sparrow
            }
        }
        (_, Tile::FlowerBed | Tile::Garden) if (hb >> 21) % 4 == 0 => Critter::Robin,
        _ => Critter::Sparrow,
    };
    // Mostly one; two one time in eight, three one in sixteen.
    let n = match kind {
        Critter::Robin => 1,
        _ => match (hb >> 24) % 16 {
            0 => 3,
            1 | 2 => 2,
            _ => 1,
        },
    };
    Some(Flock {
        key: hb & 0x7fff_ffff,
        kind,
        x: cx * CELL + 8,
        y: cy * CELL + 10,
        n,
        rank: h32(hb, 0, 0x726b),
        off: hb % EPOCH,
        hb,
        home,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vector_faces_its_sector() {
        assert_eq!(face_to(10, 0), Face8::East);
        assert_eq!(face_to(-10, 1), Face8::West);
        assert_eq!(face_to(0, 10), Face8::South);
        assert_eq!(face_to(1, -10), Face8::North);
        assert_eq!(face_to(10, 10), Face8::SouthEast);
        assert_eq!(face_to(-7, -8), Face8::NorthWest);
    }

    /// The owner's playtest (2026-10-07): a bird put up "blips somewhere else". A take-off is a
    /// hop, then a climbing arc away: no tick moves it further than a wing beat's worth.
    #[test]
    fn a_take_off_never_jumps() {
        for speed in [2, 3] {
            let mut last = flee(0, speed, -1);
            assert_eq!(last, (0, 0, 0, true), "it leaves from where it stood");
            for s in 1..FLEE {
                let now = flee(s, speed, -1);
                let (dx, dy, du) = (now.0 - last.0, now.1 - last.1, now.2 - last.2);
                assert!(
                    (0..=8).contains(&dx) && (-1..=0).contains(&dy) && (0..=8).contains(&du),
                    "tick {s}: {last:?} to {now:?}"
                );
                last = now;
            }
            assert!(last.0 > 3 * CELL && last.2 > 6 * CELL, "it is well away before it fades: {last:?}");
        }
    }

    #[test]
    fn the_caps_are_the_plans() {
        assert_eq!([cap(Tier::T0), cap(Tier::T1), cap(Tier::T2)], [12, 24, 48]);
    }
}
