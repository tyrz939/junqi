//! ART.md §5's acceptance tests for the prop kit (§8 step 5): coverage, geometry, layers and
//! emission, outline closed, heights, colour budget, determinism. The goldens are
//! `tests/golden.rs`.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use jane_art::Canvas;
use jane_art::kit;
use jane_art::looks::{self, Family, Rendered};
use jane_art::palette::{Ix, Ramp, Tone};
use jane_art::sprite::FrameId;
use jane_data::{Look, PropState, catalog};

fn all() -> &'static [Rendered] {
    static ALL: OnceLock<Vec<Rendered>> = OnceLock::new();
    ALL.get_or_init(|| looks::family(Family::Prop).expect("every prop renders"))
}

fn look(r: &Rendered) -> jane_data::PropLook {
    match looks::find(r.name) {
        Some((_, Look::Prop(p))) => *p,
        _ => unreachable!(),
    }
}

/// The first walk (ART.md §8 step 5): the station, the lamp road, Julie's house and yard, the
/// house, the cellar and the mine all dress.
const FIRST_WALK: &[&str] = &[
    "sign",
    "lamp_post",
    "town_station_sign",
    "town_shelter",
    "town_ticket_window",
    "town_trolley",
    "town_sacks",
    "town_park_bench",
    "town_milk_churn",
    "town_chimney",
    "campfire",
    "crate",
    "door",
    "barrel",
    "town_woodpile",
    "town_washing_line",
    "town_hen_coop",
    "dinner_tin",
    "hatch",
    "stove",
    "table",
    "bench",
    "chest",
    "fruit_bowl",
    "shelf",
    "torch",
    "note",
    "stairs",
    "gate_h",
    "jar",
    "jar_big",
    "boss_chest",
    "mine_lamp_n",
    "mine_lamp_e",
    "lever",
    "plate",
    "minecart",
    "broken_hoist",
    "broken_cabinet",
];

#[test]
fn coverage_the_first_walk_dresses_and_every_look_draws_its_frames() {
    for name in FIRST_WALK {
        let (_, l) = looks::find(name).unwrap_or_else(|| panic!("{name} has no look"));
        assert!(matches!(l, Look::Prop(_)), "{name} is a prop");
    }
    for r in all() {
        let got: Vec<FrameId> = r.set.frames.iter().map(|(f, _)| *f).collect();
        assert_eq!(got, kit::frame_ids(&look(r)), "{}", r.key());
    }
    // Every prop sprite a row names must have a look before the P5 gate; count the ones that do.
    let names: BTreeSet<&str> =
        catalog().story.props.iter().map(|p| catalog().sprites[usize::from(p.sprite.0)]).collect();
    let dressed = names.iter().filter(|n| looks::find(n).is_some()).count();
    println!("{dressed} of {} prop sprites dressed", names.len());
    let bare: Vec<&&str> = names.iter().filter(|n| looks::find(n).is_none()).collect();
    assert!(bare.is_empty(), "prop sprites with no look (they would draw a stand-in): {bare:?}");
}

#[test]
fn geometry_a_prop_fills_its_footprint_wide_and_rises_above_it() {
    for r in all() {
        let l = look(r);
        let (fw, fh) = kit::footprint(r.sprite).unwrap();
        let (w, h) = (i32::from(fw) * 16, i32::from(fh) * 16 + i32::from(l.rise));
        assert_eq!((r.set.w, r.set.h, r.set.ax, r.set.ay), (w, h, 0, h - i32::from(fh) * 16), "{}", r.key());
        for (f, c) in &r.set.frames {
            assert_eq!((c.w(), c.h()), (w, h), "{} {f:?}", r.key());
            // A stain is all contact shade: it darkens the floor it lies on and paints nothing.
            let drawn = if l.shape == "stain" {
                c.albedo().contains(&Ix::AO)
            } else {
                c.albedo().iter().any(|a| a.is_opaque())
            };
            assert!(drawn, "{} {f:?}: draws nothing", r.key());
        }
    }
}

#[test]
fn layers_hold_the_contract_and_only_lit_frames_emit() {
    for r in all() {
        let l = look(r);
        for (f, c) in &r.set.frames {
            c.validate().unwrap_or_else(|e| panic!("{} {f:?}: {e}", r.key()));
            let lit = c.emissive().iter().filter(|&&e| e != Ix::CLEAR).count();
            if *f == FrameId::On && !l.emits.is_empty() {
                assert!(lit > 0, "{} {f:?}: a lit lamp that does not glow", r.key());
            } else {
                assert_eq!(lit, 0, "{} {f:?}: glows unlit", r.key());
            }
        }
        if l.states.contains(&PropState::On) && !l.emits.is_empty() {
            assert_ne!(r.set.frame(FrameId::Base), r.set.frame(FrameId::On), "{}", r.key());
        }
    }
}

#[test]
fn outline_closed_and_selective() {
    for r in all() {
        if look(r).shape == "web" {
            continue;
        }
        for (f, c) in &r.set.frames {
            for y in 0..c.h() {
                for x in 0..c.w() {
                    let ix = c.get(x, y);
                    let open = |dx: i32, dy: i32| !c.get(x + dx, y + dy).is_opaque();
                    let (away, lit) = (open(1, 0) || open(0, 1), open(-1, 0) || open(0, -1));
                    // What glows is light and is not lined.
                    if !ix.is_opaque() || !(away || lit) || ix == Ix::INK || c.emissive_at(x, y) != Ix::CLEAR {
                        continue;
                    }
                    let Some((_, t)) = Ramp::of(ix) else {
                        panic!("{} {f:?}: ({x}, {y}) is an edge in {ix:?}", r.key())
                    };
                    if away {
                        assert_eq!(t, Tone::Deep, "{} {f:?}: ({x}, {y}) faces away in {t:?}", r.key());
                    } else {
                        assert!(t <= Tone::Base, "{} {f:?}: ({x}, {y}) a lit edge in {t:?}", r.key());
                    }
                }
            }
        }
    }
}

#[test]
fn heights_stand_on_the_foot() {
    // Standing things rise from the footprint's front edge; no pixel stands higher than its
    // row above the foot allows, and every drawn one stands at least 1.
    for r in all() {
        for (f, c) in &r.set.frames {
            let foot = c.h() - 1;
            for y in 0..c.h() {
                for x in 0..c.w() {
                    if !c.get(x, y).is_opaque() {
                        continue;
                    }
                    let h = i32::from(c.height_at(x, y));
                    // A thing lying on the ground (a coffin, a hatch) is at most its thickness.
                    let most = ((foot - y) * 5 / 4 + 1).max(8);
                    assert!(h >= 1 && h <= most, "{} {f:?}: ({x}, {y}) at {h}", r.key());
                }
            }
        }
    }
}

#[test]
fn colour_budget() {
    for r in all() {
        let used: BTreeSet<u16> = r
            .set
            .frames
            .iter()
            .flat_map(|(_, c)| {
                c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0)
            })
            .collect();
        assert!(used.len() <= 64, "{}: {} colours, the budget is 64", r.key(), used.len());
    }
}

#[test]
fn determinism_and_variants() {
    let again = looks::family(Family::Prop).unwrap();
    for (a, b) in all().iter().zip(&again) {
        assert_eq!(a.set, b.set, "{}", a.key());
    }
    for r in all() {
        if let (Some(a), Some(b)) = (r.set.frame(FrameId::Base), r.set.frame(FrameId::Base2)) {
            assert_ne!(a, b, "{}: base_2 is base", r.key());
        }
    }
}

/// A frame's opaque bounds.
fn bounds(c: &Canvas) -> Option<jane_core::grid::Rect> {
    c.bounds()
}

#[test]
fn lamps_glow_from_their_glass() {
    let post = all().iter().find(|r| r.name == "lamp_post").unwrap();
    let on = post.set.frame(FrameId::On).unwrap();
    let rows: Vec<i32> = (0..on.h()).filter(|&y| (0..on.w()).any(|x| on.emissive_at(x, y) != Ix::CLEAR)).collect();
    let b = bounds(on).unwrap();
    assert!(rows.iter().all(|&y| y < b.y + b.h / 3), "a street lamp glows at its head");
}
