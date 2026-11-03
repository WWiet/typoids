//! Space, underwater, and jungle scenes. Ships, hazards, and projectiles are
//! braille line art; words sit on top in plain cells so they stay readable.

use crate::game::{
    metrics, rotate, Effect, Game, Metrics, Phase, Rock, Theme, SHOP_GO, SHOP_REROLL, SHOP_SLOTS,
};
use crate::upgrades::Rarity;

mod bigfont;
mod ships;
mod stage;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;
use std::f32::consts::{PI, TAU};

struct Palette {
    bg_top: Color,
    bg_bottom: Color,
    dim: Color,
    ink: Color,
    amber: Color,
    hot: Color,
    rock: Color,
    danger: Color,
    /// Explosion / energy accent.
    accent: Color,
    star: Color,
    shadow: Color,
    dead: Color,
    faint: Color,
    /// Backing tone behind word chips.
    chip: Color,
    /// Spilled liquid: coffee, coconut milk.
    liquid: Color,
    /// Crystals, the currency dropped by hazards.
    gem: Color,
}

const SPACE: Palette = Palette {
    bg_top: Color::Rgb(5, 7, 11),
    bg_bottom: Color::Rgb(0, 0, 0),
    dim: Color::Rgb(92, 128, 104),
    ink: Color::Rgb(214, 255, 226),
    amber: Color::Rgb(255, 196, 77),
    hot: Color::Rgb(255, 246, 220),
    rock: Color::Rgb(168, 214, 184),
    danger: Color::Rgb(255, 92, 74),
    accent: Color::Rgb(255, 156, 72),
    star: Color::Rgb(46, 68, 56),
    shadow: Color::Rgb(40, 64, 50),
    dead: Color::Rgb(40, 52, 44),
    faint: Color::Rgb(62, 84, 72),
    chip: Color::Rgb(14, 22, 20),
    liquid: Color::Rgb(255, 156, 72),
    gem: Color::Rgb(120, 220, 255),
};

const UNDERWATER: Palette = Palette {
    bg_top: Color::Rgb(8, 32, 48),
    bg_bottom: Color::Rgb(1, 7, 15),
    dim: Color::Rgb(78, 140, 158),
    ink: Color::Rgb(190, 246, 250),
    amber: Color::Rgb(255, 209, 110),
    hot: Color::Rgb(240, 255, 252),
    rock: Color::Rgb(122, 192, 210),
    danger: Color::Rgb(255, 106, 92),
    accent: Color::Rgb(96, 224, 255),
    star: Color::Rgb(34, 92, 112),
    shadow: Color::Rgb(20, 78, 98),
    dead: Color::Rgb(24, 48, 60),
    faint: Color::Rgb(52, 104, 122),
    chip: Color::Rgb(6, 26, 38),
    liquid: Color::Rgb(96, 224, 255),
    gem: Color::Rgb(255, 128, 196),
};

const JUNGLE: Palette = Palette {
    bg_top: Color::Rgb(8, 30, 17),
    bg_bottom: Color::Rgb(1, 8, 4),
    dim: Color::Rgb(96, 142, 88),
    ink: Color::Rgb(218, 246, 194),
    amber: Color::Rgb(255, 208, 92),
    hot: Color::Rgb(255, 248, 205),
    rock: Color::Rgb(166, 112, 68),
    danger: Color::Rgb(255, 96, 72),
    accent: Color::Rgb(160, 226, 94),
    star: Color::Rgb(94, 142, 74),
    shadow: Color::Rgb(26, 64, 36),
    dead: Color::Rgb(42, 34, 24),
    faint: Color::Rgb(72, 112, 70),
    chip: Color::Rgb(12, 30, 18),
    liquid: Color::Rgb(238, 232, 204),
    gem: Color::Rgb(255, 110, 140),
};

const CAT_CAFE: Palette = Palette {
    bg_top: Color::Rgb(24, 16, 12),
    bg_bottom: Color::Rgb(12, 8, 6),
    dim: Color::Rgb(175, 142, 112),
    ink: Color::Rgb(255, 244, 222),
    amber: Color::Rgb(246, 194, 68),
    hot: Color::Rgb(255, 250, 238),
    rock: Color::Rgb(194, 134, 82),
    danger: Color::Rgb(228, 92, 70),
    accent: Color::Rgb(236, 168, 88),
    star: Color::Rgb(118, 88, 68),
    shadow: Color::Rgb(42, 28, 20),
    dead: Color::Rgb(46, 32, 24),
    faint: Color::Rgb(92, 72, 56),
    chip: Color::Rgb(40, 28, 20),
    liquid: Color::Rgb(176, 112, 62),
    gem: Color::Rgb(150, 190, 255),
};

fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Space => &SPACE,
        Theme::Underwater => &UNDERWATER,
        Theme::Jungle => &JUNGLE,
        Theme::CatCafe => &CAT_CAFE,
    }
}

struct InkLayer {
    w: usize,
    h: usize,
    bits: Vec<u8>,
    pri: Vec<u8>,
    col: Vec<Color>,
}

impl InkLayer {
    fn new(w: usize, h: usize, base: Color) -> Self {
        let n = w.saturating_mul(h);
        Self {
            w,
            h,
            bits: vec![0; n],
            pri: vec![0; n],
            col: vec![base; n],
        }
    }

    fn dot(&mut self, x: f32, y: f32, pri: u8, col: Color) {
        let x = x.round() as i32;
        let y = y.round() as i32;
        if x < 0 || y < 0 {
            return;
        }
        let cw = self.w as i32 * 2;
        let ch = self.h as i32 * 4;
        if x >= cw || y >= ch {
            return;
        }
        let cx = (x / 2) as usize;
        let cy = (y / 4) as usize;
        let bit = match (x % 2, y % 4) {
            (0, 0) => 0,
            (0, 1) => 1,
            (0, 2) => 2,
            (1, 0) => 3,
            (1, 1) => 4,
            (1, 2) => 5,
            (0, 3) => 6,
            (1, 3) => 7,
            _ => return,
        };
        let i = cy * self.w + cx;
        self.bits[i] |= 1 << bit;
        if pri >= self.pri[i] {
            self.pri[i] = pri;
            self.col[i] = col;
        }
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, pri: u8, col: Color) {
        let mut x = x0.round() as i32;
        let mut y = y0.round() as i32;
        let x1 = x1.round() as i32;
        let y1 = y1.round() as i32;
        let dx = (x1 - x).abs();
        let sx = if x < x1 { 1 } else { -1 };
        let dy = -(y1 - y).abs();
        let sy = if y < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.dot(x as f32, y as f32, pri, col);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
}

/// Draw a small leaf silhouette into the braille layer.
fn ink_leaf(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    at: (f32, f32),
    angle: f32,
    size: (f32, f32),
    pri: u8,
    col: Color,
) {
    let (x, y) = at;
    let (length, width) = size;
    let (s, c) = angle.sin_cos();
    let place = |lx: f32, ly: f32| (x + lx * c - ly * s, y + lx * s + ly * c);
    let steps = 5;
    let mut prev_l: Option<(f32, f32)> = None;
    let mut prev_r: Option<(f32, f32)> = None;
    for i in 0..=steps {
        let u = i as f32 / steps as f32 * 2.0 - 1.0;
        let half = width * (1.0 - u * u).max(0.0);
        let ly = u * length * 0.5;
        let (lx, ly) = place(-half, ly);
        let lp = to_px(lx, ly);
        let (rx, ry) = place(half, ly);
        let rp = to_px(rx, ry);
        if let Some(p) = prev_l {
            ink.line(p.0, p.1, lp.0, lp.1, pri, col);
        }
        if let Some(p) = prev_r {
            ink.line(p.0, p.1, rp.0, rp.1, pri, col);
        }
        prev_l = Some(lp);
        prev_r = Some(rp);
    }
    let (ax, ay) = place(0.0, -length * 0.5);
    let (bx, by) = place(0.0, length * 0.5);
    let a = to_px(ax, ay);
    let b = to_px(bx, by);
    ink.line(a.0, a.1, b.0, b.1, pri, col);
}

/// Palm fronds drooping in from the top corners frame the play field. Everything here sits at the lowest braille priority so hazards
/// and the monkey always stay on top.
fn draw_jungle_scenery(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
) {
    let vine_col = fade(pal.shadow, 0.85);
    let frond_col = fade(pal.accent, 0.2);

    // Palm fronds spilling in from the top corners.
    for (dir, cx) in [(1.0f32, -0.6f32), (-1.0, game.play_w + 0.6)] {
        for k in 0..3 {
            let t = k as f32 / 2.0;
            let spread = (t - 0.5) * 1.3;
            let angle = spread + dir * 0.18;
            let len = 3.1 - spread.abs() * 1.3;
            let x = cx + dir * (0.5 + t * 2.3);
            let y = 0.1 + (t * PI).sin() * 0.9;
            let (ax, ay) = to_px(cx, -0.7);
            let (bx, by) = to_px(x, y);
            ink.line(ax, ay, bx, by, 0, vine_col);
            let (s, c) = angle.sin_cos();
            let head = (x - s * len * 0.42, y + c * len * 0.42);
            ink_leaf(ink, to_px, head, angle, (len, 1.25), 0, frond_col);
        }
    }
}

/// Motion speed for a falling jungle leaf. Slow and gentle drift.
pub(crate) fn jungle_leaf_speed(phase: f32) -> f32 {
    0.06 + (phase.sin() * 0.5 + 0.5) * 0.06
}

/// Motion parameters for a falling jungle leaf: (x, y, angle).
/// Leaves only fall straight down the screen, oriented downward, moving slowly.
pub(crate) fn jungle_leaf_motion(
    star_x: f32,
    star_y: f32,
    phase: f32,
    time: f32,
    play_h: f32,
) -> (f32, f32, f32) {
    let speed = jungle_leaf_speed(phase);
    let y = (star_y + time * speed).rem_euclid(play_h.max(1.0));
    (star_x, y, 0.0)
}

/// Falling leaves and faint fireflies fill the jungle canopy.
fn draw_jungle_particles(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
) {
    let play_h = game.play_h.max(1.0);
    for star in &game.stars {
        if star.glyph == '✦' {
            // Firefly: a gentle bob and soft faint glow.
            let y = (star.y + (game.time * 0.2 + star.phase).sin() * 0.3).rem_euclid(play_h);
            let x = star.x + (game.time * 0.2 + star.phase * 1.3).sin() * 0.3;
            let pulse = (game.time * 0.8 + star.phase).sin() * 0.5 + 0.5;
            let col = fade(pal.star, 0.25 + 0.25 * pulse);
            let (px, py) = to_px(x, y);
            ink.dot(px, py, 0, col);
            continue;
        }

        // Leaf: drift gently straight down the screen at slow speed.
        let (x, y, angle) = jungle_leaf_motion(star.x, star.y, star.phase, game.time, play_h);
        let large = star.glyph == '❧';
        let (len, width) = if large { (2.0, 1.0) } else { (1.2, 0.6) };
        let col = if large {
            fade(pal.star, 0.5)
        } else {
            fade(pal.faint, 0.6)
        };
        ink_leaf(ink, to_px, (x, y), angle, (len, width), 0, col);
    }
}

/// Terminal cells are about twice as tall as wide, so hazard art squashes its
/// local y by this much (and widens x a little) to look physically round.
const CELL_Y: f32 = 0.625;
const CELL_X: f32 = 1.25;

/// Points on an ellipse, in unit coordinates scaled by the caller.
fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * TAU;
            (cx + a.cos() * rx, cy + a.sin() * ry)
        })
        .collect()
}

/// Connect points (given in the hazard's local frame, in units of its radius)
/// with braille lines.
fn ink_path(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    frame: (&Rock, f32),
    pts: &[(f32, f32)],
    closed: bool,
    style: (u8, Color),
) {
    let (rock, rot) = frame;
    let world: Vec<(f32, f32)> = pts
        .iter()
        .map(|(lx, ly)| {
            let (x, y) = rotate(lx * rock.radius, ly * rock.radius, rot);
            let (x, y) = (x * CELL_X, y * CELL_Y);
            to_px(rock.x + x, rock.y + y)
        })
        .collect();
    let last = if closed { world.len() } else { world.len() - 1 };
    for i in 0..last {
        let (a, b) = (world[i], world[(i + 1) % world.len()]);
        ink.line(a.0, a.1, b.0, b.1, style.0, style.1);
    }
}

/// A coconut: round, with a hairy husk, a highlight, and three pores.
fn draw_coconut(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    rock: &Rock,
    style: (u8, Color),
    pal: &Palette,
) {
    let (pri, col) = style;
    ink_path(
        ink,
        to_px,
        (rock, 0.0),
        &ellipse(0.0, 0.0, 0.92, 0.92, 22),
        true,
        style,
    );
    // Short husk fibers that turn with the nut.
    for i in 0..14 {
        let a = rock.rot + i as f32 / 14.0 * TAU;
        let len = 0.1 + ((i * 7) % 5) as f32 * 0.04;
        let inner = (a.cos() * 0.9, a.sin() * 0.9);
        let outer = (a.cos() * (0.9 + len), a.sin() * (0.9 + len));
        ink_path(
            ink,
            to_px,
            (rock, 0.0),
            &[inner, outer],
            false,
            (pri, fade(col, 0.75)),
        );
    }
    // Fixed highlight, so the light stays put while the nut spins.
    let arc: Vec<(f32, f32)> = (0..5)
        .map(|i| {
            let a = -2.5 + i as f32 * 0.2;
            (a.cos() * 0.66, a.sin() * 0.66)
        })
        .collect();
    ink_path(
        ink,
        to_px,
        (rock, 0.0),
        &arc,
        false,
        (pri + 2, fade(pal.hot, 0.55)),
    );
    // Three pores form a little face.
    for (px, py) in [(-0.26, -0.12), (0.26, -0.12), (0.0, 0.26)] {
        let (ox, oy) = rotate(px * rock.radius, py * rock.radius, rock.rot * 0.25);
        let (ox, oy) = (ox * CELL_X, oy * CELL_Y);
        let (x, y) = to_px(rock.x + ox, rock.y + oy);
        ink.dot(x, y, 4, Color::Rgb(58, 34, 18));
    }
}

/// A coffee cup on a saucer, upright and gently rocking. Steam rises only
/// while it is the one being typed.
fn draw_cup(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    rock: &Rock,
    style: (u8, Color),
    (pal, time): (&Palette, f32),
    aimed: bool,
) {
    let (pri, _) = style;
    let tilt = (time * 0.9 + rock.id as f32).sin() * 0.16;
    let f = (rock, tilt);
    // Rim, and the coffee inside it.
    ink_path(
        ink,
        to_px,
        f,
        &ellipse(0.0, -0.55, 0.82, 0.24, 18),
        true,
        style,
    );
    ink_path(
        ink,
        to_px,
        f,
        &ellipse(0.0, -0.52, 0.64, 0.16, 14),
        true,
        (pri, fade(pal.liquid, 0.9)),
    );
    // Body: straight sides tapering to a rounded base.
    let mut body = vec![(-0.82, -0.55), (-0.62, 0.3)];
    body.extend((0..=6).map(|i| {
        let a = PI * (i as f32 / 6.0);
        (-0.5 * a.cos() * 1.0, 0.3 + a.sin() * 0.36)
    }));
    body.push((0.62, 0.3));
    body.push((0.82, -0.55));
    ink_path(ink, to_px, f, &body, false, style);
    // Handle on the right.
    let handle = [(0.8, -0.38), (1.26, -0.3), (1.3, 0.05), (0.7, 0.18)];
    ink_path(ink, to_px, f, &handle, false, style);
    // Saucer sits below, still level.
    let saucer: Vec<(f32, f32)> = (0..=8)
        .map(|i| {
            let a = PI * (i as f32 / 8.0);
            (a.cos() * 1.15, 0.68 + a.sin() * 0.14)
        })
        .collect();
    ink_path(
        ink,
        to_px,
        (rock, 0.0),
        &saucer,
        false,
        (pri, fade(pal.dim, 0.9)),
    );
    if aimed {
        for side in [-0.28f32, 0.28] {
            let wave = (time * 3.0 + rock.id as f32 + side * 4.0).sin() * 0.14;
            let steam = [(side, -0.85), (side + wave, -1.1), (side - wave, -1.35)];
            ink_path(
                ink,
                to_px,
                (rock, 0.0),
                &steam,
                false,
                (pri, fade(pal.faint, 0.8)),
            );
        }
    }
}

fn projectile_colors(theme: Theme, pal: &Palette) -> (Color, Color) {
    match theme {
        Theme::Space | Theme::Underwater => (pal.hot, pal.accent),
        Theme::Jungle | Theme::CatCafe => (pal.amber, pal.amber),
    }
}

/// Every theme fires the same shape: a short bright dash with a three-dot
/// fading tail. Themes differ only in color.
fn draw_projectile(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    path: (f32, f32, f32, f32),
    life: f32,
    head: Color,
    tail: Color,
) {
    let (x0, y0, x1, y1) = path;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let dist = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / dist, dy / dist);
    let progress = 1.0 - life;
    let (hx, hy) = (x0 + dx * progress, y0 + dy * progress);
    for i in 1..=3 {
        let back = 0.6 + i as f32 * 0.45;
        let (px, py) = to_px(hx - ux * back, hy - uy * back);
        ink.dot(
            px,
            py,
            4,
            fade(tail, (1.0 - i as f32 / 4.0) * (0.3 + 0.7 * life)),
        );
    }
    let (a, b) = to_px(hx - ux * 0.6, hy - uy * 0.6);
    let (c, d) = to_px(hx, hy);
    ink.line(a, b, c, d, 5, head);
}

pub fn draw(frame: &mut Frame, game: &Game) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    let w = area.width;
    let h = area.height;
    let pal = palette(game.theme);
    if game.theme == Theme::Jungle {
        paint_jungle_canopy(buf, area, pal, game.time);
    } else {
        paint_water(buf, area, pal);
    }
    if w < 48 || h < 18 {
        put(
            buf,
            1,
            1,
            "Resize the terminal to at least 70x22.",
            style(pal.ink),
        );
        return;
    }

    let m = metrics(w, h);
    let ox = 0i32;
    let oy = m.origin_y as i32;
    let (shx, shy) = (game.shake_x, game.shake_y);

    let mut ink = InkLayer::new(m.play_w as usize, m.play_h as usize, pal.rock);
    let to_px = |x: f32, y: f32| ((x + shx) * 2.0, (y + shy) * 4.0);
    let bg_to_px = |x: f32, y: f32| (x * 2.0, y * 4.0);

    if game.theme == Theme::Jungle {
        draw_jungle_scenery(&mut ink, game, pal, &bg_to_px);
        draw_jungle_particles(&mut ink, game, pal, &bg_to_px);
    } else {
        for star in &game.stars {
            let (x, y) = match game.theme {
                Theme::Underwater => {
                    let speed = 0.22 + ((star.x * 0.73 + star.y * 0.19) % 1.0) * 0.58;
                    let y_world = (star.y - game.time * speed).rem_euclid(game.play_h.max(1.0));
                    let sway = (game.time * 0.6 + star.y * 1.7).sin() * 0.18;
                    ((star.x + sway).round() as i32, y_world.round() as i32 + oy)
                }
                Theme::CatCafe => {
                    let speed = 0.12 + ((star.x * 0.61 + star.y * 0.27) % 1.0) * 0.18;
                    let y_world = (star.y - game.time * speed).rem_euclid(game.play_h.max(1.0));
                    let sway = (game.time * 0.4 + star.y * 1.3).sin() * 0.22;
                    ((star.x + sway).round() as i32, y_world.round() as i32 + oy)
                }
                _ => (star.x.round() as i32, star.y.round() as i32 + oy),
            };
            put(buf, x, y, &star.glyph.to_string(), style(pal.star));
        }
    }

    for effect in &game.effects {
        let t = effect.life_ratio();
        match effect {
            Effect::Laser { x0, y0, x1, y1, .. } => {
                let (head, tail) = projectile_colors(game.theme, pal);
                draw_projectile(&mut ink, &to_px, (*x0, *y0, *x1, *y1), t, head, tail);
            }
            Effect::Shard { x, y, rot, len, .. } => {
                let (x0, y0) = to_px(x - rot.cos() * len, y - rot.sin() * len);
                let (x1, y1) = to_px(x + rot.cos() * len, y + rot.sin() * len);
                ink.line(x0, y0, x1, y1, 3, fade(pal.accent, 0.2 + 0.8 * t));
                ink.dot(x1, y1, 3, fade(pal.hot, 0.2 + 0.8 * t));
            }
            Effect::Ring { x, y, r, .. } => {
                let col = fade(pal.accent, 0.2 + 0.8 * t);
                let (steps, oval, wobble) = match game.theme {
                    Theme::Space => (((*r * 10.0) as i32).clamp(8, 48), 1.0, 0.0),
                    Theme::Underwater => (((*r * 9.0) as i32).clamp(12, 64), 0.55, 0.05),
                    Theme::Jungle => (((*r * 10.0) as i32).clamp(10, 56), 0.82, 0.04),
                    Theme::CatCafe => (((*r * 10.0) as i32).clamp(10, 56), 0.75, 0.03),
                };
                for i in 0..steps {
                    let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                    let wob = (a * 3.0 + game.time * 2.4).sin() * wobble;
                    let rr = *r + wob;
                    let (px, py) = to_px(x + a.cos() * rr, y + a.sin() * rr * oval);
                    ink.dot(px, py, 3, col);
                }
            }
            Effect::Drop { x, y, vx, vy, .. } => {
                // A short streak along the fall, brightest while airborne.
                let (x1, y1) = to_px(*x, *y);
                let (x0, y0) = to_px(x - vx * 0.03, y - vy * 0.03);
                let col = fade(pal.liquid, 0.3 + 0.7 * t);
                ink.line(x0, y0, x1, y1, 3, col);
            }
            Effect::Puddle { x, y, w, .. } => {
                // Holds full strength for a while, then soaks away.
                let k = (t * 2.2).min(1.0);
                let col = fade(pal.liquid, 0.25 + 0.75 * k);
                let half = w * 0.5;
                let (a, b) = to_px(x - half, *y);
                let (c, e) = to_px(x + half, *y);
                ink.line(a, b, c, e, 1, col);
                let (a, b) = to_px(x - half * 0.7, y + 0.25);
                let (c, e) = to_px(x + half * 0.7, y + 0.25);
                ink.line(a, b, c, e, 1, fade(col, 0.7));
            }
            _ => {}
        }
    }

    for rock in game.rocks.iter().filter(|r| r.live) {
        let aimed = Some(rock.id) == game.target
            || (!game.buffer.is_empty() && rock.word.starts_with(&game.buffer));
        let col = if rock.hit > 0.0 {
            pal.hot
        } else if aimed {
            pal.amber
        } else {
            pal.rock
        };
        let pri = if aimed { 2 } else { 1 };

        if game.theme == Theme::Underwater {
            // Six short horns.
            for i in 0..6 {
                let a = rock.rot + i as f32 / 6.0 * std::f32::consts::TAU;
                let (x0, y0) = to_px(
                    rock.x + a.cos() * rock.radius * 0.95,
                    rock.y + a.sin() * rock.radius * 0.95,
                );
                let (x1, y1) = to_px(
                    rock.x + a.cos() * rock.radius * 1.25,
                    rock.y + a.sin() * rock.radius * 1.25,
                );
                ink.line(x0, y0, x1, y1, pri, col);
            }
        }

        match game.theme {
            Theme::Jungle => draw_coconut(&mut ink, &to_px, rock, (pri, col), pal),
            Theme::CatCafe => draw_cup(&mut ink, &to_px, rock, (pri, col), (pal, game.time), aimed),
            Theme::Space | Theme::Underwater => {
                let n = rock.shape.len();
                for i in 0..n {
                    let (ax, ay) = rock.shape[i];
                    let (bx, by) = rock.shape[(i + 1) % n];
                    let (ax, ay) = rotate(ax, ay, rock.rot);
                    let (bx, by) = rotate(bx, by, rock.rot);
                    let (x0, y0) = to_px(rock.x + ax, rock.y + ay);
                    let (x1, y1) = to_px(rock.x + bx, rock.y + by);
                    ink.line(x0, y0, x1, y1, pri, col);
                }
            }
        }
    }

    let blink_off = game.invuln > 0.0 && ((game.time * 14.0) as i32).rem_euclid(2) == 0;
    if !blink_off && game.phase != Phase::Shop {
        ships::draw_ship(&mut ink, game, pal, to_px);
    }

    for y in 0..ink.h {
        for x in 0..ink.w {
            let i = y * ink.w + x;
            if ink.bits[i] == 0 {
                continue;
            }
            let ch = char::from_u32(0x2800 + ink.bits[i] as u32).unwrap_or(' ');
            put(
                buf,
                x as i32 + ox,
                y as i32 + oy,
                &ch.to_string(),
                style(ink.col[i]),
            );
        }
    }

    if game.phase != Phase::Title {
        for rock in game.rocks.iter().filter(|r| r.live) {
            draw_label(
                buf,
                game,
                pal,
                (rock.x + shx, rock.y + shy),
                // Squashed hazard art sits closer to its center.
                match game.theme {
                    Theme::Jungle | Theme::CatCafe => rock.radius * 0.75,
                    _ => rock.radius,
                },
                &rock.word,
                oy,
            );
        }
    }

    for gem in &game.gems {
        let glint = ((game.time * 9.0 + gem.x).sin() > 0.5) as usize;
        put(
            buf,
            (gem.x + shx).round() as i32,
            (gem.y + shy).round() as i32 + oy,
            ["◆", "◇"][glint],
            style(pal.gem).add_modifier(Modifier::BOLD),
        );
    }

    for effect in &game.effects {
        if let Effect::Spark {
            x,
            y,
            glyph,
            life,
            max,
            ..
        } = effect
        {
            let fade = if *max > 0.0 { *life / *max } else { 0.0 };
            let col = if fade > 0.45 { pal.hot } else { pal.accent };
            put(
                buf,
                (x + shx).round() as i32,
                (y + shy).round() as i32 + oy,
                &glyph.to_string(),
                style(col),
            );
        }
        if let Effect::Float {
            x,
            y,
            text,
            life,
            max,
        } = effect
        {
            if *life > 0.0 {
                let col = if *life / *max > 0.4 {
                    pal.amber
                } else {
                    pal.dim
                };
                put(
                    buf,
                    x.round() as i32,
                    y.round() as i32 + oy,
                    text,
                    style(col).add_modifier(Modifier::BOLD),
                );
            }
        }
    }

    if game.banner_t > 0.0 && !game.banner.is_empty() {
        let col = if game.banner_t > 0.4 {
            pal.hot
        } else {
            pal.dim
        };
        let x = ((m.play_w as usize).saturating_sub(game.banner.len())) as i32 / 2;
        let y = (m.play_h as i32 / 2) - 2;
        put(
            buf,
            x.max(1),
            y.max(0) + oy,
            &game.banner,
            style(col).add_modifier(Modifier::BOLD),
        );
    }

    if game.hurt > 0.0 {
        hline(buf, 0, w, m.origin_y, pal.danger);
        hline(buf, 0, w, m.dock_y.saturating_sub(1), pal.danger);
    }

    draw_header(buf, game, pal, w);
    if matches!(game.phase, Phase::Playing | Phase::Paused | Phase::Dying) {
        stage::draw_stage(buf, game, pal, m);
    }
    draw_dock(buf, game, pal, m);
    // Menus and the shop use the stage rows too; only play needs them.
    let menu = Metrics {
        play_h: m.play_h + m.stage_h,
        ..m
    };
    draw_title(buf, game, pal, w, menu);
    draw_shop(buf, game, pal, w, menu);
    draw_modal(buf, game, pal, w, menu);
}

fn draw_label(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    pos: (f32, f32),
    radius: f32,
    word: &str,
    origin_y: i32,
) {
    let (x, y) = pos;
    // One padding cell each side; the chip is word + 2 wide.
    let width = word.len() as i32 + 2;
    let max_left = (game.play_w as i32 - width).max(0);
    let left = (x.round() as i32 - width / 2).clamp(0, max_left);
    let mut row = (y + radius + 0.85).round() as i32;
    if row >= game.play_h as i32 - 1 {
        row = (y - radius - 1.2).round() as i32;
    }
    let matched = if !game.buffer.is_empty() && word.starts_with(&game.buffer) {
        game.buffer.len()
    } else {
        0
    };
    let locked = game.target_word() == Some(word);
    let bg = if locked {
        mix(pal.chip, pal.amber, 0.16)
    } else {
        pal.chip
    };
    let row = row + origin_y;
    let pad = if locked {
        style(pal.amber).bg(bg).add_modifier(Modifier::BOLD)
    } else {
        style(pal.dim).bg(bg)
    };
    put(buf, left, row, if locked { "‹" } else { " " }, pad);
    for (i, ch) in word.chars().enumerate() {
        let st = if i < matched {
            style(pal.amber).add_modifier(Modifier::BOLD)
        } else if locked {
            style(pal.ink)
        } else {
            style(pal.dim)
        };
        put(buf, left + 1 + i as i32, row, &ch.to_string(), st.bg(bg));
    }
    put(
        buf,
        left + width - 1,
        row,
        if locked { "›" } else { " " },
        pad,
    );
}

fn draw_header(buf: &mut ratatui::buffer::Buffer, game: &Game, pal: &Palette, w: u16) {
    let mut x = write(
        buf,
        1,
        0,
        &format!("WAVE {:02}", game.wave.max(1)),
        style(pal.dim),
    );
    x = write(
        buf,
        x,
        0,
        &format!("   {:06}", game.score),
        style(pal.amber).add_modifier(Modifier::BOLD),
    );
    x = write(buf, x, 0, "   ", style(pal.dim));
    let max = game.max_lives();
    for i in 0..max {
        let (glyph, col) = if i < game.lives {
            ("♥", pal.danger)
        } else {
            ("♡", pal.dead)
        };
        x = write(buf, x, 0, glyph, style(col));
        if i + 1 != max {
            x = write(buf, x, 0, " ", style(pal.dim));
        }
    }
    if game.shield > 0 {
        x = write(buf, x, 0, &format!(" ◈{}", game.shield), style(pal.accent));
    }
    let gem_col = if game.gem_flash > 0.0 {
        pal.hot
    } else {
        pal.gem
    };
    x = write(
        buf,
        x,
        0,
        &format!("   ◆ {}", game.crystals),
        style(gem_col).add_modifier(Modifier::BOLD),
    );
    if w >= 60 && game.combo >= 2 {
        let _ = write(
            buf,
            x,
            0,
            &format!("   x{}", game.combo),
            style(pal.amber).add_modifier(Modifier::BOLD),
        );
    }
    if w >= 72 {
        let best = format!("BEST {:06}", game.best);
        let bx = w.saturating_sub(best.chars().count() as u16 + 1);
        let _ = write(buf, bx, 0, &best, style(pal.faint));
    }
}

fn game_over_label(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "SHIP LOST",
        Theme::Underwater => "HULL BREACH",
        Theme::Jungle => "MONKEY DOWN",
        Theme::CatCafe => "CAT NAPPING",
    }
}

fn play_hint(game: &Game, word: &str, nmatch: usize) -> String {
    match game.theme {
        Theme::Space => {
            if game.banner_t > 0.0 && game.banner == "WAVE CLEAR" {
                "wave clear — next rocks inbound".to_string()
            } else if word.is_empty() {
                "type a word to aim the ship".to_string()
            } else if nmatch > 1 {
                "nearest rock is locked — keep typing to pin it".to_string()
            } else {
                "finish the word to detonate".to_string()
            }
        }
        Theme::Underwater => {
            if game.banner_t > 0.0 && game.banner == "MINEFIELD CLEAR" {
                "minefield clear — next mines inbound".to_string()
            } else if word.is_empty() {
                "type a word to arm the torpedo".to_string()
            } else if nmatch > 1 {
                "nearest mine is locked — keep typing to pin it".to_string()
            } else {
                "finish the word to detonate".to_string()
            }
        }
        Theme::Jungle => {
            if game.banner_t > 0.0 && game.banner == "CANOPY CLEAR" {
                "canopy clear — more coconuts inbound".to_string()
            } else if word.is_empty() {
                "type a word to aim the monkey".to_string()
            } else if nmatch > 1 {
                "nearest coconut is locked — keep typing to pin it".to_string()
            } else {
                "finish the word to throw the banana".to_string()
            }
        }
        Theme::CatCafe => {
            if game.banner_t > 0.0 && game.banner == "CAFE CLEAR" {
                "cafe clear — more coffee brewing".to_string()
            } else if word.is_empty() {
                "type a word to ready the cat".to_string()
            } else if nmatch > 1 {
                "nearest cup is locked — keep typing to pin it".to_string()
            } else {
                "finish the word to pounce and scratch".to_string()
            }
        }
    }
}

/// Two rows: an input line and one line of context. No rules or boxes.
fn draw_dock(buf: &mut ratatui::buffer::Buffer, game: &Game, pal: &Palette, m: Metrics) {
    let dock_y = m.dock_y;
    let y1 = dock_y as i32;
    let y2 = dock_y as i32 + 1;
    let bold = Modifier::BOLD;

    match game.phase {
        Phase::Title => {
            put(
                buf,
                2,
                y1,
                "h j k l / arrows move  ·  enter select  ·  q quits",
                style(pal.dim),
            );
            let line = if game.theme_menu_open {
                "ENTER applies the theme. ESC goes back."
            } else {
                match game.theme {
                    Theme::Space => "START launches. THEMES changes the look.",
                    Theme::Underwater => "START dives. THEMES changes the look.",
                    Theme::Jungle => "START swings. THEMES changes the look.",
                    Theme::CatCafe => "START purrs. THEMES changes the look.",
                }
            };
            put(buf, 2, y2, line, style(pal.amber));
        }
        Phase::Paused => {
            let x = write(
                buf,
                2,
                dock_y,
                "PAUSED",
                style(pal.amber).add_modifier(bold),
            );
            put(
                buf,
                x as i32 + 2,
                y1,
                "esc resumes  ·  q saves and quits",
                style(pal.dim),
            );
        }
        Phase::Shop => {
            put(
                buf,
                2,
                y1,
                "h l move  ·  enter select  ·  1 2 3 buy  ·  r reroll  ·  space go",
                style(pal.dim),
            );
            put(buf, 2, y2, &game.shop_note, style(pal.amber));
        }
        Phase::GameOver => {
            let x = write(
                buf,
                2,
                dock_y,
                game_over_label(game.theme),
                style(pal.danger).add_modifier(bold),
            );
            put(
                buf,
                x as i32 + 2,
                y1,
                &format!("SCORE {:06}   BEST {:06}", game.score, game.best),
                style(pal.ink),
            );
            put(buf, 2, y2, "enter selects  ·  q quits", style(pal.dim));
        }
        Phase::Dying => {
            put(
                buf,
                2,
                y1,
                game_over_label(game.theme),
                style(pal.danger).add_modifier(bold),
            );
        }
        Phase::Playing => {
            let word = game.target_word().unwrap_or("");
            let nmatch = game.matches();
            if m.stage_h > 0 {
                // The stage shows the word; the dock only reports problems.
                if game.miss > 0.08 && game.buffer.is_empty() {
                    put(
                        buf,
                        2,
                        y1,
                        miss_message(game.theme),
                        style(pal.danger).add_modifier(bold),
                    );
                } else if nmatch > 1 {
                    put(buf, 2, y1, &format!("{nmatch} match"), style(pal.dim));
                }
            } else {
                draw_input_line(buf, game, pal, dock_y);
            }
            put(buf, 2, y2, &play_hint(game, word, nmatch), style(pal.dim));
        }
    }
}

fn miss_message(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "no rock starts with that",
        Theme::Underwater => "no mine starts with that",
        Theme::Jungle => "no coconut starts with that",
        Theme::CatCafe => "no coffee starts with that",
    }
}

/// The one-line input used when the terminal is too short for the stage.
fn draw_input_line(buf: &mut ratatui::buffer::Buffer, game: &Game, pal: &Palette, dock_y: u16) {
    let y1 = dock_y as i32;
    let bold = Modifier::BOLD;
    let word = game.target_word().unwrap_or("");
    let nmatch = game.matches();
    let missed = game.miss > 0.0;
    let accent = if missed { pal.danger } else { pal.amber };
    put(buf, 2, y1, "›", style(accent).add_modifier(bold));
    if game.miss > 0.08 && game.buffer.is_empty() {
        put(
            buf,
            4,
            y1,
            miss_message(game.theme),
            style(pal.danger).add_modifier(bold),
        );
        return;
    }
    let typed_col = if missed { pal.danger } else { pal.hot };
    let mut x = write(
        buf,
        4,
        dock_y,
        &game.buffer,
        style(typed_col).add_modifier(bold),
    );
    let cursor = if ((game.time * 3.0) as i32).rem_euclid(2) == 0 {
        "█"
    } else {
        " "
    };
    x = write(buf, x, dock_y, cursor, style(accent));
    // The rest of the locked word, ghosted after the cursor.
    let rest = word.get(game.buffer.len()..).unwrap_or("");
    x = write(buf, x, dock_y, rest, style(pal.faint));
    if nmatch > 1 {
        put(
            buf,
            x as i32 + 2,
            y1,
            &format!("{nmatch} match"),
            style(pal.dim),
        );
    }
}

fn draw_title(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    w: u16,
    m: crate::game::Metrics,
) {
    if game.phase != Phase::Title {
        return;
    }

    let lines = title_lines(w >= 80);
    let art_h = lines.len() as i32;
    let art_w = lines[0].chars().count() as i32;
    let block_h = art_h + if game.theme_menu_open { 7 } else { 5 };
    let top = m.origin_y as i32 + (m.play_h as i32 - block_h).max(0) / 2;
    let left = (w as i32 - art_w) / 2;

    // Offset shadow first, then the bright face on top.
    for (i, line) in lines.iter().enumerate() {
        let y = top + i as i32;
        put(buf, left + 1, y + 1, line, style(pal.shadow));
        put(
            buf,
            left,
            y,
            line,
            style(pal.ink).add_modifier(Modifier::BOLD),
        );
    }

    let sub = match game.theme {
        Theme::Space => "typing range",
        Theme::Underwater => "~ sonar range ~",
        Theme::Jungle => "~ canopy range ~",
        Theme::CatCafe => "~ cozy cat cafe ~",
    };
    let sx = (w as i32 - sub.len() as i32) / 2;
    let sub_y = top + art_h + 1;
    clear_span(buf, sx - 2, sub_y, sub.len() as i32 + 4);
    put(buf, sx, sub_y, sub, style(pal.amber));

    let theme_line = if game.theme_menu_open {
        "SELECT THEME".to_string()
    } else {
        format!("THEME  {}", game.theme.label())
    };
    let tx = (w as i32 - theme_line.len() as i32) / 2;
    let theme_y = top + art_h + 2;
    clear_span(buf, tx - 2, theme_y, theme_line.len() as i32 + 4);
    put(
        buf,
        tx,
        theme_y,
        &theme_line,
        style(if game.theme_menu_open {
            pal.amber
        } else {
            pal.faint
        }),
    );

    let labels: Vec<String> = game
        .menu_items()
        .iter()
        .map(|item| format!("[ {item} ]"))
        .collect();
    let gap = "    ";
    let total = labels
        .iter()
        .map(|label| label.chars().count() as i32)
        .sum::<i32>()
        + gap.len() as i32 * (labels.len().saturating_sub(1)) as i32;
    let menu_x = (w as i32 - total) / 2;
    let menu_y = top + art_h + 3;
    clear_span(buf, menu_x - 3, menu_y, total + 6);
    let mut x = menu_x;
    for (i, label) in labels.iter().enumerate() {
        let st = if i == game.menu && !game.theme_menu_open {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else {
            style(pal.dim)
        };
        put(buf, x, menu_y, label, st);
        x += label.chars().count() as i32 + gap.len() as i32;
    }

    if game.theme_menu_open {
        let choices: Vec<String> = Theme::ALL
            .iter()
            .map(|theme| format!("[ {} ]", theme.label()))
            .collect();
        let choice_gap = "   ";
        let choice_total = choices
            .iter()
            .map(|label| label.chars().count() as i32)
            .sum::<i32>()
            + choice_gap.len() as i32 * (choices.len().saturating_sub(1)) as i32;
        let choice_x = (w as i32 - choice_total) / 2;
        let choice_y = top + art_h + 5;
        clear_span(buf, choice_x - 3, choice_y, choice_total + 6);
        let mut cx = choice_x;
        for (i, label) in choices.iter().enumerate() {
            let st = if i == game.theme_choice {
                style(Color::Rgb(20, 16, 8))
                    .bg(pal.ink)
                    .add_modifier(Modifier::BOLD)
            } else {
                style(pal.dim)
            };
            put(buf, cx, choice_y, label, st);
            cx += label.chars().count() as i32 + choice_gap.len() as i32;
        }
    }
}

fn draw_modal(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    w: u16,
    m: crate::game::Metrics,
) {
    if !matches!(game.phase, Phase::Paused | Phase::GameOver) {
        return;
    }

    let is_over = game.phase == Phase::GameOver;
    let (heading, heading_col) = if is_over {
        (
            match game.theme {
                Theme::Space => "SHIP LOST",
                Theme::Underwater => "HULL BREACH",
                Theme::Jungle => "MONKEY DOWN",
                Theme::CatCafe => "CAT NAPPING",
            },
            pal.danger,
        )
    } else {
        ("PAUSED", pal.amber)
    };

    let items = game.menu_items();
    let labels: Vec<String> = items.iter().map(|item| format!("[ {item} ]")).collect();
    let gap = "   ";
    let menu_w = labels
        .iter()
        .map(|label| label.chars().count())
        .sum::<usize>()
        + gap.len() * labels.len().saturating_sub(1);

    let theme_line = if game.theme_menu_open {
        "SELECT THEME".to_string()
    } else {
        format!("THEME  {}", game.theme.label())
    };
    let score_line = if is_over {
        format!("SCORE {:06}    BEST {:06}", game.score, game.best)
    } else {
        String::new()
    };
    let hint = if game.theme_menu_open {
        "enter applies theme    esc backs out"
    } else if is_over {
        "h j k l / arrows move    enter select"
    } else {
        "h j k l / arrows move    enter select    esc resumes"
    };

    let choices: Vec<String> = Theme::ALL
        .iter()
        .map(|theme| format!("[ {} ]", theme.label()))
        .collect();
    let choice_gap = "   ";
    let choice_w = if game.theme_menu_open {
        choices
            .iter()
            .map(|label| label.chars().count())
            .sum::<usize>()
            + choice_gap.len() * choices.len().saturating_sub(1)
    } else {
        0
    };

    let inner_w = heading
        .chars()
        .count()
        .max(menu_w)
        .max(theme_line.chars().count())
        .max(score_line.chars().count())
        .max(choice_w)
        .max(hint.chars().count());
    let box_w = inner_w + 4;
    if box_w > w as usize {
        return;
    }

    // Lay out the rows inside the border.
    let mut rel = 1usize;
    let heading_row = rel;
    rel += 2; // heading + blank
    let score_row = if is_over {
        let row = rel;
        rel += 2; // score + blank
        Some(row)
    } else {
        None
    };
    let theme_row = rel;
    rel += 2; // theme + blank
    let menu_row = rel;
    rel += 1;
    let choices_row = if game.theme_menu_open {
        rel += 1; // blank
        let row = rel;
        rel += 1;
        Some(row)
    } else {
        None
    };
    rel += 1; // blank
    let hint_row = rel;
    rel += 1;
    let box_h = rel + 2;

    let left = (w as usize - box_w) / 2;
    let top = m.origin_y as usize + (m.play_h as usize).saturating_sub(box_h) / 2;
    let bg = mix(pal.bg_top, pal.bg_bottom, 0.22);

    for y in top..top + box_h {
        let blank = " ".repeat(box_w);
        put(buf, left as i32, y as i32, &blank, Style::default().bg(bg));
    }

    let border = style(pal.dim).bg(bg);
    let top_line = format!("┌{}┐", "─".repeat(box_w - 2));
    let bottom_line = format!("└{}┘", "─".repeat(box_w - 2));
    put(buf, left as i32, top as i32, &top_line, border);
    put(
        buf,
        left as i32,
        (top + box_h - 1) as i32,
        &bottom_line,
        border,
    );
    for y in top + 1..top + box_h - 1 {
        put(buf, left as i32, y as i32, "│", border);
        put(buf, (left + box_w - 1) as i32, y as i32, "│", border);
    }

    let inner_x = (left + 2) as i32;
    let inner_width = (box_w - 4) as i32;
    put_centered(
        buf,
        inner_x,
        inner_width,
        (top + heading_row) as i32,
        heading,
        style(heading_col).bg(bg).add_modifier(Modifier::BOLD),
    );
    if let Some(row) = score_row {
        put_centered(
            buf,
            inner_x,
            inner_width,
            (top + row) as i32,
            &score_line,
            style(pal.ink).bg(bg),
        );
    }
    put_centered(
        buf,
        inner_x,
        inner_width,
        (top + theme_row) as i32,
        &theme_line,
        style(if game.theme_menu_open {
            pal.amber
        } else {
            pal.faint
        })
        .bg(bg),
    );

    let mut x = inner_x + (inner_width - menu_w as i32).max(0) / 2;
    for (i, label) in labels.iter().enumerate() {
        let st = if !game.theme_menu_open && i == game.menu {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else {
            style(pal.dim).bg(bg)
        };
        put(buf, x, (top + menu_row) as i32, label, st);
        x += label.chars().count() as i32 + gap.len() as i32;
    }

    if let Some(row) = choices_row {
        let mut x = inner_x + (inner_width - choice_w as i32).max(0) / 2;
        for (i, label) in choices.iter().enumerate() {
            let st = if i == game.theme_choice {
                style(Color::Rgb(20, 16, 8))
                    .bg(pal.ink)
                    .add_modifier(Modifier::BOLD)
            } else {
                style(pal.dim).bg(bg)
            };
            put(buf, x, (top + row) as i32, label, st);
            x += label.chars().count() as i32 + choice_gap.len() as i32;
        }
    }

    put_centered(
        buf,
        inner_x,
        inner_width,
        (top + hint_row) as i32,
        hint,
        style(pal.faint).bg(bg),
    );
}

fn rarity_color(rarity: Rarity, pal: &Palette) -> Color {
    match rarity {
        Rarity::Common => pal.dim,
        Rarity::Uncommon => pal.accent,
        Rarity::Rare => pal.amber,
    }
}

/// Greedy word wrap.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

fn shop_name(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "SUPPLY DEPOT",
        Theme::Underwater => "DOCKYARD",
        Theme::Jungle => "TRADING POST",
        Theme::CatCafe => "THE COUNTER",
    }
}

/// Fill a rectangle with a background and trace its border.
fn draw_frame(
    buf: &mut ratatui::buffer::Buffer,
    rect: (i32, i32, i32, i32),
    glyphs: [&str; 6],
    st: Style,
    fill: Color,
) {
    let (x, y, w, h) = rect;
    let [tl, tr, bl, br, horiz, vert] = glyphs;
    let blank = " ".repeat(w.max(0) as usize);
    for row in 0..h {
        put(buf, x, y + row, &blank, Style::default().bg(fill));
    }
    let st = st.bg(fill);
    for col in 1..w - 1 {
        put(buf, x + col, y, horiz, st);
        put(buf, x + col, y + h - 1, horiz, st);
    }
    for row in 1..h - 1 {
        put(buf, x, y + row, vert, st);
        put(buf, x + w - 1, y + row, vert, st);
    }
    put(buf, x, y, tl, st);
    put(buf, x + w - 1, y, tr, st);
    put(buf, x, y + h - 1, bl, st);
    put(buf, x + w - 1, y + h - 1, br, st);
}

fn draw_shop(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    w: u16,
    m: crate::game::Metrics,
) {
    let Some(shop) = game.shop.as_ref().filter(|_| game.phase == Phase::Shop) else {
        return;
    };
    let gap = 2;
    let cols = SHOP_SLOTS as i32;
    let card_w = ((w as i32 - 4 - gap * (cols - 1)) / cols).clamp(14, 24);
    let total = card_w * cols + gap * (cols - 1);
    let left = (w as i32 - total) / 2;
    let card_h = (m.play_h as i32 - 7).clamp(7, 10);
    let block_h = card_h + 6;
    let top = m.origin_y as i32 + (m.play_h as i32 - block_h).max(0) / 2;

    // Heading: shop name, wave, and the purse.
    let heading = format!(
        "{}  ·  WAVE {} CLEARED",
        shop_name(game.theme),
        game.wave.saturating_sub(1)
    );
    put_centered(
        buf,
        0,
        w as i32,
        top,
        &heading,
        style(pal.amber).add_modifier(Modifier::BOLD),
    );
    put_centered(
        buf,
        0,
        w as i32,
        top + 1,
        &format!("◆ {} crystals", game.crystals),
        style(pal.gem).add_modifier(Modifier::BOLD),
    );

    let cards_y = top + 3;
    for slot in 0..SHOP_SLOTS {
        let x = left + slot as i32 * (card_w + gap);
        let Some(offer) = shop.offers.get(slot) else {
            // Nothing left to sell: keep the slot on screen, clearly empty.
            let selected = game.shop_sel == slot;
            let edge = if selected { pal.hot } else { pal.faint };
            draw_frame(
                buf,
                (x, cards_y, card_w, card_h),
                ["┌", "┐", "└", "┘", "─", "│"],
                style(edge),
                pal.chip,
            );
            put_centered(
                buf,
                x,
                card_w,
                cards_y + card_h / 2,
                "SOLD OUT",
                Style::default()
                    .fg(pal.dim)
                    .bg(pal.chip)
                    .add_modifier(Modifier::BOLD),
            );
            continue;
        };
        let def = offer.upgrade.def();
        let selected = game.shop_sel == slot;
        let rare = def.rarity == Rarity::Rare;
        let edge = if offer.sold {
            pal.dim
        } else if selected {
            pal.hot
        } else {
            rarity_color(def.rarity, pal)
        };
        let glyphs = match (selected, rare) {
            (_, true) => ["╔", "╗", "╚", "╝", "═", "║"],
            (true, false) => ["┏", "┓", "┗", "┛", "━", "┃"],
            (false, false) => ["┌", "┐", "└", "┘", "─", "│"],
        };
        let fill = if selected {
            mix(pal.chip, pal.amber, 0.14)
        } else {
            pal.chip
        };
        draw_frame(
            buf,
            (x, cards_y, card_w, card_h),
            glyphs,
            style(edge).add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
            fill,
        );
        let inner = (card_w - 4) as usize;
        let body = |col: Color| style(col).bg(fill);
        let name = format!("{} {}", slot + 1, def.name);
        let name: String = name.chars().take(inner).collect();
        let name_col = if offer.sold { pal.dim } else { pal.ink };
        put(
            buf,
            x + 2,
            cards_y + 1,
            &name,
            body(name_col).add_modifier(Modifier::BOLD),
        );
        let tag = if rare {
            format!("★ {}", def.rarity.label())
        } else {
            def.rarity.label().to_string()
        };
        put(
            buf,
            x + 2,
            cards_y + 2,
            &tag,
            body(if offer.sold {
                pal.dim
            } else {
                rarity_color(def.rarity, pal)
            }),
        );
        let desc_rows = (card_h - 6).max(1) as usize;
        let blurb = offer
            .upgrade
            .blurb(game.upgrades.level(offer.upgrade).saturating_add(1));
        for (i, line) in wrap(&blurb, inner).iter().take(desc_rows).enumerate() {
            put(
                buf,
                x + 2,
                cards_y + 4 + i as i32,
                line,
                body(if offer.sold { pal.faint } else { pal.dim }),
            );
        }
        let foot_y = cards_y + card_h - 2;
        if offer.sold {
            put(
                buf,
                x + 2,
                foot_y,
                "✓ OWNED",
                body(pal.accent).add_modifier(Modifier::BOLD),
            );
        } else {
            let cost = game.upgrade_cost(offer.upgrade);
            let afford = game.crystals >= cost;
            let price = format!("◆ {cost}");
            let px = x + card_w - 2 - price.chars().count() as i32;
            put(
                buf,
                px,
                foot_y,
                &price,
                body(if afford { pal.gem } else { pal.danger }).add_modifier(Modifier::BOLD),
            );
            let held = game.upgrades.level(offer.upgrade);
            if held > 0 && !def.consumable {
                put(
                    buf,
                    x + 2,
                    foot_y,
                    &format!("owned {held}"),
                    body(pal.faint),
                );
            }
        }
    }

    // Reroll and next-wave buttons.
    let reroll_cost = game.reroll_cost();
    let reroll = if reroll_cost == 0 {
        "[ REROLL free ]".to_string()
    } else {
        format!("[ REROLL ◆ {reroll_cost} ]")
    };
    let go = "[ NEXT WAVE ]".to_string();
    let btn_gap = 4;
    let btn_total = (reroll.chars().count() + go.chars().count()) as i32 + btn_gap;
    let mut bx = (w as i32 - btn_total) / 2;
    let by = cards_y + card_h + 1;
    for (idx, label) in [(SHOP_REROLL, reroll), (SHOP_GO, go)] {
        let selected = game.shop_sel == idx;
        let st = if selected {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else {
            style(pal.ink)
        };
        put(buf, bx, by, &label, st);
        bx += label.chars().count() as i32 + btn_gap;
    }

    // What the run holds so far.
    let owned: Vec<String> = game
        .upgrades
        .held()
        .map(|(up, level)| {
            if up.def().max > 1 {
                format!("{} {level}", up.def().name)
            } else {
                up.def().name.to_string()
            }
        })
        .collect();
    let line = if owned.is_empty() {
        "nothing installed yet".to_string()
    } else {
        owned.join("  ·  ")
    };
    let room = (w as usize).saturating_sub(4);
    let line: String = if line.chars().count() > room {
        line.chars()
            .take(room.saturating_sub(1))
            .chain(['…'])
            .collect()
    } else {
        line
    };
    put_centered(buf, 0, w as i32, by + 2, &line, style(pal.faint));
}

fn put_centered(
    buf: &mut ratatui::buffer::Buffer,
    x0: i32,
    width: i32,
    y: i32,
    text: &str,
    st: Style,
) {
    let len = text.chars().count() as i32;
    put(buf, x0 + (width - len).max(0) / 2, y, text, st);
}

const TITLE_GLYPHS: [[u8; 5]; 7] = [
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100], // T
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100], // Y
    [0b11110, 0b10001, 0b11110, 0b10000, 0b10000], // P
    [0b01110, 0b10001, 0b10001, 0b10001, 0b01110], // O
    [0b11111, 0b00100, 0b00100, 0b00100, 0b11111], // I
    [0b11110, 0b10001, 0b10001, 0b10001, 0b11110], // D
    [0b11111, 0b10000, 0b01110, 0b00001, 0b11111], // S
];

fn title_lines(wide: bool) -> Vec<String> {
    let mut lines = Vec::with_capacity(TITLE_GLYPHS[0].len());
    for row in 0..TITLE_GLYPHS[0].len() {
        let mut line = String::new();
        for (i, glyph) in TITLE_GLYPHS.iter().enumerate() {
            if i > 0 {
                line.push(' ');
            }
            for col in (0..5).rev() {
                if glyph[row] & (1 << col) != 0 {
                    line.push('█');
                    if wide {
                        line.push('█');
                    }
                } else {
                    line.push(' ');
                    if wide {
                        line.push(' ');
                    }
                }
            }
        }
        lines.push(line);
    }
    lines
}

fn hline(buf: &mut ratatui::buffer::Buffer, x0: u16, x1: u16, y: u16, col: Color) {
    for x in x0..x1 {
        put(buf, x as i32, y as i32, "─", style(col));
    }
}

fn write(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, text: &str, style: Style) -> u16 {
    put(buf, x as i32, y as i32, text, style);
    x.saturating_add(text.chars().count() as u16)
}

fn put(buf: &mut ratatui::buffer::Buffer, x: i32, y: i32, text: &str, style: Style) {
    if x < 0 || y < 0 {
        return;
    }
    let x = x as u16;
    let y = y as u16;
    if y >= buf.area.height || x >= buf.area.width {
        return;
    }
    buf.set_string(x, y, text, style);
}

fn clear_span(buf: &mut ratatui::buffer::Buffer, x: i32, y: i32, width: i32) {
    if width <= 0 {
        return;
    }
    let blank = " ".repeat(width as usize);
    put(buf, x, y, &blank, Style::default());
}

fn style(fg: Color) -> Style {
    Style::default().fg(fg)
}

fn paint_water(buf: &mut ratatui::buffer::Buffer, area: Rect, pal: &Palette) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let span = area.height.saturating_sub(1).max(1) as f32;
    for y in 0..area.height {
        let t = y as f32 / span;
        let col = mix(pal.bg_top, pal.bg_bottom, t);
        buf.set_style(
            Rect::new(area.x, area.y + y, area.width, 1),
            Style::default().bg(col),
        );
    }
}

/// A shaded canopy: deep green gradient with subtle, soft dappled shading
/// and a dark vignette that stays calm and readable behind gameplay.
fn paint_jungle_canopy(buf: &mut ratatui::buffer::Buffer, area: Rect, pal: &Palette, _time: f32) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let w = area.width as f32;
    let h = area.height.max(1) as f32;
    let leaf_shade = fade(pal.shadow, 0.4);
    let deep = pal.bg_bottom;
    for y in 0..area.height {
        let ty = y as f32 / h;
        let base = mix(pal.bg_top, pal.bg_bottom, ty.powf(0.82));
        let ceiling = (1.0 - ty * 5.5).max(0.0);
        for x in 0..area.width {
            let fx = x as f32 / w;

            // Soft, static dappled pools of shade from the canopy above.
            let dapple = (fx * 6.0).sin() * (ty * 4.0 + 0.8).cos();
            let dappled = (dapple * 0.5 + 0.5).powi(2);

            // Shadowed ceiling and a soft vignette toward the sides.
            let edge = (fx - 0.5).abs() * 2.0;
            let edge = edge * edge * edge;

            let mut col = mix(base, leaf_shade, dappled * 0.08);
            col = mix(col, deep, (ceiling * 0.7 + edge * 0.3).min(1.0));
            buf[(area.x + x, area.y + y)].set_bg(col);
        }
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => Color::Rgb(
            (ar as f32 + (br as f32 - ar as f32) * t).round() as u8,
            (ag as f32 + (bg as f32 - ag as f32) * t).round() as u8,
            (ab as f32 + (bb as f32 - ab as f32) * t).round() as u8,
        ),
        _ => b,
    }
}

fn fade(color: Color, amount: f32) -> Color {
    let k = amount.clamp(0.0, 1.0);
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * k).round() as u8,
            (g as f32 * k).round() as u8,
            (b as f32 * k).round() as u8,
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{OVER_AGAIN, OVER_ITEMS, PAUSE_CONTINUE};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(game: &Game, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| draw(frame, game)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn paused_popup_shows_its_choices() {
        let mut game = Game::new(80, 24, 7);
        game.phase = Phase::Paused;
        game.menu = PAUSE_CONTINUE;
        let screen = render(&game, 80, 24);
        assert!(screen.contains("PAUSED"), "{screen}");
        for item in crate::game::PAUSE_ITEMS {
            assert!(screen.contains(item), "{item} missing\n{screen}");
        }
    }

    #[test]
    fn jungle_theme_renders_its_ship_and_hazards() {
        use crate::game::Rock;
        let mut game = Game::new(80, 24, 42);
        game.open_themes();
        game.theme_choice = Theme::Jungle.index();
        game.apply_theme();
        game.start();
        game.rocks.clear();
        game.effects.clear();
        game.banner.clear();
        game.banner_t = 0.0;
        game.phase = Phase::Playing;
        let shape = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                (a.cos() * 2.0, a.sin() * 2.0)
            })
            .collect();
        game.rocks.push(Rock {
            id: 99,
            x: 70.0,
            y: 8.0,
            vx: 0.0,
            vy: 0.0,
            radius: 2.0,
            rot: 0.2,
            spin: 0.0,
            shape,
            word: "coconut".into(),
            live: true,
            hit: 0.0,
        });
        game.type_char('c');
        for _ in 0..6 {
            game.update(1.0 / 60.0);
        }
        let screen = render(&game, 80, 24);
        assert!(screen.contains("coconut"), "{screen}");
        assert!(screen.contains("throw the banana"), "{screen}");
    }

    /// A started run in `theme` with one locked rock carrying `word`.
    fn locked_rock_game(theme: Theme, word: &str) -> Game {
        use crate::game::Rock;
        let mut game = Game::new(80, 24, 42);
        game.open_themes();
        game.theme_choice = theme.index();
        game.apply_theme();
        game.start();
        game.rocks.clear();
        game.effects.clear();
        game.banner.clear();
        game.banner_t = 0.0;
        game.phase = Phase::Playing;
        let shape = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                (a.cos() * 2.0, a.sin() * 2.0)
            })
            .collect();
        game.rocks.push(Rock {
            id: 99,
            x: 60.0,
            y: 8.0,
            vx: 0.0,
            vy: 0.0,
            radius: 2.0,
            rot: 0.2,
            spin: 0.0,
            shape,
            word: word.into(),
            live: true,
            hit: 0.0,
        });
        game.type_char(word.chars().next().unwrap());
        // Long enough for the stage animations to settle.
        for _ in 0..30 {
            game.update(1.0 / 60.0);
        }
        game
    }

    #[test]
    fn every_theme_has_one_line_hud_and_word_chips() {
        for theme in Theme::ALL {
            let game = locked_rock_game(theme, "comet");
            let screen = render(&game, 80, 24);
            let rows: Vec<&str> = screen.lines().collect();
            assert!(rows[0].contains("WAVE 01"), "{screen}");
            assert!(rows[0].contains("♥"), "{screen}");
            assert!(rows[0].contains("BEST"), "{screen}");
            assert!(!screen.contains("HULL"), "{screen}");
            assert!(screen.contains("‹comet›"), "locked chip\n{screen}");
            assert!(rows[23].contains("finish the word"), "{screen}");
        }
    }

    fn stage_rows(screen: &str, from: usize, to: usize) -> String {
        screen
            .lines()
            .skip(from)
            .take(to - from)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_stage_draws_the_locked_word_in_big_letters() {
        for theme in Theme::ALL {
            let game = locked_rock_game(theme, "comet");
            // 80x24: header, 16 play rows, a 5-row stage (rows 17..22), 2 dock rows.
            let screen = render(&game, 80, 24);
            let stage = stage_rows(&screen, 17, 21);
            assert!(stage.contains('█'), "typed letter\n{screen}");
            assert!(stage.contains('▀') || stage.contains('▄'), "{screen}");
            assert!(screen.lines().nth(21).unwrap().starts_with('─'), "{screen}");
            // Five letters of 10 columns each fill most of the row; the old
            // one-line input is gone.
            assert!(!screen.lines().nth(22).unwrap().contains('›'), "{screen}");
        }
    }

    #[test]
    fn tall_terminals_keep_the_same_small_stage() {
        let game = locked_rock_game(Theme::Space, "comet");
        let screen = render(&game, 80, 48);
        // 1 header, 40 play rows, a 5-row stage from row 41, 2 dock rows.
        let stage = stage_rows(&screen, 41, 45);
        assert!(stage.contains('█'), "{screen}");
        assert!(screen.lines().nth(45).unwrap().starts_with('─'), "{screen}");
    }

    #[test]
    fn short_terminals_keep_the_one_line_input() {
        let game = locked_rock_game(Theme::Space, "comet");
        let screen = render(&game, 70, 20);
        let rows: Vec<&str> = screen.lines().collect();
        assert!(
            rows[18].contains('›') && rows[18].contains("omet"),
            "{screen}"
        );
    }

    #[test]
    fn letters_rise_from_the_floor_after_being_typed() {
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.update(0.0);
        let settled = stage_rows(&render(&game, 80, 24), 17, 21);
        // Type the next letter: for an instant it is still below the floor.
        game.type_char('o');
        let fresh = stage_rows(&render(&game, 80, 24), 17, 21);
        let blocks = |s: &str| s.chars().filter(|c| "█▀▄".contains(*c)).count();
        assert!(blocks(&settled) > 0, "{settled}");
        game.update(0.3);
        let risen = stage_rows(&render(&game, 80, 24), 17, 21);
        assert!(blocks(&risen) > blocks(&fresh), "{risen}\n---\n{fresh}");
    }

    #[test]
    fn the_shop_shows_three_cards_and_its_buttons() {
        use crate::game::Offer;
        use crate::upgrades::Upgrade;
        for theme in Theme::ALL {
            let mut game = locked_rock_game(theme, "comet");
            game.rocks.clear();
            game.open_shop_for_test();
            game.crystals = 9;
            let shop = game.shop.as_mut().unwrap();
            shop.offers[0] = Offer {
                upgrade: Upgrade::SteadyHands,
                sold: false,
            };
            shop.offers[1] = Offer {
                upgrade: Upgrade::Multishot,
                sold: false,
            };
            shop.offers[2] = Offer {
                upgrade: Upgrade::SecondWind,
                sold: true,
            };
            let screen = render(&game, 80, 24);
            assert!(screen.contains("CLEARED"), "{screen}");
            assert!(screen.contains("1 STEADY HANDS"), "{screen}");
            assert!(screen.contains("2 MULTISHOT"), "{screen}");
            assert!(screen.contains("✓ OWNED"), "{screen}");
            assert!(screen.contains("REROLL"), "{screen}");
            assert!(screen.contains("NEXT WAVE"), "{screen}");
            assert!(screen.contains("◆ 9"), "{screen}");
            // Still readable on the smallest supported terminal.
            let small = render(&game, 70, 22);
            assert!(small.contains("NEXT WAVE"), "{small}");
        }
    }

    #[test]
    fn the_header_shows_crystals_and_extra_hearts() {
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.crystals = 42;
        game.upgrades.set(crate::upgrades::Upgrade::ExtraHeart, 2);
        let screen = render(&game, 80, 24);
        let top = screen.lines().next().unwrap();
        assert!(top.contains("◆ 42"), "{top}");
        assert_eq!(
            top.matches('♥').count() + top.matches('♡').count(),
            5,
            "{top}"
        );
    }

    #[test]
    fn an_empty_shop_still_draws_sold_out_slots() {
        let mut game = locked_rock_game(Theme::Underwater, "comet");
        game.rocks.clear();
        for up in crate::upgrades::Upgrade::ALL {
            game.upgrades.set(up, up.def().max);
        }
        game.lives = game.max_lives();
        game.open_shop_for_test();
        let screen = render(&game, 80, 24);
        assert_eq!(screen.matches("SOLD OUT").count(), 3, "{screen}");
        assert!(screen.contains("NEXT WAVE"), "{screen}");
        assert!(screen.contains("Sold out!"), "{screen}");
    }

    #[test]
    fn game_over_popup_shows_play_again_and_theme_menu() {
        let mut game = Game::new(80, 24, 7);
        game.phase = Phase::GameOver;
        game.menu = OVER_AGAIN;
        let screen = render(&game, 80, 24);
        for item in OVER_ITEMS {
            assert!(screen.contains(item), "{item} missing\n{screen}");
        }
        assert!(screen.contains("SCORE"), "{screen}");

        game.open_themes();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("SELECT THEME"), "{screen}");
        assert!(screen.contains("UNDERWATER"), "{screen}");
        assert!(screen.contains("CAT CAFE"), "{screen}");
    }

    #[test]
    fn cat_cafe_theme_renders_its_cat_and_hazards() {
        use crate::game::Rock;
        let mut game = Game::new(80, 24, 42);
        game.open_themes();
        game.theme_choice = Theme::CatCafe.index();
        game.apply_theme();
        game.start();
        game.rocks.clear();
        game.effects.clear();
        game.banner.clear();
        game.banner_t = 0.0;
        game.phase = Phase::Playing;
        let shape = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                (a.cos() * 2.0, a.sin() * 2.0)
            })
            .collect();
        game.rocks.push(Rock {
            id: 99,
            x: 70.0,
            y: 8.0,
            vx: 0.0,
            vy: 0.0,
            radius: 2.0,
            rot: 0.2,
            spin: 0.0,
            shape,
            word: "coffee".into(),
            live: true,
            hit: 0.0,
        });
        game.type_char('c');
        for _ in 0..6 {
            game.update(1.0 / 60.0);
        }
        let screen = render(&game, 80, 24);
        assert!(screen.contains("coffee"), "{screen}");
        assert!(screen.contains("pounce and scratch"), "{screen}");
    }

    #[test]
    fn jungle_leaves_only_fall_down_and_move_slowly() {
        let star_x = 40.0;
        let star_y = 5.0;
        let play_h = 24.0;

        for phase in [0.0, 1.0, 2.5, 4.0, 5.5] {
            let speed = jungle_leaf_speed(phase);
            assert!(
                speed <= 0.15,
                "speed {speed} should be much slower (<= 0.15)"
            );
            assert!(speed > 0.0, "speed {speed} must be positive");

            let (x0, y0, a0) = jungle_leaf_motion(star_x, star_y, phase, 0.0, play_h);
            let (x1, y1, a1) = jungle_leaf_motion(star_x, star_y, phase, 1.0, play_h);
            let (x2, y2, a2) = jungle_leaf_motion(star_x, star_y, phase, 5.0, play_h);

            // X must never sway or deviate horizontally: leaves only fall down the screen.
            assert_eq!(x0, star_x);
            assert_eq!(x1, star_x);
            assert_eq!(x2, star_x);

            // Angle must point straight down.
            assert_eq!(a0, 0.0);
            assert_eq!(a1, 0.0);
            assert_eq!(a2, 0.0);

            // Y must progress downwards.
            assert!(y1 > y0, "leaf should fall downward: y0={y0}, y1={y1}");
            assert!(y2 > y1, "leaf should fall downward: y1={y1}, y2={y2}");
        }
    }

    #[test]
    fn jungle_canopy_background_is_stable_over_time() {
        let area = Rect::new(0, 0, 80, 24);
        let pal = palette(Theme::Jungle);

        let mut buf0 = ratatui::buffer::Buffer::empty(area);
        paint_jungle_canopy(&mut buf0, area, pal, 0.0);

        let mut buf1 = ratatui::buffer::Buffer::empty(area);
        paint_jungle_canopy(&mut buf1, area, pal, 10.0);

        // Background colors should remain calm and stable across frames.
        for y in 0..area.height {
            for x in 0..area.width {
                assert_eq!(
                    buf0[(x, y)].bg,
                    buf1[(x, y)].bg,
                    "background at ({x}, {y}) should not flicker over time"
                );
            }
        }
    }
}
