//! The coin-op dressing: the attract-mode title (marquee, high-score
//! table, options, how-to-play), big pixel call-outs, the combo announcer,
//! and the initials screen.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use super::bigtext::{draw_text, noise, TEXT_ROWS};
use super::{clear_span, fade, mark, mix, put, put_centered, put_char, style, Palette};
use crate::game::{Game, Hit, Metrics, Phase, Theme, TitleAction, OPTION_ITEMS};
use crate::scores;

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
                let on = glyph[row] & (1 << col) != 0;
                let c = if on { '█' } else { ' ' };
                line.push(c);
                if wide {
                    line.push(c);
                }
            }
        }
        lines.push(line);
    }
    lines
}

fn subtitle(theme: Theme) -> &'static str {
    match theme {
        Theme::Space => "~ typing range ~",
        Theme::Underwater => "~ sonar range ~",
        Theme::Jungle => "~ canopy range ~",
        Theme::CatCafe => "~ cozy cat cafe ~",
    }
}

/// Dark text on a bright bar: the selected menu entry.
fn selected(pal: &Palette) -> Style {
    style(Color::Rgb(20, 16, 8))
        .bg(pal.amber)
        .add_modifier(Modifier::BOLD)
}

const HELP: [&str; 7] = [
    "Type the word on a hazard: the first letters lock it, the rest fire.",
    "Kills drop crystals ◆. Spend them in the shop between waves.",
    "Clean letters charge OVERDRIVE on the floor line. TAB fires it.",
    "End-to-start words (echo → orbit) LINK for a growing bonus.",
    "Every fifth wave is a boss: beat its clock or the run ends.",
    "backspace erases · ctrl+w clears · esc pauses · tab overdrive",
    "press any key",
];

pub fn draw_title(buf: &mut Buffer, game: &Game, pal: &Palette, w: u16, m: Metrics) {
    if game.phase != Phase::Title {
        return;
    }
    let body = body_rows(game);
    let avail = m.play_h as i32;
    let full_art = 7 + 3 + body <= avail;
    let art_h = if full_art { 7 } else { 1 };
    let block_h = art_h + 3 + body;
    let top = m.origin_y as i32 + (avail - block_h).max(0) / 2;

    if full_art {
        draw_marquee(buf, game, pal, w, top);
    } else {
        put_centered(
            buf,
            0,
            w as i32,
            top,
            "T Y P O I D S",
            style(pal.ink).add_modifier(Modifier::BOLD),
        );
    }
    let sub = subtitle(game.theme);
    let sub_y = top + art_h;
    blank_row(buf, w, sub_y, sub);
    put_centered(buf, 0, w as i32, sub_y, sub, style(pal.amber));
    let coin = "INSERT COIN  ·  PRESS ENTER";
    let coin_y = sub_y + 1;
    blank_row(buf, w, coin_y, coin);
    if ((game.time * 2.0) as i32).rem_euclid(2) == 0 {
        put_centered(
            buf,
            0,
            w as i32,
            coin_y,
            coin,
            style(pal.hot).add_modifier(Modifier::BOLD),
        );
    }

    let y = coin_y + 2;
    if game.help_open {
        draw_help(buf, pal, w, y);
    } else if game.options_open {
        draw_options(buf, game, pal, w, y);
    } else if game.attract_table() {
        draw_hiscores(buf, game, pal, w, y);
    } else {
        draw_menu(buf, game, pal, w, y);
    }
}

/// Rows the part under the marquee needs.
fn body_rows(game: &Game) -> i32 {
    if game.help_open {
        HELP.len() as i32
    } else if game.options_open {
        OPTION_ITEMS.len() as i32 + 1
    } else if game.attract_table() {
        7
    } else {
        // Menu, a blank, the theme line (or theme choices) and a note.
        game.menu_items().len() as i32 + 3
    }
}

fn blank_row(buf: &mut Buffer, w: u16, y: i32, text: &str) {
    let len = text.chars().count() as i32 + 4;
    clear_span(buf, (w as i32 - len) / 2, y, len);
}

/// The TYPOIDS block letters with a shadow and a band of light sweeping
/// across them, like a lit cabinet marquee.
fn draw_marquee(buf: &mut Buffer, game: &Game, pal: &Palette, w: u16, top: i32) {
    let lines = title_lines(w >= 80);
    let art_w = lines[0].chars().count() as i32;
    let left = (w as i32 - art_w) / 2;
    let sweep = ((game.time * 0.35).rem_euclid(1.6) - 0.3) * art_w as f32;
    for (i, line) in lines.iter().enumerate() {
        put(buf, left + 1, top + i as i32 + 1, line, style(pal.shadow));
    }
    for (i, line) in lines.iter().enumerate() {
        for (x, c) in line.chars().enumerate() {
            if c == ' ' {
                continue;
            }
            // The band leans, so it reads as a glint, not a column.
            let d = (x as f32 - sweep + i as f32 * 1.5).abs();
            let band = (1.0 - d / 7.0).max(0.0);
            let col = mix(pal.ink, pal.hot, band);
            put_char(
                buf,
                left + x as i32,
                top + i as i32,
                c,
                style(col).add_modifier(Modifier::BOLD),
            );
        }
    }
}

fn draw_menu(buf: &mut Buffer, game: &Game, pal: &Palette, w: u16, top: i32) {
    let items = game.menu_items();
    let heat = game.settings.heat_unlocked > 0;
    let labels: Vec<String> = (0..items.len())
        .map(|i| match items[i] {
            "START" | "NEW RUN" if heat => {
                format!("{}  ‹ HEAT {} ›", items[i], game.settings.heat)
            }
            item => item.to_string(),
        })
        .collect();
    let width = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i32 + 6;
    let left = (w as i32 - width) / 2;
    for (i, label) in labels.iter().enumerate() {
        let y = top + i as i32;
        let on = i == game.menu && !game.theme_menu_open;
        let st = if on { selected(pal) } else { style(pal.dim) };
        clear_span(buf, left, y, width);
        if on {
            put(buf, left, y, &" ".repeat(width as usize), st);
            put(buf, left + 1, y, "▸", st);
            put(buf, left + width - 2, y, "◂", st);
        }
        put_centered(buf, left, width, y, label, st);
        mark((left, y, width, 1), Hit::Menu(i));
    }
    let y = top + items.len() as i32 + 1;
    if game.theme_menu_open {
        draw_theme_choices(buf, game, pal, w, y);
        return;
    }
    let theme_line = format!("THEME  {}", game.theme.label());
    blank_row(buf, w, y, &theme_line);
    put_centered(buf, 0, w as i32, y, &theme_line, style(pal.faint));
    let note = menu_note(game);
    if !note.is_empty() {
        blank_row(buf, w, y + 1, &note);
        put_centered(buf, 0, w as i32, y + 1, &note, style(pal.amber));
    }
}

/// A line about the highlighted entry: today's daily run, or what the
/// chosen heat adds.
fn menu_note(game: &Game) -> String {
    match game.title_action() {
        TitleAction::Daily => {
            let theme = Theme::from_index(game.day as usize % Theme::ALL.len());
            let best = game
                .scores
                .daily_best(game.day)
                .map_or("no score yet".to_string(), |s| format!("best {s:06}"));
            format!(
                "DAILY {}  ·  {}  ·  {best}",
                scores::date_label(game.day),
                theme.label()
            )
        }
        TitleAction::Start if game.settings.heat > 0 => {
            let rule = crate::game::HEAT_RULES[game.settings.heat as usize - 1];
            format!("heat {}: + {rule}", game.settings.heat)
        }
        TitleAction::Start if game.settings.heat_unlocked > 0 => "h / l picks the heat".to_string(),
        _ => String::new(),
    }
}

fn draw_theme_choices(buf: &mut Buffer, game: &Game, pal: &Palette, w: u16, y: i32) {
    let choices: Vec<String> = Theme::ALL
        .iter()
        .map(|t| format!("[ {} ]", t.label()))
        .collect();
    let gap = 3;
    let total: i32 = choices
        .iter()
        .map(|c| c.chars().count() as i32)
        .sum::<i32>()
        + gap * (choices.len() as i32 - 1);
    let mut x = (w as i32 - total) / 2;
    clear_span(buf, x - 2, y, total + 4);
    for (i, label) in choices.iter().enumerate() {
        let st = if i == game.theme_choice {
            style(Color::Rgb(20, 16, 8))
                .bg(pal.ink)
                .add_modifier(Modifier::BOLD)
        } else {
            style(pal.dim)
        };
        let len = label.chars().count() as i32;
        put(buf, x, y, label, st);
        mark((x, y, len, 1), Hit::Theme(i));
        x += len + gap;
    }
}

fn draw_hiscores(buf: &mut Buffer, game: &Game, pal: &Palette, w: u16, top: i32) {
    let width = 34;
    let left = (w as i32 - width) / 2;
    clear_span(buf, left, top, width);
    put_centered(
        buf,
        left,
        width,
        top,
        "HI-SCORES",
        style(pal.amber).add_modifier(Modifier::BOLD),
    );
    let ranks = ["1ST", "2ND", "3RD", "4TH", "5TH"];
    for (i, rank) in ranks.iter().enumerate() {
        let y = top + 2 + i as i32;
        clear_span(buf, left, y, width);
        let line = match game.scores.entries.get(i) {
            Some(e) => format!(
                "{rank}   {}   {:06}   W{:02} {}",
                e.initials,
                e.score,
                e.wave,
                if e.won { "★" } else { " " }
            ),
            None => format!("{rank}   ---   ------   ---  "),
        };
        let col = if i == 0 { pal.hot } else { pal.dim };
        put_centered(buf, left, width, y, &line, style(col));
    }
}

fn draw_options(buf: &mut Buffer, game: &Game, pal: &Palette, w: u16, top: i32) {
    let width = 34;
    let left = (w as i32 - width) / 2;
    clear_span(buf, left, top, width);
    put_centered(
        buf,
        left,
        width,
        top,
        "OPTIONS",
        style(pal.amber).add_modifier(Modifier::BOLD),
    );
    for (i, item) in OPTION_ITEMS.iter().enumerate() {
        let y = top + 1 + i as i32;
        let on = i == game.options_sel;
        let st = if on { selected(pal) } else { style(pal.dim) };
        clear_span(buf, left, y, width);
        let line = format!(
            "{} {:<16}{:>6}",
            if on { "▸" } else { " " },
            item,
            game.option_value(i)
        );
        put(buf, left + 2, y, &format!("{line:<30}"), st);
        mark((left, y, width, 1), Hit::Option(i));
    }
}

fn draw_help(buf: &mut Buffer, pal: &Palette, w: u16, top: i32) {
    let width = HELP.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i32 + 4;
    let left = (w as i32 - width).max(0) / 2;
    for (i, line) in HELP.iter().enumerate() {
        let y = top + i as i32;
        clear_span(buf, left, y, width);
        let last = i + 1 == HELP.len();
        let col = if last {
            pal.amber
        } else if i == HELP.len() - 2 {
            pal.faint
        } else {
            pal.ink
        };
        put_centered(buf, left, width, y, line, style(col));
    }
}

/// Call-outs that warn rather than cheer get a hot-to-red gradient.
fn alarming(text: &str) -> bool {
    matches!(text, "GAME OVER" | "WARNING" | "TIME UP")
}

/// Rows below the call-out's top where a small banner fits under it.
pub const BANNER_GAP: i32 = TEXT_ROWS as i32 + 1;

/// Top row of the call-out band, centered in the play area.
pub fn callout_top(m: Metrics) -> i32 {
    m.origin_y as i32 + (m.play_h as i32 / 2 - 3).max(0)
}

/// The active call-out in big pixel letters. Letters drop in one by one,
/// glow hot at the top and warm toward the bottom, and crumble away at the
/// end. Returns true if one was drawn.
pub fn draw_callout(buf: &mut Buffer, game: &Game, pal: &Palette, m: Metrics) -> bool {
    if !matches!(
        game.phase,
        Phase::Playing | Phase::Paused | Phase::Dying | Phase::Victory
    ) {
        return false;
    }
    let Some(c) = game.active_callout() else {
        return false;
    };
    let age = game.time - c.t0;
    let left = c.dur - age;
    let still = game.settings.reduced_motion;
    let warm = if alarming(&c.text) {
        pal.danger
    } else {
        pal.amber
    };
    let top = callout_top(m);
    if top < 0 || top as u16 + TEXT_ROWS > m.origin_y + m.play_h {
        return false;
    }
    draw_text(
        buf,
        &c.text,
        (0, top as u16, m.play_w.saturating_sub(2)),
        |i, px, py| {
            let n = noise(i, px, py);
            if left < 0.3 && n > left / 0.3 {
                return None;
            }
            let t = ((age - i as f32 * 0.035) / 0.18).clamp(0.0, 1.0);
            if t <= 0.0 && !still {
                return None;
            }
            let drop = if still {
                0
            } else {
                -(((1.0 - t) * (1.0 - t)) * 5.0).round() as i32
            };
            Some((mix(pal.hot, warm, py as f32 / 6.0), drop))
        },
    )
}

/// The combo announcer: a spaced-out shout across the top of the field.
pub fn draw_announce(buf: &mut Buffer, game: &Game, pal: &Palette, m: Metrics) {
    if game.phase != Phase::Playing {
        return;
    }
    let Some(a) = game.announce.as_ref() else {
        return;
    };
    let k = ((game.time - a.t0) / a.dur).clamp(0.0, 1.0);
    let spaced: String = a
        .text
        .chars()
        .flat_map(|c| [c, ' '])
        .collect::<String>()
        .trim_end()
        .to_string();
    let col = if k < 0.5 {
        mix(pal.hot, pal.accent, k * 2.0)
    } else {
        fade(pal.accent, 1.6 - k)
    };
    put_centered(
        buf,
        0,
        m.play_w as i32,
        m.origin_y as i32 + 1,
        &spaced,
        style(col).add_modifier(Modifier::BOLD),
    );
}

/// NEW HIGH SCORE: three letters in big pixels, the next slot blinking.
pub fn draw_initials(buf: &mut Buffer, game: &Game, pal: &Palette, m: Metrics) {
    if game.phase != Phase::Initials {
        return;
    }
    let w = m.play_w as i32;
    let top = m.origin_y as i32 + (m.play_h as i32 - 10).max(0) / 2;
    let fill = Style::default().bg(mix(pal.bg_top, pal.bg_bottom, 0.22));
    for y in top - 1..top + 10 {
        put(buf, 0, y, &" ".repeat(w as usize), fill);
    }
    put_centered(
        buf,
        0,
        w,
        top,
        "NEW HIGH SCORE",
        fill.fg(pal.amber).add_modifier(Modifier::BOLD),
    );
    put_centered(
        buf,
        0,
        w,
        top + 1,
        &format!("{:06}  ·  WAVE {}", game.score, game.wave),
        fill.fg(pal.ink),
    );
    let typed = game.initials.chars().count();
    let shown: String = (0..3)
        .map(|i| game.initials.chars().nth(i).unwrap_or('-'))
        .collect();
    let blink = ((game.time * 3.0) as i32).rem_euclid(2) == 0;
    draw_text(buf, &shown, (0, (top + 3) as u16, m.play_w), |i, _, _| {
        let col = match i.cmp(&typed) {
            std::cmp::Ordering::Less => pal.hot,
            std::cmp::Ordering::Equal if blink => pal.amber,
            _ => pal.faint,
        };
        Some((col, 0))
    });
    put_centered(
        buf,
        0,
        w,
        top + 8,
        "type your initials  ·  enter to confirm",
        fill.fg(pal.dim),
    );
}
