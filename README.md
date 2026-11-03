# TYPOIDS

TYPOIDS is Asteroids with words on the hazards. Four themes are included: the original space field with a spaceship and asteroids, an underwater field with a submarine and spiked sea mines, a jungle canopy where falling leaves and fireflies drift past a banana-throwing monkey and its coconuts, and a cozy cat cafe where warm steam curls past a pouncing cat batting at cups of coffee with scratch marks. Type a word, the ship turns, fires, and blows that hazard apart.

This is a terminal game (ratatui). It is meant to be played in your own terminal, not in a browser.

## Run

```bash
cd typoids
cargo run --release
```

Rust 1.75 or newer. The terminal should be at least 70×22 and speak UTF-8 (braille glyphs draw the ship and the hazards).

## Play

- At the title menu, use `h`/`j`/`k`/`l` or the arrow keys to pick Start, Themes, or Quit, then press Enter. `Q` also quits.
- Choose Themes to switch between Space, Underwater, Jungle, and Cat Cafe. `h`/`l` or the arrow keys move the highlight, Enter applies the theme, and Esc backs out without changing it.
- Type the word under a hazard. The first letters lock the nearest match; the ship turns to face it.
- The word you are typing is shown in big pixel letters along the bottom of the screen, above a floor line. Typed letters slide up from the floor, the rest of the word assembles in a dim tone, and a finished word dissolves. Hazards still come from every edge, right down to the stage. Very short terminals (under 22 rows) fall back to a single input line.
- Each correct letter fires a shot, torpedo, banana, or claw scratch and shoves the ship. Finishing the word detonates the hazard.
- Wrong letters are ignored (the buffer does not change).
- Backspace erases. Esc opens the pause popup, where you can Continue, Restart, or Quit. Quit returns to the title screen. Use `h`/`j`/`k`/`l` or the arrow keys to choose, Enter to select, and Esc to resume.
- When a run ends, the game-over popup offers Play Again, Themes, or Quit. Play Again is selected by default, so pressing Enter starts a new run immediately. Themes works the same as on the title screen.
- `Q` returns to the title from the pause popup, and quits the game from the title or game-over popups.
- Destroyed hazards drop crystals (◆) that fly to your ship. Longer words drop more.
- Every fifth wave is a boss: a typing test instead of a field of hazards. Type the passage word by word before the clock runs out. The boss demands 40 WPM, then 60, 80, 100 and finally 120. Every wrong key makes the boss strike and costs a heart (with a short grace period). Beat it for a pile of crystals and a heart; run out of time and you lose two hearts and the boss escapes. Underwater, the boss is a great white that swims laps while you type.
- After each wave a shop offers three cards (four with Wide Shop). Use `h`/`l` or the arrows with `Enter`, or press `1`-`4` to buy, `r` to reroll (10 crystals, and a little more each time), and `space` for the next wave. Every upgrade is a line of separate cards: buying Slow Tide does not restock it, and Deep Current, the next card in the line, has to turn up on a later shop or reroll. There are over 25 lines, including Steady Hands, Multishot and Scattershot, Focus, Brake Pads, Lucky Penny, Clean Slate, Shrapnel, Last Stand, Recycler, Bounty, Sweeper, Discount, Wide Shop, Second Wind and Alliteration.
- Pause and pick Save & Quit (or press `Q` in the pause menu or shop) to save the run to `~/.typoids-run`. The title menu then offers Continue. A mid-wave save resumes at the start of that wave. The save is cleared when a run ends.
- Hazards that reach the ship cost a life. The ship is briefly invulnerable afterward and hazards pass through it during that time. You start with three hearts (Extra Heart adds more); at zero the run ends.
- Best score is remembered in `~/.typoids-best`.
