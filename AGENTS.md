# AGENTS.md

Guidance for AI agents working on TYPOIDS. Read this before changing the code.

## Project

TYPOIDS is a ratatui terminal game: Asteroids with words printed on the hazards.
The player types the word attached to the nearest hazard; the ship turns and
fires, and finishing the word destroys the hazard.

It is a single binary crate. There are no assets, no network access, and no
browser build.

- Rust 1.75+
- `ratatui` 0.29 (crossterm backend)
- UTF-8 terminal. Braille glyphs (`U+2800..U+28FF`) draw ships, hazards, and
  scenery. Text is drawn in plain cells so it stays readable.

## Commands

Run these before finishing any change:

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

Run the game with:

```bash
cargo run --release
```

There is no separate lint config and no CI; `-D warnings` is the standard.

## File map

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Terminal setup, event loop, key handling per phase. |
| `src/game.rs` | Simulation: state machine, hazards, typing buffer, scoring, effects, RNG, theme list, tests. |
| `src/draw.rs` | All rendering: palettes, background, braille ink layer, ships, hazards, effects, HUD, menus, render tests. |
| `src/words.rs` | Lowercase ASCII word pools (`SHORT`, `MID`, `LONG`). |
| `README.md` | Player-facing documentation. Update it when controls or themes change. |

`mod` declarations live in `main.rs`; the crate has no library target. Modules
are siblings and may use `crate::game::...`, `crate::draw::...`, etc.

## Simulation rules

- `Game::new(term_w, term_h, seed)` seeds everything. Tests must pass a fixed
  seed; never read wall-clock time inside `game.rs` except through the seed
  supplied by `main.rs`.
- `Game::update(dt)` advances the current phase. `main.rs` calls
  `update(0.0)` while paused and bumps `game.time` separately for blinking.
- `Game::set_screen` recomputes the play area and reseeds background particles
  when the terminal size changes. It is safe to call every frame.
- Phases are `Title`, `Playing`, `Paused`, `Dying`, `GameOver`.
- Pause menu **Quit** (`PAUSE_QUIT`) and `Q` during pause call
  `Game::return_to_title()`. Title and game-over Quit still set `game.quit`.
- Menu indices are shared between the title, pause, and game-over popups, but
  each popup has its own item list (`MENU_ITEMS`, `PAUSE_ITEMS`, `OVER_ITEMS`)
  and constants (`MENU_START`, `PAUSE_CONTINUE`, `OVER_AGAIN`, ...). When you
  change a list, update the matching `main.rs` key handling and the tests.
- The applied theme is `game.theme`; the highlighted menu choice is
  `game.theme_choice`. Only `apply_theme()` copies choice into theme.
- Hazards are `Rock` values with a circular word label, live flag, hit flash,
  and a polygonal `shape`. Coconuts are rounder and denser than asteroids.
- Typing uses `game.buffer`. Correct letters extend the buffer; wrong letters
  are ignored; backspace removes one letter. A hazard is locked when its word
  starts with the buffer.
- Word pools must remain lowercase ASCII, unique, and typeable on a normal
  keyboard. `pool()` escalates from short to mid to long words by wave.
- RNG is a small deterministic LCG (`Rng`): use `self.rng.range`, `self.rng.f32`,
  and `self.rng.pick`. Do not add `rand`.

## Rendering rules

- `draw::draw(frame, game)` paints the background first, then returns early with
  a resize message below `48x18`; the intended full layout is at least `70x22`.
  Keep the small-terminal path working.
- Each theme has a `Palette` const (`SPACE`, `UNDERWATER`, `JUNGLE`). Reuse
  palette fields instead of hardcoding colors in new UI.
- Braille art goes through `InkLayer`:
  - World units are terminal cells: 1 unit = 2 braille pixels in x and 4 in y.
  - Gameplay and ships convert with `to_px`, which includes screen shake.
  - Background scenery converts with `bg_to_px`, which deliberately excludes
    shake so the canopy stays still.
  - `ink.dot` and `ink.line` take a priority. Higher priority wins the cell
    color. Background scenery is priority `0`, hazards `1..2`, ships and
    projectiles `4..5`. Keep the background below gameplay.
- Text (words, HUD, menus) is drawn with `put`/`write` in ordinary cells using
  `style(color)` so it stays legible over the braille art.
- `draw()` is exhaustive over `Theme` and `Phase`. Adding a theme or phase means
  visiting every `match` in `game.rs` and `draw.rs`.
- Backgrounds:
  - Space and Underwater use `paint_water` (vertical gradient).
  - Jungle uses `paint_jungle_canopy` (gradient plus dappled light), then
    `draw_jungle_scenery` (fronds, vines, undergrowth) and
    `draw_jungle_particles` (falling leaves and fireflies).
- `Star.glyph` doubles as a particle species marker. For Jungle: `'✦'` is a
  firefly, `'❧'` a large leaf, `','` a small leaf; `Star.phase` makes motion
  deterministic. Other themes use stars/bubbles in plain cells.
- Keep Jungle background density low (`reseed_stars` uses a larger divisor for
  Jungle). It is scenery, not the main event.

### Adding or changing a theme

1. Add the variant to `Theme`, `Theme::ALL`, `label`, `index`, and `from_index`.
2. Add a `Palette` const and the `palette()` arm.
3. Add theme text arms in `game.rs` (banners, hints, scoring) and `draw.rs`
   (title subtitle, game-over text, prompts).
4. Pick the ship renderer in the `match game.theme` block in `draw()`. Local
   ship geometry belongs in `game.rs` as a `*_local()` function.
5. Give hazards a theme-specific silhouette in the rock loop if needed.
6. Give projectiles a theme-specific `Effect::Laser` branch.
7. Seed background particles in `reseed_stars`.
8. Update the theme-selection test in `game.rs` and the render test in `draw.rs`.
9. Update `README.md`.

## Style

- Match surrounding code; keep the existing module layout and helper style.
- Keep functions small enough to satisfy clippy. `ink_leaf` already groups its
  position and size to avoid `too_many_arguments`.
- Use `f32` math for simulation, `u16`/`i32` for terminal coordinates.
- Prefer `fade`, `mix`, and palette colors over literals in draw code.
- Avoid new dependencies unless they are clearly necessary. ratatui already
  covers terminal I/O and layout.

## Tests

Tests are colocated in `#[cfg(test)] mod tests` inside `game.rs` and `draw.rs`.

- Game tests use `Game::new(w, h, seed)` (seeded) and helpers such as
  `playing()`. Do not depend on timing or filesystem state except through
  `Game::new`'s best-score load.
- Draw tests render with `ratatui::backend::TestBackend` via the `render()`
  helper and assert on the resulting text (e.g. menu labels and hint copy).
  Add a render assertion when you add player-visible strings or theme visuals.
- Every behavior change should add or update a focused test. Keep the suite
  green; currently 18 tests.

## Gotchas

- `reseed_stars` is called on resize and theme changes; particle state is not
  preserved across those events.
- Screen shake must not move the background; only gameplay conversion includes
  it.
- `Game::start()` resets run state but keeps the selected theme and best score.
- Best score persists at `~/.typoids-best` (`load_best`/`save_best`).
- Hazards that reach the ship cost a life; during invulnerability hazards pass
  through rather than being destroyed for free.
- Don't put gameplay elements at braille priority `0`; they would be
  overpainted by background scenery.
