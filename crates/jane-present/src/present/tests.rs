//! `Present`'s tests that run it whole: the atlas, carried things, drops, and a console's chunks
//! as it walks, sprints, comes out of a house and enters a zone.

use jane_sim::input::{InputFrame, StepInput};
use jane_sim::state::{LootState, Prop};
use jane_sim::{Seat, Sim};

use super::*;

/// The atlas's bytes, every layer of every page, for the low-RAM budget (ART-PLAN §9): run
/// with `--nocapture` to read them.
#[test]
#[ignore = "builds every tier's art; slow in debug"]
fn atlas_bytes() {
    for tier in [Tier::T0, Tier::T2] {
        let p = Present::new(tier);
        let a = p.atlas();
        let bytes: usize = a
            .pages
            .iter()
            .map(|pg| {
                pg.albedo.len() * 2 + pg.normal.len() * 2 + pg.emissive.len() * 2 + pg.height.len() + pg.glow.len() * 8
            })
            .sum();
        let rows: usize = a.pages.iter().map(|pg| usize::from(pg.h)).sum();
        println!("atlas {tier:?}: {} pages, {rows} rows, {bytes} bytes", a.pages.len());
    }
}

/// The owner's playtest, 2026-10-07: a rock picked up never looked picked up. Carried, it
/// rises over [`CARRY_BEAT`] ticks from where it lay to over her head, follows her there in
/// front of her, and let go comes down over as many to where the sim put it.
#[test]
fn a_carried_rock_is_lifted_over_her_head_and_set_down_again() {
    let cat = jane_data::catalog();
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas((768, 432));
    let step = |sim: &mut Sim, p: &mut Present| {
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        let events = sim.drain_events().to_vec();
        p.tick(&sim.view(Seat(0)).expect("seat 0 plays"), &events);
    };
    step(&mut sim, &mut p);
    let (zone, me, (cx, cy), feet) = {
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        let b = v.body();
        (v.zone(), b.id, b.pos.cell(), (b.pos.x.0 >> FX_TO_CANVAS, b.pos.y.0 >> FX_TO_CANVAS))
    };
    let rock = cat.story.prop_id("rock").expect("the rock row");
    let st = sim.state_mut();
    let id = st.next.prop();
    let key = st.syms.intern("test_rock");
    let cell = jane_core::Cell::new(cx as u16 + 2, cy as u16);
    st.zone_mut(zone).expect("her zone").props.push(Prop {
        id,
        key,
        def: rock,
        spawn: None,
        cell,
        solid: false,
        hidden: false,
        locked: false,
        used: false,
        on: false,
        under_done: false,
        more: jane_core::Rare::empty(),
    });
    sim.rebuild_runtimes();
    step(&mut sim, &mut p);
    let rec = |p: &Present| *p.props.iter().find(|r| r.id == id.get()).expect("the rock is drawn");
    let lying = (i32::from(cell.x) * CELL, i32::from(cell.y) * CELL);
    assert_eq!((rec(&p).x, rec(&p).y, rec(&p).lift, rec(&p).follows), (lying.0, lying.1, 0, None));
    let carry = |sim: &mut Sim, c: Option<jane_sim::PropId>| {
        sim.state_mut().zone_mut(zone).and_then(|z| z.unit_mut(me)).expect("her body").carrying = c;
    };
    // Lifted: on its way up after a tick, held over her head after the beat.
    carry(&mut sim, Some(id));
    step(&mut sim, &mut p);
    let r = rec(&p);
    assert!(r.lift > 0 && r.lift < CARRY_HELD && r.follows == Some(me.get()), "rising: {r:?}");
    for _ in 0..CARRY_BEAT {
        step(&mut sim, &mut p);
    }
    let r = rec(&p);
    assert_eq!((r.x + r.w / 2, r.y + r.h - 1, r.lift), (feet.0, feet.1, CARRY_HELD), "held over her: {r:?}");
    assert_eq!(r.sort_foot, Some(feet.1 + 1), "in front of her");
    // Put down where the sim puts it: on its way down after a tick, on the ground after the beat.
    carry(&mut sim, None);
    step(&mut sim, &mut p);
    let r = rec(&p);
    assert!(r.lift > 0 && r.lift < CARRY_HELD && r.follows.is_none(), "coming down: {r:?}");
    for _ in 0..CARRY_BEAT {
        step(&mut sim, &mut p);
    }
    let r = rec(&p);
    assert_eq!((r.x, r.y, r.lift), (lying.0, lying.1, 0), "down where it lies");
}

/// A thing left lying is drawn as what it holds, and a drop as its item: Mrs Bettany's key
/// was a note on the grass (the owner's first playtest). Every key item has a ground look
/// that is not the note's, and the note comes back once the key is taken.
#[test]
fn a_key_on_the_ground_is_drawn_as_a_key() {
    let cat = jane_data::catalog();
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas((768, 432));
    let step = |sim: &mut Sim, p: &mut Present| {
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        let events = sim.drain_events().to_vec();
        p.tick(&sim.view(Seat(0)).expect("seat 0 plays"), &events);
    };
    step(&mut sim, &mut p);
    let (zone, (cx, cy)) = {
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        (v.zone(), v.body().pos.cell())
    };
    let key = cat.combat.item_id("bettany_key").expect("Mrs Bettany's key");
    let key_look = p.kit.loot_look(cat.combat.item(key).icon).expect("a key's ground look");
    let lost = cat.story.prop_id("lost_thing").expect("the lost thing row");
    let note = p.kit.look(cat.story.prop(lost).sprite, 1, props::State::default());
    assert_ne!(Some(key_look), note, "a key is not a note");
    for item in cat.combat.items.iter().filter(|i| i.id.starts_with("key_") || i.id.ends_with("_key")) {
        let look = p.kit.loot_look(item.icon).unwrap_or_else(|| panic!("{}: no ground look", item.id));
        assert_ne!(Some(look), note, "{} lies as a key", item.id);
    }
    // The lost thing, holding the key, two cells east of her; and a dropped key beyond it.
    let st = sim.state_mut();
    let id = st.next.prop();
    let key_sym = st.syms.intern("test_lost_key");
    let mut lost_key = Prop {
        id,
        key: key_sym,
        def: lost,
        spawn: None,
        cell: jane_core::Cell::new(cx as u16 + 2, cy as u16),
        solid: false,
        hidden: false,
        locked: false,
        used: false,
        on: false,
        under_done: false,
        more: jane_core::Rare::empty(),
    };
    lost_key.loot = LootState::Left(vec![jane_core::Stack { item: key, qty: 1 }]);
    st.zone_mut(zone).expect("her zone").props.push(lost_key);
    let drop = st.next.drop();
    let (born, pos) = (st.tick, jane_core::Vec2::centre(cx + 4, cy));
    st.zone_mut(zone).expect("her zone").drops.push(jane_sim::state::Drop { id: drop, item: key, qty: 1, pos, born });
    sim.rebuild_runtimes();
    step(&mut sim, &mut p);
    let rec = |p: &Present, id: u32| p.props.iter().find(|r| r.id == id).map(|r| r.look);
    assert_eq!(rec(&p, id.get()), Some(key_look), "the lost thing lies as the key it holds");
    assert_eq!(rec(&p, DROP_KEY | drop.get()), Some(key_look), "the dropped key lies as a key");
    // Taken: the lost thing is its own sprite again, and the drop is gone.
    let zs = sim.state_mut().zone_mut(zone).expect("her zone");
    let ix = zs.prop_ix(id).expect("the lost thing") as usize;
    zs.props[ix].loot = LootState::Left(Vec::new());
    zs.drops.clear();
    sim.rebuild_runtimes();
    step(&mut sim, &mut p);
    assert_eq!(rec(&p, id.get()), note);
    assert_eq!(rec(&p, DROP_KEY | drop.get()), None);
}

/// A console's cache (12 slots) walked across the county by a painter that lands a job a
/// tick or a few after it is taken (PLAN-psp-v041 B): the paint band is held to the slots, and
/// a chunk on screen is never let go, so none is swatched twice.
#[test]
fn a_console_walking_never_loses_a_chunk_on_screen() {
    for delay in [1, 6] {
        console_walk(delay);
    }
}

/// A console in and out of a house and the cellar (the bench's churn): back on the county,
/// nothing is read or painted again on the game's thread, as it was (PSP v0.5.0: 1.5 to
/// 3.6 s), and what it did not keep stands in, in the county's own ground.
#[test]
fn a_console_coming_out_of_a_house_finds_the_county_as_it_left_it() {
    use jane_sim::input::DevOp;
    let tables = Present::new(Tier::T0).tables();
    let mut p = Present::from_tables_console(Tier::T0, &tables, 12).expect("the tables read back");
    let bps = jane_sim::Blueprints::build(1).expect("seed 1 builds").packed();
    let mut sim = Sim::new_game_with(bps, "Jane");
    let plan = [
        (30, ZoneId::House, "front"),
        (240, ZoneId::County, "house_front"),
        (450, ZoneId::Cellar, "stair_a"),
        (660, ZoneId::County, "house_front"),
    ];
    let (mut pending, mut seq, mut back) = (None::<(crate::terrain::PaintJob, u32)>, 0u16, 0);
    for t in 0..900u32 {
        let mut cmds = Vec::new();
        if let Some(&(_, zone, m)) = plan.iter().find(|e| e.0 == t) {
            let mark = sim.view(Seat(0)).and_then(|v| v.sym(m)).expect("a mark");
            seq += 1;
            let cmd = jane_sim::Command::Dev(DevOp::Tp { zone, mark });
            cmds.push(jane_sim::StampedCommand { seat: Some(Seat(0)), seq, cmd });
        }
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
        let evs = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        if pending.as_ref().is_some_and(|&(_, due)| t >= due) {
            p.land(pending.take().expect("a job").0);
        }
        let (zone_was, landed) = (p.zone.map(|z| z.0), p.chunks_landed());
        p.tick(&v, &evs);
        if v.zone() == ZoneId::County && zone_was.is_some_and(|z| z != ZoneId::County) {
            back += 1;
            assert_eq!(p.chunks_landed(), landed, "tick {t}: the county painted again on coming back");
            assert!(p.samples_held() > 0, "tick {t}: the county's samples were let go");
            assert_eq!(p.chunks_mush(), 0, "tick {t}: flat stand-ins on coming back");
        }
        if pending.is_none()
            && let Some(mut job) = p.take_paint_job(&v)
        {
            job.run();
            pending = Some((job, t + 30));
        }
    }
    assert_eq!(back, 2);
    assert!(p.sampled_max_tick <= 1, "{} sampled stand-ins in one tick", p.sampled_max_tick);
}

fn console_walk(delay: u32) {
    use jane_core::Angle;
    let mut k = 0u32;
    let tables = Present::new(Tier::T0).tables();
    let mut p = Present::from_tables_console(Tier::T0, &tables, 12).expect("the tables read back");
    let bps = jane_sim::Blueprints::build(1).expect("seed 1 builds").packed();
    let mut sim = Sim::new_game_with(bps, "Jane");
    let slots = p.chunks.most();
    assert_eq!(slots, 12);
    let (mut pending, mut seen, mut widest) = (None, Vec::<ChunkId>::new(), 0);
    let (mut held_before, mut events) = (Vec::<ChunkId>::new(), 0u32);
    for leg in 0..16 {
        let dir = if leg % 2 == 0 { Angle::EAST } else { Angle::SOUTH };
        for _ in 0..120 {
            sim.step(&StepInput {
                frames: [InputFrame::walk(dir), InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE],
                commands: &[],
            });
            let evs = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).expect("seat 0 plays");
            // The painter takes a few ticks to a job (a slow one: chunks arrive unpainted).
            k += 1;
            if pending.as_ref().is_some_and(|&(_, due)| k >= due) {
                p.land(pending.take().expect("a job").0);
            }
            p.tick(&v, &evs);
            if pending.is_none()
                && let Some(mut job) = p.take_paint_job(&v)
            {
                job.run();
                pending = Some((job, k + delay));
            }
            // What is on screen now, and what the cache held after the tick before.
            let cam = (p.camera.pos.0 >> FX_TO_CANVAS, p.camera.pos.1 >> FX_TO_CANVAS);
            let (x0, y0, x1, y1) = p.chunk_range(cam, Margins::uniform(0)).expect("a zone");
            let on: Vec<ChunkId> =
                (y0..=y1).flat_map(|cy| (x0..=x1).map(move |cx| ChunkId { cx: cx as u16, cy: cy as u16 })).collect();
            for id in &on {
                if held_before.contains(id) {
                    assert!(p.chunks.find(*id).is_some(), "chunk {id:?} on screen was evicted");
                } else {
                    events += 1;
                }
                if !seen.contains(id) {
                    seen.push(*id);
                }
            }
            widest = widest.max(p.band.len());
            assert!(p.band.len() <= slots, "{} chunks in the band", p.band.len());
            held_before = (x0 - 3..=x1 + 3)
                .flat_map(|cx| (y0 - 3..=y1 + 3).map(move |cy| (cx, cy)))
                .filter(|&(cx, cy)| cx >= 0 && cy >= 0)
                .map(|(cx, cy)| ChunkId { cx: cx as u16, cy: cy as u16 })
                .filter(|&id| p.chunks.find(id).is_some())
                .collect();
        }
    }
    // Each swatch is a chunk that came on screen unpainted; the rest were landed by jobs.
    // (Or one painted while the painter was behind, for a chunk about to come on.)
    let swatches = p.chunks_painted() - p.chunks_landed() - p.stand_ins_ahead - p.stand_in_upgrades;
    assert!(swatches <= events, "{swatches} swatches for {events} chunks arriving");
    assert!(seen.len() >= 12, "she walked past only {} chunks", seen.len());
    assert_eq!(widest, slots, "the band never reached the cap, which is no test of it");
    // The landed chunks lent the stand-ins their ground: none on screen now is flat colour.
    assert!(p.samples_held() >= 2, "{} cells sampled", p.samples_held());
    assert_eq!(p.chunks_mush(), 0, "a stand-in on screen is mostly flat colour");
    assert!(p.sampled_max_tick <= 1, "{} sampled stand-ins in one tick", p.sampled_max_tick);
    std::println!("seen {} chunks, band {widest}, {swatches} swatches, {} landed", seen.len(), p.chunks_landed());
}

/// A console sprinting across its baked county (P1c), its loader modelled as the PSP's: a
/// few blob reads a tick (asked chunks first, then those read ahead), one decode a tick,
/// landed the tick after. After the first view no stand-in shows on screen, the painter is
/// handed nothing, and every chunk the loader landed is the console painter's chunk exactly.
#[test]
fn a_console_sprinting_on_baked_ground_shows_no_stand_ins() {
    use crate::chunk_pack::{Baker, Codec, Decoder, Landing};
    use jane_core::Angle;
    let tables = Present::new(Tier::T0).tables();
    let mut p = Present::from_tables_console(Tier::T0, &tables, 12).expect("the tables read back");
    // The PSP's canvas.
    p.set_canvas((480, 272));
    let mut sim = Sim::new_game_with(jane_sim::Blueprints::build(1).expect("seed 1 builds").packed(), "Jane");
    // The bake's own county, as a New Game finds it.
    let pristine = Sim::new_game_with(jane_sim::Blueprints::build(1).expect("seed 1 builds").packed(), "Jane");
    let v0 = pristine.view(Seat(0)).expect("seat 0 plays");
    let mut baker = Baker::new(&v0);
    let (w, h) = v0.size();
    let cells = crate::frame::CHUNK_CELLS as u32;
    let (cols, rows) = (w.div_ceil(cells) as u16, h.div_ceil(cells) as u16);
    p.set_baked(cols, rows);
    let mut blobs: Vec<(ChunkId, Vec<u8>)> = Vec::new();
    let mut blob_of = |baker: &mut Baker, id: ChunkId| {
        if let Some(b) = blobs.iter().find(|b| b.0 == id) {
            return b.1.clone();
        }
        let b = baker.paint(&v0, id).blob(lz4_flex::block::compress);
        blobs.push((id, b.clone()));
        b
    };
    let (mut decoder, mut landing, mut ready) = (Decoder::new(), Landing::new(), None::<ChunkId>);
    let (mut read, mut landed, mut first_view) = (Vec::<ChunkId>::new(), Vec::<ChunkId>::new(), None);
    let (mut jobs, mut seen) = (0, Vec::<ChunkId>::new());
    for leg in 0..12 {
        let dir = [Angle::EAST, Angle::SOUTH, Angle::EAST, Angle::NORTH][leg % 4];
        for _ in 0..150 {
            let mut f = InputFrame::walk(dir);
            f.sprint = true;
            sim.step(&StepInput { frames: [f, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands: &[] });
            let evs = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).expect("seat 0 plays");
            // What the loader decoded last tick lands first, as the game takes it.
            if let Some(id) = ready.take() {
                landing.id = id;
                if p.land_baked(&mut landing) {
                    landed.push(id);
                }
            }
            p.tick(&v, &evs);
            first_view = first_view.or(Some(p.stand_ins_shown));
            jobs += u32::from(p.take_paint_job(&v).is_some());
            // The loader: two blobs read a tick, then one decoded.
            let (wants, ahead) = p.baked_wants();
            let mut reads = 2;
            for &id in wants.iter().chain(ahead) {
                if reads > 0 && !read.contains(&id) {
                    read.push(id);
                    reads -= 1;
                }
            }
            if let Some(&id) = wants.iter().find(|id| read.contains(id)) {
                let blob = blob_of(&mut baker, id);
                decoder.decode_landing(Codec::Lz4, &blob, &mut landing).expect("the blob decodes");
                ready = Some(id);
            }
            read.retain(|id| p.chunks.find(*id).is_none());
            let cam = (p.camera.pos.0 >> FX_TO_CANVAS, p.camera.pos.1 >> FX_TO_CANVAS);
            let (x0, y0, x1, y1) = p.chunk_range(cam, Margins::uniform(0)).expect("a zone");
            for (cx, cy) in (y0..=y1).flat_map(|cy| (x0..=x1).map(move |cx| (cx, cy))) {
                let id = ChunkId { cx: cx as u16, cy: cy as u16 };
                if !seen.contains(&id) {
                    seen.push(id);
                }
            }
        }
    }
    let first = first_view.expect("a tick");
    std::println!(
        "seen {} chunks, {} landed baked, stand-ins {} after the first view's {first}",
        seen.len(),
        p.baked_landed,
        p.stand_ins_shown - first
    );
    assert!(seen.len() >= 20, "she sprinted past only {} chunks", seen.len());
    assert!(p.baked_landed as usize + 12 >= seen.len(), "{} landed baked", p.baked_landed);
    assert_eq!(p.stand_ins_shown, first, "stand-ins on screen after the first view");
    assert_eq!(jobs, 0, "the painter was handed a chunk the bake holds");
    // Each baked chunk still held is the console painter's, bit for bit.
    let mut checked = 0;
    for id in landed {
        if let (Need::Nothing, Some((slot, _))) = (p.chunks.need(id), p.chunks.find(id)) {
            let held = p.terrain.baked(slot, &p.frame.layers[usize::from(slot)]);
            assert!(held == baker.paint(&v0, id), "chunk {id:?} landed baked is not the painter's");
            checked += 1;
        }
    }
    assert!(checked >= 6, "{checked} baked chunks checked");
}

/// A console's baked county gives back to the painter the chunks round a changed cell or span,
/// and a chunk the loader failed on, and keeps the rest.
#[test]
fn a_changed_cell_or_span_takes_its_chunks_from_the_bake() {
    let mut g = crate::chunk_pack::Ground::new(8, 8);
    let id = |cx, cy| ChunkId { cx, cy };
    g.read(1, [(16, 16)].into_iter(), core::iter::empty());
    // Cell (16, 16) is the corner of chunk (1, 1); 8 cells round it reach chunk (0, 0) too.
    for c in [id(0, 0), id(1, 0), id(0, 1), id(1, 1)] {
        assert!(!g.holds(c), "{c:?}");
    }
    assert!(g.holds(id(2, 1)) && g.holds(id(1, 2)));
    // Read again at the same generation: nothing changes; at a new one with no changes, all back.
    g.read(1, core::iter::empty(), core::iter::empty());
    assert!(!g.holds(id(1, 1)));
    g.read(2, core::iter::empty(), [Rect::new(100, 40, 3, 1)].into_iter());
    assert!(g.holds(id(1, 1)) && !g.holds(id(6, 2)) && !g.holds(id(5, 2)) && g.holds(id(7, 2)));
    g.fail(id(3, 3));
    g.read(3, core::iter::empty(), core::iter::empty());
    assert!(!g.holds(id(3, 3)) && g.holds(id(6, 2)));
    assert!(!g.holds(id(8, 0)), "beyond the pack");
}

/// The prop cache changes nothing: a played sequence from day to night, with props toggled
/// and loot taken, gives the same records with it on and off.
#[test]
fn the_prop_cache_gives_what_rebuilding_every_tick_gives() {
    use jane_sim::input::DevOp;
    let mut sim = Sim::new_game(1, "Jane");
    let mark = sim.view(Seat(0)).and_then(|v| v.sym("town_square")).expect("the square's mark");
    let tp = [jane_sim::StampedCommand {
        seat: Some(Seat(0)),
        seq: 1,
        cmd: jane_sim::Command::Dev(DevOp::Tp { zone: ZoneId::County, mark }),
    }];
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &tp });
    sim.state_mut().clock = 17 * jane_sim::tuning::TICKS_PER_HOUR;
    let (mut a, mut b) = (Present::new(Tier::T0), Present::new(Tier::T0));
    b.prop_cache_on = false;
    a.clock = Some(host_us);
    b.clock = Some(host_us);
    let mut seen = 0usize;
    let mut spent = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    for t in 0..1500u32 {
        if t % 100 == 50 {
            // Toggle the nth prop's state, and take a chest's loot, as play would.
            let z = sim.state_mut().zone_mut(ZoneId::County).expect("the county");
            let n = z.props.len();
            for k in 0..4 {
                let q = &mut z.props[(t as usize * 7 + k * 13) % n];
                match k {
                    0 => q.on = !q.on,
                    1 => q.used = !q.used,
                    2 => q.solid = !q.solid,
                    _ => q.loot = jane_sim::state::LootState::Left(Vec::new()),
                }
            }
            sim.rebuild_runtimes();
        }
        sim.step(&StepInput::IDLE);
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        let t0 = std::time::Instant::now();
        a.tick(&v, &events);
        let t1 = std::time::Instant::now();
        b.tick(&v, &events);
        spent.0 += t1 - t0;
        spent.1 += t1.elapsed();
        seen += a.props.len();
        assert_eq!(std::format!("{:?}", a.props), std::format!("{:?}", b.props), "tick {t}: props differ");
    }
    assert!(seen > 1000, "only {seen} prop records over the run");
    std::println!(
        "tick time over 1500 ticks: cached {:?}, rebuilt {:?}; prop scan: cached {} us, rebuilt {} us",
        spent.0,
        spent.1,
        a.prof[10],
        b.prof[10]
    );
}

/// Host microseconds since the first call, for [`Present::clock`] in a test.
fn host_us() -> u32 {
    use std::time::Instant;
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_micros() as u32
}

/// How many chunks of the view are on the console's screen.
fn chunks_on_screen(p: &Present) -> u32 {
    let cam = (p.camera.pos.0 >> FX_TO_CANVAS, p.camera.pos.1 >> FX_TO_CANVAS);
    let (x0, y0, x1, y1) = p.chunk_range(cam, Margins::uniform(0)).expect("a zone");
    ((x1 - x0 + 1) * (y1 - y0 + 1)) as u32
}

/// One presenter tick, and what the game thread painted during it.
fn timed_tick(p: &mut Present, v: &View<'_>, evs: &[Event]) -> (u32, u32, u32) {
    p.prof = [0; 12];
    let landed0 = p.chunks_landed();
    let t0 = std::time::Instant::now();
    p.tick(v, evs);
    let us = t0.elapsed().as_micros() as u32;
    (us, p.prof[2], p.chunks_landed() - landed0)
}

/// A console enters `zone` at `mark`. No tick paints a chunk on the game thread; the painter
/// lands the first view, and what is on screen is painted once it has caught up.
fn enter_capped(p: &mut Present, sim: &mut Sim, zone: ZoneId, mark: &str, seq: &mut u16) {
    use jane_sim::input::DevOp;
    *seq += 1;
    let m = sim.view(Seat(0)).and_then(|v| v.sym(mark)).unwrap_or_else(|| panic!("no mark {mark}"));
    let cmd = jane_sim::Command::Dev(DevOp::Tp { zone, mark: m });
    let cmds = [jane_sim::StampedCommand { seat: Some(Seat(0)), seq: *seq, cmd }];
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
    let evs = sim.drain_events().to_vec();
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    assert_eq!(v.zone(), zone, "she did not arrive in {}", zone.name());
    let (us, paint, landed) = timed_tick(p, &v, &evs);
    let on = chunks_on_screen(p);
    std::println!(
        "entry {zone:?}: tick {us} us, paint {paint} us, landed {landed} of {on} on screen, swatched {}, mush {}",
        p.chunks_waiting(),
        p.chunks_mush()
    );
    // Nothing on the game thread: one chunk of the Library was already more than a quarter
    // of its first view. The painter lands the view over the ticks that follow.
    assert_eq!(landed, 0, "{zone:?}: the entry tick painted {landed} chunks");
    assert!(on >= 4, "{zone:?} shows {on} chunks, too few for the cap to mean anything");
    assert!(landed * 4 <= on, "{zone:?}: {landed} of {on} is more than a quarter of the first view");
    // The painter lands what the entry left, one job a tick, as the console's worker does.
    for _ in 0..16 {
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        if let Some(mut job) = p.take_paint_job(&v) {
            job.run();
            p.land(job);
        }
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        let evs = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        let (us, paint, landed) = timed_tick(p, &v, &evs);
        std::println!("  {zone:?} after: tick {us} us, paint {paint} us, landed {landed}");
        assert_eq!(landed, 0, "{zone:?}: a later tick painted {landed} chunks");
        if p.chunks_waiting() == 0 {
            break;
        }
    }
    let cam = (p.camera.pos.0 >> FX_TO_CANVAS, p.camera.pos.1 >> FX_TO_CANVAS);
    let (x0, y0, x1, y1) = p.chunk_range(cam, Margins::uniform(0)).expect("a zone");
    for cy in y0..=y1 {
        for cx in x0..=x1 {
            let id = ChunkId { cx: cx as u16, cy: cy as u16 };
            assert_eq!(p.chunks.need(id), Need::Nothing, "{zone:?} left {id:?} unpainted");
        }
    }
    assert_eq!(p.chunks_mush(), 0, "{zone:?}: flat stand-ins left once the painter caught up");
}

/// A console entering the house, the cellar, the Arms, the church and the Library paints at
/// most a quarter of that first view on any tick. The PC, on the same Library view, still
/// paints the whole of it.
#[test]
fn a_console_entering_the_library_paints_its_first_view_a_little_at_a_time() {
    let tables = Present::new(Tier::T0).tables();
    let mut p = Present::from_tables_console(Tier::T0, &tables, 12).expect("the tables read back");
    p.set_canvas((480, 272));
    p.clock = Some(host_us);
    let bps = jane_sim::Blueprints::build(1).expect("seed 1 builds").packed();
    let mut sim = Sim::new_game_with(bps, "Jane");
    let mut seq = 0u16;
    // Settle on the county, as a game does before she walks into a door. Its first view
    // is still painted whole: the cap is only for the zones that keep no samples.
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
    let evs = sim.drain_events().to_vec();
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    let (us, paint, landed) = timed_tick(&mut p, &v, &evs);
    std::println!("entry county: tick {us} us, paint {paint} us, landed {landed}");
    assert!(landed >= 4, "the county's first view was capped: {landed}");
    let mut library_on = 0u32;
    for (zone, mark) in [
        (ZoneId::House, "front"),
        (ZoneId::Cellar, "stair_a"),
        (ZoneId::Arms, "entry"),
        (ZoneId::Church, "entry"),
        (ZoneId::Library, "entry"),
    ] {
        enter_capped(&mut p, &mut sim, zone, mark, &mut seq);
        if zone == ZoneId::Library {
            library_on = chunks_on_screen(&p);
        }
    }
    // The PC path: the same view, and the whole first view painted on the tick she arrives.
    let mut pc = Present::from_tables(Tier::T0, &tables).expect("the tables read back");
    pc.set_canvas((480, 272));
    pc.clock = Some(host_us);
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    assert_eq!(v.zone(), ZoneId::Library);
    let (us, paint, landed) = timed_tick(&mut pc, &v, &[]);
    let on = chunks_on_screen(&pc);
    std::println!("entry library pc: tick {us} us, paint {paint} us, landed {landed} of {on}");
    assert!(landed >= on && landed > 0, "the PC paints its first view whole: {landed} of {on}");
    assert_eq!(on, library_on, "the PC and the console are not looking at the same view");
}
