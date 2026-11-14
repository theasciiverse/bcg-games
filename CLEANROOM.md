# How these games were made: clean room

bcg-games are new games after programs in David H. Ahl's *BASIC
Computer Games* (Creative Computing, 1978). They are built from the rules
of those games, which are free for anyone to use, and not from the
programs, which belong to their authors. This file records how that line
was kept.

## First release: sources that were read

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
- Today's planetary gravity figures (Splat), and for Civil War each
  battle's Wikipedia article (men engaged, casualties, result), listed
  with their figures in asciiverse/specs/civil-war.md.

## First release: sources that were not read

- The BASIC listings (`.bas` files) of the eight games, and the ports in
  other languages in that repository. The listings were downloaded only
  to be run, unread, as black boxes.

## First release: play as a black box

On 2026-11-13 Hammurabi, LUNAR, Bagels, Orbit, Splat and Civil War were
run in PC-BASIC 2.0.8 (`pcbasic -n`) with typed or scripted answers, and
what they did was written down: starting values, ranges, probabilities,
what ends a game. Animal and Hexapawn were not run; their descriptions
give the whole game.

## Writing (both releases)

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

## The second release (2026-11-14)

Ten more games: Basketball, Football, Boxing, Bullfight, Golf, Acey
Ducey, King, Fur Trader, 3-D Tic-Tac-Toe, and Hurkle, Mugwump and Depth
Charge as one program, Hide and Seek. They were made more strictly still:

- **Nothing from the programs was fetched or run.** No listing was
  downloaded, not even to run unread, and there was no black-box play.
  The book-description READMEs in the repository that also holds the
  ports were not opened either.
- **The rules are our own,** designed from the general idea of each game
  (a basketball game where you call the shots and the defence; a bout
  where you choose punches; a bet on whether a third card falls between
  two; a nation to govern for eight years; furs to carry to market) and,
  for the hide-and-seek games, from how histories describe them. Every
  number in the specs is marked as ours unless a cited history gives it.
- **Credits and history** come from secondary sources only: Wikipedia,
  histories of early computer games (obsessivegamingchronology.blogspot.com,
  bluerenga.blog), Jim Storer's own page at Brandeis for King, and
  search-result summaries for a few magazine credits. Pages whose main
  content is program code were not opened. Each spec's "The original"
  section names its sources; a credit that could not be verified is left
  out rather than guessed.
- **Content:** Bullfight follows the bloodless Portuguese style, where
  the bull is not killed in the ring; Boxing is an amateur bout in
  sporting language. King's island and Fur Trader's posts, routes and
  partners are our own, written with care for the history they touch.
- **Names:** Basketball, Football, Boxing, Bullfight, Golf, King, Fur
  Trader, 3-D Tic-Tac-Toe and Hide and Seek are ordinary words; Acey
  Ducey is the card game's old name; the hurkle (from Theodore Sturgeon's
  story) and the mugwump are the creatures PCC's games hid.

Done by Claude (Anthropic's model) working for David Sandquist,
2026-11-13 and 2026-11-14. The legal line followed: a new and
independent work made in free association with another is its maker's own (Swedish copyright act,
1960:729, 4 § second paragraph); a translation of a program into another
language would not be (17 U.S.C. § 103, URL 4 § first paragraph).
