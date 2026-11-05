//! Player settings: plain `key=value` lines in `~/.typoids-settings`.
//! Pure data plus `encode`/`decode`; only `main.rs` turns on writing them.

use std::path::PathBuf;

/// How hard the screen shakes on hits and kills.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Shake {
    #[default]
    Full,
    Low,
    Off,
}

impl Shake {
    pub fn label(self) -> &'static str {
        match self {
            Shake::Full => "FULL",
            Shake::Low => "LOW",
            Shake::Off => "OFF",
        }
    }

    /// Multiplier applied to the shake offset.
    pub fn factor(self) -> f32 {
        match self {
            Shake::Full => 1.0,
            Shake::Low => 0.4,
            Shake::Off => 0.0,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Shake::Full => Shake::Low,
            Shake::Low => Shake::Off,
            Shake::Off => Shake::Full,
        }
    }

    fn code(self) -> u32 {
        match self {
            Shake::Full => 0,
            Shake::Low => 1,
            Shake::Off => 2,
        }
    }

    fn from_code(code: u32) -> Self {
        match code {
            1 => Shake::Low,
            2 => Shake::Off,
            _ => Shake::Full,
        }
    }
}

/// How loud the sound effects play.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Volume {
    #[default]
    Full,
    Low,
    Off,
}

impl Volume {
    pub fn label(self) -> &'static str {
        match self {
            Volume::Full => "FULL",
            Volume::Low => "LOW",
            Volume::Off => "OFF",
        }
    }

    /// Multiplier on every sound.
    pub fn factor(self) -> f32 {
        match self {
            Volume::Full => 1.0,
            Volume::Low => 0.4,
            Volume::Off => 0.0,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Volume::Full => Volume::Low,
            Volume::Low => Volume::Off,
            Volume::Off => Volume::Full,
        }
    }

    fn code(self) -> u32 {
        match self {
            Volume::Full => 0,
            Volume::Low => 1,
            Volume::Off => 2,
        }
    }

    fn from_code(code: u32) -> Self {
        match code {
            1 => Volume::Low,
            2 => Volume::Off,
            _ => Volume::Full,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// Darken every other row like an old CRT.
    pub scanlines: bool,
    pub shake: Shake,
    /// No hit-stop, no call-out motion, no shake.
    pub reduced_motion: bool,
    /// Highest heat level unlocked by winning runs (0 = none yet).
    pub heat_unlocked: u8,
    /// Heat chosen for the next run.
    pub heat: u8,
    pub sound: Volume,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scanlines: true,
            shake: Shake::Full,
            reduced_motion: false,
            heat_unlocked: 0,
            heat: 0,
            sound: Volume::Full,
        }
    }
}

/// The highest heat level there is.
pub const MAX_HEAT: u8 = 5;

impl Settings {
    /// Shake multiplier after reduced motion.
    pub fn shake_factor(&self) -> f32 {
        if self.reduced_motion {
            0.0
        } else {
            self.shake.factor()
        }
    }
}

pub fn encode(s: &Settings) -> String {
    format!(
        "scanlines={}\nshake={}\nreduced_motion={}\nheat_unlocked={}\nheat={}\nsound={}\n",
        s.scanlines as u8,
        s.shake.code(),
        s.reduced_motion as u8,
        s.heat_unlocked,
        s.heat,
        s.sound.code()
    )
}

/// Unknown keys and bad values fall back to the defaults.
pub fn decode(text: &str) -> Settings {
    let mut s = Settings::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Ok(num) = value.trim().parse::<u32>() else {
            continue;
        };
        match key.trim() {
            "scanlines" => s.scanlines = num != 0,
            "shake" => s.shake = Shake::from_code(num),
            "reduced_motion" => s.reduced_motion = num != 0,
            "heat_unlocked" => s.heat_unlocked = num.min(MAX_HEAT as u32) as u8,
            "heat" => s.heat = num.min(MAX_HEAT as u32) as u8,
            "sound" => s.sound = Volume::from_code(num),
            _ => {}
        }
    }
    s.heat = s.heat.min(s.heat_unlocked);
    s
}

pub fn path() -> Option<PathBuf> {
    crate::save::home_file(".typoids-settings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip() {
        let s = Settings {
            scanlines: false,
            shake: Shake::Low,
            reduced_motion: true,
            heat_unlocked: 3,
            heat: 2,
            sound: Volume::Low,
        };
        assert_eq!(decode(&encode(&s)), s);
    }

    #[test]
    fn junk_falls_back_to_defaults_and_heat_stays_unlocked() {
        assert_eq!(decode("nonsense\nshake=banana\n"), Settings::default());
        let s = decode("heat=4\nheat_unlocked=1\n");
        assert_eq!(s.heat, 1, "cannot pick a heat that is still locked");
    }

    #[test]
    fn shake_cycles_and_reduced_motion_silences_it() {
        assert_eq!(Shake::Full.next().next().next(), Shake::Full);
        let s = Settings {
            reduced_motion: true,
            ..Settings::default()
        };
        assert_eq!(s.shake_factor(), 0.0);
    }
}
