//! `Present::draw` (PRESENTATION.md §1.11): the `Frame` at `alpha`, from the presenter's own
//! records alone; with the sway, flutter and fall of foliage and the heights things are drawn at.
//! `draw` is one long fn and is left whole.

use super::*;

impl Present {
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
            // The night sky in the water (NIGHT.md §4.6): fewer on a console.
            self.atmos.water_night(f, cam, if self.deferred { 64 } else { 220 });
            // The waterfalls falling (MAP.md §2.6), on the ground's layer, under what stands.
            falls(f, &self.chunks, &self.terrain, self.tick, self.atmos.features.water);
            // A console's night overlays (NIGHT.md §4.3): sprites from the night page over the
            // chunks, the nearest her first, at most its preset's cap (a PC laid them in).
            if !self.terrain.lays_overlays() && self.terrain.night_stage() > 0 && !self.night_art.refs.is_empty() {
                let mid = (i32::from(canvas.0) / 2, i32::from(canvas.1) / 2);
                let s0 = f.sprites.len();
                let near = &mut self.night_near;
                near.clear();
                for c in &f.chunks {
                    for o in self.terrain.overlays(c.slot) {
                        let Some(&id) = self.night_art.refs.get(usize::from(o.look)) else { continue };
                        let r = self.atlas.get(id);
                        let (x, y) = (c.x + i32::from(o.x) - i32::from(r.ax), c.y + i32::from(o.y) - i32::from(r.ay));
                        let (w, h) = (i32::from(r.src.w), i32::from(r.src.h));
                        if x + w <= 0 || y + h <= 0 || x >= i32::from(canvas.0) || y >= i32::from(canvas.1) {
                            continue;
                        }
                        let flags = Flags { mirror: o.mirror, ..Flags::default() };
                        let d = (x + w / 2 - mid.0).unsigned_abs() + (y + h / 2 - mid.1).unsigned_abs();
                        near.push((d, sprite(r, x, y, flags)));
                    }
                }
                near.sort_unstable_by_key(|n| n.0);
                f.sprites.extend(near.iter().take(usize::from(self.night_cap)).map(|n| n.1));
                if f.sprites.len() > s0 {
                    f.passes.push(Pass::Sprites { layer: Depth::Ground, cmds: Span::since(s0, f.sprites.len()) });
                }
            }
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
        let (chunks, layers, cells, terr) = (&self.chunks, &self.frame.layers, self.zone_cells, &self.terrain);
        // How high the ground at zone px `(x, y)` is (MAP.md §6.2): its chunk's cell's base.
        // A console lays its shadows and what hides on the screen over heights kept relative to
        // their ground (`terrain::relative`, MAP.md §6.3): its bases are all 0.
        let rel = self.frame.t8;
        let base_at = |x: i32, y: i32| if rel { 0 } else { ground_base(chunks, terr, cells, (x, y)) };
        // Over ground `base` px up, the terrain's heights count from it.
        let behind = |x: i32, y: i32, base: u8| {
            let h = drawn_height(chunks, layers, cells, (x, y - 1));
            Foot::hides((h.clamp(0, 255) as u8).saturating_sub(base), y - 1, y)
        };
        // A held thing goes where its carrier is drawn at this alpha, not where she stood at the
        // tick: canvas px from her feet at the tick to her feet as drawn.
        let units = &self.units;
        let follow = |id: u32| {
            let Some(u) = units.iter().find(|u| u.id == id) else { return (0, 0) };
            let (dx, dy) = (i64::from(u.cur.0 - u.prev.0), i64::from(u.cur.1 - u.prev.1));
            if dx * dx + dy * dy > SNAP_FX * SNAP_FX {
                return (0, 0);
            }
            let (fx, fy) = (u.prev.0 + ((dx * a) >> 8) as i32, u.prev.1 + ((dy * a) >> 8) as i32);
            ((fx >> FX_TO_CANVAS) - (u.cur.0 >> FX_TO_CANVAS), (fy >> FX_TO_CANVAS) - (u.cur.1 >> FX_TO_CANVAS))
        };
        for p in &self.props {
            let r = self.atlas.get(p.look);
            let (ox, oy) = p.follows.map_or((0, 0), follow);
            // Bottom-centred on the footprint.
            let x = p.x + ox + (p.w - i32::from(r.src.w)) / 2 - cam.0;
            let y = p.y + oy + p.h - i32::from(r.src.h) - cam.1 - p.lift;
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
            let foot = p.sort_foot.unwrap_or(p.y + p.h) + oy - cam.1;
            let pbase = base_at(p.x + p.w / 2, p.y + p.h - 1);
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
                    base: pbase,
                }
            });
            let mut cmd = DrawCmd { y: foot, key: p.id, sprite: sprite(r, x, y, Flags::default()), caster };
            if !p.flat && !p.flush && !p.on_top && behind(p.x + p.w / 2, p.y + p.h, pbase) {
                cmd.sprite.foot = Some(Foot { y: clamp16(0, foot).1, see: false, base: pbase });
            }
            if p.flat {
                self.ground.push(cmd);
            } else {
                self.standing.push(cmd);
            }
            // Julie's ring round the post at the height of her hand, drawn just after it.
            if p.ring
                && let Some(&id) = jane_art::night::all()
                    .iter()
                    .position(|&l| l == jane_art::night::Look::Ring)
                    .and_then(|i| self.night_art.refs.get(i))
            {
                let rr = self.atlas.get(id);
                let (rx, ry) = (p.x + p.w / 2 - cam.0, p.y + p.h - cam.1 - rows_up(RING_HEIGHT));
                let s = sprite(rr, rx - i32::from(rr.ax), ry - i32::from(rr.ay), Flags::default());
                self.standing.push(DrawCmd { y: foot, key: p.id | 0x4000_0000, sprite: s, caster: None });
            }
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
                        let fbase = base_at(fx + cam.0, fy + cam.1 - 1);
                        if behind(fx + cam.0, fy + cam.1, fbase) {
                            sp.foot = Some(Foot { y: clamp16(0, fy).1, see: false, base: fbase });
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
                                base: fbase,
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
            let ub = if rel { 0 } else { u.base };
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
                i32::from(ub),
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
                    base: ub,
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
                    base: ub,
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
                base: ub,
                ..Caster::default()
            });
            let mut sp = sprite(r, x, y, Flags { mirror, tint, bend: crate::frame::Bend::NONE });
            // Carried over a ledge: up and down an arc over the hop's half second.
            if u.hop {
                let t = self.tick.wrapping_sub(u.hop_at) as i32;
                let n = i32::from(jane_sim::tuning::LEDGE_HOP_TICKS).max(1);
                let tt = t.clamp(0, n);
                sp.y = clamp16(0, i32::from(sp.y) - 4 * HOP_ARC * tt * (n - tt) / (n * n)).1;
            }
            // A player shows through what hides her. On a deck nothing of the ground does.
            if !u.deck && behind(fx >> FX_TO_CANVAS, fy >> FX_TO_CANVAS, ub) {
                sp.foot = Some(Foot { y: clamp16(0, sy).1, see: u.player, base: ub });
            }
            if u.seen && on_canvas(x, y, r.src.w, r.src.h) {
                self.seen.push(SpriteCmd {
                    flags: Flags { mirror, tint: Tint::Seen, bend: crate::frame::Bend::NONE },
                    foot: None,
                    ..sp
                });
            }
            // On a deck: drawn over the decks, and nothing of the ground under it hides it; its
            // foot carries its base for the tiers' heights.
            if u.deck {
                // Sorted just after its deck's pieces (at the deck's south edge), among what
                // stands on it by its feet.
                sp.foot = Some(Foot { y: i16::MAX, see: false, base: ub });
                let (zx, zy) = ((fx >> FX_TO_CANVAS) / CELL, (fy >> FX_TO_CANVAS) / CELL);
                let south = self
                    .deck_spans
                    .iter()
                    .find(|d| d.rect.contains(zx, zy))
                    .map_or(sy, |d| d.south() * CELL - 1 - cam.1);
                let order = (sy - south + 0x800).clamp(0, 0xfff) as u32;
                let key = DECK_KEY | 0x0800_0000 | order << 12 | (u.id & 0xfff);
                self.standing.push(DrawCmd { y: south, key, sprite: sp, caster });
            } else {
                self.standing.push(DrawCmd { y: sy, key: UNIT_KEY | u.id, sprite: sp, caster });
            }
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
                        if behind(px >> FX_TO_CANVAS, py >> FX_TO_CANVAS, ub) {
                            sp.foot = Some(Foot { y: clamp16(0, qy).1, see: false, base: ub });
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
                                base: ub,
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
                        base: ub,
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
            // On a fence or a wall: its feet on the terrain's top in its column (its height over
            // the ground's base, on raised ground).
            let abase = base_at(a.x, a.y - 1);
            let lift = if a.perch {
                (a.y - 24..a.y)
                    .find(|&yy| {
                        let h = drawn_height(&self.chunks, &self.frame.layers, self.zone_cells, (a.x, yy))
                            - i32::from(abase);
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
            if !a.perch && !a.flat && a.up < 4 && behind(a.x, a.y, abase) {
                sp.foot = Some(Foot { y: clamp16(0, sy).1, see: false, base: abase });
            }
            let cmd = DrawCmd { y: sy, key: ambient::AMBIENT_KEY | a.key, sprite: sp, caster: None };
            if a.flat { self.ground.push(cmd) } else { self.standing.push(cmd) }
        }
        let (sort_top, rows) = (band.top + SORT_PAST, (ch + band.top + band.bottom + 2 * SORT_PAST) as u32);
        let block_range = self.chunk_range(cam, band);
        let window_range = self.chunk_range(cam, Margins::uniform(CAST_MARGIN));
        // The night's turn (NIGHT.md §2.2): the sky's light by row (the light pass's band), and
        // every light but the kept ones guttering with it.
        let turn = self.turn_look(ch as u16);
        // The decks (MAP.md §2.5, §6.2): each span's pieces stand in the standing list at its south
        // edge (its face's, under one running east-west), so what is under it or north of it is
        // drawn first and covered, and what stands south of it after, in front; what stands on a
        // deck goes just after its pieces. Each piece stands on its deck's height.
        crate::decks::lay(&self.deck_spans, &mut self.deck_laid, &mut self.deck_scratch);
        for (i, &(l, kind, level, south)) in self.deck_laid.iter().enumerate() {
            let Some(id) = self.deck_art.get(kind, l.piece) else { continue };
            let r = self.atlas.get(id);
            let (x, y) = (l.x * CELL - cam.0 - i32::from(r.ax), l.y * CELL - cam.1 - i32::from(r.ay));
            if !on_canvas(x, y, r.src.w, r.src.h) {
                continue;
            }
            let mut sp = sprite(r, x, y, Flags::default());
            let base = (i32::from(level) * jane_art::terrain::levels::LEVEL_PX).min(255) as u8;
            sp.foot = Some(Foot { y: i16::MAX, see: false, base });
            let y = south * CELL - 1 - cam.1;
            self.standing.push(DrawCmd { y, key: DECK_KEY | (i as u32 & 0x07ff_ffff), sprite: sp, caster: None });
        }
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
        let standing = Span::since(s0, f.sprites.len());
        self.holders.sort_unstable();
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
            let k = if l.kept { k } else { (u32::from(k) * turn.lamp_at(y) / 256) as u8 };
            // On raised ground (MAP.md §6.2) it stands on its ground's height (a console's: 0).
            let base = if f.t8 { 0 } else { ground_base(&self.chunks, &self.terrain, self.zone_cells, (l.x, l.y - 1)) };
            let b = i32::from(base);
            f.lights.push(Light {
                pos: (x, y + if b > 0 { rows_up(b) } else { 0 }),
                height: (i32::from(l.height) + b).min(255) as u8,
                colour: scale(l.colour, k),
                radius: l.radius,
                size: l.size,
                casts: true,
                kind: LightKind::Point,
                // The prop that carries it: its post, its bracket, its cage.
                holder: Some(l.id),
                base,
            });
        }
        let n_props = f.lights.len();
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
                                // Its heights are the chunk's, its base the cell's it is set in.
                                base: if f.t8 {
                                    0
                                } else {
                                    self.terrain.base(
                                        slot,
                                        i32::from(w.x),
                                        i32::from(w.y) - rows_up(i32::from(w.height)),
                                    )
                                },
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
        // The lit windows, the spell glows and her lantern gutter with the lamps.
        if turn.lamp < 256 {
            for l in &mut f.lights[n_props..] {
                // What a unit carries (her lantern, a cast's glow) dips half as deep: units stay
                // drawn through the dark second, so no fight is made blind (NIGHT.md §2.2).
                let carried = l.holder.is_some_and(|h| h & UNIT_KEY != 0);
                let k = turn.lamp_at(l.pos.1);
                let k = if carried { u32::midpoint(k, 256) } else { k };
                l.colour = l.colour.map(|c| (u32::from(c) * k / 256) as u8);
            }
        }
        // A light never shadows what holds it: each holder's key to its sprite, or none.
        let holders = &self.holders;
        for l in &mut f.lights {
            l.holder = l.holder.and_then(|k| holders.binary_search_by_key(&k, |h| h.0).ok().map(|i| holders[i].1));
        }
        // The tier's counts, or fewer where the `max_lights` and `shadows` rows are turned down:
        // the nearest casting lights by one rule on every tier.
        let rows = self.atmos.features;
        let most = max_lights(f.tier).min(usize::from(rows.max_lights));
        // A console's presenter lets every light that may cast cast (PORT.md §13.12: the PSP
        // caches the static shadows and draws the rest per frame); the PC tiers the nearest few.
        let casting = if self.deferred { most } else { usize::from(rows.shadows) };
        pick_lights(&mut f.lights, (cw / 2, ch / 2), most, casting);
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
        let band = turn.band;
        if f.tier > Tier::T0 || sky.ambient.iter().any(|&c| c < 254) || points.len > 0 || !band.is_none() {
            f.passes.push(Pass::Lights {
                ambient: sky.ambient,
                fill: sky.fill,
                sun: sky.sun,
                points,
                casters,
                blocks,
                band,
            });
        }
        // What is lit by the sky after the light pass (the marks on the ground, the fog, the
        // effects in the air) by the band's light at the canvas's middle row.
        let lit = band.at(ch / 2).min(256);
        let dimmed;
        let sky = if lit < 256 {
            let k = |c: Rgb| c.map(|v| (u32::from(v) * lit / 256) as u8);
            dimmed = Sky {
                ambient: k(sky.ambient),
                fill: k(sky.fill),
                sun: sky.sun.map(|s| Directional { colour: k(s.colour), ..s }),
                ..*sky
            };
            &dimmed
        } else {
            sky
        };
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
        // The night's stage term (NIGHT.md §4.6): by the intensity where she stands, coming in
        // behind the turn's band and leaving under dawn's.
        let staged = crate::light::night_grade(sky.post, self.night_q * u32::from(turn.grade) / 256);
        let post = if rows.grade { staged } else { Post { bloom: sky.post.bloom, ..Post::NONE } };
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

/// Water sheets falling down every waterfall in the frame's chunks (MAP.md §2.6, its "draw-time
/// motion"): on each fall cell streaks slide down at the tick's pace, each column its own phase
/// and length, and at a fall's foot spray kicks up and falls back; presentation only, by the
/// tick, the same on every tier (particles). Fewer where the `water` row is off.
fn falls(f: &mut Frame, chunks: &ChunkCache, terrain: &crate::terrain::Terrain, tick: u32, rich: bool) {
    use crate::frame::{PartShape, Particle};
    let p0 = f.parts.len();
    let (cw, ch) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
    for c in &f.chunks {
        let Some((slot, _)) = chunks.find(c.id) else { continue };
        let falls = terrain.falls(slot);
        for &(fx, fy) in falls {
            let (x0, y0) = (c.x + i32::from(fx) * CELL, c.y + i32::from(fy) * CELL);
            if x0 + CELL < 0 || y0 + CELL < 0 || x0 >= cw || y0 - CELL >= ch {
                continue;
            }
            let cells = crate::frame::CHUNK_CELLS;
            let (wx, wy) = (i32::from(c.id.cx) * cells + i32::from(fx), i32::from(c.id.cy) * cells + i32::from(fy));
            let foot = !falls.contains(&(fx, fy + 1));
            let streaks = if rich { 5 } else { 3 };
            for k in 0..streaks {
                let h = jane_core::hash::mix32((wx as u32) << 16 ^ (wy as u32) << 4 ^ k);
                let speed = 2 + (h >> 8) % 2;
                let len = 4 + (h >> 12) % 4;
                let along = (tick.wrapping_mul(speed).wrapping_add(h >> 16)) % (CELL as u32 + len);
                let y = y0 + along as i32 - len as i32;
                if y + len as i32 <= y0 || y >= y0 + CELL {
                    continue;
                }
                f.parts.push(Particle {
                    x: (x0 + 2 + (h % 12) as i32) as i16,
                    y: y.max(y0) as i16,
                    shape: PartShape::Streak { dx: 0, dy: (len as i32).min(y0 + CELL - y.max(y0)) as i8 },
                    colour: [214, 236, 246],
                    alpha: 150 + (h >> 20) as u8 % 70,
                    glow: 0,
                    height: 20,
                });
            }
            if foot {
                // Spray at the foot: drops thrown up a few px and falling back, a slow mist above.
                for k in 0..if rich { 6 } else { 3 } {
                    let h = jane_core::hash::mix32((wx as u32) << 12 ^ (wy as u32) ^ (k + 99) << 24);
                    let period = 14 + h % 8;
                    let t = (tick.wrapping_add(h >> 8)) % period;
                    let up = (t * (period - t) * 9 / (period * period)) as i32;
                    f.parts.push(Particle {
                        x: (x0 + (h >> 4) as i32 % CELL) as i16,
                        y: (y0 + CELL - 2 - up) as i16,
                        shape: PartShape::Dot { size: 1 + (h >> 16) as u8 % 2 },
                        colour: [232, 244, 250],
                        alpha: 200 - (t * 120 / period) as u8,
                        glow: 0,
                        height: 4,
                    });
                }
            }
        }
    }
    if f.parts.len() > p0 {
        f.passes.push(Pass::Particles { layer: Depth::Ground, parts: Span::since(p0, f.parts.len()) });
    }
}

/// How high the ground at zone canvas px `(x, y)` is (MAP.md §6.2): its painted chunk's cell's
/// base, 0 where no chunk is painted or the zone has no levels.
fn ground_base(chunks: &ChunkCache, terrain: &crate::terrain::Terrain, cells: (u32, u32), (x, y): (i32, i32)) -> u8 {
    let (zw, zh) = (cells.0 as i32 * CELL, cells.1 as i32 * CELL);
    if x < 0 || y < 0 || x >= zw || y >= zh {
        return 0;
    }
    let id = ChunkId { cx: (x / CHUNK_PX) as u16, cy: (y / CHUNK_PX) as u16 };
    chunks.find(id).map_or(0, |(s, _)| terrain.base(s, x % CHUNK_PX, y % CHUNK_PX))
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
///
/// On raised ground (MAP.md §6.2) the lift is the ground's height `base` and then the step's over
/// it, `MAX_LIFT` at most over the base: a plateau raises her light as high as it stands.
fn light_lift(under: bool, drawn: i32, base: i32) -> i32 {
    let rel = drawn - base;
    if under || rel <= shadow::RELIEF { base } else { base + rel.min(MAX_LIFT) }
}

/// `0xRRGGBB` as bytes.
/// How high Julie's chalk ring stands on a lamp's post, true px: the height of her hand.
const RING_HEIGHT: i32 = 15;

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A step or a dais she stands on raises her lights (capped), the ground's relief does not,
    /// and under a house's eaves the roof drawn at her feet raises nothing (§1.7).
    #[test]
    fn her_lights_stand_on_a_platform_and_not_on_a_roof_she_is_behind() {
        assert_eq!(light_lift(false, 24, 0), 24, "a platform raises her light");
        assert_eq!(light_lift(false, 90, 0), MAX_LIFT);
        assert_eq!(light_lift(false, shadow::RELIEF, 0), 0);
        assert_eq!(light_lift(true, 57, 0), 0, "a roof in front of her raises nothing");
        assert_eq!(light_lift(false, 44, 40), 40, "a plateau raises it as high as it stands");
        assert_eq!(light_lift(false, 1, 40), 40, "so does a deck over low ground");
        assert_eq!(light_lift(false, 64, 40), 64, "and a step on the plateau over that");
    }
}
