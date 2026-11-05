//! Simulation for TYPOIDS. The terminal shell only feeds keys and paints this.

use crate::audio::Sfx;
use crate::boss::{self, Boss};
use crate::hazards::Kind;
use crate::save::{self, RunSave};
use crate::scores::Scores;
use crate::settings::Settings;
use crate::upgrades::{Rarity, Upgrade, Upgrades};
use crate::words;

mod arcade;
mod fun;
mod pointer;
mod rack;

pub use arcade::{Callout, Stats, HEAT_RULES, OPTION_ITEMS, VICTORY_ITEMS};
pub use pointer::Hit;
use std::f32::consts::{PI, TAU};
use std::fs;
use std::path::PathBuf;

const TURN_SHARPNESS: f32 = 9.0;
const SHIP_HIT: f32 = 1.35;

pub const MENU_ITEMS: &[&str] = &[
    "START",
    "DAILY RUN",
    "THEMES",
    "OPTIONS",
    "HOW TO PLAY",
    "QUIT",
];
pub const MENU_ITEMS_SAVED: &[&str] = &[
    "CONTINUE",
    "NEW RUN",
    "DAILY RUN",
    "THEMES",
    "OPTIONS",
    "HOW TO PLAY",
    "QUIT",
];
pub const MENU_START: usize = 0;

/// What the highlighted title entry does. The title list grows a CONTINUE
/// entry when a run is saved, so key handling asks this instead of an index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TitleAction {
    Continue,
    Start,
    Daily,
    Themes,
    Options,
    Help,
    Quit,
}

pub const PAUSE_ITEMS: &[&str] = &["CONTINUE", "RESTART", "SAVE & QUIT"];
pub const PAUSE_CONTINUE: usize = 0;
pub const PAUSE_RESTART: usize = 1;
pub const PAUSE_QUIT: usize = 2;

pub const OVER_ITEMS: &[&str] = &["PLAY AGAIN", "THEMES", "QUIT"];
pub const OVER_AGAIN: usize = 0;
pub const OVER_THEMES: usize = 1;
pub const OVER_QUIT: usize = 2;

/// Offers per shop (a fourth with Wide Shop), plus a reroll button and a
/// "next wave" button after the cards.
pub const SHOP_SLOTS: usize = 3;
const BASE_HEARTS: u32 = 3;
const REROLL_BASE: u32 = 10;
/// Seconds menu keys are ignored after a screen appears, so letters still
/// being typed at the end of a wave do not buy cards or restart the run.
pub const MENU_LOCK: f32 = 0.6;
/// Seconds of "get ready" before the first hazards of a wave arrive.
const READY: f32 = 1.2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Space,
    Underwater,
    Jungle,
    CatCafe,
}

impl Theme {
    pub const ALL: [Theme; 4] = [
        Theme::Space,
        Theme::Underwater,
        Theme::Jungle,
        Theme::CatCafe,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Space => "SPACE",
            Theme::Underwater => "UNDERWATER",
            Theme::Jungle => "JUNGLE",
            Theme::CatCafe => "CAT CAFE",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Theme::Space => 0,
            Theme::Underwater => 1,
            Theme::Jungle => 2,
            Theme::CatCafe => 3,
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Title,
    Playing,
    Paused,
    Shop,
    Dying,
    /// Typing three initials for the high-score table.
    Initials,
    /// The final boss is beaten: go endless or finish the run.
    Victory,
    GameOver,
}

#[derive(Clone, Debug)]
pub enum Effect {
    Spark {
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        life: f32,
        max: f32,
        glyph: char,
    },
    Shard {
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        rot: f32,
        spin: f32,
        len: f32,
        life: f32,
        max: f32,
    },
    Laser {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        life: f32,
        max: f32,
    },
    Ring {
        x: f32,
        y: f32,
        r: f32,
        vr: f32,
        life: f32,
        max: f32,
    },
    Float {
        x: f32,
        y: f32,
        life: f32,
        max: f32,
        text: String,
    },
    /// A falling droplet of spilled liquid. It stops at `floor` and soaks in.
    Drop {
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        floor: f32,
        life: f32,
        max: f32,
    },
    /// A puddle that spreads out to `reach` and lingers before fading.
    Puddle {
        x: f32,
        y: f32,
        w: f32,
        reach: f32,
        life: f32,
        max: f32,
    },
}

impl Effect {
    pub fn life_ratio(&self) -> f32 {
        let (life, max) = match self {
            Effect::Spark { life, max, .. }
            | Effect::Shard { life, max, .. }
            | Effect::Laser { life, max, .. }
            | Effect::Ring { life, max, .. }
            | Effect::Float { life, max, .. }
            | Effect::Drop { life, max, .. }
            | Effect::Puddle { life, max, .. } => (*life, *max),
        };
        if max <= 0.0 {
            0.0
        } else {
            (life / max).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Rock {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub radius: f32,
    pub rot: f32,
    pub spin: f32,
    pub shape: Vec<(f32, f32)>,
    pub word: String,
    pub live: bool,
    pub hit: f32,
    pub kind: Kind,
    /// Completions left before an armored hazard breaks.
    pub armor: u8,
}

/// A crystal dropped by a destroyed hazard. It drifts out, then flies to the
/// ship and is collected on arrival.
#[derive(Clone, Debug)]
pub struct Gem {
    pub x: f32,
    pub y: f32,
    vx: f32,
    vy: f32,
    age: f32,
}

impl Gem {
    #[cfg(test)]
    pub fn resting(x: f32, y: f32) -> Self {
        Gem {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            age: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Offer {
    pub upgrade: Upgrade,
    /// The level this card grants, which names and prices it.
    pub tier: u8,
    pub sold: bool,
}

impl Offer {
    pub fn new(upgrade: Upgrade, tier: u8) -> Self {
        Offer {
            upgrade,
            tier,
            sold: false,
        }
    }

    pub fn name(&self) -> &'static str {
        self.upgrade.tier(self.tier).name
    }

    pub fn rarity(&self) -> Rarity {
        self.upgrade.tier(self.tier).rarity
    }
}

#[derive(Clone, Debug)]
pub struct Shop {
    pub offers: Vec<Offer>,
    /// Rerolls spent in this shop; each makes the next one dearer.
    pub rerolls: u32,
}

#[derive(Clone, Debug)]
pub struct Star {
    pub x: f32,
    pub y: f32,
    pub glyph: char,
    pub phase: f32,
}

pub struct Game {
    pub phase: Phase,
    pub quit: bool,
    pub menu: usize,
    pub theme: Theme,
    pub theme_choice: usize,
    pub theme_menu_open: bool,
    pub time: f32,
    pub score: u32,
    pub best: u32,
    pub wave: u32,
    pub lives: u32,
    pub crystals: u32,
    pub upgrades: Upgrades,
    pub gems: Vec<Gem>,
    pub gem_flash: f32,
    pub shield: u32,
    /// The boss encounter on a boss wave; `None` on normal waves.
    pub boss: Option<Boss>,
    /// Seconds left of Brake Pads at the start of a wave.
    pub brake: f32,
    /// Clean Slate has already forgiven a slip this wave.
    slate_used: bool,
    pub shop: Option<Shop>,
    pub shop_sel: usize,
    pub shop_note: String,
    pub combo: u32,
    pub buffer: String,
    /// `time` at which each letter of `buffer` was typed (drives the stage animation).
    pub buffer_t: Vec<f32>,
    /// `time` at which the current target was locked.
    pub lock_t: f32,
    /// The word just finished, kept briefly so the stage can shatter it.
    pub spent: Option<Spent>,
    pub target: Option<u32>,
    pub rocks: Vec<Rock>,
    pub effects: Vec<Effect>,
    pub stars: Vec<Star>,
    pub angle: f32,
    pub spin: f32,
    pub thrust: f32,
    pub invuln: f32,
    pub ox: f32,
    pub oy: f32,
    kick_vx: f32,
    kick_vy: f32,
    pub shake: f32,
    pub shake_x: f32,
    pub shake_y: f32,
    pub miss: f32,
    pub hurt: f32,
    pub banner: String,
    pub banner_t: f32,
    pub play_w: f32,
    pub play_h: f32,
    pub term_w: u16,
    pub term_h: u16,
    rng: Rng,
    next_id: u32,
    budget: u32,
    spawn_timer: f32,
    combo_left: f32,
    dying: f32,
    pending_clear: bool,
    clear_t: f32,
    exhaust: f32,
    best_dirty: bool,
    kills: u32,
    revive_used: bool,
    /// The WPM a boss demanded when it got away and ended the run.
    pub boss_escaped: Option<u32>,
    /// A wrong key was pressed since the last kill (breaks Steady Hands).
    fumbled: bool,
    /// Only the real binary writes save files; tests leave this off.
    persist: bool,
    pub has_save: bool,

    /// Seconds before menu keys count again (see `MENU_LOCK`).
    pub input_lock: f32,
    /// What the pause menu returns to: `Playing` or `Shop`.
    pub paused_from: Phase,
    /// The last key that matched nothing, for the miss message.
    pub last_wrong: Option<char>,
    pub wave_stats: Stats,
    pub run_stats: Stats,
    last_key_t: f32,
    /// `time` the shop opened (cards rise in from it).
    pub shop_t0: f32,
    /// `time` each shop slot was bought, for the purchase flash.
    pub bought_t: [f32; 4],
    /// Score and crystals as the odometer shows them, rolling toward the real values.
    pub shown_score: u32,
    pub shown_crystals: u32,
    /// `time` of the last key press (drives the title's attract mode).
    pub last_input: f32,
    pub settings: Settings,
    pub options_open: bool,
    pub options_sel: usize,
    pub help_open: bool,
    /// Big pixel call-outs ("WAVE 3", "GO!", "K.O.!"), possibly queued ahead.
    pub callouts: Vec<Callout>,
    /// Sound effects to play, with a delay in seconds. `main.rs` drains it
    /// every frame; nothing in the simulation reads it.
    pub sounds: Vec<(Sfx, f32)>,
    /// The combo announcer ("NICE!", "RADICAL!").
    pub announce: Option<Callout>,
    pub scores: Scores,
    pub initials: String,
    /// Brief bright flash of the stage floor after a long word.
    pub floor_flash: f32,
    /// Seconds the world is frozen after a kill, for weight.
    pub hitstop: f32,
    hurt_this_wave: bool,
    /// Rack slots earned by beating bosses.
    pub rack_bonus: u32,
    /// The shop's selection is on the installed rack (to sell), not the shelf.
    pub rack_focus: bool,
    pub rack_sel: usize,
    typed_kills: u32,
    /// Heat of the current run (0 = normal).
    pub heat: u8,
    /// The final boss has fallen this run.
    pub won: bool,
    finale: bool,
    ready: f32,
    /// Overdrive charge, 0 to `DRIVE_FULL`.
    pub drive: f32,
    /// Seconds of Overdrive left.
    pub overdrive: f32,
    boss_hold: f32,
    /// Seconds every hazard stays frozen (the Freeze power-up).
    pub freeze: f32,
    magnet: bool,
    nuking: bool,
    pub last_pickup: Option<crate::hazards::Pickup>,
    /// Consecutive linked words (last letter = next first letter).
    pub link: u32,
    last_final: Option<char>,
    last_kill: Option<(f32, f32)>,
    /// Today's day number (days since 1970), supplied by `main.rs`.
    pub day: u32,
    /// The day of the daily run being played, if this is one.
    pub daily: Option<u32>,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.0
    }

    fn f32(&mut self) -> f32 {
        ((self.next() >> 40) as u32 as f32) / ((1u32 << 24) as f32)
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }

    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[(self.next() as usize) % items.len()]
    }
}

pub fn forward(angle: f32) -> (f32, f32) {
    (angle.sin(), -angle.cos())
}

pub fn rotate(lx: f32, ly: f32, angle: f32) -> (f32, f32) {
    let (s, c) = angle.sin_cos();
    (lx * c - ly * s, lx * s + ly * c)
}

pub fn angle_delta(from: f32, to: f32) -> f32 {
    let mut d = (to - from) % TAU;
    if d > PI {
        d -= TAU;
    }
    if d < -PI {
        d += TAU;
    }
    d
}

/// A finished word on its way off the stage.
#[derive(Clone)]
pub struct Spent {
    pub word: String,
    pub t0: f32,
}

#[derive(Clone, Copy)]
pub struct Metrics {
    pub play_w: u16,
    pub play_h: u16,
    pub origin_y: u16,
    /// First row of the typing stage; its last row is the floor line.
    pub stage_y: u16,
    /// Rows in the stage, floor line included. Zero on very short terminals.
    pub stage_h: u16,
    pub dock_y: u16,
}

/// Rows the big-letter stage takes: none when the terminal is too short,
/// otherwise five (a floor line under four rows of half-block letters).
fn stage_height(term_h: u16) -> u16 {
    if term_h < 22 {
        0
    } else {
        5
    }
}

/// Downward pull on spilled droplets, in cells per second squared.
const DROP_GRAVITY: f32 = 26.0;

pub fn metrics(term_w: u16, term_h: u16) -> Metrics {
    let dock_h = 2;
    let origin_y = 1;
    let stage_h = stage_height(term_h);
    let play_h = term_h.saturating_sub(origin_y + stage_h + dock_h);
    Metrics {
        play_w: term_w,
        play_h,
        origin_y,
        stage_y: origin_y + play_h,
        stage_h,
        dock_y: origin_y + play_h + stage_h,
    }
}

impl Game {
    pub fn new(term_w: u16, term_h: u16, seed: u64) -> Self {
        let mut game = Self {
            phase: Phase::Title,
            quit: false,
            menu: 0,
            theme: Theme::Underwater,
            theme_choice: Theme::Underwater.index(),
            theme_menu_open: false,
            time: 0.0,
            score: 0,
            best: load_best(),
            wave: 1,
            lives: BASE_HEARTS,
            crystals: 0,
            upgrades: Upgrades::default(),
            gems: Vec::new(),
            gem_flash: 0.0,
            shield: 0,
            boss: None,
            brake: 0.0,
            slate_used: false,
            shop: None,
            shop_sel: 0,
            shop_note: String::new(),
            combo: 0,
            buffer: String::new(),
            buffer_t: Vec::new(),
            lock_t: 0.0,
            spent: None,
            target: None,
            rocks: Vec::new(),
            effects: Vec::new(),
            stars: Vec::new(),
            angle: 0.0,
            spin: 0.0,
            thrust: 0.0,
            ox: 0.0,
            oy: 0.0,
            kick_vx: 0.0,
            kick_vy: 0.0,
            shake: 0.0,
            shake_x: 0.0,
            shake_y: 0.0,
            miss: 0.0,
            hurt: 0.0,
            banner: String::new(),
            banner_t: 0.0,
            play_w: 80.0,
            play_h: 24.0,
            term_w: 80,
            term_h: 24,
            rng: Rng(seed | 1),
            next_id: 1,
            budget: 0,
            spawn_timer: 0.0,
            combo_left: 0.0,
            invuln: 0.0,
            dying: 0.0,
            pending_clear: false,
            clear_t: 0.0,
            exhaust: 0.0,
            best_dirty: false,
            kills: 0,
            revive_used: false,
            boss_escaped: None,
            fumbled: false,
            persist: false,
            has_save: false,
            input_lock: 0.0,
            paused_from: Phase::Playing,
            last_wrong: None,
            wave_stats: Stats::default(),
            run_stats: Stats::default(),
            last_key_t: -10.0,
            shop_t0: 0.0,
            bought_t: [-10.0; 4],
            shown_score: 0,
            shown_crystals: 0,
            last_input: 0.0,
            settings: Settings::default(),
            options_open: false,
            options_sel: 0,
            help_open: false,
            callouts: Vec::new(),
            sounds: Vec::new(),
            announce: None,
            scores: Scores::default(),
            initials: String::new(),
            floor_flash: 0.0,
            hitstop: 0.0,
            hurt_this_wave: false,
            rack_bonus: 0,
            rack_focus: false,
            rack_sel: 0,
            typed_kills: 0,
            heat: 0,
            won: false,
            finale: false,
            ready: 0.0,
            drive: 0.0,
            overdrive: 0.0,
            boss_hold: 0.0,
            freeze: 0.0,
            magnet: false,
            nuking: false,
            last_pickup: None,
            link: 0,
            last_final: None,
            last_kill: None,
            day: 0,
            daily: None,
        };
        game.set_screen(term_w, term_h);
        game.seed_title();
        game
    }

    pub fn set_screen(&mut self, term_w: u16, term_h: u16) {
        let term_w = term_w.max(1);
        let term_h = term_h.max(1);
        if term_w == self.term_w && term_h == self.term_h && !self.stars.is_empty() {
            return;
        }
        let m = metrics(term_w, term_h);
        self.term_w = term_w.max(1);
        self.term_h = term_h.max(1);
        self.play_w = m.play_w.max(1) as f32;
        self.play_h = m.play_h.max(1) as f32;
        self.reseed_stars();
    }

    pub fn ship_center(&self) -> (f32, f32) {
        // Against a boss the ship drops to the bottom, under the passage.
        let y = if self.boss.is_some() { 0.86 } else { 0.5 };
        (self.play_w * 0.5, self.play_h * y)
    }

    /// Where the boss hangs, top and centre of the play area.
    /// Where the boss is now: its home spot plus its idle drift.
    pub fn boss_pos(&self) -> (f32, f32) {
        let (x, y) = self.boss_home();
        let age = self.boss.as_ref().map_or(0.0, |b| b.age);
        let (dx, dy) = boss::drift(self.theme, age, self.play_w, self.play_h);
        (x + dx, y + dy)
    }

    /// The boss's resting spot; the passage panel is laid out from this.
    pub fn boss_home(&self) -> (f32, f32) {
        (self.play_w * 0.5, self.play_h * 0.3)
    }

    pub fn ship_pos(&self) -> (f32, f32) {
        let (cx, cy) = self.ship_center();
        let bob = if self.thrust > 0.2 {
            0.0
        } else {
            (self.time * 2.2).sin() * 0.12
        };
        (cx + self.ox, cy + self.oy + bob)
    }

    pub fn matches(&self) -> usize {
        if self.buffer.is_empty() {
            return 0;
        }
        self.rocks
            .iter()
            .filter(|r| r.live && r.word.starts_with(&self.buffer))
            .count()
    }

    pub fn target_word(&self) -> Option<&str> {
        if let Some(boss) = self.boss.as_ref() {
            return boss.word().filter(|_| boss.fighting());
        }
        let id = self.target?;
        self.rocks
            .iter()
            .find(|r| r.id == id && r.live)
            .map(|r| r.word.as_str())
    }

    pub fn menu_move(&mut self, delta: isize) {
        let len = self.menu_items().len() as isize;
        self.menu = (self.menu as isize + delta).rem_euclid(len) as usize;
        self.sfx(Sfx::Hover);
    }

    /// The choices shown for the current phase. The title and the two popups
    /// all share the same `menu` index, but each has its own wording.
    pub fn menu_items(&self) -> &'static [&'static str] {
        match self.phase {
            Phase::Paused => PAUSE_ITEMS,
            Phase::GameOver => OVER_ITEMS,
            Phase::Victory => VICTORY_ITEMS,
            _ if self.has_save => MENU_ITEMS_SAVED,
            _ => MENU_ITEMS,
        }
    }

    pub fn title_action(&self) -> TitleAction {
        let offset = self.has_save as usize;
        match self.menu.checked_sub(offset) {
            None => TitleAction::Continue,
            Some(0) => TitleAction::Start,
            Some(1) => TitleAction::Daily,
            Some(2) => TitleAction::Themes,
            Some(3) => TitleAction::Options,
            Some(4) => TitleAction::Help,
            Some(_) => TitleAction::Quit,
        }
    }

    pub fn max_lives(&self) -> u32 {
        BASE_HEARTS + self.upgrades.level(Upgrade::ExtraHeart) as u32
    }

    /// Queue a sound effect.
    pub fn sfx(&mut self, sfx: Sfx) {
        self.sfx_after(sfx, 0.0);
    }

    pub fn sfx_after(&mut self, sfx: Sfx, delay: f32) {
        // A burst (Multishot, Nuke) needs only a few of the same sound.
        if self.sounds.iter().filter(|(s, _)| *s == sfx).count() < 3 {
            self.sounds.push((sfx, delay));
        }
    }

    /// Open the pause menu from play or from the shop.
    pub fn pause(&mut self) {
        if matches!(self.phase, Phase::Playing | Phase::Shop) {
            self.sfx(Sfx::Pause);
            self.paused_from = self.phase;
            self.phase = Phase::Paused;
            self.menu = PAUSE_CONTINUE;
        }
    }

    pub fn resume(&mut self) {
        if self.phase == Phase::Paused {
            self.phase = self.paused_from;
        }
    }

    /// The shop is on screen, possibly under the pause menu.
    pub fn in_shop(&self) -> bool {
        self.phase == Phase::Shop
            || (self.phase == Phase::Paused && self.paused_from == Phase::Shop)
    }

    /// Menu keys count: the screen has been up long enough.
    pub fn accepts_menu_keys(&self) -> bool {
        self.input_lock <= 0.0
    }

    pub fn open_themes(&mut self) {
        self.theme_menu_open = true;
        self.theme_choice = self.theme.index();
    }

    pub fn close_themes(&mut self) {
        self.theme_menu_open = false;
        if self.phase == Phase::GameOver {
            self.menu = OVER_AGAIN;
        }
    }

    pub fn theme_move(&mut self, delta: isize) {
        let len = Theme::ALL.len() as isize;
        self.theme_choice = (self.theme_choice as isize + delta).rem_euclid(len) as usize;
        self.sfx(Sfx::Hover);
    }

    pub fn apply_theme(&mut self) {
        self.theme = Theme::from_index(self.theme_choice);
        self.theme_menu_open = false;
        if self.phase == Phase::GameOver {
            self.menu = OVER_AGAIN;
        }
        self.reseed_stars();
    }

    /// Start a new run at the heat chosen on the title screen.
    pub fn start(&mut self) {
        let heat = self.settings.heat.min(self.settings.heat_unlocked);
        self.begin_run(heat);
    }

    fn begin_run(&mut self, heat: u8) {
        self.phase = Phase::Playing;
        self.theme_menu_open = false;
        self.reset_run();
        self.heat = heat;
        if self.heat_rule(5) {
            self.lives = 2;
        }
        self.clear_save();
        self.begin_wave();
    }

    /// Abandon the current run and go back to the title screen.
    pub fn return_to_title(&mut self) {
        self.phase = Phase::Title;
        self.menu = MENU_START;
        self.theme_menu_open = false;
        self.reset_run();
        self.seed_title();
    }

    fn reset_run(&mut self) {
        self.rocks.clear();
        self.effects.clear();
        self.clear_typed();
        self.spent = None;
        self.score = 0;
        self.lives = BASE_HEARTS;
        self.crystals = 0;
        self.upgrades = Upgrades::default();
        self.gems.clear();
        self.gem_flash = 0.0;
        self.shield = 0;
        self.boss = None;
        self.shop = None;
        self.shop_sel = 0;
        self.shop_note.clear();
        self.kills = 0;
        self.revive_used = false;
        self.boss_escaped = None;
        self.fumbled = false;
        self.combo = 0;
        self.combo_left = 0.0;
        self.wave = 1;
        self.ox = 0.0;
        self.oy = 0.0;
        self.thrust = 0.0;
        self.spin = 0.0;
        self.angle = 0.0;
        self.invuln = 0.0;
        self.dying = 0.0;
        self.pending_clear = false;
        self.miss = 0.0;
        self.hurt = 0.0;
        self.shake = 0.0;
        self.shake_x = 0.0;
        self.shake_y = 0.0;
        self.banner.clear();
        self.banner_t = 0.0;
        self.wave_stats = Stats::default();
        self.run_stats = Stats::default();
        self.last_wrong = None;
        self.callouts.clear();
        self.announce = None;
        self.initials.clear();
        self.floor_flash = 0.0;
        self.hitstop = 0.0;
        self.hurt_this_wave = false;
        self.rack_bonus = 0;
        self.rack_focus = false;
        self.rack_sel = 0;
        self.typed_kills = 0;
        self.heat = 0;
        self.won = false;
        self.finale = false;
        self.ready = 0.0;
        self.drive = 0.0;
        self.overdrive = 0.0;
        self.boss_hold = 0.0;
        self.freeze = 0.0;
        self.magnet = false;
        self.last_pickup = None;
        self.link = 0;
        self.last_final = None;
        self.last_kill = None;
        self.daily = None;
        self.input_lock = 0.0;
        self.paused_from = Phase::Playing;
        self.snap_counters();
    }

    /// Forget what the player has typed and the hazard it locked.
    fn clear_typed(&mut self) {
        self.buffer.clear();
        self.buffer_t.clear();
        self.target = None;
    }

    pub fn type_char(&mut self, c: char) {
        if self.phase == Phase::Initials {
            self.initials_type(c);
            return;
        }
        if self.phase != Phase::Playing || self.pending_clear || self.ready > 0.0 {
            return;
        }
        let c = c.to_ascii_lowercase();
        if !c.is_ascii_lowercase() {
            return;
        }
        if self.boss.is_some() {
            self.boss_type(c);
            return;
        }
        let mut next = self.buffer.clone();
        next.push(c);
        let any = self
            .rocks
            .iter()
            .any(|r| r.live && r.word.starts_with(&next));
        if !any && self.upgrades.has(Upgrade::CleanSlate) && !self.slate_used {
            self.slate_used = true;
            self.banner = "SLIP FORGIVEN".to_string();
            self.banner_t = 0.7;
            return;
        }
        if !any {
            self.slip(c);
            return;
        }
        self.note_key(true);
        self.charge_drive(true);
        self.buffer = next;
        self.buffer_t.push(self.time);
        self.sfx(Sfx::Fire(self.buffer.len() as u8 - 1));
        self.lock_target();
        let Some(id) = self.target else {
            return;
        };
        let complete = self
            .rocks
            .iter()
            .any(|r| r.id == id && r.live && r.word == self.buffer);
        self.fire_at(id, complete);
    }

    /// A key that matched nothing: flash the miss and remember the key.
    fn slip(&mut self, c: char) {
        self.sfx(Sfx::Typo);
        self.miss = 0.32;
        self.fumbled = true;
        self.last_wrong = Some(c);
        self.shake = self.shake.max(0.12);
        self.note_key(false);
        self.charge_drive(false);
        if self.heat_rule(3) {
            self.combo = 0;
        }
    }

    /// The letter the locked word wants next, if a word is locked.
    pub fn expected_letter(&self) -> Option<char> {
        let word = self.target_word()?;
        word.chars().nth(self.buffer.chars().count())
    }

    /// Ctrl+W: drop the whole typed word.
    pub fn clear_word(&mut self) {
        if self.phase == Phase::Playing {
            self.clear_typed();
            self.sync_boss();
        }
    }

    pub fn backspace(&mut self) {
        if self.phase == Phase::Initials {
            self.initials.pop();
            return;
        }
        if self.phase != Phase::Playing || self.buffer.is_empty() {
            return;
        }
        self.buffer.pop();
        self.buffer_t.pop();
        if self.boss_word().is_some() {
            self.sync_boss();
            return;
        }
        if self.buffer.is_empty() {
            self.target = None;
        } else {
            self.lock_target();
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        // Hit-stop freezes the world for a beat; effects keep fading so
        // the screen never looks hung.
        let world = if self.hitstop > 0.0 {
            self.hitstop = (self.hitstop - dt).max(0.0);
            0.0
        } else {
            dt
        };
        self.decay_fx(world);
        self.input_lock = (self.input_lock - dt).max(0.0);
        self.floor_flash = (self.floor_flash - dt).max(0.0);
        self.tick_counters(dt);
        self.prune_callouts();
        self.update_effects(dt);
        self.spring_ship(dt);
        self.spawn_exhaust(dt);

        match self.phase {
            Phase::Title => self.update_title(dt),
            Phase::Paused | Phase::GameOver | Phase::Shop | Phase::Initials | Phase::Victory => {}
            Phase::Dying => {
                self.integrate_rocks(world, false);
                self.dying -= world;
                if self.dying <= 0.0 {
                    self.finish_run();
                }
            }
            Phase::Playing => self.update_play(world),
        }
    }

    fn seed_title(&mut self) {
        self.rocks.clear();
        for _ in 0..5 {
            self.spawn_drifter();
        }
    }

    fn update_title(&mut self, dt: f32) {
        if self.rocks.len() < 5 {
            self.spawn_drifter();
        }
        for rock in &mut self.rocks {
            rock.x += rock.vx * dt;
            rock.y += rock.vy * dt;
            rock.rot += rock.spin * dt;
            if rock.x < -6.0 {
                rock.x = self.play_w + 4.0;
            } else if rock.x > self.play_w + 6.0 {
                rock.x = -4.0;
            }
            if rock.y < -4.0 {
                rock.y = self.play_h + 3.0;
            } else if rock.y > self.play_h + 4.0 {
                rock.y = -3.0;
            }
        }
        self.aim_ship(dt, true);
    }

    fn update_play(&mut self, dt: f32) {
        self.update_gems(dt, self.pending_clear);
        if self.pending_clear {
            self.clear_t -= dt;
            self.integrate_rocks(dt, false);
            if self.clear_t <= 0.0 {
                self.pending_clear = false;
                self.crystals += self.gems.len() as u32;
                self.gems.clear();
                self.wave += 1;
                if std::mem::take(&mut self.finale) {
                    self.enter_victory();
                } else {
                    self.open_shop(true);
                }
            }
            return;
        }
        self.wave_stats.secs += dt;
        self.run_stats.secs += dt;

        if self.boss.is_some() {
            self.update_boss(dt);
            if !self.rocks.is_empty() {
                self.integrate_rocks(dt, true);
            }
            self.aim_ship(dt, false);
            return;
        }

        // "WAVE 3 ... GO!": nothing arrives until the call-out has played.
        if self.ready > 0.0 {
            self.ready -= dt;
            if self.ready <= 0.0 {
                let immediate = 3.min(self.budget);
                for _ in 0..immediate {
                    self.spawn_threat();
                    self.budget -= 1;
                }
            }
            self.aim_ship(dt, false);
            return;
        }

        self.spawn_timer -= dt;
        let cap = (3 + self.wave / 2).min(if self.wave > 12 { 9 } else { 7 }) as usize;
        if self.budget > 0 && self.rocks.len() < cap && self.spawn_timer <= 0.0 {
            self.spawn_threat();
            self.budget -= 1;
            let gap = spawn_gap(self.wave);
            self.spawn_timer = gap;
        }

        self.integrate_rocks(dt, true);
        self.aim_ship(dt, false);

        if self.budget == 0 && self.rocks.iter().all(|r| !r.live) {
            self.pending_clear = true;
            self.clear_t = 1.45;
            self.banner = clear_banner(self.theme).to_string();
            if !self.hurt_this_wave {
                // Skill pays: a wave without losing a heart earns a bonus.
                let bonus = self.flawless_bonus();
                self.crystals += bonus;
                self.banner = format!("{}  ·  FLAWLESS +{bonus} ◆", self.banner);
            }
            self.banner_t = 1.45;
            self.callout("CLEAR!", 0.0, 1.3);
            self.sfx_after(Sfx::WaveClear, 0.15);
            self.clear_typed();
        }
    }

    /// The wave's hazards are all gone and the shop is on its way.
    pub fn wave_cleared(&self) -> bool {
        self.pending_clear && self.boss.is_none() && self.phase == Phase::Playing
    }

    fn begin_wave(&mut self) {
        self.shield = self.upgrades.level(Upgrade::Ward) as u32;
        let pads = self.upgrades.level(Upgrade::BrakePads);
        self.brake = if pads > 0 {
            Upgrade::brake_secs(pads)
        } else {
            0.0
        };
        self.slate_used = false;
        self.hurt_this_wave = false;
        self.magnet = false;
        self.freeze = 0.0;
        self.wave_stats = Stats::default();
        if boss::is_boss_wave(self.wave) {
            self.start_boss();
            return;
        }
        self.budget = 5 + self.wave * 2;
        self.spawn_timer = 0.35;
        self.pending_clear = false;
        self.banner.clear();
        self.banner_t = 0.0;
        self.ready = READY;
        self.callout(format!("WAVE {}", self.wave), 0.0, READY - 0.35);
        self.callout("GO!", READY - 0.35, 0.6);
        self.sfx_after(Sfx::WaveStart, READY - 0.35);
    }

    fn integrate_rocks(&mut self, dt: f32, collide: bool) {
        let (sx, sy) = self.ship_pos();
        let mut hits = Vec::new();
        let pace = self.hazard_pace();
        // Focus slows only the hazard being typed.
        let focus = Upgrade::focus_pace(self.upgrades.level(Upgrade::Focus));
        let typed = if collide { self.target } else { None };
        for rock in &mut self.rocks {
            if !rock.live {
                continue;
            }
            let pace = if Some(rock.id) == typed {
                pace * focus
            } else {
                pace
            };
            rock.x += rock.vx * dt * pace;
            rock.y += rock.vy * dt * pace;
            rock.rot += rock.spin * dt;
            rock.hit = (rock.hit - dt).max(0.0);
            if !collide || self.phase != Phase::Playing {
                continue;
            }
            let dx = rock.x - sx;
            let dy = rock.y - sy;
            let reach = rock.radius * 0.72 + SHIP_HIT;
            if dx * dx + dy * dy < reach * reach {
                hits.push(rock.id);
            }
        }
        for id in hits {
            self.collide(id);
        }
        self.rocks.retain(|r| {
            r.live
                && r.x > -12.0
                && r.y > -8.0
                && r.x < self.play_w + 12.0
                && r.y < self.play_h + 8.0
        });
        if self.target.is_some()
            && !self
                .rocks
                .iter()
                .any(|r| Some(r.id) == self.target && r.live)
        {
            self.target = None;
            if !self.buffer.is_empty() {
                self.lock_target();
            }
        }
    }

    fn aim_ship(&mut self, dt: f32, attract: bool) {
        let at_boss = self.boss.as_ref().is_some_and(|b| b.fighting());
        let desired = if !attract && at_boss {
            Some(self.boss_pos())
        } else if !attract && self.buffer.is_empty() {
            None
        } else if let Some(id) = self.target {
            self.rocks.iter().find(|r| r.id == id).map(|r| (r.x, r.y))
        } else if attract {
            self.nearest_rock().map(|r| (r.x, r.y))
        } else {
            None
        };

        if let Some((tx, ty)) = desired {
            let (sx, sy) = self.ship_pos();
            let want = (tx - sx).atan2(-(ty - sy));
            let d = angle_delta(self.angle, want);
            let k = 1.0 - (-TURN_SHARPNESS * dt).exp();
            self.spin = if dt > 0.0 { d * k / dt } else { 0.0 };
            self.angle += d * k;
            if attract && self.spin.abs() > 0.6 {
                self.thrust = self.thrust.max(0.35);
            }
        } else {
            self.spin *= (1.0 - dt * 4.0).max(0.0);
        }
    }

    fn nearest_rock(&self) -> Option<&Rock> {
        let (sx, sy) = self.ship_center();
        self.rocks.iter().filter(|r| r.live).min_by(|a, b| {
            let da = (a.x - sx).powi(2) + (a.y - sy).powi(2);
            let db = (b.x - sx).powi(2) + (b.y - sy).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn lock_target(&mut self) {
        if self.buffer.is_empty() {
            self.target = None;
            return;
        }
        if let Some(id) = self.target {
            if self
                .rocks
                .iter()
                .any(|r| r.id == id && r.live && r.word.starts_with(&self.buffer))
            {
                return;
            }
        }
        let (sx, sy) = self.ship_center();
        let before = self.target;
        self.target = self
            .rocks
            .iter()
            .filter(|r| r.live && r.word.starts_with(&self.buffer))
            .min_by(|a, b| {
                let da = (a.x - sx).powi(2) + (a.y - sy).powi(2);
                let db = (b.x - sx).powi(2) + (b.y - sy).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.id);
        if self.target != before {
            self.lock_t = self.time;
        }
    }

    fn fire_at(&mut self, id: u32, kill: bool) {
        let Some(rock) = self.rocks.iter().find(|r| r.id == id && r.live) else {
            return;
        };
        let tx = rock.x;
        let ty = rock.y;
        let word = rock.word.clone();
        let radius = rock.radius;
        let (sx, sy) = self.ship_pos();
        let (fx, fy) = forward(self.angle);
        let nose = 1.9;
        self.effects.push(Effect::Laser {
            x0: sx + fx * nose,
            y0: sy + fy * nose,
            x1: tx,
            y1: ty,
            life: 0.26,
            max: 0.26,
        });
        let (dx, dy) = normalize(tx - sx, ty - sy);
        self.ox += dx * 0.15;
        self.oy += dy * 0.15;
        // Recoil opposite the torpedo, classic submersible kick.
        self.thrust = 1.0;
        let recoil = if kill { 18.0 } else { 11.0 };
        self.kick(-dx, -dy, recoil);
        self.effects.push(Effect::Spark {
            x: tx,
            y: ty,
            vx: self.rng.range(-3.0, 3.0),
            vy: self.rng.range(-3.0, 3.0),
            life: 0.18,
            max: 0.18,
            glyph: match (self.theme, kill) {
                (Theme::Space, true) => '*',
                (Theme::Space, false) => '+',
                (Theme::Underwater, true) => '°',
                (Theme::Underwater, false) => '·',
                (Theme::Jungle, true) => '✦',
                (Theme::Jungle, false) => '·',
                (Theme::CatCafe, true) => '✧',
                (Theme::CatCafe, false) => '·',
            },
        });
        if let Some(rock) = self.rocks.iter_mut().find(|r| r.id == id) {
            rock.hit = 0.14;
        }
        if kill && self.crack(id) {
            return;
        }
        if kill {
            self.typed_kills += 1;
            self.update_link(&word, tx, ty);
            self.detonate(id, &word, radius, tx, ty, false);
            self.chain_kills(tx, ty);
            if self.upgrades.has(Upgrade::Sweeper)
                && self.typed_kills / SWEEP_EVERY * SWEEP_EVERY == self.typed_kills
            {
                self.sweep(tx, ty);
            }
        }
    }

    fn kick(&mut self, dx: f32, dy: f32, power: f32) {
        self.ox = self.ox.clamp(-2.4, 2.4);
        self.oy = self.oy.clamp(-1.8, 1.8);
        // Velocity lives in ox/oy via the spring: impulse the offset's derivative
        // by storing it in shake-independent fields. We keep derivatives in
        // thrust kick using ox as position and a side channel.
        self.kick_vx += dx * power;
        self.kick_vy += dy * power;
    }

    /// Destroy a hazard and pay for it. `chained` kills (Multishot,
    /// Scattershot, Sweeper, Nuke) score normally but drop a single crystal
    /// and leave the typing streak (Steady Hands) alone.
    fn detonate(&mut self, id: u32, word: &str, radius: f32, x: f32, y: f32, chained: bool) {
        let before = self.combo;
        let step = if self.overdrive > 0.0 { 2 } else { 1 };
        self.combo = self.combo.saturating_add(step);
        self.combo_left = self.combo_window();
        self.run_stats.best_combo = self.run_stats.best_combo.max(self.combo);
        self.wave_stats.best_combo = self.wave_stats.best_combo.max(self.combo);
        self.announce_combo(before);
        let big = self.upgrades.has(Upgrade::BigGame) && word.len() >= 7;
        let last_stand = self.lives == 1 && self.upgrades.has(Upgrade::LastStand);
        let bonus = 1 + big as u32 + last_stand as u32;
        let clean = !chained && !self.fumbled;
        if !chained {
            self.fumbled = false;
        }
        self.sfx(Sfx::Break {
            big: word.len() >= 7,
        });
        if clean {
            self.sfx_after(Sfx::Perfect, 0.05);
        }
        let steady = if clean {
            self.upgrades.level(Upgrade::SteadyHands) as u32
        } else {
            0
        };
        let link = if chained { 0 } else { self.link };
        let pts = word.len() as u32 * 50 * self.combo * bonus * (100 + 25 * steady) / 100
            * fun::link_bonus(link)
            / 100;
        self.score += pts;
        if self.score > self.best {
            self.best = self.score;
            self.best_dirty = true;
        }
        self.shake = self.shake.max(0.28 + (word.len() as f32 * 0.02));
        self.effects.push(Effect::Float {
            x,
            y: y - radius,
            life: 0.85,
            max: 0.85,
            text: match (link, self.combo) {
                (1.., _) => format!("+{pts} LINK x{}", link + 1),
                (_, 2..) => format!("+{pts} x{}", self.combo),
                _ => format!("+{pts}"),
            },
        });
        let crystals = if chained {
            1
        } else {
            let prospect = Upgrade::prospector_chance(self.upgrades.level(Upgrade::Prospector));
            let found = prospect > 0.0 && self.rng.f32() < prospect;
            let base = 1
                + (word.len() >= 6) as u32
                + found as u32
                + (steady > 0) as u32
                + if big { 2 } else { 0 }
                + (self.combo >= 5 && self.upgrades.has(Upgrade::Bounty)) as u32;
            let luck = Upgrade::lucky_chance(self.upgrades.level(Upgrade::LuckyPenny));
            let lucky = luck > 0.0 && self.rng.f32() < luck;
            base * (1 + lucky as u32)
        };
        self.drop_gems(x, y, crystals);
        self.shove_neighbors(id, x, y);
        self.kills += 1;
        if self.upgrades.has(Upgrade::Leech)
            && self.kills / 12 != (self.kills - 1) / 12
            && self.lives < self.max_lives()
        {
            self.lives += 1;
        }
        self.burst(x, y, radius);
        self.kill_weight(word.len(), x, y, radius);
        let gone = self
            .rocks
            .iter()
            .position(|r| r.id == id)
            .map(|i| self.rocks.remove(i));
        if let Some(rock) = gone {
            self.on_kill(&rock);
        }
        self.clear_typed();
        // Chained kills fire in the same instant; the typed word keeps the stage.
        let stale = match &self.spent {
            Some(s) => self.time - s.t0 > 0.3,
            None => true,
        };
        if stale {
            self.spent = Some(Spent {
                word: word.to_string(),
                t0: self.time,
            });
        }
    }

    /// Multishot: a kill also pops the nearest hazard with a short word. The
    /// word length grows with Multishot; Scattershot adds more targets.
    fn chain_kills(&mut self, x: f32, y: f32) {
        let level = self.upgrades.level(Upgrade::Multishot);
        if level == 0 {
            return;
        }
        let longest = Upgrade::multishot_letters(level);
        let targets = 1 + self.upgrades.level(Upgrade::Scattershot);
        let (sx, sy) = self.ship_center();
        for _ in 0..targets {
            let pick = self
                .rocks
                .iter()
                .filter(|r| r.live && r.word.len() <= longest)
                .min_by(|a, b| {
                    let da = (a.x - sx).powi(2) + (a.y - sy).powi(2);
                    let db = (b.x - sx).powi(2) + (b.y - sy).powi(2);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|r| (r.id, r.word.clone(), r.radius, r.x, r.y));
            let Some((id, word, radius, rx, ry)) = pick else {
                break;
            };
            self.effects.push(Effect::Laser {
                x0: x,
                y0: y,
                x1: rx,
                y1: ry,
                life: 0.3,
                max: 0.3,
            });
            self.detonate(id, &word, radius, rx, ry, true);
        }
    }

    /// Shrapnel: a kill throws nearby hazards back, harder the closer they were.
    fn shove_neighbors(&mut self, id: u32, x: f32, y: f32) {
        let level = self.upgrades.level(Upgrade::Shrapnel);
        if level == 0 {
            return;
        }
        let reach = Upgrade::shrapnel_radius(level);
        for rock in self.rocks.iter_mut().filter(|r| r.live && r.id != id) {
            let (dx, dy) = (rock.x - x, rock.y - y);
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            if dist < reach {
                let push = (reach - dist) * 0.7;
                rock.x += dx / dist * push;
                rock.y += dy / dist * push;
            }
        }
    }

    /// Sweeper: detonate every other live hazard on screen.
    fn sweep(&mut self, x: f32, y: f32) {
        let all: Vec<(u32, String, f32, f32, f32)> = self
            .rocks
            .iter()
            .filter(|r| r.live)
            .map(|r| (r.id, r.word.clone(), r.radius, r.x, r.y))
            .collect();
        for (id, word, radius, rx, ry) in all {
            self.effects.push(Effect::Laser {
                x0: x,
                y0: y,
                x1: rx,
                y1: ry,
                life: 0.3,
                max: 0.3,
            });
            self.detonate(id, &word, radius, rx, ry, true);
        }
        self.banner = "SWEPT CLEAN".to_string();
        self.banner_t = 0.9;
    }

    fn drop_gems(&mut self, x: f32, y: f32, count: u32) {
        for _ in 0..count {
            self.gems.push(Gem {
                x,
                y,
                vx: self.rng.range(-6.0, 6.0),
                vy: self.rng.range(-5.0, 2.0),
                age: 0.0,
            });
        }
    }

    /// Gems drift out, then home in on the ship. `rush` skips the drift (used
    /// once a wave is cleared so nothing is left behind).
    fn update_gems(&mut self, dt: f32, rush: bool) {
        let (sx, sy) = self.ship_pos();
        let (w, h) = (self.play_w, self.play_h);
        let rush = rush || self.magnet;
        let delay = if rush { 0.0 } else { 0.45 };
        let before = self.gems.len();
        self.gems.retain_mut(|g| {
            g.age += dt;
            if g.age < delay {
                g.x = (g.x + g.vx * dt).clamp(0.0, w - 1.0);
                g.y = (g.y + g.vy * dt).clamp(0.0, h - 1.0);
                g.vx *= 1.0 - dt * 3.0;
                g.vy *= 1.0 - dt * 3.0;
                return true;
            }
            let (dx, dy) = (sx - g.x, sy - g.y);
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let step = (12.0 + (g.age - delay) * 70.0).min(80.0) * dt;
            if dist <= step + 0.9 {
                return false;
            }
            g.x += dx / dist * step;
            g.y += dy / dist * step;
            true
        });
        let got = before - self.gems.len();
        if got > 0 {
            self.sfx(Sfx::Gem);
            self.crystals += got as u32;
            self.gem_flash = 0.3;
        }
    }

    /// Hazards break apart in one of two ways. Space and Underwater pop in a
    /// ring and shards; Jungle and Cat Cafe spill their contents.
    fn burst(&mut self, x: f32, y: f32, radius: f32) {
        if matches!(self.theme, Theme::Jungle | Theme::CatCafe) {
            self.pour(x, y, radius);
            return;
        }
        self.effects.push(Effect::Ring {
            x,
            y,
            r: radius * 0.4,
            vr: 18.0,
            life: 0.42,
            max: 0.42,
        });
        self.shards(x, y, 6);
    }

    fn shards(&mut self, x: f32, y: f32, count: usize) {
        for i in 0..count {
            let a = (i as f32 / count as f32) * TAU;
            self.effects.push(Effect::Shard {
                x,
                y,
                vx: a.cos() * self.rng.range(4.0, 11.0),
                vy: a.sin() * self.rng.range(4.0, 11.0),
                rot: a,
                spin: self.rng.range(-6.0, 6.0),
                len: self.rng.range(0.6, 1.5),
                life: self.rng.range(0.35, 0.6),
                max: 0.6,
            });
        }
    }

    /// Liquid pours out: droplets arc out and fall, then a puddle spreads
    /// where the hazard sat and lingers.
    fn pour(&mut self, x: f32, y: f32, radius: f32) {
        let floor = (y + radius * 0.5).min(self.play_h - 1.0).max(y);
        for _ in 0..16 {
            let life = self.rng.range(0.9, 1.6);
            self.effects.push(Effect::Drop {
                x: x + self.rng.range(-radius * 0.5, radius * 0.5),
                y: y + self.rng.range(-radius * 0.4, 0.0),
                vx: self.rng.range(-5.0, 5.0),
                vy: self.rng.range(-8.0, -1.0),
                floor,
                life,
                max: life,
            });
        }
        self.effects.push(Effect::Puddle {
            x,
            y: floor,
            w: 0.0,
            reach: radius * 2.2,
            life: 2.4,
            max: 2.4,
        });
        self.shards(x, y, 3);
    }

    fn collide(&mut self, id: u32) {
        // Invulnerability blocks hull damage. It must not also destroy rocks or
        // clear the typing buffer, otherwise grazing the ship during i-frames
        // becomes a free way to clear the board.
        if self.invuln > 0.0 {
            return;
        }
        let Some(rock) = self.rocks.iter().find(|r| r.id == id && r.live) else {
            return;
        };
        let (x, y, radius) = (rock.x, rock.y, rock.radius);
        if self.shield > 0 {
            self.shield -= 1;
            self.invuln = 0.9;
            self.shake = 0.3;
            self.burst(x, y, radius);
            self.rocks.retain(|r| r.id != id);
            self.clear_typed();
            return;
        }
        self.lose_life(1.45);
        self.clear_typed();
        let kx = self.rng.range(-1.0, 1.0);
        let ky = self.rng.range(-1.0, 1.0);
        self.kick(kx, ky, 10.0);
        self.burst(x, y, radius);
        self.rocks.retain(|r| r.id != id);
    }

    /// Take one heart: Second Wind may save the run, otherwise the last heart
    /// starts the death sequence. `invuln` is the grace period afterwards.
    fn lose_life(&mut self, invuln: f32) {
        self.lives = self.lives.saturating_sub(1);
        self.hurt_this_wave = true;
        self.link = 0;
        if self.lives == 0 && self.upgrades.has(Upgrade::SecondWind) && !self.revive_used {
            self.revive_used = true;
            self.lives = 1;
            self.banner = "SECOND WIND".to_string();
            self.banner_t = 1.2;
        }
        self.invuln = invuln;
        self.hurt = 0.5;
        self.shake = 0.45;
        self.combo = 0;
        self.combo_left = 0.0;
        self.sfx(Sfx::Hurt);
        if self.lives == 0 {
            self.sfx_after(Sfx::GameOver, 0.3);
            self.phase = Phase::Dying;
            self.dying = 1.3;
            self.banner = lost_banner(self.theme).to_string();
            self.banner_t = 1.3;
            self.callouts.clear();
            self.callout("GAME OVER", 0.0, 1.3);
        }
    }

    fn start_boss(&mut self) {
        self.ready = 0.0;
        self.rocks.clear();
        self.budget = 0;
        self.spawn_timer = 99.0;
        self.pending_clear = false;
        self.clear_typed();
        let tier = boss::tier_of(self.wave);
        let rng = &mut self.rng;
        let mut boss = Boss::new(tier, |n| (rng.next() >> 33) as usize % n);
        if self.heat_rule(4) {
            boss.set_wpm(boss.wpm + 10);
        }
        self.banner.clear();
        self.banner_t = 0.0;
        self.boss = Some(boss);
        self.callout("WARNING", 0.0, 1.2);
        self.sfx(Sfx::BossWarning);
    }

    fn update_boss(&mut self, dt: f32) {
        let Some(b) = self.boss.as_mut() else {
            return;
        };
        if b.intro > 0.0 {
            b.intro -= dt;
            if b.intro <= 0.0 {
                self.lock_t = self.time;
            }
            return;
        }
        if !b.active() {
            return;
        }
        // Overdrive holds the clock for a moment.
        if self.boss_hold > 0.0 {
            self.boss_hold = (self.boss_hold - dt).max(0.0);
            return;
        }
        b.elapsed += dt;
        if b.elapsed >= b.limit {
            self.boss_timeout();
        }
    }

    /// A letter against the boss, Monkeytype style: every key goes into the
    /// buffer, right or wrong, and wrong ones stay (shown red) until
    /// backspaced. Only clean letters fire and count toward the pace.
    fn boss_type(&mut self, c: char) {
        let Some(word) = self.boss_word() else {
            return;
        };
        if self.buffer.len() >= word.len() + boss::MAX_EXTRA {
            self.slip(c);
            return;
        }
        let before = boss::clean_len(word, &self.buffer);
        self.buffer.push(c);
        self.buffer_t.push(self.time);
        self.sync_boss();
        if boss::clean_len(word, &self.buffer) == before {
            self.slip(c);
            return;
        }
        self.note_key(true);
        self.charge_drive(true);
        self.sfx(Sfx::Fire(self.buffer.len() as u8 - 1));
        self.boss_shot();
        // The last word needs no space: the passage ends on its last letter.
        if self.buffer == word && self.boss.as_ref().is_some_and(|b| b.on_last_word()) {
            self.boss_word_done(word);
        }
    }

    /// Space against the boss: on to the next word, but only once this one is
    /// spelled exactly right. A space on a typo is refused and counts as a miss.
    pub fn type_space(&mut self) {
        if self.phase != Phase::Playing || self.pending_clear || self.ready > 0.0 {
            return;
        }
        let Some(word) = self.boss_word() else {
            return;
        };
        if self.buffer.is_empty() {
            return;
        }
        if self.buffer != word {
            self.slip(' ');
            return;
        }
        self.note_key(true);
        self.charge_drive(true);
        self.boss_shot();
        self.boss_word_done(word);
    }

    /// The boss word being typed, while the fight is on.
    fn boss_word(&self) -> Option<&'static str> {
        self.boss.as_ref().filter(|b| b.fighting())?.word()
    }

    /// Recount the boss's pace after the buffer changes.
    fn sync_boss(&mut self) {
        if let Some(b) = self.boss.as_mut() {
            b.sync(&self.buffer);
        }
    }

    /// One laser from the ship to the boss.
    fn boss_shot(&mut self) {
        let (sx, sy) = self.ship_pos();
        let (bx, by) = self.boss_pos();
        let (fx, fy) = forward(self.angle);
        self.effects.push(Effect::Laser {
            x0: sx + fx * 1.9,
            y0: sy + fy * 1.9,
            x1: bx + self.rng.range(-2.0, 2.0),
            y1: by,
            life: 0.16,
            max: 0.16,
        });
        self.thrust = 1.0;
        self.kick(0.0, 1.0, 5.0);
    }

    fn boss_word_done(&mut self, word: &'static str) {
        self.sfx(Sfx::BossHit);
        if !self.fumbled {
            self.sfx_after(Sfx::Perfect, 0.05);
        }
        self.fumbled = false;
        let before = self.combo;
        self.combo = self.combo.saturating_add(1);
        self.combo_left = self.combo_window();
        self.run_stats.best_combo = self.run_stats.best_combo.max(self.combo);
        self.wave_stats.best_combo = self.wave_stats.best_combo.max(self.combo);
        self.announce_combo(before);
        let pts = word.len() as u32 * 40 * self.combo.min(12);
        self.score += pts;
        if self.score > self.best {
            self.best = self.score;
            self.best_dirty = true;
        }
        let (bx, by) = self.boss_pos();
        self.effects.push(Effect::Float {
            x: bx,
            y: by + 2.0,
            life: 0.7,
            max: 0.7,
            text: format!("+{pts}"),
        });
        self.clear_typed();
        self.spent = Some(Spent {
            word: word.to_string(),
            t0: self.time,
        });
        self.lock_t = self.time;
        let Some(b) = self.boss.as_mut() else {
            return;
        };
        b.bank();
        if b.idx >= b.words.len() {
            self.boss_win();
        }
    }

    fn boss_win(&mut self) {
        let Some(b) = self.boss.as_mut() else {
            return;
        };
        b.outcome = Some(true);
        let tier = b.tier;
        let wpm = (b.chars_total as f32 / 5.0 / (b.elapsed.max(1.0) / 60.0)).round() as u32;
        let reward = 12 + 8 * tier;
        self.crystals += reward;
        self.score += 1000 * tier + wpm * 10;
        if self.score > self.best {
            self.best = self.score;
            self.best_dirty = true;
        }
        self.lives = (self.lives + 1).min(self.max_lives());
        self.rack_bonus += 1;
        self.banner = format!("BOSS DOWN  {wpm} WPM  +{reward} CRYSTALS  +1 RACK SLOT");
        self.banner_t = 2.2;
        self.pending_clear = true;
        self.clear_t = 2.4;
        self.callout("K.O.!", 0.0, 1.8);
        self.sfx(Sfx::BossDown);
        self.hit_stop(0.2);
        self.finale = tier == boss::FINAL_TIER && !self.won;
        let (bx, by) = self.boss_pos();
        for _ in 0..6 {
            let (dx, dy) = (self.rng.range(-6.0, 6.0), self.rng.range(-2.0, 2.0));
            self.burst(bx + dx, by + dy, 2.5);
        }
        self.shake = 0.6;
    }

    /// Missing the pace ends the run: the boss escapes and no heart, Ward
    /// or Second Wind can save it.
    fn boss_timeout(&mut self) {
        let Some(b) = self.boss.as_mut() else {
            return;
        };
        b.outcome = Some(false);
        self.boss_escaped = Some(b.wpm);
        self.clear_typed();
        self.rocks.clear();
        self.lives = 0;
        self.hurt = 0.5;
        self.shake = 0.45;
        self.combo = 0;
        self.combo_left = 0.0;
        self.phase = Phase::Dying;
        // Long enough to watch the boss float away.
        self.dying = 2.0;
        self.banner = "TIME UP  THE BOSS ESCAPES".to_string();
        self.banner_t = 2.0;
        self.callouts.clear();
        self.callout("TIME UP", 0.0, 1.6);
        self.sfx(Sfx::Hurt);
        self.sfx_after(Sfx::GameOver, 0.4);
    }

    #[cfg(test)]
    pub fn begin_wave_for_test(&mut self, wave: u32) {
        self.wave = wave;
        self.begin_wave();
    }

    #[cfg(test)]
    pub fn open_shop_for_test(&mut self) {
        self.open_shop(false);
        self.input_lock = 0.0;
        self.shop_t0 = -10.0;
        self.snap_counters();
    }

    /// A copy of a run at the victory screen (Game itself is not Clone).
    #[cfg(test)]
    pub fn clone_for_test(&self) -> Game {
        let mut g = Game::new(self.term_w, self.term_h, 7);
        g.load_run(self.snapshot(false));
        g.phase = self.phase;
        g.menu = self.menu;
        g
    }

    /// Skip the "WAVE n ... GO!" beat so a test can type at once.
    #[cfg(test)]
    pub fn skip_ready_for_test(&mut self) {
        self.ready = 0.0;
        self.callouts.clear();
    }

    /// Cards on the shelf: three, or four with Wide Shop.
    pub fn shop_slots(&self) -> usize {
        SHOP_SLOTS + self.upgrades.has(Upgrade::WideShop) as usize - self.heat_rule(2) as usize
    }

    /// Menu index of the reroll button; the next-wave button follows it.
    pub fn shop_reroll_index(&self) -> usize {
        self.shop_slots()
    }

    pub fn shop_go_index(&self) -> usize {
        self.shop_slots() + 1
    }

    pub fn shop_move(&mut self, delta: isize) {
        self.sfx(Sfx::Hover);
        let entries = self.shop_slots() as isize + 2;
        self.shop_sel = (self.shop_sel as isize + delta).rem_euclid(entries) as usize;
    }

    /// Enter the shop. `pay_interest` is false when resuming a saved shop.
    fn open_shop(&mut self, pay_interest: bool) {
        self.phase = Phase::Shop;
        self.boss = None;
        self.rocks.clear();
        self.clear_typed();
        self.shop_note.clear();
        self.input_lock = MENU_LOCK;
        self.shop_t0 = self.time;
        self.bought_t = [-10.0; 4];
        self.rack_focus = false;
        self.overdrive = 0.0;
        self.freeze = 0.0;
        if pay_interest && self.upgrades.has(Upgrade::Interest) {
            let gain = (self.crystals / 5).min(5);
            self.crystals += gain;
            if gain > 0 {
                self.shop_note = format!("Interest paid {gain} crystals.");
            }
        }
        let offers = self.roll_offers();
        if offers.is_empty() {
            self.shop_note = "Sold out! You own everything. Head on to the next wave.".to_string();
        }
        self.shop = Some(Shop { offers, rerolls: 0 });
        self.shop_sel = 0;
        self.autosave(true);
    }

    fn eligible(&self, up: Upgrade) -> bool {
        match up {
            Upgrade::PatchKit => self.lives < self.max_lives(),
            // Scattershot only improves Multishot.
            Upgrade::Scattershot => {
                self.upgrades.has(Upgrade::Multishot) && !self.upgrades.maxed(up)
            }
            _ => !self.upgrades.maxed(up),
        }
    }

    /// The card a shop would show for this line: its next level.
    fn next_card(&self, up: Upgrade) -> Offer {
        Offer::new(up, self.upgrades.level(up) + 1)
    }

    fn roll_offers(&mut self) -> Vec<Offer> {
        let mut pool: Vec<Upgrade> = Upgrade::ALL
            .into_iter()
            .filter(|&u| self.eligible(u))
            .collect();
        let mut offers = Vec::new();
        let weight = |g: &Game, u: Upgrade| g.next_card(u).rarity().weight();
        while offers.len() < self.shop_slots() && !pool.is_empty() {
            let total: f32 = pool.iter().map(|&u| weight(self, u)).sum();
            let mut roll = self.rng.f32() * total;
            let mut pick = pool.len() - 1;
            for (i, &u) in pool.iter().enumerate() {
                roll -= weight(self, u);
                if roll <= 0.0 {
                    pick = i;
                    break;
                }
            }
            let up = pool.remove(pick);
            offers.push(self.next_card(up));
        }
        offers
    }

    /// What a card costs right now, after Discount.
    /// What a card costs right now: its base price, plus inflation from
    /// everything already held (not for patch kits), minus Discount.
    pub fn offer_cost(&self, offer: &Offer) -> u32 {
        let off = Upgrade::discount_off(self.upgrades.level(Upgrade::Discount));
        let inflation = if offer.upgrade.def().consumable {
            0
        } else {
            self.inflation()
        };
        (offer.upgrade.cost(offer.tier - 1) + inflation)
            .saturating_sub(off)
            .max(1)
    }

    pub fn reroll_cost(&self) -> u32 {
        let spent = self.shop.as_ref().map_or(0, |s| s.rerolls);
        let free = self.upgrades.has(Upgrade::Haggler) as u32;
        if free == 1 && spent == 0 {
            0
        } else {
            REROLL_BASE + spent.saturating_sub(free)
        }
    }

    pub fn shop_buy(&mut self, slot: usize) {
        if self.phase != Phase::Shop {
            return;
        }
        let Some(offer) = self.shop.as_ref().and_then(|s| s.offers.get(slot)) else {
            return;
        };
        let (up, tier) = (offer.upgrade, offer.tier);
        if offer.sold {
            self.shop_note = "Already bought.".to_string();
            self.sfx(Sfx::Deny);
            return;
        }
        let cost = self.offer_cost(offer);
        if self.rack_full_for(up) {
            self.shop_note = format!(
                "Rack full ({}/{}): press tab to sell a card first.",
                self.rack_used(),
                self.rack_size()
            );
            self.sfx(Sfx::Deny);
            return;
        }
        if self.crystals < cost {
            self.shop_note = format!("Need {} more crystals.", cost - self.crystals);
            self.sfx(Sfx::Deny);
            return;
        }
        self.crystals -= cost;
        self.sfx(Sfx::Buy);
        if let Some(t) = self.bought_t.get_mut(slot) {
            *t = self.time;
        }
        match up {
            Upgrade::PatchKit => self.lives = (self.lives + 1).min(self.max_lives()),
            Upgrade::ExtraHeart => self.lives += 1,
            _ => {}
        }
        self.upgrades.add(up);
        // The card stays sold: the next tier has to roll in on a later shop
        // or reroll.
        let name = up.tier(tier).name;
        if let Some(offer) = self.shop.as_mut().and_then(|s| s.offers.get_mut(slot)) {
            offer.sold = true;
        }
        self.shop_note = if self
            .shop
            .as_ref()
            .is_some_and(|s| s.offers.iter().all(|o| o.sold))
        {
            format!("Bought {name}. Sold out: reroll or head on.")
        } else {
            format!("Bought {name}.")
        };
        self.autosave(true);
    }

    pub fn shop_reroll(&mut self) {
        if self.phase != Phase::Shop {
            return;
        }
        if !Upgrade::ALL.into_iter().any(|u| self.eligible(u)) {
            // Rerolling would only show the same empty shelf.
            self.shop_note = "Sold out! Nothing left to sell.".to_string();
            self.sfx(Sfx::Deny);
            return;
        }
        let cost = self.reroll_cost();
        if self.crystals < cost {
            self.shop_note = format!("Need {} more crystals.", cost - self.crystals);
            self.sfx(Sfx::Deny);
            return;
        }
        self.crystals -= cost;
        let refund = Upgrade::recycler_refund(self.upgrades.level(Upgrade::Recycler));
        if self.upgrades.has(Upgrade::Recycler) {
            self.crystals += refund.min(cost);
        }
        let offers = self.roll_offers();
        if let Some(shop) = self.shop.as_mut() {
            shop.rerolls += 1;
            shop.offers = offers;
        }
        self.shop_note = "Rerolled.".to_string();
        self.sfx(Sfx::Reroll);
        self.autosave(true);
    }

    pub fn shop_activate(&mut self) {
        let sel = self.shop_sel;
        if sel == self.shop_reroll_index() {
            self.shop_reroll();
        } else if sel == self.shop_go_index() {
            self.leave_shop();
        } else {
            self.shop_buy(sel);
        }
    }

    pub fn leave_shop(&mut self) {
        if self.phase != Phase::Shop {
            return;
        }
        self.sfx(Sfx::Select);
        self.shop = None;
        self.rack_focus = false;
        self.phase = Phase::Playing;
        self.begin_wave();
    }

    /// Turn on save files. Only the real binary calls this.
    pub fn enable_persistence(&mut self) {
        self.persist = true;
        self.has_save = save::path().is_some_and(|p| p.exists());
        self.load_files();
    }

    fn snapshot(&self, in_shop: bool) -> RunSave {
        RunSave {
            wave: self.wave,
            score: self.score,
            lives: self.lives,
            crystals: self.crystals,
            kills: self.kills,
            revive_used: self.revive_used,
            in_shop,
            upgrades: self.upgrades,
            rack_bonus: self.rack_bonus,
            heat: self.heat,
            typed_kills: self.typed_kills,
            daily: self.daily.unwrap_or(0),
            won: self.won,
        }
    }

    fn autosave(&mut self, in_shop: bool) {
        if !self.persist {
            return;
        }
        if let Some(path) = save::path() {
            self.has_save = fs::write(path, save::encode(&self.snapshot(in_shop))).is_ok();
        }
    }

    fn clear_save(&mut self) {
        if self.persist {
            if let Some(path) = save::path() {
                let _ = fs::remove_file(path);
            }
        }
        self.has_save = false;
    }

    /// Save the run (at the start of the current wave, or in the shop) and go
    /// back to the title screen.
    pub fn save_and_quit_to_title(&mut self) {
        if matches!(self.phase, Phase::Playing | Phase::Paused | Phase::Shop) {
            let in_shop = self.in_shop();
            self.autosave(in_shop);
        }
        self.return_to_title();
    }

    /// Resume the saved run from disk. Returns false if there is none.
    pub fn continue_saved(&mut self) -> bool {
        let run = save::path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|text| save::decode(&text));
        match run {
            Some(run) => {
                self.load_run(run);
                true
            }
            None => {
                self.has_save = false;
                false
            }
        }
    }

    pub fn load_run(&mut self, run: RunSave) {
        self.theme_menu_open = false;
        self.reset_run();
        self.wave = run.wave;
        self.score = run.score;
        self.crystals = run.crystals;
        self.kills = run.kills;
        self.revive_used = run.revive_used;
        self.upgrades = run.upgrades;
        self.rack_bonus = run.rack_bonus;
        self.heat = run.heat;
        self.typed_kills = run.typed_kills;
        self.daily = (run.daily > 0).then_some(run.daily);
        self.won = run.won;
        self.lives = run.lives.min(self.max_lives());
        self.snap_counters();
        if run.in_shop {
            self.open_shop(false);
        } else {
            self.phase = Phase::Playing;
            self.begin_wave();
        }
    }

    fn spawn_threat(&mut self) {
        let kind = self.roll_kind();
        let word = match kind {
            Kind::Runner => self.pick_word_from(words::SHORT, 4),
            _ => self.pick_word(),
        };
        let radius = match kind {
            Kind::Runner => 1.5,
            _ => 1.8 + (word.len() as f32 * 0.15).min(1.2),
        };
        let (x, y) = self.edge_point();
        let (cx, cy) = self.ship_center();
        let tx = cx + self.rng.range(-7.0, 7.0);
        let ty = cy + self.rng.range(-4.0, 4.0);
        let speed = (hazard_speed(self.wave) + self.rng.range(0.0, 0.8)) * kind.speed();
        let (dx, dy) = normalize(tx - x, ty - y);
        let mut rock = self.make_rock(x, y, radius, word);
        rock.vx = dx * speed;
        rock.vy = dy * speed;
        rock.kind = kind;
        rock.armor = (kind == Kind::Armored) as u8;
        self.rocks.push(rock);
    }

    fn spawn_drifter(&mut self) {
        let word = self.rng.pick(words::SHORT).to_string();
        let radius = 1.8 + (word.len() as f32 * 0.15);
        let x = self.rng.range(2.0, self.play_w - 2.0);
        let y = self.rng.range(2.0, (self.play_h - 2.0).max(3.0));
        let mut rock = self.make_rock(x, y, radius, word);
        let a = self.rng.range(0.0, TAU);
        let sp = self.rng.range(2.0, 5.0);
        rock.vx = a.cos() * sp;
        rock.vy = a.sin() * sp * 0.6;
        self.rocks.push(rock);
    }

    fn edge_point(&mut self) -> (f32, f32) {
        let side = self.rng.next() % 4;
        let m = 2.0;
        match side {
            0 => (self.rng.range(0.0, self.play_w), -m),
            1 => (self.rng.range(0.0, self.play_w), self.play_h + m),
            2 => (-m, self.rng.range(0.0, self.play_h)),
            _ => (self.play_w + m, self.rng.range(0.0, self.play_h)),
        }
    }

    fn make_rock(&mut self, x: f32, y: f32, radius: f32, word: String) -> Rock {
        let round = self.theme == Theme::Jungle || self.theme == Theme::CatCafe;
        let n = if round {
            12
        } else {
            8 + (self.rng.next() as usize % 4)
        };
        let mut shape = Vec::with_capacity(n);
        for i in 0..n {
            let jitter = if round {
                self.rng.range(-0.06, 0.06)
            } else {
                self.rng.range(-0.22, 0.22)
            };
            let a = (i as f32 + jitter) / n as f32 * TAU;
            let r = radius
                * if round {
                    self.rng.range(0.94, 1.04)
                } else {
                    self.rng.range(0.72, 1.12)
                };
            shape.push((a.cos() * r, a.sin() * r));
        }
        let id = self.next_id;
        self.next_id += 1;
        Rock {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            radius,
            rot: self.rng.range(0.0, TAU),
            spin: self.rng.range(-0.8, 0.8),
            shape,
            word,
            live: true,
            hit: 0.0,
            kind: Kind::Normal,
            armor: 0,
        }
    }

    fn pick_word(&mut self) -> String {
        let pool = self.pool();
        self.pick_word_from(pool, usize::MAX)
    }

    /// A word from `pool` of at most `longest` letters that does not clash
    /// with a word already on screen (same start letter, or a prefix).
    fn pick_word_from(&mut self, pool: &'static [&'static str], longest: usize) -> String {
        let used: Vec<&str> = self.rocks.iter().map(|r| r.word.as_str()).collect();
        for _ in 0..64 {
            let word = self.rng.pick(pool);
            if word.len() > longest || used.iter().any(|u| conflicts(u, word)) {
                continue;
            }
            return word.to_string();
        }
        for _ in 0..32 {
            let word = self.rng.pick(pool);
            if word.len() > longest {
                continue;
            }
            if used
                .iter()
                .any(|u| *u == word || u.starts_with(word) || word.starts_with(u))
            {
                continue;
            }
            return word.to_string();
        }
        self.rng.pick(pool).to_string()
    }

    /// The word pool for the next spawn. Short words first, then mid, then
    /// long words ramping in a little more every wave (never above 60%).
    fn pool(&mut self) -> &'static [&'static str] {
        let roll = self.rng.f32();
        let (short, long) = word_mix(self.wave);
        if roll < short {
            words::SHORT
        } else if roll < short + long {
            words::LONG
        } else {
            words::MID
        }
    }

    fn spring_ship(&mut self, dt: f32) {
        let k = 32.0;
        let d = 9.0;
        self.kick_vx += (-k * self.ox - d * self.kick_vx) * dt;
        self.kick_vy += (-k * self.oy - d * self.kick_vy) * dt;
        self.ox += self.kick_vx * dt;
        self.oy += self.kick_vy * dt;
        self.ox = self.ox.clamp(-3.0, 3.0);
        self.oy = self.oy.clamp(-2.2, 2.2);
    }

    fn spawn_exhaust(&mut self, dt: f32) {
        let yawing = self.spin.abs() > 1.4 && self.phase != Phase::Paused;
        if self.thrust < 0.08 && !yawing {
            return;
        }
        self.exhaust += dt;
        let (sx, sy) = self.ship_pos();
        let (fx, fy) = forward(self.angle);
        // Sparse exhaust: one dim dot every so often, drifting back from the hull.
        let period = 0.06;
        let (back, speed) = match self.theme {
            Theme::Space => (1.45, 9.0),
            Theme::Underwater => (1.95, 7.5),
            Theme::Jungle => (1.4, 4.0),
            Theme::CatCafe => (1.65, 5.0),
        };
        while self.exhaust > period {
            self.exhaust -= period;
            let spread = self.rng.range(-0.45, 0.45);
            let (px, py) = rotate(spread, 0.0, self.angle);
            self.effects.push(Effect::Spark {
                x: sx - fx * back + px,
                y: sy - fy * back + py,
                vx: -fx * speed * self.rng.range(0.6, 1.2) + self.rng.range(-1.2, 1.2),
                vy: -fy * speed * self.rng.range(0.6, 1.2) + self.rng.range(-1.2, 1.2),
                life: self.rng.range(0.15, 0.35),
                max: 0.35,
                glyph: '·',
            });
        }
    }

    fn update_effects(&mut self, dt: f32) {
        for effect in &mut self.effects {
            match effect {
                Effect::Spark {
                    x, y, vx, vy, life, ..
                } => {
                    *x += *vx * dt;
                    *y += *vy * dt;
                    *vx *= 1.0 - dt * 1.4;
                    *vy *= 1.0 - dt * 1.4;
                    *life -= dt;
                }
                Effect::Shard {
                    x,
                    y,
                    vx,
                    vy,
                    rot,
                    spin,
                    life,
                    ..
                } => {
                    *x += *vx * dt;
                    *y += *vy * dt;
                    *rot += *spin * dt;
                    *life -= dt;
                }
                Effect::Laser { life, .. } => *life -= dt,
                Effect::Ring { r, vr, life, .. } => {
                    *r += *vr * dt;
                    *life -= dt;
                }
                Effect::Float { y, life, .. } => {
                    *y -= 6.0 * dt;
                    *life -= dt;
                }
                Effect::Drop {
                    x,
                    y,
                    vx,
                    vy,
                    floor,
                    life,
                    ..
                } => {
                    if *y < *floor {
                        *vy += DROP_GRAVITY * dt;
                        *x += *vx * dt;
                        *y = (*y + *vy * dt).min(*floor);
                        *vx *= 1.0 - dt * 1.2;
                        *life -= dt;
                    } else {
                        // Landed: soak in quickly.
                        *vy = 0.0;
                        *life -= dt * 4.0;
                    }
                }
                Effect::Puddle { w, reach, life, .. } => {
                    *w += (*reach - *w) * (dt * 5.0).min(1.0);
                    *life -= dt;
                }
            }
        }
        self.effects.retain(|e| match e {
            Effect::Spark { life, .. }
            | Effect::Shard { life, .. }
            | Effect::Laser { life, .. }
            | Effect::Ring { life, .. }
            | Effect::Float { life, .. }
            | Effect::Drop { life, .. }
            | Effect::Puddle { life, .. } => *life > 0.0,
        });
    }

    fn decay_fx(&mut self, dt: f32) {
        self.thrust = (self.thrust - dt * 1.6).max(0.0);
        self.miss = (self.miss - dt).max(0.0);
        self.hurt = (self.hurt - dt).max(0.0);
        self.invuln = (self.invuln - dt).max(0.0);
        self.banner_t = (self.banner_t - dt).max(0.0);
        self.gem_flash = (self.gem_flash - dt).max(0.0);
        self.brake = (self.brake - dt).max(0.0);
        if self.phase == Phase::Playing {
            self.overdrive = (self.overdrive - dt).max(0.0);
            self.freeze = (self.freeze - dt).max(0.0);
        }
        if let Some(b) = self.boss.as_mut() {
            b.age += dt;
            b.strike = (b.strike - dt).max(0.0);
            if !b.active() {
                b.outcome_t += dt;
            }
        }
        if self.combo > 0 && self.phase == Phase::Playing {
            self.combo_left -= dt;
            if self.combo_left <= 0.0 {
                self.combo = 0;
            }
        }
        if self.shake > 0.0 {
            self.shake = (self.shake - dt * 1.7).max(0.0);
            let mag = self.shake.min(0.85) * self.settings.shake_factor();
            self.shake_x = (self.time * 47.0).sin() * mag;
            self.shake_y = (self.time * 63.0).cos() * mag * 0.6;
        } else {
            self.shake_x = 0.0;
            self.shake_y = 0.0;
        }
    }

    fn reseed_stars(&mut self) {
        self.stars.clear();
        let density = match self.theme {
            Theme::Jungle => 260.0,
            Theme::CatCafe => 200.0,
            _ => 70.0,
        };
        let count = ((self.play_w * self.play_h) / density) as usize;
        let cap = match self.theme {
            Theme::Jungle => 30,
            Theme::CatCafe => 30,
            _ => 80,
        };
        for _ in 0..count.min(cap) {
            let glyph = match self.theme {
                Theme::Space => {
                    if self.rng.f32() < 0.93 {
                        '·'
                    } else {
                        '+'
                    }
                }
                Theme::Underwater => {
                    let roll = self.rng.f32();
                    if roll < 0.68 {
                        '·'
                    } else if roll < 0.9 {
                        '°'
                    } else {
                        '∘'
                    }
                }
                Theme::Jungle => {
                    let roll = self.rng.f32();
                    if roll < 0.16 {
                        '✦'
                    } else if roll < 0.56 {
                        '❧'
                    } else {
                        ','
                    }
                }
                Theme::CatCafe => {
                    let roll = self.rng.f32();
                    if roll < 0.35 {
                        '~'
                    } else if roll < 0.70 {
                        '·'
                    } else {
                        '✧'
                    }
                }
            };
            self.stars.push(Star {
                x: self.rng.range(0.0, self.play_w),
                y: self.rng.range(0.0, self.play_h),
                glyph,
                phase: self.rng.range(0.0, TAU),
            });
        }
    }

    /// How long `main.rs` may wait between frames. Screens where only a
    /// cursor blinks redraw at 30 fps; keys still cut the wait short.
    pub fn frame_budget(&self) -> std::time::Duration {
        let micros = match self.phase {
            Phase::Playing | Phase::Dying | Phase::Title => 8_333,
            _ => 33_333,
        };
        std::time::Duration::from_micros(micros)
    }

    pub fn save_best(&mut self) {
        if !self.best_dirty {
            return;
        }
        self.best_dirty = false;
        if let Some(path) = best_path() {
            let _ = fs::write(path, self.best.to_string());
        }
    }
}

/// Words between Sweeper blasts.
const SWEEP_EVERY: u32 = 25;

/// Base drift speed of a wave's hazards. It grows quickly at first and then
/// levels off, so late waves get harder through elites, not raw speed.
fn hazard_speed(wave: u32) -> f32 {
    3.4 + 2.2 * (1.0 + 0.45 * wave as f32).ln()
}

/// Seconds between spawns. It tightens slowly so an average typist can
/// keep up for the first boss or two; elites carry the late difficulty.
fn spawn_gap(wave: u32) -> f32 {
    (2.0 - 0.09 * wave as f32).max(0.9)
}

/// Shares of short and long words in a wave; the rest are mid-length.
fn word_mix(wave: u32) -> (f32, f32) {
    match wave {
        0 | 1 => (1.0, 0.0),
        2 | 3 => (0.5, 0.0),
        4 | 5 => (0.2, 0.0),
        _ => (0.1, (0.1 + 0.06 * (wave - 5) as f32).min(0.6)),
    }
}

fn clear_banner(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "WAVE CLEAR",
        Theme::Underwater => "MINEFIELD CLEAR",
        Theme::Jungle => "CANOPY CLEAR",
        Theme::CatCafe => "CAFE CLEAR",
    }
}

/// The banner when the last heart goes; the game-over screens reuse it.
pub fn lost_banner(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "SHIP LOST",
        Theme::Underwater => "HULL BREACH",
        Theme::Jungle => "MONKEY DOWN",
        Theme::CatCafe => "CAT NAPPING",
    }
}

fn conflicts(existing: &str, word: &str) -> bool {
    existing == word
        || existing.starts_with(word)
        || word.starts_with(existing)
        || existing.as_bytes().first() == word.as_bytes().first()
}

fn normalize(x: f32, y: f32) -> (f32, f32) {
    let m = (x * x + y * y).sqrt();
    if m < 0.001 {
        (1.0, 0.0)
    } else {
        (x / m, y / m)
    }
}

fn load_best() -> u32 {
    best_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn best_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".typoids-best"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> Game {
        let mut g = Game::new(100, 40, 42);
        g.phase = Phase::Playing;
        g.rocks.clear();
        g.budget = 0;
        g.spawn_timer = 99.0;
        g.effects.clear();
        g
    }

    #[test]
    fn jungle_and_cat_hazards_pour_and_linger() {
        for theme in [Theme::Jungle, Theme::CatCafe] {
            let mut g = playing();
            g.theme = theme;
            g.burst(50.0, 20.0, 2.0);
            let has = |g: &Game, f: fn(&Effect) -> bool| g.effects.iter().any(f);
            assert!(has(&g, |e| matches!(e, Effect::Drop { .. })), "{theme:?}");
            assert!(has(&g, |e| matches!(e, Effect::Puddle { .. })), "{theme:?}");
            assert!(!has(&g, |e| matches!(e, Effect::Ring { .. })), "{theme:?}");
            // Droplets are gone quickly; the puddle is still there after 1.5s.
            for _ in 0..90 {
                g.update_effects(1.0 / 60.0);
            }
            assert!(!has(&g, |e| matches!(e, Effect::Drop { .. })), "{theme:?}");
            assert!(has(&g, |e| matches!(e, Effect::Puddle { .. })), "{theme:?}");
            // ...and it eventually soaks away.
            for _ in 0..120 {
                g.update_effects(1.0 / 60.0);
            }
            assert!(
                !has(&g, |e| matches!(e, Effect::Puddle { .. })),
                "{theme:?}"
            );
        }
    }

    #[test]
    fn space_and_underwater_hazards_still_pop() {
        for theme in [Theme::Space, Theme::Underwater] {
            let mut g = playing();
            g.theme = theme;
            g.burst(50.0, 20.0, 2.0);
            assert!(g.effects.iter().any(|e| matches!(e, Effect::Ring { .. })));
            assert!(!g.effects.iter().any(|e| matches!(e, Effect::Puddle { .. })));
        }
    }

    #[test]
    fn title_menu_lists_every_action_and_wraps() {
        let mut g = Game::new(100, 40, 42);
        assert_eq!(g.menu, MENU_START);
        let n = MENU_ITEMS.len();
        let actions: Vec<TitleAction> = (0..n)
            .map(|_| {
                let a = g.title_action();
                g.menu_move(1);
                a
            })
            .collect();
        assert_eq!(
            actions,
            [
                TitleAction::Start,
                TitleAction::Daily,
                TitleAction::Themes,
                TitleAction::Options,
                TitleAction::Help,
                TitleAction::Quit
            ]
        );
        assert_eq!(g.menu, MENU_START, "wraps back to the top");
        g.menu_move(-1);
        assert_eq!(g.title_action(), TitleAction::Quit);
    }

    #[test]
    fn a_saved_run_adds_continue_to_the_title() {
        let mut g = Game::new(100, 40, 42);
        g.has_save = true;
        assert_eq!(g.menu_items(), MENU_ITEMS_SAVED);
        assert_eq!(g.title_action(), TitleAction::Continue);
        g.menu_move(1);
        assert_eq!(g.title_action(), TitleAction::Start);
    }

    /// A game mid-wave with `n` crystals and one locked-in rock to kill.
    fn with_rock(word: &str) -> Game {
        let mut g = playing();
        g.budget = 5;
        let rock = g.make_rock(60.0, 10.0, 2.0, word.to_string());
        g.rocks.push(rock);
        g
    }

    fn type_word(g: &mut Game, text: &str) {
        for c in text.chars() {
            g.type_char(c);
        }
    }

    #[test]
    fn kills_drop_crystals_that_fly_to_the_ship() {
        let mut g = with_rock("star");
        type_word(&mut g, "star");
        assert!(g.rocks.is_empty());
        assert!(!g.gems.is_empty(), "a kill drops gems");
        assert_eq!(g.crystals, 0, "not collected yet");
        for _ in 0..180 {
            g.update(1.0 / 60.0);
            if g.gems.is_empty() {
                break;
            }
        }
        assert!(g.gems.is_empty());
        assert!(g.crystals >= 1);
    }

    #[test]
    fn clearing_a_wave_opens_the_shop_with_three_offers() {
        let mut g = playing();
        g.budget = 0;
        g.wave = 1;
        for _ in 0..400 {
            g.update(1.0 / 60.0);
            if g.phase == Phase::Shop {
                break;
            }
        }
        assert_eq!(g.phase, Phase::Shop);
        assert_eq!(g.wave, 2);
        let shop = g.shop.as_ref().unwrap();
        assert_eq!(shop.offers.len(), SHOP_SLOTS);
        let mut ids: Vec<_> = shop.offers.iter().map(|o| o.upgrade as usize).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), SHOP_SLOTS, "offers are distinct");
        g.leave_shop();
        assert_eq!(g.phase, Phase::Playing);
        assert!(g.shop.is_none());
    }

    #[test]
    fn buying_spends_crystals_and_applies_the_upgrade() {
        let mut g = playing();
        g.open_shop(false);
        g.shop.as_mut().unwrap().offers[0] = Offer::new(Upgrade::ExtraHeart, 1);
        g.crystals = 3;
        g.shop_buy(0);
        assert_eq!(g.crystals, 3, "too poor");
        assert!(g.shop_note.contains("Need"));
        g.crystals = 40;
        g.shop_buy(0);
        assert_eq!(g.crystals, 40 - Upgrade::ExtraHeart.cost(0));
        assert_eq!(g.max_lives(), 4);
        assert_eq!(g.lives, 4);
        // The card stays sold; the next tier costs more and has to roll in.
        assert!(g.shop.as_ref().unwrap().offers[0].sold);
        let next = g.next_card(Upgrade::ExtraHeart);
        assert_eq!(next.tier, 2);
        assert_eq!(g.offer_cost(&next), Upgrade::ExtraHeart.cost(1));

        g.shop.as_mut().unwrap().offers[1] = Offer::new(Upgrade::SecondWind, 1);
        g.shop_buy(1);
        let before = g.crystals;
        g.shop_buy(1);
        assert_eq!(g.crystals, before, "sold cards cannot be bought twice");
    }

    #[test]
    fn a_shop_with_nothing_left_says_sold_out() {
        let mut g = playing();
        for up in Upgrade::ALL {
            g.upgrades.set(up, up.max());
        }
        g.lives = g.max_lives();
        g.crystals = 100;
        g.open_shop(false);
        assert!(g.shop.as_ref().unwrap().offers.is_empty());
        assert!(g.shop_note.contains("Sold out"));
        g.shop_reroll();
        assert_eq!(g.crystals, 100, "no charge for rerolling an empty shelf");
        g.shop_buy(0);
        assert_eq!(g.crystals, 100);
        g.leave_shop();
        assert_eq!(g.phase, Phase::Playing, "can still move on");
    }

    #[test]
    fn rerolling_costs_more_each_time_and_haggler_gives_one_free() {
        let mut g = playing();
        g.open_shop(false);
        g.crystals = 50;
        assert_eq!(g.reroll_cost(), REROLL_BASE);
        g.shop_reroll();
        assert_eq!(g.crystals, 50 - REROLL_BASE);
        assert_eq!(g.reroll_cost(), REROLL_BASE + 1);

        let mut h = playing();
        h.upgrades.set(Upgrade::Haggler, 1);
        h.open_shop(false);
        h.crystals = 50;
        assert_eq!(h.reroll_cost(), 0);
        h.shop_reroll();
        assert_eq!(h.crystals, 50);
        assert_eq!(h.reroll_cost(), REROLL_BASE);
    }

    #[test]
    fn steady_hands_rewards_clean_words_only() {
        let mut plain = with_rock("planet");
        type_word(&mut plain, "planet");
        let (base_score, base_gems) = (plain.score, plain.gems.len());

        let mut clean = with_rock("planet");
        clean.upgrades.set(Upgrade::SteadyHands, 2);
        type_word(&mut clean, "planet");
        assert_eq!(clean.score, base_score * 150 / 100);
        assert_eq!(clean.gems.len(), base_gems + 1, "one crystal at any level");

        let mut sloppy = with_rock("planet");
        sloppy.upgrades.set(Upgrade::SteadyHands, 2);
        type_word(&mut sloppy, "pz");
        type_word(&mut sloppy, "lanet");
        assert_eq!(sloppy.score, base_score, "a slip forfeits the bonus");
        assert_eq!(sloppy.gems.len(), base_gems);
    }

    #[test]
    fn stackable_lines_sell_one_tier_at_a_time() {
        let mut g = playing();
        g.open_shop(false);
        g.crystals = 500;
        g.shop.as_mut().unwrap().offers[0] = Offer::new(Upgrade::SlowTide, 1);
        g.shop_buy(0);
        assert_eq!(g.upgrades.level(Upgrade::SlowTide), 1);
        assert!(g.shop.as_ref().unwrap().offers[0].sold, "no restock");
        let held = g.crystals;
        g.shop_buy(0);
        assert_eq!(g.crystals, held, "cannot buy the same card twice");

        // Later shops only offer the next tier of the line.
        for _ in 0..30 {
            let offers = g.roll_offers();
            for o in offers.iter().filter(|o| o.upgrade == Upgrade::SlowTide) {
                assert_eq!(o.tier, 2);
                assert_eq!(o.name(), "DEEP CURRENT");
            }
        }
        g.upgrades.set(Upgrade::SlowTide, Upgrade::SlowTide.max());
        assert!(!g.eligible(Upgrade::SlowTide), "nothing past the last tier");
    }

    #[test]
    fn every_card_has_its_own_short_name() {
        let mut seen = Vec::new();
        for up in Upgrade::ALL {
            for tier in up.def().tiers {
                assert!(tier.name.chars().count() <= 12, "{} too long", tier.name);
                assert!(!seen.contains(&tier.name), "{} repeats", tier.name);
                seen.push(tier.name);
            }
        }
    }

    #[test]
    fn wide_shop_adds_a_fourth_card() {
        let mut g = playing();
        assert_eq!(g.shop_slots(), 3);
        g.upgrades.set(Upgrade::WideShop, 1);
        g.open_shop(false);
        assert_eq!(g.shop_slots(), 4);
        assert_eq!(g.shop.as_ref().unwrap().offers.len(), 4);
        assert_eq!(g.shop_reroll_index(), 4);
        assert_eq!(g.shop_go_index(), 5);
        g.shop_sel = 5;
        g.shop_move(1);
        assert_eq!(g.shop_sel, 0, "selection wraps over cards and buttons");
    }

    #[test]
    fn discount_and_recycler_change_what_shopping_costs() {
        let mut g = playing();
        let card = Offer::new(Upgrade::SteadyHands, 1);
        let full = g.offer_cost(&card);
        g.upgrades.set(Upgrade::Discount, 2);
        // Holding two Discount tiers also adds a crystal of inflation.
        assert_eq!(g.offer_cost(&card), full + 1 - 4);
        assert_eq!(g.offer_cost(&Offer::new(Upgrade::PatchKit, 1)), 4);

        g.upgrades.set(Upgrade::Recycler, 1);
        g.open_shop(false);
        g.crystals = 50;
        g.shop_reroll();
        assert_eq!(g.crystals, 50 - REROLL_BASE + Upgrade::recycler_refund(1));
    }

    #[test]
    fn brake_pads_slow_a_new_wave() {
        let mut g = with_rock("planet");
        g.rocks[0].vx = -10.0;
        let x0 = g.rocks[0].x;
        g.integrate_rocks(0.1, false);
        let plain = x0 - g.rocks[0].x;

        let mut b = with_rock("planet");
        b.upgrades.set(Upgrade::BrakePads, 1);
        b.begin_wave();
        assert_eq!(b.brake, Upgrade::brake_secs(1));
        b.rocks.clear();
        let rock = b.make_rock(60.0, 10.0, 2.0, "planet".to_string());
        b.rocks.push(rock);
        b.rocks[0].vx = -10.0;
        b.integrate_rocks(0.1, false);
        assert!(x0 - b.rocks[0].x < plain * 0.5);
        for _ in 0..80 {
            b.update(0.05);
        }
        assert_eq!(b.brake, 0.0);
    }

    #[test]
    fn clean_slate_forgives_one_slip_per_wave() {
        let mut g = with_rock("planet");
        g.upgrades.set(Upgrade::CleanSlate, 1);
        g.begin_wave();
        g.skip_ready_for_test();
        g.type_char('z');
        assert_eq!(g.miss, 0.0);
        assert!(g.banner.contains("FORGIVEN"));
        g.type_char('z');
        assert!(g.miss > 0.0, "the second slip counts");
    }

    #[test]
    fn last_stand_and_bounty_pay_out_in_their_moments() {
        let mut base = with_rock("planet");
        type_word(&mut base, "planet");

        let mut stand = with_rock("planet");
        stand.upgrades.set(Upgrade::LastStand, 1);
        stand.lives = 1;
        type_word(&mut stand, "planet");
        assert_eq!(stand.score, base.score * 2);

        let mut calm = with_rock("planet");
        calm.upgrades.set(Upgrade::LastStand, 1);
        type_word(&mut calm, "planet");
        assert_eq!(calm.score, base.score, "only at one heart");

        let mut hot = with_rock("planet");
        hot.upgrades.set(Upgrade::Bounty, 1);
        hot.combo = 4;
        type_word(&mut hot, "planet");
        let mut cold = with_rock("planet");
        cold.upgrades.set(Upgrade::Bounty, 1);
        type_word(&mut cold, "planet");
        assert_eq!(hot.gems.len(), cold.gems.len() + 1);
    }

    #[test]
    fn lucky_penny_sometimes_doubles_a_drop() {
        let mut plain = 0;
        let mut lucky = 0;
        for seed in 0..60 {
            let mut a = with_rock("star");
            a.rng = Rng(seed * 7919 + 11);
            type_word(&mut a, "star");
            plain += a.gems.len();
            let mut b = with_rock("star");
            b.rng = Rng(seed * 7919 + 11);
            b.upgrades.set(Upgrade::LuckyPenny, 2);
            type_word(&mut b, "star");
            lucky += b.gems.len();
        }
        assert!(lucky > plain, "{lucky} vs {plain}");
        assert!(lucky < plain * 2, "only some drops double");
    }

    #[test]
    fn shrapnel_shoves_close_hazards_back() {
        let mut g = with_rock("planet");
        g.upgrades.set(Upgrade::Shrapnel, 1);
        let rock = g.make_rock(64.0, 10.0, 1.8, "cat".to_string());
        g.rocks.push(rock);
        type_word(&mut g, "planet");
        let cat = g.rocks.iter().find(|r| r.word == "cat").unwrap();
        assert!(cat.x > 64.0, "pushed away from the blast at x = 60");
    }

    #[test]
    fn sweeper_clears_the_screen_on_every_twenty_fifth_typed_word() {
        let mut g = with_rock("planet");
        g.upgrades.set(Upgrade::Sweeper, 1);
        for (x, word) in [(30.0, "cat"), (40.0, "moon"), (50.0, "orbit")] {
            let rock = g.make_rock(x, 20.0, 1.8, word.to_string());
            g.rocks.push(rock);
        }
        g.typed_kills = SWEEP_EVERY - 1;
        type_word(&mut g, "planet");
        assert!(g.rocks.iter().all(|r| !r.live) || g.rocks.is_empty());
        assert_eq!(g.kills, 4);
        // Chained kills do not count toward the next sweep.
        assert_eq!(g.typed_kills, SWEEP_EVERY);
    }

    /// Type one boss word and the space after it, as a typist would. The
    /// passage's last word ends on its last letter, so it gets no space.
    fn type_boss_word(g: &mut Game, word: &str) {
        type_word(g, word);
        if g.boss.as_ref().is_some_and(|b| b.active()) {
            g.type_space();
        }
    }

    /// A game on its first boss wave with the intro over.
    fn boss_fight() -> Game {
        let mut g = playing();
        g.begin_wave_for_test(5);
        while g.boss.as_ref().is_some_and(|b| b.intro > 0.0) {
            g.update(0.05);
        }
        g
    }

    /// The first letter that is not the next letter of the current word.
    fn wrong_key(g: &Game) -> char {
        let next = g
            .target_word()
            .unwrap()
            .chars()
            .nth(g.buffer.len())
            .unwrap();
        if next == 'z' {
            'q'
        } else {
            'z'
        }
    }

    #[test]
    fn boss_math_scales_to_the_wpm_cap() {
        assert!(boss::is_boss_wave(5) && boss::is_boss_wave(10));
        assert!(!boss::is_boss_wave(0) && !boss::is_boss_wave(4));
        let wpm: Vec<u32> = (1..=7).map(boss::target_wpm).collect();
        assert_eq!(wpm, [40, 60, 80, 100, 120, 120, 120]);
        let mut k = 0;
        let b = Boss::new(2, |n| {
            k += 7;
            k % n
        });
        let pace = b.chars_total as f32 / 5.0 / (b.limit / 60.0);
        assert!((pace - 60.0).abs() < 0.01, "the clock matches the pace");
        assert!(b.words.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn a_boss_wave_is_a_typing_test_not_a_field_of_hazards() {
        let mut g = playing();
        g.begin_wave_for_test(5);
        assert!(g.boss.is_some());
        assert!(g.rocks.is_empty());
        assert_eq!(g.target_word(), None, "no word during the warning");
        g.type_char('a');
        assert!(g.buffer.is_empty(), "keys do nothing yet");
        let before = g.lives;
        g.type_char('z');
        assert_eq!(g.lives, before, "no strike during the intro");
        for _ in 0..70 {
            g.update(0.05);
        }
        assert!(g.target_word().is_some());
        for _ in 0..60 {
            g.update(0.05);
        }
        assert!(g.rocks.is_empty(), "nothing spawns during a boss");
    }

    #[test]
    fn a_typo_stays_until_fixed_and_never_costs_a_heart() {
        let mut g = boss_fight();
        g.lives = 1;
        let word = g.target_word().unwrap().to_string();
        let first = word.chars().next().unwrap();
        g.type_char(first);
        let bad = wrong_key(&g);
        g.type_char(bad);
        g.type_char(bad);
        assert_eq!(g.lives, 1, "mistakes never cost hearts");
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(
            g.buffer,
            format!("{first}{bad}{bad}"),
            "typos stay, shown red"
        );
        assert_eq!(
            g.boss.as_ref().unwrap().chars_done,
            1,
            "only clean letters count"
        );
        assert!(
            g.run_stats.accuracy() < Some(100),
            "typos still cost accuracy"
        );
        g.backspace();
        g.backspace();
        assert_eq!(g.buffer, first.to_string(), "backspace fixes them");
        g.clear_word();
        assert!(g.buffer.is_empty(), "Ctrl+W clears the word");
        assert_eq!(g.boss.as_ref().unwrap().chars_done, 0);
        for _ in 0..20 {
            g.type_char(bad);
        }
        assert_eq!(
            g.buffer.len(),
            word.len() + boss::MAX_EXTRA,
            "extras are capped"
        );
    }

    #[test]
    fn space_moves_on_only_when_the_word_is_spelled_right() {
        let mut g = boss_fight();
        let words = g.boss.as_ref().unwrap().words.clone();
        g.type_space();
        assert_eq!(
            g.boss.as_ref().unwrap().idx,
            0,
            "a leading space is ignored"
        );
        assert_eq!(g.miss, 0.0);

        // Half a word, or a word with a typo, cannot be skipped.
        g.type_char(words[0].chars().next().unwrap());
        g.type_space();
        assert_eq!(g.boss.as_ref().unwrap().idx, 0);
        assert!(g.miss > 0.0 && g.last_wrong == Some(' '));
        g.clear_word();
        type_word(&mut g, words[0]);
        assert_eq!(
            g.boss.as_ref().unwrap().idx,
            0,
            "a full word waits for space"
        );
        g.type_space();
        let b = g.boss.as_ref().unwrap();
        assert_eq!(b.idx, 1);
        assert_eq!(b.chars_done, words[0].len() + 1, "the space counts");
        assert!(g.buffer.is_empty());
    }

    #[test]
    fn finishing_the_passage_beats_the_boss_and_opens_the_shop() {
        let mut g = boss_fight();
        g.lives = 2;
        let before = g.crystals;
        let words = g.boss.as_ref().unwrap().words.clone();
        for word in words {
            assert_eq!(g.target_word(), Some(word));
            type_boss_word(&mut g, word);
        }
        let b = g.boss.as_ref().unwrap();
        assert_eq!(b.outcome, Some(true));
        assert_eq!(b.progress(), 1.0);
        assert_eq!(g.crystals, before + 20);
        assert_eq!(g.lives, 3, "a win heals one heart");
        assert!(g.banner.contains("BOSS DOWN"));
        for _ in 0..80 {
            g.update(0.05);
        }
        assert_eq!(g.phase, Phase::Shop);
        assert!(g.boss.is_none());
        assert_eq!(g.wave, 6);
    }

    #[test]
    fn hovering_selects_only_what_the_screen_shows() {
        let mut g = Game::new(100, 40, 3);
        assert!(g.hover(Hit::Menu(2)));
        assert_eq!(g.menu, 2);
        assert!(!g.hover(Hit::Menu(99)));
        assert!(!g.hover(Hit::Theme(1)), "theme picker is closed");
        g.open_themes();
        assert!(!g.hover(Hit::Menu(0)), "menu sleeps under the theme picker");
        assert!(g.hover(Hit::Theme(1)));
        assert_eq!(g.theme_choice, 1);

        let mut shop = playing();
        shop.upgrades.set(Upgrade::Focus, 1);
        shop.open_shop_for_test();
        assert!(shop.hover(Hit::Shop(shop.shop_go_index())));
        assert_eq!(shop.shop_sel, shop.shop_go_index());
        assert!(
            !shop.hover(Hit::Rack(0)),
            "rows belong to the upgrades screen"
        );
        assert!(!shop.hover(Hit::Sell));
        shop.shop_toggle_rack();
        assert!(shop.hover(Hit::Rack(0)) && shop.hover(Hit::Sell));
        assert!(!shop.hover(Hit::Shop(0)), "the shelf is hidden");
    }

    #[test]
    fn running_out_of_time_against_a_boss_ends_the_run() {
        let mut g = boss_fight();
        g.lives = 5;
        g.shield = 2;
        g.upgrades.set(Upgrade::SecondWind, 1);
        let limit = g.boss.as_ref().unwrap().limit;
        for _ in 0..((limit / 0.05) as usize + 5) {
            g.update(0.05);
        }
        assert_eq!(g.boss.as_ref().unwrap().outcome, Some(false));
        assert_eq!(
            g.phase,
            Phase::Dying,
            "no heart, ward or second wind saves it"
        );
        assert_eq!(g.lives, 0);
        assert_eq!(g.boss_escaped, Some(40));
        for _ in 0..80 {
            g.update(0.05);
        }
        assert!(matches!(g.phase, Phase::GameOver | Phase::Initials));
        assert_eq!(g.crystals, 0, "no reward for losing");
        g.start();
        assert_eq!(g.boss_escaped, None, "a new run forgets it");
    }

    #[test]
    fn the_shark_swims_laps_on_screen_and_the_ship_tracks_it() {
        let mut g = playing();
        g.theme = Theme::Underwater;
        g.begin_wave_for_test(5);
        let home = g.boss_home();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let mut headings = Vec::new();
        for _ in 0..400 {
            g.update(0.05);
            let (x, _) = g.boss_pos();
            lo = lo.min(x);
            hi = hi.max(x);
            headings.push(boss::heading(g.boss.as_ref().unwrap().age));
        }
        assert!(lo < home.0 - 5.0 && hi > home.0 + 5.0, "{lo}..{hi}");
        assert!(lo > 0.0 && hi < g.play_w, "{lo}..{hi}");
        assert!(headings.iter().any(|&h| h > 0.9) && headings.iter().any(|&h| h < -0.9));
        // The ship keeps its nose on the shark as it swims.
        let (sx, sy) = g.ship_pos();
        let (bx, by) = g.boss_pos();
        let want = (bx - sx).atan2(-(by - sy));
        assert!(angle_delta(g.angle, want).abs() < 0.2);
    }

    #[test]
    fn the_boss_clock_stops_while_paused_and_typing_sets_the_pace() {
        let mut g = boss_fight();
        let t = g.boss.as_ref().unwrap().elapsed;
        g.phase = Phase::Paused;
        g.update(0.0);
        assert_eq!(g.boss.as_ref().unwrap().elapsed, t);
        g.phase = Phase::Playing;
        let word = g.target_word().unwrap().to_string();
        type_boss_word(&mut g, &word);
        for _ in 0..25 {
            g.update(0.05);
        }
        assert!(g.boss.as_ref().unwrap().live_wpm() > 0);
    }

    /// A kill target plus short and long bystanders, for Multishot tests.
    fn crowded() -> Game {
        let mut g = with_rock("planet");
        for (x, y, word) in [
            (30.0, 12.0, "cat"),
            (35.0, 6.0, "moon"),
            (20.0, 8.0, "orbit"),
        ] {
            let rock = g.make_rock(x, y, 1.8, word.to_string());
            g.rocks.push(rock);
        }
        g
    }

    fn remaining(g: &Game) -> Vec<&str> {
        let mut words: Vec<&str> = g.rocks.iter().map(|r| r.word.as_str()).collect();
        words.sort();
        words
    }

    #[test]
    fn multishot_pops_one_short_word_and_grows_with_levels() {
        // Level 1: only the nearest word of 4 letters or fewer ("moon").
        let mut g = crowded();
        g.upgrades.set(Upgrade::Multishot, 1);
        type_word(&mut g, "planet");
        assert_eq!(remaining(&g), ["cat", "orbit"]);

        // Level 1 ignores a five-letter word; level 2 takes it.
        for (level, expect_left) in [(1u8, 1usize), (2, 0)] {
            let mut g = with_rock("planet");
            let rock = g.make_rock(30.0, 12.0, 1.8, "orbit".to_string());
            g.rocks.push(rock);
            g.upgrades.set(Upgrade::Multishot, level);
            type_word(&mut g, "planet");
            assert_eq!(g.rocks.len(), expect_left, "level {level}");
        }
    }

    #[test]
    fn scattershot_adds_targets_and_needs_multishot() {
        let mut g = crowded();
        g.upgrades.set(Upgrade::Multishot, 1);
        g.upgrades.set(Upgrade::Scattershot, 1);
        type_word(&mut g, "planet");
        assert_eq!(remaining(&g), ["orbit"], "two short words popped");

        let mut g = playing();
        assert!(!g.eligible(Upgrade::Scattershot));
        g.upgrades.set(Upgrade::Multishot, 1);
        assert!(g.eligible(Upgrade::Scattershot));
    }

    #[test]
    fn focus_slows_only_the_typed_hazard_and_more_per_level() {
        let slow = |level: u8| -> f32 {
            let mut g = with_rock("planet");
            let other = g.make_rock(40.0, 10.0, 1.8, "moon".to_string());
            g.rocks.push(other);
            for r in &mut g.rocks {
                r.vx = 4.0;
                r.vy = 0.0;
            }
            g.upgrades.set(Upgrade::Focus, level);
            g.type_char('p');
            let (x0, ox0) = (g.rocks[0].x, g.rocks[1].x);
            g.integrate_rocks(0.5, true);
            let moved = g.rocks[0].x - x0;
            assert!((g.rocks[1].x - ox0 - 2.0).abs() < 1e-3, "others unaffected");
            moved
        };
        let (none, one, two, four) = (slow(0), slow(1), slow(2), slow(4));
        assert!(
            one < none && two < one && four < two,
            "{none} {one} {two} {four}"
        );
    }

    #[test]
    fn ward_absorbs_one_hit_per_wave() {
        let mut g = playing();
        g.upgrades.set(Upgrade::Ward, 1);
        g.begin_wave();
        assert_eq!(g.shield, 1);
        let (sx, sy) = g.ship_pos();
        let mut rock = g.make_rock(sx, sy, 2.0, "zzz".to_string());
        rock.vx = 0.0;
        rock.vy = 0.0;
        let id = rock.id;
        g.rocks = vec![rock];
        g.collide(id);
        assert_eq!(g.lives, BASE_HEARTS, "shield soaked the hit");
        assert_eq!(g.shield, 0);
    }

    #[test]
    fn second_wind_survives_one_fatal_hit() {
        let mut g = playing();
        g.upgrades.set(Upgrade::SecondWind, 1);
        g.lives = 1;
        let (sx, sy) = g.ship_pos();
        let rock = g.make_rock(sx, sy, 2.0, "zzz".to_string());
        let id = rock.id;
        g.rocks = vec![rock];
        g.collide(id);
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.lives, 1);
        g.invuln = 0.0;
        let rock = g.make_rock(sx, sy, 2.0, "yyy".to_string());
        let id = rock.id;
        g.rocks = vec![rock];
        g.collide(id);
        assert_eq!(g.phase, Phase::Dying, "only once per run");
    }

    #[test]
    fn a_saved_run_resumes_in_the_shop() {
        let mut g = playing();
        g.wave = 4;
        g.score = 900;
        g.crystals = 12;
        g.lives = 2;
        g.upgrades.set(Upgrade::Leech, 1);
        let run = g.snapshot(true);
        let mut fresh = Game::new(100, 40, 7);
        fresh.load_run(run);
        assert_eq!(fresh.phase, Phase::Shop);
        assert_eq!(fresh.wave, 4);
        assert_eq!(fresh.score, 900);
        assert_eq!(fresh.crystals, 12);
        assert_eq!(fresh.lives, 2);
        assert!(fresh.upgrades.has(Upgrade::Leech));
    }

    #[test]
    fn starting_a_new_run_drops_old_upgrades() {
        let mut g = playing();
        g.upgrades.set(Upgrade::Prospector, 3);
        g.crystals = 99;
        g.start();
        assert_eq!(g.crystals, 0);
        assert!(!g.upgrades.has(Upgrade::Prospector));
    }

    #[test]
    fn pause_menu_defaults_to_continue() {
        let mut g = playing();
        g.pause();
        assert_eq!(g.phase, Phase::Paused);
        assert_eq!(g.menu, PAUSE_CONTINUE);
        assert_eq!(g.menu_items(), PAUSE_ITEMS);
        g.menu_move(1);
        assert_eq!(g.menu, PAUSE_RESTART);
        g.menu_move(1);
        assert_eq!(g.menu, PAUSE_QUIT);
        g.menu_move(1);
        assert_eq!(g.menu, PAUSE_CONTINUE);
        g.resume();
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn returning_to_title_resets_the_run() {
        let mut g = playing();
        g.score = 1234;
        g.wave = 3;
        g.lives = 1;
        g.buffer = "ab".into();
        g.return_to_title();
        assert_eq!(g.phase, Phase::Title);
        assert_eq!(g.menu, MENU_START);
        assert!(!g.quit);
        assert_eq!(g.score, 0);
        assert_eq!(g.wave, 1);
        assert_eq!(g.lives, 3);
        assert!(g.buffer.is_empty());
        assert!(!g.rocks.is_empty());
    }

    #[test]
    fn game_over_menu_defaults_to_play_again() {
        let mut g = playing();
        g.phase = Phase::Dying;
        g.dying = 0.01;
        g.update(0.05);
        assert_eq!(g.phase, Phase::GameOver);
        assert_eq!(g.menu, OVER_AGAIN);
        assert_eq!(g.menu_items(), OVER_ITEMS);
        g.menu_move(-1);
        assert_eq!(g.menu, OVER_QUIT);
        g.menu_move(-1);
        assert_eq!(g.menu, OVER_THEMES);
        g.menu_move(-1);
        assert_eq!(g.menu, OVER_AGAIN);
    }

    #[test]
    fn closing_themes_from_game_over_resets_to_play_again() {
        let mut g = playing();
        g.phase = Phase::GameOver;
        g.menu = OVER_THEMES;
        g.open_themes();
        g.close_themes();
        assert!(!g.theme_menu_open);
        assert_eq!(g.menu, OVER_AGAIN);
    }

    #[test]
    fn theme_selection_applies_and_closes() {
        let mut g = Game::new(100, 40, 42);
        assert_eq!(g.theme, Theme::Underwater);
        g.open_themes();
        assert!(g.theme_menu_open);
        assert_eq!(g.theme_choice, Theme::Underwater.index());

        g.theme_move(1);
        assert_eq!(g.theme_choice, Theme::Jungle.index());
        g.apply_theme();
        assert_eq!(g.theme, Theme::Jungle);
        assert!(!g.theme_menu_open);
        assert!(g.stars.iter().any(|s| s.glyph == '✦'));

        g.open_themes();
        assert_eq!(g.theme_choice, Theme::Jungle.index());
        g.theme_move(1);
        assert_eq!(g.theme_choice, Theme::CatCafe.index());
        g.apply_theme();
        assert_eq!(g.theme, Theme::CatCafe);
        assert!(!g.theme_menu_open);
        assert!(g.stars.iter().any(|s| matches!(s.glyph, '~' | '·' | '✧')));

        g.open_themes();
        assert_eq!(g.theme_choice, Theme::CatCafe.index());
        g.theme_move(1);
        assert_eq!(g.theme_choice, Theme::Space.index());
        g.apply_theme();
        assert_eq!(g.theme, Theme::Space);
        assert!(g.stars.iter().any(|s| s.glyph == '+'));

        g.open_themes();
        g.theme_move(1);
        assert_eq!(g.theme_choice, Theme::Underwater.index());
        g.apply_theme();
        assert_eq!(g.theme, Theme::Underwater);
        assert!(g.stars.iter().any(|s| matches!(s.glyph, '°' | '∘')));
    }

    #[test]
    fn closing_themes_keeps_the_active_theme() {
        let mut g = Game::new(100, 40, 42);
        g.open_themes();
        g.theme_move(1);
        g.close_themes();
        assert!(!g.theme_menu_open);
        assert_eq!(g.theme, Theme::Underwater);
    }

    #[test]
    fn shortest_turn_wraps() {
        let d = angle_delta(0.1, TAU - 0.1);
        assert!(d < 0.0, "{d}");
        assert!((d.abs() - 0.2).abs() < 0.02, "{d}");
    }

    #[test]
    fn words_are_typeable() {
        for w in words::SHORT.iter().chain(words::MID).chain(words::LONG) {
            assert!(w.chars().all(|c| c.is_ascii_lowercase()), "{w}");
            assert!(w.len() >= 3 && w.len() <= 8, "{w}");
        }
    }

    #[test]
    fn typing_word_blows_it_up() {
        let mut g = playing();
        let rock = g.make_rock(78.0, 6.0, 2.2, "orbit".into());
        g.rocks.push(rock);
        for ch in ['o', 'r', 'b', 'i', 't'] {
            g.type_char(ch);
        }
        assert!(g.rocks.is_empty(), "rock should be gone");
        assert!(g.score >= 50 * 5);
        assert!(g.buffer.is_empty());
        assert!(g.effects.len() > 8);
    }

    #[test]
    fn wrong_letter_is_rejected() {
        let mut g = playing();
        let rock = g.make_rock(78.0, 6.0, 2.2, "orbit".into());
        g.rocks.push(rock);
        g.type_char('z');
        assert_eq!(g.buffer, "");
        assert!(g.miss > 0.0);
        assert_eq!(g.rocks.len(), 1);
    }

    #[test]
    fn backspace_unlocks_letters() {
        let mut g = playing();
        let rock = g.make_rock(78.0, 6.0, 2.2, "orbit".into());
        g.rocks.push(rock);
        g.type_char('o');
        g.type_char('r');
        g.backspace();
        assert_eq!(g.buffer, "o");
        assert!(g.target.is_some());
    }

    #[test]
    fn ship_turns_toward_the_rock_being_typed() {
        let mut g = playing();
        let (cx, cy) = g.ship_center();
        let rock = g.make_rock(cx + 28.0, cy, 2.0, "cat".into());
        g.rocks.push(rock);
        g.type_char('c');
        for _ in 0..90 {
            g.update(1.0 / 60.0);
        }
        let err = angle_delta(g.angle, PI / 2.0).abs();
        assert!(err < 0.25, "angle {} err {}", g.angle, err);
    }

    #[test]
    fn hull_contact_costs_a_life() {
        let mut g = playing();
        let (cx, cy) = g.ship_center();
        let rock = g.make_rock(cx, cy, 2.4, "cat".into());
        g.rocks.push(rock);
        g.update(1.0 / 60.0);
        assert_eq!(g.lives, 2);
        assert!(g.invuln > 0.0);
    }

    #[test]
    fn invulnerability_does_not_destroy_rocks_for_free() {
        let mut g = playing();
        g.invuln = 1.0;
        g.buffer = "c".into();
        let (cx, cy) = g.ship_center();
        let rock = g.make_rock(cx, cy, 2.4, "cat".into());
        let id = rock.id;
        g.rocks.push(rock);
        g.target = Some(id);
        g.update(1.0 / 60.0);
        assert_eq!(g.lives, 3);
        assert_eq!(g.rocks.len(), 1);
        assert_eq!(g.buffer, "c");
        assert_eq!(g.target, Some(id));
    }

    // --- UX ----------------------------------------------------------------

    #[test]
    fn new_screens_ignore_menu_keys_for_a_moment() {
        let mut g = playing();
        g.open_shop(false);
        assert!(
            !g.accepts_menu_keys(),
            "letters still being typed are ignored"
        );
        for _ in 0..14 {
            g.update(0.05);
        }
        assert!(g.accepts_menu_keys());

        let mut d = with_rock("planet");
        d.lives = 1;
        d.lose_life(1.0);
        while d.phase == Phase::Dying {
            d.update(0.05);
        }
        assert_eq!(d.phase, Phase::GameOver, "zero score skips initials");
        assert!(!d.accepts_menu_keys());
    }

    #[test]
    fn esc_in_the_shop_pauses_and_returns_to_the_shop() {
        let mut g = playing();
        g.open_shop(false);
        g.pause();
        assert_eq!(g.phase, Phase::Paused);
        assert!(g.in_shop(), "the shop stays under the pause menu");
        assert!(g.snapshot(g.in_shop()).in_shop);
        g.resume();
        assert_eq!(g.phase, Phase::Shop);
    }

    #[test]
    fn ctrl_w_drops_the_whole_word() {
        let mut g = with_rock("planet");
        type_word(&mut g, "pla");
        g.clear_word();
        assert!(g.buffer.is_empty() && g.buffer_t.is_empty());
        assert_eq!(g.target, None);
    }

    #[test]
    fn a_miss_remembers_the_key_and_what_was_wanted() {
        let mut g = with_rock("planet");
        g.type_char('p');
        g.type_char('z');
        assert_eq!(g.last_wrong, Some('z'));
        assert_eq!(g.expected_letter(), Some('l'));
        assert_eq!(g.wave_stats.keys_bad, 1);
        assert_eq!(g.wave_stats.accuracy(), Some(50));
    }

    #[test]
    fn typing_speed_counts_only_time_spent_typing() {
        let mut s = Stats::default();
        assert_eq!(s.wpm(), None);
        s.keys_ok = 50;
        s.typing = 10.0;
        assert_eq!(s.wpm(), Some(60));
        assert!(s.summary().contains("60 WPM"));
    }

    #[test]
    fn still_screens_redraw_less_often() {
        let mut g = playing();
        let play = g.frame_budget();
        g.open_shop(false);
        assert!(g.frame_budget() > play * 3);
    }

    // --- arcade ------------------------------------------------------------

    #[test]
    fn waves_open_with_a_call_out_and_a_ready_beat() {
        let mut g = playing();
        g.begin_wave_for_test(3);
        assert_eq!(g.active_callout().unwrap().text, "WAVE 3");
        assert!(g.rocks.is_empty(), "nothing arrives during the beat");
        g.update(0.05);
        for _ in 0..19 {
            g.update(0.05);
        }
        assert_eq!(g.active_callout().unwrap().text, "GO!");
        for _ in 0..5 {
            g.update(0.05);
        }
        assert_eq!(g.rocks.len(), 3, "then the first hazards arrive together");
    }

    #[test]
    fn the_score_rolls_like_an_odometer() {
        let mut g = playing();
        g.score = 1000;
        g.update(1.0 / 60.0);
        assert!(
            g.shown_score > 0 && g.shown_score < 1000,
            "{}",
            g.shown_score
        );
        assert!(g.score_rolling());
        for _ in 0..120 {
            g.update(1.0 / 60.0);
        }
        assert_eq!(g.shown_score, 1000);
    }

    #[test]
    fn a_high_score_asks_for_initials_then_files_them() {
        let mut g = with_rock("planet");
        g.score = 4321;
        g.lives = 1;
        g.lose_life(1.0);
        for _ in 0..40 {
            g.update(0.05);
        }
        assert_eq!(g.phase, Phase::Initials);
        for c in "abcd".chars() {
            g.type_char(c);
        }
        assert_eq!(g.initials, "ABC", "three letters at most");
        g.backspace();
        g.type_char('x');
        g.confirm_initials();
        assert_eq!(g.phase, Phase::GameOver);
        let top = &g.scores.entries[0];
        assert_eq!((top.initials.as_str(), top.score), ("ABX", 4321));
    }

    #[test]
    fn the_combo_announcer_shouts_at_milestones() {
        let mut g = with_rock("planet");
        g.combo = 9;
        type_word(&mut g, "planet");
        assert_eq!(g.announce.as_ref().unwrap().text, "GREAT!");
        let mut quiet = with_rock("planet");
        quiet.combo = 3;
        type_word(&mut quiet, "planet");
        assert!(quiet.announce.is_none());
    }

    #[test]
    fn gameplay_and_menus_queue_sound_effects() {
        use crate::audio::Sfx;
        let mut g = with_rock("star");
        g.sounds.clear();
        type_word(&mut g, "star");
        let has = |g: &Game, sfx: Sfx| g.sounds.iter().any(|(s, _)| *s == sfx);
        assert!(
            has(&g, Sfx::Fire(0)) && has(&g, Sfx::Fire(3)),
            "each letter fires"
        );
        assert!(has(&g, Sfx::Break { big: false }), "the hazard breaks");
        assert!(has(&g, Sfx::Perfect), "no mistakes: perfect");

        let mut sloppy = with_rock("star");
        sloppy.type_char('z');
        sloppy.sounds.clear();
        type_word(&mut sloppy, "star");
        assert!(!has(&sloppy, Sfx::Perfect), "a typo loses the perfect");

        let mut menu = Game::new(100, 40, 1);
        menu.menu_move(1);
        assert!(has(&menu, Sfx::Hover));
        menu.sounds.clear();
        menu.hover(Hit::Menu(menu.menu));
        assert!(menu.sounds.is_empty(), "no tick when nothing moved");

        let mut shop = playing();
        shop.open_shop_for_test();
        shop.crystals = 0;
        shop.shop_buy(0);
        assert!(has(&shop, Sfx::Deny));
    }

    #[test]
    fn options_flip_and_cycle() {
        let mut g = Game::new(100, 40, 1);
        g.open_options();
        g.options_activate();
        assert!(!g.settings.scanlines);
        g.options_move(1);
        g.options_activate();
        assert_eq!(g.option_value(1), "LOW");
        g.options_move(2);
        assert_eq!(OPTION_ITEMS[g.options_sel], "SOUND");
        g.options_activate();
        assert_eq!(g.option_value(3), "LOW");
        g.options_move(1);
        g.options_activate();
        assert!(!g.options_open, "BACK closes the panel");
    }

    #[test]
    fn the_title_shows_scores_when_left_alone() {
        let mut g = Game::new(100, 40, 1);
        assert!(!g.attract_table());
        for _ in 0..200 {
            g.update(0.05);
        }
        assert!(g.attract_table());
        g.note_input();
        assert!(!g.attract_table(), "a key brings the menu back");
    }

    #[test]
    fn the_daily_run_deals_everyone_the_same_words() {
        let first_words = |seed: u64| {
            let mut g = Game::new(100, 40, seed);
            g.set_day(20_761);
            g.start_daily();
            for _ in 0..40 {
                g.update(0.05);
            }
            assert_eq!(g.daily, Some(20_761));
            assert_eq!(g.heat, 0);
            let mut words: Vec<String> = g.rocks.iter().map(|r| r.word.clone()).collect();
            words.sort();
            (words, g.theme)
        };
        let (a, theme) = first_words(1);
        assert!(!a.is_empty());
        assert_eq!(first_words(999), (a, theme));
    }

    // --- balance -----------------------------------------------------------

    #[test]
    fn motherlode_pays_at_most_one_extra_crystal() {
        let mut base = with_rock("star");
        type_word(&mut base, "star");
        let mut rich = with_rock("star");
        rich.upgrades.set(Upgrade::Prospector, 5);
        type_word(&mut rich, "star");
        assert_eq!(rich.gems.len(), base.gems.len() + 1);
    }

    #[test]
    fn chained_kills_drop_a_single_crystal() {
        let mut g = with_rock("planet");
        g.upgrades.set(Upgrade::Multishot, 1);
        g.upgrades.set(Upgrade::Prospector, 5);
        let rock = g.make_rock(30.0, 20.0, 1.8, "cat".to_string());
        g.rocks.push(rock);
        type_word(&mut g, "planet");
        // planet: 1 + 1 (six letters) + 1 (Motherlode); cat chained: 1.
        assert_eq!(g.gems.len(), 4);
    }

    #[test]
    fn prices_inflate_with_everything_held() {
        let mut g = playing();
        let card = Offer::new(Upgrade::SteadyHands, 1);
        let before = g.offer_cost(&card);
        g.upgrades.set(Upgrade::SlowTide, 4);
        assert_eq!(g.offer_cost(&card), before + 2);
        let kit = Offer::new(Upgrade::PatchKit, 1);
        assert_eq!(g.offer_cost(&kit), 8, "patch kits stay flat");
    }

    #[test]
    fn a_flawless_wave_pays_a_bonus() {
        let clear = |hurt: bool| {
            let mut g = playing();
            g.wave = 3;
            g.hurt_this_wave = hurt;
            g.update(0.05);
            assert!(g.pending_clear);
            g.crystals
        };
        assert_eq!(clear(false), 3);
        assert_eq!(clear(true), 0);
    }

    #[test]
    fn the_rack_limits_lines_and_selling_frees_a_slot() {
        let mut g = playing();
        for up in [
            Upgrade::SlowTide,
            Upgrade::Focus,
            Upgrade::Ward,
            Upgrade::Haggler,
            Upgrade::LongFuse,
            Upgrade::Bounty,
        ] {
            g.upgrades.set(up, 1);
        }
        g.open_shop(false);
        g.crystals = 500;
        g.shop.as_mut().unwrap().offers[0] = Offer::new(Upgrade::Leech, 1);
        g.shop.as_mut().unwrap().offers[1] = Offer::new(Upgrade::SlowTide, 2);
        g.shop_buy(0);
        assert!(!g.upgrades.has(Upgrade::Leech), "rack is full");
        assert!(g.shop_note.contains("Rack full"));
        g.shop_buy(1);
        assert_eq!(
            g.upgrades.level(Upgrade::SlowTide),
            2,
            "upgrades need no slot"
        );

        g.shop_toggle_rack();
        assert!(g.rack_focus);
        g.rack_sel = 0;
        let (up, level) = g.rack_selected().unwrap();
        let before = g.crystals;
        g.shop_sell();
        assert_eq!(g.crystals, before + up.sell_value(level));
        assert!(!g.upgrades.has(up));
        g.rack_focus = false;
        g.shop_buy(0);
        assert!(g.upgrades.has(Upgrade::Leech), "the freed slot takes it");
        assert_eq!(g.rack_size(), 6);
        g.rack_bonus = 9;
        assert_eq!(g.rack_size(), RACK_MAX_FOR_TEST);
    }
    const RACK_MAX_FOR_TEST: u32 = rack::RACK_MAX;

    #[test]
    fn slowdowns_share_a_floor() {
        let mut g = playing();
        g.upgrades.set(Upgrade::SlowTide, 5);
        g.upgrades.set(Upgrade::LastStand, 1);
        g.lives = 1;
        assert!((g.hazard_pace() - 0.5).abs() < 1e-4);
        g.freeze = 1.0;
        assert_eq!(g.hazard_pace(), 0.0);
    }

    #[test]
    fn hazards_level_off_instead_of_racing_forever() {
        assert!(hazard_speed(20) < 10.0);
        assert!(hazard_speed(10) > hazard_speed(5));
    }

    #[test]
    fn splitters_break_into_two_short_words() {
        let mut g = with_rock("planet");
        g.rocks[0].kind = Kind::Splitter;
        type_word(&mut g, "planet");
        assert_eq!(g.rocks.len(), 2);
        assert_ne!(g.rocks[0].word, g.rocks[1].word);
        assert!(g
            .rocks
            .iter()
            .all(|r| r.word.len() <= 4 && r.kind == Kind::Normal));
    }

    #[test]
    fn armored_hazards_take_their_word_twice() {
        let mut g = with_rock("planet");
        g.rocks[0].kind = Kind::Armored;
        g.rocks[0].armor = 1;
        type_word(&mut g, "planet");
        assert_eq!(g.rocks.len(), 1, "the first completion only cracks it");
        assert_eq!(g.rocks[0].armor, 0);
        assert!(g.buffer.is_empty());
        type_word(&mut g, "planet");
        assert!(g.rocks.is_empty());
    }

    #[test]
    fn runners_outpace_the_wave() {
        let mut g = playing();
        g.wave = 8;
        let mut speeds = (Vec::new(), Vec::new());
        for _ in 0..300 {
            g.rocks.clear();
            g.spawn_threat();
            let r = &g.rocks[0];
            let v = (r.vx * r.vx + r.vy * r.vy).sqrt();
            match r.kind {
                Kind::Runner => speeds.0.push(v),
                Kind::Normal => speeds.1.push(v),
                _ => {}
            }
        }
        let avg = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(!speeds.0.is_empty(), "runners appear by wave 8");
        assert!(avg(&speeds.0) > avg(&speeds.1) * 1.5);
    }

    #[test]
    fn heat_rules_stack() {
        let mut g = playing();
        g.heat = 2;
        assert_eq!(g.shop_slots(), SHOP_SLOTS - 1);
        assert!((g.hazard_pace() - arcade::HEAT_SPEED).abs() < 1e-4);
        g.heat = 3;
        g.combo = 4;
        let mut r = with_rock("planet");
        r.heat = 3;
        r.combo = 4;
        r.type_char('z');
        assert_eq!(r.combo, 0, "a miss breaks the combo at heat 3");
        g.heat = 5;
        g.settings.heat_unlocked = 5;
        g.settings.heat = 5;
        g.start();
        assert_eq!(g.lives, 2);
        g.heat = 4;
        g.begin_wave_for_test(5);
        assert_eq!(g.boss.as_ref().unwrap().wpm, 50);
    }

    #[test]
    fn heat_is_picked_only_up_to_what_is_unlocked() {
        let mut g = Game::new(100, 40, 1);
        g.heat_move(1);
        assert_eq!(g.settings.heat, 0);
        g.settings.heat_unlocked = 2;
        for _ in 0..5 {
            g.heat_move(1);
        }
        assert_eq!(g.settings.heat, 2);
    }

    /// Type a whole boss passage.
    fn beat_boss(g: &mut Game) {
        while g.boss.as_ref().is_some_and(|b| b.intro > 0.0) {
            g.update(0.05);
        }
        while g.boss.as_ref().is_some_and(|b| b.active()) {
            let word = g.target_word().unwrap().to_string();
            type_boss_word(g, &word);
        }
    }

    #[test]
    fn beating_the_fourth_boss_wins_the_run() {
        let mut g = playing();
        g.score = 5000;
        g.begin_wave_for_test(20);
        beat_boss(&mut g);
        for _ in 0..80 {
            g.update(0.05);
        }
        assert_eq!(g.phase, Phase::Victory);
        assert!(g.won);
        assert_eq!(g.settings.heat_unlocked, 1, "winning unlocks heat 1");
        assert_eq!(g.rack_bonus, 1);

        let mut endless = g.clone_for_test();
        endless.input_lock = 0.0;
        endless.menu = arcade::VICTORY_ENDLESS;
        endless.victory_choose();
        assert_eq!(endless.phase, Phase::Shop);
        assert_eq!(endless.wave, 21);

        g.menu = arcade::VICTORY_FINISH;
        g.victory_choose();
        assert_eq!(g.phase, Phase::Initials);
        g.type_char('w');
        g.confirm_initials();
        assert!(g.scores.entries[0].won);
    }

    // --- fun ---------------------------------------------------------------

    #[test]
    fn hit_stop_freezes_hazards_for_a_beat() {
        let mut g = with_rock("planet");
        let rock = g.make_rock(30.0, 20.0, 1.8, "cat".to_string());
        g.rocks.push(rock);
        g.rocks[1].vx = 5.0;
        type_word(&mut g, "planet");
        assert!(g.hitstop > 0.0);
        let x = g.rocks[0].x;
        g.update(0.02);
        assert_eq!(g.rocks[0].x, x, "frozen during hit-stop");
        for _ in 0..5 {
            g.update(0.02);
        }
        assert!(g.rocks[0].x > x);
    }

    #[test]
    fn overdrive_charges_on_clean_letters_and_slows_the_field() {
        let mut g = with_rock("planet");
        g.type_char('p');
        assert_eq!(g.drive, 1.0);
        g.type_char('z');
        assert_eq!(g.drive, 0.0, "a slip drains it");
        assert!(!g.activate_overdrive(), "not charged yet");
        g.drive = DRIVE_FULL_FOR_TEST;
        assert!(g.activate_overdrive());
        assert_eq!(g.drive, 0.0);
        assert!((g.hazard_pace() - fun::OVERDRIVE_PACE).abs() < 1e-4);
        let before = g.combo;
        g.clear_word();
        type_word(&mut g, "planet");
        assert_eq!(g.combo, before + 2, "overdrive doubles the combo");
        for _ in 0..200 {
            g.update(0.05);
        }
        assert_eq!(g.overdrive, 0.0);
    }
    const DRIVE_FULL_FOR_TEST: f32 = fun::DRIVE_FULL;

    #[test]
    fn overdrive_holds_the_boss_clock() {
        let mut g = boss_fight();
        g.drive = DRIVE_FULL_FOR_TEST;
        assert!(g.activate_overdrive());
        let t = g.boss.as_ref().unwrap().elapsed;
        g.update(0.5);
        g.update(0.05);
        assert_eq!(g.boss.as_ref().unwrap().elapsed, t);
    }

    #[test]
    fn golden_hazards_grant_one_power_up() {
        let mut g = with_rock("planet");
        g.rocks[0].kind = Kind::Golden;
        type_word(&mut g, "planet");
        let pick = g.last_pickup.expect("a pickup");
        assert!(g.callouts.iter().any(|c| c.text == pick.callout()));

        let mut f = with_rock("planet");
        f.freeze = 1.0;
        f.rocks[0].vx = 5.0;
        let x = f.rocks[0].x;
        f.update(0.05);
        assert_eq!(f.rocks[0].x, x, "freeze stops hazards");
    }

    #[test]
    fn linked_words_score_more() {
        let run = |second: &str| {
            let mut g = with_rock("echo");
            type_word(&mut g, "echo");
            let rock = g.make_rock(50.0, 10.0, 2.0, second.to_string());
            g.rocks.push(rock);
            assert_eq!(g.extends_link(second), second.starts_with('o'));
            type_word(&mut g, second);
            (g.score, g.link)
        };
        let (linked, link) = run("orbit");
        let (plain, none) = run("argon");
        assert_eq!((link, none), (1, 0));
        assert!(linked > plain);
    }

    // --- balance bot ---------------------------------------------------------

    /// A bot that plays a whole run: it types `cps` correct letters per
    /// second at the nearest hazard (or the boss passage), fires Overdrive
    /// when it can, and buys the cheapest card it can afford. Returns, per
    /// shop visit, (wave, tiers held, crystals left), and the wave it ended on.
    fn bot_run(seed: u64, cps: f32, stop_wave: u32) -> (Vec<(u32, u32, u32)>, u32) {
        let mut g = Game::new(100, 40, seed);
        g.start();
        let mut log = Vec::new();
        let mut clock = 0.0;
        let dt = 1.0 / 60.0;
        for _ in 0..(60 * 60 * 60) {
            match g.phase {
                Phase::Playing => {
                    g.update(dt);
                    if g.drive_ready() {
                        g.activate_overdrive();
                    }
                    clock += dt;
                    while clock >= 1.0 / cps {
                        clock -= 1.0 / cps;
                        let next = match g.target_word() {
                            Some(w) => w.chars().nth(g.buffer.len()),
                            None => g.nearest_rock().and_then(|r| r.word.chars().next()),
                        };
                        if let Some(c) = next {
                            g.type_char(c);
                        } else if g.target_word().is_some_and(|w| w == g.buffer) {
                            // A finished boss word moves on with a space.
                            g.type_space();
                        }
                    }
                }
                Phase::Shop => {
                    if std::env::var("BOT_TRACE").is_ok() {
                        eprintln!(
                            "  wave {} lives {} crystals {} tiers {} rocks-cap {}",
                            g.wave,
                            g.lives,
                            g.crystals,
                            g.upgrades.total_levels(),
                            g.rack_used()
                        );
                    }
                    log.push((g.wave, g.upgrades.total_levels(), g.crystals));
                    if g.wave >= stop_wave {
                        return (log, g.wave);
                    }
                    loop {
                        let offers = g.shop.as_ref().unwrap().offers.clone();
                        let pick = offers
                            .iter()
                            .enumerate()
                            .filter(|(_, o)| !o.sold && !g.rack_full_for(o.upgrade))
                            .filter(|(_, o)| g.offer_cost(o) <= g.crystals)
                            .min_by_key(|(_, o)| g.offer_cost(o))
                            .map(|(i, _)| i);
                        match pick {
                            Some(i) => g.shop_buy(i),
                            None => break,
                        }
                    }
                    g.leave_shop();
                }
                Phase::Victory => {
                    g.menu = arcade::VICTORY_ENDLESS;
                    g.victory_choose();
                }
                Phase::Dying => g.update(dt),
                _ => return (log, g.wave),
            }
        }
        (log, g.wave)
    }

    /// Run with `cargo test --release balance_report -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn balance_report() {
        for cps in [4.0, 5.5, 7.0] {
            for seed in [1, 4, 10] {
                let (log, end) = bot_run(seed, cps, 26);
                let at = |w: u32| log.iter().find(|e| e.0 == w).map_or(0, |e| e.1);
                eprintln!(
                    "{cps} cps seed {seed}: tiers by wave 6:{} 11:{} 16:{} 21:{}  ended wave {end}",
                    at(6),
                    at(11),
                    at(16),
                    at(21)
                );
            }
        }
    }

    #[test]
    fn a_steady_typist_cannot_buy_the_whole_catalog_by_wave_ten() {
        let total: u32 = Upgrade::ALL
            .iter()
            .filter(|u| !u.def().consumable)
            .map(|u| u.max() as u32)
            .sum();
        let (log, _) = bot_run(5, 6.0, 11);
        let tiers = log.last().map_or(0, |e| e.1);
        assert!(tiers > 4, "shopping still matters: {tiers}");
        assert!(tiers * 3 < total, "{tiers} of {total} tiers by wave 10");
    }
}
