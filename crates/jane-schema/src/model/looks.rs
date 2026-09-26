//! Looks (ART.md §1, §5): how a thing is drawn, compiled from `data/looks/`. Their meaning lives in
//! `jane-art`, which is the only reader. Today the terrain's rows: one [`TileStyle`] per sim tile
//! and one per render-only [`Material`] (ART.md §2.6, PORT.md §6.i), from `data/looks/tiles.json`.
//!
//! Units: `rise` is screen px at 16 px a cell (ART.md §0); `detail` is per pattern (a density out of
//! 16, a course pitch in px); `wet` is 0, 1 or 2 (ART.md §2.6).
//!
//! Looks change no behaviour: the content hash leaves them out, so a palette or pattern edit never
//! refuses a save.

use jane_core::{Material, Tile};

use crate::{model, model_enum};

model_enum! {
    /// What a tile autotiles against (ART.md §2.6).
    pub enum TileGroup {
        /// Open ground: chamfers against other ground, takes a rim where it stands over lower ground.
        Ground,
        /// Water and ice: the lowest ground; a dark band under its bank and a lip elsewhere.
        Water,
        /// A block with a face where its south is open: walls, cliffs, hedges, the void.
        Wall,
        /// A roof: courses, a ridge where its north is not roof, an eave where its south is not.
        Roof,
        /// Stands on the ground: takes the ground of its neighbours (`inherit` when none has any).
        Flora,
        /// Made floor with a painter of its own per cell: boards, slabs, rails, sills, glass.
        Made,
    }
}

model_enum! {
    /// How far a tile stands above the ground plane (ART.md §2.6).
    pub enum TileHeight {
        /// Flat: its `rise` is the few px a bank stands over what is below it.
        Flat,
        /// Raised: a face is drawn below its top; `rise` is the top's height.
        Raised,
        /// Canopy: drawn in the chunk's strips above the y-sort; `rise` is the crown's top.
        Canopy,
    }
}

model_enum! {
    /// The facing the normal layer gets (ART.md §2.6).
    pub enum TileNormal {
        /// Up, with the lift of whatever strokes and stones are drawn on it.
        Flat,
        /// A rock face: south, ridged.
        Cliff,
        /// A built face: south, with mortar grooves.
        Wall,
        /// A pitch by facing: the ridge's slope south, the hips east and west.
        Roof,
        /// Flat; the renderer ripples it.
        Water,
    }
}

model_enum! {
    /// The painter a tile's cells are drawn by: the swatch's per-cell hash pattern (ART.md §2.6).
    pub enum TilePattern {
        /// Nothing: the dark outside a zone.
        Void,
        /// Grass: blades, tufts, now and then a flower, meadow patches.
        Turf,
        /// Bare earth: pebbles, scuffs.
        Earth,
        /// A made road: gravel and ruts along the run.
        Gravel,
        /// Sand: speckle and wind ripples.
        Sand,
        /// Marsh: dark wet patches, a glint of standing water.
        Marsh,
        /// Mud dried and split.
        Cracked,
        /// Dug soil in rows.
        Soil,
        /// Crops: soil with a line of leaf along each furrow.
        Crops,
        /// Setts and flags: irregular laid stones in courses.
        Setts,
        /// Water: depth from the shore, a bank shadow, a lip, ripple glints.
        Water,
        /// Ice: pale, with scratches.
        Ice,
        /// A cliff: a rocky top with a lit lip; a ridged face two cells tall where it drops south.
        Cliff,
        /// Plastered walling, with a window every few cells.
        Plaster,
        /// Brick walling in courses, with a window every few cells.
        Brick,
        /// Clay roof tiles.
        RoofTile,
        /// Slates.
        Slate,
        /// Thatch in bundles.
        Thatch,
        /// Square floor slabs.
        Slabs,
        /// Floorboards.
        Boards,
        /// Dressed stone walling in courses.
        Block,
        /// Rough rock walling.
        Rock,
        /// A rough rock floor.
        RockFloor,
        /// Rails on sleepers on ballast, bending with the line.
        Rail,
        /// A threshold stone.
        Sill,
        /// A glass case.
        Glass,
        /// A clipped hedge.
        Hedge,
        /// Planks laid across the way the walk runs.
        Boardwalk,
        /// Long grass (reeds in the wet).
        Tuft,
        /// A planted flower bed.
        Flowers,
        /// Stepping stones.
        Stepping,
        /// A broadleaf tree, in the strips.
        Tree,
        /// A conifer, in the strips.
        Pine,
        /// A bare dead trunk, in the strips.
        DeadTree,
        /// A shrub, in the strips.
        Bush,
        /// A pile of stones, in the strips.
        Rubble,
        /// A post-and-rail fence, in the strips.
        Fence,
        /// A low dry-stone wall, in the strips.
        StoneWall,
    }
}

model! {
    /// One tile's look (ART.md §2.6): the row `jane_art::terrain` paints it from.
    pub struct TileStyle {
        pub group: TileGroup,
        /// The tile whose ground shows under a standing thing no neighbour lends ground to, or the
        /// tile whose ground this one is drawn as (a grown path is grass).
        pub inherit: Option<Tile>,
        pub height: TileHeight,
        /// Screen px: a ground's standing over lower ground; a wall's or roof's top; a crown's top.
        pub rise: u8,
        /// The swatch's material ramp, by `jane_art::Ramp` name (a jane-art test resolves every one).
        pub ramp: &'static str,
        /// A second ramp the pattern draws with (bark under leaf, timber on plaster), or `""`.
        pub accent: &'static str,
        pub pattern: TilePattern,
        /// The sub-cell table's one number: a density out of 16 or a pitch in px, by pattern.
        pub detail: u8,
        pub normal: TileNormal,
        /// How the tile takes rain: 0 stays matt, 1 darkens, 2 darkens and reflects the sky.
        pub wet: u8,
        /// Out of doors a wide block of it is a building's roof; its face shows where its south is open.
        pub wall_like: bool,
        /// Wild ground, which the speckle filter may redraw as its surroundings.
        pub wild: bool,
    }
}

model! {
    /// The looks compiled so far (ART.md §8 step 3: terrain).
    pub struct Looks {
        /// Every sim tile's style, in `Tile::ALL` order.
        pub tiles: &'static [(Tile, TileStyle)],
        /// Every render-only material's style, in `Material` order.
        pub materials: &'static [(Material, TileStyle)],
    }
}

impl Looks {
    /// No looks: what a fixture without `data/looks` compiles to.
    pub const EMPTY: Looks = Looks { tiles: &[], materials: &[] };

    /// The style of `t`, if the data has one.
    pub fn tile(&self, t: Tile) -> Option<&'static TileStyle> {
        let tiles: &'static [(Tile, TileStyle)] = self.tiles;
        tiles.iter().find(|(k, _)| *k == t).map(|(_, s)| s)
    }

    /// The style of `m`, if the data has one.
    pub fn material(&self, m: Material) -> Option<&'static TileStyle> {
        let mats: &'static [(Material, TileStyle)] = self.materials;
        mats.iter().find(|(k, _)| *k == m).map(|(_, s)| s)
    }
}
