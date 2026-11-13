//! Orbit: a cloaked raider circles your planet; each hour you get one
//! bomb, aimed by angle and distance, and learn only how far from the ship
//! it burst. Rules in asciiverse/specs/orbit.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::symbols::Marker;
use ratatui::widgets::canvas::{Canvas, Circle, Line as CLine, Points};
use ratatui::widgets::Widget;

/// Distances are in hundreds of miles.
pub const MIN_R: f64 = 100.0;
pub const MAX_R: f64 = 300.0;
pub const PLANET_R: f64 = 50.0;
pub const KILL: f64 = 50.0;
pub const HOURS: u32 = 7;
pub const MIN_PERIOD: f64 = 12.0;
pub const MAX_PERIOD: f64 = 36.0;

#[derive(Clone, Copy, Debug)]
pub struct Ship {
    pub r: f64,
    /// degrees an hour, counterclockwise
    pub rate: f64,
    /// degrees at hour 1
    pub a0: f64,
}

impl Ship {
    pub fn random(rng: &mut Rng) -> Ship {
        let period = rng.between(MIN_PERIOD as f32, MAX_PERIOD as f32) as f64;
        Ship {
            r: rng.range(MIN_R as i64, MAX_R as i64) as f64,
            rate: 360.0 / period,
            a0: rng.range(0, 359) as f64,
        }
    }

    /// Where it is at an hour (1 = the first).
    pub fn angle(&self, hour: u32) -> f64 {
        (self.a0 + self.rate * (hour as f64 - 1.0)).rem_euclid(360.0)
    }

    pub fn xy(&self, hour: u32) -> (f64, f64) {
        polar(self.r, self.angle(hour))
    }
}

pub fn polar(r: f64, deg: f64) -> (f64, f64) {
    let t = deg.to_radians();
    (r * t.cos(), r * t.sin())
}

/// Distance between a burst and the ship, by the law of cosines.
pub fn miss(ship_r: f64, ship_a: f64, bomb_r: f64, bomb_a: f64) -> f64 {
    let d = (ship_a - bomb_a).to_radians();
    (ship_r * ship_r + bomb_r * bomb_r - 2.0 * ship_r * bomb_r * d.cos())
        .max(0.0)
        .sqrt()
}

#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub hour: u32,
    pub angle: f64,
    pub dist: f64,
    pub miss: f64,
}

/// A hunter's guess at where the ship will be next hour, from the shots
/// so far: every orbit that fits them all, averaged by angle.
pub fn hunt(shots: &[Shot], hour: u32) -> (f64, f64) {
    if shots.is_empty() {
        return (200.0, 90.0);
    }
    let mut best: Vec<Ship> = Vec::new();
    let mut tol = 1.5;
    while best.is_empty() && tol < 40.0 {
        let mut r = MIN_R;
        while r <= MAX_R {
            let mut p = MIN_PERIOD;
            while p <= MAX_PERIOD {
                let rate = 360.0 / p;
                let mut a = 0.0;
                while a < 360.0 {
                    let s = Ship { r, rate, a0: a };
                    if shots.iter().all(|sh| {
                        (miss(s.r, s.angle(sh.hour), sh.dist, sh.angle) - sh.miss).abs() < tol
                    }) {
                        best.push(s);
                    }
                    a += 2.0;
                }
                p += 0.5;
            }
            r += 2.0;
        }
        tol *= 2.0;
    }
    if best.is_empty() {
        return (200.0, 0.0);
    }
    let (mut sx, mut sy, mut sr) = (0.0, 0.0, 0.0);
    for s in &best {
        let (x, y) = s.xy(hour);
        sx += x;
        sy += y;
        sr += s.r;
    }
    let n = best.len() as f64;
    let ang = (sy / n).atan2(sx / n).to_degrees().rem_euclid(360.0);
    (sr / n, ang)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Aiming,
    Hit,
    Escaped,
}

pub struct Orbit {
    ctx: Ctx,
    ship: Ship,
    hour: u32,
    shots: Vec<Shot>,
    phase: Phase,
    angle: NumField,
    dist: NumField,
    focus: usize,
    fresh: bool,
    plot: Rect,
    flash: f32,
    record: Kv,
    quit: bool,
    demo_clock: f32,
    demo_aim: Option<(f64, f64)>,
}

impl Orbit {
    fn new_hunt(&mut self) {
        self.ship = Ship::random(&mut self.ctx.rng);
        self.hour = 1;
        self.shots.clear();
        self.phase = Phase::Aiming;
        self.angle.set(90);
        self.dist.set(200);
        self.focus = 0;
        self.fresh = true;
        log::info!(
            "new raider r {} rate {:.2} a0 {}",
            self.ship.r,
            self.ship.rate,
            self.ship.a0
        );
    }

    fn fire(&mut self) {
        if self.phase != Phase::Aiming || !self.angle.valid() || !self.dist.valid() {
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        let a = self.angle.value() as f64;
        let d = self.dist.value() as f64;
        let m = miss(self.ship.r, self.ship.angle(self.hour), d, a);
        self.shots.push(Shot {
            hour: self.hour,
            angle: a,
            dist: d,
            miss: m,
        });
        self.flash = 0.6;
        log::info!("hour {} bomb at {a} deg {d}: missed by {m:.1}", self.hour);
        if m <= KILL {
            self.phase = Phase::Hit;
            self.ctx.audio.play(Sound::Boom);
            self.ctx.audio.play(Sound::Win);
            self.record_result(true);
        } else if self.hour >= HOURS {
            self.phase = Phase::Escaped;
            self.ctx.audio.play(Sound::Lose);
            self.record_result(false);
        } else {
            self.hour += 1;
            self.ctx
                .audio
                .play(Sound::Beep(300.0 + (2000.0 / (m / 10.0).max(1.0)) as f32));
        }
    }

    fn record_result(&mut self, won: bool) {
        if self.ctx.demo {
            return;
        }
        let k = if won { "destroyed" } else { "escaped" };
        let n = self.record.get::<u32>(k).unwrap_or(0) + 1;
        self.record.set(k, n);
        self.record.save(Self::ID, "record");
    }

    /// Plot coordinates (hundreds of miles) of a screen cell, if inside.
    fn plot_point(&self, x: u16, y: u16) -> Option<(f64, f64)> {
        let p = self.plot;
        if !hit(&p, x, y) {
            return None;
        }
        let (xb, yb) = bounds(p);
        let fx = (x - p.x) as f64 + 0.5;
        let fy = (y - p.y) as f64 + 0.5;
        let px = xb.0 + (xb.1 - xb.0) * fx / p.width as f64;
        let py = yb.1 - (yb.1 - yb.0) * fy / p.height as f64;
        Some((px, py))
    }
}

/// Canvas bounds that keep circles round: a cell is about twice as tall as wide.
fn bounds(r: Rect) -> ((f64, f64), (f64, f64)) {
    let half_h = 330.0;
    let half_w = half_h * r.width as f64 / (r.height as f64 * 2.0);
    ((-half_w, half_w), (-half_h, half_h))
}

impl Game for Orbit {
    const ID: &'static str = "orbit";
    const TITLE: &'static str = "Orbit";
    const HELP: &'static str = "A cloaked raider circles your planet, counterclockwise, somewhere
between 10,000 and 30,000 miles out, once every 12 to 36 hours. In seven
hours it will have the power to escape. Each hour you fire one bomb, set
by an angle (0 to 360 degrees, 0 to the east, 90 straight up) and a
distance (100 to 300, in hundreds of miles), and learn how far from the
raider it burst. A burst within 5,000 miles destroys it.
Click the plot to aim, or type: Tab moves between angle and distance,
Up/Down change them, Enter fires.";

    fn new(ctx: Ctx) -> Self {
        let mut ctx = ctx;
        let ship = Ship::random(&mut ctx.rng);
        let mut g = Orbit {
            ctx,
            ship,
            hour: 1,
            shots: Vec::new(),
            phase: Phase::Aiming,
            angle: NumField::new("angle", 90, 0, 360, 5),
            dist: NumField::new("distance", 200, 100, 300, 5),
            focus: 0,
            fresh: true,
            plot: Rect::default(),
            flash: 0.0,
            record: Kv::load("orbit", "record"),
            quit: false,
            demo_clock: 0.0,
            demo_aim: None,
        };
        g.new_hunt();
        g
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Enter if self.phase != Phase::Aiming => self.new_hunt(),
            Key::Enter | Key::Char(' ') => self.fire(),
            Key::Tab | Key::BackTab | Key::Left | Key::Right => {
                self.focus = 1 - self.focus;
                self.fresh = true;
            }
            other => {
                let mut fresh = self.fresh;
                let f = if self.focus == 0 {
                    &mut self.angle
                } else {
                    &mut self.dist
                };
                if f.key(&other, &mut fresh) {
                    self.fresh = fresh;
                }
            }
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if self.ctx.demo {
            return;
        }
        let Some((x, y)) = self.plot_point(m.x, m.y) else {
            return;
        };
        let r = (x * x + y * y).sqrt();
        let a = y.atan2(x).to_degrees().rem_euclid(360.0);
        match m.kind {
            MouseKind::Move if self.phase == Phase::Aiming => {
                self.angle.set(a.round() as i64);
                self.dist.set(r.round() as i64);
            }
            MouseKind::Click => {
                if self.phase != Phase::Aiming {
                    self.new_hunt();
                } else {
                    self.angle.set(a.round() as i64);
                    self.dist.set(r.round() as i64);
                    self.fire();
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        if self.phase != Phase::Aiming {
            if self.demo_clock > 4.0 {
                self.demo_clock = 0.0;
                self.new_hunt();
                self.demo_aim = None;
            }
            return;
        }
        if self.demo_aim.is_none() {
            let (r, a) = hunt(&self.shots, self.hour);
            // the first shot goes where a ship is most likely: mid-range, anywhere
            let (r, a) = if self.shots.is_empty() {
                (200.0, self.ctx.rng.range(0, 359) as f64)
            } else {
                (r, a)
            };
            self.demo_aim = Some((r.clamp(MIN_R, MAX_R), a));
            self.demo_clock = 0.0;
        }
        let (r, a) = self.demo_aim.unwrap();
        // swing the aim across, then fire
        let k = (self.demo_clock / 1.4).min(1.0) as f64;
        let ca = self.angle.value() as f64;
        let cr = self.dist.value() as f64;
        let mut da = a - ca;
        if da > 180.0 {
            da -= 360.0;
        }
        if da < -180.0 {
            da += 360.0;
        }
        self.angle
            .set((ca + da * k * 0.3).rem_euclid(360.0).round() as i64);
        self.dist.set((cr + (r - cr) * k * 0.3).round() as i64);
        if self.demo_clock > 2.2 {
            self.angle.set(a.round() as i64);
            self.dist.set(r.round() as i64);
            self.fire();
            self.demo_aim = None;
            self.demo_clock = 0.0;
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "hour {} phase {:?} aim {} {} shots {}",
            self.hour,
            self.phase,
            self.angle.text,
            self.dist.text,
            self.shots.len()
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let status = match self.phase {
            Phase::Aiming => format!("hour {} of {HOURS}", self.hour),
            Phase::Hit => "raider destroyed".into(),
            Phase::Escaped => "the raider escaped".into(),
        };
        let keys: &[(&str, &str)] = if self.phase == Phase::Aiming {
            &[
                ("mouse", "aim/fire"),
                ("Tab", "angle/distance"),
                ("↑↓", "change"),
                ("Enter", "fire"),
                ("M", "sound"),
                ("Esc", "quit"),
            ]
        } else {
            &[("Enter", "new hunt"), ("Esc", "quit")]
        };
        let inner = frame(buf, area, "Orbit", &status, keys);
        if too_small(buf, inner, 72, 22) {
            return;
        }
        let side_w = 32.min(inner.width / 3);
        let plot_r = Rect {
            width: inner.width - side_w,
            ..inner
        };
        let side = Rect {
            x: inner.x + plot_r.width,
            width: side_w,
            ..inner
        };

        let p = panel(buf, plot_r, "Your planet, from above", AMBER);
        self.plot = p;
        let (xb, yb) = bounds(p);
        let shots = self.shots.clone();
        let over = self.phase != Phase::Aiming;
        let ship = self.ship;
        let hour = self.hour;
        let aim = polar(self.dist.value() as f64, self.angle.value() as f64);
        let flash = self.flash;
        Canvas::default()
            .background_color(BG)
            .marker(Marker::Braille)
            .x_bounds([xb.0, xb.1])
            .y_bounds([yb.0, yb.1])
            .paint(|ctx| {
                for r in [100.0, 200.0, 300.0] {
                    ctx.draw(&Circle {
                        x: 0.0,
                        y: 0.0,
                        radius: r,
                        color: Color::Rgb(44, 44, 52),
                    });
                }
                ctx.draw(&CLine {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 320.0,
                    y2: 0.0,
                    color: Color::Rgb(44, 44, 52),
                });
                // the planet, filled with rings
                let mut r = PLANET_R;
                while r > 0.0 {
                    ctx.draw(&Circle {
                        x: 0.0,
                        y: 0.0,
                        radius: r,
                        color: Color::Rgb(60, 110, 170),
                    });
                    r -= 6.0;
                }
                ctx.layer();
                // each burst, and the ring of places the raider could have been
                for (i, s) in shots.iter().enumerate() {
                    let (x, y) = polar(s.dist, s.angle);
                    let latest = i + 1 == shots.len();
                    let c = if latest && flash > 0.0 {
                        AMBER
                    } else {
                        mix(RED, BG, 0.25)
                    };
                    ctx.draw(&Circle {
                        x,
                        y,
                        radius: s.miss,
                        color: if latest {
                            mix(AMBER, BG, 0.3)
                        } else {
                            Color::Rgb(70, 50, 40)
                        },
                    });
                    ctx.draw(&Circle {
                        x,
                        y,
                        radius: 6.0,
                        color: c,
                    });
                }
                if !over {
                    let (x, y) = aim;
                    ctx.draw(&CLine {
                        x1: x - 10.0,
                        y1: y,
                        x2: x + 10.0,
                        y2: y,
                        color: GREEN,
                    });
                    ctx.draw(&CLine {
                        x1: x,
                        y1: y - 10.0,
                        x2: x,
                        y2: y + 10.0,
                        color: GREEN,
                    });
                    ctx.draw(&Circle {
                        x,
                        y,
                        radius: KILL,
                        color: mix(GREEN, BG, 0.5),
                    });
                } else {
                    // the raider's true path through the hunt
                    let pts: Vec<(f64, f64)> =
                        (0..=720).map(|i| polar(ship.r, i as f64 / 2.0)).collect();
                    ctx.draw(&Points {
                        coords: &pts,
                        color: Color::Rgb(90, 90, 110),
                    });
                    for h in 1..=hour {
                        let (x, y) = ship.xy(h);
                        ctx.draw(&Circle {
                            x,
                            y,
                            radius: 5.0,
                            color: if h == hour {
                                VIOLET
                            } else {
                                mix(VIOLET, BG, 0.5)
                            },
                        });
                    }
                }
            })
            .render(p, buf);
        put(
            buf,
            p.right().saturating_sub(5),
            p.y + p.height / 2,
            "0°",
            st(DIM),
        );
        put(buf, p.x + p.width / 2 - 1, p.y, "90°", st(DIM));

        let s = panel(
            buf,
            Rect {
                height: 9.min(side.height),
                ..side
            },
            "Next bomb",
            AMBER,
        );
        if self.phase == Phase::Aiming {
            self.angle.draw(buf, s.x + 2, s.y, 10, 8, self.focus == 0);
            self.dist
                .draw(buf, s.x + 2, s.y + 2, 10, 8, self.focus == 1);
            put(buf, s.x + 2, s.y + 3, "×100 miles", st(DIM));
            put(
                buf,
                s.x,
                s.y + 5,
                &format!("{} bombs left", HOURS - self.hour + 1),
                st(INK),
            );
        } else if self.phase == Phase::Hit {
            para(buf, s, &format!("Hour {}: the bomb burst {:.0} miles from the raider and tore it apart. The planet is safe!", self.hour, self.shots.last().map_or(0.0, |x| x.miss * 100.0)), bold(st(GREEN)));
        } else {
            para(buf, s, "Seven hours gone: the raider has its power and slips away into deep space. Its path is drawn in violet.", bold(st(RED)));
        }
        let l = panel(
            buf,
            Rect {
                y: side.y + 9,
                height: side.height.saturating_sub(9),
                ..side
            },
            "Bursts",
            CYAN,
        );
        put(buf, l.x, l.y, "hr  angle  dist   missed by", st(DIM));
        for (i, sh) in self.shots.iter().enumerate() {
            let y = l.y + 1 + i as u16;
            if y < l.bottom() {
                let c = if sh.miss <= KILL {
                    GREEN
                } else if sh.miss < 100.0 {
                    AMBER
                } else {
                    INK
                };
                put(
                    buf,
                    l.x,
                    y,
                    &format!(
                        "{:>2}  {:>5.0}  {:>4.0}   {:>6.0} mi",
                        sh.hour,
                        sh.angle,
                        sh.dist,
                        sh.miss * 100.0
                    ),
                    st(c),
                );
            }
        }
        let y = l.y + 2 + HOURS as u16;
        if y + 1 < l.bottom() {
            let d = self.record.get::<u32>("destroyed").unwrap_or(0);
            let e = self.record.get::<u32>("escaped").unwrap_or(0);
            put(buf, l.x, y, &format!("destroyed {d}, escaped {e}"), st(DIM));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn law_of_cosines() {
        assert!((miss(200.0, 0.0, 200.0, 0.0)).abs() < 1e-9);
        assert!((miss(100.0, 0.0, 100.0, 180.0) - 200.0).abs() < 1e-9);
        assert!((miss(300.0, 90.0, 400.0, 0.0) - 500.0).abs() < 1e-9);
    }

    #[test]
    fn a_careful_hunter_usually_wins() {
        let mut rng = Rng::new(9);
        let mut won = 0;
        for _ in 0..12 {
            let ship = Ship::random(&mut rng);
            let mut shots = Vec::new();
            for hour in 1..=HOURS {
                let (r, a) = if shots.is_empty() {
                    (200.0, rng.range(0, 359) as f64)
                } else {
                    hunt(&shots, hour)
                };
                let r = r.clamp(MIN_R, MAX_R);
                let m = miss(ship.r, ship.angle(hour), r, a);
                shots.push(Shot {
                    hour,
                    angle: a,
                    dist: r,
                    miss: m,
                });
                if m <= KILL {
                    won += 1;
                    break;
                }
            }
        }
        assert!(won >= 9, "won {won} of 12");
    }
}
