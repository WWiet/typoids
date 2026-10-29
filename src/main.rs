mod draw;
mod game;
mod words;

use std::io::Result;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use game::{Game, Phase, MENU_QUIT, MENU_START, MENU_THEMES};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

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
    let mut last = Instant::now();

    loop {
        let size = terminal.size()?;
        game.set_screen(size.width, size.height);
        terminal.draw(|frame| draw::draw(frame, &game))?;

        let timeout = Duration::from_millis(16);
        if event::poll(timeout)? {
            while event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        on_key(&mut game, key.code, key.modifiers)
                    }
                    _ => {}
                }
            }
        }

        if game.quit {
            game.save_best();
            break;
        }

        let dt = last.elapsed().as_secs_f32();
        last = Instant::now();
        if game.phase != Phase::Paused {
            game.update(dt);
        } else {
            game.update(0.0);
            // Keep the cursor blinking on the pause screen without moving rocks.
            // update(0) still advances nothing meaningful; bump time for the blink.
            game.time += dt.min(0.05);
        }
    }
    Ok(())
}

fn on_key(game: &mut Game, code: KeyCode, mods: KeyModifiers) {
    if mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'C')) {
        game.quit = true;
        return;
    }
    match game.phase {
        Phase::Title if game.theme_menu_open => match code {
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
        },
        Phase::Title => match code {
            KeyCode::Char('j' | 'J' | 'l' | 'L') | KeyCode::Down | KeyCode::Right => {
                game.menu_move(1);
            }
            KeyCode::Char('k' | 'K' | 'h' | 'H') | KeyCode::Up | KeyCode::Left => {
                game.menu_move(-1);
            }
            KeyCode::Enter => match game.menu {
                MENU_START => game.start(),
                MENU_THEMES => game.open_themes(),
                MENU_QUIT => game.quit = true,
                _ => {}
            },
            KeyCode::Char('q' | 'Q') => game.quit = true,
            _ => {}
        },
        Phase::Playing => match code {
            KeyCode::Esc => game.phase = Phase::Paused,
            KeyCode::Backspace => game.backspace(),
            KeyCode::Char(c) if c.is_ascii_alphabetic() => game.type_char(c),
            _ => {}
        },
        Phase::Paused => match code {
            KeyCode::Esc | KeyCode::Enter => game.phase = Phase::Playing,
            KeyCode::Char('q' | 'Q') => game.quit = true,
            _ => {}
        },
        Phase::GameOver => match code {
            KeyCode::Enter => game.start(),
            KeyCode::Char('q' | 'Q') => game.quit = true,
            _ => {}
        },
        Phase::Dying => {}
    }
}
