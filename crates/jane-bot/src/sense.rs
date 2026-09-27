//! Questions a bot asks of a [`View`]: where a prop or unit is, what a door leads to, what she
//! holds, who is hostile. Each is a read of the view and the catalog, nothing kept.

use jane_core::action::Action;
use jane_core::blueprint::Door;
use jane_core::num::CELL_FX;
use jane_core::{DialogueId, Fx, ItemId, ListRef, Rect, SpellId, Sym, UnitDefId, Vec2};
use jane_data::Faction;
use jane_sim::ids::UnitId;
use jane_sim::{Prop, Unit, View};

/// The whole zone.
pub fn everywhere(v: &View<'_>) -> Rect {
    let (w, h) = v.size();
    Rect::new(0, 0, w as i32, h as i32)
}

/// A prop's footprint, cells.
pub fn prop_rect(p: &Prop) -> Rect {
    let d = jane_data::catalog().story.prop(p.def);
    Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
}

/// The middle of a prop's footprint.
pub fn prop_centre(p: &Prop) -> Vec2 {
    let r = prop_rect(p);
    Vec2::new(Fx(r.x * CELL_FX + r.w * CELL_FX / 2), Fx(r.y * CELL_FX + r.h * CELL_FX / 2))
}

/// The middle of a rect of cells.
pub fn rect_centre(r: Rect) -> Vec2 {
    Vec2::new(Fx(r.x * CELL_FX + r.w * CELL_FX / 2), Fx(r.y * CELL_FX + r.h * CELL_FX / 2))
}

/// From a point to a prop's footprint (0 inside), `Fx`.
pub fn to_prop(p: &Prop, at: Vec2) -> i64 {
    let r = prop_rect(p);
    let d = |v: i32, lo: i32, hi: i32| i64::from((lo - v).max(v - hi).max(0));
    let (dx, dy) = (d(at.x.0, r.x * CELL_FX, r.right() * CELL_FX), d(at.y.0, r.y * CELL_FX, r.bottom() * CELL_FX));
    i64::from(jane_core::num::isqrt((dx * dx + dy * dy) as u64))
}

pub fn prop_by_key<'a>(v: &View<'a>, key: Sym) -> Option<&'a Prop> {
    v.props().find(|p| p.key == key)
}

pub fn prop_named<'a>(v: &View<'a>, name: &str) -> Option<&'a Prop> {
    v.sym(name).and_then(|s| prop_by_key(v, s))
}

/// An awake unit here by key.
pub fn unit_by_key<'a>(v: &View<'a>, key: Sym) -> Option<&'a Unit> {
    v.units_in(everywhere(v)).map(|u| u.unit).find(|u| u.key == Some(key))
}

/// Awake units of a def here, alive, nearest first.
pub fn units_of<'a>(v: &View<'a>, def: UnitDefId) -> Vec<&'a Unit> {
    let at = v.body().pos;
    let mut out: Vec<&Unit> = v.units_in(everywhere(v)).map(|u| u.unit).filter(|u| u.def == def && u.alive).collect();
    out.sort_by_key(|u| (jane_core::num::dist_sq(u.pos, at), u.id));
    out
}

/// Where a door leads.
pub fn door_of(v: &View<'_>, p: &Prop) -> Option<Door> {
    v.prop_spawn(p).and_then(|s| s.to)
}

/// Doors here into zone `z`, nearest first.
pub fn doors_to<'a>(v: &View<'a>, z: jane_core::ZoneId) -> Vec<&'a Prop> {
    let at = v.body().pos;
    let mut out: Vec<&Prop> = v.props().filter(|p| door_of(v, p).is_some_and(|d| d.zone == z)).collect();
    out.sort_by_key(|p| (to_prop(p, at), p.id));
    out
}

/// A catalog flag's key, by name.
fn named_flag(name: &str) -> Option<jane_core::action::FlagKey> {
    jane_data::catalog().name_id(name).map(|n| jane_core::action::FlagKey::Named(jane_core::Key::Name(n)))
}

/// Does this action end the game, and which way (`the_end` 1 hold, 2 hill, 3 train)?
pub fn sets_the_end(a: &Action) -> Option<u8> {
    match *a {
        Action::Flag { key, op: jane_core::action::FlagOp::Set(n) } if Some(key) == named_flag("the_end") && n > 0 => {
            Some(n as u8)
        }
        _ => None,
    }
}

/// Does this action signal the Sunday train (the name board's line)?
pub fn signals_train(a: &Action) -> bool {
    matches!(*a, Action::Flag { key, op: jane_core::action::FlagOp::Set(n) }
        if Some(key) == named_flag("train_signalled") && n != 0)
}

/// Can she open it: it is not locked, or she holds a key whose tag fits.
pub fn can_open(v: &View<'_>, p: &Prop) -> bool {
    !p.locked || keyed_for(v, p)
}

/// The hours a door keeps now, as the sim reads them (`interact::night_lock_of`): a verb's, else
/// its row's. What its notice says ("Open ten to four").
pub fn night_lock(v: &View<'_>, p: &Prop) -> Option<jane_core::NightLock> {
    use jane_sim::state::NightState;
    match p.night {
        NightState::AsSpawned => v.prop_spawn(p).and_then(|s| s.night_lock),
        NightState::Locked(l) => Some(l),
        NightState::Open => None,
    }
}

/// Is this door shut to her at `hour` (inside its hours, and not a keyed lock she holds the key
/// to)?
pub fn shut_at(v: &View<'_>, p: &Prop, hour: u8) -> bool {
    night_lock(v, p).is_some_and(|l| l.shut_at(hour) && !(l.keyed && keyed_for(v, p)))
}

/// Does she hold a key whose tag fits this prop's lock?
pub fn keyed_for(v: &View<'_>, p: &Prop) -> bool {
    let cat = jane_data::catalog();
    let Some(jane_core::Key::Name(tag)) = v.prop_spawn(p).and_then(|s| s.key_tag) else { return false };
    v.me().bag.iter().flatten().any(|s| cat.combat.item(s.item).opens == Some(tag))
}

/// Hours until some door here into `z` is answered, when every one is shut now (0: one is open
/// now, or there is none here to wait for).
pub fn hours_till_open(v: &View<'_>, z: jane_core::ZoneId) -> u8 {
    let doors = doors_to(v, z);
    let hour = v.hour();
    (0..24u8).find(|&h| doors.iter().any(|p| !shut_at(v, p, (hour + h) % 24))).unwrap_or(0)
}

/// Free bag slots below which she throws something out.
pub const BAG_SPARE: usize = 2;

/// A bag slot to empty when fewer than [`BAG_SPARE`] are free: of what destroy allows (found
/// again somewhere, not bound, not a key) and no quest in the log wants, the least use: first
/// what nothing is made from or mended with, then ingredients, then food and potions, then the
/// wood and iron broken things want; the smaller stack first.
pub fn junk_slot(v: &View<'_>) -> Option<u8> {
    let cat = jane_data::catalog();
    let bag = &v.me().bag;
    if bag.iter().filter(|s| s.is_none()).count() >= BAG_SPARE {
        return None;
    }
    let wanted: Vec<ItemId> = v
        .quests()
        .flat_map(|q| cat.story.quest(q.quest).requirements.iter())
        .filter_map(|r| match r.target {
            jane_data::ReqTarget::Acquire(i) => Some(i),
            _ => None,
        })
        .collect();
    let rank = |i: ItemId| -> u8 {
        let d = cat.combat.item(i);
        let mends = matches!(d.id, "wood" | "iron");
        let ingredient = cat.combat.recipes.iter().any(|r| r.inputs.contains(&i));
        if mends {
            3
        } else if d.usable {
            2
        } else {
            u8::from(ingredient)
        }
    };
    bag.iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|s| (i, s)))
        .filter(|(_, s)| {
            let d = cat.combat.item(s.item);
            !d.kept() && !d.story && !wanted.contains(&s.item)
        })
        .min_by_key(|(i, s)| (rank(s.item), s.qty, *i))
        .map(|(i, _)| i as u8)
}

pub fn holds(v: &View<'_>, item: ItemId) -> u32 {
    jane_sim::bag::bag_count(&v.me().bag[..], item)
}

pub fn item(name: &str) -> ItemId {
    jane_data::catalog().combat.item_id(name).unwrap_or_else(|| panic!("no item {name}"))
}

pub fn spell(name: &str) -> SpellId {
    jane_data::catalog().combat.spell_id(name).unwrap_or_else(|| panic!("no spell {name}"))
}

pub fn knows(v: &View<'_>, s: SpellId) -> bool {
    v.learned().contains(&s) || jane_data::catalog().combat.unit(v.body().def).book.contains(&s)
}

/// Hostile to the party.
pub fn hostile(u: &Unit) -> bool {
    u.faction != Faction::Friendly
}

/// Enemies alive and awake here, nearest first.
pub fn enemies<'a>(v: &View<'a>) -> Vec<&'a Unit> {
    let at = v.body().pos;
    let mut out: Vec<&Unit> = v.units_in(everywhere(v)).map(|u| u.unit).filter(|u| u.alive && hostile(u)).collect();
    out.sort_by_key(|u| (jane_core::num::dist_sq(u.pos, at), u.id));
    out
}

/// Hp as permille of the most.
pub fn hp_permille(u: &Unit) -> i32 {
    let max = jane_sim::units::max_hp(u).0.max(1);
    (i64::from(u.hp.0) * 1000 / i64::from(max)) as i32
}

pub fn mp(u: &Unit) -> i32 {
    u.mp.0
}

/// A unit's centre distance from her, `Fx`.
pub fn dist_to(v: &View<'_>, at: Vec2) -> i64 {
    crate::nav::dist(v.body().pos, at)
}

/// Every action of a list, looking into `If` branches and a `Send`'s list. `lists` resolves a
/// list: `View::list` in the zone, the catalog's alone for another zone.
pub fn visit<'l>(lists: &impl Fn(ListRef) -> &'l [Action], r: ListRef, f: &mut impl FnMut(&Action)) {
    visit_depth(lists, r, f, 0);
}

fn visit_depth<'l>(lists: &impl Fn(ListRef) -> &'l [Action], r: ListRef, f: &mut impl FnMut(&Action), depth: u8) {
    if depth > 6 {
        return;
    }
    for a in lists(r) {
        f(a);
        match *a {
            Action::If { then, els, .. } => {
                visit_depth(lists, then, f, depth + 1);
                if let Some(e) = els {
                    visit_depth(lists, e, f, depth + 1);
                }
            }
            Action::Send { then: Some(t), .. } => visit_depth(lists, t, f, depth + 1),
            _ => {}
        }
    }
}

/// Every action a conversation can run (a node's list or an option's).
pub fn visit_tree<'l>(lists: &impl Fn(ListRef) -> &'l [Action], tree: DialogueId, f: &mut impl FnMut(&Action)) {
    let t = jane_data::catalog().story.dialogue(tree);
    for n in t.nodes {
        if let Some(l) = n.actions {
            visit(lists, l, f);
        }
        for o in n.options {
            if let Some(l) = o.actions {
                visit(lists, l, f);
            }
        }
    }
}

/// The catalog's lists alone (a blueprint's own lists read as empty).
pub fn catalog_lists(r: ListRef) -> &'static [Action] {
    jane_data::catalog().list(r)
}

/// Does this action list do `pred` anywhere, looking into `If` branches?
pub fn list_has(v: &View<'_>, r: ListRef, pred: &impl Fn(&Action) -> bool) -> bool {
    let mut hit = false;
    visit(&|l| v.list(l), r, &mut |a| hit |= pred(a));
    hit
}

/// Does a conversation do `pred` anywhere (a node's list or an option's)?
pub fn tree_has(v: &View<'_>, tree: DialogueId, pred: &impl Fn(&Action) -> bool) -> bool {
    let mut hit = false;
    visit_tree(&|l| v.list(l), tree, &mut |a| hit |= pred(a));
    hit
}

/// Every action a prop can run when used or read.
pub fn visit_prop(v: &View<'_>, p: &Prop, f: &mut impl FnMut(&Action)) {
    let Some(s) = v.prop_spawn(p) else { return };
    if let Some(l) = s.use_list {
        visit(&|l| v.list(l), l, f);
    }
    if let Some(t) = s.talk {
        visit_tree(&|l| v.list(l), t, f);
    }
}

/// A spine action: it moves the story (gives, hands in, teaches, grows, or marks a place).
pub fn spine(a: &Action) -> bool {
    matches!(a, Action::Quest(_) | Action::HandIn(_) | Action::Learn(_) | Action::Grow { .. } | Action::Location(_))
}

/// What a prop does when used or read, as far as its rows say: does it do `pred`?
pub fn prop_does(v: &View<'_>, p: &Prop, pred: &impl Fn(&Action) -> bool) -> bool {
    let Some(s) = v.prop_spawn(p) else { return false };
    s.use_list.is_some_and(|l| list_has(v, l, pred)) || s.talk.is_some_and(|t| tree_has(v, t, pred))
}

/// The unit def whose conversation is `tree`.
pub fn talker_of(tree: DialogueId) -> Option<UnitDefId> {
    let cat = jane_data::catalog();
    cat.combat.units.iter().position(|u| u.talk == Some(tree)).map(|i| UnitDefId(i as u16))
}

/// Friendly units here that talk, nearest first.
pub fn talkers<'a>(v: &View<'a>) -> Vec<&'a Unit> {
    let cat = jane_data::catalog();
    let at = v.body().pos;
    let me = v.me().unit;
    let mut out: Vec<&Unit> = v
        .units_in(everywhere(v))
        .map(|u| u.unit)
        .filter(|u| u.id != me && u.alive && !hostile(u) && cat.combat.unit(u.def).talk.is_some())
        .collect();
    out.sort_by_key(|u| (jane_core::num::dist_sq(u.pos, at), u.id));
    out
}

/// The walkable point just outside a prop's side nearest her (where to stand to reach it).
pub fn bench_side(v: &View<'_>, p: &Prop) -> Vec2 {
    crate::task::sides(v, p, v.body().pos).first().map_or_else(|| prop_centre(p), |s| s.0)
}

/// Is `id` the body of some seat?
pub fn is_player(v: &View<'_>, id: UnitId) -> bool {
    v.body().id == id
        || v.units_in(everywhere(v)).any(|u| u.unit.id == id && u.unit.controller == jane_data::Controller::Player)
}
