# Lunar Descent: rules spec

Written 2026-11-13 from the book's description of LUNAR (p. 106) and from
playing LUNAR as a black box under PC-BASIC.

## Facts from the description and from play

- A landing craft starts 120 miles above the Moon, falling at 3,600 mph,
  with 16,500 lb of fuel; the description gives the capsule as 32,500 lb.
- You set the burn rate of the retro rocket, in pounds of fuel a second,
  every 10 seconds: 0 is free fall, 200 is full burn; the engine does not
  light between 1 and 7.
- Observed: in free fall the speed grows by 36 mph every 10 seconds
  (lunar gravity, 1.6 m/s2) and the craft hits at about 4,000 mph after
  about 113 seconds.
- The trap: braking too early empties the tanks high above the ground.

## Our design

- Physics: our own integration in small steps with the rocket equation;
  dry mass 16,000 lb plus 16,500 lb of fuel; exhaust speed 1.8 miles a
  second (a real descent engine's is about 1.9).
- Two ways to fly: live, with time running at a chosen speed and the burn
  changed at any moment, or turns, stopping every 10 seconds for orders as
  the original did.
- Landing grades by touchdown speed: up to 1.2 mph perfect, 10 good, 22
  hard (crew fine, craft damaged), 60 wrecked, beyond that a crater.
- The screen: a descent view on a square-root height scale, instruments
  with fuel and burn bars and a projection of the touchdown speed if the
  burn is held, and a log every 10 seconds.
