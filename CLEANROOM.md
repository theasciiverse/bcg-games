# How these games were made: clean room

bcg-games are new games after eight programs in David H. Ahl's *BASIC
Computer Games* (Creative Computing, 1978). They are built from the rules
of those games, which are free for anyone to use, and not from the
programs, which belong to their authors. This file records how that line
was kept.

## Sources that were read

- The book's description of each game, as transcribed in the README of
  each folder of github.com/coding-horror/basic-computer-games (the
  "As published in Basic Computer Games (1978)" texts). Ahl placed what he
  wrote and edited in the public domain.
- For Lunar Descent, that README also carries a contributor's porting
  note naming the program's variables and its use of the rocket equation;
  it was read while gathering the description. The rocket equation is
  physics, and none of the note's structure was used.
- Wikipedia's articles on *Hamurabi* and the lunar lander genre, for the
  history and the credits.
- Today's planetary gravity figures (Splat) and standard histories of the
  Civil War battles (men engaged and casualties, rounded).

## Sources that were not read

- The BASIC listings (`.bas` files) of the eight games, and the ports in
  other languages in that repository. The listings were downloaded only
  to be run, unread, as black boxes.

## Play as a black box

On 2026-11-13 Hammurabi, LUNAR, Bagels, Orbit, Splat and Civil War were
run in PC-BASIC 2.0.8 (`pcbasic -n`) with typed or scripted answers, and
what they did was written down: starting values, ranges, probabilities,
what ends a game. Animal and Hexapawn were not run; their descriptions
give the whole game.

## Writing

`asciiverse/specs/<game>.md` states each game's rules in our own words,
from those two sources, and marks which numbers were observed and which
are our own design. The code was written from the specs. No text from the
programs (prompts, messages, jokes, endings) is used: every word on screen
is ours, as are the structure, the screens and the extras (charts, the
learned tree, the computer's memory, the orbit plot, live flight).

The names: Hammurabi is a historical name; Animal, Hexapawn, Bagels,
Orbit and Splat are ordinary words and Martin Gardner's game's name; the
lander is called Lunar Descent; Civil War is descriptive. The original
programs' enemy in Orbit is replaced by our own cloaked raider.

Done by Claude (Anthropic's model) working for David Sandquist,
2026-11-13. The legal line followed: a new and independent work made in
free association with another is its maker's own (Swedish copyright act,
1960:729, 4 § second paragraph); a translation of a program into another
language would not be (17 U.S.C. § 103, URL 4 § first paragraph).
