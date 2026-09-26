//! `Present` (PRESENTATION.md §1.11): `tick()` once per accumulated tick, whether or not the sim
//! ran; `draw(alpha)` pure in the tick and `alpha`.
//!
//! `tick` reads the `View` and this tick's events into the presenter's own records (units with
//! their last two positions and timers, props near the view, the lights the view says are lit
//! in reach of it, the camera, the chunks under it, the sky); `draw` turns those records into a
//! `Frame` at `alpha` and reads nothing else.
//! Neither reads `GameState`, and neither writes anything but the presenter.

use jane_core::action::{CameraMode, Facing};
use jane_core::num::CELL_SHIFT;
use jane_core::{Rect, ZoneId};
use jane_data::{Controller, Faction, Region};
use jane_sim::event::{Event, EventKind, events_for};
use jane_sim::ids::PropIx;
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::backend::AtlasPages;
use crate::camera::{Camera, alpha_256};
use crate::chunks::{ChunkCache, LRU, Need};
use crate::drawlist::{DrawCmd, DrawList};
use crate::frame::{
    CANVAS_H, CANVAS_W, CELL, CHUNK_PX, Caster, ChunkCmd, ChunkId, Depth, FX_TO_CANVAS, Flags, Frame, Light, LightKind,
    Pass, Rgb, Span, SpriteCmd, Tier, Tint,
};
use crate::light::{Sky, flicker, lantern_lit, sky};
use crate::people::{self, People};
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
/// How many steps a second a flame's flicker walks (§1.7).
const FLICKER_RATE: u32 = 10;

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
    stand: StandIns,
    people: People,
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
    hurt: Vec<u32>,
    props: Vec<PropRec>,
    prop_scratch: Vec<PropIx>,
    standing: DrawList,
    ground: DrawList,
    lights: Vec<LightRec>,
    light_scratch: Vec<PropIx>,
    sky: Sky,
}

impl Present {
    /// A presenter at `tier`, its atlas built.
    pub fn new(tier: Tier) -> Present {
        let mut atlas = Atlas::with_layers(tier > Tier::T0);
        let stand = StandIns::build(&mut atlas);
        let people = People::build(&mut atlas);
        let terrain = Terrain::build(&mut atlas, LRU);
        let mut frame = Frame::new(tier);
        let chunks = ChunkCache::reserved(&mut frame.layers, tier);
        Present {
            atlas,
            stand,
            people,
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
            hurt: Vec::with_capacity(64),
            props: Vec::with_capacity(1024),
            prop_scratch: Vec::with_capacity(1024),
            standing: DrawList::default(),
            ground: DrawList::default(),
            lights: Vec::with_capacity(256),
            light_scratch: Vec::with_capacity(1024),
            sky: sky(12 * 7200, 0, false, 1000, Region::Lowfields),
        }
    }

    /// The sky as of the last tick.
    pub fn sky(&self) -> &Sky {
        &self.sky
    }

    /// The pages every backend uploads at boot.
    pub fn atlas(&self) -> &AtlasPages {
        &self.atlas.pages
    }

    /// Ticks presented since New Game.
    pub fn ticks(&self) -> u32 {
        self.tick
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
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
        }
        self.zone_cells = view.size();
        self.hurt.clear();
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
        let (clock, day) = view.clock();
        self.sky = sky(clock, day, view.indoor(), view.ambient().0, view.region());
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
        for uv in view.units_in(area) {
            let u = uv.unit;
            let id = u.id.get();
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
            // A seat's coat: hers by her seat; another player's, one of the other three, by id
            // (the view names no other seat's body).
            let seat = match kind {
                UnitKind::Me => my_seat,
                UnitKind::Seat => {
                    let s = 1 + (id % 3) as u8;
                    if s == my_seat { 0 } else { s }
                }
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
    }

    fn read_props(&mut self, view: &View<'_>, area: Rect) {
        let cat = jane_data::catalog();
        let (props, stand) = (&mut self.props, &self.stand);
        props.clear();
        view.for_props_in(area, &mut self.prop_scratch, |p| {
            let d = cat.story.prop(p.def);
            // An open gate is a doorway.
            if d.gate && !p.solid {
                return;
            }
            props.push(PropRec {
                id: p.id.get(),
                x: i32::from(p.cell.x) * CELL,
                y: i32::from(p.cell.y) * CELL,
                w: i32::from(d.w) * CELL,
                h: i32::from(d.h) * CELL,
                look: {
                    let look = stand.prop(d.w, d.h, d.flat, d.light.is_some());
                    // A lamp the view says is out has dark glass.
                    if d.light.is_some() && view.light_showing(p).is_none() { stand.unlit(look) } else { look }
                },
                flat: d.flat,
            });
        });
        props.sort_unstable_by_key(|p| p.id);
    }

    /// The prop lights the view says are showing, in reach of the view (THE rule, `View::light_showing`).
    fn read_lights(&mut self, view: &View<'_>, area: Rect) {
        let cat = jane_data::catalog();
        let reach =
            Rect::new(area.x - LIGHT_CELLS, area.y - LIGHT_CELLS, area.w + 2 * LIGHT_CELLS, area.h + 2 * LIGHT_CELLS);
        let (lights, stand, atlas) = (&mut self.lights, &self.stand, &self.atlas);
        lights.clear();
        view.for_props_in(reach, &mut self.light_scratch, |p| {
            let Some(l) = view.light_showing(p) else { return };
            let d = cat.story.prop(p.def);
            let (x, y) = (i32::from(p.cell.x) * CELL, i32::from(p.cell.y) * CELL);
            let (w, h) = (i32::from(d.w) * CELL, i32::from(d.h) * CELL);
            // Where the flame is: a building's lit windows on its front, low; a thing on the
            // floor just above it; else where its sprite glows, standing on its foot (the row
            // its sprite stands on, as it is drawn: bottom-centred on the footprint).
            let look = stand.prop(d.w, d.h, d.flat, true);
            let r = atlas.get(look);
            let (gx, gy, height, size) = if d.w >= 3 || d.h >= 3 {
                (x + w / 2, y + h + 4, 16, 12)
            } else if d.flat {
                (x + w / 2, y + h / 2, 4, 6)
            } else {
                let foot = y + h - i32::from(r.src.h) + i32::from(r.ay);
                let glass = stand.glass(look).map_or(i32::from(r.height) * 2 / 3, i32::from);
                (x + w / 2, foot, glass.clamp(4, 60), if d.w == 1 { 6 } else { 10 })
            };
            lights.push(LightRec {
                id: p.id.get(),
                x: gx,
                y: gy,
                height: height as u8,
                colour: rgb(l.color),
                radius: (l.radius.0 >> FX_TO_CANVAS).clamp(0, 2048) as u16,
                size: size as u8,
                dip: l.flicker.0,
            });
        });
        lights.sort_unstable_by_key(|l| l.id);
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
                    terrain.swatched(slot);
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
            let caster = (!p.flat).then(|| Caster {
                sprite: 0,
                foot: clamp16(x + i32::from(r.src.w) / 2, y + i32::from(r.ay)),
                height: r.src.h.min(255) as u8,
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
                            caster: Some(Caster { sprite: 0, foot: clamp16(fx, fy), height: r.height, depth: fl.depth }),
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
                });
                n_glows += 1;
            }
            // A person shows its walk, breathe or dead frame (ART.md §4); a stand-in walks with
            // a one-px bob.
            let (look, mirror, bob) = match u.person {
                Some(set) => {
                    let pose = people::Pose { facing: u.facing, anim: u.anim, tick: self.tick, dead: u.dead, id: u.id };
                    let (look, mirror) = self.people.frame(set, pose);
                    (look, mirror, 0)
                }
                None => (u.look, u.mirror, i32::from((u.anim / 9) & 1 == 1)),
            };
            let r = self.atlas.get(look);
            let (x, y) = (sx - i32::from(r.ax), sy - i32::from(r.ay) - bob);
            if !on_canvas(x, y, r.src.w, r.src.h) {
                continue;
            }
            let tint = if u.dead && u.person.is_some() {
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
                height: r.ay.clamp(1, 255) as u8,
                depth: 5,
            });
            self.standing.push(DrawCmd {
                y: sy,
                key: 0x8000_0000 | u.id,
                sprite: sprite(r, x, y, Flags { mirror, tint }),
                caster,
            });
        }
        let rows = (ch + 2 * SORT_MARGIN) as u32;
        let f = &mut self.frame;
        let g0 = f.sprites.len();
        f.sprites.extend(self.ground.sort(-SORT_MARGIN, rows).iter().map(|c| c.sprite));
        let ground = Span::since(g0, f.sprites.len());
        let s0 = f.sprites.len();
        for c in self.standing.sort(-SORT_MARGIN, rows) {
            if let Some(k) = c.caster {
                f.casters.push(Caster { sprite: f.sprites.len() as u32, ..k });
            }
            f.sprites.push(c.sprite);
        }
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
            });
        }
        f.lights.extend(glows[..n_glows].iter().flatten());
        let (most, casting) = max_lights(f.tier);
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

        let sky = &self.sky;
        f.passes.push(Pass::Sprites { layer: Depth::Ground, cmds: ground });
        // Silhouette sun shadows under the standing things, where the tier has no shadow maps:
        // from a sun or a moon, not from the afterglow, a sky too broad to throw a silhouette.
        if f.tier <= Tier::T1
            && let Some(sun) = sky.sun.filter(|s| s.spread <= crate::light::SILHOUETTE_SPREAD)
            && casters.len > 0
        {
            f.passes.push(Pass::Silhouettes { sun, shade: sky.shade, casters });
        }
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: standing });
        // The light pass: on T0 left out when the multiply would change nothing (day is free).
        if f.tier > Tier::T0 || sky.ambient.iter().any(|&c| c < 254) {
            f.passes.push(Pass::Lights { ambient: sky.ambient, fill: sky.fill, sun: sky.sun, points, casters });
        }
        if f.tier >= Tier::T2 {
            f.passes.push(Pass::Post(sky.post));
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
