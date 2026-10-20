//! A grid read back (postcard, as saves and baked zones are) is the grid written, held at exactly
//! its cells: a county's 1536 x 1536 tiles are read into one list of their size, never grown by
//! doubling past serde's 1 MB first guess (the PSP's crash after a played world: a 2 MB block
//! asked for in RAM split into smaller ones, 2026-10-21).

use jane_core::grid::Grid;

fn round(g: &Grid<u8>) -> Grid<u8> {
    let bytes = postcard::to_allocvec(g).expect("written");
    postcard::from_bytes(&bytes).expect("read")
}

#[test]
fn a_big_grid_reads_back_whole_at_its_size() {
    let (w, h) = (1536u32, 1536u32);
    let cells: Vec<u8> = (0..w * h).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8).collect();
    let g = Grid::from_vec(w, h, cells);
    let r = round(&g);
    assert_eq!(r, g);
    assert_eq!(r.capacity(), (w * h) as usize, "held at exactly its cells");
}

#[test]
fn a_hollow_grid_reads_back_hollow() {
    let r = round(&Grid::hollow(300, 200));
    assert!(r.is_hollow());
    assert_eq!((r.w(), r.h(), r.capacity()), (300, 200, 0));
}

#[test]
fn cells_not_its_size_are_refused() {
    let bytes = postcard::to_allocvec(&(4u32, 4u32, vec![1u8; 15])).expect("written");
    assert!(postcard::from_bytes::<Grid<u8>>(&bytes).is_err());
    // More cells than it says are refused too, however few it says.
    let bytes = postcard::to_allocvec(&(1u32, 1u32, vec![1u8; 3])).expect("written");
    assert!(postcard::from_bytes::<Grid<u8>>(&bytes).is_err());
}
