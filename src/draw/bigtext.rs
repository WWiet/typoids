//! Pixel lettering shared by the typing stage and the arcade call-outs: a
//! grid of lit pixels, blitted to the terminal as half blocks.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use super::bigfont::{lit, GLYPH_H, GLYPH_W};

/// Lit pixels in letter-grid coordinates; `x0` is the terminal column of
/// grid column 0.
pub struct Grid {
    pub x0: i32,
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Option<Color>>,
}

impl Grid {
    pub fn new(x0: i32, cols: usize, rows: usize) -> Self {
        Grid {
            x0,
            cols,
            rows,
            cells: vec![None; cols * rows],
        }
    }

    pub fn set(&mut self, cx: i32, row: i32, color: Color) {
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

/// Columns per letter pixel: wide when the text fits, narrow when it must.
pub fn scale_for(letters: usize, width: u16) -> usize {
    let fits = |s: usize| letters * (GLYPH_W + 1) * s <= width.saturating_sub(2) as usize;
    if fits(2) {
        2
    } else {
        1
    }
}

/// Paint the grid into `cell_rows` terminal rows from `top`: two pixels
/// per cell (half blocks) when the grid is taller than the rows, else one.
pub fn blit(buf: &mut Buffer, grid: &Grid, top: u16, cell_rows: usize) {
    let half = grid.rows > cell_rows;
    for r in 0..cell_rows {
        let y = top + r as u16;
        for x in 0..grid.cols {
            let (upper, lower) = if half {
                (grid.get(x, r * 2), grid.get(x, r * 2 + 1))
            } else {
                (grid.get(x, r), None)
            };
            let cell_x = grid.x0 + x as i32;
            let (glyph, fg, bg) = match (upper, lower) {
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

/// Rows of terminal cells a line of pixel text takes (7 pixels, half blocks).
pub const TEXT_ROWS: u16 = 4;

/// Draw `text` in pixel letters centered in `width` columns from `left`,
/// with its top on terminal row `top`. `paint(letter, px, py)` colors each
/// lit pixel, or hides it with `None`; it may also nudge the pixel down by
/// returning a row offset. Returns false (drawing nothing) if the text
/// cannot fit even at one column per pixel.
pub fn draw_text(
    buf: &mut Buffer,
    text: &str,
    area: (i32, u16, u16),
    paint: impl Fn(usize, usize, usize) -> Option<(Color, i32)>,
) -> bool {
    let (left, top, width) = area;
    let letters: Vec<char> = text.chars().collect();
    let s = scale_for(letters.len(), width);
    let cols = letters.len() * (GLYPH_W + 1) * s;
    if cols > width as usize {
        return false;
    }
    let x0 = left + (width as i32 - cols as i32) / 2;
    let rows = TEXT_ROWS as usize * 2;
    let mut grid = Grid::new(x0, cols, rows);
    for (i, &c) in letters.iter().enumerate() {
        for py in 0..GLYPH_H {
            for px in 0..GLYPH_W {
                if !lit(c, px, py) {
                    continue;
                }
                let Some((color, drop)) = paint(i, px, py) else {
                    continue;
                };
                for dx in 0..s {
                    let cx = x0 + ((i * (GLYPH_W + 1) + px) * s + dx) as i32;
                    grid.set(cx, py as i32 + drop, color);
                }
            }
        }
    }
    blit(buf, &grid, top, TEXT_ROWS as usize);
    true
}

/// Deterministic 0..1 noise for a pixel, so animations never use the RNG.
pub fn noise(letter: usize, px: usize, py: usize) -> f32 {
    let h = (letter as u32 + 1).wrapping_mul(73_856_093)
        ^ (px as u32 + 7).wrapping_mul(19_349_663)
        ^ (py as u32 + 13).wrapping_mul(83_492_791);
    (h % 1000) as f32 / 1000.0
}
