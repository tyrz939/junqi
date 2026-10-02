//! Compile `data/looks/*.json` into the looks table ([`model::Looks`]; ART.md §1, §5).
//!
//! Run after every group, so every sprite a row names is interned; it only looks sprites up and
//! never interns one, so the catalog is the same with or without it.
//!
//! The checks:
//!
//! - a look is keyed by a sprite some row names (a look no row uses is an error);
//! - `vary` lists multiply to at most four variants, and no list repeats a value;
//! - `emits: ["held"]` needs a held thing, and `emits: ["glass"]` needs glasses.
//!
//! That every sprite a row names has a look is the P5 gate's rule (PORT.md §7.1): until then a
//! row without a look draws a stand-in, and `jane-art`'s coverage test holds the families that
//! have landed.

use serde::Deserialize;

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::source::{Source, typed};
use crate::model::{
    Anatomy, Boots, Build, Coat, CreatureLook, CreatureRamps, Ears, EmitRole, Extra, Face, Front, Hair, Hat, HeldItem,
    HouseLook, HouseStyle, IconClass, IconLook, IconMark, Legs, Look, Marking, Mount, PersonBody, PersonHead,
    PersonLook, PersonVary, Plan, PropFamily, PropLook, PropMaterials, PropState, Roofing, Skin, Tail, Walling,
};
use jane_core::ids::SpriteId;

#[derive(Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
enum RawLook {
    Person(Box<RawPerson>),
    Creature(RawCreature),
    Container(RawProp),
    Furniture(RawProp),
    Sign(RawProp),
    Lamp(RawProp),
    Machine(RawProp),
    Barrier(RawProp),
    Vegetation(RawProp),
    Debris(RawProp),
    SmallThing(RawProp),
    Ritual(RawProp),
    Structure(RawProp),
    Building(RawHouse),
    Icon(RawIcon),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIcon {
    class: IconClass,
    ramp: String,
    #[serde(default)]
    trim: Option<String>,
    #[serde(default = "no_mark")]
    mark: IconMark,
    #[serde(default)]
    glow: bool,
}

fn no_mark() -> IconMark {
    IconMark::None
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHouse {
    style: HouseStyle,
    #[serde(default = "one")]
    storeys: u8,
    roof: Roofing,
    wall: Walling,
    #[serde(default)]
    dormers: bool,
    #[serde(default)]
    porch: bool,
    #[serde(default)]
    lean_to: bool,
    #[serde(default)]
    boarded: bool,
    #[serde(default)]
    silhouette: bool,
    rise: u8,
    #[serde(default = "wood_dark")]
    door: String,
    #[serde(default = "wood_dark")]
    trim: String,
    #[serde(default)]
    lit: bool,
}

fn wood_dark() -> String {
    "wood_dark".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCreature {
    plan: Plan,
    anatomy: Anatomy,
    ramps: RawCreatureRamps,
    #[serde(default)]
    features: RawFeatures,
    #[serde(default)]
    emits: Vec<EmitRole>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCreatureRamps {
    body: String,
    #[serde(default)]
    belly: Option<String>,
    #[serde(default)]
    mark: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFeatures {
    #[serde(default = "no_ears")]
    ears: Ears,
    #[serde(default = "no_tail")]
    tail: Tail,
    #[serde(default)]
    markings: Vec<Marking>,
    #[serde(default)]
    collar: Option<String>,
}

impl Default for RawFeatures {
    fn default() -> Self {
        RawFeatures { ears: Ears::None, tail: Tail::None, markings: Vec::new(), collar: None }
    }
}

fn no_ears() -> Ears {
    Ears::None
}
fn no_tail() -> Tail {
    Tail::None
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProp {
    shape: String,
    rise: u8,
    materials: RawMaterials,
    #[serde(default = "base_only")]
    states: Vec<PropState>,
    #[serde(default = "one")]
    vary: u8,
    #[serde(default = "floor")]
    mount: Mount,
    #[serde(default)]
    text_rows: u8,
    #[serde(default)]
    emits: Vec<EmitRole>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMaterials {
    body: String,
    #[serde(default)]
    trim: Option<String>,
    #[serde(default)]
    accent: Option<String>,
}

fn base_only() -> Vec<PropState> {
    vec![PropState::Base]
}
fn one() -> u8 {
    1
}
fn floor() -> Mount {
    Mount::Floor
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPerson {
    build: Build,
    head: RawHead,
    body: RawBody,
    #[serde(default)]
    held: Option<RawHeld>,
    #[serde(default)]
    extras: Vec<Extra>,
    #[serde(default)]
    emits: Vec<EmitRole>,
    #[serde(default)]
    ghost: bool,
    #[serde(default)]
    vary: RawVary,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHead {
    hair: Hair,
    hair_ramp: String,
    #[serde(default = "no_hat")]
    hat: Hat,
    #[serde(default)]
    hat_ramp: Option<String>,
    #[serde(default = "skin")]
    skin: Skin,
    #[serde(default = "plain")]
    face: Face,
}

fn no_hat() -> Hat {
    Hat::None
}
fn skin() -> Skin {
    Skin::Skin
}
fn plain() -> Face {
    Face::Plain
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBody {
    coat: Coat,
    coat_ramp: String,
    #[serde(default = "no_front")]
    front: Front,
    #[serde(default)]
    front_ramp: Option<String>,
    legs: Legs,
    legs_ramp: String,
    boots: Boots,
    #[serde(default)]
    boots_ramp: Option<String>,
    #[serde(default)]
    pack: bool,
    #[serde(default)]
    roll: bool,
}

fn no_front() -> Front {
    Front::None
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeld {
    item: HeldItem,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVary {
    #[serde(default)]
    hair: Vec<Hair>,
    #[serde(default)]
    hair_ramp: Vec<String>,
    #[serde(default)]
    hat: Vec<Hat>,
    #[serde(default)]
    face: Vec<Face>,
    #[serde(default)]
    coat_ramp: Vec<String>,
    #[serde(default)]
    front_ramp: Vec<String>,
    #[serde(default)]
    legs_ramp: Vec<String>,
}

/// The most variants a row may have (ART.md §3).
pub const MAX_VARIANTS: usize = 4;

fn strs(v: &[String]) -> &'static [&'static str] {
    leak(v.iter().map(|s| leak_str(s)).collect())
}

fn unique<T: PartialEq>(v: &[T]) -> bool {
    v.iter().enumerate().all(|(i, a)| !v[..i].contains(a))
}

/// Looks drawn ahead of the rows that will name them (2026-10-02, the fire she makes: the art
/// branch landed before the sim's rows). Empty this once those rows are in.
const AWAITING: [&str; 0] = [];

pub fn compile(src: &Source, cx: &mut Ctx) -> &'static [(SpriteId, Look)] {
    let mut out = Vec::new();
    // The terrain's looks are keyed by tile, not sprite: `tile_looks.rs` compiles them, and a
    // tile may share a name with a sprite (`rubble`) without the two colliding.
    let sprite_looks = |f: &str| f != "looks/tiles.json" && !f.starts_with("looks/tiles/");
    for (key, row) in src.table_of("looks", sprite_looks, &mut cx.diag) {
        let at = format!("looks.{key}");
        let sprite = match cx.sprites.get(&key) {
            Some(s) => s,
            // Drawn ahead of the rows that will name them (a branch in flight): interned here,
            // after every row, so no other sprite's id moves. Each is a warning until its row
            // lands; then drop it from `AWAITING`.
            None if AWAITING.contains(&key.as_str()) => {
                cx.diag.warn(format!("{}: {at}", row.file), "a look awaiting the row that will name it");
                cx.sprites.intern(&key)
            }
            None => {
                cx.diag.error(format!("{}: {at}", row.file), "no row names this sprite: a look no row uses");
                continue;
            }
        };
        let Some(raw) = typed::<RawLook>(&row, &at, &mut cx.diag) else { continue };
        let look = match raw {
            RawLook::Person(p) => person(*p, &at, cx),
            RawLook::Creature(c) => creature(&c),
            RawLook::Container(p) => prop(PropFamily::Container, &p, &at, cx),
            RawLook::Furniture(p) => prop(PropFamily::Furniture, &p, &at, cx),
            RawLook::Sign(p) => prop(PropFamily::Sign, &p, &at, cx),
            RawLook::Lamp(p) => prop(PropFamily::Lamp, &p, &at, cx),
            RawLook::Machine(p) => prop(PropFamily::Machine, &p, &at, cx),
            RawLook::Barrier(p) => prop(PropFamily::Barrier, &p, &at, cx),
            RawLook::Vegetation(p) => prop(PropFamily::Vegetation, &p, &at, cx),
            RawLook::Debris(p) => prop(PropFamily::Debris, &p, &at, cx),
            RawLook::SmallThing(p) => prop(PropFamily::SmallThing, &p, &at, cx),
            RawLook::Ritual(p) => prop(PropFamily::Ritual, &p, &at, cx),
            RawLook::Structure(p) => prop(PropFamily::Structure, &p, &at, cx),
            RawLook::Icon(i) => Look::Icon(IconLook {
                class: i.class,
                ramp: leak_str(&i.ramp),
                trim: opt(i.trim.as_deref()),
                mark: i.mark,
                glow: i.glow,
            }),
            RawLook::Building(h) => {
                cx.diag.need((1..=3).contains(&h.storeys), &at, "a building has one to three storeys");
                Look::Building(HouseLook {
                    style: h.style,
                    storeys: h.storeys,
                    roof: h.roof,
                    wall: h.wall,
                    dormers: h.dormers,
                    porch: h.porch,
                    lean_to: h.lean_to,
                    boarded: h.boarded,
                    silhouette: h.silhouette,
                    rise: h.rise,
                    door: leak_str(&h.door),
                    trim: leak_str(&h.trim),
                    lit: h.lit,
                })
            }
        };
        out.push((SpriteId(sprite as u16), look));
    }
    leak(out)
}

fn opt(s: Option<&str>) -> Option<&'static str> {
    s.map(leak_str)
}

fn creature(c: &RawCreature) -> Look {
    Look::Creature(CreatureLook {
        plan: c.plan,
        anatomy: c.anatomy,
        ramps: CreatureRamps {
            body: leak_str(&c.ramps.body),
            belly: opt(c.ramps.belly.as_deref()),
            mark: opt(c.ramps.mark.as_deref()),
        },
        ears: c.features.ears,
        tail: c.features.tail,
        markings: leak(c.features.markings.clone()),
        collar: opt(c.features.collar.as_deref()),
        emits: leak(c.emits.clone()),
    })
}

fn prop(family: PropFamily, p: &RawProp, at: &str, cx: &mut Ctx) -> Look {
    cx.diag.need((1..=3).contains(&p.vary), at, "vary is 1 to 3 renders (base, base_2, base_3)");
    cx.diag.need(p.states.contains(&PropState::Base), at, "a prop always draws its base state");
    cx.diag.need(unique(&p.states), at, "a state is listed twice");
    Look::Prop(PropLook {
        family,
        shape: leak_str(&p.shape),
        rise: p.rise,
        materials: PropMaterials {
            body: leak_str(&p.materials.body),
            trim: opt(p.materials.trim.as_deref()),
            accent: opt(p.materials.accent.as_deref()),
        },
        states: leak(p.states.clone()),
        vary: p.vary,
        mount: p.mount,
        text_rows: p.text_rows,
        emits: leak(p.emits.clone()),
    })
}

fn person(p: RawPerson, at: &str, cx: &mut Ctx) -> Look {
    {
        let v = &p.vary;
        let lists_unique = unique(&v.hair)
            && unique(&v.hair_ramp)
            && unique(&v.hat)
            && unique(&v.face)
            && unique(&v.coat_ramp)
            && unique(&v.front_ramp)
            && unique(&v.legs_ramp);
        cx.diag.need(lists_unique, at, "a vary list repeats a value");
        let held = p.held.map_or(HeldItem::None, |h| h.item);
        cx.diag.need(
            !p.emits.contains(&EmitRole::Held) || held != HeldItem::None,
            at,
            "emits \"held\" with nothing in the hand",
        );
        cx.diag.need(
            !p.emits.contains(&EmitRole::Glass) || p.head.face == Face::Glasses || p.head.hat == Hat::Diving,
            at,
            "emits \"glass\" with no glasses and no diver's port",
        );
        let look = PersonLook {
            build: p.build,
            head: PersonHead {
                hair: p.head.hair,
                hair_ramp: leak_str(&p.head.hair_ramp),
                hat: p.head.hat,
                hat_ramp: p.head.hat_ramp.as_deref().map(leak_str),
                skin: p.head.skin,
                face: p.head.face,
            },
            body: PersonBody {
                coat: p.body.coat,
                coat_ramp: leak_str(&p.body.coat_ramp),
                front: p.body.front,
                front_ramp: p.body.front_ramp.as_deref().map(leak_str),
                legs: p.body.legs,
                legs_ramp: leak_str(&p.body.legs_ramp),
                boots: p.body.boots,
                boots_ramp: p.body.boots_ramp.as_deref().map(leak_str),
                pack: p.body.pack,
                roll: p.body.roll,
            },
            held,
            extras: leak(p.extras.clone()),
            emits: leak(p.emits.clone()),
            ghost: p.ghost,
            vary: PersonVary {
                hair: leak(v.hair.clone()),
                hair_ramp: strs(&v.hair_ramp),
                hat: leak(v.hat.clone()),
                face: leak(v.face.clone()),
                coat_ramp: strs(&v.coat_ramp),
                front_ramp: strs(&v.front_ramp),
                legs_ramp: strs(&v.legs_ramp),
            },
        };
        let n = look.vary.count();
        cx.diag.need(n <= MAX_VARIANTS, at, format!("vary makes {n} variants; at most {MAX_VARIANTS}"));
        Look::Person(look)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::build_source;

    fn build(looks: &str) -> crate::compile::Built {
        let src = Source::from_files(&[("looks/persons.json", looks)]).unwrap();
        build_source(&src)
    }

    #[test]
    fn a_look_no_row_uses_is_an_error() {
        let b = build(
            r#"{"nobody": {"family": "person", "build": "slim",
                "head": {"hair": "short", "hair_ramp": "hair_dark"},
                "body": {"coat": "coat", "coat_ramp": "cloth_grey", "legs": "trousers", "legs_ramp": "cloth_brown", "boots": "boots"}}}"#,
        );
        assert!(b.diag.errors.iter().any(|e| e.msg.contains("a look no row uses")), "{}", b.diag);
    }

    fn parse(json: &str) -> Result<(), String> {
        let row = crate::compile::source::Row {
            file: "looks/persons.json".into(),
            value: serde_json::from_str(json).unwrap(),
        };
        let mut d = crate::compile::diag::Diagnostics::default();
        typed::<RawLook>(&row, "looks.x", &mut d).map(|_| ()).ok_or_else(|| d.to_string())
    }

    #[test]
    fn unknown_fields_and_values_are_errors() {
        let good = r#"{"family": "person", "build": "slim",
            "head": {"hair": "short", "hair_ramp": "hair_dark"},
            "body": {"coat": "coat", "coat_ramp": "cloth_grey", "legs": "trousers", "legs_ramp": "cloth_brown", "boots": "boots"}}"#;
        parse(good).unwrap();
        let e = parse(&good.replace("\"slim\"", "\"lanky\"")).unwrap_err();
        assert!(e.contains("unknown variant"), "{e}");
        let e = parse(&good.replace("\"build\"", "\"colour\": 1, \"build\"")).unwrap_err();
        assert!(e.contains("unknown field"), "{e}");
    }

    #[test]
    fn variants_multiply_in_field_order() {
        let v = PersonVary { hair: &[Hair::Long, Hair::Bun], coat_ramp: &["a", "b"], ..PersonVary::NONE };
        assert_eq!(v.count(), 4);
        let look = PersonLook {
            build: Build::Slim,
            head: PersonHead {
                hair: Hair::Short,
                hair_ramp: "h",
                hat: Hat::None,
                hat_ramp: None,
                skin: Skin::Skin,
                face: Face::Plain,
            },
            body: PersonBody {
                coat: Coat::Coat,
                coat_ramp: "c",
                front: Front::None,
                front_ramp: None,
                legs: Legs::Trousers,
                legs_ramp: "l",
                boots: Boots::Boots,
                boots_ramp: None,
                pack: false,
                roll: false,
            },
            held: HeldItem::None,
            extras: &[],
            emits: &[],
            ghost: false,
            vary: v,
        };
        let got: Vec<(Hair, &str)> = (0..4).map(|k| look.variant(k)).map(|l| (l.head.hair, l.body.coat_ramp)).collect();
        assert_eq!(got, [(Hair::Long, "a"), (Hair::Bun, "a"), (Hair::Long, "b"), (Hair::Bun, "b")]);
        assert_eq!(look.variant(1).vary, PersonVary::NONE);
    }
}
