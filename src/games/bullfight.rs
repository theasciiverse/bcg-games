//! Bullfight: a bloodless corrida, as the Portuguese fight and as the
//! Portuguese communities of California keep it, where the bull leaves the
//! ring alive behind the steers. Work him with the cape, then the muleta,
//! and close with the forcados' catch or the steers. Rules in
//! asciiverse/specs/bullfight.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[derive(Clone, Debug)]
pub struct Bull {
    pub name: &'static str,
    /// 1 to 5: how straight and true he charges, which the crowd loves
    pub bravery: u8,
    /// 1 to 5: how fast, which makes every pass more dangerous
    pub speed: u8,
    pub weight: u32,
    /// 100 when he comes in; every charge spends some
    pub energy: f32,
}

const NAMES: [&str; 16] = [
    "Lucero",
    "Bravio",
    "Trovador",
    "Carbonero",
    "Galante",
    "Relampago",
    "Marinheiro",
    "Estrela",
    "Faraon",
    "Centelha",
    "Lancero",
    "Valiente",
    "Moreno",
    "Tormenta",
    "Campeiro",
    "Sereno",
];

pub fn new_bull(rng: &mut Rng) -> Bull {
    Bull {
        name: NAMES[rng.range(0, NAMES.len() as i64 - 1) as usize],
        bravery: rng.range(1, 5) as u8,
        speed: rng.range(1, 5) as u8,
        weight: rng.range(46, 58) as u32 * 10,
        energy: 100.0,
    }
}

pub fn quality_word(q: u8) -> &'static str {
    ["", "poor", "fair", "good", "fine", "superb"][q.min(5) as usize]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Cape,
    Muleta,
    Finale,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Cape => "the cape",
            Stage::Muleta => "the muleta",
            Stage::Finale => "the finale",
        }
    }
    pub fn most(self) -> u32 {
        match self {
            Stage::Cape => 4,
            Stage::Muleta => 6,
            Stage::Finale => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    pub name: &'static str,
    pub what: &'static str,
    pub danger: f32,
    pub applause: f32,
    /// a closing flourish settles the bull and the nerves: the run of
    /// daring passes starts again
    pub closes: bool,
    /// how close the horns come, in rows of the ring picture
    pub close: bool,
}

pub const CAPE: [Pass; 3] = [
    Pass {
        name: "Veronica",
        what: "slow and close, the cape held in both hands",
        danger: 0.13,
        applause: 10.0,
        closes: false,
        close: true,
    },
    Pass {
        name: "Larga",
        what: "a wide, sweeping pass at arm's length",
        danger: 0.045,
        applause: 4.0,
        closes: false,
        close: false,
    },
    Pass {
        name: "Media veronica",
        what: "a half pass that wraps the cape and fixes the bull",
        danger: 0.08,
        applause: 7.0,
        closes: true,
        close: true,
    },
];

pub const MULETA: [Pass; 3] = [
    Pass {
        name: "Natural",
        what: "the left hand, the purest and most exposed pass",
        danger: 0.16,
        applause: 13.0,
        closes: false,
        close: true,
    },
    Pass {
        name: "Derechazo",
        what: "the right hand, the cloth spread wide",
        danger: 0.1,
        applause: 8.5,
        closes: false,
        close: true,
    },
    Pass {
        name: "Pase de pecho",
        what: "the chest pass, a flourish that sends him past",
        danger: 0.085,
        applause: 7.5,
        closes: true,
        close: false,
    },
];

/// The chance a pass ends with the matador caught.
pub fn danger(p: &Pass, bull: &Bull, team: u8, streak: u32) -> f32 {
    let fierce = 0.7 + 0.12 * bull.speed as f32;
    let fresh = 0.5 + 0.7 * bull.energy / 100.0;
    let nerve = 1.0 + 0.35 * streak as f32;
    let help = 1.15 - 0.1 * team as f32;
    (p.danger * fierce * fresh * nerve * help).clamp(0.01, 0.75)
}

/// Applause for a clean pass, before chance.
pub fn applause(p: &Pass, bull: &Bull, stage: Stage, streak: u32, repeats: u32) -> f32 {
    let brave = 0.6 + 0.15 * bull.bravery as f32;
    let run = 1.0 + 0.2 * streak.min(4) as f32;
    let stage_k = if stage == Stage::Muleta { 1.2 } else { 1.0 };
    let tired = if repeats >= 2 { 0.6 } else { 1.0 };
    p.applause * brave * run * stage_k * tired
}

/// The forcados' chance of catching the bull at the first try.
pub fn pega_chance(bull: &Bull, team: u8) -> f32 {
    (0.3 + 0.08 * team as f32 + 0.35 * (1.0 - bull.energy / 100.0)).clamp(0.1, 0.9)
}

pub fn ovation(score: f32) -> (&'static str, &'static str) {
    match score as i32 {
        i32::MIN..=29 => (
            "Polite silence",
            "The plaza is quiet. There will be other afternoons.",
        ),
        30..=59 => (
            "Warm applause",
            "Steady applause sees you to the barrier.",
        ),
        60..=94 => (
            "A lap of the ring",
            "The crowd calls you out, and you walk a lap of the ring to their applause.",
        ),
        95..=134 => (
            "Flowers and hats",
            "Flowers and hats rain onto the sand as you walk the ring twice.",
        ),
        _ => (
            "Out on shoulders",
            "The crowd will not let you walk: you are carried out on shoulders through the great gate.",
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Intro,
    Choose,
    Passing,
    Tossed,
    Pega,
    LeadOut,
    Result,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    Clean,
    Close,
    Caught,
}

const CROWD_GOOD: [&str; 6] = [
    "Ole!",
    "The plaza rises: Ole!",
    "A roar from the shade side.",
    "Ole! Hats in the air.",
    "The band strikes up a few bars.",
    "Ole! Even the old aficionados nod.",
];
const CROWD_FAIR: [&str; 4] = [
    "Applause, here and there.",
    "A ripple of approval.",
    "Polite clapping from the sun side.",
    "The crowd waits for more.",
];
const CROWD_CLOSE: [&str; 3] = [
    "A horn brushes the sequins. Then a roar.",
    "Too close! The crowd is on its feet.",
    "So near that the plaza holds its breath.",
];

pub struct Bullfight {
    ctx: Ctx,
    bull: Bull,
    team: u8,
    stage: Stage,
    passes: u32,
    streak: u32,
    last: Option<&'static str>,
    repeats: u32,
    score: f32,
    tosses: u32,
    tries: u32,
    phase: Phase,
    anim: f32,
    from_left: bool,
    current: Option<(Pass, Outcome)>,
    pega_ok: bool,
    sel: usize,
    note: String,
    crowd: String,
    cheer: f32,
    stats: Kv,
    show_help: bool,
    quit: bool,
    rows: Vec<(Rect, usize)>,
    demo_clock: f32,
    log: Vec<(String, Color)>,
}

impl Bullfight {
    fn new_fight(&mut self) {
        self.bull = new_bull(&mut self.ctx.rng);
        self.team = self.ctx.rng.range(1, 4) as u8;
        self.stage = Stage::Cape;
        self.passes = 0;
        self.streak = 0;
        self.last = None;
        self.repeats = 0;
        self.score = 0.0;
        self.tosses = 0;
        self.tries = 0;
        self.phase = Phase::Intro;
        self.anim = 0.0;
        self.sel = 0;
        self.current = None;
        self.log.clear();
        self.cheer = 0.0;
        self.crowd = "The plaza fills. The trumpet sounds.".into();
        self.note = format!(
            "{}, {} kilos, comes in at a run. Your team is {}.",
            self.bull.name,
            self.bull.weight,
            quality_word(self.team + 1)
        );
        log::info!(
            "new fight: {} bravery {} speed {} team {}",
            self.bull.name,
            self.bull.bravery,
            self.bull.speed,
            self.team
        );
        self.ctx.audio.play(Sound::Good);
    }

    fn passes_now(&self) -> &'static [Pass; 3] {
        if self.stage == Stage::Muleta {
            &MULETA
        } else {
            &CAPE
        }
    }

    /// The choices on offer: three passes and a way to end the stage, or
    /// in the finale, the forcados or the steers.
    fn choices(&self) -> Vec<String> {
        match self.stage {
            Stage::Finale => vec![
                "The forcados' catch: eight men meet his charge".into(),
                "Call the steers to lead him out".into(),
            ],
            s => {
                let mut v: Vec<String> = self
                    .passes_now()
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.what))
                    .collect();
                v.push(if s == Stage::Cape {
                    "Done with the cape: take up the muleta".into()
                } else {
                    "Done with the muleta: on to the finale".into()
                });
                v
            }
        }
    }

    fn choose(&mut self, i: usize) {
        if self.phase != Phase::Choose {
            return;
        }
        match self.stage {
            Stage::Finale => {
                if i == 0 {
                    self.tries += 1;
                    let p = pega_chance(&self.bull, self.team) + 0.12 * (self.tries - 1) as f32;
                    self.pega_ok = self.ctx.rng.chance(p);
                    self.phase = Phase::Pega;
                    self.anim = 0.0;
                    self.note =
                        "The forcados line up across the sand, their leader in front.".into();
                    self.ctx.audio.play(Sound::Click);
                    log::info!("pega try {} chance {p:.2} ok {}", self.tries, self.pega_ok);
                } else {
                    self.lead_out("The steers trot in, bells ringing, and he goes with them.");
                }
            }
            _ if i == 3 => self.end_stage(),
            _ => {
                let p = self.passes_now()[i.min(2)];
                self.pass(p);
            }
        }
    }

    fn end_stage(&mut self) {
        if self.stage == Stage::Cape {
            self.stage = Stage::Muleta;
            self.note = "You trade the big cape for the small red muleta.".into();
        } else {
            self.stage = Stage::Finale;
            self.note = "The work is done. How will he leave the ring?".into();
        }
        self.passes = 0;
        self.streak = 0;
        self.sel = 0;
        self.ctx.audio.play(Sound::Click);
        log::info!("stage {:?}", self.stage);
    }

    fn pass(&mut self, p: Pass) {
        let d = danger(&p, &self.bull, self.team, self.streak);
        let r = self.ctx.rng.unit();
        let out = if r < d {
            Outcome::Caught
        } else if r < d * 2.2 {
            Outcome::Close
        } else {
            Outcome::Clean
        };
        if self.last == Some(p.name) {
            self.repeats += 1;
        } else {
            self.repeats = 0;
        }
        self.last = Some(p.name);
        self.current = Some((p, out));
        self.phase = Phase::Passing;
        self.anim = 0.0;
        self.from_left = !self.from_left;
        self.note = format!("{}: {}.", p.name, p.what);
        log::info!("pass {} danger {d:.2} -> {out:?}", p.name);
        self.ctx.audio.play(Sound::Hiss(0.6));
    }

    fn finish_pass(&mut self) {
        let Some((p, out)) = self.current.take() else {
            return;
        };
        self.passes += 1;
        self.bull.energy = (self.bull.energy - self.ctx.rng.between(7.0, 12.0)).max(0.0);
        let pick = |rng: &mut Rng, v: &[&'static str]| v[rng.range(0, v.len() as i64 - 1) as usize];
        match out {
            Outcome::Caught => {
                self.tosses += 1;
                self.streak = 0;
                self.phase = Phase::Tossed;
                self.anim = 0.0;
                self.note = "He hooks the cloth and you with it: you are tossed onto the sand. Your team runs in with their capes.".into();
                self.crowd = "A cry goes round the plaza.".into();
                self.cheer = 0.0;
                self.ctx.audio.play(Sound::Boom);
                self.log.push((format!("{}: tossed", p.name), RED));
                return;
            }
            Outcome::Close => {
                let a = applause(&p, &self.bull, self.stage, self.streak, self.repeats) * 1.3;
                self.score += a;
                self.crowd = pick(&mut self.ctx.rng, &CROWD_CLOSE).to_string();
                self.cheer = 1.0;
                self.ctx.audio.play(Sound::Good);
                self.log
                    .push((format!("{}: close! +{:.0}", p.name, a), AMBER));
            }
            Outcome::Clean => {
                let a = applause(&p, &self.bull, self.stage, self.streak, self.repeats)
                    * self.ctx.rng.between(0.8, 1.2);
                self.score += a;
                self.cheer = (a / 14.0).min(1.0);
                self.crowd = if self.repeats >= 2 {
                    "Seen that one. The crowd wants more.".to_string()
                } else if a >= 9.0 {
                    pick(&mut self.ctx.rng, &CROWD_GOOD).to_string()
                } else {
                    pick(&mut self.ctx.rng, &CROWD_FAIR).to_string()
                };
                self.ctx
                    .audio
                    .play(if a >= 9.0 { Sound::Good } else { Sound::Tick });
                self.log.push((format!("{}: +{:.0}", p.name, a), GREEN));
            }
        }
        if p.closes {
            self.streak = 0;
        } else {
            self.streak += 1;
        }
        if self.log.len() > 30 {
            self.log.remove(0);
        }
        self.phase = Phase::Choose;
        if self.bull.energy < 12.0 && self.stage != Stage::Finale {
            self.note = format!("{} stands and will charge no more.", self.bull.name);
            self.stage = Stage::Muleta;
            self.end_stage();
            self.note = format!(
                "{} stands and will charge no more. How will he leave the ring?",
                self.bull.name
            );
        } else if self.passes >= self.stage.most() {
            let n = format!("That is all the passes {} allows.", self.stage.name());
            self.end_stage();
            self.note = format!("{n} {}", self.note);
        } else {
            self.note = match out {
                Outcome::Close => "The horn passed a hand's width away.".into(),
                _ => format!(
                    "{} turns and squares up for another charge.",
                    self.bull.name
                ),
            };
        }
    }

    fn after_toss(&mut self) {
        if self.tosses >= 2 {
            self.note =
                "Your team helps you out of the ring to rest. The steers come for the bull."
                    .into();
            self.score = self.score * 0.6 + 6.0;
            self.lead_out_quiet();
        } else {
            self.score += 5.0;
            self.cheer = 1.0;
            self.note =
                "You are up again, shaking the sand off. The crowd roars for your nerve.".into();
            self.crowd = "Applause for your courage, all round.".into();
            self.phase = Phase::Choose;
            self.ctx.audio.play(Sound::Good);
        }
    }

    fn lead_out(&mut self, why: &str) {
        self.note = why.to_string();
        self.lead_out_quiet();
    }

    fn lead_out_quiet(&mut self) {
        self.phase = Phase::LeadOut;
        self.anim = 0.0;
        self.ctx.audio.play(Sound::Click);
    }

    fn finish_pega(&mut self) {
        if self.pega_ok {
            self.score += 18.0 + 2.0 * self.bull.bravery as f32;
            self.cheer = 1.0;
            self.crowd = "The plaza erupts for the forcados.".into();
            self.log.push(("The catch: held!".into(), GREEN));
            self.ctx.audio.play(Sound::Win);
            self.lead_out(
                "The leader takes the charge on his chest, the line piles on, and they hold him. The steers lead him out.",
            );
        } else if self.tries < 3 {
            self.score += 2.0;
            self.crowd = "Applause for the try.".into();
            self.log.push(("The catch: shaken off".into(), AMBER));
            self.note =
                "He shakes them off and they scatter, unhurt. Try again, or call the steers?"
                    .into();
            self.phase = Phase::Choose;
            self.ctx.audio.play(Sound::Bad);
        } else {
            self.log.push(("The catch: shaken off".into(), AMBER));
            self.lead_out("Three tries is the custom. The steers come in to lead him out.");
        }
    }

    fn result(&mut self) {
        self.phase = Phase::Result;
        let (title, _) = ovation(self.score);
        log::info!("result {:.0} {title}", self.score);
        self.ctx.audio.play(if self.score >= 50.0 {
            Sound::Win
        } else {
            Sound::Click
        });
        if self.ctx.demo {
            return;
        }
        let n = self.stats.get::<u32>("fights").unwrap_or(0) + 1;
        self.stats.set("fights", n);
        let best = self.stats.get::<f32>("best").unwrap_or(0.0).max(self.score);
        self.stats.set("best", best.round());
        if self.score >= 135.0 {
            let s = self.stats.get::<u32>("shoulders").unwrap_or(0) + 1;
            self.stats.set("shoulders", s);
        }
        self.stats.save(Self::ID, "scores");
    }

    fn go_on(&mut self) {
        match self.phase {
            Phase::Intro => {
                self.phase = Phase::Choose;
                self.note = format!("{} paws the sand. Choose your pass.", self.bull.name);
            }
            Phase::Choose => self.choose(self.sel),
            Phase::Result => self.new_fight(),
            _ => {}
        }
    }

    fn demo_choice(&mut self) -> usize {
        if self.stage == Stage::Finale {
            return if self.team >= 2 && self.tries < 2 {
                0
            } else {
                1
            };
        }
        if self.passes >= 3 && self.ctx.rng.chance(0.3) {
            return 3;
        }
        let ps = self.passes_now();
        let d0 = danger(&ps[0], &self.bull, self.team, self.streak);
        if d0 > 0.2 {
            2
        } else if self.ctx.rng.chance(0.55) {
            0
        } else if self.ctx.rng.chance(0.5) {
            1
        } else {
            2
        }
    }
}

/// Where things stand in the ring picture, in cells from its centre.
struct Spot {
    x: f32,
    y: f32,
}

impl Game for Bullfight {
    const ID: &'static str = "bullfight";
    const TITLE: &'static str = "Bullfight";
    const HELP: &'static str = "A bloodless bullfight, as in Portugal and California's Portuguese
festivals: the bull is not harmed, and leaves the ring behind the steers.
Work him with the big cape, then the small red muleta. Close, slow passes
win the most applause and carry the most risk, and a run of them grows
riskier until a flourish settles things. Caught twice and your team takes
you out to rest. End with the forcados' catch or call the steers; the
crowd's ovation is your score.
1 to 4, Up and Down with Enter, or a click choose. Tab shows the rules, N
a new afternoon, M sound, Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let mut g = Bullfight {
            ctx,
            bull: Bull {
                name: NAMES[0],
                bravery: 3,
                speed: 3,
                weight: 500,
                energy: 100.0,
            },
            team: 2,
            stage: Stage::Cape,
            passes: 0,
            streak: 0,
            last: None,
            repeats: 0,
            score: 0.0,
            tosses: 0,
            tries: 0,
            phase: Phase::Intro,
            anim: 0.0,
            from_left: false,
            current: None,
            pega_ok: false,
            sel: 0,
            note: String::new(),
            crowd: String::new(),
            cheer: 0.0,
            stats: Kv::load("bullfight", "scores"),
            show_help: false,
            quit: false,
            rows: Vec::new(),
            demo_clock: 0.0,
            log: Vec::new(),
        };
        g.new_fight();
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
            Key::Char('n') => self.new_fight(),
            Key::Up if self.phase == Phase::Choose => {
                self.sel = (self.sel + n - 1) % n;
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Down if self.phase == Phase::Choose => {
                self.sel = (self.sel + 1) % n;
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Char(c @ '1'..='4') => {
                let i = (c as u8 - b'1') as usize;
                if self.phase == Phase::Intro {
                    self.go_on();
                }
                if self.phase == Phase::Choose && i < n {
                    self.sel = i;
                    self.choose(i);
                }
            }
            Key::Enter | Key::Char(' ') => self.go_on(),
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if self.ctx.demo || m.kind != MouseKind::Click {
            return;
        }
        if let Some((_, i)) = self.rows.iter().find(|(r, _)| hit(r, m.x, m.y)).cloned() {
            if self.phase == Phase::Choose {
                self.sel = i;
                self.choose(i);
            }
        } else if matches!(self.phase, Phase::Intro | Phase::Result) {
            self.go_on();
        }
    }

    fn tick(&mut self, dt: f32) {
        self.cheer = (self.cheer - dt * 0.25).max(0.0);
        match self.phase {
            Phase::Passing => {
                self.anim += dt / 1.5;
                if self.anim >= 1.0 {
                    self.finish_pass();
                    self.demo_clock = 0.0;
                }
            }
            Phase::Tossed => {
                self.anim += dt / 2.6;
                if self.anim >= 1.0 {
                    self.after_toss();
                    self.demo_clock = 0.0;
                }
            }
            Phase::Pega => {
                self.anim += dt / 2.2;
                if self.anim >= 1.0 {
                    self.finish_pega();
                    self.demo_clock = 0.0;
                }
            }
            Phase::LeadOut => {
                self.anim += dt / 4.0;
                if self.anim >= 1.0 {
                    self.result();
                    self.demo_clock = 0.0;
                }
            }
            _ => {}
        }
        if self.ctx.demo {
            self.demo_clock += dt;
            match self.phase {
                Phase::Intro if self.demo_clock > 1.5 => {
                    self.demo_clock = 0.0;
                    self.go_on();
                }
                Phase::Choose if self.demo_clock > 1.1 => {
                    self.demo_clock = 0.0;
                    let i = self.demo_choice();
                    self.sel = i;
                    self.ctx.audio.play(Sound::Tick);
                    self.choose(i);
                }
                Phase::Result if self.demo_clock > 4.0 => {
                    self.demo_clock = 0.0;
                    self.new_fight();
                }
                _ => {}
            }
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "phase {:?} stage {:?} passes {} streak {} energy {:.0} score {:.0} tosses {}",
            self.phase,
            self.stage,
            self.passes,
            self.streak,
            self.bull.energy,
            self.score,
            self.tosses
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.rows.clear();
        let status = format!("{}  applause {:.0}", self.stage.name(), self.score);
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("1-4", "choose"),
                ("↑↓", "move"),
                ("Enter", "go"),
                ("N", "new bull"),
                ("Tab", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 76, 22) {
            return;
        }
        let side_w = 32u16.min(inner.width / 2);
        let menu_h = 8u16;
        let ring_r = Rect {
            width: inner.width - side_w,
            height: inner.height - menu_h,
            ..inner
        };
        let side = Rect {
            x: inner.x + ring_r.width,
            width: side_w,
            height: inner.height - menu_h,
            ..inner
        };
        let low = Rect {
            y: inner.bottom() - menu_h,
            height: menu_h,
            ..inner
        };
        self.draw_ring(buf, ring_r);
        self.draw_side(buf, side);
        self.draw_menu(buf, low);
    }
}

impl Bullfight {
    fn draw_ring(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "The plaza", AMBER);
        let sand = Color::Rgb(214, 182, 120);
        let sand_dk = Color::Rgb(196, 162, 100);
        let wood = Color::Rgb(150, 40, 30);
        let cx = p.x as f32 + p.width as f32 / 2.0;
        let cy = p.y as f32 + p.height as f32 / 2.0 + 0.5;
        let rx = p.width as f32 / 2.0 - 4.0;
        let ry = p.height as f32 / 2.0 - 1.4;
        let t = self.anim;
        // the stands, the barrier, the sand
        let mut seed = (self.passes * 7 + self.tosses * 13 + (t * 6.0) as u32) as u64 + 1;
        for y in p.top()..p.bottom() {
            for x in p.left()..p.right() {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let d = dx * dx + dy * dy;
                let cell = &mut buf[(x, y)];
                if d <= 1.0 {
                    let rake = (x + y * 3) % 7 == 0;
                    cell.set_symbol(if rake { "·" } else { " " })
                        .set_style(on(sand_dk, sand));
                } else if d <= 1.18 {
                    cell.set_symbol("█").set_style(on(wood, BG));
                } else {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    let k = (seed % 100) as f32 / 100.0;
                    let lively = 0.25 + 0.6 * self.cheer;
                    let (sym, c) = if k < lively * 0.4 {
                        ("o", Color::Rgb(230, 200, 160))
                    } else if k < lively {
                        ("∙", Color::Rgb(180, 150, 120))
                    } else {
                        (".", Color::Rgb(90, 80, 70))
                    };
                    let hat = self.cheer > 0.6 && k > 0.97;
                    cell.set_symbol(if hat { "^" } else { sym })
                        .set_style(st(if hat { AMBER } else { c }));
                }
            }
        }
        let gate_y = cy as u16;
        let gate_x = (cx + rx + 0.6) as u16;
        for gy in gate_y.saturating_sub(1)..=gate_y + 1 {
            if gy < p.bottom() && gate_x < p.right() {
                buf[(gate_x, gy)]
                    .set_symbol(" ")
                    .set_style(on(sand_dk, sand));
            }
        }
        let at = |buf: &mut Buffer, s: &Spot, text: &str, fg: Color| {
            let x = (cx + s.x).round() as i32;
            let y = (cy + s.y).round() as i32;
            let mut xx = x;
            for ch in text.chars() {
                if xx >= p.x as i32
                    && xx < p.right() as i32
                    && y >= p.y as i32
                    && y < p.bottom() as i32
                {
                    buf[(xx as u16, y as u16)]
                        .set_symbol(&ch.to_string())
                        .set_style(on(fg, sand));
                }
                xx += 1;
            }
        };
        let bull_c = Color::Rgb(30, 22, 18);
        let gold = Color::Rgb(255, 210, 60);
        let cape_c = if self.stage == Stage::Muleta {
            Color::Rgb(200, 20, 30)
        } else {
            Color::Rgb(220, 60, 150)
        };
        let me = Spot { x: 0.0, y: 0.0 };
        let span = rx - 3.0;
        match self.phase {
            Phase::Passing => {
                let (pass, out) = self.current.unwrap();
                let dir = if self.from_left { 1.0 } else { -1.0 };
                let bx = -dir * span + dir * 2.0 * span * t;
                let by = if pass.close { 1.0 } else { 2.0 };
                let caught = out == Outcome::Caught && (t - 0.5).abs() < 0.08;
                let bull = if dir > 0.0 {
                    "~███▶"
                } else {
                    "◀███~"
                };
                at(buf, &Spot { x: bx - 2.0, y: by }, bull, bull_c);
                if !caught {
                    at(buf, &me, "Å", gold);
                }
                // the cape swings across as the horns go by
                let side = if (t < 0.5) == (dir > 0.0) { -1.0 } else { 1.0 };
                let cape_x = if (t - 0.5).abs() < 0.12 {
                    -0.5
                } else {
                    side * 1.5
                };
                at(buf, &Spot { x: cape_x, y: 1.0 }, "▓▓", cape_c);
            }
            Phase::Tossed => {
                let lift = if t < 0.3 {
                    (t / 0.3 * std::f32::consts::PI).sin() * 2.5
                } else {
                    0.0
                };
                let me_y = -lift;
                let sym = if t < 0.3 { "Å" } else { "_Å_" };
                let mx = if t < 0.3 { 0.0 } else { -1.0 };
                at(buf, &Spot { x: mx, y: me_y }, sym, gold);
                let bx = 4.0 + t * span * 0.6;
                at(buf, &Spot { x: bx, y: 1.0 }, "~███▶", bull_c);
                // the team runs in with their capes and draws him off
                let n = (t * 4.0).min(3.0) as i32;
                for i in 0..n {
                    let k = (t - 0.2 * i as f32).clamp(0.0, 1.0);
                    let tx = -span + (span - 3.0 + i as f32 * 2.0) * k;
                    at(
                        buf,
                        &Spot {
                            x: tx,
                            y: -1.0 + i as f32,
                        },
                        "o▓",
                        Color::Rgb(60, 60, 140),
                    );
                }
            }
            Phase::Pega => {
                let bx = span - 2.0 * span * (t.min(0.6) / 0.6) * 0.75;
                at(
                    buf,
                    &Spot {
                        x: bx - 2.0,
                        y: 0.0,
                    },
                    "◀███~",
                    bull_c,
                );
                for i in 0..8 {
                    let fx = -span * 0.55 + i as f32 * 0.9;
                    let fy = if i % 2 == 0 { -0.4 } else { 0.6 };
                    let pile = t > 0.6 && self.pega_ok;
                    let scatter = t > 0.6 && !self.pega_ok;
                    let (x, y) = if pile {
                        (bx - 3.0 - (i as f32 * 0.6), fy)
                    } else if scatter {
                        (
                            fx - (i as f32 - 3.5) * 2.0 * (t - 0.6),
                            fy * 4.0 * (t - 0.4),
                        )
                    } else {
                        (fx, fy)
                    };
                    at(buf, &Spot { x, y }, "♀", Color::Rgb(40, 90, 50));
                }
            }
            Phase::LeadOut | Phase::Result => {
                let k = if self.phase == Phase::Result { 1.0 } else { t };
                let start = -span * 0.4;
                let gate = rx + 2.0;
                let x = start + (gate - start) * k;
                at(
                    buf,
                    &Spot {
                        x: x + 6.0,
                        y: -1.0,
                    },
                    "~▒▒▒▶",
                    Color::Rgb(160, 120, 70),
                );
                at(
                    buf,
                    &Spot { x: x + 6.0, y: 1.0 },
                    "~▒▒▒▶",
                    Color::Rgb(160, 120, 70),
                );
                at(buf, &Spot { x, y: 0.0 }, "~███▶", bull_c);
                at(
                    buf,
                    &Spot {
                        x: -span * 0.6,
                        y: 2.0,
                    },
                    "Å",
                    gold,
                );
                if self.phase == Phase::Result && self.score >= 80.0 {
                    for i in 0..6 {
                        let fx = -span + i as f32 * span / 3.0;
                        at(
                            buf,
                            &Spot {
                                x: fx,
                                y: -ry + 1.5 + (i % 2) as f32,
                            },
                            "✿",
                            Color::Rgb(220, 40, 90),
                        );
                    }
                }
            }
            _ => {
                let dir = if self.from_left { -1.0 } else { 1.0 };
                let bx = dir * span * 0.8;
                let bull = if dir < 0.0 {
                    "~███▶"
                } else {
                    "◀███~"
                };
                at(
                    buf,
                    &Spot {
                        x: bx - 2.0,
                        y: 1.0,
                    },
                    bull,
                    bull_c,
                );
                at(buf, &me, "Å", gold);
                at(
                    buf,
                    &Spot {
                        x: -dir * 1.5,
                        y: 1.0,
                    },
                    "▓▓",
                    cape_c,
                );
            }
        }
        // the crowd's voice along the bottom of the stands
        let y = p.bottom() - 1;
        let w = (self.crowd.chars().count() as u16).min(p.width.saturating_sub(2));
        let x = p.x + p.width.saturating_sub(w + 2) / 2;
        fill(buf, Rect::new(x, y, (w + 2).min(p.width), 1), st(INK));
        put_max(buf, x + 1, y, &self.crowd, bold(st(AMBER)), w);
    }

    fn draw_side(&self, buf: &mut Buffer, r: Rect) {
        if self.show_help {
            let p = panel(buf, r, "How it works", CYAN);
            para(
                buf,
                p,
                "No swords, no harm: the bull leaves the ring alive. Close passes earn the most applause and carry the most risk, more with each daring pass in a row; a flourish (Media veronica, Pase de pecho) settles things. A brave bull earns you more; a fast one is more dangerous. Caught twice, you rest. Finish with the forcados' catch, or the steers.",
                st(INK),
            );
            return;
        }
        let p = panel(buf, r, &format!("The bull: {}", self.bull.name), AMBER);
        let rows = [
            (format!("bravery  {}", quality_word(self.bull.bravery)), INK),
            (format!("speed    {}", quality_word(self.bull.speed)), INK),
            (format!("weight   {} kg", self.bull.weight), DIM),
            (format!("your team {}", quality_word(self.team + 1)), DIM),
        ];
        for (i, (s, c)) in rows.iter().enumerate() {
            if p.y + (i as u16) < p.bottom() {
                put_max(buf, p.x, p.y + i as u16, s, st(*c), p.width);
            }
        }
        let y = p.y + 4;
        if y < p.bottom() {
            put(buf, p.x, y, "charge", st(DIM));
            bar(
                buf,
                p.x + 9,
                y,
                p.width.saturating_sub(9),
                self.bull.energy / 100.0,
                RED,
            );
        }
        let y = y + 1;
        if y < p.bottom() {
            put(buf, p.x, y, "applause", st(DIM));
            bar(
                buf,
                p.x + 9,
                y,
                p.width.saturating_sub(9),
                self.score / 120.0,
                GREEN,
            );
        }
        let y = y + 1;
        if y < p.bottom() && self.stage != Stage::Finale {
            let d = danger(&self.passes_now()[0], &self.bull, self.team, self.streak);
            let word = if d > 0.3 {
                "very risky"
            } else if d > 0.18 {
                "risky"
            } else if d > 0.1 {
                "steady"
            } else {
                "calm"
            };
            put_max(
                buf,
                p.x,
                y,
                &format!(
                    "{} {}/{}  nerves {word}",
                    if self.stage == Stage::Cape {
                        "cape"
                    } else {
                        "muleta"
                    },
                    self.passes,
                    self.stage.most()
                ),
                st(if d > 0.18 { AMBER } else { DIM }),
                p.width,
            );
        }
        let y = y + 2;
        let keep = if self.phase == Phase::Result { 1 } else { 0 };
        let rows = p.bottom().saturating_sub(y + keep) as usize;
        let skip = self.log.len().saturating_sub(rows);
        for (i, (s, c)) in self.log.iter().skip(skip).enumerate() {
            put_max(buf, p.x, y + i as u16, s, st(*c), p.width);
        }
        if self.phase == Phase::Result {
            let best = self.stats.get::<f32>("best").unwrap_or(0.0);
            let s = format!(
                "best {:.0}  fights {}",
                best,
                self.stats.get::<u32>("fights").unwrap_or(0)
            );
            fill(buf, Rect::new(p.x, p.bottom() - 1, p.width, 1), st(DIM));
            put_max(buf, p.x, p.bottom() - 1, &s, st(DIM), p.width);
        }
    }

    fn draw_menu(&mut self, buf: &mut Buffer, r: Rect) {
        let title = match self.phase {
            Phase::Intro => "The bull comes in",
            Phase::Result => "The ovation",
            Phase::Choose if self.stage == Stage::Finale => "The finale",
            _ => "Your pass",
        };
        let p = panel(buf, r, title, CYAN);
        let lines = wrap(&self.note, p.width as usize);
        let mut y = p.y;
        for l in lines.iter().take(2) {
            put_max(buf, p.x, y, l, bold(st(CYAN)), p.width);
            y += 1;
        }
        match self.phase {
            Phase::Choose => {
                let items: Vec<String> = self
                    .choices()
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("{} {s}", i + 1))
                    .collect();
                let rows = menu(
                    buf,
                    Rect::new(p.x, y, p.width, p.bottom().saturating_sub(y)),
                    &items,
                    self.sel,
                );
                self.rows = rows.into_iter().enumerate().map(|(i, r)| (r, i)).collect();
            }
            Phase::Result => {
                let (t, words) = ovation(self.score);
                put(
                    buf,
                    p.x,
                    y,
                    &format!("{t}  ({:.0})", self.score),
                    bold(st(GREEN)),
                );
                let mut text = words.to_string();
                if self.bull.bravery >= 4 {
                    text += &format!(
                        " {} is applauded too, as a brave bull is, on his way out to pasture.",
                        self.bull.name
                    );
                }
                para(
                    buf,
                    Rect::new(p.x, y + 1, p.width, p.bottom().saturating_sub(y + 2)),
                    &text,
                    st(INK),
                );
                put(
                    buf,
                    p.x,
                    p.bottom() - 1,
                    "Enter: another afternoon",
                    st(DIM),
                );
            }
            Phase::Intro => {
                put(
                    buf,
                    p.x,
                    y + 1,
                    &format!(
                        "He looks {} and {}.",
                        match self.bull.bravery {
                            1 | 2 => "wary",
                            3 => "willing",
                            _ => "bold",
                        },
                        match self.bull.speed {
                            1 | 2 => "steady on his feet",
                            3 => "quick",
                            _ => "very fast",
                        }
                    ),
                    st(INK),
                );
                put(
                    buf,
                    p.x,
                    p.bottom() - 1,
                    "Enter: step out with the cape",
                    st(DIM),
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bull(bravery: u8, speed: u8) -> Bull {
        Bull {
            name: "Test",
            bravery,
            speed,
            weight: 500,
            energy: 100.0,
        }
    }

    #[test]
    fn close_passes_are_riskier_and_a_run_of_them_more_so() {
        let b = bull(3, 3);
        let close = danger(&CAPE[0], &b, 2, 0);
        let wide = danger(&CAPE[1], &b, 2, 0);
        assert!(close > wide);
        assert!(danger(&CAPE[0], &b, 2, 3) > close);
        assert!(danger(&CAPE[0], &b, 4, 0) < close);
        assert!(danger(&CAPE[0], &bull(3, 5), 2, 0) > close);
    }

    #[test]
    fn a_brave_bull_earns_more_and_repeats_earn_less() {
        let a = applause(&MULETA[0], &bull(5, 3), Stage::Muleta, 0, 0);
        let b = applause(&MULETA[0], &bull(1, 3), Stage::Muleta, 0, 0);
        assert!(a > b);
        assert!(applause(&MULETA[0], &bull(5, 3), Stage::Muleta, 0, 2) < a);
    }

    #[test]
    fn a_tired_bull_is_easier_to_catch() {
        let mut b = bull(3, 3);
        let fresh = pega_chance(&b, 2);
        b.energy = 20.0;
        assert!(pega_chance(&b, 2) > fresh);
    }

    #[test]
    fn ovations_rise_with_the_score() {
        assert_eq!(ovation(5.0).0, "Polite silence");
        assert_eq!(ovation(70.0).0, "A lap of the ring");
        assert_eq!(ovation(140.0).0, "Out on shoulders");
    }

    #[test]
    fn a_whole_demo_fight_ends_with_the_bull_led_out() {
        let mut g = Bullfight::new(Ctx {
            audio: crate::audio::Audio::silent(),
            rng: Rng::new(11),
            demo: true,
        });
        let mut saw_lead_out = false;
        for _ in 0..20_000 {
            g.tick(0.05);
            if g.phase == Phase::LeadOut {
                saw_lead_out = true;
            }
            if g.phase == Phase::Result {
                break;
            }
        }
        assert!(saw_lead_out);
        assert_eq!(g.phase, Phase::Result);
    }
}
