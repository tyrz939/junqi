//! `Present::tick` (PRESENTATION.md §1.11): one tick of presentation, reading the `View` and
//! this tick's events; with it the emotes, the camera's area and the units it keeps.

use super::*;

impl Present {
    /// One tick of presentation: reads the view and this tick's events (all of them, or this
    /// seat's; others are filtered out here).
    pub fn tick(&mut self, view: &View<'_>, events: &[Event]) {
        let clock = self.clock;
        let now = || clock.map_or(0, |c| c());
        let mut t = now();
        let mut lap = |prof: &mut [u32; 12], k: usize| {
            let n = now();
            prof[k] = prof[k].wrapping_add(n.wrapping_sub(t));
            t = n;
        };
        self.tick = self.tick.wrapping_add(1);
        let key = (view.zone(), view.seed());
        if self.zone != Some(key) {
            let was = self.zone.replace(key);
            self.fresh.clear();
            if self.deferred {
                self.keep_county(was, view);
            } else {
                self.chunks.drop_all();
                if let Some(s) = self.samples.as_mut() {
                    s.clear();
                }
                // A small zone (a house, a crypt) keeps only the slots it has chunks for; the
                // county grows them back as its view wants (PLAY-PLAN.md §7).
                let (w, h) = view.size();
                let span = |c: u32| (c as usize * CELL as usize).div_ceil(CHUNK_PX as usize);
                self.chunks.shrink(&mut self.frame.layers, (span(w) * span(h)).min(RESERVE));
            }
            self.terrain.zone(view);
            self.entered = true;
            self.camera.reset();
            self.units.clear();
            self.atmos.zone(view);
            self.fx.zone(view);
            self.lessons.zone(view);
            self.ambient.zone();
            self.lived = lived_houses(view, self.terrain.houses());
            self.prop_cache.clear();
            self.emotes.clear();
            self.walls.clear();
        }
        // A room's windows lay daylight on its floor and are dark at night (ART-PLAN M4): its
        // chunks are painted again when the lamps come on or go off.
        if self.terrain.set_daylight(!view.lamps_lit()) {
            let (w, h) = view.size();
            self.chunks.invalidate(Rect::new(0, 0, w as i32, h as i32));
            if let Some(s) = self.samples.as_mut() {
                s.clear();
            }
        }
        self.zone_cells = view.size();
        // The spans as they stand (MAP.md §2.5): a deck mended or broken repaints the shade by it.
        if let Some(r) = self.terrain.read_spans(view) {
            self.chunks.invalidate(r);
        }
        self.read_decks(view);
        let kit = &self.kit;
        self.cues.zone(view, |s| kit.glass(s));
        self.hour = view.hour();
        self.hurt.clear();
        self.struck.clear();
        for e in events_for(events, view.me()) {
            self.marks_stale |= moves_marks(&e.kind);
            match e.kind {
                // A tile reaches a few cells round it in what the painter draws.
                EventKind::Tiles(r) => {
                    self.job_stale = true;
                    self.prop_cache.clear();
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
        let her = (body.pos.x.0, body.pos.y.0);
        // A console's view goes with her when she is moved far in a tick (a travel within the
        // zone, a respawn) rather than easing over the ground between, every chunk of which it
        // would have to stand in for on the way.
        if self.deferred && self.her_was.is_some_and(|w| (w.0 - her.0).abs().max((w.1 - her.1).abs()) > CAMERA_SNAP_FX)
        {
            let lock = self.camera.lock;
            self.camera.reset();
            self.camera.lock = lock;
        }
        self.her_was = Some(her);
        self.camera.tick(her, self.zone_cells, self.canvas, self.tick);
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
        lap(&mut self.prof, 7);
        self.read_units(view, area);
        lap(&mut self.prof, 0);
        self.read_emotes(view, area);
        lap(&mut self.prof, 1);
        self.read_props(view, area);
        lap(&mut self.prof, 8);
        self.read_lights(view, area);
        lap(&mut self.prof, 9);
        self.paint_chunks(view);
        lap(&mut self.prof, 2);
        self.against_walls();
        lap(&mut self.prof, 3);
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
        self.night_tick(view);
        self.margins = crate::shadow::cast_margins(self.sky.sun.as_ref());
        self.atmos.tick(view, self.tick);
        lap(&mut self.prof, 4);
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
        lap(&mut self.prof, 5);
        // The ambient life (ART-PLAN M1): the chimneys that smoke and the lamps moths circle, then
        // who is where this tick.
        let atlas = &self.atlas;
        let chimneys = self.props.iter().filter(|p| p.smokes).map(|p| {
            let r = atlas.get(p.look);
            let top = p.y + p.h - rows_up(i32::from(r.top)) + 1;
            (p.id, p.x + p.w / 2, top, p.y + p.h)
        });
        let lamps = self
            .lights
            .iter()
            .filter(|l| l.height >= 20 && !l.wrong)
            .map(|l| (l.id, l.x, l.y - rows_up(i32::from(l.height))));
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
            clock: self.clock,
        };
        self.ambient.tick(view, &ctx, &self.creatures);
        lap(&mut self.prof, 6);
    }

    /// What the ambient layer's parts took (`ambient::Ambient::prof`), summed until read; and
    /// let go of.
    pub fn take_ambient_prof(&mut self) -> [u32; 4] {
        core::mem::take(&mut self.ambient.prof)
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
                let night = !(6..21).contains(&(view.clock().0 / jane_core::num::TICKS_PER_HOUR % 24));
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
        let hour = view.clock().0 / jane_core::num::TICKS_PER_HOUR % 24;
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
        // Each one's quest mark (a dialogue tree walked): on a console asked again every
        // `MARKS_EVERY` ticks, its own tick by its id, as the things' (`read_props`).
        let every = if self.deferred { MARKS_EVERY } else { 1 };
        if self.marks_stale || every == 1 {
            self.unit_marks.clear();
        }
        let tick = self.tick;
        let (marks, marks_next) = (&self.unit_marks, &mut self.unit_marks_next);
        marks_next.clear();
        for uv in view.units_in(area) {
            let u = uv.unit;
            let id = u.id.get();
            let kept = marks.binary_search_by_key(&id, |m| m.0).ok().map(|i| marks[i].1);
            let asked = match kept {
                Some(m) if (id.wrapping_add(tick)) % every != 0 => m,
                _ => view.quest_mark(u),
            };
            marks_next.push((id, asked));
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
                    // Under a roof's eaves, or under a whole deck it is not on (MAP.md §2.5): what
                    // is drawn at its feet is over it, not a step it stands on.
                    let (x, y) = u.pos.cell();
                    view.tile(x, y).is_roof()
                        || u.on_span.is_none()
                            && view.spans().iter().enumerate().any(|(i, s)| {
                                s.rect.contains(x, y) && view.span_whole(i as jane_core::blueprint::SpanIx)
                            })
                },
                mark: if talking_to == Some(u.id) { None } else { asked },
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
                base: if view.has_levels() || u.on_span.is_some() {
                    (i32::from(view.unit_level(u)) * jane_art::terrain::levels::LEVEL_PX).min(255) as u8
                } else {
                    0
                },
                deck: u.on_span.is_some(),
                hop: view.has_levels() && view.hopping(u),
                hop_at: match old {
                    Some(o) if o.hop => o.hop_at,
                    _ => self.tick,
                },
            });
        }
        self.units_next.sort_unstable_by_key(|r| r.id);
        core::mem::swap(&mut self.units, &mut self.units_next);
        self.unit_marks_next.sort_unstable_by_key(|m| m.0);
        core::mem::swap(&mut self.unit_marks, &mut self.unit_marks_next);
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
