# Hammurabi: rules spec

Written 2026-11-13 from the book's description (*BASIC Computer Games*,
1978, p. 78) and from playing the original as a black box under PC-BASIC.

## Facts from the description and from play

- You rule a Sumerian city for ten years. It starts with 100 people,
  1,000 acres and grain in store.
- After the first, modest year (observed: 5 newcomers, 3 bushels an acre,
  200 bushels lost to rats) the stores hold 2,800 bushels.
- Land trades with neighbouring cities at 17 to 26 bushels an acre, a new
  price each year.
- Each year you decide: land to buy or sell, grain to feed the people,
  acres to plant.
- Observed in play: a person eats 20 bushels a year; a person can work 10
  acres; seed is 1 bushel for every 2 acres planted.
- Observed in play: harvests ran 1 to 5 bushels an acre; rats ate part of
  the store in some years; a few newcomers (1 to 10) arrived in years
  without starvation; plague (half the city dies) struck in about 11% of
  135 observed years.
- Let more than about 45% of the people starve in one year and the ruler
  is thrown out at once.
- After ten years the reign is judged by the average share who starved
  each year and by the land per person.

## Our design (our own numbers where play only showed ranges)

- Harvest: a whole number from 1 to 5 bushels an acre planted.
- Rats: a 40% chance each year of losing 5% to 25% of the store.
- Newcomers, only in years with no starvation: (yield x (20 x acres +
  store) / people / 100) / 3 + 1 + a random 0 to 3.
- Plague: 15% a year, halving the people.
- Judgement: average starved > 33% or under 7 acres a person: hated;
  > 10% or under 9: harsh; > 3% or under 10: fair; otherwise wise.
- The screen: a written yearly report, a bar chart of people, grain and
  land over the years, and the year's three orders with a live check of
  what they leave in store.
