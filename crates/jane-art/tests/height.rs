//! Height in the chunk painter (MAP.md §6.1, §6.2): a zone without levels paints as it did (no
//! bases, no steps); a zone with levels paints its heights over each cell's base, a plateau's top
//! flat at its level, a face climbing from its foot's to its top's; every deck piece draws.

use jane_art::deck::{self, Kind, Piece};
use jane_art::terrain::levels::LEVEL_PX;
use jane_art::terrain::{CHUNK_CELLS, CHUNK_PX, Chunk, Painter, TileMap};
use jane_core::Tile;
use jane_core::grid::{Grid, Rect};

/// A 32 x 32 ground: level 2 north of row 8, a face in rows 6 and 7, level 1 below; a stair in
/// the face at columns 10 to 12, a waterfall at 13 and 14 under a stream.
fn ground(levels: bool) -> TileMap {
    let (w, h) = (32u32, 32u32);
    let mut tiles = Grid::new(w, h, Tile::Grass);
    tiles.fill_rect(Rect::new(0, 6, 32, 2), Tile::Cliff);
    tiles.fill_rect(Rect::new(10, 6, 3, 2), Tile::Stair);
    tiles.fill_rect(Rect::new(13, 0, 2, 6), Tile::Water);
    tiles.fill_rect(Rect::new(13, 6, 2, 2), Tile::Waterfall);
    tiles.fill_rect(Rect::new(12, 8, 4, 3), Tile::Water);
    let mut m = TileMap::new(tiles, true);
    if levels {
        let mut lv = Grid::new(w, h, 1u8);
        lv.fill_rect(Rect::new(0, 0, 32, 8), 2);
        lv.fill_rect(Rect::new(10, 7, 3, 1), 1);
        m.levels = Some(lv);
    }
    m
}

fn paint(m: &TileMap) -> Chunk {
    let mut p = Painter::new();
    let mut c = Chunk::new();
    jane_art::terrain::paint_chunk(&mut p, m, 7, 0, 0, &mut c);
    c
}

fn h(c: &Chunk, x: i32, y: i32) -> i32 {
    i32::from(c.layers.height[(y * CHUNK_PX + x) as usize])
}

#[test]
fn a_zone_without_levels_paints_no_height() {
    let c = paint(&ground(false));
    assert!(c.bases.iter().all(|&b| b == 0));
    assert!(c.steps.iter().all(|&s| s == 0));
    assert!(c.falls.is_empty());
}

#[test]
fn a_plateau_stands_at_its_level_and_its_face_climbs_from_its_foot() {
    let c = paint(&ground(true));
    let cell = |x: i32, y: i32| c.bases[(y * CHUNK_CELLS + x) as usize];
    assert_eq!(i32::from(cell(3, 3)), 2 * LEVEL_PX, "the plateau's base");
    assert_eq!(i32::from(cell(3, 12)), LEVEL_PX, "the field's");
    assert_eq!(i32::from(cell(3, 6)), LEVEL_PX, "a face stands on its foot");
    // The plateau's top is flat at its level (its tufts are texture), the field's at its own.
    for x in [5, 37, 70] {
        assert_eq!(h(&c, x, 3 * 16 + 5), 2 * LEVEL_PX + 1, "the plateau's top at x {x}");
        assert_eq!(h(&c, x, 13 * 16 + 5), LEVEL_PX + 1, "the field at x {x}");
    }
    // Up the face, its heights rise from the field's to the plateau's.
    let col: Vec<i32> = (6 * 16..8 * 16).map(|y| h(&c, 100, y)).collect();
    assert!(col.first().copied().unwrap_or(0) >= 2 * LEVEL_PX - 2, "its top at the plateau: {col:?}");
    assert!(col.last().copied().unwrap_or(0) <= LEVEL_PX + 2, "its foot at the field: {col:?}");
    assert!(col.windows(2).filter(|w| w[1] > w[0] + 1).count() < 6, "it falls as it goes down: {col:?}");
    // Its cells and the stair's are steps (what a console's blocks leave out); the fall is a fall.
    assert_ne!(c.steps[6] >> 3 & 1, 0);
    assert_ne!(c.steps[7] >> 11 & 1, 0, "the stair");
    assert!(
        c.falls.contains(&(13, 7)) && c.falls.contains(&(14, 6)),
        "{:?} {:016b} {:016b}",
        c.falls,
        c.steps[6],
        c.steps[7]
    );
}

#[test]
fn a_chunk_with_height_paints_the_same_bytes_after_a_zone_without() {
    let lv = ground(true);
    let alone = paint(&lv);
    let mut p = Painter::new();
    let mut c = Chunk::new();
    jane_art::terrain::paint_chunk(&mut p, &lv, 7, 0, 0, &mut c);
    jane_art::terrain::paint_chunk(&mut p, &ground(false), 7, 0, 0, &mut c);
    let flat = c.hash();
    jane_art::terrain::paint_chunk(&mut p, &lv, 7, 0, 0, &mut c);
    assert_eq!(c.hash(), alone.hash());
    jane_art::terrain::paint_chunk(&mut p, &ground(false), 7, 0, 0, &mut c);
    assert_eq!(c.hash(), flat, "the painter keeps nothing of a zone with height");
    assert_eq!(flat, paint(&ground(false)).hash());
}

#[test]
fn every_deck_piece_draws_inside_its_frame() {
    for k in Kind::ALL {
        for p in Piece::ALL {
            let c = deck::render(k, p);
            let (w, h, _, _) = p.frame();
            assert_eq!((c.w(), c.h()), (w, h), "{k:?} {p:?}");
            let opaque = c.albedo().iter().filter(|a| a.is_opaque()).count();
            assert!(opaque * 4 > (16 * 16) as usize, "{k:?} {p:?}: {opaque} px");
        }
    }
}

#[test]
fn a_broken_span_keeps_a_stub_at_each_end_and_an_arch_stands_on_its_piers() {
    let mut out = Vec::new();
    deck::layout((46, 18, 2, 11), false, false, 1, &mut out);
    assert!(out.iter().all(|l| l.y <= 19 || l.y >= 27), "only the stubs: {out:?}");
    assert!(out.iter().any(|l| l.piece == Piece::EdgeW) && !out.is_empty());
    out.clear();
    deck::layout((19, 10, 6, 3), true, true, 1, &mut out);
    let face: Vec<Piece> = out.iter().filter(|l| l.y == 13).map(|l| l.piece).collect();
    assert_eq!(face, [Piece::FacePier, Piece::ArchW, Piece::ArchMid, Piece::ArchMid, Piece::ArchE, Piece::FacePier]);
    assert_eq!(Kind::of(0, 4, false), Kind::Stone);
    assert_eq!(Kind::of(2, 2, false), Kind::Iron);
    assert_eq!(Kind::of(1, 2, true), Kind::Timber);
    assert_eq!(Kind::of(0, 3, true), Kind::Rail);
}
