//! `Present` (PRESENTATION.md §1.11): `tick()` once per accumulated tick, whether or not the sim
//! ran; `draw(alpha)` pure in the tick and `alpha`.
//!
//! `tick` reads the `View` and this tick's events into the presenter's own records (units with
//! their last two positions and timers, props near the view, the lights the view says are lit
//! in reach of it, the camera, the chunks under it, the sky); `draw` turns those records into a
//! `Frame` at `alpha` and reads nothing else.
//! Neither reads `GameState`, and neither writes anything but the presenter.

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
/// Chunks the terrain painter lands in a tick at most (§1.6): the rest show their swatches, or
/// what they last had, until it reaches them. The tick a zone is entered paints all it shows.
const LAND_PER_TICK: usize = 2;
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
}

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
    /// Its quest mark for this seat (a book, a board, a door that gives quests), unless she is
    /// reading it.
    mark: Option<QuestMark>,
    /// A chimney on a house someone lives in: it smokes (ART §2.8, ART-PLAN M1).
    smokes: bool,
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
    /// The houses someone lives in (a schedule names a door of theirs), by their block's corner:
    /// their chimneys smoke.
    lived: Vec<(i32, i32)>,
    /// Emotes over heads (ART-PLAN B3): `(unit, emote, tick it popped)`.
    emotes: Vec<(u32, Emote, u32)>,
    /// This frame's emotes, where each head is.
    emote_marks: Vec<EmoteMarker>,
    /// Whom she was talking to last tick, and the line: a new line may bring an emote.
    talk: Option<(u32, u16)>,
}

impl Present {
    /// A presenter at `tier`, its atlas built.
    pub fn new(tier: Tier) -> Present {
        let mut atlas = Atlas::with_layers(tier > Tier::T0);
        let stand = StandIns::build(&mut atlas);
        let people = People::build(&mut atlas);
        let creatures = Creatures::build(&mut atlas);
        let kit = Props::build(&mut atlas);
        let terrain = Terrain::build(&mut atlas, LRU);
        let atmos = Atmosphere::new(tier, &mut atlas);
        let fx = Fx::new(tier, atmos.features.max_particles);
        let cues = Cues::new(&mut atlas);
        let ambient = Ambient::build(tier, &mut atlas, &creatures);
        // The UI's page goes last, so no world sprite moves when it grows (PRESENTATION.md §3.1).
        let (ui_art, mut ui_page) = crate::ui::UiArt::build(atlas.pages.pages.len() as u8);
        if atlas.lit() {
            let n = ui_page.albedo.len();
            (ui_page.normal, ui_page.emissive, ui_page.height) = (vec![[128, 128]; n], vec![0; n], vec![0; n]);
        }
        atlas.pages.pages.push(ui_page);
        // Packed: the pages' slack goes (PLAY-PLAN.md §7).
        for p in &mut atlas.pages.pages {
            std::sync::Arc::make_mut(&mut p.albedo).shrink_to_fit();
            p.emissive.shrink_to_fit();
            p.normal.shrink_to_fit();
            p.height.shrink_to_fit();
            p.glow.shrink_to_fit();
        }
        let mut frame = Frame::new(tier);
        let chunks = ChunkCache::reserved(&mut frame.layers, tier);
        Present {
            atlas,
            ui_art,
            stand,
            people,
            creatures,
            kit,
            frame,
            tick: 0,
            sway: [0; 4],
            sway_at: 0,
            canvas: (CANVAS_W, CANVAS_H),
            camera: Camera::default(),
            zone: None,
            zone_cells: (0, 0),
            chunks,
            terrain,
            wants: Vec::with_capacity(64),
            entered: false,
            units: Vec::with_capacity(256),
            units_next: Vec::with_capacity(256),
            trails: Vec::new(),
            hurt: Vec::with_capacity(64),
            struck: Vec::with_capacity(64),
            props: Vec::with_capacity(1024),
            prop_scratch: Vec::with_capacity(1024),
            standing: DrawList::default(),
            seen: Vec::with_capacity(64),
            ground: DrawList::default(),
            lights: Vec::with_capacity(256),
            light_scratch: Vec::with_capacity(1024),
            holders: Vec::with_capacity(1024),
            sky: sky(12 * 7200, 0, false, 1000, Region::Lowfields),
            margins: Margins::default(),
            boss_grade: 0,
            atmos,
            fx,
            lessons: Lessons::new(tier),
            marks: Vec::with_capacity(16),
            dark: false,
            aimed: None,
            walk_to: None,
            fight: FightOverlay::default(),
            cues,
            hour: 12,
            ambient,
            lived: Vec::new(),
            emotes: Vec::with_capacity(16),
            emote_marks: Vec::with_capacity(16),
            talk: None,
        }
    }

    /// This frame's ring under her target, the cast bars and the walk marker (`ui::fight`).
    pub fn fight(&self) -> &FightOverlay {
        &self.fight
    }

    /// The moment under way, and the gifts waiting (what the UI's card, the sound and the app's
    /// hold read).
    pub fn lessons(&self) -> &Lessons {
        &self.lessons
    }

    /// This frame's quest marks over heads, in unit id order (§3.8): what `ui::marks` draws.
    pub fn marks(&self) -> &[QuestMarker] {
        &self.marks
    }

    /// Dark enough that her lantern is lit: the marks glow.
    pub fn dark(&self) -> bool {
        self.dark
    }

    /// The atmosphere (the weather, the fog, the sky): what F2 and the sheet tools read, and
    /// where a sheet holds the weather (`Atmosphere::force`).
    pub fn atmos_mut(&mut self) -> &mut Atmosphere {
        &mut self.atmos
    }

    /// The `Features` rows in force (§1.3).
    pub fn features(&self) -> Features {
        self.atmos.features
    }

    /// Sets the rows, each held to what this presenter's tier draws; they show from the next
    /// tick (the particle pool) or the next frame (the rest).
    pub fn set_features(&mut self, f: Features) {
        let tier = self.frame.tier;
        let mut held = Features::of(tier);
        for k in Features::KEYS {
            if let Some(v) = f.get(k) {
                held.set(tier, k, &v);
            }
        }
        self.atmos.features = held;
    }

    /// The effect pool's parts alive: the effects' and the weather's.
    pub fn fx_count(&self) -> (usize, usize) {
        self.fx.count()
    }

    /// The sky as of the last tick.
    pub fn sky(&self) -> &Sky {
        &self.sky
    }

    /// The pages every backend uploads at boot.
    pub fn atlas(&self) -> &AtlasPages {
        &self.atlas.pages
    }

    /// Lets go of the atlas's px once the backend has them (PLAY-PLAN.md §7): a GPU backend
    /// holds them on the card, and `soft` and `gl2` keep a share of the albedo (`Page::albedo`
    /// is shared, not copied). The page table (sizes, the CLUT, the mist) stays; nothing in play
    /// reads an atlas px.
    pub fn release_atlas(&mut self) {
        for g in &mut self.atlas.pages.pages {
            *g = crate::backend::Page { w: g.w, h: g.h, ..Default::default() };
        }
    }

    /// The UI's page table: what `ui::Ui::new` takes.
    pub fn ui_art(&self) -> &crate::ui::UiArt {
        &self.ui_art
    }

    /// The frame as last drawn (with the `Ui` pass as last finished).
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The frame, for the UI to finish its pass into after [`draw`](Self::draw).
    pub fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    /// Ticks presented since New Game.
    pub fn ticks(&self) -> u32 {
        self.tick
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The presenter's largest heap holdings, bytes by capacity (`jane bench --mem`, PLAY-PLAN.md
    /// §7): the atlas's pages by layer, and the terrain chunk cache's layers.
    pub fn mem(&self) -> Vec<(&'static str, usize)> {
        fn cap<T>(v: &Vec<T>) -> usize {
            v.capacity() * std::mem::size_of::<T>()
        }
        let p = &self.atlas.pages;
        let (mut albedo, mut lit) = (0, 0);
        for g in &p.pages {
            albedo += cap(&*g.albedo) + cap(&g.glow);
            lit += cap(&g.normal) + cap(&g.emissive) + cap(&g.height);
        }
        let chunks: usize = self
            .frame
            .layers
            .iter()
            .map(|l| {
                cap(&l.albedo)
                    + cap(&l.normal)
                    + cap(&l.emissive)
                    + cap(&l.height)
                    + cap(&l.surface)
                    + cap(&l.water)
                    + cap(&l.fence)
                    + cap(&l.glow)
            })
            .sum();
        vec![
            ("atlas px held: albedo (u16 CLUT)", albedo + cap(&p.clut) + cap(&p.mist)),
            ("atlas px held: lit layers", lit),
            ("terrain chunk cache", chunks),
        ]
    }

    /// A chunk's slot and generation in the cache, if it is painted and fresh (the F3 view).
    pub fn chunk(&self, id: ChunkId) -> Option<(u16, u32)> {
        self.chunks.find(id)
    }

    /// The zone's size in cells, as of the last tick.
    pub fn zone_cells(&self) -> (u32, u32) {
        self.zone_cells
    }

    /// Chunks painted so far (each paint is a new `(id, generation)`).
    pub fn chunks_painted(&self) -> u32 {
        self.chunks.painted
    }

    /// Of them, those the terrain painter landed (the rest were swatches standing in).
    pub fn chunks_landed(&self) -> u32 {
        self.chunks.landed
    }

    /// Units and props kept this tick (in and round the view).
    pub fn seen(&self) -> (usize, usize) {
        (self.units.len(), self.props.len())
    }

    /// Tells the presenter the canvas before the first frame (else it frames for 768 x 432 until
    /// the first `draw`).
    pub fn set_canvas(&mut self, canvas: (u16, u16)) {
        self.canvas = canvas;
    }

    /// One tick of presentation: reads the view and this tick's events (all of them, or this
    /// seat's; others are filtered out here).
    pub fn tick(&mut self, view: &View<'_>, events: &[Event]) {
        self.tick = self.tick.wrapping_add(1);
        let key = (view.zone(), view.seed());
        if self.zone != Some(key) {
            self.zone = Some(key);
            self.chunks.drop_all();
            // A small zone (a house, a crypt) keeps only the slots it has chunks for; the county
            // grows them back as its view wants (PLAY-PLAN.md §7).
            let (w, h) = view.size();
            let span = |c: u32| (c as usize * CELL as usize).div_ceil(CHUNK_PX as usize);
            self.chunks.shrink(&mut self.frame.layers, (span(w) * span(h)).min(RESERVE));
            self.terrain.zone(view);
            self.entered = true;
            self.camera.reset();
            self.units.clear();
            self.atmos.zone(view);
            self.fx.zone(view);
            self.lessons.zone(view);
            self.ambient.zone();
            self.lived = lived_houses(view, self.terrain.houses());
            self.emotes.clear();
        }
        // A room's windows lay daylight on its floor and are dark at night (ART-PLAN M4): its
        // chunks are painted again when the lamps come on or go off.
        if self.terrain.set_daylight(!view.lamps_lit()) {
            let (w, h) = view.size();
            self.chunks.invalidate(Rect::new(0, 0, w as i32, h as i32));
        }
        self.zone_cells = view.size();
        let kit = &self.kit;
        self.cues.zone(view, |s| kit.glass(s));
        self.hour = view.hour();
        self.hurt.clear();
        self.struck.clear();
        for e in events_for(events, view.me()) {
            match e.kind {
                // A tile reaches a few cells round it in what the painter draws.
                EventKind::Tiles(r) => {
                    let g = jane_art::terrain::REACH;
                    self.chunks.invalidate(Rect::new(r.x - g, r.y - g, r.w + 2 * g, r.h + 2 * g));
                }
                EventKind::Shake(n) => self.camera.shake(n),
                EventKind::Camera { mode, rect } => {
                    self.camera.lock = match mode {
                        CameraMode::Lock => rect.and_then(|s| view.rect(s)),
                        CameraMode::Follow => None,
                    };
                }
                EventKind::Damage { unit, .. } => self.hurt.push(unit.get()),
                EventKind::Cast { unit, spell, .. } => self.struck.push((unit.get(), spell)),
                _ => {}
            }
        }
        let body = view.body();
        self.camera.tick((body.pos.x.0, body.pos.y.0), self.zone_cells, self.canvas, self.tick);
        let area = self.area();
        // Her side of a fight (PLAY-PLAN §2.1): her target as the sim kept it, a click-walk's end.
        let cat = jane_data::catalog();
        self.aimed = view.target().map(|to| match to {
            jane_sim::input::TargetRef::Unit(id) => {
                let u = view.unit(id);
                TargetSeen {
                    to,
                    hostile: view.target_hostile(to),
                    name: u.map_or("", |u| crate::text::text(cat.combat.unit(u.def).name)),
                    hp: u.map(|u| (u.hp.0 / 1000, jane_sim::units::max_hp(u).0 / 1000)),
                }
            }
            jane_sim::input::TargetRef::Prop(_) => TargetSeen { to, hostile: false, name: "", hp: None },
        });
        self.walk_to = view
            .fight()
            .walk
            .as_ref()
            .filter(|w| {
                matches!(
                    w.then,
                    jane_sim::state::WalkThen::Stop
                        | jane_sim::state::WalkThen::Use(jane_sim::input::TargetRef::Prop(_))
                )
            })
            .map(|w| (w.to.x.0 >> FX_TO_CANVAS, w.to.y.0 >> FX_TO_CANVAS));
        self.read_units(view, area);
        self.read_emotes(view, area);
        self.read_props(view, area);
        self.read_lights(view, area);
        self.paint_chunks(view);
        self.against_walls();
        let (clock, day) = view.clock();
        self.sky = sky(clock, day, view.indoor(), view.ambient().0, view.region());
        // A dungeon's theme grades it, and shifts when its boss's fight begins (ART-PLAN B2):
        // she stands in the boss's room with the boss alive in it.
        if let Some(theme) = jane_data::dungeon_themes().of(view.zone()) {
            let (hx, hy) = body.pos.cell();
            let boss_room = self
                .terrain
                .dungeon()
                .and_then(|d| d.room_at(hx, hy))
                .filter(|r| r.role == jane_art::terrain::dungeon::Role::Boss);
            let cat = jane_data::catalog();
            let fighting = boss_room
                .is_some_and(|r| view.units_in(r.rect).any(|u| u.unit.alive && cat.combat.unit(u.unit.def).boss));
            self.boss_grade = if fighting { (self.boss_grade + 6).min(256) } else { self.boss_grade.saturating_sub(3) };
            crate::light::theme_grade(&mut self.sky, theme, view.indoor(), self.boss_grade);
        }
        self.margins = crate::shadow::cast_margins(self.sky.sun.as_ref());
        self.atmos.tick(view, self.tick);
        self.fx.on_events(view, events);
        let me = view.me().unit.get();
        let her = self.units.binary_search_by_key(&me, |r| r.id).ok().map(|i| {
            let u = &self.units[i];
            Her {
                at: (u.cur.0 >> FX_TO_CANVAS, u.cur.1 >> FX_TO_CANVAS),
                facing: nearest_axis(u.face),
                walking: u.anim > 0,
                alive: !u.dead,
            }
        });
        self.lessons.tick(view, events, her);
        let (cw, ch) = (i32::from(self.canvas.0), i32::from(self.canvas.1));
        let cam = (self.camera.pos.0 >> FX_TO_CANVAS, self.camera.pos.1 >> FX_TO_CANVAS);
        self.fx.set_cap(self.atmos.features.max_particles);
        self.fx.tick(view, &self.atmos, self.tick, (cam.0, cam.1, cw, ch));
        // A cast building draws its motes in to her hands (PLAY-PLAN §2.1).
        for u in &self.units {
            if let Some((frac, _, spell)) = u.build.filter(|_| !u.dead) {
                let facing =
                    view.unit(jane_sim::UnitId::new(u.id).expect("a unit id")).map_or(Facing::South, |b| b.facing);
                self.fx.gather((u.cur.0 >> FX_TO_CANVAS, u.cur.1 >> FX_TO_CANVAS), facing, spell, frac, u.id);
            }
        }
        // The ambient life (ART-PLAN M1): the chimneys that smoke and the lamps moths circle, then
        // who is where this tick.
        let atlas = &self.atlas;
        let chimneys = self.props.iter().filter(|p| p.smokes).map(|p| {
            let r = atlas.get(p.look);
            let top = p.y + p.h - rows_up(i32::from(r.top)) + 1;
            (p.id, p.x + p.w / 2, top, p.y + p.h)
        });
        let lamps =
            self.lights.iter().filter(|l| l.height >= 20).map(|l| (l.id, l.x, l.y - rows_up(i32::from(l.height))));
        self.ambient.set_sources(chimneys, lamps);
        let people: Vec<(i32, i32)> = self
            .units
            .iter()
            .filter(|u| !u.dead)
            .map(|u| ((u.cur.0 >> FX_TO_CANVAS).div_euclid(CELL), (u.cur.1 >> FX_TO_CANVAS).div_euclid(CELL)))
            .collect();
        let her_feet = self
            .units
            .iter()
            .find(|u| u.me && !u.dead)
            .map(|u| ((u.cur.0 >> FX_TO_CANVAS, u.cur.1 >> FX_TO_CANVAS), u.prev != u.cur));
        let ctx = ambient::Ctx {
            tick: self.tick,
            view_px: (cam.0, cam.1, cw, ch),
            her: her_feet,
            atmos: &self.atmos,
            people: &people,
        };
        self.ambient.tick(view, &ctx, &self.creatures);
    }

    /// This frame's emotes over heads (ART-PLAN B3), for the UI to draw over the world.
    pub fn emotes(&self) -> &[EmoteMarker] {
        &self.emote_marks
    }

    /// Emotes (ART-PLAN B3), presentation events keyed on what is said and what people are doing.
    /// In a conversation, a line that asks shows a "?" over the speaker, one that exclaims a "!",
    /// one that trails off three dots, and the dog a heart. Out of one, now and then on each one's
    /// own beat: the sweeper and the milkman whistle at their work, Mr Cobb sings outside the Arms
    /// of an evening, the courting pair show a heart when they are near each other, Mrs Tace's
    /// three dots, the dog's heart when she stands by it.
    fn read_emotes(&mut self, view: &View<'_>, area: Rect) {
        let t = self.tick;
        self.emotes.retain(|e| t.wrapping_sub(e.2) < EMOTE_TICKS);
        let cat = jane_data::catalog();
        let showing = |emotes: &[(u32, Emote, u32)], id: u32| emotes.iter().any(|e| e.0 == id);
        let talk = view.dialogue().and_then(|d| match d.speaker {
            jane_sim::state::Speaker::Unit(id) => Some((id, d)),
            _ => None,
        });
        if let Some((id, d)) = talk {
            let key = (id.get(), d.line);
            if self.talk != Some(key) && !showing(&self.emotes, key.0) {
                let line =
                    d.node.and_then(|n| n.lines.get(usize::from(d.line))).map_or("", |l| crate::text::text(l.text));
                let line = line.trim();
                let dog = view.unit(id).is_some_and(|u| cat.combat.unit(u.def).id == "dog");
                let first = self.talk.is_none_or(|w| w.0 != key.0);
                // Castle talks quietly (VOICE.md): a question asked anywhere in the line is a "?",
                // news passed on or a trailing-off is three dots, someone come on after dark
                // starts.
                let news = d.node.is_some_and(|n| n.id.starts_with("news"));
                let night = !(6..21).contains(&(view.clock().0 / 7200 % 24));
                let e = if line.contains('?') {
                    Some(Emote::Ask)
                } else if news || line.contains("...") || line.contains('\u{2026}') || line.contains("don't know") {
                    Some(Emote::Dots)
                } else if line.ends_with('!') || first && night && !dog {
                    Some(Emote::Bang)
                } else if dog && first {
                    Some(Emote::Heart)
                } else {
                    None
                };
                if let Some(e) = e {
                    self.emotes.push((key.0, e, t));
                }
            }
            self.talk = Some(key);
        } else {
            self.talk = None;
        }
        let hour = view.clock().0 / 7200 % 24;
        let her = (view.body().pos.x.0, view.body().pos.y.0);
        let reach = |cells: i32| (cells * CELL) << FX_TO_CANVAS;
        let talking = self.talk.map(|k| k.0);
        let mut courting: [Option<(i32, i32, u32)>; 2] = [None, None];
        let mut due = Vec::new();
        for uv in view.units_in(area) {
            let u = uv.unit;
            let id = u.id.get();
            if !u.alive || Some(id) == talking || showing(&self.emotes, id) {
                continue;
            }
            let name = cat.combat.unit(u.def).id;
            let at = (u.pos.x.0, u.pos.y.0);
            if name == "town_courting_a" {
                courting[0] = Some((at.0, at.1, id));
            } else if name == "town_courting_b" {
                courting[1] = Some((at.0, at.1, id));
            }
            // Each on its own beat, about once in twenty seconds.
            if (t + jane_core::hash::mix32(id)) % 1200 != 0 {
                continue;
            }
            let still = !uv.moved;
            let near_her = (her.0 - at.0).abs() < reach(3) && (her.1 - at.1).abs() < reach(3);
            let e = match name {
                "town_sweeper" | "town_milkman" if still => Some(Emote::Note),
                "town_drinker" if still && hour >= 18 => Some(Emote::Note),
                "town_widow" if still => Some(Emote::Dots),
                "dog" if near_her => Some(Emote::Heart),
                _ => None,
            };
            if let Some(e) = e {
                due.push((id, e, t));
            }
        }
        if let [Some(a), Some(b)] = courting {
            let near = (a.0 - b.0).abs() < reach(3) && (a.1 - b.1).abs() < reach(2);
            if near && (t + a.2) % 900 == 0 && !showing(&self.emotes, a.2) {
                due.push((a.2, Emote::Heart, t));
            }
        }
        self.emotes.extend(due);
    }

    /// The ambient layer's living things this tick (ART-PLAN §7 rule 5's test reads it).
    pub fn ambient(&self) -> &Ambient {
        &self.ambient
    }

    /// The cells the view covers this tick, with the margin.
    fn area(&self) -> Rect {
        let (x, y) = (self.camera.pos.0 >> CELL_SHIFT, self.camera.pos.1 >> CELL_SHIFT);
        let (w, h) = (i32::from(self.canvas.0) / CELL + 2, i32::from(self.canvas.1) / CELL + 2);
        let m = self.margins.grow(KEEP_PAST);
        let cells = |px: i32| (px + CELL - 1) / CELL;
        let (l, t, r, b) = (cells(m.left), cells(m.top), cells(m.right), cells(m.bottom));
        Rect::new(x - l, y - t, w + l + r, h + t + b)
    }

    fn read_units(&mut self, view: &View<'_>, area: Rect) {
        let me = view.me().unit;
        let my_seat = view.seat().index() as u8;
        let cat = jane_data::catalog();
        self.units_next.clear();
        self.trails.clear();
        // Whom she is talking to: the mark over them is down while she does.
        let talking_to = view.dialogue().and_then(|d| match d.speaker {
            jane_sim::state::Speaker::Unit(id) => Some(id),
            _ => None,
        });
        for uv in view.units_in(area) {
            let u = uv.unit;
            let id = u.id.get();
            if let Some(s) = u.snake.as_deref().filter(|_| u.alive) {
                self.trails.push((id, s.trail.iter().map(|p| (p.x.0, p.y.0)).collect()));
            }
            let cur = (u.pos.x.0, u.pos.y.0);
            let old = self.units.binary_search_by_key(&id, |r| r.id).ok().map(|i| self.units[i]);
            // Last tick's position as this presenter drew it; a newcomer from the view's.
            let prev = old.map_or((uv.prev_pos.x.0, uv.prev_pos.y.0), |o| o.cur);
            let kind = if !u.alive {
                UnitKind::Dead
            } else if u.id == me {
                UnitKind::Me
            } else if u.controller == Controller::Player {
                UnitKind::Seat
            } else if u.faction == Faction::Friendly {
                UnitKind::Folk(uv.variant)
            } else {
                UnitKind::Hostile
            };
            // A seat's coat: hers by her seat, another player's by his (the same coat on every
            // machine at the table).
            let seat = match kind {
                UnitKind::Me => my_seat,
                UnitKind::Seat => view.seat_of(u.id).map_or(0, |s| s.index() as u8),
                _ => 0,
            };
            let person = self.people.set(cat.combat.unit(u.def).sprite, uv.variant, seat);
            let face = old.map_or(Face8::of(u.facing), |o| o.face).moving(
                i64::from(cur.0 - prev.0),
                i64::from(cur.1 - prev.1),
                u.facing,
            );
            // Landed: she came into the zone, or anyone pulled up out of a sprint (a last step
            // longer than its walk's).
            let walk = i64::from(cat.combat.unit(u.def).walk.0);
            let sprinted = old.is_some_and(|o| {
                let (dx, dy) = (i64::from(o.cur.0 - o.prev.0), i64::from(o.cur.1 - o.prev.1));
                o.anim > 0 && (dx * dx + dy * dy) * 64 > walk * walk * 81
            });
            let landed = (old.is_none() && matches!(kind, UnitKind::Me)) || (prev == cur && sprinted);
            self.units_next.push(UnitRec {
                id,
                prev,
                cur,
                anim: if prev == cur { 0 } else { old.map_or(0, |o| o.anim) + 1 },
                hurt_until: old.map_or(0, |o| o.hurt_until),
                look: self.stand.unit(kind),
                mirror: u.facing == Facing::West,
                dead: !u.alive,
                me: matches!(kind, UnitKind::Me),
                player: matches!(kind, UnitKind::Me | UnitKind::Seat),
                seen: matches!(kind, UnitKind::Me | UnitKind::Seat | UnitKind::Hostile),
                glow: jane_data::catalog()
                    .combat
                    .unit(u.def)
                    .glow
                    .filter(|_| u.alive)
                    .map(|g| ((g.radius.0 >> FX_TO_CANVAS).clamp(0, 1024) as u16, rgb(g.color))),
                person,
                creature: person.map_or_else(|| self.creatures.set(cat.combat.unit(u.def).sprite), |_| None),
                still: if prev == cur { old.map_or(0, |o| o.still.saturating_add(1)) } else { 0 },
                struck: old.and_then(|o| o.struck),
                raise: u.feel.windup.filter(|_| u.alive).map(|w| w.spell),
                face,
                under: {
                    let (x, y) = u.pos.cell();
                    view.tile(x, y).is_roof()
                },
                mark: if talking_to == Some(u.id) { None } else { view.quest_mark(u) },
                build: view.casting(u.id).filter(|_| u.alive).map(|c| {
                    let (len, done) =
                        (c.done.0.saturating_sub(c.started.0).max(1), view.tick().0.saturating_sub(c.started.0));
                    ((done.min(len) * 256 / len) as u16, cat.combat.spell(c.spell).school, c.spell)
                }),
                ring: ((cat.combat.unit(u.def).bounds.0 >> FX_TO_CANVAS) + 6).clamp(9, 30),
                turned: match old {
                    Some(o) if o.face != face => Some((self.tick, o.face)),
                    _ => old.and_then(|o| o.turned),
                },
                landed: if landed { Some(self.tick) } else { old.and_then(|o| o.landed) },
                task: person.and_then(|s| self.people.at_work(s, || view.schedule_state(u.id))),
            });
        }
        self.units_next.sort_unstable_by_key(|r| r.id);
        std::mem::swap(&mut self.units, &mut self.units_next);
        for &id in &self.hurt {
            if let Ok(i) = self.units.binary_search_by_key(&id, |r| r.id) {
                self.units[i].hurt_until = self.tick + HURT_TICKS;
            }
        }
        for &(id, spell) in &self.struck {
            if let Ok(i) = self.units.binary_search_by_key(&id, |r| r.id) {
                self.units[i].struck = Some((self.tick, spell));
            }
        }
    }

    fn read_props(&mut self, view: &View<'_>, area: Rect) {
        let cat = jane_data::catalog();
        let (props, stand, kit) = (&mut self.props, &self.stand, &self.kit);
        let houses = self.terrain.houses();
        let lived = &self.lived;
        // A chimney smokes on a house someone lives in (ART-PLAN M1); an empty one's is cold.
        let chimney = jane_art::looks::find("town_chimney").map(|(s, _)| s);
        let room = self.terrain.room().map(|r| r.kind);
        props.clear();
        // What she is reading: the mark over it is down while she does.
        let reading = view.dialogue().and_then(|d| match d.speaker {
            jane_sim::state::Speaker::Prop(id) => Some(id),
            _ => None,
        });
        // The pit she is laying a fire in: USE held on it (`jane_sim::fire::hold`).
        let laying = (view.body().hold > 0)
            .then(|| view.focus())
            .flatten()
            .filter(|f| f.verb == jane_sim::interact::Verb::MakeFire)
            .and_then(|f| match f.target {
                jane_sim::interact::FocusRef::Prop(id) => Some(id),
                _ => None,
            });
        // What the dungeon's theme gathers off its floors into its motif (bones into heaps, ART-PLAN
        // Q5): drawn there, not where it lies, unless it is a thing anyone can use.
        let gathers: Vec<&'static str> = jane_data::dungeon_themes()
            .of(view.zone())
            .map(|t| t.floor.iter().flat_map(|f| f.gathers.iter().copied()).collect())
            .unwrap_or_default();
        view.for_props_in(area, &mut self.prop_scratch, |p| {
            let d = cat.story.prop(p.def);
            if !gathers.is_empty()
                && d.flat
                && !p.solid
                && gathers.contains(&cat.sprites.get(usize::from(d.sprite.0)).copied().unwrap_or(""))
                && view.prop_loot(p).is_empty()
                && view.prop_spawn(p).is_none_or(|s| s.talk.is_none() && s.to.is_none())
            {
                return;
            }
            // A made fire (`jane_sim::fire`): cold, laid while she lays it, lit, or ash.
            let fire = d.made.then(|| {
                use props::FireState;
                if p.on {
                    FireState::Lit
                } else if laying == Some(p.id) {
                    FireState::Laid
                } else if p.used {
                    FireState::Ash
                } else {
                    FireState::Cold
                }
            });
            // An open gate is a doorway.
            if d.gate && !p.solid {
                return;
            }
            // Its look from the kit, in its state (a lamp alight, a chest looted), else its
            // stand-in.
            let lit = d.light.is_some() && view.light_showing(p).is_some();
            let state = props::State {
                on: p.on || lit,
                open: p.used || !matches!(p.loot, jane_sim::state::LootState::AsSpawned),
            };
            // A thing left lying (a key, a glove) is drawn as what it holds, not as its row's
            // sprite: Mrs Bettany's key was a note on the grass (the owner's first playtest).
            let held = d
                .shows_loot
                .then(|| view.prop_loot(p).first())
                .flatten()
                .and_then(|s| kit.loot_look(cat.combat.item(s.item).icon));
            // A room lays a rug under its table, along its bed's foot and before its hearth
            // (ART-PLAN M4): flat on the floor under everything, nothing a foot meets.
            // A pub lays none under its tables, a church none at all.
            let lays = |rug: props::Rug| match room {
                Some(jane_art::terrain::houses::RoomKind::Julie) => true,
                Some(jane_art::terrain::houses::RoomKind::Inn) => rug != props::Rug::Table,
                _ => false,
            };
            if let Some((rug, look)) = kit.rug(d.sprite, p.id.get()).filter(|r| lays(r.0)) {
                let (cx, cy, cw, ch) = (i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
                let (mut x, y, w, h) = match rug {
                    props::Rug::Table => (cx + cw / 2 - 2, cy + ch / 2 - 1, 4, 3),
                    props::Rug::Bed => (cx + cw - 6, cy + ch, 6, 2),
                    props::Rug::Hearth => (cx + cw / 2 - 2, cy + ch, 4, 3),
                };
                // Kept off the walls: moved along until both its ends lie on the floor.
                let solid = |x: i32| view.tile(x, y).flags() & jane_core::tile::F_SOLID != 0;
                while solid(x) && x < cx + cw {
                    x += 1;
                }
                while solid(x + w - 1) && x > cx - w {
                    x -= 1;
                }
                props.push(PropRec {
                    id: RUG_KEY | p.id.get(),
                    x: x * CELL,
                    y: y * CELL,
                    w: w * CELL,
                    h: h * CELL,
                    look,
                    flat: true,
                    flush: false,
                    on_top: false,
                    surface: None,
                    lift: 0,
                    sort_foot: None,
                    mark: None,
                    smokes: false,
                });
            }
            props.push(PropRec {
                id: p.id.get(),
                x: i32::from(p.cell.x) * CELL,
                y: i32::from(p.cell.y) * CELL,
                w: i32::from(d.w) * CELL,
                h: i32::from(d.h) * CELL,
                look: held
                    .or_else(|| {
                        // A front door or a chimney on a house with a look is its house's own
                        // (ART-PLAN Q2); St Anne's keeps its oak.
                        let house = houses.at(i32::from(p.cell.x), i32::from(p.cell.y));
                        let house = house.filter(|h| h.kind != jane_art::terrain::houses::Kind::Church);
                        house.and_then(|h| kit.house_look(d.sprite, &h.look()))
                    })
                    .or_else(|| match fire {
                        Some(f) => kit.fire_look(d.sprite, p.id.get(), f),
                        None => kit.look(d.sprite, p.id.get(), state),
                    })
                    .unwrap_or_else(|| {
                        let look = stand.prop(d.w, d.h, d.flat, d.light.is_some());
                        // A lamp the view says is out has dark glass.
                        if d.light.is_some() && !lit { stand.unlit(look) } else { look }
                    }),
                flat: d.flat,
                flush: false,
                on_top: {
                    let t = view.tile(i32::from(p.cell.x), i32::from(p.cell.y));
                    t.is_roof() || t.flags() & jane_core::tile::F_SOLID != 0
                },
                surface: kit.surface(d.sprite),
                lift: 0,
                sort_foot: None,
                mark: if reading == Some(p.id) { None } else { view.prop_quest_mark(p) },
                smokes: chimney.is_some_and(|c| c == d.sprite)
                    && houses
                        .at(i32::from(p.cell.x), i32::from(p.cell.y))
                        .is_some_and(|h| lived.contains(&(h.rect.x, h.rect.y))),
            });
        });
        stand_on_tops(props);
        // The Hoar Stone on the stair's block: drawn while any of its height can be on screen,
        // so it shows over the bottom edge before the block does.
        // The regions' landmarks likewise, each as tall as it is (`cues`): the chimney, the
        // spire, the statue.
        let stone = self.cues.stone().map(|(foot, look)| (foot, look, jane_art::far::HOAR_W / 2));
        let tall = self.cues.standing(self.hour).map(|(foot, look, ax, _)| (foot, look, ax));
        for (i, ((sx, sy), look, ax)) in stone.into_iter().chain(tall).enumerate() {
            let r = self.atlas.get(look);
            let reach = Rect::new(area.x - 4, area.y, area.w + 8, area.h + i32::from(r.src.h) / CELL + 1);
            if reach.contains(sx.div_euclid(CELL), sy.div_euclid(CELL)) {
                props.push(PropRec {
                    id: STONE_KEY + i as u32,
                    x: sx - ax,
                    y: sy - CELL,
                    w: i32::from(r.src.w),
                    h: CELL,
                    look,
                    flat: false,
                    flush: false,
                    on_top: true,
                    surface: None,
                    lift: 0,
                    sort_foot: None,
                    mark: None,
                    smokes: false,
                });
            }
        }
        // What lies on the ground (a creature's loot where it fell) as its item, a cell's
        // footprint round its point, on the ground under everything standing.
        for d in view.drops() {
            let (x, y) = (d.pos.x.0 >> FX_TO_CANVAS, d.pos.y.0 >> FX_TO_CANVAS);
            let inside = area.contains(x.div_euclid(CELL), y.div_euclid(CELL));
            let Some(look) = kit.loot_look(cat.combat.item(d.item).icon).filter(|_| inside) else { continue };
            props.push(PropRec {
                id: DROP_KEY | d.id.get(),
                x: x - CELL / 2,
                y: y - CELL / 2,
                w: CELL,
                h: CELL,
                look,
                flat: true,
                flush: false,
                on_top: false,
                surface: None,
                lift: 0,
                sort_foot: None,
                mark: None,
                smokes: false,
            });
        }
        props.sort_unstable_by_key(|p| p.id);
    }

    /// The prop lights the view says are showing, in reach of the view (THE rule, `View::light_showing`).
    fn read_lights(&mut self, view: &View<'_>, area: Rect) {
        let cat = jane_data::catalog();
        let reach =
            Rect::new(area.x - LIGHT_CELLS, area.y - LIGHT_CELLS, area.w + 2 * LIGHT_CELLS, area.h + 2 * LIGHT_CELLS);
        let (lights, stand, atlas, kit) = (&mut self.lights, &self.stand, &self.atlas, &self.kit);
        lights.clear();
        view.for_props_in(reach, &mut self.light_scratch, |p| {
            let Some(l) = view.light_showing(p) else { return };
            let d = cat.story.prop(p.def);
            let (x, y) = (i32::from(p.cell.x) * CELL, i32::from(p.cell.y) * CELL);
            let (w, h) = (i32::from(d.w) * CELL, i32::from(d.h) * CELL);
            // Where the flame is: a building's lit windows on its front, low; a thing on the
            // floor just above it; else where its sprite glows, standing on its foot (the row
            // its sprite stands on, as it is drawn: bottom-centred on the footprint).
            let on = props::State { on: true, open: false };
            let look = kit.look(d.sprite, p.id.get(), on).unwrap_or_else(|| stand.prop(d.w, d.h, d.flat, true));
            let r = atlas.get(look);
            let (gx, gy, height, size) = if d.w >= 3 || d.h >= 3 {
                (x + w / 2, y + h + 4, 16, 12)
            } else if d.flat {
                (x + w / 2, y + h / 2, 4, 6)
            } else {
                let foot = y + h - i32::from(r.src.h) + i32::from(r.ay);
                // A kit lamp shines from its lit glass; a stand-in from its demo glass. The glass
                // is counted in rows up from the canvas's foot; the light's height is true px
                // over the sprite's own foot row (`rows_up`'s inverse).
                let over_foot = i32::from(r.src.h) - i32::from(r.ay);
                let glass = kit
                    .glass(d.sprite)
                    .or_else(|| stand.glass(look))
                    .map_or(i32::from(r.height) * 2 / 3, |g| height_of_rows(i32::from(g) - over_foot));
                // A low flame (a fire, a stove, a lantern on the floor) lights from over its
                // tongues, as its light does in the air round it: a light at its glass would only
                // graze the ground and throw no pool.
                (x + w / 2, foot, glass.clamp(FLAME_HEIGHT, 90), if d.w == 1 { 6 } else { 10 })
            };
            lights.push(LightRec {
                id: p.id.get(),
                x: gx,
                y: gy,
                height: height as u8,
                colour: rgb(l.color),
                // An open flame (a fire, a brazier: a light that dips a fifth or more) throws its pool
                // half again as far as its row says the sentries see it: the eye sees a fire's glow
                // well past where it lights a face. Presentation only; `light_showing` is unchanged.
                radius: {
                    let r = (l.radius.0 >> FX_TO_CANVAS).clamp(0, 2048);
                    (if l.flicker.0 >= 200 { r * 3 / 2 } else { r }) as u16
                },
                size: size as u8,
                dip: l.flicker.0,
            });
        });
        lights.sort_unstable_by_key(|l| l.id);
    }

    /// A light standing on the terrain's height field, in it or over it (a torch hung on a wall,
    /// whose sprite stands on the wall's top) hangs from the wall's face instead: its ground
    /// point moves to open ground within a cell and a half, toward the viewer if there is any
    /// that way, else the nearest, and two px clear of it. A light never shadows the wall it hangs on (PRESENTATION.md §1.7), and a
    /// torch's pool lies on the floor it lights, not in the masonry, on every tier.
    ///
    /// And a prop drawn over a wall's face and standing on the face's foot (a door, a sign on the
    /// wall, a hanging: chains or a portrait, which stands on its cell's back edge by
    /// `jane_art::kit::hung`) is set into it: its px halfway and three quarters up stand on its
    /// own foot row as the face's there do. It throws no shadow of its own, so a door never
    /// shadows the wall it is set in, and chains under a torch never throw a wedge across the
    /// floor.
    fn against_walls(&mut self) {
        /// How far a light is looked for open ground, px.
        const REACH: i32 = 24;
        let (layers, chunks) = (&self.frame.layers, &self.chunks);
        let (zw, zh) = (self.zone_cells.0 as i32 * CELL, self.zone_cells.1 as i32 * CELL);
        let last: std::cell::Cell<Option<(ChunkId, Option<u16>)>> = std::cell::Cell::new(None);
        // The terrain's drawn height at zone px (x, y).
        let drawn = |x: i32, y: i32| -> i32 {
            if x < 0 || y < 0 || x >= zw || y >= zh {
                return 0;
            }
            let id = ChunkId { cx: (x / CHUNK_PX) as u16, cy: (y / CHUNK_PX) as u16 };
            let slot = match last.get() {
                Some((k, s)) if k == id => s,
                _ => {
                    let s = chunks.find(id).map(|f| f.0);
                    last.set(Some((id, s)));
                    s
                }
            };
            let Some(l) = slot.and_then(|s| layers.get(usize::from(s))).filter(|l| l.has_height()) else { return 0 };
            i32::from(l.height[((y % CHUNK_PX) * CHUNK_PX + x % CHUNK_PX) as usize])
        };
        // The height field there: the tallest terrain px standing on ground (x, y) (a px `h` up
        // stands `rows_up(h)` rows below it, its field two rows deep), as tall as the terrain
        // stands: a mine's rock is taller than a house, and a torch on it was left buried in it
        // (T2 lit nothing with it, the lower tiers lit the passage through the rock).
        let field = |x: i32, y: i32| -> i32 {
            let mut most = 0;
            for r in 0..=rows_up(255) {
                let h = drawn(x, y - r);
                if h > 2 && (rows_up(h) == r || rows_up(h) == r + 1) {
                    most = most.max(h);
                }
            }
            most
        };
        let atlas = &self.atlas;
        for p in &mut self.props {
            let r = atlas.get(p.look);
            let (cx, foot) = (p.x + p.w / 2, p.y + p.h - i32::from(r.src.h) + i32::from(r.ay));
            let rows = i32::from(r.ay);
            p.flush = !p.flat
                && rows >= 8
                && [rows / 2, rows * 3 / 4].into_iter().all(|up| {
                    let h = drawn(cx, foot - up);
                    h > shadow::GROUND && (rows_up(h) - up).abs() <= 1
                });
        }
        for l in &mut self.lights {
            // On the terrain, under its light or over it: a torch on a wall's top 20 px up, its
            // light at 28, lit the rock's top and left the passage beside it in the rock's
            // shadow on every tier that cast from it (the mine, 2026-09-27).
            if field(l.x, l.y) <= 2 {
                continue;
            }
            // Toward the viewer first, as far as the reach (a face looks south: a torch on it
            // lights the floor in front of it, not the room round the wall's end); then the
            // nearest open ground any other way.
            let south = (1..=REACH).find(|&d| field(l.x, l.y + d) <= 2).map(|d| (0, 1, d));
            let other = || {
                (1..=REACH).find_map(|d| {
                    [(-1, 0), (1, 0), (0, -1)]
                        .into_iter()
                        .find(|&(dx, dy)| field(l.x + dx * d, l.y + dy * d) <= 2)
                        .map(|(dx, dy)| (dx, dy, d))
                })
            };
            if let Some((dx, dy, d)) = south.or_else(other) {
                (l.x, l.y) = (l.x + dx * (d + 2), l.y + dy * (d + 2));
            }
        }
    }

    /// Paints the chunks under the view (and a little round it) that want it: at most
    /// [`LAND_PER_TICK`] by the terrain painter, those on screen and nearest the middle first;
    /// one with nothing to show yet takes its swatches meanwhile.
    fn paint_chunks(&mut self, view: &View<'_>) {
        let cam = (self.camera.pos.0 >> FX_TO_CANVAS, self.camera.pos.1 >> FX_TO_CANVAS);
        // Ahead of her: the band reaches a further chunk on each side the view is moving toward,
        // so the cache, sized to the band (PLAY-PLAN.md §7), has new ground painted before it
        // shows.
        let (dx, dy) = (self.camera.pos.0 - self.camera.prev.0, self.camera.pos.1 - self.camera.prev.1);
        let mut ahead = self.margins.grow(CHUNK_AHEAD);
        let lead = |d: i32, toward: bool| if toward && d != 0 { MOVE_AHEAD } else { 0 };
        ahead.left += lead(dx, dx < 0);
        ahead.right += lead(dx, dx > 0);
        ahead.top += lead(dy, dy < 0);
        ahead.bottom += lead(dy, dy > 0);
        let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, ahead) else {
            return;
        };
        self.chunks.fit(((cx1 - cx0 + 1) * (cy1 - cy0 + 1)) as usize);
        let (sx0, sy0, sx1, sy1) = self.chunk_range(cam, Margins::uniform(0)).unwrap_or((cx0, cy0, cx1, cy1));
        let mid = (cam.0 + i32::from(self.canvas.0) / 2, cam.1 + i32::from(self.canvas.1) / 2);
        let (cells, outside, now) = (self.zone_cells, self.frame.clear, self.tick);
        self.wants.clear();
        let mut shown = 0;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let id = ChunkId { cx: cx as u16, cy: cy as u16 };
                self.chunks.touch(id, now);
                let need = self.chunks.need(id);
                if need == Need::Nothing {
                    continue;
                }
                let on = (sx0..=sx1).contains(&cx) && (sy0..=sy1).contains(&cy);
                shown += usize::from(on);
                let (dx, dy) = (cx * CHUNK_PX + CHUNK_PX / 2 - mid.0, cy * CHUNK_PX + CHUNK_PX / 2 - mid.1);
                let d = (dx.unsigned_abs() + dy.unsigned_abs()).min(0x00ff_ffff);
                self.wants.push((u32::from(!on) << 24 | d, id, need));
            }
        }
        self.wants.sort_unstable_by_key(|w| (w.0, w.1.cy, w.1.cx));
        let budget = if std::mem::take(&mut self.entered) { shown.max(LAND_PER_TICK) } else { LAND_PER_TICK };
        let (chunks, terrain, layers) = (&mut self.chunks, &mut self.terrain, &mut self.frame.layers);
        for (i, &(key, id, need)) in self.wants.iter().enumerate() {
            if i < budget {
                chunks.want(id, now, layers, false, |slot, l| terrain.paint(view, id, slot, outside, l));
            } else if need == Need::Missing && key >> 24 == 0 {
                chunks.want(id, now, layers, true, |slot, l| {
                    terrain.swatched(slot, l);
                    stand_in::paint_chunk(id, cells, outside, |x, y| view.tile(x, y), l);
                    terrain.stand(slot, l);
                });
            }
        }
    }

    /// The chunks under a view whose top-left is `cam` (canvas px), grown by `m` px each side and
    /// held to the zone: inclusive `(cx0, cy0, cx1, cy1)`, or `None` for an empty zone.
    fn chunk_range(&self, cam: (i32, i32), m: Margins) -> Option<(i32, i32, i32, i32)> {
        let (zw, zh) = (self.zone_cells.0 as i32 * CELL, self.zone_cells.1 as i32 * CELL);
        if zw <= 0 || zh <= 0 {
            return None;
        }
        let last = ((zw - 1) / CHUNK_PX, (zh - 1) / CHUNK_PX);
        let c0 = |v: i32, hi: i32| v.div_euclid(CHUNK_PX).clamp(0, hi);
        Some((
            c0(cam.0 - m.left, last.0),
            c0(cam.1 - m.top, last.1),
            c0(cam.0 + i32::from(self.canvas.0) + m.right, last.0),
            c0(cam.1 + i32::from(self.canvas.1) + m.bottom, last.1),
        ))
    }

    /// The frame at `alpha` (0..=255, how far from the last tick to the next) for a canvas of
    /// `canvas` px.
    pub fn draw(&mut self, alpha: u8, canvas: (u16, u16)) -> &Frame {
        self.canvas = canvas;
        let cam = self.camera.at(alpha);
        let f = &mut self.frame;
        f.canvas = canvas;
        f.camera = cam;
        f.passes.clear();
        f.chunks.clear();
        f.sprites.clear();
        f.lights.clear();
        f.casters.clear();
        f.blocks.clear();
        self.marks.clear();
        self.fight.target = None;
        self.fight.bars.clear();
        self.fight.tick = self.tick;
        self.fight.walk = self.walk_to.map(|(x, y)| (x - cam.0, y - cam.1));
        self.emote_marks.clear();
        self.dark = lantern_lit(self.sky.ambient);
        if self.zone.is_none() {
            return &self.frame;
        }
        // The sky and the far things on its horizon, behind everything (§1.9).
        self.atmos.draw_back(&mut self.frame, cam, self.zone_cells, &self.atlas);

        // Terrain: every painted chunk under the view and its casting band (the band's are
        // clipped away on screen; T2's G-buffer stands them in its field).
        let band = self.margins;
        self.frame.guard = band.most().clamp(0, i32::from(u16::MAX)) as u16;
        if let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, band) {
            let f = &mut self.frame;
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    let id = ChunkId { cx: cx as u16, cy: cy as u16 };
                    if let Some((slot, generation)) = self.chunks.find(id) {
                        f.chunks.push(ChunkCmd {
                            id,
                            generation,
                            x: cx * CHUNK_PX - cam.0,
                            y: cy * CHUNK_PX - cam.1,
                            slot,
                        });
                    }
                }
            }
            f.passes.push(Pass::Terrain { chunks: Span::since(0, f.chunks.len()) });
            crate::atmos::water_pass(f, self.atmos.features.water);
        }

        // Props and units, flat ones on the ground, the rest y-sorted with the units.
        self.ground.clear();
        self.standing.clear();
        self.seen.clear();
        let a = i64::from(alpha_256(alpha));
        let (cw, ch) = (i32::from(canvas.0), i32::from(canvas.1));
        let on_canvas =
            |x: i32, y: i32, w: u16, h: u16| x + i32::from(w) > 0 && y + i32::from(h) > 0 && x < cw && y < ch;
        // What stands in the casting band round the canvas is kept, if it casts: its shadow may
        // lie on screen (§1.7). Drawn, it is clipped away.
        let in_band = |x: i32, y: i32, w: u16, h: u16| {
            x + i32::from(w) > -band.left && y + i32::from(h) > -band.top && x < cw + band.right && y < ch + band.bottom
        };
        // Whether the terrain stands in front of feet at zone px `(x, y)` (PRESENTATION.md §1.6,
        // *behind the terrain*): what is drawn over the row above them stands on ground south of
        // it. Then every tier leaves out what of the sprite the terrain stands over.
        let (chunks, layers, cells) = (&self.chunks, &self.frame.layers, self.zone_cells);
        let behind = |x: i32, y: i32| {
            let h = drawn_height(chunks, layers, cells, (x, y - 1));
            Foot::hides(h.clamp(0, 255) as u8, y - 1, y)
        };
        for p in &self.props {
            let r = self.atlas.get(p.look);
            // Bottom-centred on the footprint.
            let x = p.x + (p.w - i32::from(r.src.w)) / 2 - cam.0;
            let y = p.y + p.h - i32::from(r.src.h) - cam.1 - p.lift;
            let casts = !p.flat && !p.flush && p.sort_foot.is_none();
            if !(on_canvas(x, y, r.src.w, r.src.h) || casts && in_band(x, y, r.src.w, r.src.h)) {
                continue;
            }
            // Her target: a gold ring at its front edge.
            if let Some(t) =
                self.aimed.filter(|t| matches!(t.to, jane_sim::input::TargetRef::Prop(pid) if pid.get() == p.id))
            {
                self.fight.target = Some(TargetMarker {
                    x: p.x + p.w / 2 - cam.0,
                    y: p.y + p.h - cam.1 - p.lift,
                    half: (p.w / 2 + 6).clamp(11, 40),
                    head: y,
                    hostile: false,
                    name: t.name,
                    hp: None,
                });
            }
            // Over its top, as over a head.
            if let Some(mark) = p.mark {
                let id = PROP_MARK_KEY | p.id;
                self.marks.push(QuestMarker { id, x: x + i32::from(r.src.w) / 2, y: y - MARK_CLEAR, mark });
            }
            let foot = p.sort_foot.unwrap_or(p.y + p.h) - cam.1;
            let caster = casts.then(|| {
                // It stands on what is drawn (§1.7): a thing drawn over its footprint's front
                // edge (a fire in the middle of its cell, a sign's post over its contact shadow)
                // throws from its lowest drawn row, its heights less what they counted under it.
                let (stand, sink) = if r.base >= 0 && r.base + 1 < r.ay {
                    (r.base + 1, r.base_height.saturating_sub(1))
                } else {
                    (r.ay, 0)
                };
                // Its flame and its lit glass are light, not matter: they cast nothing.
                let burn = if r.burn.0 >= 0 {
                    let hv = |row: i16| (stand - row).clamp(1, 255) as u8;
                    (hv(r.burn.1), hv(r.burn.0))
                } else {
                    (0, 0)
                };
                Caster {
                    sprite: 0,
                    foot: clamp16(x + i32::from(r.src.w) / 2, y + i32::from(stand)),
                    height: r.top.saturating_sub(sink).max(1),
                    depth: (p.h / 4).clamp(4, 12) as u8,
                    sink,
                    burn,
                }
            });
            let mut cmd = DrawCmd { y: foot, key: p.id, sprite: sprite(r, x, y, Flags::default()), caster };
            if !p.flat && !p.flush && !p.on_top && behind(p.x + p.w / 2, p.y + p.h) {
                cmd.sprite.foot = Some(Foot { y: clamp16(0, foot).1, see: false });
            }
            if p.flat { self.ground.push(cmd) } else { self.standing.push(cmd) }
        }
        // The chunks' trees, shrubs and stones, from the atlas, by their feet. Crowns, shrubs,
        // ferns and reeds sway (ART-PLAN M2): the frame by the tick, the plant's own phase and the
        // wind (a breeze slow, a storm quick), rest, one way, rest, the other; reeds and long grass
        // she walks through rustle, quick, while she moves among them. Presentation only.
        let wind = self.atmos.wind();
        // Quicker in wind: a breeze's pace, down to half of it in a gale.
        let pace = |class: crate::terrain::SwayClass| {
            (class.period() as i32 * 32 / (32 + wind.unsigned_abs().min(32) as i32)).max(40) as u32
        };
        // The phases, advanced over the ticks since the last draw (a long gap only so far).
        let steps = self.tick.wrapping_sub(self.sway_at).min(600);
        self.sway_at = self.tick;
        for class in crate::terrain::SwayClass::ALL {
            let per = ((1u64 << 32) / u64::from(pace(class))) as u32;
            let k = class as usize;
            self.sway[k] = self.sway[k].wrapping_add(per.wrapping_mul(steps));
        }
        let phases = self.sway;
        let walker = self
            .units
            .iter()
            .find(|u| u.me)
            .filter(|u| u.prev != u.cur)
            .map(|u| ((u.cur.0 >> FX_TO_CANVAS) - cam.0, (u.cur.1 >> FX_TO_CANVAS) - cam.1));
        let tick = self.tick;
        self.fx.loose.clear();
        if let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, band.grow(SORT_PAST)) {
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    let Some((slot, _)) = self.chunks.find(ChunkId { cx: cx as u16, cy: cy as u16 }) else {
                        continue;
                    };
                    let (ox, oy) = (cx * CHUNK_PX - cam.0, cy * CHUNK_PX - cam.1);
                    for (i, pl) in self.terrain.placed(slot).iter().enumerate() {
                        let fl = self.terrain.flora(pl.sprite);
                        let (fx, fy) = (ox + i32::from(pl.x), oy + i32::from(pl.y));
                        let (zx, zy) = (fx + cam.0, fy + cam.1);
                        let own = jane_core::hash::mix32(zx as u32 ^ (zy as u32).rotate_left(16));
                        let rustle =
                            fl.rustles && walker.is_some_and(|(wx, wy)| (wx - fx).abs() <= 8 && (wy - fy).abs() <= 6);
                        let (level, gust) = if rustle {
                            ([-2, 2][((tick / 3).wrapping_add(own) % 2) as usize], 65536)
                        } else {
                            sway(fl.class, zx, zy, own, phases[fl.class as usize] >> 16, tick, wind)
                        };
                        let r = self.atlas.get(fl.look);
                        let (x, y) = (fx - i32::from(r.ax), fy - i32::from(r.ay));
                        let bend = Bend { lean: level as i8, ..fl.bend };
                        let reach = bend.reach();
                        if !in_band(x - reach, y, r.src.w + 2 * reach as u16, r.src.h) {
                            continue;
                        }
                        let mut sp = sprite(r, x, y, Flags { bend, ..Flags::default() });
                        if behind(fx + cam.0, fy + cam.1) {
                            sp.foot = Some(Foot { y: clamp16(0, fy).1, see: false });
                        }
                        let key = 0x4000_0000 | u32::from(slot) << 10 | i as u32;
                        self.standing.push(DrawCmd {
                            y: fy,
                            key,
                            sprite: sp,
                            // A stand of reeds or long grass casts nothing, as the tufts painted
                            // round it cast nothing (`shadow::RELIEF`): it is growth, not a thing.
                            caster: (!fl.rustles).then(|| Caster {
                                sprite: 0,
                                foot: clamp16(fx, fy - i32::from(fl.lift)),
                                height: r.top.max(1),
                                depth: fl.depth,
                                ..Caster::default()
                            }),
                        });
                        // Its loose leaves on the windward edge (ART-PLAN §9): drawn over it,
                        // the same key, so straight after it.
                        let windward = if wind < 0 { 1 } else { -1 };
                        for (k, cl) in fl.leaves.iter().enumerate().filter(|(_, c)| c.side == windward) {
                            let (cx, cy) = (x + i32::from(cl.x) + bend.shift(i32::from(cl.y)), y + i32::from(cl.y));
                            let h = jane_core::hash::mix32(own ^ (k as u32 + 1).wrapping_mul(0x9e37_79b9));
                            if let Some((dx, dy, lit)) = flutter(h, tick, gust, wind) {
                                let lr = self.atlas.get(if lit { cl.lit } else { cl.plain });
                                let mut lp = sprite(lr, cx + dx * i32::from(windward), cy + dy, Flags::default());
                                lp.foot = sp.foot;
                                self.standing.push(DrawCmd { y: fy, key, sprite: lp, caster: None });
                            }
                            // The odd one lets go and joins the wind's leaves.
                            if let Some(age) = let_go(h, tick) {
                                let (gx, gy) = (cx + cam.0 + 1, fy + cam.1);
                                self.fx.loose.push(crate::fx::Loose {
                                    x: gx,
                                    y: gy,
                                    z: fy - cy,
                                    age,
                                    seed: h,
                                    colour: cl.colour,
                                });
                            }
                        }
                    }
                }
            }
        }
        let lantern = lantern_lit(self.sky.ambient);
        let mut glows: [Option<Light>; 64] = [None; 64];
        let mut n_glows = 0;
        // Her feet on the canvas at this alpha, and her draw key: where a lesson's light and
        // motes gather.
        let mut me_feet = None;
        let me_key = self.units.iter().find(|u| u.me).map(|u| UNIT_KEY | u.id);
        for u in &self.units {
            let (dx, dy) = (i64::from(u.cur.0 - u.prev.0), i64::from(u.cur.1 - u.prev.1));
            let (fx, fy) = if dx * dx + dy * dy > SNAP_FX * SNAP_FX {
                u.cur
            } else {
                (u.prev.0 + ((dx * a) >> 8) as i32, u.prev.1 + ((dy * a) >> 8) as i32)
            };
            let (sx, sy) = ((fx >> FX_TO_CANVAS) - cam.0, (fy >> FX_TO_CANVAS) - cam.1);
            // What she stands on, if the terrain raises it (a step, a dais): what she holds is
            // that much higher, so its light is over the step, not in it (§1.7). Under a house's
            // eaves the roof drawn at her feet stands in front of her, on the house: she stands
            // on the ground behind it, and her light falls there, not on the roof.
            let lift = light_lift(
                u.under,
                drawn_height(
                    &self.chunks,
                    &self.frame.layers,
                    self.zone_cells,
                    (fx >> FX_TO_CANVAS, fy >> FX_TO_CANVAS),
                ),
            );
            // A creature's glow at its heart.
            if let Some((radius, colour)) = u.glow.filter(|_| n_glows < glows.len()) {
                let height = 14 + lift;
                glows[n_glows] = Some(Light {
                    pos: (sx, sy + rows_up(lift)),
                    height: height as u8,
                    colour,
                    radius,
                    size: 8,
                    casts: false,
                    kind: LightKind::Point,
                    holder: Some(UNIT_KEY | u.id),
                });
                n_glows += 1;
            }
            // A person shows its walk, breathe or dead frame (ART.md §4); a stand-in walks with
            // a one-px bob.
            let mut cast_glow = None;
            // A spell learned (§2.1): standing, she turns to us and holds her hands out.
            let lesson = if u.me { self.lessons.pose() } else { None };
            let face = if lesson.is_some() { Face8::South } else { u.face };
            if u.me {
                me_feet = Some((sx, sy));
            }
            // Where her lantern's glass is, from her feet, when she holds it: `(dx, rows up)`.
            let mut glass: Option<((i32, i32), Face8)> = None;
            let (look, mirror, bob) = match (u.person, u.creature) {
                (Some(set), _) => {
                    // A blow or a spell under way plays its three beats; a blow taken, its hurt.
                    // A cast building holds her hands out on its second beat until it lands.
                    let building = u.build.filter(|_| !u.dead).map(|_| people::Act::Cast(people::ACT_TICKS));
                    let act = lesson.map(|(_, t)| people::Act::Cast(t)).or(building).or_else(|| {
                        u.struck
                            .and_then(|(t, spell)| {
                                let t = self.tick.wrapping_sub(t);
                                (t < 3 * people::ACT_TICKS).then(|| {
                                    match jane_data::catalog().combat.spell(spell).anim {
                                        jane_data::CastAnim::Cast => people::Act::Cast(t),
                                        _ => people::Act::Attack(t),
                                    }
                                })
                            })
                            // Winding up: the raise held, its first beat, until the blow lands.
                            .or_else(|| {
                                u.raise.map(|spell| match jane_data::catalog().combat.spell(spell).anim {
                                    jane_data::CastAnim::Cast => people::Act::Cast(0),
                                    _ => people::Act::Attack(0),
                                })
                            })
                    });
                    let hurt = self.tick < u.hurt_until;
                    // Hands out on the cast's second beat: the school's light between them.
                    if let Some((school, t)) = lesson {
                        if t / people::ACT_TICKS == 1 {
                            cast_glow = Some(school);
                        }
                    } else if let Some((frac, school, _)) = u.build.filter(|_| !u.dead) {
                        // The light between her hands only in the cast's last quarter: before that the
                        // motes gathering are the tell, and she and what she faces stay readable.
                        if frac >= 192 {
                            cast_glow = Some(school);
                        }
                    } else if let (Some(people::Act::Cast(t)), Some((_, spell))) = (act, u.struck) {
                        if t / people::ACT_TICKS == 1 && !u.dead {
                            cast_glow = Some(jane_data::catalog().combat.spell(spell).school);
                        }
                    }
                    let since = |t: Option<u32>| t.map_or(u32::MAX, |t| self.tick.wrapping_sub(t));
                    let pose = people::Pose {
                        dead: u.dead,
                        act,
                        hurt,
                        still: u.still,
                        turned: since(u.turned.map(|t| t.0)),
                        from: u.turned.map_or(face, |t| t.1),
                        landed: since(u.landed),
                        task: u.task.filter(|_| lesson.is_none()),
                        ..people::Pose::plain(face, u.anim, self.tick, u.id)
                    };
                    // Her lantern, lit, in her hand, and its light from its glass (§1.7).
                    let held =
                        (u.player && lantern).then(|| self.people.holding_lantern(set, pose, &self.atlas)).flatten();
                    if let Some(h) = held {
                        glass = Some((h.glass, face));
                        (h.look, h.mirror, 0)
                    } else {
                        let (look, mirror) = self.people.frame(set, pose);
                        (look, mirror, 0)
                    }
                }
                // A creature trots, sits a while after it stops, and strikes in three beats.
                (None, Some(set)) => {
                    // Winding up a blow, it holds its first beat (the raise) until the blow lands.
                    let attack = u
                        .struck
                        .map(|(t, _)| self.tick.wrapping_sub(t))
                        .filter(|&t| t < 3 * creatures::ATTACK_TICKS)
                        .or(u.raise.map(|_| 0));
                    let pose = creatures::Pose {
                        facing: u.face,
                        anim: u.anim,
                        still: u.still,
                        tick: self.tick,
                        dead: u.dead,
                        attack,
                        id: u.id,
                    };
                    let (look, mirror) = self.creatures.frame(set, pose);
                    (look, mirror, 0)
                }
                (None, None) => (u.look, u.mirror, i32::from((u.anim / 9) & 1 == 1)),
            };
            // A player without a person's look (a stand-in) holds it at her side.
            if u.player && lantern && !u.dead && glass.is_none() && u.person.is_none() {
                glass = Some(((if mirror { -9 } else { 9 }, 13), u.face));
            }
            if let Some(((dx, up), facing)) = glass.filter(|_| n_glows < glows.len()) {
                // It hangs in front of her facing the viewer, behind her facing away, at her side
                // otherwise: its ground point is under its glass, and it shines from its glass's
                // height, but never lower than a low flame's (`LANTERN_LOW`): a light at her
                // knee only grazes the ground.
                let ahead = match facing {
                    Face8::South | Face8::SouthEast | Face8::SouthWest => 2,
                    Face8::North | Face8::NorthEast | Face8::NorthWest => -2,
                    Face8::East | Face8::West => 1,
                };
                let height = (height_of_rows((up + ahead).max(1)).max(LANTERN_LOW) + lift).clamp(1, 255);
                let k = flicker(self.tick, u.id, 80, FLICKER_RATE);
                glows[n_glows] = Some(Light {
                    pos: (sx + dx, sy + ahead + rows_up(lift)),
                    height: height as u8,
                    colour: scale(LANTERN, k),
                    radius: LANTERN_RADIUS,
                    size: 3,
                    casts: true,
                    kind: LightKind::Point,
                    // Hers: resolved to her sprite once the list is sorted.
                    holder: Some(UNIT_KEY | u.id),
                });
                n_glows += 1;
            }
            let r = self.atlas.get(look);
            let (x, y) = (sx - i32::from(r.ax), sy - i32::from(r.ay) - bob);
            if !(on_canvas(x, y, r.src.w, r.src.h) || !u.dead && in_band(x, y, r.src.w, r.src.h)) {
                continue;
            }
            // Over the head: the top of what it stands, a few rows clear. An emote over it puts
            // its quest mark by while it shows.
            let head = sy - rows_up(i32::from(r.top.max(16))) - bob;
            let emote = self.emotes.iter().find(|e| e.0 == u.id).filter(|_| !u.dead);
            if let Some(&(_, emote, at)) = emote {
                let age = self.tick.wrapping_sub(at);
                self.emote_marks.push(EmoteMarker { id: u.id, x: sx, y: head - MARK_CLEAR + 4, emote, age });
            } else if let Some(mark) = u.mark.filter(|_| !u.dead) {
                self.marks.push(QuestMarker { id: u.id, x: sx, y: head - MARK_CLEAR, mark });
            }
            // Her side of a fight: the ring under her target, a bar under anyone casting.
            if let Some(t) =
                self.aimed.filter(|t| matches!(t.to, jane_sim::input::TargetRef::Unit(id) if id.get() == u.id))
            {
                let head = sy - rows_up(i32::from(r.top.max(16))) - bob;
                self.fight.target =
                    Some(TargetMarker { x: sx, y: sy, half: u.ring, head, hostile: t.hostile, name: t.name, hp: t.hp });
            }
            if let Some((frac, school, _)) = u.build.filter(|_| !u.dead) {
                self.fight.bars.push(CastBar { x: sx, y: sy, frac, school });
            }
            let tint = if u.dead && (u.person.is_some() || u.creature.is_some()) {
                Tint::None
            } else if u.dead {
                Tint::Ghost(160)
            } else if self.tick < u.hurt_until && u.hurt_until - self.tick > HURT_TICKS - FLASH_TICKS {
                Tint::Flash(128)
            } else {
                Tint::None
            };
            let caster = (!u.dead).then(|| Caster {
                sprite: 0,
                foot: clamp16(sx, sy),
                height: r.top.max(1),
                depth: 5,
                ..Caster::default()
            });
            let mut sp = sprite(r, x, y, Flags { mirror, tint, bend: crate::frame::Bend::NONE });
            if behind(fx >> FX_TO_CANVAS, fy >> FX_TO_CANVAS) {
                // A player shows through what hides her.
                sp.foot = Some(Foot { y: clamp16(0, sy).1, see: u.player });
            }
            if u.seen && on_canvas(x, y, r.src.w, r.src.h) {
                self.seen.push(SpriteCmd {
                    flags: Flags { mirror, tint: Tint::Seen, bend: crate::frame::Bend::NONE },
                    foot: None,
                    ..sp
                });
            }
            self.standing.push(DrawCmd { y: sy, key: UNIT_KEY | u.id, sprite: sp, caster });
            // A serpent's body along its trail, tail first, a segment at every point, each
            // standing where it lies so it sorts among what is round it.
            if let (Some(t), Some(segs)) = (
                self.trails.iter().find(|(id, _)| *id == u.id).map(|(_, t)| t),
                u.creature.and_then(|s| self.creatures.segments(s)),
            ) {
                let n = t.len().max(1);
                for (k, &(px, py)) in t.iter().enumerate().skip(2).rev() {
                    let seg = segs[(k * segs.len() / n).min(segs.len() - 1)];
                    let r = self.atlas.get(seg);
                    let (qx, qy) = ((px >> FX_TO_CANVAS) - cam.0, (py >> FX_TO_CANVAS) - cam.1);
                    let (x, y) = (qx - i32::from(r.ax), qy - i32::from(r.ay));
                    if in_band(x, y, r.src.w, r.src.h) {
                        let mut sp =
                            sprite(r, x, y, Flags { mirror: false, tint: Tint::None, bend: crate::frame::Bend::NONE });
                        if behind(px >> FX_TO_CANVAS, py >> FX_TO_CANVAS) {
                            sp.foot = Some(Foot { y: clamp16(0, qy).1, see: false });
                        }
                        self.standing.push(DrawCmd {
                            y: qy,
                            key: UNIT_KEY | u.id,
                            sprite: sp,
                            // Each coil throws its own shadow, footed where it lies.
                            caster: (!u.dead).then(|| Caster {
                                sprite: 0,
                                foot: clamp16(qx, qy),
                                height: r.top.max(1),
                                depth: 4,
                                ..Caster::default()
                            }),
                        });
                    }
                }
            }
            if let Some(school) = cast_glow {
                // The light gathered between her hands: in front of her facing the viewer or to
                // the side, behind her facing away; it lights what is round it.
                let (dx, dy, ahead) = match face {
                    Face8::East => (9, -21, 1),
                    Face8::West => (-9, -21, 1),
                    Face8::South => (0, -18, 1),
                    Face8::North => (0, -19, -1),
                    Face8::SouthEast => (5, -18, 1),
                    Face8::SouthWest => (-5, -18, 1),
                    Face8::NorthEast => (5, -19, -1),
                    Face8::NorthWest => (-5, -19, -1),
                };
                let g = self.atlas.get(self.people.glow(school));
                let (gx, gy) = (sx + dx - i32::from(g.src.w) / 2, sy + dy - i32::from(g.src.h) / 2);
                self.standing.push(DrawCmd {
                    y: sy + ahead,
                    key: 0x4000_0000 | u.id,
                    sprite: sprite(g, gx, gy, Flags::default()),
                    caster: None,
                });
                if n_glows < glows.len() {
                    let c = jane_art::palette::rgb(jane_art::palette::Ramp::at(
                        jane_art::fx::school_ramp(school),
                        jane_art::palette::Tone::Light,
                    ));
                    glows[n_glows] = Some(Light {
                        pos: (sx + dx, sy),
                        height: (-dy).clamp(0, 255) as u8,
                        colour: c,
                        // A cast building gathers: its light grows from a spark to the full glow.
                        radius: u.build.filter(|_| !u.dead).map_or(64, |(f, _, _)| 20 + f * 60 / 256),
                        size: 4,
                        casts: false,
                        kind: LightKind::Point,
                        holder: Some(UNIT_KEY | u.id),
                    });
                    n_glows += 1;
                }
            }
        }
        // The ambient life (ART-PLAN M1): critters on the ground and in the air, crows and a cat on
        // what the terrain raises, smoke over the chimneys, pads on the water. No footprint, no
        // shadow: they stand in the draw lists and nothing else.
        for a in self.ambient.actors() {
            let r = self.atlas.get(a.look);
            let (sx, sy) = (a.x - cam.0, a.y - cam.1);
            // On a fence or a wall: its feet on the terrain's top in its column.
            let lift = if a.perch {
                (a.y - 24..a.y)
                    .find(|&yy| {
                        let h = drawn_height(&self.chunks, &self.frame.layers, self.zone_cells, (a.x, yy));
                        h > i32::from(Foot::RELIEF) && yy + rows_up(h) >= a.y - 6
                    })
                    .map_or(0, |top| a.y - top - 1)
            } else {
                0
            };
            let (x, y) = (sx - i32::from(r.ax), sy - a.up - lift - i32::from(r.ay));
            if !on_canvas(x, y, r.src.w, r.src.h) {
                continue;
            }
            let tint = if a.alpha < 255 { Tint::Ghost(a.alpha) } else { Tint::None };
            let mut sp = sprite(r, x, y, Flags { mirror: a.mirror, tint, bend: crate::frame::Bend::NONE });
            if !a.perch && !a.flat && a.up < 4 && behind(a.x, a.y) {
                sp.foot = Some(Foot { y: clamp16(0, sy).1, see: false });
            }
            let cmd = DrawCmd { y: sy, key: ambient::AMBIENT_KEY | a.key, sprite: sp, caster: None };
            if a.flat { self.ground.push(cmd) } else { self.standing.push(cmd) }
        }
        let (sort_top, rows) = (band.top + SORT_PAST, (ch + band.top + band.bottom + 2 * SORT_PAST) as u32);
        let block_range = self.chunk_range(cam, band);
        let window_range = self.chunk_range(cam, Margins::uniform(CAST_MARGIN));
        let f = &mut self.frame;
        let g0 = f.sprites.len();
        f.sprites.extend(self.ground.sort(-sort_top, rows).iter().map(|c| c.sprite));
        let ground = Span::since(g0, f.sprites.len());
        let s0 = f.sprites.len();
        self.holders.clear();
        for c in self.standing.sort(-sort_top, rows) {
            if let Some(k) = c.caster {
                self.holders.push((c.key, f.sprites.len() as u32));
                f.casters.push(Caster { sprite: f.sprites.len() as u32, ..k });
            }
            f.sprites.push(c.sprite);
        }
        self.holders.sort_unstable();
        let standing = Span::since(s0, f.sprites.len());
        // Whoever is seen through what stands in front of them, drawn again over it all.
        let v0 = f.sprites.len();
        f.sprites.extend_from_slice(&self.seen);
        let seen = Span::since(v0, f.sprites.len());
        let casters = Span::since(0, f.casters.len());
        // What the terrain stands, from every painted chunk under the canvas and T2's guard band
        // round it, each block held to that band: the same ground T2's field casts from. A block
        // can stand under its own chunk (its tallest px's rows), so the chunk row over the band
        // is read too.
        if let Some((cx0, cy0, cx1, cy1)) = block_range {
            let (lo, hi) = ((-band.left, -band.top), (cw + band.right, ch + band.bottom));
            for cy in (cy0 - 1).max(0)..=cy1 {
                for cx in cx0..=cx1 {
                    let Some((slot, _)) = self.chunks.find(ChunkId { cx: cx as u16, cy: cy as u16 }) else {
                        continue;
                    };
                    let (ox, oy) = (cx * CHUNK_PX - cam.0, cy * CHUNK_PX - cam.1);
                    for b in self.terrain.blocks(slot) {
                        let (x0, x1) = ((ox + i32::from(b.x0)).max(lo.0), (ox + i32::from(b.x1)).min(hi.0));
                        let (y0, y1) = ((oy + i32::from(b.y0)).max(lo.1), (oy + i32::from(b.y1)).min(hi.1));
                        if x0 < x1 && y0 < y1 {
                            f.blocks.push(Block {
                                x0: x0 as i16,
                                y0: y0 as i16,
                                x1: x1 as i16,
                                y1: y1 as i16,
                                height: b.height,
                                lo: b.lo,
                                fence: b.fence,
                            });
                        }
                    }
                }
            }
        }
        let blocks = Span::since(0, f.blocks.len());

        // The lights: every prop light the view says shows, the glows and her lantern, those
        // whose reach touches the canvas, the nearest first up to the tier's count.
        for l in &self.lights {
            let (x, y) = (l.x - cam.0, l.y - cam.1);
            let r = i32::from(l.radius);
            if x + r < 0 || y - i32::from(l.height) - r > ch || x - r > cw || y + r < 0 {
                continue;
            }
            let k = flicker(self.tick, l.id, l.dip, FLICKER_RATE);
            f.lights.push(Light {
                pos: (x, y),
                height: l.height,
                colour: scale(l.colour, k),
                radius: l.radius,
                size: l.size,
                casts: true,
                kind: LightKind::Point,
                // The prop that carries it: its post, its bracket, its cage.
                holder: Some(l.id),
            });
        }
        // Each lit window of the terrain: a small warm pool on the ground in front of it, and
        // what stands in it throws its shadow away from the window (§1.7). Low and short, they
        // cast by the one nearest-first rule with every other light. Lit as her lantern is, when
        // the flat light is low.
        if lantern {
            if let Some((cx0, cy0, cx1, cy1)) = window_range {
                for cy in cy0..=cy1 {
                    for cx in cx0..=cx1 {
                        let Some((slot, _)) = self.chunks.find(ChunkId { cx: cx as u16, cy: cy as u16 }) else {
                            continue;
                        };
                        let (ox, oy) = (cx * CHUNK_PX - cam.0, cy * CHUNK_PX - cam.1);
                        for w in self.terrain.windows(slot) {
                            let (x, y) = (ox + i32::from(w.x), oy + i32::from(w.y));
                            let r = i32::from(WINDOW_RADIUS);
                            if x + r < 0 || y - r - 64 > ch || x - r > cw || y + r < 0 {
                                continue;
                            }
                            f.lights.push(Light {
                                pos: (x, y),
                                height: w.height,
                                colour: w.colour.map(|c| (u32::from(c) * 7 / 8) as u8),
                                radius: WINDOW_RADIUS,
                                size: 5,
                                casts: true,
                                // Out of the window, not back onto its own wall: a spot turned
                                // to the viewer, the face it is set in behind it.
                                kind: LightKind::Spot { dir: jane_core::Angle::SOUTH, cone: WINDOW_CONE },
                                holder: None,
                            });
                        }
                    }
                }
            }
        }
        f.lights.extend(glows[..n_glows].iter().flatten());
        // The effects' lights: a bolt lights the wall it passes (§2); a lesson's between her hands.
        self.fx.lights(f, cam, alpha);
        self.lessons.lights(f, me_feet, me_key);
        // A light never shadows what holds it: each holder's key to its sprite, or none.
        let holders = &self.holders;
        for l in &mut f.lights {
            l.holder = l.holder.and_then(|k| holders.binary_search_by_key(&k, |h| h.0).ok().map(|i| holders[i].1));
        }
        // The tier's counts, or fewer where the `max_lights` and `shadows` rows are turned down:
        // the nearest casting lights by one rule on every tier.
        let rows = self.atmos.features;
        let most = max_lights(f.tier).min(usize::from(rows.max_lights));
        pick_lights(&mut f.lights, (cw / 2, ch / 2), most, usize::from(rows.shadows));
        let points = Span::since(0, f.lights.len());

        // The sky as the weather has it: rain dims and cools, lightning flashes (§1.9).
        // A sun too faint to cast (`light::FAINTEST`) casts on no tier.
        let mut sky = self.atmos.light(&self.sky);
        if let Some(s) = sky.sun.as_mut().filter(|s| !s.casts()) {
            s.strength = 0;
        }
        let sky = &sky;
        f.passes.push(Pass::Sprites { layer: Depth::Ground, cmds: ground });
        // Silhouette sun shadows under the standing things, where the tier has no shadow maps:
        // from the sun, the moon or the afterglow, as faint as it is.
        if f.tier <= Tier::T1
            && rows.silhouettes
            && let Some(sun) = sky.sun.filter(Directional::casts)
            && (casters.len > 0 || blocks.len > 0)
        {
            let shade = shadow::shade_at(sky.shade, sun.strength);
            f.passes.push(Pass::Silhouettes { sun, shade, casters, blocks });
        }
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: standing });
        if seen.len > 0 {
            f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: seen });
        }
        f.passes.push(Pass::Weather(self.atmos.atmos()));
        // Below T2 the parts that do not glow go under the light, so the lamps light the rain.
        self.fx.draw_under_light(f, cam, alpha);
        self.ambient.draw_parts(f, cam, true, None);
        // The light pass: on T0 left out when the multiply would change nothing (day is free),
        // unless a light shows: its flame's glow and its faint pool are this pass's, and a fire
        // at noon drawn without them read as out (the owner, 2026-10-01).
        if f.tier > Tier::T0 || sky.ambient.iter().any(|&c| c < 254) || points.len > 0 {
            f.passes.push(Pass::Lights { ambient: sky.ambient, fill: sky.fill, sun: sky.sun, points, casters, blocks });
        }
        // Over what is lit: the marks and splashes on the ground, the fog, the effects in the
        // air, the rain.
        self.fx.draw_ground(f, cam, alpha, sky);
        self.ambient.draw_parts(f, cam, false, Some(sky));
        self.atmos.draw_fog(f, cam, sky);
        // Past the screen's edge: crows over the camps, the dead lamps' glass, the Hoar Stone
        // out of the fog.
        let fog_at_stone = self.cues.stone().map_or(0, |((x, y), _)| self.atmos.fog_at(x, y - CELL));
        let her = me_feet.map(|(x, y)| (x + cam.0, y + cam.1));
        let t256 = u64::from(self.tick) * 256 + u64::from(alpha);
        let moon = self.atmos.moon_up();
        self.cues.draw(f, &self.atlas, cam, t256, sky, her, self.hour, moon, fog_at_stone);
        self.fx.draw_air(f, cam, alpha, sky);
        self.lessons.draw(f, cam, alpha, me_feet);
        // The grade (§1.3 `grade`): the same on every tier, so a frame from any of them is the
        // same hour and mood, the bloom with it where the `bloom` row is on (every tier since
        // 2026-09-27).
        // The `grade` row off leaves the lit frame as it is; the `bloom` row off, the glow.
        let post = if rows.grade { sky.post } else { Post { bloom: sky.post.bloom, ..Post::NONE } };
        let post = Post { bloom: if rows.bloom { post.bloom } else { 0 }, ..post };
        // T0's flat light past 255 (T2's noon is brighter than the art as drawn) is exposure.
        let post = if f.tier == Tier::T0 {
            let e = u32::from(post.exposure) * u32::from(sky.t0_gain) / 256;
            Post { exposure: e.min(255) as u8, ..post }
        } else {
            post
        };
        // A lesson's hush drains a little colour and light (§2.1).
        let post = self.lessons.grade(post);
        if post != Post::NONE {
            f.passes.push(Pass::Post(post));
        }
        &self.frame
    }
}

/// The houses of the zone someone lives in, by their block's corner: those a door of which a
/// schedule names (someone goes in there for the night). Their chimneys smoke; an empty house's
/// does not (ART-PLAN M1).
fn lived_houses(view: &View<'_>, houses: &jane_art::terrain::houses::Houses) -> Vec<(i32, i32)> {
    let cat = jane_data::catalog();
    let mut names: Vec<jane_core::Sym> = cat
        .combat
        .units
        .iter()
        .flat_map(|u| u.schedule.iter())
        .filter_map(|r| match r.slot {
            jane_data::ScheduleSlot::Inside(n) => Some(jane_sim::sym::of_name(n)),
            _ => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut lived: Vec<(i32, i32)> = view
        .props()
        .filter(|p| names.binary_search(&p.key).is_ok())
        .filter_map(|p| houses.at(i32::from(p.cell.x), i32::from(p.cell.y)).map(|h| (h.rect.x, h.rect.y)))
        .collect();
    lived.sort_unstable();
    lived.dedup();
    lived
}

/// How far a flora sprite leans, `-3..=3` levels of its bend (ART-PLAN M2 and §9, the owner's
/// 2026-10-03 notes: smaller steps, faster, never in step), and the slow gust over it, `0..=65536`:
/// a sine of its own pace (`base`, its class's phase of 65536, advanced by the presenter), its
/// phase put back by a gust that travels across the county down the
/// wind (neighbours lean in a ripple, one after the other) and nudged by its own hash, its reach
/// swelling and easing with the slower gust, one way stronger than the other with the wind.
/// Integer; the lean changes a level at a time.
fn sway(class: crate::terrain::SwayClass, x: i32, y: i32, own: u32, base: u32, tick: u32, wind: i32) -> (i32, i32) {
    use jane_core::Angle;
    use jane_core::angle::sin_q15;
    if class == crate::terrain::SwayClass::Still {
        return (0, 0);
    }
    // The ripple: a crest every 384 px across, travelling with the wind's side.
    let along = if wind < 0 { -x } else { x };
    let wave = (i64::from(along + y / 2) * 65536 / 384) as u32;
    let jitter = own % 9000;
    let phase = base.wrapping_sub(wave).wrapping_add(jitter);
    let s = i64::from(sin_q15(Angle(phase as u16)).0);
    // The slow gust: a reach of a third to the whole, rolling over the county in about 20 s.
    let gust = i64::from(sin_q15(Angle(tick.wrapping_mul(55).wrapping_sub((along * 40 + y * 20) as u32) as u16)).0);
    let reach = 2 * 32768 + gust; // 32768..=98304, of 98304
    let lean = s * reach / 98304 * 3;
    // Leaning more with the wind than against it: the rest a px downwind in a blow.
    let bias = i64::from((wind / 16).clamp(-1, 1)) * 32768;
    let level = ((lean + bias) * 2 + 32768 * (lean + bias).signum()) / 65536;
    let reach = crate::terrain::SWAY_REACH;
    (level.clamp(-i64::from(reach), i64::from(reach)) as i32, (gust + 32768) as i32)
}

/// A loose leaf cluster's flutter this tick (ART-PLAN §9), its hash `h`: none most of the time;
/// now and then, oftener and longer in a gust and a wind, a few ticks tipped a px out from the
/// crown (and a px up or down), then a few turned to show its pale side. Each cluster on its own
/// beat, so a crown's edge flickers and never all at once. `(out, down, lit)`.
fn flutter(h: u32, tick: u32, gust: i32, wind: i32) -> Option<(i32, i32, bool)> {
    let period = 40 + h % 23;
    let t = tick.wrapping_add(h >> 8) % period;
    let cycle = tick.wrapping_add(h >> 8) / period;
    // One beat in three or so rests; a gust and a wind make the beat longer.
    if jane_core::hash::mix32(h ^ cycle) % 3 == 0 {
        return None;
    }
    let duty = 2 + (gust.clamp(0, 65536) * 6 / 65536) as u32 + (wind.unsigned_abs() / 8).min(4);
    if t >= 2 * duty {
        return None;
    }
    let down = if (h >> 4) & 1 == 0 { 1 } else { -1 };
    Some(if t < duty { (1, down, false) } else { (0, 0, true) })
}

/// Ticks a loose leaf of a cluster, hash `h`, has been falling, if one is (ART-PLAN §9): now and
/// then (about one cluster in eight a window of ten seconds) a leaf lets go, and falls for
/// [`crate::fx::LOOSE_LIFE`] ticks.
fn let_go(h: u32, tick: u32) -> Option<u32> {
    const WINDOW: u32 = 600;
    let at = tick.wrapping_add(h >> 12);
    let w = jane_core::hash::mix32(h ^ (at / WINDOW).wrapping_mul(0x85eb_ca6b));
    if w % 8 != 0 {
        return None;
    }
    let start = (w >> 8) % (WINDOW - crate::fx::LOOSE_LIFE);
    (at % WINDOW).checked_sub(start).filter(|&age| age < crate::fx::LOOSE_LIFE)
}

/// The terrain's drawn height at zone canvas px `(x, y)`, 0 where no chunk is painted.
fn drawn_height(
    chunks: &ChunkCache,
    layers: &[crate::frame::ChunkLayers],
    cells: (u32, u32),
    (x, y): (i32, i32),
) -> i32 {
    let (zw, zh) = (cells.0 as i32 * CELL, cells.1 as i32 * CELL);
    if x < 0 || y < 0 || x >= zw || y >= zh {
        return 0;
    }
    let id = ChunkId { cx: (x / CHUNK_PX) as u16, cy: (y / CHUNK_PX) as u16 };
    let Some(l) = chunks.find(id).and_then(|(s, _)| layers.get(usize::from(s))).filter(|l| l.has_height()) else {
        return 0;
    };
    i32::from(l.height[((y % CHUNK_PX) * CHUNK_PX + x % CHUNK_PX) as usize])
}

/// How much a unit's lights are raised by the terrain drawn `drawn` px up at its feet (§1.7): a
/// step or a dais it stands on raises them (by `MAX_LIFT` at most); the ground's relief does not,
/// nor does a roof drawn over it where it stands `under` the eaves (the roof is in front of it,
/// on the house, and its light falls on the ground behind the house).
fn light_lift(under: bool, drawn: i32) -> i32 {
    if under || drawn <= shadow::RELIEF { 0 } else { drawn.min(MAX_LIFT) }
}

/// `0xRRGGBB` as bytes.
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

/// Each small thing whose footprint lies on a top (a bowl on a table) stands on it: drawn up on
/// the top, as far back on it as its footprint is back on the table's, and sorted just after the
/// table so the table never covers it. Tops are few: each thing is tried against those alone.
fn stand_on_tops(props: &mut [PropRec]) {
    let tops: Vec<Top> = props.iter().filter_map(|t| t.surface.map(|s| (t.x, t.y, t.w, t.h, s))).collect();
    if tops.is_empty() {
        return;
    }
    for p in props.iter_mut().filter(|p| !p.flat && p.surface.is_none()) {
        let Some(&(_, ty, _, th, (front, depth))) = tops
            .iter()
            .find(|&&(tx, ty, tw, th, _)| p.x >= tx && p.y >= ty && p.x + p.w <= tx + tw && p.y + p.h <= ty + th)
        else {
            continue;
        };
        // Its foot's place on the top: a few rows in from the top's front edge for the table's
        // front row, toward its back edge as the footprint goes back (`back` rows of floor
        // behind the table's foot, which its own foot already stands up).
        let back = ty + th - (p.y + p.h);
        p.lift = front + SURFACE_INSET + back * (depth - SURFACE_INSET) / th.max(1) - back;
        p.sort_foot = Some(ty + th + 1);
    }
}

/// A top's footprint `(x, y, w, h)` and its `(front, depth)` rows.
type Top = (i32, i32, i32, i32, (i32, i32));

/// Rows in from a top's front edge that a thing on it stands.
const SURFACE_INSET: i32 = 3;

/// The sim facing nearest a shown one: a diagonal is the axis she walks nearer (down or up
/// the screen, since that is how she is seen from).
fn nearest_axis(f: Face8) -> Facing {
    match f {
        Face8::South | Face8::SouthEast | Face8::SouthWest => Facing::South,
        Face8::North | Face8::NorthEast | Face8::NorthWest => Facing::North,
        Face8::East => Facing::East,
        Face8::West => Facing::West,
    }
}

#[cfg(test)]
mod tests {
    use jane_sim::input::{InputFrame, StepInput};
    use jane_sim::state::{LootState, NightState, Prop};
    use jane_sim::{Seat, Sim};

    use super::*;

    /// The atlas's bytes, every layer of every page, for the low-RAM budget (ART-PLAN §9): run
    /// with `--nocapture` to read them.
    #[test]
    #[ignore = "builds every tier's art; slow in debug"]
    fn atlas_bytes() {
        for tier in [Tier::T0, Tier::T2] {
            let p = Present::new(tier);
            let a = p.atlas();
            let bytes: usize = a
                .pages
                .iter()
                .map(|pg| {
                    pg.albedo.len() * 2
                        + pg.normal.len() * 2
                        + pg.emissive.len() * 2
                        + pg.height.len()
                        + pg.glow.len() * 8
                })
                .sum();
            let rows: usize = a.pages.iter().map(|pg| usize::from(pg.h)).sum();
            println!("atlas {tier:?}: {} pages, {rows} rows, {bytes} bytes", a.pages.len());
        }
    }

    #[test]
    fn foliage_sways_a_frame_at_a_time_in_a_cycle_of_seconds_and_never_in_step() {
        use crate::terrain::SwayClass;
        for class in [SwayClass::Crown, SwayClass::Bush, SwayClass::Reed] {
            let p = class.period();
            assert!((90..=180).contains(&p), "{class:?}: {p} ticks");
            let per = ((1u64 << 32) / u64::from(p)) as u32;
            let at = |x: i32, t: u32| {
                sway(class, x, 40, jane_core::hash::mix32(x as u32), per.wrapping_mul(t) >> 16, t, 0).0
            };
            // A level at a time: the top moves a px at most, never a jump.
            for t in 0..600 {
                assert!(at(100, t).abs_diff(at(100, t + 1)) <= 1, "{class:?} jumps at {t}");
            }
            // It goes somewhere: more than three leans in a cycle.
            let mut seen: Vec<i32> = (0..p).map(|t| at(100, t)).collect();
            seen.sort_unstable();
            seen.dedup();
            assert!(seen.len() >= 4, "{class:?}: {seen:?}");
            // A row of neighbours a cell apart never steps all on one tick.
            for t in 0..p {
                let changed = (0..8).filter(|k| at(100 + 16 * k, t) != at(100 + 16 * k, t + 1)).count();
                assert!(changed < 8, "{class:?}: all eight step at {t}");
            }
        }
        assert_eq!(sway(SwayClass::Still, 5, 5, 9, 77, 77, 0).0, 0);
    }

    /// The wind changes how fast a plant sways, never where it is in the sway: its phase runs on
    /// from where it was, so a change between two ticks moves no plant more than a level (the
    /// owner, 2026-10-06: "swaying doesn't seem to loop perfectly").
    #[test]
    fn a_change_in_the_wind_never_jumps_a_plant() {
        use crate::terrain::SwayClass;
        let pace = |class: SwayClass, wind: i32| {
            (class.period() as i32 * 32 / (32 + wind.unsigned_abs().min(32) as i32)).max(40) as u32
        };
        // Pairs of winds the same way and the same side of the downwind rest.
        for (w0, w1) in [(0, 15), (15, 0), (1, 12), (16, 40), (40, 16), (-4, -15), (20, 31)] {
            for class in [SwayClass::Crown, SwayClass::Bush, SwayClass::Reed] {
                let mut acc = 0u32;
                for t in 0..400u32 {
                    let wind = if t < 200 { w0 } else { w1 };
                    let before = acc;
                    acc = acc.wrapping_add(((1u64 << 32) / u64::from(pace(class, wind))) as u32);
                    for x in (0..640).step_by(37) {
                        let own = jane_core::hash::mix32(x as u32);
                        let prev_wind = if t <= 200 { w0 } else { w1 };
                        let a = sway(class, x, 60, own, before >> 16, t, prev_wind).0;
                        let b = sway(class, x, 60, own, acc >> 16, t + 1, wind).0;
                        assert!(a.abs_diff(b) <= 1, "{class:?} at x {x} jumps at {t} ({w0} to {w1})");
                    }
                }
            }
        }
    }

    /// A crown's loose leaves (ART-PLAN §9): a cluster flutters now and then, a few ticks at a
    /// time, a crown's three never all on one tick; a leaf lets go seldom, and falls its life.
    #[test]
    fn leaves_flutter_on_their_own_beats_and_the_odd_one_lets_go() {
        let hs: Vec<u32> =
            (0..3).map(|k| jane_core::hash::mix32(0x4d ^ (k + 1u32).wrapping_mul(0x9e37_79b9))).collect();
        let mut on = 0;
        for t in 0..6000 {
            let n = hs.iter().filter(|&&h| flutter(h, t, 32768, 4).is_some()).count();
            on += n;
            let changed = hs.iter().filter(|&&h| flutter(h, t, 32768, 4) != flutter(h, t + 1, 32768, 4)).count();
            assert!(changed < 3, "all three change at {t}");
        }
        // Busy but not frantic: a cluster stirs a tenth to a half of the time.
        assert!((1800..9000).contains(&on), "{on} cluster-ticks of 18000");
        // A gust stirs them more than a calm.
        let calm: usize = (0..6000).filter(|&t| flutter(hs[0], t, 0, 0).is_some()).count();
        let gale: usize = (0..6000).filter(|&t| flutter(hs[0], t, 65536, 40).is_some()).count();
        assert!(gale > calm, "{calm} {gale}");
        // Of a hundred clusters over a minute a few let go, each falling one life from age 0.
        let mut falls = 0;
        for k in 0..100u32 {
            let h = jane_core::hash::mix32(k);
            for t in 0..3600 {
                if let_go(h, t) == Some(0) {
                    falls += 1;
                }
                if let Some(a) = let_go(h, t + 1).filter(|&a| a > 0) {
                    assert_eq!(let_go(h, t), Some(a - 1), "a leaf falls on, tick by tick");
                }
            }
        }
        assert!((20..=150).contains(&falls), "{falls} leaves");
    }

    /// A bowl whose footprint lies on a table's stands on its top: drawn up on the top (further
    /// back for a footprint further back), sorted after the table, throwing no shadow of its own;
    /// a thing beside the table is left alone.
    #[test]
    fn a_bowl_on_a_table_stands_on_its_top() {
        let rec = |x: i32, y: i32, w: i32, h: i32, surface: Option<(i32, i32)>| PropRec {
            id: 0,
            x,
            y,
            w,
            h,
            look: 0,
            flat: false,
            flush: false,
            on_top: false,
            surface,
            lift: 0,
            sort_foot: None,
            mark: None,
            smokes: false,
        };
        let mut v = vec![
            rec(96, 176, 48, 32, Some((10, 18))),
            rec(112, 192, 32, 16, None),
            rec(112, 176, 32, 16, None),
            rec(160, 192, 32, 16, None),
        ];
        stand_on_tops(&mut v);
        let table_foot = 176 + 32;
        let (front, back, beside) = (v[1], v[2], v[3]);
        assert_eq!(front.sort_foot, Some(table_foot + 1), "after the table");
        let drawn = |p: PropRec| p.y + p.h - p.lift;
        assert_eq!(drawn(front), table_foot - 10 - SURFACE_INSET, "a few rows onto the top");
        assert!(drawn(back) < drawn(front) && drawn(back) > table_foot - 10 - 18, "further back, still on it");
        assert_eq!((beside.lift, beside.sort_foot), (0, None), "beside the table: on the floor");
    }

    /// A step or a dais she stands on raises her lights (capped), the ground's relief does not,
    /// and under a house's eaves the roof drawn at her feet raises nothing (§1.7).
    #[test]
    fn her_lights_stand_on_a_platform_and_not_on_a_roof_she_is_behind() {
        assert_eq!(light_lift(false, 24), 24, "a platform raises her light");
        assert_eq!(light_lift(false, 90), MAX_LIFT);
        assert_eq!(light_lift(false, shadow::RELIEF), 0);
        assert_eq!(light_lift(true, 57), 0, "a roof in front of her raises nothing");
    }

    /// A thing left lying is drawn as what it holds, and a drop as its item: Mrs Bettany's key
    /// was a note on the grass (the owner's first playtest). Every key item has a ground look
    /// that is not the note's, and the note comes back once the key is taken.
    #[test]
    fn a_key_on_the_ground_is_drawn_as_a_key() {
        let cat = jane_data::catalog();
        let mut sim = Sim::new_game(1, "Jane");
        let mut p = Present::new(Tier::T0);
        p.set_canvas((768, 432));
        let step = |sim: &mut Sim, p: &mut Present| {
            sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
            let events = sim.drain_events().to_vec();
            p.tick(&sim.view(Seat(0)).expect("seat 0 plays"), &events);
        };
        step(&mut sim, &mut p);
        let (zone, (cx, cy)) = {
            let v = sim.view(Seat(0)).expect("seat 0 plays");
            (v.zone(), v.body().pos.cell())
        };
        let key = cat.combat.item_id("bettany_key").expect("Mrs Bettany's key");
        let key_look = p.kit.loot_look(cat.combat.item(key).icon).expect("a key's ground look");
        let lost = cat.story.prop_id("lost_thing").expect("the lost thing row");
        let note = p.kit.look(cat.story.prop(lost).sprite, 1, props::State::default());
        assert_ne!(Some(key_look), note, "a key is not a note");
        for item in cat.combat.items.iter().filter(|i| i.id.starts_with("key_") || i.id.ends_with("_key")) {
            let look = p.kit.loot_look(item.icon).unwrap_or_else(|| panic!("{}: no ground look", item.id));
            assert_ne!(Some(look), note, "{} lies as a key", item.id);
        }
        // The lost thing, holding the key, two cells east of her; and a dropped key beyond it.
        let st = sim.state_mut();
        let id = st.next.prop();
        let key_sym = st.syms.intern("test_lost_key");
        st.zone_mut(zone).expect("her zone").props.push(Prop {
            id,
            key: key_sym,
            def: lost,
            spawn: None,
            cell: jane_core::Cell::new(cx as u16 + 2, cy as u16),
            solid: false,
            hidden: false,
            locked: false,
            used: false,
            on: false,
            loot: LootState::Left(vec![jane_core::Stack { item: key, qty: 1 }]),
            under_done: false,
            regrow: None,
            burns_until: None,
            night: NightState::AsSpawned,
        });
        let drop = st.next.drop();
        let (born, pos) = (st.tick, jane_core::Vec2::centre(cx + 4, cy));
        st.zone_mut(zone).expect("her zone").drops.push(jane_sim::state::Drop {
            id: drop,
            item: key,
            qty: 1,
            pos,
            born,
        });
        sim.rebuild_runtimes();
        step(&mut sim, &mut p);
        let rec = |p: &Present, id: u32| p.props.iter().find(|r| r.id == id).map(|r| r.look);
        assert_eq!(rec(&p, id.get()), Some(key_look), "the lost thing lies as the key it holds");
        assert_eq!(rec(&p, DROP_KEY | drop.get()), Some(key_look), "the dropped key lies as a key");
        // Taken: the lost thing is its own sprite again, and the drop is gone.
        let zs = sim.state_mut().zone_mut(zone).expect("her zone");
        let ix = zs.prop_ix(id).expect("the lost thing") as usize;
        zs.props[ix].loot = LootState::Left(Vec::new());
        zs.drops.clear();
        sim.rebuild_runtimes();
        step(&mut sim, &mut p);
        assert_eq!(rec(&p, id.get()), note);
        assert_eq!(rec(&p, DROP_KEY | drop.get()), None);
    }
}
