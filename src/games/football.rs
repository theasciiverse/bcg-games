//! Football: four quarters of American football against the computer. Call
//! a play on every down with the ball and a defence without it; the field,
//! the downs and the play-by-play follow. Rules in
//! asciiverse/specs/football.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const QUARTER_SECS: u32 = 15 * 60;
pub const EXTRA_POINT: f32 = 0.94;
const PLAY_SECS: f32 = 1.3;

const TEAMS: [&str; 2] = ["Northfield", "Granite Falls"];
const BACKS: [[&str; 3]; 2] = [
    ["Mbeki", "Sorensen", "Delgado"],
    ["Whitfield", "Nakamura", "Ortiz"],
];
const PASSERS: [&str; 2] = ["Haldane", "Castillo"];
const DEFENDERS: [[&str; 3]; 2] = [
    ["Brannigan", "Oyelaran", "Fiske"],
    ["Petrakis", "Lund", "Abara"],
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Play {
    RunInside,
    RunOutside,
    ShortPass,
    LongPass,
    Screen,
    Punt,
    FieldGoal,
}

pub const PLAYS: [Play; 7] = [
    Play::RunInside,
    Play::RunOutside,
    Play::ShortPass,
    Play::LongPass,
    Play::Screen,
    Play::Punt,
    Play::FieldGoal,
];

impl Play {
    pub fn name(self) -> &'static str {
        match self {
            Play::RunInside => "Run up the middle",
            Play::RunOutside => "Sweep to the outside",
            Play::ShortPass => "Short pass",
            Play::LongPass => "Long pass",
            Play::Screen => "Screen pass",
            Play::Punt => "Punt",
            Play::FieldGoal => "Field goal try",
        }
    }
    fn is_pass(self) -> bool {
        matches!(self, Play::ShortPass | Play::LongPass | Play::Screen)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Defence {
    Stack,
    Balanced,
    Cover,
    Blitz,
}

pub const DEFENCES: [Defence; 4] = [
    Defence::Stack,
    Defence::Balanced,
    Defence::Cover,
    Defence::Blitz,
];

impl Defence {
    pub fn name(self) -> &'static str {
        match self {
            Defence::Stack => "Stack the line (stop the run)",
            Defence::Balanced => "Balanced",
            Defence::Cover => "Drop back in coverage",
            Defence::Blitz => "Blitz the passer",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Defence::Stack => "a stacked line",
            Defence::Balanced => "a balanced defence",
            Defence::Cover => "deep coverage",
            Defence::Blitz => "the blitz",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Res {
    /// a run or a caught pass, in yards (may be a loss)
    Gain(i32),
    Incomplete,
    Sack(i32),
    /// caught by the defence this many yards past the line
    Intercepted(i32),
    /// lost on the ground at this gain
    Fumble(i32),
    /// net yards of the punt
    Punt(i32),
    FieldGoal {
        good: bool,
        dist: i32,
    },
}

pub fn field_goal_chance(dist: i32, def: Defence) -> f32 {
    let p = 1.0 - (dist - 20) as f32 * 0.022 - if def == Defence::Blitz { 0.05 } else { 0.0 };
    p.clamp(0.05, 0.98)
}

/// One play from `pos` yards out of the offence's own goal line.
pub fn resolve(rng: &mut Rng, play: Play, def: Defence, pos: i32) -> Res {
    use Defence::*;
    let breakaway = |rng: &mut Rng, p: f32| {
        if rng.chance(p) {
            rng.range(10, 45) as i32
        } else {
            0
        }
    };
    match play {
        Play::RunInside | Play::RunOutside => {
            let inside = play == Play::RunInside;
            let (mean, sd) = if inside { (3.5, 3.0) } else { (4.0, 5.0) };
            let adj = match (def, inside) {
                (Stack, true) => -2.5,
                (Stack, false) => -2.0,
                (Cover, true) => 2.0,
                (Cover, false) => 2.5,
                (Blitz, true) => {
                    if rng.chance(0.5) {
                        -1.5
                    } else {
                        3.0
                    }
                }
                (Blitz, false) => -2.0,
                _ => 0.0,
            };
            let mut y = (rng.normal() * sd + mean + adj).round() as i32;
            let bp = if inside { 0.04 } else { 0.06 } + if def == Cover { 0.04 } else { 0.0 };
            y += breakaway(rng, bp);
            if rng.chance(0.02) {
                Res::Fumble(y.min(5))
            } else {
                Res::Gain(y)
            }
        }
        Play::ShortPass | Play::LongPass | Play::Screen => {
            let (sack, comp, int, mean, sd, min) = match play {
                Play::ShortPass => (
                    if def == Blitz { 0.14 } else { 0.04 },
                    0.65 + match def {
                        Stack => 0.10,
                        Cover => -0.15,
                        Blitz => -0.05,
                        _ => 0.0,
                    },
                    if def == Cover { 0.06 } else { 0.03 },
                    7.0 + if def == Blitz { 4.0 } else { 0.0 },
                    4.0,
                    1,
                ),
                Play::LongPass => (
                    if def == Blitz { 0.18 } else { 0.06 },
                    0.38 + match def {
                        Stack => 0.10,
                        Cover => -0.15,
                        Blitz => 0.02,
                        _ => 0.0,
                    },
                    if def == Cover { 0.10 } else { 0.06 },
                    25.0,
                    8.0,
                    12,
                ),
                _ => (
                    0.02,
                    0.75 + match def {
                        Blitz => 0.10,
                        Cover => -0.05,
                        _ => 0.0,
                    },
                    0.02,
                    5.0 + match def {
                        Blitz => 6.0,
                        Stack => -2.0,
                        _ => 0.0,
                    },
                    5.0,
                    -4,
                ),
            };
            if rng.chance(sack) {
                return Res::Sack(-(rng.range(3, 9) as i32));
            }
            if rng.chance(int) {
                let air = if play == Play::LongPass {
                    rng.range(15, 45)
                } else {
                    rng.range(3, 20)
                };
                return Res::Intercepted(air as i32);
            }
            if !rng.chance(comp) {
                return Res::Incomplete;
            }
            let mut y = ((rng.normal() * sd + mean).round() as i32).max(min);
            y += breakaway(rng, 0.04);
            Res::Gain(y)
        }
        Play::Punt => {
            let kick = (rng.normal() * 6.0 + 42.0).round() as i32;
            let ret = rng.range(0, 12) as i32;
            Res::Punt((kick - ret).max(10))
        }
        Play::FieldGoal => {
            let dist = 100 - pos + 17;
            Res::FieldGoal {
                good: rng.chance(field_goal_chance(dist, def)),
                dist,
            }
        }
    }
}

/// What the computer calls with the ball, by the situation and against the
/// defence it saw last.
pub fn computer_play(rng: &mut Rng, down: u8, to_go: i32, pos: i32, seen: Option<Defence>) -> Play {
    if down == 4 {
        if pos >= 62 {
            return Play::FieldGoal;
        }
        if to_go <= 1 && pos >= 45 {
            return Play::RunInside;
        }
        return Play::Punt;
    }
    let roll = rng.unit();
    if down == 3 && to_go >= 7 {
        return if roll < 0.4 {
            Play::LongPass
        } else if roll < 0.85 {
            Play::ShortPass
        } else {
            Play::Screen
        };
    }
    let lean_pass = matches!(seen, Some(Defence::Stack));
    let lean_run = matches!(seen, Some(Defence::Cover));
    let w: [f32; 5] = [
        30.0 + if lean_run { 15.0 } else { 0.0 },
        20.0 + if lean_run { 10.0 } else { 0.0 },
        25.0 + if lean_pass { 15.0 } else { 0.0 },
        10.0 + if lean_pass { 10.0 } else { 0.0 },
        15.0 + if matches!(seen, Some(Defence::Blitz)) {
            20.0
        } else {
            0.0
        },
    ];
    let total: f32 = w.iter().sum();
    let mut r = roll * total;
    for (i, x) in w.iter().enumerate() {
        if r < *x {
            return PLAYS[i];
        }
        r -= x;
    }
    Play::RunInside
}

/// The computer's defence, by the situation and against the play it saw last.
pub fn computer_defence(rng: &mut Rng, down: u8, to_go: i32, seen: Option<Play>) -> Defence {
    if rng.chance(0.3) {
        return DEFENCES[rng.range(0, 3) as usize];
    }
    if down >= 3 && to_go >= 7 {
        return if rng.chance(0.5) {
            Defence::Cover
        } else {
            Defence::Blitz
        };
    }
    if to_go <= 2 {
        return Defence::Stack;
    }
    match seen {
        Some(Play::RunInside) | Some(Play::RunOutside) => Defence::Stack,
        Some(Play::LongPass) => Defence::Cover,
        Some(Play::ShortPass) => Defence::Blitz,
        _ => Defence::Balanced,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Us,
    Them,
}

impl Side {
    fn i(self) -> usize {
        match self {
            Side::Us => 0,
            Side::Them => 1,
        }
    }
    fn other(self) -> Side {
        match self {
            Side::Us => Side::Them,
            Side::Them => Side::Us,
        }
    }
}

#[derive(Debug)]
enum Phase {
    Pick,
    Play {
        t: f32,
        play: Play,
        def: Defence,
        res: Res,
    },
    Break,
    Over,
}

pub struct Football {
    ctx: Ctx,
    score: [u32; 2],
    quarter: u32,
    clock: u32,
    ball: Side,
    first_ball: Side,
    /// yards from the offence's own goal line
    pos: i32,
    down: u8,
    /// the line to gain, in the same yards as `pos`
    mark: i32,
    phase: Phase,
    sel: usize,
    log: Vec<(String, Color)>,
    seen_play: Option<Play>,
    seen_def: Option<Defence>,
    stats: Kv,
    quit: bool,
    show_help: bool,
    demo_clock: f32,
    rows: Vec<Rect>,
}

fn ordinal(d: u8) -> &'static str {
    match d {
        1 => "1st",
        2 => "2nd",
        3 => "3rd",
        _ => "4th",
    }
}

impl Football {
    fn new_game(&mut self) {
        self.score = [0, 0];
        self.quarter = 1;
        self.clock = QUARTER_SECS;
        self.log.clear();
        self.seen_play = None;
        self.seen_def = None;
        self.sel = 0;
        let receiver = if self.ctx.rng.chance(0.5) {
            Side::Us
        } else {
            Side::Them
        };
        self.first_ball = receiver;
        self.say(
            format!("Coin toss: {} will receive.", TEAMS[receiver.i()]),
            CYAN,
        );
        self.kickoff(receiver);
        self.phase = Phase::Pick;
        log::info!("new game, {:?} receive", receiver);
    }

    fn say(&mut self, s: String, c: Color) {
        self.log.push((s, c));
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }

    fn kickoff(&mut self, receiver: Side) {
        self.ball = receiver;
        let r = BACKS[receiver.i()][2];
        if self.ctx.rng.chance(0.5) {
            self.pos = 25;
            self.say(
                format!(
                    "Kickoff sails deep: touchback, {} at their 25.",
                    TEAMS[receiver.i()]
                ),
                DIM,
            );
        } else {
            self.pos = self.ctx.rng.range(15, 38) as i32;
            self.say(
                format!("Kickoff, {r} brings it back to the {}.", self.pos),
                DIM,
            );
        }
        self.first_down();
    }

    fn first_down(&mut self) {
        self.down = 1;
        self.mark = (self.pos + 10).min(100);
    }

    fn turnover(&mut self, spot: i32) {
        self.ball = self.ball.other();
        self.pos = (100 - spot).clamp(1, 99);
        self.first_down();
    }

    fn where_(&self) -> String {
        let p = self.pos;
        if p < 50 {
            format!("own {p}")
        } else if p == 50 {
            "midfield".into()
        } else {
            format!("{} {}", TEAMS[self.ball.other().i()], 100 - p)
        }
    }

    fn situation(&self) -> String {
        let to_go = self.mark - self.pos;
        let go = if self.mark >= 100 {
            "goal".to_string()
        } else {
            to_go.to_string()
        };
        format!("{} & {go}, {}", ordinal(self.down), self.where_())
    }

    fn choices(&self) -> Vec<String> {
        if self.ball == Side::Us {
            PLAYS
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if *p == Play::FieldGoal {
                        let d = 100 - self.pos + 17;
                        format!("{} {} ({d} yd)", i + 1, p.name())
                    } else {
                        format!("{} {}", i + 1, p.name())
                    }
                })
                .collect()
        } else {
            DEFENCES
                .iter()
                .enumerate()
                .map(|(i, d)| format!("{} {}", i + 1, d.name()))
                .collect()
        }
    }

    fn pick(&mut self, i: usize) {
        if !matches!(self.phase, Phase::Pick) || i >= self.choices().len() {
            return;
        }
        self.ctx.audio.play(Sound::Click);
        let to_go = self.mark - self.pos;
        let (play, def) = if self.ball == Side::Us {
            let play = PLAYS[i];
            let def = computer_defence(&mut self.ctx.rng, self.down, to_go, self.seen_play);
            self.seen_play = Some(play);
            (play, def)
        } else {
            let def = DEFENCES[i];
            let play = computer_play(&mut self.ctx.rng, self.down, to_go, self.pos, self.seen_def);
            self.seen_def = Some(def);
            (play, def)
        };
        let res = resolve(&mut self.ctx.rng, play, def, self.pos);
        log::info!(
            "{:?} ball at {} down {}: {play:?} vs {def:?} -> {res:?}",
            self.ball,
            self.pos,
            self.down
        );
        self.phase = Phase::Play {
            t: 0.0,
            play,
            def,
            res,
        };
    }

    /// Where the ball ends up, in absolute yards from the left goal line,
    /// for the picture.
    fn abs(&self, pos: i32) -> f32 {
        if self.ball == Side::Us {
            pos as f32
        } else {
            100.0 - pos as f32
        }
    }

    fn landing(&self, res: Res) -> i32 {
        match res {
            Res::Gain(y) | Res::Sack(y) | Res::Fumble(y) => (self.pos + y).clamp(-5, 105),
            Res::Intercepted(a) => (self.pos + a).min(99),
            Res::Punt(n) => (self.pos + n).min(105),
            Res::FieldGoal { .. } => 105,
            Res::Incomplete => self.pos,
        }
    }

    fn score_points(&mut self, side: Side, n: u32) {
        self.score[side.i()] += n;
    }

    fn finish(&mut self, play: Play, def: Defence, res: Res) {
        let side = self.ball;
        let ours = side == Side::Us;
        let (good, bad) = if ours { (GREEN, RED) } else { (RED, GREEN) };
        let team = TEAMS[side.i()];
        let back = BACKS[side.i()][self.ctx.rng.range(0, 2) as usize];
        let qb = PASSERS[side.i()];
        let tackler = DEFENDERS[side.other().i()][self.ctx.rng.range(0, 2) as usize];
        let lead = format!("{team}, {} vs {}:", play.name().to_lowercase(), def.short());
        let mut secs = self.ctx.rng.range(28, 42) as u32;
        match res {
            Res::Gain(y) | Res::Sack(y) => {
                let new = self.pos + y;
                let who = if play.is_pass() {
                    format!("{qb} to {back}")
                } else {
                    back.to_string()
                };
                if new >= 100 {
                    self.score_points(side, 6);
                    let xp = self.ctx.rng.chance(EXTRA_POINT);
                    if xp {
                        self.score_points(side, 1);
                    }
                    self.say(
                        format!(
                            "{lead} {who}, all the way! TOUCHDOWN {team}! {}",
                            if xp {
                                "The kick is good."
                            } else {
                                "The kick is wide."
                            }
                        ),
                        good,
                    );
                    self.ctx
                        .audio
                        .play(if ours { Sound::Win } else { Sound::Lose });
                    self.kickoff(side.other());
                } else if new <= 0 {
                    self.score_points(side.other(), 2);
                    self.say(
                        format!(
                            "{lead} {tackler} drops {back} in the end zone. Safety, two points!"
                        ),
                        bad,
                    );
                    self.ctx
                        .audio
                        .play(if ours { Sound::Bad } else { Sound::Good });
                    self.ball = side.other();
                    self.pos = self.ctx.rng.range(30, 45) as i32;
                    self.first_down();
                } else {
                    let text = if matches!(res, Res::Sack(_)) {
                        format!("{lead} {tackler} gets to {qb}, a sack for {} lost.", -y)
                    } else if y > 0 {
                        format!("{lead} {who} for {y}.")
                    } else if y == 0 {
                        format!("{lead} {who}, stopped for no gain.")
                    } else {
                        format!("{lead} {who}, dropped for a loss of {}.", -y)
                    };
                    self.pos = new;
                    if new >= self.mark {
                        self.say(format!("{text} First down!"), good);
                        self.ctx
                            .audio
                            .play(if ours { Sound::Good } else { Sound::Bad });
                        self.first_down();
                    } else if self.down >= 4 {
                        self.say(format!("{text} Short: turnover on downs."), bad);
                        self.ctx
                            .audio
                            .play(if ours { Sound::Bad } else { Sound::Good });
                        self.turnover(new);
                    } else {
                        self.say(text, if y >= 4 { good } else { INK });
                        self.ctx.audio.play(Sound::Tick);
                        self.down += 1;
                    }
                }
            }
            Res::Incomplete => {
                secs = self.ctx.rng.range(5, 9) as u32;
                if self.down >= 4 {
                    self.say(
                        format!("{lead} {qb}'s pass falls incomplete. Turnover on downs."),
                        bad,
                    );
                    let p = self.pos;
                    self.turnover(p);
                } else {
                    self.say(format!("{lead} {qb} throws, and it's incomplete."), DIM);
                    self.down += 1;
                }
                self.ctx.audio.play(Sound::Tick);
            }
            Res::Intercepted(air) => {
                self.say(
                    format!("{lead} {qb} throws... picked off by {tackler}!"),
                    bad,
                );
                self.ctx
                    .audio
                    .play(if ours { Sound::Bad } else { Sound::Good });
                let spot = self.pos + air;
                if spot >= 100 {
                    self.ball = side.other();
                    self.pos = 20;
                    self.first_down();
                } else {
                    let ret = self.ctx.rng.range(0, 15) as i32;
                    self.turnover((spot - ret).max(1));
                }
            }
            Res::Fumble(y) => {
                self.say(
                    format!("{lead} {back} coughs it up, {tackler} recovers!"),
                    bad,
                );
                self.ctx
                    .audio
                    .play(if ours { Sound::Bad } else { Sound::Good });
                let spot = (self.pos + y).clamp(1, 99);
                self.turnover(spot);
            }
            Res::Punt(n) => {
                let land = self.pos + n;
                self.ctx.audio.play(Sound::Tick);
                if land >= 100 {
                    self.say(
                        format!("{lead} a booming punt into the end zone. Touchback."),
                        INK,
                    );
                    self.ball = side.other();
                    self.pos = 20;
                    self.first_down();
                } else {
                    self.say(format!("{lead} the punt nets {n} yards."), INK);
                    self.turnover(land);
                }
            }
            Res::FieldGoal { good: g, dist } => {
                secs = 5;
                if g {
                    self.score_points(side, 3);
                    self.say(
                        format!("{lead} from {dist} yards... it's GOOD! Three points."),
                        good,
                    );
                    self.ctx
                        .audio
                        .play(if ours { Sound::Good } else { Sound::Bad });
                    self.kickoff(side.other());
                } else {
                    self.say(format!("{lead} from {dist} yards... no good."), bad);
                    self.ctx
                        .audio
                        .play(if ours { Sound::Bad } else { Sound::Good });
                    let spot = self.pos.min(80);
                    self.turnover(spot);
                }
            }
        }
        self.clock = self.clock.saturating_sub(secs);
        self.phase = Phase::Pick;
        if self.clock == 0 {
            self.end_quarter();
        }
    }

    fn end_quarter(&mut self) {
        let [a, b] = self.score;
        match self.quarter {
            1 | 3 => {
                self.say(
                    format!("End of the {} quarter.", ordinal(self.quarter as u8)),
                    AMBER,
                );
                self.quarter += 1;
                self.clock = QUARTER_SECS;
            }
            2 => {
                self.say(
                    format!("Halftime: {} {a}, {} {b}.", TEAMS[0], TEAMS[1]),
                    AMBER,
                );
                self.quarter = 3;
                self.clock = QUARTER_SECS;
                self.phase = Phase::Break;
                self.ctx.audio.play(Sound::Beep(440.0));
            }
            _ => {
                let (k, c, snd, line) = if a > b {
                    ("won", GREEN, Sound::Win, "Your team wins!")
                } else if a < b {
                    ("lost", RED, Sound::Lose, "A loss this time.")
                } else {
                    ("tied", AMBER, Sound::Bad, "A tie.")
                };
                self.say(
                    format!("Final: {} {a}, {} {b}. {line}", TEAMS[0], TEAMS[1]),
                    c,
                );
                self.ctx.audio.play(snd);
                self.phase = Phase::Over;
                log::info!("game over {a}-{b}");
                if !self.ctx.demo {
                    let n = self.stats.get::<u32>(k).unwrap_or(0) + 1;
                    self.stats.set(k, n);
                    if a > b {
                        let best = self.stats.get::<u32>("best").unwrap_or(0).max(a - b);
                        self.stats.set("best", best);
                    }
                    self.stats.save(Self::ID, "scores");
                }
            }
        }
    }

    fn go_on(&mut self) {
        match self.phase {
            Phase::Break => {
                self.say("Second half kickoff.".into(), CYAN);
                let r = self.first_ball.other();
                self.kickoff(r);
                self.phase = Phase::Pick;
            }
            Phase::Over => self.new_game(),
            Phase::Pick => self.pick(self.sel),
            _ => {}
        }
    }

    fn draw_field(&self, buf: &mut Buffer, r: Rect) {
        if r.width < 30 || r.height < 5 {
            return;
        }
        let ez = 4u16;
        let fw = r.width - 2 * ez;
        let x_of = |abs: f32| -> u16 {
            let x = r.x as f32 + ez as f32 + abs.clamp(-4.0, 104.0) / 100.0 * (fw as f32 - 1.0);
            (x.round() as u16).clamp(r.x, r.right() - 1)
        };
        let g1 = Color::Rgb(26, 70, 34);
        let g2 = Color::Rgb(30, 80, 38);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let bg = if x < r.x + ez {
                    Color::Rgb(36, 54, 110)
                } else if x >= r.right() - ez {
                    Color::Rgb(110, 40, 36)
                } else {
                    let yd = ((x - r.x - ez) as f32 / (fw as f32 - 1.0) * 100.0) as i32;
                    if (yd / 10) % 2 == 0 {
                        g1
                    } else {
                        g2
                    }
                };
                buf[(x, y)].set_symbol(" ").set_style(on(INK, bg));
            }
        }
        // goal lines and yard numbers
        for y in r.y..r.bottom() {
            for abs in [0.0, 100.0] {
                let bg = buf[(x_of(abs), y)].bg;
                buf[(x_of(abs), y)].set_symbol("┃").set_style(on(INK, bg));
            }
        }
        for yd in (10..=90).step_by(10) {
            let n = if yd > 50 { 100 - yd } else { yd };
            let x = x_of(yd as f32);
            for y in [r.y, r.bottom() - 1] {
                let bg = buf[(x, y)].bg;
                put(
                    buf,
                    x.saturating_sub(1),
                    y,
                    &n.to_string(),
                    on(mix(INK, g1, 0.25), bg),
                );
            }
            for y in r.y + 1..r.bottom() - 1 {
                let bg = buf[(x, y)].bg;
                buf[(x, y)]
                    .set_symbol("│")
                    .set_style(on(mix(INK, g1, 0.55), bg));
            }
        }
        let ezl = Rect::new(r.x, r.y, ez, r.height);
        let ezr = Rect::new(r.right() - ez, r.y, ez, r.height);
        for (z, name) in [(ezl, TEAMS[0]), (ezr, TEAMS[1])] {
            // the whole name down the end zone if it fits, else its first word
            let mut letters: String = name.chars().filter(|c| *c != ' ').collect();
            if letters.chars().count() > r.height as usize {
                letters = name.split(' ').next().unwrap_or(name).to_string();
            }
            let top = (r.height as usize).saturating_sub(letters.chars().count()) as u16 / 2;
            for (i, ch) in letters.chars().take(r.height as usize).enumerate() {
                let i = i as u16 + top;
                let bg = buf[(z.x + 1, z.y + i)].bg;
                buf[(z.x + 1, z.y + i)]
                    .set_symbol(&ch.to_uppercase().to_string())
                    .set_style(on(mix(INK, bg, 0.4), bg));
            }
        }
        let mid = r.y + r.height / 2;
        let los = self.abs(self.pos);
        let dir: f32 = if self.ball == Side::Us { 1.0 } else { -1.0 };
        // the line to gain
        if self.mark < 100 {
            let x = x_of(self.abs(self.mark));
            for y in r.y + 1..r.bottom() - 1 {
                let bg = buf[(x, y)].bg;
                buf[(x, y)].set_symbol("┃").set_style(on(AMBER, bg));
            }
        }
        let (t, res, play) = match &self.phase {
            Phase::Play { t, res, play, .. } => {
                ((*t / PLAY_SECS).clamp(0.0, 1.0), Some(*res), Some(*play))
            }
            _ => (0.0, None, None),
        };
        // the two lines of players
        let (oc, dc) = if self.ball == Side::Us {
            (CYAN, RED)
        } else {
            (RED, CYAN)
        };
        let push = t * 1.5 * dir;
        let rows = (r.height - 2).max(1);
        for i in 0..rows.min(7) {
            let y = r.y + 1 + (i * rows / rows.min(7));
            let ox = x_of(los - 1.0 * dir + push);
            let dx = x_of(los + 1.5 * dir + push);
            let bg = buf[(ox, y)].bg;
            buf[(ox, y)].set_symbol("●").set_style(bold(on(oc, bg)));
            let bg = buf[(dx, y)].bg;
            buf[(dx, y)].set_symbol("●").set_style(bold(on(dc, bg)));
        }
        // the ball
        let (bx, by) = match res {
            Some(res) => {
                let end = self.abs(self.landing(res));
                let end = match res {
                    Res::Incomplete => los + 12.0 * dir,
                    _ => end,
                };
                let x = los + (end - los) * t;
                let air = matches!(play, Some(Play::Punt) | Some(Play::FieldGoal))
                    || play.is_some_and(|p| p.is_pass());
                let lift = if air {
                    (t * std::f32::consts::PI).sin() * (r.height as f32 / 2.0 - 1.0)
                } else {
                    0.0
                };
                (x_of(x), (mid as f32 - lift).round() as u16)
            }
            None => (x_of(los - 3.0 * dir), mid),
        };
        let by = by.clamp(r.y, r.bottom() - 1);
        let bg = buf[(bx, by)].bg;
        buf[(bx, by)].set_symbol("◆").set_style(bold(on(AMBER, bg)));
    }
}

impl Game for Football {
    const ID: &'static str = "football";
    const TITLE: &'static str = "Football";
    const HELP: &'static str =
        "Coach Northfield against Granite Falls: four fifteen-minute quarters.
With the ball, call a play each down: runs inside and outside, short,
long and screen passes, a punt or a field goal try. Four downs to gain
ten yards. Without the ball, call a defence: stack the line against the
run, drop back against the pass, blitz the passer, or stay balanced.
1-7 or arrows and Enter to call, Tab the rules, N a new game, M sound,
Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let mut g = Football {
            ctx,
            score: [0, 0],
            quarter: 1,
            clock: QUARTER_SECS,
            ball: Side::Us,
            first_ball: Side::Us,
            pos: 25,
            down: 1,
            mark: 35,
            phase: Phase::Pick,
            sel: 0,
            log: Vec::new(),
            seen_play: None,
            seen_def: None,
            stats: Kv::load(Self::ID, "scores"),
            quit: false,
            show_help: false,
            demo_clock: 0.0,
            rows: Vec::new(),
        };
        g.new_game();
        g
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        let n = self.choices().len();
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Tab | Key::Char('?') => self.show_help = !self.show_help,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Char('n') => self.new_game(),
            Key::Up => {
                self.sel = (self.sel + n - 1) % n;
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Down => {
                self.sel = (self.sel + 1) % n;
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if i < n {
                    self.sel = i;
                    self.pick(i);
                }
            }
            Key::Enter | Key::Char(' ') => self.go_on(),
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if self.ctx.demo {
            return;
        }
        if let Some(i) = self.rows.iter().position(|r| hit(r, m.x, m.y)) {
            match m.kind {
                MouseKind::Move => self.sel = i,
                MouseKind::Click => {
                    self.sel = i;
                    self.pick(i);
                }
                _ => {}
            }
        } else if m.kind == MouseKind::Click && !matches!(self.phase, Phase::Pick) {
            self.go_on();
        }
    }

    fn tick(&mut self, dt: f32) {
        if let Phase::Play { t, play, def, res } = &mut self.phase {
            *t += dt;
            if *t >= PLAY_SECS {
                let (p, d, r) = (*play, *def, *res);
                self.finish(p, d, r);
            }
            return;
        }
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = match self.phase {
            Phase::Pick => 0.6,
            Phase::Break => 2.0,
            _ => 4.0,
        };
        if self.demo_clock < wait {
            return;
        }
        self.demo_clock = 0.0;
        match self.phase {
            Phase::Pick => {
                let to_go = self.mark - self.pos;
                let i = if self.ball == Side::Us {
                    let p =
                        computer_play(&mut self.ctx.rng, self.down, to_go, self.pos, self.seen_def);
                    PLAYS.iter().position(|x| *x == p).unwrap_or(0)
                } else {
                    let d = computer_defence(&mut self.ctx.rng, self.down, to_go, self.seen_play);
                    DEFENCES.iter().position(|x| *x == d).unwrap_or(0)
                };
                self.sel = i;
                self.pick(i);
            }
            _ => self.go_on(),
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "q{} clock {} score {:?} ball {:?} pos {} down {} mark {} phase {:?}",
            self.quarter,
            self.clock,
            self.score,
            self.ball,
            self.pos,
            self.down,
            self.mark,
            self.phase
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.rows.clear();
        let q = if self.quarter <= 4 {
            format!("{} qtr", ordinal(self.quarter as u8))
        } else {
            "final".into()
        };
        let status = format!("{q} {}:{:02}", self.clock / 60, self.clock % 60);
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("1-7", "call"),
                ("Enter", "go"),
                ("Tab", "rules"),
                ("N", "new game"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 80, 22) {
            return;
        }
        let h = inner.height;
        let bottom_h = 10 + (h - 22) / 2;
        let field_h = h - 2 - bottom_h;

        // scoreboard and the down
        let y = inner.y;
        let left = format!("{} {}", TEAMS[0].to_uppercase(), self.score[0]);
        let right = format!("{} {}", self.score[1], TEAMS[1].to_uppercase());
        let mid = format!("   {}   ", self.situation());
        let total = (left.len() + mid.chars().count() + right.len()) as u16;
        let x0 = inner.x + inner.width.saturating_sub(total) / 2;
        let x = put(buf, x0, y, &left, bold(st(CYAN)));
        let x = put(
            buf,
            x,
            y,
            &mid,
            st(if self.ball == Side::Us { CYAN } else { RED }),
        );
        put(buf, x, y, &right, bold(st(RED)));
        let arrow = if self.ball == Side::Us {
            "▶ ball"
        } else {
            "ball ◀"
        };
        let ax = if self.ball == Side::Us {
            inner.x + 1
        } else {
            inner.right() - 7
        };
        put(buf, ax, y, arrow, st(AMBER));

        self.draw_field(
            buf,
            Rect::new(inner.x + 1, inner.y + 2, inner.width - 2, field_h),
        );

        let by = inner.y + 2 + field_h;
        let lw = 38;
        let leftp = Rect::new(inner.x, by, lw, bottom_h);
        let rightp = Rect::new(inner.x + lw, by, inner.width - lw, bottom_h);
        let title = if matches!(self.phase, Phase::Over) {
            "Full time"
        } else if matches!(self.phase, Phase::Break) {
            "Halftime"
        } else if self.ball == Side::Us {
            "Your ball: call a play"
        } else {
            "Their ball: call a defence"
        };
        let p = panel(buf, leftp, title, AMBER);
        match self.phase {
            Phase::Pick | Phase::Play { .. } => {
                let items = self.choices();
                let mr = Rect::new(p.x.saturating_sub(1), p.y, p.width + 1, items.len() as u16);
                let sel = if matches!(self.phase, Phase::Pick) {
                    self.sel
                } else {
                    usize::MAX
                };
                self.rows = menu(buf, mr, &items, sel);
            }
            Phase::Break => {
                para(buf, p, "Halftime. Enter for the second half.", st(AMBER));
            }
            Phase::Over => {
                let won = self.stats.get::<u32>("won").unwrap_or(0);
                let lost = self.stats.get::<u32>("lost").unwrap_or(0);
                let tied = self.stats.get::<u32>("tied").unwrap_or(0);
                let best = self.stats.get::<u32>("best").unwrap_or(0);
                para(
                    buf,
                    p,
                    &format!("Final gun. Enter for a new game.\n\nRecord: {won} won, {lost} lost, {tied} tied{}",
                        if best > 0 { format!("\nBiggest win: {best} points") } else { String::new() }),
                    st(INK),
                );
            }
        }
        if self.show_help {
            let q = panel(buf, rightp, "How it plays", CYAN);
            para(buf, q, "Four downs to gain ten yards. Runs gain steadily and beat coverage; the stacked line stops them. Short passes are safe unless the defence drops back; long passes gain big but get picked off and sacked. The screen pass punishes a blitz. Punt on 4th down, or kick a field goal: shorter is surer. Touchdown 6, the kick after 1, field goal 3, safety 2.", st(INK));
        } else {
            let q = panel(buf, rightp, "Play by play", CYAN);
            let rows = q.height as usize;
            let mut lines: Vec<(String, Color)> = Vec::new();
            for (s, c) in &self.log {
                for l in wrap(s, q.width as usize) {
                    lines.push((l, *c));
                }
            }
            let skip = lines.len().saturating_sub(rows);
            for (i, (l, c)) in lines.iter().skip(skip).enumerate() {
                put_max(buf, q.x, q.y + i as u16, l, st(*c), q.width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_goals_get_harder_with_distance() {
        assert!(
            field_goal_chance(25, Defence::Balanced) > field_goal_chance(50, Defence::Balanced)
        );
        assert!(field_goal_chance(70, Defence::Balanced) <= 0.1);
    }

    #[test]
    fn runs_do_better_against_coverage_than_a_stacked_line() {
        let mut rng = Rng::new(3);
        let avg = |rng: &mut Rng, d| -> f32 {
            (0..3000)
                .map(|_| match resolve(rng, Play::RunInside, d, 30) {
                    Res::Gain(y) | Res::Fumble(y) => y as f32,
                    _ => 0.0,
                })
                .sum::<f32>()
                / 3000.0
        };
        assert!(avg(&mut rng, Defence::Cover) > avg(&mut rng, Defence::Stack) + 2.0);
    }

    #[test]
    fn the_computer_kicks_on_fourth_down() {
        let mut rng = Rng::new(1);
        assert_eq!(computer_play(&mut rng, 4, 8, 20, None), Play::Punt);
        assert_eq!(computer_play(&mut rng, 4, 8, 75, None), Play::FieldGoal);
    }

    #[test]
    fn long_passes_are_never_short_gains() {
        let mut rng = Rng::new(11);
        for _ in 0..1000 {
            if let Res::Gain(y) = resolve(&mut rng, Play::LongPass, Defence::Balanced, 30) {
                assert!(y >= 12);
            }
        }
    }
}
