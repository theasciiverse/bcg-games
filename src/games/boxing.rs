//! Boxing: a three-round amateur bout against the computer. Each boxer has
//! a best punch and a weak spot; you choose yours, find out the other's as
//! the bout goes on, and call every exchange. Rules in
//! asciiverse/specs/boxing.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const ROUNDS: u32 = 3;
pub const EXCHANGES: u32 = 8;
/// three knockdowns in one round and the referee stops it
pub const DOWNS_TO_STOP: u8 = 3;
const SHOW_SECS: f32 = 1.0;
const COUNT_SECS: f32 = 2.6;

const OPPONENTS: [&str; 5] = [
    "Dario Fenn",
    "Tomas Vela",
    "Jules Okoro",
    "Kasimir Lund",
    "Ren Ishida",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Jab,
    Cross,
    Hook,
    Upper,
    Guard,
}

pub const ACTS: [Act; 5] = [Act::Jab, Act::Cross, Act::Hook, Act::Upper, Act::Guard];
pub const PUNCHES: [Act; 4] = [Act::Jab, Act::Cross, Act::Hook, Act::Upper];

impl Act {
    pub fn name(self) -> &'static str {
        match self {
            Act::Jab => "Jab",
            Act::Cross => "Cross",
            Act::Hook => "Hook",
            Act::Upper => "Uppercut",
            Act::Guard => "Cover up",
        }
    }
    fn base(self) -> f32 {
        match self {
            Act::Jab => 0.62,
            Act::Cross => 0.48,
            Act::Hook => 0.40,
            Act::Upper => 0.32,
            Act::Guard => 0.0,
        }
    }
    fn power(self) -> u32 {
        match self {
            Act::Jab => 1,
            Act::Cross => 2,
            Act::Hook | Act::Upper => 3,
            Act::Guard => 0,
        }
    }
    fn cost(self) -> f32 {
        match self {
            Act::Jab => 0.04,
            Act::Cross => 0.07,
            Act::Hook => 0.09,
            Act::Upper => 0.10,
            Act::Guard => -0.12,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Boxer {
    pub name: String,
    pub best: Act,
    pub weak: Act,
    pub stamina: f32,
    /// punishment taken this round
    pub hurt: f32,
    pub downs_round: u8,
    pub downs: u8,
    /// clean punch points this round
    pub pts: u32,
    /// what this boxer has learned of the other
    pub knows_weak: Option<Act>,
    pub knows_best: Option<Act>,
}

impl Boxer {
    pub fn new(name: &str, best: Act, weak: Act) -> Boxer {
        Boxer {
            name: name.into(),
            best,
            weak,
            stamina: 1.0,
            hurt: 0.0,
            downs_round: 0,
            downs: 0,
            pts: 0,
            knows_weak: None,
            knows_best: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Blow {
    pub landed: bool,
    pub effect: u32,
    pub weak: bool,
    pub best: bool,
}

pub fn land_chance(att: &Boxer, p: Act, def: &Boxer, def_act: Act) -> f32 {
    let mut c = p.base();
    if p == att.best {
        c += 0.15;
    }
    if p == def.weak {
        c += 0.10;
    }
    if def_act == Act::Guard {
        c *= 0.45;
    }
    c *= 0.6 + 0.4 * att.stamina;
    c.clamp(0.0, 0.95)
}

pub fn throw(rng: &mut Rng, att: &Boxer, p: Act, def: &Boxer, def_act: Act) -> Blow {
    if p == Act::Guard {
        return Blow::default();
    }
    if !rng.chance(land_chance(att, p, def, def_act)) {
        return Blow::default();
    }
    let best = p == att.best;
    let weak = p == def.weak;
    let mut effect = p.power() + if best { 1 } else { 0 };
    if weak {
        effect *= 2;
    }
    Blow {
        landed: true,
        effect,
        weak,
        best,
    }
}

/// The chance a boxer beats the count after a knockdown.
pub fn rise_chance(b: &Boxer) -> f32 {
    (0.92 - 0.18 * b.downs.saturating_sub(1) as f32 - (1.0 - b.stamina) * 0.25).clamp(0.05, 0.95)
}

/// What a boxer does next: its best punch often, the other's weak spot
/// once found, cover up when tired.
pub fn choose(rng: &mut Rng, me: &Boxer) -> Act {
    if me.stamina < 0.3 && rng.chance(0.65) {
        return Act::Guard;
    }
    if let Some(w) = me.knows_weak {
        if rng.chance(0.45) {
            return w;
        }
    }
    if rng.chance(0.35) {
        return me.best;
    }
    if rng.chance(0.12) {
        return Act::Guard;
    }
    PUNCHES[rng.range(0, 3) as usize]
}

/// One judge's card for a round: 10 to the boxer who did more, 9 (or 10
/// for an even round) to the other, a point off for each knockdown.
pub fn judge(rng: &mut Rng, a: &Boxer, b: &Boxer) -> (u32, u32) {
    let d = a.pts as f32 - b.pts as f32 + rng.normal() * 1.5;
    let (mut x, mut y) = if d.abs() < 1.0 {
        (10, 10)
    } else if d > 0.0 {
        (10, 9)
    } else {
        (9, 10)
    };
    x -= a.downs_round.min(3) as u32;
    y -= b.downs_round.min(3) as u32;
    (x.max(7), y.max(7))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Setup {
    Best,
    Weak,
}

#[derive(Debug, Clone, Copy)]
struct Show {
    t: f32,
    acts: [Act; 2],
    blows: [Blow; 2],
    /// a boxer down, and whether they beat the count
    down: Option<(usize, bool)>,
}

#[derive(Debug)]
enum Phase {
    Setup(Setup),
    Pick,
    Show(Show),
    Corner,
    Over,
}

pub struct Boxing {
    ctx: Ctx,
    b: [Boxer; 2],
    round: u32,
    exchange: u32,
    cards: Vec<[(u32, u32); 3]>,
    phase: Phase,
    sel: usize,
    log: Vec<(String, Color)>,
    stats: Kv,
    quit: bool,
    show_help: bool,
    demo_clock: f32,
    rows: Vec<Rect>,
    result: String,
}

impl Boxing {
    fn new_bout(&mut self) {
        let opp = OPPONENTS[self.ctx.rng.range(0, OPPONENTS.len() as i64 - 1) as usize];
        let best = PUNCHES[self.ctx.rng.range(0, 3) as usize];
        let mut weak = PUNCHES[self.ctx.rng.range(0, 3) as usize];
        while weak == best {
            weak = PUNCHES[self.ctx.rng.range(0, 3) as usize];
        }
        self.b = [
            Boxer::new("You", Act::Jab, Act::Hook),
            Boxer::new(opp, best, weak),
        ];
        self.round = 1;
        self.exchange = 0;
        self.cards.clear();
        self.log.clear();
        self.result.clear();
        self.sel = 0;
        self.phase = Phase::Setup(Setup::Best);
        self.say(format!("Tonight's opponent: {opp}. Three rounds."), CYAN);
        self.say("Choose your best punch.".into(), AMBER);
        log::info!("new bout against {opp}");
    }

    fn say(&mut self, s: String, c: Color) {
        self.log.push((s, c));
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }

    fn choices(&self) -> Vec<String> {
        match self.phase {
            Phase::Setup(Setup::Best) => PUNCHES
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} {}", i + 1, p.name()))
                .collect(),
            Phase::Setup(Setup::Weak) => PUNCHES
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} open to the {}", i + 1, p.name().to_lowercase()))
                .collect(),
            _ => ACTS
                .iter()
                .enumerate()
                .map(|(i, a)| format!("{} {}", i + 1, a.name()))
                .collect(),
        }
    }

    fn pick(&mut self, i: usize) {
        if i >= self.choices().len() {
            return;
        }
        match self.phase {
            Phase::Setup(Setup::Best) => {
                self.b[0].best = PUNCHES[i];
                self.phase = Phase::Setup(Setup::Weak);
                self.say(
                    format!("Your best punch: the {}.", PUNCHES[i].name().to_lowercase()),
                    INK,
                );
                self.say("And your weak spot?".into(), AMBER);
                self.ctx.audio.play(Sound::Click);
                self.sel = 0;
            }
            Phase::Setup(Setup::Weak) => {
                if PUNCHES[i] == self.b[0].best {
                    self.say("Your weak spot can't be your best punch.".into(), RED);
                    self.ctx.audio.play(Sound::Nope);
                    return;
                }
                self.b[0].weak = PUNCHES[i];
                self.say(
                    format!(
                        "You're open to the {}. Keep it covered.",
                        PUNCHES[i].name().to_lowercase()
                    ),
                    INK,
                );
                self.say("Ding! Round 1.".into(), AMBER);
                self.ctx.audio.play(Sound::Beep(1200.0));
                self.phase = Phase::Pick;
                self.sel = 0;
            }
            Phase::Pick => self.exchange_with(ACTS[i]),
            _ => {}
        }
    }

    fn exchange_with(&mut self, mine: Act) {
        let theirs = choose(&mut self.ctx.rng, &self.b[1]);
        let acts = [mine, theirs];
        let blows = [
            throw(&mut self.ctx.rng, &self.b[0], mine, &self.b[1], theirs),
            throw(&mut self.ctx.rng, &self.b[1], theirs, &self.b[0], mine),
        ];
        self.exchange += 1;
        log::info!(
            "round {} exchange {}: {mine:?} vs {theirs:?} -> {blows:?}",
            self.round,
            self.exchange
        );
        for s in 0..2 {
            self.b[s].stamina = (self.b[s].stamina - acts[s].cost()).clamp(0.05, 1.0);
        }
        let mut lines = Vec::new();
        let mut down = None;
        let mut downline = None;
        for s in 0..2 {
            let o = 1 - s;
            let blow = blows[s];
            let who = self.b[s].name.clone();
            let other = self.b[o].name.clone();
            let colour = if s == 0 { GREEN } else { RED };
            if acts[s] == Act::Guard {
                continue;
            }
            let p = acts[s].name().to_lowercase();
            if !blow.landed {
                let how = if acts[o] == Act::Guard {
                    "is caught on the gloves"
                } else {
                    "misses"
                };
                let what = if s == 0 {
                    format!("Your {p} {how}.")
                } else {
                    format!("{who}'s {p} {how}.")
                };
                lines.push((what, DIM));
                continue;
            }
            self.b[s].pts += blow.effect;
            self.b[o].hurt += blow.effect as f32;
            let mut text = if s == 0 {
                format!("Your {p} lands clean!")
            } else {
                format!("{who} lands a {p}.")
            };
            if blow.weak {
                if self.b[s].knows_weak.is_none() {
                    text.push_str(if s == 0 {
                        " A weak spot found!"
                    } else {
                        " Right on your weak spot."
                    });
                } else {
                    text.push_str(" Right on the weak spot.");
                }
                self.b[s].knows_weak = Some(acts[s]);
            }
            if blow.best && self.b[o].knows_best.is_none() && s == 1 {
                text.push_str(" That looks like the best punch in that corner.");
                self.b[o].knows_best = Some(acts[s]);
            }
            lines.push((text, colour));
            if down.is_none()
                && (self.b[o].hurt >= 12.0 || (blow.effect >= 4 && self.ctx.rng.chance(0.25)))
            {
                self.b[o].downs += 1;
                self.b[o].downs_round += 1;
                self.b[o].hurt = 4.0;
                let up = self.b[o].downs_round < DOWNS_TO_STOP
                    && self.ctx.rng.chance(rise_chance(&self.b[o]));
                down = Some((o, up));
                let name = if o == 0 {
                    "You're".to_string()
                } else {
                    format!("{other} is")
                };
                downline = Some(format!("{name} down!"));
                log::info!("knockdown {o}, up {up}");
            }
        }
        if acts == [Act::Guard, Act::Guard] {
            lines.push((
                "Both cover up and circle, feeling each other out.".into(),
                DIM,
            ));
        }
        if let Some(l) = downline {
            lines.push((l, AMBER));
        }
        for (l, c) in lines {
            self.say(l, c);
        }
        let landed = [blows[0].landed, blows[1].landed];
        self.ctx.audio.play(if down.is_some() {
            Sound::Boom
        } else if landed[0] && !landed[1] {
            Sound::Good
        } else if landed[1] && !landed[0] {
            Sound::Bad
        } else {
            Sound::Tick
        });
        self.phase = Phase::Show(Show {
            t: 0.0,
            acts,
            blows,
            down,
        });
    }

    fn after_show(&mut self, s: Show) {
        if let Some((who, up)) = s.down {
            if up {
                let name = if who == 0 {
                    "You beat".to_string()
                } else {
                    format!("{} beats", self.b[who].name)
                };
                self.say(
                    format!("{name} the count and the referee waves the bout on."),
                    INK,
                );
            } else {
                let stop = self.b[who].downs_round >= DOWNS_TO_STOP;
                let how = if stop {
                    "Third time down: the referee waves it off."
                } else {
                    "The count reaches ten."
                };
                self.say(how.into(), AMBER);
                self.finish(Some(1 - who), true);
                return;
            }
        }
        if self.exchange >= EXCHANGES {
            self.end_round();
        } else {
            self.phase = Phase::Pick;
        }
    }

    fn end_round(&mut self) {
        let mut card = [(0, 0); 3];
        for c in card.iter_mut() {
            *c = judge(&mut self.ctx.rng, &self.b[0], &self.b[1]);
        }
        self.cards.push(card);
        self.say(
            format!(
                "End of round {}. Judges: {}-{}, {}-{}, {}-{}.",
                self.round, card[0].0, card[0].1, card[1].0, card[1].1, card[2].0, card[2].1
            ),
            AMBER,
        );
        self.ctx.audio.play(Sound::Beep(1200.0));
        if self.round >= ROUNDS {
            let mut votes = [0, 0];
            for j in 0..3 {
                let (a, b) = self
                    .cards
                    .iter()
                    .fold((0, 0), |(x, y), c| (x + c[j].0, y + c[j].1));
                if a > b {
                    votes[0] += 1;
                } else if b > a {
                    votes[1] += 1;
                }
            }
            let w = if votes[0] > votes[1] {
                Some(0)
            } else if votes[1] > votes[0] {
                Some(1)
            } else {
                None
            };
            self.finish(w, false);
            return;
        }
        for b in self.b.iter_mut() {
            b.stamina = (b.stamina + 0.35).min(1.0);
            b.hurt = 0.0;
            b.downs_round = 0;
            b.pts = 0;
        }
        self.round += 1;
        self.exchange = 0;
        self.phase = Phase::Corner;
    }

    fn finish(&mut self, winner: Option<usize>, stopped: bool) {
        let opp = self.b[1].name.clone();
        let (k, line, c, snd) = match winner {
            Some(0) => (
                "won",
                if stopped {
                    format!("You win, stopping {opp}!")
                } else {
                    format!("You win on the judges' cards over {opp}!")
                },
                GREEN,
                Sound::Win,
            ),
            Some(_) => (
                "lost",
                if stopped {
                    format!("{opp} wins: the bout is stopped.")
                } else {
                    format!("{opp} takes it on the cards.")
                },
                RED,
                Sound::Lose,
            ),
            None => (
                "drawn",
                "A draw on the cards.".to_string(),
                AMBER,
                Sound::Bad,
            ),
        };
        self.result = line.clone();
        self.say(line, c);
        self.say(
            "The two touch gloves and embrace in the centre of the ring.".into(),
            INK,
        );
        self.ctx.audio.play(snd);
        self.phase = Phase::Over;
        log::info!("bout over: {k}, stopped {stopped}");
        if !self.ctx.demo {
            let n = self.stats.get::<u32>(k).unwrap_or(0) + 1;
            self.stats.set(k, n);
            if winner == Some(0) && stopped {
                let n = self.stats.get::<u32>("stoppages").unwrap_or(0) + 1;
                self.stats.set("stoppages", n);
            }
            self.stats.save(Self::ID, "scores");
        }
    }

    fn go_on(&mut self) {
        match self.phase {
            Phase::Corner => {
                self.say(format!("Ding! Round {}.", self.round), AMBER);
                self.phase = Phase::Pick;
            }
            Phase::Over => self.new_bout(),
            Phase::Pick | Phase::Setup(_) => self.pick(self.sel),
            _ => {}
        }
    }

    fn draw_ring(&self, buf: &mut Buffer, r: Rect) {
        let canvas = Color::Rgb(40, 40, 52);
        fill(buf, r, on(INK, canvas));
        if r.width < 30 || r.height < 6 {
            return;
        }
        let rope = on(Color::Rgb(200, 60, 60), canvas);
        for (i, y) in [r.y, r.y + 1].into_iter().enumerate() {
            for x in r.x + 2..r.right() - 2 {
                buf[(x, y)]
                    .set_symbol(if i == 0 { "═" } else { "─" })
                    .set_style(rope);
            }
        }
        for x in r.x + 2..r.right() - 2 {
            buf[(x, r.bottom() - 1)].set_symbol("═").set_style(rope);
        }
        for y in r.y..r.bottom() {
            buf[(r.x + 1, y)]
                .set_symbol("█")
                .set_style(on(Color::Rgb(108, 140, 236), canvas));
            buf[(r.right() - 2, y)]
                .set_symbol("█")
                .set_style(on(Color::Rgb(232, 86, 70), canvas));
        }
        let (show, t) = match &self.phase {
            Phase::Show(s) => (Some(*s), s.t),
            _ => (None, 0.0),
        };
        let cx = r.x + r.width / 2;
        let close = show.map_or(0.0, |_| {
            (t / SHOW_SECS * std::f32::consts::PI).sin().max(0.0) * 2.0
        }) as u16;
        let base_y = r.bottom() - 4;
        for side in 0..2 {
            let act = show.map(|s| s.acts[side]);
            let landed_on = show.is_some_and(|s| s.blows[1 - side].landed) && t > SHOW_SECS * 0.4;
            let is_down = show.is_some_and(|s| s.down.is_some_and(|(w, _)| w == side))
                && t > SHOW_SECS * 0.45;
            let swinging = act.is_some_and(|a| a != Act::Guard) && t > 0.15 && t < SHOW_SECS * 0.8;
            let sprite: [&str; 3] = if is_down {
                ["     ", "     ", "●O═<_"]
            } else if act == Some(Act::Guard) {
                ["●O● ", " │  ", "/ \\ "]
            } else if swinging {
                match act {
                    Some(Act::Upper) => [" O ●", " │╯ ", "/ \\ "],
                    Some(Act::Hook) => [" O╮ ", " │ ●", "/ \\ "],
                    _ => [" O  ", " ├──●", "/ \\ "],
                }
            } else {
                [" O  ", "●│● ", "/ \\ "]
            };
            let colour = if side == 0 { CYAN } else { RED };
            let colour = if landed_on && !is_down {
                mix(colour, INK, 0.6)
            } else {
                colour
            };
            let gap = 6u16.saturating_sub(close);
            for (row, line) in sprite.iter().enumerate() {
                let line: String = if side == 1 {
                    line.chars()
                        .rev()
                        .map(|c| match c {
                            '/' => '\\',
                            '\\' => '/',
                            '<' => '>',
                            '╮' => '╭',
                            '╯' => '╰',
                            '├' => '┤',
                            c => c,
                        })
                        .collect()
                } else {
                    line.to_string()
                };
                let w = line.chars().count() as u16;
                let x = if side == 0 {
                    cx - gap / 2 - w
                } else {
                    cx + gap / 2
                };
                let y = base_y + row as u16;
                for (i, ch) in line.chars().enumerate() {
                    if ch != ' ' {
                        buf[(x + i as u16, y)]
                            .set_symbol(&ch.to_string())
                            .set_style(bold(on(colour, canvas)));
                    }
                }
            }
            if let (Some(a), true) = (act, swinging) {
                let label = a.name().to_uppercase();
                let x = if side == 0 {
                    cx.saturating_sub(gap / 2 + 12)
                } else {
                    cx + gap / 2 + 4
                };
                put(buf, x, base_y - 2, &label, bold(on(colour, canvas)));
            }
        }
        centre(
            buf,
            r,
            r.y + 5,
            &format!("ROUND {}", self.round),
            on(mix(canvas, INK, 0.25), canvas),
        );
        // the count, when someone is down
        if let Some(Show {
            down: Some((who, up)),
            t,
            ..
        }) = show
        {
            if t > SHOW_SECS * 0.5 {
                let n = (((t - SHOW_SECS * 0.5) / 0.22) as u32 + 1).min(if up { 8 } else { 10 });
                let text = format!("COUNT: {n}");
                centre(buf, r, r.y + 3, &text, bold(on(AMBER, canvas)));
                let _ = who;
            }
        }
        put(
            buf,
            r.x + 3,
            r.y + 2,
            &self.b[0].name.to_uppercase(),
            bold(on(CYAN, canvas)),
        );
        let n = self.b[1].name.to_uppercase();
        put(
            buf,
            r.right() - 3 - n.chars().count() as u16,
            r.y + 2,
            &n,
            bold(on(RED, canvas)),
        );
    }

    fn scouting(&self) -> (String, String) {
        let w = self.b[0]
            .knows_weak
            .map_or("?".to_string(), |a| a.name().to_lowercase());
        let b = self.b[0]
            .knows_best
            .map_or("?".to_string(), |a| a.name().to_lowercase());
        (
            format!("their best punch: {b}"),
            format!("their weak spot:  {w}"),
        )
    }
}

impl Game for Boxing {
    const ID: &'static str = "boxing";
    const TITLE: &'static str = "Boxing";
    const HELP: &'static str =
        "A three-round amateur bout. First choose your best punch and your weak
spot; your opponent has both too, and you find them out as you go.
Each exchange, throw a jab, cross, hook or uppercut, or cover up to
catch breath and make the other miss. Judges score every round; three
knockdowns in a round and the referee stops it.
1-5 or arrows and Enter to choose, Tab the rules, N a new bout,
M sound, Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let mut g = Boxing {
            ctx,
            b: [
                Boxer::new("You", Act::Jab, Act::Hook),
                Boxer::new("", Act::Jab, Act::Hook),
            ],
            round: 1,
            exchange: 0,
            cards: Vec::new(),
            phase: Phase::Pick,
            sel: 0,
            log: Vec::new(),
            stats: Kv::load(Self::ID, "scores"),
            quit: false,
            show_help: false,
            demo_clock: 0.0,
            rows: Vec::new(),
            result: String::new(),
        };
        g.new_bout();
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
            Key::Char('n') => self.new_bout(),
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
                if i < n && matches!(self.phase, Phase::Pick | Phase::Setup(_)) {
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
        } else if m.kind == MouseKind::Click && matches!(self.phase, Phase::Corner | Phase::Over) {
            self.go_on();
        }
    }

    fn tick(&mut self, dt: f32) {
        if let Phase::Show(s) = &mut self.phase {
            s.t += dt;
            let end = if s.down.is_some() {
                SHOW_SECS + COUNT_SECS
            } else {
                SHOW_SECS
            };
            if s.t >= end {
                let s = *s;
                self.after_show(s);
            }
            return;
        }
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = match self.phase {
            Phase::Pick => 0.5,
            Phase::Setup(_) => 0.9,
            Phase::Corner => 2.0,
            _ => 4.0,
        };
        if self.demo_clock < wait {
            return;
        }
        self.demo_clock = 0.0;
        match self.phase {
            Phase::Setup(Setup::Best) => {
                let i = self.ctx.rng.range(0, 3) as usize;
                self.sel = i;
                self.pick(i);
            }
            Phase::Setup(Setup::Weak) => {
                let best = PUNCHES
                    .iter()
                    .position(|p| *p == self.b[0].best)
                    .unwrap_or(0);
                let i = (best + self.ctx.rng.range(1, 3) as usize) % 4;
                self.sel = i;
                self.pick(i);
            }
            Phase::Pick => {
                let a = choose(&mut self.ctx.rng, &self.b[0]);
                let i = ACTS.iter().position(|x| *x == a).unwrap_or(0);
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
            "round {} exchange {} pts {}-{} stamina {:.2}/{:.2} downs {}/{} phase {:?}",
            self.round,
            self.exchange,
            self.b[0].pts,
            self.b[1].pts,
            self.b[0].stamina,
            self.b[1].stamina,
            self.b[0].downs,
            self.b[1].downs,
            self.phase
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.rows.clear();
        let status = format!(
            "round {}/{ROUNDS}  exchange {}/{EXCHANGES}",
            self.round,
            self.exchange.min(EXCHANGES)
        );
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("1-5", "choose"),
                ("Enter", "go"),
                ("Tab", "rules"),
                ("N", "new bout"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 80, 22) {
            return;
        }
        let h = inner.height;
        let bottom_h = 10 + (h - 22) / 2;
        let ring_h = h - 1 - bottom_h;

        // stamina and this round's punch count
        let y = inner.y;
        let half = inner.width / 2;
        for s in 0..2 {
            let colour = if s == 0 { CYAN } else { RED };
            let x = inner.x + 1 + s as u16 * half;
            let label = format!("{:<12} wind ", self.b[s].name.to_uppercase());
            let x2 = put(buf, x, y, &label, bold(st(colour)));
            bar(
                buf,
                x2,
                y,
                10,
                self.b[s].stamina,
                if self.b[s].stamina < 0.3 { RED } else { GREEN },
            );
            put(
                buf,
                x2 + 11,
                y,
                &format!("pts {:>2}", self.b[s].pts),
                st(INK),
            );
        }

        self.draw_ring(
            buf,
            Rect::new(inner.x + 1, inner.y + 1, inner.width - 2, ring_h),
        );

        let by = inner.y + 1 + ring_h;
        let lw = 34;
        let left = Rect::new(inner.x, by, lw, bottom_h);
        let right = Rect::new(inner.x + lw, by, inner.width - lw, bottom_h);
        let title = match self.phase {
            Phase::Setup(Setup::Best) => "Your best punch",
            Phase::Setup(Setup::Weak) => "Your weak spot",
            Phase::Corner => "In the corner",
            Phase::Over => "The result",
            _ => "Your move",
        };
        let p = panel(buf, left, title, AMBER);
        match self.phase {
            Phase::Setup(_) | Phase::Pick | Phase::Show(_) => {
                let items = self.choices();
                let mr = Rect::new(p.x.saturating_sub(1), p.y, p.width + 1, items.len() as u16);
                let sel = if matches!(self.phase, Phase::Show(_)) {
                    usize::MAX
                } else {
                    self.sel
                };
                self.rows = menu(buf, mr, &items, sel);
                if !matches!(self.phase, Phase::Setup(_)) {
                    let (a, b) = self.scouting();
                    put(buf, p.x, p.bottom().saturating_sub(2), &a, st(DIM));
                    put(buf, p.x, p.bottom().saturating_sub(1), &b, st(DIM));
                }
            }
            Phase::Corner => {
                para(
                    buf,
                    p,
                    "A minute's rest: wind comes back.\n\nEnter for the next round.",
                    st(INK),
                );
            }
            Phase::Over => {
                let won = self.stats.get::<u32>("won").unwrap_or(0);
                let lost = self.stats.get::<u32>("lost").unwrap_or(0);
                let drawn = self.stats.get::<u32>("drawn").unwrap_or(0);
                let stops = self.stats.get::<u32>("stoppages").unwrap_or(0);
                para(
                    buf,
                    p,
                    &format!("{}\n\nRecord: {won} won ({stops} early), {lost} lost, {drawn} drawn\n\nEnter for a new bout.", self.result),
                    st(INK),
                );
            }
        }
        if self.show_help {
            let q = panel(buf, right, "How it works", CYAN);
            para(buf, q, "Jabs land most and score least; hooks and uppercuts land least and score most. Your best punch lands more often and harder. A punch to the other's weak spot lands more and counts double. Covering up makes most punches miss and brings your wind back; punching tires you, and a tired boxer misses. Too much punishment in a round brings a knockdown; three in a round ends it. Three judges score each round 10-9, a point off for each knockdown.", st(INK));
        } else {
            let q = panel(buf, right, "Ringside", CYAN);
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
    fn best_punch_and_weak_spot_help() {
        let a = Boxer::new("a", Act::Hook, Act::Jab);
        let b = Boxer::new("b", Act::Jab, Act::Cross);
        assert!(land_chance(&a, Act::Hook, &b, Act::Jab) > Act::Hook.base());
        assert!(land_chance(&a, Act::Cross, &b, Act::Jab) > Act::Cross.base());
        assert!(
            land_chance(&a, Act::Cross, &b, Act::Guard) < land_chance(&a, Act::Cross, &b, Act::Jab)
        );
    }

    #[test]
    fn a_weak_spot_hit_counts_double() {
        let a = Boxer::new("a", Act::Jab, Act::Hook);
        let b = Boxer::new("b", Act::Jab, Act::Upper);
        let mut rng = Rng::new(4);
        for _ in 0..300 {
            let w = throw(&mut rng, &a, Act::Upper, &b, Act::Jab);
            if w.landed {
                assert!(w.weak);
                assert_eq!(w.effect, 6);
            }
        }
    }

    #[test]
    fn judges_take_a_point_for_a_knockdown() {
        let mut rng = Rng::new(2);
        let mut a = Boxer::new("a", Act::Jab, Act::Hook);
        let b = Boxer::new("b", Act::Jab, Act::Hook);
        a.pts = 20;
        a.downs_round = 1;
        let (x, y) = judge(&mut rng, &a, &b);
        assert_eq!((x, y), (9, 9));
    }

    #[test]
    fn rising_gets_harder() {
        let mut b = Boxer::new("b", Act::Jab, Act::Hook);
        b.downs = 1;
        let first = rise_chance(&b);
        b.downs = 3;
        assert!(rise_chance(&b) < first);
    }
}
