# Hide and Seek: rules spec

Written 2026-11-14 from the idea of the book's three games, without their
listings or black-box play. Three games of finding something hidden on a
grid from clues are one program here, with a choice of game at the start.

## The originals

- **Hide and Seek**, a program by Bud Valenti's students at Project SOLO
  in Pittsburgh, gave the People's Computer Company the idea for
  **Mugwump**, written or adapted by Bob Albrecht and printed in PCC's
  newspaper in 1973: a mugwump hides on a 10x10 grid with coordinates 0
  to 9, and each miss tells the player how far away it is.
  Ahl's 1978 book hides four of them.
- **Hurkle**, also from PCC in Menlo Park, California, credited to Bob
  Albrecht (1973): a hurkle hides on a 10x10 grid and each guess is
  answered with the compass direction to go; the PCC version allowed five
  guesses. The name comes from Theodore Sturgeon's story "The Hurkle Is a
  Happy Beast".
- **Depth Charge**, by Dana Noftle of Acton, Massachusetts, in the first
  issue of *Creative Computing* (November and December 1974): a
  submarine hides in a cube of sea whose size the player chooses; each
  depth charge is aimed with two coordinates and a depth, and sonar says
  where the submarine lies from it.

Sources: en.wikipedia.org/wiki/Mugwump_(game);
bluerenga.blog/2019/03/15/before-adventure-part-2-mugwump-hurkle-snark-1973;
obsessivegamingchronology.blogspot.com/2025/09/014-hurkle-solitary-relative-of-mugwump.html;
en.wikipedia.org/wiki/Hunt_the_Wumpus (all read 2026-11-14). Dana
Noftle's name, age and town are from the issue's table of contents and
the Best of Creative Computing (1976) credit, as quoted in search results
(the pages themselves carry the listing and were not opened).

## Our design

- The field is 10x10, x from 0 at the west edge, y from 0 at the south
  edge. Depth Charge adds depth 0 (the surface) to 9.
- **Hurkle:** one hurkle, five guesses; each miss answers with one of
  eight compass words from the guess to it (north, north-east, east, ...).
  Halving the field on each axis finds it in at most four.
- **Mugwump:** four mugwumps on four different spots, ten guesses; each
  guess finds any mugwump standing there and gives the straight-line
  distance, to a tenth, from the guess to each one still hidden, in
  hiding order and without saying which is which.
- **Depth Charge:** one submarine in a 10x10x10 sea, five charges; each
  miss answers with the compass word as in Hurkle and whether the
  submarine is deeper, shallower or at the same depth.
- H shades every spot that still fits all the clues (for Mugwump: every
  spot whose distance from each guess matches one of the distances given).
- Win and loss counts and fewest guesses per game in the state folder.
- Our own endings: the hurkle scuttles off, the submarine surfaces and
  its crew wave a white flag.
