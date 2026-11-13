//! Lunar Descent: bring a landing craft down onto the Moon from orbit by
//! setting how hard the descent engine burns. Rules in
//! asciiverse/specs/lunar-descent.md.

use crate::audio::Sound;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub const START_ALTITUDE: f64 = 120.0; // miles
pub const START_SPEED: f64 = 3600.0; // mph, downward
pub const DRY_MASS: f64 = 16_000.0; // lb
pub const FUEL: f64 = 16_500.0; // lb
pub const GRAVITY: f64 = 0.001; // miles per second per second (3.6 mph each second)
pub const EXHAUST: f64 = 1.8; // miles per second
pub const MAX_BURN: f64 = 200.0; // lb of fuel per second
pub const MIN_BURN: f64 = 8.0; // the engine won't light below this
pub const TURN: f64 = 10.0; // seconds between orders in turn mode
const FEET_PER_MILE: f64 = 5280.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing {
    Perfect,
    Good,
    Rough,
    Wrecked,
    Crater,
}

pub fn judge(mph: f64) -> Landing {
    if mph <= 1.2 {
        Landing::Perfect
    } else if mph <= 10.0 {
        Landing::Good
    } else if mph <= 22.0 {
        Landing::Rough
    } else if mph <= 60.0 {
        Landing::Wrecked
    } else {
        Landing::Crater
    }
}

/// The craft. Speed is downward-positive, in miles per second.
#[derive(Clone, Debug)]
pub struct Craft {
    pub t: f64,
    pub altitude: f64,
    pub speed: f64,
    pub fuel: f64,
    pub burn: f64,
    pub landed: Option<f64>,
}

impl Craft {
    pub fn new() -> Craft {
        Craft {
            t: 0.0,
            altitude: START_ALTITUDE,
            speed: START_SPEED / 3600.0,
            fuel: FUEL,
            burn: 0.0,
            landed: None,
        }
    }

    pub fn mph(&self) -> f64 {
        self.speed * 3600.0
    }

    /// The burn as the engine can give it: nothing below the ignition rate,
    /// nothing at all once the tanks are dry.
    pub fn effective_burn(&self) -> f64 {
        if self.fuel <= 0.0 || self.burn < MIN_BURN {
            0.0
        } else {
            self.burn.min(MAX_BURN)
        }
    }

    /// Advance `dt` seconds, in small steps.
    pub fn step(&mut self, dt: f64) {
        let mut left = dt;
        while left > 0.0 && self.landed.is_none() {
            let h = left.min(0.05);
            left -= h;
            let mass = DRY_MASS + self.fuel;
            let burn = self.effective_burn();
            let used = (burn * h).min(self.fuel);
            // the rocket equation over this step: dv = ve ln(m0/m1)
            let dv = EXHAUST * (mass / (mass - used)).ln();
            let v0 = self.speed;
            self.speed = self.speed + GRAVITY * h - dv;
            self.fuel -= used;
            let fall = (v0 + self.speed) / 2.0 * h;
            if self.altitude - fall <= 0.0 {
                // touch down part way through the step
                let k = if fall > 0.0 {
                    self.altitude / fall
                } else {
                    1.0
                };
                let v = v0 + (self.speed - v0) * k;
                self.t += h * k;
                self.altitude = 0.0;
                self.speed = v.max(0.0);
                self.landed = Some(v.max(0.0) * 3600.0);
                return;
            }
            self.altitude -= fall;
            self.t += h;
            if self.altitude > START_ALTITUDE * 3.0 {
                // flew off into space: counted as lost
                self.landed = Some(f64::INFINITY);
            }
        }
    }

    /// Touchdown speed in mph if the burn were held as it is, or None if
    /// the craft would stop and climb first.
    pub fn projection(&self) -> Option<f64> {
        let mut c = self.clone();
        for _ in 0..4000 {
            c.step(0.5);
            if let Some(v) = c.landed {
                return Some(v);
            }
            if c.speed < 0.0 {
                return None;
            }
        }
        None
    }

    /// An autopilot's burn: aim for a speed that shrinks with height.
    pub fn autopilot(&self) -> f64 {
        let h = self.altitude.max(1e-6);
        let mass = DRY_MASS + self.fuel;
        let full = MAX_BURN * EXHAUST / mass;
        let v = self.speed;
        let want_accel = if h * FEET_PER_MILE < 150.0 {
            // the last few yards: settle at a walking pace
            (v - 2.0 / 3600.0) / 1.0 + GRAVITY
        } else {
            // the braking that would stop us 120 feet up, plus gravity;
            // wait until it needs most of the engine, then follow it
            let need = v * v / (2.0 * (h - 120.0 / FEET_PER_MILE).max(1e-6)) + GRAVITY;
            if need < full * 0.8 {
                return 0.0;
            }
            need
        };
        if want_accel <= 0.0 {
            return 0.0;
        }
        let rate = want_accel * mass / EXHAUST;
        if rate < MIN_BURN {
            0.0
        } else {
            rate.clamp(MIN_BURN, MAX_BURN).round()
        }
    }
}

impl Default for Craft {
    fn default() -> Self {
        Craft::new()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// time runs on; change the burn whenever you like
    Live,
    /// time stops every ten seconds for the next order, as at the console
    Turns,
}

pub struct Lunar {
    ctx: Ctx,
    craft: Craft,
    mode: Mode,
    paused: bool,
    waiting: bool,
    speedup: f64,
    log: Vec<(f64, f64, f64, f64, f64)>,
    next_log: f64,
    record: Kv,
    quit: bool,
    hiss: f32,
    demo_clock: f32,
    stars: Vec<(u16, u16)>,
}

impl Lunar {
    fn restart(&mut self) {
        self.craft = Craft::new();
        self.log.clear();
        self.next_log = 0.0;
        self.paused = false;
        self.waiting = self.mode == Mode::Turns;
        self.demo_clock = 0.0;
        log::info!("new descent, mode {:?}", self.mode);
    }

    fn set_burn(&mut self, b: f64) {
        let b = b.clamp(0.0, MAX_BURN);
        // between 0 and the ignition rate there is nothing: jump past the gap
        let b = if b > 0.0 && b < MIN_BURN {
            if self.craft.burn >= MIN_BURN {
                0.0
            } else {
                MIN_BURN
            }
        } else {
            b
        };
        if b != self.craft.burn {
            self.craft.burn = b;
            self.ctx.audio.play(Sound::Tick);
        }
    }

    fn finished(&mut self) {
        let Some(v) = self.craft.landed else { return };
        let j = judge(v);
        log::info!(
            "landed at {v:.2} mph after {:.1} s with {:.0} lb fuel: {j:?}",
            self.craft.t,
            self.craft.fuel
        );
        self.ctx.audio.play(match j {
            Landing::Perfect | Landing::Good => Sound::Win,
            Landing::Rough => Sound::Bad,
            _ => Sound::Boom,
        });
        if !self.ctx.demo && v.is_finite() {
            let n = self.record.get::<u32>("descents").unwrap_or(0) + 1;
            self.record.set("descents", n);
            if matches!(j, Landing::Perfect | Landing::Good) {
                let best = self.record.get::<f64>("softest").unwrap_or(f64::MAX);
                if v < best {
                    self.record.set("softest", format!("{v:.2}"));
                }
                let k = self.record.get::<u32>("landed").unwrap_or(0) + 1;
                self.record.set("landed", k);
            }
            self.record.save(Self::ID, "record");
        }
    }

    fn draw_descent(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "Descent", AMBER);
        if p.height < 6 {
            return;
        }
        for &(sx, sy) in &self.stars {
            let x = p.x + sx % p.width.max(1);
            let y = p.y + sy % (p.height - 3).max(1);
            put(buf, x, y, "·", st(LINE));
        }
        // the ground: a row of grey with craters
        let gy = p.bottom() - 1;
        for x in p.x..p.right() {
            let s = if (x * 7 + 3) % 11 == 0 { "▂" } else { "▀" };
            buf[(x, gy)]
                .set_symbol(s)
                .set_style(on(Color::Rgb(150, 146, 136), Color::Rgb(70, 68, 64)));
        }
        // height on a square-root scale, so the last mile gets most of the room
        let rows = (p.height - 2) as f64;
        let frac = (self.craft.altitude / START_ALTITUDE)
            .clamp(0.0, 1.0)
            .sqrt();
        let cy = p.y + ((1.0 - frac) * rows).round() as u16;
        let cx = p.x + p.width / 2 - 2;
        let crashed = self
            .craft
            .landed
            .is_some_and(|v| judge(v) == Landing::Crater || judge(v) == Landing::Wrecked);
        if crashed {
            put(buf, cx - 1, gy.saturating_sub(1), "\\ * /", bold(st(RED)));
            put(
                buf,
                cx - 1,
                gy,
                "~~*~~",
                bold(on(RED, Color::Rgb(70, 68, 64))),
            );
        } else {
            let y = cy.min(gy.saturating_sub(2));
            put(buf, cx, y, "▗█▖", bold(st(INK)));
            put(buf, cx - 1, y + 1, "╱▀▀▀╲", st(INK));
            if self.craft.effective_burn() > 0.0 && self.craft.landed.is_none() {
                let f = self.craft.effective_burn() / MAX_BURN;
                let flame = if f > 0.66 {
                    "▼▼▼"
                } else if f > 0.33 {
                    " ▼ "
                } else {
                    " ▾ "
                };
                put(
                    buf,
                    cx,
                    y + 2,
                    flame,
                    bold(st(if f > 0.66 { AMBER } else { RED })),
                );
            }
        }
        // a scale down the left edge
        for (h, label) in [
            (120.0, "120 mi"),
            (30.0, "30 mi"),
            (5.0, "5 mi"),
            (1.0, "1 mi"),
        ] {
            let fy = p.y + ((1.0 - (h / START_ALTITUDE).sqrt()) * rows).round() as u16;
            if fy < gy {
                put(buf, p.x, fy, &format!("{label:>6} ┤"), st(DIM));
            }
        }
    }

    fn draw_instruments(&self, buf: &mut Buffer, r: Rect) {
        let c = &self.craft;
        let p = panel(buf, r, "Instruments", CYAN);
        let miles = c.altitude.floor();
        let feet = ((c.altitude - miles) * FEET_PER_MILE).floor();
        let rows: Vec<(String, String, Color)> = vec![
            ("time".into(), format!("{:.0} s", c.t), INK),
            ("height".into(), format!("{miles:.0} mi {feet:.0} ft"), INK),
            (
                "falling at".into(),
                format!("{:.1} mph", c.mph()),
                if c.mph() > 200.0 && c.altitude < 2.0 {
                    RED
                } else {
                    INK
                },
            ),
            (
                "fuel".into(),
                format!("{:.0} lb", c.fuel),
                if c.fuel < 2000.0 { AMBER } else { INK },
            ),
            (
                "burn".into(),
                format!("{:.0} lb/s", c.effective_burn()),
                AMBER,
            ),
        ];
        for (i, (k, v, col)) in rows.iter().enumerate() {
            let y = p.y + i as u16;
            if y < p.bottom() {
                put(buf, p.x, y, k, st(DIM));
                put(buf, p.x + 12, y, v, bold(st(*col)));
            }
        }
        let y = p.y + 6;
        if y + 3 < p.bottom() {
            put(buf, p.x, y, "fuel", st(DIM));
            bar(
                buf,
                p.x + 12,
                y,
                p.width.saturating_sub(12),
                (c.fuel / FUEL) as f32,
                AMBER,
            );
            put(buf, p.x, y + 1, "burn", st(DIM));
            bar(
                buf,
                p.x + 12,
                y + 1,
                p.width.saturating_sub(12),
                (c.burn / MAX_BURN) as f32,
                RED,
            );
            if c.burn > 0.0 && c.burn < MIN_BURN {
                put(buf, p.x + 12, y + 2, "below ignition", st(RED));
            }
        }
        let y = p.y + 9;
        if y < p.bottom() && c.landed.is_none() {
            let txt = match c.projection() {
                Some(v) => format!("held like this: down at {v:.0} mph"),
                None => "held like this: you'll climb".into(),
            };
            put_max(buf, p.x, y, &txt, st(DIM), p.width);
        }
        if let Some(v) = c.landed {
            let (msg, col) = match judge(v) {
                _ if !v.is_finite() => ("You burned too long and climbed away into space.".to_string(), RED),
                Landing::Perfect => (format!("A perfect landing at {v:.2} mph! The crew steps out onto the dust."), GREEN),
                Landing::Good => (format!("Down safely at {v:.1} mph. A good landing."), GREEN),
                Landing::Rough => (format!("A hard landing at {v:.1} mph: the crew is fine, the legs are bent, and the ride home is in doubt."), AMBER),
                Landing::Wrecked => (format!("Down at {v:.0} mph. The craft is wrecked; the crew survives in the wreckage, waiting for rescue."), RED),
                Landing::Crater => (format!("You struck the Moon at {v:.0} mph, digging a fresh crater {:.0} feet deep.", v * 0.23), RED),
            };
            let pr = Rect {
                y: p.y + 10,
                height: p.height.saturating_sub(10),
                ..p
            };
            let used = para(buf, pr, &msg, bold(st(col)));
            if pr.y + used + 1 < p.bottom() {
                put(buf, p.x, pr.y + used + 1, "Enter tries again.", st(DIM));
            }
        }
    }

    fn draw_log(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "Log, every 10 s", DIM);
        put(
            buf,
            p.x,
            p.y,
            "  sec      mi   ft      mph     fuel  burn",
            st(DIM),
        );
        let rows = p.height.saturating_sub(1) as usize;
        let skip = self.log.len().saturating_sub(rows);
        for (i, (t, a, v, f, b)) in self.log.iter().skip(skip).enumerate() {
            let miles = a.floor();
            let feet = ((a - miles) * FEET_PER_MILE).floor();
            put_max(
                buf,
                p.x,
                p.y + 1 + i as u16,
                &format!("{t:>5.0} {miles:>7.0} {feet:>4.0} {v:>8.0} {f:>8.0} {b:>5.0}"),
                st(INK),
                p.width,
            );
        }
    }
}

use ratatui::style::Color;

impl Game for Lunar {
    const ID: &'static str = "lunar-descent";
    const TITLE: &'static str = "Lunar Descent";
    const HELP: &'static str =
        "Bring a landing craft down to the Moon from 120 miles up, falling at
3,600 mph, by setting how much fuel the descent engine burns each second:
0 is free fall, 200 is full thrust, and the engine won't light below 8.
Brake too early and the tanks run dry high above the ground; too late and
you dig a crater. Touch down under 10 mph for a good landing.
Up/Down change the burn by 10 (PageUp/PageDown by 50), 0 cuts the engine,
Space pauses, T switches between flying live and stopping every ten
seconds for orders, + and - change the speed of time, Enter starts again.";

    fn new(ctx: Ctx) -> Self {
        let mut ctx = ctx;
        let stars = (0..40)
            .map(|_| (ctx.rng.range(0, 200) as u16, ctx.rng.range(0, 60) as u16))
            .collect();
        let mut g = Lunar {
            ctx,
            craft: Craft::new(),
            mode: Mode::Live,
            paused: false,
            waiting: false,
            speedup: 8.0,
            log: Vec::new(),
            next_log: 0.0,
            record: Kv::load("lunar-descent", "record"),
            quit: false,
            hiss: 0.0,
            demo_clock: 0.0,
            stars,
        };
        if g.ctx.demo {
            g.speedup = 10.0;
        }
        g.restart();
        g
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        let b = self.craft.burn;
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Enter if self.craft.landed.is_some() => self.restart(),
            Key::Enter | Key::Char(' ') if self.waiting => self.waiting = false,
            Key::Char(' ') => self.paused = !self.paused,
            Key::Char('t') => {
                self.mode = if self.mode == Mode::Live {
                    Mode::Turns
                } else {
                    Mode::Live
                };
                self.waiting = false;
                self.ctx.audio.play(Sound::Click);
            }
            Key::Up | Key::Char('w') | Key::Char('k') => self.set_burn(b + 10.0),
            Key::Down | Key::Char('s') | Key::Char('j') => self.set_burn(b - 10.0),
            Key::PageUp => self.set_burn(b + 50.0),
            Key::PageDown => self.set_burn(b - 50.0),
            Key::Char('0') => self.set_burn(0.0),
            Key::Char(c @ '1'..='9') => {
                self.set_burn((c as u8 - b'0') as f64 * 20.0 + if c == '9' { 20.0 } else { 0.0 })
            }
            Key::Char('+') | Key::Char('=') => self.speedup = (self.speedup * 2.0).min(32.0),
            Key::Char('-') => self.speedup = (self.speedup / 2.0).max(1.0),
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        match m.kind {
            MouseKind::ScrollUp => self.set_burn(self.craft.burn + 10.0),
            MouseKind::ScrollDown => self.set_burn(self.craft.burn - 10.0),
            MouseKind::Click if self.craft.landed.is_some() => self.restart(),
            MouseKind::Click if self.waiting => self.waiting = false,
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.ctx.demo {
            if self.craft.landed.is_some() {
                self.demo_clock += dt;
                if self.demo_clock > 4.0 {
                    self.restart();
                }
                return;
            }
            let want = self.craft.autopilot();
            self.craft.burn = want;
        }
        if self.paused || self.craft.landed.is_some() {
            return;
        }
        if self.waiting {
            return;
        }
        while self.craft.t >= self.next_log && self.craft.landed.is_none() {
            let c = &self.craft;
            self.log
                .push((c.t, c.altitude, c.mph(), c.fuel, c.effective_burn()));
            self.next_log += TURN;
            if self.mode == Mode::Turns && c.t > 0.0 {
                self.waiting = true;
                self.ctx.audio.play(Sound::Beep(660.0));
                return;
            }
        }
        // slow time near the ground, so the last seconds can be flown
        let near = (self.craft.altitude * FEET_PER_MILE / 2000.0).clamp(0.25, 1.0);
        let step = dt as f64 * self.speedup * near;
        let until_log = (self.next_log - self.craft.t).max(0.0);
        let step = if self.mode == Mode::Turns {
            step.min(until_log + 1e-9)
        } else {
            step
        };
        self.craft.step(step);
        let burn = self.craft.effective_burn() as f32 / MAX_BURN as f32;
        self.hiss += dt;
        if burn > 0.0 && self.hiss > 0.11 {
            self.hiss = 0.0;
            self.ctx.audio.play(Sound::Hiss(burn));
        }
        if self.craft.landed.is_some() {
            self.finished();
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        let c = &self.craft;
        format!("t {:.1} alt {:.4} mph {:.1} fuel {:.0} burn {:.0} mode {:?} paused {} waiting {} landed {:?}", c.t, c.altitude, c.mph(), c.fuel, c.burn, self.mode, self.paused, self.waiting, c.landed)
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let status = format!(
            "{}  ×{:.0}{}",
            if self.mode == Mode::Live {
                "live"
            } else {
                "10-second turns"
            },
            self.speedup,
            if self.paused { "  paused" } else { "" }
        );
        let keys: &[(&str, &str)] = if self.craft.landed.is_some() {
            &[("Enter", "again"), ("T", "live/turns"), ("Esc", "quit")]
        } else if self.waiting {
            &[
                ("↑↓", "burn"),
                ("0", "cut"),
                ("Enter", "burn for 10 s"),
                ("T", "live"),
                ("Esc", "quit"),
            ]
        } else {
            &[
                ("↑↓", "burn ±10"),
                ("PgUp/Dn", "±50"),
                ("0", "cut"),
                ("Space", "pause"),
                ("+/-", "time"),
                ("T", "turns"),
                ("Esc", "quit"),
            ]
        };
        let inner = frame(buf, area, "Lunar Descent", &status, keys);
        if too_small(buf, inner, 76, 22) {
            return;
        }
        let left_w = inner.width * 2 / 5;
        self.draw_descent(
            buf,
            Rect {
                width: left_w,
                ..inner
            },
        );
        let right = Rect {
            x: inner.x + left_w,
            width: inner.width - left_w,
            ..inner
        };
        let top_h = 15.min(right.height);
        self.draw_instruments(
            buf,
            Rect {
                height: top_h,
                ..right
            },
        );
        self.draw_log(
            buf,
            Rect {
                y: right.y + top_h,
                height: right.height - top_h,
                ..right
            },
        );
        if self.waiting && self.craft.landed.is_none() {
            let msg = format!(
                " Burn rate for the next 10 seconds: {:.0} lb/s, Enter to fire ",
                self.craft.burn
            );
            let w = msg.chars().count() as u16;
            let x = inner.x + inner.width.saturating_sub(w) / 2;
            put(buf, x, inner.bottom() - 1, &msg, bold(on(BG, AMBER)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_fall_makes_a_crater() {
        let mut c = Craft::new();
        while c.landed.is_none() {
            c.step(1.0);
        }
        let v = c.landed.unwrap();
        assert!(v > 3900.0 && v < 4100.0, "{v}");
        assert!((c.t - 113.0).abs() < 3.0, "{}", c.t);
    }

    #[test]
    fn gravity_adds_36_mph_every_ten_seconds() {
        let mut c = Craft::new();
        c.step(10.0);
        assert!((c.mph() - 3636.0).abs() < 0.5, "{}", c.mph());
    }

    #[test]
    fn the_autopilot_lands_softly() {
        let mut c = Craft::new();
        while c.landed.is_none() {
            c.burn = c.autopilot();
            c.step(0.2);
        }
        let v = c.landed.unwrap();
        assert!(v <= 10.0, "landed at {v} mph with {} lb left", c.fuel);
    }

    #[test]
    fn no_burn_below_ignition() {
        let mut c = Craft::new();
        c.burn = 5.0;
        assert_eq!(c.effective_burn(), 0.0);
    }
}

#[cfg(test)]
mod trace {
    use super::*;
    #[test]
    #[ignore]
    fn print_autopilot() {
        let mut c = Craft::new();
        let mut n = 0;
        while c.landed.is_none() {
            c.burn = c.autopilot();
            c.step(0.2);
            n += 1;
            if n % 50 == 0 {
                println!(
                    "t {:.0} alt {:.3} mph {:.0} fuel {:.0} burn {:.0}",
                    c.t,
                    c.altitude,
                    c.mph(),
                    c.fuel,
                    c.burn
                );
            }
        }
        println!("{:?}", c.landed);
    }
}
