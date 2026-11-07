//! The chunks under the view: a console's county kept across a zone change, the painter's jobs
//! and their landing, the baked chunks and the stand-ins until they come, and what to ask the
//! loader for ahead of the camera.

use super::*;

impl Present {
    /// A console's zone change (from `was`): the county's chunks, its stand-ins' samples and its
    /// terrain set-up are kept while she is in a house or under the ground, so coming out shows
    /// the county as she left it at once (PSP v0.5.0: reading and painting it again held the game
    /// 1.5 to 3.6 s, and ran it at 7 to 10 fps a while after); any other zone starts afresh. The
    /// county's are let go if its tiles changed meanwhile, or the game did.
    pub(super) fn keep_county(&mut self, was: Option<(ZoneId, u32)>, view: &View<'_>) {
        let (zone, seed) = (view.zone(), view.seed());
        let key = |z: ZoneId| z as u32 + 1;
        if was.is_some_and(|w| w.0 == ZoneId::County) && zone != ZoneId::County {
            // Out of the county: its samples put by.
            self.county_samples = self.samples.as_mut().map(core::mem::take);
        }
        self.chunks.set_zone(key(zone));
        if zone == ZoneId::County {
            let now = (seed, view.ground_digest());
            if self.county_kept != Some(now) {
                self.chunks.drop_zone(key(zone));
                self.county_samples = None;
            }
            self.county_kept = Some(now);
            if let (Some(s), Some(kept)) = (self.samples.as_mut(), self.county_samples.take()) {
                *s = kept;
            } else if let Some(s) = self.samples.as_mut() {
                s.clear();
            }
            // Its kept chunks' night overlays laid again (a stage may have turned indoors).
            self.renight = true;
        } else {
            self.chunks.drop_zone(key(zone));
            if let Some(s) = self.samples.as_mut() {
                s.clear();
            }
        }
    }

    /// Chunks on screen the painter has not reached yet (swatches showing): a console gives its
    /// painter more time while there are (PORT.md §13.12).
    pub fn chunks_waiting(&self) -> usize {
        self.swatched
    }

    /// Of [`chunks_waiting`](Self::chunks_waiting), those that show mostly flat colour (their
    /// tiles not sampled yet): the blocks the stand-in's samples are there to hide.
    pub fn chunks_mush(&self) -> usize {
        self.mush
    }

    /// Cells of ground a console's stand-in has to draw with (a test).
    pub fn samples_held(&self) -> usize {
        self.samples.as_ref().map_or(0, stand_in::Samples::held)
    }

    /// The chunk most wanted, as a job holding the painter and a snapshot of the zone round it;
    /// `None` when nothing wants painting or a job is out.
    pub fn take_paint_job(&mut self, view: &View<'_>) -> Option<crate::terrain::PaintJob> {
        if !self.deferred || self.job_out.is_some() {
            return None;
        }
        let id = self.job_next.take()?;
        let job = self.terrain.job(view, id)?;
        self.job_out = Some((id, (view.zone(), view.seed())));
        self.job_stale = false;
        Some(job)
    }

    /// A job back: its chunk laid into the cache if the zone is still the one it was painted for
    /// and its tiles have not changed since, and the painter home again.
    pub fn land(&mut self, job: crate::terrain::PaintJob) {
        let id = job.id;
        let (mut work, size) = job.into_parts();
        let fresh = self.job_out.is_some_and(|(j, key)| j == id && Some(key) == self.zone) && !self.job_stale;
        self.job_out = None;
        // A chunk the view has left (the job took a while) is let go if every slot is wanted
        // this tick: landing it would evict one on screen.
        let need = self.chunks.need(id);
        if fresh && need != Need::Nothing && (need != Need::Missing || self.chunks.has_spare(self.tick)) {
            let (outside, now) = (self.frame.clear, self.tick);
            let (chunks, terrain, layers) = (&mut self.chunks, &mut self.terrain, &mut self.frame.layers);
            let flat = &mut self.flat_cells;
            chunks.want(id, now, layers, false, |slot, l| {
                terrain.land(&mut work, size, id, slot, outside, l);
                if let Some(f) = flat.get_mut(usize::from(slot)) {
                    *f = 0;
                }
            });
            if self.samples.is_some() && self.fresh.len() < self.fresh.capacity() {
                self.fresh.push(id);
            }
        }
        self.terrain.home_again(work);
    }

    /// A console's county is baked (PLAN-PSP-SPEED.md §5, P1c): a pack of `cols` x `rows` chunks
    /// a loader reads. From now on a county chunk whose tiles and spans are the blueprint's is
    /// asked of the loader ([`baked_wants`](Self::baked_wants)) and landed by
    /// [`land_baked`](Self::land_baked); the painter paints the rest, and every other zone.
    pub fn set_baked(&mut self, cols: u16, rows: u16) {
        self.baked = Some(crate::chunk_pack::Ground::new(cols, rows));
    }

    /// What the loader is asked for, as the last tick left it: the chunks to land, most wanted
    /// first, then those to read ahead of her (each list short; both empty off the county).
    pub fn baked_wants(&self) -> (&[ChunkId], &[ChunkId]) {
        self.baked.as_ref().map_or((&[], &[]), |b| (&b.wants, &b.ahead))
    }

    /// Whether chunk `id` is the bake's to land now: the county's, its tiles and spans the
    /// blueprint's, the loader not failed on it.
    fn bakes(&self, id: ChunkId) -> bool {
        self.zone.is_some_and(|z| z.0 == ZoneId::County) && self.baked.as_ref().is_some_and(|b| b.holds(id))
    }

    /// The loader could not give chunk `id` (a failed read, a bad blob): the painter's for good.
    pub fn baked_failed(&mut self, id: ChunkId) {
        if let Some(b) = self.baked.as_mut() {
            b.fail(id);
        }
    }

    /// A decoded chunk landed (P1c): its layers swapped into the cache's slot, nothing copied, so
    /// `landing` comes back holding that slot's old layers (the loader's next spare). Not landed
    /// (false) if the chunk is no longer the bake's, or not wanted, or wanting a slot every one
    /// of which this tick wants.
    pub fn land_baked(&mut self, landing: &mut crate::chunk_pack::Landing) -> bool {
        let id = landing.id;
        let need = self.chunks.need(id);
        if !self.bakes(id) || need == Need::Nothing || (need == Need::Missing && !self.chunks.has_spare(self.tick)) {
            return false;
        }
        let now = self.tick;
        let (chunks, terrain, layers) = (&mut self.chunks, &mut self.terrain, &mut self.frame.layers);
        let (flat, capped) = (&mut self.flat_cells, &mut self.capped);
        chunks.want(id, now, layers, false, |slot, l| {
            core::mem::swap(l, &mut landing.layers);
            terrain.land_baked(slot, id, &mut landing.side);
            if let Some(f) = flat.get_mut(usize::from(slot)) {
                *f = 0;
            }
            if let Some(c) = capped.get_mut(usize::from(slot)) {
                *c = false;
            }
        });
        if self.samples.is_some() && self.fresh.len() < self.fresh.capacity() {
            self.fresh.push(id);
        }
        self.baked_landed += 1;
        true
    }

    /// Paints the chunks under the view (and a little round it) that want it: at most
    /// [`LAND_PER_TICK`] by the terrain painter, those on screen and nearest the middle first;
    /// one with nothing to show yet takes its swatches meanwhile.
    pub(super) fn paint_chunks(&mut self, view: &View<'_>) {
        let cam = (self.camera.pos.0 >> FX_TO_CANVAS, self.camera.pos.1 >> FX_TO_CANVAS);
        // Ahead of her: the band reaches a further chunk on each side the view is moving toward,
        // so the cache, sized to the band (PLAY-PLAN.md §7), has new ground painted before it
        // shows.
        let (dx, dy) = (self.camera.pos.0 - self.camera.prev.0, self.camera.pos.1 - self.camera.prev.1);
        // A console casts no sun shadows (`Features::c2`) and keeps few slots: no casting band.
        let band = if self.deferred { Margins::uniform(0) } else { self.margins };
        let mut ahead = band.grow(CHUNK_AHEAD);
        let lead = |d: i32, toward: bool| if toward && d != 0 { MOVE_AHEAD } else { 0 };
        ahead.left += lead(dx, dx < 0);
        ahead.right += lead(dx, dx > 0);
        ahead.top += lead(dy, dy < 0);
        ahead.bottom += lead(dy, dy > 0);
        let Some((cx0, cy0, cx1, cy1)) = self.chunk_range(cam, ahead) else {
            return;
        };
        let reach = ahead;
        self.chunks.fit(((cx1 - cx0 + 1) * (cy1 - cy0 + 1)) as usize);
        // A console's baked county: what no longer shows as baked (a tile or a span changed).
        let county = view.zone() == ZoneId::County;
        if let Some(b) = self.baked.as_mut().filter(|_| county) {
            b.read(view.flags_generation(), view.tile_deltas(), view.spans_changed());
        }
        // The chunks landed since the last tick give the stand-ins their cells (a console's).
        if let Some(samples) = self.samples.as_mut() {
            for id in self.fresh.drain(..) {
                if let (Some((slot, _)), Need::Nothing) = (self.chunks.find(id), self.chunks.need(id)) {
                    let l = &self.frame.layers[usize::from(slot)];
                    samples.harvest(id, self.zone_cells, |x, y| view.tile(x, y), l);
                }
            }
        }
        let (sx0, sy0, sx1, sy1) = self.chunk_range(cam, Margins::uniform(0)).unwrap_or((cx0, cy0, cx1, cy1));
        let mid = (cam.0 + i32::from(self.canvas.0) / 2, cam.1 + i32::from(self.canvas.1) / 2);
        let (cells, outside, now) = (self.zone_cells, self.frame.clear, self.tick);
        self.wants.clear();
        // The band, each chunk with its priority: on screen first, nearest the middle first. A
        // console's band is held to its slots, so no chunk wanted this tick can be evicted by
        // another (the LRU's ties went to slot 0, whatever it held: an on-screen chunk swatched
        // again, 105 ms each on the PSP, PLAN-psp-v041 B). Off screen it is ranked by how soon it
        // shows, so the ground she walks onto comes first (P1c: by its gap to the view; a point
        // a chunk ahead lost the row a turn brings on at the side).
        let (sdx, sdy) = (dx.signum(), dy.signum());
        self.band.clear();
        let (vx1, vy1) = (cam.0 + i32::from(self.canvas.0), cam.1 + i32::from(self.canvas.1));
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let on = (sx0..=sx1).contains(&cx) && (sy0..=sy1).contains(&cy);
                let d = if on || !self.deferred {
                    let (px, py) = (cx * CHUNK_PX + CHUNK_PX / 2 - mid.0, cy * CHUNK_PX + CHUNK_PX / 2 - mid.1);
                    px.unsigned_abs() + py.unsigned_abs()
                } else {
                    // A console's: by how far the chunk is from showing, its gap to the view on
                    // each side, a gap the view is moving away from four times as far (a turn
                    // brings a row on at the side, not only ahead).
                    let gap = |lo: i32, hi: i32, v0: i32, v1: i32, d: i32| {
                        let (before, after) = ((v0 - hi).max(0), (lo - v1).max(0));
                        let toward = (after > 0 && d > 0) || (before > 0 && d < 0);
                        (before + after) as u32 * if toward { 1 } else { 4 }
                    };
                    let (x0, y0) = (cx * CHUNK_PX, cy * CHUNK_PX);
                    gap(x0, x0 + CHUNK_PX, cam.0, vx1, sdx) + gap(y0, y0 + CHUNK_PX, cam.1, vy1, sdy)
                };
                let d = d.min(0x00ff_ffff);
                self.band.push((u32::from(!on) << 24 | d, ChunkId { cx: cx as u16, cy: cy as u16 }));
            }
        }
        if self.deferred {
            self.band.sort_unstable_by_key(|b| (b.0, b.1.cy, b.1.cx));
            self.band.truncate(self.chunks.most());
        }
        let (mut shown, mut swatched, mut on_screen) = (0, 0, 0);
        for &(key, id) in &self.band {
            self.chunks.touch(id, now);
            let on = key >> 24 == 0;
            on_screen += usize::from(on);
            let need = self.chunks.need(id);
            if need == Need::Nothing {
                continue;
            }
            shown += usize::from(on);
            swatched += usize::from(on && need != Need::Repaint);
            self.wants.push((key, id, need));
        }
        self.wants.sort_unstable_by_key(|w| (w.0, w.1.cy, w.1.cx));
        // A console's view that jumped (a teleport, a fast travel: the view moved half a chunk
        // or more in a tick, or nothing on screen is painted, and more than half of what shows is
        // swatches) is painted here, as a zone's first view is, not left to the jobs. A walk the
        // painter's thread is behind is left to it (PORT.md §13.13: painting here held a tick
        // 50 to 110 ms where she walked the county at night); the console gives it more time.
        let moved = dx.unsigned_abs().max(dy.unsigned_abs());
        let leap = moved >= ((CHUNK_PX / 2) << FX_TO_CANVAS) as u32;
        // Nothing on screen painted counts only while the view moves faster than any walk (a
        // camera easing after a travel): on the real PSP a painter behind a sprint left a view of
        // swatches, and painting it here held the tick a second (PORT.md §13.13).
        let fast = moved >= (FAST_VIEW << FX_TO_CANVAS) as u32;
        let jumped = self.deferred && swatched * 2 > on_screen && (leap || (fast && swatched == on_screen));
        self.view_jumps += u32::from(jumped);
        self.swatched = swatched;
        // A console coming into a zone, or whose view jumped, with samples of its ground (back on
        // the county, a travel across it) stands in for what it did not keep rather than paint
        // it all now (PSP v0.5.0: one to three seconds held). Anywhere but the county there are
        // no samples, and the same whole-view paint held the Library's entry 33 ms on the host.
        // The chunk under her there was 19 ms on its own, more than a quarter of that, so none
        // of it is painted here: the view stands in flat and the painter takes it, nearest the
        // middle first.
        let entered = core::mem::take(&mut self.entered);
        let away = self.deferred && view.zone() != ZoneId::County;
        let now_all = (entered || jumped) && !(self.deferred && self.samples_held() > 0) && !away;
        let budget = match (now_all, self.deferred) {
            _ if !self.terrain.home() => 0,
            (true, _) => shown.max(LAND_PER_TICK),
            (false, true) => 0,
            (false, false) => LAND_PER_TICK,
        };
        // A console's swatches softened (a painter behind a walk shows them a moment).
        let soft = self.deferred;
        let (chunks, terrain, layers) = (&mut self.chunks, &mut self.terrain, &mut self.frame.layers);
        let (samples, flat) = (&mut self.samples, &mut self.flat_cells);
        if soft && flat.len() < chunks.most() {
            flat.resize(chunks.most(), 0);
        }
        let capped = &mut self.capped;
        if soft && capped.len() < chunks.most() {
            capped.resize(chunks.most(), false);
        }
        // A console samples one stand-in a tick (about 20 ms each on the PSP); the rest are
        // flat this tick and upgraded on later ones, nearest the middle first.
        let mut sampled = 0u32;
        let mut upgrades = 0u32;
        // While the painter is behind (stand-ins on screen), a console's stand-in for the band's
        // nearest chunk ahead not yet on screen, one a tick: a run brings a row of them on at
        // once, and three in a tick hold it three times as long.
        let mut ahead = usize::from(soft && samples.is_some() && swatched > 0);
        // A baked chunk comes from the loader in a tick or two: it takes no stand-in ahead of
        // her and no upgrade of a flat one, only one on screen meanwhile (P1c).
        let baked = self.baked.as_ref().filter(|_| county);
        let bakes = |id: ChunkId| baked.is_some_and(|b| b.holds(id));
        for (i, &(key, id, need)) in self.wants.iter().enumerate() {
            let on = key >> 24 == 0;
            let mut none = 0;
            let ahead = if bakes(id) && !on { &mut none } else { &mut ahead };
            self.stand_ins_ahead += u32::from(!on && *ahead > 0 && need == Need::Missing && i >= budget);
            let upgrade = !bakes(id)
                && soft
                && on
                && i >= budget
                && sampled == 0
                && samples.is_some()
                && need == Need::Paint
                && chunks.find(id).is_some_and(|(slot, _)| capped.get(usize::from(slot)).copied().unwrap_or(false));
            if upgrade {
                chunks.mark_stale(id);
                upgrades += 1;
            }
            if i < budget {
                chunks.want(id, now, layers, false, |slot, l| {
                    terrain.paint(view, id, slot, outside, l);
                    if let Some(f) = flat.get_mut(usize::from(slot)) {
                        *f = 0;
                    }
                    if let Some(c) = capped.get_mut(usize::from(slot)) {
                        *c = false;
                    }
                    if let Some(s) = samples.as_mut() {
                        s.harvest(id, cells, |x, y| view.tile(x, y), l);
                    }
                });
            } else if upgrade || (need == Need::Missing && (on || *ahead > 0)) {
                *ahead -= usize::from(!on && !upgrade);
                self.stand_ins_shown += u32::from(on && need == Need::Missing);
                chunks.want(id, now, layers, true, |slot, l| {
                    terrain.swatched(slot, l);
                    if soft && l.is_t8() {
                        // Cheap, standing nothing (no relief, no blocks): the zone's own ground
                        // as the painter has laid it nearby, sampled, and flat colour where it
                        // has laid none of a tile yet.
                        let tile = |x, y| view.tile(x, y);
                        let mut was_capped = false;
                        let n = match samples.as_mut() {
                            Some(s) if sampled == 0 => {
                                sampled += 1;
                                stand_in::paint_chunk_sampled_t8(id, cells, outside, tile, s, l)
                            }
                            Some(_) => {
                                was_capped = true;
                                stand_in::paint_chunk_flat_t8(id, cells, outside, tile, l);
                                (crate::frame::CHUNK_CELLS * crate::frame::CHUNK_CELLS) as u16
                            }
                            None => {
                                stand_in::paint_chunk_flat_t8(id, cells, outside, tile, l);
                                (crate::frame::CHUNK_CELLS * crate::frame::CHUNK_CELLS) as u16
                            }
                        };
                        if let Some(f) = flat.get_mut(usize::from(slot)) {
                            *f = n;
                        }
                        if let Some(c) = capped.get_mut(usize::from(slot)) {
                            *c = was_capped;
                        }
                        terrain.stand_flat(slot);
                        return;
                    }
                    if soft {
                        stand_in::paint_chunk_soft(id, cells, outside, |x, y| view.tile(x, y), l);
                    } else {
                        stand_in::paint_chunk(id, cells, outside, |x, y| view.tile(x, y), l);
                    }
                    terrain.stand(slot, l);
                });
            }
        }
        self.sampled_this_tick = sampled;
        self.sampled_max_tick = self.sampled_max_tick.max(sampled);
        self.stand_in_upgrades += upgrades;
        if self.deferred {
            // Skip a chunk this tick already painted: the job is the next one still wanted,
            // so the painter does not paint the same chunk again. A baked one is the loader's.
            let out = self.job_out.map(|j| j.0);
            let (chunks, baked) = (&self.chunks, self.baked.as_ref().filter(|_| county));
            let bakes = |id: ChunkId| baked.is_some_and(|b| b.holds(id));
            self.job_next = self
                .wants
                .iter()
                .map(|w| w.1)
                .find(|&id| Some(id) != out && chunks.need(id) != Need::Nothing && !bakes(id));
            self.ask_loader(cam, reach, (dx, dy));
        }
        // What shows as blocks of colour: a stand-in on screen mostly unsampled.
        self.mush = 0;
        if soft {
            for &(_, id) in self.band.iter().filter(|b| b.0 >> 24 == 0) {
                if let (Need::Paint, Some((slot, _))) = (self.chunks.need(id), self.chunks.find(id)) {
                    let waiting = self.capped.get(usize::from(slot)).copied().unwrap_or(false);
                    self.mush += usize::from(
                        !waiting && self.flat_cells.get(usize::from(slot)).is_some_and(|&n| n >= MUSH_CELLS),
                    );
                }
            }
        }
    }

    /// The loader's lists for the ticks to come (a console's baked county, P1c): the band's
    /// chunks it holds not landed yet, in the band's order, at most [`BAKED_WANTS`]; then at most
    /// [`BAKED_AHEAD`] it holds beyond the band (past the slots the band is held to, or a chunk
    /// further along her way), nearest the ground she is heading for first, to read ahead.
    fn ask_loader(&mut self, cam: (i32, i32), reach: Margins, (dx, dy): (i32, i32)) {
        let mut far = reach;
        far.left += if dx < 0 { CHUNK_PX } else { 0 };
        far.right += if dx > 0 { CHUNK_PX } else { 0 };
        far.top += if dy < 0 { CHUNK_PX } else { 0 };
        far.bottom += if dy > 0 { CHUNK_PX } else { 0 };
        let range = self.chunk_range(cam, far);
        let county = self.zone.is_some_and(|z| z.0 == ZoneId::County);
        let (chunks, band, wants) = (&self.chunks, &self.band, &self.wants);
        let Some(b) = self.baked.as_mut() else { return };
        b.wants.clear();
        b.ahead.clear();
        let Some((x0, y0, x1, y1)) = range.filter(|_| county) else { return };
        for &(_, id, _) in wants {
            if b.wants.len() < BAKED_WANTS && b.holds(id) && chunks.need(id) != Need::Nothing {
                b.wants.push(id);
            }
        }
        let mid = (cam.0 + i32::from(self.canvas.0) / 2, cam.1 + i32::from(self.canvas.1) / 2);
        let to = (mid.0 + dx.signum() * 2 * CHUNK_PX, mid.1 + dy.signum() * 2 * CHUNK_PX);
        let mut far_ones: [(u32, ChunkId); 32] = [(u32::MAX, ChunkId { cx: 0, cy: 0 }); 32];
        let mut n = 0;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let id = ChunkId { cx: cx as u16, cy: cy as u16 };
                if n == far_ones.len() || band.iter().any(|w| w.1 == id) || !b.holds(id) || chunks.find(id).is_some() {
                    continue;
                }
                let (px, py) = (cx * CHUNK_PX + CHUNK_PX / 2 - to.0, cy * CHUNK_PX + CHUNK_PX / 2 - to.1);
                far_ones[n] = (px.unsigned_abs() + py.unsigned_abs(), id);
                n += 1;
            }
        }
        far_ones[..n].sort_unstable_by_key(|f| (f.0, f.1.cy, f.1.cx));
        b.ahead.extend(far_ones[..n].iter().take(BAKED_AHEAD).map(|f| f.1));
    }

    /// The chunks under a view whose top-left is `cam` (canvas px), grown by `m` px each side and
    /// held to the zone: inclusive `(cx0, cy0, cx1, cy1)`, or `None` for an empty zone.
    pub(super) fn chunk_range(&self, cam: (i32, i32), m: Margins) -> Option<(i32, i32, i32, i32)> {
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
}
