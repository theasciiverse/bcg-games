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

- Our own list of fourteen battles and our own descriptions, with the
  figures below (who attacked and who won as history records it).
- Our own model: each side's losses are history's, scaled by a matchup
  table of strategies, the enemy's ammunition and the side's morale; poor
  morale brings desertion; the side losing the smaller share of its army
  carries the field; falling back always gives it up.
- The computer general spends evenly and leans against the South's last
  plan.
- The screen: briefing, orders, and the whole war's table with history's
  results beside yours.

## Battle figures and sources

Men engaged (or present for duty) and casualties (killed, wounded,
missing and captured), South and North, and the result, from each
battle's Wikipedia infobox as read on 2026-11-13. Where the infobox gives
a range, the game uses its middle, marked "mid".

| Battle | Men, South / North | Casualties, South / North | Result | Source |
|---|---|---|---|---|
| First Bull Run, 21 Jul 1861 | c. 18,000 / c. 18,000 engaged | 1,982 / 2,708 | Confederate victory | en.wikipedia.org/wiki/First_Battle_of_Bull_Run |
| Shiloh, 6-7 Apr 1862 | 44,699 / 66,812 | 10,699 / 13,047 | Union victory | en.wikipedia.org/wiki/Battle_of_Shiloh |
| Seven Days, 25 Jun - 1 Jul 1862 | 112,200 / 101,434 present for duty | 20,204 / 15,855 | Confederate victory | en.wikipedia.org/wiki/Seven_Days_Battles |
| Second Bull Run, 28-30 Aug 1862 | 50,000 / 62,000 engaged | 7,298 / 14,462 | Confederate victory | en.wikipedia.org/wiki/Second_Battle_of_Bull_Run |
| Antietam, 17 Sep 1862 | 30,646 / 53,632 engaged | 10,316 / 12,410 | Union victory | en.wikipedia.org/wiki/Battle_of_Antietam |
| Fredericksburg, 11-15 Dec 1862 | 72,500 / 114,000 engaged | 5,377 / 12,653 | Confederate victory | en.wikipedia.org/wiki/Battle_of_Fredericksburg |
| Stones River, 31 Dec 1862 - 2 Jan 1863 | 35,000 / 43,400 | 11,739 / 13,906 | tactically inconclusive, strategic Union victory (game: North) | en.wikipedia.org/wiki/Battle_of_Stones_River |
| Chancellorsville, 30 Apr - 6 May 1863 | 60,298 / 133,868 (campaign) | 12,764 / 17,287 | Confederate victory | en.wikipedia.org/wiki/Battle_of_Chancellorsville |
| Vicksburg, 18 May - 4 Jul 1863 | c. 33,000 / c. 77,000 | 32,697 / 4,835 | Union victory | en.wikipedia.org/wiki/Siege_of_Vicksburg |
| Gettysburg, 1-3 Jul 1863 | 65,000-75,000 (mid 70,000) / 93,500-104,256 (mid 98,900) | 23,000-28,000 (mid 25,500) / 23,049 | Union victory | en.wikipedia.org/wiki/Battle_of_Gettysburg |
| Chickamauga, 18-20 Sep 1863 | c. 65,000 / c. 60,000 | 18,454 / 16,170 | Confederate victory | en.wikipedia.org/wiki/Battle_of_Chickamauga |
| Chattanooga, 21 Sep - 25 Nov 1863 (campaign) | 48,948 / 72,533 | 6,667 (Confederate report) / 5,824 | Union victory | en.wikipedia.org/wiki/Chattanooga_campaign |
| Spotsylvania, 8-21 May 1864 | 52,000-63,000 (mid 57,500) / 108,000-115,400 (mid 111,700) | 12,687 / 18,399 | inconclusive (game: drawn) | en.wikipedia.org/wiki/Battle_of_Spotsylvania_Court_House |
| Atlanta, 22 Jul 1864 | 40,438 / 34,863 | 5,000-5,500 (mid 5,250) / 3,722 | Union victory | en.wikipedia.org/wiki/Battle_of_Atlanta |

Chattanooga has no separate figures for the fighting of 23-25 November,
so the campaign's are used, and the game shows the campaign's dates.
