# Football: rules spec

Written 2026-11-14 from the idea of the book's game, without its listing or black-box play.

## Our design

- You coach Northfield against the computer's Granite Falls in American
  football: four quarters of fifteen minutes. A coin toss decides who
  receives; the other team receives to start the second half. A game can
  end tied.
- Each play uses 28 to 42 seconds; an incomplete pass 5 to 9; a field
  goal try 5. Four downs to gain ten yards.
- Kickoffs: half are touchbacks to the 25, the rest are returned to the
  15 to 38.
- With the ball you call one of seven plays; the defence calls one of
  four (stack the line, balanced, drop back in coverage, blitz):
  - Run up the middle: about 3.5 yards; -2.5 against a stacked line, +2
    against coverage, against a blitz either stuffed (-1.5) or through a
    gap (+3).
  - Sweep: about 4 yards with a wider spread; -2 against a stacked line
    or a blitz, +2.5 against coverage. Runs break away for 10 to 45 more
    yards now and then (more often against coverage) and fumble 2% of
    the time.
  - Short pass: 65% complete, about 7 yards; better against a stacked
    line, worse against coverage; a blitz sacks 14% of the time but a
    completion beats it for 4 more yards. Intercepted 3% (6% against
    coverage).
  - Long pass: 38% complete, at least 12 and about 25 yards; sacked 6%
    (18% against a blitz), intercepted 6% (10% against coverage).
  - Screen pass: 75% complete, about 5 yards, 6 more against a blitz,
    2 fewer against a stacked line.
  - Punt: about 42 yards less a return of up to 12; into the end zone is
    a touchback to the 20.
  - Field goal: from the line of scrimmage plus 17 yards; 98% close in,
    2.2% less for every yard past 20, 5% less against a blitz. A miss
    gives the ball to the other side at the spot (no closer than its 20).
- Touchdown 6 and a kick after (94%), field goal 3, safety 2 (then the
  ball goes to the other side between its 30 and 45). An interception in
  the end zone is a touchback; otherwise it is returned up to 15 yards.
- The computer on offence: on fourth down it kicks a field goal inside
  the other side's 38, goes for it on fourth and one from its own 45 on, and
  punts otherwise; on third and long it passes; otherwise it mixes runs
  and passes, leaning against your last defence. On defence it covers or
  blitzes on third and long, stacks the line on short yardage, and
  otherwise leans against your last play; three times in ten it calls
  anything.
- The screen: a 100-yard field with end zones, both lines, the ball in
  flight and the line to gain, the scoreboard with down and distance,
  the call menu and a play-by-play. Wins, losses, ties and the biggest
  winning margin are kept in the state folder.
