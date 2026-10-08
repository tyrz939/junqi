//! Cues past the screen edge (PLAY-PLAN.md Phase 5; the world audit §3, the last fifty metres):
//! what pulls her sideways toward a find she cannot see yet. Presentation only, read from the
//! view; nothing here touches the sim.
//!
//! - **The regions' landmarks.** The Works' chimney with its plume (the furnace's glow under it
//!   by night), St Anne's spire and the lake's statue (gilded at dawn) stand in the county as
//!   silhouettes as tall as they are, drawn over the scene, so their tops show over the bottom
//!   and side edges of her screen from a screen or two off; past the top edge, within three
//!   screens, the plume or the tip pokes in at the edge, fainter the farther. The far layer's
//!   versions (`atmos`) stay on the horizon besides.
//! - **Crows over the enemy camps.** The camps the stories put down (Rendle's, the hurst's) and
//!   the bands of ruffians round their own lit fires: five to twenty a seed, not every cold
//!   hearth the wild things stand near. Four or five crows circle over each by day, wide enough
//!   and high enough to cross her screen from about half a screen off it, each a black
//!   wing-stroke "v" that flaps in bursts and glides between, its shadow sweeping the ground
//!   under it while the sun casts.
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

use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use jane_art::hash::h32;
use jane_core::angle::{cos_q15, sin_q15};
use jane_core::{Angle, ZoneId};
use jane_data::Faction;
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::atmos::Far;
use crate::frame::{CELL, Depth, Flags, Frame, PartShape, Particle, Pass, Rgb, Span, SpriteCmd, Tier, Tint};
use crate::light::Sky;

/// Crows over a camp.
const CROWS: u32 = 4;
/// Hostile spawns are counted within this many cells of a fire.
const CAMP_REACH: i32 = 4;
/// A lit fire with this many hostile spawns round it is a band's camp.
const RUFFIANS: usize = 4;
/// A crow's colour before light: near black.
const CROW: Rgb = [14, 12, 20];
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
    /// The regions' landmarks standing in the county, each at its foot (zone canvas px).
    landmarks: Vec<(Far, (i32, i32))>,
    art: WorldArt,
}

/// Frames of the world plume.
const PLUME: usize = jane_art::far::PLUME_FRAMES as usize;
/// Ticks a frame of the world plume is held.
const PLUME_TICKS: u32 = 12;
/// Rows of a landmark's top that poke in at the top edge when it stands past it.
const TIP_ROWS: i32 = 160;
/// The most a landmark's tip shows at the top edge, of 255.
const TIP_ALPHA: u32 = 230;

/// The world landmarks' looks (`jane_art::far::*_world`): day and night (or dark and lit, plain
/// and gilded), the plume's frames, and the tips that poke in at the top edge.
#[derive(Debug)]
struct WorldArt {
    chimney: [RefId; 2],
    plume: [[RefId; PLUME]; 2],
    spire: [RefId; 2],
    statue: [RefId; 2],
    plume_tip: [RefId; PLUME],
    spire_tip: RefId,
    statue_tip: RefId,
}

/// The top `rows` of `c`, thinning by the Bayer pattern over their lower third into nothing: a
/// landmark's tip as it pokes in at the top edge.
fn tip(c: &jane_art::Canvas, rows: i32) -> jane_art::Canvas {
    let top = (0..c.h()).find(|&y| (0..c.w()).any(|x| c.get(x, y).is_opaque())).unwrap_or(0);
    let mut t = jane_art::Canvas::new(c.w(), rows);
    for y in 0..rows {
        let fade = (rows - y) * 48 / rows;
        for x in 0..c.w() {
            let ix = c.get(x, top + y);
            if ix.is_opaque() && i32::from(jane_art::canvas::bayer(x, y)) < fade {
                t.dot(x, y, ix, 1);
            }
        }
    }
    t
}

impl Cues {
    /// The stone's two looks and the world landmarks' packed into `atlas`.
    pub fn new(atlas: &mut Atlas) -> Cues {
        use jane_art::far;
        let stone = far::hoar_stone();
        let ghost = far::hoar_silhouette();
        let h = stone.h();
        let mut add = |c: &jane_art::Canvas| atlas.add_canvas(c, (0, c.h() as i16), 1, |_, _, t| t);
        let art = WorldArt {
            chimney: [false, true].map(|n| add(&far::chimney_world(n))),
            plume: [false, true].map(|n| core::array::from_fn(|k| add(&far::plume_world(n, k as u8)))),
            spire: [false, true].map(|l| add(&far::spire_world(l))),
            statue: [false, true].map(|d| add(&far::statue_world(d))),
            plume_tip: core::array::from_fn(|k| add(&tip(&far::plume_world(false, k as u8), TIP_ROWS))),
            spire_tip: add(&tip(&far::spire_world(false), TIP_ROWS)),
            statue_tip: add(&tip(&far::statue_world(false), TIP_ROWS)),
        };
        Cues {
            zone: None,
            camps: Vec::new(),
            stone: None,
            lamps: Vec::new(),
            stone_look: atlas.add_canvas(&stone, (0, h as i16), h.clamp(1, 255) as u8, |_, _, t| t),
            stone_ghost: atlas.add_canvas(&ghost, (0, h as i16), 1, |_, _, t| t),
            landmarks: Vec::new(),
            art,
        }
    }

    /// The regions' landmarks standing in this zone, each at its foot (zone canvas px).
    pub fn landmarks(&self) -> &[(Far, (i32, i32))] {
        &self.landmarks
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
        // The story's camps (Rendle's, the hurst's): the bounds of each place a story claimed
        // as a camp.
        let local = |k: jane_core::Key| match k {
            jane_core::Key::Name(n) => view.name(jane_sim::sym::of_name(n)).to_owned(),
            jane_core::Key::Local(i) => bp.local_names.get(i as usize).map(ToOwned::to_owned).unwrap_or_default(),
        };
        let story_camps: Vec<jane_core::Rect> = bp
            .stories
            .values()
            .filter_map(|s| match s {
                jane_core::blueprint::StoryPlace::Placed { kind, bounds, .. } if local(*kind) == "camp" => {
                    Some(*bounds)
                }
                _ => None,
            })
            .collect();
        for p in view.props() {
            let d = cat.story.prop(p.def);
            let (cx, cy) = (i32::from(p.cell.x), i32::from(p.cell.y));
            if matches!(d.id, "camp_fire" | "campfire_cold") {
                let (mx, my) = (cx + i32::from(d.w) / 2, cy + i32::from(d.h) / 2);
                let near = hostile
                    .iter()
                    .filter(|&&(x, y)| (x - mx).abs() <= CAMP_REACH && (y - my).abs() <= CAMP_REACH)
                    .count();
                // A camp the story put there, or a band of ruffians round their own lit fire:
                // not every cold hearth the wild things stand near (the county has over a
                // hundred of those, and crows over all of them would say nothing).
                let story = story_camps.iter().any(|b| b.contains(mx, my));
                let band = d.id == "camp_fire" && near >= RUFFIANS;
                if story || band {
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
        self.landmarks.clear();
        if view.zone() == ZoneId::County {
            self.find_landmarks(view);
        }
    }

    /// Where each region's landmark stands: the chimney over the back of the Works' sheds, the
    /// spire at the west end of St Anne's roof, the statue behind her plinth.
    fn find_landmarks(&mut self, view: &View<'_>) {
        let mark =
            |name: &str| view.sym(name).and_then(|s| view.mark(s)).map(|m| (i32::from(m.cell.x), i32::from(m.cell.y)));
        // The box of the cells `keep` holds within `r` of `(x, y)`, north of it.
        let bbox = |(x, y): (i32, i32), r: i32, keep: &dyn Fn(jane_core::Tile) -> bool| {
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for yy in y - r..=y {
                for xx in x - r..=x + r {
                    if keep(view.tile(xx, yy)) {
                        (x0, y0, x1, y1) = (x0.min(xx), y0.min(yy), x1.max(xx), y1.max(yy));
                    }
                }
            }
            (x0 <= x1).then_some((x0, y0, x1, y1))
        };
        if let Some(m) = mark("factory_mouth") {
            let foot = match bbox(m, 40, &|t| t == jane_core::Tile::Wall) {
                // The stack over the back of the sheds, two thirds of the way east.
                Some((x0, y0, x1, _)) => ((x0 + (x1 - x0) * 2 / 3) * CELL, (y0 + 6) * CELL),
                None => (m.0 * CELL, (m.1 - 12) * CELL),
            };
            self.landmarks.push((Far::Chimney, foot));
        }
        if let Some(m) = mark("church_door") {
            // The roof the door is in: walk up from the door to its ridge and west to its end.
            let roof = |x: i32, y: i32| view.tile(x, y).is_roof();
            let mut y = m.1 - 1;
            while y > m.1 - 8 && !roof(m.0, y) {
                y -= 1;
            }
            let (mut x0, mut y0) = (m.0, y);
            while roof(m.0, y0 - 1) && y0 > m.1 - 40 {
                y0 -= 1;
            }
            while roof(x0 - 1, y) && x0 > m.0 - 30 {
                x0 -= 1;
            }
            let foot = if roof(m.0, y) { ((x0 + 3) * CELL, (y0 + 4) * CELL) } else { (m.0 * CELL, (m.1 - 6) * CELL) };
            self.landmarks.push((Far::Spire, foot));
        }
        if let Some(m) = mark("lake_statue_mouth") {
            let foot = match bbox(m, 14, &|t| t == jane_core::Tile::TempleWall) {
                Some((x0, y0, x1, _)) => ((x0 + x1 + 1) * CELL / 2, y0 * CELL + CELL),
                None => (m.0 * CELL + CELL / 2, (m.1 - 8) * CELL),
            };
            self.landmarks.push((Far::Statue, foot));
        }
    }

    /// A landmark's look at `hour`, where its foot is across its canvas (px from the left),
    /// whether it is mirrored, and its tip's look at `tick`.
    fn landmark_look(&self, kind: Far, hour: u8, tick: u32) -> (RefId, i32, bool, RefId) {
        let a = &self.art;
        let [chimney, spire, statue] = jane_art::far::WORLD_FEET;
        match kind {
            Far::Chimney => (
                a.chimney[usize::from(!(6..18).contains(&hour))],
                chimney,
                false,
                a.plume_tip[(tick / PLUME_TICKS) as usize % PLUME],
            ),
            // Evensong, half past five to eight: a candle in the tower's window.
            Far::Spire => (a.spire[usize::from((17..20).contains(&hour))], spire, false, a.spire_tip),
            // Gilded in the dawn's haze; turned the other way by night.
            Far::Statue => {
                (a.statue[usize::from((5..9).contains(&hour))], statue, !(6..21).contains(&hour), a.statue_tip)
            }
            Far::School => (a.spire[0], spire, false, a.spire_tip),
        }
    }

    /// The landmarks standing in the county at `hour`, to be drawn among the standing things:
    /// each one's foot (zone canvas px), its look, where the foot is across it, and whether it
    /// is mirrored.
    pub fn standing(&self, hour: u8) -> impl Iterator<Item = ((i32, i32), RefId, i32, bool)> + '_ {
        self.landmarks.iter().map(move |&(kind, foot)| {
            let (look, ax, mirror, _) = self.landmark_look(kind, hour, 0);
            (foot, look, ax, mirror)
        })
    }

    /// Over the scene, after the fog: the chimney's plume over its mouth wherever any of it is
    /// in view (smoke, so a little thin), with the furnace's glow under it by night; and each
    /// landmark that stands past the top edge within three screens pokes its tip (the plume,
    /// the spire's point, the statue's hand) in at the edge at its bearing, fainter the farther.
    /// The landmarks themselves stand among the standing things (`Present::read_props`, from
    /// [`Cues::standing`]), as tall as they are.
    fn draw_landmarks(&self, f: &mut Frame, atlas: &Atlas, cam: (i32, i32), tick: u32, hour: u8, sky: &Sky) {
        let (w, h) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        // Below T2 a sprite after the light is drawn as it is: the dark thins what shows of it.
        let luma = (u32::from(sky.ambient[0]) + u32::from(sky.ambient[1]) + u32::from(sky.ambient[2])) / 3;
        let dim = if f.tier >= Tier::T2 { 256 } else { (luma + 64).min(256) };
        let s0 = f.sprites.len();
        let mut glow: Option<(i32, i32)> = None;
        let sprite = |id: RefId, x: i32, y: i32, a: u32| {
            let r = atlas.get(id);
            SpriteCmd {
                page: r.page,
                src: r.src,
                x: x.clamp(-4096, 4096) as i16,
                y: y.clamp(-4096, 4096) as i16,
                flags: Flags {
                    mirror: false,
                    tint: Tint::Ghost((a * dim / 256).min(255) as u8),
                    bend: crate::frame::Bend::NONE,
                },
                height_px: 0,
                foot: None,
            }
        };
        let night = !(6..18).contains(&hour);
        for &(kind, (fx, fy)) in &self.landmarks {
            let (sx, sy) = (fx - cam.0, fy - cam.1);
            if kind == Far::Chimney && sy >= 0 {
                let p = self.art.plume[usize::from(night)][(tick / PLUME_TICKS) as usize % PLUME];
                let pr = atlas.get(p);
                let (mx, my) = (sx, sy - jane_art::far::WORLD_MOUTH.1);
                let (px, py) =
                    (mx - jane_art::far::PLUME_MOUTH.0, my + jane_art::far::PLUME_MOUTH.1 - i32::from(pr.src.h));
                if px + i32::from(pr.src.w) > 0 && px < w && py + i32::from(pr.src.h) > 0 && py < h {
                    f.sprites.push(sprite(p, px, py, 215));
                    if night {
                        glow = Some((mx, my));
                    }
                }
            } else if sy < 0 && sy > -3 * h && sx > -w / 2 && sx < w + w / 2 {
                let (_, _, _, tip_look) = self.landmark_look(kind, hour, tick);
                let tr = atlas.get(tip_look);
                let k = (3 * h + sy) as u32 * 256 / (3 * h) as u32;
                let side = if (0..w).contains(&sx) { 256 } else { 128 };
                let a = TIP_ALPHA * k / 256 * side / 256;
                let tw = i32::from(tr.src.w);
                let tx = if kind == Far::Chimney { sx - jane_art::far::PLUME_MOUTH.0 } else { sx - tw / 2 };
                f.sprites.push(sprite(tip_look, tx.clamp(-tw / 2, w - tw / 2), 0, a));
            }
        }
        if f.sprites.len() > s0 {
            f.passes.push(Pass::Sprites { layer: Depth::NearFog, cmds: Span::since(s0, f.sprites.len()) });
        }
        if let Some((x, y)) = glow {
            let p0 = f.parts.len();
            glint(f, x, y + 3, [255, 120, 60], 12, 230, 255);
            f.passes.push(Pass::Particles { layer: Depth::Canopy, parts: Span::since(p0, f.parts.len()) });
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
                    flags: Flags { mirror: false, tint: Tint::Ghost(a), bend: crate::frame::Bend::NONE },
                    height_px: 0,
                    foot: None,
                });
                f.passes.push(Pass::Sprites { layer: Depth::NearFog, cmds: Span::since(s0, f.sprites.len()) });
            }
        }
        let tick = (t256 / 256) as u32;
        self.draw_landmarks(f, atlas, cam, tick, hour, sky);
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

/// One crow at canvas px `(x, y)`, about nineteen px across: a body with its wedge of a tail, and
/// each wing two px thick to its tip and three at its root (the arm is broader than the
/// hand); up, level, down, or a glide's shallow "v" with the tips lifted, the inner wing a px lower
/// as it banks round the camp. A stroke is laid both ways so it is even to its tip.
fn crow_parts(f: &mut Frame, x: i32, y: i32, c: &Crow, colour: Rgb, up: u8) {
    // (tip dx, tip dy, root dx, root dy) of the left wing; the right is its mirror.
    let (tx, ty, rx, ry) = match c.wings {
        1 => (-6, -6, -4, -3),
        2 => (-9, 0, -5, 1),
        3 => (-8, 3, -4, 2),
        _ => (-9, -2, -5, 0),
    };
    let bank = if c.cw { (0, 1) } else { (1, 0) };
    let part =
        |shape, x: i32, y: i32| Particle { x: x as i16, y: y as i16, shape, colour, alpha: 255, glow: 0, height: up };
    let mut stroke = |a: (i32, i32), b: (i32, i32)| {
        let d = |p: i32, q: i32| (q - p).clamp(-40, 40) as i8;
        f.parts.push(part(PartShape::Streak { dx: d(a.0, b.0), dy: d(a.1, b.1) }, a.0, a.1));
        f.parts.push(part(PartShape::Streak { dx: d(b.0, a.0), dy: d(b.1, a.1) }, b.0, b.1));
    };
    for (side, dip) in [(-1, bank.0), (1, bank.1)] {
        let root = if side < 0 { x - 1 } else { x };
        stroke((root, y), (root + side * -tx, y + ty + dip));
        stroke((root, y + 1), (root + side * -rx, y + 1 + ry + dip));
        stroke((root + side, y + 1), (root + side * (1 - tx), y + 1 + ty + dip));
    }
    f.parts.push(part(PartShape::Dot { size: 2 }, x - 1, y));
    f.parts.push(part(PartShape::Dot { size: 1 }, x - 1, y + 2));
    f.parts.push(part(PartShape::Dot { size: 1 }, x, y - 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether any sprite of a `NearFog` pass in `f` lands on its canvas.
    fn shows(f: &Frame) -> bool {
        let (w, h) = (i32::from(f.canvas.0), i32::from(f.canvas.1));
        f.passes.iter().any(|p| match p {
            Pass::Sprites { layer: Depth::NearFog, cmds } => f.sprites[cmds.range()].iter().any(|s| {
                let (x, y) = (i32::from(s.x), i32::from(s.y));
                x + i32::from(s.src.w) > 0 && x < w && y + i32::from(s.src.h) > 0 && y < h
            }),
            _ => false,
        })
    }

    #[test]
    fn each_region_s_landmark_shows_from_a_screen_and_a_half_off() {
        let mut atlas = Atlas::new();
        let mut c = Cues::new(&mut atlas);
        let sky = crate::light::sky(12 * jane_core::num::TICKS_PER_HOUR, 0, false, 1000, jane_data::Region::Lowfields);
        let (w, h) = (768, 432);
        for seed in [1u32, 4, 7] {
            let sim = jane_sim::Sim::new_game(seed, "Tess");
            let v = sim.view(jane_sim::Seat(0)).unwrap();
            c.zone(&v, |_| None);
            let kinds: Vec<Far> = c.landmarks().iter().map(|l| l.0).collect();
            assert_eq!(kinds, [Far::Chimney, Far::Spire, Far::Statue], "seed {seed}: a landmark a region");
            for (&(kind, (fx, fy)), (_, look, ax, _)) in c.landmarks().iter().zip(c.standing(12)) {
                // A screen and a half south of it, its tip pokes in at the top edge.
                let cam = (fx - w / 2, fy + h * 3 / 2 - h / 2);
                let mut f = Frame::new(Tier::T0);
                f.canvas = (w as u16, h as u16);
                c.draw(&mut f, &atlas, cam, 0, &sky, None, 12, false, 0);
                assert!(shows(&f), "seed {seed}: {kind:?}'s tip shows from a screen and a half south of it");
                // A screen and a half north of it, it stands up over the bottom edge.
                let cam = (fx - w / 2, fy - h * 3 / 2 - h / 2);
                let r = atlas.get(look);
                let (x, y) = (fx - ax - cam.0, fy - i32::from(r.src.h) - cam.1);
                assert!(
                    x + i32::from(r.src.w) > 0 && x < w && y < h,
                    "seed {seed}: {kind:?} stands over the bottom edge from a screen and a half north of it ({y})"
                );
            }
        }
    }

    #[test]
    fn crows_wheel_high_over_every_camp_and_cross_a_screen_from_off_it() {
        let mut atlas = Atlas::new();
        let mut c = Cues::new(&mut atlas);
        for seed in [1u32, 2, 3] {
            let sim = jane_sim::Sim::new_game(seed, "Tess");
            let v = sim.view(jane_sim::Seat(0)).unwrap();
            c.zone(&v, |_| None);
            let n = c.camps().len();
            eprintln!(
                "seed {seed}: {n} camps with crows over them, the first at {:?}",
                c.camps().first().map(|k| (k.x / CELL, k.y / CELL))
            );
            assert!((5..=20).contains(&n), "seed {seed}: the enemy camps, not every hearth ({n})");
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
        let sky = crate::light::sky(12 * jane_core::num::TICKS_PER_HOUR, 0, false, 1000, jane_data::Region::Lowfields);
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
