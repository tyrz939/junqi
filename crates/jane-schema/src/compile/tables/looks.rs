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
    Boots, Build, Coat, EmitRole, Extra, Face, Front, Hair, Hat, HeldItem, Legs, Look, PersonBody, PersonHead,
    PersonLook, PersonVary, Skin,
};
use jane_core::ids::SpriteId;

#[derive(Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
enum RawLook {
    Person(RawPerson),
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

pub fn compile(src: &Source, cx: &mut Ctx) -> &'static [(SpriteId, Look)] {
    let mut out = Vec::new();
    for (key, row) in src.table("looks", &mut cx.diag) {
        let at = format!("looks.{key}");
        let Some(sprite) = cx.sprites.get(&key) else {
            cx.diag.error(format!("{}: {at}", row.file), "no row names this sprite: a look no row uses");
            continue;
        };
        let Some(RawLook::Person(p)) = typed::<RawLook>(&row, &at, &mut cx.diag) else { continue };
        let v = &p.vary;
        let lists_unique = unique(&v.hair)
            && unique(&v.hair_ramp)
            && unique(&v.hat)
            && unique(&v.face)
            && unique(&v.coat_ramp)
            && unique(&v.front_ramp)
            && unique(&v.legs_ramp);
        cx.diag.need(lists_unique, &at, "a vary list repeats a value");
        let held = p.held.map_or(HeldItem::None, |h| h.item);
        cx.diag.need(
            !p.emits.contains(&EmitRole::Held) || held != HeldItem::None,
            &at,
            "emits \"held\" with nothing in the hand",
        );
        cx.diag.need(
            !p.emits.contains(&EmitRole::Glass) || p.head.face == Face::Glasses,
            &at,
            "emits \"glass\" with no glasses",
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
        cx.diag.need(n <= MAX_VARIANTS, &at, format!("vary makes {n} variants; at most {MAX_VARIANTS}"));
        out.push((SpriteId(sprite as u16), Look::Person(look)));
    }
    leak(out)
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
