//! Gates (and every other lock): a locked prop she can stand beside opens if she holds a key its
//! tag fits, and that key is spent. Keys are spent greedily, in prop order; whether some order
//! of spending strands her is check C3's question, not this one's.

use super::Changed;
use crate::solve::model::{KeyTag, Solve};

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    for i in 0..s.bp.props.len() {
        if s.props[i].withheld {
            continue;
        }
        let Some(at) = s.at(i) else { continue };
        if !s.locked_in(i, at) || s.props[i].shut {
            continue;
        }
        let Some(tag) = s.bp.props[i].key_tag.map(KeyTag::Tag) else { continue };
        let have = s.have(tag);
        if have <= 0 {
            continue;
        }
        s.keys.insert(tag, have - 1);
        s.props[i].open = true;
        s.mark_fired(i);
        // Only a gate's opening changes where she can walk. A chest's is for the next sweep.
        if s.def(i).gate {
            s.opened = true;
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;

    use super::*;
    use crate::solve::sketch::Sketch;

    #[test]
    fn a_held_key_opens_the_gate_it_fits_and_is_spent() {
        let mut k = Sketch::new(ZoneId::Arms, &["#####", "#...#", "#####"]);
        k.mark("start", 1, 1);
        let generic = k.name("generic");
        let g = k.prop("g", "gate_v", 2, 0);
        k.bp.props[g].locked = true;
        k.bp.props[g].key_tag = Some(generic);
        let other = k.prop("other", "gate_v", 3, 0);
        k.bp.props[other].locked = true;
        k.bp.props[other].key_tag = Some(k.name("basement"));
        k.with(|s| {
            s.flood_all();
            assert!(!run(s), "no key, nothing opens");
            s.add_key(KeyTag::Tag(generic), 1);
            assert!(run(s));
            assert!(s.props[g].open && !s.props[other].open);
            assert!(s.opened);
            assert_eq!(s.have(KeyTag::Tag(generic)), 0);
            assert!(!run(s), "a gate opens once");
        });
    }

    #[test]
    fn a_gate_kept_shut_stays_shut_and_keeps_the_key() {
        let mut k = Sketch::new(ZoneId::Arms, &["#####", "#...#", "#####"]);
        k.mark("start", 1, 1);
        let generic = k.name("generic");
        let g = k.prop("g", "gate_v", 2, 0);
        k.bp.props[g].locked = true;
        k.bp.props[g].key_tag = Some(generic);
        let key = k.bp.props[g].key;
        k.opts.shut.push(key);
        k.with(|s| {
            s.flood_all();
            s.add_key(KeyTag::Tag(generic), 1);
            assert!(!run(s));
            assert_eq!(s.have(KeyTag::Tag(generic)), 1);
        });
    }
}
