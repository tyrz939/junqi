//! What the map remembers, and the names she crosses into (EXPERIENCE.md §5.2; the world audit's
//! recommendations 3 and 4, `progress/audit/world-quests.md` §6).
//!
//! **The county's named places** ([`county_places`]): the skeleton's patches as laid (a ring of
//! their radius: "The Long Hedge"), the stories' places (their box: "Rendle's camp") and the
//! way's sites (their ground: "the Gold Mine"). Read off the county's blueprint once a seed.
//!
//! **Crossings** ([`Crossings`]): the first time each day she walks into a patch's ring or a
//! story's box, its name goes up as a short banner. Only a crossing counts: standing in one when
//! the game starts or loads says nothing until she has left it and come back.
//!
//! **The map's ink** ([`MapMemory`]): Hollow Knight's quill. What she learns goes into `pending`
//! (a sign read where it stands, with its words; a place's name, entered or read on a sign), and
//! only a rest inks it: then it is on the chart, with the fire she rested at as a warm dot. Her
//! pins (at most [`PINS`]) are hers at once. Nothing is inked she has not been to or read: a sign
//! and a fire are where she stood, a name is a place entered or a name a sign she read gave. The
//! app keeps the ink and the pins in the slot's note, per seat, as it keeps the tracker.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::blueprint::StoryPlace;
use jane_core::{Blueprint, Rect, ZoneId};
use jane_sim::View;

/// Her own pins on the chart, at most.
pub const PINS: usize = 5;
/// A pin within this many cells of another is that pin (a second press there takes it out).
pub const PIN_NEAR: i32 = 12;

/// A named place's ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A skeleton patch: the ring of its radius, cells.
    Ring { c: (i32, i32), r: i32 },
    /// A story's place, or a site's ground.
    Box(Rect),
}

impl Shape {
    pub fn holds(&self, (x, y): (i32, i32)) -> bool {
        match *self {
            Shape::Ring { c, r } => {
                let (dx, dy) = (i64::from(x - c.0), i64::from(y - c.1));
                dx * dx + dy * dy <= i64::from(r) * i64::from(r)
            }
            Shape::Box(b) => b.contains(x, y),
        }
    }

    /// Where its name is lettered on the chart.
    pub fn centre(&self) -> (i32, i32) {
        match *self {
            Shape::Ring { c, .. } => c,
            Shape::Box(b) => b.centre(),
        }
    }
}

/// A named place of the county.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// As a banner and the chart letter it: "The Long Hedge".
    pub name: String,
    pub shape: Shape,
    /// Crossing into it puts its name up (the patches and the stories; a site's ground is a
    /// building or a town, which has its own door or banner).
    pub banner: bool,
}

/// Capitalised, as a heading starts.
fn heading(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// The county's named places: its patches, then its stories' places, then the way's sites; each
/// name once (the first wins).
pub fn county_places(bp: &Blueprint, sites: &[(String, Rect)]) -> Vec<Place> {
    let cat = jane_data::catalog();
    let mut out: Vec<Place> = Vec::new();
    let push = |out: &mut Vec<Place>, name: String, shape: Shape, banner: bool| {
        if !name.is_empty() && !out.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            out.push(Place { name, shape, banner });
        }
    };
    for a in &bp.areas {
        let jane_core::Key::Name(id) = a.name else { continue };
        let Some(def) = cat.county.areas.iter().find(|d| d.id == id) else { continue };
        let name = heading(cat.text(def.name));
        push(&mut out, name, Shape::Ring { c: a.rect.centre(), r: a.rect.w / 2 }, true);
    }
    for s in bp.stories.values() {
        let StoryPlace::Placed { name, bounds, .. } = s else { continue };
        let name = match *name {
            jane_core::action::TextRef::Text(t) => cat.text(t),
            r @ jane_core::action::TextRef::Local(_) => bp.text(r).unwrap_or(""),
        };
        push(&mut out, heading(name), Shape::Box(*bounds), true);
    }
    for (name, ground) in sites {
        push(&mut out, heading(name), Shape::Box(*ground), false);
    }
    out
}

/// Does a sign's words name `place`? Posts write names in capitals and without "the".
pub fn names(words: &str, place: &str) -> bool {
    let up = words.to_uppercase();
    let p = place.to_uppercase();
    let p = p.strip_prefix("THE ").unwrap_or(&p);
    if p.len() < 4 {
        return false;
    }
    up.match_indices(p).any(|(i, _)| {
        let before = up[..i].chars().next_back();
        let after = up[i + p.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// Is a sign's whole text a place's name ("CLOVER COTTAGE.")? Such a board is a name on the
/// chart, not a post of its own.
pub fn plate(words: &str, place: &str) -> bool {
    let norm = |s: &str| {
        let up = s.trim().trim_end_matches('.').trim().to_uppercase();
        up.strip_prefix("THE ").map_or(up.clone(), str::to_owned)
    };
    norm(words) == norm(place)
}

/// The first time each day she crosses into a named place: which.
#[derive(Clone, Debug, Default)]
pub struct Crossings {
    seed: Option<u32>,
    pub places: Vec<Place>,
    /// Was she inside each, the last look.
    inside: Vec<bool>,
    /// The day the `shown` bits are for.
    day: u32,
    shown: Vec<bool>,
    /// The first look only notes where she stands.
    primed: bool,
}

impl Crossings {
    /// The county's places for `v`'s seed (`sites`: the way's named places, read once a seed).
    pub fn follow(&mut self, v: &View<'_>, sites: impl FnOnce() -> Vec<(String, Rect)>) {
        if self.seed == Some(v.seed()) {
            return;
        }
        self.seed = Some(v.seed());
        self.places = county_places(v.blueprints().get(ZoneId::County), &sites());
        self.inside = vec![false; self.places.len()];
        self.shown = vec![false; self.places.len()];
        self.primed = false;
    }

    /// Where she stands in the county on `day`: the places she has just crossed into, for the
    /// first time today. Out of the county nothing moves.
    pub fn cross(&mut self, at: (i32, i32), day: u32, out: &mut Vec<usize>) {
        if day != self.day {
            self.day = day;
            self.shown.fill(false);
        }
        for (i, p) in self.places.iter().enumerate() {
            let now = p.shape.holds(at);
            let was = core::mem::replace(&mut self.inside[i], now);
            if now && !was && self.primed && p.banner && !self.shown[i] {
                self.shown[i] = true;
                out.push(i);
            }
        }
        self.primed = true;
    }
}

/// A made fire's state, as the chart draws it (the made fires: a burn arc, a cold grey ring).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireState {
    /// A fire the county keeps (the Halt's, a hub's): a warm dot.
    Kept,
    /// One she made: how much of its burn is left, of 255.
    Made { burn: u8 },
    /// A made fire gone out: a cold pit.
    Cold,
}

/// What a mark on the chart says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    Fire(FireState),
    /// A sign read, with its words to read again.
    Sign(String),
    /// A place's name.
    Name(String),
}

impl Note {
    fn same(&self, o: &Note) -> bool {
        match (self, o) {
            (Note::Fire(_), Note::Fire(_)) | (Note::Sign(_), Note::Sign(_)) => true,
            (Note::Name(a), Note::Name(b)) => a == b,
            _ => false,
        }
    }
}

/// A mark on the chart of a zone, at a cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapMark {
    pub zone: ZoneId,
    pub at: (i32, i32),
    pub note: Note,
}

/// One of her pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin {
    pub zone: ZoneId,
    pub at: (i32, i32),
}

/// What a press on the chart did to her pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinEdit {
    Placed,
    Removed,
    /// Every pin is out already.
    Full,
}

/// The chart's ink (see the module doc).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapMemory {
    /// On the chart.
    pub inked: Vec<MapMark>,
    /// Learned since the last rest: inked at the next.
    pub pending: Vec<MapMark>,
    pub pins: Vec<Pin>,
}

impl MapMemory {
    fn has(list: &[MapMark], m: &MapMark) -> Option<usize> {
        list.iter().position(|o| {
            o.zone == m.zone
                && o.note.same(&m.note)
                && match m.note {
                    // A name is one name, wherever it was learned.
                    Note::Name(_) => true,
                    _ => o.at == m.at,
                }
        })
    }

    /// Something learned: on the chart at the next rest, unless it is there or waiting already.
    pub fn learn(&mut self, m: MapMark) {
        if Self::has(&self.inked, &m).is_none() && Self::has(&self.pending, &m).is_none() {
            self.pending.push(m);
        }
    }

    /// A rest: what was learned since the last goes on the chart, with the fire rested at, if
    /// it was one (`fire`).
    pub fn ink(&mut self, fire: Option<(ZoneId, (i32, i32), FireState)>) {
        if let Some((zone, at, state)) = fire {
            let m = MapMark { zone, at, note: Note::Fire(state) };
            match Self::has(&self.inked, &m) {
                Some(i) => self.inked[i].note = m.note,
                None => self.inked.push(m),
            }
        }
        for m in core::mem::take(&mut self.pending) {
            if Self::has(&self.inked, &m).is_none() {
                self.inked.push(m);
            }
        }
    }

    /// The inked marks of a zone.
    pub fn of(&self, zone: ZoneId) -> impl Iterator<Item = &MapMark> + '_ {
        self.inked.iter().filter(move |m| m.zone == zone)
    }

    /// A press on the chart at `at`: her pin there comes out, else one goes in (while she has
    /// one left).
    pub fn toggle_pin(&mut self, zone: ZoneId, at: (i32, i32)) -> PinEdit {
        let near = |p: &Pin| p.zone == zone && (p.at.0 - at.0).abs() <= PIN_NEAR && (p.at.1 - at.1).abs() <= PIN_NEAR;
        if let Some(i) = self.pins.iter().position(near) {
            self.pins.remove(i);
            return PinEdit::Removed;
        }
        if self.pins.len() >= PINS {
            return PinEdit::Full;
        }
        self.pins.push(Pin { zone, at });
        PinEdit::Placed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_names_a_place_in_capitals_and_without_the() {
        assert!(names("EAST: GOLD MINE, 300 m; MUSEUM, 1.2 km.", "the Gold Mine"));
        assert!(names("SCHOOL, NORTH, 400 m.", "The School"));
        assert!(!names("EAST: SCHOOLHOUSE LANE", "The School"), "a whole word only");
        assert!(!names("NORTH: CASTLE", "the Gold Mine"));
        assert!(plate("CLOVER COTTAGE.", "Clover Cottage"));
        assert!(!plate("CLOVER COTTAGE. KNOCK AT THE BACK.", "Clover Cottage"));
    }

    #[test]
    fn ink_waits_for_a_rest_and_pins_are_five() {
        let mut m = MapMemory::default();
        let sign = MapMark { zone: ZoneId::County, at: (10, 10), note: Note::Sign("NORTH: CASTLE".into()) };
        m.learn(sign.clone());
        m.learn(sign.clone());
        assert_eq!(m.of(ZoneId::County).count(), 0, "nothing on the chart before a rest");
        assert_eq!(m.pending.len(), 1, "learned once");
        m.ink(Some((ZoneId::County, (40, 40), FireState::Kept)));
        assert_eq!(m.of(ZoneId::County).count(), 2, "the sign and the fire");
        assert!(m.pending.is_empty());
        m.learn(sign);
        assert!(m.pending.is_empty(), "a sign inked is not learned again");
        for k in 0..PINS as i32 {
            assert_eq!(m.toggle_pin(ZoneId::County, (k * 100, 0)), PinEdit::Placed);
        }
        assert_eq!(m.toggle_pin(ZoneId::County, (900, 900)), PinEdit::Full);
        assert_eq!(m.toggle_pin(ZoneId::County, (203, 4)), PinEdit::Removed, "a press by a pin takes it out");
        assert_eq!(m.pins.len(), PINS - 1);
    }

    #[test]
    fn a_crossing_is_once_a_day_and_never_where_she_starts() {
        let mut c = Crossings {
            places: vec![Place {
                name: "The Long Hedge".into(),
                shape: Shape::Ring { c: (100, 100), r: 50 },
                banner: true,
            }],
            inside: vec![false],
            shown: vec![false],
            ..Crossings::default()
        };
        let mut out = Vec::new();
        c.cross((100, 100), 0, &mut out);
        assert!(out.is_empty(), "standing in it at the start says nothing");
        c.cross((300, 100), 0, &mut out);
        c.cross((120, 100), 0, &mut out);
        assert_eq!(out, [0], "crossed in");
        out.clear();
        c.cross((300, 100), 0, &mut out);
        c.cross((120, 100), 0, &mut out);
        assert!(out.is_empty(), "once a day");
        c.cross((300, 100), 1, &mut out);
        c.cross((140, 100), 1, &mut out);
        assert_eq!(out, [0], "again the next day");
        out.clear();
        c.cross((100, 160), 1, &mut out);
        c.cross((100, 140), 2, &mut out);
        assert_eq!(out, [0], "a ring, not its square: (100, 160) is outside");
    }
}
