# Boxing: rules spec

Written 2026-11-14 from the idea of the book's game, without its listing or black-box play.

## Our design

- An amateur bout of three rounds against a computer boxer (one of five
  named opponents). Each round is eight exchanges.
- Before the bout you choose your best punch and your weak spot (a
  different punch). The opponent has both too, chosen at random and
  hidden; you learn its best punch when one lands with that extra force,
  and its weak spot when you land on it. The computer learns yours the
  same way.
- Each exchange both boxers choose at once: jab, cross, hook, uppercut,
  or cover up.
  - A punch lands with 62% (jab), 48% (cross), 40% (hook) or 32%
    (uppercut); +15% if it is the thrower's best punch, +10% if it is
    the other's weak spot, less than half as often against a boxer
    covering up, and less when the thrower is tired (60% to 100% of the
    chance, by wind).
  - A landed punch scores 1 (jab), 2 (cross) or 3 (hook, uppercut), one
    more for the best punch, all of it doubled on the weak spot. The
    score counts for the judges and as punishment taken.
  - Punching costs wind (4% to 10%); covering up brings back 12%; the
    break between rounds 35%.
- Knockdowns: when a boxer has taken 12 in a round, or now and then
  from a single blow worth 4 or more. The boxer beats the count with 92%
  less 18% for each earlier knockdown and up to 25% for being tired;
  three knockdowns in one round and the referee stops it.
- Three judges score each round 10-9 to the boxer who landed more (an
  even round 10-10), with a little of their own judgement, and a point
  off for each knockdown. After three rounds the cards decide; the
  judges can split, and the bout can be a draw.
- The computer covers up when tired, goes to your weak spot once it has
  found it, and favours its best punch.
- No injuries are described: knockdowns, counts, the referee waving it
  off, and a sporting embrace at the end. Wins (and early wins), losses
  and draws are kept in the state folder.
