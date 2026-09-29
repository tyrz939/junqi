//! What the terrain blocks against what it is drawn standing on (the owner's playtest, September
//! 2026: "walking up to lots of things from above doesn't let you get close"). A solid tile blocks
//! its one cell; from above she stops with her feet on its top edge. So whatever a solid tile
//! draws must stand on ground inside that cell: a trunk, a shrub, a stone, a fence or a low wall
//! whose foot were a row further south would keep her a row short of it, and one whose foot were
//! north of it would stand in ground she walks on.
//!
//! Each kind is painted alone on grass and in a run of three west to east (a fence, a hedge, a
//! wall), as the game paints (`Standing::Placed`): the flora as placements of the bank's sprites,
//! the rest into the ground's height layer. Every px it draws stands on ground `rows_up(height)`
//! rows below it (ART.md §1.1); the northmost of those over the tile's columns is at most a row
//! (and the px `rows_up` rounds up) south of the cell's top edge, where her feet stop.
//!
//! Not here, and why: a wood is one mass under one canopy, its trunks on a lattice (`standing`'s
//! `thing`), and the block of wall, roof, cliff or hedge more than a cell deep is drawn in the
//! ground layers under her (only the flora is sorted among units), so its back rows are its top,
//! which she may not stand on; both block cell for cell as drawn.

use jane_art::canvas::rows_up;
use jane_art::terrain::{CELL, Chunk, Painter, Standing, TileMap};
use jane_core::Tile;
use jane_core::grid::Grid;

/// Where the thing stands: cell (6, 8) of a chunk of grass.
const AT: (i32, i32) = (6, 8);

/// Painted into the ground layers under her, and its drawing reaches rows south of its cell: a
/// roof stands on its house's walls, a storey down. She may not step onto the ground it overhangs
/// from above, for she would be drawn over the roof; she stops at its edge as at a wall's top.
const UNDER_HER: [Tile; 1] = [Tile::HouseRoof];

fn scene(t: Tile, run: i32) -> TileMap {
    let mut g = Grid::new(16, 16, Tile::Grass);
    for dx in 0..run {
        g.set(AT.0 + dx, AT.1, t);
    }
    TileMap::new(g, true)
}

/// The northmost and southmost ground rows, chunk px, that what is drawn over columns
/// `x0..x1` stands on.
fn ground_rows(t: Tile, run: i32) -> Option<(i32, i32)> {
    let mut p = Painter::new();
    p.set_standing(Standing::Placed);
    // The grass alone, then with the thing: what it drew is where the heights differ.
    let mut grass = Chunk::new();
    p.paint(&scene(Tile::Grass, 0), 1, 0, 0, &mut grass);
    let mut c = Chunk::new();
    p.paint(&scene(t, run), 1, 0, 0, &mut c);
    let (x0, x1) = (AT.0 * CELL, (AT.0 + run) * CELL);
    let mut span: Option<(i32, i32)> = None;
    let mut stand = |gy: i32| span = Some(span.map_or((gy, gy), |(a, b)| (a.min(gy), b.max(gy))));
    // The flora: each placement's sprite over its foot.
    let bank = p.bank().all();
    for pl in &c.placed {
        let s = bank[usize::from(pl.sprite)].1;
        let (fx, fy) = (i32::from(pl.x), i32::from(pl.y));
        for y in 0..s.canvas.h() {
            for x in 0..s.canvas.w() {
                let (wx, wy) = (fx - s.ax + x, fy - s.ay + y);
                let h = i32::from(s.canvas.height_at(x, y));
                if s.canvas.get(x, y) != jane_art::palette::Ix::CLEAR && (x0..x1).contains(&wx) {
                    stand(wy + rows_up(h));
                }
            }
        }
    }
    // The ground layers: fences, low walls, hedges, walls.
    let side = 16 * CELL;
    for y in 0..side {
        for x in x0..x1 {
            let k = (y * side + x) as usize;
            let h = i32::from(c.layers.height[k]);
            if h != i32::from(grass.layers.height[k]) && h > 1 {
                stand(y + rows_up(h));
            }
        }
    }
    span
}

#[test]
fn every_solid_tile_is_drawn_standing_in_the_cell_it_blocks() {
    let top = AT.1 * CELL;
    let mut bad = Vec::new();
    let mut seen = 0;
    for &t in Tile::ALL {
        if t.flags() & jane_core::tile::BLOCK_MOVE == 0 || matches!(t, Tile::Void | Tile::Water) {
            continue;
        }
        for run in [1, 3] {
            let Some((north, south)) = ground_rows(t, run) else {
                bad.push(format!("{} (run {run}): draws nothing standing", t.name()));
                continue;
            };
            seen += 1;
            eprintln!("{:<12} run {run}: stands on rows {}..={} of its cell", t.name(), north - top, south - top);
            // Its ground's north edge more than a row south of the cell's top: she stops more
            // than a row short of it.
            if north > top + CELL + 1 && !UNDER_HER.contains(&t) {
                bad.push(format!(
                    "{} (run {run}): stands on rows {}..={} of the cell (0..=15 is its own)",
                    t.name(),
                    north - top,
                    south - top
                ));
            }
        }
    }
    assert!(seen >= 20, "the solid tiles were painted ({seen})");
    assert!(bad.is_empty(), "drawn standing outside the cell it blocks: {bad:#?}");
}
