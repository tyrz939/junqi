//! ART.md §5's acceptance tests for the icons (§2.5, §8 step 6): every icon a row names has a
//! look; each renders at 32 and at 16; the layers hold; at most 24 colours; what glows emits
//! and nothing else does; no two icons are the same drawing.

use std::collections::BTreeSet;

use jane_art::looks::{self, Family};
use jane_art::palette::Ix;
use jane_data::{Look, catalog};

#[test]
fn every_icon_is_drawn_at_32_and_16() {
    let c = catalog();
    let mut named = BTreeSet::new();
    for i in c.combat.items {
        named.insert(c.sprites[usize::from(i.icon.0)]);
    }
    for s in c.combat.spells {
        named.insert(c.sprites[usize::from(s.icon.0)]);
    }
    let missing: Vec<&&str> = named.iter().filter(|n| looks::find(n).is_none()).collect();
    assert!(missing.is_empty(), "icons with no look: {missing:?}");
    let all = looks::family(Family::Icon).unwrap();
    let mut seen = BTreeSet::new();
    for r in &all {
        let size = if r.variant == 0 { 32 } else { 16 };
        assert_eq!((r.set.w, r.set.h), (size, size), "{}", r.key());
        let Some((_, Look::Icon(l))) = looks::find(r.name) else { unreachable!() };
        let c = &r.set.frames[0].1;
        c.validate().unwrap_or_else(|e| panic!("{}: {e}", r.key()));
        let used: BTreeSet<u16> =
            c.albedo().iter().filter(|a| a.is_opaque() && **a != Ix::INK && **a != Ix::SEAM).map(|a| a.0).collect();
        assert!(used.len() <= 24, "{}: {} colours, the budget is 24", r.key(), used.len());
        let glows = c.emissive().iter().any(|&e| e != Ix::CLEAR);
        assert!(!glows || l.glow, "{}: glows undeclared", r.key());
        assert!(glows || !l.glow || r.variant == 1, "{}: declared to glow, and dark", r.key());
        if r.variant == 0 {
            assert!(seen.insert(c.hash()), "{}: the same drawing as another icon", r.key());
        }
    }
}
