//! `Present` (PRESENTATION.md §1.11): `tick()` once per accumulated tick, whether or not the sim
//! ran; `draw(alpha)` pure in the tick and `alpha`.
//!
//! `tick` reads the `View` and this tick's events into the presenter's own records (units with
//! their last two positions and timers, props near the view, the camera, the chunks under it,
//! the ambient); `draw` turns those records into a `Frame` at `alpha` and reads nothing else.
//! Neither reads `GameState`, and neither writes anything but the presenter.

use jane_core::action::{CameraMode, Facing};
use jane_core::num::CELL_SHIFT;
use jane_core::{Rect, ZoneId};
use jane_data::{Controller, Faction};
use jane_sim::event::{Event, EventKind, events_for};
use jane_sim::ids::PropIx;
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::backend::AtlasPages;
use crate::camera::{Camera, alpha_256};
use crate::chunks::ChunkCache;
use crate::drawlist::{DrawCmd, DrawList};
use crate::frame::{
    CANVAS_H, CANVAS_W, CELL, CHUNK_PX, ChunkCmd, ChunkId, Depth, FX_TO_CANVAS, Flags, Frame, Pass, Span, SpriteCmd,
    Tier, Tint,
};
use crate::light::ambient;
use crate::stand_in::{self, StandIns, UnitKind};

/// A unit moving further than this in a tick (40 sim px, `Fx`) snaps instead of sliding: travel,
/// a respawn, a hop.
const SNAP_FX: i64 = 40 * 256;
/// Cells round the view whose units and props are kept: a tall sprite standing below the view
/// still reaches into it.
const MARGIN_CELLS: i32 = 6;
/// Canvas px round the view painted ahead, so a frame between two ticks never finds a hole.
const CHUNK_AHEAD: i32 = 64;
/// Canvas px round the view the draw list sorts over; things further out are culled.
const SORT_MARGIN: i32 = 96;
/// Ticks a hurt unit shows it, and the first ticks of them it flashes (§1.11).
const HURT_TICKS: u32 = 8;
const FLASH_TICKS: u32 = 4;

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

/// The presenter: its own state, never the sim's.
#[derive(Debug)]
pub struct Present {
    atlas: Atlas,
    stand: StandIns,
    frame: Frame,
    tick: u32,
    /// The canvas the last frame was drawn for; the camera frames for it.
    canvas: (u16, u16),
    camera: Camera,
    zone: Option<(ZoneId, u32)>,
    zone_cells: (u32, u32),
    chunks: ChunkCache,
    units: Vec<UnitRec>,
    units_next: Vec<UnitRec>,
    hurt: Vec<u32>,
    props: Vec<PropRec>,
    prop_scratch: Vec<PropIx>,
    standing: DrawList,
    ground: DrawList,
    ambient: [u8; 3],
}

impl Present {
    /// A presenter at `tier`, its atlas built.
    pub fn new(tier: Tier) -> Present {
        let mut atlas = Atlas::new();
        let stand = StandIns::build(&mut atlas);
        let mut frame = Frame::new(tier);
        let chunks = ChunkCache::reserved(&mut frame.layers);
        Present {
            atlas,
            stand,
            frame,
            tick: 0,
            canvas: (CANVAS_W, CANVAS_H),
            camera: Camera::default(),
            zone: None,
            zone_cells: (0, 0),
            chunks,
            units: Vec::with_capacity(256),
            units_next: Vec::with_capacity(256),
            hurt: Vec::with_capacity(64),
            props: Vec::with_capacity(1024),
            prop_scratch: Vec::with_capacity(1024),
            standing: DrawList::default(),
            ground: DrawList::default(),
            ambient: [255; 3],
        }
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
            self.camera.reset();
            self.units.clear();
        }
        self.zone_cells = view.size();
        self.hurt.clear();
        for e in events_for(events, view.me()) {
            match e.kind {
                EventKind::Tiles(r) => self.chunks.invalidate(r),
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
        self.paint_chunks(view);
        self.ambient = ambient(view.clock().0, view.indoor(), view.ambient().0);
    }

    /// The cells the view covers this tick, with the margin.
    fn area(&self) -> Rect {
        let (x, y) = (self.camera.pos.0 >> CELL_SHIFT, self.camera.pos.1 >> CELL_SHIFT);
        let (w, h) = (i32::from(self.canvas.0) / CELL + 2, i32::from(self.canvas.1) / CELL + 2);
        Rect::new(x - MARGIN_CELLS, y - MARGIN_CELLS, w + 2 * MARGIN_CELLS, h + 2 * MARGIN_CELLS)
    }

    fn read_units(&mut self, view: &View<'_>, area: Rect) {
        let me = view.me().unit;
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
            self.units_next.push(UnitRec {
                id,
                prev,
                cur,
                anim: if prev == cur { 0 } else { old.map_or(0, |o| o.anim) + 1 },
                hurt_until: old.map_or(0, |o| o.hurt_until),
                look: self.stand.unit(kind),
                mirror: u.facing == Facing::West,
                dead: !u.alive,
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
                look: stand.prop(d.w, d.h, d.flat, d.light.is_some()),
                flat: d.flat,
            });
        });
        props.sort_unstable_by_key(|p| p.id);
    }

    /// Paints every chunk under the view (and a little round it) that is not painted yet.
    fn paint_chunks(&mut self, view: &View<'_>) {
        let Some((cx0, cy0, cx1, cy1)) =
            self.chunk_range((self.camera.pos.0 >> FX_TO_CANVAS, self.camera.pos.1 >> FX_TO_CANVAS), CHUNK_AHEAD)
        else {
            return;
        };
        let (cells, outside, now) = (self.zone_cells, self.frame.clear, self.tick);
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let id = ChunkId { cx: cx as u16, cy: cy as u16 };
                self.chunks.want(id, now, &mut self.frame.layers, |px| {
                    stand_in::paint_chunk(id, cells, outside, |x, y| view.tile(x, y), px);
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
            let cmd = DrawCmd { y: p.y + p.h - cam.1, key: p.id, sprite: sprite(r, x, y, Flags::default()) };
            if p.flat { self.ground.push(cmd) } else { self.standing.push(cmd) }
        }
        for u in &self.units {
            let (dx, dy) = (i64::from(u.cur.0 - u.prev.0), i64::from(u.cur.1 - u.prev.1));
            let (fx, fy) = if dx * dx + dy * dy > SNAP_FX * SNAP_FX {
                u.cur
            } else {
                (u.prev.0 + ((dx * a) >> 8) as i32, u.prev.1 + ((dy * a) >> 8) as i32)
            };
            let (sx, sy) = ((fx >> FX_TO_CANVAS) - cam.0, (fy >> FX_TO_CANVAS) - cam.1);
            let r = self.atlas.get(u.look);
            // The walk frame, as far as a stand-in can walk: a one-px bob.
            let bob = i32::from((u.anim / 9) & 1 == 1);
            let (x, y) = (sx - i32::from(r.ax), sy - i32::from(r.ay) - bob);
            if !on_canvas(x, y, r.src.w, r.src.h) {
                continue;
            }
            let tint = if u.dead {
                Tint::Ghost(160)
            } else if self.tick < u.hurt_until && u.hurt_until - self.tick > HURT_TICKS - FLASH_TICKS {
                Tint::Flash(128)
            } else {
                Tint::None
            };
            self.standing.push(DrawCmd {
                y: sy,
                key: 0x8000_0000 | u.id,
                sprite: sprite(r, x, y, Flags { mirror: u.mirror, tint }),
            });
        }
        let rows = (ch + 2 * SORT_MARGIN) as u32;
        let f = &mut self.frame;
        for (list, layer) in [(&mut self.ground, Depth::Ground), (&mut self.standing, Depth::Standing)] {
            let start = f.sprites.len();
            f.sprites.extend(list.sort(-SORT_MARGIN, rows).iter().map(|c| c.sprite));
            f.passes.push(Pass::Sprites { layer, cmds: Span::since(start, f.sprites.len()) });
        }

        // The light pass: the ambient, left out when it would change nothing.
        if self.ambient.iter().any(|&c| c < 254) {
            f.passes.push(Pass::Lights { ambient: self.ambient });
        }
        &self.frame
    }
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
