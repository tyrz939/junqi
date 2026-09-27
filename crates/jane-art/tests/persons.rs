//! ART.md §5's acceptance tests for the people (§8 step 2): coverage, geometry, determinism,
//! layers, outline closed, colour budget, silhouette distinct, dead frames, and the seat swaps.
//! The goldens are `tests/golden.rs` (every set's hash is in `tests/golden.txt`).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use jane_art::Canvas;
use jane_art::looks::{self, Rendered};
use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_art::person::{self, AX, AY, H, W};
use jane_art::sprite::{FrameId, Role};
use jane_data::{Controller, Look, catalog};

fn all() -> &'static [Rendered] {
    static ALL: OnceLock<Vec<Rendered>> = OnceLock::new();
    ALL.get_or_init(|| looks::family(looks::Family::Person).expect("every look renders"))
}

/// The seat-0, variant-0 set of every look.
fn bases() -> impl Iterator<Item = &'static Rendered> {
    all().iter().filter(|r| r.seat == 0 && r.variant == 0)
}

fn frames(r: &Rendered) -> impl Iterator<Item = (FrameId, &Canvas)> {
    r.set.frames.iter().map(|(f, c)| (*f, c))
}

/// Opaque pixels' bounds: `(x0, y0, x1, y1)` inclusive.
fn drawn(c: &Canvas) -> (i32, i32, i32, i32) {
    let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in 0..c.h() {
        for x in 0..c.w() {
            if c.get(x, y).is_opaque() {
                b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
            }
        }
    }
    b
}

/// A shade (`ghost`, ART.md §2.1): it comes apart below the hip in a checker, stands on no
/// feet, draws no closed line there, and stands half its height. The tests of a solid body
/// skip it where those are the look, and [`a_shade_comes_apart_and_stands_half_height`] holds
/// it to its own.
fn ghost(r: &Rendered) -> bool {
    matches!(looks::find(r.name), Some((_, Look::Person(p))) if p.ghost)
}

/// A creature, not a person: these unit sprites are step 4's (ART.md §8).
fn is_creature(sprite: &str) -> bool {
    ["cat", "hen", "sheep"].iter().any(|k| sprite.contains(k))
}

#[test]
fn coverage_jane_and_the_townsfolk_have_looks() {
    let c = catalog();
    let mut missing = BTreeSet::new();
    for u in c.combat.units {
        let name = c.sprites[usize::from(u.sprite.0)];
        let person_row = u.controller == Controller::Player
            || (["town_", "folk_", "villager_"].iter().any(|p| name.starts_with(p)) && !is_creature(name));
        if person_row && looks::find(name).is_none() {
            missing.insert(name);
        }
    }
    assert!(missing.is_empty(), "sprites with no look: {missing:?}");
}

#[test]
fn coverage_every_unit_sprite_has_a_look() {
    let c = catalog();
    let mut missing = BTreeSet::new();
    for u in c.combat.units {
        let name = c.sprites[usize::from(u.sprite.0)];
        if looks::find(name).is_none() {
            missing.insert(name);
        }
    }
    assert!(missing.is_empty(), "sprites with no look: {missing:?}");
}

#[test]
fn coverage_every_look_renders_every_frame_it_promises() {
    let want = |r: &Rendered| person::frame_ids_for(looks::fight(r.sprite));
    for r in all() {
        let got: Vec<FrameId> = r.set.frames.iter().map(|(f, _)| *f).collect();
        assert_eq!(got, want(r), "{}", r.key());
    }
    for (id, look) in jane_data::looks() {
        let Look::Person(p) = look else { continue };
        let n = all().iter().filter(|r| r.sprite == *id && r.seat == 0).count();
        assert_eq!(n, p.vary.count(), "{}: one set a variant", looks::name_of(*id));
        assert!(p.vary.count() <= 4, "vary budget");
    }
}

#[test]
fn geometry_a_person_is_32_by_40_standing_on_16_36() {
    for r in all() {
        assert_eq!((r.set.w, r.set.h, r.set.ax, r.set.ay), (W, H, AX, AY));
        assert_eq!((W, H, AX, AY), (32, 40, 16, 36));
        for (f, c) in frames(r) {
            assert_eq!((c.w(), c.h()), (32, 40), "{} {f:?}", r.key());
            let (x0, y0, x1, y1) = drawn(c);
            assert!(x0 >= 0 && y0 >= 0 && x1 < 32 && y1 < 40, "{} {f:?}", r.key());
            if f == FrameId::Down || f == FrameId::Up || f == FrameId::Side {
                assert_eq!(y1, AY, "{} {f:?}: the soles are on the anchor row", r.key());
            } else if !f.is_dead() {
                assert!((AY - 1..=AY + 1).contains(&y1), "{} {f:?}: a foot is off the ground by {}", r.key(), AY - y1);
            }
        }
    }
}

#[test]
fn geometry_the_silhouette_rules_hold() {
    // Head 16 wide (hair included, hats aside), a neck under it, the feet 4 wide.
    for r in bases() {
        let Some((_, Look::Person(p))) = looks::find(r.name) else { unreachable!() };
        let c = r.set.frame(FrameId::Down).unwrap();
        let pr = person::proportions(p.build);
        // In a rocking chair the chair's back posts stand either side of her head: not her head.
        let seated = p.extras.contains(&jane_data::Extra::Seated);
        if p.head.hat == jane_data::Hat::None && p.head.hair != jane_data::Hair::Pigtails && !seated {
            for y in pr.skull_y()..pr.chin_y() {
                let w = (0..32).filter(|&x| c.get(x, y).is_opaque()).count();
                assert!(w <= 16 || y >= pr.chin_y() - 2, "{}: the head is {w} wide on row {y}", r.key());
            }
        }
        // The boots: each foot's bottom row is 4 px.
        if ghost(r) {
            continue;
        }
        let row: Vec<i32> = (0..32).filter(|&x| c.get(x, AY).is_opaque()).collect();
        assert_eq!(row.len(), 8, "{}: two feet of four on the ground, got {row:?}", r.key());
    }
}

#[test]
fn determinism_twice_is_the_same_bytes() {
    let again = looks::family(looks::Family::Person).unwrap();
    for (a, b) in all().iter().zip(&again) {
        assert_eq!(a.set, b.set, "{}", a.key());
    }
}

#[test]
fn layers_hold_the_contract_and_only_declared_roles_emit() {
    for r in all() {
        let eyes = r.set.emits.contains(&Role::Eye);
        // A lantern, or a diver's port lit from inside: a handful of px each.
        let held = r.set.emits.contains(&Role::Held) || r.set.emits.contains(&Role::Glass);
        for (f, c) in frames(r) {
            c.validate().unwrap_or_else(|e| panic!("{} {f:?}: {e}", r.key()));
            let lit = c.emissive().iter().filter(|&&e| e != Ix::CLEAR).count();
            if f.is_dead() || !(eyes || held) {
                assert_eq!(lit, 0, "{} {f:?}: emits with nothing declared", r.key());
            } else if held {
                // A lantern's glass: a handful of px, never the whole figure.
                assert!(lit <= 24, "{} {f:?}: only the eyes and the lantern emit ({lit})", r.key());
            } else {
                assert!(lit <= 2, "{} {f:?}: only the eyes emit", r.key());
            }
        }
    }
}

/// Where a drawn pixel meets clear: `(below or right, above or left)`.
fn open_sides(c: &Canvas, x: i32, y: i32) -> (bool, bool) {
    let open = |dx: i32, dy: i32| !c.get(x + dx, y + dy).is_opaque();
    (open(1, 0) || open(0, 1), open(-1, 0) || open(0, -1))
}

#[test]
fn outline_closed_and_selective() {
    // Every drawn pixel meeting clear is a line: `k`, or its material's own dark. Away from the
    // light (below, right) the line is the ramp's deep or `k`; on the lit side it is no lighter
    // than the ramp's base (ART.md §3, sel-out). A dead frame keeps the lines it stood up with.
    for r in all().iter().filter(|r| !ghost(r)) {
        for (f, c) in frames(r) {
            for y in 0..c.h() {
                for x in 0..c.w() {
                    let ix = c.get(x, y);
                    let (away, lit) = open_sides(c, x, y);
                    if !ix.is_opaque() || !(away || lit) || ix == Ix::INK || palette::is_pallid(ix) || f.is_dead() {
                        continue;
                    }
                    let Some((_, t)) = Ramp::of(ix) else {
                        panic!("{} {f:?}: ({x}, {y}) is an edge in {ix:?}, neither k nor a ramp's", r.key())
                    };
                    if away {
                        assert_eq!(t, Tone::Deep, "{} {f:?}: ({x}, {y}) faces away from the light in {t:?}", r.key());
                    } else {
                        assert!(t <= Tone::Base, "{} {f:?}: ({x}, {y}) a lit edge in {t:?}", r.key());
                    }
                }
            }
        }
    }
}

/// Orphans: pixels of the sprite's own materials (its roles' ramps) whose colour none of
/// their eight neighbours shares, not counting the outline. Eyes, glints, buttons and buckles
/// are studs, not shading, and are not counted.
fn orphans(r: &Rendered, c: &Canvas) -> Vec<(i32, i32)> {
    let mine: Vec<Ramp> = r.set.roles.iter().map(|(_, ramp)| *ramp).collect();
    let mut out = Vec::new();
    for y in 0..c.h() {
        for x in 0..c.w() {
            let ix = c.get(x, y);
            if !Ramp::of(ix).is_some_and(|(ramp, _)| mine.contains(&ramp)) {
                continue;
            }
            let (away, lit) = open_sides(c, x, y);
            if away || lit {
                continue;
            }
            let alone = (-1..=1).all(|dy| (-1..=1).all(|dx| (dx, dy) == (0, 0) || c.get(x + dx, y + dy) != ix));
            if alone {
                out.push((x, y));
            }
        }
    }
    out
}

#[test]
fn clean_clusters_few_orphans() {
    // Shading in clusters of two or more: an orphan is a buckle, a button, a nose tip or a
    // mistake, and a frame has a few of the first three at most.
    let mut worst = (0, String::new());
    for r in bases() {
        for (f, c) in frames(r) {
            if f.is_dead() {
                continue;
            }
            let n = orphans(r, c).len();
            if n > worst.0 {
                worst = (n, format!("{} {f:?}: {:?}", r.key(), orphans(r, c)));
            }
        }
    }
    println!("most orphans: {} ({})", worst.0, worst.1);
    assert!(worst.0 <= MAX_ORPHANS, "{} orphans in {}", worst.0, worst.1);
}

/// The most orphans a frame may have (after `declutter`, these are a boot heel or a knuckle
/// where two shapes cross).
const MAX_ORPHANS: usize = 5;

#[test]
fn no_pillow_shading() {
    // The light comes from the left. Take every run of one material across a row that is at
    // least five px long and has the outline at both ends: one px in from each end, the left
    // must be no darker than the right. Pillow shading (dark all round, light in the middle)
    // fails a run as often as it passes one; a sprite may have one run in four against it (a
    // parting, a fold line, a strap one px in from an edge).
    let mut failures = Vec::new();
    for r in bases() {
        for f in [FrameId::Down, FrameId::Side, FrameId::Up] {
            let c = r.set.frame(f).unwrap();
            let (mut runs, mut against) = (0, 0);
            for y in 0..c.h() {
                let mut x = 0;
                while x < c.w() {
                    let Some((ramp, _)) = Ramp::of(c.get(x, y)) else {
                        x += 1;
                        continue;
                    };
                    let start = x;
                    while x < c.w() && Ramp::of(c.get(x, y)).is_some_and(|(r, _)| r == ramp) {
                        x += 1;
                    }
                    let end = x - 1;
                    let lined = |px: i32| !c.get(px, y).is_opaque() || c.get(px, y) == Ix::INK;
                    if end - start >= 4 && lined(start - 1) && lined(end + 1) {
                        runs += 1;
                        let (l, rr) = (palette::luma(c.get(start + 1, y)), palette::luma(c.get(end - 1, y)));
                        against += usize::from(l < rr);
                    }
                }
            }
            if against * 4 > runs {
                failures.push(format!("{} {f:?}: {against} of {runs}", r.key()));
            }
        }
    }
    assert!(failures.is_empty(), "lit from the right: {failures:?}");
}

/// Spikes: drawn pixels with clear on three sides.
fn spikes(c: &Canvas) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in 0..c.h() {
        for x in 0..c.w() {
            let open = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .filter(|&&(dx, dy)| !c.get(x + dx, y + dy).is_opaque())
                .count();
            if c.get(x, y).is_opaque() && open >= 3 {
                out.push((x, y));
            }
        }
    }
    out
}

#[test]
fn no_spikes_on_the_silhouette() {
    // A drawn pixel with clear on three sides is a jaggy (`despike` takes them off; the nose
    // in profile is two px tall so that it stays).
    let mut bad = Vec::new();
    for r in bases().filter(|r| !ghost(r)) {
        for (f, c) in frames(r) {
            let n = spikes(c).len();
            if n > 0 && !f.is_dead() {
                bad.push(format!("{} {f:?} {:?}", r.key(), spikes(c)));
            }
        }
    }
    assert!(bad.is_empty(), "spikes:\n{}", bad.join("\n"));
}

#[test]
fn heights_are_true() {
    // The height layer is what a sun or a lamp casts from: a standing frame's pixel stands its
    // row's height above the feet (the head 40), a lying one no more than its thickness.
    for r in all() {
        // A shade stands half its height, so its shadow is faint.
        let half = |h: i32| if ghost(r) { (h / 2).max(1) } else { h };
        for (f, c) in frames(r) {
            for y in 0..c.h() {
                for x in 0..c.w() {
                    if !c.get(x, y).is_opaque() {
                        continue;
                    }
                    let h = i32::from(c.height_at(x, y));
                    if f.is_dead() {
                        assert!((1..=5).contains(&h), "{} {f:?}: ({x}, {y}) lies {h} high", r.key());
                    } else {
                        assert_eq!(h, half(((AY - y) * 5 / 4).max(1)), "{} {f:?}: ({x}, {y})", r.key());
                    }
                }
            }
        }
        let (_, top, _, _) = drawn(r.set.frame(FrameId::Down).unwrap());
        let head = i32::from(r.set.frame(FrameId::Down).unwrap().heights().iter().copied().max().unwrap());
        assert!((half(30)..=half(46)).contains(&head), "{}: the head stands {head} (top row {top})", r.key());
    }
}

#[test]
fn colour_budget_and_no_dithered_skin() {
    for r in all() {
        // The living frames share one budget; the dead frames, in their pallid twins, another.
        let (mut used, mut dead) = (BTreeSet::new(), BTreeSet::new());
        for (f, c) in frames(r) {
            let set = if f.is_dead() { &mut dead } else { &mut used };
            set.extend(c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0));
            let skin = r.set.ramp_of(Role::Skin).unwrap();
            let is_skin = |ix: Ix| matches!(Ramp::of(ix), Some((s, _)) if s == skin);
            for y in 0..c.h() - 1 {
                for x in 0..c.w() - 1 {
                    let (a, b, e, d) = (c.get(x, y), c.get(x + 1, y), c.get(x, y + 1), c.get(x + 1, y + 1));
                    let checker = a == d && b == e && a != b && [a, b].iter().all(|&i| is_skin(i));
                    assert!(
                        !checker,
                        "{} {f:?}: skin dithered at ({x}, {y}): {:?}",
                        r.key(),
                        [a, b, e, d].map(|i| Ramp::of(i).map(|t| t.1))
                    );
                }
            }
        }
        assert!(used.len() <= 48, "{}: {} colours, the budget is 48", r.key(), used.len());
        assert!(dead.len() <= 48, "{}: {} colours dead, the budget is 48", r.key(), dead.len());
    }
}

/// Of the pixels both frames draw in different colours: how many, and their mean RGB distance
/// (per channel, 0..=255).
fn colour_distance(a: &Canvas, b: &Canvas) -> (usize, u32) {
    let (mut sum, mut n) = (0u32, 0u32);
    for (x, y) in a.albedo().iter().zip(b.albedo()) {
        if x.is_opaque() && y.is_opaque() && x != y {
            let (p, q) = (palette::rgb(*x), palette::rgb(*y));
            sum += (0..3).map(|k| u32::from(p[k].abs_diff(q[k]))).sum::<u32>() / 3;
            n += 1;
        }
    }
    (n as usize, sum / n.max(1))
}

#[test]
fn silhouette_distinct() {
    // Any two sets: the masks of their standing frames differ by 6 % of the union, or they are
    // one cut in other cloth (a seat, a variant, two neighbours dressed alike): then at least
    // 6 % of the union is recoloured, by 24 on average (ART.md §5).
    let sets: Vec<&Rendered> = all().iter().collect();
    let mut bad = Vec::new();
    for (i, a) in sets.iter().enumerate() {
        for b in &sets[i + 1..] {
            let (ca, cb) = (a.set.frame(FrameId::Down).unwrap(), b.set.frame(FrameId::Down).unwrap());
            let (mut xor, mut union) = (0, 0);
            for (p, q) in ca.albedo().iter().zip(cb.albedo()) {
                let (p, q) = (p.is_opaque(), q.is_opaque());
                xor += usize::from(p != q);
                union += usize::from(p || q);
            }
            let shape = xor * 100 >= union * 6;
            let (n, dist) = colour_distance(ca, cb);
            if !shape && (n * 100 < union * 6 || dist < 24) {
                bad.push(format!(
                    "{} and {}: masks differ by {xor} of {union}; {n} px recoloured by {dist}",
                    a.key(),
                    b.key()
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "too alike:
{}",
        bad.join(
            "
"
        )
    );
}

#[test]
fn walk_frames_are_distinct_poses() {
    for r in bases() {
        let ids = [
            [
                FrameId::Down,
                FrameId::Down1,
                FrameId::Down2,
                FrameId::Down3,
                FrameId::Down4,
                FrameId::Down5,
                FrameId::DownB,
            ],
            [FrameId::Up, FrameId::Up1, FrameId::Up2, FrameId::Up3, FrameId::Up4, FrameId::Up5, FrameId::UpB],
            [
                FrameId::Side,
                FrameId::Side1,
                FrameId::Side2,
                FrameId::Side3,
                FrameId::Side4,
                FrameId::Side5,
                FrameId::SideB,
            ],
        ];
        for cycle in ids {
            for (i, a) in cycle.iter().enumerate() {
                for b in &cycle[i + 1..] {
                    assert_ne!(r.set.frame(*a), r.set.frame(*b), "{}: {a:?} is {b:?}", r.key());
                }
            }
        }
    }
}

#[test]
fn dead_frames_lie_down() {
    // ART.md §4.1: not a living frame, shorter, not floating, more than 16 across; pallid.
    for r in all() {
        let standing = r.set.frame(FrameId::Down).unwrap();
        let (_, sy0, _, sy1) = drawn(standing);
        for f in [FrameId::Dead, FrameId::Dead2] {
            let c = r.set.frame(f).unwrap();
            assert!(r.set.frames.iter().all(|(g, d)| *g == f || d != c) || f == FrameId::Dead2, "{} {f:?}", r.key());
            let (x0, y0, x1, y1) = drawn(c);
            assert!(y1 - y0 < sy1 - sy0, "{} {f:?}: {} tall lying, {} standing", r.key(), y1 - y0 + 1, sy1 - sy0 + 1);
            assert!(y1 >= AY - 2, "{} {f:?}: floats, lowest row {y1}", r.key());
            assert!(x1 - x0 + 1 > 16, "{} {f:?}: {} across", r.key(), x1 - x0 + 1);
            let live =
                c.albedo().iter().filter(|a| Ramp::of(**a).is_some_and(|(r, _)| palette::PALLID.contains(&r))).count();
            assert_eq!(live, 0, "{} {f:?}: a person's ramp shows unpallid", r.key());
        }
        assert_ne!(r.set.frame(FrameId::Dead), r.set.frame(FrameId::Dead2), "{}", r.key());
    }
}

#[test]
fn seats_swap_the_coat_and_nothing_else() {
    let janes: Vec<&Rendered> = all().iter().filter(|r| r.name == "jane").collect();
    assert_eq!(janes.len(), 4, "jane and her three seats");
    let base = janes[0];
    let coat = base.set.ramp_of(Role::Coat).unwrap();
    for seat in &janes[1..] {
        let to = person::SEAT_COATS[usize::from(seat.seat) - 1];
        assert_eq!(seat.set.ramp_of(Role::Coat), Some(to));
        for ((f, a), (_, b)) in base.set.frames.iter().zip(&seat.set.frames) {
            assert_eq!(a.normals(), b.normals(), "{f:?}: normals are shared");
            assert_eq!(a.heights(), b.heights(), "{f:?}: heights are shared");
            assert_eq!(a.emissive(), b.emissive(), "{f:?}: emissive is shared");
            let mut changed = 0;
            for (p, q) in a.albedo().iter().zip(b.albedo()) {
                if p != q {
                    changed += 1;
                    let live = |ix: Ix, r: Ramp| {
                        palette::Tone::ALL.iter().any(|&t| r.at(t) == ix || palette::pallor(r.at(t)) == ix)
                    };
                    assert!(live(*p, coat) && live(*q, to), "{f:?}: a pixel not of the coat changed");
                }
            }
            // Every coat pixel the frame shows changed (from behind, her hair and pack hide
            // most of it; a cast seen from behind hides nearly all).
            let shown = a
                .albedo()
                .iter()
                .filter(|&&p| palette::Tone::ALL.iter().any(|&t| coat.at(t) == p || palette::pallor(coat.at(t)) == p))
                .count();
            assert!(changed == shown && (shown > 20 || f.name().contains("up")), "{f:?}: the coat did not change");
        }
    }
    // Only a player's sprite has seats.
    assert!(all().iter().filter(|r| r.seat > 0).all(|r| r.name == "jane"));
}

#[test]
fn a_mirrored_side_frame_faces_west() {
    // West is east mirrored at draw time (ART.md §4): the face's pixel past the skull flips side.
    let jane = bases().find(|r| r.name == "jane").unwrap();
    let mut west = jane.set.frame(FrameId::Side).unwrap().clone();
    west.mirror_x();
    let (x0, _, x1, _) = drawn(jane.set.frame(FrameId::Side).unwrap());
    let (w0, _, w1, _) = drawn(&west);
    assert_eq!((w0, w1), (31 - x1, 31 - x0));
    west.validate().unwrap();
}

#[test]
fn a_shade_comes_apart_and_stands_half_height() {
    // Above the hip a shade is whole; below it thins to a checker and then to nothing, so no
    // row near the feet is as full as its chest, and it casts from half height. Its eyes stay lit.
    let shades: Vec<&Rendered> = bases().filter(|r| ghost(r)).collect();
    assert!(!shades.is_empty(), "the shade has a look");
    for r in shades {
        let c = r.set.frame(FrameId::Down).unwrap();
        let row = |y: i32| (0..c.w()).filter(|&x| c.get(x, y).is_opaque()).count();
        let chest = (12..24).map(row).max().unwrap_or(0);
        assert!(chest >= 12, "{}: a chest", r.key());
        assert!(row(AY) * 3 <= chest, "{}: the feet come apart ({} of {chest})", r.key(), row(AY));
        assert!(!c.has(Ix::AO), "{}: a shade lays no contact shadow", r.key());
        assert!(c.emissive().iter().any(|&e| e != Ix::CLEAR), "{}: its eyes are lit", r.key());
        let top = i32::from(c.heights().iter().copied().max().unwrap());
        assert!(top <= 23, "{}: half height ({top})", r.key());
    }
}
