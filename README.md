# bcg-games

Eight games after David H. Ahl's *BASIC Computer Games* (Creative
Computing, 1978), the book that put computer games into a generation's
hands, rebuilt for today's terminal. Each is its own program, with a real
screen, sound made in code, mouse and keys, and what it learns kept
between runs.

| Program | The game | After |
|---|---|---|
| `hammurabi` | Rule a Sumerian city for ten years | Doug Dyment's *The Sumer Game* (1968), in Ahl's *Hamurabi* |
| `lunar-descent` | Land on the Moon by managing the burn | Jim Storer's *Lunar* (1969) |
| `animal` | It guesses your animal, and learns | Arthur Luehrmann (Dartmouth), Nathan Teichholtz, Steve North |
| `hexapawn` | A pawn game against a computer that learns | Martin Gardner's game (1962); program by R. A. Kaapke, Jeff Dalton, Steve North |
| `bagels` | Three digits from Pico, Fermi, Bagels clues | D. Resek and P. Rowe, Lawrence Hall of Science |
| `orbit` | Bomb a cloaked ship circling your planet | Jeff Lederer, Project SOLO |
| `splat` | Open your chute as late as you dare | John Yegge, Oak Ridge Associated Universities |
| `civil-war` | Fourteen battles, measured against history | L. Cram, L. Goodie and D. Hibbard (1968); G. Paul and R. Hess |

These are new games written from the rules, not ports of the old
programs: see [CLEANROOM.md](CLEANROOM.md) and `asciiverse/specs/`.

## Play

With [av](https://asciiverse.sh): `av install hammurabi` (or any of the
others). Each runs on its own: `hammurabi --help` lists its keys, `--demo`
lets it play itself, `--mute` keeps it quiet, M toggles the sound.
What a game keeps (scores, the animal tree, Hexapawn's memory) is in the
state folder: `~/.local/state/<game>` on Linux, Application Support on
macOS, `%LOCALAPPDATA%` on Windows.

Build: `cargo build --release`. Logging: `RUST_LOG=debug hammurabi 2>log`.

MIT licence. Part of Asciiverse.
