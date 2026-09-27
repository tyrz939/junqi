//! The pattern language of `data/audio/songs` (PRESENTATION.md §5): one token a step, like a
//! tracker's column, so a bar of melody reads as a bar.
//!
//! ```text
//! 5 - - 4 3 - 2 .   a degree sounds, `-` holds it, `.` lets it go
//! 1' 7, #4 b7       `'` an octave up, `,` down; `#` and `b` alter it by a semitone
//! 5! 3? 2~          `!` accented, `?` soft, `~` played on some counties and loops, not others
//! x X o             a chord (chord tracks), a hit, an accent, a ghost (drums), a place to drift
//! |                 a bar line, for the eye only
//! ```
//!
//! Melody digits are degrees of the key; bass and arp digits are degrees above the chord's root
//! (1 the root, 3 its third, 5 its fifth, 7 its seventh, 8 the root an octave up), so an
//! arpeggio follows the chords without being written out again.

use crate::model::{Mode, TrackKind};

/// One step of a pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Rest,
    Hold,
    /// A degree (0-based: `1` is 0), a semitone alteration and an octave shift.
    Note { deg: i32, acc: i32, oct: i32, vel: f32, maybe: bool },
    /// A chord, a drum hit, or a place a drift may sound.
    Hit { vel: f32, maybe: bool },
}

/// A chord as the progression names it, for half a bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The root's degree, 0-based, and its alteration (`b7` is 6 and -1).
    pub deg: i32,
    pub acc: i32,
    /// A forced third in semitones (3 minor, 4 major).
    pub third: Option<i32>,
    pub seventh: bool,
    /// 0, 2 or 4: a suspended second or fourth in place of the third.
    pub sus: i32,
}

/// Semitones above the tonic of a degree (0-based, any octave) in `mode`.
pub fn degree(mode: Mode, deg: i32) -> i32 {
    let steps = mode.steps();
    steps[deg.rem_euclid(7) as usize] + 12 * deg.div_euclid(7)
}

impl Chord {
    /// Semitones above the tonic of the chord's root.
    pub fn root(self, mode: Mode) -> i32 {
        degree(mode, self.deg) + self.acc
    }

    /// Semitones above the tonic of chord degree `k` (0 root, 1 second, 2 third, 3 fourth,
    /// 4 fifth, 5 sixth, 6 seventh, 7 the root an octave up, and on). A borrowed root (`b7`)
    /// takes a major chord with a minor seventh unless the token forced the third.
    pub fn tone(self, mode: Mode, k: i32) -> i32 {
        let root = self.root(mode);
        let oct = 12 * k.div_euclid(7);
        let k = k.rem_euclid(7);
        let diatonic = |k: i32| degree(mode, self.deg + k) - degree(mode, self.deg);
        let borrowed = self.acc != 0;
        let semis = match k {
            0 => 0,
            2 => match (self.sus, self.third) {
                (2, _) => if borrowed { 2 } else { diatonic(1) },
                (4, _) => if borrowed { 5 } else { diatonic(3) },
                (_, Some(t)) => t,
                _ if borrowed => 4,
                _ => diatonic(2),
            },
            4 if borrowed => 7,
            6 if borrowed => 10,
            1 if borrowed => 2,
            3 if borrowed => 5,
            5 if borrowed => 9,
            _ => diatonic(k),
        };
        root + semis + oct
    }

    /// The chord's pitch classes above the tonic, root first: three, or four with its seventh.
    pub fn tones(self, mode: Mode) -> Vec<i32> {
        let mut v = vec![self.tone(mode, 0), self.tone(mode, 2), self.tone(mode, 4)];
        if self.seventh {
            v.push(self.tone(mode, 6));
        }
        v
    }
}

/// Parses a pattern for a track of `kind` into its steps.
pub fn parse(s: &str, kind: TrackKind) -> Result<Vec<Step>, String> {
    let mut out = Vec::new();
    for tok in s.split_whitespace() {
        if tok == "|" {
            continue;
        }
        out.push(token(tok, kind)?);
    }
    Ok(out)
}

fn token(tok: &str, kind: TrackKind) -> Result<Step, String> {
    match tok {
        "." => return Ok(Step::Rest),
        "-" => return Ok(Step::Hold),
        _ => {}
    }
    // Trailing marks: velocity and chance.
    let mut body = tok;
    let mut vel = 1.0;
    let mut maybe = false;
    while let Some(c) = body.chars().last() {
        match c {
            '!' => vel = 1.3,
            '?' => vel = 0.55,
            '~' => maybe = true,
            _ => break,
        }
        body = &body[..body.len() - 1];
    }
    let hits = matches!(kind, TrackKind::Chord | TrackKind::Drum | TrackKind::Drift);
    match body {
        "x" if hits => return Ok(Step::Hit { vel, maybe }),
        "X" if hits => return Ok(Step::Hit { vel: vel * 1.3, maybe }),
        "o" if hits => return Ok(Step::Hit { vel: vel * 0.5, maybe }),
        _ => {}
    }
    if matches!(kind, TrackKind::Chord | TrackKind::Drift) {
        return Err(format!("{tok:?}: a {kind:?} track takes x, X, o, - and ."));
    }
    let mut acc = 0;
    let mut rest = body;
    while let Some(r) = rest.strip_prefix('#') {
        acc += 1;
        rest = r;
    }
    while let Some(r) = rest.strip_prefix('b') {
        acc -= 1;
        rest = r;
    }
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return Err(format!("{tok:?} is not a step"));
    }
    let n: i32 = digits.parse().map_err(|_| format!("{tok:?}: bad number"))?;
    if !(1..=15).contains(&n) {
        return Err(format!("{tok:?}: degrees run 1 to 15"));
    }
    let mut oct = 0;
    for c in rest[digits.len()..].chars() {
        match c {
            '\'' => oct += 1,
            ',' => oct -= 1,
            _ => return Err(format!("{tok:?}: what is {c:?}?")),
        }
    }
    Ok(Step::Note { deg: n - 1, acc, oct, vel, maybe })
}

/// A list of plain degrees ("1 3 5 8"), 0-based.
pub fn degrees(s: &str) -> Result<Vec<i32>, String> {
    s.split_whitespace()
        .map(|t| match t.parse::<i32>() {
            Ok(n) if (1..=15).contains(&n) => Ok(n - 1),
            _ => Err(format!("{t:?} is not a degree 1 to 15")),
        })
        .collect()
}

/// A progression, two chords a bar (a bar's one chord fills both halves).
pub fn chords(s: &str, _mode: Mode) -> Result<Vec<Chord>, String> {
    let mut out = Vec::new();
    for tok in s.split_whitespace() {
        if tok == "|" {
            continue;
        }
        match tok.split_once('/') {
            Some((a, b)) => {
                out.push(chord(a)?);
                out.push(chord(b)?);
            }
            None => {
                let c = chord(tok)?;
                out.push(c);
                out.push(c);
            }
        }
    }
    Ok(out)
}

fn chord(tok: &str) -> Result<Chord, String> {
    let mut rest = tok;
    let mut acc = 0;
    if let Some(r) = rest.strip_prefix('#') {
        acc = 1;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('b') {
        acc = -1;
        rest = r;
    }
    let mut ch = rest.chars();
    let d = ch.next().and_then(|c| c.to_digit(10)).ok_or_else(|| format!("chord {tok:?}: a degree 1 to 7 first"))?;
    if !(1..=7).contains(&d) {
        return Err(format!("chord {tok:?}: a degree 1 to 7 first"));
    }
    let mut rest = ch.as_str();
    let mut third = None;
    if let Some(r) = rest.strip_prefix('m') {
        third = Some(3);
        rest = r;
    } else if let Some(r) = rest.strip_prefix('M') {
        third = Some(4);
        rest = r;
    }
    let mut seventh = false;
    if let Some(r) = rest.strip_prefix('7') {
        seventh = true;
        rest = r;
    }
    let sus = match rest {
        "" => 0,
        "s2" => 2,
        "s4" => 4,
        _ => return Err(format!("chord {tok:?}: what is {rest:?}? (m, M, 7, s2, s4)")),
    };
    Ok(Chord { deg: d as i32 - 1, acc, third, seventh, sus })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_read_as_written() {
        let p = parse("5 - | #4' b7,? 1~ .", TrackKind::Melody).unwrap();
        assert_eq!(p.len(), 6);
        assert_eq!(p[1], Step::Hold);
        assert_eq!(p[2], Step::Note { deg: 3, acc: 1, oct: 1, vel: 1.0, maybe: false });
        assert_eq!(p[3], Step::Note { deg: 6, acc: -1, oct: -1, vel: 0.55, maybe: false });
        assert!(matches!(p[4], Step::Note { maybe: true, .. }));
        assert!(parse("x", TrackKind::Melody).is_err());
        assert!(parse("5", TrackKind::Chord).is_err());
        assert!(parse("0", TrackKind::Melody).is_err());
        assert!(parse("5q", TrackKind::Melody).is_err());
    }

    #[test]
    fn chords_follow_the_mode_and_borrowed_roots_are_major() {
        let c = chords("1 4/5M 6 b7", Mode::Minor).unwrap();
        assert_eq!(c.len(), 8);
        // i in D minor: D F A.
        assert_eq!(c[0].tones(Mode::Minor), vec![0, 3, 7]);
        // V with a forced major third: A C# E.
        assert_eq!(c[3].tones(Mode::Minor), vec![7, 11, 14]);
        // VI: Bb D F.
        assert_eq!(c[4].tones(Mode::Minor), vec![8, 12, 15]);
        // An alteration is against the mode: b7 in D minor lowers C to B, and a borrowed root
        // is major: B D# F#.
        assert_eq!(c[6].root(Mode::Minor), 9);
        assert_eq!(c[6].tones(Mode::Minor), vec![9, 13, 16]);
        // sus4 on I in C: C F G; v minor with its seventh: G Bb D F.
        let s = chords("1s4 5m7", Mode::Major).unwrap();
        assert_eq!(s[0].tones(Mode::Major), vec![0, 5, 7]);
        assert_eq!(s[2].tones(Mode::Major), vec![7, 10, 14, 17]);
        assert!(chords("8", Mode::Major).is_err());
        assert!(chords("1q", Mode::Major).is_err());
    }
}
