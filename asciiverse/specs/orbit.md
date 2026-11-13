# Orbit: rules spec

Written 2026-11-13 from the book's description (p. 124) and from playing
the original as a black box.

## Facts from the description and from play

- An invisible enemy ship circles the planet counterclockwise at a fixed
  distance of 10,000 to 30,000 miles, once every 12 to 36 hours.
- Seven hours to destroy it; one bomb an hour, aimed by an angle (0 to 360
  degrees) and a distance (100 to 300, in hundreds of miles).
- After each bomb you are told how far from the ship it burst (the law of
  cosines, as the description explains). A burst within 5,000 miles
  destroys it.

## Our design

- The enemy is our own: a cloaked raider.
- The plot is drawn from above in Braille: the planet, rings at 100, 200
  and 300, each burst with the circle of places the raider could have been,
  and the aim with its kill radius. Aim with the mouse or by numbers.
- When the hunt ends, the raider's true orbit and positions are shown.
