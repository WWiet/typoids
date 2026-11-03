//! Boss waves: a timed typing test instead of a field of hazards.
//!
//! Every fifth wave the player faces a passage of words and a clock. Typing
//! well drains the boss, a wrong key makes it strike, and running out of time
//! costs hearts. Pure data and math; `game.rs` runs the encounter.

use crate::game::Theme;
use crate::words;

/// A boss arrives on every wave that is a multiple of this.
pub const EVERY: u32 = 5;
/// Seconds of warning before the clock starts.
pub const INTRO: f32 = 3.0;
/// Hearts lost when the clock runs out.
pub const TIMEOUT_HEARTS: u32 = 2;

pub fn is_boss_wave(wave: u32) -> bool {
    wave > 0 && wave / EVERY * EVERY == wave
}

/// 1 for the first boss, 2 for the second, and so on.
pub fn tier_of(wave: u32) -> u32 {
    wave / EVERY
}

/// Words per minute the boss demands: 40, 60, 80, 100, then 120 forever.
pub fn target_wpm(tier: u32) -> u32 {
    (20 + 20 * tier).min(120)
}

/// Words in the passage; later bosses ask for more, up to a cap.
pub fn passage_len(tier: u32) -> usize {
    (12 + 4 * tier as usize).min(40)
}

/// Seconds a strike lasts: the boss lunges and flashes for this long.
pub const STRIKE: f32 = 0.4;
/// How fast the shark swims its lap, in radians per second.
const SWIM: f32 = 0.42;
/// The shark is drawn smaller than the heads: it is long, not tall.
pub const SHARK_SCALE: f32 = 0.72;

/// Size of the boss art, in local units per visual unit.
pub fn art_scale(play_h: f32) -> f32 {
    (play_h * 0.2).clamp(3.0, 8.5)
}

/// Offset from the boss's home spot at `age` seconds: the shark swims slow
/// laps across the water, every other boss hovers and sways a little.
pub fn drift(theme: Theme, age: f32, play_w: f32, play_h: f32) -> (f32, f32) {
    match theme {
        Theme::Underwater => {
            // Half the shark's length in cells, so the lap stays on screen.
            let half = 2.9 * SHARK_SCALE * art_scale(play_h) * 1.25;
            let lap = (play_w * 0.5 - half - 2.0).clamp(0.0, 16.0);
            ((age * SWIM).sin() * lap, (age * 1.1).sin() * 0.3)
        }
        _ => ((age * 0.7).sin() * 0.7, (age * 1.6).sin() * 0.35),
    }
}

/// Which way the shark faces: 1 swimming right, -1 swimming left, eased
/// through 0 at the turns so it banks round instead of flipping.
pub fn heading(age: f32) -> f32 {
    ((age * SWIM).cos() * 4.0).clamp(-1.0, 1.0)
}

pub struct Boss {
    pub tier: u32,
    pub words: Vec<&'static str>,
    /// Index of the word being typed.
    pub idx: usize,
    /// Correct characters so far; a finished word also counts its space,
    /// matching how typing tests count.
    pub chars_done: usize,
    pub chars_total: usize,
    pub wpm: u32,
    /// Seconds allowed, from the target pace.
    pub limit: f32,
    pub elapsed: f32,
    /// Seconds until the clock starts.
    pub intro: f32,
    /// Counts down after the boss strikes, for the flash.
    pub strike: f32,
    /// `Some(true)` once beaten, `Some(false)` once time ran out.
    pub outcome: Option<bool>,
    /// Seconds since the outcome, for the exit animation.
    pub outcome_t: f32,
    /// Seconds since the boss arrived; drives its idle motion.
    pub age: f32,
}

impl Boss {
    /// `pick(n)` returns an index below `n`; the game passes its seeded RNG.
    pub fn new(tier: u32, mut pick: impl FnMut(usize) -> usize) -> Self {
        let n = passage_len(tier);
        let mut words: Vec<&'static str> = Vec::with_capacity(n);
        let mut tries = 0;
        while words.len() < n {
            tries += 1;
            // Bigger bosses lean on longer words.
            let long = (tier as usize * 15).min(60);
            let pool = match pick(100) {
                r if r < long => words::LONG,
                r if r < long + 45 => words::MID,
                _ => words::SHORT,
            };
            let word = pool[pick(pool.len())];
            // Skip an immediate repeat, unless the picker keeps insisting.
            if words.last() != Some(&word) || tries > n * 20 {
                words.push(word);
            }
        }
        let chars_total: usize = words.iter().map(|w| w.len() + 1).sum();
        let wpm = target_wpm(tier);
        Boss {
            tier,
            words,
            idx: 0,
            chars_done: 0,
            chars_total,
            wpm,
            limit: chars_total as f32 / 5.0 / wpm as f32 * 60.0,
            elapsed: 0.0,
            intro: INTRO,
            age: 0.0,
            strike: 0.0,
            outcome: None,
            outcome_t: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        self.outcome.is_none()
    }

    /// True once the intro is over and keys count.
    pub fn fighting(&self) -> bool {
        self.active() && self.intro <= 0.0
    }

    pub fn word(&self) -> Option<&'static str> {
        self.words.get(self.idx).copied()
    }

    /// Fraction of the passage typed, 0 to 1.
    pub fn progress(&self) -> f32 {
        (self.chars_done as f32 / self.chars_total as f32).min(1.0)
    }

    pub fn time_left(&self) -> f32 {
        (self.limit - self.elapsed).max(0.0)
    }

    /// Words per minute so far; 0 until there is a second of data.
    pub fn live_wpm(&self) -> u32 {
        if self.elapsed < 1.0 {
            return 0;
        }
        (self.chars_done as f32 / 5.0 / (self.elapsed / 60.0)).round() as u32
    }
}

pub fn name(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "THE MOTHERSHIP",
        Theme::Underwater => "THE GREAT WHITE",
        Theme::Jungle => "KING KONUT",
        Theme::CatCafe => "THE HEAD BARISTA",
    }
}
