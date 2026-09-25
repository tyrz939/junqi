//! The county's skeleton: everything about a seed that can be decided on a coarse grid in
//! milliseconds, before a single tile is drawn (WORLDGEN.md, PLAN.md §2.3).
//!
//! One macro cell is 16 x 16 cells; a cell is a metre, so a macro cell is 16 m and the county
//! (125 x 125 macro cells) is 2000 m square.

/// Cells per macro cell, and so metres per macro cell.
pub const MACRO: i32 = 16;
pub const SKEL_W: i32 = 125;
pub const SKEL_H: i32 = 125;
/// The county in cells.
pub const COUNTY_W: i32 = SKEL_W * MACRO;
pub const COUNTY_H: i32 = SKEL_H * MACRO;

/// The three regions, west to east: the catalog's own enum, so a row's `region` and a macro
/// cell's are one type. Lowfields: the south-west, the town, the first walk, every story place.
/// Waters: east of the river. Works: the north, the hill, the School on its crown.
pub use jane_data::Region;

/// Every region, in the enum's order (`r as usize` indexes arrays of three).
pub const REGIONS: [Region; 3] = [Region::Lowfields, Region::Waters, Region::Works];

/// A region's content id (`"lowfields"`).
pub const fn region_name(r: Region) -> &'static str {
    match r {
        Region::Lowfields => "lowfields",
        Region::Waters => "waters",
        Region::Works => "works",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum Biome {
    #[default]
    Field,
    Hedge,
    Wood,
    Foothill,
    Reed,
    Marsh,
    WetWood,
    Garden,
    Slag,
    Yard,
    Hill,
    Town,
}

impl Biome {
    pub const ALL: [Biome; 12] = [
        Biome::Field,
        Biome::Hedge,
        Biome::Wood,
        Biome::Foothill,
        Biome::Reed,
        Biome::Marsh,
        Biome::WetWood,
        Biome::Garden,
        Biome::Slag,
        Biome::Yard,
        Biome::Hill,
        Biome::Town,
    ];
}

/// What water a macro cell holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum Water {
    #[default]
    Dry,
    /// A road may bridge it, dearly.
    River,
    /// Nothing crosses it.
    Lake,
}

pub const fn inside(mx: i32, my: i32) -> bool {
    mx >= 0 && my >= 0 && mx < SKEL_W && my < SKEL_H
}

/// Squared crow's distance between two macro cells, in square metres. Compare, never root.
pub const fn metres_sq(ax: i32, ay: i32, bx: i32, by: i32) -> i64 {
    let dx = (ax - bx) as i64;
    let dy = (ay - by) as i64;
    (dx * dx + dy * dy) * (MACRO as i64) * (MACRO as i64)
}

/// Whether two macro cells are nearer than `tenths` tenths of a metre, crow's distance.
pub const fn nearer_than(ax: i32, ay: i32, bx: i32, by: i32, tenths: i64) -> bool {
    metres_sq(ax, ay, bx, by) * 100 < tenths * tenths
}

/// A macro cell as one number, `y * SKEL_W + x`: the order candidate lists and rankings use.
pub const fn cell_of(x: i32, y: i32) -> u32 {
    (y * SKEL_W + x) as u32
}

/// The macro cell a [`cell_of`] number names.
pub const fn xy_of(c: u32) -> (i32, i32) {
    ((c % SKEL_W as u32) as i32, (c / SKEL_W as u32) as i32)
}
