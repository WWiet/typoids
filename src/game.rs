//! Simulation for TYPOIDS. The terminal shell only feeds keys and paints this.

use crate::save::{self, RunSave};
use crate::upgrades::{Rarity, Upgrade, Upgrades};
use crate::words;
use std::f32::consts::{PI, TAU};
use std::fs;
use std::path::PathBuf;

const TURN_SHARPNESS: f32 = 9.0;
const SHIP_HIT: f32 = 1.35;

pub const MENU_ITEMS: &[&str] = &["START", "THEMES", "QUIT"];
pub const MENU_ITEMS_SAVED: &[&str] = &["CONTINUE", "NEW RUN", "THEMES", "QUIT"];
pub const MENU_START: usize = 0;

/// What the highlighted title entry does. The title list grows a CONTINUE
/// entry when a run is saved, so key handling asks this instead of an index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TitleAction {
    Continue,
    Start,
    Themes,
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
    last_initial: Option<char>,
    /// A wrong key was pressed since the last kill (breaks Steady Hands).
    fumbled: bool,
    /// Only the real binary writes save files; tests leave this off.
    persist: bool,
    pub has_save: bool,
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
            last_initial: None,
            fumbled: false,
            persist: false,
            has_save: false,
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
        (self.play_w * 0.5, self.play_h * 0.5)
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
        let id = self.target?;
        self.rocks
            .iter()
            .find(|r| r.id == id && r.live)
            .map(|r| r.word.as_str())
    }

    pub fn menu_move(&mut self, delta: isize) {
        let len = self.menu_items().len() as isize;
        self.menu = (self.menu as isize + delta).rem_euclid(len) as usize;
    }

    /// The choices shown for the current phase. The title and the two popups
    /// all share the same `menu` index, but each has its own wording.
    pub fn menu_items(&self) -> &'static [&'static str] {
        match self.phase {
            Phase::Paused => PAUSE_ITEMS,
            Phase::GameOver => OVER_ITEMS,
            _ if self.has_save => MENU_ITEMS_SAVED,
            _ => MENU_ITEMS,
        }
    }

    pub fn title_action(&self) -> TitleAction {
        let offset = self.has_save as usize;
        match self.menu.checked_sub(offset) {
            None => TitleAction::Continue,
            Some(0) => TitleAction::Start,
            Some(1) => TitleAction::Themes,
            Some(_) => TitleAction::Quit,
        }
    }

    pub fn max_lives(&self) -> u32 {
        BASE_HEARTS + self.upgrades.level(Upgrade::ExtraHeart) as u32
    }

    pub fn pause(&mut self) {
        if self.phase == Phase::Playing {
            self.phase = Phase::Paused;
            self.menu = PAUSE_CONTINUE;
        }
    }

    pub fn resume(&mut self) {
        if self.phase == Phase::Paused {
            self.phase = Phase::Playing;
        }
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
    }

    pub fn apply_theme(&mut self) {
        self.theme = Theme::from_index(self.theme_choice);
        self.theme_menu_open = false;
        if self.phase == Phase::GameOver {
            self.menu = OVER_AGAIN;
        }
        self.reseed_stars();
    }

    pub fn start(&mut self) {
        self.phase = Phase::Playing;
        self.theme_menu_open = false;
        self.reset_run();
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
        self.shop = None;
        self.shop_sel = 0;
        self.shop_note.clear();
        self.kills = 0;
        self.revive_used = false;
        self.last_initial = None;
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
    }

    /// Forget what the player has typed and the hazard it locked.
    fn clear_typed(&mut self) {
        self.buffer.clear();
        self.buffer_t.clear();
        self.target = None;
    }

    pub fn type_char(&mut self, c: char) {
        if self.phase != Phase::Playing || self.pending_clear {
            return;
        }
        let c = c.to_ascii_lowercase();
        if !c.is_ascii_lowercase() {
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
            self.miss = 0.32;
            self.fumbled = true;
            self.shake = self.shake.max(0.12);
            return;
        }
        self.buffer = next;
        self.buffer_t.push(self.time);
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

    pub fn backspace(&mut self) {
        if self.phase != Phase::Playing || self.buffer.is_empty() {
            return;
        }
        self.buffer.pop();
        self.buffer_t.pop();
        if self.buffer.is_empty() {
            self.target = None;
        } else {
            self.lock_target();
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        self.decay_fx(dt);
        self.update_effects(dt);
        self.spring_ship(dt);
        self.spawn_exhaust(dt);

        match self.phase {
            Phase::Title => self.update_title(dt),
            Phase::Paused | Phase::GameOver | Phase::Shop => {}
            Phase::Dying => {
                self.integrate_rocks(dt, false);
                self.dying -= dt;
                if self.dying <= 0.0 {
                    self.phase = Phase::GameOver;
                    self.menu = OVER_AGAIN;
                    self.gems.clear();
                    self.clear_save();
                    self.save_best();
                }
            }
            Phase::Playing => self.update_play(dt),
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
                self.open_shop(true);
            }
            return;
        }

        self.spawn_timer -= dt;
        let cap = (3 + self.wave / 2).min(7) as usize;
        if self.budget > 0 && self.rocks.len() < cap && self.spawn_timer <= 0.0 {
            self.spawn_threat();
            self.budget -= 1;
            let gap = (1.85 - self.wave as f32 * 0.12).max(0.72);
            self.spawn_timer = gap;
        }

        self.integrate_rocks(dt, true);
        self.aim_ship(dt, false);

        if self.budget == 0 && self.rocks.iter().all(|r| !r.live) {
            self.pending_clear = true;
            self.clear_t = 1.45;
            self.banner = match self.theme {
                Theme::Space => "WAVE CLEAR".to_string(),
                Theme::Underwater => "MINEFIELD CLEAR".to_string(),
                Theme::Jungle => "CANOPY CLEAR".to_string(),
                Theme::CatCafe => "CAFE CLEAR".to_string(),
            };
            self.banner_t = 1.45;
            self.clear_typed();
        }
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
        self.budget = 5 + self.wave * 2;
        self.spawn_timer = 0.35;
        self.pending_clear = false;
        self.banner = format!("WAVE {}", self.wave);
        self.banner_t = 1.5;
        let immediate = 3.min(self.budget);
        for _ in 0..immediate {
            self.spawn_threat();
            self.budget -= 1;
        }
    }

    fn integrate_rocks(&mut self, dt: f32, collide: bool) {
        let (sx, sy) = self.ship_pos();
        let mut hits = Vec::new();
        let mut pace = 1.0 - 0.1 * self.upgrades.level(Upgrade::SlowTide) as f32;
        if self.brake > 0.0 {
            pace *= 0.35;
        }
        if self.lives == 1 && self.upgrades.has(Upgrade::LastStand) {
            pace *= 0.8;
        }
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
        let desired = if !attract && self.buffer.is_empty() {
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
        if kill {
            let before = self.kills;
            self.detonate(id, &word, radius, tx, ty);
            self.chain_kills(tx, ty);
            if self.upgrades.has(Upgrade::Sweeper) && self.kills / 20 != before / 20 {
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

    fn detonate(&mut self, id: u32, word: &str, radius: f32, x: f32, y: f32) {
        self.combo = self.combo.saturating_add(1);
        self.combo_left = 3.4 + 1.5 * self.upgrades.level(Upgrade::LongFuse) as f32;
        let initial = word.chars().next();
        let echo = self.upgrades.has(Upgrade::Alliteration)
            && initial.is_some()
            && initial == self.last_initial;
        self.last_initial = initial;
        let big = self.upgrades.has(Upgrade::BigGame) && word.len() >= 7;
        let last_stand = self.lives == 1 && self.upgrades.has(Upgrade::LastStand);
        let bonus = 1 + echo as u32 + big as u32 + last_stand as u32;
        let clean = !self.fumbled;
        self.fumbled = false;
        let steady = if clean {
            self.upgrades.level(Upgrade::SteadyHands) as u32
        } else {
            0
        };
        let pts = word.len() as u32 * 50 * self.combo * bonus * (100 + 25 * steady) / 100;
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
            text: if self.combo >= 2 {
                format!("+{pts} x{}", self.combo)
            } else {
                format!("+{pts}")
            },
        });
        let crystals = 1
            + (word.len() >= 6) as u32
            + self.upgrades.level(Upgrade::Prospector) as u32
            + echo as u32
            + steady
            + if big { 2 } else { 0 }
            + (self.combo >= 5 && self.upgrades.has(Upgrade::Bounty)) as u32;
        let luck = Upgrade::lucky_chance(self.upgrades.level(Upgrade::LuckyPenny));
        let lucky = luck > 0.0 && self.rng.f32() < luck;
        self.drop_gems(x, y, crystals * (1 + lucky as u32));
        self.shove_neighbors(id, x, y);
        self.kills += 1;
        if self.upgrades.has(Upgrade::Leech)
            && self.kills / 12 != (self.kills - 1) / 12
            && self.lives < self.max_lives()
        {
            self.lives += 1;
        }
        self.burst(x, y, radius);
        self.rocks.retain(|r| r.id != id);
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
            self.detonate(id, &word, radius, rx, ry);
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
            self.detonate(id, &word, radius, rx, ry);
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
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 && self.upgrades.has(Upgrade::SecondWind) && !self.revive_used {
            self.revive_used = true;
            self.lives = 1;
            self.banner = "SECOND WIND".to_string();
            self.banner_t = 1.2;
        }
        self.invuln = 1.45;
        self.hurt = 0.5;
        self.shake = 0.45;
        self.combo = 0;
        self.combo_left = 0.0;
        self.clear_typed();
        let kx = self.rng.range(-1.0, 1.0);
        let ky = self.rng.range(-1.0, 1.0);
        self.kick(kx, ky, 10.0);
        self.burst(x, y, radius);
        self.rocks.retain(|r| r.id != id);
        if self.lives == 0 {
            self.phase = Phase::Dying;
            self.dying = 0.95;
            self.banner = match self.theme {
                Theme::Space => "SHIP LOST".to_string(),
                Theme::Underwater => "HULL BREACH".to_string(),
                Theme::Jungle => "MONKEY DOWN".to_string(),
                Theme::CatCafe => "CAT NAPPING".to_string(),
            };
            self.banner_t = 0.95;
        }
    }

    #[cfg(test)]
    pub fn open_shop_for_test(&mut self) {
        self.open_shop(false);
    }

    /// Cards on the shelf: three, or four with Wide Shop.
    pub fn shop_slots(&self) -> usize {
        SHOP_SLOTS + self.upgrades.has(Upgrade::WideShop) as usize
    }

    /// Menu index of the reroll button; the next-wave button follows it.
    pub fn shop_reroll_index(&self) -> usize {
        self.shop_slots()
    }

    pub fn shop_go_index(&self) -> usize {
        self.shop_slots() + 1
    }

    pub fn shop_move(&mut self, delta: isize) {
        let entries = self.shop_slots() as isize + 2;
        self.shop_sel = (self.shop_sel as isize + delta).rem_euclid(entries) as usize;
    }

    /// Enter the shop. `pay_interest` is false when resuming a saved shop.
    fn open_shop(&mut self, pay_interest: bool) {
        self.phase = Phase::Shop;
        self.clear_typed();
        self.shop_note.clear();
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
    pub fn offer_cost(&self, offer: &Offer) -> u32 {
        let off = Upgrade::discount_off(self.upgrades.level(Upgrade::Discount));
        offer
            .upgrade
            .cost(offer.tier - 1)
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
            return;
        }
        let cost = self.offer_cost(offer);
        if self.crystals < cost {
            self.shop_note = format!("Need {} more crystals.", cost - self.crystals);
            return;
        }
        self.crystals -= cost;
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
            return;
        }
        let cost = self.reroll_cost();
        if self.crystals < cost {
            self.shop_note = format!("Need {} more crystals.", cost - self.crystals);
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
        self.shop = None;
        self.phase = Phase::Playing;
        self.begin_wave();
    }

    /// Turn on save files. Only the real binary calls this.
    pub fn enable_persistence(&mut self) {
        self.persist = true;
        self.has_save = save::path().is_some_and(|p| p.exists());
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
            let in_shop = self.phase == Phase::Shop;
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
        self.lives = run.lives.min(self.max_lives());
        if run.in_shop {
            self.open_shop(false);
        } else {
            self.phase = Phase::Playing;
            self.begin_wave();
        }
    }

    fn spawn_threat(&mut self) {
        let word = self.pick_word();
        let radius = 1.8 + (word.len() as f32 * 0.15).min(1.2);
        let (x, y) = self.edge_point();
        let (cx, cy) = self.ship_center();
        let tx = cx + self.rng.range(-7.0, 7.0);
        let ty = cy + self.rng.range(-4.0, 4.0);
        let speed = 3.6 + self.wave as f32 * 0.55 + self.rng.range(0.0, 0.8);
        let (dx, dy) = normalize(tx - x, ty - y);
        let mut rock = self.make_rock(x, y, radius, word);
        rock.vx = dx * speed;
        rock.vy = dy * speed;
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
        }
    }

    fn pick_word(&mut self) -> String {
        let pool = self.pool();
        let used: Vec<String> = self.rocks.iter().map(|r| r.word.clone()).collect();
        for _ in 0..48 {
            let word = self.rng.pick(pool);
            if used.iter().any(|u| conflicts(u, word)) {
                continue;
            }
            return word.to_string();
        }
        for _ in 0..24 {
            let word = self.rng.pick(pool);
            if used
                .iter()
                .any(|u| u == word || u.starts_with(word) || word.starts_with(u))
            {
                continue;
            }
            return word.to_string();
        }
        self.rng.pick(pool).to_string()
    }

    fn pool(&mut self) -> &'static [&'static str] {
        let roll = self.rng.f32();
        match self.wave {
            1 => words::SHORT,
            2 | 3 => {
                if roll < 0.5 {
                    words::SHORT
                } else {
                    words::MID
                }
            }
            4 | 5 => {
                if roll < 0.2 {
                    words::SHORT
                } else {
                    words::MID
                }
            }
            _ => {
                if roll < 0.45 {
                    words::MID
                } else {
                    words::LONG
                }
            }
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
        if self.combo > 0 && self.phase == Phase::Playing {
            self.combo_left -= dt;
            if self.combo_left <= 0.0 {
                self.combo = 0;
            }
        }
        if self.shake > 0.0 {
            self.shake = (self.shake - dt * 1.7).max(0.0);
            let mag = self.shake.min(0.85);
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
    fn menu_wraps_across_three_items() {
        let mut g = Game::new(100, 40, 42);
        assert_eq!(g.menu, MENU_START);
        assert_eq!(MENU_ITEMS.len(), 3);
        let actions: Vec<TitleAction> = (0..3)
            .map(|_| {
                let a = g.title_action();
                g.menu_move(1);
                a
            })
            .collect();
        assert_eq!(
            actions,
            [TitleAction::Start, TitleAction::Themes, TitleAction::Quit]
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
        assert_eq!(clean.gems.len(), base_gems + 2);

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
        assert_eq!(g.offer_cost(&card), full - 4);
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
    fn sweeper_clears_the_screen_on_every_twentieth_kill() {
        let mut g = with_rock("planet");
        g.upgrades.set(Upgrade::Sweeper, 1);
        for (x, word) in [(30.0, "cat"), (40.0, "moon"), (50.0, "orbit")] {
            let rock = g.make_rock(x, 20.0, 1.8, word.to_string());
            g.rocks.push(rock);
        }
        g.kills = 19;
        type_word(&mut g, "planet");
        assert!(g.rocks.iter().all(|r| !r.live) || g.rocks.is_empty());
        assert_eq!(g.kills, 23);
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
    fn alliteration_doubles_score_for_repeated_initials() {
        let mut plain = with_rock("star");
        type_word(&mut plain, "star");
        let base = plain.score;

        let mut g = with_rock("star");
        g.upgrades.set(Upgrade::Alliteration, 1);
        g.last_initial = Some('s');
        type_word(&mut g, "star");
        assert_eq!(g.score, base * 2);
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
}
