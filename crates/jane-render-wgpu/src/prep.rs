//! A `Frame` turned into what the GPU reads: instance lists in draw order, the lights with
//! their tiles, and the globals. Plain CPU work over the frame, so it runs in a test with no
//! device; the byte layouts are the shaders' (`shaders/*.wgsl`).

use std::ops::Range;

use jane_present::frame::{Atmos, PartShape, SkyLook};
use jane_present::{Depth, Frame, LightKind, Pass, Post, Tint};

/// Canvas px round the canvas the G-buffer and the height field cover, so a caster just off
/// screen still casts in.
pub const GUARD: u32 = 64;
/// Light tiles are this many canvas px square.
pub const TILE: u32 = 32;
/// Lights a tile lists at most.
pub const TILE_CAP: usize = 64;
/// Lights the GPU list holds at most (§1.3 `max_lights` at T2).
pub const MAX_LIGHTS: usize = 128;
/// How much brighter a point light is than its colour byte says, in linear light: a lamp's pool
/// is brighter than the dusk round it.
const POINT_GAIN: f32 = 3.0;
/// The least a point light's luminance is held to before its gain (a warm lamp's, about).
const MIN_LUMA: f32 = 0.42;
/// The emissive layer's gain: lamp glass and lit windows read as sources.
const EMISSIVE_GAIN: f32 = 1.35;
/// The sun's light on flat ground is its colour byte times this, in linear light: a low sun
/// throws strong light, and the eye adapts to the dimmer ground (the exposure's job, here).
const SUN_GAIN: f32 = 1.6;
/// The tallest thing the terrain raises, px (the stand-in walls and canopy).
const TERRAIN_TOP: f32 = 64.0;

/// How a run of sprites is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The contact shadows (index 1) of a whole pass, onto the ground before it.
    Contact,
    /// Opaque texels into all three targets.
    Opaque,
    /// A ghost: the albedo alone, over what is under it.
    Ghost,
}

/// One draw call: a run of sprite instances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draw {
    pub kind: Kind,
    pub range: Range<u32>,
}

/// What one frame uploads, in the shaders' layouts, reused frame to frame.
#[derive(Debug, Default)]
pub struct Prep {
    /// `ChunkIn`: x, y, layer, 0 (i32 each).
    pub chunks: Vec<u8>,
    pub n_chunks: u32,
    /// `(slot, generation)` of every chunk drawn, to upload the ones the GPU does not hold.
    pub chunk_slots: Vec<(u16, u32)>,
    /// `SpriteIn`: src (4 x u32), dst (x, y, page, flags as i32), extra (depth, id, 0, 0): its id
    /// is its index in the frame plus one, what the G-buffer's id target and the height field
    /// carry (0 is the terrain).
    pub sprites: Vec<u8>,
    pub n_sprites: u32,
    /// The sprite draws, in order.
    pub draws: Vec<Draw>,
    /// `Light` (3 x vec4).
    pub lights: Vec<u8>,
    pub n_lights: u32,
    /// `(first, count)` per tile.
    pub tiles: Vec<u8>,
    pub tile_lights: Vec<u8>,
    pub tiles_x: u32,
    pub tiles_y: u32,
    /// The globals uniform, [`GLOBALS`] bytes.
    pub globals: Vec<u8>,
    /// The frame's clear, linear.
    pub clear: [f64; 3],
    /// The sky's sprites (`SpriteIn`, the same layout), drawn onto the sky backdrop.
    pub sky_sprites: Vec<u8>,
    pub n_sky_sprites: u32,
    /// The stars: `x, up, brightness, 0` as f32.
    pub stars: Vec<u8>,
    pub n_stars: u32,
    /// Whether the frame has a sky pass, a water pass, fog, light shafts.
    pub has_sky: bool,
    pub has_water: bool,
    pub n_fog: u32,
    /// The light shafts' strength, 0 for none.
    pub rays: u8,
    /// `FogVolume` (3 x vec4): rect; colour and density; edge, top.
    pub fog: Vec<u8>,
    /// `PartIn` (3 x vec4): x, y, shape, a; b, c, height, glow; colour, alpha.
    pub parts: Vec<u8>,
    pub n_parts: u32,
    /// The particle draws in order, by layer: ground, then canopy, then weather.
    pub part_draws: Vec<(Depth, Range<u32>)>,
    /// Draw the sun's term alone (`Wgpu::show_sun`).
    pub show_sun: bool,
    depth: Vec<u8>,
    lists: Vec<Vec<u32>>,
}

/// The globals uniform's size, bytes.
pub const GLOBALS: usize = 256;
/// Fog volumes a frame draws at most.
pub const MAX_FOG: usize = 16;

fn u32s(out: &mut Vec<u8>, v: &[u32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn i32s(out: &mut Vec<u8>, v: &[i32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn f32s(out: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

/// The G-buffer id of `Frame::sprites[i]`: one more than its index, held to the id target's 16
/// bits (0 is the terrain).
pub fn sprite_id(i: usize) -> u32 {
    (i as u32 + 1).min(u32::from(u16::MAX))
}

/// An sRGB byte as linear light.
pub fn linear(c: u8) -> f32 {
    let v = f32::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn lin3(c: [u8; 3]) -> [f32; 3] {
    c.map(linear)
}

/// How far every light (the fill, the sun, the afterglow, the lamps) keeps the chroma its byte
/// names rather than the power curve's, 0..1. Taken channel by channel through the curve, a dusk blue
/// and a low sun's orange both come out far more saturated than the byte (a linear orange is a
/// red), and the two meet on a lit face as mauve; T1 multiplies the bytes as they are. So their
/// brightness goes through the curve and most of their hue does not (the art-director pass,
/// 2026-09-27): a dusk reads gold where the sun is and blue where it is not, and a lamp's pool
/// fades into the night through a warm grey rather than through mauve.
const SKY_CHROMA: f32 = 0.85;

/// A light's byte colour as linear light, its luminance through the curve and its chroma
/// [`SKY_CHROMA`] of the way to the byte's own.
fn light3(c: [u8; 3]) -> [f32; 3] {
    let s = c.map(|v| f32::from(v) / 255.0);
    let luma = 0.2126 * s[0] + 0.7152 * s[1] + 0.0722 * s[2];
    if luma <= 0.0 {
        return [0.0; 3];
    }
    let k = ((luma + 0.055) / 1.055).powf(2.4).min(luma) / luma;
    let l = lin3(c);
    [0, 1, 2].map(|i| l[i] + (s[i] * k - l[i]) * SKY_CHROMA)
}

/// A jane-core angle (65536 a turn) in radians.
fn rad(a: u16) -> f32 {
    f32::from(a) * std::f32::consts::TAU / 65536.0
}

impl Prep {
    /// Fills every list from `frame`.
    pub fn build(&mut self, frame: &Frame, ticks: u32) {
        let (w, h) = (u32::from(frame.canvas.0), u32::from(frame.canvas.1));
        self.chunks.clear();
        self.chunk_slots.clear();
        self.sprites.clear();
        self.draws.clear();
        self.lights.clear();
        self.tiles.clear();
        self.tile_lights.clear();
        self.globals.clear();
        self.n_chunks = 0;
        self.n_sprites = 0;
        self.n_lights = 0;
        self.sky_sprites.clear();
        self.n_sky_sprites = 0;
        self.stars.clear();
        self.n_stars = 0;
        self.has_sky = false;
        self.has_water = false;
        self.n_fog = 0;
        self.rays = 0;
        self.fog.clear();
        self.parts.clear();
        self.n_parts = 0;
        self.part_draws.clear();
        let c = frame.clear;
        self.clear = [(c >> 16) as u8, (c >> 8) as u8, c as u8].map(|v| f64::from(linear(v)));

        // Each sprite's depth across the ground: its caster's, else thin.
        self.depth.clear();
        self.depth.resize(frame.sprites.len(), 2);
        for c in &frame.casters {
            if let Some(d) = self.depth.get_mut(c.sprite as usize) {
                *d = c.depth.max(1);
            }
        }

        let mut hmax = TERRAIN_TOP;
        let mut sky = None;
        let mut post = Post { tint: [255; 3], lift: [0; 3], saturation: 128, bloom: 0, exposure: 128 };
        let mut backdrop: Option<SkyLook> = None;
        let mut atmos = Atmos::default();
        let mut drift = (0i16, 0i16);
        let mut rays = 0u8;
        for pass in &frame.passes {
            match *pass {
                Pass::Terrain { chunks } => {
                    for c in frame.chunks_in(chunks) {
                        i32s(&mut self.chunks, &[c.x, c.y, i32::from(c.slot), 0]);
                        self.chunk_slots.push((c.slot, c.generation));
                        self.n_chunks += 1;
                    }
                }
                Pass::Sprites { cmds, layer } => {
                    let first = self.n_sprites;
                    for (k, s) in frame.sprites_in(cmds).iter().enumerate() {
                        let (kind, a) = match s.flags.tint {
                            Tint::None => (0u32, 0u32),
                            Tint::Flash(a) => (1, u32::from(a)),
                            Tint::Ghost(a) => (2, u32::from(a)),
                        };
                        let flags = u32::from(s.flags.mirror) | a << 8 | kind << 16;
                        let depth = self.depth.get(cmds.start as usize + k).copied().unwrap_or(2);
                        u32s(
                            &mut self.sprites,
                            &[u32::from(s.src.x), u32::from(s.src.y), u32::from(s.src.w), u32::from(s.src.h)],
                        );
                        i32s(&mut self.sprites, &[i32::from(s.x), i32::from(s.y), i32::from(s.page), flags as i32]);
                        let id = sprite_id(cmds.start as usize + k);
                        u32s(&mut self.sprites, &[u32::from(depth), id, 0, 0]);
                        if layer == Depth::Standing {
                            hmax = hmax.max(f32::from(s.height_px));
                        }
                        self.n_sprites += 1;
                    }
                    let end = self.n_sprites;
                    if end > first {
                        self.draws.push(Draw { kind: Kind::Contact, range: first..end });
                        // Runs of one kind, in order.
                        let mut run = first;
                        let ghost = |i: u32| {
                            matches!(
                                frame.sprites[cmds.start as usize + (i - first) as usize].flags.tint,
                                Tint::Ghost(_)
                            )
                        };
                        for i in first + 1..=end {
                            if i == end || ghost(i) != ghost(run) {
                                let kind = if ghost(run) { Kind::Ghost } else { Kind::Opaque };
                                self.draws.push(Draw { kind, range: run..i });
                                run = i;
                            }
                        }
                    }
                }
                // Replaced by the shadow maps at T2 (§1.3 `silhouettes`).
                Pass::Silhouettes { .. } => {}
                Pass::Lights { fill, sun, points, .. } => {
                    sky = Some((fill, sun));
                    for l in frame.lights_in(points).iter().take(MAX_LIGHTS) {
                        let (dir, cone) = match l.kind {
                            LightKind::Point => ((0.0, 0.0), -2.0),
                            LightKind::Spot { dir, cone } => ((rad(dir.0).cos(), rad(dir.0).sin()), rad(cone.0).cos()),
                        };
                        let g = GUARD as f32;
                        f32s(
                            &mut self.lights,
                            &[l.pos.0 as f32 + g, l.pos.1 as f32 + g, f32::from(l.height), f32::from(l.radius)],
                        );
                        let [r, gg, b] = light3(l.colour);
                        // Flame light leans warm: a yellow lamp reads as a lamp on green grass,
                        // not as lime; and not so far that its pool, fading into the blue of the
                        // night, passes through mauve (the art-director pass, 2026-09-27). A deep orange flame (a fire) is held up to a lamp's
                        // brightness, or the grass it stands on would swallow its pool.
                        let [r, gg, b] = [r, gg * 0.82, b * 0.6];
                        let luma = 0.2126 * r + 0.7152 * gg + 0.0722 * b;
                        let k = POINT_GAIN * (MIN_LUMA / luma.max(0.01)).clamp(1.0, 1.8);
                        let [r, gg, b] = [r * k, gg * k, b * k];
                        f32s(&mut self.lights, &[r, gg, b, f32::from(l.size)]);
                        // w: 0 when it casts nothing, else one more than its holder's id (1: none),
                        // so its trace skips what carries it.
                        let casts =
                            if l.casts { 1.0 + l.holder.map_or(0, |h| sprite_id(h as usize)) as f32 } else { 0.0 };
                        f32s(&mut self.lights, &[dir.0, dir.1, cone, casts]);
                        self.n_lights += 1;
                    }
                    self.tile(frame);
                }
                Pass::Post(p) => post = p,
                Pass::Sky(s) => {
                    self.has_sky = true;
                    backdrop = Some(s);
                    for st in &frame.stars[s.star_list.range()] {
                        f32s(&mut self.stars, &[f32::from(st.x), f32::from(st.up), f32::from(st.bright) / 255.0, 0.0]);
                        self.n_stars += 1;
                    }
                }
                Pass::Parallax { sprites, factor, .. } => {
                    // The farther, the hazier: the School at an eighth, the trees at a quarter.
                    let haze = if factor <= 32 { 50 } else { 24 };
                    for s in frame.sprites_in(sprites) {
                        u32s(
                            &mut self.sky_sprites,
                            &[u32::from(s.src.x), u32::from(s.src.y), u32::from(s.src.w), u32::from(s.src.h)],
                        );
                        i32s(&mut self.sky_sprites, &[i32::from(s.x), i32::from(s.y), i32::from(s.page), 0]);
                        u32s(&mut self.sky_sprites, &[haze, 0, 0, 0]);
                        self.n_sky_sprites += 1;
                    }
                }
                Pass::Water { .. } => self.has_water = true,
                Pass::Weather(a) => atmos = a,
                Pass::Fog { volumes, drift: d } => {
                    drift = d;
                    for v in frame.fog_in(volumes).iter().take(MAX_FOG - self.n_fog as usize) {
                        let (x0, y0, x1, y1) = v.rect;
                        f32s(&mut self.fog, &[x0 as f32, y0 as f32, x1 as f32, y1 as f32]);
                        let [r, gg, b] = lin3(v.colour);
                        f32s(&mut self.fog, &[r, gg, b, f32::from(v.density) / 255.0]);
                        f32s(&mut self.fog, &[f32::from(v.edge.max(1)), f32::from(v.top), 0.0, 0.0]);
                        self.n_fog += 1;
                    }
                }
                Pass::Rays { strength } => {
                    rays = strength;
                    self.rays = strength;
                }
                Pass::Particles { layer, parts } => {
                    let first = self.n_parts;
                    for p in frame.parts_in(parts) {
                        let (shape, a, b, c) = match p.shape {
                            PartShape::Streak { dx, dy } => (0.0, f32::from(dx), f32::from(dy), 0.0),
                            PartShape::Dot { size } => (1.0, f32::from(size), 0.0, 0.0),
                            PartShape::Ring { r } => (2.0, f32::from(r), 0.0, 0.0),
                            PartShape::Glow { r } => (3.0, f32::from(r), 0.0, 0.0),
                        };
                        f32s(&mut self.parts, &[f32::from(p.x), f32::from(p.y), shape, a]);
                        f32s(&mut self.parts, &[b, c, f32::from(p.height), f32::from(p.glow) / 255.0]);
                        let [r, gg, bb] = lin3(p.colour);
                        f32s(&mut self.parts, &[r, gg, bb, f32::from(p.alpha) / 255.0]);
                        self.n_parts += 1;
                    }
                    if self.n_parts > first {
                        self.part_draws.push((layer, first..self.n_parts));
                    }
                }
            }
        }
        if sky.is_none() {
            // A frame with no light pass is lit flat (a T0 frame at noon).
            self.tile(frame);
        }

        let (fill, sun) = sky.unwrap_or(([255; 3], None));
        let g = GUARD as f32;
        f32s(&mut self.globals, &[(w + 2 * GUARD) as f32, (h + 2 * GUARD) as f32, w as f32, h as f32, g, hmax + 2.0]);
        u32s(&mut self.globals, &[self.n_lights, self.tiles_x]);
        let [fr, fg, fb] = light3(fill);
        f32s(&mut self.globals, &[fr, fg, fb, 0.0]);
        match sun {
            Some(s) => {
                let (az, el) = (rad(s.azimuth.0), rad(s.elevation.0));
                f32s(&mut self.globals, &[el.cos() * az.cos(), el.cos() * az.sin(), el.sin(), 1.0]);
                let [r, gg, b] = light3(s.colour).map(|v| v * SUN_GAIN);
                let k = 1.0 / rad(s.spread.max(60)).tan();
                f32s(&mut self.globals, &[r, gg, b, k]);
            }
            None => {
                f32s(&mut self.globals, &[0.0, 0.0, 1.0, 0.0]);
                f32s(&mut self.globals, &[0.0; 4]);
            }
        }
        let [tr, tg, tb] = post.tint.map(|c| f32::from(c) / 255.0);
        f32s(&mut self.globals, &[tr, tg, tb, f32::from(post.saturation) / 128.0]);
        let [lr, lg, lb] = post.lift.map(|c| f32::from(c) / 255.0);
        f32s(&mut self.globals, &[lr, lg, lb, f32::from(post.exposure) / 128.0]);
        // w: the sun's term alone, a debug view (`Wgpu::show_sun`).
        let show = if self.show_sun { 1.0 } else { 0.0 };
        f32s(&mut self.globals, &[f32::from(post.bloom) / 255.0 * 1.4, ticks as f32, EMISSIVE_GAIN, show]);
        // The weather and the sky (§1.8, §1.9).
        let unit = |b: u8| f32::from(b) / 255.0;
        f32s(&mut self.globals, &[unit(atmos.rain), unit(atmos.mist), unit(atmos.wet), unit(atmos.flash)]);
        f32s(&mut self.globals, &[f32::from(atmos.wind), frame.tick as f32, f32::from(drift.0), f32::from(drift.1)]);
        let s = backdrop.unwrap_or(SkyLook {
            zenith: [0; 3],
            horizon: [0; 3],
            glow: [0; 3],
            glow_x: 0,
            glow_amount: 0,
            stars: 0,
            star_list: jane_present::Span::default(),
            moon: None,
            zone: (0, 0, 0, 0),
            tick: 0,
        });
        let [zr, zg, zb] = lin3(s.zenith);
        f32s(&mut self.globals, &[zr, zg, zb, unit(s.stars)]);
        let [hr, hg, hb] = lin3(s.horizon);
        f32s(&mut self.globals, &[hr, hg, hb, f32::from(s.glow_x)]);
        let [gr, gg, gb] = lin3(s.glow);
        f32s(&mut self.globals, &[gr, gg, gb, unit(s.glow_amount)]);
        f32s(&mut self.globals, &[s.zone.1 as f32, f32::from(u8::from(self.has_sky)), unit(rays), self.n_fog as f32]);
        f32s(
            &mut self.globals,
            &[frame.camera.0 as f32, frame.camera.1 as f32, f32::from(u8::from(self.has_water)), self.n_parts as f32],
        );
        f32s(&mut self.globals, &[s.zone.0 as f32, s.zone.2 as f32, s.zone.3 as f32, 0.0]);
        debug_assert_eq!(self.globals.len(), GLOBALS);
        if self.fog.is_empty() {
            f32s(&mut self.fog, &[0.0; 12]);
        }
    }

    /// The tiles: for each 32 x 32 tile of the canvas, the lights whose reach may touch a
    /// pixel of it (a pixel `h` px up stands `h` further down the ground, so a light reaches
    /// up to 64 px above its disc on screen).
    fn tile(&mut self, frame: &Frame) {
        let (w, h) = (u32::from(frame.canvas.0), u32::from(frame.canvas.1));
        self.tiles_x = w.div_ceil(TILE);
        self.tiles_y = h.div_ceil(TILE);
        let n = (self.tiles_x * self.tiles_y) as usize;
        if self.lists.len() < n {
            self.lists.resize_with(n, Vec::new);
        }
        for l in &mut self.lists[..n] {
            l.clear();
        }
        let lights = frame.passes.iter().find_map(|p| match p {
            Pass::Lights { points, .. } => Some(frame.lights_in(*points)),
            _ => None,
        });
        for (i, l) in lights.unwrap_or(&[]).iter().take(MAX_LIGHTS).enumerate() {
            let r = i32::from(l.radius);
            let (x0, x1) = (l.pos.0 - r, l.pos.0 + r);
            let (y0, y1) = (l.pos.1 - r - 64, l.pos.1 + r);
            let t = TILE as i32;
            let tx = (x0.div_euclid(t).max(0), x1.div_euclid(t).min(self.tiles_x as i32 - 1));
            let ty = (y0.div_euclid(t).max(0), y1.div_euclid(t).min(self.tiles_y as i32 - 1));
            for y in ty.0..=ty.1 {
                for x in tx.0..=tx.1 {
                    let list = &mut self.lists[(y as u32 * self.tiles_x + x as u32) as usize];
                    if list.len() < TILE_CAP {
                        list.push(i as u32);
                    }
                }
            }
        }
        let mut first = 0u32;
        for list in &self.lists[..n] {
            u32s(&mut self.tiles, &[first, list.len() as u32]);
            u32s(&mut self.tile_lights, list);
            first += list.len() as u32;
        }
        if self.tile_lights.is_empty() {
            u32s(&mut self.tile_lights, &[0]);
        }
        if self.lights.is_empty() {
            f32s(&mut self.lights, &[0.0; 12]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::{Flags, Light, Span, SpriteCmd, Src, Tier};

    fn sprite(x: i16, tint: Tint) -> SpriteCmd {
        SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 8, h: 8 },
            x,
            y: 0,
            flags: Flags { mirror: false, tint },
            height_px: 40,
        }
    }

    #[test]
    fn a_pass_draws_its_contact_shadows_first_then_its_runs_in_order() {
        let mut f = Frame::new(Tier::T2);
        f.sprites.extend([
            sprite(0, Tint::None),
            sprite(1, Tint::Ghost(90)),
            sprite(2, Tint::None),
            sprite(3, Tint::Flash(9)),
        ]);
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 4 } });
        let mut p = Prep::default();
        p.build(&f, 0);
        let got: Vec<(Kind, Range<u32>)> = p.draws.iter().map(|d| (d.kind, d.range.clone())).collect();
        assert_eq!(got, [(Kind::Contact, 0..4), (Kind::Opaque, 0..1), (Kind::Ghost, 1..2), (Kind::Opaque, 2..4)]);
        assert_eq!(p.sprites.len(), 4 * 48);
        assert_eq!(p.globals.len(), GLOBALS);
    }

    #[test]
    fn a_light_is_listed_in_the_tiles_it_reaches_and_no_others() {
        let mut f = Frame::new(Tier::T2);
        f.lights.push(Light {
            pos: (100, 100),
            height: 30,
            colour: [255, 200, 100],
            radius: 20,
            size: 4,
            casts: true,
            kind: LightKind::Point,
            holder: None,
        });
        f.passes.push(Pass::Lights {
            ambient: [60; 3],
            fill: [40; 3],
            sun: None,
            points: Span { start: 0, len: 1 },
            casters: Span::default(),
        });
        let mut p = Prep::default();
        p.build(&f, 0);
        assert_eq!((p.tiles_x, p.tiles_y), (24, 14));
        let tile = |x: u32, y: u32| {
            let i = ((y * p.tiles_x + x) * 8) as usize;
            u32::from_le_bytes(p.tiles[i + 4..i + 8].try_into().unwrap())
        };
        // The disc spans x 80..120 (tiles 2, 3) and, with the 64 px lift, y 16..120 (tiles 0 to 3).
        assert_eq!(tile(3, 3), 1);
        assert_eq!(tile(2, 0), 1);
        assert_eq!(tile(4, 3), 0);
        assert_eq!(tile(3, 4), 0);
        assert_eq!(p.n_lights, 1);
    }
}
