# Acey Ducey: rules spec

Written 2026-11-14 from the idea of the book's game, without its listing or black-box play.

## Our design

- One 52-card deck, shuffled; it is shuffled anew when it runs low.
  Ranks 2 to 10, jack, queen, king, ace; aces are high.
- Two cards are dealt face up. A pair with no rank between them (equal or
  next to each other) goes back under the deck and two more are dealt,
  so there is always something to bet on.
- The player bets any whole number of dollars from 0 up to everything in
  the purse; a bet of 0 is a pass. The third card is turned over: if its
  rank is strictly between the two, the bet is won, otherwise lost. A
  card equal to either one loses.
- The purse starts at $100. The game ends when it is empty, or when the
  player walks away with what is left.
- Help on the table: the ranks that win are lit along a ruler, and the
  share of the cards left in the deck that would win is shown before the
  bet.
- Kept in the state folder: games played, times broke, the best purse
  walked away with, and the most ever held.
- The demo player bets a share of its purse that grows with the odds, and
  passes on long shots.
