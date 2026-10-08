//! `jane bake` (PORT.md §13.4): runs jane-art's generators once and writes the **canonical
//! pack**, target-neutral: every sprite frame as master-palette indices in all four layers where
//! it has them, its anchor and rect, the palette, and a manifest keyed by `SpriteId`. Per-target
//! packers (`--target psp`, `bake_psp.rs`) read the pack and write what a console loads.
//!
//! Reads the generators; changes nothing they draw. The same build writes the same bytes, and
//! the pack's FNV-1a 64 hash is printed (and tested).
//!
//! What is in it, by category (sprite id, frame and the variant/seat byte are the key):
//!
//! | Category | From | Key |
//! | --- | --- | --- |
//! | units | `looks::all()`, people and creatures, every variant, seat and frame | `SpriteId`, `FrameId`, variant, seat |
//! | props, buildings | `looks::all()`, the kit and the house painter | `SpriteId`, `FrameId` |
//! | icons | `looks::all()`, 32 (variant 0) and 16 (variant 1) | `SpriteId`, variant |
//! | flora | the chunk painter's bank (`Painter::bank().all()`) | bank index |
//! | terrain | every tile style in its sixteen neighbour contexts, the centre cell and what it stands above it | style index, mask |
//! | font | every glyph of every face, regular and bold, as a mask in `Ix::INK` | code point, face * 2 + bold |
//!
//! Terrain is painted as resolved colour (`0xFFRRGGBB`) by the chunk painter, so a colour of a
//! terrain px that is not a master-palette entry is appended after the master palette: every
//! albedo in the pack is still one `u16` index into the pack's palette.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use jane_art::Canvas;
use jane_art::canvas::Normal;
use jane_art::font::{self, Face};
use jane_art::looks::{self, Family};
use jane_art::palette::{self, Ix};
use jane_art::terrain::{self, CELL, Painter, TileMap};
use jane_core::grid::{Grid, Rect};
use jane_core::tile::F_INDOOR;
use jane_core::{Material, Tile};
use jane_data::{TileHeight, TilePattern};

pub const USAGE: &str = "  bake [--out DIR] [--target psp] [--force]
                                      the art bake (PORT.md §13.4): the canonical pack (every sprite
                                      in all four layers, the palette, a manifest), and with --target
                                      psp the PSP pack (8-bit paged albedo, swizzled); default out
                                      target/bake; skipped when the build that wrote DIR is this one";

/// What a sprite is, for grouping and the size report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cat {
    Terrain = 0,
    Flora = 1,
    Units = 2,
    Props = 3,
    Buildings = 4,
    Icons = 5,
    Font = 6,
    /// The presenter's own sprites no look holds (`jane_present::atlas::cat::SCENE`): stand-ins,
    /// the sky, the cues, critters, glows. Only the PSP pack has them, keyed by `RefId`.
    Scene = 7,
}

impl Cat {
    pub const ALL: [Cat; 8] =
        [Cat::Terrain, Cat::Flora, Cat::Units, Cat::Props, Cat::Buildings, Cat::Icons, Cat::Font, Cat::Scene];

    /// The category numbered `n` (`jane_present::atlas::cat`).
    pub fn of(n: u8) -> Option<Cat> {
        Cat::ALL.get(usize::from(n)).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Cat::Terrain => "terrain",
            Cat::Flora => "flora",
            Cat::Units => "units",
            Cat::Props => "props",
            Cat::Buildings => "buildings",
            Cat::Icons => "icons",
            Cat::Font => "font",
            Cat::Scene => "scene",
        }
    }
}

/// One frame of one sprite, all four layers at `w x h` (the flat ones carry albedo alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub cat: Cat,
    /// `SpriteId` for looks; bank index, style index or code point otherwise.
    pub sprite: u16,
    /// `FrameId as u8` for looks; the mask for terrain; face * 2 + bold for glyphs.
    pub frame: u8,
    /// Variant in the high nibble, seat in the low.
    pub vs: u8,
    /// For people: `jane@2#1/down`.
    pub name: String,
    pub w: u16,
    pub h: u16,
    pub ax: i16,
    pub ay: i16,
    pub albedo: Vec<u16>,
    /// Empty for a flat sprite (the font): normal up, no glow, height 0.
    pub normal: Vec<Normal>,
    pub emissive: Vec<u16>,
    pub height: Vec<u8>,
}

impl Item {
    fn of_canvas(cat: Cat, key: (u16, u8, u8), name: String, c: &Canvas, anchor: (i32, i32)) -> Item {
        let (w, h) = (c.w(), c.h());
        let n = (w * h) as usize;
        let (mut albedo, mut normal, mut emissive, mut height) =
            (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        for y in 0..h {
            for x in 0..w {
                albedo.push(c.get(x, y).0);
                normal.push(c.normal_at(x, y));
                emissive.push(c.emissive_at(x, y).0);
                height.push(c.height_at(x, y));
            }
        }
        Item {
            cat,
            sprite: key.0,
            frame: key.1,
            vs: key.2,
            name,
            w: w as u16,
            h: h as u16,
            ax: anchor.0 as i16,
            ay: anchor.1 as i16,
            albedo,
            normal,
            emissive,
            height,
        }
    }

    pub fn flat(&self) -> bool {
        self.normal.is_empty()
    }

    /// Sort key: category, sprite, variant and seat, frame.
    pub fn key(&self) -> (Cat, u16, u8, u8) {
        (self.cat, self.sprite, self.vs, self.frame)
    }
}

/// The canonical pack: the palette (the master palette, then terrain's own colours) and every
/// item in key order.
#[derive(Clone, Debug)]
pub struct Pack {
    pub palette: Vec<[u8; 3]>,
    /// How many of `palette`'s entries are the master palette's.
    pub master: usize,
    pub items: Vec<Item>,
}

/// FNV-1a, 64 bits.
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Runs every generator the pack holds.
pub fn build() -> Result<Pack, String> {
    let mut palette: Vec<[u8; 3]> = palette::PALETTE.to_vec();
    let master = palette.len();
    let mut items = Vec::new();
    for r in looks::all()? {
        let cat = match looks::find(r.name).map(|(_, l)| Family::of(l)) {
            Some(Family::Person | Family::Creature) => Cat::Units,
            Some(Family::Prop) => Cat::Props,
            Some(Family::Building) => Cat::Buildings,
            Some(Family::Icon) => Cat::Icons,
            None => return Err(format!("{}: rendered but not in the looks table", r.name)),
        };
        let vs = (r.variant << 4) | r.seat;
        for (f, c) in &r.set.frames {
            let name = format!("{}/{f:?}", r.key());
            items.push(Item::of_canvas(cat, (r.sprite.0, *f as u8, vs), name, c, (r.set.ax, r.set.ay)));
        }
    }
    let mut painter = Painter::new();
    for (i, (name, s)) in painter.bank().all().into_iter().enumerate() {
        items.push(Item::of_canvas(Cat::Flora, (i as u16, 0, 0), name, &s.canvas, (s.ax, s.ay)));
    }
    terrain_items(&mut painter, &mut palette, &mut items);
    font_items(&mut items);
    items.sort_by_key(Item::key);
    Ok(Pack { palette, master, items })
}

/// Contexts per tile: the sixteen ways its four neighbours can be the same tile.
const MASKS: i32 = 16;
/// Cells between one context's centre and the next.
const PITCH: i32 = 5;
/// The scene's rows: tall things need room above.
const SCENE_H: i32 = 9;
/// The context's centre row.
const MID: i32 = 6;

/// The tile sheet's scene (`jane sheet terrain`), or with `t` `None` its bare background.
fn scene(t: Option<Tile>, mat: Option<Material>, indoor: bool) -> TileMap {
    let bg = if indoor { Tile::Floor } else { Tile::Grass };
    let mut tiles = Grid::new((MASKS * PITCH) as u32, SCENE_H as u32, bg);
    let mut cells = Vec::new();
    for m in 0..MASKS {
        let x = m * PITCH + 2;
        cells.push((x, MID));
        for (bit, (dx, dy)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            if m >> bit & 1 == 1 {
                cells.push((x + dx, MID + dy));
            }
        }
    }
    if let Some(t) = t {
        for &(x, y) in &cells {
            tiles.set(x, y, t);
        }
    }
    let mut map = TileMap::new(tiles, !indoor);
    if let (Some(m), Some(_)) = (mat, t) {
        for &(x, y) in &cells {
            map.set_material(Rect::new(x, y, 1, 1), m);
        }
    }
    map
}

/// Every tile style in each of its sixteen contexts: the centre cell, and above it whatever the
/// tile stands over its bare background (a wall's face, a tree's crown), the rest clear.
fn terrain_items(p: &mut Painter, palette: &mut Vec<[u8; 3]>, items: &mut Vec<Item>) {
    let mut index: BTreeMap<u32, u16> = BTreeMap::new();
    for (i, c) in palette.iter().enumerate().skip(2) {
        index.entry(u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])).or_insert(i as u16);
    }
    let win = Rect::new(0, 0, MASKS * PITCH * CELL, SCENE_H * CELL);
    let bare = [false, true].map(|indoor| terrain::sheet::compose(p, &scene(None, None, indoor), 7, win));
    for (si, (name, t, mat)) in p.styles().all().into_iter().enumerate() {
        let st = p.styles().cell(t, mat);
        let tall = st.row.height != TileHeight::Flat || st.row.pattern == TilePattern::Cliff;
        let indoor = t.flags() & F_INDOOR != 0;
        let c = terrain::sheet::compose(p, &scene(Some(t), mat, indoor), 7, win);
        let bg = &bare[usize::from(indoor)];
        for m in 0..MASKS {
            let x0 = (m * PITCH + 2) * CELL;
            let (y0, y1) = (if tall { 0 } else { MID * CELL }, (MID + 1) * CELL);
            // The first row above the centre cell that differs from the bare scene.
            let top = (y0..MID * CELL)
                .find(|&y| {
                    (x0..x0 + CELL).any(|x| c.albedo[(y * c.w + x) as usize] != bg.albedo[(y * c.w + x) as usize])
                })
                .unwrap_or(MID * CELL);
            let (w, h) = (CELL, y1 - top);
            let n = (w * h) as usize;
            let mut it = Item {
                cat: Cat::Terrain,
                sprite: si as u16,
                frame: m as u8,
                vs: 0,
                name: format!("{name}/{m}"),
                w: w as u16,
                h: h as u16,
                ax: 0,
                ay: (h - CELL) as i16,
                albedo: Vec::with_capacity(n),
                normal: Vec::with_capacity(n),
                emissive: Vec::with_capacity(n),
                height: Vec::with_capacity(n),
            };
            for y in top..y1 {
                for x in x0..x0 + CELL {
                    let i = (y * c.w + x) as usize;
                    let clear = y < MID * CELL && c.albedo[i] == bg.albedo[i];
                    let a = if clear {
                        0
                    } else {
                        let rgb = c.albedo[i] & 0x00ff_ffff;
                        *index.entry(rgb).or_insert_with(|| {
                            palette.push([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
                            (palette.len() - 1) as u16
                        })
                    };
                    it.albedo.push(a);
                    it.normal.push(if clear { jane_art::canvas::FLAT } else { c.normal[i] });
                    it.emissive.push(if clear { 0 } else { c.emissive[i].0 });
                    it.height.push(if clear { 0 } else { c.height[i] });
                }
            }
            items.push(it);
        }
    }
}

/// Every glyph, every face, regular and bold: a flat mask in `Ix::INK`.
fn font_items(items: &mut Vec<Item>) {
    for (fi, &face) in Face::ALL.iter().enumerate() {
        for bold in [false, true] {
            for ch in font::chars() {
                let g = font::rasterise(font::strokes(ch).unwrap_or(&[]), face, bold);
                let albedo = (0..g.h)
                    .flat_map(|y| (0..g.w).map(move |x| (x, y)))
                    .map(|(x, y)| if g.at(x, y) { Ix::INK.0 } else { 0 })
                    .collect();
                items.push(Item {
                    cat: Cat::Font,
                    sprite: ch as u32 as u16,
                    frame: (fi * 2 + usize::from(bold)) as u8,
                    vs: 0,
                    name: format!("{face:?}{}/U+{:04X}", if bold { "+bold" } else { "" }, ch as u32),
                    w: g.w as u16,
                    h: g.h as u16,
                    ax: 0,
                    ay: 0,
                    albedo,
                    normal: Vec::new(),
                    emissive: Vec::new(),
                    height: Vec::new(),
                });
            }
        }
    }
}

impl Pack {
    /// The pack as bytes (all little-endian):
    ///
    /// ```text
    /// "JBK1"  u32 palette_len  u32 master_len  u32 item_count
    /// palette_len x [r, g, b]
    /// item_count x { u8 cat, u16 sprite, u8 frame, u8 vs, u8 flat, u16 w, u16 h, i16 ax, i16 ay }
    /// item_count x { albedo u16 x w*h, and unless flat: normal [u8; 2] x w*h, emissive u16 x w*h, height u8 x w*h }
    /// ```
    pub fn bytes(&self) -> Vec<u8> {
        let px: usize = self.items.iter().map(|i| i.albedo.len() * if i.flat() { 2 } else { 7 }).sum();
        let mut out = Vec::with_capacity(16 + self.palette.len() * 3 + self.items.len() * 15 + px);
        out.extend_from_slice(b"JBK1");
        for n in [self.palette.len(), self.master, self.items.len()] {
            out.extend_from_slice(&(n as u32).to_le_bytes());
        }
        for c in &self.palette {
            out.extend_from_slice(c);
        }
        for i in &self.items {
            out.push(i.cat as u8);
            out.extend_from_slice(&i.sprite.to_le_bytes());
            out.extend_from_slice(&[i.frame, i.vs, u8::from(i.flat())]);
            for v in [i.w, i.h] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for v in [i.ax, i.ay] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for i in &self.items {
            i.albedo.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
            if !i.flat() {
                i.normal.iter().for_each(|n| out.extend_from_slice(n));
                i.emissive.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
                out.extend_from_slice(&i.height);
            }
        }
        out
    }

    /// The manifest, one line per item, for people and diffs.
    pub fn manifest(&self, hash: u64) -> String {
        let mut s = format!(
            "# jane bake: canonical pack {hash:016x}, {} items, palette {} ({} master)\n\
             # cat\tsprite\tframe\tvariant\tseat\tw\th\tax\tay\tlayers\tname\n",
            self.items.len(),
            self.palette.len(),
            self.master
        );
        for i in &self.items {
            let _ = writeln!(
                s,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                i.cat.name(),
                i.sprite,
                i.frame,
                i.vs >> 4,
                i.vs & 15,
                i.w,
                i.h,
                i.ax,
                i.ay,
                if i.flat() { 1 } else { 4 },
                i.name
            );
        }
        s
    }
}

fn arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// The build's own hash: the running executable, which holds the generators and the looks.
fn build_stamp() -> Option<u64> {
    std::env::current_exe().ok().and_then(|p| std::fs::read(p).ok()).map(|b| fnv64(&b))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let out = arg(args, "--out")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/bake"), PathBuf::from);
    let target = arg(args, "--target");
    if let Some(t) = target {
        if t != "psp" {
            return Err(format!("bake: unknown target \"{t}\" (psp)"));
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let stamp = build_stamp().map(|h| format!("{h:016x} {}\n", target.unwrap_or("canonical")));
    let stamp_path = out.join("stamp.txt");
    if !args.iter().any(|a| a == "--force")
        && stamp.is_some()
        && std::fs::read_to_string(&stamp_path).ok() == stamp
        && let Ok(summary) = std::fs::read_to_string(out.join("summary.txt"))
    {
        print!("{summary}");
        println!("(cached: the build that wrote {} is this one; --force to bake again)", out.display());
        return Ok(());
    }
    let t0 = std::time::Instant::now();
    let pack = build()?;
    let bytes = pack.bytes();
    let hash = fnv64(&bytes);
    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "canonical pack {hash:016x}: {} items, {} bytes, palette {} ({} master, {} terrain)",
        pack.items.len(),
        bytes.len(),
        pack.palette.len(),
        pack.master,
        pack.palette.len() - pack.master
    );
    write(&out.join("canonical.jbk"), &bytes)?;
    write(&out.join("manifest.tsv"), pack.manifest(hash).as_bytes())?;
    // The presenter's own atlas, as it builds it (T0: the albedo and the sparse glow), so the
    // packs and the presenter cannot drift (PORT.md §13.4, `JAT1`).
    let present = jane_present::Present::new(jane_present::Tier::T0);
    let atlas = present.sprites();
    let jat = atlas.to_pack();
    let _ = writeln!(
        summary,
        "presenter atlas {:016x}: {} refs ({} keyed to a look), {} pages, {} bytes",
        fnv64(&jat),
        atlas.refs.len(),
        atlas.keys.iter().filter(|k| k.is_some()).count(),
        atlas.pages.pages.len(),
        jat.len()
    );
    write(&out.join("presenter.jat"), &jat)?;
    // Its tables, so a console's presenter boots without the generators (`JPT1`, PORT.md
    // §13.12): the sprite table without px and each module's table.
    let jpt = present.tables();
    let _ = writeln!(summary, "presenter tables {:016x}: {} bytes", fnv64(&jpt), jpt.len());
    write(&out.join("present.jpt"), &jpt)?;
    if target == Some("psp") {
        let psp = crate::bake_psp::pack(&pack, atlas)?;
        let file = psp.bytes(hash);
        let _ = writeln!(summary, "psp pack {:016x}: {} bytes", fnv64(&file), file.len());
        summary.push_str(&psp.report());
        write(&out.join("jane-psp.jpk"), &file)?;
        // The sound (PORT.md §13.4): the songs' patterns over one bank of the synth's samples.
        let (jau, st) = jane_audio::bake::module(jane_audio::library());
        let resident = jau[..jane_audio::tracker::Bank::ram_len(&jau).unwrap_or(jau.len())].to_vec();
        let held = jane_audio::tracker::Mixer::new(jane_audio::tracker::Bank::parse(resident).map_err(String::from)?, 22_050, 1).ram();
        let _ = writeln!(
            summary,
            "psp sound {:016x}: {} bytes (patterns {}, instruments {} in {} zones, effects {}, beds {}; {} on the Memory Stick only); {held} held in RAM; worst ADPCM {:.0} dB ({})",
            fnv64(&jau),
            jau.len(),
            st.header,
            st.music,
            st.zones,
            st.sfx,
            st.beds,
            st.far,
            st.worst_snr.0,
            st.worst_snr.1
        );
        write(&out.join("jane-psp.jau"), &jau)?;
    }
    let _ = writeln!(summary, "baked in {} ms", t0.elapsed().as_millis());
    write(&out.join("summary.txt"), summary.as_bytes())?;
    if let Some(s) = stamp {
        write(&stamp_path, s.as_bytes())?;
    }
    print!("{summary}");
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bake is deterministic: two builds, the same bytes (PORT.md §13.4).
    #[test]
    fn same_build_same_bytes() {
        let pack = build().unwrap();
        let a = pack.bytes();
        let b = build().unwrap().bytes();
        assert_eq!(fnv64(&a), fnv64(&b));
        assert!(a == b);
        // And the PSP pack over it and the presenter's atlas: every keyed ref draws the
        // generators' px (else `pack` fails), and every ref finds its record.
        let present = jane_present::Present::new(jane_present::Tier::T0);
        let p = crate::bake_psp::pack(&pack, present.sprites()).unwrap();
        assert_eq!(p.bytes(fnv64(&a)), crate::bake_psp::pack(&pack, present.sprites()).unwrap().bytes(fnv64(&a)));
        assert_eq!(p.refs.len(), present.sprites().refs.len());
        let drawn = p.refs.iter().flatten().count();
        assert!(drawn * 100 >= p.refs.len() * 99, "{drawn} of {} refs on the PSP", p.refs.len());
        // A unit's pages are its own: each unit group is one sprite, and no two groups share one.
        let groups = p.groups();
        assert!(groups.iter().filter(|g| g.0 == Cat::Units).count() > 50);
        for (k, pg) in p.pages.iter().enumerate() {
            let g = groups.iter().find(|g| (g.2..g.2 + g.3).contains(&(k as u16))).unwrap();
            assert_eq!((g.0, g.1), (pg.cat, pg.group));
        }
        // Every frame's record is in its page, and every page's colours are exact (no page
        // quantised) and within the GE's 512.
        for r in p.recs.iter().filter(|r| r.page != u16::MAX) {
            let pg = &p.pages[usize::from(r.page)];
            assert!(r.u + r.w <= pg.w && r.v + r.h <= pg.h, "{r:?}");
        }
        assert!(p.pages.iter().all(|pg| !pg.quantised && pg.w <= 512 && pg.h <= 512 && pg.w.is_power_of_two()));
    }

    #[test]
    fn font_is_flat_and_inked() {
        let mut items = Vec::new();
        font_items(&mut items);
        assert!(items.iter().all(|i| i.flat() && i.albedo.iter().all(|&a| a == 0 || a == Ix::INK.0)));
        assert!(items.iter().any(|i| i.albedo.contains(&Ix::INK.0)));
    }
}
