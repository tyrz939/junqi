//! Dungeon themes (ART.md §2.6.1, ART-PLAN M5 and B2): how a generated dungeon is framed and
//! dressed, as data, from `data/looks/dungeon_themes.json`. A theme names the zones that wear it
//! and, for the generic painter in `jane_art::terrain::dungeon`, the head trim of its two-cell
//! faces, its door frames, its floor border, its lane wear, the motifs it repeats on every room's
//! walls and floors, its boss room's floor and emblem, and its grade. Nothing in the painter
//! knows a dungeon by name: another game on this engine adds a row and gets framed, dressed
//! dungeons.
//!
//! Like the looks, a static of its own outside the catalog and its content hash: a theme moves no
//! tile, no collision and no hash of the world. Ramps are `jane_art::palette::Ramp` names (a
//! jane-art test resolves every one); colours are `0xRRGGBB`.

use jane_core::ids::ZoneId;

use crate::{model, model_enum};

model_enum! {
    /// The head trim along the top of a two-cell face.
    pub enum ThemeTrim {
        /// The face's own material, lit along its top.
        Plain,
        /// A squared timber cap, the posts' heads lit under it.
        Cap,
        /// A moulding over a course of dentils.
        Dentil,
        /// A gilt frieze, egg and dart.
        Frieze,
        /// A carved wooden cornice.
        Cornice,
        /// A riveted iron beam.
        Beam,
        /// A string course on corbels.
        Course,
        /// A cornice over a picture rail.
        Rail,
    }
}

model_enum! {
    /// What the upper half of a face is.
    pub enum ThemeUpper {
        /// The face's own walling carried up.
        Carry,
        /// Bookshelves, both halves of the face.
        Shelves,
    }
}

model_enum! {
    /// The inset band a cell in from a room's walls.
    pub enum ThemeBorder {
        /// A nailed edging (the first ramp; the second its nails).
        Edging,
        /// Dark stone with lozenges inlaid (the first ramp the inlay).
        Lozenge,
        /// A strip between two metal lines (the first ramp the lines, the second the strip).
        Inlay,
        /// Hazard stripes on the slant (the first ramp and the second).
        Stripes,
        /// A kerb of the floor's own stone, slotted.
        Kerb,
        /// A painted line (the first ramp).
        Line,
        /// Pebbles (the first ramp).
        Pebbles,
    }
}

model_enum! {
    /// What a door's frame carries.
    pub enum ThemeCarving {
        /// Plain pilasters.
        Plain,
        /// A carved angel on each.
        Angel,
        /// A lamp on each.
        Lamp,
        /// Fluting.
        Fluted,
    }
}

model_enum! {
    /// A motif hung on the faces.
    pub enum WallMotifKind {
        /// A lamp on a hook from a post.
        Lamp,
        /// A candle in an arched niche.
        Niche,
        /// Ivy hung from the head.
        Ivy,
        /// A painting in a gilt frame.
        Painting,
        /// A warning sign.
        Sign,
        /// A valve wheel on a pipe.
        Valve,
        /// A high window, lit by the moon.
        Window,
        /// A cast-iron radiator.
        Radiator,
        /// Coat pegs, a coat on some.
        Pegs,
    }
}

model_enum! {
    /// A motif laid on the floors.
    pub enum FloorMotifKind {
        /// Rails along the lanes.
        Rails,
        /// A channel of water down the room, grates across it.
        Channel,
        /// A belt along the room.
        Belt,
        /// A gantry's shadow across the floor every few cells.
        Gantry,
        /// A carpet two cells in from the walls.
        Carpet,
        /// The high windows' moonlight on the floor.
        Moon,
        /// Bones heaped in a few places.
        Heaps,
        /// Plinths with labels along the north wall.
        Plinths,
        /// Stanchions and a rope along the north band.
        Stanchions,
        /// A chalk drawing.
        Chalk,
        /// A ring of toadstools in a few places.
        Toadstools,
    }
}

model_enum! {
    /// The motif writ large over the boss's room.
    pub enum ThemeEmblem {
        /// Nothing.
        Plain,
        /// A spoked wheel (the first ramp the rim, the second the hub).
        Wheel,
        /// A headframe: a wheel on splayed legs.
        Headframe,
        /// A great angel.
        Angel,
        /// A furnace mouth, glowing.
        Furnace,
        /// A clock face, stopped.
        Clock,
        /// A great portrait in a gilt frame.
        Portrait,
        /// A rose window.
        Rose,
    }
}

model! {
    /// A motif on the faces: what, every how many cells, and how far into its period (px).
    pub struct WallMotif {
        pub kind: WallMotifKind,
        pub every: u8,
        pub at: u8,
    }
}

model! {
    /// A motif on the floors: what, how often (cells, for the motifs that repeat) and how many
    /// (for the ones that are counted), and its ramp (or `""`).
    pub struct FloorMotif {
        pub kind: FloorMotifKind,
        pub every: u8,
        pub count: u8,
        pub ramp: &'static str,
        /// The sprites of scattered props this motif gathers into itself (bones into heaps):
        /// drawn here, not where they lie. Drawing only; a prop anyone can use is never hidden.
        pub gathers: &'static [&'static str],
    }
}

model! {
    /// One theme (`data/looks/dungeon_themes.json`, keyed by its id).
    pub struct DungeonTheme {
        pub id: &'static str,
        /// The zones that wear it.
        pub zones: &'static [ZoneId],
        pub trim: ThemeTrim,
        /// The trim's ramp, or `""` for the face's own.
        pub trim_ramp: &'static str,
        pub upper: ThemeUpper,
        /// The door frames', the posts' and the lintels' ramp.
        pub frame: &'static str,
        pub carving: ThemeCarving,
        pub border: ThemeBorder,
        /// The border's two ramps (`""`: the floor's own).
        pub border_ramps: [&'static str; 2],
        /// A lane's wear: 1 polishes it a tone lighter, -1 grimes it a tone darker, 0 none.
        pub lane_wear: i8,
        pub wall: &'static [WallMotif],
        pub floor: &'static [FloorMotif],
        /// The boss's and the set room's floor: the inlay's ramp, the boss floor's spokes and the
        /// set room's.
        pub inlay: &'static str,
        pub boss_spokes: u8,
        pub set_spokes: u8,
        /// How many braziers stand in the boss's room (0, 2 or 4); the set room has two unless none.
        pub braziers: u8,
        pub emblem: ThemeEmblem,
        pub emblem_ramps: [&'static str; 2],
        /// The grade: what the lit frame is multiplied by, what its shadows lean to, its
        /// saturation (128 as lit), and the tint it shifts to when the boss's fight begins.
        pub tint: u32,
        pub lift: u32,
        pub saturation: u8,
        pub fight: u32,
    }
}

model! {
    /// Every theme, in id order.
    pub struct DungeonThemes {
        pub themes: &'static [DungeonTheme],
    }
}

impl DungeonThemes {
    /// No themes: what a fixture without `data/looks/dungeon_themes.json` compiles to.
    pub const EMPTY: DungeonThemes = DungeonThemes { themes: &[] };

    /// The theme zone `z` wears, if any.
    pub fn of(&self, z: ZoneId) -> Option<&'static DungeonTheme> {
        let themes: &'static [DungeonTheme] = self.themes;
        themes.iter().find(|t| t.zones.contains(&z))
    }
}
