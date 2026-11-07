//! `Present` (PRESENTATION.md §1.11): `tick()` once per accumulated tick, whether or not the sim
//! ran; `draw(alpha)` pure in the tick and `alpha`.
//!
//! `tick` reads the `View` and this tick's events into the presenter's own records (units with
//! their last two positions and timers, props near the view, the lights the view says are lit
//! in reach of it, the camera, the chunks under it, the sky); `draw` turns those records into a
//! `Frame` at `alpha` and reads nothing else.
//! Neither reads `GameState`, and neither writes anything but the presenter.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::action::{CameraMode, Facing, School};
use jane_core::ids::SpellId;
use jane_core::num::CELL_SHIFT;
use jane_core::{Rect, ZoneId};
use jane_data::{Controller, Faction, Region};
use jane_sim::event::{Event, EventKind, events_for};
use jane_sim::ids::PropIx;
use jane_sim::view::{QuestMark, View};

use crate::ambient::{self, Ambient};
use crate::atlas::{Atlas, RefId};
use crate::atmos::Atmosphere;
use crate::backend::AtlasPages;
use crate::camera::{Camera, alpha_256};
use crate::chunks::{ChunkCache, LRU, Need, RESERVE};
use crate::creatures::{self, Creatures};
use crate::cues::Cues;
use crate::drawlist::{DrawCmd, DrawList};
use crate::facing::Face8;
use crate::frame::{
    Bend, Block, CANVAS_H, CANVAS_W, CAST_MARGIN, CELL, CHUNK_PX, Caster, ChunkCmd, ChunkId, Depth, Directional,
    FX_TO_CANVAS, Features, Flags, Foot, Frame, Light, LightKind, Margins, Pass, Post, Rgb, Span, SpriteCmd, Tier,
    Tint, height_of_rows, rows_up,
};
use crate::fx::Fx;
use crate::lesson::{Her, Lessons};
use crate::light::{Sky, flicker, lantern_lit, sky};
use crate::people::{self, People};
use crate::props::{self, Props};
use crate::shadow;
use crate::stand_in::{self, StandIns, UnitKind};
use crate::terrain::Terrain;
use crate::ui::marks::{EMOTE_TICKS, Emote, EmoteMarker};

mod boot;
mod draw;
mod lights;
mod paint;
mod read_props;
mod tick;

#[cfg(test)]
mod tests;

/// A unit moving further than this in a tick (40 sim px, `Fx`) snaps instead of sliding: travel,
/// a respawn, a hop.
const SNAP_FX: i64 = 40 * 256;
/// Px past the casting band whose units and props are kept: a tall sprite standing below the
/// band that still reaches into it.
const KEEP_PAST: i32 = 96;
/// Canvas px past the casting band painted ahead, so a frame between two ticks never finds a
/// hole, and the casting band's ground and the lights just past it stand on painted chunks.
const CHUNK_AHEAD: i32 = 32;
/// Canvas px further ahead on each side the view is moving toward (PLAY-PLAN.md §7): a chunk.
const MOVE_AHEAD: i32 = CHUNK_PX;
/// Canvas px a tick the view moves at the least to count as a travel's catching up, not a walk
/// (a sprint is a few px a tick): a console paints a view of swatches at once only then.
const FAST_VIEW: i32 = 24;
/// Chunks the terrain painter lands in a tick at most (§1.6): the rest show their swatches, or
/// what they last had, until it reaches them. The tick a zone is entered paints all it shows,
/// except on a console anywhere but the county: that first view is left to the painter.
const LAND_PER_TICK: usize = 2;
/// A console's stand-in with this many cells (of 256) left flat colour, no sample of their tile
/// yet, reads as blocks of colour (`Present::chunks_mush`).
const MUSH_CELLS: u16 = 32;

/// The most chunks a console's loader is asked to land at once (`Present::baked_wants`), and to
/// read ahead.
const BAKED_WANTS: usize = 12;
const BAKED_AHEAD: usize = 6;
/// A move in a tick past this (`Fx`: 120 sim px, half a console's screen) snaps a console's
/// camera to her.
const CAMERA_SNAP_FX: i32 = 120 * 256;
/// What a console's draw lists reserve up front (`from_tables_console`).
const CONSOLE_RESERVE: usize = 1024;
/// The most answers `against_walls` keeps before it starts again (moving lights add one a tick).
const WALLS_KEPT: usize = 512;
/// Canvas px past the casting band the draw list sorts over; things further out are culled: the
/// tallest thing standing below it.
const SORT_PAST: i32 = 128;
/// Ticks a hurt unit shows it, and the first ticks of them it flashes (§1.11).
const HURT_TICKS: u32 = 8;
const FLASH_TICKS: u32 = 4;
/// Cells past the view whose lights are read: 208 canvas px, the light reach (§1.7).
const LIGHT_CELLS: i32 = 13;
/// Her lantern (§1.7): its reach, canvas px, and its warm colour.
const LANTERN_RADIUS: u16 = 136;
const LANTERN: Rgb = [255, 190, 116];
/// How far a lit window's light reaches, canvas px: a small pool at the foot of its wall.
const WINDOW_RADIUS: u16 = 64;
/// Its spill's half-angle, round from straight out of the wall.
const WINDOW_CONE: jane_core::Angle = jane_core::Angle((80 * 65536 / 360) as u16);
/// The lowest her lantern shines from, px: hung at her knee it would only graze the ground.
const LANTERN_LOW: i32 = 18;
/// The most a unit's lights are raised by what it stands on, px (§1.7).
const MAX_LIFT: i32 = 48;
/// The lowest a standing prop light shines from, px: a low flame lights the ground round it.
const FLAME_HEIGHT: i32 = 28;
/// How many steps a second a flame's flicker walks (§1.7).
const FLICKER_RATE: u32 = 10;
/// A unit's draw key: its id with the top bit set (a prop's is its id).
const UNIT_KEY: u32 = 0x8000_0000;
/// A drop's key in the draw list and the prop list: its id with this bit, above every prop's.
const DROP_KEY: u32 = 0x2000_0000;
/// A rug's key in the prop list (ART-PLAN M4): the id of the thing it is laid by, with this bit.
const RUG_KEY: u32 = 0x0800_0000;
/// The Hoar Stone's key in the prop list (`cues`): above every prop's and every drop's.
const STONE_KEY: u32 = 0x4000_0000;

/// Which lights a frame draws and which of them cast (PRESENTATION.md §1.7), one rule for every
/// tier: of the lights that may cast (`Light::casts` as they come: prop lights, her lantern, a
/// bolt's head), the `casting` nearest the middle of the canvas (`mid`) cast and the rest do not;
/// then the lights are kept to `most`, the casting ones first and the rest nearest first. Ties
/// keep the order they came in. So a tier that casts from 4 casts from the first 4 of the 8 a
/// tier of 8 casts from, and those of the 32, whatever else is lit.
pub fn pick_lights(lights: &mut Vec<Light>, mid: (i32, i32), most: usize, casting: usize) {
    let d2 = |l: &Light| {
        let (dx, dy) = (i64::from(l.pos.0 - mid.0), i64::from(l.pos.1 - mid.1));
        dx * dx + dy * dy
    };
    lights.sort_by_key(|l| d2(l));
    let mut left = casting;
    for l in lights.iter_mut() {
        if l.casts {
            l.casts = left > 0;
            left = left.saturating_sub(1);
        }
    }
    if lights.len() > most {
        lights.sort_by_key(|l| (!l.casts, d2(l)));
        lights.truncate(most);
    }
}

/// Lights on screen at most, by tier (§1.3 `max_lights`); how many of them cast is the
/// `shadows` row (`Features::shadows`: T0 4, T1 8, T2 32).
pub fn max_lights(tier: Tier) -> usize {
    match tier {
        Tier::T0 => 16,
        Tier::T1 => 32,
        Tier::T2 => 128,
    }
}

/// Rows between a head and the quest mark over it (§3.8).
const MARK_CLEAR: i32 = 0;
/// A thing's quest mark keeps its own key apart from a person's (its bob's phase).
pub const PROP_MARK_KEY: u32 = 0x1000_0000;

/// A unit as the presenter keeps it between ticks.
#[derive(Clone, Copy, Debug)]
struct UnitRec {
    id: u32,
    /// Feet, `Fx`: last tick's and this tick's.
    prev: (i32, i32),
    cur: (i32, i32),
    /// Ticks walked without stopping (the walk frame).
    anim: u32,
    hurt_until: u32,
    look: RefId,
    mirror: bool,
    dead: bool,
    /// Hers (this seat's own body).
    me: bool,
    /// A player's unit, hers or another seat's: it carries a lantern at night.
    player: bool,
    /// Seen through whatever stands in front of it (a player, or a threat): `Tint::Seen`.
    seen: bool,
    /// Its glow: radius canvas px and colour.
    glow: Option<(u16, Rgb)>,
    /// The unit's person look in [`People`], when its sprite has one; else `look` stands in.
    person: Option<u16>,
    /// The unit's creature look in [`Creatures`], when its sprite has one.
    creature: Option<u16>,
    /// Ticks stood still (a creature sits, grazes or pecks after a while), and the tick it last
    /// struck (its attack's three beats).
    still: u32,
    struck: Option<(u32, SpellId)>,
    /// A blow it is winding up (`jane_sim::feel`): it holds its raise, the first beat of the
    /// attack or the cast, until the blow lands.
    raise: Option<SpellId>,
    /// Which of the eight ways it shows itself facing: the diagonal it walks on, kept when it
    /// stops while the sim's facing agrees ([`Face8`]).
    face: Face8,
    /// It stands on the ground under what the terrain draws over it (a house's eaves): what is
    /// drawn at its feet is the roof in front of it, not a step under it, so it lifts no light.
    under: bool,
    /// The quest mark over its head for this seat (`View::quest_mark`), none while she is
    /// talking to it.
    mark: Option<QuestMark>,
    /// The tick it last turned to a new facing, and the tick it last landed (came into the
    /// zone, or pulled up out of a sprint): the people's turn and landing frames (ART-PLAN B3).
    turned: Option<(u32, Face8)>,
    landed: Option<u32>,
    /// Its work, while its schedule row is the one the work belongs to (ART-PLAN Q4).
    task: Option<jane_data::Task>,
    /// A seat's cast building (`View::casting`): how far along in 256ths, and its school.
    build: Option<(u16, School, SpellId)>,
    /// Half the width of the ring under it when it is her target, canvas px.
    ring: i32,
    /// How high the ground it stands on is, px (MAP.md §6.2: its level's, or its deck's).
    base: u8,
    /// On a deck (MAP.md §2.5): drawn over the decks, after everything on the ground.
    deck: bool,
    /// Carried over a ledge (`View::hopping`): its arc, from the presenter's tick it began.
    hop: bool,
    hop_at: u32,
}

/// The draw keys of a deck's pieces and of what stands on it (MAP.md §2.5): after anything else
/// sorted at the deck's south edge's row, the pieces first.
const DECK_KEY: u32 = 0xf000_0000;

/// How high a hop's arc rises over the ledge, canvas px (MAP.md §2.4).
const HOP_ARC: i32 = 10;

/// What is drawn over the world for her side of a fight this frame (PLAY-PLAN §2.1,
/// PRESENTATION.md §3.9): the ring under her target, every cast bar, where a click sent her.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FightOverlay {
    pub target: Option<TargetMarker>,
    pub bars: Vec<CastBar>,
    /// Where a click on open ground sent her, canvas px, and the presenter's tick.
    pub walk: Option<(i32, i32)>,
    pub tick: u32,
}

/// Her target this frame: where it stands on the canvas and what the ring says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetMarker {
    /// Its feet (a prop's front edge, middle), canvas px.
    pub x: i32,
    pub y: i32,
    /// Half the ring's width, canvas px.
    pub half: i32,
    /// The top of what it stands (its head), canvas px: the name and the bar go over it.
    pub head: i32,
    /// A foe (a red ring), else a prop (gold).
    pub hostile: bool,
    /// A foe's name; empty for a prop.
    pub name: &'static str,
    /// A foe's health, points: now and full.
    pub hp: Option<(i32, i32)>,
}

/// A slim bar under a seat's body while her cast builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastBar {
    /// Her feet, canvas px.
    pub x: i32,
    pub y: i32,
    /// 0 to 256.
    pub frac: u16,
    pub school: School,
}

/// Her target as the view had it this tick.
#[derive(Clone, Copy, Debug)]
struct TargetSeen {
    to: jane_sim::input::TargetRef,
    hostile: bool,
    name: &'static str,
    hp: Option<(i32, i32)>,
}

/// A quest mark to draw over a person's head, or a thing's top, this frame (§3.8): where its foot is on the canvas
/// (the glyph stands on it), and which mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestMarker {
    /// The unit's id, or a thing's id with [`PROP_MARK_KEY`]: its bob's phase.
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub mark: QuestMark,
}

/// How high a carried thing is held, canvas px from her feet to its foot: over her head, in
/// both hands (the owner's playtest, 2026-10-07: a rock lifted looked never picked up).
const CARRY_HELD: i32 = 20;
/// Ticks a lift or a set-down takes to draw: the thing rises to her head, or comes down to the
/// ground where the sim put it. The sim has it already; this is the look only.
const CARRY_BEAT: u32 = 10;

/// A carried thing on its way up or down: where on the ground it lies (its footprint's top-left,
/// canvas px), and the carrier's feet at the moment it was put down.
#[derive(Clone, Copy, Debug)]
struct CarryBeat {
    prop: jane_sim::PropId,
    start: u32,
    up: bool,
    ground: (i32, i32),
    feet: (i32, i32),
}

/// A carried thing's footprint (top-left) and lift, `t` of 256 of the way from lying at `ground`
/// to held over feet at `feet`.
fn carry_at(ground: (i32, i32), feet: (i32, i32), (w, h): (i32, i32), t: i32) -> (i32, i32, i32) {
    let held = (feet.0 - w / 2, feet.1 - h + 1);
    let lerp = |a: i32, b: i32| a + (b - a) * t / 256;
    (lerp(ground.0, held.0), lerp(ground.1, held.1), CARRY_HELD * t / 256)
}

/// A prop near the view, this tick.
#[derive(Clone, Copy, Debug)]
struct PropRec {
    id: u32,
    /// The footprint in canvas px, in the zone.
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    look: RefId,
    flat: bool,
    /// Set into a wall's face (a door, a lamp on its bracket, a hanging): drawn over the face and
    /// standing on the face's foot, so it throws no shadow of its own (the wall throws it).
    flush: bool,
    /// Stands on what the terrain raises (a chimney on a roof, a torch on a wall): drawn over it,
    /// never behind it.
    on_top: bool,
    /// A top things stand on (a table): `(front, depth)` rows (`Props::surface`).
    surface: Option<(i32, i32)>,
    /// Stands on another prop's top (Julie's fruit bowl on her table): drawn that many rows up,
    /// sorted just after the table's foot, and it throws no shadow of its own.
    lift: i32,
    /// The foot it sorts by, when it stands on a top: the top's.
    sort_foot: Option<i32>,
    /// Held by this unit (its id): drawn where the unit is drawn at the frame's alpha.
    follows: Option<u32>,
    /// Its quest mark for this seat (a book, a board, a door that gives quests), unless she is
    /// reading it.
    mark: Option<QuestMark>,
    /// A chimney on a house someone lives in: it smokes (ART §2.8, ART-PLAN M1).
    /// A lamp kept tonight wears Julie's ring of chalk round its post (NIGHT.md §5.2, the
    /// night kit's `Ring`, from N2).
    ring: bool,
    smokes: bool,
}

/// What a prop's records were built from (`Present::read_props`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PropKey {
    cell: (u16, u16),
    flags: u8,
    loot: Option<(u16, Option<jane_core::ids::ItemId>)>,
    stages: (u8, u8),
}

/// The records a prop yields: its rug under it, if any, and itself (`None`: it is not drawn).
#[derive(Clone, Copy, Debug)]
struct Built {
    rug: Option<PropRec>,
    rec: Option<PropRec>,
}

impl Built {
    const SKIP: Built = Built { rug: None, rec: None };
}

#[derive(Clone, Copy, Debug)]
struct PropEntry {
    key: PropKey,
    built: Built,
}

/// A prop light the view says is showing, this tick.
#[derive(Clone, Copy, Debug)]
struct LightRec {
    id: u32,
    /// Its ground point in the zone, canvas px.
    x: i32,
    y: i32,
    height: u8,
    colour: Rgb,
    radius: u16,
    size: u8,
    /// Permille it dips as it flickers.
    dip: i16,
    /// Kept through the turn's gutter (NIGHT.md §2.2): a fire, or a lamp where the shield holds
    /// (a hub's, Julie's yard's). The rest gutter.
    kept: bool,
    /// A lamp gone wrong (NIGHT.md §4.5): cold, and no moths at it.
    wrong: bool,
}

/// The presenter: its own state, never the sim's.
#[derive(Debug)]
pub struct Present {
    atlas: Atlas,
    /// Where the UI's glyphs, marks and icons are in the atlas.
    ui_art: crate::ui::UiArt,
    stand: StandIns,
    people: People,
    creatures: Creatures,
    /// The prop kit's looks.
    kit: Props,
    frame: Frame,
    tick: u32,
    /// What each body carried last tick: `(prop, unit)` (the sim's `Unit::carrying`).
    carrying: Vec<(jane_sim::ids::PropId, u32)>,
    /// Rocks being lifted or set down this moment ([`CarryBeat`]).
    carry_beats: Vec<CarryBeat>,
    /// Each sway class's phase, of 2^32, advanced a tick at a time at the pace of the wind then:
    /// a change in the wind changes how fast the plants sway, never where they are in it.
    sway: [u32; 4],
    /// The tick `sway` was last advanced to.
    sway_at: u32,
    /// The canvas the last frame was drawn for; the camera frames for it.
    canvas: (u16, u16),
    camera: Camera,
    zone: Option<(ZoneId, u32)>,
    zone_cells: (u32, u32),
    chunks: ChunkCache,
    terrain: Terrain,
    /// This tick's chunks to paint, by priority (scratch).
    wants: Vec<(u32, ChunkId, Need)>,
    /// The chunks of this tick's paint band and their priority (scratch; a console keeps the
    /// first of them its slots hold).
    band: Vec<(u32, ChunkId)>,
    /// A zone was entered this tick.
    entered: bool,
    units: Vec<UnitRec>,
    units_next: Vec<UnitRec>,
    /// Each serpent's trail this tick, `Fx`, newest first (its body is drawn along it).
    trails: Vec<(u32, Vec<(i32, i32)>)>,
    hurt: Vec<u32>,
    /// Units that struck (cast) this tick.
    struck: Vec<(u32, SpellId)>,
    props: Vec<PropRec>,
    prop_scratch: Vec<PropIx>,
    standing: DrawList,
    /// The sprites seen through what stands in front of them (`Tint::Seen`), this frame.
    seen: Vec<SpriteCmd>,
    ground: DrawList,
    lights: Vec<LightRec>,
    light_scratch: Vec<PropIx>,
    /// This frame's standing casters by draw key, `(key, sprite)`, sorted: what holds a light.
    holders: Vec<(u32, u32)>,
    sky: Sky,
    /// The casting band round the canvas for the sky's sun (`shadow::cast_margins`).
    margins: Margins,
    /// How far the boss's fight has shifted the dungeon's grade, 0 to 256 (ART-PLAN B2).
    boss_grade: u32,
    /// The night's turn under way (NIGHT.md §2.2, `crate::turn`): its kind and the ticks since
    /// it began, from the sim's latched night (`tick - turned_at`); where it is seen.
    turn: Option<(crate::turn::Kind, u32)>,
    room: crate::turn::Room,
    /// The latched stage, and the last night's (dawn's turn takes its grade out).
    stage: u8,
    last_stage: u8,
    /// The night's intensity where she stands, 256ths, eased as she crosses from a hub to the
    /// fields (NIGHT.md §3.2): the grade's stage term.
    night_q: u32,
    /// The weather, the fog and the sky (§1.9), and the effects (§2).
    atmos: Atmosphere,
    fx: Fx,
    /// The moment a spell is learned, or a jar or a page found (§2.1).
    lessons: Lessons,
    /// This frame's quest marks over heads (§3.8), and whether it is dark enough for them to glow.
    marks: Vec<QuestMarker>,
    dark: bool,
    /// Her target as the view said this tick, and where a click on open ground sent her (zone
    /// canvas px).
    aimed: Option<TargetSeen>,
    walk_to: Option<(i32, i32)>,
    /// This frame's ring, cast bars and walk marker.
    fight: FightOverlay,
    /// Cues past the screen edge: crows over camps, the dead lamps' glass, the Hoar Stone.
    cues: Cues,
    /// The hour of the clock, as last read.
    hour: u8,
    /// The ambient life: birds, ducks, crows, a cat, butterflies, moths, smoke, the water's life
    /// (ART-PLAN M1, B4).
    ambient: Ambient,
    /// The night kit in the atlas, and how many overlays a console draws at most (its preset's,
    /// `night::console_cap`; a PC lays them into its chunks).
    night_art: crate::night::NightArt,
    /// The decks' pieces (MAP.md §2.5), the zone's spans as they are drawn, and each frame's
    /// pieces laid (scratch).
    deck_art: crate::decks::DeckArt,
    deck_spans: Vec<crate::decks::DeckSpan>,
    deck_laid: Vec<(jane_art::deck::Laid, jane_art::deck::Kind, u8, i32)>,
    deck_scratch: Vec<jane_art::deck::Laid>,
    night_cap: u16,
    night_near: Vec<(u32, SpriteCmd)>,
    /// The houses someone lives in (a schedule names a door of theirs), by their block's corner:
    /// their chimneys smoke.
    lived: Vec<(i32, i32)>,
    /// Emotes over heads (ART-PLAN B3): `(unit, emote, tick it popped)`.
    emotes: Vec<(u32, Emote, u32)>,
    /// This frame's emotes, where each head is.
    emote_marks: Vec<EmoteMarker>,
    /// Whom she was talking to last tick, and the line: a new line may bring an emote.
    talk: Option<(u32, u16)>,
    /// What `against_walls` found for each light, `(id, ground point as read, where it stands)`,
    /// good while the chunks' layers are as they were when it looked (`walls_at`, the paint
    /// count; cleared on a zone change): the same answers without the search each tick.
    walls: Vec<(u32, i32, i32, i32, i32)>,
    walls_at: u32,
    /// Chunks painted away from the presenter (`take_paint_job`, `land`): a console's worker.
    deferred: bool,
    /// Chunks on screen still swatches or nothing at the last tick (a console's deferred paint).
    swatched: usize,
    /// Of them, those whose stand-in is mostly flat colour ([`MUSH_CELLS`] or more cells with no
    /// sample of their tile): what a player sees as blocks of colour.
    mush: usize,
    /// A console's: cells of the zone's ground the painter has painted, which its stand-ins draw
    /// with (`stand_in::Samples`); the chunks landed since the last tick, to take them from; and
    /// each slot's stand-in's flat cells (0 once it is painted properly).
    samples: Option<stand_in::Samples>,
    fresh: Vec<ChunkId>,
    flat_cells: Vec<u16>,
    /// Each prop's records by prop id, sorted; dropped on a zone change or a tile change.
    prop_cache: Vec<(u32, PropEntry)>,
    /// Whether the cache is on (a test turns it off to compare).
    pub prop_cache_on: bool,
    /// Per slot: its stand-in is the cheap flat fill because this tick's one sampled stand-in
    /// went to a chunk nearer the middle; a later tick upgrades it (a console).
    capped: Vec<bool>,
    /// Sampled stand-ins painted in the last tick (a console paints at most one).
    pub sampled_this_tick: u32,
    /// Most sampled stand-ins painted in any one tick.
    pub sampled_max_tick: u32,
    /// Flat stand-ins upgraded to sampled ones (not counted as swatches).
    pub stand_in_upgrades: u32,
    /// Stand-ins painted for chunks not yet on screen (a test's count).
    pub stand_ins_ahead: u32,
    /// A console's county kept while she is elsewhere (`keep_county`): the game's seed and the
    /// county's ground digest it was kept at (`View::ground_digest`), and its samples.
    county_kept: Option<(u32, u64)>,
    county_samples: Option<stand_in::Samples>,
    /// The kept chunks' night overlays are to be laid again.
    renight: bool,
    /// Where she stood last tick (`Fx`): a console's camera snaps after a far move.
    her_was: Option<(i32, i32)>,
    /// Ticks a console's view of swatches was painted at once (a leap): a capture's event.
    pub view_jumps: u32,
    /// The chunk the next job paints, the one a job is painting now, and whether the tiles
    /// under it changed since it was snapshot (it is painted again then).
    job_next: Option<ChunkId>,
    job_out: Option<(ChunkId, (ZoneId, u32))>,
    job_stale: bool,
    /// A clock (microseconds) a console times `tick`'s parts by, and what it measured, summed
    /// until read: the units, the emotes, props and lights, the chunks to paint, the walls, the
    /// sky and the atmosphere, the effects, the ambient life, the rest. Never read on PC.
    pub clock: Option<fn() -> u32>,
    pub prof: [u32; 12],
    /// Things' quest marks as last asked, `(prop, mark)` by prop (a console's, `read_props`),
    /// the next tick's being gathered, and whether something happened that can change one.
    prop_marks: Vec<(u32, Option<QuestMark>)>,
    marks_next: Vec<(u32, Option<QuestMark>)>,
    marks_stale: bool,
    /// The same for the units in view (`read_units`): `(unit, mark)` by unit.
    unit_marks: Vec<(u32, Option<QuestMark>)>,
    unit_marks_next: Vec<(u32, Option<QuestMark>)>,
    /// A console's baked county (PLAN-PSP-SPEED.md §5, P1c): its chunks are landed from the pack
    /// by [`land_baked`](Self::land_baked), the painter keeps the rest (`set_baked`).
    baked: Option<crate::chunk_pack::Ground>,
    /// Chunks landed from the pack.
    pub baked_landed: u32,
    /// Stand-ins painted for chunks on screen (a console's: what the bake is there to end).
    pub stand_ins_shown: u32,
}

/// A console asks each one's quest mark (a person's, a thing's) again this often, ticks (the PC
/// every tick): about half a second. Anything that can change a mark asks them all at once.
const MARKS_EVERY: u32 = 32;

/// Whether `k` can change what a thing's quest mark says (a quest, the bag, a talk, a thing's
/// state, who is in the zone): a console's kept marks are asked again at once.
fn moves_marks(k: &EventKind) -> bool {
    matches!(
        k,
        EventKind::Quest { .. }
            | EventKind::Bag
            | EventKind::Store { .. }
            | EventKind::Bench { .. }
            | EventKind::Dialogue
            | EventKind::Loot { .. }
            | EventKind::Learn(_)
            | EventKind::Rest
            | EventKind::Rested { .. }
            | EventKind::Journal(_)
            | EventKind::Consequence(_)
            | EventKind::Prop { .. }
            | EventKind::Zone { .. }
            | EventKind::Party { .. }
            | EventKind::Spawned { .. }
            | EventKind::Despawned { .. }
            | EventKind::Death { .. }
            | EventKind::Respawn { .. }
            | EventKind::PlayerDied
            | EventKind::Toast(_)
            | EventKind::Tiles(_)
    )
}

/// Whether lamp `p`, its light `l` showing, has gone wrong tonight (NIGHT.md §4.5, §5.4): R1's
/// rule (`jane_sim::light::warm`), read from the blueprint's shield ranks against the latched
/// stage, once the night's look is in (`look`: the stage the chunks are painted at). A light the
/// row marks `cold` (the Burial's torches) is cold by its nature, not wrong; a fire never is.
fn wrong_lamp(view: &View<'_>, p: &jane_sim::state::Prop, l: &jane_data::Light, look: u8) -> bool {
    if look == 0 || l.cold || l.flicker.0 >= 200 || view.zone() != ZoneId::County {
        return false;
    }
    let stage = view.night().stage;
    let shield = &view.blueprints().get(view.zone()).shield;
    p.spawn.and_then(|row| shield.iter().find(|e| e.0 == row).map(|e| e.1)).is_some_and(|from| stage >= from)
}

fn rgb(c: u32) -> Rgb {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

/// A colour at `k` of 255.
fn scale(c: Rgb, k: u8) -> Rgb {
    c.map(|v| ((u32::from(v) * (u32::from(k) + 1)) >> 8) as u8)
}

fn clamp16(x: i32, y: i32) -> (i16, i16) {
    (x.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16, y.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
}

fn sprite(r: &crate::atlas::SpriteRef, x: i32, y: i32, flags: Flags) -> SpriteCmd {
    SpriteCmd {
        page: r.page,
        src: r.src,
        x: x.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
        y: y.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
        flags,
        height_px: r.height,
        foot: None,
    }
}
