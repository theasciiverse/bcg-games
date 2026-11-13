# Civil War: rules spec

Written 2026-11-13 from the book's description (p. 46) and from playing
the original as a black box.

## Facts from the description and from play

- Fourteen battles of the American Civil War with facts and figures from
  the real battles. The player commands the South; the computer, or a
  second player, the North.
- Before each battle: men, money and inflation for each side (observed:
  money about $4.50 a man; Southern inflation rising from 25% by 2% a
  battle, Northern falling from 10% by 2%).
- Spend money on food, salaries and ammunition; morale follows.
- Choose a strategy: four for defence (artillery, fortify against a
  frontal attack, fortify against flanking, fall back) and four for attack
  (artillery, frontal, flanking, encirclement).
- After each battle, casualties and desertions are compared with the real
  battle's, and a winner is declared.

## Our design

- Our own list of fourteen battles and our own descriptions, with men
  engaged and casualties rounded from standard histories (approximate
  figures; who attacked and who won as history records it).
- Our own model: each side's losses are history's, scaled by a matchup
  table of strategies, the enemy's ammunition and the side's morale; poor
  morale brings desertion; the side losing the smaller share of its army
  carries the field; falling back always gives it up.
- The computer general spends evenly and leans against the South's last
  plan.
- The screen: briefing, orders, and the whole war's table with history's
  results beside yours.
