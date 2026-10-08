//! A solve from start to finish: the checks before anyone walks (ARCHITECTURE.md §5.3 point 2:
//! the contract's names exist, no key is written twice, every list names things in this
//! blueprint, nothing stands in a wall), the states worked out, then flood and sweep to a fixed
//! point, then what is still out of reach.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use jane_core::blueprint::Blueprint;
use jane_core::grid::Grid;
use jane_core::tile::F_SOLID;

use super::ablate::Grant;
use super::flood::Layers;
use super::model::{KeyTag, MAX_PASSES, MAX_STATES, Options, PropState, Solve, States, Trail, TriggerRow, ZoneRules};
use super::passes::{Pass, gates, hops, ifs, kills, loot, mechanisms, repairs, states, triggers};
use super::report::{BuildInfo, Owner, Report, SolveError, TriggerName, name_of};
use super::rows;

/// The sweep, in order. Each looks at what the last flood reached and does what it can there.
pub(crate) const SWEEP: [(&str, Pass); 8] = [
    ("gates", gates::run),
    ("hops", hops::run),
    ("loot", loot::run),
    ("mechanisms", mechanisms::run),
    ("repairs", repairs::run),
    ("kills", kills::run),
    ("triggers", triggers::run),
    ("ifs", ifs::run),
];

/// Judge a blueprint as its zone's rules say: every name the story leans on is there, and
/// everything required can be reached, from the entrance, by someone who wins every fight.
pub fn validate(bp: &Blueprint, rules: &ZoneRules) -> Report {
    solve(bp, rules, &Options::default())
}

/// [`validate`], asked a particular way: withholding something, keeping a gate shut, from
/// another mark, a piece of a zone, or traced.
pub fn solve(bp: &Blueprint, rules: &ZoneRules, opts: &Options) -> Report {
    let mut s = match Solve::new(bp, rules, opts) {
        Ok(s) => s,
        Err(errors) => return Report { errors, info: BuildInfo::default() },
    };
    s.run();
    s.finish()
}

/// [`solve`], keeping a [`Trail`] of it for [`resolve`] to go on from.
pub fn solve_kept(bp: &Blueprint, rules: &ZoneRules, opts: &Options) -> (Report, Trail) {
    let mut s = match Solve::new(bp, rules, opts) {
        Ok(s) => s,
        Err(errors) => return (Report { errors, info: BuildInfo::default() }, Trail::default()),
    };
    s.trail = Some(Box::new(Trail { prop_ix: s.prop_ix.clone(), states: s.states.clone(), ..Trail::default() }));
    s.run();
    let trail = s.trail.take().map(|t| *t).unwrap_or_default();
    (s.finish(), trail)
}

/// The same report as `solve(bp, rules, opts)`, where `opts` is `base` with more withheld or
/// shut (what `Options::without` makes) and `base` was solved by [`solve_kept`] into `report` and
/// `trail` (C1 and C6 ask this of every lock and lock-in).
///
/// Until the first pass in which the base solve had, worked or opened what is taken away, the
/// solve without it goes exactly as the base did, so it goes on from the trail's copy of the
/// solve as that pass began, with the thing taken away: a key tag, spell or flag from the pass it
/// was first had in (from the start if given at the door); a prop's lists from the pass it was
/// first fired, looted, read or opened, or a flood went on from it into another layer; a gate
/// kept shut from the pass it was first opened, or from the start if it is ever unlocked without
/// being opened. If none of them ever made a difference, the report is the base's.
pub fn resolve(
    bp: &Blueprint,
    rules: &ZoneRules,
    base: &Options,
    report: &Report,
    trail: &Trail,
    opts: &Options,
) -> Report {
    let (w, bw) = (&opts.withhold, &base.withhold);
    let same = opts.entry == base.entry && opts.fragment == base.fragment && opts.trace == base.trace;
    let more = w.keys.starts_with(&bw.keys)
        && w.verbs.starts_with(&bw.verbs)
        && w.flags.starts_with(&bw.flags)
        && w.props.starts_with(&bw.props)
        && opts.shut.starts_with(&base.shut);
    if !same || !more || trail.starts.is_empty() {
        return solve(bp, rules, opts);
    }
    let grants = w.keys[bw.keys.len()..]
        .iter()
        .map(|&k| Grant::Key(k))
        .chain(w.verbs[bw.verbs.len()..].iter().map(|&v| Grant::Verb(v)))
        .chain(w.flags[bw.flags.len()..].iter().map(|&f| Grant::Flag(f)))
        .chain(w.props[bw.props.len()..].iter().map(|&p| Grant::Prop(p)))
        .chain(opts.shut[base.shut.len()..].iter().map(|&p| Grant::Shut(p)));
    // The first pass each grant made a difference in: `Some(0)` from the start, `None` never.
    let fired = |k| report.info.fired(k).map(|p| p + 1);
    let mut from: Option<u16> = None;
    for g in grants {
        let first = match g {
            Grant::Key(tag) => {
                if rules.given_keys.iter().any(|&k| KeyTag::Tag(k) == tag) {
                    return solve(bp, rules, opts);
                }
                trail.keys.get(&tag).copied()
            }
            Grant::Verb(sp) => {
                if rules.given_verbs.as_ref().is_some_and(|v| v.contains(&sp)) {
                    return solve(bp, rules, opts);
                }
                trail.verbs.get(&sp).copied()
            }
            Grant::Flag(f) => trail.flags.get(&f).copied(),
            Grant::Prop(k) => {
                let edge = trail.prop_ix.get(&k).and_then(|i| trail.edges.get(i)).copied();
                [fired(k), edge].into_iter().flatten().min()
            }
            Grant::Shut(k) => {
                let Some(&i) = trail.prop_ix.get(&k) else { continue };
                let unlocked =
                    !bp.props[i].locked || trail.states.governed.iter().flatten().any(|g| g[i].locked == Some(false));
                if unlocked { Some(1) } else { fired(k) }
            }
        };
        if let Some(p) = first {
            from = Some(from.map_or(p, |f| f.min(p)));
        }
    }
    let Some(from) = from else { return report.clone() };
    let pass = from - 1;
    let mut s = Solve::restore(bp, rules, opts, trail, pass);
    for (i, p) in bp.props.iter().enumerate() {
        s.props[i].shut |= opts.shut.contains(&p.key);
        s.props[i].withheld |= w.props.contains(&p.key);
    }
    s.run_from(pass);
    s.finish()
}

impl<'a> Solve<'a> {
    /// Everything that can be checked before anyone walks. Errors here stop the solve.
    pub(crate) fn new(bp: &'a Blueprint, rules: &'a ZoneRules, opts: &'a Options) -> Result<Self, Vec<SolveError>> {
        let cat = jane_data::catalog();
        let mut errors = Vec::new();
        let mut prop_ix = BTreeMap::new();
        for (i, p) in bp.props.iter().enumerate() {
            if let Some(first) = prop_ix.insert(p.key, i) {
                errors.push(SolveError::DuplicateProp(p.key));
                prop_ix.insert(p.key, first);
            }
        }
        let mut unit_keys = BTreeSet::new();
        for u in &bp.units {
            if !unit_keys.insert(u.key) {
                errors.push(SolveError::DuplicateUnit(u.key));
            }
        }
        let c = &rules.contract;
        errors.extend(c.units.iter().filter(|k| !unit_keys.contains(k)).map(|&k| SolveError::MissingUnit(k)));
        errors.extend(c.props.iter().filter(|k| !prop_ix.contains_key(k)).map(|&k| SolveError::MissingProp(k)));
        errors.extend(c.marks.iter().filter(|k| !bp.marks.contains_key(*k)).map(|&k| SolveError::MissingMark(k)));
        errors.extend(c.rects.iter().filter(|k| !bp.rects.contains_key(*k)).map(|&k| SolveError::MissingRect(k)));
        for p in &bp.props {
            if p.def.index() >= cat.story.props.len() {
                errors.push(SolveError::UnknownPropDef { prop: p.key, def: p.def });
            }
            // An item that does not exist is not a quiet mistake: the bag throws on it the moment she opens the chest.
            for st in p.loot.iter().filter(|st| st.item.index() >= cat.combat.items.len()) {
                errors.push(SolveError::UnknownLootItem { prop: p.key, item: st.item });
            }
        }
        for u in bp.units.iter().filter(|u| u.def.index() >= cat.combat.units.len()) {
            errors.push(SolveError::UnknownUnitDef { unit: u.key, def: u.def });
        }

        // The catalog's trigger rows for this zone, then the blueprint's own: the sim's merge. A
        // fragment is not its zone (the TypeScript's harness renamed it `<zone>_room`): the story's
        // rows for the zone look for rects of rooms that are not here, and are not its rows.
        let mut trigger_rows: Vec<TriggerRow> = if opts.fragment {
            Vec::new()
        } else {
            cat.story
                .triggers_in(bp.zone)
                .map(|(id, t)| TriggerRow { name: TriggerName::Catalog(id), t: t.trigger, fired: false })
                .collect()
        };
        for (&k, &t) in &bp.triggers {
            if cat.story.trigger_id(name_of(bp, k)).is_some() {
                errors.push(SolveError::TriggerClash(k));
            } else {
                trigger_rows.push(TriggerRow { name: TriggerName::Blueprint(k), t, fired: false });
            }
        }
        for row in &trigger_rows {
            if !bp.rects.contains_key(&row.t.rect) {
                errors.push(SolveError::TriggerNoRect { trigger: row.name, rect: row.t.rect });
            }
        }

        // Lists the blueprint wrote are rows like any other. Unless this IS only a piece of a zone
        // (`fragment`): a control's list names things in other rooms by definition, and in a
        // fragment those are simply absent. An action that cannot find its prop does nothing.
        let mut lists = Vec::new();
        for (&k, t) in &bp.triggers {
            lists.push((Owner::Trigger(k), t.actions, t.when));
            if let Some(r) = t.reset {
                lists.push((Owner::TriggerReset(k), r, None));
            }
        }
        for p in &bp.props {
            if let Some(u) = p.use_list {
                lists.push((Owner::Prop(p.key), u, None));
            }
            if let Some(r) = p.release {
                lists.push((Owner::PropRelease(p.key), r, None));
            }
        }
        for &(at, list, when) in &lists {
            errors.extend(
                rows::row_faults(bp, cat, list, when).into_iter().map(|fault| SolveError::BadRow { at, fault }),
            );
            if !opts.fragment {
                for (kind, name) in rows::missing_names(bp, cat, list, &prop_ix) {
                    errors.push(SolveError::NoSuchName { at, kind, name });
                }
            }
        }
        for p in &bp.props {
            if let Some(tree) = p.talk.filter(|t| t.index() >= cat.story.dialogue.len()) {
                errors.push(SolveError::UnknownDialogue { prop: p.key, tree });
            }
            // In a fragment a door to another room of the zone leads out of the piece: not an error.
            if let Some(d) = p.to.filter(|d| !opts.fragment && d.zone == bp.zone && !bp.marks.contains_key(&d.mark)) {
                errors.push(SolveError::DoorToNoMark { prop: p.key, mark: d.mark });
            }
        }
        if rules.states.len() > MAX_STATES {
            errors.push(SolveError::TooManyStates(rules.states.len()));
        }
        let entry = opts.entry.iter().chain(&rules.entrances).find_map(|k| bp.marks.get(k));
        if entry.is_none() {
            errors.push(SolveError::NoEntrance);
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let (w, h) = (bp.w() as i32, bp.h() as i32);
        let solid = |x: i32, y: i32| !bp.tiles.inside(x, y) || bp.tile(x, y).flags() & F_SOLID != 0;
        for (&k, m) in &bp.marks {
            if solid(i32::from(m.cell.x), i32::from(m.cell.y)) {
                errors.push(SolveError::MarkInWall(k));
            }
        }
        for u in &bp.units {
            if solid(i32::from(u.cell.x), i32::from(u.cell.y)) {
                errors.push(SolveError::UnitInWall(u.key));
            }
        }
        for p in &bp.props {
            let def = cat.story.prop(p.def);
            if i32::from(p.cell.x) + i32::from(def.w) > w || i32::from(p.cell.y) + i32::from(def.h) > h {
                errors.push(SolveError::PropOutside(p.key));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let w = &opts.withhold;
        let props = bp
            .props
            .iter()
            .map(|p| PropState {
                hidden: p.hidden,
                shut: opts.shut.contains(&p.key),
                withheld: w.props.contains(&p.key),
                ..PropState::default()
            })
            .collect();
        let states = States { flags: rules.states.clone(), ..States::default() };
        let entry = entry.map(|m| (i32::from(m.cell.x), i32::from(m.cell.y))).unwrap_or_default();
        let mut s = Solve {
            bp,
            cat,
            rules,
            opts,
            prop_ix,
            props,
            triggers: trigger_rows,
            keys: BTreeMap::new(),
            verbs: rules.given_verbs.as_ref().map(|v| v.iter().copied().filter(|sp| !w.verbs.contains(sp)).collect()),
            flags: BTreeMap::new(),
            dead: BTreeSet::new(),
            waiting: Vec::new(),
            deferred: Vec::new(),
            layers: Layers::new(bp.w(), bp.h(), states.layers(), opts.trace),
            states,
            hops: Vec::new(),
            entry,
            opened: false,
            pass: 0,
            reached_cells: 0,
            trail: None,
        };
        for &k in &rules.given_keys {
            s.add_key(KeyTag::Tag(k), 99);
        }
        let errors = states::prepare(&mut s);
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(s)
    }

    /// Flood, then keep collecting and spending on what that flood reached until nothing new
    /// happens; flood again only if something that blocks feet changed.
    pub(crate) fn run(&mut self) {
        self.run_from(0);
    }

    /// [`Self::run`] from pass `first` on, with the solve as that pass begins.
    pub(crate) fn run_from(&mut self, first: u16) {
        for pass in first..MAX_PASSES {
            if self.trail.is_some() {
                let snap = self.snapshot();
                if let Some(t) = self.trail.as_mut() {
                    t.starts.push(snap);
                }
            }
            self.pass = pass;
            self.flood_all();
            self.settle();
            if !self.opened {
                break;
            }
        }
    }

    /// Sweep the passes over the current flood until none does anything, or a gate opens.
    pub(crate) fn settle(&mut self) {
        self.opened = false;
        loop {
            let mut progress = false;
            for (_, pass) in SWEEP {
                progress |= pass(self);
            }
            // With states, anything new may be what a control was waiting for (a spell, a part): flood again.
            if self.layers.count > 1 && progress {
                self.opened = true;
            }
            if !progress || self.opened {
                break;
            }
        }
    }

    /// What is still out of reach, and what the solve found.
    pub(crate) fn finish(self) -> Report {
        let bp = self.bp;
        let l = &self.layers;
        let at = |c: jane_core::Cell| l.seen_any(i32::from(c.x), i32::from(c.y));
        let mut errors = Vec::new();
        for (&k, m) in &bp.marks {
            if !at(m.cell) {
                errors.push(SolveError::MarkUnreachable(k));
            }
        }
        for &k in &self.rules.contract.units {
            if bp.units.iter().find(|u| u.key == k).is_some_and(|u| !at(u.cell)) {
                errors.push(SolveError::UnitUnreachable(k));
            }
        }
        for &k in &self.rules.contract.props {
            let Some(&i) = self.prop_ix.get(&k) else { continue };
            let mut shown = false;
            let mut opens = false;
            for s in (0..l.count).filter(|&s| s == 0 || l.reached[s]) {
                shown |= !self.hidden_in(i, s);
                opens |= !self.locked_in(i, s);
            }
            if !shown {
                continue;
            }
            if !l.touches_any(self.ring(i)) {
                errors.push(SolveError::PropUnreachable(k));
            }
            if !opens && self.def(i).gate {
                errors.push(SolveError::GateNeverOpens(k));
            }
        }
        let first_seen = l.first_seen.clone().map(|f| Grid::from_vec(bp.w(), bp.h(), f));
        let info = BuildInfo {
            reached_cells: self.reached_cells,
            passes: self.pass + 1,
            floods: l.floods,
            cells_visited: l.visited,
            layers: (0..l.count).filter(|&s| l.reached[s]).map(|s| s as u8).collect(),
            fired_at: bp.props.iter().zip(&self.props).filter_map(|(p, st)| Some((p.key, st.fired_at?))).collect(),
            verbs: self.verbs.map(|v| v.into_iter().collect()),
            flags: self.flags,
            dead: self.dead.into_iter().collect(),
            first_seen,
        };
        Report { errors, info }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_names_every_pass_once() {
        let names: BTreeSet<&str> = SWEEP.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), SWEEP.len());
    }
}
