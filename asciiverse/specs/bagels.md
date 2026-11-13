# Bagels: rules spec

Written 2026-11-13 from the book's description (p. 9) and from playing the
original as a black box.

## Facts from the description and from play

- The computer picks a three-digit number with no digit repeated.
- Twenty guesses. After each, clues: Pico for each right digit in the
  wrong place, Fermi for each right digit in the right place, Bagels when
  no digit is right. The clues do not say which digit is which.
- A good strategy needs at most about eight guesses.

## Our design

- Fermis are listed before Picos.
- A digit pad that dims digits a Bagels clue has ruled out, a count of the
  numbers still possible, and a record of games solved and average guesses
  kept in the state folder.
