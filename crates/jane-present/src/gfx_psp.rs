//! The console's Graphics page (PORT.md §13.13, Pause > Graphics): each costly effect on or off,
//! three presets and the frame rate, kept in `settings.txt` with the pad's settings. The PC never
//! reads it; the PSP's glue turns it into the lister's `fx` and `atmos_fx` bits and the GE's
//! pacing. No sim state: the world steps the same whatever is drawn.

/// One effect the page turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The colour grade (exposure, tint, lift, the afterglow, the saturation).
    Grade,
    /// Lamps' pools of light (off: the flat ambient).
    Lamps,
    /// Lamps' shadows.
    LampShadows,
    /// The sun's and the moon's silhouettes.
    SunShadows,
    /// Sprites' relief to the sun and the lamps (the normal pages).
    Relief,
    /// The mist's drift.
    Fog,
    /// Light shafts against a low sun.
    Shafts,
    /// Water's and puddles' reflections, wet ground, lamp glints on water.
    Reflections,
    /// Effects' particles: sparks, smoke, motes, glints.
    Particles,
    /// The rain and its splashes.
    Rain,
}

impl Effect {
    pub const ALL: [Effect; 10] = [
        Effect::Grade,
        Effect::Lamps,
        Effect::LampShadows,
        Effect::SunShadows,
        Effect::Relief,
        Effect::Fog,
        Effect::Shafts,
        Effect::Reflections,
        Effect::Particles,
        Effect::Rain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Effect::Grade => "Colour grade",
            Effect::Lamps => "Lamp light",
            Effect::LampShadows => "Lamp shadows",
            Effect::SunShadows => "Sun shadows",
            Effect::Relief => "Relief",
            Effect::Fog => "Fog",
            Effect::Shafts => "Light shafts",
            Effect::Reflections => "Reflections",
            Effect::Particles => "Particles",
            Effect::Rain => "Rain",
        }
    }

    /// The settings file's word.
    pub fn key(self) -> &'static str {
        match self {
            Effect::Grade => "grade",
            Effect::Lamps => "lamps",
            Effect::LampShadows => "lamp_shadows",
            Effect::SunShadows => "sun_shadows",
            Effect::Relief => "relief",
            Effect::Fog => "fog",
            Effect::Shafts => "shafts",
            Effect::Reflections => "reflections",
            Effect::Particles => "particles",
            Effect::Rain => "rain",
        }
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// How often a frame is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameRate {
    /// At the vblank after each frame: 60 when it fits.
    Sixty,
    /// Every second vblank: a steady 30.
    #[default]
    Thirty,
    /// As soon as drawn (a tear line), as fast as it goes.
    Unlocked,
}

impl FrameRate {
    pub const ALL: [FrameRate; 3] = [FrameRate::Sixty, FrameRate::Thirty, FrameRate::Unlocked];

    pub fn label(self) -> &'static str {
        match self {
            FrameRate::Sixty => "60",
            FrameRate::Thirty => "30 locked",
            FrameRate::Unlocked => "Unlocked",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            FrameRate::Sixty => "60",
            FrameRate::Thirty => "30",
            FrameRate::Unlocked => "free",
        }
    }

    pub fn from_key(k: &str) -> Option<FrameRate> {
        FrameRate::ALL.into_iter().find(|v| v.key() == k)
    }
}

/// A named set of effects and a frame rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Full,
    Balanced,
    Fast,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Full, Preset::Balanced, Preset::Fast];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Full => "Full",
            Preset::Balanced => "Balanced",
            Preset::Fast => "Fast",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Preset::Full => "full",
            Preset::Balanced => "balanced",
            Preset::Fast => "fast",
        }
    }

    pub fn from_key(k: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|v| v.key() == k)
    }

    /// Its settings (PORT.md §13.12's table says why each effect is where it is).
    pub fn graphics(self) -> Graphics {
        use Effect as E;
        let on: &[Effect] = match self {
            Preset::Full => &Effect::ALL,
            Preset::Balanced => {
                &[E::Grade, E::Lamps, E::LampShadows, E::SunShadows, E::Fog, E::Reflections, E::Particles, E::Rain]
            }
            Preset::Fast => &[E::Grade, E::Lamps, E::Particles, E::Rain],
        };
        Graphics { on: on.iter().fold(0, |m, e| m | e.bit()), rate: FrameRate::Thirty }
    }
}

/// The Graphics page's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graphics {
    /// [`Effect`] bits, set when on.
    pub on: u16,
    pub rate: FrameRate,
}

impl Default for Graphics {
    /// The default a PSP-1000 starts with (PORT.md §13.12).
    fn default() -> Graphics {
        DEFAULT_PRESET.graphics()
    }
}

/// The preset a new player's PSP starts on.
pub const DEFAULT_PRESET: Preset = Preset::Balanced;

impl Graphics {
    pub fn has(self, e: Effect) -> bool {
        self.on & e.bit() != 0
    }

    pub fn set(&mut self, e: Effect, on: bool) {
        if on {
            self.on |= e.bit();
        } else {
            self.on &= !e.bit();
        }
    }

    /// The preset these are, if any (the frame rate is the player's own).
    pub fn preset(self) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.graphics().on == self.on)
    }

    /// `key value` lines for `settings.txt`.
    pub fn write(&self, out: &mut alloc::string::String) {
        use core::fmt::Write as _;
        let _ = writeln!(out, "fps {}", self.rate.key());
        for e in Effect::ALL {
            let _ = writeln!(out, "gfx_{} {}", e.key(), u8::from(self.has(e)));
        }
    }

    /// One `settings.txt` line, if it is the page's (`preset` sets them all).
    pub fn read_line(&mut self, k: &str, v: &str) -> bool {
        if k == "fps" {
            self.rate = FrameRate::from_key(v).unwrap_or(self.rate);
            return true;
        }
        if k == "preset" {
            if let Some(p) = Preset::from_key(v) {
                self.on = p.graphics().on;
            }
            return true;
        }
        let Some(name) = k.strip_prefix("gfx_") else { return false };
        if let Some(e) = Effect::ALL.into_iter().find(|e| e.key() == name) {
            self.set(e, v != "0");
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_read_back_and_presets_are_known() {
        let mut g = Preset::Fast.graphics();
        g.rate = FrameRate::Unlocked;
        let mut s = alloc::string::String::new();
        g.write(&mut s);
        let mut r = Graphics::default();
        for l in s.lines() {
            let (k, v) = l.split_once(' ').unwrap();
            assert!(r.read_line(k, v));
        }
        assert_eq!(r, g);
        assert_eq!(r.preset(), Some(Preset::Fast));
        r.set(Effect::Fog, true);
        assert_eq!(r.preset(), None, "a turned effect is no preset");
        assert_eq!(Preset::Full.graphics().on.count_ones() as usize, Effect::ALL.len());
        assert!(!Graphics::default().has(Effect::Shafts));
    }
}
