//! The effect tables (ART.md §8 step 6; PRESENTATION.md §2): every spell row and every effect
//! row has an entry here, and every entry is a recipe of particles that draws pixels (a test).
//! The recipes are data: how many, what shape, which of the look's three colours, from where,
//! how fast, how long. The pool that runs them is the presenter's (`jane_present::fx`); the
//! stepping of one particle is here, so the sheet and the game move them the same way.
//!
//! Units: canvas px at 16 a cell; speeds and positions in 1/16 px (`Q4`); a particle stands on
//! a ground point `(x, y)` at a height `z`, and is seen at `(x, y - z)`. Ticks at 60 a second.
//! Integer throughout; the dice are the presenter's own [`Lcg`], never the sim's.

use jane_core::Angle;
use jane_core::angle::{cos_q15, sin_q15};

/// A presentation-only generator (PRESENTATION.md §2): a linear congruential step, reseeded per
/// zone from `h32(seed, zone)`, so the same fight looks the same twice and nothing it rolls
/// touches the sim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg(pub u32);

impl Lcg {
    /// The next 32 bits.
    pub fn roll(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // The low bits of an LCG are weak; hand back a mix of the state.
        jane_core::hash::mix32(self.0)
    }

    /// `0..n`.
    pub fn below(&mut self, n: u32) -> u32 {
        crate::hash::below(self.roll(), n.max(1))
    }

    /// `lo..=hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo { lo } else { lo + self.below((hi - lo + 1) as u32) as i32 }
    }
}

/// What a caster's hands do.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cast {
    Frost,
    Fire,
    Venom,
    Spark,
    Blast,
    Throw,
    Swing,
    Repair,
    Grow,
    None,
}

/// The thing in flight.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bolt {
    Frost,
    Fire,
    Venom,
    Needle,
    Spark,
    Charge,
}

/// How a spell lands (a melee blow too).
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Impact {
    Frost,
    Fire,
    Venom,
    Needle,
    Spark,
    Blast,
    Slash,
    Bite,
    ShockGrip,
    Lash,
    None,
}

/// What lies on the ground after a ground spell.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundFx {
    Web,
    Net,
    Dust,
    Charge,
}

/// What a status looks like on the one wearing it.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Frost,
    Burning,
    Poison,
    Stars,
    Web,
    Dust,
    Jolt,
    Shield,
    Drain,
    Glint,
    Stone,
    EmberHands,
    FrostHands,
    SparkHands,
    ThornHands,
    Softened,
    /// Instant (duration 0): one rising flicker as it lands.
    Instant,
}

/// A spell's effect: its cast, its bolt if it flies, its impact, its ground if it leaves one.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellFx {
    pub cast: Cast,
    pub bolt: Option<Bolt>,
    pub impact: Impact,
    pub ground: Option<GroundFx>,
}

const fn sf(cast: Cast, bolt: Option<Bolt>, impact: Impact, ground: Option<GroundFx>) -> SpellFx {
    SpellFx { cast, bolt, impact, ground }
}

/// Every spell row's entry, by content id (`data/spells*.json`). A spell without one is a red
/// test, never a missing effect.
pub const SPELLS: &[(&str, SpellFx)] = &[
    ("melee_player", sf(Cast::Swing, None, Impact::Slash, None)),
    ("melee", sf(Cast::Swing, None, Impact::Slash, None)),
    ("melee_fast", sf(Cast::Swing, None, Impact::Slash, None)),
    ("melee_stun", sf(Cast::Swing, None, Impact::Slash, None)),
    ("hand_bell", sf(Cast::Throw, None, Impact::None, Some(GroundFx::Dust))),
    ("root_lash", sf(Cast::None, None, Impact::Lash, None)),
    ("spider_bite", sf(Cast::None, None, Impact::Bite, None)),
    ("icebolt", sf(Cast::Frost, Some(Bolt::Frost), Impact::Frost, None)),
    ("icebolt_ai", sf(Cast::Frost, Some(Bolt::Frost), Impact::Frost, None)),
    ("fireball", sf(Cast::Fire, Some(Bolt::Fire), Impact::Fire, None)),
    ("repair", sf(Cast::Repair, None, Impact::None, None)),
    ("poisonbolt", sf(Cast::Venom, Some(Bolt::Venom), Impact::Venom, None)),
    ("poisonbolt_far", sf(Cast::Venom, Some(Bolt::Venom), Impact::Venom, None)),
    ("cactus_spray", sf(Cast::None, Some(Bolt::Needle), Impact::Needle, None)),
    ("snake_ring", sf(Cast::Venom, Some(Bolt::Venom), Impact::Venom, None)),
    ("webshot", sf(Cast::Throw, None, Impact::None, Some(GroundFx::Web))),
    ("net_throw", sf(Cast::Throw, None, Impact::None, Some(GroundFx::Net))),
    ("scale_dust", sf(Cast::Throw, None, Impact::None, Some(GroundFx::Dust))),
    ("charge_lob", sf(Cast::Throw, None, Impact::None, Some(GroundFx::Charge))),
    ("explosion", sf(Cast::Blast, Some(Bolt::Charge), Impact::Blast, None)),
    ("grow", sf(Cast::Grow, None, Impact::None, None)),
    ("spark", sf(Cast::Spark, Some(Bolt::Spark), Impact::Spark, None)),
    ("spark_ai", sf(Cast::Spark, Some(Bolt::Spark), Impact::Spark, None)),
    ("melee_shock", sf(Cast::Swing, None, Impact::ShockGrip, None)),
];

/// Every effect row's entry, by content id (`data/effects*.json`).
pub const STATUSES: &[(&str, Status)] = &[
    ("chilled", Status::Frost),
    ("burning", Status::Burning),
    ("poisoned", Status::Poison),
    ("spider_venom", Status::Poison),
    ("stunned", Status::Stars),
    ("webbed", Status::Web),
    ("manashield", Status::Shield),
    ("lifesteal", Status::Drain),
    ("critical", Status::Glint),
    ("stoneskin", Status::Stone),
    ("firelash", Status::EmberHands),
    ("sparktongue", Status::SparkHands),
    ("winterbite", Status::FrostHands),
    ("stranglethorn", Status::ThornHands),
    ("mana_25", Status::Instant),
    ("staggered", Status::Stars),
    ("dazzled", Status::Stars),
    ("jolted", Status::Jolt),
    ("dusted", Status::Dust),
    ("softened", Status::Softened),
];

/// A spell's entry by its content id.
pub fn spell(id: &str) -> Option<SpellFx> {
    SPELLS.iter().find(|(k, _)| *k == id).map(|&(_, f)| f)
}

/// An effect's entry by its content id.
pub fn status(id: &str) -> Option<Status> {
    STATUSES.iter().find(|(k, _)| *k == id).map(|&(_, s)| s)
}

/// A look's three colours: the hot core, the body, the deep edge (and what smoke it leaves).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hue {
    /// The hottest, brightest part.
    pub core: [u8; 3],
    /// The body of it.
    pub mid: [u8; 3],
    /// Its dark edge, its ash, its debris.
    pub deep: [u8; 3],
}

/// The families of colour the effects draw in: muted, a little cold, fire the warmest thing on
/// screen and still not neon (the TS build's palette, carried).
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Frost,
    Fire,
    Venom,
    Spark,
    Blast,
    Needle,
    Pale,
    Blood,
    Web,
    Dust,
    Leaf,
    Repair,
    Shield,
    Stone,
    Gold,
    Drain,
    Smoke,
}

/// A tint's colours.
pub const fn hue(t: Tint) -> Hue {
    let (core, mid, deep) = match t {
        Tint::Frost => ([234, 246, 252], [160, 212, 236], [84, 128, 170]),
        Tint::Fire => ([255, 236, 188], [244, 150, 64], [178, 68, 46]),
        Tint::Venom => ([206, 232, 140], [130, 184, 70], [58, 96, 48]),
        Tint::Spark => ([246, 242, 255], [190, 176, 246], [104, 88, 190]),
        Tint::Blast => ([255, 240, 190], [242, 186, 72], [110, 74, 44]),
        Tint::Needle => ([242, 232, 204], [216, 198, 158], [138, 116, 86]),
        Tint::Pale => ([252, 248, 238], [212, 204, 188], [124, 114, 104]),
        Tint::Blood => ([198, 110, 112], [140, 48, 56], [86, 30, 40]),
        Tint::Web => ([238, 236, 226], [216, 212, 200], [150, 144, 130]),
        Tint::Dust => ([222, 212, 232], [184, 170, 198], [120, 108, 136]),
        Tint::Leaf => ([206, 238, 160], [120, 176, 72], [58, 108, 50]),
        Tint::Repair => ([255, 250, 222], [240, 222, 156], [138, 143, 152]),
        Tint::Shield => ([214, 234, 252], [124, 178, 226], [60, 92, 144]),
        Tint::Stone => ([190, 192, 198], [138, 143, 152], [88, 92, 100]),
        Tint::Gold => ([255, 246, 200], [240, 206, 72], [160, 118, 40]),
        Tint::Drain => ([236, 170, 210], [170, 82, 142], [90, 40, 82]),
        Tint::Smoke => ([112, 104, 108], [74, 68, 72], [48, 44, 50]),
    };
    Hue { core, mid, deep }
}

/// Which of a look's colours a part takes.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Core,
    Mid,
    Deep,
    /// Smoke, whatever the look.
    Smoke,
}

impl Role {
    /// The colour of this role in `h`.
    pub const fn of(self, h: Hue) -> [u8; 3] {
        match self {
            Role::Core => h.core,
            Role::Mid => h.mid,
            Role::Deep => h.deep,
            Role::Smoke => hue(Tint::Smoke).mid,
        }
    }
}

/// A particle's shape (the frame's `PartShape`, before the presenter places it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A square `n` px across.
    Dot(u8),
    /// A stroke along its motion, as long as two ticks of it.
    Streak,
    /// A ring widening from `r0` to `r1` px over its life, squashed on the ground.
    Ring(u8, u8),
    /// A soft disc of radius `r`, shrinking to nothing over its life.
    Glow(u8),
}

/// Where a part starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    /// At the point.
    Point,
    /// On a circle of radius `r` px round it, moving in to it over its life (a gather).
    Gather(u8),
    /// Anywhere in a box `w x h` px round it (on the ground, a scatter).
    Scatter(u8, u8),
}

/// One emission of a recipe: `n` parts alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emit {
    /// How many.
    pub n: u8,
    /// What shape.
    pub shape: Shape,
    /// Their colour, and the colour they turn in the last half of their life.
    pub role: Role,
    /// See `role`.
    pub late: Option<Role>,
    /// Where they start.
    pub from: From,
    /// Speed across the ground, Q4 px a tick, `(lo, hi)`.
    pub speed: (u8, u8),
    /// How far round from the direction they fly, of 256 (256 all round).
    pub spread: u16,
    /// Upward speed at the start, Q4 px a tick (negative falls).
    pub rise: i8,
    /// Added to the upward speed each tick, Q4 (negative is gravity; positive, smoke rising).
    pub grav: i8,
    /// Of 256 of its speed kept each tick.
    pub drag: u8,
    /// Ticks it lives, `(lo, hi)`.
    pub life: (u8, u8),
    /// Of 255 that glows unlit (and blooms on T2).
    pub glow: u8,
    /// Its height above the ground at the start, px.
    pub z: u8,
    /// A mark on the ground: drawn under the standing things, and it does not move.
    pub ground: bool,
}

/// An emission with the defaults: one dot of the body colour at the point, still, 12 ticks.
pub const E: Emit = Emit {
    n: 1,
    shape: Shape::Dot(2),
    role: Role::Mid,
    late: None,
    from: From::Point,
    speed: (0, 0),
    spread: 256,
    rise: 0,
    grav: 0,
    drag: 230,
    life: (12, 12),
    glow: 0,
    z: 16,
    ground: false,
};

/// A light an effect throws while it lasts (PRESENTATION.md §2: a bolt lights the wall it
/// passes): its reach, px, its colour's role, how long, and how high.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FxLight {
    /// Reach, canvas px.
    pub radius: u16,
    /// Its colour.
    pub role: Role,
    /// Ticks it lasts, fading.
    pub ticks: u8,
    /// Px above the ground.
    pub z: u8,
}

/// A recipe: its tint, its emissions, and the light it throws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// The colours it draws in.
    pub tint: Tint,
    /// What it emits, all at once.
    pub emits: &'static [Emit],
    /// The light it throws, if any.
    pub light: Option<FxLight>,
}

const NONE: Recipe = Recipe { tint: Tint::Pale, emits: &[], light: None };

/// Motes drawn in to the hands: the tell that something is coming, a third of a second.
const fn gather(tint: Tint, n: u8, light: u16) -> Recipe {
    Recipe {
        tint,
        emits: &[],
        light: if light > 0 { Some(FxLight { radius: light, role: Role::Mid, ticks: 20, z: 18 }) } else { None },
    }
    .with(n)
}

impl Recipe {
    const fn with(self, n: u8) -> Recipe {
        // A gather: `n` motes of the body colour, a third of them the core.
        let emits: &'static [Emit] = match n {
            0 => &[],
            6 => &[
                Emit { n: 4, from: From::Gather(18), role: Role::Mid, glow: 200, life: (16, 20), shape: Shape::Dot(2), ..E },
                Emit { n: 2, from: From::Gather(14), role: Role::Core, glow: 255, life: (14, 18), shape: Shape::Dot(2), ..E },
            ],
            _ => &[
                Emit { n: 6, from: From::Gather(20), role: Role::Mid, glow: 200, life: (16, 22), shape: Shape::Dot(2), ..E },
                Emit { n: 3, from: From::Gather(14), role: Role::Core, glow: 255, life: (14, 18), shape: Shape::Dot(2), ..E },
                Emit { n: 1, shape: Shape::Glow(6), role: Role::Mid, glow: 255, life: (18, 18), ..E },
            ],
        };
        Recipe { emits, ..self }
    }
}

/// What a cast's hands do.
pub fn cast(c: Cast) -> Recipe {
    match c {
        Cast::Frost => gather(Tint::Frost, 8, 56),
        Cast::Fire => gather(Tint::Fire, 8, 72),
        Cast::Venom => gather(Tint::Venom, 6, 40),
        Cast::Spark => Recipe {
            tint: Tint::Spark,
            emits: &[
                Emit { n: 6, shape: Shape::Streak, role: Role::Core, speed: (24, 40), glow: 255, life: (4, 7), drag: 200, ..E },
                Emit { n: 1, shape: Shape::Glow(7), role: Role::Mid, glow: 255, life: (10, 10), ..E },
            ],
            light: Some(FxLight { radius: 72, role: Role::Mid, ticks: 10, z: 18 }),
        },
        Cast::Blast => Recipe {
            tint: Tint::Blast,
            emits: &[
                Emit { n: 6, from: From::Gather(18), role: Role::Mid, glow: 200, life: (16, 20), ..E },
                Emit { n: 2, shape: Shape::Dot(4), role: Role::Smoke, grav: 1, life: (30, 40), ..E },
            ],
            light: Some(FxLight { radius: 64, role: Role::Mid, ticks: 20, z: 18 }),
        },
        Cast::Throw => Recipe {
            tint: Tint::Pale,
            emits: &[Emit { n: 2, role: Role::Mid, rise: 14, grav: -2, life: (10, 12), ..E }],
            light: None,
        },
        Cast::Repair => Recipe {
            tint: Tint::Repair,
            emits: &[
                Emit { n: 9, role: Role::Core, speed: (10, 22), rise: 10, grav: -2, life: (10, 18), glow: 160, z: 8, ..E },
                Emit { n: 4, role: Role::Deep, speed: (6, 12), rise: 6, grav: -2, life: (14, 20), z: 8, ..E },
                Emit { n: 2, shape: Shape::Streak, role: Role::Core, speed: (30, 40), glow: 255, life: (4, 6), z: 8, ..E },
            ],
            light: Some(FxLight { radius: 36, role: Role::Core, ticks: 8, z: 8 }),
        },
        Cast::Grow => Recipe {
            tint: Tint::Leaf,
            emits: &[
                Emit { n: 12, from: From::Scatter(24, 6), role: Role::Mid, rise: 6, grav: 0, drag: 252, life: (26, 40), z: 0, ..E },
                Emit { n: 4, from: From::Scatter(18, 4), role: Role::Core, rise: 8, life: (30, 40), glow: 120, z: 0, ..E },
                Emit { n: 5, from: From::Scatter(20, 4), role: Role::Deep, life: (60, 70), ground: true, z: 0, ..E },
            ],
            light: None,
        },
        Cast::Swing | Cast::None => NONE,
    }
}

/// A melee swing: a pale arc in front of the swinger, whether or not it connects.
pub fn swing() -> Recipe {
    Recipe {
        tint: Tint::Pale,
        emits: &[Emit { n: 7, shape: Shape::Streak, role: Role::Core, speed: (26, 34), spread: 90, drag: 150, life: (6, 7), glow: 60, ..E }],
        light: None,
    }
}

/// What a bolt sheds every tick of its flight (its head is drawn from the view's projectile).
pub fn trail(b: Bolt) -> Recipe {
    match b {
        Bolt::Frost => Recipe {
            tint: Tint::Frost,
            emits: &[Emit { n: 2, role: Role::Mid, speed: (2, 6), life: (10, 16), glow: 170, grav: -1, shape: Shape::Dot(2), ..E }],
            light: None,
        },
        Bolt::Fire => Recipe {
            tint: Tint::Fire,
            emits: &[
                Emit { n: 2, role: Role::Core, late: Some(Role::Deep), speed: (2, 8), grav: 2, life: (10, 16), glow: 220, ..E },
                Emit { n: 1, role: Role::Smoke, shape: Shape::Dot(3), grav: 2, life: (18, 26), ..E },
            ],
            light: None,
        },
        Bolt::Venom => Recipe {
            tint: Tint::Venom,
            emits: &[Emit { n: 1, role: Role::Mid, late: Some(Role::Deep), speed: (0, 4), grav: -3, life: (12, 18), glow: 90, ..E }],
            light: None,
        },
        Bolt::Needle => Recipe { tint: Tint::Needle, emits: &[Emit { n: 1, role: Role::Deep, life: (5, 7), ..E }], light: None },
        Bolt::Spark => Recipe {
            tint: Tint::Spark,
            emits: &[Emit { n: 2, shape: Shape::Streak, role: Role::Core, speed: (16, 34), life: (3, 5), glow: 255, ..E }],
            light: None,
        },
        Bolt::Charge => Recipe {
            tint: Tint::Blast,
            emits: &[Emit { n: 1, role: Role::Core, late: Some(Role::Smoke), speed: (2, 6), grav: 1, life: (12, 18), glow: 200, ..E }],
            light: None,
        },
    }
}

/// A bolt's head: its colours, its glow's radius px, and the light it carries.
pub fn head(b: Bolt) -> (Tint, u8, Option<FxLight>) {
    let l = |radius, role| Some(FxLight { radius, role, ticks: 1, z: 16 });
    match b {
        Bolt::Frost => (Tint::Frost, 5, l(96, Role::Mid)),
        Bolt::Fire => (Tint::Fire, 6, l(128, Role::Mid)),
        Bolt::Venom => (Tint::Venom, 5, l(64, Role::Mid)),
        Bolt::Needle => (Tint::Needle, 2, None),
        Bolt::Spark => (Tint::Spark, 5, l(110, Role::Core)),
        Bolt::Charge => (Tint::Blast, 4, l(56, Role::Mid)),
    }
}

/// Where a bolt or a blow lands.
pub fn impact(i: Impact) -> Recipe {
    let light = |radius, ticks| Some(FxLight { radius, role: Role::Mid, ticks, z: 12 });
    match i {
        Impact::Frost => Recipe {
            tint: Tint::Frost,
            emits: &[
                Emit { n: 8, role: Role::Core, speed: (18, 26), rise: 8, grav: -2, drag: 220, life: (16, 20), glow: 150, ..E },
                Emit { n: 1, shape: Shape::Ring(3, 20), role: Role::Mid, life: (14, 14), glow: 120, z: 0, ground: true, ..E },
                Emit { n: 6, from: From::Scatter(24, 8), role: Role::Core, life: (50, 70), ground: true, z: 0, ..E },
            ],
            light: light(88, 16),
        },
        Impact::Fire => Recipe {
            tint: Tint::Fire,
            emits: &[
                Emit { n: 10, role: Role::Core, late: Some(Role::Deep), speed: (12, 28), rise: 10, grav: 1, drag: 215, life: (14, 24), glow: 230, ..E },
                Emit { n: 4, shape: Shape::Dot(3), role: Role::Smoke, speed: (4, 8), grav: 2, life: (30, 44), ..E },
                Emit { n: 1, shape: Shape::Glow(10), role: Role::Mid, life: (10, 10), glow: 255, ..E },
                Emit { n: 3, from: From::Scatter(14, 6), role: Role::Deep, life: (70, 90), ground: true, z: 0, ..E },
            ],
            light: light(136, 22),
        },
        Impact::Venom => Recipe {
            tint: Tint::Venom,
            emits: &[
                Emit { n: 8, role: Role::Mid, late: Some(Role::Deep), speed: (10, 20), rise: 12, grav: -3, life: (14, 22), glow: 60, ..E },
                Emit { n: 1, shape: Shape::Ring(2, 14), role: Role::Deep, life: (16, 16), z: 0, ground: true, ..E },
                Emit { n: 5, from: From::Scatter(16, 6), role: Role::Deep, life: (60, 80), ground: true, z: 0, ..E },
            ],
            light: light(48, 12),
        },
        Impact::Needle => Recipe {
            tint: Tint::Needle,
            emits: &[Emit { n: 5, shape: Shape::Streak, role: Role::Core, speed: (16, 26), rise: 4, grav: -2, life: (6, 10), ..E }],
            light: None,
        },
        Impact::Spark => Recipe {
            tint: Tint::Spark,
            emits: &[
                Emit { n: 10, shape: Shape::Streak, role: Role::Core, speed: (24, 44), rise: 6, grav: -2, drag: 210, life: (5, 10), glow: 255, ..E },
                Emit { n: 1, shape: Shape::Glow(9), role: Role::Mid, life: (8, 8), glow: 255, ..E },
                Emit { n: 1, shape: Shape::Ring(2, 16), role: Role::Core, life: (8, 8), glow: 200, z: 0, ground: true, ..E },
            ],
            light: light(120, 10),
        },
        Impact::Blast => Recipe {
            tint: Tint::Blast,
            emits: &[
                Emit { n: 1, shape: Shape::Ring(6, 56), role: Role::Core, life: (14, 14), glow: 220, z: 0, ground: true, ..E },
                Emit { n: 1, shape: Shape::Glow(22), role: Role::Mid, life: (12, 12), glow: 255, ..E },
                Emit { n: 16, role: Role::Core, late: Some(Role::Deep), speed: (20, 44), rise: 16, grav: -2, drag: 225, life: (16, 28), glow: 200, ..E },
                Emit { n: 8, role: Role::Deep, shape: Shape::Dot(2), speed: (18, 36), rise: 22, grav: -3, life: (20, 30), ..E },
                Emit { n: 6, shape: Shape::Dot(4), role: Role::Smoke, speed: (6, 14), grav: 2, life: (40, 60), ..E },
                Emit { n: 8, from: From::Scatter(40, 20), role: Role::Smoke, life: (90, 120), ground: true, z: 0, shape: Shape::Dot(3), ..E },
            ],
            light: light(180, 26),
        },
        Impact::Slash => Recipe {
            tint: Tint::Pale,
            emits: &[
                Emit { n: 4, shape: Shape::Streak, role: Role::Core, speed: (20, 30), spread: 64, life: (4, 6), glow: 80, ..E },
                Emit { n: 3, role: Role::Deep, speed: (8, 16), rise: 8, grav: -3, life: (10, 14), ..E },
            ],
            light: None,
        },
        Impact::Bite => Recipe {
            tint: Tint::Blood,
            emits: &[
                Emit { n: 5, role: Role::Mid, speed: (8, 18), rise: 10, grav: -3, life: (10, 16), ..E },
                Emit { n: 2, from: From::Scatter(8, 4), role: Role::Deep, life: (60, 80), ground: true, z: 0, ..E },
            ],
            light: None,
        },
        Impact::ShockGrip => Recipe {
            tint: Tint::Spark,
            emits: &[
                Emit { n: 8, shape: Shape::Streak, role: Role::Core, speed: (20, 36), life: (4, 8), glow: 255, ..E },
                Emit { n: 1, shape: Shape::Glow(8), role: Role::Mid, life: (8, 8), glow: 255, ..E },
            ],
            light: light(96, 8),
        },
        Impact::Lash => Recipe {
            tint: Tint::Leaf,
            emits: &[
                Emit { n: 5, shape: Shape::Streak, role: Role::Mid, speed: (18, 28), spread: 80, life: (6, 8), ..E },
                Emit { n: 4, role: Role::Deep, speed: (6, 14), rise: 8, grav: -3, life: (10, 16), ..E },
            ],
            light: None,
        },
        Impact::None => NONE,
    }
}

/// What a ground spell leaves where it lands.
pub fn ground(g: GroundFx) -> Recipe {
    match g {
        GroundFx::Web => Recipe {
            tint: Tint::Web,
            emits: &[
                Emit { n: 1, shape: Shape::Ring(4, 22), role: Role::Mid, life: (12, 12), z: 0, ground: true, ..E },
                Emit { n: 14, from: From::Scatter(36, 18), role: Role::Core, life: (90, 120), z: 0, ground: true, shape: Shape::Dot(1), ..E },
            ],
            light: None,
        },
        GroundFx::Net => Recipe {
            tint: Tint::Needle,
            emits: &[Emit { n: 16, from: From::Scatter(32, 16), role: Role::Deep, life: (90, 120), z: 0, ground: true, shape: Shape::Dot(2), ..E }],
            light: None,
        },
        GroundFx::Dust => Recipe {
            tint: Tint::Dust,
            emits: &[
                Emit { n: 14, from: From::Scatter(36, 18), role: Role::Mid, rise: 4, grav: 0, drag: 250, life: (40, 70), shape: Shape::Dot(3), z: 4, ..E },
                Emit { n: 6, from: From::Scatter(28, 14), role: Role::Core, rise: 3, life: (30, 50), z: 8, ..E },
            ],
            light: None,
        },
        GroundFx::Charge => Recipe {
            tint: Tint::Blast,
            emits: &[
                Emit { n: 1, shape: Shape::Glow(5), role: Role::Core, life: (60, 60), glow: 255, z: 2, ..E },
                Emit { n: 6, role: Role::Core, speed: (6, 14), rise: 10, grav: -2, life: (8, 14), glow: 220, ..E },
            ],
            light: Some(FxLight { radius: 48, role: Role::Mid, ticks: 60, z: 4 }),
        },
    }
}

/// What a status sheds on its wearer, every `every` ticks while it is on.
pub fn wear(s: Status) -> (Recipe, u8) {
    let r = |tint, emits, light| Recipe { tint, emits, light };
    match s {
        Status::Frost => (r(Tint::Frost, &[Emit { n: 1, from: From::Scatter(18, 10), role: Role::Core, rise: -2, life: (18, 26), glow: 120, z: 24, ..E }], None), 8),
        Status::Burning => (
            r(
                Tint::Fire,
                &[
                    Emit { n: 2, from: From::Scatter(14, 6), role: Role::Core, late: Some(Role::Deep), rise: 10, grav: 1, life: (14, 22), glow: 230, z: 10, ..E },
                    Emit { n: 1, role: Role::Smoke, shape: Shape::Dot(3), rise: 8, grav: 1, life: (26, 34), z: 30, ..E },
                ],
                Some(FxLight { radius: 64, role: Role::Mid, ticks: 6, z: 20 }),
            ),
            5,
        ),
        Status::Poison => (r(Tint::Venom, &[Emit { n: 1, from: From::Scatter(16, 6), role: Role::Mid, rise: 6, life: (20, 28), glow: 60, z: 20, ..E }], None), 10),
        Status::Stars => (r(Tint::Gold, &[Emit { n: 1, from: From::Gather(12), role: Role::Core, life: (20, 20), glow: 200, z: 44, ..E }], None), 7),
        Status::Web => (r(Tint::Web, &[Emit { n: 2, from: From::Scatter(16, 12), role: Role::Core, life: (10, 14), z: 14, shape: Shape::Dot(1), ..E }], None), 6),
        Status::Dust => (r(Tint::Dust, &[Emit { n: 1, from: From::Scatter(18, 8), role: Role::Mid, rise: 2, life: (24, 30), z: 30, ..E }], None), 8),
        Status::Jolt => (r(Tint::Spark, &[Emit { n: 2, shape: Shape::Streak, role: Role::Core, speed: (16, 30), life: (3, 5), glow: 255, z: 24, ..E }], Some(FxLight { radius: 40, role: Role::Mid, ticks: 4, z: 20 })), 9),
        Status::Shield => (r(Tint::Shield, &[Emit { n: 1, shape: Shape::Ring(14, 16), role: Role::Mid, life: (14, 14), glow: 140, z: 0, ground: true, ..E }], None), 14),
        Status::Drain => (r(Tint::Drain, &[Emit { n: 1, from: From::Gather(16), role: Role::Mid, life: (18, 18), glow: 150, z: 20, ..E }], None), 9),
        Status::Glint => (r(Tint::Gold, &[Emit { n: 1, from: From::Scatter(18, 20), role: Role::Core, life: (8, 10), glow: 255, z: 20, shape: Shape::Dot(1), ..E }], None), 12),
        Status::Stone => (r(Tint::Stone, &[Emit { n: 1, from: From::Scatter(18, 6), role: Role::Deep, rise: -4, grav: -2, life: (10, 14), z: 30, ..E }], None), 12),
        Status::EmberHands => (r(Tint::Fire, &[Emit { n: 1, role: Role::Core, late: Some(Role::Deep), rise: 8, grav: 1, life: (12, 18), glow: 230, z: 18, ..E }], Some(FxLight { radius: 44, role: Role::Mid, ticks: 6, z: 18 })), 6),
        Status::FrostHands => (r(Tint::Frost, &[Emit { n: 1, role: Role::Core, rise: -2, life: (14, 18), glow: 160, z: 18, ..E }], None), 7),
        Status::SparkHands => (r(Tint::Spark, &[Emit { n: 1, shape: Shape::Streak, role: Role::Core, speed: (14, 24), life: (3, 5), glow: 255, z: 18, ..E }], Some(FxLight { radius: 36, role: Role::Core, ticks: 4, z: 18 })), 8),
        Status::ThornHands => (r(Tint::Leaf, &[Emit { n: 1, role: Role::Deep, rise: 4, life: (14, 18), z: 18, ..E }], None), 9),
        Status::Softened => (r(Tint::Dust, &[Emit { n: 1, from: From::Scatter(16, 8), role: Role::Deep, rise: -3, grav: -1, life: (14, 20), z: 30, ..E }], None), 10),
        Status::Instant => (r(Tint::Shield, &[Emit { n: 4, from: From::Scatter(16, 6), role: Role::Core, rise: 12, life: (16, 22), glow: 200, z: 8, ..E }], None), 255),
    }
}

/// A death: the body's breath going out of it, a little dust where it falls.
pub fn death() -> Recipe {
    Recipe {
        tint: Tint::Dust,
        emits: &[
            Emit { n: 6, from: From::Scatter(20, 8), role: Role::Mid, rise: 8, grav: 0, drag: 250, life: (26, 40), shape: Shape::Dot(3), z: 4, ..E },
            Emit { n: 4, from: From::Scatter(24, 10), role: Role::Deep, life: (40, 60), z: 0, ground: true, ..E },
        ],
        light: None,
    }
}

/// One particle in flight: ground point and height in Q4 px, velocity in Q4 px a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spark {
    /// Q4 px, zone canvas coordinates.
    pub x: i32,
    /// Q4 px.
    pub y: i32,
    /// Q4 px above the ground.
    pub z: i32,
    /// Q4 px a tick.
    pub vx: i32,
    /// Q4 px a tick.
    pub vy: i32,
    /// Q4 px a tick, up.
    pub vz: i32,
    /// Ticks lived.
    pub age: u8,
    /// Ticks it lives.
    pub life: u8,
    /// Its shape.
    pub shape: Shape,
    /// Its colour, and the colour it turns in the last half of its life.
    pub colour: [u8; 3],
    /// See `colour`.
    pub late: [u8; 3],
    /// Of 255 that glows.
    pub glow: u8,
    /// Q4 added to `vz` a tick.
    pub grav: i8,
    /// Of 256 of its speed kept a tick.
    pub drag: u8,
    /// A mark on the ground.
    pub ground: bool,
}

impl Spark {
    /// One tick: moves, falls, slows, ages. `false` when it is done.
    pub fn step(&mut self) -> bool {
        self.age = self.age.saturating_add(1);
        if self.age >= self.life {
            return false;
        }
        if self.ground {
            return true;
        }
        self.x += self.vx;
        self.y += self.vy;
        self.z += self.vz;
        self.vz += i32::from(self.grav);
        let d = i32::from(self.drag);
        self.vx = self.vx * d / 256;
        self.vy = self.vy * d / 256;
        if self.z < 0 {
            // It lands and stops.
            self.z = 0;
            self.vz = 0;
            self.vx /= 2;
            self.vy /= 2;
        }
        true
    }

    /// Its colour now: `colour` for the first half of its life, turning to `late` over the rest.
    pub fn colour_now(&self) -> [u8; 3] {
        let (a, l) = (u32::from(self.age), u32::from(self.life.max(1)));
        if 2 * a <= l {
            return self.colour;
        }
        let t = (2 * a - l) * 256 / l;
        [0, 1, 2].map(|k| ((u32::from(self.colour[k]) * (256 - t) + u32::from(self.late[k]) * t) >> 8) as u8)
    }

    /// Its opacity now, of 255: full, fading over its last third.
    pub fn alpha_now(&self) -> u8 {
        let (a, l) = (u32::from(self.age), u32::from(self.life.max(1)));
        let left = l.saturating_sub(a);
        if left * 3 >= l { 255 } else { (left * 3 * 255 / l).min(255) as u8 }
    }
}

/// Emits `r` at ground point `(x, y)` (canvas px) toward `dir`, every part through `out`.
pub fn emit(r: &Recipe, (x, y): (i32, i32), dir: Angle, rng: &mut Lcg, out: &mut impl FnMut(Spark)) {
    let h = hue(r.tint);
    for e in r.emits {
        for _ in 0..e.n {
            let spread = i32::from(e.spread.min(256));
            let turn = if spread >= 256 { rng.below(65536) as i32 } else { rng.range(-spread * 128, spread * 128) };
            let a = Angle((i32::from(dir.0) + turn) as u16);
            let s = rng.range(i32::from(e.speed.0), i32::from(e.speed.1));
            let (c, sn) = (cos_q15(a).0, sin_q15(a).0);
            // Across the ground the 3/4 view foreshortens y.
            let (mut vx, mut vy) = ((s * c) >> 15, (s * sn * 3 / 4) >> 15);
            let life = rng.range(i32::from(e.life.0), i32::from(e.life.1)).clamp(1, 255) as u8;
            let (mut px, mut py) = (x * 16, y * 16);
            match e.from {
                From::Point => {}
                From::Gather(rad) => {
                    let g = Angle(rng.below(65536) as u16);
                    let rr = i32::from(rad) * 16;
                    let (ox, oy) = ((rr * cos_q15(g).0) >> 15, (rr * sin_q15(g).0 * 3 / 4) >> 15);
                    px += ox;
                    py += oy;
                    vx = -ox / i32::from(life);
                    vy = -oy / i32::from(life);
                }
                From::Scatter(w, hh) => {
                    px += rng.range(-i32::from(w) * 8, i32::from(w) * 8);
                    py += rng.range(-i32::from(hh) * 8, i32::from(hh) * 8);
                }
            }
            let colour = e.role.of(h);
            out(Spark {
                x: px,
                y: py,
                z: i32::from(e.z) * 16,
                vx,
                vy,
                vz: i32::from(e.rise),
                age: 0,
                life,
                shape: e.shape,
                colour,
                late: e.late.map_or(colour, |l| l.of(h)),
                glow: e.glow,
                grav: e.grav,
                drag: if matches!(e.from, From::Gather(_)) { 255 } else { e.drag },
                ground: e.ground,
            });
        }
    }
}

/// Draws `sparks` as they would stand into a `w x h` buffer of `0xRRGGBB` (0 is nothing), their
/// ground points offset by `(ox, oy)`: the sheet's rasteriser, and the test that an entry draws.
pub fn rasterise(sparks: &[Spark], px: &mut [u32], w: i32, h: i32, (ox, oy): (i32, i32)) -> usize {
    let mut n = 0;
    let mut put = |x: i32, y: i32, c: [u8; 3]| {
        if x >= 0 && y >= 0 && x < w && y < h {
            px[(y * w + x) as usize] = u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]) | 0x0100_0000;
            n += 1;
        }
    };
    for s in sparks {
        let (x, y) = (s.x / 16 + ox, (s.y - s.z) / 16 + oy);
        let c = s.colour_now();
        match s.shape {
            Shape::Dot(k) => {
                for dy in 0..i32::from(k.max(1)) {
                    for dx in 0..i32::from(k.max(1)) {
                        put(x + dx, y + dy, c);
                    }
                }
            }
            Shape::Streak => crate::canvas::bresenham(x, y, x - s.vx * 2 / 16, y - (s.vy - s.vz) * 2 / 16, |a, b| put(a, b, c)),
            Shape::Ring(r0, r1) => {
                let r = i32::from(r0) + (i32::from(r1) - i32::from(r0)) * i32::from(s.age) / i32::from(s.life.max(1));
                for k in 0..32 {
                    let a = Angle((k * 2048) as u16);
                    put(x + ((r * cos_q15(a).0) >> 15), y + ((r * sin_q15(a).0 / 2) >> 15), c);
                }
            }
            Shape::Glow(r) => {
                let r = i32::from(r) * (i32::from(s.life) - i32::from(s.age)) / i32::from(s.life.max(1));
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r {
                            put(x + dx, y + dy, c);
                        }
                    }
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `r` for `ticks` and counts the px it drew over them.
    fn draws(r: &Recipe, ticks: u32) -> usize {
        let mut rng = Lcg(7);
        let mut pool = Vec::new();
        emit(r, (0, 0), Angle::EAST, &mut rng, &mut |s| pool.push(s));
        let mut px = vec![0u32; 160 * 160];
        let mut n = 0;
        for _ in 0..ticks {
            n += rasterise(&pool, &mut px, 160, 160, (80, 100));
            pool.retain_mut(Spark::step);
        }
        n
    }

    #[test]
    fn every_spell_row_and_every_effect_row_has_an_entry_that_draws_pixels() {
        let cat = jane_data::catalog();
        for s in cat.combat.spells {
            let f = spell(s.id).unwrap_or_else(|| panic!("spell {} has no fx entry", s.id));
            let mut n = draws(&cast(f.cast), 30) + draws(&impact(f.impact), 30);
            if let Some(b) = f.bolt {
                n += draws(&trail(b), 20);
                assert!(head(b).1 > 0, "{}", s.id);
            }
            if let Some(g) = f.ground {
                n += draws(&ground(g), 30);
            }
            if matches!(f.cast, Cast::Swing) {
                n += draws(&swing(), 10);
            }
            assert!(n > 0, "spell {} draws nothing", s.id);
        }
        for e in cat.combat.effects {
            let st = status(e.id).unwrap_or_else(|| panic!("effect {} has no fx entry", e.id));
            let (r, every) = wear(st);
            assert!(every > 0);
            assert!(draws(&r, 30) > 0, "effect {} draws nothing", e.id);
        }
        // And no entry names a row that is not there.
        for (id, _) in SPELLS {
            assert!(cat.combat.spells.iter().any(|s| s.id == *id), "fx entry {id} names no spell");
        }
        for (id, _) in STATUSES {
            assert!(cat.combat.effects.iter().any(|s| s.id == *id), "fx entry {id} names no effect");
        }
        assert!(draws(&death(), 30) > 0);
    }

    #[test]
    fn a_part_fades_and_ends() {
        let mut rng = Lcg(1);
        let mut pool = Vec::new();
        emit(&impact(Impact::Fire), (0, 0), Angle::EAST, &mut rng, &mut |s| pool.push(s));
        assert!(!pool.is_empty());
        for _ in 0..200 {
            pool.retain_mut(Spark::step);
        }
        assert!(pool.is_empty(), "every part ends");
        let mut s = pool.first().copied().unwrap_or(Spark {
            x: 0,
            y: 0,
            z: 0,
            vx: 0,
            vy: 0,
            vz: 0,
            age: 0,
            life: 30,
            shape: Shape::Dot(1),
            colour: [255, 0, 0],
            late: [0, 0, 255],
            glow: 0,
            grav: 0,
            drag: 255,
            ground: false,
        });
        assert_eq!(s.alpha_now(), 255);
        s.age = 29;
        assert!(s.alpha_now() < 40);
        assert!(s.colour_now()[2] > s.colour_now()[0]);
    }

    #[test]
    fn the_dice_are_the_same_twice() {
        let (mut a, mut b) = (Lcg(99), Lcg(99));
        assert!((0..100).all(|_| a.roll() == b.roll()));
        assert!((0..1000).all(|_| a.range(-3, 3).abs() <= 3));
    }
}
