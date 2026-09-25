//! Repairs, growing and everything else a spell switches on: a prop that `answers` Repair, Grow
//! or a school fires, once, only when she knows a spell of that kind (given `ZoneRules::given_verbs`;
//! without them nothing is gated) and holds what it `needs`, which is then spent. Its `then`s are
//! never kept for later: a thing that answers a spell is switched on once.

use super::Changed;
use crate::solve::model::Solve;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    for i in 0..s.bp.props.len() {
        let p = &s.bp.props[i];
        if s.props[i].withheld || p.use_list.is_none() || !p.loot.is_empty() || s.def(i).answers.is_none() {
            continue;
        }
        let Some(at) = s.at(i) else { continue };
        changed |= s.work(i, at);
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;
    use jane_core::action::{Action, Stack};

    use super::*;
    use crate::solve::model::KeyTag;
    use crate::solve::sketch::Sketch;

    fn steps(k: &mut Sketch) -> usize {
        let st = k.prop("steps", "broken_steps", 2, 1);
        let me = k.bp.props[st].key;
        let l = k.list(vec![Action::Hide(me)]);
        k.bp.props[st].use_list = Some(l);
        st
    }

    #[test]
    fn it_fires_only_once_a_spell_of_its_kind_is_known() {
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#.....#", "#.....#", "#######"]);
        k.mark("start", 1, 1);
        let st = steps(&mut k);
        k.rules.given_verbs = Some(vec![k.spell("icebolt")]);
        let repair = k.spell("repair");
        k.with(|s| {
            s.flood_all();
            assert!(!run(s), "icebolt mends nothing");
            s.verbs.as_mut().unwrap().insert(repair);
            assert!(run(s));
            assert!(s.props[st].removed && s.opened);
            assert!(!run(s), "mended once");
        });
    }

    #[test]
    fn ungated_it_mends_itself_and_a_school_answers_a_bolt() {
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#.....#", "#.....#", "#######"]);
        k.mark("start", 1, 1);
        let st = steps(&mut k);
        k.with(|s| {
            s.flood_all();
            assert!(run(s));
            assert!(s.props[st].removed);
        });
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#.....#", "#.....#", "#######"]);
        k.mark("start", 1, 1);
        let web = k.prop("web", "web_wall_h", 2, 1);
        let me = k.bp.props[web].key;
        let l = k.list(vec![Action::Hide(me)]);
        k.bp.props[web].use_list = Some(l);
        k.rules.given_verbs = Some(vec![k.spell("icebolt")]);
        let fire = k.spell("fireball");
        k.with(|s| {
            s.flood_all();
            assert!(!run(s), "frost does not burn a web");
            s.verbs.as_mut().unwrap().insert(fire);
            assert!(run(s));
        });
    }

    #[test]
    fn it_needs_its_materials_and_spends_them_once() {
        let mut k = Sketch::new(ZoneId::Arms, &["#######", "#.....#", "#.....#", "#######"]);
        k.mark("start", 1, 1);
        let st = steps(&mut k);
        let wood = k.item("wood");
        k.bp.props[st].needs = vec![Stack { item: wood, qty: 2 }];
        k.with(|s| {
            s.flood_all();
            s.add_key(KeyTag::Item(wood), 1);
            assert!(!run(s), "one plank short");
            s.add_key(KeyTag::Item(wood), 2);
            assert!(run(s));
            assert_eq!(s.have(KeyTag::Item(wood)), 1);
        });
    }
}
