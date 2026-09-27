//! The looks table (ART.md §1, §5; PORT.md §5.3): `data/looks/*.json`, keyed by the sprite id a
//! row names, each value a [`Look`]. Its meaning lives in `jane-art`, which renders every entry;
//! nothing else in the game reads a look.
//!
//! The looks are not part of the [`Catalog`](crate::model::Catalog): they change no behaviour, so
//! they are emitted as a static of their own (`LOOKS`) and stay out of the content hash, and a
//! new coat never makes a save or a replay stale.
//!
//! Today, ART.md §8 step 2: the Person family. Ramps are named by the strings `jane-art`'s
//! palette knows (`"cloth_plum"`); `jane-art`'s tests resolve every one.

use jane_core::ids::SpriteId;

use crate::{model, model_enum};

model_enum! {
    /// A person's proportions: a per-build table of eight numbers in `jane-art` (ART.md §2.1).
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Build { Slim, Broad, Child, Stout }
}

model_enum! {
    /// How the hair is worn.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Hair { Short, Cropped, Long, Bun, Pigtails, Bald, Curlers, Wet }
}

model_enum! {
    /// What is on the head, over the hair.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Hat { None, Cap, Brim, Peaked, Helmet, Scarf, Veil, Cloche, Panama, Diving }
}

model_enum! {
    /// The skin ramp; `bone`, `wax`, `stone` and `metal` are the skeleton, the waxwork, the
    /// statue and the armour, `none` with `ghost` is the shade, and `gilt` is Goldskin.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Skin { Skin, SkinPale, SkinDark, Bone, Wax, Stone, Metal, None, Gilt }
}

model_enum! {
    /// What the face carries besides its eyes, nose and mouth.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Face { Plain, Glasses, Beard, Grim, None }
}

model_enum! {
    /// The garment over the body; the seat swap replaces its ramp and nothing else.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Coat { Coat, Dress, Gown, Apron, Smock, Jacket, Nightdress, Overcoat, Canvas, Cardigan }
}

model_enum! {
    /// What shows at the front of the coat; it goes to coat on `up`.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Front { None, Apron, Shirt, Waistcoat, Scarf, Tie, Braces }
}

model_enum! {
    /// What is on the legs.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Legs { Trousers, Skirt, Bare, Pyjamas }
}

model_enum! {
    /// What is on the feet.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Boots { Boots, Shoes, Bare }
}

model_enum! {
    /// A thing in the hand; the composer owns the hand's position per frame.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum HeldItem { None, Hammer, Pole, Suitcase, Dish, Bell, Lantern, Billhook, Broom, Book, Pipe, Net }
}

model_enum! {
    /// Something extra; any number. `stoop`: an old back, the head carried low and forward;
    /// `keys`: a ring of keys at the belt; `knuckles`: iron over the fists.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Extra { WatchChain, BellAnkle, Shawl, Seated, Wet, Stoop, Keys, Knuckles }
}

model_enum! {
    /// A role that may write the emissive layer; any other that does is a test failure.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum EmitRole { Eye, Glass, Held }
}

model! {
    /// The head: hair, hat, skin and face.
    pub struct PersonHead {
        pub hair: Hair,
        /// A ramp of the hair group.
        pub hair_ramp: &'static str,
        pub hat: Hat,
        /// The hat's ramp; `jane-art` picks one when there is none.
        pub hat_ramp: Option<&'static str>,
        pub skin: Skin,
        pub face: Face,
    }
}

model! {
    /// The body: coat, front, legs, boots, pack.
    pub struct PersonBody {
        pub coat: Coat,
        pub coat_ramp: &'static str,
        pub front: Front,
        /// The front's ramp; `jane-art` picks one when there is none.
        pub front_ramp: Option<&'static str>,
        pub legs: Legs,
        pub legs_ramp: &'static str,
        pub boots: Boots,
        /// The boots' ramp; `jane-art` picks leather when there is none.
        pub boots_ramp: Option<&'static str>,
        /// A pack on the back: drawn on `up`, a strap on `side`.
        pub pack: bool,
        /// A bedroll strapped across the top of the pack (ART.md §2.1): a traveller's.
        pub roll: bool,
    }
}

model! {
    /// Per-instance variants (ART.md §3): each list replaces its axis, the product of the list
    /// lengths (an empty list counts one) is at most four, and variant `k` is `k` in mixed radix
    /// over the lists in field order.
    pub struct PersonVary {
        pub hair: &'static [Hair],
        pub hair_ramp: &'static [&'static str],
        pub hat: &'static [Hat],
        pub face: &'static [Face],
        pub coat_ramp: &'static [&'static str],
        pub front_ramp: &'static [&'static str],
        pub legs_ramp: &'static [&'static str],
    }
}

impl PersonVary {
    /// No variation: one variant.
    pub const NONE: PersonVary =
        PersonVary { hair: &[], hair_ramp: &[], hat: &[], face: &[], coat_ramp: &[], front_ramp: &[], legs_ramp: &[] };

    /// The list lengths in field order, an empty list as one.
    pub fn radices(&self) -> [usize; 7] {
        [
            self.hair.len(),
            self.hair_ramp.len(),
            self.hat.len(),
            self.face.len(),
            self.coat_ramp.len(),
            self.front_ramp.len(),
            self.legs_ramp.len(),
        ]
        .map(|n| n.max(1))
    }

    /// How many variants the row has: the product of [`PersonVary::radices`].
    pub fn count(&self) -> usize {
        self.radices().iter().product()
    }
}

model! {
    /// A person (ART.md §2.1): one composer at 32 x 40, feet on (16, 36).
    pub struct PersonLook {
        pub build: Build,
        pub head: PersonHead,
        pub body: PersonBody,
        pub held: HeldItem,
        pub extras: &'static [Extra],
        pub emits: &'static [EmitRole],
        /// Ramps go to mist and the figure is a 50 % checker; height halves.
        pub ghost: bool,
        pub vary: PersonVary,
    }
}

impl PersonLook {
    /// Variant `k` of this look (`k` below [`PersonVary::count`]; wraps), with its `vary` emptied.
    pub fn variant(&self, k: usize) -> PersonLook {
        let v = self.vary;
        let mut out = PersonLook { vary: PersonVary::NONE, ..*self };
        let mut k = k % v.count();
        let mut pick = |n: usize| {
            let n = n.max(1);
            let i = k % n;
            k /= n;
            i
        };
        let i = pick(v.hair.len());
        if let Some(&h) = v.hair.get(i) {
            out.head.hair = h;
        }
        let i = pick(v.hair_ramp.len());
        if let Some(&r) = v.hair_ramp.get(i) {
            out.head.hair_ramp = r;
        }
        let i = pick(v.hat.len());
        if let Some(&h) = v.hat.get(i) {
            out.head.hat = h;
        }
        let i = pick(v.face.len());
        if let Some(&f) = v.face.get(i) {
            out.head.face = f;
        }
        let i = pick(v.coat_ramp.len());
        if let Some(&r) = v.coat_ramp.get(i) {
            out.body.coat_ramp = r;
        }
        let i = pick(v.front_ramp.len());
        if let Some(&r) = v.front_ramp.get(i) {
            out.body.front_ramp = Some(r);
        }
        let i = pick(v.legs_ramp.len());
        if let Some(&r) = v.legs_ramp.get(i) {
            out.body.legs_ramp = r;
        }
        out
    }
}

model_enum! {
    /// A creature's body plan (ART.md §2.2): each has its own box, anchor and gait.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Plan { QuadrupedMid, QuadrupedSmall, Bird, FlyerInsect, FlyerBat, Arachnid, SerpentHead, Plant, Crawler }
}

model_enum! {
    /// Which animal a plan draws: the anatomy (proportions, head, how it sits and how it dies).
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Anatomy {
        Dog, Sheep, Cat, Rat, Rabbit, Fox, Hen, Crow, Butterfly, Moth, Emperor, Bat, Spider, Queen, Lurker, Snake,
        Cactus, Flower, GreatFlower, Pumpkin,
    }
}

model_enum! {
    /// A creature's ears. `flop_one`: one up and one folded, a dog who has heard it all before.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Ears { Flop, FlopOne, Prick, Round, Tall, Side, None }
}

model_enum! {
    /// A creature's tail.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Tail { Plume, Long, Thin, Puff, Stub, Brush, Fan, None }
}

model_enum! {
    /// Markings over the body's colour, in the `mark` and `belly` ramps.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Marking { TanPoints, Blaze, Grizzle, Socks, Tabby, Mask, TipWhite, DarkFace }
}

model! {
    /// A creature's ramps: the pelt or plumage, its underside, its markings.
    pub struct CreatureRamps {
        pub body: &'static str,
        pub belly: Option<&'static str>,
        pub mark: Option<&'static str>,
    }
}

model! {
    /// A creature (ART.md §2.2): a plan, an anatomy, and its features.
    pub struct CreatureLook {
        pub plan: Plan,
        pub anatomy: Anatomy,
        pub ramps: CreatureRamps,
        pub ears: Ears,
        pub tail: Tail,
        pub markings: &'static [Marking],
        /// A collar's ramp, with a brass tag.
        pub collar: Option<&'static str>,
        pub emits: &'static [EmitRole],
    }
}

model_enum! {
    /// A prop's family (ART.md §2.3): one routine each, its shapes by name.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum PropFamily {
        Container, Furniture, Sign, Lamp, Machine, Barrier, Vegetation, Debris, SmallThing, Ritual, Structure,
    }
}

model_enum! {
    /// A prop's drawn states: `base` at rest, `on` lit or thrown, `open` with its lid up.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum PropState { Base, On, Open }
}

model_enum! {
    /// Where a lamp or a sign is fixed: on a post or the floor, or on a wall facing its way.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Mount { Floor, Post, WallN, WallE, WallS, WallW }
}

model! {
    /// A prop's materials, as ramps: its body, its trim (bands, a frame), and an accent.
    pub struct PropMaterials {
        pub body: &'static str,
        pub trim: Option<&'static str>,
        pub accent: Option<&'static str>,
    }
}

model! {
    /// A prop (ART.md §2.3): a family's shape at the size of the footprint its rows give.
    pub struct PropLook {
        pub family: PropFamily,
        /// The family's shape, by name (`"barrel"`); `jane-art` knows the names.
        pub shape: &'static str,
        /// How far it stands above its footprint, px: what its heights top out at.
        pub rise: u8,
        pub materials: PropMaterials,
        pub states: &'static [PropState],
        /// `base_2` and `base_3`: up to two more renders at the next seeds.
        pub vary: u8,
        pub mount: Mount,
        /// Rows of illegible writing on a sign.
        pub text_rows: u8,
        pub emits: &'static [EmitRole],
    }
}

model_enum! {
    /// A building's style (ART.md §2.4): its proportions and its details.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum HouseStyle { Cottage, Farmhouse, Barn, Inn, Shed, Hut, Steeple }
}

model_enum! {
    /// A roof's material.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Roofing { Thatch, Slate, Tile, Tin, Reed }
}

model_enum! {
    /// A wall's material: plaster, timber framing over plaster, stone, brick or boards.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Walling { Plaster, Timber, Stone, Brick, Board, Reed }
}

model! {
    /// A building (ART.md §2.4): the `house()` painter's row.
    pub struct HouseLook {
        pub style: HouseStyle,
        pub storeys: u8,
        pub roof: Roofing,
        pub wall: Walling,
        pub dormers: bool,
        pub porch: bool,
        pub lean_to: bool,
        pub boarded: bool,
        /// Its mass against the sky only: the title and the far landmark.
        pub silhouette: bool,
        /// How far it stands above its footprint, px.
        pub rise: u8,
        /// The door's and the trim's ramps.
        pub door: &'static str,
        pub trim: &'static str,
        /// Its windows glow at night.
        pub lit: bool,
    }
}

model_enum! {
    /// An icon's class (ART.md §2.5): the object's shape.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum IconClass {
        Flask, Vial, Key, Bar, Orb, Stone, Gem, Herb, Bloom, Mushroom, Fruit, Grapes, Egg, Loaf, Crepe, Meat, Potatoes,
        Pot, Tin, Glove, Hat, Cap, Coat, Scarf, Fleece, Letter, Parcel, Sack, Tool, Scissors, Spanner, Spoons, Can, Net,
        Plate, Ring, Amulet, Spectacles, Logs, Butterfly, Tortoise, Dust, Spell, Status,
    }
}

model_enum! {
    /// The mark on a spell's disc or a status ring, or an icon's small overlay.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum IconMark { None, Flame, Frost, Leaf, Bolt, Burst, Fist, Hammer, Web, Skull, Shield, Drop, Star, Thorn, Heart, Cork }
}

model! {
    /// An icon (ART.md §2.5): a class in a material's ramp, with a mark; drawn at 32 and again
    /// at 16, never downscaled.
    pub struct IconLook {
        pub class: IconClass,
        pub ramp: &'static str,
        /// A second ramp: a flask's glass, a key's ring, a status's rim.
        pub trim: Option<&'static str>,
        pub mark: IconMark,
        /// It glows a little (a light stone, an orb, a potion).
        pub glow: bool,
    }
}

/// A look: what a sprite id is drawn as. One variant per generator family as the families land
/// (ART.md §8): people, creatures, props, buildings and icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Person(PersonLook),
    Creature(CreatureLook),
    Prop(PropLook),
    Building(HouseLook),
    Icon(IconLook),
}

impl crate::emit::Emit for Look {
    fn emit(&self, out: &mut String) {
        let (name, inner): (&str, &dyn crate::emit::Emit) = match self {
            Look::Person(p) => ("Look::Person(", p),
            Look::Creature(c) => ("Look::Creature(", c),
            Look::Prop(p) => ("Look::Prop(", p),
            Look::Building(b) => ("Look::Building(", b),
            Look::Icon(i) => ("Look::Icon(", i),
        };
        out.push_str(name);
        inner.emit(out);
        out.push(')');
    }
}

/// The compiled looks table: every look by the sprite id it draws, in file then key order.
pub type Looks = &'static [(SpriteId, Look)];
