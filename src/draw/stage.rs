//! The typing stage: the target word in big pixel letters along the bottom.
//!
//! Typed letters slide up from the floor line, the rest of the word assembles
//! pixel by pixel in a ghosted tone when a hazard is locked, and a finished
//! word dissolves upward. Timing comes from `Game::buffer_t`, `lock_t` and
//! `spent`; this module only reads them.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use super::bigfont::{lit, GLYPH_H, GLYPH_W};
use super::{fade, mix, put, style, Palette};
use crate::game::{Game, Metrics};

/// Seconds for a typed letter to rise onto the floor.
const SLIDE: f32 = 0.14;
/// Seconds for a freshly locked word to finish assembling.
const ASSEMBLE: f32 = 0.22;
/// Seconds a finished word takes to dissolve.
const SHATTER: f32 = 0.34;

/// One lit pixel of the stage, in letter-grid coordinates.
struct Grid {
    x0: i32,
    cols: usize,
    rows: usize,
    cells: Vec<Option<Color>>,
}

impl Grid {
    fn set(&mut self, cx: i32, row: i32, color: Color) {
        let x = cx - self.x0;
        if x < 0 || row < 0 || x as usize >= self.cols || row as usize >= self.rows {
            return;
        }
        self.cells[row as usize * self.cols + x as usize] = Some(color);
    }

    fn get(&self, x: usize, row: usize) -> Option<Color> {
        if row >= self.rows {
            return None;
        }
        self.cells[row * self.cols + x]
    }
}

/// Deterministic 0..1 noise for a pixel, so animations never use the RNG.
fn noise(letter: usize, px: usize, py: usize) -> f32 {
    let h = (letter as u32 + 1).wrapping_mul(73_856_093)
        ^ (px as u32 + 7).wrapping_mul(19_349_663)
        ^ (py as u32 + 13).wrapping_mul(83_492_791);
    (h % 1000) as f32 / 1000.0
}

/// Columns per letter pixel: wide when the word fits, narrow when it must.
fn scale_for(letters: usize, width: u16) -> usize {
    let fits = |s: usize| letters * (GLYPH_W + 1) * s <= width.saturating_sub(2) as usize;
    if fits(2) {
        2
    } else {
        1
    }
}

pub fn draw_stage(buf: &mut Buffer, game: &Game, pal: &Palette, m: Metrics) {
    if m.stage_h < 2 {
        return;
    }
    let floor_y = (m.stage_y + m.stage_h - 1) as i32;
    let missed = game.miss > 0.0;
    let floor_col = if missed {
        pal.danger
    } else {
        fade(pal.dim, 0.7)
    };
    let bar = "─".repeat(m.play_w as usize);
    put(buf, 0, floor_y, &bar, style(floor_col));

    let spent_word = game
        .spent
        .as_ref()
        .filter(|s| game.time - s.t0 < SHATTER)
        .map(|s| s.word.as_str());
    let Some(word) = game.target_word().or(spent_word) else {
        return;
    };
    let letters: Vec<char> = word.chars().collect();
    let s = scale_for(letters.len(), m.play_w);
    let cols = letters.len() * (GLYPH_W + 1) * s;
    let rows = (m.stage_h - 1) as usize * if m.stage_h > 5 { 1 } else { 2 };
    let x0 = (m.play_w as i32 - cols as i32) / 2;
    // One spare column each side holds the insertion bar.
    let mut grid = Grid {
        x0: x0 - 1,
        cols: cols + 2,
        rows,
        cells: vec![None; (cols + 2) * rows],
    };
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
                    let col = if missed {
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

    blit(buf, &grid, m);
}

/// Turn the pixel grid into cells: full blocks on tall stages, half blocks otherwise.
fn blit(buf: &mut Buffer, grid: &Grid, m: Metrics) {
    let half = grid.rows > (m.stage_h - 1) as usize;
    let cell_rows = (m.stage_h - 1) as usize;
    for r in 0..cell_rows {
        let y = m.stage_y + r as u16;
        for x in 0..grid.cols {
            let (top, bottom) = if half {
                (grid.get(x, r * 2), grid.get(x, r * 2 + 1))
            } else {
                (grid.get(x, r), None)
            };
            let cell_x = grid.x0 + x as i32;
            let (glyph, fg, bg) = match (top, bottom) {
                (None, None) => continue,
                (Some(a), None) if !half => ('█', a, None),
                (Some(a), Some(b)) if a == b => ('█', a, None),
                (Some(a), Some(b)) => ('▀', a, Some(b)),
                (Some(a), None) => ('▀', a, None),
                (None, Some(b)) => ('▄', b, None),
            };
            if cell_x < 0 || cell_x >= buf.area.width as i32 || y >= buf.area.height {
                continue;
            }
            let cell = &mut buf[(cell_x as u16, y)];
            cell.set_char(glyph).set_fg(fg);
            if let Some(bg) = bg {
                cell.set_bg(bg);
            }
        }
    }
}
