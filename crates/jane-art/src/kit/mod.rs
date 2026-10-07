//! The prop kit (ART.md §2.3, §8 step 5): one routine per family, each drawing its shapes at the
//! size of the footprint the prop rows give (`w * 16` wide, never overhanging it) and `rise` px
//! above it.
//!
//! ```text
//! kit::render(look, sprite, seed) -> SpriteSet   Base (Base2, Base3 by vary), then On and Open
//! kit::footprint(sprite)          -> (w, h)      in cells, from the prop rows that name it
//! ```
//!
//! Every routine ends the same way ([`finish`]): the materials' own tones in clusters, the
//! contact shadow under the foot, the selective outline, then the true heights: a face stands
//! up row by row from the footprint's front edge, 5 px of height to 4 rows as a person does
//! ([`Canvas::upright`]), and a lid or a table top is flat at the height of the face under it
//! ([`Canvas::lid`]); a thing lying flat on the ground is capped at a few px.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

mod barrier;
mod container;
mod fire;
mod furniture;
mod growing;
mod lamp;
mod machine;
pub(crate) mod parts;
mod ritual;
mod set;
mod sign;
mod structure;

use jane_core::grid::Rect;
use jane_core::ids::SpriteId;
use jane_data::{EmitRole, PropFamily, PropLook, PropState};

use crate::canvas::{CELL_PX, Canvas};
use crate::palette::{Ramp, Tone};
use crate::sprite::{FrameId, Role, SpriteSet};

/// A look resolved: its ramps and its box.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Kit {
    pub look: PropLook,
    pub body: Ramp,
    pub trim: Ramp,
    pub accent: Ramp,
    /// The footprint in cells.
    pub fw: i32,
    pub fh: i32,
    /// The canvas: `fw * 16` by `fh * 16 + rise`.
    pub w: i32,
    pub h: i32,
    pub seed: u32,
    /// A house's own make of it (ART-PLAN Q2, [`house_variants`]): a front door's paint and
    /// style, a chimney's pots and stack. None for the look as its row draws it.
    pub house: Option<u32>,
}

impl Kit {
    /// The footprint's front edge: the row the prop stands on.
    pub fn foot(&self) -> i32 {
        self.h - 1
    }

    /// The footprint's top row on the canvas.
    pub fn back(&self) -> i32 {
        self.h - self.fh * CELL_PX
    }

    /// Every ramp the prop is painted in.
    pub fn ramps(&self) -> Vec<Ramp> {
        let mut v = vec![self.body, self.trim, self.accent];
        v.dedup();
        v
    }
}

/// The footprint of the props drawn as `sprite`, in cells: every row that names it must agree.
pub fn footprint(sprite: SpriteId) -> Result<(u8, u8), String> {
    let props = jane_data::catalog().story.props;
    let mut sizes = props.iter().filter(|p| p.sprite == sprite).map(|p| (p.w, p.h));
    let name = jane_data::catalog().sprites.get(usize::from(sprite.0)).copied().unwrap_or("");
    let first = match sizes.next() {
        Some(f) => f,
        None => UNPLACED.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).ok_or("no prop row names this sprite")?,
    };
    match sizes.find(|s| *s != first) {
        Some(other) => Err(format!("its rows disagree on the footprint: {first:?} and {other:?}")),
        None => Ok(first),
    }
}

/// Looks drawn before any prop row names them, and the footprint their rows will give: Pell's
/// iron brazier (the old `campfire_cold`, renamed when that became the fire pit) and the old
/// grate, 2026-10-02. A row that names one wins; drop it from here once its rows are in.
const UNPLACED: [(&str, (u8, u8)); 0] = [];

/// The stable seed of a sprite id.
pub fn seed(sprite: &str) -> u32 {
    crate::person::seed(sprite)
}

/// The shapes hung on the wall over the cell they are placed in (a dungeon's `scatter_wall`:
/// DUNGEONS.md, the scatter): drawn up the face behind the footprint, not standing on it.
const HUNG: [&str; 6] = ["chains", "cobweb", "poster", "frame", "moss", "tools"];

/// Whether `look` hangs on the wall behind its footprint (chains, a cobweb, a notice, a portrait,
/// moss, tools leant against it): its heights stand up from the footprint's back edge, the
/// wall's face's foot, as the face's own do, so it lies on the face in every tier's shadow and
/// never stands a cell in front of the wall as a post (PRESENTATION.md §1.7). The presenter
/// stands it on that row (`jane_present::props`).
pub fn hung(look: &PropLook) -> bool {
    look.family == PropFamily::SmallThing && HUNG.contains(&look.shape)
}

/// A top a thing can stand on (a table, a desk, a workbench, a counter, an altar), `fh` cells
/// deep: `(front, depth)`, the rows from the foot up to the top's front edge and the rows of
/// the top behind it, as its painter draws them (`furniture::draw`). A thing whose footprint
/// lies on it is drawn standing there (`jane_present::props`: Julie's fruit bowl on her table).
pub fn surface(look: &PropLook, fh: i32) -> Option<(i32, i32)> {
    if look.family != PropFamily::Furniture {
        return None;
    }
    match look.shape {
        "table" | "desk" | "workbench" => Some((furniture::TABLE_LEG + 1, furniture::table_depth(fh))),
        "counter" => Some((furniture::COUNTER_FACE, furniture::counter_depth(fh, furniture::COUNTER_FACE))),
        "altar" => Some((furniture::ALTAR_FACE, furniture::counter_depth(fh, furniture::ALTAR_FACE))),
        _ => None,
    }
}

/// Whether `look` is a fire she makes (`lamp` shapes `pit` and `grate`, `fire.rs`): four frames,
/// `Base` cold, `Open2` laid, `On` lit and `Open` ash, whose last embers glow (the one unlit
/// frame that emits).
pub fn fire_pit(look: &PropLook) -> bool {
    look.family == PropFamily::Lamp && matches!(look.shape, "pit" | "grate")
}

/// The frames a look promises: its bases, then `On` and `Open` as its states list them. A made
/// fire ([`fire_pit`]) promises its four: cold, lit, ash, laid.
pub fn frame_ids(look: &PropLook) -> Vec<FrameId> {
    if fire_pit(look) {
        return vec![FrameId::Base, FrameId::On, FrameId::Open, FrameId::Open2];
    }
    let mut v = vec![FrameId::Base];
    if look.vary >= 2 {
        v.push(FrameId::Base2);
    }
    if look.vary >= 3 {
        v.push(FrameId::Base3);
    }
    if look.states.contains(&PropState::On) {
        v.push(FrameId::On);
    }
    if look.states.contains(&PropState::Open) {
        v.push(FrameId::Open);
        // Each base its own open frame, so what is opened keeps the look it had.
        if look.vary >= 2 {
            v.push(FrameId::Open2);
        }
        if look.vary >= 3 {
            v.push(FrameId::Open3);
        }
    }
    v
}

/// What a frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Base,
    On,
    Open,
    /// A made fire's wood laid in it, not yet lit (`fire.rs`).
    Laid,
}

/// Render every frame of `look`, drawn as `sprite` (whose rows give the footprint).
pub fn render(look: &PropLook, sprite: SpriteId, seed: u32) -> Result<SpriteSet, String> {
    let (fw, fh) = footprint(sprite)?;
    let (fw, fh) = (i32::from(fw), i32::from(fh));
    let r = crate::person::ramp;
    let body = r(look.materials.body)?;
    let kit = Kit {
        look: *look,
        body,
        trim: look.materials.trim.map_or(Ok(Ramp::Iron), r)?,
        accent: look.materials.accent.map_or(Ok(Ramp::Brass), r)?,
        fw,
        fh,
        w: fw * CELL_PX,
        h: fh * CELL_PX + i32::from(look.rise),
        seed,
        house: None,
    };
    let mut frames = Vec::new();
    for id in frame_ids(look) {
        let (state, s) = match id {
            // A made fire keeps one seed through its states: the same stones, cold or lit.
            FrameId::Open2 if fire_pit(look) => (State::Laid, seed),
            FrameId::On => (State::On, seed),
            FrameId::Open => (State::Open, seed),
            FrameId::Open2 => (State::Open, seed.wrapping_add(1)),
            FrameId::Open3 => (State::Open, seed.wrapping_add(2)),
            FrameId::Base2 => (State::Base, seed.wrapping_add(1)),
            FrameId::Base3 => (State::Base, seed.wrapping_add(2)),
            _ => (State::Base, seed),
        };
        let k = Kit { seed: s, ..kit };
        let mut c = Canvas::new(k.w, k.h);
        let flat = draw(&mut c, &k, state)?;
        finish(&mut c, &k, flat);
        frames.push((id, c));
    }
    let mut roles = vec![(Role::Body, kit.body)];
    if kit.trim != kit.body {
        roles.push((Role::Trim, kit.trim));
    }
    let emits = if look.emits.contains(&EmitRole::Glass) { vec![Role::Glass, Role::Flame] } else { Vec::new() };
    Ok(SpriteSet { w: kit.w, h: kit.h, ax: 0, ay: kit.back(), frames, roles, emits })
}

/// How many house variants a look has ([`house_variants`]): a front door twelve (four paints by
/// three makes: `terrain::houses::Look::door_variant`), a chimney nine (one to three pots by three
/// heights of stack: `Look::chimney_variant`); anything else none.
pub fn house_count(look: &PropLook) -> u32 {
    match (look.family, look.shape) {
        (PropFamily::Barrier, "door") => 12,
        (PropFamily::Structure, "chimney") => 9,
        _ => 0,
    }
}

/// The base frame of `look`, drawn as `sprite`, in each of its house variants ([`house_count`]):
/// what the renderer draws a door or a chimney on a house with a look as (ART-PLAN Q2). Each is
/// the canvas [`render`] gives, anchored the same.
pub fn house_variants(look: &PropLook, sprite: SpriteId, seed: u32) -> Result<Vec<Canvas>, String> {
    let n = house_count(look);
    if n == 0 {
        return Ok(Vec::new());
    }
    let (fw, fh) = footprint(sprite)?;
    let (fw, fh) = (i32::from(fw), i32::from(fh));
    let r = crate::person::ramp;
    let base = Kit {
        look: *look,
        body: r(look.materials.body)?,
        trim: look.materials.trim.map_or(Ok(Ramp::Iron), r)?,
        accent: look.materials.accent.map_or(Ok(Ramp::Brass), r)?,
        fw,
        fh,
        w: fw * CELL_PX,
        h: fh * CELL_PX + i32::from(look.rise),
        seed,
        house: None,
    };
    (0..n)
        .map(|v| {
            // A door's leaf is painted in its house's paint.
            let body = if look.shape == "door" {
                crate::terrain::houses::DOORS[(v / 3) as usize % crate::terrain::houses::DOORS.len()]
            } else {
                base.body
            };
            let k = Kit { house: Some(v), body, ..base };
            let mut c = Canvas::new(k.w, k.h);
            let flat = draw(&mut c, &k, State::Base)?;
            finish(&mut c, &k, flat);
            Ok(c)
        })
        .collect()
}

/// How the heights are finished: stood up row by row, or lying flat at most `n` px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stand {
    /// Upright from the foot, then these flat tops (a lid, a table's top) at their heights.
    Up(&'static [(Rect, u8)]),
    /// Upright, and the tops given here at run time.
    Tops([(Rect, u8); 2]),
    /// Lying flat, at most this high.
    Flat(u8),
    /// A hearth: lying flat at most `.0` px, but what glows (a flame) stands up from row `.1`,
    /// the bed it burns on.
    Hearth(u8, i32),
}

fn draw(c: &mut Canvas, k: &Kit, state: State) -> Result<Stand, String> {
    let unknown = || format!("no {:?} shape \"{}\"", k.look.family, k.look.shape);
    let own = match k.look.family {
        PropFamily::Sign => sign::draw(c, k, state),
        PropFamily::Lamp => lamp::draw(c, k, state),
        PropFamily::Barrier => barrier::draw(c, k, state),
        PropFamily::Container => container::draw(c, k, state),
        PropFamily::Ritual => ritual::draw(c, k, state),
        PropFamily::Furniture => furniture::draw(c, k, state),
        PropFamily::Structure => structure::draw(c, k, state),
        PropFamily::Vegetation | PropFamily::Debris => growing::draw(c, k, state),
        PropFamily::Machine | PropFamily::SmallThing => machine::draw(c, k, state),
    };
    // A dungeon's set pieces (`set.rs`) are drawn under whichever family their look names.
    own.or_else(|| set::draw(c, k, state)).ok_or_else(unknown)
}

/// What the lit primitives' bands become in wood, stone and metal: a shade, a mid, a base, a
/// light and the high for an edge that catches the sun.
pub(crate) const HARD: [Tone; 8] =
    [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::High, Tone::High];

/// Clusters, the contact shadow, the selective outline, the true heights.
fn finish(c: &mut Canvas, k: &Kit, stand: Stand) {
    // What glows is light, not paint: it keeps its colour through the clean-up and the outline
    // (a candle's flame is five px, and a line would eat it).
    let glow: Vec<(i32, i32, crate::palette::Ix)> = (0..c.h())
        .flat_map(|y| (0..c.w()).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let e = c.emissive_at(x, y);
            (e != crate::palette::Ix::CLEAR).then_some((x, y, e))
        })
        .collect();
    for r in k.ramps() {
        c.declutter(r);
    }
    // Silk is a thread a px wide and pale: despiking would eat it from its ends and a line
    // would turn it to soot. It is the one thing in the kit drawn unlined.
    if k.look.shape != "web" {
        c.despike();
        c.outline();
    }
    for r in k.ramps() {
        c.declutter(r);
    }
    c.relight(&glow);
    match stand {
        Stand::Up(tops) => {
            // A hanging stands on the face's foot, as the face does: what lies on the floor in
            // front of it (a tool's head, the moss's clump) is the ground.
            c.upright(if hung(&k.look) { k.back() } else { k.foot() });
            for &(r, h) in tops {
                c.lid(r, h);
            }
        }
        Stand::Tops(tops) => {
            c.upright(k.foot());
            for (r, h) in tops {
                if r.w > 0 {
                    c.lid(r, h);
                }
            }
        }
        Stand::Flat(h) => c.cap_heights(h),
        Stand::Hearth(h, bed) => {
            c.cap_heights(h);
            for &(x, y, _) in &glow {
                if y < bed {
                    let up = crate::canvas::height_of_rows(bed - y).max(i32::from(h));
                    c.heights_by(Rect::new(x, y, 1, 1), |_, _| up);
                }
            }
        }
    }
}
