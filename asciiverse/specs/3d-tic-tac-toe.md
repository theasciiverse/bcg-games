# 3-D Tic-Tac-Toe: rules spec

Written 2026-11-14 from the idea of the book's game and the board game it
plays, without its listing or black-box play.

## The original

Ahl wrote that the program turned up, author unknown, on a General
Electric time-sharing system in 1968. It was called *Qubic* in *101 BASIC
Computer Games* (DEC, 1973) and *3-D Tic-Tac-Toe* in *BASIC Computer
Games* (1978). The player names a cell by level, row and column, each 1
to 4, and plays the computer on a 4x4x4 cube. Parker Brothers sold the
board game as Qubic from 1964; Oren Patashnik showed with a
computer-assisted proof in 1980 that the first player can always win.

Sources: en.wikipedia.org/wiki/3D_tic-tac-toe;
obsessivegamingchronology.blogspot.com/2024/07/prehistory-xii-qubic-3d-tic-tac-toe.html
(both read 2026-11-14).

## Our design

- A cube of four levels, each 4x4. Players take turns placing a mark; the
  first with four in a line wins. There are 76 lines: 48 along the rows,
  columns and verticals, 24 diagonals across a level or a vertical slice,
  4 through the centre corner to corner. A full cube with no line is a
  draw.
- You are X and the computer O; who moves first alternates game by game.
- The computer, in order: completes a line of its own; blocks yours
  (easy forgets this one time in seven); makes a move that leaves two
  lines of three at once; takes the cell where you could do that (hard
  first tries to force you with a line of three that also covers that
  cell); otherwise the cell worth most, counting the open lines through
  it weighted by the marks already in them, with random noise: much on
  easy, some on fair, little on hard.
- Screen: the four levels two by two, level 1 the top; a cursor moved with
  the arrows within a level and PgUp/PgDn between levels; clicks place.
  The winning line is lit across the levels.
- Record of won, lost and drawn per strength, in the state folder.
