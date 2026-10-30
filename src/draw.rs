//! Space, underwater, and jungle scenes. Ships, hazards, and projectiles are
//! braille line art; words sit on top in plain cells so they stay readable.

use crate::game::{
    metrics, rotate, spaceship_local, submarine_local, Effect, Game, Phase, Theme, MENU_ITEMS,
};
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
};

fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Space => &SPACE,
        Theme::Underwater => &UNDERWATER,
        Theme::Jungle => &JUNGLE,
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

/// Hanging vines, and palm fronds drooping in from the canopy edges, frame the
/// play field. Everything here sits at the lowest braille priority so hazards
/// and the monkey always stay on top.
fn draw_jungle_scenery(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
) {
    let vine_col = fade(pal.star, 0.45);
    let leaf_col = fade(pal.star, 0.62);
    let frond_col = fade(pal.accent, 0.34);
    let fern_col = fade(pal.faint, 0.5);

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

    // A couple of long vines hanging from the canopy.
    let anchors = [0.18, 0.55, 0.86];
    for (i, anchor) in anchors.iter().enumerate() {
        let seed = i as f32 * 1.7;
        let sway = (game.time * 0.28 + seed).sin();
        let x0 = game.play_w * anchor + sway * 0.3;
        let len = 3.0 + (seed * 1.3).sin().abs() * 2.4;
        let steps = 8;
        let mut prev = (x0, -0.5);
        for step in 1..=steps {
            let u = step as f32 / steps as f32;
            let x = x0 + (u * 3.4 + game.time * 0.35 + seed).sin() * 0.24;
            let y = -0.5 + len * u;
            let a = to_px(prev.0, prev.1);
            let b = to_px(x, y);
            ink.line(a.0, a.1, b.0, b.1, 0, vine_col);
            prev = (x, y);
        }
        for k in 1..=2 {
            let u = k as f32 / 2.0;
            let x = x0 + (u * 3.4 + game.time * 0.35 + seed).sin() * 0.24;
            let y = -0.5 + len * u;
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let flop = (game.time * 0.4 + seed + k as f32).sin() * 0.15;
            let angle = side * (1.0 + flop);
            let (s, c) = angle.sin_cos();
            let len_leaf = 1.0 - u * 0.2;
            let head = (x - s * len_leaf * 0.4, y + c * len_leaf * 0.4);
            ink_leaf(ink, to_px, head, angle, (len_leaf, 0.45), 0, leaf_col);
        }
    }

    // A little undergrowth creeping up the bottom corners.
    for (dir, cx) in [(-1.0f32, 0.2f32), (1.0, game.play_w - 0.2)] {
        for k in 0..2 {
            let t = k as f32 / 1.0;
            let angle = dir * (0.2 + t * 0.5) + (game.time * 0.25 + t).sin() * 0.06;
            let len = 1.5 - t * 0.4;
            let base_x = cx + dir * t * 0.9;
            let base_y = game.play_h + 0.4;
            let (s, c) = angle.sin_cos();
            let head = (base_x - s * len * 0.5, base_y - c * len * 0.5);
            ink_leaf(ink, to_px, head, angle + PI, (len, 0.35), 0, fern_col);
        }
    }
}

/// Falling leaves and blinking fireflies fill the jungle canopy.
fn draw_jungle_particles(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: &impl Fn(f32, f32) -> (f32, f32),
) {
    let play_h = game.play_h.max(1.0);
    for star in &game.stars {
        if star.glyph == '✦' {
            // Firefly: a slow bob and a soft amber pulse.
            let y = (star.y + (game.time * 0.25 + star.phase).sin() * 0.5).rem_euclid(play_h);
            let x = star.x + (game.time * 0.45 + star.phase * 1.7).sin() * 0.6;
            let pulse = (game.time * 2.4 + star.phase * 2.0).sin() * 0.5 + 0.5;
            let col = mix(pal.star, pal.amber, 0.35 + 0.5 * pulse);
            let (px, py) = to_px(x, y);
            ink.dot(px, py, 0, col);
            ink.dot(px + 1.0, py, 0, col);
            ink.dot(px, py + 1.0, 0, col);
            ink.dot(px + 1.0, py + 1.0, 0, col);
            continue;
        }

        // Leaf: drift downward, sway, and tumble so it flashes edge-on.
        let speed = 0.28 + (star.phase.sin() * 0.5 + 0.5) * 0.75;
        let y = (star.y + game.time * speed).rem_euclid(play_h);
        let sway = (game.time * (0.7 + speed * 0.4) + star.phase).sin()
            * (0.7 + (star.phase * 1.3).sin().abs() * 0.9);
        let x = star.x + sway;
        let large = star.glyph == '❧';
        let (len, width) = if large { (2.45, 1.4) } else { (1.35, 0.8) };
        let angle = star.phase + game.time * if large { 0.42 } else { 0.85 };
        // The leaf appears to spin around its long axis: it narrows or widens
        // as the face turns toward or away from the viewer.
        let tumble = (game.time * (1.1 + speed) + star.phase * 2.3).cos().abs();
        let width = width * (0.55 + 0.45 * tumble);
        let roll = (star.phase * 3.7).sin() * 0.5 + 0.5;
        let col = if roll > 0.94 {
            mix(pal.star, pal.amber, 0.5)
        } else if roll > 0.82 {
            mix(pal.star, pal.accent, 0.4)
        } else if large {
            pal.star
        } else {
            pal.faint
        };
        ink_leaf(ink, to_px, (x, y), angle, (len, width), 0, col);
    }
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
                _ => (star.x.round() as i32, star.y.round() as i32 + oy),
            };
            put(buf, x, y, &star.glyph.to_string(), style(pal.star));
        }
    }

    for effect in &game.effects {
        let t = effect.life_ratio();
        match effect {
            Effect::Laser { x0, y0, x1, y1, .. } => match game.theme {
                Theme::Space => {
                    let (a, b) = to_px(*x0, *y0);
                    let (c, d) = to_px(*x1, *y1);
                    ink.line(a, b, c, d, 4, fade(pal.hot, 0.25 + 0.75 * t));
                }
                Theme::Underwater => {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let dist = (dx * dx + dy * dy).sqrt().max(0.001);
                    let (ux, uy) = (dx / dist, dy / dist);
                    let progress = 1.0 - t;
                    let hx = x0 + dx * progress;
                    let hy = y0 + dy * progress;
                    let trail = dist.min(2.4);
                    for i in 0..7 {
                        let f = i as f32 / 6.0;
                        let wob = (i as f32 * 1.9 + game.time * 24.0).sin() * 0.09;
                        let bx = hx - ux * trail * f - uy * wob;
                        let by = hy - uy * trail * f + ux * wob;
                        let (px, py) = to_px(bx, by);
                        ink.dot(px, py, 3, fade(pal.accent, (1.0 - f) * (0.25 + 0.75 * t)));
                    }
                    let (a, b) = to_px(hx - ux * 0.5, hy - uy * 0.5);
                    let (c, d) = to_px(hx, hy);
                    ink.line(a, b, c, d, 5, pal.hot);
                    ink.dot(c, d, 5, pal.hot);
                }
                Theme::Jungle => {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let dist = (dx * dx + dy * dy).sqrt().max(0.001);
                    let (ux, uy) = (dx / dist, dy / dist);
                    let progress = 1.0 - t;
                    let hx = x0 + dx * progress;
                    let hy = y0 + dy * progress;

                    // A bright trail behind the thrown banana.
                    let trail = dist.min(1.9);
                    for i in 0..7 {
                        let f = i as f32 / 6.0;
                        let wob = (i as f32 * 2.1 + game.time * 20.0).sin() * 0.08;
                        let bx = hx - ux * trail * f - uy * wob;
                        let by = hy - uy * trail * f + ux * wob;
                        let (px, py) = to_px(bx, by);
                        ink.dot(px, py, 3, fade(pal.amber, (1.0 - f) * (0.18 + 0.82 * t)));
                    }

                    // The banana itself: a short curved crescent along the flight.
                    let (bx, by) = (-uy, ux);
                    let span = 0.9;
                    let bow = 0.24;
                    let mut prev: Option<(f32, f32)> = None;
                    for i in 0..=5 {
                        let f = i as f32 / 5.0 - 0.5;
                        let bend = (1.0 - (2.0 * f).abs()) * bow;
                        let px = hx + ux * span * f + bx * bend;
                        let py = hy + uy * span * f + by * bend;
                        let p = to_px(px, py);
                        if let Some(q) = prev {
                            ink.line(q.0, q.1, p.0, p.1, 5, pal.amber);
                        }
                        prev = Some(p);
                    }
                    let stem_f = -0.5;
                    let (sx, sy) = to_px(hx + ux * span * stem_f, hy + uy * span * stem_f);
                    ink.dot(sx, sy, 5, pal.hot);
                }
            },
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
                };
                for i in 0..steps {
                    let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                    let wob = (a * 3.0 + game.time * 2.4).sin() * wobble;
                    let rr = *r + wob;
                    let (px, py) = to_px(x + a.cos() * rr, y + a.sin() * rr * oval);
                    ink.dot(px, py, 3, col);
                }
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
            // Mine horns.
            let spikes = 10;
            for i in 0..spikes {
                let a = rock.rot + i as f32 / spikes as f32 * std::f32::consts::TAU;
                let inner = rock.radius * 0.92;
                let outer = rock.radius * (1.26 + (i % 2) as f32 * 0.13);
                let (x0, y0) = to_px(rock.x + a.cos() * inner, rock.y + a.sin() * inner);
                let (x1, y1) = to_px(rock.x + a.cos() * outer, rock.y + a.sin() * outer);
                ink.line(x0, y0, x1, y1, pri, col);
                ink.dot(x1, y1, pri, col);
            }
        }

        // Rock / mine body.
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

        if game.theme == Theme::Underwater {
            // A blinking contact fuse in the middle.
            let pulse = (game.time * 4.0 + rock.id as f32).sin();
            let core = if aimed || rock.hit > 0.0 {
                pal.hot
            } else if pulse > 0.55 {
                pal.danger
            } else {
                pal.dim
            };
            let (cx, cy) = to_px(rock.x, rock.y);
            ink.dot(cx, cy, 3, core);
        }

        if game.theme == Theme::Jungle {
            // Coconut stem, plus the three germination pores ("eyes").
            let (ax, ay) = rotate(0.0, -rock.radius * 0.96, rock.rot);
            let (bx, by) = rotate(0.0, -rock.radius * 1.16, rock.rot);
            let (x0, y0) = to_px(rock.x + ax, rock.y + ay);
            let (x1, y1) = to_px(rock.x + bx, rock.y + by);
            ink.line(x0, y0, x1, y1, pri, pal.faint);
            ink.dot(x1, y1, pri, pal.faint);

            let pores = [(-0.28, -0.16), (0.28, -0.16), (0.0, 0.24)];
            for (ox, oy) in pores {
                let (px, py) = rotate(ox, oy, rock.rot);
                let (x, y) = to_px(rock.x + px, rock.y + py);
                ink.dot(x, y, 4, Color::Rgb(58, 34, 18));
            }
        }

        if aimed {
            let steps = 20;
            let rr = rock.radius + 0.55;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * std::f32::consts::TAU + game.time * 0.8;
                let (px, py) = to_px(rock.x + a.cos() * rr, rock.y + a.sin() * rr * 0.55);
                ink.dot(px, py, 2, pal.amber);
            }
        }
    }

    let blink_off = game.invuln > 0.0 && ((game.time * 14.0) as i32).rem_euclid(2) == 0;
    if !blink_off {
        match game.theme {
            Theme::Space => draw_space_ship(&mut ink, game, pal, to_px),
            Theme::Underwater => draw_submarine(&mut ink, game, pal, to_px),
            Theme::Jungle => draw_monkey(&mut ink, game, pal, to_px),
        }
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
                rock.radius,
                &rock.word,
                oy,
            );
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
        hline(buf, 0, w, 1, pal.danger);
        hline(buf, 0, w, m.dock_y.saturating_sub(1), pal.danger);
    }

    draw_header(buf, game, pal, w);
    draw_dock(buf, game, pal, w, m.dock_y);
    draw_title(buf, game, pal, w, m);
    draw_modal(buf, game, pal, w, m);
}

fn draw_space_ship(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: impl Fn(f32, f32) -> (f32, f32),
) {
    let (cx, cy) = game.ship_pos();
    let pts = spaceship_local(game.spin);
    let world: Vec<(f32, f32)> = pts
        .iter()
        .map(|(lx, ly)| {
            let (x, y) = rotate(*lx, *ly, game.angle);
            (cx + x, cy + y)
        })
        .collect();
    for i in 0..world.len() {
        let a = world[i];
        let b = world[(i + 1) % world.len()];
        let (x0, y0) = to_px(a.0, a.1);
        let (x1, y1) = to_px(b.0, b.1);
        ink.line(x0, y0, x1, y1, 5, pal.ink);
    }

    let flick = 0.55 + 0.45 * (game.time * 34.0).sin().abs();
    let yaw = game.spin.abs() > 1.5;
    if game.thrust > 0.04 || yaw {
        let len = (0.25 + game.thrust * 1.55) * flick;
        let (x0, y0) = rotate(0.0, 0.62, game.angle);
        let (x1, y1) = rotate(0.0, 0.62 + len, game.angle);
        let (a, b) = to_px(cx + x0, cy + y0);
        let (c, d) = to_px(cx + x1, cy + y1);
        ink.line(a, b, c, d, 5, pal.accent);
        let (l0, l1) = rotate(-0.28, 0.7 + len * 0.65, game.angle);
        let (r0, r1) = rotate(0.28, 0.7 + len * 0.65, game.angle);
        let (p0, q0) = to_px(cx + l0, cy + l1);
        let (p1, q1) = to_px(cx + r0, cy + r1);
        let (n0, m0) = to_px(cx + x0, cy + y0);
        ink.line(n0, m0, p0, q0, 5, pal.amber);
        ink.line(n0, m0, p1, q1, 5, pal.amber);
    }
    if game.spin.abs() > 1.6 {
        let side = if game.spin > 0.0 { -1.0 } else { 1.0 };
        let (x0, y0) = rotate(side * 0.95, 0.55, game.angle);
        let (x1, y1) = rotate(side * 1.7, 0.95, game.angle);
        let (a, b) = to_px(cx + x0, cy + y0);
        let (c, d) = to_px(cx + x1, cy + y1);
        ink.line(a, b, c, d, 5, pal.amber);
    }
}

fn draw_submarine(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: impl Fn(f32, f32) -> (f32, f32),
) {
    let (cx, cy) = game.ship_pos();
    let place = |p: (f32, f32)| {
        let (x, y) = rotate(p.0, p.1, game.angle);
        (cx + x, cy + y)
    };
    let seg = |ink: &mut InkLayer, a: (f32, f32), b: (f32, f32), pri: u8, col: Color| {
        let (ax, ay) = place(a);
        let (bx, by) = place(b);
        let (x0, y0) = to_px(ax, ay);
        let (x1, y1) = to_px(bx, by);
        ink.line(x0, y0, x1, y1, pri, col);
    };
    let dot = |ink: &mut InkLayer, p: (f32, f32), pri: u8, col: Color| {
        let (px, py) = place(p);
        let (x, y) = to_px(px, py);
        ink.dot(x, y, pri, col);
    };

    let pts = submarine_local(game.spin);
    for i in 0..pts.len() {
        seg(ink, pts[i], pts[(i + 1) % pts.len()], 5, pal.ink);
    }
    seg(ink, (0.0, -1.72), (0.0, 1.48), 4, pal.dim);

    let bank = (-game.spin * 0.025).clamp(-0.5, 0.5) * 0.6;
    let sail = [
        (-0.45 + bank, -0.45),
        (0.45 + bank, -0.45),
        (0.54 + bank, -0.12),
        (0.45 + bank, 0.55),
        (0.24 + bank, 0.72),
        (-0.24 + bank, 0.72),
        (-0.45 + bank, 0.55),
        (-0.54 + bank, -0.12),
    ];
    for i in 0..sail.len() {
        seg(ink, sail[i], sail[(i + 1) % sail.len()], 5, pal.ink);
    }
    seg(ink, (-0.27 + bank, 0.03), (0.27 + bank, 0.03), 4, pal.amber);

    // Periscope and bridge.
    seg(ink, (0.0, -0.45), (0.0, -1.12), 4, pal.dim);
    seg(ink, (-0.27, -1.04), (0.27, -1.04), 4, pal.hot);

    // Portholes.
    for py in [-1.15, 0.95] {
        for px in [-0.45, 0.45] {
            dot(ink, (px, py), 4, pal.amber);
            dot(ink, (px * 0.78, py), 3, fade(pal.amber, 0.4));
        }
    }

    // Bow planes and stern fins.
    seg(ink, (0.68, -0.95), (1.52, -1.18), 4, pal.dim);
    seg(ink, (-0.68, -0.95), (-1.52, -1.18), 4, pal.dim);
    seg(ink, (0.52, 1.05), (1.36, 1.45), 4, pal.dim);
    seg(ink, (-0.52, 1.05), (-1.36, 1.45), 4, pal.dim);

    // Spinning propeller.
    let prop_y = 1.62;
    let prop = game.time * (7.0 + game.thrust * 18.0);
    for i in 0..3 {
        let a = prop + i as f32 * std::f32::consts::TAU / 3.0;
        let tip = (a.cos() * 0.72, prop_y + a.sin() * 0.32);
        seg(ink, (0.0, prop_y), tip, 4, fade(pal.accent, 0.75));
        dot(ink, tip, 3, pal.accent);
    }
    dot(ink, (0.0, prop_y), 5, pal.hot);

    let yaw = game.spin.abs() > 1.5;
    if game.thrust > 0.04 || yaw {
        let flick = 0.55 + 0.45 * (game.time * 34.0).sin().abs();
        let len = (0.25 + game.thrust * 1.15) * flick;
        seg(
            ink,
            (0.0, 1.78),
            (0.0, 1.78 + len),
            5,
            fade(pal.accent, 0.75),
        );
        dot(ink, (-0.11, 1.9 + len * 0.45), 3, pal.hot);
        dot(ink, (0.09, 2.0 + len * 0.65), 3, pal.accent);
    }
    if game.spin.abs() > 1.6 {
        let side = if game.spin > 0.0 { -1.0 } else { 1.0 };
        seg(ink, (side * 1.3, 1.08), (side * 1.7, 1.5), 5, pal.amber);
    }
}

fn draw_monkey(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: impl Fn(f32, f32) -> (f32, f32),
) {
    let (cx, cy) = game.ship_pos();
    // The monkey reads better a little larger than the other ships.
    let scale = 2.0;
    let place = |p: (f32, f32)| {
        let (x, y) = rotate(p.0 * scale, p.1 * scale, game.angle);
        (cx + x, cy + y)
    };
    let seg = |ink: &mut InkLayer, a: (f32, f32), b: (f32, f32), pri: u8, col: Color| {
        let (ax, ay) = place(a);
        let (bx, by) = place(b);
        let (x0, y0) = to_px(ax, ay);
        let (x1, y1) = to_px(bx, by);
        ink.line(x0, y0, x1, y1, pri, col);
    };
    let dot = |ink: &mut InkLayer, p: (f32, f32), pri: u8, col: Color| {
        let (px, py) = place(p);
        let (x, y) = to_px(px, py);
        ink.dot(x, y, pri, col);
    };
    let ring = |ink: &mut InkLayer,
                center: (f32, f32),
                rx: f32,
                ry: f32,
                steps: usize,
                pri: u8,
                col: Color| {
        let mut prev = (center.0 + rx, center.1);
        for i in 1..=steps {
            let a = i as f32 / steps as f32 * TAU;
            let p = (center.0 + a.cos() * rx, center.1 + a.sin() * ry);
            seg(ink, prev, p, pri, col);
            prev = p;
        }
    };

    // Tail curls behind the body.
    let wag = (game.time * 2.6).sin() * 0.16 + game.spin * 0.03;
    seg(ink, (0.0, 1.0), (0.3, 1.5 + wag), 3, pal.dim);
    seg(ink, (0.3, 1.5 + wag), (0.9, 1.7 + wag), 3, pal.dim);
    seg(ink, (0.9, 1.7 + wag), (1.2, 1.4 + wag), 3, pal.dim);
    seg(ink, (1.2, 1.4 + wag), (1.05, 1.1 + wag), 3, pal.dim);

    // Legs and feet.
    seg(ink, (-0.3, 1.1), (-0.42, 1.5), 4, pal.ink);
    seg(ink, (-0.42, 1.5), (-0.65, 1.55), 4, pal.ink);
    seg(ink, (0.3, 1.1), (0.42, 1.5), 4, pal.ink);
    seg(ink, (0.42, 1.5), (0.65, 1.55), 4, pal.ink);

    // Body and head.
    ring(ink, (0.0, 0.52), 0.55, 0.62, 12, 5, pal.ink);
    ring(ink, (0.0, -0.82), 0.75, 0.75, 12, 5, pal.ink);

    // Ears.
    ring(ink, (-0.88, -0.85), 0.28, 0.3, 8, 5, pal.ink);
    ring(ink, (0.88, -0.85), 0.28, 0.3, 8, 5, pal.ink);

    // Face.
    dot(ink, (-0.28, -0.96), 5, pal.hot);
    dot(ink, (0.28, -0.96), 5, pal.hot);
    dot(ink, (-0.1, -1.22), 4, pal.dim);
    dot(ink, (0.1, -1.22), 4, pal.dim);
    seg(ink, (-0.18, -1.32), (0.18, -1.32), 4, pal.dim);

    // Hair tuft.
    seg(ink, (0.0, -1.56), (-0.12, -1.76), 3, pal.ink);
    seg(ink, (0.0, -1.56), (0.12, -1.76), 3, pal.ink);

    // Arms. The right hand swings forward whenever a banana is thrown.
    seg(ink, (-0.5, 0.15), (-0.95, 0.45), 4, pal.ink);
    seg(ink, (-0.95, 0.45), (-1.15, 0.85), 4, pal.ink);
    dot(ink, (-1.15, 0.85), 4, pal.ink);

    // A banana ready in the other hand.
    seg(ink, (-1.1, 0.76), (-1.3, 0.9), 4, pal.amber);
    seg(ink, (-1.3, 0.9), (-1.18, 1.07), 4, pal.amber);
    dot(ink, (-1.14, 1.09), 4, pal.hot);

    let throw = game.thrust.clamp(0.0, 1.0);
    let elbow = (0.9, 0.5 - 0.3 * throw);
    let hand = (1.12 + 0.3 * throw, 0.7 - 1.4 * throw);
    seg(ink, (0.5, 0.15), elbow, 4, pal.ink);
    seg(ink, elbow, hand, 4, pal.ink);
    dot(ink, hand, 4, pal.ink);
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
    let len = word.len() as i32;
    let max_left = (game.play_w as i32 - len).max(0);
    let left = (x.round() as i32 - len / 2).clamp(0, max_left);
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
    for (i, ch) in word.chars().enumerate() {
        let style = if i < matched {
            style(pal.amber).add_modifier(Modifier::BOLD)
        } else if locked {
            style(pal.ink)
        } else {
            style(pal.dim)
        };
        put(buf, left + i as i32, row + origin_y, &ch.to_string(), style);
    }
}

fn draw_header(buf: &mut ratatui::buffer::Buffer, game: &Game, pal: &Palette, w: u16) {
    hline(buf, 0, w, 1, pal.dim);
    let mut x = 1u16;
    x = write(
        buf,
        x,
        0,
        "TYPOIDS",
        style(pal.ink).add_modifier(Modifier::BOLD),
    );
    x = write(
        buf,
        x,
        0,
        &format!("  WAVE {:02}", game.wave.max(1)),
        style(pal.dim),
    );
    x = write(
        buf,
        x,
        0,
        &format!("  SCORE {:06}", game.score),
        style(pal.amber),
    );
    if w >= 72 {
        x = write(
            buf,
            x,
            0,
            &format!("  BEST {:06}", game.best),
            style(pal.dim),
        );
    }
    x = write(buf, x, 0, "  HULL ", style(pal.dim));
    for i in 0..3 {
        let col = if i < game.lives { pal.ink } else { pal.dead };
        x = write(buf, x, 0, "#", style(col));
        if i != 2 {
            x = write(buf, x, 0, " ", style(pal.dim));
        }
    }
    if w >= 60 && game.combo >= 2 {
        let _ = write(
            buf,
            x,
            0,
            &format!("  x{}", game.combo),
            style(pal.amber).add_modifier(Modifier::BOLD),
        );
    }
}

fn draw_dock(buf: &mut ratatui::buffer::Buffer, game: &Game, pal: &Palette, w: u16, dock_y: u16) {
    hline(
        buf,
        0,
        w,
        dock_y,
        if game.miss > 0.0 { pal.danger } else { pal.dim },
    );
    let y1 = dock_y + 1;
    let y2 = dock_y + 2;
    let y3 = dock_y + 3;
    let y4 = dock_y + 4;

    match game.phase {
        Phase::Title => {
            put(
                buf,
                2,
                y1 as i32,
                "h j k l / arrows  move      enter  select",
                style(pal.ink),
            );
            put(
                buf,
                2,
                y2 as i32,
                match game.theme {
                    Theme::Space => "START launches. THEMES changes the look. QUIT exits.",
                    Theme::Underwater => "START dives. THEMES changes the look. QUIT exits.",
                    Theme::Jungle => "START swings. THEMES changes the look. QUIT exits.",
                },
                style(pal.dim),
            );
            put(
                buf,
                2,
                y3 as i32,
                if game.theme_menu_open {
                    "ENTER applies the theme. ESC goes back."
                } else {
                    "Q also quits"
                },
                style(pal.amber),
            );
        }
        Phase::Paused => {
            put(
                buf,
                2,
                y1 as i32,
                "PAUSED",
                style(pal.amber).add_modifier(Modifier::BOLD),
            );
            put(
                buf,
                2,
                y2 as i32,
                "ESC resumes     Q returns to title",
                style(pal.dim),
            );
        }
        Phase::GameOver => {
            put(
                buf,
                2,
                y1 as i32,
                match game.theme {
                    Theme::Space => "SHIP LOST",
                    Theme::Underwater => "HULL BREACH",
                    Theme::Jungle => "MONKEY DOWN",
                },
                style(pal.danger).add_modifier(Modifier::BOLD),
            );
            put(
                buf,
                2,
                y2 as i32,
                &format!("SCORE {:06}    BEST {:06}", game.score, game.best),
                style(pal.ink),
            );
            put(
                buf,
                2,
                y3 as i32,
                "ENTER selects     Q quits",
                style(pal.amber),
            );
        }
        Phase::Dying => {
            put(
                buf,
                2,
                y1 as i32,
                match game.theme {
                    Theme::Space => "SHIP LOST",
                    Theme::Underwater => "HULL BREACH",
                    Theme::Jungle => "MONKEY DOWN",
                },
                style(pal.danger).add_modifier(Modifier::BOLD),
            );
        }
        Phase::Playing => {
            let word = game.target_word().unwrap_or("");
            let nmatch = game.matches();
            if word.is_empty() {
                put(buf, 2, y1 as i32, "NO LOCK", style(pal.dim));
            } else {
                put(buf, 2, y1 as i32, "LOCK", style(pal.dim));
                let mut x = 8i32;
                for (i, ch) in word.chars().enumerate() {
                    let on = i < game.buffer.len();
                    let st = if on {
                        style(Color::Rgb(20, 16, 8))
                            .bg(pal.amber)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        style(pal.ink)
                    };
                    put(buf, x, y1 as i32, &ch.to_ascii_uppercase().to_string(), st);
                    x += 2;
                }
                if nmatch > 1 {
                    put(
                        buf,
                        x + 2,
                        y1 as i32,
                        &format!("{nmatch} MATCH"),
                        style(pal.dim),
                    );
                }
            }

            let cursor = if ((game.time * 3.0) as i32).rem_euclid(2) == 0 {
                "█"
            } else {
                " "
            };
            let typed = if game.miss > 0.08 && game.buffer.is_empty() {
                match game.theme {
                    Theme::Space => "no rock starts with that".to_string(),
                    Theme::Underwater => "no mine starts with that".to_string(),
                    Theme::Jungle => "no coconut starts with that".to_string(),
                }
            } else {
                format!("{}{cursor}", game.buffer)
            };
            let col = if game.miss > 0.0 {
                pal.danger
            } else {
                pal.amber
            };
            put(
                buf,
                2,
                y2 as i32,
                ">",
                style(col).add_modifier(Modifier::BOLD),
            );
            put(
                buf,
                4,
                y2 as i32,
                &typed,
                style(if game.miss > 0.0 { pal.danger } else { pal.hot })
                    .add_modifier(Modifier::BOLD),
            );

            let hint = match game.theme {
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
            };
            put(buf, 2, y3 as i32, &hint, style(pal.dim));
            if w >= 70 {
                put(
                    buf,
                    2,
                    y4 as i32,
                    match game.theme {
                        Theme::Space => {
                            "backspace erases    esc pauses    rocks that reach the hull cost a life"
                        }
                        Theme::Underwater => {
                            "backspace erases    esc pauses    mines that reach the sub cost a life"
                        }
                        Theme::Jungle => {
                            "backspace erases    esc pauses    coconuts that reach the monkey cost a life"
                        }
                    },
                    style(pal.faint),
                );
            }
        }
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

    let labels: Vec<String> = MENU_ITEMS
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
    x.saturating_add(text.len() as u16)
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

/// A shaded canopy: deep green gradient, dappled pools of light, and slow sun
/// shafts spilling through the leaves above the play field.
fn paint_jungle_canopy(buf: &mut ratatui::buffer::Buffer, area: Rect, pal: &Palette, time: f32) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let w = area.width as f32;
    let h = area.height.max(1) as f32;
    let leaf_light = Color::Rgb(64, 92, 44);
    let sun = Color::Rgb(146, 166, 92);
    let deep = Color::Rgb(1, 7, 4);
    for y in 0..area.height {
        let ty = y as f32 / h;
        let base = mix(pal.bg_top, pal.bg_bottom, ty.powf(0.82));
        let falloff = (1.0 - ty).powf(0.7);
        let ceiling = (1.0 - ty * 5.5).max(0.0);
        let sky = (ty * 21.0 - time * 0.15).sin();
        for x in 0..area.width {
            let fx = x as f32 / w;

            // Dappled light: three interacting waves keep the patches from
            // looking like a repeating pattern as they drift.
            let dapple =
                (fx * 47.0 + time * 0.24).sin() * sky * (fx * 13.0 - ty * 17.0 + 0.9).cos();
            let v = dapple * 0.5 + 0.5;
            let dappled = v * v * v;

            // Thin diagonal shafts falling from the canopy.
            let ray = ((fx * 5.0 - ty * 2.4) * PI + time * 0.12).sin() * 0.5 + 0.5;
            let shaft = ray.powi(11) * falloff;

            // Shadowed ceiling and a soft vignette toward the sides.
            let edge = (fx - 0.5).abs() * 2.0;
            let edge = edge * edge * edge;

            let mut col = mix(base, leaf_light, dappled * 0.16);
            col = mix(col, sun, shaft * 0.14);
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
    }
}
