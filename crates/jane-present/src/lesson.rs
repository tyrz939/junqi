//! The moment a gift is found (PRESENTATION.md §2.1, §3.2, §5.1): a spell learned, or a jar or
//! a page that leaves her a little stronger. Growth in Castle comes only from finding things
//! (`PLAN.md`: there is no experience to earn), so each finding is given a moment of its own,
//! and the first spell she ever learns the longest of them.
//!
//! A spell, beat by beat: the world hushes (the grade drains a little colour and light, the
//! music and the weather step back, and alone the world holds still for a breath); she turns to
//! us and holds her hands out, and motes of the spell's school spiral in to them while the
//! cast's glow and a light between her hands swell; the light blooms, a ring widens on the
//! ground and the motes lift away; the card comes up (`ui::lesson`); the icon flies down to the
//! bar slot the sim bound it to, and the slot glints. The first spell is the same, longer and
//! deeper, with a line about what it is. A jar or a page is the same language, smaller: no hold,
//! no card, a little light and the words above her, and the gauge it grew glints.
//!
//! Presentation only: it reads the `View` and the tick's events and writes the presenter's frame,
//! and nothing here reaches the sim. Two moments never overlap: a second gift waits its turn, and
//! every gift waits while her conversation is open (so the orb's words are read before its
//! moment) or while she is down. The dice are the presenter's own `Lcg`, never the sim's.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use jane_art::fx::{Lcg, Shape, Spark, school_ramp};
use jane_art::palette::{self, Ramp, Tone};
use jane_core::action::{Facing, School, Stat};
use jane_core::angle::{cos_q15, sin_q15};
use jane_core::{Angle, SpellId};
use jane_sim::event::{Event, EventKind, ToastKind, events_for};
use jane_sim::view::View;

use crate::frame::{Depth, Frame, Light, LightKind, PartShape, Particle, Pass, Post, Rgb, Span, Tier};

/// What was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gift {
    /// A spell the table learned; `first` when it is the first the world has ever learned (every
    /// seat learns it at once, so it is the first for each of them: `View::learned`).
    Spell { spell: SpellId, school: School, first: bool },
    /// A jar (strength) or a page (spirit).
    Growth(Stat),
}

impl Gift {
    /// Its beats.
    pub const fn beats(self) -> Beats {
        match self {
            Gift::Spell { first: true, .. } => FIRST,
            Gift::Spell { .. } => SPELL,
            Gift::Growth(_) => GROWTH,
        }
    }

    /// The ramp its light is drawn in: the spell's school's, or the stat's (gold for strength,
    /// pale ice for spirit).
    pub const fn ramp(self) -> Ramp {
        match self {
            Gift::Spell { school, .. } => school_ramp(school),
            Gift::Growth(Stat::Strength) => Ramp::UiGold,
            Gift::Growth(Stat::Spirit) => Ramp::Ice,
        }
    }
}

/// A moment's beats, presenter ticks from its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beats {
    /// Ticks the hush takes to settle.
    pub hush_in: u32,
    /// The gather ends, the light blooms, and the card (or the words) come up.
    pub bloom: u32,
    /// Alone, the world is held still until here; with company, never (ENGINE.md §4).
    pub hold: u32,
    /// The first spell's line comes up (0: none).
    pub line: u32,
    /// The icon leaves the card, and lands on the bar (0: no flight).
    pub fly: u32,
    pub land: u32,
    /// The hush begins to lift, and the moment is over.
    pub lift: u32,
    pub end: u32,
    /// How deep the hush goes, of 255.
    pub depth: u8,
    /// How far out the motes gather from, px.
    pub reach: u8,
    /// Motes a tick at the gather's height.
    pub motes: u8,
    /// The light's reach at its bloom, px, and the ticks it takes to fade.
    pub light: u16,
    pub fade: u32,
    /// The music and the beds at their lowest, of 255 of their level.
    pub duck: u8,
}

/// A spell: about six seconds, the world held for the first one and a half.
pub const SPELL: Beats = Beats {
    hush_in: 24,
    bloom: 60,
    hold: 96,
    line: 0,
    fly: 300,
    land: 336,
    lift: 300,
    end: 372,
    depth: 150,
    reach: 44,
    motes: 2,
    light: 150,
    fade: 100,
    duck: 80,
};

/// The first spell she ever learns: a longer, deeper hush (the world held four seconds), a
/// wider gather, a fuller light, the card up longer with its line.
pub const FIRST: Beats = Beats {
    hush_in: 48,
    bloom: 150,
    hold: 240,
    line: 250,
    fly: 620,
    land: 660,
    lift: 590,
    end: 700,
    depth: 220,
    reach: 84,
    motes: 3,
    light: 220,
    fade: 170,
    duck: 30,
};

/// A jar or a page: three seconds, nothing held.
pub const GROWTH: Beats = Beats {
    hush_in: 16,
    bloom: 36,
    hold: 0,
    line: 0,
    fly: 0,
    land: 0,
    lift: 140,
    end: 190,
    depth: 60,
    reach: 26,
    motes: 1,
    light: 90,
    fade: 60,
    duck: 160,
};

/// Ticks between one moment's end and the next one's start.
pub const BREATH: u32 = 20;

/// A moment under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub gift: Gift,
    /// Presenter ticks since it began.
    pub age: u32,
}

impl Moment {
    pub const fn beats(&self) -> Beats {
        self.gift.beats()
    }

    /// How deep the hush is now, 0 to the beats' `depth`: eased in, held, eased out.
    pub fn hush(&self) -> u8 {
        let b = self.beats();
        let d = u32::from(b.depth);
        let k = if self.age < b.hush_in {
            ease(self.age, b.hush_in)
        } else if self.age < b.lift {
            256
        } else {
            256 - ease(self.age - b.lift, b.end.saturating_sub(b.lift).max(1))
        };
        (d * k / 256) as u8
    }

    /// How bright the gathered light is (0 to 256) and how far it reaches, px.
    pub fn light(&self) -> (u32, u16) {
        let b = self.beats();
        let reach = u32::from(b.light);
        if self.age < b.bloom {
            // It swells as the motes come in.
            let t = self.age * 256 / b.bloom;
            (48 + 120 * t * t / 65536, (reach * (24 + t * 40 / 256) / 100) as u16)
        } else if self.age < b.bloom + b.fade {
            // The bloom, then a long fall: the light remembers it a while.
            let u = (self.age - b.bloom) * 256 / b.fade;
            let k = (256 - u) * (256 - u) / 256;
            (k, (reach * (100 - u * 30 / 256) / 100) as u16)
        } else {
            (0, 0)
        }
    }

    /// Her hands, while she holds them out: the school, and the tick of the cast's three beats
    /// to show (wind-up, hands out, recover).
    pub fn pose(&self) -> Option<(School, u32)> {
        let Gift::Spell { school, .. } = self.gift else { return None };
        let b = self.beats();
        let act = crate::people::ACT_TICKS;
        let out = b.bloom + 30;
        if self.age < act {
            Some((school, 0))
        } else if self.age < out {
            Some((school, act))
        } else if self.age < out + act {
            Some((school, 2 * act))
        } else {
            None
        }
    }
}

/// `t` of `n` as 0 to 256 on a smooth step.
pub fn ease(t: u32, n: u32) -> u32 {
    let x = (t.min(n) * 256 / n.max(1)) as i64;
    (x * x * (768 - 2 * x) / 65536) as u32
}

/// A mote drawn in to her hands: where it is round them, and how it turns.
#[derive(Clone, Copy, Debug)]
struct Mote {
    ang: u16,
    /// Its distance at birth and its height over the hands at birth, Q4 px.
    r0: i32,
    z0: i32,
    spin: i16,
    age: u8,
    life: u8,
    colour: Rgb,
    size: u8,
}

impl Mote {
    /// Where it stands round the hands `t` 256ths of a tick into its life, Q4 px: across, and up
    /// the screen.
    fn at(&self, t: i32) -> (i32, i32) {
        let life = i32::from(self.life.max(1));
        let t = t.clamp(0, life * 256);
        let left = life * 256 - t;
        // It comes in slowly and then quickly, turning faster as it nears.
        let r = (i64::from(self.r0) * i64::from(left) * i64::from(left) / (i64::from(life) * 256).pow(2)) as i32;
        let turn = i32::from(self.spin) * t / 256 * (256 + 512 * t / (life * 256)) / 256;
        let a = Angle(self.ang.wrapping_add(turn as u16));
        let z = self.z0 * left / (life * 256);
        ((r * cos_q15(a).0) >> 15, ((r * sin_q15(a).0 * 3 / 4) >> 15) - z)
    }
}

/// Where she is for the moment: her feet in the zone, canvas px, which way she faces, and whether
/// she is walking or down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Her {
    pub at: (i32, i32),
    pub facing: Facing,
    pub walking: bool,
    pub alive: bool,
}

/// Where her hands are, from her feet, px: across, and up (the cast's light between them).
pub const fn hands(f: Facing) -> (i32, i32) {
    match f {
        Facing::East => (9, 21),
        Facing::West => (-9, 21),
        Facing::South => (0, 18),
        Facing::North => (0, 19),
    }
}

/// The gifts waiting, the one under way, and its motes.
#[derive(Debug)]
pub struct Lessons {
    queue: VecDeque<Gift>,
    now: Option<Moment>,
    began: Option<Gift>,
    /// Ticks to wait after one moment before the next.
    rest: u32,
    rng: Lcg,
    tier: Tier,
    motes: Vec<Mote>,
    sparks: Vec<Spark>,
    her: Option<Her>,
    /// Where her feet were on the canvas at the last draw (the card keeps clear of her).
    screen: Option<(i32, i32)>,
}

impl Lessons {
    pub fn new(tier: Tier) -> Lessons {
        Lessons {
            queue: VecDeque::with_capacity(4),
            now: None,
            began: None,
            rest: 0,
            rng: Lcg(0x6c65_7373),
            tier,
            motes: Vec::with_capacity(256),
            sparks: Vec::with_capacity(256),
            her: None,
            screen: None,
        }
    }

    /// The moment under way.
    pub fn moment(&self) -> Option<&Moment> {
        self.now.as_ref()
    }

    /// The gift whose moment began this tick (the sound starts with it).
    pub fn began(&self) -> Option<Gift> {
        self.began
    }

    /// Gifts waiting their turn.
    pub fn waiting(&self) -> usize {
        self.queue.len()
    }

    /// Whether the app holds the world this tick: only alone, and only for the moment's first
    /// breath. With company nothing holds (ENGINE.md §4), whatever the moment.
    pub fn holds_world(&self, alone: bool) -> bool {
        alone && self.now.is_some_and(|m| m.age < m.beats().hold)
    }

    /// The music and the beds' share of their level now, of 255 (255: not hushed).
    pub fn duck(&self) -> u8 {
        let Some(m) = self.now else { return 255 };
        let b = m.beats();
        let h = u32::from(m.hush());
        let d = u32::from(b.depth.max(1));
        (255 - (255 - u32::from(b.duck)) * h / d) as u8
    }

    /// How deep the hush is now, 0 to 255.
    pub fn hush(&self) -> u8 {
        self.now.map_or(0, |m| m.hush())
    }

    /// Her pose while she holds her hands out: the school and the cast beat's tick. Only while
    /// she stands: walking (with company the world goes on), she walks.
    pub fn pose(&self) -> Option<(School, u32)> {
        let her = self.her?;
        if her.walking || !her.alive {
            return None;
        }
        self.now.and_then(|m| m.pose())
    }

    /// Where her feet were on the canvas at the last draw.
    pub fn screen(&self) -> Option<(i32, i32)> {
        self.screen
    }

    /// A new zone: the motes do not follow her through a door; the moment does.
    pub fn zone(&mut self, view: &View<'_>) {
        self.motes.clear();
        self.sparks.clear();
        self.rng = Lcg(jane_art::hash::h32(view.seed(), view.zone() as u32, 0x6c65_7373));
    }

    /// One tick: this tick's gifts join the queue, the moment steps, and the next begins when
    /// there is none, she is not talking and she is up.
    pub fn tick(&mut self, view: &View<'_>, events: &[Event], her: Option<Her>) {
        self.began = None;
        self.her = her;
        let cat = jane_data::catalog();
        for e in events_for(events, view.me()) {
            let gift = match e.kind {
                EventKind::Learn(spell) => Gift::Spell {
                    spell,
                    school: cat.combat.spell(spell).school,
                    first: view.learned().first() == Some(&spell),
                },
                EventKind::Toast(ToastKind::Stronger) => Gift::Growth(Stat::Strength),
                EventKind::Toast(ToastKind::WordsStay) => Gift::Growth(Stat::Spirit),
                _ => continue,
            };
            self.queue.push_back(gift);
        }
        self.rest = self.rest.saturating_sub(1);
        if let Some(m) = &mut self.now {
            m.age += 1;
            if m.age >= m.beats().end {
                self.now = None;
                self.rest = BREATH;
            }
        }
        let free = view.me().dialogue.is_none() && her.is_some_and(|h| h.alive);
        if self.now.is_none()
            && self.rest == 0
            && free
            && let Some(gift) = self.queue.pop_front()
        {
            self.now = Some(Moment { gift, age: 0 });
            self.began = Some(gift);
        }
        self.motes.retain_mut(|m| {
            m.age = m.age.saturating_add(1);
            m.age < m.life
        });
        self.sparks.retain_mut(Spark::step);
        if let (Some(m), Some(h)) = (self.now, her) {
            self.shed(m, h);
        }
    }

    /// The moment's parts this tick: motes in during the gather; at the bloom, the ring and the
    /// burst; after it, a last few drifting off.
    fn shed(&mut self, m: Moment, her: Her) {
        let b = m.beats();
        let ramp = m.gift.ramp();
        let tone = |t: Tone| palette::rgb(ramp.at(t));
        // T0's pool is small: half the motes.
        let most = if self.tier == Tier::T0 { 120 } else { 360 };
        let (hx, hz) = hands(if self.pose().is_some() { Facing::South } else { her.facing });
        let hands_at = (her.at.0 + hx, her.at.1);
        if m.age < b.bloom {
            // The gather: more motes, and nearer, as it goes on; none born that cannot arrive.
            let left = b.bloom - m.age;
            let n = if m.age * 3 < b.bloom { 1 } else { u32::from(b.motes) };
            for _ in 0..n {
                if left < 10 || self.motes.len() >= most {
                    break;
                }
                let life = self.rng.range(20, 48).min(left as i32 - 2).max(8) as u8;
                let reach = i32::from(b.reach);
                let r0 = self.rng.range(reach * 2 / 3, reach) * 16;
                let core = self.rng.below(3) == 0;
                self.motes.push(Mote {
                    ang: self.rng.below(65536) as u16,
                    r0,
                    z0: self.rng.range(-6, 14) * 16,
                    spin: self.rng.range(40, 90) as i16 * if self.rng.below(2) == 0 { 1 } else { -1 } * 16,
                    age: 0,
                    life,
                    colour: if core { tone(Tone::Glint) } else { tone(Tone::Light) },
                    size: if core { 2 } else { 1 },
                });
            }
            // The first spell: the ground round her gives up a few of its own, rising.
            if b.motes >= 3 && m.age % 3 == 0 && self.sparks.len() < most {
                let reach = i32::from(b.reach);
                let (dx, dy) = (self.rng.range(-reach, reach), self.rng.range(-reach / 2, reach / 2));
                self.spark(hands_at.0 + dx, hands_at.1 + dy, 0, (0, 0, 10), 70, tone(Tone::Light), tone(Tone::Base), 0);
            }
        } else if m.age == b.bloom || (m.age == b.bloom + 14 && b.motes >= 3) {
            // The bloom: a ring on the ground, and the motes lifted away from the hands.
            let (r1, burst) =
                if m.age == b.bloom { (b.reach, 16 + 8 * b.motes) } else { (b.reach.saturating_add(40), 0) };
            self.sparks.push(Spark {
                x: hands_at.0 * 16,
                y: hands_at.1 * 16,
                z: 0,
                vx: 0,
                vy: 0,
                vz: 0,
                age: 0,
                life: 44,
                shape: Shape::Ring(4, r1),
                colour: tone(Tone::High),
                late: tone(Tone::Base),
                glow: 255,
                grav: 0,
                drag: 255,
                ground: true,
            });
            let (rise, grav) = drift(m.gift);
            for _ in 0..burst {
                if self.sparks.len() >= most {
                    break;
                }
                let a = Angle(self.rng.below(65536) as u16);
                let s = self.rng.range(6, 26);
                let (vx, vy) = ((s * cos_q15(a).0) >> 15, (s * sin_q15(a).0 * 3 / 4) >> 15);
                let core = self.rng.below(3) == 0;
                let c = if core { tone(Tone::Glint) } else { tone(Tone::Light) };
                let life = self.rng.range(50, 100) as u8;
                let vz = rise + self.rng.range(-3, 3);
                self.spark(hands_at.0, hands_at.1, hz, (vx, vy, vz), life, c, tone(Tone::Mid), grav);
            }
        } else if m.age < b.bloom + b.fade / 2 && m.age % 5 == 0 && self.sparks.len() < most {
            // What is left of the light drifts off her hands a while.
            let (rise, grav) = drift(m.gift);
            let (vx, vy) = (self.rng.range(-6, 6), self.rng.range(-4, 4));
            self.spark(hands_at.0, hands_at.1, hz, (vx, vy, rise), 60, tone(Tone::Light), tone(Tone::Mid), grav);
        }
    }

    /// A free part of the moment at ground point `(x, y)`, `z` px up.
    #[allow(clippy::too_many_arguments)]
    fn spark(&mut self, x: i32, y: i32, z: i32, (vx, vy, vz): (i32, i32, i32), life: u8, c: Rgb, late: Rgb, grav: i8) {
        self.sparks.push(Spark {
            x: x * 16,
            y: y * 16,
            z: z * 16,
            vx,
            vy,
            vz,
            age: 0,
            life,
            shape: Shape::Dot(1 + u8::from(self.rng.below(4) == 0)),
            colour: c,
            late,
            glow: 255,
            grav,
            drag: 250,
            ground: false,
        });
    }

    /// The grade under the hush: a little colour and a little light drained from the world, so
    /// what she holds is the brightest thing in it.
    pub fn grade(&self, p: Post) -> Post {
        let h = u32::from(self.hush());
        if h == 0 {
            return p;
        }
        let sat = u32::from(p.saturation);
        let exp = u32::from(p.exposure);
        Post {
            saturation: (sat - sat * h * 40 / (100 * 255)) as u8,
            exposure: (exp - exp * h * 14 / (100 * 255)) as u8,
            ..p
        }
    }

    /// The light between her hands, into the frame's list: `feet` her feet on the canvas at this
    /// frame's alpha, `holder` her sprite's draw key (a light never shadows who holds it).
    pub fn lights(&self, f: &mut Frame, feet: Option<(i32, i32)>, holder: Option<u32>) {
        let (Some(m), Some(feet), Some(her)) = (self.now, feet, self.her) else { return };
        let (k, radius) = m.light();
        if k == 0 || radius == 0 {
            return;
        }
        let (hx, hz) = hands(if self.pose().is_some() { Facing::South } else { her.facing });
        let c = palette::rgb(m.gift.ramp().at(Tone::Light));
        f.lights.push(Light {
            pos: (feet.0 + hx, feet.1),
            height: hz as u8,
            colour: c.map(|v| (u32::from(v) * k.min(256) / 256) as u8),
            radius,
            size: 4,
            casts: true,
            kind: LightKind::Point,
            holder,
        });
    }

    /// The moment's parts into the frame, over the effects in the air: the motes round her hands
    /// (they follow her), the glow between them, and the free parts (the burst, the ring).
    pub fn draw(&mut self, f: &mut Frame, cam: (i32, i32), alpha: u8, feet: Option<(i32, i32)>) {
        self.screen = feet;
        let p0 = f.parts.len();
        if let (Some(m), Some(feet), Some(her)) = (self.now, feet, self.her) {
            let (hx, hz) = hands(if self.pose().is_some() { Facing::South } else { her.facing });
            let (x, y) = (feet.0 + hx, feet.1 - hz);
            for mo in &self.motes {
                // Each mote with a short tail back along its turn, so the spiral reads.
                let now = i32::from(mo.age) * 256 + i32::from(alpha);
                let (dx, dy) = mo.at(now);
                let (px, py) = mo.at(now - 3 * 256);
                let fade = (u32::from(mo.age) * 40).min(255) as u8;
                let (tx, ty) = ((px - dx) / 16, (py - dy) / 16);
                f.parts.push(Particle {
                    x: (x + dx / 16) as i16,
                    y: (y + dy / 16) as i16,
                    shape: if mo.size > 1 || tx.abs() + ty.abs() < 2 {
                        PartShape::Dot { size: mo.size }
                    } else {
                        PartShape::Streak { dx: tx.clamp(-40, 40) as i8, dy: ty.clamp(-40, 40) as i8 }
                    },
                    colour: mo.colour,
                    alpha: fade,
                    glow: 255,
                    height: hz as u8,
                });
            }
            // The light gathered between her hands: it grows with the gather, and goes with the
            // bloom. Small: it is in her hands, not in front of her face.
            let b = m.beats();
            let most = 1 + u32::from(b.reach) / 20;
            let grown = if m.age < b.bloom {
                1 + (most - 1) * m.age / b.bloom
            } else {
                most.saturating_sub((m.age - b.bloom) / 6)
            };
            if grown > 0 {
                let ramp = m.gift.ramp();
                for (r, tone, a) in [(grown + 2, Tone::Base, 90u8), (grown, Tone::Light, 170)] {
                    f.parts.push(Particle {
                        x: x as i16,
                        y: y as i16,
                        shape: PartShape::Glow { r: r.min(255) as u8 },
                        colour: palette::rgb(ramp.at(tone)),
                        alpha: a,
                        glow: 255,
                        height: hz as u8,
                    });
                }
            }
        }
        let a = i32::from(alpha);
        for s in &self.sparks {
            let (x, y, z) = if s.ground {
                (s.x, s.y, s.z)
            } else {
                (s.x + s.vx * a / 256, s.y + s.vy * a / 256, (s.z + s.vz * a / 256).max(0))
            };
            let life = i32::from(s.life.max(1));
            let shape = match s.shape {
                Shape::Ring(r0, r1) => PartShape::Ring {
                    r: (i32::from(r0) + (i32::from(r1) - i32::from(r0)) * i32::from(s.age) / life).clamp(0, 255) as u8,
                },
                Shape::Dot(n) => PartShape::Dot { size: n },
                Shape::Glow(r) => PartShape::Glow { r },
                Shape::Streak => PartShape::Dot { size: 1 },
            };
            f.parts.push(Particle {
                x: (x / 16 - cam.0) as i16,
                y: ((y - z) / 16 - cam.1) as i16,
                shape,
                colour: s.colour_now(),
                alpha: s.alpha_now(),
                glow: 255,
                height: (z / 16).clamp(0, 255) as u8,
            });
        }
        if f.parts.len() > p0 {
            f.passes.push(Pass::Particles { layer: Depth::Canopy, parts: Span::since(p0, f.parts.len()) });
        }
    }
}

/// How a gift's motes leave her: frost settles like snow, fire and a jar's warmth rise like
/// embers, a charge's jump and fall, the rest float up slowly. `(rise, gravity)` in Q4 px a
/// tick.
const fn drift(g: Gift) -> (i32, i8) {
    match g {
        Gift::Spell { school: School::Frost, .. } => (-2, 0),
        Gift::Spell { school: School::Fire, .. } | Gift::Growth(Stat::Strength) => (8, 0),
        Gift::Spell { school: School::Blast, .. } => (14, -2),
        _ => (5, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_is_longer_and_deeper_than_any_other_and_a_growth_the_least() {
        for (a, b) in [(FIRST, SPELL), (SPELL, GROWTH)] {
            assert!(a.end > b.end && a.hold >= b.hold && a.depth > b.depth && a.duck < b.duck);
            assert!(a.reach > b.reach && a.light > b.light);
        }
        assert_eq!(GROWTH.hold, 0, "a jar holds nothing");
        for b in [FIRST, SPELL, GROWTH] {
            assert!(b.hush_in < b.bloom && b.bloom <= b.lift && b.lift < b.end);
            assert!(b.hold <= b.lift, "the world is let go before the hush lifts");
            assert!(b.fly == 0 || (b.bloom < b.fly && b.fly < b.land && b.land < b.end));
        }
        const { assert!(FIRST.line > FIRST.bloom && FIRST.line < FIRST.fly) };
    }

    #[test]
    fn the_hush_eases_in_holds_and_lifts_to_nothing() {
        let m = |age| Moment { gift: Gift::Growth(Stat::Spirit), age };
        let b = GROWTH;
        assert_eq!(m(0).hush(), 0);
        assert_eq!(m(b.hush_in).hush(), b.depth);
        assert_eq!(m(b.lift).hush(), b.depth);
        assert_eq!(m(b.end).hush(), 0);
        let mut last = 0;
        for a in 0..=b.hush_in {
            let h = m(a).hush();
            assert!(h >= last, "it only deepens coming in");
            last = h;
        }
    }

    #[test]
    fn the_light_swells_blooms_and_fades() {
        let m = |age| Moment { gift: Gift::Spell { spell: SpellId(0), school: School::Frost, first: false }, age };
        let b = SPELL;
        let (k0, r0) = m(0).light();
        let (k1, r1) = m(b.bloom).light();
        assert!(k0 < k1 && r0 < r1, "the bloom is the brightest and the widest");
        assert_eq!(k1, 256);
        assert_eq!(m(b.bloom + b.fade).light().0, 0);
        assert!(m(b.bloom + b.fade / 2).light().0 < k1);
    }

    #[test]
    fn her_hands_go_out_for_a_spell_and_not_for_a_jar() {
        let spell = Moment { gift: Gift::Spell { spell: SpellId(0), school: School::Fire, first: true }, age: 40 };
        assert_eq!(spell.pose(), Some((School::Fire, crate::people::ACT_TICKS)));
        assert_eq!(Moment { age: 0, ..spell }.pose(), Some((School::Fire, 0)));
        assert_eq!(Moment { age: FIRST.end - 1, ..spell }.pose(), None);
        assert_eq!(Moment { gift: Gift::Growth(Stat::Strength), age: 20 }.pose(), None);
    }
}
