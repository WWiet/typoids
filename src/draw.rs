//! Space, underwater, and jungle scenes. Ships, hazards, and projectiles are
//! braille line art; words sit on top in plain cells so they stay readable.

use crate::game::{lost_banner, metrics, rotate, Effect, Game, Hit, Metrics, Phase, Rock, Theme};
use crate::hazards::{Kind, MASK_SHOWN};
use crate::upgrades::{Rarity, Upgrade};

mod arcade;
mod bigfont;
mod bigtext;
mod bossfight;
mod ships;
mod stage;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;
use std::cell::RefCell;
use std::f32::consts::{PI, TAU};

/// A screen rectangle in cells: (x, y, width, height).
type Area = (i32, i32, i32, i32);

thread_local! {
    /// Where this frame put each clickable thing. Filled while drawing, read
    /// by `main.rs` through `hit_at`.
    static HITS: RefCell<Vec<(Area, Hit)>> = const { RefCell::new(Vec::new()) };
}

/// Record a clickable area. Later marks sit on top of earlier ones.
fn mark(area: Area, hit: Hit) {
    HITS.with(|h| h.borrow_mut().push((area, hit)));
}

/// What the mouse is over at cell (x, y) in the last frame drawn.
pub fn hit_at(x: u16, y: u16) -> Option<Hit> {
    let (x, y) = (x as i32, y as i32);
    HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|((ax, ay, w, hh), _)| x >= *ax && x < ax + w && y >= *ay && y < ay + hh)
            .map(|&(_, hit)| hit)
    })
}

#[derive(Clone, Copy)]
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

impl Palette {
    /// The palette washed toward its accent, for Overdrive.
    fn tinted(&self, k: f32) -> Palette {
        Palette {
            bg_top: mix(self.bg_top, self.accent, k),
            bg_bottom: mix(self.bg_bottom, self.accent, k * 0.6),
            chip: mix(self.chip, self.accent, k),
            ..*self
        }
    }
}

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
        let Some((x0, y0, x1, y1)) = self.clip(x0, y0, x1, y1) else {
            return;
        };
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

impl InkLayer {
    /// Trim a segment to the pixel grid (plus a one-pixel margin) so lines
    /// that are mostly or entirely off-screen cost nothing to walk.
    /// Liang-Barsky; `None` when nothing is visible.
    fn clip(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Option<(f32, f32, f32, f32)> {
        let (lo_x, lo_y) = (-1.0, -1.0);
        let (hi_x, hi_y) = (self.w as f32 * 2.0, self.h as f32 * 4.0);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for (p, q) in [
            (-dx, x0 - lo_x),
            (dx, hi_x - x0),
            (-dy, y0 - lo_y),
            (dy, hi_y - y0),
        ] {
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
                continue;
            }
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return None;
            }
        }
        Some((x0 + dx * t0, y0 + dy * t0, x0 + dx * t1, y0 + dy * t1))
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
    let place = |&(lx, ly): &(f32, f32)| {
        let (x, y) = rotate(lx * rock.radius, ly * rock.radius, rot);
        to_px(rock.x + x * CELL_X, rock.y + y * CELL_Y)
    };
    let Some(first) = pts.first().map(place) else {
        return;
    };
    let mut prev = first;
    for p in pts.iter().skip(1) {
        let next = place(p);
        ink.line(prev.0, prev.1, next.0, next.1, style.0, style.1);
        prev = next;
    }
    if closed {
        ink.line(prev.0, prev.1, first.0, first.1, style.0, style.1);
    }
}

/// A fixed highlight arc, so the light stays put while a hazard spins.
fn highlight(radius: f32) -> Vec<(f32, f32)> {
    (0..5)
        .map(|i| {
            let a = -2.5 + i as f32 * 0.2;
            (a.cos() * radius, a.sin() * radius)
        })
        .collect()
}

/// A craggy asteroid: a jagged outline, two pits, and a highlight.
fn draw_asteroid(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    rock: &Rock,
    style: (u8, Color),
    pal: &Palette,
) {
    let (pri, col) = style;
    // Push alternate corners in and out so it reads as broken rock, not a pebble.
    const CRAG: [f32; 5] = [1.12, 0.78, 1.0, 0.86, 1.2];
    let outline: Vec<(f32, f32)> = rock
        .shape
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let k = CRAG[(i + rock.id as usize) % CRAG.len()] / rock.radius;
            (x * k, y * k)
        })
        .collect();
    ink_path(ink, to_px, (rock, rock.rot), &outline, true, style);
    // Pits turn with the rock.
    for (px, py) in [(-0.28, 0.14), (0.3, -0.26)] {
        let (ox, oy) = rotate(px * rock.radius, py * rock.radius, rock.rot);
        let (x, y) = to_px(rock.x + ox * CELL_X, rock.y + oy * CELL_Y);
        ink.dot(x, y, pri, fade(col, 0.6));
    }
    ink_path(
        ink,
        to_px,
        (rock, 0.0),
        &highlight(0.7),
        false,
        (pri + 2, fade(pal.hot, 0.5)),
    );
}

/// A naval mine: a round shell, eight horns with bright tips, and a short
/// chain trailing behind its drift.
fn draw_mine(
    ink: &mut InkLayer,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
    rock: &Rock,
    style: (u8, Color),
    pal: &Palette,
) {
    let (pri, _) = style;
    ink_path(
        ink,
        to_px,
        (rock, 0.0),
        &ellipse(0.0, 0.0, 0.78, 0.78, 20),
        true,
        style,
    );
    // Horns: a short stud with a bright tip.
    for i in 0..8 {
        let a = rock.rot + i as f32 / 8.0 * TAU;
        let (c, s) = (a.cos(), a.sin());
        ink_path(
            ink,
            to_px,
            (rock, 0.0),
            &[(c * 0.78, s * 0.78), (c * 1.02, s * 1.02)],
            false,
            style,
        );
        let (ox, oy) = ((c * 1.1) * rock.radius, (s * 1.1) * rock.radius);
        let (x, y) = to_px(rock.x + ox * CELL_X, rock.y + oy * CELL_Y);
        ink.dot(x, y, pri + 1, pal.hot);
    }
    // Chain links trail opposite the direction of travel.
    let speed = (rock.vx * rock.vx + rock.vy * rock.vy).sqrt().max(0.01);
    // Travel direction in cells, mapped back into the squashed local frame.
    let (bx, by) = (-rock.vx / speed / CELL_X, -rock.vy / speed / CELL_Y);
    let norm = (bx * bx + by * by).sqrt().max(0.01);
    let (bx, by) = (bx / norm, by / norm);
    let mut prev = (bx * 1.25, by * 1.25);
    for k in 1..=3 {
        let d = 1.25 + k as f32 * 0.3;
        let next = (bx * d, by * d);
        ink_path(
            ink,
            to_px,
            (rock, 0.0),
            &[prev, next],
            false,
            (pri, fade(pal.dim, 0.9)),
        );
        prev = next;
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
    ink_path(
        ink,
        to_px,
        (rock, 0.0),
        &highlight(0.66),
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

/// Fast movers that would hop whole cells as glyphs: crystals (a small
/// diamond that glints) and exhaust dots. In braille they glide.
fn draw_motes(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: impl Fn(f32, f32) -> (f32, f32),
) {
    for gem in &game.gems {
        let (x, y) = to_px(gem.x, gem.y);
        let glint = (game.time * 9.0 + gem.x).sin() > 0.5;
        let col = if glint { pal.hot } else { pal.gem };
        let tips = [(x, y - 2.0), (x + 1.0, y), (x, y + 2.0), (x - 1.0, y)];
        for i in 0..4 {
            let (a, b) = (tips[i], tips[(i + 1) % 4]);
            ink.line(a.0, a.1, b.0, b.1, 4, col);
        }
    }
    for effect in &game.effects {
        if let Effect::Spark {
            x,
            y,
            glyph: '·',
            life,
            max,
            ..
        } = effect
        {
            let (px, py) = to_px(*x, *y);
            let k = if *max > 0.0 { life / max } else { 0.0 };
            ink.dot(px, py, 3, if k > 0.45 { pal.hot } else { pal.accent });
        }
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
    HITS.with(|h| h.borrow_mut().clear());
    let area = frame.area();
    let buf = frame.buffer_mut();
    let w = area.width;
    let h = area.height;
    let tinted;
    let pal = if game.overdrive > 0.0 {
        tinted = palette(game.theme).tinted(0.12);
        &tinted
    } else {
        palette(game.theme)
    };
    let scan = game.settings.scanlines;
    if game.theme == Theme::Jungle {
        paint_jungle_canopy(buf, area, pal, scan);
    } else {
        paint_water(buf, area, pal, scan);
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
            put_char(buf, x, y, star.glyph, style(pal.star));
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
        } else if rock.kind == Kind::Golden {
            pal.gem
        } else {
            pal.rock
        };
        let pri = if aimed { 2 } else { 1 };

        let style = (pri, col);
        match game.theme {
            Theme::Space => draw_asteroid(&mut ink, &to_px, rock, style, pal),
            Theme::Underwater => draw_mine(&mut ink, &to_px, rock, style, pal),
            Theme::Jungle => draw_coconut(&mut ink, &to_px, rock, style, pal),
            Theme::CatCafe => draw_cup(&mut ink, &to_px, rock, style, (pal, game.time), aimed),
        }
    }

    let blink_off = game.invuln > 0.0 && ((game.time * 14.0) as i32).rem_euclid(2) == 0;
    if !blink_off && !game.in_shop() {
        ships::draw_ship(&mut ink, game, pal, to_px);
    }
    bossfight::draw_boss(&mut ink, game, pal, to_px);
    draw_motes(&mut ink, game, pal, to_px);

    for y in 0..ink.h {
        for x in 0..ink.w {
            let i = y * ink.w + x;
            if ink.bits[i] == 0 {
                continue;
            }
            let ch = char::from_u32(0x2800 + ink.bits[i] as u32).unwrap_or(' ');
            put_char(buf, x as i32 + ox, y as i32 + oy, ch, style(ink.col[i]));
        }
    }

    if game.phase != Phase::Title {
        for rock in game.rocks.iter().filter(|r| r.live) {
            draw_label(buf, game, pal, rock, (rock.x + shx, rock.y + shy), oy);
        }
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
            // Plain dots glide in braille instead (see `draw_motes`).
            if *glyph == '·' {
                continue;
            }
            let fade = if *max > 0.0 { *life / *max } else { 0.0 };
            let col = if fade > 0.45 { pal.hot } else { pal.accent };
            put_char(
                buf,
                (x + shx).round() as i32,
                (y + shy).round() as i32 + oy,
                *glyph,
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

    let called = arcade::draw_callout(buf, game, pal, m);
    arcade::draw_announce(buf, game, pal, m);
    if game.banner_t > 0.0 && !game.banner.is_empty() {
        let col = if game.banner_t > 0.4 {
            pal.hot
        } else {
            pal.dim
        };
        // Under a call-out the banner becomes its subtitle.
        let y = if called {
            arcade::callout_top(m) + arcade::BANNER_GAP
        } else {
            (m.play_h as i32 / 2) - 2 + oy
        };
        put_centered(
            buf,
            0,
            m.play_w as i32,
            y.max(oy),
            &game.banner,
            style(col).add_modifier(Modifier::BOLD),
        );
    }

    if game.hurt > 0.0 {
        hline(buf, 0, w, m.origin_y, pal.danger);
        hline(buf, 0, w, m.dock_y.saturating_sub(1), pal.danger);
    }

    draw_header(buf, game, pal, w);
    let on_stage = match game.phase {
        Phase::Playing | Phase::Dying => true,
        Phase::Paused => game.paused_from == Phase::Playing,
        _ => false,
    };
    if on_stage {
        stage::draw_stage(buf, game, pal, m);
    }
    bossfight::draw_boss_ui(buf, game, pal, m);
    draw_dock(buf, game, pal, m);
    // Menus and the shop use the stage rows too; only play needs them.
    let menu = Metrics {
        play_h: m.play_h + m.stage_h,
        ..m
    };
    arcade::draw_title(buf, game, pal, w, menu);
    draw_shop(buf, game, pal, w, menu);
    draw_modal(buf, game, pal, w, menu);
    arcade::draw_initials(buf, game, pal, menu);
}

/// The colour of an elite's chip mark.
fn mark_color(kind: Kind, pal: &Palette) -> Color {
    match kind {
        Kind::Splitter => pal.accent,
        Kind::Runner => pal.danger,
        Kind::Armored => pal.amber,
        Kind::Golden => pal.gem,
        Kind::Masked | Kind::Normal => pal.dim,
    }
}

/// A hazard's word on its chip, under (or above) the hazard. Elites carry
/// a mark in front of the chip; masked words hide their tail until locked;
/// a word that would extend the link shows a small ring in its left pad.
fn draw_label(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    rock: &Rock,
    pos: (f32, f32),
    origin_y: i32,
) {
    let (x, y) = pos;
    let word = rock.word.as_str();
    // Squashed hazard art sits closer to its center.
    let radius = rock.radius * 0.75;
    let mark = rock.kind.mark(rock.armor);
    let mark_w = mark.map_or(0, |m| m.chars().count() as i32);
    // One padding cell each side; the chip is word + 2 wide.
    let width = word.len() as i32 + 2 + mark_w;
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
    let locked = game.target == Some(rock.id) && game.target_word() == Some(word);
    let bg = if locked {
        mix(pal.chip, pal.amber, 0.16)
    } else {
        pal.chip
    };
    let row = row + origin_y;
    if let Some(mark) = mark {
        put(
            buf,
            left,
            row,
            mark,
            style(mark_color(rock.kind, pal))
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        );
    }
    let left = left + mark_w;
    let pad = if locked {
        style(pal.amber).bg(bg).add_modifier(Modifier::BOLD)
    } else {
        style(pal.dim).bg(bg)
    };
    let (open, open_st) = if locked {
        ('‹', pad)
    } else if game.extends_link(word) {
        ('∘', style(pal.accent).bg(bg))
    } else {
        (' ', pad)
    };
    put_char(buf, left, row, open, open_st);
    let masked = rock.kind == Kind::Masked && !locked;
    for (i, ch) in word.chars().enumerate() {
        let st = if i < matched {
            style(pal.amber).add_modifier(Modifier::BOLD)
        } else if locked {
            style(pal.ink)
        } else {
            style(pal.dim)
        };
        let ch = if masked && i >= MASK_SHOWN.max(matched) {
            '·'
        } else {
            ch
        };
        put_char(buf, left + 1 + i as i32, row, ch, st.bg(bg));
    }
    put_char(
        buf,
        left + width - mark_w - 1,
        row,
        if locked { '›' } else { ' ' },
        pad,
    );
}

/// One row, arcade style: wave, 1UP score (rolling like an odometer),
/// hearts, wards, crystals, combo, and the HI score on the right.
fn draw_header(buf: &mut ratatui::buffer::Buffer, game: &Game, pal: &Palette, w: u16) {
    let bold = Modifier::BOLD;
    let mut x = write(
        buf,
        1,
        0,
        &format!("WAVE {:02}", game.wave.max(1)),
        style(pal.dim),
    );
    x = write(buf, x, 0, "   1UP ", style(pal.faint));
    let score_col = if game.score_rolling() {
        pal.hot
    } else {
        pal.amber
    };
    x = write(
        buf,
        x,
        0,
        &format!("{:06}", game.shown_score),
        style(score_col).add_modifier(bold),
    );
    x = write(buf, x, 0, "   ", style(pal.dim));
    let max = game.max_lives();
    // The last heart throbs.
    let pulse = (game.time * 5.0).sin() * 0.5 + 0.5;
    for i in 0..max {
        let (glyph, col) = if i >= game.lives {
            ("♡", pal.dead)
        } else if game.lives == 1 {
            ("♥", mix(pal.danger, pal.hot, pulse * 0.6))
        } else {
            ("♥", pal.danger)
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
        &format!("   ◆ {}", game.shown_crystals),
        style(gem_col).add_modifier(bold),
    );
    if w >= 60 && game.combo >= 2 {
        // The combo fades as its timer runs down; big combos glow.
        let col = if game.combo >= 10 {
            pal.accent
        } else {
            mix(pal.dim, pal.amber, game.combo_ratio())
        };
        x = write(
            buf,
            x,
            0,
            &format!("   x{}", game.combo),
            style(col).add_modifier(bold),
        );
    }
    let tag = match (game.daily, game.heat) {
        (Some(_), _) => "DAILY".to_string(),
        (None, 1..) => format!("HEAT {}", game.heat),
        _ => String::new(),
    };
    let hi = format!("HI {:06}", game.best.max(game.shown_score));
    if w >= 72 {
        let hx = w.saturating_sub(hi.chars().count() as u16 + 1);
        let _ = write(buf, hx, 0, &hi, style(pal.faint));
        let tx = hx.saturating_sub(tag.chars().count() as u16 + 3);
        if !tag.is_empty() && tx > x + 1 && game.phase != Phase::Title {
            let _ = write(buf, tx, 0, &tag, style(pal.danger).add_modifier(bold));
        }
    }
}

/// What the hint line says in play, per theme: (wave cleared, nothing
/// typed yet, several matches, one locked word).
fn hint_copy(theme: Theme) -> [&'static str; 4] {
    match theme {
        Theme::Space => [
            "wave clear — next rocks inbound",
            "type a word to aim the ship",
            "nearest rock is locked — keep typing to pin it",
            "finish the word to detonate",
        ],
        Theme::Underwater => [
            "minefield clear — next mines inbound",
            "type a word to arm the torpedo",
            "nearest mine is locked — keep typing to pin it",
            "finish the word to detonate",
        ],
        Theme::Jungle => [
            "canopy clear — more coconuts inbound",
            "type a word to aim the monkey",
            "nearest coconut is locked — keep typing to pin it",
            "finish the word to throw the banana",
        ],
        Theme::CatCafe => [
            "cafe clear — more coffee brewing",
            "type a word to ready the cat",
            "nearest cup is locked — keep typing to pin it",
            "finish the word to pounce and scratch",
        ],
    }
}

fn play_hint(game: &Game, word: &str, nmatch: usize) -> String {
    let [cleared, idle, many, locked] = hint_copy(game.theme);
    let text = if game.wave_cleared() {
        cleared
    } else if word.is_empty() && game.drive_ready() {
        "overdrive is charged — press tab to fire it"
    } else if word.is_empty() {
        idle
    } else if nmatch > 1 {
        many
    } else {
        locked
    };
    text.to_string()
}

/// The hazard noun for the miss line.
fn hazard_noun(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "rock",
        Theme::Underwater => "mine",
        Theme::Jungle => "coconut",
        Theme::CatCafe => "coffee",
    }
}

/// What went wrong with the last key: which key, and what was wanted.
fn miss_message(game: &Game) -> String {
    let key = game.last_wrong.map_or(String::new(), |c| format!("‹{c}› "));
    match game.expected_letter() {
        Some(want) => format!("{key}expected \"{want}\""),
        None => format!(
            "{key}no {} starts with \"{}{}\"",
            hazard_noun(game.theme),
            game.buffer,
            game.last_wrong.map_or(String::new(), |c| c.to_string())
        ),
    }
}

/// Against the boss a miss never hurts; say what to fix before moving on.
fn boss_miss_message(game: &Game) -> String {
    match game.last_wrong {
        Some(' ') => "‹space› the word has a typo: backspace and fix it first".to_string(),
        Some(c) => format!("‹{c}› typo: backspace to fix it"),
        None => miss_message(game),
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
                "j k / arrows move  ·  enter select  ·  q quits",
                style(pal.dim),
            );
            let line = if game.theme_menu_open {
                "ENTER applies the theme. ESC goes back."
            } else if game.options_open {
                "ENTER or h / l changes a setting. ESC goes back."
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
            let keys = match game.rack_selected().filter(|_| game.rack_focus) {
                Some((up, level)) => format!(
                    "j k choose  ·  enter sells {} for ◆ {}  ·  tab back to the shelf",
                    up.tier(level).name,
                    up.sell_value(level)
                ),
                None => format!(
                    "h l move  ·  enter select  ·  1-{} buy  ·  r reroll  ·  tab upgrades  ·  space go",
                    game.shop_slots()
                ),
            };
            put(buf, 2, y1, &keys, style(pal.dim));
            put(buf, 2, y2, &game.shop_note, style(pal.amber));
        }
        Phase::Initials => {
            put(
                buf,
                2,
                y1,
                "type three letters  ·  backspace erases  ·  enter confirms",
                style(pal.dim),
            );
        }
        Phase::Victory => {
            put(buf, 2, y1, "h l choose  ·  enter selects", style(pal.dim));
        }
        Phase::GameOver => {
            let x = write(
                buf,
                2,
                dock_y,
                game_over_heading(game),
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
                lost_banner(game.theme),
                style(pal.danger).add_modifier(bold),
            );
        }
        Phase::Playing => {
            let word = game.target_word().unwrap_or("");
            let nmatch = game.matches();
            if m.stage_h > 0 {
                // The stage shows the word; the dock only reports problems.
                if game.miss > 0.08 {
                    let text = if game.boss.is_some() {
                        boss_miss_message(game)
                    } else {
                        miss_message(game)
                    };
                    put(buf, 2, y1, &text, style(pal.danger).add_modifier(bold));
                } else if nmatch > 1 {
                    put(buf, 2, y1, &format!("{nmatch} match"), style(pal.dim));
                }
            } else {
                draw_input_line(buf, game, pal, dock_y);
            }
            let hint = match &game.boss {
                Some(b) if b.intro > 0.0 => {
                    "boss incoming: type the passage, space between words".to_string()
                }
                Some(_) => {
                    "space for the next word · backspace fixes typos · beat the clock".to_string()
                }
                None => play_hint(game, word, nmatch),
            };
            put(buf, 2, y2, &hint, style(pal.dim));
        }
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
            &miss_message(game),
            style(pal.danger).add_modifier(bold),
        );
        return;
    }
    // A boss typo stays in the buffer until fixed, so show it red.
    let clean = word.is_empty() || word.starts_with(game.buffer.as_str());
    let typed_col = if missed || !clean {
        pal.danger
    } else {
        pal.hot
    };
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
    let rest = if clean {
        word.get(game.buffer.len()..).unwrap_or("")
    } else {
        ""
    };
    x = write(buf, x, dock_y, rest, style(pal.faint));
    if nmatch > 1 {
        put(
            buf,
            x as i32 + 2,
            y1,
            &format!("{nmatch} match"),
            style(pal.dim),
        );
    } else if game.drive_ready() {
        put(
            buf,
            x as i32 + 2,
            y1,
            "TAB » OVERDRIVE",
            style(pal.hot).add_modifier(bold),
        );
    }
}

/// The game-over heading: the theme's loss line, or VICTORY after a won run.
fn game_over_heading(game: &Game) -> &'static str {
    if game.won {
        "VICTORY"
    } else {
        lost_banner(game.theme)
    }
}

/// Lines of text between a popup's heading and its menu.
fn modal_lines(game: &Game, pal: &Palette) -> Vec<(String, Color)> {
    let score = (
        format!("SCORE {:06}    BEST {:06}", game.score, game.best),
        pal.ink,
    );
    let stats = (game.run_stats.summary(), pal.dim);
    match game.phase {
        Phase::GameOver => match game.boss_escaped {
            Some(wpm) => vec![
                score,
                stats,
                (format!("the boss escaped: it needed {wpm} WPM"), pal.danger),
            ],
            None => vec![score, stats],
        },
        Phase::Victory => {
            let mut lines = vec![score, stats];
            if game.daily.is_none() && game.settings.heat_unlocked > game.heat {
                lines.push((
                    format!("HEAT {} UNLOCKED", game.settings.heat_unlocked),
                    pal.danger,
                ));
            }
            lines.push((
                "endless keeps going  ·  finish banks the score".to_string(),
                pal.faint,
            ));
            lines
        }
        _ => vec![stats],
    }
}

fn draw_modal(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    w: u16,
    m: crate::game::Metrics,
) {
    if !matches!(game.phase, Phase::Paused | Phase::GameOver | Phase::Victory) {
        return;
    }
    let (heading, heading_col) = match game.phase {
        Phase::GameOver if game.won => (game_over_heading(game), pal.amber),
        Phase::GameOver => (game_over_heading(game), pal.danger),
        Phase::Victory => ("THE FINAL BOSS IS DOWN", pal.amber),
        _ => ("PAUSED", pal.amber),
    };
    let victory = game.phase == Phase::Victory;
    let lines = modal_lines(game, pal);

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
    let hint = if game.theme_menu_open {
        "enter applies theme    esc backs out"
    } else if game.phase == Phase::Paused {
        "h j k l / arrows move    enter select    esc resumes"
    } else {
        "h j k l / arrows move    enter select"
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

    let inner_w = lines
        .iter()
        .map(|(l, _)| l.chars().count())
        .chain([
            heading.chars().count(),
            menu_w,
            theme_line.chars().count(),
            choice_w,
            hint.chars().count(),
        ])
        .max()
        .unwrap_or(0);
    let box_w = inner_w + 4;
    if box_w > w as usize {
        return;
    }

    // Lay out the rows inside the border.
    let heading_row = 1usize;
    let mut rel = 3; // heading + blank
    let lines_row = rel;
    rel += lines.len() + 1;
    let theme_row = if victory {
        None
    } else {
        let row = rel;
        rel += 2; // theme + blank
        Some(row)
    };
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
    let box_h = rel + 1;

    let left = (w as usize - box_w) / 2;
    let top = m.origin_y as usize + (m.play_h as usize).saturating_sub(box_h) / 2;
    let bg = mix(pal.bg_top, pal.bg_bottom, 0.22);

    let blank = " ".repeat(box_w);
    for y in top..top + box_h {
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
        put_char(buf, left as i32, y as i32, '│', border);
        put_char(buf, (left + box_w - 1) as i32, y as i32, '│', border);
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
    for (i, (line, col)) in lines.iter().enumerate() {
        put_centered(
            buf,
            inner_x,
            inner_width,
            (top + lines_row + i) as i32,
            line,
            style(*col).bg(bg),
        );
    }
    if let Some(row) = theme_row {
        put_centered(
            buf,
            inner_x,
            inner_width,
            (top + row) as i32,
            &theme_line,
            style(if game.theme_menu_open {
                pal.amber
            } else {
                pal.faint
            })
            .bg(bg),
        );
    }

    let mut x = inner_x + (inner_width - menu_w as i32).max(0) / 2;
    for (i, label) in labels.iter().enumerate() {
        let st = if !game.theme_menu_open && i == game.menu {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else {
            style(pal.dim).bg(bg)
        };
        let len = label.chars().count() as i32;
        put(buf, x, (top + menu_row) as i32, label, st);
        mark((x, (top + menu_row) as i32, len, 1), Hit::Menu(i));
        x += len + gap.len() as i32;
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
            let len = label.chars().count() as i32;
            put(buf, x, (top + row) as i32, label, st);
            mark((x, (top + row) as i32, len, 1), Hit::Theme(i));
            x += len + choice_gap.len() as i32;
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

    // Paused: the build so far, under the box when there is room.
    if game.phase == Phase::Paused {
        let below = (top + box_h + 1) as i32;
        let rows = (m.origin_y + m.play_h) as i32 - below;
        let held: Vec<(Upgrade, u8)> = game.upgrades.held().collect();
        if rows >= 2 {
            draw_installed(buf, game, pal, (2, below, w as i32 - 4, rows - 1), &held);
        }
    }
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
    let Some(shop) = game.shop.as_ref().filter(|_| game.in_shop()) else {
        return;
    };
    if game.rack_focus {
        draw_loadout(buf, game, pal, w, m);
        return;
    }
    let gap = 2;
    let cols = game.shop_slots() as i32;
    let gap = if cols > 3 { 1 } else { gap };
    let card_w = ((w as i32 - 4 - gap * (cols - 1)) / cols).clamp(14, 24);
    let total = card_w * cols + gap * (cols - 1);
    let left = (w as i32 - total) / 2;
    // Installed upgrades get a compact table under the buttons; the cards
    // give up height when the table needs it.
    let held: Vec<(Upgrade, u8)> = game.upgrades.held().collect();
    let table_cols = ((w as i32 - 4) / TABLE_COL_W).max(1) as usize;
    // Cards keep room for a three-line description; the table takes the rest
    // (a header row, then as many rows as fit).
    let spare = (m.play_h as i32 - (9 + 6)).max(1);
    let table_rows = if held.is_empty() {
        0
    } else {
        (held.len().div_ceil(table_cols) as i32 + 1).min(spare)
    };
    let card_h = (m.play_h as i32 - 7 - table_rows).clamp(7, 10);
    let block_h = card_h + 6 + table_rows;
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
    // The purse, then how the wave was typed.
    let purse = format!("◆ {} crystals", game.shown_crystals);
    let report = format!("  ·  {}", game.wave_stats.summary());
    let line_w = (purse.chars().count() + report.chars().count()) as i32;
    let px = (w as i32 - line_w).max(0) / 2;
    put(
        buf,
        px,
        top + 1,
        &purse,
        style(pal.gem).add_modifier(Modifier::BOLD),
    );
    if line_w <= w as i32 {
        put(
            buf,
            px + purse.chars().count() as i32,
            top + 1,
            &report,
            style(pal.dim),
        );
    }

    let cards_y = top + 3;
    for slot in 0..game.shop_slots() {
        let x = left + slot as i32 * (card_w + gap);
        // Cards rise into place one after another when the shop opens.
        let rise = if game.settings.reduced_motion {
            0.0
        } else {
            ((game.time - game.shop_t0 - slot as f32 * 0.05) / 0.3).clamp(0.0, 1.0)
        };
        let cards_y = cards_y
            + if game.settings.reduced_motion {
                0
            } else {
                ((1.0 - rise) * (1.0 - rise) * 8.0).round() as i32
            };
        mark((x, cards_y, card_w, card_h), Hit::Shop(slot));
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
        let rarity = offer.rarity();
        let selected = game.shop_sel == slot && !game.rack_focus;
        let rare = rarity == Rarity::Rare;
        let edge = if offer.sold {
            pal.dim
        } else if selected {
            pal.hot
        } else {
            rarity_color(rarity, pal)
        };
        let glyphs = match (selected, rare) {
            (_, true) => ["╔", "╗", "╚", "╝", "═", "║"],
            (true, false) => ["┏", "┓", "┗", "┛", "━", "┃"],
            (false, false) => ["┌", "┐", "└", "┘", "─", "│"],
        };
        let bought = game.time - game.bought_t.get(slot).copied().unwrap_or(-10.0);
        let fill = if (0.0..0.3).contains(&bought) {
            // A purchase flashes the card once.
            mix(pal.chip, pal.hot, 0.5 * (1.0 - bought / 0.3))
        } else if selected {
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
        let name = format!("{} {}", slot + 1, offer.name());
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
            format!("★ {}", rarity.label())
        } else {
            rarity.label().to_string()
        };
        put(
            buf,
            x + 2,
            cards_y + 2,
            &tag,
            body(if offer.sold {
                pal.dim
            } else {
                rarity_color(rarity, pal)
            }),
        );
        let desc_rows = (card_h - 6).max(1) as usize;
        let blurb = offer.upgrade.blurb_delta(offer.tier.saturating_sub(1));
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
            let cost = game.offer_cost(offer);
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
            let note = if held > 0 && !def.consumable {
                format!("owned {held}")
            } else if !def.consumable && game.rack_used() >= game.rack_size() {
                "rack full".to_string()
            } else {
                String::new()
            };
            put(buf, x + 2, foot_y, &note, body(pal.faint));
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
    let mine = "[ MY UPGRADES ]".to_string();
    let btn_gap = 4;
    let btn_total =
        (reroll.chars().count() + go.chars().count() + mine.chars().count()) as i32 + btn_gap * 2;
    let mut bx = (w as i32 - btn_total).max(0) / 2;
    let by = cards_y + card_h + 1;
    for (hit, label) in [
        (Hit::Shop(game.shop_reroll_index()), reroll),
        (Hit::Shop(game.shop_go_index()), go),
        (Hit::Loadout, mine),
    ] {
        let selected = hit == Hit::Shop(game.shop_sel);
        let st = if selected {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else if hit == Hit::Loadout {
            style(pal.dim)
        } else {
            style(pal.ink)
        };
        let len = label.chars().count() as i32;
        put(buf, bx, by, &label, st);
        mark((bx, by, len, 1), hit);
        bx += len + btn_gap;
    }

    draw_installed(buf, game, pal, (left, by + 2, total, table_rows - 1), &held);
}

/// One installed line on the upgrades screen: its rows of text.
fn loadout_entry(up: Upgrade, level: u8, inner: usize) -> Vec<String> {
    let mut rows = wrap(&up.blurb(level), inner);
    if level < up.max() {
        let next = up.tier(level + 1).name;
        // Lines whose numbers grow say by how much; the rest just name it.
        let step = up.blurb_delta(level);
        let line = if step == up.blurb(level) {
            format!("next: {next}")
        } else {
            format!("next: {next} · {step}")
        };
        rows.extend(wrap(&line, inner));
    }
    rows.iter_mut()
        .for_each(|r| *r = r.chars().take(inner).collect());
    rows
}

/// The shop's upgrades screen (Tab): every installed line with its card
/// name, level, what it does now and what the next card adds, plus its
/// sell price. The cursor picks the line to sell.
fn draw_loadout(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    w: u16,
    m: crate::game::Metrics,
) {
    let held: Vec<(Upgrade, u8)> = game.upgrades.held().collect();
    let width = (w as i32 - 4).min(76);
    let left = (w as i32 - width) / 2;
    let inner = (width - 4).max(10) as usize;
    let top = m.origin_y as i32;
    let bottom = (m.origin_y + m.play_h) as i32 - 1;
    // A clean page: no background specks between the rows.
    for y in top..=bottom {
        clear_span(buf, left - 2, y, width + 4);
    }
    put_centered(
        buf,
        0,
        w as i32,
        top,
        &format!(
            "MY UPGRADES  ·  RACK {}/{}",
            game.rack_used(),
            game.rack_size()
        ),
        style(pal.amber).add_modifier(Modifier::BOLD),
    );
    put_centered(
        buf,
        0,
        w as i32,
        top + 1,
        &format!("◆ {} crystals", game.shown_crystals),
        style(pal.gem).add_modifier(Modifier::BOLD),
    );

    // Each entry is a name line plus its description rows and a gap.
    let entries: Vec<Vec<String>> = held
        .iter()
        .map(|&(up, level)| loadout_entry(up, level, inner))
        .collect();
    let height = |i: usize| entries[i].len() as i32 + 2;
    let list_top = top + 3;
    let room = bottom - 1 - list_top;
    // Scroll so the cursor's entry is on screen.
    let sel = game.rack_sel.min(held.len().saturating_sub(1));
    let mut first = 0;
    while first < sel && (first..=sel).map(height).sum::<i32>() > room {
        first += 1;
    }
    let mut y = list_top;
    let mut last = first;
    for (i, &(up, level)) in held.iter().enumerate().skip(first) {
        if y + height(i) - 1 > list_top + room {
            break;
        }
        last = i + 1;
        let on = i == sel;
        let tier = up.tier(level);
        let fill = if on {
            mix(pal.chip, pal.amber, 0.14)
        } else {
            pal.chip
        };
        let h = height(i) - 1;
        for r in 0..h {
            put(
                buf,
                left,
                y + r,
                &" ".repeat(width as usize),
                style(pal.ink).bg(fill),
            );
        }
        let body = |col: Color| style(col).bg(fill);
        let lead = if on { "▸ " } else { "  " };
        put(
            buf,
            left,
            y,
            &format!("{lead}{}", tier.name),
            body(rarity_color(tier.rarity, pal)).add_modifier(Modifier::BOLD),
        );
        let pips: String = (0..up.max())
            .map(|n| if n < level { '●' } else { '○' })
            .collect();
        put(buf, left + 16, y, &pips, body(pal.dim));
        let price = format!("sell ◆ {}", up.sell_value(level));
        let px = left + width - 2 - price.chars().count() as i32;
        put(
            buf,
            px,
            y,
            &price,
            body(if on { pal.gem } else { pal.faint }),
        );
        let now_rows = wrap(&up.blurb(level), inner).len();
        for (r, line) in entries[i].iter().enumerate() {
            let col = if r >= now_rows { pal.faint } else { pal.ink };
            put(buf, left + 2, y + 1 + r as i32, line, body(col));
        }
        mark((left, y, width, h), Hit::Rack(i));
        y += height(i);
    }
    let hidden = held.len() - (last - first);
    if hidden > 0 {
        let above = if first > 0 { "↑ " } else { "" };
        let below = if last < held.len() { " ↓" } else { "" };
        put_centered(
            buf,
            0,
            w as i32,
            bottom - 1,
            &format!("{above}{hidden} more{below}"),
            style(pal.faint),
        );
    }

    // Buttons for the mouse; Enter and Tab do the same.
    let sell = match held.get(sel) {
        Some(&(up, level)) => format!("[ SELL ◆ {} ]", up.sell_value(level)),
        None => "[ SELL ]".to_string(),
    };
    let back = "[ BACK TO SHOP ]".to_string();
    let total = (sell.chars().count() + back.chars().count()) as i32 + 4;
    let mut bx = (w as i32 - total).max(0) / 2;
    for (hit, label, col) in [(Hit::Sell, sell, pal.danger), (Hit::Loadout, back, pal.ink)] {
        let len = label.chars().count() as i32;
        put(
            buf,
            bx,
            bottom,
            &label,
            style(col).add_modifier(Modifier::BOLD),
        );
        mark((bx, bottom, len, 1), hit);
        bx += len + 4;
    }
}

/// Width of one column in the installed table: a 12-letter name, a gap, pips.
const TABLE_COL_W: i32 = 22;

/// The run's rack as a table: one cell per line showing its newest card
/// and level pips, under a RACK used/size heading. In the shop's sell mode
/// the cursor sits on a cell. Rows that do not fit are summarized.
fn draw_installed(
    buf: &mut ratatui::buffer::Buffer,
    game: &Game,
    pal: &Palette,
    area: (i32, i32, i32, i32),
    held: &[(Upgrade, u8)],
) {
    let (left, top, width, rows) = area;
    let heading = format!("RACK {}/{}", game.rack_used(), game.rack_size());
    if held.is_empty() {
        put_centered(
            buf,
            0,
            buf.area.width as i32,
            top,
            &format!("{heading}  ·  nothing installed yet"),
            style(pal.faint),
        );
        return;
    }
    let cols = ((width / TABLE_COL_W).max(1) as usize).min(held.len());
    let table_w = cols as i32 * TABLE_COL_W;
    let x0 = left + (width - table_w).max(0) / 2;
    let cursor = game.rack_focus.then_some(game.rack_sel);
    // Scroll so the cursor's row is always shown.
    let fit = rows.max(1) as usize;
    let first_row = cursor.map_or(0, |c| (c / cols + 1).saturating_sub(fit));
    let skip = first_row * cols;
    let shown = (fit * cols).min(held.len() - skip);
    let more = held.len() - shown;
    let more = if more > 0 {
        format!("  (+{more} more)")
    } else {
        String::new()
    };
    let sell = if game.in_shop() && !game.rack_focus {
        "  ·  tab for details"
    } else {
        ""
    };
    put(
        buf,
        x0,
        top,
        &format!("{heading}{more}{sell}"),
        style(pal.dim).add_modifier(Modifier::BOLD),
    );
    mark((x0, top, table_w, rows.max(1) + 1), Hit::Loadout);
    for (i, &(up, level)) in held.iter().enumerate().skip(skip).take(shown) {
        let cell = i - skip;
        let x = x0 + (cell % cols) as i32 * TABLE_COL_W;
        let y = top + 1 + (cell / cols) as i32;
        let tier = up.tier(level);
        let st = if cursor == Some(i) {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else {
            style(rarity_color(tier.rarity, pal))
        };
        put(buf, x, y, tier.name, st);
        if up.max() > 1 {
            let pips: String = (0..up.max())
                .map(|n| if n < level { '●' } else { '○' })
                .collect();
            put(buf, x + 13, y, &pips, style(pal.dim));
        }
    }
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

fn hline(buf: &mut ratatui::buffer::Buffer, x0: u16, x1: u16, y: u16, col: Color) {
    for x in x0..x1 {
        put_char(buf, x as i32, y as i32, '─', style(col));
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

/// One glyph in one cell, without building a `String`.
fn put_char(buf: &mut ratatui::buffer::Buffer, x: i32, y: i32, ch: char, st: Style) {
    if x < 0 || y < 0 || x >= buf.area.width as i32 || y >= buf.area.height as i32 {
        return;
    }
    buf[(x as u16, y as u16)].set_char(ch).set_style(st);
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

/// Background shade of odd rows with scanlines on.
const SCANLINE: f32 = 0.9;

fn scanline(col: Color, y: u16, on: bool) -> Color {
    if on && y % 2 == 1 {
        fade(col, SCANLINE)
    } else {
        col
    }
}

fn paint_water(buf: &mut ratatui::buffer::Buffer, area: Rect, pal: &Palette, scan: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let span = area.height.saturating_sub(1).max(1) as f32;
    for y in 0..area.height {
        let t = y as f32 / span;
        let col = scanline(mix(pal.bg_top, pal.bg_bottom, t), y, scan);
        buf.set_style(
            Rect::new(area.x, area.y + y, area.width, 1),
            Style::default().bg(col),
        );
    }
}

/// A shaded canopy: deep green gradient with subtle, soft dappled shading
/// and a dark vignette that stays calm and readable behind gameplay.
fn paint_jungle_canopy(buf: &mut ratatui::buffer::Buffer, area: Rect, pal: &Palette, scan: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    // The canopy never changes for a given size, so shade it once and copy.
    CANOPY.with(|cache| {
        let mut cache = cache.borrow_mut();
        let key = (area.width, area.height, pal.bg_top, scan);
        if cache.0 != key {
            *cache = (key, shade_canopy(area, pal, scan));
        }
        if buf.area == area {
            // The common case: the canopy covers the whole buffer.
            for (cell, col) in buf.content.iter_mut().zip(&cache.1) {
                cell.set_bg(*col);
            }
            return;
        }
        let w = area.width as usize;
        for (i, col) in cache.1.iter().enumerate() {
            let (x, y) = ((i % w) as u16, (i / w) as u16);
            buf[(area.x + x, area.y + y)].set_bg(*col);
        }
    });
}

type CanopyKey = (u16, u16, Color, bool);

thread_local! {
    static CANOPY: std::cell::RefCell<(CanopyKey, Vec<Color>)> =
        const { std::cell::RefCell::new(((0, 0, Color::Reset, false), Vec::new())) };
}

/// Background colors for every cell of the canopy, row by row.
fn shade_canopy(area: Rect, pal: &Palette, scan: bool) -> Vec<Color> {
    let mut out = Vec::with_capacity(area.width as usize * area.height as usize);
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
            out.push(scanline(col, y, scan));
        }
    }
    out
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

    /// Frame cost per theme. Run with
    /// `cargo test --release frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn frame_cost() {
        let (w, h) = (200, 60);
        for theme in Theme::ALL {
            let mut game = Game::new(w, h, 7);
            game.theme_choice = theme.index();
            game.apply_theme();
            game.start();
            for _ in 0..240 {
                game.update(1.0 / 120.0);
            }
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            let n = 2000;
            let t0 = std::time::Instant::now();
            for _ in 0..n {
                terminal.draw(|frame| draw(frame, &game)).unwrap();
            }
            let ours = t0.elapsed() / n;
            let t1 = std::time::Instant::now();
            for _ in 0..n {
                terminal.draw(|_| {}).unwrap();
            }
            eprintln!(
                "{theme:?}: {ours:?} per frame (empty frame {:?})",
                t1.elapsed() / n
            );
        }
    }

    #[test]
    fn lines_far_off_screen_are_clipped_away() {
        let mut ink = InkLayer::new(10, 5, Color::Reset);
        let t0 = std::time::Instant::now();
        ink.line(-1e7, -1e7, -1e7, 1e7, 1, Color::Reset);
        ink.line(-1e7, 3.0, 1e7, 3.0, 1, Color::Reset);
        assert!(t0.elapsed().as_millis() < 50, "walked the whole line");
        // The crossing line still lands on screen, the other does not.
        assert_eq!(ink.bits.iter().filter(|&&b| b != 0).count(), 10);
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
        game.skip_ready_for_test();
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
            kind: Kind::Normal,
            armor: 0,
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
        game.skip_ready_for_test();
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
            kind: Kind::Normal,
            armor: 0,
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
            assert!(
                rows[0].contains("1UP") && rows[0].contains("HI"),
                "{screen}"
            );
            assert!(!screen.contains("HULL"), "{screen}");
            assert!(screen.contains("‹comet›"), "locked chip\n{screen}");
            assert!(rows[23].contains("finish the word"), "{screen}");
        }
    }

    /// The stage floor: a rule, possibly part-filled by the Overdrive meter.
    fn floor_line(row: &str) -> bool {
        row.starts_with('─') || row.starts_with('━')
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
            assert!(floor_line(screen.lines().nth(21).unwrap()), "{screen}");
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
        assert!(floor_line(screen.lines().nth(45).unwrap()), "{screen}");
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
            game.snap_counters();
            let shop = game.shop.as_mut().unwrap();
            shop.offers[0] = Offer::new(Upgrade::SteadyHands, 1);
            shop.offers[1] = Offer::new(Upgrade::Multishot, 1);
            shop.offers[2] = Offer::new(Upgrade::SecondWind, 1);
            shop.offers[2].sold = true;
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
        game.snap_counters();
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
    fn installed_upgrades_form_a_compact_table() {
        use crate::upgrades::Upgrade;
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.rocks.clear();
        for (up, level) in [
            (Upgrade::Prospector, 2),
            (Upgrade::SlowTide, 3),
            (Upgrade::Multishot, 1),
            (Upgrade::Haggler, 1),
            (Upgrade::Focus, 2),
        ] {
            game.upgrades.set(up, level);
        }
        game.open_shop_for_test();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("RACK 5/6"), "{screen}");
        assert!(
            screen.contains("GOLD PAN"),
            "newest card of the line\n{screen}"
        );
        assert!(screen.contains("STILL WATER"), "{screen}");
        assert!(screen.contains("●●○○○"), "level pips\n{screen}");
        assert!(screen.contains("HAGGLER"), "{screen}");
        // Cards still draw above it.
        assert!(screen.contains("NEXT WAVE"), "{screen}");
    }

    #[test]
    fn crystals_glide_in_braille_and_the_underwater_boss_is_a_shark() {
        let mut game = locked_rock_game(Theme::Underwater, "comet");
        game.gems.push(crate::game::Gem::resting(20.3, 6.6));
        let screen = render(&game, 80, 24);
        let field: String = screen.lines().skip(1).collect();
        assert!(
            !field.contains('◆'),
            "gems are braille, not glyphs\n{screen}"
        );

        game.begin_wave_for_test(5);
        let screen = render(&game, 80, 24);
        assert!(screen.contains("THE GREAT WHITE"), "{screen}");
    }

    #[test]
    fn a_boss_wave_shows_the_boss_clock_and_passage() {
        for theme in Theme::ALL {
            let mut game = locked_rock_game(theme, "comet");
            game.begin_wave_for_test(5);
            let intro = render(&game, 80, 24);
            assert!(intro.contains("BOSS FIGHT"), "{intro}");
            assert!(intro.contains("40 WPM"), "{intro}");
            assert!(intro.contains("starting in"), "{intro}");
            assert!(intro.contains("fix typos"), "{intro}");
            for _ in 0..70 {
                game.update(0.05);
            }
            let fight = render(&game, 80, 24);
            assert!(fight.contains("HP"), "{fight}");
            assert!(fight.contains("TIME"), "{fight}");
            assert!(fight.contains("/ 40 WPM"), "{fight}");
            let word = game.target_word().unwrap().to_string();
            assert!(fight.contains(&word), "passage shows {word}\n{fight}");
            assert!(fight.contains("space for the next word"), "{fight}");
            // The seconds never run into the pace readout on narrow screens.
            let narrow = render(&game, 70, 22);
            let status = narrow.lines().nth(1).unwrap();
            assert!(
                status.contains("s ") && status.contains(" / 40 WPM"),
                "{status}"
            );
        }
    }

    #[test]
    fn boss_typos_say_how_to_fix_them() {
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.begin_wave_for_test(5);
        for _ in 0..70 {
            game.update(0.05);
        }
        let word = game.target_word().unwrap().to_string();
        let bad = if word.starts_with('z') { 'q' } else { 'z' };
        game.type_char(bad);
        let screen = render(&game, 80, 24);
        assert!(
            screen.contains(&format!("‹{bad}› typo: backspace")),
            "{screen}"
        );
        game.update(0.5);
        game.type_space();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("‹space› the word has a typo"), "{screen}");
        assert!(!screen.contains("strikes"), "{screen}");
    }

    #[test]
    fn wide_shop_draws_four_cards_that_fit() {
        use crate::game::Offer;
        use crate::upgrades::Upgrade;
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.rocks.clear();
        game.upgrades.set(Upgrade::WideShop, 1);
        game.open_shop_for_test();
        game.crystals = 99;
        let cards = [
            Offer::new(Upgrade::SteadyHands, 2),
            Offer::new(Upgrade::Prospector, 1),
            Offer::new(Upgrade::Focus, 2),
            Offer::new(Upgrade::Shrapnel, 2),
        ];
        game.shop.as_mut().unwrap().offers = cards.to_vec();
        let screen = render(&game, 80, 24);
        for name in [
            "1 IRON NERVES",
            "2 PROSPECTOR",
            "3 BLINKERS",
            "4 CLUSTER BOMB",
        ] {
            assert!(screen.contains(name), "{name} missing\n{screen}");
        }
        assert!(screen.contains("NEXT WAVE"), "{screen}");
    }

    #[test]
    fn an_empty_shop_still_draws_sold_out_slots() {
        let mut game = locked_rock_game(Theme::Underwater, "comet");
        game.rocks.clear();
        for up in crate::upgrades::Upgrade::ALL {
            game.upgrades.set(up, up.max());
        }
        game.lives = game.max_lives();
        game.open_shop_for_test();
        let screen = render(&game, 80, 24);
        // Everything owned includes Wide Shop, so all four slots show.
        assert_eq!(screen.matches("SOLD OUT").count(), 4, "{screen}");
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
        game.skip_ready_for_test();
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
            kind: Kind::Normal,
            armor: 0,
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
        paint_jungle_canopy(&mut buf0, area, pal, false);

        let mut buf1 = ratatui::buffer::Buffer::empty(area);
        paint_jungle_canopy(&mut buf1, area, pal, false);

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

    #[test]
    fn the_title_is_an_attract_screen() {
        let mut game = Game::new(80, 24, 7);
        let screen = render(&game, 80, 24);
        assert!(screen.contains("PRESS ENTER"), "{screen}");
        assert!(screen.contains("DAILY RUN"), "{screen}");
        assert!(screen.contains("HOW TO PLAY"), "{screen}");
        for _ in 0..200 {
            game.update(0.05);
        }
        let idle = render(&game, 80, 24);
        assert!(idle.contains("HI-SCORES"), "{idle}");
        assert!(idle.contains("1ST"), "{idle}");
        // The smallest terminal drops the block letters but keeps the menu.
        let small = render(&Game::new(48, 18, 7), 48, 18);
        assert!(small.contains("T Y P O I D S"), "{small}");
        assert!(small.contains("START"), "{small}");
    }

    #[test]
    fn options_and_help_draw_on_the_title() {
        let mut game = Game::new(80, 24, 7);
        game.open_options();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("SCREEN SHAKE"), "{screen}");
        assert!(screen.contains("FULL"), "{screen}");
        game.close_options();
        game.open_help();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("OVERDRIVE"), "{screen}");
        assert!(screen.contains("press any key"), "{screen}");
    }

    #[test]
    fn wave_call_outs_are_big_pixel_letters() {
        let mut game = Game::new(80, 24, 7);
        game.start();
        for _ in 0..6 {
            game.update(0.05);
        }
        let screen = render(&game, 80, 24);
        let middle = stage_rows(&screen, 5, 12);
        let blocks = middle.chars().filter(|c| "█▀▄".contains(*c)).count();
        assert!(blocks > 30, "WAVE 1 in pixels\n{screen}");
    }

    #[test]
    fn elite_chips_carry_their_mark_and_masks_hide_letters() {
        let mut game = locked_rock_game(Theme::Space, "comet");
        let mut masked = game.rocks[0].clone();
        masked.id = 7;
        masked.word = "planet".into();
        masked.kind = Kind::Masked;
        masked.x = 20.0;
        let mut split = masked.clone();
        split.id = 8;
        split.word = "zinc".into();
        split.kind = Kind::Splitter;
        split.x = 40.0;
        split.y = 3.0;
        game.rocks.push(masked);
        game.rocks.push(split);
        let screen = render(&game, 80, 24);
        assert!(screen.contains("?"), "{screen}");
        assert!(screen.contains("pl····"), "{screen}");
        assert!(screen.contains("÷"), "{screen}");
        assert!(screen.contains("zinc"), "{screen}");
    }

    #[test]
    fn a_miss_names_the_key() {
        let mut game = locked_rock_game(Theme::Underwater, "comet");
        game.type_char('q');
        let screen = render(&game, 80, 24);
        assert!(screen.contains("‹q› expected \"o\""), "{screen}");
    }

    #[test]
    fn scanlines_darken_every_other_row() {
        let mut game = Game::new(80, 24, 7);
        let bg = |game: &Game, y: u16| {
            let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
            t.draw(|f| draw(f, game)).unwrap();
            t.backend().buffer()[(79, y)].bg
        };
        let (on2, on3) = (bg(&game, 2), bg(&game, 3));
        game.settings.scanlines = false;
        let (off2, off3) = (bg(&game, 2), bg(&game, 3));
        assert_eq!(on2, off2, "even rows untouched");
        assert_ne!(on3, off3, "odd rows darker");
    }

    #[test]
    fn my_upgrades_screen_says_what_each_upgrade_does() {
        use crate::upgrades::Upgrade;
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.rocks.clear();
        game.upgrades.set(Upgrade::SlowTide, 1);
        game.upgrades.set(Upgrade::Multishot, 2);
        game.open_shop_for_test();
        game.input_lock = 0.0;
        game.shop_toggle_rack();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("MY UPGRADES"), "{screen}");
        for (up, level) in [(Upgrade::SlowTide, 1), (Upgrade::Multishot, 2)] {
            let name = up.tier(level).name;
            assert!(screen.contains(name), "{name}\n{screen}");
            let words: String = up.blurb(level).chars().take(20).collect();
            assert!(screen.contains(&words), "describes {name}\n{screen}");
        }
        assert!(screen.contains("next: "), "shows the next card\n{screen}");
        assert!(screen.contains("[ SELL ◆"), "{screen}");
        assert!(screen.contains("[ BACK TO SHOP ]"), "{screen}");
        assert!(!screen.contains("REROLL"), "replaces the shelf\n{screen}");
    }

    /// The first cell whose hit is `want`, after a render.
    fn find_hit(w: u16, h: u16, want: Hit) -> Option<(u16, u16)> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .find(|&(x, y)| hit_at(x, y) == Some(want))
    }

    #[test]
    fn menus_and_shop_record_where_the_mouse_can_click() {
        let mut game = Game::new(80, 24, 1);
        render(&game, 80, 24);
        let at = find_hit(80, 24, Hit::Menu(1)).expect("title items are clickable");
        assert_eq!(hit_at(at.0, at.1), Some(Hit::Menu(1)));
        assert!(game.hover(Hit::Menu(1)));
        assert_eq!(game.menu, 1);
        assert!(!game.hover(Hit::Shop(0)), "no shop on the title");

        let mut shop = locked_rock_game(Theme::Space, "comet");
        shop.rocks.clear();
        shop.upgrades.set(crate::upgrades::Upgrade::Focus, 1);
        shop.open_shop_for_test();
        render(&shop, 80, 24);
        for hit in [
            Hit::Shop(0),
            Hit::Shop(2),
            Hit::Shop(shop.shop_go_index()),
            Hit::Loadout,
        ] {
            assert!(find_hit(80, 24, hit).is_some(), "{hit:?}");
        }
        shop.pause();
        render(&shop, 80, 24);
        let (x, y) = find_hit(80, 24, Hit::Menu(PAUSE_CONTINUE)).expect("pause menu");
        assert_eq!(hit_at(x, y), Some(Hit::Menu(PAUSE_CONTINUE)));
        assert!(
            !shop.hover(Hit::Shop(0)),
            "the shop under the pause is not live"
        );
    }

    #[test]
    fn the_shop_reports_the_wave_and_pause_shows_the_rack() {
        let mut game = locked_rock_game(Theme::Space, "comet");
        game.type_char('z');
        game.rocks.clear();
        game.upgrades.set(crate::upgrades::Upgrade::Focus, 1);
        game.open_shop_for_test();
        let screen = render(&game, 80, 24);
        assert!(screen.contains("ACC"), "{screen}");
        assert!(screen.contains("WPM"), "{screen}");
        assert!(screen.contains("tab for details"), "{screen}");
        assert!(screen.contains("[ MY UPGRADES ]"), "{screen}");
        game.pause();
        let paused = render(&game, 80, 24);
        assert!(paused.contains("PAUSED"), "{paused}");
        assert!(paused.contains("RACK 1/6"), "{paused}");
    }

    #[test]
    fn initials_and_victory_have_their_own_screens() {
        let mut game = locked_rock_game(Theme::Jungle, "comet");
        game.phase = Phase::Initials;
        game.type_char('a');
        let screen = render(&game, 80, 24);
        assert!(screen.contains("NEW HIGH SCORE"), "{screen}");
        assert!(screen.contains("enter confirms"), "{screen}");

        game.phase = Phase::Victory;
        game.menu = 0;
        let screen = render(&game, 80, 24);
        assert!(screen.contains("ENDLESS"), "{screen}");
        assert!(screen.contains("FINISH RUN"), "{screen}");
    }

    #[test]
    fn the_floor_line_is_the_overdrive_meter() {
        let mut game = locked_rock_game(Theme::CatCafe, "comet");
        game.drive = 30.0;
        let screen = render(&game, 80, 24);
        let floor = screen.lines().nth(21).unwrap();
        let filled = floor.chars().filter(|&c| c == '━').count();
        assert!((38..=42).contains(&filled), "half full\n{floor}");
        game.drive = 60.0;
        let screen = render(&game, 80, 24);
        assert!(screen.contains("TAB » OVERDRIVE"), "{screen}");
    }
}
