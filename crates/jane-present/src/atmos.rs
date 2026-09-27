//! Atmosphere (PRESENTATION.md §1.9): the sky and the far things on its horizon, the weather the
//! view says is over her, the fog volumes of the place and the hour, and how the weather changes
//! the light. What the sky does is the sim's (`View::weather`, rolled hourly per region, never on
//! the first walk); this module never rolls it. It eases each change in over seconds, strikes the
//! lightning from its own `Lcg`, and draws.
//!
//! Every timer is by tick (§1.11); everything that reaches a T0 frame is integer.

use jane_art::fx::Lcg;
use jane_art::weather::{self, Star};
use jane_core::Angle;
use jane_core::ZoneId;
use jane_core::angle::sin_q15;
use jane_data::{AtmosLayer, FogHeight, Region};
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::frame::{
    Atmos, CELL, Depth, Features, Flags, FogVolume, Frame, Moon, Pass, Rgb, SkyLook, Span, SpriteCmd, StarCmd, Tier,
    WaterCmd, WeatherKind, ZENITH_PX,
};
use crate::light::Sky;

/// Ticks in an hour of the clock.
const HOUR: u32 = 7200;
/// Of 65535 a level moves a tick: a turn of the sky comes on over ten seconds.
const EASE: u32 = 110;
/// A lightning flash's brightness by tick: full for two ticks, then gone quickly (§1.11).
const FLASH: [u8; 7] = [255, 255, 150, 70, 30, 12, 0];

/// The sky's sprites in the atlas (ART.md §2.8): the School in three distance bands and three
/// lightings (dark, one window, two), a treeline per region, the moon in eight phases.
#[derive(Debug)]
pub struct SkyArt {
    school: [[RefId; 3]; 3],
    treeline: [RefId; 3],
    moon: [RefId; 8],
}

impl SkyArt {
    /// Generates and packs them.
    pub fn build(atlas: &mut Atlas) -> SkyArt {
        let foot = |c: &jane_art::Canvas| (0, c.h() as i16);
        let mut add = |c: jane_art::Canvas| atlas.add_canvas(&c, foot(&c), 1, |_, _, t| t);
        let school = [0u8, 1, 2].map(|band| [0u8, 1, 2].map(|lit| add(weather::school(band, lit))));
        let treeline = [Region::Lowfields, Region::Waters, Region::Works].map(|r| add(weather::treeline(r, 0x7472_6565)));
        let moon = [0u8, 1, 2, 3, 4, 5, 6, 7].map(|p| add(weather::moon(p)));
        SkyArt { school, treeline, moon }
    }
}

/// A fog layer resolved on a zone: where it lies, and how thick it is now.
#[derive(Clone, Copy, Debug)]
struct Vol {
    layer: &'static AtmosLayer,
    /// Zone canvas px `(x0, y0, x1, y1)`, or `None` for the whole view (a region's, a zone's).
    rect: Option<(i32, i32, i32, i32)>,
    /// 0..=65535, eased.
    level: u32,
}

/// The atmosphere's state: the presenter's own, never the sim's.
#[derive(Debug)]
pub struct Atmosphere {
    tier: Tier,
    pub features: Features,
    art: SkyArt,
    stars: Vec<Star>,
    zone: Option<ZoneId>,
    indoor: bool,
    region: Region,
    clock: u32,
    day: u32,
    tick: u32,
    kind: WeatherKind,
    /// 0..=65535, eased.
    rain: u32,
    mist: u32,
    /// Q4 canvas px a tick.
    wind: i32,
    flash_at: Option<u32>,
    strike_at: u32,
    wet: u8,
    rng: Lcg,
    /// A sheet's or film's sky, over the view's (`jane sheet scene --weather`).
    force: Option<(WeatherKind, u8)>,
    /// The School's door in the county, zone canvas px.
    school: Option<(i32, i32)>,
    vols: Vec<Vol>,
    /// The mist tile's drift, Q4 px.
    drift: (i32, i32),
}

/// `a` toward `b` by `n` of 65535.
fn mix(a: Rgb, b: Rgb, n: u32) -> Rgb {
    let n = n.min(65535);
    [0, 1, 2].map(|k| ((u32::from(a[k]) * (65535 - n) + u32::from(b[k]) * n) / 65535) as u8)
}

/// `c` scaled by `k` of 256.
fn scale(c: Rgb, k: u32) -> Rgb {
    c.map(|v| (u32::from(v) * k / 256).min(255) as u8)
}

fn rgb(c: u32) -> Rgb {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

impl Atmosphere {
    /// The atmosphere at `tier`, its sprites packed into `atlas` and its mist tile made.
    pub fn new(tier: Tier, atlas: &mut Atlas) -> Atmosphere {
        atlas.pages.mist = weather::mist_tile(0x6d69_7374);
        Atmosphere {
            tier,
            features: Features::of(tier),
            art: SkyArt::build(atlas),
            stars: weather::stars(0x7374_6172),
            zone: None,
            indoor: false,
            region: Region::Lowfields,
            clock: 12 * HOUR,
            day: 0,
            tick: 0,
            kind: WeatherKind::Clear,
            rain: 0,
            mist: 0,
            wind: 4,
            flash_at: None,
            strike_at: 600,
            wet: 0,
            rng: Lcg(1),
            force: None,
            school: None,
            vols: Vec::with_capacity(32),
            drift: (0, 0),
        }
    }

    /// Holds the sky to `kind` with the ground `wet` (0..=255) whatever the view says: the sheet
    /// and film tools' way to look at rain at night. `None` gives the sky back to the view.
    pub fn force(&mut self, sky: Option<(WeatherKind, u8)>) {
        self.force = sky;
    }

    /// A new zone: its fog layers found, its dice reseeded, the weather set without easing.
    pub fn zone(&mut self, view: &View<'_>) {
        let zone = view.zone();
        self.zone = Some(zone);
        self.rng = Lcg(jane_art::hash::h32(view.seed(), zone as u32, 0x6174_6d6f));
        self.school = if zone == ZoneId::County {
            view.sym("school_mouth").and_then(|s| view.mark(s)).map(|m| {
                (i32::from(m.cell.x) * CELL + CELL / 2, i32::from(m.cell.y) * CELL)
            })
        } else {
            None
        };
        self.vols.clear();
        let px = |r: jane_core::Rect| (r.x * CELL, r.y * CELL, (r.x + r.w) * CELL, (r.y + r.h) * CELL);
        for layer in jane_data::atmosphere().layers {
            if layer.zones.contains(&zone) {
                self.vols.push(Vol { layer, rect: None, level: 0 });
            }
            if !layer.regions.is_empty() {
                self.vols.push(Vol { layer, rect: None, level: 0 });
            }
            for (s, r) in view.areas() {
                if layer.areas.contains(&view.name(s)) {
                    self.vols.push(Vol { layer, rect: Some(px(r)), level: 0 });
                }
            }
            for (s, r) in view.rects() {
                if layer.rects.contains(&view.name(s)) {
                    self.vols.push(Vol { layer, rect: Some(px(r)), level: 0 });
                }
            }
        }
        self.read(view);
        let (rain, mist) = self.targets();
        (self.rain, self.mist) = (rain, mist);
        let hour = (self.clock / HOUR) as u8;
        for i in 0..self.vols.len() {
            self.vols[i].level = if self.vol_shows(&self.vols[i], hour) { 65535 } else { 0 };
        }
    }

    /// What the view says of the sky and the clock.
    fn read(&mut self, view: &View<'_>) {
        (self.clock, self.day) = view.clock();
        self.indoor = view.indoor();
        self.region = view.region();
        let (kind, wet) = self.force.unwrap_or_else(|| {
            let k = match view.weather().kind {
                jane_sim::state::WeatherKind::Clear => WeatherKind::Clear,
                jane_sim::state::WeatherKind::Mist => WeatherKind::Mist,
                jane_sim::state::WeatherKind::Rain => WeatherKind::Rain,
                jane_sim::state::WeatherKind::Storm => WeatherKind::Storm,
            };
            (k, view.wetness())
        });
        // Indoors the sky is shut out; the ground is as dry as the floor.
        (self.kind, self.wet) = if self.indoor { (WeatherKind::Clear, 0) } else { (kind, wet) };
    }

    /// Where the rain and the mist are going, 0..=65535.
    fn targets(&self) -> (u32, u32) {
        if !self.features.weather {
            return (0, 0);
        }
        match self.kind {
            WeatherKind::Clear => (0, 0),
            WeatherKind::Mist => (0, 52000),
            // Rain hangs a haze in the air.
            WeatherKind::Rain => (40000, 7000),
            WeatherKind::Storm => (65535, 12000),
        }
    }

    fn vol_shows(&self, v: &Vol, hour: u8) -> bool {
        if !self.features.fog {
            return false;
        }
        let sky = self.kind as usize;
        let here = v.rect.is_some() || v.layer.regions.contains(&self.region) || self.zone.is_some_and(|z| v.layer.zones.contains(&z));
        here && v.layer.shows(hour, sky)
    }

    /// One tick (§1.11): the sky read, every level eased toward it, the lightning, the drift.
    pub fn tick(&mut self, view: &View<'_>, tick: u32) {
        self.tick = tick;
        self.read(view);
        let (rain, mist) = self.targets();
        let ease = |v: &mut u32, to: u32| *v = if *v < to { (*v + EASE).min(to) } else { v.saturating_sub(EASE).max(to) };
        ease(&mut self.rain, rain);
        ease(&mut self.mist, mist);
        let hour = (self.clock / HOUR) as u8;
        for i in 0..self.vols.len() {
            let to = if self.vol_shows(&self.vols[i], hour) { 65535 } else { 0 };
            ease(&mut self.vols[i].level, to);
        }
        // The wind: a breath by day, a push in rain, a gale in a storm; gusting slowly by tick.
        let base = 4 + (self.rain * 40 / 65535) as i32 + if self.kind == WeatherKind::Storm { 24 } else { 0 };
        let gust = sin_q15(Angle((tick.wrapping_mul(37)) as u16)).0;
        self.wind = base + base * gust / (3 * 32768);
        // Lightning, in a storm's full dark: a strike every six to eighteen seconds, now and then
        // a second on its heels.
        if self.kind == WeatherKind::Storm && self.rain > 50000 && !self.indoor {
            if tick >= self.strike_at {
                self.flash_at = Some(tick);
                self.strike_at = tick + if self.rng.below(4) == 0 { 14 } else { 360 + self.rng.below(720) };
            }
        } else if self.strike_at < tick + 240 {
            self.strike_at = tick + 240;
        }
        // The mist tile drifts with the wind and the strongest layer's own drift.
        let (lx, ly) = self
            .vols
            .iter()
            .filter(|v| v.level > 0)
            .max_by_key(|v| v.level * u32::from(v.layer.density))
            .map_or((0, 0), |v| (i32::from(v.layer.drift.0), i32::from(v.layer.drift.1)));
        self.drift.0 += self.wind / 2 + lx * 16 / 60;
        self.drift.1 += ly * 16 / 60;
    }

    /// The flash of lightning now, 0..=255.
    fn flash(&self) -> u8 {
        self.flash_at.map_or(0, |t| FLASH.get(self.tick.wrapping_sub(t) as usize).copied().unwrap_or(0))
    }

    /// This frame's weather (`Pass::Weather`).
    pub fn atmos(&self) -> Atmos {
        Atmos {
            kind: self.kind,
            rain: (self.rain >> 8) as u8,
            mist: (self.mist >> 8) as u8,
            wind: (self.wind / 4).clamp(-127, 127) as i8,
            flash: self.flash(),
            wet: if self.features.wet { self.wet } else { 0 },
        }
    }

    /// The wind, Q4 canvas px a tick: what the rain leans by.
    pub fn wind(&self) -> i32 {
        self.wind
    }

    /// How hard it rains, 0..=65535.
    pub fn rain(&self) -> u32 {
        self.rain
    }

    /// Whether the sky is over her.
    pub fn outdoors(&self) -> bool {
        !self.indoor
    }

    /// The region she stands in.
    pub fn region(&self) -> Region {
        self.region
    }

    /// The sky as the weather changes it (§1.9): rain and storm put out the sun and darken and
    /// cool the flat light; mist spreads the sun into the sky's own light; a lightning flash
    /// lifts everything toward white for its two ticks. The grade follows: cooler and greyer in
    /// rain, paler in mist.
    pub fn light(&self, sky: &Sky) -> Sky {
        let mut s = *sky;
        if self.indoor {
            return s;
        }
        let (r, m) = (self.rain, self.mist);
        // Overcast: the sun's share goes into a greyer, darker sky light.
        let cloud = r.max(m * 3 / 4);
        if let Some(sun) = &mut s.sun {
            let keep = 65535 - (cloud * 7 / 8);
            sun.colour = sun.colour.map(|c| (u32::from(c) * keep / 65535) as u8);
            sun.spread = sun.spread.saturating_add((cloud / 2048) as u16 * 40);
        }
        let dark = 256 - r * 72 / 65535;
        s.ambient = scale(s.ambient, dark);
        s.ambient[2] = s.ambient[2].saturating_add((r * 10 / 65535) as u8);
        let grey = |c: Rgb| {
            let l = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
            [l as u8, (l + 4).min(255) as u8, (l + 14).min(255) as u8]
        };
        s.fill = mix(s.fill, grey(s.fill), cloud * 2 / 3);
        s.fill = scale(s.fill, 256 - r * 50 / 65535);
        // Mist lifts the shadows: the light comes from everywhere.
        s.fill = mix(s.fill, scale(s.ambient, 220), m / 3);
        s.shade = mix(s.shade, [250, 250, 255], cloud / 2);
        // The grade: rain cools and greys, mist pales.
        let p = &mut s.post;
        p.saturation = (i32::from(p.saturation) - (r * 26 / 65535) as i32 - (m * 22 / 65535) as i32).clamp(60, 255) as u8;
        p.tint = mix(p.tint, [226, 238, 255], r * 3 / 4);
        p.lift = mix(p.lift, scale(weather::mist_colour(self.region), 40), m / 2);
        let flash = u32::from(self.flash());
        if flash > 0 {
            let f = flash * 257;
            s.ambient = mix(s.ambient, [230, 236, 255], f);
            s.fill = mix(s.fill, [200, 210, 250], f);
            p.exposure = (u32::from(p.exposure) + flash / 4).min(255) as u8;
        }
        s
    }

    /// The sky at this moment, as the backdrop draws it.
    fn sky_look(&self, zone: (i32, i32, i32, i32), canvas_w: i32) -> SkyLook {
        let c = weather::sky(self.clock);
        let cloud = self.rain.max(self.mist * 3 / 4);
        let grey = |c: Rgb, k: u32| {
            let l = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
            let g = [l * k / 256, (l * k / 256 + 6).min(255), (l * k / 256 + 16).min(255)].map(|v| v as u8);
            mix(c, g, cloud * 4 / 5)
        };
        let t = self.clock % (24 * HOUR);
        // The afterglow: in the west (left) round sunset, in the east at dawn.
        let (glow_x, peak, width) =
            if t >= 12 * HOUR { (canvas_w / 5, HOUR * 75 / 4, HOUR * 3 / 2) } else { (canvas_w * 4 / 5, HOUR * 23 / 4, HOUR) };
        let off = t.abs_diff(peak);
        let glow_amount = if off < width { 255 - off * 255 / width } else { 0 };
        // Stars from half past eight to five, fading at the edges; cloud hides them.
        let night_from = HOUR * 41 / 2;
        let night_to = HOUR * 5;
        let stars = if t >= night_from || t < night_to {
            let edge = if t >= night_from { (t - night_from).min(HOUR) } else { (night_to - t).min(HOUR) };
            (edge * 255 / HOUR).min(255)
        } else {
            0
        };
        let stars = stars * (65535 - cloud.min(65535)) / 65535;
        // The moon crosses from east to west over the night.
        let (set, rise) = (HOUR * 37 / 2, HOUR * 11 / 2);
        let day = 24 * HOUR;
        let into = (t + day - set) % day;
        let len = (rise + day - set) % day;
        let moon = (into < len && cloud < 50000).then(|| {
            let frac = into * 1024 / len;
            let up = 20 + ((sin_q15(Angle((frac * 32) as u16)).0 * 170) >> 15);
            Moon {
                x: (canvas_w * 17 / 20 - canvas_w * 7 / 10 * frac as i32 / 1024) as i16,
                up: up as i16,
                phase: ((4 + (self.day % 16) / 2) % 8) as u8,
                colour: [226, 230, 240],
            }
        });
        SkyLook {
            zenith: grey(c.zenith, 150),
            horizon: grey(c.horizon, 190),
            glow: c.glow,
            glow_x: glow_x as i16,
            glow_amount: (glow_amount * (65535 - cloud * 3 / 4) / 65535) as u8,
            stars: stars as u8,
            moon,
            zone,
            tick: self.tick,
            star_list: Span::default(),
        }
    }

    /// The sky and the far things on its horizon (`Sky`, `Parallax`), before the terrain:
    /// outdoors only. `cam` is the view's top-left in the zone, canvas px; `cells` the zone's
    /// size.
    pub fn draw_back(&self, f: &mut Frame, cam: (i32, i32), cells: (u32, u32), atlas: &Atlas) {
        f.water.clear();
        f.fog.clear();
        f.parts.clear();
        f.stars.clear();
        f.tick = self.tick;
        if self.indoor || !self.features.sky {
            return;
        }
        let (w, h) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        let zone = (-cam.0, -cam.1, cells.0 as i32 * CELL - cam.0, cells.1 as i32 * CELL - cam.1);
        let mut look = self.sky_look(zone, w);
        if look.stars > 0 {
            // A slow drift with the camera (they are far), one in eight twinkling by tick.
            let s0 = f.stars.len();
            for (i, s) in self.stars.iter().enumerate() {
                let x = (i32::from(s.x) * w / 1024 - cam.0 / 64).rem_euclid(w);
                let mut b = u32::from(s.bright) * u32::from(look.stars) / 255;
                if s.twinkles {
                    b = b * u32::from(crate::light::flicker(self.tick, i as u32, 700, 3)) / 255;
                }
                if b > 8 {
                    f.stars.push(StarCmd { x: x as i16, up: i16::from(s.up), bright: b as u8 });
                }
            }
            look.star_list = Span::since(s0, f.stars.len());
        }
        f.passes.push(Pass::Sky(look));
        let sprite = |id: RefId, x: i32, y: i32| {
            let r = atlas.get(id);
            SpriteCmd {
                page: r.page,
                src: r.src,
                x: x.clamp(-4096, 4096) as i16,
                y: y.clamp(-4096, 4096) as i16,
                flags: Flags::default(),
                height_px: 0,
            }
        };
        // The moon, then the School to the north at an eighth, then the treeline at a quarter.
        let s0 = f.sprites.len();
        if let Some(m) = look.moon {
            let r = atlas.get(self.art.moon[usize::from(m.phase % 8)]);
            f.sprites.push(sprite(self.art.moon[usize::from(m.phase % 8)], i32::from(m.x) - i32::from(r.src.w) / 2, -i32::from(m.up)));
        }
        if let Some((sx, sy)) = self.school {
            let (cx, cy) = (cam.0 + w / 2, cam.1 + h / 2);
            let north = cy - sy;
            if north > 0 {
                // Its bearing puts it across the canvas: straight ahead mid-screen, at 45 degrees
                // two thirds of the way out.
                let x = w / 2 + (sx - cx) * (w / 3) / north.max(1);
                let cells = (north + (sx - cx).abs() / 2) / CELL;
                let band = usize::from(cells < 1300) + usize::from(cells < 400);
                let lit = if self.windows_lit() {
                    // One window steady, a second that comes and goes.
                    1 + usize::from(crate::light::flicker(self.tick, 0x5c48_4f4c, 900, 1) < 160)
                } else {
                    0
                };
                let id = self.art.school[band][lit];
                let r = atlas.get(id);
                let x = x - i32::from(r.src.w) / 2;
                if x + i32::from(r.src.w) > -64 && x < w + 64 {
                    f.sprites.push(sprite(id, x, -i32::from(r.src.h)));
                }
            }
        }
        let far = Span::since(s0, f.sprites.len());
        f.passes.push(Pass::Parallax { layer: Depth::FarLandmark, factor: 32, sprites: far });
        let s1 = f.sprites.len();
        let tree = self.art.treeline[self.region as usize];
        let r = atlas.get(tree);
        let tw = i32::from(r.src.w);
        let mut x = -(cam.0 / 4).rem_euclid(tw);
        while x < w {
            f.sprites.push(sprite(tree, x, -i32::from(r.src.h)));
            x += tw;
        }
        f.passes.push(Pass::Parallax { layer: Depth::FarTreeline, factor: 64, sprites: Span::since(s1, f.sprites.len()) });
    }

    /// The School's windows are lit from when the lamps come on (18:30) until six.
    fn windows_lit(&self) -> bool {
        let t = self.clock % (24 * HOUR);
        !(HOUR * 6..HOUR * 37 / 2).contains(&t)
    }

    /// The fog volumes in view and the light shafts (`Fog`, `Rays`), after the lights. `sky` is
    /// the frame's lit sky: below T2 a fog's colour is lit by it here, since the tier cannot.
    pub fn draw_fog(&self, f: &mut Frame, cam: (i32, i32), sky: &Sky) {
        if !self.features.fog && !self.features.weather {
            return;
        }
        let (w, h) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        let lit = |c: Rgb| -> Rgb {
            if self.tier >= Tier::T2 {
                c
            } else {
                // The fog glows a little of its own at night, so a fogged night is not black.
                [0, 1, 2].map(|k| ((u32::from(c[k]) * (u32::from(sky.ambient[k]) + 24)) >> 8).min(255) as u8)
            }
        };
        let f0 = f.fog.len();
        if self.mist > 0 && !self.indoor {
            let d = (self.mist * 170 / 65535) as u8;
            f.fog.push(FogVolume {
                rect: (-64, -64, w + 64, h + 64),
                edge: 1,
                density: d,
                colour: lit(weather::mist_colour(self.region)),
                top: 0,
            });
        }
        for v in &self.vols {
            if v.level == 0 {
                continue;
            }
            let density = (u32::from(v.layer.density) * 255 / 1000 * v.level / 65535) as u8;
            let spread = i32::from(v.layer.spread) * CELL;
            let rect = match v.rect {
                None => (-64, -64, w + 64, h + 64),
                Some((x0, y0, x1, y1)) => (x0 - cam.0 - spread, y0 - cam.1 - spread, x1 - cam.0 + spread, y1 - cam.1 + spread),
            };
            if rect.2 < 0 || rect.3 < 0 || rect.0 > w || rect.1 > h || density == 0 {
                continue;
            }
            f.fog.push(FogVolume {
                rect,
                edge: spread.clamp(1, 4096) as u16,
                density,
                colour: lit(rgb(v.layer.colour)),
                top: if v.layer.height == FogHeight::Ground { v.layer.top } else { 0 },
            });
        }
        if f.fog.len() > f0 {
            let drift = ((self.drift.0 >> 4).rem_euclid(1 << 15) as i16, (self.drift.1 >> 4).rem_euclid(1 << 15) as i16);
            f.passes.push(Pass::Fog { volumes: Span::since(f0, f.fog.len()), drift });
        }
        // Light shafts: through what stands against a low sun, in clear air or mist (§1.9).
        if self.features.god_rays
            && self.tier == Tier::T2
            && !self.indoor
            && matches!(self.kind, WeatherKind::Clear | WeatherKind::Mist)
            && let Some(sun) = sky.sun.filter(|s| s.spread <= crate::light::SILHOUETTE_SPREAD + 200)
        {
            let el = u32::from(sun.elevation.0);
            let low = 30 * 65536 / 360;
            if el < low && sun.colour.iter().any(|&c| c > 40) {
                let k = (low - el) * 255 / low;
                let strength = (k * (160 + (self.mist * 95 / 65535))) / 255;
                f.passes.push(Pass::Rays { strength: strength.min(255) as u8 });
            }
        }
    }
}

/// The water cells of the chunks in view (`Water`), after the terrain.
pub fn water_pass(f: &mut Frame) {
    let w0 = f.water.len();
    let (w, h) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
    for c in &f.chunks {
        let l = &f.layers[usize::from(c.slot)];
        for &(x, y, phase) in &l.water {
            let (px, py) = (c.x + i32::from(x) * CELL, c.y + i32::from(y) * CELL);
            if px + CELL > 0 && py + CELL > 0 && px < w && py < h {
                f.water.push(WaterCmd { x: px as i16, y: py as i16, phase });
            }
        }
    }
    if f.water.len() > w0 {
        f.passes.push(Pass::Water { cells: Span::since(w0, f.water.len()) });
    }
}

/// The sky backdrop's colour at `up` px above the horizon and canvas `x`: the zenith down to the
/// horizon, the afterglow low over its bearing. Integer, shared by every tier's sky and T0's
/// reflections.
pub fn sky_at(s: &SkyLook, x: i32, up: i32) -> Rgb {
    let t = (up.clamp(0, ZENITH_PX) * 65535 / ZENITH_PX) as u32;
    // Most of the change is low: the horizon's band is narrow.
    let t = 65535 - ((65535 - t) * (65535 - t) / 65535);
    let base = mix(s.horizon, s.zenith, t);
    if s.glow_amount == 0 {
        return base;
    }
    let dx = (x - i32::from(s.glow_x)).unsigned_abs();
    let across = 65535u32.saturating_sub(dx * 65535 / 520);
    let low = 65535u32.saturating_sub(up.max(0) as u32 * 65535 / 150);
    let k = (across / 256) * (low / 256) / 255 * u32::from(s.glow_amount) / 255 * 256;
    mix(base, s.glow, k.min(65535) * 3 / 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_runs_from_horizon_to_zenith_and_the_afterglow_is_low_in_its_bearing() {
        let s = SkyLook {
            zenith: [20, 24, 60],
            horizon: [200, 120, 110],
            glow: [255, 130, 80],
            glow_x: 100,
            glow_amount: 255,
            stars: 0,
            moon: None,
            zone: (0, 0, 0, 0),
            tick: 0,
            star_list: Span::default(),
        };
        assert_eq!(sky_at(&s, 700, ZENITH_PX), [20, 24, 60]);
        let low_glow = sky_at(&s, 100, 0);
        let low_away = sky_at(&s, 700, 0);
        assert!(low_glow[0] > low_away[0] || low_glow[1] > low_away[1], "{low_glow:?} {low_away:?}");
        assert!(sky_at(&s, 700, 40)[2] < 110 && sky_at(&s, 700, 40)[0] < 200);
    }
}
