# TYPOIDS

TYPOIDS is Asteroids with words on the hazards, dressed like a 90s arcade cabinet. Four themes are included: the original space field with a spaceship and asteroids, an underwater field with a submarine and spiked sea mines, a jungle canopy where falling leaves and fireflies drift past a banana-throwing monkey and its coconuts, and a cozy cat cafe where warm steam curls past a pouncing cat batting at cups of coffee with scratch marks. Type a word, the ship turns, fires, and blows that hazard apart.

This is a terminal game (ratatui). It is meant to be played in your own terminal, not in a browser.

## Run

```bash
cd typoids
cargo run --release
```

Rust 1.75 or newer. The terminal should be at least 70×22 and speak UTF-8 (braille glyphs draw the ship and the hazards).

## Sound

The game makes chiptune sound effects for menus, typing, breaking hazards, perfect words, the shop, the boss and more. They are generated in code. On Linux they play through `pw-cat`, `pacat` or `aplay`, whichever your system has (PipeWire, PulseAudio or plain ALSA); on Mac and Windows they go straight to the system audio. Turn them down or off under **Options › SOUND**.

## The title screen

- The mouse works in every menu: point at an item to highlight it and click to choose it. That covers the title, options, themes, the pause, game-over and victory popups, and the shop's cards and buttons.

- `j`/`k` or the arrow keys move through **Start, Daily Run, Themes, Options, How to Play, Quit** (plus **Continue** when a run is saved). Enter selects, `Q` quits.
- Leave the title alone and it turns into an attract screen that shows the **HI-SCORES** table. Any key brings the menu back.
- **Daily Run**: everyone gets the same words today. The theme rotates by day and heat is off. Your best daily score is kept.
- **Options**: CRT scanlines, screen shake (full, low, off), reduced motion (no hit-stop, shake or call-out motion) and sound (full, low, off).
- **How to Play** opens automatically the first time you launch the game.
- **Heat**: winning a run unlocks Heat 1, and winning at Heat N unlocks N+1 (up to 5). With START highlighted, `h`/`l` picks the heat. Each level adds a rule: hazards 12% faster, one card fewer in the shop, a miss breaks the combo, bosses demand 10 more WPM, start with two hearts.

## Play

- Type the word under a hazard. The first letters lock the nearest match; the ship turns to face it. Each correct letter fires a shot, torpedo, banana, or claw scratch. Finishing the word destroys the hazard.
- Every wave opens with a big **WAVE n … GO!** call-out before the first hazards arrive.
- The word you are typing is shown in big pixel letters along the bottom of the screen. Very short terminals (under 22 rows) fall back to a single input line.
- Wrong letters are ignored, and the dock tells you which key you pressed and which letter was expected. Backspace erases one letter; **Ctrl+W** (or Ctrl+Backspace) clears the whole word.
- **Overdrive**: clean letters charge a meter drawn on the floor line under the big letters (a slip drains it a little). When it shimmers, press **Tab**: for five seconds the field slows down and every kill counts double toward the combo. Against a boss it stops the clock for three seconds.
- **Links**: finish a word that starts with the last letter of the previous one (echo → orbit) for a growing score bonus. Words that would extend the link show a small ring at the start of their chip.
- **Combos** are announced at x5, x10, x15, x20 and x30 (NICE!, GREAT!, RADICAL!, TUBULAR!, UNSTOPPABLE!). The combo number fades as its timer runs out.
- **Elite hazards** appear as the waves go on, marked in front of their word:
  - `÷` splitters break into two short-word fragments.
  - `»` runners are short and fast.
  - `×2` armored hazards must be typed twice.
  - `?` masked words only show their first letters until you lock them.
- **Golden hazards** (`◆`) drop a power-up: FREEZE (everything stops), NUKE (every short word pops), MAGNET (crystals fly straight to you) or BUBBLE (an extra ward).
- Destroyed hazards drop crystals (◆) that fly to your ship. A wave cleared without losing a heart pays a **FLAWLESS** bonus.
- Every fifth wave is a boss: a typing test instead of a field of hazards. It plays like Monkeytype: type the passage with a **space** between words before the clock runs out. Typos show in red and must be fixed with Backspace (or Ctrl+W for the whole word) before space moves you on, so mistakes cost time, not hearts. The boss demands 40 WPM, then 60, 80, 100 and finally 120. Beat it for crystals, a heart and an extra rack slot. **Run out of time and the run is over**: the boss escapes, and no spare heart or upgrade saves you.
- **Win the run** by beating the wave-20 boss. Then choose **Endless** to keep going, or **Finish Run** to bank the score.
- Esc opens the pause popup (Continue, Restart, Save & Quit). It also shows your installed upgrades and your accuracy, WPM and best combo so far.
- If you switch away from the terminal, the game pauses itself.
- Hazards that reach the ship cost a life. The ship is briefly invulnerable afterward and hazards pass through it during that time. You start with three hearts (Extra Heart adds more); at zero the run ends.
- A score that makes the top five asks for your **initials** (type three letters, Enter confirms).

## The shop

- After each wave a shop offers three cards (four with Wide Shop). Its heading shows the wave's accuracy, WPM and best combo.
- Use `h`/`l` or the arrows with `Enter`, or press `1`-`4` to buy, `r` to reroll (10 crystals, and a little more each time), and `space` for the next wave.
- Cards that scale show the step they buy, for example "Hazards drift 20% → 30% slower."
- Every upgrade is a line of separate cards: buying Slow Tide does not restock it. Deep Current, the next card in the line, has to turn up on a later shop or reroll.
- There are 27 lines, including Steady Hands, Prospector, Multishot and Scattershot, Focus, Brake Pads, Lucky Penny, Clean Slate, Shrapnel, Last Stand, Recycler, Bounty, Sweeper, Discount, Wide Shop, Second Wind, Overclock and Afterburn.
- **The rack**: you can hold six upgrade lines at once, plus one per boss beaten (up to ten). Upgrading a line you already own needs no new slot. Press **Tab** (or click **MY UPGRADES**) to open your upgrades: every line you hold, what it does right now, what its next card adds, and what it sells for. Choose one with `j`/`k` and press Enter (or click **SELL**) to sell it for half its last price. Tab goes back to the shelf.
- Prices rise a little with every tier you hold. Kills made by Multishot, Scattershot, Sweeper or a Nuke drop one crystal each.
- For a moment after the shop opens, keys are ignored, so letters you were still typing do not buy anything by accident.
- Esc in the shop opens the pause popup; `Q` saves and quits.

## Saves and scores

- Pause and pick Save & Quit (or press `Q` in the pause menu or shop) to save the run to `~/.typoids-run`. The title menu then offers Continue. A mid-wave save resumes at the start of that wave. The save is cleared when a run ends.
- Best score is remembered in `~/.typoids-best`, the high-score table and daily best in `~/.typoids-scores`, and options and unlocked heat in `~/.typoids-settings`.
