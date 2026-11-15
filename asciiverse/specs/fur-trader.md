# Fur Trader: rules spec

Written 2026-11-14 from the idea of the book's game, without its listing or black-box play.

## The original

Dan G. Bachor of the Department of Psychology at the University of
Calgary wrote *Fur Trader*, printed in *101 BASIC Computer Games* (DEC,
1973) and *BASIC Computer Games* (1978): a fur-trading expedition based
around Lake Ontario, dividing the year's furs and choosing a fort to sell
them at, each with its own prices and risks.

Our posts are real places of the time: Fort Frontenac, built in 1673 by
Governor Frontenac where Lake Ontario empties into the St. Lawrence
(today's Kingston), to hold the Great Lakes fur trade; Montreal, the
trade's centre; and Albany on the Hudson, the Dutch Fort Orange, English
since 1664. Beaver was the key pelt, for hat felt; voyageurs were the
licensed canoe men who carried the trade from Montreal.

Sources: obsessivegamingchronology.blogspot.com/2026/03/fur-trader.html;
en.wikipedia.org/wiki/Fort_Frontenac; en.wikipedia.org/wiki/Fort_Orange_(New_Netherland);
en.wikipedia.org/wiki/North_American_fur_trade; en.wikipedia.org/wiki/Voyageurs
(all read 2026-11-14).

## Our design

You are an independent fur trader on the north shore of Lake Ontario in
the 1700s, for eight seasons. Money is in livres.

- **Start:** 600 livres.
- **Autumn, outfitting:** buy bales of trade goods (cloth, kettles,
  knives, beads) and hire 2 to 12 voyageurs at 25 livres each for the
  season. Each voyageur carries 3 bales. The price of a bale is set by the
  post you last sold at: 50 at Fort Frontenac and Montreal, 45 at
  Albany (Montreal for the first season).
- **Winter, trading:** you winter in the upper country and trade your
  goods with Indigenous trappers and traders (Anishinaabe, Odawa, Wendat
  or Haudenosaunee), who supply the furs. Each bale brings 10 to 16
  pelts; a hard winter brings 30% fewer (15%), a good one 20% more (15%).
  The pelts divide about half beaver, a fifth marten, an eighth fox, a
  tenth otter and the rest mink.
- **Spring, selling:** choose one of three posts:
  - **Fort Frontenac**, along the lake to its eastern end: pays 95% of the
    base prices; no risk of its own.
  - **Montreal**, down the St. Lawrence and its rapids: pays 110%; 30%
    chance a canoe takes water and 10% to 30% of the pelts are lost.
  - **Albany**, by Oswego, the Great Carrying Place and the Mohawk River
    to the Hudson: pays 115% and sells the cheapest goods; 20% chance a
    Mohawk guide shows a drier path, otherwise 30% chance rain on the
    carry spoils 15% to 30% of the pelts; 25% chance a rival trader got
    there first and prices are 15% lower.
- **On any route:** 12% chance of a storm on the lake (5 livres more a
  voyageur in wages), 4% chance a canoe overturns in a squall (the crew
  are safe, a third of the pelts are lost).
- **Prices:** base livres a pelt are beaver 6, otter 9, marten 4, fox 5,
  mink 3, times the post's rate and a 0.9 to 1.1 swing for each kind.
  Beaver also follows the market in Europe, 0.8 to 1.25, announced when
  it is high or low.
- (0.7.0: Fort Frontenac paid 85% with goods at 55, which left almost
  nothing after wages, and Albany 120% with goods at 40, which made it
  the only sensible choice. Now Frontenac is a small, safe profit and
  the two far posts are close to even.)
- **The end:** after eight seasons, or earlier when the purse cannot pay a
  crew of two and a bale. The score is the fortune; the best is kept.

## Care with the period

The Indigenous nations of the region appear as the trappers, traders and
guides on whom the trade depended, never as raiders or villains. The
hazards are the country's (rapids, carries, storms, winters) and the
market's (rivals, prices in Europe). Place names are real places of the
time: Fort Frontenac at the lake's outlet, Montreal, Oswego, the Great
Carrying Place, the Mohawk River and Albany on the Hudson.

## The screen

A map of the lake and the three ways to market with the chosen route and
the canoe moving along it, a season log, a ledger, the season's action
(outfit, winter, where to sell, the journey, the sale with its price
table) and the fortune by season. The demo trader outfits as fully as the
purse allows and picks a post at random, leaning to Montreal and Albany.
