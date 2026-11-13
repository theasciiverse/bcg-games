# Splat: rules spec

Written 2026-11-13 from the book's description (p. 151) and from playing
the original as a black box.

## Facts from the description and from play

- A parachute jump: open the chute as late as possible without hitting the
  ground.
- You may be on any planet out to Neptune, the Moon or the Sun; you are
  told the height, the acceleration of gravity and your terminal velocity,
  each "plus or minus 5%".
- Observed: heights of a few thousand feet, terminal velocities up to about
  1,000 mph.
- You set the free-fall time; progress is reported at eight intervals.
- Good jumps are compared with earlier good jumps.

## Our design

- Gravity values are today's figures for each world (ours, not the
  program's).
- Heights 1,000 to 9,500 ft; terminal velocity 60 to 1,000 mph; the true
  values differ from those told by up to 5%.
- Fall: accelerate at g up to terminal velocity, then fall at it.
- Ranking against earlier good jumps by the height the chute opened at,
  kept in the state folder.
