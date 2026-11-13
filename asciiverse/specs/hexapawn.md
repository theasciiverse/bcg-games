# Hexapawn: rules spec

Written 2026-11-13 from the book's description (p. 83); the original was
not run, as the description gives the whole game.

## Facts from the description

- A 3x3 board, three pawns each. Pawns move one square forward onto an
  empty square or capture one square diagonally forward.
- A side wins by reaching the far row, by capturing all enemy pawns, or
  when the other side cannot move.
- The computer learns by elimination: it remembers every position it meets
  and the legal moves from it; when it loses, it strikes out the move that
  led to the loss; when a position has no moves left it strikes out the
  move that led there and resigns. In time it cannot be beaten.

## Our design

- The human moves first, from the bottom row.
- The memory is saved in the state folder, so the computer keeps learning
  across runs; R wipes it.
- The screen shows the memory: positions met, moves struck out, a run of
  recent results, and for the current position each move it still trusts
  or has struck out.
- Moves by arrows and Enter, the number pad's layout, or the mouse.
