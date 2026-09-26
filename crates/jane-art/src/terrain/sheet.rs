//! The terrain's sheets (ART.md §5, PRESENTATION.md §6): `jane sheet terrain`, every tile in
//! every autotile context, dry and wet; and `jane sheet county --full`, the real chunk painter
//! over a window of a built county, all four layers side by side, and the same window lit by
//! day and by night through the integer light pass.
//!
//! Sheets are for looking, never asserted.

use jane_core::angle::Angle;
use jane_core::grid::{Grid, Rect};
use jane_core::tile::F_INDOOR;
use jane_core::{Blueprint, Material, Tile};
use jane_data::{TileGroup, TileHeight, TilePattern};

use super::{CELL, CHUNK_CELLS, CHUNK_PX, Chunk, Painter, TileMap, TileSource, paint_chunk};
use crate::canvas::{Normal, decode};
use crate::font::{Face, Font};
use crate::light::{self, Point, Scene, Sun};
use crate::palette::{self, Ix, Ramp, Tone};
use crate::sheet::{Image, label};

/// A window of painted terrain composed into flat layers: the ground of every chunk under it and
/// the strips over it in y order.
#[derive(Clone, Debug)]
pub struct Composite {
    /// Size in px.
    pub w: i32,
    /// Size in px.
    pub h: i32,
    /// `0xFFRRGGBB`.
    pub albedo: Vec<u32>,
    /// Normals.
    pub normal: Vec<Normal>,
    /// Emissive indices.
    pub emissive: Vec<Ix>,
    /// Px above the ground.
    pub height: Vec<u8>,
    /// Per px, the wetness response of the cell under it (0, 1, 2).
    pub wet: Vec<u8>,
}

impl Composite {
    /// The layers as a scene for the light pass.
    pub fn scene(&self) -> Scene<'_> {
        Scene {
            w: self.w,
            h: self.h,
            albedo: &self.albedo,
            normal: &self.normal,
            emissive: &self.emissive,
            height: &self.height,
        }
    }
}

/// Paint every chunk of `src` under the world px rect `win` and compose them. Chunks whose strips
/// reach up into the window from below it are painted too.
pub fn compose(p: &mut Painter, src: &impl TileSource, seed: u32, win: Rect) -> Composite {
    let (zw, zh) = src.size();
    let n = (win.w * win.h) as usize;
    let mut c = Composite {
        w: win.w,
        h: win.h,
        albedo: vec![0xff00_0000; n],
        normal: vec![crate::canvas::FLAT; n],
        emissive: vec![Ix::CLEAR; n],
        height: vec![1; n],
        wet: vec![0; n],
    };
    let cx0 = win.x.div_euclid(CHUNK_PX).max(0);
    let cy0 = win.y.div_euclid(CHUNK_PX).max(0);
    let cx1 = (win.right() - 1).div_euclid(CHUNK_PX).min((zw - 1).div_euclid(CHUNK_CELLS));
    let cy1 = (win.bottom() - 1 + super::STRIP_H).div_euclid(CHUNK_PX).min((zh - 1).div_euclid(CHUNK_CELLS));
    let mut chunk = Chunk::new();
    // Strips, gathered from every chunk and drawn after all the ground: (base y, x, chunk, index).
    let mut strips: Vec<(i32, i32, super::Strip)> = Vec::new();
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            paint_chunk(p, src, seed, cx, cy, &mut chunk);
            let (ox, oy) = (cx * CHUNK_PX - win.x, cy * CHUNK_PX - win.y);
            let l = &chunk.layers;
            for y in 0..CHUNK_PX {
                let ty = oy + y;
                if ty < 0 || ty >= win.h {
                    continue;
                }
                for x in 0..CHUNK_PX {
                    let tx = ox + x;
                    if tx < 0 || tx >= win.w {
                        continue;
                    }
                    let (s, d) = ((y * CHUNK_PX + x) as usize, (ty * win.w + tx) as usize);
                    c.albedo[d] = l.albedo[s];
                    c.normal[d] = l.normal[s];
                    c.emissive[d] = l.emissive[s];
                    c.height[d] = l.height[s];
                    c.wet[d] = l.wet[((y / CELL) * CHUNK_CELLS + x / CELL) as usize];
                }
            }
            for s in chunk.strips() {
                strips.push((cy * CHUNK_PX + s.base_y(), cx * CHUNK_PX + i32::from(s.x), s.clone()));
            }
        }
    }
    strips.sort_by_key(|(y, x, _)| (*y, *x));
    for (base, x, s) in &strips {
        // `x` is the strip's world px; `base - base_y` its chunk's top.
        let (ox, oy) = (x - win.x, base - s.base_y() + i32::from(s.y) - win.y);
        for y in 0..i32::from(s.h) {
            let ty = oy + y;
            if ty < 0 || ty >= win.h {
                continue;
            }
            for x in 0..i32::from(s.w) {
                let tx = ox + x;
                if tx < 0 || tx >= win.w {
                    continue;
                }
                let i = (y * i32::from(s.w) + x) as usize;
                if s.albedo[i] == 0 {
                    continue;
                }
                let d = (ty * win.w + tx) as usize;
                c.albedo[d] = s.albedo[i];
                c.normal[d] = s.normal[i];
                c.emissive[d] = Ix::CLEAR;
                c.height[d] = s.height[i].max(c.height[d]);
            }
        }
    }
    c
}

/// Albedo `a` after rain: `kind` 1 darkens; 2 darkens more and takes a cool sheen of the sky.
/// `wetness` 0..=255 is how wet it is (PRESENTATION.md §1.8).
pub fn wet_colour(a: u32, kind: u8, wetness: u8) -> u32 {
    if kind == 0 || wetness == 0 {
        return a;
    }
    let w = u32::from(wetness);
    let dark = if kind == 1 { 64 * w / 255 } else { 84 * w / 255 };
    let sky = palette::rgb(Ramp::Sky.at(Tone::Light));
    let sheen = if kind == 2 { 34 * w / 255 } else { 0 };
    let ch = |shift: u32, k: usize| {
        let v = (a >> shift) & 255;
        let v = v * (256 - dark) / 256;
        let v = v + (u32::from(sky[k]) * sheen) / 256;
        v.min(255) << shift
    };
    (a & 0xff00_0000) | ch(16, 0) | ch(8, 1) | ch(0, 2)
}

fn rgba(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

/// Background and ink for sheets.
const BG: [u8; 4] = [22, 20, 30, 255];
const TEXT: [u8; 3] = [216, 208, 192];
const DIM: [u8; 3] = [140, 134, 150];

/// Contexts per tile: the sixteen ways its four neighbours can be the same tile.
const MASKS: usize = 16;
/// Cells between one context's centre and the next.
const PITCH: i32 = 5;
/// The scene's rows: tall things need room above.
const SCENE_H: i32 = 9;
/// The context's centre row.
const MID: i32 = 6;

/// The name of a neighbour mask: which of N, E, S, W are the same tile.
fn mask_name(m: usize) -> String {
    let s: String = ["N", "E", "S", "W"].iter().enumerate().filter(|(i, _)| m >> i & 1 == 1).map(|(_, n)| *n).collect();
    if s.is_empty() { "alone".into() } else { s }
}

/// A made-up scene for one tile: its sixteen contexts in a row on its natural ground.
fn scene(t: Tile, mat: Option<Material>) -> TileMap {
    let indoor = t.flags() & F_INDOOR != 0;
    let bg = if indoor { Tile::Floor } else { Tile::Grass };
    let mut tiles = Grid::new(MASKS as u32 * PITCH as u32, SCENE_H as u32, bg);
    let mut cells = Vec::new();
    for m in 0..MASKS as i32 {
        let x = m * PITCH + 2;
        cells.push((x, MID));
        for (bit, (dx, dy)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            if m >> bit & 1 == 1 {
                cells.push((x + dx, MID + dy));
            }
        }
    }
    for &(x, y) in &cells {
        tiles.set(x, y, t);
    }
    let mut map = TileMap::new(tiles, !indoor);
    if let Some(m) = mat {
        for &(x, y) in &cells {
            map.set_material(Rect::new(x, y, 1, 1), m);
        }
    }
    map
}

/// Whether a style needs the scene's full height (it stands tall).
fn tall(p: &Painter, t: Tile, mat: Option<Material>) -> bool {
    let st = p.styles().cell(t, mat);
    st.row.height != TileHeight::Flat || st.row.pattern == TilePattern::Cliff
}

/// `jane sheet terrain`: one image per tile group, each tile a row of its sixteen neighbour
/// contexts at 1x, dry above and wet below (full wetness), on grass out of doors and on a
/// stone floor indoors.
pub fn terrain(p: &mut Painter, font: &Font, only: Option<&str>) -> Vec<(String, Image)> {
    let groups = [
        (TileGroup::Ground, "ground"),
        (TileGroup::Water, "water"),
        (TileGroup::Flora, "flora"),
        (TileGroup::Wall, "walls"),
        (TileGroup::Roof, "roofs"),
        (TileGroup::Made, "made"),
    ];
    let all = p.styles().all();
    let mut out = Vec::new();
    for (group, gname) in groups {
        let rows: Vec<&(String, Tile, Option<Material>)> = all
            .iter()
            .filter(|(n, t, m)| p.styles().cell(*t, *m).row.group == group && only.is_none_or(|o| o == n.as_str()))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let label_w = 150;
        let scene_w = MASKS as i32 * PITCH * CELL;
        let heights: Vec<i32> = rows.iter().map(|(_, t, m)| if tall(p, *t, *m) { SCENE_H - 1 } else { 4 }).collect();
        let total: i32 = heights.iter().map(|h| 2 * h * CELL + 12).sum::<i32>() + 40;
        let mut img = Image::new((label_w + scene_w + 8) as u32, total as u32, BG);
        label(
            &mut img,
            font,
            8,
            8,
            &format!("terrain: {gname}, sixteen neighbour contexts each, dry over wet"),
            Face::Small,
            TEXT,
        );
        for m in 0..MASKS {
            let x = label_w + m as i32 * PITCH * CELL + 2 * CELL - 4;
            label(&mut img, font, x as u32, 28, &mask_name(m), Face::Fine, DIM);
        }
        let mut y = 40;
        for ((name, t, mat), rows_h) in rows.iter().zip(&heights) {
            let map = scene(*t, *mat);
            let top = SCENE_H - 1 - rows_h;
            let win = Rect::new(0, top * CELL, scene_w, rows_h * CELL);
            let c = compose(p, &map, 7, win);
            label(&mut img, font, 8, (y + 4) as u32, name, Face::Small, TEXT);
            label(&mut img, font, 8, (y + rows_h * CELL + 4) as u32, "wet", Face::Fine, DIM);
            for py in 0..c.h {
                for px in 0..c.w {
                    let i = (py * c.w + px) as usize;
                    let a = c.albedo[i];
                    img.set((label_w + px) as u32, (y + py) as u32, rgba(a));
                    let wet = wet_colour(a, c.wet[i], 255);
                    img.set((label_w + px) as u32, (y + c.h + py) as u32, rgba(wet));
                }
            }
            y += 2 * rows_h * CELL + 12;
        }
        out.push((format!("terrain-{gname}"), img));
    }
    out
}

/// `jane sheet flora`: every sprite of the bank at 2x on meadow grass, its foot marked, and
/// under each its normal and height layers at 1x.
pub fn flora(bank: &crate::flora::Bank, font: &Font) -> Image {
    const S: u32 = 2;
    let all = bank.all();
    let cols = 8u32;
    let (cw, ch) = (64 * S + 16, 80 * S + 60 + 88);
    let rows = (all.len() as u32).div_ceil(cols);
    let mut img = Image::new(cols * cw + 16, rows * ch + 36, BG);
    label(
        &mut img,
        font,
        8,
        8,
        "flora: the chunk painter's bank at 2x, with normal and height at 1x",
        Face::Small,
        TEXT,
    );
    let grass = palette::rgb(Ramp::Turf.at(Tone::Base));
    for (k, (name, s)) in all.iter().enumerate() {
        let (gx, gy) = (8 + (k as u32 % cols) * cw, 32 + (k as u32 / cols) * ch);
        let c = &s.canvas;
        let (w, h) = (c.w() as u32, c.h() as u32);
        img.fill(gx, gy, 64 * S, 80 * S, grass);
        let (ox, oy) = (gx + (64 * S - w * S) / 2, gy + 80 * S - h * S);
        crate::sheet::put_albedo(&mut img, c, ox, oy, S);
        let maxh = c.heights().iter().copied().max().unwrap_or(1).max(1);
        for y in 0..h {
            for x in 0..w {
                if !c.get(x as i32, y as i32).is_opaque() {
                    continue;
                }
                let [nx, ny, nz] = decode(c.normal_at(x as i32, y as i32));
                img.set(gx + x, gy + 80 * S + 18 + y, [(128 + nx) as u8, (128 + ny) as u8, (128 + nz) as u8, 255]);
                let t = u32::from(c.height_at(x as i32, y as i32)) * 255 / u32::from(maxh);
                img.set(gx + w + 4 + x, gy + 80 * S + 18 + y, [t as u8, t as u8, (t / 2 + 40) as u8, 255]);
            }
        }
        label(&mut img, font, gx, gy + 80 * S + 4, name, Face::Fine, DIM);
    }
    img
}

/// A made-up stretch of county holding every outdoor tile in a natural context: lanes and a
/// road, a pond with a sandy shore, stepping stones and a boardwalk, marsh, dried mud, a garden
/// and crops, a cobbled square, cottages under tile, slate and thatch, a brick house, a wood of
/// broadleaves and pines with dead trees, shrubs, fences, stone walls, hedges, a cliff, rubble,
/// flower beds, long grass, a grown-over path, ice, and a railway round a bend. 64 x 48 cells:
/// the goldens' and the tests' ground, and `jane sheet terrain --sample`.
pub fn sample_county() -> TileMap {
    use Tile as T;
    let (w, h) = (64, 48);
    let mut g = Grid::new(w, h, T::Grass);
    let mut mats: Vec<(Rect, Material)> = Vec::new();
    let fill = |g: &mut Grid<Tile>, x: i32, y: i32, rw: i32, rh: i32, t: Tile| g.fill_rect(Rect::new(x, y, rw, rh), t);
    // A road across, lanes off it, one on a diagonal.
    fill(&mut g, 0, 22, 64, 3, T::Road);
    fill(&mut g, 20, 0, 2, 22, T::Dirt);
    for i in 0..14 {
        fill(&mut g, 30 + i, 25 + i, 2, 2, T::Dirt);
    }
    // A pond: water in an ellipse, sand round its north shore, stepping stones and a boardwalk.
    for y in 28..44 {
        for x in 2..22 {
            // Doubled px from the pond's middle, over its radii (9 and 7 cells, doubled).
            let (dx, dy) = (x * 2 - 23, y * 2 - 71);
            let d = dx * dx * 14 * 14 + dy * dy * 18 * 18;
            let r = 18 * 18 * 14 * 14;
            if d < r {
                g.set(x, y, T::Water);
            } else if d < r * 3 / 2 && y < 34 {
                g.set(x, y, T::Sand);
            }
        }
    }
    for x in 6..12 {
        g.set(x, 36, T::Boardwalk);
    }
    for (x, y) in [(14, 33), (15, 34), (16, 35)] {
        g.set(x, y, T::Stepping);
    }
    fill(&mut g, 5, 42, 5, 2, T::Ice);
    // Marsh, dried mud, a garden and its crops.
    fill(&mut g, 0, 44, 20, 4, T::Moss);
    fill(&mut g, 24, 42, 8, 6, T::DryBed);
    fill(&mut g, 44, 26, 12, 3, T::Garden);
    fill(&mut g, 44, 29, 12, 3, T::Crops);
    // A cobbled square with two cottages and a brick house on it.
    fill(&mut g, 24, 2, 18, 18, T::Cobble);
    fill(&mut g, 26, 3, 7, 5, T::HouseRoof);
    fill(&mut g, 26, 8, 7, 3, T::HouseWall);
    mats.push((Rect::new(26, 3, 7, 5), Material::RoofSlate));
    fill(&mut g, 35, 4, 6, 4, T::HouseRoof);
    fill(&mut g, 35, 8, 6, 2, T::HouseWall);
    mats.push((Rect::new(35, 4, 6, 4), Material::RoofThatch));
    fill(&mut g, 27, 12, 8, 4, T::HouseRoof);
    fill(&mut g, 27, 16, 8, 3, T::HouseWall);
    mats.push((Rect::new(27, 16, 8, 3), Material::BrickWall));
    fill(&mut g, 37, 13, 3, 1, T::FlowerBed);
    // A wood of broadleaves, pines among them, a dead tree or two; shrubs round its edge.
    fill(&mut g, 44, 0, 20, 12, T::Tree);
    mats.push((Rect::new(56, 0, 8, 7), Material::Pine));
    for (x, y) in [(47, 13), (52, 14), (60, 13)] {
        g.set(x, y, T::DeadTree);
    }
    for (x, y) in [(43, 3), (43, 8), (45, 13), (58, 14), (50, 12)] {
        g.set(x, y, T::Bush);
    }
    // Fences, a stone wall, a hedge, a cliff with rubble under it, long grass, a grown path.
    fill(&mut g, 2, 2, 12, 1, T::Fence);
    fill(&mut g, 2, 2, 1, 8, T::Fence);
    fill(&mut g, 13, 2, 1, 8, T::Fence);
    fill(&mut g, 2, 12, 14, 1, T::StoneWall);
    fill(&mut g, 15, 12, 1, 6, T::StoneWall);
    fill(&mut g, 2, 16, 10, 1, T::Hedge);
    fill(&mut g, 4, 4, 7, 4, T::GrassTall);
    fill(&mut g, 2, 18, 12, 2, T::GrownPath);
    fill(&mut g, 50, 34, 10, 5, T::Cliff);
    for (x, y) in [(49, 40), (52, 40), (58, 41)] {
        g.set(x, y, T::Rubble);
    }
    // The railway, round a bend.
    fill(&mut g, 34, 45, 20, 1, T::Rail);
    for i in 0..5 {
        g.set(54 + i, 45 - i, T::Rail);
        g.set(55 + i, 45 - i, T::Rail);
    }
    fill(&mut g, 60, 30, 1, 12, T::Rail);
    // Two pools that touch only at a corner.
    fill(&mut g, 44, 34, 2, 2, T::Water);
    fill(&mut g, 46, 36, 2, 2, T::Water);
    let mut m = TileMap::new(g, true);
    for (r, mat) in mats {
        m.set_material(r, mat);
    }
    m
}

/// A made-up interior: rooms of every floor walled in every wall, a cave, a glass case, sills,
/// rails in a mine: 48 x 32 cells, indoors.
pub fn sample_indoor() -> TileMap {
    use Tile as T;
    let mut g = Grid::new(48, 32, T::Void);
    let rooms = [
        (Rect::new(1, 1, 14, 9), T::Floor, T::Wall),
        (Rect::new(17, 1, 14, 9), T::TempleFloor, T::TempleWall),
        (Rect::new(33, 1, 14, 9), T::MuseumFloor, T::MuseumWall),
        (Rect::new(1, 12, 14, 9), T::PipeFloor, T::PipeWall),
        (Rect::new(17, 12, 14, 9), T::WorksFloor, T::WorksWall),
        (Rect::new(33, 12, 14, 9), T::SchoolFloor, T::SchoolWall),
        (Rect::new(1, 22, 22, 9), T::CaveFloor, T::CaveWall),
        (Rect::new(25, 22, 22, 9), T::FloorWood, T::WallTop),
    ];
    for (r, floor, wall) in rooms {
        g.fill_rect(r, wall);
        g.fill_rect(r.grow(-1), floor);
        // A wall's depth: the room's top row is two thick, so a top shows over the face.
        g.fill_rect(Rect::new(r.x, r.y + 1, r.w, 1), wall);
    }
    g.fill_rect(Rect::new(38, 5, 3, 1), T::Glass);
    g.fill_rect(Rect::new(7, 9, 2, 1), T::Sill);
    g.fill_rect(Rect::new(3, 26, 16, 1), T::Track);
    g.fill_rect(Rect::new(8, 24, 3, 2), T::Rubble);
    g.fill_rect(Rect::new(20, 4, 3, 3), T::Moss);
    TileMap::new(g, false)
}

/// The terrain goldens: the hash of every chunk of the two samples at seed 7, and of every
/// flora sprite; `tests/terrain_golden.txt` holds them (re-blessed with `jane sheet --bless`).
pub fn golden_file(p: &mut Painter) -> String {
    use std::fmt::Write as _;
    let mut s = String::from(
        "# jane-art terrain goldens: FNV-1a over every chunk of the samples and every flora sprite (ART.md §5).\n\
         # Regenerate with `jane sheet --bless` or `JANE_BLESS=1 cargo test -p jane-art --test terrain`.\n",
    );
    for s2 in p.bank().all() {
        let _ = writeln!(s, "flora/{} {:08x}", s2.0, s2.1.canvas.hash());
    }
    let mut chunk = Chunk::new();
    for (name, map) in [("county", sample_county()), ("indoor", sample_indoor())] {
        let (w, h) = map.size();
        for cy in 0..(h + CHUNK_CELLS - 1) / CHUNK_CELLS {
            for cx in 0..(w + CHUNK_CELLS - 1) / CHUNK_CELLS {
                paint_chunk(p, &map, 7, cx, cy, &mut chunk);
                let _ = writeln!(s, "chunk/{name}/{cx}_{cy} {:08x}", chunk.hash());
            }
        }
    }
    s
}

/// `jane sheet terrain --sample`: the two samples composed at 1x, the county dry then lit by day
/// and by night, and the interior.
pub fn samples(p: &mut Painter) -> Vec<(String, Image)> {
    let mut out = Vec::new();
    for (name, map, outdoor) in [("county", sample_county(), true), ("indoor", sample_indoor(), false)] {
        let (w, h) = map.size();
        let win = Rect::new(0, 0, w * CELL, h * CELL);
        let c = compose(p, &map, 7, win);
        let (iw, ih) = (c.w as u32, c.h as u32);
        let mut img = Image::new(iw, ih, BG);
        for (i, a) in c.albedo.iter().enumerate() {
            img.set(i as u32 % iw, i as u32 / iw, rgba(*a));
        }
        out.push((format!("terrain-sample-{name}"), img));
        if outdoor {
            let sun = Sun {
                azimuth: Angle::WEST.wrapping_add(-4096),
                elevation: Angle::from_degrees(32),
                colour: [250, 226, 186],
            };
            let day = light::light_scene(&c.scene(), &sun, [132, 128, 140], &[], 0);
            let moon = Sun {
                azimuth: Angle::NORTH.wrapping_add(-6000),
                elevation: Angle::from_degrees(50),
                colour: [26, 32, 58],
            };
            let lamps =
                [(12 * CELL, 21 * CELL), (36 * CELL, 21 * CELL), (22 * CELL, 32 * CELL), (40 * CELL, 11 * CELL)]
                    .map(|(x, y)| Point { x, y, z: 40, colour: [190, 150, 96], radius: 7 * CELL });
            let mut night = light::light_scene(&c.scene(), &moon, [70, 72, 124], &lamps, 256);
            grade_night(&mut night);
            for (suffix, px) in [("day", day), ("night", night)] {
                let mut img = Image::new(iw, ih, BG);
                for (i, v) in px.iter().enumerate() {
                    img.set(i as u32 % iw, i as u32 / iw, [v[0], v[1], v[2], 255]);
                }
                out.push((format!("terrain-sample-{name}-{suffix}"), img));
            }
        }
    }
    out
}

/// A lamp in the county: where it stands and what it throws.
fn lamps(bp: &Blueprint, win: Rect) -> Vec<Point> {
    let cat = jane_data::catalog();
    let mut out = Vec::new();
    for prop in &bp.props {
        let row = cat.story.prop(prop.def);
        let Some(l) = row.light else { continue };
        if row.day_only || (row.light_when_on && !prop.on) {
            continue;
        }
        let (x, y) =
            (i32::from(prop.cell.x) * CELL + CELL / 2 - win.x, i32::from(prop.cell.y) * CELL + CELL / 2 - win.y);
        // Fx is 1/256 sim px; a canvas px is half a sim px.
        let radius = l.radius.0 * 2 / 256;
        if x < -radius || y < -radius || x > win.w + radius || y > win.h + radius {
            continue;
        }
        let c = l.color;
        let colour = [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| (v * 190 / 255) as u16);
        out.push(Point { x, y, z: 40, colour, radius: radius.max(CELL) });
    }
    out
}

/// The night's grade, for the sheet (the renderer's grading row is PRESENTATION.md §1.9): the
/// dark shifts toward a deep blue-violet of its own value, so unlit ground keeps a readable value
/// and a colour, while lit pixels (a lamp's pool, a window) keep theirs.
pub fn grade_night(px: &mut [[u8; 3]]) {
    const VIOLET: [i32; 3] = [58, 62, 118];
    for p in px.iter_mut() {
        let l = (299 * i32::from(p[0]) + 587 * i32::from(p[1]) + 114 * i32::from(p[2])) / 1000;
        // How much of the grade: all of it in the dark, none at full light.
        let w = (180 - l).clamp(0, 180) * 256 / 180 * 5 / 8;
        for k in 0..3 {
            let tint = VIOLET[k] * (l + 40) / 128;
            let v = (i32::from(p[k]) * (256 - w) + tint * w) / 256;
            p[k] = v.clamp(0, 255) as u8;
        }
    }
}

/// What `jane sheet county --full` writes.
#[derive(Clone, Debug)]
pub struct CountySheets {
    /// Albedo, normal, emissive and height side by side.
    pub layers: Image,
    /// The albedo alone, at 1x.
    pub albedo: Image,
    /// Lit at five in the afternoon: a low sun in the west-south-west, long shadows east.
    pub day: Image,
    /// Lit at night: a dim blue moon, the lamps that burn after dark, the lit windows.
    pub night: Image,
}

/// `jane sheet county <seed> --full`: the chunk painter over the cells `cells` of a built
/// county `bp`.
pub fn county(p: &mut Painter, bp: &Blueprint, seed: u32, cells: Rect, font: &Font) -> CountySheets {
    let map = TileMap::from_blueprint(bp);
    let win = Rect::new(cells.x * CELL, cells.y * CELL, cells.w * CELL, cells.h * CELL);
    let c = compose(p, &map, seed, win);
    let (w, h) = (c.w as u32, c.h as u32);
    let mut albedo = Image::new(w, h, BG);
    for (i, a) in c.albedo.iter().enumerate() {
        albedo.set(i as u32 % w, i as u32 / w, rgba(*a));
    }
    // Layers side by side.
    let pad = 8;
    let top = 28;
    let mut layers = Image::new(4 * w + 5 * pad, h + top + pad, BG);
    let maxh = c.height.iter().copied().max().unwrap_or(1).max(1);
    let names = ["albedo".to_string(), "normal".into(), "emissive".into(), format!("height 0-{maxh} px")];
    for (k, n) in names.iter().enumerate() {
        label(&mut layers, font, pad + k as u32 * (w + pad), 8, n, Face::Small, TEXT);
    }
    let (dark, light) = (palette::rgb(Ramp::Void.at(Tone::Deep)), palette::rgb(Ramp::Sand.at(Tone::Glint)));
    for i in 0..(w * h) as usize {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let [nx, ny, nz] = decode(c.normal[i]);
        let normal = [(128 + nx) as u8, (128 + ny) as u8, (128 + nz) as u8, 255];
        let e = c.emissive[i];
        let em = if e == Ix::CLEAR {
            let a = rgba(c.albedo[i]);
            [a[0] / 6, a[1] / 6, a[2] / 6, 255]
        } else {
            let g = palette::rgb(e);
            [g[0], g[1], g[2], 255]
        };
        let t = u32::from(c.height[i]);
        let m = u32::from(maxh);
        let ht = [0, 1, 2].map(|k| ((u32::from(dark[k]) * (m - t.min(m)) + u32::from(light[k]) * t.min(m)) / m) as u8);
        for (k, px) in [rgba(c.albedo[i]), normal, em, [ht[0], ht[1], ht[2], 255]].into_iter().enumerate() {
            layers.set(pad + k as u32 * (w + pad) + x, top + y, px);
        }
    }
    // Lit: five in the afternoon, and night.
    let sun =
        Sun { azimuth: Angle::WEST.wrapping_add(-4096), elevation: Angle::from_degrees(32), colour: [250, 226, 186] };
    let day_px = light::light_scene(&c.scene(), &sun, [132, 128, 140], &[], 0);
    let moon =
        Sun { azimuth: Angle::NORTH.wrapping_add(-6000), elevation: Angle::from_degrees(50), colour: [26, 32, 58] };
    let points = lamps(bp, win);
    let mut night_px = light::light_scene(&c.scene(), &moon, [70, 72, 124], &points, 256);
    grade_night(&mut night_px);
    let to_img = |px: &[[u8; 3]]| {
        let mut img = Image::new(w, h, BG);
        for (i, v) in px.iter().enumerate() {
            img.set(i as u32 % w, i as u32 / w, [v[0], v[1], v[2], 255]);
        }
        img
    };
    CountySheets { layers, albedo, day: to_img(&day_px), night: to_img(&night_px) }
}
