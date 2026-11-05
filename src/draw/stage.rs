//! The typing stage: the target word in big pixel letters along the bottom.
//!
//! Typed letters slide up from the floor line, the rest of the word assembles
//! pixel by pixel in a ghosted tone when a hazard is locked, and a finished
//! word dissolves upward. Timing comes from `Game::buffer_t`, `lock_t` and
//! `spent`; this module only reads them.

use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

use super::bigfont::{lit, GLYPH_H, GLYPH_W};
use super::bigtext::{blit, noise, scale_for, Grid};
use super::{fade, mix, put, style, Palette};
use crate::game::{Game, Metrics};

/// Seconds for a typed letter to rise onto the floor.
const SLIDE: f32 = 0.14;
/// Seconds for a freshly locked word to finish assembling.
const ASSEMBLE: f32 = 0.22;
/// Seconds a finished word takes to dissolve.
const SHATTER: f32 = 0.34;

pub fn draw_stage(buf: &mut Buffer, game: &Game, pal: &Palette, m: Metrics) {
    if m.stage_h < 2 {
        return;
    }
    let floor_y = (m.stage_y + m.stage_h - 1) as i32;
    let missed = game.miss > 0.0;
    draw_floor(buf, game, pal, m.play_w, floor_y);

    let spent_word = game
        .spent
        .as_ref()
        .filter(|s| game.time - s.t0 < SHATTER)
        .map(|s| s.word.as_str());
    let Some(word) = game.target_word().or(spent_word) else {
        return;
    };
    // Boss typos can run past the end of the word; those extras show too.
    let extra = game.buffer.get(word.len()..).unwrap_or("");
    let letters: Vec<char> = word.chars().chain(extra.chars()).collect();
    let typo = |i: usize| game.buffer.as_bytes().get(i) != word.as_bytes().get(i);
    let s = scale_for(letters.len(), m.play_w);
    let cols = letters.len() * (GLYPH_W + 1) * s;
    let rows = (m.stage_h - 1) as usize * if m.stage_h > 5 { 1 } else { 2 };
    let x0 = (m.play_w as i32 - cols as i32) / 2;
    // One spare column each side holds the insertion bar.
    let mut grid = Grid::new(x0 - 1, cols + 2, rows);
    // Letters sit on the floor: pad the top when the grid is taller than 7.
    let pad = rows as i32 - GLYPH_H as i32;

    let typed = game.buffer.chars().count();
    let finishing = game.target_word().is_none();
    for (i, &c) in letters.iter().enumerate() {
        let typed_now = finishing || i < typed;
        for py in 0..GLYPH_H {
            for px in 0..GLYPH_W {
                if !lit(c, px, py) {
                    continue;
                }
                let n = noise(i, px, py);
                let (row, color) = if finishing {
                    let age = game.time - game.spent.as_ref().map_or(0.0, |s| s.t0);
                    if age > SHATTER * (0.45 + n * 0.55) {
                        continue;
                    }
                    let rise = (age * (14.0 + n * 26.0)) as i32;
                    (
                        py as i32 + pad - rise,
                        mix(pal.hot, pal.accent, age / SHATTER),
                    )
                } else if typed_now {
                    let age = game.time - game.buffer_t.get(i).copied().unwrap_or(0.0);
                    let t = (age / SLIDE).clamp(0.0, 1.0);
                    let drop = ((1.0 - t) * (1.0 - t) * GLYPH_H as f32).round() as i32;
                    let col = if missed || typo(i) {
                        pal.danger
                    } else if age < 0.1 {
                        pal.ink
                    } else {
                        pal.hot
                    };
                    (py as i32 + pad + drop, col)
                } else {
                    let age = game.time - game.lock_t - i as f32 * 0.02;
                    if age < n * ASSEMBLE {
                        continue;
                    }
                    let spark = age < n * ASSEMBLE + 0.06;
                    (py as i32 + pad, if spark { pal.hot } else { pal.dim })
                };
                for dx in 0..s {
                    let cx = x0 + ((i * (GLYPH_W + 1) + px) * s + dx) as i32;
                    grid.set(cx, row, color);
                }
            }
        }
    }

    // Blinking insertion bar in the gap before the next letter to type.
    let blink_on = ((game.time * 3.0) as i32).rem_euclid(2) == 0;
    if !finishing && typed < letters.len() && blink_on {
        let cx = x0 + (typed * (GLYPH_W + 1) * s) as i32 - 1;
        for py in 0..GLYPH_H {
            grid.set(cx, py as i32 + pad, pal.amber);
        }
    }

    blit(buf, &grid, m.stage_y, (m.stage_h - 1) as usize);
}

/// The floor line under the letters doubles as the Overdrive meter: it
/// fills in amber as clean letters charge it, shimmers when full, and
/// drains in a bright line while Overdrive runs. It flashes after a long
/// word, turns red on a miss, and breathes red at one heart.
pub fn draw_floor(buf: &mut Buffer, game: &Game, pal: &Palette, width: u16, y: i32) {
    let w = width as usize;
    let pulse = (game.time * 3.0).sin() * 0.5 + 0.5;
    let base = if game.miss > 0.0 {
        pal.danger
    } else if game.floor_flash > 0.0 {
        pal.hot
    } else if game.lives == 1 {
        mix(fade(pal.dim, 0.7), pal.danger, 0.25 + 0.35 * pulse)
    } else {
        fade(pal.dim, 0.7)
    };
    put(buf, 0, y, &"─".repeat(w), style(base));
    let (ratio, col) = if game.overdrive > 0.0 {
        (game.overdrive_ratio(), pal.hot)
    } else if game.drive_ready() {
        let shimmer = ((game.time * 8.0) as i32).rem_euclid(2) == 0;
        (1.0, if shimmer { pal.hot } else { pal.amber })
    } else {
        (game.drive_ratio(), fade(pal.amber, 0.8))
    };
    let filled = (ratio * w as f32).round() as usize;
    if filled > 0 && game.miss <= 0.0 {
        put(buf, 0, y, &"━".repeat(filled.min(w)), style(col));
    }
    let label = if game.overdrive > 0.0 {
        Some(" OVERDRIVE ")
    } else if game.drive_ready() {
        Some(" TAB » OVERDRIVE ")
    } else {
        None
    };
    if let Some(label) = label {
        let x = w.saturating_sub(label.chars().count() + 2) as i32;
        put(
            buf,
            x,
            y,
            label,
            style(pal.hot).add_modifier(Modifier::BOLD),
        );
    }
}
