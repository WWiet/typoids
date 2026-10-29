# TYPOIDS

TYPOIDS is Asteroids with words on the hazards. Two themes are included: the original space field with a spaceship and asteroids, and an underwater field with a submarine and spiked sea mines. Type a word, the ship turns, fires, and blows that hazard apart.

This is a terminal game (ratatui). It is meant to be played in your own terminal, not in a browser.

## Run

```bash
cd typoids
cargo run --release
```

Rust 1.75 or newer. The terminal should be at least 70×22 and speak UTF-8 (braille glyphs draw the ship and the hazards).

## Play

- At the title menu, use `h`/`j`/`k`/`l` or the arrow keys to pick Start, Themes, or Quit, then press Enter. `Q` also quits.
- Choose Themes to switch between Space and Underwater. `h`/`l` or the arrow keys move the highlight, Enter applies the theme, and Esc backs out without changing it.
- Type the word under a hazard. The first letters lock the nearest match; the ship turns to face it.
- Each correct letter fires a shot or torpedo and shoves the ship. Finishing the word detonates the hazard.
- Wrong letters are ignored (the buffer does not change).
- Backspace erases. Esc pauses. On the title, pause, or game over, Q quits.
- Hazards that reach the ship cost a life. The ship is briefly invulnerable afterward and hazards pass through it during that time. Three lives, then the wave starts over.
- Best score is remembered in `~/.typoids-best`.
