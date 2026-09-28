//! The silhouette shadow's integers (PRESENTATION.md §1.7, T0 and T1): what `soft` lays in its
//! mask and `gl2` draws as span quads, one function for both so the two tiers lay the same
//! shadow to the px (the gl2 albedo test holds them to it).
//!
//! A caster stands on its foot row. Each row of its silhouette `hv` rows above the foot is
//! `height_of_rows(hv)` true px up (the 3/4 view's one projection, `rows_up`), so the sun lays it
//! that height times the cotangent of its elevation away from the sun, stretched to meet the row
//! above it and as thick as the caster is deep, its depth behind the foot row (a sprite's lowest
//! px is the front of what it stands on; T2's field stands it the same). Under the rows, a **foot**: an ellipse two px
//! wider each side than the silhouette's lowest rows, drawn out a few px along the shadow, so the
//! shadow is seen to grow out of the feet even where the body hides where it starts (in the 3/4
//! view a shadow thrown up the screen runs behind the body that casts it).
//!
//! Every band carries a **reach**: how high over the ground it lies the shadow reaches, the ray
//! over the caster's top there. A lifted receiver (a wall's face, a roof) is shadowed where the
//! ground under it (`rows_up` of its height below it) is, if the shadow there reaches its height:
//! a post's shadow runs along the ground to a wall's foot and climbs it, and never lies across
//! the wall's face as if it were ground.

use jane_core::angle::{cos_q15, sin_q15};

use crate::frame::{Caster, Directional, Rgb, SpriteCmd, height_of_rows, rows_up};

/// The longest a shadow gets, in heights: a sun this low casts no further (4 x 256; the sky
/// holds the sun and the moon at `light::LOWEST` for their shadows, where it is this, and fades
/// their strength out by 3 degrees).
pub const MAX_COT_Q8: i32 = 4 * 256;
/// How much of its strength a shadow keeps at its tip, of 256.
pub const TIP: i32 = 170;
/// A receiver at or under this height is the ground (a tuft, a cobble's relief): it takes the
/// shadow laid on it.
pub const GROUND: i32 = 4;
/// The terrain's relief at or under this height (a cobble's top, a tuft, a kerb) is its texture,
/// not a caster: it makes no block (`terrain::blocks`) and stands in no field on T2 (decided
/// 2026-09-27, so the three tiers cast from the same terrain; T2's cobbles lost a px of self-shade
/// at a low sun, and a town view went from four thousand blocks to four hundred).
pub const RELIEF: i32 = 8;
/// How long a shadow is per px of true height, Q8: the cotangent of the sun's elevation, no
/// more than [`MAX_COT_Q8`]; none with the sun down.
fn cot(sun: &Directional) -> Option<i32> {
    let (se, ce) = (sin_q15(sun.elevation).0, cos_q15(sun.elevation).0);
    if se <= 0 {
        return None;
    }
    Some((ce * 256 / se).min(MAX_COT_Q8))
}

/// The shadow's reach per px of true height, Q8, across the ground: away from the sun.
pub fn shear(sun: &Directional) -> Option<(i32, i32)> {
    let cot = cot(sun)?;
    let (ca, sa) = (cos_q15(sun.azimuth).0, sin_q15(sun.azimuth).0);
    Some((-(ca * cot) >> 15, -(sa * cot) >> 15))
}

/// **The fence rule** (decided 2026-09-28, redrawn 2026-09-29, the owner, PRESENTATION.md §1.7):
/// the sun is south all day, so every shadow runs up the screen, and a fence's or a hedge's lies
/// behind its own drawn rows where nothing sees it. A fence (its posts and rails, `Block::fence`)
/// and a low block (a hedge) are thrown by the sun mirrored into the north of the sky: the
/// shadow is as long as the true one and as far east or west, its north turned south (down the
/// screen, toward the viewer), and turned at least this far off the east-west line, so an
/// east-west fence's posts throw lines that stand apart rather than a hatch along the fence
/// (they had run 20 degrees off it, 2026-09-28, and every post's line lay over the next's).
/// Sin and cos of 40 degrees, Q15.
pub const SPILL_SIN_Q15: i32 = 21063;
pub const SPILL_COS_Q15: i32 = 25102;
/// A terrain block up to this height spills whatever its shape: a hedge (9 px), a kerb.
pub const SPILL_LOW: i32 = 10;

/// The shadow's reach per px of true height for what spills (the fence rule, [`SPILL_SIN_Q15`]):
/// [`shear`]'s length, mirrored south and turned at least 40 degrees off the east-west line.
pub fn spill_shear(sun: &Directional) -> Option<(i32, i32)> {
    let cot = cot(sun)?;
    let (ca, sa) = (cos_q15(sun.azimuth).0, sin_q15(sun.azimuth).0);
    // Away from the sun, a unit vector Q15; its north mirrored south.
    let (dx, dy) = (-ca, sa.abs());
    let (dx, dy) = if dy < SPILL_SIN_Q15 {
        (if dx < 0 { -SPILL_COS_Q15 } else { SPILL_COS_Q15 }, SPILL_SIN_Q15)
    } else {
        (dx, dy)
    };
    Some(((dx * cot) >> 15, (dy * cot) >> 15))
}

/// Whether block `b` is thrown by [`spill_shear`] rather than [`shear`]: a fence's part, or a
/// block no taller than [`SPILL_LOW`].
pub fn spills(b: &crate::frame::Block) -> bool {
    b.fence || i32::from(b.height) <= SPILL_LOW
}

/// A gap in a row of 2 px or more parts it into two runs: a lantern hung off its post, a hand
/// held out from the body. A single px (a dithered edge, a notch between leaves) does not: T2's
/// field keeps a column's whole height, and closes it with the rows over and under.
pub const GAP: usize = 2;

/// How many px wider than its one dither step a silhouette's edge is feathered for a sun or
/// moon of `spread` (`Directional::spread`): none under a high clear sun, one for the low sun of
/// five o'clock or seven, two at the horizon, three under cloud. T2's penumbra widens with the
/// spread and the distance from what casts; T0 and T1 widen the edge by this.
pub fn feather(spread: u16) -> i32 {
    let deg = |d: i32| d * 65536 / 360;
    match i32::from(spread) {
        s if s <= deg(2) => 0,
        s if s <= deg(4) => 1,
        s if s <= deg(6) => 2,
        _ => 3,
    }
}

/// A silhouette's multiplier `shade` laid at `strength` (of 255, `Directional::strength`): the
/// whole of it under a high clear sun, lighter for a low one, faint under cloud.
pub fn shade_at(shade: Rgb, strength: u8) -> Rgb {
    shade.map(|c| (255 - (255 - u32::from(c)) * u32::from(strength) / 255) as u8)
}

/// A caster's rows, from the foot up: `(rows above the foot, first, last)` px from the sprite's
/// left of each opaque run in the row, a row parted where it has a [`GAP`] (the contact shadow,
/// index 1, is not the silhouette), for the sprite `s` drawn from an atlas page's albedo
/// `albedo`, `page_w` wide, thrown as caster `c` (standing on its foot's row of the canvas; the
/// rows it burns, `Caster::burn`, left out: a flame casts nothing). Appended to `out`. So a run
/// that is not over another, lower one (a canopy past its trunk, a lamp's head past its post, a
/// lantern on its bracket) lays its shadow only where its own height throws it, apart from the
/// root's, as T2's field has it floating.
pub fn rows(albedo: &[u16], page_w: u16, s: &SpriteCmd, c: &Caster, out: &mut Vec<(i32, i32, i32)>) {
    let pw = usize::from(page_w);
    let sw = i32::from(s.src.w);
    let foot_y = i32::from(c.foot.1);
    let burns = |hv: i32| c.burn.0 > 0 && (i32::from(c.burn.0)..=i32::from(c.burn.1)).contains(&hv);
    for v in (0..i32::from(s.src.h)).rev() {
        let hv = foot_y - (i32::from(s.y) + v);
        if hv <= 0 || burns(hv) {
            continue;
        }
        if hv > 255 {
            break;
        }
        let start = (usize::from(s.src.y) + v as usize) * pw + usize::from(s.src.x);
        let Some(row) = albedo.get(start..start + usize::from(s.src.w)) else { continue };
        let mut run: Option<(usize, usize)> = None;
        let mut emit = |(first, last): (usize, usize)| {
            let (u0, u1) = if s.flags.mirror {
                (sw - 1 - last as i32, sw - 1 - first as i32)
            } else {
                (first as i32, last as i32)
            };
            out.push((hv, u0, u1));
        };
        for (u, _) in row.iter().enumerate().filter(|&(_, &i)| i > 1) {
            run = match run {
                Some((a, b)) if u - b <= GAP => Some((a, u)),
                Some(r) => {
                    emit(r);
                    Some((u, u))
                }
                None => Some((u, u)),
            };
        }
        if let Some(r) = run {
            emit(r);
        }
    }
}

/// One band of a shadow: `[x0, x1) x [y0, y1)` canvas px at `strength` (of 255), reaching
/// `reach` px up over the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
    pub strength: u8,
    pub reach: u8,
}

/// The bands of caster `c`'s shadow: its `rows` (from [`rows`]) sheared by `(kx, ky)`
/// ([`shear`]), for its sprite drawn with its left edge at `x`, each row its true height's shear
/// away and as thick as the caster is deep behind its foot row, as T2's field stands it.
///
/// There is no foot under it any more (decided 2026-09-28, the owner, measured against T2): an
/// ellipse wider than the feet laid under every caster at full strength (five rows, two px each
/// side; then three rows, one px) read on T0 and T1 as a blob under everything beside T2, whose
/// only foot is its field (the rows behind the foot) and the painted contact shadow (index 1),
/// which every tier draws the same. The lowest rows' own bands start at the foot row, so the
/// shadow still grows out of the feet.
pub fn bands(rows: &[(i32, i32, i32)], x: i32, c: &Caster, k: (i32, i32), emit: impl FnMut(Band)) {
    let Some(&(top, _, _)) = rows.last() else { return };
    bands_of_rows(rows, x, c, k, top, emit);
}

/// The rows' bands of [`bands`]: each row its true height's shear away, stretched
/// to meet the row above, as thick as the caster is deep.
fn bands_of_rows(
    rows: &[(i32, i32, i32)],
    x: i32,
    c: &Caster,
    (kx, ky): (i32, i32),
    top: i32,
    mut emit: impl FnMut(Band),
) {
    let fy = i32::from(c.foot.1);
    let depth = i32::from(c.depth).max(2);
    let tall = i32::from(c.height).max(1);
    let up = |hv: i32| height_of_rows(hv).min(tall);
    let htop = up(top);
    for &(hv, u0, u1) in rows {
        // The row `hv` over the foot row spans its bottom to its top: the lowest drawn row from
        // the ground, so its shadow starts at the feet whatever way the sun lies.
        let (h0, h1) = (up(hv - 1), up(hv));
        let (ax, bx) = ((h0 * kx) >> 8, (h1 * kx) >> 8);
        let (ay, by) = ((h0 * ky) >> 8, (h1 * ky) >> 8);
        let strength = 256 - (256 - TIP) * hv.min(top) / top;
        // No deeper than it is wide (T2's field: its middle column's `2 inset + 2`): a post
        // 2 px wide throws a shadow 2 px thick whichever way it falls.
        let deep = depth.min(2 * ((u1 - u0) / 2) + 2);
        emit(Band {
            x0: x + u0 + ax.min(bx),
            x1: x + u1 + 1 + ax.max(bx),
            y0: fy + ay.min(by) - deep + 1,
            y1: fy + ay.max(by) + 1,
            strength: strength.clamp(1, 255) as u8,
            reach: (htop - h0).clamp(1, 255) as u8,
        });
    }
}

/// Rows in front of its ground point a lifted receiver looks its shadow up (T2's `front` in
/// `light.wgsl`, T1's for the terrain): a face stands on its foot row and the rows of the
/// terrain behind it, so the wall it is the face of never shadows it, where another's shadow on
/// the ground at its foot does.
pub const FRONT: i32 = 2;

/// Where a receiver `h` px up at canvas row `y` takes its shadow from: the ground row under it
/// ([`FRONT`] rows in front of its foot) and the reach the shadow there must have (the ground
/// takes any).
pub fn ground_of(y: i32, h: u8) -> (i32, u8) {
    let h = i32::from(h);
    if h <= GROUND { (y, 0) } else { (y + rows_up(h) + FRONT, h as u8) }
}

/// How far under a block's top its shadow reaches over its own footprint and past it, px: a
/// block stands the tallest of heights up to `terrain::BLOCK_TOLERANCE` apart, and a roof's px
/// looks its shadow up two rows toward the viewer ([`FRONT`]), where the next course stands a px
/// higher. So a roof never shadows itself, where T2 traces each px from just over its own height.
const OWN_TOP: i32 = crate::terrain::BLOCK_TOLERANCE as i32 + 2;

/// How far a block's shadow moves from one slice of its height to the next at most, px, and
/// how tall a slice is at most: what the reach up a wall it falls on steps by.
const SLICE: i32 = 8;

/// The bands of block `b`'s shadow in a sun sheared `k` ([`shear`]; a fence's part or a low
/// block takes `ks`, [`spill_shear`], the fence rule).
///
/// A block's true shadow is its footprint laid at every slice of its height, each slice
/// stretched to meet the next (the rows' rule, [`bands`]), so the whole is the footprint swept
/// along the sun by its height, as T2's field throws it. A slice reaches as high as the ray over
/// the block's top there, less [`OWN_TOP`]. Its foot is its footprint (the terrain's own contact
/// shade is painted); it keeps its whole strength to the tip, as a large thing's umbra does (a
/// thin thing's fades, [`TIP`]). Each slice is laid only where the slice under it did not lie:
/// the mask keeps the strongest and the highest, the strength is one and the reach falls slice
/// by slice, so what is left out is what the lower slice had already laid higher.
///
/// A spilling block is its footprint swept from its bottom (`Block::lo`: a rail floats, a post
/// stands from the ground) to its top along `ks`, exactly, a band a row ([`spill_rows`]): a post
/// throws a line from its foot, each rail a line of its own with lit ground under and between
/// them. It reaches no higher than the ground: it lies on the ground alone, and never on the
/// fence nor the hedge that throws it (every tier; a fence's px stand at least
/// `jane_art::terrain::FENCE_FLOOR` up, over [`GROUND`]).
pub fn block_bands(b: &crate::frame::Block, k: (i32, i32), ks: (i32, i32), mut emit: impl FnMut(Band)) {
    let hgt = i32::from(b.height);
    let (x0, y0, x1, y1) = (i32::from(b.x0), i32::from(b.y0), i32::from(b.x1), i32::from(b.y1));
    if hgt <= GROUND || x0 >= x1 || y0 >= y1 {
        return;
    }
    if spills(b) {
        // A block of the height field stands a px wider each side than what is drawn (for T2's
        // steps, `terrain::block`); a fence's parts are as drawn.
        let (x0, x1) = if !b.fence && x1 - x0 > 2 { (x0 + 1, x1 - 1) } else { (x0, x1) };
        spill_rows((x0, y0, x1, y1), (i32::from(b.lo).min(hgt), hgt), ks, emit);
        return;
    }
    let (kx, ky) = k;
    let far = (hgt * kx.abs().max(ky.abs())) >> 8;
    let n = ((hgt + SLICE - 1) / SLICE).max((far + SLICE - 1) / SLICE).max(1);
    let mut under: Option<(i32, i32, i32, i32)> = None;
    for i in 0..n {
        let (h0, h1) = (hgt * i / n, hgt * (i + 1) / n);
        let (ax, bx) = ((h0 * kx) >> 8, (h1 * kx) >> 8);
        let (ay, by) = ((h0 * ky) >> 8, (h1 * ky) >> 8);
        let r = (x0 + ax.min(bx), y0 + ay.min(by), x1 + ax.max(bx), y1 + ay.max(by));
        let reach = (hgt - h1 - OWN_TOP).clamp(1, 255) as u8;
        minus(r, under, |(x0, y0, x1, y1)| emit(Band { x0, x1, y0, y1, strength: 255, reach }));
        under = Some(r);
    }
}

/// The footprint `(x0, y0, x1, y1)` swept along `(kx, ky)` (Q8 px a px of height) from `lo` to
/// `hi` px up, exactly: a band a row, each the px whose middles the swept box covers there, at
/// full strength, on the ground alone (reach 1). Rows of the same span are one band. So a thin
/// post's shadow is a line as thick as the post whatever its slant (slices of it, boxes along a
/// diagonal, had filled in from one post's line to the next).
pub fn spill_rows(
    (x0, y0, x1, y1): (i32, i32, i32, i32),
    (lo, hi): (i32, i32),
    (kx, ky): (i32, i32),
    mut emit: impl FnMut(Band),
) {
    if x0 >= x1 || y0 >= y1 || hi <= lo {
        return;
    }
    // The box's offset (Q8 px) at its bottom and over its height.
    let (ax, ay) = (lo * kx, lo * ky);
    let (dx, dy) = ((hi - lo) * kx, (hi - lo) * ky);
    let (omin, omax) = (ay.min(ay + dy), ay.max(ay + dy));
    let first = y0 + (omin >> 8) - 1;
    let last = y1 + ((omax + 255) >> 8) + 1;
    let mut open: Option<Band> = None;
    for y in first..=last {
        // The box's offsets whose rows cover this row's middle: `y0 + oy <= Y < y1 + oy`.
        let yc = y * 256 + 128;
        let (p, q) = (omin.max(yc - y1 * 256 + 1), omax.min(yc - y0 * 256));
        if p > q {
            continue;
        }
        // And the east-west offsets those take.
        let (ox0, ox1) = if dy == 0 {
            (ax.min(ax + dx), ax.max(ax + dx))
        } else {
            let at = |o: i32| ax + ((i64::from(o - ay) * i64::from(dx)) / i64::from(dy)) as i32;
            (at(p).min(at(q)), at(p).max(at(q)))
        };
        // The px whose middles lie in `[x0 + ox0, x1 + ox1)`.
        let xa = (x0 * 256 + ox0 - 128 + 255).div_euclid(256);
        let xb = (x1 * 256 + ox1 - 128 + 255).div_euclid(256);
        if xa >= xb {
            continue;
        }
        let band = Band { x0: xa, x1: xb, y0: y, y1: y + 1, strength: 255, reach: 1 };
        open = match open {
            Some(o) if o.x0 == xa && o.x1 == xb && o.y1 == y => Some(Band { y1: y + 1, ..o }),
            Some(o) => {
                emit(o);
                Some(band)
            }
            None => Some(band),
        };
    }
    if let Some(o) = open {
        emit(o);
    }
}

/// The rect `r` less the rect `a`, as up to four rects `(x0, y0, x1, y1)`.
fn minus(r: (i32, i32, i32, i32), a: Option<(i32, i32, i32, i32)>, mut emit: impl FnMut((i32, i32, i32, i32))) {
    let Some(a) = a.filter(|a| a.0 < r.2 && r.0 < a.2 && a.1 < r.3 && r.1 < a.3) else {
        emit(r);
        return;
    };
    let (my0, my1) = (r.1.max(a.1), r.3.min(a.3));
    if r.1 < my0 {
        emit((r.0, r.1, r.2, my0));
    }
    if my1 < r.3 {
        emit((r.0, my1, r.2, r.3));
    }
    if r.0 < a.0 {
        emit((r.0, my0, a.0, my1));
    }
    if a.2 < r.2 {
        emit((a.2, my0, r.2, my1));
    }
}

/// How much of a point light a shadow from it keeps, of 256 (every tier: T2's `LAMP_BOUNCE`, T1's
/// light pass, T0's `pointshadow`): its pool's light bounces into its umbra, so a lamp's shadow
/// is a deep dusk, not a hole (decided 2026-09-28, the owner: shadows read harsh on T2).
pub const LAMP_BOUNCE: u32 = 31;

/// Sub-px steps a canvas px of a point light's shadow geometry ([`Lamp`], [`Slab`]).
pub const SUB: i32 = 16;
/// A point light's shadow reaches this far past its radius, px, then stops.
pub const SHADOW_PAST: i32 = 24;
/// A caster whose foot is further than this past a light's radius throws none of its shadow.
const CAST_PAST: i32 = 48;

/// A point light as its shadow's geometry sees it (T0 and T1, PRESENTATION.md §1.7): its ground
/// point in [`SUB`] steps of a px (the middle of its px), its height and its radius, px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lamp {
    pub x: i32,
    pub y: i32,
    pub h: i32,
    pub r: i32,
}

impl Lamp {
    pub fn of(l: &crate::frame::Light) -> Lamp {
        Lamp {
            x: l.pos.0 * SUB + SUB / 2,
            y: l.pos.1 * SUB + SUB / 2,
            h: i32::from(l.height.max(1)),
            r: i32::from(l.radius),
        }
    }
}

/// A quad of a point light's shadow on the ground: its corners in [`SUB`] steps of a px, in the
/// order a triangle strip takes them (`a0, b0, a1, b1`: the slab's foot, then its top), each with
/// how high the shadow reaches over that ground point, px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slab {
    pub c: [(i32, i32, i32); 4],
}

/// A point `(px, py)` ([`SUB`] steps) `z` px up, seen from `lamp`, laid on the ground, with the
/// reach of the ray over a caster `top` px tall there: `t = d * h / (h - z)` from the light (a
/// point at or over the light's height reaches the rim plus [`SHADOW_PAST`] and stops), the reach
/// `h + (top - h) * t / d`.
fn project(lamp: &Lamp, (px, py): (i32, i32), z: i32, top: i32) -> (i32, i32, i32) {
    let (dx, dy) = (i64::from(px - lamp.x), i64::from(py - lamp.y));
    let d = i64::from(jane_core::num::isqrt((dx * dx + dy * dy) as u64)).max(i64::from(SUB / 2));
    let far = i64::from((lamp.r + SHADOW_PAST) * SUB);
    let (h, z) = (i64::from(lamp.h), i64::from(z));
    let t = if 2 * z >= 2 * h - 1 { far } else { (d * h / (h - z)).min(far) };
    let reach = (h + (i64::from(top) - h) * t / d).clamp(0, 255);
    ((i64::from(lamp.x) + dx * t / d) as i32, (i64::from(lamp.y) + dy * t / d) as i32, reach as i32)
}

/// The slab of a vertical face standing on the ground from `a` to `b` ([`SUB`] steps), from
/// `za` px up to `zb`, seen from `lamp`, for a caster `top` px tall.
fn slab(lamp: &Lamp, a: (i32, i32), b: (i32, i32), (za, zb): (i32, i32), top: i32) -> Slab {
    Slab {
        c: [project(lamp, a, za, top), project(lamp, b, za, top), project(lamp, a, zb, top), project(lamp, b, zb, top)],
    }
}

/// Caster `c`'s shadow from `lamp` (T0 and T1, PRESENTATION.md §1.7): each run of equal rows of
/// its sprite (`rows`, from [`rows`], its left edge at `x`), a vertical slab at the front and at
/// the back of its footprint (the silhouettes' footprint, [`bands`]), projected from the light
/// onto the ground. Nothing when its foot is well past the light's reach.
pub fn row_slabs(rows: &[(i32, i32, i32)], x: i32, c: &Caster, lamp: &Lamp, mut emit: impl FnMut(Slab)) {
    if !reaches(c, lamp) {
        return;
    }
    let fy = i32::from(c.foot.1) * SUB + SUB / 2;
    let depth = i32::from(c.depth.max(2));
    let tall = i32::from(c.height).max(1);
    let up = |hv: i32| height_of_rows(hv).min(tall);
    let top = rows.last().map_or(1, |r| up(r.0));
    let mut k = 0;
    while k < rows.len() {
        let (h0, u0, u1) = rows[k];
        let mut j = k + 1;
        while j < rows.len() && rows[j].1 == u0 && rows[j].2 == u1 && rows[j].0 == rows[j - 1].0 + 1 {
            j += 1;
        }
        let h1 = rows[j - 1].0 + 1;
        k = j;
        let (xa, xb) = ((x + u0) * SUB, (x + u1 + 1) * SUB);
        let deep = depth.min(2 * ((u1 - u0) / 2) + 2);
        for by in [fy + SUB / 2 - deep * SUB, fy + SUB / 2] {
            emit(slab(lamp, (xa, by), (xb, by), (up(h0 - 1), up(h1)), top));
        }
    }
}

/// Whether caster `c`'s foot lies near enough `lamp` for it to throw any of its shadow
/// ([`row_slabs`] throws none past it).
pub fn reaches(c: &Caster, lamp: &Lamp) -> bool {
    let (fx, fy) = (i32::from(c.foot.0) * SUB + SUB / 2, i32::from(c.foot.1) * SUB + SUB / 2);
    let (dx, dy) = (i64::from(fx - lamp.x), i64::from(fy - lamp.y));
    let near = i64::from((lamp.r + CAST_PAST) * SUB);
    dx * dx + dy * dy <= near * near
}

/// Block `b`'s shadow from `lamp`: each of its sides turned away from the light a slab from its
/// bottom (`Block::lo`: a rail floats) to its height, projected from the light onto the ground (a side higher than the light
/// reaches the rim); the sides turned to it throw nothing past those (a block is a box). Nothing
/// when it lies well past the light's reach.
pub fn block_slabs(b: &crate::frame::Block, lamp: &Lamp, mut emit: impl FnMut(Slab)) {
    let (x0, y0, x1, y1) = (i32::from(b.x0) * SUB, i32::from(b.y0) * SUB, i32::from(b.x1) * SUB, i32::from(b.y1) * SUB);
    let hgt = i32::from(b.height);
    if hgt <= GROUND {
        return;
    }
    let (nx, ny) = (lamp.x.clamp(x0, x1), lamp.y.clamp(y0, y1));
    let (dx, dy) = (i64::from(nx - lamp.x), i64::from(ny - lamp.y));
    let near = i64::from((lamp.r + CAST_PAST) * SUB);
    if dx * dx + dy * dy > near * near {
        return;
    }
    let sides = [
        (lamp.y > y0, (x0, y0), (x1, y0)),
        (lamp.y < y1, (x0, y1), (x1, y1)),
        (lamp.x > x0, (x0, y0), (x0, y1)),
        (lamp.x < x1, (x1, y0), (x1, y1)),
    ];
    for (away, a, c) in sides {
        if away {
            emit(slab(lamp, a, c, (i32::from(b.lo).min(hgt), hgt), hgt));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Flags, Src};
    use jane_core::Angle;

    fn sun(azimuth: Angle, deg: i32) -> Directional {
        Directional { azimuth, elevation: Angle::from_degrees(deg), colour: [255; 3], spread: 0, strength: 255 }
    }

    #[test]
    fn a_low_western_sun_lays_shadows_long_to_the_east() {
        let (kx, ky) = shear(&sun(Angle::WEST, 20)).unwrap();
        // cot 20 degrees is 2.75: east, level.
        assert!((kx - 704).abs() < 8 && ky.abs() < 4, "{kx} {ky}");
        let (kx, ky) = shear(&sun(Angle::SOUTH, 45)).unwrap();
        assert!(kx.abs() < 4 && (ky + 256).abs() < 4, "{kx} {ky}");
        assert!(shear(&sun(Angle::WEST, 0)).is_none());
    }

    /// A 4 x 20 post of index 2 over a contact shadow row, standing on canvas row 30.
    fn post() -> (Vec<u16>, SpriteCmd, Caster) {
        let mut albedo = vec![2u16; 4 * 21];
        albedo[80..84].fill(1);
        let s = SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 4, h: 21 },
            x: 20,
            y: 11,
            flags: Flags::default(),
            height_px: 25,
        };
        (albedo, s, Caster { sprite: 0, foot: (22, 31), height: 25, depth: 4, ..Caster::default() })
    }

    #[test]
    fn the_shadow_grows_out_of_the_feet_whatever_the_hour() {
        let (albedo, s, c) = post();
        let mut r = Vec::new();
        rows(&albedo, 4, &s, &c, &mut r);
        // The contact shadow's row is not the silhouette; the first row is the one on the foot.
        assert_eq!(r.first(), Some(&(1, 0, 3)));
        assert_eq!(r.last().map(|r| r.0), Some(20));
        for (az, el) in [(Angle::WEST, 15), (Angle::SOUTH, 46), (Angle::EAST, 30), (Angle::NORTH, 20)] {
            let k = shear(&sun(az, el)).unwrap();
            let mut bands = Vec::new();
            super::bands(&r, i32::from(s.x), &c, k, |b| bands.push(b));
            // The feet's own px on the foot row are in it, and nothing is laid round them: no
            // foot two px past the feet or rows under them, as on T2.
            let fy = i32::from(c.foot.1);
            let covers = |x: i32, y: i32| bands.iter().any(|b| b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1);
            for x in 20..24 {
                assert!(covers(x, fy), "{az:?} {el}: ({x}, {fy})");
            }
            // A sun that throws its shadow up the screen or across it lays nothing under them.
            if az != Angle::NORTH {
                assert!(!covers(20, fy + 2) && !covers(23, fy + 2), "{az:?} {el}: a foot under the feet");
            }
        }
    }

    #[test]
    fn a_shadow_reaches_up_as_high_as_the_ray_over_the_top() {
        let (albedo, s, c) = post();
        let mut r = Vec::new();
        rows(&albedo, 4, &s, &c, &mut r);
        let mut bands = Vec::new();
        super::bands(&r, i32::from(s.x), &c, shear(&sun(Angle::WEST, 20)).unwrap(), |b| bands.push(b));
        let rows_only = &bands[..];
        // At the root it reaches nearly the post's height (25 px); at the tip, nothing.
        assert!(rows_only[0].reach >= 23, "{:?}", rows_only[0]);
        assert!(rows_only.last().unwrap().reach <= 2);
        assert!(rows_only.windows(2).all(|w| w[0].reach >= w[1].reach));
        // A 20-row post is 25 px tall: its shadow is that times cot 20 degrees, 2.75, long,
        // past its right edge.
        let far = rows_only.iter().map(|b| b.x1).max().unwrap();
        assert_eq!(far, 24 + ((25 * 704) >> 8), "{far}");
        // A face 10 px up takes its shadow from the ground 8 rows under it and 2 in front of that,
        // if it reaches 10.
        assert_eq!(ground_of(50, 10), (60, 10));
        assert_eq!(ground_of(50, 3), (50, 0));
    }

    /// A 40 x 58 sprite of index 2 where `shape` says, over a contact shadow row, its foot on
    /// row 56, standing on canvas `(120, 100)`; and its bands in a sun in the north at 40
    /// degrees, so its shadow runs down the screen.
    fn cast(shape: impl Fn(i32, i32) -> bool) -> (Vec<Band>, (i32, i32)) {
        let (w, h) = (40usize, 58usize);
        let mut albedo = vec![0u16; w * h];
        for y in 0..h {
            for x in 0..w {
                if shape(x as i32, y as i32) {
                    albedo[y * w + x] = 2;
                }
            }
        }
        albedo[57 * w + 16..57 * w + 24].fill(1);
        let foot = (120, 100);
        let s = SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: w as u16, h: h as u16 },
            x: (foot.0 - 20) as i16,
            y: (foot.1 - 56) as i16,
            flags: Flags::default(),
            height_px: 70,
        };
        let c = Caster { sprite: 0, foot: (foot.0 as i16, foot.1 as i16), height: 70, depth: 6, ..Caster::default() };
        let mut r = Vec::new();
        rows(&albedo, w as u16, &s, &c, &mut r);
        let mut bands = Vec::new();
        super::bands(&r, i32::from(s.x), &c, shear(&sun(Angle::NORTH, 40)).unwrap(), |b| bands.push(b));
        (bands, foot)
    }

    fn covered(bands: &[Band], y: i32) -> Vec<i32> {
        (0..400).filter(|&x| bands.iter().any(|b| b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1)).collect()
    }

    #[test]
    fn a_trees_shadow_is_its_trunks_at_the_root_and_its_crowns_further_out() {
        // A crown 32 wide from row 4 to row 30 (33 px to 65 px up) on a trunk 4 wide.
        let (bands, foot) =
            cast(|x, y| ((4..=30).contains(&y) && (4..36).contains(&x)) || (y > 30 && (18..22).contains(&x)));
        // cot 40 degrees is 1.19: the trunk's shadow is the 39 rows south of the foot, and near
        // the root it is the trunk, its foot two px wider each side, and no wider.
        for dy in 4..30 {
            let n = covered(&bands, foot.1 + dy).len();
            assert!((3..=8).contains(&n), "{dy} rows south of the root: {n} px, the crown's shadow stands on the root");
        }
        for dy in 50..70 {
            let n = covered(&bands, foot.1 + dy).len();
            assert!(n >= 32, "{dy} rows south of the root: {n} px, no crown's shadow");
        }
    }

    #[test]
    fn a_lantern_hung_off_its_post_throws_its_shadow_apart_from_the_posts() {
        // A post 2 wide, an arm east along rows 8 and 9, a lantern 8 wide hanging from it, rows 10
        // to 22, 8 px clear of the post.
        let (bands, foot) = cast(|x, y| {
            (y >= 8 && (10..12).contains(&x))
                || ((8..=9).contains(&y) && (10..28).contains(&x))
                || ((10..=22).contains(&y) && (20..28).contains(&x))
        });
        let x0 = foot.0 - 20;
        // Near the root the post's alone; under where the lantern hangs, nothing.
        for dy in 4..30 {
            let c = covered(&bands, foot.1 + dy);
            assert!(
                !c.iter().any(|x| (x0 + 20..x0 + 28).contains(x)),
                "{dy} rows south: the lantern's shadow is on the root: {c:?}"
            );
        }
        // Further out the lantern's, and between it and the post's, the ground.
        let c = covered(&bands, foot.1 + 58);
        assert!((x0 + 20..x0 + 28).all(|x| c.contains(&x)), "no lantern's shadow: {c:?}");
        assert!((x0 + 13..x0 + 19).any(|x| !c.contains(&x)), "the lantern's shadow runs into the post's: {c:?}");
    }

    #[test]
    fn what_flies_has_no_foot() {
        // A bat: a body 12 wide from row 30 to row 40, 20 rows over its anchor on row 56.
        let (bands, foot) = cast(|x, y| (30..=40).contains(&y) && (14..26).contains(&x));
        // Nothing under it: the nearest band is its lowest row's, from the bottom of that row, 15
        // rows' height (19 px) times 1.19 away (22 rows), its depth (6 rows) behind that.
        let near = bands.iter().map(|b| b.y0).min().unwrap();
        assert!(near >= foot.1 + 16, "a shadow at its anchor: {near}");
    }

    #[test]
    fn a_blocks_shadow_is_its_footprint_swept_along_the_sun_by_its_height() {
        let b = crate::frame::Block { x0: 100, y0: 100, x1: 180, y1: 140, height: 60, ..Default::default() };
        let k = shear(&sun(Angle::WEST, 20)).unwrap();
        let mut bands = Vec::new();
        block_bands(&b, k, k, |band| bands.push(band));
        let at = |x: i32, y: i32| {
            bands.iter().filter(|b| b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1).map(|b| b.reach).max()
        };
        // 60 px at cot 20 degrees (2.75) is 165 px east of its east wall: covered to there, and no
        // further; the footprint itself too, but never as high as the block's own top.
        let far = 180 + ((60 * k.0) >> 8);
        assert!(at(far - 2, 120).is_some() && at(far + 2, 120).is_none(), "{far}");
        assert!(at(140, 120).is_some_and(|r| i32::from(r) < 60 - crate::terrain::BLOCK_TOLERANCE as i32));
        // The reach falls along the shadow, as the ray over its top does.
        let (near, mid) = (at(185, 120).unwrap(), at(260, 120).unwrap());
        assert!(near > mid && mid > 1, "{near} {mid}");
        // West of it, nothing; and no px is laid twice by the slices.
        assert!(at(95, 120).is_none());
        let area: i32 = bands.iter().map(|b| (b.x1 - b.x0) * (b.y1 - b.y0)).sum();
        assert!(area <= (far - 100) * 41, "{area}");
    }

    /// The sun at five (15 degrees south of west, 16 up: `light::sky`'s) and noon (a little west
    /// of south, 46 up).
    fn five() -> Directional {
        sun(Angle(32768 - 2730), 16)
    }
    fn noon() -> Directional {
        sun(Angle(16384 + 1500), 46)
    }

    /// An east-west run of `n` fence cells as the painter draws it, foot row `99`, its first
    /// cell's left edge at x 96: its posts and rails (`jane_art::terrain::fence_parts`).
    fn fence_run(n: i32) -> Vec<crate::frame::Block> {
        let mut out = Vec::new();
        for i in 0..n {
            let nb = (i > 0, i < n - 1, false, false);
            jane_art::terrain::fence_parts((96 + 16 * i, 84), nb, |f| {
                out.push(crate::frame::Block {
                    x0: f.x0,
                    y0: f.y0,
                    x1: f.x1,
                    y1: f.y1,
                    height: f.hi,
                    lo: f.lo,
                    fence: true,
                });
            });
        }
        out
    }

    fn laid(blocks: &[crate::frame::Block], s: &Directional) -> Vec<Band> {
        let (k, ks) = (shear(s).unwrap(), spill_shear(s).unwrap());
        let mut bands = Vec::new();
        for b in blocks {
            block_bands(b, k, ks, |band| bands.push(band));
        }
        bands
    }

    fn at(bands: &[Band], x: i32, y: i32) -> bool {
        bands.iter().any(|b| b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1)
    }

    #[test]
    fn a_fence_spills_south_as_long_as_the_true_shadow_and_a_wall_does_not() {
        for s in [five(), noon(), sun(Angle(16384 - 12000), 20), sun(Angle::WEST, 14)] {
            let (k, ks) = (shear(&s).unwrap(), spill_shear(&s).unwrap());
            // As long as the true shadow (within a px in 256), toward the viewer, as far east or
            // west, and at least 40 degrees off the east-west line.
            let len = |(x, y): (i32, i32)| jane_core::num::isqrt((x * x + y * y) as u64) as i32;
            assert!((len(k) - len(ks)).abs() <= 2, "{k:?} {ks:?}");
            assert!(ks.1 > 0 && ks.0.signum() == k.0.signum(), "{k:?} {ks:?}");
            assert!(ks.1 * SPILL_COS_Q15 >= ks.0.abs() * SPILL_SIN_Q15 - (1 << 16), "{ks:?}");
        }
        // At five: east, turned 40 degrees south. At noon: south, as long as the true one north.
        let (k, ks) = (shear(&noon()).unwrap(), spill_shear(&noon()).unwrap());
        assert!(k.1 < 0 && (ks.1 + k.1).abs() <= 2 && ks.0 == k.0, "{k:?} {ks:?}");
        // A wall at five keeps the true sun: nothing south of its foot.
        let wall = crate::frame::Block { x0: 100, y0: 40, x1: 196, y1: 96, height: 13, ..Default::default() };
        assert!(!spills(&wall));
        assert!(laid(&[wall], &five()).iter().all(|b| b.y1 <= 96));
        // A hedge is low and spills; a house does not.
        assert!(spills(&crate::frame::Block { x0: 100, y0: 80, x1: 120, y1: 100, height: 9, ..Default::default() }));
        assert!(!spills(&crate::frame::Block { x0: 100, y0: 40, x1: 180, y1: 100, height: 60, ..Default::default() }));
    }

    #[test]
    fn a_fences_shadow_is_its_posts_rooted_at_their_feet_and_two_rails_on_the_ground_alone() {
        let run = fence_run(6);
        for s in [five(), noon(), sun(Angle(16384 - 12000), 20)] {
            let bands = laid(&run, &s);
            // On the ground alone: never on the fence it is thrown by.
            assert!(bands.iter().all(|b| b.reach == 1 && b.strength == 255));
            let ks = spill_shear(&s).unwrap();
            // Each post's line starts at its foot, under the post (x 101..107 for the second
            // cell's, 112 + 5), and runs away along the spill: the row two under the foot is
            // covered under or beside the post.
            for i in 1..5 {
                let px = 96 + 16 * i + 8;
                assert!((px - 4..px + 4).any(|x| at(&bands, x, 100)), "{ks:?}: post {i} not rooted");
            }
            // Down a column between two posts, the ground is lit under the lower rail's line,
            // between the two rails' lines, and past the upper one's, each rail a line.
            let row = |h: i32| 99 + ((h * ks.1) >> 8);
            let x = |h: i32| 96 + 16 * 2 + 1 + ((h * ks.0) >> 8);
            let lower = (7..=11).any(|h| at(&bands, x(h), row(h)));
            let upper = (17..=21).any(|h| at(&bands, x(h), row(h)));
            assert!(lower && upper, "{ks:?}: a rail throws no line");
            assert!(!at(&bands, x(3), row(3)), "{ks:?}: lit under the lower rail");
            assert!(!at(&bands, x(14), row(14)), "{ks:?}: lit between the rails");
        }
        // At five the posts' lines stand apart: every row of the spill off the rails' lines (2.2
        // rows a px of height: the lower rail's rows 114 to 124, the upper's 137 to 147) has lit
        // ground between one post's line and the next's (16 px apart, each no wider than 10).
        let bands = laid(&run, &five());
        for y in (101..113).chain(126..136) {
            let lit = (120..180).filter(|&x| !at(&bands, x, y)).count();
            assert!(lit >= 12, "row {y}: {lit} px of 60 lit, the posts' lines run into a slab");
        }
    }

    #[test]
    fn a_spill_row_is_the_px_whose_middles_the_swept_box_covers() {
        // A box 6 x 2 at (100, 98) swept 16 px east and 16 south from 0 to 16 px up (a shear of
        // one px a px each way): a diagonal line 6 px wide and 2 rows deep at its root.
        let mut bands = Vec::new();
        spill_rows((100, 98, 106, 100), (0, 16), (256, 256), |b| bands.push(b));
        assert!(at(&bands, 100, 98) && at(&bands, 105, 99) && !at(&bands, 99, 98) && !at(&bands, 106, 98));
        for y in 98..116 {
            let xs: Vec<i32> = (80..140).filter(|&x| at(&bands, x, y)).collect();
            assert!((6..=8).contains(&xs.len()), "row {y}: {xs:?}");
        }
        assert!(!(80..140).any(|x| at(&bands, x, 116)));
        // Floating from 8 px: nothing near the root.
        let mut bands = Vec::new();
        spill_rows((100, 98, 106, 100), (8, 16), (256, 256), |b| bands.push(b));
        assert!(!(80..140).any(|x| at(&bands, x, 100)) && at(&bands, 110, 107));
    }

    #[test]
    fn a_blocks_sides_turned_to_a_lamp_throw_nothing_and_the_rest_reach_the_rim() {
        let b = crate::frame::Block { x0: 100, y0: 100, x1: 180, y1: 108, height: 60, ..Default::default() };
        let lamp = Lamp { x: 140 * SUB + 8, y: 130 * SUB + 8, h: 30, r: 100 };
        let mut slabs = Vec::new();
        block_slabs(&b, &lamp, |q| slabs.push(q));
        // South of it: its north side and its two ends turn away; its south side faces the lamp.
        assert_eq!(slabs.len(), 3);
        // Taller than the lamp, its top reaches the rim plus the past, away from the lamp; its
        // foot is where it stands, and there the shadow reaches as high as the block.
        for q in &slabs {
            for (x, y, r) in q.c {
                let (dx, dy) = (i64::from(x - lamp.x), i64::from(y - lamp.y));
                let d = jane_core::num::isqrt((dx * dx + dy * dy) as u64) as i32 / SUB;
                assert!(y <= 108 * SUB, "a corner south of the block: {}", y / SUB);
                assert!((d - 100 - SHADOW_PAST).abs() <= 1 || r == 60, "a corner {d} px out reaching {r}");
            }
        }
    }
}
