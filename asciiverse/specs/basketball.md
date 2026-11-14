# Basketball: rules spec

Written 2026-11-14 from the idea of the book's game, without its listing or black-box play.

## Our design

- You coach Lakeside against the computer's Cedar State: two halves of
  twenty minutes, five minutes of overtime at a time while the score is
  level. A tip-off decides the first ball; the other team has it to start
  the second half.
- Each possession uses 14 to 30 seconds of the clock (a missed shot kept
  alive by an offensive rebound, 10 to 20; working the ball around, 6 to
  12). A shot as the clock runs out still counts.
- With the ball you call a play, without it a defence:
  - Long jump shot (3 points, 34%), short jump shot (2, 46%), drive for a
    layup (2, 58%), set shot (2, 42%), or work it around: no shot, a
    chance of a steal, and the next shot gets 10% more.
  - Full-court press (steals 14%, fouls 14%, layups +6%, other shots -2%),
    man-to-man (steals 7%, fouls 11%, jump shots -7%, set shots -3%,
    layups -2%), zone (steals 5%, fouls 8%, layups -13%, short jumpers
    -3%, long shots +5%), sag off and rest (steals 2%, fouls 5%, every
    shot +7%).
- A shot: first a steal (60% of the defence's steal rate; working it
  around, 130% plus 3%), then a shooting foul (the defence's foul rate:
  as many free throws as the shot was worth, each 70%), then the shot.
  A miss is kept alive by the offence 28% of the time; otherwise the
  ball changes hands.
- The computer leans against your last call: against the press it
  drives, against a zone it shoots from outside; on defence it plays
  man-to-man after jump shots, a zone after layups and presses passers;
  three times in ten it calls anything.
- The screen: a court with both teams in their formations and the ball
  in flight, the scoreboard, the call menu and a play-by-play. Wins,
  losses and the biggest winning margin are kept in the state folder.
