//! The `TileStyle` rows (ART.md §2.6), resolved once into what the painter reads per cell: the
//! row itself and its ramps as palette [`Ramp`]s.

use jane_core::{Material, Tile};
use jane_data::{Looks, TileGroup, TileHeight, TileStyle};

use crate::palette::Ramp;

/// One tile's (or material's) look, with its ramps resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// The tile it is the look of (for a material, the tile it paints over).
    pub tile: Tile,
    /// The row as data wrote it.
    pub row: TileStyle,
    /// The swatch's ramp.
    pub ramp: Ramp,
    /// The second ramp, when the row names one.
    pub accent: Option<Ramp>,
}

impl Style {
    /// Stands above the ground plane with a face: walls, roofs, hedges, cliffs.
    pub fn raised(&self) -> bool {
        self.row.height != TileHeight::Flat
    }

    /// Water or ice.
    pub fn is_water(&self) -> bool {
        self.row.group == TileGroup::Water
    }
}

/// Every tile's and material's style, by tile id and material.
#[derive(Clone, Debug)]
pub struct Styles {
    tiles: Vec<Option<Style>>,
    materials: [Option<Style>; 4],
}

/// The tile a material paints over (PORT.md §6.i).
pub const fn over(m: Material) -> Tile {
    match m {
        Material::RoofSlate | Material::RoofThatch => Tile::HouseRoof,
        Material::BrickWall => Tile::HouseWall,
        Material::Pine => Tile::Tree,
    }
}

const fn material_ix(m: Material) -> usize {
    match m {
        Material::RoofSlate => 0,
        Material::RoofThatch => 1,
        Material::BrickWall => 2,
        Material::Pine => 3,
    }
}

fn resolve(tile: Tile, row: &TileStyle, what: &str) -> Result<Style, String> {
    let ramp = Ramp::by_name(row.ramp).ok_or_else(|| format!("{what}: no ramp is called \"{}\"", row.ramp))?;
    let accent = if row.accent.is_empty() {
        None
    } else {
        Some(Ramp::by_name(row.accent).ok_or_else(|| format!("{what}: no ramp is called \"{}\"", row.accent))?)
    };
    Ok(Style { tile, row: *row, ramp, accent })
}

impl Styles {
    /// Resolve every row of `looks`. An unknown ramp name, a tile with no row or a material with
    /// no row is an error naming it.
    pub fn from_looks(looks: &Looks) -> Result<Styles, String> {
        let mut tiles = vec![None; 64];
        for (t, row) in looks.tiles {
            tiles[usize::from(t.id())] = Some(resolve(*t, row, t.name())?);
        }
        for t in Tile::ALL {
            if tiles[usize::from(t.id())].is_none() {
                return Err(format!("tile {} has no look", t.name()));
            }
        }
        let mut materials = [None; 4];
        for (m, row) in looks.materials {
            materials[material_ix(*m)] = Some(resolve(over(*m), row, &format!("{m:?}"))?);
        }
        if materials.iter().any(Option::is_none) {
            return Err("a render-only material has no look".into());
        }
        Ok(Styles { tiles, materials })
    }

    /// The styles compiled into this build (`data/looks/tiles.json`).
    pub fn compiled() -> Styles {
        Styles::from_looks(&jane_data::catalog().looks)
            .expect("data/looks/tiles.json covers every tile (a jane-art test)")
    }

    /// The style of `t`.
    pub fn tile(&self, t: Tile) -> &Style {
        self.tiles[usize::from(t.id())].as_ref().unwrap_or_else(|| self.tiles[0].as_ref().expect("Void has a style"))
    }

    /// The style of tile id `id` (a surface id); Void's for an id no tile has.
    pub fn id(&self, id: u8) -> &Style {
        Tile::from_id(id).map_or_else(|| self.tile(Tile::Void), |t| self.tile(t))
    }

    /// The style a cell is drawn with: `m` when it paints over `t`, else `t`'s.
    pub fn cell(&self, t: Tile, m: Option<Material>) -> &Style {
        match m {
            Some(m) if over(m) == t => self.materials[material_ix(m)].as_ref().unwrap_or_else(|| self.tile(t)),
            _ => self.tile(t),
        }
    }

    /// Every style with its name, tiles in `Tile::ALL` order then the materials: for sheets.
    pub fn all(&self) -> Vec<(String, Tile, Option<Material>)> {
        let mut out: Vec<(String, Tile, Option<Material>)> =
            Tile::ALL.iter().map(|&t| (snake(t.name()), t, None)).collect();
        for m in [Material::RoofSlate, Material::RoofThatch, Material::BrickWall, Material::Pine] {
            out.push((snake(&format!("{m:?}")), over(m), Some(m)));
        }
        out
    }
}

/// `GrassTall` as `grass_tall`, the name data keys a look by.
pub fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
