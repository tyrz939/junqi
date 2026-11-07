//! The night's turn and grade (NIGHT.md §2, §4.6), the decks, and the lights the view says are
//! lit in reach of it (PRESENTATION.md §1.7), with those that stand against walls.

use super::*;

impl Present {
    /// The night's turn and the stage's grade (NIGHT.md §2, §4.6), from the sim's latched night:
    /// the turn under way is derived from `tick - turned_at` alone, so a save loaded mid-turn
    /// continues it; the intensity where she stands eases as she crosses from a hub to the
    /// fields, and is taken at once on entering a zone or by day.
    pub(super) fn night_tick(&mut self, view: &View<'_>) {
        /// A step of intensity in about a second.
        const EASE: u32 = 5;
        let night = view.night();
        let room = crate::turn::Room::of(view.zone());
        self.turn = crate::turn::under_way(night, view.tick().0, view.flag("bell_stopped") != 0);
        if night.stage > 0 {
            self.last_stage = night.stage;
        }
        let entered = self.room != room || self.stage != night.stage;
        self.room = room;
        self.stage = night.stage;
        let dawn = night.stage == 0 && self.turn.is_some();
        let stage = if dawn { self.last_stage } else { night.stage };
        let want = if stage == 0 {
            0
        } else {
            let (cx, cy) = view.body().pos.cell();
            u32::from(view.night_map().intensity(stage, cx, cy)) * 256
        };
        self.night_q = if entered || stage == 0 {
            want
        } else if self.night_q < want {
            (self.night_q + EASE).min(want)
        } else {
            self.night_q.saturating_sub(EASE).max(want)
        };
        // The night's look (NIGHT.md §4): in at the held dark, out as dawn's band starts; a save
        // loaded at night has it at once. The chunks are painted again under it (two a tick, the
        // view's in a fraction of the held dark).
        let look_in = match self.turn {
            Some((kind, since)) => crate::turn::night_in(kind, since),
            None => night.stage > 0,
        };
        let look_stage = if look_in { stage.max(1) } else { 0 };
        let bp = view.blueprints().get(view.zone());
        let turned = self.terrain.set_night(bp, look_stage, view.seed());
        if turned && self.terrain.lays_overlays() {
            let (w, h) = view.size();
            self.chunks.invalidate(Rect::new(0, 0, w as i32, h as i32));
        } else if (turned || core::mem::take(&mut self.renight)) && !self.terrain.lays_overlays() {
            // A console: its CLUTs fold the night; only the overlays are laid again.
            self.terrain.place_night_all();
        }
        // A sprite reads its albedo at the intensity where she stands, in whole steps.
        self.frame.night = if look_in { ((self.night_q + 128) / 256).min(4) as u8 } else { 0 };
        self.atmos.night = self.frame.night;
    }

    /// How many night overlays a console draws at most (`night::console_cap` of its preset).
    pub fn set_night_cap(&mut self, n: u16) {
        self.night_cap = n;
    }

    /// The turn as it stands this tick on a canvas `h` rows tall (`crate::turn::look`).
    pub(super) fn turn_look(&self, h: u16) -> crate::turn::Look {
        self.turn.map_or(crate::turn::Look::NONE, |(k, since)| crate::turn::look(k, self.room, since, self.stage, h))
    }

    /// The turn this tick at the canvas last drawn: what the light pass's band and each
    /// guttering light are given (the tests compare it across tiers).
    pub fn turn_now(&self) -> crate::turn::Look {
        self.turn_look(self.canvas.1)
    }

    /// The prop lights the view says are showing, in reach of the view (THE rule, `View::light_showing`).
    /// The zone's spans as decks to draw (MAP.md §2.5): each one's material by its own kind and its
    /// region (`jane_art::deck::Kind::for_span`), whole or broken now, and how many levels it stands over
    /// the ground under its middle.
    pub(super) fn read_decks(&mut self, view: &View<'_>) {
        self.deck_spans.clear();
        let county = view.zone() == ZoneId::County;
        for (i, s) in view.spans().iter().enumerate() {
            let r = s.rect;
            let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
            let region = if county {
                match view.region_at(cx, cy) {
                    Region::Lowfields => 0,
                    Region::Waters => 1,
                    Region::Works => 2,
                }
            } else {
                0
            };
            self.deck_spans.push(crate::decks::DeckSpan {
                rect: r,
                along_x: s.along_x,
                whole: view.span_whole(i as jane_core::blueprint::SpanIx),
                kind: jane_art::deck::Kind::for_span(s.kind, region),
                level: s.deck_level,
                over: s.deck_level.saturating_sub(view.level(cx, cy)),
            });
        }
    }

    pub(super) fn read_lights(&mut self, view: &View<'_>, area: Rect) {
        let cat = jane_data::catalog();
        let reach =
            Rect::new(area.x - LIGHT_CELLS, area.y - LIGHT_CELLS, area.w + 2 * LIGHT_CELLS, area.h + 2 * LIGHT_CELLS);
        let (lights, stand, atlas, kit) = (&mut self.lights, &self.stand, &self.atlas, &self.kit);
        lights.clear();
        let clock = self.clock;
        let t_scan = clock.map_or(0, |c| c());
        // What the turn's gutter keeps (NIGHT.md §2.2): every fire, and in the county the lamps
        // where the shield holds tonight, a place whose intensity is under the stage (a hub's,
        // Julie's yard's). Indoors only the fire: the room's lamps gutter.
        let stage = view.night().stage.max(1);
        let map = view.night_map();
        let county = view.zone() == ZoneId::County;
        let look_stage = self.terrain.night_stage();
        let tick = self.tick;
        view.for_props_in(reach, &mut self.light_scratch, |p| {
            let Some(l) = view.light_showing(p) else { return };
            let wrong = wrong_lamp(view, p, l, look_stage);
            let kept = !wrong
                && (l.flicker.0 >= 200
                    || county && map.intensity(stage, i32::from(p.cell.x), i32::from(p.cell.y)) < stage);
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
                // A wrong lamp: cold green-white, breathing slowly (NIGHT.md §4.5).
                colour: if wrong {
                    scale(crate::night::LIGHT_COLD, crate::night::cold_flicker(tick, p.id.get()))
                } else {
                    rgb(l.color)
                },
                // An open flame (a fire, a brazier: a light that dips a fifth or more) throws its pool
                // half again as far as its row says the sentries see it: the eye sees a fire's glow
                // well past where it lights a face. Presentation only; `light_showing` is unchanged.
                radius: {
                    let r = (l.radius.0 >> FX_TO_CANVAS).clamp(0, 2048);
                    (if l.flicker.0 >= 200 { r * 3 / 2 } else { r }) as u16
                },
                size: size as u8,
                dip: if wrong { 0 } else { l.flicker.0 },
                kept,
                wrong,
            });
        });
        self.prof[11] = self.prof[11].wrapping_add(clock.map_or(0, |c| c()).wrapping_sub(t_scan));
        // The night shift's cold glow (its row's `glow`): a small pale light about each, so a
        // thing of the night reads at a glance in the dark. Presentation only; it keeps nothing
        // off and lights nothing for the sim.
        for u in view.units_in(reach) {
            let def = cat.combat.unit(u.unit.def);
            let Some(g) = def.glow.filter(|_| def.night_only && u.unit.alive && !u.unit.hidden) else { continue };
            lights.push(LightRec {
                id: 0x8000_0000 | u.unit.id.get(),
                x: u.unit.pos.x.0 >> FX_TO_CANVAS,
                y: u.unit.pos.y.0 >> FX_TO_CANVAS,
                height: 12,
                colour: rgb(g.color),
                radius: (g.radius.0 >> FX_TO_CANVAS).clamp(0, 2048) as u16,
                size: 4,
                dip: 150,
                kept: false,
                wrong: false,
            });
        }
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
    pub(super) fn against_walls(&mut self) {
        /// How far a light is looked for open ground, px.
        const REACH: i32 = 24;
        let (layers, chunks) = (&self.frame.layers, &self.chunks);
        let (zw, zh) = (self.zone_cells.0 as i32 * CELL, self.zone_cells.1 as i32 * CELL);
        let last: core::cell::Cell<Option<(ChunkId, Option<u16>)>> = core::cell::Cell::new(None);
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
        // The layers changed since the answers were found: they are found again.
        if self.walls_at != chunks.painted || self.walls.len() >= WALLS_KEPT {
            self.walls.clear();
            self.walls_at = chunks.painted;
        }
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
            if let Some(w) = self.walls.iter().find(|w| (w.0, w.1, w.2) == (l.id, l.x, l.y)) {
                (l.x, l.y) = (w.3, w.4);
                continue;
            }
            let (x0, y0) = (l.x, l.y);
            // On the terrain, under its light or over it: a torch on a wall's top 20 px up, its
            // light at 28, lit the rock's top and left the passage beside it in the rock's
            // shadow on every tier that cast from it (the mine, 2026-09-27).
            if field(l.x, l.y) <= 2 {
                self.walls.push((l.id, x0, y0, l.x, l.y));
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
            self.walls.push((l.id, x0, y0, l.x, l.y));
        }
    }
}
