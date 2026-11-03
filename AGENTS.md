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
| `src/main.rs` | Terminal setup, frame-paced event loop, key handling per phase. |
| `src/game.rs` | Simulation: state machine, hazards, typing buffer, scoring, effects, RNG, theme list, tests. |
| `src/draw.rs` | All rendering: palettes, background, braille ink layer, ships, hazards, effects, HUD, menus, render tests. |
| `src/boss.rs` | Boss-wave rules: cadence, WPM targets, passage and clock (`Boss`). Data and math only. |
| `src/draw/bossfight.rs` | The boss art, its HP/clock/pace bar, and the passage panel. |
| `src/draw/ships.rs` | The four player craft (`Craft` helper plus one function per theme). |
| `src/draw/stage.rs` | The typing stage: big pixel letters along the bottom, with slide, assemble and shatter animations. |
| `src/draw/bigfont.rs` | 5x7 capital-letter pixel font for the stage. Data only. |
| `src/upgrades.rs` | Roguelike upgrade catalog: `Upgrade`, `Rarity`, `Upgrades` (levels held), costs. Data only. |
| `src/save.rs` | `RunSave` plus pure `encode`/`decode` for the run save file. |
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
- Phases are `Title`, `Playing`, `Paused`, `Shop`, `Dying`, `GameOver`.
- Pause menu **Save & Quit** (`PAUSE_QUIT`) and `Q` during pause or the shop
  call `Game::save_and_quit_to_title()`, which writes the run then calls
  `return_to_title()`. Title and game-over Quit still set `game.quit`.
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

## Roguelike layer

A run is a chain of waves. Kills drop crystals (`Gem`), crystals buy upgrades
in a three-card shop that opens after every cleared wave (`Phase::Shop`).

- **Crystals.** `detonate` calls `drop_gems`. Gems drift for 0.45s, then home
  to the ship in `update_gems`; arrival adds to `game.crystals`. After a wave is
  cleared any stragglers are banked instantly. They are not simulated in `draw`.
- **Shop.** `open_shop` rolls `shop_slots()` distinct lines (three, four with
  Wide Shop) via `roll_offers`, weighted by the rarity of each line's *next
  tier* and skipping maxed or pointless ones via `eligible`. Buying, rerolling
  and leaving go through `shop_buy`, `shop_reroll` and `leave_shop`; the shop
  never touches typing state. A bought card stays sold: the next tier has to
  roll in on a later shop or reroll. Menu indices come from `shop_slots()`,
  `shop_reroll_index()` and `shop_go_index()`, never constants. Shop keys live
  in `main.rs`: `h`/`l`, `1`-`4`, `r`, `Enter`, `space`.
- **Lines and tiers.** `upgrades.rs` holds the catalog. An `Upgrade` is a line
  of cards; every level is its own card (`Def::tiers`: name and rarity), so
  Slow Tide, Deep Current and Still Water are three cards in one line. The
  line's maximum is `tiers.len()`. An `Offer` carries the `tier` it grants.
  The simulation still reads the *level held* with
  `self.upgrades.level(Upgrade::X)` / `.has(...)` at the point the effect
  applies (see `detonate`, `chain_kills`, `integrate_rocks`, `collide`,
  `begin_wave`, `open_shop`, `shop_reroll`). Never store an effect flag that
  duplicates a level.
- **Adding a card.** (1) New line: add the variant to `Upgrade`, `Upgrade::ALL`,
  `COUNT` and a `DEFS` entry in the same order. New tier: add a `t(name, rarity)`
  to the line. Names must be unique and at most 12 characters (four cards share
  80 columns); descriptions stay under about 55 characters. (2) Add the hook in
  `game.rs` and a focused test. (3) If a number scales with level, put it in a
  helper next to `Upgrade::blurb` (`multishot_letters`, `shrapnel_radius`, ...)
  so the card text and the simulation agree. (4) If it changes what the player
  sees, update `draw.rs` and add a render assertion. Save files use the line's
  `key` and level, so never rename a key or reorder tiers.
- **Rarity and cost.** Rare cards should be rule-benders, not bigger numbers.
  Later tiers of a line are usually rarer than earlier ones. A card costs
  `Rarity::base_cost` of its tier plus 2 per level already held; patch kits are
  flat, and Discount lowers every price (never below 1). An upgrade that only
  improves another must gate itself in `eligible`.
- **Saves.** `~/.typoids-run` holds a `RunSave` (wave, score, lives, crystals,
  kills, revive flag, upgrades, and whether to resume in the shop). It is
  written when the shop opens, after each purchase or reroll, and by
  Save & Quit; it is deleted when a run ends or a new one starts. A mid-wave
  save resumes at the start of that wave. **Tests never write it:** only
  `main.rs` calls `enable_persistence()`, so `persist` is off in `Game::new`.
  Test save logic through `snapshot`/`load_run` and `save::encode`/`decode`.

## Boss waves

Every fifth wave (`boss::EVERY`) replaces the hazard field with a timed typing
test. `begin_wave` calls `start_boss`, which sets `Game::boss`; while it is
`Some`, `update_play` runs `update_boss` instead of spawning hazards.

- **Rules.** The boss demands `target_wpm(tier)`: 40, 60, 80, 100, then 120
  forever. The passage is `passage_len(tier)` words (up to 40) and the clock is
  `chars / 5 / wpm * 60` seconds, counting a space after every word as typing
  tests do. A three-second intro (`INTRO`) lets the player read; keys do nothing
  until it ends and the clock starts afterwards.
- **Typing.** `type_char` routes to `boss_type`. The buffer only ever holds the
  correct start of the current word and finishing the word advances at once (no
  space key). `target_word()` returns the current word, so the stage draws it
  big with the normal slide and assemble animations. Backspace is disabled.
- **Damage.** A wrong key calls `boss_miss`: Clean Slate forgives the first
  slip each wave, a Ward absorbs one, otherwise `lose_life(0.7)` costs a heart
  with a short grace period so a burst of slips is one hit. The correct letters
  stay in the buffer. `lose_life` is shared with hazard collisions.
- **Outcomes.** Finishing the passage (`boss_win`) pays `12 + 8 * tier`
  crystals, score by tier and pace, and heals one heart. Running out of time
  (`boss_timeout`) costs `TIMEOUT_HEARTS` (2) hearts and pays nothing; if that
  empties the hearts the normal death sequence runs. Both reuse the
  `pending_clear` path into the shop, which clears `Game::boss`.
- **Layout.** With a boss the ship sits low (`ship_center`), the boss rests at
  `boss_home` and is drawn at `boss_pos` (home plus `boss::drift`, driven by
  `Boss::age`, so lasers and aiming follow it), and the passage panel is laid
  out from `boss_home` so it never jitters.
- **Art.** Space, Jungle and Cat Cafe bosses are a head plus an accessory
  (`draw_head`); Underwater is a side-on great white (`draw_shark`) that swims
  laps, faces its heading (`boss::heading`) and opens its jaw on a strike. A
  strike is one smooth lunge over `boss::STRIKE`, never a per-frame jitter. The status line (name,
  HP, clock that turns red when behind pace, live WPM) is play row 0.
- **Tuning.** Cadence, pace, passage length, pay and the timeout penalty are
  constants and small functions in `boss.rs` and the `boss_*` methods in
  `game.rs`. Add a test when you change one.

## Rendering rules

- `draw::draw(frame, game)` paints the background first, then returns early with
  a resize message below `48x18`; the intended full layout is at least `70x22`.
  Keep the small-terminal path working.
- Each theme has a `Palette` const (`SPACE`, `UNDERWATER`, `JUNGLE`, `CAT_CAFE`). Reuse
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
    `draw_jungle_scenery` (corner fronds only) and
    `draw_jungle_particles` (falling leaves and fireflies).
- `Star.glyph` doubles as a particle species marker. For Jungle: `'✦'` is a
  firefly, `'❧'` a large leaf, `','` a small leaf; `Star.phase` makes motion
  deterministic. Other themes use stars/bubbles in plain cells.
- Keep Jungle background density low (`reseed_stars` uses a larger divisor for
  Jungle). It is scenery, not the main event.

### Visual design rules

The look is "quiet field, loud words": a calm retro braille field where only
the things the player acts on carry color. Hold to this when adding visuals.

- **Layout.** The header is one row (row 0): wave, score, hearts, combo, and a
  right-aligned best score. Below the play area sits the typing stage, then a
  two-row dock with one hint line (and the miss / match-count line above it).
  `metrics()` owns all of it (`origin_y = 1`, `dock_h = 2`, `stage_h` from
  `stage_height`: 0 below 22 rows, otherwise 5). The play area, spawns
  and ship centre shrink to what remains; menus, the shop and the title draw
  over the stage rows too. No rule lines or boxes except the stage's floor
  line; spacing and color separate things. Do not add HUD rows without a strong
  reason.
- **The typing stage.** The locked word is drawn in big pixel letters
  (`draw/stage.rs`): half-blocks (`▀▄`) in four rows above the floor line, two columns
  per pixel when the word fits and one when it does not. Typed letters are `hot`
  and slide up from the floor line, the untyped rest is `dim` and assembles by
  pixel when the target locks, and a finished word dissolves upward
  (`Game::spent`). Animation timing is read from `Game::buffer_t`, `lock_t` and
  `spent`; `draw` never mutates them, and pixel noise is a hash, not the RNG.
  Always clear the typed buffer with `clear_typed()` so the timestamps stay in
  step with `buffer`. Below 22 rows `stage_h` is 0 and the dock falls back to
  the one-line input (`draw_input_line`).
- **Shop cards.** `draw_shop` lays out three framed cards by rarity: single
  line for common, heavy for the selected card, double line and `★` for rare.
  The run's upgrades sit under the buttons as a compact table (`draw_installed`: newest card per line plus level pips, capped so cards keep three description lines). Prices use `pal.gem`, or `pal.danger` when unaffordable. Wrap descriptions
  with `wrap`; never let text spill past a card's inner width.
- **Words are chips.** `draw_label` paints every word on a `pal.chip`
  background with one padding cell on each side. The locked word gets a warmer
  chip and `‹ ›` brackets instead of padding. Do not draw bare words over the
  braille layer.
- **One shape per job, themes only recolor.** Projectiles go through
  `draw_projectile` (dash plus three-dot tail) and `projectile_colors`.
  Space and Underwater explosions are `Game::burst` (one ring, six shards).
  Jungle and Cat Cafe hazards spill instead (`Game::pour`: `Effect::Drop`
  droplets that fall under `DROP_GRAVITY`, plus a lingering `Effect::Puddle`
  colored `pal.liquid`). Exhaust is one braille dot. Anything fast that
  moves (crystals, exhaust, hit dots) goes in the ink layer via `draw_motes`
  so it glides by sub-pixel; whole-cell glyphs only for slow scenery and text.
  Do not reintroduce per-theme spark confetti, letter bursts, lock rings, or
  blinking fuses; if a theme needs character, put it in the ship, the hazard
  silhouette, the palette, or the ring's squash/wobble.
- **Hazards are an outline plus at most one signature mark** (asteroids: jagged
  outline and two pits; mines: a round shell, eight horns with bright tips, and
  a chain trailing behind the drift; coconuts: husk fibers and a pore face;
  cups: rim, handle, and saucer, with steam only while locked). Every theme has
  its own `draw_asteroid` / `draw_mine` / `draw_coconut` / `draw_cup`, all drawn
  from local-frame points via `ink_path`. `Rock.shape` only feeds the asteroid
  outline.
- **Mind the cell aspect.** A cell is about twice as tall as wide, so a circle
  in world units draws as a tall oval. Hazard art goes through `ink_path`,
  which squashes local y (`CELL_Y`/`CELL_X`) so shapes look round. Reuse it for
  any new silhouette instead of calling `to_px` on raw circle points.
  The lock state is shown by color and the chip, not extra geometry.
- **Backgrounds stay dim.** Particle density is low (`reseed_stars`), scenery
  sits at braille priority 0 in the faintest palette tones, and the Jungle
  canopy keeps only the top-corner fronds. If you can only see the background
  and not the hazards, it is too loud.
- **Unicode.** Stick to single-width glyphs (`·`, `›`, `‹`, `♥`, `♡`, `█`). Use
  `chars().count()` for text width, never `len()`, whenever a string may hold
  non-ASCII. `write()` already does.

### Adding or changing a theme

1. Add the variant to `Theme`, `Theme::ALL`, `label`, `index`, and `from_index`.
2. Add a `Palette` const and the `palette()` arm.
3. Add theme text arms in `game.rs` (banners, hints, scoring) and `draw.rs`
   (title subtitle, game-over text, prompts).
4. Add the ship as a function in `src/draw/ships.rs` and a `draw_ship` arm
   (plus its scale). Ships are described in square visual units with the nose
   toward negative y and drawn through `Craft`, which rotates then squashes by
   the cell aspect; do not hand-roll `to_px` calls.
5. Give hazards a theme-specific silhouette in the rock loop if needed. Keep it
   to an outline plus one signature mark.
6. Pick head and tail colors in `projectile_colors`. Do not write a new
   projectile; add a `Palette::chip` color for word chips.
7. Seed a few dim background particles in `reseed_stars`, and add the theme's
   exhaust distance and speed in `spawn_exhaust`'s tuple.
8. Update the theme-selection test in `game.rs`. `every_theme_has_one_line_hud_and_word_chips`
   in `draw.rs` loops over `Theme::ALL` and covers HUD and chips automatically.
9. Update `README.md`.

## Style

- Match surrounding code; keep the existing module layout and helper style.
- Keep functions small enough to satisfy clippy. `ink_leaf` already groups its
  position and size to avoid `too_many_arguments`.
- Use `f32` math for simulation, `u16`/`i32` for terminal coordinates.
- Prefer `fade`, `mix`, and palette colors over literals in draw code.
- Avoid new dependencies unless they are clearly necessary. ratatui already
  covers terminal I/O and layout.

### Writing code in this codebase

- **Prefer data over branching.** If a `match game.theme` arm repeats the same
  body with different numbers or colors, collapse it to one body that takes a
  tuple or a small helper (see `burst`, `projectile_colors`, the exhaust
  tuple). A fourth theme should be a new row of data, not a new copy of code.
  Do not copy-paste a block per theme.
- **Extract before a function grows.** `draw()` is already long. New drawing
  goes in its own helper that takes `ink`, `to_px`, and the palette, like
  `draw_projectile`. Return `String`/`&'static str` from small copy helpers
  (`play_hint`, `game_over_label`) instead of inlining strings in three places.
- **Single source for repeated copy.** Game-over labels, hints, and menu text
  live in one helper each. If you need the same text in two screens, call the
  helper.
- **Delete, don't comment out.** Remove dead code and unused parameters when a
  change makes them dead (clippy `-D warnings` will tell you).
- **Make edits precisely.** Do not use blanket `sed` or regex deletes across
  `game.rs`/`draw.rs`; identical lines recur in several functions (for example
  `let word = rock.word.clone();`). Edit with exact context and rebuild.
- **Keep game logic out of `draw.rs`.** Rendering reads `Game`; it never
  mutates it or rolls RNG.
- **Look at it.** Tests only prove strings render. After a visual change, run
  `cargo run --release` in a terminal of at least 80x24 (and a small one) and
  check each theme you touched.

## Tests

Tests are colocated in `#[cfg(test)] mod tests` inside `game.rs` and `draw.rs`.

- Game tests use `Game::new(w, h, seed)` (seeded) and helpers such as
  `playing()`. Do not depend on timing or filesystem state except through
  `Game::new`'s best-score load.
- Build scenario games with helpers such as `locked_rock_game(theme, word)`
  rather than copying rock setup into each test.
- Draw tests render with `ratatui::backend::TestBackend` via the `render()`
  helper and assert on the resulting text (e.g. menu labels and hint copy).
  Add a render assertion when you add player-visible strings or theme visuals.
- Every behavior change should add or update a focused test. Keep the suite
  green; currently 70 tests.

## Gotchas

- Frame pacing lives in `main.rs`: update, then `present` (a synchronized
  update, DEC mode 2026, so frames never tear), then wait for input until the
  next `FRAME` deadline (120 fps). A keypress cuts the wait short so typing
  shows at once. A frame costs well under 1 ms; keep it that way, since the
  whole budget is 8 ms. Do not go back to drawing before updating or to a
  fixed sleep after drawing; both read as lag.

- ratatui only sends cells that changed. A terminal that wipes cells itself
  (fullscreen toggles resize in several quick steps) leaves stale blanks that
  never repaint on their own. `main.rs` therefore calls `terminal.clear()` on
  any size change and again `SETTLE` after the last one; Ctrl+L forces a repaint.
  Do not remove this, and keep it in mind for any new screen that rarely
  changes (the shop's left card was the symptom).
- `reseed_stars` is called on resize and theme changes; particle state is not
  preserved across those events.
- Screen shake must not move the background; only gameplay conversion includes
  it.
- `Game::start()` resets run state but keeps the selected theme and best score.
- Best score persists at `~/.typoids-best` (`load_best`/`save_best`); the run
  save is separate (`~/.typoids-run`, see the roguelike section).
- The title menu gains a `CONTINUE` entry when a save exists (`has_save`), so
  key handling asks `Game::title_action()` instead of comparing indices.
- Hazards that reach the ship cost a life; during invulnerability hazards pass
  through rather than being destroyed for free.
- Don't put gameplay elements at braille priority `0`; they would be
  overpainted by background scenery.
