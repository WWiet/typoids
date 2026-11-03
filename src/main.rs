mod boss;
mod draw;
mod game;
mod save;
mod upgrades;
mod words;

use std::io::Result;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use game::{
    Game, Phase, TitleAction, OVER_AGAIN, OVER_QUIT, OVER_THEMES, PAUSE_CONTINUE, PAUSE_QUIT,
    PAUSE_RESTART,
};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};

/// How long the window size must hold still before the final repaint.
const SETTLE: Duration = Duration::from_millis(250);
/// Frame budget: 120 fps. A frame costs well under a millisecond to build, and
/// braille motion looks noticeably smoother than at 60 on fast displays.
const FRAME: Duration = Duration::from_micros(8_333);

fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
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
    let mut last = Instant::now();
    let mut next_frame = last;
    let mut seen = size;
    // When a resize has settled, repaint everything once more.
    let mut repaint_at: Option<Instant> = None;

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
        present(terminal, &game)?;

        // Wait for the next frame, handling input as it arrives. A keypress
        // ends the wait early so typed letters show up at once.
        next_frame = (next_frame + FRAME).max(Instant::now());
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
                Event::Resize(..) => repaint_at = Some(Instant::now() + SETTLE),
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

fn on_key(game: &mut Game, code: KeyCode, mods: KeyModifiers) {
    if mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'C')) {
        game.quit = true;
        return;
    }
    match game.phase {
        Phase::Title if game.theme_menu_open => theme_keys(game, code),
        Phase::Title => match code {
            KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => {
                game.menu_move(1);
            }
            KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => {
                game.menu_move(-1);
            }
            KeyCode::Enter => match game.title_action() {
                TitleAction::Continue => {
                    game.continue_saved();
                }
                TitleAction::Start => game.start(),
                TitleAction::Themes => game.open_themes(),
                TitleAction::Quit => game.quit = true,
            },
            KeyCode::Char('q' | 'Q') => game.quit = true,
            _ => {}
        },
        Phase::Playing => match code {
            KeyCode::Esc => game.pause(),
            KeyCode::Backspace => game.backspace(),
            KeyCode::Char(c) if c.is_ascii_alphabetic() => game.type_char(c),
            _ => {}
        },
        Phase::Paused | Phase::GameOver if game.theme_menu_open => theme_keys(game, code),
        Phase::Paused => match code {
            KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => {
                game.menu_move(1);
            }
            KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => {
                game.menu_move(-1);
            }
            KeyCode::Esc => game.resume(),
            KeyCode::Enter => match game.menu {
                PAUSE_CONTINUE => game.resume(),
                PAUSE_RESTART => game.start(),
                PAUSE_QUIT => game.save_and_quit_to_title(),
                _ => {}
            },
            KeyCode::Char('q' | 'Q') => game.save_and_quit_to_title(),
            _ => {}
        },
        Phase::GameOver => match code {
            KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => {
                game.menu_move(1);
            }
            KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => {
                game.menu_move(-1);
            }
            KeyCode::Enter => match game.menu {
                OVER_AGAIN => game.start(),
                OVER_THEMES => game.open_themes(),
                OVER_QUIT => game.quit = true,
                _ => {}
            },
            KeyCode::Char('q' | 'Q') => game.quit = true,
            _ => {}
        },
        Phase::Shop => match code {
            KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => {
                game.shop_move(1);
            }
            KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => {
                game.shop_move(-1);
            }
            KeyCode::Char(c @ '1'..='4') => game.shop_buy(c as usize - '1' as usize),
            KeyCode::Char('r' | 'R') => game.shop_reroll(),
            KeyCode::Char('n' | 'N' | ' ') => game.leave_shop(),
            KeyCode::Enter => game.shop_activate(),
            KeyCode::Char('q' | 'Q') | KeyCode::Esc => game.save_and_quit_to_title(),
            _ => {}
        },
        Phase::Dying => {}
    }
}

fn theme_keys(game: &mut Game, code: KeyCode) {
    match code {
        KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => {
            game.theme_move(1);
        }
        KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => {
            game.theme_move(-1);
        }
        KeyCode::Enter => game.apply_theme(),
        KeyCode::Esc => game.close_themes(),
        KeyCode::Char('q' | 'Q') => game.quit = true,
        _ => {}
    }
}
