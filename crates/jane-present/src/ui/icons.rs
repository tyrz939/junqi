//! Icons and the chrome's marks. An item, spell or status icon is `jane-art`'s icon look (ART.md
//! §2.5: drawn at 32, and its chip drawn again at 16, never a downscale) when the name has one,
//! which every row's does ([`look`]); the stand-ins below, drawn from the name (`item_key_brass`
//! a brass key, `spell_frost` a frost medallion), are what a name without a look falls back to.

use alloc::vec::Vec;
use jane_art::canvas::{Canvas, Dir, Z};
use jane_art::palette::{Ix, Ramp, Tone};
use jane_core::grid::Rect;

use crate::ui::art::Mark;

/// An icon's canvas side, px.
pub const ICON: i32 = 32;

fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect::new(x, y, w, h)
}

const Z0: Z = Z::flat(1);

/// A ramp named by the last word of an icon's name (`key_brass`, `potion_red`).
fn ramp_of(word: &str, or: Ramp) -> Ramp {
    match word {
        "red" => Ramp::ClothRed,
        "blue" => Ramp::ClothBlue,
        "cyan" => Ramp::Sky,
        "green" => Ramp::ClothGreen,
        "grey" => Ramp::ClothGrey,
        "orange" => Ramp::Copper,
        "purple" => Ramp::ClothPlum,
        "yellow" => Ramp::ClothMustard,
        "gold" | "brass" => Ramp::Brass,
        "iron" => Ramp::Iron,
        "brown" => Ramp::Leather,
        _ => or,
    }
}

/// The icon look `name` renders to, at 32 and as its 16 chip: `jane-art`'s icon family (the
/// bag's, the bar's and a thing lying on the ground's), or `None` for a name with no look.
pub fn look(name: &str) -> Option<(Canvas, Canvas)> {
    let sets = jane_art::looks::render(name).ok()?;
    let at = |v: u8| sets.iter().find(|r| r.variant == v).and_then(|r| r.set.frames.first()).map(|(_, c)| c.clone());
    Some((at(0)?, at(1)?))
}

/// The icon for `name` at 32 and at 16: its look, else the stand-in and the stand-in halved.
pub fn both(name: &str) -> (Canvas, Canvas) {
    look(name).unwrap_or_else(|| {
        let big = icon(name);
        let small = half(&big);
        (big, small)
    })
}

/// The stand-in icon for an item, spell or effect named `name`; a plain parcel for anything
/// unknown.
pub fn icon(name: &str) -> Canvas {
    let mut c = Canvas::flat(ICON, ICON);
    let (family, rest) = name.split_once('_').unwrap_or((name, ""));
    let last = rest.rsplit('_').next().unwrap_or("");
    match family {
        "item" => item(&mut c, rest, last),
        "spell" => spell(&mut c, rest),
        "fx" => effect(&mut c, rest),
        _ => parcel(&mut c, Ramp::ClothTweed),
    }
    c.outline();
    c
}

fn item(c: &mut Canvas, rest: &str, last: &str) {
    let head = rest.split('_').next().unwrap_or("");
    match head {
        "key" => key(c, ramp_of(last, Ramp::Iron)),
        "potion" => potion(c, ramp_of(last, Ramp::ClothRed)),
        "vial" => vial(c, last == "water"),
        "herb" => herb(
            c,
            match last {
                "rose" => Ramp::ClothRed,
                "pansy" => Ramp::ClothPlum,
                "lily" => Ramp::ClothLinen,
                "nasturtium" => Ramp::Copper,
                "cap" => Ramp::WoodPale,
                "moss" => Ramp::Leaf,
                "root" | "snakeroot" => Ramp::WoodDark,
                _ => Ramp::Bloom,
            },
        ),
        "letter" | "washing" => letter(c),
        "gold" if last == "bar" => gold_bar(c),
        "gold" => dust(c, Ramp::Brass),
        "coal" => lump(c, Ramp::ClothBlack),
        "rock" => lump(c, Ramp::Stone),
        "stone" => lump(c, Ramp::Slate),
        "iron" => lump(c, Ramp::Iron),
        "wood" => log(c),
        "meat" => meat(c, if last == "green" { Ramp::ClothGreen } else { Ramp::ClothRose }),
        "apple" => fruit(c, Ramp::ClothRed),
        "grape" => grapes(c),
        "glass" => pane(c),
        "fire" => gem(c, Ramp::Ember),
        "light" => gem(c, Ramp::GlassLit),
        "present" => present(c),
        "hat" | "cap" => hat(c, Ramp::ClothBrown),
        "glove" => glove(c, ramp_of(last, Ramp::Leather)),
        "coat" | "fleece" | "scarf" => garment(c, if head == "fleece" { Ramp::ClothCream } else { Ramp::ClothTweed }),
        "haversack" | "sack" => parcel(c, Ramp::Leather),
        "amulet" | "ring" => ring(c),
        "ball" => orb(c),
        "butterfly" => butterfly(c),
        "net" => net(c),
        "tin" | "oil" | "honey" => tin(c, if head == "honey" { Ramp::Brass } else { Ramp::Iron }),
        "plate" => plate(c),
        "spanner" | "spoons" | "scissors" | "billhook" => tool(c),
        "spectacles" => spectacles(c),
        "loaf" | "crepe" | "potatoes" => bread(c),
        "egg" => egg(c),
        "tortoise" => tortoise(c),
        _ => parcel(c, Ramp::ClothTweed),
    }
}

fn key(c: &mut Canvas, ramp: Ramp) {
    // A bow ring upper left, a shaft to the lower right, two teeth.
    c.ellipse_lit(r(4, 4, 13, 13), ramp, Z0);
    c.ellipse(r(8, 8, 5, 5), Ix::CLEAR, 0);
    for d in 0..3 {
        c.line((14 + d, 13), (25 + d, 24), ramp.at(if d == 0 { Tone::Light } else { Tone::Mid }), 1, 1);
    }
    c.line((14, 13), (25, 24), ramp.at(Tone::Light), 1, 1);
    c.fill_rect(r(20, 23, 3, 5), ramp.at(Tone::Base), 1);
    c.fill_rect(r(24, 19, 5, 3), ramp.at(Tone::Shade), 1);
    c.dot(7, 6, ramp.at(Tone::Glint), 1);
}

fn potion(c: &mut Canvas, liquid: Ramp) {
    c.soft_ellipse(r(6, 10, 20, 19), Ramp::Glass, Z0);
    c.soft_ellipse(r(7, 14, 18, 15), liquid, Z0);
    c.fill_rect(r(13, 5, 6, 7), Ramp::Glass.at(Tone::Light), 1);
    c.fill_rect(r(12, 3, 8, 4), Ramp::WoodOak.at(Tone::Base), 1);
    c.hline(12, 19, 3, Ramp::WoodOak.at(Tone::Light), 1);
    c.fill_rect(r(10, 13, 2, 3), Ramp::Glass.at(Tone::Glint), 1);
}

fn vial(c: &mut Canvas, water: bool) {
    c.fill_rect(r(12, 6, 8, 22), Ramp::Glass.at(Tone::Light), 1);
    c.gradient(r(12, 6, 8, 22), Ramp::Glass, Dir::Right, Tone::High, Tone::Mid, true);
    if water {
        c.fill_rect(r(13, 16, 6, 11), Ramp::Water.at(Tone::Light), 1);
        c.gradient(r(13, 16, 6, 11), Ramp::Water, Dir::Down, Tone::Light, Tone::Shade, true);
    }
    c.fill_rect(r(11, 3, 10, 4), Ramp::WoodOak.at(Tone::Base), 1);
    c.vline(14, 8, 25, Ramp::Glass.at(Tone::Glint), 1);
}

fn herb(c: &mut Canvas, flower: Ramp) {
    c.line((16, 29), (16, 12), Ramp::Leaf.at(Tone::Shade), 2, 1);
    c.ellipse_lit(r(5, 15, 11, 6), Ramp::Leaf, Z0);
    c.ellipse_lit(r(17, 18, 11, 6), Ramp::Leaf, Z0);
    c.ellipse_lit(r(9, 3, 14, 12), flower, Z0);
    c.disc_lit(16, 9, 2, Ramp::ClothMustard, Z0);
}

fn letter(c: &mut Canvas) {
    c.fill_rect(r(3, 8, 26, 18), Ramp::ClothLinen.at(Tone::Light), 1);
    c.gradient(r(3, 8, 26, 18), Ramp::ClothLinen, Dir::Down, Tone::High, Tone::Mid, true);
    c.line((3, 8), (16, 18), Ramp::ClothLinen.at(Tone::Shade), 1, 1);
    c.line((28, 8), (16, 18), Ramp::ClothLinen.at(Tone::Shade), 1, 1);
    c.disc_lit(16, 18, 3, Ramp::ClothRed, Z0);
}

fn gold_bar(c: &mut Canvas) {
    c.polygon_lit(&[(8, 10), (24, 10), (29, 23), (3, 23)], Ramp::Brass, 0, Z0);
    c.hline(9, 23, 11, Ramp::Brass.at(Tone::Glint), 1);
    c.line((24, 11), (28, 22), Ramp::Brass.at(Tone::Shade), 1, 1);
}

fn dust(c: &mut Canvas, ramp: Ramp) {
    c.ellipse_lit(r(4, 14, 24, 14), Ramp::Leather, Z0);
    c.ellipse_lit(r(8, 10, 16, 10), ramp, Z0);
    for (x, y) in [(11, 12), (17, 11), (20, 14), (14, 15)] {
        c.dot(x, y, ramp.at(Tone::Glint), 1);
    }
}

fn lump(c: &mut Canvas, ramp: Ramp) {
    c.polygon_lit(&[(6, 14), (12, 7), (22, 8), (28, 16), (25, 26), (9, 27), (4, 21)], ramp, 3, Z0);
    c.line((12, 16), (18, 14), ramp.at(Tone::Shade), 1, 1);
    c.dot(13, 10, ramp.at(Tone::High), 1);
}

fn log(c: &mut Canvas) {
    c.rect_round(r(3, 11, 24, 12), Ramp::Bark, 2, 4, Z0);
    c.ellipse_lit(r(21, 10, 9, 14), Ramp::WoodPale, Z0);
    c.ellipse(r(24, 15, 3, 4), Ramp::WoodPale.at(Tone::Shade), 1);
    c.hline(6, 19, 15, Ramp::Bark.at(Tone::Shade), 1);
    c.hline(8, 17, 19, Ramp::Bark.at(Tone::Shade), 1);
}

fn meat(c: &mut Canvas, ramp: Ramp) {
    c.line((20, 20), (27, 27), Ramp::Bone.at(Tone::Light), 3, 1);
    c.disc_lit(27, 27, 2, Ramp::Bone, Z0);
    c.ellipse_lit(r(3, 4, 21, 19), ramp, Z0);
    c.dot(9, 9, ramp.at(Tone::Glint), 1);
}

fn fruit(c: &mut Canvas, ramp: Ramp) {
    c.soft_ellipse(r(5, 8, 22, 21), ramp, Z0);
    c.line((16, 9), (17, 4), Ramp::Bark.at(Tone::Base), 2, 1);
    c.ellipse_lit(r(18, 3, 8, 5), Ramp::Leaf, Z0);
}

fn grapes(c: &mut Canvas) {
    for (x, y) in [(10, 9), (16, 9), (22, 9), (13, 15), (19, 15), (16, 21)] {
        c.disc_lit(x, y, 4, Ramp::ClothPlum, Z0);
    }
    c.line((16, 4), (18, 1), Ramp::Bark.at(Tone::Base), 2, 1);
}

fn pane(c: &mut Canvas) {
    c.polygon_lit(&[(6, 6), (24, 4), (27, 26), (8, 28)], Ramp::Glass, 0, Z0);
    c.line((10, 9), (14, 22), Ramp::Glass.at(Tone::Glint), 1, 1);
}

fn gem(c: &mut Canvas, ramp: Ramp) {
    c.polygon_lit(&[(16, 3), (27, 13), (16, 29), (5, 13)], ramp, 0, Z0);
    c.line((5, 13), (27, 13), ramp.at(Tone::Light), 1, 1);
    c.line((16, 3), (12, 13), ramp.at(Tone::High), 1, 1);
    c.dot(13, 8, ramp.at(Tone::Glint), 1);
}

fn present(c: &mut Canvas) {
    c.rect_bevel(r(5, 11, 22, 17), Ramp::ClothRed, 2, Z0);
    c.fill_rect(r(14, 11, 4, 17), Ramp::Brass.at(Tone::Light), 1);
    c.fill_rect(r(5, 17, 22, 3), Ramp::Brass.at(Tone::Base), 1);
    c.ellipse_lit(r(8, 5, 8, 7), Ramp::Brass, Z0);
    c.ellipse_lit(r(16, 5, 8, 7), Ramp::Brass, Z0);
}

fn hat(c: &mut Canvas, ramp: Ramp) {
    c.ellipse_lit(r(2, 18, 28, 9), ramp, Z0);
    c.rect_round(r(8, 7, 16, 15), ramp, 2, 4, Z0);
    c.fill_rect(r(8, 17, 16, 3), Ramp::ClothRed.at(Tone::Mid), 1);
}

fn glove(c: &mut Canvas, ramp: Ramp) {
    c.rect_round(r(9, 12, 14, 16), ramp, 2, 4, Z0);
    for i in 0..4 {
        c.rect_round(r(9 + i * 4, 4 + (i % 2), 4, 11), ramp, 1, 2, Z0);
    }
    c.rect_round(r(3, 14, 8, 5), ramp, 1, 2, Z0);
}

fn garment(c: &mut Canvas, ramp: Ramp) {
    c.polygon_cloth(&[(10, 4), (22, 4), (29, 12), (25, 15), (24, 29), (8, 29), (7, 15), (3, 12)], ramp, 0, Z0);
    c.line((16, 5), (16, 28), ramp.at(Tone::Shade), 1, 1);
}

fn parcel(c: &mut Canvas, ramp: Ramp) {
    c.rect_round(r(5, 9, 22, 19), ramp, 2, 5, Z0);
    c.ellipse_lit(r(10, 4, 12, 8), ramp, Z0);
    c.hline(8, 23, 14, ramp.at(Tone::Shade), 1);
    c.fill_rect(r(14, 14, 4, 4), Ramp::Brass.at(Tone::Light), 1);
}

fn ring(c: &mut Canvas) {
    c.ellipse_lit(r(6, 10, 20, 18), Ramp::Brass, Z0);
    c.ellipse(r(10, 14, 12, 10), Ix::CLEAR, 0);
    c.polygon_lit(&[(16, 3), (21, 8), (16, 13), (11, 8)], Ramp::ClothRed, 0, Z0);
    c.dot(15, 6, Ramp::ClothRed.at(Tone::Glint), 1);
}

fn orb(c: &mut Canvas) {
    c.soft_ellipse(r(4, 4, 24, 24), Ramp::Brass, Z0);
    c.dot(11, 10, Ramp::Brass.at(Tone::Glint), 1);
    c.dot(12, 10, Ramp::Brass.at(Tone::High), 1);
}

fn butterfly(c: &mut Canvas) {
    c.ellipse_lit(r(3, 6, 13, 12), Ramp::Bloom, Z0);
    c.ellipse_lit(r(16, 6, 13, 12), Ramp::Bloom, Z0);
    c.ellipse_lit(r(6, 17, 10, 9), Ramp::ClothMustard, Z0);
    c.ellipse_lit(r(16, 17, 10, 9), Ramp::ClothMustard, Z0);
    c.line((16, 6), (16, 26), Ramp::ClothBlack.at(Tone::Base), 2, 1);
}

fn net(c: &mut Canvas) {
    c.line((4, 28), (14, 16), Ramp::WoodOak.at(Tone::Base), 2, 1);
    c.ellipse_lit(r(12, 3, 17, 17), Ramp::Reed, Z0);
    c.ellipse(r(15, 6, 11, 11), Ix::CLEAR, 0);
    for i in 0..3 {
        c.line((15 + i * 4, 6), (15 + i * 4, 16), Ramp::Reed.at(Tone::Shade), 1, 1);
        c.line((15, 7 + i * 4), (25, 7 + i * 4), Ramp::Reed.at(Tone::Shade), 1, 1);
    }
}

fn tin(c: &mut Canvas, ramp: Ramp) {
    c.rect_round(r(8, 8, 16, 20), ramp, 2, 3, Z0);
    c.ellipse_lit(r(8, 5, 16, 6), ramp, Z0);
    c.fill_rect(r(8, 14, 16, 7), Ramp::ClothRed.at(Tone::Mid), 1);
}

fn plate(c: &mut Canvas) {
    c.ellipse_lit(r(3, 7, 26, 20), Ramp::Plaster, Z0);
    c.ellipse(r(9, 11, 14, 11), Ramp::Plaster.at(Tone::Light), 1);
}

fn tool(c: &mut Canvas) {
    c.line((6, 26), (22, 10), Ramp::Iron.at(Tone::Light), 3, 1);
    c.disc_lit(24, 8, 5, Ramp::Iron, Z0);
    c.ellipse(r(23, 4, 4, 5), Ix::CLEAR, 0);
    c.rect_round(r(3, 23, 7, 7), Ramp::WoodOak, 1, 2, Z0);
}

fn spectacles(c: &mut Canvas) {
    for x in [4, 18] {
        c.ellipse_lit(r(x, 10, 11, 11), Ramp::Brass, Z0);
        c.ellipse(r(x + 2, 12, 7, 7), Ramp::Glass.at(Tone::Light), 1);
    }
    c.hline(14, 18, 13, Ramp::Brass.at(Tone::Base), 1);
}

fn bread(c: &mut Canvas) {
    c.soft_ellipse(r(3, 9, 26, 18), Ramp::WoodPale, Z0);
    for x in [10, 16, 22] {
        c.line((x - 2, 12), (x + 1, 17), Ramp::WoodPale.at(Tone::Shade), 1, 1);
    }
}

fn egg(c: &mut Canvas) {
    c.soft_ellipse(r(8, 4, 16, 23), Ramp::ClothCream, Z0);
}

fn tortoise(c: &mut Canvas) {
    c.ellipse_lit(r(22, 14, 8, 7), Ramp::Grass, Z0);
    c.soft_ellipse(r(3, 8, 22, 18), Ramp::ClothGreen, Z0);
    c.line((8, 16), (20, 16), Ramp::ClothGreen.at(Tone::Shade), 1, 1);
    c.line((14, 10), (14, 23), Ramp::ClothGreen.at(Tone::Shade), 1, 1);
}

/// A spell: a dark medallion in its school's colour with the school's mark on it.
fn spell(c: &mut Canvas, what: &str) {
    let ramp = match what {
        "frost" => Ramp::Sky,
        "fire" => Ramp::Ember,
        "blast" => Ramp::Copper,
        "shock" => Ramp::GlassLit,
        "nature" | "grow" => Ramp::Leaf,
        "repair" => Ramp::Brass,
        "web" => Ramp::Bone,
        _ => Ramp::Iron,
    };
    c.ellipse_lit(r(1, 1, 30, 30), Ramp::UiSlot, Z0);
    c.ellipse(r(4, 4, 24, 24), ramp.at(Tone::Deep), 1);
    let (hi, mid) = (ramp.at(Tone::High), ramp.at(Tone::Base));
    match what {
        "frost" => flake(c, 16, 16, 9, hi, mid),
        "fire" | "blast" => flame(c, ramp),
        "shock" => bolt(c, 16, 16, hi),
        "nature" | "grow" => {
            c.line((11, 24), (16, 14), mid, 2, 1);
            c.ellipse_lit(r(9, 7, 11, 9), ramp, Z0);
            c.ellipse_lit(r(16, 11, 9, 7), ramp, Z0);
        }
        "repair" => {
            c.line((10, 23), (19, 14), Ramp::WoodOak.at(Tone::Light), 2, 1);
            c.rect_round(r(15, 7, 11, 7), Ramp::Iron, 1, 2, Z0);
        }
        "web" => {
            for (dx, dy) in [(0, -9), (8, -4), (8, 5), (0, 9), (-8, 5), (-8, -4)] {
                c.line((16, 16), (16 + dx, 16 + dy), hi, 1, 1);
            }
            c.polyline(&[(16, 11), (20, 14), (20, 18), (16, 21), (12, 18), (12, 14), (16, 11)], mid, 1, 1);
        }
        _ => {
            c.line((10, 22), (22, 10), Ramp::Iron.at(Tone::High), 2, 1);
            c.line((9, 17), (15, 23), Ramp::Brass.at(Tone::Light), 2, 1);
        }
    }
}

fn flake(c: &mut Canvas, x: i32, y: i32, n: i32, hi: Ix, mid: Ix) {
    c.line((x - n, y), (x + n, y), hi, 1, 1);
    c.line((x, y - n), (x, y + n), hi, 1, 1);
    let d = n * 7 / 10;
    c.line((x - d, y - d), (x + d, y + d), mid, 1, 1);
    c.line((x - d, y + d), (x + d, y - d), mid, 1, 1);
    c.dot(x, y, hi, 1);
}

fn flame(c: &mut Canvas, ramp: Ramp) {
    c.polygon_lit(&[(16, 5), (22, 14), (23, 20), (19, 26), (13, 26), (9, 20), (10, 14), (13, 12)], ramp, 3, Z0);
    c.polygon_lit(&[(16, 13), (19, 19), (17, 24), (14, 23), (13, 19)], Ramp::GlassLit, 2, Z0);
}

fn bolt(c: &mut Canvas, x: i32, y: i32, ink: Ix) {
    c.polyline_fill(
        &[(x + 2, y - 11), (x - 5, y + 1), (x, y + 1), (x - 3, y + 11), (x + 6, y - 2), (x + 1, y - 2)],
        ink,
        1,
    );
}

/// A status: a small glyph on a clear ground.
fn effect(c: &mut Canvas, what: &str) {
    match what {
        "burning" | "firelash" => flame(c, Ramp::Ember),
        "chilled" | "winterbite" => flake(c, 16, 16, 11, Ramp::Sky.at(Tone::High), Ramp::Sky.at(Tone::Base)),
        "sparktongue" | "stunned" => bolt(c, 16, 16, Ramp::GlassLit.at(Tone::High)),
        "lifesteal" => heart(c, Ramp::ClothRed),
        "manashield" => {
            c.polygon_lit(&[(5, 5), (27, 5), (27, 15), (16, 28), (5, 15)], Ramp::Water, 2, Z0);
        }
        "poisoned" => {
            c.polygon_lit(&[(16, 4), (24, 18), (20, 27), (12, 27), (8, 18)], Ramp::ClothGreen, 2, Z0);
        }
        "stoneskin" => lump(c, Ramp::Stone),
        "stranglethorn" | "webbed" => {
            c.line((5, 27), (27, 5), Ramp::Leaf.at(Tone::Base), 2, 1);
            for i in 0..4 {
                let p = 8 + i * 5;
                c.line((p, 32 - p), (p - 3, 32 - p - 4), Ramp::Leaf.at(Tone::Light), 1, 1);
            }
        }
        _ => star(c, Ramp::ClothMustard),
    }
}

fn heart(c: &mut Canvas, ramp: Ramp) {
    c.disc_lit(11, 12, 6, ramp, Z0);
    c.disc_lit(21, 12, 6, ramp, Z0);
    c.polyline_fill(&[(5, 14), (27, 14), (16, 27)], ramp.at(Tone::Base), 1);
    c.dot(9, 10, ramp.at(Tone::Glint), 1);
}

fn star(c: &mut Canvas, ramp: Ramp) {
    c.polygon_lit(
        &[(16, 3), (19, 12), (28, 12), (21, 18), (24, 27), (16, 21), (8, 27), (11, 18), (4, 12), (13, 12)],
        ramp,
        0,
        Z0,
    );
}

/// `c` at half size, each 2 x 2 block its most-inked texel (the outline kept), re-outlined.
pub fn half(c: &Canvas) -> Canvas {
    let (w, h) = (c.w() / 2, c.h() / 2);
    let mut o = Canvas::flat(w, h);
    for y in 0..h {
        for x in 0..w {
            let block = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| c.get(2 * x + dx, 2 * y + dy));
            let opaque: Vec<Ix> = block.iter().copied().filter(|i| i.is_opaque() && *i != Ix::INK).collect();
            if let Some(&i) = opaque.first() {
                // The lightest of the block's colours, so a glint survives the halving.
                let pick = opaque.iter().copied().max_by_key(|&i| jane_art::palette::luma(i)).unwrap_or(i);
                o.dot(x, y, if opaque.len() >= 2 { pick } else { i }, 1);
            }
        }
    }
    o.outline();
    o
}

/// A mark (16 px; the diamond 7): drawn in its own colours.
pub fn mark(m: Mark) -> Canvas {
    let mut c = Canvas::flat(16, 16);
    let cream = Ramp::UiInk.at(Tone::High);
    match m {
        Mark::Arrow => {
            c.polyline_fill(&[(1, 1), (1, 13), (4, 10), (7, 15), (9, 14), (7, 9), (11, 9)], cream, 1);
            c.line((2, 3), (2, 10), Ramp::UiInk.at(Tone::Glint), 1, 1);
        }
        Mark::Reticle => {
            let g = Ramp::UiGold.at(Tone::High);
            for (a, b) in [((7, 1), (7, 4)), ((7, 10), (7, 13)), ((1, 7), (4, 7)), ((10, 7), (13, 7))] {
                c.line(a, b, g, 2, 1);
            }
            c.dot(7, 7, g, 1);
        }
        Mark::Hand => {
            c.rect_round(r(3, 6, 10, 9), Ramp::SkinPale, 1, 3, Z0);
            for i in 0..3 {
                c.rect_round(r(3 + i * 3, 3, 4, 6), Ramp::SkinPale, 1, 2, Z0);
            }
        }
        Mark::Sun => {
            let s = Ramp::GlassLit;
            for (a, b) in [((7, 0), (7, 2)), ((7, 13), (7, 15)), ((0, 7), (2, 7)), ((13, 7), (15, 7))] {
                c.line(a, b, s.at(Tone::High), 1, 1);
            }
            for (a, b) in [((2, 2), (3, 3)), ((12, 2), (11, 3)), ((2, 12), (3, 11)), ((12, 12), (11, 11))] {
                c.line(a, b, s.at(Tone::Light), 1, 1);
            }
            c.soft_ellipse(r(4, 4, 8, 8), s, Z0);
        }
        Mark::Moon => {
            c.soft_ellipse(r(2, 2, 12, 12), Ramp::Bone, Z0);
            c.ellipse(r(6, 0, 11, 11), Ix::CLEAR, 0);
        }
        Mark::PadA | Mark::PadB | Mark::PadX | Mark::PadY => {
            let ramp = match m {
                Mark::PadA => Ramp::ClothGreen,
                Mark::PadB => Ramp::ClothRed,
                Mark::PadX => Ramp::ClothBlue,
                _ => Ramp::ClothMustard,
            };
            c.ellipse_lit(r(1, 1, 14, 14), ramp, Z0);
        }
        Mark::PadShoulder => c.rect_round(r(0, 3, 16, 11), Ramp::Iron, 1, 3, Z0),
        Mark::PadStick => {
            c.ellipse_lit(r(1, 1, 14, 14), Ramp::Iron, Z0);
            c.ellipse(r(5, 5, 6, 6), Ramp::Iron.at(Tone::Shade), 1);
        }
        Mark::Diamond => {
            c.polyline_fill(&[(3, 0), (6, 3), (3, 6), (0, 3)], Ramp::UiGold.at(Tone::Light), 1);
            c.dot(3, 2, Ramp::UiGold.at(Tone::Glint), 1);
        }
        Mark::Skull => {
            c.soft_ellipse(r(2, 1, 12, 11), Ramp::Bone, Z0);
            c.fill_rect(r(5, 10, 6, 4), Ramp::Bone.at(Tone::Base), 1);
            c.ellipse(r(4, 5, 3, 3), Ix::INK, 1);
            c.ellipse(r(9, 5, 3, 3), Ix::INK, 1);
        }
        Mark::Lock => {
            c.ellipse_lit(r(4, 1, 8, 9), Ramp::Iron, Z0);
            c.ellipse(r(6, 3, 4, 5), Ix::CLEAR, 0);
            c.rect_bevel(r(3, 7, 10, 8), Ramp::Brass, 1, Z0);
        }
        Mark::Heart => {
            c.disc_lit(5, 6, 3, Ramp::ClothRed, Z0);
            c.disc_lit(10, 6, 3, Ramp::ClothRed, Z0);
            c.polyline_fill(&[(2, 7), (13, 7), (8, 13)], Ramp::ClothRed.at(Tone::Base), 1);
        }
        Mark::Flake => flake(&mut c, 7, 7, 6, Ramp::Sky.at(Tone::High), Ramp::Sky.at(Tone::Base)),
        Mark::Bolt => {
            c.polyline_fill(&[(9, 1), (3, 8), (7, 8), (5, 14), (12, 6), (8, 6)], Ramp::GlassLit.at(Tone::High), 1);
        }
        Mark::Star => c.polyline_fill(
            &[(7, 1), (9, 6), (14, 6), (10, 9), (12, 14), (7, 11), (2, 14), (4, 9), (0, 6), (5, 6)],
            Ramp::UiGold.at(Tone::Light),
            1,
        ),
    }
    c.outline();
    if m == Mark::Diamond {
        let mut d = Canvas::flat(7, 7);
        for y in 0..7 {
            for x in 0..7 {
                let i = c.get(x, y);
                if i.is_opaque() {
                    d.dot(x, y, i, 1);
                }
            }
        }
        return d;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_a_row_names_draws_and_is_closed() {
        let cat = jane_data::catalog();
        let names = cat.combat.items.iter().map(|i| i.icon).chain(cat.combat.spells.iter().map(|s| s.icon));
        for id in names {
            let name = cat.sprites[usize::from(id.0)];
            let c = icon(name);
            let inked = c.albedo().iter().filter(|i| i.is_opaque()).count();
            assert!(inked > 60, "{name} draws {inked} px");
            assert!(matches!(c.get(0, 0), Ix::CLEAR | Ix::INK), "{name}: the corner is clear or outline");
        }
        for m in Mark::ALL {
            assert!(mark(m).albedo().iter().any(|i| i.is_opaque()), "{m:?}");
        }
    }

    #[test]
    fn a_half_icon_keeps_its_outline_and_most_of_its_ink() {
        let c = icon("item_key_brass");
        let h = half(&c);
        assert_eq!((h.w(), h.h()), (16, 16));
        let inked = h.albedo().iter().filter(|i| i.is_opaque()).count();
        assert!(inked > 30);
    }
}
