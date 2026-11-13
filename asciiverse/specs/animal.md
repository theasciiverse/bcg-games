# Animal: rules spec

Written 2026-11-13 from the book's description (p. 4); the original was
not run, as the description gives the whole game.

## Facts from the description

- You think of an animal; the computer asks yes-or-no questions and then
  guesses.
- When it guesses wrong it asks for your animal and a question that tells
  the two apart, and which answer fits your animal. It learns.
- It starts knowing two animals, a fish and a bird.
- The original could list the animals it knows; it could not save them.

## Our design

- The knowledge is a tree of questions with animals at the leaves; it is
  saved in the player's state folder and kept between runs.
- The first question is our own ("Does it live in water").
- Answers by key (Y/N), arrows or clicking buttons; questions are typed,
  tidied (no question mark, capital first letter).
- A side panel lists every animal it knows, or shows the whole tree with
  the current path lit.
