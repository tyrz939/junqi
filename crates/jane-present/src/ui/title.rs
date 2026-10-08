//! The title (ART.md §7; PRESENTATION.md §3.2): the county at night from across the lake, the
//! School on its hill with its windows lit and one of them flickering, mist moving on the water,
//! fireflies in the reeds; the name in the Title face; New Game (with her name), Continue, Load,
//! Controls, Quit.
//!
//! The still part is painted once per canvas size into a UI image; what moves is drawn over it
//! each frame from the tick, so the title is alive and costs a few fills a frame.

use alloc::string::String;
use alloc::vec::Vec;
use jane_art::font::Face;
use jane_art::hash::h32;
use jane_art::palette::{self, Ix, Ramp, Tone};

use crate::frame::Src;
use crate::input::UiAction;
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, ButtonKind, Ink, PanelStyle, Ui, text_w, wid};
use crate::ui::menus::{self, MenuState};
use crate::ui::style::{self, argb};

/// The UI image slot the backdrop is painted into.
pub const BACKDROP: u16 = 0;
const SALT: u32 = 0x5449_544c;

/// The title's own state.
#[derive(Clone, Debug, Default)]
pub struct TitleState {
    pub menu: MenuState,
    /// The name field is open (New Game was picked).
    pub naming: bool,
    pub name: String,
    pub painted: (i32, i32),
    /// Where the School's lit windows are on the backdrop, for the flicker.
    pub windows: Vec<(i32, i32, i32, i32)>,
}

/// What the title needs to know.
#[derive(Clone, Copy, Debug, Default)]
pub struct TitleInfo {
    /// A save exists: Continue and Load are live.
    pub has_save: bool,
    /// A console (PORT.md §13.13): no Host or Join yet, and no keyboard to name her, so New
    /// Game begins at once with the name she has.
    pub console: bool,
}

/// The horizon line, px from the top, on a canvas `h` tall.
const fn horizon(h: i32) -> i32 {
    h * 62 / 100
}

fn rgb_of(ix: Ix) -> [i32; 3] {
    let [r, g, b] = palette::rgb(ix);
    [i32::from(r), i32::from(g), i32::from(b)]
}

fn mix(a: [i32; 3], b: [i32; 3], t: i32) -> [i32; 3] {
    [a[0] + (b[0] - a[0]) * t / 256, a[1] + (b[1] - a[1]) * t / 256, a[2] + (b[2] - a[2]) * t / 256]
}

fn pack(c: [i32; 3]) -> u32 {
    0xff00_0000 | (c[0].clamp(0, 255) as u32) << 16 | (c[1].clamp(0, 255) as u32) << 8 | c[2].clamp(0, 255) as u32
}

/// A smooth ridge: the height of a silhouette at column `x`, from hashed control points every
/// `step` px, eased between them.
fn ridge(x: i32, step: i32, amp: i32, seed: u32) -> i32 {
    let k = x.div_euclid(step);
    let f = x.rem_euclid(step) * 256 / step;
    let at = |i: i32| (h32(i as u32, seed, SALT) % (amp as u32 + 1)) as i32;
    let (a, b) = (at(k), at(k + 1));
    // Smoothstep in 1/256ths.
    let s = f * f * (768 - 2 * f) / 65536;
    a + (b - a) * s / 256
}

/// Paints the still backdrop at `w x h` into `px` (0xAARRGGBB). Returns the School's lit windows.
pub fn paint(px: &mut [u32], w: i32, h: i32) -> Vec<(i32, i32, i32, i32)> {
    let hz = horizon(h);
    // The sky: indigo overhead through violet to a low ember glow where the town's lamps are,
    // in flat bands with an ordered dither where one meets the next (pixel art's gradient).
    let bands = [
        rgb_of(Ramp::ClothNavy.at(Tone::Deep)),
        rgb_of(Ramp::ClothNavy.at(Tone::Shade)),
        mix(rgb_of(Ramp::ClothNavy.at(Tone::Shade)), rgb_of(Ramp::ClothPlum.at(Tone::Shade)), 128),
        rgb_of(Ramp::ClothPlum.at(Tone::Shade)),
        rgb_of(Ramp::ClothPlum.at(Tone::Mid)),
        mix(rgb_of(Ramp::ClothPlum.at(Tone::Mid)), rgb_of(Ramp::ClothRose.at(Tone::Mid)), 128),
        rgb_of(Ramp::ClothRose.at(Tone::Mid)),
    ];
    let nb = bands.len() as i32;
    for y in 0..h {
        // Where this row falls among the bands, in sixteenths; the bands crowd toward the horizon.
        let t = (y * y / hz.max(1)).min(hz) * (nb - 1) * 16 / hz.max(1);
        for x in 0..w {
            let (k, f) = (t / 16, t % 16);
            let k = if f > i32::from(jane_art::canvas::bayer(x, y)) { k + 1 } else { k };
            px[(y * w + x) as usize] = pack(bands[k.clamp(0, nb - 1) as usize]);
        }
    }
    let (top, mid) = (bands[0], bands[3]);
    let _ = top;
    // Stars: sparse above, none near the glow.
    for i in 0..(w * h / 1400) as u32 {
        let hs = h32(i, 1, SALT);
        let (x, y) = ((hs % w as u32) as i32, ((hs >> 12) % (hz as u32 * 7 / 10)) as i32);
        let b = 110 + (hs >> 24) as i32 % 120;
        px[(y * w + x) as usize] = pack([b, b, b + 20]);
        if hs.trailing_zeros() >= 6 && x + 1 < w && y + 1 < h {
            // A few brighter ones cross.
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (xx, yy) = (x + dx, y + dy);
                if xx >= 0 && yy >= 0 {
                    px[(yy * w + xx) as usize] = pack([b - 60, b - 60, b - 30]);
                }
            }
        }
    }
    // The moon with a soft halo.
    let (mx, my, mr) = (w * 88 / 100, h * 13 / 100, 12);
    let bone = rgb_of(Ramp::Bone.at(Tone::Light));
    for y in (my - mr * 4).max(0)..(my + mr * 4).min(h) {
        for x in (mx - mr * 4).max(0)..(mx + mr * 4).min(w) {
            let d2 = (x - mx) * (x - mx) + (y - my) * (y - my);
            let i = (y * w + x) as usize;
            if d2 <= mr * mr {
                // The lit crescent: the far side in shade.
                let shade = (x - mx + y - my) > mr / 2;
                px[i] = pack(if shade { mix(bone, mid, 110) } else { bone });
            } else if d2 < mr * mr * 16 {
                let halo = 36 - (d2 * 36 / (mr * mr * 16));
                let d = i32::from(jane_art::canvas::bayer(x, y));
                if halo > d * 2 {
                    let c = [(px[i] >> 16 & 0xff) as i32, (px[i] >> 8 & 0xff) as i32, (px[i] & 0xff) as i32];
                    px[i] = pack(mix(c, bone, 40));
                }
            }
        }
    }
    // The far treeline, then the School's hill against the glow.
    let far = rgb_of(Ramp::ClothNavy.at(Tone::Shade));
    let hill = rgb_of(Ramp::UiSlot.at(Tone::Deep));
    for x in 0..w {
        let top_far = hz - 18 - ridge(x, 23, 16, 1) - ridge(x, 7, 4, 2);
        for y in top_far.max(0)..hz {
            px[(y * w + x) as usize] = pack(far);
        }
        // The hill: a broad rise to the School's crown right of centre.
        let cx = w * 66 / 100;
        let dx = (x - cx).abs();
        let rise = (90 - dx * dx / (w * 2).max(1)).max(0);
        let top_hill = hz - rise * 7 / 10 - ridge(x, 11, 3, 3);
        for y in top_hill.max(0)..hz {
            px[(y * w + x) as usize] = pack(hill);
        }
    }
    // The School: a long block, a wing, a bell tower with a pointed roof, windows lit.
    let ink = rgb_of(Ix::INK);
    let cx = w * 66 / 100;
    let base = hz - 60;
    let mut windows = Vec::new();
    let mut block = |x0: i32, y0: i32, x1: i32, y1: i32| {
        for y in y0.max(0)..y1.min(h) {
            for x in x0.max(0)..x1.min(w) {
                px[(y * w + x) as usize] = pack(ink);
            }
        }
    };
    block(cx - 70, base - 28, cx + 64, base + 4);
    block(cx - 88, base - 16, cx - 70, base + 4);
    block(cx - 12, base - 62, cx + 12, base - 28);
    // Roofs: the main pitch and the tower's spire.
    for k in 0..14 {
        block(cx - 70 + k * 2, base - 28 - k, cx + 64 - k * 2, base - 27 - k);
    }
    for k in 0..30 {
        let half = 12 - k * 12 / 30;
        block(cx - half, base - 62 - k, cx + half, base - 61 - k);
    }
    // The bell's arch, open to the sky.
    let sky_at = |px: &[u32], x: i32, y: i32| px[(y * w + x) as usize];
    for y in base - 56..base - 42 {
        for x in cx - 5..cx + 5 {
            let arch = y > base - 53 || (x - cx) * (x - cx) <= 25 - (base - 53 - y) * 3;
            if arch && x >= 0 && y >= 0 && x < w && y < h {
                let c = sky_at(px, x + 30, (y - 40).max(0));
                px[(y * w + x) as usize] = c;
            }
        }
    }
    // Windows: two rows along the block, most lit amber, a few dark.
    let lit = rgb_of(Ramp::GlassLit.at(Tone::Light));
    let lit_hi = rgb_of(Ramp::GlassLit.at(Tone::High));
    for row in 0..2 {
        for col in 0..11 {
            let x = cx - 62 + col * 12;
            let y = base - 22 + row * 12;
            let hs = h32(col as u32, row as u32, SALT ^ 7);
            if hs % 5 == 0 {
                continue;
            }
            for yy in y..y + 6 {
                for xx in x..x + 4 {
                    if xx >= 0 && yy >= 0 && xx < w && yy < h {
                        px[(yy * w + xx) as usize] = pack(if yy == y { lit_hi } else { lit });
                    }
                }
            }
            windows.push((x, y, 4, 6));
        }
    }
    // The lake: from the horizon down, the sky mirrored and darkened, broken into streaks, with
    // the windows' light drawn down it.
    let water = rgb_of(Ramp::Water.at(Tone::Deep));
    for y in hz..h {
        let depth = (y - hz) * 256 / (h - hz).max(1);
        for x in 0..w {
            let my = (2 * hz - y - 1).max(0);
            let wob = ridge(x + y * 3, 9, 3, 4) - 1;
            let sx = (x + wob).clamp(0, w - 1);
            let s = px[(my * w + sx) as usize];
            let c = [(s >> 16 & 0xff) as i32, (s >> 8 & 0xff) as i32, (s & 0xff) as i32];
            let c = mix(c, water, 120 + depth / 3);
            let streak = h32(x as u32 / 3, y as u32, SALT ^ 9) % 23 == 0;
            px[(y * w + x) as usize] = pack(if streak { mix(c, bone, 30) } else { c });
        }
    }
    for &(x, y, ww, _) in &windows {
        let ry = 2 * hz - y;
        for yy in ry..(ry + 22).min(h) {
            if (yy - ry) % 3 == 2 {
                continue;
            }
            let fade = 150 - (yy - ry) * 6;
            for xx in (x - 1)..=(x + ww) {
                let wob = ridge(xx + yy * 5, 5, 2, 5);
                let xx = xx + wob - 1;
                if xx >= 0 && xx < w && yy < h {
                    let i = (yy * w + xx) as usize;
                    let c = [(px[i] >> 16 & 0xff) as i32, (px[i] >> 8 & 0xff) as i32, (px[i] & 0xff) as i32];
                    px[i] = pack(mix(c, lit, fade.max(0)));
                }
            }
        }
    }
    // The near shore: reeds and a dark bank along the bottom.
    let bank = rgb_of(Ramp::Grass.at(Tone::Deep));
    let reed = rgb_of(Ramp::Reed.at(Tone::Shade));
    for x in 0..w {
        let top = h - 22 - ridge(x, 17, 10, 6);
        for y in top.max(0)..h {
            px[(y * w + x) as usize] = pack(if y == top { mix(bank, reed, 80) } else { bank });
        }
        let hs = h32(x as u32, 3, SALT ^ 0xb);
        if hs % 3 == 0 {
            let tall = 8 + (hs >> 8) as i32 % 18;
            let lean = (hs >> 16) as i32 % 3 - 1;
            for k in 0..tall {
                let (xx, yy) = (x + lean * k / 8, top - k);
                if xx >= 0 && xx < w && yy >= 0 {
                    px[(yy * w + xx) as usize] = pack(if k > tall - 3 { mix(reed, bone, 50) } else { reed });
                }
            }
        }
    }
    windows
}

/// Draws the title: the backdrop, what moves on it, the name, and the menu or the name field.
pub fn draw(ui: &mut Ui, st: &mut TitleState, info: TitleInfo) {
    let (cw, ch) = ui.canvas;
    if st.painted != (cw, ch) {
        let img = ui.image_mut(BACKDROP, cw as u16, ch as u16);
        st.windows = paint(&mut img.argb, cw, ch);
        ui.image_changed(BACKDROP);
        st.painted = (cw, ch);
    }
    ui.image(BACKDROP, Src { x: 0, y: 0, w: cw as u16, h: ch as u16 }, Rect::new(0, 0, cw, ch), 255);
    let t = ui.tick;
    // One window flickers, as the art says: dim, back, dim twice, then steady a while.
    if let Some(&(x, y, w, h)) = st.windows.get(st.windows.len() * 2 / 3) {
        let phase = t % 300;
        let off = matches!(phase, 0..=5 | 11..=13 | 18..=20);
        let a = if off { 210 } else { 0 };
        ui.fill(Rect::new(x, y, w, h), argb(Ramp::UiSlot.at(Tone::Deep), a));
    }
    // Mist drifting over the water: long thin bands, each at its own speed.
    let hz = horizon(ch);
    for k in 0..5 {
        let y = hz + 6 + k * 13 + (((t / 7 + k as u32 * 40) % 64) as i32 / 32 - 1);
        let len = cw * (40 + k * 7) / 100;
        let x = ((t as i32 / (3 + k)) + k * 173).rem_euclid(cw + len) - len;
        ui.fill(Rect::new(x, y, len, 3), argb(Ramp::Bone.at(Tone::Light), 16 + (k as u8) * 3));
        ui.fill(Rect::new(x + len / 6, y + 1, len * 2 / 3, 1), argb(Ramp::Bone.at(Tone::Light), 18));
    }
    // Fireflies in the reeds.
    for k in 0..9u32 {
        let hs = h32(k, 5, SALT);
        let bx = (hs % cw as u32) as i32;
        let by = ch - 30 - ((hs >> 10) % 40) as i32;
        let ph = t.wrapping_add(hs >> 20);
        let dx = tri(ph / 3, 24) - 12;
        let dy = tri(ph / 5 + 7, 12) - 6;
        let pulse = tri(ph, 90);
        if pulse > 30 {
            let a = ((pulse - 30) * 4).min(255) as u8;
            let (x, y) = (bx + dx, by + dy);
            ui.fill(Rect::new(x - 1, y - 1, 3, 3), argb(Ramp::GlassLit.at(Tone::Light), a / 4));
            ui.fill(Rect::new(x, y, 1, 1), argb(Ramp::GlassLit.at(Tone::Glint), a));
        }
    }

    // The name, on the left over the water, gold lit from above.
    let name = "JANE";
    let col = cw * 28 / 100;
    let tw = text_w(Face::Title, name);
    let (tx, ty) = (col - tw / 2, ch * 14 / 100);
    let ink = Ink::title(Ramp::UiGold.at(Tone::Mid)).bold();
    ui.text(tx, ty, name, ink.outline().shadow());
    ui.set_clip(Rect::new(0, 0, cw, ty + 18));
    ui.text(tx, ty, name, ink.ix(Ramp::UiGold.at(Tone::High)));
    ui.set_clip(Rect::new(0, 0, cw, ty + 9));
    ui.text(tx, ty, name, ink.ix(Ramp::UiGold.at(Tone::Glint)));
    ui.set_clip(Rect::CANVAS);
    ui.rule(col - 100, col + 100, ty + 54, style::gold());
    let sub = "The bell rings at nine";
    ui.text(col - text_w(Face::Fine, sub) / 2, ty + 61, sub, Ink::fine(Ramp::Bone.at(Tone::Light)).shadow());

    if st.naming {
        naming(ui, st);
        return;
    }
    // The menu, on a quiet plate low on the left of the School.
    let (labels, enabled, picks): (&[&str], &[bool], &[u8]) = if info.console {
        (
            &["New Game", "Continue", "Load", "Controls", "Quit"],
            &[true, info.has_save, info.has_save, true, true],
            &[0, 1, 2, 5, 6],
        )
    } else {
        (
            &["New Game", "Continue", "Load", "Host", "Join", "Controls", "Quit"],
            &[true, info.has_save, info.has_save, true, true, true, true],
            &[0, 1, 2, 3, 4, 5, 6],
        )
    };
    let w = 220;
    // A short canvas (a console's) closes the rows up under the name.
    let (row_h, top) = if ui.compact() { (26, ty + 74) } else { (28, ch * 38 / 100) };
    let r = Rect::new(col - w / 2, top, w, labels.len() as i32 * row_h + 16);
    ui.fill(r, argb(Ramp::UiSlot.at(Tone::Deep), 110));
    ui.rule(i32::from(r.x), r.right(), i32::from(r.y), style::gold_deep());
    ui.rule(i32::from(r.x), r.right(), r.bottom() - 1, style::gold_deep());
    ui.claim(r);
    match menus::rows(
        ui,
        &mut st.menu,
        "title",
        Rect::new(i32::from(r.x) + 6, i32::from(r.y) + 8, w - 12, 0),
        row_h,
        labels,
        enabled,
    )
    .and_then(|k| picks.get(k).copied())
    {
        Some(0) if info.console => {
            let name = crate::text::clean_name(&st.name);
            ui.intent(AppIntent::NewGame { name });
        }
        Some(0) => {
            st.naming = true;
            ui.close_popover();
        }
        Some(1) => ui.intent(AppIntent::Continue),
        Some(2) => ui.intent(AppIntent::LoadMenu),
        Some(3) => ui.intent(AppIntent::HostMenu),
        Some(4) => ui.intent(AppIntent::JoinMenu),
        Some(5) => ui.intent(AppIntent::Controls),
        Some(6) => ui.intent(AppIntent::Quit),
        _ => {}
    }
}

fn naming(ui: &mut Ui, st: &mut TitleState) {
    let (cw, ch) = ui.canvas;
    let (w, h) = (320, 132);
    let r = Rect::new(cw * 28 / 100 - w / 2, ch * 42 / 100, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    ui.text_in(Rect::new(x, y + 10, w, 20), "Her name", Ink::small(style::gold()).shadow());
    let field = Rect::new(x + 30, y + 36, w - 60, 26);
    let out = ui.text_field(field, &mut st.name, 16, Ink::small(style::text_bright()).shadow());
    let begin = Rect::new(x + 30, y + 82, 120, 26);
    let back = Rect::new(x + w - 150, y + 82, 120, 26);
    let go = ui.button(wid("name-go", 0), begin, "Begin", ButtonKind::Menu, true, true) || out.submitted;
    if go {
        let name = crate::text::clean_name(&st.name);
        st.name.clone_from(&name);
        ui.intent(AppIntent::NewGame { name });
    }
    if ui.button(wid("name-back", 0), back, "Back", ButtonKind::Menu, true, false)
        || (ui.interactive && ui.input.has(UiAction::Cancel))
    {
        st.naming = false;
    }
}

/// A triangle wave 0..=n over period 2n.
fn tri(t: u32, n: u32) -> i32 {
    let n = n.max(1);
    let p = t % (2 * n);
    (if p < n { p } else { 2 * n - p }) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backdrop_fills_the_canvas_and_lights_the_schools_windows() {
        let (w, h) = (768, 432);
        let mut px = vec![0u32; (w * h) as usize];
        let windows = paint(&mut px, w, h);
        assert!(px.iter().all(|&p| p >> 24 == 0xff), "opaque everywhere");
        assert!(windows.len() > 10, "{} lit windows", windows.len());
        // The sky darkens upward: the top row is darker than the horizon's.
        let luma = |p: u32| (p >> 16 & 0xff) + (p >> 8 & 0xff) + (p & 0xff);
        let row = |y: i32| (0..w).map(|x| luma(px[(y * w + x) as usize])).sum::<u32>();
        assert!(row(2) < row(horizon(h) - 70));
        // A wide canvas paints too.
        let mut wide = vec![0u32; (1008 * h) as usize];
        paint(&mut wide, 1008, h);
    }
}
