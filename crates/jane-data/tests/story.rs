//! Rules the story tables keep, held against the compiled catalog (props, quests, dialogue,
//! triggers, the clock, the start). Each test names the TypeScript it carries.

use jane_core::action::{Action, Condition, ListRef};
use jane_core::blueprint::TriggerMode;
use jane_core::ids::{Key, TextId, ZoneId};
use jane_data::{Answers, BarSlot, Catalog, DialogueTree, QuestDef, ReqTarget, catalog};

/// Every action in a list and the lists inside it (`if`'s branches, `send`'s `then`), in written
/// order: the TypeScript's `eachAction` (jane/src/sim/catalog.ts).
fn each_action(c: &Catalog, l: ListRef, f: &mut dyn FnMut(&Action)) {
    for a in c.list(l) {
        f(a);
        match *a {
            Action::If { then, els, .. } => {
                each_action(c, then, f);
                if let Some(e) = els {
                    each_action(c, e, f);
                }
            }
            Action::Send { then: Some(t), .. } => each_action(c, t, f),
            _ => {}
        }
    }
}

fn quest(id: &str) -> &'static QuestDef {
    let s = &catalog().story;
    s.quest(s.quest_id(id).unwrap_or_else(|| panic!("no quest \"{id}\"")))
}

fn tree(id: &str) -> &'static DialogueTree {
    let s = &catalog().story;
    s.dialogue(s.dialogue_id(id).unwrap_or_else(|| panic!("no dialogue \"{id}\"")))
}

fn prop(id: &str) -> &'static jane_data::PropDef {
    let s = &catalog().story;
    s.prop(s.prop_id(id).unwrap_or_else(|| panic!("no prop \"{id}\"")))
}

#[test]
fn every_table_is_there() {
    let s = &catalog().story;
    // jane/test/quests.test.ts: "at least 7 + 19 quests".
    assert!(s.quests.len() >= 26, "{} quests", s.quests.len());
    assert!(!s.props.is_empty() && !s.dialogue.is_empty() && !s.triggers.is_empty() && !s.clock.is_empty());
    for (i, q) in s.quests.iter().enumerate() {
        assert_eq!(s.quest_id(q.id).map(jane_core::QuestId::index), Some(i), "quests are in sorted-id order");
    }
}

/// jane/src/sim/catalog.ts validateCatalog, dialogue: a start, the last start row unconditional,
/// every node has lines and at most two options, every index lands on a node.
#[test]
fn every_dialogue_tree_starts_and_ends_somewhere() {
    let c = catalog();
    for t in c.story.dialogue {
        let last = t.start.last().unwrap_or_else(|| panic!("{}: no start", t.id));
        assert!(last.when.is_none(), "{}: the last start row is conditional", t.id);
        let n = t.nodes.len();
        for s in t.start {
            assert!(usize::from(s.node) < n, "{}: start past the nodes", t.id);
        }
        for node in t.nodes {
            assert!(!node.lines.is_empty(), "{}.{}: no lines", t.id, node.id);
            assert!(node.options.len() <= 2, "{}.{}: more than two options", t.id, node.id);
            for g in node.goto.iter().chain(node.options.iter().filter_map(|o| o.goto.as_ref())) {
                assert!(usize::from(*g) < n, "{}.{}: goto past the nodes", t.id, node.id);
            }
        }
    }
    // The dog is its own first tree: "Mina." is its intro's first line (jane/test/rest.test.ts).
    let dog = tree("dog");
    let intro = dog.node(dog.node_index("intro").expect("the dog has an intro"));
    assert_eq!(c.text(intro.lines[0].text), "{name}.");
}

/// jane/test/quests.test.ts "in every tree a hand-in row stands above every other row, and leads
/// to its hand-in". The one documented exception (QUESTS.md ASKS C): Julie's garden book shows a
/// night-time progress line for `rose_and_stone`, whose hand-in home is the dog.
#[test]
fn a_ready_row_stands_first_and_hands_its_quest_in() {
    let c = catalog();
    let s = &c.story;
    let rose = s.quest_id("rose_and_stone");
    for t in s.dialogue {
        let mut last_ready: Option<usize> = None;
        let mut first_other = usize::MAX;
        for (n, row) in t.start.iter().enumerate() {
            let when = row.when.map_or(&[][..], |w| c.conds_of(w));
            let ready: Vec<_> = when
                .iter()
                .filter(|k| !k.not)
                .filter_map(|k| if let Condition::QuestReady(q) = k.c { Some(q) } else { None })
                .collect();
            if t.id == "garden_book" && ready.iter().any(|q| Some(*q) == rose) {
                continue;
            }
            let Some(&quest) = ready.first() else {
                first_other = first_other.min(n);
                continue;
            };
            last_ready = Some(n);
            // From this row's node, some node or option hands the quest in.
            let mut open = vec![row.node];
            let mut seen = vec![false; t.nodes.len()];
            let mut found = false;
            while let Some(i) = open.pop() {
                if found || std::mem::replace(&mut seen[usize::from(i)], true) {
                    continue;
                }
                let node = t.node(i);
                let lists = node.actions.iter().chain(node.options.iter().filter_map(|o| o.actions.as_ref()));
                for l in lists {
                    each_action(c, *l, &mut |a| found |= *a == Action::HandIn(quest));
                }
                open.extend(node.goto);
                open.extend(node.options.iter().filter_map(|o| o.goto));
            }
            assert!(found, "{}: the row ready for \"{}\" never hands it in", t.id, s.quest(quest).id);
        }
        if let Some(l) = last_ready {
            assert!(l < first_other, "{}: a questReady row sits below a row that would answer first", t.id);
        }
    }
}

/// jane/test/quest-audit.test.ts "every quest says who takes it back": a phrase that follows
/// "Back to", so it opens lower case or on a name.
#[test]
fn every_quest_says_who_takes_it_back() {
    const OPENS: [&str; 5] = ["the ", "Pell's ", "Mrs ", "Miss ", "Mr "];
    let c = catalog();
    let names_first = |s: &str| {
        // [A-Z][a-z]+'s  or  [A-Z][a-z]+,
        let mut b = s.bytes();
        if !b.next().is_some_and(|x| x.is_ascii_uppercase()) {
            return false;
        }
        let rest = &s[1..];
        let word = rest.bytes().take_while(u8::is_ascii_lowercase).count();
        word > 0 && (rest[word..].starts_with("'s ") || rest[word..].starts_with(", "))
    };
    for q in c.story.quests {
        let back = c.text(q.return_to);
        assert!(!back.is_empty(), "{}: returnTo", q.id);
        assert!(OPENS.iter().any(|o| back.starts_with(o)) || names_first(back), "{}: returnTo \"{back}\"", q.id);
    }
}

/// jane/test/quests.test.ts "asks only for places, creatures and things that exist" (the
/// seedless half): every kill target is a unit def, every acquire an item, every location a name.
#[test]
fn quest_steps_name_real_things() {
    let c = catalog();
    let letter = quest("the_letter");
    assert!(matches!(letter.requirements, [r] if r.target == ReqTarget::Location(c.name_id("stoop").unwrap())));
    let skeleton = quest("defeat_skeleton");
    assert!(matches!(skeleton.requirements[0].target, ReqTarget::Kill(_)));
    for q in c.story.quests {
        assert!(!q.requirements.is_empty(), "{}", q.id);
        for r in q.requirements {
            assert!(r.qty >= 1, "{}", q.id);
            if matches!(r.target, ReqTarget::Location(_)) {
                assert_eq!(r.qty, 1, "{}: a location is visited once", q.id);
            }
            if let ReqTarget::Acquire(i) = r.target {
                assert!(
                    c.combat.items.is_empty() || c.combat.item(i).story,
                    "{}: an acquired item is a story item",
                    q.id
                );
            }
        }
    }
}

/// jane/src/sim/catalog.ts StartDef, jane/test/bot.ts (which moves `start.mark`): New Game gives
/// the letter, carries it, and stands her on the county's `start` mark.
#[test]
fn new_game_gives_the_letter() {
    let c = catalog();
    let st = &c.story.start;
    assert_eq!(st.quests, &[c.story.quest_id("the_letter").unwrap()]);
    assert_eq!(c.name(st.mark), "start");
    let letter =
        st.items.iter().find(|s| c.combat.item(s.item).id == "julies_letter").expect("the letter is in the kit");
    assert_eq!(letter.qty, 1);
    assert!(matches!(st.bar[0], Some(BarSlot::Spell(_))), "the first slot is her melee");
    assert!(st.bar.contains(&Some(BarSlot::Item(letter.item))), "the letter is on the bar");
}

/// jane/data/triggers.json `arrive_stoop` as quests.test.ts walks it: the county's stoop rect
/// sets the location and hands in the letter, once.
#[test]
fn the_stoop_hands_in_the_letter() {
    let c = catalog();
    let s = &c.story;
    let t = s.trigger(s.trigger_id("arrive_stoop").expect("arrive_stoop"));
    assert_eq!(t.zone, ZoneId::County);
    assert_eq!(t.trigger.rect, Key::Name(c.name_id("stoop").unwrap()));
    assert_eq!(t.trigger.mode, TriggerMode::Enter);
    assert!(t.trigger.once && t.trigger.reset.is_none());
    let acts = c.list(t.trigger.actions);
    assert_eq!(acts.len(), 2);
    assert_eq!(acts[1], Action::HandIn(s.quest_id("the_letter").unwrap()));
    assert!(s.triggers_in(ZoneId::County).any(|(id, _)| s.trigger(id).id == "arrive_stoop"));
    assert!(s.triggers_in(ZoneId::Burial).all(|(_, t)| t.zone == ZoneId::Burial));
    // The Burial's lock-in undoes itself on a death (jane/src/sim/sim.ts revivePlayer).
    let lockin = s.trigger(s.trigger_id("burial_lockin").expect("burial_lockin"));
    assert!(lockin.trigger.reset.is_some());
}

/// jane/src/data/clock.json (the bell at nine and at six) and ARCHITECTURE.md §5.2: a clock row
/// runs with no actor, so nothing in it needs one.
#[test]
fn the_bell_rings_for_nobody() {
    let c = catalog();
    assert!(c.story.clock_at(21, 0).count() >= 1 && c.story.clock_at(6, 0).count() >= 1);
    for row in c.story.clock {
        assert!(row.hour < 24);
        each_action(c, row.actions, &mut |a| assert!(!a.needs_actor(), "clock at {}: {a:?}", row.hour));
    }
}

/// jane/test/dungeon-dressing.test.ts "the Works' cages are empty and the Burial's torches are
/// cold"; jane/test/factory.test.ts "the things that answer a spark answer it"; forest.test.ts
/// (the moonbeam is night-only); catalog.ts (a light flag needs a light).
#[test]
fn lights_and_answers() {
    for s in ["n", "s", "e", "w"] {
        assert!(prop(&format!("lamp_cage_{s}")).light.is_none());
        assert!(prop(&format!("cold_sconce_{s}")).light.is_some_and(|l| l.cold));
    }
    for id in ["socket_dead", "fuse_box", "call_box", "grid_socket", "generator_cold", "relay_box"] {
        assert_eq!(prop(id).answers, Some(Answers::Shock), "{id}");
    }
    assert_eq!(prop("broken_generator").answers, Some(Answers::Repair));
    assert!(prop("works_lamp").light_when_on);
    let moon = prop("forest_moonbeam");
    assert!(moon.night_only && moon.light_shows(false, true) && !moon.light_shows(false, false));
    for p in catalog().story.props {
        assert!(p.w >= 1 && p.h >= 1, "{}", p.id);
        if p.light_when_on || p.night_only || p.day_only {
            assert!(p.light.is_some(), "{}: a light flag without a light", p.id);
        }
        assert!(!(p.night_only && p.day_only), "{}", p.id);
        assert!(p.answers.is_none_or(|a| a.school() != Some(jane_core::action::School::Heal)));
        if let Some(l) = p.light {
            assert!(l.radius.0 > 0 && (0..=1000).contains(&l.flicker.0) && l.color <= 0x00ff_ffff, "{}", p.id);
        }
    }
}

/// jane/test/rest.test.ts "is what Castle calls her: no row has a name baked in" (dialogue,
/// quests, triggers and the clock).
#[test]
fn no_story_row_has_her_name_baked_in() {
    let c = catalog();
    let s = &c.story;
    let mut texts: Vec<TextId> = Vec::new();
    let mut lists: Vec<ListRef> = Vec::new();
    for t in s.dialogue {
        texts.push(t.speaker);
        for n in t.nodes {
            texts.extend(n.lines.iter().map(|l| l.text));
            texts.extend(n.options.iter().map(|o| o.label));
            lists.extend(n.actions);
            lists.extend(n.options.iter().filter_map(|o| o.actions));
        }
    }
    for q in s.quests {
        texts.extend([q.name, q.description, q.completion, q.return_to]);
        texts.extend(q.requirements.iter().map(|r| r.text));
        lists.push(q.rewards);
    }
    lists.extend(s.triggers.iter().map(|t| t.trigger.actions));
    lists.extend(s.clock.iter().map(|r| r.actions));
    for l in lists {
        each_action(c, l, &mut |a| {
            if let Action::Toast(jane_core::TextRef::Text(t)) | Action::Read(jane_core::TextRef::Text(t)) = *a {
                texts.push(t);
            }
        });
    }
    for t in texts {
        assert!(!c.text(t).contains("Jane"), "\"{}\"", c.text(t));
    }
}
