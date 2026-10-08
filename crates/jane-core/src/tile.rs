//! Tiles and cell flags. Ids are the TypeScript's, so content and room legends that name a tile
//! by number keep meaning the same tile. The four render-only tiles left the enum
//! (PORT.md §6.i): their ids (45, 46, 47, 49) are gaps, and they live on as [`Material`] paint.

pub const F_SOLID: u8 = 1;
pub const F_BLOCK_LOS: u8 = 2;
pub const F_WATER: u8 = 4;
pub const F_PROP_SOLID: u8 = 8;
pub const F_PROP_LOS: u8 = 16;
pub const F_INDOOR: u8 = 32;
pub const F_OCC: u8 = 64;
/// Feet may cross, a pushed prop may not: the sill of a generated room.
pub const F_NOPUSH: u8 = 128;

/// Bits that survive a tile change: they describe what is on the cell, not the cell.
pub const KEEP_ON_TILE_CHANGE: u8 = F_PROP_SOLID | F_PROP_LOS | F_OCC;
pub const BLOCK_MOVE: u8 = F_SOLID | F_PROP_SOLID;
pub const BLOCK_SIGHT: u8 = F_BLOCK_LOS | F_PROP_LOS;
/// What stops a bolt: sight, plus any solid prop (a barrel is cover).
pub const BLOCK_SHOT: u8 = F_BLOCK_LOS | F_PROP_LOS | F_PROP_SOLID;

macro_rules! tiles {
    ($($name:ident = $id:literal : $flags:expr),* $(,)?) => {
        /// A terrain tile.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
        #[repr(u8)]
        pub enum Tile {
            #[default]
            $($name = $id),*
        }

        impl Tile {
            pub const ALL: &'static [Tile] = &[$(Tile::$name),*];

            /// The tile with this id, or `None` for an id no tile has.
            pub const fn from_id(id: u8) -> Option<Tile> {
                match id {
                    $($id => Some(Tile::$name),)*
                    _ => None,
                }
            }

            pub const fn id(self) -> u8 {
                self as u8
            }

            /// Terrain flags: what the cell is, before any prop or unit.
            pub const fn flags(self) -> u8 {
                match self {
                    $(Tile::$name => $flags),*
                }
            }

            pub const fn name(self) -> &'static str {
                match self {
                    $(Tile::$name => stringify!($name)),*
                }
            }
        }
    };
}

tiles! {
    Void = 0: F_SOLID | F_BLOCK_LOS,
    Grass = 1: 0,
    GrassTall = 2: 0,
    Dirt = 3: 0,
    Road = 4: 0,
    // Water blocks feet, not sight: 2020 set block_los on water but its LOS never tested it.
    Water = 5: F_SOLID | F_WATER,
    Sand = 6: 0,
    Bush = 7: F_SOLID,
    Tree = 8: F_SOLID | F_BLOCK_LOS,
    Fence = 9: F_SOLID,
    HouseWall = 10: F_SOLID | F_BLOCK_LOS,
    HouseRoof = 11: F_SOLID | F_BLOCK_LOS,
    Floor = 12: F_INDOOR,
    FloorWood = 13: F_INDOOR,
    Wall = 14: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    WallTop = 15: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    CaveFloor = 16: F_INDOOR,
    CaveWall = 17: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    TempleFloor = 18: F_INDOOR,
    TempleWall = 19: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    Moss = 20: F_INDOOR,
    DryBed = 21: F_INDOOR,
    Garden = 22: F_INDOOR,
    Rubble = 23: F_SOLID | F_INDOOR,
    Track = 24: F_INDOOR,
    GrownPath = 25: 0,
    Cobble = 26: 0,
    Rail = 27: F_SOLID,
    Cliff = 28: F_SOLID | F_BLOCK_LOS,
    // The floor just inside a generated room's mouth: barrels never leave their room.
    Sill = 29: F_INDOOR | F_NOPUSH,
    // Museum cases: stops feet, not eyes.
    Glass = 30: F_SOLID | F_INDOOR,
    Hedge = 31: F_SOLID | F_BLOCK_LOS,
    Ice = 32: 0,
    MuseumFloor = 33: F_INDOOR,
    MuseumWall = 34: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    PipeFloor = 35: F_INDOOR,
    PipeWall = 36: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    WorksFloor = 37: F_INDOOR,
    WorksWall = 38: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    SchoolFloor = 39: F_INDOOR,
    SchoolWall = 40: F_SOLID | F_BLOCK_LOS | F_INDOOR,
    FlowerBed = 41: 0,
    Stepping = 42: 0,
    Crops = 43: 0,
    // A low dry-stone wall: stops feet, not eyes or bolts.
    StoneWall = 44: F_SOLID,
    Boardwalk = 48: 0,
    // A bare trunk: stops feet, not eyes.
    DeadTree = 50: F_SOLID,
    // The back rows of a house's roof: drawn as the roof, but the ground under them lies behind
    // the house (its roof stands on its walls, `EAVES_ROWS` rows south of its back edge), so feet
    // and eyes pass and she is drawn behind the roof.
    Eaves = 51: 0,
}

/// How many rows at the back of a house's roof are [`Tile::Eaves`]: what the roof overhangs of
/// the ground behind the house. The roof's back edge stands 57 px up and so is drawn three rows
/// and a half north of the ground it stands on (`jane-art`'s `terrain_reach` holds the painter to
/// this); the fourth row is half over the house and blocks, as a prop's base rounds up.
pub const EAVES_ROWS: i32 = 3;

impl Tile {
    /// Drawn as a house's roof: [`Tile::HouseRoof`] and the [`Tile::Eaves`] behind it.
    pub const fn is_roof(self) -> bool {
        matches!(self, Tile::HouseRoof | Tile::Eaves)
    }
}

/// A tile is saved as its id, the number content and room legends use, so a save does not
/// depend on where a variant sits in the enum.
#[cfg(feature = "serde")]
impl serde::Serialize for Tile {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.id())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Tile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Tile, D::Error> {
        let id = <u8 as serde::Deserialize>::deserialize(d)?;
        Tile::from_id(id).ok_or_else(|| serde::de::Error::custom(format_args!("no tile has id {id}")))
    }
}

/// Render-only variation a builder paints over terrain (`Blueprint::paint`). The sim never reads it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Material {
    /// Over `HouseRoof`.
    RoofSlate,
    RoofThatch,
    /// Over `HouseWall`.
    BrickWall,
    /// Over `Tree`: a conifer, a tree in every rule but its drawing.
    Pine,
    /// Over `Dirt`: open earth the land laid (a foothill's, the slag's, the yards'), not a lane,
    /// a yard or a town's ground. Only it drifts into the wild ground beside it (`jane-art`'s
    /// ecotone); dirt without it is laid by hand and keeps its edge.
    WildEarth,
}

impl Material {
    /// The TypeScript tile id this material replaces, for content that still names it.
    pub const fn from_ts_tile(id: u8) -> Option<(Tile, Material)> {
        match id {
            45 => Some((Tile::HouseRoof, Material::RoofSlate)),
            46 => Some((Tile::HouseRoof, Material::RoofThatch)),
            47 => Some((Tile::HouseWall, Material::BrickWall)),
            49 => Some((Tile::Tree, Material::Pine)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_gaps_are_materials() {
        for &t in Tile::ALL {
            assert_eq!(Tile::from_id(t.id()), Some(t));
        }
        for id in [45u8, 46, 47, 49] {
            assert_eq!(Tile::from_id(id), None);
            assert!(Material::from_ts_tile(id).is_some());
        }
        assert_eq!(Tile::ALL.len(), 48);
        assert_eq!(Tile::from_id(51), Some(Tile::Eaves));
        assert_eq!(Tile::from_id(52), None);
    }

    #[test]
    fn a_few_rules() {
        assert_eq!(Tile::Water.flags() & F_BLOCK_LOS, 0);
        assert_ne!(Tile::Water.flags() & F_SOLID, 0);
        assert_eq!(Tile::Glass.flags() & BLOCK_SIGHT, 0);
        assert_eq!(Tile::Sill.flags() & BLOCK_MOVE, 0);
        assert_eq!(Tile::default(), Tile::Void);
    }
}
