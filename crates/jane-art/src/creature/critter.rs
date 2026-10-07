//! The county's small lives (ART-PLAN M1): the birds, ducks and butterflies the presenter's
//! ambient layer scatters over a scene, the smoke over a lived-in roof, and the lily pads at a
//! lake's margin. None of them is a unit: they have no footprint, no id in the sim and no row in
//! the catalog; the presenter owns them, seeded by cell and tick.
//!
//! They are small (a sparrow is nine px from beak to tail), so each is drawn from hand-placed
//! parts rather than inflated volumes: a body with its folded wing, a head, legs, a raised or a
//! lowered wing, each a little grid of tones in the creature's own ramps, composed per frame with
//! the head and body offset by the pose (a peck drops the head, a hop lifts the bird, a flap
//! raises the wing). The birds then take the selective outline, a contact shadow and their true
//! heights, as every creature does; the butterflies are too thin to outline and carry their own
//! dark edge.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::grid::Rect;

use crate::canvas::{Canvas, FLAT, Normal, UNIT, normal};
use crate::hash::h32;
use crate::palette::{Ix, Ramp, Tone};
use jane_core::Angle;
use jane_core::angle::{cos_q15, sin_q15};

/// A small life of the ambient layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Critter {
    /// A house sparrow: brown streaked back, grey crown, black bib. Gardens, hedges, the square.
    Sparrow,
    /// A robin: olive-brown back, an orange face and breast. Gardens, a spade's handle.
    Robin,
    /// A feral pigeon: blue-grey, a sheen of green and purple at the neck. The square, the Works.
    Pigeon,
    /// A mallard drake on the water: a green head, a white collar, a chestnut breast.
    Drake,
    /// A mallard duck: mottled brown, an orange bill.
    Duck,
    /// A cabbage white.
    White,
    /// A red admiral: black with a red band and white tips.
    Admiral,
    /// A small tortoiseshell: orange with a dark edge.
    Tortoiseshell,
}

impl Critter {
    /// Every critter, in a stable order.
    pub const ALL: [Critter; 8] = [
        Critter::Sparrow,
        Critter::Robin,
        Critter::Pigeon,
        Critter::Drake,
        Critter::Duck,
        Critter::White,
        Critter::Admiral,
        Critter::Tortoiseshell,
    ];

    /// The name a sheet labels it with.
    pub const fn name(self) -> &'static str {
        match self {
            Critter::Sparrow => "sparrow",
            Critter::Robin => "robin",
            Critter::Pigeon => "pigeon",
            Critter::Drake => "drake",
            Critter::Duck => "duck",
            Critter::White => "white",
            Critter::Admiral => "admiral",
            Critter::Tortoiseshell => "tortoiseshell",
        }
    }

    /// A butterfly (it flutters and never stands).
    pub const fn flutters(self) -> bool {
        matches!(self, Critter::White | Critter::Admiral | Critter::Tortoiseshell)
    }

    /// A duck (it sits on the water and never stands).
    pub const fn swims(self) -> bool {
        matches!(self, Critter::Drake | Critter::Duck)
    }

    /// The frames it has, in order.
    pub fn poses(self) -> &'static [Pose] {
        if self.flutters() {
            &[Pose::Fly1, Pose::Fly2, Pose::Glide]
        } else if self.swims() {
            &[Pose::Stand, Pose::Look, Pose::Peck]
        } else {
            &[Pose::Stand, Pose::Peck, Pose::Look, Pose::Hop, Pose::Fly1, Pose::Fly2]
        }
    }
}

/// What a critter's frame shows. A duck's `Stand` and `Look` are its paddle's two beats (the
/// ripple at its breast this side, then that), its `Peck` a dabble, tail up. A butterfly's
/// `Fly1`, `Fly2` and `Glide` are its wings open, half and shut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pose {
    /// Standing, side on, facing east (west is this mirrored).
    Stand,
    /// The head down to the ground.
    Peck,
    /// The head up, alert.
    Look,
    /// In the air between two hops, the feet tucked.
    Hop,
    /// The wings up over the back.
    Fly1,
    /// The wings down under the body.
    Fly2,
    /// Wings held.
    Glide,
}

/// One critter's frames: every frame one box, its anchor where it stands (a bird's feet, a
/// duck's waterline, a butterfly's middle).
#[derive(Clone, Debug)]
pub struct CritterSet {
    /// Frame width, px.
    pub w: i32,
    /// Frame height, px.
    pub h: i32,
    /// Anchor x.
    pub ax: i32,
    /// Anchor y.
    pub ay: i32,
    /// Every frame, in [`Critter::poses`] order.
    pub frames: Vec<(Pose, Canvas)>,
}

/// The ramps a critter is painted in, by slot.
#[derive(Clone, Copy, Debug)]
struct Coat {
    body: Ramp,
    wing: Ramp,
    belly: Ramp,
    head: Ramp,
    accent: Ramp,
    accent2: Ramp,
    beak: Ramp,
}

/// A part: rows of slot letters, placed at `(x, y)`, standing `z` px.
type Part = &'static [&'static str];

/// The index a part's letter paints, or `None` for clear. Body `1..5` (deep to high), wing
/// `d a b c` (deep, shade, base, light), belly `p q r` (shade, base, light), head `h i j`, the
/// accents `x y z` and `m n`, beak `v w` (base, shade), `k` ink, `g` a glint, `f` a leg (the
/// beak's shade), `u` a ripple of the water's light, `s` the water's lit lip.
fn ix(c: Coat, ch: u8) -> Option<Ix> {
    Some(match ch {
        b'1' => c.body.at(Tone::Deep),
        b'2' => c.body.at(Tone::Shade),
        b'3' => c.body.at(Tone::Base),
        b'4' => c.body.at(Tone::Light),
        b'5' => c.body.at(Tone::High),
        b'd' => c.wing.at(Tone::Deep),
        b'a' => c.wing.at(Tone::Shade),
        b'b' => c.wing.at(Tone::Base),
        b'c' => c.wing.at(Tone::Light),
        b'p' => c.belly.at(Tone::Shade),
        b'q' => c.belly.at(Tone::Base),
        b'r' => c.belly.at(Tone::Light),
        b'h' => c.head.at(Tone::Shade),
        b'i' => c.head.at(Tone::Base),
        b'j' => c.head.at(Tone::Light),
        b'x' => c.accent.at(Tone::Shade),
        b'y' => c.accent.at(Tone::Base),
        b'z' => c.accent.at(Tone::Light),
        b'm' => c.accent2.at(Tone::Base),
        b'n' => c.accent2.at(Tone::Light),
        b'v' => c.beak.at(Tone::Base),
        b'w' | b'f' => c.beak.at(Tone::Shade),
        b'k' => Ix::INK,
        b'g' => Ramp::HairWhite.at(Tone::High),
        b'u' => Ramp::Water.at(Tone::Light),
        b's' => Ramp::Water.at(Tone::High),
        _ => return None,
    })
}

/// Stamp `part` at `(x, y)`, `z` px high, as one part of the canvas: each px's normal leans out
/// from the part's middle, so a body turns to a lamp as a ball does.
fn stamp(cv: &mut Canvas, c: Coat, part: Part, (x, y): (i32, i32), z: u8, mirror: bool) {
    cv.begin();
    let h = part.len() as i32;
    let w = part.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
    for (r, row) in part.iter().enumerate() {
        for (k, &ch) in row.as_bytes().iter().enumerate() {
            let Some(px) = ix(c, ch) else { continue };
            let (r, k) = (r as i32, k as i32);
            let k = if mirror { w - 1 - k } else { k };
            let n = lean(k, r, w, h);
            cv.put(x + k, y + r, px, n, z);
        }
    }
}

/// A normal leaning out from the middle of a `w` x `h` part at `(k, r)`.
fn lean(k: i32, r: i32, w: i32, h: i32) -> Normal {
    if w < 2 || h < 2 {
        return FLAT;
    }
    normal((2 * k - (w - 1)) * UNIT * 2 / (3 * w), (2 * r - (h - 1)) * UNIT * 2 / (3 * h))
}

// The sparrow, east-facing. Its body carries the folded wing (streaked) and the tail.
const SPARROW_BODY: Part = &[
    "......3344..", //
    "....23abab3q", //
    "..2aabcbaabq", //
    "22aabbaabqqr", //
    "..2aabbbqqr.", //
    ".....ppqqp..", //
];
const SPARROW_HEAD: Part = &[
    ".hhh..", //
    "hiiii.", //
    "xikivv", //
    "xjjkw.", //
    ".jkk..", //
];
const ROBIN_BODY: Part = &[
    ".....3344y..", //
    "...233abyyy.", //
    ".22abbaayyz.", //
    "22aabbaqyyz.", //
    "..2abbqqqr..", //
    "....ppqqp...", //
];
const ROBIN_HEAD: Part = &[
    ".hhh..", //
    "hiiii.", //
    "iykyvv", //
    "yyyyw.", //
    ".yyy..", //
];
const PIGEON_BODY: Part = &[
    "......333444..", //
    "....2333bbcc4q", //
    "..12aabdbbbcqq", //
    "1122aabdbbbqqq", //
    "..12aaabbbpqq.", //
    "....2aaapppq..", //
    "......pppp....", //
];
const PIGEON_HEAD: Part = &[
    ".hhh...", //
    "hiiih..", //
    "iikiivv", //
    "hiiiiw.", //
    ".mnmn..", //
    ".nmnm..", //
];
/// A raised wing over the back (a bird's flap, up).
const WING_UP: Part = &[
    "....bc", //
    "...abc", //
    "..abcc", //
    ".aabc.", //
    "aabb..", //
];
/// A lowered wing under the body (the flap's down beat).
const WING_DOWN: Part = &[
    "abbbbc", //
    ".abbc.", //
    "..ab..", //
];
const PIGEON_WING_UP: Part = &[
    ".....bbc", //
    "....abbc", //
    "...adbcc", //
    "..aadbc.", //
    ".aadbb..", //
    "aadb....", //
];
const PIGEON_WING_DOWN: Part = &[
    "aadbbbbc", //
    ".aadbbc.", //
    "..aadb..", //
    "...ab...", //
];
// A duck on the water: the hull to the waterline, its ripple under it.
const DRAKE_BODY: Part = &[
    ".kk.....333444.....", //
    "kkk2333abbbcc4yy...", //
    ".kk2aabbbbbcqqyyz..", //
    "..2aaabbbbqqqqyy...", //
    "...222aaappppyy....", //
];
const DRAKE_HEAD: Part = &[
    ".hhh...", //
    "hiiij..", //
    "iikiivv", //
    "hiiiivw", //
    ".rrrr..", //
];
const DUCK_BODY: Part = &[
    ".33.....333444.....", //
    "323a2a3abab4c4qq...", //
    ".22aabababbcqqqqr..", //
    "..2aaabababqpqqq...", //
    "...222aaappppqq....", //
];
const DUCK_HEAD: Part = &[
    ".hhh...", //
    "hiiij..", //
    "ixkiivv", //
    "hiiiivw", //
    ".iiii..", //
];
/// The duck's tail tipped up as it dabbles: the hull tilted forward, the head under.
const DRAKE_DABBLE: Part = &[
    "..kk...............", //
    "..kkk3344..........", //
    "...kk2333abbcc4....", //
    "....22aabbbbcqqyy..", //
    ".....222aaapppyyy..", //
];
const DUCK_DABBLE: Part = &[
    "..33...............", //
    "..323a344..........", //
    "...22aabab4c4q.....", //
    "....22aababcqqqq...", //
    ".....222aaappqqq...", //
];
/// The ripple at the waterline, its two beats.
const RIPPLE_A: Part = &["us...........suu..", ".uu.........uu...."];
const RIPPLE_B: Part = &["..us.......su.....", "uu...........uu..."];

// A butterfly, seen from above and a little behind: wings open, half and shut. Its own dark
// edge (`d`), the body `k`.
const FLY_OPEN: Part = &[
    "dbb...bbd", //
    "dbcbkbcbd", //
    ".dbbkbbd.", //
    "..dbkbd..", //
    "..da.ad..", //
];
const FLY_HALF: Part = &[
    "...b.b...", //
    "..dbkbd..", //
    "..dckcd..", //
    "...dkd...", //
];
const FLY_SHUT: Part = &[
    "....d....", //
    "...dbd...", //
    "...dck...", //
    "....k....", //
];

/// The coat a critter is painted in.
fn coat(c: Critter) -> Coat {
    let plain = |r: Ramp| Coat { body: r, wing: r, belly: r, head: r, accent: r, accent2: r, beak: Ramp::HairBlack };
    match c {
        Critter::Sparrow => Coat {
            body: Ramp::HairBrown,
            wing: Ramp::ClothBrown,
            belly: Ramp::ClothLinen,
            head: Ramp::HairGrey,
            accent: Ramp::HairRed,
            accent2: Ramp::HairRed,
            beak: Ramp::HairBlack,
        },
        Critter::Robin => Coat {
            body: Ramp::HairBrown,
            wing: Ramp::ClothBrown,
            belly: Ramp::ClothCream,
            head: Ramp::HairBrown,
            accent: Ramp::ClothBrick,
            accent2: Ramp::ClothBrick,
            beak: Ramp::HairBlack,
        },
        Critter::Pigeon => Coat {
            body: Ramp::Slate,
            wing: Ramp::ClothGrey,
            belly: Ramp::ClothGrey,
            head: Ramp::Slate,
            accent: Ramp::ClothTeal,
            accent2: Ramp::ClothPlum,
            beak: Ramp::HairGrey,
        },
        Critter::Drake => Coat {
            body: Ramp::ClothGrey,
            wing: Ramp::ClothLinen,
            belly: Ramp::ClothLinen,
            head: Ramp::ClothGreen,
            accent: Ramp::HairRed,
            accent2: Ramp::HairRed,
            beak: Ramp::ClothMustard,
        },
        Critter::Duck => Coat {
            body: Ramp::HairBrown,
            wing: Ramp::ClothBrown,
            belly: Ramp::ClothOchre,
            head: Ramp::HairBrown,
            accent: Ramp::HairBrown,
            accent2: Ramp::HairBrown,
            beak: Ramp::ClothOchre,
        },
        Critter::White => Coat { wing: Ramp::ClothCream, ..plain(Ramp::ClothCream) },
        Critter::Admiral => Coat { wing: Ramp::ClothRed, ..plain(Ramp::ClothBlack) },
        Critter::Tortoiseshell => Coat { wing: Ramp::ClothOchre, ..plain(Ramp::HairBrown) },
    }
}

/// The box and anchor `(w, h, ax, ay)` of a critter.
pub const fn size(c: Critter) -> (i32, i32, i32, i32) {
    match c {
        Critter::Sparrow | Critter::Robin => (18, 14, 9, 12),
        Critter::Pigeon => (22, 16, 11, 14),
        Critter::Drake | Critter::Duck => (22, 12, 11, 9),
        Critter::White | Critter::Admiral | Critter::Tortoiseshell => (9, 5, 4, 2),
    }
}

/// Every frame of `c`.
pub fn render(c: Critter) -> CritterSet {
    let (w, h, ax, ay) = size(c);
    let k = coat(c);
    let frames = c
        .poses()
        .iter()
        .map(|&p| {
            let mut cv = Canvas::new(w, h);
            if c.flutters() {
                let part = match p {
                    Pose::Fly1 => FLY_OPEN,
                    Pose::Fly2 => FLY_HALF,
                    _ => FLY_SHUT,
                };
                stamp(&mut cv, k, part, (0, (h - part.len() as i32) / 2), 3, false);
                // Its wings' own pattern: an admiral's white tips, a tortoiseshell's dark spots.
                if c == Critter::Admiral && p == Pose::Fly1 {
                    for x in [1, 7] {
                        cv.recolour(x, 0, Ramp::HairWhite.at(Tone::Light));
                    }
                }
                cv.upright(h - 1);
                // Light as it is: never under a contact shadow, a few px over its ground point.
                cv.heights_by(Rect::new(0, 0, w, h), |_, _| 8);
            } else if c.swims() {
                swim(&mut cv, k, c, p);
                cv.outline();
                cv.upright(ay);
            } else {
                stand(&mut cv, k, c, p);
                cv.outline();
                cv.upright(ay);
            }
            (p, cv)
        })
        .collect();
    CritterSet { w, h, ax, ay, frames }
}

/// A bird on its feet or in the air.
fn stand(cv: &mut Canvas, k: Coat, c: Critter, p: Pose) {
    let (_, _, ax, ay) = size(c);
    let pigeon = c == Critter::Pigeon;
    let (body, head, up, down) = match c {
        Critter::Robin => (ROBIN_BODY, ROBIN_HEAD, WING_UP, WING_DOWN),
        Critter::Pigeon => (PIGEON_BODY, PIGEON_HEAD, PIGEON_WING_UP, PIGEON_WING_DOWN),
        _ => (SPARROW_BODY, SPARROW_HEAD, WING_UP, WING_DOWN),
    };
    let bw = body[0].len() as i32;
    let bh = body.len() as i32;
    // The body's foot row sits a px over the feet.
    let (bx, by) = (ax - bw / 2 - 1, ay - bh);
    let head_at = (bx + bw - 3, by - head.len() as i32 + 2);
    let (lift, (hx, hy), legs) = match p {
        Pose::Stand => (0, head_at, true),
        Pose::Peck => (0, (head_at.0 + 1, head_at.1 + 3 + i32::from(pigeon)), true),
        Pose::Look => (0, (head_at.0, head_at.1 - 1), true),
        Pose::Hop => (2, head_at, false),
        _ => (3, head_at, false),
    };
    if p == Pose::Fly2 {
        // The far wing, down behind the body.
        stamp(cv, k, down, (bx + 3, by + bh - 2 - lift), 2, false);
    }
    stamp(cv, k, body, (bx, by - lift), 4, false);
    stamp(cv, k, head, (hx, hy - lift), 6, false);
    match p {
        Pose::Fly1 => stamp(cv, k, up, (bx + 2, by - up.len() as i32 + 2 - lift), 7, false),
        Pose::Fly2 => stamp(cv, k, down, (bx + 2, by + bh - 3 - lift), 7, false),
        _ => {}
    }
    if legs {
        let leg = k.beak.at(Tone::Shade);
        cv.begin();
        for x in [ax - 1, ax + 1] {
            cv.put(x, ay - 1, leg, FLAT, 1);
            cv.put(x, ay, leg, FLAT, 1);
        }
        cv.put(ax + 2, ay, leg, FLAT, 1);
        cv.put(ax, ay, leg, FLAT, 1);
        cv.ao_contact(Rect::new(ax - 4, ay - 1, 9, 3), 0);
    }
}

/// A duck on the water: the hull to its waterline, the head, the ripple's beat.
fn swim(cv: &mut Canvas, k: Coat, c: Critter, p: Pose) {
    let (_, _, ax, ay) = size(c);
    let drake = c == Critter::Drake;
    let (body, dabble, head) =
        if drake { (DRAKE_BODY, DRAKE_DABBLE, DRAKE_HEAD) } else { (DUCK_BODY, DUCK_DABBLE, DUCK_HEAD) };
    let bw = body[0].len() as i32;
    let (bx, by) = (ax - bw / 2, ay - body.len() as i32 + 1);
    if p == Pose::Peck {
        stamp(cv, k, dabble, (bx, by), 3, false);
    } else {
        stamp(cv, k, body, (bx, by), 3, false);
        let hx = bx + bw - 7 + i32::from(p == Pose::Look);
        stamp(cv, k, head, (hx, by - 4), 5, false);
        if drake {
            // The white collar.
            cv.begin();
            for x in hx + 1..hx + 5 {
                cv.put(x, by, Ramp::HairWhite.at(Tone::Light), FLAT, 5);
            }
        }
    }
    let ripple = if p == Pose::Look { RIPPLE_B } else { RIPPLE_A };
    stamp(cv, k, ripple, (bx, ay), 1, false);
}

/// A chimney's puff of smoke, `r` px across its middle (1 to 5): a soft lump in clusters, lit on
/// its upper left and greyer under, a bump or two on its rim by `seed`. Flat to the light (smoke
/// has no relief), its height its own middle so a lamp lights it from beside.
pub fn puff(r: i32, seed: u32) -> Canvas {
    let r = r.clamp(1, 6);
    let d = 2 * r + 5;
    let mut cv = Canvas::new(d, d);
    let ramp = Ramp::HairGrey;
    let (cx, cy) = (d / 2, d / 2 + 1);
    // A lump of three or four rounds, the biggest low in the middle, the others hashed round
    // its upper rim: a cauliflower, not a ball.
    let mut lumps = vec![(cx, cy, r)];
    for b in 0..(1 + r.min(3)) as u32 {
        let hb = h32(seed, b, 0x5e0c);
        let a = Angle((hb & 0xffff) as u16 | 0x8000);
        let rr = (r * 2 / 3).max(1) - (hb >> 20) as i32 % 2;
        let dist = r - rr / 2;
        let (ox, oy) = (cos_q15(a).0 * dist / 32768, sin_q15(a).0 * dist / 32768);
        lumps.push((cx + ox, cy + oy.min(0), rr.max(1)));
    }
    cv.begin();
    for y in 0..d {
        for x in 0..d {
            // Lit from its upper left, in the lump it is nearest the top of.
            let mut best: Option<i32> = None;
            for &(lx, ly, lr) in &lumps {
                let (dx, dy) = (x - lx, y - ly);
                if dx * dx + dy * dy <= lr * lr + lr / 2 {
                    let s = (-dx - dy * 3 / 2) * 6 / (lr + 1);
                    best = Some(best.map_or(s, |b: i32| b.max(s)));
                }
            }
            let Some(s) = best else { continue };
            let t = match s {
                s if s > 5 => Tone::High,
                s if s > 1 => Tone::Light,
                s if s > -3 => Tone::Lift,
                s if s > -6 => Tone::Base,
                _ => Tone::Mid,
            };
            cv.put(x, y, ramp.at(t), FLAT, 2);
        }
    }
    cv
}

/// A lily pad, its notch toward `seed`'s side; some yellowed for October, one in three with a
/// flower. Flat on the water.
pub fn lily_pad(seed: u32) -> Canvas {
    let (w, h) = (13, 8);
    let mut cv = Canvas::new(w, h);
    let ramp = match seed % 4 {
        0 => Ramp::LeafOlive,
        1 => Ramp::TurfDry,
        _ => Ramp::Leaf,
    };
    // The notch: a wedge from the middle out to the rim, east or west.
    let east = seed >> 2 & 1 == 1;
    let (cx2, cy2) = (w - 1, h - 2);
    cv.begin();
    for y in 0..h - 1 {
        for x in 0..w {
            // An ellipse seen low: twice as wide as it is deep.
            let (dx, dy) = (2 * x - cx2, 2 * y - cy2);
            if dx * dx * (h - 1) * (h - 1) + dy * dy * w * w > w * w * (h - 1) * (h - 1) {
                continue;
            }
            let wedge = if east { dx > 0 } else { dx < 0 };
            if wedge && dy.abs() * 3 <= dx.abs() && dy <= 0 {
                continue;
            }
            // Lit on its far rim, shaded on its near one; the veins a shade from the notch.
            let edge = dx * dx * (h - 1) * (h - 1) + dy * dy * w * w > w * w * (h - 1) * (h - 1) * 2 / 3;
            let t = if edge && dy < 0 {
                Tone::Light
            } else if edge && dy > 0 {
                Tone::Shade
            } else if (dx == 0 || dx.abs() == dy.abs() * 3) && dy > 0 {
                Tone::Mid
            } else {
                Tone::Base
            };
            cv.put(x, y, ramp.at(t), FLAT, 1);
        }
    }
    if seed % 3 == 0 {
        // A water lily: white petals round a yellow heart, a pink tip or two.
        let fx = if east { 3 } else { w - 5 };
        for (x, y, ix) in [
            (1, 0, Ramp::HairWhite.at(Tone::High)),
            (0, 1, Ramp::HairWhite.at(Tone::Light)),
            (1, 1, Ramp::ClothMustard.at(Tone::Light)),
            (2, 1, Ramp::HairWhite.at(Tone::Light)),
            (1, 2, Ramp::HairWhite.at(Tone::Base)),
            (2, 0, Ramp::ClothRose.at(Tone::Light)),
        ] {
            cv.put(fx + x, y, ix, FLAT, 2);
        }
    }
    cv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_critter_draws_every_pose_and_its_parts_line_up() {
        for c in Critter::ALL {
            let s = render(c);
            assert_eq!(s.frames.len(), c.poses().len(), "{}", c.name());
            for (p, cv) in &s.frames {
                let n = cv.albedo().iter().filter(|a| a.is_opaque()).count();
                assert!(n >= 8, "{} {p:?}: {n} px", c.name());
                assert!(cv.validate().is_ok(), "{} {p:?}", c.name());
            }
        }
        // The parts' rows are one width each.
        for part in [
            SPARROW_BODY,
            SPARROW_HEAD,
            ROBIN_BODY,
            ROBIN_HEAD,
            PIGEON_BODY,
            PIGEON_HEAD,
            WING_UP,
            WING_DOWN,
            PIGEON_WING_UP,
            PIGEON_WING_DOWN,
            DRAKE_BODY,
            DRAKE_HEAD,
            DUCK_BODY,
            DUCK_HEAD,
            DRAKE_DABBLE,
            DUCK_DABBLE,
            RIPPLE_A,
            RIPPLE_B,
            FLY_OPEN,
            FLY_HALF,
            FLY_SHUT,
        ] {
            assert!(part.iter().all(|r| r.len() == part[0].len()), "{part:?}");
        }
    }

    #[test]
    fn a_peck_drops_the_head_and_a_flap_lifts_the_bird() {
        let s = render(Critter::Sparrow);
        let top = |p: Pose| {
            let cv = &s.frames.iter().find(|(q, _)| *q == p).unwrap().1;
            cv.bounds().unwrap()
        };
        assert!(top(Pose::Peck).y > top(Pose::Stand).y);
        assert!(top(Pose::Fly1).y < top(Pose::Stand).y);
    }

    #[test]
    fn puffs_grow_with_their_size_and_pads_lie_flat() {
        let n = |c: &Canvas| c.albedo().iter().filter(|a| a.is_opaque()).count();
        assert!(n(&puff(1, 3)) < n(&puff(3, 3)) && n(&puff(3, 3)) < n(&puff(5, 3)));
        for s in 0..6 {
            let p = lily_pad(s);
            assert!(p.heights().iter().all(|&h| h <= 2));
            assert!(n(&p) > 30, "pad {s}: {} px", n(&p));
        }
    }
}
