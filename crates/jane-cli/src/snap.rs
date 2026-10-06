//! `jane play --snap OUT.png`: a picture of the world around her as the sim holds it, drawn
//! straight from the `View` (tiles, props, lights, units). Not the game's renderer (that is
//! `jane-present`, P6): a plain map of what is there, so a headless run can be looked at.

use jane_art::sheet::{Image, label};
use jane_art::{Face, Font};
use jane_core::Rect;
use jane_core::view::{VIEW_H_CELLS, VIEW_W_CELLS};
use jane_data::Faction;
use jane_sim::view::View;

/// Screen px per cell in the picture.
const PX: u32 = 12;

/// Two screens wide, two high, centred on her.
pub fn snap(v: &View<'_>) -> Image {
    let cat = jane_data::catalog();
    let (cw, ch) = (VIEW_W_CELLS as i32 * 2, VIEW_H_CELLS as i32 * 2);
    let (bx, by) = v.body().pos.cell();
    let (zw, zh) = v.size();
    let x0 = (bx - cw / 2).clamp(0, (zw as i32 - cw).max(0));
    let y0 = (by - ch / 2).clamp(0, (zh as i32 - ch).max(0));
    let bar = 22;
    let mut img = Image::new(cw as u32 * PX, ch as u32 * PX + bar, [16, 16, 20, 255]);
    let night = v.is_night() && !v.indoor();
    let dim = |c: [u8; 3]| if night { c.map(|x| (u32::from(x) * 45 / 100) as u8) } else { c };
    for y in 0..ch {
        for x in 0..cw {
            let t = v.tile(x0 + x, y0 + y);
            img.fill(x as u32 * PX, y as u32 * PX, PX, PX, dim(crate::view::tile_rgb(t)));
        }
    }
    let area = Rect::new(x0, y0, cw, ch);
    // Lights first, as a warm or cold wash round each lit prop.
    for p in v.props_in(area) {
        if let Some(l) = v.light_showing(p) {
            let d = cat.story.prop(p.def);
            let (lx, ly) = (i32::from(p.cell.x) - x0, i32::from(p.cell.y) - y0);
            let r = (l.radius.0 / 2048).max(1);
            let rgb = [(l.color >> 16) as u8, (l.color >> 8) as u8, l.color as u8];
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        let (cx, cy) = (lx + i32::from(d.w) / 2 + dx, ly + i32::from(d.h) / 2 + dy);
                        if cx >= 0 && cy >= 0 && cx < cw && cy < ch {
                            let t = crate::view::tile_rgb(v.tile(x0 + cx, y0 + cy));
                            let c = [0, 1, 2].map(|k| u8::midpoint(t[k], rgb[k]));
                            img.fill(cx as u32 * PX, cy as u32 * PX, PX, PX, c);
                        }
                    }
                }
            }
        }
    }
    for p in v.props_in(area) {
        let d = cat.story.prop(p.def);
        let (px, py) = ((i32::from(p.cell.x) - x0).max(0), (i32::from(p.cell.y) - y0).max(0));
        let w = (i32::from(d.w)).min(cw - px).max(1) as u32 * PX;
        let h = (i32::from(d.h)).min(ch - py).max(1) as u32 * PX;
        let rgb = if p.locked {
            [200, 120, 60]
        } else if p.solid {
            [150, 120, 70]
        } else {
            [220, 200, 110]
        };
        img.fill(px as u32 * PX + 2, py as u32 * PX + 2, w.saturating_sub(4), h.saturating_sub(4), dim(rgb));
    }
    let me = v.body().id;
    for u in v.units_in(area) {
        let (ux, uy) =
            (u.unit.pos.x.0 * PX as i32 / 2048 - x0 * PX as i32, u.unit.pos.y.0 * PX as i32 / 2048 - y0 * PX as i32);
        let rgb = if u.unit.id == me {
            [250, 250, 250]
        } else if !u.unit.alive {
            [90, 90, 90]
        } else {
            match u.unit.faction {
                Faction::Friendly => [90, 170, 250],
                _ => [230, 60, 60],
            }
        };
        let r = if u.unit.id == me { 7 } else { 5 };
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    img.set((ux + dx).max(0) as u32, (uy + dy - 4).max(0) as u32, [rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
    }
    let font = Font::build();
    let (clock, day) = v.clock();
    let (h, m) = (
        clock / jane_core::num::TICKS_PER_HOUR,
        clock % jane_core::num::TICKS_PER_HOUR / jane_core::num::TICKS_PER_MINUTE,
    );
    let body = v.body();
    let text = format!(
        "{}  day {day}  {h:02}:{m:02}{}  hp {}/{}  quests done {}",
        v.zone().name(),
        if night { " night" } else { "" },
        body.hp.points(),
        jane_sim::units::max_hp(body).points(),
        v.quests_done().len()
    );
    label(&mut img, &font, 6, ch as u32 * PX + 6, &text, Face::Small, [235, 225, 200]);
    img
}
