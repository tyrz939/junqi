//! `Lister::new` and the build (PORT.md §13.12): the presenter's refs resolved to the pack's,
//! the patches' arena, and `build_with`, which walks the frame's passes in order and lays each
//! (the GE pass marks, the palette grade). `build_with` is one long fn and is left whole.

use alloc::vec::Vec;

use jane_present::atlas::SpriteRef;
use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, Pass, Tint};

use super::lightmap::{band_strips, dim};
use super::{
    Blend, IDENTITY, LAMP_RELIEFS, Lister, Mode, NoPx, ORDERED, PATCH_FIRST, PATCH_WORDS, PagePx, Palette, QUADS_FIRST,
    Quad, QuadBuf, StripTex, Target, Tex, abgr, atmos_fx, fx, order, rgba,
};
use crate::capture::pass as P;
use crate::pack::Pack;

impl Lister {
    /// Over the presenter's sprite table (`Present::sprites().refs`) and the pack's.
    pub fn new(refs: &[SpriteRef], pack: &Pack) -> Lister {
        let mut by_pos: Vec<(u8, u16, u16, u16)> =
            refs.iter().enumerate().map(|(i, r)| (r.page, r.src.y, r.src.x, i as u16)).collect();
        by_pos.sort_unstable();
        let targets = refs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let link = pack.refs.get(i)?;
                let rec = pack.recs.get(link.rec as usize)?;
                (rec.page != u16::MAX && rec.w > 0 && rec.h > 0).then_some(Target {
                    cat: rec.cat,
                    page: rec.page,
                    u: rec.u,
                    v: rec.v,
                    w: rec.w,
                    h: rec.h,
                    tx: r.ax - link.ax,
                    ty: r.ay - link.ay,
                })
            })
            .collect();
        Lister {
            by_pos,
            rects: refs.iter().map(|r| (r.src.x, r.src.y)).collect(),
            tops: refs.iter().map(|r| r.top).collect(),
            targets,
            quads: QuadBuf::with_capacity(QUADS_FIRST),
            chunks: Vec::with_capacity(32),
            placed: Vec::with_capacity(32),
            relief_rects: Vec::new(),
            relief_read: None,
            water_rects: Vec::new(),
            water_cover: Vec::with_capacity(24),
            #[cfg(test)]
            water_loose: false,
            stencil_from_clear: None,
            patches: Vec::new(),
            patch_px: Vec::new(),
            patches_skipped: 0,
            arena_failed: false,
            polys_skipped: 0,
            bands_skipped: 0,
            light: crate::light::LightMap::default(),
            relief: None,
            has_normals: pack.pages.iter().map(|p| p.normal.is_some()).collect(),
            lamp_reliefs: Vec::with_capacity(LAMP_RELIEFS),
            has_glow: pack.pages.iter().map(|p| p.glow.is_some()).collect(),
            glows: QuadBuf::with_capacity(64),
            polys: Vec::with_capacity(256),
            shadow_levels: Vec::new(),
            caster_px: Vec::new(),
            masks: Vec::new(),
            mask_clock: 0,
            rows_kept: Vec::new(),
            rows_hash: Vec::new(),
            rows_clock: 0,
            shadow_runs: 0,
            slab_count: 0,
            clock: None,
            prof: [0; 13],
            slabs: Vec::new(),
            coarse: Vec::new(),
            lamp_marks: QuadBuf::default(),
            still: Vec::new(),
            fence_rects: Vec::new(),
            fixed: Vec::new(),
            moving: Vec::new(),
            near: Vec::new(),
            lamps: crate::lamps::LampCache::default(),
            effects: fx::ALL,
            atmos_off: 0,
            palette_grade: false,
            palette: None,
            night: 0,
            clear: 0xff00_0000,
            marks: Vec::with_capacity(32),
            sub_marks: Vec::with_capacity(16),
            misses: 0,
            strips: Vec::with_capacity(32),
            verts: Vec::with_capacity(512),
            cluts: Vec::new(),
            cluts_gen: 0,
            grade: crate::grade::Grade::default(),
            sky: None,
            fog_clut: [0; 256],
            noise_clut: [0; 256],
            water_dim: QuadBuf::default(),
            weather: None,
            sun: None,
            ambient: [255; 3],
            fill: [255; 3],
            order_passes: 1 << P::SPRITES,
            order: order::Order::default(),
            w: 480,
            h: 272,
        }
    }

    /// The presenter's ref whose rect holds `(x, y)` of `page`: the shelf (rects packed in rows
    /// at one `y`) the last at or above `y`, then in it the last at or left of `x`.
    pub(super) fn find(&self, page: u8, x: u16, y: u16) -> Option<usize> {
        let at = self.by_pos.partition_point(|e| (e.0, e.1) <= (page, y));
        let shelf = self.by_pos.get(at.checked_sub(1)?)?;
        if shelf.0 != page {
            return None;
        }
        let row = shelf.1;
        let i = self.by_pos.partition_point(|e| (e.0, e.1, e.2) <= (page, row, x)).checked_sub(1)?;
        let e = self.by_pos[i];
        (e.0 == page && e.1 == row).then_some(usize::from(e.3))
    }

    /// The patches' arena: 1 once it is reserved (the first patch wanted; 0 until), and its
    /// bytes. Flat from then on, whatever a frame lays.
    pub fn patch_bytes(&self) -> (usize, usize) {
        (usize::from(self.patch_px.capacity() > 0), self.patch_px.capacity() * 4)
    }

    /// The patches' arena able to hold `end` words: reserved as the frames need it (doubled from
    /// [`PATCH_FIRST`] to [`PATCH_WORDS`], what it holds kept) and then kept, so a scene with a
    /// patch or two costs 16 KB and a wood of them at most 128; false when the heap cannot give
    /// it (it is asked once a frame).
    pub(super) fn arena(&mut self, end: usize) -> bool {
        if self.patch_px.capacity() >= end {
            return true;
        }
        if self.arena_failed {
            return false;
        }
        let target = end.next_power_of_two().clamp(PATCH_FIRST, PATCH_WORDS);
        if self.patch_px.try_reserve_exact(target - self.patch_px.len()).is_ok() {
            return true;
        }
        self.arena_failed = true;
        false
    }

    /// What this frame left out for want of room: quads (any of the buffers at [`QUADS_MOST`],
    /// or the heap unable to grow one, and the sun's bands past [`SUN_QUADS_MOST`]), polygons and
    /// patches. The frame is the poorer; the program still runs.
    pub fn left_out(&self) -> (u32, u32, u32) {
        let q = self.quads.dropped + self.glows.dropped + self.lamp_marks.dropped + self.water_dim.dropped;
        (q + self.bands_skipped, self.polys_skipped, self.patches_skipped)
    }

    /// Builds the frame's quads.
    pub fn build(&mut self, frame: &Frame) -> &[Quad] {
        self.build_with(frame, &mut NoPx)
    }

    /// Builds the frame's quads, reading casters' texels through `px`.
    pub fn build_with(&mut self, frame: &Frame, px: &mut dyn PagePx) -> &[Quad] {
        self.shadow_runs = 0;
        self.lamp_reliefs.clear();
        self.slab_count = 0;
        self.lamps.begin();
        self.glows.clear();
        self.polys.clear();
        self.quads.clear();
        self.marks.clear();
        self.sub_marks.clear();
        self.stencil_from_clear = None;
        self.palette = None;
        self.night = frame.night.min(4);
        self.chunks.clear();
        self.placed.clear();
        self.relief_read = None;
        self.patches.clear();
        self.patch_px.clear();
        (self.patches_skipped, self.polys_skipped, self.bands_skipped, self.arena_failed) = (0, 0, 0, false);
        self.quads.dropped = 0;
        self.glows.dropped = 0;
        self.lamp_marks.dropped = 0;
        self.water_dim.dropped = 0;
        self.misses = 0;
        self.strips.clear();
        self.verts.clear();
        self.sky = None;
        self.weather = None;
        self.sun = None;
        self.ambient = [255; 3];
        (self.w, self.h) = (i32::from(frame.canvas.0), i32::from(frame.canvas.1));
        self.clear = abgr(frame.clear);
        // The sun, read ahead: the ground sprites, drawn before its pass, take their relief too.
        self.relief = frame.passes.iter().filter(|_| self.effects & fx::RELIEF != 0).find_map(|p| match *p {
            Pass::Silhouettes { sun, shade, .. } => {
                crate::normals::clut(&sun, jane_present::shadow::shade_at(shade, sun.strength))
            }
            _ => None,
        });
        let mut lit = false;
        let mut watered = false;
        for pass in &frame.passes {
            // The water and the wet ground as T1 draws them, over the light (the sky they mirror
            // is not lit by the ground's light), under what glows.
            if lit && !watered {
                watered = true;
                self.pass_mark(P::SHAFTS);
                self.shafts();
                self.pass_mark(P::WATER_FX);
                let t = self.now();
                self.water_fx(frame);
                self.prof[12] += self.now().wrapping_sub(t);
            }
            // What glows goes over the light: laid as the pass after the light comes.
            if lit && !self.glows.is_empty() {
                self.pass_mark(P::GLOW);
                self.quads.append(&mut self.glows);
            }
            self.pass_mark(match *pass {
                Pass::Terrain { .. } => P::TERRAIN,
                Pass::Silhouettes { .. } => P::SUN_SHADOWS,
                Pass::Sprites { .. } => P::SPRITES,
                Pass::Lights { .. } => P::LIGHTMAP,
                Pass::Sky(_) | Pass::Parallax { .. } => P::SKY,
                Pass::Fog { .. } => P::FOG,
                Pass::Water { .. } => P::SHIMMER,
                Pass::Particles { .. } => P::PARTICLES,
                Pass::Weather(_) | Pass::Rays { .. } | Pass::Post(_) => P::SATURATION,
            });
            match *pass {
                Pass::Terrain { chunks } => {
                    for c in frame.chunks_in(chunks) {
                        self.chunks.push((c.slot, c.generation));
                        self.placed.push((c.x, c.y, c.slot));
                        let q = Quad {
                            tex: Tex::Chunk(c.slot),
                            mode: Mode::Opaque,
                            colour: 0xffff_ffff,
                            x0: 0,
                            y0: 0,
                            x1: 0,
                            y1: 0,
                            u0: 0,
                            v0: 0,
                            u1: 0,
                            v1: 0,
                        };
                        self.push(q, c.x, c.y, CHUNK_PX, CHUNK_PX, 0, 0, false);
                    }
                }
                Pass::Silhouettes { sun, shade, casters, blocks } => {
                    self.sun = Some(sun);
                    if self.effects & fx::SHADOWS != 0 {
                        let t = self.now();
                        self.silhouettes(frame, &sun, shade, (casters, blocks), px);
                        self.prof[5] += self.now().wrapping_sub(t);
                    }
                }
                Pass::Sprites { cmds, .. } => {
                    let t = self.now();
                    for (k, s) in frame.sprites_in(cmds).iter().enumerate() {
                        self.sprite(s, &frame.lights, cmds.start + k as u32);
                        if let Some(f) = s.foot
                            && s.flags.tint != Tint::Seen
                        {
                            self.patch(frame, s, f);
                        }
                    }
                    self.prof[6] += self.now().wrapping_sub(t);
                }
                Pass::Lights { ambient, fill, points, casters, blocks, band, .. } => {
                    lit = true;
                    self.ambient = ambient;
                    self.fill = fill;
                    // The terrain's own glow (lit windows), over the light with the sprites'.
                    if self.effects & fx::GLOW != 0 {
                        self.chunk_glows(frame);
                    }
                    let points = frame.lights_in(points);
                    if !points.is_empty() && self.effects & fx::LAMPS != 0 {
                        // T0's lightmap (`soft`'s): the ambient and every pool, at half size,
                        // half a cell back so the GE's filter lands its cells where `soft`'s do.
                        let t0 = self.now();
                        self.gpu_lightmap(frame, ambient, band, points, (casters, blocks), px);
                        self.prof[1] += self.now().wrapping_sub(t0);
                        // Each light's halo over it, as T0's bloom gathers round what glows,
                        // stronger as the dark comes.
                        if self.effects & fx::GLOW != 0 {
                            let dark = jane_present::light::pool(ambient);
                            for l in points {
                                let r = (i32::from(l.radius) / 6).clamp(8, 28);
                                let (x, y) = (l.pos.0, l.pos.1 - jane_present::rows_up(i32::from(l.height)));
                                let k = |c: u8| (u32::from(c) * dark / 512).min(255);
                                let c = 0xff00_0000 | k(l.colour[2]) << 16 | k(l.colour[1]) << 8 | k(l.colour[0]);
                                if x + r <= 0 || y + r <= 0 || x - r >= self.w || y - r >= self.h {
                                    continue;
                                }
                                let d = crate::light::DISC as u16;
                                self.glows.push(Quad {
                                    tex: Tex::Disc,
                                    mode: Mode::Halo,
                                    colour: c,
                                    x0: (x - r) as i16,
                                    y0: (y - r) as i16,
                                    x1: (x + r) as i16,
                                    y1: (y + r) as i16,
                                    u0: 0,
                                    v0: 0,
                                    u1: d,
                                    v1: d,
                                });
                            }
                        }
                        let (w, h) = (self.light.w, self.light.h);
                        let half = crate::light::CELL as i16 / 2;
                        self.pass_mark(P::LIGHT_MUL);
                        self.quads.push(Quad {
                            tex: Tex::LightRt,
                            mode: Mode::Multiply2Opaque,
                            colour: 0xffff_ffff,
                            x0: -half,
                            y0: -half,
                            x1: (w * crate::light::CELL) as i16 - half,
                            y1: (h * crate::light::CELL) as i16 - half,
                            u0: 0,
                            v0: 0,
                            u1: w as u16,
                            v1: h as u16,
                        });
                    } else if ambient.iter().any(|&c| c < 254) || !band.is_none() {
                        self.pass_mark(P::LIGHT_MUL);
                        // The flat light (T0's ambient, the sun's share in it): what is drawn,
                        // times it; through the night's turn a strip at a time down the band.
                        let w = self.w as i16;
                        let quads = &mut self.quads;
                        band_strips(band, self.h, 8, |y0, y1, k| {
                            quads.push(Quad {
                                tex: Tex::None,
                                mode: Mode::Multiply,
                                colour: rgba(dim(ambient, k), 255),
                                x0: 0,
                                y0: y0 as i16,
                                x1: w,
                                y1: y1 as i16,
                                u0: 0,
                                v0: 0,
                                u1: 0,
                                v1: 0,
                            });
                        });
                    }
                }
                // The sky's look, read by the grade (its afterglow).
                Pass::Sky(sky) => {
                    self.sky = Some(sky);
                    if self.atmos_off & atmos_fx::SKY == 0 {
                        self.sky(&sky, &frame.stars[sky.star_list.range()]);
                    }
                }
                Pass::Parallax { sprites, .. } => {
                    if let Some(sky) = self.sky.filter(|_| self.atmos_off & atmos_fx::SKY == 0) {
                        for (k, sp) in frame.sprites_in(sprites).iter().enumerate() {
                            self.far_thing(&sky, sp, sprites.start + k as u32);
                        }
                    }
                }
                Pass::Fog { volumes, drift } => {
                    if self.atmos_off & atmos_fx::FOG == 0 {
                        self.fog(frame.fog_in(volumes), frame.camera, drift);
                    }
                }
                Pass::Water { cells } => {
                    if self.atmos_off & atmos_fx::WATER == 0 {
                        self.water(frame.water_in(cells), frame.tick);
                    }
                }
                Pass::Particles { parts, .. } => {
                    if self.atmos_off & atmos_fx::PARTICLES == 0 {
                        self.particles(frame.parts_in(parts));
                    }
                }
                // What the sky is doing reaches C2 as it reaches T0: through the ambient and the
                // grade, its rain as particles. Rays a C2 frame never holds (`Features::c2`).
                Pass::Weather(a) => self.weather = Some(a),
                Pass::Rays { .. } => {}
                Pass::Post(p) => self.grade(&p),
            }
        }
        if !watered {
            self.pass_mark(P::SHAFTS);
            self.shafts();
            self.pass_mark(P::WATER_FX);
            let t = self.now();
            self.water_fx(frame);
            self.prof[12] += self.now().wrapping_sub(t);
        }
        // A frame with no light pass (the day on T0), or a light pass last: its glows now.
        if !self.glows.is_empty() {
            self.pass_mark(P::GLOW);
            self.quads.append(&mut self.glows);
        }
        // The night folded into the CLUTs even where nothing grades them (the full grade reads
        // the frame back; the grade off).
        if self.palette.is_none() && self.night > 0 {
            let n = self.night;
            self.palette = Some(Palette {
                generation: 0x8000_0000 | u32::from(n),
                lut: IDENTITY,
                sat: 128,
                night: n,
                grade: false,
            });
        }
        if let Some(p) = self.palette.filter(|p| p.grade) {
            self.palette_pass(&p);
        }
        // The passes that only blend, grouped by texture (fewer binds and CLUT loads, the same
        // px).
        let passes = self.order_passes & ORDERED;
        if passes != 0 {
            let t = self.now();
            let n = self.quads.len();
            for k in 0..self.marks.len() {
                let (at, id) = self.marks[k];
                if id < 32 && passes >> id & 1 != 0 {
                    let end = self.marks.get(k + 1).map_or(n, |m| m.0 as usize).min(n);
                    let at = (at as usize).min(end);
                    self.order.by_texture(&mut self.quads[at..end]);
                }
            }
            self.prof[10] += self.now().wrapping_sub(t);
        }
        // Marks with nothing after them go (the GE signals only where something is drawn).
        let n = self.quads.len() as u32;
        self.marks.retain(|m| m.0 < n);
        // A sub-pass with nothing in it goes: one at the quad another pass begins at (not its own
        // parent's first quad, where its first sub-pass begins with it).
        let marks = &self.marks;
        self.sub_marks
            .retain(|m| m.0 < n && !marks.iter().any(|p| p.0 == m.0 && Some(p.1) != crate::capture::pass::parent(m.1)));
        &self.quads
    }

    /// The palette grade on what the GE does not draw through a CLUT: the flat quads' and lines'
    /// colours and the smooth strips' over what is there (not the light's multiplies and adds),
    /// the fog's CLUT, the clear and the patches' px.
    fn palette_pass(&mut self, p: &Palette) {
        let graded = |q: &Quad| {
            matches!(q.tex, Tex::None | Tex::Line(_) | Tex::Spot | Tex::Strip(_))
                && matches!(q.mode, Mode::Alpha | Mode::AddGlow | Mode::Masked(Blend::Alpha, _))
        };
        for k in 0..self.quads.len() {
            let q = self.quads[k];
            if !graded(&q) {
                continue;
            }
            if let Tex::Strip(s) = q.tex {
                if let Some(st) = self.strips.get(usize::from(s)).filter(|st| st.tex == StripTex::Flat) {
                    let (a, n) = (st.start as usize, usize::from(st.len));
                    for v in &mut self.verts[a..a + n] {
                        v.colour = p.colour(v.colour);
                    }
                }
            } else {
                self.quads[k].colour = p.colour(q.colour);
            }
        }
        for e in &mut self.fog_clut {
            *e = p.colour(*e);
        }
        self.clear = p.colour(self.clear);
        for px in &mut self.patch_px {
            if *px >> 24 != 0 {
                *px = p.colour(*px);
            }
        }
    }

    /// GE pass `id` begins at the next quad (a mark at the same quad is replaced).
    pub(crate) fn pass_mark(&mut self, id: u8) {
        let at = self.quads.len() as u32;
        match self.marks.last_mut() {
            Some(m) if m.0 == at => m.1 = id,
            Some(m) if m.1 == id => {}
            _ => self.marks.push((at, id)),
        }
    }

    /// Sub-pass `id` begins at the next quad (a sub-mark at the same quad is replaced).
    pub(crate) fn sub_mark(&mut self, id: u8) {
        let at = self.quads.len() as u32;
        match self.sub_marks.last_mut() {
            Some(m) if m.0 == at => m.1 = id,
            _ => self.sub_marks.push((at, id)),
        }
    }

    pub(super) fn now(&self) -> u32 {
        self.clock.map_or(0, |c| c())
    }
}
