# TYPOIDS improvement plan

> **Status (2026-11-04): implemented.** All five tracks are in. Where the
> build differs from this plan:
> - 2.6 skipped: ratatui's `draw()` already queries the terminal size every
>   frame, so dropping our own call saves nothing.
> - 2.7: the `hits` vector only allocates on a collision, so it was left as
>   is. `ink_path` was made allocation-free instead.
> - 4.4: the 0.5 pace floor covers only lasting slow-downs (Slow Tide, Last
>   Stand), so Brake Pads still does what its card says.
> - 4.5: the curve was retuned with the balance bot: speed is
>   `3.4 + 2.2·ln(1 + 0.45w)`, plus a gentler `spawn_gap` and a long-word
>   ramp (`word_mix`).
> - 1.6: `~/.typoids-best` stays the HI number; the table starts empty.
> - 5.6: minions enter from the sides so the passage panel never hides them.

Five improvement tracks, each broken into steps that can be implemented and
merged one at a time. Every step follows `AGENTS.md`: no new dependencies,
data over branching, palette colors only, game logic stays out of `draw.rs`,
and each behavior change gets a focused test. After every step run:

```bash
cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings
```

Then play it with `cargo run --release` at 80x24 and at 60x20.

**Recommended order:** 2 (performance) → 3 (UX) → 4 (balance) → 1 (look) → 5 (fun).
Performance and UX fixes are low-risk and make everything after them easier
to test. Balance has to settle before new abilities are added on top. Track 1's
big-text helper and hit-stop are reused by track 5.

---

## Findings that shaped the plan

- **Frame cost (measured, release, 200x60, `TestBackend`):** draw plus diff
  costs ~240 µs per frame in Space, Underwater and Cat Cafe, and **~545 µs
  in Jungle**. `paint_jungle_canopy` runs `sin`/`cos`/`powf` plus two `mix`
  calls on every cell, every frame, but its output never changes (the `_time`
  argument is unused).
- Every braille cell, star, label letter and spark allocates a `String`
  (`&ch.to_string()`) and goes through `set_string`.
- The game redraws at 120 fps even in the shop, the pause menu and the
  game-over screen, where nothing moves.
- `InkLayer::line` walks every Bresenham step even for points far off-screen.
  There is no clipping; `dot` simply rejects each point.
- **Economy:** the 55 non-consumable tiers cost **1,242 crystals** in total.
  Base income is only ~250 by wave 10, so the leak is the stacking per-kill
  cards. Motherlode adds +5 to every kill, Steady Hands +3, then Lucky Penny
  doubles the lot, and Multishot, Scattershot and Sweeper multiply the kill
  count.
- **Difficulty:** hazard speed `3.6 + 0.55 * wave` grows without limit, but
  the field never gets more interesting (same hazards, a cap of 7 on screen
  from wave 8). Slow Tide (−50%), Focus (down to 33%), Brake Pads and Multishot
  together flatten that curve completely. There is no win condition, so a run
  has no shape.
- **UX traps:** the player is typing fast when a wave clears. A stray
  `space`, `1`-`4` or `Enter` in the new shop buys a card or skips the shop.
  `Esc` in the shop saves and quits to the title. `Enter` on game over
  restarts at once.

---

## 1. Aesthetics: a 90s arcade cabinet, kept simple

Goal: it should feel like a coin-op from 1994 (attract mode, big pixel call-outs,
an odometer score, a high-score table) without adding visual noise to the field.
"Quiet field, loud words" still holds: all new flair sits in text rows and
transitions, never in the braille layer.

### 1.1 Extend the pixel font
- `src/draw/bigfont.rs`: add digits `0-9` and `! ? : -` to the 5x7 font. Keep
  it data only.
- Test: every char in `"0123456789!?:-"` has at least one lit pixel.

### 1.2 Extract a reusable big-text helper
- `src/draw/stage.rs` already rasterises words into `Grid` and `blit`s half
  blocks. Pull that out into `pub fn big_text(buf, text, center_x, top_y, rows,
  color_fn)` in a new `src/draw/bigtext.rs`. `color_fn(letter_idx, px, py) ->
  Option<Color>` keeps the animations data-driven.
- `draw_stage` becomes a caller of it. Render tests for the stage must stay green
  unchanged; that is the regression check.

### 1.3 Arcade call-outs: READY / GO / CLEAR / BOSS
- `game.rs`: add `pub callout: Option<Callout { text: &'static str or String, t0: f32, dur: f32 }>`
  set by `begin_wave` (`"STAGE 3"` then `"GO!"`), wave clear (`"CLEAR!"`),
  `start_boss` (`"WARNING"`), `boss_win` (`"K.O.!"`, a period-perfect touch)
  and death (`"GAME OVER"`). Replace the plain-text `banner` for these moments
  only; small banners (`SLIP FORGIVEN`, `SECOND WIND`) stay plain text.
- Add a 1.2 s "READY" beat at the start of each wave: `begin_wave` sets
  `spawn_timer` so the first hazards arrive just after the GO.
- `draw.rs`: a `draw_callout` helper centered in the play area. Letters drop
  in one by one (stagger 40 ms each) and leave by the existing shatter
  noise. Colors come from `hot`, `amber` and `accent`.
- Tests: after `begin_wave_for_test(3)` the screen contains the big-text
  pixels (assert on `▀`/`█` density in the middle rows). Wave-clear sets
  `callout`.

### 1.4 Odometer score and arcade header labels
- `game.rs`: add `pub shown_score: u32`, eased toward `score` in `update`
  (roll ~8% of the gap per frame, minimum +7 a frame, snap when close). It is
  game state, so `draw` stays read-only.
- `draw_header`: keep it to one row but relabel it in arcade style:
  `1UP 004250   ♥♥♥   x4   ◆ 31          HI 012000`. Draw the score from
  `shown_score`. While rolling, the score digits flash `hot`; otherwise they
  are `amber`.
- Update `every_theme_has_one_line_hud_and_word_chips` for the new labels.

### 1.5 Attract-mode title screen
- Title letters get a slow color sweep: a band of `hot` that travels across
  the `TYPOIDS` block every 3 s (`mix(ink, hot, band(x - t))`). One function,
  no per-theme code.
- Replace the static subtitle row with `INSERT COIN  ·  PRESS ENTER`,
  blinking at 2 Hz. Keep the theme subtitle under it.
- Every 8 s while idle on the title, swap the menu block for a **HI-SCORE
  table** (top 5: rank, initials, score, wave) for 4 s. A key press brings
  the menu back. Data comes from track 1.6.
- Tests: the title render contains `PRESS ENTER`. With `game.time` advanced
  to the table window it contains `HI-SCORES`.

### 1.6 High-score table with initials
- New `src/scores.rs` (pure `encode`/`decode`, like `save.rs`). It stores 5
  entries `{initials: [u8; 3], score, wave}` in `~/.typoids-scores`. On first
  load, migrate `~/.typoids-best` into one `???` entry. `best` becomes
  `scores[0].score`. Tests use only `encode`/`decode` and never touch disk
  (same rule as run saves).
- New phase `Phase::Initials`. It is entered from `Dying` when the score
  makes the table. The player **types** three letters (it is a typing game),
  Backspace edits, and Enter confirms. Visit every exhaustive `match` on
  `Phase` in `game.rs`, `draw.rs` and `main.rs`.
- Draw: big-text initials (track 1.2) with a blinking cursor slot under
  `NEW HIGH SCORE`.
- Tests: a high score routes `Dying → Initials → GameOver`. A low score
  skips `Initials`. Round-trip `scores::encode/decode`.

### 1.7 Optional CRT scanlines
- `paint_water` / canopy: darken every other row's background by 7% when
  `settings.scanlines` is on (track 3.7). The gradient math is unchanged;
  `fade(col, 0.93)` is applied on odd rows. Default: on.
- Render test: with scanlines on, background colors of rows 2 and 3
  differ by the scanline factor.

---

## 2. Performance

All measurements use the bench in 2.1. Target: ≤150 µs per frame at
200x60 in every theme, and near-zero CPU while idle in menus.

### 2.1 Add a frame-cost bench
- `draw.rs` tests: add `#[test] #[ignore] fn frame_cost()`. It renders 2,000
  frames per theme at 200x60 into `TestBackend` and prints µs per frame. Run
  it with `cargo test --release frame_cost -- --ignored --nocapture`. It is
  ignored so the normal suite stays fast and deterministic.
- Record the baseline in the PR description (≈240 µs, Jungle ≈545 µs).

### 2.2 Cache the jungle canopy (largest win)
- The canopy depends only on `(w, h, palette)`. `draw` cannot mutate `Game`,
  so compute it in the simulation layer instead: add `pub canopy: Vec<Color>`
  to `Game`, filled in `set_screen` / `reseed_stars` (already called on resize
  and theme change). `paint_jungle_canopy` then just copies `set_bg` from the
  cache.
- Alternative if you would rather keep colors out of `game.rs`: a
  `thread_local!` `RefCell<(u16, u16, Vec<Color>)>` cache inside `draw.rs`.
  Pick one; the `Game` field is simpler to test.
- Keep `jungle_canopy_background_is_stable_over_time` green. Expect Jungle
  to fall to roughly the same cost as the other themes.

### 2.3 Stop allocating a `String` per cell
- Add `fn put_char(buf, x: i32, y: i32, ch: char, st: Style)` that bounds-checks
  and does `buf[(x, y)].set_char(ch).set_style(st)`.
- Use it in: the braille blit loop in `draw()`, stars, `draw_label`'s
  per-letter loop, plain-glyph sparks, and `hline` (which today calls `put`
  with `"─"` per cell).
- Expected: a measurable drop in the bench (most cells drawn per frame are
  braille).

### 2.4 Clip lines before walking them
- `InkLayer::line`: reject when both endpoints are off the same side of the
  pixel bounds. Clamp the step count to `w*2 + h*4` as a safety net. This
  makes off-screen lasers, shards and long boss paths free.
- Test: a line from `(-1e5, -1e5)` to `(-1e5, 1e5)` returns instantly and sets
  no bits.

### 2.5 Adaptive frame rate (CPU and battery)
- `main.rs`: pick the frame budget per phase. 120 fps in `Playing`/`Dying`
  and on the title (rocks drift). **30 fps** in `Shop`, `Paused`, `GameOver`
  and `Initials`, where only a cursor blinks. Keypresses still cut the wait
  short, so menus feel exactly as responsive.
- Enable crossterm focus events (`EnableFocusChange`). On `FocusLost`
  during `Playing`, call `game.pause()` (UX win too) and idle at 10 fps until
  `FocusGained`. Disable focus change on exit alongside `ratatui::restore()`.

### 2.6 Read the size only on resize
- `terminal.size()` is an ioctl every frame. Keep `seen` and update it only on
  `Event::Resize(w, h)` (already matched), with one `terminal.size()` at
  startup. Keep the `SETTLE` repaint logic exactly as documented in the
  Gotchas.

### 2.7 Small simulation cleanups
- `pick_word` clones every on-screen word into a `Vec<String>` per spawn; borrow
  instead (`let used: Vec<&str>`). The hot path is tiny, but it is free.
- Reuse the `hits` vector in `integrate_rocks` (or use `SmallVec`-style
  `[u32; 8]` + len) so the play loop does not allocate per frame.
- Re-run the bench and record the after numbers.

---

## 3. User experience and UI

### 3.1 Input guard on screen changes (fixes a real trap)
- `game.rs`: add `pub input_lock: f32`, set to 0.6 s by `open_shop`, by
  entering `GameOver`, and by entering `Initials`. While it is above 0, shop,
  game-over and initials keys are ignored (`main.rs` checks
  `game.accepts_menu_keys()`). The shop cards slide up into place during the
  lock (track 3.2), so the delay reads as animation, not lag.
- Tests: `open_shop_for_test()` then `shop_buy(0)` does nothing until
  `update(0.7)`. Same for `OVER_AGAIN` on game over.

### 3.2 Shop entrance and clearer purchase feedback
- Cards rise from the stage row with a 40 ms stagger. The offset comes from
  `game.time - shop_t0`, so `draw` stays pure.
- On purchase the card flashes `hot` once and the `◆` counter ticks down with
  the odometer from 1.4, reused for crystals as `shown_crystals`.
- Each card shows the change, not only the next value, for lines that scale:
  `Hazards drift 20% → 30% slower.` Add `Upgrade::blurb_delta(held)` next to
  `blurb` so text and simulation agree (per AGENTS card rules).
- Tests: blurb for Slow Tide at level 2 contains `→`. The card text never
  exceeds the inner width at 80 columns (extend
  `the_shop_shows_three_cards_and_its_buttons`).

### 3.3 Consistent, safer keys
- Shop: `Esc` no longer saves and quits. It opens the pause modal (Continue /
  Save & Quit), so one mistaken `Esc` never ends a session. `q` keeps its
  current meaning. Update `README.md` and the `AGENTS.md` shop key list.
- Playing: `Ctrl+W` and `Ctrl+Backspace` clear the whole buffer (a
  typist's reflex when locked onto the wrong word). Route through
  `clear_typed()` so stage timestamps stay in step.
- One shared helper `fn nav_delta(code) -> Option<isize>` in `main.rs`
  replaces the four copies of the `h/j/k/l/arrows` match arms.
- Tests: `clear_word()` clears buffer, `buffer_t` and target.

### 3.4 Better miss feedback
- The miss line shows the offending key and what was expected:
  `‹q› no rock starts with "q"`. With a locked word:
  `‹q› expected "e"`. Add `pub last_wrong: Option<char>` to `Game`.
  Keep the copy in one helper (`miss_message(theme, wrong, expected)`).
- Track accuracy for the wave and the run: `keys_ok`, `keys_bad`, plus
  `chars_typed` and active typing time for WPM.
- Tests: wrong key sets `last_wrong`, and the render contains the expected
  letter.

### 3.5 Wave report in the shop heading
- Shop heading line 2: `◆ 31   ·   ACC 96%   ·   54 WPM   ·   BEST COMBO x9`.
  Data from 3.4, reset in `begin_wave`. Game over shows the same line for
  the whole run.
- Render test: shop render contains `ACC` and `WPM`.

### 3.6 Pause shows your build
- The pause modal reuses `draw_installed` under its buttons, so the player can
  check the build mid-run without waiting for a shop. Add the run stats line
  from 3.5.
- Render test: paused game with an upgrade shows `INSTALLED 1`.

### 3.7 Settings and How to Play
- Title menu gains `OPTIONS` (scanlines on/off, screen shake on/off/low,
  reduced motion that disables callout motion and shake). Persist to
  `~/.typoids-settings` via pure `encode`/`decode` in a new `settings.rs`. As
  with saves, only `main.rs` enables persistence.
- `HOW TO PLAY`: a modal with five short lines and the key legend. It is
  shown automatically on the very first launch (no settings file yet).
- Because title items change, switch key handling to `game.title_action()`
  (already the pattern) and update menu tests.

### 3.8 Clearer danger signals
- At one heart, the heart glyph pulses `danger` and the stage floor line
  breathes red faintly. Uses existing `hurt` styling; no new rows.
- The combo label `x7` fades from `amber` to `dim` as `combo_left` drains, so
  the timer is visible without a bar. Expose `combo_ratio()`.

---

## 4. Enhancements: depth, difficulty, a run with a shape

Target experience: a full catalog is **impossible in one run**. By wave 10 a
good player owns ~12-15 tiers and has made real choices. Wave 20 is a hard
but fair finish line.

### 4.1 Give the run an end: 4 bosses, then victory
- Beating the wave-20 boss (100 WPM) is a **win**: big `YOU WIN` callout, a
  victory screen with stats, and the high-score entry. Offer `ENDLESS` to keep
  going (score keeps counting; the table marks endless runs with `+`).
- Add `Phase::Victory` and visit every `Phase` match.
- Tests: `boss_win` on tier 4 enters `Victory`. `ENDLESS` continues at wave 21.

### 4.2 Fix the crystal snowball
The changes, in order of impact:
1. **Prospector** becomes chance-based: each level gives +20% chance of one
   extra crystal per kill (Motherlode = always +1, not +5). Same save key.
2. **Steady Hands** gives +1 crystal at every level (score bonus still scales).
3. **Chain kills pay less**: Multishot, Scattershot and Sweeper kills drop one
   crystal flat and skip per-kill bonuses (Prospector, Lucky, Bounty). Pass a
   `chained: bool` into `detonate`.
4. **Inflation**: card cost adds `+1 per 2 tiers held across the run`
   (`Upgrades::total_levels() / 2`). The shop shows the inflated price,
   Discount still applies, and the price never drops below 1.
5. **Skill income instead**: a wave cleared without losing a heart pays a
   `FLAWLESS +N` bonus (N = 2 + wave / 3). Good typing, not card stacking, is
   the reliable money.
- Put every number in helpers next to `Upgrade::blurb` so card text agrees.
- Add an ignored `balance_report` test that simulates income for a
  greedy buyer at 1 kill/1.2 s and prints owned tiers by wave. Tune until
  it lands at ~13 by wave 10 and ~30 by wave 20.
- Tests: Motherlode never adds more than one crystal per kill. Chained kills
  drop exactly one. Price rises with total levels held. Flawless pays only
  when no heart was lost.

### 4.3 Build limit: the rack
- Installed lines are capped at **6 rack slots**, +1 per boss beaten (max 10).
  Upgrading a line you already own needs no new slot. Buying a new line with a
  full rack fails with `Rack full: sell a card first`.
- Selling: in the shop, `Tab` moves focus between the shelf and the
  installed table (which already exists). `Enter` on a rack card sells it for
  half its last price and removes the whole line. The selection index stays
  computed by helpers (`shop_rack_index`), never constants.
- Save format: add `rack=N`. Old saves decode with the default.
- This is the change that makes runs different from each other: you cannot
  hold every Slow-Tide-style safety card and every crystal card.
- Tests: buying a seventh line fails. Selling frees a slot and refunds
  crystals. `rack` round-trips through `save::encode/decode`.

### 4.4 Cap the safety stack
- Combined hazard pace from Slow Tide, Last Stand and Brake Pads has a floor of
  0.55 of base speed (Focus stays separate since it is per-target). Add
  `fn hazard_pace(&self) -> f32` so there is one place to read and test.
- Multishot can no longer chain off a chained kill (no recursion through
  Sweeper → Multishot), and Sweeper triggers on every 25th *typed* kill.

### 4.5 A better difficulty curve: elites, not just speed
- Speed: `3.6 + 2.6 * ln(1 + 0.45 * wave)`, which is gentler late and stops
  runaway speeds. The on-screen cap rises to 9 after wave 12.
- **Elite hazards**, rolled in `spawn_threat` by wave and marked **on the chip
  only**. Per the design rules, no extra geometry, just a prefix glyph and
  chip color:
  - `÷` **Splitter** (wave 3+): on death breaks into two short-word fragments
    that fly apart. This is classic Asteroids and gets an Asteroids-style
    reward (fragments drop crystals).
  - `»` **Runner** (wave 6+): 3-4 letters, 1.8x speed, `danger` chip text.
  - `×2` **Armored** (wave 9+): type the word twice. The first completion
    cracks it (hit flash plus ring) and re-arms with the same word.
  - `?` **Masked** (wave 13+): letters after the first two show as `·` until
    you lock it. Reading ahead stops working; reflex typing matters.
- Add `pub kind: Kind` to `Rock` (default `Normal`). Mix tables by wave live
  in one data array (`ELITE_TABLE: [(min_wave, kind, weight)]`), not branches.
- Tests per kind: splitter spawns two children with distinct words, armored
  needs two completions, a runner is faster than a normal rock in the same
  wave, and masked chips hide letters until locked.

### 4.6 Heat levels (replay value)
- Winning a run unlocks **Heat 1**, winning on Heat N unlocks N+1 (max 5),
  stored in settings. Heat is chosen on the title (`START  HEAT 2`).
- Each level adds one rule: (1) hazards +12% speed, (2) the shop shows one
  card fewer, (3) a miss also breaks the combo, (4) bosses +10 WPM, (5)
  start with two hearts. Keep it as a data table in `game.rs`.
- Tests: each heat rule applies, and heat persists via settings
  `encode/decode`.

---

## 5. Fun: abilities, juice and surprises

Everything here respects the design rules: no letter bursts, no confetti, and
new visuals go through rings, chips, the stage, callouts and palette color.

### 5.1 Hit-stop and kill weight
- `Game::hitstop: f32`. `update` scales `dt` to 0 while it is above 0
  (effects still decay so it never looks frozen). Kills set 35 ms, words of 8+
  letters 70 ms, and boss K.O. 200 ms. Respect `reduced motion`.
- A word of 8+ letters also gets a second, wider ring (via `burst`) and a
  one-frame `hot` flash on the stage floor line.
- Test: during hit-stop, rock positions do not change. Effects still tick.

### 5.2 OVERDRIVE meter (new active ability)
- Clean words (no slips) charge a meter: +1 per letter, full at 60. The
  meter **is the stage floor line**: it fills left to right in `amber`, then
  shimmers when full. No new HUD row.
- Press `Tab` (never a letter, so no typing conflict) to fire **OVERDRIVE**
  for 5 s: the world runs at 45% speed (the ship and stage at full speed), the
  combo counts double, and the palette tints toward `hot`. A big-text
  `OVERDRIVE!` callout plays.
- On boss waves, OVERDRIVE pauses the boss clock for 3 s instead.
- New cards: `OVERCLOCK` (meter fills 30% faster, uncommon) and `AFTERBURN`
  (overdrive lasts +2 s, rare, gated on Overclock in `eligible`).
- Tests: clean letters charge and slips do not. `Tab` with a full meter starts
  overdrive and halves hazard pace. The meter empties afterwards.

### 5.3 Golden hazards and power-ups
- About 1 in 18 spawns (not on boss waves) is **golden**: `◆` prefix, `gem`
  chip color, and it drifts slower. Destroying it grants a random
  pickup, announced with a callout:
  - **FREEZE**: all hazards stop for 3 s.
  - **NUKE**: every word of 4 letters or fewer detonates (counts as chained,
    per 4.2).
  - **MAGNET**: crystals are collected instantly for the rest of the wave.
  - **BUBBLE**: +1 ward for this wave.
- Pickups are data: `enum Pickup` plus a `PICKUPS` table of name and duration.
- Tests: a golden kill grants exactly one pickup. FREEZE zeroes hazard motion
  for its duration.

### 5.4 Combo announcer (90s arcade voice)
- At combo milestones a one-word big-text callout appears over the stage for
  0.6 s: x5 `NICE!`, x10 `GREAT!`, x15 `RADICAL!`, x20 `TUBULAR!`, x30
  `UNSTOPPABLE`. The table lives next to the callout data.
- On reaching x10 for the first time in a run the score multiplier label in
  the header briefly turns `accent`.
- Test: the 10th consecutive kill sets the `GREAT!` callout.

### 5.5 Word links
- Finishing a word whose **first letter matches the last letter of the
  previous word** (`echo → orbit`) is a **LINK**. A braille laser runs from the
  old kill spot to the new one, the float text says `LINK x2`, and links stack
  a 1.25x score bonus each (resetting on a non-link). It rewards scanning the
  field strategically, a new skill layer that fits a typing game.
- The chip of any word that would extend the current link gets a faint
  `accent` underline glyph on its left pad (chip-only, rules-compliant).
- Tests: a two-word link scores more than two unlinked words of the same
  length. Breaking the link resets it.

### 5.6 Boss phase two
- At 50% passage progress the boss **roars** (strike animation plus callout
  `PHASE 2`). It throws two minion hazards with short words that drift toward
  the ship. The passage pauses on its current word until both are typed,
  and the clock keeps running. Minions use the normal rock code
  (`spawn_threat` with a forced short word), so they need no new art.
- Underwater: the shark circles faster in phase two (`SWIM * 1.4` via
  `boss::drift`).
- Tests: crossing 50% spawns exactly two minions once. Passage input routes to
  minions until they are cleared. The clock never pauses.

### 5.7 Daily run
- Title option `DAILY`: the seed is the date (`days since epoch`, supplied by
  `main.rs` per the RNG rule), Heat 0, and a fixed theme rotation by weekday.
  Its own best score line in the scores file: `DAILY 2026-11-04  012340`.
- Test: two games built with the same daily seed spawn identical first words.

---

## Definition of done (every step)

1. `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
   are clean.
2. New behavior has a focused test. New visible strings have a render
   assertion.
3. `README.md` is updated for any change to controls, menus, cards or rules.
   `AGENTS.md` is updated when a rule, file or phase list changes (new
   `scores.rs`, `settings.rs`, `bigtext.rs`, phases `Initials` and `Victory`,
   new shop keys).
4. Played in a real terminal at 80x24 and at a small size (stage-less layout)
   in every theme touched.
5. For track 2 steps, before and after numbers from the `frame_cost` bench go
   in the commit message.
