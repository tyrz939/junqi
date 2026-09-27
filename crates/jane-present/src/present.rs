//! `Present` (PRESENTATION.md §1.11): `tick()` once per accumulated tick, whether or not the sim
//! ran; `draw(alpha)` pure in the tick and `alpha`.
//!
//! `tick` reads the `View` and this tick's events into the presenter's own records (units with
//! their last two positions and timers, props near the view, the lights the view says are lit
//! in reach of it, the camera, the chunks under it, the sky); `draw` turns those records into a
//! `Frame` at `alpha` and reads nothing else.
//! Neither reads `GameState`, and neither writes anything but the presenter.

use jane_core::action::{CameraMode, Facing};
use jane_core::ids::SpellId;
use jane_core::num::CELL_SHIFT;
use jane_core::{Rect, ZoneId};
use jane_data::{Controller, Faction, Region};
use jane_sim::event::{Event, EventKind, events_for};
use jane_sim::ids::PropIx;
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::atmos::Atmosphere;
use crate::backend::AtlasPages;
use crate::camera::{Camera, alpha_256};
use crate::chunks::{ChunkCache, LRU, Need};
use crate::creatures::{self, Creatures};
use crate::drawlist::{DrawCmd, DrawList};
use crate::frame::{
    CANVAS_H, CANVAS_W, CELL, CHUNK_PX, Caster, ChunkCmd, ChunkId, Depth, Directional, FX_TO_CANVAS, Features, Flags,
    Frame, Light, LightKind, Pass, Post, Rgb, Span, SpriteCmd, Tier, Tint, height_of_rows, rows_up,
};
use crate::fx::Fx;
use crate::light::{Sky, flicker, lantern_lit, sky};
use crate::people::{self, People};
use crate::props::{self, Props};
use crate::shadow;
use crate::stand_in::{self, StandIns, UnitKind};
use crate::terrain::Terrain;

/// A unit moving further than this in a tick (40 sim px, `Fx`) snaps instead of sliding: travel,
/// a respawn, a hop.
const SNAP_FX: i64 = 40 * 256;
/// Cells round the view whose units and props are kept: a tall sprite standing below the view
/// still reaches into it.
const MARGIN_CELLS: i32 = 6;
/// Canvas px round the view painted ahead, so a frame between two ticks never finds a hole.
const CHUNK_AHEAD: i32 = 64;
/// Chunks the terrain painter lands in a tick at most (§1.6): the rest show their swatches, or
/// what they last had, until it reaches them. The tick a zone is entered paints all it shows.
const LAND_PER_TICK: usize = 2;
/// Canvas px round the view the draw list sorts over; things further out are culled.
const SORT_MARGIN: i32 = 96;
/// Ticks a hurt unit shows it, and the first ticks of them it flashes (§1.11).
const HURT_TICKS: u32 = 8;
const FLASH_TICKS: u32 = 4;
/// Cells past the view whose lights are read: 208 canvas px, the light reach (§1.7).
const LIGHT_CELLS: i32 = 13;
/// Her lantern (§1.7): its reach, canvas px, and its warm colour.
const LANTERN_RADIUS: u16 = 136;
const LANTERN: Rgb = [255, 190, 116];
/// The lowest a standing prop light shines from, px: a low flame lights the ground round it.
const FLAME_HEIGHT: i32 = 28;
/// How many steps a second a flame's flicker walks (§1.7).
const FLICKER_RATE: u32 = 10;
/// A unit's draw key: its id with the top bit set (a prop's is its id).
const UNIT_KEY: u32 = 0x8000_0000;

/// Lights on screen at most, and how many of them throw shadows, by tier (§1.3 `max_lights`,
/// §1.7: T1 the nearest 8, T2 the nearest 32).
pub fn max_lights(tier: Tier) -> (usize, usize) {
    match tier {
        Tier::T0 => (16, 0),
        Tier::T1 => (32, 8),
        Tier::T2 => (128, 32),
    }
}

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
    me: bool,
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
    facing: Facing,
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
    ground: DrawList,
    lights: Vec<LightRec>,
    light_scratch: Vec<PropIx>,
    /// This frame's standing casters by draw key, `(key, sprite)`, sorted: what holds a light.
    holders: Vec<(u32, u32)>,
    sky: Sky,
    /// The weather, the fog and the sky (§1.9), and the effects (§2).
    atmos: Atmosphere,
    fx: Fx,
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
        // The UI's page goes last, so no world sprite moves when it grows (PRESENTATION.md §3.1).
        let (ui_art, mut ui_page) = crate::ui::UiArt::build(atlas.pages.pages.len() as u8);
        if atlas.lit() {
            let n = ui_page.albedo.len();
            (ui_page.normal, ui_page.emissive, ui_page.height) = (vec![[128, 128]; n], vec![0; n], vec![0; n]);
        }
        atlas.pages.pages.push(ui_page);
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
            ground: DrawList::default(),
            lights: Vec::with_capacity(256),
            light_scratch: Vec::with_capacity(1024),
            holders: Vec::with_capacity(1024),
            sky: sky(12 * 7200, 0, false, 1000, Region::Lowfields),
            atmos,
            fx,
        }
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
            self.terrain.zone(view);
            self.entered = true;
            self.camera.reset();
            self.units.clear();
            self.atmos.zone(view);
            self.fx.zone(view);
        }
        self.zone_cells = view.size();
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
        self.read_units(view, area);
        self.read_props(view, area);
        self.read_lights(view, area);
        self.paint_chunks(view);
        self.against_walls();
        let (clock, day) = view.clock();
        self.sky = sky(clock, day, view.indoor(), view.ambient().0, view.region());
        self.atmos.tick(view, self.tick);
        self.fx.on_events(view, events);
        let (cw, ch) = (i32::from(self.canvas.0), i32::from(self.canvas.1));
        let cam = (self.camera.pos.0 >> FX_TO_CANVAS, self.camera.pos.1 >> FX_TO_CANVAS);
        self.fx.set_cap(self.atmos.features.max_particles);
        self.fx.tick(view, &self.atmos, self.tick, (cam.0, cam.1, cw, ch));
    }

    /// The cells the view covers this tick, with the margin.
    fn area(&self) -> Rect {
        let (x, y) = (self.camera.pos.0 >> CELL_SHIFT, self.camera.pos.1 >> CELL_SHIFT);
        let (w, h) = (i32::from(self.canvas.0) / CELL + 2, i32::from(self.canvas.1) / CELL + 2);
        Rect::new(x - MARGIN_CELLS, y - MARGIN_CELLS, w + 2 * MARGIN_CELLS, h + 2 * MARGIN_CELLS)
    }

    fn read_units(&mut self, view: &View<'_>, area: Rect) {
        let me = view.me().unit;
        let my_seat = view.seat().index() as u8;
        let cat = jane_data::catalog();
        self.units_next.clear();
        self.trails.clear();
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
            self.units_next.push(UnitRec {
                id,
                prev,
                cur,
                anim: if prev == cur { 0 } else { old.map_or(0, |o| o.anim) + 1 },
                hurt_until: old.map_or(0, |o| o.hurt_until),
                look: self.stand.unit(kind),
                mirror: u.facing == Facing::West,
                dead: !u.alive,
                me: u.id == me,
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
                facing: u.facing,
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
        props.clear();
        view.for_props_in(area, &mut self.prop_scratch, |p| {
            let d = cat.story.prop(p.def);
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
            props.push(PropRec {
                id: p.id.get(),
                x: i32::from(p.cell.x) * CELL,
                y: i32::from(p.cell.y) * CELL,
                w: i32::from(d.w) * CELL,
                h: i32::from(d.h) * CELL,
                look: kit.look(d.sprite, p.id.get(), state).unwrap_or_else(|| {
                    let look = stand.prop(d.w, d.h, d.flat, d.light.is_some());
                    // A lamp the view says is out has dark glass.
                    if d.light.is_some() && !lit { stand.unlit(look) } else { look }
                }),
                flat: d.flat,
                flush: false,
            });
        });
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

    /// A light standing inside the terrain's height field (a torch hung on a wall, whose sprite
    /// stands on the wall's top) hangs from the wall's face instead: its ground point moves to
    /// the nearest open ground within a cell and a half, toward the viewer first, and two px
    /// clear of it. A light never shadows the wall it hangs on (PRESENTATION.md §1.7), and a
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
        // stands `rows_up(h)` rows below it, its field two rows deep).
        let field = |x: i32, y: i32| -> i32 {
            let mut most = 0;
            for r in 0..=80 {
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
            if field(l.x, l.y) + 2 < i32::from(l.height) {
                continue;
            }
            'out: for d in 1..=REACH {
                for (dx, dy) in [(0, 1), (-1, 0), (1, 0), (0, -1)] {
                    let (x, y) = (l.x + dx * d, l.y + dy * d);
                    if field(x, y) <= 2 {
                        (l.x, l.y) = (x + dx * 2, y + dy * 2);
                        break 'out;
                    }
                }
            }
        }
    }

    /// Paints the chunks under the view (and a little round it) that want it: at most
    /// [`LAND_PER_TICK`] by the terrain painter, those on screen and nearest the middle first;
    /// one with nothing to show yet takes its swatches meanwhile.
    fn paint_chunks(&mut self, view: &View<'_>) {
        let cam = (self.camera.pos.0 >> FX_TO_CANVAS, self.camera.pos.1 >> FX_TO_CANVAS);
        let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, CHUNK_AHEAD) else {
            return;
        };
        let (sx0, sy0, sx1, sy1) = self.chunk_range(cam, 0).unwrap_or((cx0, cy0, cx1, cy1));
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
                });
            }
        }
    }

    /// The chunks under a view whose top-left is `cam` (canvas px), grown by `margin` px and
    /// held to the zone: inclusive `(cx0, cy0, cx1, cy1)`, or `None` for an empty zone.
    fn chunk_range(&self, cam: (i32, i32), margin: i32) -> Option<(i32, i32, i32, i32)> {
        let (zw, zh) = (self.zone_cells.0 as i32 * CELL, self.zone_cells.1 as i32 * CELL);
        if zw <= 0 || zh <= 0 {
            return None;
        }
        let last = ((zw - 1) / CHUNK_PX, (zh - 1) / CHUNK_PX);
        let c0 = |v: i32, hi: i32| v.div_euclid(CHUNK_PX).clamp(0, hi);
        Some((
            c0(cam.0 - margin, last.0),
            c0(cam.1 - margin, last.1),
            c0(cam.0 + i32::from(self.canvas.0) + margin, last.0),
            c0(cam.1 + i32::from(self.canvas.1) + margin, last.1),
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
        if self.zone.is_none() {
            return &self.frame;
        }
        // The sky and the far things on its horizon, behind everything (§1.9).
        self.atmos.draw_back(&mut self.frame, cam, self.zone_cells, &self.atlas);

        // Terrain: every painted chunk under the view.
        if let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, 0) {
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
        let a = i64::from(alpha_256(alpha));
        let (cw, ch) = (i32::from(canvas.0), i32::from(canvas.1));
        let on_canvas =
            |x: i32, y: i32, w: u16, h: u16| x + i32::from(w) > 0 && y + i32::from(h) > 0 && x < cw && y < ch;
        for p in &self.props {
            let r = self.atlas.get(p.look);
            // Bottom-centred on the footprint.
            let x = p.x + (p.w - i32::from(r.src.w)) / 2 - cam.0;
            let y = p.y + p.h - i32::from(r.src.h) - cam.1;
            if !on_canvas(x, y, r.src.w, r.src.h) {
                continue;
            }
            let foot = p.y + p.h - cam.1;
            let caster = (!p.flat && !p.flush).then(|| Caster {
                sprite: 0,
                foot: clamp16(x + i32::from(r.src.w) / 2, y + i32::from(r.ay)),
                height: r.top.max(1),
                depth: (p.h / 4).clamp(4, 12) as u8,
            });
            let cmd = DrawCmd { y: foot, key: p.id, sprite: sprite(r, x, y, Flags::default()), caster };
            if p.flat { self.ground.push(cmd) } else { self.standing.push(cmd) }
        }
        // The chunks' trees, shrubs and stones, from the atlas, by their feet.
        if let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, SORT_MARGIN) {
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    let Some((slot, _)) = self.chunks.find(ChunkId { cx: cx as u16, cy: cy as u16 }) else {
                        continue;
                    };
                    let (ox, oy) = (cx * CHUNK_PX - cam.0, cy * CHUNK_PX - cam.1);
                    for (i, pl) in self.terrain.placed(slot).iter().enumerate() {
                        let fl = self.terrain.flora(pl.sprite);
                        let r = self.atlas.get(fl.look);
                        let (fx, fy) = (ox + i32::from(pl.x), oy + i32::from(pl.y));
                        let (x, y) = (fx - i32::from(r.ax), fy - i32::from(r.ay));
                        if !on_canvas(x, y, r.src.w, r.src.h) {
                            continue;
                        }
                        self.standing.push(DrawCmd {
                            y: fy,
                            key: 0x4000_0000 | u32::from(slot) << 10 | i as u32,
                            sprite: sprite(r, x, y, Flags::default()),
                            caster: Some(Caster {
                                sprite: 0,
                                foot: clamp16(fx, fy - i32::from(fl.lift)),
                                height: r.top.max(1),
                                depth: fl.depth,
                            }),
                        });
                    }
                }
            }
        }
        let lantern = lantern_lit(self.sky.ambient);
        let mut glows: [Option<Light>; 64] = [None; 64];
        let mut n_glows = 0;
        for u in &self.units {
            let (dx, dy) = (i64::from(u.cur.0 - u.prev.0), i64::from(u.cur.1 - u.prev.1));
            let (fx, fy) = if dx * dx + dy * dy > SNAP_FX * SNAP_FX {
                u.cur
            } else {
                (u.prev.0 + ((dx * a) >> 8) as i32, u.prev.1 + ((dy * a) >> 8) as i32)
            };
            let (sx, sy) = ((fx >> FX_TO_CANVAS) - cam.0, (fy >> FX_TO_CANVAS) - cam.1);
            // Her lantern, held at her side the way she faces; a creature's glow at its heart.
            if u.me && lantern && n_glows < glows.len() {
                let side = if u.mirror { -9 } else { 9 };
                let k = flicker(self.tick, u.id, 80, FLICKER_RATE);
                glows[n_glows] = Some(Light {
                    pos: (sx + side, sy + 3),
                    height: 20,
                    colour: scale(LANTERN, k),
                    radius: LANTERN_RADIUS,
                    size: 5,
                    casts: true,
                    kind: LightKind::Point,
                    // Her own: resolved to her sprite once the list is sorted.
                    holder: Some(UNIT_KEY | u.id),
                });
                n_glows += 1;
            }
            if let Some((radius, colour)) = u.glow.filter(|_| n_glows < glows.len()) {
                glows[n_glows] = Some(Light {
                    pos: (sx, sy),
                    height: 14,
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
            let (look, mirror, bob) = match (u.person, u.creature) {
                (Some(set), _) => {
                    // A blow or a spell under way plays its three beats; a blow taken, its hurt.
                    let act = u.struck.and_then(|(t, spell)| {
                        let t = self.tick.wrapping_sub(t);
                        (t < 3 * people::ACT_TICKS).then(|| match jane_data::catalog().combat.spell(spell).anim {
                            jane_data::CastAnim::Cast => people::Act::Cast(t),
                            _ => people::Act::Attack(t),
                        })
                    });
                    let hurt = self.tick < u.hurt_until;
                    // Hands out on the cast's second beat: the school's light between them.
                    if let (Some(people::Act::Cast(t)), Some((_, spell))) = (act, u.struck) {
                        if t / people::ACT_TICKS == 1 && !u.dead {
                            cast_glow = Some(jane_data::catalog().combat.spell(spell).school);
                        }
                    }
                    let pose = people::Pose {
                        facing: u.facing,
                        anim: u.anim,
                        tick: self.tick,
                        dead: u.dead,
                        id: u.id,
                        act,
                        hurt,
                    };
                    let (look, mirror) = self.people.frame(set, pose);
                    (look, mirror, 0)
                }
                // A creature trots, sits a while after it stops, and strikes in three beats.
                (None, Some(set)) => {
                    let attack =
                        u.struck.map(|(t, _)| self.tick.wrapping_sub(t)).filter(|&t| t < 3 * creatures::ATTACK_TICKS);
                    let pose = creatures::Pose {
                        facing: u.facing,
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
            let r = self.atlas.get(look);
            let (x, y) = (sx - i32::from(r.ax), sy - i32::from(r.ay) - bob);
            if !on_canvas(x, y, r.src.w, r.src.h) {
                continue;
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
            let caster = (!u.dead).then(|| Caster { sprite: 0, foot: clamp16(sx, sy), height: r.top.max(1), depth: 5 });
            self.standing.push(DrawCmd {
                y: sy,
                key: UNIT_KEY | u.id,
                sprite: sprite(r, x, y, Flags { mirror, tint }),
                caster,
            });
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
                    if on_canvas(x, y, r.src.w, r.src.h) {
                        self.standing.push(DrawCmd {
                            y: qy,
                            key: UNIT_KEY | u.id,
                            sprite: sprite(r, x, y, Flags { mirror: false, tint: Tint::None }),
                            // Each coil throws its own shadow, footed where it lies.
                            caster: (!u.dead).then(|| Caster {
                                sprite: 0,
                                foot: clamp16(qx, qy),
                                height: r.top.max(1),
                                depth: 4,
                            }),
                        });
                    }
                }
            }
            if let Some(school) = cast_glow {
                // The light gathered between her hands: in front of her facing the viewer or to
                // the side, behind her facing away; it lights what is round it.
                let (dx, dy, ahead) = match u.facing {
                    Facing::East => (9, -21, 1),
                    Facing::West => (-9, -21, 1),
                    Facing::South => (0, -18, 1),
                    Facing::North => (0, -19, -1),
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
                        radius: 64,
                        size: 4,
                        casts: false,
                        kind: LightKind::Point,
                        holder: Some(UNIT_KEY | u.id),
                    });
                    n_glows += 1;
                }
            }
        }
        let rows = (ch + 2 * SORT_MARGIN) as u32;
        let f = &mut self.frame;
        let g0 = f.sprites.len();
        f.sprites.extend(self.ground.sort(-SORT_MARGIN, rows).iter().map(|c| c.sprite));
        let ground = Span::since(g0, f.sprites.len());
        let s0 = f.sprites.len();
        self.holders.clear();
        for c in self.standing.sort(-SORT_MARGIN, rows) {
            if let Some(k) = c.caster {
                self.holders.push((c.key, f.sprites.len() as u32));
                f.casters.push(Caster { sprite: f.sprites.len() as u32, ..k });
            }
            f.sprites.push(c.sprite);
        }
        self.holders.sort_unstable();
        let standing = Span::since(s0, f.sprites.len());
        let casters = Span::since(0, f.casters.len());

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
        f.lights.extend(glows[..n_glows].iter().flatten());
        // The effects' lights: a bolt lights the wall it passes (§2).
        self.fx.lights(f, cam, alpha);
        // A light never shadows what holds it: each holder's key to its sprite, or none.
        let holders = &self.holders;
        for l in &mut f.lights {
            l.holder = l.holder.and_then(|k| holders.binary_search_by_key(&k, |h| h.0).ok().map(|i| holders[i].1));
        }
        // The tier's counts, or fewer where the `max_lights` and `shadows` rows are turned down.
        let (most, casting) = max_lights(f.tier);
        let rows = self.atmos.features;
        let (most, casting) = (most.min(usize::from(rows.max_lights)), casting.min(usize::from(rows.shadows)));
        if f.lights.len() > most || f.lights.iter().filter(|l| l.casts).count() > casting {
            let mid = (cw / 2, ch / 2);
            let d2 = |l: &Light| {
                let (dx, dy) = (i64::from(l.pos.0 - mid.0), i64::from(l.pos.1 - mid.1));
                dx * dx + dy * dy
            };
            f.lights.sort_by_key(|l| d2(l));
            f.lights.truncate(most);
            let mut left = casting;
            for l in &mut f.lights {
                if l.casts {
                    l.casts = left > 0;
                    left = left.saturating_sub(1);
                }
            }
        }
        let points = Span::since(0, f.lights.len());

        // The sky as the weather has it: rain dims and cools, lightning flashes (§1.9).
        let sky = &self.atmos.light(&self.sky);
        f.passes.push(Pass::Sprites { layer: Depth::Ground, cmds: ground });
        // Silhouette sun shadows under the standing things, where the tier has no shadow maps:
        // from a sun or a moon, not from the afterglow, a sky too broad to throw a silhouette.
        if f.tier <= Tier::T1
            && rows.silhouettes
            && let Some(sun) = sky.sun.filter(Directional::silhouettes)
            && casters.len > 0
        {
            let shade = shadow::shade_at(sky.shade, sun.strength);
            f.passes.push(Pass::Silhouettes { sun, shade, casters });
        }
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: standing });
        f.passes.push(Pass::Weather(self.atmos.atmos()));
        // Below T2 the parts that do not glow go under the light, so the lamps light the rain.
        self.fx.draw_under_light(f, cam, alpha);
        // The light pass: on T0 left out when the multiply would change nothing (day is free).
        if f.tier > Tier::T0 || sky.ambient.iter().any(|&c| c < 254) {
            f.passes.push(Pass::Lights { ambient: sky.ambient, fill: sky.fill, sun: sky.sun, points, casters });
        }
        // Over what is lit: the marks and splashes on the ground, the fog, the effects in the
        // air, the rain.
        self.fx.draw_ground(f, cam, alpha, sky);
        self.atmos.draw_fog(f, cam, sky);
        self.fx.draw_air(f, cam, alpha, sky);
        // The grade (§1.3 `grade`): all of it at T2; at T1 the tint and the lift alone, and none
        // of the exposure, saturation or bloom T1 does not draw.
        // The `grade` row off leaves the lit frame as it is; the `bloom` row off, the glow.
        let post = if rows.grade { sky.post } else { Post { bloom: sky.post.bloom, ..Post::NONE } };
        let post = Post { bloom: if rows.bloom { post.bloom } else { 0 }, ..post };
        if f.tier >= Tier::T2 {
            f.passes.push(Pass::Post(post));
        } else if f.tier == Tier::T1 {
            f.passes.push(Pass::Post(Post { tint: post.tint, lift: post.lift, ..Post::NONE }));
        }
        &self.frame
    }
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
    }
}
