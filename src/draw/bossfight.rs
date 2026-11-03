//! Boss waves: the boss itself, its health and clock bars, and the passage.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use super::ships::Craft;
use super::{mix, put, style, InkLayer, Palette};
use crate::boss::{self, Boss};
use crate::game::{Game, Metrics, Phase, Theme};

fn visible(game: &Game) -> Option<&Boss> {
    game.boss
        .as_ref()
        .filter(|_| matches!(game.phase, Phase::Playing | Phase::Paused | Phase::Dying))
}

type Pt = (f32, f32);

/// The boss. Space, Jungle and Cat Cafe send a big angry head with a theme
/// accessory; Underwater sends a shark that swims laps. Either way it scars as
/// it loses health, lunges when it strikes, and leaves in its own way.
pub fn draw_boss(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: impl Fn(f32, f32) -> (f32, f32),
) {
    let Some(b) = visible(game) else {
        return;
    };
    let (bx, by) = game.boss_pos();
    // Beaten bosses blink out as they burst; timed-out ones float away.
    let (dy, gone) = match b.outcome {
        Some(true) => (0.0, b.outcome_t > 0.6),
        Some(false) => (-b.outcome_t * 7.0, by - b.outcome_t * 7.0 < -4.0),
        None => (0.0, false),
    };
    if gone || (b.outcome == Some(true) && ((b.outcome_t * 18.0) as i32) % 2 == 0) {
        return;
    }
    // A strike is one smooth lunge toward the ship and back.
    let lunge = if b.strike > 0.0 {
        (std::f32::consts::PI * (1.0 - b.strike / boss::STRIKE)).sin() * 1.2
    } else {
        0.0
    };
    let shark = game.theme == Theme::Underwater;
    let mut c = Craft {
        ink,
        to_px,
        at: (bx, by + dy + lunge),
        angle: 0.0,
        scale: boss::art_scale(game.play_h) * if shark { boss::SHARK_SCALE } else { 1.0 },
    };
    let body = if b.strike > 0.0 { pal.danger } else { pal.rock };
    let eye = if b.strike > 0.0 { pal.danger } else { pal.hot };
    let look = Look { body, eye, pal };

    let scars = if shark {
        draw_shark(&mut c, b.age, b.strike > 0.0, &look)
    } else {
        draw_head(&mut c, game.theme, b.age, &look)
    };
    // Scars show how hurt it is.
    let hurt = b.progress();
    for (i, scar) in scars.iter().enumerate() {
        if hurt > 0.25 * (i + 1) as f32 {
            c.path(scar, false, 3, pal.dim);
        }
    }
}

struct Look<'a> {
    body: Color,
    eye: Color,
    pal: &'a Palette,
}

type Scars = [[Pt; 3]; 3];

/// A big angry head: brow, eyes, jagged teeth, and the theme's accessory.
fn draw_head(
    c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>,
    theme: Theme,
    age: f32,
    look: &Look,
) -> Scars {
    let body = look.body;
    c.ring((0.0, 0.0), 1.3, 1.0, 4, body);
    c.pair(&[(-0.95, -0.6), (-0.2, -0.38)], false, 4, body);
    for s in [-1.0, 1.0] {
        c.ring((s * 0.5, -0.12), 0.26, 0.2, 4, body);
        c.dot((s * 0.5, -0.1), 4, look.eye);
    }
    let teeth: Vec<Pt> = (0..=8)
        .map(|i| {
            let x = -0.8 + 1.6 * i as f32 / 8.0;
            (x, if i % 2 == 0 { 0.38 } else { 0.68 })
        })
        .collect();
    c.path(&teeth, false, 4, body);
    accessory(c, theme, age, look.pal, body);
    [
        [(0.1, -1.0), (0.3, -0.7), (0.15, -0.5)],
        [(-1.0, 0.2), (-0.8, 0.4), (-0.95, 0.65)],
        [(0.9, 0.1), (0.7, 0.45), (0.85, 0.7)],
    ]
}

/// A great white side on: torpedo body, tall dorsal fin, beating tail, gills,
/// a mean little eye and a toothy grin. It faces the way it swims and narrows
/// through the turns; on a strike its jaw drops open.
fn draw_shark(
    c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>,
    age: f32,
    striking: bool,
    look: &Look,
) -> Scars {
    let h = boss::heading(age);
    // Never quite edge-on, so the turn reads as a bank rather than a vanish.
    let face = h.signum() * h.abs().max(0.2);
    let wag = (age * 5.5).sin() * 0.18;
    let gape = if striking { 0.22 } else { 0.0 };
    let flip = |pts: &[Pt]| -> Vec<Pt> { pts.iter().map(|p| (p.0 * face, p.1)).collect() };
    let body = look.body;

    let outline = [
        (2.3, 0.02),
        (1.95, -0.34),
        (1.2, -0.55),
        (0.35, -0.62),
        (-0.05, -1.45),
        (-0.5, -0.56),
        (-1.3, -0.38),
        (-2.05, -0.12),
        (-2.85, -0.95 + wag),
        (-2.45, 0.02 + wag * 0.5),
        (-2.75, 0.62 + wag),
        (-2.05, 0.14),
        (-1.3, 0.36),
        (-0.2, 0.5),
        (0.8, 0.46),
        (1.6, 0.36 + gape),
        (2.05, 0.2 + gape),
    ];
    c.path(&flip(&outline), true, 4, body);
    // Pectoral fin, with a lazy paddle.
    let paddle = (age * 2.2).sin() * 0.1;
    c.path(
        &flip(&[(0.65, 0.45), (0.05, 1.12 + paddle), (0.2, 0.48)]),
        false,
        4,
        body,
    );
    // Gill slits.
    for gx in [0.8, 0.98, 1.16] {
        c.seg(
            (gx * face, -0.22),
            ((gx - 0.08) * face, 0.2),
            3,
            look.pal.dim,
        );
    }
    // Eye and an angry brow.
    c.ring((1.62 * face, -0.2), 0.11, 0.1, 4, body);
    c.dot((1.64 * face, -0.2), 5, look.eye);
    c.seg((1.38 * face, -0.4), (1.84 * face, -0.3), 4, body);
    // Grin: a jaw line with a row of teeth, wider open on a strike.
    let teeth: Vec<Pt> = (0..=6)
        .map(|i| {
            let x = 2.02 - i as f32 * 0.14;
            let base = 0.17 + i as f32 * 0.02 + gape * (1.0 - i as f32 / 6.0);
            (x, if i % 2 == 0 { base } else { base + 0.13 })
        })
        .collect();
    c.path(&flip(&teeth), false, 4, look.pal.ink);
    let scars = [
        [(-0.1, -0.35), (0.15, -0.1), (0.0, 0.1)],
        [(-1.2, -0.2), (-1.0, 0.0), (-1.15, 0.2)],
        [(0.5, -0.45), (0.65, -0.2), (0.5, 0.05)],
    ];
    scars.map(|s| s.map(|p| (p.0 * face, p.1)))
}

fn accessory(
    c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>,
    theme: Theme,
    time: f32,
    pal: &Palette,
    body: Color,
) {
    match theme {
        // A saucer rim and wobbling antennae.
        Theme::Space => {
            c.ring((0.0, 0.7), 2.1, 0.3, 4, body);
            let wob = (time * 3.0).sin() * 0.15;
            for s in [-1.0, 1.0] {
                c.path(&[(s * 0.5, -0.9), (s * (0.8 + wob), -1.6)], false, 4, body);
                c.dot((s * (0.8 + wob), -1.7), 5, pal.amber);
            }
        }
        // Big ears and heavy arms, ready to thump.
        Theme::Jungle => {
            c.pair(&[(1.3, 0.0)], false, 4, body);
            for s in [-1.0, 1.0] {
                c.ring((s * 1.55, -0.1), 0.35, 0.3, 4, body);
                c.path(
                    &[(s * 1.2, 0.8), (s * 2.0, 1.2), (s * 1.8, 1.9)],
                    false,
                    4,
                    body,
                );
                c.ring((s * 1.8, 2.0), 0.3, 0.26, 4, body);
            }
        }
        // Cat ears and whiskers.
        Theme::CatCafe => {
            c.pair(&[(-1.05, -0.7), (-1.2, -1.7), (-0.4, -1.0)], false, 4, body);
            for s in [-1.0, 1.0] {
                c.seg((s * 1.3, 0.0), (s * 2.1, -0.2), 3, pal.dim);
                c.seg((s * 1.3, 0.3), (s * 2.1, 0.5), 3, pal.dim);
            }
        }
        // The shark is drawn whole by `draw_shark`.
        Theme::Underwater => {}
    }
}

fn bar(len: usize, filled: f32) -> String {
    let on = (filled.clamp(0.0, 1.0) * len as f32).round() as usize;
    (0..len).map(|i| if i < on { '█' } else { '░' }).collect()
}

/// Status line and passage for the boss fight.
pub fn draw_boss_ui(buf: &mut Buffer, game: &Game, pal: &Palette, m: Metrics) {
    let Some(b) = visible(game) else {
        return;
    };
    let w = m.play_w as i32;
    status_line(buf, game, b, pal, (m.origin_y as i32, w));
    let s = boss::art_scale(game.play_h);
    let (_, by) = game.boss_home();
    let top = m.origin_y as i32 + (by + 1.3 * 0.625 * s).round() as i32 + 1;
    let room = (game.ship_center().1 - 2.5).round() as i32 + m.origin_y as i32 - top;
    let rows = room.clamp(1, 3);
    passage_panel(buf, game, b, pal, (top, w, rows));
}

fn status_line(buf: &mut Buffer, game: &Game, b: &Boss, pal: &Palette, at: (i32, i32)) {
    let (y, w) = at;
    let bold = Modifier::BOLD;
    let name = boss::name(game.theme);
    let mut x = 2;
    put(buf, x, y, name, style(pal.amber).add_modifier(bold));
    x += name.chars().count() as i32 + 2;
    let len = (((w - x - 24) / 2).clamp(6, 16)) as usize;

    put(buf, x, y, "HP", style(pal.dim));
    put(
        buf,
        x + 3,
        y,
        &bar(len, 1.0 - b.progress()),
        style(pal.danger),
    );
    x += 4 + len as i32;

    let behind = b.time_left() / b.limit < 1.0 - b.progress();
    put(buf, x, y, "TIME", style(pal.dim));
    let clock = if behind { pal.danger } else { pal.ink };
    put(
        buf,
        x + 5,
        y,
        &bar(len, b.time_left() / b.limit),
        style(clock),
    );
    x += 6 + len as i32;
    let secs = b.time_left().ceil() as u32;
    put(
        buf,
        x,
        y,
        &format!("{secs}s"),
        style(clock).add_modifier(bold),
    );

    let pace = format!("{} / {} WPM", b.live_wpm(), b.wpm);
    let pace_col = if b.live_wpm() >= b.wpm {
        pal.hot
    } else {
        pal.dim
    };
    let px = w - 2 - pace.chars().count() as i32;
    put(buf, px, y, &pace, style(pace_col).add_modifier(bold));
}

/// Wrap the passage into lines of word indices.
fn wrap_words(words: &[&str], width: usize) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let (mut start, mut used) = (0, 0);
    for (i, w) in words.iter().enumerate() {
        let need = w.len() + usize::from(used > 0);
        if used + need > width && used > 0 {
            lines.push((start, i));
            start = i;
            used = 0;
        }
        used += w.len() + usize::from(used > 0);
    }
    lines.push((start, words.len()));
    lines
}

fn passage_panel(buf: &mut Buffer, game: &Game, b: &Boss, pal: &Palette, at: (i32, i32, i32)) {
    let (top, w, rows) = at;
    let panel_w = (w - 6).clamp(20, 62);
    let left = (w - panel_w) / 2;
    let bg = mix(pal.bg_top, pal.bg_bottom, 0.3);
    let fill = Style::default().bg(mix(bg, pal.chip, 0.6));
    for r in 0..rows {
        put(buf, left, top + r, &" ".repeat(panel_w as usize), fill);
    }
    if b.intro > 0.0 {
        let lines = [
            format!("BOSS FIGHT  ·  {} WPM", b.wpm),
            format!(
                "{} words in {}s  ·  wrong keys hurt",
                b.words.len(),
                b.limit.ceil()
            ),
            format!("starting in {}", b.intro.ceil()),
        ];
        for (i, line) in lines.iter().take(rows as usize).enumerate() {
            let col = if i == 0 { pal.amber } else { pal.ink };
            let x = left + (panel_w - line.chars().count() as i32) / 2;
            put(
                buf,
                x,
                top + i as i32,
                line,
                fill.fg(col).add_modifier(Modifier::BOLD),
            );
        }
        return;
    }

    let lines = wrap_words(&b.words, (panel_w - 4) as usize);
    let cur = lines
        .iter()
        .position(|&(a, z)| b.idx >= a && b.idx < z)
        .unwrap_or(lines.len() - 1);
    // Keep the current line in the middle when three lines show.
    let first = if rows >= 3 {
        cur.saturating_sub(1)
    } else {
        cur
    };
    let missed = game.miss > 0.0;
    for r in 0..rows as usize {
        let Some(&(a, z)) = lines.get(first + r) else {
            break;
        };
        let mut x = left + 2;
        for i in a..z {
            let word = b.words[i];
            if i == b.idx {
                let typed = game.buffer.len().min(word.len());
                let hot = if missed { pal.danger } else { pal.hot };
                put(
                    buf,
                    x,
                    top + r as i32,
                    &word[..typed],
                    fill.fg(hot).add_modifier(Modifier::BOLD),
                );
                put(
                    buf,
                    x + typed as i32,
                    top + r as i32,
                    &word[typed..],
                    fill.fg(pal.ink)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                );
            } else {
                let col = if i < b.idx { pal.faint } else { pal.dim };
                put(buf, x, top + r as i32, word, fill.fg(col));
            }
            x += word.len() as i32 + 1;
        }
    }
}
