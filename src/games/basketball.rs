//! Basketball: coach a college team through two halves against the
//! computer, calling a shot or a play when you have the ball and a defence
//! when they do. Rules in asciiverse/specs/basketball.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const HALF_SECS: u32 = 20 * 60;
pub const OT_SECS: u32 = 5 * 60;
pub const FREE_THROW: f32 = 0.70;
pub const OFFENSIVE_REBOUND: f32 = 0.28;
pub const OPEN_LOOK: f32 = 0.10;

const TEAMS: [&str; 2] = ["Lakeside", "Cedar State"];
const ROSTERS: [[&str; 5]; 2] = [
    ["Okafor", "Lindqvist", "Ramos", "Bell", "Tanaka"],
    ["Haugen", "Price", "Moreau", "Adeyemi", "Kowalski"],
];
const PLAY_SECS: f32 = 1.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shot {
    Long,
    Short,
    Layup,
    Set,
    Pass,
}

pub const SHOTS: [Shot; 5] = [Shot::Long, Shot::Short, Shot::Layup, Shot::Set, Shot::Pass];

impl Shot {
    pub fn name(self) -> &'static str {
        match self {
            Shot::Long => "Long jump shot",
            Shot::Short => "Short jump shot",
            Shot::Layup => "Drive for a layup",
            Shot::Set => "Set shot",
            Shot::Pass => "Work it around",
        }
    }

    pub fn points(self) -> u32 {
        match self {
            Shot::Long => 3,
            Shot::Pass => 0,
            _ => 2,
        }
    }

    pub fn base(self) -> f32 {
        match self {
            Shot::Long => 0.36,
            Shot::Short => 0.49,
            Shot::Layup => 0.60,
            Shot::Set => 0.45,
            Shot::Pass => 0.0,
        }
    }

    /// how far out it starts, as a share of the half court
    fn range(self) -> f32 {
        match self {
            Shot::Long => 0.42,
            Shot::Short => 0.24,
            Shot::Layup => 0.05,
            Shot::Set => 0.30,
            Shot::Pass => 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Defence {
    Press,
    Man,
    Zone,
    Sag,
}

pub const DEFENCES: [Defence; 4] = [Defence::Press, Defence::Man, Defence::Zone, Defence::Sag];

impl Defence {
    pub fn name(self) -> &'static str {
        match self {
            Defence::Press => "Full-court press",
            Defence::Man => "Man-to-man",
            Defence::Zone => "Zone",
            Defence::Sag => "Sag off and rest",
        }
    }

    /// what the defence adds to (or takes from) a shot's chance
    pub fn shot_mod(self, s: Shot) -> f32 {
        match (self, s) {
            (Defence::Press, Shot::Layup) => 0.06,
            (Defence::Press, _) => -0.02,
            (Defence::Man, Shot::Long) | (Defence::Man, Shot::Short) => -0.07,
            (Defence::Man, Shot::Set) => -0.03,
            (Defence::Man, Shot::Layup) => -0.02,
            (Defence::Zone, Shot::Long) => 0.05,
            (Defence::Zone, Shot::Short) => -0.03,
            (Defence::Zone, Shot::Layup) => -0.13,
            (Defence::Sag, _) => 0.07,
            _ => 0.0,
        }
    }

    pub fn steal(self) -> f32 {
        match self {
            Defence::Press => 0.14,
            Defence::Man => 0.07,
            Defence::Zone => 0.05,
            Defence::Sag => 0.02,
        }
    }

    pub fn foul(self) -> f32 {
        match self {
            Defence::Press => 0.14,
            Defence::Man => 0.11,
            Defence::Zone => 0.08,
            Defence::Sag => 0.05,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Stolen,
    /// the pass play kept the ball and found an open look
    Worked,
    Fouled {
        made: u32,
        of: u32,
    },
    Made {
        points: u32,
    },
    Missed {
        offensive_rebound: bool,
    },
}

pub fn chance(shot: Shot, def: Defence, open: bool) -> f32 {
    (shot.base() + def.shot_mod(shot) + if open { OPEN_LOOK } else { 0.0 }).clamp(0.05, 0.9)
}

/// One possession's play, called by the offence against the defence.
pub fn resolve(rng: &mut Rng, shot: Shot, def: Defence, open: bool) -> Outcome {
    if shot == Shot::Pass {
        return if rng.chance(def.steal() * 1.3 + 0.03) {
            Outcome::Stolen
        } else {
            Outcome::Worked
        };
    }
    if rng.chance(def.steal() * 0.6) {
        return Outcome::Stolen;
    }
    if rng.chance(def.foul()) {
        let of = shot.points();
        let made = (0..of).filter(|_| rng.chance(FREE_THROW)).count() as u32;
        return Outcome::Fouled { made, of };
    }
    if rng.chance(chance(shot, def, open)) {
        Outcome::Made {
            points: shot.points(),
        }
    } else {
        Outcome::Missed {
            offensive_rebound: rng.chance(OFFENSIVE_REBOUND),
        }
    }
}

/// The computer's shot: it leans against the defence it saw last.
pub fn computer_shot(rng: &mut Rng, seen: Option<Defence>, open: bool) -> Shot {
    if open {
        return if rng.chance(0.5) {
            Shot::Short
        } else {
            Shot::Long
        };
    }
    if rng.chance(0.35) {
        return SHOTS[rng.range(0, 4) as usize];
    }
    match seen {
        Some(Defence::Press) => Shot::Layup,
        Some(Defence::Man) => {
            if rng.chance(0.5) {
                Shot::Layup
            } else {
                Shot::Pass
            }
        }
        Some(Defence::Zone) => {
            if rng.chance(0.6) {
                Shot::Long
            } else {
                Shot::Set
            }
        }
        _ => Shot::Short,
    }
}

/// The computer's defence: it leans against the shot it saw last.
pub fn computer_defence(rng: &mut Rng, seen: Option<Shot>) -> Defence {
    if rng.chance(0.3) {
        return DEFENCES[rng.range(0, 3) as usize];
    }
    match seen {
        Some(Shot::Long) | Some(Shot::Short) => Defence::Man,
        Some(Shot::Layup) => Defence::Zone,
        Some(Shot::Set) | Some(Shot::Pass) => Defence::Press,
        None => Defence::Man,
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
        shot: Shot,
        def: Defence,
        out: Outcome,
        shooter: &'static str,
    },
    Break,
    Over,
}

pub struct Basketball {
    ctx: Ctx,
    score: [u32; 2],
    period: u32,
    clock: u32,
    ball: Side,
    first_ball: Side,
    open: bool,
    phase: Phase,
    sel: usize,
    log: Vec<(String, Color)>,
    seen_shot: Option<Shot>,
    seen_def: Option<Defence>,
    stats: Kv,
    quit: bool,
    show_help: bool,
    demo_clock: f32,
    rows: Vec<Rect>,
}

fn period_name(p: u32) -> String {
    match p {
        1 => "1st half".into(),
        2 => "2nd half".into(),
        3 => "overtime".into(),
        n => format!("overtime {}", n - 2),
    }
}

impl Basketball {
    fn new_game(&mut self) {
        self.score = [0, 0];
        self.period = 1;
        self.clock = HALF_SECS;
        self.ball = if self.ctx.rng.chance(0.5) {
            Side::Us
        } else {
            Side::Them
        };
        self.first_ball = self.ball;
        self.open = false;
        self.phase = Phase::Pick;
        self.sel = 0;
        self.log.clear();
        self.seen_shot = None;
        self.seen_def = None;
        let tip = ROSTERS[self.ball.i()][3];
        self.say(
            format!("Tip-off! {tip} wins it for {}.", TEAMS[self.ball.i()]),
            CYAN,
        );
        log::info!("new game, first ball {:?}", self.ball);
    }

    fn say(&mut self, s: String, c: Color) {
        self.log.push((s, c));
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }

    fn choices(&self) -> Vec<String> {
        if self.ball == Side::Us {
            SHOTS
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let p = if s.points() > 0 {
                        format!(" ({})", s.points())
                    } else {
                        String::new()
                    };
                    format!("{} {}{p}", i + 1, s.name())
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
        let open = self.open;
        let (shot, def) = if self.ball == Side::Us {
            let shot = SHOTS[i];
            let def = computer_defence(&mut self.ctx.rng, self.seen_shot);
            self.seen_shot = Some(shot);
            (shot, def)
        } else {
            let def = DEFENCES[i];
            let shot = computer_shot(&mut self.ctx.rng, self.seen_def, open);
            self.seen_def = Some(def);
            (shot, def)
        };
        let out = resolve(&mut self.ctx.rng, shot, def, open);
        let r = &ROSTERS[self.ball.i()];
        let shooter = r[self.ctx.rng.range(0, 4) as usize];
        log::info!("{:?} ball: {shot:?} against {def:?} -> {out:?}", self.ball);
        self.phase = Phase::Play {
            t: 0.0,
            shot,
            def,
            out,
            shooter,
        };
    }

    fn finish(&mut self, shot: Shot, def: Defence, out: Outcome, shooter: &'static str) {
        let side = self.ball;
        let team = TEAMS[side.i()];
        let guard = ROSTERS[side.other().i()][self.ctx.rng.range(0, 4) as usize];
        let ours = side == Side::Us;
        let good = if ours { GREEN } else { RED };
        let bad = if ours { RED } else { GREEN };
        let lead = format!(
            "{team}, {} vs {}:",
            shot.name().to_lowercase(),
            def.name().to_lowercase()
        );
        let secs = match out {
            Outcome::Worked => self.ctx.rng.range(6, 12),
            Outcome::Missed {
                offensive_rebound: true,
            } => self.ctx.rng.range(10, 20),
            _ => self.ctx.rng.range(14, 30),
        } as u32;
        self.open = false;
        match out {
            Outcome::Stolen => {
                self.say(format!("{lead} {guard} picks it off from {shooter}!"), bad);
                self.ctx
                    .audio
                    .play(if ours { Sound::Bad } else { Sound::Good });
                self.ball = side.other();
            }
            Outcome::Worked => {
                self.say(
                    format!("{lead} {shooter} swings it round, and there's an opening."),
                    INK,
                );
                self.ctx.audio.play(Sound::Tick);
                self.open = true;
            }
            Outcome::Fouled { made, of } => {
                self.ctx.audio.play(Sound::Beep(1900.0));
                self.score[side.i()] += made;
                self.say(
                    format!(
                        "{lead} whistle, {guard} fouls {shooter}. {made} of {of} from the line."
                    ),
                    if made > 0 { good } else { DIM },
                );
                self.ball = side.other();
            }
            Outcome::Made { points } => {
                self.score[side.i()] += points;
                let how = if points == 3 {
                    "from way out, three!"
                } else {
                    "it's good."
                };
                self.say(format!("{lead} {shooter}, {how}"), good);
                self.ctx
                    .audio
                    .play(if ours { Sound::Good } else { Sound::Bad });
                self.ball = side.other();
            }
            Outcome::Missed { offensive_rebound } => {
                if offensive_rebound {
                    let mate = ROSTERS[side.i()][self.ctx.rng.range(0, 4) as usize];
                    self.say(
                        format!("{lead} {shooter} misses, but {mate} grabs the board!"),
                        INK,
                    );
                } else {
                    self.say(format!("{lead} {shooter} misses; {guard} rebounds."), DIM);
                    self.ball = side.other();
                }
                self.ctx.audio.play(Sound::Tick);
            }
        }
        self.clock = self.clock.saturating_sub(secs);
        self.phase = Phase::Pick;
        if self.clock == 0 {
            self.end_period();
        }
    }

    fn end_period(&mut self) {
        let [a, b] = self.score;
        if self.period == 1 {
            self.say(
                format!("Halftime: {} {a}, {} {b}.", TEAMS[0], TEAMS[1]),
                AMBER,
            );
            self.period = 2;
            self.clock = HALF_SECS;
            self.ball = self.first_ball.other();
            self.phase = Phase::Break;
            self.ctx.audio.play(Sound::Beep(440.0));
        } else if a == b {
            self.say(format!("Tied at {a}! Five minutes more."), AMBER);
            self.period += 1;
            self.clock = OT_SECS;
            self.ball = if self.ctx.rng.chance(0.5) {
                Side::Us
            } else {
                Side::Them
            };
            self.phase = Phase::Break;
            self.ctx.audio.play(Sound::Beep(440.0));
        } else {
            let won = a > b;
            self.say(
                format!(
                    "Final: {} {a}, {} {b}. {}",
                    TEAMS[0],
                    TEAMS[1],
                    if won {
                        "Your team wins!"
                    } else {
                        "A loss tonight."
                    }
                ),
                if won { GREEN } else { RED },
            );
            self.ctx
                .audio
                .play(if won { Sound::Win } else { Sound::Lose });
            self.phase = Phase::Over;
            log::info!("game over {a}-{b}");
            if !self.ctx.demo {
                let k = if won { "won" } else { "lost" };
                let n = self.stats.get::<u32>(k).unwrap_or(0) + 1;
                self.stats.set(k, n);
                if won {
                    let best = self.stats.get::<u32>("best").unwrap_or(0).max(a - b);
                    self.stats.set("best", best);
                }
                self.stats.save(Self::ID, "scores");
            }
        }
    }

    fn go_on(&mut self) {
        match self.phase {
            Phase::Break => {
                self.say(
                    format!(
                        "{}: {} have the ball.",
                        period_name(self.period),
                        TEAMS[self.ball.i()]
                    ),
                    CYAN,
                );
                self.phase = Phase::Pick;
            }
            Phase::Over => self.new_game(),
            Phase::Pick => self.pick(self.sel),
            _ => {}
        }
    }

    fn draw_court(&self, buf: &mut Buffer, r: Rect) {
        let floor = Color::Rgb(44, 32, 20);
        let paint = Color::Rgb(58, 38, 24);
        fill(buf, r, on(LINE, floor));
        if r.width < 20 || r.height < 5 {
            return;
        }
        let mid_y = r.y + r.height / 2;
        let (l, rt) = (r.x, r.right() - 1);
        let line = on(mix(LINE, INK, 0.3), floor);
        for x in l..=rt {
            buf[(x, r.y)].set_symbol("─").set_style(line);
            buf[(x, r.bottom() - 1)].set_symbol("─").set_style(line);
        }
        for y in r.y..r.bottom() {
            buf[(l, y)].set_symbol("│").set_style(line);
            buf[(rt, y)].set_symbol("│").set_style(line);
            buf[(r.x + r.width / 2, y)].set_symbol("│").set_style(line);
        }
        let half = r.width as f32 / 2.0;
        let hh = (r.height as f32 - 2.0) / 2.0;
        // the paint and the three-point arc at each end
        let lane_w = (r.width / 9).max(3);
        for y in mid_y.saturating_sub(2)..=mid_y + 2 {
            if y <= r.y || y >= r.bottom() - 1 {
                continue;
            }
            for x in 1..=lane_w {
                buf[(l + x, y)].set_style(on(LINE, paint));
                buf[(rt - x, y)].set_style(on(LINE, paint));
            }
        }
        let arc_r = half * 0.55;
        for y in r.y + 1..r.bottom() - 1 {
            for x in l + 1..rt {
                let dy = (y as f32 - mid_y as f32) / hh * (half * 0.45);
                for bx in [l as f32 + 1.0, rt as f32 - 1.0] {
                    let dx = x as f32 - bx;
                    let d = (dx * dx + dy * dy).sqrt();
                    if (d - arc_r).abs() < 0.55 {
                        buf[(x, y)]
                            .set_symbol("·")
                            .set_style(on(mix(LINE, INK, 0.3), floor));
                    }
                }
            }
        }
        let cx = r.x + r.width / 2;
        for (dx, s) in [(-2i32, "("), (2, ")")] {
            buf[((cx as i32 + dx) as u16, mid_y)]
                .set_symbol(s)
                .set_style(line);
        }
        // baskets
        buf[(l + 1, mid_y)]
            .set_symbol("Ⓞ")
            .set_style(on(AMBER, floor));
        buf[(rt - 1, mid_y)]
            .set_symbol("Ⓞ")
            .set_style(on(AMBER, floor));

        // players: the offence attacks the basket on its own side
        let attack_right = self.ball == Side::Us;
        let at = |u: f32, v: f32| -> (u16, u16) {
            let x = if attack_right {
                rt as f32 - 1.0 - u * 1.6 * half
            } else {
                l as f32 + 1.0 + u * 1.6 * half
            };
            let y = mid_y as f32 + v * (hh - 0.5);
            (
                (x.round() as u16).clamp(l + 1, rt - 1),
                (y.round() as u16).clamp(r.y + 1, r.bottom() - 2),
            )
        };
        let (def, shot, t) = match &self.phase {
            Phase::Play { def, shot, t, .. } => (Some(*def), Some(*shot), *t / PLAY_SECS),
            _ => (self.seen_def.filter(|_| self.ball == Side::Them), None, 0.0),
        };
        let offence = [
            (0.42, 0.0),
            (0.33, -0.65),
            (0.33, 0.65),
            (0.14, -0.85),
            (0.12, 0.8),
        ];
        let defence: Vec<(f32, f32)> = match def.unwrap_or(Defence::Man) {
            Defence::Zone => vec![
                (0.25, -0.4),
                (0.25, 0.4),
                (0.1, -0.7),
                (0.05, 0.0),
                (0.1, 0.7),
            ],
            Defence::Press => offence.iter().map(|(u, v)| (u + 0.06, v * 0.9)).collect(),
            Defence::Sag => vec![
                (0.12, -0.3),
                (0.1, 0.3),
                (0.05, -0.6),
                (0.04, 0.0),
                (0.06, 0.6),
            ],
            Defence::Man => offence.iter().map(|(u, v)| (u - 0.04, v * 0.9)).collect(),
        };
        let (oc, dc) = if attack_right {
            (CYAN, RED)
        } else {
            (RED, CYAN)
        };
        let wob = (t * 6.0).sin() * 0.02;
        for (u, v) in &defence {
            let (x, y) = at(u + wob, *v);
            buf[(x, y)].set_symbol("●").set_style(bold(on(dc, floor)));
        }
        for (i, (u, v)) in offence.iter().enumerate() {
            let (x, y) = at(u - wob, *v);
            buf[(x, y)].set_symbol("●").set_style(bold(on(oc, floor)));
            let _ = i;
        }
        // the ball: held at the top of the key, or in the air on a shot
        let (bu, bv) = match shot {
            Some(s) => {
                let start = s.range();
                let k = t.clamp(0.0, 1.0);
                let lift = if s == Shot::Pass {
                    0.0
                } else {
                    (k * std::f32::consts::PI).sin() * 0.5
                };
                let end = if s == Shot::Pass { 0.2 } else { 0.0 };
                (start + (end - start) * k, -lift)
            }
            None => (0.45, 0.0),
        };
        let (x, y) = at(bu, bv);
        buf[(x, y)]
            .set_symbol("◉")
            .set_style(bold(on(AMBER, floor)));
    }
}

impl Game for Basketball {
    const ID: &'static str = "basketball";
    const TITLE: &'static str = "Basketball";
    const HELP: &'static str = "Coach Lakeside against Cedar State: two twenty-minute halves.
With the ball, call a shot (long, short, layup, set) or work it around
for an open look. Without it, call a defence: the press steals and fouls,
man-to-man smothers jump shots, the zone shuts the lane, sagging off
rests your legs but gives up open shots. The computer reads your last
call and leans against it.
1-5 or arrows and Enter to call, Tab the rules, N a new game, M sound,
Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let mut g = Basketball {
            ctx,
            score: [0, 0],
            period: 1,
            clock: HALF_SECS,
            ball: Side::Us,
            first_ball: Side::Us,
            open: false,
            phase: Phase::Pick,
            sel: 0,
            log: Vec::new(),
            seen_shot: None,
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
        if let Phase::Play {
            t,
            shot,
            def,
            out,
            shooter,
        } = &mut self.phase
        {
            *t += dt;
            if *t >= PLAY_SECS {
                let (s, d, o, p) = (*shot, *def, *out, *shooter);
                self.finish(s, d, o, p);
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
                let i = if self.ball == Side::Us {
                    let s = computer_shot(&mut self.ctx.rng, self.seen_def, self.open);
                    SHOTS.iter().position(|x| *x == s).unwrap_or(0)
                } else {
                    let d = computer_defence(&mut self.ctx.rng, self.seen_shot);
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
            "period {} clock {} score {:?} ball {:?} open {} phase {:?}",
            self.period, self.clock, self.score, self.ball, self.open, self.phase
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.rows.clear();
        let status = format!(
            "{} {}:{:02}",
            period_name(self.period),
            self.clock / 60,
            self.clock % 60
        );
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("1-5", "call"),
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
        let court_h = h - 1 - bottom_h;

        // the scoreboard
        let y = inner.y;
        let ball_mark = |s: Side| if self.ball == s { "◉ " } else { "  " };
        let line = format!(
            "{}{}  {:>3}   {:<3}  {}{}",
            ball_mark(Side::Us),
            TEAMS[0].to_uppercase(),
            self.score[0],
            self.score[1],
            TEAMS[1].to_uppercase(),
            if self.ball == Side::Them { " ◉" } else { "" }
        );
        centre(buf, inner, y, &line, bold(st(INK)));
        let w = line.chars().count() as u16;
        let x0 = inner.x + inner.width.saturating_sub(w) / 2;
        put(buf, x0 + 2, y, &TEAMS[0].to_uppercase(), bold(st(CYAN)));
        let tx = x0 + w - TEAMS[1].len() as u16 - if self.ball == Side::Them { 2 } else { 0 };
        put(buf, tx, y, &TEAMS[1].to_uppercase(), bold(st(RED)));

        self.draw_court(
            buf,
            Rect::new(inner.x + 1, inner.y + 1, inner.width - 2, court_h),
        );

        let by = inner.y + 1 + court_h;
        let left = Rect::new(inner.x, by, 34, bottom_h);
        let right = Rect::new(inner.x + 34, by, inner.width - 34, bottom_h);
        let title = if matches!(self.phase, Phase::Over) {
            "Full time"
        } else if matches!(self.phase, Phase::Break) {
            "The break"
        } else if self.ball == Side::Us {
            "Your ball: call it"
        } else {
            "Their ball: defend"
        };
        let p = panel(buf, left, title, AMBER);
        match self.phase {
            Phase::Pick | Phase::Play { .. } => {
                if self.open && self.ball == Side::Us {
                    put(buf, p.x, p.y, "Open look: +10% on this shot", st(GREEN));
                }
                let items = self.choices();
                let mr = Rect::new(
                    p.x.saturating_sub(1),
                    p.y + 1,
                    p.width + 1,
                    items.len() as u16,
                );
                let sel = if matches!(self.phase, Phase::Pick) {
                    self.sel
                } else {
                    usize::MAX
                };
                self.rows = menu(buf, mr, &items, sel);
            }
            Phase::Break => {
                para(buf, p, "The break. Enter to play on.", st(AMBER));
            }
            Phase::Over => {
                let won = self.stats.get::<u32>("won").unwrap_or(0);
                let lost = self.stats.get::<u32>("lost").unwrap_or(0);
                let best = self.stats.get::<u32>("best").unwrap_or(0);
                para(
                    buf,
                    p,
                    &format!(
                        "Final whistle. Enter for a new game.\n\nSeason: {won} won, {lost} lost{}",
                        if best > 0 { format!("\nBiggest win: {best} points") } else { String::new() }
                    ),
                    st(INK),
                );
            }
        }

        if self.show_help {
            let q = panel(buf, right, "How it plays", CYAN);
            para(buf, q, "With the ball: a long jump shot scores three, the others two. Layups go in most often but die against a zone; long shots punish a zone. Working it around costs a few seconds and risks a steal, but gives the next shot a better look.\nOn defence: the press steals and fouls most and gives up layups; man-to-man smothers jump shots; the zone shuts the lane; sagging off rests but gives open shots. Free throws go in 70% of the time.", st(INK));
        } else {
            let q = panel(buf, right, "Play by play", CYAN);
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
    fn defences_change_the_odds() {
        assert!(
            chance(Shot::Layup, Defence::Zone, false) < chance(Shot::Layup, Defence::Press, false)
        );
        assert!(chance(Shot::Long, Defence::Zone, false) > chance(Shot::Long, Defence::Man, false));
        assert!(chance(Shot::Short, Defence::Man, true) > chance(Shot::Short, Defence::Man, false));
    }

    #[test]
    fn outcomes_score_the_right_points() {
        let mut rng = Rng::new(5);
        for _ in 0..500 {
            for s in SHOTS {
                match resolve(&mut rng, s, Defence::Man, false) {
                    Outcome::Made { points } => assert_eq!(points, s.points()),
                    Outcome::Fouled { made, of } => assert!(made <= of && of == s.points()),
                    Outcome::Worked => assert_eq!(s, Shot::Pass),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn layups_beat_long_shots_against_the_press() {
        let mut rng = Rng::new(9);
        let pts = |rng: &mut Rng, s| -> u32 {
            (0..4000)
                .map(|_| match resolve(rng, s, Defence::Press, false) {
                    Outcome::Made { points } => points,
                    Outcome::Fouled { made, .. } => made,
                    _ => 0,
                })
                .sum()
        };
        assert!(pts(&mut rng, Shot::Layup) > pts(&mut rng, Shot::Set));
    }
}
