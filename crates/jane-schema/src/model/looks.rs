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
    /// statue and the armour, and `none` with `ghost` is the shade.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Skin { Skin, SkinPale, SkinDark, Bone, Wax, Stone, Metal, None }
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
    pub enum HeldItem { None, Hammer, Pole, Suitcase, Dish, Bell, Lantern, Billhook, Broom, Book, Pipe }
}

model_enum! {
    /// Something extra; any number. `stoop`: an old back, the head carried low and forward.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "snake_case"))]
    pub enum Extra { WatchChain, BellAnkle, Shawl, Seated, Wet, Stoop }
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

/// A look: what a sprite id is drawn as. One variant per generator family as the families land
/// (ART.md §8); today, people.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Person(PersonLook),
}

impl crate::emit::Emit for Look {
    fn emit(&self, out: &mut String) {
        match self {
            Look::Person(p) => {
                out.push_str("Look::Person(");
                p.emit(out);
                out.push(')');
            }
        }
    }
}

/// The compiled looks table: every look by the sprite id it draws, in file then key order.
pub type Looks = &'static [(SpriteId, Look)];
