//! Simulation for TYPOIDS. The terminal shell only feeds keys and paints this.

use crate::words;
use std::f32::consts::{PI, TAU};
use std::fs;
use std::path::PathBuf;

const TURN_SHARPNESS: f32 = 9.0;
const SHIP_HIT: f32 = 1.35;

pub const MENU_ITEMS: &[&str] = &["START", "THEMES", "QUIT"];
pub const MENU_START: usize = 0;
pub const MENU_THEMES: usize = 1;
pub const MENU_QUIT: usize = 2;

pub const PAUSE_ITEMS: &[&str] = &["CONTINUE", "RESTART", "QUIT"];
pub const PAUSE_CONTINUE: usize = 0;
pub const PAUSE_RESTART: usize = 1;
pub const PAUSE_QUIT: usize = 2;

pub const OVER_ITEMS: &[&str] = &["PLAY AGAIN", "THEMES", "QUIT"];
pub const OVER_AGAIN: usize = 0;
pub const OVER_THEMES: usize = 1;
pub const OVER_QUIT: usize = 2;

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
}

impl Effect {
    pub fn life_ratio(&self) -> f32 {
        let (life, max) = match self {
            Effect::Spark { life, max, .. }
            | Effect::Shard { life, max, .. }
            | Effect::Laser { life, max, .. }
            | Effect::Ring { life, max, .. }
            | Effect::Float { life, max, .. } => (*life, *max),
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
    pub combo: u32,
    pub buffer: String,
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

pub fn submarine_local(spin: f32) -> [(f32, f32); 12] {
    let bank = (-spin * 0.025).clamp(-0.5, 0.5);
    [
        (0.0, -2.05),
        (0.68 + bank, -1.75),
        (1.02 + bank, -1.15),
        (1.16 + bank, -0.35),
        (1.08 + bank, 0.65),
        (0.76 + bank, 1.25),
        (0.0, 1.55),
        (-0.76 + bank, 1.25),
        (-1.08 + bank, 0.65),
        (-1.16 + bank, -0.35),
        (-1.02 + bank, -1.15),
        (-0.68 + bank, -1.75),
    ]
}

pub fn spaceship_local(spin: f32) -> [(f32, f32); 4] {
    let bank = (-spin * 0.035).clamp(-0.4, 0.4);
    [
        (0.0, -1.75),
        (1.2 + bank, 1.2),
        (bank * 0.4, 0.55),
        (-1.2 + bank, 1.2),
    ]
}

pub fn cat_local(spin: f32) -> [(f32, f32); 6] {
    let tilt = (-spin * 0.03).clamp(-0.35, 0.35);
    [
        (-0.68 + tilt, -0.85), // Left ear outer base
        (-0.62 + tilt, -1.65), // Left ear tip
        (-0.18 + tilt, -1.15), // Forehead left
        (0.18 + tilt, -1.15),  // Forehead right
        (0.62 + tilt, -1.65),  // Right ear tip
        (0.68 + tilt, -0.85),  // Right ear outer base
    ]
}

#[derive(Clone, Copy)]
pub struct Metrics {
    pub play_w: u16,
    pub play_h: u16,
    pub origin_y: u16,
    pub dock_y: u16,
}

pub fn metrics(term_w: u16, term_h: u16) -> Metrics {
    let dock_h = 2;
    let origin_y = 1;
    let play_h = term_h.saturating_sub(origin_y + dock_h);
    Metrics {
        play_w: term_w,
        play_h,
        origin_y,
        dock_y: origin_y + play_h,
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
            lives: 3,
            combo: 0,
            buffer: String::new(),
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
            _ => MENU_ITEMS,
        }
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
        self.buffer.clear();
        self.target = None;
        self.score = 0;
        self.lives = 3;
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
        if !any {
            self.miss = 0.32;
            self.shake = self.shake.max(0.12);
            return;
        }
        self.buffer = next;
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
            Phase::Paused | Phase::GameOver => {}
            Phase::Dying => {
                self.integrate_rocks(dt, false);
                self.dying -= dt;
                if self.dying <= 0.0 {
                    self.phase = Phase::GameOver;
                    self.menu = OVER_AGAIN;
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
        if self.pending_clear {
            self.clear_t -= dt;
            self.integrate_rocks(dt, false);
            if self.clear_t <= 0.0 {
                self.pending_clear = false;
                self.wave += 1;
                self.begin_wave();
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
            self.buffer.clear();
            self.target = None;
        }
    }

    fn begin_wave(&mut self) {
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
        for rock in &mut self.rocks {
            if !rock.live {
                continue;
            }
            rock.x += rock.vx * dt;
            rock.y += rock.vy * dt;
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
            self.detonate(id, &word, radius, tx, ty);
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
        self.combo_left = 3.4;
        let pts = word.len() as u32 * 50 * self.combo;
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
        self.burst(x, y, radius);
        self.rocks.retain(|r| r.id != id);
        self.buffer.clear();
        self.target = None;
    }

    /// One explosion shape for every theme: a ring and a few shards. Themes
    /// differ only in how `draw` colors and squashes the ring.
    fn burst(&mut self, x: f32, y: f32, radius: f32) {
        self.effects.push(Effect::Ring {
            x,
            y,
            r: radius * 0.4,
            vr: 18.0,
            life: 0.42,
            max: 0.42,
        });
        for i in 0..6 {
            let a = (i as f32 / 6.0) * TAU;
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
        self.lives = self.lives.saturating_sub(1);
        self.invuln = 1.45;
        self.hurt = 0.5;
        self.shake = 0.45;
        self.combo = 0;
        self.combo_left = 0.0;
        self.buffer.clear();
        self.target = None;
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
            Theme::Space => (1.15, 9.0),
            Theme::Underwater => (1.72, 7.5),
            Theme::Jungle => (1.05, 4.0),
            Theme::CatCafe => (1.1, 5.0),
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
            }
        }
        self.effects.retain(|e| match e {
            Effect::Spark { life, .. }
            | Effect::Shard { life, .. }
            | Effect::Laser { life, .. }
            | Effect::Ring { life, .. }
            | Effect::Float { life, .. } => *life > 0.0,
        });
    }

    fn decay_fx(&mut self, dt: f32) {
        self.thrust = (self.thrust - dt * 1.6).max(0.0);
        self.miss = (self.miss - dt).max(0.0);
        self.hurt = (self.hurt - dt).max(0.0);
        self.invuln = (self.invuln - dt).max(0.0);
        self.banner_t = (self.banner_t - dt).max(0.0);
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
    fn menu_wraps_across_three_items() {
        let mut g = Game::new(100, 40, 42);
        assert_eq!(g.menu, MENU_START);
        assert_eq!(MENU_ITEMS.len(), 3);
        g.menu_move(1);
        assert_eq!(g.menu, MENU_THEMES);
        g.menu_move(1);
        assert_eq!(g.menu, MENU_QUIT);
        g.menu_move(1);
        assert_eq!(g.menu, MENU_START);
        g.menu_move(-1);
        assert_eq!(g.menu, MENU_QUIT);
        g.menu_move(-1);
        assert_eq!(g.menu, MENU_THEMES);
        g.menu_move(-1);
        assert_eq!(g.menu, MENU_START);
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
