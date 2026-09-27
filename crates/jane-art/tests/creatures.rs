//! ART.md §5's acceptance tests for the creatures (§8 step 4): coverage, geometry, determinism,
//! layers, outline closed, true heights, colour budget, silhouettes distinct, dead frames. The
//! goldens are `tests/golden.rs` (every set's hash is in `tests/golden.txt`).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use jane_art::Canvas;
use jane_art::creature;
use jane_art::looks::{self, Family, Rendered};
use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_art::sprite::{FrameId, Role};
use jane_data::{Look, catalog};

fn all() -> &'static [Rendered] {
    static ALL: OnceLock<Vec<Rendered>> = OnceLock::new();
    ALL.get_or_init(|| looks::family(Family::Creature).expect("every creature renders"))
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

/// The animals of the first walk and the county (ART.md §8 step 4): the plans built so far.
const STEP_4: [&str; 12] = [
    "dog",
    "sheep",
    "town_sheep",
    "town_cat_black",
    "town_cat_ginger",
    "rat",
    "rabbit",
    "museum_fox",
    "hen",
    "town_hen",
    "town_hen_brown",
    "crow",
];

#[test]
fn coverage_the_first_walks_animals_have_looks_and_every_frame() {
    let c = catalog();
    for name in STEP_4 {
        let (id, look) = looks::find(name).unwrap_or_else(|| panic!("{name} has no look"));
        assert!(matches!(look, Look::Creature(_)), "{name} is drawn as a creature");
        assert!(c.combat.units.iter().any(|u| u.sprite == id), "{name}: a look no unit uses");
    }
    for r in all() {
        let want = creature::frame_ids(looks::attacks(r.sprite));
        let got: Vec<FrameId> = r.set.frames.iter().map(|(f, _)| *f).collect();
        assert_eq!(got, want, "{}", r.key());
    }
    // The hostile beasts fight; the town's animals never do.
    assert!(all().iter().find(|r| r.name == "rat").unwrap().set.frame(FrameId::Atk2).is_some());
    assert!(all().iter().find(|r| r.name == "dog").unwrap().set.frame(FrameId::Atk1).is_none());
}

#[test]
fn geometry_each_plan_has_its_box_and_stands_on_its_anchor() {
    for r in all() {
        let Some((_, Look::Creature(l))) = looks::find(r.name) else { unreachable!() };
        let (w, h, ax, ay) = creature::size_of(l.plan, l.anatomy);
        assert_eq!((r.set.w, r.set.h, r.set.ax, r.set.ay), (w, h, ax, ay), "{}", r.key());
        assert_eq!((ax, ay), (w / 2, h - 4), "{}: feet at (w / 2, h - 4)", r.key());
        for (f, c) in frames(r) {
            assert_eq!((c.w(), c.h()), (w, h), "{} {f:?}", r.key());
            let (x0, y0, x1, y1) = drawn(c);
            assert!(x0 >= 0 && y0 >= 0 && x1 < w && y1 < h, "{} {f:?}", r.key());
            if creature::hovers(l.plan) {
                // A flyer hovers over its anchor, where its shadow lies: never on it.
                assert!(
                    f.is_dead() || (ay - h / 2..=ay - 2).contains(&y1),
                    "{} {f:?}: hovering {} over",
                    r.key(),
                    ay - y1
                );
            } else if matches!(f, FrameId::Down | FrameId::Up | FrameId::Side | FrameId::Idle) {
                assert_eq!(y1, ay, "{} {f:?}: standing on the anchor row", r.key());
            } else if !f.is_dead() {
                assert!((ay - 1..=ay + 1).contains(&y1), "{} {f:?}: off the ground by {}", r.key(), ay - y1);
            }
        }
    }
}

#[test]
fn determinism_twice_is_the_same_bytes() {
    let again = looks::family(Family::Creature).unwrap();
    for (a, b) in all().iter().zip(&again) {
        assert_eq!(a.set, b.set, "{}", a.key());
    }
}

#[test]
fn layers_hold_the_contract_and_only_the_eyes_emit() {
    for r in all() {
        let eyes = r.set.emits.contains(&Role::Eye);
        for (f, c) in frames(r) {
            c.validate().unwrap_or_else(|e| panic!("{} {f:?}: {e}", r.key()));
            let lit = c.emissive().iter().filter(|&&e| e != Ix::CLEAR).count();
            if f.is_dead() || !eyes {
                assert_eq!(lit, 0, "{} {f:?}: emits with nothing declared", r.key());
            } else {
                assert!(lit <= 8, "{} {f:?}: only the eyes emit ({lit} px)", r.key());
            }
        }
    }
    let fox = all().iter().find(|r| r.name == "museum_fox").unwrap();
    assert!(fox.set.frame(FrameId::Side).unwrap().emissive().iter().any(|&e| e != Ix::CLEAR), "the fox's glass eyes");
}

#[test]
fn outline_closed_and_selective() {
    for r in all() {
        for (f, c) in frames(r) {
            if f.is_dead() {
                continue;
            }
            for y in 0..c.h() {
                for x in 0..c.w() {
                    let ix = c.get(x, y);
                    let open = |dx: i32, dy: i32| !c.get(x + dx, y + dy).is_opaque();
                    let (away, lit) = (open(1, 0) || open(0, 1), open(-1, 0) || open(0, -1));
                    if !ix.is_opaque() || !(away || lit) || ix == Ix::INK {
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

#[test]
fn heights_are_true() {
    for r in all() {
        let ay = r.set.ay;
        for (f, c) in frames(r) {
            for y in 0..c.h() {
                for x in 0..c.w() {
                    if !c.get(x, y).is_opaque() {
                        continue;
                    }
                    let h = i32::from(c.height_at(x, y));
                    if f.is_dead() {
                        assert!((1..=4).contains(&h), "{} {f:?}: ({x}, {y}) lies {h} high", r.key());
                    } else {
                        assert_eq!(h, ((ay - y) * 5 / 4).max(1), "{} {f:?}: ({x}, {y})", r.key());
                    }
                }
            }
        }
    }
}

#[test]
fn colour_budget() {
    for r in all() {
        let (mut used, mut dead) = (BTreeSet::new(), BTreeSet::new());
        for (f, c) in frames(r) {
            let set = if f.is_dead() { &mut dead } else { &mut used };
            set.extend(c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0));
        }
        assert!(used.len() <= 40, "{}: {} colours, the budget is 40", r.key(), used.len());
        assert!(dead.len() <= 40, "{}: {} colours dead, the budget is 40", r.key(), dead.len());
    }
}

#[test]
fn silhouettes_distinct() {
    // Two creatures of one plan: their standing masks differ by 6 % of the union, or at least
    // 6 % of it is recoloured by 24 a channel on average (ART.md §5).
    let sets: Vec<&Rendered> = all().iter().collect();
    let mut bad = Vec::new();
    for (i, a) in sets.iter().enumerate() {
        for b in &sets[i + 1..] {
            if (a.set.w, a.set.h) != (b.set.w, b.set.h) {
                continue;
            }
            for f in [FrameId::Down, FrameId::Side] {
                let (ca, cb) = (a.set.frame(f).unwrap(), b.set.frame(f).unwrap());
                let (mut xor, mut union, mut n, mut sum) = (0usize, 0usize, 0usize, 0u32);
                for (p, q) in ca.albedo().iter().zip(cb.albedo()) {
                    xor += usize::from(p.is_opaque() != q.is_opaque());
                    union += usize::from(p.is_opaque() || q.is_opaque());
                    if p.is_opaque() && q.is_opaque() && p != q {
                        let (x, y) = (palette::rgb(*p), palette::rgb(*q));
                        sum += (0..3).map(|k| u32::from(x[k].abs_diff(y[k]))).sum::<u32>() / 3;
                        n += 1;
                    }
                }
                let recoloured = n * 100 >= union * 6 && sum / n.max(1) as u32 >= 24;
                if xor * 100 < union * 6 && !recoloured {
                    bad.push(format!("{} and {} {f:?}", a.key(), b.key()));
                }
            }
        }
    }
    assert!(bad.is_empty(), "too alike: {bad:?}");
}

#[test]
fn walk_frames_are_distinct_poses() {
    use FrameId as F;
    for r in all() {
        for cycle in [
            [F::Down, F::Down1, F::Down2, F::Down3, F::Down4, F::Down5],
            [F::Up, F::Up1, F::Up2, F::Up3, F::Up4, F::Up5],
            [F::Side, F::Side1, F::Side2, F::Side3, F::Side4, F::Side5],
        ] {
            let distinct: BTreeSet<u32> = cycle.iter().map(|&f| r.set.frame(f).unwrap().hash()).collect();
            assert!(distinct.len() >= 4, "{}: a walk of {} poses in six frames", r.key(), distinct.len());
        }
        assert_ne!(r.set.frame(F::Idle), r.set.frame(F::Idle2), "{}: the idle's two beats", r.key());
    }
}

#[test]
fn no_spikes_on_the_silhouette() {
    let mut bad = Vec::new();
    for r in all() {
        for (f, c) in frames(r) {
            for y in 0..c.h() {
                for x in 0..c.w() {
                    let open = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .filter(|&&(dx, dy)| !c.get(x + dx, y + dy).is_opaque())
                        .count();
                    if c.get(x, y).is_opaque() && open >= 3 && !f.is_dead() {
                        bad.push(format!("{} {f:?} ({x}, {y})", r.key()));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "spikes:\n{}", bad.join("\n"));
}

#[test]
fn dead_frames_lie_down() {
    for r in all() {
        let standing = r.set.frame(FrameId::Side).unwrap();
        let (_, sy0, _, sy1) = drawn(standing);
        let c = r.set.frame(FrameId::Dead).unwrap();
        assert!(r.set.frames.iter().all(|(g, d)| *g == FrameId::Dead || d != c), "{}", r.key());
        let (_, y0, _, y1) = drawn(c);
        assert!(y1 - y0 < sy1 - sy0, "{}: {} tall lying, {} standing", r.key(), y1 - y0 + 1, sy1 - sy0 + 1);
        assert!(y1 >= r.set.ay - 2, "{}: floats, lowest row {y1}", r.key());
        let live =
            c.albedo().iter().filter(|a| Ramp::of(**a).is_some_and(|(r, _)| palette::PALLID.contains(&r))).count();
        assert_eq!(live, 0, "{}: a ramp shows unpallid", r.key());
        assert!(c.emissive().iter().all(|&e| e == Ix::CLEAR), "{}: the dead never shine", r.key());
    }
}

#[test]
fn julie_is_herself() {
    // The dog is the heart of the story (STORY.md §3): one ear up and one folded, a collar
    // with a brass tag, a white tail tip, and eyes with a glint when she sits and looks at you.
    let dog = all().iter().find(|r| r.name == "dog").unwrap();
    let sit = dog.set.frame(FrameId::Idle).unwrap();
    let glint = palette::letter('w').unwrap();
    assert_eq!(sit.albedo().iter().filter(|&&a| a == glint).count(), 2, "two eyes, each with its glint");
    assert!(sit.albedo().iter().any(|&a| Ramp::of(a).is_some_and(|(r, _)| r == Ramp::Brass)), "her tag");
    assert!(sit.albedo().iter().any(|&a| Ramp::of(a).is_some_and(|(r, _)| r == Ramp::ClothBrick)), "her collar");
    let (idle, tilt) = (dog.set.frame(FrameId::Idle).unwrap(), dog.set.frame(FrameId::Idle2).unwrap());
    let moved = idle.albedo().iter().zip(tilt.albedo()).filter(|(a, b)| a != b).count();
    assert!(moved >= 20, "the second beat tilts the head and sweeps the tail ({moved} px)");
}
