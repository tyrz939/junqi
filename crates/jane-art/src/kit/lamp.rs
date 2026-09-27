//! `lamp` (ART.md §2.3): the county's lights. `on` turns the glass to the lit ramp (or lights
//! the flame) and writes it to the emissive layer; `base` is the same lamp out, its glass cool
//! and dark. The light itself is the renderer's; the sprite only glows.
//!
//! Shapes: `post` (a street lamp), `gas` and `sconce` and `cage` and `miner` (wall lamps, by
//! their mount), `torch`, `brazier`, `great_torch`, `fire`, `signal`. The flame's ramp is the
//! accent: `ember` for a warm flame, `glass` or `sky` for a cold one.

use jane_core::grid::Rect;
use jane_data::Mount;

use super::parts::{self, ao, glass, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let lit = state == State::On;
    let (w, foot) = (k.w, k.foot());
    Some(match k.look.shape {
        "post" => {
            // A street lamp: a fluted iron foot, a slim post, a four-paned lantern with a cap and
            // a finial, a ladder bar under it for the lamplighter.
            let cx = w / 2;
            let top = foot - (k.h - 3).min(50);
            ao(c, cx - 4, cx + 3, foot - 1, 4);
            c.rect_bevel(Rect::new(cx - 3, foot - 5, 6, 5), k.body, 1, Z::new(2, 4));
            c.rect_bevel(Rect::new(cx - 2, foot - 8, 4, 3), k.body, 1, Z::new(3, 4));
            post(c, cx - 1, top + 12, foot - 8, 2, k.body, 3);
            c.hline(cx - 3, cx + 2, top + 14, k.body.at(Tone::Base), 4);
            lantern(c, k, cx, top, lit);
            Stand::Up(&[])
        }
        "gas" | "sconce" | "cage" | "miner" => {
            let (x, y) = wall_point(k);
            wall_lamp(c, k, x, y, lit);
            Stand::Up(&[])
        }
        "torch" => {
            // A torch in an iron stake: a wrapped head, the flame over it when lit, embers when
            // not.
            let cx = w / 2;
            let head = foot - 22;
            ao(c, cx - 3, cx + 2, foot - 1, 4);
            post(c, cx - 1, head + 4, foot - 2, 2, k.trim, 3);
            c.hline(cx - 3, cx + 2, foot - 2, k.trim.at(Tone::Base), 3);
            c.rect_round(Rect::new(cx - 2, head, 4, 5), k.body, 1, 1, Z::new(4, 5));
            c.hline(cx - 2, cx + 1, head + 2, k.body.at(Tone::Shade), 5);
            if lit {
                parts::flame(c, cx, head, 9, 0, 6);
                recolour_flame(c, k);
            } else {
                coals(c, cx - 1, head, 2);
            }
            Stand::Up(&[])
        }
        "brazier" | "great_torch" => {
            // An iron bowl of coals on three legs (a great torch stands it on a tall shaft).
            let cx = w / 2;
            let tall = k.look.shape == "great_torch";
            let bowl_y = if tall { foot - 30 } else { foot - 10 };
            let bw = if tall { 12 } else { 10 };
            ao(c, cx - bw / 2, cx + bw / 2 - 1, foot - 1, 5);
            if tall {
                c.rect_bevel(Rect::new(cx - 5, foot - 4, 10, 4), k.trim, 1, Z::new(2, 4));
                post(c, cx - 2, bowl_y + 4, foot - 4, 4, k.trim, 3);
            } else {
                for (x0, x1) in [(cx - 4, cx - 5), (cx + 3, cx + 4), (cx, cx)] {
                    c.line((x0, bowl_y + 4), (x1, foot - 1), k.trim.at(Tone::Shade), 1, 3);
                }
            }
            c.polygon_lit(&[(cx - bw / 2, bowl_y), (cx + bw / 2 - 1, bowl_y), (cx + bw / 2 - 3, bowl_y + 4), (cx - bw / 2 + 2, bowl_y + 4)], k.trim, 90, Z::flat(5));
            c.hline(cx - bw / 2, cx + bw / 2 - 1, bowl_y, k.trim.at(Tone::Light), 5);
            if lit {
                embers(c, cx - bw / 2 + 2, bowl_y, bw - 4);
                parts::flame(c, cx - 2, bowl_y, if tall { 12 } else { 9 }, 1, 6);
                parts::flame(c, cx + 2, bowl_y, if tall { 9 } else { 6 }, -1, 6);
                recolour_flame(c, k);
            } else {
                coals(c, cx - bw / 2 + 2, bowl_y - 1, bw - 4);
            }
            Stand::Up(&[])
        }
        "fire" => {
            // A campfire: a ring of stones, logs crossed in it; lit, flames and a glow of embers;
            // out, ash and a last red coal.
            let (cx, cy) = (w / 2, foot - 10);
            c.ao_contact(Rect::new(cx - 12, cy - 5, 24, 13), 1);
            ring(c, k, cx, cy);
            c.line((cx - 6, cy + 3), (cx + 5, cy - 2), k.trim.at(Tone::Base), 2, 3);
            c.line((cx - 5, cy - 2), (cx + 6, cy + 3), k.trim.at(Tone::Mid), 2, 3);
            if lit {
                embers(c, cx - 4, cy + 1, 8);
                parts::flame(c, cx - 2, cy + 1, 12, 1, 5);
                parts::flame(c, cx + 2, cy + 1, 9, -1, 5);
                parts::flame(c, cx, cy + 2, 7, 0, 6);
                recolour_flame(c, k);
            } else {
                coals(c, cx - 4, cy, 8);
            }
            Stand::Up(&[])
        }
        "bulb" => {
            // A lamp set in the floor: an iron ring, a glass dome in it, warm when it is lit.
            let cx = w / 2;
            let r = Rect::new(cx - 5, foot - 9, 10, 7);
            c.ellipse(Rect::new(r.x - 1, r.y + 2, r.w + 2, r.h - 1), k.body.at(Tone::Shade), 1);
            if lit {
                c.set_emitting(true);
                c.ellipse_lit(Rect::new(r.x + 1, r.y, r.w - 2, r.h - 1), k.accent, Z::new(2, 3));
                c.set_emitting(false);
            } else {
                c.ellipse(Rect::new(r.x + 1, r.y, r.w - 2, r.h - 1), Ramp::Glass.at(Tone::Shade), 2);
                c.dot(r.x + 3, r.y + 1, Ramp::Glass.at(Tone::Light), 2);
            }
            Stand::Flat(3)
        }
        "beam" => {
            // Light falling through a gap onto the floor: a soft pool with motes in it, in the
            // accent (warm sun or cold moon). By day off-light it is only the floor's dust.
            let r = Rect::new(2, foot - (k.fh * 16) + 3, w - 4, k.fh * 16 - 5);
            if lit {
                // The pool in three bands (a hot core, the light, its rim), the rim broken into
                // two-px clusters toward its edge so it thins without a checker; the floor round
                // it is lit by the light pass, not painted. Motes stand in the shaft over it.
                c.set_emitting(true);
                let (cx2, cy2) = (2 * r.x + r.w, 2 * r.y + r.h);
                for y in r.y..r.bottom() {
                    for x in r.x..r.right() {
                        let (dx, dy) = ((2 * x + 1 - cx2) * 16 / r.w, (2 * y + 1 - cy2) * 16 / r.h);
                        let d = dx * dx + dy * dy;
                        let keep = d < 170 || (d < 256 && super::parts::hash(k.seed, x / 2, y / 2 + 90) % 3 != 0);
                        if keep {
                            let t = if d < 50 { Tone::High } else if d < 150 { Tone::Light } else { Tone::Base };
                            c.fill_rect(Rect::new(x, y, 1, 1), k.accent.at(t), 1);
                        }
                    }
                }
                for i in 0..5 {
                    let hh = super::parts::hash(k.seed, i, 82);
                    let (x, y) = (r.x + 4 + (hh % (r.w - 8) as u32) as i32, r.y + 2 + ((hh >> 8) % (r.h - 6) as u32) as i32);
                    c.fill_rect(Rect::new(x, y, 1, 2), k.accent.at(Tone::Glint), 2);
                }
                c.set_emitting(false);
            } else {
                for i in 0..4 {
                    let hh = super::parts::hash(k.seed, i, 81);
                    let (x, y) = (r.x + 3 + (hh % (r.w - 6) as u32) as i32, r.y + 3 + ((hh >> 8) % (r.h - 6) as u32) as i32);
                    c.fill_rect(Rect::new(x, y, 2, 2), Ramp::Stone.at(Tone::Light), 1);
                }
            }
            Stand::Flat(1)
        }
        "signal" => {
            // A railway signal: a post and a round lamp with its lens, red or green by accent.
            let cx = w / 2;
            let top = foot - (k.h - 3).min(40);
            ao(c, cx - 3, cx + 2, foot - 1, 4);
            post(c, cx - 1, top + 6, foot - 2, 3, k.trim, 3);
            c.disc_lit(cx, top + 3, 4, k.body, Z::flat(6));
            let lens = Rect::new(cx - 2, top + 1, 5, 5);
            if lit {
                c.set_emitting(true);
                c.ellipse_lit(lens, k.accent, Z::flat(7));
                c.set_emitting(false);
            } else {
                c.ellipse(lens, k.accent.at(Tone::Shade), 7);
                c.dot(cx - 1, top + 2, k.accent.at(Tone::Base), 7);
            }
            Stand::Up(&[])
        }
        _ => return None,
    })
}

/// A lantern head on a post at `(cx, top)`: a cap with a finial, four panes of glass between
/// iron bars, a bottom plate.
fn lantern(c: &mut Canvas, k: &Kit, cx: i32, top: i32, lit: bool) {
    let iron = k.body;
    c.disc_lit(cx, top + 1, 1, Ramp::Brass, Z::flat(9));
    c.polyline_fill(&[(cx - 1, top + 2), (cx, top + 2), (cx + 4, top + 5), (cx - 5, top + 5)], iron.at(Tone::Base), 9);
    c.hline(cx - 4, cx - 1, top + 3, iron.at(Tone::Light), 9);
    glass(c, Rect::new(cx - 3, top + 6, 6, 6), lit, 8);
    for x in [cx - 4, cx - 1, cx + 2] {
        c.vline(x, top + 6, top + 11, iron.at(if x < cx { Tone::Base } else { Tone::Shade }), 9);
    }
    c.fill_normal(Rect::new(cx - 4, top + 12, 7, 2), iron.at(Tone::Base), parts::south(), 8);
    c.hline(cx - 4, cx + 2, top + 12, iron.at(Tone::Light), 8);
}

/// Where a wall lamp's fixture hangs, by its mount: on the north wall it shows high on the
/// wall's face, on the east or west at the cell's side, on the south at its foot.
fn wall_point(k: &Kit) -> (i32, i32) {
    let (w, back) = (k.w, k.back());
    match k.look.mount {
        Mount::WallE => (w - 5, back + 4),
        Mount::WallW => (4, back + 4),
        Mount::WallS => (w / 2, k.foot() - 6),
        Mount::WallN | Mount::Floor | Mount::Post => (w / 2, back - 6),
    }
}

/// A wall lamp at `(x, y)` (its glass's middle): a bracket to the wall by its mount, and the
/// shape's head.
fn wall_lamp(c: &mut Canvas, k: &Kit, x: i32, y: i32, lit: bool) {
    let (iron, brass) = (k.body, k.trim);
    // The backplate and bracket, toward the wall it hangs on.
    match k.look.mount {
        Mount::WallE => c.line((x + 2, y - 3), (k.w - 1, y - 3), iron.at(Tone::Base), 1, 6),
        Mount::WallW => c.line((0, y - 3), (x - 2, y - 3), iron.at(Tone::Base), 1, 6),
        Mount::WallS => c.line((x, y + 3), (x, k.foot()), iron.at(Tone::Shade), 1, 3),
        _ => {
            c.rect_bevel(Rect::new(x - 2, y - 8, 4, 6), brass, 1, Z::new(4, 5));
            c.vline(x, y - 3, y - 2, iron.at(Tone::Base), 6);
        }
    }
    match k.look.shape {
        "sconce" => {
            // A brass dish and a candle; the flame when lit.
            c.hline(x - 3, x + 2, y + 3, brass.at(Tone::Light), 6);
            c.hline(x - 2, x + 1, y + 4, brass.at(Tone::Shade), 6);
            c.fill_normal(Rect::new(x - 1, y - 1, 2, 4), Ramp::ClothLinen.at(Tone::Base), parts::south(), 7);
            c.dot(x - 1, y - 1, Ramp::ClothLinen.at(Tone::Light), 7);
            if lit {
                parts::flame(c, x, y - 1, 5, 0, 8);
                recolour_flame(c, k);
            } else {
                c.dot(x, y - 2, Ix::SEAM, 7);
            }
        }
        "cage" => {
            // A bulb in a wire cage.
            let r = Rect::new(x - 2, y - 2, 4, 5);
            if lit {
                c.set_emitting(true);
                c.ellipse_lit(r, k.accent, Z::flat(7));
                c.set_emitting(false);
            } else if k.accent != Ramp::GlassLit {
                c.ellipse(r, Ramp::Glass.at(Tone::Shade), 7);
                c.dot(x - 1, y - 1, Ramp::Glass.at(Tone::Light), 7);
            }
            for dx in [-3, 0, 2] {
                c.vline(x + dx, y - 3, y + 3, iron.at(Tone::Base), 8);
            }
            c.hline(x - 3, x + 2, y + 3, iron.at(Tone::Shade), 8);
            c.hline(x - 3, x + 2, y - 3, iron.at(Tone::Light), 8);
        }
        "miner" => {
            // A miner's lamp on a hook: a brass font, a glass chimney, a brass cap.
            c.fill_normal(Rect::new(x - 2, y + 2, 4, 3), brass.at(Tone::Base), parts::south(), 7);
            c.hline(x - 2, x + 1, y + 2, brass.at(Tone::Light), 7);
            glass(c, Rect::new(x - 1, y - 2, 3, 4), lit, 7);
            if lit {
                recolour_glass(c, k);
            }
            c.hline(x - 2, x + 1, y - 3, brass.at(Tone::Base), 8);
            c.vline(x, y - 6, y - 4, iron.at(Tone::Shade), 8);
        }
        _ => {
            // A gas lamp: a little lantern on the bracket.
            glass(c, Rect::new(x - 2, y - 2, 5, 5), lit, 7);
            if lit {
                recolour_glass(c, k);
            }
            c.hline(x - 3, x + 3, y - 3, iron.at(Tone::Base), 8);
            c.hline(x - 2, x + 2, y - 4, iron.at(Tone::Light), 8);
            c.hline(x - 3, x + 3, y + 3, iron.at(Tone::Shade), 8);
        }
    }
}

/// A cold flame: the lit ramp's pixels in the accent's instead (a blue torch in the Burial).
fn recolour_flame(c: &mut Canvas, k: &Kit) {
    if k.accent == Ramp::Ember || k.accent == Ramp::GlassLit {
        return;
    }
    let to = k.accent;
    c.set_emitting(true);
    c.remap_emitting(|ix| match Ramp::of(ix) {
        Some((Ramp::Ember | Ramp::GlassLit, t)) => to.at(t),
        _ => ix,
    });
    c.set_emitting(false);
}

/// Lit glass in the accent's colour, when the accent is not the warm lit glass.
fn recolour_glass(c: &mut Canvas, k: &Kit) {
    recolour_flame(c, k);
}

/// A row of embers: red and orange coals, the brightest emitting.
fn embers(c: &mut Canvas, x: i32, y: i32, w: i32) {
    c.set_emitting(true);
    for i in 0..w {
        let t = if i % 3 == 1 { Tone::High } else { Tone::Base };
        c.dot(x + i, y, Ramp::Ember.at(t), 6);
    }
    c.set_emitting(false);
}

/// Dead coals: ash and one last red coal.
fn coals(c: &mut Canvas, x: i32, y: i32, w: i32) {
    for i in 0..w {
        c.dot(x + i, y, Ramp::Stone.at(if i % 2 == 0 { Tone::Mid } else { Tone::Shade }), 6);
    }
    c.dot(x + w / 2, y, Ramp::Ember.at(Tone::Shade), 6);
}

/// A fire ring: eight stones round `(cx, cy)`, each a little lit dome.
fn ring(c: &mut Canvas, k: &Kit, cx: i32, cy: i32) {
    for i in 0..10 {
        let a = jane_core::angle::Angle((i * 65536 / 10) as u16);
        let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
        let (x, y) = (cx + ((co * 9) >> 15), cy + ((s * 5) >> 15));
        let h = parts::hash(k.seed, i, 50);
        c.ellipse_lit(Rect::new(x - 2, y - 1, 4 + (h & 1) as i32, 3), k.body, Z::new(2, 3));
    }
    c.retone(k.body, super::HARD);
}
