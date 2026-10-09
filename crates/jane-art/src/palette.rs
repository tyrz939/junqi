//! The one RGB table (ART.md §2.7, §3). Everything else in the crate is an index into it.
//!
//! Layout: index 0 is clear, 1 is the baked contact shadow (AO, "the destination at 70 %"), 2 to 26
//! are the 25 base colours of the TS build's `art/types.ts` in its order (so the county reads as
//! the same place), and from [`RAMP_BASE`] on come the ramps, eight entries each, in luminance
//! order. A ramp is generated from one key colour: its shade tones mix toward a cool dark
//! ([`SHADOW_TINT`]) and its light tones toward a warm cream ([`LIGHT_TINT`]), so shadows lean
//! purple and highlights lean gold across every material. The table is computed at compile time;
//! no colour is written out tone by tone.

use alloc::vec::Vec;

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
    Setts "setts" 0x8e8472,
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
    // People (ART.md §2.1, §8 step 2): hair, the seat coats, cloth, leather, the pool under the
    // fallen. Appended, so the indices above them never move.
    HairBrown "hair_brown" 0x6a4a30,
    HairGrey "hair_grey" 0xa09c98,
    HairWhite "hair_white" 0xd4d0c8,
    HairRed "hair_red" 0xa0482c,
    HairBlack "hair_black" 0x34283a,
    /// Seat 2's coat.
    ClothTeal "cloth_teal" 0x2f7f8c,
    /// Seat 3's coat.
    ClothMoss "cloth_moss" 0x6b8a3a,
    /// Seat 4's coat.
    ClothOchre "cloth_ochre" 0xb8752e,
    ClothNavy "cloth_navy" 0x34406a,
    ClothBlack "cloth_black" 0x363440,
    ClothTweed "cloth_tweed" 0x7a6a4a,
    ClothRose "cloth_rose" 0xa8606a,
    ClothCream "cloth_cream" 0xd0c4a8,
    ClothSky "cloth_sky" 0x5a86a8,
    ClothBrick "cloth_brick" 0x9a5038,
    ClothLinen "cloth_linen" 0xe4dccc,
    Leather "leather" 0x5e3e2a,
    /// The pool under a fallen person: muted, never bright red.
    Pool "pool" 0x5a2c30,
    /// The Works' grass: slag-grey with a little olive left in it (`terrain::region`).
    TurfSlag "turf_slag" 0x6e7456,
    // Autumn (ART-PLAN Q1, ART.md §2.7): laid after the pallid twins (`LATE_BASE`), so no index
    // before them moved when the cap went from 1024 to 2048.
    /// Beech in October: copper gold.
    LeafBeech "leaf_beech" 0xb87a2c,
    /// Oak turned: russet.
    LeafOak "leaf_oak" 0x8e5634,
    /// Field maple and birch: butter yellow.
    LeafMaple "leaf_maple" 0xc4aa3e,
    /// Yew: the churchyard's near-black green.
    LeafYew "leaf_yew" 0x2a4634,
    // Houses with owners (ART-PLAN Q2): the plasters, the knapped flint, a roof newly tiled, the
    // doors' paints and the wisteria over them. Laid after the autumn ramps, so nothing moves.
    /// Suffolk pink plaster.
    PlasterPink "plaster_pink" 0xd6a494,
    /// Ochre plaster.
    PlasterOchre "plaster_ochre" 0xd4ac68,
    /// Limewash: near white, warm.
    Limewash "limewash" 0xe2ddd0,
    /// Knapped flint: blue-grey nodules in their mortar.
    Flint "flint" 0x58606a,
    /// A clay tile roof newly laid: brighter, more orange than the weathered one.
    RoofTileNew "roof_tile_new" 0xb85c3a,
    /// A door's oxblood paint.
    Oxblood "oxblood" 0x6e2a30,
    /// A door's racing green.
    RacingGreen "racing_green" 0x2c5a40,
    /// Wisteria in flower: lilac.
    Wisteria "wisteria" 0x9a88c4,
    // The night (NIGHT.md §4.5): laid last, so nothing moves.
    /// A lamp gone wrong: its glass burns a cold green-white (`#c8eed6` at its brightest);
    /// emits.
    GlassCold "glass_cold" 0x6cb89c,
}

impl Ramp {
    /// The palette index of `tone` in this ramp.
    pub const fn at(self, tone: Tone) -> Ix {
        let r = self as u16;
        if r < LATE {
            Ix(RAMP_BASE + r * RAMP_LEN + tone as u16)
        } else {
            Ix(LATE_BASE + (r - LATE) * RAMP_LEN + tone as u16)
        }
    }

    /// The ramp called `name` in data.
    pub fn by_name(name: &str) -> Option<Ramp> {
        Ramp::ALL.iter().copied().find(|r| r.name() == name)
    }

    /// The ramp and tone an index belongs to, if it is a ramp entry.
    pub const fn of(ix: Ix) -> Option<(Ramp, Tone)> {
        let (base, first) = if ix.0 < RAMP_BASE || (ix.0 >= PALLID_BASE && ix.0 < LATE_BASE) {
            return None;
        } else if ix.0 < PALLID_BASE {
            (RAMP_BASE, 0)
        } else {
            (LATE_BASE, LATE as usize)
        };
        let r = first + ((ix.0 - base) / RAMP_LEN) as usize;
        if r >= Ramp::ALL.len() {
            return None;
        }
        Some((Ramp::ALL[r], Tone::ALL[((ix.0 - base) % RAMP_LEN) as usize]))
    }
}

/// The ramps that have a pallid twin for dead frames (ART.md §4.1): what a person is made of.
/// Their twins follow the ramps, in this order; [`pallor`] maps a tone to its twin's.
pub const PALLID: [Ramp; 29] = [
    Ramp::Skin,
    Ramp::SkinPale,
    Ramp::SkinDark,
    Ramp::HairDark,
    Ramp::HairFair,
    Ramp::HairBrown,
    Ramp::HairGrey,
    Ramp::HairWhite,
    Ramp::HairRed,
    Ramp::HairBlack,
    Ramp::ClothPlum,
    Ramp::ClothMustard,
    Ramp::ClothBrown,
    Ramp::ClothGrey,
    Ramp::ClothRed,
    Ramp::ClothBlue,
    Ramp::ClothGreen,
    Ramp::ClothTeal,
    Ramp::ClothMoss,
    Ramp::ClothOchre,
    Ramp::ClothNavy,
    Ramp::ClothBlack,
    Ramp::ClothTweed,
    Ramp::ClothRose,
    Ramp::ClothCream,
    Ramp::ClothSky,
    Ramp::ClothBrick,
    Ramp::ClothLinen,
    Ramp::Leather,
];

/// The first ramp laid after the pallid twins: every ramp from here on is appended at
/// [`LATE_BASE`], so the indices of the ramps and twins before it never move.
const LATE: u16 = Ramp::LeafBeech as u16;

/// Where the pallid twins start in the table.
pub const PALLID_BASE: u16 = RAMP_BASE + LATE * RAMP_LEN;

/// Where the ramps added after the twins start (the autumn ramps first).
pub const LATE_BASE: u16 = PALLID_BASE + PALLID.len() as u16 * RAMP_LEN;

/// The most entries the master palette may have (ART.md §2.7): the renderers' CLUT is this wide.
pub const CAP: usize = 2048;

/// Entries in the master palette. At most [`CAP`], checked at compile time.
pub const LEN: usize = LATE_BASE as usize + (Ramp::KEYS.len() - LATE as usize) * RAMP_LEN as usize;
const _: () = assert!(LEN <= CAP, "the master palette is at most CAP entries");

/// The position of `r` among the pallid ramps, if it has a twin.
const fn pallid_slot(r: Ramp) -> Option<usize> {
    let mut i = 0;
    while i < PALLID.len() {
        if PALLID[i] as u16 == r as u16 {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The pallor of a dead frame (ART.md §4.1): a ramp tone goes to its pallid twin's (20 % greyer,
/// 14 % darker); a ramp without a twin goes a tone darker; everything else (`k`, `K`, the base
/// colours) stays.
pub const fn pallor(ix: Ix) -> Ix {
    let Some((r, t)) = Ramp::of(ix) else { return ix };
    match pallid_slot(r) {
        Some(k) => Ix(PALLID_BASE + k as u16 * RAMP_LEN + t as u16),
        None => r.at(t.step(-1)),
    }
}

/// Whether `ix` is a pallid twin's tone.
pub const fn is_pallid(ix: Ix) -> bool {
    ix.0 >= PALLID_BASE && ix.0 < LATE_BASE
}

/// A colour made pallid: greyed by 20 % toward its own luma, then darkened by 14 %.
const fn pallid_key(c: u32) -> u32 {
    let [r, g, b] = split(c);
    let l = (299 * r as u32 + 587 * g as u32 + 114 * b as u32) / 1000;
    let rr = (r as u32 * 800 + l * 200) / 1000 * 860 / 1000;
    let gg = (g as u32 * 800 + l * 200) / 1000 * 860 / 1000;
    let bb = (b as u32 * 800 + l * 200) / 1000 * 860 / 1000;
    (rr << 16) | (gg << 8) | bb
}

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

/// Tone `k` of `ramp` on `key`: hue-shifted for the ramps [`crate::hue`] lists, else the
/// step-1 mix toward [`SHADOW_TINT`] and [`LIGHT_TINT`].
const fn ramp_tone(ramp: Ramp, key: [u8; 3], k: usize, dark: [u8; 3], light: [u8; 3]) -> [u8; 3] {
    match crate::hue::shadow_hue(ramp) {
        Some(cool) => crate::hue::tone(key, k, cool, crate::hue::light_hue(ramp)),
        None => {
            let m = TONE_MIX[k];
            if m < 0 { mix(key, dark, -m) } else { mix(key, light, m) }
        }
    }
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
        let (dark, light) = (split(SHADOW_TINT), split(LIGHT_TINT));
        let mut k = 0;
        while k < 8 {
            t[Ramp::ALL[r].at(Tone::ALL[k]).0 as usize] = ramp_tone(Ramp::ALL[r], key, k, dark, light);
            k += 1;
        }
        r += 1;
    }
    let mut p = 0;
    while p < PALLID.len() {
        // Each live tone made pallid, so the twin keeps its ramp's hue shift.
        let live = PALLID[p].at(Tone::Deep).0 as usize;
        let mut k = 0;
        while k < 8 {
            let [r, g, b] = t[live + k];
            let c = (r as u32) << 16 | (g as u32) << 8 | b as u32;
            t[PALLID_BASE as usize + p * 8 + k] = split(pallid_key(c));
            k += 1;
        }
        p += 1;
    }
    t
}

/// The master palette: RGB per index.
pub static PALETTE: [[u8; 3]; LEN] = PALETTE_CONST;

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

/// The contact shadow's multiply per channel in 1/256ths: a cool darkening, blue held up more
/// than red, so a shadow on grass reads as shade and not as grey; deep enough (two fifths off the
/// red at its core) that a thing reads as standing on the ground at 1x, not pasted over it.
pub const AO_TINT: [u16; 3] = [150, 158, 198];

/// The contact shadow over `under`, `cover` of 9 strong: index 1's pixels are a crisp mask, and
/// the blit softens it by how much of each pixel's 3 x 3 the mask covers (a pixel just outside
/// the mask takes a little of it), so the shadow has a soft edge and no dither.
pub fn ao(under: [u8; 3], cover: u32) -> [u8; 3] {
    let cover = cover.min(9);
    let mut out = under;
    for (k, v) in out.iter_mut().enumerate() {
        let f = 256 - (256 - u32::from(AO_TINT[k])) * cover / 9;
        *v = (u32::from(*v) * f / 256) as u8;
    }
    out
}

/// How much of the 3 x 3 round `(x, y)` a canvas's contact shadow covers, 0 to 9.
pub fn ao_cover(c: &crate::Canvas, x: i32, y: i32) -> u32 {
    let mut n = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            n += u32::from(c.get(x + dx, y + dy) == Ix::AO);
        }
    }
    n
}

/// Rec. 601 luma of `ix` in 0..=255000 (thousandths), for luminance order and contrast tests.
pub fn luma(ix: Ix) -> u32 {
    let [r, g, b] = rgb(ix);
    299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)
}

// ---- The night's albedo (NIGHT.md §4.2) ------------------------------------------------------

/// The most night intensity: 0 is the day, 4 the full night colour; each step a quarter of the way.
pub const NIGHT_MAX: u8 = 4;

/// What the night does to a material (NIGHT.md §4.2): each class's colour goes toward its own
/// night material at the colour's own luma, so the hue shifts and the lightness order holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NightClass {
    /// Untouched: a person's ramps (skin, hair, their cloth and leather, the pallid twins), what
    /// emits (lit glass, embers, the wrong lamp's glass), the chrome, the sky, the outlines.
    Keep,
    /// Turf, leaves, crops, hedges, paint greens: toward olive and umber.
    Green,
    /// Plaster, limewash, creams and pinks, sand, bone: toward a stained umber.
    Pale,
    /// Brass, copper, gilt: toward rust.
    Metal,
    /// Unlit glass and ice: toward near-black, a cold tint left in it.
    Glass,
    /// Blue paint, water, slate: toward grey-umber.
    Blue,
    /// Wood, thatch, reed, bark: a little soot.
    Wood,
    /// Stone, setts, gravel, brick, tile: a little stain.
    Stone,
    /// Blooms and wisteria: the window boxes gone brown.
    Bloom,
    /// Iron and pipes: rust in the grain.
    Iron,
    /// Anything else: a little toward umber.
    Other,
}

/// The class of a ramp (NIGHT.md §4.2's list; people and emissives kept).
pub const fn night_class_of(r: Ramp) -> NightClass {
    use NightClass as N;
    if pallid_slot(r).is_some() {
        return N::Keep;
    }
    match r {
        Ramp::UiPanel
        | Ramp::UiInk
        | Ramp::UiSlot
        | Ramp::UiVeil
        | Ramp::UiGold
        | Ramp::UiLag
        | Ramp::GlassLit
        | Ramp::Ember
        | Ramp::GlassCold
        | Ramp::Sky
        | Ramp::Void
        | Ramp::Pool => N::Keep,
        Ramp::Turf
        | Ramp::Grass
        | Ramp::Leaf
        | Ramp::Marsh
        | Ramp::Hedge
        | Ramp::LeafOlive
        | Ramp::LeafDeep
        | Ramp::Needle
        | Ramp::Shrub
        | Ramp::Crop
        | Ramp::TurfDry
        | Ramp::TurfSlag
        | Ramp::LeafYew
        | Ramp::RacingGreen => N::Green,
        Ramp::Plaster
        | Ramp::PlasterPink
        | Ramp::PlasterOchre
        | Ramp::Limewash
        | Ramp::Sand
        | Ramp::Bone
        | Ramp::WoodPale => N::Pale,
        Ramp::Brass | Ramp::Copper | Ramp::LeafBeech | Ramp::LeafMaple => N::Metal,
        Ramp::Glass | Ramp::Ice => N::Glass,
        Ramp::Water | Ramp::Slate | Ramp::Flint | Ramp::Temple | Ramp::TempleWall => N::Blue,
        Ramp::WoodOak | Ramp::WoodDark | Ramp::Thatch | Ramp::Reed | Ramp::Bark | Ramp::Deadwood | Ramp::LeafOak => {
            N::Wood
        }
        Ramp::Stone
        | Ramp::Brick
        | Ramp::Earth
        | Ramp::Gravel
        | Ramp::Mud
        | Ramp::Soil
        | Ramp::Setts
        | Ramp::Rock
        | Ramp::RockFace
        | Ramp::RoofTile
        | Ramp::RoofTileNew
        | Ramp::FloorStone
        | Ramp::Ballast
        | Ramp::Works
        | Ramp::WorksWall
        | Ramp::Cave
        | Ramp::CaveWall
        | Ramp::Museum
        | Ramp::MuseumWall
        | Ramp::WallDark
        | Ramp::SchoolWall
        | Ramp::Oxblood => N::Stone,
        Ramp::Bloom | Ramp::Wisteria => N::Bloom,
        Ramp::Iron | Ramp::Pipe | Ramp::PipeWall => N::Iron,
        _ => N::Other,
    }
}

/// The class of a palette index: its ramp's; the base colours by their letter (the outlines and
/// the skin kept); clear and the contact shadow kept.
pub const fn night_class(ix: Ix) -> NightClass {
    use NightClass as N;
    if ix.0 < 2 {
        return N::Keep;
    }
    if let Some((r, _)) = Ramp::of(ix) {
        return night_class_of(r);
    }
    if is_pallid(ix) || ix.0 >= LEN as u16 {
        return N::Keep;
    }
    match LEGACY[(ix.0 - 2) as usize].0 {
        b'k' | b'K' | b's' | b'S' => N::Keep,
        b'w' | b'W' | b'm' => N::Pale,
        b'g' | b'G' => N::Stone,
        b'l' | b'n' | b'N' => N::Green,
        b'b' | b'B' => N::Blue,
        b'i' => N::Glass,
        b'o' | b'Y' => N::Metal,
        b't' | b'T' | b'e' => N::Wood,
        b'p' | b'P' => N::Bloom,
        _ => N::Other,
    }
}

/// Each class's night material at full intensity: the colour it goes toward (taken at the source
/// colour's luma, times `luma` per mille), and how much of the way, per mille.
const fn night_target(c: NightClass) -> ([u8; 3], u32, u32) {
    use NightClass as N;
    match c {
        N::Keep => ([0, 0, 0], 1000, 0),
        N::Green => ([0x6c, 0x6a, 0x2c], 930, 620),
        N::Pale => ([0x9a, 0x76, 0x58], 860, 720),
        N::Metal => ([0x96, 0x4c, 0x2a], 920, 850),
        N::Glass => ([0x1c, 0x2a, 0x2e], 330, 900),
        N::Blue => ([0x56, 0x58, 0x5c], 880, 460),
        N::Wood => ([0x60, 0x4a, 0x38], 900, 420),
        N::Stone => ([0x74, 0x62, 0x54], 920, 420),
        N::Bloom => ([0x80, 0x56, 0x36], 780, 850),
        N::Iron => ([0x74, 0x56, 0x46], 950, 480),
        N::Other => ([0x70, 0x5c, 0x4a], 940, 300),
    }
}

/// `rgb` as a material of class `c` at night intensity `i` (0 to [`NIGHT_MAX`]): toward its
/// class's night material at its own luma, a quarter of the way a step. Integer; the same on
/// every machine.
pub const fn night_mix(rgb: [u8; 3], c: NightClass, i: u8) -> [u8; 3] {
    let i = if i > NIGHT_MAX { NIGHT_MAX } else { i } as u32;
    let (t, lum, amount) = night_target(c);
    if i == 0 || amount == 0 {
        return rgb;
    }
    let l = 299 * rgb[0] as u32 + 587 * rgb[1] as u32 + 114 * rgb[2] as u32;
    let lt = 299 * t[0] as u32 + 587 * t[1] as u32 + 114 * t[2] as u32;
    let want = l / 1000 * lum;
    let k = amount * i / NIGHT_MAX as u32;
    let mut out = [0u8; 3];
    let mut ch = 0;
    while ch < 3 {
        // The target at the source's luma (held under white), then mixed in.
        let mut tc = t[ch] as u32 * want / if lt == 0 { 1 } else { lt };
        if tc > 250 {
            tc = 250;
        }
        let v = (rgb[ch] as u32 * (1000 - k) + tc * k + 500) / 1000;
        out[ch] = if v < 8 { 8 } else { v as u8 };
        ch += 1;
    }
    out
}

/// Palette index `ix` at night intensity `i`.
pub const fn night_ix(ix: Ix, i: u8) -> [u8; 3] {
    let c = if (ix.0 as usize) < LEN { PALETTE_CONST[ix.0 as usize] } else { PALETTE_CONST[2] };
    night_mix(c, night_class(ix), i)
}

const PALETTE_CONST: [[u8; 3]; LEN] = build();

/// Slots of [`BY_COLOUR`]: a power of two, under half full.
const BY_COLOUR_LEN: usize = 4096;

const fn colour_slot(key: u32) -> usize {
    (key.wrapping_mul(0x9e37_79b1) >> 20) as usize & (BY_COLOUR_LEN - 1)
}

/// Every palette colour and the first index that has it (a person's ramp before a later one of
/// the same key), open-addressed by colour: [`night_class_rgb`]'s lookup, built at compile time.
/// An empty slot holds index 0.
static BY_COLOUR: [(u32, u16); BY_COLOUR_LEN] = {
    let mut t = [(0u32, 0u16); BY_COLOUR_LEN];
    let mut i = 2;
    while i < LEN {
        let c = PALETTE_CONST[i];
        let key = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
        let mut s = colour_slot(key);
        loop {
            if t[s].1 == 0 {
                t[s] = (key, i as u16);
                break;
            }
            if t[s].0 == key {
                break;
            }
            s = (s + 1) & (BY_COLOUR_LEN - 1);
        }
        i += 1;
    }
    t
};

/// The first palette index whose colour is exactly `rgb`.
pub fn index_of_colour(rgb: [u8; 3]) -> Option<Ix> {
    let key = u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2]);
    let mut s = colour_slot(key);
    loop {
        let (k, i) = BY_COLOUR[s];
        if i == 0 {
            return None;
        }
        if k == key {
            return Some(Ix(i));
        }
        s = (s + 1) & (BY_COLOUR_LEN - 1);
    }
}

/// The class a colour alone reads as, where its palette index is not known (a console chunk's
/// CLUT, a page's): the class of the first palette index with exactly that colour, else by its
/// hue and lightness.
pub fn night_class_rgb(rgb: [u8; 3]) -> NightClass {
    if let Some(ix) = index_of_colour(rgb) {
        return night_class(ix);
    }
    let [r, g, b] = rgb.map(i32::from);
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    let chroma = hi - lo;
    if chroma < 14 {
        return if hi > 170 { NightClass::Pale } else { NightClass::Stone };
    }
    if g >= r && g > b {
        NightClass::Green
    } else if b > r && b >= g {
        NightClass::Blue
    } else if r > g && g > b && chroma > 70 && hi > 150 {
        NightClass::Metal
    } else if hi > 175 {
        NightClass::Pale
    } else {
        NightClass::Other
    }
}

/// The night's albedo (NIGHT.md §4.2), a function of the colour alone: `rgb` as its class
/// ([`night_class_rgb`]) reads at intensity `i`. Where the palette index is known, prefer
/// [`night_ix`], which keeps a person's ramps whatever their colour.
pub fn night_of(rgb: [u8; 3], i: u8) -> [u8; 3] {
    night_mix(rgb, night_class_rgb(rgb), i)
}

/// The renderers' CLUT at night intensity `i` (`0xAARRGGBB`, [`CAP`] wide): a sprite's albedo
/// read through it is the night's material, its kept indices as by day.
pub fn night_clut(i: u8) -> Vec<u32> {
    let mut v = alloc::vec![0xff00_0000u32; CAP];
    for (k, e) in v.iter_mut().enumerate().take(LEN) {
        let c = night_ix(Ix(k as u16), i);
        *e = 0xff00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
    }
    v
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
    fn the_late_ramps_follow_the_twins_and_move_nothing() {
        // Before the cap was raised the table ended with the twins at 1011 entries; every index
        // under that is where it was, and the autumn ramps start after it.
        assert_eq!(LATE_BASE, 1011);
        assert_eq!(Ramp::TurfSlag.at(Tone::Glint).0 + 1, PALLID_BASE);
        assert_eq!(Ramp::LeafBeech.at(Tone::Deep), Ix(LATE_BASE));
        const { assert!(LEN > 1024 && LEN <= CAP) };
        assert_eq!(pallor(Ramp::Leather.at(Tone::Base)).0, LATE_BASE - RAMP_LEN + Tone::Base as u16);
        assert!(!is_pallid(Ramp::LeafYew.at(Tone::Base)));
    }

    #[test]
    fn every_ramp_is_in_luminance_order() {
        for &r in Ramp::ALL {
            let l: Vec<u32> = Tone::ALL.iter().map(|&t| luma(r.at(t))).collect();
            assert!(l.windows(2).all(|w| w[0] < w[1]), "{} is out of order: {l:?}", r.name());
        }
    }

    #[test]
    fn no_pure_black_or_white() {
        for (i, c) in PALETTE.iter().enumerate() {
            assert!(*c != [0, 0, 0] && *c != [255, 255, 255], "index {i} is {c:?}");
        }
    }

    /// NIGHT.md §4.2: the night shifts hue, not lightness order. Every ramp keeps its tones in
    /// luminance order at every intensity; a person's ramps and what emits are untouched; a step
    /// of intensity is a quarter of the way.
    #[test]
    fn the_night_shifts_hue_keeps_lightness_order_and_leaves_people_and_light_alone() {
        for &r in Ramp::ALL {
            for i in 0..=NIGHT_MAX {
                let l: Vec<u32> = Tone::ALL
                    .iter()
                    .map(|&t| {
                        let [a, b, c] = night_ix(r.at(t), i);
                        299 * u32::from(a) + 587 * u32::from(b) + 114 * u32::from(c)
                    })
                    .collect();
                assert!(l.windows(2).all(|w| w[0] < w[1]), "{} at {i} is out of order: {l:?}", r.name());
            }
        }
        for r in
            [Ramp::Skin, Ramp::HairDark, Ramp::ClothBlue, Ramp::Leather, Ramp::GlassLit, Ramp::Ember, Ramp::GlassCold]
        {
            for t in Tone::ALL {
                assert_eq!(night_ix(r.at(t), NIGHT_MAX), rgb(r.at(t)), "{} is kept", r.name());
            }
        }
        assert_eq!(night_ix(pallor(Ramp::Skin.at(Tone::Base)), 4), rgb(pallor(Ramp::Skin.at(Tone::Base))));
        assert_eq!(night_ix(Ix::INK, 4), rgb(Ix::INK), "the outline is kept");
        // Turf goes olive: less green over red; plaster stained: darker, warmer; glass dark.
        let (day, night) = (rgb(Ramp::Turf.at(Tone::Base)), night_ix(Ramp::Turf.at(Tone::Base), 4));
        assert!(i32::from(night[1]) - i32::from(night[0]) < i32::from(day[1]) - i32::from(day[0]), "{day:?} {night:?}");
        assert!(luma_of(night_ix(Ramp::Plaster.at(Tone::Base), 4)) < luma(Ramp::Plaster.at(Tone::Base)));
        assert!(luma_of(night_ix(Ramp::Glass.at(Tone::Base), 4)) < luma(Ramp::Glass.at(Tone::Base)) / 2);
        // Halfway is between.
        let (a, b, c) = (
            rgb(Ramp::Brass.at(Tone::Base)),
            night_ix(Ramp::Brass.at(Tone::Base), 2),
            night_ix(Ramp::Brass.at(Tone::Base), 4),
        );
        assert!((0..3).all(|k| b[k].min(a[k].max(c[k])) >= a[k].min(c[k])), "{a:?} {b:?} {c:?}");
        assert_eq!(night_ix(Ramp::Brass.at(Tone::Base), 0), a);
    }

    fn luma_of([r, g, b]: [u8; 3]) -> u32 {
        299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)
    }

    /// The colour-only form reads a palette colour as its index does, and the CLUT is the index form.
    #[test]
    fn the_night_of_a_colour_is_its_first_index_and_the_clut_holds_it() {
        for k in 2..LEN as u16 {
            let ix = Ix(k);
            let first = index_of_colour(rgb(ix)).expect("every colour is found");
            assert!(first.0 <= k && rgb(first) == rgb(ix));
            if first == ix {
                assert_eq!(night_of(rgb(ix), 3), night_ix(ix, 3));
            }
        }
        assert_eq!(index_of_colour([1, 2, 3]), None);
        let clut = night_clut(4);
        assert_eq!(clut.len(), CAP);
        let [r, g, b] = night_ix(Ramp::Turf.at(Tone::Base), 4);
        let want = 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
        assert_eq!(clut[usize::from(Ramp::Turf.at(Tone::Base).0)], want);
        assert!(Ramp::GlassCold.at(Tone::Deep).0 as usize >= 1107, "the night's ramp is laid last");
        let hi = rgb(Ramp::GlassCold.at(Tone::High));
        assert!(hi[1] > hi[0] && hi[1] >= hi[2], "cold green-white: {hi:?}");
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
