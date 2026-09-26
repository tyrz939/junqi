//! The English (PRESENTATION.md §3.7): the sim has none. Content strings come from `jane_data`'s
//! `TEXT` (and a zone's generated texts through `View::text`); everything the UI adds of its own
//! (a toast, a verb, a spell's refusal, a zone's name) is a row here, and a `ToastKind`, `Verb` or
//! `SpellError` without a row is a build error: every match below is exhaustive.
//!
//! Numbers are formatted by integer helpers; nothing here calls a float formatter.

use std::fmt::Write as _;

use jane_core::{QuestId, TextRef};
use jane_data::Region;
use jane_sim::View;
use jane_sim::event::{SpellError, ToastKind};
use jane_sim::interact::Verb;

/// A content string by id.
pub fn text(t: jane_core::TextId) -> &'static str {
    jane_data::catalog().text(t)
}

/// `{name}` becomes the heroine's name; `{place:<story>}` the name that story's place has on
/// this seed (`jane_world::names::story_name`); anything else in braces is left as it is.
pub fn expand(s: &str, heroine: &str, seed: u32, out: &mut String) {
    let mut rest = s;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return;
        };
        let key = &after[..close];
        if key == "name" {
            out.push_str(heroine);
        } else if let Some(story) = key.strip_prefix("place:")
            && let Some(d) = jane_data::catalog().county.stories.iter().find(|d| d.key == story)
        {
            out.push_str(&jane_world::names::story_name(seed, d.id));
        } else {
            out.push('{');
            out.push_str(key);
            out.push('}');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
}

/// [`expand`] into a new string.
pub fn expanded(s: &str, heroine: &str, seed: u32) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    expand(s, heroine, seed, &mut out);
    out
}

/// A string a row or the generator wrote, expanded, into `out`.
pub fn say(v: &View<'_>, r: TextRef, out: &mut String) {
    expand(v.text(r), v.heroine(), v.seed(), out);
}

/// The word on the prompt.
pub fn verb(v: Verb) -> &'static str {
    match v {
        Verb::Enter => "Enter",
        Verb::TryTheDoor => "Try the door",
        Verb::Unlock => "Unlock",
        Verb::Open => "Open",
        Verb::PickUp => "Pick up",
        Verb::Craft => "Craft",
        Verb::Read => "Read",
        Verb::Use => "Use",
        Verb::HoldToPush => "Hold to push",
        Verb::Take(_) => "Take",
        Verb::Talk => "Talk",
        Verb::PutDown => "Put down",
        Verb::Custom(t) => text(t),
    }
}

/// Why a cast failed, as she says it; `None` for the quiet ones (GCD, cooldown, a plain miss).
pub fn spell_error(e: SpellError) -> Option<&'static str> {
    Some(match e {
        SpellError::CastUnsuccessful | SpellError::OnCooldown | SpellError::OnGcd => return None,
        SpellError::YouAreDead => "I can't do that while dead",
        SpellError::TooFar => "Too far",
        SpellError::NoTarget => "I need a target",
        SpellError::NotEnoughMp => "Not enough mana",
        SpellError::NotEnoughEnergy => "Not enough energy",
        SpellError::NotInLos => "Not in line of sight",
        SpellError::NotValidTarget => "I can't cast at that",
    })
}

/// How loud a toast is: the colour of its line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Something she notices.
    Plain,
    /// Something gained or done: a step, a spell, a key that turned.
    Good,
    /// Something refused.
    Refused,
}

/// A toast's words into `out`, and its tone. The view resolves names the event carries by id.
pub fn toast(v: &View<'_>, kind: &ToastKind, out: &mut String) -> Tone {
    let cat = jane_data::catalog();
    let (heroine, seed) = (v.heroine(), v.seed());
    let quest = |q: QuestId| cat.story.quest(q);
    match *kind {
        ToastKind::Text(t) => {
            say(v, t, out);
            Tone::Plain
        }
        ToastKind::PutDownFirst => {
            out.push_str("I should put this down first");
            Tone::Refused
        }
        ToastKind::LeftWhatMattered => {
            out.push_str("Someone left, and handed on what mattered");
            Tone::Plain
        }
        ToastKind::QuestGiven(q) => {
            out.push_str("New: ");
            expand(text(quest(q).name), heroine, seed, out);
            Tone::Good
        }
        ToastKind::QuestDone(q) => {
            out.push_str("Done: ");
            expand(text(quest(q).name), heroine, seed, out);
            Tone::Good
        }
        ToastKind::KillProgress { quest: q, req, n, of } => {
            let d = quest(q);
            if let Some(r) = d.requirements.get(usize::from(req)) {
                expand(text(r.text), heroine, seed, out);
            }
            let _ = write!(out, " {n} of {of}");
            Tone::Good
        }
        ToastKind::InventoryFull => {
            out.push_str("My bag is full");
            Tone::Refused
        }
        ToastKind::TooTired => {
            out.push_str("Too tired");
            Tone::Refused
        }
        ToastKind::Needs { item, qty } => {
            let _ = write!(out, "Needs {} x{qty}", text(cat.combat.item(item).name));
            Tone::Refused
        }
        ToastKind::NothingToRepair => {
            out.push_str("Nothing here to repair");
            Tone::Refused
        }
        ToastKind::NothingGrowsWithoutLight => {
            out.push_str("Nothing grows without light");
            Tone::Refused
        }
        ToastKind::NothingGrows => {
            out.push_str("Nothing here will grow");
            Tone::Refused
        }
        ToastKind::Locked { prop } => {
            let says = v.prop(prop).map(|p| {
                let d = cat.story.prop(p.def);
                let label = v.prop_spawn(p).and_then(|s| s.label).map_or(text(d.name), |l| v.text(l));
                (label, d.locked_says.map(text))
            });
            match says {
                Some((label, Some(s))) => {
                    expand(label, heroine, seed, out);
                    out.push(' ');
                    expand(s, heroine, seed, out);
                }
                Some((label, None)) => {
                    expand(label, heroine, seed, out);
                    out.push_str(" is locked");
                }
                None => out.push_str("It is locked"),
            }
            Tone::Refused
        }
        ToastKind::UnlockedWith(item) => {
            let _ = write!(out, "Unlocked with the {}", text(cat.combat.item(item).name).to_lowercase());
            Tone::Good
        }
        ToastKind::NightLock(t) => {
            say(v, t, out);
            Tone::Refused
        }
        ToastKind::ItShifts => {
            out.push_str("It shifts a little. Hold to push it.");
            Tone::Plain
        }
        ToastKind::NoRoom => {
            out.push_str("No room to put it down");
            Tone::Refused
        }
        ToastKind::ShouldKeep => {
            out.push_str("I should keep that");
            Tone::Refused
        }
        ToastKind::NotHurt => {
            out.push_str("I'm not hurt");
            Tone::Refused
        }
        ToastKind::FitsALock => {
            out.push_str("It fits a lock somewhere");
            Tone::Plain
        }
        ToastKind::NightWaits => {
            out.push_str("The night will not pass until everyone is resting");
            Tone::Plain
        }
        ToastKind::Stronger => {
            out.push_str("A little stronger");
            Tone::Good
        }
        ToastKind::WordsStay => {
            out.push_str("The words stay");
            Tone::Good
        }
        ToastKind::Under { top, found } => {
            let name = |p| v.prop(p).map_or("something", |p| text(cat.story.prop(p.def).name));
            let _ = write!(out, "Under the {}: {}", name(top).to_lowercase(), name(found).to_lowercase());
            Tone::Good
        }
        ToastKind::SpellError(e) => {
            out.push_str(spell_error(e).unwrap_or(""));
            Tone::Refused
        }
        ToastKind::Learned(s) => {
            let _ = write!(out, "Learned {}", text(cat.combat.spell(s).name));
            Tone::Good
        }
        ToastKind::WokeAtRest => {
            out.push_str("I woke where I last rested");
            Tone::Plain
        }
        ToastKind::WokeAtDoor => {
            out.push_str("I woke at the door I came in by");
            Tone::Plain
        }
    }
}

/// A zone's name as the HUD and the banner show it; the county by the region she stands in.
pub fn zone_name(zone: jane_core::ZoneId, region: Region) -> &'static str {
    use jane_core::ZoneId as Z;
    match zone {
        Z::County => match region {
            Region::Lowfields => "The Lowfields",
            Region::Waters => "The Waters",
            Region::Works => "The Works",
        },
        Z::House => "Auntie's House",
        Z::Cellar => "The Cellar",
        Z::Mine => "The Mine",
        Z::Burial => "The Burial Ground",
        Z::Arms => "The Arms",
        Z::Church => "The Church",
        Z::Factory => "The Factory",
        Z::Forest => "The Forest",
        Z::Library => "The Library",
        Z::Museum => "The Museum",
        Z::Pipes => "The Pipes",
        Z::School => "The School",
    }
}

/// `HH:MM` for ticks since midnight (7200 a game hour).
pub fn clock(ticks: u32, out: &mut String) {
    let minutes = ticks / 120 % (24 * 60);
    let _ = write!(out, "{:02}:{:02}", minutes / 60, minutes % 60);
}

/// A time span in ticks as she would read it: "12 s", "3 min", "1 h".
pub fn span(ticks: u32, out: &mut String) {
    let s = ticks.div_ceil(60);
    if s < 120 {
        let _ = write!(out, "{s} s");
    } else if s < 7200 {
        let _ = write!(out, "{} min", s.div_ceil(60));
    } else {
        let _ = write!(out, "{} h", s / 3600);
    }
}

/// A name as it may be used: printable ASCII and single spaces, trimmed, at most 16 characters;
/// "Jane" when nothing is left.
pub fn clean_name(raw: &str) -> String {
    let mut out = String::with_capacity(16);
    let mut space = false;
    for c in raw.trim().chars() {
        if out.chars().count() >= 16 {
            break;
        }
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if !(c.is_ascii_graphic()) {
            continue;
        }
        if space && out.chars().count() < 15 {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    if out.is_empty() { "Jane".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_fills_the_name_and_the_places_and_leaves_the_rest() {
        let mut s = String::new();
        expand("Good night, {name}.", "Tess", 7, &mut s);
        assert_eq!(s, "Good night, Tess.");
        let story = jane_data::catalog().county.stories.iter().find(|d| d.name.is_none()).map(|d| d.key);
        if let Some(id) = story {
            let got = expanded(&format!("Go to {{place:{id}}} now"), "Tess", 7);
            assert!(!got.contains('{') && got.starts_with("Go to ") && got.ends_with(" now"), "{got}");
            // A name is a pure function of the seed.
            assert_eq!(got, expanded(&format!("Go to {{place:{id}}} now"), "Tess", 7));
        }
        assert_eq!(expanded("{} and {nope} and {", "T", 1), "{} and {nope} and {");
    }

    #[test]
    fn a_clock_reads_hours_and_minutes() {
        let mut s = String::new();
        clock(17 * 7200 + 5 * 120, &mut s);
        assert_eq!(s, "17:05");
        s.clear();
        clock(24 * 7200 + 60, &mut s);
        assert_eq!(s, "00:00");
        s.clear();
        span(90, &mut s);
        assert_eq!(s, "2 s");
    }

    #[test]
    fn a_name_is_cleaned_before_it_is_used() {
        assert_eq!(clean_name("  Tess   Mary  "), "Tess Mary");
        assert_eq!(clean_name(""), "Jane");
        assert_eq!(clean_name("\u{7}\t"), "Jane");
        assert_eq!(clean_name("abcdefghijklmnopqrstuvwxyz").len(), 16);
    }

    #[test]
    fn every_spell_error_that_speaks_has_words() {
        for e in [
            SpellError::YouAreDead,
            SpellError::TooFar,
            SpellError::NoTarget,
            SpellError::NotEnoughMp,
            SpellError::NotEnoughEnergy,
            SpellError::NotInLos,
            SpellError::NotValidTarget,
        ] {
            assert_eq!(spell_error(e).is_some(), e.says(), "{e:?}");
        }
    }
}
