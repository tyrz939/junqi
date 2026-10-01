//! Setting a side quest aside (`quests::abandon`, `Command::Abandon`) on the real seed: the main
//! line refuses; a side quest leaves the log, its giver offers it again, it can be taken again
//! and finished; what the giver handed over goes back, what only it asked for is set down at her
//! feet, kills made for it still count.
//!
//! **The softlock search** (`every_side_quest_set_aside_is_offered_again`): for every side quest
//! in the content, from New Game, each place that gives it is found in the dialogue (a start row,
//! the nodes it reaches, the list with the `quest` verb); the row's conditions are made to hold
//! (quests it needs done are done, one it hands in is made ready, flags and the hour set, things
//! held) until its giver would offer it, as the "!" over him is asked; then it is taken by that
//! very list (the giver's `give`s and flags with it), set aside at once, and must be offered
//! again; then taken again, every step made good, set aside once more, and offered again. A quest
//! whose giver cannot be brought to offer it at all is a failure too, so nothing escapes the
//! search by being hard to reach.

mod common;

use common::bot::*;
use common::{bps, new_game};
use jane_core::action::{Cond, Condition, FlagTest};
use jane_core::{Action, DialogueId, Key, ListRef, QuestId, ZoneId};
use jane_data::{Catalog, ReqTarget};
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::state::{Dialogue, FlagKey, QuestProgress, Speaker};
use jane_sim::sym::of_name;
use jane_sim::{Command, Seat, Sim};

fn quest(name: &str) -> QuestId {
    jane_data::catalog().story.quest_id(name).unwrap_or_else(|| panic!("no quest {name}"))
}

fn active(s: &Sim, q: QuestId) -> bool {
    s.state().quests.active.iter().any(|p| p.quest == q)
}

fn done(s: &Sim, q: QuestId) -> bool {
    s.state().quests.done.contains(&q)
}

fn held(s: &Sim, item: jane_core::ItemId) -> u32 {
    s.state().connected().map(|p| jane_sim::bag::bag_count(&p.bag[..], item)).sum()
}

/// Does `list` (or a list it runs) hold `want`?
fn holds(c: &Catalog, list: ListRef, want: &dyn Fn(&Action) -> bool, depth: u8) -> bool {
    depth < 6
        && c.list(list).iter().any(|a| {
            want(a)
                || match *a {
                    Action::If { then, els, .. } => {
                        holds(c, then, want, depth + 1) || els.is_some_and(|e| holds(c, e, want, depth + 1))
                    }
                    Action::Send { then: Some(t), .. } => holds(c, t, want, depth + 1),
                    _ => false,
                }
        })
}

/// A place in the dialogue that runs a list: a tree, the start row that reaches it, the node,
/// and its option (`None`: the node's own actions).
#[derive(Clone, Copy, Debug)]
struct Site {
    tree: DialogueId,
    row: usize,
    node: u16,
    option: Option<u8>,
}

/// Every site whose list does `want`, in tree and row order.
fn sites(want: &dyn Fn(&Action) -> bool) -> Vec<Site> {
    let c = jane_data::catalog();
    let mut out = Vec::new();
    for (ti, t) in c.story.dialogue.iter().enumerate() {
        let tree = DialogueId(ti as u16);
        for (row, r) in t.start.iter().enumerate() {
            let mut seen = vec![false; t.nodes.len()];
            let mut todo = vec![r.node];
            while let Some(i) = todo.pop() {
                if std::mem::replace(&mut seen[usize::from(i)], true) {
                    continue;
                }
                let n = t.node(i);
                if n.actions.is_some_and(|l| holds(c, l, want, 0)) {
                    out.push(Site { tree, row, node: i, option: None });
                }
                for (o, opt) in n.options.iter().enumerate() {
                    if opt.actions.is_some_and(|l| holds(c, l, want, 0)) {
                        out.push(Site { tree, row, node: i, option: Some(o as u8) });
                    }
                    todo.extend(opt.goto);
                }
                todo.extend(n.goto);
            }
        }
    }
    out
}

/// Who speaks as `tree` in the county.
fn speaker(s: &Sim, tree: DialogueId) -> Option<Speaker> {
    let cat = jane_data::catalog();
    let z = s.state().zone(ZoneId::County)?;
    if let Some(u) = z.units.iter().find(|u| u.alive && cat.combat.unit(u.def).talk == Some(tree)) {
        return Some(Speaker::Unit(u.id));
    }
    let v = s.view(Seat(0))?;
    v.props().find(|p| v.prop_spawn(p).is_some_and(|sp| sp.talk == Some(tree))).map(|p| Speaker::Prop(p.id))
}

fn flag(k: jane_core::FlagKey) -> Option<FlagKey> {
    match k {
        jane_core::FlagKey::Named(Key::Name(n)) => Some(FlagKey::Named(of_name(n))),
        jane_core::FlagKey::Been(Key::Name(n)) => Some(FlagKey::Been(of_name(n))),
        jane_core::FlagKey::Dead(Key::Name(n)) => Some(FlagKey::Dead(of_name(n))),
        _ => None,
    }
}

fn mark_done(s: &mut Sim, p: QuestId) {
    let st = s.state_mut();
    st.quests.active.retain(|a| a.quest != p);
    if !st.quests.done.contains(&p) {
        st.quests.done.push(p);
    }
}

/// Every step of `p` made good (in the log first, if it is not).
fn make_ready(s: &mut Sim, p: QuestId) {
    let d = jane_data::catalog().story.quest(p);
    if !active(s, p) {
        s.state_mut().quests.active.push(QuestProgress { quest: p, counts: vec![0; d.requirements.len()] });
    }
    for (i, r) in d.requirements.iter().enumerate() {
        match r.target {
            ReqTarget::Kill(_) => {
                let st = s.state_mut();
                let prog = st.quests.active.iter_mut().find(|a| a.quest == p).unwrap();
                prog.counts[i] = r.qty;
            }
            ReqTarget::Acquire(item) => {
                let have = held(s, item);
                if have < u32::from(r.qty) {
                    cmd(s, Command::Dev(DevOp::Give { item, qty: r.qty - have as u16 }));
                }
            }
            ReqTarget::Location(n) => {
                s.state_mut().flags.insert(FlagKey::Been(of_name(n)), 1);
            }
        }
    }
    assert!(jane_sim::quests::ready(s.state(), p), "{} made ready", d.id);
}

/// The row's conditions made to hold, as far as a test's hand can: quests done or in the log, a
/// quest the site hands in made ready, flags, the hour, things held. What cannot be forced (what
/// a speaker has heard, a fact known) is left to the offer check that follows.
fn force(s: &mut Sim, conds: &[Cond], q: QuestId) {
    for c in conds {
        match (c.not, c.c) {
            (false, Condition::QuestDone(p)) if p != q => mark_done(s, p),
            (false, Condition::QuestActive(p)) if p != q && !active(s, p) && !done(s, p) => {
                let n = jane_data::catalog().story.quest(p).requirements.len();
                s.state_mut().quests.active.push(QuestProgress { quest: p, counts: vec![0; n] });
            }
            (false, Condition::QuestReady(p)) if p != q => make_ready(s, p),
            (not, Condition::Flag { key, test }) => {
                let Some(k) = flag(key) else { continue };
                let want = match test {
                    FlagTest::Eq(v) | FlagTest::Min(v) => v,
                    FlagTest::NonZero => 1,
                };
                if not {
                    s.state_mut().flags.remove(&k);
                    if want == 0 {
                        s.state_mut().flags.insert(k, 1);
                    }
                } else {
                    s.state_mut().flags.insert(k, want);
                }
            }
            (false, Condition::Night) => s.state_mut().clock = 22 * jane_sim::tuning::TICKS_PER_HOUR,
            (true, Condition::Night) => s.state_mut().clock = 12 * jane_sim::tuning::TICKS_PER_HOUR,
            (false, Condition::HasItem(st)) => {
                let have = held(s, st.item);
                if have < u32::from(st.qty) {
                    cmd(s, Command::Dev(DevOp::Give { item: st.item, qty: st.qty - have as u16 }));
                }
            }
            _ => {}
        }
    }
}

/// Things whose use gives `q` ("use to read the label": found, not offered).
fn labels(q: QuestId) -> Vec<jane_core::ItemId> {
    let c = jane_data::catalog();
    (0..c.combat.items.len())
        .map(|i| jane_core::ItemId(i as u16))
        .filter(|&i| c.combat.item(i).use_list.is_some_and(|l| holds(c, l, &|a| *a == Action::Quest(q), 0)))
        .collect()
}

/// Would anyone in the county offer `q` now, by day or by night, or does she hold the thing
/// whose label gives it?
fn offered(s: &mut Sim, q: QuestId) -> bool {
    if labels(q).into_iter().any(|i| held(s, i) > 0) {
        return true;
    }
    let trees: Vec<DialogueId> = {
        let mut t: Vec<DialogueId> = sites(&|a| *a == Action::Quest(q)).iter().map(|x| x.tree).collect();
        t.dedup();
        t
    };
    let clock = s.state().clock;
    let mut yes = false;
    for hour in [None, Some(12), Some(22)] {
        if let Some(h) = hour {
            s.state_mut().clock = h * jane_sim::tuning::TICKS_PER_HOUR;
        }
        for &t in &trees {
            if let Some(sp) = speaker(s, t) {
                yes |= s.view(Seat(0)).unwrap().would_offer(t, sp, q);
            }
        }
        if yes {
            break;
        }
    }
    s.state_mut().clock = clock;
    yes
}

/// Run a site's list as the conversation would: on the node's last line, its option chosen (or
/// the node left).
fn run_site(s: &mut Sim, site: Site, sp: Speaker) {
    let t = jane_data::catalog().story.dialogue(site.tree);
    let n = t.node(site.node);
    let line = (n.lines.len() - 1) as u16;
    s.state_mut().players[0].dialogue =
        Some(Dialogue { tree: Some(site.tree), node: site.node, line, speaker: sp, read: None });
    match (site.option, n.options.is_empty()) {
        (Some(o), _) => cmd(s, Command::Choose { option: o }),
        (None, false) => cmd(s, Command::Choose { option: 0 }),
        (None, true) => cmd(s, Command::Advance),
    }
    s.state_mut().players[0].dialogue = None;
    idle(s, 1);
}

/// New Game, saved once: each quest's search starts from it.
fn start() -> Vec<u8> {
    let mut s = new_game();
    idle(&mut s, 2);
    s.save()
}

/// How a quest was given: a conversation's list, or a thing's label read.
#[derive(Clone, Copy, Debug)]
enum Way {
    Talk(Site, Speaker),
    Use(jane_core::ItemId),
}

/// Take it again the way it was first taken.
fn retake(s: &mut Sim, way: Way) {
    match way {
        Way::Talk(site, sp) => run_site(s, site, sp),
        Way::Use(item) => {
            if held(s, item) == 0 {
                cmd(s, Command::Dev(DevOp::Give { item, qty: 1 }));
            }
            idle(s, 200);
            cmd(s, Command::Item(item));
            idle(s, 2);
        }
    }
}

/// A sim at the place `q` is given, it offered and then given there; `Err` says why not.
fn taken(bytes: &[u8], q: QuestId) -> Result<(Sim, Way), String> {
    let cat = jane_data::catalog();
    let mut why = String::from("no dialogue gives it");
    for site in sites(&|a| *a == Action::Quest(q)) {
        let mut s = Sim::from_save_with(bytes, bps()).expect("loads");
        let Some(sp) = speaker(&s, site.tree) else {
            why = format!("{} speaks nowhere in the county", cat.story.dialogue(site.tree).id);
            continue;
        };
        let row = cat.story.dialogue(site.tree).start[site.row];
        let conds = row.when.map_or(&[][..], |w| cat.conds_of(w));
        force(&mut s, conds, q);
        // A row above it that answers while a quest before this one is not done: that one is.
        for above in &cat.story.dialogue(site.tree).start[..site.row] {
            for c in above.when.map_or(&[][..], |w| cat.conds_of(w)) {
                if let (true, Condition::QuestDone(p)) = (c.not, c.c)
                    && p != q
                    && !conds.iter().any(|k| k.not && k.c == Condition::QuestDone(p))
                {
                    mark_done(&mut s, p);
                }
            }
        }
        if !s.view(Seat(0)).unwrap().would_offer(site.tree, sp, q) {
            why = format!("{} row {}: brought to it, he does not offer it", cat.story.dialogue(site.tree).id, site.row);
            continue;
        }
        run_site(&mut s, site, sp);
        if !active(&s, q) {
            why = format!("{} node {}: the list did not give it", cat.story.dialogue(site.tree).id, site.node);
            continue;
        }
        return Ok((s, Way::Talk(site, sp)));
    }
    for item in labels(q) {
        let mut s = Sim::from_save_with(bytes, bps()).expect("loads");
        retake(&mut s, Way::Use(item));
        if active(&s, q) {
            return Ok((s, Way::Use(item)));
        }
        why = format!("reading the {} does not give it", cat.combat.item(item).id);
    }
    Err(why)
}

fn abandon(s: &mut Sim, q: QuestId) {
    cmd(s, Command::Abandon(q));
    idle(s, 1);
}

#[test]
fn every_side_quest_set_aside_is_offered_again() {
    let cat = jane_data::catalog();
    let bytes = start();
    let mut bad = Vec::new();
    let mut n = 0;
    for (i, d) in cat.story.quests.iter().enumerate() {
        if d.main {
            continue;
        }
        let q = QuestId(i as u16);
        n += 1;
        let (mut s, way) = match taken(&bytes, q) {
            Ok(t) => t,
            Err(why) => {
                bad.push(format!("{}: {why}", d.id));
                continue;
            }
        };
        // At once.
        abandon(&mut s, q);
        assert!(!active(&s, q), "{}: set aside", d.id);
        if !offered(&mut s, q) {
            bad.push(format!("{}: set aside at once, nobody offers it again", d.id));
            continue;
        }
        // Again, every step made good, then set aside.
        retake(&mut s, way);
        if !active(&s, q) {
            bad.push(format!("{}: offered again, the giver's list does not give it", d.id));
            continue;
        }
        make_ready(&mut s, q);
        abandon(&mut s, q);
        if !offered(&mut s, q) {
            bad.push(format!("{}: set aside when ready, nobody offers it again", d.id));
        }
    }
    assert!(n > 60, "{n} side quests");
    assert!(bad.is_empty(), "{} of {n} side quests are not offered again:\n{}", bad.len(), bad.join("\n"));
}

fn item(name: &str) -> jane_core::ItemId {
    jane_data::catalog().combat.item_id(name).unwrap_or_else(|| panic!("no item {name}"))
}

/// Hand `q` in at the row that answers while it is ready.
fn hand_in(s: &mut Sim, q: QuestId) {
    let cat = jane_data::catalog();
    let site = sites(&|a| *a == Action::HandIn(q))
        .into_iter()
        .find(|x| {
            let row = cat.story.dialogue(x.tree).start[x.row];
            row.when.is_some_and(|w| cat.conds_of(w).iter().any(|c| !c.not && c.c == Condition::QuestReady(q)))
        })
        .expect("a ready row hands it in");
    let sp = speaker(s, site.tree).expect("its taker is in the county");
    run_site(s, site, sp);
}

/// Pick up whatever lies at her feet.
fn pick_up(s: &mut Sim) {
    for _ in 0..6 {
        match s.view(Seat(0)).unwrap().focus().map(|f| f.target) {
            Some(jane_sim::interact::FocusRef::Drop(_)) => cmd(s, Command::Use),
            _ => break,
        }
        idle(s, 1);
    }
}

fn toasts(s: &mut Sim) -> Vec<(Option<Seat>, ToastKind)> {
    events(s)
        .into_iter()
        .filter_map(|e| if let EventKind::Toast(t) = e.kind { Some((e.to, t)) } else { None })
        .collect()
}

#[test]
fn the_main_line_is_never_set_aside() {
    let mut s = new_game();
    idle(&mut s, 2);
    let letter = quest("the_letter");
    assert!(active(&s, letter), "New Game gives the letter");
    assert_eq!(jane_sim::quests::can_abandon(s.state(), letter), Err(jane_sim::quests::Keep::Main));
    let _ = events(&mut s);
    abandon(&mut s, letter);
    assert!(active(&s, letter), "the story's own stays in the log");
    let said = toasts(&mut s);
    assert!(said.contains(&(Some(Seat(0)), ToastKind::StoryOwn)), "she is told why: {said:?}");
    assert!(!said.iter().any(|(_, t)| matches!(t, ToastKind::QuestAbandoned { .. })));
    // The main line is marked in the data, and only it.
    let cat = jane_data::catalog();
    let main: Vec<&str> = cat.story.quests.iter().filter(|d| d.main).map(|d| d.id).collect();
    assert_eq!(main.len(), 12, "{main:?}");
    assert!(main.contains(&"the_choice") && main.contains(&"rats_below"));
}

#[test]
fn set_aside_then_taken_again_and_finished() {
    let bytes = start();
    let q = quest("ames_spectacles");
    let specs = item("ames_spectacles");
    let (mut s, way) = taken(&bytes, q).expect("Mr Ames asks");
    cmd(&mut s, Command::Dev(DevOp::Give { item: specs, qty: 1 }));
    assert!(jane_sim::quests::ready(s.state(), q), "found");
    let _ = events(&mut s);
    abandon(&mut s, q);
    assert!(!active(&s, q) && !done(&s, q), "out of the log, not done");
    let said = toasts(&mut s);
    assert!(said.contains(&(None, ToastKind::QuestAbandoned { quest: q, by: Seat(0) })), "{said:?}");
    // Only this quest asks for his spectacles: they are set down at her feet, not lost.
    assert_eq!(held(&s, specs), 0, "out of her bag");
    let at = me(&s).pos;
    let z = s.state().zone(ZoneId::County).unwrap();
    assert!(z.drops.iter().any(|d| d.item == specs && d.pos == at), "at her feet");
    assert!(offered(&mut s, q), "Mr Ames asks again");
    retake(&mut s, way);
    assert!(active(&s, q) && !jane_sim::quests::ready(s.state(), q), "taken again; the spectacles lie on the ground");
    pick_up(&mut s);
    assert_eq!(held(&s, specs), 1, "picked up again");
    assert!(jane_sim::quests::ready(s.state(), q));
    let apples = held(&s, item("apple"));
    hand_in(&mut s, q);
    idle(&mut s, 2);
    assert!(done(&s, q), "handed in");
    assert_eq!(held(&s, item("apple")), apples + 2, "paid once");
}

#[test]
fn kills_made_for_it_still_count_when_it_is_taken_again() {
    // The ruffians at Hurst's camp do not stand up again: a kill made and then set aside must
    // not be lost, or the quest could never be finished.
    let bytes = start();
    let q = quest("hurst_camp");
    let (mut s, way) = taken(&bytes, q).expect("Hurst asks");
    let d = jane_data::catalog().story.quest(q);
    let k = d.requirements.iter().position(|r| matches!(r.target, ReqTarget::Kill(_))).expect("a kill step");
    s.state_mut().quests.active.iter_mut().find(|p| p.quest == q).unwrap().counts[k] = 1;
    abandon(&mut s, q);
    assert_eq!(s.state().quests.set_aside.iter().find(|p| p.quest == q).map(|p| p.counts[k]), Some(1));
    retake(&mut s, way);
    let prog = s.state().quests.active.iter().find(|p| p.quest == q).expect("taken again");
    assert_eq!(prog.counts[k], 1, "the dead stay dead");
    assert!(s.state().quests.set_aside.iter().all(|p| p.quest != q), "taken again, nothing parked");
}

#[test]
fn what_the_giver_handed_over_goes_back_and_comes_again() {
    let bytes = start();
    let q = quest("two_loaves");
    let loaf = item("town_loaf");
    let (mut s, way) = taken(&bytes, q).expect("the baker asks");
    assert_eq!(held(&s, loaf), 1, "the loaf comes with it");
    abandon(&mut s, q);
    assert_eq!(held(&s, loaf), 0, "taken back");
    retake(&mut s, way);
    assert_eq!(held(&s, loaf), 1, "handed over again, not twice");
    // An ordinary thing she holds is hers to keep.
    let apples = held(&s, item("apple"));
    abandon(&mut s, q);
    assert_eq!(held(&s, item("apple")), apples);
}

#[test]
fn any_seat_may_set_a_quest_aside_for_the_party() {
    let bytes = start();
    let q = quest("ames_spectacles");
    let (mut s, _) = taken(&bytes, q).expect("Mr Ames asks");
    cmd(&mut s, Command::Open(true));
    let join = [jane_sim::StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: jane_sim::ClientToken(77) } }];
    s.step(&jane_sim::StepInput { commands: &join, ..jane_sim::StepInput::IDLE });
    idle(&mut s, 2);
    assert!(s.state().player(Seat(1)).is_some_and(|p| p.connected), "a guest sits down");
    let _ = events(&mut s);
    cmd_as(&mut s, 1, Command::Abandon(q));
    idle(&mut s, 1);
    assert!(!active(&s, q), "the party's log");
    let said = toasts(&mut s);
    assert!(said.contains(&(None, ToastKind::QuestAbandoned { quest: q, by: Seat(1) })), "whose coat: {said:?}");
}
