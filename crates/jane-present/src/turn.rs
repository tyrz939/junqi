//! The night's turn on screen (NIGHT.md Â§2.2 to Â§2.4, Â§2.6): the gutter, the held dark and the
//! band of returning light from the north at the bell; the slower, shallower turn in silence once
//! the bell has stopped; dawn's band from the south at six. Indoors (the Arms, the church) the
//! room's lamps gutter and come back with no band; Julie's house and cellar and every dungeon
//! never turn.
//!
//! The whole timeline is one pure function, [`look`], of the ticks since the turn (`tick -
//! turned_at`, the sim's latched [`Night`]), the turn's [`Kind`], where it is seen ([`Room`]) and
//! the canvas's height: every seat sees the same frame at the same tick, a save loaded mid-turn
//! continues it exactly (nothing of it is kept but the sim's own field), and every tier is handed
//! the same numbers (a [`Band`] on the light pass and each guttering light's share).
//!
//! What it does not do yet (NIGHT.md Â§9 R3, R4): the night's albedo LUT, the overlays and the
//! night props swapped in the dark second. They go in under the held dark when they land.

use jane_core::ZoneId;
use jane_sim::night::Night;

use crate::frame::Band;

/// Ticks a second (the sim's).
const S: u32 = jane_core::num::TICK_RATE;

/// What turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The bell's turn at nine (or at ten to, on the early bell's Tuesday).
    Bell,
    /// The turn after `bell_stopped`: no stroke, slower, its gutter shallower (Â§2.2).
    Silent,
    /// Dawn's reverse turn at six (Â§2.3): no gutter, the band from the south.
    Dawn,
}

/// Where the turn is seen (Â§2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    /// The county: the gutter, the held dark and the band.
    Open,
    /// The Arms and the church: the room's lamps gutter and come back; no band.
    Inside,
    /// Julie's house and cellar, every dungeon: no turn.
    Never,
}

impl Room {
    pub const fn of(zone: ZoneId) -> Room {
        match zone {
            ZoneId::County => Room::Open,
            ZoneId::Arms | ZoneId::Church => Room::Inside,
            _ => Room::Never,
        }
    }
}

/// One turn's timings and depths, ticks and 256ths.
struct Shape {
    /// The gutter's dip ends.
    gutter: u32,
    /// The held dark ends and the light starts back.
    held: u32,
    /// The turn ends.
    end: u32,
    /// The sky's light at the bottom of the dip, of 256 (20 per cent: never black).
    floor: u16,
    /// A guttering lamp at the bottom of the dip (a fifth of itself).
    lamp: u16,
}

const BELL: Shape = Shape { gutter: 2 * S / 5, held: 7 * S / 5, end: 4 * S, floor: 51, lamp: 51 };
/// Slower, and shallower: the county no longer needs telling.
const SILENT: Shape = Shape { gutter: 4 * S / 5, held: 14 * S / 5, end: 8 * S, floor: 102, lamp: 102 };
/// Dawn: 3 s of band from the south; what it has not reached yet keeps the night's light.
const DAWN_END: u32 = 3 * S;
const DAWN_DARK: u16 = 200;
/// The dawn's held-back light comes in over this many ticks, so the turn starts with no jump.
const DAWN_IN: u32 = S / 4;
/// Indoors the room's light comes back all at once after the held dark, over this many ticks.
const INSIDE_BACK: u32 = 3 * S / 5;

/// The band's soft edge, canvas rows: the night's, and dawn's (wider, gentler); and the light
/// on its leading line over all of it, of 256.
const SOFT: u8 = 40;
const DAWN_SOFT: u8 = 64;
const CREST: u16 = 256;
const DAWN_CREST: u16 = 128;

/// The longest any turn lasts: past it nothing is drawn of one.
pub const LONGEST: u32 = SILENT.end;

/// One moment of the turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    /// The sky's light by canvas row (the light pass's `band`).
    pub band: Band,
    /// A guttering light on the dark side, of 256: lights scale between this and all of
    /// themselves as the band does between its `dark` and all ([`Look::lamp_at`]). The kept
    /// lights (a fire, a hub's lamps, Julie's yard lamp) never gutter.
    pub lamp: u16,
    /// How far in the stage's night grade is, of 256 (`light::night_grade`): it comes in behind
    /// the night's band and leaves under dawn's.
    pub grade: u16,
}

impl Look {
    /// No turn: the night as it stands (or the day).
    pub const NONE: Look = Look { band: Band::NONE, lamp: 256, grade: 256 };

    /// No turn under way.
    pub const fn is_none(&self) -> bool {
        self.band.is_none() && self.lamp >= 256
    }

    /// A guttering light standing at canvas row `y`: its share of itself, of 256.
    pub fn lamp_at(&self, y: i32) -> u32 {
        let lamp = u32::from(self.lamp.min(256));
        if lamp >= 256 {
            return 256;
        }
        lamp + (256 - lamp) * lit(self.band, y) / 256
    }
}

/// How far row `y` is onto the band's lit side, of 256.
fn lit(b: Band, y: i32) -> u32 {
    let into = if b.down { i32::from(b.edge) - y } else { y - i32::from(b.edge) };
    let soft = i32::from(b.soft.max(1));
    into.clamp(0, soft) as u32 * 256 / soft as u32
}

/// `a` toward `b` by `num / den`, 256ths.
fn mix(a: u16, b: u16, num: u32, den: u32) -> u16 {
    let n = num.min(den);
    let (a, b) = (i64::from(a), i64::from(b));
    (a + (b - a) * i64::from(n) / i64::from(den.max(1))) as u16
}

/// The whole screen on the dark side.
const fn all_dark(dark: u16) -> Band {
    Band { edge: -1024, soft: 1, dark, down: true, crest: 0 }
}

/// The turn on screen `since` ticks after it began (`tick - turned_at`), of kind `kind`, at
/// night stage `stage` (0 by day: only dawn's turn runs then), seen in `room` on a canvas `h`
/// rows tall. Pure: the same numbers give the same look on every seat and every tier.
pub fn look(kind: Kind, room: Room, since: u32, stage: u8, h: u16) -> Look {
    if room == Room::Never || (stage == 0) != (kind == Kind::Dawn) {
        return Look::NONE;
    }
    let h = i32::from(h);
    if kind == Kind::Dawn {
        if since >= DAWN_END {
            return Look { grade: 0, ..Look::NONE };
        }
        let grade = mix(256, 0, since, DAWN_END);
        if room == Room::Inside {
            return Look { grade, ..Look::NONE };
        }
        let dark = mix(256, DAWN_DARK, since, DAWN_IN);
        // From the bottom edge to over the top with its trail, the hill last.
        let soft = i32::from(DAWN_SOFT);
        let edge = h - (h + 3 * soft) * since as i32 / DAWN_END as i32;
        let band = Band { edge: edge as i16, soft: DAWN_SOFT, dark, down: false, crest: DAWN_CREST };
        return Look { band, lamp: 256, grade };
    }
    let s = if kind == Kind::Silent { &SILENT } else { &BELL };
    if since >= s.end {
        return Look::NONE;
    }
    if since < s.gutter {
        // The dip, eased in: quick at first, settling at the floor.
        let k = since * 256 / s.gutter;
        let ease = 256 - (256 - k) * (256 - k) / 256;
        let dark = mix(256, s.floor, ease, 256);
        return Look { band: all_dark(dark), lamp: mix(256, s.lamp, ease, 256), grade: 0 };
    }
    if since < s.held {
        return Look { band: all_dark(s.floor), lamp: s.lamp, grade: 0 };
    }
    let back = since - s.held;
    if room == Room::Inside {
        let k = back.min(INSIDE_BACK);
        if back >= INSIDE_BACK {
            return Look::NONE;
        }
        return Look {
            band: all_dark(mix(s.floor, 256, k, INSIDE_BACK)),
            lamp: mix(s.lamp, 256, k, INSIDE_BACK),
            grade: mix(0, 256, k, INSIDE_BACK),
        };
    }
    // The light comes back from the north: the band from the top edge to under the bottom with
    // its trail, so nothing of it is on screen when it ends.
    let span = s.end - s.held;
    let soft = i32::from(SOFT);
    let edge = (h + 3 * soft) * back as i32 / span as i32;
    Look {
        band: Band { edge: edge as i16, soft: SOFT, dark: s.floor, down: true, crest: CREST },
        lamp: s.lamp,
        grade: mix(0, 256, back, span),
    }
}

/// Whether the night's look is in (NIGHT.md §2.2, §2.3) `since` ticks into a turn of `kind`: the
/// LUT, the overlays and the wrong lamps go in when the gutter has reached its floor (the held
/// dark) and leave as dawn's band starts, so no seat sees them swap but in the dark or under the
/// band. A turn kept indoors or never seen swaps at the same moments.
pub const fn night_in(kind: Kind, since: u32) -> bool {
    match kind {
        Kind::Bell => since >= BELL.gutter,
        Kind::Silent => since >= SILENT.gutter,
        Kind::Dawn => since < DAWN_IN,
    }
}

/// The turn under way for the sim's latched `night` at `tick`: its kind and the ticks since it
/// began, or `None` when none is (New Game's tick 0 is no turn; nor is one long past).
/// `bell_stopped` is the world's flag: the night then turns in silence.
pub fn under_way(night: Night, tick: u32, bell_stopped: bool) -> Option<(Kind, u32)> {
    let at = night.turned_at.0;
    if at == 0 || tick < at {
        return None;
    }
    let since = tick - at;
    let kind = match (night.stage, bell_stopped) {
        (0, _) => Kind::Dawn,
        (_, true) => Kind::Silent,
        _ => Kind::Bell,
    };
    let len = match kind {
        Kind::Bell => BELL.end,
        Kind::Silent => SILENT.end,
        Kind::Dawn => DAWN_END,
    };
    (since < len).then_some((kind, since))
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u16 = crate::frame::CANVAS_H;

    #[test]
    fn the_gutter_dips_to_a_fifth_holds_then_the_light_comes_back_from_the_top() {
        let at = |t: u32| look(Kind::Bell, Room::Open, t, 1, H);
        assert!(at(0).band.dark > 240 && at(0).lamp > 240, "it starts from the night as it stood");
        let dark = at(BELL.gutter);
        assert_eq!((dark.band.dark, dark.lamp), (51, 51));
        // Never black: the floor is a fifth of the night's light.
        assert!((0..i32::from(H)).all(|y| dark.band.at(y) >= 51));
        // Held through the dark second.
        assert_eq!(at(BELL.held - 1), dark);
        // The band: the top lit first, the bottom last.
        let mid = at(u32::midpoint(BELL.held, BELL.end));
        assert!(mid.band.at(0) == 256 && mid.band.at(i32::from(H) - 1) == 51, "{mid:?}");
        assert!(mid.lamp_at(0) == 256 && mid.lamp_at(i32::from(H) - 1) == 51);
        assert!(mid.grade > 64 && mid.grade < 200);
        // At four seconds it is over.
        assert_eq!(at(BELL.end), Look::NONE);
        // Row by row the light only ever comes back, never goes again.
        for y in [0, 100, 200, 359] {
            // Up to the crest as it passes, then all of the light.
            let v: alloc::vec::Vec<u32> = (BELL.gutter..BELL.end).map(|t| at(t).band.at(y).min(256)).collect();
            assert!(v.windows(2).all(|w| w[1] >= w[0]), "row {y}");
            assert!((BELL.held..BELL.end).any(|t| at(t).band.at(y) > 300), "row {y}: the crest passes");
        }
    }

    #[test]
    fn the_silent_turn_is_slower_and_shallower_and_dawn_comes_from_the_south() {
        let bell = look(Kind::Bell, Room::Open, BELL.held, 4, H);
        let silent = look(Kind::Silent, Room::Open, SILENT.gutter, 4, H);
        assert!(silent.band.dark > bell.band.dark && SILENT.end == 2 * BELL.end);
        let dawn = look(Kind::Dawn, Room::Open, DAWN_END / 2, 0, H);
        assert!(!dawn.band.down && dawn.band.at(i32::from(H) - 1) >= 256 && dawn.band.at(0) < 256, "{dawn:?}");
        assert_eq!(dawn.lamp, 256, "no gutter at dawn");
        assert_eq!(look(Kind::Dawn, Room::Open, DAWN_END, 0, H).grade, 0, "the stage's grade gone");
        // The day has no night turn, and the night no dawn.
        assert_eq!(look(Kind::Bell, Room::Open, 30, 0, H), Look::NONE);
        assert_eq!(look(Kind::Dawn, Room::Open, 30, 2, H), Look::NONE);
    }

    #[test]
    fn indoors_the_lamps_gutter_and_come_back_with_no_band_and_the_house_never_turns() {
        let t = look(Kind::Bell, Room::Inside, BELL.held + INSIDE_BACK / 2, 2, H);
        assert_eq!(t.band.at(0), t.band.at(i32::from(H) - 1), "no band indoors");
        assert!(t.band.dark > 51 && t.band.dark < 256);
        assert_eq!(look(Kind::Bell, Room::Inside, BELL.held + INSIDE_BACK, 2, H), Look::NONE);
        for z in [ZoneId::House, ZoneId::Cellar, ZoneId::Mine, ZoneId::School] {
            assert_eq!(look(Kind::Bell, Room::of(z), 30, 3, H), Look::NONE, "{z:?}");
        }
    }

    #[test]
    fn the_turn_under_way_is_derived_from_the_latched_night() {
        let n = |stage, at| Night { stage, turned_at: jane_core::Tick(at) };
        assert_eq!(under_way(n(0, 0), 10, false), None, "New Game is no turn");
        assert_eq!(under_way(n(2, 1000), 1030, false), Some((Kind::Bell, 30)));
        assert_eq!(under_way(n(4, 1000), 1030, true), Some((Kind::Silent, 30)));
        assert_eq!(under_way(n(0, 1000), 1030, false), Some((Kind::Dawn, 30)));
        assert_eq!(under_way(n(2, 1000), 1000 + BELL.end, false), None, "long past");
        assert_eq!(under_way(n(4, 1000), 1000 + BELL.end, true), Some((Kind::Silent, BELL.end)));
    }
}
