//! The Gold Mine (DUNGEONS.md §3.1): Iron Knuckles under the hoists.
//!
//! The arena is built to be played one way: four broken hoists round the floor, each over a
//! marked patch of ground, and a chest of iron. Mend a hoist (Repair, one iron) and its lever
//! shows; pull the lever while he stands under it and the load comes down on him (a `strike` of
//! the lever's rect, and the `staggered` it names: he cannot move, and takes double). He is slow
//! and she is not, so alone she leads him under and pulls as he arrives. A plain fight with him
//! (he out-hits her four to one) is what is left when the hoists are spent.
//!
//! What the tactic reads is what a player is shown: the boss the room holds, the props that
//! answer Repair and what their use lists do (show something: "There is a lever"), the levers
//! and what theirs do (strike a rect, with an effect). It takes the fight over from
//! [`crate::fight`] once he is on her, and lets go when he is down, or she is.
//!
//! Each frame, in order: step out of a tell and eat when low (the fight's own rules); pull a
//! lever she faces while he stands in its rect; carry on with a chest or a mend under way; open
//! the arena's chest when the hoists want iron she lacks; mend a hoist while he is held or far
//! off; work a lever: wait at it once placed; go to one he is lined up behind (his way to it
//! runs under its load, and she will be there first); else lead him round to the far side of
//! one (its bait point), keeping off him when he is close; and, with no hoist left, the plain
//! fight, swinging while he is staggered.

use jane_core::action::Action;
use jane_core::num::CELL_FX;
use jane_core::{Angle, EffectId, Fx, Rect, Vec2, ZoneId};
use jane_data::Answers;
use jane_sim::ids::PropId;
use jane_sim::interact::FocusRef;
use jane_sim::{Command, InputFrame, Prop, Unit, View};

use crate::Act;
use crate::nav::{Go, dist, walkable};
use crate::sense::{self, holds, prop_centre, rect_centre};
use crate::task::{Ctx, Status, Task, UseProp};

/// What the tactic keeps between frames.
#[derive(Debug, Default)]
pub struct Mine {
    /// A chest being opened, or a hoist being mended (the crawl's own tasks).
    task: Option<Task>,
    /// The lever she stands at, waiting for him.
    placed: Option<Placed>,
    /// The lever whose bait point she is making for, and for how many frames.
    bait: Option<(PropId, u32)>,
    /// On her way to the stove before the big door, and how many times she has gone.
    resting: bool,
    rests: u32,
    /// Frames the tactic has held the fight.
    pub frames: u32,
    /// Levers pulled with him under them.
    pub pulled: u32,
    /// Frames since a lever last came down on him: led round the hoists this long for nothing
    /// (he stops short to wind his blow up, out of the rect), she fights him plainly.
    since_pull: u32,
}

/// Led round the hoists this long with no load on him: the plain fight instead.
const LEVER_PATIENCE: u32 = 45 * 60;

/// At a lever: where she stands, the way she faces, and how long he has stood at her out of
/// its rect.
#[derive(Clone, Debug)]
struct Placed {
    lever: PropId,
    stand: Vec2,
    face: Angle,
    missed: u32,
}

/// A lever that drops a load: its prop, its rect, the effect it lays; where to stand to pull it
/// (the side of it nearest the rect) and the bait point past the rect's far side.
struct Lever<'a> {
    prop: &'a Prop,
    rect: Rect,
    effect: Option<EffectId>,
    stand: Vec2,
    face: Angle,
    bait: Vec2,
}

const CELL: i64 = CELL_FX as i64;

/// Within this of him (centre to centre) she keeps off him unless placed at a lever.
const CLOSE: i64 = 5 * CELL;

impl Mine {
    /// The frame's act while Iron Knuckles is on her; `None` when the tactic has nothing to add.
    pub fn think(&mut self, v: &View<'_>, cx: &mut Ctx) -> Option<Act> {
        if v.zone() != ZoneId::Mine || !v.body().alive {
            self.reset();
            return None;
        }
        // Not on her (or asleep out of her sight, as he is while she walks to the stove): the
        // walk to the stove, if she is on it.
        let Some(k) = boss(v).filter(|k| crate::fight::on_me(v, k)) else {
            self.placed = None;
            self.bait = None;
            return self.prepare(v, cx);
        };
        if matches!(self.task, Some(Task::Use(_))) && self.resting {
            self.task = None;
        }
        self.resting = false;
        self.frames += 1;
        cx.foes.insert(k.id);
        cx.fight.target = Some(k.id);
        let me = v.body().pos;
        if let Some(at) = crate::fight::tell_under(v) {
            return Some(Act::hold(crate::nav::stick(at, me, true)));
        }
        if let Some(c) = crate::fight::eat(v) {
            return Some(Act::press(c));
        }
        let levers = levers(v);
        let staggered = levers.iter().filter_map(|l| l.effect).any(|e| k.statuses.iter().any(|s| s.effect == e));
        let near = dist(k.pos, me) <= CLOSE;

        // A lever she faces with him in its rect: pull.
        self.since_pull += 1;
        if !staggered && levers.iter().any(|l| in_rect(k, l.rect) && focused(v, l.prop.id)) {
            self.pulled += 1;
            self.since_pull = 0;
            self.placed = None;
            return Some(Act::press(Command::Use));
        }
        // A chest or a mend under way runs on while it may (a mend is let go when he comes).
        if let Some(t) = &mut self.task {
            if staggered || !near || matches!(t, Task::Use(_)) {
                match t.tick(v, cx) {
                    Status::Act(a) => return Some(a),
                    Status::Done | Status::Failed(_) => self.task = None,
                }
            } else {
                self.task = None;
            }
        }
        let repair = sense::spell("repair");
        let hoists = hoists(v, k);
        let iron = sense::item("iron");
        let have = holds(v, iron);
        // Iron for the hoists: the arena's chest, while he is held or far off (or when she has
        // none to mend with), so a second try at him has it too.
        let can_mend = hoists.iter().any(|&(_, n)| have >= n);
        let calm = staggered || dist(k.pos, me) > 2 * CLOSE;
        if !hoists.is_empty() && (!can_mend || calm && self.placed.is_none()) {
            if let Some(chest) = iron_chest(v) {
                if let Some(a) = self.start(v, cx, Task::Use(UseProp::new(chest.id))) {
                    return Some(a);
                }
            }
        }
        // A hoist to mend: while he is held, or with nothing to pull, or far enough off to cast
        // in peace.
        let ready = sense::knows(v, repair) && crate::fight::ready(v.body(), repair, v.tick());
        if ready && (staggered || self.placed.is_none() && !near) {
            let mendable = hoists
                .iter()
                .filter(|(p, n)| have >= *n && (staggered || dist(k.pos, prop_centre(p)) > 2 * CLOSE))
                .min_by_key(|(p, _)| (sense::to_prop(p, me), p.id));
            if let Some((p, _)) = mendable {
                if let Some(a) = self.start(v, cx, Task::cast(repair, p.id)) {
                    return Some(a);
                }
            }
        }
        if !levers.is_empty() && self.since_pull < LEVER_PATIENCE {
            return Some(self.work(v, cx, k, staggered, &levers));
        }
        if !levers.is_empty() {
            return Some(kite(v, cx, k, staggered));
        }
        self.placed = None;
        if !hoists.is_empty() {
            // Nothing to pull yet (Repair cooling): fight him plainly while he is close (out of
            // each blow's way as it winds up, a bolt in his recovery), else wait, facing him.
            if !staggered && dist(k.pos, me) < 2 * CLOSE {
                return Some(kite(v, cx, k, staggered));
            }
            return Some(Act::hold(InputFrame { aim: Some(towards(me, k.pos)), ..InputFrame::IDLE }));
        }
        // The hoists are spent: the plain fight.
        Some(kite(v, cx, k, staggered))
    }

    fn reset(&mut self) {
        self.task = None;
        self.placed = None;
        self.bait = None;
        self.resting = false;
        self.since_pull = 0;
    }

    /// Before the big door: with its key in her bag and hurt, the First Aid stove first (the
    /// door drops behind her, and he out-hits her four to one). A few trips at most.
    fn prepare(&mut self, v: &View<'_>, cx: &mut Ctx) -> Option<Act> {
        // Whatever comes at her on the way is the fight's (the walk waits for it).
        if sense::enemies(v).iter().any(|u| crate::fight::on_me(v, u)) {
            return None;
        }
        if self.resting {
            if let Some(t) = &mut self.task {
                match t.tick(v, cx) {
                    Status::Act(a) => return Some(a),
                    Status::Done => {}
                    Status::Failed(_) => self.rests = u32::MAX,
                }
            }
            self.task = None;
            self.resting = false;
            return None;
        }
        if sense::hp_permille(v.body()) >= 850 || self.rests >= 3 {
            return None;
        }
        let me = v.body().pos;
        // At the door (on her way to unlock it), not the moment the key is in her bag: what she
        // meets on the way there would only need mending again.
        if door_she_holds(v).is_none_or(|d| sense::to_prop(d, me) > 16 * CELL) {
            return None;
        }
        let cat = jane_data::catalog();
        let stove = v
            .props()
            .filter(|p| !p.hidden && cat.story.prop(p.def).rest && v.prop_spawn(p).is_some_and(|s| s.talk.is_some()))
            .min_by_key(|p| (sense::to_prop(p, me), p.id))?;
        self.rests += 1;
        let a = self.start(v, cx, Task::Use(UseProp::new(stove.id)));
        self.resting = a.is_some();
        a
    }

    /// Begin a task, if its first frame acts.
    fn start(&mut self, v: &View<'_>, cx: &mut Ctx, mut t: Task) -> Option<Act> {
        match t.tick(v, cx) {
            Status::Act(a) => {
                self.task = Some(t);
                Some(a)
            }
            _ => None,
        }
    }

    /// A frame of working the levers.
    fn work(&mut self, v: &View<'_>, cx: &mut Ctx, k: &Unit, staggered: bool, levers: &[Lever<'_>]) -> Act {
        let me = v.body().pos;
        let him = k.pos;
        // Placed: wait for him, facing the lever. At her and not under it for a while: lead
        // him round again.
        if let Some(p) = &mut self.placed {
            if levers.iter().any(|l| l.prop.id == p.lever) && dist(me, p.stand) <= CELL {
                if !focused(v, p.lever) {
                    return Act::hold(crate::task::nudge(p.face));
                }
                let rect = levers.iter().find(|l| l.prop.id == p.lever).map(|l| l.rect);
                if !staggered && crate::fight::gap(v.body(), k) < CELL && rect.is_some_and(|r| !in_rect(k, r)) {
                    p.missed += 1;
                }
                if p.missed <= 20 {
                    return Act::idle();
                }
            }
            self.placed = None;
        }
        // A lever he is lined up behind: his way to its stand runs under its load, he is not in
        // her way there, and she will be there first (she runs at better than twice his pace).
        let lined = levers
            .iter()
            .filter(|l| {
                let ours = dist(me, l.stand);
                let his = dist(him, l.stand);
                crosses(him, l.stand, l.rect) && ours + 2 * CELL < 2 * his && !beside_path(him, me, l.stand)
            })
            .min_by_key(|l| (dist(me, l.stand), l.prop.id));
        if let Some(l) = lined {
            self.bait = None;
            return match cx.nav.go(v, l.stand, Fx::from_px(2), true) {
                Go::Walk(f) => Act::hold(f),
                Go::Arrived | Go::NoWay => {
                    self.placed = Some(Placed { lever: l.prop.id, stand: l.stand, face: l.face, missed: 0 });
                    Act::hold(crate::task::nudge(l.face))
                }
            };
        }
        // None: close to him, keep off; else to a bait point (the one she can reach far sooner
        // than he can, kept a while), and wait there for him to come round behind its rect.
        if !staggered && dist(him, me) < 3 * CELL {
            return Act::hold(keep_off(v, cx, k));
        }
        let kept = self.bait.filter(|&(id, t)| t < 600 && levers.iter().any(|l| l.prop.id == id));
        let bait = match kept {
            Some((id, t)) => {
                self.bait = Some((id, t + 1));
                levers.iter().find(|l| l.prop.id == id)
            }
            None => {
                let l = levers.iter().min_by_key(|l| (dist(me, l.bait) - 2 * dist(him, l.bait), l.prop.id));
                self.bait = l.map(|l| (l.prop.id, 0));
                l
            }
        };
        match bait.map(|l| cx.nav.go(v, l.bait, Fx::from_px(4), true)) {
            Some(Go::Walk(f)) => Act::hold(f),
            _ => Act::hold(InputFrame { aim: Some(towards(me, him)), ..InputFrame::IDLE }),
        }
    }
}

fn towards(from: Vec2, to: Vec2) -> Angle {
    jane_core::angle::iatan2(to.y.0 - from.y.0, to.x.0 - from.x.0)
}

/// The dungeon's boss, standing (the one its boss room holds).
fn boss<'a>(v: &View<'a>) -> Option<&'a Unit> {
    let def = crate::crawl::boss_of(v.zone())?;
    sense::units_of(v, def).into_iter().find(|u| u.alive && !u.hidden)
}

/// The big door's lock: the Large Mine Key's tag ("Opens the door everyone in the mine stopped
/// opening").
const BIG_DOOR: &str = "mine_boss";

/// The big door (the one the hub looks at), locked, with its key in her bag.
fn door_she_holds<'a>(v: &View<'a>) -> Option<&'a Prop> {
    let cat = jane_data::catalog();
    v.props().filter(|p| p.locked).find(|p| {
        let tag = v.prop_spawn(p).and_then(|s| s.key_tag);
        let Some(jane_core::Key::Name(n)) = tag else { return false };
        cat.name(n) == BIG_DOOR && v.me().bag.iter().flatten().any(|s| cat.combat.item(s.item).opens == Some(n))
    })
}

/// The point `i`/16 of the way from `a` to `b`.
fn along(a: Vec2, b: Vec2, i: i32) -> Vec2 {
    Vec2::new(Fx(a.x.0 + (b.x.0 - a.x.0) * i / 16), Fx(a.y.0 + (b.y.0 - a.y.0) * i / 16))
}

/// Does the straight way from `from` to `to` run through the rect's inside (a cell in from its
/// edge)? Walking at her there, he passes under the load.
fn crosses(from: Vec2, to: Vec2, r: Rect) -> bool {
    let inner = Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2);
    (0..=16).any(|i| {
        let (x, y) = along(from, to, i).cell();
        inner.contains(x, y)
    })
}

/// Is `p` in her way from `a` to `b`: nearer `b` than she is, or within three cells of the far
/// part of the way? (Close behind her is where he should be: she outruns him.)
fn beside_path(p: Vec2, a: Vec2, b: Vec2) -> bool {
    dist(p, b) <= dist(a, b) || (6..=16).any(|i| dist(along(a, b, i), p) < 3 * CELL)
}

/// Is his centre's cell in the rect (as a `strike` counts it)?
fn in_rect(u: &Unit, r: Rect) -> bool {
    let (x, y) = u.pos.cell();
    r.contains(x, y)
}

/// Is USE aimed at this prop now?
fn focused(v: &View<'_>, id: PropId) -> bool {
    v.focus().is_some_and(|f| f.target == FocusRef::Prop(id))
}

/// Levers shown and not yet pulled whose use list drops a load on a rect.
fn levers<'a>(v: &View<'a>) -> Vec<Lever<'a>> {
    let mut out = Vec::new();
    for p in v.props().filter(|p| !p.hidden && !p.used) {
        let Some(s) = v.prop_spawn(p) else { continue };
        let Some(list) = s.use_list else { continue };
        let mut hit = None;
        sense::visit(&|l| v.list(l), list, &mut |a| {
            if let Action::Strike { rect, effect, .. } = *a {
                hit = hit.or(Some((rect, effect)));
            }
        });
        let Some((rect, effect)) = hit else { continue };
        let Some(r) = v.rect(v.key_sym(rect)) else { continue };
        let c = rect_centre(r);
        let Some(&(stand, face)) = crate::task::sides(v, p, c).first() else { continue };
        let bait = beyond(v, stand, c, r);
        out.push(Lever { prop: p, rect: r, effect, stand, face, bait });
    }
    out
}

/// Broken things in his ground (within his leash of home) that, mended, show something (the
/// toast says what: "There is a lever"; what is hidden is not in the view); with the iron each
/// wants.
fn hoists<'a>(v: &View<'a>, k: &Unit) -> Vec<(&'a Prop, u32)> {
    let cat = jane_data::catalog();
    let iron = sense::item("iron");
    let leash = i64::from(cat.combat.unit(k.def).leash.0);
    v.props()
        .filter(|p| !p.hidden && !p.used && !p.on && cat.story.prop(p.def).answers == Some(Answers::Repair))
        .filter(|p| dist(prop_centre(p), k.home) <= leash)
        .filter(|p| sense::prop_does(v, p, &|a| matches!(a, Action::Show(_))))
        .filter_map(|p| {
            let s = v.prop_spawn(p)?;
            let n: u32 = s.needs.iter().filter(|n| n.item == iron).map(|n| u32::from(n.qty)).sum();
            Some((p, n))
        })
        .collect()
}

/// An unopened chest with iron in it, nearest first.
fn iron_chest<'a>(v: &View<'a>) -> Option<&'a Prop> {
    let iron = sense::item("iron");
    let me = v.body().pos;
    v.props()
        .filter(|p| !p.hidden && !p.used && !p.locked)
        .filter(|p| v.prop_spawn(p).is_some_and(|s| s.loot.iter().any(|l| l.item == iron)))
        .min_by_key(|p| (sense::to_prop(p, me), p.id))
}

/// A walkable point on the line from `stand` through the rect's middle, three cells past its far
/// side (or as far along as there is floor).
fn beyond(v: &View<'_>, stand: Vec2, c: Vec2, r: Rect) -> Vec2 {
    let (dx, dy) = (i64::from(c.x.0 - stand.x.0), i64::from(c.y.0 - stand.y.0));
    let len = dist(stand, c).max(1);
    let reach = len + i64::from(r.w.max(r.h)) * CELL / 2 + 3 * CELL;
    for step in (0..=reach).rev().step_by(CELL_FX as usize / 2) {
        let at = Vec2::new(
            Fx((i64::from(stand.x.0) + dx * step / len) as i32),
            Fx((i64::from(stand.y.0) + dy * step / len) as i32),
        );
        let (x, y) = at.cell();
        if walkable(v, x, y) {
            return at;
        }
    }
    c
}

/// The plain fight with a thing she outruns, shut in with it (no backing off out of the room: he
/// stays on her, and would mend at home): staggered, in and swing, he takes double; else frost
/// from range, and off him when he comes close. Only what she presses.
fn kite(v: &View<'_>, cx: &mut Ctx, k: &Unit, staggered: bool) -> Act {
    let cat = jane_data::catalog();
    let me = v.body();
    let d = dist(me.pos, k.pos);
    let dir = towards(me.pos, k.pos);
    let gap = crate::fight::gap(me, k);
    let now = v.tick();
    if staggered && sense::hp_permille(me) > 300 {
        let melee = sense::spell("melee_player");
        if gap <= i64::from(cat.combat.spell(melee).range.0) {
            let cmds = if crate::fight::ready(me, melee, now) {
                vec![Command::Cast { spell: melee, on: Some(k.id) }]
            } else {
                Vec::new()
            };
            return Act { frame: InputFrame { aim: Some(dir), mv_dir: dir, mv_mag: 10, ..InputFrame::IDLE }, cmds };
        }
        return match cx.nav.go(v, k.pos, Fx::from_px(10), true) {
            Go::Walk(f) => Act::hold(InputFrame { aim: Some(dir), ..f }),
            _ => Act::hold(InputFrame { aim: Some(dir), ..crate::nav::stick(me.pos, k.pos, false) }),
        };
    }
    // His blow winding up at her: out of its way first (a hop on its last ticks).
    if let Some(a) = crate::fight::dodge_tell(v, cx) {
        return a;
    }
    let ice = sense::spell("icebolt");
    let def = cat.combat.spell(ice);
    // Up close only while he is not winding up: his blow takes longer than her cast holds her.
    if sense::knows(v, ice)
        && crate::fight::ready(me, ice, now)
        && (d > 3 * CELL || k.feel.windup.is_none())
        && gap <= i64::from(def.range.0) * 9 / 10
        && v.sight(me.pos, k.pos)
    {
        return Act {
            frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
            cmds: vec![Command::Cast { spell: ice, on: Some(k.id) }],
        };
    }
    if staggered || d > 7 * CELL {
        return Act::hold(InputFrame { aim: Some(dir), ..InputFrame::IDLE });
    }
    Act::hold(InputFrame { aim: Some(dir), ..keep_off(v, cx, k) })
}

/// Away from him, inside two thirds of his leash (kited past it he walks home and mends).
fn keep_off(v: &View<'_>, cx: &mut Ctx, k: &Unit) -> InputFrame {
    let me = v.body().pos;
    let leash = i64::from(jane_data::catalog().combat.unit(k.def).leash.0);
    let tether = (leash > 0).then_some((k.home, leash * 2 / 3));
    let far = cx.fight.retreat.filter(|r| {
        dist(*r, k.pos) > dist(me, k.pos) + CELL
            && dist(*r, me) > CELL
            && tether.is_none_or(|(home, radius)| dist(*r, home) <= radius)
    });
    let to = match far {
        Some(r) => r,
        None => match crate::fight::retreat_point(v, me, k.pos, tether) {
            Some(r) => {
                cx.fight.retreat = Some(r);
                r
            }
            None => return InputFrame::IDLE,
        },
    };
    match cx.nav.go(v, to, Fx::from_px(3), true) {
        Go::Walk(f) => f,
        _ => {
            cx.fight.retreat = None;
            crate::nav::stick(me, to, true)
        }
    }
}
