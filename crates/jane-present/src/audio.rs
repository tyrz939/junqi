//! The audio hooks and the cue table (PRESENTATION.md §5): what the game sounds like, decided
//! here from the `View` and the tick's `Event`s, and handed to an [`AudioBus`] that makes it.
//! Nothing here makes a sample: `jane-audio` does, behind the app's bus. The tests and
//! `jane serve` use [`NullBus`].
//!
//! - **Music**: the title on the title screen; the zone's cue on entry (each region of the county
//!   by day and by night, each dungeon its own); `Combat` once a hostile has been at her for 2 s,
//!   back to the zone 6 s after the last; `Dead` when she falls; `Bell` while the School's bell
//!   strikes the hour, then the hour's cue; `Silence` at the end, and for the nine that has no bell.
//! - **Effects**: every sound event, placed over 24 cells from the listener and panned
//!   ([`place`]); her footsteps by the ground under her; every bell the sim rings
//!   (`EventKind::Bell`: the School's at nine and six, the early one, the Timekeeper's rope, the
//!   church at evensong), a strike at a time; the Sunday train.
//! - **Beds**: rain and wind from the view's weather, birds at dusk, crickets at night, the lake
//!   in the Waters, the hum in the Works, drips underground, a fire in the Arms, a clock indoors.
//!
//! Every timer counts ticks (§1.11); the one random draw (when an owl calls) is seeded by the
//! county, so a replay sounds the same.

// Fx positions become distances in cells and angles become offsets: a county is far inside the
// range an f32 holds exactly.
#![allow(clippy::cast_precision_loss)]

use jane_core::action::School;
use jane_core::num::{CELL_FX, Fx};
use jane_core::{Tile, Vec2, ZoneId};
use jane_data::{Faction, Region};
use jane_sim::event::{Event, EventKind, PropChange, QuestChange, SfxKind as SimSfx};
use jane_sim::ids::{PropId, Seat, UnitId};
use jane_sim::state::{CombatState, WeatherKind};
use jane_sim::tuning::{TICKS_PER_DAY, TICKS_PER_HOUR};
use jane_sim::view::View;

/// A place in the world, as the bus hears it.
pub type At = (Fx, Fx);

/// What the music is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicCue {
    Title,
    /// A zone's cue: the county's by the region under her and the hour, a dungeon's its own
    /// (its region and night are normalised away, so a dungeon's music does not change at nine).
    Zone(ZoneId, Region, bool),
    /// The hush under the bell's strikes.
    Bell,
    Combat,
    Dead,
    Silence,
}

impl MusicCue {
    /// A zone's cue, normalised: only the county's depends on the region and the hour.
    pub fn zone(zone: ZoneId, region: Region, night: bool) -> MusicCue {
        if zone == ZoneId::County {
            MusicCue::Zone(zone, region, night)
        } else {
            MusicCue::Zone(zone, Region::Lowfields, false)
        }
    }

    /// The song it plays (`data/audio/songs`), `None` for silence.
    pub fn song(self) -> Option<&'static str> {
        Some(match self {
            MusicCue::Title => "title",
            MusicCue::Bell => "bell",
            MusicCue::Combat => "combat",
            MusicCue::Dead => "dead",
            MusicCue::Silence => return None,
            MusicCue::Zone(zone, region, night) => match zone {
                ZoneId::County => match (region, night) {
                    (Region::Lowfields, false) => "lowfields_day",
                    (Region::Lowfields, true) => "lowfields_night",
                    (Region::Waters, false) => "waters_day",
                    (Region::Waters, true) => "waters_night",
                    (Region::Works, false) => "works_day",
                    (Region::Works, true) => "works_night",
                },
                ZoneId::House | ZoneId::Cellar => "home",
                ZoneId::Mine | ZoneId::Pipes => "deep",
                ZoneId::Burial => "burial",
                ZoneId::Arms => "arms",
                ZoneId::Church => "church",
                ZoneId::Factory => "factory",
                ZoneId::Forest => "forest",
                ZoneId::Library | ZoneId::Museum => "halls",
                ZoneId::School => "school",
            },
        })
    }

    /// Every cue a game can ask for (the app checks each has a song).
    pub fn all() -> Vec<MusicCue> {
        let mut v = vec![MusicCue::Title, MusicCue::Bell, MusicCue::Combat, MusicCue::Dead, MusicCue::Silence];
        for z in ZoneId::ALL {
            for r in [Region::Lowfields, Region::Waters, Region::Works] {
                for night in [false, true] {
                    v.push(MusicCue::zone(z, r, night));
                }
            }
        }
        v.dedup();
        v
    }
}

/// How the music moves from one cue to the next, in milliseconds: (the old one fading out, the
/// new one fading in). A fight comes in fast and leaves slowly; a fall cuts; the bell hushes.
pub fn fades(from: Option<MusicCue>, to: MusicCue) -> (u16, u16) {
    match (from, to) {
        (_, MusicCue::Combat) => (700, 300),
        (_, MusicCue::Dead) => (300, 0),
        (_, MusicCue::Bell) => (2500, 1500),
        (Some(MusicCue::Combat), _) => (2500, 2500),
        (Some(MusicCue::Bell), _) => (3000, 2500),
        (_, MusicCue::Silence) => (2500, 0),
        (None | Some(MusicCue::Title), _) => (2000, 1200),
        _ => (2500, 2000),
    }
}

/// A sound the presentation asks for. Each is a row of `data/audio/sfx.json` by [`SfxKind::name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxKind {
    StepGrass,
    StepRoad,
    StepCobble,
    StepWood,
    StepWater,
    Swing,
    /// A foe's wind-up begins (`feel.rs`): a breath drawn up, the tell's sound. Steady (a boss
    /// tell nothing breaks) is lower and longer.
    Windup,
    WindupSteady,
    /// Her hop.
    Hop,
    Strike,
    HitPhysical,
    HitFrost,
    HitFire,
    HitNature,
    HitBlast,
    HitShock,
    Cast,
    CastFailed,
    Hurt,
    Crit,
    Heal,
    Death,
    Loot,
    QuestGiven,
    QuestDone,
    /// A spell learned (`crate::lesson`, §5.1): a breath drawn in, then the theme's first notes
    /// (up a fifth, lean on the sixth) in the school's key and voice over its chord.
    LearnHeal,
    LearnPhysical,
    LearnFrost,
    LearnFire,
    LearnNature,
    LearnBlast,
    LearnShock,
    /// The first spell she ever learns: a longer breath, then the theme's whole first phrase in
    /// the school's key and voice, stopping on the second as the theme does.
    FirstHeal,
    FirstPhysical,
    FirstFrost,
    FirstFire,
    FirstNature,
    FirstBlast,
    FirstShock,
    /// A jar (strength) or a page (spirit): two notes, up a fifth.
    GrowStrength,
    GrowSpirit,
    Journal,
    Open,
    Use,
    Unlock,
    Locked,
    Switch,
    Push,
    PlateDown,
    PlateUp,
    Door,
    Rest,
    Respawn,
    Status,
    Save,
    Thunder,
    Owl,
    Crow,
    /// A hive's bees, heard before it is seen (the last fifty metres, [`FarCall`]).
    Bees,
    /// The diver's knocking from under the water.
    Knocking,
    /// The night shift's boots on Cinder Walk.
    Boots,
    /// The School's bell, across the county.
    BellFar,
    /// The School's bell heard through walls and earth.
    BellWithin,
    /// The School's bell where it hangs, or where its ringer pulls it, within hearing.
    BellNear,
    /// The church's: a smaller bell, not *the* bell.
    ChurchBell,
    TrainWhistle,
    /// Julie's dog (`STORY.md` §3): a bark, a whine as it goes for the night, panting beside her.
    DogBark,
    DogWhine,
    DogPant,
    UiMove,
    UiConfirm,
    UiBack,
    UiOpen,
    UiClose,
    /// Her cast building (PLAY-PLAN §2.1): four short swells, each a step higher, at a quarter,
    /// a half and three quarters of the way, so the tone rises to the release and stops when it
    /// is cut short.
    CastRise1,
    CastRise2,
    CastRise3,
    CastRise4,
    /// A cast that built lands: a thump and a crack.
    CastRelease,
}

impl SfxKind {
    /// The sound a lesson's moment makes (`crate::lesson::Gift`): the school's cue, its first-spell
    /// phrase, or a jar's or a page's two notes.
    pub const fn of_gift(g: crate::lesson::Gift) -> SfxKind {
        use crate::lesson::Gift;
        use jane_core::action::Stat;
        match g {
            Gift::Spell { school, first: false, .. } => match school {
                School::Heal => SfxKind::LearnHeal,
                School::Physical => SfxKind::LearnPhysical,
                School::Frost => SfxKind::LearnFrost,
                School::Fire => SfxKind::LearnFire,
                School::Nature => SfxKind::LearnNature,
                School::Blast => SfxKind::LearnBlast,
                School::Shock => SfxKind::LearnShock,
            },
            Gift::Spell { school, first: true, .. } => match school {
                School::Heal => SfxKind::FirstHeal,
                School::Physical => SfxKind::FirstPhysical,
                School::Frost => SfxKind::FirstFrost,
                School::Fire => SfxKind::FirstFire,
                School::Nature => SfxKind::FirstNature,
                School::Blast => SfxKind::FirstBlast,
                School::Shock => SfxKind::FirstShock,
            },
            Gift::Growth(Stat::Strength) => SfxKind::GrowStrength,
            Gift::Growth(Stat::Spirit) => SfxKind::GrowSpirit,
        }
    }

    pub const ALL: [SfxKind; 79] = [
        SfxKind::StepGrass,
        SfxKind::StepRoad,
        SfxKind::StepCobble,
        SfxKind::StepWood,
        SfxKind::StepWater,
        SfxKind::Swing,
        SfxKind::Windup,
        SfxKind::WindupSteady,
        SfxKind::Hop,
        SfxKind::Strike,
        SfxKind::HitPhysical,
        SfxKind::HitFrost,
        SfxKind::HitFire,
        SfxKind::HitNature,
        SfxKind::HitBlast,
        SfxKind::HitShock,
        SfxKind::Cast,
        SfxKind::CastFailed,
        SfxKind::Hurt,
        SfxKind::Crit,
        SfxKind::Heal,
        SfxKind::Death,
        SfxKind::Loot,
        SfxKind::QuestGiven,
        SfxKind::QuestDone,
        SfxKind::LearnHeal,
        SfxKind::LearnPhysical,
        SfxKind::LearnFrost,
        SfxKind::LearnFire,
        SfxKind::LearnNature,
        SfxKind::LearnBlast,
        SfxKind::LearnShock,
        SfxKind::FirstHeal,
        SfxKind::FirstPhysical,
        SfxKind::FirstFrost,
        SfxKind::FirstFire,
        SfxKind::FirstNature,
        SfxKind::FirstBlast,
        SfxKind::FirstShock,
        SfxKind::GrowStrength,
        SfxKind::GrowSpirit,
        SfxKind::Journal,
        SfxKind::Open,
        SfxKind::Use,
        SfxKind::Unlock,
        SfxKind::Locked,
        SfxKind::Switch,
        SfxKind::Push,
        SfxKind::PlateDown,
        SfxKind::PlateUp,
        SfxKind::Door,
        SfxKind::Rest,
        SfxKind::Respawn,
        SfxKind::Status,
        SfxKind::Save,
        SfxKind::Thunder,
        SfxKind::Owl,
        SfxKind::Crow,
        SfxKind::Bees,
        SfxKind::Knocking,
        SfxKind::Boots,
        SfxKind::BellFar,
        SfxKind::BellWithin,
        SfxKind::BellNear,
        SfxKind::ChurchBell,
        SfxKind::TrainWhistle,
        SfxKind::DogBark,
        SfxKind::DogWhine,
        SfxKind::DogPant,
        SfxKind::UiMove,
        SfxKind::UiConfirm,
        SfxKind::UiBack,
        SfxKind::UiOpen,
        SfxKind::UiClose,
        SfxKind::CastRise1,
        SfxKind::CastRise2,
        SfxKind::CastRise3,
        SfxKind::CastRise4,
        SfxKind::CastRelease,
    ];

    /// Its row in `data/audio/sfx.json`.
    pub const fn name(self) -> &'static str {
        match self {
            SfxKind::StepGrass => "step_grass",
            SfxKind::StepRoad => "step_road",
            SfxKind::StepCobble => "step_cobble",
            SfxKind::StepWood => "step_wood",
            SfxKind::StepWater => "step_water",
            SfxKind::Swing => "swing",
            SfxKind::Windup => "windup",
            SfxKind::WindupSteady => "windup_steady",
            SfxKind::Hop => "hop",
            SfxKind::Strike => "strike",
            SfxKind::HitPhysical => "hit_physical",
            SfxKind::HitFrost => "hit_frost",
            SfxKind::HitFire => "hit_fire",
            SfxKind::HitNature => "hit_nature",
            SfxKind::HitBlast => "hit_blast",
            SfxKind::HitShock => "hit_shock",
            SfxKind::Cast => "cast",
            SfxKind::CastFailed => "cast_failed",
            SfxKind::Hurt => "hurt",
            SfxKind::Crit => "crit",
            SfxKind::Heal => "heal",
            SfxKind::Death => "death",
            SfxKind::Loot => "loot",
            SfxKind::QuestGiven => "quest_given",
            SfxKind::QuestDone => "quest_done",
            SfxKind::LearnHeal => "learn_heal",
            SfxKind::LearnPhysical => "learn_physical",
            SfxKind::LearnFrost => "learn_frost",
            SfxKind::LearnFire => "learn_fire",
            SfxKind::LearnNature => "learn_nature",
            SfxKind::LearnBlast => "learn_blast",
            SfxKind::LearnShock => "learn_shock",
            SfxKind::FirstHeal => "learn_first_heal",
            SfxKind::FirstPhysical => "learn_first_physical",
            SfxKind::FirstFrost => "learn_first_frost",
            SfxKind::FirstFire => "learn_first_fire",
            SfxKind::FirstNature => "learn_first_nature",
            SfxKind::FirstBlast => "learn_first_blast",
            SfxKind::FirstShock => "learn_first_shock",
            SfxKind::GrowStrength => "grow_strength",
            SfxKind::GrowSpirit => "grow_spirit",
            SfxKind::Journal => "journal",
            SfxKind::Open => "open",
            SfxKind::Use => "use",
            SfxKind::Unlock => "unlock",
            SfxKind::Locked => "locked",
            SfxKind::Switch => "switch",
            SfxKind::Push => "push",
            SfxKind::PlateDown => "plate_down",
            SfxKind::PlateUp => "plate_up",
            SfxKind::Door => "door",
            SfxKind::Rest => "rest",
            SfxKind::Respawn => "respawn",
            SfxKind::Status => "status",
            SfxKind::Save => "save",
            SfxKind::Thunder => "thunder",
            SfxKind::Owl => "owl",
            SfxKind::Crow => "crow",
            SfxKind::Bees => "bees",
            SfxKind::Knocking => "knocking",
            SfxKind::Boots => "boots",
            SfxKind::BellFar => "bell_far",
            SfxKind::BellWithin => "bell_within",
            SfxKind::BellNear => "bell_near",
            SfxKind::ChurchBell => "church_bell",
            SfxKind::TrainWhistle => "train_whistle",
            SfxKind::DogBark => "dog_bark",
            SfxKind::DogWhine => "dog_whine",
            SfxKind::DogPant => "dog_pant",
            SfxKind::UiMove => "ui_move",
            SfxKind::UiConfirm => "ui_confirm",
            SfxKind::UiBack => "ui_back",
            SfxKind::UiOpen => "ui_open",
            SfxKind::UiClose => "ui_close",
            SfxKind::CastRise1 => "cast_rise_1",
            SfxKind::CastRise2 => "cast_rise_2",
            SfxKind::CastRise3 => "cast_rise_3",
            SfxKind::CastRise4 => "cast_rise_4",
            SfxKind::CastRelease => "cast_release",
        }
    }
}

/// The loops under everything. Each is a bed of `jane-audio` by [`Bed::name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bed {
    Rain,
    RainRoof,
    Wind,
    Birds,
    Crickets,
    Lake,
    Hum,
    Cave,
    Fire,
    Clock,
}

impl Bed {
    pub const ALL: [Bed; 10] = [
        Bed::Rain,
        Bed::RainRoof,
        Bed::Wind,
        Bed::Birds,
        Bed::Crickets,
        Bed::Lake,
        Bed::Hum,
        Bed::Cave,
        Bed::Fire,
        Bed::Clock,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Bed::Rain => "rain",
            Bed::RainRoof => "rain_roof",
            Bed::Wind => "wind",
            Bed::Birds => "birds",
            Bed::Crickets => "crickets",
            Bed::Lake => "lake",
            Bed::Hum => "hum",
            Bed::Cave => "cave",
            Bed::Fire => "fire",
            Bed::Clock => "clock",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// What makes the sound (PRESENTATION.md §5). The app's is the SDL device and `jane-audio`;
/// [`NullBus`] is silence for `jane serve` and the tests.
pub trait AudioBus {
    fn music(&mut self, cue: MusicCue);
    /// A sound at `at`, heard from `listener`: attenuated over [`HEARING_CELLS`] and panned
    /// ([`place`]). A sound with no place is played at the listener.
    fn sfx(&mut self, kind: SfxKind, at: At, listener: At);
    /// A sound of hers carrying her growth (`might` in 256ths, [`crate::fx::might`]: 256 at New
    /// Game, 512 at the end): the app's plays it fuller and deeper. A bus that cannot, plays it
    /// as it is.
    fn sfx_with(&mut self, kind: SfxKind, at: At, listener: At, might: u16) {
        let _ = might;
        self.sfx(kind, at, listener);
    }
    /// A bed to a level (0 silent, 255 full); the bus fades it there.
    fn bed(&mut self, bed: Bed, level: u8);
    /// Once a tick, after everything else.
    fn tick(&mut self);
    /// The music and the beds to this share of their level, of 255 (a lesson's hush, §5.1);
    /// 255 lets them back. Effects are never ducked. A bus that cannot duck ignores it.
    fn duck(&mut self, _share: u8) {}
}

/// No sound at all.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullBus;

impl AudioBus for NullBus {
    fn music(&mut self, _cue: MusicCue) {}
    fn sfx(&mut self, _kind: SfxKind, _at: At, _listener: At) {}
    fn bed(&mut self, _bed: Bed, _level: u8) {}
    fn tick(&mut self) {}
}

/// The player's three volumes, 0 to 100 (`config.json`'s `volume`; the Controls screen's row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volumes {
    pub master: u8,
    pub music: u8,
    pub sfx: u8,
}

impl Default for Volumes {
    fn default() -> Volumes {
        Volumes { master: 80, music: 70, sfx: 80 }
    }
}

impl Volumes {
    /// As gains, on a square law so the steps sound even: (master, music, effects and beds).
    pub fn gains(self) -> (f32, f32, f32) {
        let g = |v: u8| {
            let x = f32::from(v.min(100)) / 100.0;
            x * x
        };
        (g(self.master), g(self.music), g(self.sfx))
    }
}

/// How far a sound carries, in cells: half the view across (40 cells).
pub const HEARING_CELLS: f32 = 20.0;

/// A sound placed for the listener: its gain (0 to 1), its pan (-1 left to 1 right) and how much
/// more of it the room gives back (further is wetter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub gain: f32,
    pub pan: f32,
    pub send: f32,
}

/// Where `at` is heard from `listener`, or `None` past [`HEARING_CELLS`]. The gain falls as the
/// square of the distance's remainder, so it reaches nothing smoothly at the edge.
pub fn place(at: At, listener: At) -> Option<Placed> {
    let dx = (at.0.0 - listener.0.0) as f32 / CELL_FX as f32;
    let dy = (at.1.0 - listener.1.0) as f32 / CELL_FX as f32;
    let d = (dx * dx + dy * dy).sqrt();
    if d >= HEARING_CELLS {
        return None;
    }
    let near = 1.0 - d / HEARING_CELLS;
    Some(Placed { gain: near * near, pan: (dx / 12.0).clamp(-1.0, 1.0) * 0.85, send: 0.35 * (1.0 - near) })
}

/// The ground under her feet, as her steps hear it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Grass,
    Road,
    Cobble,
    Wood,
    Water,
}

impl Surface {
    /// What a tile sounds like underfoot; `None` for what she cannot stand on.
    pub fn of(tile: Tile) -> Option<Surface> {
        Some(match tile {
            Tile::Grass
            | Tile::GrassTall
            | Tile::Garden
            | Tile::FlowerBed
            | Tile::Crops
            | Tile::Moss
            | Tile::GrownPath => Surface::Grass,
            Tile::Dirt | Tile::Road | Tile::Track | Tile::Sand | Tile::DryBed | Tile::Rubble | Tile::Rail => {
                Surface::Road
            }
            Tile::Cobble
            | Tile::Stepping
            | Tile::Floor
            | Tile::CaveFloor
            | Tile::TempleFloor
            | Tile::MuseumFloor
            | Tile::SchoolFloor
            | Tile::Sill
            | Tile::WorksFloor
            | Tile::PipeFloor
            | Tile::Ice => Surface::Cobble,
            Tile::FloorWood | Tile::Boardwalk => Surface::Wood,
            Tile::Water => Surface::Water,
            _ => return None,
        })
    }

    /// Soft ground that has taken the rain splashes.
    pub fn wet(self, wetness: u8) -> Surface {
        match self {
            Surface::Grass | Surface::Road if wetness >= WET_STEPS => Surface::Water,
            s => s,
        }
    }

    pub const fn sfx(self) -> SfxKind {
        match self {
            Surface::Grass => SfxKind::StepGrass,
            Surface::Road => SfxKind::StepRoad,
            Surface::Cobble => SfxKind::StepCobble,
            Surface::Wood => SfxKind::StepWood,
            Surface::Water => SfxKind::StepWater,
        }
    }
}

/// The wetness at which grass and road splash underfoot (the puddles' own line is lower).
pub const WET_STEPS: u8 = 150;

/// What the cue table reads of the world each tick: a plain struct, so the table is tested
/// without a county.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sense {
    pub seat: Seat,
    pub me: UnitId,
    pub pos: At,
    pub alive: bool,
    pub zone: ZoneId,
    pub region: Region,
    pub indoor: bool,
    pub night: bool,
    /// Ticks since New Game's midnight: `day * TICKS_PER_DAY + clock`.
    pub abs: u64,
    pub weather: WeatherKind,
    /// The ground under her, rain included.
    pub surface: Option<Surface>,
    /// A hostile is fighting her.
    pub hostile: bool,
    /// The School's bell has stopped: the one flag the table reads for the bell, since the nine
    /// it leaves silent is marked by no event.
    pub bell_stopped: bool,
    pub the_end: bool,
    pub seed: u32,
    /// How near a lit fire is (0 none, 255 beside it): a campfire, a brazier or a stove within
    /// [`FIRE_CELLS`], doused ones not.
    pub fire: u8,
    /// The dog, where it stands, if it is out and within hearing; and whether something hostile
    /// is fighting within six cells of it.
    pub dog: Option<At>,
    pub dog_alarmed: bool,
    /// Her growth as her casts and blows carry it: her spirit's and her strength's
    /// [`crate::fx::might`].
    pub might: (u16, u16),
    /// Her cast building (`View::fight`): the tick it began, and how far along in 256ths.
    pub building: Option<(u32, u16)>,
}

/// How far a fire's crackle carries, in cells.
pub const FIRE_CELLS: i32 = 8;

/// Whether a prop is a fire the ear should find: a campfire, a brazier, a stove.
pub fn is_fire(def_id: &str) -> bool {
    (def_id.contains("fire") || def_id.contains("brazier") || def_id.contains("stove")) && !def_id.contains("scroll")
}

fn at(v: Vec2) -> At {
    (v.x, v.y)
}

/// Cells between two places.
fn cells(a: At, b: At) -> f32 {
    let dx = (a.0.0 - b.0.0) as f32 / CELL_FX as f32;
    let dy = (a.1.0 - b.1.0) as f32 / CELL_FX as f32;
    (dx * dx + dy * dy).sqrt()
}

impl Sense {
    /// Reads a seat's view.
    pub fn of(view: &View<'_>) -> Sense {
        let body = view.body();
        let me = view.me().unit;
        let (clock, day) = view.clock();
        let (cx, cy) = body.pos.cell();
        let surface = Surface::of(view.tile(cx, cy)).map(|s| {
            if view.indoor() {
                s
            } else {
                let wet = u16::try_from(cx)
                    .ok()
                    .zip(u16::try_from(cy).ok())
                    .map_or(0, |(x, y)| view.wetness_at(jane_core::Cell { x, y }));
                s.wet(wet)
            }
        });
        let r = HEARING_CELLS as i32;
        let area = jane_core::Rect::new(cx - r, cy - r, 2 * r, 2 * r);
        let mut hostile = false;
        let units = &jane_data::catalog().combat.units;
        let dog_def = units.iter().position(|u| u.id == "dog");
        let mut dog = None;
        let mut fighting: Vec<At> = Vec::new();
        for u in view.units_in(area) {
            let u = u.unit;
            let angry = u.alive && u.faction != Faction::Friendly && u.combat == CombatState::Combat;
            if angry && u.target == Some(me) {
                hostile = true;
            }
            if angry {
                fighting.push(at(u.pos));
            }
            if u.alive && Some(u.def.index()) == dog_def {
                dog = Some(at(u.pos));
            }
        }
        let dog_alarmed = dog.is_some_and(|d| fighting.iter().any(|f| cells(*f, d) < 6.0));
        let mut fire = 0u8;
        let near = jane_core::Rect::new(cx - FIRE_CELLS, cy - FIRE_CELLS, 2 * FIRE_CELLS, 2 * FIRE_CELLS);
        for p in view.props_in(near) {
            let def = jane_data::catalog().story.prop(p.def);
            if !is_fire(def.id) || view.light_showing(p).is_none() {
                continue;
            }
            let fx = f32::from(p.cell.x) + f32::from(def.w) / 2.0 - (cx as f32 + 0.5);
            let fy = f32::from(p.cell.y) + f32::from(def.h) / 2.0 - (cy as f32 + 0.5);
            let near = (1.0 - (fx * fx + fy * fy).sqrt() / FIRE_CELLS as f32).max(0.0);
            fire = fire.max((255.0 * near * near) as u8);
        }
        Sense {
            seat: view.seat(),
            me,
            pos: at(body.pos),
            alive: body.alive,
            zone: view.zone(),
            region: view.region(),
            indoor: view.indoor(),
            night: view.is_night(),
            abs: u64::from(day) * u64::from(TICKS_PER_DAY) + u64::from(clock),
            weather: view.weather().kind,
            surface,
            hostile,
            bell_stopped: view.flag("bell_stopped") != 0,
            the_end: view.the_end() != 0,
            seed: view.seed(),
            fire,
            dog,
            dog_alarmed,
            might: (crate::fx::might(body.spirit), crate::fx::might(body.strength)),
            building: view.fight().cast.map(|c| {
                let len = c.done.0.saturating_sub(c.started.0).max(1);
                (c.started.0, (view.tick().0.saturating_sub(c.started.0).min(len) * 256 / len) as u16)
            }),
        }
    }

    fn clock(&self) -> u32 {
        (self.abs % u64::from(TICKS_PER_DAY)) as u32
    }

    fn hour(&self) -> u32 {
        self.clock() / TICKS_PER_HOUR
    }
}

/// Ticks a fight must last before the music takes it up, and after its last blow before it lets
/// it go (§5: 2 s and 6 s).
pub const COMBAT_AFTER: u32 = 120;
pub const COMBAT_TAIL: u32 = 360;
/// Ticks between the bell's strikes: a hand-rung tower bell, a stroke and the swing back.
pub const STRIKE_TICKS: u32 = 150;
/// Ticks between the church's strikes (a smaller, quicker bell).
const CHURCH_TICKS: u32 = 84;
/// Ticks the bell's hush lasts after its last strike, while the last one rings out.
const BELL_TAIL: u32 = 300;
/// Ticks between the dog's barks, and between its panting at her side.
const DOG_BARK_TICKS: u32 = 300;
const DOG_PANT_TICKS: u32 = 1200;
/// Ticks of silence where the nine o'clock bell used to be.
const NO_BELL: u32 = 480;
/// Ticks of walking between footfalls: half the walk cycle (six frames of `WALK_TICKS`), so each
/// step lands on a frame where a foot comes down, whatever her speed.
pub const STEP_TICKS: u32 = crate::people::WALK_TICKS * 3;

/// A bell being rung (`EventKind::Bell`), a strike at a time: where it hangs, which bell, the
/// strikes left and the ticks to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Toll {
    hangs: Option<(ZoneId, At)>,
    church: bool,
    left: u8,
    wait: u32,
}

impl Toll {
    /// Ticks between this bell's strikes.
    const fn spacing(&self) -> u32 {
        if self.church { CHURCH_TICKS } else { STRIKE_TICKS }
    }
}

/// The cue table: all the state the sound of the game keeps between ticks.
#[derive(Clone, Debug, Default)]
pub struct Soundtrack {
    cue: Option<MusicCue>,
    engaged: u32,
    calm: u32,
    fighting: bool,
    dead: bool,
    last_abs: Option<u64>,
    /// The bells ringing now: an hour's toll and a ringer's stroke can overlap.
    tolls: Vec<Toll>,
    hush: u32,
    no_bell: u32,
    /// Ticks she has been walking without a stop.
    walking: u32,
    last_pos: Option<At>,
    beds: [u8; 10],
    zone: Option<ZoneId>,
    /// The dog as last heard, whether she was beside it, and ticks before it pants or barks again.
    dog: Option<At>,
    by_dog: bool,
    dog_pant: u32,
    dog_bark: u32,
    /// Ticks to the next owl, crow or thunder, and the draw that sets them.
    wild: u32,
    thunder: u32,
    rng: u32,
    pub ticks: u32,
    /// Her cast building as last heard: when it began, and the swells already played.
    rising: Option<(u32, u8)>,
    /// Ticks to the next of the far calls.
    far_wait: u32,
}

/// The swell for a cast `frac` 256ths of the way: a step a quarter.
pub const fn rise_step(frac: u16) -> u8 {
    if frac >= 192 { 3 } else { (frac / 64) as u8 }
}

/// What is heard before it is seen, about a screen out: bees at a hive, the diver knocking under
/// the water while he is down, the night shift's boots after dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FarCall {
    Bees,
    Knocking,
    Boots,
}

impl FarCall {
    pub const fn sfx(self) -> SfxKind {
        match self {
            FarCall::Bees => SfxKind::Bees,
            FarCall::Knocking => SfxKind::Knocking,
            FarCall::Boots => SfxKind::Boots,
        }
    }
}

/// How far a far call carries, cells: a screen and more across (40 cells), from her.
pub const FAR_CELLS: i32 = 37;
/// Ticks between far calls, at least (as much again at most).
const FAR_TICKS: u32 = 150;

/// The nearest source of each far call within [`FAR_CELLS`] of her: a hive, the diver's pump
/// while his helmet is not yet up, the night shift on the move (after dark).
pub fn far_sources(view: &View<'_>, night: bool) -> Vec<(FarCall, At)> {
    let cat = jane_data::catalog();
    let (cx, cy) = view.body().pos.cell();
    let me = at(view.body().pos);
    let area = jane_core::Rect::new(cx - FAR_CELLS, cy - FAR_CELLS, 2 * FAR_CELLS, 2 * FAR_CELLS);
    let mut best: [Option<(f32, At)>; 3] = [None; 3];
    let mut offer = |k: usize, p: At| {
        let d = cells(p, me);
        if d <= FAR_CELLS as f32 && best[k].is_none_or(|b| d < b.0) {
            best[k] = Some((d, p));
        }
    };
    let mut helmet_up = false;
    for p in view.props_in(area) {
        let d = cat.story.prop(p.def);
        let c = (Fx(i32::from(p.cell.x) * CELL_FX + CELL_FX / 2), Fx(i32::from(p.cell.y) * CELL_FX + CELL_FX / 2));
        match d.id {
            "beehive" | "tale_hive" if !p.hidden => offer(0, c),
            "tale_air_pump" => offer(1, c),
            "tale_helmet" if !p.hidden => helmet_up = true,
            _ => {}
        }
    }
    if night {
        let shift = cat.combat.units.iter().position(|u| u.id == "night_skeleton");
        for u in view.units_in(area) {
            let u = u.unit;
            if u.alive && Some(u.def.index()) == shift {
                offer(2, at(u.pos));
            }
        }
    }
    if helmet_up {
        best[1] = None;
    }
    let calls = [FarCall::Bees, FarCall::Knocking, FarCall::Boots];
    calls.into_iter().zip(best).filter_map(|(c, b)| b.map(|b| (c, b.1))).collect()
}

/// Where a far call at `src` is put for the bus to hear from `listener`: along the true bearing,
/// with the distance drawn in so [`FAR_CELLS`] lands at the edge of [`HEARING_CELLS`]: faint and
/// to one side a screen out, full when she is on it. `None` past [`FAR_CELLS`].
pub fn far_heard(src: At, listener: At) -> Option<At> {
    let d = cells(src, listener);
    if d > FAR_CELLS as f32 {
        return None;
    }
    let k = (HEARING_CELLS - 1.0) / FAR_CELLS as f32;
    let pull = |a: Fx, b: Fx| Fx(b.0 + ((a.0 - b.0) as f32 * k) as i32);
    Some((pull(src.0, listener.0), pull(src.1, listener.1)))
}

/// The two clock times the table keeps itself: the nine a stopped bell leaves silent (no event
/// marks it), and the Sunday train. When a bell rings is the sim's (`data/clock.json`).
const NINE: u32 = 21 * TICKS_PER_HOUR;
const TRAIN: u32 = 13 * TICKS_PER_HOUR + TICKS_PER_HOUR / 30;

/// The latest day on which clock time `at` fell in `(prev, now]`, if it did.
fn crossed(prev: u64, now: u64, at: u32) -> Option<u64> {
    let day = u64::from(TICKS_PER_DAY);
    let (d0, d1) = (prev / day, now / day);
    (d0..=d1).rev().find(|d| {
        let t = d * day + u64::from(at);
        prev < t && t <= now
    })
}

/// Which bell she hears for one strike, and from where. The church's small bell where it hangs if
/// she is near it, else anywhere in the town (the church, the Arms, the house, the Lowfields),
/// else not at all. The School's where it hangs (or its ringer pulls it) within hearing; through
/// the walls or the earth anywhere but the open county; else across the county.
fn bell_heard(hangs: Option<(ZoneId, At)>, church: bool, s: &Sense) -> Option<(SfxKind, At)> {
    let near = hangs.filter(|&(z, p)| z == s.zone && place(p, s.pos).is_some_and(|q| q.gain > 0.02)).map(|h| h.1);
    if church {
        let town = matches!(s.zone, ZoneId::Church | ZoneId::Arms | ZoneId::House)
            || (s.zone == ZoneId::County && s.region == Region::Lowfields);
        return match near {
            Some(p) => Some((SfxKind::ChurchBell, p)),
            None => town.then_some((SfxKind::ChurchBell, s.pos)),
        };
    }
    Some(match near {
        Some(p) => (SfxKind::BellNear, p),
        None if s.indoor || s.zone != ZoneId::County => (SfxKind::BellWithin, s.pos),
        None => (SfxKind::BellFar, s.pos),
    })
}

impl Soundtrack {
    pub fn new() -> Soundtrack {
        Soundtrack::default()
    }

    /// The title screen or the loading screen: the theme, and nothing else.
    pub fn title(&mut self, bus: &mut dyn AudioBus) {
        self.set_music(MusicCue::Title, bus);
        for b in Bed::ALL {
            self.set_bed(b, 0, bus);
        }
        self.last_abs = None;
        self.last_pos = None;
        self.zone = None;
        self.tolls.clear();
        self.dead = false;
        self.fighting = false;
        self.ticks = self.ticks.wrapping_add(1);
        bus.duck(255);
        bus.tick();
    }

    /// A lesson's moment (§5.1, `crate::lesson`): its sound as it begins, at the listener (it is
    /// hers, wherever she stands), and the music and the beds stepped back under its hush. Each
    /// machine at a table hears its own seat's.
    pub fn lesson(&mut self, l: &crate::lesson::Lessons, bus: &mut dyn AudioBus) {
        if let Some(g) = l.began() {
            self.ui(SfxKind::of_gift(g), bus);
        }
        bus.duck(l.duck());
    }

    /// A sound of the UI: at the listener, unplaced.
    pub fn ui(&mut self, kind: SfxKind, bus: &mut dyn AudioBus) {
        bus.sfx(kind, (Fx(0), Fx(0)), (Fx(0), Fx(0)));
    }

    /// One tick of play: this seat's view and the tick's events.
    pub fn tick(&mut self, view: &View<'_>, events: &[Event], bus: &mut dyn AudioBus) {
        let s = Sense::of(view);
        let locate = |p: PropId| {
            view.prop(p).map(|p| {
                let h = CELL_FX / 2;
                (Fx(i32::from(p.cell.x) * CELL_FX + h), Fx(i32::from(p.cell.y) * CELL_FX + h))
            })
        };
        self.far_calls(view, &s, bus);
        self.step(&s, events, &locate, bus);
    }

    /// The things worth finding that are heard before they are seen, about a screen out (the
    /// world audit's last fifty metres): every so often the nearest of each kind calls, placed
    /// by [`far_heard`] so it is faint and to one side at the edge and grows as she comes in.
    fn far_calls(&mut self, view: &View<'_>, s: &Sense, bus: &mut dyn AudioBus) {
        if self.rng == 0 || !s.alive || s.zone != ZoneId::County {
            return;
        }
        if self.far_wait > 0 {
            self.far_wait -= 1;
            return;
        }
        self.far_wait = FAR_TICKS + self.draw() % FAR_TICKS;
        for (call, src) in far_sources(view, s.night) {
            if let Some(p) = far_heard(src, s.pos) {
                bus.sfx(call.sfx(), p, s.pos);
            }
        }
    }

    /// [`Soundtrack::tick`] over a [`Sense`]: the whole table, testable without a county.
    pub fn step(&mut self, s: &Sense, events: &[Event], locate: &dyn Fn(PropId) -> Option<At>, bus: &mut dyn AudioBus) {
        self.ticks = self.ticks.wrapping_add(1);
        if self.rng == 0 {
            self.rng = s.seed | 1;
        }
        let me = s.pos;
        // Her cast building: a swell a quarter, each a step higher, the tone rising to the
        // release (PLAY-PLAN §2.1); a cast cut short stops rising.
        match s.building {
            Some((began, frac)) => {
                let step = rise_step(frac);
                let from = match self.rising {
                    Some((b, played)) if b == began => played + 1,
                    _ => 0,
                };
                // Only the newest swell: a late frame does not stack the ones it missed.
                if from <= step {
                    let rises = [SfxKind::CastRise1, SfxKind::CastRise2, SfxKind::CastRise3, SfxKind::CastRise4];
                    bus.sfx_with(rises[usize::from(step)], me, me, s.might.0);
                    self.rising = Some((began, step));
                }
            }
            None => self.rising = None,
        }
        let mut hostile = s.hostile;
        // The tick's events, those that are hers to hear.
        for e in events.iter().filter(|e| e.to.is_none_or(|t| t == s.seat) && e.in_zone.is_none_or(|z| z == s.zone)) {
            match e.kind {
                EventKind::Sfx { kind, at: p } => {
                    let k = match kind {
                        SimSfx::Strike => SfxKind::Strike,
                        SimSfx::Locked => SfxKind::Locked,
                        SimSfx::Push => SfxKind::Push,
                        SimSfx::PlateDown => SfxKind::PlateDown,
                        SimSfx::PlateUp => SfxKind::PlateUp,
                        SimSfx::Kindle => SfxKind::HitFire,
                    };
                    bus.sfx(k, at(p), me);
                }
                EventKind::Swing { unit, at: p, .. } => {
                    let m = if unit == s.me { s.might.1 } else { 256 };
                    bus.sfx_with(SfxKind::Swing, at(p), me, m);
                }
                // A foe's tell, heard where its blow will land.
                EventKind::Windup { at: p, interruptible, .. } => {
                    bus.sfx(if interruptible { SfxKind::Windup } else { SfxKind::WindupSteady }, at(p), me);
                }
                EventKind::Hop { at: p, .. } => bus.sfx(SfxKind::Hop, at(p), me),
                EventKind::Impact { spell, school, at: p } => {
                    let k = match school {
                        School::Heal => SfxKind::Heal,
                        School::Physical => SfxKind::HitPhysical,
                        School::Frost => SfxKind::HitFrost,
                        School::Fire => SfxKind::HitFire,
                        School::Nature => SfxKind::HitNature,
                        School::Blast => SfxKind::HitBlast,
                        School::Shock => SfxKind::HitShock,
                    };
                    let m = match (crate::fx::players_spell(spell), school) {
                        (false, _) => 256,
                        (true, School::Physical) => s.might.1,
                        (true, _) => s.might.0,
                    };
                    bus.sfx_with(k, at(p), me, m);
                }
                EventKind::Damage { unit, from, at: p, crit, .. } => {
                    if unit == s.me {
                        bus.sfx(SfxKind::Hurt, me, me);
                        hostile |= from.is_some_and(|f| f != s.me);
                    } else if crit {
                        bus.sfx(SfxKind::Crit, at(p), me);
                    }
                    if from == Some(s.me) && unit != s.me {
                        hostile = true;
                    }
                }
                EventKind::Heal { unit, .. } if unit == s.me => bus.sfx(SfxKind::Heal, me, me),
                EventKind::Death { unit, at: p, .. } if unit != s.me => bus.sfx(SfxKind::Death, at(p), me),
                EventKind::Cast { unit, spell, at: p } => {
                    let m = if unit == s.me { s.might.0 } else { 256 };
                    // A cast that built lands with a thump and a crack; an instant as before.
                    let built = jane_data::catalog().combat.spell(spell).cast.0 > 0;
                    bus.sfx_with(if built { SfxKind::CastRelease } else { SfxKind::Cast }, at(p), me, m);
                }
                EventKind::CastFailed { unit, why, .. } if unit == s.me && why.says() => {
                    bus.sfx(SfxKind::CastFailed, me, me);
                }
                EventKind::Status { unit, on: true, .. } if unit == s.me => bus.sfx(SfxKind::Status, me, me),
                EventKind::Respawn { unit } if unit == s.me => {
                    self.dead = false;
                    bus.sfx(SfxKind::Respawn, me, me);
                }
                EventKind::PlayerDied => self.dead = true,
                EventKind::Loot { .. } => bus.sfx(SfxKind::Loot, me, me),
                EventKind::Quest { change: QuestChange::Given, .. } => bus.sfx(SfxKind::QuestGiven, me, me),
                EventKind::Quest { change: QuestChange::Done, .. } => bus.sfx(SfxKind::QuestDone, me, me),
                EventKind::Journal(_) => bus.sfx(SfxKind::Journal, me, me),
                EventKind::Rest => bus.sfx(SfxKind::Rest, me, me),
                EventKind::Weather { kind: WeatherKind::Storm, .. } => self.thunder = self.thunder.min(40),
                EventKind::Bell { strikes, at: hangs, church } => {
                    self.ring(strikes, hangs.map(|(z, p)| (z, at(p))), church);
                }
                EventKind::Prop { prop, change } => {
                    let k = match change {
                        PropChange::Open => SfxKind::Open,
                        PropChange::Use => SfxKind::Use,
                        PropChange::Unlock => SfxKind::Unlock,
                        PropChange::Lock => SfxKind::Locked,
                        PropChange::Switch => SfxKind::Switch,
                        // A push is heard as the sim's own `Sfx::Push`; showing and hiding are silent.
                        PropChange::Push | PropChange::Show | PropChange::Hide => continue,
                    };
                    bus.sfx(k, locate(prop).unwrap_or(me), me);
                }
                _ => {}
            }
        }
        // A door: into another zone.
        let same_zone = self.zone == Some(s.zone);
        if self.zone.is_some_and(|z| z != s.zone) {
            bus.sfx(SfxKind::Door, me, me);
            self.last_pos = None;
        }
        self.zone = Some(s.zone);
        self.the_dog(s, same_zone, bus);
        if !s.alive {
            self.dead = true;
        } else if self.dead && s.alive {
            self.dead = false;
        }
        // The fight: 2 s of it before the music turns, 6 s of quiet before it turns back.
        if hostile && s.alive {
            self.engaged += 1;
            self.calm = 0;
        } else {
            self.calm = self.calm.saturating_add(1);
            if !self.fighting && self.calm > 60 {
                self.engaged = 0;
            }
        }
        if !self.fighting && self.engaged >= COMBAT_AFTER {
            self.fighting = true;
        }
        if self.fighting && (self.calm >= COMBAT_TAIL || self.dead) {
            self.fighting = false;
            self.engaged = 0;
        }
        self.clock(s, bus);
        self.footsteps(s, bus);
        self.wildlife(s, bus);
        self.beds(s, bus);
        // The music.
        self.hush = self.hush.saturating_sub(1);
        self.no_bell = self.no_bell.saturating_sub(1);
        let cue = if s.the_end {
            MusicCue::Silence
        } else if self.dead {
            MusicCue::Dead
        } else if self.hush > 0 {
            MusicCue::Bell
        } else if self.no_bell > 0 {
            MusicCue::Silence
        } else if self.fighting {
            MusicCue::Combat
        } else {
            MusicCue::zone(s.zone, s.region, s.night)
        };
        self.set_music(cue, bus);
        bus.tick();
    }

    /// The music the table last asked for.
    pub fn cue(&self) -> Option<MusicCue> {
        self.cue
    }

    fn set_music(&mut self, cue: MusicCue, bus: &mut dyn AudioBus) {
        if self.cue != Some(cue) {
            self.cue = Some(cue);
            bus.music(cue);
        }
    }

    fn set_bed(&mut self, bed: Bed, level: u8, bus: &mut dyn AudioBus) {
        if self.beds[bed.index()] != level {
            self.beds[bed.index()] = level;
            bus.bed(bed, level);
        }
    }

    /// The level each bed asks for now.
    pub fn bed_level(&self, bed: Bed) -> u8 {
        self.beds[bed.index()]
    }

    /// The train by the clock, the nine a stopped bell leaves, and the bells rung, a strike at a
    /// time.
    fn clock(&mut self, s: &Sense, bus: &mut dyn AudioBus) {
        if let Some(prev) = self.last_abs.filter(|&p| p < s.abs) {
            // "Nine o'clock. No bell." The music stops to listen for it: the one bell rule kept
            // here, since a bell that is not rung sends no event. A sleep through nine is not it.
            if s.bell_stopped && s.abs - prev < u64::from(TICKS_PER_HOUR) && crossed(prev, s.abs, NINE).is_some() {
                self.no_bell = NO_BELL;
            }
            if let Some(d) = crossed(prev, s.abs, TRAIN)
                && d % 7 == 0
            {
                bus.sfx(SfxKind::TrainWhistle, s.pos, s.pos);
            }
        }
        self.last_abs = Some(s.abs);
        for t in &mut self.tolls {
            if t.wait == 0 {
                if let Some((kind, pos)) = bell_heard(t.hangs, t.church, s) {
                    bus.sfx(kind, pos, s.pos);
                }
                t.left -= 1;
                t.wait = t.spacing() - 1;
            } else {
                t.wait -= 1;
            }
        }
        self.tolls.retain(|t| t.left > 0);
    }

    /// Julie's dog: a bark when it comes out in the morning or something fights near it, a whine
    /// when it goes for the night while she is near, and panting when she comes to its side.
    fn the_dog(&mut self, s: &Sense, same_zone: bool, bus: &mut dyn AudioBus) {
        self.dog_pant = self.dog_pant.saturating_sub(1);
        self.dog_bark = self.dog_bark.saturating_sub(1);
        match (self.dog, s.dog) {
            (None, Some(d)) if same_zone && self.ticks > 2 => {
                bus.sfx(SfxKind::DogBark, d, s.pos);
                self.dog_bark = DOG_BARK_TICKS;
            }
            (Some(d), None) if same_zone && s.alive && cells(d, s.pos) < 12.0 => bus.sfx(SfxKind::DogWhine, d, s.pos),
            _ => {}
        }
        if let Some(d) = s.dog {
            if s.dog_alarmed && self.dog_bark == 0 {
                bus.sfx(SfxKind::DogBark, d, s.pos);
                self.dog_bark = DOG_BARK_TICKS;
            }
            let by = cells(d, s.pos) < 3.0;
            if by && !self.by_dog && self.dog_pant == 0 {
                bus.sfx(SfxKind::DogPant, d, s.pos);
                self.dog_pant = DOG_PANT_TICKS;
            }
            self.by_dog = by;
        } else {
            self.by_dog = false;
        }
        self.dog = s.dog;
    }

    /// A bell the sim rang (`EventKind::Bell`): its strikes from this tick on. The School's bell
    /// striking an hour hushes the music under it and while the last stroke rings out; a single
    /// stroke (the Timekeeper's rope, in a fight) and the church do not.
    fn ring(&mut self, strikes: u8, hangs: Option<(ZoneId, At)>, church: bool) {
        if strikes == 0 {
            return;
        }
        let t = Toll { hangs, church, left: strikes, wait: 0 };
        if !church && strikes > 1 {
            self.hush = self.hush.max(u32::from(strikes) * t.spacing() + BELL_TAIL);
        }
        self.tolls.push(t);
    }

    /// Her steps, in time with the walk cycle, on what is under her: the first as she steps
    /// off (the drawing's first contact), then every [`STEP_TICKS`] while she keeps moving.
    fn footsteps(&mut self, s: &Sense, bus: &mut dyn AudioBus) {
        if !s.alive {
            self.last_pos = None;
            self.walking = 0;
            return;
        }
        let moved = self.last_pos.map(|last| {
            let dx = i64::from(s.pos.0.0 - last.0.0);
            let dy = i64::from(s.pos.1.0 - last.1.0);
            dx * dx + dy * dy
        });
        let reach = i64::from(3 * CELL_FX);
        match moved {
            // A door or a warp is not a walk; standing still ends one.
            Some(d2) if d2 > 0 && d2 <= reach * reach => {
                if self.walking % STEP_TICKS == 0
                    && let Some(surface) = s.surface
                {
                    bus.sfx(surface.sfx(), s.pos, s.pos);
                }
                self.walking += 1;
            }
            _ => self.walking = 0,
        }
        self.last_pos = Some(s.pos);
    }

    fn draw(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Things that live out there: an owl at night, a crow over the fields by day, thunder in a storm.
    fn wildlife(&mut self, s: &Sense, bus: &mut dyn AudioBus) {
        let out = !s.indoor && s.zone == ZoneId::County;
        if self.wild == 0 {
            self.wild = 1800 + self.draw() % 3600;
            if out && s.weather != WeatherKind::Storm {
                let kind = if s.night && s.region != Region::Works {
                    Some(SfxKind::Owl)
                } else if !s.night && s.region == Region::Lowfields && s.weather == WeatherKind::Clear {
                    Some(SfxKind::Crow)
                } else {
                    None
                };
                if let Some(k) = kind {
                    // Somewhere off in the dark, ten to eighteen cells away.
                    let a = (self.draw() % 360) as f32 * std::f32::consts::PI / 180.0;
                    let r = (10 + self.draw() % 8) as f32 * CELL_FX as f32;
                    let p = (Fx(s.pos.0.0 + (a.cos() * r) as i32), Fx(s.pos.1.0 + (a.sin() * r) as i32));
                    bus.sfx(k, p, s.pos);
                }
            }
        }
        self.wild -= 1;
        if s.weather == WeatherKind::Storm && !matches!(s.zone, ZoneId::Mine | ZoneId::Pipes | ZoneId::Cellar) {
            if self.thunder == 0 {
                self.thunder = 720 + self.draw() % 1500;
                bus.sfx(SfxKind::Thunder, s.pos, s.pos);
            }
            self.thunder -= 1;
        } else {
            self.thunder = self.thunder.max(120);
        }
    }

    /// The beds, by where she is, the hour and the sky.
    fn beds(&mut self, s: &Sense, bus: &mut dyn AudioBus) {
        let l = bed_levels(s);
        for b in Bed::ALL {
            self.set_bed(b, l[b.index()], bus);
        }
    }
}

/// What each bed should be at for this sense of the world.
pub fn bed_levels(s: &Sense) -> [u8; 10] {
    let mut l = [0u8; 10];
    if !s.alive {
        return l;
    }
    let hour = s.hour();
    let out = !s.indoor;
    let underground = matches!(s.zone, ZoneId::Mine | ZoneId::Pipes | ZoneId::Cellar);
    let wet = matches!(s.weather, WeatherKind::Rain | WeatherKind::Storm);
    let storm = s.weather == WeatherKind::Storm;
    if wet {
        if out {
            l[Bed::Rain.index()] = if storm { 255 } else { 170 };
        } else if !underground {
            l[Bed::RainRoof.index()] = if storm { 220 } else { 150 };
        }
    }
    if out {
        let base: u16 = match s.region {
            Region::Lowfields => 70,
            Region::Waters => 55,
            Region::Works => 95,
        };
        let w = match s.weather {
            WeatherKind::Storm => 235,
            WeatherKind::Rain => base + 50,
            WeatherKind::Mist => 40,
            WeatherKind::Clear => base + if s.night { 30 } else { 0 },
        };
        l[Bed::Wind.index()] = w.min(255) as u8;
        let county = s.zone == ZoneId::County;
        let forest = s.zone == ZoneId::Forest;
        // The birds: a dawn chorus, a quiet noon, a full evening before the bell; never in the Works.
        if (county && s.region != Region::Works) || forest {
            let birds: u8 = match hour {
                5..=7 => 190,
                8..=16 => 90,
                17..=20 => 220,
                _ => 0,
            };
            l[Bed::Birds.index()] = if wet { birds / 3 } else { birds };
        }
        // Crickets from eight in the evening to four in the morning, not in the rain.
        if county && !wet && !(4..20).contains(&hour) {
            l[Bed::Crickets.index()] = match (s.region, hour) {
                (Region::Works, _) => 40,
                (_, 20) => 100,
                _ => 170,
            };
        }
        if county && s.region == Region::Waters {
            l[Bed::Lake.index()] = 150;
        }
    }
    let hum = match (s.zone, s.region) {
        (ZoneId::County, Region::Works) if out => {
            if s.night {
                170
            } else {
                110
            }
        }
        (ZoneId::Factory, _) => 200,
        (ZoneId::Pipes, _) => 90,
        _ => 0,
    };
    l[Bed::Hum.index()] = hum;
    l[Bed::Cave.index()] = match s.zone {
        ZoneId::Mine => 220,
        ZoneId::Pipes => 180,
        ZoneId::Cellar => 120,
        ZoneId::Burial => 90,
        _ => 0,
    };
    // The Arms' fire and the house's range, and any lit fire she stands near: a campfire, a
    // brazier, a stove.
    let hearth = match s.zone {
        ZoneId::Arms => 150,
        ZoneId::House => 60,
        _ => 0,
    };
    l[Bed::Fire.index()] = hearth.max(s.fire);
    l[Bed::Clock.index()] = match s.zone {
        ZoneId::House => 110,
        ZoneId::Library | ZoneId::Museum => 140,
        _ => 0,
    };
    l
}

#[cfg(test)]
mod far_tests {
    use super::*;

    #[test]
    fn a_far_call_is_heard_a_screen_out_and_grows_as_she_comes_in() {
        let c = |x: i32| (Fx(x * CELL_FX), Fx(0));
        let me = c(0);
        let edge = far_heard(c(34), me).and_then(|p| place(p, me)).expect("most of a screen out, still heard");
        let near = far_heard(c(10), me).and_then(|p| place(p, me)).expect("near");
        assert!(edge.gain > 0.0 && edge.gain < 0.05, "faint at the edge ({})", edge.gain);
        assert!(near.gain > edge.gain * 5.0, "fuller as she comes in");
        assert!(edge.pan > 0.5, "and to the side it is on");
        assert!(far_heard(c(FAR_CELLS + 2), me).is_none());
    }

    #[test]
    fn a_hive_on_the_county_is_heard_from_off_the_screen() {
        // Seed 7: stand her 30 cells from a hive, and the bees are among the far calls.
        let mut sim = jane_sim::Sim::new_game(7, "Tess");
        let county = sim.view(jane_sim::Seat(0)).unwrap().blueprints().get(ZoneId::County).clone();
        let cat = jane_data::catalog();
        let hives: Vec<(i32, i32)> = county
            .props
            .iter()
            .filter(|p| matches!(cat.story.prop(p.def).id, "beehive" | "tale_hive") && !p.hidden)
            .map(|p| (i32::from(p.cell.x), i32::from(p.cell.y)))
            .collect();
        assert!(!hives.is_empty(), "the county keeps bees");
        let open = |x: i32, y: i32| {
            county.tiles.inside(x, y)
                && county.tiles.read(x, y, jane_core::Tile::Void).flags() & jane_core::tile::F_SOLID == 0
        };
        // Somewhere every hive is at least 26 cells off, and one is within 36.
        let to = hives
            .iter()
            .flat_map(|&(hx, hy)| {
                const AROUND: [(i32, i32); 8] =
                    [(30, 0), (21, 21), (0, 30), (-21, 21), (-30, 0), (-21, -21), (0, -30), (21, -21)];
                AROUND.into_iter().map(move |(dx, dy)| (hx + dx, hy + dy))
            })
            .find(|&(x, y)| {
                let d2 = |h: &(i32, i32)| (h.0 - x).pow(2) + (h.1 - y).pow(2);
                open(x, y) && hives.iter().all(|h| d2(h) >= 26 * 26) && hives.iter().any(|h| d2(h) <= 36 * 36)
            })
            .expect("open ground a screen out from the bees");
        let (z, id) = (sim.state().players[0].zone, sim.state().players[0].unit);
        assert_eq!(z, ZoneId::County);
        sim.state_mut().zone_mut(z).and_then(|zs| zs.unit_mut(id)).expect("her body").pos =
            jane_core::Vec2::centre(to.0, to.1);
        sim.rebuild_runtimes();
        let v = sim.view(jane_sim::Seat(0)).unwrap();
        let calls = far_sources(&v, false);
        let bees = calls.iter().find(|c| c.0 == FarCall::Bees).expect("the bees are heard");
        let me = at(v.body().pos);
        assert!(cells(bees.1, me) > 20.0, "from off the screen");
        assert!(far_heard(bees.1, me).and_then(|p| place(p, me)).is_some_and(|q| q.gain > 0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bus that writes down what it was asked.
    #[derive(Default)]
    struct Heard {
        music: Vec<MusicCue>,
        sfx: Vec<(SfxKind, At)>,
        beds: Vec<(Bed, u8)>,
    }

    impl AudioBus for Heard {
        fn music(&mut self, cue: MusicCue) {
            self.music.push(cue);
        }
        fn sfx(&mut self, kind: SfxKind, at: At, _listener: At) {
            self.sfx.push((kind, at));
        }
        fn bed(&mut self, bed: Bed, level: u8) {
            self.beds.push((bed, level));
        }
        fn tick(&mut self) {}
    }

    fn sense() -> Sense {
        Sense {
            seat: Seat(0),
            me: UnitId::new(1).unwrap(),
            pos: (Fx(100 * CELL_FX), Fx(100 * CELL_FX)),
            alive: true,
            zone: ZoneId::County,
            region: Region::Lowfields,
            indoor: false,
            night: false,
            abs: u64::from(17 * TICKS_PER_HOUR),
            weather: WeatherKind::Clear,
            surface: Some(Surface::Grass),
            hostile: false,
            bell_stopped: false,
            the_end: false,
            seed: 5,
            fire: 0,
            dog: None,
            dog_alarmed: false,
            might: (256, 256),
            building: None,
        }
    }

    fn none(_: PropId) -> Option<At> {
        None
    }

    #[test]
    fn a_fight_takes_the_music_after_two_seconds_and_gives_it_back_six_after() {
        let mut t = Soundtrack::new();
        let mut bus = Heard::default();
        let mut s = sense();
        t.step(&s, &[], &none, &mut bus);
        assert_eq!(t.cue(), Some(MusicCue::Zone(ZoneId::County, Region::Lowfields, false)));
        s.hostile = true;
        for _ in 0..COMBAT_AFTER - 1 {
            t.step(&s, &[], &none, &mut bus);
        }
        assert_ne!(t.cue(), Some(MusicCue::Combat), "not yet at 119 ticks");
        t.step(&s, &[], &none, &mut bus);
        assert_eq!(t.cue(), Some(MusicCue::Combat));
        s.hostile = false;
        for _ in 0..COMBAT_TAIL - 1 {
            t.step(&s, &[], &none, &mut bus);
        }
        assert_eq!(t.cue(), Some(MusicCue::Combat), "the tail holds");
        t.step(&s, &[], &none, &mut bus);
        assert_eq!(t.cue(), Some(MusicCue::Zone(ZoneId::County, Region::Lowfields, false)));
        // A skirmish shorter than two seconds never turns the music.
        let mut t = Soundtrack::new();
        s.hostile = true;
        for _ in 0..100 {
            t.step(&s, &[], &none, &mut bus);
        }
        s.hostile = false;
        for _ in 0..200 {
            t.step(&s, &[], &none, &mut bus);
        }
        s.hostile = true;
        for _ in 0..100 {
            t.step(&s, &[], &none, &mut bus);
        }
        assert_ne!(t.cue(), Some(MusicCue::Combat));
    }

    /// The School's door on the hill, where the clock rows hang the bell, far from `sense()`.
    const DOOR: (ZoneId, Vec2) = (ZoneId::County, Vec2 { x: Fx(400 * CELL_FX), y: Fx(40 * CELL_FX) });

    /// A bell the sim rang: a clock row's (to the whole party) or, with `zone`, a ringer's.
    fn bell(strikes: u8, at: Option<(ZoneId, Vec2)>, church: bool, zone: Option<ZoneId>) -> Event {
        Event { to: None, in_zone: zone, kind: EventKind::Bell { strikes, at, church } }
    }

    /// Steps `n` ticks from `s`, the first with `first`'s events, and counts the strikes of `kind`.
    fn strikes_of(t: &mut Soundtrack, s: &mut Sense, first: &[Event], n: u32, kind: SfxKind, bus: &mut Heard) -> usize {
        let mut count = 0;
        for i in 0..n {
            bus.sfx.clear();
            t.step(s, if i == 0 { first } else { &[] }, &none, bus);
            count += bus.sfx.iter().filter(|(k, _)| *k == kind).count();
            s.abs += 1;
        }
        count
    }

    #[test]
    fn the_bell_strikes_what_the_sim_rings_under_its_hush_then_the_night() {
        let mut t = Soundtrack::new();
        let mut bus = Heard::default();
        let mut s = sense();
        s.abs = u64::from(NINE) - 1;
        t.step(&s, &[], &none, &mut bus);
        s.abs += 1;
        s.night = true;
        // Nine o'clock with no event: no bell, and the music goes on.
        bus.sfx.clear();
        t.step(&s, &[], &none, &mut bus);
        assert!(bus.sfx.is_empty() && t.cue() != Some(MusicCue::Bell), "the clock alone rings nothing");
        let nine = [bell(9, Some(DOOR), false, None)];
        let mut strikes = 0;
        for i in 0..(9 * STRIKE_TICKS + BELL_TAIL + 10) {
            bus.sfx.clear();
            t.step(&s, if i == 0 { &nine } else { &[] }, &none, &mut bus);
            let here = bus.sfx.iter().filter(|(k, _)| *k == SfxKind::BellFar).count();
            if here > 0 {
                assert_eq!(i % STRIKE_TICKS, 0, "a strike every {STRIKE_TICKS} ticks");
            }
            strikes += here;
            if i == 5 {
                assert_eq!(t.cue(), Some(MusicCue::Bell));
            }
            s.abs += 1;
        }
        assert_eq!(strikes, 9, "the strikes the event says, heard across the county");
        assert_eq!(t.cue(), Some(MusicCue::Zone(ZoneId::County, Region::Lowfields, true)));
        // Six in the morning: six.
        let mut t = Soundtrack::new();
        let mut s = sense();
        s.abs = u64::from(6 * TICKS_PER_HOUR);
        let six = [bell(6, Some(DOOR), false, None)];
        assert_eq!(strikes_of(&mut t, &mut s, &six, 7 * STRIKE_TICKS, SfxKind::BellFar, &mut bus), 6);
        // Beside the School's door, the bell is where it hangs.
        let mut t = Soundtrack::new();
        let mut s = sense();
        s.pos = (Fx(DOOR.1.x.0 + 3 * CELL_FX), DOOR.1.y);
        t.step(&s, &six, &none, &mut bus);
        assert!(bus.sfx.contains(&(SfxKind::BellNear, (DOOR.1.x, DOOR.1.y))));
    }

    #[test]
    fn a_stopped_bell_is_a_silence_at_nine_and_the_church_is_its_own_small_bell() {
        let mut bus = Heard::default();
        let mut t = Soundtrack::new();
        let mut s = sense();
        s.bell_stopped = true;
        s.abs = u64::from(NINE) - 1;
        t.step(&s, &[], &none, &mut bus);
        s.abs += 1;
        t.step(&s, &[], &none, &mut bus);
        assert_eq!(t.cue(), Some(MusicCue::Silence), "nine o'clock, no bell");
        assert!(!bus.sfx.iter().any(|(k, _)| matches!(k, SfxKind::BellFar)));
        for _ in 0..NO_BELL {
            s.abs += 1;
            t.step(&s, &[], &none, &mut bus);
        }
        assert_ne!(t.cue(), Some(MusicCue::Silence), "the silence lasts {NO_BELL} ticks");
        // A sleep through nine wakes to no silence.
        let mut t = Soundtrack::new();
        s.abs = u64::from(20 * TICKS_PER_HOUR);
        t.step(&s, &[], &none, &mut bus);
        s.abs = u64::from(TICKS_PER_DAY + 6 * TICKS_PER_HOUR);
        t.step(&s, &[], &none, &mut bus);
        assert_ne!(t.cue(), Some(MusicCue::Silence));
        // Evensong: the church's five, quicker, over the town's music, not under a hush.
        let mut t = Soundtrack::new();
        let mut s = sense();
        let church_door = (ZoneId::County, Vec2 { x: Fx(300 * CELL_FX), y: Fx(300 * CELL_FX) });
        let five = [bell(5, Some(church_door), true, None)];
        let mut heard = 0;
        for i in 0..(5 * CHURCH_TICKS + 10) {
            bus.sfx.clear();
            t.step(&s, if i == 0 { &five } else { &[] }, &none, &mut bus);
            let here = bus.sfx.iter().filter(|(k, _)| *k == SfxKind::ChurchBell).count();
            if here > 0 {
                assert_eq!(i % CHURCH_TICKS, 0);
            }
            heard += here;
            assert_ne!(t.cue(), Some(MusicCue::Bell));
            s.abs += 1;
        }
        assert_eq!(heard, 5);
        assert!(!bus.sfx.iter().any(|(k, _)| matches!(k, SfxKind::BellFar | SfxKind::BellNear)));
        // Out of town (the Waters), the church is not heard.
        let mut t = Soundtrack::new();
        s.region = Region::Waters;
        assert_eq!(strikes_of(&mut t, &mut s, &five, 5 * CHURCH_TICKS, SfxKind::ChurchBell, &mut bus), 0);
    }

    #[test]
    fn the_ringer_strikes_where_he_stands_and_indoors_the_bell_is_heard_through_walls() {
        let mut bus = Heard::default();
        let mut t = Soundtrack::new();
        let mut s = sense();
        s.zone = ZoneId::School;
        s.indoor = true;
        let belfry = Vec2 { x: Fx(104 * CELL_FX), y: Fx(98 * CELL_FX) };
        let rope = [bell(1, Some((ZoneId::School, belfry)), false, Some(ZoneId::School))];
        t.step(&s, &rope, &none, &mut bus);
        assert!(bus.sfx.contains(&(SfxKind::BellNear, (belfry.x, belfry.y))));
        assert_ne!(t.cue(), Some(MusicCue::Bell), "one stroke in a fight does not hush it");
        // Far across the School, the same stroke comes through its walls.
        s.pos = (Fx(10 * CELL_FX), Fx(10 * CELL_FX));
        bus.sfx.clear();
        t.step(&s, &rope, &none, &mut bus);
        assert_eq!(bus.sfx, vec![(SfxKind::BellWithin, s.pos)]);
        // His zone's stroke is not heard in the county.
        let mut t = Soundtrack::new();
        let mut s = sense();
        t.step(&s, &[], &none, &mut bus);
        bus.sfx.clear();
        t.step(&s, &rope, &none, &mut bus);
        assert!(bus.sfx.is_empty());
        // A stroke of his during the hour's toll leaves the toll whole.
        let mut t = Soundtrack::new();
        s.zone = ZoneId::School;
        s.indoor = true;
        let both = [bell(9, Some(DOOR), false, None), rope[0]];
        let mut n = 0;
        for i in 0..(9 * STRIKE_TICKS + 5) {
            bus.sfx.clear();
            let ev: &[Event] = match i {
                0 => &both[..1],
                40 => &both[1..],
                _ => &[],
            };
            t.step(&s, ev, &none, &mut bus);
            n += bus.sfx.iter().filter(|(k, _)| matches!(k, SfxKind::BellWithin | SfxKind::BellNear)).count();
            s.abs += 1;
        }
        assert_eq!(n, 10, "nine for the hour and his one");
        // In the mine at nine the bell comes through the earth.
        let mut t = Soundtrack::new();
        let mut s = sense();
        s.zone = ZoneId::Mine;
        s.indoor = true;
        let nine = [bell(9, Some(DOOR), false, None)];
        assert_eq!(strikes_of(&mut t, &mut s, &nine, 2, SfxKind::BellWithin, &mut bus), 1);
        // A bell from nowhere in particular is heard across the county, or through the walls.
        let mut t = Soundtrack::new();
        let mut s = sense();
        let anywhere = [bell(3, None, false, None)];
        assert_eq!(strikes_of(&mut t, &mut s, &anywhere, 3 * STRIKE_TICKS, SfxKind::BellFar, &mut bus), 3);
    }

    #[test]
    fn a_fall_is_the_dead_cue_until_she_wakes() {
        let mut bus = Heard::default();
        let mut t = Soundtrack::new();
        let mut s = sense();
        t.step(&s, &[], &none, &mut bus);
        let died = Event { to: Some(Seat(0)), in_zone: None, kind: EventKind::PlayerDied };
        s.alive = false;
        t.step(&s, &[died], &none, &mut bus);
        assert_eq!(t.cue(), Some(MusicCue::Dead));
        assert!(bed_levels(&s).iter().all(|&l| l == 0), "the world goes quiet");
        s.alive = true;
        t.step(&s, &[], &none, &mut bus);
        assert_eq!(t.cue(), Some(MusicCue::Zone(ZoneId::County, Region::Lowfields, false)));
    }

    #[test]
    fn the_dog_barks_in_the_morning_pants_at_her_side_and_whines_at_night() {
        let mut bus = Heard::default();
        let mut t = Soundtrack::new();
        let mut s = sense();
        for _ in 0..3 {
            t.step(&s, &[], &none, &mut bus);
        }
        let step = (Fx(s.pos.0.0 + 5 * CELL_FX), s.pos.1);
        s.dog = Some(step);
        t.step(&s, &[], &none, &mut bus);
        assert!(bus.sfx.contains(&(SfxKind::DogBark, step)), "out on the step: a bark");
        s.dog = Some((Fx(s.pos.0.0 + CELL_FX), s.pos.1));
        t.step(&s, &[], &none, &mut bus);
        assert!(bus.sfx.iter().any(|(k, _)| *k == SfxKind::DogPant));
        let pants = |b: &Heard| b.sfx.iter().filter(|(k, _)| *k == SfxKind::DogPant).count();
        s.dog = Some(step);
        t.step(&s, &[], &none, &mut bus);
        s.dog = Some((Fx(s.pos.0.0 + CELL_FX), s.pos.1));
        t.step(&s, &[], &none, &mut bus);
        assert_eq!(pants(&bus), 1, "not again so soon");
        s.dog = None;
        t.step(&s, &[], &none, &mut bus);
        assert!(bus.sfx.iter().any(|(k, _)| *k == SfxKind::DogWhine), "gone for the night");
    }

    #[test]
    fn sounds_fade_over_twenty_cells_and_pan_to_their_side() {
        let me = (Fx(0), Fx(0));
        let here = place(me, me).unwrap();
        assert_eq!((here.gain, here.pan), (1.0, 0.0));
        let right = place((Fx(6 * CELL_FX), Fx(0)), me).unwrap();
        assert!(right.pan > 0.3 && right.gain < 1.0);
        let left = place((Fx(-6 * CELL_FX), Fx(0)), me).unwrap();
        assert!((left.pan + right.pan).abs() < 1e-6);
        let far = place((Fx(19 * CELL_FX), Fx(0)), me).unwrap();
        assert!(far.gain < 0.01 && far.send > here.send);
        assert!(place((Fx(20 * CELL_FX), Fx(0)), me).is_none());
    }

    #[test]
    fn footsteps_fall_in_time_with_the_walk_on_the_ground_under_her() {
        let mut bus = Heard::default();
        let mut t = Soundtrack::new();
        let mut s = sense();
        s.surface = Some(Surface::Wood);
        // Stand, then walk 90 ticks (a step as she sets off, then one every 18), then stop.
        t.step(&s, &[], &none, &mut bus);
        for _ in 0..90 {
            s.pos.0 = Fx(s.pos.0.0 + CELL_FX / 8);
            t.step(&s, &[], &none, &mut bus);
        }
        for _ in 0..30 {
            t.step(&s, &[], &none, &mut bus);
        }
        let steps = bus.sfx.iter().filter(|(k, _)| *k == SfxKind::StepWood).count();
        assert_eq!(steps, 5, "90 ticks of walking");
        assert_eq!(Surface::of(Tile::Boardwalk), Some(Surface::Wood));
        assert_eq!(Surface::of(Tile::Cobble), Some(Surface::Cobble));
        assert_eq!(Surface::of(Tile::Grass).map(|g| g.wet(200)), Some(Surface::Water));
        assert_eq!(Surface::of(Tile::HouseWall), None);
    }

    #[test]
    fn the_beds_follow_the_sky_the_hour_and_the_place() {
        let mut s = sense();
        s.abs = u64::from(19 * TICKS_PER_HOUR);
        let dusk = bed_levels(&s);
        assert!(dusk[Bed::Birds.index()] > 150, "birds at dusk");
        assert_eq!(dusk[Bed::Crickets.index()], 0);
        s.abs = u64::from(23 * TICKS_PER_HOUR);
        s.night = true;
        let night = bed_levels(&s);
        assert_eq!(night[Bed::Birds.index()], 0);
        assert!(night[Bed::Crickets.index()] > 100, "crickets at night");
        s.weather = WeatherKind::Storm;
        let storm = bed_levels(&s);
        assert_eq!(storm[Bed::Rain.index()], 255);
        assert!(storm[Bed::Wind.index()] > 200);
        assert_eq!(storm[Bed::Crickets.index()], 0);
        s.zone = ZoneId::Arms;
        s.indoor = true;
        let inn = bed_levels(&s);
        assert_eq!(inn[Bed::Rain.index()], 0);
        assert!(inn[Bed::RainRoof.index()] > 0 && inn[Bed::Fire.index()] > 0);
        s.zone = ZoneId::Mine;
        assert_eq!(bed_levels(&s)[Bed::RainRoof.index()], 0, "no rain under the ground");
        s.zone = ZoneId::County;
        s.indoor = false;
        s.region = Region::Waters;
        s.weather = WeatherKind::Clear;
        assert!(bed_levels(&s)[Bed::Lake.index()] > 0);
        s.region = Region::Works;
        assert!(bed_levels(&s)[Bed::Hum.index()] > 0);
        // A campfire crackles as she comes to it.
        s.fire = 220;
        assert_eq!(bed_levels(&s)[Bed::Fire.index()], 220);
        assert!(is_fire("campfire") && is_fire("museum_stove") && is_fire("brazier"));
        assert!(!is_fire("scroll_fire") && !is_fire("lamp_post"));
    }

    #[test]
    fn every_zone_has_a_song_and_the_county_changes_at_night() {
        for cue in MusicCue::all() {
            if cue != MusicCue::Silence {
                assert!(cue.song().is_some(), "{cue:?}");
            }
        }
        assert_ne!(
            MusicCue::zone(ZoneId::County, Region::Waters, false).song(),
            MusicCue::zone(ZoneId::County, Region::Waters, true).song()
        );
        assert_eq!(
            MusicCue::zone(ZoneId::Mine, Region::Works, true),
            MusicCue::zone(ZoneId::Mine, Region::Lowfields, false)
        );
        assert_eq!(fades(Some(MusicCue::Combat), MusicCue::Zone(ZoneId::County, Region::Works, false)), (2500, 2500));
    }
}
