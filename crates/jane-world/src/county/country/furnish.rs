//! What each kind of place holds (`furnish` and the builders under it in `country.ts`). Every
//! draw is the place's own dice ([`super::places::stamp`] throws them for its kind and where it
//! stands). A place names its props (`own`), keeps open cells for a story (`keep`, `keep_board`),
//! and records its people and what bites in it, all on the [`super::Place`] being stamped.
//!
//! The coordinates below are each place's layout, relative to its footprint's corner or centre,
//! as the TypeScript drew it.

use jane_core::num::Permille;
use jane_core::{DialogueId, PropDefId, Rect, Sfc32, Stack, Tile, UnitDefId};
use jane_data::{Region, RuinKind};

use super::defs::{Defs, defs};
use super::places::Kind;
use super::{County, Side, folk, ground, hostile, keep, lane, own, pad, pen, put, put_with, road_side, run};
use crate::skeleton::Biome;

/// A coin toss at `p` permille.
fn chance(rng: &mut Sfc32, p: i16) -> bool {
    rng.chance(Permille(p))
}

/// One of a short list, uniformly.
fn pick<T: Copy>(rng: &mut Sfc32, items: &[T]) -> T {
    *rng.pick(items).expect("a list of at least one")
}

/// A prop that answers with a dialogue tree.
fn talk(c: &mut County<'_>, def: PropDefId, x: i32, y: i32, tree: DialogueId) -> Option<jane_core::Key> {
    put_with(c, def, x, y, |p| p.talk = Some(tree))
}

/// A prop that holds something.
fn holding(c: &mut County<'_>, def: PropDefId, x: i32, y: i32, loot: Vec<Stack>) -> Option<jane_core::Key> {
    put_with(c, def, x, y, |p| p.loot = loot)
}

/// What a chest out here holds: something of its region's, one to three of it.
fn loot(c: &County<'_>, rng: &mut Sfc32, x: i32, y: i32) -> Vec<Stack> {
    let [low, waters, works] = jane_data::catalog().county.furnishing.chests;
    let items = match ground(c.sk, x, y).region {
        Region::Lowfields => low,
        Region::Waters => waters,
        Region::Works => works,
    };
    let item = pick(rng, items);
    vec![Stack { item, qty: 1 + rng.irandom(2) as u16 }]
}

/// The place of `kind` centred on `(x, y)` over `b`, furnished.
pub fn furnish(c: &mut County<'_>, rng: &mut Sfc32, kind: Kind, x: i32, y: i32, b: Rect) {
    let d = defs();
    let (x0, y0) = (b.x, b.y);
    match kind {
        Kind::Hamlet => hamlet(c, rng, d, x0, y0),
        Kind::Farmstead => farmstead(c, rng, d, x0, y0),
        Kind::Cottage => cottage(c, rng, d, x0, y0),
        Kind::Inn => inn(c, rng, d, x0, y0),
        Kind::Orchard => orchard(c, rng, d, x0, y0),
        Kind::Field => field(c, rng, d, b),
        Kind::Shrine => {
            pad(c, rng, x, y + 1, 8, 6, Tile::Cobble);
            talk(c, d.p.wayside_shrine, x - 1, y - 2, d.t.country_shrine);
            for _ in 0..4 {
                let fx = x - 4 + rng.irandom(9);
                let fy = y + rng.irandom(3);
                put(c, d.p.flowers, fx, fy);
            }
        }
        Kind::Well => {
            pad(c, rng, x, y, 10, 8, Tile::Cobble);
            talk(c, d.p.well, x - 1, y - 2, d.t.country_well);
            put(c, d.p.trough, x + 2, y + 1);
            put(c, d.p.log, x - 5, y + 1);
        }
        Kind::Wreck => {
            pad(c, rng, x, y, 10, 6, Tile::Dirt);
            put(c, d.p.cart_wreck, x - 2, y - 1);
            let box_ = if chance(rng, 500) { d.p.crate_ } else { d.p.barrel };
            put(c, box_, x + 2, y + 1);
            put(c, d.p.bones, x - 3, y + 2);
            if chance(rng, 350) {
                let l = loot(c, rng, x, y);
                holding(c, d.p.chest, x - 5, y - 2, l);
            }
        }
        Kind::Hay => {
            pad(c, rng, x, y, 14, 10, Tile::Dirt);
            put(c, d.p.haystack, x - 5, y - 3);
            put(c, d.p.haystack, x - 1, y - 4);
            put(c, d.p.hay_cart, x + 3, y);
            if chance(rng, 600) {
                put(c, d.p.haystack, x - 3, y + 1);
            }
        }
        Kind::Meadow => {
            for _ in 0..9 {
                let fx = x - 6 + rng.irandom(13);
                let fy = y - 5 + rng.irandom(11);
                put(c, d.p.flowers, fx, fy);
            }
            if chance(rng, 600) {
                talk(c, d.p.beehive, x - 2, y - 1, d.t.country_hive);
                put(c, d.p.beehive, x + 1, y - 1);
            }
        }
        Kind::Herd => herd(c, rng, d, x, y),
        Kind::Woodcutter => woodcutter(c, rng, d, b),
        Kind::Pond => pond(c, rng, d, x, y),
        Kind::Stones => stones(c, rng, d, x, y),
        Kind::Camp => camp(c, rng, d, x, y),
        Kind::Den => den(c, rng, d, x, y),
        Kind::Ruin => ruin(c, rng, d, x0, y0),
        Kind::Outcrop => outcrop(c, rng, d, x, y),
        Kind::Reedhut => reedhut(c, rng, d, x0, y0),
        Kind::Glass => glasshouse(c, rng, d, x0, y0),
        Kind::Slag => slag(c, rng, d, x, y),
        Kind::Graves => graves(c, rng, d, x0, y0),
    }
}

/// The board a place's name will be painted on (the stories stage puts it up): two cells on the
/// edge of the place that faces the road, as near the middle of that edge as the place left room,
/// with open ground below it. Kept as the `board` slot.
fn keep_board(c: &mut County<'_>, b: Rect) {
    if c.country.cur.is_none() {
        return;
    }
    let (x0, y0, w, h) = (b.x, b.y, b.w, b.h);
    let side = road_side(c, x0 + (w >> 1), y0 + (h >> 1));
    let across = matches!(side, Side::N | Side::S);
    let along = if across { w } else { h };
    let mut d = 0;
    while 2 * d < along - 4 {
        let offsets: &[i32] = if d == 0 { &[0] } else { &[d, -d] };
        for &s in offsets {
            let x = match side {
                Side::N | Side::S => x0 + (w >> 1) - 1 + s,
                Side::E => x0 + w - 3,
                Side::W => x0 + 1,
            };
            let y = match side {
                Side::E | Side::W => y0 + (h >> 1) + s,
                Side::S => y0 + h - 2,
                Side::N => y0 + 1,
            };
            for inset in 0..3 {
                let ix = match side {
                    Side::E => x - inset,
                    Side::W => x + inset,
                    _ => x,
                };
                let iy = match side {
                    Side::S => y - inset,
                    Side::N => y + inset,
                    _ => y,
                };
                if !c.k.fits(ix, iy, 2, 1, 0) || c.k.solid(ix, iy + 1) || c.k.is_claimed(ix, iy + 1) {
                    continue;
                }
                c.k.claim(Rect::new(ix, iy, 2, 1));
                if let Some(cur) = c.country.cur.as_mut() {
                    cur.slots.push(("board", (ix, iy)));
                }
                return;
            }
        }
        d += 1;
    }
}

/// A house's name on its place: the first `house`, then `house_2`, `house_3`.
const HOUSES: [&str; 3] = ["house", "house_2", "house_3"];

/// Two or three cottages round a green with a well, a hen house, a garden, and people.
fn hamlet(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    // The north row: up to three houses, doors on the green.
    let n = 2 + rng.irandom(2);
    let slots: &[i32] = if n == 2 { &[3, 21] } else { &[0, 12, 24] };
    let first = rng.irandom(4) as usize;
    let cottages = d.cottages();
    for (i, &sx) in slots.iter().enumerate() {
        let def = cottages[(first + i) % 4];
        let w = if def == d.p.cottage_tile { 9 } else { 8 };
        let hx = x0 + sx - i32::from(i == slots.len() - 1 && w == 9);
        let door = pick(rng, &d.doors());
        let house = talk(c, def, hx, y0 + 1, door);
        own(c, HOUSES[i], house);
        let bed = if chance(rng, 500) { 0 } else { w - 2 };
        put(c, d.p.flowerbed, hx + bed, y0 + 7);
        if chance(rng, 500) {
            for f in 0..3 {
                c.k.set(hx + 3 + f, y0 + 8, Tile::FlowerBed);
            }
        }
    }
    // The green: worn ground, and the well in the middle of it.
    let (gx, gy) = (x0 + 18, y0 + 15);
    pad(c, rng, gx, gy, 22, 10, Tile::Dirt);
    pad(c, rng, gx, gy, 8, 5, Tile::Cobble);
    let well = talk(c, d.p.well, gx - 1, gy - 1, d.t.country_well);
    own(c, "well", well);
    put(c, d.p.trough, gx + 3, gy + 1);
    put(c, d.p.log, gx - 7, gy - 1);
    // South of the green: a hen house and its hens, a vegetable plot, a line of washing.
    let coop = talk(c, d.p.hen_coop, x0 + 4, y0 + 22, d.t.country_coop);
    own(c, "coop", coop);
    for _ in 0..3 {
        let hx = x0 + 3 + rng.irandom(6);
        let hy = y0 + 24 + rng.irandom(3);
        folk(c, rng, d.u.hen, hx, hy, 3);
    }
    let (plot_x, plot_y) = (x0 + 21, y0 + 21);
    for j in 0..6 {
        for i in 0..12 {
            c.k.set(plot_x + i, plot_y + j, if j % 2 == 0 { Tile::Crops } else { Tile::Dirt });
        }
    }
    for i in (1..12).step_by(4) {
        put(c, d.p.crop, plot_x + i, plot_y + 2);
    }
    run(c, plot_x - 1, plot_y + 6, 14, true, Tile::Fence, 0);
    let washing = put(c, d.p.washing_line, x0 + 11, y0 + 24);
    own(c, "washing", washing);
    if chance(rng, 500) {
        let pile = put(c, d.p.woodpile, x0 + 13, y0 + 20);
        own(c, "woodpile", pile);
    }
    // People: two or three, about the green.
    let m = 2 + rng.irandom(2);
    for _ in 0..m {
        let who = pick(rng, &d.people());
        let px = gx - 8 + rng.irandom(16);
        let py = gy - 3 + rng.irandom(6);
        folk(c, rng, who, px, py, 5);
    }
    let (w, h) = Kind::Hamlet.size();
    keep_board(c, Rect::new(x0, y0, w, h));
    keep(c, "green", gx + 4, gy - 3);
    keep(c, "plot", plot_x + 6, plot_y + 3);
    keep(c, "yard", x0 + 8, y0 + 18);
    c.k.claim(Rect::new(x0, y0, w, h));
    // The lane leaves by the side the road is on: between two houses if that is north.
    match road_side(c, gx, gy) {
        Side::N => lane(c, x0 + if n == 2 { 15 } else { 10 }, y0 + 4, 2),
        Side::E => lane(c, x0 + 34, gy, 2),
        Side::W => lane(c, x0 + 1, gy, 2),
        Side::S => lane(c, gx, gy + 5, 2),
    }
}

/// A farmhouse and a barn across a yard, a fenced field in crops, a pen of sheep, a cart.
fn farmstead(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    let door = pick(rng, &d.doors());
    let house = talk(c, d.p.farmhouse, x0 + 2, y0 + 1, door);
    own(c, "house", house);
    let barn = talk(c, d.p.barn, x0 + 27, y0 + 1, d.t.country_barn);
    own(c, "barn", barn);
    pad(c, rng, x0 + 20, y0 + 11, 24, 7, Tile::Dirt);
    let cart = put(c, d.p.hay_cart, x0 + 23, y0 + 9);
    own(c, "cart", cart);
    let stack = put(c, d.p.haystack, x0 + 15, y0 + 2);
    own(c, "haystack", stack);
    let pump = talk(c, d.p.pump, x0 + 14, y0 + 8, d.t.country_pump);
    own(c, "pump", pump);
    let coop = talk(c, d.p.hen_coop, x0 + 3, y0 + 10, d.t.country_coop);
    own(c, "coop", coop);
    for _ in 0..3 {
        let hx = x0 + 2 + rng.irandom(6);
        let hy = y0 + 12 + rng.irandom(2);
        folk(c, rng, d.u.hen, hx, hy, 3);
    }
    // The field: rows of crops inside a fence, the gate on the yard.
    let (fx, fy) = (x0 + 1, y0 + 15);
    for j in 1..13 {
        for i in 1..23 {
            c.k.set(fx + i, fy + j, if j % 2 == 1 { Tile::Crops } else { Tile::Dirt });
        }
    }
    for j in (1..13).step_by(4) {
        for i in (3..22).step_by(6) {
            put(c, d.p.crop, fx + i, fy + j);
        }
    }
    pen(c, Rect::new(fx, fy, 24, 14), Side::N, Tile::Fence);
    if chance(rng, 600) {
        let crow = put(c, d.p.scarecrow, fx + 11, fy + 6);
        own(c, "scarecrow", crow);
    }
    // The sheep pen.
    let (px, py) = (x0 + 27, y0 + 17);
    pen(c, Rect::new(px, py, 12, 10), Side::W, Tile::Fence);
    for _ in 0..3 {
        let sx = px + 3 + rng.irandom(6);
        let sy = py + 3 + rng.irandom(4);
        folk(c, rng, d.u.sheep, sx, sy, 2);
    }
    folk(c, rng, d.u.folk_farmer, x0 + 20, y0 + 12, 6);
    if chance(rng, 700) {
        folk(c, rng, d.u.folk_wife, x0 + 8, y0 + 9, 4);
    }
    let (w, h) = Kind::Farmstead.size();
    keep_board(c, Rect::new(x0, y0, w, h));
    keep(c, "yard", x0 + 17, y0 + 12);
    keep(c, "field", fx + 6, fy + 8);
    keep(c, "pen", px + 7, py + 6);
    c.k.claim(Rect::new(x0, y0, w, h));
    // Out of the yard by the side the road is on, never through the field.
    match road_side(c, x0 + 20, y0 + 15) {
        Side::N => lane(c, x0 + 19, y0 + 8, 2),
        Side::S => lane(c, x0 + 25, y0 + 14, 2),
        Side::E => lane(c, x0 + 38, y0 + 12, 2),
        Side::W => lane(c, x0 + 1, y0 + 12, 2),
    }
}

/// One house, its garden, its washing, its woodpile, and whoever lives there.
fn cottage(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    let def = pick(rng, &d.cottages());
    let hx = x0 + 2;
    let door = pick(rng, &d.doors());
    let house = talk(c, def, hx, y0 + 1, door);
    own(c, "house", house);
    put(c, d.p.flowerbed, hx, y0 + 7);
    // The garden to the east: a fenced plot, rows and a gate.
    let (gx, gy) = (x0 + 12, y0 + 2);
    for j in 1..9 {
        for i in 1..9 {
            c.k.set(gx + i, gy + j, if j % 2 == 1 { Tile::Crops } else { Tile::Dirt });
        }
    }
    for j in (1..9).step_by(4) {
        let cx = gx + 2 + rng.irandom(5);
        put(c, d.p.crop, cx, gy + j);
    }
    pen(c, Rect::new(gx, gy, 10, 10), Side::S, Tile::Fence);
    let wood = if chance(rng, 500) { d.p.woodpile } else { d.p.beehive };
    let wood = put(c, wood, x0 + 1, y0 + 10);
    own(c, "wood", wood);
    if chance(rng, 600) {
        let washing = put(c, d.p.washing_line, x0 + 4, y0 + 12);
        own(c, "washing", washing);
    }
    let who = pick(rng, &d.people());
    folk(c, rng, who, hx + 4, y0 + 9, 4);
    let (w, h) = Kind::Cottage.size();
    keep_board(c, Rect::new(x0, y0, w, h));
    keep(c, "garden", gx + 4, gy + 5);
    keep(c, "yard", hx + 7, y0 + 10);
    c.k.claim(Rect::new(x0, y0, w, h));
    lane(c, hx + 4, y0 + 9, 2);
}

/// The inn: a long stone house with a sign, a yard, a trough and a cart, and a lamp by the door.
fn inn(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    let house = talk(c, d.p.inn, x0 + 2, y0 + 1, d.t.country_inn);
    own(c, "house", house);
    pad(c, rng, x0 + 12, y0 + 12, 24, 8, Tile::Cobble);
    put(c, d.p.lamp_post, x0 + 15, y0 + 9);
    let trough = put(c, d.p.trough, x0 + 16, y0 + 12);
    own(c, "trough", trough);
    let cart = put(c, d.p.hay_cart, x0 + 20, y0 + 14);
    own(c, "cart", cart);
    put(c, d.p.log, x0 + 3, y0 + 10);
    put(c, d.p.log, x0 + 8, y0 + 13);
    let barrels = put(c, d.p.barrel, x0 + 24, y0 + 2);
    own(c, "barrels", barrels);
    put(c, d.p.barrel, x0 + 24, y0 + 5);
    folk(c, rng, d.u.folk_keeper, x0 + 10, y0 + 10, 3);
    // The regular: a man in a grey coat whose tree is about this inn, its sign and its lamp.
    folk(c, rng, d.u.folk_regular, x0 + 6, y0 + 12, 4);
    let (w, h) = Kind::Inn.size();
    keep_board(c, Rect::new(x0, y0, w, h));
    keep(c, "yard", x0 + 18, y0 + 11);
    keep(c, "back", x0 + 25, y0 + 9);
    c.k.claim(Rect::new(x0, y0, w, h));
    lane(c, x0 + 12, y0 + 14, 3);
}

/// Apple trees in rows, a fence along one side, hives.
fn orchard(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    for j in 0..3 {
        for i in 0..4 {
            let picked = chance(rng, 500);
            let fruit = jane_data::catalog().county.furnishing.orchard;
            let loot = match fruit.filter(|_| !picked) {
                Some(item) => vec![Stack { item, qty: 1 }],
                None => Vec::new(),
            };
            holding(c, d.p.apple_tree, x0 + 2 + i * 5, y0 + 2 + j * 5, loot);
        }
    }
    run(c, x0, y0 + 15, 20, true, Tile::Fence, 10);
    if chance(rng, 500) {
        talk(c, d.p.beehive, x0 + 19, y0 + 8, d.t.country_hive);
    }
    // An orchard's farmer talks about the trees, not about a barn the orchard does not have.
    if chance(rng, 400) {
        folk(c, rng, d.u.folk_orchard, x0 + 9, y0 + 13, 5);
    }
}

/// A field in crops, fenced, the gate toward the road. Sometimes a scarecrow.
fn field(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, b: Rect) {
    let (x0, y0, w, h) = (b.x, b.y, b.w, b.h);
    let crop = chance(rng, 500);
    for j in 1..h - 1 {
        for i in 1..w - 1 {
            let t = match (j % 2 == 1, crop) {
                (true, true) => Tile::Crops,
                (true, false) => Tile::Garden,
                (false, true) => Tile::Dirt,
                (false, false) => Tile::Grass,
            };
            c.k.set(x0 + i, y0 + j, t);
        }
    }
    for j in (1..h - 1).step_by(4) {
        for i in (3..w - 2).step_by(6) {
            put(c, d.p.crop, x0 + i, y0 + j);
        }
    }
    let gate = road_side(c, x0 + (w >> 1), y0 + (h >> 1));
    let t = if chance(rng, 250) { Tile::Bush } else { Tile::Fence };
    pen(c, b, gate, t);
    if chance(rng, 450) {
        put(c, d.p.scarecrow, x0 + (w >> 1) - 1, y0 + (h >> 1));
    }
}

/// A flock: sheep on grass, hens by a house, rabbits anywhere. Friendly, and gone at night.
fn herd(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    let def = if ground(c.sk, x, y).region == Region::Works {
        d.u.rabbit
    } else {
        pick(rng, &[d.u.sheep, d.u.sheep, d.u.rabbit, d.u.hen])
    };
    let n = if def == d.u.sheep { 3 + rng.irandom(3) } else { 2 + rng.irandom(2) };
    for _ in 0..n {
        let fx = x - 5 + rng.irandom(11);
        let fy = y - 4 + rng.irandom(9);
        folk(c, rng, def, fx, fy, 4);
    }
    if def == d.u.sheep && chance(rng, 500) {
        put(c, d.p.trough, x - 1, y + 4);
    }
}

/// A clearing in the wood: stumps, logs, stacked wood, a shelter, and in the Lowfields the
/// woodcutter.
fn woodcutter(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, b: Rect) {
    let (x0, y0, w, h) = (b.x, b.y, b.w, b.h);
    pad(c, rng, x0 + (w >> 1), y0 + (h >> 1), 16, 12, Tile::Dirt);
    for _ in 0..6 {
        let sx = x0 + 2 + rng.irandom(w - 4);
        let sy = y0 + 2 + rng.irandom(h - 4);
        put(c, d.p.stump, sx, sy);
    }
    put(c, d.p.log, x0 + 4, y0 + h - 5);
    put(c, d.p.log, x0 + 12, y0 + 3);
    let pile = put(c, d.p.woodpile, x0 + 7, y0 + 4);
    own(c, "woodpile", pile);
    put(c, d.p.woodpile, x0 + 10, y0 + 4);
    // Known by what it is: a story that says "his shed" only ever claims a clearing with a shed.
    let (name, def) = if chance(rng, 500) { ("shed", d.p.shed) } else { ("tent", d.p.tent) };
    let shelter = talk(c, def, x0 + 13, y0 + 8, d.t.country_shed);
    own(c, name, shelter);
    let fire = put(c, d.p.campfire_cold, x0 + 6, y0 + 9);
    own(c, "fire", fire);
    if ground(c.sk, x0, y0).region == Region::Lowfields {
        folk(c, rng, d.u.folk_woodcutter, x0 + 9, y0 + 12, 5);
    }
    keep_board(c, b);
    keep(c, "clearing", x0 + (w >> 1) + 1, y0 + (h >> 1) + 2);
}

/// Still water in a ragged bowl, tall grass round it, a plank to stand on.
fn pond(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    let rx = 3 + rng.irandom(3);
    let ry = 2 + rng.irandom(2);
    for j in -ry - 2..=ry + 2 {
        for i in -rx - 2..=rx + 2 {
            if super::ellipse_within(i, j, 2 * rx, 2 * ry, 100) {
                c.k.set(x + i, y + j, Tile::Water);
            } else if super::ellipse_within(i, j, 2 * rx, 2 * ry, 160) {
                let t = if chance(rng, 500) { Tile::GrassTall } else { Tile::Moss };
                c.k.set(x + i, y + j, t);
            }
        }
    }
    if chance(rng, 500) {
        for j in 0..3 {
            c.k.set(x, y + ry - j + 1, Tile::FloorWood);
        }
    }
    for _ in 0..3 {
        let fx = x - rx - 3 + rng.irandom(2 * rx + 6);
        let fy = y + ry + 2 + rng.irandom(2);
        put(c, d.p.flowers, fx, fy);
    }
}

/// A ring of stones, some fallen; bones or flowers in the middle, depending on who comes.
fn stones(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    pad(c, rng, x, y, 12, 12, Tile::Dirt);
    for (ox, oy) in [(0, -5), (4, -3), (5, 1), (3, 4), (-1, 5), (-4, 3), (-5, -1), (-3, -4)] {
        if chance(rng, 800) {
            put(c, d.p.standing_stone, x + ox, y + oy);
        }
    }
    let middle = if chance(rng, 500) { d.p.bones } else { d.p.flowers };
    put(c, middle, x, y);
}

/// A camp of something hostile round its fire: three to five of them, and what they keep.
fn camp(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    /// Where they stand round the fire.
    const RING: [(i32, i32); 7] = [(-3, -3), (3, -3), (4, 1), (-4, 1), (0, 4), (0, -4), (-2, 3)];
    let g = ground(c.sk, x, y);
    let u = &d.u;
    pad(c, rng, x, y, 14, 11, Tile::Dirt);
    let mut lit = false;
    let mut who: Vec<UnitDefId> = match g.region {
        Region::Lowfields => match rng.irandom(2) {
            0 => {
                lit = true;
                vec![u.ruffian; 4]
            }
            1 => vec![u.skeleton; 3],
            _ if g.biome == Biome::Wood => vec![u.spider; 3],
            _ => vec![u.crow; 4],
        },
        Region::Waters => match rng.irandom(2) {
            0 => {
                lit = true;
                vec![u.ruffian; 3]
            }
            1 => vec![u.skeleton, u.skeleton, u.bat, u.skeleton],
            _ if g.biome == Biome::Garden => vec![u.flower, u.flower, u.statue],
            _ => vec![u.rat; 4],
        },
        Region::Works => {
            if chance(rng, 500) {
                vec![u.soldier, u.soldier, u.skeleton_guard]
            } else {
                vec![u.skeleton_guard, u.skeleton_clerk, u.skeleton_guard, u.soldier]
            }
        }
    };
    if chance(rng, 350) {
        who.push(who[0]);
    }
    let fire = put(c, if lit { d.p.camp_fire } else { d.p.campfire_cold }, x - 1, y - 1);
    own(c, "fire", fire);
    if lit {
        let tent = talk(c, d.p.tent, x - 6, y - 5, d.t.country_tent);
        own(c, "tent", tent);
        put(c, d.p.bedroll, x + 2, y - 4);
        put(c, d.p.bedroll, x - 5, y + 2);
    } else if g.region == Region::Works {
        put(c, d.p.sleepers, x - 6, y - 3);
        put(c, d.p.barrel, x + 3, y - 4);
    } else {
        put(c, d.p.bones, x + 2, y + 2);
        put(c, d.p.bones, x - 4, y - 3);
    }
    let l = loot(c, rng, x, y);
    let chest = holding(c, d.p.chest, x + 3, y + 1, l);
    own(c, "chest", chest);
    let crate_ = put(c, d.p.crate_, x - 6, y + 1);
    own(c, "crate", crate_);
    for (i, &def) in who.iter().enumerate() {
        let (ox, oy) = RING[i % RING.len()];
        hostile(c, def, x + ox, y + oy, Vec::new());
    }
    let (w, h) = Kind::Camp.size();
    keep_board(c, Rect::new(x - (w >> 1), y - (h >> 1), w, h));
    keep(c, "ground", x + 1, y + 3);
}

/// A hole in a bank, and what lives in it.
fn den(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    let g = ground(c.sk, x, y);
    pad(c, rng, x, y + 1, 10, 8, Tile::Dirt);
    let who = if g.region == Region::Works {
        if g.biome == Biome::Slag { d.u.cactus } else { d.u.wall_spider }
    } else if matches!(g.biome, Biome::Wood | Biome::WetWood) {
        d.u.spider
    } else if chance(rng, 300) {
        d.u.bat
    } else {
        d.u.rat
    };
    if who == d.u.spider {
        put(c, d.p.web, x - 5, y - 3);
    } else {
        let den = talk(c, d.p.den, x - 1, y - 2, d.t.country_den);
        own(c, "den", den);
    }
    put(c, d.p.bones, x + 3, y + 2);
    put(c, d.p.bones, x - 3, y + 3);
    let n = 2 + rng.irandom(3);
    for _ in 0..n {
        let hx = x - 3 + rng.irandom(7);
        let hy = y + 1 + rng.irandom(3);
        hostile(c, who, hx, hy, Vec::new());
    }
}

/// A house with no roof, or four walls to the sill: a doorway, rubble inside, growth coming in. A
/// ruin built for a tale is the sort the tale asked for.
fn ruin(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    let region = ground(c.sk, x0, y0).region;
    let (bw, bh) = Kind::Ruin.size();
    let b = Rect::new(x0, y0, bw, bh);
    let house = match c.country.ruin_as {
        Some(as_) => as_ == RuinKind::House && region != Region::Works,
        None => region != Region::Works && chance(rng, 350),
    };
    if house {
        let h = talk(c, d.p.cottage_empty, x0 + 4, y0 + 2, d.t.country_door_empty);
        own(c, "house", h);
        pad(c, rng, x0 + 8, y0 + 10, 10, 4, Tile::Dirt);
        for _ in 0..4 {
            let bx = x0 + 1 + rng.irandom(14);
            let by = y0 + 9 + rng.irandom(3);
            let k = &c.k;
            if !k.is_claimed(bx, by)
                && !k.solid(bx - 1, by)
                && !k.solid(bx + 1, by)
                && !k.solid(bx, by - 1)
                && !k.solid(bx, by + 1)
            {
                c.k.set(bx, by, Tile::Bush);
            }
        }
        keep_board(c, b);
        // A roofless cottage is looked at from its yard: there is no "inside the walls".
        keep(c, "yard", x0 + 9, y0 + 10);
        keep(c, "step", x0 + 5, y0 + 10);
        return;
    }
    let wall = if region == Region::Works || !chance(rng, 500) { Tile::Wall } else { Tile::HouseWall };
    let (wx, wy, w, h) = (x0 + 3, y0 + 2, 10, 8);
    for j in 0..h {
        for i in 0..w {
            let t = if chance(rng, 250) { Tile::Rubble } else { Tile::Dirt };
            c.k.set(wx + i, wy + j, t);
        }
    }
    // The walls, with a doorway south and a gap where the east wall came down.
    for i in 0..w {
        c.k.set(wx + i, wy, wall);
        if !(3..=5).contains(&i) {
            c.k.set(wx + i, wy + h - 1, wall);
        }
    }
    for j in 0..h {
        c.k.set(wx, wy + j, wall);
        if !(2..=4).contains(&j) {
            c.k.set(wx + w - 1, wy + j, wall);
        }
    }
    // Rubble is solid: inside, only where it cannot close the doorway off.
    for j in 1..h - 1 {
        for i in 1..w - 1 {
            if c.k.get(wx + i, wy + j) == Tile::Rubble && (j >= h - 3 || i == 1 || i == w - 2) {
                c.k.set(wx + i, wy + j, Tile::Dirt);
            }
        }
    }
    if region == Region::Works {
        put(c, d.p.barrel, wx + 2, wy + 2);
        put(c, d.p.sleepers, wx + w + 1, wy + 3);
    } else {
        let crate_ = put(c, d.p.crate_, wx + 2, wy + 2);
        own(c, "crate", crate_);
        let fire = put(c, d.p.campfire_cold, wx + 5, wy + 3);
        own(c, "fire", fire);
    }
    if chance(rng, 300) {
        let l = loot(c, rng, wx, wy);
        let chest = holding(c, d.p.chest, wx + 6, wy + 1, l);
        own(c, "chest", chest);
    }
    keep_board(c, b);
    keep(c, "inside", wx + 5, wy + 5);
    keep(c, "step", wx + 4, wy + h + 1);
}

/// Bare rock breaking the turf: a crag you walk round, boulders fallen off it.
fn outcrop(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    let r = 2 + rng.irandom(2);
    for j in -r..=r {
        for i in -r - 1..=r + 1 {
            // i² * 0.7 + j² <= r², in tenths.
            if 7 * i * i + 10 * j * j <= 10 * r * r {
                c.k.set(x + i, y + j, Tile::Cliff);
            }
        }
    }
    put(c, d.p.boulder, x + r + 2, y + 1);
    put(c, d.p.rock, x - r - 2, y + 2);
    put(c, d.p.rock, x + 1, y + r + 2);
}

/// A reedcutter's hut on the wet ground: thatch, a plank landing, a stack of cut reed.
fn reedhut(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    pad(c, rng, x0 + 8, y0 + 8, 14, 8, Tile::Dirt);
    let door = pick(rng, &d.doors());
    talk(c, d.p.reed_hut, x0 + 2, y0 + 1, door);
    put(c, d.p.haystack, x0 + 10, y0 + 2);
    put(c, d.p.crate_, x0 + 10, y0 + 6);
    for i in 0..4 {
        c.k.set(x0 + 12 + i, y0 + 10, Tile::FloorWood);
    }
    if ground(c.sk, x0, y0).region == Region::Waters && chance(rng, 800) {
        folk(c, rng, d.u.folk_reedcutter, x0 + 7, y0 + 8, 4);
    }
}

/// A glasshouse with the glass mostly out of it, beds of something still growing inside.
fn glasshouse(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    let (w, h, gx, gy) = (14, 9, x0 + 2, y0 + 2);
    for j in 0..h {
        for i in 0..w {
            c.k.set(gx + i, gy + j, if j % 2 == 0 { Tile::Garden } else { Tile::Dirt });
        }
    }
    for i in 0..w {
        if chance(rng, 700) {
            c.k.set(gx + i, gy, Tile::Glass);
        }
        if !(5..=8).contains(&i) && chance(rng, 700) {
            c.k.set(gx + i, gy + h - 1, Tile::Glass);
        }
    }
    for j in 0..h {
        if chance(rng, 700) {
            c.k.set(gx, gy + j, Tile::Glass);
        }
        if chance(rng, 700) {
            c.k.set(gx + w - 1, gy + j, Tile::Glass);
        }
    }
    for _ in 0..5 {
        let fx = gx + 2 + rng.irandom(w - 4);
        let fy = gy + 2 + rng.irandom(h - 4);
        put(c, d.p.flowers, fx, fy);
    }
    put(c, d.p.barrel, gx + w + 1, gy + 1);
}

/// The Works' leavings: a heap of slag, a length of track, sleepers, a tub off its wheels.
fn slag(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x: i32, y: i32) {
    pad(c, rng, x, y, 14, 10, Tile::DryBed);
    put(c, d.p.slag_heap, x - 5, y - 4);
    if chance(rng, 500) {
        put(c, d.p.slag_heap, x + 1, y - 5);
    }
    for i in -6..=6 {
        if c.k.get(x + i, y + 3) != Tile::Water {
            c.k.set(x + i, y + 3, Tile::Track);
        }
    }
    put(c, d.p.minecart, x + 2, y + 1);
    put(c, d.p.sleepers, x - 4, y + 1);
}

/// A few graves inside a low wall with a gate.
fn graves(c: &mut County<'_>, rng: &mut Sfc32, d: &Defs, x0: i32, y0: i32) {
    let (gx, gy) = (x0 + 1, y0 + 1);
    pen(c, Rect::new(gx, gy, 14, 10), Side::S, Tile::StoneWall);
    for j in 0..2 {
        for i in 0..4 {
            if chance(rng, 800) {
                let (sx, sy) = (gx + 2 + i * 3, gy + 2 + j * 3);
                if i == 1 && j == 0 {
                    talk(c, d.p.gravestone, sx, sy, d.t.country_grave);
                } else {
                    put(c, d.p.gravestone, sx, sy);
                }
            }
        }
    }
    put(c, d.p.flowers, gx + 6, gy + 7);
}
