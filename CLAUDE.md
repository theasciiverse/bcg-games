# bcg-games

Our clean-room remakes of eight games from David H. Ahl's *BASIC Computer
Games* (1978): Hammurabi, Lunar Descent, Animal, Hexapawn, Bagels, Orbit,
Splat, Civil War. One crate, one release, eight binaries; each is its own
title in the Asciiverse catalog (`collection = "bcg-games"`). The second
release's candidates are in `../bcg-games-ii/CLAUDE.md`.

**Status:** 0.5.0 (2026-11-13), all eight playable, tested, with demos.
Not yet played through by a person.

## The rule that matters most: clean room

Never read the original BASIC listings or any port of them, and never use
their text. Work from `asciiverse/specs/<game>.md` (rules in our own
words, from the book's description and black-box play); change a spec
first, then the code. CLEANROOM.md records the process; keep it true.

## Layout

- `src/shell.rs`: command line, terminal, keys/mouse/gamepad as `Key`,
  frame clock, session logging; the `Game` trait every game implements.
- `src/ui.rs`: the shared look (palette, frame, panels, bars, NumField,
  TextField, menu). `src/audio.rs`: one sound palette, made in code.
  `src/store.rs`: the state folder. `src/rng.rs`.
- `src/games/<game>.rs`: rules, screens, demo player and tests of one
  game. `src/bin/<id>.rs`: one-line mains.
- `asciiverse/record-previews.sh`: the 100x30 catalog casts.

## Conventions

- `--demo` plays itself (previews); `--mute` never opens the audio device.
- Saves in the state folder only; demos never write there.
- No em or en dashes anywhere.
- Versions: 0.x.0 while unfinished, 1.0.0 when David has played them all.
