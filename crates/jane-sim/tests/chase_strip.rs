//! A picture of a chase on the terraces (MAP.md R2 follow-up, "chasers"): the test map from above,
//! her walking a loop slowly enough to be caught, six chasers after her, each with the path it
//! holds drawn out from its feet, and a bar of the searches made so far (a blue block each ten).
//! One strip of four frames, a hundred ticks apart. It draws the build it is run with (`tuning::LEAN_CHASE`), so the
//! strip of the lean chase and the one of the old ways are the same test run either side of that
//! constant: `JANE_STRIP=path/to/strip.png cargo test -p jane-sim --release --test chase_strip --
//! --ignored`.

// The picture goes to a path given in the environment; nothing of it reaches the sim.
#![allow(clippy::disallowed_methods)]

mod field;

use std::sync::Arc;

use field::{Z, body_of, cmd, edit, spawn};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::num::CELL_FX;
use jane_core::{Angle, Blueprint, Cell, Key, Tile, Vec2, ZoneId};
use jane_sim::ai::live_path;
use jane_sim::height::terraces as T;
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, Command, DevOp, InputFrame, Sim, StepInput};

/// Pixels to a cell.
const K: i32 = 6;
const FRAMES: [u32; 4] = [100, 200, 300, 400];

struct Canvas {
    w: i32,
    h: i32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: i32, h: i32) -> Canvas {
        Canvas { w, h, px: vec![255; (w * h * 4) as usize] }
    }

    fn set(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            let i = ((y * self.w + x) * 4) as usize;
            self.px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    fn line(&mut self, (mut x0, mut y0): (i32, i32), (x1, y1): (i32, i32), c: [u8; 3]) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            self.set(x0, y0, c);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity(self.px.len() + self.h as usize);
        for row in self.px.chunks((self.w * 4) as usize) {
            raw.push(0);
            raw.extend_from_slice(row);
        }
        let mut z = vec![0x78, 0x01];
        let mut blocks = raw.chunks(65535).peekable();
        while let Some(b) = blocks.next() {
            z.push(u8::from(blocks.peek().is_none()));
            let n = b.len() as u16;
            z.extend_from_slice(&n.to_le_bytes());
            z.extend_from_slice(&(!n).to_le_bytes());
            z.extend_from_slice(b);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &v in &raw {
            a = (a + u32::from(v)) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        let mut chunk = |kind: &[u8; 4], data: &[u8]| {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let mut c = 0xffff_ffffu32;
            for &v in kind.iter().chain(data) {
                c ^= u32::from(v);
                for _ in 0..8 {
                    c = if c & 1 == 1 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
            }
            out.extend_from_slice(&(c ^ 0xffff_ffff).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        out
    }
}

fn world() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            let mut bp = T::blueprint(z);
            bp.marks
                .insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(30, 10), facing: Some(Facing::East) });
            return Arc::new(bp);
        }
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: None });
        Arc::new(bp)
    });
    let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let me = body_of(&s, 0);
    edit(&mut s, me, |u| u.pos = Vec2::centre(30, 10));
    for (x, y) in [(20, 8), (40, 6), (24, 22), (44, 24), (70, 20), (66, 9)] {
        let f = spawn(&mut s, "quarryman", x, y);
        edit(&mut s, f, |u| {
            u.strength = 2000;
            u.hp = jane_sim::units::max_hp(u);
            u.target = Some(me);
            u.combat = CombatState::Combat;
        });
    }
    s
}

/// Her walk: along the plateau, west then south, off the ledge, east below the face.
fn walk(s: &Sim) -> Angle {
    let me = body_of(s, 0);
    let (x, y) = s.state().zone(Z).and_then(|z| z.unit(me)).expect("her").pos.cell();
    if y < T::FACE.y {
        if x > 12 { if y > 10 { Angle::NORTH } else { Angle::WEST } } else { Angle::SOUTH }
    } else if y < 20 && x < 58 {
        Angle::SOUTH
    } else {
        Angle::EAST
    }
}

fn at_px(p: Vec2) -> (i32, i32) {
    (p.x.0 * K / CELL_FX, p.y.0 * K / CELL_FX)
}

fn frame(s: &Sim, c: &mut Canvas, ox: i32) {
    let rt = s.runtime(Z).expect("the zone");
    let g = &rt.grid;
    for y in 0..T::H as i32 {
        for x in 0..T::W as i32 {
            let t = g.tile_at(x, y);
            let lv = g.level_at(x, y);
            let col = match t {
                Tile::Wall => [24, 24, 30],
                Tile::Cliff => [96, 66, 44],
                Tile::LedgeS | Tile::LedgeN | Tile::LedgeE | Tile::LedgeW => [214, 150, 56],
                Tile::Stair => [236, 208, 128],
                Tile::Ladder => [250, 250, 250],
                Tile::Road => [176, 156, 134],
                Tile::Hedge => [40, 96, 44],
                _ if lv >= 2 => [158, 196, 126],
                _ => [112, 150, 100],
            };
            c.rect(ox + x * K, y * K, K, K, col);
        }
    }
    let me = body_of(s, 0);
    let z = s.state().zone(Z).expect("zone");
    let w = g.w();
    for u in z.units.iter().filter(|u| u.controller == jane_data::Controller::Ai) {
        let (px, py) = at_px(u.pos);
        if let Some(p) = live_path(u) {
            let mut from = (ox + px, py);
            for &cell in p.cells.iter().skip(usize::from(p.at)) {
                let to = (ox + ((cell.0 % w) as i32) * K + K / 2, ((cell.0 / w) as i32) * K + K / 2);
                c.line(from, to, [250, 250, 255]);
                from = to;
            }
        }
    }
    for u in z.units.iter().filter(|u| u.controller == jane_data::Controller::Ai) {
        let (px, py) = at_px(u.pos);
        c.rect(ox + px - 2, py - 2, 5, 5, [214, 36, 36]);
    }
    let (px, py) = at_px(z.unit(me).expect("her").pos);
    c.rect(ox + px - 2, py - 2, 5, 5, [255, 232, 0]);
    // The searches so far: a block each ten, along the top.
    for i in 0..(s.path_stats().searches / 10).min(70) as i32 {
        c.rect(ox + 3 + i * 8, 3, 6, 6, [30, 50, 230]);
    }
}

#[test]
#[ignore = "draws a strip to $JANE_STRIP; a picture, not a rule"]
fn the_chase_on_the_terraces_in_four_frames() {
    let Ok(path) = std::env::var("JANE_STRIP") else { return };
    let mut s = world();
    let mut c = Canvas::new(T::W as i32 * K * FRAMES.len() as i32, T::H as i32 * K);
    let mut shot = 0;
    for f in 0..=*FRAMES.last().expect("frames") {
        let dir = walk(&s);
        let input = InputFrame { aim: Some(dir), mv_mag: 40, ..InputFrame::walk(dir) };
        s.step(&StepInput { frames: [input, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands: &[] });
        s.drain_events();
        if FRAMES.get(shot) == Some(&(f + 1)) {
            frame(&s, &mut c, shot as i32 * T::W as i32 * K);
            shot += 1;
        }
    }
    assert_eq!(shot, FRAMES.len());
    std::fs::write(&path, c.png()).expect("the strip is written");
    println!(
        "{} searches, {} kept, {} straight, to {path}",
        s.path_stats().searches,
        s.path_stats().kept,
        s.path_stats().straight
    );
}
