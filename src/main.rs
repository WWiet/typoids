mod audio;
mod boss;
mod draw;
mod game;
mod hazards;
mod save;
mod scores;
mod settings;
mod upgrades;
mod words;

use std::io::Result;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use game::{
    Game, Hit, Phase, TitleAction, OPTION_ITEMS, OVER_AGAIN, OVER_QUIT, OVER_THEMES,
    PAUSE_CONTINUE, PAUSE_QUIT, PAUSE_RESTART,
};
use ratatui::crossterm::event::{
    self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture, Event,
    KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};

/// How long the window size must hold still before the final repaint.
const SETTLE: Duration = Duration::from_millis(250);
/// Frame budget while the terminal is in the background: nothing to watch.
const UNFOCUSED: Duration = Duration::from_millis(100);

fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    // Focus events let the game pause itself when you switch away; mouse
    // capture lets menus be pointed at and clicked.
    let _ = execute!(
        terminal.backend_mut(),
        EnableFocusChange,
        EnableMouseCapture
    );
    let result = run(&mut terminal);
    let _ = execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        DisableFocusChange
    );
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let size = terminal.size()?;
    let mut game = Game::new(size.width, size.height, seed);
    game.enable_persistence();
    let day = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    game.set_day(day as u32);
    let audio = audio::Audio::start();
    let mut last = Instant::now();
    let mut next_frame = last;
    let mut seen = size;
    // When a resize has settled, repaint everything once more.
    let mut repaint_at: Option<Instant> = None;
    let mut focused = true;

    loop {
        let size = terminal.size()?;
        if size != seen {
            // Resizing (fullscreen in particular) arrives as several quick
            // steps, and the terminal can wipe cells ratatui believes are
            // still painted. ratatui only sends changed cells, so force a full
            // repaint now and again once the size has stopped moving.
            seen = size;
            terminal.clear()?;
            repaint_at = Some(Instant::now() + SETTLE);
        }
        if repaint_at.is_some_and(|at| Instant::now() >= at) {
            terminal.clear()?;
            repaint_at = None;
        }
        game.set_screen(size.width, size.height);

        // Simulate up to now, then show it, so a frame never lags the input.
        let now = Instant::now();
        let dt = (now - last).as_secs_f32();
        last = now;
        if game.phase != Phase::Paused {
            game.update(dt);
        } else {
            game.update(0.0);
            // Keep the cursor blinking on the pause screen without moving rocks.
            game.time += dt.min(0.05);
        }
        let volume = game.settings.sound.factor();
        for (sfx, delay) in game.sounds.drain(..) {
            audio.play(sfx, delay, volume);
        }
        present(terminal, &game)?;

        // Wait for the next frame, handling input as it arrives. A keypress
        // ends the wait early so typed letters show up at once.
        // 120 fps in play, 30 on still screens, 10 in the background.
        let budget = if focused {
            game.frame_budget()
        } else {
            UNFOCUSED
        };
        next_frame = (next_frame + budget).max(Instant::now());
        while event::poll(next_frame.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('l' | 'L'))
                    {
                        // Ctrl+L: manual full repaint.
                        terminal.clear()?;
                        continue;
                    }
                    on_key(&mut game, key.code, key.modifiers);
                    // Take any keys already queued, then draw straight away.
                    while event::poll(Duration::ZERO)? {
                        match event::read()? {
                            Event::Key(key) if key.kind == KeyEventKind::Press => {
                                on_key(&mut game, key.code, key.modifiers)
                            }
                            Event::Resize(..) => repaint_at = Some(Instant::now() + SETTLE),
                            _ => {}
                        }
                    }
                    break;
                }
                Event::Mouse(mouse) => {
                    if on_mouse(&mut game, mouse) {
                        break;
                    }
                }
                Event::Resize(..) => repaint_at = Some(Instant::now() + SETTLE),
                Event::FocusLost => {
                    focused = false;
                    game.pause();
                }
                Event::FocusGained => {
                    focused = true;
                    // Some terminals drop cells while hidden.
                    repaint_at = Some(Instant::now());
                }
                _ => {}
            }
        }

        if game.quit {
            game.save_best();
            break;
        }
    }
    Ok(())
}

/// Draw one frame inside a synchronized update, so the terminal shows it whole
/// instead of tearing halfway through. Terminals without support ignore it.
fn present(terminal: &mut ratatui::DefaultTerminal, game: &Game) -> Result<()> {
    queue!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
    terminal.draw(|frame| draw::draw(frame, game))?;
    execute!(terminal.backend_mut(), EndSynchronizedUpdate)
}

/// Pointing selects what is under the mouse; a left click also presses Enter
/// on it (or Tab, for the upgrades screen button). Rows of the upgrades
/// screen only select, so a stray click never sells. Returns true when the
/// screen changed and should be drawn straight away.
fn on_mouse(game: &mut Game, mouse: MouseEvent) -> bool {
    let hit = draw::hit_at(mouse.column, mouse.row);
    let none = KeyModifiers::NONE;
    match mouse.kind {
        MouseEventKind::Moved | MouseEventKind::Drag(_) => {
            game.note_input();
            hit.is_some_and(|hit| game.hover(hit))
        }
        MouseEventKind::Down(MouseButton::Left) => {
            match hit {
                Some(Hit::Loadout) if game.hover(Hit::Loadout) => on_key(game, KeyCode::Tab, none),
                Some(Hit::Rack(i)) => {
                    game.note_input();
                    game.hover(Hit::Rack(i));
                }
                Some(hit) if game.hover(hit) => on_key(game, KeyCode::Enter, none),
                // A click anywhere closes the how-to-play card.
                None if game.help_open => on_key(game, KeyCode::Enter, none),
                _ => game.note_input(),
            }
            true
        }
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp if game.rack_focus => {
            let d = if mouse.kind == MouseEventKind::ScrollDown {
                1
            } else {
                -1
            };
            game.rack_move(d);
            true
        }
        _ => false,
    }
}

/// h/j/k/l and the arrows: +1 for down/right, -1 for up/left.
fn nav_delta(code: KeyCode) -> Option<isize> {
    match code {
        KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => Some(1),
        KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => Some(-1),
        _ => None,
    }
}

/// Left/right only (the vertical title menu uses h/l for the heat).
fn side_delta(code: KeyCode) -> Option<isize> {
    match code {
        KeyCode::Char('l' | 'L') | KeyCode::Right => Some(1),
        KeyCode::Char('h' | 'H') | KeyCode::Left => Some(-1),
        _ => None,
    }
}

fn on_key(game: &mut Game, code: KeyCode, mods: KeyModifiers) {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(code, KeyCode::Char('c' | 'C')) {
        game.quit = true;
        return;
    }
    game.note_input();
    // Screens that just appeared ignore keys for a moment, so letters still
    // being typed at the end of a wave cannot buy, skip or restart.
    let guarded = matches!(
        game.phase,
        Phase::Shop | Phase::GameOver | Phase::Initials | Phase::Victory
    );
    if guarded && !game.accepts_menu_keys() {
        return;
    }
    // Menus click when a choice is made; the shop has its own sounds.
    let menu = !matches!(game.phase, Phase::Playing | Phase::Shop | Phase::Dying);
    if code == KeyCode::Enter && menu {
        game.sfx(audio::Sfx::Select);
    }
    match game.phase {
        Phase::Title => title_keys(game, code),
        Phase::Playing => match code {
            KeyCode::Esc => game.pause(),
            KeyCode::Tab => {
                game.activate_overdrive();
            }
            KeyCode::Char('w' | 'W' | 'h' | 'H') | KeyCode::Backspace if ctrl => game.clear_word(),
            KeyCode::Backspace => game.backspace(),
            KeyCode::Char(' ') if !ctrl => game.type_space(),
            KeyCode::Char(c) if c.is_ascii_alphabetic() && !ctrl => game.type_char(c),
            _ => {}
        },
        Phase::Paused | Phase::GameOver if game.theme_menu_open => theme_keys(game, code),
        Phase::Paused => match code {
            KeyCode::Esc => game.resume(),
            KeyCode::Enter => match game.menu {
                PAUSE_CONTINUE => game.resume(),
                PAUSE_RESTART => game.start(),
                PAUSE_QUIT => game.save_and_quit_to_title(),
                _ => {}
            },
            KeyCode::Char('q' | 'Q') => game.save_and_quit_to_title(),
            _ => {
                if let Some(d) = nav_delta(code) {
                    game.menu_move(d);
                }
            }
        },
        Phase::GameOver => match code {
            KeyCode::Enter => match game.menu {
                OVER_AGAIN => game.start(),
                OVER_THEMES => game.open_themes(),
                OVER_QUIT => game.quit = true,
                _ => {}
            },
            KeyCode::Char('q' | 'Q') => game.quit = true,
            _ => {
                if let Some(d) = nav_delta(code) {
                    game.menu_move(d);
                }
            }
        },
        Phase::Victory => match code {
            KeyCode::Enter => game.victory_choose(),
            _ => {
                if let Some(d) = nav_delta(code) {
                    game.menu_move(d);
                }
            }
        },
        Phase::Initials => match code {
            KeyCode::Enter => game.confirm_initials(),
            KeyCode::Backspace => game.backspace(),
            KeyCode::Char(c) if c.is_ascii_alphabetic() => game.type_char(c),
            _ => {}
        },
        Phase::Shop if game.rack_focus => match code {
            KeyCode::Tab | KeyCode::Esc => game.shop_toggle_rack(),
            KeyCode::Enter | KeyCode::Char('x' | 'X') => game.shop_sell(),
            _ => {
                if let Some(d) = nav_delta(code) {
                    game.rack_move(d);
                }
            }
        },
        Phase::Shop => match code {
            KeyCode::Char(c @ '1'..='4') => game.shop_buy(c as usize - '1' as usize),
            KeyCode::Char('r' | 'R') => game.shop_reroll(),
            KeyCode::Char('n' | 'N' | ' ') => game.leave_shop(),
            KeyCode::Tab => game.shop_toggle_rack(),
            KeyCode::Enter => game.shop_activate(),
            KeyCode::Esc => game.pause(),
            KeyCode::Char('q' | 'Q') => game.save_and_quit_to_title(),
            _ => {
                if let Some(d) = nav_delta(code) {
                    game.shop_move(d);
                }
            }
        },
        Phase::Dying => {}
    }
}

fn title_keys(game: &mut Game, code: KeyCode) {
    if game.theme_menu_open {
        return theme_keys(game, code);
    }
    if game.help_open {
        if !matches!(code, KeyCode::Char('q' | 'Q')) {
            game.close_help();
        } else {
            game.quit = true;
        }
        return;
    }
    if game.options_open {
        match code {
            KeyCode::Esc => game.close_options(),
            KeyCode::Enter => game.options_activate(),
            KeyCode::Char('j' | 'J') | KeyCode::Down => game.options_move(1),
            KeyCode::Char('k' | 'K') | KeyCode::Up => game.options_move(-1),
            _ if side_delta(code).is_some() && game.options_sel + 1 < OPTION_ITEMS.len() => {
                game.options_activate()
            }
            _ => {}
        }
        return;
    }
    match code {
        KeyCode::Enter => match game.title_action() {
            TitleAction::Continue => {
                game.continue_saved();
            }
            TitleAction::Start => game.start(),
            TitleAction::Daily => game.start_daily(),
            TitleAction::Themes => game.open_themes(),
            TitleAction::Options => game.open_options(),
            TitleAction::Help => game.open_help(),
            TitleAction::Quit => game.quit = true,
        },
        KeyCode::Char('q' | 'Q') => game.quit = true,
        KeyCode::Char('j' | 'J') | KeyCode::Down => game.menu_move(1),
        KeyCode::Char('k' | 'K') | KeyCode::Up => game.menu_move(-1),
        _ => {
            if let Some(d) = side_delta(code) {
                if game.title_action() == TitleAction::Start && game.settings.heat_unlocked > 0 {
                    game.heat_move(d);
                } else {
                    game.menu_move(d);
                }
            }
        }
    }
}

fn theme_keys(game: &mut Game, code: KeyCode) {
    match code {
        KeyCode::Enter => game.apply_theme(),
        KeyCode::Esc => game.close_themes(),
        KeyCode::Char('q' | 'Q') => game.quit = true,
        _ => {
            if let Some(d) = nav_delta(code) {
                game.theme_move(d);
            }
        }
    }
}
