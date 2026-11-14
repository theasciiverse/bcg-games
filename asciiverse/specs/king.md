# King: rules spec

Written 2026-11-14 from the idea of the book's game, without its listing or black-box play.

## Our design

You are premier of Pellora, a small island nation of our own invention,
for an eight-year term. All money is in crowns, all land in hectares.

- **Start:** about 500 citizens (500 plus or minus 12), no foreign
  workers, about 60,000 crowns (plus or minus 4,000), 1,000 ha of forest
  and 1,000 ha of farmland, no industry.
- **Each year the island offers:** a price for land from foreign industry
  (90 to 120 crowns a hectare) and a cost of planting (10 to 15 crowns a
  hectare), both drawn fresh each year.
- **Four decisions a year:**
  1. Land to sell to industry. Half comes from farmland and half from
     forest (farmland takes the odd hectare; when one runs out the other
     gives the rest). It pays at once.
  2. Crowns for the people's food: 80 a citizen feeds them for the year.
  3. Farmland to plant: at most the farmland left after the sale, and at
     most 2 ha a citizen.
  4. Crowns against pollution: every 10 crowns clears one industrial
     hectare's worth of pollution for the year.
  The four together may not spend more than the treasury plus the sale.
- **Pollution** is the industrial hectares not cleaned up, divided by
  1,000, capped at 0.8.
- **The year:**
  - Citizens not fed go hungry: a quarter of them are lost to hunger and
    half of them leave the island.
  - Pollution brings illness (4% of citizens times the pollution are
    lost) and departures (5% of citizens times the pollution leave).
  - When nobody went hungry and pollution is under 0.2, the population
    grows by 2% to 4%.
  - Harvest: each planted hectare yields 18 to 32 crowns, less half the
    pollution's share.
  - Tourism: each hectare of forest brings 10 to 16 crowns, less the
    pollution's share.
  - Every hectare sold brings half a foreign worker. Industry pays a levy
    of 15 crowns a hectare a year, and each worker 40 crowns.
- **Ending early:**
  - more than 8% of the citizens lost (hunger and illness) in one year:
    the assembly passes a vote of no confidence;
  - more than a tenth went hungry while the treasury still holds two
    years' food money: the assembly removes you for hoarding;
  - the citizens fall below three fifths of the starting number: you
    resign from an emptied island.
- **Serving the full term:** the election is scored out of 100: citizens
  kept (50, full marks at 120% of the start), forest kept (20), clean air
  (20, by the last year's pollution), money in hand (10, full at 30,000
  crowns). 85 or more is a landslide, 70 re-elected, 55 narrowly
  re-elected, below that defeated at the polls.
- Losses are told plainly and soberly ("lost to hunger"), and the ways out
  of office are votes and resignation, nothing more.

## The screen

A report of the year, four order fields with hints (the price, what
feeds everyone, what cleans everything), a map of the island showing
forest, farms and factories in their current shares under a haze that
thickens with pollution, and a small chart of people, money and forest by
year. A record of terms served and the best score is kept in the state
folder. The demo premier feeds everyone, plants all it can, cleans most
of the pollution, and sells land when the treasury runs low, and now and
then a little more.
