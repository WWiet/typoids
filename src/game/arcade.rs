//! The arcade layer around a run: call-outs, the rolling score, typing stats,
//! the high-score table and initials, victory, heat, options, the how-to-play
//! card, the attract mode, and the daily run.

use super::*;
use crate::scores::{self, Entry};
use crate::settings::{self, MAX_HEAT};

/// A big pixel call-out. `t0` may lie in the future to queue one up.
#[derive(Clone, Debug)]
pub struct Callout {
    pub text: String,
    pub t0: f32,
    pub dur: f32,
}

/// How a wave (or a whole run) was typed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub keys_ok: u32,
    pub keys_bad: u32,
    /// Seconds spent actually typing (gaps over 1.5 s are not counted).
    pub typing: f32,
    /// Seconds of play.
    pub secs: f32,
    pub best_combo: u32,
}

impl Stats {
    /// Correct keys as a percentage of all keys, once anything was typed.
    pub fn accuracy(&self) -> Option<u32> {
        let total = self.keys_ok + self.keys_bad;
        (total > 0).then(|| (self.keys_ok as f32 * 100.0 / total as f32).round() as u32)
    }

    /// Words (of five letters) per minute of typing.
    pub fn wpm(&self) -> Option<u32> {
        (self.typing >= 2.0)
            .then(|| (self.keys_ok as f32 / 5.0 / (self.typing / 60.0)).round() as u32)
    }

    /// One short line for the shop heading and the game-over popup.
    pub fn summary(&self) -> String {
        let acc = self
            .accuracy()
            .map_or("--".to_string(), |a| format!("{a}%"));
        let wpm = self.wpm().map_or("--".to_string(), |w| w.to_string());
        format!(
            "ACC {acc}  ·  {wpm} WPM  ·  BEST COMBO x{}",
            self.best_combo
        )
    }
}

/// Combo milestones and what the announcer shouts.
const COMBO_CALLS: [(u32, &str); 5] = [
    (5, "NICE!"),
    (10, "GREAT!"),
    (15, "RADICAL!"),
    (20, "TUBULAR!"),
    (30, "UNSTOPPABLE!"),
];
const ANNOUNCE_SECS: f32 = 0.9;

pub const VICTORY_ITEMS: &[&str] = &["ENDLESS", "FINISH RUN"];
pub const VICTORY_ENDLESS: usize = 0;
pub const VICTORY_FINISH: usize = 1;

pub const OPTION_ITEMS: &[&str] = &[
    "SCANLINES",
    "SCREEN SHAKE",
    "REDUCED MOTION",
    "SOUND",
    "BACK",
];

/// What each heat level adds. Heat N plays with every rule up to N.
pub const HEAT_RULES: [&str; 5] = [
    "hazards 12% faster",
    "one card fewer in the shop",
    "a miss breaks the combo",
    "bosses demand 10 more WPM",
    "start with two hearts",
];
/// Speed multiplier from heat 1 on.
pub(super) const HEAT_SPEED: f32 = 1.12;

/// Idle seconds on the title before the high-score table shows, how long it
/// stays, and how often it comes back.
const ATTRACT_AFTER: f32 = 8.0;
const ATTRACT_SHOW: f32 = 5.0;
const ATTRACT_CYCLE: f32 = 13.0;

impl Game {
    // --- call-outs -------------------------------------------------------

    pub(super) fn callout(&mut self, text: impl Into<String>, delay: f32, dur: f32) {
        self.callouts.push(Callout {
            text: text.into(),
            t0: self.time + delay,
            dur,
        });
    }

    /// The call-out on screen right now (the newest one that has started).
    pub fn active_callout(&self) -> Option<&Callout> {
        self.callouts
            .iter()
            .rev()
            .find(|c| self.time >= c.t0 && self.time < c.t0 + c.dur)
    }

    pub(super) fn prune_callouts(&mut self) {
        let now = self.time;
        self.callouts.retain(|c| now < c.t0 + c.dur);
        if self.announce.as_ref().is_some_and(|a| now >= a.t0 + a.dur) {
            self.announce = None;
        }
    }

    /// Shout when the combo crosses a milestone.
    pub(super) fn announce_combo(&mut self, before: u32) {
        let hit = COMBO_CALLS
            .iter()
            .rev()
            .find(|&&(n, _)| before < n && self.combo >= n);
        if let Some(&(_, text)) = hit {
            self.announce = Some(Callout {
                text: text.to_string(),
                t0: self.time,
                dur: ANNOUNCE_SECS,
            });
        }
    }

    /// Seconds a combo lasts after a kill.
    pub(super) fn combo_window(&self) -> f32 {
        3.4 + 1.5 * self.upgrades.level(Upgrade::LongFuse) as f32
    }

    /// How much of the combo timer is left, 1 down to 0.
    pub fn combo_ratio(&self) -> f32 {
        if self.combo == 0 {
            return 0.0;
        }
        (self.combo_left / self.combo_window()).clamp(0.0, 1.0)
    }

    // --- odometer --------------------------------------------------------

    /// Roll the shown score and crystals toward the real ones.
    pub(super) fn tick_counters(&mut self, dt: f32) {
        self.shown_score = roll(self.shown_score, self.score, dt);
        self.shown_crystals = roll(self.shown_crystals, self.crystals, dt);
    }

    /// Jump the counters to the real values (new run, loaded run).
    pub fn snap_counters(&mut self) {
        self.shown_score = self.score;
        self.shown_crystals = self.crystals;
    }

    pub fn score_rolling(&self) -> bool {
        self.shown_score != self.score
    }

    // --- typing stats ----------------------------------------------------

    pub(super) fn note_key(&mut self, ok: bool) {
        let gap = self.time - self.last_key_t;
        // A burst of keys counts the time between them; the first key of a
        // burst counts a typical keystroke.
        let typing = match (ok, gap <= 1.5) {
            (false, _) => 0.0,
            (true, true) => gap.max(0.0),
            (true, false) => 0.2,
        };
        for s in [&mut self.wave_stats, &mut self.run_stats] {
            if ok {
                s.keys_ok += 1;
                s.typing += typing;
            } else {
                s.keys_bad += 1;
            }
        }
        if ok {
            self.last_key_t = self.time;
        }
    }

    // --- end of a run ----------------------------------------------------

    /// The run is over: record it, then ask for initials if the score made
    /// the table, or go straight to the game-over popup.
    pub(super) fn finish_run(&mut self) {
        self.gems.clear();
        self.boss = None;
        self.clear_typed();
        self.clear_save();
        self.save_best();
        if let Some(day) = self.daily {
            if self.scores.record_daily(day, self.score) {
                self.save_scores();
            }
        }
        if self.scores.qualifies(self.score) {
            self.phase = Phase::Initials;
            self.initials.clear();
            self.input_lock = MENU_LOCK;
        } else {
            self.enter_game_over();
        }
    }

    fn enter_game_over(&mut self) {
        self.phase = Phase::GameOver;
        self.menu = OVER_AGAIN;
        self.input_lock = MENU_LOCK;
    }

    pub(super) fn initials_type(&mut self, c: char) {
        if c.is_ascii_alphabetic() && self.initials.len() < 3 {
            self.initials.push(c.to_ascii_uppercase());
        }
    }

    /// Enter on the initials screen: file the score and show game over.
    pub fn confirm_initials(&mut self) {
        if self.phase != Phase::Initials || self.initials.is_empty() {
            return;
        }
        self.scores.insert(Entry {
            initials: scores::clean_initials(&self.initials),
            score: self.score,
            wave: self.wave,
            won: self.won,
        });
        self.save_scores();
        self.enter_game_over();
    }

    fn save_scores(&self) {
        if !self.persist {
            return;
        }
        if let Some(path) = scores::path() {
            let _ = fs::write(path, scores::encode(&self.scores));
        }
    }

    // --- victory ---------------------------------------------------------

    /// The final boss fell: celebrate, unlock the next heat, and offer
    /// endless mode or the finish line.
    pub(super) fn enter_victory(&mut self) {
        self.phase = Phase::Victory;
        self.menu = VICTORY_ENDLESS;
        self.won = true;
        self.boss = None;
        self.rocks.clear();
        self.clear_typed();
        self.input_lock = MENU_LOCK * 2.0;
        self.callouts.clear();
        self.callout("YOU WIN!", 0.0, 2.4);
        if self.daily.is_none() {
            let next = (self.heat + 1).min(MAX_HEAT);
            if next > self.settings.heat_unlocked {
                self.settings.heat_unlocked = next;
                self.save_settings();
            }
        }
        self.autosave(true);
    }

    /// Enter on the victory screen.
    pub fn victory_choose(&mut self) {
        if self.phase != Phase::Victory {
            return;
        }
        if self.menu == VICTORY_FINISH {
            self.finish_run();
        } else {
            self.open_shop(true);
        }
    }

    // --- heat ------------------------------------------------------------

    /// Does the current run play with heat rule `n` (1-based)?
    pub(super) fn heat_rule(&self, n: u8) -> bool {
        self.heat >= n
    }

    /// Left/right on START: choose the heat for the next run.
    pub fn heat_move(&mut self, delta: isize) {
        let top = self.settings.heat_unlocked as isize;
        if top == 0 {
            return;
        }
        let next = (self.settings.heat as isize + delta).clamp(0, top);
        self.settings.heat = next as u8;
        self.save_settings();
    }

    // --- options, help and attract mode ----------------------------------

    pub fn open_options(&mut self) {
        self.options_open = true;
        self.options_sel = 0;
    }

    pub fn close_options(&mut self) {
        self.options_open = false;
    }

    pub fn options_move(&mut self, delta: isize) {
        let len = OPTION_ITEMS.len() as isize;
        self.options_sel = (self.options_sel as isize + delta).rem_euclid(len) as usize;
        self.sfx(Sfx::Hover);
    }

    /// Enter (or left/right) on an option: flip it, or close on BACK.
    pub fn options_activate(&mut self) {
        let s = &mut self.settings;
        match self.options_sel {
            0 => s.scanlines = !s.scanlines,
            1 => s.shake = s.shake.next(),
            2 => s.reduced_motion = !s.reduced_motion,
            3 => s.sound = s.sound.next(),
            _ => {
                self.options_open = false;
                return;
            }
        }
        self.save_settings();
    }

    /// The value shown next to option `i`.
    pub fn option_value(&self, i: usize) -> &'static str {
        let on = |b: bool| if b { "ON" } else { "OFF" };
        match i {
            0 => on(self.settings.scanlines),
            1 => self.settings.shake.label(),
            2 => on(self.settings.reduced_motion),
            3 => self.settings.sound.label(),
            _ => "",
        }
    }

    pub fn open_help(&mut self) {
        self.help_open = true;
    }

    pub fn close_help(&mut self) {
        self.help_open = false;
    }

    /// Any key: restart the attract-mode timer.
    pub fn note_input(&mut self) {
        self.last_input = self.time;
    }

    /// The title is idle long enough to show the high-score table.
    pub fn attract_table(&self) -> bool {
        if self.phase != Phase::Title || self.theme_menu_open || self.options_open || self.help_open
        {
            return false;
        }
        let idle = self.time - self.last_input - ATTRACT_AFTER;
        idle >= 0.0 && idle.rem_euclid(ATTRACT_CYCLE) < ATTRACT_SHOW
    }

    // --- files -----------------------------------------------------------

    pub(super) fn save_settings(&self) {
        if !self.persist {
            return;
        }
        if let Some(path) = settings::path() {
            let _ = fs::write(path, settings::encode(&self.settings));
        }
    }

    /// Read settings and scores. The first launch (no settings file yet)
    /// opens the how-to-play card.
    pub(super) fn load_files(&mut self) {
        match settings::path().and_then(|p| fs::read_to_string(p).ok()) {
            Some(text) => self.settings = settings::decode(&text),
            None => {
                self.help_open = true;
                self.save_settings();
            }
        }
        if let Some(text) = scores::path().and_then(|p| fs::read_to_string(p).ok()) {
            self.scores = scores::decode(&text);
        }
    }

    // --- daily run -------------------------------------------------------

    /// `main.rs` passes today's day number (days since 1970-01-01).
    pub fn set_day(&mut self, day: u32) {
        self.day = day;
    }

    /// Everyone gets the same words today: the seed comes from the date,
    /// the theme rotates by day, and heat is off.
    pub fn start_daily(&mut self) {
        let theme = Theme::from_index(self.day as usize % Theme::ALL.len());
        self.theme_choice = theme.index();
        self.apply_theme();
        self.rng = Rng(daily_seed(self.day));
        self.begin_run(0);
        self.daily = Some(self.day);
    }
}

fn daily_seed(day: u32) -> u64 {
    (day as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03 | 1
}

/// One odometer step: close most of the gap quickly, never stall.
fn roll(shown: u32, target: u32, dt: f32) -> u32 {
    if shown == target || dt <= 0.0 {
        return shown;
    }
    let gap = target.abs_diff(shown) as f32;
    let step = (gap * (dt * 9.0).min(1.0)).max(dt * 400.0).ceil() as u32;
    let step = step.min(target.abs_diff(shown));
    if target > shown {
        shown + step
    } else {
        shown - step
    }
}
