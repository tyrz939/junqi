//! The F3 overlay (PRESENTATION.md §3.2): the world's bones drawn over it in canvas space,
//! unlit, under the UI. Keys 1 to 9 toggle its layers while it is up; a legend says which are on.
//! The pointer over a tile, a unit or a prop opens a panel of its fields.
//!
//! Everything is read from the `View` (and the presenter's own camera, chunks and frame); like
//! the rest of `jane-present`, it never reads `GameState`.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_core::tile::{F_BLOCK_LOS, F_OCC, F_PROP_SOLID, F_SOLID, F_WATER};
use jane_core::{Rect as Cells, Vec2};
use jane_sim::View;
use jane_sim::ids::PropIx;
use jane_sim::state::CombatState;

use crate::frame::{CELL, CHUNK_PX, ChunkId, FX_TO_CANVAS, Frame};
use crate::present::Present;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, PanelStyle, Ui, line_h, text_w};
use crate::ui::style::{self, argb};

/// The layers, keyed 1 to 9.
pub const LAYERS: [&str; 9] = ["solid", "chunks", "units", "triggers", "lights", "fog", "rects", "props", "camera"];
/// What F3 shows first: units, triggers, rects, camera.
pub const DEFAULT_LAYERS: u16 = 1 << 2 | 1 << 3 | 1 << 6 | 1 << 8;

/// F3's state.
#[derive(Clone, Debug)]
pub struct WorldDebug {
    pub on: bool,
    pub layers: u16,
    scratch: Vec<PropIx>,
}

impl Default for WorldDebug {
    fn default() -> WorldDebug {
        WorldDebug { on: false, layers: DEFAULT_LAYERS, scratch: Vec::new() }
    }
}

impl WorldDebug {
    pub fn toggle(&mut self, layer: usize) {
        if layer < LAYERS.len() {
            self.layers ^= 1 << layer;
        }
    }

    pub fn has(&self, layer: usize) -> bool {
        self.layers & (1 << layer) != 0
    }
}

fn fx_to_canvas(v: i32) -> i32 {
    v >> FX_TO_CANVAS
}

/// Draws F3 over the world for `v`, the presenter's frame and camera.
pub fn draw(ui: &mut Ui, st: &mut WorldDebug, v: &View<'_>, present: &Present, frame: &Frame) {
    let cam = frame.camera;
    let (cw, ch) = ui.canvas;
    let (zw, zh) = v.size();
    let (cx0, cy0) = (cam.0.div_euclid(CELL), cam.1.div_euclid(CELL));
    let (ncx, ncy) = (cw / CELL + 2, ch / CELL + 2);
    let view_cells = Cells::new(cx0, cy0, ncx, ncy);
    let cat = jane_data::catalog();
    let px = |cx: i32, cy: i32| (cx * CELL - cam.0, cy * CELL - cam.1);
    let outline = |ui: &mut Ui, r: Rect, c: u32| {
        ui.fill(Rect::new(i32::from(r.x), i32::from(r.y), i32::from(r.w), 1), c);
        ui.fill(Rect::new(i32::from(r.x), r.bottom() - 1, i32::from(r.w), 1), c);
        ui.fill(Rect::new(i32::from(r.x), i32::from(r.y), 1, i32::from(r.h)), c);
        ui.fill(Rect::new(r.right() - 1, i32::from(r.y), 1, i32::from(r.h)), c);
    };

    // 1: solid cells, walls and props, water; occupancy.
    if st.has(0) {
        for cy in cy0..cy0 + ncy {
            for cx in cx0..cx0 + ncx {
                let f = v.flags(cx, cy);
                let (x, y) = px(cx, cy);
                if f & F_SOLID != 0 {
                    ui.fill(Rect::new(x, y, CELL, CELL), argb(Ramp::ClothRed.at(Tone::Mid), 70));
                } else if f & F_PROP_SOLID != 0 {
                    ui.fill(Rect::new(x, y, CELL, CELL), argb(Ramp::Copper.at(Tone::Light), 70));
                } else if f & F_WATER != 0 {
                    ui.fill(Rect::new(x, y, CELL, CELL), argb(Ramp::Water.at(Tone::Light), 60));
                }
                if f & F_BLOCK_LOS != 0 && f & F_SOLID == 0 {
                    ui.fill(Rect::new(x + 6, y + 6, 4, 4), argb(Ramp::ClothPlum.at(Tone::Light), 180));
                }
                if f & F_OCC != 0 {
                    outline(ui, Rect::new(x + 1, y + 1, CELL - 2, CELL - 2), argb(style::warn(), 200));
                }
            }
        }
    }

    // 2: chunk borders, id and generation.
    if st.has(1) {
        let (k0x, k0y) = (cam.0.div_euclid(CHUNK_PX), cam.1.div_euclid(CHUNK_PX));
        for ky in k0y..=k0y + ch / CHUNK_PX + 1 {
            for kx in k0x..=k0x + cw / CHUNK_PX + 1 {
                if kx < 0 || ky < 0 || kx * CHUNK_PX >= zw as i32 * CELL || ky * CHUNK_PX >= zh as i32 * CELL {
                    continue;
                }
                let (x, y) = (kx * CHUNK_PX - cam.0, ky * CHUNK_PX - cam.1);
                outline(ui, Rect::new(x, y, CHUNK_PX, CHUNK_PX), argb(Ramp::Sky.at(Tone::Light), 150));
                let id = ChunkId { cx: kx as u16, cy: ky as u16 };
                let s = match present.chunk(id) {
                    Some((slot, gen_)) => format!("{kx},{ky} g{gen_} s{slot}"),
                    None => format!("{kx},{ky} unpainted"),
                };
                ui.text(x + 3, y + 3, &s, Ink::fine(Ramp::Sky.at(Tone::High)).outline());
            }
        }
    }

    // 6: fog-of-war: unseen blocks shaded.
    if st.has(5) {
        let b = v.fog_block().max(1) as i32;
        let mut by = cy0.div_euclid(b) * b;
        while by < cy0 + ncy {
            let mut bx = cx0.div_euclid(b) * b;
            while bx < cx0 + ncx {
                if !v.seen(bx, by) {
                    let (x, y) = px(bx, by);
                    ui.fill(Rect::new(x, y, b * CELL, b * CELL), argb(Ramp::UiSlot.at(Tone::Deep), 110));
                }
                bx += b;
            }
            by += b;
        }
    }

    // 7: named rects.
    if st.has(6) {
        for (name, r) in v.rects() {
            if !r.overlaps(view_cells) {
                continue;
            }
            let (x, y) = px(r.x, r.y);
            let rr = Rect::new(x, y, r.w * CELL, r.h * CELL);
            outline(ui, rr, argb(Ramp::Leaf.at(Tone::Light), 190));
            ui.text(x + 3, y + 3, v.name(name), Ink::fine(Ramp::Leaf.at(Tone::High)).outline());
        }
    }

    // 4: triggers and plates.
    if st.has(3) {
        for (t, fired) in v.triggers() {
            let Some(r) = v.rect(t.rect) else { continue };
            if !r.overlaps(view_cells) {
                continue;
            }
            let (x, y) = px(r.x, r.y);
            let c = if fired { style::dim() } else { Ramp::Bloom.at(Tone::Light) };
            let rr = Rect::new(x, y, r.w * CELL, r.h * CELL);
            ui.fill(rr.inset(1), argb(c, 16));
            outline(ui, rr, argb(c, 220));
            let mode = match t.trigger.mode {
                jane_core::blueprint::TriggerMode::Enter => "enter",
                jane_core::blueprint::TriggerMode::While => "while",
            };
            let label = format!(
                "{} {mode}{}{}",
                v.name(t.rect),
                if t.trigger.once { " once" } else { "" },
                if fired { " fired" } else { "" }
            );
            ui.text(x + 3, rr.bottom() - 13, &label, Ink::fine(c).outline());
        }
    }

    // 8: props, their ids and state; plates marked.
    let mut scratch = std::mem::take(&mut st.scratch);
    if st.has(7) {
        v.for_props_in(view_cells, &mut scratch, |p| {
            let d = cat.story.prop(p.def);
            let (x, y) = px(i32::from(p.cell.x), i32::from(p.cell.y));
            let rr = Rect::new(x, y, i32::from(d.w) * CELL, i32::from(d.h) * CELL);
            let c = if d.plate { Ramp::ClothMustard.at(Tone::High) } else { Ramp::Copper.at(Tone::High) };
            outline(ui, rr, argb(c, 150));
            let mut s = format!("#{} {}", p.id.get(), d.id);
            for (on, w) in [(p.on, " on"), (p.locked, " locked"), (p.used, " used"), (p.solid, " solid")] {
                if on {
                    s.push_str(w);
                }
            }
            ui.text(x + 1, y - 11, &s, Ink::fine(c).outline());
        });
    }

    // 3: units: id, state, path, target, aggro and leash.
    if st.has(2) {
        let (w, _) = v.size();
        for uv in v.units_in(view_cells) {
            let u = uv.unit;
            let d = cat.combat.unit(u.def);
            let (x, y) = (fx_to_canvas(u.pos.x.0) - cam.0, fx_to_canvas(u.pos.y.0) - cam.1);
            let hostile = u.faction != jane_data::Faction::Friendly;
            let c = if !u.alive {
                style::dim()
            } else if hostile {
                Ramp::ClothRed.at(Tone::High)
            } else {
                style::good()
            };
            if hostile && u.alive {
                ring(ui, x, y, fx_to_canvas(d.aggro.0), argb(Ramp::ClothRed.at(Tone::Light), 110));
                if u.combat != CombatState::Idle {
                    let (hx, hy) = (fx_to_canvas(u.home.x.0) - cam.0, fx_to_canvas(u.home.y.0) - cam.1);
                    ring(ui, hx, hy, fx_to_canvas(d.leash.0), argb(style::warn(), 90));
                }
            }
            if let Some(path) = &u.path {
                let mut last = (x, y);
                for &ix in path.cells.iter().skip(usize::from(path.at)) {
                    let (pcx, pcy) = (ix.0 as i32 % w as i32, ix.0 as i32 / w as i32);
                    let (px_, py_) = (pcx * CELL - cam.0 + CELL / 2, pcy * CELL - cam.1 + CELL / 2);
                    line(ui, last, (px_, py_), argb(Ramp::Sky.at(Tone::High), 200));
                    last = (px_, py_);
                }
            }
            if let Some(t) = u.target.and_then(|t| v.unit(t)) {
                let (tx, ty) = (fx_to_canvas(t.pos.x.0) - cam.0, fx_to_canvas(t.pos.y.0) - cam.1);
                line(ui, (x, y - 10), (tx, ty - 10), argb(style::bad(), 200));
            }
            ui.fill(Rect::new(x - 2, y - 2, 5, 5), argb(c, 255));
            let state = match u.combat {
                CombatState::Idle => "idle",
                CombatState::Combat => "combat",
                CombatState::Leash => "leash",
                CombatState::Evade => "evade",
            };
            let s = format!("#{} {} {state}", u.id.get(), d.id);
            ui.text(x - text_w(Face::Fine, &s) / 2, y + 4, &s, Ink::fine(c).outline());
        }
    }

    // 5: lights and casters from the frame; the sun's way.
    if st.has(4) {
        for l in &frame.lights {
            ring(
                ui,
                l.pos.0,
                l.pos.1,
                i32::from(l.radius),
                argb(Ramp::GlassLit.at(Tone::Light), if l.casts { 170 } else { 90 }),
            );
            ui.fill(
                Rect::new(l.pos.0 - 1, l.pos.1 - i32::from(l.height) - 1, 3, 3),
                argb(Ramp::GlassLit.at(Tone::Glint), 255),
            );
            line(ui, l.pos, (l.pos.0, l.pos.1 - i32::from(l.height)), argb(Ramp::GlassLit.at(Tone::Light), 150));
        }
        for c in &frame.casters {
            let (fx, fy) = (i32::from(c.foot.0), i32::from(c.foot.1));
            line(ui, (fx, fy), (fx, fy - i32::from(c.height)), argb(Ramp::ClothPlum.at(Tone::High), 170));
        }
        if let Some(sun) = present.sky().sun {
            let (cxm, cym) = (cw - 40, ch - 60);
            ring(ui, cxm, cym, 18, argb(style::quiet(), 150));
            let a = sun.azimuth;
            let dx = jane_core::angle::cos_q15(a).0 * 18 / 32768;
            let dy = jane_core::angle::sin_q15(a).0 * 18 / 32768;
            line(ui, (cxm, cym), (cxm + dx, cym + dy), argb(Ramp::GlassLit.at(Tone::High), 255));
            ui.text(cxm - 12, cym + 22, "sun", Ink::fine(style::quiet()).outline());
        }
    }

    // 9: the camera's lock and the zone's edge.
    if st.has(8) {
        let r = Rect::new(-cam.0, -cam.1, zw as i32 * CELL, zh as i32 * CELL);
        outline(ui, r, argb(style::gold(), 220));
        if let Some(l) = present.camera().lock {
            let (x, y) = px(l.x, l.y);
            outline(ui, Rect::new(x, y, l.w * CELL, l.h * CELL), argb(style::gold(), 255));
            ui.text(x + 3, y + 3, "camera lock", Ink::fine(style::gold()).outline());
        }
        let s = format!("cam {},{} px", cam.0, cam.1);
        ui.text(6, ch - 50 - LAYERS.len() as i32 * line_h(Face::Fine) - 36, &s, Ink::fine(style::gold()).outline());
    }

    // Hover inspect.
    if let Some(p) = ui.input.pointer {
        let world =
            Vec2 { x: jane_core::Fx((p.0 + cam.0) << FX_TO_CANVAS), y: jane_core::Fx((p.1 + cam.1) << FX_TO_CANVAS) };
        let (hx, hy) = ((p.0 + cam.0).div_euclid(CELL), (p.1 + cam.1).div_euclid(CELL));
        let mut lines: Vec<String> = Vec::new();
        let near = Cells::new(hx - 2, hy - 2, 5, 6);
        let under = v.units_in(near).map(|u| u.unit).min_by_key(|u| {
            let (dx, dy) = (i64::from(u.pos.x.0 - world.x.0), i64::from(u.pos.y.0 - 12 * 128 - world.y.0));
            dx * dx + dy * dy
        });
        if let Some(u) = under.filter(|u| {
            (u.pos.x.0 - world.x.0).abs() < 10 << 8
                && (u.pos.y.0 - world.y.0) < 24 << 8
                && (world.y.0 - u.pos.y.0) < 4 << 8
        }) {
            let d = cat.combat.unit(u.def);
            lines.push(format!("unit #{} {}", u.id.get(), d.id));
            lines.push(format!(
                "hp {} of {}  mp {}",
                u.hp.points(),
                jane_sim::units::max_hp(u).points(),
                u.mp.points()
            ));
            lines.push(format!("{:?} {:?} {:?}", u.faction, u.controller, u.combat));
            lines.push(format!("at {},{} cell  facing {:?}", u.pos.cell().0, u.pos.cell().1, u.facing));
            for s in &u.statuses {
                lines.push(format!("status {} until {}", cat.combat.effect(s.effect).id, s.until.0));
            }
            if let Some(t) = u.target {
                lines.push(format!("target #{}", t.get()));
            }
        } else {
            let mut prop_line = None;
            v.for_props_in(Cells::new(hx, hy, 1, 1), &mut scratch, |pr| {
                let d = cat.story.prop(pr.def);
                if prop_line.is_none() {
                    prop_line = Some(vec![
                        format!("prop #{} {}", pr.id.get(), d.id),
                        format!(
                            "{}x{} solid {} locked {} on {} used {}",
                            d.w, d.h, pr.solid, pr.locked, pr.on, pr.used
                        ),
                        format!(
                            "light {}",
                            v.light_showing(pr).map_or("none".into(), |l| format!("{} px", l.radius.0 >> 8))
                        ),
                    ]);
                }
            });
            if let Some(pl) = prop_line {
                lines.extend(pl);
            }
            let f = v.flags(hx, hy);
            lines.push(format!("cell {hx},{hy}  {:?}", v.tile(hx, hy)));
            lines.push(format!(
                "flags {f:08b}{}{}{}",
                if f & F_SOLID != 0 { " solid" } else { "" },
                if f & F_OCC != 0 { " occ" } else { "" },
                if v.seen(hx, hy) { " seen" } else { "" }
            ));
        }
        let lh = line_h(Face::Fine);
        let w = lines.iter().map(|l| text_w(Face::Fine, l)).max().unwrap_or(40) + 16;
        let h = lines.len() as i32 * lh + 10;
        let (mut x, mut y) = (p.0 + 16, p.1 + 16);
        if x + w > cw {
            x = p.0 - w - 8;
        }
        if y + h > ch {
            y = p.1 - h - 8;
        }
        ui.panel(Rect::new(x, y, w, h), PanelStyle::Debug);
        for (i, l) in lines.iter().enumerate() {
            let ink = if i == 0 { style::gold() } else { style::text() };
            ui.text(x + 8, y + 5 + i as i32 * lh, l, Ink::fine(ink));
        }
        outline(ui, Rect::new(hx * CELL - cam.0, hy * CELL - cam.1, CELL, CELL), argb(style::gold(), 255));
    }
    st.scratch = scratch;

    legend(ui, st);
}

fn legend(ui: &mut Ui, st: &WorldDebug) {
    let (_, ch) = ui.canvas;
    let lh = line_h(Face::Fine);
    let h = LAYERS.len() as i32 * lh + 22;
    let (x, y) = (6, ch - h - 40);
    ui.panel(Rect::new(x, y, 112, h), PanelStyle::Debug);
    ui.text(x + 8, y + 4, "F3 layers", Ink::fine(style::gold()));
    for (i, name) in LAYERS.iter().enumerate() {
        let on = st.has(i);
        let s = format!("{} {name}", i + 1);
        let yy = y + 18 + i as i32 * lh;
        ui.fill(Rect::new(x + 8, yy + 3, 5, 5), argb(if on { style::good() } else { style::dim() }, 255));
        ui.text(x + 18, yy, &s, Ink::fine(if on { style::text_bright() } else { style::dim() }));
    }
}

/// A one-px ring of radius `r` around `(cx, cy)`, drawn as short dashes.
fn ring(ui: &mut Ui, cx: i32, cy: i32, r: i32, c: u32) {
    if r <= 0 {
        return;
    }
    let steps = (r * 2).clamp(16, 256);
    for k in 0..steps {
        if k % 2 == 1 {
            continue;
        }
        let a = jane_core::Angle((k * 65536 / steps) as u16);
        let x = cx + jane_core::angle::cos_q15(a).0 * r / 32768;
        let y = cy + jane_core::angle::sin_q15(a).0 * r / 32768;
        ui.fill(Rect::new(x, y, 1, 1), c);
    }
}

/// A line of one-px fills from `a` to `b`.
fn line(ui: &mut Ui, a: (i32, i32), b: (i32, i32), c: u32) {
    let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
    for k in 0..=n {
        ui.fill(Rect::new(a.0 + (b.0 - a.0) * k / n, a.1 + (b.1 - a.1) * k / n, 1, 1), c);
    }
}
