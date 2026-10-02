//! Cues past the screen edge (PLAY-PLAN.md Phase 5; the world audit §3, the last fifty metres):
//! what pulls her sideways toward a find she cannot see yet. Presentation only, read from the
//! view; nothing here touches the sim.
//!
//! - **Crows over every camp.** A fire with two or more hostile spawns round it is a camp; four
//!   or five crows circle high over it by day, wide enough and high enough to cross her screen
//!   from about half a screen off it, each a dark wing-stroke "v" that flaps in bursts and
//!   glides between, its shadow sweeping the ground under it while the sun casts.
//! - **The dead lamps.** At night a dead lamp's glass catches the moon, a cold point that comes
//!   and goes, and its post takes a cool rim down its moon side, so it shows dark against the
//!   moonlit ground; within her lantern's reach the glass throws her lantern back.
//! - **The Hoar Stone.** A monolith over eight cells tall on the stair's block, drawn among the
//!   standing things by the presenter ([`Cues::stone`]); on the tiers whose fog has no height (T0,
//!   T1) its silhouette is laid again over the fog at the fog's density, its foot thinning, so
//!   it rises out of the ground fog. T2's fog has a `top`, and the stone's true heights stand
//!   through it.
//!
//! Every motion is a function of the tick (§1.11) and the seed: the same crows wheel the same
//! way on every machine and every replay.

use jane_art::hash::h32;
use jane_core::angle::{cos_q15, sin_q15};
use jane_core::{Angle, ZoneId};
use jane_data::Faction;
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::frame::{CELL, Depth, Flags, Frame, PartShape, Particle, Pass, Rgb, Span, SpriteCmd, Tier, Tint};
use crate::light::Sky;

/// Crows over a camp.
const CROWS: u32 = 4;
/// Hostile spawns within this many cells of a fire make it a camp.
const CAMP_REACH: i32 = 7;
/// A crow's colour before light: a blue-black.
const CROW: Rgb = [30, 28, 38];
/// A dead lamp's glass catching the moon, and her lantern.
const MOONLIT: Rgb = [206, 218, 255];
const LANTERN_BACK: Rgb = [255, 214, 150];
/// Her lantern's reach, canvas px (`present::LANTERN_RADIUS`).
const LANTERN_REACH: i32 = 136;
/// The prop defs that are dead lamps.
const DEAD_LAMPS: [&str; 2] = ["lamp_dead", "forest_lamp_dead"];
/// Where a dead lamp's glass stands, px over its foot, when its look does not say.
const GLASS_UP: i32 = 34;

/// A camp: its fire's middle, zone canvas px, and its dice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camp {
    pub x: i32,
    pub y: i32,
    seed: u32,
}

/// A crow's place at a moment: zone canvas px of the point under it, its height, and its wings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crow {
    pub x: i32,
    pub y: i32,
    pub up: i32,
    /// 0 gliding, 1 wings up, 2 level, 3 down.
    pub wings: u8,
    /// Flying clockwise round the camp (the inner wing is its right).
    pub cw: bool,
}

/// The cues' own state: the presenter's, per zone.
#[derive(Debug)]
pub struct Cues {
    zone: Option<(ZoneId, u32)>,
    camps: Vec<Camp>,
    /// The Hoar Stone's foot, zone canvas px.
    stone: Option<(i32, i32)>,
    /// Dead lamps: prop id, foot (zone canvas px, the cell's middle), glass px over the foot.
    lamps: Vec<(u32, (i32, i32), i32)>,
    stone_look: RefId,
    stone_ghost: RefId,
}

impl Cues {
    /// The stone's two looks packed into `atlas`.
    pub fn new(atlas: &mut Atlas) -> Cues {
        let stone = jane_art::far::hoar_stone();
        let ghost = jane_art::far::hoar_silhouette();
        let h = stone.h();
        Cues {
            zone: None,
            camps: Vec::new(),
            stone: None,
            lamps: Vec::new(),
            stone_look: atlas.add_canvas(&stone, (0, h as i16), h.clamp(1, 255) as u8, |_, _, t| t),
            stone_ghost: atlas.add_canvas(&ghost, (0, h as i16), 1, |_, _, t| t),
        }
    }

    /// Finds the zone's camps, its stone and its dead lamps, once a zone.
    pub fn zone(&mut self, view: &View<'_>, glass: impl Fn(jane_core::SpriteId) -> Option<u8>) {
        let key = (view.zone(), view.seed());
        if self.zone == Some(key) {
            return;
        }
        self.zone = Some(key);
        self.camps.clear();
        self.lamps.clear();
        self.stone = None;
        if view.indoor() {
            return;
        }
        let cat = jane_data::catalog();
        let bp = view.blueprints().get(view.zone());
        let hostile: Vec<(i32, i32)> = bp
            .units
            .iter()
            .filter(|u| cat.combat.unit(u.def).faction != Faction::Friendly)
            .map(|u| (i32::from(u.cell.x), i32::from(u.cell.y)))
            .collect();
        for p in view.props() {
            let d = cat.story.prop(p.def);
            let (cx, cy) = (i32::from(p.cell.x), i32::from(p.cell.y));
            if matches!(d.id, "camp_fire" | "campfire_cold") {
                let (mx, my) = (cx + i32::from(d.w) / 2, cy + i32::from(d.h) / 2);
                let near =
                    hostile.iter().filter(|&&(x, y)| (x - mx).abs() <= CAMP_REACH && (y - my).abs() <= CAMP_REACH);
                if near.count() >= 2 {
                    self.camps.push(Camp {
                        x: cx * CELL + i32::from(d.w) * CELL / 2,
                        y: cy * CELL + i32::from(d.h) * CELL / 2,
                        seed: h32(view.seed(), p.id.get(), 0x4352_4f57),
                    });
                }
            } else if DEAD_LAMPS.contains(&d.id) {
                let up = glass(d.sprite).map_or(GLASS_UP, i32::from);
                self.lamps.push((p.id.get(), (cx * CELL + CELL / 2, (cy + i32::from(d.h)) * CELL), up));
            }
        }
        // The Hoar Stone stands behind the stair's block, rising over its back edge:
        // the block is the temple wall round the stair's door, however the chunk was turned.
        if view.zone() == ZoneId::County
            && let Some(door) = view.props().find(|p| cat.story.prop(p.def).id == "burial_mouth")
        {
            let (dx, dy) = (i32::from(door.cell.x), i32::from(door.cell.y));
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for y in dy - 12..=dy + 12 {
                for x in dx - 12..=dx + 12 {
                    if view.tile(x, y) == jane_core::Tile::TempleWall {
                        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                    }
                }
            }
            if x0 <= x1 {
                self.stone = Some(((x0 + x1 + 1) * CELL / 2, y0 * CELL + CELL / 2));
            }
        }
    }

    /// The camps found in this zone.
    pub fn camps(&self) -> &[Camp] {
        &self.camps
    }

    /// The Hoar Stone's foot and its look, when the zone has it.
    pub fn stone(&self) -> Option<((i32, i32), RefId)> {
        self.stone.map(|s| (s, self.stone_look))
    }

    /// Crow `i` of `camp` at time `t256` (ticks in 256ths).
    pub fn crow(camp: &Camp, i: u32, t256: u64) -> Crow {
        let h = h32(camp.seed, i, 0x5749_4e47);
        let h2 = h32(h, i, 0x4c4f_4f50);
        // Its circle: five to eleven cells out, a loop in eight to fourteen seconds, either way
        // round; the circle breathes wider and narrower, and the whole flock's centre wanders.
        let r = (5 + (h % 7) as i32) * CELL;
        let period = u64::from(480 + (h >> 4) % 360);
        let cw = (h >> 12) & 1 == 0;
        let phase = (h >> 16) & 0xffff;
        let a = ((t256 * 65536 / (period * 256)) as u32).wrapping_add(phase) as u16;
        let a = if cw { a } else { 0u16.wrapping_sub(a) };
        let breathe = sin_q15(Angle(((t256 / 256) as u32).wrapping_mul(23).wrapping_add(h2) as u16)).0;
        let r = r + r * breathe / (5 * 32768);
        let wander =
            |salt: u32| sin_q15(Angle(((t256 / 256) as u32).wrapping_mul(7).wrapping_add(camp.seed ^ salt) as u16)).0;
        let (wx, wy) = (wander(0x11) * CELL * 2 / 32768, wander(0x2b7) * CELL / 32768);
        // Seen from above and before her, a level circle is an ellipse three fifths as deep.
        let x = camp.x + wx + cos_q15(Angle(a)).0 * r / 32768;
        let y = camp.y + wy + sin_q15(Angle(a)).0 * r * 3 / (5 * 32768);
        let up = 56 + (h2 % 48) as i32 + sin_q15(Angle(a.wrapping_mul(2))).0 * 6 / 32768;
        // Flapping in bursts: a second or so in every four, the rest a glide.
        let tick = (t256 / 256) as u32 + (h2 >> 8) % 240;
        let wings = if tick % 240 < 54 { [1, 2, 3, 2][(tick / 5 % 4) as usize] } else { 0 };
        Crow { x, y, up, wings, cw }
    }

    /// Crows, glints and the stone's ghost, over everything that is lit and the fog. `cam` is the
    /// view's top-left and `her` her feet (zone canvas px), `t256` the tick in 256ths, `moon`
    /// whether the moon is up and clear, `fog_at_stone` the fog's density over the stone.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        f: &mut Frame,
        atlas: &Atlas,
        cam: (i32, i32),
        t256: u64,
        sky: &Sky,
        her: Option<(i32, i32)>,
        hour: u8,
        moon: bool,
        fog_at_stone: u8,
    ) {
        let (w, h) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        let t2 = f.tier >= Tier::T2;
        // Below T2 what is drawn after the light is lit here by the sky's flat light.
        let lit = |c: Rgb| -> Rgb {
            if t2 {
                c
            } else {
                [0, 1, 2].map(|k| ((u32::from(c[k]) * (u32::from(sky.ambient[k]) + 16)) >> 8).min(255) as u8)
            }
        };
        // The stone's ghost over the fog, on the tiers whose fog has no height.
        if !t2
            && fog_at_stone > 16
            && let Some((sx, sy)) = self.stone
        {
            let r = atlas.get(self.stone_ghost);
            let (x, y) = (sx - cam.0 - i32::from(r.src.w) / 2, sy - cam.1 - i32::from(r.src.h));
            if x + i32::from(r.src.w) > 0 && x < w && y + i32::from(r.src.h) > 0 && y < h {
                let s0 = f.sprites.len();
                let a = (u32::from(fog_at_stone) * 3 / 2).min(220) as u8;
                f.sprites.push(SpriteCmd {
                    page: r.page,
                    src: r.src,
                    x: x as i16,
                    y: y as i16,
                    flags: Flags { mirror: false, tint: Tint::Ghost(a) },
                    height_px: 0,
                    foot: None,
                });
                f.passes.push(Pass::Sprites { layer: Depth::NearFog, cmds: Span::since(s0, f.sprites.len()) });
            }
        }
        let tick = (t256 / 256) as u32;
        // Crows by day, from six till eight in the evening, and their shadows while the sun casts.
        let s0 = f.parts.len();
        let day = (6..20).contains(&hour);
        if day && sky.sun.is_some_and(|s| s.casts()) {
            for camp in self.near(cam, w, h) {
                for i in 0..CROWS + camp.seed % 2 {
                    let c = Self::crow(camp, i, t256);
                    let (gx, gy) = (c.x - cam.0, c.y - cam.1);
                    if gx > -8 && gy > -2 && gx < w + 8 && gy < h + 2 {
                        // A soft dash the width of its wings, a shorter one under it.
                        for (dx, dy, len) in [(-5, 0, 10), (-3, 1, 6)] {
                            for (from, by) in [(gx + dx, len), (gx + dx + len, -len)] {
                                f.parts.push(Particle {
                                    x: from as i16,
                                    y: (gy + dy) as i16,
                                    shape: PartShape::Streak { dx: by as i8, dy: 0 },
                                    colour: [12, 12, 20],
                                    alpha: 34,
                                    glow: 0,
                                    height: 0,
                                });
                            }
                        }
                    }
                }
            }
            if f.parts.len() > s0 {
                f.passes.push(Pass::Particles { layer: Depth::Ground, parts: Span::since(s0, f.parts.len()) });
            }
        }
        let a0 = f.parts.len();
        if day {
            for camp in self.near(cam, w, h) {
                for i in 0..CROWS + camp.seed % 2 {
                    let c = Self::crow(camp, i, t256);
                    let (x, y) = (c.x - cam.0, c.y - c.up - cam.1);
                    if x > -8 && y > -8 && x < w + 8 && y < h + 8 {
                        crow_parts(f, x, y, &c, lit(CROW), c.up.clamp(0, 255) as u8);
                    }
                }
            }
        }
        // The dead lamps at night: the moon on the glass and down the post; her lantern thrown back.
        if !day || sky.ambient.iter().all(|&c| c < 110) {
            for &(id, (lx, ly), up) in &self.lamps {
                let (x, gy) = (lx - cam.0, ly - up - cam.1);
                if x < -8 || x > w + 8 || gy < -8 || gy > h + 8 {
                    continue;
                }
                let z = up.clamp(0, 255) as u8;
                if moon {
                    // A cold point on the glass's moon side that comes and goes.
                    let tw = u32::from(crate::light::flicker(tick, id ^ 0x4d4f_4f4e, 500, 2));
                    let a = 90 + tw * 140 / 255;
                    glint(f, x - 2, gy - 2, MOONLIT, 5, a, z);
                    // At its brightest the point spikes: a small four-pointed star.
                    if tw > 190 {
                        for (dx, dy) in [(-3, 0), (3, 0), (0, -3), (0, 3)] {
                            f.parts.push(Particle {
                                x: (x - 2) as i16,
                                y: (gy - 2) as i16,
                                shape: PartShape::Streak { dx, dy },
                                colour: MOONLIT,
                                alpha: 150,
                                glow: 255,
                                height: z,
                            });
                        }
                    }
                    // The post's moon side: a cool line from under the glass to the ground.
                    f.parts.push(Particle {
                        x: (x - 3) as i16,
                        y: (gy + 5) as i16,
                        shape: PartShape::Streak { dx: 0, dy: (up - 6).clamp(1, 40) as i8 },
                        colour: [150, 166, 214],
                        alpha: 70,
                        glow: 220,
                        height: z / 2,
                    });
                }
                if let Some((hx, hy)) = her {
                    let d = (hx - lx).abs().max((hy - ly).abs());
                    if d < LANTERN_REACH {
                        let k = ((LANTERN_REACH - d) * 255 / LANTERN_REACH) as u32;
                        // The pane toward her takes it.
                        let side = if hx < lx { -2 } else { 1 };
                        glint(f, x + side, gy - 1, LANTERN_BACK, 4, 60 + k * 3 / 4, z);
                    }
                }
            }
        }
        if f.parts.len() > a0 {
            f.passes.push(Pass::Particles { layer: Depth::Canopy, parts: Span::since(a0, f.parts.len()) });
        }
    }

    /// The camps whose crows may cross a view `w` x `h` at `cam`.
    fn near(&self, cam: (i32, i32), w: i32, h: i32) -> impl Iterator<Item = &Camp> {
        // A crow is out to 14 cells and 2 more of wander from its camp, up to 110 px high.
        let reach = 16 * CELL;
        self.camps.iter().filter(move |c| {
            let (x, y) = (c.x - cam.0, c.y - cam.1);
            x > -reach && y > -reach && x < w + reach && y < h + reach + 110
        })
    }
}

/// A glint: a soft glow and a bright px at its heart, both unlit.
fn glint(f: &mut Frame, x: i32, y: i32, colour: Rgb, r: u8, a: u32, z: u8) {
    let a = a.min(255);
    f.parts.push(Particle {
        x: x as i16,
        y: y as i16,
        shape: PartShape::Glow { r },
        colour,
        alpha: (a / 3) as u8,
        glow: 255,
        height: z,
    });
    f.parts.push(Particle {
        x: x as i16,
        y: y as i16,
        shape: PartShape::Dot { size: 1 },
        colour,
        alpha: a as u8,
        glow: 255,
        height: z,
    });
}

/// One crow at canvas px `(x, y)`, about thirteen px across: a body with its wedge of a tail, and
/// each wing a long stroke to its tip over a short one at its root (the arm is broader than the
/// hand); up, level, down, or a glide's shallow "v" with the tips lifted, the inner wing a px lower
/// as it banks round the camp. A stroke is laid both ways so it is even to its tip.
fn crow_parts(f: &mut Frame, x: i32, y: i32, c: &Crow, colour: Rgb, up: u8) {
    // (tip dx, tip dy, root dx, root dy) of the left wing; the right is its mirror.
    let (tx, ty, rx, ry) = match c.wings {
        1 => (-5, -5, -3, -2),
        2 => (-7, 0, -4, 1),
        3 => (-6, 3, -3, 2),
        _ => (-7, -2, -4, 0),
    };
    let bank = if c.cw { (0, 1) } else { (1, 0) };
    let part =
        |shape, x: i32, y: i32| Particle { x: x as i16, y: y as i16, shape, colour, alpha: 245, glow: 0, height: up };
    let mut stroke = |a: (i32, i32), b: (i32, i32)| {
        let d = |p: i32, q: i32| (q - p).clamp(-40, 40) as i8;
        f.parts.push(part(PartShape::Streak { dx: d(a.0, b.0), dy: d(a.1, b.1) }, a.0, a.1));
        f.parts.push(part(PartShape::Streak { dx: d(b.0, a.0), dy: d(b.1, a.1) }, b.0, b.1));
    };
    for (side, dip) in [(-1, bank.0), (1, bank.1)] {
        let root = if side < 0 { x - 1 } else { x };
        stroke((root, y), (root + side * -tx, y + ty + dip));
        stroke((root, y + 1), (root + side * -rx, y + 1 + ry + dip));
    }
    f.parts.push(part(PartShape::Dot { size: 2 }, x - 1, y));
    f.parts.push(part(PartShape::Dot { size: 1 }, x - 1, y + 2));
    f.parts.push(part(PartShape::Dot { size: 1 }, x, y - 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crows_wheel_high_over_every_camp_and_cross_a_screen_from_off_it() {
        let mut atlas = Atlas::new();
        for seed in [1u32, 2, 3] {
            let sim = jane_sim::Sim::new_game(seed, "Tess");
            let v = sim.view(jane_sim::Seat(0)).unwrap();
            let mut c = Cues::new(&mut atlas);
            c.zone(&v, |_| None);
            assert!(c.camps().len() >= 3, "seed {seed}: the county's camps are found ({})", c.camps().len());
            assert!(c.stone().is_some(), "seed {seed}: the Hoar Stone stands on the stair's block");
            for camp in c.camps() {
                let mut far = 0;
                for i in 0..CROWS {
                    for t in (0..3600u64).step_by(37) {
                        let k = Cues::crow(camp, i, t * 256);
                        let (dx, dy) = ((k.x - camp.x).abs(), (k.y - camp.y).abs());
                        assert!(dx <= 16 * CELL && dy <= 10 * CELL, "a crow keeps over its camp");
                        assert!(k.up >= 48, "and high");
                        far = far.max(dx);
                    }
                }
                // A 48-cell view: a crow ranges further than a sixth of it from its camp.
                assert!(far >= 8 * CELL, "seed {seed}: the crows range wide enough to be seen off screen");
            }
        }
    }

    #[test]
    fn a_crow_is_drawn_by_day_over_a_camp_off_the_screen() {
        let mut atlas = Atlas::new();
        let sim = jane_sim::Sim::new_game(2, "Tess");
        let v = sim.view(jane_sim::Seat(0)).unwrap();
        let mut c = Cues::new(&mut atlas);
        c.zone(&v, |_| None);
        let camp = c.camps()[0];
        // The camp eight cells below the bottom edge of a 768 x 432 view.
        let cam = (camp.x - 384, camp.y - 432 - 8 * CELL);
        let sky = crate::light::sky(12 * 7200, 0, false, 1000, jane_data::Region::Lowfields);
        let seen = (0..600u64).step_by(10).any(|t| {
            let mut f = Frame::new(Tier::T0);
            f.canvas = (768, 432);
            c.draw(&mut f, &atlas, cam, t * 256, &sky, None, 12, false, 0);
            f.passes.iter().any(|p| matches!(p, Pass::Particles { layer: Depth::Canopy, .. }))
        });
        assert!(seen, "her screen shows the crows before the camp");
        // At midnight no crow flies, and with no moon and no lantern a dead lamp shows nothing.
        let mut f = Frame::new(Tier::T0);
        f.canvas = (768, 432);
        c.draw(&mut f, &atlas, cam, 0, &sky, None, 0, false, 0);
        assert!(f.parts.is_empty());
    }
}
