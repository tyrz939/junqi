//! Walks the compiled looks table (`jane_data::looks()`, `data/looks/*.json`) and renders every
//! entry: every look, every `vary` variant, every seat of a sprite a player drives (ART.md §1,
//! §3, §5). This is what the atlas builder packs and `jane sheet units` draws; nothing else in
//! the game reads a look.
//!
//! ```text
//! looks::find(name)         -> Option<(SpriteId, &Look)>
//! looks::render(name)       -> Result<Vec<Rendered>, String>   one sprite: variants x seats
//! looks::all()              -> Result<Vec<Rendered>, String>   every look in the table
//! looks::family(Family)     -> Result<Vec<Rendered>, String>   every look of one family
//! ```

use jane_core::ids::SpriteId;
use jane_data::{Controller, Faction, Look, catalog, looks};

use crate::sprite::SpriteSet;
use crate::{creature, kit, person};

/// One rendered set: which sprite, which variant of its `vary`, which seat (0 is the look as
/// written; seats 1 to 3 are the coat swaps of a sprite a player drives).
#[derive(Clone, Debug)]
pub struct Rendered {
    /// The sprite id.
    pub sprite: SpriteId,
    /// The sprite's name (`"jane"`).
    pub name: &'static str,
    /// The variant, below the look's variant count; the renderer picks `h32(unit, 0, VARY) % n`.
    pub variant: u8,
    /// The seat: 0, or 1 to 3 for `jane@1` to `jane@3`.
    pub seat: u8,
    /// Every frame.
    pub set: SpriteSet,
}

impl Rendered {
    /// The atlas key: `jane`, `jane@2`, `villager_old#1`.
    pub fn key(&self) -> String {
        let seat = if self.seat > 0 { format!("@{}", self.seat) } else { String::new() };
        let variant = if self.variant > 0 { format!("#{}", self.variant) } else { String::new() };
        format!("{}{seat}{variant}", self.name)
    }
}

/// A look's generator family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// People (ART.md §2.1).
    Person,
    /// Creatures (§2.2).
    Creature,
    /// The prop kit (§2.3).
    Prop,
}

impl Family {
    /// The family of `look`.
    pub fn of(look: &Look) -> Family {
        match look {
            Look::Person(_) => Family::Person,
            Look::Creature(_) => Family::Creature,
            Look::Prop(_) => Family::Prop,
        }
    }
}

/// The look drawn for sprite `name`.
pub fn find(name: &str) -> Option<(SpriteId, &'static Look)> {
    let sprites = catalog().sprites;
    looks().iter().find(|(id, _)| sprites.get(usize::from(id.0)) == Some(&name)).map(|(id, l)| (*id, l))
}

/// The name of sprite `id`.
pub fn name_of(id: SpriteId) -> &'static str {
    catalog().sprites.get(usize::from(id.0)).copied().unwrap_or("?")
}

/// How many variants sprite `id` has (1 without a look or a `vary`).
pub fn variants(id: SpriteId) -> usize {
    looks().iter().find(|(s, _)| *s == id).map_or(1, |(_, l)| match l {
        Look::Person(p) => p.vary.count(),
        Look::Creature(_) | Look::Prop(_) => 1,
    })
}

/// Whether a player drives some unit drawn as `id`: its look gets the seats' coat swaps.
pub fn has_seats(id: SpriteId) -> bool {
    catalog().combat.units.iter().any(|u| u.sprite == id && u.controller == Controller::Player)
}

/// Whether some unit drawn as `id` fights (it is not friendly and has a spell book): its look
/// gets the attack cycle.
pub fn attacks(id: SpriteId) -> bool {
    catalog().combat.units.iter().any(|u| u.sprite == id && u.faction != Faction::Friendly && !u.book.is_empty())
}

/// Every set sprite `name` renders to: each variant, and for a player's sprite each seat.
pub fn render(name: &str) -> Result<Vec<Rendered>, String> {
    let (id, look) = find(name).ok_or_else(|| format!("no look for \"{name}\""))?;
    render_entry(id, look)
}

fn render_entry(id: SpriteId, look: &Look) -> Result<Vec<Rendered>, String> {
    let name = name_of(id);
    let mut out = Vec::new();
    match look {
        Look::Person(p) => {
            for v in 0..p.vary.count() {
                let set = person::render(&p.variant(v), person::seed(name)).map_err(|e| format!("{name}: {e}"))?;
                let seats = if has_seats(id) { 1 + person::SEAT_COATS.len() } else { 1 };
                for seat in 0..seats {
                    let set = person::seat(&set, seat);
                    out.push(Rendered { sprite: id, name, variant: v as u8, seat: seat as u8, set });
                }
            }
        }
        Look::Creature(c) => {
            let set = creature::render(c, creature::seed(name), attacks(id)).map_err(|e| format!("{name}: {e}"))?;
            out.push(Rendered { sprite: id, name, variant: 0, seat: 0, set });
        }
        Look::Prop(p) => {
            let set = kit::render(p, id, kit::seed(name)).map_err(|e| format!("{name}: {e}"))?;
            out.push(Rendered { sprite: id, name, variant: 0, seat: 0, set });
        }
    }
    Ok(out)
}

/// Every look in the table, rendered.
pub fn all() -> Result<Vec<Rendered>, String> {
    let mut out = Vec::new();
    for (id, look) in looks() {
        out.extend(render_entry(*id, look)?);
    }
    Ok(out)
}

/// Every look of `family`, rendered.
pub fn family(family: Family) -> Result<Vec<Rendered>, String> {
    let mut out = Vec::new();
    for (id, look) in looks().iter().filter(|(_, l)| Family::of(l) == family) {
        out.extend(render_entry(*id, look)?);
    }
    Ok(out)
}
