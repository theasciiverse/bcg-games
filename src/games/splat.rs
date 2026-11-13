//! Splat: a parachute jump on a random world. Set the free-fall timer to
//! open the chute as late as you dare. Rules in asciiverse/specs/splat.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Surface gravity in feet per second per second.
pub const WORLDS: &[(&str, f64)] = &[
    ("Mercury", 12.1),
    ("Venus", 29.1),
    ("Earth", 32.2),
    ("the Moon", 5.3),
    ("Mars", 12.2),
    ("Jupiter", 81.3),
    ("Saturn", 34.3),
    ("Uranus", 28.5),
    ("Neptune", 36.6),
    ("the Sun", 899.0),
];

#[derive(Clone, Debug)]
pub struct Jump {
    pub world: usize,
    pub altitude: f64,
    /// as told to the jumper
    pub g: f64,
    pub terminal: f64,
    /// as it really is, up to 5% off
    pub true_g: f64,
    pub true_terminal: f64,
}

impl Jump {
    pub fn random(rng: &mut Rng) -> Jump {
        let world = rng.range(0, WORLDS.len() as i64 - 1) as usize;
        let g = WORLDS[world].1;
        let terminal = rng.range(60, 1000) as f64 * 5280.0 / 3600.0; // mph to ft/s
        Jump {
            world,
            altitude: rng.range(1000, 9500) as f64,
            g,
            terminal,
            true_g: g * rng.between(0.95, 1.05) as f64,
            true_terminal: terminal * rng.between(0.95, 1.05) as f64,
        }
    }

    /// Feet fallen after `t` seconds, with the true values.
    pub fn fallen(&self, t: f64) -> f64 {
        fallen(self.true_g, self.true_terminal, t)
    }

    /// Seconds to the ground, by the values the jumper was told.
    pub fn told_time_to_ground(&self) -> f64 {
        time_to(self.g, self.terminal, self.altitude)
    }
}

pub fn fallen(g: f64, vt: f64, t: f64) -> f64 {
    let t_term = vt / g;
    if t <= t_term {
        g * t * t / 2.0
    } else {
        vt * vt / (2.0 * g) + vt * (t - t_term)
    }
}

pub fn time_to(g: f64, vt: f64, h: f64) -> f64 {
    let t_term = vt / g;
    let h_term = vt * vt / (2.0 * g);
    if h <= h_term {
        (2.0 * h / g).sqrt()
    } else {
        t_term + (h - h_term) / vt
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Setting,
    Falling,
    Opened(f64),
    Splat,
}

pub struct Splat {
    ctx: Ctx,
    jump: Jump,
    phase: Phase,
    timer: String,
    t: f64,
    marks: Vec<(f64, f64)>,
    /// heights the chute opened at, on earlier good jumps
    history: Vec<f64>,
    quit: bool,
    demo_clock: f32,
    beep: f64,
}

impl Splat {
    fn load_history() -> Vec<f64> {
        crate::store::read("splat", "jumps")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect()
    }

    fn new_jump(&mut self) {
        self.jump = Jump::random(&mut self.ctx.rng);
        self.phase = Phase::Setting;
        self.timer.clear();
        self.t = 0.0;
        self.marks.clear();
        log::info!("new jump {:?}", self.jump);
    }

    fn timer_value(&self) -> Option<f64> {
        self.timer
            .parse::<f64>()
            .ok()
            .filter(|v| *v > 0.0 && *v < 1000.0)
    }

    fn go(&mut self) {
        let Some(t) = self.timer_value() else {
            self.ctx.audio.play(Sound::Nope);
            return;
        };
        log::info!("jump with timer {t}");
        self.phase = Phase::Falling;
        self.t = 0.0;
        self.marks.clear();
        self.beep = 0.0;
        self.ctx.audio.play(Sound::Click);
    }

    fn land(&mut self, opened_at: Option<f64>) {
        match opened_at {
            Some(h) => {
                self.phase = Phase::Opened(h);
                self.ctx.audio.play(Sound::Good);
                if !self.ctx.demo {
                    self.history.push(h);
                    let text: String = self.history.iter().map(|h| format!("{h:.1}\n")).collect();
                    crate::store::write(Self::ID, "jumps", &text);
                }
            }
            None => {
                self.phase = Phase::Splat;
                self.ctx.audio.play(Sound::Boom);
            }
        }
        log::info!("landed {:?}", self.phase);
    }

    /// How this good jump ranks against the earlier good ones.
    fn verdict(&self, h: f64) -> String {
        let earlier: Vec<f64> = self.history.iter().copied().filter(|&x| x != h).collect();
        if earlier.is_empty() {
            return "Your first good jump! Every one after this one is measured against it.".into();
        }
        let lower = earlier.iter().filter(|&&x| x < h).count();
        let pct = 100.0 * (earlier.len() - lower) as f64 / earlier.len() as f64;
        let n = earlier.len() + 1;
        if lower == 0 {
            format!("The latest opening of all {n} good jumps. Superb nerve!")
        } else if pct >= 75.0 {
            format!(
                "Later than {pct:.0}% of your {} earlier good jumps. Splendid.",
                earlier.len()
            )
        } else if pct >= 50.0 {
            format!("Better than half your earlier jumps ({pct:.0}%). Good.")
        } else if pct >= 25.0 {
            format!("Only {pct:.0}% of your earlier jumps opened higher. A bit timid.")
        } else {
            format!(
                "Nearly every earlier jump opened lower ({:.0}%). Were you nervous?",
                100.0 - pct
            )
        }
    }
}

impl Game for Splat {
    const ID: &'static str = "splat";
    const TITLE: &'static str = "Splat";
    const HELP: &'static str =
        "A parachute jump on a world picked at random, from the Moon to the Sun.
You are told the height, the pull of gravity and your terminal speed,
each up to 5% off, and set the timer that opens your chute. Open it as
late as you dare: every good jump is ranked against your earlier ones.
Type the seconds (a point for tenths) and press Enter to jump.";

    fn new(ctx: Ctx) -> Self {
        let mut ctx = ctx;
        let jump = Jump::random(&mut ctx.rng);
        let demo = ctx.demo;
        let mut g = Splat {
            ctx,
            jump,
            phase: Phase::Setting,
            timer: String::new(),
            t: 0.0,
            marks: Vec::new(),
            history: if demo {
                Vec::new()
            } else {
                Splat::load_history()
            },
            quit: false,
            demo_clock: 0.0,
            beep: 0.0,
        };
        g.new_jump();
        g
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        match (self.phase, k) {
            (_, Key::Esc) | (_, Key::Char('q')) => self.quit = true,
            (_, Key::Char('m')) => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            (Phase::Setting, Key::Char(c))
                if c.is_ascii_digit() || (c == '.' && !self.timer.contains('.')) =>
            {
                if self.timer.len() < 6 {
                    self.timer.push(c);
                    self.ctx.audio.play(Sound::Tick);
                }
            }
            (Phase::Setting, Key::Backspace) => {
                self.timer.pop();
            }
            (Phase::Setting, Key::Up) => {
                self.timer = format!("{:.1}", self.timer_value().unwrap_or(0.0) + 0.5)
            }
            (Phase::Setting, Key::Down) => {
                self.timer = format!("{:.1}", (self.timer_value().unwrap_or(0.5) - 0.5).max(0.1))
            }
            (Phase::Setting, Key::Tab) => self.new_jump(),
            (Phase::Setting, Key::Enter) => self.go(),
            (Phase::Opened(_) | Phase::Splat, Key::Enter | Key::Char(' ')) => self.new_jump(),
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if m.kind == MouseKind::Click && matches!(self.phase, Phase::Opened(_) | Phase::Splat) {
            self.new_jump();
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.ctx.demo {
            self.demo_clock += dt;
            match self.phase {
                Phase::Setting => {
                    // a jumper who trusts the numbers to within a few per cent
                    let target = self.jump.told_time_to_ground() * 0.9;
                    let s = format!("{target:.1}");
                    let n = ((self.demo_clock - 0.8) / 0.25).max(0.0) as usize;
                    self.timer = s.chars().take(n).collect();
                    if self.demo_clock > 0.8 + 0.25 * s.len() as f32 + 0.6 {
                        self.timer = s;
                        self.go();
                        self.demo_clock = 0.0;
                    }
                }
                Phase::Opened(_) | Phase::Splat => {
                    if self.demo_clock > 3.5 {
                        self.new_jump();
                        self.demo_clock = 0.0;
                    }
                }
                Phase::Falling => {}
            }
        }
        if self.phase != Phase::Falling {
            return;
        }
        let timer = self.timer_value().unwrap_or(0.0);
        // a long fall plays out in about five seconds, a short one at life speed
        let speed = (timer / 5.0).max(1.0);
        self.t += dt as f64 * speed;
        // eight reports on the way down
        let next = (self.marks.len() + 1) as f64 * timer / 8.0;
        if self.t >= next && self.marks.len() < 8 {
            let h = self.jump.altitude - self.jump.fallen(next);
            self.marks.push((next, h));
        }
        let left = self.jump.altitude - self.jump.fallen(self.t.min(timer));
        self.beep += dt as f64;
        if self.beep > 0.25 {
            self.beep = 0.0;
            self.ctx
                .audio
                .play(Sound::Hiss((self.t / timer).min(1.0) as f32));
        }
        if left <= 0.0 {
            self.land(None);
        } else if self.t >= timer {
            self.land(Some(left));
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "phase {:?} world {} alt {:.0} timer {:?} t {:.2}",
            self.phase, WORLDS[self.jump.world].0, self.jump.altitude, self.timer, self.t
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let j = &self.jump;
        let world = WORLDS[j.world].0;
        let status = format!("on {world}");
        let keys: &[(&str, &str)] = match self.phase {
            Phase::Setting => &[
                ("0-9 .", "timer"),
                ("↑↓", "±0.5 s"),
                ("Enter", "jump"),
                ("Tab", "another jump"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
            Phase::Falling => &[("Esc", "quit")],
            _ => &[("Enter", "next jump"), ("Esc", "quit")],
        };
        let inner = frame(buf, area, "Splat", &status, keys);
        if too_small(buf, inner, 70, 22) {
            return;
        }
        let sky_w = 30.min(inner.width / 3);
        let sky = Rect {
            width: sky_w,
            ..inner
        };
        let right = Rect {
            x: inner.x + sky_w,
            width: inner.width - sky_w,
            ..inner
        };

        // the drop: a column of sky, the jumper and the ground
        let p = panel(buf, sky, "", CYAN);
        let gy = p.bottom() - 1;
        for x in p.x..p.right() {
            buf[(x, gy)].set_symbol("▀").set_style(on(
                ratatui::style::Color::Rgb(90, 130, 70),
                ratatui::style::Color::Rgb(60, 80, 40),
            ));
        }
        let fallen = match self.phase {
            Phase::Setting => 0.0,
            Phase::Opened(h) => j.altitude - h,
            Phase::Splat => j.altitude,
            Phase::Falling => j.fallen(self.t),
        };
        let frac = (fallen / j.altitude).clamp(0.0, 1.0);
        let rows = (gy - p.y).saturating_sub(2) as f64;
        let y = p.y + (frac * rows).round() as u16;
        let x = p.x + p.width / 2 - 1;
        match self.phase {
            Phase::Splat => {
                put(buf, x - 2, gy.saturating_sub(1), "\\ ✶ /", bold(st(RED)));
            }
            Phase::Opened(_) => {
                put(buf, x - 2, y, "╭───╮", bold(st(AMBER)));
                put(buf, x - 2, y + 1, " \\ / ", st(INK));
                put(buf, x, y + 2, "o", bold(st(INK)));
            }
            _ => {
                put(buf, x, y, "\\o/", bold(st(INK)));
                put(buf, x + 1, y + 1, "▼", st(INK));
            }
        }
        // a few altitude ticks
        for k in 0..=4 {
            let f = k as f64 / 4.0;
            let ty = p.y + (f * rows).round() as u16;
            put(
                buf,
                p.x,
                ty,
                &format!("{:>5.0}", j.altitude * (1.0 - f)),
                st(DIM),
            );
        }

        let info_h = 9.min(right.height);
        let s = panel(
            buf,
            Rect {
                height: info_h,
                ..right
            },
            &format!("Jumping on {world}"),
            AMBER,
        );
        let lines = [
            format!("height             {:.0} ft", j.altitude),
            format!("gravity            {:.1} ft/s² (±5%)", j.g),
            format!(
                "terminal speed     {:.0} mph (±5%)",
                j.terminal * 3600.0 / 5280.0
            ),
        ];
        for (i, l) in lines.iter().enumerate() {
            put(buf, s.x, s.y + i as u16, l, st(INK));
        }
        let ty = s.y + 4;
        put(
            buf,
            s.x,
            ty,
            "free-fall timer",
            st(if self.phase == Phase::Setting {
                INK
            } else {
                DIM
            }),
        );
        let field = Rect::new(s.x + 19, ty, 10, 1);
        let focus = self.phase == Phase::Setting;
        fill(
            buf,
            field,
            on(
                AMBER,
                if focus {
                    ratatui::style::Color::Rgb(40, 34, 20)
                } else {
                    PAPER
                },
            ),
        );
        let t = if self.timer.is_empty() {
            "_"
        } else {
            &self.timer
        };
        put(
            buf,
            field.x + 1,
            ty,
            &format!("{t} s"),
            bold(on(
                AMBER,
                if focus {
                    ratatui::style::Color::Rgb(40, 34, 20)
                } else {
                    PAPER
                },
            )),
        );

        let r2 = Rect {
            y: right.y + info_h,
            height: right.height - info_h,
            ..right
        };
        let half = r2.width / 2;
        let lp = panel(buf, Rect { width: half, ..r2 }, "On the way down", CYAN);
        put(buf, lp.x, lp.y, " second    feet to go", st(DIM));
        for (i, (t, h)) in self.marks.iter().enumerate() {
            put(
                buf,
                lp.x,
                lp.y + 1 + i as u16,
                &format!("{t:>7.2}  {h:>11.0}", h = h.max(0.0)),
                st(if *h <= 0.0 { RED } else { INK }),
            );
        }
        let rp = panel(
            buf,
            Rect {
                x: r2.x + half,
                width: r2.width - half,
                ..r2
            },
            "Result",
            AMBER,
        );
        match self.phase {
            Phase::Setting => {
                para(buf, rp, "Set the timer: when it runs out, the chute opens. Open as close to the ground as you dare.", st(DIM));
            }
            Phase::Falling => {
                put(
                    buf,
                    rp.x,
                    rp.y,
                    &format!("falling... {:.1} s", self.t),
                    bold(st(CYAN)),
                );
            }
            Phase::Opened(h) => {
                let used = para(
                    buf,
                    rp,
                    &format!("Chute open at {h:.0} feet."),
                    bold(st(GREEN)),
                );
                para(
                    buf,
                    Rect {
                        y: rp.y + used + 1,
                        height: rp.height.saturating_sub(used + 1),
                        ..rp
                    },
                    &self.verdict(h),
                    st(INK),
                );
            }
            Phase::Splat => {
                let rest = time_to(j.true_g, j.true_terminal, j.altitude);
                para(buf, rp, &format!("SPLAT. You hit {world} after {rest:.2} seconds, with the chute still packed. A small crater marks the spot; may it rest in peace."), bold(st(RED)));
            }
        }
        if !self.history.is_empty() {
            let y = rp.bottom().saturating_sub(1);
            put(
                buf,
                rp.x,
                y,
                &format!("{} good jumps so far", self.history.len()),
                st(DIM),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fall_matches_the_formulas() {
        // no terminal speed reached: h = g t^2 / 2
        assert!((fallen(32.2, 1e9, 2.0) - 64.4).abs() < 1e-9);
        // terminal reached at 1 s, then steady
        assert!((fallen(10.0, 10.0, 3.0) - (5.0 + 20.0)).abs() < 1e-9);
        let t = time_to(32.2, 176.0, 5000.0);
        assert!((fallen(32.2, 176.0, t) - 5000.0).abs() < 1e-6);
    }

    #[test]
    fn a_ninety_per_cent_timer_survives() {
        let mut rng = Rng::new(4);
        for _ in 0..200 {
            let j = Jump::random(&mut rng);
            let t = j.told_time_to_ground() * 0.9;
            assert!(j.fallen(t) < j.altitude, "{j:?}");
        }
    }
}
