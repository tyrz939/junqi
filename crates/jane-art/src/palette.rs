//! The one RGB table (ART.md §2.7, §3). Everything else in the crate is an index into it.
//!
//! Layout: index 0 is clear, 1 is the baked contact shadow (AO, "the destination at 70 %"), 2 to 26
//! are the 25 base colours of the TS build's `art/types.ts` in its order (so the county reads as
//! the same place), and from [`RAMP_BASE`] on come the ramps, eight entries each, in luminance
//! order. A ramp is generated from one key colour: its shade tones mix toward a cool dark
//! ([`SHADOW_TINT`]) and its light tones toward a warm cream ([`LIGHT_TINT`]), so shadows lean
//! purple and highlights lean gold across every material. The table is computed at compile time;
//! no colour is written out tone by tone.

/// A master-palette index. Albedo and emissive layers hold these; only this module holds colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ix(pub u16);

impl Ix {
    /// Clear: nothing drawn. Height 0, normal flat.
    pub const CLEAR: Ix = Ix(0);
    /// The baked contact shadow: the blit darkens what is under it to 70 %. Not a surface.
    pub const AO: Ix = Ix(1);
    /// `k`, the outline: near black, never pure black.
    pub const INK: Ix = Ix(2);
    /// `K`, the soft outline for interior seams.
    pub const SEAM: Ix = Ix(3);
    /// `W`, off white: the chrome's bevel light.
    pub const BEVEL_LIGHT: Ix = Ix(5);
    /// `G`, dark grey: the chrome's bevel shade and a bar's track.
    pub const BEVEL_SHADE: Ix = Ix(7);

    /// Opaque: a drawn surface (not clear and not the AO shadow).
    pub const fn is_opaque(self) -> bool {
        self.0 >= 2
    }
}

/// The TS build's palette letters in its order; `LEGACY[i]` is index `2 + i`.
const LEGACY: [(u8, u32); 25] = [
    (b'k', 0x1a1420),
    (b'K', 0x2e2838),
    (b'w', 0xf4f0e6),
    (b'W', 0xcfc8b8),
    (b'g', 0x8a8f98),
    (b'G', 0x555a66),
    (b's', 0xe8b890),
    (b'S', 0xc08860),
    (b'r', 0xc8403c),
    (b'R', 0x7a2430),
    (b'o', 0xe08838),
    (b'y', 0xf0d048),
    (b'Y', 0xb08828),
    (b'l', 0x78c850),
    (b'n', 0x3c8844),
    (b'N', 0x22503a),
    (b'b', 0x58a8e8),
    (b'B', 0x3060b0),
    (b'i', 0xa8e0f8),
    (b'p', 0xa868c8),
    (b'P', 0x5c3878),
    (b't', 0xa87848),
    (b'T', 0x6e4a2c),
    (b'm', 0xc89868),
    (b'e', 0x4a3626),
];

/// The index of a TS palette letter (`k`, `K`, `w`, ... `e`), for code ported from the TS build.
pub fn letter(c: char) -> Option<Ix> {
    LEGACY.iter().position(|&(l, _)| char::from(l) == c).map(|i| Ix(2 + i as u16))
}

/// Where the ramps start in the table.
pub const RAMP_BASE: u16 = 2 + LEGACY.len() as u16;
/// Entries per ramp.
pub const RAMP_LEN: u16 = 8;

/// Shade tones mix toward this cool dark (0xRRGGBB).
pub const SHADOW_TINT: u32 = 0x1c1428;
/// Light tones mix toward this warm cream (0xRRGGBB).
pub const LIGHT_TINT: u32 = 0xfff2d0;

/// A tone of a ramp, darkest first. A generator asks by tone, never by index (ART.md §2.7):
/// `deep, shade, base, light, high, glint`, with `Mid` and `Lift` the two half-steps for
/// dither pairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tone {
    /// The darkest: core shadow, the far rim.
    Deep = 0,
    /// Shadow.
    Shade = 1,
    /// Half a step between shade and base, for dither pairs.
    Mid = 2,
    /// The key colour itself.
    Base = 3,
    /// Half a step between base and light, for dither pairs.
    Lift = 4,
    /// Lit.
    Light = 5,
    /// Highlight.
    High = 6,
    /// The brightest: a glint, a specular point.
    Glint = 7,
}

impl Tone {
    /// Every tone, darkest first.
    pub const ALL: [Tone; 8] =
        [Tone::Deep, Tone::Shade, Tone::Mid, Tone::Base, Tone::Lift, Tone::Light, Tone::High, Tone::Glint];

    /// The tone `steps` lighter (negative: darker), clamped to the ramp.
    pub const fn step(self, steps: i32) -> Tone {
        let i = self as i32 + steps;
        let i = if i < 0 {
            0
        } else if i > 7 {
            7
        } else {
            i
        };
        Tone::ALL[i as usize]
    }
}

/// Mix per tone, permille: negative toward [`SHADOW_TINT`], positive toward [`LIGHT_TINT`].
const TONE_MIX: [i32; 8] = [-560, -360, -180, 0, 150, 300, 460, 640];

macro_rules! ramps {
    ($($(#[$m:meta])* $v:ident $name:literal $key:literal,)*) => {
        /// A ramp of eight tones in the master palette, by material. Data names it by
        /// [`Ramp::name`] (`"cloth_plum"`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Ramp { $($(#[$m])* #[doc = concat!("`", $name, "`.")] $v,)* }

        impl Ramp {
            /// Every ramp, in table order.
            pub const ALL: &'static [Ramp] = &[$(Ramp::$v,)*];
            const KEYS: &'static [u32] = &[$($key,)*];

            /// The name data uses for this ramp.
            pub const fn name(self) -> &'static str {
                match self { $(Ramp::$v => $name,)* }
            }
        }
    };
}

ramps! {
    /// Chrome: the panel fill, drawn at 85 % ([`alpha`]).
    UiPanel "ui_panel" 0x34304a,
    /// Chrome: text on a panel.
    UiInk "ui_ink" 0xd8d0c0,
    /// Chrome: the slot's dark well.
    UiSlot "ui_slot" 0x242232,
    /// Chrome: the cooldown and GCD veil, drawn at 30 % ([`alpha`]).
    UiVeil "ui_veil" 0xcfc8b8,
    /// Chrome: focus and selection.
    UiGold "ui_gold" 0xd0a840,
    /// Chrome: a bar's damage-lag tail.
    UiLag "ui_lag" 0xe0c8a0,
    Stone "stone" 0x8c8678,
    Slate "slate" 0x5c6474,
    Brick "brick" 0xa4543e,
    Plaster "plaster" 0xd4c8aa,
    WoodOak "wood_oak" 0x9a6a40,
    WoodDark "wood_dark" 0x6a4a30,
    WoodPale "wood_pale" 0xc49a68,
    Iron "iron" 0x6c7080,
    Brass "brass" 0xc09040,
    Copper "copper" 0xb46a44,
    Glass "glass" 0x78a8c0,
    /// Lit glass: a lamp at night. Also its emissive colour.
    GlassLit "glass_lit" 0xf0b050,
    /// Embers and flame; emits.
    Ember "ember" 0xe06a30,
    Skin "skin" 0xe0b08a,
    SkinPale "skin_pale" 0xecccb0,
    SkinDark "skin_dark" 0x8a5a3e,
    Bone "bone" 0xd4ccb2,
    HairDark "hair_dark" 0x3e2c28,
    HairFair "hair_fair" 0xcca460,
    ClothPlum "cloth_plum" 0x7a4a78,
    ClothMustard "cloth_mustard" 0xc4983a,
    ClothBrown "cloth_brown" 0x7a5a3a,
    ClothGrey "cloth_grey" 0x7c7e88,
    ClothRed "cloth_red" 0xa83e3e,
    ClothBlue "cloth_blue" 0x3e5c9a,
    ClothGreen "cloth_green" 0x4a7a4e,
    Leaf "leaf" 0x4e8a3c,
    Grass "grass" 0x78a652,
    Bark "bark" 0x6a5242,
    Bloom "bloom" 0xd46a8a,
    Reed "reed" 0xb0a062,
    Water "water" 0x3a76a6,
    Sky "sky" 0x7aaee0,
    // Terrain and flora (ART.md §2.6): the county's ground, walls, roofs and growing things.
    /// Meadow grass: the Lowfields' warm green.
    Turf "turf" 0x5f9a45,
    /// Bare earth, a lane.
    Earth "earth" 0x94704a,
    /// A made road's gravel.
    Gravel "gravel" 0x9c8e76,
    Sand "sand" 0xd2bc86,
    /// Marsh and moss.
    Marsh "marsh" 0x4c7044,
    /// Mud dried and split.
    Mud "mud" 0x80745e,
    /// Dug soil.
    Soil "soil" 0x6e5236,
    /// Setts and flags.
    Setts "setts" 0x86817a,
    Ice "ice" 0xa8d4e4,
    /// A cliff's top.
    Rock "rock" 0x8a7a64,
    /// A cliff's face.
    RockFace "rock_face" 0x6a5646,
    RoofTile "roof_tile" 0xa04a38,
    Thatch "thatch" 0xb49052,
    /// Stone floor slabs.
    FloorStone "floor_stone" 0x8c8478,
    /// The dressed walls of cellars and crypts.
    WallDark "wall_dark" 0x4a4452,
    Cave "cave" 0x6a5a48,
    CaveWall "cave_wall" 0x3e332c,
    Temple "temple" 0x5c6672,
    TempleWall "temple_wall" 0x323a48,
    Museum "museum" 0x9a8e7c,
    MuseumWall "museum_wall" 0x564650,
    Pipe "pipe" 0x566260,
    PipeWall "pipe_wall" 0x344042,
    Works "works" 0x6c6660,
    WorksWall "works_wall" 0x483c38,
    SchoolWall "school_wall" 0x46564e,
    /// Railway ballast.
    Ballast "ballast" 0x655a4e,
    /// Outside the zone.
    Void "void" 0x2a2432,
    Hedge "hedge" 0x2f6642,
    /// A drier, olive wood.
    LeafOlive "leaf_olive" 0x6a8038,
    /// The dark wet-wood green.
    LeafDeep "leaf_deep" 0x2f6450,
    /// Conifer needles.
    Needle "needle" 0x2c5c46,
    /// Shrubs.
    Shrub "shrub" 0x3f8442,
    /// Crops and seedlings.
    Crop "crop" 0x74a848,
    /// A dead tree's grey wood.
    Deadwood "deadwood" 0x6e665e,
    /// Meadow grass where it runs dry: the olive of a summer's end.
    TurfDry "turf_dry" 0x8c9a46,
}

/// Where a hue-shifted ramp's tones lean (ART.md §2.7, §3): its shade tones mix toward `.1`, a
/// cool dark (blue, violet), and its light tones toward `.2`, a warm light (yellow), in place of
/// [`SHADOW_TINT`] and [`LIGHT_TINT`]. So the shadows of grass run to deep blue-green and its
/// lights to a sunlit yellow-green, and saturation peaks in the midtones: light on a surface, not
/// one hue from dark to pale. The terrain's and flora's ramps.
const TINTS: [(Ramp, u32, u32); 45] = {
    const GREEN: (u32, u32) = (0x0a1c40, 0xfff6c0);
    const EARTH: (u32, u32) = (0x1e1038, 0xfff4d4);
    const STONE: (u32, u32) = (0x121840, 0xfff0c4);
    const WATER: (u32, u32) = (0x040a34, 0xe6fff0);
    const RED: (u32, u32) = (0x240c34, 0xffe8c4);
    const WOOD: (u32, u32) = (0x1c0e34, 0xfff4dc);
    const PLASTER: (u32, u32) = (0x2a2254, 0xfffadc);
    [
        (Ramp::Grass, GREEN.0, GREEN.1),
        (Ramp::Leaf, GREEN.0, GREEN.1),
        (Ramp::Turf, GREEN.0, GREEN.1),
        (Ramp::TurfDry, GREEN.0, GREEN.1),
        (Ramp::Marsh, GREEN.0, 0xf6f4d6),
        (Ramp::Hedge, GREEN.0, GREEN.1),
        (Ramp::LeafOlive, GREEN.0, GREEN.1),
        (Ramp::LeafDeep, GREEN.0, GREEN.1),
        (Ramp::Needle, GREEN.0, GREEN.1),
        (Ramp::Shrub, GREEN.0, GREEN.1),
        (Ramp::Crop, GREEN.0, GREEN.1),
        (Ramp::Earth, EARTH.0, EARTH.1),
        (Ramp::Gravel, EARTH.0, EARTH.1),
        (Ramp::Sand, EARTH.0, EARTH.1),
        (Ramp::Mud, EARTH.0, EARTH.1),
        (Ramp::Soil, EARTH.0, EARTH.1),
        (Ramp::Ballast, EARTH.0, EARTH.1),
        (Ramp::Cave, EARTH.0, EARTH.1),
        (Ramp::Reed, EARTH.0, 0xfffac0),
        (Ramp::Thatch, EARTH.0, EARTH.1),
        (Ramp::Stone, STONE.0, STONE.1),
        (Ramp::Slate, STONE.0, STONE.1),
        (Ramp::Setts, STONE.0, STONE.1),
        (Ramp::Rock, STONE.0, STONE.1),
        (Ramp::RockFace, STONE.0, STONE.1),
        (Ramp::FloorStone, STONE.0, STONE.1),
        (Ramp::WallDark, STONE.0, STONE.1),
        (Ramp::CaveWall, STONE.0, STONE.1),
        (Ramp::Temple, STONE.0, STONE.1),
        (Ramp::TempleWall, STONE.0, STONE.1),
        (Ramp::Museum, STONE.0, STONE.1),
        (Ramp::MuseumWall, STONE.0, STONE.1),
        (Ramp::Pipe, STONE.0, STONE.1),
        (Ramp::PipeWall, STONE.0, STONE.1),
        (Ramp::Works, STONE.0, STONE.1),
        (Ramp::WorksWall, STONE.0, STONE.1),
        (Ramp::SchoolWall, STONE.0, STONE.1),
        (Ramp::Deadwood, STONE.0, STONE.1),
        (Ramp::Water, WATER.0, WATER.1),
        (Ramp::Ice, WATER.0, WATER.1),
        (Ramp::Brick, RED.0, RED.1),
        (Ramp::RoofTile, RED.0, RED.1),
        (Ramp::Bark, WOOD.0, WOOD.1),
        (Ramp::WoodOak, WOOD.0, WOOD.1),
        (Ramp::Plaster, PLASTER.0, PLASTER.1),
    ]
};

/// The dark and light a ramp's tones lean to: its [`TINTS`] row, or the defaults.
const fn tints(r: usize) -> ([u8; 3], [u8; 3]) {
    let mut i = 0;
    while i < TINTS.len() {
        if TINTS[i].0 as usize == r {
            return (split(TINTS[i].1), split(TINTS[i].2));
        }
        i += 1;
    }
    (split(SHADOW_TINT), split(LIGHT_TINT))
}

/// Whether a ramp is hue-shifted (has a [`TINTS`] row): the ones the hue test holds to.
pub fn hue_shifted(r: Ramp) -> bool {
    TINTS.iter().any(|t| t.0 == r)
}

impl Ramp {
    /// The palette index of `tone` in this ramp.
    pub const fn at(self, tone: Tone) -> Ix {
        Ix(RAMP_BASE + self as u16 * RAMP_LEN + tone as u16)
    }

    /// The ramp called `name` in data.
    pub fn by_name(name: &str) -> Option<Ramp> {
        Ramp::ALL.iter().copied().find(|r| r.name() == name)
    }

    /// The ramp and tone an index belongs to, if it is a ramp entry.
    pub const fn of(ix: Ix) -> Option<(Ramp, Tone)> {
        if ix.0 < RAMP_BASE {
            return None;
        }
        let r = ((ix.0 - RAMP_BASE) / RAMP_LEN) as usize;
        if r >= Ramp::ALL.len() {
            return None;
        }
        Some((Ramp::ALL[r], Tone::ALL[((ix.0 - RAMP_BASE) % RAMP_LEN) as usize]))
    }
}

/// Entries in the master palette. At most 1024 (ART.md §2.7), checked at compile time.
pub const LEN: usize = RAMP_BASE as usize + Ramp::KEYS.len() * RAMP_LEN as usize;
const _: () = assert!(LEN <= 1024, "the master palette is at most 1024 entries");

const fn split(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

/// `a` mixed toward `b` by `t` permille, rounded.
const fn mix(a: [u8; 3], b: [u8; 3], t: i32) -> [u8; 3] {
    let mut out = [0u8; 3];
    let mut i = 0;
    while i < 3 {
        let (x, y) = (a[i] as i32, b[i] as i32);
        out[i] = ((x * (1000 - t) + y * t + 500) / 1000) as u8;
        i += 1;
    }
    out
}

const fn build() -> [[u8; 3]; LEN] {
    let mut t = [[0u8; 3]; LEN];
    // Index 0 is never shown; index 1 is shown only as a darkening. Both hold k so that the
    // table has no black in it.
    t[0] = split(LEGACY[0].1);
    t[1] = split(LEGACY[0].1);
    let mut i = 0;
    while i < LEGACY.len() {
        t[2 + i] = split(LEGACY[i].1);
        i += 1;
    }
    let mut r = 0;
    while r < Ramp::KEYS.len() {
        let key = split(Ramp::KEYS[r]);
        let (dark, light) = tints(r);
        let mut k = 0;
        while k < 8 {
            let m = TONE_MIX[k];
            let c = if m < 0 { mix(key, dark, -m) } else { mix(key, light, m) };
            t[RAMP_BASE as usize + r * 8 + k] = c;
            k += 1;
        }
        r += 1;
    }
    t
}

/// The master palette: RGB per index.
pub static PALETTE: [[u8; 3]; LEN] = build();

/// The colour of `ix`. Clear and AO return `k`; the blit treats them specially.
pub fn rgb(ix: Ix) -> [u8; 3] {
    PALETTE.get(usize::from(ix.0)).copied().unwrap_or(PALETTE[2])
}

/// Coverage of `ix` in 0..=255 when chrome is laid over the scene: 0 for clear, 217 (85 %) for the
/// panel fill, 77 (30 %) for the cooldown veil, 255 for everything else. Sprites have no alpha
/// (ART.md §2.1, `ghost`); only chrome reads this.
pub fn alpha(ix: Ix) -> u8 {
    match Ramp::of(ix) {
        _ if ix == Ix::CLEAR => 0,
        Some((Ramp::UiPanel, _)) => 217,
        Some((Ramp::UiVeil, _)) => 77,
        _ => 255,
    }
}

/// Rec. 601 luma of `ix` in 0..=255000 (thousandths), for luminance order and contrast tests.
pub fn luma(ix: Ix) -> u32 {
    let [r, g, b] = rgb(ix);
    299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_fits_and_starts_where_the_contract_says() {
        assert_eq!(Ix::CLEAR.0, 0);
        assert_eq!(Ix::AO.0, 1);
        assert_eq!(letter('k'), Some(Ix::INK));
        assert_eq!(letter('K'), Some(Ix::SEAM));
        assert_eq!(letter('W'), Some(Ix::BEVEL_LIGHT));
        assert_eq!(letter('G'), Some(Ix::BEVEL_SHADE));
        assert_eq!(letter('e'), Some(Ix(RAMP_BASE - 1)));
    }

    #[test]
    fn every_ramp_is_in_luminance_order() {
        for &r in Ramp::ALL {
            let l: Vec<u32> = Tone::ALL.iter().map(|&t| luma(r.at(t))).collect();
            assert!(l.windows(2).all(|w| w[0] < w[1]), "{} is out of order: {l:?}", r.name());
        }
    }

    /// Hue in degrees and chroma (max - min channel, 0..=255), or `None` for a grey.
    fn hue_sat(c: [u8; 3]) -> Option<(i32, i32)> {
        let [r, g, b] = c.map(i32::from);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let d = max - min;
        if d == 0 {
            return None;
        }
        let h = if max == r {
            (60 * (g - b) / d).rem_euclid(360)
        } else if max == g {
            60 * (b - r) / d + 120
        } else {
            60 * (r - g) / d + 240
        };
        Some((h, d))
    }

    /// Degrees between two hues, 0..=180.
    fn apart(a: i32, b: i32) -> i32 {
        let d = (a - b).rem_euclid(360);
        d.min(360 - d)
    }

    /// ART.md §2.7: a hue-shifted ramp's shadows lean cool and its lights warm. Its deep and shade
    /// tones sit nearer blue (240°) than its base does, its light and high tones nearer yellow
    /// (60°); and a coloured one (base chroma 40 or more) peaks in its midtones: neither end tone
    /// has more chroma than the most colourful of mid, base and lift. A near-grey (stone, slate)
    /// is let take its colour at the ends, which is where its cool shadows and warm lights are.
    #[test]
    fn hue_shifted_ramps_lean_cool_in_shadow_and_warm_in_light() {
        let mut bad = Vec::new();
        for &r in Ramp::ALL.iter().filter(|&&r| hue_shifted(r)) {
            let hs = |t: Tone| hue_sat(rgb(r.at(t)));
            let Some((hb, cb)) = hs(Tone::Base) else {
                bad.push(format!("{}: its base is grey", r.name()));
                continue;
            };
            for t in [Tone::Deep, Tone::Shade] {
                if let Some((h, _)) = hs(t) {
                    if apart(h, 240) > apart(hb, 240) {
                        bad.push(format!("{} {t:?} at {h}° is warmer than its base at {hb}°", r.name()));
                    }
                }
            }
            for t in [Tone::Light, Tone::High] {
                if let Some((h, _)) = hs(t) {
                    if apart(h, 60) > apart(hb, 60) {
                        bad.push(format!("{} {t:?} at {h}° is cooler than its base at {hb}°", r.name()));
                    }
                }
            }
            let peak =
                [Tone::Mid, Tone::Base, Tone::Lift].iter().filter_map(|&t| hs(t)).map(|(_, s)| s).max().unwrap_or(0);
            for t in [Tone::Deep, Tone::Glint].into_iter().filter(|_| cb >= 40) {
                if let Some((_, s)) = hs(t) {
                    if s > peak {
                        bad.push(format!("{} {t:?} is more saturated ({s}) than its midtones ({peak})", r.name()));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn no_pure_black_or_white() {
        for (i, c) in PALETTE.iter().enumerate() {
            assert!(*c != [0, 0, 0] && *c != [255, 255, 255], "index {i} is {c:?}");
        }
    }

    #[test]
    fn names_round_trip_and_are_unique() {
        for &r in Ramp::ALL {
            assert_eq!(Ramp::by_name(r.name()), Some(r));
            for t in Tone::ALL {
                assert_eq!(Ramp::of(r.at(t)), Some((r, t)));
            }
        }
        assert_eq!(Ramp::of(Ix(RAMP_BASE - 1)), None);
        assert_eq!(Ramp::of(Ix(LEN as u16)), None);
    }
}
