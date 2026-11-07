//! The props near the view as the presenter keeps them between ticks (`PropRec`), and the small
//! things that stand on a table's top.

use super::*;

impl Present {
    pub(super) fn read_props(&mut self, view: &View<'_>, area: Rect) {
        let cat = jane_data::catalog();
        // What is carried now, by whom, from where: a new lift rises, a thing let go comes down.
        let feet_of = |u: &jane_sim::state::Unit| (u.pos.x.0 >> FX_TO_CANVAS, u.pos.y.0 >> FX_TO_CANVAS);
        let carried: Vec<(jane_sim::PropId, u32, (i32, i32))> = view
            .units_in(area)
            .filter_map(|uv| uv.unit.carrying.map(|p| (p, uv.unit.id.get(), feet_of(uv.unit))))
            .collect();
        let lying =
            |p: jane_sim::PropId| view.prop(p).map(|p| (i32::from(p.cell.x) * CELL, i32::from(p.cell.y) * CELL));
        for &(p, u, feet) in &carried {
            if !self.carrying.iter().any(|&(q, v)| (q, v) == (p, u)) {
                if let Some(ground) = lying(p) {
                    self.carry_beats.push(CarryBeat { prop: p, start: self.tick, up: true, ground, feet });
                }
            }
        }
        for &(p, u) in &self.carrying {
            if !carried.iter().any(|&(q, _, _)| q == p) {
                let feet =
                    self.units.iter().find(|r| r.id == u).map(|r| (r.cur.0 >> FX_TO_CANVAS, r.cur.1 >> FX_TO_CANVAS));
                if let (Some(ground), Some(feet)) = (lying(p), feet) {
                    self.carry_beats.push(CarryBeat { prop: p, start: self.tick, up: false, ground, feet });
                }
            }
        }
        let tick = self.tick;
        self.carry_beats.retain(|b| tick.wrapping_sub(b.start) < CARRY_BEAT);
        self.carrying.clear();
        self.carrying.extend(carried.iter().map(|&(p, u, _)| (p, u)));
        let beats = &self.carry_beats;
        let beat = |p: jane_sim::PropId, up: bool| {
            beats.iter().rev().find(|b| b.prop == p && b.up == up).map(|b| {
                let t = (tick.wrapping_sub(b.start) + 1) as i32 * 256 / CARRY_BEAT as i32;
                (b, if up { t } else { 256 - t })
            })
        };
        let (props, stand, kit) = (&mut self.props, &self.stand, &self.kit);
        let look_stage = self.terrain.night_stage();
        let houses = self.terrain.houses();
        let lived = &self.lived;
        // A chimney smokes on a house someone lives in (ART-PLAN M1); an empty one's is cold.
        let chimney = jane_art::looks::find("town_chimney").map(|(s, _)| s);
        let room = self.terrain.room().map(|r| r.kind);
        props.clear();
        // What she is reading: the mark over it is down while she does.
        let reading = view.dialogue().and_then(|d| match d.speaker {
            jane_sim::state::Speaker::Prop(id) => Some(id),
            _ => None,
        });
        // The pit she is laying a fire in: USE held on it (`jane_sim::fire::hold`).
        let laying = (view.body().hold > 0)
            .then(|| view.focus())
            .flatten()
            .filter(|f| f.verb == jane_sim::interact::Verb::MakeFire)
            .and_then(|f| match f.target {
                jane_sim::interact::FocusRef::Prop(id) => Some(id),
                _ => None,
            });
        // What the dungeon's theme gathers off its floors into its motif (bones into heaps, ART-PLAN
        // Q5): drawn there, not where it lies, unless it is a thing anyone can use.
        let gathers: Vec<&'static str> = jane_data::dungeon_themes()
            .of(view.zone())
            .map(|t| t.floor.iter().flat_map(|f| f.gathers.iter().copied()).collect())
            .unwrap_or_default();
        // A thing's quest mark (a dialogue tree walked, its conditions asked): on a console each
        // is asked again every `MARKS_EVERY` ticks, its own tick by its id, and all of them at
        // once when something that can change one happens (`marks_stale`).
        let every = if self.deferred { MARKS_EVERY } else { 1 };
        if self.marks_stale || every == 1 {
            self.prop_marks.clear();
            self.marks_stale = false;
        }
        let (marks, marks_next) = (&self.prop_marks, &mut self.marks_next);
        marks_next.clear();
        let clock = self.clock;
        let t_scan = clock.map_or(0, |c| c());
        if !self.prop_cache_on {
            self.prop_cache.clear();
        }
        let pcache = &mut self.prop_cache;
        view.for_props_in(area, &mut self.prop_scratch, |p| {
            let d = cat.story.prop(p.def);
            // What a prop's records depend on, short of the zone and its tiles (the cache is
            // dropped on those): its state, her laying a fire in it, a lamp's light showing,
            // the night's stage. Same key, same records.
            let lit = d.light.is_some() && view.light_showing(p).is_some();
            let key = PropKey {
                cell: (p.cell.x, p.cell.y),
                flags: u8::from(p.on)
                    | u8::from(p.used) << 1
                    | u8::from(p.solid) << 2
                    | u8::from(lit) << 3
                    | u8::from(laying == Some(p.id)) << 4,
                loot: match &p.loot {
                    jane_sim::state::LootState::AsSpawned => None,
                    jane_sim::state::LootState::Left(v) => Some((v.len() as u16, v.first().map(|s| s.item))),
                },
                stages: (look_stage, view.night().stage),
            };
            let pid = p.id.get();
            let slot = pcache.binary_search_by_key(&pid, |e| e.0);
            let built = match slot {
                Ok(i) if pcache[i].1.key == key => pcache[i].1.built,
                _ => {
                    let built = (|| -> Built {
                        if !gathers.is_empty()
                            && d.flat
                            && !p.solid
                            && gathers.contains(&cat.sprites.get(usize::from(d.sprite.0)).copied().unwrap_or(""))
                            && view.prop_loot(p).is_empty()
                            && view.prop_spawn(p).is_none_or(|s| s.talk.is_none() && s.to.is_none())
                        {
                            return Built::SKIP;
                        }
                        // A made fire (`jane_sim::fire`): cold, laid while she lays it, lit, or ash.
                        let fire = d.made.then(|| {
                            use props::FireState;
                            if p.on {
                                FireState::Lit
                            } else if laying == Some(p.id) {
                                FireState::Laid
                            } else if p.used {
                                FireState::Ash
                            } else {
                                FireState::Cold
                            }
                        });
                        // An open gate is a doorway.
                        if d.gate && !p.solid {
                            return Built::SKIP;
                        }
                        // Its look from the kit, in its state (a lamp alight, a chest looted), else its
                        // stand-in.
                        let state = props::State {
                            on: p.on || lit,
                            open: p.used || !matches!(p.loot, jane_sim::state::LootState::AsSpawned),
                        };
                        // A thing left lying (a key, a glove) is drawn as what it holds, not as its row's
                        // sprite: Mrs Bettany's key was a note on the grass (the owner's first playtest).
                        let held = d
                            .shows_loot
                            .then(|| view.prop_loot(p).first())
                            .flatten()
                            .and_then(|s| kit.loot_look(cat.combat.item(s.item).icon));
                        // A room lays a rug under its table, along its bed's foot and before its hearth
                        // (ART-PLAN M4): flat on the floor under everything, nothing a foot meets.
                        // A pub lays none under its tables, a church none at all.
                        let lays = |rug: props::Rug| match room {
                            Some(jane_art::terrain::houses::RoomKind::Julie) => true,
                            Some(jane_art::terrain::houses::RoomKind::Inn) => rug != props::Rug::Table,
                            _ => false,
                        };
                        let mut rug_rec = None;
                        if let Some((rug, look)) = kit.rug(d.sprite, p.id.get()).filter(|r| lays(r.0)) {
                            let (cx, cy, cw, ch) =
                                (i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
                            let (mut x, y, w, h) = match rug {
                                props::Rug::Table => (cx + cw / 2 - 2, cy + ch / 2 - 1, 4, 3),
                                props::Rug::Bed => (cx + cw - 6, cy + ch, 6, 2),
                                props::Rug::Hearth => (cx + cw / 2 - 2, cy + ch, 4, 3),
                            };
                            // Kept off the walls: moved along until both its ends lie on the floor.
                            let solid = |x: i32| view.tile(x, y).flags() & jane_core::tile::F_SOLID != 0;
                            while solid(x) && x < cx + cw {
                                x += 1;
                            }
                            while solid(x + w - 1) && x > cx - w {
                                x -= 1;
                            }
                            rug_rec = Some(PropRec {
                                id: RUG_KEY | p.id.get(),
                                x: x * CELL,
                                y: y * CELL,
                                w: w * CELL,
                                h: h * CELL,
                                look,
                                flat: true,
                                flush: false,
                                on_top: false,
                                surface: None,
                                lift: 0,
                                sort_foot: None,
                                follows: None,
                                mark: None,
                                ring: false,
                                smokes: false,
                            });
                        }
                        let look = held
                            .or_else(|| {
                                // A front door or a chimney on a house with a look is its house's own
                                // (ART-PLAN Q2); St Anne's keeps its oak.
                                let house = houses.at(i32::from(p.cell.x), i32::from(p.cell.y));
                                let house = house.filter(|h| h.kind != jane_art::terrain::houses::Kind::Church);
                                house.and_then(|h| kit.house_look(d.sprite, &h.look()))
                            })
                            .or_else(|| {
                                // A lamp gone wrong burns cold (NIGHT.md §4.5).
                                let wrong =
                                    lit && view.light_showing(p).is_some_and(|l| wrong_lamp(view, p, l, look_stage));
                                wrong.then(|| kit.cold_look(d.sprite)).flatten()
                            })
                            .or_else(|| match fire {
                                Some(f) => kit.fire_look(d.sprite, p.id.get(), f),
                                None => kit.look(d.sprite, p.id.get(), state),
                            })
                            .unwrap_or_else(|| {
                                let look = stand.prop(d.w, d.h, d.flat, d.light.is_some());
                                // A lamp the view says is out has dark glass.
                                if d.light.is_some() && !lit { stand.unlit(look) } else { look }
                            });
                        // Julie's chalk ring on a kept lamp's post, from N2 (NIGHT.md §5.2): a lamp on a
                        // post, lit, its light not a flame's, not gone wrong.
                        let ring = look_stage >= 2
                            && lit
                            && d.w == 1
                            && !d.flat
                            && view.zone() == ZoneId::County
                            && view
                                .light_showing(p)
                                .is_some_and(|l| l.flicker.0 < 200 && !l.cold && !wrong_lamp(view, p, l, look_stage));
                        Built {
                            rug: rug_rec,
                            rec: Some(PropRec {
                                id: p.id.get(),
                                x: i32::from(p.cell.x) * CELL,
                                y: i32::from(p.cell.y) * CELL,
                                w: i32::from(d.w) * CELL,
                                h: i32::from(d.h) * CELL,
                                look,
                                flat: d.flat,
                                flush: false,
                                on_top: {
                                    let t = view.tile(i32::from(p.cell.x), i32::from(p.cell.y));
                                    t.is_roof() || t.flags() & jane_core::tile::F_SOLID != 0
                                },
                                surface: kit.surface(d.sprite),
                                lift: 0,
                                sort_foot: None,
                                follows: None,
                                mark: None,
                                ring,
                                smokes: chimney.is_some_and(|c| c == d.sprite)
                                    && houses
                                        .at(i32::from(p.cell.x), i32::from(p.cell.y))
                                        .is_some_and(|h| lived.contains(&(h.rect.x, h.rect.y))),
                            }),
                        }
                    })();
                    match slot {
                        Ok(i) => pcache[i].1 = PropEntry { key, built },
                        Err(i) => pcache.insert(i, (pid, PropEntry { key, built })),
                    }
                    built
                }
            };
            let Some(rec) = built.rec else { return };
            if let Some(r) = built.rug {
                props.push(r);
            }
            let id = p.id.get();
            let kept = marks.binary_search_by_key(&id, |m| m.0).ok().map(|i| marks[i].1);
            let asked = match kept {
                Some(m) if (id.wrapping_add(tick)) % every != 0 => m,
                _ => view.prop_quest_mark(p),
            };
            marks_next.push((id, asked));
            let mark = if reading == Some(p.id) { None } else { asked };
            let mut rec = rec;
            rec.mark = mark;
            props.push(rec);
        });
        self.prof[10] = self.prof[10].wrapping_add(clock.map_or(0, |c| c()).wrapping_sub(t_scan));
        self.marks_next.sort_unstable_by_key(|m| m.0);
        core::mem::swap(&mut self.prop_marks, &mut self.marks_next);
        stand_on_tops(props);
        // Carried (`Unit::carrying`): held over her head, rising there from where it lay; let go,
        // coming down from there to where it was put. Drawn in front of her, casting nothing.
        for &(pid, unit, feet) in &carried {
            let Some(p) = view.prop(pid) else { continue };
            let d = cat.story.prop(p.def);
            let ix = props.iter().position(|r| r.id == pid.get()).unwrap_or_else(|| {
                let look = kit
                    .look(d.sprite, pid.get(), props::State { on: false, open: false })
                    .unwrap_or_else(|| stand.prop(d.w, d.h, d.flat, false));
                props.push(PropRec {
                    id: pid.get(),
                    x: 0,
                    y: 0,
                    w: i32::from(d.w) * CELL,
                    h: i32::from(d.h) * CELL,
                    look,
                    flat: d.flat,
                    flush: false,
                    on_top: false,
                    surface: None,
                    lift: 0,
                    sort_foot: None,
                    follows: None,
                    mark: None,
                    ring: false,
                    smokes: false,
                });
                props.len() - 1
            });
            let r = &mut props[ix];
            let (ground, t) = beat(pid, true).map_or(((r.x, r.y), 256), |(b, t)| (b.ground, t));
            (r.x, r.y, r.lift) = carry_at(ground, feet, (r.w, r.h), t);
            r.sort_foot = Some(feet.1 + 1);
            r.follows = Some(unit);
            (r.flat, r.on_top, r.mark) = (false, false, None);
        }
        for r in props.iter_mut() {
            let Some((b, t)) = jane_sim::PropId::new(r.id).and_then(|p| beat(p, false)) else { continue };
            if carried.iter().any(|&(p, _, _)| p.get() == r.id) {
                continue;
            }
            (r.x, r.y, r.lift) = carry_at(b.ground, b.feet, (r.w, r.h), t);
            r.sort_foot = Some(r.y + r.h + 1);
        }
        // The Hoar Stone on the stair's block: drawn while any of its height can be on screen,
        // so it shows over the bottom edge before the block does.
        // The regions' landmarks likewise, each as tall as it is (`cues`): the chimney, the
        // spire, the statue.
        let stone = self.cues.stone().map(|(foot, look)| (foot, look, jane_art::far::HOAR_W / 2));
        let tall = self.cues.standing(self.hour).map(|(foot, look, ax, _)| (foot, look, ax));
        for (i, ((sx, sy), look, ax)) in stone.into_iter().chain(tall).enumerate() {
            let r = self.atlas.get(look);
            let reach = Rect::new(area.x - 4, area.y, area.w + 8, area.h + i32::from(r.src.h) / CELL + 1);
            if reach.contains(sx.div_euclid(CELL), sy.div_euclid(CELL)) {
                props.push(PropRec {
                    id: STONE_KEY + i as u32,
                    x: sx - ax,
                    y: sy - CELL,
                    w: i32::from(r.src.w),
                    h: CELL,
                    look,
                    flat: false,
                    flush: false,
                    on_top: true,
                    surface: None,
                    lift: 0,
                    sort_foot: None,
                    follows: None,
                    mark: None,
                    ring: false,
                    smokes: false,
                });
            }
        }
        // What lies on the ground (a creature's loot where it fell) as its item, a cell's
        // footprint round its point, on the ground under everything standing.
        for d in view.drops() {
            let (x, y) = (d.pos.x.0 >> FX_TO_CANVAS, d.pos.y.0 >> FX_TO_CANVAS);
            let inside = area.contains(x.div_euclid(CELL), y.div_euclid(CELL));
            let Some(look) = kit.loot_look(cat.combat.item(d.item).icon).filter(|_| inside) else { continue };
            props.push(PropRec {
                id: DROP_KEY | d.id.get(),
                x: x - CELL / 2,
                y: y - CELL / 2,
                w: CELL,
                h: CELL,
                look,
                flat: true,
                flush: false,
                on_top: false,
                surface: None,
                lift: 0,
                sort_foot: None,
                follows: None,
                mark: None,
                ring: false,
                smokes: false,
            });
        }
        props.sort_unstable_by_key(|p| p.id);
    }
}

/// Each small thing whose footprint lies on a top (a bowl on a table) stands on it: drawn up on
/// the top, as far back on it as its footprint is back on the table's, and sorted just after the
/// table so the table never covers it. Tops are few: each thing is tried against those alone.
fn stand_on_tops(props: &mut [PropRec]) {
    let tops: Vec<Top> = props.iter().filter_map(|t| t.surface.map(|s| (t.x, t.y, t.w, t.h, s))).collect();
    if tops.is_empty() {
        return;
    }
    for p in props.iter_mut().filter(|p| !p.flat && p.surface.is_none()) {
        let Some(&(_, ty, _, th, (front, depth))) = tops
            .iter()
            .find(|&&(tx, ty, tw, th, _)| p.x >= tx && p.y >= ty && p.x + p.w <= tx + tw && p.y + p.h <= ty + th)
        else {
            continue;
        };
        // Its foot's place on the top: a few rows in from the top's front edge for the table's
        // front row, toward its back edge as the footprint goes back (`back` rows of floor
        // behind the table's foot, which its own foot already stands up).
        let back = ty + th - (p.y + p.h);
        p.lift = front + SURFACE_INSET + back * (depth - SURFACE_INSET) / th.max(1) - back;
        p.sort_foot = Some(ty + th + 1);
    }
}

/// A top's footprint `(x, y, w, h)` and its `(front, depth)` rows.
type Top = (i32, i32, i32, i32, (i32, i32));

/// Rows in from a top's front edge that a thing on it stands.
const SURFACE_INSET: i32 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    /// A bowl whose footprint lies on a table's stands on its top: drawn up on the top (further
    /// back for a footprint further back), sorted after the table, throwing no shadow of its own;
    /// a thing beside the table is left alone.
    #[test]
    fn a_bowl_on_a_table_stands_on_its_top() {
        let rec = |x: i32, y: i32, w: i32, h: i32, surface: Option<(i32, i32)>| PropRec {
            id: 0,
            x,
            y,
            w,
            h,
            look: 0,
            flat: false,
            flush: false,
            on_top: false,
            surface,
            lift: 0,
            sort_foot: None,
            follows: None,
            mark: None,
            ring: false,
            smokes: false,
        };
        let mut v = vec![
            rec(96, 176, 48, 32, Some((10, 18))),
            rec(112, 192, 32, 16, None),
            rec(112, 176, 32, 16, None),
            rec(160, 192, 32, 16, None),
        ];
        stand_on_tops(&mut v);
        let table_foot = 176 + 32;
        let (front, back, beside) = (v[1], v[2], v[3]);
        assert_eq!(front.sort_foot, Some(table_foot + 1), "after the table");
        let drawn = |p: PropRec| p.y + p.h - p.lift;
        assert_eq!(drawn(front), table_foot - 10 - SURFACE_INSET, "a few rows onto the top");
        assert!(drawn(back) < drawn(front) && drawn(back) > table_foot - 10 - 18, "further back, still on it");
        assert_eq!((beside.lift, beside.sort_foot), (0, None), "beside the table: on the floor");
    }
}
